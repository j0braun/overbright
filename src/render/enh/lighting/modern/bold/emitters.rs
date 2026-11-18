use super::*;
// ---------------------------------------------------------------------
// Emitters: the neon and the lit signs.

/// Emitters merge per cell of this size (m).
pub const EMITTER_CELL: f32 = 5.0;
/// An emitter's light per unit of mean emitted colour (vertex-colour units).
pub const EMITTER_GAIN: f32 = 1.3;

/// An additive texture that lights its surroundings: the signs and the
/// neon (SGN_*, *NEON*), not the fake light beams, the flare / mist
/// sprites, the distant city grid, the headlight glows or the windows
/// (measured with the private developer probe `modern_light_inventory`).
pub fn emissive_name(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    (n.starts_with("SGN_") || n.contains("NEON") || n.contains("_SIGN"))
        && !n.starts_with("SFX_FLARE")
        && !n.starts_with("SFX_LIGHT_BEAM")
        && !n.contains("BARRIER")
}

/// The world's emitters: per [`EMITTER_CELL`] cell (and section), the
/// emissive triangles' area-weighted centre and mean colour (the texture's
/// mean × alpha × the vertex colour × 2, world.efx's), as a point light
/// whose reach and strength grow with the area.
pub fn emitters(world: &crate::world::World) -> Vec<Lamp> {
    use crate::formats::texture::Blend;
    use std::collections::HashMap;
    let _span = crate::prof::acc::span("modern lighting: emitters");
    #[derive(Default)]
    struct Cell {
        area: f32,
        centre: Vec3,
        colour: Vec3,
        normal: Vec3,
    }
    let mut cells: HashMap<(u32, i32, i32, i32), Cell> = HashMap::new();
    let mut means: HashMap<u32, Option<[f32; 4]>> = HashMap::new();
    for p in &world.placements {
        let Some(solid) = world.solids.get(p.solid) else { continue };
        // Hidden at load: race-only scenery (barriers) and markers.
        if solid.name == crate::render::SKYDOME_SOLID || p.flags & crate::world::HIDDEN_FLAG != 0 {
            continue;
        }
        for mat in &solid.materials {
            let Some(&hash) = solid.textures.get(mat.texture_slot as usize) else { continue };
            let mean = *means.entry(hash).or_insert_with(|| {
                let t = world.textures.get(&hash)?;
                (matches!(t.blend, Blend::Additive | Blend::DestAlphaAdditive) && emissive_name(&t.name))
                    .then(|| crate::render::rt::texture_mean(t))
            });
            let Some(mean) = mean else { continue };
            let end = (mat.first_index + mat.index_count) as usize;
            let Some(ix) = solid.indices.get(mat.first_index as usize..end.min(solid.indices.len())) else { continue };
            for tri in ix.chunks_exact(3) {
                let [Some(a), Some(b), Some(c)] = [tri[0], tri[1], tri[2]].map(|i| solid.vertices.get(i as usize))
                else {
                    continue;
                };
                let w = [a, b, c].map(|v| p.transform.transform_point3(Vec3::from(v.position)));
                let cross = (w[1] - w[0]).cross(w[2] - w[0]);
                let area = cross.length() * 0.5;
                // Huge quads are sky / horizon cards, not signs.
                if !(area > 1e-4 && area < 400.0) {
                    continue;
                }
                let vc = [a, b, c].iter().fold(Vec3::ZERO, |s, v| {
                    s + Vec3::new(v.color[2] as f32, v.color[1] as f32, v.color[0] as f32) / 765.0
                });
                let colour = vc * Vec3::new(mean[0], mean[1], mean[2]) * (2.0 * mean[3]);
                let centre = (w[0] + w[1] + w[2]) / 3.0;
                let k = (centre / EMITTER_CELL).floor();
                let e = cells.entry((p.section, k.x as i32, k.y as i32, k.z as i32)).or_default();
                e.area += area;
                e.centre += centre * area;
                e.colour += colour * area;
                e.normal += cross * 0.5;
            }
        }
    }
    let mut out: Vec<Lamp> = cells
        .into_iter()
        .filter_map(|((section, ..), c)| {
            emitter_light(c.area, c.centre / c.area.max(1e-6), c.colour / c.area.max(1e-6), c.normal / c.area.max(1e-6))
                .map(|light| Lamp { section, light })
        })
        .collect();
    // A stable order (HashMap's is not): by position.
    out.sort_by(|a, b| {
        a.light.position.to_array().partial_cmp(&b.light.position.to_array()).unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

pub(in crate::render) static EMITTERS: RwLock<Vec<Lamp>> = RwLock::new(Vec::new());

/// Work out the loaded world's [`emitters`] (at a region's load).
pub fn load_emitters(world: &crate::world::World) {
    let t = std::time::Instant::now();
    let e = emitters(world);
    log::info!("modern lighting: {} emitters in {:.1} ms", e.len(), t.elapsed().as_secs_f64() * 1e3);
    *EMITTERS.write().unwrap() = e;
}

/// Make `emitters` (the [`emitters`] of the world being loaded, worked
/// out ahead on another thread) the loaded world's, as [`load_emitters`]
/// would at this point.
pub fn set_loaded_emitters(emitters: Vec<Lamp>) {
    *EMITTERS.write().unwrap() = emitters;
}

/// The loaded world's emitters.
pub fn loaded_emitters() -> std::sync::RwLockReadGuard<'static, Vec<Lamp>> {
    EMITTERS.read().unwrap()
}

/// One emitter's light from its cell's emissive `area` (m²), centre,
/// mean colour and mean normal (area-weighted: its length the share of
/// the area facing one way); None when it is too dim to light anything.
/// The light's direction is the normal where most of the area faces one
/// way (it lights only the half-space in front: a sign does not light the
/// facade it hangs on), else zero (all around).
pub fn emitter_light(area: f32, centre: Vec3, colour: Vec3, normal: Vec3) -> Option<WorldLight> {
    let peak = colour.max_element();
    if area < 0.05 || peak < 0.04 {
        return None;
    }
    // Its hue at full value, its strength by the colour's value and the
    // area's size (a sign of a few m² lights the pavement in front of it).
    let size = area.sqrt();
    // Neon reads bright whatever its texture's mean (the tubes are thin on
    // a black card): the hue at full value, by the size.
    let strength = (0.35 + 0.3 * size).min(1.5) * (0.5 + 0.5 * (peak * 4.0).min(1.0));
    let c = colour / peak * strength * EMITTER_GAIN;
    let q = |v: f32| (v / EMITTER_COLOUR_SCALE * 255.0).round().clamp(1.0, 255.0) as u8;
    Some(WorldLight {
        kind: 2,
        sub: SUB_EMITTER,
        on: true,
        colour: [q(c.x), q(c.y), q(c.z)],
        position: centre,
        outer: (7.0 + 3.0 * size).clamp(8.0, 24.0),
        direction: if normal.length() > 0.4 { normal.normalize() } else { Vec3::ZERO },
        intensity: 1.0,
        inner: 0.0,
        cone: 0.0,
    })
}

/// A synthetic light's colour byte 255 in Modern Lighting units.
pub const EMITTER_COLOUR_SCALE: f32 = 2.0;

// ---------------------------------------------------------------------
// The cars' lights.

/// The tail lights' colour (vertex-colour units at their source) and reach.
pub const TAIL: [f32; 3] = [0.45, 0.04, 0.02];
