//! Tables of SPEED2.EXE that are pure maths, computed from their formula
//! instead of read (a data test checks every value against the player's
//! executable; the formula's margin to a rounding step is over 0.0015 for
//! every entry, far above any libm difference).

use std::sync::LazyLock;

/// The 16-bit arctangent's table (0x7EACB0, 256 bytes, read by 0x43C570):
/// entry i = trunc(atan((i + ½) / 256) / (π/4) · 256), the angle of the
/// middle of the ratio's step in 1/256 of 45°.
pub fn atan_table() -> &'static [u8; 256] {
    static T: LazyLock<[u8; 256]> = LazyLock::new(|| {
        std::array::from_fn(|i| ((((i as f64 + 0.5) / 256.0).atan() / std::f64::consts::FRAC_PI_4) * 256.0) as u8)
    });
    &T
}

/// The quarter sine of the event VM (0x8181D8, 257 u16, read by
/// 0x7254CF): entry i = min(65535, trunc(65536 · sin(i · π / 512))),
/// exactly (verified: the 257 stored u16 equal the formula with
/// no exception, in the file and read from the running game; no code writes
/// the table). The six entries 11, 13, 15, 19, 20, 21 = 4420, 5222, 6023,
/// 7623, 8022, 8421 were once one less in the clone.
pub fn quarter_sine() -> &'static [u16; 257] {
    static T: LazyLock<[u16; 257]> = LazyLock::new(|| {
        std::array::from_fn(|i| (65536.0 * (i as f64 * std::f64::consts::PI / 512.0).sin()).min(65535.0) as u16)
    });
    &T
}

/// Where the generated tables sit in the executable, for the data test.
pub const ATAN_TABLE_VA: u32 = 0x7E_ACB0;
pub const QUARTER_SINE_VA: u32 = 0x81_81D8;

/// The mixer's dB → linear table (0x7EDE88, 602 i32, read by the mix map):
/// entry i = floor(32768 · 10^((i − 602) / 2000)), the gain of
/// (i − 602) hundredths of a decibel in 1/32768, except entry 0, which is
/// stored as exactly half (16384; the formula gives 16385).
pub fn db_to_lin() -> &'static [i32; 602] {
    static T: LazyLock<[i32; 602]> = LazyLock::new(|| {
        std::array::from_fn(|i| {
            if i == 0 { 16384 } else { (32768.0 * 10f64.powf((i as f64 - 602.0) / 2000.0)).floor() as i32 }
        })
    });
    &T
}

/// The inverse table (0x7EE7F0, 512 i32): entry j = the number of
/// [`db_to_lin`] entries below (512 + j) · 32, less one, at least 0: the
/// dB step of the linear level (512 + j) / 1024.
pub fn lin_to_db() -> &'static [i32; 512] {
    static T: LazyLock<[i32; 512]> = LazyLock::new(|| {
        let db = db_to_lin();
        std::array::from_fn(|j| {
            let below = db.iter().filter(|&&v| v < (512 + j as i32) * 32).count() as i32;
            (below - 1).max(0)
        })
    });
    &T
}

/// The arcsine table (0x7EA628, 208 entries of {u16 angle, f32 slope},
/// read by the AI driver and the mix map). The grid of x in 0..1 is 13
/// runs of 16 steps, each run covering half of what is left to 1 and the
/// last run the rest: run k starts at 1 − 2^−k with steps of 2^−(k+5)
/// (the last, k = 12, steps of 2^−16 up to 1). Each entry holds
/// trunc(asin(x) · 32768 / π), the angle in 1/65536 of a turn, and the
/// slope to the next grid point, (asin(x′) − asin(x)) / (x′ − x) / 2π,
/// rounded to four decimals. Twenty slopes are stored one step (0.0001)
/// lower than that rounding gives ([`ASIN_SLOPES_ROUNDED_DOWN`]); how the
/// original rounded them is not known.
pub fn asin_table() -> &'static [(u16, f32); 208] {
    static T: LazyLock<[(u16, f32); 208]> = LazyLock::new(|| {
        let x = |i: usize| -> f64 {
            if i >= 208 {
                return 1.0;
            }
            let (k, j) = (i / 16, i % 16);
            let step = if k < 12 { (-((k + 5) as f64)).exp2() } else { (-16f64).exp2() };
            1.0 - (-(k as f64)).exp2() + j as f64 * step
        };
        std::array::from_fn(|i| {
            let angle = (x(i).asin() * 32768.0 / std::f64::consts::PI) as u16;
            let slope = (x(i + 1).asin() - x(i).asin()) / (x(i + 1) - x(i)) / std::f64::consts::TAU;
            let mut steps = (slope * 1e4).round();
            if ASIN_SLOPES_ROUNDED_DOWN.contains(&i) {
                steps -= 1.0;
            }
            (angle, (steps / 1e4) as f32)
        })
    });
    &T
}

/// The [`asin_table`] entries whose stored slope is one 0.0001 step below
/// the rounded secant slope.
pub const ASIN_SLOPES_ROUNDED_DOWN: [usize; 20] =
    [6, 11, 14, 30, 32, 57, 61, 72, 78, 102, 107, 113, 115, 126, 129, 143, 165, 173, 193, 195];
