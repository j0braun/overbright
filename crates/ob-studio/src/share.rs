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

/// The "Ready to share" list's lines for the mod's textures in `dir` (docs/modding/studio-looks.md): each texture
/// the list (`looks.toml`) names must be in the pack data the game reads (`textures/…`), as the picture the list
/// names, in a size the game takes; else the shared copy would lack it or carry another picture. None with no
/// textures.
pub fn looks(dir: &Path) -> Vec<Point> {
    use crate::looks::{FILE, Looks, PACK_DIR, image_hash, work};
    let text = std::fs::read_to_string(dir.join(FILE)).unwrap_or_default();
    let list = match Looks::parse(&text) {
        Ok(l) => l,
        Err(e) => {
            return vec![Point {
                level: Level::Error,
                ok: false,
                text: format!("The texture list does not read ({e})"),
            }];
        }
    };
    if list.entries.is_empty() {
        return Vec::new();
    }
    let mut v = Vec::new();
    for e in list.entries.values() {
        let name = if e.name.is_empty() { e.id.to_string() } else { e.name.clone() };
        let file = format!("{PACK_DIR}/{}", e.id.pack_file());
        let why = match std::fs::read(dir.join(&file)) {
            Err(_) => Some(format!("Texture {name}: its picture is missing ({file})")),
            Ok(png) => match work::check(&png) {
                Err(err) => Some(format!("Texture {name}: its picture is {err}")),
                Ok(_) if image_hash(&png) != e.image => {
                    Some(format!("Texture {name}: its picture ({file}) is not the one the texture list names"))
                }
                Ok(_) => None,
            },
        };
        if let Some(text) = why {
            v.push(Point { level: Level::Error, ok: false, text });
        }
    }
    if v.is_empty() {
        v.push(Point {
            level: Level::Error,
            ok: true,
            text: format!("{} in place", plural(list.entries.len(), "texture")),
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

// ---- mod packs ----------------------------------------------------------------------------------------------------

pub use nfsu2::studio_api::mod_packs::{Converted, Exported as PackExported};

/// What the author chooses for a pack file.
#[derive(Clone, Debug)]
pub struct PackOpts {
    pub version: semver::Version,
    /// An SPDX expression; without one the pack is `local-only` (a public pack needs a licence).
    pub licence: Option<String>,
    /// The yes to textures made from the original game's pixels (the pack is then `local-only`).
    pub derived_ok: bool,
}

impl Default for PackOpts {
    fn default() -> PackOpts {
        PackOpts { version: semver::Version::new(1, 0, 0), licence: None, derived_ok: false }
    }
}

fn pack_export_opts(o: &PackOpts) -> nfsu2::studio_api::mod_packs::ExportOpts<'_> {
    nfsu2::studio_api::mod_packs::ExportOpts {
        version: o.version.clone(),
        licence: o.licence.as_deref(),
        derived_ok: o.derived_ok,
        build: build(),
    }
}

/// The file name `<id>-<version>.obpk` of a pack made with `o`.
pub fn pack_file_name(folder: &str, o: &PackOpts) -> String {
    ob_asset_format::modpack::file_name(folder, &o.version)
}

/// Exports the mod in `dir` as a mod pack file `<id>-<version>.obpk` (docs/modding/asset-format/layout.md §4) to `to`
/// (a `.obpk` path, or a folder): the same pack and provenance the launcher's export writes, made for the base set in
/// use. Textures from the original game's pixels need `derived_ok` ([`Refused::Derived`]).
pub fn export_pack(dir: &Path, folder: &str, to: &Path, o: &PackOpts) -> Result<PackExported, Refused> {
    let d = derived(dir);
    if !d.is_empty() && !o.derived_ok {
        return Err(Refused::Derived(d));
    }
    let base = nfsu2::studio_api::mod_packs::base_in_use();
    nfsu2::studio_api::mod_packs::export_mod_dir(dir, folder, to, base.as_ref(), &pack_export_opts(o))
        .map_err(Refused::Failed)
}

/// Converts a plain map mod or an Overbright texture pack (a folder or `.zip`) to a mod pack file at `to`; the provenance
/// says it was converted. What the pack format cannot hold is refused or listed in [`Converted::skipped`].
pub fn convert_pack(from: &Path, to: &Path, id: Option<&str>, o: &PackOpts) -> Result<Converted, String> {
    let base = nfsu2::studio_api::mod_packs::base_in_use();
    nfsu2::studio_api::mod_packs::convert_legacy(from, to, id, base.as_ref(), &pack_export_opts(o))
}

#[cfg(test)]
mod tests;
