/// Lens Drops (clone deviation): drops landing on the lens while it rains
/// (not under a cover zone), sliding down slowly when standing, blown out
/// toward the screen's edges and off by the speed. Drawn by the native
/// lens pass (0x5C7AC0's shader, `rain::LensPass`) after its own 20.
/// **Inferred**, by eye.
#[derive(Default)]
pub struct LensSim {
    drops: Vec<LensSimDrop>,
    rng: u32,
    last_clock: Option<f32>,
    carry: f32,
    /// Rain Detail Next-Gen: drops of many sizes; the big ones run
    /// down (their weight beats the lens's grip), leave a trail of tiny
    /// drops and swallow the drops they meet; the small ones stick.
    pub next_gen: bool,
    trail: f32,
}

#[derive(Clone, Copy, Debug)]
struct LensSimDrop {
    x: f32,
    y: f32,
    size: f32,
    age: f32,
    life: f32,
    slide: f32,
}

/// The extra lens drops at most.
pub const LENS_MAX: usize = 60;
/// Next-Gen: the size below which a drop sticks to the lens.
pub const LENS_GRIP: f32 = 0.2;

impl LensSim {
    fn rand(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }

    /// One step of `dt` s: `rain` the amount (0 under a cover), `speed` m/s.
    pub fn step(&mut self, rain: f32, speed: f32, dt: f32) {
        if self.next_gen {
            return self.step_next_gen(rain, speed, dt);
        }
        let wipe = ((speed - 8.0) / 22.0).clamp(0.0, 1.0);
        self.carry += 9.0 * rain * (1.0 - 0.6 * wipe) * dt;
        while self.carry >= 1.0 {
            self.carry -= 1.0;
            if self.drops.len() < LENS_MAX {
                let (x, y, s, l, d) = (self.rand(), self.rand(), self.rand(), self.rand(), self.rand());
                self.drops.push(LensSimDrop {
                    x,
                    y,
                    size: 0.12 + 0.3 * s * s,
                    age: 0.0,
                    life: 2.5 + 5.0 * l,
                    slide: 0.004 + 0.02 * d,
                });
            }
        }
        let push = 0.025 * speed.max(0.0);
        for d in &mut self.drops {
            d.age += dt;
            // From the screen's centre outward with the speed, down with
            // gravity when slow.
            let (cx, cy) = (d.x - 0.5, d.y - 0.42);
            let len = (cx * cx + cy * cy).sqrt().max(0.05);
            d.x += cx / len * push * wipe * dt;
            d.y += (cy / len * push * wipe + d.slide * (1.0 - wipe)) * dt;
        }
        self.drops.retain(|d| d.age < d.life && (-0.1..1.05).contains(&d.x) && (-0.1..1.05).contains(&d.y));
    }

    /// Next-Gen: a drop of size `s` (the native pass's size units, 0.12 to
    /// 0.42 as On's) runs down at `slide_speed(s)` screens per second when
    /// standing: none below the grip's size, then ∝ (s − grip)².
    /// **Inferred**, by eye.
    pub fn slide_speed(size: f32) -> f32 {
        let over = (size - LENS_GRIP).max(0.0);
        0.002 + 3.0 * over * over
    }

    fn step_next_gen(&mut self, rain: f32, speed: f32, dt: f32) {
        let wipe = ((speed - 8.0) / 22.0).clamp(0.0, 1.0);
        self.carry += 10.0 * rain * (1.0 - 0.6 * wipe) * dt;
        while self.carry >= 1.0 {
            self.carry -= 1.0;
            if self.drops.len() < LENS_MAX {
                let (x, y, s, l) = (self.rand(), self.rand(), self.rand(), self.rand());
                // Many small drops, few big ones.
                self.drops.push(LensSimDrop {
                    x,
                    y,
                    size: 0.08 + 0.3 * s * s * s,
                    age: 0.0,
                    life: 3.0 + 6.0 * l,
                    slide: 0.0,
                });
            }
        }
        let push = 0.025 * speed.max(0.0);
        let mut trail = Vec::new();
        self.trail += dt;
        let leave = self.trail >= 0.12;
        if leave {
            self.trail = 0.0;
        }
        for d in &mut self.drops {
            d.age += dt;
            let (cx, cy) = (d.x - 0.5, d.y - 0.42);
            let len = (cx * cx + cy * cy).sqrt().max(0.05);
            let run = Self::slide_speed(d.size);
            d.x += cx / len * push * wipe * dt;
            d.y += (cy / len * push * wipe + run * (1.0 - wipe)) * dt;
            d.slide = run;
            // A running drop leaves part of itself behind (just out of its
            // reach, above it) and shrinks.
            if leave && run > 0.02 && d.size > LENS_GRIP {
                trail.push(LensSimDrop {
                    x: d.x,
                    y: d.y - (d.size + 0.05) * 0.05 - 0.004,
                    size: 0.05,
                    age: 0.0,
                    life: 1.5,
                    slide: 0.0,
                });
                d.size -= 0.004;
            }
        }
        for t in trail {
            if self.drops.len() < LENS_MAX {
                self.drops.push(t);
            }
        }
        // Merging: a drop swallows a smaller one it touches (area kept).
        let n = self.drops.len();
        for i in 0..n {
            for j in 0..n {
                if i == j
                    || self.drops[i].size <= 0.0
                    || self.drops[j].size <= 0.0
                    || self.drops[j].size > self.drops[i].size
                {
                    continue;
                }
                let (a, b) = (self.drops[i], self.drops[j]);
                let r = (a.size + b.size) * 0.05;
                if (a.x - b.x).powi(2) + (a.y - b.y).powi(2) < r * r {
                    self.drops[i].size = (a.size * a.size + b.size * b.size).sqrt().min(0.5);
                    self.drops[i].age = self.drops[i].age.min(b.age);
                    self.drops[j].size = 0.0;
                }
            }
        }
        self.drops
            .retain(|d| d.size > 0.0 && d.age < d.life && (-0.1..1.05).contains(&d.x) && (-0.1..1.05).contains(&d.y));
    }

    /// Spray (`hit` 0..1) thrown at the lens: up to 40 small drops a
    /// second, landing anywhere, short-lived (the speed blows them off).
    pub fn splash(&mut self, hit: f32, dt: f32) {
        self.carry += 40.0 * hit * dt;
        while self.carry >= 1.0 {
            self.carry -= 1.0;
            if self.drops.len() < LENS_MAX {
                let (x, y, s, l, d) = (self.rand(), self.rand(), self.rand(), self.rand(), self.rand());
                self.drops.push(LensSimDrop {
                    x,
                    y: 0.15 + 0.8 * y,
                    size: 0.08 + 0.18 * s * s,
                    age: 0.0,
                    life: 0.8 + 1.6 * l,
                    slide: 0.004 + 0.02 * d,
                });
            }
        }
    }

    pub fn drops(&self) -> impl Iterator<Item = crate::weather::LensDrop> + '_ {
        self.drops.iter().map(|d| crate::weather::LensDrop {
            x: d.x,
            y: d.y,
            size: d.size,
            alpha: ((1.0 - d.age / d.life) * 3.0).min(1.0) * 0.85,
        })
    }

    pub fn len(&self) -> usize {
        self.drops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.drops.is_empty()
    }
}

static LENS: std::sync::Mutex<Option<LensSim>> = std::sync::Mutex::new(None);

/// How much of the spray the eye sat in last frame (0..1,
/// `wet_spray::hit`): the lens catches the spray of the car ahead.
static SPRAY_HIT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn set_spray_hit(hit: f32) {
    SPRAY_HIT.store(hit.clamp(0.0, 1.0).to_bits(), std::sync::atomic::Ordering::Relaxed);
}

pub fn spray_hit() -> f32 {
    f32::from_bits(SPRAY_HIT.load(std::sync::atomic::Ordering::Relaxed))
}

/// The tick: Lens Drops' sim stepped by the world clock's
/// advance since the last tick (paused: held; a jump restarts), with the
/// spray the eye sat in at this tick ([`spray_hit`], set by the tick's
/// spray step before). The row off drops it. `on` / `next_gen`: Lens
/// Drops and Rain Detail Next-Gen as the tick took them.
pub fn lens_tick(rain: f32, covered: bool, clock: f32, speed: f32, on: bool, next_gen: bool) {
    let mut lens = LENS.lock().unwrap();
    if !on {
        *lens = None;
        return;
    }
    let sim = lens.get_or_insert_with(LensSim::default);
    if sim.next_gen != next_gen {
        *sim = LensSim { next_gen, ..LensSim::default() };
    }
    let dt = match sim.last_clock {
        Some(c) if clock > c && clock - c < 0.5 => clock - c,
        Some(c) if clock == c => 0.0,
        _ => {
            // A jump (a load, a restart): the sim starts over, its
            // generator too (the same drops from a load whatever
            // ran before).
            *sim = LensSim { next_gen, ..LensSim::default() };
            0.0
        }
    };
    sim.last_clock = Some(clock);
    let steps = (dt * 60.0).round() as u32;
    for _ in 0..steps.min(30) {
        sim.step(if covered { 0.0 } else { rain }, speed, 1.0 / 60.0);
        // The spray thrown at the lens (also under a cover: it is
        // the road's water).
        let hit = spray_hit();
        if hit > 0.0 {
            sim.splash(hit, 1.0 / 60.0);
        }
    }
}

/// Lens Drops' extra drops as the last tick left them ("stepped" class:
/// the draw reads, never steps; none with the row off: `on`).
pub fn lens_frame(on: bool) -> Vec<crate::weather::LensDrop> {
    if !on {
        return Vec::new();
    }
    LENS.lock().unwrap().as_ref().map_or_else(Vec::new, |sim| sim.drops().collect())
}

/// A digest of the lens sim's drops (the rate tests compare it at equal
/// ticks).
pub fn lens_digest() -> u64 {
    let lens = LENS.lock().unwrap();
    let Some(sim) = lens.as_ref() else { return 0 };
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for d in &sim.drops {
        for v in [d.x, d.y, d.size, d.age, d.life, d.slide] {
            h = (h ^ u64::from(v.to_bits())).wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lens drops: only while it rains; at speed they are blown off the
    /// screen sooner than they would dry.
    #[test]
    fn lens_drops_wiped_by_speed() {
        let mut l = LensSim::default();
        for _ in 0..600 {
            l.step(0.0, 0.0, 1.0 / 60.0);
        }
        assert!(l.is_empty());
        for _ in 0..600 {
            l.step(1.0, 0.0, 1.0 / 60.0);
        }
        let standing = l.len();
        assert!(standing > 20 && standing <= LENS_MAX, "{standing}");
        let mut fast = LensSim::default();
        for _ in 0..600 {
            fast.step(1.0, 40.0, 1.0 / 60.0);
        }
        assert!(fast.len() < standing / 3, "{} vs {standing}", fast.len());
        // After the rain stops, they go.
        for _ in 0..600 {
            l.step(0.0, 0.0, 1.0 / 60.0);
        }
        assert!(l.is_empty());
    }

    /// Spray at the lens adds short-lived drops (within the cap).
    #[test]
    fn spray_hits_the_lens() {
        let mut l = LensSim::default();
        for _ in 0..60 {
            l.step(0.0, 30.0, 1.0 / 60.0);
        }
        assert!(l.is_empty());
        for _ in 0..30 {
            l.splash(1.0, 1.0 / 60.0);
        }
        assert!(l.len() >= 15 && l.len() <= LENS_MAX, "{}", l.len());
    }

    /// Next-Gen lens drops: small drops stick, big ones run down leaving a
    /// trail of tiny ones, touching drops merge, and speed still wipes.
    #[test]
    fn next_gen_lens_drops_run_and_merge() {
        assert!(LensSim::slide_speed(0.1) < 0.005);
        assert!(LensSim::slide_speed(0.4) > 20.0 * LensSim::slide_speed(0.1));
        let mut l = LensSim { next_gen: true, ..LensSim::default() };
        l.drops.push(LensSimDrop { x: 0.3, y: 0.2, size: 0.4, age: 0.0, life: 100.0, slide: 0.0 });
        l.drops.push(LensSimDrop { x: 0.7, y: 0.2, size: 0.1, age: 0.0, life: 100.0, slide: 0.0 });
        for _ in 0..60 {
            l.step(0.0, 0.0, 1.0 / 60.0);
        }
        let big = l.drops.iter().find(|d| (d.x - 0.3).abs() < 0.01 && d.size > 0.3).unwrap();
        let small = l.drops.iter().find(|d| (d.x - 0.7).abs() < 0.01 && d.size > 0.09).unwrap();
        assert!(big.y > 0.25, "{}", big.y);
        assert!(small.y < 0.21, "{}", small.y);
        assert!(l.drops.iter().filter(|d| d.size <= 0.05).count() >= 3, "trail");
        // Two touching drops become one, the area kept.
        let mut m = LensSim { next_gen: true, ..LensSim::default() };
        m.drops.push(LensSimDrop { x: 0.5, y: 0.5, size: 0.15, age: 0.0, life: 100.0, slide: 0.0 });
        m.drops.push(LensSimDrop { x: 0.505, y: 0.5, size: 0.12, age: 0.0, life: 100.0, slide: 0.0 });
        m.step(0.0, 0.0, 1.0 / 60.0);
        assert_eq!(m.len(), 1);
        assert!((m.drops[0].size - (0.15f32 * 0.15 + 0.12 * 0.12).sqrt()).abs() < 1e-3);
        let mut fast = LensSim { next_gen: true, ..LensSim::default() };
        let mut stand = LensSim { next_gen: true, ..LensSim::default() };
        for _ in 0..600 {
            fast.step(1.0, 40.0, 1.0 / 60.0);
            stand.step(1.0, 0.0, 1.0 / 60.0);
        }
        assert!(fast.len() < stand.len() / 2, "{} vs {}", fast.len(), stand.len());
    }
}
