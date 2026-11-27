use super::*;
pub const TAIL_REACH: f32 = 5.0;
/// The other cars' headlights (spots): reach, cone (cosine of the half
/// angle), strength on the pool colour.
pub const HEAD_REACH: f32 = 28.0;
pub const HEAD_COS: f32 = 0.82;
pub const HEAD_GAIN: f32 = 0.9;

/// This frame's car lights: every car's tail lights (one red light behind
/// its box, `occluders` index as the owner, whose own ellipsoid it skips)
/// and, unless Headlight Beams draws them (`beams`), a spot per headlight
/// pool but the player's (the native projector lights the player's road).
pub fn car_lights(occluders: &[Occluder], pools: &[crate::render::HeadlightPool], beams: bool) -> Vec<Lamp> {
    let mut out = Vec::new();
    let q = |v: f32| (v / EMITTER_COLOUR_SCALE * 255.0).round().clamp(0.0, 255.0) as u8;
    for (i, o) in occluders.iter().enumerate().take(crate::render::lamps::MAX_OCCLUDERS) {
        let h = o.max.z - o.min.z;
        let at = o.transform.transform_point3(Vec3::new(o.min.x - 0.35, 0.5 * (o.min.y + o.max.y), o.min.z + 0.45 * h));
        out.push(Lamp {
            section: u32::MAX,
            light: WorldLight {
                kind: 2,
                sub: SUB_TAIL,
                on: true,
                colour: TAIL.map(q),
                position: at,
                outer: TAIL_REACH,
                direction: Vec3::NEG_Z,
                intensity: 1.0,
                inner: i as f32,
                cone: 0.0,
            },
        });
    }
    if !beams {
        for p in pools.iter().skip(1) {
            let m: Mat4 = p.transform;
            let fwd = m.transform_vector3(Vec3::X).normalize_or(Vec3::X);
            let at = m.transform_point3(Vec3::new(2.4, 0.0, 0.7));
            let owner = occluders
                .iter()
                .position(|o| o.transform.w_axis.truncate().distance(m.w_axis.truncate()) < 1.0)
                .map_or(-1.0, |i| i as f32);
            let c = Vec3::new(p.color[0] as f32, p.color[1] as f32, p.color[2] as f32) / 255.0 * HEAD_GAIN;
            out.push(Lamp {
                section: u32::MAX,
                light: WorldLight {
                    kind: 3,
                    sub: SUB_HEAD,
                    on: true,
                    colour: [q(c.x), q(c.y), q(c.z)],
                    position: at,
                    outer: HEAD_REACH,
                    direction: (fwd - Vec3::Z * 0.12).normalize(),
                    intensity: 1.0,
                    inner: owner,
                    cone: 2.0 * HEAD_COS.acos(),
                },
            });
        }
    }
    out
}

/// A light's Modern Lighting values in its uploaded `GpuLamp`: the pack
/// lamps phase 1's fitted colour (`modern_light::colour`, 0 outside the
/// district) and colour.w ≥ 0; the emitters colour.w = −1, the car lights
/// −2 with shadow.x = −2 − the owner car (its ellipsoid skipped), the
/// headlights' cone in direction.w.
pub fn tag(lights: &mut [GpuLamp], kept: &[usize], lamps: &[Lamp], district: ([f32; 2], [f32; 2])) {
    for (g, &i) in lights.iter_mut().zip(kept) {
        let l = &lamps[i].light;
        match l.sub {
            SUB_EMITTER | SUB_TAIL | SUB_HEAD => {
                let c = l.colour.map(|b| f32::from(b) / 255.0 * EMITTER_COLOUR_SCALE);
                g.shadow[1] = c[0];
                g.shadow[2] = c[1];
                g.shadow[3] = c[2];
                if l.sub == SUB_EMITTER {
                    g.colour[3] = -1.0;
                    // Its facing (zero: all around), not gpu_lamp's default.
                    g.direction = [l.direction.x, l.direction.y, l.direction.z, -2.0];
                } else {
                    g.colour[3] = -2.0;
                    g.shadow[0] = if l.inner >= 0.0 { -2.0 - l.inner } else { -1.0 };
                }
            }
            _ => {
                let c = crate::render::modern_light::colour(l, district);
                g.shadow[1] = c[0];
                g.shadow[2] = c[1];
                g.shadow[3] = c[2];
                g.colour[3] = g.colour[3].max(0.0);
            }
        }
    }
}
