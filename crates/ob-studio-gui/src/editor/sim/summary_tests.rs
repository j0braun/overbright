use glam::{Vec2, Vec3};

pub(crate) use ob_studio::runs::fixtures::{lines, race};
use super::*;

pub(crate) fn props() -> Vec<(String, Vec3)> {
    vec![("LAMP_POST (lamp1)".into(), Vec3::new(382.0, 3.0, 0.0))]
}

pub(crate) fn findings(rec: &Recording) -> RunFindings {
    let props = props();
    let road = |p: Vec2| p.y.abs();
    run_findings(rec, &Context { props: &props, road: Some(&road) })
}

#[test]
fn one_run_says_what_went_wrong_and_where() {
    let rec = race(1, true, false);
    let groups = summarize(&[findings(&rec)]);
    let texts: Vec<&str> = groups.iter().map(|g| g.text.as_str()).collect();
    assert!(texts.contains(&"Stuck before checkpoint 3: 1 car"), "{texts:?}");
    assert!(texts.contains(&"Crashed into prop LAMP_POST (lamp1) before checkpoint 2: 1 car"), "{texts:?}");
    assert!(texts.contains(&"Missed checkpoint 2: 1 car"), "{texts:?}");
    assert!(texts.contains(&"Missed checkpoint 3: 1 car"), "{texts:?}");
    assert!(texts.contains(&"Left the base track's roads before checkpoint 1: 1 car"), "{texts:?}");
    assert_eq!(texts.last(), Some(&"Finish spread: 8.0 s; 1 did not finish"));

    let stuck = groups.iter().find(|g| g.kind == Kind::Stuck).unwrap();
    let s = &stuck.items[0];
    // Stopped at 10 s, never moved again until the end at 60 s.
    assert_eq!(item_text(s, 1), "Civic: 50 s, never moved again at 0:14.0");
    assert_eq!((s.car, s.at.map(|p| p.x)), (1, Some(200.0)));

    let crash = groups.iter().find(|g| matches!(g.kind, Kind::CrashProp(_))).unwrap();
    assert_eq!(item_text(&crash.items[0], 1), "Golf: 36 km/h lost at 0:20.0");
    assert!((crash.items[0].time - 20.0).abs() < 1e-4);

    let missed = groups.iter().find(|g| g.kind == Kind::Missed && g.checkpoint == 2).unwrap();
    assert_eq!(missed.items[0].text, "Golf: passed over it at 0:25.0");
    let never = groups.iter().find(|g| g.kind == Kind::Missed && g.checkpoint == 3).unwrap();
    assert_eq!(never.items[0].text, "Civic: never reached it");

    let off = groups.iter().find(|g| g.kind == Kind::OffRoad).unwrap();
    assert_eq!(off.items[0].text, "Your slot (AI): 40 m off for 4 s");
    assert!(off.items[0].at.is_some_and(|p| p.y == 40.0));
    // Every finding has a place to show.
    assert!(groups.iter().flat_map(|g| &g.items).all(|f| f.at.is_some()));
}

#[test]
fn a_crash_away_from_props_and_cars_is_a_wall_and_one_at_the_grid_is_start_trouble() {
    let rec = race(1, false, true);
    let groups = summarize(&[findings(&rec)]);
    assert_eq!(groups[0].kind, Kind::Start, "start trouble first");
    assert_eq!(groups[0].items[0].text, "Golf: crashed 1.5 s after the green");
    // Racer 2's grid spot is 20 m behind the player's: within 5.5 m of nobody.
    assert!(groups.iter().any(|g| g.kind == Kind::CrashWall));
    assert!(!groups.iter().any(|g| matches!(g.kind, Kind::CrashProp(_))));
}

#[test]
fn several_runs_count_how_often_each_problem_happened() {
    let runs = [findings(&race(1, true, false)), findings(&race(2, false, false)), findings(&race(3, false, false))];
    let groups = summarize(&runs);
    let text = |k: &Kind, cp: u32| groups.iter().find(|g| &g.kind == k && g.checkpoint == cp).map(|g| g.text.clone());
    assert_eq!(text(&Kind::Stuck, 3).as_deref(), Some("Stuck before checkpoint 3: 3 of 3 runs, 3 times"));
    let prop = Kind::CrashProp("LAMP_POST (lamp1)".into());
    assert_eq!(
        text(&prop, 2).as_deref(),
        Some("Crashed into prop LAMP_POST (lamp1) before checkpoint 2: 1 of 3 runs, once")
    );
    assert_eq!(text(&Kind::Spread, 0).as_deref(), Some("Finish spread: 8.0 s; did not finish in 3 of 3 runs"));
    // The most frequent first; the occurrences by seed, each saying its seed.
    assert!(groups[0].runs == 3);
    let stuck = groups.iter().find(|g| g.kind == Kind::Stuck).unwrap();
    assert_eq!(stuck.items.iter().map(|f| f.seed).collect::<Vec<_>>(), vec![1, 2, 3]);
    assert!(item_text(&stuck.items[1], 3).starts_with("Seed 2, Civic: "));
}

/// Through the whole editor: two recorded runs loaded as `--sim a,b` give
/// the summary over both; the keyboard moves between its rows, each row
/// shows its run (the other seed's recording swapped in), time, car and spot,
/// and the Map centres there; Space on a row does not start the playback.
#[test]
fn the_summary_is_shown_and_walked_with_the_keyboard() {
    use crate::editor::app::App;
    use crate::editor::state::Mode;
    use crate::tools::map::assets::{GivenAssets, TrackInfo};
    use egui::{Event, Pos2, Rect};

    let dir = crate::testkit::Scratch::new("sim-summary");
    let mut files = Vec::new();
    for (seed, prop) in [(1, true), (2, false)] {
        let p = dir.path().join(format!("seed{seed}.run"));
        let text: String = lines(seed, prop, false).iter().map(|l| l.write() + "\n").collect();
        std::fs::write(&p, text).unwrap();
        files.push(p.display().to_string());
    }
    let mut a = App::new(dir.path().join("game"), dir.path().join("maps"), Box::new(|_, _| Ok(Default::default())));
    a.new_mod("r", "Races", "L4RA").unwrap();
    {
        let st = a.state.as_mut().unwrap();
        st.assets = Box::new(GivenAssets {
            tracks: vec![TrackInfo { number: 4001, name: "Test road".into(), modes: vec![], area: String::new() }],
            nets: [(4001, crate::editor::validate::tests::straight(false))].into(),
            ..Default::default()
        });
        st.session.map.races = vec![crate::editor::validate::tests::sprint(&[(20.0, 0.0), (380.0, 0.0)])];
        st.session.map.props.push(crate::tools::map::format::PropEdit::Add {
            id: "lamp1".into(),
            region: "L4RA".into(),
            solid: "LAMP_POST".into(),
            position: props()[0].1,
            rotation: glam::Mat3::IDENTITY,
        });
        st.race = Some(0);
        st.mode = Mode::Races;
        a.panels.viewport.set_look(st, "top");
    }
    a.tab = super::super::TAB;
    let frame = |a: &mut App, events: Vec<Event>| {
        let _ = a.frame(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 1000.0))),
            events,
            ..Default::default()
        });
    };
    let key = |k: egui::Key| {
        let e =
            |pressed| Event::Key { key: k, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
        vec![e(true), e(false)]
    };
    super::super::panel::load_recording(&a.ctx, &files.join(","));
    for _ in 0..3 {
        frame(&mut a, vec![]);
    }
    let sim = a.panels.sim.clone();
    {
        let s = sim.borrow();
        assert_eq!(s.batch.len(), 2);
        assert_eq!(s.list, super::super::panel::List::Summary, "a run with problems opens on them");
        assert_eq!(s.rec.header.as_ref().unwrap().seed, 2, "the last run is shown");
        assert!(s.row_ids.len() > 4, "rows drawn");
    }
    // The row before the prop crash's occurrence (seed 1 only) has the focus; Down moves to it.
    let k = {
        let s = sim.borrow();
        let mut k = 0;
        let mut at = None;
        for g in &s.groups {
            if matches!(g.kind, Kind::CrashProp(_)) {
                at = Some(k + 1);
            }
            k += 1 + g.items.len().min(SHOWN);
        }
        at.expect("a prop crash group")
    };
    let id = sim.borrow().row_ids[k - 1];
    a.ctx.memory_mut(|m| m.request_focus(id));
    frame(&mut a, vec![]);
    frame(&mut a, key(egui::Key::ArrowDown));
    frame(&mut a, vec![]);
    {
        let s = sim.borrow();
        assert_eq!(s.pick, Some(k));
        assert_eq!(s.rec.header.as_ref().unwrap().seed, 1, "the crash's own run is shown");
        assert!((s.play.time - 20.0).abs() < 0.3, "{}", s.play.time);
        assert_eq!(s.play.follow, Some(2));
        assert!(!s.play.playing);
    }
    let (centre, _, _) = super::super::panel::map_transform(&a.ctx).unwrap();
    assert!(centre.distance(Vec2::new(380.0, 0.0)) < 1.0, "the Map centres on the crash: {centre}");
    // Space on a row is the row's, not Play.
    frame(&mut a, key(egui::Key::Space));
    assert!(!sim.borrow().play.playing);
    // Up and Enter: the row above (the group's line) shows its first occurrence.
    frame(&mut a, key(egui::Key::ArrowUp));
    frame(&mut a, vec![]);
    assert_eq!(sim.borrow().pick, Some(k - 1));
    frame(&mut a, key(egui::Key::Enter));
    frame(&mut a, vec![]);
    assert_eq!(sim.borrow().pick, Some(k - 1));
}
