//! Animated Water (clone deviation, opt-in; docs/water.md).
//!
//! Nothing here is native. SPEED2.EXE draws its water as ordinary world
//! textures: the fountain basins, the canals of the industrial park and
//! one terrain pond with the flip-book `ANM_WATERA_` (14 frames at 12 fps,
//! each frame scrolling), the falling water of the fountains and
//! waterfalls with `TRN_WATERFALL_A` / `_LL` (scrolling down), the hill
//! streams with `TRN_WATER_SURFACE_A_LL` (scrolling) and the swimming
//! pools with the still `ARC_POOL_A_DM` (docs/formats.md "Texture
//! animation and scrolling"; the list of surfaces in docs/water.md, from
//! `examples/water_census.rs`). Off (the default) is that, byte-identical.
//!
//! On: the main view leaves those draws out (`WorldRenderer::set_hide_water`)
//! and this module draws the same triangles, copied to world space at the
//! first use, with `water.wgsl`: the native shading under flow-mapped
//! ripples, splash rings where the sheets land, rain rings, refraction-like
//! distortion, the car cube's reflection and the light-pack lamps' glints
//! (surfaces); a second, faster layer, streaks, a broken bottom and a wet
//! sheen (sheets); then analytic spray and mist billboards where the sheets
//! land, soft against the scene's depth (as the GPU smoke's soft particles). It all moves
//! on the world clock (0x7FB718), so shots are deterministic. The road
//! reflection, the cube and the rear-view mirror keep the native water.

pub mod hq;
pub mod ng;

use crate::render::Gpu;
use crate::render::camera::Camera;
use crate::world::World;
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use std::ops::Range;

/// What a water texture is drawn as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// `ANM_WATERA_` (and its flip-book frames): fountain basins, canals.
    Basin = 0,
    /// `ARC_POOL_A_DM`: the swimming pools (still).
    Pool = 1,
    /// `TRN_WATER_SURFACE_A_LL`: the hill streams.
    Stream = 2,
    /// `TRN_WATERFALL_A`: the fountains' and waterfalls' falling water.
    Sheet = 3,
    /// `TRN_WATERFALL_A_LL`: pipe outflows, the hill waterfall.
    SheetFar = 4,
}

impl Kind {
    pub fn is_sheet(self) -> bool {
        matches!(self, Kind::Sheet | Kind::SheetFar)
    }
}

/// The water textures by name (docs/water.md "Surfaces"). The panorama's
/// far ocean (`TRN_OCEANA_DM`) and the LOD stand-ins are left native.
pub fn kind_of(name: &str) -> Option<Kind> {
    match name {
        n if n.starts_with("ANM_WATERA_") => Some(Kind::Basin),
        "ARC_POOL_A_DM" => Some(Kind::Pool),
        "TRN_WATER_SURFACE_A_LL" => Some(Kind::Stream),
        "TRN_WATERFALL_A" => Some(Kind::Sheet),
        "TRN_WATERFALL_A_LL" => Some(Kind::SheetFar),
        _ => None,
    }
}

/// Whether a mesh draws with worldreflect (mesh flag 0x40, 0x5C58B0):
/// its water is left native.
pub fn is_reflective(mesh_flags: u32) -> bool {
    mesh_flags & crate::formats::solid::MESH_FLAG_REFLECTIVE != 0
}

/// The flip-book's base name.
const FLIPBOOK: &str = "ANM_WATERA_";
/// The single textures after the flip-book's frames, in `Kind` order.
const SINGLES: [&str; 4] = ["ARC_POOL_A_DM", "TRN_WATER_SURFACE_A_LL", "TRN_WATERFALL_A", "TRN_WATERFALL_A_LL"];
/// The texture array's size (every layer resampled to it, with mips).
const TEX: u32 = 256;
const MAX_LAYERS: usize = 24;
const MAX_LAMPS: usize = 8;
/// Landings fed to the splash rings and the spray (nearest first).
pub const MAX_LANDINGS: usize = 32;
/// Spray billboards per landing (24 mist, the rest drops).
pub const PER_EMITTER: u32 = 48;
/// Arcs' apexes fed to the spray (nearest first), and their drops each.
pub const MAX_APEXES: usize = 24;
pub const PER_APEX: u32 = 40;
/// How far the spray is drawn (m).
pub const SPRAY_REACH: f32 = 70.0;
/// Speed of a basin's radial flow and a stream's flow (m/s).
const BASIN_FLOW: f32 = 0.25;
const STREAM_FLOW: f32 = 0.8;
/// Landing cells (m): the bottoms of a sheet are clustered on this grid.
const LANDING_CELL: f32 = 1.25;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub kind: f32,
    pub uv: [f32; 2],
    /// Surfaces: the flow (m/s, world xy). Sheets: height in the fall
    /// (0 landing, 1 top), the fall's length (m).
    pub aux: [f32; 2],
    pub color: [f32; 4],
}

/// A landing: where a sheet's bottom meets the water.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landing {
    pub position: Vec3,
    /// 0.4–1.4, by how much of the sheet lands there.
    pub strength: f32,
    /// The water's native colour there (2 · texture mean · v0).
    pub color: Vec3,
}

/// An arcing jet's apex (the casino's and the park's XO_FOUNTAINWTRA
/// arcs): where drops break off, with the jet's velocity there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Apex {
    pub position: Vec3,
    /// The jet's horizontal velocity at the apex (m/s), from its reach and
    /// heights under gravity (inferred: ballistic).
    pub velocity: Vec3,
    pub color: Vec3,
}

/// How a sheet's triangles run: the flow coordinate of each vertex, 0
/// where the water lands, 1 at its source. A falling sheet by its height;
/// an arc (its top inside its length, reaching further than it rises) by
/// v, the axis its native scroll moves along, toward the end the scroll
/// carries the texture to (`-speed_v · dP/dv`: the low-v end when the v
/// speed is positive).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SheetShape {
    Fall { zlo: f32, height: f32 },
    Arc { vlo: f32, vhi: f32, forward: bool },
}

impl SheetShape {
    /// 0 landing … 1 source.
    pub fn along(self, z: f32, v: f32) -> f32 {
        match self {
            SheetShape::Fall { zlo, height } => ((z - zlo) / height).clamp(0.0, 1.0),
            SheetShape::Arc { vlo, vhi, forward } => {
                let f = ((v - vlo) / (vhi - vlo).max(1e-4)).clamp(0.0, 1.0);
                if forward { f } else { 1.0 - f }
            }
        }
    }
}

/// A sheet's shape from its vertices (world position, v) and its texture's
/// v scroll speed.
pub fn sheet_shape(points: &[(Vec3, f32)], speed_v: f32) -> SheetShape {
    let (mut zlo, mut zhi, mut vlo, mut vhi) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    let mut top = (f32::NEG_INFINITY, 0.0);
    for &(p, v) in points {
        zlo = zlo.min(p.z);
        zhi = zhi.max(p.z);
        vlo = vlo.min(v);
        vhi = vhi.max(v);
        lo = lo.min(p);
        hi = hi.max(p);
        if p.z > top.0 {
            top = (p.z, v);
        }
    }
    let height = (zhi - zlo).max(1e-3);
    let reach = (hi - lo).truncate().length();
    let apex_at = (top.1 - vlo) / (vhi - vlo).max(1e-4);
    // Both ends (the lowest and highest tenth of v) well below the top: a
    // jet rising and falling, not a fall or a cascade (their source is
    // their top).
    let end_z = |lo_end: bool| {
        let z: Vec<f32> = points
            .iter()
            .filter(|p| {
                let f = (p.1 - vlo) / (vhi - vlo).max(1e-4);
                if lo_end { f < 0.1 } else { f > 0.9 }
            })
            .map(|p| p.0.z)
            .collect();
        if z.is_empty() { zhi } else { z.iter().sum::<f32>() / z.len() as f32 }
    };
    let below = (zhi - end_z(true)).min(zhi - end_z(false));
    if reach > height * 2.5 && (0.15..=0.85).contains(&apex_at) && below > (0.4 * height).max(0.5) && speed_v != 0.0 {
        SheetShape::Arc { vlo, vhi, forward: speed_v > 0.0 }
    } else {
        SheetShape::Fall { zlo, height }
    }
}

/// An arc's apex from its vertices (world position, flow coordinate): the
/// top, and the velocity a ballistic jet from the source end to the
/// landing end has there.
pub fn arc_apex(points: &[(Vec3, f32)]) -> Option<(Vec3, Vec3)> {
    let zmax = points.iter().map(|p| p.0.z).fold(f32::NEG_INFINITY, f32::max);
    let mean = |pred: &dyn Fn(&(Vec3, f32)) -> bool| {
        let v: Vec<Vec3> = points.iter().filter(|p| pred(p)).map(|p| p.0).collect();
        (!v.is_empty()).then(|| v.iter().copied().sum::<Vec3>() / v.len() as f32)
    };
    let apex = mean(&|p| p.0.z > zmax - 0.3)?;
    let source = mean(&|p| p.1 > 0.92)?;
    let landing = mean(&|p| p.1 < 0.08)?;
    let g = 9.81;
    let up = ((apex.z - source.z).max(0.05) * 2.0 / g).sqrt();
    let down = ((apex.z - landing.z).max(0.05) * 2.0 / g).sqrt();
    let d = (landing - source).truncate();
    let speed = d.length() / (up + down);
    Some((apex, (d.normalize_or_zero() * speed).extend(0.0)))
}

/// One placement's water at one LOD.
#[derive(Clone, Debug)]
pub struct Record {
    pub placement: usize,
    /// Drawn when the main view draws LOD B (`Placement::mid_solid`).
    pub lod_b: bool,
    pub section: u32,
    pub flags: u16,
    pub surfaces: Range<u32>,
    pub sheets: Range<u32>,
    pub centre: Vec3,
    pub radius: f32,
    pub cull_position: Vec3,
    pub cull_radius: f32,
    pub landings: Vec<Landing>,
    pub apexes: Vec<Apex>,
}

/// The world's water, in world space.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    /// The surfaces' triangles first, then the sheets'.
    pub indices: Vec<u32>,
    pub records: Vec<Record>,
}

fn rgba(c: [u8; 4]) -> [f32; 4] {
    // D3DCOLOR bytes B, G, R, A.
    [c[2] as f32 / 255.0, c[1] as f32 / 255.0, c[0] as f32 / 255.0, c[3] as f32 / 255.0]
}

/// The mean colour of a texture's top level (premultiplied by nothing).
fn texture_mean(world: &World, hash: u32) -> Vec3 {
    let Some(t) = world.textures.get(&hash) else { return Vec3::splat(0.5) };
    let px = t.decode_rgba();
    let n = (px.len() / 4).max(1) as f32;
    let mut s = Vec3::ZERO;
    for c in px.chunks_exact(4) {
        s += Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32);
    }
    s / n / 255.0
}

/// Every water triangle of the world, per placement and LOD.
pub fn build(world: &World) -> Mesh {
    // Which solids have water, and on which materials.
    let mut solid_water: HashMap<usize, Vec<(usize, Kind, u32)>> = HashMap::new();
    for (si, s) in world.solids.iter().enumerate() {
        // Water on a reflective mesh (worldreflect: the industrial park's
        // canals, one terrain pond) keeps its native draw, the road
        // reflection and Wet Roads / SSR with it (`Draw::water` likewise).
        if is_reflective(s.mesh_flags) {
            continue;
        }
        let mut v = Vec::new();
        for (mi, m) in s.materials.iter().enumerate() {
            let Some(&h) = s.textures.get(m.texture_slot as usize) else { continue };
            if let Some(k) = world.textures.get(&h).and_then(|t| kind_of(&t.name)) {
                if m.index_count > 0 {
                    v.push((mi, k, h));
                }
            }
        }
        if !v.is_empty() {
            solid_water.insert(si, v);
        }
    }
    let mut means: HashMap<u32, Vec3> = HashMap::new();
    let mut mesh = Mesh::default();
    let mut surface_idx: Vec<u32> = Vec::new();
    let mut sheet_idx: Vec<u32> = Vec::new();
    let mut sheet_ranges: Vec<Range<u32>> = Vec::new();
    for (pi, p) in world.placements.iter().enumerate() {
        for (lod_b, solid) in [(false, Some(p.solid)), (true, p.mid_solid)] {
            let Some(si) = solid else { continue };
            let Some(mats) = solid_water.get(&si) else { continue };
            let s = &world.solids[si];
            let surf_start = surface_idx.len() as u32;
            let sheet_start = sheet_idx.len() as u32;
            let mut lo = Vec3::splat(f32::INFINITY);
            let mut hi = Vec3::splat(f32::NEG_INFINITY);
            let mut bottoms: HashMap<(i32, i32), (Vec3, u32, f32, Vec3)> = HashMap::new();
            let mut apexes: Vec<Apex> = Vec::new();
            for &(mi, kind, hash) in mats {
                let m = &s.materials[mi];
                let tris = &s.indices[m.first_index as usize..(m.first_index + m.index_count) as usize];
                let mut remap: HashMap<u16, u32> = HashMap::new();
                let world_pos = |i: u16| p.transform.transform_point3(Vec3::from(s.vertices[i as usize].position));
                // The fall's extent (sheets) and the centre (basins).
                let (mut zlo, mut zhi) = (f32::INFINITY, f32::NEG_INFINITY);
                let (mut clo, mut chi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
                for &i in tris {
                    let w = world_pos(i);
                    zlo = zlo.min(w.z);
                    zhi = zhi.max(w.z);
                    clo = clo.min(w);
                    chi = chi.max(w);
                }
                let centre = (clo + chi) * 0.5;
                let fall = (zhi - zlo).max(1e-3);
                let speed_v = world.textures.get(&hash).map_or(0.0, |t| t.scroll.speed[1]);
                // Classified on the solid's own (unscaled) geometry: the
                // placements scale the same jet differently (det 0.05–1.55).
                let shape = match kind.is_sheet().then(|| {
                    let pts: Vec<(Vec3, f32)> = tris
                        .iter()
                        .map(|&i| (Vec3::from(s.vertices[i as usize].position), s.vertices[i as usize].uv[1]))
                        .collect();
                    sheet_shape(&pts, speed_v)
                }) {
                    Some(arc @ SheetShape::Arc { .. }) => arc,
                    _ => SheetShape::Fall { zlo, height: fall },
                };
                let is_arc = matches!(shape, SheetShape::Arc { .. });
                // A stream's flow: against its native scroll in uv, mapped to
                // world space per vertex through the triangles' uv gradients.
                let mut flows: HashMap<u16, Vec3> = HashMap::new();
                if kind == Kind::Stream {
                    let sc = world.textures.get(&hash).map(|t| t.scroll.speed).unwrap_or([0.0, 0.0]);
                    for t in tris.chunks_exact(3) {
                        let (p0, p1, p2) = (world_pos(t[0]), world_pos(t[1]), world_pos(t[2]));
                        let [u0, u1, u2] = [t[0], t[1], t[2]].map(|i| glam::Vec2::from(s.vertices[i as usize].uv));
                        let (e1, e2) = (p1 - p0, p2 - p0);
                        let (d1, d2) = (u1 - u0, u2 - u0);
                        let det = d1.x * d2.y - d2.x * d1.y;
                        if det.abs() < 1e-8 {
                            continue;
                        }
                        let dpdu = (e1 * d2.y - e2 * d1.y) / det;
                        let dpdv = (e2 * d1.x - e1 * d2.x) / det;
                        let dir = -(dpdu * sc[0] + dpdv * sc[1]);
                        for &i in t {
                            *flows.entry(i).or_default() += dir;
                        }
                    }
                }
                let mean = *means.entry(hash).or_insert_with(|| texture_mean(world, hash));
                for &i in tris {
                    let idx = *remap.entry(i).or_insert_with(|| {
                        let v = &s.vertices[i as usize];
                        let w = world_pos(i);
                        lo = lo.min(w);
                        hi = hi.max(w);
                        let color = rgba(v.color);
                        let aux = match kind {
                            Kind::Sheet | Kind::SheetFar => {
                                let h = shape.along(w.z, v.uv[1]);
                                if h < 0.08 {
                                    let key =
                                        ((w.x / LANDING_CELL).floor() as i32, (w.y / LANDING_CELL).floor() as i32);
                                    let e = bottoms.entry(key).or_insert((Vec3::ZERO, 0, f32::INFINITY, Vec3::ZERO));
                                    e.0 += w;
                                    e.1 += 1;
                                    e.2 = e.2.min(w.z);
                                    e.3 += (2.0 * mean * Vec3::new(color[0], color[1], color[2])).min(Vec3::ONE);
                                }
                                // An arc's length negative: the shader's edge and
                                // pulse only for arcs.
                                [h, if is_arc { -fall } else { fall }]
                            }
                            Kind::Basin => {
                                let d = (w - centre).truncate();
                                let r = d.length();
                                let dir = if r > 1e-3 { d / r } else { glam::Vec2::ZERO };
                                (dir * BASIN_FLOW * (r / 2.0).min(1.0)).to_array()
                            }
                            Kind::Stream => flows
                                .get(&i)
                                .map_or([0.0, 0.0], |f| (f.truncate().normalize_or_zero() * STREAM_FLOW).to_array()),
                            Kind::Pool => [0.0, 0.0],
                        };
                        mesh.vertices.push(Vertex {
                            position: w.to_array(),
                            kind: kind as u32 as f32,
                            uv: v.uv,
                            aux,
                            color,
                        });
                        (mesh.vertices.len() - 1) as u32
                    });
                    if kind.is_sheet() { sheet_idx.push(idx) } else { surface_idx.push(idx) }
                }
                if is_arc {
                    // Sorted: the sums below in a fixed order (shots stay
                    // deterministic run to run).
                    let mut keys: Vec<u16> = remap.keys().copied().collect();
                    keys.sort_unstable();
                    let pts: Vec<(Vec3, f32)> = keys
                        .iter()
                        .map(|&i| (world_pos(i), shape.along(world_pos(i).z, s.vertices[i as usize].uv[1])))
                        .collect();
                    let v0 =
                        keys.iter().map(|&i| Vec3::from_slice(&rgba(s.vertices[i as usize].color)[..3])).sum::<Vec3>()
                            / keys.len().max(1) as f32;
                    if let Some((position, velocity)) = arc_apex(&pts) {
                        apexes.push(Apex { position, velocity, color: (2.0 * mean * v0).min(Vec3::ONE) });
                    }
                }
            }
            let mut landings: Vec<Landing> = bottoms
                .into_values()
                .map(|(sum, n, zmin, col)| {
                    let c = sum / n as f32;
                    Landing {
                        position: Vec3::new(c.x, c.y, zmin),
                        strength: ((n as f32).sqrt() / 2.5).clamp(0.4, 1.4),
                        color: col / n as f32,
                    }
                })
                .collect();
            landings
                .sort_by(|a, b| a.position.to_array().map(f32::to_bits).cmp(&b.position.to_array().map(f32::to_bits)));
            let centre = (lo + hi) * 0.5;
            mesh.records.push(Record {
                placement: pi,
                lod_b,
                section: p.section,
                flags: p.flags,
                surfaces: surf_start..surface_idx.len() as u32,
                sheets: 0..0,
                centre,
                radius: (hi - lo).length() * 0.5,
                cull_position: p.transform.w_axis.truncate(),
                cull_radius: p.cull_radius,
                landings,
                apexes,
            });
            sheet_ranges.push(sheet_start..sheet_idx.len() as u32);
        }
    }
    let base = surface_idx.len() as u32;
    for (r, s) in mesh.records.iter_mut().zip(sheet_ranges) {
        r.sheets = s.start + base..s.end + base;
    }
    surface_idx.extend(sheet_idx);
    mesh.indices = surface_idx;
    mesh
}

/// The frame's view of the water.
pub struct View<'a> {
    pub camera: Camera,
    pub aspect: f32,
    /// Reversed-Z depth (`qol::reversed_z`).
    pub reversed_z: bool,
    /// The race runs reversed (the scenery cull's 0x04 / 0x08 flags).
    pub race_reversed: bool,
    /// The drawn sections (None: all).
    pub sections: Option<&'a [u32]>,
    /// The main view draws LOD B (`WorldRenderer::main_lod_b`).
    pub lod_b: bool,
    /// The world clock (0x7FB718), s.
    pub clock: f32,
    /// How far the frame lies behind the newest tick (s), taken
    /// off the clock: (1 − α) · the tick's dt between two ticks, 0 on a
    /// tick's own frame (was `Water::lag`, set from the draw).
    pub lag: f32,
    /// world.efx's fog (value, colour), None when Fog is off.
    pub fog: Option<([f32; 3], [f32; 3])>,
    pub rain: f32,
    /// The light-pack lamps nearest the eye: position, outer, colour, inner.
    pub lamps: Vec<(Vec3, f32, Vec3, f32)>,
    /// The car cube and whether it is on world axes (Enhanced Reflections);
    /// None: the cube is off (black).
    pub env: Option<(&'a wgpu::TextureView, bool)>,
    /// The main view's size need ([`crate::render::main_min_size`]
    /// of the frame's settings).
    pub min_size: f32,
    /// Animated Water's tier (the frame's settings: 2 Realistic, 3
    /// Next-Gen).
    pub tier: u8,
    /// Modern Lighting on (the frame's settings): the Next-Gen mist's
    /// lights.
    pub modern: bool,
}

/// Which records a view draws: in a drawn section, not hidden or culled by
/// the main view's flags, big enough for the scenery cull (0x5D64E0 with
/// the main view's size need), inside the view's side planes.
pub fn pick(mesh: &Mesh, view: &View, hidden: &dyn Fn(usize) -> bool) -> Vec<usize> {
    let sections: Option<Vec<bool>> = view.sections.map(|list| {
        let n = list.iter().copied().max().map_or(0, |m| m as usize + 1);
        let mut v = vec![false; n];
        for &s in list {
            v[s as usize] = true;
        }
        v
    });
    let cam = &view.camera;
    let eye = cam.position;
    let forward = cam.forward();
    let scale = crate::render::cull_scale(cam.game_fov.unwrap_or(cam.fov_y));
    let need = view.min_size;
    let planes = crate::render::side_planes(cam.view_projection(view.aspect));
    let mut out = Vec::new();
    for (i, r) in mesh.records.iter().enumerate() {
        if r.lod_b != view.lod_b {
            continue;
        }
        if let Some(v) = &sections
            && !v.get(r.section as usize).copied().unwrap_or(false)
        {
            continue;
        }
        if hidden(r.placement) || crate::render::WorldView::Main.culls(r.flags, view.race_reversed) {
            continue;
        }
        if r.cull_radius > 0.0
            && crate::render::scenery_cull_size(r.cull_position, r.cull_radius, eye, forward, scale) < need
        {
            continue;
        }
        if crate::render::outside(&planes, r.centre, r.radius) {
            continue;
        }
        out.push(i);
    }
    out
}

/// The landings of the picked records nearest the eye (within `reach`).
/// The apexes of the picked records nearest the eye (within `reach`).
pub fn nearest_apexes(mesh: &Mesh, picked: &[usize], eye: Vec3, reach: f32) -> Vec<(Apex, u32)> {
    let mut v: Vec<(f32, Apex, u32)> = Vec::new();
    for &i in picked {
        for (k, a) in mesh.records[i].apexes.iter().enumerate() {
            let d = a.position.distance(eye);
            if d < reach {
                v.push((d, *a, (i as u32).wrapping_mul(16).wrapping_add(k as u32)));
            }
        }
    }
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v.into_iter().take(MAX_APEXES).map(|(_, a, id)| (a, id)).collect()
}

pub fn nearest_landings(mesh: &Mesh, picked: &[usize], eye: Vec3, reach: f32) -> Vec<(Landing, u32)> {
    let mut v: Vec<(f32, Landing, u32)> = Vec::new();
    for &i in picked {
        for (k, l) in mesh.records[i].landings.iter().enumerate() {
            let d = l.position.distance(eye);
            if d < reach {
                v.push((d, *l, (i as u32).wrapping_mul(64).wrapping_add(k as u32)));
            }
        }
    }
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v.into_iter().take(MAX_LANDINGS).map(|(_, l, id)| (l, id)).collect()
}

/// Ranges of `indices` merged where they touch.
fn merged(ranges: impl Iterator<Item = Range<u32>>) -> Vec<Range<u32>> {
    let mut out: Vec<Range<u32>> = Vec::new();
    for r in ranges.filter(|r| !r.is_empty()) {
        match out.last_mut() {
            Some(last) if last.end == r.start => last.end = r.end,
            _ => out.push(r),
        }
    }
    out
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    eye: [f32; 4],
    forward: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    fog_value: [f32; 4],
    fog_color: [f32; 4],
    anim: [f32; 4],
    counts: [u32; 4],
    lamps: [[f32; 4]; 2 * MAX_LAMPS],
    scroll: [[f32; 4]; MAX_LAYERS],
    landings: [[f32; 4]; MAX_LANDINGS],
    landing_color: [[f32; 4]; MAX_LANDINGS],
    /// x = apexes drawn.
    counts2: [u32; 4],
    /// Per apex: camera-relative position + 1, the jet's velocity + id,
    /// colour.
    apexes: [[f32; 4]; 3 * MAX_APEXES],
}

/// Box-filtered resample of RGBA8 to `TEX`².
fn resample(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((TEX * TEX * 4) as usize);
    for y in 0..TEX {
        for x in 0..TEX {
            let sx = (x as f32 + 0.5) / TEX as f32 * w as f32;
            let sy = (y as f32 + 0.5) / TEX as f32 * h as f32;
            let (x0, y0) = ((sx - 0.5).floor(), (sy - 0.5).floor());
            let (fx, fy) = (sx - 0.5 - x0, sy - 0.5 - y0);
            let at = |xx: f32, yy: f32, c: usize| {
                let xi = (xx as i64).rem_euclid(w as i64) as usize;
                let yi = (yy as i64).rem_euclid(h as i64) as usize;
                rgba[(yi * w as usize + xi) * 4 + c] as f32
            };
            for c in 0..4 {
                let v = at(x0, y0, c) * (1.0 - fx) * (1.0 - fy)
                    + at(x0 + 1.0, y0, c) * fx * (1.0 - fy)
                    + at(x0, y0 + 1.0, c) * (1.0 - fx) * fy
                    + at(x0 + 1.0, y0 + 1.0, c) * fx * fy;
                out.push(v.round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    out
}

/// The next mip level (2 × 2 box).
fn half(level: &[u8], size: u32) -> Vec<u8> {
    let n = size / 2;
    let mut out = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            for c in 0..4 {
                let s = |xx: u32, yy: u32| level[((yy * size + xx) * 4) as usize + c] as u32;
                out.push(
                    ((s(2 * x, 2 * y) + s(2 * x + 1, 2 * y) + s(2 * x, 2 * y + 1) + s(2 * x + 1, 2 * y + 1) + 2) / 4)
                        as u8,
                );
            }
        }
    }
    out
}

/// The texture array's layers: the flip-book's frames, then [`SINGLES`].
struct Layers {
    /// RGBA8 at `TEX`², per layer.
    data: Vec<Vec<u8>>,
    /// Per layer: scroll kind, speed u, speed v, step.
    scroll: Vec<[f32; 4]>,
    fps: f32,
    frames: u32,
}

fn layers(world: &World) -> Layers {
    let mut data = Vec::new();
    let mut scroll = Vec::new();
    let grey = || {
        let mut v = vec![128u8; (TEX * TEX * 4) as usize];
        v.chunks_exact_mut(4).for_each(|c| c[3] = 255);
        v
    };
    let mut push = |hash: Option<u32>| match hash.and_then(|h| world.textures.get(&h)) {
        Some(t) => {
            data.push(resample(&t.decode_rgba(), t.width.max(1), t.height.max(1)));
            scroll.push([t.scroll.kind as f32, t.scroll.speed[0], t.scroll.speed[1], t.scroll.step]);
        }
        None => {
            data.push(grey());
            scroll.push([0.0; 4]);
        }
    };
    let base = crate::formats::hash::bin_hash(FLIPBOOK);
    let (fps, frames) = match world.texture_animations.get(&base) {
        Some(a) if !a.frames.is_empty() => {
            for &h in a.frames.iter().take(MAX_LAYERS - SINGLES.len()) {
                push(Some(h));
            }
            (a.fps as f32, a.frames.len().min(MAX_LAYERS - SINGLES.len()) as u32)
        }
        _ => {
            push(Some(base));
            (0.0, 1)
        }
    };
    for name in SINGLES {
        push(Some(crate::formats::hash::bin_hash(name)));
    }
    Layers { data, scroll, fps, frames }
}

/// The time the water moves by: the world clock (0x7FB718), less how far
/// an uncapped frame lies behind the newest update; 0 for the plain check.
/// Nothing else (no wall clock, no frame count) enters the animation.
pub fn frame_clock(clock: f32, lag: f32, plain: bool) -> f32 {
    if plain { 0.0 } else { clock - lag }
}

/// The GPU half.
struct GpuWater {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    frame: wgpu::Buffer,
    texture: wgpu::TextureView,
    sampler: wgpu::Sampler,
    env_sampler: wgpu::Sampler,
    black_cube: wgpu::TextureView,
    layout: wgpu::BindGroupLayout,
    depth_layouts: [wgpu::BindGroupLayout; 2],
    shader: wgpu::ShaderModule,
    shader_ms: wgpu::ShaderModule,
    pipelines: std::sync::Mutex<HashMap<(wgpu::TextureFormat, u32, bool, u8), wgpu::RenderPipeline>>,
    scroll: Vec<[f32; 4]>,
    fps: f32,
    frames: u32,
}

impl GpuWater {
    fn new(gpu: &Gpu, mesh: &Mesh, world: &World) -> GpuWater {
        use wgpu::util::DeviceExt;
        let device = &gpu.device;
        let empty = [Vertex::default()];
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("water vertices"),
            contents: bytemuck::cast_slice(if mesh.vertices.is_empty() { &empty[..] } else { &mesh.vertices }),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("water indices"),
            contents: bytemuck::cast_slice(if mesh.indices.is_empty() { &[0u32; 3][..] } else { &mesh.indices }),
            usage: wgpu::BufferUsages::INDEX,
        });
        let frame = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("water frame"),
                size: std::mem::size_of::<FrameUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let l = layers(world);
        let mips = TEX.ilog2() + 1;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("water textures"),
            size: wgpu::Extent3d { width: TEX, height: TEX, depth_or_array_layers: l.data.len() as u32 },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (layer, top) in l.data.iter().enumerate() {
            let mut level = top.clone();
            let mut size = TEX;
            for mip in 0..mips {
                gpu.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: mip,
                        origin: wgpu::Origin3d { x: 0, y: 0, z: layer as u32 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &level,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(size * 4),
                        rows_per_image: Some(size),
                    },
                    wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
                );
                if size > 1 {
                    level = half(&level, size);
                    size /= 2;
                }
            }
        }
        let texture = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let env_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water cube"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let black = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("water black cube"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 6 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &black,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[0u8; 24],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 6 },
        );
        let black_cube = black.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let vf = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        let fs = wgpu::ShaderStages::FRAGMENT;
        let layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("water"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: vf,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: fs,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: fs,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: fs,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: fs,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let depth_layout = |multisampled| {
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("water spray depth"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: fs,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled,
                    },
                    count: None,
                }],
            })
        };
        let source =
            shader_src("water.wgsl", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water.wgsl")));
        let module = |label, src: String| {
            crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        let shader_ms = module("water ms", source.replace("texture_depth_2d", "texture_depth_multisampled_2d"));
        let shader = module("water", source.to_string());
        GpuWater {
            vertices,
            indices,
            frame,
            texture,
            sampler,
            env_sampler,
            black_cube,
            layout,
            depth_layouts: [depth_layout(false), depth_layout(true)],
            shader,
            shader_ms,
            pipelines: Default::default(),
            scroll: l.scroll,
            fps: l.fps,
            frames: l.frames,
        }
    }

    /// `which`: 0 surfaces, 1 sheets, 2 spray.
    fn pipeline(
        &self,
        gpu: &Gpu,
        format: wgpu::TextureFormat,
        samples: u32,
        reversed: bool,
        which: u8,
        ms_depth: bool,
    ) -> wgpu::RenderPipeline {
        let key = (format, samples, reversed || (which == 2 && ms_depth), which);
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(p) = pipelines.get(&key) {
            return p.clone();
        }
        let groups: Vec<Option<&wgpu::BindGroupLayout>> = if which == 2 {
            vec![Some(&self.layout), Some(&self.depth_layouts[usize::from(ms_depth)])]
        } else {
            vec![Some(&self.layout)]
        };
        let layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("water"),
            bind_group_layouts: &groups,
            immediate_size: 0,
        });
        let module = if which == 2 && ms_depth { &self.shader_ms } else { &self.shader };
        let attributes =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x2, 3 => Float32x2, 4 => Float32x4];
        let buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        })];
        // The native alpha blend (SRCALPHA / INVSRCALPHA, both channels)
        // for the sheets; premultiplied over for the spray; none for the
        // surfaces (their textures are opaque, Blend::None).
        let alpha = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let premultiplied = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            ..alpha
        };
        let (vs, fs, blend) = match which {
            0 => ("vs_water", "fs_surface", None),
            1 => ("vs_water", "fs_sheet", Some(alpha)),
            _ => ("vs_spray", "fs_spray", Some(premultiplied)),
        };
        let depth_stencil = (which != 2).then(|| wgpu::DepthStencilState {
            format: crate::render::DEPTH_FORMAT,
            // As world.efx: LESSEQUAL (GREATEREQUAL reversed), no write
            // when blended.
            depth_write_enabled: Some(which == 0),
            depth_compare: Some(if reversed {
                wgpu::CompareFunction::GreaterEqual
            } else {
                wgpu::CompareFunction::LessEqual
            }),
            stencil: Default::default(),
            bias: Default::default(),
        });
        let pipeline = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some(fs),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(vs),
                compilation_options: Default::default(),
                buffers: if which == 2 { &[] } else { &buffers },
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil,
            multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
            multiview_mask: None,
            cache: gpu.disk_pipeline_cache(),
        });
        pipelines.insert(key, pipeline.clone());
        pipeline
    }

    fn group(&self, gpu: &Gpu, env: Option<&wgpu::TextureView>) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.frame.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.texture) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(env.unwrap_or(&self.black_cube)),
                },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.env_sampler) },
            ],
        })
    }
}

/// The frame's draw list ([`Water::prepare`]).
#[derive(Default)]
struct Prepared {
    surfaces: Vec<Range<u32>>,
    sheets: Vec<Range<u32>>,
    emitters: u32,
    apexes: u32,
    reversed: bool,
    group: Option<wgpu::BindGroup>,
    /// Spray billboards per landing (On: [`PER_EMITTER`]).
    per_emitter: u32,
    /// The Realistic tier's frame (None: On's drawing).
    hq: Option<crate::render::water_hq::Frame>,
    /// The Next-Gen tier draws this frame (`Water::ng`'s frame).
    ng: bool,
}

/// Animated Water's state, per drive.
#[derive(Default)]
pub struct Water {
    /// (placements, solids) of the world the mesh was built for.
    built_for: Option<(usize, usize)>,
    mesh: Mesh,
    gpu: Option<GpuWater>,
    /// Realistic: its fields and GPU half, made at its first use.
    hq: Option<crate::render::water_hq::Hq>,
    /// Next-Gen: its jets, basins and mist, made at its first use.
    ng: Option<crate::render::water_ng::Ng>,
    prepared: Prepared,
    /// The eye and the records the Next-Gen tier decided on
    /// at the last world tick (`render::enh::tick_views`).
    tick_pick: Option<(Vec3, Vec<usize>, Vec<(Landing, u32)>)>,
}

impl Water {
    /// Whether this frame draws anything ([`Water::prepare`] picked some).
    pub fn active(&self) -> bool {
        !self.prepared.surfaces.is_empty()
            || !self.prepared.sheets.is_empty()
            || self.prepared.emitters > 0
            || self.prepared.apexes > 0
    }

    /// The Next-Gen tier's tick-stepped state (its basins'
    /// schedule) as a digest; 0 without the tier.
    pub fn tick_digest(&self) -> u64 {
        use std::hash::Hasher;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        if let Some(n) = &self.ng {
            n.tick_digest(&mut h);
        }
        h.finish()
    }

    /// What a new drive on the same world starts with when this drive's
    /// water is kept across a car change: the world's mesh and the GPU half
    /// made from it (its buffers, the layers' textures and its pipelines;
    /// nothing in them changes while driving: the frame's uniform is
    /// written by each [`Water::prepare`] before the draw). The frame's
    /// draw list and the Next-Gen tier's tick pick start empty, and the
    /// Realistic and Next-Gen halves (their fields step with the drive)
    /// are dropped: [`Water::build`] makes them again as for a new drive.
    pub fn for_new_drive(self) -> Water {
        Water { built_for: self.built_for, mesh: self.mesh, gpu: self.gpu, ..Default::default() }
    }

    /// A digest of what [`Water::for_new_drive`] keeps (the mesh, the GPU
    /// half's layer table) and of the per-drive state it clears, for the
    /// test that a kept water is a new one.
    pub fn new_drive_digest(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.built_for.hash(&mut h);
        bytemuck::cast_slice::<_, u8>(&self.mesh.vertices).hash(&mut h);
        self.mesh.indices.hash(&mut h);
        if let Some(g) = &self.gpu {
            for s in &g.scroll {
                s.map(f32::to_bits).hash(&mut h);
            }
            (g.fps.to_bits(), g.frames).hash(&mut h);
        }
        (self.hq.is_some(), self.ng.is_some(), self.active(), self.tick_pick.is_some()).hash(&mut h);
        h.finish()
    }

    /// The world's water mesh (built on the first use).
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }

    /// Pick the frame's water and write its uniform. Call each frame while
    /// the row is on, before [`Water::draw_surfaces`] / [`Water::draw_spray`].
    pub fn prepare(&mut self, gpu: &Gpu, world: &World, hidden: &dyn Fn(usize) -> bool, view: &View) {
        self.build(gpu, world, 1);
        let g = self.gpu.as_ref().unwrap();
        let picked = pick(&self.mesh, view, hidden);
        let mesh = &self.mesh;
        let surfaces = merged(picked.iter().map(|&i| mesh.records[i].surfaces.clone()));
        let sheets = merged(picked.iter().map(|&i| mesh.records[i].sheets.clone()));
        let cam = &view.camera;
        let eye = cam.position;
        let landings = nearest_landings(mesh, &picked, eye, SPRAY_REACH);
        // NFSU2_WATER_PLAIN=1 (a check aid): the native shading alone, at
        // the shots' native time 0, no spray: against Off it shows the copy.
        // (Read once: a getenv per frame takes the environment lock.)
        let plain = {
            static PLAIN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            *PLAIN.get_or_init(|| crate::render::debug_env::nfsu2_water_plain())
        };
        let clock = frame_clock(view.clock, view.lag, plain);
        let rot = Mat4::look_to_rh(Vec3::ZERO, cam.forward(), cam.up);
        let view_proj = cam.projection_depth(view.aspect, view.reversed_z) * rot;
        let (fwd, right) = (cam.forward(), cam.forward().cross(cam.up).normalize_or(Vec3::X));
        let up = right.cross(fwd);
        let (fog_value, fog_color, fogged) = match view.fog {
            Some((v, c)) => (v, c, 1.0),
            None => ([1.0e9, 0.0, 1.0], [0.0; 3], 0.0),
        };
        let env_mode = match view.env {
            None => 0.0,
            Some((_, true)) => 2.0,
            Some((_, false)) => 1.0,
        };
        let mut u = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            view: cam.view().to_cols_array_2d(),
            eye: eye.extend(clock).to_array(),
            forward: fwd.extend(if view.reversed_z { 1.0 } else { 0.0 }).to_array(),
            right: right.extend(cam.near).to_array(),
            up: up.extend(cam.far).to_array(),
            fog_value: [fog_value[0], fog_value[1], fog_value[2], fogged],
            fog_color: [fog_color[0], fog_color[1], fog_color[2], env_mode],
            anim: [g.fps, g.frames as f32, view.rain, if plain { 1.0 } else { 0.0 }],
            counts: [0, 0, 0, PER_EMITTER],
            lamps: [[0.0; 4]; 2 * MAX_LAMPS],
            scroll: [[0.0; 4]; MAX_LAYERS],
            landings: [[0.0; 4]; MAX_LANDINGS],
            landing_color: [[0.0; 4]; MAX_LANDINGS],
            counts2: [0; 4],
            apexes: [[0.0; 4]; 3 * MAX_APEXES],
        };
        for (i, s) in g.scroll.iter().take(MAX_LAYERS).enumerate() {
            u.scroll[i] = *s;
        }
        let mut n = 0;
        for (p, outer, c, inner) in view.lamps.iter().take(MAX_LAMPS) {
            u.lamps[n * 2] = (*p - eye).extend(*outer).to_array();
            u.lamps[n * 2 + 1] = c.extend(*inner).to_array();
            n += 1;
        }
        u.counts[0] = n as u32;
        for (i, (l, id)) in landings.iter().enumerate() {
            u.landings[i] = (l.position - eye).extend(l.strength).to_array();
            u.landing_color[i] = l.color.extend((*id % 4096) as f32).to_array();
        }
        u.counts[1] = if plain { 0 } else { landings.len() as u32 };
        u.counts[2] = if plain { 0 } else { u.counts[1] };
        let apexes = if plain { Vec::new() } else { nearest_apexes(mesh, &picked, eye, SPRAY_REACH) };
        for (i, (a, id)) in apexes.iter().enumerate() {
            u.apexes[i * 3] = (a.position - eye).extend(1.0).to_array();
            u.apexes[i * 3 + 1] = a.velocity.extend((*id % 4096) as f32).to_array();
            u.apexes[i * 3 + 2] = a.color.extend(0.0).to_array();
        }
        u.counts2[0] = apexes.len() as u32;
        // Realistic, docs/water.md: its bodies and more spray; On's
        // uniform is otherwise the same.
        let hq = (view.tier >= 2 && !plain).then(|| {
            let h = self.hq.get_or_insert_with(|| crate::render::water_hq::Hq::new(gpu, &self.mesh));
            let proj = cam.projection_depth(view.aspect, view.reversed_z);
            let lum = fog_color[0] * 0.3 + fog_color[1] * 0.59 + fog_color[2] * 0.11;
            u.counts[3] = crate::render::water_hq::PER_EMITTER;
            crate::render::water_hq::Frame {
                bodies: h.bodies(picked.iter().map(|&i| self.mesh.records[i].placement), eye),
                eye,
                tan: glam::Vec2::new(1.0 / proj.x_axis.x, 1.0 / proj.y_axis.y),
                daylight: if view.fog.is_some() { (lum * 2.5).clamp(0.0, 1.0) } else { 0.5 },
                rain: view.rain,
            }
        });
        // Next-Gen, docs/water.md: the jets as particles, the basins
        // simulated, the mist volumetric; On's spray billboards are off.
        let ng = hq.is_some() && view.tier >= 3;
        if ng {
            let h = self.hq.as_ref().unwrap();
            // docs/uncapped-fps.md, "Enhancement effects on
            // ticks"): which jets get particles and which basins are
            // simulated (the steps' impulses) is decided at the newest
            // world tick's eye and from its view, and kept by the frames
            // between two ticks, so the basins' waves at tick n are the
            // same at every display rate (at 60 Hz, the frame's own).
            let frame = crate::render::enh::tick_views::frame();
            match frame.as_ref().map(|f| f.views.last()) {
                // No published ticks (shots): the frame's own pick.
                None => self.tick_pick = None,
                // The tick drawn at α = 1: the frame's own pick.
                Some(Some(v)) if v.is_frame(cam, view.sections, v.far.as_deref()) => {
                    self.tick_pick = Some((eye, picked.clone(), landings.clone()));
                }
                Some(Some(v)) => {
                    let camera = Camera { jitter: cam.jitter, ..v.camera };
                    let tv = View {
                        camera,
                        aspect: view.aspect,
                        reversed_z: view.reversed_z,
                        race_reversed: view.race_reversed,
                        sections: v.sections.as_deref(),
                        lod_b: view.lod_b,
                        clock: v.clock,
                        lag: 0.0,
                        fog: None,
                        rain: view.rain,
                        lamps: Vec::new(),
                        env: None,
                        min_size: view.min_size,
                        tier: view.tier,
                        modern: view.modern,
                    };
                    let p = pick(&self.mesh, &tv, hidden);
                    let l = nearest_landings(&self.mesh, &p, camera.position, SPRAY_REACH);
                    self.tick_pick = Some((camera.position, p, l));
                }
                // A frame between two ticks: the last tick's (none yet:
                // the frame's own).
                Some(None) => {}
            }
            let (decide_eye, decide_picked, decide_landings): (Vec3, &[usize], &[(Landing, u32)]) =
                match self.tick_pick.as_ref() {
                    Some((e, p, l)) => (*e, p, l),
                    None => (eye, &picked, &landings),
                };
            let n = self.ng.get_or_insert_with(|| crate::render::water_ng::Ng::new(gpu, &self.mesh, h, g.frames));
            n.prepare(
                gpu,
                &self.mesh,
                h,
                &picked,
                (decide_eye, decide_picked, decide_landings),
                cam,
                view_proj,
                view.reversed_z,
                clock,
                view.clock,
                view.rain,
                &landings,
                &view.lamps,
                g.frames,
                view.modern,
            );
            u.counts[2] = 0;
            u.counts2[0] = 0;
        }
        gpu.queue.write_buffer(&g.frame, 0, bytemuck::bytes_of(&u));
        self.prepared = Prepared {
            surfaces,
            sheets,
            emitters: u.counts[2],
            apexes: u.counts2[0],
            reversed: view.reversed_z,
            group: Some(g.group(gpu, view.env.map(|(v, _)| v))),
            per_emitter: u.counts[3],
            hq,
            ng,
        };
    }

    /// What the first [`Water::prepare`] at `tier` (1 On, 2 Realistic, 3
    /// Next-Gen) makes, made now (docs/first-use-builds.md: a drive's load
    /// and a tier switched on): the world's water mesh (once per world),
    /// the GPU half and the tiers' halves up to `tier`, with their shader
    /// modules and the pipelines they make in their constructors. Nothing
    /// is stepped and no uniform written; the frames then find them made.
    pub fn build(&mut self, gpu: &Gpu, world: &World, tier: u8) {
        let key = (world.placements.len(), world.solids.len());
        if self.built_for != Some(key) {
            let _span = crate::prof::span("water: build");
            self.mesh = build(world);
            self.gpu = None;
            self.hq = None;
            self.ng = None;
            self.built_for = Some(key);
        }
        if self.gpu.is_none() {
            self.gpu = Some(GpuWater::new(gpu, &self.mesh, world));
        }
        if tier >= 2 && self.hq.is_none() {
            self.hq = Some(crate::render::water_hq::Hq::new(gpu, &self.mesh));
        }
        if tier >= 3 && self.ng.is_none() {
            let frames = self.gpu.as_ref().unwrap().frames;
            let h = self.hq.as_ref().unwrap();
            self.ng = Some(crate::render::water_ng::Ng::new(gpu, &self.mesh, h, frames));
        }
    }

    /// The pipelines a drive's frames draw the water with at `tier` into a
    /// scene of `format` with `samples` (`reversed`: reversed depth), made
    /// now, after [`Water::build`]: the surfaces and sheets, the spray
    /// soft against the scene's depth and, after the temporal pass, against
    /// its single-sampled copy; Realistic's wet stone, scene copy, surfaces
    /// and sheets; Next-Gen's basins, lips, sheets, drops and mist. The
    /// objects are the ones the frames make (the device's cache keeps
    /// them), so nothing drawn changes.
    pub fn warm_tier(&self, gpu: &Gpu, tier: u8, format: wgpu::TextureFormat, samples: u32, reversed: bool) {
        let Some(g) = self.gpu.as_ref() else { return };
        for which in [0, 1] {
            g.pipeline(gpu, format, samples, reversed, which, false);
        }
        for s in [samples, 1] {
            g.pipeline(gpu, format, s, false, 2, s > 1);
        }
        let Some(h) = self.hq.as_ref().filter(|_| tier >= 2) else { return };
        let group = g.group(gpu, None);
        let base = crate::render::water_hq::Base {
            layout0: &g.layout,
            depth_layouts: &g.depth_layouts,
            group0: &group,
            vertices: &g.vertices,
            indices: &g.indices,
            surfaces: &[],
            sheets: &[],
            reversed,
            water_tex: &g.texture,
        };
        h.warm(gpu, &base, format, samples);
        if let Some(n) = self.ng.as_ref().filter(|_| tier >= 3) {
            n.warm(gpu, h, &base, format, samples);
        }
    }

    /// The surfaces, then the sheets, into the scene (`target`, `depth`:
    /// the main view's, `samples` each), after the world and the cars.
    /// The pipelines a [`Water::draw_surfaces`] into `format`
    /// (`samples`) and a [`Water::draw_spray`] against a depth of
    /// `depth_samples` use, built now (once the water's GPU half exists),
    /// so the first frame with water in view is not a pipeline build.
    /// Nothing drawn changes.
    pub fn warm(&self, gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, depth_samples: u32, surfaces: bool) {
        let Some(g) = self.gpu.as_ref() else { return };
        if surfaces {
            for which in [0, 1] {
                g.pipeline(gpu, format, samples, self.prepared.reversed, which, false);
            }
        }
        g.pipeline(gpu, format, samples, false, 2, depth_samples > 1);
    }

    /// `size`: the target's size (px; the Realistic tier's copy of it).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_surfaces(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        let (Some(g), Some(group)) = (self.gpu.as_ref(), self.prepared.group.as_ref()) else { return };
        if self.prepared.surfaces.is_empty()
            && self.prepared.sheets.is_empty()
            && self.prepared.hq.as_ref().is_none_or(|h| h.bodies.is_empty())
        {
            return;
        }
        // The Realistic tier draws them its own way.
        if let (Some(frame), Some(hq)) = (self.prepared.hq.as_ref(), self.hq.as_mut()) {
            let base = crate::render::water_hq::Base {
                layout0: &g.layout,
                depth_layouts: &g.depth_layouts,
                group0: group,
                vertices: &g.vertices,
                indices: &g.indices,
                surfaces: &self.prepared.surfaces,
                sheets: &self.prepared.sheets,
                reversed: self.prepared.reversed,
                water_tex: &g.texture,
            };
            let ng = if self.prepared.ng { self.ng.as_mut() } else { None };
            hq.draw(gpu, encoder, &base, frame, target, format, samples, depth, size, ng);
            if let (true, Some(n)) = (self.prepared.ng, self.ng.as_mut()) {
                n.build_mist(gpu, encoder);
            }
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("water"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, group, &[]);
        pass.set_vertex_buffer(0, g.vertices.slice(..));
        pass.set_index_buffer(g.indices.slice(..), wgpu::IndexFormat::Uint32);
        for (which, list) in [(0u8, &self.prepared.surfaces), (1, &self.prepared.sheets)] {
            if list.is_empty() {
                continue;
            }
            pass.set_pipeline(&g.pipeline(gpu, format, samples, self.prepared.reversed, which, false));
            for r in list {
                pass.draw_indexed(r.clone(), 0, 0..1);
            }
        }
        drop(pass);
        crate::prof::gpu::mark(encoder, "graphics: water");
    }

    /// The spray and mist, soft against `depth` (`depth_samples`), with the
    /// temporal pass's viewport when given (as the GPU smoke's puffs).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_spray(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        samples: u32,
        depth: &wgpu::TextureView,
        depth_samples: u32,
        viewport: Option<[f32; 4]>,
    ) {
        let (Some(g), Some(group)) = (self.gpu.as_ref(), self.prepared.group.as_ref()) else { return };
        // The Next-Gen tier's mist (volumetric) instead of the billboards.
        if let (true, Some(n)) = (self.prepared.ng, self.ng.as_ref()) {
            n.draw_mist(gpu, encoder, target, format, samples, depth, depth_samples, viewport);
            return;
        }
        if self.prepared.emitters == 0 && self.prepared.apexes == 0 {
            return;
        }
        let ms = depth_samples > 1;
        let pipeline = g.pipeline(gpu, format, samples, false, 2, ms);
        let depth_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water spray depth"),
            layout: &g.depth_layouts[usize::from(ms)],
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(depth) }],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("water spray"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Some([x, y, w, h]) = viewport {
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
        }
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.set_bind_group(1, &depth_group, &[]);
        pass.draw(0..6, 0..self.prepared.emitters * self.prepared.per_emitter + self.prepared.apexes * PER_APEX);
        drop(pass);
        crate::prof::gpu::mark(encoder, "graphics: water spray");
    }
}

/// A water shader's source: the built-in one, or (a tuning aid, not an
/// option) the file of that name in `NFSU2_WATER_SRC_DIR` when it is set.
pub(crate) fn shader_src(name: &str, builtin: &'static str) -> std::borrow::Cow<'static, str> {
    match crate::render::debug_env::nfsu2_water_src_dir()
        .and_then(|d| std::fs::read_to_string(std::path::Path::new(&d).join(name)).ok())
    {
        Some(s) => s.into(),
        None => builtin.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without NFSU2_WATER_SRC_DIR (a tuning aid) the built-in
    /// source is used; a directory without the file falls back to it too.
    #[test]
    fn shader_source_is_built_in_by_default() {
        if crate::render::debug_env::nfsu2_water_src_dir().is_none() {
            assert!(std::ptr::eq(
                shader_src("water.wgsl", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water.wgsl")))
                    .as_ref(),
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water.wgsl"))
            ));
        }
        assert_eq!(shader_src("no-such-file.wgsl", "x"), "x");
    }

    #[test]
    fn water_textures_by_name() {
        assert_eq!(kind_of("ANM_WATERA_"), Some(Kind::Basin));
        assert_eq!(kind_of("ANM_WATERA_007"), Some(Kind::Basin));
        assert_eq!(kind_of("ARC_POOL_A_DM"), Some(Kind::Pool));
        assert_eq!(kind_of("TRN_WATER_SURFACE_A_LL"), Some(Kind::Stream));
        assert_eq!(kind_of("TRN_WATERFALL_A"), Some(Kind::Sheet));
        assert_eq!(kind_of("TRN_WATERFALL_A_LL"), Some(Kind::SheetFar));
        // Left native: the panorama's ocean, LODs, the marker rings, signs.
        for n in [
            "TRN_OCEANA_DM",
            "LOD_HARBORA_1Z_LL",
            "XE_RINGWAVEA",
            "SGN_PALMRIVER",
            "ANM_BILLBOARDLITES_A_LL",
            "WATERDROPLET",
        ] {
            assert_eq!(kind_of(n), None, "{n}");
        }
    }

    /// The animation runs on the world clock only: the same clock gives the
    /// same time (shots are deterministic), frames between updates step
    /// back by their lag, the plain check is the shots' native time 0.
    #[test]
    fn animation_time_is_the_world_clock() {
        assert_eq!(frame_clock(170.0, 0.0, false), 170.0);
        assert_eq!(frame_clock(170.0, 0.0, false).to_bits(), frame_clock(170.0, 0.0, false).to_bits());
        assert!((frame_clock(170.0, 1.0 / 120.0, false) - (170.0 - 1.0 / 120.0)).abs() < 1e-6);
        assert_eq!(frame_clock(170.0, 0.004, true), 0.0);
        let w = Water::default();
        assert!(!w.active(), "nothing drawn before a prepare");
    }

    /// A jet that rises and falls along v is an arc (flow by v, toward the
    /// end the native scroll carries the texture to); a fall or a cascade
    /// whose source is its top is not.
    #[test]
    fn arcs_and_falls() {
        // An arc 20 m long, 5 m high, v along it.
        let arc: Vec<(Vec3, f32)> = (0..=20)
            .map(|i| {
                let x = i as f32;
                (Vec3::new(x, 0.0, 5.0 * (1.0 - ((x - 10.0) / 10.0).powi(2))), x / 20.0)
            })
            .collect();
        let shape = sheet_shape(&arc, 0.9375);
        assert_eq!(shape, SheetShape::Arc { vlo: 0.0, vhi: 1.0, forward: true });
        // Speed +v moves the texture toward -v: the landing (0) at v = 0.
        assert_eq!(shape.along(0.0, 0.0), 0.0);
        assert_eq!(shape.along(0.0, 1.0), 1.0);
        let flow: Vec<(Vec3, f32)> = arc.iter().map(|&(p, v)| (p, shape.along(p.z, v))).collect();
        let (apex, jet) = arc_apex(&flow).unwrap();
        assert!((apex.x - 10.0).abs() < 1.0 && apex.z > 4.8, "{apex:?}");
        assert!(jet.x < -1.0 && jet.y.abs() < 1e-3 && jet.z == 0.0, "toward the landing: {jet:?}");
        assert_eq!(sheet_shape(&arc, -0.5), SheetShape::Arc { vlo: 0.0, vhi: 1.0, forward: false });
        // A fall: 1 m out, 6 m down, v up the sheet.
        let fall: Vec<(Vec3, f32)> =
            (0..=6).map(|i| (Vec3::new(i as f32 / 6.0, 0.0, 6.0 - i as f32), 1.0 - i as f32 / 6.0)).collect();
        assert!(matches!(sheet_shape(&fall, 0.9375), SheetShape::Fall { .. }));
        // A lip falling over an edge (source = its top): long but not an arc.
        let lip: Vec<(Vec3, f32)> = (0..=20)
            .map(|i| {
                let x = i as f32;
                (Vec3::new(x, 0.0, if x < 10.0 { 2.0 } else { 2.0 - (x - 10.0) * 0.2 }), 1.0 - x / 20.0)
            })
            .collect();
        assert!(matches!(sheet_shape(&lip, 0.9375), SheetShape::Fall { .. }));
    }

    #[test]
    fn ranges_merge_where_they_touch() {
        assert_eq!(merged([0..3, 3..9, 12..15, 15..15, 15..18].into_iter()), vec![0..9, 12..18]);
    }

    #[test]
    fn mips_halve_to_one_texel() {
        let top = vec![200u8; (TEX * TEX * 4) as usize];
        let mut level = top;
        let mut size = TEX;
        while size > 1 {
            level = half(&level, size);
            size /= 2;
        }
        assert_eq!(level, vec![200u8; 4]);
        assert_eq!(resample(&[10, 20, 30, 40], 1, 1).len(), (TEX * TEX * 4) as usize);
    }

    fn assets() -> World {
        World::load(&crate::testkit::game_root(), "L4RA").unwrap()
    }

    /// Bayview's water (asset test, ignored without NFSU2_ROOT): the
    /// fountains, waterfalls, pools, canals and streams of docs/water.md,
    /// every triangle of a water texture, with landings for the sheets.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn bayview_water_census() {
        let world = assets();
        let mesh = build(&world);
        let names = |pred: &dyn Fn(&Record) -> bool| -> std::collections::BTreeSet<String> {
            mesh.records
                .iter()
                .filter(|r| pred(r))
                .map(|r| world.solids[world.placements[r.placement].solid].name.clone())
                .collect()
        };
        let with_sheets = names(&|r| !r.sheets.is_empty() && !r.lod_b);
        for n in
            ["XO_FOUNTAINWATC_1A_00", "XO_FOUNTAINWTRA_1A_LL_00", "XO_UC_FOUNTAINWATN_1A_00", "XO_WATERFALLB_1A_00"]
        {
            assert!(with_sheets.contains(n), "{n} has sheets: {with_sheets:?}");
        }
        let with_surfaces = names(&|r| !r.surfaces.is_empty() && !r.lod_b);
        for n in ["XO_FOUNTAINBASEC_1A_00", "XO_ROUNDFOUNTAINA_1A_LL_00", "XB_POOLA_1A_DM_00"] {
            assert!(with_surfaces.contains(n), "{n} has a surface: {with_surfaces:?}");
        }
        // The arcing jets (casino, park, plaza: XO_FOUNTAINWTRA, 28) and
        // only they have an apex, one each.
        for r in mesh.records.iter().filter(|r| !r.lod_b) {
            let n = &world.solids[world.placements[r.placement].solid].name;
            assert_eq!(r.apexes.len(), usize::from(n == "XO_FOUNTAINWTRA_1A_LL_00"), "{n}");
        }
        // Every sheet record lands somewhere; the surfaces don't.
        for r in mesh.records.iter().filter(|r| !r.lod_b) {
            assert_eq!(r.sheets.is_empty(), r.landings.is_empty(), "placement {}", r.placement);
        }
        // The triangles: as many as the census counts (top LOD).
        let tris: usize =
            mesh.records.iter().filter(|r| !r.lod_b).map(|r| (r.surfaces.len() + r.sheets.len()) / 3).sum();
        assert!(tris > 6_000, "{tris} water triangles");
        // The canals on reflective meshes are left native.
        assert!(mesh.records.iter().all(|r| {
            !is_reflective(
                world.solids[if r.lod_b {
                    world.placements[r.placement].mid_solid.unwrap()
                } else {
                    world.placements[r.placement].solid
                }]
                .mesh_flags,
            )
        }));
        // Deterministic: built twice, the same.
        let again = build(&world);
        assert_eq!(again.vertices, mesh.vertices);
        assert_eq!(again.indices, mesh.indices);
    }

    /// The pick follows the native main view: a fountain in front of the
    /// eye is drawn, behind it not, a hidden one not, LOD B only at LOD B.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn pick_follows_the_view() {
        let world = assets();
        let mesh = build(&world);
        // XO_FOUNTAINBASEC at (-785.1, -284.5, 23.5).
        let target = Vec3::new(-785.1, -284.5, 26.0);
        let rec = mesh
            .records
            .iter()
            .position(|r| !r.lod_b && r.centre.distance(target) < 4.0 && !r.surfaces.is_empty())
            .expect("the fountain's basin");
        let view = |eye: Vec3, look: Vec3, lod_b: bool| View {
            camera: Camera::look_at(eye, look, Vec3::Z, 50f32.to_radians()),
            aspect: 4.0 / 3.0,
            reversed_z: false,
            race_reversed: false,
            sections: None,
            lod_b,
            clock: 10.0,
            lag: 0.0,
            fog: None,
            rain: 0.0,
            lamps: Vec::new(),
            env: None,
            min_size: crate::render::main_min_size(&crate::render::RenderOptions::in_force()),
            tier: 1,
            modern: false,
        };
        let eye = Vec3::new(-805.0, -296.0, 31.0);
        let front = view(eye, target, false);
        assert!(pick(&mesh, &front, &|_| false).contains(&rec));
        let behind = view(eye, eye * 2.0 - target, false);
        assert!(!pick(&mesh, &behind, &|_| false).contains(&rec));
        let p = mesh.records[rec].placement;
        assert!(!pick(&mesh, &front, &|i| i == p).contains(&rec));
        assert!(pick(&mesh, &view(eye, target, true), &|_| false).iter().all(|&i| mesh.records[i].lod_b));
        let section = mesh.records[rec].section;
        let other = [section + 1];
        let without = View { sections: Some(&other), ..view(eye, target, false) };
        assert!(!pick(&mesh, &without, &|_| false).contains(&rec));
        let landings = nearest_landings(&mesh, &pick(&mesh, &front, &|_| false), eye, SPRAY_REACH);
        assert!(!landings.is_empty() && landings.len() <= MAX_LANDINGS);
    }
}
