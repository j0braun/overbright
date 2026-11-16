//! Defaults, Restore Native, first-run display and camera defaults.

use super::*;

impl Default for Settings {
    /// The profile constructor's defaults (0x5116E0); a fresh run's
    /// resolution is the clone's 1280 × 960 window (the game's 0x5CECA0
    /// sets index 0, 640 × 480, which Default Settings still does); a fresh
    /// windowed run replaces the detail 5 and its custom values with
    /// [`Settings::first_run_display`]'s and saves them.
    fn default() -> Settings {
        Settings {
            master: 1.0,
            speech: 1.0,
            menu_music: 0.7,
            game_music: 1.0,
            effects: 1.0,
            engine: 1.0,
            tires: 1.0,
            stereo: true,
            resolution: crate::render::display::DEFAULT_MODE,
            fullscreen: false,
            aspect: 0,
            vsync_off_mailbox: true,
            detail: 5,
            custom: custom_presets(5),
            jump_camera: true,
            rearview: true,
            // +0x16 is "the language is not English" (0x512360); the clone
            // runs in English.
            metric: false,
            // The profile reset 0x515AA0 (0x534700, on every new profile)
            // sets the career difficulty +0x250 = 1 (Medium, 0x515B15) and
            // the save options +0xA0F8 = 1 (prompts), +0xA0F9 = 0 (no
            // autosave) (0x515AFB / 0x515B02). Under Wine a fresh profile
            // shows Career Difficulty Medium and Save Prompt (2026-10-29;
            // peeked 0x83AC30 = 1, 0x844AD8 = 1, 0x844AD9 = 0).
            difficulty: 1,
            explore_map: 2,
            race_map: 2,
            save_prompts: true,
            autosave: false,
            // The profile's reset (0x515AA0, at 0x515B46) fills the
            // playlist in table order: "FE" tracks Menu (1), "IG" Racing
            // (2), "AL" All (3), else Off; the table's first six are FE.
            trax: std::array::from_fn(|i| if i < 6 { 1 } else { 2 }),
            trax_order: std::array::from_fn(|i| i as u8),
            trax_random: true,
            keys: DEFAULT_KEYS.map(|k| k.map(String::from)),
            pad_blocks: Vec::new(),
            players: [PlayerSettings::default(); 2],
            qr: std::array::from_fn(super::quick_race::QrRecord::new_profile),
            enh: crate::qol::Enhancements::default(),
            custom_raised: [None; 22],
            overlay: crate::overlay::Settings::default(),
        }
    }
}

impl Settings {
    /// The deviations from native switched on (the F3 overlay's
    /// Deviations rows, crash reports): the Enhancements page's, and
    /// the Display page's Aspect Ratio and Vsync Off Mode when not native.
    pub fn deviations(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.aspect != 1 {
            v.push("wide aspect".to_string());
        }
        if self.vsync_off_mailbox {
            v.push("vsync-off mailbox".to_string());
        }
        v.extend(self.enh.deviations());
        v
    }

    /// The Enhancements page's "Restore Native": every deviation the
    /// options hold off — the Enhancements page's values native, Aspect
    /// Ratio Stretched and Vsync Off Mode Immediate (the window mode and
    /// the resolution stay).
    pub fn restore_native(&mut self) {
        // The Controller Layout is a device setting, like the resolution
        // (controller support): Restore Native leaves it.
        let layout = self.enh.pad.layout;
        // L1: the launcher's own choice (`launcher.skip`) is not a
        // deviation of the game: Restore Native leaves it too.
        let launcher_skip = self.enh.launcher_skip;
        self.enh = crate::qol::Enhancements::default();
        self.enh.pad.layout = layout;
        self.enh.launcher_skip = launcher_skip;
        self.overlay = crate::overlay::Settings::default();
        self.aspect = 1;
        self.vsync_off_mailbox = false;
        self.native_display();
    }

    /// Customize Display back to the original's values for the World Detail
    /// level set in the original's options (design choice: Restore Native
    /// and the Original preset set the graphics to the native values of the level
    /// set in the original's settings; this replaces the earlier giving back
    /// of remembered values, which could bring back stale low ones).
    pub fn native_display(&mut self) {
        self.custom = custom_presets(self.detail);
        self.custom_raised = [None; 22];
        super::graphics_table::normalize(self);
    }

    /// Put the Enhancements page's values in force (`qol`'s globals).
    pub fn apply_enhancements(&self) {
        // The settings page's Compare shows the original's look (the
        // Original preset's graphics values, QoL kept) without changing or
        // saving anything (docs/settings-page.md).
        if crate::qol::comparing() {
            let mut e = self.enh;
            crate::qol::Preset::Original.apply(&mut e);
            e.apply();
        } else {
            self.enh.apply();
        }
        crate::qol::crash::set_deviations(self.deviations().join(", "));
    }

    /// "Default Settings" (0x5CECA0): resolution index 0 and the detail
    /// estimated from the hardware ([`estimate_detail`]).
    pub fn reset_display(&mut self, card: (u32, u32)) {
        // Index 0: 640 × 480 (the clone options keep theirs).
        self.resolution = crate::render::display::NATIVE_MODES[0];
        self.estimated_detail(estimate_detail(card.0, card.1, cpu_mhz()));
        // 0x5BE880 above detail 3 on a score-5 card (ftol(0x5C0CE0) > 4):
        // index 1, 800 × 600 (0x5BE9B6).
        if self.detail > 3 && super::gpu_rating::card_score(card.0, card.1) > 4 {
            self.resolution = crate::render::display::NATIVE_MODES[1];
        }
    }

    /// A fresh run's display values (0x5CED70: no "NotFirstTime" → 0x5CECA0,
    /// the Default Settings routine): World Detail from the hardware
    /// ([`estimate_detail`]) and the Customize Display values for it. The
    /// display mode (resolution, window, aspect, Vsync Off Mode) stays the
    /// clone's default. The caller saves options.txt at once, so a later
    /// [`Settings::load`] keeps it (natively the registry save 0x5BEEB0).
    pub fn first_run_display(&mut self, card: (u32, u32)) {
        self.estimated_detail(estimate_detail(card.0, card.1, cpu_mhz()));
    }

    /// 0x5CECA0's detail part: the estimate stored unclamped (0 on Intel),
    /// then 0x5CED44 → 0x5BE880: the custom values follow the detail (Vsync
    /// 0), clamped to the caps (0x5BE6A0).
    pub(crate) fn estimated_detail(&mut self, detail: u8) {
        self.detail = detail;
        self.custom = custom_presets(self.detail);
        self.custom_raised = [None; 22];
        super::graphics_table::normalize(self);
    }

    /// The race set-ups' camera setting (player 1's, 0x83AA74; **verified**):
    /// a drift race puts it on 4, Super Far (kind 5): Quick Race's drift
    /// set-up 0x4FF070 (0x4FF0CC) and the career's 0x5322F0 for an event
    /// whose +0x0F is 5 (0x532569); every other set-up — the default
    /// 0x4FEF80, drag 0x4FEFF0, 0x4FF0E0, the career / free roam path
    /// 0x4FF160 (0x526980 with its flag set) and 0x532575 — turns a 4 back
    /// into 3, Far. Under Wine a profile saved with 4 resumed to free roam
    /// with the far view (2026-11-05, docs/world-systems-parity.md).
    pub fn race_camera(&mut self, drift: bool) {
        let p = &mut self.players[0];
        if drift {
            p.camera = 4;
        } else if p.camera == 4 {
            p.camera = 3;
        }
    }

    /// Step the camera setting the way the camera button does (0x502540):
    /// round 0–3, or 0–4 (Super Far) in drift races.
    pub fn cycle_camera(&mut self, player: usize, forward: bool, drift: bool) {
        let top = if drift { 4 } else { 3 };
        let p = &mut self.players[player.min(1)];
        p.camera = if forward {
            if p.camera >= top { 0 } else { p.camera + 1 }
        } else if p.camera == 0 {
            top
        } else {
            p.camera - 1
        };
    }
}
