use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use ob_map::tools::map::format::{CustomRace, MapMod, RaceKind};
use ob_map::tools::map::store;
use ob_testkit::Scratch;

use super::*;

pub(crate) fn map(name: &str, races: &[(&str, u32)]) -> String {
    let mut m = MapMod { name: name.into(), ..MapMod::default() };
    for (id, laps) in races {
        let mut r = CustomRace::new(id, 1, RaceKind::Circuit);
        r.laps = *laps;
        m.races.push(r);
    }
    m.write()
}

fn open(dir: &Path) -> History {
    History::open(dir, &dir.join(store::EDITOR_DIR), &[store::FILE], STUDIO, Arc::new(disk::Real)).unwrap()
}

fn put(text: &str) -> BTreeMap<String, Option<String>> {
    [(store::FILE.to_string(), Some(text.to_string()))].into()
}

fn file(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(store::FILE)).unwrap()
}

#[test]
fn steps_undo_and_redo_survive_a_reopen() {
    let s = Scratch::new("history-reopen");
    std::fs::write(s.path().join(store::FILE), map("a", &[])).unwrap();
    let mut h = open(s.path());
    assert_eq!(h.commit(STUDIO, "To b", put(&map("b", &[]))).unwrap(), Outcome::Made(1));
    assert_eq!(h.commit(STUDIO, "To c", put(&map("c", &[]))).unwrap(), Outcome::Made(2));
    assert_eq!(h.commit(STUDIO, "Again c", put(&map("c", &[]))).unwrap(), Outcome::Unchanged);
    assert_eq!(h.undo(STUDIO).unwrap().as_deref(), Some("To c"));
    drop(h);
    // Not saved: the journal has it, the file not yet; reopening offers it back.
    assert_eq!(file(s.path()), map("a", &[]));
    let mut h = open(s.path());
    assert_eq!(h.doc(store::FILE), Some(map("b", &[]).as_str()));
    assert!(h.pending().is_some());
    assert_eq!(h.undone().len(), 1);
    assert_eq!(h.redo(STUDIO).unwrap().as_deref(), Some("To c"));
    h.restore();
    h.save().unwrap();
    assert_eq!(file(s.path()), map("c", &[]));
    drop(h);
    let h = open(s.path());
    assert!(h.pending().is_none());
    assert_eq!(h.steps().iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["To b", "To c"]);
}

#[test]
fn every_save_records_the_build_that_wrote_it() {
    let s = Scratch::new("history-saved-by");
    std::fs::write(s.path().join(store::FILE), map("a", &[])).unwrap();
    let mut h = open(s.path());
    assert_eq!(h.saved_by(), None, "nothing written by Studio yet");
    h.commit(STUDIO, "To b", put(&map("b", &[]))).unwrap();
    h.save().unwrap();
    let by = h.saved_by().expect("the save stamped itself");
    assert_eq!(by.build, crate::share::build());
    assert_eq!(by.docs[store::FILE], format!("{:016x}", journal::hash(&file(s.path()))), "the file as written");
    // The next save stamps again, with the new file's hash; a reopened history reads the same.
    h.commit(STUDIO, "To c", put(&map("c", &[]))).unwrap();
    h.save().unwrap();
    let again = h.saved_by().unwrap();
    assert_eq!(again.docs[store::FILE], format!("{:016x}", journal::hash(&map("c", &[]))));
    drop(h);
    assert_eq!(open(s.path()).saved_by(), Some(again));
}

#[test]
fn discarding_unsaved_steps_is_one_undoable_step() {
    let s = Scratch::new("history-discard");
    std::fs::write(s.path().join(store::FILE), map("a", &[])).unwrap();
    let mut h = open(s.path());
    h.commit(STUDIO, "To b", put(&map("b", &[]))).unwrap();
    drop(h);
    let mut h = open(s.path());
    assert!(h.pending().is_some());
    h.discard().unwrap();
    assert_eq!(h.doc(store::FILE), Some(map("a", &[]).as_str()));
    assert_eq!(h.undo(STUDIO).unwrap().as_deref(), Some("Discard unsaved changes"));
    assert_eq!(h.doc(store::FILE), Some(map("b", &[]).as_str()));
}

#[test]
fn files_are_written_at_a_quiet_point() {
    let s = Scratch::new("history-quiet");
    std::fs::write(s.path().join(store::FILE), map("a", &[])).unwrap();
    let mut h = open(s.path());
    h.commit(STUDIO, "To b", put(&map("b", &[]))).unwrap();
    assert!(!h.tick(Instant::now()).unwrap().saved, "still editing");
    assert_eq!(file(s.path()), map("a", &[]));
    assert!(h.tick(Instant::now() + QUIET).unwrap().saved);
    assert_eq!(file(s.path()), map("b", &[]));
    assert!(!h.unsaved());
    // Editing on without a pause is written after LONGEST_UNSAVED at the latest.
    h.commit(STUDIO, "To c", put(&map("c", &[]))).unwrap();
    h.unsaved_since = Some(Instant::now() - LONGEST_UNSAVED);
    assert!(h.tick(Instant::now()).unwrap().saved);
    assert_eq!(file(s.path()), map("c", &[]));
}

#[test]
fn a_change_against_an_older_step_merges_item_by_item() {
    let s = Scratch::new("history-merge");
    std::fs::write(s.path().join(store::FILE), map("a", &[("r1", 2), ("r2", 2)])).unwrap();
    let mut h = open(s.path());
    let base = h.seq();
    h.commit(STUDIO, "Laps r1", put(&map("a", &[("r1", 3), ("r2", 2)]))).unwrap();
    // Another writer read the mod at `base` and changed r2: both changes stay.
    let o = h.transact("cli", "Laps r2", Some(base), &put(&map("a", &[("r1", 2), ("r2", 5)])), None).unwrap();
    assert!(matches!(o, Outcome::Made(_)), "{o:?}");
    assert_eq!(h.doc(store::FILE), Some(map("a", &[("r1", 3), ("r2", 5)]).as_str()));
    // … and one that changed r1 too is a conflict naming that race; nothing is made.
    let before = h.state().clone();
    let o = h.transact("cli", "Laps r1", Some(base), &put(&map("a", &[("r1", 4), ("r2", 2)])), None).unwrap();
    assert_eq!(o, Outcome::Conflict(vec![format!("{}#race:r1", store::FILE)]));
    assert_eq!(h.state(), &before);
    assert_eq!(h.steps().last().unwrap().author, "cli");
}

#[test]
fn any_step_of_any_author_can_be_reverted() {
    let s = Scratch::new("history-revert");
    std::fs::write(s.path().join(store::FILE), map("a", &[("r1", 2), ("r2", 2)])).unwrap();
    let mut h = open(s.path());
    let Outcome::Made(first) = h.commit("agent: helper", "Laps r1", put(&map("a", &[("r1", 3), ("r2", 2)]))).unwrap()
    else {
        panic!()
    };
    h.commit(STUDIO, "Laps r2", put(&map("a", &[("r1", 3), ("r2", 4)]))).unwrap();
    // The older step goes back on its own; the later one stays.
    assert!(matches!(h.revert(first, STUDIO).unwrap(), Outcome::Made(_)));
    assert_eq!(h.doc(store::FILE), Some(map("a", &[("r1", 2), ("r2", 4)]).as_str()));
    assert_eq!(h.steps().last().unwrap().label, "Revert: Laps r1");
    // A step whose items were changed again later is a conflict.
    let Outcome::Made(third) = h.commit(STUDIO, "Laps r2 again", put(&map("a", &[("r1", 2), ("r2", 6)]))).unwrap()
    else {
        panic!()
    };
    h.commit(STUDIO, "Laps r2 once more", put(&map("a", &[("r1", 2), ("r2", 7)]))).unwrap();
    assert!(matches!(h.revert(third, STUDIO).unwrap(), Outcome::Conflict(_)));
}

#[test]
fn versions_are_kept_and_restored_as_a_step() {
    let s = Scratch::new("history-versions");
    std::fs::write(s.path().join(store::FILE), map("a", &[])).unwrap();
    let mut h = open(s.path());
    h.mark_version("first").unwrap();
    h.commit(STUDIO, "To b", put(&map("b", &[]))).unwrap();
    drop(h);
    let mut h = open(s.path());
    assert_eq!(h.versions().len(), 1);
    assert!(matches!(h.restore_version("first", STUDIO).unwrap(), Outcome::Made(_)));
    assert_eq!(h.doc(store::FILE), Some(map("a", &[]).as_str()));
    assert_eq!(h.undo(STUDIO).unwrap().as_deref(), Some("Back to version first"));
}

#[test]
fn another_programs_write_to_the_files_becomes_an_undoable_step() {
    let s = Scratch::new("history-outside");
    std::fs::write(s.path().join(store::FILE), map("a", &[("r1", 2), ("r2", 2)])).unwrap();
    let mut h = open(s.path());
    h.commit(STUDIO, "Laps r1", put(&map("a", &[("r1", 3), ("r2", 2)]))).unwrap();
    // While open (the unsaved step is kept: a different item).
    std::fs::write(s.path().join(store::FILE), map("a", &[("r1", 2), ("r2", 9)])).unwrap();
    h.stamps.clear();
    let t = h.tick(Instant::now()).unwrap();
    assert_eq!(t.outside.len(), 1);
    assert_eq!(h.steps().last().unwrap().author, OUTSIDE);
    assert_eq!(h.doc(store::FILE), Some(map("a", &[("r1", 3), ("r2", 9)]).as_str()));
    assert_eq!(h.undo(STUDIO).unwrap().unwrap(), outside_label(store::FILE));
    h.save().unwrap();
    drop(h);
    // While closed.
    std::fs::write(s.path().join(store::FILE), map("z", &[])).unwrap();
    let mut h = open(s.path());
    assert_eq!(h.steps().last().unwrap().author, OUTSIDE);
    assert_eq!(h.doc(store::FILE), Some(map("z", &[]).as_str()));
    assert!(h.pending().is_none());
    h.undo(STUDIO).unwrap();
    assert_eq!(h.doc(store::FILE), Some(map("a", &[("r1", 3), ("r2", 2)]).as_str()));
}

/// Two files written by another program (a checkout) are two steps, each named by its file, each taken back alone.
#[test]
fn each_file_another_program_wrote_is_its_own_step() {
    let s = Scratch::new("history-outside-each");
    let looks = "looks.toml";
    std::fs::write(s.path().join(store::FILE), map("a", &[])).unwrap();
    std::fs::write(s.path().join(looks), "format = 1\n").unwrap();
    let open2 = |d: &Path| History::open(d, &d.join(store::EDITOR_DIR), &[store::FILE, looks], STUDIO, Arc::new(disk::Real)).unwrap();
    let mut h = open2(s.path());
    std::fs::write(s.path().join(store::FILE), map("b", &[])).unwrap();
    std::fs::write(s.path().join(looks), "format = 1\n# by hand\n").unwrap();
    h.stamps.clear();
    let t = h.tick(Instant::now()).unwrap();
    assert_eq!(t.outside.len(), 2, "one step per file");
    let labels: Vec<&str> = h.steps().iter().rev().take(2).map(|s| s.label.as_str()).collect();
    assert!(labels.contains(&outside_label(store::FILE).as_str()) && labels.contains(&outside_label(looks).as_str()), "{labels:?}");
    assert_eq!(h.undo(STUDIO).unwrap().unwrap(), outside_label(store::FILE));
    assert_eq!(h.doc(store::FILE), Some(map("a", &[]).as_str()));
    assert_eq!(h.doc(looks), Some("format = 1\n# by hand\n"), "the other file's step stays");
    assert_eq!(h.undo(STUDIO).unwrap().unwrap(), outside_label(looks));
}

#[test]
fn a_second_opener_is_read_only_and_follows_the_writer() {
    let s = Scratch::new("history-readonly");
    std::fs::write(s.path().join(store::FILE), map("a", &[])).unwrap();
    let mut w = open(s.path());
    let mut r = open(s.path());
    assert!(w.writable());
    let by = r.read_only_by().expect("read-only").expect("owner named");
    assert_eq!((by.program.as_str(), by.pid), (STUDIO, std::process::id()));
    assert!(matches!(r.commit(STUDIO, "x", put(&map("x", &[]))), Err(Error::ReadOnly(Some(_)))));
    assert!(matches!(r.save(), Err(Error::ReadOnly(_))));
    w.commit(STUDIO, "To b", put(&map("b", &[]))).unwrap();
    assert!(r.tick(Instant::now()).unwrap().reloaded);
    assert_eq!(r.doc(store::FILE), Some(map("b", &[]).as_str()));
    assert!(!r.retry_write(STUDIO).unwrap());
    drop(w);
    assert!(r.retry_write(STUDIO).unwrap());
    assert_eq!(r.doc(store::FILE), Some(map("b", &[]).as_str()));
}

#[test]
fn the_inbox_makes_steps_and_an_ok_is_final() {
    let s = Scratch::new("history-inbox");
    std::fs::write(s.path().join(store::FILE), map("a", &[("r1", 2), ("r2", 2)])).unwrap();
    let mut h = open(s.path());
    let edir = h.editor_dir().to_path_buf();
    let base = h.seq();
    h.commit(STUDIO, "Laps r1", put(&map("a", &[("r1", 3), ("r2", 2)]))).unwrap();
    let req = |id: &str, r1, r2| inbox::Request {
        id: id.into(),
        author: "overbright-studio (command line)".into(),
        label: "Set laps".into(),
        base: Some(base),
        docs: put(&map("a", &[("r1", r1), ("r2", r2)])),
    };
    inbox::submit(&edir, &req("one", 2, 5)).unwrap();
    inbox::submit(&edir, &req("two", 4, 2)).unwrap();
    let t = h.tick(Instant::now()).unwrap();
    assert_eq!(t.answered.len(), 2);
    let Some(inbox::Reply::Ok { seq }) = inbox::reply(&edir, "one") else { panic!("{:?}", t.answered) };
    assert_eq!(h.steps().last().unwrap().seq, seq);
    assert_eq!(h.steps().last().unwrap().author, "overbright-studio (command line)");
    assert!(matches!(inbox::reply(&edir, "two"), Some(inbox::Reply::Conflict { .. })));
    assert_eq!(h.doc(store::FILE), Some(map("a", &[("r1", 3), ("r2", 5)]).as_str()));
    // Undo takes the program's step back like any other.
    assert_eq!(h.undo(STUDIO).unwrap().unwrap(), "overbright-studio (command line): Set laps");
    h.redo(STUDIO).unwrap();
    // A request made before a crash but not answered is answered from the journal, not made again.
    inbox::submit(&edir, &req("one", 2, 5)).unwrap();
    inbox::forget(&edir, "one");
    let n = h.steps().len();
    drop(h);
    let mut h = open(s.path());
    h.tick(Instant::now()).unwrap();
    assert_eq!(h.steps().len(), n);
    assert_eq!(inbox::reply(&edir, "one"), Some(inbox::Reply::Ok { seq }));
}

#[test]
fn the_journal_is_compacted_and_keeps_the_history() {
    let s = Scratch::new("history-compact");
    std::fs::write(s.path().join(store::FILE), map("a", &[])).unwrap();
    let mut h = open(s.path());
    for k in 0..MAX_STEPS + 20 {
        h.commit(STUDIO, &format!("Step {k}"), put(&map(&format!("m{k}"), &[]))).unwrap();
    }
    h.undo(STUDIO).unwrap();
    h.mark_version("v").unwrap();
    h.save().unwrap();
    let before = (h.state().clone(), h.steps().len(), h.undone().len());
    let len = std::fs::metadata(h.editor_dir().join(JOURNAL)).unwrap().len();
    h.compact().unwrap();
    assert!(std::fs::metadata(h.editor_dir().join(JOURNAL)).unwrap().len() < len);
    drop(h);
    let mut h = open(s.path());
    assert_eq!((h.state().clone(), h.steps().len(), h.undone().len()), before);
    assert_eq!(before.1 + before.2, MAX_STEPS);
    assert!(h.pending().is_none());
    assert_eq!(h.versions().len(), 1);
    assert!(h.redo(STUDIO).unwrap().is_some());
}

/// The history never writes a map file its reader refuses: a change to one is refused (a commit as an error, a
/// transaction, as the inbox makes them, as `Refused`), nothing reaches the journal or the file, and the mod
/// still opens.
#[test]
fn a_change_that_would_not_read_is_refused_and_nothing_is_written() {
    let s = Scratch::new("history-unreadable");
    std::fs::write(s.path().join(store::FILE), map("a", &[])).unwrap();
    let mut h = open(s.path());
    let bad = "overbright-map 1\n\n[race]\nid = s1\ntrack = 4001\nmode = sprint\nlaps = 3\n";
    let e = h.commit(STUDIO, "Laps 3", put(bad)).expect_err("refused");
    assert!(matches!(&e, Error::Unreadable(w) if w.contains("a sprint race runs one lap")), "{e}");
    match h.transact(STUDIO, "Laps 3", None, &put(bad), None).unwrap() {
        Outcome::Refused(why) => assert!(why.contains("one lap"), "{why}"),
        o => panic!("{o:?}"),
    }
    assert!(h.steps().is_empty());
    h.save().unwrap();
    drop(h);
    assert_eq!(file(s.path()), map("a", &[]));
    let h = open(s.path());
    assert!(h.steps().is_empty() && h.pending().is_none());
}
