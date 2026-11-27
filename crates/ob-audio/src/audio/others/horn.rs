use glam::Vec3;

/// CARSFX_TrafficHorn's state.
#[derive(Default)]
pub(super) struct Horn {
    pub(super) event: Option<u32>,
    pub(super) variant: i32,
    /// +0xBC start, +0xB8 length, +0xC0 last stop, +0xC8 last refusal.
    pub(super) start: f32,
    pub(super) length: f32,
    pub(super) stopped: f32,
    pub(super) refused: f32,
    /// +0xC5: playing its length.
    pub(super) timed: bool,
    /// The fade (+0xD0): time, duration, from, to, value, done; +0xEC.
    pub(super) fade: (f32, f32, f32, f32, f32, bool),
    pub(super) fading: bool,
}

impl Horn {
    pub(super) fn set_fade(&mut self, dur: f32, from: f32, to: f32) {
        self.fade = (0.0, dur, from, to, from, false);
    }

    /// 0x46B100 (type 0: linear).
    pub(super) fn step_fade(&mut self, dt: f32) {
        let f = &mut self.fade;
        if f.5 {
            return;
        }
        f.0 += dt;
        if f.0 > f.1 {
            f.4 = f.3;
            f.5 = true;
        } else {
            f.4 = (f.3 - f.2) * (f.0 / f.1) + f.2;
        }
    }
}

/// 0x475B30: whether a traffic car at `pos` moving with `vel` honks at
/// the player's car (`player`, `forward`).
pub fn horn_aims(pos: Vec3, vel: Vec3, player: Vec3, forward: Vec3, drag: bool) -> bool {
    if player.distance(pos) > 20.0 {
        return false;
    }
    let d = Vec3::new(player.x - pos.x, player.y - pos.y, 0.0).normalize_or_zero();
    let n = Vec3::new(vel.x, vel.y, 0.0).normalize_or_zero();
    if drag {
        return n.dot(d) >= 3.577_925_f32.cos();
    }
    n.dot(d) >= 2.268_928_f32.cos() && forward.normalize_or_zero().dot(n) <= 1.919_862_2_f32.cos()
}
