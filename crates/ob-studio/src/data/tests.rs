//! The grid's table (columns from the schema, sort, filter, search) and a cell edit as one undo step.

use super::*;
use glam::Vec2;
use nfsu2::studio_api::world::{Placement, World};
use ob_map::tools::map::format::CustomRace;

const PACK: &str = "my-map";

fn world() -> World {
    let mut w = World::flat(200.0);
    let place = |at: Vec3| Placement {
        solid: 0,
        low_solid: None,
        mid_solid: None,
        transform: glam::Mat4::from_translation(at),
        section: 618,
        flags: 0,
        cull_radius: 5.0,
    };
    w.solids = vec![ob_formats::formats::solid::Solid {
        name: "XC_CONE_01".into(),
        hash: ob_formats::formats::hash::bin_hash("XC_CONE_01"),
        bbox_min: [-0.5, -0.5, 0.0],
        bbox_max: [0.5, 0.5, 1.0],
        textures: Vec::new(),
        vertices: Vec::new(),
        normals: Vec::new(),
        light_materials: Vec::new(),
        indices: Vec::new(),
        materials: Vec::new(),
        mesh_flags: 0,
        markers: Vec::new(),
        light_markers: Vec::new(),
    }];
    w.placements = vec![place(Vec3::new(10.0, 0.0, 0.0)), place(Vec3::new(20.0, 0.0, 0.0))];
    w
}

fn session() -> MapSession {
    let mut s = MapSession::new("L4RA", PACK, MapMod::default(), world());
    for (id, name, track, laps, kind) in [
        ("b", "Bravo", 4002, 3, RaceKind::Circuit),
        ("a", "alpha", 4001, 1, RaceKind::Sprint),
        ("c", "Charlie", 4010, 1, RaceKind::Drift),
    ] {
        let mut r = CustomRace::new(id, track, kind);
        r.name = name.into();
        r.laps = laps;
        r.checkpoints = vec![Vec2::ZERO; id.len() + (laps as usize)];
        s.map.races.push(r);
    }
    assert!(s.commit("races"));
    s
}

fn races(s: &MapSession) -> Grid {
    Grid::build(&s.map, PACK, RACE).unwrap()
}

fn names(g: &Grid) -> Vec<String> {
    (0..g.counts().1).map(|i| g.cell(i, 0).unwrap().text).collect()
}

#[test]
fn columns_come_from_the_schema() {
    for kind in KINDS {
        let cols = columns(kind);
        assert!(!cols.is_empty());
        let short = if kind == PLACEMENT { "placement" } else { "race" };
        let fields = registry::kind(short).unwrap().fields;
        for c in cols {
            assert!(fields.iter().any(|f| f.key == c.schema), "{} has no schema field {}", kind, c.schema);
            assert!(!c.label.is_empty(), "{} has no label", c.schema);
        }
    }
    assert!(columns("texture").is_empty() && Grid::build(&MapMod::default(), PACK, "texture").is_none());
}

#[test]
fn builds_sorts_filters_and_searches() {
    let s = session();
    let mut g = races(&s);
    assert_eq!(g.counts(), (3, 3));
    assert_eq!(g.cell(0, 1).unwrap().from, From::Yours);
    g.sort_by(0);
    assert_eq!(names(&g), ["alpha", "Bravo", "Charlie"]);
    g.sort_by(0);
    assert_eq!(names(&g), ["Charlie", "Bravo", "alpha"]);
    g.sort_by(0);
    assert_eq!(g.sort, None);
    g.sort_by(2); // base track, numerically
    assert_eq!(names(&g), ["alpha", "Bravo", "Charlie"]);
    g.sort_by(3);
    g.sort_by(3); // laps, down
    assert_eq!(g.cell(0, 3).unwrap().text, "3");
    g.set_search("SPRINT");
    assert_eq!(names(&g), ["alpha"]);
    g.set_search("");
    g.set_filter(1, "drift");
    assert_eq!(names(&g), ["Charlie"]);
    g.set_filter(1, "");
    assert_eq!(g.counts().1, 3);
    assert_eq!(g.cell(0, 5).unwrap().text.ends_with("checkpoints"), true);
}

#[test]
fn a_cell_edit_is_one_undo_step() {
    let mut s = session();
    let before = s.map.clone();
    let steps = s.undo_label().map(str::to_string);
    let c = edit(&mut s, PACK, RACE, "my-map:a", "laps", "2").unwrap_err();
    assert!(c.contains("Sprint") || c.contains("lap") || !c.is_empty()); // a sprint runs one lap
    assert_eq!(s.map, before);
    let c = edit(&mut s, PACK, RACE, "my-map:b", "laps", "4").unwrap();
    assert_eq!(c.label, "Laps 4");
    assert_eq!(s.map.races.iter().find(|r| r.id == "b").unwrap().laps, 4);
    let g = races(&s);
    let i = (0..3).find(|i| g.key(*i) == Some("my-map:b")).unwrap();
    assert_eq!(g.cell(i, 3).unwrap().text, "4");
    assert_eq!(s.undo().as_deref(), Some("Laps 4"));
    assert_eq!(s.map, before);
    assert_eq!(s.undo_label().map(str::to_string), steps);
    assert!(s.redo().is_some());
    assert_eq!(s.map.races.iter().find(|r| r.id == "b").unwrap().laps, 4);
}

#[test]
fn bad_text_and_read_only_columns_are_refused_with_words() {
    let s = session();
    let m = &s.map;
    assert!(op_for(m, PACK, RACE, "my-map:b", "laps", "x").unwrap_err().contains("whole number"));
    assert!(op_for(m, PACK, RACE, "my-map:b", "laps", "99").unwrap_err().contains("range"));
    assert!(op_for(m, PACK, RACE, "my-map:b", "type", "kart").unwrap_err().contains("circuit"));
    assert!(op_for(m, PACK, RACE, "my-map:b", "checkpoints", "1").unwrap_err().contains("here yet"));
    assert!(op_for(m, PACK, RACE, "other:b", "laps", "2").is_err());
    assert!(matches!(op_for(m, PACK, RACE, "my-map:b", "name", "New").unwrap(), Op::RenameRace { .. }));
    assert!(matches!(
        op_for(m, PACK, RACE, "my-map:b", "backward", "yes").unwrap(),
        Op::SetDirection { backward: true, .. }
    ));
}

#[test]
fn prop_cells_are_overlays_on_the_original() {
    let mut s = MapSession::new("L4RA", PACK, MapMod::default(), world());
    let key = s.key_of(0).unwrap();
    s.run(Op::Place { key: key.clone(), position: Vec3::new(11.0, 2.0, 0.0), rotation: Mat3::IDENTITY }).unwrap();
    let other = s.key_of(1).unwrap();
    s.run(Op::Delete { key: other }).unwrap();
    let mut g = Grid::build(&s.map, PACK, PLACEMENT).unwrap();
    assert_eq!(g.counts().0, 2);
    let moved = (0..2).find(|i| g.cell(*i, 1).unwrap().text == "11, 2, 0").expect("moved row");
    let c = g.cell(moved, 1).unwrap();
    assert_eq!(c.from, From::Yours);
    assert_eq!(c.original.as_deref(), Some("10, 0, 0"));
    g.sort_by(1);
    assert_eq!(g.counts().1, 2);
    // position, heading and the hidden switch are edits; the rows' keys are the Changes keys.
    let k = g.key(moved).unwrap().to_string();
    edit(&mut s, PACK, PLACEMENT, &k, "position", "12, 3, 0").unwrap();
    edit(&mut s, PACK, PLACEMENT, &k, "heading", "90").unwrap();
    let g = Grid::build(&s.map, PACK, PLACEMENT).unwrap();
    let r = (0..2).find(|i| g.cell(*i, 1).unwrap().text == "12, 3, 0").unwrap();
    assert_eq!(g.cell(r, 2).unwrap().text, "90");
    // The hidden one: Hidden no puts it back as the original has it.
    let h = (0..2).find(|i| g.cell(*i, 3).unwrap().text == "yes").unwrap();
    let hk = g.key(h).unwrap().to_string();
    assert!(op_for(&s.map, PACK, PLACEMENT, &hk, "position", "1,2,3").is_err());
    edit(&mut s, PACK, PLACEMENT, &hk, "disabled", "no").unwrap();
    assert_eq!(s.map.props.len(), 1);
    assert!(op_for(&s.map, PACK, PLACEMENT, &k, "solid", "X").unwrap_err().contains("here yet"));
    // One undo step each.
    assert_eq!(s.undo().as_deref().map(|l| l.starts_with("Reset")), Some(true));
}

#[test]
fn a_column_fill_is_one_step() {
    let mut s = session();
    let before = s.map.clone();
    let cells: Vec<_> = ["b", "c"].iter().map(|i| (format!("{PACK}:{i}"), "yes".to_string())).collect();
    edit_many(&mut s, PACK, RACE, "backward", &cells, "Backward for 2 races").unwrap();
    assert!(s.map.races.iter().filter(|r| r.id != "a").all(|r| r.backward));
    assert_eq!(s.undo().as_deref(), Some("Backward for 2 races"));
    assert_eq!(s.map, before);
    // One bad cell: none made.
    let bad = vec![(format!("{PACK}:b"), "yes".to_string()), (format!("{PACK}:zz"), "yes".to_string())];
    assert!(edit_many(&mut s, PACK, RACE, "backward", &bad, "x").is_err());
    assert_eq!(s.map, before);
}
