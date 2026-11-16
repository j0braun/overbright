//! Differences found by playing the career against the
//! original (docs/native-fixes/NP-PLAY.tsv),
//! on sandbox profiles (`test_isolation`).
use super::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// A career past the intro with save prompts on, the garage's hint not yet
/// said (Rachel's crib line, hints +10 = 0, as on the first visit).
fn first_garage_visit() -> Game {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("Mira");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    p.hints.events = false;
    p.hints.fe_last = [0; 28];
    // Its front-end clock (+0x11C, 4000 Hz) has run, as in any career
    // (the bot's was 2926 at the garage).
    p.hints.fe_clock = 2926;
    game.profile = Some(p);
    game.settings.autosave = false;
    game.settings.save_prompts = true;
    game
}

/// UI_CareerCrib's INIT (0x4AF3A0 on 0x35F8620B) runs Rachel's line
/// once (0x5243E0(0x11) at 0x4AF502: on the first visit it sets hints +10
/// and returns at 0x5244F0 without stamping +0x120 + 0x11 * 4), then takes
/// the snapshot (0x52A470 at 0x4AF515). Leaving (0x4AF360) asks to save
/// only when 0x52A4D0 finds the profile changed. Frames spent in the
/// garage must not change it: under Wine the first visit's Yes drives
/// straight out (the original's career at stage 1, orig/20-*, 21-out-*).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn garage_first_visit_frames_do_not_change_the_snapshot() {
    let mut game = first_garage_visit();
    game.mode = game.icon_menu(MenuKind::Crib);
    for _ in 0..5 {
        game.update(1.0 / 60.0).unwrap();
    }
    let p = game.profile.as_ref().unwrap();
    assert!(p.hints.events, "the crib's first line was said (hints +10)");
    assert_eq!(p.hints.fe_last[0x11], 0, "the first visit's line does not stamp its time (0x5244F0 returns first)");
    assert!(game.save_flow.snapshot.borrow().is_some(), "the crib's snapshot (0x4AF515)");
    assert!(!game.changed_since_snapshot(), "nothing changed in the garage: no save question");
}

/// The dialog over the garage: "Return to Explore Mode?" is a pushed
/// package (0x5587A0); its No (0x43DA9FD0) only re-takes the kept snapshot
/// (0x4AF4BB → 0x52A470), no second INIT, so no second line.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn garage_dialog_no_does_not_rerun_the_line() {
    let mut game = first_garage_visit();
    game.mode = game.icon_menu(MenuKind::Crib);
    for _ in 0..3 {
        game.update(1.0 / 60.0).unwrap();
    }
    game.act(Action::Explore, MenuKind::Crib).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Dialog(..))), "Return to Explore Mode asks");
    for _ in 0..3 {
        game.update(1.0 / 60.0).unwrap();
    }
    // No is focused.
    game.menu_key(Key::Accept).unwrap();
    assert!(matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))), "back in the garage");
    for _ in 0..3 {
        game.update(1.0 / 60.0).unwrap();
    }
    assert_eq!(game.profile.as_ref().unwrap().hints.fe_last[0x11], 0, "no second line on the dialog's No");
    assert!(!game.changed_since_snapshot(), "still unchanged: no save question");
}
