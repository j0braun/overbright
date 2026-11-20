//! `kill -9` of an editor session while it saves (ARCHITECTURE.md, Studio): a child process edits a map mod
//! through a [`MapSession`] kept in the mod's history, saving at a quiet point after nearly every step, and is
//! killed (SIGKILL on Linux, TerminateProcess on Windows) at varying points. The next session gets the mod for
//! writing, its file whole, and restoring the unsaved steps gives every step the child reported made.
//!
//! In its own test program: a child started while another test thread holds a mod's lock would hold it too
//! until its `exec`.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ob_map::tools::map::format::MapMod;
use ob_map::tools::map::ops::Op;
use ob_map::tools::map::store;
use ob_studio::history::disk::Real;
use ob_studio::history::{History, STUDIO};
use ob_studio::session::MapSession;
use ob_studio::studio_api::world::World;
use ob_testkit::Scratch;

/// The mod in `dir` open in a session kept in its history, its unsaved steps restored.
fn session(dir: &Path) -> MapSession {
    let file = dir.join(store::FILE);
    let map = MapMod::parse(&std::fs::read_to_string(&file).unwrap())
        .unwrap_or_else(|e| panic!("the file is not whole: {e:?}"));
    let mut h = History::open(dir, &dir.join(store::EDITOR_DIR), &[store::FILE], STUDIO, Arc::new(Real)).unwrap();
    assert!(h.writable(), "the killed session's lock is still held");
    h.restore();
    let mut s = MapSession::new("L4RA", "kill", map, World::default());
    s.keep_in(h);
    s
}

fn made(s: &MapSession) -> u64 {
    s.map.name.trim_start_matches('k').parse().unwrap()
}

/// The child's side of the kill test (does nothing unless the parent started it).
#[test]
fn child_session() {
    let Some(dir) = std::env::var_os("OB_STUDIO_SESSION_KILL_CHILD") else { return };
    let mut s = session(Path::new(&dir));
    let mut k = made(&s);
    loop {
        k += 1;
        let op =
            Op::Details { name: format!("k{k}"), author: "kill test".into(), description: "x".repeat(k as usize % 97) };
        let c = s.run(op).unwrap();
        assert!(c.changed && s.note.is_none(), "step {k} not kept: {:?}", s.note);
        println!("made {k}");
        if k % 5 != 0 {
            // A quiet point: the file is written now.
            let h = s.history.as_mut().unwrap();
            h.tick(Instant::now() + Duration::from_secs(3600)).unwrap();
            s.sync();
        }
    }
}

#[test]
fn kill_9_during_saving_reopens_with_every_reported_step() {
    let s = Scratch::new("session-kill");
    std::fs::write(s.path().join(store::FILE), MapMod { name: "k0".into(), ..MapMod::default() }.write()).unwrap();
    let exe = std::env::current_exe().unwrap();
    let mut last_made = 0u64;
    for round in 0..12u64 {
        let mut child = std::process::Command::new(&exe)
            .args(["--exact", "child_session", "--nocapture", "--test-threads=1"])
            .env("OB_STUDIO_SESSION_KILL_CHILD", s.path())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let out = child.stdout.take().unwrap();
        // Kill after a varying number of reported steps (the child goes on saving meanwhile).
        let want = 2 + (round * 29) % 80;
        let mut seen = 0;
        for line in BufReader::new(out).lines() {
            let Ok(line) = line else { break };
            if let Some(k) = line.strip_prefix("made ").and_then(|k| k.parse::<u64>().ok()) {
                last_made = k;
                seen += 1;
                if seen >= want {
                    break;
                }
            }
        }
        if round % 3 == 1 {
            std::thread::sleep(Duration::from_millis(round % 5));
        }
        child.kill().unwrap();
        let _ = child.wait();
        assert!(seen >= want, "round {round}: the child stopped early");

        let mut again = session(s.path());
        let k = made(&again);
        assert!(k >= last_made, "round {round}: step {last_made} was reported made, the session has {k}");
        // Editing goes on: one more step is kept.
        again
            .run(Op::Details { name: format!("k{}", k + 1), author: String::new(), description: String::new() })
            .unwrap();
        assert!(again.note.is_none() && made(&again) == k + 1, "round {round}: {:?}", again.note);
        assert_eq!(again.undo_label(), Some("Mod details"));
        last_made = k + 1;
    }
}
