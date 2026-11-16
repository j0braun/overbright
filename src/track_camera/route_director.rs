use glam::{Mat4, Vec3};

use super::*;
use crate::formats::ai_routes::RouteRecord;
use crate::formats::camera_tracks::{CameraEntry, CameraGroup, CameraKind, CameraTrack};
use crate::opponents::GameRng;
use crate::render::camera::Camera;

/// A track must run at least this long, s, from the car's fraction
/// (0x44F7B0 against 0x802148).
pub const MIN_RUN: f32 = 1.5;

/// The shot the director holds: the route group and track, and the car's
/// frame when it began (an entry's +8 holds that frame).
#[derive(Clone, Debug, Default)]
pub struct Shot {
    pub group: usize,
    pub track: usize,
    pub frame: Mat4,
}

/// A record's fraction of the car's race coordinate `along` (0x5D4550 of the
/// record's +0x38 / +0x3C range, wrapped by the road length), or None when
/// the car is outside the record (the coverage test of [`route_group`]).
/// The native value, which is a number outside the record too, is
/// [`record_fraction`] (0x5D4550).
pub(super) fn section_fraction(record: &RouteRecord, along: f32, length: f32) -> Option<f32> {
    let (lo, hi) = record.coord_range;
    let mut span = hi - lo;
    if span < 0.0 {
        span += length;
    }
    if span.abs() < 0.001 {
        return None;
    }
    let s = (along - lo).rem_euclid(length.max(1.0));
    (s <= span).then(|| s / span)
}

/// The route group the car is on: a race record covering the car's
/// coordinate whose (id, direction bit) keys a route group (0x44F9A0's
/// controller record, its flags bit 1 the direction; the record's own
/// direction first, then the other). **Inferred**: which of several
/// overlapping records the game's controller is; the first with a group is
/// taken. Returns the group and the car's fraction along the record.
pub fn route_group(
    groups: &[CameraGroup],
    records: &[RouteRecord],
    length: f32,
    along: f32,
    backward: bool,
) -> Option<(usize, f32)> {
    for bit in [backward, !backward] {
        for record in records.iter().filter(|r| r.is_race() && (r.flags >> 1 & 1 != 0) == bit) {
            let Some(fraction) = section_fraction(record, along, length) else { continue };
            if let Some(g) =
                groups.iter().position(|g| g.kind == CameraKind::Route && g.route_key() == (record.id, bit as u16))
            {
                return Some((g, fraction));
            }
        }
    }
    None
}

/// The entry a fraction lies in (0x447910): the last whose start is at or
/// before it. A fraction outside 0..1 has none (0x44FE80).
pub fn entry_index(track: &CameraTrack, fraction: f32) -> Option<usize> {
    if !(0.0..=1.0).contains(&fraction) {
        return None;
    }
    (0..track.entries.len()).rev().find(|&i| track.entries[i].start <= fraction)
}

/// The shot the camera shows at `fraction`: the first entry of the chain
/// of entries ([`chains`]) holding the entry then. A change of shot is a
/// cut (the camera jumps), drawn without a blend between the updates.
pub fn shot(track: &CameraTrack, fraction: f32) -> Option<usize> {
    let mut i = entry_index(track, fraction)?;
    while i > 0 && chains(&track.entries[i - 1], &track.entries[i]) {
        i -= 1;
    }
    Some(i)
}

/// Whether one entry's end is the next one's start (0x44A160 → 0x4434F0,
/// and inline in 0x44F7B0 at 0x44F8E8; **verified**): the next entry is a
/// camera (type ≠ 0), the tangent scales a +0x14 and b +0x10 and the
/// lenses a +0x4C and b +0x48 exactly equal, the same type and spaces
/// (+0, +4, +5), and the eye and look ends within 1e-5 per coordinate,
/// inclusive.
pub(super) fn chains(a: &CameraEntry, b: &CameraEntry) -> bool {
    let near = |p: [f32; 3], q: [f32; 3]| p.iter().zip(q).all(|(x, y)| (x - y).abs() <= 1e-5);
    b.mover != 0
        && a.tangent[1] == b.tangent[0]
        && a.fov[1] == b.fov[0]
        && a.mover == b.mover
        && a.eye_space == b.eye_space
        && a.look_space == b.look_space
        && near(a.eye[1], b.eye[0])
        && near(a.look[1], b.look[0])
}

/// A record's fraction of the car's race coordinate (0x5D4550,
/// **verified**): span = hi − lo (+ the lap length L 0x88F2B0 when
/// negative); 0 when |span| ≤ 0.001; else the record is re-centred on the
/// half lap: mid = (lo + span / 2) mod⁺ L, k = L / 2 − mid, and the result
/// is ((k + along) mod⁺ L − (k + lo) mod⁺ L) / span — continuous through
/// the record and below 0 / above 1 outside it, never "none" (0x43C240's
/// positive modulo).
pub fn record_fraction(record: &RouteRecord, along: f32, length: f32) -> f32 {
    let (lo, hi) = record.coord_range;
    let mut span = hi - lo;
    if span < 0.0 {
        span += length;
    }
    if span.abs() <= 0.001 {
        return 0.0;
    }
    let m = length.abs();
    let modp = |x: f32| if m > 0.0 { x.rem_euclid(m) } else { x };
    let mid = modp(lo + span * 0.5);
    let k = length * 0.5 - mid;
    (modp(k + along) - modp(k + lo)) / span
}

/// The route group of the car's own road record (0x44F9A0, **verified**:
/// only with the race coordinates built, 0x88F2BE; the record is the route
/// manager's 0x88A334[road node +0x0C & 0x7FF] of the car's controller's
/// road node, car +0x2C → +0x0C, via 0x5EC0B0 / 0x40F690), keyed by (its
/// id, its flags bit 1) through 0x4429C0 — one record, no search over
/// overlapping ones, no second try with the other direction. Returns the
/// group and [`record_fraction`].
pub fn route_group_of(
    groups: &[CameraGroup],
    record: Option<&RouteRecord>,
    built: bool,
    length: f32,
    along: f32,
) -> Option<(usize, f32)> {
    let record = record.filter(|_| built)?;
    let key = (record.id, (record.flags >> 1 & 1) as u16);
    let g = groups.iter().position(|g| g.kind == CameraKind::Route && g.route_key() == key)?;
    Some((g, record_fraction(record, along, length)))
}

/// The entry at a fraction as the route draw's candidate loop takes it
/// (0x447910, **verified**): the last whose start is at or before it, entry
/// 0 for a fraction before the first start, so the last entry past 1.
pub fn entry_at(track: &CameraTrack, fraction: f32) -> Option<usize> {
    if track.entries.is_empty() {
        return None;
    }
    Some((0..track.entries.len()).rev().find(|&i| track.entries[i].start <= fraction).unwrap_or(0))
}

/// The seconds a track keeps the car's entry `i` (0x44F7B0): from the
/// fraction to the start of the first entry after the chain of entries that
/// chain on, over the car's signed speed. A still car gets 1000 s.
pub fn run_time(track: &CameraTrack, i: usize, fraction: f32, span: f32, speed: f32) -> f32 {
    if speed.abs() <= 1e-4 {
        return 1000.0;
    }
    let mut j = i;
    while j + 1 < track.entries.len() && chains(&track.entries[j], &track.entries[j + 1]) {
        j += 1;
    }
    let end = track.entries.get(j + 1).map_or(1.0, |e| e.start);
    ((end - fraction) * span / speed).max(0.0)
}

#[derive(Clone, Debug, Default)]
pub struct TrackDirector {
    pub shot: Option<Shot>,
    /// This frame's shot's look point when its entry has the push flag +7
    /// (0x453F50, a route track: 0x444530(view, no car, &look) — the
    /// world push from the look point, without the car's sphere step; its
    /// result is not kept). The caller pushes the camera from it.
    pub push_from: Option<Vec3>,
}

impl TrackDirector {
    /// One director frame on automatic (0x5FB060 → 0x44FE80): the held
    /// shot's view while its entry is still a track camera (mover above 1),
    /// else a new pick among the group's tracks whose entry at the car's
    /// fraction is a track camera and runs MIN_RUN or more, one at random
    /// (0x44F9A0). None when no track is showing (the chase views then).
    /// `may_draw` (0x5FB060's bVar5): with no shot held, whether a new one
    /// may be drawn; a held shot whose entry ended is drawn again anyway.
    // prov: open track_camera.visible-2
    /// **Open**: the typed groups.
    #[allow(clippy::too_many_arguments)]
    pub fn direct(
        &mut self,
        groups: &[CameraGroup],
        records: &[RouteRecord],
        length: f32,
        along: f32,
        backward: bool,
        car: Mat4,
        speed: f32,
        rng: &mut GameRng,
        view: &View,
        may_draw: bool,
        blocked: &mut dyn FnMut(Vec3, Vec3) -> bool,
    ) -> Option<Camera> {
        self.push_from = None;
        if self.shot.is_none() && !may_draw {
            return None;
        }
        let Some((group, fraction)) = route_group(groups, records, length, along, backward) else {
            self.shot = None;
            return None;
        };
        if let Some(shot) = self.shot.as_ref().filter(|s| s.group == group)
            && let Some((cam, push)) = follow(&groups[group], shot.track, fraction, car, shot.frame)
        {
            self.push_from = push;
            return Some(cam);
        }
        // A new shot: the group's tracks whose entry at the fraction is a
        // track camera and runs long enough.
        let span = route_span(records, groups, group, length);
        let candidates: Vec<usize> = groups[group]
            .tracks
            .iter()
            .enumerate()
            .filter_map(|(ti, track)| {
                let i = entry_index(track, fraction)?;
                (track.entries[i].mover != 0
                    && run_time(track, i, fraction, span, speed) >= MIN_RUN
                    && visible(&track.entries[i], car, view, blocked))
                .then_some(ti)
            })
            .collect();
        if candidates.is_empty() {
            self.shot = None;
            return None;
        }
        let track = candidates[rng.below(candidates.len() as u32) as usize];
        self.shot = Some(Shot { group, track, frame: car });
        let (cam, push) = follow(&groups[group], track, fraction, car, car)?;
        self.push_from = push;
        Some(cam)
    }
}

impl TrackDirector {
    /// One director frame on automatic, native (0x5FB060 → 0x44FE80 →
    /// 0x44F9A0, **verified** unless marked): `record` is the car's own road
    /// record and `built` the race coordinates (see [`route_group_of`]);
    /// `fresh` the director's +0x224 (set by the cameras' reset 0x5FAF90,
    /// cleared when a track camera starts).
    ///
    /// * A held shot whose entry at the car's fraction is a track camera
    ///   (type > 1) keeps showing; no draw.
    /// * A held shot whose entry is type 1 is dropped and nothing is drawn
    ///   this frame (0x5FB32F `cmp ecx,1; jg` fails → +0x220 = 0): the
    ///   caller goes back to the chase mover and its draw, as when a shot
    ///   ends.
    /// * A held shot whose entry is type 0 (or none) draws again whatever
    ///   the gate says; with no shot, `may_draw` (bVar5) decides.
    /// * Before the draw, with +0x224 clear, one 0x43C1F0(1.0) on the
    ///   shared generator (0x5FB2EA, compared with 0.0 — always passes —
    ///   so it only steps the generator).
    /// * The draw: the group's tracks whose entry at the fraction
    ///   ([`entry_at`]) is not type 0, runs [`MIN_RUN`] or more (0x44F7B0)
    ///   and passes [`visible`]; one by 0x43C1C0(count). A picked track
    ///   whose entry is type 1 is not kept (no camera; next frame's gate
    ///   applies).
    ///
    /// `blocked` is the sight test plus, for the native result, the car
    /// test [`eye_in_car`] (0x448280 runs both on the same point).
    #[allow(clippy::too_many_arguments)]
    pub fn direct_on(
        &mut self,
        groups: &[CameraGroup],
        record: Option<&RouteRecord>,
        built: bool,
        length: f32,
        along: f32,
        car: Mat4,
        speed: f32,
        rng: &mut GameRng,
        view: &View,
        may_draw: bool,
        fresh: bool,
        blocked: &mut dyn FnMut(Vec3, Vec3) -> bool,
    ) -> Option<Camera> {
        self.push_from = None;
        let on = route_group_of(groups, record, built, length, along);
        let draw = match self.shot.as_ref() {
            Some(shot) => {
                // The held entry (0x442DE0): 0 when the car left its group
                // or the fraction is outside the track.
                let kind = on
                    .filter(|&(g, _)| g == shot.group)
                    .and_then(|(g, f)| {
                        let t = groups[g].tracks.get(shot.track)?;
                        Some(t.entries[entry_index(t, f)?].mover)
                    })
                    .unwrap_or(0);
                if kind > 1 {
                    let (g, f) = on?;
                    let (cam, push) = follow(&groups[g], shot.track, f, car, shot.frame)?;
                    self.push_from = push;
                    return Some(cam);
                }
                if kind == 1 {
                    self.shot = None;
                    return None;
                }
                true
            }
            None => may_draw,
        };
        if !draw {
            return None;
        }
        if !fresh {
            let _ = rng.unit(1.0);
        }
        let Some((group, fraction)) = on else {
            self.shot = None;
            return None;
        };
        let span = record.map_or(length, |r| {
            let s = r.coord_range.1 - r.coord_range.0;
            if s < 0.0 { s + length } else { s }
        });
        let candidates: Vec<usize> = groups[group]
            .tracks
            .iter()
            .enumerate()
            .filter_map(|(ti, track)| {
                let i = entry_at(track, fraction)?;
                (track.entries[i].mover != 0
                    && run_time(track, i, fraction, span, speed) >= MIN_RUN
                    && visible(&track.entries[i], car, view, blocked))
                .then_some(ti)
            })
            .collect();
        if candidates.is_empty() {
            self.shot = None;
            return None;
        }
        let track = candidates[rng.below(candidates.len() as u32) as usize];
        match follow(&groups[group], track, fraction, car, car) {
            Some((cam, push)) => {
                self.shot = Some(Shot { group, track, frame: car });
                self.push_from = push;
                Some(cam)
            }
            None => {
                self.shot = None;
                None
            }
        }
    }
}

/// The camera of a track at the fraction, if its entry there is a track
/// camera (mover above 1, 0x5FB060; else None, and the chase views show).
/// An entry with the frozen flag (+8) uses the car's frame from the shot's
/// start.
pub(super) fn follow(
    group: &CameraGroup,
    track: usize,
    fraction: f32,
    car: Mat4,
    frame: Mat4,
) -> Option<(Camera, Option<Vec3>)> {
    let track = group.tracks.get(track)?;
    let i = entry_index(track, fraction)?;
    let entry = &track.entries[i];
    if entry.mover <= 1 {
        return None;
    }
    let frame = if entry.frozen_frame { frame } else { car };
    let cam = camera(track, i, fraction, frame)?;
    let push = entry.avoid[1].then(|| {
        let end = track.entries.get(i + 1).map_or(1.0, |n| n.start);
        let t = if end > entry.start { ((fraction - entry.start) / (end - entry.start)).clamp(0.0, 1.0) } else { 0.0 };
        to_world(path(entry.look, end_tangents(track, i).1, entry.spline, t), entry.look_space, frame, None, None)
    });
    Some((cam, push))
}

/// The span (m) of the record the group's fraction runs over: the record's
/// coordinate range, wrapped by the road length (0x5D4530).
pub(super) fn route_span(records: &[RouteRecord], groups: &[CameraGroup], group: usize, length: f32) -> f32 {
    let key = groups[group].route_key();
    records
        .iter()
        .find(|r| (r.id, (r.flags >> 1 & 1) as u16) == key)
        .map(|r| {
            let span = r.coord_range.1 - r.coord_range.0;
            if span < 0.0 { span + length } else { span }
        })
        .unwrap_or(length)
}
