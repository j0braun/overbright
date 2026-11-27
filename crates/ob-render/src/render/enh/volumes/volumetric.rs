//! Volumetric Fog (docs/notes/graphics/atmosphere.md; a **clone deviation**, opt-in;
//! SPEED2.EXE has no such pass: its fog is world.efx's per-vertex fog and
//! the horizon fog grid). Every tier (Low, High, Next-Gen) is the one froxel
//! medium, [`crate::render::atmosphere::Atmosphere`], at the tier's grid, range
//! and light counts ([`crate::render::atmosphere::tier`]); this module holds a
//! frame's inputs and the nearest-lamps pick other effects share.

use crate::render::camera::Camera;
use glam::Vec3;

pub const MAX_LAMPS: usize = 64;

/// Lamps further than this are left out by [`nearest_lamps`].
pub const REACH: f32 = 350.0;

/// A frame's inputs.
#[derive(Clone)]
pub struct Frame {
    pub depth: wgpu::TextureView,
    pub samples: u32,
    pub camera: Camera,
    pub aspect: f32,
    pub reversed: bool,
    /// The medium's inputs (its tier, lights, spots, shadows, smoke).
    pub next: Box<crate::render::atmosphere::NextGen>,
}

/// The `n` lamps nearest `eye` and not behind it (within [`REACH`]), the
/// nearest first.
pub fn nearest_lamps(
    lamps: &[crate::render::lamps::Lamp],
    eye: Vec3,
    forward: Vec3,
    n: usize,
) -> Vec<(Vec3, f32, Vec3)> {
    let mut v: Vec<(f32, usize)> = lamps
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            let p = l.light.position;
            let d = p - eye;
            let dist = d.length();
            (dist < REACH && d.dot(forward) > -l.light.outer).then_some((dist, i))
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v.into_iter()
        .take(n.min(MAX_LAMPS))
        .map(|(_, i)| {
            let g = crate::render::lamps::gpu_lamp(&lamps[i].light, -1);
            (Vec3::from_slice(&g.position[..3]), g.position[3], Vec3::from_slice(&g.colour[..3]))
        })
        .collect()
}
