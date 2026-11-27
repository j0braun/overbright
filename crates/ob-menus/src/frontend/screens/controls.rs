use crate::frontend::fe_ids;
use super::*;

/// UI_Options_PC_Controller (Options' Controls, category 4; screen
/// 0x4ECD10): title "Controls" (0x04DC6DB5), header "Keyboard
/// Configuration" (0x5CF08EB7), "Primary" / "Secondary" over the key
/// columns and "Configure your settings" (0x8BAB66EF); 0x4DBA50 builds a
/// row per action, its name from the row's vtable slot 3 and each key from
/// the input map 0x874C40 (+0x370 × action; 0x4ACF40 names it, 0x5C20F0
/// → 0x5B8DA0 for the keyboard: the DirectInput object's name of the
/// action's first or second DIACTION). Rows: the name hash and the action
/// id (the defaults are `options::DEFAULT_KEYS`).
pub const CONTROL_ROWS: [(u32, u8); 18] = [
    (fe_ids::CONTROL_STEERING_LEFT, 3),  // Steering (Left)
    (fe_ids::CONTROL_STEERING_RIGHT, 4),  // Steering (Right)
    (fe_ids::CONTROL_THROTTLE, 1),  // Throttle
    (fe_ids::CONTROL_BACK_REVERSE, 2),  // Back / Reverse
    (fe_ids::CONTROL_SHIFT_UP, 7),  // Shift Up
    (fe_ids::CONTROL_SHIFT_DOWN, 8),  // Shift Down
    (fe_ids::CONTROL_HANDBRAKE, 5),  // Handbrake
    (fe_ids::CONTROL_NITROUS_BOOST, 6),  // Nitrous Boost
    (fe_ids::CONTROL_CAMERA, 10), // Camera
    (fe_ids::CONTROL_LOOK_BACK, 9),  // Look Back
    (fe_ids::CONTROL_ENTER_RACE_SHOP, 15), // Enter Race / Shop
    (fe_ids::CONTROL_WORLD_MAP, 16), // World Map
    (fe_ids::CONTROL_READ_SMS, 17), // Read SMS
    (fe_ids::CONTROL_CAREER_STATISTICS, 18), // Career Statistics
    (fe_ids::CONTROL_RESET_CAR, 14), // Reset Car
    (fe_ids::CONTROL_HYDRAULICS_PRESSURE, 22), // Hydraulics Pressure
    (fe_ids::CONTROL_HYDRAULICS_BOUNCE, 19), // Hydraulics Bounce
    (fe_ids::CONTROL_PAUSE, 12), // Pause
];

#[derive(Clone, Debug)]
pub struct ControlsScreen {
    pub selected: usize,
    pub top: usize,
    pub opened: f32,
    /// The key column: 0 primary, 1 secondary (0x5C2110's slot, map
    /// +0x340).
    pub column: usize,
    /// Waiting for a key to bind (0x5C2110 → 0x5B9130: the keyboard set
    /// to report the next key press).
    pub capturing: bool,
    /// When the capture began (the screen's clock), for the cell's Pulse.
    pub capture_at: f32,
    /// A key was bound: the instructions read "Press enter to assign a new
    /// key or button" (0xFED0D3DF, 0x4F982B once +0x14F is set by the
    /// capture's end, 0x4F989A).
    pub bound: bool,
    /// The device shown (0x8364EC into the list 0x4ACE00 builds: the input
    /// records with a channel, the keyboard first): 0 the keyboard, k the
    /// k-th controller.
    pub device: usize,
    /// The header is selected (+0x64 = 0: Up on the first row, 0x4F9434;
    /// Down takes the first row again, 0x4F954A); there Left / Right change
    /// the device (0x4ACAA0).
    pub header: bool,
    /// Restore Defaults' OK is pending (the dialog's outcome).
    pub restore: bool,
}

/// The Controls page a device gets (0xAF0BBD92 → by 0x5B8730's kind:
/// 0x4DBA50 the keyboard, 0x4DC9F0 a joypad, 0x4DDC10 a wheel; the same 18
/// rows each, only the header's string differs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlsPage {
    Keyboard,
    Joypad,
    Wheel,
}

impl ControlsPage {
    /// The header string (0x4DBA72 0x5CF08EB7 "Keyboard Configuration",
    /// 0x4DCA12 0xF91FBA4D, 0x4DDC32 0x14D4BD3B).
    pub fn title(self) -> u32 {
        match self {
            ControlsPage::Keyboard => 0x5CF0_8EB7,
            ControlsPage::Joypad => 0xF91F_BA4D,
            ControlsPage::Wheel => 0x14D4_BD3B,
        }
    }
}

/// UI_Options_PC_Controller's rows shown at once.
pub fn controls_visible() -> usize {
    controls::VISIBLE
}

/// The list's masters: the name column and the two key columns.
pub const NAME_MASTER: u32 = fe_ids::TITLE_MASTER;
pub const PRIMARY_MASTER: u32 = fe_ids::DATA_MASTER;
pub const SECONDARY_MASTER: u32 = fe_ids::SECONDARY_MASTER;
/// Rows shown (**inferred**: the masters' column down to the
/// instructions).
pub const VISIBLE: usize = 7;
/// The row strings: name, primary and secondary, for rows 1–10 and
/// 11–18 (the package numbers them in two runs each).
pub const NAMES: [u32; 18] = [
    fe_ids::NAMES_0,
    fe_ids::NAMES_1,
    fe_ids::NAMES_2,
    fe_ids::NAMES_3,
    fe_ids::NAMES_4,
    fe_ids::NAMES_5,
    fe_ids::NAMES_6,
    fe_ids::NAMES_7,
    fe_ids::NAMES_8,
    fe_ids::NAMES_9,
    fe_ids::NAME,
    fe_ids::NAMES_11,
    fe_ids::NAMES_12,
    fe_ids::NAMES_13,
    fe_ids::NAMES_14,
    fe_ids::NAMES_15,
    fe_ids::NAMES_16,
    fe_ids::NAMES_17,
];
pub const PRIMARY: [u32; 18] = [
    fe_ids::PRIMARY_0,
    fe_ids::PRIMARY_1,
    fe_ids::PRIMARY_2,
    fe_ids::PRIMARY_3,
    fe_ids::PRIMARY_4,
    fe_ids::PRIMARY_5,
    fe_ids::PRIMARY_6,
    fe_ids::PRIMARY_7,
    fe_ids::PRIMARY_8,
    fe_ids::PRIMARY_9,
    fe_ids::DATA,
    fe_ids::PRIMARY_11,
    fe_ids::PRIMARY_12,
    fe_ids::PRIMARY_13,
    fe_ids::PRIMARY_14,
    fe_ids::PRIMARY_15,
    fe_ids::PRIMARY_16,
    fe_ids::PRIMARY_17,
];
pub const SECONDARY: [u32; 18] = [
    fe_ids::SECONDARY_0,
    fe_ids::SECONDARY_1,
    fe_ids::SECONDARY_2,
    fe_ids::SECONDARY_3,
    fe_ids::SECONDARY_4,
    fe_ids::SECONDARY_5,
    fe_ids::SECONDARY_6,
    fe_ids::SECONDARY_7,
    fe_ids::SECONDARY_8,
    fe_ids::SECONDARY_9,
    fe_ids::SECONDARY_10,
    fe_ids::SECONDARY_11,
    fe_ids::SECONDARY_12,
    fe_ids::SECONDARY_13,
    fe_ids::SECONDARY_14,
    fe_ids::SECONDARY_15,
    fe_ids::SECONDARY_16,
    fe_ids::SECONDARY_17,
];

/// Half a one-line string object's height on the canvas (its font's line
/// step times its scale), for placing a vertically centred string by its
/// top edge.
fn half_height(assets: &Assets, package: &crate::frontend::feng::Package, hash: u32) -> f32 {
    let Some(o) = package.object(hash) else { return 0.0 };
    let Some(font) = package.resources.get(o.resource as usize).and_then(|r| assets.font_for(&r.name)) else {
        return 0.0;
    };
    (o.leading as f32 + font.line_height) * o.attributes.size().y * 0.5
}

/// A key cell's text: the key's name, or the game's "---" for an unbound
/// entry (0x4ACF40 draws the string at 0x790B94 when 0x5B8DA0 returns "").
pub(super) fn key_cell(name: &str) -> String {
    if name.is_empty() { "---".into() } else { name.into() }
}

impl ControlsScreen {
    pub fn new(opened: f32) -> ControlsScreen {
        ControlsScreen {
            selected: 0,
            top: 0,
            opened,
            column: 0,
            capturing: false,
            capture_at: 0.0,
            bound: false,
            device: 0,
            header: false,
            restore: false,
        }
    }

    /// A key; `devices` the list's length (the keyboard and the attached
    /// controllers). True when the device changed.
    pub fn key(&mut self, key: Key, now: f32, devices: usize) -> bool {
        let n = CONTROL_ROWS.len();
        let was = self.device;
        match key {
            Key::Up if self.selected == 0 => self.header = true,
            Key::Down if self.header => self.header = false,
            Key::Up => self.selected -= 1,
            Key::Down => self.selected = (self.selected + 1).min(n - 1),
            // The header: the previous / next device, wrapping (0x4ACAA0).
            Key::Left if self.header => self.device = (self.device + devices.max(1) - 1) % devices.max(1),
            Key::Right if self.header => self.device = (self.device + 1) % devices.max(1),
            // **Inferred**: Left / Right pick the column, Accept binds.
            Key::Left => self.column = 0,
            Key::Right => self.column = 1,
            // Accept on a row (0x4F93CC: none on the header).
            Key::Accept if !self.header => {
                self.capturing = true;
                self.capture_at = now;
            }
            _ => {}
        }
        if self.selected < self.top {
            self.top = self.selected;
        } else if self.selected >= self.top + controls::VISIBLE {
            self.top = self.selected + 1 - controls::VISIBLE;
        }
        self.device != was
    }

    /// `keys` the cells' names by row (the keyboard's keys, or a
    /// controller's buttons and axes, 0x5C20F0), `page` the device's
    /// page, `devices` the list's length.
    pub fn quads(
        &self,
        assets: &Assets,
        keys: &[[String; 2]; 18],
        page: ControlsPage,
        devices: usize,
        now: f32,
    ) -> Vec<Quad> {
        use controls::*;
        let Some(package) = assets.package("UI_Options_PC_Controller") else { return Vec::new() };
        let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        let mut state = ScreenState::default();
        footer(&mut state, package, "Select");
        state.text.insert(fe_ids::UI_MAIN_TITLE, label(fe_ids::TXT_CONTROLS));
        state.text.insert(fe_ids::OBJ_4E11CF90, label(page.title()));
        state.text.insert(0x6665_93C4, label(0x1B15_F53C));
        state.text.insert(0x59AB_A1E8, label(0x37E7_40E0));
        // Instructions_Label: "Configure your settings" (0x8BAB66EF, the
        // constructor 0x4ECF76); while a key is awaited "Press a button to
        // use for this function" (0x23CF49AA, Accept 0x4F9420 / 0x4ED202);
        // after a bind "Press enter to assign a new key or button"
        // (0xFED0D3DF, 0x4F982B). Wine menus113 o5 / o6 / o8.
        let instructions = if self.capturing {
            0x23CF_49AA
        } else if self.bound {
            0xFED0_D3DF
        } else {
            fe_ids::TXT_CONFIGURE_YOUR_SETTINGS
        };
        state.text.insert(fe_ids::INSTRUCTIONS_LABEL, label(instructions));
        // The constructor 0x4ECD10 runs "UnHighlight" on the header
        // (0x4E11CF90) before building the rows; Up on the first row
        // highlights it (0x4F94C8, "Highlight" 0x78F228).
        state.run(fe_ids::OBJ_4E11CF90, if self.header { "HIGHLIGHT" } else { "UNHIGHLIGHT" }, 0.0);
        // 0x4ACE00 counts the input devices with a configuration (input
        // map +0x230 ≠ −1, 0x874C40 + 0x370 each): with one (the keyboard
        // alone) it hides the header's device arrows (+0xDC / +0xE0,
        // 0xD1CDA594 / 0x9345E4A7), else shows them on "Static" (a change
        // blinks one, "Blink", 0x4ACBC9; **not ported**).
        if devices > 1 {
            for a in [fe_ids::OBJ_D1CDA594, fe_ids::OBJ_9345E4A7] {
                state.shown.insert(a);
                state.run(a, "STATIC", 0.0);
            }
        } else {
            state.hidden.insert(fe_ids::OBJ_D1CDA594);
            state.hidden.insert(fe_ids::OBJ_9345E4A7);
        }
        // The scroll bar ("scrollbar", 0x542AD0 at the constructor):
        // 0x538090(1, +0x134 − 1, the selected row's position); +0x134
        // starts at 1 and counts the rows 0x4CB4B0 adds.
        scroll_bar_at(&mut state, package, "ScrollBar", false, 1, CONTROL_ROWS.len(), self.selected + 1);
        for m in [NAME_MASTER, PRIMARY_MASTER, SECONDARY_MASTER] {
            state.hidden.insert(m);
        }
        let boxes = (
            master_box(package, NAME_MASTER),
            master_box(package, PRIMARY_MASTER),
            master_box(package, SECONDARY_MASTER),
        );
        let (Some((nl, ns)), Some((pl, ps)), Some((sl, ss))) = boxes else {
            return layout(assets, package, &state, now - self.opened);
        };
        for (i, &(name, _)) in CONTROL_ROWS.iter().enumerate() {
            let [primary, secondary] = &keys[i];
            let shown = i >= self.top && i < self.top + VISIBLE;
            for h in [NAMES[i], PRIMARY[i], SECONDARY[i]] {
                if shown {
                    state.shown.insert(h);
                } else {
                    state.hidden.insert(h);
                }
            }
            if !shown {
                continue;
            }
            // Rows step by the name master's height (as UI_Options' list,
            // 0x552790); the name at its top left, the keys centred on
            // their columns.
            // 0x4CB4B0 steps the row y (+0x100) by +0x130 + +0x108 and
            // gives the key cells (+0x120 / +0x128) the same y as the
            // name. A key cell's placement (0x52D6C0) sets its text's top
            // left (0x51D4E0 / 0x51D080) there rather than its position:
            // the key strings are centred (justify 5), so their position
            // is half the text's height lower. Under Wine the keys sit on
            // the names' line.
            let dy = ns.y * (i - self.top) as f32;
            let y = nl.y + dy;
            state.text.insert(NAMES[i], label(name));
            state.position.insert(NAMES[i], Vec2::new(nl.x, y));
            state.text.insert(PRIMARY[i], key_cell(primary));
            state
                .position
                .insert(PRIMARY[i], Vec2::new(pl.x + ps.x * 0.5, y + half_height(assets, package, PRIMARY[i])));
            state.text.insert(SECONDARY[i], key_cell(secondary));
            state
                .position
                .insert(SECONDARY[i], Vec2::new(sl.x + ss.x * 0.5, y + half_height(assets, package, SECONDARY[i])));
            // The mouse's cells (0x4CB6B0 over the cells' boxes +0xC..+0x18;
            // **inferred**: a box is its column master's width at the row,
            // one row tall).
            for (col, (l, sz)) in [(pl, ps), (sl, ss)].into_iter().enumerate() {
                let lo = Vec2::new(l.x, y) + crate::frontend::CANVAS * 0.5;
                crate::frontend::mouse::record_rect(
                    "UI_Options_PC_Controller",
                    crate::frontend::mouse::CONTROL_CELL + (i * 2 + col) as u32,
                    lo,
                    lo + Vec2::new(sz.x, ns.y),
                    Default::default(),
                );
            }
            // Each row's name item (0x51E0A0, vtable 0x79B348) keeps
            // "Options_Backing_<n>" (n = the row's number from 1, 0x4CB4B0)
            // at +0x24 and places it with the text (0x51E100): at the
            // text's position less (+0x1C, +0x20) = (−276, −8). Its
            // Highlight (slot +0x24, 0x51E140) / UnHighlight (+0x28,
            // 0x51E030) run on the text and on the backing; the key cells'
            // the same on their text (0x52D780). The constructor highlights
            // the first row's name and its primary key cell.
            let backing = fe_hash(&format!("Options_Backing_{}", i + 1));
            state.position.insert(backing, Vec2::new(nl.x + 276.0, y + 8.0));
            let on = i == self.selected && !self.header;
            // (Script names hash case-blind, 0x505450.)
            let script = if on { "HIGHLIGHT" } else { "UNHIGHLIGHT" };
            state.run(NAMES[i], script, 0.0);
            state.run(backing, script, 0.0);
            let cell = if self.column == 0 { PRIMARY[i] } else { SECONDARY[i] };
            let other = if self.column == 0 { SECONDARY[i] } else { PRIMARY[i] };
            state.run(cell, script, 0.0);
            state.run(other, "UNHIGHLIGHT", 0.0);
            // The cell being bound keeps its key's name and runs "Pulse"
            // (Accept 0x4F93FA: 0x51CF80(text, "Pulse", 1); the package's
            // 1.2 s loop from grey to green and back; Wine menus113 o6 /
            // o7).
            if on && self.capturing {
                state.run(cell, "PULSE", self.capture_at - self.opened);
            }
        }
        // The rows not on screen keep their backings where the package
        // has them (off screen).
        layout(assets, package, &state, now - self.opened)
    }
}
