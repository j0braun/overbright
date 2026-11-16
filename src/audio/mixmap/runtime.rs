use super::*;

/// What the mixer reads from the game: the object fields its ids name
/// (type 0x4 sound objects via 0x468F70, 0x6 controllers via 0x468FE0:
/// object +4 + 4 · (id & 0xF); the group bits 16–23, the class bits 4–10
/// (0x7F), the instance bits 11–15), the view mode 0x82BB48 and the
/// pause flag ([0x82B884]+0x14).
pub trait World {
    /// The field `id & 0xF` of the object `id` names in `copy` of
    /// `section`; `None` when the object does not exist.
    fn field(&self, section: usize, copy: usize, id: u32) -> Option<i32>;
    fn view(&self) -> u32 {
        0
    }
    fn paused(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct EvtState {
    /// +0x24 the timer (ms, f32), +0x28 the release start, +0x1C the
    /// envelope, byte +4 the latch, +8 the latched value, +0x18 the output.
    pub(super) timer: f32,
    pub(super) rel: f32,
    pub(super) env: i32,
    pub(super) latch: bool,
    pub(super) held: i32,
    pub(super) out: i32,
}

#[derive(Clone, Debug)]
pub(super) struct TdState {
    pub(super) variant: usize,
    pub(super) az: i32,
    pub(super) mb: i32,
}

/// One copy's node values.
#[derive(Clone, Debug, Default)]
pub(super) struct Copy {
    pub(super) dp: Vec<i32>,
    pub(super) ctl: Vec<i32>,
    pub(super) evt: Vec<EvtState>,
    pub(super) td: Vec<TdState>,
    pub(super) sub: Vec<i32>,
    pub(super) master: Vec<i32>,
    /// Per object class (OutID bits 4–10): its 16-dword block.
    pub(super) out: HashMap<u32, [u32; 16]>,
    /// Per object class: slot → block dword (the binder 0x480420).
    pub(super) slots: HashMap<u32, [Option<u8>; 16]>,
}

/// The running mix: copies per section.
#[derive(Clone, Debug, Default)]
pub struct Mix {
    pub map: MixMap,
    pub(super) copies: Vec<Vec<Copy>>,
}

/// One object instance's slots as the sound objects read them: per
/// object class (bits 4–10 of its id) eight packed dwords (slot 0x38 the
/// level = bits 0–14, slot 0x3C the azimuth = bits 16–31). The default
/// (no mix map) is every level 0x7FFF at azimuth 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels(pub [[u32; 8]; 16]);

impl Default for Levels {
    fn default() -> Self {
        Levels([[0x7FFF; 8]; 16])
    }
}

impl Levels {
    /// Slot 0x38 (0x45E390): [S] & 0x7FFF.
    pub fn level(&self, k: usize, slot: usize) -> i32 {
        (self.0[k & 15][slot & 7] & 0x7FFF) as i32
    }
    /// Slot 0x3C (0x45E3B0): WORD [S+2].
    pub fn az(&self, k: usize, slot: usize) -> i32 {
        (self.0[k & 15][slot & 7] >> 16) as i32
    }
}

/// 0x480910's default input (0x7EDE80).
pub(super) const FULL: i32 = 0x7FFF;

impl Mix {
    pub fn new(map: MixMap) -> Mix {
        let mut m = Mix { map, copies: Vec::new() };
        m.copies = vec![Vec::new(); m.map.sections.len()];
        for s in 0..m.copies.len() {
            m.set_copies(s, 1);
        }
        m
    }

    /// Make `n` copies of section `s` (one per object instance; 0x4837C0).
    pub fn set_copies(&mut self, s: usize, n: usize) {
        let Some(sec) = self.map.sections.get(s) else {
            return;
        };
        let fresh = || {
            let mut c = Copy {
                dp: vec![0; sec.controls.len()],
                ctl: vec![0; sec.controls.len()],
                evt: vec![EvtState::default(); sec.evts.len()],
                td: sec.threed.iter().map(|_| TdState { variant: 0, az: 0, mb: -10000 }).collect(),
                sub: vec![0; sec.subs.len()],
                master: vec![0; sec.masters.len()],
                ..Default::default()
            };
            for m in &sec.masters {
                let k = (m.out >> 4) & 0x7F;
                c.out.entry(k).or_insert([0; 16]);
                let slots = c.slots.entry(k).or_insert([None; 16]);
                for w in &m.map {
                    slots[((w >> 16) & 0xF) as usize] = Some(((w >> 24) & 0xF) as u8);
                }
            }
            c
        };
        let v = &mut self.copies[s];
        v.truncate(n);
        while v.len() < n {
            v.push(fresh());
        }
    }

    pub fn copies(&self, s: usize) -> usize {
        self.copies.get(s).map_or(0, Vec::len)
    }

    /// The packed dword object class `k` of `copy` in section `s` reads at
    /// `slot` (slot 0x38 = its low 15 bits, slot 0x3C = its high word);
    /// `None` when the map gives that object no such slot.
    pub fn packed(&self, s: usize, copy: usize, k: u32, slot: usize) -> Option<u32> {
        let c = self.copies.get(s)?.get(copy)?;
        let i = (*c.slots.get(&k)?.get(slot)?)?;
        Some(c.out.get(&k)?[i as usize])
    }

    /// Every object's slots of one copy (a slot the map does not bind
    /// reads 0: S is null, 0x45E390 / 0x45E3B0 return 0).
    pub fn levels(&self, s: usize, copy: usize) -> Levels {
        let mut l = Levels([[0; 8]; 16]);
        for k in 0..16 {
            for slot in 0..8 {
                l.0[k][slot] = self.packed(s, copy, k as u32, slot).unwrap_or(0);
            }
        }
        l
    }

    /// Slot 0x38 (0x45E390): the level, 0..0x7FFF (0 with no slot).
    pub fn level(&self, s: usize, copy: usize, k: u32, slot: usize) -> i32 {
        self.packed(s, copy, k, slot).map_or(0, |v| (v & 0x7FFF) as i32)
    }

    /// Slot 0x3C (0x45E3B0): the azimuth word.
    pub fn azimuth(&self, s: usize, copy: usize, k: u32, slot: usize) -> u16 {
        self.packed(s, copy, k, slot).map_or(0, |v| (v >> 16) as u16)
    }

    /// A field input (0x480910 for controls and event controls; 0x4809E0
    /// for data procs): 3D (0x8) the 3D control's mB, 0x4 / 0x6 the
    /// object's field, otherwise (and for a missing object) 0x7FFF.
    fn input(&self, w: &dyn World, s: usize, copy: usize, id: u32) -> i32 {
        match id & 0xE000_0000 {
            0x8000_0000 => self.copies[s][copy].td.get((id & 0xFF) as usize).map_or(FULL, |t| t.mb),
            0x4000_0000 | 0x6000_0000 => w.field(s, copy, id).unwrap_or(FULL),
            _ => FULL,
        }
    }

    /// A data proc's own input (0x4809E0 on its id).
    fn dp_input(&self, w: &dyn World, s: usize, copy: usize, id: u32) -> i32 {
        let c = &self.copies[s][copy];
        match id & 0xE000_0000 {
            0x2000_0000 => {
                let t = ((id >> 16) & 0xFF) as usize;
                let Some(c0) = self.copies.get(t).and_then(|v| v.first()) else {
                    return 0;
                };
                let list = if id & 0x1_0000 != 0 { &c0.sub } else { &c0.master };
                list.get((id & 0xFF) as usize).copied().unwrap_or(0)
            }
            0x8000_0000 => c.td.get((id & 0xFF) as usize).map_or(0, |t| t.mb),
            0 => c.ctl.get((id & 0xFF) as usize).copied().unwrap_or(0),
            0x4000_0000 | 0x6000_0000 => w.field(s, copy, id).unwrap_or(0),
            _ => 0,
        }
    }

    /// A channel input word (0x478240): bits 28–31 the kind (2 a master,
    /// 3 a sub channel, 0 a control, 0xA / 0xB an event control), bits
    /// 16–23 the section, bits 0–7 the node. The own section reads the own
    /// copy ("Local"); another section is summed over all its copies
    /// ("Ext", one input per copy).
    pub(super) fn chan_input(&self, s: usize, copy: usize, word: u32) -> i32 {
        let t = ((word >> 16) & 0xFF) as usize;
        let i = (word & 0xFF) as usize;
        let read = |c: &Copy| -> i32 {
            match word >> 28 {
                2 => c.master.get(i).copied().unwrap_or(0),
                3 => c.sub.get(i).copied().unwrap_or(0),
                0 => c.ctl.get(i).copied().unwrap_or(0),
                0xA | 0xB => c.evt.get(i).map_or(0, |e| e.out),
                _ => 0,
            }
        };
        if t == s {
            self.copies[s].get(copy).map_or(0, read)
        } else {
            self.copies.get(t).map_or(0, |v| v.iter().map(read).sum())
        }
    }

    /// One mixer pass (0x478FE0 with `dt` seconds): every section's
    /// copies in order.
    pub fn update(&mut self, w: &dyn World, dt: f32) {
        let map = std::mem::take(&mut self.map);
        for (s, sec) in map.sections.iter().enumerate() {
            for copy in 0..self.copies[s].len() {
                self.update_copy(sec, w, s, copy, dt);
            }
        }
        self.map = map;
    }

    fn update_copy(&mut self, sec: &Section, w: &dyn World, s: usize, copy: usize, dt: f32) {
        // Data procs (0x477C30): curve(type = id bits 24–28) of the input
        // clamped to 0..0x7FFF.
        for (i, c) in sec.controls.iter().enumerate() {
            let v = self.dp_input(w, s, copy, c.id).clamp(0, 0x7FFF);
            self.copies[s][copy].dp[i] = curve((c.id >> 24) & 0x1F, v, false);
        }
        // Controls (0x477BD0).
        for (i, c) in sec.controls.iter().enumerate() {
            let dp = self.copies[s][copy].dp[i];
            let mut v = lin_to_db(0x7FFF - (((0x7FFF - dp) * c.k) >> 15)) + c.boost;
            let mut iv = self.input(w, s, copy, c.input);
            if c.input & 0xE000_0000 == 0x8000_0000 {
                iv = db_to_lin(iv);
            }
            v = (v * iv) >> 15;
            self.copies[s][copy].ctl[i] = v;
        }
        // Event controls (0x468D40).
        for (i, e) in sec.evts.iter().enumerate() {
            let trig = self.input(w, s, copy, e.w[3]);
            let scale = self.input(w, s, copy, e.w[2]);
            let st = &mut self.copies[s][copy].evt[i];
            evt_step(st, &e.w, trig, scale, dt);
        }
        // 3D controls (0x4684B0 variant by view, 0x468580).
        let view = w.view();
        for (i, t) in sec.threed.iter().enumerate() {
            let pos = 0x6000_0000 | (t.id & 0x00FF_FFF0);
            let f = |k: u32| w.field(s, copy, pos | k);
            let st = &mut self.copies[s][copy].td[i];
            if let Some(v) = t.variants.iter().position(|v| (v[0] >> 24) & 0xF == view) {
                st.variant = v;
            }
            let Some(v) = t.variants.get(st.variant) else {
                continue;
            };
            // A missing position object leaves the control as it was.
            let (Some(f0), Some(f1), Some(f2), Some(f3)) = (f(0), f(1), f(2), f(3)) else {
                continue;
            };
            threed_step(st, v, [f0, f1, f2, f3]);
        }
        // Sub channels (0x467E70 inline): base + inputs, clamped.
        let paused = w.paused();
        for (i, c) in sec.subs.iter().enumerate() {
            let v = if paused {
                -10000
            } else {
                (c.base + c.inputs.iter().map(|&x| self.chan_input(s, copy, x)).sum::<i32>()).min(c.max).max(c.min)
            };
            self.copies[s][copy].sub[i] = v;
        }
        // Master channels (0x467460, 0x477AA0).
        for (i, c) in sec.masters.iter().enumerate() {
            let v = if paused {
                -10000
            } else {
                (c.base + c.inputs.iter().map(|&x| self.chan_input(s, copy, x)).sum::<i32>()).min(c.max).max(c.min)
            };
            let cp = &mut self.copies[s][copy];
            cp.master[i] = v;
            let k = (c.out >> 4) & 0x7F;
            let mut block = cp.out.get(&k).copied().unwrap_or([0; 16]);
            for &m in &c.map {
                let mut mb = v + (m as u16 as i16) as i32;
                let mut az = 0u32;
                if !c.threed.is_empty() {
                    let j = ((m >> 20) & 0xF) as usize;
                    if let Some(t) = c.threed.get(j).and_then(|x| cp.td.get((x & 0xFF) as usize)) {
                        mb += t.mb;
                        az = t.az as u32 & 0xFFFF;
                    }
                }
                let level = if mb < -10000 {
                    0
                } else if mb > 0 {
                    0x7FFF
                } else {
                    db_to_lin(mb)
                };
                block[((m >> 24) & 0xF) as usize] = level as u32 | (az << 16);
            }
            cp.out.insert(k, block);
        }
    }
}
