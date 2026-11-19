//! The visual rating against the original (docs/notes/graphics/visual-rating.md).
//! Ignored without NFSU2_ROOT.
mod common;

use std::collections::BTreeMap;

use nfsu2::formats::parts::PartDb;

struct Data {
    db: PartDb,
    data: nfsu2::frontend::career::CareerData,
    types: Vec<nfsu2::car::CarType>,
    root: std::path::PathBuf,
}

fn load() -> Data {
    let root = common::game_root();
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    Data { db: PartDb::load(&globalb).unwrap(), types: nfsu2::car::parse_car_types(&globalb).unwrap(), data: nfsu2::frontend::career::CareerData::load(&root).unwrap(), root }
}

impl Data {
    fn paint(&self, car: &str) -> u32 {
        self.types.iter().find(|t| t.name == car).map_or(0, |t| t.stock_paint)
    }
}

fn parse_ride(words: &[&str]) -> BTreeMap<usize, Option<usize>> {
    words.iter().map(|kv| {
        let (s, i) = kv.split_once(':').unwrap();
        (s.parse().unwrap(), i.parse().ok())
    }).collect()
}

/// Every ride the original rated with 0x5234D0 under Wine (the saves'
/// career rides, each slot's parts swapped one by one, emptied slots, a
/// widebody with each bumper and skirt, every slot's last part): the
/// clone's `PartDb::visual_rating` on the same parts, bit for bit.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn native_rides_rate_as_the_original() {
    let d = load();
    let text = common::private_str("tests/fixtures/visual_rating/rides.txt");
    let mut bases: BTreeMap<String, (String, BTreeMap<usize, Option<usize>>)> = BTreeMap::new();
    let (mut n, mut bad) = (0, Vec::new());
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        let (car, label, bits, ride) = match w.first() {
            Some(&"ride") => {
                let ride = parse_ride(&w[4..]);
                bases.insert(w[1].to_string(), (w[2].to_string(), ride.clone()));
                (w[2].to_string(), "career", w[3], ride)
            }
            Some(&"diff") => {
                let (car, base) = bases[w[1]].clone();
                let mut ride = base;
                ride.extend(parse_ride(&w[4..]));
                (car, w[2], w[3], ride)
            }
            _ => continue,
        };
        let native = f32::from_bits(u32::from_str_radix(bits, 16).unwrap());
        let fitted = |slot: usize| ride.get(&slot).copied().flatten().and_then(|i| d.db.parts.get(i)).copied();
        let clone = d.db.visual_rating(&d.data.costs, d.paint(&car), &fitted);
        n += 1;
        if clone.to_bits() != native.to_bits() {
            bad.push(format!("{car} {label}: native {native} clone {clone}"));
        }
    }
    assert!(n > 100, "{n} rides");
    assert!(bad.is_empty(), "{} of {n} differ: {:?}", bad.len(), &bad[..bad.len().min(10)]);
}

/// The career car of native saves rated through the clone's own path
/// (the save → `Profile::from_native` → the ride with its derived slots →
/// `car::ride_visual_rating`) against the original's 0x528420 read under
/// Wine (the maintainer's tool `rating.py`, the maintainer's private capture).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn native_saves_rate_as_the_original() {
    let d = load();
    let ctx = nfsu2::frontend::career::NativeContext::new(&d.root, &d.data);
    for (save, car, bits) in SAVES {
        let s = nfsu2::formats::native_save::NativeSave::parse(&common::private(&format!("tests/fixtures/native_saves/{save}"))).unwrap();
        let p = nfsu2::frontend::career::Profile::from_native(&s, &ctx);
        assert_eq!(p.car, *car, "{save}");
        let r = nfsu2::car::ride_visual_rating(&d.db, &d.data.costs, &p.car, d.paint(&p.car), &p.installed);
        assert_eq!(r.to_bits(), *bits, "{save}: clone {r}, native {}", f32::from_bits(*bits));
    }
}

/// (fixture save, career car, the original's 0x528420 bits).
/// Read under Wine on 2026-11-05 (the maintainer's tool `rating.py`; 0x528420 and
/// 0x5234D0 on its ride gave the same bits for every save).
const SAVES: &[(&str, &str, u32)] = &[
    ("garage.sav", "240SX", 0x0000_0000),
    ("stage4_start.sav", "PEUGOT", 0x0000_0000),
    ("career_complete.sav", "LANCEREVO8", 0x402F_5C29),
    ("community_body_bumper_orion.sav", "ESCALADE", 0x4101_C28F),
    ("community_body_escalade.sav", "ESCALADE", 0x4107_D70A),
    ("community_body_hood.sav", "LANCEREVO8", 0x410A_3D71),
    ("community_body_rx7_spoiler.sav", "RX7", 0x4105_70A4),
    ("community_perf_after_buys.sav", "LANCEREVO8", 0x4110_A3D7),
];
