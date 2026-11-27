use super::*;

/// What the tier does with a native description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Spark,
    Flame,
    /// The contact's hot glow (`XECS_SolidWall_GlowA`, HEADLIGHTGLOW) and
    /// the bottom-out flash (`XEBO_ROAD_FlashA`): native camera-facing
    /// sprites cut by the wall they sit on; drawn as a soft hot spot.
    Glow,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Spark => "spark",
            Kind::Flame => "flame",
            Kind::Glow => "glow",
        }
    }
}

/// The descriptions the tier restyles: the contact groups' and the flag
/// girl's spark descriptions (`XECS_SolidWall_SprksA`, `XEBO_ROAD_SprksA`,
/// `XECI_Car_SprksA`, `XECH_Flag_Girl_Flare_SprksA`) and the nitrous flame
/// (`XES_NOS_FlameA`, the only `CARFX_NITRO` description).
pub fn kind_of(d: &Description) -> Option<Kind> {
    let n = d.name.to_ascii_lowercase();
    if n.contains("sprks") {
        Some(Kind::Spark)
    } else if n.contains("nos_flame") {
        Some(Kind::Flame)
    } else if n.contains("solidwall_glow") || n.contains("road_flash") {
        Some(Kind::Glow)
    } else {
        None
    }
}

/// Fragments per native spark birth at full contact intensity, and at
/// none (the native births already scale with the burst's share;
/// a light touch makes about one fragment in four births, a hard scrape
/// two a birth).
pub const FRAGMENTS_MAX: f32 = 2.0;
pub const FRAGMENTS_MIN: f32 = 0.25;
/// The intensity of births that come from no contact (the flag girl's
/// flare).
pub const FREE_INTENSITY: f32 = 0.5;
/// The most fragments one world update adds (a pile-up's many pushes do
/// not make a shower).
pub const MAX_NEW: usize = 40;
/// The most fragments alive (the oldest go first).
pub const MAX_SPARKS: usize = 2048;
/// The most flame particles drawn (the native pool is 1000).
pub const MAX_FLAMES: usize = 1024;
/// The most contact glows drawn.
pub const MAX_GLOWS: usize = 256;
/// Lights a frame, and of them the sparks' at most.
pub const MAX_LIGHTS: usize = 16;
pub const MAX_SPARK_LIGHTS: usize = 4;
const GRAVITY: f32 = 9.81;
/// Air drag on a fragment, 1/m (v̇ = −k |v| v): a hot chip of a few
/// tenths of a millimetre slows fast.
const DRAG: f32 = 0.06;
/// A fragment's life: the description's, within these bounds (s), times
/// 0.5..1.2 (XEBO_ROAD_SprksA's 2 s would hang in the air).
const LIFE_MIN: f32 = 0.2;
pub(super) const LIFE_MAX: f32 = 0.6;
/// Bounce: the normal speed kept, the tangential speed kept.
const RESTITUTION: f32 = 0.38;
const FRICTION: f32 = 0.72;

/// A small deterministic random source (xorshift32).
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u32);

impl Rng {
    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / 16_777_216.0
    }
    fn unit(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(self.next() * 2.0 - 1.0, self.next() * 2.0 - 1.0, self.next() * 2.0 - 1.0);
            let l = v.length_squared();
            if l > 1e-4 && l <= 1.0 {
                return v / l.sqrt();
            }
        }
    }
}

/// One contact burst of a world update (`particles::Burst`): the births
/// it made (indices into the update's births) and the push behind it.
#[derive(Clone, Debug)]
pub struct Contact {
    pub births: std::ops::Range<usize>,
    /// Sliding speed (m/s) and |J| of the push.
    pub slide: f32,
    pub impulse: f32,
    /// Off the surface (a wall's: toward the car).
    pub normal: Vec3,
    /// Two cars: the normal's sign says nothing.
    pub two_sided: bool,
}

/// |J| of a push that counts as a hard contact (a 100 km/h glancing wall
/// scrape, a car-to-car shove; the wall group's own impulse band is
/// 10..15, `particles::post_contact`).
pub const IMPULSE_FULL: f32 = 12.0;

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A contact's spark intensity 0..1: the sliding speed makes sparks (no
/// slide, no grinding), the push's force how many; a hard hit throws some
/// even with little slide. A light touch stays near 0.
pub fn intensity(slide: f32, impulse: f32) -> f32 {
    let speed = smoothstep(3.0, 30.0, slide);
    let force = (impulse / IMPULSE_FULL).clamp(0.0, 1.0);
    (speed * (0.4 + 0.6 * force))
        .max(0.6 * smoothstep(0.5, 1.0, force) * smoothstep(1.0, 8.0, slide.max(impulse * 0.5)))
}

/// A hard hit's extra burst: × 1 up to × 3 between |J| 15 and 40 (one
/// push; a scrape's pushes stay near × 1).
pub fn hit_burst(impulse: f32) -> f32 {
    1.0 + 2.0 * smoothstep(15.0, 40.0, impulse)
}

/// One hot fragment.
#[derive(Clone, Copy, Debug)]
pub struct Spark {
    /// Its number (the draw pairs a fragment of S_{n−1} with
    /// S_n's by it).
    pub id: u32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub age: f32,
    pub life: f32,
    /// The road's height under its contact (None: no road found).
    pub ground: Option<f32>,
    /// Its starting heat (0..1) and a per-fragment seed.
    pub heat: f32,
    pub seed: f32,
}

impl Spark {
    /// 0..1: 1 white hot, 0 gone dark.
    pub fn temperature(&self) -> f32 {
        let x = (self.age / self.life).clamp(0.0, 1.0);
        self.heat * (1.0 - x * x)
    }
}

/// The CPU half: the fragments, stepped with the world (1/60 s updates).
#[derive(Default)]
pub struct Sim {
    pub sparks: Vec<Spark>,
    /// The fragments as the tick before left them (S_{n−1}; the
    /// draw blends to `sparks`, S_n: [`Sim::drawn`]).
    pub prev: Vec<Spark>,
    next_id: u32,
    rng: Option<Rng>,
    /// The kind per description index (made once per library).
    kinds: Vec<Option<Kind>>,
    pub time: f32,
}

impl Sim {
    /// The kind of description `i` (cached).
    pub fn kind(&mut self, library: &Library, i: usize) -> Option<Kind> {
        if self.kinds.len() != library.descriptions.len() {
            self.kinds = library.descriptions.iter().map(kind_of).collect();
        }
        self.kinds.get(i).copied().flatten()
    }

    /// The kind of description `i` once [`Sim::kind`] has run (draw side).
    pub fn kind_cached(&self, i: usize) -> Option<Kind> {
        self.kinds.get(i).copied().flatten()
    }

    /// Take one world update's births: each spark birth throws a few
    /// fragments (by its contact's intensity, [`intensity`]) with its
    /// velocity, a small scatter off the contact's surface and a little
    /// lift, each with the road under it from `ground`. `contacts`: the
    /// update's bursts and the births each made (none: every birth at
    /// [`FREE_INTENSITY`]).
    pub fn record(
        &mut self,
        library: &Library,
        births: &[Particle],
        contacts: &[Contact],
        ground: &dyn Fn(Vec3) -> Option<f32>,
    ) {
        // This tick starts: what the last one left is S_{n−1}.
        self.prev.clone_from(&self.sparks);
        let mut rng = self.rng.unwrap_or(Rng(0x9E37_79B9));
        let mut added = 0;
        for (i, p) in births.iter().enumerate() {
            if self.kind(library, p.description as usize) != Some(Kind::Spark) {
                continue;
            }
            let Some(d) = library.descriptions.get(p.description as usize) else { continue };
            let contact = contacts.iter().find(|c| c.births.contains(&i));
            let e = contact.map_or(FREE_INTENSITY, |c| intensity(c.slide, c.impulse));
            // Stochastic rounding: the mean is exact over a scrape. A hard
            // hit's one push throws up to three times as many at once.
            let n =
                (FRAGMENTS_MIN + (FRAGMENTS_MAX - FRAGMENTS_MIN) * e) * contact.map_or(1.0, |c| hit_burst(c.impulse));
            let n = n.floor() as usize + usize::from(rng.next() < n.fract());
            // Off the surface: a wall's normal (toward the car); two cars'
            // either way; none known: up.
            let normal = contact.map_or(Vec3::Z, |c| c.normal.normalize_or(Vec3::Z));
            let two_sided = contact.is_some_and(|c| c.two_sided);
            let v0 = p.velocity();
            let g = ground(p.position);
            for _ in 0..n {
                if added >= MAX_NEW {
                    break;
                }
                // Thrown with the contact's motion (the native birth's
                // velocity: most of the car's), torn off along the surface,
                // a little off it and up; harder contacts throw wider.
                let r = rng.unit();
                // Along the surface, flatter than up (half the vertical).
                let along = r - normal * r.dot(normal);
                let along = Vec3::new(along.x, along.y, along.z * 0.5);
                let off = if two_sided && rng.next() < 0.5 { -normal } else { normal };
                let scatter = along * ((0.6 + 1.8 * rng.next()) * (0.5 + 0.5 * e))
                    + off * (0.3 + 1.2 * rng.next())
                    + Vec3::Z * (0.4 + 1.4 * rng.next());
                let v = v0 * (0.45 + 0.5 * rng.next()) + scatter;
                let life = d.life[0].clamp(LIFE_MIN, LIFE_MAX) * (0.5 + 0.7 * rng.next());
                self.next_id = self.next_id.wrapping_add(1);
                self.sparks.push(Spark {
                    id: self.next_id,
                    position: p.position,
                    velocity: v,
                    age: 0.0,
                    life,
                    ground: g,
                    heat: 0.85 + 0.15 * rng.next(),
                    seed: rng.next(),
                });
                added += 1;
            }
        }
        if self.sparks.len() > MAX_SPARKS {
            let extra = self.sparks.len() - MAX_SPARKS;
            self.sparks.drain(..extra);
        }
        self.rng = Some(rng);
    }

    /// Move every fragment by `dt`: gravity, drag, the road's bounce.
    pub fn step(&mut self, dt: f32) {
        self.time += dt;
        if dt <= 0.0 {
            return;
        }
        self.sparks.retain_mut(|s| {
            s.age += dt;
            if s.age >= s.life {
                return false;
            }
            let v = s.velocity;
            let v = v - v * (v.length() * DRAG * dt) - Vec3::Z * (GRAVITY * dt);
            s.velocity = v;
            s.position += v * dt;
            if let Some(g) = s.ground
                && s.position.z < g
            {
                s.position.z = g + (g - s.position.z) * RESTITUTION;
                if s.velocity.z < 0.0 {
                    s.velocity.z = -s.velocity.z * RESTITUTION;
                    // Friction, and a rough road turns the skid a little
                    // (by the fragment's own seed: deterministic).
                    let turn = (s.seed - 0.5) * 0.5;
                    let (sn, cs) = turn.sin_cos();
                    let (x, y) = (s.velocity.x * FRICTION, s.velocity.y * FRICTION);
                    s.velocity.x = x * cs - y * sn;
                    s.velocity.y = x * sn + y * cs;
                    s.seed = (s.seed * 7.31).fract();
                    // A hit sheds heat.
                    s.heat *= 0.8;
                }
            }
            true
        });
    }

    pub fn clear(&mut self) {
        self.sparks.clear();
        self.prev.clear();
    }

    /// The "blend" class: the fragments as a frame `alpha` of the way
    /// from S_{n−1} to S_n draws them: S_n itself at α ≥ 1; else every
    /// fragment of S_{n−1}, moved (position, velocity, age, heat) toward
    /// its S_n self where it lives on, as it was where it died in the
    /// tick; one born in the tick is not drawn before the frame reaches
    /// it. Both lists keep their order of birth (ids rising).
    pub fn drawn<'a>(&'a self, alpha: f32, scratch: &'a mut Vec<Spark>) -> &'a [Spark] {
        if alpha >= 1.0 {
            return &self.sparks;
        }
        scratch.clear();
        let mut cur = self.sparks.iter().peekable();
        for a in &self.prev {
            while cur.peek().is_some_and(|b| b.id < a.id) {
                cur.next();
            }
            let mut s = *a;
            if let Some(b) = cur.peek().filter(|b| b.id == a.id) {
                s.position = a.position.lerp(b.position, alpha);
                s.velocity = a.velocity.lerp(b.velocity, alpha);
                s.age = a.age + (b.age - a.age) * alpha;
                s.heat = a.heat + (b.heat - a.heat) * alpha;
            }
            scratch.push(s);
        }
        scratch
    }

    /// A digest of the fragments (the rate tests compare it at equal ticks).
    pub fn digest(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for s in &self.sparks {
            for v in [s.position.x, s.position.y, s.position.z, s.velocity.x, s.velocity.y, s.velocity.z, s.age] {
                h = (h ^ u64::from(v.to_bits())).wrapping_mul(0x100_0000_01b3);
            }
            h = (h ^ u64::from(s.id)).wrapping_mul(0x100_0000_01b3);
        }
        h
    }
}
