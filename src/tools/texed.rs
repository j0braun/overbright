//! The live texture browser and editor (docs/texture-editor.md): an opt-in
//! modding tool. Off by default and unreachable until the game is started
//! with `--editor`; then F7 opens and closes its panel.
//!
//! While the panel is open the renderers note the textures they draw
//! ([`recording`]: the world's main view and the cars), and the panel lists
//! them (name, hash, size, format, mips, the pack that replaces it). For the
//! selected texture: **Highlight** draws it with a checker tint wherever it
//! is used; **Export PNG / DDS** writes the game's original to
//! `<data>/overbright/texture-editor/exports/`; **Import** takes the edited
//! export into the workspace pack (an Overbright pack, `obpack`) and shows
//! it at once, and from then on every save of that file shows again (hot
//! reload); **Revert** takes it out of the pack. **Save pack** writes the
//! pack's manifest (name, author, licence) and compiles it; **Packs** lists
//! the installed packs, Enter on one makes it the workspace pack. Nothing is
//! ever written into the game's folder.

use super::overlay::{self, Input, ListState, NavKey, Panel, TextField, colour};
use crate::formats::texture::{PixelFormat, Texture};
use crate::obpack::{self, Domain, Manifest};
use glam::Vec2;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// ------------------------------------------------------------- the switch

static ENABLED: AtomicBool = AtomicBool::new(false);
static RECORDING: AtomicBool = AtomicBool::new(false);

/// `--editor`: the tool can be opened (its hotkey works).
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// The renderers note what they draw (only while the panel is open).
pub fn recording() -> bool {
    RECORDING.load(Ordering::Relaxed)
}

// ------------------------------------------------------------- what was drawn

#[derive(Default)]
struct Noted {
    world_slots: Vec<usize>,
    textures: Vec<(Domain, Texture)>,
}

fn noted() -> &'static Mutex<Noted> {
    static N: OnceLock<Mutex<Noted>> = OnceLock::new();
    N.get_or_init(Default::default)
}

/// The world renderer's main view drew these texture slots.
pub fn note_world_slots(slots: impl Iterator<Item = usize>) {
    let mut n = noted().lock().unwrap_or_else(|e| e.into_inner());
    n.world_slots.extend(slots);
    n.world_slots.sort_unstable();
    n.world_slots.dedup();
}

/// A renderer drew these textures (`car`: the cars').
pub fn note_textures<'a>(car: bool, textures: impl Iterator<Item = &'a Texture>) {
    let d = if car { Domain::Car } else { Domain::World };
    let mut n = noted().lock().unwrap_or_else(|e| e.into_inner());
    for t in textures {
        if !n.textures.iter().any(|(dd, x)| *dd == d && x.hash == t.hash) {
            n.textures.push((d, t.clone()));
        }
    }
}

/// What was noted since the last call: the world's slots and the other
/// textures.
pub fn take_noted() -> (Vec<usize>, Vec<(Domain, Texture)>) {
    let mut n = noted().lock().unwrap_or_else(|e| e.into_inner());
    (std::mem::take(&mut n.world_slots), std::mem::take(&mut n.textures))
}

// ------------------------------------------------------------- the panel

/// One listed texture.
#[derive(Clone, Debug)]
pub struct Row {
    pub domain: Domain,
    pub texture: Texture,
}

impl Row {
    pub fn format_name(&self) -> &'static str {
        format_name(self.texture.format)
    }
}

pub fn format_name(f: PixelFormat) -> &'static str {
    match f {
        PixelFormat::Dxt1 => "DXT1",
        PixelFormat::Dxt3 => "DXT3",
        PixelFormat::Dxt5 => "DXT5",
        PixelFormat::P8 => "P8",
        PixelFormat::Bgra8 => "BGRA8",
        PixelFormat::Rgba8 => "RGBA8",
        PixelFormat::Bc7 => "BC7",
    }
}

/// Where the editor reads and writes.
#[derive(Clone, Debug)]
pub struct Dirs {
    /// Installed packs (`obpack::packs_dir`).
    pub packs: PathBuf,
    /// Compiled packs (`obpack::compiled_root`).
    pub compiled: PathBuf,
    /// Exported originals, and the edited files read back.
    pub exports: PathBuf,
}

impl Dirs {
    pub fn player() -> Dirs {
        Dirs {
            packs: obpack::packs_dir(),
            compiled: obpack::compiled_root(),
            exports: crate::platform::dirs::app().join("texture-editor").join("exports"),
        }
    }
}

/// What the list shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Textures,
    Packs,
}

/// A finished background job: its message and whether it went well.
type JobResult = (String, bool, Vec<(Domain, u32)>);

/// The editor's state (kept by the game while the tool is enabled).
pub struct Editor {
    pub open: bool,
    pub dirs: Dirs,
    view: View,
    /// Every texture seen while the list was live, by (domain, hash).
    seen: BTreeMap<(Domain, u32), Row>,
    /// The rows shown (filtered), refreshed at most every [`REFRESH`].
    rows: Vec<(Domain, u32)>,
    refreshed: Option<Instant>,
    /// The list keeps what it has (no new textures added).
    pub frozen: bool,
    list: ListState,
    packs_list: ListState,
    filter: TextField,
    pub name: TextField,
    pub author: TextField,
    pub licence: TextField,
    /// The workspace pack's id (None: made at the first import).
    pub workspace: Option<String>,
    highlight: Option<(Domain, u32)>,
    /// Imported textures and their source file's stamp: a change re-imports.
    linked: HashMap<(Domain, u32), (PathBuf, Option<(u64, u128)>)>,
    polled: Option<Instant>,
    status: (String, bool),
    jobs: Vec<std::thread::JoinHandle<JobResult>>,
    /// The packs in use (which one replaces a texture), read at a refresh.
    winners: Vec<obpack::Pack>,
    /// The installed packs, read when the packs view opens.
    installed: Vec<obpack::Listed>,
}

/// How often the list takes the textures drawn since.
pub const REFRESH: Duration = Duration::from_millis(500);
/// How often linked files are looked at.
pub const POLL: Duration = Duration::from_millis(250);
/// Rows the list shows.
pub const ROWS: usize = 18;
/// The panel's place and width (canvas points, left edge).
pub const ORIGIN: Vec2 = Vec2::new(6.0, 6.0);
pub const WIDTH: f32 = 236.0;

/// The checker tint the highlight draws with: magenta and cyan cells, each
/// one eighth of the texture's longer side, over the texture's own pixels.
pub fn checker(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let cell = (w.max(h) / 8).max(1);
    let mut out = rgba.to_vec();
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let tint: [u16; 3] = if ((x / cell) + (y / cell)) % 2 == 0 { [255, 0, 255] } else { [0, 255, 255] };
            for c in 0..3 {
                out[i + c] = ((u16::from(rgba[i + c]) + 3 * tint[c]) / 4) as u8;
            }
            // Cut-outs stay visible where they are drawn at all.
            out[i + 3] = rgba[i + 3].max(160);
        }
    }
    out
}

/// The file stem an export of `t` is written under.
pub fn export_stem(t: &Texture) -> String {
    obpack::stem_for(t.hash, &t.name)
}

/// `t`'s top level as PNG bytes.
pub fn png_of(t: &Texture) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    let img = image::RgbaImage::from_raw(t.width, t.height, t.decode_rgba())
        .ok_or_else(|| anyhow::anyhow!("{}: no pixels", t.name))?;
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}

/// `t` as a DDS: DXT textures as they are (DX9, every stored level), others
/// as BC7 (DX10) of their top level with a mip chain.
pub fn dds_of(t: &Texture) -> Vec<u8> {
    if matches!(t.format, PixelFormat::Dxt1 | PixelFormat::Dxt3 | PixelFormat::Dxt5)
        && t.width % 4 == 0
        && t.height % 4 == 0
    {
        let size: usize = (0..t.mip_count)
            .map(|m| t.format.level_size((t.width >> m).max(1), (t.height >> m).max(1)))
            .sum::<usize>()
            .min(t.data.len());
        let e = crate::upscale::Encoded {
            format: t.format,
            width: t.width,
            height: t.height,
            mips: t.mip_count,
            data: t.data[..size].to_vec(),
        };
        return crate::upscale::dds_bytes(&e);
    }
    obpack::dds::dx10_bytes(&obpack::dds::encode_bc7(&t.decode_rgba(), t.width, t.height))
}

fn stamp(p: &Path) -> Option<(u64, u128)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.len(), m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos()))
}

impl Editor {
    pub fn new(dirs: Dirs) -> Editor {
        Editor {
            open: false,
            dirs,
            view: View::Textures,
            seen: BTreeMap::new(),
            rows: Vec::new(),
            refreshed: None,
            frozen: false,
            list: ListState::default(),
            packs_list: ListState::default(),
            filter: TextField::new(""),
            name: TextField::new("My textures"),
            author: TextField::new(""),
            licence: TextField::new(""),
            workspace: None,
            highlight: None,
            linked: HashMap::new(),
            polled: None,
            status: ("Select a texture.".into(), true),
            jobs: Vec::new(),
            winners: Vec::new(),
            installed: Vec::new(),
        }
    }

    /// F7: open or close the panel. Closing takes the highlight away and
    /// stops the renderers' notes.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        RECORDING.store(self.open, Ordering::Relaxed);
        if !self.open {
            self.set_highlight(None);
        } else {
            take_noted();
        }
    }

    fn say(&mut self, text: impl Into<String>, ok: bool) {
        self.status = (text.into(), ok);
    }

    /// The selected texture.
    pub fn selected(&self) -> Option<&Row> {
        let k = self.rows.get(self.list.selected?)?;
        self.seen.get(k)
    }

    /// The textures noted this frame, as rows (`world`: slot → texture, the
    /// world renderer's own).
    pub fn take_frame(&mut self, world: &dyn Fn(usize) -> Option<Texture>, now: Instant) {
        if !self.open {
            return;
        }
        let (slots, others) = take_noted();
        if self.frozen {
            return;
        }
        for t in slots.into_iter().filter_map(world) {
            self.seen.entry((Domain::World, t.hash)).or_insert(Row { domain: Domain::World, texture: t });
        }
        for (d, t) in others {
            self.seen.entry((d, t.hash)).or_insert(Row { domain: d, texture: t });
        }
        if self.refreshed.is_none_or(|r| now.duration_since(r) >= REFRESH) {
            self.refreshed = Some(now);
            self.refilter();
        }
    }

    /// Forget the listed textures (a new world).
    pub fn clear(&mut self) {
        self.set_highlight(None);
        self.seen.clear();
        self.rows.clear();
        self.list = ListState::default();
    }

    fn refilter(&mut self) {
        self.winners = obpack::enabled_in(&self.dirs.packs);
        let keep = self.list.selected.and_then(|i| self.rows.get(i).copied());
        let f = self.filter.text.to_ascii_lowercase();
        self.rows = self
            .seen
            .iter()
            .filter(|(_, r)| {
                f.is_empty()
                    || r.texture.name.to_ascii_lowercase().contains(&f)
                    || format!("{:08x}", r.texture.hash).contains(&f)
            })
            .map(|(k, _)| *k)
            .collect();
        self.rows.sort_by(|a, b| {
            let (ra, rb) = (&self.seen[a], &self.seen[b]);
            (ra.domain, ra.texture.name.as_str(), ra.texture.hash).cmp(&(
                rb.domain,
                rb.texture.name.as_str(),
                rb.texture.hash,
            ))
        });
        self.list.selected = keep.and_then(|k| self.rows.iter().position(|r| *r == k));
        self.list.clamp(self.rows.len(), ROWS);
    }

    fn row_text(&self, k: &(Domain, u32)) -> String {
        let winners = &self.winners;
        let r = &self.seen[k];
        let t = &r.texture;
        let from = obpack::winner(winners, r.domain, t.hash).map(|p| format!(" [{}]", p.id)).unwrap_or_default();
        let lit = if self.highlight == Some(*k) { "* " } else { "" };
        let name = if t.name.is_empty() { format!("{:08X}", t.hash) } else { t.name.clone() };
        format!(
            "{lit}{}{} {}x{} {}{}",
            if r.domain == Domain::Car { "car " } else { "" },
            name,
            t.width,
            t.height,
            r.format_name(),
            from
        )
    }

    // ----------------------------------------------------------- actions

    fn set_highlight(&mut self, k: Option<(Domain, u32)>) {
        if let Some((d, h)) = self.highlight.take() {
            crate::render::pack_reload::highlight(d == Domain::Car, h, None);
        }
        if let Some(k) = k
            && let Some(r) = self.seen.get(&k)
        {
            let t = &r.texture;
            let tinted = checker(&t.decode_rgba(), t.width, t.height);
            crate::render::pack_reload::highlight(k.0 == Domain::Car, k.1, Some((t.width, t.height, tinted)));
            self.highlight = Some(k);
        }
    }

    /// Export the selected texture's original (`dds`: as DDS, else PNG).
    pub fn export(&mut self, dds: bool) -> anyhow::Result<PathBuf> {
        let r = self.selected().ok_or_else(|| anyhow::anyhow!("no texture selected"))?.clone();
        std::fs::create_dir_all(&self.dirs.exports)?;
        let stem = export_stem(&r.texture);
        let path = self.dirs.exports.join(format!("{stem}.{}", if dds { "dds" } else { "png" }));
        let bytes = if dds { dds_of(&r.texture) } else { png_of(&r.texture)? };
        crate::qol::saves::write_atomic(&path, &bytes, false)?;
        Ok(path)
    }

    /// The workspace pack, made (and put first in the order) when there is
    /// none yet.
    fn workspace_pack(&mut self) -> anyhow::Result<obpack::Pack> {
        if let Some(id) = &self.workspace
            && let Ok(p) = obpack::Pack::open(&self.dirs.packs.join(id))
        {
            return Ok(p);
        }
        let m = self.manifest();
        let id = obpack::new_id(&self.dirs.packs, &m.name);
        let p = obpack::create(&self.dirs.packs, &id, &m)?;
        self.workspace = Some(id.clone());
        put_first(&self.dirs.packs, &id)?;
        Ok(p)
    }

    /// The manifest the fields say.
    pub fn manifest(&self) -> Manifest {
        Manifest {
            format: obpack::manifest::FORMAT,
            name: if self.name.text.trim().is_empty() { "My textures".into() } else { self.name.text.trim().into() },
            author: self.author.text.trim().into(),
            licence: self.licence.text.trim().into(),
            priority: 100,
            source: "editor".into(),
            ..Default::default()
        }
    }

    /// The edited file for the selected texture: its export, PNG or DDS,
    /// whichever was saved last.
    fn edited_file(&self, t: &Texture) -> Option<PathBuf> {
        let stem = export_stem(t);
        ["png", "dds"]
            .iter()
            .map(|e| self.dirs.exports.join(format!("{stem}.{e}")))
            .filter(|p| p.is_file())
            .max_by_key(|p| stamp(p).map(|s| s.1))
    }

    /// Import the edited export of the selected texture into the workspace
    /// pack (in the background) and keep watching it.
    pub fn import_selected(&mut self) -> anyhow::Result<()> {
        let r = self.selected().ok_or_else(|| anyhow::anyhow!("no texture selected"))?.clone();
        let file = self
            .edited_file(&r.texture)
            .ok_or_else(|| anyhow::anyhow!("export it first, edit {}.png, then import", export_stem(&r.texture)))?;
        let pack = self.workspace_pack()?;
        self.linked.insert((r.domain, r.texture.hash), (file.clone(), stamp(&file)));
        self.spawn_import(pack, r, file);
        Ok(())
    }

    fn spawn_import(&mut self, pack: obpack::Pack, r: Row, file: PathBuf) {
        let compiled = self.dirs.compiled.join(&pack.id);
        self.jobs.push(std::thread::spawn(move || {
            let res = (|| -> anyhow::Result<()> {
                let bytes = std::fs::read(&file)?;
                put_file(&pack, r.domain, &r.texture, &file, &bytes)?;
                let pack = obpack::Pack::open(&pack.dir)?;
                let rep = obpack::sync(&pack, &compiled, &mut |_, _| {})?;
                if let Some((f, why)) = rep.failed.first() {
                    anyhow::bail!("{}: {why}", f.display());
                }
                Ok(())
            })();
            match res {
                Ok(()) => (format!("Imported {}.", export_stem(&r.texture)), true, vec![(r.domain, r.texture.hash)]),
                Err(e) => (format!("Import failed: {e:#}"), false, Vec::new()),
            }
        }));
    }

    /// Take the selected texture out of the workspace pack.
    pub fn revert_selected(&mut self) -> anyhow::Result<()> {
        let r = self.selected().ok_or_else(|| anyhow::anyhow!("no texture selected"))?.clone();
        let id = self.workspace.clone().ok_or_else(|| anyhow::anyhow!("no workspace pack"))?;
        let pack = obpack::Pack::open(&self.dirs.packs.join(&id))?;
        self.linked.remove(&(r.domain, r.texture.hash));
        let Some(e) = pack.index.domain(r.domain).get(&r.texture.hash) else {
            anyhow::bail!("the pack does not replace it");
        };
        std::fs::remove_file(pack.dir.join(&e.file))?;
        let pack = obpack::Pack::open(&pack.dir)?;
        obpack::sync(&pack, &self.dirs.compiled.join(&id), &mut |_, _| {})?;
        changed(&[(r.domain, r.texture.hash)]);
        Ok(())
    }

    /// Write the workspace pack's manifest and compile it (in the background).
    pub fn save_pack(&mut self) -> anyhow::Result<()> {
        let pack = self.workspace_pack()?;
        obpack::write_manifest(&pack, &self.manifest())?;
        put_first(&self.dirs.packs, &pack.id)?;
        let compiled = self.dirs.compiled.join(&pack.id);
        let keys: Vec<(Domain, u32)> =
            Domain::ALL.iter().flat_map(|&d| pack.index.domain(d).keys().map(move |&h| (d, h))).collect();
        self.jobs.push(std::thread::spawn(move || {
            let r = obpack::Pack::open(&pack.dir).and_then(|p| obpack::sync(&p, &compiled, &mut |_, _| {}));
            match r {
                Ok(r) => (
                    format!("Saved {} ({} textures, {} compiled).", pack.id, keys.len(), r.compiled),
                    r.failed.is_empty(),
                    keys,
                ),
                Err(e) => (format!("Save failed: {e:#}"), false, Vec::new()),
            }
        }));
        Ok(())
    }

    /// Make installed pack `id` the workspace pack (first in the order, on)
    /// and take its manifest into the fields; it is compiled in the
    /// background.
    pub fn load_pack(&mut self, id: &str) -> anyhow::Result<()> {
        let p = obpack::Pack::open(&self.dirs.packs.join(id))?;
        self.name.text = p.manifest.name.clone();
        self.author.text = p.manifest.author.clone();
        self.licence.text = p.manifest.licence.clone();
        self.workspace = Some(p.id.clone());
        self.linked.clear();
        put_first(&self.dirs.packs, &p.id)?;
        let compiled = self.dirs.compiled.join(&p.id);
        let keys: Vec<(Domain, u32)> =
            Domain::ALL.iter().flat_map(|&d| p.index.domain(d).keys().map(move |&h| (d, h))).collect();
        self.jobs.push(std::thread::spawn(move || match obpack::sync(&p, &compiled, &mut |_, _| {}) {
            Ok(_) => (format!("Editing {} ({} textures).", p.id, keys.len()), true, keys),
            Err(e) => (format!("{e:#}"), false, Vec::new()),
        }));
        Ok(())
    }

    /// Finished jobs' messages, and linked files saved again.
    pub fn poll(&mut self, now: Instant) {
        let (done, running): (Vec<_>, Vec<_>) = self.jobs.drain(..).partition(|j| j.is_finished());
        self.jobs = running;
        for j in done {
            if let Ok((msg, ok, keys)) = j.join() {
                changed(&keys);
                self.say(msg, ok);
                self.winners = obpack::enabled_in(&self.dirs.packs);
            }
        }
        if self.polled.is_some_and(|p| now.duration_since(p) < POLL) || !self.jobs.is_empty() {
            return;
        }
        self.polled = Some(now);
        let Some(id) = self.workspace.clone() else { return };
        let mut again = Vec::new();
        for (k, (file, seen)) in self.linked.iter_mut() {
            let s = stamp(file);
            if s.is_some() && s != *seen {
                *seen = s;
                again.push((*k, file.clone()));
            }
        }
        if again.is_empty() {
            return;
        }
        let Ok(pack) = obpack::Pack::open(&self.dirs.packs.join(&id)) else { return };
        for (k, file) in again {
            if let Some(r) = self.seen.get(&k).cloned() {
                self.spawn_import(pack.clone(), r, file);
            }
        }
    }

    // ----------------------------------------------------------- drawing

    /// Build the panel for this frame's input; the actions it asks for are
    /// done here. None while closed.
    pub fn panel(&mut self, font: &crate::frontend::font::Font, input: &Input) -> Option<overlay::Built> {
        if !self.open {
            return None;
        }
        let before = self.filter.text.clone();
        let mut p = Panel::new(font, input, ORIGIN, WIDTH);
        p.title("Texture editor (F7 closes)");
        let mut action = None;
        match self.view {
            View::Textures => {
                p.label(
                    &format!("{} textures{}", self.rows.len(), if self.frozen { ", frozen" } else { " drawn" }),
                    colour::DIM,
                );
                p.text_field("Find", &mut self.filter);
                let items: Vec<String> = self.rows.iter().map(|k| self.row_text(k)).collect();
                let ev = p.list(&mut self.list, &items, ROWS);
                if ev.activated {
                    action = Some("highlight");
                }
                match self.selected() {
                    Some(r) => {
                        let t = &r.texture;
                        p.label(
                            &format!("{} {:08X}", if t.name.is_empty() { "-" } else { &t.name }, t.hash),
                            colour::TEXT,
                        );
                        p.label(
                            &format!("{}x{} {} mips {}", t.width, t.height, format_name(t.format), t.mip_count),
                            colour::DIM,
                        );
                    }
                    None => {
                        p.label("-", colour::DIM);
                        p.label("", colour::DIM);
                    }
                }
                let lit = self.highlight.is_some()
                    && self.highlight == self.rows.get(self.list.selected.unwrap_or(usize::MAX)).copied();
                match p.buttons(&[if lit { "Unlight" } else { "Highlight" }, "Export PNG", "Export DDS"]) {
                    Some(0) => action = Some("highlight"),
                    Some(1) => action = Some("png"),
                    Some(2) => action = Some("dds"),
                    _ => {}
                }
                match p.buttons(&["Import", "Revert", if self.frozen { "Live" } else { "Freeze" }]) {
                    Some(0) => action = Some("import"),
                    Some(1) => action = Some("revert"),
                    Some(2) => action = Some("freeze"),
                    _ => {}
                }
            }
            View::Packs => {
                let list = self.installed.clone();
                let items: Vec<String> = list
                    .iter()
                    .map(|l| match &l.pack {
                        Ok(p) => format!("{} {} ({})", if l.on { "on " } else { "off" }, p.manifest.name, l.id),
                        Err(e) => format!("bad {}: {e}", l.id),
                    })
                    .collect();
                p.label("Installed packs (Enter: edit this one)", colour::DIM);
                let ev = p.list(&mut self.packs_list, &items, ROWS + 4);
                if ev.activated
                    && let Some(l) = self.packs_list.selected.and_then(|i| list.get(i))
                {
                    let id = l.id.clone();
                    match self.load_pack(&id) {
                        Ok(()) => {
                            self.say(format!("Loading {id}..."), true);
                            self.view = View::Textures;
                        }
                        Err(e) => self.say(format!("{e:#}"), false),
                    }
                }
            }
        }
        p.space(2.0);
        p.label(&format!("Pack: {}", self.workspace.as_deref().unwrap_or("(new at the first import)")), colour::DIM);
        p.text_field("Name", &mut self.name);
        p.text_field("Author", &mut self.author);
        p.text_field("Licence", &mut self.licence);
        match p.buttons(&["Save pack", if self.view == View::Packs { "Textures" } else { "Packs" }]) {
            Some(0) => action = Some("save"),
            Some(1) => action = Some("view"),
            _ => {}
        }
        let (msg, ok) = self.status.clone();
        p.label(&msg, if ok { colour::OK } else { colour::WARN });
        if !self.jobs.is_empty() {
            p.label("Working...", colour::DIM);
        }
        let built = p.finish();
        if self.filter.text != before {
            self.refilter();
        }
        if input.keys.contains(&NavKey::Escape) && !built.typing && action.is_none() {
            // Escape outside a field closes the packs view.
            if self.view == View::Packs {
                self.view = View::Textures;
            }
        }
        if let Some(a) = action {
            self.act(a);
        }
        Some(built)
    }

    fn act(&mut self, a: &str) {
        let k = self.list.selected.and_then(|i| self.rows.get(i).copied());
        let r: anyhow::Result<String> = match a {
            "highlight" => {
                let on = k.is_some() && self.highlight != k;
                self.set_highlight(if on { k } else { None });
                Ok(if on { "Highlighted (checker tint)." } else { "Highlight off." }.into())
            }
            "png" | "dds" => self.export(a == "dds").map(|p| format!("Wrote {}", p.display())),
            "import" => self.import_selected().map(|()| "Importing...".into()),
            "revert" => self.revert_selected().map(|()| "Reverted to the original.".into()),
            "freeze" => {
                self.frozen = !self.frozen;
                Ok(if self.frozen { "List frozen." } else { "List live." }.into())
            }
            "save" => self.save_pack().map(|()| "Saving...".into()),
            "view" => {
                self.view = if self.view == View::Packs { View::Textures } else { View::Packs };
                self.installed = obpack::list_in(&self.dirs.packs);
                Ok(String::new())
            }
            _ => Ok(String::new()),
        };
        match r {
            Ok(m) if m.is_empty() => {}
            Ok(m) => self.say(m, true),
            Err(e) => self.say(format!("{e:#}"), false),
        }
    }
}

/// The renderers load these textures again (the loader's packs re-read).
fn changed(keys: &[(Domain, u32)]) {
    if keys.is_empty() {
        return;
    }
    obpack::forget_roots();
    for (d, h) in keys {
        crate::render::pack_reload::texture_changed(*d == Domain::Car, *h);
    }
}

/// Pack `id` first in the order, on.
pub fn put_first(packs: &Path, id: &str) -> std::io::Result<()> {
    let mut order = obpack::load_order(packs);
    order.retain(|e| e.id != id);
    // Packs not named yet keep their place after it.
    let named: Vec<String> = order.iter().map(|e| e.id.clone()).collect();
    for l in obpack::list_in(packs) {
        if l.id != id && !named.contains(&l.id) {
            order.push(obpack::OrderEntry { id: l.id, on: l.on });
        }
    }
    order.insert(0, obpack::OrderEntry { id: id.to_string(), on: true });
    obpack::save_order(packs, &order)
}

/// An edited file (PNG or DDS) into `pack` as `t`'s replacement for `domain`.
fn put_file(pack: &obpack::Pack, domain: Domain, t: &Texture, file: &Path, bytes: &[u8]) -> anyhow::Result<PathBuf> {
    let dds = file.extension().is_some_and(|e| e.eq_ignore_ascii_case("dds"));
    if !dds {
        return obpack::put_png(pack, domain, t.hash, &t.name, bytes);
    }
    let e = obpack::parse_any_dds(bytes).ok_or_else(|| anyhow::anyhow!("not a DDS Overbright reads"))?;
    if e.width > obpack::MAX_SIDE || e.height > obpack::MAX_SIDE {
        anyhow::bail!("{}x{} is larger than {}", e.width, e.height, obpack::MAX_SIDE);
    }
    let dir = pack.dir.join(domain.folder());
    std::fs::create_dir_all(&dir)?;
    if let Some(old) = pack.index.domain(domain).get(&t.hash)
        && old.file.starts_with(domain.folder())
    {
        let _ = std::fs::remove_file(pack.dir.join(&old.file));
    }
    let path = dir.join(format!("{}.dds", obpack::stem_for(t.hash, &t.name)));
    crate::qol::saves::write_atomic(&path, bytes, false)?;
    Ok(path)
}

#[cfg(test)]
mod tests;
