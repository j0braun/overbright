use super::*;

/// The Graphics row's tiers (`qol::particles`): 0 Original (the native
/// path, nothing here runs), 1 High, 2 Ultra.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TierParams {
    /// Puffs per native smoke particle.
    pub children: u32,
    /// A puff's size against the native particle's.
    pub size: f32,
    /// A puff's life against the native particle's (lingering).
    pub linger: f32,
    /// A puff's opacity against the native particle's.
    pub alpha: f32,
    /// The GPU pool (puffs alive at once; the oldest are reused).
    pub pool: u32,
    /// Street lamps that light the smoke.
    pub lamps: usize,
    /// Procedural detail (noise erosion) strength, 0..1.
    pub detail: f32,
}

pub const HIGH: TierParams =
    TierParams { children: 4, size: 0.62, linger: 1.35, alpha: 0.85, pool: 16384, lamps: 8, detail: 0.7 };
pub const ULTRA: TierParams =
    TierParams { children: 8, size: 0.48, linger: 1.7, alpha: 0.72, pool: 49152, lamps: 16, detail: 1.0 };

pub fn tier_params(tier: u8) -> Option<TierParams> {
    match tier {
        0 => None,
        1 => Some(HIGH),
        _ => Some(ULTRA),
    }
}

/// Whether a description is smoke the detailed path takes over: names with
/// "Smk" / "Smoke" in GLOBALB's tyre (XETT_), contact (XEBO_, XECI_, XECS_)
/// and purge (XES_NOSBO_) groups. The event markers' and front end's smoke
/// (XEE_, XEFE_: icons and haze that mark a spot) and the flag girl's flare
/// (XECH_) stay native.
pub fn is_smoke(d: &Description) -> bool {
    let n = d.name.to_ascii_lowercase();
    (n.contains("smk") || n.contains("smoke"))
        && ["xett_", "xebo_", "xeci_", "xecs_", "xes_nosbo"].iter().any(|p| n.starts_with(p))
}

/// The view depth of a depth-buffer value (the scene's projection,
/// `Camera::projection_depth`: near at 0 and far at 1, or reversed).
/// Mirrors gpu_particles.wgsl `linear_depth`.
pub fn linear_depth(d: f32, near: f32, far: f32, reversed: bool) -> f32 {
    if reversed { near * far / (near + d * (far - near)) } else { near * far / (far - d * (far - near)) }
}

/// The soft-particle fade: 0 at or behind the scene's surface, rising to 1
/// over `soft` metres in front of it. Mirrors gpu_particles.wgsl.
pub fn soft_fade(scene_z: f32, particle_z: f32, soft: f32) -> f32 {
    ((scene_z - particle_z) / soft.max(1e-3)).clamp(0.0, 1.0)
}

/// The soft distance for a puff of `size` metres: a quarter of it, 0.15
/// to 0.6 m (a wider fade thinned the big low tyre puffs on the road to
/// nothing where the native sprite shows its upper half in full).
pub fn softness(size: f32) -> f32 {
    (0.25 * size).clamp(0.15, 0.6)
}

/// One native smoke particle at its birth, as the compute spawn reads it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Birth {
    pub position: [f32; 3],
    /// Its life (s).
    pub life: f32,
    pub velocity: [f32; 3],
    /// The share of its owner's velocity it is carried by (0..1).
    pub share: f32,
    /// Description index, colour entry ([`Detail`]'s colour table:
    /// descriptions, then the colour slots), seed, owner car (u32::MAX none).
    pub ids: [u32; 4],
}

/// One 1/60 s world update: its births and its cars (position, velocity),
/// by the owner index the carried particles use.
#[derive(Clone, Debug, Default)]
pub struct Tick {
    pub dt: f32,
    pub births: Vec<Birth>,
    pub cars: Vec<Option<(Vec3, Vec3)>>,
    /// The world time after the tick (s; the replay's clock, the
    /// same whichever frame replays it) and Cinematic's rain smoothed per
    /// tick (`Feeder::wet`; 0 for High / Ultra).
    pub time: f64,
    pub wet: f32,
}

/// Ticks kept for the next draw at most (10 s of world time: a headless
/// shot steps many updates before it draws; older puffs would be dead).
pub const MAX_TICKS: usize = 600;
/// Births sent in one draw at most.
pub const MAX_BIRTHS: usize = 16384;
/// Cars the update sees (wake, carried).
pub const MAX_CARS: usize = 8;

/// The CPU half: classifies the native births and queues them per tick.
#[derive(Default)]
pub struct Feeder {
    smoke: Vec<bool>,
    pub ticks: VecDeque<Tick>,
    seed: u32,
    /// World time (s) and when the last puff dies.
    pub time: f64,
    pub alive_until: f64,
    /// Cinematic's rain on new smoke, smoothed per tick toward the
    /// rain's amount (None until the path's first tick: then the rain as
    /// it is; before, it was smoothed per drawn frame).
    pub wet: Option<f32>,
}

/// The owner keys of the clone's carried car particles (game.rs FLAME_KEY).
pub const CAR_KEY: u32 = 0x4E4F_0000;

impl Feeder {
    /// Whether description `i` is drawn by the detailed path.
    pub fn smoke(&self, i: usize) -> bool {
        self.smoke.get(i).copied().unwrap_or(false)
    }

    pub(super) fn classify(&mut self, library: &Library) {
        if self.smoke.len() != library.descriptions.len() {
            self.smoke = library.descriptions.iter().map(is_smoke).collect();
        }
    }

    /// After one native world update (`Effects::step`): take its births,
    /// keep the smoke ones (with `colors` the number of descriptions, the
    /// colour table's first colour-slot entry) and queue the tick.
    pub fn record(
        &mut self,
        library: &Library,
        births: Vec<Particle>,
        cars: Vec<Option<(Vec3, Vec3)>>,
        dt: f32,
        tier: TierParams,
        rain: f32,
    ) {
        self.classify(library);
        self.time += dt as f64;
        // Smooth the rain's effect on new smoke (from the current rain when
        // the path starts, not from dry).
        let rain = rain.clamp(0.0, 1.0);
        let wet = self.wet.map_or(rain, |w| w + (rain - w) * 0.1);
        self.wet = Some(wet);
        let n = library.descriptions.len() as u32;
        let mut out = Vec::new();
        for p in births.iter() {
            if !self.smoke(p.description as usize) {
                continue;
            }
            self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let life = p.life as f32 / 1024.0;
            // Carried (+0x1E) particles follow their owner car (0x492E10).
            let carried =
                library.descriptions.get(p.description as usize).is_some_and(|d| d.carried && d.inherit[0] > 0.0);
            let car = p.key.checked_sub(CAR_KEY).filter(|&c| (c as usize) < MAX_CARS && carried).unwrap_or(u32::MAX);
            let color = if p.palette != 0 { n + p.palette as u32 - 1 } else { p.description as u32 };
            out.push(Birth {
                position: p.position.to_array(),
                life,
                velocity: p.velocity().to_array(),
                share: p.carried as f32 / 255.0,
                ids: [p.description as u32, color, self.seed, car],
            });
            self.alive_until = self.alive_until.max(self.time + (life * tier.linger * 1.25) as f64);
        }
        self.ticks.push_back(Tick { dt, births: out, cars, time: self.time, wet });
        while self.ticks.len() > MAX_TICKS {
            self.ticks.pop_front();
        }
    }

    /// Whether any puff may still be alive.
    pub fn live(&self) -> bool {
        self.time <= self.alive_until
    }
}

/// Puffs written per native birth for the spawn's ring (`head` advanced
/// by count × children, wrapping at the pool).
pub fn ring_slots(head: u32, births: u32, children: u32, pool: u32) -> impl Iterator<Item = u32> {
    (0..births * children).map(move |i| (head + i) % pool)
}
