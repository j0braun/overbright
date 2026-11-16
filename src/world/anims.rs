use glam::{Mat4, Vec3};

use super::*;

/// A shop door (a "DoorAnim" world-animation instance): its slats are
/// placements posed by their animations at the door's frame.
#[derive(Clone, Debug)]
pub struct WorldDoor {
    /// The instance's +0x20 / +0x24: the shop's world trigger.
    pub keys: [u32; 2],
    pub transform: Mat4,
    /// (placement, animation hash, rest matrix) per slat.
    pub pieces: Vec<(usize, u32, Mat4)>,
    pub state: crate::shop_scene::Door,
}

/// A world-animation instance that plays by itself (0x43A4A0: no
/// message key at +0x20 and no +0x3C, so each entity controller starts
/// playing, +0x10 = 1, at 0x43A7F9).
#[derive(Clone, Debug, Default)]
pub struct WorldLoop {
    /// The instance record's +0x00 (the sound objects' key, 0x439F40).
    pub id: u32,
    /// The instance's matrix (record +0x40), the root entity's world matrix
    /// (0x43A78D → 0x4012B0).
    pub transform: Mat4,
    pub player: LoopPlayer,
    /// The entities below the root, in tree order (a parent before its
    /// children).
    pub pieces: Vec<LoopPiece>,
    /// The record's track window (+0x38..+0x3B, [`track_window_admits`]).
    pub window: [u8; 4],
    /// Made for the world's track ([`World::set_track`]): an instance
    /// 0x43A4A0 would not create is kept out of sight, still and silent.
    pub active: bool,
}

/// The instance record's playback fields as the entity controllers' set-up
/// 0x438860 puts them in each player (0x435010).
#[derive(Clone, Copy, Debug, Default)]
pub struct LoopPlayer {
    /// Player +0x34: the record's +0x0C, or 0x28 when 0 (0x438A1D), with
    /// 0x80 and 0x100 (0x4389D1, 0x4389E9).
    pub flags: u32,
    /// Player +0x5C: record +0x10, 1 when 0 (0x4389AE).
    pub speed: f32,
    /// Player +0x68 / +0x6C: record +0x14 / +0x18.
    pub delay: [f32; 2],
    /// Player +0x38 / +0x3C with flag 0x40: record +0x1C / +0x1E (0x438B23).
    pub range: [u16; 2],
}

#[derive(Clone, Debug, Default)]
pub struct LoopPiece {
    /// The entity's hash (its controller's vtable +0xC, 0x439F40).
    pub hash: u32,
    pub placement: usize,
    pub animation: u32,
    pub rest: Mat4,
    /// The parent piece (None: the root, the instance's matrix).
    pub parent: Option<usize>,
    /// Player +0x60: the animation's length in frames.
    pub length: f32,
    /// Player +0x64.
    pub frame: f32,
    /// Player +0x14 / +0x18: seconds into the start delay / the pause.
    pub delay_time: f32,
    pub pause_time: f32,
    /// Player +0x08: 1 while a back-and-forth (flag 0x10) runs back.
    pub back: bool,
    /// The pause limit +0x6C: the record's +0x18, or with flag 0x200
    /// (0x438B3B) r × ½ × length ÷ 30 ÷ speed.
    pub pause: f32,
    /// Player +0x10 = 2: a back-and-forth with 0x800 stopped at an end.
    pub stopped: bool,
}

/// The "random" factor 0x43C170 gives 0x438860's flag 0x200 / 0x400 draws:
/// the seed is a fresh local 0x69BABE69 each time, so the factor is always
/// (0x69BABE69 mod 0x7FFFFFFF) × 4.656613e-10.
pub const LOOP_RANDOM: f64 = 1_773_846_121.0 * 4.656613e-10;

impl LoopPiece {
    /// Whether the player waits in its start delay (0x431610: flag 0x80,
    /// 30 × speed × the timer +0x14 short of 30 × +0x68, and the frame
    /// short of the end) or its pause (0x431660: flag 0x100, +0x18
    /// against +0x6C) — what 0x439F40 reports as hidden.
    pub fn waiting(&self, p: &LoopPlayer) -> bool {
        let ranged = p.flags & 0x40 != 0;
        let end = if ranged { p.range[1] as f32 * p.speed } else { self.length };
        let short = |timer: f32, limit: f32| 30.0 * p.speed * timer < 30.0 * limit && self.frame < end;
        (p.flags & 0x80 != 0 && short(self.delay_time, p.delay[0]))
            || (p.flags & 0x100 != 0 && short(self.pause_time, self.pause))
    }

    /// One update of the player, 0x435140 (from the controller's update
    /// 0x435980) with state +0x10 = 1, `dt` seconds on: the rate +0x40 is
    /// 1 (0x435010) for these, so frames advance 30 × speed a second.
    /// First a start delay (+0x68, flag 0x80), then a pause (+0x6C, flag
    /// 0x100) while the frame is short of the end; then forward (0x08),
    /// backward with 0x1000, wrapping by the span with 0x20 unless 0x800
    /// (the pause timer restarts at each wrap), else stopping at the end;
    /// with 0x10 instead of 0x08 back and forth. Returns whether the frame
    /// moved.
    pub fn step(&mut self, p: &LoopPlayer, dt: f32) -> bool {
        let rs = p.speed;
        let step = 30.0 * rs * dt;
        let ranged = p.flags & 0x40 != 0;
        let start = if ranged { p.range[0] as f32 * rs } else { 0.0 };
        let end = if ranged { p.range[1] as f32 * rs } else { self.length };
        let span = if ranged { end - start } else { self.length };
        if self.stopped {
            return false;
        }
        let mut flags = p.flags;
        // 0x435228: 0x08 with 0x10 drops 0x10; neither sets 0x08.
        match (flags & 0x08 != 0, flags & 0x10 != 0) {
            (true, true) => flags &= !0x10,
            (false, false) => flags |= 0x08,
            _ => {}
        }
        let wraps = flags & 0x20 != 0 && flags & 0x800 == 0;
        let before = self.frame;
        // The start delay and the pause, in frames (×30·speed).
        for (timer, limit, bit) in [(&mut self.delay_time, p.delay[0], 0x80), (&mut self.pause_time, self.pause, 0x100)]
        {
            let t = 30.0 * rs * *timer;
            let lim = 30.0 * limit;
            if flags & bit != 0 && t < lim && self.frame < end {
                let t = t + step;
                if t > lim {
                    // 0x43C240: the overshoot (fmod) carries into the frame.
                    self.frame += t - (t / lim).floor() * lim;
                }
                *timer = if rs != 0.0 { t / (30.0 * rs) } else { *timer };
                return self.frame != before;
            }
        }
        if flags & 0x08 != 0 {
            if flags & 0x1000 != 0 {
                self.frame -= step;
                if self.frame < start {
                    if wraps {
                        self.frame += span;
                        self.pause_time = 0.0;
                    } else {
                        self.frame = start;
                    }
                }
            } else {
                self.frame += step;
                if self.frame > end {
                    if wraps {
                        self.frame -= span;
                        self.pause_time = 0.0;
                    } else {
                        self.frame = end;
                    }
                }
            }
        } else if !self.back {
            self.frame += step;
            if self.frame > end {
                self.frame = end - (self.frame - end);
                self.back = true;
                self.stopped = flags & 0x800 != 0;
            }
        } else {
            self.frame -= step;
            if self.frame < start {
                self.frame = start;
                self.back = false;
                self.pause_time = 0.0;
                self.stopped = flags & 0x800 != 0;
            }
        }
        self.frame != before
    }
}

/// The section a world-animation placement is drawn with: the always-drawn
/// one. **Inferred**: the world animations are drawn by their manager, not
/// culled by the scenery's section visibility.
pub(super) const WORLD_ANIM_SECTION: u32 = 2600;

/// 0x43A4A0's track window (**verified**: 0x43A4C1…
/// 0x43A526): with record +0x38 (as a dword) non-zero, an instance is made
/// only when r = 0x89E7A0 % 100 (the track number; 4000 in free roam, so
/// 0) lies in [+0x39, +0x38] or [+0x3B, +0x3A] (bytes, signed compares).
pub fn track_window_admits(window: [u8; 4], track: u32) -> bool {
    if window == [0; 4] {
        return true;
    }
    let r = (track as i32) % 100;
    let within = |lo: u8, hi: u8| (i32::from(lo)..=i32::from(hi)).contains(&r);
    within(window[1], window[0]) || within(window[3], window[2])
}

impl World {
    /// The shop doors (0x43A4A0 for the DoorAnim instances): a placement
    /// per slat with a model, posed shut (frame 0; the instance starts
    /// held, state 0, and the event tracks call frame 0 shut).
    pub(super) fn add_doors(&mut self, track_bun: &[u8]) {
        use crate::formats::world_anim::{DOOR_TREE, WorldAnimations};
        self.world_anims = WorldAnimations::parse(track_bun);
        let instances: Vec<_> = self.world_anims.instances.iter().filter(|i| i.tree == DOOR_TREE).cloned().collect();
        for inst in instances {
            let mut door = WorldDoor {
                keys: inst.keys,
                transform: inst.transform,
                pieces: Vec::new(),
                state: crate::shop_scene::Door::default(),
            };
            let entities: Vec<_> = self.world_anims.tree(inst.tree).filter(|e| e.parent != 0).cloned().collect();
            for e in entities {
                let Some(&solid) = self.solid_by_hash.get(&e.solid) else { continue };
                if self.solids[solid].indices.is_empty() {
                    continue;
                }
                let frames = self.world_anims.animations.get(&e.animation).map_or(0, |a| a.frames());
                // The channel binding 0x4350C0 writes the compound header's frame
                // count (0x6E2110) as the length +0x60: 91 for the shop doors.
                door.state.length = door.state.length.max(frames as f32);
                let radius = {
                    let s = &self.solids[solid];
                    s.vertices.iter().map(|v| Vec3::from(v.position).length()).fold(0.0f32, f32::max)
                };
                door.pieces.push((self.placements.len(), e.animation, e.rest));
                self.placements.push(Placement {
                    solid,
                    low_solid: None,
                    mid_solid: Some(solid),
                    transform: inst.transform * e.rest,
                    section: WORLD_ANIM_SECTION,
                    flags: 0,
                    cull_radius: radius + 6.0,
                });
            }
            let poses = self.door_poses(&door);
            for (p, t) in poses {
                self.placements[p].transform = t;
            }
            self.doors.push(door);
        }
    }

    /// Each slat's world transform at the door's frame.
    pub fn door_poses(&self, door: &WorldDoor) -> Vec<(usize, Mat4)> {
        door.pieces
            .iter()
            .map(|&(p, anim, rest)| {
                let local = self.world_anims.animations.get(&anim).map_or(rest, |a| a.pose(door.state.frame, rest));
                (p, door.transform * local)
            })
            .collect()
    }

    /// The door message (0x438CA0 → 0x435660) to the doors keyed `key`.
    pub fn door_message(&mut self, key: u32, position: f32, duration: f32) {
        for d in self.doors.iter_mut().filter(|d| key != 0 && d.keys.contains(&key)) {
            d.state.message(position, duration);
        }
    }

    /// Move the doors `dt` seconds on; the slats that moved, posed.
    pub fn step_doors(&mut self, dt: f32) -> Vec<(usize, Mat4)> {
        let mut out = Vec::new();
        for i in 0..self.doors.len() {
            let before = self.doors[i].state.frame;
            self.doors[i].state.step(dt);
            if self.doors[i].state.frame != before {
                out.extend(self.door_poses(&self.doors[i]));
            }
        }
        out
    }

    /// Every door's slats as they stand.
    pub fn all_door_poses(&self) -> Vec<(usize, Mat4)> {
        self.doors.iter().flat_map(|d| self.door_poses(d)).collect()
    }

    /// The world animations that play by themselves (0x43A4A0 for an
    /// instance with neither +0x20 nor +0x3C): in L4RA the three vista
    /// searchlights (`XO_VISTA_SPOTLIGHT_1A_DW_00`, beams 986 m long, at
    /// (−459, −470), (−232, −448), (81, −448)), a bridge searchlight, the
    /// jumbo planes, the tower cranes, the graffiti trains and the pub's
    /// sails. Their entities' own scenery instances are hidden (0x10, in
    /// no scenery group), as with the doors. An instance whose track
    /// window (+0x38..+0x3B, [`track_window_admits`]) is set is loaded
    /// inactive, its placements hidden: the world is loaded without its
    /// track number, and [`World::set_track`] makes it when the track
    /// admits it (L4RB's four windowed instances, window 20–29 or 0: its
    /// free roam and drag tracks 4221–4223, not 4201–4213).
    pub(super) fn add_loops(&mut self) {
        use crate::formats::world_anim::DOOR_TREE;
        let instances: Vec<_> = self
            .world_anims
            .instances
            .iter()
            .filter(|i| i.tree != DOOR_TREE && i.keys[0] == 0 && i.hold == 0)
            .cloned()
            .collect();
        for inst in instances {
            let flags = if inst.flags == 0 { 0x28 } else { inst.flags } | 0x80 | 0x100;
            let speed = if inst.speed == 0.0 { 1.0 } else { inst.speed };
            let player = LoopPlayer { flags, speed, delay: inst.delay, range: inst.range };
            let active = inst.window == [0; 4];
            let mut lp =
                WorldLoop { id: inst.id, transform: inst.transform, player, pieces: Vec::new(), window: inst.window, active };
            let entities: Vec<_> = self.world_anims.tree(inst.tree).cloned().collect();
            let Some(root) = entities.iter().find(|e| e.parent == 0).map(|e| e.hash) else { continue };
            // Parents before children: walk the tree breadth first.
            let mut order: Vec<(u32, Option<usize>)> = vec![(root, None)];
            let mut k = 0;
            while k < order.len() {
                let (hash, _) = order[k];
                let here = if hash == root { None } else { Some(k - 1) };
                for e in entities.iter().filter(|e| e.parent == hash && e.hash != root) {
                    order.push((e.hash, here));
                }
                k += 1;
            }
            for &(hash, parent) in &order[1..] {
                let Some(e) = entities.iter().find(|e| e.hash == hash) else { continue };
                let solid = self.solid_by_hash.get(&e.solid).copied().filter(|&s| !self.solids[s].indices.is_empty());
                let length = self.world_anims.animations.get(&e.animation).map_or(0.0, |a| a.length());
                // 0x438860: flag 0x200 draws the pause, 0x400 the start frame.
                let pause = if flags & 0x200 != 0 {
                    (LOOP_RANDOM * (length as f64 / 30.0 / speed as f64) * 0.5) as f32
                } else {
                    inst.delay[1]
                };
                let frame = if flags & 0x400 != 0 { (LOOP_RANDOM * length as f64) as f32 } else { 0.0 };
                let placement = match solid {
                    Some(solid) => {
                        let s = &self.solids[solid];
                        let radius = s.vertices.iter().map(|v| Vec3::from(v.position).length()).fold(0.0f32, f32::max);
                        let scale = inst
                            .transform
                            .x_axis
                            .truncate()
                            .length()
                            .max(inst.transform.z_axis.truncate().length())
                            .max(1.0);
                        self.placements.push(Placement {
                            solid,
                            low_solid: None,
                            mid_solid: Some(solid),
                            transform: inst.transform * e.rest,
                            section: WORLD_ANIM_SECTION,
                            flags: if active { 0 } else { HIDDEN_FLAG },
                            cull_radius: radius * scale + 6.0,
                        });
                        self.placements.len() - 1
                    }
                    None => usize::MAX,
                };
                lp.pieces.push(LoopPiece {
                    hash: e.hash,
                    placement,
                    animation: e.animation,
                    rest: e.rest,
                    parent,
                    length,
                    frame,
                    pause,
                    ..Default::default()
                });
            }
            for (p, t) in self.loop_poses(&lp) {
                self.placements[p].transform = t;
            }
            self.loops.push(lp);
        }
    }

    /// Each piece's world matrix: its parent's (the instance's matrix for
    /// the root's children) × its animation's pose. Traced: 0x438860 links
    /// each entity's render node to its parent's (0x6345C0: node +0xE0, the
    /// parent's child list +0x08), the controller update 0x435980 writes
    /// the evaluated pose (0x4316B0 → 0x6E3B10, bone 1) over the node's
    /// local +0x50 (0x40EFE0), and the node's world 0x60BB20 is
    /// local × parent world (0x43C990 → 0x4013F0, row-major). The root
    /// (entity hash = tree hash) gets no model (0x4388E9) and its local is
    /// the instance matrix (0x43A7A7); it has no channels, so no player.
    fn loop_poses(&self, lp: &WorldLoop) -> Vec<(usize, Mat4)> {
        let mut world: Vec<Mat4> = Vec::with_capacity(lp.pieces.len());
        let mut out = Vec::new();
        for piece in &lp.pieces {
            let local =
                self.world_anims.animations.get(&piece.animation).map_or(piece.rest, |a| a.bone_pose(piece.frame));
            let parent = piece.parent.map_or(lp.transform, |i| world[i]);
            let m = parent * local;
            world.push(m);
            if piece.placement != usize::MAX {
                out.push((piece.placement, m));
            }
        }
        out
    }

    /// The track the world is loaded for (0x89E7A0: the race's number,
    /// 4000 in free roam), as 0x43A4A0 reads it when the track's world
    /// animations are created (**verified** 0x43A4C1…
    /// 0x43A526): a windowed instance exists only when the track admits it
    /// ([`track_window_admits`]). The clone loads the world before it knows
    /// the track, so the windowed instances are made inactive at load and
    /// switched here: placements shown or hidden (for the renderer through
    /// [`World::take_changed`]), not stepped and not heard while inactive.
    /// Kept per track in [`World::anim_track`]; the same track again does
    /// nothing. **Not ported**: the generator draws of a created
    /// instance's flags 0x400 / 0x200 (0x43A5EE…0x43A60C; the RNG unit).
    pub fn set_track(&mut self, track: u32) {
        if self.anim_track == Some(track) {
            return;
        }
        self.anim_track = Some(track);
        for i in 0..self.loops.len() {
            let on = track_window_admits(self.loops[i].window, track);
            if on == self.loops[i].active {
                continue;
            }
            self.loops[i].active = on;
            for k in 0..self.loops[i].pieces.len() {
                let p = self.loops[i].pieces[k].placement;
                if p == usize::MAX {
                    continue;
                }
                let flags = self.placements[p].flags;
                self.placements[p].flags = if on { flags & !HIDDEN_FLAG } else { flags | HIDDEN_FLAG };
                self.changed.push(p);
            }
        }
    }

    /// Every self-playing world-animation entity as the sound objects
    /// find it (0x439F40(instance number, entity hash)): its node's world
    /// position (+0x40, the translation of the world matrix +0x10 that
    /// 0x60BB20 brings up to date) and whether its player waits
    /// ([`LoopPiece::waiting`]).
    pub fn anim_points(&self) -> Vec<crate::audio::enviro::AnimPoint> {
        let mut out = Vec::new();
        for lp in self.loops.iter().filter(|l| l.active) {
            let mut world: Vec<Mat4> = Vec::with_capacity(lp.pieces.len());
            for piece in &lp.pieces {
                let local =
                    self.world_anims.animations.get(&piece.animation).map_or(piece.rest, |a| a.bone_pose(piece.frame));
                let m = piece.parent.map_or(lp.transform, |i| world[i]) * local;
                world.push(m);
                out.push(crate::audio::enviro::AnimPoint {
                    id: lp.id,
                    hash: piece.hash,
                    position: m.w_axis.truncate(),
                    hidden: piece.waiting(&lp.player),
                });
            }
        }
        out
    }

    /// Play the world animations `dt` seconds on (the controllers'
    /// update 0x435980 → 0x435140); the placements that moved, posed.
    pub fn step_loops(&mut self, dt: f32) -> Vec<(usize, Mat4)> {
        let mut out = Vec::new();
        for i in 0..self.loops.len() {
            if !self.loops[i].active {
                continue;
            }
            let player = self.loops[i].player;
            let mut moved = false;
            for piece in &mut self.loops[i].pieces {
                moved |= piece.step(&player, dt);
            }
            if moved {
                out.extend(self.loop_poses(&self.loops[i]));
            }
        }
        out
    }
}

#[cfg(test)]
mod window_tests {
    use super::track_window_admits;

    /// 0x43A4E7: L4RB's windowed instances (bytes 1D 14 00 00: 20–29, or
    /// 0) are made in free roam (4000) and on 4221–4223, not on 4201–4213.
    #[test]
    fn the_track_window_picks_tracks_by_number() {
        let w = [0x1D, 0x14, 0, 0];
        assert!(track_window_admits(w, 4000));
        assert!(track_window_admits(w, 4221) && track_window_admits(w, 4223));
        assert!(!track_window_admits(w, 4201) && !track_window_admits(w, 4213));
        assert!(track_window_admits([0; 4], 4201));
    }

    /// 0x43A4A0, 0x43A4C1…0x43A526: L4RB's windowed world
    /// animations exist for free roam (4000) and the drag tracks, not for
    /// its circuits; the clone loads them inactive and hidden and
    /// `World::set_track` switches them, their placements reported changed,
    /// and only active ones play and sound (`anim_points`).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn the_world_makes_its_windowed_animations_for_its_track() {
        use crate::world::{HIDDEN_FLAG, World};
        let root = crate::testkit::game_root();
        let mut world = World::load(&root, "L4RB").unwrap();
        let windowed: Vec<usize> = (0..world.loops.len()).filter(|&i| world.loops[i].window != [0; 4]).collect();
        assert_eq!(windowed.len(), 4, "L4RB's four windowed instances");
        let placements = |w: &World, i: usize| -> Vec<usize> {
            w.loops[i].pieces.iter().map(|p| p.placement).filter(|&p| p != usize::MAX).collect()
        };
        let hidden = |w: &World, i: usize| placements(w, i).iter().all(|&p| w.placements[p].flags & HIDDEN_FLAG != 0);
        let ids = |w: &World| w.anim_points().iter().map(|a| a.id).collect::<std::collections::BTreeSet<u32>>();
        // As loaded: none of them made.
        assert!(windowed.iter().all(|&i| !world.loops[i].active && hidden(&world, i)));
        assert!(windowed.iter().all(|&i| !ids(&world).contains(&world.loops[i].id)));
        let _ = world.take_changed();
        world.set_track(4000);
        assert!(windowed.iter().all(|&i| world.loops[i].active && !hidden(&world, i)));
        assert!(windowed.iter().all(|&i| ids(&world).contains(&world.loops[i].id)));
        let changed = world.take_changed();
        assert!(windowed.iter().flat_map(|&i| placements(&world, i)).all(|p| changed.contains(&p)));
        // The same track again: nothing.
        world.set_track(4000);
        assert!(world.take_changed().is_empty());
        // A circuit (4201): out again, still.
        world.set_track(4201);
        assert!(windowed.iter().all(|&i| !world.loops[i].active && hidden(&world, i)));
        let frames: Vec<f32> = windowed.iter().flat_map(|&i| world.loops[i].pieces.iter().map(|p| p.frame)).collect();
        let _ = world.step_loops(1.0);
        let after: Vec<f32> = windowed.iter().flat_map(|&i| world.loops[i].pieces.iter().map(|p| p.frame)).collect();
        assert_eq!(frames, after, "an inactive instance does not play");
        // A drag track (4221).
        world.set_track(4221);
        assert!(windowed.iter().all(|&i| world.loops[i].active));
    }
}
