use ob_formats::formats::visibility::{DrivableBoundary, VisibleSections};

use super::*;

fn square(section: u32, x: f32) -> DrivableBoundary {
    let (lo, hi) = (Vec2::new(x, 0.0), Vec2::new(x + 100.0, 100.0));
    DrivableBoundary {
        section,
        bbox_min: lo,
        bbox_max: hi,
        points: vec![lo, Vec2::new(hi.x, lo.y), hi, Vec2::new(lo.x, hi.y)],
    }
}

/// Section 101 (x 0..100) draws 101 and 102; section 102 (x 100..200) draws only itself; 103 (x 1000..) draws all.
fn vis() -> Visibility {
    Visibility {
        boundaries: vec![square(101, 0.0), square(102, 100.0), square(103, 1000.0)],
        lists: vec![
            VisibleSections { section: 101, visible: vec![101, 102] },
            VisibleSections { section: 102, visible: vec![102] },
            VisibleSections { section: 103, visible: vec![101, 102, 103] },
        ],
        detail_limit: 20,
    }
}

fn prop(section: u32, at: Vec3, was: Option<Vec3>, added: bool) -> Prop {
    Prop { name: "XO_Cone".into(), added, at, was, section, radius: 6.0, solid: 1, empty: false }
}

#[test]
fn a_prop_out_of_sight_of_the_roads_that_do_not_draw_it_is_fine() {
    // Section 101's prop in the middle of 101: 102 (which does not draw 101) is 50 m away, so it pops in from there.
    let p = prop(101, Vec3::new(50.0, 50.0, 0.0), None, true);
    assert_eq!(popin(&[p], &vis()).len(), 1);
    // Far from every section that does not list it (103's own list has it).
    let p = prop(101, Vec3::new(-300.0, 50.0, 0.0), None, true);
    assert!(popin(&[p], &vis()).is_empty());
}

#[test]
fn the_sight_of_a_prop_grows_with_its_size() {
    assert!(sight(1.0) < sight(6.0));
    assert!(sight(60.0) <= MAX_SIGHT);
    // A tiny prop is not seen from the neighbour 50 m away; a large one is.
    let mut p = prop(101, Vec3::new(50.0, 50.0, 0.0), None, true);
    p.radius = 1.0;
    assert!(popin(&[p.clone()], &vis()).is_empty(), "sight {}", sight(1.0));
    p.radius = 6.0;
    assert_eq!(popin(&[p], &vis()).len(), 1);
}

#[test]
fn a_moved_prop_is_reported_only_for_the_roads_it_newly_pops_from() {
    // Moved within 101: 102 saw neither place as drawn, so nothing is new.
    let p = prop(101, Vec3::new(60.0, 50.0, 0.0), Some(Vec3::new(40.0, 50.0, 0.0)), false);
    assert!(popin(&[p], &vis()).is_empty());
    // Moved from far away (out of 102's sight) to beside it: new.
    let p = prop(101, Vec3::new(60.0, 50.0, 0.0), Some(Vec3::new(-300.0, 50.0, 0.0)), false);
    let c = popin(&[p], &vis());
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].level, Level::Warning);
    assert!(c[0].text.contains("moved prop") && c[0].text.contains("102"), "{}", c[0].text);
    assert_eq!(c[0].at, Some(Vec2::new(60.0, 50.0)), "Go to takes the camera to the prop");
}

#[test]
fn the_list_is_capped_with_a_count() {
    let ps: Vec<Prop> = (0..MAX_LISTED + 5).map(|_| prop(101, Vec3::new(50.0, 50.0, 0.0), None, true)).collect();
    let c = popin(&ps, &vis());
    assert_eq!(c.len(), MAX_LISTED + 1);
    assert!(c.last().unwrap().text.starts_with("5 more"));
}

#[test]
fn an_added_model_no_drawn_section_has_is_named() {
    let p = prop(101, Vec3::new(50.0, 50.0, 0.0), None, true);
    // The model is placed in section 102 (drawn with 101 from 101 and 103, not from 102).
    let have: Models = [(1, 102)].into();
    let c = models(&[p.clone()], &vis(), &have);
    assert_eq!(c.len(), 0, "102 draws only 102, so 102's list holds the model but not 101: 101's roads with 102 see it");
    // Placed nowhere near: error, every road that draws 101 lacks it.
    let c = models(&[p.clone()], &vis(), &Models::new());
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].level, Level::Error);
    // Placed in 101 itself: loaded everywhere 101 draws.
    assert!(models(&[p.clone()], &vis(), &[(1, 101)].into()).is_empty());
    // A moved original is never judged (its model came with it).
    let moved = Prop { added: false, ..p };
    assert!(models(&[moved], &vis(), &Models::new()).is_empty());
}

#[test]
fn an_added_prop_without_a_mesh_is_an_error() {
    let p = Prop { empty: true, ..prop(101, Vec3::ZERO, None, true) };
    let c = models(&[p], &vis(), &Models::new());
    assert_eq!((c.len(), c[0].level), (1, Level::Error));
}
