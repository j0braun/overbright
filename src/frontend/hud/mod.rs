//! The in-game HUD: the HUD_SingleRace package driven the way the game
//! drives it (see docs/formats.md, "HUD").
//!
//! The game builds the HUD (0x4F8BF0) from elements. Each element finds its
//! objects by name and carries a feature mask (low and high words at
//! +0x10/+0x14). Every frame 0x6000A0 works out which features the race
//! wants; when they change, 0x4CA090 tells each element and the element
//! shows all its objects if its mask meets the features, or hides them
//! (0x4C6690). Then each element's update fills in its objects (0x4EC5A0).
//! Objects that belong to no element keep the package's own state.

use crate::frontend::fe_ids;
use crate::fxhash::FxHashSet as HashSet;

use super::{Assets, Quad, ScreenState, argb, grouped, layout};
use crate::formats::hash::fe_hash;

mod bars;
mod board;
mod car_gauges;
mod cash;
mod drag;
mod elements;
mod features;
mod gauges;
mod input;
mod leaderboard;
mod minimap;
mod objects;
mod outrun_meter;
mod popups;
mod prompt_queue;
mod prompts;
mod race_info;
mod style;

use bars::*;
use drag::*;
use elements::*;
pub use features::*;
pub use gauges::*;
pub use super::format::{signed_time_text, time_text};
use super::format::ticks;
pub use input::*;
pub use leaderboard::*;
use minimap::*;
use popups::*;
pub use prompts::*;

pub const PACKAGE: &str = "HUD_SingleRace";

/// The HUD package a race uses (0x5F1983): HUD_Drag in a drag race,
/// HUD_Drift in a drift race or with the 0x89E7E2 flag (the clone's
/// `Url`), HUD_Short_Track with the 0x89E7E3 flag (street X), otherwise
/// HUD_SingleRace.
pub fn package_for(race: crate::race::RaceMode) -> &'static str {
    use crate::race::RaceMode;
    match race {
        RaceMode::Drag => "HUD_Drag",
        RaceMode::Drift | RaceMode::Url => "HUD_Drift",
        RaceMode::StreetX => "HUD_Short_Track",
        RaceMode::FreeRoam => PACKAGE,
    }
}

/// The HUD's running state: its objects' scripts and what it last showed.
#[derive(Default)]
pub struct Hud {
    state: ScreenState,
    /// Cash_Reward's text since the popup last came (0x537BF0 keeps it).
    cash_reward: Option<String>,
    /// The bank element's (0x4F4F10, HUD +0x80) cash pickup waiting to be
    /// shown (+0x28; 0x528380 → 0x4AC1B0 writes it).
    bank_pending: u32,
    /// Cash_Bank's text since a pickup was last shown.
    bank_text: Option<String>,
    /// The engage prompt's queue ([`Prompts`]).
    prompt: Prompts,
    /// The group scripts 0x4CA090 last ran, by object.
    groups: crate::fxhash::FxHashMap<u32, &'static str>,
    wrong_way: bool,
    race_over: bool,
    /// 0x4DA190's leader board phase: since when (ms of `Input::race_age`;
    /// hud +0x1BC) and
    /// whether the rows show names (+0x1C0) or gaps.
    board_since: Option<f32>,
    board_names: bool,
    /// 0x4EBB10's race time (hud +0x50), the laps it last saw and when
    /// (ms; +0x4C) a new lap came in.
    race_time: f32,
    laps_seen: u32,
    lap_at: Option<f32>,
    /// Hurry_Text's message queue (element +0x48, 0x4EBAC0 / 0x4EB9F0):
    /// when its "Hurry!" stops showing (ms).
    hurry_until: Option<f32>,
    /// The hurry counter's time left at the last update, s: the step since
    /// then stands for 0x865150 in the "Hurry!" test.
    hurry_prev: Option<f32>,
    /// The drag tachometer's last gear (+0x80), when its ShiftMessage
    /// started (ms) and whether it is a rating's (+0x88) or an over-rev /
    /// short shift's (+0x8A / +0x8B), and RPM_Redline blinking (+0x8D).
    drag_gear: usize,
    shift_message: Option<(f32, u32)>,
    redline_blink: bool,
    /// The needle's last tint written (None: its own colour). Reverse keeps
    /// it, since the game writes no tint there and `quads` clears the map.
    needle_color: Option<[f32; 4]>,
    /// The outrun meter (0x4F87E0): its last value (+0x24), the frames it
    /// has been shown (+0x44, at most 4) and Out_Run_Message's label and
    /// when its ZoomIn started (ms).
    meter_last: f32,
    meter_frames: u16,
    outrun_message: Option<(u32, f32)>,
    /// The SMS element's last unread count (HUD +0x7C +0).
    sms_last: u32,
    /// Each element's object hashes for one package and tachometer
    /// ([`ElementObjects`]), which do not change from frame to frame.
    elements: Option<ElementObjects>,
    #[cfg(test)]
    reference_hierarchy: bool,
    /// The style element's trick popups (+0x20, see [`Popups`]).
    popups: Popups,
    /// The features the last update asked for (low, high; 0x6005A2 keeps
    /// them at player +0x258 / +0x25C).
    features: (u32, u32),
    /// The race kind the last [`Hud::step`] built for: its package
    /// ([`package_for`]) is the one [`Hud::layout`] lays out.
    race: crate::race::RaceMode,
}

/// The HUD elements' objects as `Hud::quads` finds them in a package:
/// per element (and per `HASHED` entry) its mask and the hashes of its
/// objects, the numbered ones only where the package has them.
struct ElementObjects {
    /// The package they were found in (its name and object list) and
    /// whether the drag tachometer stood in for the first element.
    package: (String, usize, usize),
    drag: bool,
    /// The car's turbo when they were found (it picks the drift angle's
    /// backing, [`DRIFT_ANGLE_BACKING`]).
    turbo: bool,
    elements: Vec<(u32, u32, Vec<u32>)>,
    // Immutable package hierarchy, in the original object-list order.
    children: crate::fxhash::FxHashMap<u32, Vec<(u32, u32)>>,
    /// The hidden set the masks give (below), for the last (low mask, high
    /// mask, drift race): a function of those and of this package alone,
    /// so it is worked out again only when one of them changes.
    hidden: Option<((u32, u32, bool), HashSet<u32>)>,
}

impl Hud {
    /// Dev harness (rate gate, `state_hash` `hud`): the screen state and
    /// every other running field as text (Debug; floats print their exact
    /// value), the groups sorted.
    #[cfg(feature = "dev-harness")]
    pub fn dev_state(&self) -> (&ScreenState, String) {
        let mut groups: Vec<_> = self.groups.iter().collect();
        groups.sort();
        let p = &self.prompt;
        let text = format!(
            "{:?}",
            (
                (self.cash_reward.as_ref(), self.bank_pending, self.bank_text.as_ref(), groups),
                (p.prompts, &p.dropped, p.marker_kind, p.phone_queued, &p.sounds),
                (self.wrong_way, self.race_over, self.board_since, self.board_names, self.race_time, self.laps_seen),
                (self.lap_at, self.hurry_until, self.hurry_prev),
                (self.drag_gear, self.shift_message, self.redline_blink),
                (self.needle_color, self.meter_last, self.meter_frames, self.outrun_message, self.sms_last),
                (&self.popups, self.features),
            )
        );
        (&self.state, text)
    }

    /// A cash pickup collected (0x528380 → 0x4AC1B0 with the player's HUD
    /// +0x80): the bank element shows it at its next update while shown.
    pub fn bank_reward(&mut self, cash: u32) {
        self.bank_pending = cash;
    }

    /// Re-time the scripts started at `from` to start at `to` (for shots,
    /// whose render runs at a time of their own).
    pub fn retime(&mut self, from: f32, to: f32) {
        for (_, start) in self.state.scripts.values_mut() {
            if *start == from {
                *start = to;
            }
        }
    }

    /// The pickup still waiting to be shown (0 none).
    pub fn bank_pending(&self) -> u32 {
        self.bank_pending
    }

    /// The features (low, high) the last [`Hud::quads`] asked for.
    pub fn features(&self) -> (u32, u32) {
        self.features
    }

    /// The SMS slot's state (1 queued, 2–3 flashing, 4 resting), if any.
    pub fn sms_state(&self) -> Option<u8> {
        self.prompt_state(0x13)
    }

    /// The state of the engage prompt's slot of `kind`, if one is used.
    pub fn prompt_state(&self, kind: u8) -> Option<u8> {
        self.prompt.prompts.iter().find(|x| x.state != 0 && x.kind == kind).map(|x| x.state)
    }

    /// The engage prompt's sounds since the last call, as menu events on
    /// controls for 0x51DE40 (always FOCUS on control 0xD, 0xE or 0xF).
    pub fn take_sounds(&mut self) -> Vec<(u8, u8)> {
        std::mem::take(&mut self.prompt.sounds)
    }

    /// An object's text as the last update left it.
    pub fn text_of(&self, object: &str) -> Option<&str> {
        self.state.text.get(&fe_hash(object)).map(String::as_str)
    }
}

/// What every piece of [`Hud::quads`] reads: its arguments, the package,
/// the features (low, high) and whether a drag race.
#[derive(Clone, Copy)]
pub(super) struct QuadsFrame<'a> {
    assets: &'a Assets,
    package: &'a super::feng::Package,
    input: &'a Input,
    lo: u32,
    hi: u32,
    now: f32,
    drag: bool,
}

/// The HUD package of `race`, or HUD_SingleRace when the archive lacks it.
fn package_of(assets: &Assets, race: crate::race::RaceMode) -> Option<&super::feng::Package> {
    assets.package(package_for(race)).or_else(|| assets.package(PACKAGE))
}

impl Hud {
    /// The HUD's update and then its quads at `now` ms ([`Hud::step`],
    /// [`Hud::layout`]).
    pub fn quads(&mut self, assets: &Assets, input: &Input, now: f32) -> Vec<Quad> {
        self.step(assets, input, now);
        self.layout(assets, now)
    }

    /// The HUD's quads from its state as the last [`Hud::step`] left it, at
    /// `now` ms: a pure read (the game draws this between its ticks;
    /// drawing it twice, or not at all, changes nothing).
    pub fn layout(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        let Some(package) = package_of(assets, self.race) else { return Vec::new() };
        // In a race or free roam the game holds the in-game bundles' copies
        // of the HUD's textures, not FrontB.lzc's (see `Assets::in_game`):
        // the red zones (3RDPERSON_TURBORED_1, 3RDPERSON_REDLINE_<rpm>) are
        // additive there, so the package's dark red (0xC8750B0B) lightens.
        let mut quads = layout(assets, package, &self.state, now);
        for q in &mut quads {
            if let Some(alias) = q.texture.and_then(|h| assets.in_game.get(&h)) {
                q.texture = Some(*alias);
            }
        }
        quads
    }

    /// One HUD update at `now` ms (0x600910 → 0x6000A0 the features, then
    /// each element's update 0x4EC5A0): the features, every element's
    /// objects, scripts, texts and timers. Once per tick.
    pub fn step(&mut self, assets: &Assets, input: &Input, now: f32) {
        let Some(package) = package_of(assets, input.race) else { return };
        self.race = input.race;
        let (lo, hi) = features(input);
        self.features = (lo, hi);
        let s = &mut self.state;
        s.hidden.clear();
        s.text.clear();
        s.color.clear();
        s.angle.clear();
        s.size.clear();
        s.image.clear();
        s.uv.clear();
        s.position.clear();
        s.chain_init = true;
        let f = QuadsFrame { assets, package, input, lo, hi, now, drag: input.drag.is_some() };
        self.element_objects(&f);
        self.element_masks(&f);
        self.turbo_gauge(&f);
        self.prompt_queue(&f);
        self.style_popups(&f);
        self.tachometer(&f);
        self.lap_info(&f);
        self.drift_readouts(&f);
        self.wrong_way_sign(&f);
        self.race_over_message(&f);
        self.outrun_meter(&f);
        self.countdown(&f);
        self.track_map(&f);
        self.lap_box_groups(&f);
        self.race_position(&f);
        self.leader_board(&f);
        self.bank_and_cash_reward(&f);
        self.challenge_timer(&f);
        self.drag_gauges(&f);
    }

    /// The HUD update of a pause tick at `now` ms (measured under Wine,
    /// **traced**): under the pause (flow state 6) 0x600910 still runs the
    /// widget updates 0x4EC5A0 every iteration, but 0x6000A0 gives them
    /// only the pause features (low 0x40000, high 0); each widget tests its
    /// bits first. The cash popup (0x4A9FF0, high 0x80) hides and returns
    /// before its count (the game's `cash_popup_update` is not called);
    /// the outrun meter (0x4DABD0, high 0x40) turns its group INACTIVE and
    /// clears its shown-frame count +0x44 unless in an outrun (race type
    /// 7, `outrun`). The other elements' pause behaviour is their bits'
    /// (not traced one by one): they are left as the last update left them.
    pub fn pause_step(&mut self, now: f32, outrun: bool) {
        self.features = (0x4_0000, 0);
        if self.groups.get(&fe_ids::OBJ_8CA76279) != Some(&"INACTIVE") {
            self.state.run(fe_ids::OBJ_8CA76279, "INACTIVE", now);
            self.groups.insert(fe_ids::OBJ_8CA76279, "INACTIVE");
        }
        if !outrun {
            self.meter_frames = 0;
        }
        self.outrun_message = None;
    }
}

#[cfg(test)]
mod hierarchy_tests;
#[cfg(test)]
mod tests;

/// The CRT's `_ftol` (0x75BED4): `fistp qword` with truncation, the low
/// dword kept. A value outside the 64-bit range, ±inf or NaN gives the
/// integer-indefinite 0x8000000000000000, whose low dword is 0.
pub(crate) fn ftol(v: f32) -> i32 {
    let v = f64::from(v);
    if v.is_finite() && v.abs() < 9.223_372_036_854_775_808e18 { v.trunc() as i64 as i32 } else { 0 }
}
