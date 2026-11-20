//! The map editor's edits, without the window or the GPU.
//!
//! The prop operations are free functions that take the placement they
//! work on and the position or rotation it gets (the desktop editor feeds
//! them its mouse ray and gizmo; the game's in-world editor its
//! crosshair through [`Editor`]). Each changes the mod being edited and
//! the world it is shown in at once: moved and hidden placements reach
//! the renderer through `World::take_changed`; an added placement is in
//! the world at once ([`add`] with a world) but a renderer builds its
//! instances once, so it is drawn after the renderer is built again.

use glam::{Mat3, Vec2, Vec3};

use super::format::{CustomRace, MapMod, PropEdit, PropRef, RaceKind, Spot};
// The race parts and the refusal reasons are the map data's (ob-map), kept at their old paths here.
pub use super::ops::{RacePart, Refused};
use crate::draw::camera::Camera;
use crate::world::{World, parts_of, transform_of};

/// Placements larger than this (radius, metres) are not picked: road and
/// terrain pieces, buildings' shells.
pub const MAX_PICK_RADIUS: f32 = 40.0;
/// Picking reaches this far.
pub const PICK_RANGE: f32 = 300.0;

/// The editor's two pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Props,
    Races,
}

/// The free camera: yaw / pitch as [`Camera`]'s.
pub fn fly(cam: &mut Camera, mv: Vec3, turn: Vec2, dt: f32, fast: bool) {
    let speed = if fast { 80.0 } else { 20.0 };
    cam.yaw += turn.x * 1.8 * dt;
    cam.pitch = (cam.pitch + turn.y * 1.4 * dt).clamp(-1.5, 1.5);
    let f = cam.forward();
    let flat = Vec3::new(f.x, f.y, 0.0).normalize_or(Vec3::Y);
    let right = flat.cross(Vec3::Z);
    cam.position += (flat * mv.x + right * mv.y + Vec3::Z * mv.z) * speed * dt;
}

/// The bounding sphere of placement `i` (centre, radius), world space.
pub fn placement_sphere(world: &World, i: usize) -> Option<(Vec3, f32)> {
    let p = world.placements.get(i)?;
    let s = world.solids.get(p.solid)?;
    let (lo, hi) = (Vec3::from_array(s.bbox_min), Vec3::from_array(s.bbox_max));
    let centre = p.transform.transform_point3((lo + hi) * 0.5);
    Some((centre, (hi - lo).length() * 0.5))
}

/// The placement the ray from `eye` along `dir` meets first: shown,
/// with a matrix, at most [`MAX_PICK_RADIUS`] and not around the eye.
pub fn pick(world: &World, eye: Vec3, dir: Vec3) -> Option<usize> {
    let dir = dir.normalize_or_zero();
    let mut best: Option<(usize, f32)> = None;
    for (i, p) in world.placements.iter().enumerate() {
        if p.flags & (crate::world::HIDDEN_FLAG | 0x0100) != 0 {
            continue;
        }
        let Some((c, r)) = placement_sphere(world, i) else { continue };
        if r > MAX_PICK_RADIUS || r < 0.05 {
            continue;
        }
        let oc = c - eye;
        if oc.length() < r {
            continue;
        }
        let t = oc.dot(dir);
        if t < 0.0 || t > PICK_RANGE {
            continue;
        }
        let d2 = oc.length_squared() - t * t;
        if d2 > r * r {
            continue;
        }
        let enter = t - (r * r - d2).sqrt();
        if best.is_none_or(|(_, b)| enter < b) {
            best = Some((i, enter));
        }
    }
    best.map(|b| b.0)
}

// ---- prop operations ----------------------------------------------------

/// What this mod does to a placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropState {
    /// As the original has it.
    Original,
    /// Moved or turned by this mod.
    Moved,
    /// Hidden by this mod.
    Deleted,
    /// Added by this mod (its edit's index in `MapMod::props`).
    Added(usize),
    /// Added by another mod.
    OtherMod,
}

impl PropState {
    pub fn name(self) -> &'static str {
        match self {
            PropState::Original => "original",
            PropState::Moved => "moved",
            PropState::Deleted => "deleted",
            PropState::Added(_) => "added",
            PropState::OtherMod => "other mod",
        }
    }
}

/// The id of placement `i`'s add when this mod (`folder`) added it.
pub fn own_add<'w>(world: &'w World, folder: &str, i: usize) -> Option<&'w str> {
    world.map_overlay.added.get(&i).filter(|(f, _)| f == folder).map(|(_, id)| id.as_str())
}

/// The index in `map.props` of this mod's add of placement `i`.
fn own_add_edit(map: &MapMod, folder: &str, world: &World, i: usize) -> Option<usize> {
    map.add_index(own_add(world, folder, i)?)
}

/// The index in `map.props` of this mod's edit of the original `target`.
fn edit_of(map: &MapMod, target: &PropRef) -> Option<usize> {
    map.props.iter().position(|e| e.target() == Some(target))
}

/// What this mod (`map`, kept in `folder`) does to placement `i`.
pub fn prop_state(map: &MapMod, folder: &str, world: &World, region: &str, i: usize) -> PropState {
    if let Some(k) = own_add_edit(map, folder, world, i) {
        return PropState::Added(k);
    }
    if world.map_overlay.added.contains_key(&i) {
        return PropState::OtherMod;
    }
    let Some(target) = world.placement_ref(region, i) else { return PropState::Original };
    match edit_of(map, &target).map(|k| &map.props[k]) {
        Some(PropEdit::Move { .. }) => PropState::Moved,
        Some(PropEdit::Delete { .. }) => PropState::Deleted,
        _ => PropState::Original,
    }
}

/// Placement `i`'s position and rotation (rows: local axes).
pub fn placement_parts(world: &World, i: usize) -> Option<(Vec3, Mat3)> {
    world.placements.get(i).map(|p| parts_of(p.transform))
}

/// Put placement `i` at `position` with `rotation` (rows: local axes):
/// in the world, and in the mod as a move of the original (replacing a
/// delete of it, which shows it again) or as the new place of a prop this
/// mod added.
pub fn set_transform(
    map: &mut MapMod,
    folder: &str,
    world: &mut World,
    region: &str,
    i: usize,
    position: Vec3,
    rotation: Mat3,
) -> Result<(), Refused> {
    if i >= world.placements.len() {
        return Err(Refused::NoPlacement);
    }
    if own_add(world, folder, i).is_some() {
        if let Some(PropEdit::Add { position: p, rotation: r, .. }) =
            own_add_edit(map, folder, world, i).and_then(|k| map.props.get_mut(k))
        {
            (*p, *r) = (position, rotation);
        }
    } else if world.map_overlay.added.contains_key(&i) {
        return Err(Refused::OtherMod);
    } else {
        let target = world.placement_ref(region, i).ok_or(Refused::NoPlacement)?;
        world.note_original(i, &target);
        let was_deleted = matches!(edit_of(map, &target).map(|k| &map.props[k]), Some(PropEdit::Delete { .. }));
        map.props.retain(|e| e.target() != Some(&target));
        map.props.push(PropEdit::Move { target, position, rotation });
        if was_deleted {
            world.show_placement(i);
        }
    }
    world.set_placement_transform(i, transform_of(position, rotation));
    Ok(())
}

/// Delete placement `i`: hidden in the world; in the mod a delete of the
/// original (replacing a move of it), or this mod's add of it removed.
pub fn delete(map: &mut MapMod, folder: &str, world: &mut World, region: &str, i: usize) -> Result<(), Refused> {
    if i >= world.placements.len() {
        return Err(Refused::NoPlacement);
    }
    if own_add(world, folder, i).is_some() {
        if let Some(k) = own_add_edit(map, folder, world, i) {
            map.props.remove(k);
        }
        world.map_overlay.added.remove(&i);
    } else if world.map_overlay.added.contains_key(&i) {
        return Err(Refused::OtherMod);
    } else {
        let target = world.placement_ref(region, i).ok_or(Refused::NoPlacement)?;
        world.note_original(i, &target);
        map.props.retain(|e| e.target() != Some(&target));
        map.props.push(PropEdit::Delete { target });
    }
    world.hide_placement(i);
    Ok(())
}

/// How far a reset went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reset {
    /// The placement is as the original has it.
    Exact,
    /// Only its position is back; the rest comes with the next load (no
    /// original world to take it from).
    AfterReload,
}

/// Put placement `i` back as the original has it: this mod's edit of it
/// removed, and, with the original world `base` (the same region with
/// no mod), its transform and visibility taken from there.
pub fn reset(
    map: &mut MapMod,
    folder: &str,
    world: &mut World,
    base: Option<&World>,
    i: usize,
) -> Result<Reset, Refused> {
    if i >= world.placements.len() {
        return Err(Refused::NoPlacement);
    }
    if own_add(world, folder, i).is_some() {
        return Err(Refused::Added);
    }
    if world.map_overlay.added.contains_key(&i) {
        return Err(Refused::OtherMod);
    }
    let target = world.map_overlay.originals.get(&i).cloned().ok_or(Refused::NotEdited)?;
    let before = map.props.len();
    map.props.retain(|e| e.target() != Some(&target));
    if map.props.len() == before {
        return Err(Refused::NotEdited);
    }
    match base.and_then(|b| b.placements.get(i)) {
        Some(p) => {
            let (t, hidden) = (p.transform, p.flags & crate::world::HIDDEN_FLAG != 0);
            world.set_placement_transform(i, t);
            if !hidden {
                world.show_placement(i);
            }
            Ok(Reset::Exact)
        }
        None => {
            let (_, rot) = parts_of(world.placements[i].transform);
            world.set_placement_transform(i, transform_of(target.at, rot));
            Ok(Reset::AfterReload)
        }
    }
}

/// What [`add`] made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Added {
    /// The add edit's index in `MapMod::props`.
    pub edit: usize,
    /// The new placement's index in the world (None when no world was
    /// given: it appears with the next load).
    pub placement: Option<usize>,
}

/// A new placement of the solid named `solid` at `position` with
/// `rotation`: an add edit in the mod and, with a world, the placement in
/// it at once (a renderer draws it once it is built again).
pub fn add(
    map: &mut MapMod,
    folder: &str,
    world: Option<&mut World>,
    region: &str,
    solid: &str,
    position: Vec3,
    rotation: Mat3,
) -> Result<Added, Refused> {
    let edit = map.props.len();
    let id = map.fresh_add_id();
    let placement = match world {
        Some(w) => {
            let i = w.add_placement(solid, transform_of(position, rotation)).ok_or(Refused::NoSolid)?;
            w.map_overlay.first_added.get_or_insert(i);
            w.map_overlay.added.insert(i, (folder.to_string(), id.clone()));
            Some(i)
        }
        None => None,
    };
    map.props.push(PropEdit::Add { id, region: region.into(), solid: solid.into(), position, rotation });
    Ok(Added { edit, placement })
}

/// A copy of placement `i` (its solid and rotation) at `position`, in the
/// mod and in the world.
pub fn duplicate(
    map: &mut MapMod,
    folder: &str,
    world: &mut World,
    region: &str,
    i: usize,
    position: Vec3,
) -> Result<Added, Refused> {
    let p = world.placements.get(i).ok_or(Refused::NoPlacement)?;
    let solid = world.solids.get(p.solid).ok_or(Refused::NoSolid)?.name.clone();
    let (_, rotation) = parts_of(p.transform);
    add(map, folder, Some(world), region, &solid, position, rotation)
}

/// The lowest point (world z) of solid `solid`'s box under `position` /
/// `rotation`, and the box's centre (world).
fn box_bottom(world: &World, solid: usize, position: Vec3, rotation: Mat3) -> Option<(f32, Vec3)> {
    let s = world.solids.get(solid)?;
    let (lo, hi) = (Vec3::from_array(s.bbox_min), Vec3::from_array(s.bbox_max));
    let t = transform_of(position, rotation);
    let mut low = f32::INFINITY;
    for k in 0..8 {
        let c = Vec3::new(
            if k & 1 == 0 { lo.x } else { hi.x },
            if k & 2 == 0 { lo.y } else { hi.y },
            if k & 4 == 0 { lo.z } else { hi.z },
        );
        low = low.min(t.transform_point3(c).z);
    }
    Some((low, t.transform_point3((lo + hi) * 0.5)))
}

/// The position at which placement `i`, turned as `rotation`, stands on
/// the ground under `position` (its box's lowest point on the floor
/// below the box's top, else the highest floor at its x/y). None with no
/// floor there.
pub fn on_ground(world: &World, i: usize, position: Vec3, rotation: Mat3) -> Option<Vec3> {
    let p = world.placements.get(i)?;
    let (low, centre) = box_bottom(world, p.solid, position, rotation)?;
    let top = 2.0 * centre.z - low;
    let floor = world.grid_ground(Vec3::new(centre.x, centre.y, top.max(low + 0.5)))?;
    Some(position + Vec3::Z * (floor - low))
}

/// How far placement `i`'s lowest point is above the floor under it
/// (negative: in it). None with no floor there.
pub fn ground_gap(world: &World, i: usize) -> Option<f32> {
    let (position, rotation) = placement_parts(world, i)?;
    on_ground(world, i, position, rotation).map(|g| position.z - g.z)
}

/// Drop placement `i` onto the floor under it (see [`on_ground`]); its
/// new position.
pub fn drop_to_ground(
    map: &mut MapMod,
    folder: &str,
    world: &mut World,
    region: &str,
    i: usize,
) -> Result<Option<Vec3>, Refused> {
    let (position, rotation) = placement_parts(world, i).ok_or(Refused::NoPlacement)?;
    let Some(to) = on_ground(world, i, position, rotation) else { return Ok(None) };
    set_transform(map, folder, world, region, i, to, rotation)?;
    Ok(Some(to))
}

/// The first point at which the ray from `eye` along `dir` meets a floor
/// (1 m steps over at most `range` metres).
pub fn ray_ground(world: &World, eye: Vec3, dir: Vec3, range: f32) -> Option<Vec3> {
    let f = dir.normalize_or_zero();
    if f == Vec3::ZERO {
        return None;
    }
    let mut p = eye;
    let mut walked = 0.0;
    while walked < range {
        p += f;
        walked += 1.0;
        if let Some(z) = world.grid_ground(p + Vec3::Z * 0.5)
            && z >= p.z - 0.5
            && (z - p.z).abs() < 3.0
        {
            return Some(p.truncate().extend(z));
        }
    }
    None
}

/// The heading (degrees around +z from +x, the game's convention) of a
/// rotation: where its local x axis points.
pub fn heading_of(rotation: Mat3) -> f32 {
    let x = rotation.row(0);
    x.y.atan2(x.x).to_degrees()
}

/// `rotation` turned about the world's z axis to `heading` degrees (its
/// tilt kept).
pub fn with_heading(rotation: Mat3, heading: f32) -> Mat3 {
    turned(rotation, heading - heading_of(rotation))
}

/// `rotation` turned by `degrees` about the world's z axis
/// (counter-clockwise seen from above).
pub fn turned(rotation: Mat3, degrees: f32) -> Mat3 {
    // Rows are the local axes: each turns as a world vector, R' = R Rzᵀ.
    rotation * Mat3::from_rotation_z(-degrees.to_radians())
}

/// An upright rotation with the given heading.
pub fn upright(heading: f32) -> Mat3 {
    let (s, c) = heading.to_radians().sin_cos();
    Mat3::from_cols(Vec3::new(c, -s, 0.0), Vec3::new(s, c, 0.0), Vec3::Z)
}

/// How far a rotation leans: its local z axis' angle from the world's z
/// towards x and towards y, degrees.
pub fn tilt_of(rotation: Mat3) -> Vec2 {
    let z = rotation.row(2);
    Vec2::new(z.x.atan2(z.z).to_degrees(), z.y.atan2(z.z).to_degrees())
}

/// `v` to the nearest multiple of `step` (unchanged for a step ≤ 0).
pub fn snap(v: f32, step: f32) -> f32 {
    if step > 0.0 { (v / step).round() * step } else { v }
}

/// Each coordinate of `v` snapped to `step`.
pub fn snap3(v: Vec3, step: f32) -> Vec3 {
    Vec3::new(snap(v.x, step), snap(v.y, step), snap(v.z, step))
}

/// The editor.
#[derive(Clone, Debug)]
pub struct Editor {
    /// The mod being edited (its folder under `maps/`).
    pub folder: String,
    pub map: MapMod,
    /// Changed since the last save.
    pub dirty: bool,
    /// Added props wait for the world to be loaded again.
    pub needs_reload: bool,
    pub page: Page,
    pub camera: Camera,
    /// The picked placement.
    pub selected: Option<usize>,
    /// The race being edited (index into `map.races`) and its part.
    pub race: Option<usize>,
    pub part: RacePart,
    /// The checkpoint / grid slot selected within the part.
    pub point: usize,
    /// The last message and its kind (true: a warning).
    pub status: Option<(String, bool)>,
}

impl Editor {
    pub fn new(folder: &str, map: MapMod, camera: Camera) -> Editor {
        let race = (!map.races.is_empty()).then_some(0);
        Editor {
            folder: folder.into(),
            map,
            dirty: false,
            needs_reload: false,
            page: Page::Props,
            camera,
            selected: None,
            race,
            part: RacePart::Checkpoint,
            point: 0,
            status: None,
        }
    }

    fn say(&mut self, text: impl Into<String>, warn: bool) {
        self.status = Some((text.into(), warn));
    }

    /// The point the camera looks at on the ground (the world's floor
    /// along the view ray, else 15 m ahead).
    pub fn aim(&self, world: &World) -> Vec3 {
        let f = self.camera.forward();
        ray_ground(world, self.camera.position, f, 200.0).unwrap_or_else(|| {
            let at = self.camera.position + f * 15.0;
            at.truncate().extend(world.grid_ground(at).unwrap_or(at.z))
        })
    }

    /// The camera's heading, degrees (the game's: around +z from +x).
    pub fn heading(&self) -> f32 {
        let f = self.camera.forward();
        f.y.atan2(f.x).to_degrees()
    }

    // ---- props --------------------------------------------------------

    /// Pick the placement in the middle of the view.
    pub fn pick(&mut self, world: &World) {
        self.selected = pick(world, self.camera.position, self.camera.forward());
        match self.selected.and_then(|i| world.solids.get(world.placements[i].solid)) {
            Some(s) => {
                let text = format!("Picked {}", s.name);
                self.say(text, false)
            }
            None => self.say("Nothing to pick there", true),
        }
    }

    /// Move the picked placement by `by` and turn it by `turn` degrees
    /// about z.
    pub fn nudge(&mut self, world: &mut World, region: &str, by: Vec3, turn: f32) {
        let Some(i) = self.selected else { return };
        let Some((pos, rot)) = placement_parts(world, i) else { return };
        let rot = if turn != 0.0 { turned(rot, -turn) } else { rot };
        match set_transform(&mut self.map, &self.folder, world, region, i, pos + by, rot) {
            Ok(()) => self.dirty = true,
            Err(e) => self.say(e.to_string(), true),
        }
    }

    /// Delete the picked placement.
    pub fn delete(&mut self, world: &mut World, region: &str) {
        let Some(i) = self.selected.take() else { return };
        match delete(&mut self.map, &self.folder, world, region, i) {
            Ok(()) => {
                self.dirty = true;
                self.say("Deleted", false);
            }
            Err(e) => self.say(e.to_string(), true),
        }
    }

    /// Put the picked placement back as the original has it (this mod's
    /// edit of it removed). A deleted one comes back after a reload.
    pub fn restore(&mut self, world: &mut World) {
        let Some(i) = self.selected else { return };
        match reset(&mut self.map, &self.folder, world, None, i) {
            Ok(_) => {
                self.needs_reload = true;
                self.dirty = true;
                self.say("Restored (exactly so after Reload)", false);
            }
            Err(Refused::NotEdited) if world.map_overlay.originals.contains_key(&i) => {
                self.say("Changed by another map mod", true)
            }
            Err(Refused::NotEdited) => self.say("Not changed by a map mod", true),
            Err(e) => self.say(e.to_string(), true),
        }
    }

    /// A copy of the picked placement's solid at the aim point (drawn
    /// after the world is loaded again).
    pub fn duplicate(&mut self, world: &World, region: &str) {
        let Some(i) = self.selected else {
            self.say("Pick a prop to copy first", true);
            return;
        };
        let p = &world.placements[i];
        let solid = world.solids[p.solid].name.clone();
        let (_, rotation) = parts_of(p.transform);
        let at = self.aim(world);
        match add(&mut self.map, &self.folder, None, region, &solid, at, rotation) {
            Ok(_) => {
                self.dirty = true;
                self.needs_reload = true;
                self.say(format!("Added {solid}: shown after Reload"), false);
            }
            Err(e) => self.say(e.to_string(), true),
        }
    }

    // ---- races --------------------------------------------------------

    pub fn current_race(&mut self) -> Option<&mut CustomRace> {
        self.race.and_then(|r| self.map.races.get_mut(r))
    }

    /// A new race on base track `track` starting at the aim point.
    pub fn new_race(&mut self, world: &World, track: u16, kind: RaceKind) {
        let mut n = self.map.races.len() + 1;
        while self.map.races.iter().any(|r| r.id == format!("race-{n}")) {
            n += 1;
        }
        let mut r = CustomRace::new(&format!("race-{n}"), track, kind);
        r.name = format!("Race {n}");
        let at = self.aim(world);
        r.start = Some(Spot { position: at, heading: self.heading() });
        r.checkpoints.push(at.truncate());
        self.map.races.push(r);
        self.race = Some(self.map.races.len() - 1);
        (self.part, self.point) = (RacePart::Checkpoint, 0);
        self.dirty = true;
        self.say(format!("New race on track {track}: add checkpoints with Enter"), false);
    }

    /// Enter on the race page: a checkpoint or grid slot added after the
    /// selected one, or the start / finish line set, at the aim point.
    pub fn place(&mut self, world: &World) {
        let at = self.aim(world);
        let heading = self.heading();
        let (part, point) = (self.part, self.point);
        let Some(r) = self.current_race() else {
            self.say("Make a race first (N)", true);
            return;
        };
        let msg = match part {
            RacePart::Checkpoint => {
                if r.checkpoints.len() >= super::format::MAX_CHECKPOINTS {
                    "Too many checkpoints".to_string()
                } else {
                    let k = (point + 1).min(r.checkpoints.len());
                    r.checkpoints.insert(k, at.truncate());
                    self.point = k;
                    format!("Checkpoint {}", k + 1)
                }
            }
            RacePart::Grid => {
                if r.grid.len() >= super::format::MAX_GRID {
                    "The grid is full (8)".to_string()
                } else {
                    let k = (point + 1).min(r.grid.len());
                    r.grid.insert(k, Spot { position: at, heading });
                    self.point = k;
                    format!("Grid slot {}", k + 1)
                }
            }
            RacePart::Start => {
                r.start = Some(Spot { position: at, heading });
                "Start line set".to_string()
            }
            RacePart::Finish => {
                r.finish = Some(Spot { position: at, heading });
                "Finish line set".to_string()
            }
        };
        self.dirty = true;
        self.say(msg, false);
    }

    /// Delete on the race page: the selected checkpoint or grid slot, or
    /// the start / finish marker.
    pub fn remove_point(&mut self) {
        let (part, point) = (self.part, self.point);
        let Some(r) = self.current_race() else { return };
        match part {
            RacePart::Checkpoint if point < r.checkpoints.len() => {
                r.checkpoints.remove(point);
            }
            RacePart::Grid if point < r.grid.len() => {
                r.grid.remove(point);
            }
            RacePart::Start => r.start = None,
            RacePart::Finish => r.finish = None,
            _ => return,
        }
        self.point = self.point.saturating_sub(1);
        self.dirty = true;
    }

    /// Move the selected checkpoint / grid slot / marker by `by` (x, y).
    pub fn move_point(&mut self, by: Vec2) {
        let (part, point) = (self.part, self.point);
        let Some(r) = self.current_race() else { return };
        match part {
            RacePart::Checkpoint => {
                if let Some(c) = r.checkpoints.get_mut(point) {
                    *c += by;
                }
            }
            RacePart::Grid => {
                if let Some(g) = r.grid.get_mut(point) {
                    g.position += by.extend(0.0);
                }
            }
            RacePart::Start => {
                if let Some(s) = r.start.as_mut() {
                    s.position += by.extend(0.0);
                }
            }
            RacePart::Finish => {
                if let Some(s) = r.finish.as_mut() {
                    s.position += by.extend(0.0);
                }
            }
        }
        self.dirty = true;
    }

    /// The selected point's position (for the camera to jump to).
    pub fn point_position(&self) -> Option<Vec3> {
        let r = self.map.races.get(self.race?)?;
        match self.part {
            RacePart::Checkpoint => {
                r.checkpoints.get(self.point).map(|c| c.extend(r.start.map_or(0.0, |s| s.position.z)))
            }
            RacePart::Grid => r.grid.get(self.point).map(|g| g.position),
            RacePart::Start => r.start.map(|s| s.position),
            RacePart::Finish => r.finish.map(|s| s.position),
        }
    }

    /// The race mode cycled.
    pub fn cycle_kind(&mut self) {
        let Some(r) = self.current_race() else { return };
        let k = RaceKind::ALL.iter().position(|k| *k == r.kind).unwrap_or(0);
        r.kind = RaceKind::ALL[(k + 1) % RaceKind::ALL.len()];
        if !r.kind.loops() {
            r.laps = 1;
        } else if r.laps < 2 {
            r.laps = 2;
        }
        self.dirty = true;
    }

    /// Laps changed by `d` (circuits only).
    pub fn laps(&mut self, d: i32) {
        let Some(r) = self.current_race() else { return };
        if r.kind.loops() {
            r.laps = (r.laps as i32 + d).clamp(1, super::format::MAX_LAPS as i32) as u32;
            self.dirty = true;
        }
    }

    /// Delete the current race.
    pub fn delete_race(&mut self) {
        let Some(k) = self.race else { return };
        self.map.races.remove(k);
        self.race = (!self.map.races.is_empty()).then(|| k.min(self.map.races.len() - 1));
        self.point = 0;
        self.dirty = true;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use glam::{Mat3, Mat4, Vec3};

    use super::*;
    use crate::world::{HIDDEN_FLAG, Placement, World};

    const REGION: &str = "L4RA";
    const FOLDER: &str = "my-map";

    fn solid(name: &str) -> crate::formats::solid::Solid {
        crate::formats::solid::Solid {
            name: name.into(),
            hash: crate::formats::hash::bin_hash(name),
            bbox_min: [-0.5, -0.5, 0.0],
            bbox_max: [0.5, 0.5, 1.0],
            textures: Vec::new(),
            vertices: Vec::new(),
            normals: Vec::new(),
            light_materials: Vec::new(),
            indices: Vec::new(),
            materials: Vec::new(),
            mesh_flags: 0,
            markers: Vec::new(),
            light_markers: Vec::new(),
        }
    }

    /// A flat floor at z = 0 with two cones standing on it and a barrier
    /// 2 m above it.
    pub(crate) fn world() -> World {
        let mut w = World::flat(200.0);
        w.solids = vec![solid("XC_CONE_01"), solid("XC_BARRIER_03")];
        let place = |solid, at: Vec3| Placement {
            solid,
            low_solid: None,
            mid_solid: None,
            transform: Mat4::from_translation(at),
            section: 618,
            flags: 0,
            cull_radius: 5.0,
        };
        w.placements = vec![
            place(0, Vec3::new(10.0, 0.0, 0.0)),
            place(0, Vec3::new(20.0, 0.0, 0.0)),
            place(1, Vec3::new(30.0, 0.0, 2.0)),
        ];
        w
    }

    fn at(w: &World, i: usize) -> Vec3 {
        w.placements[i].transform.w_axis.truncate()
    }

    #[test]
    fn moving_an_original_writes_one_move_and_keeps_its_name() {
        let (mut m, mut w) = (MapMod::default(), world());
        let original = w.placement_ref(REGION, 0).unwrap();
        assert_eq!(prop_state(&m, FOLDER, &w, REGION, 0), PropState::Original);
        set_transform(&mut m, FOLDER, &mut w, REGION, 0, Vec3::new(11.0, 1.0, 0.0), Mat3::IDENTITY).unwrap();
        set_transform(&mut m, FOLDER, &mut w, REGION, 0, Vec3::new(12.0, 1.0, 0.0), Mat3::IDENTITY).unwrap();
        assert_eq!(at(&w, 0), Vec3::new(12.0, 1.0, 0.0));
        // One edit per placement, naming the original's place.
        match m.props.as_slice() {
            [PropEdit::Move { target, position, .. }] => {
                assert_eq!(*target, original);
                assert_eq!(*position, Vec3::new(12.0, 1.0, 0.0));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(prop_state(&m, FOLDER, &w, REGION, 0), PropState::Moved);
        assert!(w.take_changed().contains(&0));
        // The world with the mod applied from scratch agrees.
        let mut again = world();
        again.apply_map_mods(REGION, &[(FOLDER.into(), m.clone())]);
        assert_eq!(at(&again, 0), at(&w, 0));
    }

    #[test]
    fn deleting_replaces_a_move_and_moving_brings_it_back() {
        let (mut m, mut w) = (MapMod::default(), world());
        set_transform(&mut m, FOLDER, &mut w, REGION, 1, Vec3::new(21.0, 0.0, 0.0), Mat3::IDENTITY).unwrap();
        delete(&mut m, FOLDER, &mut w, REGION, 1).unwrap();
        assert!(matches!(m.props.as_slice(), [PropEdit::Delete { .. }]));
        assert!(w.placements[1].flags & HIDDEN_FLAG != 0);
        assert_eq!(prop_state(&m, FOLDER, &w, REGION, 1), PropState::Deleted);
        set_transform(&mut m, FOLDER, &mut w, REGION, 1, Vec3::new(22.0, 0.0, 0.0), Mat3::IDENTITY).unwrap();
        assert!(matches!(m.props.as_slice(), [PropEdit::Move { .. }]));
        assert!(w.placements[1].flags & HIDDEN_FLAG == 0, "shown again");
    }

    #[test]
    fn reset_with_the_original_world_is_exact() {
        let base = world();
        let (mut m, mut w) = (MapMod::default(), world());
        let turned_rot = with_heading(Mat3::IDENTITY, 90.0);
        set_transform(&mut m, FOLDER, &mut w, REGION, 0, Vec3::new(5.0, 5.0, 1.0), turned_rot).unwrap();
        delete(&mut m, FOLDER, &mut w, REGION, 1).unwrap();
        assert_eq!(reset(&mut m, FOLDER, &mut w, Some(&base), 0), Ok(Reset::Exact));
        assert_eq!(w.placements[0].transform, base.placements[0].transform);
        assert_eq!(reset(&mut m, FOLDER, &mut w, Some(&base), 1), Ok(Reset::Exact));
        assert!(w.placements[1].flags & HIDDEN_FLAG == 0);
        assert!(m.props.is_empty());
        assert_eq!(reset(&mut m, FOLDER, &mut w, Some(&base), 0), Err(Refused::NotEdited));
        assert_eq!(reset(&mut m, FOLDER, &mut w, Some(&base), 99), Err(Refused::NoPlacement));
        // Without the original world: the position now, the rest on reload.
        set_transform(&mut m, FOLDER, &mut w, REGION, 0, Vec3::new(5.0, 5.0, 1.0), turned_rot).unwrap();
        assert_eq!(reset(&mut m, FOLDER, &mut w, None, 0), Ok(Reset::AfterReload));
        assert_eq!(at(&w, 0), at(&base, 0));
    }

    #[test]
    fn added_and_duplicated_props_are_in_the_world_at_once() {
        let (mut m, mut w) = (MapMod::default(), world());
        let n = w.placements.len();
        let a = duplicate(&mut m, FOLDER, &mut w, REGION, 0, Vec3::new(40.0, 0.0, 0.0)).unwrap();
        assert_eq!(a, Added { edit: 0, placement: Some(n) });
        assert_eq!(w.placements[n].solid, 0);
        assert_eq!(prop_state(&m, FOLDER, &w, REGION, n), PropState::Added(0));
        let b = add(&mut m, FOLDER, Some(&mut w), REGION, "XC_BARRIER_03", Vec3::ZERO, Mat3::IDENTITY).unwrap();
        assert_eq!(b.placement, Some(n + 1));
        assert_eq!(
            add(&mut m, FOLDER, Some(&mut w), REGION, "NO_SUCH", Vec3::ZERO, Mat3::IDENTITY),
            Err(Refused::NoSolid)
        );
        // Moving an added prop changes its add edit, not a move.
        set_transform(&mut m, FOLDER, &mut w, REGION, n, Vec3::new(41.0, 0.0, 0.0), Mat3::IDENTITY).unwrap();
        assert!(matches!(&m.props[0], PropEdit::Add { position, .. } if *position == Vec3::new(41.0, 0.0, 0.0)));
        assert_eq!(reset(&mut m, FOLDER, &mut w, None, n), Err(Refused::Added));
        // Deleting the first add keeps the second's id; the file names it.
        assert_eq!(w.map_overlay.added.get(&(n + 1)), Some(&(FOLDER.to_string(), "a2".to_string())));
        delete(&mut m, FOLDER, &mut w, REGION, n).unwrap();
        assert_eq!(m.props.len(), 1);
        assert_eq!(w.map_overlay.added.get(&(n + 1)), Some(&(FOLDER.to_string(), "a2".to_string())));
        assert_eq!(prop_state(&m, FOLDER, &w, REGION, n + 1), PropState::Added(0));
        assert!(m.write().contains("[prop.add]\nid = a2\n"));
        assert_eq!(MapMod::parse(&m.write()).unwrap(), m);
        // The next add takes an id no add has.
        let c = add(&mut m, FOLDER, Some(&mut w), REGION, "XC_CONE_01", Vec3::ONE, Mat3::IDENTITY).unwrap();
        assert_eq!(m.props[c.edit].add_id(), Some("a3"));
        // The mod applied from scratch gives the same added placement.
        let mut again = world();
        again.apply_map_mods(REGION, &[(FOLDER.into(), m.clone())]);
        assert_eq!(again.placements.len(), n + 2);
        assert_eq!(again.placements[n].solid, 1);
        assert_eq!(again.map_overlay.placement_of_add(FOLDER, "a2"), Some(n));
        assert_eq!(again.map_overlay.placement_of_add(FOLDER, "a3"), Some(n + 1));
    }

    #[test]
    fn another_mods_prop_is_left_alone() {
        let (mut m, mut w) = (MapMod::default(), world());
        let other =
            add(&mut MapMod::default(), "other", Some(&mut w), REGION, "XC_CONE_01", Vec3::ZERO, Mat3::IDENTITY)
                .unwrap()
                .placement
                .unwrap();
        assert_eq!(prop_state(&m, FOLDER, &w, REGION, other), PropState::OtherMod);
        assert_eq!(
            set_transform(&mut m, FOLDER, &mut w, REGION, other, Vec3::ONE, Mat3::IDENTITY),
            Err(Refused::OtherMod)
        );
        assert_eq!(delete(&mut m, FOLDER, &mut w, REGION, other), Err(Refused::OtherMod));
        assert!(m.props.is_empty());
    }

    #[test]
    fn drop_to_ground_stands_the_box_on_the_floor() {
        let (mut m, mut w) = (MapMod::default(), world());
        // The barrier floats 2 m up.
        assert!((ground_gap(&w, 2).unwrap() - 2.0).abs() < 1e-4);
        let to = drop_to_ground(&mut m, FOLDER, &mut w, REGION, 2).unwrap().unwrap();
        assert!(to.z.abs() < 1e-4, "{to}");
        assert!(ground_gap(&w, 2).unwrap().abs() < 1e-4);
        // Upside down, the box hangs below its origin: the origin goes up.
        let flipped = Mat3::from_diagonal(Vec3::new(1.0, -1.0, -1.0));
        let up = on_ground(&w, 0, Vec3::new(10.0, 0.0, 0.0), flipped).unwrap();
        assert!((up.z - 1.0).abs() < 1e-4, "{up}");
        // Off the floor: nothing to stand on.
        assert_eq!(on_ground(&w, 0, Vec3::new(1000.0, 0.0, 0.0), Mat3::IDENTITY), None);
    }

    #[test]
    fn headings_turn_about_z_and_keep_tilt() {
        for h in [-170.0f32, -45.0, 0.0, 15.0, 90.0, 179.0] {
            let r = with_heading(Mat3::IDENTITY, h);
            assert!((heading_of(r) - h).abs() < 1e-3, "{h}: {}", heading_of(r));
            assert!((heading_of(upright(h)) - h).abs() < 1e-3);
            assert!(r.abs_diff_eq(upright(h), 1e-5));
        }
        // Turning by 90° counter-clockwise sends local x from +x to +y.
        let r = turned(Mat3::IDENTITY, 90.0);
        assert!(r.row(0).abs_diff_eq(Vec3::Y, 1e-6));
        // A tilted rotation keeps its tilt when turned.
        let tilted = Mat3::from_rotation_x(0.2).transpose();
        let t = tilt_of(tilted);
        let r = with_heading(tilted, 60.0);
        assert!((heading_of(r) - 60.0).abs() < 1e-3);
        assert!((r.row(2).z - tilted.row(2).z).abs() < 1e-6);
        assert!(t.length() > 5.0 && tilt_of(upright(30.0)).length() < 1e-4);
    }

    #[test]
    fn snapping() {
        assert_eq!(snap(1.13, 0.25), 1.25);
        assert_eq!(snap(-0.37, 0.25), -0.25);
        assert_eq!(snap(1.13, 0.0), 1.13);
        assert_eq!(snap(22.0, 15.0), 15.0);
        assert_eq!(snap3(Vec3::new(0.1, 0.2, 0.4), 0.25), Vec3::new(0.0, 0.25, 0.5));
    }

    #[test]
    fn a_ray_meets_the_floor() {
        let w = world();
        let hit = ray_ground(&w, Vec3::new(0.0, 0.0, 10.0), Vec3::new(1.0, 0.0, -1.0), 100.0).unwrap();
        assert!((hit.x - 10.0).abs() < 1.5 && hit.z.abs() < 1e-4, "{hit}");
        assert_eq!(ray_ground(&w, Vec3::new(0.0, 0.0, 10.0), Vec3::Z, 100.0), None);
    }
}
