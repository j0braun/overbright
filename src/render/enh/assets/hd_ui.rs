//! Startup selection of the optional HD HUD and map pack. Changes require
//! restart, unless Reload Packs When Changed (docs/pack-hot-reload.md)
//! puts the row in force live ([`set_live`]) and re-checks the folder
//! ([`recheck`]).
use std::{
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};

static CANDIDATE: OnceLock<Option<PathBuf>> = OnceLock::new();
/// The candidate's completeness: 0 not checked yet, 1 incomplete, 2 complete.
static COMPLETE: AtomicU8 = AtomicU8::new(0);
static ACTIVE: OnceLock<Option<PathBuf>> = OnceLock::new();
/// 0 the startup choice ([`ACTIVE`]), 1 off, 2 on (live).
static LIVE: AtomicU8 = AtomicU8::new(0);

/// `NFSU2_TEXTURE_PACK`, else the folder kept by the launcher (L1,
/// `qol::setup::saved_texture_pack`), else `texture-pack` beside the exe.
fn candidate() -> Option<PathBuf> {
    match std::env::var_os("NFSU2_TEXTURE_PACK") {
        Some(p) => (!p.is_empty()).then(|| PathBuf::from(p)),
        None => crate::qol::setup::saved_texture_pack()
            .map(|p| crate::platform::dirs::load_path(&p))
            .or_else(|| std::env::current_exe().ok()?.parent().map(|p| p.join("texture-pack"))),
    }
}

/// Whether `root` holds the complete HD UI pack (the same test as the
/// game's at startup); for the launcher's texture-pack status.
pub fn is_complete(root: &Path) -> bool {
    complete(root)
}

fn complete(root: &Path) -> bool {
    let Ok(index) = std::fs::read_to_string(root.join("hd-ui-pack.list")) else { return false };
    let mut lines = index.lines();
    if lines.next() != Some("NFSU2 HD UI pack v1") {
        return false;
    }
    if !["manifest.json", "map-manifest.json", "map-overrides.tsv"].iter().all(|p| root.join(p).is_file()) {
        return false;
    }
    let Ok(metadata) = std::fs::read_to_string(root.join("map-overrides.tsv")) else { return false };
    let Ok(rows) = crate::frontend::map_pack::parse(&metadata) else { return false };
    let tracks: std::collections::HashSet<_> = rows.iter().map(|r| r.0).collect();
    // The v1 combined pack must carry all of its calibration, not merely a
    // syntactically valid fragment. Generic pack application remains separate.
    if rows.len() != 88 || tracks.len() != 27 {
        return false;
    }
    let mut names = std::collections::HashSet::new();
    for name in lines {
        if name.len() != 12 || !name.ends_with(".png") || !name.as_bytes()[..8].iter().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
        if !image::image_dimensions(root.join(name)).is_ok_and(|(w, h)| w > 0 && h > 0 && w <= 4096 && h <= 4096) {
            return false;
        }
        if !names.insert(name) {
            return false;
        }
    }
    // This pack version contains 595 HUD images plus 115 maps.
    names.len() == 710
        && tracks.iter().all(|number| {
            let hash = crate::formats::hash::bin_hash(&format!("TRACKMAP{number}"));
            names.contains(format!("{hash:08X}.png").as_str())
        })
}

/// What a HUD mod folder holds, for the launcher's check line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inspect {
    /// The folder has the pack's index (`hd-ui-pack.list`, v1).
    pub is_pack: bool,
    /// Images the index names, and of them the ones present.
    pub listed: usize,
    pub present: usize,
    /// Indexed images not in the folder (at most 5 named).
    pub missing: Vec<String>,
    /// The map calibration files are there and complete.
    pub maps: bool,
    /// The game's own test ([`is_complete`]).
    pub complete: bool,
}

/// [`Inspect`] for `root` (reads the index and looks for each file; does
/// not decode the images, which [`is_complete`] does).
pub fn inspect(root: &Path) -> Inspect {
    let mut i = Inspect::default();
    let Ok(index) = std::fs::read_to_string(root.join("hd-ui-pack.list")) else { return i };
    let mut lines = index.lines();
    if lines.next() != Some("NFSU2 HD UI pack v1") {
        return i;
    }
    i.is_pack = true;
    for name in lines.filter(|l| !l.trim().is_empty()) {
        i.listed += 1;
        if root.join(name).is_file() {
            i.present += 1;
        } else if i.missing.len() < 5 {
            i.missing.push(name.to_string());
        }
    }
    i.maps = ["manifest.json", "map-manifest.json", "map-overrides.tsv"].iter().all(|p| root.join(p).is_file())
        && std::fs::read_to_string(root.join("map-overrides.tsv"))
            .ok()
            .and_then(|t| crate::frontend::map_pack::parse(&t).ok())
            .is_some_and(|rows| rows.len() == 88);
    i.complete = complete(root);
    i
}

/// The pack folder the game looks in (`NFSU2_TEXTURE_PACK`, the folder
/// kept by the launcher, or `texture-pack` beside the game), complete or
/// not.
pub fn candidate_root() -> Option<&'static Path> {
    CANDIDATE.get_or_init(candidate).as_deref()
}

fn available() -> Option<&'static Path> {
    let root = candidate_root()?;
    if COMPLETE.load(Ordering::Relaxed) == 0 {
        COMPLETE.store(if complete(root) { 2 } else { 1 }, Ordering::Relaxed);
    }
    (COMPLETE.load(Ordering::Relaxed) == 2).then_some(root)
}

/// Checks the folder again (it appeared, went or changed); true when
/// its completeness changed.
pub fn recheck() -> bool {
    let Some(root) = candidate_root() else { return false };
    let now = if complete(root) { 2 } else { 1 };
    COMPLETE.swap(now, Ordering::Relaxed) != now
}

/// The HD UI Art row in force from now on (Reload Packs When Changed),
/// instead of the startup choice.
pub fn set_live(enabled: bool) {
    LIVE.store(if enabled { 2 } else { 1 }, Ordering::Relaxed);
}

/// Called before the game loads calibration or uploads any textures.
pub fn initialize(enabled: bool) {
    ACTIVE.get_or_init(|| enabled.then(|| available().map(Path::to_path_buf)).flatten());
}

pub fn installed() -> bool {
    available().is_some()
}
pub fn active() -> bool {
    active_root().is_some()
}

/// Non-game asset tools may explicitly select an arbitrary pack with the env var.
/// The game always calls initialize first, so its saved setting remains authoritative.
pub fn active_root() -> Option<&'static Path> {
    match LIVE.load(Ordering::Relaxed) {
        1 => return None,
        2 => return available(),
        _ => {}
    }
    ACTIVE
        .get_or_init(|| std::env::var_os("NFSU2_TEXTURE_PACK").filter(|p| !p.is_empty()).map(PathBuf::from))
        .as_deref()
}

fn status_for(requested: bool, installed: bool, active: bool) -> &'static str {
    if requested && !installed {
        "Pack missing or incomplete"
    } else if requested != active {
        "Restart required"
    } else if active {
        "On"
    } else {
        "Off"
    }
}
pub fn status(requested: bool) -> &'static str {
    status_for(requested, installed(), active())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hd_pack_status_distinguishes_missing_and_pending() {
        assert_eq!(status_for(true, false, false), "Pack missing or incomplete");
        assert_eq!(status_for(true, true, false), "Restart required");
        assert_eq!(status_for(false, true, true), "Restart required");
        assert_eq!(status_for(true, true, true), "On");
        assert_eq!(status_for(false, false, false), "Off");
    }
    #[test]
    fn hd_pack_missing_file_is_not_installed() {
        let p = std::env::temp_dir().join(format!("hd-pack-install-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        assert!(!complete(&p));
        for name in ["manifest.json", "map-manifest.json", "map-overrides.tsv"] {
            std::fs::write(p.join(name), "{}").unwrap();
        }
        std::fs::write(p.join("hd-ui-pack.list"), "NFSU2 HD UI pack v1\n12345678.png\n").unwrap();
        assert!(!complete(&p));
        image::RgbaImage::new(4, 4).save(p.join("12345678.png")).unwrap();
        assert!(!complete(&p), "a truncated index is not a complete pack");
        let mut metadata = String::new();
        let mut files = Vec::new();
        for track in 4700..4727 {
            for field in ["TrackMapCalibrationOffsetX", "TrackMapCalibrationOffsetY", "TrackMapCalibrationZoomIn"] {
                metadata.push_str(&format!("{track} {field} 1\n"));
            }
            if track < 4707 {
                metadata.push_str(&format!("{track} MenuMapZoomWidth 200\n"));
            }
            let hash = crate::formats::hash::bin_hash(&format!("TRACKMAP{track}"));
            files.push(format!("{hash:08X}.png"));
        }
        assert_eq!(metadata.lines().count(), 88);
        files.extend((0..683).map(|n| format!("{n:08X}.png")));
        let mut index = String::from("NFSU2 HD UI pack v1\n");
        for name in &files {
            image::RgbaImage::new(1, 1).save(p.join(name)).unwrap();
            index.push_str(name);
            index.push('\n');
        }
        std::fs::write(p.join("hd-ui-pack.list"), &index).unwrap();
        assert!(!complete(&p), "malformed calibration must not enable HD map images");
        std::fs::write(p.join("map-overrides.tsv"), &metadata).unwrap();
        assert!(complete(&p));
        for invalid in [
            metadata.lines().take(87).collect::<Vec<_>>().join("\n"),
            metadata.replacen("4700 MenuMapZoomWidth 200", "4700 TrackMapCalibrationOffsetX 2", 1),
            metadata.replacen("TrackMapCalibrationZoomIn 1", "TrackMapCalibrationZoomIn NaN", 1),
            metadata.replacen("4700 MenuMapZoomWidth", "4800 MenuMapZoomWidth", 1),
        ] {
            std::fs::write(p.join("map-overrides.tsv"), invalid).unwrap();
            assert!(!complete(&p), "truncated, duplicate, nonfinite or wrong-track-count metadata is incomplete");
        }
        std::fs::write(p.join("map-overrides.tsv"), &metadata).unwrap();
        // Keep 710 valid indexed images, but leave one calibration's map out.
        image::RgbaImage::new(1, 1).save(p.join("00010000.png")).unwrap();
        std::fs::write(p.join("hd-ui-pack.list"), index.replacen(&files[0], "00010000.png", 1)).unwrap();
        assert!(!complete(&p), "each calibrated map must be in the validated inventory");
        std::fs::write(p.join("hd-ui-pack.list"), &index).unwrap();
        assert!(complete(&p));
        std::fs::write(p.join("hd-ui-pack.list"), format!("{index}00000000.png\n")).unwrap();
        assert!(!complete(&p), "duplicate inventory entries are invalid");
        std::fs::write(p.join("hd-ui-pack.list"), &index).unwrap();
        std::fs::remove_file(p.join("00000000.png")).unwrap();
        assert!(!complete(&p), "a missing indexed image is detected");
        std::fs::remove_file(p.join("12345678.png")).unwrap();
        assert!(!complete(&p));
        std::fs::remove_dir_all(p).unwrap();
    }
}
