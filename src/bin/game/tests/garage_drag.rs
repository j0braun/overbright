//! Turning the car on the front end's platform with the mouse (GarageMain
//! 0x4A5D90 / 0x4D82D0, docs/formats/front-end.md "Turning the car"): a
//! middle-button drag turns the car by the same angle at every display
//! rate, as the original at 60 fps (0.5° per 640-wide unit of the window);
//! the clicking button does not turn it; the drawn frames between the
//! ticks turn it the short way across 360°.
use super::*;
use crate::InputEvent;
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

fn menu_game() -> Game {
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("DRAG");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.mode = game.icon_menu(MenuKind::Main);
    game
}

/// A drag of 4 px a tick for 50 ticks (200 px of the 1280-wide window:
/// 100 units) with `button`, run at `hz`; the car's angle at each tick end
/// from tick 20 on, and the drawn angles.
fn drag(hz: Option<u32>, button: fn(bool) -> InputEvent) -> (Vec<(u64, u32)>, Vec<f32>) {
    let mut game = menu_game();
    let base = game.pacing.clocks.tick;
    let at = |k: u32| InputEvent::CursorMoved(glam::Vec2::new(400.0 + 4.0 * k as f32, 480.0));
    game.input_at(base + 20, at(0));
    game.input_at(base + 30, button(true));
    for k in 1..=50 {
        game.input_at(base + 30 + u64::from(k), at(k));
    }
    game.input_at(base + 90, button(false));
    let (mut ticks, mut drawn) = (Vec::new(), Vec::new());
    let mut s = 11u64;
    while game.pacing.clocks.tick < base + 120 {
        let units = match hz {
            Some(hz) => nfsu2::frame_clock::UNITS / i64::from(hz),
            None => {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (4 + ((s >> 20) % 37) as i64) * 36
            }
        };
        let alpha = game.advance(units as f32 / nfsu2::frame_clock::UNITS as f32).unwrap();
        let tick = game.pacing.clocks.tick - base;
        if let Some(r) = game.garage_fe.request.as_ref()
            && tick >= 20
            && ticks.last().is_none_or(|(t, _)| *t != tick)
        {
            ticks.push((tick, r.view.at(r.now).car_angle.to_bits()));
        }
        if let Some(d) = game.frame_garage(alpha) {
            drawn.push(d.view.car_angle);
        }
    }
    (ticks, drawn)
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn a_middle_drag_turns_the_car_alike_at_every_rate() {
    testkit::game();
    let (want, _) = drag(Some(60), InputEvent::Middle);
    let first = f32::from_bits(want[0].1);
    let last = f32::from_bits(want.last().unwrap().1);
    // 100 units at 0.5° (the ticks' dt: 29–31 of 1/1800 s, so ± a little).
    let turned = (last - first).rem_euclid(360.0);
    assert!((turned - 50.0).abs() < 1.0, "turned {turned}° ({first} → {last})");
    for hz in [Some(30), Some(120), Some(200), None] {
        let (got, drawn) = drag(hz, InputEvent::Middle);
        for (t, a) in &got {
            if let Some((_, w)) = want.iter().find(|(wt, _)| wt == t) {
                assert_eq!(a, w, "{hz:?} Hz: the angle differs at tick {t}");
            }
        }
        // The drawn frames step the short way (no frame swings back
        // through 180° where the angle wraps at 360°).
        for w in drawn.windows(2) {
            let d = (w[1] - w[0]).rem_euclid(360.0);
            assert!(d < 10.0 || d > 350.0, "{hz:?} Hz: a drawn step of {d}°");
        }
    }
    // The clicking button drags nothing.
    let (left, _) = drag(Some(60), InputEvent::Button);
    assert!(left.iter().all(|(_, a)| *a == left[0].1), "the left button turned the car");
}
