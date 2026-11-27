//! Animated Water, Next-Gen (clone deviation, opt-in; docs/notes/graphics/water.md
//! "Next-Gen"). Nothing here is native (SPEED2.EXE has no water system;
//! docs/notes/graphics/water.md "What the original does").
//!
//! The row's fourth tier, on top of Realistic (`render::water_hq`, whose
//! wet stone, scene copy and surface shading it reuses):
//! * **Jets that break into drops** ([`charts`]): every falling sheet and
//!   arc keeps the native art (Realistic's sheet shading: the art's streaks,
//!   tint and translucency); near the eye thousands of particles move along
//!   its authored path (a chart of the sheet: across × along → world
//!   position, timed by gravity), and the water that breaks away before it
//!   lands, and the drops thrown up where it lands, are drawn as lit drops
//!   (energy-bounded, smeared over a fixed exposure), the sheet thinning
//!   where they leave it.
//! * **Basins with a wave simulation**: a damped wave equation on a
//!   heightfield per near basin, driven by the particles that land in it
//!   and by rain drops; drawn as a generated, displaced grid (normals and
//!   foam from the simulation) with Realistic's reflections, refraction,
//!   absorption and caustics.
//! * **Mist** as volumetric particles (`render::volume_fx`), lit by the
//!   lamps.
//!
//! Everything moves on the world clock, the simulation in fixed 1/60 s
//! steps warmed up over the last seconds when a basin comes into reach, so
//! shots are deterministic.

use crate::render::water::{Kind, Mesh};
use glam::{Vec2, Vec3};

/// A chart's texels across and along the flow.
pub const CHART_U: usize = 16;
pub const CHART_V: usize = 64;
/// Entries of a chart's timing table (along-coordinate at equal times).
pub const TABLE: usize = 32;
pub const GRAVITY: f32 = 9.81;
/// A fall's speed where it leaves its lip (m/s; inferred).
pub const LIP_SPEED: f32 = 1.2;

/// One falling sheet or arc of the native art (a connected part of a
/// record's sheet triangles), as a chart: `texels[a + CHART_U * s]` is the
/// world position at across `a` (0..1 of its u range) and along `s` (0 at
/// its source … 1 where it lands), w = 1 where the art covers it.
#[derive(Clone, Debug)]
pub struct Chart {
    pub record: usize,
    pub placement: usize,
    pub kind: Kind,
    pub arc: bool,
    pub texels: Vec<[f32; 4]>,
    /// The along-coordinate at τ = k / (TABLE − 1) of the fall's time.
    pub table: [f32; TABLE],
    /// Source to landing (s).
    pub life: f32,
    /// The sheet's area (m²) and path length (m).
    pub area: f32,
    pub length: f32,
    pub centre: Vec3,
    pub radius: f32,
    /// The art's uv range: u across, v along (source → landing as `s`).
    pub u_range: Vec2,
    pub v_range: Vec2,
    /// The water's lit colour (2 · texture mean · v0, as the landings').
    pub color: Vec3,
    /// The native colour's alpha (the sheet's opacity, v0.a).
    pub alpha: f32,
}

impl Chart {
    /// The chart's world position at (a, s), bilinear.
    pub fn at(&self, a: f32, s: f32) -> Vec3 {
        let x = (a.clamp(0.0, 1.0) * (CHART_U - 1) as f32).min((CHART_U - 1) as f32 - 1e-3);
        let y = (s.clamp(0.0, 1.0) * (CHART_V - 1) as f32).min((CHART_V - 1) as f32 - 1e-3);
        let (x0, y0) = (x as usize, y as usize);
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let t = |xx: usize, yy: usize| Vec3::from_slice(&self.texels[xx + CHART_U * yy][..3]);
        let a0 = t(x0, y0).lerp(t(x0 + 1, y0), fx);
        let a1 = t(x0, y0 + 1).lerp(t(x0 + 1, y0 + 1), fx);
        a0.lerp(a1, fy)
    }
}

/// Union-find root.
fn root(parent: &mut [u32], mut i: u32) -> u32 {
    while parent[i as usize] != i {
        parent[i as usize] = parent[parent[i as usize] as usize];
        i = parent[i as usize];
    }
    i
}

/// The charts of every top-LOD record's sheets (deterministic: records,
/// components and texels in a fixed order).
pub fn charts(mesh: &Mesh) -> Vec<Chart> {
    let mut out = Vec::new();
    for (ri, r) in mesh.records.iter().enumerate() {
        if r.lod_b || r.sheets.is_empty() {
            continue;
        }
        let idx = &mesh.indices[r.sheets.start as usize..r.sheets.end as usize];
        // Connected parts (shared vertices).
        let mut verts: Vec<u32> = idx.to_vec();
        verts.sort_unstable();
        verts.dedup();
        let local = |v: u32| verts.binary_search(&v).unwrap() as u32;
        let mut parent: Vec<u32> = (0..verts.len() as u32).collect();
        for t in idx.chunks_exact(3) {
            let (a, b, c) = (local(t[0]), local(t[1]), local(t[2]));
            for (x, y) in [(a, b), (b, c)] {
                let (rx, ry) = (root(&mut parent, x), root(&mut parent, y));
                if rx != ry {
                    parent[rx.max(ry) as usize] = rx.min(ry);
                }
            }
        }
        let mut parts: std::collections::BTreeMap<u32, Vec<[u32; 3]>> = Default::default();
        for t in idx.chunks_exact(3) {
            let k = root(&mut parent, local(t[0]));
            parts.entry(k).or_default().push([t[0], t[1], t[2]]);
        }
        for tris in parts.values() {
            if let Some(c) = chart(mesh, ri, tris) {
                out.push(c);
            }
        }
    }
    out
}

/// One connected sheet's chart, None when it is degenerate (no extent in
/// uv, or too small to matter).
fn chart(mesh: &Mesh, record: usize, tris: &[[u32; 3]]) -> Option<Chart> {
    let r = &mesh.records[record];
    let vx = |i: u32| &mesh.vertices[i as usize];
    let (mut ulo, mut uhi, mut vlo, mut vhi) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
    let arc = vx(tris[0][0]).aux[1] < 0.0;
    let kind = if vx(tris[0][0]).kind > 3.5 { Kind::SheetFar } else { Kind::Sheet };
    let mut area = 0.0;
    let (mut col, mut alpha, mut n) = (Vec3::ZERO, 0.0, 0.0);
    for t in tris {
        let p = t.map(|i| Vec3::from(vx(i).position));
        area += (p[1] - p[0]).cross(p[2] - p[0]).length() * 0.5;
        for &i in t {
            let v = vx(i);
            ulo = ulo.min(v.uv[0]);
            uhi = uhi.max(v.uv[0]);
            vlo = vlo.min(v.uv[1]);
            vhi = vhi.max(v.uv[1]);
            col += Vec3::from_slice(&v.color[..3]);
            alpha += v.color[3];
            n += 1.0;
        }
    }
    if uhi - ulo < 1e-4 || vhi - vlo < 1e-4 || area < 0.05 {
        return None;
    }
    // s = 0 at the source: the end the native scroll carries the texture
    // away from (TRN_WATERFALL_A scrolls v +0.9375/s: the high-v end;
    // TRN_WATERFALL_A_LL −0.586/s: the low-v end). It is the art's flow:
    // falls fall, the centre jets rise (their v runs up the column).
    let source_high = kind == Kind::Sheet;
    let along = |v: f32| {
        let f = (v - vlo) / (vhi - vlo);
        if source_high { 1.0 - f } else { f }
    };
    let across = |u: f32| (u - ulo) / (uhi - ulo);
    // Rasterize into the chart (positions averaged where parts overlap).
    let mut acc = vec![[0.0f32; 4]; CHART_U * CHART_V];
    for t in tris {
        let p = t.map(|i| Vec3::from(vx(i).position));
        let q =
            t.map(|i| Vec2::new(across(vx(i).uv[0]) * (CHART_U - 1) as f32, along(vx(i).uv[1]) * (CHART_V - 1) as f32));
        let det = (q[1] - q[0]).perp_dot(q[2] - q[0]);
        if det.abs() < 1e-6 {
            continue;
        }
        let x0 = q.iter().map(|v| v.x).fold(f32::INFINITY, f32::min).floor().max(0.0) as usize;
        let x1 = (q.iter().map(|v| v.x).fold(f32::NEG_INFINITY, f32::max).ceil() as usize).min(CHART_U - 1);
        let y0 = q.iter().map(|v| v.y).fold(f32::INFINITY, f32::min).floor().max(0.0) as usize;
        let y1 = (q.iter().map(|v| v.y).fold(f32::NEG_INFINITY, f32::max).ceil() as usize).min(CHART_V - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let c = Vec2::new(x as f32, y as f32);
                let w1 = (c - q[0]).perp_dot(q[2] - q[0]) / det;
                let w2 = (q[1] - q[0]).perp_dot(c - q[0]) / det;
                let w0 = 1.0 - w1 - w2;
                // A little slack: texels on an edge belong to both sides.
                let e = -0.02;
                if w0 >= e && w1 >= e && w2 >= e {
                    let w = p[0] * w0 + p[1] * w1 + p[2] * w2;
                    let a = &mut acc[x + CHART_U * y];
                    a[0] += w.x;
                    a[1] += w.y;
                    a[2] += w.z;
                    a[3] += 1.0;
                }
            }
        }
    }
    let mut texels: Vec<[f32; 4]> =
        acc.iter().map(|a| if a[3] > 0.0 { [a[0] / a[3], a[1] / a[3], a[2] / a[3], 1.0] } else { [0.0; 4] }).collect();
    if texels.iter().filter(|t| t[3] > 0.0).count() < CHART_U * CHART_V / 8 {
        return None;
    }
    // Holes filled from covered neighbours (w stays 0: no water there).
    for _ in 0..(CHART_U + CHART_V) {
        let mut changed = false;
        let prev = texels.clone();
        for y in 0..CHART_V {
            for x in 0..CHART_U {
                if prev[x + CHART_U * y][3] > 0.0 || prev[x + CHART_U * y][3] < 0.0 {
                    continue;
                }
                let mut s = Vec3::ZERO;
                let mut k = 0.0;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= CHART_U as i32 || ny >= CHART_V as i32 {
                        continue;
                    }
                    let t = prev[nx as usize + CHART_U * ny as usize];
                    if t[3] != 0.0 {
                        s += Vec3::new(t[0], t[1], t[2]);
                        k += 1.0;
                    }
                }
                if k > 0.0 {
                    let v = s / k;
                    texels[x + CHART_U * y] = [v.x, v.y, v.z, -1.0];
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for t in &mut texels {
        if t[3] < 0.0 {
            t[3] = 0.0;
        }
    }
    let mut c = Chart {
        record,
        placement: r.placement,
        kind,
        arc,
        texels,
        table: [0.0; TABLE],
        life: 1.0,
        area,
        length: 0.0,
        centre: r.centre,
        radius: r.radius,
        u_range: Vec2::new(ulo, uhi),
        v_range: if source_high { Vec2::new(vhi, vlo) } else { Vec2::new(vlo, vhi) },
        color: (col / n).min(Vec3::ONE),
        alpha: alpha / n,
    };
    // The path along the middle: its length and timing under gravity
    // (a fall leaves its lip at LIP_SPEED; an arc has its apex's
    // horizontal speed there, from its reach and heights).
    let pts: Vec<Vec3> = (0..CHART_V).map(|k| c.at(0.5, k as f32 / (CHART_V - 1) as f32)).collect();
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for t in c.texels.iter().filter(|t| t[3] > 0.0) {
        let p = Vec3::new(t[0], t[1], t[2]);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    c.centre = (lo + hi) * 0.5;
    c.radius = (hi - lo).length() * 0.5 + 0.5;
    let ztop = pts.iter().map(|p| p.z).fold(f32::NEG_INFINITY, f32::max);
    let v_top = if arc {
        let src = pts[0];
        let land = pts[CHART_V - 1];
        let up = ((ztop - src.z).max(0.05) * 2.0 / GRAVITY).sqrt();
        let down = ((ztop - land.z).max(0.05) * 2.0 / GRAVITY).sqrt();
        (land - src).truncate().length() / (up + down)
    } else {
        LIP_SPEED
    };
    let mut times = vec![0.0f32; CHART_V];
    let mut len = 0.0;
    for k in 1..CHART_V {
        let d = pts[k].distance(pts[k - 1]);
        len += d;
        let z = 0.5 * (pts[k].z + pts[k - 1].z);
        let v = (v_top * v_top + 2.0 * GRAVITY * (ztop - z).max(0.0)).sqrt().max(0.3);
        times[k] = times[k - 1] + d / v;
    }
    c.length = len;
    c.life = times[CHART_V - 1].max(0.05);
    for (k, e) in c.table.iter_mut().enumerate() {
        let tau = c.life * k as f32 / (TABLE - 1) as f32;
        let j = times.partition_point(|&t| t <= tau).clamp(1, CHART_V - 1);
        let (t0, t1) = (times[j - 1], times[j]);
        let f = if t1 > t0 { ((tau - t0) / (t1 - t0)).clamp(0.0, 1.0) } else { 0.0 };
        *e = ((j - 1) as f32 + f) / (CHART_V - 1) as f32;
    }
    c.table[TABLE - 1] = 1.0;
    Some(c)
}

// ---------------------------------------------------------------------
// The GPU half.

use crate::render::Gpu;
use crate::render::volume_fx;
use glam::Mat4;
use std::collections::HashMap;
use std::ops::Range;

/// Particle entries (charts and landings' splashes) a frame.
pub const MAX_ACTIVE: usize = 128;
/// Simulated basins at once (slots).
pub const MAX_SIM: usize = 6;
const SLOTS: usize = 8;
/// The simulation's cells per side of a basin's field.
pub const SIM: u32 = 128;
/// The basins' drawn grid: quads per side (water_ng.wgsl `GRID`).
pub const GRID: u32 = 96;
/// The particle buffer (jets and splashes).
pub const PARTICLES: u32 = 49152;
/// The particles draw within this distance (m), fading out from FADE_FROM
/// (the native sheet fading in).
pub const REACH: f32 = 60.0;
pub const FADE_FROM: f32 = 42.0;
/// Basins simulated within this distance (m).
pub const SIM_REACH: f32 = 60.0;
/// Steps a basin is warmed up over when it comes into reach (2.5 s).
pub const WARM: i64 = 150;
/// Sheets bigger than this (m², the hills' falls) keep the mesh.
pub const MAX_AREA: f32 = 80.0;
/// Drops per landing splash, splashes a frame.
pub const PER_SPLASH: u32 = 96;
pub const MAX_SPLASH: usize = 24;
/// Mist puffs per landing (volumetric), landings with mist.
pub const MIST_PER: usize = 14;
pub const MAX_MIST: usize = 24;
const STEP_STRIDE: u64 = 256;
const MAX_STEPS: usize = WARM as usize + 16;

/// The look's knobs (inferred, by eye: no native reference exists).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// The landing particles' push on the water.
    pub impulse: f32,
    /// The waves' speed (m/s), damping a step, foam made per push.
    pub wave_speed: f32,
    pub damping: f32,
    pub foam: f32,
    /// The particles per m² near the eye and their floor far away.
    pub density: f32,
    pub density_min: f32,
    /// The particles' radius against their spacing (their push on the
    /// water; the drops are drawn at their own size).
    pub radius: f32,
}

pub const LOOK: Look = Look {
    impulse: 30.0,
    wave_speed: 0.7,
    damping: 0.03,
    foam: 0.3,
    density: 2600.0,
    density_min: 50.0,
    radius: 0.9,
};

impl Look {
    /// `NFSU2_WATER_NG_LOOK=k=v,…` overrides by name (tuning only).
    pub fn from_env() -> Look {
        let mut l = LOOK;
        if let Ok(s) = crate::render::debug_env::nfsu2_water_ng_look() {
            for kv in s.split(',') {
                let mut it = kv.splitn(2, '=');
                let (Some(k), Some(v)) = (it.next(), it.next().and_then(|v| v.trim().parse::<f32>().ok())) else {
                    continue;
                };
                match k.trim() {
                    "impulse" => l.impulse = v,
                    "wave_speed" => l.wave_speed = v,
                    "damping" => l.damping = v,
                    "foam" => l.foam = v,
                    "density" => l.density = v,
                    "density_min" => l.density_min = v,
                    "radius" => l.radius = v,
                    _ => {}
                }
            }
        }
        l
    }
}

/// A particle entry: a chart's share of the buffer, or a landing's
/// splash (chart −1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub chart: i32,
    pub first: u32,
    pub count: u32,
    pub fade: f32,
    /// Charts: the radius; splashes: position and strength.
    pub extra: [f32; 4],
}

/// The particles' share per chart: the nearest records first, by area and
/// distance, scaled into `budget`. `charts`: (chart index, area, distance,
/// fade). Returns the entries (first indices packed) and the radius each.
pub fn allocate(charts: &[(usize, f32, f32, f32)], budget: u32, look: &Look) -> Vec<Entry> {
    let density = |d: f32| (look.density / (1.0 + d / 6.0).powi(2)).max(look.density_min);
    let want: f32 = charts.iter().map(|c| c.1 * density(c.2)).sum();
    let scale = if want > budget as f32 { budget as f32 / want } else { 1.0 };
    let mut out = Vec::new();
    let mut first = 0u32;
    for &(chart, area, d, fade) in charts {
        if out.len() >= MAX_ACTIVE {
            break;
        }
        let dens = density(d) * scale;
        let count = ((area * dens) as u32).max(64);
        if first + count > budget {
            break;
        }
        let radius = (look.radius / dens.sqrt()).clamp(0.01, 0.25);
        out.push(Entry { chart: chart as i32, first, count, fade, extra: [radius, 0.0, 0.0, 0.0] });
        first += count;
    }
    out
}

/// A slot's simulation steps: `next` the step it runs next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub field: usize,
    pub next: i64,
}

/// One step's work: its index, the slots it steps and those it clears.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub n: i64,
    pub mask: u32,
    pub clear: u32,
}

/// Assign the wanted fields (nearest first) to slots, keeping those that
/// stay; bring every slot to step `now` (new ones warmed up over WARM
/// steps, cleared first; one whose clock ran back or far ahead likewise).
pub fn schedule(slots: &mut [Option<Slot>; SLOTS], wanted: &[usize], now: i64) -> Vec<Step> {
    for s in slots.iter_mut() {
        if s.is_some_and(|s| !wanted.contains(&s.field)) {
            *s = None;
        }
    }
    let mut fresh = 0u32;
    for &f in wanted.iter().take(MAX_SIM) {
        if slots.iter().any(|s| s.is_some_and(|s| s.field == f)) {
            continue;
        }
        if let Some(i) = slots.iter().position(|s| s.is_none()) {
            slots[i] = Some(Slot { field: f, next: now - WARM - 1 });
            fresh |= 1 << i;
        }
    }
    for (i, s) in slots.iter_mut().enumerate() {
        if let Some(s) = s
            && (s.next > now || now - s.next > 2 * WARM)
        {
            s.next = now - WARM - 1;
            fresh |= 1 << i;
        }
    }
    let start = slots.iter().flatten().map(|s| s.next).min().unwrap_or(now);
    let mut steps = Vec::new();
    for n in start..now {
        let mut mask = 0;
        let mut clear = 0;
        for (i, s) in slots.iter().enumerate() {
            if let Some(s) = s
                && s.next <= n
            {
                mask |= 1 << i;
                if s.next == n && fresh & (1 << i) != 0 {
                    clear |= 1 << i;
                }
            }
        }
        steps.push(Step { n, mask, clear });
    }
    for s in slots.iter_mut().flatten() {
        s.next = now;
    }
    steps
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct NgUniform {
    counts: [u32; 4],
    time: [f32; 4],
    eye: [f32; 4],
    look: [f32; 4],
    active: [[f32; 4]; MAX_ACTIVE],
    active2: [[f32; 4]; MAX_ACTIVE],
    slots: [[f32; 4]; 2 * SLOTS],
    slot_col: [[f32; 4]; SLOTS],
    rect: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct StepUniform {
    n: [u32; 4],
    t: [f32; 4],
    pad: [f32; 56],
}

/// A body's surface for the grid: kind, native uv per metre, mean v0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub kind: Kind,
    pub uv_scale: f32,
    pub color: [f32; 4],
}

/// The surfaces' kind, uv density and mean vertex colour per field (from
/// the field's record's flat surface triangles).
pub fn bodies(mesh: &Mesh, fields: &[crate::render::water_hq::WetField]) -> Vec<Body> {
    fields
        .iter()
        .map(|f| {
            let r = mesh.records.iter().find(|r| !r.lod_b && r.placement == f.placement);
            let mut kinds = [0usize; 5];
            let (mut uvs, mut lens, mut col, mut n) = (0.0f32, 0.0f32, [0.0f32; 4], 0.0f32);
            if let Some(r) = r {
                for t in mesh.indices[r.surfaces.start as usize..r.surfaces.end as usize].chunks_exact(3) {
                    let v = [t[0], t[1], t[2]].map(|i| mesh.vertices[i as usize]);
                    for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                        let dp = (Vec3::from(v[a].position) - Vec3::from(v[b].position)).truncate().length();
                        let du = (Vec2::from(v[a].uv) - Vec2::from(v[b].uv)).length();
                        if dp > 0.05 {
                            uvs += du;
                            lens += dp;
                        }
                    }
                    for x in &v {
                        kinds[(x.kind as usize).min(4)] += 1;
                        for k in 0..4 {
                            col[k] += x.color[k];
                        }
                        n += 1.0;
                    }
                }
            }
            let kind = if kinds[1] > kinds[0] { Kind::Pool } else { Kind::Basin };
            Body {
                kind,
                uv_scale: if lens > 0.0 { (uvs / lens).clamp(0.02, 4.0) } else { 0.25 },
                color: col.map(|c| if n > 0.0 { c / n } else { 1.0 }),
            }
        })
        .collect()
}

/// A field's surface attributes per cell (its record's flat water
/// triangles rasterized as the field's level is): the native uv and
/// vertex colour (the art's bake), filled outward from the water.
pub fn field_attrs(mesh: &Mesh, f: &crate::render::water_hq::WetField) -> (Vec<[f32; 2]>, Vec<[u8; 4]>) {
    use crate::render::water_hq::WET_CELLS as N;
    let mut uv = vec![[0.0f32; 2]; N * N];
    let mut col = vec![[255u8; 4]; N * N];
    let mut set = vec![false; N * N];
    if let Some(r) = mesh.records.iter().find(|r| !r.lod_b && r.placement == f.placement) {
        for t in mesh.indices[r.surfaces.start as usize..r.surfaces.end as usize].chunks_exact(3) {
            let v = [t[0], t[1], t[2]].map(|i| mesh.vertices[i as usize]);
            let p = v.map(|x| Vec3::from(x.position));
            if (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero().z.abs() <= 0.7 {
                continue;
            }
            let a = p.map(|q| (Vec2::new(q.x, q.y) - f.origin) / f.cell - Vec2::splat(0.5));
            let det = (a[1] - a[0]).perp_dot(a[2] - a[0]);
            if det.abs() < 1e-9 {
                continue;
            }
            let x0 = a.iter().map(|q| q.x).fold(f32::INFINITY, f32::min).floor().max(0.0) as usize;
            let x1 = a.iter().map(|q| q.x).fold(f32::NEG_INFINITY, f32::max).ceil().min(N as f32 - 1.0);
            let y0 = a.iter().map(|q| q.y).fold(f32::INFINITY, f32::min).floor().max(0.0) as usize;
            let y1 = a.iter().map(|q| q.y).fold(f32::NEG_INFINITY, f32::max).ceil().min(N as f32 - 1.0);
            if x1 < 0.0 || y1 < 0.0 {
                continue;
            }
            for y in y0..=y1 as usize {
                for x in x0..=x1 as usize {
                    let c = Vec2::new(x as f32, y as f32);
                    let w1 = (c - a[0]).perp_dot(a[2] - a[0]) / det;
                    let w2 = (a[1] - a[0]).perp_dot(c - a[0]) / det;
                    let w0 = 1.0 - w1 - w2;
                    if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                        let k = y * N + x;
                        let u = Vec2::from(v[0].uv) * w0 + Vec2::from(v[1].uv) * w1 + Vec2::from(v[2].uv) * w2;
                        let c4: [f32; 4] =
                            std::array::from_fn(|i| v[0].color[i] * w0 + v[1].color[i] * w1 + v[2].color[i] * w2);
                        uv[k] = u.to_array();
                        col[k] = c4.map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8);
                        set[k] = true;
                    }
                }
            }
        }
    }
    // Outward from the water: each empty cell takes a set neighbour's
    // values (the grid's quads at the edge reach a cell past it).
    for _ in 0..N {
        let prev = set.clone();
        let mut changed = false;
        for y in 0..N {
            for x in 0..N {
                let k = y * N + x;
                if prev[k] {
                    continue;
                }
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx >= 0 && ny >= 0 && nx < N as i32 && ny < N as i32 && prev[ny as usize * N + nx as usize] {
                        let j = ny as usize * N + nx as usize;
                        uv[k] = uv[j];
                        col[k] = col[j];
                        set[k] = true;
                        changed = true;
                        break;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    (uv, col)
}

/// The frame's part (CPU), made by [`Ng::prepare`].
#[derive(Clone, Default)]
pub struct Frame {
    pub entries: Vec<Entry>,
    pub particles: u32,
    /// Records drawn by particles: their sheets' native fade (0: hidden).
    pub drawn: HashMap<usize, f32>,
    /// Records whose flat water the grid draws.
    pub simulated: Vec<usize>,
    pub slots: u32,
    pub steps: Vec<Step>,
    pub now: i64,
    /// The entries' world spheres (the drops' screen rectangle).
    pub spheres: Vec<(Vec3, f32)>,
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub mist: u32,
    /// The draw lists: plain surfaces, lips, sheets (range, instance).
    pub surfaces: Vec<Range<u32>>,
    pub lips: Vec<Range<u32>>,
    pub sheets: Vec<(Range<u32>, u32)>,
}

/// What `Hq::draw` hands over.
pub(crate) struct Pass<'a> {
    pub group2: &'a wgpu::BindGroup,
    pub depth_group: &'a wgpu::BindGroup,
    pub target: &'a wgpu::TextureView,
    pub format: wgpu::TextureFormat,
    pub samples: u32,
    pub depth: &'a wgpu::TextureView,
    pub size: (u32, u32),
    pub ms: bool,
}

pub struct Ng {
    pub charts: Vec<Chart>,
    record_charts: HashMap<usize, Vec<usize>>,
    bodies: Vec<Body>,
    look: Look,
    slots: [Option<Slot>; SLOTS],
    pub frame: Frame,
    uniform: wgpu::Buffer,
    steps: wgpu::Buffer,
    texels: wgpu::Buffer,
    info: wgpu::Buffer,
    particles: wgpu::Buffer,
    sim: wgpu::Buffer,
    force: wgpu::Buffer,
    sim_view: wgpu::TextureView,
    sim_store: wgpu::TextureView,
    attr_uv: wgpu::TextureView,
    attr_col: wgpu::TextureView,
    sampler: wgpu::Sampler,
    compute_layout: wgpu::BindGroupLayout,
    compute: [wgpu::ComputePipeline; 4],
    layout3: wgpu::BindGroupLayout,
    shaders: [wgpu::ShaderModule; 2],
    pipelines: std::sync::Mutex<HashMap<(u8, wgpu::TextureFormat, u32, bool, bool), wgpu::RenderPipeline>>,
    pub mist: volume_fx::VolumeFx,
    mist_built: bool,
    layout2: Option<wgpu::BindGroupLayout>,
}

/// The mist's volume: 160 × 160 × 32 voxels of 0.35 m, 2 Ki particles.
pub const MIST_BUDGET: volume_fx::Budget =
    volume_fx::Budget { dims: [160, 160, 32], voxel: 0.35, particles: 2048, steps: 96 };
/// The mist's shading: brighter, whiter, softer than smoke (inferred).
pub const MIST_LOOK: volume_fx::Look = volume_fx::Look {
    scene: 0.0,
    near: 0.3,
    shadow: 0.0,
    ambient: Vec3::new(0.75, 0.8, 0.85),
    g: 0.6,
    erosion: 0.7,
    detail: 0.22,
    thin_white: 0.95,
    shade: 0.25,
    sharp: 1.2,
    lamp: 1.3,
    spot: 1.2,
    point: 1.0,
    fine: 0.0,
    directional: 0.0,
    temporal: 0.0,
    step: 1.0,
};

fn storage(device: &wgpu::Device, label: &str, size: u64, extra: wgpu::BufferUsages) -> wgpu::Buffer {
    crate::render::zeroed_buffer(
        device,
        &wgpu::BufferDescriptor {
            label: Some(label),
            size: size.max(16),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | extra,
            mapped_at_creation: false,
        },
    )
}

impl Ng {
    /// `frames`: the flip-book's frames in the water texture array (the
    /// sheets' layers follow them, water.rs `Layers`).
    pub(crate) fn new(gpu: &Gpu, mesh: &Mesh, hq: &crate::render::water_hq::Hq, frames: u32) -> Ng {
        let device = &gpu.device;
        let charts = charts(mesh);
        let mut record_charts: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, c) in charts.iter().enumerate() {
            record_charts.entry(c.record).or_default().push(i);
        }
        let bodies = bodies(mesh, &hq.fields);
        let texels: Vec<[f32; 4]> = charts.iter().flat_map(|c| c.texels.iter().copied()).collect();
        let mut info: Vec<[f32; 4]> = Vec::with_capacity(charts.len() * 12);
        for c in &charts {
            for k in 0..8 {
                info.push([c.table[k * 4], c.table[k * 4 + 1], c.table[k * 4 + 2], c.table[k * 4 + 3]]);
            }
            info.push([c.life, c.area, c.length, if c.arc { 1.0 } else { 0.0 }]);
            info.push([c.u_range.x, c.u_range.y, c.v_range.x, c.v_range.y]);
            info.push([c.color.x, c.color.y, c.color.z, c.alpha]);
            // The texture layer (water.rs `Layers`: the flip-book's frames, then one each).
            info.push([(frames + c.kind as u32 - 1) as f32, 0.0, 0.0, 0.0]);
        }
        let texels_buf = storage(device, "water ng charts", (texels.len() * 16) as u64, wgpu::BufferUsages::empty());
        if !texels.is_empty() {
            gpu.queue.write_buffer(&texels_buf, 0, bytemuck::cast_slice(&texels));
        }
        let info_buf = storage(device, "water ng chart info", (info.len() * 16) as u64, wgpu::BufferUsages::empty());
        if !info.is_empty() {
            gpu.queue.write_buffer(&info_buf, 0, bytemuck::cast_slice(&info));
        }
        let uniform = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("water ng"),
                size: std::mem::size_of::<NgUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let steps = crate::render::zeroed_buffer(
            device,
            &wgpu::BufferDescriptor {
                label: Some("water ng steps"),
                size: STEP_STRIDE * (MAX_STEPS as u64 + 1),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        let particles = storage(device, "water ng particles", PARTICLES as u64 * 48, wgpu::BufferUsages::empty());
        let sim =
            storage(device, "water ng sim", SLOTS as u64 * 4 * (SIM * SIM) as u64 * 4, wgpu::BufferUsages::empty());
        let force =
            storage(device, "water ng force", SLOTS as u64 * (SIM * SIM) as u64 * 4, wgpu::BufferUsages::empty());
        let sim_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("water ng sim"),
            size: wgpu::Extent3d { width: SIM, height: SIM, depth_or_array_layers: SLOTS as u32 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let array =
            wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() };
        let sim_view = sim_tex.create_view(&array);
        let sim_store = sim_tex.create_view(&array);
        // The basins' native uv and vertex colour per field cell.
        let (attr_uv, attr_col) = {
            use crate::render::water_hq::WET_CELLS as N;
            let layers = hq.fields.len().clamp(1, 256) as u32;
            let mk = |label, format| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: N as u32, height: N as u32, depth_or_array_layers: layers },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                })
            };
            let tu = mk("water ng field uv", wgpu::TextureFormat::Rg32Float);
            let tc = mk("water ng field colour", wgpu::TextureFormat::Rgba8Unorm);
            for (layer, f) in hq.fields.iter().take(256).enumerate() {
                let (uv, col) = field_attrs(mesh, f);
                let at = |t| wgpu::TexelCopyTextureInfo {
                    texture: t,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: layer as u32 },
                    aspect: wgpu::TextureAspect::All,
                };
                let ext = wgpu::Extent3d { width: N as u32, height: N as u32, depth_or_array_layers: 1 };
                gpu.queue.write_texture(
                    at(&tu),
                    bytemuck::cast_slice(&uv),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(N as u32 * 8),
                        rows_per_image: Some(N as u32),
                    },
                    ext,
                );
                gpu.queue.write_texture(
                    at(&tc),
                    bytemuck::cast_slice(&col),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(N as u32 * 4),
                        rows_per_image: Some(N as u32),
                    },
                    ext,
                );
            }
            let arr = wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            };
            (tu.create_view(&arr), tc.create_view(&arr))
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water ng"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let cs = wgpu::ShaderStages::COMPUTE;
        let buf = |binding, ty, dynamic| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: cs,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: dynamic, min_binding_size: None },
            count: None,
        };
        let tex = |binding, dim| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: cs,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: dim,
                multisampled: false,
            },
            count: None,
        };
        let ro = wgpu::BufferBindingType::Storage { read_only: true };
        let rw = wgpu::BufferBindingType::Storage { read_only: false };
        let compute_layout = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("water ng compute"),
            entries: &[
                buf(0, wgpu::BufferBindingType::Uniform, false),
                buf(1, ro, false),
                buf(2, ro, false),
                buf(3, rw, false),
                tex(4, wgpu::TextureViewDimension::D2Array),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: cs,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                tex(6, wgpu::TextureViewDimension::D2Array),
                buf(7, rw, false),
                buf(8, rw, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: cs,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
                buf(10, wgpu::BufferBindingType::Uniform, true),
            ],
        });
        let module = crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
            label: Some("water ng sim"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water_ng_sim.wgsl")).into(),
            ),
        });
        let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("water ng compute"),
            bind_group_layouts: &[Some(&compute_layout)],
            immediate_size: 0,
        });
        let compute = ["cs_particles", "cs_impulse", "cs_step", "cs_export"].map(|e| {
            crate::render::core::device_cache::compute_pipeline(gpu, &wgpu::ComputePipelineDescriptor {
                label: Some(e),
                layout: Some(&pl),
                module: &module,
                entry_point: Some(e),
                compilation_options: Default::default(),
                cache: gpu.disk_pipeline_cache(),
            })
        });
        let vf = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        let ftex = |binding, filterable, dim, vis| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vis,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable },
                view_dimension: dim,
                multisampled: false,
            },
            count: None,
        };
        let layout3 = crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
            label: Some("water ng"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: vf,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: ro, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                ftex(5, true, wgpu::TextureViewDimension::D2Array, vf),
                ftex(6, false, wgpu::TextureViewDimension::D2Array, wgpu::ShaderStages::VERTEX),
                ftex(7, true, wgpu::TextureViewDimension::D2Array, wgpu::ShaderStages::VERTEX),
            ],
        });
        let source = format!(
            "{}\n{}\n{}",
            crate::render::water::shader_src(
                "water.wgsl",
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water.wgsl"))
            ),
            crate::render::water::shader_src(
                "water_hq.wgsl",
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water_hq.wgsl"))
            ),
            crate::render::water::shader_src(
                "water_ng.wgsl",
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water_ng.wgsl"))
            )
        );
        let m = |label, src: String| {
            crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        let shaders = [
            m("water ng", source.clone()),
            m(
                "water ng ms",
                source
                    .replace("texture_depth_2d", "texture_depth_multisampled_2d")
                    .replace("var scene_tex: texture_2d<f32>", "var scene_tex: texture_multisampled_2d<f32>"),
            ),
        ];
        let mist = volume_fx::VolumeFx::new(gpu, MIST_BUDGET, MIST_LOOK);
        Ng {
            charts,
            record_charts,
            bodies,
            look: Look::from_env(),
            slots: [None; SLOTS],
            frame: Frame::default(),
            uniform,
            steps,
            texels: texels_buf,
            info: info_buf,
            particles,
            sim,
            force,
            sim_view,
            sim_store,
            attr_uv,
            attr_col,
            sampler,
            compute_layout,
            compute,
            layout3,
            shaders,
            pipelines: Default::default(),
            mist,
            mist_built: false,
            layout2: Some(hq.layout2.clone()),
        }
    }

    /// The frame's particles, basins and draw lists (CPU), and its
    /// uniforms. `picked`: the records the view draws; `clock`: the
    /// frame's time (s, the world clock less the frame's lag); `world_clock`:
    /// the newest update's (the simulation's steps).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        &mut self,
        gpu: &Gpu,
        mesh: &Mesh,
        hq: &crate::render::water_hq::Hq,
        picked: &[usize],
        decide: (Vec3, &[usize], &[(crate::render::water::Landing, u32)]),
        cam: &crate::render::camera::Camera,
        view_proj: Mat4,
        reversed: bool,
        clock: f32,
        world_clock: f32,
        rain: f32,
        landings: &[(crate::render::water::Landing, u32)],
        lamps: &[(Vec3, f32, Vec3, f32)],
        frames: u32,
        modern: bool,
    ) {
        let eye = cam.position;
        // The jets' particles and the simulated basins are decided
        // at the newest world tick's eye and records (`Water::prepare`).
        let (deye, dpicked, dlandings) = decide;
        let look = self.look;
        // The jets: records near enough, nearest first.
        let mut drawn: HashMap<usize, f32> = HashMap::new();
        let mut list: Vec<(usize, f32, f32, f32)> = Vec::new();
        for &ri in dpicked {
            let r = &mesh.records[ri];
            let Some(cs) = self.record_charts.get(&ri) else { continue };
            if r.lod_b || cs.iter().any(|&c| self.charts[c].area > MAX_AREA) {
                continue;
            }
            let d = (deye.distance(r.centre) - r.radius).max(0.0);
            if d >= REACH {
                continue;
            }
            let fade = 1.0 - smoothstep(FADE_FROM, REACH, d);
            drawn.insert(ri, fade);
            for &c in cs {
                let ch = &self.charts[c];
                list.push((c, ch.area, (deye.distance(ch.centre) - ch.radius).max(0.0), fade));
            }
        }
        list.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
        let splashes: Vec<&(crate::render::water::Landing, u32)> =
            dlandings.iter().filter(|(l, _)| l.position.distance(deye) < 35.0).take(MAX_SPLASH).collect();
        let budget = PARTICLES - splashes.len() as u32 * PER_SPLASH;
        let mut entries = allocate(&list, budget, &look);
        let mut first = entries.last().map_or(0, |e| e.first + e.count);
        for (l, _) in &splashes {
            if entries.len() >= MAX_ACTIVE {
                break;
            }
            entries.push(Entry {
                chart: -1,
                first,
                count: PER_SPLASH,
                fade: 1.0,
                extra: [l.position.x, l.position.y, l.position.z, l.strength],
            });
            first += PER_SPLASH;
        }
        // A record whose charts did not all get particles keeps its mesh.
        let given: std::collections::HashSet<usize> =
            entries.iter().filter(|e| e.chart >= 0).map(|e| self.charts[e.chart as usize].record).collect();
        drawn.retain(|r, _| {
            given.contains(r) && self.record_charts[r].iter().all(|c| entries.iter().any(|e| e.chart == *c as i32))
        });
        entries.retain(|e| e.chart < 0 || drawn.contains_key(&self.charts[e.chart as usize].record));
        // Packed again after the drops.
        let mut first = 0;
        for e in &mut entries {
            e.first = first;
            first += e.count;
        }
        let particles = first;
        let spheres: Vec<(Vec3, f32)> = entries
            .iter()
            .map(|e| {
                if e.chart >= 0 {
                    let c = &self.charts[e.chart as usize];
                    (c.centre, c.radius)
                } else {
                    (Vec3::from_slice(&e.extra[..3]) + Vec3::Z * 0.8, 2.0)
                }
            })
            .collect();
        // The basins: the nearest fields of the drawn top-LOD records.
        let mut near: Vec<(f32, usize)> = dpicked
            .iter()
            .filter(|&&ri| !mesh.records[ri].lod_b)
            .filter_map(|&ri| hq.field_of.get(&mesh.records[ri].placement).copied())
            .filter(|&f| f < 256)
            .map(|f| (deye.clamp(hq.fields[f].lo, hq.fields[f].hi).distance(deye), f))
            .filter(|(d, _)| *d < SIM_REACH)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        near.dedup_by_key(|x| x.1);
        let wanted: Vec<usize> = near.iter().take(MAX_SIM).map(|x| x.1).collect();
        let now = (world_clock as f64 * 60.0).floor() as i64;
        let mut steps = schedule(&mut self.slots, &wanted, now);
        if steps.len() > MAX_STEPS {
            steps.drain(..steps.len() - MAX_STEPS);
        }
        let in_slot: HashMap<usize, usize> =
            self.slots.iter().enumerate().filter_map(|(i, s)| s.map(|s| (s.field, i))).collect();
        let simulated: Vec<usize> = picked
            .iter()
            .copied()
            .filter(|&ri| {
                !mesh.records[ri].lod_b
                    && hq.field_of.get(&mesh.records[ri].placement).is_some_and(|f| in_slot.contains_key(f))
            })
            .collect();
        // Draw lists.
        let mut surfaces = Vec::new();
        let mut lips = Vec::new();
        let mut sheets: Vec<(Range<u32>, u32)> = Vec::new();
        for &ri in picked {
            let r = &mesh.records[ri];
            if !r.surfaces.is_empty() {
                if simulated.contains(&ri) { lips.push(r.surfaces.clone()) } else { surfaces.push(r.surfaces.clone()) }
            }
            // Every sheet keeps its mesh (the jets' body: the art's streaks and
            // tint); the particles add the water that leaves it.
            if !r.sheets.is_empty() {
                let inst = 255;
                match sheets.last_mut() {
                    Some((last, i)) if *i == inst && last.end == r.sheets.start => last.end = r.sheets.end,
                    _ => sheets.push((r.sheets.clone(), inst)),
                }
            }
        }
        // The uniforms.
        let mut u: NgUniform = bytemuck::Zeroable::zeroed();
        u.counts = [entries.len() as u32, particles, SLOTS as u32, frames];
        u.time = [clock, 0.0, 0.0, 0.0];
        u.eye = eye.extend(0.0).to_array();
        u.look = [look.impulse, look.wave_speed, look.damping, look.foam];
        for (i, e) in entries.iter().enumerate() {
            u.active[i] = [e.chart as f32, e.first as f32, e.count as f32, e.fade];
            u.active2[i] = e.extra;
        }
        for i in 0..SLOTS {
            match self.slots[i] {
                Some(s) => {
                    let fl = &hq.fields[s.field];
                    let b = &self.bodies[s.field];
                    u.slots[i * 2] = [fl.origin.x, fl.origin.y, fl.cell, s.field as f32];
                    u.slots[i * 2 + 1] = [fl.base_z, b.uv_scale, b.kind as u32 as f32, s.field as f32];
                    u.slot_col[i] = b.color;
                }
                None => {
                    u.slots[i * 2] = [1.0e7, 1.0e7, 1.0, 0.0];
                    u.slots[i * 2 + 1] = [-1.0e6, 1.0, 0.0, 0.0];
                }
            }
        }
        // The last slot (never simulated, MAX_SIM < SLOTS) carries the
        // nearest jet's vertex colour: the aerated water's hue.
        if let Some(e) = entries.iter().find(|e| e.chart >= 0) {
            let c = self.charts[e.chart as usize].color;
            u.slot_col[SLOTS - 1] = [c.x, c.y, c.z, 1.0];
        }
        u.rect = [0.0, 0.0, 0.0, debug_view()];
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let mut data = vec![0u8; (steps.len() + 1) * STEP_STRIDE as usize];
        for (i, st) in steps.iter().chain(std::iter::once(&Step { n: now, mask: 0, clear: 0 })).enumerate() {
            let su = StepUniform {
                n: [st.n as u32, st.mask, st.clear, 0],
                t: [st.n as f32 / 60.0, (st.n + 1) as f32 / 60.0, rain, 0.0],
                pad: [0.0; 56],
            };
            data[i * STEP_STRIDE as usize..i * STEP_STRIDE as usize + std::mem::size_of::<StepUniform>()]
                .copy_from_slice(bytemuck::bytes_of(&su));
        }
        gpu.queue.write_buffer(&self.steps, 0, &data);
        // The mist: puffs rising where the water lands (volumetric).
        let mut puffs = Vec::new();
        for (l, id) in landings.iter().filter(|(l, _)| l.position.distance(eye) < 45.0).take(MAX_MIST) {
            puffs.extend(mist_puffs(l, *id, clock));
        }
        let count = puffs.len().min(MIST_BUDGET.particles as usize) as u32;
        self.mist.write_particles(gpu, 0, &puffs[..count as usize]);
        let lights = volume_fx::SceneLights {
            lamps: lamps.iter().map(|&(p, outer, c, _)| (p, outer, c)).collect(),
            spots: Vec::new(),
            points: Vec::new(),
            modern,
        };
        self.mist.frame(
            gpu,
            volume_fx::View {
                eye,
                forward: cam.forward(),
                view_proj,
                near: cam.near,
                far: cam.far,
                reversed,
                time: clock,
                fog: None,
            },
            &lights,
        );
        self.mist_built = false;
        if debug() {
            eprintln!(
                "water ng: {} entries, {} particles, slots {:?}, {} steps, {} simulated records, {} lips, {} plain surfaces, {} mist",
                entries.len(),
                particles,
                self.slots.iter().map(|s| s.map(|s| s.field)).collect::<Vec<_>>(),
                steps.len(),
                simulated.len(),
                lips.len(),
                surfaces.len(),
                count
            );
        }
        self.frame = Frame {
            entries,
            particles,
            drawn,
            simulated,
            slots: SLOTS as u32,
            steps,
            now,
            spheres,
            view_proj,
            eye,
            mist: count,
            surfaces,
            lips,
            sheets,
        };
    }

    /// The pipelines [`Ng::draw`] and the mist's draw use into `format`
    /// (`samples`), made now (`Water::warm_tier`): the plain surfaces
    /// (Realistic's), the basins' grid, the lips, the sheets and the drops,
    /// and the mist against the scene's depth and against its
    /// single-sampled copy after the temporal pass.
    pub(crate) fn warm(
        &self,
        gpu: &Gpu,
        hq: &crate::render::water_hq::Hq,
        base: &crate::render::water_hq::Base,
        format: wgpu::TextureFormat,
        samples: u32,
    ) {
        let ms = samples > 1;
        hq.pipeline(gpu, base, 0, format, samples, ms);
        for which in 10..=13 {
            self.pipeline(gpu, base, which, format, samples, ms);
        }
        for s in [samples, 1] {
            self.mist.warm(gpu, format, s, s);
        }
    }

    /// `which`: 10 grid, 11 lips, 12 sheets, 13 drops.
    fn pipeline(
        &self,
        gpu: &Gpu,
        base: &crate::render::water_hq::Base,
        which: u8,
        format: wgpu::TextureFormat,
        samples: u32,
        ms: bool,
    ) -> wgpu::RenderPipeline {
        let key = (which, format, samples, base.reversed && which <= 12, ms);
        let mut pipelines = self.pipelines.lock().unwrap();
        if let Some(p) = pipelines.get(&key) {
            return p.clone();
        }
        let hq_layout2 = self.layout2.as_ref().unwrap();
        // (The basins' passes write the scene's depth: no depth binding.)
        let depth = (which > 12).then(|| &base.depth_layouts[usize::from(ms)]);
        let groups = [Some(base.layout0), depth, Some(hq_layout2), Some(&self.layout3)];
        let layout = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
            label: Some("water ng"),
            bind_group_layouts: &groups,
            immediate_size: 0,
        });
        let module = &self.shaders[usize::from(ms)];
        let attributes =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x2, 3 => Float32x2, 4 => Float32x4];
        let buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<crate::render::water::Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        })];
        let comp = |s, d| wgpu::BlendComponent { src_factor: s, dst_factor: d, operation: wgpu::BlendOperation::Add };
        use wgpu::BlendFactor as F;
        let premultiplied = wgpu::BlendState {
            color: comp(F::One, F::OneMinusSrcAlpha),
            alpha: comp(F::SrcAlpha, F::OneMinusSrcAlpha),
        };
        let target = |format, blend| Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL });
        let (vs, fs, color, vertex_buffers, samples) = match which {
            10 => ("vs_grid", "fs_grid", target(format, None), false, samples),
            11 => ("vs_water_ng", "fs_surface_lips", target(format, None), true, samples),
            12 => ("vs_water_ng", "fs_sheet_ng", target(format, Some(premultiplied)), true, samples),
            _ => ("vs_drop", "fs_drop", target(format, Some(premultiplied)), false, samples),
        };
        let depth_stencil = match which {
            10..=12 => Some(wgpu::DepthStencilState {
                format: crate::render::DEPTH_FORMAT,
                depth_write_enabled: Some(which != 12),
                depth_compare: Some(if base.reversed {
                    wgpu::CompareFunction::GreaterEqual
                } else {
                    wgpu::CompareFunction::LessEqual
                }),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            _ => None,
        };
        let pipeline = crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
            label: Some(fs),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(vs),
                compilation_options: Default::default(),
                buffers: if vertex_buffers { &buffers } else { &[] },
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[color],
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

    fn group3(&self, gpu: &Gpu) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water ng"),
            layout: &self.layout3,
            entries: &[
                wgpu::BindGroupEntry { binding: 1, resource: self.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.particles.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.sim_view) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&self.attr_uv) },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&self.attr_col) },
            ],
        })
    }

    /// The drops' rectangle on a `size` target (x, y, w, h), None when
    /// nothing is on screen.
    pub fn rect(spheres: &[(Vec3, f32)], view_proj: Mat4, eye: Vec3, size: (u32, u32)) -> Option<[u32; 4]> {
        let (mut lo, mut hi) = (glam::Vec2::splat(1.0), glam::Vec2::splat(-1.0));
        for &(c, r) in spheres {
            for k in 0..8 {
                let o = Vec3::new(
                    if k & 1 == 0 { -r } else { r },
                    if k & 2 == 0 { -r } else { r },
                    if k & 4 == 0 { -r } else { r },
                );
                let q = view_proj * (c + o - eye).extend(1.0);
                if q.w <= 0.1 {
                    return Some([0, 0, size.0, size.1]);
                }
                let n = q.truncate().truncate() / q.w;
                lo = lo.min(n);
                hi = hi.max(n);
            }
        }
        let lo = lo.clamp(glam::Vec2::splat(-1.0), glam::Vec2::splat(1.0));
        let hi = hi.clamp(glam::Vec2::splat(-1.0), glam::Vec2::splat(1.0));
        if hi.x <= lo.x || hi.y <= lo.y {
            return None;
        }
        let x0 = ((lo.x * 0.5 + 0.5) * size.0 as f32).floor() as u32;
        let x1 = ((hi.x * 0.5 + 0.5) * size.0 as f32).ceil().min(size.0 as f32) as u32;
        let y0 = ((0.5 - hi.y * 0.5) * size.1 as f32).floor() as u32;
        let y1 = ((0.5 - lo.y * 0.5) * size.1 as f32).ceil().min(size.1 as f32) as u32;
        (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
    }

    /// The tier's passes (after the wet stone and the scene copy).
    pub(crate) fn draw(
        &mut self,
        hq: &crate::render::water_hq::Hq,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        base: &crate::render::water_hq::Base,
        pass: &Pass,
    ) {
        let fr = std::mem::take(&mut self.frame);
        // 1. Compute: the particles, the basins' steps, their texture.
        {
            let cg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("water ng compute"),
                layout: &self.compute_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.texels.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.info.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: self.particles.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(base.water_tex) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&hq.wet) },
                    wgpu::BindGroupEntry { binding: 7, resource: self.sim.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 8, resource: self.force.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::TextureView(&self.sim_store) },
                    wgpu::BindGroupEntry {
                        binding: 10,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.steps,
                            offset: 0,
                            size: std::num::NonZeroU64::new(std::mem::size_of::<StepUniform>() as u64),
                        }),
                    },
                ],
            });
            let mut cp = encoder
                .begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("water ng"), timestamp_writes: None });
            let groups = fr.particles.div_ceil(64);
            if groups > 0 {
                cp.set_pipeline(&self.compute[0]);
                cp.set_bind_group(0, &cg, &[0]);
                cp.dispatch_workgroups(groups, 1, 1);
            }
            let sg = SIM.div_ceil(8);
            for (i, _) in fr.steps.iter().enumerate() {
                let off = [(i as u64 * STEP_STRIDE) as u32];
                if groups > 0 {
                    cp.set_pipeline(&self.compute[1]);
                    cp.set_bind_group(0, &cg, &off);
                    cp.dispatch_workgroups(groups, 1, 1);
                }
                cp.set_pipeline(&self.compute[2]);
                cp.set_bind_group(0, &cg, &off);
                cp.dispatch_workgroups(sg, sg, SLOTS as u32);
            }
            cp.set_pipeline(&self.compute[3]);
            cp.set_bind_group(0, &cg, &[(fr.steps.len() as u64 * STEP_STRIDE) as u32]);
            cp.dispatch_workgroups(sg, sg, SLOTS as u32);
        }
        crate::prof::gpu::mark(encoder, "graphics: water ng sim");
        let color_load = |view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })
        };
        let depth_load = |view| {
            Some(wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            })
        };
        // 2. The plain surfaces (Realistic's), then the simulated basins,
        // the lips and the sheets left to the mesh.
        if !fr.surfaces.is_empty() {
            let p = hq.pipeline(gpu, base, 0, pass.format, pass.samples, pass.ms);
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("water ng surfaces"),
                color_attachments: &[color_load(pass.target)],
                depth_stencil_attachment: depth_load(pass.depth),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&p);
            rp.set_bind_group(0, base.group0, &[]);
            rp.set_bind_group(2, pass.group2, &[]);
            rp.set_vertex_buffer(0, base.vertices.slice(..));
            rp.set_index_buffer(base.indices.slice(..), wgpu::IndexFormat::Uint32);
            for r in &fr.surfaces {
                rp.draw_indexed(r.clone(), 0, 0..1);
            }
        }
        let occupied: Vec<u32> = (0..SLOTS as u32).filter(|&s| self.slots[s as usize].is_some()).collect();
        let g3 = self.group3(gpu);
        if !occupied.is_empty() || !fr.lips.is_empty() || !fr.sheets.is_empty() {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("water ng basins"),
                color_attachments: &[color_load(pass.target)],
                depth_stencil_attachment: depth_load(pass.depth),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_bind_group(0, base.group0, &[]);
            rp.set_bind_group(2, pass.group2, &[]);
            rp.set_bind_group(3, &g3, &[]);
            if !occupied.is_empty() {
                rp.set_pipeline(&self.pipeline(gpu, base, 10, pass.format, pass.samples, pass.ms));
                for &s in &occupied {
                    rp.draw(0..GRID * GRID * 6, s..s + 1);
                }
            }
            rp.set_vertex_buffer(0, base.vertices.slice(..));
            rp.set_index_buffer(base.indices.slice(..), wgpu::IndexFormat::Uint32);
            if !fr.lips.is_empty() {
                rp.set_pipeline(&self.pipeline(gpu, base, 11, pass.format, pass.samples, pass.ms));
                for r in &fr.lips {
                    rp.draw_indexed(r.clone(), 0, 255..256);
                }
            }
            if !fr.sheets.is_empty() {
                rp.set_pipeline(&self.pipeline(gpu, base, 12, pass.format, pass.samples, pass.ms));
                for (r, inst) in &fr.sheets {
                    rp.draw_indexed(r.clone(), 0, *inst..*inst + 1);
                }
            }
        }
        crate::prof::gpu::mark(encoder, "graphics: water");
        // 3. The drops that leave the jets and those thrown up where they land.
        let rect = if fr.particles > 0 { Self::rect(&fr.spheres, fr.view_proj, fr.eye, pass.size) } else { None };
        if let Some([x, y, w, h]) = rect {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("water ng drops"),
                color_attachments: &[color_load(pass.target)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&self.pipeline(gpu, base, 13, pass.format, pass.samples, pass.ms));
            rp.set_scissor_rect(x, y, w, h);
            rp.set_bind_group(0, base.group0, &[]);
            rp.set_bind_group(1, pass.depth_group, &[]);
            rp.set_bind_group(2, pass.group2, &[]);
            rp.set_bind_group(3, &g3, &[]);
            rp.draw(0..6, 0..fr.particles);
            drop(rp);
            crate::prof::gpu::mark(encoder, "graphics: water ng drops");
        }
        self.frame = fr;
    }

    /// The mist's volume (once a frame, after `draw`).
    pub fn build_mist(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder) {
        if !self.mist_built && self.frame.mist > 0 {
            self.mist.build(gpu, encoder, self.frame.mist);
            self.mist_built = true;
        }
    }

    /// The mist over the scene (`depth`: soft against it).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_mist(
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
        if self.mist_built {
            self.mist.warm(gpu, format, samples, depth_samples);
            self.mist.draw(gpu, encoder, target, format, samples, depth, depth_samples, viewport);
            crate::prof::gpu::mark(encoder, "graphics: water ng mist");
        }
    }
}

/// `NFSU2_WATER_NG_DEBUG=1`: the frame's counts on stderr (a check aid).
fn debug() -> bool {
    static D: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *D.get_or_init(|| crate::render::debug_env::nfsu2_water_ng_debug())
}

/// `NFSU2_WATER_NG_VIEW=1`: every drop drawn opaque red (a check aid: where
/// the drops are).
fn debug_view() -> f32 {
    static D: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *D.get_or_init(|| crate::render::debug_env::nfsu2_water_ng_view().unwrap_or(0.0))
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(747796405).wrapping_add(2891336453) ^ b.wrapping_mul(0x9E37_79B9);
    h = ((h >> ((h >> 28) + 4)) ^ h).wrapping_mul(277803737);
    ((h >> 22) ^ h) as f32 / u32::MAX as f32
}

/// A landing's mist puffs at `clock` (analytic: each rises, spreads and
/// thins over 2.2–3.6 s, cycle after cycle; deterministic).
pub fn mist_puffs(l: &crate::render::water::Landing, id: u32, clock: f32) -> Vec<volume_fx::Particle> {
    (0..MIST_PER as u32)
        .map(|k| {
            let seed = id.wrapping_mul(131).wrapping_add(k * 7);
            let life = 2.2 + 1.4 * hash(seed, 1);
            let x = clock / life + hash(seed, 2);
            let age = x.fract();
            let cycle = x.floor() as i64 as u32;
            let a = hash(seed, cycle.wrapping_mul(3).wrapping_add(5)) * std::f32::consts::TAU;
            let rr = hash(seed, cycle.wrapping_mul(5).wrapping_add(7)).sqrt() * (0.3 + 0.5 * l.strength);
            let pos = l.position
                + Vec3::new(a.cos() * rr, a.sin() * rr, 0.15)
                + Vec3::new(0.3, 0.2, 1.0) * age * (0.5 + 0.4 * l.strength);
            let r = (0.35 + 0.8 * age) * (0.7 + 0.5 * hash(seed, 9)) * (0.6 + 0.4 * l.strength);
            let alpha = 0.3 * (std::f32::consts::PI * age).sin().powi(2) * l.strength.min(1.0);
            let albedo = (l.color * 0.4 + Vec3::splat(0.6)).min(Vec3::ONE);
            volume_fx::Particle {
                pos_r: pos.extend(r).to_array(),
                albedo_sigma: albedo.extend(volume_fx::sigma_for_opacity(alpha, r)).to_array(),
                emit_lump: [0.0, 0.0, 0.0, 0.3],
                noise: [hash(seed, 11), hash(seed, 12), hash(seed, 13), hash(seed, 14) * 2.0 - 1.0],
                misc: [age, 1.0, 0.0, 0.0],
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::water::{Record, Vertex};

    fn validate(label: &str, src: &str) {
        use wgpu::naga;
        let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("{label}: {}", e.emit_to_string(src)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{label}: {e:?}"));
    }

    /// The tier's shaders parse and validate (both depth variants), and
    /// the Realistic module with the shared surface function still does.
    #[test]
    fn shaders_validate() {
        validate("sim", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water_ng_sim.wgsl")));
        let src = format!(
            "{}\n{}\n{}",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water.wgsl")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water_hq.wgsl")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water_ng.wgsl"))
        );
        validate("ng", &src);
        validate(
            "ng ms",
            &src.replace("texture_depth_2d", "texture_depth_multisampled_2d")
                .replace("var scene_tex: texture_2d<f32>", "var scene_tex: texture_multisampled_2d<f32>"),
        );
        validate(
            "hq",
            &format!(
                "{}\n{}",
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water.wgsl")),
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water_hq.wgsl"))
            ),
        );
    }

    /// The jets' rules that the pictures rest on: the drops are smeared over
    /// a fixed exposure (the same streak at any frame rate), never send more
    /// than DROP_MAX of the light that reaches them, and the sheets under them
    /// are Realistic's with the Next-Gen look; Realistic's own look unchanged.
    #[test]
    fn drops_and_sheets_keep_their_rules() {
        let ng = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water_ng.wgsl"));
        assert!(ng.contains("const DROP_EXPOSURE: f32 = 0.0083333;"));
        assert!(ng.contains("len = rv + 0.5 * sp * DROP_EXPOSURE;"));
        assert!(ng.contains("min(min(in.color.rgb + sky * 0.2, in.bound + sky * DROP_MAX), vec3<f32>(DROP_MAX))"));
        assert!(ng.contains("o.bound = (base + reach) * DROP_MAX;"));
        assert!(ng.contains("return sheet_hq(in, SheetLook(0.55, 0.42, 0.55, 0.05));"));
        let hq = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/water_hq.wgsl"));
        assert!(hq.contains("return sheet_hq(in, SheetLook(0.0, 0.14, 0.35, 0.25));"));
    }

    /// A sheet: a 2 m wide curtain falling 3 m, v running down it as the
    /// native art (TRN_WATERFALL_A: v grows toward the source).
    fn curtain() -> Mesh {
        let mut vertices = Vec::new();
        for j in 0..=4 {
            for i in 0..=2 {
                let s = j as f32 / 4.0;
                vertices.push(Vertex {
                    position: [i as f32, 0.3 * s, 10.0 - 3.0 * s],
                    kind: Kind::Sheet as u32 as f32,
                    uv: [i as f32 * 0.5, 1.0 - s],
                    aux: [1.0 - s, 3.0],
                    color: [0.8, 0.8, 0.8, 1.0],
                });
            }
        }
        let mut indices = Vec::new();
        for j in 0..4u32 {
            for i in 0..2u32 {
                let a = j * 3 + i;
                indices.extend([a, a + 1, a + 4, a, a + 4, a + 3]);
            }
        }
        let n = indices.len() as u32;
        Mesh {
            vertices,
            indices,
            records: vec![Record {
                placement: 3,
                lod_b: false,
                section: 0,
                flags: 0,
                surfaces: 0..0,
                sheets: 0..n,
                centre: Vec3::new(1.0, 0.0, 8.5),
                radius: 2.0,
                cull_position: Vec3::ZERO,
                cull_radius: 0.0,
                landings: vec![],
                apexes: vec![],
            }],
        }
    }

    #[test]
    fn a_curtain_charts_from_its_source_down() {
        let c = &charts(&curtain())[0];
        assert_eq!(c.placement, 3);
        assert!(!c.arc);
        // Source at the top (s = 0), landing at the bottom.
        assert!((c.at(0.5, 0.0).z - 10.0).abs() < 0.05, "{:?}", c.at(0.5, 0.0));
        assert!((c.at(0.5, 1.0).z - 7.0).abs() < 0.05, "{:?}", c.at(0.5, 1.0));
        assert!((c.at(0.0, 0.5).x - 0.0).abs() < 0.05 && (c.at(1.0, 0.5).x - 2.0).abs() < 0.05);
        assert!(c.texels.iter().all(|t| t[3] > 0.0), "covered");
        assert!((c.area - 2.0 * (9.0f32 + 0.09).sqrt()).abs() < 0.05, "{}", c.area);
        // Falling 3 m from 1.2 m/s: about 0.67 s; the table speeds up.
        let free = ((LIP_SPEED * LIP_SPEED + 2.0 * GRAVITY * 3.0).sqrt() - LIP_SPEED) / GRAVITY;
        assert!((c.life - free).abs() < 0.08, "{} vs {free}", c.life);
        assert!(c.table[0] == 0.0 && c.table[TABLE - 1] == 1.0);
        assert!(c.table.windows(2).all(|w| w[1] >= w[0]));
        assert!(
            c.table[TABLE / 2] < 0.5,
            "accelerating: less of the way in the first half of the time: {}",
            c.table[TABLE / 2]
        );
        let again = &charts(&curtain())[0];
        assert_eq!(c.texels, again.texels);
    }

    #[test]
    fn particles_are_shared_by_distance() {
        let l = LOOK;
        let e = allocate(&[(0, 10.0, 2.0, 1.0), (1, 10.0, 30.0, 1.0)], 65536, &l);
        assert_eq!(e.len(), 2);
        assert!(e[0].count > e[1].count * 4, "{:?}", e);
        assert!(e[0].extra[0] < e[1].extra[0], "nearer: smaller drops");
        assert_eq!(e[1].first, e[0].count);
        // Over budget: scaled down, packed, inside it.
        let many: Vec<(usize, f32, f32, f32)> = (0..40).map(|i| (i, 60.0, 1.0, 1.0)).collect();
        let e = allocate(&many, 20000, &l);
        let total: u32 = e.iter().map(|x| x.count).sum();
        assert!(total <= 20000 && total > 15000, "{total}");
    }

    #[test]
    fn slots_warm_up_and_keep() {
        let mut slots = [None; 8];
        let steps = schedule(&mut slots, &[5, 9], 1000);
        assert_eq!(steps.len() as i64, WARM + 1);
        assert_eq!(steps[0].clear, 0b11);
        assert!(steps[1..].iter().all(|s| s.clear == 0 && s.mask == 0b11));
        // The next frame: one step; a new body warms alone while the others step.
        let steps = schedule(&mut slots, &[5, 9], 1001);
        assert_eq!(steps, vec![Step { n: 1000, mask: 0b11, clear: 0 }]);
        let steps = schedule(&mut slots, &[9, 7], 1002);
        assert_eq!(slots[0].map(|s| s.field), Some(7), "the freed slot reused");
        assert_eq!(steps.len() as i64, WARM + 1);
        assert_eq!(steps[0].mask, 0b01);
        assert_eq!(steps.last().unwrap().mask, 0b11);
        // The clock ran back (another shot): warmed again.
        let steps = schedule(&mut slots, &[9, 7], 10);
        assert_eq!(steps[0].clear, 0b11);
        assert!(slots.iter().flatten().all(|s| s.next == 10));
    }

    #[test]
    fn mist_is_deterministic_and_rises() {
        let l = crate::render::water::Landing {
            position: Vec3::new(1.0, 2.0, 3.0),
            strength: 1.0,
            color: Vec3::splat(0.6),
        };
        let a = mist_puffs(&l, 7, 100.0);
        assert_eq!(a, mist_puffs(&l, 7, 100.0));
        assert_eq!(a.len(), MIST_PER);
        assert!(a.iter().all(|p| p.pos_r[2] >= 3.0 && p.pos_r[3] > 0.0 && p.albedo_sigma[3] >= 0.0));
    }

    #[test]
    fn the_rect_covers_the_jets() {
        let cam = crate::render::camera::Camera::new(Vec3::new(0.0, 0.0, 2.0), 0.0, 0.0);
        let vp = cam.projection_depth(16.0 / 9.0, false) * Mat4::look_to_rh(Vec3::ZERO, cam.forward(), cam.up);
        let ahead = cam.position + cam.forward() * 20.0;
        let r = Ng::rect(&[(ahead, 1.0)], vp, cam.position, (1920, 1080)).unwrap();
        assert!(r[0] > 800 && r[0] + r[2] < 1120 && r[2] > 20, "{r:?}");
        assert!(
            Ng::rect(&[(cam.position - cam.forward() * 50.0, 1.0)], vp, cam.position, (1920, 1080))
                .is_none_or(|r| r[2] == 1920)
        );
    }

    /// Asset test (ignored without NFSU2_ROOT): Bayview's jets chart, every
    /// fountain kind among them, deterministic.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn bayview_charts() {
        let root = crate::testkit::game_root();
        let world = crate::world::World::load(std::path::Path::new(&root), "L4RA").unwrap();
        let mesh = crate::render::water::build(&world);
        let c = charts(&mesh);
        let names: std::collections::BTreeSet<String> =
            c.iter().map(|c| world.solids[world.placements[c.placement].solid].name.clone()).collect();
        for n in
            ["XO_FOUNTAINWTRA_1A_LL_00", "XO_UC_FOUNTAINWATN_1A_00", "XO_FOUNTAINWATC_1A_00", "XO_WATERFALLB_1A_00"]
        {
            assert!(names.contains(n), "{n}: {names:?}");
        }
        assert_eq!(c.iter().filter(|c| c.arc).count(), 112, "the 28 arcs' parts");
        assert!(c.iter().all(|c| c.life > 0.0 && c.life < 8.0 && c.area > 0.0));
        let again = charts(&mesh);
        assert!(c.iter().zip(&again).all(|(a, b)| a.texels == b.texels && a.table == b.table));
        println!("{} charts", c.len());
    }
}

impl Ng {
    /// The basins' schedule (which field each slot simulates and
    /// the step it reached) as a digest.
    pub(crate) fn tick_digest(&self, h: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        for s in &self.slots {
            s.map(|s| (s.field, s.next)).hash(h);
        }
    }
}
