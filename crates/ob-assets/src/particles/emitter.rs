use super::*;

/// An emitter instance (0xD0 bytes): its description, matrix (+0x40), the
/// owner's velocity (+0x80), on/off state (+0x10, +0x14), the time to its
/// next particle (+0x18) and its random state (+0x24).
#[derive(Clone, Debug)]
pub struct Emitter {
    pub description: usize,
    pub transform: Mat4,
    pub velocity: Vec3,
    /// +0x10: 1 emitting, 0 waiting; +0x14: the time left in that state (0
    /// for ever).
    pub on: bool,
    pub timer: f32,
    pub wait: f32,
    pub rng: Rng,
    /// The group key (0x491E50's third argument; the owner whose velocity
    /// carried particles follow).
    pub key: u32,
    /// +0x30: a colour slot (0x8A3F60, set by 0x48B920) the draw takes
    /// instead of the description's +0x100 (0x48BDF0); 0 none, else
    /// [`System::palettes`] index + 1.
    pub palette: u16,
}

impl Emitter {
    pub fn new(description: usize, transform: Mat4, seed: u32) -> Emitter {
        Emitter {
            description,
            transform,
            velocity: Vec3::ZERO,
            on: true,
            timer: 0.0,
            wait: 0.0,
            rng: Rng(seed | 1),
            key: 0,
            palette: 0,
        }
    }

    /// 0x48BC50 for an emitter in state 1 (`on`) or 0: the time it stays
    /// so, +0x24 ± +0x28 emitting or +0x2C ± +0x30 waiting (0 for ever),
    /// drawn from its own random state, and no wait.
    pub fn start(&mut self, library: &Library) {
        let Some(d) = library.descriptions.get(self.description) else { return };
        let span = if self.on { d.on_time } else { d.off_time };
        self.timer = if span[0] == 0.0 { 0.0 } else { self.rng.spread(span) };
        self.wait = 0.0;
    }
}
