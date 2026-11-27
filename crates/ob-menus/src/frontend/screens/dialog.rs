use crate::frontend::fe_ids;
use super::*;

/// The game's dialog (GenericDialog_SMALL in the front end,
/// IG_GenericDialog_SMALL in game; opened by 0x5587A0 with its text and
/// button labels): the message (0x1E2640FA) and one centred button
/// (group 0xB8A7C6CE, label 0xDA473D31) or two (groups 0xB8A7C6CC /
/// 0xB8A7C6CD, labels 0xDA473D2F / 0xDA473D30), the chosen one run through
/// its HIGHLIGHT script and the other UNHIGHLIGHT.
/// The generic dialog's package for a request that names none (0x526C50,
/// called by 0x558030 at 0x558072; 46 of the 0x5587A0 / 0x558690 /
/// 0x558580 call sites pass "" at 0x783FE5): the text's length (strlen,
/// 0x43DBB0) × the line height of the font CONDUITMDITC_TT21I (0x51BD70 by
/// bin hash 0x5B9D88B9: the font record's +0x24, which 0x51BDC0 fills
/// with the FNTF header's +0x24 line height as a float). Under 2561 SMALL,
/// from 5122 LARGE, else MED.
pub fn dialog_package(assets: &Assets, text: &str) -> &'static str {
    let height = assets.fonts.get(&bin_hash("CONDUITMDITC_TT21I")).map_or(0.0, |f| f.line_height);
    dialog_size(text.chars().count(), height)
}

/// 0x526C50's thresholds on strlen × line height.
pub fn dialog_size(len: usize, line_height: f32) -> &'static str {
    let width = len as f32 * line_height;
    if width < 2561.0 {
        "GenericDialog_SMALL"
    } else if width >= 5122.0 {
        "GenericDialog_LARGE"
    } else {
        "GenericDialog_MED"
    }
}

/// 0x526C50 for a help request (+0x32D): HelpDialog_MED for more than four
/// lines ('\n' or '^' each start one), else HelpDialog_SMALL.
pub fn help_dialog_package(text: &str) -> &'static str {
    let lines = 1 + text.chars().filter(|&c| c == '\n' || c == '^').count();
    if !text.is_empty() && lines > 4 { "HelpDialog_MED" } else { "HelpDialog_SMALL" }
}

/// PC_Loading.fng over a track load. The track loader's first step
/// (TrackLoader::LoadHandler 0x57F2E0, step 1 at 0x57F333) and the region
/// loader's (RegionLoader::LoadHandler 0x57DC70, at 0x57DCC9 unless
/// 0x864FA8) open it (0x4C57C0: 0x555E90 when 0x52CF70 finds it not up);
/// the game flow's LOADING state (handler 0x6038F0) closes it with
/// loading_boot and HUD_World_Loading when it leaves (message 2 →
/// 0x4A9310). The package has no screen class: it runs its own scripts
/// (the controls list, then "LOADING" and its dots). The clone loads
/// synchronously, so it draws the package once as the load starts.
#[derive(Clone, Copy, Debug)]
pub struct Loading {
    pub package: &'static str,
    pub opened: f32,
}

impl Loading {
    pub const PC: &'static str = "PC_Loading";

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package(self.package) else { return Vec::new() };
        // No screen class runs the package, so every zero-length INIT goes
        // on with its chain at once: the controls group AF4A85EB's INIT
        // (alpha 255) → 0x0016A259 (alpha 1, its strings at 0) and the
        // message 0x6B93C93D at 600 ms → 5079C8F8 (alpha 255 − 254 = 1)
        // keep the PC controls list invisible (verified under Wine
        // 2026-10-29: the race and career loads show only the backdrop,
        // "LOADING" and its dots).
        let state = ScreenState { chain_init: true, ..ScreenState::default() };
        // Clone deviation (wide screens, `display::Place`): the 4:3
        // backdrop (PC_load.tga, 640 × 480) keeps its shape, centred, with
        // black beside it, so a black flat quad across the canvas goes
        // first; it reaches the screen's edges (`render::ui`) and hides
        // what the load covers (at 4:3 the opaque backdrop covers it).
        let mut quads = vec![Quad {
            texture: None,
            rect: [Vec2::ZERO, crate::frontend::CANVAS],
            uv: [Vec2::ZERO, Vec2::ONE],
            color: [0.0, 0.0, 0.0, 1.0],
            z: 255.0,
            angle: 0.0,
            place: Place::CENTRE,
        }];
        quads.extend(layout(assets, package, &state, now - self.opened));
        quads
    }
}

pub struct Dialog {
    pub package: &'static str,
    pub text: String,
    pub buttons: Vec<String>,
    pub selected: usize,
    pub opened: f32,
    /// The outcome that is the screen the dialog was pushed over and that
    /// stays drawn under it (ShowDialog 0x558030 → 0x555D10 op 0x16 over
    /// the caller's package, arg 1 of 0x5587A0): set where the screen asks
    /// straight from its handler without running FORWARD first (the car
    /// lot's Back, 0x4EDE8B / 0x4EDED8). None: the clone draws no screen
    /// under it.
    pub under: Option<usize>,
    /// The screen under it plays DIM on these objects of this package
    /// right after asking: its Event Handler (0x47FF4E7C) by StartDim
    /// (0x538C10) or 0x537C10(package, 0x47FF4E7C, "DIM") (the car lot's
    /// stock / trade-in questions, 0x4EDCBC). The handler's DIM sends
    /// 0xA301C015, to which the screen's objects answer with their own DIM
    /// (alpha tracks over 100 ms); UNDIM (0x9F218E63) brings it back. See
    /// `frontend::with_dim`.
    pub dim_under: Option<(&'static str, &'static [u32])>,
    /// The objects the screen plays UNDIM on when the dialog is answered
    /// back to it, when not those of `dim_under` (UI_CareerCrib's No also
    /// "UnDim"s 0x444969FD / 0x444969FE, 0x4AF4BB).
    pub undim_under: Option<&'static [u32]>,
    /// The scripts `dim_under` plays in place of DIM / UNDIM (the parts
    /// browser's exit question: MENUFADEOUT, and MENUFADE on No).
    pub dim_scripts: Option<(u32, u32)>,
    /// Whether Back answers (screen +0x384, the request's flag: 0x5587A0,
    /// 0x558760 and 0x558580 set it; 0x558620, 0x5585F0, 0x558830 and
    /// 0x558870 do not, and Back then does nothing).
    pub back: bool,
    /// The outcome Back gives when it is not the last button's: the
    /// request's own cancel message (0x5587A0's seventh argument) where it
    /// differs from No's (the dyno's save question 0x71851654: No
    /// 0x1FAB5998, cancel 0x34DC1BCF = StopDim, back on the dyno menu).
    pub cancel: Option<usize>,
    /// A sound the opening screen's handler plays when it gets this
    /// button's answer (e.g. UI_CareerCarLot's 0x51123592 → BACK).
    pub answer_sound: Option<(usize, u8)>,
}

/// The Event Handler alone: StartDim's (0x538C10) object.
pub const DIM_HANDLER: &[u32] = &[crate::frontend::EVENT_HANDLER];

/// UI_Paint's 0x53BD30 in a slot (+0x188): StartDim, then "Dim" (0x78FEB8)
/// on its group 0xDB285767 (whose DIM sets its alpha to 0 at once: the
/// strip and title go).
pub const DIM_PAINT_SLOT: &[u32] = &[crate::frontend::EVENT_HANDLER, fe_ids::SWATCH_PANEL];

pub const MESSAGE: u32 = fe_ids::DIALOG_MESSAGE;
pub const ONE: (u32, u32) = (fe_ids::BUTTON, fe_ids::ONE_1);
pub const LEFT: (u32, u32) = (fe_ids::BUTTON_TEXT_0, fe_ids::LEFT_1);
pub const RIGHT: (u32, u32) = (fe_ids::BUTTON_TEXT_1, fe_ids::RIGHT_1);

impl Dialog {
    pub fn new(package: &'static str, text: &str, buttons: &[&str], now: f32) -> Dialog {
        Dialog {
            package,
            text: text.into(),
            buttons: buttons.iter().map(|b| b.to_string()).collect(),
            selected: 0,
            opened: now,
            under: None,
            dim_under: None,
            undim_under: None,
            dim_scripts: None,
            back: true,
            cancel: None,
            answer_sound: None,
        }
    }

    /// Left/Right move between buttons (Up/Down down the three-button
    /// dialog's column); Accept gives the chosen one; Back gives the last
    /// (No / Cancel).
    pub fn key(&mut self, key: Key) -> Option<usize> {
        let n = self.buttons.len().max(1);
        match key {
            Key::Left | Key::Up => self.selected = self.selected.saturating_sub(1),
            Key::Right | Key::Down => self.selected = (self.selected + 1).min(n - 1),
            Key::Accept => return Some(self.selected),
            Key::Back if self.back => return Some(self.cancel.unwrap_or(n - 1)),
            _ => {}
        }
        None
    }

    /// Each button's group object, by button (for the mouse).
    pub fn button_groups(&self) -> Vec<u32> {
        let slots: &[(u32, u32)] = match self.buttons.len() {
            1 => &[dialog::ONE],
            3 => &[dialog::ONE, dialog::RIGHT, dialog::LEFT],
            _ => &[dialog::LEFT, dialog::RIGHT],
        };
        slots.iter().map(|s| s.0).collect()
    }

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package(self.package) else { return Vec::new() };
        // The header (0x426C7B4D "Memory Card") and its icon (0xC26A8D5C)
        // INIT into HIDE; only the memory-card screens' code shows them
        // (0x4BEBEB, 0x4D6FD4). The dialog group's own INIT → HIDE is
        // brought back by the Event Handler's FADEIN broadcast.
        let mut state = ScreenState { chain_init: true, ..ScreenState::default() };
        footer(&mut state, package, "Select");
        state.text.insert(dialog::MESSAGE, self.text.replace('^', "\n"));
        // GenericDialog_ThreeButton stacks its buttons down the dialog:
        // 0xB8A7C6CE, 0xB8A7C6CD, 0xB8A7C6CC from the top.
        let slots: &[(u32, u32)] = match self.buttons.len() {
            1 => &[dialog::ONE],
            3 => &[dialog::ONE, dialog::RIGHT, dialog::LEFT],
            _ => &[dialog::LEFT, dialog::RIGHT],
        };
        // The labels of the layout not used stay hidden.
        for (group, label) in [dialog::ONE, dialog::LEFT, dialog::RIGHT] {
            if !slots.iter().any(|s| s.0 == group) {
                state.hidden.insert(group);
                state.hidden.insert(label);
            }
        }
        for (i, (&(group, label), text)) in slots.iter().zip(&self.buttons).enumerate() {
            state.shown.insert(group);
            state.shown.insert(label);
            state.text.insert(label, text.clone());
            let script = if i == self.selected { "HIGHLIGHT" } else { "UNHIGHLIGHT" };
            state.run(group, script, 0.0);
            state.run(label, script, 0.0);
        }
        layout(assets, package, &state, now - self.opened)
    }
}
