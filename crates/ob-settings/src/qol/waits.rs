//! What a player waits for and how smooth the drive was, in the game's own
//! log file (docs/developing/waits-log.md). A plain game run writes, at Info and to
//! its log file even while that keeps only warnings (`log_file::fact`):
//!
//! * `wait: start …` once, after the window's first frame: the time from
//!   `main`, its parts, the process's age at `main`, and the cache folder
//!   as the start found it;
//! * `wait: <from> -> <to> …` at the first frame of every new screen (and
//!   of a drive loaded again on the same screen): the time from the key
//!   that asked for it, and for a drive's load its parts;
//! * `wait: race -> green light …` when a race's countdown ends;
//! * `drive: …` when a drive's frames end (another screen, the game's end):
//!   its frame times as a summary.
//!
//! Nothing is sent anywhere and nothing but times, counts, screen, movie
//! and region names and cache sizes is written (no paths, names or
//! settings). The logic here is pure (the clock is passed in); the game's
//! hooks are `src/bin/game/shell/waits.rs`.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

/// The process's start as the game sees it: the first call (made first
/// thing in `main`).
pub fn started() -> Instant {
    *START.get_or_init(Instant::now)
}

/// Milliseconds since [`started`].
pub fn since_start_ms() -> f64 {
    started().elapsed().as_secs_f64() * 1000.0
}

/// How old the process was when `main` began (the program's loading by
/// the system: the exec, the dynamic loader, the C runtime), from the
/// system's own record of its start: Linux `/proc` (10 ms steps), Windows
/// `GetProcessTimes` (the system clock's steps); None elsewhere or when
/// the system does not say. Read once.
pub fn age_at_main_ms() -> Option<f64> {
    static AGE: OnceLock<Option<f64>> = OnceLock::new();
    *AGE.get_or_init(|| process_age_ms().map(|age| (age - since_start_ms()).max(0.0)))
}

#[cfg(target_os = "linux")]
fn process_age_ms() -> Option<f64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // Fields after the command's closing parenthesis; field 22 of the
    // whole line (starttime, clock ticks since boot) is the 20th of those.
    let rest = stat.rsplit_once(')')?.1;
    let ticks: f64 = rest.split_whitespace().nth(19)?.parse().ok()?;
    let uptime: f64 = std::fs::read_to_string("/proc/uptime").ok()?.split_whitespace().next()?.parse().ok()?;
    // _SC_CLK_TCK (USER_HZ) is 100 on every Linux the game is built for.
    Some(uptime * 1000.0 - ticks * 10.0)
}

#[cfg(windows)]
fn process_age_ms() -> Option<f64> {
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn GetProcessTimes(
            process: *mut std::ffi::c_void,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel_time: *mut FileTime,
            user_mode_time: *mut FileTime,
        ) -> i32;
        fn GetSystemTimeAsFileTime(now: *mut FileTime);
    }
    let (mut creation, mut exit, mut kernel_time, mut user_mode_time, mut now) =
        (FileTime::default(), FileTime::default(), FileTime::default(), FileTime::default(), FileTime::default());
    // SAFETY: the process's own pseudo handle and pointers to locals of
    // the layout the API writes.
    if unsafe { GetProcessTimes(GetCurrentProcess(), &mut creation, &mut exit, &mut kernel_time, &mut user_mode_time) }
        == 0
    {
        return None;
    }
    // SAFETY: a pointer to a local of the layout the API writes.
    unsafe { GetSystemTimeAsFileTime(&mut now) };
    // 100 ns units since 1601.
    let ms = |t: &FileTime| ((u64::from(t.high) << 32) | u64::from(t.low)) as f64 / 10_000.0;
    Some(ms(&now) - ms(&creation))
}

#[cfg(not(any(target_os = "linux", windows)))]
fn process_age_ms() -> Option<f64> {
    None
}

/// Whether the lines go anywhere: the game's log file is being written,
/// or the log shows Info (`RUST_LOG`). Read once, at the first frame (the
/// log is set up first thing in `main`). Off, the hooks do nothing.
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| super::log_file::active() || log::log_enabled!(target: "game", log::Level::Info))
}

/// The most lines one run writes ([`log`]): a bound on the log file
/// whatever happens (a screen that changed every frame would otherwise
/// write a line a frame). A long session writes a few hundred.
pub const MAX_LINES: usize = 20_000;

static WRITTEN: AtomicUsize = AtomicUsize::new(0);

/// What [`log`] does with its `n`th line (from 0).
#[derive(Debug, PartialEq)]
pub enum Admit {
    Write,
    /// The first line over [`MAX_LINES`]: a note that the rest is not written.
    Note,
    Drop,
}

pub fn admit(n: usize) -> Admit {
    match n.cmp(&MAX_LINES) {
        std::cmp::Ordering::Less => Admit::Write,
        std::cmp::Ordering::Equal => Admit::Note,
        std::cmp::Ordering::Greater => Admit::Drop,
    }
}

/// One line of the log at Info (`wait: …`, `drive: …`), at most
/// [`MAX_LINES`] a run.
pub fn log(line: &str) {
    match admit(WRITTEN.fetch_add(1, Ordering::Relaxed)) {
        Admit::Write => super::log_file::fact(line),
        Admit::Note => {
            super::log_file::fact(&format!("wait: {MAX_LINES} wait and drive lines written; no more this run"))
        }
        Admit::Drop => {}
    }
}

/// The start's line (without the cache folder's state, which a thread
/// adds: [`cache_state`]): `total_ms` from `main` to the end of the first
/// frame, the parts' times from `main`, the process's age at `main`.
pub fn start_line(total_ms: f64, first: &str, parts: &[(&str, f64)], age_ms: Option<f64>) -> String {
    let parts: Vec<String> = parts.iter().map(|(what, ms)| format!("{what} {ms:.0}")).collect();
    let age = age_ms.map_or(String::new(), |a| format!("; the process {a:.0} ms old at main"));
    format!("wait: start -> first frame ({first}) {total_ms:.0} ms ({}{age})", parts.join(", "))
}

/// The frames one `drive:` line sums up at most: a longer drive is written
/// in parts of this many (1 MB of frame times; 14.6 min at 300 fps), so the
/// memory a drive's frames take and a part's summary (about 1 ms on the
/// main thread, once a part) are bounded.
pub const PART_FRAMES: usize = 1 << 18;

/// A drive's frame intervals (ms), from its first frame to its end.
pub struct Frames {
    what: String,
    ms: Vec<f32>,
    /// The part being filled (1 until the drive outgrows [`PART_FRAMES`]).
    part: u32,
    part_frames: usize,
}

impl Frames {
    pub fn new(what: &str) -> Frames {
        Frames::with_part_frames(what, PART_FRAMES)
    }

    /// [`Frames::new`] with parts of `part_frames` (tests).
    pub fn with_part_frames(what: &str, part_frames: usize) -> Frames {
        Frames {
            what: what.into(),
            ms: Vec::with_capacity(4096.min(part_frames)),
            part: 1,
            part_frames: part_frames.max(1),
        }
    }

    /// One frame's interval; the line of the part it filled, if it did
    /// (the next part starts empty).
    pub fn push(&mut self, ms: f32) -> Option<String> {
        self.ms.push(ms);
        if self.ms.len() < self.part_frames {
            return None;
        }
        let line = self.part_line(true);
        self.part += 1;
        self.ms.clear();
        line
    }

    /// The drive's (last part's) line at its end: None for under 10 frames.
    pub fn line(&self) -> Option<String> {
        self.part_line(self.part > 1)
    }

    fn part_line(&self, parted: bool) -> Option<String> {
        let s = summarize(&self.ms)?;
        let what = if parted { format!("{} (part {})", self.what, self.part) } else { self.what.clone() };
        Some(format!("drive: {what}, {s}"))
    }
}

/// A frame-time summary: the median, the 99th percentile (the "1 % low"
/// as the performance overlay counts it: 1000 / p99), the hitches (frames
/// over twice the median and over 4 ms) and the three longest frames.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub frames: usize,
    pub secs: f64,
    pub median_ms: f32,
    pub p99_ms: f32,
    pub hitches: usize,
    pub hitch_ms: f64,
    /// The hitches in the first 60 s (a load's first minute).
    pub first_minute_hitches: usize,
    /// The three longest frames (ms) and when they ended (s from the
    /// first frame's start), longest first; the earlier first if equal.
    pub longest: Vec<(f32, f64)>,
}

/// Frames per second of a frame time (0 for none).
pub fn fps(ms: f32) -> f32 {
    if ms > 0.0 { 1000.0 / ms } else { 0.0 }
}

impl std::fmt::Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let longest: Vec<String> = self.longest.iter().map(|(ms, at)| format!("{ms:.1} ms at {at:.1} s")).collect();
        write!(
            f,
            "{:.0} s, {} frames, median {:.2} ms ({:.0} fps), 1% low {:.0} fps (p99 {:.2} ms), \
             hitches over 2x median: {} ({:.0} ms in all, {} in the first minute), longest {}",
            self.secs,
            self.frames,
            self.median_ms,
            fps(self.median_ms),
            fps(self.p99_ms),
            self.p99_ms,
            self.hitches,
            self.hitch_ms,
            self.first_minute_hitches,
            longest.join(", ")
        )
    }
}

/// The summary of frame intervals `ms` (None under 10). O(n): two
/// selections on a copy and one pass.
pub fn summarize(ms: &[f32]) -> Option<Summary> {
    if ms.len() < 10 {
        return None;
    }
    let mut sorted = ms.to_vec();
    let median = crate::prof::percentile(&mut sorted, 0.5);
    let p99 = crate::prof::percentile(&mut sorted, 0.99);
    drop(sorted);
    let line = (2.0 * median).max(4.0);
    let (mut t, mut hitches, mut hitch_ms, mut first_minute) = (0.0f64, 0, 0.0f64, 0);
    let mut longest: Vec<(f32, f64)> = Vec::with_capacity(4);
    for &m in ms {
        t += f64::from(m) / 1000.0;
        if m > line {
            hitches += 1;
            hitch_ms += f64::from(m);
            first_minute += usize::from(t <= 60.0);
        }
        // Kept longest first; an equal later frame does not displace.
        if longest.len() < 3 || m > longest[2].0 {
            let at = longest.iter().position(|&(l, _)| m > l).unwrap_or(longest.len());
            longest.insert(at, (m, t));
            longest.truncate(3);
        }
    }
    Some(Summary {
        frames: ms.len(),
        secs: t,
        median_ms: median,
        p99_ms: p99,
        hitches,
        hitch_ms,
        first_minute_hitches: first_minute,
        longest,
    })
}

/// The cache folder as a start finds it: its files and their size, the
/// graphics driver's pipeline cache kept there (MB), and whether the
/// launcher's prepare step's stamp is there (docs/developing/prepare.md).
pub fn cache_state(dir: Option<&std::path::Path>) -> String {
    let Some(dir) = dir else { return "no cache folder".into() };
    let Ok(entries) = std::fs::read_dir(dir) else { return "cache folder empty (new)".into() };
    let (mut files, mut bytes, mut pipelines, mut stamp) = (0u64, 0u64, 0u64, false);
    for e in entries.flatten() {
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        files += 1;
        bytes += meta.len();
        if name.starts_with("wgpu_pipeline_cache") {
            pipelines += meta.len();
        }
        stamp |= name == "prepared.txt";
    }
    format!(
        "cache {files} files {:.0} MB, pipeline cache {:.1} MB, prepare stamp {}",
        bytes as f64 / 1e6,
        pipelines as f64 / 1e6,
        if stamp { "yes" } else { "no" }
    )
}

/// What the process had done when a load began: pipelines built
/// (`render::pipelines_built`) and cache files opened and missed
/// (`cache::open_counts`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counters {
    pub pipelines: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// A drive's load (`drive_in`): its parts so far and what it took.
pub struct Load {
    pub what: String,
    begun: Instant,
    last: Instant,
    parts: Vec<(&'static str, f32)>,
    at_begin: Counters,
    /// Where the world came from: kept, loaded ahead (by the menus), loaded.
    pub world: &'static str,
    /// Where the car came from, the same way.
    pub car: &'static str,
    /// What the set-up after the load adds (a race's rivals loaded ahead).
    pub notes: Vec<String>,
    /// PC_Loading went up.
    pub shown: bool,
}

impl Load {
    pub fn begin(what: String, now: Instant, counters: Counters) -> Load {
        Load {
            what,
            begun: now,
            last: now,
            parts: Vec::new(),
            at_begin: counters,
            world: "kept by the drive",
            car: "kept by the drive",
            notes: Vec::new(),
            shown: false,
        }
    }

    /// The part that ends at `now`.
    pub fn part(&mut self, what: &'static str, now: Instant) {
        self.parts.push((what, (now - self.last).as_secs_f32() * 1000.0));
        self.last = now;
    }

    /// The load's half of its screen's line, at the end of the first frame
    /// (`now`) with the counters then.
    pub fn describe(&self, now: Instant, counters: Counters) -> String {
        let total = (self.last - self.begun).as_secs_f32() * 1000.0;
        let parts: Vec<String> = self.parts.iter().map(|(w, ms)| format!("{w} {ms:.0}")).collect();
        let b = self.at_begin;
        let notes: String = self.notes.iter().map(|n| format!(", {n}")).collect();
        format!(
            "load of {} {total:.0} ms ({}), world {}, car {}{notes}, cache files {} read {} missing, {} programs made, \
             loading screen {}; first frame {:.0} ms",
            self.what,
            parts.join(", "),
            self.world,
            self.car,
            counters.cache_hits.saturating_sub(b.cache_hits),
            counters.cache_misses.saturating_sub(b.cache_misses),
            counters.pipelines.saturating_sub(b.pipelines),
            if self.shown { "shown" } else { "not shown" },
            now.saturating_duration_since(self.last).as_secs_f32() * 1000.0
        )
    }
}

/// A key or button older than this asked for nothing that comes now (a
/// movie's or a scene's own end).
pub const KEY_MAX_MS: f32 = 20_000.0;

/// One drawn frame as the hooks see it.
pub struct FrameIn<S> {
    /// The screen it showed (compared, never printed).
    pub screen: S,
    /// Free roam or a race being driven (not a replay, not a menu).
    pub driving: bool,
    /// The window was in the foreground for the frame (an interval spent
    /// behind another window is no frame time of the drive).
    pub foreground: bool,
    /// The race's clock (s from its green light, negative in the start
    /// scene and the countdown) while a race is on screen.
    pub race_clock: Option<f32>,
    /// The screen is the race's pause menu.
    pub race_paused: bool,
    /// From the start of the frame before to this one's start.
    pub interval_ms: f32,
    /// The end of the frame's drawing.
    pub now: Instant,
}

/// A race's start scene and countdown on screen.
struct Countdown {
    first: Instant,
    paused_ms: f32,
    /// The pause menu went up then, over the race clock's value.
    paused: Option<(Instant, f32)>,
}

/// The waits and the drives' frames of a run, fed once a frame
/// ([`Tracker::frame`]); the lines go to `out`.
pub struct Tracker<S> {
    key: Option<Instant>,
    screen: Option<S>,
    name: String,
    /// The drive's load done in this frame, logged with it.
    pub load: Option<Load>,
    frames: Option<Frames>,
    countdown: Option<Countdown>,
    race_clock: Option<f32>,
    /// The next frame's interval holds the frame that loaded or changed
    /// the screen (logged with the change): not a drive's frame.
    skip_next: bool,
}

impl<S> Default for Tracker<S> {
    fn default() -> Self {
        Tracker {
            key: None,
            screen: None,
            name: String::new(),
            load: None,
            frames: None,
            countdown: None,
            race_clock: None,
            skip_next: false,
        }
    }
}

impl<S: Copy + PartialEq> Tracker<S> {
    /// A key, a mouse button or a controller's button pressed at `at`.
    pub fn key(&mut self, at: Instant) {
        self.key = Some(at);
    }

    /// The first screen's name, once the first frame is drawn.
    pub fn first_screen(&self) -> Option<&str> {
        self.screen.is_some().then_some(self.name.as_str())
    }

    /// A frame drawn. `name` names its screen (called when it is new);
    /// `counters`, what the process has done (called with a load).
    pub fn frame(
        &mut self,
        f: &FrameIn<S>,
        name: impl FnOnce() -> String,
        counters: impl FnOnce() -> Counters,
        out: &mut impl FnMut(String),
    ) {
        let first = self.screen.is_none();
        if self.screen != Some(f.screen) || self.load.is_some() {
            let name = name();
            let from = std::mem::replace(&mut self.name, name.clone());
            let key = self.key.take().map(|k| f.now.saturating_duration_since(k).as_secs_f32() * 1000.0);
            let key = key.filter(|&k| k < KEY_MAX_MS);
            if !first {
                let mut line = format!(
                    "wait: {from} -> {name}: {}",
                    key.map_or("no key".into(), |k| format!("{k:.0} ms from the key"))
                );
                if let Some(l) = self.load.take() {
                    line += "; ";
                    line += &l.describe(f.now, counters());
                }
                out(line);
            }
            self.load = None;
            // A drive's frames end with its screen (the pause menu over it
            // included: its frames are no driving frames).
            if let Some(line) = self.frames.take().and_then(|d| d.line()) {
                out(line);
            }
            if f.driving {
                self.frames = Some(Frames::new(&name));
            }
            self.countdown_screen(f);
            self.skip_next = true;
            self.screen = Some(f.screen);
        } else if !f.foreground {
            self.skip_next = true;
        } else if std::mem::take(&mut self.skip_next) {
        } else if f.driving
            && let Some(line) = self.frames.as_mut().and_then(|d| d.push(f.interval_ms))
        {
            out(line);
        }
        // The race's green light: the start scene and the countdown are
        // the original's, not a load.
        if let (Some(clock), Some(c)) = (f.race_clock, self.countdown.as_ref())
            && c.paused.is_none()
            && clock >= 0.0
        {
            let ms = f.now.saturating_duration_since(c.first).as_secs_f32() * 1000.0 - c.paused_ms;
            let paused =
                if c.paused_ms > 0.0 { format!(", {:.0} ms paused not counted", c.paused_ms) } else { String::new() };
            out(format!(
                "wait: race -> green light {ms:.0} ms after the race's first frame (the start scene and the countdown{paused})"
            ));
            self.countdown = None;
        }
        if f.race_clock.is_some() {
            self.race_clock = f.race_clock;
        }
    }

    /// A new screen's countdown: a race's first frame before its green
    /// light starts one; the race's pause menu holds it; back in the same
    /// race (its clock not set back) it goes on.
    fn countdown_screen(&mut self, f: &FrameIn<S>) {
        match f.race_clock {
            Some(clock) if clock < 0.0 => {
                let resumed = self.countdown.as_mut().and_then(|c| {
                    let (at, _) = c.paused.filter(|&(_, then)| clock >= then)?;
                    c.paused_ms += f.now.saturating_duration_since(at).as_secs_f32() * 1000.0;
                    c.paused = None;
                    Some(())
                });
                if resumed.is_none() {
                    self.countdown = Some(Countdown { first: f.now, paused_ms: 0.0, paused: None });
                }
            }
            Some(_) => self.countdown = None,
            None if f.race_paused => {
                if let (Some(c), Some(clock)) = (self.countdown.as_mut(), self.race_clock) {
                    c.paused = Some((f.now, clock));
                }
            }
            None => self.countdown = None,
        }
    }

    /// The run's end: the drive's line, if one is on.
    pub fn end(&mut self, out: &mut impl FnMut(String)) {
        if let Some(line) = self.frames.take().and_then(|d| d.line()) {
            out(line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_steady_drive_has_no_hitches() {
        let ms = vec![10.0f32; 600];
        let s = summarize(&ms).unwrap();
        assert_eq!((s.frames, s.hitches, s.median_ms, s.p99_ms), (600, 0, 10.0, 10.0));
        assert!((s.secs - 6.0).abs() < 1e-6);
    }

    /// The 1 % low is the frame rate of the 99th percentile's frame time
    /// (index ⌊0.99 n⌋ of the sorted times, the F3 overlay's): with 1000
    /// frames, the 991st slowest-first frame decides.
    #[test]
    fn the_one_percent_low_is_the_99th_percentile_frame() {
        let drive = |slow: usize| {
            let mut ms = vec![10.0f32; 1000 - slow];
            ms.extend(std::iter::repeat_n(50.0f32, slow));
            summarize(&ms).unwrap()
        };
        // 10 frames of 50 ms in 1000: sorted[990] is the first of them.
        assert_eq!(drive(10).p99_ms, 50.0);
        assert_eq!(fps(drive(10).p99_ms), 20.0);
        // 9 of them: sorted[990] is still a 10 ms frame.
        assert_eq!(drive(9).p99_ms, 10.0);
        assert_eq!(fps(drive(9).p99_ms), 100.0);
        assert!(drive(10).to_string().contains("1% low 20 fps (p99 50.00 ms)"), "{}", drive(10));
        assert!(drive(9).to_string().contains("1% low 100 fps (p99 10.00 ms)"), "{}", drive(9));
    }

    #[test]
    fn the_drive_line_has_its_format() {
        let mut f = Frames::new("free roam");
        for i in 0..1000 {
            assert!(f.push(if i == 500 || i == 900 { 60.0 } else { 5.0 }).is_none());
        }
        let s = summarize(&f.ms).unwrap();
        assert_eq!((s.hitches, s.hitch_ms), (2, 120.0));
        assert!((s.longest[0].1 - (500.0 * 0.005 + 0.06)).abs() < 1e-9, "{s:?}");
        assert_eq!(
            f.line().unwrap(),
            "drive: free roam, 5 s, 1000 frames, median 5.00 ms (200 fps), 1% low 200 fps (p99 5.00 ms), \
             hitches over 2x median: 2 (120 ms in all, 2 in the first minute), \
             longest 60.0 ms at 2.6 s, 60.0 ms at 4.6 s, 5.0 ms at 0.0 s"
        );
    }

    #[test]
    fn the_longest_frames_are_the_three_longest_in_order() {
        let ms: Vec<f32> = (0..20).map(|i| [3.0, 9.0, 7.0, 9.0, 1.0][i % 5]).collect();
        let s = summarize(&ms).unwrap();
        let longest: Vec<f32> = s.longest.iter().map(|l| l.0).collect();
        assert_eq!(longest, [9.0, 9.0, 9.0]);
        // The earliest of the equal ones first.
        assert!(s.longest[0].1 < s.longest[1].1 && s.longest[1].1 < s.longest[2].1, "{s:?}");
        assert!((s.longest[0].1 - 0.012).abs() < 1e-9, "{s:?}");
    }

    #[test]
    fn a_long_drive_is_written_in_parts() {
        let mut f = Frames::with_part_frames("race", 100);
        let lines: Vec<String> = (0..250).filter_map(|_| f.push(8.0)).collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("drive: race (part 1), 1 s, 100 frames, median 8.00 ms"), "{}", lines[0]);
        assert!(lines[1].starts_with("drive: race (part 2), 1 s, 100 frames"), "{}", lines[1]);
        assert!(f.line().unwrap().starts_with("drive: race (part 3), 0 s, 50 frames"), "{:?}", f.line());
        assert!(f.ms.capacity() <= 4096, "the part's frames are kept, not the drive's");
    }

    #[test]
    fn a_short_drive_writes_no_summary() {
        let mut f = Frames::new("race");
        (0..9).for_each(|_| assert!(f.push(16.0).is_none()));
        assert!(f.line().is_none());
    }

    #[test]
    fn small_frames_are_no_hitches() {
        // Twice a 1 ms median is no stutter a player sees: the 4 ms floor.
        let ms: Vec<f32> = (0..100).map(|i| if i % 10 == 0 { 3.5 } else { 1.0 }).collect();
        assert_eq!(summarize(&ms).unwrap().hitches, 0);
    }

    #[test]
    fn the_log_stops_at_its_line_limit() {
        assert_eq!(admit(0), Admit::Write);
        assert_eq!(admit(MAX_LINES - 1), Admit::Write);
        assert_eq!(admit(MAX_LINES), Admit::Note);
        assert_eq!(admit(MAX_LINES + 1), Admit::Drop);
        assert_eq!(admit(usize::MAX), Admit::Drop);
    }

    #[test]
    fn the_start_line_has_its_format() {
        let line =
            start_line(147.4, "boot Blank", &[("data loaded", 38.2), ("window", 120.0), ("audio", 121.0)], Some(13.0));
        assert_eq!(
            line,
            "wait: start -> first frame (boot Blank) 147 ms (data loaded 38, window 120, audio 121; the process 13 ms old at main)"
        );
        assert_eq!(
            start_line(80.0, "menu Main", &[("data loaded", 30.0)], None),
            "wait: start -> first frame (menu Main) 80 ms (data loaded 30)"
        );
    }

    #[test]
    fn the_cache_state_counts_the_folder() {
        let dir = std::env::temp_dir().join(format!("waits-cache-{}", crate::unique_tag()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("track-L4RA.nfc"), vec![0u8; 600_000]).unwrap();
        std::fs::write(dir.join("wgpu_pipeline_cache_vulkan_1_2"), vec![0u8; 250_000]).unwrap();
        std::fs::write(dir.join("prepared.txt"), b"x").unwrap();
        assert_eq!(cache_state(Some(&dir)), "cache 3 files 1 MB, pipeline cache 0.2 MB, prepare stamp yes");
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(cache_state(Some(&dir)), "cache folder empty (new)");
        assert_eq!(cache_state(None), "no cache folder");
    }

    /// The hooks' frames for a test: screen `s` at `t` ms.
    fn frame(s: u8, t: u64, driving: bool, race_clock: Option<f32>, race_paused: bool, t0: Instant) -> FrameIn<u8> {
        FrameIn {
            screen: s,
            driving,
            foreground: true,
            race_clock,
            race_paused,
            interval_ms: 10.0,
            now: t0 + Duration::from_millis(t),
        }
    }

    fn run(tr: &mut Tracker<u8>, f: FrameIn<u8>, name: &str, out: &mut Vec<String>) {
        let c = Counters { pipelines: 5, cache_hits: 3, cache_misses: 1 };
        tr.frame(&f, || name.to_string(), || c, &mut |l| out.push(l));
    }

    #[test]
    fn a_screen_change_is_timed_from_its_key_with_its_load() {
        let t0 = Instant::now();
        let (mut tr, mut out) = (Tracker::<u8>::default(), Vec::new());
        run(&mut tr, frame(1, 0, false, None, false, t0), "menu Main", &mut out);
        assert!(out.is_empty(), "the first screen is the start line's");
        assert_eq!(tr.first_screen(), Some("menu Main"));
        tr.key(t0 + Duration::from_millis(100));
        let mut load = Load::begin(
            "L4RA".into(),
            t0 + Duration::from_millis(120),
            Counters { pipelines: 4, cache_hits: 1, cache_misses: 1 },
        );
        load.part("world and car", t0 + Duration::from_millis(300));
        load.part("the rest", t0 + Duration::from_millis(320));
        (load.world, load.car) = ("loaded ahead", "loaded");
        load.notes.push("rivals 2 of 3 loaded ahead".into());
        tr.load = Some(load);
        run(&mut tr, frame(2, 600, true, None, false, t0), "free roam", &mut out);
        assert_eq!(
            out,
            [
                "wait: menu Main -> free roam: 500 ms from the key; load of L4RA 200 ms (world and car 180, the rest 20), \
              world loaded ahead, car loaded, rivals 2 of 3 loaded ahead, cache files 2 read 0 missing, 1 programs made, loading screen not shown; \
              first frame 280 ms"
            ]
        );
        // The interval after the change holds the load's frame: not counted.
        out.clear();
        for i in 0..20 {
            run(&mut tr, frame(2, 610 + i * 10, true, None, false, t0), "free roam", &mut out);
        }
        // A key of over 20 s ago asked for nothing.
        tr.key(t0);
        run(&mut tr, frame(1, 25_000, false, None, false, t0), "menu ExplorePause", &mut out);
        assert_eq!(out[0], "wait: free roam -> menu ExplorePause: no key");
        assert!(out[1].starts_with("drive: free roam, 0 s, 19 frames, median 10.00 ms (100 fps)"), "{}", out[1]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn a_load_on_the_same_screen_is_a_wait_too() {
        let t0 = Instant::now();
        let (mut tr, mut out) = (Tracker::<u8>::default(), Vec::new());
        for i in 0..12 {
            run(&mut tr, frame(2, i * 10, true, None, false, t0), "free roam", &mut out);
        }
        tr.load = Some(Load::begin("L4RA".into(), t0 + Duration::from_millis(125), Counters::default()));
        run(&mut tr, frame(2, 400, true, None, false, t0), "free roam", &mut out);
        assert!(out[0].starts_with("wait: free roam -> free roam: no key; load of L4RA 0 ms ()"), "{out:?}");
        assert!(out[1].starts_with("drive: free roam, 0 s, 10 frames"), "{out:?}");
    }

    #[test]
    fn a_window_in_the_background_adds_no_frames() {
        let t0 = Instant::now();
        let (mut tr, mut out) = (Tracker::<u8>::default(), Vec::new());
        for i in 0..30 {
            let mut f = frame(2, i * 10, true, None, false, t0);
            f.foreground = !(10..15).contains(&i);
            f.interval_ms = if i == 15 { 5000.0 } else { 10.0 };
            run(&mut tr, f, "free roam", &mut out);
        }
        tr.end(&mut |l| out.push(l));
        // 30 frames less the first two, the five in the background and the
        // one back from it (its interval spans the time away).
        assert!(out[0].starts_with("drive: free roam, 0 s, 22 frames, median 10.00 ms"), "{out:?}");
        assert!(!out[0].contains("5000"), "{out:?}");
    }

    #[test]
    fn the_green_light_is_timed_without_the_pause() {
        let t0 = Instant::now();
        let (mut tr, mut out) = (Tracker::<u8>::default(), Vec::new());
        run(&mut tr, frame(1, 0, false, None, false, t0), "menu Dialog", &mut out);
        run(&mut tr, frame(2, 1000, true, Some(-8.0), false, t0), "race", &mut out);
        run(&mut tr, frame(2, 3000, true, Some(-6.0), false, t0), "race", &mut out);
        // Paused for 5 s, then the same countdown goes on.
        run(&mut tr, frame(3, 3100, false, None, true, t0), "menu RacePause", &mut out);
        run(&mut tr, frame(2, 8100, true, Some(-5.9), false, t0), "race", &mut out);
        out.clear();
        run(&mut tr, frame(2, 14_000, true, Some(-0.01), false, t0), "race", &mut out);
        assert!(out.is_empty());
        run(&mut tr, frame(2, 14_010, true, Some(0.0), false, t0), "race", &mut out);
        assert_eq!(
            out,
            ["wait: race -> green light 8010 ms after the race's first frame (the start scene and the countdown, \
              5000 ms paused not counted)"]
        );
        // Back from a pause after the green light: no countdown.
        out.clear();
        run(&mut tr, frame(3, 15_000, false, None, true, t0), "menu RacePause", &mut out);
        run(&mut tr, frame(2, 16_000, true, Some(1.0), false, t0), "race", &mut out);
        assert!(out.iter().all(|l| !l.contains("green light")), "{out:?}");
        // A restart from the pause menu: the clock set back, a new countdown.
        run(&mut tr, frame(2, 17_000, true, Some(2.0), false, t0), "race", &mut out);
        run(&mut tr, frame(3, 17_100, false, None, true, t0), "menu RacePause", &mut out);
        run(&mut tr, frame(2, 18_000, true, Some(-8.0), false, t0), "race", &mut out);
        out.clear();
        run(&mut tr, frame(2, 27_000, true, Some(0.01), false, t0), "race", &mut out);
        assert_eq!(
            out,
            ["wait: race -> green light 9000 ms after the race's first frame (the start scene and the countdown)"]
        );
    }
}
