//! Optional map calibration supplied with an external texture pack.
use crate::frontend::career::Track;
use std::{collections::HashMap, path::Path};

/// Validate the whole sidecar without filesystem access or mutation. Values
/// retain Binary's world coordinates and 512-reference menu units.
/// Each row is (track number, field index, value).
pub(crate) fn parse(text: &str) -> anyhow::Result<Vec<(u16, usize, f32)>> {
    use anyhow::ensure;
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let parts: Vec<_> = line.split_whitespace().collect();
        ensure!(parts.len() == 3, "map override requires track, field, value");
        let track: u16 = parts[0].parse()?;
        let value: f32 = parts[2].parse()?;
        ensure!(value.is_finite(), "nonfinite map override");
        let field = match parts[1] {
            "TrackMapCalibrationOffsetX" => 0,
            "TrackMapCalibrationOffsetY" => 1,
            "TrackMapCalibrationZoomIn" => {
                ensure!(value > 0.0, "map zoom must be positive");
                2
            }
            "MenuMapZoomOffsetX" => 3,
            "MenuMapZoomOffsetY" => 4,
            "MenuMapZoomWidth" => {
                ensure!(value >= 0.0, "map window cannot be negative");
                5
            }
            other => anyhow::bail!("unknown map override field: {other}"),
        };
        ensure!(seen.insert((track, field)), "duplicate map override");
        rows.push((track, field, value));
    }
    Ok(rows)
}

/// Parse before changing anything. Generic packs may override just one field;
/// the combined HD UI pack's completeness requirements live in hd_ui_pack.
pub(crate) fn apply(root: &Path, tracks: &mut HashMap<u16, Track>) -> anyhow::Result<usize> {
    use anyhow::Context;
    let path = root.join("map-overrides.tsv");
    if !path.exists() {
        return Ok(0);
    }
    let text = std::fs::read_to_string(&path).context("map overrides")?;
    let rows = parse(&text)?;
    let mut applied = 0;
    for (number, field, value) in rows {
        let name = format!("TRACKMAP{number}");
        let hash = crate::formats::hash::bin_hash(&name);
        if !root.join(format!("{hash:08X}.png")).is_file() && !root.join(format!("{name}.png")).is_file() {
            continue;
        }
        let Some(track) = tracks.get_mut(&number) else { continue };
        match field {
            0 => track.map_left = value,
            1 => track.map_top = value,
            2 => track.map_zoom = value,
            3..=5 => track.map_window[field - 3] = value,
            // `parse` gives fields 0..=5 only.
            _ => unreachable!(),
        }
        applied += 1;
    }
    Ok(applied)
}

/// docs/modding/pack-hot-reload.md: each track's six calibrated values as the
/// game data has them, kept at load so a live HD UI Art change can undo
/// the pack's calibration.
static NATIVE: std::sync::Mutex<Option<HashMap<u16, [f32; 6]>>> = std::sync::Mutex::new(None);

fn fields(t: &Track) -> [f32; 6] {
    [t.map_left, t.map_top, t.map_zoom, t.map_window[0], t.map_window[1], t.map_window[2]]
}

/// Called by the career data's load before any pack applies.
pub(crate) fn remember_native(tracks: &HashMap<u16, Track>) {
    let native = tracks.iter().map(|(n, t)| (*n, fields(t))).collect();
    *NATIVE.lock().unwrap_or_else(|e| e.into_inner()) = Some(native);
}

/// The tracks' calibration put back to the game data's, then the pack's
/// at `root` (if any) applied over it. Returns the overrides applied.
pub fn reapply(root: Option<&Path>, tracks: &mut HashMap<u16, Track>) -> anyhow::Result<usize> {
    if let Some(native) = NATIVE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        for (n, t) in tracks.iter_mut() {
            if let Some(v) = native.get(n) {
                (t.map_left, t.map_top, t.map_zoom) = (v[0], v[1], v[2]);
                t.map_window = [v[3], v[4], v[5]];
            }
        }
    }
    match root {
        Some(root) => apply(root, tracks),
        None => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn map_metadata_parser_validates_without_requiring_a_complete_pack() {
        assert_eq!(
            parse("# generic override\n4701 TrackMapCalibrationOffsetX 160\n4701 MenuMapZoomWidth 200\n").unwrap(),
            [(4701, 0, 160.0), (4701, 5, 200.0)]
        );
        for text in [
            "4701 TrackMapCalibrationOffsetX",
            "4701 unknown 1",
            "4701 TrackMapCalibrationZoomIn 0",
            "4701 MenuMapZoomWidth -1",
            "4701 TrackMapCalibrationOffsetX NaN",
            "4701 TrackMapCalibrationOffsetX 1\n4701 TrackMapCalibrationOffsetX 2\n",
        ] {
            assert!(parse(text).is_err(), "invalid metadata: {text}");
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn map_pack_alignment_requires_texture_and_is_transactional() {
        // Real game assets (ignored without NFSU2_ROOT).
        let root = crate::testkit::game_root();
        let mut data = crate::frontend::career::CareerData::load(&root).expect("real game assets required");
        let path = std::env::temp_dir().join(format!("nfsu2-map-test-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let old = data.tracks[&4701].map_left;
        std::fs::write(
            path.join("map-overrides.tsv"),
            "4701\tTrackMapCalibrationOffsetX\t160\n4701\tMenuMapZoomWidth\t200\n",
        )
        .unwrap();
        assert_eq!(apply(&path, &mut data.tracks).unwrap(), 0);
        assert_eq!(data.tracks[&4701].map_left, old);
        let hash = crate::formats::hash::bin_hash("TRACKMAP4701");
        std::fs::write(path.join(format!("{hash:08X}.png")), b"fixture").unwrap();
        assert_eq!(apply(&path, &mut data.tracks).unwrap(), 2);
        assert_eq!(data.tracks[&4701].map_left, 160.0);
        assert_eq!(data.tracks[&4701].map_window[2], 200.0);
        std::fs::write(
            path.join("map-overrides.tsv"),
            "4701\tTrackMapCalibrationOffsetX\t999\n4701\tTrackMapCalibrationZoomIn\tNaN\n",
        )
        .unwrap();
        assert!(apply(&path, &mut data.tracks).is_err());
        assert_eq!(data.tracks[&4701].map_left, 160.0, "invalid metadata cannot partially apply");
        // A live change puts the game data's values back, then the pack's.
        assert_eq!(reapply(None, &mut data.tracks).unwrap(), 0);
        assert_eq!(data.tracks[&4701].map_left, old);
        std::fs::write(path.join("map-overrides.tsv"), "4701\tTrackMapCalibrationOffsetX\t170\n").unwrap();
        assert_eq!(reapply(Some(&path), &mut data.tracks).unwrap(), 1);
        assert_eq!(data.tracks[&4701].map_left, 170.0);
        std::fs::remove_dir_all(path).unwrap();
    }
}
