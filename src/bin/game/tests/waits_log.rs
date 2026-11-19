//! The waits log's hooks (docs/developing/waits-log.md): the game's screens, a
//! drive's load and its frames reach the lines, named with the game's own
//! names only (no profile, car or folder).
use super::*;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_waits_lines_follow_the_screens_and_the_load_and_hold_nothing_of_the_player() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut profile = Profile::new("WAITSLOGTEST");
    profile.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    profile.stage = 1;
    game.profile = Some(profile);
    game.loads.waits.captured = Some(Vec::new());
    game.mode = game.icon_menu(MenuKind::Main);
    game.waits_frame(16.0);
    game.loads.waits.key();
    game.explore(None).unwrap();
    assert!(matches!(game.mode, Mode::Explore));
    for _ in 0..12 {
        game.waits_frame(10.0);
    }
    game.mode = game.icon_menu(MenuKind::ExplorePause);
    game.waits_frame(10.0);
    game.waits_end();
    let lines = game.loads.waits.captured.take().unwrap();
    assert_eq!(lines.len(), 5, "{lines:#?}");
    assert!(lines[0].starts_with("wait: start -> first frame (menu Main) "), "{}", lines[0]);
    assert!(lines[0].contains("; cache ") || lines[0].contains("; no cache folder"), "{}", lines[0]);
    let load = &lines[1];
    assert!(
        load.starts_with("wait: menu Main -> free roam: ") && load.contains(" ms from the key; load of L4RA "),
        "{load}"
    );
    for part in ["world and car ", "set-up ", "the rest "] {
        assert!(load.contains(part), "{part}: {load}");
    }
    assert!(
        load.contains("world loaded, car loaded, cache files ")
            && (load.contains("loading screen not shown; first frame ")
                || load.contains("loading screen shown; first frame ")),
        "{load}"
    );
    assert_eq!(lines[2], "wait: free roam -> menu ExplorePause: no key");
    // The drive's first frame and the one after it are the load's.
    assert!(
        lines[3].starts_with("drive: free roam, 0 s, 10 frames, median 10.00 ms (100 fps), 1% low 100 fps"),
        "{}",
        lines[3]
    );
    assert!(lines[4].starts_with("wait: the game ran "), "{}", lines[4]);
    let root = game.game_data.root.to_string_lossy().into_owned();
    let home = std::env::var("HOME").unwrap_or_default();
    for l in &lines {
        assert!(!l.contains("WAITSLOGTEST") && !l.contains("PEUGOT"), "a profile or car name: {l}");
        assert!(!l.contains(&root) && (home.len() < 2 || !l.contains(&home)), "a folder: {l}");
    }
}
