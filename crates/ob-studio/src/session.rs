//! One map mod open on one region, without a window: the mod, the world
//! it is shown in, the undo history. [`MapSession::run`] makes one
//! [`Op`] as one undo step; the desktop editor keeps its UI state (camera,
//! selection, tools) next to a session, and the console tools drive one
//! directly.

use glam::Vec3;

use nfsu2::studio_api::map::edit;
use nfsu2::studio_api::world::World;
use ob_map::tools::map::format::{CustomRace, MAX_LAPS, MAX_TEXT, MapMod, PropEdit, Spot, valid_id, valid_solid};
use ob_map::tools::map::ops::{Change, Op, PropKey, RacePart, RacePoint, Refused, ensure, max_points, valid_position};
use ob_map::tools::map::race;

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
        if let Err(e) = self.exec(op, &mut change) {
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

    /// Record the edit just made to `map` as one undo step named `label`
    /// and apply the mod to the world again; false (and nothing done)
    /// when the mod did not change. [`run`](Self::run) ends in this; the
    /// editor's panels that edit `map` in place call it too.
    pub fn commit(&mut self, label: &str) -> bool {
        let before_adds: Vec<PropEdit> = adds(self.undo.current()).into_iter().cloned().collect();
        if !self.undo.record(label, &self.map) {
            // The mod is as before, the world maybe not (see `run`).
            self.settle();
            return false;
        }
        self.dirty = true;
        let changed_adds = adds(&self.map).into_iter().ne(before_adds.iter());
        self.apply(changed_adds);
        true
    }

    /// Take back the last step; its name.
    pub fn undo(&mut self) -> Option<String> {
        let label = self.undo.undo_label()?.to_string();
        let m = self.undo.undo()?;
        self.step_to(m);
        Some(label)
    }

    /// Make the last undone step again; its name.
    pub fn redo(&mut self) -> Option<String> {
        let label = self.undo.redo_label()?.to_string();
        let m = self.undo.redo()?;
        self.step_to(m);
        Some(label)
    }

    fn step_to(&mut self, m: MapMod) {
        let changed_adds = adds(&self.map) != adds(&m);
        self.map = m;
        self.dirty = true;
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

    fn exec(&mut self, op: Op, c: &mut Change) -> Result<(), Refused> {
        let (folder, region) = (self.folder.clone(), self.region.clone());
        match op {
            Op::Place { key, position, rotation } => {
                ensure(valid_position(position) && rotation.is_finite(), Refused::BadValue)?;
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
                ensure(valid_position(position) && rotation.is_finite() && valid_solid(&solid), Refused::BadValue)?;
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
                ensure((1..=MAX_LAPS).contains(&laps), Refused::BadValue)?;
                self.race_mut(&race)?.laps = laps;
                c.race = Some(race);
            }
            Op::SetDirection { race, backward } => {
                self.race_mut(&race)?.backward = backward;
                c.race = Some(race);
            }
            Op::AddPoint { race, part, index, position, heading } => {
                ensure(valid_position(position) && heading.is_finite(), Refused::BadValue)?;
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
