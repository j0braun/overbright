//! Sharing a mod (docs/modding/studio.md, "The Mod page"): what a shared copy holds, whether the mod is ready to
//! share, and the export as one `.zip` (or a folder) that imports on another computer.
//!
//! - **What is shared** ([`files`]): the mod's folder without its work files ([`SKIPPED`]: `work/`, the editor's
//!   `.editor/` and every other name starting with a dot). The map (`map.obmap`) is the map-mod format the
//!   launcher and the editor already read; the texture list and the pack data (`looks.toml`, `textures/…`) come
//!   along unchanged.
//! - **The `.zip`** holds `<folder>/<file>` for each; the launcher's, the map editor's and Studio's Import take it
//!   and bring every shared file back ([`import`]). The copy itself is the map data's
//!   (`ob_map::tools::map::share`): this module adds the build, and the "Ready to share" list.
//! - **Provenance** ([`PROVENANCE`]): every export writes `provenance.toml` beside the map: the build that wrote
//!   it and the textures whose pixels started from the original game's ([`crate::looks::Origin::Game`]).
//!   Exporting a mod with such textures needs the author's yes ([`Refused::Derived`]); the shared copy says so.

use std::path::Path;

use crate::session::Level;
use crate::validate::Check;
use ob_map::tools::map::format::MapMod;
// The shared copy itself is the map data's (the launcher and the map editor export and import the same way).
pub use ob_map::tools::map::share::{
    Derived, Exported, File, FORMAT, MAX_BYTES, MAX_FILES, PROVENANCE, Refused, SKIPPED, default_zip, derived,
    derived_warning, files, import, safe, shared, textures,
};

/// The build that writes a file: `Overbright 0.5.0 (build 0123456789abcdef)`.
pub fn build() -> String {
    format!("Overbright {} (build {:016x})", env!("CARGO_PKG_VERSION"), nfsu2::studio_api::build::BUILD_ID.0)
}

/// The text of `provenance.toml`, written by this build.
pub fn provenance(derived: &[Derived]) -> String {
    ob_map::tools::map::share::provenance(&build(), derived)
}

/// Why the mod is (not yet) ready to share.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub level: Level,
    /// It passed.
    pub ok: bool,
    pub text: String,
}

/// The "Ready to share" list: the map checks (`None`: they could not run, the game's race tracks are not known),
/// the name, the derived textures (a warning, never a block). The tested mark comes with the proof runs.
pub fn ready(map: &MapMod, checks: Option<&[Check]>, derived: usize) -> Vec<Point> {
    let mut v = Vec::new();
    match checks {
        Some(c) => {
            let errors = c.iter().filter(|c| c.level == Level::Error && !c.ok).count();
            let warnings = c.iter().filter(|c| c.level == Level::Warning && !c.ok).count();
            v.push(Point {
                level: Level::Error,
                ok: errors == 0,
                text: match (errors, warnings) {
                    (0, 0) => "Map checks pass".into(),
                    (0, w) => format!("Map checks pass · {}", plural(w, "warning")),
                    (e, _) => format!("Map checks: {}", plural(e, "error")),
                },
            });
        }
        None if map.races.is_empty() => {
            v.push(Point { level: Level::Error, ok: true, text: "Map checks pass".into() });
        }
        None => v.push(Point {
            level: Level::Error,
            ok: false,
            text: "Map checks need the game's race tracks (set the game folder in the launcher)".into(),
        }),
    }
    v.push(Point {
        level: Level::Error,
        ok: !map.name.trim().is_empty(),
        text: if map.name.trim().is_empty() { "Give the mod a name".into() } else { "Named".into() },
    });
    if derived > 0 {
        v.push(Point {
            level: Level::Warning,
            ok: false,
            text: format!("{} from the original game's pixels", plural(derived, "texture")),
        });
    }
    v
}

/// No error in `points`: the mod may be exported.
pub fn is_ready(points: &[Point]) -> bool {
    points.iter().all(|p| p.ok || p.level != Level::Error)
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 { format!("1 {word}") } else { format!("{n} {word}s") }
}

/// Export the mod in `<maps>/<folder>` to `to`: a `.zip` (`<folder>/<file>` entries; an existing file is
/// replaced) or, for any other name, a new or empty folder. `derived_ok`: the author said yes to sharing textures
/// made from the original game's pixels.
pub fn export(maps: &Path, folder: &str, to: &Path, derived_ok: bool) -> Result<Exported, Refused> {
    ob_map::tools::map::share::export(maps, folder, to, derived_ok, &build())
}

/// [`export`] of the mod in `dir`, its entries in the `.zip` under `folder/`.
pub fn export_dir(dir: &Path, folder: &str, to: &Path, derived_ok: bool) -> Result<Exported, Refused> {
    ob_map::tools::map::share::export_dir(dir, folder, to, derived_ok, &build())
}

#[cfg(test)]
mod tests;
