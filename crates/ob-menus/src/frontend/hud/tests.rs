use super::*;

/// 0x4C6B80 → 0x4AA160: 104° at no boost, −14° (written 374°) at −1,
/// 222° at 1; the drag range runs backwards.
#[test]
fn board_rows_window() {
    assert_eq!(board_rows(4, 6, 4), vec![0, 3, 4, 5]);
    assert_eq!(board_rows(5, 6, 4), vec![0, 3, 4, 5]);
    assert_eq!(board_rows(0, 6, 4), vec![0, 1, 2, 3]);
    assert_eq!(board_rows(2, 6, 4), vec![0, 1, 2, 3]);
    assert_eq!(board_rows(3, 4, 4), vec![0, 1, 2, 3]);
    assert_eq!(board_rows(0, 2, 4), vec![0, 1]);
    assert_eq!(board_rows(3, 6, 4), vec![0, 2, 3, 4]);
}

/// 0x6000A0: the crash / jump camera (event kinds 6, 7) and the replay
/// key camera (kind 1) give no HUD at all — observed under Wine, the
/// whole HUD gone for the crash camera's span.
#[test]
fn event_cameras_hide_the_hud() {
    let mut input = Input { racing: true, clock: 10.0, ..Default::default() };
    input.options.gauges = true;
    input.options.map = true;
    assert_ne!(features(&input), (0, 0));
    input.event_camera = true;
    assert_eq!(features(&input), (0, 0));
}

#[test]
fn turbo_needle_range() {
    assert_eq!(turbo_needle(0.0, false), 104.0);
    assert_eq!(turbo_needle(1.0, false), 222.0);
    assert_eq!(turbo_needle(-1.0, false), 374.0);
    assert_eq!(turbo_needle(0.0, true), 127.0);
}

#[test]
fn done_racers_show_their_status() {
    assert_eq!(status_label(0), FINISHED);
    assert_eq!(status_label(3), 0x5D82_DBA2);
    assert_eq!(status_label(8), 0x01E6_6364);
    assert_eq!(status_label(9), 0xB7B7_5185);
    assert_eq!(status_label(10), FINISHED);
}

#[test]
fn times_print_like_the_game() {
    assert_eq!(time_text(0), "0.00");
    assert_eq!(time_text(ticks(0.64)), "0.64");
    assert_eq!(time_text(1200), "0.30");
    assert_eq!(time_text(ticks(5.25)), " 5.25");
    assert_eq!(time_text(ticks(83.5)), " 1:23.50");
    assert_eq!(time_text(ticks(600.0)), "10:00.00");
}

#[test]
fn gaps_print_signed() {
    assert_eq!(signed_time_text(ticks(-5.25)), "-5.25");
    assert_eq!(signed_time_text(ticks(12.5)), "+12.50");
    assert_eq!(signed_time_text(0), "+0.00");
    assert_eq!(nth(1), 0xBC4C_E12F);
}

#[test]
fn nitrous_gauge_only_with_a_tank_outside_drift_and_street_x() {
    let race = Input { racing: true, racers: 2, ..Default::default() };
    assert_eq!(features(&race).1 & 0x4_0000, 0);
    let nos = Input { nitrous: Some(0.5), ..race };
    assert_eq!(features(&nos).1 & 0x4_0000, 0x4_0000);
    for mode in [crate::race::RaceMode::Drift, crate::race::RaceMode::StreetX] {
        let i = Input { nitrous: Some(0.5), race: mode, racing: true, ..Default::default() };
        assert_eq!(features(&i).1 & 0x4_0000, 0);
    }
}

#[test]
fn rear_view_mirror_only_while_driving() {
    // Free roam and a running race ask for HUD_FEATURE_RVM (0x6004B7).
    let roam = Input { rear_view: true, ..Default::default() };
    assert_ne!(features(&roam).0 & 0x1000, 0);
    let race = Input { rear_view: true, racing: true, racers: 2, clock: 3.0, ..Default::default() };
    assert_ne!(features(&race).0 & 0x1000, 0);
    // Not before the green light, nor at the race over, nor without
    // the option's test.
    assert_eq!(features(&Input { clock: -1.0, ..race.clone() }).0 & 0x1000, 0);
    assert_eq!(features(&Input { finished: Some(60.0), ..race.clone() }).0 & 0x1000, 0);
    assert_eq!(features(&Input { rear_view: false, ..race }).0 & 0x1000, 0);
}

#[test]
fn free_roam_shows_the_nitrous_gauge_with_a_tank() {
    let roam = Input { career: true, ..Default::default() };
    assert_eq!(features(&roam).1 & 0x4_0000, 0);
    let nos = Input { nitrous: Some(1.0), ..roam.clone() };
    assert_ne!(features(&nos).1 & 0x4_0000, 0);
    let off = Input { options: HudOptions { gauges: false, ..Default::default() }, ..nos };
    assert_eq!(features(&off).1 & 0x4_0000, 0);
}

#[test]
fn single_racer_hides_the_position() {
    let race = Input { racing: true, racers: 1, ..Default::default() };
    assert_eq!(features(&race).0 & 0x200, 0);
    let done = Input { finished: Some(60.0), ..race };
    assert_eq!(features(&done), (0x4, 0x2_0000));
}

#[test]
fn an_outrun_shows_the_getaway_meter_and_no_leader_board() {
    // 0x5F33C0 in race state 7: high 0x40, no 0x80_0000; the position
    // with the two racers left.
    let outrun = Input { racing: true, racers: 2, outrun: Some(OutrunHud::default()), ..Default::default() };
    let (lo, hi) = features(&outrun);
    assert_eq!((hi & 0x40, lo & 0x80_0000, lo & 0x200), (0x40, 0, 0x200));
    let race = Input { racing: true, racers: 2, ..Default::default() };
    assert_eq!(features(&race).1 & 0x40, 0);
    // Its race-over message back in free roam: the element alone.
    let over = Input { finished: Some(20.0), outrun_result: Some((true, Some(100))), ..Default::default() };
    assert_eq!(features(&over), (0x4, 0));
}

/// 0x4AB210 with the 206's angles (record +0x7FC / +0x800 = −43 / 43,
/// read in the original's memory): 0 rpm −43°, the maximum 43°, a
/// wrap when the end is not past the start.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drag_shift_light_pulse_ends_in_package_time() {
    // audit 2o-002: HUD_Drag's Shift_light pulses run 600 package
    // ms, which the package manager (0x558C10, × 1200 / s) plays in 500
    // real ms; the HUD's "still running" test (0x51D050, package time)
    // must end them there, not at 600 real ms (a 100 ms dark gap).
    let root = crate::testkit::game_root();
    let assets = Assets::load(&root).unwrap();
    let package = assets.package("HUD_Drag").unwrap();
    let light = fe_hash("Shift_light");
    for pulse in ["PulseRed", "PulseBlue", "PulseGreen"] {
        let package_ms = package
            .objects
            .iter()
            .find(|o| o.hash == light)
            .and_then(|o| o.scripts.iter().find(|s| s.name == fe_hash(pulse)))
            .map(|s| s.length)
            .unwrap();
        assert_eq!(package_ms, 600, "{pulse}");
        assert!((script_length(package, light, pulse) - 500.0).abs() < 1e-3, "{pulse}");
    }
    let message = fe_hash("ShiftMessage");
    let zoom = package
        .objects
        .iter()
        .find(|o| o.hash == message)
        .and_then(|o| o.scripts.iter().find(|s| s.name == fe_hash("ZoomIn")))
        .map(|s| s.length)
        .unwrap();
    // ZoomIn 600 package ms: natively "Over Rev!" shows 30 frames (500 ms).
    assert_eq!(zoom, 600);
    assert!((script_length(package, message, "ZoomIn") - 500.0).abs() < 1e-3);
}

#[test]
fn drag_needle_sweeps_the_records_angles() {
    assert_eq!(drag_needle_angle(0.0, 7000.0, [-43.0, 43.0]), -43.0);
    assert_eq!(drag_needle_angle(7000.0, 7000.0, [-43.0, 43.0]), 43.0);
    assert_eq!(drag_needle_angle(3500.0, 7000.0, [-43.0, 43.0]), 0.0);
    assert_eq!(drag_needle_angle(9000.0, 7000.0, [-43.0, 43.0]), 43.0);
    assert_eq!(drag_needle_angle(3500.0, 7000.0, [300.0, 20.0]), 340.0);
}

/// The engage prompt's queue (0x4C9170 / 0x4DAB80 / 0x4DA9A0 /
/// 0x4C8AC0): a marker's prompt flashes at the first update with
/// FE_SOUND_ENGAGE_PHONE (id 14), steps to state 3 after 1 s with
/// FE_SOUND_ENGAGE_DPAD (id 15) and never rests (0x4C8B80 sets 4 back
/// to 3, so nothing more plays); a new SMS waits under it, flashes
/// with FE_SOUND_ENGAGE_SMS (id 16) when the marker is left, rests
/// after 4 s and flashes again from rest on the next message.
#[test]
fn engage_prompt_queue() {
    use crate::audio::fe::{control as c, event::FOCUS};
    let mut p = Prompts::default();
    let mut s = ScreenState::default();
    let mut step = |p: &mut Prompts, now: f32| {
        p.update(&mut s, 8, false, now);
        p.sounds.drain(..).map(|(_, c)| c).collect::<Vec<_>>()
    };
    p.queue(0xC, 0, 0.0);
    assert_eq!(p.prompts[0].state, 1);
    assert_eq!(step(&mut p, 0.0), [c::ENGAGE_PHONE]);
    assert!(step(&mut p, 900.0).is_empty());
    assert_eq!(step(&mut p, 1100.0), [c::ENGAGE_DPAD]);
    for t in [4100.0, 6000.0, 20000.0] {
        assert!(step(&mut p, t).is_empty());
        assert_eq!(p.prompts[0].state, 3, "a marker's prompt never rests");
    }
    // An SMS arrives: queued under the marker (priority 2 < 4).
    p.queue(0x13, 1, 20000.0);
    assert!(step(&mut p, 20100.0).is_empty());
    assert_eq!(p.prompts[1].state, 1);
    // The marker is left: the SMS flashes.
    p.drop_kind(0xC);
    assert_eq!(p.dropped.len(), 1);
    assert_eq!(step(&mut p, 21000.0), [c::ENGAGE_SMS]);
    assert_eq!(step(&mut p, 22100.0), [c::ENGAGE_DPAD]);
    assert!(step(&mut p, 25100.0).is_empty());
    assert_eq!(p.prompts[1].state, 4, "the SMS rests after 4 s");
    // Another message: from rest straight back to flashing, at the queue.
    p.queue(0x13, 2, 26000.0);
    assert_eq!(p.sounds, [(FOCUS, c::ENGAGE_SMS)]);
    p.sounds.clear();
    // A marker entered while the SMS flashes takes over: the SMS rests.
    p.queue(0xB, 0, 26100.0);
    assert_eq!(step(&mut p, 26100.0), [c::ENGAGE_PHONE]);
    assert_eq!(p.prompts[1].state, 4);
    // The encounter's prompt (priority 3) waits under a marker's, and a
    // marker's takes over the encounter's, which ends (freed).
    p.drop_kind(0xB);
    p.queue(9, 0, 27000.0);
    assert_eq!(step(&mut p, 27000.0), [c::ENGAGE_PHONE]);
    p.queue(1, 0, 27500.0);
    assert_eq!(step(&mut p, 27500.0), [c::ENGAGE_PHONE]);
    assert!(p.prompts.iter().all(|x| x.kind != 9), "the encounter's slot freed");
    // The D-Day prompt (0x14) is silent; its state 2 lasts its value.
    let mut q = Prompts::default();
    q.queue(0x14, 2, 0.0);
    assert!(step(&mut q, 0.0).is_empty());
    assert!(step(&mut q, 1500.0).is_empty());
    assert_eq!(q.prompts[0].state, 2);
    step(&mut q, 2100.0);
    assert_eq!(q.prompts[0].state, 3);
}

/// 0x4C7E20 leaves +0x24 unwritten and the heap hands a freed popup's
/// block to the next one: a popup built after a
/// faded-out one was freed starts with +0x24 = 5 (a one-off then does not
/// fade on its second showing); one built on a never-used block with None.
#[test]
fn a_new_popup_inherits_the_freed_blocks_state() {
    use crate::drift::{TrickReport, TrickState};
    let trick = |index| TrickReport {
        index,
        name: 0x1234,
        continuous: false,
        state: TrickState::Running,
        points: 100.0,
        multiplier: 1.0,
    };
    let mut pp = super::popups::Popups::default();
    pp.queue(&[trick(1)]);
    assert_eq!(pp.queue[0].state, None, "a never-used block");
    pp.queue[0].state = Some(5);
    pp.free(0);
    pp.queue(&[trick(2)]);
    assert_eq!(pp.queue[0].state, Some(5), "the faded popup's block, last freed first");
    pp.queue[0].state = Some(0);
    pp.free(0);
    pp.queue(&[trick(3), trick(4)]);
    assert_eq!((pp.queue[0].state, pp.queue[1].state), (Some(0), None));
}

/// 0x4EBB10: "Hurry!" queues in the update whose time left is within the
/// world step (0x865150, 0 while paused) above an even second. The step is
/// the hurry clock's own move, so at 60 fps the 1/60 s tick (as native), and
/// at any draw rate exactly one update fires per even second; a redraw with
/// no tick (high rates, a pause) never does.
#[test]
fn hurry_fires_once_per_even_second_at_any_draw_rate() {
    use super::race_info::hurry_due;
    for draws_per_tick in [1usize, 2, 4] {
        let mut fired = Vec::new();
        let mut prev = None;
        let mut t = 30.5f32;
        for _ in 0..(3 * 60) {
            t -= 1.0 / 60.0;
            for _ in 0..draws_per_tick {
                if hurry_due(prev, t) {
                    fired.push(t as i32);
                }
                prev = Some(t);
            }
        }
        assert_eq!(fired, vec![30, 28], "{draws_per_tick} draws a tick");
    }
    // A long pause: the clock did not move, nothing fires.
    assert!(!hurry_due(Some(28.001), 28.001));
}

/// The pause tick (traced): under the pause
/// the HUD update runs with the pause features (low 0x40000, high 0); the
/// outrun meter (0x4DABD0, high 0x40) turns its group INACTIVE and clears
/// its shown-frame count +0x44, except in an outrun (race type 7).
#[test]
fn pause_tick_resets_the_outrun_meter_outside_an_outrun() {
    for (outrun, kept) in [(false, 0u16), (true, 4)] {
        let mut hud = Hud { meter_frames: 4, outrun_message: Some((1, 0.0)), ..Default::default() };
        hud.groups.insert(fe_ids::OBJ_8CA76279, "ACTIVE");
        hud.pause_step(1000.0, outrun);
        assert_eq!(hud.features(), (0x4_0000, 0));
        assert_eq!(hud.meter_frames, kept, "outrun {outrun}");
        assert_eq!(hud.outrun_message, None);
        assert_eq!(hud.groups.get(&fe_ids::OBJ_8CA76279), Some(&"INACTIVE"));
        assert_eq!(hud.state.scripts.get(&fe_ids::OBJ_8CA76279).map(|s| s.1), Some(1000.0));
    }
}

/// `layout` is a read of the state `step` left: laid out twice,
/// the same quads, and the same as `quads` (step + layout).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn layout_reads_the_last_step() {
    let assets = Assets::load(&crate::testkit::game_root()).unwrap();
    let mut stepped = Hud::default();
    let mut input = Input { racing: true, racers: 4, clock: 3.0, speed: 30.0, rpm: 5000.0, ..Default::default() };
    input.options.map = true;
    input.options.gauges = true;
    for k in 0..120 {
        let now = k as f32 * 16.0;
        input.clock = 3.0 + k as f32 / 60.0;
        let whole = stepped.quads(&assets, &input, now);
        let dump = |h: &Hud| format!("{:?} {:?} {:?} {:?}", h.state, h.features, h.groups, h.race);
        let before = dump(&stepped);
        let a = stepped.layout(&assets, now);
        let b = stepped.layout(&assets, now);
        assert_eq!(format!("{whole:?}"), format!("{a:?}"), "step {k}");
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "step {k}");
        assert_eq!(before, dump(&stepped), "step {k}: the layout wrote state");
    }
}

/// The drift angle readout as 0x4F6C00 builds it: without a turbo the
/// round-ended plate at the tachometer (Drift_Angle_Backing3) with the
/// texts moved onto it ("NoTurbo"); with one the plate left of the turbo
/// gauge (Drift_Angle_Backing1, "Turbo"). Never both plates.
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn drift_angle_backing_follows_the_turbo() {
    let assets = Assets::load(&crate::testkit::game_root()).unwrap();
    let (b1, b3) = (fe_hash("Drift_Angle_Backing1"), fe_hash("Drift_Angle_Backing3"));
    for turbo in [false, true] {
        let mut hud = Hud::default();
        let race = crate::race::RaceMode::Drift;
        let mut input = Input { race, racing: true, racers: 4, turbo, ..Default::default() };
        input.options.gauges = true;
        hud.quads(&assets, &input, 0.0);
        let (shown, gone) = if turbo { (b1, b3) } else { (b3, b1) };
        assert!(!hud.state.hidden.contains(&shown), "turbo {turbo}: its backing shows");
        assert!(hud.state.hidden.contains(&gone), "turbo {turbo}: the other backing stays hidden");
        let script = if turbo { "Turbo" } else { "NoTurbo" };
        for t in DRIFT_ANGLE_TEXTS {
            assert_eq!(hud.state.scripts.get(&fe_hash(t)).map(|s| s.0), Some(fe_hash(script)), "{t}");
        }
        // The same frame again keeps them (built once, as the constructor).
        let quads = hud.quads(&assets, &input, 500.0);
        assert_eq!(hud.state.scripts.get(&fe_hash("Angle_degrees")).map(|s| s.1), Some(0.0));
        // On the screen the plate stays joined to the tachometer at every
        // common size and HUD layout: same anchor, the plate's right end
        // under the dial (as at 4:3), never pulled apart: its distance from
        // the needle's hub in screen heights is the canvas's.
        let tex = |n: &str| assets.texture_for(n).unwrap_or_else(|| panic!("no texture {n}"));
        let find = |t: u32| quads.iter().filter(|q| q.texture == Some(t)).collect::<Vec<_>>();
        let plates = find(tex(if turbo { "Drift_Angle_Backing1.tga" } else { "Drift_Angle_Backing3.tga" }));
        assert!(find(tex(if turbo { "Drift_Angle_Backing3.tga" } else { "Drift_Angle_Backing1.tga" })).is_empty());
        assert_eq!(plates.len(), 1);
        let plate = plates[0];
        // The tachometer's group (the needle's parent) and where it goes.
        let package = assets.package(package_for(input.race)).unwrap();
        let needle = package.objects.iter().find(|o| o.hash == fe_hash("3rdPersonNeedle")).unwrap();
        let tach = package.objects.iter().find(|o| Some(o.guid) == needle.parent).unwrap();
        let place = crate::frontend::enh::widescreen::element_place(package, tach, &hud.state, glam::Vec2::ZERO);
        assert_eq!(plate.place, place, "the plate moves with the tachometer");
        let hub = tach.attributes.position().truncate() + crate::frontend::feng::state::CANVAS * 0.5;
        let gap = |f: &crate::render::display::UiFrame| {
            let end = f.to_pixels(plate.rect[0], plate.place).x.max(f.to_pixels(plate.rect[1], plate.place).x);
            f.to_pixels(hub, place).x - end
        };
        let canvas = gap(&crate::render::display::UiFrame::with_hud((640, 480), false)) / 480.0;
        let sizes = [(1024, 768), (1280, 960), (1920, 1080), (1366, 768), (1920, 1200), (2560, 1080), (3440, 1440)];
        for size in sizes.into_iter().chain([(3840, 2160)]) {
            for full in [false, true] {
                let g = gap(&crate::render::display::UiFrame::with_hud(size, full)) / size.1 as f32;
                assert!((g - canvas).abs() < 0.01, "turbo {turbo} {size:?} full {full}: gap {g} vs {canvas}");
            }
        }
    }
}

/// A package holding the minimap's frame and one marker object each.
fn minimap_package(markers: &[&str]) -> crate::frontend::feng::Package {
    let object = |hash: u32, size: f32| {
        let mut attributes = crate::frontend::feng::Attributes { words: [0; 37], len: 37 };
        attributes.words[14] = size.to_bits();
        attributes.words[15] = size.to_bits();
        crate::frontend::feng::Object {
            kind: crate::frontend::feng::Kind::Image,
            hash,
            guid: 0,
            flags: 0,
            resource: 0,
            parent: None,
            attributes,
            image_flags: 0,
            text: String::new(),
            label: None,
            justify: 0,
            leading: 0,
            max_width: 0,
            scripts: Vec::new(),
            responses: Vec::new(),
            messages: Vec::new(),
        }
    };
    let mut objects = vec![object(fe_hash("Track2000_map"), 100.0)];
    objects.extend(markers.iter().map(|n| object(fe_hash(n), 10.0)));
    crate::frontend::feng::Package { name: String::new(), path: String::new(), resources: Vec::new(), objects, buttons: Vec::new() }
}

fn minimap_view(icons: Vec<(&'static str, glam::Vec2, Option<f32>, bool)>) -> MapView {
    MapView { texture: Some(1), size: 1000.0, zoom: 1.0, range: 1.0, icons, ..Default::default() }
}

/// 0x4AC460, called per item by 0x4CA840: an event marker whose event is a
/// sponsor race (+0x34 = 1) takes the cross texture 0xB90F50BB (the World
/// Map's), the others keep the object's own image.
#[test]
fn sponsor_races_take_the_cross_on_the_minimap() {
    let package = minimap_package(&["CIRCUITEVENTINDICATOR_0", "CIRCUITEVENTINDICATOR_1"]);
    let at = glam::Vec2::new(10.0, 0.0);
    let view = minimap_view(vec![
        ("CIRCUITEVENTINDICATOR_", at, None, true),
        ("CIRCUITEVENTINDICATOR_", at, None, false),
    ]);
    let mut s = ScreenState::default();
    minimap(&mut s, &package, &|_| true, &view);
    assert_eq!(s.image.get(&fe_hash("CIRCUITEVENTINDICATOR_0")), Some(&fe_ids::BOX_OFF));
    assert_eq!(s.image.get(&fe_hash("CIRCUITEVENTINDICATOR_1")), None);
    // Without the cross texture in the archive the icon stays as it is.
    let mut s = ScreenState::default();
    minimap(&mut s, &package, &|_| false, &view);
    assert!(s.image.get(&fe_hash("CIRCUITEVENTINDICATOR_0")).is_none());
}

/// 0x4C8410: the GPS group 0x697D2BCA goes to the target only while the
/// GPS reads on (states 0-3); once the router is idle (arrival, a race's
/// start, deactivation: state 4) it stays hidden, and a minimap without a
/// GPS point hides it.
#[test]
fn gps_ring_stops_when_the_gps_reads_off() {
    use crate::frontend::career::Trigger;
    use glam::{Vec2, Vec3};
    let t = Trigger {
        name: 0,
        position: Vec3::new(5.0, 5.0, 0.0),
        gps: Vec3::ZERO,
        min: Vec2::ZERO,
        max: Vec2::ZERO,
        polygon: [Vec2::ZERO; 8],
        points: 0,
    };
    let mut gps = crate::frontend::dday::Gps::engage(1, &t, None);
    // No car, no search: reads off.
    assert_eq!(gps.state(), 4);
    assert_eq!(gps.ring_point(), None);
    // A found route: the ring sits at the target.
    gps.router.result = 2;
    assert_eq!(gps.state(), 3);
    assert_eq!(gps.ring_point(), Some(Vec2::new(5.0, 5.0)));
    // Arrival (0x42A360) and a race's start (0x541170) clear the router.
    gps.drop_route();
    assert_eq!(gps.state(), 4);
    assert_eq!(gps.ring_point(), None);
    // The minimap hides the group without a point and shows it with one.
    let package = minimap_package(&[]);
    let mut view = minimap_view(Vec::new());
    view.gps = gps.ring_point();
    let mut s = ScreenState::default();
    s.hidden.insert(fe_ids::GPS);
    minimap(&mut s, &package, &|_| true, &view);
    assert!(s.hidden.contains(&fe_ids::GPS) && !s.shown.contains(&fe_ids::GPS));
    gps.router.result = 2;
    view.gps = gps.ring_point();
    minimap(&mut s, &package, &|_| true, &view);
    assert!(s.shown.contains(&fe_ids::GPS));
}
