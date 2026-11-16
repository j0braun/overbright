//! The keyboard map: default keys, action rows, binding.

use super::*;

/// The game's default keyboard map (DIACTION table 0x7FCAC0, 0x28 bytes an
/// entry: action id, then the DIKEYBOARD_ semantic 0x81000400 | DIK; the
/// first entry of an action is its primary key, 0x5B8DA0) by Controls row,
/// each row's action being its vtable's (0x4DBA50): Steering Left 3, Right
/// 4, Throttle 1, Back / Reverse 2, Shift Up 7, Shift Down 8, Handbrake 5,
/// Nitrous 6, Camera 10, Look Back 9, Enter Race / Shop 15, World Map 16,
/// Read SMS 17, Career Statistics 18, Reset Car 14, Hydraulics Pressure 22,
/// Hydraulics Bounce 19, Pause 12. The names are DirectInput's English key
// prov: inferred frontend.options.DEFAULT_KEYS
/// names (**inferred**: the OS supplies them), "Shift" / "Alt" / "Ctrl"
/// shown as "Left …" in English (0x5B8DA0).
pub const DEFAULT_KEYS: [[&str; 2]; 18] = [
    ["Left", "A"],
    ["Right", "D"],
    ["Up", "W"],
    ["Down", "S"],
    ["Left Shift", ";"],
    ["Left Ctrl", "."],
    ["Space", "End"],
    ["Left Alt", "Right Alt"],
    ["C", "Insert"],
    ["B", "K"],
    ["Enter", "F1"],
    ["M", "F2"],
    ["Tab", "F3"],
    ["T", "F4"],
    ["R", "Home"],
    ["H", "Y"],
    ["J", "U"],
    ["Esc", "P"],
];

/// Controls rows the clone's driving reads.
pub mod action {
    pub const STEER_LEFT: usize = 0;
    pub const STEER_RIGHT: usize = 1;
    pub const THROTTLE: usize = 2;
    pub const BRAKE: usize = 3;
    pub const SHIFT_UP: usize = 4;
    pub const SHIFT_DOWN: usize = 5;
    pub const HANDBRAKE: usize = 6;
    pub const NITROUS: usize = 7;
    pub const CAMERA: usize = 8;
    /// Enter Race / Shop (DIACTION 15): the descriptor row 46 (0x7FE2D0)
    /// sends it as "ENGAGE", code 0xBF (JOY_EVENT_REQUEST_ENGAGE_INFO in the
    /// player's event handler 0x602BE0): it engages the current marker and
    /// is an outrun's challenge key (the help text 0x9FD2C234).
    pub const ENGAGE: usize = 10;
    /// World Map (DIACTION 16): row 47 sends WORLD_MAP, code 0xC0, which
    /// in career free roam opens UI_InGame_WorldMap (0x602EF5).
    pub const WORLD_MAP: usize = 11;
    /// Read SMS (DIACTION 17, Tab / F3): descriptor row 49 sends code 0xC2
    /// (0x602BE0 → 0x501480, game state 0xD).
    pub const READ_SMS: usize = 12;
    /// Career Statistics (DIACTION 18, T / F4): row 48 sends LEGEND, code
    /// 0xC1, which in career free roam opens UI_Status_Master (state 0x13,
    /// 0x6068D5).
    pub const CAREER_STATS: usize = 13;
    pub const RESET: usize = 14;
    pub const PAUSE: usize = 17;
}

impl Settings {
    /// Bind key `name` to Controls row `row`'s `column` (0 primary, 1
    /// secondary) the way 0x5B8560 does once the keyboard reports it: the
    /// key leaves every entry that had it (any action, either column),
    /// goes in the column asked for, then a primary left empty takes the
    /// secondary's key, and a secondary equal to the primary is cleared.
    /// An empty name is an unbound entry.
    pub fn bind(&mut self, row: usize, column: usize, name: &str) {
        for k in self.keys.iter_mut().flatten() {
            if k == name {
                k.clear();
            }
        }
        let pair = &mut self.keys[row];
        pair[column.min(1)] = name.to_string();
        if pair[0].is_empty() {
            pair[0] = pair[1].clone();
        }
        if pair[0] == pair[1] {
            pair[1].clear();
        }
    }
}
