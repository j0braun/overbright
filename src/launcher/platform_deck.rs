//! The launcher on gamescope (Steam Deck Game Mode) and the input facts
//! the screens need (synthesis §2.7–2.8):
//!
//! * [`is_gamescope`] / [`is_deck`]: where the launcher runs (read once).
//! * [`window_size`]: the window's first size: today's desktop rule, the
//!   whole screen under gamescope.
//! * [`open_keyboard`]: Steam's on-screen keyboard (`steam://open/keyboard`).
//! * [`Dedup`]: one press counted once when Steam Input sends both a
//!   key (its desktop layout) and the raw pad's button.
//! * [`InputKind`], [`last_input`] / [`note_input`]: the last device used
//!   (the legends' glyphs, the cursor).
//! * [`held_at_start`]: Shift / Ctrl, or a pad's View (LB as a fallback),
//!   held while Overbright starts (the way back to the launcher).
//!
//! Nothing here was tried on a real Deck: the detection is tested with
//! fake environments, the pad's names and vendor ids are the documented
//! ones (synthesis §5.3 lists what a Deck run must confirm).

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use super::ui::Key;

// The input kinds are the shared look's (ob-look: the legends follow them).
pub use ob_look::legends::InputKind;

/// Not set yet: [`default_input`] answers.
const UNSET: u8 = u8::MAX;
static LAST: AtomicU8 = AtomicU8::new(UNSET);

/// The device used last; before any input, [`default_input`].
pub fn last_input() -> InputKind {
    match LAST.load(Ordering::Relaxed) {
        UNSET => default_input(),
        v => InputKind::from_u8(v),
    }
}

/// Records the device just used; true when it changed (redraw the legends).
pub fn note_input(kind: InputKind) -> bool {
    let old = LAST.swap(kind.to_u8(), Ordering::Relaxed);
    old == UNSET && kind != default_input() || old != UNSET && old != kind.to_u8()
}

/// The glyphs before the first input: the Deck's on a Deck, else the
/// keyboard's ([`InputKind::default`]).
pub fn default_input() -> InputKind {
    if is_deck() { InputKind::Deck } else { InputKind::default() }
}

/// The glyph set for a pad, from its name and USB vendor id (gilrs).
/// Sony's pads: PlayStation. Valve's (the Deck's own controls, Steam's
/// virtual pad): Deck on a Deck, else Xbox (Steam Input's virtual pad
/// looks like an Xbox pad). Everything else: Xbox.
pub fn pad_kind(name: &str, vendor: Option<u16>) -> InputKind {
    pad_kind_on(name, vendor, is_deck())
}

/// [`pad_kind`] with "on a Deck" given (tests).
pub fn pad_kind_on(name: &str, vendor: Option<u16>, deck: bool) -> InputKind {
    const SONY: u16 = 0x054c;
    const VALVE: u16 = 0x28de;
    let n = name.to_ascii_lowercase();
    if vendor == Some(SONY)
        || ["dualsense", "dualshock", "playstation", "ps3", "ps4", "ps5"].iter().any(|s| n.contains(s))
        || n == "wireless controller"
    {
        InputKind::PlayStation
    } else if n.contains("steam deck") || (deck && (vendor == Some(VALVE) || n.contains("steam"))) {
        InputKind::Deck
    } else if deck && vendor.is_none() && !n.contains("xbox") {
        // A Deck with a pad that tells nothing: its own controls.
        InputKind::Deck
    } else {
        InputKind::Xbox
    }
}

/// Under gamescope (Steam Deck Game Mode, or a desktop's gamescope
/// session), read once.
pub fn is_gamescope() -> bool {
    static V: OnceLock<bool> = OnceLock::new();
    *V.get_or_init(|| gamescope_in(&|k| std::env::var(k).ok()))
}

/// On a Steam Deck, read once.
pub fn is_deck() -> bool {
    static V: OnceLock<bool> = OnceLock::new();
    *V.get_or_init(|| {
        let vendor = std::fs::read_to_string("/sys/class/dmi/id/board_vendor").ok();
        deck_in(&|k| std::env::var(k).ok(), vendor.as_deref())
    })
}

/// [`is_gamescope`] for a given environment: `XDG_CURRENT_DESKTOP`
/// naming gamescope, or `GAMESCOPE_WAYLAND_DISPLAY` set.
/// `OB_GAMESCOPE=0/1` overrides (tests, and a way out if it guesses wrong).
pub fn gamescope_in(get: &dyn Fn(&str) -> Option<String>) -> bool {
    if let Some(v) = get("OB_GAMESCOPE") {
        return flag(&v);
    }
    get("XDG_CURRENT_DESKTOP").is_some_and(|d| d.split(':').any(|p| p.trim().eq_ignore_ascii_case("gamescope")))
        || get("GAMESCOPE_WAYLAND_DISPLAY").is_some_and(|d| !d.is_empty())
}

/// [`is_deck`] for a given environment and DMI board vendor: `SteamDeck=1`
/// or the vendor `Valve`. `OB_DECK=0/1` overrides.
pub fn deck_in(get: &dyn Fn(&str) -> Option<String>, board_vendor: Option<&str>) -> bool {
    if let Some(v) = get("OB_DECK") {
        return flag(&v);
    }
    get("SteamDeck").is_some_and(|v| v.trim() == "1") || board_vendor.is_some_and(|v| v.trim() == "Valve")
}

fn flag(v: &str) -> bool {
    !matches!(v.trim(), "" | "0" | "false" | "no" | "off")
}

/// The window's first size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowSize {
    /// A window on the desktop, logical pixels.
    Logical(f64, f64),
    /// The whole screen (gamescope), physical pixels; full screen.
    Screen(u32, u32),
}

/// Under gamescope the screen's size (1280 × 800 when the monitor is not
/// known: the Deck's panel); else the desktop rule
/// ([`super::window::initial_size`]).
pub fn window_size(gamescope: bool, monitor: Option<(u32, u32)>, scale: f64) -> WindowSize {
    match (gamescope, monitor) {
        (true, Some((w, h))) if w > 0 && h > 0 => WindowSize::Screen(w, h),
        (true, _) => WindowSize::Screen(1280, 800),
        (false, Some((w, h))) => {
            let (lw, lh) = super::window::initial_size(w, h, scale);
            WindowSize::Logical(lw, lh)
        }
        (false, None) => WindowSize::Logical(960.0, 720.0),
    }
}

/// The URL that shows Steam's on-screen keyboard.
pub const KEYBOARD_URL: &str = "steam://open/keyboard";

/// The program and arguments that open [`KEYBOARD_URL`]: `steam` when
/// found on PATH (it hands the URL to the running client), else `xdg-open`.
pub fn keyboard_command(steam_on_path: bool) -> (&'static str, [&'static str; 1]) {
    (if steam_on_path { "steam" } else { "xdg-open" }, [KEYBOARD_URL])
}

/// Shows Steam's on-screen keyboard (gamescope: text typed there arrives
/// as key presses in the focused window). A failure is logged. Unverified
/// on a Deck.
pub fn open_keyboard() {
    if let Err(e) = try_open_keyboard() {
        log::warn!("keyboard: {e}");
    }
}

/// [`open_keyboard`] with its error.
pub fn try_open_keyboard() -> Result<(), String> {
    let steam = std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("steam").is_file()));
    let (prog, args) = keyboard_command(steam);
    log::info!("keyboard: {prog} {}", args[0]);
    std::process::Command::new(prog)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{prog}: {e}"))
}

/// Where a key press came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Keyboard,
    Pad,
}

/// Counts a press once when the same key arrives from both the keyboard
/// and a pad within [`Dedup::WINDOW`] (Steam's desktop layout turns A
/// into Enter while the raw pad is read too). Presses from one source are
/// never dropped.
#[derive(Clone, Debug, Default)]
pub struct Dedup {
    last: Vec<(Key, Source, Instant)>,
}

impl Dedup {
    /// A frame or two at the slowest poll (16 ms) plus the delivery's jitter.
    pub const WINDOW: Duration = Duration::from_millis(60);

    /// Whether this press counts.
    pub fn accept(&mut self, key: Key, src: Source, now: Instant) -> bool {
        self.last.retain(|(_, _, t)| now.saturating_duration_since(*t) <= Self::WINDOW);
        if self.last.iter().any(|&(k, s, _)| k == key && s != src) {
            // Taken by the other source: this one is its echo; once only.
            self.last.retain(|&(k, s, _)| !(k == key && s != src));
            return false;
        }
        self.last.push((key, src, now));
        true
    }
}

/// Whether the player holds Shift or Ctrl, or a pad's View (Select; LB as
/// a fallback where Steam takes View) while Overbright starts.
/// `OB_HELD_AT_START=0/1` answers instead (the start tests).
pub fn held_at_start() -> bool {
    if let Ok(v) = std::env::var("OB_HELD_AT_START") {
        return flag(&v);
    }
    held_at_start_with(keys_held, pad_view_held)
}

/// [`held_at_start`] with its two checks given (the keyboard's first: no
/// pad poll when a key already says so).
pub fn held_at_start_with(keys: impl FnOnce() -> bool, pad: impl FnOnce() -> bool) -> bool {
    keys() || pad()
}

/// Shift or Ctrl held now. Windows: GetAsyncKeyState. X11 / XWayland: the
/// server's key map. Wayland alone and gamescope give no keyboard state
/// without a window: false there.
pub fn keys_held() -> bool {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetAsyncKeyState(key: i32) -> i16;
        }
        const VK_SHIFT: i32 = 0x10;
        const VK_CONTROL: i32 = 0x11;
        // SAFETY: a plain documented call.
        [VK_SHIFT, VK_CONTROL].iter().any(|&k| (unsafe { GetAsyncKeyState(k) } as u16 & 0x8000) != 0)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        x11_keys_held()
    }
    #[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
    false
}

#[cfg(all(unix, not(target_os = "macos")))]
fn x11_keys_held() -> bool {
    use x11_dl::keysym::{XK_Control_L, XK_Control_R, XK_Shift_L, XK_Shift_R};
    if std::env::var_os("DISPLAY").is_none_or(|d| d.is_empty()) {
        return false;
    }
    let Ok(x) = x11_dl::xlib::Xlib::open() else { return false };
    // SAFETY: Xlib calls on a display this function opens and closes.
    unsafe {
        let d = (x.XOpenDisplay)(std::ptr::null());
        if d.is_null() {
            return false;
        }
        let mut keys = [0 as std::os::raw::c_char; 32];
        (x.XQueryKeymap)(d, keys.as_mut_ptr());
        let mut held = false;
        for sym in [XK_Shift_L, XK_Shift_R, XK_Control_L, XK_Control_R] {
            let kc = (x.XKeysymToKeycode)(d, sym as std::os::raw::c_ulong) as usize;
            if kc != 0 && kc < 256 && (keys[kc / 8] as u8) & (1 << (kc % 8)) != 0 {
                held = true;
            }
        }
        (x.XCloseDisplay)(d);
        held
    }
}

/// A pad's View (Select) or LB held now (gilrs; pads report their state
/// after a short poll). `NFSU2_NO_PADS` set: false.
pub fn pad_view_held() -> bool {
    if std::env::var_os("NFSU2_NO_PADS").is_some() {
        return false;
    }
    let Ok(mut g) = gilrs::Gilrs::new() else { return false };
    let until = Instant::now() + Duration::from_millis(60);
    while Instant::now() < until {
        while g.next_event().is_some() {}
        std::thread::sleep(Duration::from_millis(10));
    }
    // A pad that reports more buttons held than a hand presses is not read
    // correctly (`pad_filter::MAX_HELD`): it holds nothing.
    g.gamepads().any(|(_, p)| {
        p.state().buttons().filter(|(_, d)| d.is_pressed()).count() <= super::pad_filter::MAX_HELD
            && pad_says_held(&|b| p.is_pressed(b))
    })
}

/// The buttons that count as "held at start".
pub fn pad_says_held(pressed: &dyn Fn(gilrs::Button) -> bool) -> bool {
    pressed(gilrs::Button::Select) || pressed(gilrs::Button::LeftTrigger)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn gamescope_from_the_environment() {
        assert!(!gamescope_in(&env(&[])));
        assert!(!gamescope_in(&env(&[("XDG_CURRENT_DESKTOP", "KDE")])));
        assert!(gamescope_in(&env(&[("XDG_CURRENT_DESKTOP", "gamescope")])));
        assert!(gamescope_in(&env(&[("XDG_CURRENT_DESKTOP", "foo:Gamescope")])));
        assert!(gamescope_in(&env(&[("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")])));
        assert!(!gamescope_in(&env(&[("GAMESCOPE_WAYLAND_DISPLAY", "")])));
        assert!(!gamescope_in(&env(&[("XDG_CURRENT_DESKTOP", "gamescope"), ("OB_GAMESCOPE", "0")])));
        assert!(gamescope_in(&env(&[("OB_GAMESCOPE", "1")])));
    }

    #[test]
    fn deck_from_the_environment_and_dmi() {
        assert!(!deck_in(&env(&[]), None));
        assert!(!deck_in(&env(&[]), Some("LENOVO\n")));
        assert!(deck_in(&env(&[]), Some("Valve\n")));
        assert!(deck_in(&env(&[("SteamDeck", "1")]), None));
        assert!(!deck_in(&env(&[("SteamDeck", "0")]), None));
        assert!(!deck_in(&env(&[("OB_DECK", "0")]), Some("Valve")));
        assert!(deck_in(&env(&[("OB_DECK", "1")]), None));
    }

    #[test]
    fn pad_kinds() {
        assert_eq!(pad_kind_on("Xbox Wireless Controller", Some(0x045e), false), InputKind::Xbox);
        assert_eq!(pad_kind_on("DualSense Wireless Controller", Some(0x054c), false), InputKind::PlayStation);
        assert_eq!(pad_kind_on("Wireless Controller", None, false), InputKind::PlayStation);
        assert_eq!(pad_kind_on("PS4 Controller", None, true), InputKind::PlayStation);
        assert_eq!(pad_kind_on("Steam Deck", Some(0x28de), false), InputKind::Deck);
        assert_eq!(pad_kind_on("Steam Virtual Gamepad", Some(0x28de), true), InputKind::Deck);
        assert_eq!(pad_kind_on("Steam Virtual Gamepad", Some(0x28de), false), InputKind::Xbox);
        assert_eq!(pad_kind_on("Microsoft X-Box 360 pad", None, true), InputKind::Deck);
        assert_eq!(pad_kind_on("Xbox Wireless Controller", None, true), InputKind::Xbox);
        assert_eq!(pad_kind_on("8BitDo Pro 2", Some(0x2dc8), false), InputKind::Xbox);
    }

    #[test]
    fn window_size_rule() {
        assert_eq!(window_size(true, Some((1280, 800)), 1.0), WindowSize::Screen(1280, 800));
        assert_eq!(window_size(true, Some((1920, 1080)), 1.5), WindowSize::Screen(1920, 1080));
        assert_eq!(window_size(true, None, 1.0), WindowSize::Screen(1280, 800));
        assert_eq!(window_size(true, Some((0, 0)), 1.0), WindowSize::Screen(1280, 800));
        // The desktop: today's rule, unchanged.
        assert_eq!(window_size(false, Some((1920, 1080)), 1.0), WindowSize::Logical(960.0, 720.0));
        assert_eq!(
            window_size(false, Some((1366, 768)), 1.0),
            WindowSize::Logical(super::super::window::initial_size(1366, 768, 1.0).0, 652.0)
        );
        assert_eq!(window_size(false, None, 1.0), WindowSize::Logical(960.0, 720.0));
    }

    #[test]
    fn keyboard_url() {
        assert_eq!(keyboard_command(true), ("steam", ["steam://open/keyboard"]));
        assert_eq!(keyboard_command(false), ("xdg-open", ["steam://open/keyboard"]));
    }

    #[test]
    fn a_press_from_two_devices_counts_once() {
        let t = Instant::now();
        let ms = |n| t + Duration::from_millis(n);
        let mut f = Dedup::default();
        // Steam's desktop layout: A and Enter in the same frame.
        assert!(f.accept(Key::Enter, Source::Pad, ms(0)));
        assert!(!f.accept(Key::Enter, Source::Keyboard, ms(5)));
        // The other order.
        assert!(f.accept(Key::Escape, Source::Keyboard, ms(100)));
        assert!(!f.accept(Key::Escape, Source::Pad, ms(110)));
        // Two quick presses on one device both count.
        assert!(f.accept(Key::Down, Source::Pad, ms(200)));
        assert!(f.accept(Key::Down, Source::Pad, ms(210)));
        // Different keys both count.
        assert!(f.accept(Key::Up, Source::Keyboard, ms(300)));
        assert!(f.accept(Key::Enter, Source::Pad, ms(301)));
        // Outside the window: a real second press.
        assert!(f.accept(Key::Tab, Source::Pad, ms(400)));
        assert!(f.accept(Key::Tab, Source::Keyboard, ms(400 + 61)));
        // An echo is dropped once; the next press from that device counts.
        assert!(f.accept(Key::Left, Source::Pad, ms(600)));
        assert!(!f.accept(Key::Left, Source::Keyboard, ms(601)));
        assert!(f.accept(Key::Left, Source::Keyboard, ms(602)));
    }

    #[test]
    fn last_input_tracks_and_says_when_it_changed() {
        note_input(InputKind::Keyboard);
        assert_eq!(last_input(), InputKind::Keyboard);
        assert!(!note_input(InputKind::Keyboard));
        assert!(note_input(InputKind::PlayStation));
        assert_eq!(last_input(), InputKind::PlayStation);
        assert!(last_input().is_pad());
        assert!(note_input(InputKind::Mouse));
        assert!(!InputKind::Mouse.is_pad());
        for k in [InputKind::Mouse, InputKind::Keyboard, InputKind::Xbox, InputKind::PlayStation, InputKind::Deck] {
            assert_eq!(InputKind::from_u8(k.to_u8()), k);
        }
    }

    #[test]
    fn held_at_start_checks() {
        assert!(held_at_start_with(|| true, || panic!("no pad poll when a key is held")));
        assert!(held_at_start_with(|| false, || true));
        assert!(!held_at_start_with(|| false, || false));
        assert!(pad_says_held(&|b| b == gilrs::Button::Select));
        assert!(pad_says_held(&|b| b == gilrs::Button::LeftTrigger));
        assert!(!pad_says_held(&|b| b == gilrs::Button::South));
    }
}
