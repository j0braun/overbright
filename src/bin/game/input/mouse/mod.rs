//! The front end's mouse (see `nfsu2::frontend::mouse` and docs/formats.md
//! "Front-end mouse"): the window's events sampled once a frame, the
//! cursor, and each ported screen's handling of the mouse message
//! 0x9803F6E2. Every action goes through the keyboard's path
//! (`Game::menu_key`), so the screens' sounds and logic stay one.

use glam::Vec2;
use nfsu2::frontend::mouse::{self as fm, Mouse};
use nfsu2::frontend::screens::{self, Key};
use nfsu2::render::display::UiFrame;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};

use crate::{Dialog, Game, Menu, Mode, Quad};
use nfsu2::formats::hash::fe_hash;
use std::cell::Cell;

mod controls;
mod dialogs;
mod help_bar;
mod icons;
mod lists;
mod lots;
mod maps;
mod options;
mod paint;
mod parts;
mod showcase;

/// G35: the front end mouse's own state (were thread-locals), on
/// `Platform::mouse_state`.
pub(crate) struct MouseState {
    /// The held button has acted this press (`MouseCtx::held_once`).
    pub(crate) held_fired: bool,
    /// Whether the front end's sounds are held back (a silent mouse step).
    pub(crate) mute: bool,
    /// The scroll bars' last arrow click (+0x38, 0x52E480): the real-time
    /// clock 0x8651A8 then (`GameClock::real`, 1/4000 s).
    pub(crate) arrow_at: Option<u64>,
    /// Where the cursor was when the clicking button went down, while the
    /// press waits for its tick (a click acts at the point it
    /// arrived at, not where the cursor moved to before the tick).
    pub(crate) press_at: Option<Vec2>,
    /// UI_InGame_WorldMap's last zone (the zone list's [0], 0x4CBEE0):
    /// its kind, index and the map's opening time.
    pub(crate) map_zone: Option<(u8, usize, f32)>,
}

impl Default for MouseState {
    fn default() -> MouseState {
        MouseState { held_fired: false, mute: false, arrow_at: None, press_at: None, map_zone: None }
    }
}

/// One frame's mouse as the screens' handlers take it (`Game::fe_mouse`).
pub(super) struct MouseCtx<'a> {
    pub(super) m: &'a Mouse,
    pub(super) frame: &'a UiFrame,
    pub(super) px: Vec2,
    pub(super) now: f32,
    /// A pure move: no click, no wheel (0x52FC50, 0x55D420, 0x4ED140).
    pub(super) hover: bool,
    /// The wheel as a step (0x50CDA0: lZ < 0 → +1, > 0 → −1).
    pub(super) wheel: Option<BarHit>,
    /// `MouseState::held_fired` and `arrow_at` for the screen's handler,
    /// written back after it (`Game::fe_mouse`).
    pub(super) held_fired: Cell<bool>,
    pub(super) arrow_at: Cell<Option<u64>>,
    /// The real-time clock 0x8651A8 at this tick (`GameClock::real`).
    pub(super) real: u64,
}

impl MouseCtx<'_> {
    /// The button held over an object (0x52D0A0) acts each message until a
    /// dialog opens over the screen; the clone acts once a press (its Select
    /// can act without the original's question, which would repeat).
    fn held_once(&self) -> bool {
        self.m.held && !self.held_fired.replace(true)
    }

    /// 0x52D030 (an object's rectangle, hidden or not: 0x51DA00 has no
    /// visibility test).
    fn hit(&self, package: &str, hash: u32) -> bool {
        fm::rect_any(package, hash).is_some_and(|r| fm::inside(self.frame, self.px, &r))
    }

    /// Only what is drawn (the clone's stand-in where a hit test needs
    /// the object shown: the help bar's slots, the dialogs' buttons).
    fn drawn(&self, package: &str, hash: u32) -> bool {
        fm::rect(package, hash).is_some_and(|r| fm::inside(self.frame, self.px, &r))
    }

    /// A scroll bar's result this frame (`scroll_bar`).
    fn bar(&self, package: &str, prefix: &str) -> Option<BarHit> {
        scroll_bar(self.m, self.frame, self.px, package, prefix, self.real, &self.arrow_at)
    }

    /// The cursor on the centred 640×480 canvas, from its centre (the
    /// option lists' rows are kept there, `fm::slider_rows`).
    fn canvas(&self) -> Vec2 {
        fm::canvas_point(self.frame, self.px, Default::default()) - Vec2::new(320.0, 240.0)
    }
}

/// Whether a canvas point is in a box (`lo`, `size`), edges included.
fn in_box(canvas: Vec2, lo: Vec2, size: Vec2) -> bool {
    canvas.x >= lo.x && canvas.x <= lo.x + size.x && canvas.y >= lo.y && canvas.y <= lo.y + size.y
}

/// The arrows the screens name 0xCF6F0439 / 0xABA93D3F (PC_LTrigger_Icon /
/// PC_RTrigger_Icon, U2_Menu_Arrow): the triggers' steps.
const LT_ARROW: u32 = 0xCF6F_0439;
const RT_ARROW: u32 = 0xABA9_3D3F;
/// ScrollBar_Arrow_1 / _2.
const ARROW_1: u32 = 0x4449_69FD;
const ARROW_2: u32 = 0x4449_69FE;
/// MINI_MASTER, the parts columns' current-row frame.
const MINI_MASTER: u32 = 0x38E3_9AD7;
/// A track click's steps (0x7FA1A8 = 3, static).
const PAGE: usize = 3;
/// The arrows' repeat gate (0x538216..0x53823A, **traced**): an arrow
/// press acts when (0x8651A8 − +0x38) × 0.00025 > 0.0625 (0x784268,
/// 0x79BDA4), +0x38 being 0x8651A8 at the last arrow click (0x52E493 /
/// 0x52E4BC): more than 250 units of the real-time clock (1/4000 s). The
/// clock steps 67, 64, 69 per RACE tick and 133 / 136 per SCENE tick, so a
/// difference of exactly 250 (where the x87 precision would decide) never
/// occurs: the gate opens on the 4th RACE tick after a click, at every
/// display rate.
const ARROW_GATE_UNITS: u64 = 250;

/// [`ARROW_GATE_UNITS`]: whether an arrow press at real-time clock `real`
/// acts after the last arrow click at `last`.
pub(crate) fn arrow_gate_open(real: u64, last: Option<u64>) -> bool {
    last.is_none_or(|l| real.saturating_sub(l) > ARROW_GATE_UNITS)
}

/// A scroll bar's result (0x538140 → +0x3C): an arrow click or a drag step
/// (−1 / +1), a click on the track before / after the handle (codes 10 /
/// −10, here −1 / +1). Only the arrows sound (0x52E480: 0x83804C /
/// 0x838067, the table's LEFT / RIGHT on column 0); the screens' steps
/// themselves are silent (**observed** under Wine with sndwatch: on the
/// Quick Race track select an arrow click played one id 3, as the Right
/// key, and a track click stepping three tracks played nothing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BarHit {
    /// An arrow clicked (0x52E480, with the bar's own sound).
    Arrow(i32),
    /// A drag step (0x52E4F0), or the wheel: silent.
    Step(i32),
    /// A click on the track (0x52E680): silent.
    Page(i32),
}

/// 0x538140 on a package's bar `prefix` (its _Backing, _Handle, _Arrow_1,
/// _Arrow_2), **verified**: a press on the handle starts a drag (+2, the
/// grab offset +0x14 / +0x18 = |handle's top left − cursor|), a press on
/// the backing pages (0x52E680); the release ends the drag; while dragging
/// the handle follows the cursor, clamped to the track, and one step comes
/// each message once the cursor less the grab offset is past the step
/// (+0x34) from the handle's home centre (0x52E4F0); not dragging, a press
/// on an arrow steps, at most once in 62.5 ms of the real-time clock
/// (0x52E480, [`arrow_gate_open`]). Hit tests ignore
/// visibility (0x51DA00).
pub(crate) fn scroll_bar(
    m: &Mouse,
    frame: &UiFrame,
    px: Vec2,
    package: &str,
    prefix: &str,
    real: u64,
    arrow_at: &Cell<Option<u64>>,
) -> Option<BarHit> {
    let name = |s: &str| fe_hash(&format!("{prefix}_{s}"));
    let hit = |h: u32| fm::rect_any(package, h).is_some_and(|r| fm::inside(frame, px, &r));
    let mine = |d: &fm::Drag| d.package == package && d.prefix == prefix;
    let mut out = None;
    if let (Some(bar), Some(back)) = (fm::bar(package, prefix), fm::rect_any(package, name("Backing"))) {
        let p = fm::canvas_point(frame, px, back.place) - Vec2::new(320.0, 240.0);
        let along = if bar.horizontal { p.x } else { p.y };
        let handle = fm::rect_any(package, name("Handle"))
            .map(|h| if bar.horizontal { (h.lo.x - 320.0, h.hi.x - 320.0) } else { (h.lo.y - 240.0, h.hi.y - 240.0) });
        if m.pressed && !fm::drag().is_some_and(|d| mine(&d)) {
            if let Some((lo, _)) = handle.filter(|_| hit(name("Handle"))) {
                fm::set_drag(Some(fm::Drag {
                    package: package.to_string(),
                    prefix: prefix.to_string(),
                    grab: (lo - along).abs(),
                    top_left: lo,
                }));
            } else if hit(name("Backing"))
                && let Some((lo, hi)) = handle
            {
                if along < lo {
                    out = Some(BarHit::Page(-1));
                } else if along > hi {
                    out = Some(BarHit::Page(1));
                }
            }
        }
        if (m.released || !m.held) && fm::drag().is_some_and(|d| mine(&d)) {
            fm::set_drag(None);
        }
        if let Some(mut d) = fm::drag().filter(|d| mine(d)) {
            d.top_left = (along - d.grab).clamp(bar.start, bar.start + bar.length - bar.side);
            let off = along - d.grab - bar.home;
            fm::set_drag(Some(d));
            return if off > bar.step {
                Some(BarHit::Step(1))
            } else if off < -bar.step {
                Some(BarHit::Step(-1))
            } else {
                out
            };
        }
    }
    if m.pressed && arrow_gate_open(real, arrow_at.get()) {
        for (arrow, d) in [("Arrow_1", -1), ("Arrow_2", 1)] {
            if hit(name(arrow)) {
                arrow_at.set(Some(real));
                return Some(BarHit::Arrow(d));
            }
        }
    }
    out
}

/// The help bar's message for a slot's bit (0x52D180, **verified**), as
/// the key the clone's screens take; None for the bits 0x52D180 maps to
/// nothing. 0x80000 / 0x100000 / 0x200000 (Test N2O Purge, Open/Close
/// Hood / Doors) give 0xC539A4B0, which only UI_ICEMAIN's handler takes
/// (0x568BC2) and hands its parts overlay as 0xC519BFC0 — BUTTON1, the
/// clone's Key::Button1 there.
pub(crate) fn help_message(bit: u32, paint_category: bool, tuning_master: bool) -> Option<HelpMsg> {
    Some(match bit {
        4 | 8 | 0x10 | 0x20 | 0x40 | 0x80 | 0x200_0000 | 0x400_0000 | 0x800_0000 => HelpMsg::Key(Key::Accept),
        0x100 => HelpMsg::Key(Key::Accept),
        0x200 if paint_category => HelpMsg::Key(Key::Button1),
        0x200 | 0x400 => HelpMsg::Key(Key::Back),
        0x800 if tuning_master => HelpMsg::Key(Key::Button1),
        0x800 => HelpMsg::Key(Key::Button0),
        0x1000 | 0x2000 | 0x4000 | 0x4_0000 | 0x80_0000 | 0x100_0000 | 0x1000_0000 | 0x2000_0000 | 0x4000_0000 => {
            HelpMsg::Key(Key::Button1)
        }
        0x8_0000 | 0x10_0000 | 0x20_0000 => HelpMsg::Key(Key::Button1),
        0x40_0000 => HelpMsg::Tip,
        1 => HelpMsg::Quit,
        2 => HelpMsg::Logoff,
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HelpMsg {
    Key(Key),
    /// 0xC519BFC3, the tuning screens' Tip.
    Tip,
    /// Mask bit 1: "Are you sure you want to quit?" (0x55DE10).
    Quit,
    /// Mask bit 2: sound 0x4B, then the log-off question (online only).
    Logoff,
}

/// The tuning screens' hints (0x510120 for the slider rows; by category
/// on the graph / NOS screen, 0x5654E0), **verified** under Wine.
pub(crate) fn tuning_hint(param: Option<u8>, category: usize) -> u32 {
    match param {
        Some(0 | 4) => 0xD7C2_416A,
        Some(1 | 5) => 0x6AD0_772F,
        Some(2 | 6) => 0xC455_3F70,
        Some(3 | 7) => 0xF51B_D160,
        Some(8) => 0x184A_B1A3,
        Some(9) => 0x2EA4_3025,
        Some(10..=15) => 0xE2E9_8554,
        Some(18) => 0x76E7_21C7,
        Some(19 | 20) => 0xC7E6_E649,
        Some(21 | 22) => 0x9DCC_6E8B,
        _ => match category {
            4 => 0xDACF_52C1,
            5 => 0xCD08_5170,
            _ => 0x46B4_1BB8,
        },
    }
}

/// The button that clicks: g_Mouse +0x20 (0x8763F0) is set once, by the
/// mouse object's constructor 0x5C8610 (a static initializer, 0x77E580),
/// from GetSystemMetrics(SM_SWAPBUTTON) (**verified**): then the right
/// button is the one the screens test. Windows only; elsewhere no system
/// setting is read (the window system swaps the buttons itself).
fn clicking_button() -> MouseButton {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetSystemMetrics(index: i32) -> i32;
        }
        static SWAP: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        // SM_SWAPBUTTON = 0x17.
        if *SWAP.get_or_init(|| unsafe { GetSystemMetrics(0x17) } != 0) {
            return MouseButton::Right;
        }
    }
    MouseButton::Left
}

/// A window mouse event as the tick queue takes it: the
/// cursor's point, the clicking button — the left, or the right with the
/// swap flag (0x8763F0, `clicking_button`) — and the wheel in lines; None
/// for the rest.
pub(crate) fn mouse_input(event: &WindowEvent) -> Option<crate::InputEvent> {
    use crate::InputEvent;
    Some(match event {
        WindowEvent::CursorMoved { position, .. } => InputEvent::CursorMoved(Vec2::new(position.x as f32, position.y as f32)),
        WindowEvent::MouseInput { state, button, .. } if *button == clicking_button() => {
            InputEvent::Button(*state == ElementState::Pressed)
        }
        WindowEvent::MouseWheel { delta, .. } => InputEvent::Wheel(match delta {
            MouseScrollDelta::LineDelta(_, y) => *y,
            MouseScrollDelta::PixelDelta(p) => (p.y as f32).signum(),
        }),
        _ => return None,
    })
}

/// A queued mouse event applied at its tick (`now`: the screens' clock):
/// the mouse is sampled once a tick (`Game::fe_mouse`, as 0x5BF870 polls
/// DirectInput once a frame), so moves coalesce to the newest point; a
/// press keeps the point it came at (`MouseState::press_at`).
pub(crate) fn apply_mouse(mouse: &mut Mouse, state: &mut MouseState, event: &crate::InputEvent, now: f32) {
    use crate::InputEvent;
    match *event {
        InputEvent::CursorMoved(p) => {
            if mouse.at != Some(p) {
                mouse.moved_at = now;
            }
            mouse.at = Some(p);
        }
        InputEvent::Button(true) => {
            mouse.held = true;
            if !mouse.pressed {
                state.press_at = mouse.at;
            }
            mouse.pressed = true;
        }
        InputEvent::Button(false) => {
            mouse.held = false;
            mouse.released = true;
        }
        InputEvent::Wheel(lines) => mouse.wheel += lines,
        _ => {}
    }
}

/// The tick's sample of the mouse (`Game::fe_mouse`): a press acts at the
/// point it came at; the cursor's newer point is then the next tick's move
/// (`last` after `end_frame`).
pub(crate) fn sample_mouse(mouse: &mut Mouse, state: &mut MouseState) -> Mouse {
    let mut m = *mouse;
    mouse.end_frame();
    if let Some(p) = state.press_at.take().filter(|_| m.pressed) {
        m.at = Some(p);
        mouse.last = Some(p);
    }
    m
}

impl Game {
    /// The window's UI frame (a headless run's: NFSU2_SHOT_SIZE or
    /// 1280 × 960).
    pub(crate) fn ui_frame(&self) -> Option<UiFrame> {
        match self.platform.window.as_ref() {
            Some(w) => Some(UiFrame::new((w.config.width, w.config.height))),
            None => Some(UiFrame::new(crate::dev_hooks::headless_size().unwrap_or((1280, 960)))),
        }
    }

    /// The window point at `f` (0..1 across, 0..1 down) of `r`.
    pub(crate) fn mouse_point(&self, r: fm::Rect, f: Vec2) -> Option<Vec2> {
        let frame = self.ui_frame()?;
        let c = r.lo + (r.hi - r.lo) * f;
        Some(Vec2::new(
            frame.origin.0 + c.x * frame.scale.0 + r.place.x as f32 * frame.reach_x(r.place),
            frame.origin.1 + c.y * frame.scale.1 + r.place.y as f32 * frame.reach.1,
        ))
    }

    /// The cursor (0x50B460): texture 0x6B6A294F at the point less (12, 6),
    /// its own size, over the front end. 0x5378D0 draws it once the front
    /// end exists (0x8384C4), with no movie playing (0x8383AC, set by the
    /// movie player's start 0x566030 and cleared at its end 0x5661B3; the
    /// boot movies and the attract loop hold it: no cursor there under
    /// Wine, the arrow first on UG_LS_Splash) and 0x7FA29C set; 0x50B460
    /// then draws it always while g_GameState 0x8654A4 is 3 (the front
    // prov: inferred game.mouse.cursor_quad
    /// end, here no world loaded: **inferred** mapping) and else within
    /// 2 s of a move. Never while a load runs (the loading screen is not
    /// the front end's frame). Agreed deviation (docs/playing/native-deviations.md "Mouse cursor"): none
    /// while driving (free roam, races, the in-engine scenes; the
    /// original shows it there for 2 s after a move, seen under Wine);
    /// the menus over the world (pause, map, the dyno) and the replay keep
    /// the 2 s rule.
    pub(crate) fn cursor_quad(&self, now: f32) -> Option<Quad> {
        let px = self.platform.mouse.at?;
        if !self.cursor_shown(now) {
            return None;
        }
        let frame = self.ui_frame()?;
        let tex = self.game_data.assets.textures.get(&fm::CURSOR)?;
        let place = fm::cursor_place(&frame, px);
        let p = fm::canvas_point(&frame, px, place);
        let p = Vec2::new(p.x.trunc(), p.y.trunc()) + fm::CURSOR_OFFSET;
        let size = Vec2::new(tex.width as f32, tex.height as f32);
        Some(Quad {
            texture: Some(fm::CURSOR),
            rect: [p, p + size],
            uv: [Vec2::ZERO, Vec2::ONE],
            color: [1.0, 1.0, 1.0, 1.0],
            z: 0.0,
            angle: 0.0,
            place,
        })
    }

    /// Whether the game's cursor is drawn this frame (`cursor_quad`).
    pub(crate) fn cursor_shown(&self, now: f32) -> bool {
        if self.platform.mouse.at.is_none() || self.loads.loading.is_some() {
            return false;
        }
        // The texture editor's panel needs the cursor (`--editor` only).
        if crate::enh::texed::open() {
            return true;
        }
        // **Traced**: FE_DrawCursor 0x50B460 draws it while
        // (0x8651A8 − 0x83A9CC) × 0.00025 ≤ 2.0 (0x50B49E..0x50B4BD,
        // 0x7844C0), the real-time clock, not the package clock: up to
        // 7999 ticks of 1/4000 s after the move. `now` is kept for the
        // callers' signature.
        let _ = now;
        let recent = !nfsu2::game_clock::real_over(self.real_now(), self.platform.mouse.moved_real, fm::SHOW_MS / 1000.0);
        match &self.mode {
            Mode::Menu(Menu::Movie(_)) => false,
            // Agreed deviation (docs/playing/native-deviations.md "Mouse cursor"): no cursor before the
            // menus are reached; the original already shows it on the
            // "Press Enter Key" splash.
            Mode::Menu(Menu::Splash(_)) => false,
            Mode::Menu(_) => self.sim.drive.is_none() || recent,
            Mode::Race(r) => r.replay.playback.is_some() && recent,
            Mode::Explore => false,
        }
    }

    /// The screen up and its selection, for `NFSU2_INPUT_LOG` (the
    /// real-event mouse test, scripts/mouse_xtest.sh).
    pub(crate) fn input_tag(&self) -> String {
        match &self.mode {
            // The GPS's target and state (0x501100) in free roam.
            Mode::Explore => match self.career_world.gps.as_ref() {
                Some(g) => format!("Explore:gps={:08X}:{}", g.target, g.state()),
                None => "Explore:gps=-".to_string(),
            },
            Mode::Race(r) => format!("Race{}", if r.replay.playback.is_some() { ":replay" } else { "" }),
            Mode::Menu(m) => match m {
                Menu::Icons(i, k) => format!("Icons:{k:?}:{}", i.selected),
                Menu::Dialog(d, _) => format!("Dialog:{}:{}", d.package, d.selected),
                Menu::McMain(mc) => format!("McMain:{:?}:{}", mc.step, mc.selected),
                Menu::McMessage(mc, _) => format!("McMessage:{}", mc.selected),
                Menu::Options(o) => format!("Options:{}", o.selected),
                Menu::CarLot { .. } => "CarLot".to_string(),
                Menu::LotDeal { .. } => "LotDeal".to_string(),
                Menu::WorldMap { .. } => "WorldMap".to_string(),
                // The selected marker (index, target hash), the GPS
                // (on / off) and the focused legend row.
                Menu::InGameMap(map) => format!(
                    "InGameMap:sel={}:{:08X}:gps={}:focus={}",
                    map.selected.map_or("-".to_string(), |s| s.to_string()),
                    map.selected.and_then(|s| map.icons.get(s)).and_then(|i| i.target).map_or(0, |t| t.0),
                    map.gps as u8,
                    map.focus.map_or("-".to_string(), |f| f.to_string())
                ),
                Menu::QuickRace { .. } => "QuickRace".to_string(),
                Menu::QrOptions { .. } => "QrOptions".to_string(),
                Menu::McList { .. } => "McList".to_string(),
                Menu::Reward { .. } => "Reward".to_string(),
                Menu::Engage { .. } => "Engage".to_string(),
                Menu::ShopEngage { .. } => "ShopEngage".to_string(),
                Menu::Challenge { .. } => "Challenge".to_string(),
                Menu::Parts { .. } => "Parts".to_string(),
                Menu::Paint { .. } => "Paint".to_string(),
                Menu::ChooseHud { .. } => "ChooseHud".to_string(),
                Menu::Results { .. } => "Results".to_string(),
                Menu::Cover { .. } => "Cover".to_string(),
                Menu::Showcase { .. } => "Showcase".to_string(),
                Menu::DynoChart { .. } => "DynoChart".to_string(),
                Menu::TuneGraph { .. } => "TuneGraph".to_string(),
                Menu::DynoResults { .. } => "DynoResults".to_string(),
                Menu::TuneSliders { .. } => "TuneSliders".to_string(),
                Menu::Stats { .. } => "Stats".to_string(),
                Menu::SponsorPopup { .. } => "SponsorPopup".to_string(),
                Menu::Splash { .. } => "Splash".to_string(),
                Menu::IspConnect { .. } => "IspConnect".to_string(),
                Menu::LanServers { .. } => "LanServers".to_string(),
                Menu::EaLogin { .. } => "EaLogin".to_string(),
                Menu::Credits { .. } => "Credits".to_string(),
                Menu::Controls { .. } => "Controls".to_string(),
                Menu::Jukebox { .. } => "Jukebox".to_string(),
                Menu::BrandSelect { .. } => "BrandSelect".to_string(),
                Menu::Movie { .. } => "Movie".to_string(),
                Menu::SmsMailbox { .. } => "SmsMailbox".to_string(),
                Menu::SmsViewer { .. } => "SmsViewer".to_string(),
                Menu::Status { .. } => "Status".to_string(),
            },
        }
    }

    /// `NFSU2_INPUT_LOG=path`: one line a frame with mouse activity, a new
    /// screen or selection, or the cursor shown / hidden: the time (ms),
    /// the cursor's window point, the edges, the wheel, whether the game's
    /// cursor is drawn, and the screen tag after the frame's mouse.
    pub(crate) fn input_log(&mut self, m: &Mouse, before: &str, shown: bool) {
        thread_local! {
            static LOG: std::cell::RefCell<Option<(std::fs::File, String, bool)>> = std::cell::RefCell::new(
                crate::debug_env().input_log.clone().and_then(|p| std::fs::File::create(p).ok()).map(|f| (f, String::new(), false)));
        }
        let tag = self.input_tag();
        let now = self.now();
        LOG.with(|l| {
            use std::io::Write;
            let mut l = l.borrow_mut();
            let Some((f, last, last_shown)) = l.as_mut() else { return };
            if !(m.active() || tag != *last || before != tag || shown != *last_shown) {
                return;
            }
            let at = m.at.map_or("-".to_string(), |p| format!("{},{}", p.x, p.y));
            let _ = write!(
                f,
                "t={now:.0} at={at} p={} h={} r={} w={} cursor={} tag={tag}",
                m.pressed as u8, m.held as u8, m.released as u8, m.wheel, shown as u8
            );
            // NFSU2_INPUT_PROBE="Package:Object,…" (an object's name or
            // #hash): the window point at the centre of each such object
            // the last frame laid out (drawn or hidden).
            if let Some(probe) = crate::debug_env().input_probe.clone() {
                for item in probe.split(',').filter(|s| !s.is_empty()) {
                    // "rows": the option rows' title boxes and arrows.
                    if item == "rows"
                        && let Some(frame) = self.ui_frame()
                    {
                        let px = |lo: Vec2, size: Vec2| {
                            frame.to_pixels(lo + size * 0.5 + Vec2::new(320.0, 240.0), Default::default()).round()
                        };
                        for r in fm::slider_rows() {
                            let (t, a, b) = (px(r.1, r.2), px(r.3.0, r.3.1), px(r.4.0, r.4.1));
                            let _ = write!(
                                f,
                                " row{}@{},{} rowL{}@{},{} rowR{}@{},{}",
                                r.0, t.x, t.y, r.0, a.x, a.y, r.0, b.x, b.y
                            );
                        }
                        continue;
                    }
                    // "map": UI_InGame_WorldMap's marker zones (0x4CBEE0
                    // kind 0), `m<icon>=<target>@x,y`.
                    if item == "map" {
                        if let Mode::Menu(Menu::InGameMap(map)) = &self.mode
                            && let Some(pkg) = self.game_data.assets.package("UI_InGame_WorldMap")
                        {
                            for (i, h) in map.mouse_markers(pkg) {
                                let t = map.icons[i].target.map_or(0, |t| t.0);
                                if let Some(p) = fm::rect("UI_InGame_WorldMap", h)
                                    .and_then(|r| self.mouse_point(r, Vec2::splat(0.5)))
                                {
                                    let _ = write!(f, " m{i}={t:08X}@{},{}", p.x.round(), p.y.round());
                                }
                            }
                        }
                        continue;
                    }
                    let Some((package, object)) = item.split_once(':') else { continue };
                    let hash = match object.strip_prefix('#') {
                        Some(h) => u32::from_str_radix(h, 16).unwrap_or(0),
                        None => fe_hash(object),
                    };
                    if let Some(p) = fm::rect(package, hash).and_then(|r| self.mouse_point(r, Vec2::splat(0.5))) {
                        let _ = write!(f, " {item}@{},{}", p.x.round(), p.y.round());
                    }
                }
            }
            let _ = writeln!(f);
            let _ = f.flush();
            *last = tag;
            *last_shown = shown;
        });
    }

    fn mouse_key(&mut self, key: Key) -> anyhow::Result<()> {
        self.menu_key(key)?;
        self.track_undim();
        Ok(())
    }

    /// UI_ReplayControl's mouse (0x4D7BD0 → 0x4D7B30, **verified**): with
    /// the bar hidden (+0x74) a click shows it again (0x4A5200); else a
    /// press on a button (0x52D110 on the objects the constructor 0x4C0AA0
    /// keeps at +0x54…+0x6C): 0xAC6BDBC6 skip back (0x4A4CA0), 0x8A10C890
    /// slow motion (0x4A4FB0), 0xA6B6DD4A skip forward (0x4A4DD0),
    /// 0x9C0AE9AA the camera (0x4A5090), 0x8A094D85 Back (0x4C1220) and
    /// 0x8A0AB225 hide (0x4A5200, unless +0x50). Play / pause (+0x58,
    /// 0xCBEDA229) is not tested. The clone sends the keys' own paths.
    fn replay_click(&mut self, px: Vec2) -> anyhow::Result<()> {
        use winit::keyboard::KeyCode;
        let Some(frame) = self.ui_frame() else { return Ok(()) };
        let Mode::Race(r) = &mut self.mode else { return Ok(()) };
        let Some(p) = r.replay.playback.as_mut() else { return Ok(()) };
        if p.leaving {
            return Ok(());
        }
        if p.hidden {
            p.hidden = false;
            return Ok(());
        }
        let hit = |hash: u32| fm::rect("UI_ReplayControl", hash).is_some_and(|r| fm::inside(&frame, px, &r));
        if hit(0x8A0A_B225) {
            p.hidden = true;
            return Ok(());
        }
        let code = [
            (0xAC6B_DBC6, KeyCode::BracketLeft),
            (0x8A10_C890, KeyCode::Comma),
            (0xA6B6_DD4A, KeyCode::BracketRight),
            (0x9C0A_E9AA, KeyCode::KeyC),
            (0x8A09_4D85, KeyCode::Escape),
        ]
        .into_iter()
        .find(|&(h, _)| hit(h))
        .map(|(_, c)| c);
        match code {
            Some(code) => self.dispatch(Some(code), None, [false; 18], &mut || {}),
            None => Ok(()),
        }
    }

    /// The hovered button: kept with its start while the cursor stays on
    /// it, a new start on another, none off them.
    fn set_fe_hover(&mut self, over: Option<u32>, now: f32) {
        self.fe.fe_hover = match (over, self.fe.fe_hover) {
            (Some(b), Some((a, since))) if a == b => Some((b, since)),
            (Some(b), _) => Some((b, now)),
            (None, _) => None,
        };
    }

    /// The help bar the mouse takes: the screen's (`help_screen`), or
    /// UI_InGame_WorldMap's own (0x4B0FE0 → 0x54E6F0: 0x18400 with
    /// 0x10000000 Activate GPS / 0x20000000 Deactivate GPS), which the
    /// map draws itself.
    fn mouse_help(&self) -> Option<(u32, f32)> {
        self.help_screen().or_else(|| match &self.mode {
            Mode::Menu(Menu::InGameMap(map)) => {
                Some((screens::WORLD_MAP_HELP | if map.gps { 0x2000_0000 } else { 0x1000_0000 }, map.opened))
            }
            _ => None,
        })
    }

    /// One frame's mouse for the screen up (0x5663C0 → 0x9803F6E2 when
    /// 0x5BF950 sees activity): the help bar (`mouse_help_bar`), then the
    /// screen's own handler (one per screen, in this module's files).
    pub(crate) fn fe_mouse(&mut self) -> anyhow::Result<()> {
        let mut m: Mouse = sample_mouse(&mut self.platform.mouse, &mut self.platform.mouse_state);
        // A press and its release inside one frame (a quick tap, a
        // touchpad's tap-to-click): the frame sees the button held, as a
        // press lasting a frame would be (0x5BF870 samples the state once
        // a frame and could lose it; clone choice, invisible).
        m.held |= m.pressed;
        if !m.held {
            self.platform.mouse_state.held_fired = false;
        }
        // A drag (a scroll bar's +2) ends with the button (+0x23).
        if !m.held && fm::drag().is_some() {
            fm::set_drag(None);
        }
        let (Some(px), true) = (m.at, m.active()) else { return Ok(()) };
        if m.pressed && self.loads.loading.is_none() && matches!(&self.mode, Mode::Race(r) if r.replay.playback.is_some()) {
            return self.replay_click(px);
        }
        if !matches!(self.mode, Mode::Menu(_)) || self.loads.loading.is_some() {
            fm::set_drag(None);
            return Ok(());
        }
        let Some(frame) = self.ui_frame() else { return Ok(()) };
        let now = self.now();
        // A pure move: no click, no wheel (0x52FC50, 0x55D420, 0x4ED140).
        let hover = m.moved() && !m.pressed && m.wheel == 0.0;
        // The wheel as a step (0x50CDA0: lZ < 0 → +1, > 0 → −1).
        let wheel = if m.wheel < 0.0 {
            Some(BarHit::Step(1))
        } else if m.wheel > 0.0 {
            Some(BarHit::Step(-1))
        } else {
            None
        };
        let c = MouseCtx {
            m: &m,
            frame: &frame,
            px,
            now,
            hover,
            wheel,
            held_fired: Cell::new(self.platform.mouse_state.held_fired),
            arrow_at: Cell::new(self.platform.mouse_state.arrow_at),
            real: self.pacing.clocks.real,
        };
        let r = self.fe_mouse_screen(&c);
        self.platform.mouse_state.held_fired = c.held_fired.get();
        self.platform.mouse_state.arrow_at = c.arrow_at.get();
        r
    }

    /// `fe_mouse`'s help bar and the screen's own handler.
    fn fe_mouse_screen(&mut self, c: &MouseCtx) -> anyhow::Result<()> {
        if let Some(r) = self.mouse_help_bar(c) {
            return r;
        }
        match &self.mode {
            Mode::Menu(Menu::Icons(..)) => self.mouse_icons(c),
            Mode::Menu(Menu::Parts(_)) => self.mouse_parts(c),
            Mode::Menu(Menu::Paint(_)) => self.mouse_paint(c),
            Mode::Menu(Menu::ChooseHud(_)) => self.mouse_choose_hud(c),
            Mode::Menu(Menu::Cover(_)) => self.mouse_cover(c),
            Mode::Menu(Menu::Showcase(_)) => self.mouse_showcase(c),
            Mode::Menu(Menu::CarLot(_)) => self.mouse_car_lot(c),
            Mode::Menu(Menu::QuickRace(_)) => self.mouse_quick_race(c),
            Mode::Menu(Menu::Stats(_)) => self.mouse_stats(c),
            Mode::Menu(Menu::BrandSelect(..)) => self.mouse_brand_select(c),
            Mode::Menu(Menu::TuneSliders(_) | Menu::Options(_) | Menu::QrOptions(_)) => self.mouse_option_list(c),
            Mode::Menu(Menu::Controls(_)) => self.mouse_controls(c),
            Mode::Menu(Menu::Jukebox(_)) => self.mouse_jukebox(c),
            Mode::Menu(Menu::McList(_)) => self.mouse_mc_list(c),
            Mode::Menu(Menu::SmsMailbox(_)) => self.mouse_sms(c),
            Mode::Menu(Menu::WorldMap(_)) => self.mouse_world_map(c),
            Mode::Menu(Menu::InGameMap(_)) => self.mouse_ingame_map(c),
            Mode::Menu(Menu::Status(_)) => self.mouse_status(c),
            Mode::Menu(Menu::TuneGraph(_)) => self.mouse_tune_graph(c),
            Mode::Menu(Menu::McMain(mc)) if mc.step != screens::McStep::Name => self.mouse_mc_main(c),
            Mode::Menu(Menu::McMessage(..)) => self.mouse_mc_message(c),
            Mode::Menu(Menu::Dialog(..)) => self.mouse_dialog(c),
            _ => Ok(()),
        }
    }

    /// A scroll bar's result as the screen's steps: ±1 one `minus` /
    /// `plus`, a track click `page` of them (0x7FA1A8 = 3; 0: ignored).
    /// The steps are silent; an arrow click plays the bar's sound.
    fn bar_keys(&mut self, hit: Option<BarHit>, minus: Key, plus: Key, page: usize) -> anyhow::Result<bool> {
        let (d, n) = match hit {
            Some(BarHit::Arrow(d)) => {
                use nfsu2::audio::fe::{control, event};
                self.fe_sound(if d < 0 { event::LEFT } else { event::RIGHT }, control::GENERIC);
                (d, 1)
            }
            Some(BarHit::Step(d)) => (d, 1),
            Some(BarHit::Page(d)) if page > 0 => (d, page),
            _ => return Ok(false),
        };
        for _ in 0..n {
            self.mouse_step(if d < 0 { minus } else { plus })?;
        }
        Ok(true)
    }

    /// A screen's step without the key's sounds (the step functions the
    /// mouse calls play none of their own).
    fn mouse_step(&mut self, key: Key) -> anyhow::Result<()> {
        self.platform.mouse_state.mute = true;
        let r = self.mouse_key(key);
        self.platform.mouse_state.mute = false;
        r
    }

    /// The tuning screens' Tip (0xC519BFC3 in 0x5650D0 / 0x5654E0): sound
    /// 0x51DE70(_, 2) and a one-button dialog (0x558440) with the hint over
    // prov: inferred game.mouse.tuning_tip
    /// the screen (**inferred**: the button's label, OK).
    pub(crate) fn tuning_tip(&mut self) {
        let (param, category) = match &self.mode {
            Mode::Menu(Menu::TuneSliders(t)) => (t.selected_param(), t.category),
            Mode::Menu(Menu::TuneGraph(g)) => (None, g.category),
            _ => return,
        };
        self.fe_sound(nfsu2::audio::fe::event::SUMMON_HELP, nfsu2::audio::fe::control::GENERIC);
        let text = self.label(tuning_hint(param, category), "");
        let mut d =
            Dialog::new(screens::dialog_package(&self.game_data.assets, &text), &text, &[&self.label(0x0639, "OK")], self.now());
        d.under = Some(0);
        let here = std::mem::replace(&mut self.mode, Mode::Explore);
        self.mode = Mode::Menu(Menu::Dialog(d, vec![here]));
    }
}
