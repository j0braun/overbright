//! The garage's own scene between ticks (`garage::snapshot`): a drawn
//! frame's turntable, camera, car place, wheel drops, part poses and
//! particles are S_{n−1}'s exactly at α = 0, the tick's live state at
//! α ≥ 1, within the two between; the same at every display rate at equal
//! (tick, α); cut where the scene changes; and drawn by `render_garage`
//! pixel for pixel as the tick's own at α = 0.
use super::*;
use crate::InputEvent;
use crate::garage::snapshot::GarageDrawn;
#[path = "../../../../tests/common/mod.rs"]
mod testkit;

/// A display schedule's frames (1/36 000 s units): exact at `hz`, or
/// jittered (whole milliseconds 4–40, a 120 ms stall now and then).
fn frames(hz: Option<u32>, n: usize) -> Vec<i64> {
    let mut s = 11u64;
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

/// A fresh career's car in the body shop's parts browser.
fn garage_game() -> Game {
    let mut game = Game::new().unwrap();
    let mut p = Profile::new("GARAGE_BETWEEN");
    p.skip_intro(&game.game_data.career, &game.game_data.root, "PEUGOT", &mut game.rng);
    game.profile = Some(p);
    game.mode = game.parts_browser(0, None);
    game
}

/// The garage values a frame at `alpha` draws: the frame's own between
/// the ticks, or the tick's (S_n, the snapshot equal to the live scene).
fn drawn_values(game: &Game, alpha: f32) -> Option<GarageDrawn> {
    game.frame_garage(alpha).or_else(|| {
        let s = &game.interp.cur.garage;
        Some(GarageDrawn {
            view: s.view?,
            car_world: s.car_world,
            drops: s.hydro.map(|h| h.1),
            poses: s.poses,
            camber: s.camber,
            headlights: s.headlights,
            particles: s.particles.clone(),
            scale: Vec::new(),
        })
    })
}

/// Every drawn value bit for bit.
fn digest(d: &GarageDrawn) -> String {
    let v = &d.view;
    let b = |x: f32| x.to_bits();
    let mut out = format!(
        "{:?} {:?} {:?} {} {} {} {} {} {:?} {:?} {} {}",
        v.camera.eye.to_array().map(b),
        v.camera.target.to_array().map(b),
        b(v.camera.fov),
        b(v.car_angle),
        b(v.steer),
        b(v.platform_angle),
        v.disc,
        b(d.camber),
        d.car_world.map(|m| m.to_cols_array().map(b)),
        d.drops.map(|x| x.map(b)),
        d.headlights,
        d.particles.len()
    );
    if let Some(p) = d.poses.as_ref() {
        out += &format!(" {:?}", p.map(|c| c.map(|x| x.map(|x| (b(x.value), x.moved.to_array().map(b))))));
    }
    for (i, p) in d.particles.iter().enumerate() {
        out += &format!(
            " {} {:?} {} {}",
            p.id,
            p.position.to_array().map(b),
            p.life,
            d.scale.get(i).copied().unwrap_or(1.0)
        );
    }
    out
}

/// The garage scripted (the body shop's list opened: the camera blends
/// to the part's view and the car turns; stepped; closed: back to the base
/// view) on `frames`: per drawn frame (tick, α, the values drawn), and
/// the frames' checks against the two snapshots.
/// One drawn frame of [`run`]: its tick and α, the values it draws (and
/// their digest), and the newest tick's own car angle and camera eye x
/// (what every frame drew before the garage had snapshots).
struct Frame {
    tick: u64,
    alpha: f32,
    digest: String,
    drawn: Option<GarageDrawn>,
    tick_view: Option<(f32, f32)>,
    /// The car turns between the two ticks (S_{n−1}'s angle is not S_n's).
    turning: bool,
}

fn run(hz: Option<u32>) -> Vec<Frame> {
    let mut game = garage_game();
    let base = game.pacing.clocks.tick;
    for (t, k) in [(30u64, KeyCode::Enter), (120, KeyCode::ArrowRight), (210, KeyCode::Escape)] {
        game.input_at(base + t, InputEvent::press(k));
        game.input_at(base + t + 1, InputEvent::KeyUp(k));
    }
    let n = match hz {
        Some(hz) => 330 * hz as usize / 60,
        None => 330,
    };
    let mut out = Vec::new();
    for &u in &frames(hz, n) {
        let alpha = game.advance(u as f32 / nfsu2::frame_clock::UNITS as f32).unwrap();
        let tick = game.pacing.clocks.tick - base;
        let between = game.frame_garage(alpha);
        if let Some(d) = between.as_ref() {
            let (p, c) = (&game.interp.prev.garage, &game.interp.cur.garage);
            let (a, b) = (p.view.unwrap(), c.view.unwrap());
            if alpha == 0.0 {
                // S_{n−1} exactly.
                assert_eq!(
                    digest(d),
                    digest(&GarageDrawn { particles: p.particles.clone(), scale: Vec::new(), ..d.clone() })
                );
                assert_eq!(d.view.car_angle.to_bits(), a.car_angle.to_bits(), "{hz:?} Hz tick {tick}");
                assert_eq!(d.view.camera.eye.to_array().map(f32::to_bits), a.camera.eye.to_array().map(f32::to_bits));
                assert_eq!(
                    d.car_world.map(|m| m.to_cols_array().map(f32::to_bits)),
                    p.car_world.map(|m| m.to_cols_array().map(f32::to_bits))
                );
            }
            let within = nfsu2::render::native::fx::tick::within;
            assert!(within(d.view.car_angle, a.car_angle, b.car_angle), "{hz:?} Hz tick {tick} α {alpha}: car angle");
            assert!(within(d.view.platform_angle, a.platform_angle, b.platform_angle));
            assert!(within(d.view.camera.fov, a.camera.fov, b.camera.fov));
            for k in 0..3 {
                assert!(
                    within(d.view.camera.eye[k], a.camera.eye[k], b.camera.eye[k]),
                    "{hz:?} Hz tick {tick} α {alpha}: eye"
                );
            }
        }
        let drawn = drawn_values(&game, alpha);
        let tick_view = game.interp.cur.garage.view.map(|v| (v.car_angle, v.camera.eye.x));
        let turning = matches!((game.interp.prev.garage.view, game.interp.cur.garage.view), (Some(a), Some(b)) if a.car_angle != b.car_angle);
        out.push(Frame {
            tick,
            alpha,
            digest: drawn.as_ref().map(digest).unwrap_or_default(),
            drawn,
            tick_view,
            turning,
        });
    }
    out
}

/// The frames' values against the 60 Hz window's at every (tick, α) both
/// draw (α = 0 at every exact rate: 30 Hz frame k is 60 Hz frame 2k; the
/// jittered schedule's own α in between); the frames between ticks blend
/// (inside S_{n−1}..S_n, checked in `run`), and the turntable and the
/// camera move by even steps per drawn frame where the tick's state steps
/// once every 2 (120 Hz) or 3–4 (200 Hz) frames.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn garage_frames_are_the_ticks_and_the_same_at_every_rate() {
    testkit::game();
    let reference = run(Some(60));
    let want: std::collections::HashMap<(u64, u32), &String> =
        reference.iter().map(|f| ((f.tick, f.alpha.to_bits()), &f.digest)).collect();
    assert!(reference.iter().all(|f| f.alpha == 0.0), "a steady 60 Hz draws the ticks (α = 0)");
    for hz in [Some(30), Some(120), Some(200), None] {
        let got = run(hz);
        let mut common = 0;
        for f in &got {
            if let Some(w) = want.get(&(f.tick, f.alpha.to_bits())) {
                assert_eq!(&f.digest, *w, "{hz:?} Hz: the garage drawn at tick {} α {}", f.tick, f.alpha);
                common += 1;
            }
        }
        let between = got.iter().filter(|f| f.alpha > 0.0 && f.alpha < 1.0).count();
        // The jittered schedule shares only its α = 0 frames.
        let least = if hz.is_some() { 80 } else { 20 };
        assert!(common >= least, "{hz:?} Hz: only {common} frames in common with 60 Hz");
        if hz != Some(30) {
            assert!(between > 100, "{hz:?} Hz: {between} frames between ticks");
        }
        // The smoothness while the view moves (the part view's blend and
        // the car's turn after Enter at tick 30): per drawn frame, the
        // turntable angle's and the camera eye's step, drawn now against
        // the newest tick's own (what each frame drew before).
        if let Some(hz) = hz.filter(|&h| h >= 120) {
            let steps = |pick: &dyn Fn(&Frame) -> Option<f32>| -> Vec<f32> {
                got.windows(2)
                    .filter(|w| w[0].turning && w[1].turning)
                    .filter_map(|w| Some((pick(&w[1])? - pick(&w[0])?).abs()))
                    .collect()
            };
            let spread = |s: &[f32]| {
                let moving: Vec<f32> = s.iter().copied().filter(|&x| x > 1e-6).collect();
                let max = moving.iter().copied().fold(0.0f32, f32::max);
                let min = moving.iter().copied().fold(f32::INFINITY, f32::min);
                (s.len() - moving.len(), s.len(), min, max)
            };
            let after = spread(&steps(&|f| f.drawn.as_ref().map(|d| d.view.car_angle)));
            let before = spread(&steps(&|f| f.tick_view.map(|v| v.0)));
            let eye_after = spread(&steps(&|f| f.drawn.as_ref().map(|d| d.view.camera.eye.x)));
            let eye_before = spread(&steps(&|f| f.tick_view.map(|v| v.1)));
            eprintln!(
                "{hz} Hz, frames while the car turns, per drawn frame (still frames of all, smallest–largest step): car angle before {}/{} {:.4}–{:.4}°, after {}/{} {:.4}–{:.4}°; camera eye x before {}/{} {:.5}–{:.5} m, after {}/{} {:.5}–{:.5} m",
                before.0,
                before.1,
                before.2,
                before.3,
                after.0,
                after.1,
                after.2,
                after.3,
                eye_before.0,
                eye_before.1,
                eye_before.2,
                eye_before.3,
                eye_after.0,
                eye_after.1,
                eye_after.2,
                eye_after.3
            );
            assert_eq!(after.0, 0, "{hz} Hz: the turntable stands still on a drawn frame while it turns");
            assert!(before.0 > 0, "{hz} Hz: the tick's own state holds over frames");
            assert!(after.3 < 1.35 * after.2, "{hz} Hz: uneven turntable steps {}–{}", after.2, after.3);
        }
    }
}

/// The cut rules: no garage in S_{n−1} (entering it), another platform or
/// FE car, another disc, a request written after the tick: the tick's own
/// drawn (None); α ≥ 1 too.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn garage_cuts_draw_the_tick() {
    testkit::game();
    let mut game = garage_game();
    for _ in 0..40 {
        game.advance(1.0 / 60.0).unwrap();
    }
    assert!(game.frame_garage(0.5).is_some(), "a garage up for two ticks blends");
    // The capture's cost per tick, its particle buffer reused.
    let mut snap = game.interp.cur.garage.clone();
    game.capture_garage(&mut snap);
    let cap = snap.particles.capacity();
    let t = std::time::Instant::now();
    for _ in 0..2000 {
        game.capture_garage(&mut snap);
    }
    eprintln!(
        "garage capture: {} particles, {:.2} µs per tick",
        snap.particles.len(),
        t.elapsed().as_secs_f64() * 1e6 / 2000.0
    );
    assert_eq!(snap.particles.capacity(), cap, "the buffer is reused");
    assert!(game.frame_garage(1.0).is_none());
    let keep = game.interp.prev.garage.clone();
    game.interp.prev.garage.view = None;
    assert!(game.frame_garage(0.5).is_none(), "entering the garage");
    game.interp.prev.garage = keep.clone();
    game.interp.prev.garage.key ^= 1;
    assert!(game.frame_garage(0.5).is_none(), "another FE car or platform");
    game.interp.prev.garage = keep.clone();
    if let Some(v) = game.interp.prev.garage.view.as_mut() {
        v.disc = (v.disc + 1) % 3;
    }
    assert!(game.frame_garage(0.5).is_none(), "the car on another disc");
    game.interp.prev.garage = keep;
    if let Some(r) = game.garage_fe.request.as_mut() {
        r.now += 1.0;
    }
    assert!(game.frame_garage(0.5).is_none(), "a request made after the tick");
}

/// `render_garage` at α = 0 between ticks n − 1 and n draws tick n − 1's
/// picture pixel for pixel (the scene, the car, its reflection, shadow,
/// part poses and the particles), on a 120 Hz and a 200 Hz schedule, while
/// the camera blends to a part's view and the car turns; and frames
/// between the ticks differ from both ends.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn garage_images_at_alpha_zero_are_the_ticks() {
    testkit::game();
    super::test_isolation::exclusive();
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance())).unwrap();
    let (w, h) = (320u32, 240u32);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("garage between test"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let depth = create_depth(&gpu, w, h);
    let draw = |game: &mut Game, alpha: f32, drawn: Option<&GarageDrawn>| -> Vec<u8> {
        let settings =
            nfsu2::render::FrameSettings::new(&game.settings.custom, nfsu2::render::RenderOptions::in_force());
        let t = game.draw_clocks(alpha).seconds;
        let request = game.garage_fe.request.clone();
        assert!(render_garage(
            request,
            &game.game_data.root,
            &game.game_data.caches,
            &mut game.render.garage,
            &gpu,
            format,
            &view,
            &depth,
            (w, h),
            t,
            settings.display,
            &settings.opts,
            1,
            None,
            drawn,
        ));
        nfsu2::render::read_back(&gpu, &target, w, h)
    };
    // The 60 Hz harness's pictures: each tick's own (α = 1).
    let mut reference = garage_game();
    let base = reference.pacing.clocks.tick;
    reference.input_at(base + 20, InputEvent::press(KeyCode::Enter));
    reference.input_at(base + 21, InputEvent::KeyUp(KeyCode::Enter));
    let mut want = std::collections::HashMap::new();
    for _ in 0..50 {
        reference.advance(1.0 / 60.0).unwrap();
        let t = reference.pacing.clocks.tick - base;
        if (18..50).contains(&t) {
            want.insert(t, draw(&mut reference, 1.0, None));
        }
    }
    for hz in [120u32, 200] {
        let mut game = garage_game();
        let base = game.pacing.clocks.tick;
        game.input_at(base + 20, InputEvent::press(KeyCode::Enter));
        game.input_at(base + 21, InputEvent::KeyUp(KeyCode::Enter));
        let (mut exact, mut between) = (0, 0);
        for &u in &frames(Some(hz), 50 * hz as usize / 60) {
            let alpha = game.advance(u as f32 / nfsu2::frame_clock::UNITS as f32).unwrap();
            let t = game.pacing.clocks.tick - base;
            if !(20..49).contains(&t) {
                continue;
            }
            let d = game.frame_garage(alpha).expect("the garage blends while its view moves");
            let image = draw(&mut game, alpha, Some(&d));
            if alpha == 0.0 {
                assert!(
                    image == want[&(t - 1)],
                    "{hz} Hz: the α = 0 frame after tick {t} is not tick {}'s picture",
                    t - 1
                );
                exact += 1;
            } else if matches!((game.interp.prev.garage.view, game.interp.cur.garage.view), (Some(a), Some(b)) if a.car_angle != b.car_angle)
            {
                assert!(image != want[&(t - 1)] && image != want[&t], "{hz} Hz tick {t} α {alpha}: drawn at an end");
                between += 1;
            }
        }
        eprintln!(
            "{hz} Hz: {exact} α = 0 frames identical to the tick before, {between} frames between ticks differ from both ends"
        );
        // An α = 0 frame every tick at 120 Hz, every third at 200 Hz.
        assert!(exact >= 8 && between >= 10, "{hz} Hz: {exact} exact, {between} between");
    }
}
