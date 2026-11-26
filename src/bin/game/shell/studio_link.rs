//! Studio's link to a test run (`--studio-link <folder>`, `tools::map::link`).
//! Between two frames (the test loop's `about_to_wait`, never inside a
//! tick, so gameplay is the same at any frame rate) the game looks at
//! Studio's command file about ten times a second and does a new command:
//! read the mod again and start the run over with it, or just start over.
//! It writes its own state file about twice a second, so Studio can show
//! that the game is connected. Without the flag no `Link` exists and the
//! test loop does nothing more than before.
use crate::*;
use nfsu2::tools::map::link::{self, Aim, GameState, Op};

const POLL: std::time::Duration = std::time::Duration::from_millis(100);
const BEAT: std::time::Duration = std::time::Duration::from_millis(500);

pub(crate) struct Link {
    dir: std::path::PathBuf,
    seen: u64,
    polled: std::time::Instant,
    beat: std::time::Instant,
    ok: bool,
    note: String,
    /// Turns of the loop so far (the state file says them).
    turns: u64,
    /// The mod as the world was last made with it (an edit is applied in
    /// place against it).
    known: Option<nfsu2::tools::map::MapMod>,
    /// What the last command did, one line (the state file says it).
    done: String,
    /// `--map-mem-cap`: the resident memory (bytes) at which the run stops itself, and when it looked last.
    cap: Option<(u64, std::time::Instant)>,
}

/// The process's resident memory in bytes (Linux; None elsewhere).
fn resident_bytes() -> Option<u64> {
    let t = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb = t.lines().find_map(|l| l.strip_prefix("VmRSS:"))?.split_whitespace().next()?.parse::<u64>().ok()?;
    Some(kb * 1024)
}

/// The one mod this run applies, as the game has it now.
fn current_mod() -> Option<(String, nfsu2::tools::map::MapMod)> {
    nfsu2::tools::map::active().first().cloned()
}

impl Link {
    pub(crate) fn new(dir: std::path::PathBuf) -> Link {
        log::info!("map test: Studio link {}", dir.display());
        let mut l = Link { dir, seen: 0, polled: std::time::Instant::now(), beat: std::time::Instant::now(), ok: true, note: String::new(), turns: 0, known: current_mod().map(|m| m.1), done: String::new(), cap: None };
        l.write();
        l
    }

    /// Stop the run when its memory passes `mb` MiB (`--map-mem-cap`).
    pub(crate) fn with_cap(mut self, mb: Option<u64>) -> Link {
        self.cap = mb.map(|m| (m << 20, std::time::Instant::now()));
        self
    }

    /// Whether the run is over its memory cap (looked at about once a second); says so over the link when it is.
    fn over_cap(&mut self) -> bool {
        let Some((cap, at)) = self.cap.as_mut() else { return false };
        if at.elapsed() < std::time::Duration::from_secs(1) {
            return false;
        }
        *at = std::time::Instant::now();
        let cap = *cap;
        let Some(used) = resident_bytes().filter(|&u| u > cap) else { return false };
        let note = format!("the game stopped: it used {} MiB, over the cap of {} MiB", used >> 20, cap >> 20);
        log::error!("Studio link: {note}");
        (self.ok, self.note) = (false, note);
        self.write();
        true
    }

    fn write(&mut self) {
        self.beat = std::time::Instant::now();
        let s = GameState { pid: std::process::id(), seq: self.seen, beat_ms: link::now_ms(), ok: self.ok, note: self.note.clone(), turns: self.turns };
        if let Err(e) = link::write_state(&self.dir, &s) {
            log::warn!("Studio link: state not written: {e}");
        }
    }

    /// One look between two frames. True: Studio asked the run to end.
    pub(crate) fn poll(&mut self, game: &mut Game, test: &mut super::cli::MapTest, watch: &mut super::cli::EndWatch) -> bool {
        self.turns += 1;
        if self.over_cap() {
            return true;
        }
        let Some(c) = self.next() else { return false };
        match c.op {
            Op::Probe | Op::Stop => {
                // Answered at once, nothing started over.
                self.finish(Ok(()), c.seq);
                return c.op == Op::Stop;
            }
            Op::Apply | Op::Restart => {}
        }
        let r = apply(game, test, &c, &mut self.known, &mut self.done);
        let restarted = r == Ok(true);
        self.finish(r.map(|_| ()), c.seq);
        // The run began again: the end watch starts over with it.
        if restarted {
            *watch = super::cli::EndWatch::new(test);
        }
        false
    }

    /// One look of a headless run (a simulation): probe and stop only, as
    /// there is no window to start over. True: end the run.
    pub(crate) fn poll_headless(&mut self) -> bool {
        self.turns += 1;
        if self.over_cap() {
            return true;
        }
        let Some(c) = self.next() else { return false };
        let r = match c.op {
            Op::Probe | Op::Stop => Ok(()),
            Op::Apply | Op::Restart => Err("a simulation is not started over; start another".to_string()),
        };
        self.finish(r, c.seq);
        c.op == Op::Stop
    }

    /// The new command, when it is time to look and there is one; else the
    /// beat when it is due.
    fn next(&mut self) -> Option<link::Command> {
        if self.polled.elapsed() < POLL {
            return None;
        }
        self.polled = std::time::Instant::now();
        let c = link::read_command(&self.dir).filter(|c| c.seq > self.seen);
        if c.is_none() && self.beat.elapsed() >= BEAT {
            self.write();
        }
        c
    }

    fn finish(&mut self, r: Result<(), String>, seq: u64) {
        self.seen = seq;
        match r {
            Ok(()) => (self.ok, self.note) = (true, std::mem::take(&mut self.done)),
            Err(e) => {
                log::warn!("Studio link: command {seq} not done: {e}");
                (self.ok, self.note) = (false, e);
            }
        }
        self.write();
    }
}

/// Do `c` (an apply or restart): the aim it names (else the run's own), the mod read again for
/// `Apply` (free roam then goes on from where the car is), the run started
/// over. The run's aim changes only when it started.
fn apply(
    game: &mut Game,
    test: &mut super::cli::MapTest,
    c: &link::Command,
    known: &mut Option<nfsu2::tools::map::MapMod>,
    done: &mut String,
) -> Result<bool, String> {
    // An edit alone (no new aim, no new car or clock) goes into the running
    // world; what cannot goes the way of a restart, and says why.
    let mut why = None;
    if c.op == Op::Apply && c.aim.is_none() && c.focus.is_none() {
        if let Some(o) = nfsu2::tools::map::only() {
            nfsu2::tools::map::store::read_file(&o.path(&nfsu2::tools::map::store::maps_dir()))
                .map_err(|e| format!("the mod: {e:#}"))?;
        }
        nfsu2::tools::map::reload_active();
        match (known.as_ref(), current_mod()) {
            (Some(old), Some((folder, new))) => match game.apply_map_live(old, &new, &folder) {
                Ok(what) => {
                    *known = Some(new);
                    *done = what.to_string();
                    return Ok(false);
                }
                Err(e) => why = Some(e),
            },
            _ => why = Some("no mod known to the run".into()),
        }
    }
    let mut t = test.clone();
    if let Some(f) = &c.focus {
        if !f.valid() {
            return Err("the car or clock is not one the game takes".into());
        }
        (t.car, t.clock) = (f.car.clone(), f.clock);
    }
    match &c.aim {
        Some(Aim::Race(id)) => {
            if !nfsu2::tools::map::valid_id(id) {
                return Err(format!("`{id}` is not a race id"));
            }
            (t.race, t.at) = (Some(id.clone()), None);
        }
        Some(Aim::At(x, y, z, h)) => {
            let ok = |v: f32| v.is_finite() && v.abs() <= nfsu2::tools::map::MAX_COORD;
            if !(ok(*x) && ok(*y) && ok(*z) && h.is_finite()) {
                return Err("the spot is out of range".into());
            }
            (t.race, t.at) = (None, Some((Vec3::new(*x, *y, *z), *h)));
        }
        None if c.op == Op::Apply && t.race.is_none() => {
            if let Some((at, heading)) = game.here() {
                t.at = Some((at, heading.to_degrees()));
            }
        }
        None => {}
    }
    if c.op == Op::Apply {
        // A file that cannot be read would load as no mod at all: refused.
        if let Some(o) = nfsu2::tools::map::only() {
            nfsu2::tools::map::store::read_file(&o.path(&nfsu2::tools::map::store::maps_dir()))
                .map_err(|e| format!("the mod: {e:#}"))?;
        }
        nfsu2::tools::map::reload_active();
    }
    // A recording ends where the run is started over (its file stays as far as it got).
    game.end_recording("restart");
    game.restart_map_test(&t).map_err(|e| format!("{e:#}"))?;
    *test = t;
    *known = current_mod().map(|m| m.1);
    *done = why.map_or_else(|| "reloaded".to_string(), |w| format!("reloaded: {w}"));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nfsu2::tools::map::link::Command;

    fn send(dir: &std::path::Path, seq: u64, op: Op) {
        link::write_command(dir, &Command { seq, op, aim: None, focus: None }).unwrap();
        std::thread::sleep(POLL + std::time::Duration::from_millis(20));
    }

    /// A run over its memory cap says so over the link and ends; under it (or with no cap) it goes on.
    #[test]
    fn a_run_over_its_memory_cap_stops_and_says_so() {
        let d = std::env::temp_dir().join(format!("ob-link-cap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let mut l = Link::new(d.clone()).with_cap(Some(1_000_000));
        l.cap.as_mut().unwrap().1 -= std::time::Duration::from_secs(2);
        assert!(!l.poll_headless(), "far under a cap of a million MiB");
        let mut l = Link::new(d.clone()).with_cap(Some(1));
        l.cap.as_mut().unwrap().1 -= std::time::Duration::from_secs(2);
        if resident_bytes().is_some() {
            assert!(l.poll_headless(), "over a 1 MiB cap");
            let g = link::read_state(&d).unwrap();
            assert!(!g.ok && g.note.contains("cap"), "{g:?}");
        }
        let mut l = Link::new(d.clone());
        assert!(!l.poll_headless(), "no cap, no stop");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A headless run answers a probe at once, says it cannot start over,
    /// and ends on a stop; each command is dealt with once.
    #[test]
    fn a_simulation_answers_probes_and_stops() {
        let d = std::env::temp_dir().join(format!("ob-link-headless-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let mut l = Link::new(d.clone());
        assert_eq!(link::read_state(&d).unwrap().seq, 0, "alive before any command");
        std::thread::sleep(POLL + std::time::Duration::from_millis(20));
        assert!(!l.poll_headless(), "nothing sent");
        send(&d, 1, Op::Probe);
        assert!(!l.poll_headless());
        let g = link::read_state(&d).unwrap();
        assert_eq!((g.seq, g.ok), (1, true), "probed: answered");
        assert!(g.turns >= 2, "the loop's turns are said: {}", g.turns);
        send(&d, 2, Op::Apply);
        assert!(!l.poll_headless(), "a simulation is not started over");
        let g = link::read_state(&d).unwrap();
        assert_eq!((g.seq, g.ok), (2, false));
        assert!(g.note.contains("simulation"), "{}", g.note);
        send(&d, 2, Op::Stop);
        assert!(!l.poll_headless(), "a command number is dealt with once");
        send(&d, 3, Op::Stop);
        assert!(l.poll_headless(), "stop ends the run");
        assert_eq!(link::read_state(&d).unwrap().seq, 3);
        let _ = std::fs::remove_dir_all(&d);
    }
}
