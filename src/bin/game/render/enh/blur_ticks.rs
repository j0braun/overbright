//! The motion blur's motion on the ticks (docs/uncapped-fps.md, "Motion
//! blur on ticks"): the blur takes a frame's motion from the drawn state
//! one tick before it, not from the last drawn frame, and its noise phase
//! from the ticks it drew on, so a frame on a tick blurs the same at
//! every display rate. Each tick's end records what the blur reads of the
//! drawn state at that tick (the view's key, the camera, the cars' drawn
//! transforms by slot, the blur clock) in a ring of four
//! (`Snapshots::blur`); a frame picks its tick before from it
//! (`Game::frame_blur`, in `frame_poses`, a pure read).
//!
//! At a steady 60 Hz the tick before is the last drawn frame, so the
//! window's frames blur as before. The harness's plain steps record
//! nothing (no tick capture): those frames keep the last-drawn-frame path
//! (shots, the default benches, byte-identical).
use crate::*;
use nfsu2::render::blur_dof::{self, Last, MotionMode, TickMotion};
use std::collections::HashMap;

/// The ticks kept: a frame needs its tick and the one before, and between
/// two ticks the one before that.
const KEEP: usize = 4;

/// One tick's state as the blur reads it, and its noise phase.
#[derive(Clone, Debug)]
pub(crate) struct BlurTick {
    pub(crate) tick: u64,
    /// The drawn state at the tick (`aspect` is the frame's, set there).
    pub(crate) at: Last,
    /// The ticks the blur drew on up to and including this one.
    pub(crate) phase: u32,
}

impl Game {
    /// The end of a tick (`capture_tick`, after the clocks): what the blur
    /// reads at it kept, with or without a drawn frame on it. Only while
    /// the motion blur is on (else the ring is emptied: the blur starts
    /// anew, as the drawn-frame path does when it goes off).
    pub(crate) fn record_blur_tick(&mut self, tick: u64, time: f64) {
        // The tick's settings: the record belongs to the tick's state.
        let on = MotionMode::from_tier(self.pacing.tick_settings.motion_blur) != MotionMode::Original;
        let Some(d) = self.sim.drive.as_ref().filter(|_| on) else {
            self.interp.blur.clear();
            return;
        };
        let mut cars: HashMap<u64, glam::Mat4> = HashMap::with_capacity(16);
        cars.insert(0, d.state.transform());
        if let Some(c) = self.scenes.shop.as_ref().and_then(|run| run.companion.as_ref()) {
            cars.insert(1, c.state.transform());
        }
        // The slots `temporal_cars` gives the frame's cars (a car the frame
        // does not draw is only never looked up).
        let race = match &self.mode {
            Mode::Race(r) => Some(r),
            _ => None,
        };
        if let Some(r) = race {
            cars.extend(r.racers.rivals.iter().enumerate().map(|(i, x)| (100 + i as u64, x.state.transform())));
        }
        if matches!(self.mode, Mode::Explore)
            && let Some(e) = self.sim.encounters.as_ref()
        {
            cars.extend(e.cars.iter().enumerate().filter(|(_, c)| c.active).map(|(i, c)| (200 + i as u64, c.state.transform())));
        }
        let traffic = match &self.mode {
            Mode::Explore => self.sim.traffic.as_ref(),
            Mode::Race(r) => r.racers.traffic.as_ref().filter(|t| !t.paused),
            _ => None,
        };
        if let Some(t) = traffic {
            cars.extend(
                t.cars.iter().enumerate().filter(|(_, c)| crate::render::frame::traffic_drawn(c)).map(|(i, c)| (1000 + i as u64, c.state.transform())),
            );
        }
        let key = render::enh::post::temporal_key(&self.mode, self.view_history_key(), self.scenes.shop.is_some());
        let at = Last { key, camera: d.camera.unjittered(), aspect: 0.0, cars, time };
        let ring = &mut self.interp.blur;
        let before = ring.back().filter(|b| b.tick + 1 == tick);
        let phase = before.map_or(0, |b| b.phase + u32::from(blur_dof::tick_draws(&b.at, &at)));
        if ring.back().is_some_and(|b| b.tick >= tick) {
            ring.clear();
        }
        if ring.len() >= KEEP {
            ring.pop_front();
        }
        ring.push_back(BlurTick { tick, at, phase });
    }

    /// The motion of a frame standing at tick `newest` − 1 + `alpha`
    /// (`on_newest`: the frame shows the newest tick itself): the drawn
    /// state one tick before the frame's and the noise phase. On a tick
    /// (α = 0, or the newest): that tick's record before it, if the 60 Hz
    /// frame of the tick drew the blur (else no blur); between two ticks
    /// the records before both blended by α. None (the last drawn frame's
    /// path) where the ring lacks the ticks (the harness's plain steps).
    pub(crate) fn frame_blur(&self, newest: u64, alpha: f32, on_newest: bool) -> Option<TickMotion> {
        let ring = &self.interp.blur;
        let get = |t: u64| ring.iter().find(|b| b.tick == t);
        let on = |t: u64| -> Option<TickMotion> {
            let cur = get(t)?;
            let prev = get(t.checked_sub(1)?);
            let last = prev.filter(|p| blur_dof::tick_draws(&p.at, &cur.at)).map(|p| p.at.clone());
            // No record before it: the blur's first tick (no blur), as the
            // drawn-frame path's first frame.
            Some(TickMotion { last, phase: cur.phase })
        };
        if on_newest {
            return on(newest);
        }
        let n1 = newest.checked_sub(1)?;
        if alpha <= 0.0 {
            return on(n1);
        }
        // Between ticks n − 1 and n: the state one tick earlier is between
        // n − 2 and n − 1 at the same α.
        let (a, b) = (on(n1)?, on(newest)?);
        let Some(pb) = b.last else {
            // A cut at tick n: no blur until it.
            return Some(TickMotion { last: None, phase: a.phase });
        };
        let Some(pa) = a.last else {
            // A cut at tick n − 1: the motion since tick n − 1.
            return Some(TickMotion { last: Some(pb), phase: a.phase });
        };
        let pb = &pb;
        let camera = nfsu2::interpolate::camera_at(&pa.camera, &pb.camera, alpha).unwrap_or(pb.camera);
        let cars = pb
            .cars
            .iter()
            .filter_map(|(k, m)| {
                let a = pa.cars.get(k)?;
                Some((*k, nfsu2::interpolate::transform_at(a, m, alpha)?))
            })
            .collect();
        let time = pa.time + (pb.time - pa.time) * f64::from(alpha);
        Some(TickMotion { last: Some(Last { key: pb.key, camera, aspect: 0.0, cars, time }), phase: a.phase })
    }
}
