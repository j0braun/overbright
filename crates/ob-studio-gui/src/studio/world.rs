//! The World and Races workspaces: the map editor (src/editor/, docs/modding/map-editor.md) hosted in Studio's
//! window. The editor's parts are the same code `overbright-editor` draws in its own window ([`App::ui`]); here
//! Studio lays them out: the toolbar in the toolbar row, the mod's tree in the left panel, the properties in the
//! right dock's Inspector, the bottom tabs (Prop browser, Validation, Simulate) after Problems · Log · Jobs, the
//! hint line under the view, and the view itself rendered with the window's one GPU device and egui context.
//!
//! World and Races share one editor on the open mod: World brings the prop tools forward, Races the race tools
//! (the editor's own rule: what is selected picks the tools).

use std::time::Instant;

use egui::{Align2, Rect, Stroke, UiBuilder, pos2, vec2};
use nfsu2::studio_api::render::Gpu;
use ob_look::lamp::Mark;

use crate::draw::camera::Camera;
use crate::editor::app::{self, App};
use crate::editor::menu;
use crate::editor::state::{EditorState, Focus, Level, Mode, RacePoint};
use crate::tools::map::assets::Place;
use crate::editor::theme::{font, rgb, tokens};

use super::Studio;
use super::address::{Address, Target};
use super::draw;
use super::layout::Layout;
use super::workspace::{Kind, Workspace};
use super::words;

/// The hint line under the view, and the strips over it (points).
pub const STRIP_H: f32 = 26.0;

/// The map editor as Studio hosts it.
pub struct Hosted {
    pub app: App,
    /// The mod folder Studio last asked the editor to open (once per mod, so a mod that fails to open is not
    /// retried every frame).
    asked: Option<String>,
    /// The workspace the editor's tools were last brought forward for.
    tools_for: Option<Workspace>,
    /// The editor's bottom tab as last seen (a part asking for its tab brings it forward).
    seen_tab: &'static str,
    /// What each workspace had when it was left: its selection, camera and layers.
    kept: Vec<(Workspace, Kept)>,
}

/// A workspace's own view of the open mod, kept while another workspace shows.
#[derive(Clone, Debug)]
pub(super) struct Kept {
    pub selection: Vec<usize>,
    pub race: Option<usize>,
    pub point: Option<RacePoint>,
    pub camera: Camera,
    pub collision: crate::editor::collision::Show,
    pub tab: &'static str,
}

impl Kept {
    fn take(s: &EditorState, tab: &'static str) -> Kept {
        Kept {
            selection: s.selection.clone(),
            race: s.race,
            point: s.point,
            camera: s.camera,
            collision: s.collision,
            tab,
        }
    }
}

/// The editor's focus for a workspace.
pub(super) fn focus_of(ws: Workspace) -> Focus {
    match ws {
        Workspace::World => Focus::World,
        Workspace::Races => Focus::Races,
        _ => Focus::Both,
    }
}

impl Studio {
    /// Host `app` (made with [`App::new_in`] on this window's context) as the World and Races workspaces.
    pub fn attach_editor(&mut self, app: App) {
        let seen_tab = app.tab;
        crate::editor::menu::set_hosted(&app.ctx);
        let mut app = app;
        let sim = app.panels.sim.clone();
        app.panels.tabs.push(("telemetry", words::TELEMETRY, Box::new(crate::editor::sim::graphs::TelemetryTab(sim))));
        self.editor = Some(Hosted { app, asked: None, tools_for: None, seen_tab, kept: Vec::new() });
    }

    /// The editor shows in this frame: a tool workspace with a mod open in the editor.
    pub fn editing(&self) -> bool {
        self.nav.workspace.kind() == Kind::Tool && self.editor.as_ref().is_some_and(|h| h.app.state.is_some())
    }

    /// The hosted editor's state, while editing.
    pub fn editor_app(&mut self) -> Option<&mut App> {
        if !self.editing() {
            return None;
        }
        self.editor.as_mut().map(|h| &mut h.app)
    }

    /// Before a frame: the open mod opened in the editor on entering World or Races, the tools brought forward
    /// for the workspace, a part's tab request followed.
    pub(super) fn sync_editor(&mut self) {
        let ws = self.nav.workspace;
        if ws.kind() != Kind::Tool && ws != Workspace::Browser {
            return;
        }
        let folder = self.open_mod().map(|m| m.folder.clone());
        let Some(h) = self.editor.as_mut() else { return };
        if let Some(folder) = folder
            && h.asked.as_deref() != Some(folder.as_str())
        {
            h.asked = Some(folder.clone());
            h.kept.clear();
            let open = h.app.state.as_ref().is_some_and(|s| s.session.folder == folder);
            if !open && let Err(e) = h.app.open(&folder, None) {
                self.verdict = Some(format!("Could not open the mod: {e:#}"));
            }
            h.tools_for = None;
        }
        if let Some(s) = h.app.state.as_mut()
            && ws != Workspace::Browser
            && h.tools_for != Some(ws)
        {
            // Leave the last workspace's selection, camera and layers with it; take this one's back.
            let prev = h.tools_for;
            if let Some(prev) = prev {
                h.kept.retain(|(w, _)| *w != prev);
                h.kept.push((prev, Kept::take(s, h.app.tab)));
            }
            h.tools_for = Some(ws);
            s.focus = focus_of(ws);
            match h.kept.iter().find(|(w, _)| *w == ws).map(|(_, k)| k.clone()) {
                Some(k) => {
                    let n = s.session.world.placements.len();
                    s.selection = k.selection.into_iter().filter(|&i| i < n).collect();
                    s.race = k.race.filter(|&i| i < s.session.map.races.len());
                    s.point = k.point;
                    s.camera = k.camera;
                    s.collision = k.collision;
                    h.app.tab = k.tab;
                }
                None => {
                    // First time here: World starts with the prop browser, Races with the race strip,
                    // and each its own selection.
                    if prev.is_some() {
                        s.selection.clear();
                        s.point = None;
                        s.collision = crate::editor::collision::Show::Off;
                    }
                    match ws {
                        Workspace::World => h.app.tab = "browser",
                        Workspace::Races => h.app.tab = crate::editor::race_strip::TAB,
                        _ => {}
                    }
                }
            }
            s.mode = if matches!(ws, Workspace::Races | Workspace::Runs) { Mode::Races } else { Mode::Props };
            if ws == Workspace::Runs {
                // The tab of a run's graphs first; the run's own view (the editor's, with its timeline).
                h.app.tab = "telemetry";
            }
            if matches!(ws, Workspace::Races | Workspace::Runs) && s.race.is_none() && !s.session.map.races.is_empty() {
                s.race = Some(0);
            }
        }
        if h.app.tab != h.seen_tab {
            h.seen_tab = h.app.tab;
            if h.app.tab != "log" {
                self.bottom_tab = super::shell::BottomTab::Editor;
            }
        }
    }

    /// The open mod's region's neighbourhoods (World and Races only).
    pub(super) fn hoods(&self) -> Vec<Place> {
        if !self.editing() {
            return vec![];
        }
        self.editor.as_ref().and_then(|h| h.app.state.as_ref()).map(|s| s.places()).unwrap_or_default()
    }

    /// Find ▸ Places: the editor's view goes to the neighbourhood's centre.
    pub(super) fn go_hood(&mut self, i: usize) {
        let Some(p) = self.hoods().into_iter().nth(i) else { return };
        let c = p.centre();
        if let Some(s) = self.editor.as_mut().and_then(|h| h.app.state.as_mut()) {
            self.verdict = crate::editor::views::go_to(s, &format!("{:.0}, {:.0}", c.x, c.y)).err();
            s.status = format!("Went to {}", p.name);
        }
    }

    /// The names after the workspace in the address bar: the region, the neighbourhood under the camera, then the
    /// selected prop or race.
    pub(super) fn editor_crumbs(&self) -> Vec<String> {
        let Some(s) = self.editor.as_ref().and_then(|h| h.app.state.as_ref()) else { return vec![] };
        if self.nav.workspace.kind() != Kind::Tool {
            return vec![];
        }
        let mut v = vec![s.session.region.clone()];
        v.extend(s.place_name());
        match self.nav.workspace {
            Workspace::Races => {
                if let Some(r) = s.race.and_then(|i| s.session.map.races.get(i)) {
                    v.push(if r.name.is_empty() { r.id.clone() } else { r.name.clone() });
                }
            }
            Workspace::Runs => {
                let shown = self.editor.as_ref().map(|h| h.app.panels.sim.borrow().shown.clone()).unwrap_or_default();
                if !shown.is_empty() {
                    v.push(shown);
                }
            }
            _ => {
                if let Some(&i) = s.selection.first() {
                    let w = &s.session.world;
                    if let Some(p) = w.placements.get(i) {
                        v.push(w.solids.get(p.solid).map_or_else(|| format!("Prop {i}"), |so| so.name.clone()));
                    }
                }
            }
        }
        v
    }

    /// The selected item's address (`ob://<mod>/placement/<region>/<section>/<index>`, `ob://<mod>/race/<mod>:<id>`).
    pub(super) fn editor_address(&self) -> Option<Address> {
        if self.nav.workspace.kind() != Kind::Tool {
            return None;
        }
        let s = self.editor.as_ref()?.app.state.as_ref()?;
        let pack = s.session.folder.as_str();
        match self.nav.workspace {
            Workspace::Races => {
                let r = s.race.and_then(|i| s.session.map.races.get(i))?;
                Some(Address::item(pack, "race", &[&format!("{pack}:{}", r.id)]))
            }
            Workspace::Runs => None,
            _ => {
                let i = *s.selection.first()?;
                let (sec, n) = s.session.world.original_key(i)?;
                Some(Address::item(pack, "placement", &[&s.session.region, &sec.to_string(), &n.to_string()]))
            }
        }
    }

    /// Show the item `a` names in the editor (its workspace already chosen). Err: why not.
    pub(super) fn editor_go(&mut self, a: &Address) -> Result<(), String> {
        let Target::Item { kind, key, .. } = &a.target else { return Ok(()) };
        self.sync_editor();
        let Some(h) = self.editor.as_mut() else { return Ok(()) };
        let Some(s) = h.app.state.as_mut() else { return Err(words::THE_MOD_IS_NOT_OPEN_IN.into()) };
        match kind.as_str() {
            "placement" | "prop" => {
                let [region, sec, n] = key.as_slice() else {
                    return Err(words::A_PROP_S_ADDRESS_IS_REGION.into());
                };
                if *region != s.session.region {
                    return Err(format!("This mod is open on {}, not {region}", s.session.region));
                }
                let (Ok(sec), Ok(n)) = (sec.parse::<u32>(), n.parse::<u32>()) else {
                    return Err(words::A_PROP_S_ADDRESS_IS_REGION.into());
                };
                let i = s.session.world.original_placement(sec, n).ok_or(words::NO_SUCH_PROP_IN_THIS_REGION)?;
                s.selection = vec![i];
                s.mode = Mode::Props;
                menu::post(&self.ctx, menu::Command::FrameSelection);
                Ok(())
            }
            "race" => {
                let id = key.first().and_then(|k| k.split_once(':')).map(|(_, id)| id).unwrap_or_default();
                let i = s.session.map.races.iter().position(|r| r.id == id).ok_or(words::NO_SUCH_RACE_IN_THIS_MOD)?;
                s.race = Some(i);
                s.point = None;
                s.mode = Mode::Races;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// The editor's history: true when it did something (saved, took in another program's step).
    pub fn tick(&mut self, now: Instant) -> bool {
        match self.editor.as_mut() {
            Some(h) if h.app.next_tick() <= now => h.app.tick(now),
            _ => false,
        }
    }

    /// When [`Studio::tick`] wants to run next.
    pub fn next_tick(&self) -> Option<Instant> {
        self.editor.as_ref().filter(|h| h.app.state.is_some()).map(|h| h.app.next_tick())
    }

    /// The window's close button: the editor saves first (and asks only when that fails). True: close now.
    pub fn close_requested(&mut self) -> bool {
        match self.editor.as_mut() {
            Some(h) => h.app.close_requested(),
            None => true,
        }
    }

    /// Render the editor's view (before egui draws the frame), while it shows.
    pub fn render_view(&mut self, gpu: &Gpu, egui_r: &mut egui_wgpu::Renderer, encoder: &mut wgpu::CommandEncoder) {
        if self.editing()
            && let Some(h) = self.editor.as_mut()
        {
            h.app.render_view(gpu, egui_r, encoder);
        } else if self.nav.workspace == Workspace::Browser
            && let Some(h) = self.editor.as_mut()
        {
            // The Browser's model pictures, drawn by the prop browser's renderer (a few a frame).
            h.app.build_thumbs(gpu);
        }
    }

    /// The Browser shows and the prop pictures it asked for are not all drawn yet (a `--shot` waits for them).
    pub fn browser_pictures_pending(&self) -> bool {
        self.nav.workspace == Workspace::Browser
            && self.editor.as_ref().is_some_and(|h| h.app.panels.browser.borrow().thumbs.pending() > 0)
    }

    /// The editor's toolbar in the toolbar row.
    pub(super) fn editor_toolbar(&mut self, ui: &mut egui::Ui, tb: Rect) {
        let Some(app) = self.editor_app() else { return };
        ui.scope_builder(UiBuilder::new().max_rect(tb.shrink2(vec2(8.0, 0.0))), |ui| {
            ui.set_clip_rect(tb);
            app.toolbar_ui(ui);
        });
    }

    /// The mod's tree in the left panel (under Studio's tab strip).
    pub(super) fn editor_tree(&mut self, ui: &mut egui::Ui, r: Rect) {
        let Some(app) = self.editor_app() else { return };
        ui.scope_builder(UiBuilder::new().max_rect(r), |ui| {
            ui.set_clip_rect(r);
            app.tree_ui(ui, false);
        });
    }

    /// The Runs workspace's list in the left panel.
    pub(super) fn runs_left(&mut self, ui: &mut egui::Ui, r: Rect) {
        let Some(folder) = self.open_mod().map(|m| m.folder.clone()) else { return };
        let Some(h) = self.editor.as_ref() else { return };
        let sim = h.app.panels.sim.clone();
        let dir = crate::editor::sim::library::dir(&self.maps, &folder);
        // A run the Simulate button just kept is in the list at once.
        if sim.borrow().running() {
            self.runs.changed();
        }
        self.runs.ui(ui, r, &sim, &dir);
    }

    /// The selection's properties in the Inspector.
    pub(super) fn editor_inspector(&mut self, ui: &mut egui::Ui, r: Rect) {
        let Some(app) = self.editor_app() else { return };
        ui.scope_builder(UiBuilder::new().max_rect(r), |ui| {
            ui.set_clip_rect(r);
            app.properties_ui(ui);
        });
    }

    /// The editor's bottom tabs after Studio's own (key, title); the editor's Log is Studio's.
    pub(super) fn editor_tabs(&self) -> Vec<(&'static str, &'static str)> {
        match self.editor.as_ref() {
            Some(h) if self.editing() => {
                let ws = self.nav.workspace;
                let runs = ws == Workspace::Runs;
                // World: the props' tabs (no Simulate); Races: the race's (no Prop browser).
                let mut v: Vec<_> = h
                    .app
                    .tab_titles()
                    .into_iter()
                    .filter(|t| t.0 != "log" && (runs || t.0 != "telemetry"))
                    .filter(|t| !(ws == Workspace::World && t.0 == crate::editor::sim::TAB))
                    .filter(|t| !(ws == Workspace::Races && t.0 == "browser"))
                    .filter(|t| !(ws == Workspace::World && t.0 == crate::editor::race_strip::TAB))
                    .collect();
                // Runs: its graphs first, so the tab strip's room is not spent before it.
                if runs && let Some(k) = v.iter().position(|t| t.0 == "telemetry") {
                    let t = v.remove(k);
                    v.insert(0, t);
                }
                v
            }
            _ => vec![],
        }
    }

    /// The editor's tab `key` was picked in the bottom dock.
    pub(super) fn editor_pick_tab(&mut self, key: &'static str) {
        if let Some(h) = self.editor.as_mut() {
            h.app.tab = key;
            h.seen_tab = key;
        }
    }

    /// The editor's tab shown: its key.
    pub(super) fn editor_tab(&self) -> Option<&'static str> {
        self.editor.as_ref().map(|h| h.app.tab)
    }

    /// A bottom tab's body: the editor's tab, its log, or its problems.
    pub(super) fn editor_bottom(&mut self, ui: &mut egui::Ui, r: Rect, what: EditorBody) {
        let time = if self.fixed_clock { 0.15 } else { self.ctx.input(|i| i.time as f32) };
        let Some(app) = self.editor_app() else { return };
        match what {
            EditorBody::Tab => {
                ui.scope_builder(UiBuilder::new().max_rect(r), |ui| {
                    ui.set_clip_rect(r);
                    app.tab_ui(ui);
                });
            }
            EditorBody::Log => {
                let saved = app.tab;
                app.tab = "log";
                ui.scope_builder(UiBuilder::new().max_rect(r), |ui| {
                    ui.set_clip_rect(r);
                    app.tab_ui(ui);
                });
                app.tab = saved;
            }
            EditorBody::Problems => {
                let t = tokens();
                let p = ui.painter().with_clip_rect(r);
                let issues = app.state.as_ref().map(|s| s.session.issues.clone()).unwrap_or_default();
                let mut y = r.top() + 14.0;
                if issues.is_empty() {
                    draw::lamp(&p, pos2(r.left() + 8.0, y), 10.0, Mark::Ok, 0.0);
                    p.text(
                        pos2(r.left() + 24.0, y),
                        Align2::LEFT_CENTER,
                        super::words::NO_PROBLEMS,
                        font("body", 15.0),
                        rgb(t.ink),
                    );
                }
                for i in issues {
                    if y > r.bottom() {
                        break;
                    }
                    let mark = match i.level {
                        Level::Error => Mark::Error,
                        Level::Warning => Mark::Warn,
                        _ => Mark::Info,
                    };
                    draw::lamp(&p, pos2(r.left() + 8.0, y), 10.0, mark, time);
                    p.text(pos2(r.left() + 24.0, y), Align2::LEFT_CENTER, &i.text, font("body", 15.0), rgb(t.ink));
                    p.line_segment(
                        [pos2(r.left(), y + 16.0), pos2(r.right(), y + 16.0)],
                        Stroke::new(1.0, draw::rule()),
                    );
                    y += 32.0;
                }
            }
        }
    }

    /// The view: the strips over it (the game window, read-only), the editor's view, the hint line under it.
    pub(super) fn editor_view(&mut self, ui: &mut egui::Ui, l: &Layout) {
        let ctx = self.ctx.clone();
        let Some(app) = self.editor_app() else { return };
        let mut view = l.view;
        let strip_r = |v: Rect| Rect::from_min_size(v.min, vec2(v.width(), STRIP_H));
        let fill = app::strip_frame().fill;
        if let Some(words) = crate::editor::testplay::strip(&ctx) {
            let r = strip_r(view);
            ui.painter().rect_filled(r, 0.0, fill);
            ui.scope_builder(UiBuilder::new().max_rect(r.shrink2(vec2(10.0, 0.0))), |ui| app::game_strip(ui, &words));
            view.min.y += STRIP_H;
        }
        if let Some(text) = app.read_only() {
            let r = strip_r(view);
            ui.painter().rect_filled(r, 0.0, fill);
            let mut retry = false;
            ui.scope_builder(UiBuilder::new().max_rect(r.shrink2(vec2(10.0, 0.0))), |ui| {
                retry = app::read_only_strip(ui, &text);
            });
            if retry {
                app.retry_write();
            }
            view.min.y += STRIP_H;
        }
        if app.layout.hint {
            let r = Rect::from_min_size(pos2(view.left(), view.bottom() - STRIP_H), vec2(view.width(), STRIP_H));
            ui.painter().rect_filled(r, 0.0, fill);
            ui.scope_builder(UiBuilder::new().max_rect(r.shrink2(vec2(10.0, 0.0))), |ui| app.hint_ui(ui));
            view.max.y -= STRIP_H;
        }
        ui.painter().rect_filled(view, 0.0, draw::page_fill());
        ui.scope_builder(UiBuilder::new().max_rect(view), |ui| {
            ui.set_clip_rect(view);
            app.view_ui(ui);
        });
    }
}

/// What a bottom tab of the editor shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorBody {
    /// The editor's own tab (`App::tab`).
    Tab,
    Log,
    Problems,
}
