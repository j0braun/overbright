//! The front end and career data against the installed game.

mod common;

use nfsu2::frontend::Assets;
use nfsu2::frontend::career::{self, CareerData, Category, EventKind, Profile};
use nfsu2::frontend::feng::Kind;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn packages_fonts_and_strings_load() {
    let root = common::game_root();
    let assets = Assets::load(std::path::Path::new(&root)).unwrap();
    assert!(assets.packages.len() >= 150, "{} packages", assets.packages.len());
    let main = assets.package("UI_Main").expect("UI_Main");
    assert_eq!(main.name, "UI_Main.fng");
    // The title string, its label and the selected-icon slot.
    let title = main.object(0x42AD_B44C).expect("title");
    assert_eq!(title.kind, Kind::String);
    assert_eq!(assets.strings.get(title.label.unwrap()), Some("Main Menu"));
    assert!(main.object(0xB046_69E3).is_some());
    // The menu icons and the fonts the packages ask for.
    for icon in [0x0370_4F3D, 0x4E6F_BB02, 0x3CB9_2E26, 0x22F2_CEAB] {
        assert!(assets.textures.contains_key(&icon), "icon {icon:08X}");
    }
    for font in ["ConduitMdITC_TT21i.ffn", "ConduitMdITC_TT14i.ffn", "font_arial.ffn"] {
        let f = assets.font_for(font).unwrap();
        assert!(f.glyphs.contains_key(&('a' as u32)), "{font}");
    }
    assert_eq!(assets.strings.get(nfsu2::formats::hash::bin_hash("CAR_NAME_PEUGOT")), Some("206"));
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn career_events_and_routes() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let data = CareerData::load(root).unwrap();
    assert_eq!(data.events.len(), 224);
    let stage1 = data.stage_events(root, 1);
    assert_eq!(stage1.len(), 8);
    let circuit = stage1.iter().find(|e| e.track == 4013).unwrap();
    assert_eq!(circuit.kind, EventKind::Circuit);
    assert_eq!((circuit.laps, circuit.reward), (3, 250));
    let track = &data.tracks[&4013];
    assert_eq!(track.title(), "Short Circuit #3");
    let route = career::load_route(root, track, false).unwrap();
    assert!(route.loops);
    assert_eq!(route.checkpoints.len(), 7);
    assert_eq!(route.checkpoints.first(), route.checkpoints.last());
    assert_eq!(route.grid.len(), 8);
    assert!(route.start_line.is_some());
    // Every stage's events load a route.
    for stage in 1..=5 {
        for e in data.stage_events(root, stage) {
            let t = &data.tracks[&e.track];
            career::load_route(root, t, e.variant == 1).unwrap_or_else(|err| panic!("{}: {err}", t.name));
        }
    }
    assert!(career::free_roam_start(root).is_some());
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn open_events_follow_the_stage_rules() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let data = CareerData::load(root).unwrap();
    let mut p = Profile::new("Test");
    p.skip_intro(&data, root, "PEUGOT", &mut nfsu2::opponents::GameRng::default());
    p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
    // Stage 1 opens one race of each type (stage def caps 1).
    let slots: Vec<_> = p.open_events(&data).iter().map(|e| e.slot()).collect();
    assert_eq!(slots.len(), 5);
    for slot in 0..5 {
        assert!(slots.contains(&Some(slot)), "slot {slot}");
    }
    while p.stage == 1 {
        let e = p.open_events(&data)[0].clone();
        p.win(&data, root, &e, 60.0, (0, 0), &mut nfsu2::opponents::GameRng::default());
    }
    assert_eq!(p.stage, 2);
    // No sponsor races before signing; the sponsor's three after.
    assert!(p.open_events(&data).iter().all(|e| e.category != Category::Sponsor));
    let streetglow = data.sponsors.values().find(|s| s.name == "STREETGLOW").unwrap();
    p.sign(streetglow);
    p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
    let sponsor: Vec<_> = p.open_events(&data).iter().filter(|e| e.category == Category::Sponsor).map(|e| e.name.clone()).collect();
    assert_eq!(sponsor.len(), 3, "{sponsor:?}");
    assert!(sponsor.iter().all(|n| n.starts_with("S2_SPON_CIRCUIT")));
    // The first URL race opens after one normal win.
    let url = |p: &Profile| p.open_events(&data).iter().filter(|e| e.category == Category::Url).map(|e| e.name.clone()).collect::<Vec<_>>();
    assert!(url(&p).is_empty());
    let normal = p.open_events(&data).iter().find(|e| e.category == Category::Normal).map(|e| (*e).clone()).unwrap();
    // The win handler (0x541430) leaves the won event open until the
    // city start rebuilds the list (0x5412E0 → 0x527D40).
    p.win_paid(&data, root, &normal, 60.0, None);
    assert!(p.open.contains(&normal.unlock) && url(&p).is_empty());
    p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
    assert!(!p.open.contains(&normal.unlock));
    assert_eq!(url(&p).len(), 1);
    // Win everything open: the last URL race stays shut without the DVD.
    for _ in 0..60 {
        let Some(e) = p.open_events(&data).first().map(|e| (*e).clone()) else { break };
        p.win(&data, root, &e, 60.0, (0, 0), &mut nfsu2::opponents::GameRng::default());
    }
    assert_eq!(p.stage, 2);
    assert!(p.open.is_empty());
    let dvd = data.milestones.iter().find(|m| m.name == "STAGE_2_DVD_1").unwrap();
    p.milestones.insert(dvd.hash);
    p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
    assert_eq!(url(&p), ["S2_URL_3"]);
    let last = p.open_events(&data)[0].clone();
    p.win(&data, root, &last, 60.0, (0, 0), &mut nfsu2::opponents::GameRng::default());
    assert_eq!(p.stage, 3);
}

/// The career audit's rules (2026-10-31): stage 1 ends when every stage-1
/// event is won (0x5130A0), not after five wins counting stage-0 races; a
/// win's reputation goes to the won event's stage record (0x541430 →
/// 0x513430); a sponsor race pays the signed sponsor's +0x02 (0x532E60);
/// the scripted category earns no points (0x512AE0).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn career_audit_rules() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let data = CareerData::load(root).unwrap();
    let mut p = Profile::new("Test");
    p.skip_intro(&data, root, "PEUGOT", &mut nfsu2::opponents::GameRng::default());
    assert_eq!(p.stage, 1);
    let stage0: Vec<_> = data.events.iter().filter(|e| e.stage == 0 && e.category == Category::Normal).cloned().collect();
    let stage1: Vec<_> = data.events.iter().filter(|e| e.stage == 1).cloned().collect();
    assert_eq!((stage0.len(), stage1.len()), (3, 5));
    for e in &stage0 {
        p.win(&data, root, e, 60.0, (100, 0), &mut nfsu2::opponents::GameRng::default());
    }
    for e in &stage1[..4] {
        p.win(&data, root, e, 60.0, (300, 250), &mut nfsu2::opponents::GameRng::default());
    }
    assert_eq!(p.stage, 1, "seven wins, one stage-1 event left");
    assert_eq!(p.stage_rep.get(&0), Some(&(300.0, 3)));
    assert_eq!(p.stage_rep.get(&1), Some(&(1200.0, 4)));
    p.win(&data, root, &stage1[4], 60.0, (0, 0), &mut nfsu2::opponents::GameRng::default());
    assert_eq!(p.stage, 2);
    let spon = data.events.iter().find(|e| e.name == "S2_SPON_CIRCUIT_1").unwrap();
    assert_eq!(p.race_bank(&data, spon), 1000);
    let streetglow = data.sponsors.values().find(|s| s.name == "STREETGLOW").unwrap();
    p.sign(streetglow);
    assert_eq!(p.race_bank(&data, spon), 900);
    let plain = data.events.iter().find(|e| e.name == "S2_CIRCUIT_2").unwrap();
    assert_eq!(p.race_bank(&data, plain), 300);
    assert_eq!(nfsu2::frontend::career::race_reward(plain, 66.0, 300), (300, 300));
    let dday = data.events.iter().find(|e| e.unlock == nfsu2::frontend::career::DDAY_EVENT_A).unwrap();
    assert_eq!(nfsu2::frontend::career::reputation_points(dday, 66.0), 0);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn visual_rating_from_parts() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let data = CareerData::load(root).unwrap();
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let db = nfsu2::formats::parts::PartDb::load(&globalb).unwrap();
    let types = nfsu2::car::parse_car_types(&globalb).unwrap();
    let paint = types.iter().find(|t| t.name == "240SX").unwrap().stock_paint;
    let stock = |slot: usize| if slot == 0x3F { db.find("240SX", slot, Some(paint), None).copied() } else { db.stock("240SX", slot).copied() };
    // A stock car rates 0 (stock parts are level 0; the stock paint is free).
    assert_eq!(db.visual_rating(&data.costs, paint, &stock), 0.0);
    // A level-1 front bumper adds its category's points (0x12: the bumper).
    let bumper = *db.find("240SX", 1, None, Some(1)).unwrap();
    assert_eq!(db.category(1, &bumper), 0x12);
    let points = data.costs.points(0x12, 1);
    assert!(points > 0, "{points}");
    let with = |slot: usize| if slot == 1 { Some(bumper) } else { stock(slot) };
    assert_eq!(db.visual_rating(&data.costs, paint, &with), nfsu2::formats::parts::rating_from_points(points));
    assert!((nfsu2::formats::parts::rating_from_points(points) - points as f32 / 500.0).abs() < 1e-6);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn parts_reach_the_dvd_cover() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let data = CareerData::load(root).unwrap();
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let db = nfsu2::formats::parts::PartDb::load(&globalb).unwrap();
    let paint = nfsu2::car::parse_car_types(&globalb).unwrap().into_iter().find(|t| t.name == "240SX").unwrap().stock_paint;
    let mut p = Profile::new("Test");
    p.car = "240SX".into();
    p.stage = 2;
    // Every shop found (0x512FF0): parts a shop unlocks need it open
    // (0x515360 → 0x513110), and most shops wait to be found.
    for s in &data.shops {
        p.visit_shop(s);
    }
    // The best part of each body category the shop sells.
    for &(slot, _, _) in nfsu2::frontend::screens::BODY_SHOP_CATEGORIES.iter() {
        let best = p.shop_parts(&data, &db, slot as usize, false).into_iter().max_by_key(|part| part.level).copied();
        if let Some(best) = best.filter(|b| b.level > 0) {
            p.installed.insert(slot, best.hash);
        }
    }
    let fitted = |slot: usize| match p.installed.get(&(slot as u8)) {
        Some(&h) => db.find("240SX", slot, Some(h), None).copied(),
        None if slot == 0x3F => db.find("240SX", slot, Some(paint), None).copied(),
        None => db.stock("240SX", slot).copied(),
    };
    let rating = db.visual_rating(&data.costs, paint, &fitted);
    assert!(rating >= 1.0, "rating {rating}");
    let dvd = data.milestones.iter().find(|m| m.name == "STAGE_2_DVD_1").unwrap().clone();
    p.offer_covers(&data, rating);
    assert!(p.offered.contains(&dvd.hash));
    // Its photo shoot waits at a route trigger in Bayview.
    let spot = career::trigger_spot(root, dvd.spot).expect("photo spot");
    assert!((spot.x - -301.0).abs() < 1.0 && (spot.y - -554.0).abs() < 1.0, "{spot:?}");
    p.shoot(&data, dvd.hash, &mut nfsu2::opponents::GameRng::default());
    assert!(p.milestones.contains(&dvd.hash) && p.offered.is_empty());
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn customize_lists_parts_by_the_quick_race_rule() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let data = CareerData::load(root).unwrap();
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let db = nfsu2::formats::parts::PartDb::load(&globalb).unwrap();
    // Stage record 5 (0x86262C) is the last stage, with a final event.
    let last = data.stages.get(5).expect("stage 5");
    assert!(last.final_event != 0);
    let mut career = Profile::new("Test");
    career.car = "240SX".into();
    let mut scratch = Profile::new("Test");
    scratch.car = "240SX".into();
    scratch.scratch = true;
    // On a fresh career both list the same front bumpers (the cost table's
    // unlock, 0x515360), and Customize lists no level-4 part.
    let levels = |p: &Profile| p.shop_parts(&data, &db, 1, false).iter().map(|x| x.level).collect::<Vec<_>>();
    assert_eq!(levels(&career), levels(&scratch));
    assert!(!scratch.part_listed(&data, 0x12, 4, 0));
    // Once the last stage's final event is won, Customize lists level-4
    // parts (0x529D70); the career shop still never does (0x515360).
    let event = data.events.iter().find(|e| e.unlock == last.final_event).expect("final event");
    career.completed.insert(event.index);
    scratch.completed.insert(event.index);
    assert!(scratch.part_listed(&data, 0x12, 4, 0));
    assert!(!career.part_listed(&data, 0x12, 4, 0));
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn triggers_stand_for_shops_and_events() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let data = CareerData::load(root).unwrap();
    // Shops are world triggers named by their +0x3C (the world's event
    // system), but the car engages them by the route trigger of that name
    // (0x5EB050 → 0x527810, as 0x533320 lists them): the body shop's is
    // a 20 m box 14 m from the world trigger.
    let world = career::world_triggers(root, "L4RA");
    let shop = data.shops.iter().find(|s| s.name == "CC_BODYSHOP_1").unwrap();
    let t = world.get(&shop.trigger).expect("body shop trigger");
    assert!(t.contains(glam::Vec2::new(-385.0, 50.0)), "{t:?}");
    let open = data.shops.iter().filter(|s| world.contains_key(&s.trigger)).count();
    assert!(open >= 20, "{open} shops placed");
    let r = career::RouteTriggers::load(root).find(shop.trigger).expect("body shop route trigger");
    assert!(r.contains(glam::Vec2::new(-385.9, 38.7)) && !r.contains(glam::Vec2::new(-381.1, 53.3)), "{r:?}");
    // Events: route triggers named by their markers.
    let route = career::RouteTriggers::load(root);
    let ev = route.find(nfsu2::formats::hash::bin_hash("EV_01")).expect("EV_01");
    assert!(ev.contains(glam::Vec2::new(374.0, 1167.0)) && !ev.contains(glam::Vec2::new(390.0, 1167.0)));
    assert_eq!((ev.points, ev.position.z.round()), (4, 35.0));
    // The shops' names are route triggers too (kind 14, what 0x533320
    // matches against their +0x3C): 23 of the 24 world triggers.
    assert!(route.find(shop.trigger).is_some());
    assert!(world.keys().filter(|&&n| route.find(n).is_some()).count() + 1 >= world.len());
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn sponsors_are_offered_by_wins_and_average_points() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let data = CareerData::load(root).unwrap();
    let mut p = Profile::new("Test");
    p.car = "PEUGOT".into();
    p.stage = 2;
    // No win in the stage yet: nothing on offer.
    assert!(p.sponsor_offers(&data).is_empty());
    let race = data.events.iter().find(|e| e.stage == 2 && e.category == Category::Normal && !e.suv).unwrap().clone();
    // Stage 1 at 100 points a win (the average is the previous stage's
    // record, 0x5005E0 on 0x861F14 + 0x104 × stage) and one stage-2 win:
    // the sponsors asking 0 and 75 a win.
    p.stage_rep.insert(1, (500.0, 5));
    p.win(&data, root, &race, 60.0, (300, 300), &mut nfsu2::opponents::GameRng::default());
    let offered: Vec<i16> = p.sponsor_offers(&data).iter().map(|s| s.reputation).collect();
    assert_eq!(offered, [0, 75]);
    assert!(p.needs_sponsor(&data));
    p.sign(p.sponsor_offers(&data)[1]);
    assert!(p.sponsor_offers(&data).is_empty());
}

/// The race route through the road network (0x5E5BC0) comes out the
/// length the track records (+0x98): a circuit's lap is the whole road, a
/// sprint or drag runs from the start line to the finish line.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn race_roads_match_the_track_lengths() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let data = CareerData::load(root).unwrap();
    for (number, close) in [(4013, 5.0), (4014, 5.0), (4087, 5.0), (4101, 10.0), (4106, 10.0), (4213, 5.0), (4709, 5.0)] {
        let track = &data.tracks[&number];
        let route = career::load_route(root, track, false).unwrap();
        let run = if route.loops { route.road.length } else { route.finish_at - route.start_at };
        assert!((run - track.length as f32).abs() < close, "{number}: {run} against {}", track.length);
    }
}

/// An event's opponents (+0x40, +0x7C of them): S3_SPRINT_6 races Wayne,
/// Darren and Liz in random cars (bin_hash "RANDOM").
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn events_list_their_opponents() {
    let root = common::game_root();
    let data = CareerData::load(std::path::Path::new(&root)).unwrap();
    let e = data.events.iter().find(|e| e.name == "S3_SPRINT_6").unwrap();
    let names: Vec<&str> = e.opponents.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["Wayne", "Darren", "Liz"]);
    assert!(e.opponents.iter().all(|o| o.car == nfsu2::formats::hash::bin_hash("RANDOM")));
}

/// TIME BEHIND's signed times (0x577250 flag 2 → 0x576D40): the sign takes
/// a one-digit field's blank, or goes in front of two digits and the last
/// character drops.
#[test]
fn signed_times_keep_their_width() {
    use nfsu2::frontend::screens::signed_time;
    assert_eq!(signed_time(20920), "+5.23");
    assert_eq!(signed_time(49360), "+12.3");
    assert_eq!(signed_time(-260000), "-1:05.00");
}

/// The shops' stat ranges (0x34A32 → 0x86B3D0) and ratings (0x5B53B0,
/// 0x5B6A30, 0x5B54A0): every car rates within 0.03–1, and a car with
/// every level-3 package fitted rates higher on acceleration and top speed
/// and makes more torque and power (0x5B5820 / 0x5B56F0).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn performance_ratings_follow_the_upgrades() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let data = CareerData::load(root).unwrap();
    let r = &data.stat_ranges;
    assert!((r[4] - 0.129_925_39).abs() < 1e-6 && (r[5] - 0.72).abs() < 1e-6, "torque range {:?}", &r[4..6]);
    assert!((r[6] - 83.612_18).abs() < 1e-3 && (r[7] - 411.401_06).abs() < 1e-3, "power range {:?}", &r[6..8]);
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let records = nfsu2::car::car_type_records(&globalb).unwrap();
    let full: std::collections::BTreeSet<u8> = data.packages.iter().filter(|p| p.level == 3).flat_map(|p| p.parts.iter().map(|x| x.id)).collect();
    for name in ["PEUGOT", "240SX", "SKYLINE", "GTO"] {
        let (_, stock) = records.iter().find(|(n, _)| n == name).unwrap();
        let mut stock = stock.clone();
        let mut upgraded = stock.clone();
        nfsu2::performance::upgrade(&mut stock, &data.packages, &|_| false);
        nfsu2::performance::upgrade(&mut upgraded, &data.packages, &|id| full.contains(&id));
        let (a, b) = (nfsu2::performance::ratings(&stock, r), nfsu2::performance::ratings(&upgraded, r));
        for v in [a.acceleration, a.top_speed, a.handling, b.acceleration, b.top_speed, b.handling] {
            assert!((0.03..=1.0).contains(&v), "{name}: {a:?} {b:?}");
        }
        assert!(b.acceleration > a.acceleration && b.top_speed > a.top_speed, "{name}: {a:?} → {b:?}");
        let (t0, p0) = nfsu2::performance::peaks(&stock);
        let (t1, p1) = nfsu2::performance::peaks(&upgraded);
        assert!(t1 > t0 && p1 > p0, "{name}: torque {t0} → {t1}, power {p0} → {p1}");
    }
}

/// A performance part's +0x40 five stand-ins come off when it goes on
/// (0x5A6330); the Street / Pro / Extreme nitrous kits replace each other.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn performance_parts_replace_their_stand_ins() {
    let root = common::game_root();
    let data = CareerData::load(std::path::Path::new(&root)).unwrap();
    let kit = |level: u8| data.packages.iter().find(|p| p.category == 2 && p.level == level).unwrap().parts[0].id;
    let (street, pro, extreme) = (kit(1), kit(2), kit(3));
    assert!(data.packages.iter().flat_map(|p| &p.parts).all(|x| x.requires.is_empty()), "no part needs another fitted");
    let mut fitted = std::collections::BTreeSet::new();
    assert!(nfsu2::performance::install(&mut fitted, &data.packages, street));
    assert!(!nfsu2::performance::install(&mut fitted, &data.packages, street), "already fitted");
    assert!(nfsu2::performance::install(&mut fitted, &data.packages, extreme));
    assert_eq!(fitted.iter().copied().collect::<Vec<_>>(), [extreme]);
    assert!(nfsu2::performance::install(&mut fitted, &data.packages, pro));
    assert_eq!(fitted.iter().copied().collect::<Vec<_>>(), [pro]);
}

/// `%$2.2f` (UI_BuyPerformanceParts' torque and power gains).
#[test]
fn fixed_point_gains_group_thousands() {
    use nfsu2::frontend::screens::fixed2;
    assert_eq!(fixed2(2.834), "2.83");
    assert_eq!(fixed2(-0.5), "-0.50");
    assert_eq!(fixed2(1234.5), "1,234.50");
    assert_eq!(fixed2(0.0), "0.00");
}

/// A new career is the D-Day intro (0x495A30): the car lot's goal, a stock
/// car into the first slot, the garage's goal, then stage 1. The lot
/// offers the stock cars without unlock events (0x513C50), not CIVIC or
/// RSX (0x5022C0); a sponsor opens the next slot (0x532D00).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_dday_intro_leads_to_the_first_car() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let data = CareerData::load(root).unwrap();
    let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
    let types = nfsu2::car::parse_car_types(&globalb).unwrap();
    let mut p = Profile::new("Test");
    assert_eq!(p.stage, 0);
    p.start_career(&data);
    assert_eq!(p.intro, Some(career::DDAY_EVENT_A));
    let lot: Vec<&str> = p.lot_cars(&data, &types).iter().map(|&i| types[i].name.as_str()).collect();
    assert_eq!(lot, ["PEUGOT", "FOCUS", "COROLLA", "240SX", "PEUGOT106", "CORSA"]);
    // Rachel's car: the DDAY_PLAYER_CAR preset, a 350Z at performance level 2.
    let preset = nfsu2::car::parse_presets(&globalb).into_iter().find(|x| x.name == career::DDAY_PLAYER_CAR).unwrap();
    assert_eq!((preset.car.as_str(), preset.perf_level), ("350Z", 2));
    // The garage is closed until the lot's goal is reached.
    let crib = data.shops.iter().find(|s| s.name == "CRIB_1").unwrap();
    assert!(!p.shop_open(&data, crib));
    assert!(!p.enter_shop(&data, root, crib.hash, &mut nfsu2::opponents::GameRng::default()));
    let lot_shop = data.shops.iter().find(|s| s.name == "CC_CAR_LOT_1").unwrap();
    assert!(p.enter_shop(&data, root, lot_shop.hash, &mut nfsu2::opponents::GameRng::default()));
    assert_eq!(p.intro, Some(career::DDAY_EVENT_B));
    assert_eq!(p.free_slot(), Some(0));
    p.take_stock_car(0, "FOCUS");
    assert!(p.shop_open(&data, crib));
    assert!(p.enter_shop(&data, root, crib.hash, &mut nfsu2::opponents::GameRng::default()));
    assert_eq!((p.intro, p.stage, p.car.as_str()), (None, 1, "FOCUS"));
    // No free slot until a sponsor is signed; then a second car.
    assert_eq!(p.free_slot(), None);
    p.stage = 2;
    let sponsor = data.sponsors.values().next().unwrap().clone();
    p.sign(&sponsor);
    assert_eq!(p.free_slot(), Some(1));
    p.take_stock_car(1, "240SX");
    assert_eq!(p.car_slots().len(), 2);
    p.select_slot(0);
    assert_eq!(p.car, "FOCUS");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn hud_position_sits_above_the_lap_row() {
    use nfsu2::formats::hash::fe_hash;
    use nfsu2::frontend::{ScreenState, layout};
    let root = common::game_root();
    let assets = Assets::load(std::path::Path::new(&root)).unwrap();
    let hud = assets.package("HUD_SingleRace").unwrap();
    // Only the position number and the lap number (and their groups).
    let mut state = ScreenState::default();
    for o in &hud.objects {
        if o.kind != Kind::Group && o.hash != fe_hash("RacePositionNumber") && o.hash != fe_hash("LapNumber") {
            state.hidden.insert(o.hash);
        }
    }
    state.text.insert(fe_hash("RacePositionNumber"), "1".into());
    state.text.insert(fe_hash("LapNumber"), "1".into());
    let quads = layout(&assets, hud, &state, 0.0);
    // RacePositionNumber is bottom aligned (Sj 0xA, 0x51C110) at
    // y = −224 + 59, the lap number top aligned at −158.25 − 3: the
    // position's glyphs end above where the lap's begin.
    let (mut pos, mut lap) = (f32::MIN, f32::MAX);
    for q in &quads {
        // The position font draws bigger glyphs than the lap's 0.6 scale.
        if q.rect[1].y - q.rect[0].y > 20.0 { pos = pos.max(q.rect[1].y) } else { lap = lap.min(q.rect[0].y) }
    }
    assert!(pos > 0.0 && lap < 480.0, "both drawn: {quads:?}");
    assert!(pos <= 240.0 - 165.0 + 0.01 && lap >= 240.0 - 161.25 - 0.01, "position ends {pos}, laps start {lap}");
}

/// The dyno results' launch times (0x5B06B0): every car type reaches 60
/// and 100 mph from a standstill, 0-60 before 0-100, in plausible times.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn launch_times_for_every_car_type() {
    let root = common::game_root();
    let globalb = std::fs::read(std::path::Path::new(&root).join("GLOBAL/GLOBALB.BUN")).unwrap();
    let types = nfsu2::car::parse_car_types(&globalb).unwrap();
    for t in types.iter().filter(|t| t.handling.type_index < 31) {
        let l = nfsu2::handling::launch_times(&t.handling, t.handling.type_index);
        println!("{} #{}: 0-60 {:.2} 0-100 {:.2} quarter {:.2} s at {:.1} m/s", t.name, t.handling.type_index, l.zero_60, l.zero_100, l.quarter, l.quarter_speed);
        if t.traffic {
            continue;
        }
        assert!(l.zero_60 > 2.0 && l.zero_60 < 15.0, "{}: {l:?}", t.name);
        assert!(l.zero_100 > l.zero_60 && l.zero_100 < 40.0, "{}: {l:?}", t.name);
        assert!(l.quarter > 8.0 && l.quarter_speed > 30.0, "{}: {l:?}", t.name);
    }
}

/// `launch_times`' early stop for a time whose type factor is 0 (a
/// type past the factor tables, as the traffic's) returns the same figures,
/// bit for bit, as the game's loop run to its end (999,999 steps), for
/// every car type at its own index and at indexes past the tables.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn launch_times_early_stop_is_exact() {
    fn reference(h: &nfsu2::handling::Handling, type_index: usize, f60: f32, f100: f32) -> [u32; 4] {
        const H: f32 = 1.0 / 60.0;
        let g = &h.gearbox;
        let split = g.rear_split;
        let final_drive = split * g.final_drive_rear + (1.0 - split) * g.final_drive_front;
        let radius = (1.0 - split) * h.wheel_radii[0] + split * h.wheel_radii[1];
        let (up, _) = nfsu2::handling::shift_points(h);
        let _ = type_index;
        let mut out = [0.0f32; 4];
        let (mut v, mut t, mut x) = (0.0f32, 0.0f32, 0.0f32);
        let (mut clutch, mut out_stroke, mut shifting) = (1.0f32, true, false);
        let mut gear = 2usize;
        for _ in 0..999_999 {
            let ratio = g.ratios.get(gear).copied().unwrap_or(0.0);
            let inverse_radius = 1.0 / radius;
            let rpm = inverse_radius * v * ratio * final_drive * 9.549_304;
            let push = if shifting || -100.0 + up.get(gear).copied().unwrap_or(0.0) > rpm {
                shifting = false;
                h.engine.torque.at(rpm) * ratio * final_drive * inverse_radius / h.mass
            } else {
                shifting = true;
                clutch += if out_stroke { -3.0 } else { 1.5 } * H;
                if clutch > 0.0 {
                    if clutch >= 1.0 {
                        shifting = false;
                        gear += 1;
                        clutch = 1.0;
                        out_stroke = true;
                    }
                } else {
                    clutch = 0.0;
                    out_stroke = false;
                }
                0.0
            };
            t += H;
            let dv = (push - v * h.drag * v / h.mass) * H;
            x += (0.5 * dv + v) * H;
            v += dv;
            if out[0] == 0.0 && v * 2.237 >= 60.0 {
                out[0] = t * f60;
            }
            if out[1] == 0.0 && v * 2.237 >= 100.0 {
                out[1] = t * f100;
            }
            if out[2] == 0.0 && x >= 400.5 {
                out[2] = t;
                out[3] = v;
            }
            if out.iter().all(|&o| o != 0.0) {
                break;
            }
        }
        out.map(f32::to_bits)
    }
    let root = common::game_root();
    let globalb = std::fs::read(std::path::Path::new(&root).join("GLOBAL/GLOBALB.BUN")).unwrap();
    let types = nfsu2::car::parse_car_types(&globalb).unwrap();
    // A type index the tables hold: the stop is the game's own (both
    // factors non-zero), the quarter figures do not depend on the factors.
    // Past the tables (31, 45, and the traffic's own): both factors 0.
    let mut checked = 0;
    for t in &types {
        let h = &t.handling;
        for index in [h.type_index, 31, 45] {
            let got = nfsu2::handling::launch_times(h, index);
            let got = [got.zero_60, got.zero_100, got.quarter, got.quarter_speed].map(f32::to_bits);
            if index < 31 {
                let want = reference(h, index, 1.0, 1.0);
                assert_eq!(got[2..], want[2..], "{} at type index {index}", t.name);
            } else {
                assert_eq!(got, reference(h, index, 0.0, 0.0), "{} at type index {index}", t.name);
            }
            checked += 1;
        }
    }
    assert!(checked > types.len(), "{checked} checked");
}

/// The dyno pull (0x56E6F0): the manager shifts up through first and
/// second at 3000 rpm, pulls in gear 4 (third) at full gas until the
/// averaged rpm reaches max − 200, then lets off.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn dyno_pull_runs_in_third_gear() {
    let root = common::game_root();
    let car = nfsu2::car::Car::load_physics(std::path::Path::new(&root), "240SX", &[], &|_| false, None).unwrap();
    let max = car.car_type.handling.engine.max_rpm;
    let mut pull = nfsu2::dyno::DynoPull::new(car);
    let (mut t, mut pulled_at) = (0.0f32, None);
    while t < 60.0 && pull.progress() < 1.0 {
        pull.step(nfsu2::dyno::FRAME);
        t += nfsu2::dyno::FRAME;
        if std::env::var_os("DYNO_TRACE").is_some() && ((t * 60.0) as i32) % 10 == 0 {
            println!("{t:.2} stage {} gear {} rpm {:.0} mean {:.0} p {:.3}", pull.stage(), pull.gear(), pull.rpm(), pull.mean_rpm(), pull.progress());
        }
        if pull.stage() == 2 && pulled_at.is_none() {
            pulled_at = Some(t);
            assert_eq!(pull.gear(), 4);
        }
    }
    println!("pull from {pulled_at:?} to {t} s, mean {} of max {max}", pull.mean_rpm());
    assert!(pulled_at.is_some_and(|p| p > 0.5 && p < 15.0), "{pulled_at:?}");
    assert_eq!(pull.progress(), 1.0, "after {t} s: rpm {}", pull.rpm());
    assert!(t < 40.0, "{t}");
    pull.step(0.5);
    assert_eq!(pull.stage(), 3);
}

/// The dyno's car moves with the pull (a reported bug: on the
/// rollers the clone's car stood still): the settled car starts with no
/// tilt or rise, the wheels spin up under the pull (Wine, the finished-career save's Lancer:
/// to 162 rad/s, the body ≤ 0.6° nose up, 3 mm up) and the rollers turn
/// back by the wheels' spin (0x56E6F0).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn dyno_pull_moves_the_car_and_rollers() {
    let root = common::game_root();
    let load = || nfsu2::car::Car::load_physics(std::path::Path::new(&root), "LANCEREVO8", &[], &|_| false, None).unwrap();
    let car = load();
    let mut pull = nfsu2::dyno::DynoPull::new(load());
    let p0 = pull.pose();
    assert!(p0.rise.abs() < 1e-4 && p0.tilt.angle_between(glam::Quat::IDENTITY) < 1e-4, "{p0:?}");
    assert!(p0.spin.iter().all(|s| s.abs() < 1e-3), "{p0:?}");
    let com = glam::Vec3::new(10.0, 20.0, 1.0);
    let mut rollers = nfsu2::dyno::Rollers::place(&car.car_type, com, glam::Quat::IDENTITY, |_| Some(0.5));
    // Two rollers under each axle, 0.52 m apart, on the centre line, 0.175
    // below the floor (0x56D1B0).
    let c = rollers.centres;
    assert!(((c[0].x - c[1].x) - 0.52).abs() < 1e-4 && ((c[2].x - c[3].x) - 0.52).abs() < 1e-4, "{c:?}");
    assert!(c.iter().all(|p| (p.y - 20.0).abs() < 1e-4 && (p.z - 0.325).abs() < 1e-4), "{c:?}");
    let (mut top, mut pitch_up, mut t) = (0.0f32, 0.0f32, 0.0f32);
    while t < 30.0 && pull.progress() < 1.0 {
        pull.step(nfsu2::dyno::FRAME);
        t += nfsu2::dyno::FRAME;
        let p = pull.pose();
        rollers.turn_by(p.spin, nfsu2::dyno::FRAME);
        top = top.max(p.spin[2]);
        pitch_up = pitch_up.max((p.tilt * glam::Vec3::X).z.asin().to_degrees());
        assert!(p.rise.abs() < 0.05, "{t}: {p:?}");
    }
    println!("dyno pose: top spin {top:.1} rad/s, nose up {pitch_up:.2} deg, rollers {:?}", rollers.angles);
    assert!(top > 60.0, "the wheels spin up: {top}");
    assert!(pitch_up > 0.05 && pitch_up < 3.0, "the body squats: {pitch_up}");
    assert!(rollers.angles.iter().all(|a| *a > 0.0), "{:?}", rollers.angles);
}

#[test]
#[ignore = "manual dump: prints the shop movie triggers"]
fn dump_shop_movies() {
    let root = common::game_root();
    let data = nfsu2::frontend::career::CareerData::load(&root).unwrap();
    for s in &data.shops {
        let t = nfsu2::frontend::career::RouteTriggers::load(&root);
        let b = t.find(s.trigger).map(|t| (t.min, t.max, t.position));
        eprintln!("{:24} kind {} movie {:?} scene {:?} hash {:08X} trig {:?}", s.name, s.kind, s.movie, s.scene_file, s.hash, b);
    }
}

/// Career audit round 5: the sponsor race's purse (0x532E60 / 0x500C30),
/// and the last stage's end moving the career to stage 6 (0x528330).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn career_round5_rules() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let data = CareerData::load(root).unwrap();
    let mut p = Profile::new("Test");
    p.skip_intro(&data, root, "PEUGOT", &mut nfsu2::opponents::GameRng::default());
    p.stage = 2;
    let drag = data.events.iter().find(|e| e.name == "S2_SPON_DRAG_3").unwrap().clone();
    assert_eq!(p.race_bank(&data, &drag), drag.reward);
    let edelbrock = data.sponsors.values().find(|s| s.name == "EDELBROCK").unwrap().clone();
    p.sign(&edelbrock);
    // Seen under Wine: the dialog's purse with EDELBROCK signed.
    assert_eq!(p.race_bank(&data, &drag), 875);
    // Stage 5: winning everything ends it and the career goes to stage 6.
    p.stage = 5;
    let first = data.stages.iter().find(|d| d.stage == 5).and_then(|d| d.sponsors.first()).and_then(|h| data.sponsors.get(h)).unwrap().clone();
    p.sign(&first);
    p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
    for _ in 0..200 {
        if p.stage != 5 {
            break;
        }
        let Some(e) = p.open_events(&data).first().map(|e| (*e).clone()) else {
            let need: Vec<u32> = data.milestones.iter().filter(|m| m.stage == 5 && m.required && !p.milestones.contains(&m.hash)).map(|m| m.hash).collect();
            assert!(!need.is_empty(), "stuck in stage 5");
            for m in need {
                p.shoot(&data, m, &mut nfsu2::opponents::GameRng::default());
            }
            continue;
        };
        p.win(&data, root, &e, 60.0, (0, 0), &mut nfsu2::opponents::GameRng::default());
    }
    assert_eq!(p.stage, 6);
    p.refresh_open(&data, &mut nfsu2::opponents::GameRng::default());
    assert!(p.open_events(&data).is_empty());
}
