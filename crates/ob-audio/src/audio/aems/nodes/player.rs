use super::super::*;

impl Instance {
    /// 0x720C5A: no voice.
    pub(in crate::audio::aems) fn reset_player(&mut self, b: i32) {
        self.wb(b + 0x10, 0xFF);
        if self.rb(b + 0xF) != 0 {
            let at = b + 0x1C + 12 * self.rb(b + 0xE) as i32;
            self.wr(at, 0);
            self.wr(at + 4, 0);
        }
    }

    /// Node 27 (0x720D57): a sound player. +0x18 the state (0 stop, 1
    /// play, 2 release), +0x14 the sound list index, +4 the list (file
    /// pointer: count, then 12-byte entries {kind, …, +4 bank sample}),
    /// +8 the voice, +0xC / +0xD the state and the one before, +0x10 the
    /// playing entry's kind (0xFF none), byte +0xE parameters of 12
    /// bytes from +0x1C {kind, +4 last sent, +8 value}. Returns 1 while
    /// its voice plays.
    pub(super) fn player(&mut self, b: i32, host: &mut dyn VoiceHost) -> i32 {
        let s = self.rd(b + 0x18).clamp(0, 2);
        let cur = self.rb(b + 0xC) as i8 as i32;
        let params = self.rb(b + 0xE) as i32;
        if s != cur {
            let kind = self.rb(b + 0x10) as i8;
            match s {
                0 => {
                    if kind >= 0 {
                        host.stop(self.rd(b + 8) as u32);
                        self.reset_player(b);
                    }
                }
                1 if kind >= 0 => {
                    // 0x720BE7: the pitch and the volume sent again.
                    let v = self.rd(b + 8) as u32;
                    if host.alive(v) {
                        let (mut pitch, mut vol) = (0x1000, 0x7FFF);
                        for k in 0..params {
                            let e = b + 0x1C + 12 * k;
                            match self.rb(e) {
                                0 => pitch = self.rd(e + 8),
                                2 => vol = self.rd(e + 8),
                                _ => continue,
                            }
                            self.wr(e + 4, self.rd(e + 8));
                        }
                        host.set(v, 0, pitch.clamp(0, 0xFFFF));
                        host.set(v, 2, vol.clamp(0, 0x7FFF));
                    }
                }
                1 => {
                    if !(cur == 2 && self.rb(b + 0xD) == 1) {
                        let p = self.program.clone();
                        let list = self.rd(b + 4);
                        let count = p.file_u32(list);
                        let i = self.rd(b + 0x14);
                        let i = if i >= count { count - 1 } else { i.max(0) };
                        let entry = list + 4 + 12 * i;
                        let ek = p.file.get(entry as usize).copied().unwrap_or(0xFF);
                        self.wb(b + 0x10, ek);
                        // 0x720B5B (kind 0, the only start in 0x817F90).
                        let mut block = StartBlock::default();
                        for k in 0..params {
                            let e = b + 0x1C + 12 * k;
                            block.set(self.rb(e), self.rd(e + 8));
                            self.wr(e + 4, self.rd(e + 8));
                        }
                        match (ek == 0).then(|| host.start(p.file_u32(entry + 4) as u32, &block)).flatten() {
                            Some(v) => self.wr(b + 8, v as i32),
                            None => {
                                self.wr(b + 8, -1);
                                self.reset_player(b);
                            }
                        }
                    }
                }
                _ => {
                    if kind >= 0 {
                        host.release(self.rd(b + 8) as u32);
                    }
                }
            }
            self.wb(b + 0xD, cur as u8);
            self.wb(b + 0xC, s as u8);
        }
        let mut r = s;
        if s == 1 && (self.rb(b + 0x10) as i8) >= 0 {
            // 0x720C7C.
            let v = self.rd(b + 8) as u32;
            if !host.alive(v) {
                self.reset_player(b);
                r = 0;
            } else {
                for k in 0..params {
                    let e = b + 0x1C + 12 * k;
                    let val = self.rd(e + 8);
                    if val != self.rd(e + 4) {
                        host.set(v, self.rb(e), val);
                        self.wr(e + 4, val);
                    }
                }
                if self.rb(b + 0xF) != 0 {
                    let (elapsed, left) = host.position(v);
                    let at = b + 0x1C + 12 * params;
                    self.wr(at + 4, elapsed);
                    self.wr(at, left);
                }
                r = 1;
            }
        }
        if (self.rb(b + 0x10) as i8) < 0 {
            r = 0;
        }
        r
    }
}
