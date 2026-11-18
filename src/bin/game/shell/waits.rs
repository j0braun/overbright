//! The game's waits and the drives' frame times in its own log
//! (docs/waits-log.md; the lines and their logic are
//! `nfsu2::qol::waits`'s). The frame loop hands each window frame here
//! (`redraw`); a key, a click or a controller's button is noted as it
//! comes; `drive_in` notes its load's parts. Nothing here changes what is
//! drawn, heard or simulated: it reads clocks, counters and the mode, and
//! with the log off (`waits::enabled`) it does nothing but note keys.
use crate::*;
use nfsu2::qol::waits;

/// The screen a frame showed, for the log: the mode, the menu, its kind,
/// a boot movie's step and whether a race is a replay (compared once a
/// frame, no allocation).
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Screen(
    std::mem::Discriminant<Mode>,
    Option<std::mem::Discriminant<Menu>>,
    Option<MenuKind>,
    Option<BootStep>,
    bool,
);

#[derive(Default)]
pub(crate) struct Waits {
    tracker: waits::Tracker<Screen>,
    /// The start's parts (ms since `main`), until its line is written.
    start: Vec<(&'static str, f64)>,
    started_logged: bool,
    /// The screen the diagnostics' events last named.
    event_screen: Option<Screen>,
    /// Tests: the lines kept here instead of the log, with the hooks on
    /// whatever the log does.
    #[cfg(test)]
    pub(crate) captured: Option<Vec<String>>,
}

impl Waits {
    /// Whether the hooks work (`waits::enabled`).
    fn on(&self) -> bool {
        #[cfg(test)]
        if self.captured.is_some() {
            return true;
        }
        waits::enabled()
    }

    /// A line to the log (`waits::log`).
    fn emit(&mut self, line: String) {
        #[cfg(test)]
        if let Some(c) = self.captured.as_mut() {
            c.push(line);
            return;
        }
        waits::log(&line);
    }

    /// A key, a mouse button or a controller's button pressed now.
    pub(crate) fn key(&mut self) {
        self.tracker.key(Instant::now());
    }

    /// A part of the start done now.
    pub(crate) fn start_part(&mut self, what: &'static str) {
        if !self.started_logged && !self.start.iter().any(|(w, _)| *w == what) {
            self.start.push((what, waits::since_start_ms()));
        }
    }

    /// A drive's load begins (`drive_in`).
    pub(crate) fn load_begin(&mut self, what: &str) {
        nfsu2::diag::events::record(nfsu2::diag::events::Kind::Load, format!("{what}: loading"));
        if self.on() {
            self.tracker.load = Some(waits::Load::begin(what.to_string(), Instant::now(), counters()));
        }
    }

    /// The load's part that ends now.
    pub(crate) fn load_part(&mut self, what: &'static str) {
        if let Some(l) = self.tracker.load.as_mut() {
            l.part(what, Instant::now());
        }
    }

    /// The load ended; a failed one is no wait of its own (its error is
    /// logged where it is handled).
    pub(crate) fn load_end(&mut self, ok: bool) {
        nfsu2::diag::events::record(
            nfsu2::diag::events::Kind::Load,
            if ok { "load done" } else { "load FAILED" },
        );
        self.load_part("the rest");
        if !ok {
            self.tracker.load = None;
        }
    }

    pub(crate) fn load_mut(&mut self) -> Option<&mut waits::Load> {
        self.tracker.load.as_mut()
    }
}

/// What the process has done, for a load's line.
fn counters() -> waits::Counters {
    let (cache_hits, cache_misses) = nfsu2::cache::open_counts();
    waits::Counters { pipelines: nfsu2::render::pipelines_built(), cache_hits, cache_misses }
}

/// The screen's name in the log: the mode, a menu's kind, a boot
/// movie's step or a movie's name (the game's own names, nothing of the
/// player's).
fn screen_name(game: &Game) -> String {
    match &game.mode {
        Mode::Menu(Menu::Movie(m)) => match m.boot {
            Some(b) => format!("boot {b:?}"),
            None => format!("movie {}", m.queue.get(m.current).map_or("-", |s| s.as_str())),
        },
        Mode::Menu(Menu::Icons(_, k)) => format!("menu {k:?}"),
        Mode::Explore => "free roam".into(),
        Mode::Race(r) => {
            if r.replay.playback.is_some() {
                "replay".into()
            } else {
                "race".into()
            }
        }
        // The input tag's first field: the menu's variant name.
        _ => format!("menu {}", game.input_tag().split(':').next().unwrap_or("")),
    }
}

impl Game {
    fn wait_screen(&self) -> Screen {
        let (menu, kind, boot) = match &self.mode {
            Mode::Menu(m) => (
                Some(std::mem::discriminant(m)),
                if let Menu::Icons(_, k) = m { Some(*k) } else { None },
                if let Menu::Movie(mv) = m { mv.boot } else { None },
            ),
            _ => (None, None, None),
        };
        let replay = matches!(&self.mode, Mode::Race(r) if r.replay.playback.is_some());
        Screen(std::mem::discriminant(&self.mode), menu, kind, boot, replay)
    }

    /// A window frame drawn (`redraw`, after its drawing): `interval_ms`
    /// since the frame before began.
    pub(crate) fn waits_frame(&mut self, interval_ms: f32) {
        // The diagnostics' events: a new screen (one compare a frame).
        let screen = self.wait_screen();
        if self.loads.waits.event_screen != Some(screen) {
            self.loads.waits.event_screen = Some(screen);
            nfsu2::diag::events::record(nfsu2::diag::events::Kind::Screen, screen_name(self));
        }
        if !self.loads.waits.on() {
            return;
        }
        let race = match &self.mode {
            Mode::Race(r) if r.replay.playback.is_none() => Some(r.timing.clock),
            _ => None,
        };
        let f = waits::FrameIn {
            screen: self.wait_screen(),
            driving: matches!(self.mode, Mode::Explore) || race.is_some(),
            foreground: self.pacing.focused && !self.pacing.occluded,
            race_clock: race,
            race_paused: matches!(&self.mode, Mode::Menu(Menu::Icons(_, MenuKind::RacePause))),
            interval_ms,
            now: Instant::now(),
        };
        // The tracker out of the game while it names the screen (no
        // allocation: a `Tracker` holds no buffer of its own when empty).
        let mut tracker = std::mem::take(&mut self.loads.waits.tracker);
        let mut lines = Vec::new();
        tracker.frame(&f, || screen_name(self), counters, &mut |l| lines.push(l));
        self.loads.waits.tracker = tracker;
        for l in lines {
            self.loads.waits.emit(l);
        }
        if !self.loads.waits.started_logged {
            self.loads.waits.started_logged = true;
            let first = self.loads.waits.tracker.first_screen().unwrap_or("-").to_string();
            let start = std::mem::take(&mut self.loads.waits.start);
            let line = waits::start_line(waits::since_start_ms(), &first, &start, waits::age_at_main_ms());
            // The cache folder's state read on a thread of its own: its
            // files' sizes may be cold on disk after a reboot.
            let dir = nfsu2::cache::dir();
            #[cfg(test)]
            if self.loads.waits.captured.is_some() {
                self.loads.waits.emit(format!("{line}; {}", waits::cache_state(dir.as_deref())));
                return;
            }
            let line2 = line.clone();
            let scan = std::thread::Builder::new()
                .name("waits-cache".into())
                .spawn(move || waits::log(&format!("{line2}; {}", waits::cache_state(dir.as_deref()))));
            if scan.is_err() {
                waits::log(&format!("{line}; cache not read"));
            }
        }
    }

    /// The game's end (`exit`, after the window is hidden): the drive's
    /// line, if one is on, and the run's length, in the file before the
    /// process ends (at most a short wait for the log's thread).
    pub(crate) fn waits_end(&mut self) {
        if !self.loads.waits.on() {
            return;
        }
        let mut lines = Vec::new();
        self.loads.waits.tracker.end(&mut |l| lines.push(l));
        lines.push(format!("wait: the game ran {:.0} s", waits::since_start_ms() / 1000.0));
        for l in lines {
            self.loads.waits.emit(l);
        }
        nfsu2::qol::log_file::flush(std::time::Duration::from_millis(200));
    }
}
