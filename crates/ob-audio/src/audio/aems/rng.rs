/// The library's random numbers (0x724903, a sum-with-carry generator
/// over 0x8B46F8..0x8B470C; **not ported**: a 32-bit xorshift stands in,
/// only the draws' order matters to the graphs).
#[derive(Clone, Debug)]
pub struct Rng(u32);

impl Default for Rng {
    fn default() -> Self {
        Rng(0x1234_5678)
    }
}

impl Rng {
    pub fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}
