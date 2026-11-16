//! options.txt: path, load, parse, write.

use std::path::PathBuf;

use super::*;

impl Settings {
    pub fn path() -> PathBuf {
        // Overbright's settings folder (`platform::dirs::config`).
        crate::platform::dirs::config().join("options.txt")
    }

    /// The options in force: the display options and the key map from
    /// options.txt (the original's registry values, 0x5BEA30, and the
    /// clone's key map), the rest the current profile's — or, with no
    /// profile loaded, the defaults. The original keeps every other option
    /// in the profile only: the profile object's constructor fills the
    /// settings with 0x5116E0's defaults (volumes 1.0, menu music 0.7,
    /// Random play mode, …; the profile reset 0x515AA0 → 0x515800 runs on
    /// every new profile), nothing reads them from the registry (its values
    /// are 0x5BEA30's display ones only), and the boot loads a profile only
    /// after the splash ("Load complete." with exactly one, MC_List with
    /// several) — so the boot movies, the splash and a list of profiles
    /// play with the default volumes, whatever the last profile had. Applying
    /// the last profile's options at boot instead would make the first
    /// loading screen silent for a profile with music at 0 and hide the
    /// EA Trax caption.
    pub fn load() -> Settings {
        let all = Settings::load_text();
        let mut s = Settings {
            resolution: all.resolution,
            fullscreen: all.fullscreen,
            aspect: all.aspect,
            vsync_off_mailbox: all.vsync_off_mailbox,
            detail: all.detail,
            custom: all.custom,
            custom_raised: all.custom_raised,
            keys: all.keys,
            enh: all.enh,
            overlay: all.overlay,
            ..Settings::default()
        };
        // The profile's own options (the current profile's payload).
        if let Some(n) = super::profiles::current_save() {
            s.apply_native(&n);
        }
        s.apply_enhancements();
        s
    }

    /// Everything options.txt holds, parsed whole (the keys
    /// [`Settings::load`] keeps and any others).
    pub fn load_text() -> Settings {
        let Ok(text) = std::fs::read_to_string(Settings::path()) else {
            return Settings::default();
        };
        Settings::from_text(&text)
    }

    /// [`Settings::load_text`] of options.txt's text.
    pub fn from_text(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            let f = value.parse::<f32>().ok();
            let n = value.parse::<u8>().ok();
            let b = value.parse::<u8>().ok().map(|v| v != 0);
            match key {
                "master" => s.master = f.unwrap_or(s.master),
                "speech" => s.speech = f.unwrap_or(s.speech),
                "menu_music" => s.menu_music = f.unwrap_or(s.menu_music),
                "game_music" => s.game_music = f.unwrap_or(s.game_music),
                "effects" => s.effects = f.unwrap_or(s.effects),
                "engine" => s.engine = f.unwrap_or(s.engine),
                "tires" => s.tires = f.unwrap_or(s.tires),
                "stereo" => s.stereo = b.unwrap_or(s.stereo),
                // "WxH"; an older file's index (never applied) is ignored.
                "resolution" => {
                    if let Some((w, h)) = value
                        .trim()
                        .split_once('x')
                        .and_then(|(w, h)| Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)))
                        .filter(|&(w, h)| w > 0 && h > 0 && w <= 16384 && h <= 16384)
                    {
                        // At least the original's 640 × 480.
                        s.resolution = crate::render::display::clamp_min_size((w, h), "options.txt resolution");
                    }
                }
                "fullscreen" => s.fullscreen = b.unwrap_or(s.fullscreen),
                "aspect" => s.aspect = n.unwrap_or(s.aspect).min(1),
                "vsync_off" => s.vsync_off_mailbox = value.trim() != "immediate",
                // 0 is a stored native value (0x5CDE50's estimate on Intel;
                // g_PerformanceLevel reloads unclamped, 0x5BEA30).
                "detail" => {
                    s.detail = n.unwrap_or(s.detail).min(5);
                    s.custom = custom_presets(s.detail);
                }
                k if k.starts_with("custom.") => {
                    if let (Some(i), Some(v)) = (k[7..].parse::<usize>().ok().filter(|&i| i < 22), n) {
                        s.custom[i] = v;
                    }
                }
                "jump_camera" => s.jump_camera = b.unwrap_or(s.jump_camera),
                "rearview" => s.rearview = b.unwrap_or(s.rearview),
                "metric" => s.metric = b.unwrap_or(s.metric),
                "difficulty" => s.difficulty = n.unwrap_or(s.difficulty).min(2),
                "explore_map" => s.explore_map = n.unwrap_or(s.explore_map).min(2),
                "race_map" => s.race_map = n.unwrap_or(s.race_map).min(2),
                "save_prompts" => s.save_prompts = b.unwrap_or(s.save_prompts),
                "autosave" => s.autosave = b.unwrap_or(s.autosave),
                k if k.starts_with("key.") => {
                    // "key.<row>.<0|1> <name>"
                    let mut it = k[4..].split('.');
                    let row = it.next().and_then(|v| v.parse::<usize>().ok());
                    let slot = it.next().and_then(|v| v.parse::<usize>().ok());
                    if let (Some(r), Some(c)) = (row, slot)
                        && r < 18
                        && c < 2
                    {
                        s.keys[r][c] = value.to_string();
                    }
                }
                k if k.starts_with("qr.") => {
                    // "qr.<type> opponents difficulty traffic catch_up track laps reversed lap_ko"
                    if let Some(r) = k[3..].parse::<usize>().ok().and_then(|i| s.qr.get_mut(i)) {
                        let v: Vec<u16> = value.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                        if let [o, d, t, c, track, laps, rev, ko] = v[..] {
                            r.opponents = (o as u8).min(5);
                            r.difficulty = (d as u8).min(2);
                            r.traffic = (t as u8).min(3);
                            r.catch_up = c != 0;
                            r.track = track;
                            r.laps = laps.clamp(1, 10);
                            r.reversed = rev != 0;
                            r.lap_ko = ko != 0;
                        }
                    }
                }
                k if k.starts_with("overlay.") => {
                    s.overlay.parse(k, value);
                }
                k if k.starts_with("enh.raised.") => {
                    if let (Some(i), Some(v)) = (k[11..].parse::<usize>().ok().filter(|&i| i < 22), n) {
                        s.custom_raised[i] = Some(v.min(CUSTOM_CAPS[i]));
                    }
                }
                k if k.starts_with("enh.") => {
                    s.enh.parse(k, value);
                }
                // L1: the launcher's rows (`launcher.skip`), declared in the
                // same option table so a rewrite keeps them.
                k if k.starts_with("launcher.") => {
                    s.enh.parse(k, value);
                }
                "trax_random" => s.trax_random = value.trim() != "0",
                "trax" => {
                    for (t, v) in s.trax.iter_mut().zip(value.split_whitespace()) {
                        *t = v.parse::<u8>().map_or(*t, |v| v.min(3));
                    }
                }
                _ => {
                    // "p0 camera 3" style per-player keys.
                    let Some(rest) = key.strip_prefix('p') else {
                        continue;
                    };
                    let Some((idx, field)) = rest.split_once('.') else {
                        continue;
                    };
                    let Some(p) = idx.parse::<usize>().ok().and_then(|i| s.players.get_mut(i)) else {
                        continue;
                    };
                    match field {
                        "gauges" => p.gauges = b.unwrap_or(p.gauges),
                        "position" => p.position = b.unwrap_or(p.position),
                        "lap_info" => p.lap_info = b.unwrap_or(p.lap_info),
                        "score" => p.score = b.unwrap_or(p.score),
                        "leaderboard" => p.leaderboard = b.unwrap_or(p.leaderboard),
                        "feedback" => p.feedback = b.unwrap_or(p.feedback),
                        "ask_transmission" => p.ask_transmission = b.unwrap_or(p.ask_transmission),
                        "camera" => p.camera = n.unwrap_or(p.camera).min(4),
                        "split" => p.split = n.unwrap_or(p.split).min(4),
                        "manual" => p.manual = b.unwrap_or(p.manual),
                        "stability_off" => p.stability_off = b.unwrap_or(p.stability_off),
                        "wheel_range" => p.wheel_range = f.unwrap_or(p.wheel_range).clamp(0.1, 1.0),
                        "road_feedback" => p.road_feedback = f.unwrap_or(p.road_feedback).clamp(2.0, 20.0),
                        "force" => p.force = f.unwrap_or(p.force).clamp(0.0, 1.0),
                        _ => {}
                    }
                }
            }
        }
        // 0x5BE6A0 on load: each value at most its card's cap.
        for (v, cap) in s.custom.iter_mut().zip(CUSTOM_CAPS) {
            *v = (*v).min(cap);
        }
        // Old clone keys that duplicated a Customize Display row map onto
        // it; enhanced tiers keep theirs at the top.
        super::graphics_table::normalize(&mut s);
        s
    }

    pub fn save(&self) -> anyhow::Result<()> {
        self.apply_enhancements();
        self.save_to_profile();
        self.write_file()
    }

    /// The profile's options (the players' rows, the audio, the HUD, …:
    /// all but options.txt's) changed in the current profile, in memory:
    /// they reach its file when the profile is saved, as in the original.
    /// [`Settings::save`] without options.txt, for a change that leaves its
    /// text as it is (the transmission question's answer).
    pub fn save_to_profile(&self) {
        super::profiles::update_current(|n| self.write_native(n));
    }

    /// options.txt ([`Settings::options_text`]) written.
    pub(crate) fn write_file(&self) -> anyhow::Result<()> {
        // Through a temporary file and a rename (the folder made first), so
        // a crash or a full disk mid-write leaves the old options.txt whole
        // rather than a cut-off one; the bytes are the same either way.
        let path = Settings::path();
        if let Err(e) = crate::qol::saves::write_atomic(&path, self.options_text().as_bytes(), false) {
            log::warn!("options not saved to {}: {e}", path.display());
            return Err(e.into());
        }
        Ok(())
    }

    /// options.txt's text ([`Settings::save`]).
    pub fn options_text(&self) -> String {
        // Only what the original keeps outside the profile: the display
        // options (registry, 0x5BEEB0) and the key map. The rest lives in
        // the profile alone (see `load`).
        let mut out = String::new();
        out += &format!("resolution {}x{}\n", self.resolution.0, self.resolution.1);
        out += &format!("fullscreen {}\n", u8::from(self.fullscreen));
        out += &format!("aspect {}\n", self.aspect);
        out += &format!("vsync_off {}\n", if self.vsync_off_mailbox { "mailbox" } else { "immediate" });
        out += &format!("detail {}\n", self.detail);
        for (i, v) in self.custom.iter().enumerate() {
            out += &format!("custom.{i} {v}\n");
        }
        for (r, pair) in self.keys.iter().enumerate() {
            for (c, k) in pair.iter().enumerate() {
                out += &format!("key.{r}.{c} {k}\n");
            }
        }
        // The Enhancements page's values that are not native.
        self.enh.write(&mut out);
        self.overlay.write(&mut out);
        for (i, v) in self.custom_raised.iter().enumerate() {
            if let Some(v) = v {
                out += &format!("enh.raised.{i} {v}\n");
            }
        }
        out
    }
}
