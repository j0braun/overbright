//! The original's profile saves (tests/fixtures/native_saves, written by
//! SPEED2.EXE under Wine: career_start.sav after Start New Career and a
//! Profile Manager save, car_lot.sav saved by the car lot's own prompt
//! after taking a stock 240SX, garage.sav saved from the garage after the
//! D-Day garage goal, its bank set to 50000 in memory beforehand;
//! mycar.sav from a fresh profile after the main menu's Customize fitted a
//! front bumper to the Peugeot 206 and the exit's save prompt, options.sav
//! saved after that with the EA Trax play mode set to Sequential and track
//! 1 to Racing, player 1's transmission Manual, Career Difficulty Hard and
//! Steering (Left) remapped to J). Each holds only the game's numbers,
//! hashes and the profile name "NAME".

mod common;

use nfsu2::formats::hash::bin_hash;
use nfsu2::formats::native_save::{self, NativeSave};
use nfsu2::frontend::career::{self, CareerData, NativeContext, Profile};

/// The original's saves (the private set), read once per call and kept for
/// the test's run.
fn fixtures() -> [(&'static str, &'static [u8]); 5] {
    let f = |name: &str| -> &'static [u8] { Box::leak(common::private(&format!("tests/fixtures/native_saves/{name}.sav")).into_boxed_slice()) };
    [("career_start", f("career_start")), ("car_lot", f("car_lot")), ("garage", f("garage")), ("mycar", f("mycar")), ("options", f("options"))]
}

#[test]
#[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
fn originals_parse_and_rewrite_byte_for_byte() {
    for (name, bytes) in fixtures() {
        let s = NativeSave::parse(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(s.to_bytes(), bytes, "{name}");
        assert_eq!(s.name(), "NAME");
    }
}

fn context() -> (CareerData, NativeContext) {
    let root = common::game_root();
    let data = CareerData::load(&root).expect("loading the career data");
    let ctx = NativeContext::new(&root, &data);
    (data, ctx)
}

fn event(data: &CareerData, hash: u32) -> usize {
    data.events.iter().find(|e| e.unlock == hash).unwrap().index
}

#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn originals_load_into_the_clone() {
    let (data, ctx) = context();
    let load = |i: usize| Profile::from_native(&NativeSave::parse(fixtures()[i].1).unwrap(), &ctx);
    let start = load(0);
    assert_eq!((start.name.as_str(), start.cash, start.stage), ("NAME", 0, 0));
    assert!(start.car.is_empty() && start.completed.is_empty());
    assert_eq!(start.intro, Some(career::DDAY_EVENT_A));
    assert!(start.explore_set);
    assert_eq!(start.inbox.entries.len(), 1);

    let lot = load(1);
    assert_eq!(lot.car, "240SX");
    assert_eq!(lot.slot, 0);
    // The lot's car: stock parts, plus its paint (slot 0x3F, METAL_L3_TEST08)
    // and vinyl colours (0x4D…0x5C), slots with no stock part.
    assert!(lot.installed.keys().all(|&k| k == 0x3F || (0x4D..=0x5C).contains(&k)), "{:?}", lot.installed);
    assert_eq!(lot.installed.get(&0x3F), Some(&bin_hash("METAL_L3_TEST08")));
    assert!(lot.completed.contains(&event(&data, career::DDAY_EVENT_A)));
    assert!(!lot.completed.contains(&event(&data, career::DDAY_EVENT_B)));
    assert_eq!(lot.intro, Some(career::DDAY_EVENT_B));
    assert!(lot.visited.contains(&bin_hash("CC_CAR_LOT_1")));
    assert!(lot.intro_races);
    assert_eq!(lot.slots.iter().filter(|s| s.unlocked).count(), 1);

    let garage = load(2);
    assert_eq!((garage.cash, garage.stage, garage.car.as_str()), (50000, 1, "240SX"));
    assert!(garage.completed.contains(&event(&data, career::DDAY_EVENT_B)));
    assert_eq!(garage.intro, None);
    assert_eq!(garage.inbox.entries.len(), 5);
}

#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn clone_round_trip_keeps_the_originals_bytes() {
    let (_, ctx) = context();
    for (name, bytes) in fixtures() {
        let s = NativeSave::parse(bytes).unwrap();
        let p = Profile::from_native(&s, &ctx);
        let back = p.to_native(&s, &ctx);
        // What the original's own load and save gives: the input with the
        // four hint dwords the loader 0x529240 zeroes (see
        // `clone_rewrite_matches_the_originals_resave`).
        let mut want = s.clone();
        for o in native_save::HINTS_ZEROED_ON_LOAD {
            want.set_u32(native_save::HINTS + o, 0);
        }
        let diff: Vec<usize> = (0..native_save::PAYLOAD_LEN).filter(|&i| back.payload[i] != want.payload[i]).collect();
        assert!(diff.is_empty(), "{name}: {} bytes differ, first at {:#x}", diff.len(), diff[0]);
        assert_eq!(back.to_bytes(), want.to_bytes(), "{name}");
    }
}

#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn clone_progress_reaches_the_file() {
    let (data, ctx) = context();
    let base = NativeSave::parse(fixtures()[2].1).unwrap();
    let mut p = Profile::from_native(&base, &ctx);
    p.cash = 12345;
    p.stage = 2;
    let won = data.events.iter().find(|e| e.stage == 1 && e.category == career::Category::Normal && !e.suv).unwrap();
    p.completed.insert(won.index);
    p.installed.insert(0x10, 0xDEAD_BEEF);
    p.perf.insert(3);
    p.perf_owned.insert(3);
    p.races_entered = 7;
    let s = p.to_native(&base, &ctx);
    let again = NativeSave::parse(&s.to_bytes()).unwrap();
    let q = Profile::from_native(&again, &ctx);
    assert_eq!((q.cash, q.stage, q.races_entered), (12345, 2, 7));
    assert!(q.completed.contains(&won.index));
    assert_eq!(q.installed.get(&0x10), Some(&0xDEAD_BEEF));
    assert!(q.perf.contains(&3) && q.perf_owned.contains(&3));
    // The stage record files the win (0x513390) and stage 1 is unlocked.
    let st = again.stage(1);
    assert_eq!(again.u32(st + native_save::ST_NORMAL), won.unlock);
    assert_eq!(again.u8(again.stage(1) + native_save::ST_UNLOCKED), 1);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_new_profile_starts_from_the_originals_fresh_one() {
    let s = career::fresh_save("RAY");
    assert_eq!(s.name(), "RAY");
    let b = s.to_bytes();
    assert_eq!(b.len(), native_save::FILE_LEN);
    assert_eq!(NativeSave::parse(&b).unwrap(), s);
}

/// A hidden cash pickup (0x34A1E) pays once (0x528380) and its flag is
/// career +0x8768 + its +0x03 in the save (0x514685), read back by the
/// loader; the originals' new careers have every flag clear.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn cash_pickups_pay_once_and_save_their_flag() {
    let (data, ctx) = context();
    let s = NativeSave::parse(fixtures()[0].1).unwrap();
    assert!((0..native_save::BANK_FLAGS_LEN).all(|k| s.payload[native_save::BANK_FLAGS + k] == 0));
    let mut p = Profile::from_native(&s, &ctx);
    assert_eq!(data.banks.len(), 59);
    // Stage 0: only the two condition-0 pickups ($50, flags 0 and 1) are open.
    let open: Vec<_> = data.banks.iter().filter(|b| p.bank_open(&data, b)).map(|b| (b.cash, b.flag)).collect();
    assert_eq!(open, [(50, 1), (50, 0)]);
    let b = data.banks.iter().find(|b| b.flag == 1).unwrap();
    let cash = p.cash;
    assert_eq!(p.collect_bank(&data, b.trigger), Some(50));
    assert_eq!(p.collect_bank(&data, b.trigger), None);
    assert_eq!(p.cash, cash + 50);
    let back = p.to_native(&s, &ctx);
    assert_eq!(back.payload[native_save::BANK_FLAGS + 1], 1);
    assert_eq!(back.u32(native_save::CASH), cash + 50);
    let mut again = Profile::from_native(&NativeSave::parse(&back.to_bytes()).unwrap(), &ctx);
    assert!(again.banks.contains(&1) && again.collect_bank(&data, b.trigger).is_none());
    // A stage-1 pickup (condition 1, +0x04 = 0) opens past stage 0.
    let s1 = data.banks.iter().find(|b| b.condition == 1 && b.value == 0).unwrap();
    assert!(!again.bank_open(&data, s1));
    again.stage = 1;
    assert!(again.bank_open(&data, s1));
}

/// The options the profile keeps (`Settings::apply_native` /
/// `write_native`) round-trip byte for byte, and options.sav's four changes
/// (EA Trax play mode settings +0x54, track 1's state, player 1's
/// transmission settings +0x75, career difficulty +0x250) are the only
/// bytes it differs from mycar.sav in: the keyboard remap is not in the
/// profile.
#[test]
#[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
fn options_round_trip_and_reach_the_file() {
    use nfsu2::frontend::options::Settings;
    for (name, bytes) in fixtures() {
        let s = NativeSave::parse(bytes).unwrap();
        let mut o = Settings::default();
        o.apply_native(&s);
        let mut back = s.clone();
        o.write_native(&mut back);
        let diff: Vec<String> = (0..native_save::PAYLOAD_LEN).filter(|&i| back.payload[i] != s.payload[i]).map(|i| format!("{i:#x}: {} -> {}", s.payload[i], back.payload[i])).collect();
        assert!(diff.is_empty(), "{name}: {diff:?}");
    }
    let (mycar, options) = (NativeSave::parse(fixtures()[3].1).unwrap(), NativeSave::parse(fixtures()[4].1).unwrap());
    let diff: Vec<usize> = (0..native_save::PAYLOAD_LEN).filter(|&i| mycar.payload[i] != options.payload[i]).collect();
    assert_eq!(diff, [native_save::TRAX + 4, native_save::TRAX_MODE, native_save::PLAYERS + 0x15, native_save::DIFFICULTY]);
    let mut o = Settings::default();
    o.apply_native(&options);
    assert!(!o.trax_random && o.players[0].manual && o.difficulty == 2 && o.trax[0] == 2);
    let mut m = Settings::default();
    m.apply_native(&mycar);
    assert!(m.trax_random && !m.players[0].manual && m.difficulty == 1 && m.trax[0] == 1);
    let mut back = mycar.clone();
    o.write_native(&mut back);
    assert_eq!(back, options);
}

/// The garage's records in the originals: the stock list (count 0x1F),
/// Customize's "My Cars" record (filter bit 2, the Peugeot 206 with a new
/// front bumper, the stock entry's rating at +0xC), the career slots, the
/// six Ranked records, and the five dyno settings of 100 in every record.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
#[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
fn garage_records_are_where_the_writer_puts_them() {
    use native_save::*;
    let mycar = NativeSave::parse(fixtures()[3].1).unwrap();
    let fresh = career::fresh_save("NAME");
    assert_eq!(mycar.u32(LIST_COUNT), 0x1F);
    assert_eq!(mycar.u32(SLOT_COUNT), 5);
    let car = GARAGE_CARS;
    assert_eq!(fresh.u32(car + HEAD_ID), 0);
    assert_ne!(mycar.u32(car + HEAD_ID), 0);
    assert_eq!((mycar.u32(car + HEAD_USED), mycar.u32(car + HEAD_FILTER)), (1, 2));
    // The rating (0x5B69A0) is the Peugeot's stock entry's, list entry 0.
    assert_eq!(mycar.u32(car + HEAD_F32), mycar.u32(GARAGE_LIST + HEAD_F32));
    assert_eq!(mycar.f32(car + HEAD_F32).round(), 197.0);
    // The car record: filled, the car type.
    let rec = car + 0x14;
    assert_eq!(mycar.u32(rec), 1);
    assert_eq!(mycar.u32(rec + REC_CAR), bin_hash("PEUGOT"));
    for i in 0..5 {
        assert_eq!(mycar.u32(SLOTS + i * SLOT_LEN + SLOT_ID), career::slot_id(i));
        assert_eq!(mycar.u32(SLOTS + i * SLOT_LEN + HEAD_FILTER), 4);
    }
    for i in 0..RANKED_CAR_MAX {
        assert_eq!(mycar.u32(RANKED_CARS + i * RANKED_CAR_LEN + HEAD_FILTER), 0x10);
    }
    for (_, bytes) in fixtures() {
        let s = NativeSave::parse(bytes).unwrap();
        for r in (0..GARAGE_CAR_MAX).map(|i| GARAGE_CARS + i * GARAGE_CAR_LEN + 0x14).chain((0..5).map(|i| SLOTS + i * SLOT_LEN + SLOT_RECORD)).chain((0..RANKED_CAR_MAX).map(|i| RANKED_CARS + i * RANKED_CAR_LEN + 0x14)) {
            assert!(s.payload[r + REC_TUNING..r + REC_TUNING + 5 * TUNING_LEN].iter().all(|&b| b == 100));
        }
    }
}

/// The dyno settings of a career slot (car record +0x348) and the legend's
/// show flags (career +0x8869) go through the clone's profile and back.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn tuning_and_legend_reach_the_file() {
    let (_, ctx) = context();
    let base = NativeSave::parse(fixtures()[2].1).unwrap();
    let mut p = Profile::from_native(&base, &ctx);
    assert!(p.tuning.iter().all(|t| t.0 == [100; 41]));
    assert_eq!(p.map_hidden, [false; 14]);
    p.tuning[1].0[3] = 160;
    p.tuning[4].0[0x20] = 40;
    p.map_hidden[6] = true;
    p.map_hidden[13] = true;
    let s = p.to_native(&base, &ctx);
    let slot = s.slot(p.slot) + native_save::SLOT_RECORD + native_save::REC_TUNING;
    assert_eq!(s.payload[slot + native_save::TUNING_LEN + 3], 160);
    assert_eq!(s.payload[slot + 4 * native_save::TUNING_LEN + 0x20], 40);
    let legend = &s.payload[native_save::LEGEND_SHOWN..native_save::LEGEND_SHOWN + 16];
    assert_eq!(legend, [1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 1, 1, 0]);
    let q = Profile::from_native(&NativeSave::parse(&s.to_bytes()).unwrap(), &ctx);
    assert_eq!(q.tuning, p.tuning);
    assert_eq!(q.map_hidden, p.map_hidden);
}

/// mycar.sav: main-menu Customize made My Cars record 0 (TUNED_CAR_0, the
/// 206 with a new front bumper) and named it player 0's car (race record
/// +4); the clone reads both and writes them back unchanged.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn my_cars_load_and_write_back() {
    let (_, ctx) = context();
    let s = NativeSave::parse(fixtures()[3].1).unwrap();
    let p = Profile::from_native(&s, &ctx);
    let m = p.my_cars[0].as_ref().expect("TUNED_CAR_0");
    assert_eq!((m.id, m.car.as_str()), (career::tuned_car_id(0), "PEUGOT"));
    assert!(!m.installed.is_empty(), "the new front bumper");
    assert!(p.my_cars[1..].iter().all(Option::is_none));
    assert_eq!(p.menu_car_id, Some(career::tuned_car_id(0)));
    assert_eq!(p.menu_car.as_ref().map(|c| c.0.as_str()), Some("PEUGOT"));
    assert_eq!(p.to_native(&s, &ctx).to_bytes(), fixtures()[3].1);
    // Freed (0x521110) and made again from the stock 206.
    let mut q = p.clone();
    q.my_cars[0] = None;
    q.menu_car_id = Some(career::stock_id("PEUGOT"));
    let w = q.to_native(&s, &ctx);
    let back = Profile::from_native(&w, &ctx);
    assert!(back.my_cars.iter().all(Option::is_none));
    assert_eq!(back.menu_car.as_ref().map(|c| c.0.as_str()), Some("PEUGOT"));
    let fresh = Profile::from_native(&career::fresh_save("NAME"), &ctx);
    // 0x521110's clear: the head 0 but the filter 2, the car record zero.
    let o = native_save::GARAGE_CARS;
    assert_eq!(w.u32(o + 4), 0);
    assert_eq!(w.u32(o + 0x10), 2);
    assert!(w.payload[o + 0x14..o + 0x14 + native_save::RECORD_LEN].iter().all(|&b| b == 0));
    assert!(fresh.my_cars.iter().all(Option::is_none));
    q.my_cars[2] = Some(career::MyCar { id: career::tuned_car_id(2), car: "PEUGOT".into(), installed: m.installed.clone(), perf: Default::default() });
    let back = Profile::from_native(&q.to_native(&s, &ctx), &ctx);
    assert_eq!(back.my_cars[2].as_ref().map(|m| &m.installed), Some(&m.installed));
}

/// Owned visual parts go into the career slot's bit sets (0x503D50 /
/// 0x503B10) and come back.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn owned_parts_round_trip() {
    let (_, ctx) = context();
    let s = NativeSave::parse(fixtures()[2].1).unwrap();
    let mut p = Profile::from_native(&s, &ctx);
    assert!(p.owned.is_empty());
    let bits = ctx.owned_bits(&p.car);
    assert!(!bits.is_empty());
    // A few parts of different slots, rims and a hood among them.
    let pick: Vec<(u8, u32)> = [0x1Du8, 0x1E, 7, 9, 0x40]
        .iter()
        .filter_map(|&slot| bits.iter().filter(|b| b.0 == slot).nth(1).map(|b| (b.0, b.1)))
        .collect();
    assert!(pick.len() >= 3, "{pick:?}");
    p.owned.extend(pick.iter().copied());
    let w = p.to_native(&s, &ctx);
    let back = Profile::from_native(&w, &ctx);
    for x in &pick {
        assert!(back.owned.contains(x), "{x:?} lost");
    }
}

/// The memoised owned-part bits equal a fresh computation for every car
/// type, on the first call and the next (a performance audit finding).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn owned_bits_memo_matches_fresh() {
    let (_, ctx) = context();
    let mut cars: Vec<&String> = ctx.cars.values().collect();
    cars.sort();
    assert!(cars.len() > 30, "{}", cars.len());
    for car in cars {
        let fresh = ctx.owned_bits_fresh(car);
        assert_eq!(&ctx.owned_bits(car)[..], &fresh[..], "{car}");
        assert_eq!(&ctx.owned_bits(car)[..], &fresh[..], "{car} (memo)");
    }
}

/// Writes a save the way the clone does (from garage.sav: owned and fitted
/// visual parts, two My Cars records, player 0's car a My Cars record, the
/// intro played, every shop entered, Rachel's hints changed) to
/// `$NFSU2_SAVE_OUT` for loading in the original under Wine. Run with
/// `--ignored`.
#[test]
#[ignore = "manual: writes $NFSU2_SAVE_OUT for the original under Wine"]
fn write_a_clone_save_for_the_original() {
    let (data, ctx) = context();
    let out = std::env::var_os("NFSU2_SAVE_OUT").expect("NFSU2_SAVE_OUT names the file to write");
    let s = NativeSave::parse(fixtures()[2].1).unwrap();
    let mut p = Profile::from_native(&s, &ctx);
    let bits = ctx.owned_bits(&p.car);
    // Owned: the second part of a few slots; fitted: the rims and the hood.
    for slot in [0x1Du8, 0x1E, 7, 9, 0x40] {
        if let Some(b) = bits.iter().filter(|b| b.0 == slot).nth(1) {
            p.owned.insert((b.0, b.1));
            if slot == 0x1D || slot == 7 {
                p.installed.insert(b.0, b.1);
            }
        }
    }
    let db = ctx.db.as_ref().unwrap();
    let mine = |car: &str, slot: usize| db.slot_parts(car, slot).iter().find(|q| Some(q.hash) != db.stock(car, slot).map(|s| s.hash)).map(|q| q.hash).unwrap();
    p.my_cars[0] = Some(career::MyCar { id: career::tuned_car_id(0), car: "PEUGOT".into(), installed: [(1u8, mine("PEUGOT", 1))].into(), perf: Default::default() });
    p.my_cars[1] = Some(career::MyCar { id: career::tuned_car_id(1), car: "GOLF".into(), installed: [(1u8, mine("GOLF", 1))].into(), perf: [1u8].into() });
    p.menu_car_id = Some(career::tuned_car_id(1));
    p.intro_played = true;
    p.entered.extend(data.shops.iter().map(|s| s.hash));
    p.visited.extend(data.shops.iter().map(|s| s.hash));
    p.hints.garage = true;
    p.hints.events = true;
    p.hints.complaints = 2;
    let w = p.to_native(&s, &ctx);
    std::fs::write(out, w.to_bytes()).unwrap();
}

/// The records table (profile +0xA0FC): 0x502DF0's add / better rules,
/// 0x502AA0 / 0x502C50's lookups, and the table reaching the file in its
/// entries' places.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn records_are_kept_and_reach_the_file() {
    let mut p = Profile::new("NAME");
    assert!(p.add_record(false, 41, 1, 0, 4000 * 90));
    assert!(!p.add_record(false, 41, 1, 0, 4000 * 95), "a slower lap is not kept");
    assert!(p.add_record(false, 41, 1, 0, 4000 * 85));
    assert!(!p.add_record(false, 41, 1, 0, 0) && !p.add_record(false, 42, 1, 0, i32::MAX), "no time");
    assert!(p.add_record(false, 41, 0, 0, 4000 * 70), "another laps flag is its own record");
    assert!(p.add_record(true, 1301, 0, 0, 12000));
    assert!(!p.add_record(true, 1301, 0, 0, 9000), "a lower score is not kept");
    assert!(p.add_record(true, 1301, 0, 0, 15000));
    assert_eq!(p.best_score(1301, false), 15000);
    assert_eq!(p.best_score(1301, true), 0);
    assert_eq!(p.records.iter().filter(|r| r.track != 0).count(), 3);
    let (data, ctx) = context();
    let t = data.tracks.values().min_by_key(|t| t.number).unwrap();
    let mut q = Profile::new("NAME");
    let default = q.best_time(t, false);
    assert_eq!(default, (t.default_record[0] * 0.001 * 4000.0 + 0.5) as i32, "none: the track's default");
    q.add_record(false, t.number as i16, 1, 0, 4000 * 90);
    q.add_record(false, t.number as i16, 0, 0, 4000 * 70);
    q.add_record(false, t.number as i16, 0, 1, 4000 * 60);
    assert_eq!(q.best_time(t, false), 4000 * 70, "the lowest of any laps flag, forward");
    let s = NativeSave::parse(fixtures()[2].1).unwrap();
    let w = p.to_native(&s, &ctx);
    let o = native_save::RECORDS;
    assert_eq!((w.u16(o) as i16, w.u8(o + 2), w.u8(o + 3), w.u32(o + 4)), (41, 1, 0, 4000 * 85));
    let back = Profile::from_native(&w, &ctx);
    assert_eq!(back.records, p.records);
    // The race totals after the table (+0xA00..+0xA50) are the profile's:
    // a loaded profile writes them back as loaded.
    let loaded = Profile::from_native(&s, &ctx);
    let w2 = loaded.to_native(&s, &ctx);
    assert_eq!(w2.payload[o + 0xA00..o + native_save::RECORDS_LEN], s.payload[o + 0xA00..o + native_save::RECORDS_LEN]);
}

/// The race totals (profile +0xA00): 0x541A60 at a race's load and
/// 0x515990 after it, as seen under Wine in the stage-2 playthrough (a
/// sponsor circuit, kind 3: +0xA04 8 → 9 and +0xA34 1 → 2 at the load;
/// after the win +0xA08 6 → 7, +0xA38 1 → 2 and +0xA00 += the distance;
/// a URL race, no kind: only +0xA04, +0xA08 and +0xA00).
#[test]
fn race_totals_follow_the_race() {
    let mut p = Profile::new("NAME");
    p.totals.race_loaded(3);
    assert_eq!((p.totals.started(), p.totals.kind(3)), (1, [1, 0, 0]));
    p.totals.race_done(3, 1, 290.5, 1, career::style_score(1234.9, 0.0));
    assert_eq!((p.totals.firsts(), p.totals.others(), p.totals.kind(3)), (1, 0, [1, 1, 1234]));
    p.totals.race_loaded(1);
    p.totals.race_done(1, 4, 400.0, 9, 0);
    assert_eq!((p.totals.kind(1), p.totals.totaled()), ([1, 0, 0], 1));
    assert_eq!(career::style_score(10.9, 2.7), 20);
    p.totals.race_loaded(-1);
    p.totals.race_done(-1, 4, 100.0, 1, 50);
    assert_eq!((p.totals.started(), p.totals.firsts(), p.totals.others()), (3, 1, 2));
    assert_eq!(p.totals.distance(), 790.5);
    assert_eq!((0..4).map(|k| p.totals.kind(k)[0]).sum::<u32>(), 2, "no kind: no per-kind count");
}

/// The six Ranked records (garage +0x8130, filter 0x10: the online
/// ranked cars, never listed offline, 0x4F9D90) go back byte for byte.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn ranked_cars_are_kept() {
    let (_, ctx) = context();
    for (name, bytes) in fixtures() {
        let s = NativeSave::parse(bytes).unwrap();
        let mut p = Profile::from_native(&s, &ctx);
        p.cash += 1000;
        p.my_cars[5] = Some(career::MyCar { id: career::tuned_car_id(5), car: "GOLF".into(), ..Default::default() });
        let w = p.to_native(&s, &ctx);
        let r = native_save::RANKED_CARS..native_save::RANKED_CARS + native_save::RANKED_CAR_LEN * native_save::RANKED_CAR_MAX;
        assert_eq!(w.payload[r.clone()], s.payload[r], "{name}");
    }
}

/// A My Cars record the clone makes holds the stock paint and the vinyl
/// colours the original's new records hold (mycar.sav: the 206's).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn a_new_my_cars_record_has_the_stock_paint() {
    let (_, ctx) = context();
    let orig = NativeSave::parse(fixtures()[3].1).unwrap();
    let mut p = Profile::from_native(&NativeSave::parse(fixtures()[0].1).unwrap(), &ctx);
    p.my_cars[0] = Some(career::MyCar { id: career::tuned_car_id(0), car: "PEUGOT".into(), ..Default::default() });
    let w = p.to_native(&NativeSave::parse(fixtures()[0].1).unwrap(), &ctx);
    let r = native_save::GARAGE_CARS + 0x14 + native_save::REC_PARTS;
    for slot in [0x3F].into_iter().chain(0x4D..=0x5C) {
        assert_eq!(w.u32(r + 4 * slot), orig.u32(r + 4 * slot), "slot {slot:#x}");
    }
}

/// Event +0x02 / +0x04 (0x4FFDD0, 0x513A50): the movie before the race
/// and the movie after a win, each a u16 into the string pool; the name
/// is the u16 at +0x00 (a u32 read left the events with a movie nameless).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn event_movies_and_names() {
    let (data, _) = context();
    let by = |h: u32| data.events.iter().find(|e| e.unlock == h).unwrap();
    let url1 = by(0x1185_EF06);
    assert_eq!((url1.name.as_str(), url1.movie.as_str(), url1.won_movie.as_str()), ("S2_URL_1", "SCENE17", "SCENE08"));
    assert_eq!(by(0x1185_EF08).won_movie, "SUBURBAN", "stage 2's last race");
    assert_eq!(by(0x5E80_2BCE).won_movie, "INDUSTEAST");
    assert_eq!(by(0xAB7A_688E).won_movie, "INDUSTWEST");
    assert_eq!(by(0xB0DE_F68B).movie, "SCENE20");
    assert!(data.events.iter().all(|e| !e.name.is_empty()), "every event named");
}

/// A win restarted keeps its stage-record entry (0x513390 at the race's
/// end) while the visit entry stays unwon: `Profile::filed`, saved in the
/// stage record's list and read back as filed.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn a_filed_win_round_trips() {
    let (data, ctx) = context();
    let s = NativeSave::parse(fixtures()[0].1).unwrap();
    let mut p = Profile::from_native(&s, &ctx);
    let e = data.events.iter().find(|e| e.stage == 2 && e.category == career::Category::Normal && !e.suv).unwrap();
    p.filed.insert(e.index);
    assert_eq!(p.stage_wins_in(&data, 2, career::Category::Normal).len(), 1);
    assert!(!p.won(&data, e.unlock));
    let back = Profile::from_native(&p.to_native(&s, &ctx), &ctx);
    assert!(back.filed.contains(&e.index) && !back.completed.contains(&e.index));
}

/// Under Wine (2026-10-31): a clone stage-2 save, the first stage-2 win
/// (S2_SPRINT_3) filed but not settled (stage record +0xE4 = 0): the
/// sponsor popup comes and the sponsorship screen lists all five of the
/// stage's sponsors, STREETGLOW first.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn stage_two_sponsors_after_the_first_win() {
    let (data, ctx) = context();
    let s = NativeSave::parse(fixtures()[0].1).unwrap();
    let mut p = Profile::from_native(&s, &ctx);
    p.stage = 2;
    // The save's stage 1: five wins, 1500 points (the average a win is
    // the previous stage's record, 0x5005E0 on 0x861F14 + 0x104 × stage).
    p.stage_rep.insert(1, (1500.0, 5));
    let e = data.events.iter().find(|e| e.unlock == 0x7C80_E775).unwrap();
    p.filed.insert(e.index);
    let names: Vec<&str> = p.sponsor_offers(&data).iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["STREETGLOW", "EUROLITE", "EDELBROCK", "TOYO", "YOKOHAMA"]);
    assert!(p.needs_sponsor(&data));
}

/// New visit-table entries are appended in the order they were first
/// touched (0x512F10), not grouped by kind: a shop entered before an
/// event raced comes first (stage-1 playthrough, CRIB_1 before
/// STAGE_1_SHORT_1).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn visits_append_in_order() {
    let (data, ctx) = context();
    let s = NativeSave::parse(fixtures()[2].1).unwrap();
    let mut p = Profile::from_native(&s, &ctx);
    let shop = data.shops.iter().find(|x| !p.entered.contains(&x.hash) && !s.visits().iter().any(|v| v.0 == x.hash)).unwrap().hash;
    let ev = data.events.iter().find(|e| !s.visits().iter().any(|v| v.0 == e.unlock) && e.unlock != career::DDAY_EVENT_A && e.unlock != career::DDAY_EVENT_B).unwrap().clone();
    p.touch(shop);
    p.entered.insert(shop);
    p.race_started(&ev);
    let v = p.to_native(&s, &ctx).visits();
    let at = |h: u32| v.iter().position(|x| x.0 == h).unwrap();
    assert_eq!(v.len(), s.visits().len() + 2);
    assert!(at(shop) < at(ev.unlock), "the shop was touched first");
}

/// A finished career (the original's own
/// finished-career save, 2020-01-20; stage 6, 224 wins,
/// $390,885). The cost table's win unlocks (0x515360, types 2 / 3 / 4)
/// count the wins of stage record value >> 16, not of the current stage,
/// and a shop's win requirement (0x512870) those of the shop's own stage
/// (+0x9D). Under Wine (2026-10-31) the original showed with this save:
/// the Performance Shop's Engine Street / Pro / Extreme packages open, the
/// Body Shop's categories all open but Wide Body Kits (no kit for the
/// Lancer). So every level 1–3 of every cost category is open.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn finished_career_unlocks_by_the_parts_own_stage() {
    let (data, ctx) = context();
    let save = NativeSave::parse(&common::private("tests/fixtures/native_saves/career_complete.sav")).unwrap();
    let p = Profile::from_native(&save, &ctx);
    assert!(!save.name().is_empty(), "the save names its profile");
    assert_eq!((p.name.as_str(), p.stage, p.cash, p.car.as_str()), (save.name().as_str(), 6, 390_885, "LANCEREVO8"));
    // The current stage has no wins: the old rule (current stage) locked
    // 63 levels in 30 categories here.
    assert!(p.stage_wins(&data, career::Category::Normal).is_empty());
    for (i, levels) in data.costs.categories.iter().enumerate() {
        for l in 1..=levels.len().min(3) as u8 {
            assert!(p.part_unlocked(&data, i as u8 + 1, l), "category {:#04x} level {l}", i + 1);
        }
        assert!(!p.part_unlocked(&data, i as u8 + 1, 4), "uniques are never bought (0x515360 level 4)");
    }
    for s in data.shops.iter().filter(|s| s.stage <= p.stage) {
        assert!(p.shop_open(&data, s), "shop {}", s.name);
    }
    // A new career: an unlock keyed to a later stage's wins stays shut
    // (engine Pro, cost category 1 level 2: 13 normal wins of stage 3).
    let start = Profile::from_native(&NativeSave::parse(fixtures()[0].1).unwrap(), &ctx);
    let pro = &data.costs.categories[0][1];
    assert_eq!((pro.unlock, pro.unlock_value), (2, 0x0003_000D));
    assert!(!start.part_unlocked(&data, 1, 2));
}

/// Where Resume Career puts the car (career +0x8744 flag, +0x8750 place):
/// the original's saves at each D-Day step and after the first win
/// (stage1_win_streetx.sav: Street X won in stage 1, UI_PostRace's save;
/// stage-1 playthrough). Entering a shop keeps the trigger's place
/// with the flag set (0x54CB00, 0x54CC0C); a race engaged keeps the engage
/// place with the flag clear (0x541170), which Resume's garage exit sets
/// again (0x4AF616).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn originals_keep_the_resume_place() {
    let (_, ctx) = context();
    let s02: &[u8] = &common::private("tests/fixtures/native_saves/stage1_win_streetx.sav");
    let cases: [(&str, &[u8], bool, (f32, f32)); 4] = [
        ("career_start", fixtures()[0].1, true, (1695.25, -883.64)),
        ("car_lot", fixtures()[1].1, true, (-726.51, -875.20)),
        ("garage", fixtures()[2].1, true, (654.11, -97.64)),
        ("first win", s02, false, (462.42, -46.16)),
    ];
    for (name, bytes, set, (x, y)) in cases {
        let p = Profile::from_native(&NativeSave::parse(bytes).unwrap(), &ctx);
        assert!(p.intro_played, "{name}: the intro movie played (0x86347C)");
        assert_eq!(p.explore_set, set, "{name}: the place flag");
        let (at, _) = p.explore_pos.unwrap_or_else(|| panic!("{name}: a kept place"));
        assert!((at.x - x).abs() < 0.01 && (at.y - y).abs() < 0.01, "{name}: {at:?}");
        // Written back as it was.
        let back = p.to_native(&NativeSave::parse(bytes).unwrap(), &ctx);
        let orig = NativeSave::parse(bytes).unwrap();
        assert_eq!(back.payload[native_save::PLACE_SET..native_save::PLACE_HEADING + 2], orig.payload[native_save::PLACE_SET..native_save::PLACE_HEADING + 2], "{name}: the place's bytes");
    }
}

/// the finished-career save's shop visits: every shop the save's visit table holds in state 5
/// (+6 = 5, entered: 22 shops, all but UC_CAR_LOT and THE_UNIQUES_SHOP)
/// loads as entered, so the engage dialog asks the generic question
/// (0x512920 → 0x15D6B438) rather than the shop's first-visit `_BODY`
/// text; the original under Wine (2026-10-31, the same save) asked "Would
/// you like to enter the Shop?" at IND_BODYSHOP_2, IND_CAR_SPECIALTY_2 and
/// IND_GRAPHICS. The state survives a save.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn finished_career_has_entered_its_shops() {
    let (data, ctx) = context();
    let save = NativeSave::parse(&common::private("tests/fixtures/native_saves/career_complete.sav")).unwrap();
    let in_state_5: Vec<u32> = save.visits().iter().filter(|(h, b)| b[2] == 5 && data.shops.iter().any(|s| s.hash == *h)).map(|v| v.0).collect();
    assert_eq!(in_state_5.len(), 22);
    let p = Profile::from_native(&save, &ctx);
    for s in &data.shops {
        assert_eq!(p.entered.contains(&s.hash), !matches!(s.name.as_str(), "UC_CAR_LOT" | "THE_UNIQUES_SHOP"), "shop {}", s.name);
    }
    let again = Profile::from_native(&p.to_native(&save, &ctx), &ctx);
    assert_eq!(again.entered, p.entered);
}

/// career_complete_resaved.sav: career_complete.sav loaded by SPEED2.EXE under Wine
/// and saved again from the Profile Manager (2026-10-31). The
/// original's load-and-resave changes only the four hint dwords the career
/// loader 0x529240 zeroes (career +0x8898, +0x8984, +0x898C, +0x8994);
/// the clone's load and save gives the same file byte for byte (won
/// events and milestones whose visit +4 the file holds as 0 keep it).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn clone_rewrite_matches_the_originals_resave() {
    let (_, ctx) = context();
    let input = &common::private("tests/fixtures/native_saves/career_complete.sav");
    let resaved = &common::private("tests/fixtures/native_saves/career_complete_resaved.sav");
    let s = NativeSave::parse(input).unwrap();
    let back = Profile::from_native(&s, &ctx).to_native(&s, &ctx);
    let diff: Vec<usize> = (0..native_save::PAYLOAD_LEN).filter(|&i| back.payload[i] != resaved[0x24 + i]).collect();
    assert!(diff.is_empty(), "{} bytes differ, first at {:#x}", diff.len(), diff[0]);
    assert_eq!(back.to_bytes(), resaved.as_slice());
}

/// Community saves (not in the repo: no redistribution terms), each
/// `<name>.sav` with the original's own load-and-resave `<name>.resave.sav`
/// beside it, in the folder `NFSU2_COMMUNITY_SAVES` names: the clone's
/// load and save must give the original's resave. Manual (`--ignored`): the
/// folder is not part of any set the test kit knows.
#[test]
#[ignore = "manual: needs the community saves folder (NFSU2_COMMUNITY_SAVES) next to the game files"]
fn community_saves_rewrite_as_the_original_resaves_them() {
    let (_, ctx) = context();
    let dir = std::env::var_os("NFSU2_COMMUNITY_SAVES").expect("NFSU2_COMMUNITY_SAVES names the community saves folder");
    let rd = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.to_string_lossy()));
    let mut n = 0;
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".resave.sav") else { continue };
        let Ok(input) = std::fs::read(p.with_file_name(format!("{stem}.sav"))) else { continue };
        let resaved = std::fs::read(&p).unwrap();
        let s = NativeSave::parse(&input).unwrap_or_else(|e| panic!("{stem}: {e:?}"));
        // The game's save: the career, then the options (the controller
        // maps kept only for attached controllers: none here, as under
        // Wine where the original resaved them with the keyboard alone).
        let mut out = Profile::from_native(&s, &ctx).to_native(&s, &ctx);
        let mut opts = nfsu2::frontend::options::Settings::default();
        opts.apply_native(&s);
        opts.pad_blocks.clear();
        opts.write_native(&mut out);
        let back = out.to_bytes();
        let diff: Vec<usize> = (0..back.len()).filter(|&i| back[i] != resaved[i]).collect();
        assert!(diff.is_empty(), "{stem}: {} bytes differ, first at {:#x}", diff.len(), diff[0]);
        n += 1;
    }
    eprintln!("community saves checked: {n}");
}

/// A stage record's +0xFC is a latch (0x532C70 sets it once the stage is
/// completed; 0x528330 then advances only the current stage): a save
/// holding the current stage's record completed keeps it on a rewrite.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn completed_stage_records_are_kept() {
    let (_, ctx) = context();
    let mut s = NativeSave::parse(&common::private("tests/fixtures/native_saves/career_complete.sav")).unwrap();
    s.set_u32(native_save::STAGE, 5);
    let p = Profile::from_native(&s, &ctx);
    assert!(p.stage_done.contains(&5));
    let back = p.to_native(&s, &ctx);
    assert_eq!(back.u8(back.stage(5) + native_save::ST_UNLOCKED), 1);
    assert_eq!(back.u32(native_save::STAGE), 5);
}

/// What Rachel says on a resume as the clone runs it (game.rs
/// `career_intro`, the crib's and car lot's INIT, then 0x551080 each
/// free-roam frame): 0x524250 when the career's D-Day event is A, the
/// crib's 0x5243E0(0x11) and the car lot's 0x5243E0(4) when asked, then
/// `secs` of driving with Rachel always free. Returns the kinds' lines and
/// what they do, in order.
fn resume_lines(p: &mut Profile, crib: bool, car_lot: bool, secs: u32) -> Vec<(Option<nfsu2::frontend::dday::Fire>, Vec<u32>)> {
    let mut out = Vec::new();
    if p.intro == Some(career::DDAY_EVENT_A) {
        p.hints.dday_a();
    }
    let mut kinds = Vec::new();
    if crib {
        kinds.push(0x11);
    }
    if car_lot {
        kinds.push(4);
    }
    for k in kinds {
        if let Some(l) = p.hints.fe_line(k, None) {
            out.push((None, l));
        }
    }
    let dt = 1.0 / 60.0;
    for _ in 0..secs * 60 {
        p.drive_time += dt;
        if let Some(f) = p.hints.tick(p.drive_time, dt, true)
            && let Some(l) = f.line
        {
            out.push((f.fire, l));
        }
    }
    out
}

/// A finished career (career_complete.sav, the original's own save) has
/// every once-flag of Rachel's hints set and no slot queued, so resuming it
/// — crib, car lot, five minutes of driving — says no first-career line:
/// no SMS_LOC_EVENTS_OUT_THERE / SMS_LOC_GO_TO_GARAGE hint, no D-Day reveal.
/// Nothing in 0x5243E0, 0x551330 → 0x551080 → 0x54BD40 or 0x53E0C0 looks
/// at the stage (traced 2026-11-01): what the hint object holds decides,
/// so a career whose object is all zeros (a file written by a clone
/// before the hints were saved, 2026-10-30) gets the first-career lines in
/// the original too. Early saves still get theirs.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn a_finished_career_says_no_first_career_hints() {
    use nfsu2::frontend::dday::{Fire, SMS_LOC_EVENTS_OUT_THERE, SMS_LOC_GO_TO_GARAGE};
    let (_, ctx) = context();
    let load = |b: &[u8]| Profile::from_native(&NativeSave::parse(b).unwrap(), &ctx);
    // The finished career, the original's file and its own resave.
    for b in [&common::private("tests/fixtures/native_saves/career_complete.sav")[..], &common::private("tests/fixtures/native_saves/career_complete_resaved.sav")[..]] {
        let mut p = load(b);
        assert_eq!(p.stage, 6);
        assert_eq!(resume_lines(&mut p, true, true, 300), vec![], "a finished career says nothing");
        assert!(p.hints.slots.iter().all(|s| s.1 == 0));
    }
    let msg = |v: &[(Option<Fire>, Vec<u32>)]| v.iter().filter_map(|(f, _)| match f {
        Some(Fire::Message(m)) => Some(*m),
        _ => None,
    }).collect::<Vec<_>>();
    // A new career at the airport: the reveal (kind 2) 15 s in, kind 3.
    let mut p = load(fixtures()[0].1);
    let v = resume_lines(&mut p, false, false, 60);
    assert_eq!(v.len(), 2, "{v:X?}"); // kinds 2 and 3
    assert_eq!(v[0].0, Some(Fire::Reveal));
    // Past the car lot: the garage hint.
    let mut p = load(fixtures()[1].1);
    assert_eq!(msg(&resume_lines(&mut p, false, false, 60)), vec![SMS_LOC_GO_TO_GARAGE]);
    // At the garage: the events hint (kinds 6, 12, 10 queued).
    let mut p = load(fixtures()[2].1);
    assert_eq!(msg(&resume_lines(&mut p, true, false, 60)), vec![SMS_LOC_EVENTS_OUT_THERE]);
    // Stage 1 after the first win: kind 5's line, once.
    let mut p = load(&common::private("tests/fixtures/native_saves/stage1_win_streetx.sav"));
    let v = resume_lines(&mut p, true, false, 60);
    assert_eq!(v.len(), 1, "{v:X?}");
    // The same finished career with the hint object zeroed says the
    // first-career lines (no stage gate in the binary).
    let mut s = NativeSave::parse(&common::private("tests/fixtures/native_saves/career_complete.sav")).unwrap();
    s.payload[native_save::HINTS..native_save::HINTS + native_save::HINTS_LEN].fill(0);
    let mut p = Profile::from_native(&s, &ctx);
    assert_eq!(msg(&resume_lines(&mut p, true, true, 60)), vec![SMS_LOC_EVENTS_OUT_THERE, SMS_LOC_GO_TO_GARAGE]);
}

/// New profiles after regmybank and ordermebaby on the title (SPEED2.EXE
/// under Wine, docs/playing/cheats.md): cheats_boot_create.sav is the boot's
/// profile NAME (no profile current: the career in memory, bank 200 paid at
/// once by regmybank), cheats_manager_create.sav the Profile Manager's
/// Create made after it (TWO: "Creating a new profile will …" Yes resets
/// the career, 0x532840, bank 200 + 1000). The clone's new profile with
/// that bank writes both byte for byte; nothing else of the codes is saved.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn new_profiles_after_bonus_codes_match_the_originals() {
    let (_, ctx) = context();
    let saves: [(&str, &[u8], u32); 2] = [
        ("NAME", &common::private("tests/fixtures/native_saves/cheats_boot_create.sav"), 200),
        ("TWO", &common::private("tests/fixtures/native_saves/cheats_manager_create.sav"), 1200),
    ];
    let t = nfsu2::frontend::cheats::Active::default();
    t.set(1 << 17 | 1 << 19);
    for (name, bytes, bank) in saves {
        let s = NativeSave::parse(bytes).unwrap();
        assert_eq!(s.u32(native_save::CASH), bank, "{name}");
        let mut p = Profile::new(name);
        p.cash = t.creation_cash(name == "TWO", nfsu2::frontend::cheats::pay_now(200, false, 0));
        let ours = p.to_native(&career::fresh_save(name), &ctx);
        let diff: Vec<usize> = (0..native_save::PAYLOAD_LEN).filter(|&i| ours.payload[i] != s.payload[i]).collect();
        assert!(diff.is_empty(), "{name}: {} bytes differ, first at {:#x}", diff.len(), diff[0]);
        assert_eq!(ours.to_bytes(), bytes, "{name}");
    }
    // Wine: regmybank typed, the boot's create declined, then
    // main-menu Customize (Peugeot 206) and its Back's save point with no
    // profile current: "Would you like to create a new profile?" (state 2)
    // → NAME saved with the bonus bank, as the boot's create.
    let later = NativeSave::parse(&common::private("tests/fixtures/native_saves/cheats_later_create.sav")).unwrap();
    assert_eq!(later.u32(native_save::CASH), 200);
    assert_eq!(later.name(), "NAME");
}

/// Every career stage's save (the original's own saves at stage 1's first
/// win, stage 4's and 5's starts and first wins, the finished career, and
/// the clone's own stage-5 save) loads into the clone and is written back
/// as the original's own load-and-save would leave it: the input with the
/// four hint dwords 0x529240 zeroes on load, nothing else
/// (docs/notes/parity/world-systems-parity.md, Saves).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn every_stage_save_rewrites_as_the_original_resaves_it() {
    let (_, ctx) = context();
    let stages: [(&str, &[u8]); 7] = [
        ("stage1_win_streetx", &common::private("tests/fixtures/native_saves/stage1_win_streetx.sav")),
        ("stage4_start", &common::private("tests/fixtures/native_saves/stage4_start.sav")),
        ("stage4_win1", &common::private("tests/fixtures/native_saves/stage4_win1.sav")),
        ("stage5_start", &common::private("tests/fixtures/native_saves/stage5_start.sav")),
        ("stage5_win1", &common::private("tests/fixtures/native_saves/stage5_win1.sav")),
        ("career_end_won", &common::private("tests/fixtures/native_saves/career_end_won.sav")),
        ("clone_s5_final", &common::private("tests/fixtures/native_saves/clone_s5_final.sav")),
    ];
    let mut bad = Vec::new();
    for (name, bytes) in stages {
        let s = NativeSave::parse(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let back = Profile::from_native(&s, &ctx).to_native(&s, &ctx);
        let mut want = s.clone();
        for o in native_save::HINTS_ZEROED_ON_LOAD {
            want.set_u32(native_save::HINTS + o, 0);
        }
        let diff: Vec<usize> = (0..native_save::PAYLOAD_LEN).filter(|&i| back.payload[i] != want.payload[i]).collect();
        if !diff.is_empty() {
            bad.push(format!("{name}: {} bytes differ: {:x?}", diff.len(), &diff[..diff.len().min(12)]));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// A career slot whose car type the context does not
/// know (here the lot's 240SX taken out of the table) keeps its car flag,
/// record and the own-car flag on a round trip, byte for byte.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn an_unknown_slot_car_is_kept_byte_for_byte() {
    let (_, mut ctx) = context();
    ctx.cars.retain(|_, n| n != "240SX");
    let s = NativeSave::parse(fixtures()[1].1).unwrap();
    let o = s.slot(0);
    assert_eq!(s.u8(o + native_save::SLOT_HAS_CAR), 1);
    let p = Profile::from_native(&s, &ctx);
    assert!(p.slots[0].car.is_none() && p.slots[0].opaque);
    // No free slot: the car lot does not put a car over it on its own.
    assert_ne!(p.free_slot(), Some(0));
    let back = p.to_native(&s, &ctx);
    let len = native_save::SLOT_LEN;
    assert_eq!(back.payload[o..o + len], s.payload[o..o + len]);
    let mut want = s.clone();
    for h in native_save::HINTS_ZEROED_ON_LOAD {
        want.set_u32(native_save::HINTS + h, 0);
    }
    let diff: Vec<usize> = (0..native_save::PAYLOAD_LEN).filter(|&i| back.payload[i] != want.payload[i]).collect();
    assert!(diff.is_empty(), "{} bytes differ, first at {:#x}", diff.len(), diff[0]);
}

/// With no car types (GlobalB's parse failed) the
/// context refuses to write a profile; with them it does not.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn no_car_types_refuses_to_save() {
    let (mut data, ctx) = context();
    assert!(ctx.check_writable().is_ok());
    let root = common::game_root();
    data.makes.clear();
    let empty = NativeContext::new(&root, &data);
    assert!(empty.cars.is_empty());
    let e = empty.check_writable().unwrap_err();
    assert!(e.to_string().contains("car types"), "{e}");
}

/// The trigger messages' counters (0x864210 / 0x86420C,
/// not in the native file) survive a Resume (`Profile::load`).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn trigger_counters_survive_resume() {
    let (data, _) = context();
    let root = common::game_root();
    NativeContext::init(&root, &data);
    let ctx = NativeContext::get().unwrap();
    let s = NativeSave::parse(fixtures()[2].1).unwrap();
    let mut p = Profile::from_native(&s, ctx);
    p.stage = 2;
    p.fire_triggers(&data, 0, 4);
    assert_eq!(p.trigger_counters.magazine, 1);
    nfsu2::frontend::profiles::set_current("NAME", s);
    let resumed = Profile::load().unwrap();
    assert_eq!(resumed.trigger_counters, p.trigger_counters);
}

/// A stage definition listing more sponsors than the
/// five `sponsor_wins` entries no longer panics in `sponsor_offers`.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_sixth_sponsor_does_not_panic() {
    let (mut data, _) = context();
    let def = &mut data.stages[2];
    while def.sponsors.len() < 6 {
        let h = def.sponsors[0];
        def.sponsors.push(h);
    }
    let mut p = Profile::new("x");
    p.stage = 2;
    p.sponsor_offers(&data);
}

/// A stage record's win list or covers entry whose hash the context does
/// not know (an edited save, another build) is kept on a round trip.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn unknown_win_and_cover_hashes_are_kept() {
    let (_, ctx) = context();
    let mut s = NativeSave::parse(fixtures()[2].1).unwrap();
    let o = s.stage(1);
    let unknown = 0x1234_5679;
    assert!(!ctx.event_index.contains_key(&unknown) && !ctx.milestones.contains(&unknown));
    let free = (0..35).find(|&j| s.u32(o + native_save::ST_NORMAL + 4 * j) == 0).unwrap();
    s.set_u32(o + native_save::ST_NORMAL + 4 * free, unknown);
    s.set_u32(o + native_save::ST_COVERS, unknown);
    let back = Profile::from_native(&s, &ctx).to_native(&s, &ctx);
    assert_eq!(back.u32(o + native_save::ST_NORMAL + 4 * free), unknown);
    assert_eq!(back.u32(o + native_save::ST_COVERS), unknown);
}

/// Settings +0x20 (0x83AA24) is 127.5 in the fresh save and in the
/// original's saves (the finished-career save with the Jump Camera off included): the
/// event cameras' gate for kinds 1..5 is open natively.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn settings_20_is_the_constructors_127_5() {
    assert_eq!(career::fresh_save("NAME").f32(native_save::SETTINGS_20), native_save::SETTINGS_20_DEFAULT);
    for b in [&common::private("tests/fixtures/native_saves/career_complete.sav")[..], &common::private("tests/fixtures/native_saves/career_end_won.sav"), &common::private("tests/fixtures/native_saves/stage5_win1.sav")] {
        assert_eq!(NativeSave::parse(b).unwrap().f32(native_save::SETTINGS_20), native_save::SETTINGS_20_DEFAULT);
    }
}

/// A profile imported or restored under a new name (the launcher's Saves
/// page, `launcher::saves::store::Files::renamed`): the folder, the file
/// and the payload's name change, nothing else but the checksums. The game
/// lists it under the new folder name, loads it with the new name, and its
/// own save writes the same bytes back.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn a_renamed_copy_loads_under_its_new_name() {
    use nfsu2::launcher::saves::store::{FileCopy, Files};
    let (_, ctx) = context();
    let root = std::env::temp_dir().join(format!("ob-renamed-{}", nfsu2::unique_tag()));
    for f in ["career_complete_resaved.sav", "garage.sav", "stage4_win1.sav"] {
        let b = common::private(&format!("tests/fixtures/native_saves/{f}"));
        let files = Files { profile: FileCopy { name: "NAME".into(), bytes: b.clone(), mtime: None }, covers: Vec::new() };
        let renamed = files.renamed("MIKE2").unwrap();
        let _ = std::fs::remove_dir_all(&root);
        renamed.write_all(&root).unwrap();
        assert_eq!(nfsu2::frontend::profiles::list_in(&root), ["MIKE2"], "{f}");
        let file = nfsu2::frontend::profiles::find_in(&root, "mike2");
        let bytes = std::fs::read(&file).unwrap();
        let s = NativeSave::parse(&bytes).unwrap();
        let p = Profile::from_native(&s, &ctx);
        assert_eq!(p.name, "MIKE2", "{f}");
        // Everything but the name as the original's file.
        let orig = Profile::from_native(&NativeSave::parse(&b).unwrap(), &ctx);
        assert_eq!((p.cash, p.stage, p.races_entered), (orig.cash, orig.stage, orig.races_entered), "{f}");
        if f == "career_complete_resaved.sav" {
            assert_eq!(p.to_native(&s, &ctx).to_bytes(), bytes, "the game's own save of it: the same bytes");
            // The file the original loaded under Wine as its only profile
            // ("Load complete.", the help bar showing MIKE2;
            // docs/formats/saves-options.md).
            assert_eq!(
                nfsu2::launcher::saves::store::sha256(&bytes),
                "9ab470c0d3d7168b3543b41b7c65358f0def33df74810eecaadfc1f4fc36375e"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}
