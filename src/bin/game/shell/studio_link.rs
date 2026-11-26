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
}

impl Link {
    pub(crate) fn new(dir: std::path::PathBuf) -> Link {
        log::info!("map test: Studio link {}", dir.display());
        let mut l = Link { dir, seen: 0, polled: std::time::Instant::now(), beat: std::time::Instant::now(), ok: true, note: String::new() };
        l.write();
        l
    }

    fn write(&mut self) {
        self.beat = std::time::Instant::now();
        let s = GameState { pid: std::process::id(), seq: self.seen, beat_ms: link::now_ms(), ok: self.ok, note: self.note.clone() };
        if let Err(e) = link::write_state(&self.dir, &s) {
            log::warn!("Studio link: state not written: {e}");
        }
    }

    /// One look between two frames.
    pub(crate) fn poll(&mut self, game: &mut Game, test: &mut super::cli::MapTest, watch: &mut super::cli::EndWatch) {
        if self.polled.elapsed() < POLL {
            return;
        }
        self.polled = std::time::Instant::now();
        match link::read_command(&self.dir).filter(|c| c.seq > self.seen) {
            Some(c) => {
                self.seen = c.seq;
                match apply(game, test, &c) {
                    Ok(()) => (self.ok, self.note) = (true, String::new()),
                    Err(e) => {
                        log::warn!("Studio link: command {} not done: {e}", c.seq);
                        (self.ok, self.note) = (false, e);
                    }
                }
                // The run began again: the end watch starts over with it.
                *watch = super::cli::EndWatch::new(test);
                self.write();
            }
            None if self.beat.elapsed() >= BEAT => self.write(),
            None => {}
        }
    }
}

/// Do `c`: the aim it names (else the run's own), the mod read again for
/// `Apply` (free roam then goes on from where the car is), the run started
/// over. The run's aim changes only when it started.
fn apply(game: &mut Game, test: &mut super::cli::MapTest, c: &link::Command) -> Result<(), String> {
    let mut t = test.clone();
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
    game.restart_map_test(&t).map_err(|e| format!("{e:#}"))?;
    *test = t;
    Ok(())
}
