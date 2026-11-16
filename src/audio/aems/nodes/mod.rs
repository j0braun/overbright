mod curve;
mod delay;
mod envelope;
mod lfo;
mod player;
mod slew;

use super::*;

impl Instance {
    /// The node table 0x817FE0.
    pub(super) fn node(&mut self, n: u8, b: i32, host: &mut dyn VoiceHost, rng: &mut Rng) -> i32 {
        let tick = TICK_MS as f64;
        match n {
            // 0x71FEEF: the release flag, read once.
            0 => {
                let v = self.rd(b + 0x10);
                self.wr(b + 0x10, 0);
                v
            }
            // 0x71FEFB / 0x71FF03: the first / second parameter.
            1 => self.rd(b + 0x14),
            2 => self.rd(b + 0x18),
            // 0x71FF0B: a flag read once.
            3 => {
                let v = self.rd(b);
                self.wr(b, 0);
                v
            }
            // 0x71FF15: the end; with +0xC set the voices stop and the
            // instance is freed.
            4 => {
                if self.rd(b + 0xC) != 0 {
                    self.stop(host);
                    self.dead = true;
                }
                0
            }
            // 0x7201FE: the input when in [+0, +4], else a counter
            // stepping by the signed byte +0xC (while +0x10 > 0) and
            // wrapping in the range.
            6 => {
                let (x, lo, hi) = (self.rd(b + 0x14), self.rd(b), self.rd(b + 4));
                if x >= lo && x <= hi {
                    return x;
                }
                if self.rd(b + 0x10) > 0 {
                    let c = self.rd(b + 8).wrapping_add(self.rb(b + 0xC) as i8 as i32);
                    self.wr(
                        b + 8,
                        if c > hi {
                            lo
                        } else if c < lo {
                            hi
                        } else {
                            c
                        },
                    );
                }
                self.rd(b + 8)
            }
            // 0x72023C: on a trigger (+0xC), +8 = +0 + rand % +4.
            7 => {
                if self.rd(b + 0xC) != 0 {
                    let m = self.rd(b + 4) as u32;
                    let r = if m == 0 { 0 } else { rng.next() % m };
                    self.wr(b + 8, self.rd(b).wrapping_add(r as i32));
                }
                self.rd(b + 8)
            }
            // 0x72025F: a shuffled pick without repeats.
            8 => {
                let trig = b + self.rw(b) as i32;
                if self.rd(trig) == 0 {
                    return self.rd(b + 0xC);
                }
                let i = self.rw(b + 8) as i32;
                let count = self.rw(b + 0xA) as i32;
                let span = (count - self.rb(b + 3) as i8 as i32 - i) as u32;
                let j = if span == 0 { i } else { (rng.next() % span) as i32 + i };
                let bytes = self.rb(b + 2) == 1;
                let (pi, pj) = if bytes { (b + 0x10 + i, b + 0x10 + j) } else { (b + 0x10 + 2 * i, b + 0x10 + 2 * j) };
                let v = if bytes { self.rb(pj) as i32 } else { self.rw(pj) as i32 };
                if bytes {
                    let x = self.rb(pi);
                    self.wb(pj, x);
                    self.wb(pi, v as u8);
                } else {
                    let x = self.rw(pi);
                    self.ww(pj, x);
                    self.ww(pi, v as u16);
                }
                self.wr(b + 0xC, v.wrapping_add(self.rd(b + 4)));
                let i = i + 1;
                if i >= count {
                    self.ww(b + 8, 0);
                    self.wb(b + 3, 1);
                } else {
                    self.ww(b + 8, i as u16);
                    self.wb(b + 3, 0);
                }
                self.rd(b + 0xC)
            }
            // 0x720340: 1 once when the input enters [+0, +4]; re-armed
            // in [+8, +0xC].
            10 => {
                let x = self.rd(b + 0x14);
                if x >= self.rd(b) && x <= self.rd(b + 4) {
                    if self.rb(b + 0x10) == 0 {
                        self.wb(b + 0x10, 1);
                        self.wb(b + 0x11, 1);
                        return 1;
                    }
                } else if x >= self.rd(b + 8) && x <= self.rd(b + 0xC) {
                    self.wb(b + 0x10, 0);
                }
                self.wb(b + 0x11, 0);
                0
            }
            // 0x720376: a one-shot timer (+8 restarts it; fires 1 when the
            // float clock +0 reaches the ms at +0xC, then idles at −1).
            11 => {
                if self.rd(b + 8) != 0 {
                    self.wf(b, 0.0);
                }
                let t = self.rf(b);
                if t >= 0.0 {
                    if self.rd(b + 0xC) as f32 <= t {
                        self.wf(b, -1.0);
                        self.wb(b + 4, 1);
                        return 1;
                    }
                    self.wf(b, (tick + t as f64) as f32);
                }
                self.wb(b + 4, 0);
                0
            }
            // 0x7203BD: the value of the first set flag.
            12 => {
                let flags = b + self.rw(b) as i32;
                for k in 0..self.rb(b + 2) as i32 {
                    if self.rd(flags + 4 * k) != 0 {
                        self.wr(b + 4, self.rd(b + 8 + 4 * k));
                        break;
                    }
                }
                self.rd(b + 4)
            }
            // 0x7203EB: any input set.
            13 => (0..self.rb(b) as i32).any(|k| self.rd(b + 4 + 4 * k) != 0) as i32,
            14 => self.envelope(b),
            15 => self.curve(b),
            16 => self.delay(b),
            // 0x720684: input number +4 (1-based) of byte +0.
            17 => {
                let k = self.rd(b + 4);
                if k > 0 && k <= self.rb(b) as i32 { self.rd(b + 4 + 4 * k) } else { 0 }
            }
            // 0x72069E: route +8 to output +4 (1-based), clearing the last.
            18 => {
                let last = self.rw(b + 2) as i16 as i32;
                self.wr(b + 8 + 4 * last, 0);
                let k = self.rd(b + 4);
                if k > 0 && k <= self.rb(b) as i32 {
                    self.wr(b + 8 + 4 * k, self.rd(b + 8));
                    self.ww(b + 2, self.rw(b + 4));
                }
                self.rd(b + 0xC)
            }
            // 0x7206CC / 0x720702: min / max of byte +0 inputs from +4.
            19 | 20 => {
                let mut v = self.rd(b + 4);
                for k in 1..self.rb(b) as i32 {
                    let x = self.rd(b + 4 + 4 * k);
                    v = if n == 19 { v.min(x) } else { v.max(x) };
                }
                v
            }
            // 0x720738: the product of byte +0 inputs from +8, × float +4.
            21 => {
                let mut v = self.rd(b + 8) as f64;
                for k in 1..self.rb(b) as i32 {
                    v *= self.rd(b + 8 + 4 * k) as f64;
                }
                fist((v * self.rf(b + 4) as f64) as f32 as f64)
            }
            // 0x720784: the sum of byte +0 inputs from +4.
            22 => (1..self.rb(b) as i32).fold(self.rd(b + 4), |a, k| a.wrapping_add(self.rd(b + 4 + 4 * k))),
            // 0x7207AA −, 0x7207B4 ×, 0x7207BF ÷ and 0x7207D3 % (0 for a
            // zero divisor), 0x7207A0 +.
            23 => self.rd(b).wrapping_sub(self.rd(b + 4)),
            24 => self.rd(b + 4).wrapping_mul(self.rd(b)),
            25 => match self.rd(b + 4) {
                0 => 0,
                d => self.rd(b).wrapping_div(d),
            },
            26 => match self.rd(b + 4) {
                0 => 0,
                d => self.rd(b).wrapping_rem(d),
            },
            27 => self.player(b, host),
            28 => self.lfo(b),
            29 => self.slew(b),
            // 0x720FD1: min(sum of byte +0 inputs from +8, +4).
            30 => {
                let s = (1..self.rb(b) as i32).fold(self.rd(b + 8), |a, k| a.wrapping_add(self.rd(b + 8 + 4 * k)));
                s.min(self.rd(b + 4))
            }
            // 0x720FF8: max(+4 − +8, +0).
            31 => self.rd(b + 4).wrapping_sub(self.rd(b + 8)).max(self.rd(b)),
            // 0x72100B: min(+8 × +4, +0).
            32 => self.rd(b + 8).wrapping_mul(self.rd(b + 4)).min(self.rd(b)),
            // 0x7206F2 / 0x720728: min / max of +0 and +4.
            33 => self.rd(b).min(self.rd(b + 4)),
            34 => self.rd(b).max(self.rd(b + 4)),
            // 0x720767: +8 × +4 × float +0.
            35 => fist(((self.rd(b + 8) as f64 * self.rd(b + 4) as f64) * self.rf(b) as f64) as f32 as f64),
            36 => self.rd(b + 4).wrapping_add(self.rd(b)),
            // 0x72101F: byte +0x19 read once.
            37 => {
                let v = self.rb(b + 0x19) as i32;
                self.wb(b + 0x19, 0);
                v
            }
            // 0x7200A2: the sub-event output. Block: +0 / +4 the named
            // output it is bound to (the bank's event entry of type 1 names
            // the block, e.g. "CAR_SputOutput", declared by
            // EVT_SYS/UG2_ENGINES_AEMS2.csi; the game adds listeners to it,
            // 0x735900), +8 the open message, byte +0xC ranges given, byte
            // +0xD n, then n {min, max} when ranged, then E = {start, stop,
            // n parameters}. Stop set: the message released (0x735550).
            // Start set with none open: the parameters clamped and the
            // message opened (0x735490, every listener called). Neither:
            // an open message gets the clamped parameters (0x735080). It
            // returns the message's count (+4 = 1 while open) or 0.
            38 => {
                let ranged = self.rb(b + 0xC) != 0;
                let n = self.rb(b + 0xD) as i32;
                let e = if ranged { b + 0x10 + 8 * n } else { b + 0x10 };
                let clamp = |me: &mut Self| {
                    if ranged {
                        for i in 0..n {
                            let (lo, hi) = (me.rd(b + 0x10 + 8 * i), me.rd(b + 0x14 + 8 * i));
                            let v = me.rd(e + 8 + 4 * i);
                            me.wr(
                                e + 8 + 4 * i,
                                if v < lo {
                                    lo
                                } else if v > hi {
                                    hi
                                } else {
                                    v
                                },
                            );
                        }
                    }
                };
                if self.rd(e + 4) != 0 {
                    if self.rd(b + 8) != 0 {
                        self.send(b, Vec::new());
                    }
                    self.wr(b + 8, 0);
                } else if self.rd(e) != 0 {
                    if self.rd(b + 8) == 0 {
                        clamp(self);
                        self.wr(b + 8, 1);
                        let params = (0..n).map(|i| self.rd(e + 8 + 4 * i)).collect();
                        self.send(b, params);
                    }
                } else if self.rd(b + 8) != 0 {
                    clamp(self);
                    let params = (0..n).map(|i| self.rd(e + 8 + 4 * i)).collect();
                    self.send(b, params);
                }
                (self.rd(b + 8) != 0) as i32
            }
            // 0x7201C2: a named value (0x7356D0 notifies its listeners):
            // on a change of the input +0x14 (last +0x10) the value is the
            // input clamped to [+8, +0xC]; returns 0. No installed bank
            // uses it.
            39 => {
                let v = self.rd(b + 0x14);
                if self.rd(b + 0x10) != v {
                    self.wr(b + 0x10, v);
                    let (lo, hi) = (self.rd(b + 8), self.rd(b + 0xC));
                    let c = if v < lo {
                        lo
                    } else if v > hi {
                        hi
                    } else {
                        v
                    };
                    self.send(b, vec![c]);
                }
                0
            }
            // 5 and 9 are unused in the install.
            _ => 0,
        }
    }
}
