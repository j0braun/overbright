//! The career's speech callbacks (`dday::Hints`) played through the audio's offline speech path: a test that names
//! both, so it lives with the front end (the audio sits below it).

use super::Hints;

/// Exercise the production career callbacks, including retention of the
/// voice chosen at engagement and the first-outrun state transition.
/// Asset-backed: ignored without NFSU2_ROOT.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn outrun_result_callbacks_reach_offline_speech_for_every_win_voice() {
    let root = crate::testkit::game_root();
    let mixer = crate::audio::mixer::Mixer::offline(22050);
    let mut streams = crate::audio::streams::Streams::new(std::path::Path::new(&root));
    let mut play = |tuple: &[u32]| {
        // Game::rachel_outrun stops the previous line, then submits
        // this tuple unchanged. No device or game renderer is opened.
        streams.stop_speech(&mixer);
        assert!(streams.say(&mixer, tuple), "callback tuple rejected: {tuple:?}");
        let mut mixed = Vec::new();
        mixer.pump(22050 * 3, &mut mixed);
        assert!(mixed.iter().all(|v| v.is_finite()));
        assert!(mixed.iter().any(|v| v.abs() > 0.001), "silent callback tuple: {tuple:?}");
    };
    for (pick, voice, selector) in [(0, 1, 3), (1, 2, 1), (2, 4, 2), (3, 8, 4), (4, 0x200, 5)] {
        for win_draw in [0, 1] {
            let mut hints = Hints::default();
            hints.dday_over = true;
            let mut start_draws = Vec::new();
            let start = hints.outrun_start(&mut |bound| {
                start_draws.push(bound);
                pick
            });
            assert_eq!(start_draws, [5]);
            assert_eq!(start, [8, 1, voice, selector]);
            play(&start);
            let mut end_draws = Vec::new();
            let win = hints.outrun_end(true, &mut |bound| {
                end_draws.push(bound);
                win_draw
            });
            assert_eq!(end_draws, [2]);
            assert_eq!(win, [8, if win_draw == 0 { 8 } else { 16 }, voice, selector]);
            play(&win);
        }
    }
    let mut hints = Hints::default();
    assert!(!hints.dday_over);
    let start = hints.outrun_start(&mut |_| panic!("the first outrun does not choose a random voice"));
    assert_eq!(start, [8, 0x20, 0x200, 5]);
    play(&start);
    let win = hints.outrun_end(true, &mut |_| panic!("the first result does not draw a variant"));
    assert_eq!(win, [8, 0x40, 0x200, 5]);
    assert!(hints.dday_over);
    play(&win);
    let start = hints.outrun_start(&mut |_| 0);
    assert_eq!(start, [8, 1, 1, 3]);
    play(&start);
    let win = hints.outrun_end(true, &mut |_| 1);
    assert_eq!(win, [8, 16, 1, 3]);
    play(&win);
}
