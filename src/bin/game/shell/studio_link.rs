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
}

impl Link {
    pub(crate) fn new(dir: std::path::PathBuf) -> Link {
        log::info!("map test: Studio link {}", dir.display());
        let mut l = Link { dir, seen: 0, polled: std::time::Instant::now(), beat: std::time::Instant::now(), ok: true, note: String::new(), turns: 0 };
        l.write();
        l
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
        let Some(c) = self.next() else { return false };
        match c.op {
            Op::Probe | Op::Stop => {
                // Answered at once, nothing started over.
                self.finish(Ok(()), c.seq);
                return c.op == Op::Stop;
            }
            Op::Apply | Op::Restart => {}
        }
        let r = apply(game, test, &c);
        self.finish(r, c.seq);
        // The run began again: the end watch starts over with it.
        *watch = super::cli::EndWatch::new(test);
        false
    }

    /// One look of a headless run (a simulation): probe and stop only, as
    /// there is no window to start over. True: end the run.
    pub(crate) fn poll_headless(&mut self) -> bool {
        self.turns += 1;
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
            Ok(()) => (self.ok, self.note) = (true, String::new()),
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
fn apply(game: &mut Game, test: &mut super::cli::MapTest, c: &link::Command) -> Result<(), String> {
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nfsu2::tools::map::link::Command;

    fn send(dir: &std::path::Path, seq: u64, op: Op) {
        link::write_command(dir, &Command { seq, op, aim: None, focus: None }).unwrap();
        std::thread::sleep(POLL + std::time::Duration::from_millis(20));
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
