//! The three differences of the career's stages 1–2 against the
//! original found by playing it (docs/native-fixes/NP-FIX-1.tsv),
//! on sandbox profiles
//! (`test_isolation`).
use super::*;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// A stage-1 career past the intro with save prompts on, its front-end
/// clock run (hints +0x11C; the bot's was 2926 at the garage) and no
/// front-end line said yet, in the shop named `shop`.
fn career_in(shop: &str) -> Game {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("Tess");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    p.hints.fe_last = [0; 28];
    p.hints.fe_clock = 2926;
    game.profile = Some(p);
    game.settings.autosave = false;
    game.settings.save_prompts = true;
    game.shop.visit = game.game_data.career.shops.iter().find(|s| s.name == shop).map(|s| s.hash);
    assert!(game.shop.visit.is_some(), "{shop} in the career data");
    game
}

fn frames(game: &mut Game, n: usize) {
    for _ in 0..n {
        game.update(1.0 / 60.0).unwrap();
    }
}

/// UI_ChoosePerformanceCategory's INIT (0x55C530 on
/// 0x35F8620B) says kind 1 and takes the snapshot (0x55C697); opening a
/// category (the item's action 0x53B780 on 0x0C407210 → 0x525050(+0x48),
/// table 0x5250E8: Engine 3 → kind 7) then stamps hints +0x120 + 7·4 =
/// +0x13C, so leaving without a purchase asks to save (Wine: the
/// snapshot and the saved profile differ only there, 0 → 0x005489C3). A
/// second visit within the hour (0x5243E0: +0x11C − time ≤ 3600 × 4000)
/// says nothing, stamps nothing: no question.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn perf_shop_engine_asks_on_the_first_visit_only() {
    let mut game = career_in("CC_PERF_SHOP");
    game.mode = game.perf_category_menu(0);
    let Mode::Menu(Menu::Icons(m, _)) = &game.mode else { panic!("the categories") };
    assert_eq!(m.options[0].action, Action::PerfCategory(3), "Engine open in stage 1 (Wine: opened there)");
    frames(&mut game, 3);
    assert!(game.save_flow.snapshot.borrow().is_some(), "the categories' snapshot (0x55C697)");
    game.act(Action::PerfCategory(3), MenuKind::PerfCategory).unwrap();
    assert_eq!(game.profile.as_ref().unwrap().hints.fe_last[7], 2926, "Engine's line (kind 7) stamps +0x13C");
    frames(&mut game, 3);
    assert!(game.changed_since_snapshot(), "first visit: leaving asks to save");
    // The second visit, a few seconds of front-end time later.
    game.mode = Mode::Explore;
    frames(&mut game, 1);
    game.profile.as_mut().unwrap().hints.fe_clock += 5 * 4000;
    game.mode = game.perf_category_menu(0);
    frames(&mut game, 3);
    assert!(game.save_flow.snapshot.borrow().is_some());
    game.act(Action::PerfCategory(3), MenuKind::PerfCategory).unwrap();
    assert_eq!(game.profile.as_ref().unwrap().hints.fe_last[7], 2926, "within the hour: no line, no stamp");
    assert!(!game.changed_since_snapshot(), "second visit: no question");
}

/// The other option posts: UI_PerformanceTuning_Master's open category
/// (0x55D920 → 0x525110(+0x48) at 0x55D9CC, table 0x525188: 0x13, 0x14,
/// 0x18, 0x19, 0x15, 0x16, 0x17) and every performance category (0x525050);
/// UI_Paint's INIT on the Paint category (0x563420: kind 8 when 0x83899C
/// = 2). None outside a career (0x5243E0 checks 0x850078 = 1).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn shop_option_lines_stamp_their_kinds() {
    let mut game = career_in("CC_PERF_SHOP");
    for (c, kind) in [(3u8, 7usize), (7, 6), (1, 0xC), (5, 0xA), (2, 9), (8, 0xB), (6, 3), (0, 0x10), (4, 0xD)] {
        game.act(Action::PerfCategory(c), MenuKind::PerfCategory).unwrap();
        assert_eq!(game.profile.as_ref().unwrap().hints.fe_last[kind], 2926, "category {c}: kind {kind:#x}");
    }
    for (i, kind) in [0x13usize, 0x14, 0x18, 0x19, 0x15, 0x16, 0x17].into_iter().enumerate() {
        game.act(Action::TuningCategory(i), MenuKind::Tuning).unwrap();
        assert_eq!(game.profile.as_ref().unwrap().hints.fe_last[kind], 2926, "tuning kind {kind:#x}");
    }
    // UI_Paint (Paint, not a vinyl's colours): kind 8 on its opening.
    let mut game = career_in("CC_GRAPHICS");
    game.mode = game.icon_menu(MenuKind::Graphics);
    frames(&mut game, 2);
    game.act(Action::Paint, MenuKind::Graphics).unwrap();
    assert!(matches!(&game.mode, Mode::Menu(Menu::Paint(p)) if p.vinyl.is_none()), "UI_Paint");
    frames(&mut game, 2);
    assert_eq!(game.profile.as_ref().unwrap().hints.fe_last[8], 2926, "UI_Paint's INIT: kind 8");
    // Customize (no career): nothing.
    let mut game = career_in("CC_PERF_SHOP");
    game.save_flow.customize = Some(None);
    game.act(Action::PerfCategory(3), MenuKind::PerfCategory).unwrap();
    assert_eq!(game.profile.as_ref().unwrap().hints.fe_last[7], 0, "not in a career");
}

/// Stage 2: 0x527D40 copies the open list aside, clears it
/// (0x527D4B..0x527D84) and returns while career +0xC08 is set (0x527DA3):
/// a world challenge's start (0x533BF0, its 0x533320) empties the open
/// events (Wine: 16 → 0 on STAGE_2_SHOW_1's Accept, still 0 at the
/// shoot), every rebuild during it keeps them empty, and its end
/// (0x541970 → 0x533690 → 0x533320) or give-up (0x533D00 state 7) rebuilds
/// from nothing: every slot drawn anew, the event triggers with it. A
/// drop (quit, Jump to Garage: 0x5139C0 / 0x513B30) clears +0xC08 without
/// a rebuild; the next city start (0x5412E0) fills the list. +0xC08 is not
/// saved (0x514530).
#[test]
#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
fn world_challenge_empties_the_open_events_until_it_ends() {
    let mut game = career_in("CC_PERF_SHOP");
    game.shop.visit = None;
    let (index, stage) = game
        .game_data.career.mag_triggers
        .iter()
        .enumerate()
        .find(|(_, t)| t.stage == 2)
        .map(|(i, t)| (i, t.stage as u8))
        .expect("a stage-2 world challenge");
    {
        let data = &game.game_data.career;
        let p = game.profile.as_mut().unwrap();
        p.stage = stage;
        p.refresh_open(data, &mut game.rng);
    }
    game.refresh_event_triggers();
    let before = game.profile.as_ref().unwrap().open.clone();
    assert!(!before.is_empty(), "stage 2 has open events");
    let triggers = |g: &Game| g.career_world.event_starts.as_ref().map_or(0, |(_, s)| s.len());

    game.start_challenge(index);
    assert!(game.profile.as_ref().unwrap().open.is_empty(), "the challenge's start empties the list");
    assert!(game.profile.as_ref().unwrap().open_events(&game.game_data.career).is_empty());
    assert_eq!(triggers(&game), 0, "no event triggers in the world");
    {
        let data = &game.game_data.career;
        let p = game.profile.as_mut().unwrap();
        p.refresh_open(data, &mut game.rng);
        assert!(p.open.is_empty(), "a rebuild during the challenge (a city start) keeps it empty");
    }

    // Its end: the list rebuilt from an empty one (what a rebuild of an
    // empty list with the same generator draws).
    let mut expect = game.profile.as_ref().unwrap().clone();
    expect.challenge_running = false;
    let mut rng = game.rng.clone();
    expect.refresh_open(&game.game_data.career, &mut rng);
    game.finish_challenge();
    let after = game.profile.as_ref().unwrap().open.clone();
    assert_eq!(after, expect.open, "drawn anew from an empty list");
    assert_eq!(after.len(), before.len(), "the slots filled again");
    assert!(triggers(&game) > 0, "the triggers back");

    // A drop (quit / Jump to Garage): empty until the next rebuild.
    game.start_challenge(index);
    game.drop_challenge();
    let p = game.profile.as_ref().unwrap();
    assert!(!p.challenge_running && p.open.is_empty(), "0x5139C0 / 0x513B30: +0xC08 cleared, no rebuild");
    // Saved and loaded in that state: not running, the list rebuilt by
    // the city start.
    let ctx = career::NativeContext::get().unwrap();
    let blank = nfsu2::formats::native_save::NativeSave::parse(&testkit::private("tests/fixtures/native_saves/career_end_won.sav"))
    .unwrap();
    let mut loaded = Profile::from_native(&p.to_native(&blank, ctx), ctx);
    assert!(!loaded.challenge_running, "+0xC08 is not saved");
    loaded.refresh_open(&game.game_data.career, &mut game.rng);
    assert!(!loaded.open.is_empty(), "the city start fills it");
}

/// Stage 2: UI_GenericParts_Browser's category strip is
/// built only by 0x54FAF0 → 0x545B60, from the constructor (0x55E830 at
/// 0x55EA8A) and on leaving carbon fibre (0x567830); back from a category
/// or after a purchase the items keep their +0x22. So a category a
/// purchase would open (Carbon Fiber, 0x544F10: a carbon part listed for
/// slot 9, 0x0C, 0x20 or 7) stays locked until the screen is built again
/// (Wine: locked after the spoiler bought in the same visit, orig/70-c*).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn body_strip_locks_are_taken_when_the_screen_is_built() {
    let mut game = career_in("CC_PERF_SHOP");
    game.shop.visit = game.game_data.career.shops.iter().find(|s| s.kind == 2).map(|s| s.hash);
    let carbon = screens::BODY_SHOP_CATEGORIES.iter().position(|c| c.0 == 0x5E).unwrap();
    let locked = |m: &Mode| match m {
        Mode::Menu(Menu::Parts(b)) => b.categories[carbon].locked,
        _ => panic!("the body shop"),
    };
    let m = game.new_parts_browser(0);
    assert!(locked(&m), "stage 1, nothing carbon: locked");
    // What a purchase would do to 0x544F10's count: a carbon spoiler
    // owned (listed by 0x515500's owned test).
    let car = game.profile.as_ref().unwrap().car.clone();
    let spoiler = game
        .game_data.parts
        .slot_parts(&car, 0x0C)
        .into_iter()
        .find(|p| p.level > 0 && game.game_data.parts.attribute(p, 0x721A_FF7C).is_some_and(|v| v != 0))
        .map(|p| p.hash)
        .expect("a carbon spoiler for the car");
    game.profile.as_mut().unwrap().owned.insert((0x0C, spoiler));
    assert!(locked(&game.parts_browser(0, None)), "back on the strip in the same visit: still locked");
    assert!(locked(&game.parts_browser_in(None, 3, Some(0))), "after a purchase: still locked");
    assert!(!locked(&game.new_parts_browser(0)), "the screen built again: open");
}

/// The categories open at each of the career's unlock points, the car a
/// stock 206 (PEUGOT), every screen built afresh: the body shop's strip
/// (0x545B60: a category with no part past the stock one locked, 0x545710;
/// rims 0x539C30 two rims; carbon 0x544F10; kits 0x545710), the
/// performance categories (0x530410: locked until a level 1–3 of the cost
/// category unlocks, 0x52A120) and the graphics categories (Decals by
/// 0x529F10). Normal events are won in data order, stage by stage, every
/// shop driven into (0x512FF0, the announced ones' visit flag); a row
/// is written whenever a lock changes. One letter per category, upper
/// case open, `.` locked. Body: F R S P H O L T M X I C W; perf: E C T
/// U N Y B W O (the 0x530500 order); graphics: P V D.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn category_locks_at_the_career_unlock_points() {
    let mut game = career_in("CC_PERF_SHOP");
    let body_shop = game.game_data.career.shops.iter().find(|s| s.kind == 2).map(|s| s.hash);
    let row = |game: &mut Game| -> String {
        let mut out = String::new();
        game.shop.visit = body_shop;
        let Mode::Menu(Menu::Parts(b)) = game.new_parts_browser(0) else { panic!() };
        for (c, l) in b.categories.iter().zip("FRSPHOLTMXICW".chars()) {
            out.push(if c.locked { '.' } else { l });
        }
        out.push(' ');
        let Mode::Menu(Menu::Icons(m, _)) = game.perf_category_menu(0) else { panic!() };
        for (o, l) in m.options.iter().zip("ECTUNYBWO".chars()) {
            out.push(if o.action == Action::Unavailable { '.' } else { l });
        }
        out.push(' ');
        let Mode::Menu(Menu::Icons(m, _)) = game.graphics_menu(0) else { panic!() };
        for (o, l) in m.options.iter().zip("PVD".chars()) {
            out.push(if o.action == Action::Unavailable { '.' } else { l });
        }
        out
    };
    let mut table = Vec::new();
    let mut last = String::new();
    for stage in 1..=5u8 {
        game.profile.as_mut().unwrap().stage = stage;
        let events: Vec<_> = game
            .game_data.career.events
            .iter()
            .filter(|e| e.stage == stage && e.category == career::Category::Normal)
            .cloned()
            .collect();
        for (n, e) in std::iter::once(None).chain(events.iter().map(Some)).enumerate() {
            if let Some(e) = e {
                let (data, root) = (&game.game_data.career, &game.game_data.root);
                game.profile.as_mut().unwrap().win_paid(data, root, e, 60.0, None);
            }
            // Every shop driven into once open to the stage (0x512FF0).
            let shops = game.game_data.career.shops.clone();
            for s in &shops {
                game.profile.as_mut().unwrap().visit_shop(s);
            }
            let r = row(&mut game);
            if r != last {
                table.push(format!("stage {stage} wins {n:2}: {r}"));
                last = r;
            }
        }
    }
    let table = table.join("\n");
    eprintln!("{table}");
    assert_eq!(table, EXPECTED_LOCKS.trim());
}

/// Stage 2 before its 4th normal win (`stage 1 wins 5` row, unchanged
/// through stage 2's third) is the strip seen under Wine (the original's career at stage 2,
/// orig/70-c*.jpg, the 206): Roof Scoops, Side Mirrors, Carbon Fiber and
/// Wide Body Kits locked, the rest open; Side Mirrors open at the 4th
/// (SMS_UNLOCK_04). The 206 has no wide body kit (0x545710: none past
/// stock), so W never opens.
const EXPECTED_LOCKS: &str = "
stage 1 wins  0: FRS...LT..I.. ECTUN.... PVD
stage 1 wins  4: FRS...LT.XI.. ECTUN.... PVD
stage 1 wins  5: FRSPH.LT.XI.. ECTUNYBW. PVD
stage 2 wins  4: FRSPH.LTMXI.. ECTUNYBW. PVD
stage 2 wins 13: FRSPHOLTMXI.. ECTUNYBW. PVD
stage 3 wins  3: FRSPHOLTMXIC. ECTUNYBW. PVD
stage 4 wins 13: FRSPHOLTMXIC. ECTUNYBWO PVD
";
