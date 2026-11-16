//! The event movies on the race dialog's Accept
//! (docs/native-fixes/NP-PLAY.tsv), on a
//! sandbox profile (`test_isolation`).
use super::*;
// The test kit (tests/common/mod.rs, docs/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// The race dialog's Accept (0x499B50) queues the event's +0x02 movie
/// whenever its name is not empty (0x4FFDD0; 0x53D640 adds "DriftTut" after
/// "Scene20", mode 1): no played-once flag, no option. Under Wine, with the
/// "_fr" movie files present (0x51C740's name rule), the original played
/// Scene20 on the first Accept of a fresh stage-1 drift and again on the
/// second Accept after a time-out (the original's career at stage 2,
/// orig/drift-accept-mem.txt, drift-accept2-mem.txt). Stage 1 saw none
/// because the stage-1 Wine copy had no "_fr" files: 0x566250 skips a
/// missing movie (0xC3960EB9).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn event_movie_queued_on_every_accept() {
    testkit::game();
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("Nora");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.settings.autosave = false;
    game.settings.save_prompts = false;
    let events = game.game_data.career.events.clone();
    let find = |movie: &str| events.iter().position(|e| e.movie.eq_ignore_ascii_case(movie)).unwrap();
    for (movie, queue) in
        [("StreetXTut", vec!["StreetXTut"]), ("DragTut", vec!["DragTut"]), ("Scene20", vec!["Scene20", "DriftTut"])]
    {
        let event = find(movie);
        for accept in 0..2 {
            game.engage_accepted(event).unwrap();
            let Mode::Menu(Menu::Movie(m)) = &game.mode else { panic!("{movie}: no movie on Accept {accept}") };
            let names: Vec<String> = m.queue.iter().map(|s| s.to_ascii_lowercase()).collect();
            let want: Vec<String> = queue.iter().map(|s| s.to_ascii_lowercase()).collect();
            assert_eq!(names, want, "{movie}: the queue on Accept {accept}");
            assert_eq!((m.mode, m.event), (1, Some(event)), "{movie}: mode 1, its end starts the race");
            game.mode = Mode::Explore;
        }
    }
}
