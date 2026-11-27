use crate::formats::reader::{FormatResult, Reader};
use crate::formats::viv::Viv;

/// Most entries (`.idx`) and events (`.evt`) one bank may declare: the
/// shipped banks have 14 / 10 (UG2_Speech) and 4 / 4 (nisaudio); the
/// library's own tables are a few hundred at most.
const MAX_ENTRIES: usize = 1024;
const MAX_EVENTS: usize = 1024;
/// Most alternatives, steps and constraints one `.evt` may hold in all
/// (shipped: 14 / 14 / 37); events may share them, so a hostile file could
/// otherwise make the reader walk the same records millions of times.
const MAX_PARTS: usize = 1 << 16;

fn spend(budget: &mut usize, n: usize) -> FormatResult<()> {
    *budget = budget.checked_sub(n).ok_or(crate::formats::reader::FormatError::LimitExceeded {
        what: "speech event parts",
        got: MAX_PARTS + n,
        max: MAX_PARTS,
    })?;
    Ok(())
}

/// Round up to a dword (the library's `+ 3 & ~3`).
fn align4(n: usize) -> usize {
    (n + 3) & !3
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u16,
    /// Header +2: paired with the id by 0x735C30 (outrun selectors 1..5).
    pub selector: u16,
    pub keys: usize,
    /// (offset in bytes from the entry's base, key bytes).
    pub records: Vec<(u32, Vec<u8>)>,
    /// The end of the last record, bytes.
    pub end: u32,
    /// The entry's base in the `.big`.
    pub base: u32,
    /// Header +6: the depth of the entry's recently played ring
    /// (0x735D30 / 0x735D70; 0 = none).
    pub history: u8,
    /// The ring as the file holds it after the key table (0x7359E0):
    /// its position byte, then `history` record indices (0xFF unused).
    pub ring: (u8, Vec<u8>),
    /// After the records (aligned): per key the union of the values the
    /// records carry (0x7371C0's quick test).
    pub unions: Vec<u32>,
    /// Header +4 bit 7: an enable bit per record follows the key table
    /// (0x737140); `None` without.
    pub enabled: Option<Vec<u8>>,
}

impl Entry {
    /// 0x738A20: record `r`'s offset and size.
    pub fn range(&self, r: usize) -> Option<(u32, u32)> {
        let (o, _) = self.records.get(r)?;
        let next = self.records.get(r + 1).map_or(self.end, |n| n.0);
        Some((self.base + o, next.saturating_sub(*o)))
    }

    /// 0x737140 (variant 0xFFFF): the record may play — always, unless
    /// the entry carries enable bits and record `r`'s is clear.
    pub fn enabled(&self, r: usize) -> bool {
        self.enabled.as_ref().is_none_or(|b| b.get(r / 8).is_some_and(|&x| x & (1 << (r % 8)) != 0))
    }
}

/// One constraint of a play step (8 bytes, 0x736F30): the record's key
/// j must be a bit of `mask`, and of request value `component` (1-based;
/// 0 and 0xFE: any; 0xFF: the alternative's state table, not ported).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Constraint {
    pub mask: u32,
    pub component: u8,
    /// Byte +6: the state slot of the 0xFE / 0xFF components (bit 7 set:
    /// slot `& 0x7F`).
    pub slot: u8,
}

/// A play step of an alternative (at alternative + 4 · offset byte).
#[derive(Clone, Debug)]
pub struct Step {
    /// The packed index entry descriptor: low word the entry id, byte +2
    /// the request value a mode-1 descriptor selects with, byte +3 the
    /// mode (0x737030: 0 the id alone, 1 (id, value as u16) → 0x735C30).
    pub entry: u16,
    pub entry_component: u8,
    pub entry_mode: u8,
    pub constraints: Vec<Constraint>,
}

/// An alternative of an event (0x737470 / 0x738340 / 0x7377B0).
#[derive(Clone, Debug)]
pub struct Alternative {
    /// Byte +0: the weight (b & 0x1F) · 4^(b >> 5) (table 0x819A94).
    pub weight: u8,
    /// Byte +1: tried when Rand(100) < it (0x738489).
    pub chance: u8,
    /// Byte +2 & 3: 0 always, 1 only on channel 2, 2 never on channel 2
    /// (0x7377B0).
    pub mode: u8,
    /// Byte +3: the alternative's state words (non-zero: the shuffle
    /// branch of 0x737B20, not ported).
    pub state: u8,
    pub steps: Vec<Step>,
}

impl Alternative {
    /// 0x737470: (b & 0x1F) · table 0x819A94[b >> 5] = 4^(b >> 5).
    pub fn weight(&self) -> u32 {
        (self.weight as u32 & 0x1F) << (2 * (self.weight as u32 >> 5))
    }
}

/// An event of a `.evt` (0x7363A0 finds it by id; 0x738630 builds its
/// request from its switch's tuple).
#[derive(Clone, Debug)]
pub struct Event {
    pub id: u16,
    /// +2 / +4: the request's lifetime and priority in the library's
    /// queue (0x736860; the queue is not modelled: a request is evaluated
    /// at once).
    pub lifetime: u16,
    pub priority: u16,
    /// +7: the event's keys for the game callback 0x8B514C (0x738C00).
    pub keys: u8,
    /// +8: low nibble the count limit on its channel (0x738340), high
    /// nibble the switch-mask count (0x7382A0).
    pub limits: u8,
    /// +9 (signed): the request passes when Rand(100) ≤ it (0x73698A).
    pub chance: i8,
    /// +0xA: bit 0 the channel-state query (0x736530), bit 3 the
    /// alternative id match (0x738200 / 0x736E50), bit 5 0x7367F0.
    pub flags: u8,
    /// +0xB: the request values copied from the switch's tuple
    /// (0x738630).
    pub values: u8,
    pub alternatives: Vec<Alternative>,
}

impl Event {
    /// Whether every feature this event uses is ported. The shipped banks
    /// use none of the others (`speech::tests::the_shipped_banks_use_only_
    /// the_ported_features`); an event that needs one plays nothing here.
    pub fn ported(&self) -> bool {
        self.keys == 0
            && self.limits >> 4 == 0
            && self.flags & 0x09 == 0
            && self.alternatives.iter().all(|a| {
                a.state == 0
                    && a.mode == 0
                    && a.steps.len() == 1
                    && a.steps.iter().all(|s| s.constraints.iter().all(|c| c.component != 0xFF && c.component != 0xFE))
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Bank {
    pub big: String,
    pub entries: Vec<Entry>,
    pub events: Vec<Event>,
    /// Switch name → id (`.csi`).
    pub switches: Vec<(String, u32)>,
    /// The `.evt`'s switch table (0x7386A0): switch i fires event i.
    pub event_switches: Vec<String>,
}

fn read_idx(d: &[u8]) -> FormatResult<Vec<Entry>> {
    let r = Reader::new(d);
    let n = r.u32_at(0x54)? as usize;
    let mut table = Reader::new(d);
    table.seek(0x58)?;
    let rows = table.records("speech index entries", n, 16, MAX_ENTRIES)?;
    let mut out = Vec::new();
    for row in rows {
        let row = Reader::new(row);
        let (size, off, base) = (row.u32_at(4)? as usize, row.u32_at(8)? as usize, row.u32_at(12)?);
        let Ok(e) = r.sub_at(off, size) else {
            continue;
        };
        if e.len() < 0xC {
            continue;
        }
        let k4 = e.u8_at(4)?;
        let k = (k4 & 0x7F) as usize;
        let count = e.u8_at(5)? as usize;
        let history = e.u8_at(6)?;
        let unit = (e.u8_at(7)? as u32 + 1) * 0x100;
        let end = e.u16_at(8)? as u32 * unit;
        let mut records = Vec::new();
        for j in 0..count {
            let p = 0xC + (k + 2) * j;
            let Ok(rec) = e.bytes_at(p, k + 2) else {
                break;
            };
            records.push((u16::from_be_bytes([rec[0], rec[1]]) as u32 * unit, rec[2..].to_vec()));
        }
        // 0x7371C0 / 0x7359E0: the key table after the records, aligned.
        let table = align4(0xC + count * (k + 2));
        let unions = (0..k).map(|i| e.u32_at(table + 4 * i).unwrap_or(0)).collect();
        let mut tail = table + 4 * k;
        let enabled = (k4 & 0x80 != 0).then(|| {
            let n = (count + 7) >> 3;
            let b = e.bytes_at(tail + 1, n).map(<[u8]>::to_vec).unwrap_or_default();
            tail += n + 1;
            b
        });
        let ring = (e.u8_at(tail).unwrap_or(0), e.bytes_at(tail + 1, history as usize).map(<[u8]>::to_vec).unwrap_or_default());
        out.push(Entry {
            id: e.u16_at(0)?,
            selector: e.u16_at(2)?,
            keys: k,
            records,
            end,
            base,
            history,
            ring,
            unions,
            enabled,
        });
    }
    Ok(out)
}

/// The index entries (`.idx`); a malformed table gives the entries read
/// so far (none for a short header).
pub fn parse_idx(d: &[u8]) -> Vec<Entry> {
    read_idx(d).unwrap_or_default()
}

fn read_step(at: usize, f: &Reader, budget: &mut usize) -> FormatResult<Step> {
    let count = f.u8_at(at + 4)? as i8;
    spend(budget, 1 + count.max(0) as usize)?;
    let constraints = (0..count.max(0) as usize)
        .map(|c| {
            let p = at + 8 + 8 * c;
            Ok(Constraint { mask: f.u32_at(p)?, component: f.u8_at(p + 4)?, slot: f.u8_at(p + 6)? })
        })
        .collect::<FormatResult<_>>()?;
    Ok(Step {
        entry: f.u16_at(at)?,
        entry_component: f.u8_at(at + 2)?,
        entry_mode: f.u8_at(at + 3)?,
        constraints,
    })
}

fn read_event(at: usize, f: &Reader, budget: &mut usize) -> FormatResult<Event> {
    let count = f.u8_at(at + 6)? as usize;
    spend(budget, count)?;
    let alternatives = (0..count)
        .map(|i| {
            // +0xC: per alternative a u16 offset in dwords from the event.
            let a = at + 4 * f.u16_at(at + 0xC + 2 * i)? as usize;
            let steps = f.u8_at(a + 2)?;
            let steps_at = (0..(steps >> 2) as usize)
                .map(|j| read_step(a + 4 * f.u8_at(a + 8 + j)? as usize, f, budget))
                .collect::<FormatResult<_>>()?;
            Ok(Alternative {
                weight: f.u8_at(a)?,
                chance: f.u8_at(a + 1)?,
                mode: steps & 3,
                state: f.u8_at(a + 3)?,
                steps: steps_at,
            })
        })
        .collect::<FormatResult<_>>()?;
    Ok(Event {
        id: f.u16_at(at)?,
        lifetime: f.u16_at(at + 2)?,
        priority: f.u16_at(at + 4)?,
        keys: f.u8_at(at + 7)?,
        limits: f.u8_at(at + 8)?,
        chance: f.u8_at(at + 9)? as i8,
        flags: f.u8_at(at + 0xA)?,
        values: f.u8_at(at + 0xB)?,
        alternatives,
    })
}

/// The events and their switches of a `.evt` (0x7386A0): +4 the switch
/// table's offset, +8 / +9 the bank's keys (0x736350), +0x10 the event
/// count n, +0x18 per event a u16 offset in dwords; the switch table holds
/// n records of 0x20 bytes ({name offset in the string pool after the
/// records, id, the tuple's storage, at +0x18 the listener 0x738630 and at
/// +0x1C event i's key}), so setting switch i requests event i.
fn read_evt(d: &[u8]) -> FormatResult<(Vec<Event>, Vec<String>)> {
    let f = Reader::new(d);
    let n = f.u16_at(0x10)? as usize;
    let mut offsets = Reader::new(d);
    offsets.seek(0x18)?;
    let offsets: Vec<usize> =
        offsets.records("speech events", n, 2, MAX_EVENTS)?.map(|b| 4 * u16::from_le_bytes([b[0], b[1]]) as usize).collect();
    let mut events = Vec::new();
    let mut budget = MAX_PARTS;
    for &at in &offsets {
        events.push(read_event(at, &f, &mut budget)?);
    }
    let trailer = f.u32_at(4)? as usize;
    let pool = trailer.saturating_add(n.saturating_mul(0x20));
    let mut names = Vec::new();
    for i in 0..n {
        let off = f.u32_at(trailer + 0x20 * i)? as usize;
        names.push(f.cstr_at(pool.saturating_add(off), 64)?);
    }
    Ok((events, names))
}

/// The events of a `.evt` (none for a malformed one).
pub fn parse_evt(d: &[u8]) -> Vec<Event> {
    read_evt(d).map(|e| e.0).unwrap_or_default()
}

/// The events and the switch names that fire them, index for index.
pub fn parse_evt_switches(d: &[u8]) -> (Vec<Event>, Vec<String>) {
    read_evt(d).unwrap_or_default()
}

pub fn parse_csi(d: &[u8]) -> Vec<(String, u32)> {
    let r = Reader::new(d);
    if r.bytes_at(0, 4).ok() != Some(b"MOIR") {
        return Vec::new();
    }
    let n = r.u16_at(0xA).unwrap_or(0) as usize;
    let bank = r.u32_at(0x10).unwrap_or(0) & 0xFFFF;
    (0..n.min(MAX_EVENTS))
        .filter_map(|i| {
            let at = 0x2C + 12 * i;
            let name_at = r.u32_at(at).ok()? as usize;
            let hi = r.u32_at(at + 4).ok()? & 0xFFFF;
            Some((r.cstr_at(name_at, 64).ok()?, (hi << 16) | bank))
        })
        .collect()
}

impl Bank {
    /// Load `speech/<name>.{evt,idx,csi}` from the archive.
    pub fn load(viv: &Viv, name: &str) -> Option<Bank> {
        let idx = viv.read(&format!("speech/{name}.idx"))?;
        let evt = viv.read(&format!("speech/{name}.evt"))?;
        let csi = viv.read(&format!("speech/{name}.csi"))?;
        let (events, event_switches) = parse_evt_switches(&evt);
        Some(Bank {
            big: format!("speech/{name}.big"),
            entries: parse_idx(&idx),
            events,
            switches: parse_csi(&csi),
            event_switches,
        })
    }

    /// Resolved entry key: low word id, high word selector (zero for an
    /// unqualified lookup). Keeping the selector here preserves the entry
    /// selected by 0x737030 through stream decoding at 0x737DD8/0x738A20.
    pub fn entry(&self, key: u32) -> Option<&Entry> {
        self.entry_index(key).map(|index| &self.entries[index])
    }

    /// Stable identity for the recent-line ring. Native candidate +4 is
    /// the resolved entry index, so different selectors must not collide.
    pub fn entry_index(&self, key: u32) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| u32::from(e.id) == key & 0xFFFF && (key >> 16 == 0 || u32::from(e.selector) == key >> 16))
    }

    /// The event switch `name` fires (0x7386A0: switch i → event i).
    pub fn switch_event(&self, name: &str) -> Option<&Event> {
        let i = self.event_switches.iter().position(|s| s == name)?;
        self.events.get(i)
    }

    /// 0x737030: a step's entry key for the request values.
    pub fn resolve_entry(&self, step: &Step, values: &[u32]) -> Option<u32> {
        let entry = u32::from(step.entry);
        match step.entry_mode {
            0 => self.entry(entry).map(|_| entry),
            1 => {
                let component = usize::from(step.entry_component).checked_sub(1)?;
                // 0x73709A reads a word, not a bit mask or log2.
                let selector = *values.get(component)? as u16;
                self.entries.iter().find(|e| u32::from(e.id) == entry && e.selector == selector)?;
                Some(entry | (u32::from(selector) << 16))
            }
            // Mode 2 addresses variants within one entry. Neither shipped
            // speech bank uses it; do not silently interpret it as mode 0.
            _ => None,
        }
    }

    /// 0x737230's record test for one step, without the recently played
    /// ring: 0x7371C0 (every key the step constrains by a request value
    /// must meet the entry's union of that key) and per record 0x736F30
    /// (constraint j against key j: the key a bit of the mask and, for a
    /// component 1.., of request value component − 1; 0 and 0xFE: any)
    /// and 0x737140 (the record enabled).
    pub fn fitting(&self, step: &Step, key: u32, values: &[u32]) -> Vec<usize> {
        let Some(entry) = self.entry(key) else {
            return Vec::new();
        };
        let value = |c: u8| values.get(c as usize - 1).copied().unwrap_or(0);
        let real = |c: u8| c != 0 && c != 0xFE && c != 0xFF;
        let quick = entry.unions.iter().enumerate().all(|(i, &u)| {
            step.constraints.get(i).is_none_or(|c| !real(c.component) || u & value(c.component) != 0)
        });
        if !quick {
            return Vec::new();
        }
        entry
            .records
            .iter()
            .enumerate()
            .filter(|(r, (_, keys))| {
                entry.enabled(*r)
                    && step.constraints.iter().enumerate().all(|(j, c)| {
                        let Some(&k) = keys.get(j) else { return false };
                        k < 32
                            && c.mask & (1 << k) != 0
                            && (!real(c.component) || value(c.component) & (1 << k) != 0)
                            && c.component != 0xFF
                    })
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// The records of event `event`'s first alternative that fit a
    /// request (no ring, no draws): the candidate set of 0x737230.
    pub fn select(&self, event: u16, values: &[u32]) -> Vec<usize> {
        let Some(ev) = self.events.iter().find(|e| e.id == event) else {
            return Vec::new();
        };
        self.event_candidates(ev, values).map_or_else(Vec::new, |c| c.1)
    }

    /// The first alternative's first step: its entry key and fitting
    /// records.
    pub fn event_candidates(&self, ev: &Event, values: &[u32]) -> Option<(u32, Vec<usize>)> {
        let step = ev.alternatives.first()?.steps.first()?;
        let key = self.resolve_entry(step, values)?;
        Some((key, self.fitting(step, key, values)))
    }

    /// The candidates of a switch tuple the game sets (`lines`): the event
    /// of the switch whose setter built it (0x7386A0 / 0x738630), its
    /// entry and the records that fit. `None` when the tuple is no
    /// setter's, or nothing fits.
    pub fn fit(&self, tuple: &[u32]) -> Option<(u32, Vec<usize>)> {
        let ev = self.switch_event(super::lines::switch_of(tuple)?)?;
        let (key, r) = self.event_candidates(ev, tuple)?;
        (!r.is_empty()).then_some((key, r))
    }

    /// Read and decode record `r` of entry `entry`.
    pub fn decode(&self, viv: &Viv, entry: u32, r: usize) -> Option<crate::audio::schl::Pcm> {
        let e = self.entry(entry)?;
        let (off, size) = e.range(r)?;
        let ve = viv.find(&self.big)?;
        let data = viv.read_range(ve, off as u64, size as usize)?;
        let len = crate::audio::schl::stream_len(&data)?;
        crate::audio::schl::decode(&data[..len])
    }
}
