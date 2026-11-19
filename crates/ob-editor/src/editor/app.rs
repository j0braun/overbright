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
use super::state::{EditorState, Mode, Panel, View};
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
/// The autosave in the mod's `.editor/` folder.
pub const AUTOSAVE: &str = store::AUTOSAVE;
/// Editor-only state in the mod's `.editor/` folder (camera, region, panel
/// sizes); not exported.
pub const EDITOR_TOML: &str = store::EDITOR_TOML;
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
    store::editor_dir(maps, folder).join(AUTOSAVE)
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

/// The resizable panels' sizes (points): the tree's and the properties'
/// widths, the bottom tabs' height. None: the panel's default.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PanelSizes {
    pub tree: Option<f32>,
    pub properties: Option<f32>,
    pub bottom: Option<f32>,
}

/// A size worth keeping: finite and within what any window gives a panel.
fn panel_size(v: f32) -> Option<f32> {
    (v.is_finite() && (40.0..=8192.0).contains(&v)).then_some(v)
}

/// Editor-only state of a mod (`.editor/editor.toml`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ModView {
    pub region: Option<String>,
    pub camera: Option<(Vec3, f32, f32)>,
    pub panels: PanelSizes,
}

impl ModView {
    pub fn read(maps: &Path, folder: &str) -> ModView {
        let Ok(text) = std::fs::read_to_string(store::editor_dir(maps, folder).join(EDITOR_TOML)) else {
            return ModView::default();
        };
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
        let panels = PanelSizes {
            tree: f("tree_width").and_then(panel_size),
            properties: f("properties_width").and_then(panel_size),
            bottom: f("bottom_height").and_then(panel_size),
        };
        ModView { region, camera, panels }
    }

    pub fn write(&self, maps: &Path, folder: &str) -> Result<()> {
        let mut s = String::from("# Overbright map editor: this mod's view (not part of the mod).\n");
        if let Some(r) = &self.region {
            s.push_str(&format!("region = \"{r}\"\n"));
        }
        if let Some((p, yaw, pitch)) = self.camera {
            s.push_str(&format!("camera = [{:?}, {:?}, {:?}]\nyaw = {:?}\npitch = {:?}\n", p.x, p.y, p.z, yaw, pitch));
        }
        for (key, v) in [
            ("tree_width", self.panels.tree),
            ("properties_width", self.panels.properties),
            ("bottom_height", self.panels.bottom),
        ] {
            if let Some(v) = v.and_then(panel_size) {
                s.push_str(&format!("{key} = {:?}\n", v.round()));
            }
        }
        crate::qol::saves::write_atomic(&store::editor_dir(maps, folder).join(EDITOR_TOML), s.as_bytes(), false)?;
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
/// [--shot out.png --size WxH [--scale F] [--mode props|races] [--view 3d|map|street|whole|top] [--light work|game] [--tab <name>]]`
#[derive(Clone, Debug, PartialEq)]
pub struct Cli {
    pub folder: Option<String>,
    pub region: Option<String>,
    /// The map mods' folder (default `<data>/maps`).
    pub maps: Option<PathBuf>,
    pub shot: Option<PathBuf>,
    pub size: (u32, u32),
    pub scale: f32,
    /// Which side of the selection is in front (`--mode props|races`):
    /// the properties panel and the tools follow it.
    pub mode: Option<Mode>,
    /// A toolbar tool picked (`--tool select|move|rotate|checkpoint|start|finish|grid`).
    pub tool: Option<menu::Tool>,
    pub view: Option<View>,
    /// The 3D view's look (`--view street|whole|top`).
    pub look: Option<String>,
    /// Work light or Game look (`--light work|game`).
    pub light: Option<String>,
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
            tool: None,
            view: None,
            look: None,
            light: None,
            tab: None,
            select: Vec::new(),
            sim: None,
        }
    }
}

pub const USAGE: &str = "usage: overbright-editor [--mod <folder>] [--region <code>] [--maps <dir>]\n       overbright-editor --shot out.png [--size WxH] [--scale F] [--mod <folder>] [--region <code>] [--mode props|races] [--tool <name>] [--view 3d|map|street|whole|top] [--light work|game] [--tab browser|validation|simulate|log] [--select N,M] [--sim stream.txt[@seconds[:car]]]";

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
                "--tool" => {
                    let v = val("--tool")?.to_ascii_lowercase();
                    c.tool = Some(
                        menu::Tool::ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(&v)).with_context(|| {
                            format!("--tool: `{v}` (select, move, rotate, checkpoint, start, finish or grid)")
                        })?,
                    );
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
                "--light" => {
                    let v = val("--light")?;
                    ensure!(v == "work" || v == "game", "--light: `{v}` (work or game)");
                    c.light = Some(v);
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
    /// Draw every frame at this animation time (seconds) instead of the
    /// time since the start: `--shot` pictures are then the same on every
    /// run.
    fn fix_clock(&mut self, _seconds: f32) {}
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
    /// Buttons at the toolbar's right, before Drive here and Test race
    /// (Simulate).
    pub run_tools: Vec<Box<dyn Panel>>,
    /// The props' tool and snap settings (the toolbar's Select / Move /
    /// Rotate / Snap).
    pub props: parts::Shared<super::props::Props>,
    /// The race tools and road snapping (the toolbar's race slot).
    pub races: parts::Shared<super::races::Races>,
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
        let races = std::rc::Rc::new(std::cell::RefCell::new(super::races::Races::new()));
        // Select at the start: a click selects, nothing is armed (the
        // race tools start at Select on their own).
        props.borrow_mut().tool = super::props::Tool::Select;
        Panels {
            viewport: Box::new(super::viewport::Viewport::new()),
            tree: vec![Box::new(parts::TreeRaces(races.clone())), Box::new(parts::TreeProps)],
            properties: vec![Box::new(parts::PropsPanel(props.clone())), Box::new(parts::RacesPanel(races.clone()))],
            tabs: vec![
                ("browser", "Prop browser", Box::new(parts::BrowserTab(browser.clone()))),
                ("validation", "Validation", Box::new(parts::RacesValidation(races.clone()))),
                (super::sim::TAB, "Simulate", Box::new(super::sim::SimTab(sim.clone()))),
            ],
            run_tools: vec![Box::new(super::sim::SimTool(sim.clone()))],
            hooks: vec![
                Box::new(super::testplay::TestPanel { test: None, races: Some(races.clone()) }),
                Box::new(parts::RacesHook(races.clone())),
                Box::new(super::sim::SimHook(sim.clone())),
            ],
            overlays: vec![
                Box::new(parts::Follow(races.clone())),
                Box::new(parts::PropsOverlay { props: props.clone(), browser }),
                Box::new(parts::RacesOverlay { races: races.clone(), dragging: false }),
                Box::new(super::sim::SimOverlay(sim)),
            ],
            props,
            races,
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
    use crate::editor::races::Races;
    use crate::editor::state::{Mode, View};
    use crate::editor::{tree_props, tree_races};

    pub type Shared<T> = Rc<RefCell<T>>;

    /// The properties of the selected props.
    pub struct PropsPanel(pub Shared<Props>);

    impl Panel for PropsPanel {
        fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
            if s.mode == Mode::Props {
                self.0.borrow_mut().ui(s, ui);
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
    /// selection. Picking a prop there brings the prop tools forward.
    pub struct TreeProps;

    impl Panel for TreeProps {
        fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
            let before = s.selection.clone();
            if let Some(tree_props::Event::Frame) = tree_props::ui(s, ui) {
                menu::post(ui.ctx(), Command::FrameSelection);
            }
            if s.selection != before && !s.selection.is_empty() {
                s.mode = Mode::Props;
            }
        }
    }

    /// No modes: the tools follow the selection. Runs first over the
    /// viewport each frame: a race tool stays armed only while the race's
    /// side has the selection (a prop picked anywhere disarms it).
    pub struct Follow(pub Shared<Races>);

    impl Overlay for Follow {
        fn overlay(&mut self, s: &mut EditorState, _: &egui::Context, _: &egui::Painter, _: egui::Rect) -> bool {
            let mut r = self.0.borrow_mut();
            if s.mode == Mode::Props && r.tool != crate::editor::races::Tool::Select {
                r.tool = crate::editor::races::Tool::Select;
            }
            false
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
                && let Some(solid) = s.selection.first().and_then(|&i| s.session.world.placements.get(i)).map(|pl| pl.solid)
            {
                let hidden = crate::world::HIDDEN_FLAG;
                s.selection = (0..s.session.world.placements.len())
                    .filter(|&i| s.session.world.placements[i].solid == solid && s.session.world.placements[i].flags & hidden == 0)
                    .collect();
                s.status = format!("Selected {} of {}", s.selection.len(), props::name(&s.session.world, s.selection[0]));
            }
            taken
        }
    }

    /// The tree's Races branch.
    pub struct TreeRaces(pub Shared<Races>);

    impl Panel for TreeRaces {
        fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
            tree_races::tree(&mut self.0.borrow_mut(), s, ui);
        }
    }

    /// The race and its selected point (while the race's side has the
    /// selection).
    pub struct RacesPanel(pub Shared<Races>);

    impl Panel for RacesPanel {
        fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
            if s.mode == Mode::Races {
                self.0.borrow_mut().properties(s, ui);
            }
        }
    }

    /// The Validation tab: the races' checks with Show and their fixes.
    pub struct RacesValidation(pub Shared<Races>);

    impl Panel for RacesValidation {
        fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
            self.0.borrow_mut().validation(s, ui);
        }
    }

    /// Race ▸ New race, every frame.
    pub struct RacesHook(pub Shared<Races>);

    impl Panel for RacesHook {
        fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui) {
            if menu::take(ui.ctx(), Command::NewRace) {
                self.0.borrow_mut().new_race(s);
            }
        }
    }

    /// The races over the viewport in both views (the current race bright
    /// with its grid, lines and checkpoints, the others' routes dim), and
    /// in the 3D view the left button for them:
    /// - a race tool armed: a click picks a point of the selected race or
    ///   uses the tool on the ground under the mouse; a drag moves the
    ///   picked point (one undo step on release);
    /// - otherwise (Select, Move, Rotate): a click on a race point (of any
    ///   race) selects it and may drag it; a click anywhere else is the
    ///   props' (the viewport picks), so it never adds.
    ///
    /// Del, the race keys while the race's side has the selection. The
    /// hint for the tool is the shell's hint line.
    pub struct RacesOverlay {
        pub races: Shared<Races>,
        pub dragging: bool,
    }

    impl Overlay for RacesOverlay {
        fn overlay(
            &mut self,
            s: &mut EditorState,
            ctx: &egui::Context,
            painter: &egui::Painter,
            rect: egui::Rect,
        ) -> bool {
            let mut r = self.races.borrow_mut();
            r.update(s);
            match s.view {
                View::ThreeD => r.paint_overlay(s, painter, rect),
                View::Map => {
                    if let Some((centre, scale, map)) = crate::editor::sim::panel::map_transform(ctx) {
                        let c = map.center();
                        let to = |p: glam::Vec2| {
                            egui::pos2(c.x + (p.x - centre.x) * scale, c.y - (p.y - centre.y) * scale)
                        };
                        r.paint_map(s, painter, &to);
                    }
                }
            }
            let edits = s.view == View::ThreeD;
            if self.dragging && !edits {
                r.release(s);
                self.dragging = false;
            }
            if s.mode == Mode::Races {
                if !flying(ctx) {
                    r.keys(s, ctx);
                }
                if menu::take(ctx, Command::Delete) {
                    r.delete_selected(s);
                }
            }
            if !edits {
                return false;
            }
            let (pointer, pressed, down, mods) = ctx.input(|i| {
                (i.pointer.hover_pos(), i.pointer.primary_pressed(), i.pointer.primary_down(), i.modifiers)
            });
            let over = pointer.filter(|p| {
                rect.contains(*p) && ctx.layer_id_at(*p).is_none_or(|l| l.order == egui::Order::Background)
            });
            if self.dragging {
                if down {
                    if let Some(at) = pointer.and_then(|p| props::ground_under(s, rect, p)) {
                        r.drag(s, at, mods.alt);
                    }
                    return true;
                }
                r.release(s);
                self.dragging = false;
                return true;
            }
            if !pressed || flying(ctx) || super::pointer_taken(ctx) {
                return false;
            }
            let Some(at) = over.and_then(|p| props::ground_under(s, rect, p)) else { return false };
            let armed = s.mode == Mode::Races && r.tool != crate::editor::races::Tool::Select;
            if armed {
                r.click(s, at, mods.shift, mods.alt);
                self.dragging = r.is_dragging();
                return true;
            }
            // Select / Move / Rotate: a race point under the mouse, the
            // selected race's first.
            let order = s.race.into_iter().chain((0..s.session.map.races.len()).filter(|k| Some(*k) != s.race));
            let hit = order
                .filter_map(|k| s.session.map.races.get(k).and_then(|race| crate::editor::races::pick(race, at.truncate(), crate::editor::races::PICK_M)).map(|p| (k, p)))
                .next();
            match hit {
                Some((k, _)) => {
                    (s.race, s.mode) = (Some(k), Mode::Races);
                    r.click(s, at, false, mods.alt);
                    self.dragging = r.is_dragging();
                    true
                }
                None => {
                    // The props' click: the viewport picks.
                    if s.mode == Mode::Races {
                        s.point = None;
                        s.mode = Mode::Props;
                    }
                    false
                }
            }
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
    /// The commands posted last frame that no part took (logged; a test
    /// keeps it empty).
    pub untaken: Vec<menu::Command>,
    /// Which panels are open.
    pub layout: Layout,
    /// The view's rectangle in the last frame (points).
    pub view_rect: egui::Rect,
    /// The tree was a rail in the last frame.
    pub tree_rail: bool,
    /// A fixed clock (`--shot`): no frame time in the status bar, the
    /// view's animations at a fixed time, so two runs give the same
    /// picture.
    pub fixed_clock: bool,
}

/// From this window width (points) up the tree opens by itself; below it
/// it is a rail until it is unfolded.
pub const WIDE: f32 = 1400.0;
/// `panel` at a size kept in `editor.toml` (clamped to `range`) for this
/// frame; egui keeps it from then on, and dragging works as before.
fn sized(panel: egui::Panel, restore: Option<f32>, range: std::ops::RangeInclusive<f32>) -> egui::Panel {
    match restore {
        Some(v) => panel.exact_size(v.clamp(*range.start(), range.end().max(*range.start()))),
        None => panel,
    }
}

/// The toolbar's height (points).
pub const TOOLBAR_H: f32 = 40.0;

/// Which panels are open.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The tree (None: by the window's width, [`WIDE`]).
    pub tree: Option<bool>,
    pub properties: bool,
    /// The bottom tabs (folded: only their strip).
    pub bottom: bool,
    /// The hint line under the view.
    pub hint: bool,
    /// The panels' sizes as they were drawn last (kept in `editor.toml`).
    pub sizes: PanelSizes,
    /// Sizes read from `editor.toml`, each given to its panel the next
    /// time it is drawn open.
    pub restore: PanelSizes,
}

impl Default for Layout {
    fn default() -> Layout {
        Layout {
            tree: None,
            properties: true,
            bottom: true,
            hint: true,
            sizes: PanelSizes::default(),
            restore: PanelSizes::default(),
        }
    }
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
            untaken: Vec::new(),
            layout: Layout::default(),
            view_rect: egui::Rect::NOTHING,
            tree_rail: false,
            fixed_clock: false,
        }
    }

    /// The window title.
    pub fn title(&self) -> String {
        match &self.state {
            Some(s) => {
                let name = if s.session.map.name.is_empty() { s.session.folder.as_str() } else { s.session.map.name.as_str() };
                format!("Overbright Map Editor (preview) — {name} · {}{}", s.session.region, if s.session.dirty { " ●" } else { "" })
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
        // The mod open until now keeps its view.
        self.keep_view();
        match store::move_old_sidecars(&self.maps, folder) {
            Ok(0) => {}
            Ok(n) => log::info!("editor: {n} editor file(s) of {folder} moved into {}", store::EDITOR_DIR),
            Err(e) => log::warn!("editor: {folder}'s editor files not moved: {e:#}"),
        }
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
        self.layout.restore = view.panels;
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
        ModView { region: Some(region.clone()), camera: None, panels: self.layout.sizes }.write(&self.maps, folder)?;
        self.open(folder, Some(&region))
    }

    /// Save the open mod (atomic); the autosave goes.
    pub fn save(&mut self) -> Result<()> {
        let Some(s) = self.state.as_mut() else { return Ok(()) };
        save_mod(&s.maps, &s.session.folder, &s.session.map)?;
        self.keep_view();
        let Some(s) = self.state.as_mut() else { return Ok(()) };
        remove_autosave(&s.maps, &s.session.folder);
        s.session.dirty = false;
        self.autosaved = Some(s.session.map.clone());
        self.last_autosave = Instant::now();
        self.autosave_note.clear();
        let f = s.session.folder.clone();
        self.say(format!("Saved {f}"));
        Ok(())
    }

    /// Write the open mod's view (region, camera, panel sizes) to its
    /// `editor.toml` (on save and when the window closes).
    pub fn keep_view(&mut self) {
        let Some(s) = self.state.as_ref() else { return };
        let c = self.panels.viewport.kept_camera(s);
        let mut panels = self.layout.sizes;
        // A panel not drawn open since the mod was opened keeps its size.
        let r = self.layout.restore;
        panels.tree = r.tree.or(panels.tree);
        panels.properties = r.properties.or(panels.properties);
        panels.bottom = r.bottom.or(panels.bottom);
        let view = ModView { region: Some(s.session.region.clone()), camera: Some((c.position, c.yaw, c.pitch)), panels };
        if let Err(e) = view.write(&s.maps, &s.session.folder) {
            log::warn!("editor: {EDITOR_TOML} not written: {e:#}");
        }
    }

    /// Take the autosave's mod as one undoable step.
    pub fn restore(&mut self, m: MapMod) {
        if let Some(s) = self.state.as_mut() {
            s.session.map = m;
            undo::apply_map(s);
            super::commit(s, "Restore unsaved changes");
            s.session.dirty = true;
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
        if !s.session.dirty || self.autosaved.as_ref() == Some(&s.session.map) {
            return;
        }
        match write_autosave(&s.maps, &s.session.folder, &s.session.map) {
            Ok(()) => {
                self.autosaved = Some(s.session.map.clone());
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
        if self.state.as_ref().is_some_and(|s| s.session.dirty) {
            self.dialog = Some(Dialog::Unsaved { then: Then::Quit });
            false
        } else {
            self.keep_view();
            self.quit = true;
            true
        }
    }

    fn after_unsaved(&mut self, then: Then) {
        match then {
            Then::Quit => {
                self.keep_view();
                self.quit = true;
            }
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
        if self.state.as_ref().is_some_and(|s| s.session.dirty) {
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
                        name: s.session.map.name.clone(),
                        author: s.session.map.author.clone(),
                        description: s.session.map.description.clone(),
                    });
                }
            }
            A::Export => {
                if let Some(s) = &self.state {
                    let home = crate::platform::dirs::home().unwrap_or_default();
                    self.dialog =
                        Some(Dialog::Export { path: home.join(format!("{}.zip", s.session.folder)).display().to_string() });
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
            A::Tool(t) => {
                if let Some(s) = self.state.as_mut() {
                    let p = &self.panels;
                    menu::set_tool(s, &mut p.props.borrow_mut(), &mut p.races.borrow_mut(), t);
                }
            }
            A::Hint(on) => self.layout.hint = on,
            A::View(v) => {
                if let Some(s) = self.state.as_mut() {
                    s.view = v;
                }
            }
            A::Tab(t) => self.tab = t,
            A::Controls => self.dialog = Some(Dialog::Controls),
        }
    }

    /// Lay out one frame: menu bar, toolbar, status bar; the tree (or its
    /// rail) at the left, the properties at the right, the bottom tabs and
    /// the hint line under the view, the view in the middle.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let started = Instant::now();
        let width = ui.max_rect().width();
        let mut actions = Vec::new();
        menu::shortcuts(self, ui.ctx(), &mut actions);
        egui::Panel::top("menu").show(ui, |ui| menu::bar(self, ui, &mut actions));
        if self.state.is_some() {
            egui::Panel::top("toolbar").exact_size(TOOLBAR_H).show(ui, |ui| menu::toolbar(self, ui, &mut actions));
        }
        egui::Panel::bottom("status").exact_size(26.0).show(ui, |ui| menu::status(self, ui));
        if let Some(s) = self.state.as_mut() {
            let p = &mut self.panels;
            let layout = &mut self.layout;
            let wide = width >= WIDE;
            self.tree_rail = !layout.tree.unwrap_or(wide);
            if !self.tree_rail {
                let range = 180.0..=(width * 0.4).max(200.0);
                let panel = egui::Panel::left("tree")
                    .resizable(true)
                    .default_size(if wide { 270.0 } else { 230.0 })
                    .size_range(range.clone());
                let r = sized(panel, layout.restore.tree.take(), range).show(ui, |ui| {
                    if super::tree::show(s, ui, &mut p.tree, &mut actions) {
                        layout.tree = Some(false);
                    }
                });
                layout.sizes.tree = Some(r.response.rect.width());
            } else {
                egui::Panel::left("tree-rail").exact_size(super::tree::RAIL).resizable(false).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        if super::tree::rail(ui, &super::tree::rail_labels(Some(s)), super::tree::Fold::Right) {
                            layout.tree = Some(true);
                        }
                    });
                });
            }
            let title = "Properties";
            if layout.properties {
                let range = 200.0..=(width * 0.4).max(220.0);
                let panel = egui::Panel::right("properties")
                    .resizable(true)
                    .default_size(if wide { 320.0 } else { 300.0 })
                    .size_range(range.clone());
                let r = sized(panel, layout.restore.properties.take(), range).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    if super::tree::header(ui, title, super::tree::Fold::Right) {
                        layout.properties = false;
                    }
                    // Both ways: a part wider than the panel scrolls
                    // instead of drawing past its edge.
                    egui::ScrollArea::both().id_salt("properties").auto_shrink([false, false]).show(ui, |ui| {
                        for t in p.properties.iter_mut() {
                            t.ui(s, ui);
                        }
                        if p.properties.is_empty() {
                            ui.weak("Nothing selected.");
                        }
                    });
                });
                layout.sizes.properties = Some(r.response.rect.width());
            } else {
                egui::Panel::right("properties-rail").exact_size(super::tree::RAIL).resizable(false).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        if super::tree::rail(ui, &[title.to_string()], super::tree::Fold::Left) {
                            layout.properties = true;
                        }
                    });
                });
            }
            if let Some(k) = super::sim::panel::take_tab_request(ui.ctx()) {
                self.tab = k;
                layout.bottom = true;
            }
            let tab = &mut self.tab;
            let log = &self.log;
            let titles: Vec<(&'static str, &'static str)> =
                p.tabs.iter().map(|t| (t.0, t.1)).chain([("log", "Log")]).collect();
            let current = *tab;
            // The tab strip, with the fold button at its right.
            let strip = |ui: &mut egui::Ui, open: bool, actions: &mut Vec<menu::Action>| -> bool {
                let mut fold = false;
                ui.horizontal(|ui| {
                    for &(key, title) in &titles {
                        let label = egui::RichText::new(title).font(theme::font(theme::LABEL, 15.0));
                        let on = open && current == key;
                        let label = if on { label.color(theme::lime()) } else { label };
                        if ui.selectable_label(on, label).clicked() {
                            actions.push(menu::Action::Tab(key));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let dir = if open { super::tree::Fold::Down } else { super::tree::Fold::Up };
                        fold = super::tree::chevron(ui, dir).clicked();
                    });
                });
                fold
            };
            if layout.bottom {
                // Simulate shows its results table with the events under
                // it: the panel is at least tall enough for a grid of
                // racers.
                let min = if *tab == super::sim::TAB { super::sim::panel::MIN_HEIGHT } else { 80.0 };
                let panel = egui::Panel::bottom("tabs").resizable(true).default_size(190.0).min_size(min);
                let max = (ui.available_height() - 120.0).max(min);
                let r = sized(panel, layout.restore.bottom.take(), min..=max).show(ui, |ui| {
                    if strip(ui, true, &mut actions) {
                        layout.bottom = false;
                    }
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
                layout.sizes.bottom = Some(r.response.rect.height());
            } else {
                egui::Panel::bottom("tabs-folded").exact_size(34.0).resizable(false).show(ui, |ui| {
                    if strip(ui, false, &mut actions) {
                        layout.bottom = true;
                    }
                });
            }
            if layout.hint {
                egui::Panel::bottom("hint")
                    .exact_size(26.0)
                    .resizable(false)
                    .show_separator_line(false)
                    .frame(egui::Frame::NONE.fill(theme::over_page(theme::tokens().slab)).inner_margin(egui::Margin::symmetric(10, 0)))
                    .show(ui, |ui| menu::hint_line(s, &p.props.borrow(), &p.races.borrow(), ui, &mut actions));
            }
            egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme::rgb(theme::tokens().page))).show(
                ui,
                |ui| {
                    // The overlays first (their own layer above the
                    // viewport's image), so the viewport knows whether they
                    // took the pointer; each sees whether one before it did.
                    let rect = ui.max_rect();
                    self.view_rect = rect;
                    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::unique("editor.viewport.overlay"));
                    let painter = ui.ctx().layer_painter(layer).with_clip_rect(rect);
                    let mut taken = false;
                    ui.ctx().data_mut(|d| d.insert_temp(pointer_taken_id(), false));
                    for o in p.overlays.iter_mut() {
                        taken |= o.overlay(s, ui.ctx(), &painter, rect);
                        ui.ctx().data_mut(|d| d.insert_temp(pointer_taken_id(), taken));
                    }
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
        self.untaken = menu::clear_commands(ui.ctx());
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
                                    s.session.map.name = n;
                                    s.session.map.author = a;
                                    s.session.map.description = de;
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
                                    remove_autosave(&s.maps, &s.session.folder);
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
                                    s.session.dirty = false;
                                    remove_autosave(&s.maps, &s.session.folder);
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
                    let h = (ui.ctx().content_rect().height() - 160.0).max(200.0);
                    egui::ScrollArea::vertical().max_height(h).show(ui, |ui| {
                        for (group, rows) in menu::controls() {
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(group).font(theme::font(theme::LABEL, 15.0)).color(theme::lime()));
                            egui::Grid::new(("controls", group)).num_columns(2).striped(true).show(ui, |ui| {
                                for (k, v) in rows {
                                    ui.label(egui::RichText::new(k).font(theme::font(theme::SEMI, 14.0)));
                                    ui.label(v);
                                    ui.end_row();
                                }
                            });
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
        let Some(folder) = self.state.as_ref().map(|s| s.session.folder.clone()) else { return };
        let r = (|| -> Result<()> {
            if self.state.as_ref().is_some_and(|s| s.session.dirty) {
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
    // The same picture on every run: no clocks.
    app.fixed_clock = true;
    app.panels.viewport.fix_clock(SHOT_CLOCK);
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
    if app.state.is_some() {
        println!("{}", layout_line(&app.ctx));
    }
    Ok(())
}

/// The animation time of a `--shot` (seconds).
pub const SHOT_CLOCK: f32 = 10.0;

/// The `--shot` log's verdict on the toolbar: "editor layout: toolbar
/// fits …" or "editor layout: toolbar clipped: …" (scripts/checks/
/// editor_shots.sh fails a cell on the latter or without either).
pub fn layout_line(ctx: &egui::Context) -> String {
    match menu::toolbar_fit(ctx) {
        None => "editor layout: toolbar clipped: no toolbar laid out".into(),
        Some(f) => {
            let p = f.problems();
            if p.is_empty() {
                format!(
                    "editor layout: toolbar fits ({:.0} points wide, tier {}): {}",
                    f.bar.width(),
                    f.tier,
                    f.names().join(" · ")
                )
            } else {
                format!("editor layout: toolbar clipped: {}", p.join("; "))
            }
        }
    }
}

/// Apply the shot's `--mode`, `--tool`, `--view` and `--tab`.
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
        if let Some(light) = &cli.light {
            app.panels.viewport.set_look(s, light);
        }
        if !cli.select.is_empty() {
            s.selection = cli.select.iter().copied().filter(|&i| i < s.session.world.placements.len()).collect();
            // The 3D view (also Top) framed on it, as F does: the viewport
            // takes the command in the shot's first frame.
            if !s.selection.is_empty() && s.view == View::ThreeD {
                menu::post(&app.ctx, menu::Command::FrameSelection);
            }
        }
    }
    if let Some(t) = cli.tool {
        app.act(menu::Action::Tool(t));
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
            id: "a1".into(),
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
            "--tool",
            "Checkpoint",
        ])
        .unwrap();
        assert_eq!(c.folder.as_deref(), Some("docks"));
        assert_eq!(c.region.as_deref(), Some("L4RA"));
        assert_eq!(c.select, vec![3, 7]);
        assert_eq!(c.tool, Some(menu::Tool::Checkpoint));
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
        assert_eq!(c.light, None);
        for light in ["work", "game"] {
            assert_eq!(p(&["--light", light]).unwrap().light.as_deref(), Some(light));
        }
        assert!(p(&["--light", "bright"]).is_err());
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
            &["--tool", "lasso"],
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
        a.state.as_mut().unwrap().session.map.name = "Docks 2".into();
        crate::editor::commit(a.state.as_mut().unwrap(), "Rename");
        write_autosave(&maps, "docks", &a.state.as_ref().unwrap().session.map).unwrap();
        a.save().unwrap();
        assert!(!autosave_path(&maps, "docks").exists());
        assert!(!a.state.as_ref().unwrap().session.dirty);
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
        assert_eq!(st.session.region, "L4RA");
        st.session.map.props.push(add("XC_CONE_01", 2.0));
        crate::editor::commit(st, "Add XC_CONE_01");
        let t0 = Instant::now();
        a.tick(t0);
        assert!(!autosave_path(&maps, "yard").exists(), "not before a minute");
        a.tick(t0 + AUTOSAVE_EVERY + Duration::from_secs(1));
        assert!(autosave_path(&maps, "yard").exists());
        let unsaved = a.state.as_ref().unwrap().session.map.clone();

        // A crash: the next start offers the autosave.
        let mut b = app(&s);
        b.open("yard", None).unwrap();
        let Some(Dialog::Restore { map, .. }) = b.dialog.clone() else { panic!("no restore prompt: {:?}", b.dialog) };
        assert_eq!(map, unsaved);
        assert!(b.state.as_ref().unwrap().session.map.props.is_empty());
        b.restore(map);
        let st = b.state.as_mut().unwrap();
        assert_eq!(st.session.map, unsaved);
        assert!(st.session.dirty);
        assert!(crate::editor::undo::undo(st), "the restore is one undo step");
        assert!(st.session.map.props.is_empty());

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
        st.session.map.races.push(r);
        crate::editor::commit(st, "New race");
        st.race = Some(0);
        assert_eq!(st.session.undo.undo_label(), Some("New race"));
        a.act(menu::Action::Undo);
        let st = a.state.as_ref().unwrap();
        assert!(st.session.map.races.is_empty() && st.race.is_none());
        assert_eq!(st.session.undo.redo_label(), Some("New race"));
        a.act(menu::Action::Redo);
        assert_eq!(a.state.as_ref().unwrap().session.map.races.len(), 1);
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
        st.session.map.description = "x".into();
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
        let panels = PanelSizes { tree: Some(300.0), properties: None, bottom: Some(240.0) };
        let v = ModView { region: Some("L4RA".into()), camera: Some((Vec3::new(1.5, -2.0, 30.25), 0.5, -0.25)), panels };
        v.write(&maps, "v").unwrap();
        assert_eq!(ModView::read(&maps, "v"), v);
        assert!(store::editor_dir(&maps, "v").join(EDITOR_TOML).is_file());
        std::fs::write(store::editor_dir(&maps, "v").join(EDITOR_TOML), "tree_width = 1e9\nbottom_height = -3\n").unwrap();
        assert_eq!(ModView::read(&maps, "v").panels, PanelSizes::default(), "nonsense sizes are dropped");
        let m = MapMod {
            props: vec![PropEdit::Add {
                id: "a1".into(),
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

    fn frame_at(a: &mut App, w: f32, h: f32) {
        let _ = a.frame(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h))),
            ..Default::default()
        });
    }

    /// The panels' sizes are kept in the mod's `editor.toml` and come back
    /// when it is opened again.
    #[test]
    fn panel_sizes_are_kept_with_the_mod() {
        let s = Scratch::new("editor-panel-sizes");
        let maps = s.path().join("maps");
        let mut a = app(&s);
        a.new_mod("p", "Panels", "L4RA").unwrap();
        for _ in 0..2 {
            frame_at(&mut a, 1600.0, 960.0);
        }
        let first = a.layout.sizes;
        assert!(first.tree.is_some() && first.properties.is_some() && first.bottom.is_some(), "{first:?}");
        let wanted = PanelSizes { tree: Some(333.0), properties: Some(251.0), bottom: Some(277.0) };
        let mut v = ModView::read(&maps, "p");
        v.panels = wanted;
        v.write(&maps, "p").unwrap();
        let mut b = app(&s);
        b.open("p", None).unwrap();
        for _ in 0..3 {
            frame_at(&mut b, 1600.0, 960.0);
        }
        let near = |x: Option<f32>, y: Option<f32>| (x.unwrap() - y.unwrap()).abs() < 1.0;
        let got = b.layout.sizes;
        assert!(near(got.tree, wanted.tree) && near(got.properties, wanted.properties), "{got:?}");
        assert!(near(got.bottom, wanted.bottom), "{got:?}");
        // Closing writes what is drawn (a changed size included).
        b.layout.sizes.tree = Some(290.0);
        assert!(b.close_requested());
        assert_eq!(ModView::read(&maps, "p").panels.tree, Some(290.0));
        // A panel never drawn open keeps its kept size.
        let mut c = app(&s);
        c.open("p", None).unwrap();
        c.layout.properties = false;
        for _ in 0..2 {
            frame_at(&mut c, 1600.0, 960.0);
        }
        c.save().unwrap();
        assert_eq!(ModView::read(&maps, "p").panels.properties, Some(251.0));
    }

    /// An older editor's files beside `map.obmap` move into `.editor/` when
    /// the mod is opened, and are used from there.
    #[test]
    fn old_editor_files_move_on_open() {
        let s = Scratch::new("editor-old-sidecars");
        let maps = s.path().join("maps");
        let m = MapMod { name: "Old".into(), ..Default::default() };
        save_mod(&maps, "old", &m).unwrap();
        let dir = maps.join("old");
        std::fs::write(dir.join(EDITOR_TOML), "region = \"L4RB\"\n").unwrap();
        std::fs::write(dir.join("tested.txt"), "dock 0000000000000001\n").unwrap();
        let mut unsaved = m.clone();
        unsaved.description = "unsaved".into();
        std::fs::write(dir.join(AUTOSAVE), unsaved.write()).unwrap();
        let later = std::time::SystemTime::now() + Duration::from_secs(5);
        std::fs::File::options().write(true).open(dir.join(AUTOSAVE)).unwrap().set_modified(later).unwrap();
        let mut a = app(&s);
        a.open("old", None).unwrap();
        for name in [EDITOR_TOML, "tested.txt", AUTOSAVE] {
            assert!(!dir.join(name).exists() && store::editor_dir(&maps, "old").join(name).is_file(), "{name}");
        }
        assert_eq!(a.state.as_ref().unwrap().session.region, "L4RB", "the moved view is read");
        assert!(matches!(&a.dialog, Some(Dialog::Restore { map, .. }) if *map == unsaved), "{:?}", a.dialog);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2, "map.obmap and .editor/");
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

    /// The toolbar at every window size of the resolution matrix (in
    /// points: the sizes at display scale 1, 1.25, 1.5 and 2 down to the
    /// smallest window) and in every state: nothing cut off or on top of
    /// another item, the same buttons in the same order; the view keeps
    /// room for its look bar; the tree is a rail below [`WIDE`].
    #[test]
    fn toolbar_fits_everywhere_in_one_order() {
        use crate::editor::menu::{Action, Tool};
        let s = Scratch::new("editor-toolbar");
        let mut a = app(&s);
        a.new_mod("t", "A mod with a rather long name to show", "L4RA").unwrap();
        let sizes = [
            (960.0, 600.0),
            (1024.0, 768.0),
            (1280.0, 720.0),
            (1280.0, 800.0),
            (1280.0, 1024.0),
            (1366.0, 768.0),
            (1440.0, 900.0),
            (1536.0, 864.0),
            (1600.0, 900.0),
            (1600.0, 960.0),
            (1680.0, 1050.0),
            (1706.0, 960.0),
            (1920.0, 1080.0),
            (1920.0, 1200.0),
            (2560.0, 1080.0),
            (2560.0, 1440.0),
            (3440.0, 1440.0),
            (3840.0, 2160.0),
        ];
        let order = [
            "Select", "Move", "Rotate", "Snap", "Checkpoint", "Start", "Finish", "Grid", "Undo", "Redo", "Save",
            "Simulate", "Drive here", "Test race",
        ];
        let mut race = crate::editor::validate::tests::sprint(&[(20.0, 0.0), (200.0, 0.0)]);
        race.name = "Casino sprint".into();
        for state in 0..4 {
            let st = a.state.as_mut().unwrap();
            match state {
                0 => {}
                1 => {
                    st.session.map.races = vec![race.clone()];
                    st.race = Some(0);
                    crate::editor::commit(st, "New race");
                    a.act(Action::Tool(Tool::Checkpoint));
                }
                2 => a.act(Action::Tool(Tool::Move)),
                _ => {
                    a.layout.tree = Some(true);
                    a.layout.properties = false;
                    a.layout.bottom = false;
                }
            }
            for (w, h) in sizes {
                let raw = || egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h))),
                    ..Default::default()
                };
                for _ in 0..3 {
                    let _ = a.frame(raw());
                }
                let fit = menu::toolbar_fit(&a.ctx).expect("a toolbar");
                assert_eq!(fit.problems(), Vec::<String>::new(), "{w}x{h}, state {state}: {}", layout_line(&a.ctx));
                let names: Vec<&str> = fit.names().into_iter().filter(|n| *n != "Race").collect();
                assert_eq!(names, order, "{w}x{h}, state {state}");
                assert!(
                    a.view_rect.width() >= crate::editor::viewport::LOOK_BAR_WIDTH + 20.0,
                    "{w}x{h}, state {state}: the view is {} points wide",
                    a.view_rect.width()
                );
                assert!(a.view_rect.height() >= 150.0, "{w}x{h}, state {state}: the view is {}", a.view_rect.height());
                if state < 3 {
                    assert_eq!(a.tree_rail, w < WIDE, "{w}x{h}");
                }
                assert!(layout_line(&a.ctx).starts_with("editor layout: toolbar fits"));
            }
        }
    }
}
