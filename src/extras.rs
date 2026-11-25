//! Modern extras (docs/playing/modern-extras.md): the Enhancements page's
//! Low Latency, Replay Cameras and Accessibility rows. Clone deviations,
//! every one opt-in, every default native; kept in options.txt as `enh.*`
//! lines (written only when not native) through [`crate::qol::Enhancements`].

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// HUD element scale steps, percent (100 native).
pub const ELEMENT_STEPS: [u8; 7] = [70, 80, 90, 100, 115, 130, 150];

/// The colour-vision palettes: 0 native, 1 deuteranopia, 2 protanopia,
/// 3 tritanopia.
pub const PALETTE_NAMES: [&str; 4] = ["Native", "Deuteranopia", "Protanopia", "Tritanopia"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extras {
    /// Low Latency (docs/playing/modern-extras.md §1): one frame in flight, the
    /// wait for the display moved before the input, the VRR cap.
    pub low_latency: bool,
    /// Replay Cameras "Extended" (§3): the extra replay angles on the C key
    /// after the native six, 0.5× / 0.25× smooth slow motion, Cinematic DOF
    /// on them. Off: the native replay viewer.
    pub replay_cameras: bool,
    /// Colour-vision palette for the HUD (§4): [`PALETTE_NAMES`].
    pub palette: u8,
    /// Per-element HUD scale, percent: the minimap, the speedometer /
    /// tachometer, the race information (position, laps, times).
    pub hud_minimap: u8,
    pub hud_speedo: u8,
    pub hud_info: u8,
    /// Reduced Flashing (§4): caps the full-screen flashes.
    pub reduced_flashing: bool,
    /// In the background (docs/playing/modern-extras.md §5): what the game
    /// does while its window is not the focused one or is hidden
    /// ([`BACKGROUND_NAMES`]); 0 Pause (native: SPEED2.EXE stops).
    pub background: u8,
    /// Power Saving: frame caps that save power where they change nothing
    /// the player watches ([`POWER_NAMES`], [`power_cap`]); 0 Off (native:
    /// uncapped).
    pub power_saving: u8,
    /// Mark Spot Key (docs/playing/mark-spot.md): 0 Off (native), else the key
    /// `crate::mark_spot::KEYS[n]` writes a mark-this-spot report.
    pub mark_spot: u8,
}

impl Default for Extras {
    fn default() -> Extras {
        Extras { low_latency: false, replay_cameras: false, palette: 0, hud_minimap: 100, hud_speedo: 100, hud_info: 100, reduced_flashing: false, background: 0, power_saving: 0, mark_spot: 0 }
    }
}

impl Extras {
    pub fn deviations(&self, v: &mut Vec<String>) {
        if self.low_latency {
            v.push("low latency".into());
        }
        if self.replay_cameras {
            v.push("replay cameras extended".into());
        }
        if self.palette != 0 {
            v.push(format!("HUD palette {}", PALETTE_NAMES[usize::from(self.palette.min(3))]));
        }
        for (name, s) in [("minimap", self.hud_minimap), ("speedo", self.hud_speedo), ("race info", self.hud_info)] {
            if s != 100 {
                v.push(format!("{name} {s}%"));
            }
        }
        if self.reduced_flashing {
            v.push("reduced flashing".into());
        }
        if self.background != 0 {
            v.push(format!("in the background {}", BACKGROUND_NAMES[usize::from(self.background.min(2))]));
        }
        if self.power_saving != 0 {
            v.push(format!("power saving {}", POWER_NAMES[usize::from(self.power_saving.min(2))]));
        }
        if self.mark_spot != 0 {
            v.push(format!("mark spot {}", crate::mark_spot::KEYS[usize::from(self.mark_spot).min(crate::mark_spot::KEYS.len() - 1)]));
        }
    }

    pub fn write(&self, out: &mut String) {
        let d = Extras::default();
        if self.low_latency != d.low_latency {
            out.push_str("enh.low_latency 1\n");
        }
        if self.replay_cameras != d.replay_cameras {
            out.push_str("enh.replay_cameras 1\n");
        }
        if self.palette != d.palette {
            out.push_str(&format!("enh.hud_palette {}\n", self.palette));
        }
        if self.hud_minimap != d.hud_minimap {
            out.push_str(&format!("enh.hud_minimap {}\n", self.hud_minimap));
        }
        if self.hud_speedo != d.hud_speedo {
            out.push_str(&format!("enh.hud_speedo {}\n", self.hud_speedo));
        }
        if self.hud_info != d.hud_info {
            out.push_str(&format!("enh.hud_info {}\n", self.hud_info));
        }
        if self.reduced_flashing != d.reduced_flashing {
            out.push_str("enh.reduced_flashing 1\n");
        }
        if self.background != d.background {
            out.push_str(&format!("enh.background {}\n", self.background));
        }
        if self.power_saving != d.power_saving {
            out.push_str(&format!("enh.power_caps {}\n", self.power_saving));
        }
        if self.mark_spot != d.mark_spot {
            out.push_str(&format!("enh.mark_spot {}\n", self.mark_spot));
        }
    }

    pub fn parse(&mut self, key: &str, value: &str) -> bool {
        let scale = |v: &str, old: u8| v.parse::<u8>().map_or(old, |v| v.clamp(ELEMENT_STEPS[0], ELEMENT_STEPS[ELEMENT_STEPS.len() - 1]));
        match key {
            "enh.low_latency" => self.low_latency = value != "0",
            "enh.replay_cameras" => self.replay_cameras = value != "0",
            "enh.hud_palette" => self.palette = value.parse::<u8>().map_or(self.palette, |v| v.min(3)),
            "enh.hud_minimap" => self.hud_minimap = scale(value, self.hud_minimap),
            "enh.hud_speedo" => self.hud_speedo = scale(value, self.hud_speedo),
            "enh.hud_info" => self.hud_info = scale(value, self.hud_info),
            "enh.reduced_flashing" => self.reduced_flashing = value != "0",
            "enh.background" => self.background = value.parse::<u8>().map_or(self.background, |v| v.min(2)),
            "enh.power_caps" => self.power_saving = value.parse::<u8>().map_or(self.power_saving, |v| v.min(2)),
            // The older Power Saving line (0 Off, 1 Background, 2 + Menus,
            // 3 + Battery): its background step is now In the background's
            // Draw less, the rest one step down.
            "enh.power_saving" => {
                if let Ok(v) = value.parse::<u8>() {
                    let v = v.min(3);
                    if v >= 1 {
                        self.background = BACKGROUND_DRAW_LESS;
                    }
                    self.power_saving = v.saturating_sub(1);
                }
            }
            "enh.mark_spot" => self.mark_spot = value.parse::<u8>().map_or(self.mark_spot, |v| v.min(crate::mark_spot::KEYS.len() as u8 - 1)),
            _ => return false,
        }
        true
    }

    pub fn apply(&self) {
        LOW_LATENCY.store(self.low_latency, Ordering::Relaxed);
        REPLAY_CAMERAS.store(self.replay_cameras, Ordering::Relaxed);
        PALETTE.store(self.palette, Ordering::Relaxed);
        HUD_MINIMAP.store(self.hud_minimap, Ordering::Relaxed);
        HUD_SPEEDO.store(self.hud_speedo, Ordering::Relaxed);
        HUD_INFO.store(self.hud_info, Ordering::Relaxed);
        REDUCED_FLASHING.store(self.reduced_flashing, Ordering::Relaxed);
        BACKGROUND.store(self.background, Ordering::Relaxed);
        POWER_SAVING.store(self.power_saving, Ordering::Relaxed);
    }
}

static LOW_LATENCY: AtomicBool = AtomicBool::new(false);
static REPLAY_CAMERAS: AtomicBool = AtomicBool::new(false);
static PALETTE: AtomicU8 = AtomicU8::new(0);
static HUD_MINIMAP: AtomicU8 = AtomicU8::new(100);
static HUD_SPEEDO: AtomicU8 = AtomicU8::new(100);
static HUD_INFO: AtomicU8 = AtomicU8::new(100);
static REDUCED_FLASHING: AtomicBool = AtomicBool::new(false);
static POWER_SAVING: AtomicU8 = AtomicU8::new(0);
static BACKGROUND: AtomicU8 = AtomicU8::new(0);
static KEEP_RUNNING_FORCED: AtomicBool = AtomicBool::new(false);

/// In the background's settings, for a window that is not the focused one
/// or is hidden (minimised, covered): 0 Pause (native: SPEED2.EXE's message
/// pump 0x5BF1D0 waits in GetMessage while g_AppActive 0x870988 is 0, so the
/// simulation, the drawing and the input stop and the sound is held;
/// docs/original/focus-loss.md); 1 Keep running (the simulation, the sound
/// and the drawing go on); 2 Draw less (as Keep running, the drawing held
/// to [`BACKGROUND_FPS`]).
pub const BACKGROUND_NAMES: [&str; 3] = ["Pause", "Keep running", "Draw less"];
pub const BACKGROUND_PAUSE: u8 = 0;
pub const BACKGROUND_KEEP_RUNNING: u8 = 1;
pub const BACKGROUND_DRAW_LESS: u8 = 2;

/// Power Saving's settings: 0 Off; 1 the menus held to the display's
/// refresh rate; 2 also the whole game on battery held to the display's
/// refresh rate.
pub const POWER_NAMES: [&str; 3] = ["Off", "Menus", "Menus + Battery"];

/// A background window's frame rate under Draw less (frames nobody
/// watches; the world keeps its cadence through the interpolating frame
/// clock, so the simulation is the same at any drawn rate). Never below
/// 30: a drawn frame steps at most 3 race / 2 scene ticks
/// (`frame_clock`'s cap), so under 20 / 30 fps time would be dropped and
/// the world would slow down.
pub const BACKGROUND_FPS: f64 = 30.0;

/// In the background's setting ([`BACKGROUND_NAMES`]), Keep running when
/// it is forced ([`force_keep_running`]).
pub fn background() -> u8 {
    if KEEP_RUNNING_FORCED.load(Ordering::Relaxed) {
        BACKGROUND_KEEP_RUNNING
    } else {
        BACKGROUND.load(Ordering::Relaxed)
    }
}

/// Keep running whatever the setting: the scripted and test runs (the map
/// editor's and Studio's test game, `NFSU2_KEEP_RUNNING`), which run in a
/// window nobody focuses.
pub fn force_keep_running(on: bool) {
    KEEP_RUNNING_FORCED.store(on, Ordering::Relaxed);
}

/// Power Saving's setting (0 Off).
pub fn power_saving() -> u8 {
    POWER_SAVING.load(Ordering::Relaxed)
}

/// What Power Saving looks at each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PowerState {
    /// The window has the keyboard focus and is not hidden.
    pub foreground: bool,
    /// A menu is up (the front end, a pause or a dialog).
    pub menu: bool,
    /// The machine runs on battery ([`on_battery`]).
    pub battery: bool,
    /// The display's refresh rate (Hz), when known.
    pub refresh_hz: Option<f64>,
}

/// The frame period Power Saving holds a frame to (None: no cap) for
/// setting `level` in state `s`: the longest that applies. A menu or a
/// battery cap is the display's refresh period (30–240 Hz; 60 when not
/// known): no frame the display would show is dropped (with vsync off a
/// faster frame is replaced before it is shown, or tears).
pub fn power_cap(level: u8, background: u8, s: PowerState) -> Option<std::time::Duration> {
    let refresh = std::time::Duration::from_secs_f64(1.0 / s.refresh_hz.filter(|hz| hz.is_finite() && *hz > 1.0).unwrap_or(60.0).clamp(30.0, 240.0));
    let mut cap: Option<std::time::Duration> = None;
    let mut hold = |d: std::time::Duration| cap = Some(cap.map_or(d, |c| c.max(d)));
    if background == BACKGROUND_DRAW_LESS && !s.foreground {
        hold(std::time::Duration::from_secs_f64(1.0 / BACKGROUND_FPS));
    }
    if level >= 1 && s.menu {
        hold(refresh);
    }
    if level >= 2 && s.battery {
        hold(refresh);
    }
    cap
}

/// The machine runs on battery: Linux's power supplies (`/sys/class/
/// power_supply`) list a mains adapter and none is online. False where
/// that is not known (no mains entry, other systems): a desktop is never
/// "on battery".
pub fn on_battery() -> bool {
    on_battery_in(std::path::Path::new("/sys/class/power_supply"))
}

/// [`on_battery`] over a power-supply folder (tests).
pub fn on_battery_in(dir: &std::path::Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    let mut mains = false;
    for e in entries.flatten() {
        let read = |name: &str| std::fs::read_to_string(e.path().join(name)).map(|v| v.trim().to_string()).unwrap_or_default();
        if read("type") == "Mains" {
            mains = true;
            if read("online") == "1" {
                return false;
            }
        }
    }
    mains
}

pub fn low_latency() -> bool {
    LOW_LATENCY.load(Ordering::Relaxed)
}
pub fn replay_cameras() -> bool {
    REPLAY_CAMERAS.load(Ordering::Relaxed)
}
pub fn palette() -> u8 {
    PALETTE.load(Ordering::Relaxed)
}
pub fn hud_minimap() -> u8 {
    HUD_MINIMAP.load(Ordering::Relaxed)
}
pub fn hud_speedo() -> u8 {
    HUD_SPEEDO.load(Ordering::Relaxed)
}
pub fn hud_info() -> u8 {
    HUD_INFO.load(Ordering::Relaxed)
}
pub fn reduced_flashing() -> bool {
    REDUCED_FLASHING.load(Ordering::Relaxed)
}

/// The HUD element a part (bounding box on the 640 × 480 canvas) is: the
/// minimap sits in the bottom-left third, the tachometer / speedometer in
/// the bottom-right, the race information (position, laps, lap times, the
/// leader board) in the top half (HUD_*_Track's layout; read off the
/// race HUD, `docs/evidence/modern-extras/`). Anything else: None.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudElement {
    Minimap,
    Speedo,
    RaceInfo,
}

pub fn hud_element(lo: glam::Vec2, hi: glam::Vec2) -> Option<HudElement> {
    let c = (lo + hi) * 0.5;
    if c.y >= 480.0 * 2.0 / 3.0 && c.x < 640.0 / 3.0 {
        Some(HudElement::Minimap)
    } else if c.y >= 480.0 * 2.0 / 3.0 && c.x >= 640.0 * 2.0 / 3.0 {
        Some(HudElement::Speedo)
    } else if c.y < 240.0 {
        Some(HudElement::RaceInfo)
    } else {
        None
    }
}

/// The scale of a HUD part: the HUD Scale row times its element's row.
pub fn hud_part_scale(hud: f32, lo: glam::Vec2, hi: glam::Vec2) -> f32 {
    let e = match hud_element(lo, hi) {
        Some(HudElement::Minimap) => hud_minimap(),
        Some(HudElement::Speedo) => hud_speedo(),
        Some(HudElement::RaceInfo) => hud_info(),
        None => 100,
    };
    hud * f32::from(e) / 100.0
}

/// The in-game HUD's quads with the HUD Scale row and the per-element
/// rows, and Reduced Flashing; untouched when all are native.
pub fn apply_hud(quads: &mut [crate::draw::quad::Quad]) {
    let hud = crate::qol::hud_scale();
    if hud != 1.0 || hud_minimap() != 100 || hud_speedo() != 100 || hud_info() != 100 {
        crate::qol::scale_hud_by(quads, |lo, hi| hud_part_scale(hud, lo, hi));
    }
    if reduced_flashing() {
        reduce_flashes(quads);
    }
}

/// Reduced Flashing: a bright quad covering most of the canvas (a
/// full-screen flash; dark fades and dims are left alone) is held to
/// [`FLASH_ALPHA`].
pub const FLASH_ALPHA: f32 = 0.25;

/// Reduced Flashing: the NIS SCREEN_FLASH's amount is held to this
/// (the tint pass then brightens by × 1.25 instead of × 1.99,
/// [`crate::screen_flash`]).
pub const FLASH_AMOUNT: f32 = 0.25;

pub fn reduce_flashes(quads: &mut [crate::draw::quad::Quad]) {
    for q in quads.iter_mut() {
        let (lo, hi) = (q.rect[0].min(q.rect[1]), q.rect[0].max(q.rect[1]));
        let full = (hi.x - lo.x) >= 640.0 * 0.9 && (hi.y - lo.y) >= 480.0 * 0.9;
        let luma = 0.299 * q.color[0] + 0.587 * q.color[1] + 0.114 * q.color[2];
        if full && luma > 0.5 {
            q.color[3] = q.color[3].min(FLASH_ALPHA);
        }
    }
}

/// ui.wgsl's `palette` on the CPU (the same arithmetic; for the tests).
pub fn palette_rgb(c: [f32; 3], kind: u8) -> [f32; 3] {
    let [r, g, b] = c;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    if kind == 0 || mx <= 0.0 || d / mx < 0.25 {
        return c;
    }
    let fract = |x: f32| x - x.floor();
    let smooth = |a: f32, b: f32, x: f32| {
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let near = |h: f32, c: f32, w: f32| 1.0 - smooth(w * 0.6, w, (fract(h - c + 0.5) - 0.5).abs());
    let h = if mx == r { fract((g - b) / d / 6.0 + 1.0) } else if mx == g { ((b - r) / d + 2.0) / 6.0 } else { ((r - g) / d + 4.0) / 6.0 };
    let s = d / mx;
    let mut v = mx;
    let (to, w);
    if kind < 3 {
        let (red, green) = (near(h, 0.0, 0.09), near(h, 0.33, 0.12));
        if red >= green {
            to = 0.11;
            w = red;
            if kind == 2 {
                v = (v * (1.0 + 0.35 * red)).min(1.0);
            }
        } else {
            to = 0.56;
            w = green;
        }
    } else {
        let (blue, yellow) = (near(h, 0.62, 0.1), near(h, 0.15, 0.06));
        if blue >= yellow {
            to = 0.04;
            w = blue;
        } else {
            to = 0.88;
            w = yellow;
        }
    }
    let hh = fract(h + (fract(to - h + 0.5) - 0.5) * w);
    let k = |o: f32| ((fract(hh + o) * 6.0 - 3.0).abs() - 1.0).clamp(0.0, 1.0);
    let mixc = |x: f32| v * (1.0 + (x - 1.0) * s);
    [mixc(k(1.0)), mixc(k(2.0 / 3.0)), mixc(k(1.0 / 3.0))]
}

/// The next / previous step of `steps` from `v`.
pub fn step(steps: &[u8], v: u8, right: bool) -> u8 {
    let i = steps.iter().position(|&s| s >= v).unwrap_or(steps.len() - 1);
    if right { steps[(i + 1).min(steps.len() - 1)] } else { steps[i.saturating_sub(1)] }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Power Saving and Draw less: Off / Pause / Keep running never cap;
    /// Draw less caps only a background window, at 30 fps; Menus caps the
    /// menus at the refresh rate; Battery adds the game on battery; the
    /// longest cap wins.
    #[test]
    fn power_saving_caps() {
        use std::time::Duration;
        let hz = |f: f64| Some(Duration::from_secs_f64(1.0 / f));
        let fg = PowerState { foreground: true, menu: false, battery: false, refresh_hz: Some(144.0) };
        for level in 0..=2 {
            for bgs in 0..=2 {
                assert_eq!(power_cap(level, bgs, fg), None, "level {level} background {bgs}");
            }
        }
        let bg = PowerState { foreground: false, ..fg };
        assert_eq!(power_cap(0, BACKGROUND_PAUSE, bg), None);
        assert_eq!(power_cap(0, BACKGROUND_KEEP_RUNNING, bg), None);
        assert_eq!(power_cap(0, BACKGROUND_DRAW_LESS, bg), hz(30.0));
        let menu = PowerState { menu: true, ..fg };
        assert_eq!(power_cap(0, 0, menu), None);
        assert_eq!(power_cap(1, 0, menu), hz(144.0));
        assert_eq!(power_cap(1, BACKGROUND_DRAW_LESS, PowerState { foreground: false, ..menu }), hz(30.0));
        let battery = PowerState { battery: true, ..fg };
        assert_eq!(power_cap(1, 0, battery), None);
        assert_eq!(power_cap(2, 0, battery), hz(144.0));
        assert_eq!(power_cap(2, 0, PowerState { refresh_hz: None, ..battery }), hz(60.0));
        assert_eq!(power_cap(2, 0, PowerState { refresh_hz: Some(1000.0), ..battery }), hz(240.0));
    }

    /// Draw less never draws under 30 fps: a drawn frame steps at most 3
    /// race / 2 scene ticks (60 Hz), so a slower rate would drop time.
    #[test]
    fn draw_less_keeps_the_world_rate() {
        assert!(BACKGROUND_FPS >= 30.0);
        for rate in [crate::frame_clock::RACE, crate::frame_clock::SCENE] {
            assert!(f64::from(rate.cap) * rate.update * BACKGROUND_FPS >= 1.0, "{rate:?}");
        }
    }

    /// The older Power Saving line: its Background step becomes Draw less,
    /// the rest one step down; native writes neither line.
    #[test]
    fn old_power_saving_lines_migrate() {
        for (old, background, level) in [(0, 0, 0), (1, 2, 0), (2, 2, 1), (3, 2, 2), (9, 2, 2)] {
            let mut e = Extras::default();
            assert!(e.parse("enh.power_saving", &old.to_string()));
            assert_eq!((e.background, e.power_saving), (background, level), "old {old}");
            let mut s = String::new();
            e.write(&mut s);
            assert!(!s.contains("enh.power_saving "), "{s}");
        }
        let mut e = Extras::default();
        assert!(e.parse("enh.power_saving", "2") && e.parse("enh.background", "1"));
        assert_eq!((e.background, e.power_saving), (1, 1), "a later line wins");
    }

    /// Forced Keep running (scripted runs) overrides the setting.
    #[test]
    fn forced_keep_running() {
        let e = Extras::default();
        e.apply();
        assert_eq!(background(), BACKGROUND_PAUSE);
        force_keep_running(true);
        assert_eq!(background(), BACKGROUND_KEEP_RUNNING);
        force_keep_running(false);
        assert_eq!(background(), BACKGROUND_PAUSE);
    }

    /// Battery: only with a mains adapter listed and none online.
    #[test]
    fn battery_from_power_supplies() {
        let dir = std::env::temp_dir().join(format!("nfsu2-power-{}", crate::unique_tag()));
        let supply = |name: &str, ty: &str, online: &str| {
            let d = dir.join(name);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("type"), format!("{ty}\n")).unwrap();
            std::fs::write(d.join("online"), format!("{online}\n")).unwrap();
        };
        assert!(!on_battery_in(&dir), "no folder");
        supply("BAT0", "Battery", "1");
        assert!(!on_battery_in(&dir), "no mains entry: not known");
        supply("AC", "Mains", "1");
        assert!(!on_battery_in(&dir));
        supply("AC", "Mains", "0");
        assert!(on_battery_in(&dir));
        supply("USB-C", "Mains", "1");
        assert!(!on_battery_in(&dir), "any mains online");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trip_and_defaults() {
        let d = Extras::default();
        let mut s = String::new();
        d.write(&mut s);
        assert!(s.is_empty(), "native writes nothing");
        let e = Extras { low_latency: true, replay_cameras: true, palette: 2, hud_minimap: 130, hud_speedo: 80, hud_info: 150, reduced_flashing: true, background: 2, power_saving: 2, mark_spot: 1 };
        e.write(&mut s);
        let mut back = Extras::default();
        for line in s.lines() {
            let (k, v) = line.split_once(' ').unwrap();
            assert!(back.parse(k, v));
        }
        assert_eq!(back, e);
        let mut v = Vec::new();
        e.deviations(&mut v);
        assert_eq!(v.len(), 10);
        assert!(back.parse("enh.hud_minimap", "200") && back.hud_minimap == 150);
        assert_eq!(step(&ELEMENT_STEPS, 100, true), 115);
    }

    #[test]
    fn palettes_move_the_confused_hues_only() {
        let hue = |c: [f32; 3]| {
            let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
            let d = mx - mn;
            let h = if mx == c[0] { ((c[1] - c[2]) / d / 6.0 + 1.0).fract() } else if mx == c[1] { ((c[2] - c[0]) / d + 2.0) / 6.0 } else { ((c[0] - c[1]) / d + 4.0) / 6.0 };
            h * 360.0
        };
        let (red, green, blue, yellow, grey, white) = ([0.9, 0.1, 0.1], [0.1, 0.8, 0.2], [0.1, 0.2, 0.9], [0.9, 0.85, 0.1], [0.5, 0.5, 0.5], [1.0, 1.0, 1.0]);
        for kind in 1..=3 {
            // Greys and whites never change.
            assert_eq!(palette_rgb(grey, kind), grey);
            assert_eq!(palette_rgb(white, kind), white);
            assert_eq!(palette_rgb(red, 0), red);
        }
        // Deuteranopia: red to amber (~40°), green to sky blue (~200°).
        assert!((hue(palette_rgb(red, 1)) - 40.0).abs() < 3.0, "{:?}", palette_rgb(red, 1));
        assert!((hue(palette_rgb(green, 1)) - 200.0).abs() < 8.0, "{:?}", palette_rgb(green, 1));
        // Blue untouched for red-green kinds.
        assert!((hue(palette_rgb(blue, 1)) - hue(blue)).abs() < 1.0);
        // Protanopia: the red's value raised.
        let p = palette_rgb(red, 2);
        assert!(p.iter().cloned().fold(0.0, f32::max) > 0.95);
        // Tritanopia: blue to vermilion, yellow to pink, red kept.
        assert!(hue(palette_rgb(blue, 3)) < 25.0, "{:?}", palette_rgb(blue, 3));
        assert!((hue(palette_rgb(yellow, 3)) - 317.0).abs() < 15.0, "{:?}", palette_rgb(yellow, 3));
        assert!((hue(palette_rgb(red, 3)) - hue(red)).abs() < 1.0);
    }

    #[test]
    fn hud_elements_and_flashes() {
        let v = glam::Vec2::new;
        assert_eq!(hud_element(v(20.0, 360.0), v(130.0, 470.0)), Some(HudElement::Minimap));
        assert_eq!(hud_element(v(510.0, 360.0), v(630.0, 470.0)), Some(HudElement::Speedo));
        assert_eq!(hud_element(v(470.0, 20.0), v(630.0, 120.0)), Some(HudElement::RaceInfo));
        assert_eq!(hud_element(v(280.0, 300.0), v(360.0, 340.0)), None);
        let quad = |rect: [glam::Vec2; 2], color: [f32; 4]| crate::draw::quad::Quad { texture: None, rect, uv: [v(0.0, 0.0), v(1.0, 1.0)], color, z: 0.0, angle: 0.0, place: Default::default() };
        let mut q = [quad([v(0.0, 0.0), v(640.0, 480.0)], [1.0, 1.0, 1.0, 0.9]), quad([v(0.0, 0.0), v(640.0, 480.0)], [0.0, 0.0, 0.0, 0.9]), quad([v(0.0, 0.0), v(100.0, 100.0)], [1.0, 1.0, 1.0, 1.0])];
        reduce_flashes(&mut q);
        assert_eq!(q.map(|q| q.color[3]), [FLASH_ALPHA, 0.9, 1.0]);
        assert_eq!(step(&ELEMENT_STEPS, 70, false), 70);
    }
}
