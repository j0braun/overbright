//! The editor's window contents without the window: the layout (menu,
//! toolbar, mod tree, viewport, properties, bottom tabs, status bar), the
//! dialogs, opening / making / saving a mod (atomic), autosave with its
//! restore prompt, the command line, and painting one frame with egui-wgpu
//! into any texture (the window's, or an off-screen one for `--shot`).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context as _, Result, bail, ensure};
use glam::Vec3;

use super::menu;
use super::state::{EditorState, Level, Mode, Panel, View};
use super::theme;
use super::undo;
use crate::draw::camera::Camera;
use crate::render::Gpu;
use crate::tools::map::{MapMod, store, valid_id, valid_region};
use crate::world::World;

/// The region a new mod starts on.
pub const DEFAULT_REGION: &str = "L4RA";
/// Autosave interval.
pub const AUTOSAVE_EVERY: Duration = Duration::from_secs(60);
/// The autosave beside `map.obmap`.
pub const AUTOSAVE: &str = "map.obmap.autosave";
/// Editor-only state beside `map.obmap` (camera, region); not exported.
pub const EDITOR_TOML: &str = "editor.toml";
/// How many recently opened mods the File menu lists.
pub const RECENT: usize = 8;

// --- files ------------------------------------------------------------------

/// Write `m` as `<maps>/<folder>/map.obmap`: through a temporary file,
/// flushed to the disk, then renamed over the old one, so a crash or a
/// full disk leaves the old file whole.
pub fn save_mod(maps: &Path, folder: &str, m: &MapMod) -> Result<PathBuf> {
    ensure!(valid_id(folder), "`{folder}` is not a map mod folder name (a-z, 0-9, - and _)");
    let path = maps.join(folder).join(store::FILE);
    crate::qol::saves::write_atomic(&path, m.write().as_bytes(), true)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

pub fn autosave_path(maps: &Path, folder: &str) -> PathBuf {
    maps.join(folder).join(AUTOSAVE)
}

/// Write the autosave (atomic, as a save).
pub fn write_autosave(maps: &Path, folder: &str, m: &MapMod) -> Result<()> {
    let path = autosave_path(maps, folder);
    crate::qol::saves::write_atomic(&path, m.write().as_bytes(), true)
        .with_context(|| format!("writing {}", path.display()))
}

/// Remove the autosave (after a save, or when its changes are discarded).
pub fn remove_autosave(maps: &Path, folder: &str) {
    let p = autosave_path(maps, folder);
    if p.exists()
        && let Err(e) = std::fs::remove_file(&p)
    {
        log::warn!("editor: could not remove {}: {e}", p.display());
    }
}

/// The autosave to offer on opening: newer than the saved mod (or the mod
/// is missing), readable, and different from `saved`. With its time.
pub fn pending_autosave(maps: &Path, folder: &str, saved: Option<&MapMod>) -> Option<(MapMod, SystemTime)> {
    let auto = autosave_path(maps, folder);
    let at = std::fs::metadata(&auto).and_then(|m| m.modified()).ok()?;
    let file = maps.join(folder).join(store::FILE);
    if let Ok(t) = std::fs::metadata(&file).and_then(|m| m.modified())
        && t > at
    {
        return None;
    }
    let m = match store::read_file(&auto) {
        Ok(m) => m,
        Err(e) => {
            log::warn!("editor: autosave not readable: {e:#}");
            return None;
        }
    };
    (saved != Some(&m)).then_some((m, at))
}

/// Editor-only state of a mod (`editor.toml`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ModView {
    pub region: Option<String>,
    pub camera: Option<(Vec3, f32, f32)>,
}

impl ModView {
    pub fn read(maps: &Path, folder: &str) -> ModView {
        let Ok(text) = std::fs::read_to_string(maps.join(folder).join(EDITOR_TOML)) else { return ModView::default() };
        let Ok(t) = toml::from_str::<toml::Table>(&text) else { return ModView::default() };
        let region = t.get("region").and_then(|v| v.as_str()).filter(|r| valid_region(r)).map(str::to_string);
        let f = |k: &str| {
            t.get(k).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).map(|v| v as f32)
        };
        let camera = match (t.get("camera").and_then(|v| v.as_array()), f("yaw"), f("pitch")) {
            (Some(a), Some(yaw), Some(pitch)) if a.len() == 3 => {
                let c: Vec<f32> = a
                    .iter()
                    .filter_map(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
                    .map(|v| v as f32)
                    .collect();
                (c.len() == 3 && c.iter().all(|v| v.is_finite())).then(|| (Vec3::new(c[0], c[1], c[2]), yaw, pitch))
            }
            _ => None,
        };
        ModView { region, camera }
    }

    pub fn write(&self, maps: &Path, folder: &str) -> Result<()> {
        let mut s = String::from("# Overbright map editor: this mod's view (not part of the mod).\n");
        if let Some(r) = &self.region {
            s.push_str(&format!("region = \"{r}\"\n"));
        }
        if let Some((p, yaw, pitch)) = self.camera {
            s.push_str(&format!("camera = [{:?}, {:?}, {:?}]\nyaw = {:?}\npitch = {:?}\n", p.x, p.y, p.z, yaw, pitch));
        }
        crate::qol::saves::write_atomic(&maps.join(folder).join(EDITOR_TOML), s.as_bytes(), false)?;
        Ok(())
    }
}

/// The region a mod is edited on: the command line's, else editor.toml's,
/// else its first edit's, else the first race's base region (L4RA).
pub fn mod_region(cli: Option<&str>, view: &ModView, m: &MapMod) -> String {
    cli.map(str::to_string)
        .or_else(|| view.region.clone())
        .or_else(|| m.props.first().map(|e| e.region().to_ascii_uppercase()))
        .unwrap_or_else(|| DEFAULT_REGION.to_string())
}

/// The recently opened mods' list's file name.
pub const RECENT_FILE: &str = "editor-recent.txt";

/// The recently opened mods' list: in Overbright's own folder
/// (`platform::dirs::config`, so a portable install's `data/` and the
/// locations file are honoured) for the map mods' folder in use; beside
/// another maps folder given with `--maps`.
fn recent_file(maps: &Path) -> PathBuf {
    if maps == crate::platform::dirs::maps() {
        return crate::platform::dirs::config().join(RECENT_FILE);
    }
    maps.parent().unwrap_or(maps).join(RECENT_FILE)
}

/// The recently opened mods, newest first.
pub fn recent(maps: &Path) -> Vec<String> {
    std::fs::read_to_string(recent_file(maps))
        .map(|t| t.lines().map(str::trim).filter(|l| valid_id(l)).map(str::to_string).take(RECENT).collect())
        .unwrap_or_default()
}

fn note_recent(maps: &Path, folder: &str) {
    let mut list = recent(maps);
    list.retain(|f| f != folder);
    list.insert(0, folder.to_string());
    list.truncate(RECENT);
    let _ = crate::qol::saves::write_atomic(&recent_file(maps), (list.join("\n") + "\n").as_bytes(), false);
}

// --- command line -------------------------------------------------------------

/// `overbright-editor [--mod <folder>] [--region <code>] [--maps <dir>]
/// [--shot out.png --size WxH [--scale F] [--mode props|races] [--view 3d|map|street|whole|top] [--tab <name>]]`
#[derive(Clone, Debug, PartialEq)]
pub struct Cli {
    pub folder: Option<String>,
    pub region: Option<String>,
    /// The map mods' folder (default `<data>/maps`).
    pub maps: Option<PathBuf>,
    pub shot: Option<PathBuf>,
    pub size: (u32, u32),
    pub scale: f32,
    pub mode: Option<Mode>,
    pub view: Option<View>,
    /// The 3D view's look (`--view street|whole|top`).
    pub look: Option<String>,
    pub tab: Option<String>,
    /// Placements selected for the shot (`--select 12,40`).
    pub select: Vec<usize>,
    /// A recorded simulation to show (`--sim stream.txt[@seconds[:car]]`, the car followed).
    pub sim: Option<String>,
}

impl Default for Cli {
    fn default() -> Cli {
        Cli {
            folder: None,
            region: None,
            maps: None,
            shot: None,
            size: (1600, 900),
            scale: 1.0,
            mode: None,
            view: None,
            look: None,
            tab: None,
            select: Vec::new(),
            sim: None,
        }
    }
}

pub const USAGE: &str = "usage: overbright-editor [--mod <folder>] [--region <code>] [--maps <dir>]\n       overbright-editor --shot out.png [--size WxH] [--scale F] [--mod <folder>] [--region <code>] [--mode props|races] [--view 3d|map|street|whole|top] [--tab browser|validation|simulate|log] [--select N,M] [--sim stream.txt[@seconds[:car]]]";

impl Cli {
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Cli> {
        let mut c = Cli::default();
        let mut it = args.into_iter();
        while let Some(a) = it.next() {
            let mut val = |name: &str| it.next().with_context(|| format!("{name} needs a value\n{USAGE}"));
            match a.as_str() {
                "--mod" => {
                    let f = val("--mod")?;
                    ensure!(valid_id(&f), "--mod: `{f}` is not a map mod folder name (a-z, 0-9, - and _)");
                    c.folder = Some(f);
                }
                "--region" => {
                    let r = val("--region")?.to_ascii_uppercase();
                    ensure!(valid_region(&r), "--region: `{r}` is not a region code (as L4RA)");
                    c.region = Some(r);
                }
                "--maps" => c.maps = Some(PathBuf::from(val("--maps")?)),
                "--shot" => c.shot = Some(PathBuf::from(val("--shot")?)),
                "--size" => {
                    let s = val("--size")?;
                    let (w, h) = s.split_once(['x', 'X']).with_context(|| format!("--size: `{s}` is not WxH"))?;
                    let (w, h): (u32, u32) = (w.parse()?, h.parse()?);
                    ensure!((64..=8192).contains(&w) && (64..=8192).contains(&h), "--size: {w}x{h} outside 64..8192");
                    c.size = (w, h);
                }
                "--scale" => {
                    let s: f32 = val("--scale")?.parse()?;
                    ensure!((0.5..=4.0).contains(&s), "--scale: {s} outside 0.5..4");
                    c.scale = s;
                }
                "--mode" => {
                    c.mode = Some(match val("--mode")?.as_str() {
                        "props" => Mode::Props,
                        "races" => Mode::Races,
                        m => bail!("--mode: `{m}` (props or races)"),
                    })
                }
                "--view" => {
                    let v = val("--view")?;
                    c.view = Some(match v.as_str() {
                        "3d" => View::ThreeD,
                        "map" => View::Map,
                        "street" | "whole" | "top" => {
                            c.look = Some(v.clone());
                            View::ThreeD
                        }
                        v => bail!("--view: `{v}` (3d, map, street, whole or top)"),
                    })
                }
                "--tab" => c.tab = Some(val("--tab")?.to_ascii_lowercase()),
                "--sim" => c.sim = Some(val("--sim")?),
                "--select" => {
                    let v = val("--select")?;
                    c.select = v
                        .split(',')
                        .map(|x| x.trim().parse::<usize>())
                        .collect::<Result<_, _>>()
                        .with_context(|| format!("--select: `{v}` is not a list of placement numbers"))?;
                }
                "-h" | "--help" => bail!("{USAGE}"),
                other => bail!("unknown argument `{other}`\n{USAGE}"),
            }
        }
        ensure!(c.folder.is_some() || c.region.is_none(), "--region needs --mod");
        Ok(c)
    }
}

// --- panels -----------------------------------------------------------------

/// What `paint` hands the viewport before egui draws: the device, egui's
/// renderer (to register the world's texture) and this frame's encoder.
pub struct Gfx<'a> {
    pub gpu: &'a Gpu,
    pub egui: &'a mut egui_wgpu::Renderer,
    pub encoder: &'a mut wgpu::CommandEncoder,
}

/// The centre panel: drawn as a panel, then given the GPU to render what
/// it showed.
pub trait Viewport: Panel {
    fn render(&mut self, _s: &mut EditorState, _gfx: &mut Gfx<'_>) {}
    /// Switch the 3D view's look by name (`street`, `whole`, `top`).
    fn set_look(&mut self, _s: &mut EditorState, _look: &str) {}
    /// The camera a save keeps: the 3D one while the top-down overview
    /// shows.
    fn kept_camera(&self, s: &EditorState) -> Camera {
        s.camera
    }
}

/// The window's panels. The editor's parts register here
/// ([`Panels::standard`]); the shell owns only the layout.
pub struct Panels {
    /// The centre (3D view / map).
    pub viewport: Box<dyn Viewport>,
    /// The left column (the mod tree), top to bottom.
    pub tree: Vec<Box<dyn Panel>>,
    /// The right column (properties of the selection).
    pub properties: Vec<Box<dyn Panel>>,
    /// The bottom tabs: (key for `--tab`, title, panel).
    pub tabs: Vec<(&'static str, &'static str, Box<dyn Panel>)>,
    /// Tool buttons in the toolbar after the mode switch.
    pub tools: Vec<Box<dyn Panel>>,
    /// Called every frame after the layout (no visible place): handlers of
    /// menu commands such as Test race.
    pub hooks: Vec<Box<dyn Panel>>,
    /// Drawn over the viewport, before it handles the pointer: handles,
    /// drop targets, the keys that act in the viewport.
    pub overlays: Vec<Box<dyn Overlay>>,
}

/// Something drawn over the viewport `rect` with `painter` (a layer above
/// the viewport's image). Returns true when it takes the pointer this
/// frame (a handle under it or dragged): the viewport then does not pick
/// or start a camera drag with that click ([`pointer_taken`]).
pub trait Overlay {
    fn overlay(&mut self, s: &mut EditorState, ctx: &egui::Context, painter: &egui::Painter, rect: egui::Rect) -> bool;
}

fn pointer_taken_id() -> egui::Id {
    egui::Id::unique("editor.pointer_taken")
}

/// Whether an overlay took the pointer this frame (the viewport asks
/// before picking).
pub fn pointer_taken(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(pointer_taken_id()).unwrap_or(false))
}

/// Whether the camera flies (right button held over the viewport): the
/// single-letter keys are then the camera's.
pub fn flying(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.pointer.secondary_down())
}

impl Panels {
    /// The editor's panels. Each part adds itself here.
    pub fn standard() -> Panels {
        let props = std::rc::Rc::new(std::cell::RefCell::new(super::props::Props::default()));
        let browser = std::rc::Rc::new(std::cell::RefCell::new(super::browser::Browser::default()));
        let sim = super::sim::panel::Shared::default();
        Panels {
            viewport: Box::new(super::viewport::Viewport::new()),
            tree: vec![Box::new(parts::TreeProps)],
            properties: vec![Box::new(parts::PropsPanel(props.clone()))],
            tabs: vec![
                ("browser", "Prop browser", Box::new(parts::BrowserTab(browser.clone()))),
                ("validation", "Validation", Box::new(ValidationList)),
                (super::sim::TAB, "Simulate", Box::new(super::sim::SimTab(sim.clone()))),
            ],
            tools: vec![Box::new(parts::PropsTools(props.clone())), Box::new(super::sim::SimTool(sim.clone()))],
            hooks: vec![Box::new(super::testplay::TestPanel::default()), Box::new(super::sim::SimHook(sim.clone()))],
            overlays: vec![Box::new(parts::PropsOverlay { props, browser }), Box::new(super::sim::SimOverlay(sim))],
        }
    }
}

/// The parts' panels as the shell places them: one `Props` and one
/// `Browser` shared by their panel, toolbar and viewport overlay.
mod parts {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{EditorState, Overlay, Panel, flying};
    use crate::editor::browser::Browser;
    use crate::editor::menu::{self, Command};
    use crate::editor::props::{self, Props};
    use crate::editor::state::Mode;
    use crate::editor::tree_props;

    pub type Shared<T> = Rc<RefCell<T>>;

    /// The properties of the selection (Props mode).
    pub struct PropsPanel(pub Shared<Props>);

    impl Panel for PropsPanel {
        fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
            if s.mode == Mode::Props {
                self.0.borrow_mut().ui(s, ui);
            }
        }
    }

    /// Select / Move / Rotate, snap, drop (Props mode).
    pub struct PropsTools(pub Shared<Props>);

    impl Panel for PropsTools {
        fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
            if s.mode == Mode::Props {
                self.0.borrow_mut().toolbar(s, ui);
            }
        }
    }

    pub struct BrowserTab(pub Shared<Browser>);

    impl Panel for BrowserTab {
        fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
            self.0.borrow_mut().ui(s, ui);
        }
    }

    /// The tree's Props branch; its "frame" asks the viewport to frame the
    /// selection.
    pub struct TreeProps;

    impl Panel for TreeProps {
        fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
            if let Some(tree_props::Event::Frame) = tree_props::ui(s, ui) {
                menu::post(ui.ctx(), Command::FrameSelection);
            }
        }
    }

    /// Props mode in the viewport: the handles, a tile dropped from the
    /// browser, the Props keys (not while the camera flies) and the Edit
    /// menu's prop commands.
    pub struct PropsOverlay {
        pub props: Shared<Props>,
        pub browser: Shared<Browser>,
    }

    impl Overlay for PropsOverlay {
        fn overlay(
            &mut self,
            s: &mut EditorState,
            ctx: &egui::Context,
            painter: &egui::Painter,
            rect: egui::Rect,
        ) -> bool {
            let mut taken = self.browser.borrow_mut().drop_target(s, ctx, painter, rect);
            if s.mode != Mode::Props {
                return taken;
            }
            let mut p = self.props.borrow_mut();
            taken |= p.handles(s, ctx, painter, rect);
            let cursor = ctx
                .input(|i| i.pointer.hover_pos())
                .filter(|at| rect.contains(*at))
                .and_then(|at| props::ground_under(s, rect, at));
            if !flying(ctx) {
                p.keys(s, ctx, cursor);
            }
            if menu::take(ctx, Command::Delete) {
                props::delete_selection(s);
            }
            if menu::take(ctx, Command::Duplicate) {
                props::duplicate_selection(s, cursor);
            }
            if menu::take(ctx, Command::DropToGround) {
                props::drop_selection(s);
            }
            if menu::take(ctx, Command::SelectAllOfSolid)
                && let Some(solid) = s.selection.first().and_then(|&i| s.world.placements.get(i)).map(|pl| pl.solid)
            {
                let hidden = crate::world::HIDDEN_FLAG;
                s.selection = (0..s.world.placements.len())
                    .filter(|&i| s.world.placements[i].solid == solid && s.world.placements[i].flags & hidden == 0)
                    .collect();
                s.status = format!("Selected {} of {}", s.selection.len(), props::name(&s.world, s.selection[0]));
            }
            taken
        }
    }
}

/// The validation entries as a plain list (until the races part replaces
/// it).
struct ValidationList;

impl Panel for ValidationList {
    fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
        if s.issues.is_empty() {
            ui.weak("No problems found.");
        }
        for i in &s.issues {
            let (c, tag) = match i.level {
                Level::Error => (theme::error(), "Error"),
                Level::Warning => (theme::warn(), "Warning"),
                Level::Info => (theme::dim(), "Info"),
            };
            ui.horizontal(|ui| {
                ui.colored_label(c, tag);
                ui.label(&i.text);
            });
        }
    }
}

// --- the app ------------------------------------------------------------------

/// A dialog over the window.
#[derive(Clone, Debug)]
pub enum Dialog {
    NewMod {
        folder: String,
        name: String,
        region: String,
    },
    OpenMod {
        list: Vec<String>,
        pick: Option<String>,
    },
    Details {
        name: String,
        author: String,
        description: String,
    },
    Export {
        path: String,
    },
    Import {
        path: String,
    },
    /// An autosave newer than the saved mod.
    Restore {
        map: MapMod,
        age: Duration,
    },
    /// Unsaved changes before `then`.
    Unsaved {
        then: Then,
    },
    Message {
        title: String,
        text: String,
    },
    Controls,
}

/// What waits on the unsaved-changes question.
#[derive(Clone, Debug, PartialEq)]
pub enum Then {
    Quit,
    Open(String),
    New,
    OpenList,
}

/// Opens a region's world (the window and tests pass their own).
pub type Loader = Box<dyn FnMut(&Path, &str) -> Result<World>>;

pub struct App {
    pub ctx: egui::Context,
    pub state: Option<EditorState>,
    pub panels: Panels,
    pub root: PathBuf,
    pub maps: PathBuf,
    pub loader: Loader,
    pub dialog: Option<Dialog>,
    /// The bottom tab shown (its key).
    pub tab: &'static str,
    /// Every status message, oldest first (the Log tab).
    pub log: Vec<String>,
    last_status: String,
    /// When the last autosave was written (or the mod opened / saved).
    last_autosave: Instant,
    /// The mod as last autosaved or saved.
    autosaved: Option<MapMod>,
    pub autosave_note: String,
    /// The window should close.
    pub quit: bool,
    /// Last frame's time on the CPU (status bar).
    pub frame_ms: f32,
}

impl App {
    pub fn new(root: PathBuf, maps: PathBuf, loader: Loader) -> App {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        App {
            ctx,
            state: None,
            panels: Panels::standard(),
            root,
            maps,
            loader,
            dialog: None,
            tab: "browser",
            log: Vec::new(),
            last_status: String::new(),
            last_autosave: Instant::now(),
            autosaved: None,
            autosave_note: String::new(),
            quit: false,
            frame_ms: 0.0,
        }
    }

    /// The window title.
    pub fn title(&self) -> String {
        match &self.state {
            Some(s) => {
                let name = if s.map.name.is_empty() { s.folder.as_str() } else { s.map.name.as_str() };
                format!("Overbright Map Editor (preview) — {name} · {}{}", s.region, if s.dirty { " ●" } else { "" })
            }
            None => "Overbright Map Editor (preview)".into(),
        }
    }

    fn say(&mut self, text: impl Into<String>) {
        let text = text.into();
        log::info!("editor: {text}");
        if let Some(s) = self.state.as_mut() {
            s.status = text.clone();
        }
        self.last_status = text.clone();
        self.log.push(text);
    }

    fn fail(&mut self, title: &str, e: anyhow::Error) {
        log::warn!("editor: {title}: {e:#}");
        self.log.push(format!("{title}: {e:#}"));
        self.dialog = Some(Dialog::Message { title: title.into(), text: format!("{e:#}") });
    }

    /// Open the mod in `folder` (on `region`, else its own): the world
    /// loaded, the mod applied, and the restore prompt when an autosave
    /// is newer.
    pub fn open(&mut self, folder: &str, region: Option<&str>) -> Result<()> {
        let file = self.maps.join(folder).join(store::FILE);
        let saved = if file.exists() { Some(store::read_file(&file)?) } else { None };
        let pending = pending_autosave(&self.maps, folder, saved.as_ref());
        ensure!(saved.is_some() || pending.is_some(), "no map mod `{folder}` in {}", self.maps.display());
        let map = saved.clone().unwrap_or_default();
        let view = ModView::read(&self.maps, folder);
        let region = mod_region(region, &view, &map);
        let world = (self.loader)(&self.root, &region).with_context(|| format!("loading region {region}"))?;
        let camera = match view.camera {
            Some((p, yaw, pitch)) => Camera::new(p, yaw, pitch),
            None => start_camera(&world, &map),
        };
        let mut s = EditorState::new(self.root.clone(), &region, self.maps.clone(), folder, map.clone(), world, camera);
        s.status = format!("Opened {folder}");
        self.state = Some(s);
        self.autosaved = Some(map);
        self.last_autosave = Instant::now();
        self.autosave_note.clear();
        note_recent(&self.maps, folder);
        self.say(format!("Opened {folder} on {region}"));
        if saved.is_none() {
            // Only the autosave is left: take it as it is.
            if let Some((m, _)) = pending {
                self.restore(m);
            }
        } else if let Some((m, at)) = pending {
            self.dialog =
                Some(Dialog::Restore { map: m, age: SystemTime::now().duration_since(at).unwrap_or_default() });
        }
        Ok(())
    }

    /// Make a new mod `folder` (named `name`) on `region` and open it.
    pub fn new_mod(&mut self, folder: &str, name: &str, region: &str) -> Result<()> {
        ensure!(valid_id(folder), "`{folder}` is not a folder name: use a-z, 0-9, - and _");
        let region = region.to_ascii_uppercase();
        ensure!(valid_region(&region), "`{region}` is not a region code (as L4RA)");
        ensure!(!self.maps.join(folder).join(store::FILE).exists(), "a map mod `{folder}` exists already");
        let m = MapMod { name: name.trim().to_string(), ..MapMod::default() };
        save_mod(&self.maps, folder, &m)?;
        ModView { region: Some(region.clone()), camera: None }.write(&self.maps, folder)?;
        self.open(folder, Some(&region))
    }

    /// Save the open mod (atomic); the autosave goes.
    pub fn save(&mut self) -> Result<()> {
        let Some(s) = self.state.as_mut() else { return Ok(()) };
        save_mod(&s.maps, &s.folder, &s.map)?;
        let c = self.panels.viewport.kept_camera(s);
        let view = ModView { region: Some(s.region.clone()), camera: Some((c.position, c.yaw, c.pitch)) };
        if let Err(e) = view.write(&s.maps, &s.folder) {
            log::warn!("editor: {EDITOR_TOML} not written: {e:#}");
        }
        remove_autosave(&s.maps, &s.folder);
        s.dirty = false;
        self.autosaved = Some(s.map.clone());
        self.last_autosave = Instant::now();
        self.autosave_note.clear();
        let f = s.folder.clone();
        self.say(format!("Saved {f}"));
        Ok(())
    }

    /// Take the autosave's mod as one undoable step.
    pub fn restore(&mut self, m: MapMod) {
        if let Some(s) = self.state.as_mut() {
            s.map = m;
            undo::apply_map(s);
            super::commit(s, "Restore unsaved changes");
            s.dirty = true;
        }
        self.say("Restored unsaved changes");
    }

    /// Autosave when due: every [`AUTOSAVE_EVERY`] while the mod differs
    /// from the last save or autosave.
    pub fn tick(&mut self, now: Instant) {
        let Some(s) = self.state.as_ref() else { return };
        if now.duration_since(self.last_autosave) < AUTOSAVE_EVERY {
            return;
        }
        self.last_autosave = now;
        if !s.dirty || self.autosaved.as_ref() == Some(&s.map) {
            return;
        }
        match write_autosave(&s.maps, &s.folder, &s.map) {
            Ok(()) => {
                self.autosaved = Some(s.map.clone());
                self.autosave_note = "Autosaved".into();
            }
            Err(e) => {
                self.autosave_note = "Autosave failed".into();
                log::warn!("editor: autosave: {e:#}");
            }
        }
    }

    /// When [`App::tick`] next has work (for the window's wait).
    pub fn next_tick(&self) -> Instant {
        self.last_autosave + AUTOSAVE_EVERY
    }

    /// The window was asked to close: true when it may.
    pub fn close_requested(&mut self) -> bool {
        if self.state.as_ref().is_some_and(|s| s.dirty) {
            self.dialog = Some(Dialog::Unsaved { then: Then::Quit });
            false
        } else {
            self.quit = true;
            true
        }
    }

    fn after_unsaved(&mut self, then: Then) {
        match then {
            Then::Quit => self.quit = true,
            Then::Open(f) => {
                if let Err(e) = self.open(&f, None) {
                    self.fail("Could not open the mod", e);
                }
            }
            Then::New => self.dialog = Some(new_dialog()),
            Then::OpenList => self.dialog = Some(Dialog::OpenMod { list: store::list(&self.maps), pick: None }),
        }
    }

    /// Ask about unsaved changes first, then do `then`.
    fn guarded(&mut self, then: Then) {
        if self.state.as_ref().is_some_and(|s| s.dirty) {
            self.dialog = Some(Dialog::Unsaved { then });
        } else {
            self.after_unsaved(then);
        }
    }

    /// Run a menu / toolbar / key action.
    pub fn act(&mut self, a: menu::Action) {
        use menu::Action as A;
        match a {
            A::NewMod => self.guarded(Then::New),
            A::OpenMod => self.guarded(Then::OpenList),
            A::OpenRecent(f) => self.guarded(Then::Open(f)),
            A::Save => {
                if let Err(e) = self.save() {
                    self.fail("Could not save", e);
                }
            }
            A::Details => {
                if let Some(s) = &self.state {
                    self.dialog = Some(Dialog::Details {
                        name: s.map.name.clone(),
                        author: s.map.author.clone(),
                        description: s.map.description.clone(),
                    });
                }
            }
            A::Export => {
                if let Some(s) = &self.state {
                    let home = crate::launcher::dirs::home().unwrap_or_default();
                    self.dialog =
                        Some(Dialog::Export { path: home.join(format!("{}.zip", s.folder)).display().to_string() });
                }
            }
            A::Import => self.dialog = Some(Dialog::Import { path: String::new() }),
            A::Quit => {
                self.close_requested();
            }
            A::Undo => {
                if let Some(s) = self.state.as_mut()
                    && undo::undo(s)
                {
                    let t = s.status.clone();
                    self.say(t);
                }
            }
            A::Redo => {
                if let Some(s) = self.state.as_mut()
                    && undo::redo(s)
                {
                    let t = s.status.clone();
                    self.say(t);
                }
            }
            A::Mode(m) => {
                if let Some(s) = self.state.as_mut() {
                    s.mode = m;
                }
            }
            A::View(v) => {
                if let Some(s) = self.state.as_mut() {
                    s.view = v;
                }
            }
            A::Tab(t) => self.tab = t,
            A::Controls => self.dialog = Some(Dialog::Controls),
        }
    }

    /// Lay out one frame.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let started = Instant::now();
        let mut actions = Vec::new();
        menu::shortcuts(self, ui.ctx(), &mut actions);
        egui::Panel::top("menu").show(ui, |ui| menu::bar(self, ui, &mut actions));
        if self.state.is_some() {
            egui::Panel::top("toolbar").show(ui, |ui| menu::toolbar(self, ui, &mut actions));
        }
        egui::Panel::bottom("status").exact_size(26.0).show(ui, |ui| menu::status(self, ui));
        if let Some(s) = self.state.as_mut() {
            let p = &mut self.panels;
            egui::Panel::left("tree").resizable(true).default_size(270.0).min_size(180.0).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(theme::title("Map mod"));
                egui::ScrollArea::vertical().id_salt("tree").show(ui, |ui| {
                    for t in p.tree.iter_mut() {
                        t.ui(s, ui);
                    }
                    if p.tree.is_empty() {
                        ui.weak(format!("{} prop edits, {} races", s.map.props.len(), s.map.races.len()));
                    }
                });
            });
            egui::Panel::right("properties").resizable(true).default_size(300.0).min_size(200.0).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(theme::title("Properties"));
                egui::ScrollArea::vertical().id_salt("properties").show(ui, |ui| {
                    for t in p.properties.iter_mut() {
                        t.ui(s, ui);
                    }
                    if p.properties.is_empty() {
                        ui.weak("Nothing selected.");
                    }
                });
            });
            if let Some(k) = super::sim::panel::take_tab_request(ui.ctx()) {
                self.tab = k;
            }
            let tab = &mut self.tab;
            let log = &self.log;
            egui::Panel::bottom("tabs").resizable(true).default_size(190.0).min_size(80.0).show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (key, title, _) in p.tabs.iter() {
                        let label = egui::RichText::new(*title).font(theme::font(theme::LABEL, 15.0));
                        let label = if *tab == *key { label.color(theme::lime()) } else { label };
                        if ui.selectable_label(*tab == *key, label).clicked() {
                            actions.push(menu::Action::Tab(key));
                        }
                    }
                    let label = egui::RichText::new("Log").font(theme::font(theme::LABEL, 15.0));
                    let label = if *tab == "log" { label.color(theme::lime()) } else { label };
                    if ui.selectable_label(*tab == "log", label).clicked() {
                        actions.push(menu::Action::Tab("log"));
                    }
                });
                ui.separator();
                egui::ScrollArea::both()
                    .id_salt("tab")
                    .auto_shrink([false, false])
                    .stick_to_bottom(*tab == "log")
                    .show(ui, |ui| {
                        if *tab == "log" {
                            for l in log {
                                ui.label(l);
                            }
                        } else if let Some((_, _, panel)) = p.tabs.iter_mut().find(|t| t.0 == *tab) {
                            panel.ui(s, ui);
                        }
                    });
            });
            egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme::rgb(theme::tokens().page))).show(
                ui,
                |ui| {
                    // The overlays first (their own layer above the
                    // viewport's image), so the viewport knows whether they
                    // took the pointer.
                    let rect = ui.max_rect();
                    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::unique("editor.viewport.overlay"));
                    let painter = ui.ctx().layer_painter(layer).with_clip_rect(rect);
                    let mut taken = false;
                    for o in p.overlays.iter_mut() {
                        taken |= o.overlay(s, ui.ctx(), &painter, rect);
                    }
                    ui.ctx().data_mut(|d| d.insert_temp(pointer_taken_id(), taken));
                    p.viewport.ui(s, ui);
                    for h in p.hooks.iter_mut() {
                        h.ui(s, ui);
                    }
                },
            );
            if s.status != self.last_status {
                let t = s.status.clone();
                self.say(t);
            }
        } else {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(theme::rgb(theme::tokens().page)).inner_margin(40.0))
                .show(ui, |ui| self.welcome(ui, &mut actions));
        }
        self.dialogs(ui.ctx());
        for a in actions {
            self.act(a);
        }
        menu::clear_commands(ui.ctx());
        self.frame_ms = started.elapsed().as_secs_f32() * 1000.0;
    }

    fn welcome(&mut self, ui: &mut egui::Ui, actions: &mut Vec<menu::Action>) {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Overbright Map Editor").font(theme::font(theme::TITLE, 30.0)).color(theme::lime()));
            ui.add_space(6.0);
            ui.label("Move, add and remove props and build your own races on the game's roads. Map mods never change the game's files.");
            ui.add_space(4.0);
            ui.colored_label(
                theme::warn(),
                "Preview: the editor works but is not finished. Keep a copy of map mods you care about.",
            );
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                if ui.add(theme::main_action("New map mod…")).clicked() {
                    actions.push(menu::Action::NewMod);
                }
                if ui.button("Open map mod…").clicked() {
                    actions.push(menu::Action::OpenMod);
                }
                if ui.button("Import…").clicked() {
                    actions.push(menu::Action::Import);
                }
            });
            let recent = recent(&self.maps);
            if !recent.is_empty() {
                ui.add_space(22.0);
                ui.label(theme::title("Recent"));
                for f in recent {
                    if ui.link(&f).clicked() {
                        actions.push(menu::Action::OpenRecent(f));
                    }
                }
            }
            ui.add_space(22.0);
            ui.weak(format!("Map mods folder: {}", self.maps.display()));
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.dialog.take() else { return };
        let mut keep = true;
        let mut then: Option<Box<dyn FnOnce(&mut App)>> = None;
        let modal = egui::Modal::new(egui::Id::unique("editor.dialog")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            match &mut d {
                Dialog::NewMod { folder, name, region } => {
                    ui.label(theme::title("New map mod"));
                    egui::Grid::new("new").num_columns(2).show(ui, |ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(name);
                        ui.end_row();
                        ui.label("Folder");
                        ui.text_edit_singleline(folder);
                        ui.end_row();
                        ui.label("Region");
                        ui.text_edit_singleline(region);
                        ui.end_row();
                    });
                    if folder.is_empty() {
                        ui.weak(format!("Folder: {}", folder_from(name)));
                    }
                    ui.horizontal(|ui| {
                        if ui.add(theme::main_action("Create")).clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            let f = if folder.is_empty() { folder_from(name) } else { folder.clone() };
                            let (n, r) = (name.clone(), region.clone());
                            then = Some(Box::new(move |app: &mut App| {
                                if let Err(e) = app.new_mod(&f, &n, &r) {
                                    app.fail("Could not make the mod", e);
                                }
                            }));
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::OpenMod { list, pick } => {
                    ui.label(theme::title("Open map mod"));
                    if list.is_empty() {
                        ui.weak("No map mods yet.");
                    }
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        for f in list.iter() {
                            let r = ui.selectable_label(pick.as_deref() == Some(f.as_str()), f);
                            if r.clicked() {
                                *pick = Some(f.clone());
                            }
                            if r.double_clicked() {
                                *pick = Some(f.clone());
                                keep = false;
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.add_enabled(pick.is_some(), theme::main_action("Open")).clicked() {
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() {
                            *pick = None;
                            keep = false;
                        }
                    });
                    if !keep && let Some(f) = pick.clone() {
                        then = Some(Box::new(move |app: &mut App| {
                            if let Err(e) = app.open(&f, None) {
                                app.fail("Could not open the mod", e);
                            }
                        }));
                    }
                }
                Dialog::Details { name, author, description } => {
                    ui.label(theme::title("Mod details"));
                    egui::Grid::new("details").num_columns(2).show(ui, |ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(name);
                        ui.end_row();
                        ui.label("Author");
                        ui.text_edit_singleline(author);
                        ui.end_row();
                        ui.label("Description");
                        ui.text_edit_multiline(description);
                        ui.end_row();
                    });
                    ui.horizontal(|ui| {
                        if ui.add(theme::main_action("OK")).clicked() {
                            let (n, a, de) = (name.clone(), author.clone(), description.clone());
                            then = Some(Box::new(move |app: &mut App| {
                                if let Some(s) = app.state.as_mut() {
                                    s.map.name = n;
                                    s.map.author = a;
                                    s.map.description = de;
                                    super::commit(s, "Mod details");
                                }
                            }));
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Export { path } => {
                    ui.label(theme::title("Export map mod"));
                    ui.label("A .zip file, or a folder:");
                    ui.text_edit_singleline(path);
                    ui.horizontal(|ui| {
                        if ui.add(theme::main_action("Export")).clicked() {
                            let p = PathBuf::from(path.trim());
                            then = Some(Box::new(move |app: &mut App| app.export(&p)));
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Import { path } => {
                    ui.label(theme::title("Import map mod"));
                    ui.label("A .zip file or a folder holding map.obmap:");
                    ui.text_edit_singleline(path);
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!path.trim().is_empty(), theme::main_action("Import")).clicked() {
                            let p = PathBuf::from(path.trim());
                            then = Some(Box::new(move |app: &mut App| app.import(&p)));
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Restore { map, age } => {
                    ui.label(theme::title("Restore unsaved changes?"));
                    ui.label(format!("This mod has changes from {} that were not saved.", ago(*age)));
                    ui.horizontal(|ui| {
                        if ui.add(theme::main_action("Restore")).clicked() {
                            let m = map.clone();
                            then = Some(Box::new(move |app: &mut App| app.restore(m)));
                            keep = false;
                        }
                        if ui.button("Discard them").clicked() {
                            then = Some(Box::new(|app: &mut App| {
                                if let Some(s) = &app.state {
                                    remove_autosave(&s.maps, &s.folder);
                                }
                                app.say("Discarded the unsaved changes");
                            }));
                            keep = false;
                        }
                    });
                }
                Dialog::Unsaved { then: next } => {
                    ui.label(theme::title("Save changes?"));
                    ui.label("The mod has changes that are not saved.");
                    ui.horizontal(|ui| {
                        if ui.add(theme::main_action("Save")).clicked() {
                            let n = next.clone();
                            then = Some(Box::new(move |app: &mut App| match app.save() {
                                Ok(()) => app.after_unsaved(n),
                                Err(e) => app.fail("Could not save", e),
                            }));
                            keep = false;
                        }
                        if ui.button("Don't save").clicked() {
                            let n = next.clone();
                            then = Some(Box::new(move |app: &mut App| {
                                if let Some(s) = app.state.as_mut() {
                                    s.dirty = false;
                                    remove_autosave(&s.maps, &s.folder);
                                }
                                app.after_unsaved(n);
                            }));
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                }
                Dialog::Message { title, text } => {
                    ui.label(
                        egui::RichText::new(title.as_str()).font(theme::font(theme::TITLE, 17.0)).color(theme::error()),
                    );
                    ui.label(text.as_str());
                    if ui.button("OK").clicked() {
                        keep = false;
                    }
                }
                Dialog::Controls => {
                    ui.label(theme::title("Controls"));
                    egui::Grid::new("controls").num_columns(2).striped(true).show(ui, |ui| {
                        for (k, v) in menu::CONTROLS {
                            ui.label(egui::RichText::new(*k).font(theme::font(theme::SEMI, 14.0)));
                            ui.label(*v);
                            ui.end_row();
                        }
                    });
                    if ui.button("Close").clicked() {
                        keep = false;
                    }
                }
            }
        });
        if modal.should_close() && !matches!(d, Dialog::Restore { .. } | Dialog::Unsaved { .. }) {
            keep = false;
        }
        if keep {
            self.dialog = Some(d);
        }
        if let Some(f) = then {
            f(self);
        }
    }

    fn export(&mut self, to: &Path) {
        let Some(folder) = self.state.as_ref().map(|s| s.folder.clone()) else { return };
        let r = (|| -> Result<()> {
            if self.state.as_ref().is_some_and(|s| s.dirty) {
                self.save()?;
            }
            store::export(&self.maps, &folder, to)
        })();
        match r {
            Ok(()) => self.say(format!("Exported {folder} to {}", to.display())),
            Err(e) => self.fail("Could not export", e),
        }
    }

    fn import(&mut self, from: &Path) {
        match store::import(&self.maps, from) {
            Ok(f) => {
                self.say(format!("Imported {f}"));
                self.guarded(Then::Open(f));
            }
            Err(e) => self.fail("Could not import", e),
        }
    }

    // --- painting ---

    /// Run one frame of the UI.
    pub fn frame(&mut self, raw: egui::RawInput) -> egui::FullOutput {
        let ctx = self.ctx.clone();
        ctx.run_ui(raw, |ui| self.ui(ui))
    }

    /// Paint `full` into `target` (`size` pixels): textures uploaded, the
    /// viewport rendered, egui drawn over a cleared page. Returns the
    /// command buffers to submit, in order.
    pub fn paint(
        &mut self,
        gpu: &Gpu,
        egui_r: &mut egui_wgpu::Renderer,
        target: &wgpu::TextureView,
        size: (u32, u32),
        full: egui::FullOutput,
    ) -> Vec<wgpu::CommandBuffer> {
        for (id, deltas) in &full.textures_delta.set {
            for delta in deltas {
                egui_r.update_texture(&gpu.device, &gpu.queue, *id, delta);
            }
        }
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("editor") });
        if let Some(s) = self.state.as_mut() {
            let mut gfx = Gfx { gpu, egui: egui_r, encoder: &mut encoder };
            self.panels.viewport.render(s, &mut gfx);
        }
        let jobs = self.ctx.tessellate(full.shapes, full.pixels_per_point);
        let screen =
            egui_wgpu::ScreenDescriptor { size_in_pixels: [size.0, size.1], pixels_per_point: full.pixels_per_point };
        let mut out = egui_r.update_buffers(&gpu.device, &gpu.queue, &mut encoder, &jobs, &screen);
        {
            let page = theme::tokens().page;
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: f64::from(page.0) / 255.0,
                                g: f64::from(page.1) / 255.0,
                                b: f64::from(page.2) / 255.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            egui_r.render(&mut pass, &jobs, &screen);
        }
        out.push(encoder.finish());
        for id in &full.textures_delta.free {
            egui_r.free_texture(id);
        }
        out
    }
}

/// A folder name from a mod's name ("Casino sprints" → casino-sprints).
pub fn folder_from(name: &str) -> String {
    let mut f = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            f.push(c.to_ascii_lowercase());
        } else if (c == ' ' || c == '-' || c == '_') && !f.ends_with('-') && !f.is_empty() {
            f.push('-');
        }
    }
    let f = f.trim_end_matches('-').chars().take(40).collect::<String>();
    if f.is_empty() { "my-map".into() } else { f }
}

fn new_dialog() -> Dialog {
    Dialog::NewMod { folder: String::new(), name: String::new(), region: DEFAULT_REGION.into() }
}

/// "3 minutes ago".
pub fn ago(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        0..=59 => "less than a minute ago".into(),
        60..=119 => "a minute ago".into(),
        120..=3599 => format!("{} minutes ago", s / 60),
        3600..=7199 => "an hour ago".into(),
        7200..=86_399 => format!("{} hours ago", s / 3600),
        _ => format!("{} days ago", s / 86_400),
    }
}

/// Where a mod opens without a kept camera: above its first race's start,
/// else above the region's middle.
pub fn start_camera(world: &World, m: &MapMod) -> Camera {
    let start = m.races.iter().find_map(|r| r.start.or_else(|| r.grid.first().copied()));
    if let Some(sp) = start {
        let back = Vec3::new(-sp.heading.sin(), -sp.heading.cos(), 0.0);
        let eye = sp.position + back * 40.0 + Vec3::Z * 25.0;
        return Camera::looking_at(eye, sp.position);
    }
    if world.placements.is_empty() {
        return Camera::new(Vec3::new(0.0, -100.0, 80.0), 0.0, -0.5);
    }
    crate::render::overview_camera(world)
}

// --- off-screen shot ------------------------------------------------------------

/// Render the whole window off-screen into a PNG (`--shot`): a few frames
/// so the layout settles, then the last one read back. No window.
pub fn shot(app: &mut App, gpu: &Gpu, size: (u32, u32), scale: f32, out: &Path) -> Result<()> {
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut egui_r = egui_wgpu::Renderer::new(
        &gpu.device,
        format,
        egui_wgpu::RendererOptions {
            msaa_samples: 1,
            depth_stencil_format: None,
            dithering: false,
            predictable_texture_filtering: true,
        },
    );
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("editor shot"),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size.0 as f32 / scale, size.1 as f32 / scale));
    for frame in 0..4 {
        let mut raw = egui::RawInput {
            screen_rect: Some(rect),
            time: Some(frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            focused: true,
            ..Default::default()
        };
        raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(scale);
        let full = app.frame(raw);
        let bufs = app.paint(gpu, &mut egui_r, &view, size, full);
        gpu.queue.submit(bufs);
    }
    let bytes_per_row = (size.0 * 4).div_ceil(256) * 256;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("editor shot readback"),
        size: u64::from(bytes_per_row * size.1),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: None },
        },
        wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None })?;
    let mapped = slice.get_mapped_range()?;
    let mut pixels = Vec::with_capacity((size.0 * size.1 * 4) as usize);
    for row in mapped.chunks(bytes_per_row as usize).take(size.1 as usize) {
        pixels.extend_from_slice(&row[..(size.0 * 4) as usize]);
    }
    if let Some(d) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d)?;
    }
    image::save_buffer(out, &pixels, size.0, size.1, image::ColorType::Rgba8)?;
    Ok(())
}

/// Apply the shot's `--mode`, `--view` and `--tab`.
pub fn apply_cli_view(app: &mut App, cli: &Cli) {
    if let Some(s) = app.state.as_mut() {
        if let Some(m) = cli.mode {
            s.mode = m;
        }
        if let Some(v) = cli.view {
            s.view = v;
        }
        if let Some(look) = &cli.look {
            app.panels.viewport.set_look(s, look);
        }
        if !cli.select.is_empty() {
            s.selection = cli.select.iter().copied().filter(|&i| i < s.world.placements.len()).collect();
        }
    }
    if let Some(f) = &cli.sim {
        super::sim::panel::load_recording(&app.ctx, f);
    }
    if let Some(t) = &cli.tab {
        if t == "log" {
            app.tab = "log";
        } else if let Some(k) = app.panels.tabs.iter().find(|x| x.0 == t.as_str()).map(|x| x.0) {
            app.tab = k;
        } else {
            log::warn!("editor: no tab `{t}`");
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::{Mat3, Vec2};

    use super::*;
    use crate::testkit::Scratch;
    use crate::tools::map::{CustomRace, PropEdit, RaceKind};

    fn app(s: &Scratch) -> App {
        App::new(s.path().join("game"), s.path().join("maps"), Box::new(|_, _| Ok(World::default())))
    }

    fn add(solid: &str, x: f32) -> PropEdit {
        PropEdit::Add {
            region: "L4RA".into(),
            solid: solid.into(),
            position: Vec3::new(x, 0.0, 0.0),
            rotation: Mat3::IDENTITY,
        }
    }

    #[test]
    fn cli_flags() {
        let p = |a: &[&str]| Cli::parse(a.iter().map(|s| s.to_string()));
        assert_eq!(p(&[]).unwrap(), Cli::default());
        let c = p(&[
            "--mod",
            "docks",
            "--region",
            "l4ra",
            "--shot",
            "o.png",
            "--size",
            "1280x720",
            "--mode",
            "races",
            "--view",
            "map",
            "--tab",
            "Validation",
            "--scale",
            "1.5",
            "--select",
            "3,7",
        ])
        .unwrap();
        assert_eq!(c.folder.as_deref(), Some("docks"));
        assert_eq!(c.region.as_deref(), Some("L4RA"));
        assert_eq!(c.select, vec![3, 7]);
        assert_eq!(c.shot, Some(PathBuf::from("o.png")));
        assert_eq!(
            (c.size, c.scale, c.mode, c.view, c.tab.as_deref()),
            ((1280, 720), 1.5, Some(Mode::Races), Some(View::Map), Some("validation"))
        );
        assert_eq!(c.look, None);
        for look in ["street", "whole", "top"] {
            let c = p(&["--view", look]).unwrap();
            assert_eq!((c.view, c.look.as_deref()), (Some(View::ThreeD), Some(look)));
        }
        for bad in [
            &["--mod"][..],
            &["--mod", "../up"],
            &["--region", "L4RA"],
            &["--mod", "a", "--region", "nope!"],
            &["--size", "12x9"],
            &["--size", "big"],
            &["--mode", "drift"],
            &["--view", "side"],
            &["--select", "1,x"],
            &["--frobnicate"],
        ] {
            assert!(p(bad).is_err(), "{bad:?} accepted");
        }
    }

    #[test]
    fn save_is_atomic_and_removes_the_autosave() {
        let s = Scratch::new("editor-save");
        let maps = s.path().join("maps");
        let m = MapMod { name: "Docks".into(), props: vec![add("XC_CONE_01", 1.0)], ..Default::default() };
        let path = save_mod(&maps, "docks", &m).unwrap();
        assert_eq!(store::read_file(&path).unwrap(), m);
        assert!(save_mod(&maps, "../up", &m).is_err());
        let left: Vec<_> = std::fs::read_dir(maps.join("docks")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left, vec![std::ffi::OsString::from(store::FILE)], "no temporary file left");

        let mut a = app(&s);
        a.open("docks", None).unwrap();
        assert!(a.dialog.is_none());
        a.state.as_mut().unwrap().map.name = "Docks 2".into();
        crate::editor::commit(a.state.as_mut().unwrap(), "Rename");
        write_autosave(&maps, "docks", &a.state.as_ref().unwrap().map).unwrap();
        a.save().unwrap();
        assert!(!autosave_path(&maps, "docks").exists());
        assert!(!a.state.as_ref().unwrap().dirty);
        assert_eq!(store::read_file(&path).unwrap().name, "Docks 2");
        assert_eq!(ModView::read(&maps, "docks").region.as_deref(), Some("L4RA"));
        assert_eq!(recent(&maps), vec!["docks".to_string()]);
    }

    #[test]
    fn autosave_after_a_minute_and_restore_prompt() {
        let s = Scratch::new("editor-autosave");
        let maps = s.path().join("maps");
        let mut a = app(&s);
        a.new_mod("yard", "Yard", "l4ra").unwrap();
        assert!(a.new_mod("yard", "Yard", "L4RA").is_err(), "an existing mod is never overwritten");
        let st = a.state.as_mut().unwrap();
        assert_eq!(st.region, "L4RA");
        st.map.props.push(add("XC_CONE_01", 2.0));
        crate::editor::commit(st, "Add XC_CONE_01");
        let t0 = Instant::now();
        a.tick(t0);
        assert!(!autosave_path(&maps, "yard").exists(), "not before a minute");
        a.tick(t0 + AUTOSAVE_EVERY + Duration::from_secs(1));
        assert!(autosave_path(&maps, "yard").exists());
        let unsaved = a.state.as_ref().unwrap().map.clone();

        // A crash: the next start offers the autosave.
        let mut b = app(&s);
        b.open("yard", None).unwrap();
        let Some(Dialog::Restore { map, .. }) = b.dialog.clone() else { panic!("no restore prompt: {:?}", b.dialog) };
        assert_eq!(map, unsaved);
        assert!(b.state.as_ref().unwrap().map.props.is_empty());
        b.restore(map);
        let st = b.state.as_mut().unwrap();
        assert_eq!(st.map, unsaved);
        assert!(st.dirty);
        assert!(crate::editor::undo::undo(st), "the restore is one undo step");
        assert!(st.map.props.is_empty());

        // A saved mod newer than its autosave: no prompt.
        save_mod(&maps, "yard", &unsaved).unwrap();
        let mut c = app(&s);
        c.open("yard", None).unwrap();
        assert!(c.dialog.is_none());
    }

    #[test]
    fn undo_redo_through_the_app() {
        let s = Scratch::new("editor-undo");
        let mut a = app(&s);
        a.new_mod("u", "", "L4RA").unwrap();
        let st = a.state.as_mut().unwrap();
        let mut r = CustomRace::new("sprint", 1, RaceKind::Sprint);
        r.checkpoints = vec![Vec2::ZERO, Vec2::X * 100.0];
        st.map.races.push(r);
        crate::editor::commit(st, "New race");
        st.race = Some(0);
        assert_eq!(st.undo.undo_label(), Some("New race"));
        a.act(menu::Action::Undo);
        let st = a.state.as_ref().unwrap();
        assert!(st.map.races.is_empty() && st.race.is_none());
        assert_eq!(st.undo.redo_label(), Some("New race"));
        a.act(menu::Action::Redo);
        assert_eq!(a.state.as_ref().unwrap().map.races.len(), 1);
        assert!(a.log.iter().any(|l| l.contains("Undone: New race")));
    }

    #[test]
    fn quitting_with_changes_asks_first() {
        let s = Scratch::new("editor-quit");
        let mut a = app(&s);
        assert!(a.close_requested());
        let mut a = app(&s);
        a.new_mod("q", "", "L4RA").unwrap();
        let st = a.state.as_mut().unwrap();
        st.map.description = "x".into();
        crate::editor::commit(st, "Mod details");
        assert!(!a.close_requested());
        assert!(matches!(a.dialog, Some(Dialog::Unsaved { then: Then::Quit })));
        assert!(!a.quit);
    }

    #[test]
    fn helpers() {
        assert_eq!(folder_from("Casino Sprints!"), "casino-sprints");
        assert_eq!(folder_from("  "), "my-map");
        assert_eq!(ago(Duration::from_secs(10)), "less than a minute ago");
        assert_eq!(ago(Duration::from_secs(600)), "10 minutes ago");
        let s = Scratch::new("editor-view");
        let maps = s.path().join("maps");
        let v = ModView { region: Some("L4RA".into()), camera: Some((Vec3::new(1.5, -2.0, 30.25), 0.5, -0.25)) };
        v.write(&maps, "v").unwrap();
        assert_eq!(ModView::read(&maps, "v"), v);
        let m = MapMod {
            props: vec![PropEdit::Add {
                region: "l4rb".into(),
                solid: "X".into(),
                position: Vec3::ZERO,
                rotation: Mat3::IDENTITY,
            }],
            ..Default::default()
        };
        assert_eq!(mod_region(None, &ModView::default(), &m), "L4RB");
        assert_eq!(mod_region(Some("L4RA"), &ModView::default(), &m), "L4RA");
        assert_eq!(mod_region(None, &ModView::default(), &MapMod::default()), DEFAULT_REGION);
    }

    /// One frame of the whole layout runs without a GPU (open and welcome).
    #[test]
    fn layout_runs_headless() {
        let s = Scratch::new("editor-layout");
        let mut a = app(&s);
        let raw = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0))),
            ..Default::default()
        };
        let _ = a.frame(raw());
        a.new_mod("l", "Layout", "L4RA").unwrap();
        for _ in 0..2 {
            let out = a.frame(raw());
            assert!(!out.shapes.is_empty());
        }
        assert_eq!(a.title(), "Overbright Map Editor (preview) — Layout · L4RA");
    }
}
