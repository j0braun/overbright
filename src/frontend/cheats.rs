//! The PC cheat codes typed at the title screen (UG_LS_Splash).
//!
//! **Verified** (SPEED2.EXE, and under Wine: docs/formats.md "Cheat
//! codes"). The manager is the global object 0x865930 (constructor
//! 0x575780, run at start-up by 0x77DCA0): +0x04 input on, +0x05 a code
//! matched, +0x08 the matched entry's +0x2C, +0x0C the table (0x7FB960),
//! +0x10 its count (20), +0x14 a 32-byte ring of the typed characters,
//! +0x34 the count typed since the last match. UG_LS_Splash's constructor
//! (0x4A90F0, at 0x4A9251) turns the input on (0x579CA0) and its
//! destructor (0x4A92B0, 0x4A92E0) off (0x579CB0). WM_CHAR (0x5CCD7B,
//! message 0x102) reaches 0x55DBE0, which hands the character to the
//! front end and then, with the input on, to 0x57B8C0.
//!
//! A table entry is 0x34 bytes: +0x00 the code (char[0x20]), +0x20 its
//! kind, +0x24 its value, +0x28 a group (entries sharing a non-zero group
//! switch each other off, 0x576060; every entry's is 0), +0x2C a value for
//! the manager's +0x08 (0 in every entry; nothing reads +0x05 / +0x08), +0x30
//! active, +0x31 kept (0x575FE0 clears the active flag of entries without
//! it; every entry has it), +0x32 enabled.
//!
//! Nothing is saved: the active flags and the two bonus amounts live only
//! in memory, so the codes last until the game quits.

use std::sync::atomic::{AtomicU32, Ordering};

/// What an entry's kind (+0x20) and value (+0x24) do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Kind 1 (0x579D90 by part level): in main-menu Customize (0x529D70,
    /// the shop flag 1) visual parts of this level list.
    Visual(u8),
    /// Kind 2 (0x579DA0 by package level): in main-menu Customize
    /// (0x529E40, 0x52A120) performance parts of this level list.
    Performance(u8),
    /// Kind 5, 0x5760B0 on the match: value 2 the $200 bonus (0x865128),
    /// 4 the $1000 one (0x86512C); 3 (0x579D40 / 0x579D70) opens the RX-8
    /// and Skyline in Quick Race and the contest vinyls (category 0x44).
    Bonus(u8),
    /// Kind 6 (0x579D80 by car id): the sponsor car whose garage record
    /// (0x5163C0) has the id "SPONSOR_<preset>" lists under Quick Race's
    /// "SPONSOR CARS" filter. The preset ride's name (GLOBALB 0x30220).
    SponsorCar(&'static str),
    /// Kind 7 (0x579DB0 by part hash): the vinyl (category 0x3D, group
    /// Sponsor) of this name lists, in a career's shops (0x515500) and in
    /// Customize (0x529D70).
    Vinyl(&'static str),
}

/// A table entry: the code and its effect.
#[derive(Clone, Copy, Debug)]
pub struct Code {
    pub text: &'static str,
    pub effect: Effect,
}

/// The table at 0x7FB960 in its order: entries 0–3 are static data, the
/// rest are written by 0x575780 (the sponsor values as hashes of
/// "SPONSOR_<name>" and "AD_<name>", 0x43DB50).
pub const CODES: [Code; 20] = [
    Code { text: "needperformance1", effect: Effect::Performance(1) },
    Code { text: "needperformance2", effect: Effect::Performance(2) },
    // Both visual codes hold level 2 (entries 2 and 3 are `01 00 00 00 02
    // 00 00 00` in the file and in memory).
    Code { text: "gimmevisual1", effect: Effect::Visual(2) },
    Code { text: "gimmevisual2", effect: Effect::Visual(2) },
    Code { text: "gimmechingy", effect: Effect::SponsorCar("CHINGY") },
    Code { text: "wannacapone", effect: Effect::SponsorCar("CAPONE") },
    Code { text: "wantmyd3", effect: Effect::SponsorCar("D3") },
    Code { text: "shinestreetbright", effect: Effect::SponsorCar("SHINESTREET") },
    Code { text: "davidchoeart", effect: Effect::SponsorCar("DAVIDCHOE") },
    Code { text: "tunejapantuning", effect: Effect::SponsorCar("JAPANTUNING") },
    Code { text: "yodogg", effect: Effect::SponsorCar("SNOOP_DOGG") },
    Code { text: "opendoors", effect: Effect::SponsorCar("THE_DOORS") },
    Code { text: "needmybestbuy", effect: Effect::Vinyl("AD_BESTBUY") },
    Code { text: "goforoldspice", effect: Effect::Vinyl("AD_OLDSPICE") },
    Code { text: "gottaedge", effect: Effect::Vinyl("AD_EDGE") },
    Code { text: "gottahavebk", effect: Effect::Vinyl("AD_BURGERKING") },
    Code { text: "gotmycingular", effect: Effect::Vinyl("AD_CINGULAR") },
    Code { text: "regmybank", effect: Effect::Bonus(2) },
    Code { text: "regmebaby", effect: Effect::Bonus(3) },
    Code { text: "ordermebaby", effect: Effect::Bonus(4) },
];

/// The sponsor cars' presets in the garage's order (0x5163C0 walks the
/// names at 0x7F7C6C into records 73–80 of the 91 at garage +0x9BD8).
pub const SPONSOR_CARS: [&str; 8] = ["CHINGY", "CAPONE", "D3", "SHINESTREET", "DAVIDCHOE", "JAPANTUNING", "SNOOP_DOGG", "THE_DOORS"];

/// The region byte 0x7FA73C (6 in this SPEED2.EXE; see the target note:
// prov: inferred frontend.cheats.REGION
/// region values read from this build are **inferred** to be retail).
/// 0x575780 clears needmybestbuy's enabled byte (+0x32) when it is not 0,
/// so that code does nothing here (read as 0 in memory under Wine).
pub const REGION: u32 = 6;

/// Entry `i`'s enabled byte (+0x32): 1 for all but needmybestbuy outside
/// region 0.
pub fn enabled(i: usize) -> bool {
    !(REGION != 0 && matches!(CODES.get(i), Some(Code { effect: Effect::Vinyl("AD_BESTBUY"), .. })))
}

/// The active flags (+0x30), one bit per entry.
#[derive(Debug, Default)]
pub struct Active(AtomicU32);

/// The game's table's flags, read by the shop and Quick Race rules.
pub static ACTIVE: Active = Active(AtomicU32::new(0));

impl Active {
    /// Whether entry `i` is active.
    pub fn get(&self, i: usize) -> bool {
        self.0.load(Ordering::Relaxed) >> i & 1 != 0
    }

    /// Sets the flags (shots: `NFSU2_CHEATS`).
    pub fn set(&self, mask: u32) {
        self.0.store(mask, Ordering::Relaxed);
    }

    /// The query 0x576010(kind, value): the active flag of the first
    /// entry with that effect.
    fn query(&self, effect: Effect) -> bool {
        CODES.iter().position(|c| c.effect == effect).is_some_and(|i| self.get(i))
    }

    /// 0x579D90: a visual part level opened by a code.
    pub fn visual(&self, level: u8) -> bool {
        self.query(Effect::Visual(level))
    }

    /// 0x579DA0: a performance level opened by a code.
    pub fn performance(&self, level: u8) -> bool {
        self.query(Effect::Performance(level))
    }

    /// 0x579D70 (regmebaby): the contest vinyls (category 0x44) in
    /// 0x515360, and with 0x579D40 the car types 0x15 (RX8) and 0x1B
    /// (SKYLINE) in Quick Race.
    pub fn regmebaby(&self) -> bool {
        self.query(Effect::Bonus(3))
    }

    /// 0x579D40: a stock car record's type opened by regmebaby.
    pub fn car_type(&self, index: usize) -> bool {
        self.regmebaby() && matches!(index, 0x15 | 0x1B)
    }

    /// 0x579D80: a sponsor car (by its preset's name) opened by its code.
    pub fn sponsor_car(&self, preset: &str) -> bool {
        CODES.iter().position(|c| matches!(c.effect, Effect::SponsorCar(p) if p == preset)).is_some_and(|i| self.get(i))
    }

    /// 0x579DB0: a vinyl (by its part hash) opened by its code.
    pub fn vinyl(&self, hash: u32) -> bool {
        CODES.iter().position(|c| matches!(c.effect, Effect::Vinyl(n) if crate::formats::hash::bin_hash(n) == hash)).is_some_and(|i| self.get(i))
    }

    /// The bonus amounts (0x865128, 0x86512C), set when their codes match.
    pub fn bonus(&self, value: u8) -> u32 {
        match value {
            2 if self.query(Effect::Bonus(2)) => 200,
            4 if self.query(Effect::Bonus(4)) => 1000,
            _ => 0,
        }
    }

    /// A new career's cash (0x53291D: 0x5760A0, the two bonuses' sum).
    pub fn start_cash(&self) -> u32 {
        self.bonus(2) + self.bonus(4)
    }

    /// `creation_cash` on this table.
    pub fn creation_cash(&self, replacing: bool, boot_bank: u32) -> u32 {
        if replacing { self.start_cash() } else { boot_bank }
    }
}

/// The game table's 0x579D90.
pub fn visual(level: u8) -> bool {
    ACTIVE.visual(level)
}

/// The game table's 0x579DA0.
pub fn performance(level: u8) -> bool {
    ACTIVE.performance(level)
}

/// The game table's 0x579D70.
pub fn regmebaby() -> bool {
    ACTIVE.regmebaby()
}

/// The game table's 0x579D40.
pub fn car_type(index: usize) -> bool {
    ACTIVE.car_type(index)
}

/// The game table's 0x579D80.
pub fn sponsor_car(preset: &str) -> bool {
    ACTIVE.sponsor_car(preset)
}

/// The game table's 0x579DB0.
pub fn vinyl(hash: u32) -> bool {
    ACTIVE.vinyl(hash)
}

/// The game table's new-career cash.
pub fn start_cash() -> u32 {
    ACTIVE.start_cash()
}

/// 0x5760B0's payment of a bonus code's amount into the career in memory
/// (0x500F80 on 0x861E74): only when its intro movie has not played
/// (0x86347C = 0) and its bank (0x500F90) is 0; returns the new bank.
/// **Verified** under Wine: regmybank then ordermebaby on the title left
/// the bank at 200 (the second not paid).
pub fn pay_now(bonus: u32, intro_played: bool, bank: u32) -> u32 {
    if bonus != 0 && !intro_played && bank == 0 { bonus } else { bank }
}

/// A new profile's bank (MC_Main's create, 0x4F33A0 / 0x4F3780). With a
/// profile current the question "Creating a new profile will …" (state 1,
/// tag 0x5000000) answered Yes resets the career (0x532840, through
/// 0x534700 for the Profile Manager or directly with Start New Career's
/// 0x20000): the bank is the two bonuses' sum (`start_cash`). With none
/// current (the boot's "Would you like to create a new profile?", state
/// 2) nothing resets it and the profile takes the career in memory, the
/// boot's (`boot_bank`: 0 plus what a bonus code paid at once).
/// **Verified** under Wine with regmybank and ordermebaby typed on the
/// title: the boot's profile saved with 200, one made in the Profile
/// Manager after it with 1200.
pub fn creation_cash(replacing: bool, boot_bank: u32) -> u32 {
    ACTIVE.creation_cash(replacing, boot_bank)
}

/// The typed characters (manager +0x14 / +0x34).
#[derive(Clone, Debug, Default)]
pub struct Input {
    ring: [u8; 32],
    count: u32,
}

/// A code just matched (0x579CC0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Matched {
    /// The table entry.
    pub index: usize,
    /// Not active before: 0x51DE70(_, 8) plays the menu's SELECT sound.
    pub new: bool,
    /// Its bonus amount, kind 5 (0x5760B0), to add to the career now when
    /// its intro has not played and its cash is 0.
    pub bonus: u32,
}

impl Input {
    /// A typed character with the input on (0x57B8C0): into the ring, then
    /// each entry in table order whose code is no longer than the count
    /// typed is compared with the ring's last characters, a–z matched
    /// without case; a match runs 0x579CC0 (which zeroes the count).
    pub fn char(&mut self, table: &Active, c: char) -> Vec<Matched> {
        let mut out = Vec::new();
        let Ok(b) = u8::try_from(c as u32) else { return out };
        self.ring[(self.count & 31) as usize] = b;
        self.count += 1;
        let up = |b: u8| if b.is_ascii_lowercase() { b & 0x5F } else { b };
        for (index, code) in CODES.iter().enumerate() {
            let text = code.text.as_bytes();
            let n = text.len() as u32;
            if n > self.count {
                continue;
            }
            let start = self.count - n;
            if (0..n).all(|j| up(text[j as usize]) == up(self.ring[((start + j) & 31) as usize])) {
                if let Some(m) = self.activate(table, index) {
                    out.push(m);
                }
            }
        }
        out
    }

    /// 0x579CC0: an enabled entry turns active (with the SELECT sound when
    /// it was not), its kind-5 bonus is set (0x5760B0), and the count goes
    /// back to 0.
    fn activate(&mut self, table: &Active, index: usize) -> Option<Matched> {
        if !enabled(index) {
            return None;
        }
        let new = !table.get(index);
        table.0.fetch_or(1 << index, Ordering::Relaxed);
        let bonus = match CODES[index].effect {
            Effect::Bonus(v) => table.bonus(v),
            _ => 0,
        };
        self.count = 0;
        Some(Matched { index, new, bonus })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::hash::bin_hash;

    #[test]
    fn typing() {
        let t = Active::default();
        let mut input = Input::default();
        let mut typed = |text: &str| -> Vec<Matched> { text.chars().flat_map(|c| input.char(&t, c)).collect() };
        assert_eq!(typed("xxYoDoGg"), vec![Matched { index: 10, new: true, bonus: 0 }]);
        // Again: active already, so no sound.
        assert_eq!(typed("yodogg"), vec![Matched { index: 10, new: false, bonus: 0 }]);
        // needmybestbuy is disabled in region 6.
        assert!(typed("needmybestbuy").is_empty());
        typed("gottaedge");
        let bank: Vec<u32> = typed("regmybankordermebaby").iter().map(|m| m.bonus).collect();
        typed("gimmevisual1regmebaby");
        assert!(t.sponsor_car("SNOOP_DOGG") && !t.sponsor_car("CHINGY"));
        assert!(!t.vinyl(bin_hash("AD_BESTBUY")) && t.vinyl(bin_hash("AD_EDGE")));
        assert_eq!(bank, vec![200, 1000]);
        assert_eq!(t.start_cash(), 1200);
        assert!(t.visual(2) && !t.visual(1) && t.car_type(0x15) && t.car_type(0x1B) && !t.car_type(0x16));
    }

    /// The table in memory under Wine (docs/evidence/cheats/table.json):
    /// each entry's kind (+0x20) and value (+0x24); the sponsor cars' and
    /// vinyls' values are bin hashes of SPONSOR_<preset> and the vinyl.
    #[test]
    fn table_matches_memory() {
        let memory: [(&str, u32, u32); 20] = [
            ("needperformance1", 2, 0x1),
            ("needperformance2", 2, 0x2),
            ("gimmevisual1", 1, 0x2),
            ("gimmevisual2", 1, 0x2),
            ("gimmechingy", 6, 0x3ee7d094),
            ("wannacapone", 6, 0x3e6d00c8),
            ("wantmyd3", 6, 0x54e32d29),
            ("shinestreetbright", 6, 0xe57b6400),
            ("davidchoeart", 6, 0x7760c479),
            ("tunejapantuning", 6, 0x2f98f1),
            ("yodogg", 6, 0x7090a1),
            ("opendoors", 6, 0x4baece79),
            ("needmybestbuy", 7, 0x9e52eac1),
            ("goforoldspice", 7, 0x5fd7b956),
            ("gottaedge", 7, 0x44c47b98),
            ("gottahavebk", 7, 0xcf5f6873),
            ("gotmycingular", 7, 0x7c504338),
            ("regmybank", 5, 0x2),
            ("regmebaby", 5, 0x3),
            ("ordermebaby", 5, 0x4),
        ];
        for (i, (code, (text, kind, value))) in CODES.iter().zip(memory).enumerate() {
            assert_eq!(code.text, text);
            let got = match code.effect {
                Effect::Visual(l) => (1, l as u32),
                Effect::Performance(l) => (2, l as u32),
                Effect::Bonus(v) => (5, v as u32),
                Effect::SponsorCar(p) => (6, bin_hash(&format!("SPONSOR_{p}"))),
                Effect::Vinyl(n) => (7, bin_hash(n)),
            };
            assert_eq!(got, (kind, value), "{text}");
            // Only needmybestbuy's enabled byte (+0x32) was 0 in memory.
            assert_eq!(enabled(i), text != "needmybestbuy");
        }
    }

    /// The Wine run typed these one after another on the title
    /// (docs/evidence/cheats/typing.jsonl): after each, the active codes,
    /// the count since the last match (+0x34) and the career in memory's
    /// bank (0x861E74, from 0).
    #[test]
    fn typing_as_under_wine() {
        let t = Active::default();
        let mut input = Input::default();
        let mut bank = 0;
        let steps: [(&str, &[&str], u32, u32); 13] = [
            ("regmybank", &["regmybank"], 0, 200),
            ("ordermebaby", &["regmybank", "ordermebaby"], 0, 200),
            ("NeedPerformance1", &["needperformance1"], 0, 200),
            ("xxneedperformance2", &["needperformance2"], 0, 200),
            ("gimmevisual1", &["gimmevisual1"], 0, 200),
            ("gimmevisual2", &["gimmevisual2"], 0, 200),
            ("regmebaby", &["regmebaby"], 0, 200),
            ("goforoldspice", &["goforoldspice"], 0, 200),
            ("gottaedge", &["gottaedge"], 0, 200),
            ("gottahavebk", &["gottahavebk"], 0, 200),
            ("gotmycingular", &["gotmycingular"], 0, 200),
            // Disabled: nothing turns on and the count is not reset.
            ("needmybestbuy", &[], 13, 200),
            ("regmybank", &[], 0, 200),
        ];
        let mut want_active = std::collections::BTreeSet::new();
        for (typed, new_codes, count, want_bank) in steps {
            for c in typed.chars() {
                for m in input.char(&t, c) {
                    bank = pay_now(m.bonus, false, bank);
                }
            }
            want_active.extend(new_codes.iter().copied());
            let active: std::collections::BTreeSet<&str> = (0..20).filter(|&i| t.get(i)).map(|i| CODES[i].text).collect();
            assert_eq!(active, want_active, "after {typed}");
            assert_eq!((input.count, bank), (count, want_bank), "after {typed}");
        }
        assert_eq!((t.bonus(2), t.bonus(4), t.start_cash()), (200, 1000, 1200));
    }

    /// Each code on its own turns on its effect and no other.
    #[test]
    fn each_code_alone() {
        for (i, code) in CODES.iter().enumerate() {
            let t = Active::default();
            let mut input = Input::default();
            let matched: Vec<Matched> = code.text.chars().flat_map(|c| input.char(&t, c)).collect();
            if !enabled(i) {
                assert!(matched.is_empty() && !t.get(i), "{}", code.text);
                continue;
            }
            assert_eq!(matched, vec![Matched { index: i, new: true, bonus: t.bonus(match code.effect { Effect::Bonus(v) => v, _ => 0 }) }], "{}", code.text);
            let e = code.effect;
            // 0x576010 reads the first entry of a kind and value, so
            // gimmevisual2 (the same value as gimmevisual1) opens nothing
            // on its own.
            let first = CODES.iter().position(|c| c.effect == e) == Some(i);
            assert_eq!(first, code.text != "gimmevisual2");
            assert_eq!(t.visual(2), first && e == Effect::Visual(2), "{}", code.text);
            assert_eq!(t.performance(1), e == Effect::Performance(1), "{}", code.text);
            assert_eq!(t.performance(2), e == Effect::Performance(2), "{}", code.text);
            assert!(!t.visual(1), "{}", code.text);
            assert_eq!(t.regmebaby(), e == Effect::Bonus(3), "{}", code.text);
            assert_eq!(t.car_type(0x15) && t.car_type(0x1B), e == Effect::Bonus(3), "{}", code.text);
            let cash = match e {
                Effect::Bonus(2) => 200,
                Effect::Bonus(4) => 1000,
                _ => 0,
            };
            assert_eq!(t.start_cash(), cash, "{}", code.text);
            for p in SPONSOR_CARS {
                assert_eq!(t.sponsor_car(p), e == Effect::SponsorCar(p), "{} {p}", code.text);
            }
            for c in CODES.iter().filter_map(|c| if let Effect::Vinyl(n) = c.effect { Some(n) } else { None }) {
                assert_eq!(t.vinyl(bin_hash(c)), e == Effect::Vinyl(c), "{} {c}", code.text);
            }
        }
    }

    /// The ring keeps the last 32 characters (index count & 31): a code
    /// typed after any amount of other text matches, and one split by
    /// another character does not.
    #[test]
    fn ring_and_case() {
        let t = Active::default();
        let mut input = Input::default();
        let long: String = std::iter::repeat_n('q', 100).collect();
        let mut typed = |s: &str| -> Vec<usize> { s.chars().flat_map(|c| input.char(&t, c)).map(|m| m.index).collect() };
        assert!(typed(&long).is_empty());
        assert_eq!(typed("GOTTAEDGE"), vec![14]);
        assert!(typed("gotta edge").is_empty());
        assert_eq!(typed("gOtMyCiNgUlAr"), vec![16]);
    }

    /// The immediate payment and a new profile's bank.
    #[test]
    fn bonus_payment_and_new_profiles() {
        assert_eq!(pay_now(200, false, 0), 200);
        assert_eq!(pay_now(1000, false, 200), 200);
        assert_eq!(pay_now(1000, true, 0), 0);
        assert_eq!(pay_now(0, false, 0), 0);
        let t = Active::default();
        t.set(1 << 17 | 1 << 19);
        // Wine: the boot's profile (none current) 200, the Profile
        // Manager's afterwards 1200.
        assert_eq!(t.creation_cash(false, 200), 200);
        assert_eq!(t.creation_cash(true, 200), 1200);
        assert_eq!(Active::default().creation_cash(true, 0), 0);
    }
}
