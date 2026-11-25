//! EA's movie files (MOVIES/*.vp6) and their playback as SPEED2.EXE does
//! it: the chunked container the AV library walks (0x700AFF), the VP6
//! video ([`crate::formats::vp6`]) and the EA-XA audio of the SCHl/SCDl
//! stream (0x72250B, 0x7228AF, 0x734AFF). See docs/formats.md, "Movies".

use anyhow::{Context as _, Result, bail};
use std::path::{Path, PathBuf};

use crate::formats::vp6;

/// MVhd (0x700AFF reads +0xC width, +0xE height, +0x18 rate, +0x1C the
/// rate's divisor as a u16).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Header {
    pub width: u16,
    pub height: u16,
    pub frames: u32,
    pub largest: u32,
    pub rate: u32,
    pub scale: u16,
}

impl Header {
    /// Frames per second: rate / scale, in f32 as the player divides.
    pub fn fps(&self) -> f32 {
        self.rate as f32 / self.scale.max(1) as f32
    }
}

/// The audio stream's header (SCHl): "GSTR" (0x72ABB0, big-endian data)
/// or "PT" tags (0x727A46). Only the GSTR form's defaults are taken from
/// the parser (channels 1, 48000 Hz, codec 10 unless tagged).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioHeader {
    pub channels: u8,
    pub rate: u32,
    pub samples: u32,
    /// The codec (tag 0xA0): 10 = EA-XA.
    pub codec: u8,
    /// GSTR streams store their SCDl counts and offsets big-endian.
    pub big_endian: bool,
}

/// One chunk: tag and payload range in the file.
#[derive(Clone, Copy, Debug)]
pub struct Chunk {
    pub tag: [u8; 4],
    pub at: usize,
    pub len: usize,
}

/// A movie file split into its chunks.
pub struct Movie {
    pub data: Vec<u8>,
    pub header: Header,
    pub audio: Option<AudioHeader>,
    /// The video frames (MV0K key, MV0F inter) in order: (key, chunk).
    pub frames: Vec<(bool, Chunk)>,
    /// The audio blocks (SCDl) in order.
    pub blocks: Vec<Chunk>,
}

fn be(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}

fn le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// The audio header tags: a tag byte, a length byte (0xFF: a 4-byte
/// length follows), a big-endian value of up to 4 bytes (0x72AAF8).
/// 0xFC is padding, 0xFD / 0xFE carry no length, 0xFF ends.
fn audio_tags(b: &[u8]) -> Vec<(u8, u32)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let tag = b[i];
        i += 1;
        match tag {
            0xFC => continue,
            0xFF => break,
            0xFD | 0xFE => {
                out.push((tag, 0));
                continue;
            }
            _ => {}
        }
        let Some(&n) = b.get(i) else { break };
        i += 1;
        let mut n = n as usize;
        if n == 0xFF {
            if i + 4 > b.len() {
                break;
            }
            n = be(b, i) as usize;
            i += 4;
        }
        let v = if n <= 4 { b.get(i..i + n).map_or(0, |s| s.iter().fold(0u32, |v, &c| v << 8 | c as u32)) } else { 0 };
        out.push((tag, v));
        i += n;
    }
    out
}

/// SCHl's payload (0x727A46): GSTR streams skip 8 bytes ("GSTR" and a
/// word) before the tags; channels (0x82), rate (0x84), samples (0x85)
/// and codec (0xA0) default to 1, 48000, 0 and 10 (0x72ABB0). A "PT"
/// header takes 22050 Hz and the same codec default (0x727A46: 0x5622).
pub fn parse_audio_header(p: &[u8]) -> Option<AudioHeader> {
    if p.len() >= 4 && &p[..4] == b"GSTR" {
        let tags = audio_tags(p.get(8..)?);
        let get = |t: u8, d: u32| tags.iter().rev().find(|(k, _)| *k == t).map_or(d, |(_, v)| *v);
        Some(AudioHeader { channels: get(0x82, 1) as u8, rate: get(0x84, 48000), samples: get(0x85, 0), codec: get(0xA0, 10) as u8, big_endian: true })
    } else if p.len() >= 2 && &p[..2] == b"PT" {
        let tags = audio_tags(p.get(4..)?);
        let get = |t: u8, d: u32| tags.iter().rev().find(|(k, _)| *k == t).map_or(d, |(_, v)| *v);
        Some(AudioHeader { channels: get(0x82, 1) as u8, rate: get(0x84, 0x5622), samples: get(0x85, 0), codec: get(0xA0, 10) as u8, big_endian: false })
    } else {
        None
    }
}

impl Movie {
    /// Split a movie into chunks: a 4-byte tag and a little-endian size
    /// that counts the 8-byte head.
    pub fn parse(data: Vec<u8>) -> Result<Movie> {
        let mut header = None;
        let mut audio = None;
        let mut frames = Vec::new();
        let mut blocks = Vec::new();
        let mut at = 0;
        while at + 8 <= data.len() {
            let tag: [u8; 4] = data[at..at + 4].try_into().unwrap();
            let size = le(&data, at + 4) as usize;
            if size < 8 || at + size > data.len() {
                bail!("bad movie chunk {:?} at {at}", String::from_utf8_lossy(&tag));
            }
            let chunk = Chunk { tag, at: at + 8, len: size - 8 };
            let p = &data[chunk.at..chunk.at + chunk.len];
            match &tag {
                b"MVhd" if p.len() >= 24 => {
                    header = Some(Header {
                        width: u16::from_le_bytes([p[4], p[5]]),
                        height: u16::from_le_bytes([p[6], p[7]]),
                        frames: le(p, 8),
                        largest: le(p, 12),
                        rate: le(p, 16),
                        scale: u16::from_le_bytes([p[20], p[21]]),
                    });
                }
                b"MV0K" => frames.push((true, chunk)),
                b"MV0F" => frames.push((false, chunk)),
                b"SCHl" => audio = parse_audio_header(p),
                b"SCDl" => blocks.push(chunk),
                _ => {}
            }
            at += size;
        }
        let header = header.context("no MVhd chunk")?;
        Ok(Movie { data, header, audio, frames, blocks })
    }

    pub fn payload(&self, c: &Chunk) -> &[u8] {
        &self.data[c.at..c.at + c.len]
    }
}

/// EA-XA (revision 2) as the SSE decoder 0x734AFF decodes it, in floats:
/// per channel two history samples; a block is 28 samples from one byte
/// (filter in the high nibble, shift in the low) and 14 bytes of nibbles,
/// high nibble first, or 0xEE, two big-endian history words and 28
/// big-endian PCM words (0x734A81).
#[derive(Clone, Copy, Default)]
pub struct XaChannel {
    hist1: f32,
    hist2: f32,
}

/// 0x819668 and 0x819678: the filter coefficients (indices past 3 read
/// on into the following table, as the game's would).
const XA_COEF: [f32; 8] = [0.0, 0.9375, 1.796875, 1.53125, 0.0, 0.0, -0.8125, -0.859375];

/// 0x819688: a nibble's value per shift, sign-extended nibble × 2^(12 −
/// shift) for shifts 0..12, zero for 13..15.
fn xa_nibble(shift: u8, n: u8) -> f32 {
    let v = ((n << 4) as i8 >> 4) as f32;
    if shift > 12 { 0.0 } else { v * (1u32 << (12 - shift)) as f32 }
}

fn xa_coef(i: usize) -> f32 {
    // Beyond the eight coefficients the game reads the nibble table.
    if i < 8 { XA_COEF[i] } else { xa_nibble(((i - 8) / 16) as u8, ((i - 8) % 16) as u8) }
}

impl XaChannel {
    /// Decode `samples` samples (in blocks of 28) from `b`.
    pub fn decode(&mut self, b: &[u8], samples: usize, out: &mut Vec<f32>) {
        let mut at = 0;
        let mut left = samples as i64;
        let start = out.len();
        while left > 0 {
            left -= 28;
            let Some(&head) = b.get(at) else { break };
            if head == 0xEE {
                let w = |i: usize| b.get(i..i + 2).map_or(0, |s| i16::from_be_bytes([s[0], s[1]])) as f32;
                self.hist1 = w(at + 1);
                self.hist2 = w(at + 3);
                for k in 0..28 {
                    out.push(w(at + 5 + 2 * k));
                }
                at += 5 + 56;
                continue;
            }
            let (c1, c2) = (xa_coef((head >> 4) as usize), xa_coef((head >> 4) as usize + 4));
            let shift = head & 0x0F;
            let (mut h1, mut h2) = (self.hist1, self.hist2);
            for k in 0..14 {
                let byte = b.get(at + 1 + k).copied().unwrap_or(0);
                // The SSE order: h1·c1 + nibble, then + h2·c2.
                let s0 = h1 * c1 + xa_nibble(shift, byte >> 4) + h2 * c2;
                let s1 = s0 * c1 + xa_nibble(shift, byte & 0x0F) + h1 * c2;
                out.push(s0);
                out.push(s1);
                h2 = s0;
                h1 = s1;
            }
            self.hist1 = h1;
            self.hist2 = h2;
            at += 15;
        }
        out.truncate(start + samples);
    }
}

/// One SCDl block (0x7228AF): a sample count, an offset per channel
/// (from the end of the offsets), the channels' data. Returns the
/// samples interleaved.
pub fn decode_block(audio: &AudioHeader, state: &mut [XaChannel], p: &[u8]) -> Vec<f32> {
    let ch = audio.channels.max(1) as usize;
    if p.len() < 4 + 4 * ch {
        return Vec::new();
    }
    let word = |i: usize| if audio.big_endian { be(p, i) } else { le(p, i) };
    // The sample count is a file field: no block holds more than
    // `Limits::max_count` samples (shipped blocks hold about 0.5 s).
    let samples = ((word(0) & 0x7FFF_FFFF) as usize).min(crate::formats::reader::Limits::DEFAULT.max_count);
    let base = 4 + 4 * ch;
    let mut planes = Vec::with_capacity(ch);
    for (c, st) in state.iter_mut().enumerate().take(ch) {
        let off = base + word(4 + 4 * c) as usize;
        let mut out = Vec::with_capacity(samples);
        st.decode(p.get(off..).unwrap_or(&[]), samples, &mut out);
        out.resize(samples, 0.0);
        planes.push(out);
    }
    let mut out = Vec::with_capacity(samples * ch);
    for i in 0..samples {
        for pl in &planes {
            out.push(pl[i]);
        }
    }
    out
}

/// The names 0x51C740 gives the plain ".vp6" (0x79B1D4), compared without
/// case (0x43DCC0): the strings at 0x7907E8, 0x79B224, 0x79B218,
/// 0x79B20C, 0x79B200, 0x79B1F4, 0x790878 and "blank" (0x79082C).
const PLAIN: [&str; 8] =
    ["FMVOpening", "IndustEast", "IndustWest", "Suburban", "UpperClass", "THX_logo", "THX_logo_fr", "blank"];

/// A movie's file name as 0x51C740 builds it ("%sMOVIES\%s%s", 0x79B1DC,
/// with an empty prefix 0x783FE5): `<name>.vp6` for the names above, else
/// `<name>_fr.vp6` (0x79B1EC; "ealogo" 0x790804 has a branch of its own
/// that takes the same string). The suffix is a constant of the
/// executable: no region, language or registry value is read.
pub fn file_name(name: &str) -> String {
    let plain = PLAIN.iter().any(|n| n.eq_ignore_ascii_case(name));
    format!("{name}{}", if plain { ".vp6" } else { "_fr.vp6" })
}

/// A movie's file in MOVIES (names match without case, as Win32 opens
/// them), `None` when there is none: the screen then ends at once.
///
/// 1. [`file_name`], the name the executable builds: 0x566250 (the one
///    caller of 0x51C740) asks the file layer whether it exists (0x6F909D →
///    0x6F87EA) and plays it (0x566030); without it, it sends the movie's
///    end 0xC3960EB9 at once and the screen is skipped (docs/formats.md,
///    "Boot sequence").
// prov: choice movie.path
/// 2. Without that file, this install's own name for it, as
///    [`crate::game_assets::resolve_movie`] finds it (no suffix, the text
///    language's, `_en`, any one language's). This build's "_fr" names do
///    not match other editions' data, where the original skips the logos,
///    the PSA, the career's scenes and the tutorials (seen under Wine with
///    the English data); the clone plays the installed file instead
///    (docs/playing/native-deviations.md, "Movie files"). The game-folder check
///    reports the same resolution.
pub fn path(root: &Path, name: &str) -> Option<PathBuf> {
    crate::game_assets::resolve_movie(root, name).file
}

/// The movie table (GLOBALB.BUN chunks 0x39020, the list at 0x8637BC):
/// per movie a 32-byte record, the name at +0 and the sound's volume byte
/// at +0x1F (read under Wine: ealogo 40, THX_logo 69, FMVOpening 105, PSA,
/// the scenes and the tutorials 127, the neighbourhood flybys 104).
pub fn volume_table(globalb: &[u8]) -> Vec<(String, u8)> {
    let mut out = Vec::new();
    crate::formats::bundle::walk(crate::formats::bundle::chunks(globalb, 0), &mut |c| {
        if c.id == 0x0003_9020 {
            let p = c.payload();
            if p.len() >= 0x20 {
                out.push((crate::formats::bundle::read_cstr(p, 0, 0x1F), p[0x1F]));
            }
        }
    });
    out
}

/// A movie's volume byte by name (0x51C580 compares without case; `None`
/// when it is not in the table), the table read once per run.
pub fn volume_byte(root: &Path, name: &str) -> Option<u8> {
    static TABLE: std::sync::OnceLock<Vec<(String, u8)>> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| crate::game_fs::read(root, "GLOBAL/GLOBALB.BUN").map(|d| volume_table(&d)).unwrap_or_default());
    t.iter().find(|e| e.0.eq_ignore_ascii_case(name)).map(|e| e.1)
}

/// A movie playing: the frames decoded as the clock passes, the audio
/// handed out ahead of it.
pub struct Player {
    pub movie: Movie,
    decoder: vp6::Decoder,
    /// The next frame to decode.
    next: usize,
    /// The next audio block to decode, and the samples (per channel)
    /// decoded before it.
    next_block: usize,
    audio_done: usize,
    xa: Vec<XaChannel>,
    /// Seconds since the start.
    pub clock: f32,
    /// The last frame decoded, as RGBA, and whether it changed.
    pub rgba: Vec<u8>,
    pub fresh: bool,
    pub failed: bool,
}

impl Player {
    pub fn open(path: &Path) -> Result<Player> {
        let movie = Movie::parse(crate::game_fs::read_file(path).with_context(|| format!("reading {}", path.display()))?)?;
        let ch = movie.audio.map_or(0, |a| a.channels as usize);
        Ok(Player { movie, decoder: vp6::Decoder::new(), next: 0, next_block: 0, audio_done: 0, xa: vec![XaChannel::default(); ch], clock: 0.0, rgba: Vec::new(), fresh: false, failed: false })
    }

    pub fn width(&self) -> usize {
        self.decoder.width
    }

    pub fn height(&self) -> usize {
        self.decoder.height
    }

    /// Whether every frame has been shown and its time has passed.
    pub fn finished(&self) -> bool {
        self.failed || (self.next >= self.movie.frames.len() && self.clock * self.movie.header.fps() >= self.movie.frames.len() as f32)
    }

    /// Decode frame `i` (a chunk's payload; MV0K / MV0F carry the frame
    /// directly, 0x700AFF passes +8 and size − 8).
    fn decode(&mut self, i: usize) {
        let (_, c) = self.movie.frames[i];
        let p = &self.movie.data[c.at..c.at + c.len];
        if let Err(e) = self.decoder.decode(p) {
            log::warn!("movie frame {i}: {e}");
            self.failed = true;
            return;
        }
        vp6::to_rgba(self.decoder.frame(), &mut self.rgba);
        self.fresh = true;
    }

    /// Advance the clock by `dt` seconds: decode up to the frame due. More
    /// than 16 frames behind, the player skips to the next key frame
    /// (0x700AFF).
    pub fn advance(&mut self, dt: f32) {
        self.clock += dt;
        let due = ((self.clock * self.movie.header.fps()) as usize + 1).min(self.movie.frames.len());
        if due > self.next + 16 {
            if let Some(k) = (self.next + 1..due).rev().find(|&k| self.movie.frames[k].0) {
                self.next = k;
            }
        }
        while self.next < due && !self.failed {
            self.decode(self.next);
            self.next += 1;
        }
    }

    /// The audio due before `ahead` seconds past the clock, interleaved,
    /// with its channel count and rate.
    pub fn audio(&mut self, ahead: f32) -> Option<(u8, u32, Vec<f32>)> {
        let a = self.movie.audio?;
        if a.codec != 10 {
            return None;
        }
        let mut out = Vec::new();
        let until = ((self.clock + ahead) * a.rate as f32) as usize;
        while self.next_block < self.movie.blocks.len() && self.audio_done < until {
            let c = self.movie.blocks[self.next_block];
            let p = &self.movie.data[c.at..c.at + c.len];
            let s = decode_block(&a, &mut self.xa, p);
            self.audio_done += s.len() / a.channels.max(1) as usize;
            out.extend(s);
            self.next_block += 1;
        }
        Some((a.channels, a.rate, out))
    }
}

#[cfg(test)]
mod path_tests {
    use super::{file_name, path};

    /// 0x51C740's names: the plain list compared without case, every other
    /// name with "_fr" ("ealogo" included).
    #[test]
    fn file_names_follow_the_executable() {
        for (name, file) in [
            ("blank", "blank.vp6"),
            ("FMVOpening", "FMVOpening.vp6"),
            ("fmvopening", "fmvopening.vp6"),
            ("THX_logo_fr", "THX_logo_fr.vp6"),
            ("THX_logo", "THX_logo.vp6"),
            ("UpperClass", "UpperClass.vp6"),
            ("IndustEast", "IndustEast.vp6"),
            ("ealogo", "ealogo_fr.vp6"),
            ("PSA", "PSA_fr.vp6"),
            ("scene01", "scene01_fr.vp6"),
            ("DriftTut", "DriftTut_fr.vp6"),
        ] {
            assert_eq!(file_name(name), file, "{name}");
        }
    }

    /// The order: the name the executable builds first (0x566250 →
    /// 0x6F909D), the installed language's file second, `None` (the screen
    /// ends at once) without either. Names match without case.
    #[test]
    fn built_name_first_then_installed_language() {
        let scratch = crate::testkit::Scratch::new("movie-path");
        let root = scratch.path();
        let movies = root.join("MOVIES");
        std::fs::create_dir_all(&movies).unwrap();
        // The English data's names only: the second step finds them.
        for f in ["blank.vp6", "FMVOpening.vp6", "ealogo_en.vp6", "THX_logo.vp6", "Scene01_en.vp6", "DriftTut_en.vp6"] {
            std::fs::write(movies.join(f), b"").unwrap();
        }
        assert_eq!(path(root, "blank"), Some(movies.join("blank.vp6")));
        assert_eq!(path(root, "fmvopening"), Some(movies.join("FMVOpening.vp6")));
        assert_eq!(path(root, "ealogo"), Some(movies.join("ealogo_en.vp6")));
        assert_eq!(path(root, "THX_logo_fr"), Some(movies.join("THX_logo.vp6")));
        assert_eq!(path(root, "scene01"), Some(movies.join("Scene01_en.vp6")));
        assert_eq!(path(root, "DriftTut"), Some(movies.join("DriftTut_en.vp6")));
        // Neither file: nothing to play.
        assert_eq!(path(root, "PSA"), None);
        // The built name, once there, comes first (any case).
        std::fs::write(movies.join("EALOGO_FR.VP6"), b"").unwrap();
        std::fs::write(movies.join("thx_logo_fr.vp6"), b"").unwrap();
        std::fs::write(movies.join("PSA_fr.vp6"), b"").unwrap();
        assert_eq!(path(root, "ealogo"), Some(movies.join("EALOGO_FR.VP6")));
        assert_eq!(path(root, "THX_logo_fr"), Some(movies.join("thx_logo_fr.vp6")));
        assert_eq!(path(root, "PSA"), Some(movies.join("PSA_fr.vp6")));
        assert_eq!(path(root, "scene01"), Some(movies.join("Scene01_en.vp6")));
    }
}

#[cfg(test)]
mod volume_tests {
    /// The movie table's volume bytes as the original holds them (read
    /// under Wine from the list at 0x8637BC).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn movie_volumes_match_the_original() {
        let root = crate::testkit::game_root();
        let d = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let t = super::volume_table(&d);
        let get = |n: &str| t.iter().find(|e| e.0.eq_ignore_ascii_case(n)).map(|e| e.1);
        assert_eq!(get("ealogo"), Some(40));
        assert_eq!(get("THX_logo_fr"), Some(69));
        assert_eq!(get("FMVOpening"), Some(105));
        assert_eq!(get("scene01"), Some(127));
        assert_eq!(get("UpperClass"), Some(104));
        assert_eq!(get("blank"), None);
        assert_eq!(t.len(), 40);
    }
}
