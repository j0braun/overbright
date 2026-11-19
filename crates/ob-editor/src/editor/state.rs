//! The editor's shared state: one map mod open on one region. Every panel
//! reads and changes this; edits end in [`commit`], which records an undo
//! step and applies the mod to the world again.

use std::collections::HashMap;
use std::path::PathBuf;

use glam::Vec3;

use super::undo::Undo;
use crate::draw::camera::Camera;
use crate::formats::roads::RoadNet;
use crate::tools::map::MapMod;
use crate::tools::map::edit::RacePart;
use crate::world::World;

/// The editor's two modes (Tab switches).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Props,
    Races,
}

/// What the centre shows (M switches).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    ThreeD,
    Map,
}

/// The selected part of the current race: a checkpoint, a grid slot, the
/// start or the finish line (`index` within the part).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RacePoint {
    pub part: RacePart,
    pub index: usize,
}

/// How bad a validation entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Info,
    Warning,
    /// Blocks Test race.
    Error,
}

/// One validation entry: its text and where "Show" takes the camera.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub level: Level,
    pub text: String,
    /// The race it is about (index into `map.races`).
    pub race: Option<usize>,
    /// The spot to show.
    pub at: Option<Vec3>,
}

/// The whole editor state.
pub struct EditorState {
    /// The game folder (`NFSU2_ROOT`).
    pub root: PathBuf,
    /// The region being edited ("L4RA").
    pub region: String,
    /// The map mods' folder (`<data>/maps`).
    pub maps: PathBuf,
    /// The mod being edited: its folder name under `maps`.
    pub folder: String,
    pub map: MapMod,
    /// The original world as loaded, never edited. Only its placements and
    /// knock-over props are filled (the rest lives once, in `world`).
    pub base: World,
    /// `base` with `map` applied: what the viewport draws.
    pub world: World,
    /// Selected placements (indices into `world.placements`).
    pub selection: Vec<usize>,
    /// The race being edited (index into `map.races`).
    pub race: Option<usize>,
    pub point: Option<RacePoint>,
    pub camera: Camera,
    pub mode: Mode,
    pub view: View,
    pub undo: Undo,
    /// Changed since the last save.
    pub dirty: bool,
    /// The status bar's last message.
    pub status: String,
    /// Validation results (the Races unit fills them).
    pub issues: Vec<Issue>,
    /// Road networks by base track, read on first use (None: unreadable).
    pub nets: HashMap<u16, Option<RoadNet>>,
    /// Placements moved or hidden since the viewport last looked
    /// (`world.take_changed()` holds which).
    pub world_changed: bool,
    /// The set of added placements changed: the renderer must be built
    /// again.
    pub adds_changed: bool,
}

/// A panel of the window.
pub trait Panel {
    fn ui(&mut self, s: &mut EditorState, ui: &mut egui::Ui);
}

impl EditorState {
    /// A state for `map` on `world`, a region as loaded without map mods
    /// (`World::load`); `map` is applied to it here. The undo history
    /// starts at `map`.
    pub fn new(
        root: PathBuf,
        region: &str,
        maps: PathBuf,
        folder: &str,
        map: MapMod,
        world: World,
        camera: Camera,
    ) -> EditorState {
        let mut base = World::default();
        base.placements = world.placements.clone();
        base.smackables = world.smackables.clone();
        let mut s = EditorState {
            root,
            region: region.to_string(),
            maps,
            folder: folder.to_string(),
            undo: Undo::new(&map),
            map,
            base,
            world,
            selection: Vec::new(),
            race: None,
            point: None,
            camera,
            mode: Mode::Props,
            view: View::ThreeD,
            dirty: false,
            status: String::new(),
            issues: Vec::new(),
            nets: HashMap::new(),
            world_changed: true,
            adds_changed: true,
        };
        s.race = (!s.map.races.is_empty()).then_some(0);
        super::undo::apply_map(&mut s);
        s
    }
}

/// Record the edit just made to `s.map` as one undo step named `label`
/// ("Move XC_CONE_01") and apply the mod to the world again. Nothing
/// happens when the mod did not change.
pub fn commit(s: &mut EditorState, label: &str) {
    super::undo::commit(s, label);
}

/// Where the world point `p` lands in the viewport `rect` (None behind
/// the camera).
pub fn project(s: &EditorState, rect: egui::Rect, p: Vec3) -> Option<egui::Pos2> {
    let aspect = (rect.width() / rect.height().max(1.0)).max(1e-3);
    let clip = s.camera.view_projection(aspect) * p.extend(1.0);
    if clip.w <= 1e-4 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some(egui::pos2(rect.left() + (ndc.x * 0.5 + 0.5) * rect.width(), rect.top() + (0.5 - ndc.y * 0.5) * rect.height()))
}
