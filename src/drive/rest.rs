use super::*;

impl CarState {
    /// Whether the body has a live contact record (body +0x18).
    pub(super) fn in_contact(&self) -> bool {
        !self.records.is_empty()
    }

    /// Whether a live contact is with a track polygon that is upright
    /// (0x5A0170: body +0x60 → +0x78 under 0.1); car contacts do not count.
    /// The at-rest classifier (Car_RestClassify 0x5992C0) on the body
    /// after a step: 0 when a hydraulics bounce input (controls
    /// +0x1EC…+0x1F8, 0x5ED2A0) is on or a corner's hydraulics state
    /// (mover +0x74 + 4i, 0x5ED480) is 2 or 3 (moving); else, with the gas (controls +0x20C) at most 0.3, or
    /// neutral (car +0x434 = 1), or no wheel down (+0x424 < 1) — or car
    /// +0x428 < 3, **not traced**, left out — and |v| and |ω| both at most
    /// 0.7, the forward and side axes' z (body +0x38, +0x48) within ±0.15
    /// unless no wheel is down, no recent hit (0x5EBFB0: the ghost timer
    /// +0x974 / +0x978), and a wheel down or a contact with a track
    /// polygon whose |n.z| < 0.7 (0x5ED230 → 0x5A01C0) or the class 2
    /// last time: 2 when |v| and |ω| are both at most 0.25, else 1.
    pub(super) fn rest_class(&self, gas: f32, neutral: bool, rot: Mat3) -> u8 {
        if self.hydraulics.input.bouncing() || self.hydraulics.moving() > 0 {
            return 0;
        }
        let down = self.wheels_down();
        if !(gas <= 0.3 || neutral || down < 1) {
            return 0;
        }
        let (v, w) = (self.velocity.length(), self.spin.length());
        if v > 0.7 || w > 0.7 {
            return 0;
        }
        let (fwd, side) = (rot * Vec3::X, rot * Vec3::Y);
        if !((fwd.z.abs() <= 0.15 && side.z.abs() <= 0.15) || down < 1) {
            return 0;
        }
        if self.ghost_timer > 0.0 {
            return 0;
        }
        let wall = self.contacts.iter().any(|n| n.wall);
        if !(down > 0 || wall || self.rest == 2) {
            return 0;
        }
        if v <= 0.25 && w <= 0.25 { 2 } else { 1 }
    }

    pub(super) fn upright_contact(&self) -> bool {
        self.contacts.iter().any(|n| n.touching == Touching::Track { upright: true })
    }
}
