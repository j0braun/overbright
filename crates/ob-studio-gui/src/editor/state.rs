//! The editor's shared state: one map mod open on one region (a
//! [`MapSession`]) and the window's own state. Every panel reads and
//! changes this; edits are [`Op`](crate::tools::map::ops::Op)s run through
//! [`super::undo::run`], or in-place edits ended by [`commit`], each one
//! undo step.

use std::collections::HashMap;
use std::path::PathBuf;

use glam::Vec3;

use crate::draw::camera::Camera;
use crate::formats::roads::RoadNet;
use crate::tools::map::MapMod;
use crate::tools::map::assets::{GameAssets, MapAssets};
use crate::world::World;

/// Which kind of thing is selected, and so which tools and properties
/// apply: a prop (`Props`) or a race point (`Races`). There are no
/// separate editing modes; selecting in the tree or the view sets it.
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

pub use crate::tools::map::ops::RacePoint;
pub use crate::tools::map::session::{Issue, Level, MapSession};

/// The whole editor state: the open mod (`session`: the mod, its world,
/// the undo history) and what only the window has (camera, selection,
/// tools, views).
pub struct EditorState {
    /// The game folder (`NFSU2_ROOT`).
    pub root: PathBuf,
    /// The map mods' folder (`<data>/maps`).
    pub maps: PathBuf,
    /// The mod being edited, its region and world, the undo history
    /// (`session.folder`: its folder name under `maps`).
    pub session: MapSession,
    /// Selected placements (indices into `session.world.placements`).
    pub selection: Vec<usize>,
    /// The race being edited (index into `session.map.races`).
    pub race: Option<usize>,
    pub point: Option<RacePoint>,
    pub camera: Camera,
    pub mode: Mode,
    pub view: View,
    /// The status bar's last message.
    pub status: String,
    /// Road networks by base track, read on first use through `assets`
    /// (None: unreadable).
    pub nets: HashMap<u16, Option<RoadNet>>,
    /// The game data the editor reads (tracks, road networks): the game
    /// folder's unless given.
    pub assets: Box<dyn MapAssets>,
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
        let race = (!map.races.is_empty()).then_some(0);
        EditorState {
            assets: Box::new(GameAssets::new(root.clone())),
            root,
            maps,
            session: MapSession::new(region, folder, map, world),
            selection: Vec::new(),
            race,
            point: None,
            camera,
            mode: Mode::Props,
            view: View::ThreeD,
            status: String::new(),
            nets: HashMap::new(),
        }
    }
}

/// Record the edit just made to `s.session.map` as one undo step named
/// `label` ("Move XC_CONE_01") and apply the mod to the world again.
/// Nothing happens when the mod did not change. (Panels that still edit
/// the mod in place; new code runs an [`Op`](crate::tools::map::ops::Op)
/// through [`super::undo::run`].)
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
