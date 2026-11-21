//! Open in your program: the work files a modder paints, watched, each save taken in as one step.
//!
//! - [`Workbench::start`] writes a texture's work file into the mod's [`WORK_DIR`] (`world/<name>.png`): the
//!   mod's image when the mod already changes the texture, else the game's own pixels or, for "Repaint from
//!   blank", an empty canvas of the texture's size. It notes which texture the file is and where it started
//!   ([`WORK_FILE`] in the editor folder). A work file that exists is kept: the modder's paint is never
//!   overwritten.
//! - [`Workbench::poll`] looks at the work files; a file whose size and time stayed the same over one look
//!   (a program still writing it keeps changing it) and whose image differs from what was last taken in is a
//!   save ([`Saved`]).
//! - [`Workbench::import`] takes a save in: the image is kept by its content ([`STORE_DIR`]), the texture's entry
//!   in the list ([`FILE`]) names it, as one step of the mod's history (through the writer's inbox when another
//!   program has the mod open), and the pack data is written from the list ([`materialize`]).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};

use super::{
    Entry, FILE, Looks, MAX_SIDE, Origin, PACK_DIR, STORE_DIR, TextureId, Textures, WORK_DIR, WORK_FILE, decode_png,
    encode_png, image_hash, quote,
};
use crate::history::{History, Outcome, inbox};

/// A "Repaint from blank" canvas's colour (opaque white, as a paint program's new image).
pub const BLANK: [u8; 4] = [255, 255, 255, 255];

/// One work file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkFile {
    pub id: TextureId,
    /// The texture's name in the game.
    #[serde(default)]
    pub name: String,
    /// The file, relative to the mod's folder (`work/looks/world/ROAD_MAIN.png`).
    pub file: String,
    /// Where its pixels started.
    pub origin: Origin,
    /// The image last taken in from it (or written into it), by content.
    #[serde(default)]
    pub image: String,
}

#[derive(Default, Deserialize)]
struct WorkList {
    #[serde(default, rename = "work")]
    files: Vec<WorkFile>,
}

/// A settled save of a work file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub id: TextureId,
    pub png: Vec<u8>,
}

/// What a save came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Imported {
    /// Made as this step.
    Step(u64),
    /// The image is the one the mod has.
    Unchanged,
    /// Another program has the mod open: sent to its inbox as this request (made when it answers).
    Sent(String),
}

type Stamp = Option<(u64, Option<SystemTime>)>;

fn stamp(p: &Path) -> Stamp {
    std::fs::metadata(p).ok().map(|m| (m.len(), m.modified().ok()))
}

/// A mod's work files.
pub struct Workbench {
    dir: PathBuf,
    edir: PathBuf,
    list: WorkList,
    /// Per work file: its stamp at the last look, and the stamp last handled.
    seen: HashMap<TextureId, (Stamp, Stamp)>,
}

/// A file name for a texture's name (`ROAD_MAIN`), else its hash.
fn stem(id: TextureId, name: &str) -> String {
    let safe = !name.is_empty()
        && name.len() <= 120
        && !name.starts_with('.')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'));
    if safe { name.to_string() } else { format!("0x{:08X}", id.hash) }
}

impl Workbench {
    /// The work files of the mod in `dir` (its editor folder `edir`).
    pub fn open(dir: &Path, edir: &Path) -> Workbench {
        let list = std::fs::read_to_string(edir.join(WORK_FILE))
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default();
        let mut w = Workbench { dir: dir.to_path_buf(), edir: edir.to_path_buf(), list, seen: HashMap::new() };
        // What is on disk now counts as handled: only a later save is taken in.
        for f in &w.list.files {
            let s = stamp(&w.dir.join(&f.file));
            w.seen.insert(f.id, (s, s));
        }
        w
    }

    /// The work files, by texture.
    pub fn files(&self) -> &[WorkFile] {
        &self.list.files
    }

    /// The work file of a texture.
    pub fn file(&self, id: TextureId) -> Option<&WorkFile> {
        self.list.files.iter().find(|f| f.id == id)
    }

    /// A work file's path.
    pub fn path(&self, f: &WorkFile) -> PathBuf {
        self.dir.join(&f.file)
    }

    fn keep(&self) -> Result<(), String> {
        let mut text = String::new();
        for f in &self.list.files {
            text.push_str(&format!(
                "[[work]]\nid = {}\nname = {}\nfile = {}\norigin = {}\nimage = {}\n\n",
                quote(&f.id.to_string()),
                quote(&f.name),
                quote(&f.file),
                quote(f.origin.word()),
                quote(&f.image)
            ));
        }
        ob_core::platform::fs::write_atomic(&self.edir.join(WORK_FILE), text.as_bytes(), true)
            .map_err(|e| format!("{WORK_FILE}: {e}"))
    }

    /// Open texture `id` in the modder's program: its work file, written unless it exists. `blank`: "Repaint
    /// from blank" (an empty canvas; refused for a work file that exists, which holds the modder's paint).
    pub fn start(
        &mut self,
        looks: &Looks,
        id: TextureId,
        textures: &dyn Textures,
        blank: bool,
    ) -> Result<WorkFile, String> {
        if let Some(f) = self.file(id).cloned()
            && self.path(&f).is_file()
        {
            if blank {
                return Err(format!(
                    "{} exists and holds your paint; remove it first to repaint from blank",
                    self.path(&f).display()
                ));
            }
            return Ok(f);
        }
        let (name, origin, png) = match looks.entries.get(&id) {
            // The mod's own image of it, where it started kept.
            Some(e) if !blank => {
                let png = std::fs::read(store_path(&self.edir, &e.image))
                    .map_err(|err| format!("{id}: the mod's image {} is missing: {err}", e.image))?;
                (e.name.clone(), e.origin, png)
            }
            _ => {
                let o = textures.original(id).ok_or_else(|| format!("the game has no texture {id}"))?;
                let png = if blank {
                    let px: Vec<u8> = BLANK.iter().copied().cycle().take(o.rgba.len()).collect();
                    encode_png(o.width, o.height, &px)?
                } else {
                    encode_png(o.width, o.height, &o.rgba)?
                };
                (o.name, if blank { Origin::Blank } else { Origin::Game }, png)
            }
        };
        let file = format!("{WORK_DIR}/{}/{}.png", id.domain.word(), stem(id, &name));
        let path = self.dir.join(&file);
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        ob_core::platform::fs::write_atomic(&path, &png, true).map_err(|e| format!("{}: {e}", path.display()))?;
        let f = WorkFile { id, name, file, origin, image: image_hash(&png) };
        self.list.files.retain(|x| x.id != id);
        self.list.files.push(f.clone());
        self.list.files.sort_by_key(|x| x.id);
        self.keep()?;
        let s = stamp(&path);
        self.seen.insert(id, (s, s));
        Ok(f)
    }

    /// Stop watching a texture's work file (the file stays).
    pub fn close(&mut self, id: TextureId) -> Result<bool, String> {
        let before = self.list.files.len();
        self.list.files.retain(|f| f.id != id);
        self.seen.remove(&id);
        self.keep()?;
        Ok(self.list.files.len() != before)
    }

    /// The saves since the last look: work files that changed, stayed the same over one look, and hold another
    /// image than the one last taken in. A file that does not read as PNG is left until it changes again.
    pub fn poll(&mut self) -> Vec<Result<Saved, (TextureId, String)>> {
        let mut out = Vec::new();
        for f in &self.list.files {
            let path = self.dir.join(&f.file);
            let now = stamp(&path);
            let (last, handled) = self.seen.get(&f.id).copied().unwrap_or((None, None));
            self.seen.insert(f.id, (now, handled));
            if now.is_none() || now != last || now == handled {
                continue;
            }
            self.seen.insert(f.id, (now, now));
            let png = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => {
                    out.push(Err((f.id, e.to_string())));
                    continue;
                }
            };
            if image_hash(&png) == f.image {
                continue;
            }
            match check(&png) {
                Ok(_) => out.push(Ok(Saved { id: f.id, png })),
                Err(e) => out.push(Err((f.id, e))),
            }
        }
        out
    }

    /// Take the work file of `id` in now, as it is (the command line's `looks import`).
    pub fn read_now(&mut self, id: TextureId) -> Result<Saved, String> {
        let f = self.file(id).ok_or_else(|| format!("{id} is not open (looks open first)"))?;
        let path = self.path(f);
        let png = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        check(&png)?;
        let s = stamp(&path);
        self.seen.insert(id, (s, s));
        Ok(Saved { id, png })
    }

    /// Take a save in as one step of the mod's history `h` by `author`; the pack data follows.
    pub fn import(&mut self, h: &mut History, author: &str, saved: &Saved) -> Result<Imported, String> {
        let f = self.file(saved.id).cloned().ok_or_else(|| format!("{} is not open", saved.id))?;
        let (width, height) = check(&saved.png)?;
        let image = image_hash(&saved.png);
        // The image first, by its content: a step only ever names an image that is on disk.
        let blob = store_path(&self.edir, &image);
        if !blob.is_file() {
            if let Some(p) = blob.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            ob_core::platform::fs::write_atomic(&blob, &saved.png, true).map_err(|e| e.to_string())?;
        }
        let mut looks = Looks::parse(h.doc(FILE).unwrap_or_default())?;
        let entry = Entry { id: f.id, name: f.name.clone(), origin: f.origin, image: image.clone(), width, height };
        let result = if looks.entries.get(&f.id) == Some(&entry) {
            Imported::Unchanged
        } else {
            looks.entries.insert(f.id, entry);
            let label = format!("Paint {}", if f.name.is_empty() { f.id.to_string() } else { f.name.clone() });
            let docs = BTreeMap::from([(FILE.to_string(), Some(looks.write()))]);
            if h.writable() {
                match h.transact(author, &label, None, &docs, None).map_err(|e| e.to_string())? {
                    Outcome::Made(seq) => {
                        h.save().map_err(|e| format!("not saved: {e}"))?;
                        materialize(&self.dir, &self.edir, &looks)?;
                        Imported::Step(seq)
                    }
                    Outcome::Unchanged => Imported::Unchanged,
                    o => return Err(format!("not made: {o:?}")),
                }
            } else {
                let req =
                    inbox::Request { id: inbox::new_id(), author: author.into(), label, base: Some(h.seq()), docs };
                inbox::submit(&self.edir, &req).map_err(|e| format!("the request was not sent: {e}"))?;
                Imported::Sent(req.id)
            }
        };
        if let Some(x) = self.list.files.iter_mut().find(|x| x.id == f.id) {
            x.image = image;
        }
        self.keep()?;
        Ok(result)
    }

    /// Watch the work files until `until` (or `stop` says so), taking each save in as it settles; `every`: how
    /// often they are looked at. `done(id, result)` after each save.
    pub fn watch(
        &mut self,
        h: &mut History,
        author: &str,
        every: Duration,
        until: Option<Instant>,
        stop: &dyn Fn() -> bool,
        done: &mut dyn FnMut(TextureId, Result<Imported, String>),
    ) -> Result<(), String> {
        let mut text = h.doc(FILE).map(str::to_string);
        loop {
            for s in self.poll() {
                match s {
                    Ok(saved) => {
                        let r = self.import(h, author, &saved);
                        done(saved.id, r);
                    }
                    Err((id, e)) => done(id, Err(e)),
                }
            }
            // Steps another way (undo, the inbox): the pack data follows the list.
            h.tick(Instant::now()).map_err(|e| e.to_string())?;
            let now = h.doc(FILE).map(str::to_string);
            if now != text {
                materialize(&self.dir, &self.edir, &Looks::parse(now.as_deref().unwrap_or_default())?)?;
                text = now;
            }
            if stop() || until.is_some_and(|u| Instant::now() >= u) {
                return Ok(());
            }
            std::thread::sleep(every);
        }
    }
}

/// A PNG the game's loader takes: its size.
pub fn check(png: &[u8]) -> Result<(u32, u32), String> {
    let (w, h, _) = decode_png(png).map_err(|e| format!("not a PNG the game reads: {e}"))?;
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(format!("{w}x{h}: each side must be 1 to {MAX_SIDE}"));
    }
    Ok((w, h))
}

/// The kept image `image` (a content hash) of the mod with editor folder `edir`.
pub fn store_path(edir: &Path, image: &str) -> PathBuf {
    edir.join(STORE_DIR).join(format!("{image}.png"))
}

/// After a program that has the mod open wrote its files: the pack data follows the texture list when the list's
/// text changed from `before` (as last written) to `after` (now), whoever made the step (an inbox request, undo,
/// the Changes page), so `textures/` always holds what `looks.toml` says. The number of files changed.
pub fn follow(dir: &Path, edir: &Path, before: Option<&str>, after: Option<&str>) -> Result<usize, String> {
    if before == after {
        return Ok(0);
    }
    materialize(dir, edir, &Looks::parse(after.unwrap_or_default())?)
}

/// Write the mod's pack data ([`PACK_DIR`]) from its list: each texture's image where the game reads it (only
/// the files that differ), and no file of a texture the list does not have. The number of files changed.
pub fn materialize(dir: &Path, edir: &Path, looks: &Looks) -> Result<usize, String> {
    let pack = dir.join(PACK_DIR);
    let mut changed = 0;
    let mut wanted = std::collections::HashSet::new();
    for e in looks.entries.values() {
        let path = pack.join(e.id.pack_file());
        wanted.insert(path.clone());
        if std::fs::read(&path).is_ok_and(|b| image_hash(&b) == e.image) {
            continue;
        }
        let png = std::fs::read(store_path(edir, &e.image))
            .map_err(|err| format!("{}: the image {} is missing: {err}", e.id, e.image))?;
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|err| err.to_string())?;
        }
        ob_core::platform::fs::write_atomic(&path, &png, true).map_err(|err| format!("{}: {err}", path.display()))?;
        changed += 1;
    }
    for d in super::Domain::ALL {
        let Ok(rd) = std::fs::read_dir(pack.join(d.word())) else { continue };
        for f in rd.flatten() {
            let name = f.file_name();
            let name = name.to_string_lossy();
            let ours = name.len() == 12
                && name.ends_with(".png")
                && name[..8].bytes().all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b));
            if ours && !wanted.contains(&f.path()) && std::fs::remove_file(f.path()).is_ok() {
                changed += 1;
            }
        }
    }
    Ok(changed)
}
