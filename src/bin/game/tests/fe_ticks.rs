//! docs/original/uncapped-fps.md, "Front end out of the draw": the front end's state steps on the
//! tick, never in the draw. The garage scripted at 30 / 60 / 120 / 200 Hz
//! and on a jittered schedule has the same front-end state at equal ticks;
//! drawing a frame twice, or not at all, changes nothing; a click acts on
//! the element under the point it arrived at, at the tick it is applied,
//! at every rate.
use super::*;
use crate::InputEvent;
// The test kit (tests/common/mod.rs, docs/developing/testing.md).
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// A display schedule's frames (1/36 000 s units): exact at `hz`, or
/// jittered (whole milliseconds 4–40, a 120 ms stall now and then).
fn frames(hz: Option<u32>, n: usize) -> Vec<i64> {
    let mut s = 7u64;
    (0..n)
        .map(|_| match hz {
            Some(hz) => nfsu2::frame_clock::UNITS / i64::from(hz),
            None => {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let ms = if (s >> 33) % 200 == 0 { 120 } else { 4 + ((s >> 20) % 37) as i64 };
                ms * 36
            }
        })
        .collect()
}

/// Everything the front-end steps leave for the draw (`fe_tick`): the
/// hovers, the garage's request and FE state, the scene's car, hydraulics,
/// part poses and particles, the FE sounds posted so far, the RNG.
fn fe_digest(game: &Game) -> String {
    let gf = &game.garage_fe;
    let mut out = format!(
        "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {} {} {:?}",
        game.fe.help_hover,
        game.fe.fe_hover,
        gf.lot.as_ref().map(|l| (l.0, l.1)),
        gf.crib,
        gf.type_shown,
        gf.parts_sounded,
        gf.toggled,
        gf.left,
        game.rng.0,
        game.dev.fe_played,
    );
    if let Some(r) = gf.request.as_ref() {
        let v = r.view.at(r.now);
        out += &format!(
            " req {:?} {:?} {:?} {:?} {} {} {} {:?}",
            r.car,
            r.installed,
            r.open,
            r.hydraulics,
            r.now.to_bits(),
            v.car_angle.to_bits(),
            v.platform_angle.to_bits(),
            v.camera.eye
        );
    }
    if let Some(g) = game.render.garage.as_ref() {
        out += &format!(" scene {} {:?}", g.platform.name, g.car.as_ref().map(|c| (&c.0, &c.1)));
        if let Some((h, t)) = g.hydro.as_ref() {
            out += &format!(" hy {:?} {}", h.corner.map(f32::to_bits), t.to_bits());
        }
        if let Some(p) = g.car_poses.as_ref() {
            out += &format!(" poses {:?}", p.map(|c| c.map(|x| x.map(|x| x.value.to_bits()))));
        }
        if let Some(fx) = g.fx.as_ref() {
            out += &format!(" fx {} {}", fx.0.particles.len(), fx.2.to_bits());
            for p in &fx.0.particles {
                out += &format!(" {:?}", p.position.to_array().map(f32::to_bits));
            }
        }
    }
    out
}

/// A fresh career's car (`--flow garage`'s), with a level-2 hydraulics
/// part fitted (the `hydro` shots' `install_shot_hydraulics`).
fn garage_game() -> Game {
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("FE_TICKS");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    let (car, installed, perf) = game.player_car();
    let hash = game.game_data.parts.slot_parts(&car, 0xA1).into_iter().find(|p| p.level == 2).map(|p| p.hash);
    if let Some(p) = game.profile.as_mut() {
        (p.car, p.installed, p.perf) = (car, installed, perf);
        if let Some(h) = hash {
            p.installed.insert(0xA1, h);
        }
    }
    game
}

/// Runs `frames` through the window's cadence (`advance`, then the
/// frame's draw), with `keys` given at their ticks; the digest at
/// each frame's end, by the tick index that ends it.
fn run(game: &mut Game, frames: &[i64], keys: &[(u64, KeyCode)]) -> std::collections::BTreeMap<u64, String> {
    let base = game.pacing.clocks.tick;
    for &(t, k) in keys {
        game.input_at(base + t, InputEvent::press(k));
        game.input_at(base + t + 1, InputEvent::KeyUp(k));
    }
    let mut seen = std::collections::BTreeMap::new();
    for &u in frames {
        game.advance(u as f32 / nfsu2::frame_clock::UNITS as f32).unwrap();
        // The window's draw (headless: its preparation, no GPU part).
        game.draw().unwrap();
        seen.insert(game.pacing.clocks.tick - base, fe_digest(game));
    }
    seen
}

/// The garage scripted (a body-shop list opened, stepped, closed; the
/// Hydraulics list stepped, the FE car pumping) at every rate: the
/// front-end state at each tick two schedules both end a frame on is the
/// same, the garage's sounds and particles included.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn garage_state_is_the_same_at_every_rate() {
    testkit::game();
    let script = |game: &mut Game, hz: Option<u32>| {
        let n = match hz {
            Some(hz) => 420 * hz as usize / 60,
            None => 420,
        };
        game.mode = game.parts_browser(0, None);
        let mut all = run(
            game,
            &frames(hz, n),
            &[(30, KeyCode::Enter), (90, KeyCode::ArrowRight), (150, KeyCode::ArrowRight), (240, KeyCode::Escape)],
        );
        let at = all.keys().last().copied().unwrap_or(0);
        game.mode = game.specialties_browser(5, Some(1));
        for (t, d) in run(game, &frames(hz, n), &[(60, KeyCode::ArrowRight), (200, KeyCode::ArrowLeft)]) {
            all.insert(10_000 + t, d);
        }
        (all, at)
    };
    let mut reference = garage_game();
    let (want, want_at) = script(&mut reference, Some(60));
    assert!(want.values().any(|d| d.contains(" hy ")), "the hydraulics ran");
    assert!(want.values().any(|d| d.contains(" fx ")), "the scene's particles ran");
    for hz in [Some(30), Some(120), Some(200), None] {
        let mut game = garage_game();
        let (got, got_at) = script(&mut game, hz);
        let mut common = 0;
        for (t, d) in &got {
            // The second part starts where the schedule's first ended.
            if *t >= 10_000 && got_at != want_at {
                continue;
            }
            if let Some(w) = want.get(t) {
                assert_eq!(d, w, "{hz:?} Hz: the front end differs at tick {t}");
                common += 1;
            }
        }
        assert!(common >= 40, "{hz:?} Hz: only {common} ticks in common");
    }
}

/// Drawing a menu frame twice, or not at all, changes nothing: the quads
/// built twice are the same, and a run drawn every tick ends as one never
/// drawn (front-end state, garage scene, sounds, the RNG).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drawing_twice_or_not_at_all_changes_nothing() {
    testkit::game();
    let quads = |game: &Game| format!("{:?}", game.menu_quads(game.now()));
    let mut ends = Vec::new();
    for draws in [0usize, 1, 2] {
        let mut game = garage_game();
        game.mode = game.specialties_browser(5, Some(1));
        game.input_at(game.pacing.clocks.tick + 40, InputEvent::press(KeyCode::ArrowRight));
        for _ in 0..240 {
            game.advance(1.0 / 60.0).unwrap();
            let before = (fe_digest(&game), game.fe.help_hover);
            let q = quads(&game);
            for _ in 0..draws {
                // Headless: the frame's preparation runs, the GPU part
                // has no window.
                game.draw().unwrap();
                assert_eq!(quads(&game), q, "the quads built again");
            }
            assert_eq!(fe_digest(&game), before.0, "a draw changed the front end");
        }
        ends.push(fe_digest(&game));
    }
    assert_eq!(ends[0], ends[1]);
    assert_eq!(ends[0], ends[2]);
}

/// A click on UI_Main's right arrow, its press arriving at the arrow
/// and the cursor moved off before the tick, steps the strip at the tick
/// it is applied at (tested against that tick's layout), at every rate.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_click_acts_on_its_tick_at_every_rate() {
    testkit::game();
    for hz in [Some(60), Some(30), Some(120), Some(200), None] {
        let mut game = Game::new().unwrap();
        game.mode = game.icon_menu(MenuKind::Main);
        let base = game.pacing.clocks.tick;
        // A few ticks: the tick's layout makes the map.
        for &u in &frames(Some(60), 10) {
            game.advance(u as f32 / nfsu2::frame_clock::UNITS as f32).unwrap();
        }
        let arrow = nfsu2::frontend::mouse::rect("UI_Main", nfsu2::formats::hash::fe_hash("ScrollBar_Arrow_2"))
            .expect("the right arrow in the tick's map");
        let at = game.mouse_point(arrow, Vec2::splat(0.5)).expect("a frame");
        let selected = |g: &Game| match &g.mode {
            Mode::Menu(Menu::Icons(m, _)) => m.selected,
            _ => usize::MAX,
        };
        let before = selected(&game);
        let t = base + 40;
        game.input_at(t, InputEvent::CursorMoved(at));
        game.input_at(t, InputEvent::Button(true));
        game.input_at(t, InputEvent::CursorMoved(Vec2::new(1.0, 1.0)));
        game.input_at(t + 1, InputEvent::Button(false));
        for &u in &frames(hz, 120) {
            game.advance(u as f32 / nfsu2::frame_clock::UNITS as f32).unwrap();
            let next = game.pacing.clocks.tick;
            let want = if next > t { before + 1 } else { before };
            assert_eq!(selected(&game), want, "{hz:?} Hz: at tick {next} (click at {t})");
        }
    }
}
