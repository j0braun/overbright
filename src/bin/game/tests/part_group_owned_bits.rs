//! Owned bits shared by a part group (0x503B10), on sandbox
//! profiles (`test_isolation`).
use super::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// Diagnostic: per car type and slot, the parts that share one owned bit
/// (0x503B10's position n is the same for them). Run with `--ignored
/// --nocapture`.
#[test]
#[ignore = "diagnostic: lists the parts that share an owned bit"]
fn list_shared_owned_bits() {
    testkit::game();
    let game = Game::new().unwrap();
    let ctx = career::NativeContext::get().unwrap();
    let mut cars: Vec<&String> = ctx.cars.values().collect();
    cars.sort();
    cars.dedup();
    for car in cars {
        let mut by: std::collections::BTreeMap<(u8, usize, u8), Vec<u32>> = Default::default();
        for &(slot, hash, at, mask) in ctx.owned_bits(car).iter() {
            by.entry((slot, at, mask)).or_default().push(hash);
        }
        for ((slot, at, mask), v) in by {
            if v.len() > 1 {
                let db = &game.game_data.parts;
                let levels: Vec<u8> =
                    v.iter().filter_map(|h| db.slot_parts(car, slot as usize).into_iter().find(|p| p.hash == *h).map(|p| p.level)).collect();
                println!("{car} slot {slot:#04x} byte {at:#x} mask {mask:#04x}: {} parts, levels {levels:?}", v.len());
            }
        }
    }
}

/// Diagnostic: the owned bits that differ between two saves
/// (`NPF2_A`, `NPF2_B`), decoded to the parts of the current car.
#[test]
#[ignore = "diagnostic: decodes the owned-bit difference of two saves"]
fn diff_owned_bits() {
    use nfsu2::formats::native_save::{NativeSave, SLOT_EXTRA, SLOT_LEN};
    testkit::game();
    let game = Game::new().unwrap();
    let ctx = career::NativeContext::get().unwrap();
    let a = NativeSave::parse(&std::fs::read(std::env::var("NPF2_A").unwrap()).unwrap()).unwrap();
    let b = NativeSave::parse(&std::fs::read(std::env::var("NPF2_B").unwrap()).unwrap()).unwrap();
    let p = Profile::from_native(&b, ctx);
    let (slot, car) = (p.slot, p.car.clone());
    println!("slot {slot} car {car}");
    let (oa, ob) = (a.slot(slot), b.slot(slot));
    for k in SLOT_EXTRA..SLOT_LEN {
        if a.payload[oa + k] != b.payload[ob + k] {
            println!("extra+{:#x}: {:02x} -> {:02x}", k - SLOT_EXTRA, a.payload[oa + k], b.payload[ob + k]);
        }
    }
    let db = &game.game_data.parts;
    let pa = Profile::from_native(&a, ctx);
    let bits = ctx.owned_bits(&car);
    for (s, h) in &p.installed {
        if pa.installed.get(s) != Some(h) {
            let bit: Vec<_> = bits.iter().filter(|b| b.0 == *s && b.1 == *h).map(|b| (b.2, b.3)).collect();
            let part = db.slot_parts(&car, *s as usize).into_iter().find(|q| q.hash == *h);
            println!("installed slot {s:#04x} {h:08X} group {:?} lvl {:?} clone bit {bit:x?}", part.map(|q| q.group), part.map(|q| q.level));
        }
    }
    for &(s, hash, at, mask) in ctx.owned_bits(&car).iter() {
        let e = ob + SLOT_EXTRA + at;
        if b.payload[e] & mask != 0 && a.payload[oa + SLOT_EXTRA + at] & mask == 0 {
            let part = db.slot_parts(&car, s as usize).into_iter().find(|q| q.hash == hash);
            let name = part.map(|q| db.model_names(q).join("|")).unwrap_or_default();
            println!("  now owned: slot {s:#04x} {hash:08X} {name} lvl {:?} bit at {at:#x} mask {mask:#04x}", part.map(|q| q.level));
        }
    }
}

/// The original's career at stage 2 (owned bits of a part group): 0x503D50
/// sets bit `n & 31` of the set 0x503B10 picks, n the part's position
/// among its slot's parts (0x61BA90); a part of a shared group takes car
/// index 0x2E (0x610140 finds no car type named after the group), so
/// every spoiler of the 206's shared group 0xE8E9C8A4 counts the slot's
/// default groups to the same n = 80, and 0x503DB0 reports them all owned
/// once one is bought. Under Wine (the original's stage-2 save
/// after the body shop, `stage2_body.sav`): the garage's Body Parts
/// Storage lists JORGEN, ROCKER 5, AIRFORCE, ARROW, BARRACUDA, WILDFIRE,
/// RIPPER, …, Carbon Fiber is open and lists the carbon JORGEN. The
/// clone: buying JORGEN in the body shop on the save before
/// (`stage2_url1.sav`) owns the whole group at once (no price, Carbon
/// Fiber open at the next build of the strip), and after the stage's
/// purchases the saved owned bits equal the original's byte for byte.
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn a_bought_spoiler_owns_its_whole_bit() {
    use nfsu2::formats::native_save::{NativeSave, OWNED_REMAP, SLOT_EXTRA};
    testkit::game();
    let mut game = Game::new().unwrap();
    let ctx = career::NativeContext::get().unwrap();
    let before = NativeSave::parse(&testkit::private("tests/fixtures/native_saves/stage2_url1.sav"))
    .unwrap();
    let want = NativeSave::parse(&testkit::private("tests/fixtures/native_saves/stage2_body.sav"))
    .unwrap();
    let mut p = Profile::from_native(&before, ctx);
    assert_eq!(p.car, "PEUGOT");
    p.cash = 100_000;
    p.build_shop_ride();
    // Driven into the shop's trigger (0x512FF0).
    let shop = game.game_data.career.shops.iter().find(|s| s.name == "UC_BODYSHOP").unwrap();
    p.visit_shop(shop);
    game.shop.visit = Some(shop.hash);
    game.profile = Some(p);
    const JORGEN: u32 = 0xD8CB_2118;
    let category = screens::BODY_SHOP_CATEGORIES.iter().position(|c| c.0 == 0x0C).unwrap();
    let carbon = screens::BODY_SHOP_CATEGORIES.iter().position(|c| c.0 == 0x5E).unwrap();
    let browser = |game: &Game, i: Option<usize>| match game.parts_browser(category, i) {
        Mode::Menu(Menu::Parts(b)) => b,
        _ => panic!("the body shop"),
    };
    let Mode::Menu(Menu::Parts(b)) = game.new_parts_browser(category) else { panic!("the body shop") };
    assert!(b.categories[carbon].locked, "before: Carbon Fiber locked (Wine, stage 2)");
    let list = &b.categories[category].parts;
    let i = list.iter().position(|x| x.0 == JORGEN).unwrap_or_else(|| panic!("JORGEN listed: {:08X?}", list.iter().map(|x| x.0).collect::<Vec<_>>()));
    let others: Vec<u32> = list.iter().filter(|x| x.0 != JORGEN && !x.3).map(|x| x.0).collect();
    assert!(!others.is_empty(), "other spoilers on sale");
    game.mode = game.parts_browser(category, Some(i));
    game.menu_key(Key::Accept).unwrap();
    if matches!(game.mode, Mode::Menu(Menu::Dialog(..))) {
        game.menu_key(Key::Left).unwrap();
        game.menu_key(Key::Accept).unwrap();
    }
    let p = game.profile.as_ref().unwrap();
    assert_eq!(p.installed.get(&0x0C), Some(&JORGEN), "bought and fitted");
    // The same visit: every spoiler of the group shows as owned (no price
    // to pay, re-fitting is free).
    let b = browser(&game, None);
    for h in &others {
        let x = b.categories[category].parts.iter().find(|x| x.0 == *h).unwrap();
        assert!(x.3, "{h:08X}: owned with JORGEN (one bit, 0x503DB0)");
    }
    let bit = ctx.owned_bits("PEUGOT").iter().find(|b| b.0 == 0x0C && b.1 == JORGEN).map(|b| (b.2, b.3)).unwrap();
    assert_eq!(bit, (0x286, 1), "n = 80 of slot 0xC's set (X+0x27C + 8, bit 16)");
    let carbon_spoiler = game
        .game_data.parts
        .slot_parts("PEUGOT", 0x0C)
        .into_iter()
        .find(|q| q.level > 0 && game.game_data.parts.attribute(q, 0x721A_FF7C).is_some_and(|v| v != 0))
        .map(|q| q.hash)
        .unwrap();
    assert!(p.owned.contains(&(0x0C, carbon_spoiler)), "the carbon spoilers too");
    // The strip is latched in this visit; built again, Carbon
    // Fiber is open (as in the original's garage after the load).
    assert!(browser(&game, None).categories[carbon].locked, "same visit: still locked");
    let Mode::Menu(Menu::Parts(b)) = game.new_parts_browser(category) else { panic!() };
    assert!(!b.categories[carbon].locked, "next build: open");

    // The stage-2 visit's other purchases (the original's record parts in
    // stage2_body.sav, and the hood it bought first), then the owned bits
    // as saved: equal to the original's.
    let p = game.profile.as_mut().unwrap();
    for (slot, hash) in [
        (0x01, 0xDEE8_924D),
        (0x02, 0xAF47_806E),
        (0x0B, 0x7D71_43E7),
        (0x09, 0x47A4_354E),
        (0x09, 0x4C4E_8690),
        (0x0E, 0x1569_E30D),
        (0x0F, 0x9F73_84A0),
        (0x10, 0xB5BD_B93C),
        (0x1D, 0x7081_470B),
        (0x1E, 0x7081_470B),
    ] {
        p.own_part(slot, hash);
    }
    let got = p.to_native(&before, ctx);
    let (o, w) = (got.slot(p.slot) + SLOT_EXTRA, want.slot(p.slot) + SLOT_EXTRA);
    let mut diff = Vec::new();
    for (m, len, f) in OWNED_REMAP {
        // The bit sets only (not the perf bits, the spend, the brands).
        if matches!(m, 0x15C | 0x3BC | 0x2AC) {
            continue;
        }
        diff.extend((f..f + len).filter(|&k| got.payload[o + k] != want.payload[w + k]).map(|k| {
            format!("+{k:#x}: {:02x} want {:02x}", got.payload[o + k], want.payload[w + k])
        }));
    }
    assert!(diff.is_empty(), "owned bits differ from the original's: {diff:?}");
}

/// Stages 3 and 4: UI_PerformanceDyno_Chart's
/// constructor (0x557A30) starts a pull (0x557B42 → 0x56D350) when the
/// push's argument (+0x10) is 0. The mode select's first accept pushes it
/// with 0 (0x55A9F7 in 0x55A750, 0x83A63C was 0xFF), whatever the car
/// can tune; only the results' Back (0x53D5C9 / 0x53D623) pushes 1, no
/// pull. Under Wine the Supra with Suspension tuning pulled (stage 4,
/// twice) and so did the 206 with nothing tunable (stage 3's tu2.jpg: the
/// sweep at 1,000 rpm, empty curves; its state log began after the
/// 22 s pull).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn first_dyno_chart_pulls_with_or_without_tuning() {
    testkit::game();
    for tunable in [false, true] {
        let mut game = Game::new().unwrap();
        let mut p = Profile::new("Lena");
        p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
        if tunable {
            // A performance part that opens a tuning parameter (0x34A1A).
            let part = game.game_data.career.tuning.iter().map(|u| u.part).min().unwrap();
            p.perf.insert(part);
        }
        game.profile = Some(p);
        assert_eq!(game.dyno.tuning_mode, 0xFF, "no mode chosen yet");
        game.act(Action::TuneMode(0), MenuKind::TuneMode).unwrap();
        // "Tune settings for this race mode?" (No focused): Yes.
        assert!(matches!(game.mode, Mode::Menu(Menu::Dialog(..))), "the question");
        game.menu_key(Key::Left).unwrap();
        game.menu_key(Key::Accept).unwrap();
        let Mode::Menu(Menu::DynoChart(c)) = &game.mode else { panic!("tunable {tunable}: the chart first") };
        assert!(c.pull.is_some(), "tunable {tunable}: pushed with 0, a pull");
        game.menu_key(Key::Accept).unwrap();
        assert!(matches!(game.mode, Mode::Menu(Menu::DynoResults(_))), "results");
        game.menu_key(Key::Back).unwrap();
        let Mode::Menu(Menu::DynoChart(c)) = &game.mode else { panic!("back to the chart") };
        assert!(c.pull.is_none(), "the results' Back pushes 1: no pull");
    }
}

/// Diagnostic (the original's career at stage 3, the rivals' start stretch): the rivals'
/// positions of a career event's race (`NPF2_EVENT` = its name) from the
/// grid, every 0.15 s of race clock for `NPF2_SECS` (default 45), as
/// "t idx x y z v" lines to `NPF2_TRAIL`, the player's car held at the
/// grid (as the stage-3 recording: the original's player stood still).
#[test]
#[ignore = "diagnostic: records the rivals' trails of an event's race"]
fn record_rival_trails() {
    testkit::game();
    let name = std::env::var("NPF2_EVENT").unwrap();
    let out = std::env::var("NPF2_TRAIL").unwrap();
    let secs: f32 = std::env::var("NPF2_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(45.0);
    let mut game = Game::new().unwrap();
    let event = game.game_data.career.events.iter().position(|e| e.name == name).unwrap();
    game.game_data.career.events[event].traffic = 0;
    // NPF2_TRACK: the same event on another track (forward, 3 laps), for
    // the original's Quick Race recordings.
    if let Some(track) = std::env::var("NPF2_TRACK").ok().and_then(|t| t.parse().ok()) {
        let e = &mut game.game_data.career.events[event];
        (e.track, e.variant, e.laps) = (track, 0, 3);
        e.races.clear();
    }
    game.start_race(event, true, false).unwrap();
    while game.scenes.shop.is_some() && matches!(game.mode, Mode::Race(_)) {
        game.advance_for(1.0 / 60.0).unwrap();
    }
    let mut text = String::new();
    let mut t = 0.0f32;
    let mut next = 0.0f32;
    while t < secs {
        let Mode::Race(race) = &mut game.mode else { break };
        race.racers.traffic = None;
        if t >= next {
            for (i, r) in race.racers.rivals.iter().enumerate() {
                let p = r.state.position;
                text += &format!("{t:.2} {} {:.1} {:.1} {:.1} {:.1}\n", i + 1, p.x, p.y, p.z, r.state.velocity.truncate().length());
            }
            next += 0.15;
        }
        game.advance_for(1.0 / 60.0).unwrap();
        t += 1.0 / 60.0;
    }
    std::fs::write(out, text).unwrap();
}

/// Diagnostic: the tracks (and direction) whose first grid slots lie
/// within 4 m of `NPF2_GRID` = "x,y" (to name a race recorded in the
/// original by its rivals' start positions).
#[test]
#[ignore = "diagnostic: finds a track by a grid position"]
fn find_track_by_grid() {
    testkit::game();
    let game = Game::new().unwrap();
    let g: Vec<f32> = std::env::var("NPF2_GRID").unwrap().split(',').map(|v| v.parse().unwrap()).collect();
    let p = glam::Vec2::new(g[0], g[1]);
    let mut tracks: Vec<_> = game.game_data.career.tracks.values().collect();
    tracks.sort_by_key(|t| t.number);
    for t in tracks {
        for back in [false, true] {
            let Ok(r) = career::load_route(&game.game_data.root, t, back) else { continue };
            if r.grid.iter().any(|s| s.0.truncate().distance(p) < 4.0) {
                println!("track {} backward {back} loops {} length {:.0}", t.number, r.loops, r.road.length);
            }
        }
    }
}

/// The part map made ahead (`Game::preload_owned_bits`): when the
/// profile's cars change (a car taken into a slot, a new career's), the
/// owned-part bits of a car whose map was never made are made on a thread,
/// and they are the map a save would make in place (`owned_bits_fresh`),
/// so the save's bytes are the same; with nothing new no thread is started.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_new_cars_part_map_is_made_ahead() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let ctx = career::NativeContext::get().unwrap();
    // Car types whose map no other test of this process has made yet.
    let mut fresh: Vec<String> = ctx.cars.values().filter(|c| !ctx.owned_bits_ready(c)).cloned().collect();
    fresh.sort();
    assert!(fresh.len() >= 2, "car types with no part map made yet: {fresh:?}");
    let mut p = Profile::new("PART_MAP_AHEAD");
    p.skip_intro(&game.game_data.career, &game.game_data.root, &fresh[0], &mut game.rng);
    game.profile = Some(p);
    let wait = |game: &mut Game| -> bool {
        let job = game.loads.owned_bits.as_mut().and_then(|(_, h)| h.take());
        job.map(|h| h.join().unwrap()).is_some()
    };
    game.preload_owned_bits();
    assert!(wait(&mut game), "a thread for the new career's car");
    // A second car taken into another slot.
    let p = game.profile.as_mut().unwrap();
    let slot = p.free_slot().unwrap_or(1);
    p.take_stock_car(slot, &fresh[1]);
    game.preload_owned_bits();
    assert!(wait(&mut game), "a thread for the car taken");
    let cars = game.profile.as_ref().unwrap().slot_cars();
    for car in &cars {
        assert!(ctx.owned_bits_ready(car), "{car}: its part map made ahead");
        assert_eq!(&ctx.owned_bits(car)[..], &ctx.owned_bits_fresh(car)[..], "{car}: the map a save makes in place");
    }
    // The same cars again, or cars with their maps made: no thread.
    game.preload_owned_bits();
    assert!(game.loads.owned_bits.as_ref().is_some_and(|(c, h)| *c == cars && h.is_none()), "nothing new, no thread");
    game.loads.owned_bits = None;
    game.preload_owned_bits();
    assert!(!wait(&mut game), "every map made: no thread");
    // What a car's map costs the thread that makes it (the save's main
    // thread before, at a race's end).
    let t = nfsu2::prof::thread_cpu_ns();
    let _ = ctx.owned_bits_fresh(&fresh[0]);
    eprintln!("part map of {}: {:.2} ms of one thread", fresh[0], (nfsu2::prof::thread_cpu_ns() - t) as f64 / 1e6);
}
