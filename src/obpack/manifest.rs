//! `pack.toml`: an Overbright pack's manifest (docs/modding/texture-packs.md).

use anyhow::{Result, bail};
use serde::Deserialize;

/// The manifest's file name in a pack's folder.
pub const FILE: &str = "pack.toml";
/// The newest pack format this loader reads; a higher one is refused.
pub const FORMAT: u32 = 1;
/// The longest manifest read (a manifest is a few lines).
pub const MAX_BYTES: usize = 64 * 1024;

/// How a pack's PNG images are kept for the GPU.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Compress {
    /// BC7, compiled once into the cache (4 × less video memory than RGBA8).
    #[default]
    Bc7,
    /// As RGBA8, the image's own pixels (lossless; the loader's PNG path).
    None,
}

/// A pack's manifest as read (fields not named here are ignored).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    /// The format the pack was written for (1; 0 for a folder without a
    /// manifest).
    pub format: u32,
    pub name: String,
    pub author: String,
    pub licence: String,
    /// Higher first: where the pack goes among packs the player has not put
    /// in order (`order.txt`); among equal ones, by id.
    pub priority: i32,
    pub version: String,
    pub description: String,
    pub compress: Compress,
    /// What made it (`editor`, `texmod`, …), informational.
    pub source: String,
}

#[derive(Deserialize)]
struct Raw {
    format: Option<i64>,
    name: Option<String>,
    author: Option<String>,
    authors: Option<Vec<String>>,
    licence: Option<String>,
    license: Option<String>,
    priority: Option<i64>,
    version: Option<String>,
    description: Option<String>,
    compress: Option<String>,
    source: Option<String>,
}

/// One line of text: control characters out, at most `max` characters.
fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

impl Manifest {
    /// The manifest of a folder without one (format 0): its id as the name.
    pub fn bare(id: &str) -> Manifest {
        Manifest { name: id.to_string(), ..Default::default() }
    }

    /// `pack.toml`'s text read and checked. Refused: not TOML, no or a wrong
    /// `format` (missing, below 1, above [`FORMAT`]), no `name`, a priority
    /// outside ±1,000,000, an unknown `compress`.
    pub fn parse(text: &str) -> Result<Manifest> {
        if text.len() > MAX_BYTES {
            bail!("pack.toml is larger than {MAX_BYTES} bytes");
        }
        let raw: Raw = toml::from_str(text).map_err(|e| anyhow::anyhow!("pack.toml: {}", e.message()))?;
        let format = match raw.format {
            None => bail!("pack.toml: no `format`"),
            Some(f) if f < 1 => bail!("pack.toml: format {f} is not a pack format"),
            Some(f) if f > i64::from(FORMAT) => {
                bail!("pack.toml: format {f} is newer than this Overbright reads ({FORMAT}); update Overbright")
            }
            Some(f) => f as u32,
        };
        let name = clean(raw.name.as_deref().unwrap_or(""), 80);
        if name.is_empty() {
            bail!("pack.toml: no `name`");
        }
        let priority = raw.priority.unwrap_or(0);
        if priority.abs() > 1_000_000 {
            bail!("pack.toml: priority {priority} is outside -1000000..1000000");
        }
        let author = match (raw.author, raw.authors) {
            (Some(a), _) => clean(&a, 200),
            (None, Some(list)) => clean(&list.join(", "), 200),
            (None, None) => String::new(),
        };
        let compress = match raw.compress.as_deref().map(str::to_ascii_lowercase).as_deref() {
            None | Some("bc7") | Some("auto") => Compress::Bc7,
            Some("none") => Compress::None,
            Some(other) => bail!("pack.toml: compress = {other:?} (bc7 or none)"),
        };
        Ok(Manifest {
            format,
            name,
            author,
            licence: clean(raw.licence.or(raw.license).as_deref().unwrap_or(""), 200),
            priority: priority as i32,
            version: clean(raw.version.as_deref().unwrap_or(""), 40),
            description: clean(raw.description.as_deref().unwrap_or(""), 1000),
            compress,
            source: clean(raw.source.as_deref().unwrap_or(""), 40),
        })
    }

    /// The manifest as `pack.toml` text (what [`Manifest::parse`] reads back).
    pub fn to_toml(&self) -> String {
        // TOML basic strings: backslash and quote escaped (the text has no
        // control characters, [`clean`]).
        let q = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
        let mut out = String::from("# Overbright texture pack (docs/modding/texture-packs.md)\n");
        out += &format!("format = {}\n", self.format.max(1));
        out += &format!("name = {}\n", q(&self.name));
        out += &format!("author = {}\n", q(&self.author));
        out += &format!("licence = {}\n", q(&self.licence));
        out += &format!("priority = {}\n", self.priority);
        if !self.version.is_empty() {
            out += &format!("version = {}\n", q(&self.version));
        }
        if !self.description.is_empty() {
            out += &format!("description = {}\n", q(&self.description));
        }
        if self.compress == Compress::None {
            out += "compress = \"none\"\n";
        }
        if !self.source.is_empty() {
            out += &format!("source = {}\n", q(&self.source));
        }
        out
    }
}
