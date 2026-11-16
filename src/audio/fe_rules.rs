//! Which FE sound each screen's handler plays for a key (sound round 7).
//!
//! The front end plays a menu sound only where a screen's own message
//! handler calls PlayFESound (0x458520), FE_PlayEventSound 0x50D150(_,
//! event, control), FE_PlaySoundId 0x50D180(id), 0x51DE40(event, control)
//! or 0x51DE70(_, event) (column 0) — so the sound follows what the
//! control did, not the key. These functions give, for a screen's state
//! before the key, the sound id its handler plays (see formats.md, "Sound
//! round 7"). Event table cells (0x837FE0, rows of 27, filled by 0x50CF40)
//! come from [`super::fe::sound`]; rows the fill leaves at −1 (event 0,
//! 3, 20–25) play nothing.

use super::fe::{control, event, sound};
use crate::frontend::screens::Key;

/// The pad messages the clone's keys stand for (`menu_key`).
fn cell(ev: u8, ctl: u8) -> Option<u8> {
    sound(ev, ctl)
}

/// An option row's class, by its vtable's +0x3C (the control column) and
/// +0x38 (the event filter) — the option builders 0x4B3110..0x4B8B4D and
/// 0x4B6510 make only these three.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RowClass {
    /// +0x3C 0x40CB20 (column 1, SLIDER), +0x38 0x50D6F0: Left plays
    /// unless the value before the step (+0x80, the model's +0x2C) was
    /// the minimum (+0x7C), Right unless it was the maximum (+0x78).
    Slider { at_min: bool, at_max: bool },
    /// +0x3C 0x408CD0 (column 2, TOGGLE), +0x38 0x50D500 (the default
    /// event): Left / Right always play.
    Toggle,
    /// +0x3C 0x40CBB0 (column 4, BUTTON), +0x38 0x50D800: only
    /// 0x406415E3 gives SELECT, so Left / Right play nothing.
    Button,
}

/// The list screen base handler 0x556030 (vtable 0x795CB0 +0x38; list
/// ctor 0x552790 sets +0x148 and +0x149): UI_Options (0x49C690),
/// UI_PauseOptions (0x4A8010), UI_PC_Customize_Options (0x49C540),
/// UI_Wheel_Options (0x49CBB0), UI_QRModeOptions (0x4EF620),
/// UI_PerformanceBrandSelect (0x55C940) pass their keys to it.
///
/// * Up / Down (0x72619778 / 0x911C0A4B) step the list (0x5390F0) and
///   play UP / DOWN on column 7 — even when the step stops at an end.
/// * Left / Right go to the current row when it is enabled (+0x28): the
///   row's step (vtable +4), then 0x50D460 → the row's +0x38 filter
///   ([`RowClass`]), on the row's +0x3C column.
/// * Accept (0x0C407210) and Back (0x911AB364): the row handles it, and
///   with no row or an enabled one SELECT / BACK play on the screen's
///   +0x30 column (both cells the same in every column: ids 4 / 5).
/// * The shoulder triggers, Button1 and Start: nothing.
///
/// `row` is the current row: its class and whether it is enabled; `None`
/// with no row.
pub fn list_screen(key: Key, row: Option<(RowClass, bool)>) -> Option<u8> {
    let enabled = row.is_none_or(|(_, e)| e);
    match key {
        Key::Up => cell(event::UP, 7),
        Key::Down => cell(event::DOWN, 7),
        Key::Left | Key::Right => {
            let (class, true) = row? else { return None };
            let left = key == Key::Left;
            let (ev, col) = match class {
                RowClass::Slider { at_min, at_max } => {
                    if (left && at_min) || (!left && at_max) {
                        return None;
                    }
                    (if left { event::LEFT } else { event::RIGHT }, control::SLIDER)
                }
                RowClass::Toggle => (if left { event::LEFT } else { event::RIGHT }, control::TOGGLE),
                RowClass::Button => return None,
            };
            cell(ev, col)
        }
        Key::Accept if enabled => cell(event::SELECT, control::GENERIC),
        Key::Back if enabled => cell(event::BACK, control::GENERIC),
        _ => None,
    }
}

/// UI_PerformanceTuning_Sliders / _Drivetrain (0x5650D0; their screen
/// ctor 0x55AC20 clears the base's +0x149, so 0x556030 plays nothing):
/// Down id 1, Up id 0, Left id 9, Right id 8 (direct PlayFESound, on
/// every press), Back table [BACK][0] (0x51DE70, id 5), Button1
/// (0xC519BFC0, the defaults) id 11; Accept (the row's +0x24) nothing.
pub fn tuning(key: Key) -> Option<u8> {
    match key {
        Key::Down => Some(1),
        Key::Up => Some(0),
        Key::Left => Some(9),
        Key::Right => Some(8),
        Key::Back => cell(event::BACK, 0),
        Key::Button1 => Some(11),
        _ => None,
    }
}

/// The engage dialogs' "ready" (0x35F8620B, sent by the package 180 ms
/// in): UI_EngageRaceDialog 0x50D180(15), UI_EngageMessageDialog
/// 0x50D180(15), UI_SMS_Mailbox 0x50D180(15), UI_EngageShopDialog table
/// [BACK][0] (0x51DE70, id 5); UI_EngageEventDialog plays nothing.
pub const READY_RACE: u8 = 15;
pub const READY_SHOP: u8 = 5;

/// UI_EngageRaceDialog (0x499C00). `accept_focused`: the focused button,
/// `Some(true)` Accept_Button (0xD72F002A), `Some(false)` Cancel_Button
/// (0x63857DE0), `None` neither yet. Left focuses Accept and plays LEFT
/// (column +0x30, BUTTON) unless Accept already has the focus; Right
/// likewise Cancel and RIGHT; Accept and Back play nothing (they start the
/// race or decline).
pub fn race_dialog(key: Key, accept_focused: Option<bool>) -> Option<u8> {
    match key {
        Key::Left if accept_focused != Some(true) => cell(event::LEFT, control::BUTTON),
        Key::Right if accept_focused != Some(false) => cell(event::RIGHT, control::BUTTON),
        _ => None,
    }
}

/// UI_EngageShopDialog (0x499690). Left / Right play LEFT / RIGHT on
/// every press (the focus moves only when it is elsewhere); Accept on
/// Cancel_Button plays BACK (both branches); Accept on Accept_Button
/// plays BACK only in a challenge's retry (0x501690() and 0x85B940, then
/// 0x5150D0), else enters the shop silently (0x499640); Back plays BACK
/// only in the retry (0x501640), else leaves silently.
pub fn shop_dialog(key: Key, accept_focused: Option<bool>, retry: bool) -> Option<u8> {
    match key {
        Key::Left => cell(event::LEFT, control::BUTTON),
        Key::Right => cell(event::RIGHT, control::BUTTON),
        Key::Accept if accept_focused == Some(false) || (accept_focused == Some(true) && retry) => cell(event::BACK, control::BUTTON),
        Key::Back if retry => cell(event::BACK, control::BUTTON),
        _ => None,
    }
}

/// UI_EngageEventDialog (0x498F00, a world challenge's offer; with no
/// challenge running, 0x863444 / 0x863450): Left / Right play LEFT /
/// RIGHT only when they move the focus; Accept on Cancel_Button plays
/// BACK, on Accept_Button SELECT; Back leaves silently.
pub fn event_dialog(key: Key, accept_focused: Option<bool>) -> Option<u8> {
    match key {
        Key::Left if accept_focused != Some(true) => cell(event::LEFT, control::BUTTON),
        Key::Right if accept_focused != Some(false) => cell(event::RIGHT, control::BUTTON),
        Key::Accept => match accept_focused {
            Some(false) => cell(event::BACK, control::BUTTON),
            Some(true) => cell(event::SELECT, control::BUTTON),
            None => None,
        },
        _ => None,
    }
}

/// UI_EngageMessageDialog (0x497B00; 0x544900 scrolls the text, no
/// sound): Accept (0x406415E3, state 2) plays table [BACK][0]; Back
/// (0x911AB364, back to the mailbox) and the arrows nothing.
pub fn message_dialog(key: Key) -> Option<u8> {
    match key {
        Key::Accept => cell(event::BACK, 0),
        _ => None,
    }
}

/// UI_SMS_Mailbox (0x4CCC10). Left / LTrigger (0x4B2110(−1)) and Right /
/// RTrigger (0x4CCDD1) change the mailbox and play UP on column 0 unless
/// at the first / last one; Up / Down (0x4CCB40) play UP / DOWN on column
/// 0 when they move (not off the first / last row; an empty list
/// nothing); Accept (state 2) nothing; Back BACK (column +0x30); Button1
/// on a row (the delete question) SELECT.
#[derive(Clone, Copy, Debug, Default)]
pub struct MailboxState {
    pub first_box: bool,
    pub last_box: bool,
    pub rows: usize,
    pub row: usize,
}

pub fn mailbox(key: Key, s: MailboxState) -> Option<u8> {
    match key {
        Key::Left | Key::LTrigger if !s.first_box => cell(event::UP, 0),
        Key::Right | Key::RTrigger if !s.last_box => cell(event::UP, 0),
        Key::Up if s.rows > 0 && s.row > 0 => cell(event::UP, 0),
        Key::Down if s.rows > 0 && s.row + 1 < s.rows => cell(event::DOWN, 0),
        Key::Back => cell(event::BACK, control::GENERIC),
        Key::Button1 if s.rows > 0 => cell(event::SELECT, control::GENERIC),
        _ => None,
    }
}

/// UI_CareerWorldMap (0x4E0F60). Up / Down (0x4CC140) play UP / DOWN on
/// column 7 unless on the first / last event; Left / Right (0x4E0E50)
/// change the stage and play LEFT / RIGHT on column 7 unless on the
/// first / last unlocked stage; Accept plays SELECT (column +0x30) unless
// prov: inferred audio.fe_rules.mailbox
/// 0x51F7F0() is 1 (**inferred**: no event to pick); Back BACK.
#[derive(Clone, Copy, Debug, Default)]
pub struct WorldMapState {
    pub event: usize,
    pub events: usize,
    pub first_stage: bool,
    pub last_stage: bool,
}

pub fn world_map(key: Key, s: WorldMapState) -> Option<u8> {
    match key {
        Key::Up if s.event > 0 => cell(event::UP, 7),
        Key::Down if s.event + 1 < s.events => cell(event::DOWN, 7),
        Key::Left if !s.first_stage => cell(event::LEFT, 7),
        Key::Right if !s.last_stage => cell(event::RIGHT, 7),
        Key::Accept if s.events > 0 => cell(event::SELECT, control::GENERIC),
        Key::Back => cell(event::BACK, control::GENERIC),
        _ => None,
    }
}

/// UI_QRTrackSelect (0x4EFA10): Accept (0x0C407210) 0x50D180(4); Back
/// 0x50D180(5) unless the 0xDE6EFF34 object is already in its state
/// (0x537D50); Left / Right step the track (0x4E3270 → 0x4E3390), whose
/// 0x51DE70(_, 0x18) reads row 24, which 0x50CF40 leaves at −1: nothing.
pub fn track_select(key: Key) -> Option<u8> {
    match key {
        Key::Accept => Some(4),
        Key::Back => Some(5),
        _ => None,
    }
}

// ---- Sound round 8 -------------------------------------------------------

/// FEng's sound notification (sound round 8). FEngine's pad pass
/// (0x54D4C0) hands each pad message to the focused object when it has a
/// response for it, else to the package when the package has one
/// (`PkgR`), and with either delivery queues the same message to target
/// −5; the queue (0x541FD0) passes those to the game interface's slot 7
/// (0x52CDD0 → 0x52CCA0 over the package list 0x8637B0), which runs
/// FEWidget_Handle 0x51DCE0 on the package's screen: with the screen's
/// +0x2C set it maps the message to a default event (Accept 8, Up 6, Down
/// 7, Back 10, Left 4, Right 5, Start 9, else 0), asks the screen's
/// vtable +0xC for the event to play and plays table[event][+0x30]. Most
/// screens' +0xC is 0x570A80 (returns 0: silent), and the list and icon
/// screens' constructors (0x552790, 0x552340) clear +0x2C.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    /// 0x49CB40 → 0x50D1A0(message, default, 1): Up / Down 0, the rest as
    /// given (Back 10). The engage race / shop / event dialogs.
    Buttons,
    /// 0x4E2280 (UI_QRCarSelect outside split screen): LTrigger 6,
    /// RTrigger 7, Accept 0, else 0x50D1A0(message, default, 1).
    QrCarSelect,
    /// 0x510EB0: Left / Right 0, else the default (UI_MagazineView).
    NoLeftRight,
}

/// FEWidget_Handle's default event for a pad message (0x51DCE0).
fn widget_default(key: Key) -> u8 {
    match key {
        Key::Accept => event::SELECT,
        Key::Up => event::UP,
        Key::Down => event::DOWN,
        Key::Back => event::BACK,
        Key::Left => event::LEFT,
        Key::Right => event::RIGHT,
        Key::Start => event::START,
        _ => 0,
    }
}

/// 0x50D1A0(message, event, flag): events below 6 as given, Up / Down
/// (6, 7) none, Back (10) only with the flag, the rest as given.
fn filter_50d1a0(ev: u8, flag: bool) -> u8 {
    match ev {
        0..=5 => ev,
        6 | 7 => 0,
        10 => if flag { 10 } else { 0 },
        _ => ev,
    }
}

/// The sound a delivered pad message's notification plays on a screen
/// with `filter` and control column `column` (`delivered`: the focused
/// object or the package has a response for the message).
pub fn notify(filter: Filter, key: Key, delivered: bool, column: u8) -> Option<u8> {
    if !delivered {
        return None;
    }
    let ev = widget_default(key);
    let ev = match filter {
        Filter::Buttons => filter_50d1a0(ev, true),
        Filter::QrCarSelect => match key {
            Key::LTrigger => event::UP,
            Key::RTrigger => event::DOWN,
            Key::Accept => 0,
            _ => filter_50d1a0(ev, true),
        },
        Filter::NoLeftRight => if matches!(ev, event::LEFT | event::RIGHT) { 0 } else { ev },
    };
    cell(ev, column)
}

/// UI_EngageRaceDialog / _ShopDialog / _EventDialog's notifications
/// (filter [`Filter::Buttons`], column BUTTON): their packages answer
/// Back (and the race and shop ones PAD_ACCEPT 0x406415E3), the buttons
/// 0x0C407210 — so Accept plays SELECT (id 4) with a button focused, or
/// with none on the race / shop dialog, and Back BACK (id 5). Left /
/// Right reach only the button that answers them (Cancel_Button Left,
/// Accept_Button Right), where the handler's own LEFT / RIGHT plays the
/// same id. `package_accept`: the package answers PAD_ACCEPT.
pub fn engage_notify(key: Key, accept_focused: Option<bool>, package_accept: bool) -> Option<u8> {
    let delivered = match key {
        Key::Accept => accept_focused.is_some() || package_accept,
        Key::Back => true,
        Key::Left => accept_focused == Some(false),
        Key::Right => accept_focused == Some(true),
        _ => false,
    };
    notify(Filter::Buttons, key, delivered, control::BUTTON)
}

/// UI_CareerCarLot (handler 0x4ED9E0 → 0x4AFFA0; constructor 0x4F9930
/// sets +0x30 = 12 CARLOT and +0x2C = 0): Left (0x9120409E) / Right
/// (0xB5971BF1) step the car and play LEFT / RIGHT unless on the first /
/// last (the list does not wrap); Accept (0x0C407210 / 0x406415E3, with
/// +0x1BC, set by the constructor) opens the question and plays SELECT;
/// Back (0x911AB364) opens the leave question silently; Up / Down are not
/// handled. Its question's Yes (0x51123592) plays BACK ([`LOT_LEAVE_YES`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct LotState {
    pub first: bool,
    pub last: bool,
}

pub fn car_lot(key: Key, s: LotState) -> Option<u8> {
    match key {
        Key::Left if !s.first => cell(event::LEFT, control::CARLOT),
        Key::Right if !s.last => cell(event::RIGHT, control::CARLOT),
        Key::Accept => cell(event::SELECT, control::CARLOT),
        _ => None,
    }
}

/// The car lot's leave question answered Yes (0x51123592 → 0x50D150(10,
/// +0x30)), and UI_CareerCarSelect's "drive instead" Yes (0xD05FC3A3).
pub const LOT_LEAVE_YES: u8 = 5;

/// UI_CareerCarSelect, the crib's Car Select (handler 0x4DFC50 →
/// 0x4B04C0; constructor at 0x4F9AFD, +0x30 = 12, +0x2C = 0): Left /
/// Right as the car lot's; Accept opens a dialog silently; Back plays BACK
/// (then leaves).
pub fn crib_select(key: Key, s: LotState) -> Option<u8> {
    match key {
        Key::Left if !s.first => cell(event::LEFT, control::CARLOT),
        Key::Right if !s.last => cell(event::RIGHT, control::CARLOT),
        Key::Back => cell(event::BACK, control::CARLOT),
        _ => None,
    }
}

/// UI_QRCarSelect (handler 0x4FC1F0, vtable 0x797714; +0x2C = 1, +0x30 =
/// 0 from 0x551DF0). The handler: once ready (+0x1A7, the package's
/// 0x35F8620B) Accept plays 0x50D180(4) and Back 0x50D180(5); Left /
/// Right (0x4E2590) and the triggers (0x4EED10) play nothing. Its
/// package answers Left, Right, both triggers, Back and Start, so the
/// notification ([`Filter::QrCarSelect`], column 0) plays LEFT 2 / RIGHT
/// 3 / UP 0 (LTrigger) / DOWN 1 (RTrigger) / BACK 5 / START 6 on every
/// press, ready or not. Button1 (Customize) plays SELECT only on the
/// paths that open a screen or the "limit" message.
pub fn qr_car_select(key: Key, ready: bool) -> Option<u8> {
    let own = match key {
        Key::Accept if ready => Some(4),
        Key::Back if ready => Some(5),
        _ => None,
    };
    let delivered = matches!(key, Key::Left | Key::Right | Key::LTrigger | Key::RTrigger | Key::Back | Key::Start);
    own.or(notify(Filter::QrCarSelect, key, delivered, control::GENERIC))
}

/// GarageMain's car swing (0x4D82D0, states 1 / 3 → 2: the loader's new
/// car with +0x2CE0, or +0x161 / +0x162 from the crib's car select):
/// 0x51DE40(5, 0x17) when +0x148 is 0 (the last step was Right), else
/// 0x51DE40(4, 0x17) — RIGHT 69 / LEFT 70 on the CARSEL column. +0x148
/// is set by UI_QRCarSelect's step (0x4E2590) and the crib's select
/// (0x4A7900 / 0x4A7930 / 0x4A7960); the constructor (0x4EACF3) and
/// nothing in UI_CareerCarLot leave it at 0.
pub fn garage_swing(left: bool) -> Option<u8> {
    cell(if left { event::LEFT } else { event::RIGHT }, control::CARSEL)
}

/// UI_InGame_WorldMap (handler 0x4E08E0; filter 0x570A80): with the GPS
/// off (0x4AE930) Left / Right step the marker (0x4B0850(3 / 2)) and play
/// 0x51DE40(LEFT / RIGHT, 0) when the selection changes; Up / Down
/// (0x4B1060) with a legend row focused (+0x5C) play 0x50D180(2) /
/// 0x50D180(3), none focused nothing; LTrigger / RTrigger (0x4CBDA0, the
/// zoom) 0x50D180(2) / (3); Accept (0x4B0EF0) and Button1 (0x4B10C0, the
/// GPS question) SELECT; Back BACK.
#[derive(Clone, Copy, Debug, Default)]
pub struct InGameMapState {
    pub gps: bool,
    /// Left / Right changed the selected marker.
    pub changed: bool,
    pub focused: bool,
}

pub fn in_game_map(key: Key, s: InGameMapState) -> Option<u8> {
    match key {
        Key::Left if !s.gps && s.changed => cell(event::LEFT, 0),
        Key::Right if !s.gps && s.changed => cell(event::RIGHT, 0),
        Key::Up if s.focused => Some(2),
        Key::Down if s.focused => Some(3),
        Key::LTrigger => Some(2),
        Key::RTrigger => Some(3),
        Key::Accept | Key::Button1 => cell(event::SELECT, 0),
        Key::Back => cell(event::BACK, 0),
        _ => None,
    }
}

/// UI_Status_Master (handler 0x4B1F20; filter 0x570A80): the package's
/// "ready" 0x50D180(15) ([`READY_RACE`]); Left / Right (0x4B1E60) change
/// the page and play LEFT / RIGHT on column 7 unless on the first / last;
/// Accept / Back play SELECT / BACK (+0x30) unless the screen opened this
/// tick (+0x100 = 0x8651A8), then close it; Up / Down nothing.
pub fn status(key: Key, page: usize, pages: usize) -> Option<u8> {
    match key {
        Key::Left if page > 0 => cell(event::LEFT, 7),
        Key::Right if page + 1 < pages => cell(event::RIGHT, 7),
        Key::Accept => cell(event::SELECT, 0),
        Key::Back => cell(event::BACK, 0),
        _ => None,
    }
}

/// UI_PostRaceStats (handler 0x4FBE20 → 0x4F3DE0; the list constructor
/// clears +0x2C): Left / Right step the racer (+0x184, clamped to 0 ..
/// racers − 1) and play 0x51DE70(_, LEFT / RIGHT) when it moves; Back
/// 0x51DE70(_, 10) and leaves; Accept nothing.
pub fn post_race_stats(key: Key, index: usize, racers: usize) -> Option<u8> {
    match key {
        Key::Left if index > 0 => cell(event::LEFT, 0),
        Key::Right if index + 1 < racers => cell(event::RIGHT, 0),
        Key::Back => cell(event::BACK, 0),
        _ => None,
    }
}

/// The generic dialogs (ShowDialog 0x558030 pushes the package with the
/// factory 0x5542B0 → screen 0x5541D0, vtable 0x79C894: handler 0x54C060,
/// filter 0x570A80; +0x30 = 0): Accept (0x0C407210 from a button,
/// 0xB8A7C6CC / CD / CE) plays SELECT with any button (+0x380 the count);
/// Left / Right play LEFT / RIGHT on every press with two buttons, Up /
/// Down UP / DOWN with three; Back plays BACK and answers the back
/// message only when the request allowed it (+0x384: 0x5587A0 / 0x558760
/// / 0x558580 yes, 0x558620 / 0x5585F0 / 0x558830 / 0x558870 no).
pub fn dialog(key: Key, buttons: usize, back: bool) -> Option<u8> {
    match key {
        Key::Accept if buttons > 0 => cell(event::SELECT, 0),
        Key::Left if buttons == 2 => cell(event::LEFT, 0),
        Key::Right if buttons == 2 => cell(event::RIGHT, 0),
        Key::Up if buttons == 3 => cell(event::UP, 0),
        Key::Down if buttons == 3 => cell(event::DOWN, 0),
        Key::Back if back => cell(event::BACK, 0),
        _ => None,
    }
}

/// UI_Paint (constructor 0x56AEC0 sets +0x30 = 10 PAINT; handler 0x563420
/// — the four call sites round 7 put under 0x5632A0 are this handler's).
/// On the slot strip (+0x188 clear) IconScrollerMenu 0x543D50 handles the
/// key: Left / Right LEFT / RIGHT (2 / 3) off the first / last slot,
/// Accept SELECT (or WRONG on a locked slot), Back BACK; Up / Down and the
/// triggers nothing. In a slot's swatches (+0x188): Up / Down UP / DOWN
/// on PAINT (39 / 40), Left / Right LEFT / RIGHT on PAINT_TYPE (41 / 42),
/// each press; Accept SELECT with swatches (+0x158); the triggers
/// (0x55D0E0, the finish) LEFT / RIGHT on PAINT_TYPE when the finish
/// (+0x174) is gloss, metallic or pearl; Back nothing (0x55CEA0).
#[derive(Clone, Copy, Debug, Default)]
pub struct PaintState {
    pub in_slot: bool,
    pub first: bool,
    pub last: bool,
    pub swatches: bool,
    pub finish_cycles: bool,
}

pub fn paint(key: Key, s: PaintState) -> Option<u8> {
    if !s.in_slot {
        return match key {
            Key::Left if !s.first => cell(event::LEFT, control::PAINT),
            Key::Right if !s.last => cell(event::RIGHT, control::PAINT),
            Key::Accept => cell(event::SELECT, control::PAINT),
            Key::Back => cell(event::BACK, control::PAINT),
            _ => None,
        };
    }
    match key {
        Key::Up => cell(event::UP, control::PAINT),
        Key::Down => cell(event::DOWN, control::PAINT),
        Key::Left | Key::LTrigger if key == Key::Left || s.finish_cycles => cell(event::LEFT, control::PAINT_TYPE),
        Key::Right | Key::RTrigger if key == Key::Right || s.finish_cycles => cell(event::RIGHT, control::PAINT_TYPE),
        Key::Accept if s.swatches => cell(event::SELECT, control::PAINT),
        _ => None,
    }
}

/// UI_GenericParts_Browser (handler 0x5664A0; filter 0x570A80). The
/// category strip (+0x4C): Left / Right (0x54FA00) LEFT / RIGHT unless on
/// the first / last; Accept on a locked item WRONG (0x8381E1, id 7), else
/// with items SELECT — entering a category (0x55EDE0 id 4), the rims or
/// carbon screens (0x53A320 / 0x53A3E0 id 4) or a question (SELECT on
/// +0x30); Back outside Customize (0x83898C = 0) the leave question
/// silently, in it BACK. In a category (+0x1D9): Up / Down (0x55EBB0) UP /
/// DOWN unless on the first / last part; Accept a question with SELECT;
/// Back 0x55EF90 id 5.
#[derive(Clone, Copy, Debug, Default)]
pub struct BrowserState {
    pub in_category: bool,
    pub first: bool,
    pub last: bool,
    pub locked: bool,
    pub empty: bool,
    pub customize: bool,
}

pub fn parts_browser(key: Key, s: BrowserState) -> Option<u8> {
    if s.in_category {
        return match key {
            Key::Up if !s.first => cell(event::UP, 0),
            Key::Down if !s.last => cell(event::DOWN, 0),
            Key::Accept if s.locked => cell(event::WRONG, 0),
            Key::Accept if !s.empty => Some(4),
            Key::Back => Some(5),
            _ => None,
        };
    }
    match key {
        Key::Left if !s.first => cell(event::LEFT, 0),
        Key::Right if !s.last => cell(event::RIGHT, 0),
        Key::Accept if s.locked => cell(event::WRONG, 0),
        Key::Accept if !s.empty => Some(4),
        Key::Back if s.customize => cell(event::BACK, 0),
        _ => None,
    }
}

/// UI_Options_PC_Controller (handler 0x4F9340; filter 0x570A80) with a
/// row current (+0x64): Up 0x4ED380(−1) and UP on column 7 unless on the
/// first row (which drops the row, silently); Down 0x4ED380(1) and DOWN
/// on column 7; Left / Right (0x4CB9E0) LEFT / RIGHT on column 7; Accept
/// starts the binding (the row's +4) and Back leaves, both silently.
pub fn controls(key: Key, row: usize) -> Option<u8> {
    match key {
        Key::Up if row > 0 => cell(event::UP, 7),
        Key::Down => cell(event::DOWN, 7),
        Key::Left => cell(event::LEFT, 7),
        Key::Right => cell(event::RIGHT, 7),
        _ => None,
    }
}

/// EA_Trax_Jukebox (handler 0x56B1D0; constructor 0x553C80, +0x108 = 0,
/// +0x30 = 0; filter 0x570A80): Up / Down outside the reorder mode
/// (+0x108 clear: 0x550E70) nothing; Left / Right (0x53DEC0, the song's
/// state) table [LEFT][2] / [RIGHT][2] (2 / 3) on every press; LTrigger
/// / RTrigger (0x5241F0, the play mode) [LTRIGGER][0] / [RTRIGGER][0] (2
/// / 3); PAD_ACCEPT (0x406415E3, the preview) SELECT; Back BACK; Button1
/// (0x531640, the reorder mode) table [SELECT][+0x30].
pub fn jukebox(key: Key) -> Option<u8> {
    match key {
        Key::Left => cell(event::LEFT, control::TOGGLE),
        Key::Right => cell(event::RIGHT, control::TOGGLE),
        Key::LTrigger => cell(event::LTRIGGER, 0),
        Key::RTrigger => cell(event::RTRIGGER, 0),
        Key::Accept | Key::Button1 => cell(event::SELECT, 0),
        Key::Back => cell(event::BACK, 0),
        _ => None,
    }
}

/// UI_ShowCase_Preview's menu state (0x4CEA70): Up / Down UP / DOWN when
/// they move (+0x54 within 0..3, 0x4CEB54 / 0x4CEC36); Accept on Control
/// Car (0x4B5C70: SELECT at 0x4B5C83, then EngageState_Run's BACK, played
/// by `showcase_key`), Cycle Cameras (0x4B5EA0, 0x4B5EAD) and Take Picture
/// (0x4CF460, 0x4CF46D) SELECT; on Show Off (0x4B5D80) no SELECT, only
/// EngageState_Run's BACK (0x4B5E71); Back is not handled (silent).
pub fn showcase_menu(key: Key, option: usize) -> Option<u8> {
    match key {
        Key::Up if option > 0 => cell(event::UP, 0),
        Key::Down if option < 3 => cell(event::DOWN, 0),
        Key::Accept if option != 1 => cell(event::SELECT, 0),
        _ => None,
    }
}

/// The picture state (0x4CEDC0, sound round 9): Accept (0x406415E3)
/// SELECT on +0x30 (0x4CEEA5), then the save question; Back BACK
/// (0x4CEF20, with +0x9C and +0x99 clear) and the menu again; the
/// directions go to 0x4B57B0 only when 0x50D510 answers, which does
/// nothing: silent.
pub fn showcase_picture(key: Key) -> Option<u8> {
    match key {
        Key::Accept => cell(event::SELECT, 0),
        Key::Back => cell(event::BACK, 0),
        _ => None,
    }
}

/// UI_MagazineView (handler 0x565B60, no call sites; constructor
/// 0x553BF0 keeps +0x2C = 1, filter [`Filter::NoLeftRight`]): its package
/// answers PAD_ACCEPT, Back and the triggers, so Accept plays SELECT and
/// Back BACK; the triggers' default event is 0.
pub fn cover(key: Key) -> Option<u8> {
    notify(Filter::NoLeftRight, key, matches!(key, Key::Accept | Key::Back | Key::LTrigger | Key::RTrigger), 0)
}

/// UI_Options in the Player category (builder 0x4B7CD0 sets +0x14E; the
/// constructor 0x4CF750 sets +0x2C = 1): its filter 0x4CFAC0 plays
/// 0x50D180(11) for every notification outside game mode 8 and returns
/// 0. The package answers PAD_ACCEPT, Back, Button1, Button2, Up and
/// Down; every row (option_name_1…9, 0xB2980BA8…B0, the focused object)
/// answers 0x0C407210, Left and Right (sound round 9), so each of those
/// keys plays 11 on any row.
pub fn options_player_notify(key: Key) -> Option<u8> {
    matches!(key, Key::Accept | Key::Back | Key::Button1 | Key::Up | Key::Down | Key::Left | Key::Right).then_some(11)
}

/// The screens whose handlers have no call sites but whose filters answer
/// (sound round 8; all keep +0x2C = 1 from 0x551DF0 and +0x30 = 0, so the
/// sound is the notification's alone; `delivered` from their packages'
/// `PkgR` and their objects' responses):
/// * UI_PostRaceReward (vtable 0x7950D8, filter 0x4A4750 → 0x50D230: the
///   directions 0): Accept SELECT, Back BACK.
/// * UI_SponsorPopup (0x790624, 0x49DFC0 → 0x50D1D0: Left / Right 0),
///   UI_PerformanceDyno_Results (0x79CDFC, 0x510EB0), MC_List (0x79647C,
///   0x49DFC0): Accept SELECT, Back BACK, Up / Down UP / DOWN where the
///   package answers them (MC_List).
/// * UI_PostRaceResults (0x797A94, 0x4A3F00: Back 0, Button1 8, else
///   0x50D230): Accept and Button1 SELECT, Back nothing.
/// * Credits (0x78FAC0, 0x49CA50 → 0x50D1F0: Back only): Back BACK.
/// * UI_PerformanceTuning_Graph (0x79D37C, 0x5102F0: the directions and
///   Back as given, Button1 10, else 0): Up / Down / Left / Right, Back and
///   Button1 (BACK).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum QuietScreen {
    Reward,
    SponsorPopup,
    DynoResults,
    McList,
    Results,
    Credits,
    TuneGraph,
}

pub fn quiet_screen(screen: QuietScreen, key: Key) -> Option<u8> {
    use QuietScreen::*;
    let ev = match (screen, key) {
        (Reward | SponsorPopup | DynoResults | McList | Results, Key::Accept) => event::SELECT,
        (Reward | SponsorPopup | DynoResults | McList, Key::Back) => event::BACK,
        (McList, Key::Up) => event::UP,
        (McList, Key::Down) => event::DOWN,
        (Results, Key::Button1) => event::SELECT,
        (Credits, Key::Back) => event::BACK,
        (TuneGraph, Key::Up) => event::UP,
        (TuneGraph, Key::Down) => event::DOWN,
        (TuneGraph, Key::Left) => event::LEFT,
        (TuneGraph, Key::Right) => event::RIGHT,
        (TuneGraph, Key::Back | Key::Button1) => event::BACK,
        _ => return None,
    };
    cell(ev, 0)
}

/// UG_LS_Splash (handler 0x4C5580; filter 0x570A80): Start (0xB5AF2461)
/// and PAD_ACCEPT, once the screen is ready (0x836495), play
/// 0x51DE70(_, 9) — [START][0], id 6 — and go on (0x4C4F50).
pub fn splash(key: Key, ready: bool) -> Option<u8> {
    match key {
        Key::Accept | Key::Start if ready => cell(event::START, 0),
        _ => None,
    }
}

// ---- Sound round 9 -------------------------------------------------------

/// MC_Main's button layout (its filter 0x4A33D0 reads +0x64 "buttons
/// up" and the mode +0x8C). 0x4A3890 (the Yes / No of MC_Main's states 1,
/// 2 and 5, 0x4E8D30 / 0x4A3C10) and 0x4A35C0 with two buttons set mode 1;
/// 0x4A35C0 with three mode 2, with one mode 3 (the memory-card library's
/// messages, 0x4C6050 by their button count); 0x4A39C0 (state 0x7000000)
/// shows one button and keeps the mode; 0x4A3AE0 clears +0x64, 0x4A3C80
/// clears both; the constructor 0x4E8B70 starts at 0 / 0. No writer sets
/// mode 4 (0x50D1F0, Back only).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum McButtons {
    /// +0x64 clear: the filter returns 0 for every message.
    Hidden,
    /// Mode 1, 0x50D1A0(msg, default, 1): Up / Down 0.
    Two,
    /// Mode 2, 0x50D1D0: Left / Right 0.
    Three,
    /// Mode 3, 0x50D210: only PAD_ACCEPT (0x406415E3).
    One,
}

/// MC_Main and the memory-card messages it shows (screen 0x4E8B70,
/// vtable 0x797A5C; base 0x552AA0 → 0x551DF0 keeps +0x2C = 1, +0x30 = 0;
/// no handler call sites, so the sound is FEng's notification alone). The
/// package (`PkgR`) answers PAD_ACCEPT, Back, Left and Right; the buttons
/// (0xB8A7C6CC … D1) answer 0x0C407210 (Accept to the focused object),
/// "Yes" (CC) Right and "No" (CD) Left, the three-button set Up / Down. So
/// with buttons up Accept plays SELECT (0x0C407210 → 8 before the mode
/// switch); two buttons: Left / Right LEFT / RIGHT and Back BACK (the
/// screen ignores Back, the sound still plays), Up / Down nothing; three:
/// Up / Down UP / DOWN where the focused button answers them, Back BACK;
/// one: Accept only. Start reaches no response.
pub fn mc_main(key: Key, buttons: McButtons) -> Option<u8> {
    use McButtons::*;
    let ev = match (buttons, key) {
        (Hidden, _) => return None,
        (_, Key::Accept) => event::SELECT,
        (Two, Key::Left) => event::LEFT,
        (Two, Key::Right) => event::RIGHT,
        (Two | Three, Key::Back) => event::BACK,
        (Three, Key::Up) => event::UP,
        (Three, Key::Down) => event::DOWN,
        _ => return None,
    };
    cell(ev, 0)
}

/// UI_PerformanceDyno_Chart (vtable 0x79AC68: handler 0x54A810, filter
/// 0x5103B0; constructor 0x557A30 → 0x551DF0, +0x2C = 1, +0x30 = 0). +0x4D
/// is set when the chart opens for a run (the push's argument +0x10 = 0,
/// 0x557AFA, with the dyno started) and cleared when the run's progress
/// reaches 1 (0x54A87F, then the record 0x523340) or when it reopens from
/// the results (0x537990(…, 1): +0x4D = 0 at 0x557B52). While set the
/// filter returns 0; after it Left / Right 0, else the default. The package
/// answers PAD_ACCEPT and Button1 (0xC519BFC0, default 0); Back is answered
// prov: inferred audio.fe_rules.dyno_chart
/// only by the "Event Handler" object (**inferred**: never focused, so not
/// delivered). `running`: +0x4D.
pub fn dyno_chart(key: Key, running: bool) -> Option<u8> {
    if running {
        return None;
    }
    notify(Filter::NoLeftRight, key, key == Key::Accept, 0)
}

/// The shop browsers built on an IconScrollerMenu strip with a list
/// beside it (sound round 9, each handler read key by key; all +0x30 = 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StripShop {
    /// UI_Rims_Browser (handler 0x567A60, constructor 0x5678F0 → 0x552340):
    /// the styles strip and, with +0x214 (0x55FB10 sets it and clears the
    /// strip's +0x7B), the sizes list.
    Rims,
    /// UI_ICEMAIN (0x568750) and its list UI_IcePartsOverlay (0x568D40),
    /// +0x169 = in the list.
    Ice,
    /// UI_CustomNeonMain (0x56A890) and UI_NeonPartsOverlay (0x56A250…),
    /// +0x159 = in the list.
    Neon,
    /// UI_DecalMain (0x56B3C0) and its overlay (0x563F00…), +0x174 = in
    /// the overlay.
    Decal,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StripState {
    /// In the list (sizes, parts overlay).
    pub in_list: bool,
    /// At the first / last strip item or list row.
    pub first: bool,
    pub last: bool,
    /// The strip item is locked (+0x22).
    pub locked: bool,
    /// Accept opens the item's list (ICE: 0x556940's id 4).
    pub opens_list: bool,
}

/// The strip (IconScrollerMenu 0x543D50, column +0x30): Left / Right (the
/// strip horizontal, +0x79) LEFT / RIGHT unless at the first / last
/// (0x5276E0 / 0x527710); Up / Down nothing; 0x0C407210 WRONG on a locked
/// item (0x13), else SELECT (8); Back BACK (10). The rims handler takes
/// PAD_ACCEPT itself (0x55FB10 then 0x51DE40(8, +0x30): one SELECT). In the
/// list the strip is dimmed (+0x7B = 0) or not called (+0x169 / +0x159 /
/// +0x174), so Left / Right are silent — no screen moves the strip from
/// inside its list (UI_GenericParts_Browser's 0x54FA00 also returns while
/// +0x1D9 is set). The lists: Up / Down UP / DOWN unless at the first /
/// last row (rims 0x55F750, ICE 0x560930, neon 0x55CAD0, decals 0x55D670);
/// Accept: rims SELECT (the buy question, 0x51DE40(8)), ICE / neon / decals
/// the question or the fit silently (the install sound is
/// FE_PlayInstallSound's); Back: rims 0x55FCF0 id 5, ICE 0x5605C0 id 5
/// (the overlay forwards Back to UI_ICEMAIN), decals id 5 (0x9FBBD1D4 →
// prov: inferred audio.fe_rules.strip_shop
/// 0x56B7EF; **inferred**: the overlay's Back sends it), neon silent
/// (0x562B20).
pub fn strip_shop(shop: StripShop, key: Key, s: StripState) -> Option<u8> {
    if s.in_list {
        return match key {
            Key::Up if !s.first => cell(event::UP, 0),
            Key::Down if !s.last => cell(event::DOWN, 0),
            Key::Accept if shop == StripShop::Rims && !s.locked => cell(event::SELECT, 0),
            Key::Back if shop != StripShop::Neon => Some(5),
            _ => None,
        };
    }
    match key {
        Key::Left if !s.first => cell(event::LEFT, 0),
        Key::Right if !s.last => cell(event::RIGHT, 0),
        Key::Accept if s.locked => cell(event::WRONG, 0),
        Key::Accept => cell(event::SELECT, 0),
        Key::Back => cell(event::BACK, 0),
        _ => None,
    }
}

/// UI_ICEMAIN's second sound on Accept: after the strip's SELECT its
/// handler opens the list through 0x556940, which plays id 4 itself (two
/// id-4 sounds at once).
pub fn strip_shop_extra(shop: StripShop, key: Key, s: StripState) -> Option<u8> {
    (shop == StripShop::Ice && key == Key::Accept && !s.in_list && !s.locked && s.opens_list).then_some(4)
}

/// CarPart_RequestAnim 0x61F830(car, channel, open, speed)'s sound, played
/// before the animation starts (0x433E20). In the front end ([0x82B884]
/// +0x90 = 1, set by the FE load 0x486B30; the game's load 0x486C10 sets
/// 2–5) PlayFESound by 0x50D180: hood (channel 0) open 0x51 / shut 0x4D,
/// trunk (1) 0x53 / 0x52, doors (2; channel 3 none) 0x4C / 0x4F with the
/// door style part (+0x810) attribute ANIMSTYLE = 1, else 0x50 / 0x4E.
/// In game the FE_COMMON player's 0x481500(n) starts collision-list
/// stitch record n ([`part_stitch`]).
pub fn part_sound_fe(channel: usize, open: bool, animstyle_one: bool) -> Option<u8> {
    Some(match (channel, open) {
        (0, true) => 0x51,
        (0, false) => 0x4D,
        (1, true) => 0x53,
        (1, false) => 0x52,
        (2, true) => if animstyle_one { 0x4C } else { 0x50 },
        (2, false) => if animstyle_one { 0x4F } else { 0x4E },
        _ => return None,
    })
}

/// The in-game record for 0x61F830 (0x481500's n): hood 3 / 2, trunk 5 / 4,
/// doors (channel 2) 1 / 0 for both door styles, channel 3 none.
pub fn part_stitch(channel: usize, open: bool) -> Option<usize> {
    Some(match (channel, open) {
        (0, true) => 3,
        (0, false) => 2,
        (1, true) => 5,
        (1, false) => 4,
        (2, true) => 1,
        (2, false) => 0,
        _ => return None,
    })
}

/// UI_CareerCarSelect's swing delay (0x4DFC50's tick at 0x4DFFEC): each
/// Left / Right (0x4B04C0) stores the time (+0xB8 = 0x8651A8); the first
/// tick at least 0.3 s (0x78758C; ticks × 0.00025, 0x784268) later starts
/// the swing (0x4A7930 → +0x161, or 0x4A7960 → 0x0E550F66 → +0x162, or a
/// new car: 0x4A7900 and the load 0x4D8EC0) by the last key (+0x1A8), and
/// GarageMain's next update plays its sound (0x4D836B / 0x4D83ED).
pub const CRIB_SWING_DELAY: f32 = 0.3;
