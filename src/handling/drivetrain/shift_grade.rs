use super::*;

/// Drag-race shift windows by performance class and gear, rpm (0x7FBF60).
pub(super) const SHIFT_WINDOW: [[f32; 9]; 4] = [
    [0.0, 560.0, 360.0, 175.0, 160.0, 140.0, 140.0, 0.0, 0.0],
    [0.0, 600.0, 400.0, 230.0, 220.0, 160.0, 160.0, 0.0, 0.0],
    [0.0, 850.0, 550.0, 300.0, 250.0, 200.0, 180.0, 0.0, 0.0],
    [0.0, 1100.0, 750.0, 400.0, 325.0, 290.0, 230.0, 0.0, 0.0],
];
/// The player's shift-window scale in a drag race, by difficulty (0x7A19F0
/// at 0x89E7F4, drivetrain +0xE4; set at 0x5B6130).
pub(super) const SHIFT_WINDOW_SCALE: [f32; 3] = [3.0, 1.5, 1.0];

/// 0x5B6130 (0x5B6162..0x5B6198): a human racer's drivetrain +0xE4 in a
/// drag race (0x89E7D8 / 0x890108) outside the career (0x850078 ≠ 1) is
/// 0x7A19F0[difficulty 0x89E7F4]; otherwise it is left at 1.0.
pub fn drag_shift_scale(drag: bool, career: bool, difficulty: usize) -> f32 {
    if drag && !career { SHIFT_WINDOW_SCALE[difficulty.min(2)] } else { 1.0 }
}

/// An AI racer's drivetrain clutch factor in a drag race by difficulty
/// (0x7A19FC, drivetrain +0xE8; 0x5B61DE, outside the career only).
pub(super) const DRAG_AI_CLUTCH_SCALE: [f32; 3] = [0.4, 0.8, 1.0];

/// 0x5B6130 (0x5B619E..0x5B626A) for a racer whose controller is an AI
/// racer's (car +0x14 → +4 = 2) with owner kind 2, 3 or 4 in a drag race
/// (0x89E7D8 / 0x890108): outside the career (0x850078 ≠ 1) the clutch
/// factor +0xE8 = 0x7A19FC[difficulty]; then, for a skill row (controller
/// vt+0x78) of 0..=9, the shift offsets' ranges from the AI tuning
/// (0x81CB60 + 0x68·row: +0x558/+0x55C up, +0x550/+0x554 down,
/// +0x560/+0x564 2nd gear; [`crate::ai::route::AiTuning::shift_ranges`]).
/// Rows outside 0..=9 keep the constructor's ranges (the clamp at 0x5B6204
/// then fails the equality test at 0x5B6210).
pub fn drag_ai_shift_setup(h: &mut Handling, career: bool, ranges: Option<[[f32; 2]; 3]>) {
    if !h.race.drag() {
        return;
    }
    if !career {
        h.clutch_scale = DRAG_AI_CLUTCH_SCALE[h.difficulty.min(2)];
    }
    if let Some(r) = ranges {
        h.shift_ranges = r;
    }
}

impl Drivetrain {
    /// The HUD's shift light (0x4C68D0 with 0x5ECD40 and 0x594F10): lit
    /// once the engine passes the gear's up-shift rpm less 0.95 × its
    /// shift window.
    pub fn shift_light(&self, h: &Handling) -> bool {
        let gear = self.gear.min(8);
        let Some(&up) = self.up_rpm.get(gear) else { return false };
        let window = SHIFT_WINDOW[h.performance_class.min(3)][gear] * h.shift_window_scale;
        self.rpm() > up - 0.95 * window
    }

    /// The drag tachometer's shift light (0x4C7090 with 0x594F30 without
    /// the rating flag, 0x594F10 and 0x5ECD40): off while shifting or, with
    /// no rating, more than max(500, window) under the up-shift rpm;
    /// otherwise the rating (−1 past the point: red; up to 0.5 blue; over
    /// 0.5 green). Without the flag 0x594F30 returns the unquantised
    /// 1 − early / (0.95 window) (verified: the 0.5 / 0.85 steps sit under
    /// the flag test; 0.95 is 0x7FBF5C).
    ///
    /// `gear` is the tachometer's last gear (HUD +0x80, the car's +0x434
    /// of the previous update, 0x4C7A72), and the rpm is the car's shown
    /// rpm (+0x440 unless the override 0x7FBED8 is not 100; 0x4C711A..
    /// 0x4C713E → 0x4C77BC), the same rpm the grading reads (verified).
    /// So the light's blue starts max(500, w) under the shift point while
    /// a shift grades above 0 only from 0.95 w under it: between the two
    /// the light is blue and an up-shift is "Short Shift!" (0x4C784C..
    /// 0x4C78DE; native).
    pub fn drag_shift_light(&self, h: &Handling, gear: usize) -> Option<f32> {
        let gear = gear.min(8);
        let up = *self.up_rpm.get(gear)?;
        let window = SHIFT_WINDOW[h.performance_class.min(3)][gear] * h.shift_window_scale;
        let rpm = self.choice_rpm();
        let early = up - rpm;
        let rating = if window <= 0.0 {
            0.0
        } else if early < 0.0 {
            if early.abs() < (1.0 - 0.95) * window { 1.0 } else { -1.0 }
        } else {
            (1.0 - early / (0.95 * window)).max(0.0)
        };
        if rating == 0.0 && rpm < up - window.max(500.0) {
            return None;
        }
        if self.pending { None } else { Some(rating) }
    }

    /// The drag needle's blend toward blue (0x4C7A13): (rpm − (up-shift
    /// rpm − w)) / (0.5 w), w = max(500, the window) (0x4C77D7), on the
    /// shown rpm and the tachometer's last gear as [`Self::drag_shift_light`].
    pub fn drag_needle_blend(&self, h: &Handling, gear: usize) -> f32 {
        let gear = gear.min(8);
        let Some(&up) = self.up_rpm.get(gear) else { return 0.0 };
        let w = (SHIFT_WINDOW[h.performance_class.min(3)][gear] * h.shift_window_scale).max(500.0);
        (self.choice_rpm() - (up - w)) / (0.5 * w)
    }

    /// 0x594F30 with the rating flag: how well an up-shift at `rpm` out of
    /// `gear` hit the shift point. Within 5% of the window past it, 1;
    /// further past, −1; before it, 1 − early/(0.95 window), rated 0.85 at
    /// 0.5 or more, else 0.5, and 0 when too early.
    pub(in crate::handling) fn shift_quality(&self, h: &Handling, rpm: f32, gear: usize) -> f32 {
        let early = self.up_rpm[gear] - rpm;
        let window = SHIFT_WINDOW[h.performance_class.min(3)][gear.min(8)] * h.shift_window_scale;
        if window <= 0.0 {
            return 0.0;
        }
        if early < 0.0 {
            return if early.abs() < (1.0 - 0.95) * window { 1.0 } else { -1.0 };
        }
        let q = 1.0 - early / (0.95 * window);
        if q <= 0.0 {
            0.0
        } else if q < 0.5 {
            0.5
        } else {
            0.85
        }
    }
}
