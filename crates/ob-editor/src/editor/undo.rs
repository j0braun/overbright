//! Undo / redo and the editor's way into the session's ops: whole-mod
//! snapshots, one per committed gesture or [`Op`]
//! ([`crate::tools::map::session`]). Applying a snapshot applies the mod
//! to the original world again, the same path as opening it; here the
//! window's own state (status bar, selection, current race) follows.

use super::state::EditorState;
use crate::tools::map::edit::Refused;
use crate::tools::map::ops::{Change, Op};
pub use crate::tools::map::session::{MAX_STEPS, Undo};

/// Run `op` on the open mod as one undo step; the status bar names it
/// (or the refusal). The selection keeps only placements still in the
/// world.
pub fn run(s: &mut EditorState, op: Op) -> Result<Change, Refused> {
    let r = s.session.run(op);
    match &r {
        Ok(c) if c.changed => s.status = c.label.clone(),
        Ok(_) => {}
        Err(e) => s.status = e.to_string(),
    }
    tidy(s);
    r
}

/// See [`super::state::commit`].
pub fn commit(s: &mut EditorState, label: &str) {
    if s.session.commit(label) {
        s.status = label.to_string();
        tidy(s);
    }
}

/// Take back the last step (Ctrl+Z).
pub fn undo(s: &mut EditorState) -> bool {
    let Some(label) = s.session.undo() else { return false };
    stepped(s, format!("Undone: {label}"));
    true
}

/// Do the last undone step again (Ctrl+Y, Ctrl+Shift+Z).
pub fn redo(s: &mut EditorState) -> bool {
    let Some(label) = s.session.redo() else { return false };
    stepped(s, format!("Redone: {label}"));
    true
}

fn stepped(s: &mut EditorState, status: String) {
    s.status = status;
    s.selection.retain(|&i| i < s.session.base.placements.len());
    if s.race.is_some_and(|r| r >= s.session.map.races.len()) {
        s.race = s.session.map.races.len().checked_sub(1);
        s.point = None;
    }
    tidy(s);
}

/// The selection within the world.
fn tidy(s: &mut EditorState) {
    let n = s.session.world.placements.len();
    s.selection.retain(|&i| i < n);
}

/// Apply `s.session.map` to the world again from scratch (opening a mod,
/// a new one, reverting).
pub fn apply_map(s: &mut EditorState) {
    s.session.apply_map();
    tidy(s);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::map::MapMod;

    fn named(n: &str) -> MapMod {
        MapMod { name: n.into(), ..MapMod::default() }
    }

    #[test]
    fn steps_labels_and_redo_cleared_by_a_new_step() {
        let mut u = Undo::new(&named("a"));
        assert!(!u.record("Nothing", &named("a")), "an unchanged mod is no step");
        assert!(u.record("To b", &named("b")));
        assert!(u.record("To c", &named("c")));
        assert_eq!(u.undo_label(), Some("To c"));
        assert_eq!(u.undo().unwrap().name, "b");
        assert_eq!(u.redo_label(), Some("To c"));
        assert_eq!(u.undo().unwrap().name, "a");
        assert!(u.undo().is_none());
        assert_eq!(u.redo().unwrap().name, "b");
        assert!(u.record("To d", &named("d")));
        assert!(u.redo().is_none(), "a new step drops the redo steps");
        assert_eq!(u.undo().unwrap().name, "b");
    }

    #[test]
    fn history_is_capped() {
        let mut u = Undo::new(&named("0"));
        for k in 1..=MAX_STEPS + 10 {
            u.record("step", &named(&k.to_string()));
        }
        let mut n = 0;
        while u.undo().is_some() {
            n += 1;
        }
        assert_eq!(n, MAX_STEPS);
    }

    /// The ops name their undo steps as the panels name them: the race
    /// tools build exactly the ops below, and the props panel's labels.
    #[test]
    fn op_labels_match_the_panels() {
        use super::super::races::{Tool, place_op, point_op};
        use crate::tools::map::edit::RacePart;
        use crate::tools::map::ops::RacePoint;
        use crate::tools::map::{CustomRace, RaceKind, Spot};
        use glam::{Mat3, Vec2, Vec3};

        let mut map = MapMod::default();
        let mut r = CustomRace::new("r", 1, RaceKind::Sprint);
        r.checkpoints = vec![Vec2::ZERO, Vec2::X * 100.0, Vec2::X * 200.0];
        r.start = Some(Spot { position: Vec3::ZERO, heading: 0.0 });
        r.finish = Some(Spot { position: Vec3::X * 300.0, heading: 0.0 });
        map.races.push(r.clone());
        let placed = |r: &CustomRace, tool, insert_at| {
            place_op(r, tool, Vec3::X * 50.0, None, None, insert_at, Vec2::X).unwrap().label(&map)
        };
        assert_eq!(placed(&r, Tool::Checkpoint, 1), "Insert checkpoint 2");
        assert_eq!(placed(&r, Tool::Checkpoint, 3), "Add checkpoint 4");
        assert_eq!(placed(&r, Tool::Start, 0), "Place start line");
        assert_eq!(placed(&r, Tool::Finish, 0), "Place finish line");
        // A grid slot added to a race on its default grid (full: 8 slots).
        let mut seven = r.clone();
        seven.grid = crate::tools::map::race::effective_grid(&seven);
        seven.grid.pop();
        let m7 = MapMod { races: vec![seven.clone()], ..MapMod::default() };
        let grid = place_op(&seven, Tool::Grid, Vec3::ZERO, None, None, 0, Vec2::X).unwrap();
        assert_eq!(grid.label(&m7), "Add grid slot 8");
        for p in [
            RacePoint { part: RacePart::Checkpoint, index: 1 },
            RacePoint { part: RacePart::Grid, index: 2 },
            RacePoint { part: RacePart::Start, index: 0 },
            RacePoint { part: RacePart::Finish, index: 0 },
        ] {
            // Dragging or typing a point: "Move " + the panel's title.
            let mv = point_op(&r, p).unwrap();
            let title = match p.part {
                RacePart::Checkpoint => format!("Checkpoint {}", p.index + 1),
                RacePart::Grid => format!("Grid slot {}", p.index + 1),
                RacePart::Start => "Start line".into(),
                RacePart::Finish => "Finish line".into(),
            };
            assert_eq!(mv.label(&map), format!("Move {}", title.to_lowercase()));
        }
        // The props panel: the verb and the one prop's solid name.
        let mut s = super::super::props::tests::state();
        s.selection = vec![2];
        let key = s.session.key_of(2).unwrap();
        let ops = [
            (Op::DropToGround { key: key.clone() }, "Drop to ground"),
            (Op::Delete { key: key.clone() }, "Delete"),
            (Op::Reset { key: key.clone() }, "Reset"),
            (Op::Duplicate { key: key.clone(), position: Vec3::ZERO }, "Duplicate"),
            (Op::Place { key, position: Vec3::ZERO, rotation: Mat3::IDENTITY }, "Move"),
        ];
        for (op, verb) in ops {
            assert_eq!(op.label(&s.session.map), super::super::props::label(&s, verb));
        }
        let add = Op::Add {
            solid: "XC_CONE_01".into(),
            position: Vec3::ZERO,
            rotation: Mat3::IDENTITY,
            id: None,
            ground: true,
        };
        assert_eq!(add.label(&s.session.map), "Add XC_CONE_01");
        // An added prop goes by its solid's name, in the panel and the op.
        let c = run(&mut s, add).unwrap();
        let i = s.session.placement_of(&c.added[0]).unwrap();
        s.selection = vec![i];
        assert_eq!(
            Op::Delete { key: c.added[0].clone() }.label(&s.session.map),
            super::super::props::label(&s, "Delete")
        );
    }

    #[test]
    fn a_run_op_is_one_step_and_names_the_status() {
        // Named `Prop` here: the key table test scans the sources for `Key::` paths.
        use crate::tools::map::ops::PropKey as Prop;
        let mut s = super::super::props::tests::state();
        let key = s.session.key_of(0).unwrap();
        run(&mut s, Op::Delete { key }).unwrap();
        assert_eq!(s.status, "Delete XC_CONE_01");
        assert!(s.session.dirty);
        assert_eq!(run(&mut s, Op::Delete { key: Prop::Added("x".into()) }), Err(Refused::NoPlacement));
        assert_eq!(s.status, "No such placement");
        assert!(undo(&mut s));
        assert_eq!(s.status, "Undone: Delete XC_CONE_01");
        assert!(redo(&mut s));
        assert_eq!(s.status, "Redone: Delete XC_CONE_01");
    }
}
