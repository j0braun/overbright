//! [`Assets`]: the packages, textures, fonts and strings the screens need.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::formats::bundle::{Chunk, chunks, walk};
use crate::formats::hash::bin_hash;
use crate::formats::texture::{Texture, parse_texture_pack};
use crate::frontend::feng;
use crate::frontend::feng::Package;
use crate::frontend::feng::font::Font;
use crate::frontend::feng::strings::Strings;

/// The key the in-game copy of a texture takes in [`Assets::textures`]
/// (see [`Assets::in_game`]).
pub fn in_game_alias(hash: u32) -> u32 {
    hash.rotate_left(16) ^ 0x1A6E_5EED
}

pub struct Assets {
    pub packages: crate::fxhash::FxHashMap<String, Package>,
    pub textures: crate::fxhash::FxHashMap<u32, Texture>,
    /// Textures the in-game bundles hold in another form than the front
    /// end's: name hash → the key of the in-game copy in `textures`.
    pub in_game: crate::fxhash::FxHashMap<u32, u32>,
    /// By `bin_hash` of the upper-case font name.
    pub fonts: HashMap<u32, Font>,
    pub strings: Strings,
    /// The TRACKMAP numbers whose file or texture failed to load
    /// ([`Assets::track_map`]; the HUD asks every frame).
    pub track_map_failed: crate::failed_load::FailedLoads<u16>,
}

pub(crate) fn read_bundle(path: &Path) -> Result<Vec<u8>> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(if data.starts_with(crate::formats::jdlz::MAGIC) { crate::formats::jdlz::decompress(&data)? } else { data })
}

/// A package chunk's FEng file: 0x30203 plain, 0x30210 a name hash then a
/// JDLZ or HUFF block.
pub(crate) fn package_bytes(c: &Chunk) -> Option<Vec<u8>> {
    match c.id {
        0x30203 => Some(c.body.to_vec()),
        0x30210 => {
            let at = c.body.windows(4).take(32).position(|w| w == b"JDLZ" || w == b"HUFF")?;
            let p = &c.body[at..];
            if p.starts_with(b"JDLZ") {
                crate::formats::jdlz::decompress(p).ok()
            } else {
                crate::formats::huff::decompress(p).ok()
            }
        }
        _ => None,
    }
}

/// The files [`Assets::load`] reads, in its order: the front end, then the
/// in-game screens (HUD, pause, post race), then the fonts and their
/// textures. (Also the sources of the `ui2d` cache.)
pub const ASSET_FILES: [&str; 12] = [
    "FRONTEND/FrontB.lzc",
    "GLOBAL/GlobalB.lzc",
    "GLOBAL/GLOBALA.BUN",
    "GLOBAL/InGameCommon.lzc",
    "GLOBAL/InGameRace.bun",
    "GLOBAL/InGameFreeRoam.bun",
    "GLOBAL/InGameDrag.bun",
    "GLOBAL/InGameDrift.bun",
    // The dyno and tuning screens (loaded by 0x55D800's screens).
    "GLOBAL/InGamePerformanceTuning.bun",
    "GLOBAL/HUD_CustomTextures_ALL.bin",
    "LANGUAGES/LanguageTextures.bin",
    crate::game_assets::TEXT_FILE,
];

/// The TRACKMAP numbers kept as failed: the HUD's map and the world
/// map's markers ask for several in one frame.
const TRACK_MAPS_FAILED: usize = 16;

impl Assets {
    pub fn load(root: &Path) -> Result<Assets> {
        let mut assets = Assets {
            in_game: Default::default(),
            packages: Default::default(),
            textures: Default::default(),
            fonts: HashMap::new(),
            strings: Strings::parse(&crate::game_fs::read(root, crate::game_assets::TEXT_FILE)?)?,
            track_map_failed: crate::failed_load::FailedLoads::new(TRACK_MAPS_FAILED),
        };
        let files = ASSET_FILES;
        // Each file read, unpacked and parsed on its own thread; merged in
        // the list's order (the first of a name wins).
        enum Item {
            Package(feng::Package),
            Textures(Vec<crate::formats::texture::Texture>),
            Font(Font),
        }
        use rayon::prelude::*;
        let parsed: Vec<Result<Vec<Item>>> = files
            .par_iter()
            .map(|file| {
                let data = read_bundle(&crate::game_fs::path(root, file))?;
                let mut items = Vec::new();
                walk(chunks(&data, 0), &mut |c| {
                    if let Some(bytes) = package_bytes(&c) {
                        match feng::parse(&bytes) {
                            Ok(p) => items.push(Item::Package(p)),
                            Err(e) => log::warn!("{file}: package: {e}"),
                        }
                    } else if c.id == 0xB330_0000 {
                        match parse_texture_pack(c) {
                            Ok(ts) => items.push(Item::Textures(ts)),
                            Err(e) => log::warn!("{file}: texture pack: {e}"),
                        }
                    } else if c.id == 0x30201 {
                        match Font::parse(c.body) {
                            Ok(f) => items.push(Item::Font(f)),
                            Err(e) => log::warn!("{file}: font: {e}"),
                        }
                    }
                });
                Ok(items)
            })
            .collect();
        for (file, items) in files.iter().zip(parsed) {
            // The in-game bundles (the game loads them for a race or free
            // roam, with FrontB.lzc unloaded).
            let in_game = file.starts_with("GLOBAL/InGame") && !file.contains("PerformanceTuning");
            for item in items? {
                match item {
                    Item::Package(p) => {
                        let key = p.name.trim_end_matches(".fng").to_string();
                        assets.packages.entry(key).or_insert(p);
                    }
                    Item::Textures(ts) => {
                        for t in ts {
                            // A name the front end's bundles also hold, in a
                            // different form (e.g. 3RDPERSON_TURBORED_1 and
                            // 3RDPERSON_REDLINE_<rpm>: alpha in FrontB.lzc,
                            // additive in InGameCommon / InGameRace /
                            // InGameFreeRoam / InGameDrift): the in-game copy
                            // is kept under an alias for the HUD.
                            if in_game
                                && !assets.in_game.contains_key(&t.hash)
                                && let Some(old) = assets.textures.get(&t.hash)
                                && (old.blend != t.blend
                                    || old.width != t.width
                                    || old.height != t.height
                                    || old.data[..] != t.data[..])
                            {
                                let alias = in_game_alias(t.hash);
                                assets.in_game.insert(t.hash, alias);
                                assets.textures.insert(alias, t);
                                continue;
                            }
                            assets.textures.entry(t.hash).or_insert(t);
                        }
                    }
                    Item::Font(f) => {
                        assets.fonts.insert(bin_hash(&f.name.to_uppercase()), f);
                    }
                }
            }
        }
        Ok(assets)
    }

    /// A track's minimap texture (TRACKS/TRACKMAP<n>.BIN), loaded on first
    /// use; its hash.
    pub fn track_map(&mut self, root: &Path, number: u16) -> Option<u32> {
        let name = format!("TRACKMAP{number}");
        let h = bin_hash(&name);
        if !self.textures.contains_key(&h) {
            // A file or texture that failed is not read again for the same
            // number (the original loads the map once with its track).
            if self.track_map_failed.remembers(&number) {
                return None;
            }
            self.track_map_failed.tried();
            let data = match read_bundle(&crate::game_fs::path(root, &format!("TRACKS/{name}.BIN"))) {
                Ok(data) => data,
                Err(e) => {
                    self.track_map_failed.remember(number, "minimap", e);
                    return None;
                }
            };
            walk(chunks(&data, 0), &mut |c| {
                if c.id == 0xB330_0000
                    && let Ok(ts) = parse_texture_pack(c)
                {
                    for t in ts {
                        self.textures.entry(t.hash).or_insert(t);
                    }
                }
            });
            if self.textures.contains_key(&h) {
                self.track_map_failed.forget();
            } else {
                self.track_map_failed.remember(number, "minimap", format_args!("no {name} texture in TRACKS/{name}.BIN"));
            }
        }
        self.textures.contains_key(&h).then_some(h)
    }

    pub fn package(&self, name: &str) -> Option<&Package> {
        self.packages.get(name)
    }

    /// A package resource's texture, by its file name without extension.
    pub fn texture_for(&self, name: &str) -> Option<u32> {
        let base = name.rsplit_once('.').map_or(name, |(b, _)| b);
        let h = crate::formats::hash::upper_hash(base);
        self.textures.contains_key(&h).then_some(h)
    }

    pub fn font_for(&self, name: &str) -> Option<&Font> {
        let base = name.rsplit_once('.').map_or(name, |(b, _)| b);
        self.fonts.get(&crate::formats::hash::upper_hash(base)).or_else(|| self.fonts.values().next())
    }
}

#[cfg(test)]
mod failed_track_map_tests {
    use super::*;

    /// A missing TRACKMAP file is read once
    /// across the HUD's frames, and a new track number is tried at once.
    #[test]
    fn a_missing_track_map_is_read_once_until_the_track_changes() {
        let mut assets = Assets {
            packages: Default::default(),
            textures: Default::default(),
            in_game: Default::default(),
            fonts: HashMap::new(),
            strings: Strings { map: HashMap::new() },
            track_map_failed: crate::failed_load::FailedLoads::new(TRACK_MAPS_FAILED),
        };
        let root = Path::new("/nonexistent/ob-failed-track-map");
        for _ in 0..120 {
            assert_eq!(assets.track_map(root, 4000), None);
        }
        assert_eq!(assets.track_map_failed.attempts(), 1, "one read across 120 frames");
        assert!(assets.track_map_failed.remembers(&4000));
        for _ in 0..120 {
            assert_eq!(assets.track_map(root, 4001), None);
        }
        assert_eq!(assets.track_map_failed.attempts(), 2, "the new number read once");
    }
}
