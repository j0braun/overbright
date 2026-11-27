//! The Browser's sound and string viewers: the game's sound banks and language tables, listed and searched by
//! name, a sound written as a WAV file. The window's Browser and `browser sounds` / `browser strings` on the
//! command line read the same functions, so the same words give the same answer.
//!
//! * A **sound** is an event of a sound bank (`FX_TURBO_01` in `SOUND/IG_GLOBAL/…`), with the bank's samples it
//!   starts; a bank with no events lists its samples by number. Only the banks' own codec (EA-XA, one channel)
//!   can be decoded; the others are listed and marked.
//! * A **string** is a line of a language file (`LANGUAGES/<language>.bin`), by its hash, found by its words or
//!   by the label whose hash it is.
//!
//! What is read comes through [`Media`], so the tests give their own banks and the game's files are read by
//! [`GameMedia`]. Nothing here plays a sound: the window's Play button does that on a click.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use nfsu2::studio_api::media::{Bank, Strings};
use ob_formats::formats::hash::bin_hash;

/// One sound of a bank.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    /// The bank's file under the game folder (`SOUND/FE/FE_MB.abk`).
    pub bank: String,
    /// The event's name, or `sample N` for a bank without events.
    pub name: String,
    /// The bank's samples it starts (indices), first the one the viewer plays.
    pub samples: Vec<u32>,
    /// The first sample's rate (Hz) and length (frames).
    pub rate: u32,
    pub frames: u32,
    /// Whether the first sample can be decoded.
    pub playable: bool,
}

impl Sound {
    pub fn seconds(&self) -> f32 {
        if self.rate == 0 { 0.0 } else { self.frames as f32 / self.rate as f32 }
    }

    /// "FX_TURBO_01 · 1.2 s · 22050 Hz".
    pub fn describe(&self) -> String {
        format!("{} · {:.2} s · {} Hz{}", self.name, self.seconds(), self.rate, if self.playable { "" } else { " · not decodable" })
    }
}

/// Decoded sound: floats at 16-bit scale, one channel.
#[derive(Clone, Debug, PartialEq)]
pub struct Pcm {
    pub rate: u32,
    pub samples: Vec<f32>,
}

/// One line of a language file.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub hash: u32,
    pub text: String,
}

/// What the viewers read of the game.
pub trait Media {
    /// Every sound of every bank, by bank then event.
    fn sounds(&self) -> Vec<Sound>;
    /// The decoded sample `sample` of `bank`.
    fn pcm(&self, bank: &str, sample: u32) -> Option<Pcm>;
    /// The language files (`English`, `German` …), sorted.
    fn languages(&self) -> Vec<String>;
    /// The lines of a language file, by hash (None: no such language or an unreadable file).
    fn strings(&self, language: &str) -> Option<Vec<Line>>;
}

/// The language the strings are shown in unless another is asked for.
pub const DEFAULT_LANGUAGE: &str = "English";

/// A WAV file (PCM, 16 bit, one channel) of `pcm`.
pub fn wav(pcm: &Pcm) -> Vec<u8> {
    let data: Vec<u8> = pcm.samples.iter().flat_map(|&s| (s.round().clamp(-32768.0, 32767.0) as i16).to_le_bytes()).collect();
    let mut w = Vec::with_capacity(44 + data.len());
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&pcm.rate.to_le_bytes());
    w.extend_from_slice(&(pcm.rate * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(data.len() as u32).to_le_bytes());
    w.extend_from_slice(&data);
    w
}

/// A file-name-safe name for a sound's WAV (`<bank stem>-<name>.wav`).
pub fn wav_name(s: &Sound) -> String {
    let stem = s.bank.rsplit('/').next().unwrap_or(&s.bank);
    let stem = stem.strip_suffix(".abk").or_else(|| stem.strip_suffix(".ABK")).unwrap_or(stem);
    format!("{}-{}.wav", super::gltf::file_stem(stem), super::gltf::file_stem(&s.name))
}

fn words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_ascii_lowercase).collect()
}

/// The sounds whose bank path or name holds every word of `text` (case ignored), in order.
pub fn find_sounds(all: &[Sound], text: &str) -> Vec<usize> {
    let w = words(text);
    all.iter()
        .enumerate()
        .filter(|(_, s)| {
            let hay = format!("{} {}", s.bank, s.name).to_ascii_lowercase();
            w.iter().all(|x| hay.contains(x.as_str()))
        })
        .map(|(i, _)| i)
        .collect()
}

/// The lines whose text holds every word of `text` (case ignored); `0x…` is a hash; one word that is a label
/// (`DEFAULT_STRING`) finds the line it names.
pub fn find_lines(all: &[Line], text: &str) -> Vec<usize> {
    let w = words(text);
    if w.is_empty() {
        return (0..all.len()).collect();
    }
    let hash = |x: &str| -> Option<u32> { x.strip_prefix("0x").filter(|h| !h.is_empty() && h.len() <= 8).and_then(|h| u32::from_str_radix(h, 16).ok()) };
    let label = (w.len() == 1).then(|| bin_hash(&text.trim().to_ascii_uppercase()));
    all.iter()
        .enumerate()
        .filter(|(_, l)| {
            if w.len() == 1 && hash(&w[0]) == Some(l.hash) {
                return true;
            }
            if label == Some(l.hash) {
                return true;
            }
            let low = l.text.to_ascii_lowercase();
            w.iter().all(|x| low.contains(x.as_str()))
        })
        .map(|(i, _)| i)
        .collect()
}

/// The language file the words name: an exact name (any case), else the only one they begin.
pub fn pick_language(languages: &[String], want: &str) -> Option<String> {
    let w = want.trim().to_ascii_lowercase();
    if let Some(l) = languages.iter().find(|l| l.to_ascii_lowercase() == w) {
        return Some(l.clone());
    }
    let mut starts = languages.iter().filter(|l| l.to_ascii_lowercase().starts_with(&w));
    match (starts.next(), starts.next()) {
        (Some(l), None) if !w.is_empty() => Some(l.clone()),
        _ => None,
    }
}

/// Read what `kind` looks at into `ix` when it is not there yet (the window's Browser and the command line both
/// call it): the sounds, or `language`'s lines (the English file when it is empty). Err: the words for a language
/// the game has not, with the nearest name.
pub fn load(ix: &mut super::Index, media: &dyn Media, kind: super::Kind, language: &str) -> Result<(), String> {
    match kind {
        super::Kind::Sounds if ix.sounds.is_none() => ix.set_sounds(media.sounds()),
        super::Kind::Strings => {
            let languages = media.languages();
            let want = if language.trim().is_empty() { DEFAULT_LANGUAGE } else { language };
            let Some(name) = pick_language(&languages, want) else {
                let mut m = format!("no language `{want}`: the game has {}", languages.join(", "));
                if let Some(n) = crate::browse::did_you_mean(want, languages.iter().map(String::as_str)) {
                    m.push_str(&format!(". Did you mean `{n}`?"));
                }
                return Err(m);
            };
            if ix.lines.is_none() || ix.language != name {
                let lines = media.strings(&name).ok_or_else(|| format!("the {name} language file does not read"))?;
                ix.set_lines(&name, languages, lines);
            }
        }
        _ => {}
    }
    Ok(())
}

/// Write a sound's first sample as a WAV into `dir` (refused into the game's folder), marked as from the game
/// files like a model's export: the file and [`super::gltf::NOTE_FILE`] beside it.
pub fn write_wav(dir: &Path, game_root: Option<&Path>, media: &dyn Media, sound: &Sound) -> Result<PathBuf, String> {
    super::gltf::check_target(dir, game_root)?;
    let Some(&sample) = sound.samples.first() else { return Err(format!("{} has no sample", sound.name)) };
    let Some(pcm) = media.pcm(&sound.bank, sample) else {
        return Err(format!("{} cannot be decoded: only the banks' own codec (one channel) is read", sound.name));
    };
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let put = |name: &str, bytes: &[u8]| -> Result<PathBuf, String> {
        let p = dir.join(name);
        ob_core::platform::fs::write_atomic(&p, bytes, false).map_err(|e| format!("cannot write {}: {e}", p.display()))?;
        Ok(p)
    };
    let file = put(&wav_name(sound), &wav(&pcm))?;
    put(super::gltf::NOTE_FILE, super::gltf::NOTE.as_bytes())?;
    Ok(file)
}

/// [`Media`] from the game folder; a bank is parsed when first asked for.
pub struct GameMedia {
    root: PathBuf,
    last: RefCell<Option<(String, Option<Bank>)>>,
}

impl GameMedia {
    pub fn new(root: &Path) -> GameMedia {
        GameMedia { root: root.to_path_buf(), last: RefCell::new(None) }
    }

    /// The files under the game folder's `dir` (found by name in any case) that end in `ext`, as paths
    /// relative to the folder, sorted.
    fn files(&self, dir: &str, ext: &str) -> Vec<String> {
        fn walk(at: &Path, rel: &str, ext: &str, out: &mut Vec<String>) {
            let Ok(rd) = std::fs::read_dir(at) else { return };
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let path = format!("{rel}/{name}");
                match e.file_type() {
                    Ok(t) if t.is_dir() => walk(&e.path(), &path, ext, out),
                    Ok(t) if t.is_file() && name.to_ascii_lowercase().ends_with(ext) => out.push(path),
                    _ => {}
                }
            }
        }
        let top = std::fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .flatten()
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(dir))
            .map(|e| e.file_name().to_string_lossy().to_string());
        let mut out = vec![];
        if let Some(top) = top {
            walk(&self.root.join(&top), &top, ext, &mut out);
        }
        out.sort();
        out
    }

    fn with_bank<R>(&self, file: &str, f: impl FnOnce(&Bank) -> R) -> Option<R> {
        let mut last = self.last.borrow_mut();
        if last.as_ref().is_none_or(|l| l.0 != file) {
            let bank = nfsu2::studio_api::media::read(&self.root, file).ok().and_then(Bank::parse);
            *last = Some((file.to_string(), bank));
        }
        last.as_ref().and_then(|l| l.1.as_ref()).map(f)
    }
}

/// The sounds of one parsed bank.
pub fn bank_sounds(file: &str, bank: &Bank) -> Vec<Sound> {
    let sound = |name: String, samples: Vec<u32>| {
        let first = samples.first().and_then(|&i| bank.samples.get(i as usize)).and_then(Option::as_ref);
        Sound {
            bank: file.to_string(),
            name,
            samples,
            rate: first.map_or(0, |s| s.rate),
            frames: first.map_or(0, |s| s.samples),
            playable: first.is_some_and(|s| s.codec == 10 && s.channels == 1),
        }
    };
    let mut out = vec![];
    for (name, m) in &bank.events {
        let mut samples: Vec<u32> = vec![];
        for (_, i) in bank.sound_list(&bank.modules[*m]) {
            if bank.samples.get(i as usize).is_some_and(Option::is_some) && !samples.contains(&i) {
                samples.push(i);
            }
        }
        if !samples.is_empty() {
            out.push(sound(name.clone(), samples));
        }
    }
    if out.is_empty() {
        for (i, s) in bank.samples.iter().enumerate() {
            if s.is_some() {
                out.push(sound(format!("sample {i}"), vec![i as u32]));
            }
        }
    }
    out
}

impl Media for GameMedia {
    fn sounds(&self) -> Vec<Sound> {
        self.files("SOUND", ".abk").iter().flat_map(|f| self.with_bank(f, |b| bank_sounds(f, b)).unwrap_or_default()).collect()
    }

    fn pcm(&self, bank: &str, sample: u32) -> Option<Pcm> {
        self.with_bank(bank, |b| b.decode(sample as usize).map(|(info, samples)| Pcm { rate: info.rate, samples }))?
    }

    fn languages(&self) -> Vec<String> {
        // Labels.bin, LanguageTextures.bin and Largest.bin are not languages.
        const NOT: [&str; 3] = ["labels", "languagetextures", "largest"];
        self.files("LANGUAGES", ".bin")
            .iter()
            .filter_map(|f| f.rsplit('/').next()?.rsplit_once('.').map(|s| s.0.to_string()))
            .filter(|n| !NOT.contains(&n.to_ascii_lowercase().as_str()))
            .collect()
    }

    fn strings(&self, language: &str) -> Option<Vec<Line>> {
        let file = self.files("LANGUAGES", ".bin").into_iter().find(|f| {
            f.rsplit('/').next().is_some_and(|n| n.eq_ignore_ascii_case(&format!("{language}.bin")))
        })?;
        let table = Strings::parse(&nfsu2::studio_api::media::read(&self.root, &file).ok()?).ok()?;
        let mut lines: Vec<Line> = table.map.into_iter().map(|(hash, text)| Line { hash, text }).collect();
        lines.sort_by_key(|l| l.hash);
        Some(lines)
    }
}

/// [`Media`] from lists in memory (tests).
#[derive(Default)]
pub struct GivenMedia {
    pub sounds: Vec<Sound>,
    pub pcm: Vec<(String, u32, Pcm)>,
    pub languages: Vec<(String, Vec<Line>)>,
}

impl Media for GivenMedia {
    fn sounds(&self) -> Vec<Sound> {
        self.sounds.clone()
    }

    fn pcm(&self, bank: &str, sample: u32) -> Option<Pcm> {
        self.pcm.iter().find(|p| p.0 == bank && p.1 == sample).map(|p| p.2.clone())
    }

    fn languages(&self) -> Vec<String> {
        self.languages.iter().map(|l| l.0.clone()).collect()
    }

    fn strings(&self, language: &str) -> Option<Vec<Line>> {
        self.languages.iter().find(|l| l.0.eq_ignore_ascii_case(language)).map(|l| l.1.clone())
    }
}

#[cfg(test)]
mod tests;
