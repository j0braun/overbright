/// The game's xorshift (0x7F3940 and each emitter's +0x24): x ^= x << 17,
/// x ^= x >> 13, x ^= x << 5; a draw is (x & 0x7FFFFFFF) / 2³¹.
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u32);

impl Rng {
    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 17;
        x ^= x >> 13;
        x ^= x << 5;
        self.0 = x;
        (x & 0x7FFF_FFFF) as f32 * 4.656613e-10
    }
    /// A value spread around `base`: `rand · base·spread + base − base·spread`.
    pub fn spread(&mut self, [base, spread]: [f32; 2]) -> f32 {
        let s = base * spread;
        self.next() * s + (base - s)
    }
}
