//! Studio Looks (docs/modding/studio-looks.md): a mod's textures, painted in the modder's own paint program.
//!
//! - **The mod's texture list** is one document of its history, [`FILE`] (`looks.toml`): one entry per texture
//!   the mod changes, by its [`TextureId`] (domain and the game's texture hash), with where its pixels started
//!   ([`Origin`]: the game's own texture, so derived, or a blank canvas) and the image it has now (a content
//!   hash). A change of a texture is one step of that document, so undo, redo, revert and the one-writer rule are
//!   the history's ([`crate::history`]); each entry is an item of its own, so two textures changed by two
//!   writers merge.
//! - **Images** are kept by their content in the editor folder ([`STORE_DIR`]), never changed once written: a
//!   step names an image, and undo goes back to the image the step before named.
//! - **The pack data** the game reads is [`PACK_DIR`] (`textures/world/<HASH>.png`, `textures/car/<HASH>.png`),
//!   written from the list ([`work::materialize`]): what the list says, nothing else. Looks never writes into the
//!   game's folder.
//! - **Work files** ([`WORK_DIR`]) are the images the modder opens in their own program; a save is taken in as
//!   one step ([`work::Workbench`]). They are the modder's, never part of the pack.
//!
//! Later parts of Looks (picking in the 3D view, the before/after slider, where a texture is used, the derived
//! marks and sharing warnings) read the same model: [`TextureId`] names a texture everywhere, [`Original`]
//! carries its uses, and [`Entry::origin`] says whether its pixels came from the game.

pub mod guide;
pub mod work;

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The texture list's file, in the mod's folder (a document of the mod's history).
pub const FILE: &str = ob_map::tools::map::share::LOOKS;
/// The pack data's folder in the mod's folder: `<domain>/<HASH>.png`, as the game's texture loader reads it.
pub const PACK_DIR: &str = ob_map::tools::map::store::TEXTURES;
/// The work files' folder in the mod's folder (`world/<name>.png`): what the modder paints. Never exported.
pub const WORK_DIR: &str = "work/looks";
/// The images by content, in the mod's editor folder (`<xxh3>.png`).
pub const STORE_DIR: &str = "looks";
/// The work files Looks watches, with what each was opened from, in the mod's editor folder.
pub const WORK_FILE: &str = "looks-work.toml";
/// The list's format version.
pub const FORMAT: u32 = 1;
/// The largest side the game's texture loader takes.
pub const MAX_SIDE: u32 = 4096;

/// Which of the game's renderers a texture is for (the pack's folders).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Domain {
    World,
    Car,
}

impl Domain {
    pub const ALL: [Domain; 2] = [Domain::World, Domain::Car];

    /// Its word (`world`, `car`): the pack's folder and the id's prefix.
    pub fn word(self) -> &'static str {
        match self {
            Domain::World => "world",
            Domain::Car => "car",
        }
    }
}

/// A texture of the game: its domain and the game's hash of its name (`world:0x1A2B3C4D`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureId {
    pub domain: Domain,
    pub hash: u32,
}

impl TextureId {
    pub fn new(domain: Domain, hash: u32) -> TextureId {
        TextureId { domain, hash }
    }

    /// The pack file the game reads for it, inside [`PACK_DIR`] (`world/1A2B3C4D.png`).
    pub fn pack_file(&self) -> String {
        format!("{}/{:08X}.png", self.domain.word(), self.hash)
    }
}

impl fmt::Display for TextureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:0x{:08X}", self.domain.word(), self.hash)
    }
}

impl FromStr for TextureId {
    type Err = String;

    /// `world:0x1A2B3C4D`, `car:1a2b3c4d`.
    fn from_str(s: &str) -> Result<TextureId, String> {
        let bad = || format!("`{s}` is not a texture id (world:0x1A2B3C4D or car:0x1A2B3C4D)");
        let (d, h) = s.split_once(':').ok_or_else(bad)?;
        let domain = match d {
            "world" => Domain::World,
            "car" => Domain::Car,
            _ => return Err(bad()),
        };
        let h = h.strip_prefix("0x").or_else(|| h.strip_prefix("0X")).unwrap_or(h);
        if h.is_empty() || h.len() > 8 {
            return Err(bad());
        }
        let hash = u32::from_str_radix(h, 16).map_err(|_| bad())?;
        Ok(TextureId { domain, hash })
    }
}

impl Serialize for TextureId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TextureId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<TextureId, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Where a texture's pixels started.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    /// From the game's own texture: the result is derived from the original's pixels (marked, and warned about
    /// before sharing).
    Game,
    /// "Repaint from blank": an empty canvas of the texture's size; the pixels are the modder's own.
    Blank,
}

impl Origin {
    pub fn word(self) -> &'static str {
        match self {
            Origin::Game => "game",
            Origin::Blank => "blank",
        }
    }

    /// Made from the game's pixels.
    pub fn derived(self) -> bool {
        self == Origin::Game
    }
}

/// One texture the mod changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: TextureId,
    /// The texture's name in the game (for people; the id is what counts).
    #[serde(default)]
    pub name: String,
    pub origin: Origin,
    /// The image now: its content hash in [`STORE_DIR`] (`<16 hex>`).
    pub image: String,
    pub width: u32,
    pub height: u32,
}

/// The mod's texture list ([`FILE`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Looks {
    pub entries: BTreeMap<TextureId, Entry>,
}

#[derive(Deserialize)]
struct LooksFile {
    format: u32,
    #[serde(default, rename = "texture")]
    textures: Vec<Entry>,
}

impl Looks {
    /// The list from its text (None or empty: no textures yet).
    pub fn parse(text: &str) -> Result<Looks, String> {
        if text.trim().is_empty() {
            return Ok(Looks::default());
        }
        let f: LooksFile = toml::from_str(text).map_err(|e| format!("{FILE}: {e}"))?;
        if f.format > FORMAT {
            return Err(format!("{FILE}: format {} is newer than this Studio's ({FORMAT})", f.format));
        }
        let mut entries = BTreeMap::new();
        for e in f.textures {
            if entries.insert(e.id, e.clone()).is_some() {
                return Err(format!("{FILE}: {} is listed twice", e.id));
            }
        }
        Ok(Looks { entries })
    }

    /// The list's text: textures in id order, so the same list is the same text.
    pub fn write(&self) -> String {
        let mut out = format!("format = {FORMAT}\n");
        for e in self.entries.values() {
            out.push_str(&format!(
                "\n[[texture]]\nid = {}\nname = {}\norigin = {}\nimage = {}\nwidth = {}\nheight = {}\n",
                quote(&e.id.to_string()),
                quote(&e.name),
                quote(e.origin.word()),
                quote(&e.image),
                e.width,
                e.height
            ));
        }
        out
    }

    /// The list's items for the history's merge (`texture:world:0x1A2B3C4D` → the entry as JSON).
    pub fn items(&self) -> Vec<(String, String)> {
        self.entries
            .values()
            .map(|e| (format!("texture:{}", e.id), serde_json::to_string(e).unwrap_or_default()))
            .collect()
    }

    /// The list made of [`items`](Self::items) (None: an item does not read back).
    pub fn join(items: &[(String, String)]) -> Option<Looks> {
        let mut l = Looks::default();
        for (k, v) in items {
            let e: Entry = serde_json::from_str(v).ok()?;
            if k != &format!("texture:{}", e.id) {
                return None;
            }
            l.entries.insert(e.id, e);
        }
        Some(l)
    }
}

/// A texture of the game as Looks reads it: what a work file starts from, and (for the parts of Looks that
/// follow) where the game uses it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Original {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// The top level, RGBA.
    pub rgba: Vec<u8>,
    /// Where the game uses it, for people (`L4RA/1407` scenery and object); empty when not looked up.
    pub used_by: Vec<String>,
}

/// The game's textures Looks reads.
pub trait Textures {
    fn original(&self, id: TextureId) -> Option<Original>;
}

/// A region's world (as Studio loads it): its world textures.
impl Textures for crate::studio_api::world::World {
    fn original(&self, id: TextureId) -> Option<Original> {
        if id.domain != Domain::World {
            return None;
        }
        let t = self.textures.get(&id.hash)?;
        Some(Original {
            name: t.name.clone(),
            width: t.width,
            height: t.height,
            rgba: t.decode_rgba(),
            used_by: Vec::new(),
        })
    }
}

/// A TOML basic string (a JSON string's escapes are TOML's).
pub(crate) fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// An image's content hash, as [`Entry::image`] names it.
pub fn image_hash(png: &[u8]) -> String {
    format!("{:016x}", xxhash_rust::xxh3::xxh3_64(png))
}

/// A PNG decoded with the game's loader limits: (width, height, RGBA).
pub fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes));
    reader.set_format(image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().map_err(|e| e.to_string())?.into_rgba8();
    Ok((img.width(), img.height(), img.into_raw()))
}

/// RGBA pixels as PNG bytes.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::RgbaImage::from_raw(width, height, rgba.to_vec()).ok_or("the pixels do not fill the image")?;
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests;
