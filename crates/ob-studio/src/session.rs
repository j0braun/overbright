//! One map mod open on one region, without a window: the mod, the world
//! it is shown in, the undo history. [`MapSession::run`] makes one
//! [`Op`] as one undo step; the desktop editor keeps its UI state (camera,
//! selection, tools) next to a session, and the console tools drive one
//! directly.
//!
//! A session opened on a mod's folder keeps its steps in the mod's
//! [`History`] ([`MapSession::keep_in`]): each step is in the journal on
//! the disk before it counts, undo and redo go through it, and
//! [`MapSession::sync`] takes in the steps other programs made. Without one
//! (tests, a mod not on the disk) the steps live in memory ([`Undo`]).

use glam::Vec3;

use nfsu2::studio_api::map::edit;
use nfsu2::studio_api::world::{World, face_key};
use ob_map::tools::map::format::{
    CollisionEdit, CustomRace, MAX_COLLISION, MAX_LAPS, MAX_TEXT, MapMod, PropEdit, Spot, check_laps, check_rotation, valid_id, valid_solid,
};
use ob_map::tools::map::ops::{Change, CollisionKey, Op, PropKey, RacePart, RacePoint, Refused, ensure, max_points, valid_position};
use ob_map::tools::map::race;
use ob_map::tools::map::store;

use crate::history::{History, STUDIO};

/// How many undo steps are kept.
pub const MAX_STEPS: usize = 256;

/// The undo history: whole-mod snapshots (a mod is small), one per step.
#[derive(Clone, Debug, Default)]
pub struct Undo {
    /// (step name, the mod before the step), oldest first.
    past: Vec<(String, MapMod)>,
    /// (step name, the mod after the step), next redo last.
    future: Vec<(String, MapMod)>,
    /// The mod as of the last step.
    current: MapMod,
}

impl Undo {
    pub fn new(map: &MapMod) -> Undo {
        Undo { past: Vec::new(), future: Vec::new(), current: map.clone() }
    }

    /// The name of the step Undo takes back.
    pub fn undo_label(&self) -> Option<&str> {
        self.past.last().map(|p| p.0.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.future.last().map(|p| p.0.as_str())
    }

    /// The mod as of the last step.
    pub fn current(&self) -> &MapMod {
        &self.current
    }

    /// Record `now` as a step after `current`; false when nothing changed.
    pub fn record(&mut self, label: &str, now: &MapMod) -> bool {
        if *now == self.current {
            return false;
        }
        let before = std::mem::replace(&mut self.current, now.clone());
        self.past.push((label.to_string(), before));
        if self.past.len() > MAX_STEPS {
            self.past.remove(0);
        }
        self.future.clear();
        true
    }

    /// Forget the step just recorded (it could not be kept).
    pub fn drop_last(&mut self) {
        if let Some((_, before)) = self.past.pop() {
            self.current = before;
        }
    }

    /// The mod one step back, if any.
    pub fn undo(&mut self) -> Option<MapMod> {
        let (label, before) = self.past.pop()?;
        let after = std::mem::replace(&mut self.current, before.clone());
        self.future.push((label, after));
        Some(before)
    }

    /// The mod one step forward, if any.
    pub fn redo(&mut self) -> Option<MapMod> {
        let (label, after) = self.future.pop()?;
        let before = std::mem::replace(&mut self.current, after.clone());
        self.past.push((label, before));
        Some(after)
    }
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

/// One map mod open on one region.
pub struct MapSession {
    /// The region being edited ("L4RA").
    pub region: String,
    /// The mod's folder name under `maps/`.
    pub folder: String,
    pub map: MapMod,
    /// The original world as loaded, never edited. Only its placements and
    /// knock-over props are filled (the rest lives once, in `world`).
    pub base: World,
    /// `base` with `map` applied.
    pub world: World,
    pub undo: Undo,
    /// Validation results.
    pub issues: Vec<Issue>,
    /// Changed since the last save.
    pub dirty: bool,
    /// The mod's history on the disk (None: the steps live in `undo`).
    pub history: Option<History>,
    /// Why the last step, undo or redo was not made, or the journal's
    /// file does not read (taken by the window's status bar).
    pub note: Option<String>,
    /// What was wrong with the value of the last op [`run`](Self::run) refused as [`Refused::BadValue`], when
    /// it can say more ("a sprint race runs one lap").
    pub refused_why: Option<String>,
    /// Placements moved or hidden since a viewer last looked
    /// (`world.take_changed()` holds which).
    pub world_changed: bool,
    /// The set of added placements changed: a renderer must be built
    /// again.
    pub adds_changed: bool,
}

fn adds(m: &MapMod) -> Vec<&PropEdit> {
    m.props.iter().filter(|e| matches!(e, PropEdit::Add { .. })).collect()
}

impl MapSession {
    /// A session for `map` (kept in `folder`) on `world`, a region as
    /// loaded without map mods (`World::load`); `map` is applied to it
    /// here. The undo history starts at `map`.
    pub fn new(region: &str, folder: &str, map: MapMod, world: World) -> MapSession {
        let mut base = World::default();
        base.placements = world.placements.clone();
        base.smackables = world.smackables.clone();
        let mut s = MapSession {
            region: region.to_string(),
            folder: folder.to_string(),
            undo: Undo::new(&map),
            history: None,
            note: None,
            refused_why: None,
            map,
            base,
            world,
            issues: Vec::new(),
            dirty: false,
            world_changed: true,
            adds_changed: true,
        };
        s.apply_map();
        s
    }

    /// Run `op` as one undo step named [`Op::label`]. A refused op
    /// changes nothing (a [`Op::Group`] is made whole or not at all); an
    /// op that makes no difference records no step (`changed` false).
    pub fn run(&mut self, op: Op) -> Result<Change, Refused> {
        // Every request is the same JSON the command-line tool, an operations file and an agent send: each one
        // goes through it (a few microseconds), so a request the UI makes and JSON cannot carry fails its tests.
        let op = through_json(op);
        let label = op.label(&self.map);
        let before = self.map.clone();
        let mut change = Change { label, ..Change::default() };
        self.refused_why = None;
        let mut done = self.exec(op, &mut change);
        if done.is_ok()
            && self.map != before
            && let Err(e) = MapMod::parse(&self.map.write())
        {
            // The last guard: the mod file must read back as it is written (the ops check their values
            // against the same rules; this catches any they miss), else the edit would leave a mod that no
            // longer opens.
            self.refused_why = Some(format!("the map file would not read back: {}", e.message));
            done = Err(Refused::BadValue);
        }
        if let Err(e) = done {
            // Live world edits are taken back by applying the mod again
            // (also when the mod came back to what it was: a delete put
            // back by a move leaves it as before, but not the world).
            if self.map != before {
                let changed_adds = adds(&self.map) != adds(&before);
                self.map = before;
                self.apply(changed_adds);
            } else {
                self.settle();
            }
            return Err(e);
        }
        change.changed = self.commit(&change.label);
        Ok(change)
    }

    /// Keep the steps in `history` from now on (the mod's own, opened on
    /// its folder): the session goes to the mod as the history has it,
    /// unless the history holds unsaved steps found on opening
    /// ([`History::pending`]: the session stays at the files until they are
    /// restored or discarded, then [`sync`](Self::sync)).
    pub fn keep_in(&mut self, history: History) {
        self.history = Some(history);
        self.sync();
    }

    /// Record the edit just made to `map` as one undo step named `label`
    /// and apply the mod to the world again; false (and nothing done)
    /// when the mod did not change. [`run`](Self::run) ends in this; the
    /// editor's panels that edit `map` in place call it too. With a
    /// history the step is in its journal before it counts: one the journal
    /// cannot take (read-only, a disk error) is taken back, and `note`
    /// says why.
    pub fn commit(&mut self, label: &str) -> bool {
        let before = self.undo.current().clone();
        let before_adds: Vec<PropEdit> = adds(&before).into_iter().cloned().collect();
        if !self.undo.record(label, &self.map) {
            // The mod is as before, the world maybe not (see `run`).
            self.settle();
            return false;
        }
        let changed_adds = adds(&self.map).into_iter().ne(before_adds.iter());
        if let Some(h) = self.history.as_mut() {
            let docs = [(store::FILE.to_string(), Some(self.map.write()))].into();
            if let Err(e) = h.commit(STUDIO, label, docs) {
                self.note = Some(format!("Not kept: {e}"));
                self.undo.drop_last();
                self.map = before;
                self.apply(changed_adds);
                return false;
            }
        }
        self.mark_dirty();
        self.apply(changed_adds);
        true
    }

    /// The name of the step Undo takes back.
    pub fn undo_label(&self) -> Option<&str> {
        match &self.history {
            Some(h) => h.steps().last().map(|s| s.label.as_str()),
            None => self.undo.undo_label(),
        }
    }

    /// The name of the step Redo makes again.
    pub fn redo_label(&self) -> Option<&str> {
        match &self.history {
            Some(h) => h.undone().last().map(|s| s.label.as_str()),
            None => self.undo.redo_label(),
        }
    }

    /// Take back the last step (with a history: whoever made it); its name.
    pub fn undo(&mut self) -> Option<String> {
        if let Some(h) = self.history.as_mut() {
            let r = h.undo(STUDIO);
            return self.stepped(r);
        }
        let label = self.undo.undo_label()?.to_string();
        let m = self.undo.undo()?;
        self.step_to(m);
        Some(label)
    }

    /// Make the last undone step again; its name.
    pub fn redo(&mut self) -> Option<String> {
        if let Some(h) = self.history.as_mut() {
            let r = h.redo(STUDIO);
            return self.stepped(r);
        }
        let label = self.undo.redo_label()?.to_string();
        let m = self.undo.redo()?;
        self.step_to(m);
        Some(label)
    }

    fn stepped(&mut self, r: crate::history::Result<Option<String>>) -> Option<String> {
        match r {
            Ok(label) => {
                self.sync();
                label
            }
            Err(e) => {
                self.note = Some(format!("Not kept: {e}"));
                None
            }
        }
    }

    /// Go to the mod as the history has it now (after its `tick` took in
    /// another program's steps, an undo or redo, a restore or discard of the
    /// unsaved steps): true when the mod changed. A file the history holds
    /// that does not read as a mod leaves the session as it is, with a
    /// `note`. As the writer, nothing while unsaved steps found on opening
    /// wait for an answer.
    pub fn sync(&mut self) -> bool {
        let Some(h) = self.history.as_ref() else { return false };
        if h.pending().is_some() && h.writable() {
            return false;
        }
        let unsaved = writes(h);
        let m = match h.doc(store::FILE).map(MapMod::parse) {
            None => MapMod::default(),
            Some(Ok(m)) => m,
            Some(Err(e)) => {
                self.note = Some(format!("{} does not read: {e:?}", store::FILE));
                self.dirty = unsaved;
                return false;
            }
        };
        self.undo = Undo::new(&m);
        let changed = m != self.map;
        if changed {
            self.step_to(m);
        }
        self.dirty = unsaved;
        changed
    }

    /// The mod has steps not yet in its file (that this session writes).
    fn mark_dirty(&mut self) {
        self.dirty = self.history.as_ref().is_none_or(writes);
    }

    fn step_to(&mut self, m: MapMod) {
        let changed_adds = adds(&self.map) != adds(&m);
        self.map = m;
        self.mark_dirty();
        self.apply(changed_adds);
    }

    /// Apply `map` to the world again from scratch (opening a mod, a new
    /// one, reverting).
    pub fn apply_map(&mut self) {
        self.apply(true);
    }

    /// Apply `map` to the world again: only what changed since the last
    /// time is redone (`World::reapply_map_mod`); the world is the one a
    /// full apply of `map` to the original makes.
    fn apply(&mut self, adds_changed: bool) {
        self.world.reapply_map_mod(&self.region, &self.folder, &self.map);
        self.world_changed = true;
        self.adds_changed |= adds_changed;
    }

    /// Apply the unchanged `map` again: the live world edits of ops that
    /// left it as it was taken back (only what differs is redone).
    fn settle(&mut self) {
        if self.world.reapply_map_mod(&self.region, &self.folder, &self.map) {
            self.world_changed = true;
        }
    }

    /// The key placement `i` of the world goes by (None: no such
    /// placement, or one another mod added).
    pub fn key_of(&self, i: usize) -> Option<PropKey> {
        if let Some(id) = edit::own_add(&self.world, &self.folder, i) {
            return Some(PropKey::Added(id.to_string()));
        }
        if self.world.map_overlay.added.contains_key(&i) {
            return None;
        }
        self.world.placement_ref(&self.region, i).map(PropKey::Original)
    }

    /// The world's placement of `key` (None: not in this world). The
    /// original's own placement even after this mod moved it.
    pub fn placement_of(&self, key: &PropKey) -> Option<usize> {
        match key {
            PropKey::Added(id) => self.world.map_overlay.placement_of_add(&self.folder, id),
            PropKey::Original(r) => {
                if !r.region.eq_ignore_ascii_case(&self.region) {
                    return None;
                }
                // As the mod file finds it (within 5 cm); the edit then
                // names it exactly ([`key_of`](Self::key_of)).
                self.world.find_placement(r)
            }
        }
    }

    /// The index in `map.races` of race `id`.
    pub fn race_index(&self, id: &str) -> Option<usize> {
        self.map.races.iter().position(|r| r.id == id)
    }

    fn placement(&self, key: &PropKey) -> Result<usize, Refused> {
        self.placement_of(key).ok_or(Refused::NoPlacement)
    }

    fn race_mut(&mut self, id: &str) -> Result<&mut CustomRace, Refused> {
        self.map.races.iter_mut().find(|r| r.id == id).ok_or(Refused::NoRace)
    }

    /// Refused as [`Refused::BadValue`] with the reason, unless `r` is Ok.
    /// A drawn wall's or ground's id: `id` when it is free, else the first
    /// free `<kind>-<n>`.
    fn collision_id(&self, id: Option<String>, kind: &str) -> Result<String, Refused> {
        match id {
            Some(id) => {
                ensure(valid_id(&id), Refused::BadValue)?;
                ensure(self.map.collision.iter().all(|e| e.id() != Some(id.as_str())), Refused::IdTaken)?;
                Ok(id)
            }
            None => Ok(self.map.fresh_collision_id(kind)),
        }
    }

    fn check<E: std::fmt::Display>(&mut self, r: Result<(), E>) -> Result<(), Refused> {
        r.map_err(|e| {
            self.refused_why = Some(e.to_string());
            Refused::BadValue
        })
    }

    /// A race point's place with its height from the road: the highest floor at or below it + 1 m, else the highest
    /// floor at its x/y (one under the road comes up onto it), as the game puts its own start-grid markers
    /// (`World::grid_ground`). Kept as given where the world has no floor (not read, or off the map); the checks
    /// then say so ([`crate::validate::floor_checks`]).
    fn on_floor(&self, p: Vec3) -> Vec3 {
        match self.world.grid_ground(p) {
            Some(z) => p.truncate().extend(z),
            None => p,
        }
    }

    fn exec(&mut self, op: Op, c: &mut Change) -> Result<(), Refused> {
        let (folder, region) = (self.folder.clone(), self.region.clone());
        match op {
            Op::Place { key, position, rotation } => {
                ensure(valid_position(position), Refused::BadValue)?;
                self.check(check_rotation(&rotation))?;
                let i = self.placement(&key)?;
                edit::set_transform(&mut self.map, &folder, &mut self.world, &region, i, position, rotation)?;
                self.world_changed = true;
            }
            Op::Delete { key } => {
                let i = self.placement(&key)?;
                edit::delete(&mut self.map, &folder, &mut self.world, &region, i)?;
                self.world_changed = true;
            }
            Op::Reset { key } => {
                let i = self.placement(&key)?;
                edit::reset(&mut self.map, &folder, &mut self.world, Some(&self.base), i)?;
                self.world_changed = true;
            }
            Op::Add { solid, position, rotation, id, ground } => {
                ensure(valid_position(position) && valid_solid(&solid), Refused::BadValue)?;
                self.check(check_rotation(&rotation))?;
                if let Some(id) = &id {
                    ensure(valid_id(id), Refused::BadValue)?;
                    ensure(self.map.add_index(id).is_none(), Refused::IdTaken)?;
                }
                let a = edit::add(&mut self.map, &folder, Some(&mut self.world), &region, &solid, position, rotation)?;
                if let (Some(id), Some(PropEdit::Add { id: given, .. })) = (&id, self.map.props.get_mut(a.edit)) {
                    *given = id.clone();
                    if let Some(i) = a.placement {
                        self.world.map_overlay.added.insert(i, (folder.clone(), id.clone()));
                    }
                }
                if ground
                    && let Some(i) = a.placement
                    && let Some(g) = edit::on_ground(&self.world, i, position, rotation)
                {
                    edit::set_transform(&mut self.map, &folder, &mut self.world, &region, i, g, rotation)?;
                }
                self.adds_changed = true;
                c.added.push(PropKey::Added(self.map.props[a.edit].add_id().unwrap_or_default().to_string()));
            }
            Op::Duplicate { key, position } => {
                ensure(valid_position(position), Refused::BadValue)?;
                let i = self.placement(&key)?;
                let a = edit::duplicate(&mut self.map, &folder, &mut self.world, &region, i, position)?;
                self.adds_changed = true;
                c.added.push(PropKey::Added(self.map.props[a.edit].add_id().unwrap_or_default().to_string()));
            }
            Op::DropToGround { key } => {
                let i = self.placement(&key)?;
                edit::drop_to_ground(&mut self.map, &folder, &mut self.world, &region, i)?;
                self.world_changed = true;
            }
            Op::RemoveCollision { target } => match target {
                CollisionKey::Drawn(id) => {
                    let k = self.map.collision.iter().position(|e| e.id() == Some(id.as_str())).ok_or(Refused::NoCollision)?;
                    self.map.collision.remove(k);
                }
                CollisionKey::Original(corners) => {
                    let e = CollisionEdit::Remove { region: region.clone(), corners };
                    self.check(e.check())?;
                    let CollisionEdit::Remove { corners, .. } = &e else { unreachable!() };
                    let key = face_key(corners);
                    let removed = |m: &MapMod| {
                        m.collision.iter().any(|c| {
                            matches!(c, CollisionEdit::Remove { region: r, corners: k } if r.eq_ignore_ascii_case(&region) && face_key(k) == key)
                        })
                    };
                    // A face of the original's (drawn faces join the packs later in their lists, and are no
                    // removal's), barrier group 0 only: a race's barriers are switched per race.
                    let original = self.world.ground.faces().any(|f| f.group == 0 && face_key(&f.vertices[..f.corners.clamp(3, 4)]) == key);
                    ensure(original || removed(&self.map), Refused::NoCollision)?;
                    if !removed(&self.map) {
                        ensure(self.map.collision.len() < MAX_COLLISION, Refused::Full)?;
                        self.map.collision.push(e);
                    }
                }
            },
            Op::DrawWall { from, to, height, surface, id } => {
                let id = self.collision_id(id, "wall")?;
                let e = CollisionEdit::Wall { id, region: region.clone(), from, to, height, surface };
                self.check(e.check())?;
                ensure(self.map.collision.len() < MAX_COLLISION, Refused::Full)?;
                self.map.collision.push(e);
            }
            Op::DrawGround { outline, surface, id } => {
                let id = self.collision_id(id, "ground")?;
                let e = CollisionEdit::Ground { id, region: region.clone(), outline, surface };
                self.check(e.check())?;
                ensure(self.map.collision.len() < MAX_COLLISION, Refused::Full)?;
                self.map.collision.push(e);
            }
            Op::NewRace { track, kind, id } => {
                let id = match id {
                    Some(id) => {
                        ensure(valid_id(&id), Refused::BadValue)?;
                        ensure(self.race_index(&id).is_none(), Refused::IdTaken)?;
                        id
                    }
                    None => {
                        let mut n = self.map.races.len() + 1;
                        while self.race_index(&format!("race-{n}")).is_some() {
                            n += 1;
                        }
                        format!("race-{n}")
                    }
                };
                ensure(self.map.races.len() < ob_map::tools::map::format::MAX_RACES, Refused::Full)?;
                let mut r = CustomRace::new(&id, track, kind);
                if let Some(n) = id.strip_prefix("race-").filter(|n| n.parse::<usize>().is_ok()) {
                    r.name = format!("Race {n}");
                }
                self.map.races.push(r);
                c.race = Some(id);
            }
            Op::DeleteRace { race } => {
                let k = self.race_index(&race).ok_or(Refused::NoRace)?;
                self.map.races.remove(k);
            }
            Op::RenameRace { race, name } => {
                ensure(name.chars().count() <= MAX_TEXT, Refused::BadValue)?;
                self.race_mut(&race)?.name = name;
                c.race = Some(race);
            }
            Op::SetRaceId { race, id } => {
                ensure(valid_id(&id), Refused::BadValue)?;
                let k = self.race_index(&race).ok_or(Refused::NoRace)?;
                ensure(self.race_index(&id).is_none_or(|j| j == k), Refused::IdTaken)?;
                self.map.races[k].id = id.clone();
                c.race = Some(id);
            }
            Op::SetTrack { race, track } => {
                self.race_mut(&race)?.track = track;
                c.race = Some(race);
            }
            Op::SetKind { race, kind } => {
                let r = self.race_mut(&race)?;
                if r.kind != kind {
                    r.kind = kind;
                    r.laps = if kind.loops() { r.laps.clamp(2, MAX_LAPS) } else { 1 };
                }
                c.race = Some(race);
            }
            Op::SetLaps { race, laps } => {
                let kind = self.race_mut(&race)?.kind;
                self.check(check_laps(kind, laps))?;
                self.race_mut(&race)?.laps = laps;
                c.race = Some(race);
            }
            Op::SetDirection { race, backward } => {
                self.race_mut(&race)?.backward = backward;
                c.race = Some(race);
            }
            Op::AddPoint { race, part, index, position, heading } => {
                ensure(valid_position(position) && heading.is_finite(), Refused::BadValue)?;
                let position = self.on_floor(position);
                let r = self.race_mut(&race)?;
                let spot = Spot { position, heading };
                let index = match part {
                    RacePart::Checkpoint => {
                        ensure(r.checkpoints.len() < max_points(part), Refused::Full)?;
                        let i = index.unwrap_or(r.checkpoints.len()).min(r.checkpoints.len());
                        r.checkpoints.insert(i, position.truncate());
                        i
                    }
                    RacePart::Grid => {
                        if r.grid.is_empty() {
                            r.grid = race::effective_grid(r);
                        }
                        ensure(r.grid.len() < max_points(part), Refused::Full)?;
                        let i = index.unwrap_or(r.grid.len()).min(r.grid.len());
                        r.grid.insert(i, spot);
                        i
                    }
                    RacePart::Start => {
                        r.start = Some(spot);
                        0
                    }
                    RacePart::Finish => {
                        r.finish = Some(spot);
                        0
                    }
                };
                c.race = Some(race);
                c.point = Some(RacePoint { part, index });
            }
            Op::MovePoint { race, point, position, heading } => {
                ensure(valid_position(position) && heading.is_none_or(f32::is_finite), Refused::BadValue)?;
                let position = self.on_floor(position);
                let r = self.race_mut(&race)?;
                let turn = |s: Spot| Spot { position, heading: heading.unwrap_or(s.heading) };
                match point.part {
                    RacePart::Checkpoint => {
                        *r.checkpoints.get_mut(point.index).ok_or(Refused::NoPoint)? = position.truncate()
                    }
                    RacePart::Grid => {
                        if r.grid.is_empty() {
                            r.grid = race::effective_grid(r);
                        }
                        let g = r.grid.get_mut(point.index).ok_or(Refused::NoPoint)?;
                        *g = turn(*g);
                    }
                    RacePart::Start => r.start = Some(turn(r.start.ok_or(Refused::NoPoint)?)),
                    RacePart::Finish => r.finish = Some(turn(r.finish.ok_or(Refused::NoPoint)?)),
                }
                c.race = Some(race);
                c.point = Some(point);
            }
            Op::DeletePoint { race, point } => {
                let r = self.race_mut(&race)?;
                match point.part {
                    RacePart::Checkpoint => {
                        ensure(point.index < r.checkpoints.len(), Refused::NoPoint)?;
                        r.checkpoints.remove(point.index);
                    }
                    RacePart::Grid => {
                        if r.grid.is_empty() {
                            r.grid = race::effective_grid(r);
                        }
                        ensure(point.index < r.grid.len(), Refused::NoPoint)?;
                        r.grid.remove(point.index);
                    }
                    RacePart::Start => {
                        r.start.take().ok_or(Refused::NoPoint)?;
                    }
                    RacePart::Finish => {
                        r.finish.take().ok_or(Refused::NoPoint)?;
                    }
                }
                c.race = Some(race);
            }
            Op::MoveCheckpoint { race, index, later } => {
                let r = self.race_mut(&race)?;
                let j = if later { index + 1 } else { index.wrapping_sub(1) };
                ensure(index < r.checkpoints.len() && j < r.checkpoints.len(), Refused::NoPoint)?;
                r.checkpoints.swap(index, j);
                c.race = Some(race);
                c.point = Some(RacePoint { part: RacePart::Checkpoint, index: j });
            }
            Op::PutRace { race, to } => {
                ensure(valid_race(&to), Refused::BadValue)?;
                self.check(check_laps(to.kind, to.laps))?;
                let mut to = to;
                for s in to.start.iter_mut().chain(to.finish.iter_mut()).chain(to.grid.iter_mut()) {
                    s.position = self.on_floor(s.position);
                }
                let k = self.race_index(&race).ok_or(Refused::NoRace)?;
                ensure(self.race_index(&to.id).is_none_or(|j| j == k), Refused::IdTaken)?;
                c.race = Some(to.id.clone());
                self.map.races[k] = to;
            }
            Op::Details { name, author, description } => {
                ensure(
                    [&name, &author, &description].iter().all(|t| t.chars().count() <= MAX_TEXT),
                    Refused::BadValue,
                )?;
                (self.map.name, self.map.author, self.map.description) = (name, author, description);
            }
            Op::PutMod { map } => {
                // What the file format holds, and nothing else.
                ensure(MapMod::parse(&map.write()).as_ref() == Ok(&map), Refused::BadValue)?;
                self.map = map;
                self.adds_changed = true;
                self.world_changed = true;
                self.apply(true);
            }
            Op::Group { ops, .. } => {
                for op in ops {
                    let mut sub = Change::default();
                    self.exec(op, &mut sub)?;
                    c.added.append(&mut sub.added);
                    c.race = sub.race.or(c.race.take());
                    c.point = sub.point.or(c.point.take());
                }
            }
        }
        Ok(())
    }
}

/// `op` written as its JSON and read back; panics when that does not give `op` again. A value JSON has no
/// number for (not finite: the session refuses it) is left as it is.
pub fn through_json(op: Op) -> Op {
    let text = op.to_json();
    match Op::from_json(&text) {
        Ok(back) => {
            assert_eq!(back, op, "the edit does not round-trip through its JSON: {text}");
            back
        }
        Err(e) => {
            assert!(text.contains("null"), "the edit's JSON does not read back ({e}): {text}");
            op
        }
    }
}

/// `h` has steps to write, and this session writes them (a read-only view
/// leaves them to the writer).
fn writes(h: &History) -> bool {
    h.writable() && h.unsaved()
}

/// A race the mod file can hold.
fn valid_race(r: &CustomRace) -> bool {
    let spots = r.grid.iter().chain(&r.start).chain(&r.finish);
    valid_id(&r.id)
        && r.name.chars().count() <= MAX_TEXT
        && (1..=MAX_LAPS).contains(&r.laps)
        && r.checkpoints.len() <= max_points(RacePart::Checkpoint)
        && r.grid.len() <= max_points(RacePart::Grid)
        && r.checkpoints.iter().all(|c| valid_position(c.extend(0.0)))
        && spots.clone().all(|s| valid_position(s.position) && s.heading.is_finite())
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
