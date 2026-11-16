use super::*;

/// One particle (0x20 bytes, packed as the game packs it).
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub description: u16,
    /// Its emitter's colour slot ([`Emitter::palette`]; the draw groups
    /// particles by emitter and reads the slot there, 0x48BDF0).
    pub palette: u16,
    pub key: u32,
    /// +8: life left, 1/1024 s.
    pub life: i16,
    /// +0xA: angle, 65536 a turn; bit 0 flips the spin.
    pub angle: u16,
    /// +0xC: speed, 1/128 m/s.
    pub speed: u16,
    /// +0xE: the share of the owner's motion it is carried by, 1/255.
    pub carried: u8,
    pub position: Vec3,
    /// +0x1C: direction, 1/127.
    pub direction: [i8; 3],
    /// +0x1F: flip-book frame.
    pub frame: u8,
    /// Clone only (rate independence): the particle's identity in
    /// its system, counted up at each spawn ([`System::emit`]), so a drawn
    /// frame between two ticks pairs a particle in both ([`between`]).
    /// Never read by the simulation.
    pub id: u32,
}

impl Particle {
    pub fn velocity(&self) -> Vec3 {
        let s = self.speed as f32 * 0.0078125;
        Vec3::new(self.direction[0] as f32, self.direction[1] as f32, self.direction[2] as f32) * (s * 0.007874016)
    }
    #[cfg(test)]
    pub(crate) fn set_velocity_for_test(&mut self, v: Vec3) {
        self.set_velocity(v);
    }
    pub(super) fn set_velocity(&mut self, v: Vec3) {
        let len = v.length();
        // 0x49326E: 1/|v| when |v| > 1e−5, else 1e10.
        let inv = if len > 1e-5 { 1.0 / len } else { 1e10 };
        self.speed = (len * 128.0).min(65535.0) as u16;
        self.direction = [(v.x * inv * 127.0) as i8, (v.y * inv * 127.0) as i8, (v.z * inv * 127.0) as i8];
    }
}

/// The live particles (the manager 0x8357E0's groups).
#[derive(Clone, Debug, Default)]
pub struct System {
    pub particles: Vec<Particle>,
    /// The views spawning is gated on (0x491E50 tests up to three).
    pub views: Vec<Vec3>,
    /// g_ParticleSystemEnable (0x870CEC, UI_PC_Customize_Options' Particle
    /// System) off: 0x491E50 then spawns only descriptions with the
    /// low-detail byte +0x21.
    pub particles_off: bool,
    /// The colour slots in use (0x8A3F60, 0x40 bytes each).
    pub palettes: Vec<Cubic>,
    /// Clone only (Particles High / Ultra, `render::gpu_particles`): when
    /// set, every particle spawned is also copied here. None (native): no
    /// record; the spawns are the same either way.
    pub births: Option<Vec<Particle>>,
    /// Clone only (rate independence): the id the next spawn gets ([`Particle::id`]).
    pub next_id: u32,
}

impl System {
    /// A colour slot for a cubic (an equal one is shared): the value for
    /// [`Emitter::palette`].
    pub fn palette(&mut self, c: Cubic) -> u16 {
        if let Some(i) = self.palettes.iter().position(|p| p.0 == c.0) {
            return i as u16 + 1;
        }
        self.palettes.push(c);
        self.palettes.len() as u16
    }

    /// 0x491E50: emit `amount` from an emitter — seconds of a running
    /// emitter (`amount ≥ 0`, clamped to the time it has left on), or a
    /// one-off burst (`amount < 0`, the contact effects' −share). A burst
    /// fires whatever the emitter's state; nothing spawns beyond the
    /// description's range from every view.
    pub fn emit(&mut self, library: &Library, e: &mut Emitter, amount: f32) {
        let Some(d) = library.descriptions.get(e.description) else { return };
        let at = e.transform.w_axis.truncate();
        let seen = self.views.is_empty() || self.views.iter().take(3).any(|v| v.distance(at) < d.range);
        if !seen || !(e.on || amount < 0.0) {
            return;
        }
        let mut left = if amount < 0.0 {
            -amount
        } else if e.timer != 0.0 && e.timer < amount {
            e.timer
        } else {
            amount
        };
        let mut wait = e.wait;
        if wait < left {
            loop {
                left -= wait;
                if self.particles.len() < MAX_PARTICLES && (!self.particles_off || d.low_detail) {
                    let mut p = spawn(d, e);
                    p.id = self.next_id;
                    self.next_id = self.next_id.wrapping_add(1);
                    self.particles.push(p);
                    if let Some(b) = self.births.as_mut() {
                        b.push(p);
                    }
                }
                // The next one: 1 / (rate ± spread), at most 240 a unit;
                // 0.1 when the rate is not positive.
                let r = e.rng.spread(d.rate);
                wait = if r <= 0.0 { 0.1 } else { (1.0 / r).clamp(0.0, 240.0) };
                if wait > left {
                    break;
                }
            }
        }
        e.wait = if amount < 0.0 { 0.0 } else { wait - left };
    }

    /// 0x492E10's on/off half for one running emitter: emit for `dt`, then
    /// count its state's time down and switch (on → off for +0x2C ± +0x30,
    /// off → on for +0x24 ± +0x28; 0 holds the new state for ever).
    pub fn run(&mut self, library: &Library, e: &mut Emitter, dt: f32) {
        if e.on {
            self.emit(library, e, dt);
        }
        if e.timer == 0.0 {
            return;
        }
        let left = e.timer - dt;
        if left > 0.0 {
            e.timer = left;
            return;
        }
        let Some(d) = library.descriptions.get(e.description) else { return };
        let span = if e.on { d.off_time } else { d.on_time };
        e.on = !e.on;
        e.wait = 0.0;
        e.timer = if span[0] == 0.0 { 0.0 } else { e.rng.spread(span) + left };
    }

    /// 0x492E10's particle half: age by `dt` (whole 1/1024 s), free the
    /// dead, advance the flip-book, and move: per axis v += (∓v²·drag)·dt
    /// (against the motion), z also −gravity·dt, then x += v·dt; carried
    /// particles also move by their share of the owner's velocity.
    pub fn update(&mut self, library: &Library, dt: f32, owner_velocity: impl Fn(u32) -> Option<Vec3>) {
        let ticks = (dt * TICKS) as i32;
        self.particles.retain_mut(|p| {
            let Some(d) = library.descriptions.get(p.description as usize) else { return false };
            let left = p.life as i32 - ticks;
            if left < 1 {
                return false;
            }
            if d.fps != 0 {
                let per = (TICKS / d.fps as f32) as i32;
                if per != 0 {
                    let mask = d.frames().saturating_sub(1) as i32;
                    p.frame = ((p.life as i32 / per - left / per + p.frame as i32) & mask) as u8;
                }
            }
            p.life = left as i16;
            let v = p.velocity();
            let against = |c: f32| if c >= 0.0 { -1.0 } else { 1.0 };
            let vx = against(v.x) * v.x * v.x * d.drag * dt + v.x;
            let vy = against(v.y) * v.y * v.y * d.drag * dt + v.y;
            let vz = (against(v.z) * v.z * v.z * d.drag - d.gravity) * dt + v.z;
            let nv = Vec3::new(vx, vy, vz);
            p.position += nv * dt;
            if d.carried && d.inherit[0] > 0.0 {
                if let Some(o) = owner_velocity(p.key) {
                    p.position += o * dt * (p.carried as f32 / 255.0);
                }
            }
            p.set_velocity(nv);
            true
        });
    }

    /// The quads 0x48BDF0 draws for a view: per particle the age fraction
    /// t = 1 − life / +0x3C, the colour cubic (clamped 0..255), the size
    /// cubic's column 0 (bounded so size / depth · `pixels_per_metre` ≤
    /// +0x8C; `pixels_per_metre` is the view's +0x0C, 256 / tan(fov / 2),
    /// [`crate::render::cull_scale`], not the screen's pixels: at 1280 × 960
    /// that would shrink the event markers' near rings to 2.4 m where the
    /// original's apitrace shows 3.5 m), for the second list the angle +0xA ±
    /// the size cubic's column 1, the frame's cell of the flip-book and
    /// the aspect pair. `up` is the camera's up (the look-at's −down).
    pub fn quads(
        &self,
        library: &Library,
        eye: Vec3,
        right: Vec3,
        up: Vec3,
        forward: Vec3,
        pixels_per_metre: f32,
    ) -> Vec<Quad> {
        self.quads_where(library, eye, right, up, forward, pixels_per_metre, |_| true)
    }

    /// [`System::quads`] for the particles `keep` accepts (Particles High /
    /// Ultra leave the smoke to `render::gpu_particles`).
    #[allow(clippy::too_many_arguments)]
    pub fn quads_where(
        &self,
        library: &Library,
        eye: Vec3,
        right: Vec3,
        up: Vec3,
        forward: Vec3,
        pixels_per_metre: f32,
        keep: impl Fn(&Particle) -> bool,
    ) -> Vec<Quad> {
        self.quads_of_where(&self.particles, library, eye, right, up, forward, pixels_per_metre, keep)
    }

    /// [`System::quads`] of `particles` (the system's own, or a drawn
    /// frame's copy of them: the tick snapshot moves the carried ones with their
    /// owner's drawn pose there), with this system's colour slots.
    #[allow(clippy::too_many_arguments)]
    pub fn quads_of(
        &self,
        particles: &[Particle],
        library: &Library,
        eye: Vec3,
        right: Vec3,
        up: Vec3,
        forward: Vec3,
        pixels_per_metre: f32,
    ) -> Vec<Quad> {
        self.quads_of_where(particles, library, eye, right, up, forward, pixels_per_metre, |_| true)
    }

    /// [`System::quads_where`] of `particles` (as [`System::quads_of`]).
    #[allow(clippy::too_many_arguments)]
    pub fn quads_of_where(
        &self,
        particles: &[Particle],
        library: &Library,
        eye: Vec3,
        right: Vec3,
        up: Vec3,
        forward: Vec3,
        pixels_per_metre: f32,
        keep: impl Fn(&Particle) -> bool,
    ) -> Vec<Quad> {
        self.quads_scaled_where(particles, &[], library, eye, right, up, forward, pixels_per_metre, keep)
    }

    /// [`System::quads_of_where`] with each particle's size × `scale[i]`
    /// (a particle born in the newest tick grows in over the
    /// frames before it, [`between`]); a missing scale is 1 (the native
    /// draw: no multiply).
    #[allow(clippy::too_many_arguments)]
    pub fn quads_scaled_where(
        &self,
        particles: &[Particle],
        scale: &[f32],
        library: &Library,
        eye: Vec3,
        right: Vec3,
        up: Vec3,
        forward: Vec3,
        pixels_per_metre: f32,
        keep: impl Fn(&Particle) -> bool,
    ) -> Vec<Quad> {
        let mut out = Vec::with_capacity(particles.len());
        // The first list (+0x20 = 0) draws before the second.
        for second in [false, true] {
            for (i, p) in particles.iter().enumerate().filter(|(_, p)| keep(p)) {
                let Some(d) = library.descriptions.get(p.description as usize) else { continue };
                if (d.orientation != 0) != second {
                    continue;
                }
                let t = 1.0 - p.life as f32 / TICKS / d.life[0];
                let color = p.palette.checked_sub(1).and_then(|i| self.palettes.get(i as usize)).unwrap_or(&d.color);
                let c = color.at(t).map(|x| x.clamp(0.0, 255.0));
                let curve = d.size.at(t);
                let mut size = match scale.get(i) {
                    Some(&k) if k != 1.0 => curve[0] * k,
                    _ => curve[0],
                };
                let depth = (p.position - eye).dot(forward);
                let on_screen = if size <= 0.0 || depth <= 0.0 { 0.0 } else { size / depth * pixels_per_metre };
                let bound = if d.max_pixels == 0.0 { MAX_PIXELS } else { d.max_pixels.min(MAX_PIXELS) };
                if bound < on_screen {
                    size = depth / pixels_per_metre * bound;
                }
                let cols = 1u32 << d.columns_log2.min(7);
                let rows = 1u32 << d.rows_log2.min(7);
                let (cw, rh) = (1.0 / cols as f32, 1.0 / rows as f32);
                let u0 = (p.frame as u32 & (cols - 1)) as f32 * cw;
                let v0 = ((p.frame as u32 >> d.columns_log2.min(7)) & (rows - 1)) as f32 * rh;
                let [aw, ah] = ASPECT[d.aspect as usize];
                let (w, h) = (size * aw * 0.5, size * ah * 0.5);
                let at = p.position;
                let rgba = [c[0] as u8, c[1] as u8, c[2] as u8, c[3] as u8];
                if d.orientation == 0 {
                    // The first list: 0x48BDF0 hands 0x5C0E20 the position,
                    // colour, half extents (size · aspect / 2) and the
                    // cell, but 0x5C0E20 never reads the angle (+0x50):
                    // camera sprites do not spin. Its axes are the view
                    // matrix's columns 0 and 1 (0x5B7DF0 → 0x873370,
                    // 0x8763C0), the look-at's right and down (0x5BA9B0),
                    // and 0x5BB440 writes p + wR + hD (u1, v1), p − wR + hD
                    // (u0, v1), p − wR − hD (u0, v0), p + wR − hD (u1, v0):
                    // u along right, v along down, so the texture stands
                    // upright with its width (aspect x) across the screen.
                    // A colour whose alpha is 0x14 or less is not drawn
                    // (0x5BB440 zeroes the corners).
                    if rgba[3] <= 0x14 {
                        continue;
                    }
                    let (wr, hd) = (right * w, -up * h);
                    out.push(Quad {
                        texture: d.texture,
                        corners: [at + wr + hd, at - wr + hd, at - wr - hd, at + wr - hd],
                        uv: [[u0 + cw, v0 + rh], [u0, v0 + rh], [u0, v0], [u0 + cw, v0]],
                        rgba,
                    });
                    continue;
                }
                let turn = curve[1] as i32;
                let spin = if p.angle & 1 != 0 { -turn } else { turn };
                let angle = (p.angle as i32 + spin) as u16;
                let a = angle as f32 * (std::f32::consts::TAU / 65536.0);
                let (s, co) = a.sin_cos();
                // The second list's planes (0x48BDF0): A, B are the
                // identity rows (1: X, Y; 2: X, Z; 3: Y, Z) turned by the
                // angle (0x43C370 cos, 0x43C2B0 sin): 1 A (c, s, 0) B
                // (−s, c, 0); 2 A (c, 0, −s) B (s, 0, c); 3 A (0, c, s)
                // B (0, −s, c).
                let (r, u) = match d.orientation {
                    1 => (Vec3::new(co, s, 0.0), Vec3::new(-s, co, 0.0)),
                    2 => (Vec3::new(co, 0.0, -s), Vec3::new(s, 0.0, co)),
                    _ => (Vec3::new(0.0, co, s), Vec3::new(0.0, -s, co)),
                };
                let (wr, hu) = (r * w, u * h);
                out.push(Quad {
                    texture: d.texture,
                    corners: [at - wr + hu, at - wr - hu, at + wr - hu, at + wr + hu],
                    uv: [[u0, v0], [u0 + cw, v0], [u0 + cw, v0 + rh], [u0, v0 + rh]],
                    rgba,
                });
            }
        }
        out
    }
}
