use super::*;

/// One instance (48 bytes): a spark streak (head, tail) or a flame (centre,
/// axis × half length).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    /// xyz head / centre, w the half width (m).
    pub a: [f32; 4],
    /// xyz tail / axis (half length), w kind (0 spark, 1 flame).
    pub b: [f32; 4],
    /// x age 0..1, y temperature / intensity, z seed, w nitrous blue share.
    pub c: [f32; 4],
}

/// A light the sparks or flames cast this frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Light {
    /// xyz position, w reach (m).
    pub position: [f32; 4],
    /// rgb (linear, × strength), w unused.
    pub colour: [f32; 4],
}

/// The streak's shutter: a fragment draws the path it covers in this time
/// (a 1/60 s exposure), at most [`STREAK_MAX`] long.
pub const SHUTTER: f32 = 1.0 / 60.0;
pub const STREAK_MAX: f32 = 0.4;
/// A streak's half width (m): a chip of a millimetre or two, glowing.
pub const HALF_WIDTH: [f32; 2] = [0.0035, 0.0035];

/// Spark instances from the fragments.
pub fn spark_instances(sparks: &[Spark], out: &mut Vec<Instance>) {
    for s in sparks {
        let t = s.temperature();
        if t < 0.04 {
            continue;
        }
        let travel = s.velocity * SHUTTER;
        let tail = s.position - travel * (STREAK_MAX / travel.length().max(STREAK_MAX));
        out.push(Instance {
            a: s.position.extend(HALF_WIDTH[0] + HALF_WIDTH[1] * t).to_array(),
            b: tail.extend(0.0).to_array(),
            c: [s.age / s.life, t, s.seed, 0.0],
        });
    }
}

/// Glow instances from the native contact glows: centre, a short axis
/// along the slide (the particle's motion), its radius from the native
/// size cubic (× 0.3: the sprite's halo was the texture's, the hot spot is
/// its core), the intensity its native alpha × brightness.
pub fn glow_instances(library: &Library, sim: &Sim, particles: &[Particle], out: &mut Vec<Instance>) {
    for p in particles {
        if sim.kind_cached(p.description as usize) != Some(Kind::Glow) {
            continue;
        }
        let Some(d) = library.descriptions.get(p.description as usize) else { continue };
        let t = (1.0 - p.life as f32 / 1024.0 / d.life[0].max(1e-3)).clamp(0.0, 1.0);
        let c = d.color.at(t);
        let k = (c[3] / 255.0).clamp(0.0, 1.0) * ((c[0] + c[1] + c[2]) / 765.0).clamp(0.0, 1.0);
        if k < 0.01 {
            continue;
        }
        let radius = (d.size.at(t)[0].abs() * 0.3).clamp(0.04, 0.4);
        let v = p.velocity();
        let axis = if v.length_squared() > 1e-4 { v.normalize() * (v.length() * 0.012).min(0.35) } else { Vec3::ZERO };
        let seed = (p.key.wrapping_mul(2654435761) ^ (p.angle as u32).wrapping_mul(40503)) as f32 / u32::MAX as f32;
        out.push(Instance {
            a: p.position.extend(radius).to_array(),
            b: axis.extend(2.0).to_array(),
            c: [t, k, seed, 0.0],
        });
        if out.len() >= MAX_GLOWS {
            break;
        }
    }
}

/// Flame instances from the native flame particles: centre, axis along the
/// particle's own motion (the flame's direction out of the exhaust), the
/// native size cubic at its age.
pub fn flame_instances(library: &Library, sim: &Sim, particles: &[Particle], out: &mut Vec<Instance>) {
    // Many overlapping flames (a shot's steady 8 a frame per exhaust) would
    // add up to a flat white: each owner's flames share the brightness of
    // about six (a pop's few keep theirs).
    let mut owners: Vec<(u32, u32)> = Vec::new();
    for p in particles.iter().filter(|p| sim.kind_cached(p.description as usize) == Some(Kind::Flame)) {
        match owners.iter_mut().find(|o| o.0 == p.key) {
            Some(o) => o.1 += 1,
            None => owners.push((p.key, 1)),
        }
    }
    let share = |key: u32| owners.iter().find(|o| o.0 == key).map_or(1.0, |o| (6.0 / o.1 as f32).sqrt().min(1.0));
    let mut n = 0;
    for p in particles {
        if sim.kind_cached(p.description as usize) != Some(Kind::Flame) {
            continue;
        }
        let Some(d) = library.descriptions.get(p.description as usize) else { continue };
        let t = (1.0 - p.life as f32 / 1024.0 / d.life[0].max(1e-3)).clamp(0.0, 1.0);
        let size = d.size.at(t)[0].max(0.0).clamp(0.05, 1.2);
        let v = p.velocity();
        let axis = if v.length_squared() > 1e-4 { v.normalize() } else { Vec3::NEG_Z };
        let half = size * (0.55 + 0.4 * (1.0 - t));
        let seed = (p.key.wrapping_mul(2654435761) ^ (p.angle as u32).wrapping_mul(40503)) as f32 / u32::MAX as f32;
        out.push(Instance {
            a: p.position.extend(size * 0.3).to_array(),
            b: (axis * half).extend(1.0).to_array(),
            c: [t, (1.0 - t * 0.7) * share(p.key), seed, 1.0],
        });
        n += 1;
        if n >= MAX_FLAMES {
            break;
        }
    }
}

/// The frame's lights: per owner the mean of its flames (blue-white core,
/// orange body), and the hot sparks bucketed into 3 m cells (orange-white
/// by their heat); the strongest [`MAX_LIGHTS`].
pub fn lights(library: &Library, sim: &Sim, sparks: &[Spark], particles: &[Particle]) -> Vec<Light> {
    let mut groups: Vec<(u32, Vec3, f32)> = Vec::new();
    for p in particles {
        if sim.kind_cached(p.description as usize) != Some(Kind::Flame) {
            continue;
        }
        let Some(d) = library.descriptions.get(p.description as usize) else { continue };
        let t = (1.0 - p.life as f32 / 1024.0 / d.life[0].max(1e-3)).clamp(0.0, 1.0);
        let w = 1.0 - t;
        match groups.iter_mut().find(|g| g.0 == p.key) {
            Some(g) => {
                g.1 += p.position * w;
                g.2 += w;
            }
            None => groups.push((p.key, p.position * w, w)),
        }
    }
    let mut out: Vec<(f32, Light)> = Vec::new();
    for (_, sum, w) in groups {
        if w <= 0.05 {
            continue;
        }
        let at = sum / w;
        let s = (w / 6.0).min(1.0);
        let colour = Vec3::new(0.55, 0.62, 1.0) * (0.9 * s);
        out.push((s, Light { position: at.extend(2.5 + 1.5 * s).to_array(), colour: colour.extend(0.0).to_array() }));
    }
    // The sparks light only round the contact — the hot spot and
    // the fragments still near it, in 2 m cells — a small pool (the light
    // multiplies the surface: at most × (1 + SPARK_LIGHT) at its centre,
    // the look rules' budget for light the original does not have), the
    // [`MAX_SPARK_LIGHTS`] strongest. A fragment far from its contact
    // lights nothing.
    let mut cells: Vec<((i32, i32, i32), Vec3, f32)> = Vec::new();
    let mut add = |p: Vec3, w: f32| {
        let k = ((p.x / 2.0).floor() as i32, (p.y / 2.0).floor() as i32, (p.z / 2.0).floor() as i32);
        match cells.iter_mut().find(|c| c.0 == k) {
            Some(c) => {
                c.1 += p * w;
                c.2 += w;
            }
            None => cells.push((k, p * w, w)),
        }
    };
    for s in sparks {
        let t = s.temperature();
        if t < 0.3 || s.age > 0.15 {
            continue;
        }
        add(s.position, t * 0.25);
    }
    for p in particles {
        if sim.kind_cached(p.description as usize) != Some(Kind::Glow) {
            continue;
        }
        let Some(d) = library.descriptions.get(p.description as usize) else { continue };
        let t = (1.0 - p.life as f32 / 1024.0 / d.life[0].max(1e-3)).clamp(0.0, 1.0);
        let a = (d.color.at(t)[3] / 255.0).clamp(0.0, 1.0);
        add(p.position, a);
    }
    let mut sparks: Vec<(f32, Light)> = cells
        .into_iter()
        .map(|(_, sum, w)| {
            let s = (w / 4.0).min(1.0);
            let colour = Vec3::new(1.0, 0.68, 0.38) * (SPARK_LIGHT * s);
            (
                s * 0.9,
                Light {
                    position: (sum / w + Vec3::Z * 0.1).extend(1.2 + 1.0 * s).to_array(),
                    colour: colour.extend(0.0).to_array(),
                },
            )
        })
        .collect();
    sparks.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.extend(sparks.into_iter().take(MAX_SPARK_LIGHTS));
    out.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.into_iter().take(MAX_LIGHTS).map(|(_, l)| l).collect()
}

/// The sparks' light at a pool's centre (red channel; the surface's own
/// colour × (1 + this)).
pub const SPARK_LIGHT: f32 = 0.5;

/// The light's falloff the shader applies (mirrors sparks_fire.wgsl
/// `falloff`): (1 − d²/r²)² inside the reach, 0 beyond.
pub fn falloff(d: f32, reach: f32) -> f32 {
    let x = (1.0 - (d * d) / (reach * reach)).max(0.0);
    x * x
}

/// The black-body ramp (mirrors sparks_fire.wgsl `heat_colour`): 0 dull
/// red … 1 white, linear RGB, not yet scaled by the emission.
pub fn heat_colour(t: f32) -> Vec3 {
    let t = t.clamp(0.0, 1.0);
    let r = (t * 3.0).min(1.0);
    let g = ((t - 0.25) * 1.6).clamp(0.0, 1.0);
    let b = ((t - 0.65) * 2.8).clamp(0.0, 1.0);
    Vec3::new(r, g * g, b * b) * (0.25 + 1.75 * t * t)
}

/// A position projected (for tests): NDC xyz.
pub fn project(view_proj: Mat4, p: Vec3) -> Vec3 {
    let c = view_proj * Vec4::new(p.x, p.y, p.z, 1.0);
    c.truncate() / c.w
}
