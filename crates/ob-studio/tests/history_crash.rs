//! A mod's history survives a crash at any point: the mod reads back whole, as it was before the interrupted
//! write or after it, never half (ARCHITECTURE.md, Studio). Fault injection: the same session is stopped at
//! every one of its file operations in turn, with nothing, one byte, half or all but one byte of that operation
//! on the disk, then opened again. (`kill -9` of a real process: history_kill.rs.)

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ob_map::tools::map::format::{CustomRace, MapMod, RaceKind};
use ob_map::tools::map::store;
use ob_studio::history::disk::{Cut, Disk, Faulty, Real};
use ob_studio::history::doc::State;
use ob_studio::history::{self, History, STUDIO, inbox};
use ob_testkit::Scratch;

fn map(name: &str, races: &[(&str, u32)]) -> String {
    let mut m = MapMod { name: name.into(), ..MapMod::default() };
    for (id, laps) in races {
        let mut r = CustomRace::new(id, 1, RaceKind::Circuit);
        r.laps = *laps;
        m.races.push(r);
    }
    m.write()
}

fn put(text: &str) -> BTreeMap<String, Option<String>> {
    [(store::FILE.to_string(), Some(text.to_string()))].into()
}

fn open(dir: &Path, disk: Arc<dyn Disk>) -> history::Result<History> {
    History::open(dir, &dir.join(store::EDITOR_DIR), &[store::FILE], STUDIO, disk)
}

const REQUEST: &str = "req-1";

/// A mod with one waiting inbox request.
fn setup(dir: &Path) {
    std::fs::write(dir.join(store::FILE), map("start", &[("r1", 2), ("r2", 2)])).unwrap();
    let r = inbox::Request {
        id: REQUEST.into(),
        author: "overbright-studio (command line)".into(),
        label: "Set laps".into(),
        base: Some(0),
        docs: put(&map("start", &[("r1", 2), ("r2", 7)])),
    };
    inbox::submit(&dir.join(store::EDITOR_DIR), &r).unwrap();
}

/// The session: every kind of write the history makes. After each step: the history's state and the file.
fn session(h: &mut History, after: &mut Vec<(State, String)>, dir: &Path) -> history::Result<()> {
    let later = Instant::now() + Duration::from_secs(3600);
    let mut note = |h: &History| {
        after.push((h.state().clone(), std::fs::read_to_string(dir.join(store::FILE)).unwrap_or_default()))
    };
    note(h);
    h.commit(STUDIO, "Laps r1", put(&map("start", &[("r1", 3), ("r2", 2)])))?;
    note(h);
    h.commit(STUDIO, "Rename", put(&map("renamed", &[("r1", 3), ("r2", 2)])))?;
    note(h);
    h.save()?;
    note(h);
    h.undo(STUDIO)?;
    note(h);
    h.tick(later)?; // the inbox request, then the quiet-point save
    note(h);
    h.commit("agent: helper", "Add r3", put(&map("start", &[("r1", 3), ("r2", 7), ("r3", 4)])))?;
    note(h);
    h.mark_version("v1")?;
    note(h);
    h.redo(STUDIO)?;
    note(h);
    h.save()?;
    note(h);
    h.compact()?;
    note(h);
    h.commit(STUDIO, "Laps r3", put(&map("start", &[("r1", 3), ("r2", 7), ("r3", 5)])))?;
    note(h);
    h.tick(later)?;
    note(h);
    Ok(())
}

#[test]
fn a_crash_at_any_write_leaves_the_mod_whole() {
    // The run without a fault: the states after each step and how many file operations it makes.
    let clean = Scratch::new("history-crash-clean");
    setup(clean.path());
    let count = Arc::new(Faulty::new(None, Cut::Start));
    let mut states = Vec::new();
    {
        let mut h = open(clean.path(), count.clone()).unwrap();
        session(&mut h, &mut states, clean.path()).unwrap();
    }
    let ops = count.ops();
    assert!(ops >= 15, "the session makes {ops} file operations");
    let files: Vec<&String> = states.iter().map(|(_, f)| f).collect();
    let mut runs = 0;
    for at in 0..ops {
        for cut in Cut::ALL {
            let s = Scratch::new("history-crash");
            setup(s.path());
            let disk = Arc::new(Faulty::new(Some(at), cut));
            let mut seen = Vec::new();
            let r = open(s.path(), disk.clone()).and_then(|mut h| session(&mut h, &mut seen, s.path()));
            assert!(r.is_err() && disk.crashed(), "op {at} {cut:?}: the fault was not reached");
            // `seen` ends with the last whole step's state: the crash came in the next one (or in opening).
            let done = seen.len();
            runs += 1;

            // Opened again as after a crash (the lock went with the dropped history, as with a dead process).
            let mut h = open(s.path(), Arc::new(Real)).unwrap_or_else(|e| panic!("op {at} {cut:?}: reopen: {e}"));
            assert!(h.writable(), "op {at} {cut:?}");
            let text = std::fs::read_to_string(s.path().join(store::FILE)).unwrap();
            MapMod::parse(&text).unwrap_or_else(|e| panic!("op {at} {cut:?}: the file is not whole: {e:?}"));
            assert!(
                files[..(done + 1).min(files.len())].contains(&&text) || text == map("start", &[("r1", 2), ("r2", 2)]),
                "op {at} {cut:?}: the file is neither the old one nor the new one"
            );
            // The history is at the last whole step or at the interrupted one (that one only if its record
            // reached the disk whole).
            let old = if done == 0 { None } else { Some(&states[done - 1].0) };
            let new = states.get(done).map(|s| &s.0);
            let now = h.state();
            assert!(
                Some(now) == old || Some(now) == new || (done == 0 && h.steps().is_empty()),
                "op {at} {cut:?}: after step {done} the history is at neither side of it"
            );
            // The inbox request is made at most once, and answered once Studio ticks.
            h.restore();
            h.tick(Instant::now()).unwrap();
            let made = h.steps().iter().filter(|s| s.request.as_deref() == Some(REQUEST)).count();
            assert!(made <= 1, "op {at} {cut:?}: the request was made {made} times");
            assert!(inbox::reply(h.editor_dir(), REQUEST).is_some(), "op {at} {cut:?}: no reply");
            // And it goes on working: a new step, saved, read back.
            h.commit(STUDIO, "After", put(&map("after", &[]))).unwrap();
            h.save().unwrap();
            drop(h);
            let h = open(s.path(), Arc::new(Real)).unwrap();
            assert_eq!(h.doc(store::FILE), Some(map("after", &[]).as_str()), "op {at} {cut:?}");
            assert!(h.pending().is_none());
        }
    }
    assert_eq!(runs, ops * Cut::ALL.len());
}
