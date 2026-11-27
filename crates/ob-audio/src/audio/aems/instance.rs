use super::*;

/// One running event.
#[derive(Clone, Debug)]
pub struct Instance {
    pub(super) program: Arc<Program>,
    pub mem: Vec<u8>,
    pub dead: bool,
    /// What nodes 38 / 39 sent to their named outputs since the host last
    /// took them: (block offset, parameters; empty for a release).
    pub messages: Vec<(i32, Vec<i32>)>,
}

/// The quarter sine of 0x7254CF (0x8181D8, 257 u16), computed.
pub(super) use crate::exe_tables::generated::quarter_sine;

/// 0x7254CF: a full sine of a 10-bit phase, ±65535.
pub(super) fn sine(p: i32) -> i32 {
    let i = (p & 0xFF) as usize;
    match (p >> 8) & 3 {
        0 => quarter_sine()[i] as i32,
        1 => quarter_sine()[256 - i] as i32,
        2 => -(quarter_sine()[i] as i32),
        _ => -(quarter_sine()[256 - i] as i32),
    }
}

/// x87 `fistp` in the default rounding mode (nearest, ties to even).
pub(super) fn fist(x: f64) -> i32 {
    x.round_ties_even() as i32
}

impl Instance {
    pub fn new(program: Arc<Program>) -> Instance {
        let mem = program.template.clone();
        Instance { program, mem, dead: false, messages: Vec::new() }
    }

    pub(super) fn rd(&self, o: i32) -> i32 {
        let o = o as usize;
        self.mem.get(o..o + 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()))
    }
    pub(super) fn wr(&mut self, o: i32, v: i32) {
        let o = o as usize;
        if let Some(b) = self.mem.get_mut(o..o + 4) {
            b.copy_from_slice(&v.to_le_bytes());
        }
    }
    pub(super) fn rf(&self, o: i32) -> f32 {
        f32::from_bits(self.rd(o) as u32)
    }
    pub(super) fn wf(&mut self, o: i32, v: f32) {
        self.wr(o, v.to_bits() as i32);
    }
    pub(super) fn rb(&self, o: i32) -> u8 {
        self.mem.get(o as usize).copied().unwrap_or(0)
    }
    pub(super) fn wb(&mut self, o: i32, v: u8) {
        if let Some(b) = self.mem.get_mut(o as usize) {
            *b = v;
        }
    }
    pub(super) fn rw(&self, o: i32) -> u16 {
        let o = o as usize;
        self.mem.get(o..o + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
    }
    pub(super) fn ww(&mut self, o: i32, v: u16) {
        let o = o as usize;
        if let Some(b) = self.mem.get_mut(o..o + 2) {
            b.copy_from_slice(&v.to_le_bytes());
        }
    }

    /// A node 38 / 39 message; the last 32 are kept for the host.
    pub(super) fn send(&mut self, b: i32, params: Vec<i32>) {
        if self.messages.len() >= 32 {
            self.messages.remove(0);
        }
        self.messages.push((b, params));
    }

    /// The game's parameter block (0x721046: the first byte +0x10 dwords
    /// to +0x14 on). Event parameters are its dwords from the event +4.
    pub fn set_params(&mut self, block: &[i32]) {
        let Some(p) = self.program.params else { return };
        let p = p as i32;
        let n = self.rb(p + 0x10) as usize;
        for (k, &v) in block.iter().take(n).enumerate() {
            self.wr(p + 0x14 + 4 * k as i32, v);
        }
    }

    /// The game's parameter `k` as last set ([`Instance::set_params`]).
    pub fn param(&self, k: usize) -> Option<i32> {
        let p = self.program.params? as i32;
        (k < self.rb(p + 0x10) as usize).then(|| self.rd(p + 0x14 + 4 * k as i32))
    }

    /// The handle released (0x72103A: release node +0x10 = 1).
    pub fn release(&mut self) {
        if let Some(r) = self.program.release {
            self.wr(r as i32 + 0x10, 1);
        }
    }

    /// Stop every voice (node 4's loop, 0x71FFC4).
    pub fn stop(&mut self, host: &mut dyn VoiceHost) {
        for k in 0..self.program.players.len() {
            let p = self.program.players[k] as i32;
            if (self.rb(p + 0x10) as i8) >= 0 {
                host.stop(self.rd(p + 8) as u32);
                self.reset_player(p);
            }
        }
    }

    /// One library tick: the module's code.
    pub fn tick(&mut self, host: &mut dyn VoiceHost, rng: &mut Rng) {
        let prog = self.program.clone();
        let (mut eax, mut ecx, mut esi) = (0i32, 0i32, 0x18i32);
        let mut cmp = std::cmp::Ordering::Equal;
        let mut pc = 0;
        while let Some(&op) = prog.ops.get(pc) {
            pc += 1;
            match op {
                Op::Call(n) => eax = self.node(n, esi, host, rng),
                Op::Store(d) => self.wr(esi + d, eax),
                Op::StoreByte(d, v) => self.wb(esi + d, v),
                Op::Load(d) => eax = self.rd(esi + d),
                Op::LoadEcx(d) => ecx = self.rd(esi + d),
                Op::Add(d) => eax = eax.wrapping_add(self.rd(esi + d)),
                Op::Sub(d) => eax = eax.wrapping_sub(self.rd(esi + d)),
                Op::Imul(d) => eax = eax.wrapping_mul(self.rd(esi + d)),
                Op::MovEcx => eax = ecx,
                Op::Cmp => cmp = eax.cmp(&ecx),
                Op::Jl(t) => {
                    if cmp.is_lt() {
                        pc = t;
                    }
                }
                Op::Jg(t) => {
                    if cmp.is_gt() {
                        pc = t;
                    }
                }
                Op::AddEsi(k) => esi += k,
            }
        }
    }
}

impl Instance {
    /// A sound player's (state, list index, parameter values) for probes.
    pub fn player_state(&self, k: usize) -> Option<(i32, i32, Vec<(u8, i32)>)> {
        let b = *self.program.players.get(k)? as i32;
        let params = (0..self.rb(b + 0xE) as i32)
            .map(|i| (self.rb(b + 0x1C + 12 * i), self.rd(b + 0x1C + 12 * i + 8)))
            .collect();
        Some((self.rd(b + 0x18), self.rd(b + 0x14), params))
    }
}
