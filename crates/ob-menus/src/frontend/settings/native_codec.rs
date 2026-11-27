//! The options kept in a native profile: read and write.

use super::*;

impl Settings {
    /// The options the original keeps in the profile (settings block
    /// profile +0x24, the career difficulty +0x250, the save options
    /// +0xA0F8 / +0xA0F9, the EA Trax playlist +0x2D8, the Quick Race
    /// records +0xD4 + 0x10): read from a native profile. The display
    /// options (registry values in the original, 0x5BEA30) and the key map
    /// (DirectInput's own, not the profile's) stay in options.txt; the
    /// controller maps are the profile's (0xD254, `pad_blocks`).
    pub fn apply_native(&mut self, s: &crate::formats::native_save::NativeSave) {
        use crate::formats::native_save::*;
        let v = |i: usize| s.f32(VOLUMES + 4 * i).clamp(0.0, 1.0);
        (self.master, self.speech, self.menu_music, self.game_music, self.effects, self.engine, self.tires) =
            (v(0), v(1), v(2), v(3), v(4), v(5), v(6));
        self.stereo = s.u32(AUDIO_MODE) != 0;
        self.rearview = s.u8(SETTINGS_14) != 0;
        self.metric = s.u8(SETTINGS_14 + 2) != 0;
        self.race_map = s.u8(SETTINGS_14 + 3).min(2);
        self.explore_map = s.u8(SETTINGS_14 + 4).min(2);
        self.jump_camera = s.f32(SETTINGS_14 + 8) != 0.0;
        for (i, p) in self.players.iter_mut().enumerate() {
            let o = PLAYERS + i * 0x28;
            let b = |k: usize| s.u8(o + k) != 0;
            (p.gauges, p.position, p.lap_info, p.score, p.feedback, p.leaderboard, p.ask_transmission) =
                (b(0), b(1), b(2), b(3), b(4), b(5), b(6));
            p.camera = s.u8(o + 0x10).min(4);
            p.split = s.u8(o + 0x14).min(4);
            p.manual = b(0x15);
            p.stability_off = b(0x16);
            p.wheel_range = s.f32(o + 0x1C).clamp(0.1, 1.0);
            p.road_feedback = s.f32(o + 0x20).clamp(2.0, 20.0);
            p.force = s.f32(o + 0x24).clamp(0.0, 1.0);
        }
        self.trax_random = s.u32(TRAX_MODE) != 0;
        self.pad_blocks = (0..crate::pad::RECORDS)
            .filter_map(|k| {
                let o = CONTROLLER_MAPS + k * crate::pad::BLOCK_LEN;
                let b: [u8; crate::pad::BLOCK_LEN] = s.payload.get(o..o + crate::pad::BLOCK_LEN)?.try_into().ok()?;
                (b[..16] != [0; 16] && b[..16] != GUID_SYS_KEYBOARD).then_some(b)
            })
            .collect();
        self.difficulty = s.u32(DIFFICULTY).min(2) as u8;
        self.autosave = s.u8(AUTOSAVE) != 0;
        self.save_prompts = s.u8(SAVE_PROMPTS) != 0;
        for j in 0..self.trax.len() {
            let (index, state) = (s.u32(TRAX + 8 * j) as usize, s.u32(TRAX + 8 * j + 4));
            if let Some(t) = self.trax.get_mut(index) {
                *t = state.min(3) as u8;
                self.trax_order[j] = index as u8;
            }
        }
        for (t, r) in self.qr.iter_mut().enumerate() {
            let o = RACE_RECORD + 0x10 + 20 * t;
            r.players = s.u8(o).max(1);
            r.opponents = s.u8(o + 1).min(5);
            r.difficulty = s.u8(o + 2).min(2);
            r.traffic = s.u8(o + 4).min(3);
            r.catch_up = s.u8(o + 5) != 0;
            r.track = s.u16(o + 8);
            r.laps = s.u16(o + 0xA).clamp(1, 10);
            r.reversed = s.u8(o + 0xC) != 0;
            r.lap_ko = s.u8(o + 0x10) != 0;
        }
    }

    /// [`Self::apply_native`]'s fields written into a native profile.
    pub fn write_native(&self, s: &mut crate::formats::native_save::NativeSave) {
        use crate::formats::native_save::*;
        // What the file holds as the clone reads it: a record or a player's
        // settings the clone did not change keep the original's bytes
        // (e.g. Free Run's record has 0 laps, which the clone reads as 1).
        let mut was = Settings::default();
        was.apply_native(s);
        for (i, v) in
            [self.master, self.speech, self.menu_music, self.game_music, self.effects, self.engine, self.tires]
                .into_iter()
                .enumerate()
        {
            s.set_f32(VOLUMES + 4 * i, v);
        }
        s.set_u32(AUDIO_MODE, self.stereo as u32);
        s.set_u8(SETTINGS_14, self.rearview as u8);
        s.set_u8(SETTINGS_14 + 2, self.metric as u8);
        s.set_u8(SETTINGS_14 + 3, self.race_map);
        s.set_u8(SETTINGS_14 + 4, self.explore_map);
        // On: the defaults' 127.5 kept, else the option's 127 (0x5116E0 /
        // the Jump Camera row); off: 0.
        let jump = s.f32(SETTINGS_14 + 8);
        s.set_f32(
            SETTINGS_14 + 8,
            if !self.jump_camera {
                0.0
            } else if jump != 0.0 {
                jump
            } else {
                127.0
            },
        );
        for (i, p) in self.players.iter().enumerate().filter(|&(i, p)| was.players[i] != *p) {
            let o = PLAYERS + i * 0x28;
            for (k, v) in [p.gauges, p.position, p.lap_info, p.score, p.feedback, p.leaderboard, p.ask_transmission]
                .into_iter()
                .enumerate()
            {
                s.set_u8(o + k, v as u8);
            }
            s.set_u8(o + 0x10, p.camera);
            s.set_u8(o + 0x14, p.split);
            s.set_u8(o + 0x15, p.manual as u8);
            s.set_u8(o + 0x16, p.stability_off as u8);
            s.set_f32(o + 0x1C, p.wheel_range);
            s.set_f32(o + 0x20, p.road_feedback);
            s.set_f32(o + 0x24, p.force);
        }
        s.set_u32(TRAX_MODE, self.trax_random as u32);
        // The controller maps as 0x5B9430 lays them out: the records in
        // order (the keyboard's first, kept as the file has it), zeros
        // past them. Untouched while the clone has none.
        if self.pad_blocks != was.pad_blocks {
            let n = crate::pad::BLOCK_LEN;
            let keyboard = (0..crate::pad::RECORDS)
                .map(|k| CONTROLLER_MAPS + k * n)
                .find(|&o| s.payload.get(o..o + 16) == Some(&GUID_SYS_KEYBOARD[..]))
                .map(|o| s.payload[o..o + n].to_vec());
            let mut blocks: Vec<Vec<u8>> = keyboard.into_iter().collect();
            blocks.extend(self.pad_blocks.iter().map(|b| b.to_vec()));
            for k in 0..crate::pad::RECORDS {
                let o = CONTROLLER_MAPS + k * n;
                let b = blocks.get(k).cloned().unwrap_or_else(|| vec![0; n]);
                if let Some(dst) = s.payload.get_mut(o..o + n) {
                    dst.copy_from_slice(&b);
                }
            }
        }
        s.set_u32(DIFFICULTY, self.difficulty as u32);
        s.set_u8(AUTOSAVE, self.autosave as u8);
        s.set_u8(SAVE_PROMPTS, self.save_prompts as u8);
        for j in 0..self.trax.len() {
            let index = s.u32(TRAX + 8 * j) as usize;
            if let Some(&t) = self.trax.get(index) {
                s.set_u32(TRAX + 8 * j + 4, t as u32);
            }
        }
        for (t, r) in self.qr.iter().enumerate().filter(|&(t, r)| was.qr[t] != *r) {
            let o = RACE_RECORD + 0x10 + 20 * t;
            s.set_u8(o, r.players);
            s.set_u8(o + 1, r.opponents);
            s.set_u8(o + 2, r.difficulty);
            s.set_u8(o + 4, r.traffic);
            s.set_u8(o + 5, r.catch_up as u8);
            s.set_u16(o + 8, r.track);
            s.set_u16(o + 0xA, r.laps);
            s.set_u8(o + 0xC, r.reversed as u8);
            s.set_u8(o + 0x10, r.lap_ko as u8);
        }
    }
}
