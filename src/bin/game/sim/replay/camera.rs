use crate::*;

/// The route cameras of a track's bundle (0x44A6C0's chunk 0x8003B200);
/// none when the bundle has no such chunk.
pub(crate) fn track_cameras(root: &std::path::Path, region: &str) -> Vec<CameraGroup> {
    let Ok(data) = nfsu2::game_fs::read(&root, &format!("TRACKS/{region}.BUN")) else { return Vec::new() };
    chunks(&data, 0).find(|c| c.id == ROUTE_CAMERAS).and_then(|c| parse_camera_tracks(c).ok()).unwrap_or_default()
}

/// What the typed cameras' weight functions read of the replayed car
/// (docs/replay-cameras.md): the nitrous shot, upside down (car +0xB8 < 0),
/// the forward speed (+0x42C) and speed (+0x430), the set-up's +0x20, the
// prov: inferred game.replay_typed_input
/// path's curvature (0x5EB320; **inferred** here from the velocity's turn
/// over the frame, the game's ring of logged points not being traced) and
// prov: open game.replay_typed_input-2
/// the slip angle (0x59FA20). The flight prediction (Jump) is **open**.
pub(crate) fn replay_typed_input(
    d: &Drive,
    top: f32,
    last_velocity: Option<Vec3>,
    dt: f32,
) -> nfsu2::track_camera::TypedInput {
    let v = d.state.velocity;
    // Car +0x42C, the exported forward speed (the velocity
    // along the heading was wrong).
    let forward = d.state.exported_forward_speed();
    let curvature = last_velocity.filter(|_| dt > 0.0 && v.truncate().length() > 0.5).and_then(|p| {
        let (a, b) = (p.truncate(), v.truncate());
        if a.length() < 0.5 {
            return None;
        }
        let turn = a.perp_dot(b).atan2(a.dot(b));
        Some(turn / (b.length() * dt))
    });
    nfsu2::track_camera::TypedInput {
        nitrous: d.state.nitrous.and_then(|mut t| (t.level() > 0).then(|| t.fraction())),
        upside_down: (d.state.orientation * Vec3::Z).z < 0.0,
        long_flight: false,
        forward_speed: forward,
        speed: v.length(),
        top,
        curvature,
        slip: if forward >= 1.0 { d.state.slip_degrees().to_radians() } else { 0.0 },
        wheel_slip: exported_slip_sum(&d.state.wheels),
        rpm: d.state.shown_rpm,
    }
}

/// The Burnout camera's wheel slip (0x441D82..0x441D94): car +0x3B4 +
/// +0x3BC + +0x3C4 + +0x3CC, the four wheels' slip as the car exports it
/// (the wheel's +0x100, 0 while the tyre grips), summed in wheel order.
pub(crate) fn exported_slip_sum(wheels: &[nfsu2::drive::WheelState; 4]) -> f32 {
    wheels[0].exported_slip + wheels[1].exported_slip + wheels[2].exported_slip + wheels[3].exported_slip
}

/// The replay camera (the race camera director, 0x8900B4: 0x5FAFE0 and
/// 0x5FB060; docs/replay-cameras.md): `index` (+0x21C) 0 is the automatic
/// camera, 1–5 the chase views 0x802130[index] = VIEW_ORDER[index − 1];
/// `last` (+0x218) the index at the director's last frame; `view` the chase
/// view showing; `since` when a chase view was last drawn or the cameras
/// reset (+0x214, the playback clock here; None = 0, never stamped);
/// `fresh` (+0x224) set by the reset 0x5FAF90 and cleared when a track
/// camera starts (0x5FB5BD).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ReplayCamera {
    pub(crate) index: usize,
    pub(crate) last: usize,
    pub(crate) view: Option<u16>,
    pub(crate) since: Option<f32>,
    pub(crate) fresh: bool,
}

/// 0x802148: under this long on a drawn chase view no track camera is
/// drawn (unless the cameras were just reset).
pub(crate) const REPLAY_CHASE_MIN: f32 = 1.5;

/// 0x7A555C: between REPLAY_CHASE_MIN and this the chase view holds
/// outright; past it a track camera may come, or another chase view.
pub(crate) const REPLAY_CHASE_HOLD: f32 = 4.0;

impl ReplayCamera {
    /// BUTTON1 (0x5FAFE0): the next index of six; leaving the automatic
    /// camera skips index 1 when its view (2) is already showing. Index 0
    /// changes nothing until the director's next frame; +0x214 is not
    /// touched.
    pub(crate) fn cycle(&mut self) {
        let old = self.index;
        // Replay Cameras "Extended": the extra cameras after the six
        // (`nfsu2::replay_cams::EXTRAS`), then back to automatic.
        let count = if nfsu2::extras::replay_cameras() { 6 + nfsu2::replay_cams::EXTRAS.len() } else { 6 };
        self.index = (old + 1) % count;
        if self.index == 0 || self.index >= 6 {
            return;
        }
        if old == 0 && self.view == Some(VIEW_ORDER[self.index - 1]) {
            self.index = (self.index + 1) % 6;
        }
        self.view = Some(VIEW_ORDER[self.index.max(1) - 1]);
    }

    /// The race cameras' reset (0x5FAF90, from every snapshot restore
    /// 0x57C1E0, so at the replay's start and each skip): back on the chase
    /// view, +0x214 = now, +0x224 = 1.
    pub(crate) fn reset(&mut self, now: f32) {
        self.since = Some(now);
        self.fresh = true;
    }

    /// 0x5EA160: the real time since +0x214 (0 when never stamped).
    pub(crate) fn shown(&self, now: f32) -> f32 {
        self.since.map_or(0.0, |s| now - s)
    }

    /// 0x5FB060's gate before the track draw, on automatic (index 0) in
    /// RCSTATE 5: None while a chase view holds outright (mover type 1 and
    /// 1.5 s < shown < 4 s: the frame ends, +0x218 = +0x21C); else whether
    /// a track camera may be drawn when none is held (bVar5: not when a
    /// chase view was drawn under 1.5 s ago and no reset since). A held
    /// track whose entry ended is drawn again whatever this says.
    pub(crate) fn gate(&mut self, now: f32, on_track: bool) -> Option<bool> {
        let t = self.shown(now);
        if !on_track && t > REPLAY_CHASE_MIN && t < REPLAY_CHASE_HOLD {
            self.last = self.index;
            return None;
        }
        Some(!(self.since.is_some() && !self.fresh && t < REPLAY_CHASE_MIN))
    }

    /// A track camera (mover 0x12) started (0x5FB5B8: 0x5F1FE0(0x12),
    /// +0x224 = 0).
    pub(crate) fn track_started(&mut self, now: f32) {
        self.fresh = false;
        self.last = self.index;
        self.since = Some(now);
    }

    /// The director's chase branch in RCSTATE_INSTANT_REPLAY (0x5FB060, not
    /// while paused; no track camera drawn this frame): a chosen index
    /// holds its view; on automatic it draws a view (0x43C1F0 × 5 over the
    /// weights 1 at 0x7A553C for the views 0x802134) when a track shot has
    /// just ended (`track_ended`: back to the chase mover, 0x5FA830, then a
    /// draw over all five), when the index was just set back to 0 (+0x218
    /// ≠ 0: not the view showing unless the draw runs past the last
    /// weight), or with +0x214 stamped, over 4 s on the view and the car's
    /// forward speed (+0x430) at 10 m/s or more, with chance 0.3 a frame.
    /// A draw stamps +0x214.
    pub(crate) fn direct(&mut self, rng: &mut nfsu2::opponents::GameRng, speed: f32, now: f32, track_ended: bool) {
        let last = std::mem::replace(&mut self.last, self.index);
        if self.index != 0 {
            return;
        }
        let avoid = !track_ended && last != 0;
        let pick = self.view.is_none()
            || track_ended
            || avoid
            || (speed >= 10.0 && self.since.is_some() && self.shown(now) > REPLAY_CHASE_HOLD && rng.unit(1.0) < 0.3);
        if !pick {
            return;
        }
        let r = rng.unit(5.0);
        let mut sum = 0.0;
        let mut view = VIEW_ORDER[0];
        for &v in &VIEW_ORDER {
            sum += 1.0;
            view = v;
            if (!avoid || Some(v) != self.view) && r <= sum {
                break;
            }
        }
        self.view = Some(view);
        self.since = Some(now);
    }
}

#[cfg(test)]
mod slip_tests {
    use super::exported_slip_sum;

    /// The Burnout camera reads the car's exported slip (car +0x3B4 + 8i),
    /// not the wheels' own slip: gripping tyres turning a little faster
    /// than they roll add nothing.
    #[test]
    fn burnout_camera_reads_the_exported_slip() {
        let mut wheels = [nfsu2::drive::WheelState::default(); 4];
        for w in &mut wheels {
            w.slip = 0.33;
        }
        assert_eq!(exported_slip_sum(&wheels), 0.0);
        wheels[2].exported_slip = 11.0;
        wheels[3].exported_slip = 9.5;
        assert_eq!(exported_slip_sum(&wheels), 20.5);
    }
}
