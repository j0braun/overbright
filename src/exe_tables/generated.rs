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
