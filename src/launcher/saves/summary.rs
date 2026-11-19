//! What the Saves page shows of a profile, read from its file without
//! loading it: the header and a few career fields of the payload
//! (`formats::native_save`, docs/formats/saves-options.md).
//!
//! * name: the folder's (MC_List shows it, 0x4D9AB0's layout); the payload's
//!   own name (profile +0x00, `NativeSave::name`, the help bar's) only when
//!   it differs.
//! * stage: career +0x7AF8 (`native_save::STAGE`): the index of the
//!   current stage record. 0x528330 adds one when the current stage's
//!   record is completed (its caller 0x532C70 sets the record's +0xFC
//!   latch first), so 0 is the first record, the prologue, and 6 follows
//!   the fifth stage: "Career complete". The fixtures read 0 (career start,
//!   car lot), 1–5 in the stage saves and 6 in the finished careers.
//! * money: career +0x7134 (`CASH`); races entered: career +0x872C.
//! * reputation: the eight stage records' +0xE0 (f32), summed as the
//!   clone's profile does (`Profile::from_native`).
//! * current car: career +0x8740 (`CURRENT_SLOT`) names a slot by its
//!   +4 id; that slot's record's car type hash (`REC_CAR`, the bin hash of
//!   the type's name) is looked up in the game folder's car types
//!   (`car::parse_car_types`). The internal type name is shown (the game's
//!   localized label needs a table: open).
//! * last played: the profile file's modification time (the payload has no
//!   date; imports and restores keep the source's time).
//! * problem: the file fails the checks the original's reader makes on the
//!   header (size, magic, header checksum, block type; 0x6FD3F0…0x6FD6E8),
//!   so the game can't load it. Section checksums are not checked by the
//!   loader (0x6FD73C), so a file with only those wrong is not flagged.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::formats::native_save::{self as ns, NativeSave};

/// One saved magazine cover or DVD picture of a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cover {
    /// The folder's name (`NAME DVD 3`, `NAME Magazine 12`).
    pub name: String,
    pub dvd: bool,
    pub n: u32,
    pub bytes: u64,
}

/// The car a profile drives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Car {
    /// No car yet (before the car lot).
    None,
    /// The car type's hash (`REC_CAR`).
    Hash(u32),
}

/// A profile as the Saves page lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    /// The folder's name.
    pub name: String,
    /// The payload's own name, when it differs from the folder's.
    pub payload_name: Option<String>,
    /// The profile file.
    pub file: PathBuf,
    pub stage: u32,
    pub cash: u32,
    pub races: u32,
    pub reputation: u32,
    pub car: Car,
    pub modified: Option<SystemTime>,
    pub bytes: u64,
    pub covers: Vec<Cover>,
    /// Why the game can't load it (None: it can).
    pub problem: Option<String>,
}

/// The stage's label (see the module's note).
pub fn stage_label(stage: u32) -> String {
    match stage {
        0 => "Prologue".into(),
        1..=5 => format!("Stage {stage}"),
        _ => "Career complete".into(),
    }
}

/// "$ 390,885".
pub fn money(cash: u32) -> String {
    format!("$ {}", grouped(u64::from(cash)))
}

/// 1,234,567.
pub fn grouped(v: u64) -> String {
    let s = v.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A saved cover's folder name split: (profile, DVD?, n). None: not a
/// cover's (`profiles::cover_name`).
pub fn cover_parts(name: &str) -> Option<(&str, bool, u32)> {
    let mut parts = name.rsplitn(3, ' ');
    let (n, kind, profile) = (parts.next()?, parts.next()?, parts.next()?);
    let dvd = match kind {
        "DVD" => true,
        "Magazine" => false,
        _ => return None,
    };
    if n.is_empty() || !n.chars().all(|c| c.is_ascii_digit()) || profile.is_empty() {
        return None;
    }
    Some((profile, dvd, n.parse().ok()?))
}

/// The covers of `profile` among the folder names `dirs` (any case), by
/// kind and number.
pub fn covers_of<'a>(profile: &str, dirs: impl IntoIterator<Item = &'a String>) -> Vec<&'a String> {
    let mut v: Vec<&String> =
        dirs.into_iter().filter(|d| cover_parts(d).is_some_and(|(p, _, _)| p.eq_ignore_ascii_case(profile))).collect();
    v.sort_by_key(|d| cover_parts(d).map(|(_, dvd, n)| (!dvd, n)));
    v
}

/// The fields of a parsed save.
pub fn fields(s: &NativeSave) -> (u32, u32, u32, u32, Car) {
    let stage = s.u32(ns::STAGE);
    let cash = s.u32(ns::CASH);
    let races = s.u32(ns::RACES_ENTERED);
    let rep: f32 = (0..ns::STAGE_COUNT).map(|k| s.f32(s.stage(k) + ns::ST_REP).max(0.0)).sum();
    let current = s.u32(ns::CURRENT_SLOT);
    let car = (0..5)
        .map(|i| s.slot(i))
        .find(|&o| s.u32(o + ns::SLOT_ID) == current && s.u8(o + ns::SLOT_HAS_CAR) != 0)
        .map_or(Car::None, |o| Car::Hash(s.u32(o + ns::SLOT_RECORD + ns::REC_CAR)));
    (stage, cash, races, rep as u32, car)
}

/// The summary of the profile in folder `name` under `root` (its file is
/// found as the game finds it, `profiles::store::find_in`). `dirs`: the
/// root's folder names (for the covers).
pub fn read(root: &Path, name: &str, dirs: &[String]) -> Summary {
    let file = crate::frontend::profiles::find_in(root, name);
    let meta = std::fs::metadata(&file).ok();
    let mut s = Summary {
        name: name.to_string(),
        payload_name: None,
        file: file.clone(),
        stage: 0,
        cash: 0,
        races: 0,
        reputation: 0,
        car: Car::None,
        modified: meta.as_ref().and_then(|m| m.modified().ok()),
        bytes: meta.as_ref().map_or(0, std::fs::Metadata::len),
        covers: Vec::new(),
        problem: None,
    };
    match std::fs::read(&file).map_err(|e| e.to_string()).and_then(|b| NativeSave::parse(&b).map_err(|e| e.to_string()))
    {
        Ok(save) => {
            (s.stage, s.cash, s.races, s.reputation, s.car) = fields(&save);
            let own = save.name();
            if own != name {
                s.payload_name = Some(own);
            }
        }
        Err(e) => s.problem = Some(e),
    }
    s.covers = covers_of(name, dirs)
        .into_iter()
        .filter_map(|d| {
            let (_, dvd, n) = cover_parts(d)?;
            let f = crate::frontend::profiles::find_in(root, d);
            let bytes = std::fs::metadata(&f).ok().filter(|m| m.is_file())?.len();
            Some(Cover { name: d.clone(), dvd, n, bytes })
        })
        .collect();
    s
}

/// The folder names under `root`.
pub fn dirs_in(root: &Path) -> Vec<String> {
    std::fs::read_dir(root)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Every profile under `root`, the last played first (then by name).
/// Left-over `.part` folders and files of an interrupted write are not
/// profiles (`list_in` wants a file of the folder's own name).
pub fn list(root: &Path) -> Vec<Summary> {
    let dirs = dirs_in(root);
    let mut v: Vec<Summary> = crate::frontend::profiles::list_in(root).iter().map(|n| read(root, n, &dirs)).collect();
    v.sort_by(|a, b| {
        b.modified.cmp(&a.modified).then_with(|| a.name.to_ascii_uppercase().cmp(&b.name.to_ascii_uppercase()))
    });
    v
}

/// The car types of a game folder by hash: the internal type name and the
/// manufacturer (GLOBAL/GLOBALB.BUN, `car::parse_car_types`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CarNames(pub HashMap<u32, String>);

impl CarNames {
    pub fn load(game: &Path) -> Option<CarNames> {
        let globalb = crate::game_fs::read(game, "GLOBAL/GLOBALB.BUN").ok()?;
        let types = crate::car::parse_car_types(&globalb).ok()?;
        Some(CarNames(
            types
                .into_iter()
                .map(|t| {
                    let label = if t.manufacturer.is_empty() || t.name.starts_with(&t.manufacturer) {
                        t.name.clone()
                    } else {
                        format!("{} {}", t.manufacturer, t.name)
                    };
                    (crate::formats::hash::bin_hash(&t.name), label)
                })
                .collect(),
        ))
    }

    /// The car's line: "No car yet", the type's name, or "Car not known"
    /// (no game folder, or a type the game folder does not have; never a
    /// guess).
    pub fn label(names: Option<&CarNames>, car: &Car) -> String {
        match car {
            Car::None => "No car yet".into(),
            Car::Hash(h) => names.and_then(|n| n.0.get(h)).cloned().unwrap_or_else(|| "Car not known".into()),
        }
    }
}

/// A cover's picture made small for the page: `THUMB` × `THUMB` RGBA, each
/// pixel the mean of its 8 × 8 block of the 512 × 512 picture
/// (`native_save::read_picture`).
pub const THUMB: u32 = 64;

pub fn thumb(file: &[u8]) -> Option<Vec<u8>> {
    let rgba = ns::read_picture(file).ok()?;
    let (n, t) = (ns::PICTURE_SIZE as usize, THUMB as usize);
    let k = n / t;
    let mut out = vec![0u8; t * t * 4];
    for ty in 0..t {
        for tx in 0..t {
            let mut sum = [0u32; 4];
            for y in ty * k..(ty + 1) * k {
                for x in tx * k..(tx + 1) * k {
                    let p = &rgba[(y * n + x) * 4..(y * n + x) * 4 + 4];
                    for c in 0..4 {
                        sum[c] += u32::from(p[c]);
                    }
                }
            }
            for c in 0..4 {
                out[(ty * t + tx) * 4 + c] = (sum[c] / (k * k) as u32) as u8;
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(stage_label(0), "Prologue");
        assert_eq!(stage_label(3), "Stage 3");
        assert_eq!(stage_label(6), "Career complete");
        assert_eq!(money(390_885), "$ 390,885");
        assert_eq!(money(0), "$ 0");
        assert_eq!(grouped(1_000_006_300), "1,000,006,300");
        assert_eq!(cover_parts("A B DVD 3"), Some(("A B", true, 3)));
        assert_eq!(cover_parts("RACER9 Magazine 12"), Some(("RACER9", false, 12)));
        assert_eq!(cover_parts("RACER9"), None);
        assert_eq!(cover_parts("DVD 1"), None);
        let dirs: Vec<String> = ["KIMMY Magazine 2", "KIMMY DVD 1", "kimmy Magazine 10", "KIMMYS DVD 1", "KIMMY"]
            .map(String::from)
            .to_vec();
        assert_eq!(covers_of("KIMMY", &dirs), ["KIMMY DVD 1", "KIMMY Magazine 2", "kimmy Magazine 10"]);
    }

    /// A made-up profile: the fields read back; a damaged header flagged.
    #[test]
    fn a_profile_and_a_broken_one() {
        let root = std::env::temp_dir().join(format!("ob-summary-{}", crate::unique_tag()));
        let mut s = NativeSave { payload: vec![0; ns::PAYLOAD_LEN] };
        s.set_name("Kim");
        s.set_u32(ns::STAGE, 2);
        s.set_u32(ns::CASH, 48_200);
        s.set_u32(ns::RACES_ENTERED, 12);
        s.set_f32(s.stage(1) + ns::ST_REP, 1000.0);
        s.set_f32(s.stage(2) + ns::ST_REP, 500.5);
        let slot = s.slot(1);
        s.set_u32(slot + ns::SLOT_ID, 77);
        s.set_u8(slot + ns::SLOT_HAS_CAR, 1);
        s.set_u32(slot + ns::SLOT_RECORD + ns::REC_CAR, crate::formats::hash::bin_hash("350Z"));
        s.set_u32(ns::CURRENT_SLOT, 77);
        let put = |rel: &str, b: &[u8]| {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b).unwrap();
        };
        put("KIMMY/KIMMY", &s.to_bytes());
        put("KIMMY DVD 1/KIMMY DVD 1", &ns::picture(&[9, 9, 9, 255], 1, 1));
        let mut bad = s.to_bytes();
        bad[0x18] ^= 1;
        put("BAD/BAD", &bad);
        let v = list(&root);
        let kimmy = v.iter().find(|p| p.name == "KIMMY").unwrap();
        assert_eq!((kimmy.stage, kimmy.cash, kimmy.races, kimmy.reputation), (2, 48_200, 12, 1500));
        assert_eq!(kimmy.payload_name.as_deref(), Some("Kim"));
        assert_eq!(kimmy.covers.len(), 1);
        assert!(kimmy.problem.is_none());
        let names = CarNames([(crate::formats::hash::bin_hash("350Z"), "NISSAN 350Z".to_string())].into());
        assert_eq!(CarNames::label(Some(&names), &kimmy.car), "NISSAN 350Z");
        assert_eq!(CarNames::label(None, &kimmy.car), "Car not known");
        assert_eq!(CarNames::label(None, &Car::None), "No car yet");
        assert!(v.iter().find(|p| p.name == "BAD").unwrap().problem.is_some());
        assert_eq!(v.len(), 2, "the cover is not a profile");
        let t = thumb(&std::fs::read(root.join("KIMMY DVD 1/KIMMY DVD 1")).unwrap()).unwrap();
        assert_eq!(&t[..4], &[9, 9, 9, 255]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The maintainer's saves: stage, money and car as the career's own
    /// reader sees them.
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn fixtures_summarise() {
        for (f, stage, cash, car) in [
            ("career_start.sav", 0, 0, false),
            ("car_lot.sav", 0, 0, true),
            ("garage.sav", 1, 50_000, true),
            ("stage2_body.sav", 2, 2_400, true),
            ("stage5_start.sav", 5, 148_300, true),
            ("career_complete.sav", 6, 390_885, true),
        ] {
            let b = crate::testkit::private(&format!("tests/fixtures/native_saves/{f}"));
            let s = NativeSave::parse(&b).unwrap();
            let (st, c, _, _, k) = fields(&s);
            assert_eq!((st, c, k != Car::None), (stage, cash, car), "{f}");
        }
        assert_eq!(stage_label(0), "Prologue");
    }

    /// With the game folder: the fixtures' cars have names.
    #[test]
    #[cfg_attr(
        not(all(ob_game, ob_private)),
        ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)"
    )]
    fn fixture_cars_have_names() {
        let root = std::path::PathBuf::from(std::env::var_os("NFSU2_ROOT").expect("NFSU2_ROOT"));
        let names = CarNames::load(&root).expect("the car types");
        for f in ["car_lot.sav", "garage.sav", "career_complete.sav", "community_body_rx7.sav"] {
            let s = NativeSave::parse(&crate::testkit::private(&format!("tests/fixtures/native_saves/{f}"))).unwrap();
            let label = CarNames::label(Some(&names), &fields(&s).4);
            assert!(label != "Car not known" && label != "No car yet", "{f}: {label}");
        }
    }
}
