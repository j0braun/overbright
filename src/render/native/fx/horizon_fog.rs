//! Horizon fog (verified unless marked): the 10 × 10 screen grid that
//! 0x5C7480 adds over the far scene first thing in the end-of-frame chain
//! (0x5CB81C), its colours sampled from the track's horizon fog sources.
//! See docs/formats.md "Horizon fog".
//!
//! * Sources: chunk 0x34250 of the track file (version 2 at +8, count at
//!   +0xC, 0x64-byte records from +0x10 of the 16-aligned body). 0x5E01A0
//!   links each record into list `type` of the eight at 0x88F1C8; type 6
//!   (0x88F1F8) is the horizon fog's. A record: colour +0x0C…+0x14
//!   (0–255 floats), position +0x20…+0x28, full distance +0x2C, start
//!   distance +0x34.
//! * 0x5DB910 (per frame, main view): sorts the list nearest first (merge
//!   sort 0x43E650 / 0x43BF40 on 0x5D74B0's camera distance), projects each
//!   source to screen pixels (0x5BC4B0), and weighs it: the cosine between
//!   the camera's direction (+0x50) and the way to the source; when that is
//!   positive, 0 nearer than the start, 1 beyond the full distance, linear
//!   between. Each source also gets the "Horizon Fog Visibility" map's
//!   field 0 at the camera (0x88A2C0, smoothed by 0x5FA670 over 20 units;
//!   1 when the map is not loaded).
//! * 0x5DBB10 (per grid vertex): front to back, `f = (128 − 0.45·d) / 128`
//!   from the octagonal pixel distance `d = max + min / 4`, a source with
//!   `f ≥ 0` adds `0.5·colour·f·w·(1 − A)` and `A += f·w`. The sums are
//!   scaled by `3.24 · visibility` (1.8 · v · 1.8) and clamped to 255.
//! * 0x5C7480: vertex (i, row) at clip (−1 + 2i/9, −1 + 2·row/9),
//!   z = 0.9995 − 0.0001·row, sampled at pixel (i·(W div 9),
//!   (9 − row)·(H div 9)); alpha 0 below row 5, else
//!   ftol((0.1·row − 0.4)·425). Identity transforms, FVF 0x142, indices
//!   from 0x5BB760 (two triangles a quad, diagonal b → b + 11).

use glam::{Mat4, Vec2, Vec3};

use crate::formats::bundle::{self, read_f32, read_u32};
use crate::formats::hash::bin_hash;

/// The light-and-fog record chunk (0x5E01A0).
pub const CHUNK: u32 = 0x0003_4250;
/// 0x5E01A0 links only version 2.
const VERSION: u32 = 2;
const RECORD: usize = 0x64;
/// The horizon fog's list (0x88F1C8 + 6 · 8).
const TYPE: u32 = 6;

/// 0x5DB910's map smoothing distance (0x41A00000).
const BLEND_DISTANCE: f32 = 20.0;
const VISIBILITY_MAP: &str = "Horizon Fog Visibility";

/// The grid (0x5C7480).
pub const SIDE: usize = 10;
/// 0x5BB760's 0xA2 triangles.
pub const INDEX_COUNT: usize = 9 * 9 * 6;

/// One horizon fog source (a type-6 record of chunk 0x34250).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    /// +0x0C…+0x14, 0–255.
    pub colour: [f32; 3],
    /// +0x20…+0x28.
    pub position: Vec3,
    /// +0x2C: from here on the weight is 1.
    pub full: f32,
    /// +0x34: nearer than this the weight is 0.
    pub start: f32,
}

/// The track file's horizon fog sources, in file order.
pub fn parse_sources(track_bun: &[u8]) -> Vec<Source> {
    let mut out = Vec::new();
    bundle::walk(bundle::chunks(track_bun, 0), &mut |c| {
        if c.id != CHUNK || c.is_container() {
            return;
        }
        // (body + 0xF) & ~0xF, as 0x5E01AF does with the header + 0x17.
        let b = c.aligned(16);
        if b.len() < 0x10 || read_u32(b, 8) != VERSION {
            return;
        }
        let count = read_u32(b, 0xC) as usize;
        for k in 0..count {
            let r = 0x10 + k * RECORD;
            if r + RECORD > b.len() {
                break;
            }
            if read_u32(b, r + 8) != TYPE {
                continue;
            }
            out.push(Source {
                colour: [read_f32(b, r + 0xC), read_f32(b, r + 0x10), read_f32(b, r + 0x14)],
                position: Vec3::new(read_f32(b, r + 0x20), read_f32(b, r + 0x24), read_f32(b, r + 0x28)),
                full: read_f32(b, r + 0x2C),
                start: read_f32(b, r + 0x34),
            });
        }
    });
    out
}

/// One grid vertex (FVF 0x142: position, D3DCOLOR; the UV is unused, see
/// the docs).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    /// A, R, G, B bytes as 0–1.
    pub colour: [f32; 4],
}

/// The camera for one frame.
pub struct View {
    pub position: Vec3,
    /// Camera +0x50: the unit view direction.
    pub forward: Vec3,
    /// World to clip (0x5BC4B0's matrix).
    pub view_projection: Mat4,
    /// The screen (0x870980 × 0x870984).
    pub size: (u32, u32),
}

/// 0x5BC4B0: a world point to screen pixels.
fn project(vp: Mat4, p: Vec3, (w, h): (u32, u32)) -> Vec2 {
    let c = vp * p.extend(1.0);
    let r = 1.0 / c.w;
    Vec2::new((c.x * r + 1.0) * w as f32 * 0.5, (c.y * r - 1.0) * h as f32 * -0.5)
}

/// 0x5DB910's weight of one source.
fn weight(s: &Source, view: &View) -> f32 {
    let to = s.position - view.position;
    let distance = to.length();
    let cos = to.normalize_or_zero().dot(view.forward);
    if cos <= 0.0 {
        return cos;
    }
    if distance < s.start {
        0.0
    } else if distance >= s.full {
        1.0
    } else {
        (distance - s.start) / (s.full - s.start)
    }
}

/// A source as 0x5DBB10 reads it.
struct Placed {
    colour: [f32; 3],
    screen: (i32, i32),
    weight: f32,
}

/// 0x5DBB10: the colour (0–255 each) at pixel (x, y).
fn sample(sources: &[Placed], visibility: f32, x: i32, y: i32) -> [i32; 3] {
    let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for s in sources.iter().filter(|s| s.weight > 0.0) {
        let dx = (x - s.screen.0).abs();
        let dy = (y - s.screen.1).abs();
        let d = if dx > dy { (dy >> 2) + dx } else { (dx >> 2) + dy };
        let f = (128.0 - d as f32 * 0.45) * 0.0078125;
        if f < 0.0 {
            continue;
        }
        let fw = f * s.weight;
        let t = 1.0 - a;
        if t >= 0.0 {
            r += s.colour[0] * 0.5 * fw * t;
            g += s.colour[1] * 0.5 * fw * t;
            b += s.colour[2] * 0.5 * fw * t;
        }
        a += fw;
    }
    if a > 0.0 {
        let k = 1.8 * visibility * 1.8;
        r = (r * k).min(255.0);
        g = (g * k).min(255.0);
        b = (b * k).min(255.0);
    }
    [r as i32, g as i32, b as i32]
}

/// 0x5C7480's alpha for grid row `row` (0 at the bottom).
pub fn row_alpha(row: usize) -> u8 {
    let f = row as f32 * 0.1;
    if f >= 0.4 { ((f - 0.4) * 425.0) as i32 as u8 } else { 0 }
}

/// 0x5BB760's index list.
pub fn indices() -> Vec<u16> {
    let mut out = Vec::with_capacity(INDEX_COUNT);
    for row in 0..9u16 {
        for i in 0..9u16 {
            let b = row * 10 + i;
            out.extend_from_slice(&[b, b + 10, b + 11, b + 11, b + 1, b]);
        }
    }
    out
}

/// The visibility map's smoothed sample and the sorted sources.
#[derive(Clone, Default)]
pub struct HorizonFog {
    visibility: Option<f32>,
    last: Vec2,
}

impl HorizonFog {
    /// The whole state as bits (tests).
    #[cfg(test)]
    fn dev_bits_raw(&self) -> [u32; 3] {
        [self.visibility.map_or(u32::MAX, f32::to_bits), self.last.x.to_bits(), self.last.y.to_bits()]
    }

    /// Dev harness (rate gate, `state_hash` `vis`): the smoothed visibility as bits.
    #[cfg(feature = "dev-harness")]
    pub fn dev_bits(&self) -> u32 {
        self.visibility.map_or(u32::MAX, f32::to_bits)
    }

    /// 0x5DB910's visibility (field 0 of "Horizon Fog Visibility",
    /// smoothed at the camera; 1 without the map).
    fn visibility(&mut self, world: &crate::world::World, at: Vec2) -> f32 {
        let Some(map) = world.parameter_maps.get(&bin_hash(VISIBILITY_MAP)) else { return 1.0 };
        let target = map.sample(at).first().copied().unwrap_or(0.0);
        let v = match self.visibility {
            None => target,
            Some(old) => {
                let moved = at.distance(self.last);
                let t = if moved < BLEND_DISTANCE { moved / BLEND_DISTANCE } else { 1.0 };
                t * target + (1.0 - t) * old
            }
        };
        self.visibility = Some(v);
        self.last = at;
        v
    }

    /// The frame's 100 vertices, row by row from the bottom; `None` when
    /// the track has no horizon fog sources (nothing is added). The
    /// per-frame path over the tick form: one [`HorizonFog::step`] at the
    /// view's position, then the grid at α = 0.
    pub fn grid(&mut self, world: &crate::world::World, view: &View) -> Option<Vec<Vertex>> {
        let v = self.step(world, view.position.truncate());
        grid(&world.horizon_fog, HorizonFog::drawn(v, v, 0.0), view)
    }

    /// Tick form (frame-rate independence): one tick of 0x5DB910's visibility at the
    /// **tick** camera's ground position `at` (smoothed by t = min(|p_n −
    /// p_{n−1}| / 20, 1); 1 without the map). -> S_n.
    pub fn step(&mut self, world: &crate::world::World, at: Vec2) -> f32 {
        self.visibility(world, at)
    }

    /// The visibility a frame at α between ticks draws with (`prev`
    /// exactly at α = 0); the grid itself is a pure per-frame build
    /// ([`grid`]) from the drawn camera.
    pub fn drawn(prev: f32, cur: f32, alpha: f32) -> f32 {
        super::tick::lerp(prev, cur, alpha)
    }
}

/// The grid for these sources at this visibility.
pub fn grid(sources: &[Source], visibility: f32, view: &View) -> Option<Vec<Vertex>> {
    if sources.is_empty() {
        return None;
    }
    let mut sorted: Vec<&Source> = sources.iter().collect();
    sorted.sort_by(|a, b| a.position.distance(view.position).total_cmp(&b.position.distance(view.position)));
    let placed: Vec<Placed> = sorted
        .iter()
        .map(|s| {
            let p = project(view.view_projection, s.position, view.size);
            Placed { colour: s.colour, screen: (p.x as i32, p.y as i32), weight: weight(s, view) }
        })
        .collect();
    let (step_x, step_y) = ((view.size.0 / 9) as i32, (view.size.1 / 9) as i32);
    let mut out = Vec::with_capacity(SIDE * SIDE);
    for row in 0..SIDE {
        let alpha = row_alpha(row) as f32 / 255.0;
        for i in 0..SIDE {
            let [r, g, b] = sample(&placed, visibility, i as i32 * step_x, (9 - row) as i32 * step_y);
            let byte = |v: i32| (v & 0xFF) as f32 / 255.0;
            out.push(Vertex {
                position: [
                    -1.0 + i as f32 * 0.222_222_22,
                    -1.0 + row as f32 * 0.222_222_22,
                    0.9995 - row as f32 * 0.0001,
                ],
                colour: [alpha, byte(r), byte(g), byte(b)],
            });
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The per-frame visibility before the tick form (verbatim), the oracle.
    fn legacy_visibility(s: &mut HorizonFog, world: &crate::world::World, at: Vec2) -> f32 {
        let Some(map) = world.parameter_maps.get(&bin_hash(VISIBILITY_MAP)) else { return 1.0 };
        let target = map.sample(at).first().copied().unwrap_or(0.0);
        let v = match s.visibility {
            None => target,
            Some(old) => {
                let moved = at.distance(s.last);
                let t = if moved < BLEND_DISTANCE { moved / BLEND_DISTANCE } else { 1.0 };
                t * target + (1.0 - t) * old
            }
        };
        s.visibility = Some(v);
        s.last = at;
        v
    }

    /// 600 ticks stepped = 600 frames of the per-frame code, bit for bit;
    /// the grid at α = 0 is the old grid; between ticks the visibility lies
    /// within the two snapshots.
    #[test]
    fn tick_form_is_the_per_frame_path() {
        use crate::render::native::fx::tick::{testkit, within};
        let vis: [&[f32]; 4] = [&[0.2], &[1.0], &[0.65], &[0.05]];
        let mut world = testkit::world(&[(VISIBILITY_MAP, vis)]);
        world.horizon_fog = vec![
            Source { colour: [200.0, 100.0, 50.0], position: Vec3::new(0.0, 900.0, 10.0), full: 600.0, start: 300.0 },
            Source { colour: [20.0, 140.0, 250.0], position: Vec3::new(500.0, 700.0, 0.0), full: 400.0, start: 100.0 },
        ];
        let (mut new, mut old) = (HorizonFog::default(), HorizonFog::default());
        let mut prev = None;
        for i in 0..600 {
            let p = testkit::path(i);
            let view = View {
                position: p.extend(3.0),
                forward: Vec3::new((i as f32 * 0.01).sin(), (i as f32 * 0.01).cos(), 0.0),
                view_projection: Mat4::perspective_rh(1.0, 1.333, 0.5, 5000.0)
                    * Mat4::look_to_rh(p.extend(3.0), Vec3::Y, Vec3::Z),
                size: (640, 480),
            };
            let v_old = legacy_visibility(&mut old, &world, p);
            let want = grid(&world.horizon_fog, v_old, &view);
            assert_eq!(new.clone().grid(&world, &view), want, "frame {i}");
            let cur = new.step(&world, p);
            assert_eq!(cur.to_bits(), v_old.to_bits(), "tick {i}");
            assert_eq!(new.dev_bits_raw(), old.dev_bits_raw());
            if let Some(prev) = prev {
                assert_eq!(HorizonFog::drawn(prev, cur, 0.0).to_bits(), f32::to_bits(prev));
                for a in testkit::ALPHAS {
                    assert!(within(HorizonFog::drawn(prev, cur, a), prev, cur));
                }
            }
            prev = Some(cur);
        }
        // Without the map: 1, nothing smoothed.
        let mut none = HorizonFog::default();
        assert_eq!(none.step(&crate::world::World::default(), Vec2::ZERO), 1.0);
    }

    #[test]
    fn rows_fade_in_above_the_middle() {
        let a: Vec<u8> = (0..10).map(row_alpha).collect();
        assert_eq!(a, [0, 0, 0, 0, 0, 42, 85, 127, 170, 212]);
    }

    #[test]
    fn indices_cover_the_grid() {
        let i = indices();
        assert_eq!(i.len(), 0xA2 * 3);
        assert_eq!(&i[..6], &[0, 10, 11, 11, 1, 0]);
        assert_eq!(*i.iter().max().unwrap(), 99);
    }

    #[test]
    fn a_source_ahead_tints_the_vertex_over_it() {
        let view = View { position: Vec3::ZERO, forward: Vec3::Y, view_projection: Mat4::IDENTITY, size: (900, 900) };
        // Straight ahead (weight 1), but the identity "projection" puts it
        // at y = (1 − 1000)·−450, far off the screen.
        let s =
            Source { colour: [200.0, 100.0, 50.0], position: Vec3::new(0.0, 1000.0, 0.0), full: 600.0, start: 300.0 };
        let g = grid(&[s], 1.0, &view).unwrap();
        assert_eq!(g.len(), 100);
        // Every vertex stays black.
        assert!(g.iter().all(|v| v.colour[1] == 0.0));
        // Directly at a vertex: f = 1, w = 1, 0.5 · c · (1.8f)² (3.2399998,
        // so 50 → 161.99999, truncated), red clamped to 255.
        let placed = [Placed { colour: [200.0, 100.0, 50.0], screen: (0, 0), weight: 1.0 }];
        assert_eq!(sample(&placed, 1.0, 0, 0), [255, 161, 80]);
        // Out of reach (d · 0.45 > 128).
        assert_eq!(sample(&placed, 1.0, 300, 0), [0, 0, 0]);
    }

    #[test]
    fn weight_ramps_between_start_and_full() {
        let view = View { position: Vec3::ZERO, forward: Vec3::Y, view_projection: Mat4::IDENTITY, size: (640, 480) };
        let at = |d: f32| Source { colour: [0.0; 3], position: Vec3::new(0.0, d, 0.0), full: 600.0, start: 300.0 };
        assert_eq!(weight(&at(200.0), &view), 0.0);
        assert_eq!(weight(&at(450.0), &view), 0.5);
        assert_eq!(weight(&at(700.0), &view), 1.0);
        assert!(weight(&Source { position: Vec3::new(0.0, -700.0, 0.0), ..at(0.0) }, &view) < 0.0);
    }
}

crate::persist!(Source { colour, position, full, start });
