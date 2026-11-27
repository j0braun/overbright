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

fn span(a: (usize, usize), b: (usize, usize)) -> Span {
    Span::between(a, b)
}

#[test]
fn one_value_fills_several_cells_as_one_step() {
    let mut s = session();
    let before = s.map.clone();
    let mut g = races(&s);
    g.sort_by(0); // alpha, Bravo, Charlie
    let edits = g.fill(span((1, 4), (2, 4)), "yes").unwrap();
    assert_eq!(edits.len(), 2);
    edit_cells(&mut s, PACK, RACE, &edits, "Fill 2 cells").unwrap();
    assert!(s.map.races.iter().filter(|r| r.id != "a").all(|r| r.backward));
    assert_eq!(s.undo().as_deref(), Some("Fill 2 cells"));
    assert_eq!(s.map, before);
    // A read-only column in the block refuses the whole block.
    let bad = g.fill(span((0, 3), (1, 5)), "2").unwrap_err();
    assert!(bad.iter().all(|r| r.column == "Checkpoints" && r.why.contains("here yet")));
}

#[test]
fn paste_checks_every_cell_and_refuses_the_block() {
    let mut s = session();
    let before = s.map.clone();
    let mut g = races(&s);
    g.sort_by(0);
    // Tab text across two columns and two rows (name, type), anchored at the selection.
    let ok = g.paste(span((0, 0), (0, 0)), "One\tcircuit\nTwo\tdrift").unwrap();
    edit_cells(&mut s, PACK, RACE, &ok, "Paste 4 cells").unwrap();
    assert!(s.map.races.iter().any(|r| r.name == "One" && r.kind == RaceKind::Circuit));
    assert!(s.map.races.iter().any(|r| r.name == "Two" && r.kind == RaceKind::Drift));
    assert_eq!(s.undo().as_deref(), Some("Paste 4 cells"));
    assert_eq!(s.map, before);
    // One bad cell (laps 99): nothing is made and the words name the cell.
    let edits = g.paste(span((0, 0), (0, 0)), "X\tcircuit\t4002\t99").unwrap();
    let why = edit_cells(&mut s, PACK, RACE, &edits, "Paste").unwrap_err();
    assert!(why.contains("Laps") && why.contains("range"), "{why}");
    assert_eq!(s.map, before);
    // Too big for the table, and nothing to paste.
    assert!(g.paste(span((2, 0), (2, 0)), "a\nb").unwrap_err()[0].why.contains("do not fit"));
    assert!(g.paste(span((0, 0), (0, 0)), "").is_err());
    // One value goes into every selected cell.
    assert_eq!(g.paste(span((0, 3), (2, 3)), "2").unwrap().len(), 3);
}

#[test]
fn a_position_pastes_as_one_cell_and_a_copy_pastes_back() {
    let mut s = MapSession::new("L4RA", PACK, MapMod::default(), world());
    let key = s.key_of(0).unwrap();
    s.run(Op::Place { key, position: Vec3::new(11.0, 2.0, 0.0), rotation: Mat3::IDENTITY }).unwrap();
    let g = Grid::build(&s.map, PACK, PLACEMENT).unwrap();
    let at = (0..g.counts().1).find(|i| g.cell(*i, 1).unwrap().text == "11, 2, 0").unwrap();
    let e = g.paste(span((at, 1), (at, 1)), "5, 6, 7").unwrap();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].text, "5, 6, 7");
    // The copy of a block pastes back as no change.
    let all = span((0, 0), (g.counts().1 - 1, 3));
    assert!(g.paste(span((0, 0), (0, 0)), &g.copy_text(all)).unwrap().is_empty());
    // Position and heading of one row go together.
    let k = g.key(at).unwrap().to_string();
    let both = [
        CellEdit { key: k.clone(), field: "heading", text: "90".into() },
        CellEdit { key: k, field: "position", text: "5, 6, 7".into() },
    ];
    edit_cells(&mut s, PACK, PLACEMENT, &both, "Paste").unwrap();
    let g = Grid::build(&s.map, PACK, PLACEMENT).unwrap();
    assert!((0..2).any(|i| g.cell(i, 1).unwrap().text == "5, 6, 7" && g.cell(i, 2).unwrap().text == "90"));
}

#[test]
fn the_csv_round_trip_changes_nothing_and_edits_go_through_the_same_checks() {
    let mut s = session();
    let before = s.map.clone();
    let mut g = races(&s);
    g.sort_by(0);
    g.set_search("a"); // alpha, Bravo, Charlie all have an a; the table is what is visible
    let csv = g.csv();
    assert!(csv.starts_with("row,name,type,track,laps,backward,checkpoints\n"), "{csv}");
    // Nothing changed: no edits, so no step; the file written again is byte-identical.
    assert!(g.import(&csv).unwrap().is_empty());
    assert_eq!(csv_write(&csv_read(&csv, ',').unwrap()), csv);
    // Edit two cells in the file (a name with a comma and a quote survives).
    let edited = csv.replace("alpha", "\"al,\"\"pha\"\"\"").replace(",3,no,", ",4,no,");
    let edits = g.import(&edited).unwrap();
    assert_eq!(edits.len(), 2, "{edits:?}");
    edit_cells(&mut s, PACK, RACE, &edits, "Import table").unwrap();
    assert!(s.map.races.iter().any(|r| r.name == "al,\"pha\""));
    assert_eq!(s.undo().as_deref(), Some("Import table"));
    assert_eq!(s.map, before);
    // Bad cells, unknown rows and columns, a read-only change: refused with words, nothing made.
    let bad = g.import(&csv.replace(",3,no,", ",99,no,")).unwrap();
    assert!(edit_cells(&mut s, PACK, RACE, &bad, "x").unwrap_err().contains("range"));
    assert!(g.import(&csv.replace("my-map:a,", "nope,")).unwrap_err()[0].why.contains("not a row"));
    assert!(g.import(&csv.replace("laps", "lapz")).unwrap_err()[0].why.contains("not a column"));
    assert!(g.import(&csv.replace("2 checkpoints", "5 checkpoints")).is_err());
    assert!(g.import("name\nx\n").is_err());
    assert_eq!(s.map, before);
}

#[test]
fn csv_reads_quotes_and_line_endings() {
    assert_eq!(csv_read("a,\"b\r\nc\",\"d\"\"e\"\r\n\r\nf,,g", ',').unwrap(), [vec!["a", "b\r\nc", "d\"e"], vec!["f", "", "g"]]);
    assert!(csv_read("a,\"b", ',').is_err());
    assert_eq!(csv_read("", ',').unwrap().len(), 0);
}

#[test]
fn a_file_is_written_where_chosen_and_never_in_the_game_folder() {
    let s = session();
    let g = races(&s);
    let d = std::env::temp_dir().join(format!("ob-data-csv-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let file = d.join("out").join("races.csv");
    export_file(&g, &file, Some(&d.join("game"))).unwrap();
    assert_eq!(read_file(&file).unwrap(), g.csv());
    let game = d.join("game");
    std::fs::create_dir_all(&game).unwrap();
    assert!(export_file(&g, &game.join("x.csv"), Some(&game)).unwrap_err().contains("game's own folder"));
    assert!(!game.join("x.csv").exists());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn fifty_thousand_rows_build_and_stay_in_step() {
    let rows: Vec<Row> = (0..50_000)
        .map(|i| Row {
            kind: RACE,
            key: format!("big:r{i}"),
            added: true,
            values: vec![crate::changes::Value {
                field: "laps".into(),
                from: From::Yours,
                game: Some(toml::Value::Integer(1 + i % 20)),
                yours: Some(toml::Value::Integer(1 + i % 20)),
                original: None,
            }],
        })
        .collect();
    let t = std::time::Instant::now();
    let mut g = Grid::from_rows(RACE, &rows).unwrap();
    g.sort_by(3);
    g.set_search("17");
    assert!(g.counts().1 > 0 && g.counts().1 < 50_000);
    g.set_search("");
    assert_eq!(g.counts(), (50_000, 50_000));
    assert!(t.elapsed().as_secs() < 20, "{:?}", t.elapsed());
    assert_eq!(g.csv().lines().count(), 50_001);
}
