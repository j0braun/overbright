use super::*;

/// A module ready to instance.
#[derive(Debug)]
pub struct Program {
    pub ops: Vec<Op>,
    pub(super) template: Vec<u8>,
    /// The bank file (the template's pointers are file offsets: the
    /// loader adds the bank's base, 0x72135C).
    pub(super) file: Arc<[u8]>,
    /// The sound players' block offsets (record +0x3C on, 0x721414).
    pub players: Vec<usize>,
    pub(super) release: Option<usize>,
    pub(super) params: Option<usize>,
    /// Record u16 +0x1E: the most instances at once (0x7210A4).
    pub max: u16,
}

impl Program {
    pub fn new(bank: &Bank, file: Arc<[u8]>, module: usize) -> Option<Program> {
        let m = bank.modules.get(module)?;
        let d = &bank.data;
        let rec = m.record;
        let ops = decode(d.get(read_u32(d, rec + 0x28) as usize..)?)?;
        let template = d.get(m.code.1..m.code.1 + m.size)?.to_vec();
        // 0x721096's walk of the listener nodes from +0x18.
        let mut at = 0x18;
        let release = (d[rec + 0x25] != 0).then(|| {
            at += 0x14;
            at - 0x14
        });
        at += 0x1C * read_u16(d, rec + 0x20) as usize;
        let params = (d[rec + 0x26] != 0).then_some(at);
        Some(Program { ops, template, file, players: m.players.clone(), release, params, max: read_u16(d, rec + 0x1E) })
    }

    pub(super) fn file_u32(&self, at: i32) -> i32 {
        let a = at as usize;
        if a + 4 <= self.file.len() { read_u32(&self.file, a) as i32 } else { 0 }
    }
}

/// The start block of a voice (0x721A6B's defaults, the setters 0x817F60).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StartBlock {
    /// +0: volume byte (kind 2, value >> 8), default 0x7F.
    pub volume: u8,
    /// +4: the second volume byte (kind 8, value >> 8), default 0x7F.
    pub volume2: u8,
    /// +5: the effect send byte (kind 5), default 0.
    pub send: u8,
    /// +8: kind 3 (the azimuth), default 0.
    pub azimuth: u16,
    /// +0xA: kind 4 (−0x4000..0x3FFF), default 0.
    pub spread: i16,
    /// +0xC: kind 0, the pitch (0x1000 = the recorded rate), default 0x1000.
    pub pitch: u16,
    /// +0xE: kind 1 (0x800..0x2000), default 0x1000.
    pub pitch2: u16,
}

impl Default for StartBlock {
    fn default() -> Self {
        StartBlock { volume: 0x7F, volume2: 0x7F, send: 0, azimuth: 0, spread: 0, pitch: 0x1000, pitch2: 0x1000 }
    }
}

impl StartBlock {
    /// 0x817F60[kind].
    pub(super) fn set(&mut self, kind: u8, v: i32) {
        match kind {
            0 => self.pitch = v.clamp(0, 0xFFFF) as u16,
            1 => self.pitch2 = v.clamp(0x800, 0x2000) as u16,
            2 => self.volume = (v.clamp(0, 0x7FFF) >> 8) as u8,
            3 => self.azimuth = v as u16,
            4 => self.spread = v.clamp(-0x4000, 0x3FFF) as i16,
            5 => self.send = (v.clamp(0, 0x7FFF) >> 8) as u8,
            8 => self.volume2 = (v.clamp(0, 0x7FFF) >> 8) as u8,
            _ => {}
        }
    }
}

/// What the graphs' sound players drive (the library's voices).
pub trait VoiceHost {
    /// Start bank sample `slot` (its BNKl index) with a start block
    /// (0x724333); `None` when it cannot.
    fn start(&mut self, slot: u32, block: &StartBlock) -> Option<u32>;
    /// 0x723E1E.
    fn stop(&mut self, voice: u32);
    /// 0x725480 ≥ 0: the voice still plays.
    fn alive(&mut self, voice: u32) -> bool;
    /// A live parameter (0x817F30[kind]): 0 the pitch (0..0xFFFF, 0x1000
    /// = 1), 2 the volume (0..0x7FFF, gain v / 32767 at 0x72085B), 3 the
    /// azimuth, …
    fn set(&mut self, voice: u32, kind: u8, value: i32);
    /// The release (state 2, 0x720BCD): 0x72412A(voice, 0) zeroes the
    /// voice's gain (+0x38) and 0x723D9B(voice, 0) its +0x8C.
    fn release(&mut self, voice: u32);
    /// (elapsed ms, remaining ms; 0 when looping) for players with +0xF
    /// set (0x720CDA); none of the car banks set it.
    fn position(&mut self, _voice: u32) -> (i32, i32) {
        (0, 0)
    }
}
