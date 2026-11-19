//! The rate gate (docs/developing/checks.md "Rate gate"):
//! the display schedules the rate runs draw at, the simulated wall clock,
//! the tick index flows key their inputs on, the headless frame's
//! draw-side state updates, the α = 0 image dumps and the gate's
//! self-test. Everything here is on only with `NFSU2_RATE=<schedule>`
//! (the rate gate sets it); without it the flows, benches and the state
//! hash run exactly as before. Not native: a measuring aid.
//!
//! Schedules (`NFSU2_RATE`):
//! - `<hz>`: frames of exactly 1/hz s in units of 1/36 000 s (exact for
//!   30/50/60/72/75/90/100/120/144/200/240 Hz); any other rate rounds the
//!   frame to whole units. The f32 interval handed to `Game::advance` is
//!   compensated so the sum of the intervals never falls behind the exact
//!   time (it stays less than one f32 step ahead): the window's cadence
//!   then sees the ticks at exactly the frames the integer schedule has
//!   them.
//! - `naive:<hz>`: 1/hz as an f32 every frame (the old flows' interval:
//!   its f64 sum drifts, e.g. one tick short at 200 Hz after 10 s).
//! - `jitter:<seed>`: frames of 4–40 ms (whole units, xorshift from the
//!   seed) plus one 120 ms stall about every 5 s (4–6 s apart).
//!
//! `NFSU2_RATE_TICKS=<n>` (benches): the run ends after exactly n ticks
//! from the schedule's first frame, whatever the schedule: the frame that
//! would pass the n-th tick is cut short to end on it (α = 0), so a
//! jittered run, whose stalls drop time past the tick cap, covers the
//! same ticks as the exact schedules (before, a jittered bench ran a
//! number of frames and ended a few ticks short).
//!
//! `NFSU2_RATE_SELFTEST=<tick>`: from that tick on, every drawn frame
//! draws once from the game's RNG (a harness-only dependence on the
//! drawn-frame count the gate must catch: `perf.sh rate`'s self-test).
//! `NFSU2_RATE_DRAW=<mask>`: which draw-side updates headless flows run
//! per frame (`drawn`; default 31 = all): 1 the HUD (`overlay_layout`), 2
//! the menus (`menu_quads`), 4 the mouse map's frame, 8 the garage's
//! request, 16 the drawn-frame count (`frame_drawn`); 0 none.
use crate::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// Schedule time units per second.
pub(crate) const UNITS: u64 = 36_000;

/// World updates recorded so far (`Game::update` calls, the state hash's
/// step index): the tick index flows key their inputs on.
static TICKS: AtomicU64 = AtomicU64::new(0);

/// After each `Game::update` (state_hash::record).
pub(crate) fn count_tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

/// World updates so far (the next update's tick index).
pub(crate) fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    /// Frames of this many units.
    Exact(u64),
    /// 1/hz as f32, uncompensated.
    Naive(f32),
    /// 4–40 ms frames and stalls.
    Jitter,
}

/// A display schedule: the drawn frames' intervals.
#[derive(Clone, Debug)]
pub(crate) struct Schedule {
    kind: Kind,
    /// Exact time drawn so far (units).
    units: u64,
    /// The f64 sum of the f32 intervals handed out (s).
    given: f64,
    rng: u64,
    /// When the next jitter stall comes (units).
    next_stall: u64,
    pub(crate) frames: u64,
}

impl Schedule {
    pub(crate) fn parse(s: &str) -> Option<Schedule> {
        let s = s.trim();
        let (kind, seed) = if let Some(seed) = s.strip_prefix("jitter:") {
            (Kind::Jitter, seed.parse::<u64>().ok()?)
        } else if let Some(hz) = s.strip_prefix("naive:") {
            let hz: f32 = hz.parse().ok()?;
            (hz > 0.0).then_some(())?;
            (Kind::Naive(1.0 / hz), 0)
        } else {
            let hz: f64 = s.parse().ok()?;
            (hz >= 1.0).then_some(())?;
            (Kind::Exact(((UNITS as f64 / hz).round() as u64).max(1)), 0)
        };
        let mut sch = Schedule {
            kind,
            units: 0,
            given: 0.0,
            rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
            next_stall: 0,
            frames: 0,
        };
        sch.next_stall = 5 * UNITS - UNITS + sch.draw(2 * UNITS);
        Some(sch)
    }

    /// xorshift64*: 0..n.
    fn draw(&mut self, n: u64) -> u64 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) % n.max(1)
    }

    /// The next frame's length in units.
    pub(crate) fn next_units(&mut self) -> u64 {
        let u = match self.kind {
            Kind::Exact(u) => u,
            Kind::Naive(dt) => ((f64::from(dt) * UNITS as f64).round() as u64).max(1),
            Kind::Jitter => {
                if self.units >= self.next_stall {
                    self.next_stall = self.units + 4 * UNITS + self.draw(2 * UNITS);
                    UNITS * 120 / 1000
                } else {
                    // 4..=40 ms.
                    UNITS * 4 / 1000 + self.draw(UNITS * 36 / 1000 + 1)
                }
            }
        };
        self.units += u;
        self.frames += 1;
        u
    }

    /// The next frame's interval as `Game::advance` takes it (s).
    pub(crate) fn next_dt(&mut self) -> f32 {
        self.next_dt_within(u64::MAX)
    }

    /// [`Schedule::next_dt`] with the frame cut to at most `max` units
    /// (the run's last frame, `NFSU2_RATE_TICKS`).
    fn next_dt_within(&mut self, max: u64) -> f32 {
        let mut u = self.next_units();
        if u > max {
            self.units -= u - max;
            u = max;
        }
        let dt = match self.kind {
            Kind::Naive(dt) => dt,
            _ => {
                // The smallest f32 that keeps the handed-out sum at or past
                // the exact time.
                let need = self.units as f64 / UNITS as f64 - self.given;
                let mut dt = need as f32;
                while (f64::from(dt)) < need {
                    dt = f32::from_bits(dt.to_bits() + 1);
                }
                let _ = u;
                dt
            }
        };
        self.given += f64::from(dt);
        dt
    }

    /// The exact time drawn so far (s).
    pub(crate) fn seconds(&self) -> f64 {
        self.units as f64 / UNITS as f64
    }
}

/// The rate run's state.
pub(crate) struct Ctx {
    pub(crate) schedule: Schedule,
    pub(crate) name: String,
    selftest: Option<u64>,
    /// Which draw-side updates a headless frame runs (`drawn`).
    draw: u32,
    /// The world updates before the schedule's first frame: the α = 0
    /// frames fall on ticks ≡ 0 (mod 6) counted from there.
    tick_base: Option<u64>,
    /// `NFSU2_RATE_TICKS`: the ticks the run takes from its first frame.
    budget: Option<u64>,
}

fn ctx() -> &'static std::sync::Mutex<Option<Ctx>> {
    static CTX: std::sync::OnceLock<std::sync::Mutex<Option<Ctx>>> = std::sync::OnceLock::new();
    CTX.get_or_init(|| {
        let ctx = std::env::var("NFSU2_RATE").ok().and_then(|name| {
            let schedule = Schedule::parse(&name)?;
            Some(Ctx {
                schedule,
                name,
                selftest: std::env::var("NFSU2_RATE_SELFTEST").ok().and_then(|v| v.parse().ok()),
                draw: std::env::var("NFSU2_RATE_DRAW").ok().and_then(|v| v.parse().ok()).unwrap_or(31),
                tick_base: None,
                budget: std::env::var("NFSU2_RATE_TICKS").ok().and_then(|v| v.parse().ok()),
            })
        });
        std::sync::Mutex::new(ctx)
    })
}

/// A rate run (`NFSU2_RATE` set to a schedule).
pub(crate) fn active() -> bool {
    ctx().lock().unwrap().is_some()
}

/// The schedule's name (for logs).
pub(crate) fn name() -> Option<String> {
    ctx().lock().unwrap().as_ref().map(|c| format!("{} ({:?})", c.name, c.schedule.kind))
}

/// A rate run's start (`--flow`): the game's own clock (on ticks),
/// no pinned harness clock.
pub(crate) fn start(game: &mut Game) {
    if active() {
        game.pacing.clock = None;
    }
}

/// In a rate run a flow's own failures are printed, not failed (its checks
/// are made for steady rates; the rate gate compares the state). True:
/// the flow returns Ok.
pub(crate) fn report_only(failures: usize) -> bool {
    if active() && failures > 0 {
        println!("rate run: {failures} flow check failures reported, not failed");
        return true;
    }
    false
}

/// A rate run's next drawn frame: its interval (the game's clock runs on
/// its ticks: a bench's pinned clock is dropped) and the
/// self-test's injection.
/// None outside a rate run.
pub(crate) fn frame(game: &mut Game) -> Option<f32> {
    let mut g = ctx().lock().unwrap();
    let c = g.as_mut()?;
    let base = *c.tick_base.get_or_insert(ticks());
    // The run's tick budget: a frame never passes its last tick. A frame of
    // at most `left · period − acc` units takes at most `left` ticks (the
    // catch-up loop takes a tick while acc ≥ period − snap, each takes a
    // period off), and one of exactly that many takes them all and ends
    // at α = 0. The exact schedules end on that tick anyway (never cut).
    let max = match c.budget {
        Some(b) => {
            let left = b.saturating_sub(ticks() - base) as i64;
            let cadence = &game.pacing.cadence;
            (left * game.tick_rate().period - cadence.acc).max(0) as u64
        }
        None => u64::MAX,
    };
    let dt = c.schedule.next_dt_within(max);
    game.pacing.clock = None;
    if c.selftest.is_some_and(|t| ticks() >= t) {
        // The injected dependence: one RNG draw per drawn frame.
        let _ = game.rng.below(2);
    }
    Some(dt)
}

/// The drawn frames' interval of a flow run at `hz` (0 = uncapped) as the
/// old flows had it, or the rate run's schedule.
pub(crate) fn interval(game: &mut Game, hz: f32, frame: u64) -> f32 {
    if let Some(dt) = self::frame(game) {
        return dt;
    }
    if hz > 0.0 {
        1.0 / hz
    } else {
        // Uncapped: 1..4 ms, a fixed pseudo-random pattern.
        0.001 + ((frame.wrapping_mul(2_654_435_761) >> 7) % 3000) as f32 * 1e-6
    }
}

/// A flow's input index: the world updates since `mark` (inputs keyed on
/// the tick, the same at every rate; the default for every flow, not
/// only for rate runs).
pub(crate) fn keyed(mark: u64) -> u64 {
    ticks().saturating_sub(mark)
}

/// `NFSU2_RATE_TICKS`'s budget taken: the run's last frame is drawn.
pub(crate) fn budget_spent() -> bool {
    let g = ctx().lock().unwrap();
    let Some(c) = g.as_ref() else { return false };
    match (c.budget, c.tick_base) {
        (Some(b), Some(base)) => ticks() - base >= b,
        _ => false,
    }
}

/// After a headless flow's `advance`: what the window's frame does to the
/// game's state besides drawing (`prepare_frame`: the HUD's and the
/// menus' layout with their per-draw state, the garage's request; then
/// `frame_drawn`). Rate runs only (`NFSU2_RATE_DRAW` masks the parts).
pub(crate) fn drawn(game: &mut Game) {
    let Some(mask) = ctx().lock().unwrap().as_ref().map(|c| c.draw) else { return };
    let now = game.now();
    if mask & 1 != 0 {
        let _ = game.overlay_layout(now);
    }
    if mask & 2 != 0 {
        let _ = game.menu_quads(now);
    }
    // 4 and 8: the mouse map and the garage's request step on the tick
    // (`Game::fe_tick`); the draw only reads the request.
    if mask & 8 != 0 {
        let _ = game.garage_fe.request.clone();
    }
    if mask & 16 != 0 {
        game.frame_drawn();
    }
}

/// `NFSU2_RATE_DUMP=dir` (bench): whether this drawn frame is an α = 0
/// frame at a tick ≡ 0 (mod 6) counted from the schedule's start (a frame
/// that drew exactly the state after that tick: one exists at
/// 30/60/120/200 Hz). Returns the tick index (from boot).
pub(crate) fn dump_tick(updates: usize, alpha: f32) -> Option<u64> {
    if updates == 0 {
        return None;
    }
    let base = ctx().lock().unwrap().as_ref()?.tick_base?;
    // The cadence's α is the remainder over the update period: the exact
    // schedules leave at most a few f32 steps (< 1e-4).
    let n = ticks();
    ((n - base) % 6 == 0 && alpha < 1e-4).then_some(n)
}

/// The α = 0 frame of tick `n` as `NFSU2_RATE_DUMP/tick-<n>.png`, and a
/// row (tick, α, drawn frames, schedule time) in its `ticks.tsv`.
pub(crate) fn dump(game: &mut Game, n: u64, alpha: f32, width: u32, height: u32) -> Result<()> {
    let Some(dir) = std::env::var_os("NFSU2_RATE_DUMP").map(std::path::PathBuf::from) else { return Ok(()) };
    std::fs::create_dir_all(&dir)?;
    let Some(w) = game.platform.window.as_mut() else { return Ok(()) };
    if w.offscreen.is_none() {
        return Ok(());
    }
    w.settle();
    let mut pixels = nfsu2::render::read_back(&w.gpu, w.offscreen.as_ref().unwrap(), width, height);
    for p in pixels.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    image::save_buffer(dir.join(format!("tick-{n:06}.png")), &pixels, width, height, image::ColorType::Rgba8)?;
    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("ticks.tsv"))?;
    let (frames, t) = ctx().lock().unwrap().as_ref().map_or((0, 0.0), |c| (c.schedule.frames, c.schedule.seconds()));
    writeln!(f, "{n}\t{alpha:e}\t{frames}\t{t:.6}")?;
    // The cars' environment cube this frame sampled (a digest of
    // its six faces), to compare across rates at the same tick.
    if let (Some(r), Some(w)) = (game.render.world.as_ref(), game.platform.window.as_ref()) {
        let mut e = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("env.tsv"))?;
        writeln!(e, "{n}\t{:016x}", r.environment_digest(&w.gpu))?;
    }
    Ok(())
}

/// The rate bench's WAV written (`write_audio_capture`'s format): its
/// capture, rendered per tick (`audio_tick_end`) since its timed frames
/// began.
pub(crate) fn bench_wav_write() -> Result<()> {
    let Ok(path) = std::env::var("NFSU2_AUDIO_WAV") else { return Ok(()) };
    if !active() {
        return Ok(());
    }
    write_audio_capture(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact schedules hit whole ticks: at 30/60/120/200 Hz the time
    /// after k frames is k/hz exactly, and the handed-out f32 sum never
    /// falls behind it nor runs one f32 step ahead.
    #[test]
    fn exact_schedules_track_the_exact_time() {
        for hz in [30u64, 60, 120, 200] {
            let mut s = Schedule::parse(&hz.to_string()).unwrap();
            let mut sum = 0.0f64;
            for k in 1..=20_000u64 {
                sum += f64::from(s.next_dt());
                let exact = k as f64 / hz as f64;
                assert!(sum >= exact - 1e-12, "{hz} Hz frame {k}: {sum} behind {exact}");
                assert!(sum - exact < 1e-8, "{hz} Hz frame {k}: {sum} ahead of {exact}");
            }
            assert_eq!(s.units, 20_000 * UNITS / hz);
        }
    }

    /// Jitter: 4–40 ms frames and a 120 ms stall about every 5 s, the same
    /// for the same seed.
    #[test]
    fn jitter_is_bounded_seeded_and_stalls() {
        let run = |seed: &str| {
            let mut s = Schedule::parse(seed).unwrap();
            (0..3000).map(|_| s.next_units()).collect::<Vec<_>>()
        };
        let a = run("jitter:7");
        assert_eq!(a, run("jitter:7"));
        assert_ne!(a, run("jitter:8"));
        let stalls = a.iter().filter(|&&u| u == UNITS * 120 / 1000).count();
        let secs = a.iter().sum::<u64>() as f64 / UNITS as f64;
        assert!(a.iter().all(|&u| (144..=1440).contains(&u) || u == 4320), "{a:?}");
        assert!((secs / 6.5) as usize <= stalls && stalls <= (secs / 3.5) as usize + 1, "{stalls} stalls in {secs} s");
    }

    #[test]
    fn naive_and_bad_names() {
        let mut s = Schedule::parse("naive:200").unwrap();
        assert_eq!(s.next_dt(), 1.0 / 200.0);
        assert!(Schedule::parse("x").is_none());
        assert!(Schedule::parse("0").is_none());
        assert!(Schedule::parse("jitter:").is_none());
    }
}
