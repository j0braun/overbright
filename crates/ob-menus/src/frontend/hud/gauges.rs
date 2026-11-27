pub use super::super::format::grouped_int;

/// What the drift element shows (0x4C85D0 with 0x4AB780 and 0x4AB340).
#[derive(Clone, Debug, Default)]
pub struct DriftHud {
    /// The drift trick's state: running or pending, banked this frame,
    /// lost to a hit.
    pub running: bool,
    pub banked_now: bool,
    pub lost: bool,
    /// This drift's points (+0x24), the banked points (+0x2C) and the best
    /// drift (+0x44), truncated.
    pub points: i32,
    pub banked: i32,
    pub best: i32,
    /// The multiplier level and the points the drift needs for the next
    /// (0 at the top).
    pub level: u32,
    pub next: i32,
    /// A drift tier to announce (its name) and how long ago, s.
    pub announce: Option<(u32, f32)>,
    /// The drift angle the meter shows, degrees (0x4AB390: |slip|, at most
    /// 90, 0 in reverse or rolling backwards).
    pub angle: f32,
    /// The level is falling / may not rise (drift manager +0x16 / +0x17).
    pub falling: bool,
    pub held: bool,
    /// An off-road or proximity factor over 1 (0x4C7EA0): the popup turns
    /// orange.
    pub bonus: bool,
}

/// The drag tachometer's values (0x4C7090).
#[derive(Clone, Debug, Default)]
pub struct DragHud {
    /// The last up-shift's rating (transmission +0x68, 0x5ECEB0).
    pub shift_quality: f32,
    /// The shift light's rating, None when off
    /// (`Drivetrain::drag_shift_light`).
    pub shift_light: Option<f32>,
    /// The needle's blend toward blue while the light is blue:
    /// (rpm − (up-shift rpm − window)) / (0.5 window), window =
    /// max(500, 0x594F10) (0x4C7A13).
    pub needle_blend: f32,
    /// Engine heat for HEAT_Bar (0x5ECCE0: engine +0x8C clamped to 0..1,
    /// drivetrain kind 1, else 0).
    pub heat: f32,
    /// The needle's angles at 0 and at the maximum rpm, degrees (car
    /// record +0x7FC / +0x800, `Handling::drag_needle`).
    pub needle: [f32; 2],
}

/// The drag needle's angle (0x4AB210 with one local human): from a to b
/// (b ≤ a wraps through 360) by rpm / max rpm clamped to 0..1, the result
/// kept within ±360.
pub fn drag_needle_angle(rpm: f32, max_rpm: f32, [a, b]: [f32; 2]) -> f32 {
    let t = (rpm / max_rpm).clamp(0.0, 1.0);
    let span = if b <= a { 360.0 - a + b } else { b - a };
    let mut angle = a + span * t;
    if angle > 360.0 {
        angle -= 360.0;
    }
    if angle < -360.0 {
        angle += 360.0;
    }
    angle
}

/// The turbo needle's angle in degrees (0x4C6B80 → 0x4AA160).
pub fn turbo_needle(boost: f32, drag: bool) -> f32 {
    let (a, c) = if drag { (158.0, 96.0) } else { (-14.0, 222.0) };
    let mut v = (c - a) * (boost + 1.0) * 0.5 + a;
    if v > 360.0 {
        v -= 360.0;
    }
    if v < 0.0 {
        v = 360.0 - v;
    }
    v
}
