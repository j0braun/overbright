//! The game's sound banks (`.abk`, "ABKC"): event modules (compiled
//! AEMS graphs) followed by a "BNKl" bank of EA-XA samples. Parsed as
//! the audio library does (0x7212E5 for the modules, 0x727669 / 0x727A46
//! / 0x72AAF8 for the bank), decoded by 0x734AFF.
//!
//! Header: +0x00 "ABKC", +0x0A u16 module count, +0x14 file size, +0x18
//! the bank's offset, +0x1C the first module record, +0x30 / +0x34 / +0x38
//! the call relocations, the pointer relocations and the event table.

pub use crate::audio::codec::eaxa::decode_eaxa;
use crate::formats::bundle::{read_u16, read_u32};

/// One sample of a bank (a "PT" header, 0x727A46).
#[derive(Clone, Debug)]
pub struct SampleInfo {
    /// Tag 0x84 (default 22050).
    pub rate: u32,
    /// Tag 0x82 (default 1).
    pub channels: u8,
    /// Tag 0xA0 (default 10: EA-XA, 0x728DF9 class 2).
    pub codec: u8,
    /// Tag 0x85.
    pub samples: u32,
    /// Tag 0x88: the data's offset from the bank (flags tag 0x8C 0 or 4).
    pub data: usize,
    /// Tags 0x86 / 0x87 (not read by 0x727A46).
    pub loop_start: Option<i32>,
    pub loop_length: Option<i32>,
}

/// One event module: its compiled graph and the data after it.
#[derive(Clone, Debug)]
pub struct Module {
    /// Record offset (the event table names it as offset + 4).
    pub record: usize,
    pub code: (usize, usize),
    /// Instance size (+0x30): the data from the code's end is its template.
    pub size: usize,
    /// Template offsets (+0x3C on) the loader stores the bank's base at:
    /// the sound-player (node 27) states.
    pub players: Vec<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Bank {
    pub data: Vec<u8>,
    pub bank: usize,
    pub samples: Vec<Option<SampleInfo>>,
    pub modules: Vec<Module>,
    /// Event name → module index.
    pub events: Vec<(String, usize)>,
}

/// A big-endian value of `n` bytes, sign-extended for 1–3 (0x72A195).
fn be(b: &[u8]) -> i32 {
    let v = b.iter().fold(0u32, |a, &x| (a << 8) | x as u32);
    match b.len() {
        1 if v > 0x7F => v as i32 - 0x100,
        2 if v > 0x7FFF => v as i32 - 0x1_0000,
        3 if v > 0x7F_FFFF => v as i32 - 0x100_0000,
        _ => v as i32,
    }
}

/// The tagged values after "PT\0\0" (0x72AAF8): 0xFC padding is skipped,
/// 0xFF ends, 0xFD / 0xFE carry nothing, otherwise a size byte (0xFF: a
/// 4-byte size follows) and a big-endian value (read when under 5 bytes).
pub fn tags(d: &[u8], mut o: usize) -> Vec<(u8, Option<i32>)> {
    let mut out = Vec::new();
    while o < d.len() {
        while o < d.len() && d[o] == 0xFC {
            o += 1;
        }
        let Some(&t) = d.get(o) else { break };
        if t == 0xFF {
            break;
        }
        o += 1;
        if t == 0xFD || t == 0xFE {
            out.push((t, None));
            continue;
        }
        let Some(&n) = d.get(o) else { break };
        o += 1;
        let mut n = n as usize;
        if n == 0xFF {
            let Some(size) = d.get(o..o + 4) else { break };
            n = be(size) as usize;
            o += 4;
        }
        let v = (n < 5 && o + n <= d.len()).then(|| be(&d[o..o + n]));
        out.push((t, v));
        o += n;
    }
    out
}

impl Bank {
    pub fn parse(data: Vec<u8>) -> Option<Bank> {
        if data.len() < 0x40 || &data[..4] != b"ABKC" {
            return None;
        }
        let bank = read_u32(&data, 0x18) as usize;
        let mut b = Bank { bank, ..Default::default() };
        if data.get(bank..bank + 4) == Some(b"BNKl") {
            let count = data.get(bank + 6..bank + 8).map_or(0, |w| read_u16(w, 0) as usize);
            for i in 0..count {
                let e = bank + 0x14 + i * 4;
                // Fuzzing: an entry past the file ends the table.
                let Some(off) = data.get(e..e + 4).map(|w| read_u32(w, 0) as usize) else { break };
                if off == 0 || e + off + 4 > data.len() || &data[e + off..e + off + 2] != b"PT" {
                    b.samples.push(None);
                    continue;
                }
                let mut s = SampleInfo {
                    rate: 22050,
                    channels: 1,
                    codec: 10,
                    samples: 0,
                    data: 0,
                    loop_start: None,
                    loop_length: None,
                };
                for (t, v) in tags(&data, e + off + 4) {
                    let Some(v) = v else { continue };
                    match t {
                        0x84 => s.rate = v as u32,
                        0x82 => s.channels = v as u8,
                        0xA0 => s.codec = v as u8,
                        0x85 => s.samples = v as u32,
                        0x88 => s.data = v as usize,
                        0x86 => s.loop_start = Some(v),
                        0x87 => s.loop_length = Some(v),
                        _ => {}
                    }
                }
                b.samples.push(Some(s));
            }
        }
        // Module records (0x7212E5): 0x3C bytes then (+0x24 + +0x27) dwords.
        let mut rec = read_u32(&data, 0x1C) as usize;
        for _ in 0..read_u16(&data, 0x0A) {
            if rec + 0x3C > data.len() {
                break;
            }
            // Fuzz: a player offset past the file ends the record table.
            let Some(players) = (0..data[rec + 0x24] as usize).map(|i| data.get(rec + 0x3C + i * 4..rec + 0x40 + i * 4).map(|w| read_u32(w, 0) as usize)).collect::<Option<Vec<usize>>>() else { break };
            b.modules.push(Module {
                record: rec,
                code: (read_u32(&data, rec + 0x28) as usize, read_u32(&data, rec + 0x2C) as usize),
                size: read_u32(&data, rec + 0x30) as usize,
                players,
            });
            rec += 0x3C + (data[rec + 0x24] as usize + data[rec + 0x27] as usize) * 4;
        }
        // Events (+0x38): count, then {record + 4, name block, type}; the
        // block is {u32, name} (the name from +4: "FX_TURBO_01",
        // "PlayCommonSample").
        let ev = read_u32(&data, 0x38) as usize;
        if ev + 4 <= data.len() {
            for i in 0..read_u32(&data, ev) as usize {
                let e = ev + 4 + i * 12;
                if e + 12 > data.len() {
                    break;
                }
                let rec = read_u32(&data, e) as usize;
                let name_at = read_u32(&data, e + 4) as usize + 4;
                let name: String =
                    data.get(name_at..).unwrap_or(&[]).iter().take_while(|&&c| c != 0).map(|&c| c as char).collect();
                if let Some(m) = b.modules.iter().position(|m| m.record + 4 == rec) {
                    b.events.push((name, m));
                }
            }
        }
        b.data = data;
        Some(b)
    }

    pub fn module(&self, event: &str) -> Option<&Module> {
        self.events.iter().find(|e| e.0 == event).map(|e| &self.modules[e.1])
    }

    /// The sound list a module's first sound player picks from (node 27,
    /// 0x720D57: its state's +4, a bank-relative pointer the loader
    /// relocates): count, then 12-byte entries {u8 kind, …, u32 bank
    /// sample at +4, …}. The state sits at the template's player offset
    /// (the instance is the template copied from the code's end).
    pub fn sound_list(&self, m: &Module) -> Vec<(u8, u32)> {
        let Some(&player) = m.players.first() else { return Vec::new() };
        let at = m.code.1 + player + 4;
        let Some(list) = self.data.get(at..at + 4).map(|w| read_u32(w, 0) as usize) else { return Vec::new() };
        if list + 4 > self.data.len() {
            return Vec::new();
        }
        // (fuzz: no more entries than the file holds, not 4 G iterations)
        let n = (read_u32(&self.data, list) as usize).min((self.data.len() - list - 4) / 12);
        (0..n)
            .filter_map(|i| {
                let e = list + 4 + i * 12;
                (e + 12 <= self.data.len()).then(|| (self.data[e], read_u32(&self.data, e + 4)))
            })
            .collect()
    }

    /// A sample decoded to floats at 16-bit scale.
    pub fn decode(&self, index: usize) -> Option<(SampleInfo, Vec<f32>)> {
        let s = self.samples.get(index)?.clone()?;
        if s.codec != 10 || s.channels != 1 {
            return None;
        }
        let at = self.bank + s.data;
        Some((s.clone(), decode_eaxa(self.data.get(at..)?, s.samples as usize)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_values_are_signed_big_endian() {
        let d = [0x85, 2, 0x1D, 0xF4, 0x13, 1, 0x7F, 0x99, 1, 0xF0, 0xFD, 0xFF];
        let t = tags(&d, 0);
        assert_eq!(t[0], (0x85, Some(7668)));
        assert_eq!(t[1], (0x13, Some(127)));
        assert_eq!(t[2], (0x99, Some(-16)));
        assert_eq!(t[3], (0xFD, None));
    }
}
