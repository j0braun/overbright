//! The track-map widgets of UI_CareerWorldMap and UI_QRTrackSelect step on
//! the tick (`MapWidget`, `menu_tick`): a change's hand-off 0.3 s of the
//! real-time clock later (0x4E12DF / 0x4EFC30), on the same tick at every
//! display rate; the draw only reads the widget.
use super::*;
use crate::InputEvent;
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

fn widget(game: &Game) -> Option<screens::MapWidget> {
    match &game.mode {
        Mode::Menu(Menu::WorldMap(m)) => Some(m.widget),
        Mode::Menu(Menu::QuickRace(q)) => Some(q.widget),
        _ => None,
    }
}

/// The screen opened by `open`, a key at tick 10: the widget per tick
/// (by the tick count since the key's) on a schedule of `hz` drawn frames.
fn script(open: fn(&mut Game), key: KeyCode, hz: u32) -> std::collections::BTreeMap<u64, screens::MapWidget> {
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("MAP_WIDGET");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    open(&mut game);
    let base = game.pacing.clocks.tick;
    game.input_at(base + 10, InputEvent::press(key));
    game.input_at(base + 11, InputEvent::KeyUp(key));
    let mut seen = std::collections::BTreeMap::new();
    for _ in 0..90 * hz / 60 {
        game.advance(1.0 / hz as f32).unwrap();
        let t = game.pacing.clocks.tick - base;
        if let Some(w) = widget(&game) {
            seen.insert(t, w);
        }
    }
    seen
}

/// A change (the World Map's Down onto the first event; the track
/// select's Right) is handed to the widget 18 RACE ticks after the key's
/// tick ((0x8651A8 − stamp) × 0.00025 ≥ 0.3: 1200 of 67, 64, 69 per tick),
/// the widget's countdown waiting meanwhile; at 30 / 120 / 200 Hz on the
/// same tick with the same state as at 60 Hz.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn the_map_widgets_hand_off_on_the_same_tick_at_every_rate() {
    testkit::game();
    let world_map: fn(&mut Game) = |g| g.mode = g.world_map();
    let track_select: fn(&mut Game) = |g| {
        g.mode = Mode::Menu(Menu::QuickRace(g.quick_track_select(screens::QrMode::Circuit, g.now()).unwrap()));
    };
    for (name, open, key) in
        [("world map", world_map, KeyCode::ArrowDown), ("track select", track_select, KeyCode::ArrowRight)]
    {
        let want = script(open, key, 60);
        // The key acts at the start of tick 10 (the tick count 11 after it).
        let stamped = want[&11];
        assert!(stamped.changed.is_some(), "{name}: the key stamps the screen");
        for t in 11..=28 {
            assert_eq!(want[&t].changed, stamped.changed, "{name}: waiting at {t}");
            assert_eq!(want[&t].count, stamped.count, "{name}: the zoom waits with the hand-off ({t})");
        }
        assert!(want[&29].changed.is_none(), "{name}: handed off 18 ticks after the key");
        assert_ne!(want[&29].handed, stamped.handed, "{name}: another track handed");
        assert_eq!(want[&30].shown, want[&29].handed, "{name}: shown on the widget's next tick");
        // 0x577320 in the same tick: _ftol(dt × 4000) of its 30, 29 or 31.
        assert!(matches!(screens::MapWidget::ZOOM_COUNT - want[&30].count, 64 | 66 | 68), "{name}: the zoom starts");
        for hz in [30, 120, 200] {
            let got = script(open, key, hz);
            for (t, w) in &got {
                if let Some(v) = want.get(t) {
                    assert_eq!(w, v, "{name} at {hz} Hz, tick {t}");
                }
            }
        }
    }
}
