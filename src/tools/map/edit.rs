//! The map editor's state and edits, without the window or the GPU (the
//! game's side: `src/bin/game/enh/map_editor`). The edits change the world
//! the player is in at once (moved and hidden placements go to the
//! renderer through `World::take_changed`) and the mod being edited; an
//! added prop is drawn after the world is loaded again (the renderer
//! builds its instances once per load).

use glam::{Mat3, Vec2, Vec3};

use super::format::{CustomRace, MapMod, PropEdit, RaceKind, Spot};
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

/// What a race edit works on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RacePart {
    #[default]
    Checkpoint,
    Grid,
    Start,
    Finish,
}

impl RacePart {
    pub const ALL: [RacePart; 4] = [RacePart::Checkpoint, RacePart::Grid, RacePart::Start, RacePart::Finish];

    pub fn name(self) -> &'static str {
        match self {
            RacePart::Checkpoint => "Checkpoints",
            RacePart::Grid => "Start grid",
            RacePart::Start => "Start line",
            RacePart::Finish => "Finish line",
        }
    }
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
        let mut p = self.camera.position;
        for _ in 0..200 {
            p += f;
            if let Some(z) = world.grid_ground(p + Vec3::Z * 0.5)
                && z >= p.z - 0.5
                && (z - p.z).abs() < 3.0
            {
                return p.truncate().extend(z);
            }
        }
        let at = self.camera.position + f * 15.0;
        at.truncate().extend(world.grid_ground(at).unwrap_or(at.z))
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

    /// The added placement `i`'s edit index in this mod.
    fn own_add(&self, world: &World, i: usize) -> Option<usize> {
        world.map_overlay.added.get(&i).filter(|(f, _)| *f == self.folder).map(|(_, k)| *k)
    }

    /// Move the picked placement by `by` and turn it by `turn` degrees
    /// about z.
    pub fn nudge(&mut self, world: &mut World, region: &str, by: Vec3, turn: f32) {
        let Some(i) = self.selected else { return };
        let (pos, rot) = parts_of(world.placements[i].transform);
        let rot = if turn != 0.0 { rot * Mat3::from_rotation_z(turn.to_radians()) } else { rot };
        let (pos, rot) = (pos + by, rot);
        if let Some(k) = self.own_add(world, i) {
            if let Some(PropEdit::Add { position, rotation, .. }) = self.map.props.get_mut(k) {
                (*position, *rotation) = (pos, rot);
            }
        } else if world.map_overlay.added.contains_key(&i) {
            self.say("This prop belongs to another map mod", true);
            return;
        } else {
            let Some(target) = world.placement_ref(region, i) else { return };
            world.map_overlay.originals.entry(i).or_insert_with(|| target.clone());
            self.map.props.retain(|e| e.target() != Some(&target));
            self.map.props.push(PropEdit::Move { target, position: pos, rotation: rot });
        }
        world.set_placement_transform(i, transform_of(pos, rot));
        self.dirty = true;
    }

    /// Delete the picked placement.
    pub fn delete(&mut self, world: &mut World, region: &str) {
        let Some(i) = self.selected.take() else { return };
        if let Some(k) = self.own_add(world, i) {
            self.map.props.remove(k);
            world.map_overlay.added.remove(&i);
            for (f, kk) in world.map_overlay.added.values_mut() {
                if *f == self.folder && *kk > k {
                    *kk -= 1;
                }
            }
        } else if world.map_overlay.added.contains_key(&i) {
            self.say("This prop belongs to another map mod", true);
            return;
        } else {
            let Some(target) = world.placement_ref(region, i) else { return };
            world.map_overlay.originals.entry(i).or_insert_with(|| target.clone());
            self.map.props.retain(|e| e.target() != Some(&target));
            self.map.props.push(PropEdit::Delete { target });
        }
        world.hide_placement(i);
        self.dirty = true;
        self.say("Deleted", false);
    }

    /// Put the picked placement back as the original has it (this mod's
    /// edit of it removed). A deleted one comes back after a reload.
    pub fn restore(&mut self, world: &mut World) {
        let Some(i) = self.selected else { return };
        let Some(target) = world.map_overlay.originals.get(&i).cloned() else {
            self.say("Not changed by a map mod", true);
            return;
        };
        let before = self.map.props.len();
        self.map.props.retain(|e| e.target() != Some(&target));
        if self.map.props.len() == before {
            self.say("Changed by another map mod", true);
            return;
        }
        let (_, rot) = parts_of(world.placements[i].transform);
        // The original rotation is not kept separately: the position goes
        // back now, the whole placement with the next load.
        world.set_placement_transform(i, transform_of(target.at, rot));
        self.needs_reload = true;
        self.dirty = true;
        self.say("Restored (exactly so after Reload)", false);
    }

    /// A copy of the picked placement's solid at the aim point.
    pub fn duplicate(&mut self, world: &World, region: &str) {
        let Some(i) = self.selected else {
            self.say("Pick a prop to copy first", true);
            return;
        };
        let p = &world.placements[i];
        let solid = world.solids[p.solid].name.clone();
        let (_, rotation) = parts_of(p.transform);
        let position = self.aim(world);
        self.map.props.push(PropEdit::Add { region: region.into(), solid: solid.clone(), position, rotation });
        self.dirty = true;
        self.needs_reload = true;
        self.say(format!("Added {solid}: shown after Reload"), false);
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
