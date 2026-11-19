//! Undo / redo: a stack of whole-mod snapshots (a mod is small), one per
//! committed gesture. Applying a snapshot applies the mod to the original
//! world again, the same path as opening it.

use super::state::EditorState;
use crate::tools::map::{MapMod, PropEdit};

/// How many steps are kept.
pub const MAX_STEPS: usize = 256;

/// The history.
#[derive(Clone, Debug, Default)]
pub struct Undo {
    /// (step name, the mod before the step), oldest first.
    past: Vec<(String, MapMod)>,
    /// (step name, the mod after the step), next redo last.
    future: Vec<(String, MapMod)>,
    /// The mod as of the last step.
    current: MapMod,
}

impl Undo {
    pub fn new(map: &MapMod) -> Undo {
        Undo { past: Vec::new(), future: Vec::new(), current: map.clone() }
    }

    /// The name of the step Undo takes back.
    pub fn undo_label(&self) -> Option<&str> {
        self.past.last().map(|p| p.0.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.future.last().map(|p| p.0.as_str())
    }

    /// Record `now` as a step after `current`; false when nothing changed.
    pub fn record(&mut self, label: &str, now: &MapMod) -> bool {
        if *now == self.current {
            return false;
        }
        let before = std::mem::replace(&mut self.current, now.clone());
        self.past.push((label.to_string(), before));
        if self.past.len() > MAX_STEPS {
            self.past.remove(0);
        }
        self.future.clear();
        true
    }

    /// The mod one step back, if any.
    pub fn undo(&mut self) -> Option<MapMod> {
        let (label, before) = self.past.pop()?;
        let after = std::mem::replace(&mut self.current, before.clone());
        self.future.push((label, after));
        Some(before)
    }

    /// The mod one step forward, if any.
    pub fn redo(&mut self) -> Option<MapMod> {
        let (label, after) = self.future.pop()?;
        let before = std::mem::replace(&mut self.current, after.clone());
        self.past.push((label, before));
        Some(after)
    }
}

fn adds(m: &MapMod) -> Vec<&PropEdit> {
    m.props.iter().filter(|e| matches!(e, PropEdit::Add { .. })).collect()
}

/// See [`super::state::commit`].
pub fn commit(s: &mut EditorState, label: &str) {
    let before_adds: Vec<PropEdit> = adds(s.undo_current()).into_iter().cloned().collect();
    if !s.undo.record(label, &s.map) {
        return;
    }
    s.dirty = true;
    s.status = label.to_string();
    let now: Vec<PropEdit> = adds(&s.map).into_iter().cloned().collect();
    apply(s, before_adds != now);
}

/// Take back the last step (Ctrl+Z).
pub fn undo(s: &mut EditorState) -> bool {
    let label = s.undo.undo_label().map(str::to_string);
    let Some(m) = s.undo.undo() else { return false };
    step_to(s, m, format!("Undone: {}", label.unwrap_or_default()));
    true
}

/// Do the last undone step again (Ctrl+Y, Ctrl+Shift+Z).
pub fn redo(s: &mut EditorState) -> bool {
    let label = s.undo.redo_label().map(str::to_string);
    let Some(m) = s.undo.redo() else { return false };
    step_to(s, m, format!("Redone: {}", label.unwrap_or_default()));
    true
}

fn step_to(s: &mut EditorState, m: MapMod, status: String) {
    let changed_adds = adds(&s.map) != adds(&m);
    s.map = m;
    s.dirty = true;
    s.status = status;
    s.selection.retain(|&i| i < s.base.placements.len());
    if s.race.is_some_and(|r| r >= s.map.races.len()) {
        s.race = s.map.races.len().checked_sub(1);
        s.point = None;
    }
    apply(s, changed_adds);
}

impl EditorState {
    fn undo_current(&self) -> &MapMod {
        &self.undo.current
    }
}

/// Apply `s.map` to the world again: placements back to the original,
/// then the mod's edits.
fn apply(s: &mut EditorState, adds_changed: bool) {
    let n = s.base.placements.len();
    let w = &mut s.world;
    if w.placements.len() > n {
        w.placements.truncate(n);
    }
    for i in 0..n.min(w.placements.len()) {
        let (bt, bf) = (s.base.placements[i].transform, s.base.placements[i].flags);
        let p = &mut w.placements[i];
        if p.transform != bt || p.flags != bf {
            p.flags = bf;
            w.set_placement_transform(i, bt);
        }
    }
    w.smackables = s.base.smackables.clone();
    w.map_overlay = Default::default();
    w.apply_map_mods(&s.region, &[(s.folder.clone(), s.map.clone())]);
    s.world_changed = true;
    s.adds_changed |= adds_changed;
    s.selection.retain(|&i| i < s.world.placements.len());
}

/// Apply `s.map` to the world again from scratch (opening a mod, a new
/// one, reverting).
pub fn apply_map(s: &mut EditorState) {
    apply(s, true);
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
