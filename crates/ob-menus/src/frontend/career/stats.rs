use crate::frontend::career::{CareerData, Category, EventKind, Profile};

/// The stage goals 0x7F7BA8 (16 bytes per stage record: normal, sponsor
/// and URL wins the region's completion counts against, read by
/// 0x528A70 at +0, +4, +8).
pub const STAGE_GOALS: [(u32, u32, u32); 6] = [(0, 0, 0), (5, 0, 0), (10, 3, 3), (20, 3, 5), (30, 3, 7), (35, 3, 9)];

/// The page packages in tab order (0x7F4428).
pub const PAGES: [&str; 3] = ["UI_Status_Career", "UI_Status_Region", "UI_Status_DVD"];

/// The DVD cover slots of one stage record (0x835C9C + stage × 0x28: 4 ×
/// {f32 rating, u8 won}, then the count at +0x20): UI_Status_DVD
/// (0x497330) shows the counted covers and hides groups 0x02A9AFED + k
/// for k in count..4.
pub const DVD_SLOTS: usize = 4;

/// The statistics block 0x835C00 (0x528A70), the fields the pages read.
#[derive(Clone, Debug, Default)]
pub struct CareerStats {
    /// +0x00: the bank (career +0x7134).
    pub bank: u32,
    /// +0x04: races entered (career +0x872C).
    pub entered: u32,
    /// +0x08: the wins counted below; +0x0C wins / entered.
    pub wins: u32,
    /// +0x10…+0x24: Circuit, Sprint, Drag, Drift, Street X, URL wins.
    pub by_type: [u32; 6],
    /// +0x28 / +0x2C: magazines won / all (milestone kind 1 or 3).
    pub magazines: (u32, u32),
    /// +0x30: sponsorships signed (stage records' +0xE8 set).
    pub sponsors: u32,
    /// +0x34 + 4r: per region (shop +0x9D) the hidden shops (+0x51)
    /// entered and all of them (i16 pairs).
    pub hidden: [(u32, u32); 8],
    /// +0x54: parts unlocked (0x515470 / 0x5017F0).
    pub parts: f32,
    /// +0x68: game completion, events won / all events.
    pub completion: f32,
    /// +0x6C: the current stage (career +0x7AF8).
    pub stage: u8,
    /// +0x70…+0x84: the stage's normal, sponsor and URL wins and their
    /// goals (0x5004D0, the sponsor slots +0x8C, 0x500520; 0x7F7BA8).
    pub normal: (u32, u32),
    pub sponsor: (u32, u32),
    pub url: (u32, u32),
    /// +0x88 / +0x8C: DVD covers won / all; +0x90 / +0x94 the stage's.
    pub dvds: (u32, u32),
    pub stage_dvds: (u32, u32),
    /// +0x98: the region's completion: its wins and covers over its goals
    /// and covers.
    pub region: f32,
    /// +0x9C + 0x28s: the stage's DVD covers (asked rating, won), at
    /// most [`DVD_SLOTS`].
    pub dvd_list: Vec<(f32, bool)>,
    /// +0x1DC… / +0x1FC…: reputation and its average per race, per stage
    /// record; stage record 1 holds records 0 and 1 together.
    pub reputation: [f32; 8],
    pub average: [f32; 8],
    /// Stage record +0xFC (0x500C00), per record.
    pub unlocked: [bool; 8],
}

impl CareerStats {
    /// 0x528A70 on the clone's career.
    pub fn new(profile: &Profile, data: &CareerData) -> CareerStats {
        let mut s = CareerStats {
            bank: profile.cash,
            entered: profile.races_entered,
            stage: profile.stage,
            ..Default::default()
        };
        // The visit table (career +0x7AFC): won events (state byte +6 = 1)
        // of categories 1–3 count by type, URL (+0x34 = 2) by itself; type
        // 3 (Special) and the scripted events count in none.
        let mut won = 0;
        for e in data.events.iter().filter(|e| profile.completed.contains(&e.index)) {
            won += 1;
            let slot = match (e.category, e.kind) {
                (Category::Url, _) => Some(5),
                (Category::Sponsor | Category::Normal, EventKind::Circuit) => Some(0),
                (Category::Sponsor | Category::Normal, EventKind::Sprint) => Some(1),
                (Category::Sponsor | Category::Normal, EventKind::Drag) => Some(2),
                (Category::Sponsor | Category::Normal, EventKind::Drift) => Some(3),
                (Category::Sponsor | Category::Normal, EventKind::StreetX) => Some(4),
                _ => None,
            };
            if let Some(i) = slot {
                s.by_type[i] += 1;
                s.wins += 1;
            }
        }
        // Hidden shops (+0x51) per region (+0x9D); entered ones are in the
        // visit table with its flag (+4) set.
        for shop in data.shops.iter().filter(|x| x.announced) {
            let r = &mut s.hidden[(shop.stage as usize).min(7)];
            r.1 += 1;
            if profile.visited.contains(&shop.hash) {
                r.0 += 1;
            }
        }
        // 0x515470 / 0x5017F0: the cost table's levels with an unlock
        // (type 1–4) and those the unlock test (0x515360, mode 4) passes.
        let (mut open, mut all) = (0u32, 0u32);
        for (i, levels) in data.costs.categories.iter().enumerate() {
            for l in levels.iter().filter(|l| (1..=4).contains(&l.unlock)) {
                all += 1;
                if profile.part_unlocked(data, i as u8 + 1, l.level.max(0) as u8) {
                    open += 1;
                }
            }
        }
        s.parts = if all == 0 { 0.0 } else { open as f32 / all as f32 };
        s.completion = if data.events.is_empty() { 0.0 } else { won as f32 / data.events.len() as f32 };
        let goals = STAGE_GOALS.get(s.stage as usize).copied().unwrap_or_default();
        // 0x5004D0 counts the stage record's 35 normal slots (+0x00), so
        // never more than 35 (Wine, clone_s5_final with 44 normal wins:
        // [0x1C] = 35).
        s.normal = (profile.stage_wins(data, Category::Normal).len().min(35) as u32, goals.0);
        s.sponsor = (profile.stage_wins(data, Category::Sponsor).len().min(4) as u32, goals.1);
        s.url = (profile.stage_wins(data, Category::Url).len().min(10) as u32, goals.2);
        // The milestones: DVD covers (+0x24 2 or 4) listed per stage
        // (+0x25) with their rating (+0x3C) and state (0x862840: 1 won);
        // magazines (1 or 3) counted.
        for m in &data.milestones {
            let done = profile.milestones.contains(&m.hash);
            match m.kind {
                2 | 4 => {
                    s.dvds.1 += 1;
                    s.dvds.0 += u32::from(done);
                    if m.stage == s.stage {
                        s.stage_dvds.1 += 1;
                        s.stage_dvds.0 += u32::from(done);
                        // 0x528A70 stores each cover in the stage's 4 × 8-byte
                        // slots (+0x00..+0x1F, the count at +0x20): room
                        // for DVD_SLOTS covers per stage, never a 5th.
                        if s.dvd_list.len() < DVD_SLOTS {
                            s.dvd_list.push((m.rating, done));
                        }
                    }
                }
                1 | 3 => {
                    s.magazines.1 += 1;
                    s.magazines.0 += u32::from(done);
                }
                _ => {}
            }
        }
        for k in 0..7u8 {
            let (rep, n) = profile.stage_rep.get(&k).copied().unwrap_or((0.0, 0));
            s.reputation[k as usize] = rep;
            s.average[k as usize] = if n < 1 { 0.0 } else { rep / n as f32 };
            if profile.sponsors.contains_key(&k) {
                s.sponsors += 1;
            }
        }
        let (r0, n0) = profile.stage_rep.get(&0).copied().unwrap_or((0.0, 0));
        let (r1, n1) = profile.stage_rep.get(&1).copied().unwrap_or((0.0, 0));
        s.reputation[1] = r0 + r1;
        s.average[1] = if n0 + n1 == 0 { 0.0 } else { (r0 + r1) / (n0 + n1) as f32 };
        let need = s.normal.1 + s.stage_dvds.1 + s.url.1 + s.sponsor.1;
        s.region =
            if need == 0 { 0.0 } else { (s.normal.0 + s.stage_dvds.0 + s.url.0 + s.sponsor.0) as f32 / need as f32 };
        // Stage record k's +0xFC (0x500C00): set only by the stage end
        // 0x532C70 (run by the win handler 0x541430 on the won event's
        // stage record, +0x37), which then calls 0x528330(k), the only
        // writer of the stage (career +0x7AF8, 0x528342): stage + 1 when k
        // is the current stage. So record k is unlocked exactly when the
        // career has passed stage k.
        for k in 0..8 {
            s.unlocked[k] = (k as u8) < profile.stage;
        }
        s
    }
}

/// A Career Statistics list row (0x4CC250 / 0x4CC2E0 / 0x4CC410 /
/// 0x4CC380 / 0x4CC4A0 / 0x4B1710).
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// A heading (0x4CC4A0: the label, no value).
    Header(String),
    Value(String, String),
    /// A row with a slider (0x4B1680) at this fraction.
    Slider(String, f32),
}

/// UI_Status_Career's rows (0x4E13C0), indented rows prefixed by four
/// spaces (0x797670).
pub fn career_rows(s: &CareerStats, label: &dyn Fn(u32) -> String, spent: u32) -> Vec<Row> {
    let pad = "    ";
    let v = |h: u32, n: u32| Row::Value(format!("{pad}{}", label(h)), crate::frontend::grouped(i64::from(n)));
    let frac = |h: u32, (a, b): (u32, u32)| {
        Row::Value(
            format!("{pad}{}", label(h)),
            format!("{}/{}", crate::frontend::grouped(i64::from(a)), crate::frontend::grouped(i64::from(b))),
        )
    };
    // `%$.1f` (0x795D2C through 0x50C960, rows 0x4CC410 / 0x4CC380):
    // thousands grouped (Wine: "4,325.0", "15,300.0").
    let one = |x: f32| crate::frontend::grouped_fixed(f64::from(x), 1);
    let mut rows = vec![
        Row::Header(label(0x9226_0F04)),
        Row::Slider(format!("{pad}{}", label(0xABBA_6482)), s.completion),
        v(0x13B4_9408, s.bank),
        v(0x3B10_CB47, spent),
        v(0xC967_AEE0, s.entered),
        v(0x241B_E921, s.wins),
        Row::Value(
            format!("{pad}{}", label(0x0D81_7A4A)),
            one(if s.entered == 0 { 0.0 } else { s.wins as f32 / s.entered as f32 } * 100.0),
        ),
        Row::Header(label(0xC375_5F6D)),
        v(0x151C_443F, s.by_type[0]),
        v(0x22A1_44EC, s.by_type[1]),
        v(0x8EAB_27C5, s.by_type[3]),
        v(0x8090_B36A, s.by_type[2]),
        v(0xEF31_7ADA, s.by_type[4]),
        v(0xE6E0_BC3F, s.by_type[5]),
        Row::Header(label(0x11F7_AC44)),
        frac(0xC35D_D09E, s.magazines),
        frac(0x6781_44AE, s.dvds),
        v(0x5F77_BB60, s.sponsors),
    ];
    // Per stage: Stage 1 always, the next while the one before is
    // unlocked (0x500C00), up to 5.
    let stage_label = |k: usize| format!("{pad}{}", label(0x8B84_E940).replace("%d", &k.to_string()));
    for (heading, values) in [(0xA490_03BA, &s.reputation), (0xA63A_4AF1, &s.average)] {
        rows.push(Row::Header(label(heading)));
        for k in 1..=5 {
            rows.push(Row::Value(stage_label(k), one(values[k])));
            if !s.unlocked[k] {
                break;
            }
        }
    }
    if s.unlocked[0] {
        rows.push(Row::Header(label(0x2E8D_7E4B)));
        for k in 1..=5 {
            let name = label(fe_hash_label(&format!("CSTATISTICS_REGION_NAME_{k}")));
            rows.push(Row::Value(format!("{pad}{name}"), format!("{}/{}", s.hidden[k].0, s.hidden[k].1)));
            if !s.unlocked[k] {
                break;
            }
        }
    }
    rows.push(Row::Header(label(0xEA33_E5B6)));
    rows.push(Row::Slider(format!("{pad}{}", label(0xE0AF_0D6A)), s.parts));
    rows
}

/// A language label's hash (`bin_hash`, as 0x50B990 hashes the name).
fn fe_hash_label(name: &str) -> u32 {
    crate::formats::hash::bin_hash(name)
}
