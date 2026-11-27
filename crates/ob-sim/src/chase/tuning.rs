use glam::Vec3;

/// One coordinate easing towards a target (0x5749F0). Every frame the game
/// fits a cubic from the current value and slope to the target with zero
/// slope, then moves `dt / duration` of the way along it. The slope is kept
/// per unit of that curve, so the motion stays smooth when the target moves.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Ease {
    pub(super) value: f32,
    pub(super) slope: f32,
    pub(super) target: f32,
    pub(super) duration: f32,
}

impl Ease {
    pub(super) fn step(&mut self, dt: f32) {
        let span = self.target - self.value;
        let (d, c) = (self.value, self.slope);
        let a = c - 2.0 * span;
        let b = 3.0 * span - 2.0 * c;
        let t = if self.duration <= 1e-5 { 1.0 } else { dt / self.duration };
        // The curve is valid up to t = 10 (+0x28); past that it snaps.
        if t < 10.0 {
            self.value = ((a * t + b) * t + c) * t + d;
            self.slope = (3.0 * a * t + 2.0 * b) * t + c;
        } else {
            self.value = self.target;
            self.slope = 0.0;
        }
    }

    pub(super) fn snap(&mut self) {
        self.value = self.target;
        self.slope = 0.0;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Ease3([Ease; 3]);

impl Ease3 {
    pub(super) fn with_durations(d: Vec3) -> Ease3 {
        let mut e = Ease3::default();
        for (i, c) in e.0.iter_mut().enumerate() {
            c.duration = d[i];
        }
        e
    }
    pub(super) fn set_duration(&mut self, d: f32) {
        self.0.iter_mut().for_each(|c| c.duration = d);
    }
    pub(super) fn set_target(&mut self, v: Vec3) {
        for (i, c) in self.0.iter_mut().enumerate() {
            c.target = v[i];
        }
    }
    pub(super) fn step(&mut self, dt: f32) {
        self.0.iter_mut().for_each(|c| c.step(dt));
    }
    pub(super) fn snap(&mut self) {
        self.0.iter_mut().for_each(Ease::snap);
    }
    pub(super) fn value(&self) -> Vec3 {
        Vec3::new(self.0[0].value, self.0[1].value, self.0[2].value)
    }
}

/// A car-space vector scaled and clamped: how far a point lags behind
/// acceleration.
#[derive(Clone, Copy, Debug)]
pub(super) struct Lag {
    pub(super) scale: Vec3,
    min: Vec3,
    max: Vec3,
}

impl Lag {
    pub(super) fn apply(&self, v: Vec3, bias: Vec3) -> Vec3 {
        (v * self.scale + bias).clamp(self.min, self.max)
    }
}

/// One 0xB0-byte tuning record (tables at 0x7EC610, one per view kind).
#[derive(Clone, Copy, Debug)]
pub(super) struct Tuning {
    /// Ease durations, seconds: eye, look-at point, field of view, lean.
    pub(super) ease: [f32; 4],
    /// +0x10: how much the horizon leans with acceleration.
    pub(super) lean: Lag,
    /// +0x40: how far the eye moves with acceleration.
    pub(super) eye: Lag,
    /// +0x70: how far the look-at point moves with acceleration.
    pub(super) look: Lag,
    /// +0xA0: ease durations for the direction the chase frame faces.
    pub(super) heading_ease: Vec3,
}

impl Tuning {
    /// Each kind has two records, blended by a global set to 0.5
    /// (0x787154) — so the game always uses their average (0x446DC0).
    pub(super) fn blended(kind: u16) -> Tuning {
        let [a, b] = TUNING[(kind as usize).min(5)];
        let mix = |x: Vec3, y: Vec3| (x + y) * 0.5;
        let lag = |x: Lag, y: Lag| Lag { scale: mix(x.scale, y.scale), min: mix(x.min, y.min), max: mix(x.max, y.max) };
        Tuning {
            ease: std::array::from_fn(|i| (a.ease[i] + b.ease[i]) * 0.5),
            lean: lag(a.lean, b.lean),
            eye: lag(a.eye, b.eye),
            look: lag(a.look, b.look),
            heading_ease: mix(a.heading_ease, b.heading_ease),
        }
    }
}

/// Field of view per kind (0x7EB5B4), binary angle.
pub(super) const FIELD_OF_VIEW: [u16; 6] = [13020, 13020, 16384, 16384, 16384, 13020];

/// The chase frame follows the car's heading below 8 m/s and its direction
/// of travel above 13 m/s (graph at 0x7EC420, read at 0x445CD4).
pub(super) const FOLLOW_TRAVEL: (f32, f32) = (8.0, 13.0);

/// The order the camera button steps through: bumper, hood, near, far, high
/// (player setting → kind, 0x502500).
pub const VIEW_ORDER: [u16; 5] = [2, 4, 1, 0, 5];

/// Camera shake by speed (0x452ED0, graph at 0x7EC430): five rows over
/// speed / top speed × 4 of (sway amplitude, rough-surface amplitude, sway
/// rate, rough-surface rate).
pub(super) static SHAKE: crate::exe_tables::Table<[[f32; 4]; 5]> = crate::exe_tables::Table::new(|t| &t.shake);
/// Sway rates and amplitudes for the four channels (0x7EC480, 0x7EC490):
/// sideways, up, pitch (degrees), yaw (degrees).
/// The shake graph's speed scale for a car without its racer info
/// (0x452ED0: car +0x10 null); else S = the gearing top speed.
pub(super) const SHAKE_TOP_SPEED: f32 = 100.0;
pub(super) const SWAY_RATE: [f32; 4] = [7.5, 6.875, 6.125, 5.0];
pub(super) const SWAY_AMPLITUDE: [f32; 4] = [0.003, 0.002, 0.7, 0.5];
/// The rough-surface channel's rates and amplitudes (0x7EC4A0, 0x7EC4B0;
/// read from the image, **verified**), same channel order; scaled by the
/// wheels' surface rows (0x452ED0, see [`ChaseCamera::update`]).
pub(super) const ROUGH_RATE: [f32; 4] = [3.0, 5.0, 7.0, 5.5];
pub(super) const ROUGH_AMPLITUDE: [f32; 4] = [0.007, 0.01, 0.6, 0.8];
/// A surface's row of the surface table 0x800EC0 (0x98 bytes a row) as
/// the shake reads it: +0x38 (rate) and +0x3C (amplitude). Read from the
/// image, **verified** (the same values as the drive's road-roughness rows;
/// rows past 28 are taken as asphalt).
pub(super) fn surface_shake(index: u8) -> (f32, f32) {
    match index {
        8 | 15 => (1.0, 0.05),
        9 | 10 => (1.0, 0.02),
        11 => (3.0, 0.03),
        12 => (3.0, 0.048),
        13 | 17..=21 => (1.0, 0.01),
        14 => (1.0, 0.03),
        16 => (1.1, 0.03),
        _ => (1.0, 0.0),
    }
}

/// The game's value noise (0x4411B0): a hashed value per integer, linearly
/// interpolated, in −1..1.
pub(super) fn noise(x: f32) -> f32 {
    let hash = |n: i32| {
        let n = (n << 13) ^ n;
        let v =
            (n.wrapping_mul(n).wrapping_mul(15731).wrapping_add(789_221)).wrapping_mul(n).wrapping_add(1_376_312_589)
                & 0x7fff_ffff;
        1.0 - v as f32 / 1_073_741_824.0
    };
    let i = x as i32;
    let t = x - i as f32;
    hash(i) * (1.0 - t) + hash(i.wrapping_add(1)) * t
}

/// Six octaves of [`noise`], halving each time (0x441270).
pub(super) fn fractal_noise(x: f32) -> f32 {
    (0..6).map(|k| noise(x * (1 << k) as f32) * 0.5f32.powi(k)).sum()
}

pub(super) fn binary_angle(a: u16) -> f32 {
    a as f32 / 65536.0 * std::f32::consts::TAU
}

pub(super) const TUNING: [[Tuning; 2]; 6] = [
    // kind 0: table 0x7EB650
    [
        Tuning {
            ease: [1.5, 0.5, 1.0, 1.0],
            lean: Lag {
                scale: Vec3::new(0.006, 0.006, 0.0),
                min: Vec3::new(-1.0, -1.0, 1.0),
                max: Vec3::new(1.0, 1.0, 1.0),
            },
            eye: Lag {
                scale: Vec3::new(-0.25, 0.0, 0.0),
                min: Vec3::new(-2.5, -1.0, -1.0),
                max: Vec3::new(2.0, 1.0, 1.0),
            },
            look: Lag {
                scale: Vec3::new(-0.1, 0.0, 0.0),
                min: Vec3::new(-2.0, 0.0, -1.0),
                max: Vec3::new(2.0, 0.0, 1.0),
            },
            heading_ease: Vec3::new(0.5, 0.5, 0.7),
        },
        Tuning {
            ease: [0.5, 0.2, 0.5, 0.5],
            lean: Lag {
                scale: Vec3::new(0.002, 0.002, 0.0),
                min: Vec3::new(-1.0, -1.0, 1.0),
                max: Vec3::new(1.0, 1.0, 1.0),
            },
            eye: Lag {
                scale: Vec3::new(-0.05, 0.0, 0.0),
                min: Vec3::new(-0.5, -1.0, -1.0),
                max: Vec3::new(0.5, 1.0, 1.0),
            },
            look: Lag {
                scale: Vec3::new(-0.02, 0.0, 0.0),
                min: Vec3::new(-0.5, 0.0, -1.0),
                max: Vec3::new(0.5, 0.0, 1.0),
            },
            heading_ease: Vec3::new(0.1, 0.1, 0.4),
        },
    ],
    // kind 1: table 0x7EB910
    [
        Tuning {
            ease: [1.5, 0.5, 1.0, 0.9],
            lean: Lag {
                scale: Vec3::new(0.005, 0.005, 0.0),
                min: Vec3::new(-1.0, -1.0, 1.0),
                max: Vec3::new(1.0, 1.0, 1.0),
            },
            eye: Lag {
                scale: Vec3::new(-0.2, 0.0, 0.0),
                min: Vec3::new(-2.0, -1.0, -1.0),
                max: Vec3::new(1.5, 1.0, 1.0),
            },
            look: Lag {
                scale: Vec3::new(-0.1, 0.0, 0.0),
                min: Vec3::new(-2.0, 0.0, -1.0),
                max: Vec3::new(2.0, 0.0, 1.0),
            },
            heading_ease: Vec3::new(0.5, 0.5, 0.7),
        },
        Tuning {
            ease: [0.5, 0.2, 0.5, 0.1],
            lean: Lag {
                scale: Vec3::new(0.001, 0.001, 0.0),
                min: Vec3::new(-1.0, -1.0, 1.0),
                max: Vec3::new(1.0, 1.0, 1.0),
            },
            eye: Lag {
                scale: Vec3::new(-0.05, 0.0, 0.0),
                min: Vec3::new(-0.5, -1.0, -1.0),
                max: Vec3::new(0.5, 1.0, 1.0),
            },
            look: Lag {
                scale: Vec3::new(-0.02, 0.0, 0.0),
                min: Vec3::new(-0.5, 0.0, -1.0),
                max: Vec3::new(0.5, 0.0, 1.0),
            },
            heading_ease: Vec3::new(0.1, 0.1, 0.3),
        },
    ],
    // kind 2: table 0x7EBA70
    [
        Tuning {
            ease: [0.0, 0.0, 1.0, 0.0],
            lean: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            eye: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            look: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            heading_ease: Vec3::new(0.0, 0.0, 0.0),
        },
        Tuning {
            ease: [0.0, 0.0, 0.5, 0.0],
            lean: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            eye: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            look: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            heading_ease: Vec3::new(0.0, 0.0, 0.0),
        },
    ],
    // kind 3: table 0x7EBD30
    [
        Tuning {
            ease: [0.0, 0.0, 1.0, 0.0],
            lean: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            eye: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            look: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            heading_ease: Vec3::new(0.0, 0.0, 0.0),
        },
        Tuning {
            ease: [0.0, 0.0, 0.5, 0.0],
            lean: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            eye: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            look: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            heading_ease: Vec3::new(0.0, 0.0, 0.0),
        },
    ],
    // kind 4: table 0x7EBBD0
    [
        Tuning {
            ease: [0.7, 0.7, 1.0, 0.7],
            lean: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            eye: Lag {
                scale: Vec3::new(-0.01, 0.0, 0.0),
                min: Vec3::new(-0.1, -0.1, -0.1),
                max: Vec3::new(0.2, 0.1, 0.1),
            },
            look: Lag {
                scale: Vec3::new(0.0, 0.005, 0.0),
                min: Vec3::new(0.0, -0.08, 0.0),
                max: Vec3::new(0.0, 0.08, 0.0),
            },
            heading_ease: Vec3::new(0.5, 0.5, 0.5),
        },
        Tuning {
            ease: [0.5, 0.5, 0.5, 0.5],
            lean: Lag { scale: Vec3::new(0.0, 0.0, 0.0), min: Vec3::new(0.0, 0.0, 0.0), max: Vec3::new(0.0, 0.0, 0.0) },
            eye: Lag {
                scale: Vec3::new(-0.01, 0.0, 0.0),
                min: Vec3::new(-0.1, -0.1, -0.1),
                max: Vec3::new(0.2, 0.1, 0.1),
            },
            look: Lag {
                scale: Vec3::new(0.0, 0.005, 0.0),
                min: Vec3::new(0.0, -0.08, 0.0),
                max: Vec3::new(0.0, 0.08, 0.0),
            },
            heading_ease: Vec3::new(0.1, 0.1, 0.1),
        },
    ],
    // kind 5: table 0x7EB7B0
    [
        Tuning {
            ease: [1.5, 0.5, 1.0, 1.0],
            lean: Lag {
                scale: Vec3::new(0.006, 0.006, 0.0),
                min: Vec3::new(-1.0, -1.0, 1.0),
                max: Vec3::new(1.0, 1.0, 1.0),
            },
            eye: Lag {
                scale: Vec3::new(-0.25, 0.0, 0.0),
                min: Vec3::new(-2.5, -1.0, -1.0),
                max: Vec3::new(2.0, 1.0, 1.0),
            },
            look: Lag {
                scale: Vec3::new(-0.1, 0.0, 0.0),
                min: Vec3::new(-2.0, 0.0, -1.0),
                max: Vec3::new(2.0, 0.0, 1.0),
            },
            heading_ease: Vec3::new(0.5, 0.5, 0.7),
        },
        Tuning {
            ease: [0.5, 0.2, 0.5, 0.5],
            lean: Lag {
                scale: Vec3::new(0.002, 0.002, 0.0),
                min: Vec3::new(-1.0, -1.0, 1.0),
                max: Vec3::new(1.0, 1.0, 1.0),
            },
            eye: Lag {
                scale: Vec3::new(-0.05, 0.0, 0.0),
                min: Vec3::new(-0.5, -1.0, -1.0),
                max: Vec3::new(0.5, 1.0, 1.0),
            },
            look: Lag {
                scale: Vec3::new(-0.02, 0.0, 0.0),
                min: Vec3::new(-0.5, 0.0, -1.0),
                max: Vec3::new(0.5, 0.0, 1.0),
            },
            heading_ease: Vec3::new(0.1, 0.1, 0.4),
        },
    ],
];
