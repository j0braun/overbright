//! "Mark This Spot" (clone tooling; no native counterpart): an opt-in
//! hotkey (Enhancements page, Mark Spot Key, default Off) that writes what
//! the player sees — the camera, the car's pose, the streaming section,
//! the nearest scenery solid, the world clock, the settings and their hash
//! — to a report file under the data folder, with the `--shot` / `--bench`
//! command lines that rebuild the frame, so the report a player sends can
//! be reproduced exactly. docs/playing/mark-spot.md.

use std::path::{Path, PathBuf};

pub use crate::extras::MARK_KEYS as KEYS;

/// Where the reports go: `platform::dirs::marks()` (`<data home>/overbright/marks/`).
pub fn dir() -> PathBuf {
    crate::platform::dirs::marks()
}

/// The camera: eye, a point 10 m ahead, the vertical FOV and the game
/// camera's own FOV (degrees; `Camera::game_fov`), the up vector.
#[derive(Clone, Copy, Debug, Default)]
pub struct Look {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub fov_deg: f32,
    pub game_fov_deg: Option<f32>,
    pub up: [f32; 3],
}

impl Look {
    /// NFSU2_SHOT_LOOK's value.
    pub fn shot_look(&self) -> String {
        format!("{},{},{:.4},{:.4},{:.5},{:.5},{:.5}", v3(self.eye), v3(self.target), self.fov_deg, self.game_fov_deg.unwrap_or(0.0), self.up[0], self.up[1], self.up[2])
    }
}

/// What the game saw when the key was pressed.
#[derive(Clone, Debug, Default)]
pub struct Spot {
    /// "explore", "race", "menu", "scene".
    pub mode: String,
    pub region: Option<String>,
    /// The career / quick race event's name.
    pub event: Option<String>,
    /// The player car's type name (as NFSU2_SHOT_CAR takes it).
    pub car: Option<String>,
    /// The car's position (world, Z up), heading (degrees from +X, as
    /// NFSU2_SHOT_AT takes it) and speed (km/h).
    pub position: Option<[f32; 3]>,
    pub heading_deg: f32,
    pub speed_kmh: f32,
    /// The camera.
    pub camera: Option<Look>,
    /// The streaming section drawn at the car.
    pub section: Option<u32>,
    /// The nearest scenery placement: solid name, distance (m), position.
    pub nearest: Option<(String, f32, [f32; 3])>,
    /// The world clock 0x7FB718 (s), what NFSU2_RAIN_CLOCK sets.
    pub world_clock: Option<f32>,
    /// The game clock (s since start).
    pub game_time: f64,
    /// The window's size.
    pub size: (u32, u32),
    /// options.txt's text (the settings in force).
    pub options: String,
    /// The current profile's native save as it is in memory, if one is
    /// loaded (written next to the report: NFSU2_SHOT_SAVE loads it, with
    /// the car's parts and paint).
    pub profile_save: Option<Vec<u8>>,
    /// The game files' fingerprint summary (`fingerprint::current`).
    pub game_files: Option<String>,
}

/// The settings' hash: XXH3-64 of options.txt's text.
pub fn settings_hash(options: &str) -> u64 {
    xxhash_rust::xxh3::xxh3_64(options.as_bytes())
}

/// The `NFSU2_SHOT_ENH` value for these options: every `enh.*` line (not
/// the per-row raised tiers) and `custom.N` line, `;`-separated.
pub fn shot_enh(options: &str) -> String {
    options
        .lines()
        .filter(|l| (l.starts_with("enh.") && !l.starts_with("enh.raised.")) || l.starts_with("custom."))
        .collect::<Vec<_>>()
        .join(";")
}

/// The World Detail level in these options (`detail N`).
pub fn detail(options: &str) -> Option<u8> {
    options.lines().find_map(|l| l.strip_prefix("detail ")).and_then(|v| v.trim().parse().ok())
}

fn v3(v: [f32; 3]) -> String {
    format!("{:.3},{:.3},{:.3}", v[0], v[1], v[2])
}

/// Shell-quote a value for the command lines.
fn q(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// A path for the command lines: double-quoted, the home folder as
/// `$HOME` (`%USERPROFILE%` on Windows; the report itself shows home paths
/// as `~`).
fn q_path(p: &Path) -> String {
    let mut s = p.to_string_lossy().into_owned();
    for (h, token) in crate::platform::dirs::home_tokens(&|name| std::env::var(name).ok()) {
        if let Some(rest) = s.strip_prefix(h.as_str()) {
            s = format!("{token}{rest}");
            break;
        }
    }
    let esc: String = s.chars().flat_map(|c| if matches!(c, '"' | '\\' | '`') { vec!['\\', c] } else { vec![c] }).collect();
    format!("\"{esc}\"")
}

/// The world time `--shot explore` runs after placing the car (90 updates
/// at 1/30 s): the command's NFSU2_RAIN_CLOCK is the marked clock minus
/// this, so the shot's world clock ends where the mark's was.
pub const SHOT_SETTLE_S: f32 = 3.0;

impl Spot {
    /// The environment of the reproducing commands (without the command).
    pub fn repro_env(&self, save: Option<&Path>) -> Vec<String> {
        let mut env = Vec::new();
        match save {
            Some(p) => env.push(format!("NFSU2_SHOT_SAVE={}", q_path(p))),
            None => {
                if let Some(c) = &self.car {
                    env.push(format!("NFSU2_SHOT_CAR={c}"));
                }
            }
        }
        if let Some(p) = self.position {
            env.push(format!("NFSU2_SHOT_AT={},{:.1}", v3(p), self.heading_deg));
            env.push("NFSU2_SHOT_HOLD=1".into());
        }
        if let Some(look) = self.camera {
            env.push(format!("NFSU2_SHOT_LOOK={}", look.shot_look()));
        }
        if let Some(c) = self.world_clock {
            env.push(format!("NFSU2_RAIN_CLOCK={:.3}", (c - SHOT_SETTLE_S).max(0.0)));
        }
        env.push(format!("NFSU2_SHOT_SIZE={}x{}", self.size.0, self.size.1));
        let enh = shot_enh(&self.options);
        if !enh.is_empty() {
            env.push(format!("NFSU2_SHOT_ENH={}", q(&enh)));
        }
        // The shot's screen passes follow the World Detail level
        // (NFS_DETAIL, default 5), not the custom rows.
        if let Some(d) = detail(&self.options).filter(|&d| d != 5) {
            env.push(format!("NFS_DETAIL={d}"));
        }
        env
    }

    /// The report's text. `stem` names the report (mark-<time>), `save`
    /// the copied profile save, if any.
    pub fn report(&self, stem: &str, save: Option<&Path>, when_utc: &str) -> String {
        let mut s = String::new();
        s += &format!("Overbright {} mark-this-spot report {stem}\n", env!("CARGO_PKG_VERSION"));
        s += &format!("time (UTC): {when_utc}\n");
        s += "note: (describe what is wrong here, then send this file)\n\n";
        s += &format!("mode: {}\n", self.mode);
        if let Some(r) = &self.region {
            s += &format!("region: {r}\n");
        }
        if let Some(e) = &self.event {
            s += &format!("event: {e}\n");
        }
        if let Some(c) = &self.car {
            s += &format!("car: {c}\n");
        }
        if let Some(p) = self.position {
            s += &format!("car position: {} heading {:.1} deg (from +x) speed {:.1} km/h\n", v3(p), self.heading_deg, self.speed_kmh);
        }
        if let Some(c) = self.camera {
            s += &format!("camera: eye {} looking at {} fov {:.2} deg{} up {:.3},{:.3},{:.3}\n", v3(c.eye), v3(c.target), c.fov_deg, c.game_fov_deg.map_or(String::new(), |g| format!(" (game camera {g:.2} deg)")), c.up[0], c.up[1], c.up[2]);
        }
        if let Some(sec) = self.section {
            s += &format!("section: {sec}\n");
        }
        if let Some((name, d, p)) = &self.nearest {
            s += &format!("nearest asset: {name} at {} ({d:.1} m)\n", v3(*p));
        }
        if let Some(c) = self.world_clock {
            s += &format!("world clock: {c:.3} s\n");
        }
        s += &format!("game time: {:.3} s\n", self.game_time);
        s += &format!("window: {}x{}\n", self.size.0, self.size.1);
        s += &format!("settings hash: {:016x} (xxh3 of options.txt)\n", settings_hash(&self.options));
        if let Some(g) = &self.game_files {
            s += &format!("game files: {g}\n");
        }
        if let Some(p) = save {
            s += &format!("profile save copied: {}\n", p.display());
        }
        let env = self.repro_env(save).join(" \\\n  ");
        s += "\nreproduce (headless; the shot loads free roam, puts the car at its position standing, lets it settle 3 s, then sets the camera; the camera and the world are exact, the car's speed is not):\n";
        s += &format!("  {env} \\\n  game --shot explore {stem}.png\n");
        if let Some(e) = &self.event {
            s += &format!("the race itself (start of the event, not this moment):\n  NFSU2_SHOT_EVENT={e} game --shot race {stem}-race.png\n");
        }
        let car = self.car.as_deref().unwrap_or("PEUGOT");
        let detail = detail(&self.options).map_or(String::new(), |d| format!("NFSU2_BENCH_DETAIL={d} "));
        s += &format!("performance with these settings (the bench's own free-roam route, not this spot):\n  NFSU2_BENCH_CAR={car} {detail}NFSU2_BENCH_ENH={} game --bench explore 600\n", q(&shot_enh(&self.options)));
        s += "\noptions.txt:\n";
        s += &self.options;
        crate::qol::crash::scrub(&s)
    }
}

/// Write the report (and a copy of the profile save) under [`dir`];
/// returns the report's path.
pub fn write(spot: &Spot) -> std::io::Result<PathBuf> {
    let d = dir();
    std::fs::create_dir_all(&d)?;
    let now = crate::qol::saves::timestamp(std::time::SystemTime::now());
    // A free name: two marks in one second get -2, -3 ….
    let mut stem = format!("mark-{now}");
    let mut n = 2;
    while d.join(format!("{stem}.txt")).exists() {
        stem = format!("mark-{now}-{n}");
        n += 1;
    }
    let save = match &spot.profile_save {
        Some(bytes) => {
            let to = d.join(format!("{stem}.sav"));
            std::fs::write(&to, bytes).ok().map(|_| to)
        }
        None => None,
    };
    let path = d.join(format!("{stem}.txt"));
    crate::qol::saves::write_atomic(&path, spot.report(&stem, save.as_deref(), &now).as_bytes(), false)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spot() -> Spot {
        Spot {
            mode: "explore".into(),
            region: Some("L4RA".into()),
            car: Some("COROLLA".into()),
            position: Some([1.0, 2.0, 3.0]),
            heading_deg: 90.0,
            speed_kmh: 42.0,
            camera: Some(Look { eye: [0.0, -5.0, 4.0], target: [1.0, 2.0, 3.0], fov_deg: 55.0, game_fov_deg: Some(60.0), up: [0.0, 0.0, 1.0] }),
            section: Some(2104),
            nearest: Some(("XB_LAMP_1".into(), 3.5, [2.0, 2.0, 3.0])),
            world_clock: Some(162.0),
            game_time: 12.5,
            size: (1920, 1080),
            options: "resolution 1920x1080\ndetail 3\nenh.hud_scale 120\nenh.raised.3 2\ncustom.7 0\nkey.0.0 W\n".into(),
            ..Default::default()
        }
    }

    #[test]
    fn the_report_holds_the_pose_and_the_shot_command() {
        let r = spot().report("mark-x", None, "2026-11-05_12-00-00");
        for want in [
            "note:",
            "car position: 1.000,2.000,3.000 heading 90.0",
            "section: 2104",
            "nearest asset: XB_LAMP_1",
            "world clock: 162.000",
            "NFSU2_SHOT_CAR=COROLLA",
            "NFSU2_SHOT_AT=1.000,2.000,3.000,90.0",
            "NFSU2_SHOT_LOOK=0.000,-5.000,4.000,1.000,2.000,3.000,55.0000,60.0000,0.00000,0.00000,1.00000",
            "NFSU2_RAIN_CLOCK=159.000",
            "NFSU2_SHOT_HOLD=1",
            "NFSU2_SHOT_SIZE=1920x1080",
            "NFSU2_SHOT_ENH='enh.hud_scale 120;custom.7 0'",
            "game --shot explore mark-x.png",
            "NFSU2_BENCH_DETAIL=3 NFSU2_BENCH_ENH='enh.hud_scale 120;custom.7 0' game --bench explore",
            "NFS_DETAIL=3",
            "settings hash:",
        ] {
            assert!(r.contains(want), "{want} missing in\n{r}");
        }
        assert!(!r.contains("NFSU2_SHOT_ENH='enh.raised"));
    }

    #[test]
    fn a_save_replaces_the_car_and_quotes_survive() {
        let env = spot().repro_env(Some(Path::new("/x/it\"s.sav")));
        assert_eq!(env[0], "NFSU2_SHOT_SAVE=\"/x/it\\\"s.sav\"");
        let enh = spot().repro_env(None).into_iter().find(|e| e.starts_with("NFSU2_SHOT_ENH")).unwrap();
        assert_eq!(enh, "NFSU2_SHOT_ENH='enh.hud_scale 120;custom.7 0'");
        assert!(!env.iter().any(|e| e.starts_with("NFSU2_SHOT_CAR")));
    }

    #[test]
    fn settings_hash_follows_the_text() {
        assert_eq!(settings_hash("a"), settings_hash("a"));
        assert_ne!(settings_hash("a"), settings_hash("b"));
    }

    #[test]
    fn write_puts_report_and_save_copy_in_the_data_folder() {
        crate::frontend::profiles::sandbox::enter(&std::env::temp_dir().join(format!("nfsu2-mark-{}", std::process::id())));
        let mut s = spot();
        s.profile_save = Some(b"SAVE".to_vec());
        let a = write(&s).unwrap();
        let b = write(&s).unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with(dir()));
        let text = std::fs::read_to_string(&a).unwrap();
        assert!(text.contains("NFSU2_SHOT_SAVE="), "{text}");
        assert_eq!(std::fs::read(a.with_extension("sav")).unwrap(), b"SAVE");
    }
}
