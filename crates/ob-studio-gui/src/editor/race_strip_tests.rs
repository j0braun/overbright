//! The Race tab's strip on the straight test road (validate_tests.rs).

use glam::{Vec2, Vec3};

use super::*;
use crate::editor::races::preview;
use crate::editor::validate::tests::{sprint, straight};
use crate::tools::map::{MapMod, Spot};

fn checks_of(r: &CustomRace, net: &crate::formats::roads::RoadNet) -> Vec<Check> {
    let map = MapMod { races: vec![r.clone()], ..MapMod::default() };
    validate::race_checks(&map, 0, Some(net), &[], &validate::Tested::default())
}

#[test]
fn the_strip_lists_start_checkpoints_and_finish_with_each_legs_length() {
    let net = straight(false);
    let mut r = sprint(&[(20.0, 0.0), (180.0, 0.0), (380.0, 0.0)]);
    r.start = Some(Spot { position: Vec3::new(10.0, 0.0, 0.0), heading: 0.0 });
    r.finish = Some(Spot { position: Vec3::new(390.0, 0.0, 0.0), heading: 0.0 });
    let pv = preview(Some(&net), &r, &|_| 0.0);
    let st = strip(&r, 0, Some(&pv), &checks_of(&r, &net));
    assert_eq!(st.stops.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["S", "1", "2", "3", "F"]);
    assert_eq!(st.legs.len(), 4);
    assert_eq!(st.legs[0].state, LegState::Line);
    assert!((st.legs[0].metres - 10.0).abs() < 0.1);
    assert_eq!(st.legs[1].state, LegState::Road);
    assert!(st.legs[1].metres >= 160.0, "{:?}", st.legs[1]);
    // The "+" puts a checkpoint into the leg, in the middle of its road, as checkpoint 2 (index 1).
    let (index, at) = st.legs[1].insert.unwrap();
    assert_eq!(index, 1);
    assert!(at.x > 20.0 && at.x < 180.0, "{at}");
    assert_eq!(st.legs[2].insert.map(|i| i.0), Some(2));
    assert_eq!(st.legs[3].insert, None, "no checkpoint after the last one's line");
    assert_eq!(st.badge, Badge::NotTested);
}

#[test]
fn a_leg_without_road_is_broken_and_names_its_check() {
    let net = straight(true);
    let r = sprint(&[(20.0, 0.0), (180.0, 0.0), (380.0, 0.0)]);
    let pv = preview(Some(&net), &r, &|_| 0.0);
    let checks = checks_of(&r, &net);
    let st = strip(&r, 0, Some(&pv), &checks);
    assert_eq!(st.badge, Badge::CantRace);
    assert_eq!(st.legs[0].state, LegState::Road);
    let broken = &st.legs[1];
    assert_eq!(broken.state, LegState::NoRoad);
    assert!(checks[broken.broken.unwrap()].text.contains("race barrier"), "{checks:#?}");
}

#[test]
fn a_circuit_has_no_finish_and_a_leg_back_to_the_first() {
    let net = straight(false);
    let mut r = sprint(&[(20.0, 0.0), (180.0, 0.0), (380.0, 0.0)]);
    r.kind = crate::tools::map::RaceKind::Circuit;
    r.finish = Some(Spot { position: Vec3::new(390.0, 0.0, 0.0), heading: 0.0 });
    let pv = preview(Some(&net), &r, &|_| 0.0);
    let st = strip(&r, 0, Some(&pv), &[]);
    assert_eq!(st.stops.len(), 3);
    assert_eq!(st.legs.len(), 3, "two between the checkpoints and the way back");
    assert_eq!(st.legs[2].insert, None);
    assert!(st.legs[2].metres > 300.0, "{:?}", st.legs[2]);
}

#[test]
fn an_empty_race_has_an_empty_strip() {
    let r = sprint(&[]);
    let st = strip(&r, 0, None, &[]);
    assert!(st.stops.is_empty() && st.legs.is_empty());
}

#[test]
fn the_plus_is_an_op_one_undo_step() {
    let net = straight(false);
    let mut s = crate::editor::props::tests::state();
    s.session.map.races = vec![sprint(&[(20.0, 0.0), (380.0, 0.0)])];
    s.race = Some(0);
    crate::editor::commit(&mut s, "Races");
    s.nets.insert(4001, Some(net.clone()));
    let r = s.session.map.races[0].clone();
    let pv = preview(Some(&net), &r, &|_| 0.0);
    let st = strip(&r, 0, Some(&pv), &[]);
    let pick = Pick { insert: st.legs[0].insert, ..Pick::default() };
    take(&mut s, 0, &r, &pick, &[]);
    assert_eq!(s.session.map.races[0].checkpoints.len(), 3);
    assert!(s.session.map.races[0].checkpoints[1].x.round() > 100.0);
    assert_eq!(s.status, "Insert checkpoint 2");
    assert_eq!(s.point.map(|p| p.index), Some(1));
    assert!(crate::editor::undo::undo(&mut s));
    assert_eq!(s.session.map.races[0].checkpoints.len(), 2);
    let _ = Vec2::ZERO;
}

#[test]
fn clicking_a_stop_selects_that_point() {
    let mut s = crate::editor::props::tests::state();
    let r = sprint(&[(20.0, 0.0), (380.0, 0.0)]);
    s.session.map.races = vec![r.clone()];
    s.race = Some(0);
    let p = RacePoint { part: RacePart::Checkpoint, index: 1 };
    take(&mut s, 0, &r, &Pick { stop: Some(p), ..Pick::default() }, &[]);
    assert_eq!(s.point, Some(p));
    assert_eq!(s.mode, Mode::Races);
}

#[test]
fn the_tab_draws_at_the_two_shot_sizes() {
    let net = straight(false);
    for (w, h) in [(960.0, 200.0), (1920.0, 260.0)] {
        let mut s = crate::editor::props::tests::state();
        let mut r = sprint(&[(20.0, 0.0), (180.0, 0.0), (380.0, 0.0)]);
        r.start = Some(Spot { position: Vec3::new(10.0, 0.0, 0.0), heading: 0.0 });
        s.session.map.races = vec![r.clone()];
        s.race = Some(0);
        let pv = vec![preview(Some(&net), &r, &|_| 0.0)];
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h))),
            ..Default::default()
        };
        let mut used = egui::Rect::NOTHING;
        let mut out = ctx.run_ui(input, |ui| {
            tab(&mut s, &[], &pv, ui);
            used = ui.min_rect();
        });
        out.textures_delta.clear();
        assert!(used.height() < h, "the strip fits its tab at {w}: {used:?}");
    }
}
