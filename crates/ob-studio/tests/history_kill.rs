//! `kill -9` at varying points (ARCHITECTURE.md, Studio): a child process makes steps as fast as it can and is
//! killed (SIGKILL on Linux, TerminateProcess on Windows); the next opener gets the mod for writing (the dead
//! writer's lock is gone by itself) with every step the child reported made, and the file whole.
//!
//! In its own test program: a child started while another test thread holds a mod's lock would hold it too
//! until its `exec`.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ob_map::tools::map::format::{CustomRace, MapMod, RaceKind};
use ob_map::tools::map::store;
use ob_studio::history::disk::{Disk, Real};
use ob_studio::history::{self, History, STUDIO};
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

/// The child's side of the kill test (does nothing unless the parent started it).
#[test]
fn child_writer() {
    let Some(dir) = std::env::var_os("OB_STUDIO_KILL_CHILD") else { return };
    let dir = std::path::PathBuf::from(dir);
    let mut h = open(&dir, Arc::new(Real)).unwrap();
    assert!(h.writable(), "the dead writer's lock was not let go");
    h.restore();
    let name = MapMod::parse(h.doc(store::FILE).unwrap()).unwrap().name;
    let mut k: u64 = name.trim_start_matches('k').parse().unwrap();
    loop {
        k += 1;
        h.commit(STUDIO, "Step", put(&map(&format!("k{k}"), &[("r1", (k % 9 + 1) as u32)]))).unwrap();
        println!("made {k}");
        match k % 7 {
            0 => h.save().unwrap(),
            3 => {
                h.tick(Instant::now() + Duration::from_secs(3600)).unwrap();
            }
            5 if k % 35 == 5 => h.compact().unwrap(),
            _ => {}
        }
    }
}

#[test]
fn kill_9_at_varying_points_loses_nothing_reported() {
    let s = Scratch::new("history-kill");
    std::fs::write(s.path().join(store::FILE), map("k0", &[])).unwrap();
    let exe = std::env::current_exe().unwrap();
    let mut last_made = 0u64;
    for round in 0..12u64 {
        let mut child = std::process::Command::new(&exe)
            .args(["--exact", "child_writer", "--nocapture", "--test-threads=1"])
            .env("OB_STUDIO_KILL_CHILD", s.path())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let out = child.stdout.take().unwrap();
        // Kill after a varying number of reported steps (the child goes on writing meanwhile).
        let want = 3 + (round * 37) % 120;
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
            std::thread::sleep(Duration::from_millis(round % 7));
        }
        child.kill().unwrap();
        let _ = child.wait();
        assert!(seen >= want, "round {round}: the child stopped early");

        let h = open(s.path(), Arc::new(Real)).unwrap();
        assert!(h.writable(), "round {round}: the killed writer's lock is still held");
        let text = std::fs::read_to_string(s.path().join(store::FILE)).unwrap();
        MapMod::parse(&text).unwrap_or_else(|e| panic!("round {round}: the file is not whole: {e:?}"));
        let now = MapMod::parse(h.doc(store::FILE).unwrap()).unwrap();
        let k: u64 = now.name.trim_start_matches('k').parse().unwrap();
        assert!(k >= last_made, "round {round}: step {last_made} was reported made, the history has {k}");
    }
}
