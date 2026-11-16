//! Pop meter (docs/pop-in.md; a measuring aid, `NFSU2_POP_STATS=1`):
//! every element that comes into or leaves the main view's picture while
//! it is in the view, by source, with its size on the screen and how fast
//! it changes. Nothing here changes a frame; with the variable unset no
//! event is recorded.
//!
//! An event's size is the element's projected bounding radius in output
//! pixels: the cull's size (`scale · r / (d − r)`, pixels of radius on a
//! 512-wide view, [`super::cull_scale`]) × output width / 512. Its
//! transition is the time from the first to the full change: 0 at once
//! (native), the fade's length with LOD Fade. A **visible pop** is an
//! event of at least [`VISIBLE_PX`] whose transition is shorter than
//! [`GRADUAL_SECONDS`]; a gradual one (longer) or a smaller one is counted
//! apart.

use std::sync::Mutex;

/// Below this projected radius (output pixels) an event is not counted
/// visible (a 4-pixel-wide speck, under the TAA / MSAA resolve's reach).
pub const VISIBLE_PX: f32 = 2.0;
/// Size classes reported: at least 2, 8 and 32 pixels of radius.
pub const CLASSES: [f32; 3] = [2.0, 8.0, 32.0];
/// A change taking this long or longer reads as a gradual appearance,
/// not a pop.
pub const GRADUAL_SECONDS: f32 = 0.5;

/// What popped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    /// A scenery instance crossing the size cull (props, lamps, signs,
    /// trees; their flares follow).
    Scenery,
    /// A whole section entering or leaving the drawn set (visible-list
    /// change, look-ahead ring, far set).
    Section,
    /// A car's LOD switch (A..D).
    CarLod,
    /// A car appearing or vanishing at its size cull (size 4).
    CarCull,
    /// A traffic car placed again while drawn (despawn far off, spawn in
    /// the view): it vanishes and appears elsewhere.
    TrafficRespawn,
    /// A lamp's added light switched on or off with its section (Dynamic
    /// Lamps / Modern Lighting).
    LampLight,
    /// A lamp's shadow layer handed out (its shadow appears) or taken.
    LampShadow,
}

impl Source {
    pub const ALL: [Source; 7] = [Source::Scenery, Source::Section, Source::CarLod, Source::CarCull, Source::TrafficRespawn, Source::LampLight, Source::LampShadow];

    pub fn name(self) -> &'static str {
        match self {
            Source::Scenery => "scenery cull",
            Source::Section => "section swap",
            Source::CarLod => "car LOD switch",
            Source::CarCull => "car cull (size 4)",
            Source::TrafficRespawn => "traffic respawn",
            Source::LampLight => "lamp light",
            Source::LampShadow => "lamp shadow",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Tally {
    /// Events of at least each [`CLASSES`] size, abrupt (shorter than
    /// [`GRADUAL_SECONDS`]).
    pub abrupt: [u64; 3],
    /// The same, gradual.
    pub gradual: [u64; 3],
    /// Events under [`VISIBLE_PX`].
    pub small: u64,
    /// The largest abrupt event (pixels).
    pub worst_px: f32,
    /// Frames with at least one visible pop of this source.
    pub frames: u64,
}

#[derive(Default)]
struct Meter {
    frames: u64,
    pop_frames: u64,
    width: f32,
    tallies: std::collections::BTreeMap<Source, Tally>,
    this_frame: std::collections::BTreeSet<Source>,
    /// Per frame: visible pops of each source (for the per-frame log).
    log: Vec<[u32; 7]>,
    current: [u32; 7],
}

static METER: Mutex<Option<Meter>> = Mutex::new(None);

/// Whether events are recorded (`NFSU2_POP_STATS`).
pub fn enabled() -> bool {
    super::pop_stats_enabled()
}

/// The pop reference (`NFSU2_POP_ORACLE`, a measuring aid): scenery and
/// cars drawn whatever their size (cars at their top LOD), so a frame
/// dump against it shows what a setting leaves out.
pub fn oracle() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| crate::render::debug_env::nfsu2_pop_oracle())
}

/// Per-event lines on stderr (`NFSU2_POP_VERBOSE`, with the statistics).
pub fn verbose() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| crate::render::debug_env::nfsu2_pop_verbose())
}

/// Pixels of radius on the output for a cull size (512-wide units);
/// `opts`: the frame's settings (the output's width).
pub fn px(size: f32, opts: &crate::render::RenderOptions) -> f32 {
    let w = opts.output_size_or((1280, 960)).0 as f32;
    size * w / 512.0
}

/// Records one event of `source` with projected radius `px` (output
/// pixels) changing over `seconds` (0: at once).
pub fn event(source: Source, px: f32, seconds: f32) {
    if !enabled() {
        return;
    }
    let mut g = METER.lock().unwrap();
    let m = g.get_or_insert_with(Meter::default);
    let t = m.tallies.entry(source).or_default();
    if px < VISIBLE_PX {
        t.small += 1;
        return;
    }
    let abrupt = seconds < GRADUAL_SECONDS;
    for (k, &c) in CLASSES.iter().enumerate() {
        if px >= c {
            if abrupt { t.abrupt[k] += 1 } else { t.gradual[k] += 1 }
        }
    }
    if abrupt {
        t.worst_px = t.worst_px.max(px);
        m.this_frame.insert(source);
        let i = Source::ALL.iter().position(|&s| s == source).unwrap_or(0);
        m.current[i] += 1;
    }
}

/// Closes a main-view frame (`opts`: its settings).
pub fn end_frame(opts: &crate::render::RenderOptions) {
    if !enabled() {
        return;
    }
    let mut g = METER.lock().unwrap();
    let m = g.get_or_insert_with(Meter::default);
    m.frames += 1;
    m.width = opts.output_size_or((1280, 960)).0 as f32;
    if !m.this_frame.is_empty() {
        m.pop_frames += 1;
    }
    for s in std::mem::take(&mut m.this_frame) {
        m.tallies.entry(s).or_default().frames += 1;
    }
    let c = std::mem::take(&mut m.current);
    m.log.push(c);
}

/// The report lines (and the per-frame log as TSV), the meter emptied.
pub fn take_report() -> Option<(Vec<String>, String)> {
    let m = METER.lock().unwrap().take()?;
    let mut lines = vec![format!(
        "pop meter: {} frames at {} px wide, {} with a visible pop ({:.1}%); visible = radius >= {} px, abrupt = under {} s",
        m.frames, m.width, m.pop_frames, 100.0 * m.pop_frames as f64 / m.frames.max(1) as f64, VISIBLE_PX, GRADUAL_SECONDS
    )];
    lines.push("  source               abrupt>=2px  >=8px  >=32px  gradual>=2px  under2px  frames  worst px".into());
    for s in Source::ALL {
        let t = m.tallies.get(&s).cloned().unwrap_or_default();
        lines.push(format!("  {:<20} {:>11} {:>6} {:>7} {:>13} {:>9} {:>7} {:>9.0}", s.name(), t.abrupt[0], t.abrupt[1], t.abrupt[2], t.gradual[0], t.small, t.frames, t.worst_px));
    }
    let mut tsv = String::from("frame");
    for s in Source::ALL {
        tsv += &format!("\t{}", s.name().replace(' ', "_"));
    }
    tsv.push('\n');
    for (i, f) in m.log.iter().enumerate() {
        tsv += &i.to_string();
        for v in f {
            tsv += &format!("\t{v}");
        }
        tsv.push('\n');
    }
    Some((lines, tsv))
}

#[cfg(test)]
mod tests {
    /// World Detail Ultra / Extreme set the scenery need on output
    /// pixels (1.5 px of radius: 0.2 cull units at 3840 wide); the other
    /// tiers keep 17 ÷ Detail Distance.
    #[test]
    fn ultra_cull_follows_the_output_pixels() {
        let _g = crate::qol::test_globals();
        crate::qol::set_output_size((3840, 2160));
        crate::qol::Enhancements { keep_loaded: true, detail_distance: 200, ..Default::default() }.apply();
        let need = super::super::main_min_size(&crate::render::RenderOptions::in_force());
        assert!((need - crate::render::RenderOptions::DETAIL_FULL_PX * 512.0 / 3840.0).abs() < 1e-6, "{need}");
        crate::qol::Enhancements { keep_loaded: false, detail_distance: 200, ..Default::default() }.apply();
        assert_eq!(super::super::main_min_size(&crate::render::RenderOptions::in_force()), 17.0 / 2.0);
        // A small output: the percentage's need when it is the smaller.
        crate::qol::set_output_size((64, 48));
        crate::qol::Enhancements { keep_loaded: true, detail_distance: 200, ..Default::default() }.apply();
        assert_eq!(super::super::main_min_size(&crate::render::RenderOptions::in_force()), 17.0 / 2.0);
        crate::qol::Enhancements::default().apply();
        crate::qol::set_output_size((0, 0));
        assert_eq!(super::super::main_min_size(&crate::render::RenderOptions::in_force()), 17.0);
    }

    #[test]
    fn traffic_ring_fades_to_the_despawn_distance() {
        use crate::ai::traffic::{DESPAWN, RING_FADE_FROM, ring_presence};
        assert_eq!(ring_presence(0.0), 1.0);
        assert_eq!(ring_presence(RING_FADE_FROM), 1.0);
        assert_eq!(ring_presence(DESPAWN), 0.0);
        assert_eq!(ring_presence(DESPAWN + 100.0), 0.0);
        assert!((ring_presence((RING_FADE_FROM + DESPAWN) * 0.5) - 0.5).abs() < 1e-6);
    }
}
