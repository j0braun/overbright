use super::*;
// ---------------------------------------------------------------------
// The air and the lens: car_lights.wgsl.

/// Headlight cones marched per frame at most (the nearest cars).
pub const MAX_SPOTS: usize = 8;
/// Lamps (halos and streaks) per frame at most.
pub const MAX_POINTS: usize = 64;
/// Cars farther than this have no cone / halo (their light is a sprite).
pub(in crate::render) const AIR_REACH: f32 = 120.0;

/// One headlight cone (`Spot` in car_lights.wgsl): both lamps of a car
/// with view 9's projector axes and scales (`headlight_uniform`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuSpot {
    /// Camera-relative lamp midpoint, w half the lamps' separation.
    pub position: [f32; 4],
    /// Right, w the projector's x scale.
    pub right: [f32; 4],
    /// Up, w its y scale.
    pub up: [f32; 4],
    /// Forward, w unused.
    pub dir: [f32; 4],
    /// Colour × fade.
    pub colour: [f32; 4],
}

/// One lamp (`Point` in car_lights.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuPoint {
    /// Camera-relative position, w the kind (0 head, 1 tail, 2 brake).
    pub position: [f32; 4],
    /// Colour × fade; w the air's density at the lamp, set by `CarLightsPass::run` ([`air_at`]).
    pub colour: [f32; 4],
    /// Facing, w unused.
    pub facing: [f32; 4],
}

/// The cones and lamps of `cars` seen from `eye` (camera-relative), the
/// nearest first, faded out over the last 20 % of [`AIR_REACH`]; lamps
/// behind the eye's plane by more than their reach are left out.
pub fn air_lights(cars: &[Car], eye: Vec3, forward: Vec3) -> (Vec<GpuSpot>, Vec<GpuPoint>) {
    let mut order: Vec<(f32, &Car)> = cars
        .iter()
        .map(|c| (c.position().distance(eye), c))
        .filter(|(d, c)| *d < AIR_REACH && (c.position() - eye).dot(forward) > -45.0)
        .collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut spots = Vec::new();
    let mut points = Vec::new();
    for (d, car) in order {
        let fade = 1.0 - ((d - 0.8 * AIR_REACH) / (0.2 * AIR_REACH)).clamp(0.0, 1.0);
        let heads: Vec<&Light> = car.lights.iter().filter(|l| l.kind == Kind::Head).collect();
        if spots.len() < MAX_SPOTS && !heads.is_empty() {
            let h = crate::render::headlight_uniform(Some(&crate::render::HeadlightPool {
                transform: car.transform,
                color: car.head_colour,
            }));
            let mid = heads.iter().map(|l| l.position).sum::<Vec3>() / heads.len() as f32;
            let sep = heads.iter().map(|l| (l.position - mid).length()).fold(0.0f32, f32::max);
            let rel = mid - eye;
            spots.push(GpuSpot {
                position: [rel.x, rel.y, rel.z, sep],
                right: [h[1][0], h[1][1], h[1][2], h[1][3]],
                up: [h[2][0], h[2][1], h[2][2], h[2][3]],
                dir: [h[3][0], h[3][1], h[3][2], 0.0],
                colour: [h[4][0] * fade, h[4][1] * fade, h[4][2] * fade, 0.0],
            });
        }
        for l in &car.lights {
            if points.len() >= MAX_POINTS {
                break;
            }
            let rel = l.position - eye;
            let c = l.colour * fade;
            let kind = match l.kind {
                Kind::Head => 0.0,
                Kind::Tail => 1.0,
                Kind::Brake => 2.0,
            };
            points.push(GpuPoint {
                position: [rel.x, rel.y, rel.z, kind],
                colour: [c.x, c.y, c.z, 0.0],
                facing: [l.facing.x, l.facing.y, l.facing.z, 0.0],
            });
        }
    }
    (spots, points)
}

/// The air's value-noise lattice is a periodic texture (NOISE³ texels, R8, repeat): the hash
/// of each lattice point (car_lights.wgsl's former `hash3`) quantised to 8 bits. fs_march reads it
/// with one filtered sample per octave at the smooth weights (`noise3`) instead of 8 hashes.
pub const NOISE: usize = 32;

/// The hash at lattice point `i` (0..NOISE each), as stored.
pub(in crate::render) fn lattice_hash(i: Vec3) -> f32 {
    fn fract(v: Vec3) -> Vec3 {
        v - v.floor()
    }
    let mut q = fract(i * 0.1031);
    q += Vec3::splat(q.dot(Vec3::new(q.z, q.y, q.x) + 31.32));
    let v = (q.x + q.y) * q.z;
    ((v - v.floor()) * 255.0).round() / 255.0
}

/// The lattice texture's texels (x fastest).
pub fn noise_texels() -> Vec<u8> {
    let mut out = Vec::with_capacity(NOISE * NOISE * NOISE);
    for z in 0..NOISE {
        for y in 0..NOISE {
            for x in 0..NOISE {
                out.push((lattice_hash(Vec3::new(x as f32, y as f32, z as f32)) * 255.0).round() as u8);
            }
        }
    }
    out
}

/// car_lights.wgsl's `air` (the density noise at a world point, time `t`) in f32: the point lights'
/// value, made on the CPU, on the periodic lattice (the GPU's filtered sample differs by
/// its weights' precision only). `mix(a, b, w)` as WGSL's a·(1 − w) + b·w.
pub fn air_at(p: Vec3, t: f32) -> f32 {
    fn mix(a: f32, b: f32, w: f32) -> f32 {
        a * (1.0 - w) + b * w
    }
    fn noise3(p: Vec3) -> f32 {
        let i = p.floor();
        let f = p - i;
        let w = f * f * (Vec3::splat(3.0) - 2.0 * f);
        let n = NOISE as f32;
        let h = |x: f32, y: f32, z: f32| lattice_hash((i + Vec3::new(x, y, z)).rem_euclid(Vec3::splat(n)));
        let a = mix(mix(h(0.0, 0.0, 0.0), h(1.0, 0.0, 0.0), w.x), mix(h(0.0, 1.0, 0.0), h(1.0, 1.0, 0.0), w.x), w.y);
        let b = mix(mix(h(0.0, 0.0, 1.0), h(1.0, 0.0, 1.0), w.x), mix(h(0.0, 1.0, 1.0), h(1.0, 1.0, 1.0), w.x), w.y);
        mix(a, b, w.z)
    }
    let q = p * 0.22 + Vec3::new(t * 0.05, t * 0.03, t * 0.08);
    let n = noise3(q) * 0.65 + noise3(q * 2.7 + Vec3::new(5.2, 1.3, t * 0.2)) * 0.35;
    0.55 + 0.9 * n
}

/// The air's density for the rain (0..1): the dry air's × the shared medium's thickness (one
/// medium, `volumes::medium`; was its own × (1 + 1.5 rain)).
pub fn air_density(look: &Look, rain: f32) -> f32 {
    look.density * crate::render::enh::volumes::medium::medium(rain).thickness
}

/// car_lights.wgsl's CORE and EXTINCTION (the halo's softened core, m;
/// the air's extinction length, m).
pub const CORE: f32 = 0.18;
pub const EXTINCTION: f32 = 60.0;

/// car_lights.wgsl's `point_halo` on the CPU: lamp `p`'s halo
/// along the view ray `dir` (unit, camera-relative) out to `len`, before
/// the air's density and the native fog. `gain` is [`Look::point`].
pub fn point_halo(p: &GpuPoint, dir: Vec3, len: f32, gain: f32, g: f32) -> Vec3 {
    let c = Vec3::new(p.position[0], p.position[1], p.position[2]);
    let t0 = c.dot(dir);
    let h2 = (c.dot(c) - t0 * t0).max(0.0);
    if h2 > HALO * HALO {
        return Vec3::ZERO;
    }
    let a = (t0 - HALO).max(0.0);
    let b = (t0 + HALO).min(len);
    if b <= a {
        return Vec3::ZERO;
    }
    let hs = (h2 + CORE * CORE).sqrt();
    let integral = (((b - t0) / hs).atan() - ((a - t0) / hs).atan()) / hs;
    let q = dir * t0.clamp(a, b) - c;
    let qn = q * q.dot(q).max(1e-6).sqrt().recip();
    let f = Vec3::new(p.facing[0], p.facing[1], p.facing[2]).dot(qn);
    let smooth = |lo: f32, hi: f32, x: f32| {
        let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let lobe =
        if p.position[3] < 0.5 { 0.05 + 0.95 * smooth(0.0, 0.85, f) } else { 0.25 + 0.75 * smooth(-0.3, 0.7, f) };
    let g2 = g * g;
    let phase = (1.0 - g2) / (1.0 + g2 - 2.0 * g * qn.dot(-dir)).max(1e-4).powf(1.5);
    let ph = 1.0 + (phase - 1.0) * 0.6;
    let w = 1.0 - h2 / (HALO * HALO);
    let kind_gain = if (p.position[3] - 1.0).abs() < 0.5 { 0.5 } else { 1.0 };
    Vec3::new(p.colour[0], p.colour[1], p.colour[2])
        * (kind_gain * integral * lobe * ph * w * w * gain * (-t0.max(0.0) / EXTINCTION).exp() * p.colour[3])
}

/// The view's sample directions for [`halo_veils`] (NDC grid).
const VEIL_GRID: (usize, usize) = (12, 9);

/// Per lamp the factor on its halo (`colour.w`) that
/// keeps the halos' summed light, averaged over the screen, within
/// [`Look::veil`]. Each lamp's share (its halo's mean luminance over a
/// 12 × 9 grid of view rays, `clip_to_rel`: clip → camera-relative, each ray
/// unoccluded to 400 m and without the fog, so the estimate is high, never
/// low) is capped at one level, the same for every lamp, chosen so the caps
/// sum to the budget: the lamps right at the camera, whose halos veil the
/// whole view, are dimmed; a lamp whose halo is a small glow keeps it. All
/// 1 while the sum stays under the budget.
pub fn halo_veils(points: &[GpuPoint], clip_to_rel: Mat4, density: f32, look: &Look) -> Vec<f32> {
    let mut out = vec![1.0; points.len()];
    if points.is_empty() || look.veil <= 0.0 {
        return out;
    }
    let (nx, ny) = VEIL_GRID;
    let rays: Vec<Vec3> = (0..nx * ny)
        .map(|i| {
            let x = ((i % nx) as f32 + 0.5) / nx as f32 * 2.0 - 1.0;
            let y = ((i / nx) as f32 + 0.5) / ny as f32 * 2.0 - 1.0;
            let p = clip_to_rel * glam::Vec4::new(x, y, 0.5, 1.0);
            (p.truncate() / p.w).normalize_or(Vec3::X)
        })
        .collect();
    let luma = Vec3::new(0.2126, 0.7152, 0.0722);
    let veil: Vec<f32> = points
        .iter()
        .map(|p| {
            rays.iter().map(|d| point_halo(p, *d, 400.0, look.point, look.g).dot(luma)).sum::<f32>() * density
                / rays.len() as f32
        })
        .collect();
    if veil.iter().sum::<f32>() <= look.veil {
        return out;
    }
    // The cap k with Σ min(veil, k) = budget (bisection; the sum rises with k).
    let (mut lo, mut hi) = (0.0f32, veil.iter().copied().fold(0.0, f32::max));
    for _ in 0..24 {
        let k = 0.5 * (lo + hi);
        if veil.iter().map(|v| v.min(k)).sum::<f32>() > look.veil {
            hi = k;
        } else {
            lo = k;
        }
    }
    for (o, v) in out.iter_mut().zip(&veil) {
        if *v > lo {
            *o = lo / v;
        }
    }
    out
}

/// One volume: `frame`'s tail and brake lamps for the froxel medium (Volumetric Fog Next-Gen), which then
/// holds the car lights' air ([`CarLightsPass::run`] with `air` false): world position, colour (× the
/// tail's half share of the air and the halos' summed bound, [`halo_veils`], as the march had them),
/// facing; the nearest first. The headlights' own halos stay out: looking into a lamp, its glare is the
/// native glow sprite's and the lens core's; the beam in the air is the medium's spot.
pub fn medium_lamps(frame: &Frame) -> Vec<(Vec3, Vec3, Vec3)> {
    medium_lamps_of(&frame.points, &frame.camera, frame.aspect, frame.reversed, frame.rain)
}

/// [`medium_lamps`] for `points` (camera-relative, as [`air_lights`] makes them) seen by `cam`.
pub fn medium_lamps_of(
    points: &[GpuPoint],
    cam: &Camera,
    aspect: f32,
    reversed: bool,
    rain: f32,
) -> Vec<(Vec3, Vec3, Vec3)> {
    let look = look();
    let rot = Mat4::look_to_rh(Vec3::ZERO, cam.forward(), cam.up);
    let clip_to_rel = (cam.projection_depth(aspect, reversed) * rot).inverse();
    let points: Vec<GpuPoint> = points
        .iter()
        .take(MAX_POINTS)
        .map(|p| GpuPoint { colour: [p.colour[0], p.colour[1], p.colour[2], 1.0], ..*p })
        .collect();
    let veils = halo_veils(&points, clip_to_rel, air_density(&look, rain), &look);
    points
        .iter()
        .zip(veils)
        .filter(|(p, _)| p.position[3] > 0.5)
        .map(|(p, v)| {
            let tail = if p.position[3] < 1.5 { 0.5 } else { 1.0 };
            (
                Vec3::new(p.position[0], p.position[1], p.position[2]) + cam.position,
                Vec3::new(p.colour[0], p.colour[1], p.colour[2]) * (tail * v),
                Vec3::new(p.facing[0], p.facing[1], p.facing[2]),
            )
        })
        .collect()
}

/// A frame's inputs for [`CarLightsPass::run`].
#[derive(Clone)]
pub struct Frame {
    pub depth: wgpu::TextureView,
    pub samples: u32,
    /// Unjittered (as Volumetric Fog's).
    pub camera: Camera,
    pub aspect: f32,
    pub reversed: bool,
    pub fog: Option<[f32; 3]>,
    pub rain: f32,
    pub time: f32,
    pub spots: Vec<GpuSpot>,
    pub points: Vec<GpuPoint>,
    /// HEADLIGHTS (the projector's cookie) and its sampler.
    pub cookie: (wgpu::TextureView, wgpu::Sampler),
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(in crate::render) struct Uniforms {
    clip_to_rel: [[f32; 4]; 4],
    rel_to_clip: [[f32; 4]; 4],
    /// Scene size, 1 / size.
    size: [f32; 4],
    /// The depth texel a fragment marches to = pos.xy · xy + zw.
    map: [f32; 4],
    /// density, spots, points, reversed
    params: [f32; 4],
    /// FogValue (far, 1 / (far − start), minimum), on
    fog: [f32; 4],
    /// spot gain, point gain, g, time
    gains: [f32; 4],
    /// the eye (world), streak gain
    eye: [f32; 4],
    /// The point lights' tile grid (tiles across, tile size in reduced pixels, 0, 0)
    tiles: [f32; 4],
    /// The aureole's cosines ([`Look::aureole`]), 0, 0
    shape: [f32; 4],
}

/// car_lights.wgsl's HALO: a lamp's halo reach (m).
pub const HALO: f32 = 6.0;

/// The reduced-size pixels a tile of the point lights' list covers each way.
pub const TILE: u32 = 16;

/// Per tile of the reduced-size march (`TILE` pixels square, row-major,
/// `tiles` = (across, down)) the point lights whose halo can reach a pixel in
/// it, as a 64-bit mask (bit i = point i). Conservative: a point is left out
/// of a tile only where fs_march's `h2 > HALO²` / `b ≤ a` rejects it for every
/// pixel, so the sum (in the same ascending order) is bit-identical. The halo
/// reaches a ray whose line passes within `halo` of the lamp (h² ≤ halo²) with
/// its nearest point less than `halo` behind the eye: with the lamp more than
/// `halo` in front of the eye every such ray meets the sphere (lamp, halo) in
/// front of the eye, so its pixel lies in that sphere's projection, bounded by
/// its view-space box's corners; a lamp `2·halo` or more behind the eye plane
/// reaches no ray; anything in between gets every tile.
pub fn point_tiles(points: &[Vec3], view: Mat4, proj: Mat4, low: (u32, u32), halo: f32) -> (Vec<[u32; 2]>, (u32, u32)) {
    let tiles = (low.0.div_ceil(TILE).max(1), low.1.div_ceil(TILE).max(1));
    let mut masks = vec![[0u32; 2]; (tiles.0 * tiles.1) as usize];
    for (i, &p) in points.iter().enumerate().take(MAX_POINTS) {
        let bit = (i / 32, 1u32 << (i % 32));
        let v = view.transform_point3(p);
        // View space looks down −z.
        let z = -v.z;
        if z <= -2.0 * halo {
            continue;
        }
        let rect = if z > halo + 1e-3 {
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for k in 0..8 {
                let c = v + Vec3::new(
                    if k & 1 == 0 { -halo } else { halo },
                    if k & 2 == 0 { -halo } else { halo },
                    if k & 4 == 0 { -halo } else { halo },
                );
                let h = proj * c.extend(1.0);
                if h.w <= 1e-4 {
                    // (z > halo: every corner is in front; kept for safety.)
                    x0 = f32::MIN;
                    break;
                }
                let (sx, sy) = ((h.x / h.w * 0.5 + 0.5) * low.0 as f32, (0.5 - h.y / h.w * 0.5) * low.1 as f32);
                x0 = x0.min(sx);
                y0 = y0.min(sy);
                x1 = x1.max(sx);
                y1 = y1.max(sy);
            }
            if x0 == f32::MIN { None } else { Some((x0, y0, x1, y1)) }
        } else {
            None
        };
        let (tx0, ty0, tx1, ty1) = match rect {
            // One tile of margin (the reduced pixel's centre maps into the full-size depth).
            Some((x0, y0, x1, y1)) => {
                let t = |v: f32, n: u32| ((v / TILE as f32).floor() as i64).clamp(-1, i64::from(n)) as i32;
                let (a, b, c, d) = (t(x0, tiles.0) - 1, t(y0, tiles.1) - 1, t(x1, tiles.0) + 1, t(y1, tiles.1) + 1);
                if c < 0 || d < 0 || a >= tiles.0 as i32 || b >= tiles.1 as i32 {
                    continue;
                }
                (a.max(0) as u32, b.max(0) as u32, (c as u32).min(tiles.0 - 1), (d as u32).min(tiles.1 - 1))
            }
            None => (0, 0, tiles.0 - 1, tiles.1 - 1),
        };
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                masks[(ty * tiles.0 + tx) as usize][bit.0] |= bit.1;
            }
        }
    }
    (masks, tiles)
}

pub(in crate::render) const HALF_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The reduced size's divisor: at most 720 rows marched, at least half
/// size (1080p ½, 4K ⅓ as volume_fx's march, 4K with SSAA 2x ⅙: the
/// scattered light is smooth; the upsample keeps the edges).
pub fn divisor(height: u32) -> u32 {
    height.div_ceil(720).max(2)
}

pub struct CarLightsPass {
    /// The scene format the pipelines draw into (remade when High
    /// Precision Post switches it live).
    pub format: wgpu::TextureFormat,
    layouts: [wgpu::BindGroupLayout; 2],
    march: [wgpu::RenderPipeline; 2],
    up: [wgpu::RenderPipeline; 2],
    streak: [wgpu::RenderPipeline; 2],
    uniform: wgpu::Buffer,
    spots: wgpu::Buffer,
    points: wgpu::Buffer,
    /// The point lights' tile masks ([`point_tiles`]) and their capacity (tiles).
    tiles: (wgpu::Buffer, usize),
    /// The air's noise lattice ([`noise_texels`]) and its repeating sampler.
    noise: (wgpu::TextureView, wgpu::Sampler),
    low: Option<((u32, u32), wgpu::TextureView)>,
    dummy: wgpu::TextureView,
}

pub(in crate::render) fn begin_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    label: &str,
    view: &wgpu::TextureView,
    clear: bool,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: if clear { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) } else { wgpu::LoadOp::Load },
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

pub(in crate::render) fn low_target(gpu: &Gpu, (w, h): (u32, u32)) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("car lights air"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HALF_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// car_lights.wgsl for single (false) or multisampled (true) depth.
pub fn shader_source(multisampled: bool) -> String {
    let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/car_lights.wgsl"));
    if multisampled { source.replace("texture_depth_2d", "texture_depth_multisampled_2d") } else { source.to_string() }
}

impl CarLightsPass {
    pub fn new(gpu: &Gpu, scene_format: wgpu::TextureFormat) -> CarLightsPass {
        let device = &gpu.device;
        let frag = wgpu::ShaderStages::FRAGMENT;
        let both = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let storage = |binding: u32, size: usize| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: both,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(size as u64),
            },
            count: None,
        };
        let layout = |multisampled: bool| {
            crate::render::core::device_cache::bind_group_layout(gpu, &wgpu::BindGroupLayoutDescriptor {
                label: Some("car lights"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: both,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: both,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniforms>() as u64),
                        },
                        count: None,
                    },
                    storage(2, std::mem::size_of::<GpuSpot>() * MAX_SPOTS),
                    storage(3, std::mem::size_of::<GpuPoint>() * MAX_POINTS),
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: frag,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: frag,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: frag,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: frag,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(8),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        visibility: frag,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
                        visibility: frag,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            })
        };
        let layouts = [layout(false), layout(true)];
        let add = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let shaders = [false, true].map(|ms| {
            crate::render::core::device_cache::module(gpu, wgpu::ShaderModuleDescriptor {
                label: Some("car lights"),
                source: wgpu::ShaderSource::Wgsl(shader_source(ms).into()),
            })
        });
        let pipeline = |ms: bool,
                        vs: &str,
                        fs: &str,
                        format: wgpu::TextureFormat,
                        blend: Option<wgpu::BlendState>,
                        mask: wgpu::ColorWrites| {
            let shader = &shaders[usize::from(ms)];
            let pl = crate::render::core::device_cache::pipeline_layout(gpu, &wgpu::PipelineLayoutDescriptor {
                label: Some("car lights"),
                bind_group_layouts: &[Some(&layouts[usize::from(ms)])],
                immediate_size: 0,
            });
            crate::render::core::device_cache::render_pipeline(gpu, &wgpu::RenderPipelineDescriptor {
                label: Some("car lights"),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: mask })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: gpu.disk_pipeline_cache(),
            })
        };
        let march =
            [false, true].map(|ms| pipeline(ms, "vs_full", "fs_march", HALF_FORMAT, None, wgpu::ColorWrites::ALL));
        let up =
            [false, true].map(|ms| pipeline(ms, "vs_full", "fs_up", scene_format, Some(add), wgpu::ColorWrites::COLOR));
        let streak = [false, true]
            .map(|ms| pipeline(ms, "vs_streak", "fs_streak", scene_format, Some(add), wgpu::ColorWrites::COLOR));
        let buffer = |label: &str, size: usize, usage: wgpu::BufferUsages| {
            crate::render::zeroed_buffer(
                device,
                &wgpu::BufferDescriptor {
                    label: Some(label),
                    size: size as u64,
                    usage: usage | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            )
        };
        CarLightsPass {
            format: scene_format,
            layouts,
            march,
            up,
            streak,
            uniform: buffer("car lights", std::mem::size_of::<Uniforms>(), wgpu::BufferUsages::UNIFORM),
            spots: buffer("car lights spots", std::mem::size_of::<GpuSpot>() * MAX_SPOTS, wgpu::BufferUsages::STORAGE),
            points: buffer(
                "car lights points",
                std::mem::size_of::<GpuPoint>() * MAX_POINTS,
                wgpu::BufferUsages::STORAGE,
            ),
            tiles: (buffer("car lights tiles", 8, wgpu::BufferUsages::STORAGE), 1),
            noise: {
                use wgpu::util::DeviceExt;
                let n = NOISE as u32;
                let view = device
                    .create_texture_with_data(
                        &gpu.queue,
                        &wgpu::TextureDescriptor {
                            label: Some("car lights air noise"),
                            size: wgpu::Extent3d { width: n, height: n, depth_or_array_layers: n },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D3,
                            format: wgpu::TextureFormat::R8Unorm,
                            usage: wgpu::TextureUsages::TEXTURE_BINDING,
                            view_formats: &[],
                        },
                        wgpu::util::TextureDataOrder::LayerMajor,
                        &noise_texels(),
                    )
                    .create_view(&Default::default());
                let repeat = wgpu::AddressMode::Repeat;
                let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("car lights air noise"),
                    address_mode_u: repeat,
                    address_mode_v: repeat,
                    address_mode_w: repeat,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    ..Default::default()
                });
                (view, sampler)
            },
            low: None,
            dummy: low_target(gpu, (1, 1)),
        }
    }

    /// Add `frame`'s car lights in the air and the lens streaks into
    /// `scene` (`size` big, resolved, before the native screen passes).
    /// `air` false: the lens streaks only; the froxel medium holds the cones
    /// and lamps in the air (one volume, [`medium_lamps`]).
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::TextureView,
        size: (u32, u32),
        frame: &Frame,
        air: bool,
    ) {
        if frame.spots.is_empty() && frame.points.is_empty() {
            return;
        }
        let look = look();
        let cam = &frame.camera;
        let rot = Mat4::look_to_rh(Vec3::ZERO, cam.forward(), cam.up);
        let rel_to_clip = cam.projection_depth(frame.aspect, frame.reversed) * rot;
        let k = divisor(size.1);
        let low = (size.0.div_ceil(k).max(1), size.1.div_ceil(k).max(1));
        if self.low.as_ref().is_none_or(|(s, _)| *s != low) {
            self.low = Some((low, low_target(gpu, low)));
        }
        let n_spots = frame.spots.len().min(MAX_SPOTS);
        let n_points = frame.points.len().min(MAX_POINTS);
        // The point lights' halos binned per tile of the reduced size (exact: fs_march loops only
        // over the lamps that can reach the tile, in the same order).
        let rel: Vec<Vec3> =
            frame.points[..n_points].iter().map(|p| Vec3::new(p.position[0], p.position[1], p.position[2])).collect();
        let (masks, tiles) = point_tiles(&rel, rot, cam.projection_depth(frame.aspect, frame.reversed), low, HALO);
        if self.tiles.1 < masks.len() {
            self.tiles = (
                crate::render::zeroed_buffer(
                    &gpu.device,
                    &wgpu::BufferDescriptor {
                        label: Some("car lights tiles"),
                        size: (masks.len() * 8) as u64,
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    },
                ),
                masks.len(),
            );
        }
        gpu.queue.write_buffer(&self.tiles.0, 0, bytemuck::cast_slice(&masks));
        let u = Uniforms {
            clip_to_rel: rel_to_clip.inverse().to_cols_array_2d(),
            rel_to_clip: rel_to_clip.to_cols_array_2d(),
            size: [size.0 as f32, size.1 as f32, 1.0 / size.0.max(1) as f32, 1.0 / size.1.max(1) as f32],
            map: [size.0 as f32 / low.0 as f32, size.1 as f32 / low.1 as f32, 0.0, 0.0],
            params: [
                air_density(&look, frame.rain),
                n_spots as f32,
                n_points as f32,
                if frame.reversed { 1.0 } else { 0.0 },
            ],
            fog: match frame.fog {
                Some(f) => [f[0], f[1], f[2], 1.0],
                None => [0.0; 4],
            },
            gains: [look.spot, look.point, look.g, frame.time],
            eye: [
                cam.position.x,
                cam.position.y,
                cam.position.z,
                look.streak * (1.0 + 0.6 * frame.rain.clamp(0.0, 1.0)),
            ],
            tiles: [tiles.0 as f32, TILE as f32, 0.0, 0.0],
            shape: [look.aureole.0, look.aureole.1, 0.0, 0.0],
        };
        gpu.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        if n_spots > 0 {
            gpu.queue.write_buffer(&self.spots, 0, bytemuck::cast_slice(&frame.spots[..n_spots]));
        }
        if n_points > 0 {
            // For speed: each lamp's air density (the halo's `air` at the lamp, the same for every pixel) once
            // here instead of per pixel and lamp (−3.9 ms of the pass's 13.9 on the reference integrated GPU, 1080p).
            let mut points: Vec<GpuPoint> = frame.points[..n_points]
                .iter()
                .map(|p| {
                    let mut p = *p;
                    p.colour[3] =
                        air_at(Vec3::new(p.position[0], p.position[1], p.position[2]) + cam.position, frame.time);
                    p
                })
                .collect();
            // The halos' summed light bounded (the streaks read colour.rgb, not w).
            let veils = halo_veils(&points, rel_to_clip.inverse(), u.params[0], &look);
            for (p, v) in points.iter_mut().zip(veils) {
                p.colour[3] *= v;
            }
            gpu.queue.write_buffer(&self.points, 0, bytemuck::cast_slice(&points));
        }
        let ms = usize::from(frame.samples > 1);
        let group = |low_tex: &wgpu::TextureView| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("car lights"),
                layout: &self.layouts[ms],
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&frame.depth) },
                    wgpu::BindGroupEntry { binding: 1, resource: self.uniform.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.spots.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: self.points.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(low_tex) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&frame.cookie.0) },
                    wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Sampler(&frame.cookie.1) },
                    wgpu::BindGroupEntry { binding: 7, resource: self.tiles.0.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(&self.noise.0) },
                    wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::Sampler(&self.noise.1) },
                ],
            })
        };
        let target = self.low.as_ref().unwrap().1.clone();
        if air {
            let mut p = begin_pass(encoder, "car lights: air (reduced size)", &target, true);
            p.set_pipeline(&self.march[ms]);
            p.set_bind_group(0, &group(&self.dummy), &[]);
            p.draw(0..3, 0..1);
        }
        {
            let mut p = begin_pass(encoder, "car lights: air (upsample) and lens streaks", scene, false);
            p.set_bind_group(0, &group(&target), &[]);
            if air {
                p.set_pipeline(&self.up[ms]);
                p.draw(0..3, 0..1);
            }
            if u.eye[3] > 0.0 && n_points > 0 {
                p.set_pipeline(&self.streak[ms]);
                p.draw(0..6, 0..n_points as u32);
            }
        }
        crate::prof::gpu::mark(encoder, "graphics: car lights");
    }
}
