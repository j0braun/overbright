//! The host side of [`super`]: modern pads read through gilrs (hot-plug
//! included) or a synthetic script, each presented to the native model as
//! the DirectInput device Windows would show the game.
//!
//! **Inferred** (Windows' drivers, not SPEED2.EXE): what a pad looks like
//! to DirectInput 8. An Xbox pad (vendor 0x045E) through the XInput HID
//! driver: lX / lY the left stick, lZ the two triggers on one axis (left
//! +, right −), lRx / lRy the right stick, buttons A B X Y LB RB Back
//! Start LS RS, the d-pad as POV 0, no force feedback. A PlayStation pad
//! (vendor 0x054C) through the HID driver: lX / lY, lZ / lRz the right
//! stick, lRx / lRy L2 / R2, buttons Square Cross Circle Triangle L1 R1 L2
//! R2 Share Options L3 R3 PS, POV 0, no force feedback. Any other pad as a
//! generic four-axis twelve-button pad of the time (the Logitech Dual
//! Action's layout: lX / lY, lZ / lRz, buttons West South East North L1 R1
//! L2 R2 Select Start L3 R3). Sticks read downwards positive (DirectInput's
//! lY), ±1 → ±10000.

use super::extras::PadOptions;
use super::{Caps, DevType, DiState, FeRepeat, PadMap, Report, CHANNELS};

/// A standard pad's inputs (gilrs's layout).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadInputs {
    /// Left x, left y (up +), right x, right y (up +), −1..1.
    pub sticks: [f32; 4],
    /// Left / right analog triggers, 0..1.
    pub triggers: [f32; 2],
    /// The raw Z and Rz axes, −1..1 (a wheel's pedals; gilrs's LeftZ /
    /// RightZ).
    pub z: [f32; 2],
    /// By [`Btn`].
    pub buttons: [bool; 17],
}

/// The standard buttons, in [`PadInputs::buttons`] order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Btn {
    South,
    East,
    West,
    North,
    L1,
    R1,
    L2,
    R2,
    Select,
    Start,
    Mode,
    L3,
    R3,
    Up,
    Down,
    Left,
    Right,
}

impl Btn {
    pub const ALL: [Btn; 17] = [Btn::South, Btn::East, Btn::West, Btn::North, Btn::L1, Btn::R1, Btn::L2, Btn::R2, Btn::Select, Btn::Start, Btn::Mode, Btn::L3, Btn::R3, Btn::Up, Btn::Down, Btn::Left, Btn::Right];

    pub fn parse(s: &str) -> Option<Btn> {
        Some(match s.to_ascii_lowercase().as_str() {
            "south" | "a" | "cross" => Btn::South,
            "east" | "b" | "circle" => Btn::East,
            "west" | "x" | "square" => Btn::West,
            "north" | "y" | "triangle" => Btn::North,
            "l1" | "lb" => Btn::L1,
            "r1" | "rb" => Btn::R1,
            "l2" | "lt" => Btn::L2,
            "r2" | "rt" => Btn::R2,
            "select" | "back" | "share" => Btn::Select,
            "start" | "options" => Btn::Start,
            "mode" | "guide" | "ps" => Btn::Mode,
            "l3" | "ls" => Btn::L3,
            "r3" | "rs" => Btn::R3,
            "up" => Btn::Up,
            "down" => Btn::Down,
            "left" => Btn::Left,
            "right" => Btn::Right,
            _ => return None,
        })
    }
}

/// How a pad shows itself to DirectInput (**inferred**, see the module).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Xbox,
    Sony,
    Generic,
    /// Clone extra (Controller Layout Modern): any standard pad as
    /// buttons South East West North L1 R1 Select Start L3 R3 Mode, lX / lY
    /// the left stick, lRx / lRy the right, lZ the left trigger and lRz the
    /// right (−10000 released, 10000 full), POV 0 the d-pad.
    Modern,
    /// A racing wheel ([`is_wheel`]), presented as DirectInput gives a
    /// DI8DEVTYPE_DRIVING device: lX the wheel, lY lZ lRx lRy lRz raw
    /// (gilrs's left y, LeftZ, right x / y, RightZ), the generic buttons.
    Wheel,
}

/// A pad's family, for the button names the UI shows (Modern layout).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Xbox,
    PlayStation,
    Nintendo,
    Other,
}

impl Family {
    pub fn for_vendor(vid: u16) -> Family {
        match vid {
            0x045E => Family::Xbox,
            0x054C => Family::PlayStation,
            0x057E => Family::Nintendo,
            // Steam Input's virtual pads and the Deck are Xbox-labelled.
            0x28DE => Family::Xbox,
            _ => Family::Other,
        }
    }

    /// The face button names, South East West North, then Start.
    pub fn names(self) -> [&'static str; 5] {
        match self {
            Family::Xbox | Family::Other => ["A", "B", "X", "Y", "Start"],
            Family::PlayStation => ["Cross", "Circle", "Square", "Triangle", "Options"],
            // Nintendo's labels sit on the other positions: East is A.
            Family::Nintendo => ["B", "A", "Y", "X", "+"],
        }
    }
}

impl Layout {
    /// The native layout of a product: a wheel, or by vendor.
    pub fn native(vid: u16, pid: u16) -> Layout {
        if is_wheel(vid, pid) { Layout::Wheel } else { Layout::for_vendor(vid) }
    }

    pub fn for_vendor(vid: u16) -> Layout {
        match vid {
            0x045E => Layout::Xbox,
            0x054C => Layout::Sony,
            _ => Layout::Generic,
        }
    }

    fn buttons(self) -> &'static [Btn] {
        use Btn::*;
        match self {
            Layout::Xbox => &[South, East, West, North, L1, R1, Select, Start, L3, R3],
            Layout::Sony => &[West, South, East, North, L1, R1, L2, R2, Select, Start, L3, R3, Mode],
            Layout::Generic | Layout::Wheel => &[West, South, East, North, L1, R1, L2, R2, Select, Start, L3, R3],
            Layout::Modern => &[South, East, West, North, L1, R1, Select, Start, L3, R3, Mode],
        }
    }

    pub fn caps(self, vid: u16, pid: u16) -> Caps {
        let (axes, buttons, objects) = match self {
            // X Y Z Rx Ry.
            Layout::Xbox => (5, 10, [true, true, true, true, true, false, false, false]),
            // X Y Z Rx Ry Rz (14 buttons: the touch pad's is never pressed here).
            Layout::Sony => (6, 14, [true, true, true, true, true, true, false, false]),
            // X Y Z Rz.
            Layout::Generic => (4, 12, [true, true, true, false, false, true, false, false]),
            // X Y Z Rx Ry Rz.
            Layout::Modern => (6, 11, [true, true, true, true, true, true, false, false]),
            // X Y Z Rx Ry Rz.
            Layout::Wheel => (6, 12, [true, true, true, true, true, true, false, false]),
        };
        let reported = if self == Layout::Wheel { DevType::Driving } else { DevType::Gamepad };
        Caps { dev_type: super::dev_type(vid, pid, reported), axes, buttons, povs: 1, ff: false, vid, pid, objects }
    }

    /// The DIJOYSTATE2 the pad's inputs give.
    pub fn state(self, p: &PadInputs) -> DiState {
        let r = |v: f32| (v.clamp(-1.0, 1.0) * 10000.0).round() as i32;
        let [lx, ly, rx, ry] = p.sticks;
        let [lt, rt] = p.triggers;
        let mut s = DiState::default();
        s.axes[0] = r(lx);
        s.axes[1] = r(-ly);
        match self {
            Layout::Xbox => {
                s.axes[2] = r(lt - rt);
                s.axes[3] = r(rx);
                s.axes[4] = r(-ry);
            }
            Layout::Sony => {
                s.axes[2] = r(rx);
                s.axes[5] = r(-ry);
                s.axes[3] = r(lt * 2.0 - 1.0);
                s.axes[4] = r(rt * 2.0 - 1.0);
            }
            Layout::Generic => {
                s.axes[2] = r(rx);
                s.axes[5] = r(-ry);
            }
            Layout::Wheel => {
                s.axes[2] = r(p.z[0]);
                s.axes[3] = r(rx);
                s.axes[4] = r(-ry);
                s.axes[5] = r(p.z[1]);
            }
            Layout::Modern => {
                s.axes[2] = r(lt * 2.0 - 1.0);
                s.axes[3] = r(rx);
                s.axes[4] = r(-ry);
                s.axes[5] = r(rt * 2.0 - 1.0);
            }
        }
        let held = |b: Btn| p.buttons[b as usize];
        for (i, &b) in self.buttons().iter().enumerate() {
            s.buttons[i] = held(b);
        }
        // The d-pad as POV 0 (hundredths of a degree from north, clockwise).
        let (x, y) = (held(Btn::Right) as i32 - held(Btn::Left) as i32, held(Btn::Up) as i32 - held(Btn::Down) as i32);
        s.pov[0] = match (x, y) {
            (0, 1) => 0,
            (1, 1) => 4500,
            (1, 0) => 9000,
            (1, -1) => 13500,
            (0, -1) => 18000,
            (-1, -1) => 22500,
            (-1, 0) => 27000,
            (-1, 1) => 31500,
            _ => u32::MAX,
        };
        s
    }
}

/// Where a device's inputs come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Gilrs(gilrs::GamepadId),
    Script(u32),
}

/// An attached controller: a DirectInput channel and its input record.
#[derive(Clone, Debug)]
pub struct Device {
    pub source: Source,
    /// The record's GUID in the profile block (the clone: gilrs's device
    /// uuid; the original stores the DirectInput guidInstance).
    pub guid: [u8; 16],
    pub name: String,
    pub layout: Layout,
    pub caps: Caps,
    pub map: PadMap,
    pub inputs: PadInputs,
    pub state: DiState,
    pub prev: DiState,
    pub report: Report,
    /// The report of the frame before (for edges).
    pub last: Report,
    /// The FE stick rows' repeat (0x571D60).
    pub repeat: FeRepeat,
    /// The mapping database knows it as a standard gamepad (Controller
    /// Layout Auto's test, [`standard_pad`]).
    pub standard: bool,
    /// The state 0x5CA730 last saw (a Controls capture's button edges).
    pub capture_prev: DiState,
    pub family: Family,
}

/// A Controls cell's text under the Modern layout: the pad's own names
/// (its family's face buttons) instead of DirectInput's object names.
pub fn modern_binding_name(b: super::Binding, family: Family) -> String {
    use super::{Binding, axis};
    let [s, e, w, n, start] = family.names();
    match b {
        Binding::None => String::new(),
        Binding::Axis(o) => match o {
            axis::X => "Left Stick",
            axis::Y => "Left Stick Y",
            axis::Z => "Left Trigger",
            axis::RX => "Right Stick X",
            axis::RY => "Right Stick Y",
            axis::RZ => "Right Trigger",
            _ => "Slider",
        }
        .into(),
        Binding::Button(i) => {
            let shoulder = if family == Family::PlayStation { ["L1", "R1", "Share", start, "L3", "R3", "PS"] } else { ["LB", "RB", "Back", start, "LS", "RS", "Guide"] };
            match i {
                0 => s.into(),
                1 => e.into(),
                2 => w.into(),
                3 => n.into(),
                4..=10 => shoulder[usize::from(i) - 4].into(),
                _ => format!("Button {}", i + 1),
            }
        }
        Binding::Pov(_) => "D-Pad".into(),
    }
}

/// A legacy DirectInput-style device for Controller Layout Auto: one the
/// original has a preset for (0x7FE7F8) or a type override (0x800588: a
/// wheel, the Saitek pad), which stay Original.
/// The racing wheels SDL 2 lists (`initial_wheel_devices`,
/// THIRD-PARTY-NOTICES.md): the devices whose vendor drivers report
/// DI8DEVTYPE_DRIVING under DirectInput, which the original reads from the
/// device instance (0x5BFB70; **inferred**: the reported type stands in for
/// what each driver says).
pub const WHEELS: &[(u16, u16)] = &[
    (0x0079, 0x1864),
    (0x046D, 0xC294), (0x046D, 0xC295), (0x046D, 0xC298), (0x046D, 0xC299), (0x046D, 0xC29A), (0x046D, 0xC29B),
    (0x046D, 0xC24F), (0x046D, 0xC260), (0x046D, 0xC261), (0x046D, 0xC262), (0x046D, 0xC268), (0x046D, 0xC269),
    (0x046D, 0xC272), (0x046D, 0xC26D), (0x046D, 0xC26E), (0x046D, 0xC266), (0x046D, 0xC267), (0x046D, 0xCA03),
    (0x044F, 0xB65D), (0x044F, 0xB66D), (0x044F, 0xB677), (0x044F, 0xB696), (0x044F, 0xB66E), (0x044F, 0xB66F),
    (0x044F, 0xB65E), (0x044F, 0xB664), (0x044F, 0xB669), (0x044F, 0xB67F), (0x044F, 0xB691), (0x044F, 0xB692),
    (0x0483, 0x0522), (0x0483, 0xA355),
    (0x0EB7, 0x0001), (0x0EB7, 0x0004), (0x0EB7, 0x0005), (0x0EB7, 0x0006), (0x0EB7, 0x0007), (0x0EB7, 0x0011),
    (0x0EB7, 0x0020), (0x0EB7, 0x0197), (0x0EB7, 0x038E), (0x0EB7, 0x0E03),
    (0x11FF, 0x0511), (0x1209, 0xFFB0),
    (0x16D0, 0x0D5A), (0x16D0, 0x0D5F), (0x16D0, 0x0D60), (0x16D0, 0x0D61),
    (0x2433, 0xF300), (0x2433, 0xF301), (0x2433, 0xF303), (0x2433, 0xF306),
    (0x3416, 0x0301), (0x3416, 0x0302),
    (0x346E, 0x0000), (0x346E, 0x0002), (0x346E, 0x0004), (0x346E, 0x0005), (0x346E, 0x0006),
];

/// A product presented as a wheel: [`WHEELS`] or the original's own
/// override (0x800588: 044F:04B2).
pub fn is_wheel(vid: u16, pid: u16) -> bool {
    WHEELS.contains(&(vid, pid)) || super::dev_type(vid, pid, DevType::Gamepad) == DevType::Driving
}

pub fn legacy_product(vid: u16, pid: u16) -> bool {
    super::PRESETS.iter().any(|(p, ..)| *p == (vid, pid)) || super::TYPE_OVERRIDES.iter().any(|(p, _)| *p == (vid, pid)) || is_wheel(vid, pid)
}

/// gilrs's view of a standard gamepad (**clone heuristic**): its mapping
/// (SDL_GameControllerDB or a kernel driver with the evdev gamepad codes)
/// gives the South and East face buttons and the left stick, and it is not
/// a legacy product.
fn standard_pad(p: &gilrs::Gamepad<'_>, vid: u16, pid: u16) -> bool {
    use gilrs::{Axis, Button};
    p.button_code(Button::South).is_some() && p.button_code(Button::East).is_some() && p.axis_code(Axis::LeftStickX).is_some() && !legacy_product(vid, pid)
}

/// The profile GUID of a pad under the Modern layout: the device's own
/// with its last byte flipped, so that its map and the native one are kept
/// apart (a block applies only to the layout that wrote it).
pub fn modern_guid(mut g: [u8; 16]) -> [u8; 16] {
    g[15] ^= 0x4D;
    g
}

impl Device {
    /// A pad as the native layouts present it (`modern` false), or under
    /// the Controller Layout Modern extra.
    pub fn new(source: Source, guid: [u8; 16], name: String, vid: u16, pid: u16, modern: bool) -> Device {
        let layout = if modern { Layout::Modern } else { Layout::native(vid, pid) };
        let caps = layout.caps(vid, pid);
        let map = if modern { PadMap::modern(&caps) } else { PadMap::attach(&caps) };
        let guid = if modern { modern_guid(guid) } else { guid };
        Device { source, guid, name, layout, caps, map, inputs: PadInputs::default(), state: DiState::default(), prev: DiState::default(), report: Report::default(), last: Report::default(), repeat: FeRepeat::default(), standard: false, capture_prev: DiState::default(), family: Family::for_vendor(vid) }
    }

    /// Restore Defaults (0x4ACD90 → 0x5C11B0; the Modern layout's own).
    pub fn restore_defaults(&mut self) {
        if self.layout == Layout::Modern {
            self.map = PadMap::modern(&self.caps);
        } else {
            let caps = self.caps.clone();
            self.map.defaults(&caps);
        }
    }
}

/// A synthetic pad script (`NFSU2_PAD_SCRIPT`): lines of
/// `<seconds> <command> …`, the time on the game's clock:
/// `connect <id> [vid] [pid]`, `disconnect <id>`, `axis <id> lx|ly|rx|ry|lt|rt <value>`
/// (y up positive), `button <id> <name> 0|1`.
#[derive(Clone, Debug, Default)]
pub struct Script {
    lines: Vec<(f32, Vec<String>)>,
    next: usize,
}

impl Script {
    pub fn parse(text: &str) -> Script {
        let mut lines: Vec<(f32, Vec<String>)> = text
            .lines()
            .map(|l| l.split('#').next().unwrap_or(""))
            .filter_map(|l| {
                let mut w = l.split_whitespace();
                let t = w.next()?.parse().ok()?;
                Some((t, w.map(String::from).collect()))
            })
            .collect();
        lines.sort_by(|a, b| a.0.total_cmp(&b.0));
        Script { lines, next: 0 }
    }
}

/// What changed on a poll.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub attached: bool,
    pub detached: bool,
    /// The device that drove the car (the first, [`Host::analog`]) went.
    pub driver_lost: bool,
}

/// The controllers' frame time watch: a frame over [`FrameWatch::SLOW`] is
/// counted, and a log line says so at most every [`FrameWatch::EVERY`]
/// (the support file then shows a controller frame that costs the game its
/// frame rate, as a per-frame device lookup once did).
#[derive(Debug, Default)]
pub struct FrameWatch {
    slow: u32,
    longest: std::time::Duration,
    last: Option<std::time::Instant>,
}

impl FrameWatch {
    pub const SLOW: std::time::Duration = std::time::Duration::from_millis(4);
    pub const EVERY: std::time::Duration = std::time::Duration::from_secs(10);

    pub fn note(
        &mut self,
        took: std::time::Duration,
        now: std::time::Instant,
        lookups: u32,
        lookup_time: std::time::Duration,
    ) -> Option<String> {
        if took < Self::SLOW {
            return None;
        }
        self.slow += 1;
        self.longest = self.longest.max(took);
        if self.last.is_some_and(|t| now.duration_since(t) < Self::EVERY) {
            return None;
        }
        let line = format!(
            "pads: the controllers' frame took {:.1} ms ({} slow frames over {} ms, longest {:.1} ms); force-feedback device lookups {lookups} in {:.0} ms",
            took.as_secs_f64() * 1e3,
            self.slow,
            Self::SLOW.as_millis(),
            self.longest.as_secs_f64() * 1e3,
            lookup_time.as_secs_f64() * 1e3
        );
        self.last = Some(now);
        self.slow = 0;
        self.longest = std::time::Duration::ZERO;
        Some(line)
    }
}

/// The game's controllers.
/// The driving channel's force feedback (0x870760's per-channel state and
/// the device behind it).
#[derive(Debug)]
pub struct Ff {
    pub source: Source,
    pub out: super::ff_out::Output,
    pub model: super::ff::Original,
    /// The player's input object is up (driving): the gain was set at its
    /// start (0x41E28D).
    driving: bool,
}

pub struct Host {
    gilrs: Option<gilrs::Gilrs>,
    script: Option<Script>,
    /// At most [`CHANNELS`] (0x86FFB4[2]), in attach order.
    pub devices: Vec<Device>,
    /// The Controller extras in force ([`Host::set_options`]).
    options: PadOptions,
    /// The 60 Hz tick of the last poll (0x865134).
    pub tick: i64,
    /// The poll advanced the tick (a 60 Hz frame of the original's input).
    pub ticked: bool,
    /// The scripted pads' (strong, weak) rumble as last set (tests).
    pub rumble_out: Vec<(f32, f32)>,
    /// gilrs rumble effects per device (strong, weak).
    effects: Vec<(gilrs::GamepadId, gilrs::ff::Effect, gilrs::ff::Effect)>,
    rumble_last: (f32, f32),
    /// The force-feedback device on the driving channel and the original's
    /// effect model for it (None: no device with FF actuators).
    ff: Option<Ff>,
    /// The driving channel's device the force-feedback output was last
    /// looked up for, found or not ([`Host::ff_sync`]): the lookup runs
    /// once per device that takes the channel, never per frame.
    ff_looked: Option<Source>,
    /// Force-feedback device lookups made and their time (the log's
    /// controller lines, [`FrameWatch`]).
    pub ff_lookups: u32,
    pub ff_lookup_time: std::time::Duration,
    /// The controllers' frame time watch ([`Host::frame_took`]).
    watch: FrameWatch,
    /// Scripted pads connected with `ff` (a recorder takes their effects).
    script_ff: Vec<u32>,
    /// The profile's controller blocks ([`Host::set_saved_blocks`]).
    saved: Vec<[u8; super::BLOCK_LEN]>,
    /// [`Host::both_sticks`]: both sticks were seen released on the
    /// current screen.
    sticks_armed: bool,
}

impl Host {
    /// gilrs (unless `NFSU2_NO_PADS` is set or `real` is false) and the
    /// script of `NFSU2_PAD_SCRIPT`, if any.
    pub fn new(real: bool) -> Host {
        let gilrs = if real && std::env::var_os("NFSU2_NO_PADS").is_none() {
            // No filters: the game sees the raw values (DirectInput had no
            // dead zone set, 0x5C8670 sets DIPROP_RANGE only).
            gilrs::GilrsBuilder::new().with_default_filters(false).build().map_err(|e| eprintln!("gamepads: {e}")).ok()
        } else {
            None
        };
        let script = std::env::var_os("NFSU2_PAD_SCRIPT").and_then(|p| std::fs::read_to_string(&p).map_err(|e| eprintln!("{}: {e}", p.to_string_lossy())).ok()).map(|t| Script::parse(&t));
        let mut h = Host { gilrs, script, devices: Vec::new(), options: PadOptions::default(), tick: i64::MIN, ticked: false, rumble_out: Vec::new(), effects: Vec::new(), rumble_last: (0.0, 0.0), ff: None, ff_looked: None, ff_lookups: 0, ff_lookup_time: std::time::Duration::ZERO, watch: FrameWatch::default(), script_ff: Vec::new(), saved: Vec::new(), sticks_armed: false };
        h.rescan();
        h
    }

    /// A host with the given script and no real pads (tests).
    pub fn scripted(text: &str) -> Host {
        Host { gilrs: None, script: Some(Script::parse(text)), devices: Vec::new(), options: PadOptions::default(), tick: i64::MIN, ticked: false, rumble_out: Vec::new(), effects: Vec::new(), rumble_last: (0.0, 0.0), ff: None, ff_looked: None, ff_lookups: 0, ff_lookup_time: std::time::Duration::ZERO, watch: FrameWatch::default(), script_ff: Vec::new(), saved: Vec::new(), sticks_armed: false }
    }

    /// The Controller extras in force. A layout change re-presents every
    /// attached pad (its map from the profile block of that layout, which
    /// the caller applies on `attached`); true when it did.
    pub fn set_options(&mut self, o: PadOptions) -> bool {
        self.options = o;
        self.represent(|_, _| true)
    }

    /// The profile's controller blocks (as `apply_pad_maps` gives them to
    /// the pads). A block with custom Original bindings for a pad Auto made
    /// Modern turns it back to Original (one way, so restoring its defaults
    /// does not flip it mid-session); true when a pad changed.
    pub fn set_saved_blocks(&mut self, blocks: &[[u8; super::BLOCK_LEN]]) -> bool {
        if self.saved.as_slice() == blocks {
            return false;
        }
        self.saved = blocks.to_vec();
        self.represent(|modern_now, modern| modern_now && !modern)
    }

    /// The pad's own GUID (its native profile block's).
    fn native_guid(d: &Device) -> [u8; 16] {
        if d.layout == Layout::Modern { modern_guid(d.guid) } else { d.guid }
    }

    /// The profile holds Original bindings for this pad that are not its
    /// defaults.
    fn native_custom(&self, guid: &[u8; 16], vid: u16, pid: u16) -> bool {
        self.saved.iter().any(|b| b[..16] == guid[..] && {
            let caps = Layout::native(vid, pid).caps(vid, pid);
            let mut a = PadMap::attach(&caps);
            let defaults = a.to_block(guid);
            a.apply_block(b);
            a.to_block(guid) != defaults
        })
    }

    /// Whether a pad is presented as Modern under the options in force.
    fn wants_modern(&self, standard: bool, guid: &[u8; 16], vid: u16, pid: u16) -> bool {
        self.options.modern_for(standard, self.options.layout == super::extras::LAYOUT_AUTO && self.native_custom(guid, vid, pid))
    }

    /// A new device under the options in force.
    fn present(&self, source: Source, guid: [u8; 16], name: String, vid: u16, pid: u16, standard: bool) -> Device {
        let modern = self.wants_modern(standard, &guid, vid, pid);
        Device { standard, ..Device::new(source, guid, name, vid, pid, modern) }
    }

    /// Re-presents the pads whose layout the options now resolve
    /// differently, when `allow(is modern, would be modern)`.
    fn represent(&mut self, allow: impl Fn(bool, bool) -> bool) -> bool {
        let mut changed = false;
        for i in 0..self.devices.len() {
            let d = &self.devices[i];
            let guid = Self::native_guid(d);
            let (now, want) = (d.layout == Layout::Modern, self.wants_modern(d.standard, &guid, d.caps.vid, d.caps.pid));
            if now != want && allow(now, want) {
                let n = self.present(d.source, guid, d.name.clone(), d.caps.vid, d.caps.pid, d.standard);
                let inputs = d.inputs;
                self.devices[i] = Device { inputs, ..n };
                changed = true;
            }
        }
        if changed {
            self.effects.clear();
        }
        self.publish();
        self.ff_sync();
        changed
    }

    /// The hub's and the deviations list's view of what the pads resolved to.
    fn publish(&self) {
        super::extras::set_resolved(self.devices.iter().any(|d| d.layout != Layout::Modern), self.devices.iter().any(|d| d.layout == Layout::Modern));
    }

    /// A pad is presented as Modern (the d-pad name entry, the footers).
    pub fn any_modern(&self) -> bool {
        self.devices.iter().any(|d| d.layout == Layout::Modern)
    }

    pub fn options(&self) -> PadOptions {
        self.options
    }

    /// gilrs's connected pads not yet attached take the free channels (the
    /// start-up enumeration, and after an unplug frees a channel: a third
    /// pad waiting is attached then; clone, as hot-plug is).
    fn rescan(&mut self) -> bool {
        let Some(g) = &self.gilrs else { return false };
        let pads: Vec<_> = g
            .gamepads()
            .map(|(id, p)| {
                let (vid, pid) = (p.vendor_id().unwrap_or(0), p.product_id().unwrap_or(0));
                (id, p.uuid(), p.name().to_string(), vid, pid, standard_pad(&p, vid, pid))
            })
            .collect();
        let mut any = false;
        for (id, uuid, name, vid, pid, standard) in pads {
            let d = self.present(Source::Gilrs(id), uuid, name, vid, pid, standard);
            any |= self.attach(d);
        }
        self.publish();
        self.ff_sync();
        any
    }

    /// 0x5CD200 / 0x5CA6A0: a new device takes the first free channel
    /// (none past two); 0x5CEF10's skip list first.
    fn attach(&mut self, d: Device) -> bool {
        if self.devices.len() >= CHANNELS || self.devices.iter().any(|e| e.source == d.source) || super::skipped(d.caps.vid, d.caps.pid, &d.name) {
            return false;
        }
        self.devices.push(d);
        true
    }

    fn detach(&mut self, source: Source, ch: &mut Changes) -> bool {
        let n = self.devices.len();
        if self.devices.first().is_some_and(|d| d.source == source) {
            ch.driver_lost = true;
        }
        self.devices.retain(|d| d.source != source);
        if let Source::Gilrs(id) = source {
            self.effects.retain(|e| e.0 != id);
        }
        n != self.devices.len()
    }

    /// One frame (0x5CFA20 then 0x5C1850): hot-plug, the states, the
    /// reports. `now` is the game clock (the script's time).
    pub fn poll(&mut self, now: f32) -> Changes {
        let mut ch = Changes::default();
        let tick = super::tick_at(f64::from(now));
        self.ticked = tick != self.tick;
        self.tick = tick;
        let mut events = Vec::new();
        if let Some(g) = self.gilrs.as_mut() {
            while let Some(ev) = g.next_event() {
                events.push((ev.id, ev.event));
            }
        }
        for (id, ev) in events {
            match ev {
                gilrs::EventType::Connected => {
                    let Some(g) = &self.gilrs else { continue };
                    let p = g.gamepad(id);
                    let (vid, pid) = (p.vendor_id().unwrap_or(0), p.product_id().unwrap_or(0));
                    let d = self.present(Source::Gilrs(id), p.uuid(), p.name().to_string(), vid, pid, standard_pad(&p, vid, pid));
                    ch.attached |= self.attach(d);
                }
                gilrs::EventType::Disconnected => {
                    if self.detach(Source::Gilrs(id), &mut ch) {
                        ch.detached = true;
                        ch.attached |= self.rescan();
                    }
                }
                _ => {}
            }
        }
        if let Some(g) = &self.gilrs {
            for d in &mut self.devices {
                if let Source::Gilrs(id) = d.source
                    && let Some(p) = g.connected_gamepad(id)
                {
                    d.inputs = gilrs_inputs(&p);
                }
            }
        }
        if let Some(mut s) = self.script.take() {
            while let Some((t, w)) = s.lines.get(s.next).filter(|(t, _)| *t <= now).cloned() {
                let _ = t;
                s.next += 1;
                self.script_line(&w, &mut ch);
            }
            self.script = Some(s);
        }
        let o = self.options;
        for d in &mut self.devices {
            d.prev = d.state;
            d.state = d.layout.state(&o.shape(&d.inputs));
            d.last = d.report;
            d.report = d.map.poll(&d.caps, &d.state);
        }
        if ch.attached || ch.detached {
            self.publish();
            self.ff_sync();
        }
        ch
    }

    /// Keeps the force-feedback output on the driving channel (the first
    /// device): opened for a device with FF actuators, with autocenter off
    /// on a wheel (0x5CD3E7); dropped when that device goes.
    ///
    /// The device lookup (on Windows DirectInput8Create and an EnumDevices
    /// of the attached game controllers, which can take a tenth of a second
    /// or more) runs once for each device that takes the channel, whether it
    /// finds force feedback or not: called every frame (the Controller
    /// extras re-applied), it only restores the found device's flag.
    fn ff_sync(&mut self) {
        let first = self.devices.first().map(|d| (d.source, d.caps.dev_type));
        if self.ff_looked == first.map(|f| f.0) {
            if self.ff.is_some()
                && let Some(d) = self.devices.first_mut()
            {
                d.caps.ff = true;
            }
            return;
        }
        self.ff_looked = first.map(|f| f.0);
        self.ff = None;
        let Some((source, class)) = first else { return };
        let started = std::time::Instant::now();
        let out = self.ff_open(source);
        let took = started.elapsed();
        self.ff_lookups += 1;
        self.ff_lookup_time += took;
        if let Some(d) = self.devices.first() {
            log::info!(
                "pads: force feedback on {:?} ({:04x}:{:04x}): {} (looked up in {:.1} ms)",
                d.name,
                d.caps.vid,
                d.caps.pid,
                if out.is_some() { "found" } else { "none" },
                took.as_secs_f64() * 1e3
            );
        }
        let Some((mut out, actuators)) = out else { return };
        let Some(model) = super::ff::Original::new(super::ff::Device { class, actuators }) else { return };
        if class == DevType::Driving {
            out.call(self.tick, super::ff_out::Call::Autocenter(false), actuators);
        }
        if let Some(d) = self.devices.first_mut() {
            d.caps.ff = true;
        }
        self.ff = Some(Ff { source, out, model, driving: false });
    }

    /// The force-feedback output of the device `source`, if it has one.
    fn ff_open(&self, source: Source) -> Option<(super::ff_out::Output, u8)> {
        match source {
            Source::Script(n) => self.script_ff.contains(&n).then(|| (super::ff_out::Output::recorder(), 1)),
            #[cfg(target_os = "linux")]
            Source::Gilrs(id) => {
                use gilrs::LinuxGamepadExt;
                self.gilrs.as_ref().and_then(|g| g.connected_gamepad(id)).and_then(|p| super::ff_out::Output::open(p.devpath()))
            }
            #[cfg(windows)]
            Source::Gilrs(_) => self.devices.first().and_then(|d| super::ff_out::Output::open_dinput(d.caps.vid, d.caps.pid)),
            #[cfg(not(any(target_os = "linux", windows)))]
            Source::Gilrs(_) => None,
        }
    }

    /// The controllers' frame took `took` (the game's `pad_frame`): a line
    /// for the log when frames are slow ([`FrameWatch`]).
    pub fn frame_took(&mut self, took: std::time::Duration, now: std::time::Instant) -> Option<String> {
        self.watch.note(took, now, self.ff_lookups, self.ff_lookup_time)
    }

    /// A device with force-feedback actuators drives (0x5B8250's question:
    /// the Feedback option is enabled).
    pub fn force_feedback(&self) -> bool {
        self.ff.is_some()
    }

    /// The driving channel is a wheel (0x5C1010): 0x41DFC0 steers through
    /// 0x4061C0.
    pub fn wheel_drives(&self) -> bool {
        self.devices.first().is_some_and(|d| d.caps.dev_type == DevType::Driving)
    }

    /// The force feedback's state (tests, the recorder's calls).
    pub fn ff(&self) -> Option<&Ff> {
        self.ff.as_ref()
    }

    /// One world tick of the original's force feedback: `driving` the feed
    /// and options of 0x406460 while the player drives (game state 4), the
    /// front-end path (0x5CFE40) otherwise; `strength` Overall Force
    /// Strength, set as the gain when driving starts (the input object's
    /// start, 0x41E28D → 0x5C9130). Nothing without an FF device.
    pub fn ff_tick(&mut self, driving: Option<(super::ff::Feed, super::ff::Options)>, strength: f32, clock: super::ff::Clock) {
        let Some(f) = self.ff.as_mut() else { return };
        let tick = self.tick;
        let actuators = f.model.device().actuators;
        let cmds = match driving {
            Some((feed, options)) => {
                if !f.driving {
                    f.driving = true;
                    f.out.call(tick, super::ff_out::Call::Gain(super::ff::gain(strength)), actuators);
                }
                f.model.driving(&feed, &options, clock)
            }
            None => {
                f.driving = false;
                f.model.front_end()
            }
        };
        for c in cmds {
            f.out.call(tick, super::ff_out::Call::Command(c), actuators);
        }
    }

    fn script_line(&mut self, w: &[String], ch: &mut Changes) {
        let id = |i: usize| w.get(i).and_then(|s| s.parse::<u32>().ok());
        let num = |i: usize| w.get(i).and_then(|s| s.parse::<f32>().ok());
        let hex = |i: usize, d: u16| w.get(i).and_then(|s| u16::from_str_radix(s.trim_start_matches("0x"), 16).ok()).unwrap_or(d);
        match w.first().map(String::as_str) {
            Some("connect") => {
                let Some(n) = id(1) else { return };
                let (vid, pid) = (hex(2, 0x045E), hex(3, 0x028E));
                let mut guid = [0u8; 16];
                guid[..4].copy_from_slice(&n.to_le_bytes());
                guid[4..6].copy_from_slice(&vid.to_le_bytes());
                guid[6..8].copy_from_slice(&pid.to_le_bytes());
                // `legacy` after the ids: a DirectInput-style device (Auto
                // leaves it Original); else standard unless a legacy product.
                let standard = !w.iter().any(|t| t == "legacy") && !legacy_product(vid, pid);
                // `ff`: the device reports force-feedback actuators; a
                // recorder plays its effects.
                if w.iter().any(|t| t == "ff") && !self.script_ff.contains(&n) {
                    self.script_ff.push(n);
                }
                let d = self.present(Source::Script(n), guid, format!("Script pad {n}"), vid, pid, standard);
                ch.attached |= self.attach(d);
            }
            Some("disconnect") => {
                if let Some(n) = id(1) {
                    let gone = self.detach(Source::Script(n), ch);
                    ch.detached |= gone;
                }
            }
            Some("axis") => {
                let (Some(n), Some(v)) = (id(1), num(3)) else { return };
                let Some(d) = self.devices.iter_mut().find(|d| d.source == Source::Script(n)) else { return };
                match w.get(2).map(String::as_str) {
                    Some("lx") => d.inputs.sticks[0] = v,
                    Some("ly") => d.inputs.sticks[1] = v,
                    Some("rx") => d.inputs.sticks[2] = v,
                    Some("ry") => d.inputs.sticks[3] = v,
                    Some("lt") => d.inputs.triggers[0] = v,
                    Some("rt") => d.inputs.triggers[1] = v,
                    Some("lz") => d.inputs.z[0] = v,
                    Some("rz") => d.inputs.z[1] = v,
                    _ => {}
                }
            }
            Some("button") => {
                let (Some(n), Some(b), Some(v)) = (id(1), w.get(2).and_then(|s| Btn::parse(s)), num(3)) else { return };
                if let Some(d) = self.devices.iter_mut().find(|d| d.source == Source::Script(n)) {
                    d.inputs.buttons[b as usize] = v != 0.0;
                }
            }
            _ => {}
        }
    }

    /// The analog report the driving reads: the first device's
    // prov: inferred pad.host.analog
    /// (**inferred**: the original's per-player device assignment,
    /// 0x571FC0, is not traced).
    pub fn analog(&self) -> Option<&Report> {
        self.devices.first().map(|d| &d.report)
    }

    /// Controller extra (Quit with Both Sticks): true once when both stick
    /// buttons (L3 and R3) of one pad are held together while `here` (the
    /// screen it works on) holds. It works only after every pad's stick
    /// buttons were seen released on that screen, so a stick already held
    /// when the screen appeared does nothing, and it fires again only after
    /// both are let go. Raw buttons, so the same with any layout and map.
    pub fn both_sticks(&mut self, here: bool) -> bool {
        if !here {
            self.sticks_armed = false;
            return false;
        }
        let b = |d: &Device| (d.inputs.buttons[Btn::L3 as usize], d.inputs.buttons[Btn::R3 as usize]);
        if !self.devices.iter().any(|d| matches!(b(d), (true, _) | (_, true))) {
            self.sticks_armed = true;
            return false;
        }
        if self.sticks_armed && self.devices.iter().any(|d| b(d) == (true, true)) {
            self.sticks_armed = false;
            return true;
        }
        false
    }

    /// The actions held on any controller (bit a set).
    pub fn held(&self) -> u64 {
        self.devices.iter().fold(0, |m, d| m | !d.report.mask)
    }

    /// The actions pressed this frame (held now, not last frame).
    pub fn pressed(&self) -> u64 {
        self.devices.iter().fold(0, |m, d| m | (!d.report.mask & d.last.mask))
    }

    /// The actions let go this frame (held last frame, not now).
    pub fn released(&self) -> u64 {
        self.devices.iter().fold(0, |m, d| m | (d.report.mask & !d.last.mask))
    }

    /// The FE stick rows' presses this poll (up, down, left, right): the
    /// crossing ([`FeRepeat`] at the original's tick 0: no repeat); with the
    /// Modern layout also the repeat while held, on 60 Hz ticks of the game
    /// clock.
    pub fn stick_pressed(&mut self) -> [bool; 4] {
        let mut out = [false; 4];
        for d in &mut self.devices {
            let tick = if d.layout == Layout::Modern { self.tick } else { 0 };
            let wheel = d.caps.dev_type == DevType::Driving;
            let p = d.repeat.step(super::fe_stick(&d.report, wheel), tick);
            for i in 0..4 {
                out[i] |= p[i];
            }
        }
        out
    }

    /// The motors of every attached pad (strong, weak, 0..1, already scaled
    /// by the Rumble option); 0, 0 stops them. gilrs pads without force
    /// feedback are skipped; the scripted pads record the value.
    pub fn set_rumble(&mut self, strong: f32, weak: f32) {
        let k = f32::from(self.options.rumble) / 100.0;
        let (strong, weak) = ((strong * k).clamp(0.0, 1.0), (weak * k).clamp(0.0, 1.0));
        if self.rumble_last == (strong, weak) && self.effects.len() == self.devices.iter().filter(|d| matches!(d.source, Source::Gilrs(_))).count() {
            return;
        }
        self.rumble_last = (strong, weak);
        self.rumble_out = self.devices.iter().filter(|d| matches!(d.source, Source::Script(_))).map(|_| (strong, weak)).collect();
        let Some(g) = self.gilrs.as_mut() else { return };
        for d in &self.devices {
            let Source::Gilrs(id) = d.source else { continue };
            if !self.effects.iter().any(|e| e.0 == id) {
                if strong == 0.0 && weak == 0.0 {
                    continue;
                }
                if !g.connected_gamepad(id).is_some_and(|p| p.is_ff_supported()) {
                    continue;
                }
                use gilrs::ff::{BaseEffect, BaseEffectType, EffectBuilder, Repeat, Replay, Ticks};
                let make = |g: &mut gilrs::Gilrs, kind: BaseEffectType| {
                    EffectBuilder::new().add_effect(BaseEffect { kind, scheduling: Replay { play_for: Ticks::from_ms(1000), ..Default::default() }, ..Default::default() }).repeat(Repeat::Infinitely).gamepads(&[id]).finish(g)
                };
                let (Ok(s), Ok(w)) = (make(g, BaseEffectType::Strong { magnitude: u16::MAX }), make(g, BaseEffectType::Weak { magnitude: u16::MAX })) else { continue };
                for e in [&s, &w] {
                    let _ = e.set_gain(0.0);
                    let _ = e.play();
                }
                self.effects.push((id, s, w));
            }
            if let Some(e) = self.effects.iter().find(|e| e.0 == id) {
                let _ = e.1.set_gain(strong);
                let _ = e.2.set_gain(weak);
            }
        }
    }
}

fn gilrs_inputs(p: &gilrs::Gamepad<'_>) -> PadInputs {
    use gilrs::{Axis, Button};
    let mut i = PadInputs { sticks: [p.value(Axis::LeftStickX), p.value(Axis::LeftStickY), p.value(Axis::RightStickX), p.value(Axis::RightStickY)], ..Default::default() };
    let trigger = |b: Button| p.button_data(b).map_or(0.0, |d| d.value());
    i.triggers = [trigger(Button::LeftTrigger2), trigger(Button::RightTrigger2)];
    i.z = [p.value(Axis::LeftZ), p.value(Axis::RightZ)];
    let map = [
        (Btn::South, Button::South),
        (Btn::East, Button::East),
        (Btn::West, Button::West),
        (Btn::North, Button::North),
        (Btn::L1, Button::LeftTrigger),
        (Btn::R1, Button::RightTrigger),
        (Btn::L2, Button::LeftTrigger2),
        (Btn::R2, Button::RightTrigger2),
        (Btn::Select, Button::Select),
        (Btn::Start, Button::Start),
        (Btn::Mode, Button::Mode),
        (Btn::L3, Button::LeftThumb),
        (Btn::R3, Button::RightThumb),
        (Btn::Up, Button::DPadUp),
        (Btn::Down, Button::DPadDown),
        (Btn::Left, Button::DPadLeft),
        (Btn::Right, Button::DPadRight),
    ];
    for (b, g) in map {
        i.buttons[b as usize] = p.is_pressed(g);
    }
    // A d-pad that the mapping gives as a hat axis (DPadX / DPadY, y up
    // positive) rather than as buttons.
    let (hx, hy) = (p.value(Axis::DPadX), p.value(Axis::DPadY));
    i.buttons[Btn::Left as usize] |= hx < -0.5;
    i.buttons[Btn::Right as usize] |= hx > 0.5;
    i.buttons[Btn::Up as usize] |= hy > 0.5;
    i.buttons[Btn::Down as usize] |= hy < -0.5;
    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pad::{act, axis, driving};

    /// A wheel the original's type override or the SDL list names is
    /// presented as DI8DEVTYPE_DRIVING with the wheel's raw axes (0x5C11B0
    /// then leaves pedals with lZ / lRz unbound); `ff` gives it the
    /// recorder: autocenter off at once, the front-end spring 7000 per
    /// tick, the gain from Overall Force Strength when driving starts.
    #[test]
    fn wheel_and_force_feedback() {
        use crate::pad::ff::{Clock, Command, Effect, Feed, Options, Slot};
        use crate::pad::ff_out::Call;
        assert!(is_wheel(0x046D, 0xC24F) && is_wheel(0x044F, 0x04B2) && !is_wheel(0x045E, 0x028E));
        let mut h = Host::scripted("0 connect 1 046d c24f ff\n0 axis 1 lx -0.5\n0 axis 1 rz 0.25\n");
        h.poll(0.0);
        assert!(h.wheel_drives() && h.force_feedback());
        let d = &h.devices[0];
        assert_eq!((d.layout, d.caps.dev_type, d.caps.ff), (Layout::Wheel, DevType::Driving, true));
        assert_eq!((d.state.axes[0], d.state.axes[5]), (-5000, 2500));
        // Separate pedal axes: no combined default (0x5C12EF).
        assert!(!d.map.on[3]);
        h.ff_tick(None, 1.0, Clock::default());
        let feed = Feed { speed: 20.0, front_down: [true, true], front_surface: [1, 1], ..Feed::default() };
        h.ff_tick(Some((feed, Options::default())), 0.3, Clock::default());
        let calls: Vec<Call> = h.ff().unwrap().out.calls().iter().map(|c| c.1).collect();
        assert_eq!(calls[0], Call::Autocenter(false));
        assert_eq!(calls[1], Call::Command(Command::Set(Effect::Spring { coef: 7_000 })));
        assert_eq!(calls[2], Call::Command(Command::Start(Slot::Spring)));
        assert_eq!(calls[3], Call::Gain(7_600));
        assert!(calls[4..].contains(&Call::Command(Command::Stop(Slot::Spring))));
        // A plain pad: no force feedback, no wheel.
        let mut p = Host::scripted("0 connect 1 045e 028e\n");
        p.poll(0.0);
        assert!(!p.wheel_drives() && !p.force_feedback());
        p.ff_tick(None, 1.0, Clock::default());
        assert!(p.ff().is_none());
    }

    /// gilrs finds a virtual force-feedback wheel (uinput, 046D:C24F, the
    /// node in `NFSU2_FF_NODE`): it is presented as a wheel and its node
    /// opened for the effects.
    #[test]
    #[cfg(target_os = "linux")]
    #[ignore = "needs NFSU2_FF_NODE, a virtual force-feedback wheel"]
    fn gilrs_finds_a_virtual_ff_wheel() {
        let _ = std::env::var("NFSU2_FF_NODE").expect("NFSU2_FF_NODE");
        let mut h = Host::new(true);
        h.poll(0.0);
        let d = h.devices.iter().find(|d| (d.caps.vid, d.caps.pid) == (0x046D, 0xC24F)).expect("the wheel");
        assert_eq!(d.layout, Layout::Wheel);
        assert!(h.wheel_drives() && h.force_feedback(), "{:?}", h.devices.iter().map(|d| &d.name).collect::<Vec<_>>());
        h.ff_tick(None, 1.0, crate::pad::ff::Clock::default());
    }

    #[test]
    fn layouts() {
        let mut p = PadInputs::default();
        p.sticks = [-1.0, 1.0, 0.5, -1.0];
        p.triggers = [1.0, 0.0];
        p.buttons[Btn::South as usize] = true;
        p.buttons[Btn::Right as usize] = true;
        p.buttons[Btn::Up as usize] = true;
        let x = Layout::Xbox.state(&p);
        assert_eq!(x.axes, [-10000, -10000, 10000, 5000, 10000, 0, 0, 0]);
        assert!(x.buttons[0] && x.pov[0] == 4500);
        let s = Layout::Sony.state(&p);
        assert_eq!(s.axes, [-10000, -10000, 5000, 10000, -10000, 10000, 0, 0]);
        assert!(s.buttons[1] && !s.buttons[0]);
        let c = Layout::Sony.caps(0x054C, 0x09CC);
        let m = PadMap::attach(&c);
        // The right stick down (lRz +10000) is the brake.
        assert_eq!(m.axis[3], axis::RZ);
        let r = m.poll(&c, &s);
        assert!(r.held(act::THROTTLE));
        assert!((driving(0, Some(&r), false).brake - 248.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn both_sticks_together_once_released_first() {
        // Pad 1: L3 then R3 (both held at 1.5 s), released at 2 s, both
        // again at 3 s. Pad 2: only R3, held from the start to 2.5 s.
        let script = "0 connect 1\n0 connect 2\n0 button 2 r3 1\n1.0 button 1 l3 1\n1.5 button 1 r3 1\n2.0 button 1 l3 0\n2.0 button 1 r3 0\n2.5 button 2 r3 0\n3.0 button 1 ls 1\n3.0 button 1 rs 1\n";
        for layout in [0, 1] {
            let mut h = Host::scripted(script);
            h.set_options(PadOptions { layout, ..Default::default() });
            let mut fired = Vec::new();
            for k in 0..240 {
                let t = k as f32 / 60.0;
                h.poll(t);
                if h.both_sticks(true) {
                    fired.push(k);
                }
            }
            // Pad 2's R3 held since the screen appeared: nothing until every
            // stick button was released (2.5 s), so 1.5 s does nothing; one
            // stick alone never fires; then once at 3 s.
            assert_eq!(fired, [180], "layout {layout}");
        }
        // Off the screen: never, and coming back with both held: not until released.
        let mut h = Host::scripted("0 connect 1\n0 button 1 l3 1\n0 button 1 r3 1\n1 button 1 l3 0\n1 button 1 r3 0\n1.5 button 1 l3 1\n1.5 button 1 r3 1\n");
        let mut fired = Vec::new();
        for k in 0..120 {
            h.poll(k as f32 / 60.0);
            if h.both_sticks(k >= 30) {
                fired.push(k);
            }
        }
        assert_eq!(fired, [90]);
    }

    #[test]
    fn stick_repeat_through_the_host_is_rate_independent() {
        // The left stick held right 1 s in a menu at 60, 120, 200 and 1000
        // polls a second: the same presses (the press, then 25 ticks, then
        // every 6).
        let mut seen = Vec::new();
        for hz in [60.0f64, 120.0, 200.0, 1000.0] {
            let mut h = Host::scripted("0 connect 1\n1.0 axis 1 lx 1\n2.0 axis 1 lx 0\n");
            h.set_options(PadOptions { layout: 1, ..Default::default() });
            let (mut n, mut t) = (0, 0.0f64);
            while t < 2.5 {
                h.poll(t as f32);
                n += h.stick_pressed()[3] as u32;
                t += 1.0 / hz;
            }
            seen.push(n);
        }
        // Press at tick 60, repeats at 86, 93, …, 114 (> deadline): 1 + 1 + 4.
        assert!(seen.iter().all(|&n| n == seen[0]) && (6..=7).contains(&seen[0]), "{seen:?}");
        // Native (tick 0): the press only.
        let mut h = Host::scripted("0 connect 1\n1.0 axis 1 lx 1\n");
        h.set_options(PadOptions { layout: 0, ..Default::default() });
        let mut n = 0;
        for k in 0..200 {
            h.poll(k as f32 / 60.0);
            n += h.stick_pressed()[3] as u32;
        }
        assert_eq!(n, 1);
    }

    #[test]
    fn auto_layout_resolves_per_pad() {
        use crate::pad::extras::{resolved, LAYOUT_AUTO};
        // Auto (the default): a standard pad Modern; a `legacy` device, a
        // product the original has a preset for (Logitech Dual Action) and
        // a wheel (0x800588's override) Original.
        let mut h = Host::scripted("0 connect 1 045e 0b12\n0 connect 2 046d c216\n1 disconnect 1\n1 disconnect 2\n1 connect 3 1234 5678 legacy\n1 connect 4 044f 04b2\n");
        assert_eq!(h.options().layout, LAYOUT_AUTO);
        h.poll(0.0);
        assert_eq!(h.devices.iter().map(|d| d.layout).collect::<Vec<_>>(), [Layout::Modern, Layout::Generic]);
        assert_eq!(resolved(), (true, true));
        h.poll(1.0);
        assert!(h.devices.iter().all(|d| d.layout != Layout::Modern));
        assert_eq!(h.devices[1].caps.dev_type, DevType::Driving);
        assert_eq!(resolved(), (true, false));
        // No pad: nothing resolved.
        let mut h = Host::scripted("0 connect 1\n0.5 disconnect 1\n");
        h.poll(0.0);
        h.poll(1.0);
        assert_eq!(resolved(), (false, false));
    }

    #[test]
    fn auto_keeps_custom_original_bindings() {
        // The profile holds an Xbox pad's Original map: as its defaults it
        // changes nothing (Auto stays Modern); with a custom binding Auto
        // presents it as Original, and the saved map applies.
        let mut guid = [0u8; 16];
        guid[..4].copy_from_slice(&1u32.to_le_bytes());
        guid[4..6].copy_from_slice(&0x045Eu16.to_le_bytes());
        guid[6..8].copy_from_slice(&0x028Eu16.to_le_bytes());
        let caps = Layout::Xbox.caps(0x045E, 0x028E);
        let defaults = PadMap::attach(&caps).to_block(&guid);
        let mut custom = PadMap::attach(&caps);
        custom.begin_capture(super::super::act::PAUSE, 0);
        custom.bind_button(7);
        let custom = custom.to_block(&guid);
        let mut h = Host::scripted("0 connect 1\n");
        assert!(!h.set_saved_blocks(&[defaults]));
        h.poll(0.0);
        assert_eq!(h.devices[0].layout, Layout::Modern);
        // A profile with custom bindings loads: back to Original (one way).
        assert!(h.set_saved_blocks(&[custom]));
        assert_eq!((h.devices[0].layout, h.devices[0].guid), (Layout::Xbox, guid));
        assert!(!h.set_saved_blocks(&[defaults]), "restoring the defaults does not flip it back mid-session");
        // Attached with the custom block already saved: Original at once.
        let mut h = Host::scripted("0 connect 1\n");
        h.set_saved_blocks(&[custom]);
        h.poll(0.0);
        assert_eq!(h.devices[0].layout, Layout::Xbox);
    }

    #[test]
    fn modern_relayout_and_driver_lost() {
        let mut h = Host::scripted("0 connect 1 054c 09cc\n0 connect 2\n1 disconnect 1\n");
        h.set_options(PadOptions { layout: 0, ..Default::default() });
        h.poll(0.0);
        assert_eq!(h.devices.len(), 2);
        let native_guid = h.devices[0].guid;
        assert!(h.set_options(PadOptions { layout: 1, ..Default::default() }));
        assert!(h.devices.iter().all(|d| d.layout == Layout::Modern && d.map.exact));
        assert_eq!(h.devices[0].guid, modern_guid(native_guid));
        assert_eq!(h.devices[0].family, Family::PlayStation);
        assert!(h.set_options(PadOptions { layout: 0, ..Default::default() }));
        assert_eq!((h.devices[0].guid, h.devices[0].layout), (native_guid, Layout::Sony));
        let ch = h.poll(1.0);
        assert!(ch.detached && ch.driver_lost && h.devices.len() == 1);
    }

    /// The force-feedback device is looked up once per device that takes
    /// the driving channel, found or not, not every frame (#310: on Windows
    /// a pad without force feedback made every frame run DirectInput's
    /// device enumeration, 6 fps): many frames of the per-frame calls
    /// (`set_options` then `poll`) make one lookup; a re-plug makes one more.
    #[test]
    fn force_feedback_lookup_is_bounded() {
        for ff in ["", " ff"] {
            let mut h = Host::scripted(&format!("0 connect 1 045e 028e{ff}\n5 disconnect 1\n6 connect 1 045e 028e{ff}\n"));
            for frame in 0..600 {
                let t = frame as f32 / 120.0;
                h.set_options(PadOptions { layout: (frame / 200) as u8 % 2, ..Default::default() });
                h.poll(t);
            }
            assert_eq!(h.ff_lookups, 1, "one lookup for the attached pad{ff}");
            assert_eq!(h.force_feedback(), !ff.is_empty());
            assert_eq!(h.devices[0].caps.ff, !ff.is_empty(), "the flag survives a re-presentation");
            h.poll(5.0);
            assert!(h.devices.is_empty() && !h.force_feedback());
            for frame in 0..100 {
                h.set_options(PadOptions::default());
                h.poll(6.0 + frame as f32 / 120.0);
            }
            assert_eq!(h.ff_lookups, 2, "one more for the re-plugged pad{ff}");
            assert_eq!(h.force_feedback(), !ff.is_empty());
        }
    }

    /// A slow controller frame reaches the log at most every ten seconds.
    #[test]
    fn slow_controller_frames_are_reported_rate_limited() {
        use std::time::{Duration, Instant};
        let mut w = FrameWatch::default();
        let t0 = Instant::now();
        assert!(w.note(Duration::from_millis(1), t0, 0, Duration::ZERO).is_none());
        let first = w.note(Duration::from_millis(160), t0, 3, Duration::from_millis(480)).expect("the first slow frame");
        assert!(first.contains("160.0 ms") && first.contains("lookups 3 in 480 ms"), "{first}");
        let lines = (1..60).filter_map(|i| w.note(Duration::from_millis(160), t0 + Duration::from_millis(i * 160), 3, Duration::ZERO)).count();
        assert_eq!(lines, 0, "within ten seconds");
        let again = w.note(Duration::from_millis(170), t0 + Duration::from_secs(11), 3, Duration::ZERO).expect("after ten seconds");
        assert!(again.contains("60 slow frames") && again.contains("longest 170.0 ms"), "{again}");
    }

    #[test]
    fn script_hot_plug() {
        let mut h = Host::scripted("0 connect 1 054c 09cc\n0.5 axis 1 lx -1\n1.0 button 1 cross 1\n2 disconnect 1\n2.5 connect 1\n");
        h.set_options(PadOptions { layout: 0, ..Default::default() });
        assert!(h.poll(0.0).attached);
        assert_eq!(h.devices.len(), 1);
        assert_eq!(h.devices[0].layout, Layout::Sony);
        h.poll(0.5);
        assert_eq!(driving(h.held(), h.analog(), false).steer, 0.9765625);
        h.poll(1.0);
        assert!(h.pressed() & (1 << act::THROTTLE) != 0);
        h.poll(1.1);
        assert!(h.pressed() == 0 && h.held() & (1 << act::THROTTLE) != 0);
        assert!(h.poll(2.0).detached && h.devices.is_empty());
        assert!(h.poll(3.0).attached);
        assert_eq!(h.devices[0].layout, Layout::Xbox);
    }
}
