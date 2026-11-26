//! The game's particle effects (the emitter module 0x48B000..0x493FFF):
//! emitter descriptions from the bundles, the spawner 0x491E50, the
//! per-frame update 0x492E10 and the renderer 0x48BDF0's quads.
//!
//! * Descriptions: chunk `0x35021` (GLOBALB 92, a track bundle its own, L4RA
//!   37), version 4 at body +8, the count at +0xC, then 0x220-byte records
//!   from +0x10 (0x491C70 links each into the list 0x830D08 and resolves
//!   the texture hash +0xC through 0x4901D0 into +0x14). Placed emitters:
//!   chunk `0x35020` (GLOBALB only), version 6, 0xD0-byte records (their
//!   description hash +0xC is resolved by 0x48BC00 into +0x1C, then
//!   0x48BC50 queues each on the manager 0x8357E0).
//! * An emitter's description hash at +0x10 is its *group*: the name the
//!   contact effects (0x5E0630) and the props' attached effects (0x48B8B0)
//!   gather emitters by — `XECS_SOLID_WALL` is `XECS_SolidWall_GlowA` and
//!   `XECS_SolidWall_SprksA`, `XECI_CAR` `XECI_Car_SmokeA` and
//!   `XECI_Car_SprksA`, the tyres' `XETT_ROAD_SK` `XETT_RoadSk_SmkA`, the
//!   nitrous `CARFX_NITRO` `XES_NOS_FlameA` (one system for all of them).
//! * Particles are 0x20 bytes from a pool of 1000 (300 on one hardware
//!   class, 0x488750; the clone keeps 1000), packed: life left in 1/1024 s
//!   (i16 +8), the angle (u16 +0xA, bit 0 the flip), the speed in 1/128 m/s
//!   (u16 +0xC), the carried share (u8 +0xE), orientation | aspect << 4
//!   (+0xF), the position (+0x10), the direction in 1/127 (i8 ×3 +0x1C)
//!   and the flip-book frame (+0x1F). The clone keeps the same
//!   quantisation.
//!
// prov: inferred particles.module
//! **Inferred** (not bit-exact): the random draws (the game steps one
//! xorshift per emitter, seeded from a global one through `_ftol`; the
//! clone draws one step per value), the direction distributions (below),
//! the group pool (400 groups, 0x488730) which the clone does not bound.

use glam::{Mat4, Vec3};

mod between;
mod color;
mod contact_fx;
mod emitter;
mod ground_fx;
mod rng;
mod spawn;
mod system;

pub use between::{between, JUMP as BETWEEN_JUMP};
pub use color::*;
pub use contact_fx::*;
pub use emitter::*;
pub use ground_fx::*;
pub use rng::*;
use spawn::*;
pub use system::*;

use crate::formats::bundle::{self, read_f32, read_u32};

/// Chunk id of the emitter descriptions.
pub const DESCRIPTION_CHUNK: u32 = 0x35021;
/// One description record.
const DESCRIPTION_SIZE: usize = 0x220;
/// The particle pool (0x488750: 0x3E8 entries of 0x20 bytes).
pub const MAX_PARTICLES: usize = 1000;
/// Life is kept in 1/1024 s (0x78A194 = 1024).
const TICKS: f32 = 1024.0;
/// The renderer's on-screen size bound when a description has none (+0x8C
/// = 0), and the cap for any (0x48C2C0: 512).
const MAX_PIXELS: f32 = 512.0;
/// The aspect table 0x7F3944 (w, h) by the description's +0x1F (0..3; the
/// fourth pair is not floats in the EXE, the clone uses 1 × 1).
const ASPECT: [[f32; 2]; 4] = [[1.0, 1.0], [1.0, 0.5], [0.5, 1.0], [1.0, 1.0]];

/// A cubic in the age fraction t (0x4881D0): four rows of four, `row0·t³ +
/// row1·t² + row2·t + row3`, one column per channel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cubic(pub [[f32; 4]; 4]);

impl Cubic {
    pub fn at(&self, t: f32) -> [f32; 4] {
        let m = &self.0;
        let (t2, t3) = (t * t, t * t * t);
        std::array::from_fn(|c| m[0][c] * t3 + m[1][c] * t2 + m[2][c] * t + m[3][c])
    }
}

/// An emitter description (the 0x220-byte record; offsets in the record).
#[derive(Clone, Debug, Default)]
pub struct Description {
    /// +0x180, and its hash at +8.
    pub name: String,
    pub hash: u32,
    /// +0xC: the texture's hash (+0x1C0 its name).
    pub texture: u32,
    /// +0x10: the group hash (+0x1F0 its name).
    pub group: u32,
    /// +0x18: 0 jitters the emitter's z axis by ±(+0x50)/2 about each
    /// axis, else a cone of half angle +0x50 (see `direction`).
    pub spherical: bool,
    /// +0x19 / +0x1A: the flip-book's columns and rows, as powers of two.
    pub columns_log2: u8,
    pub rows_log2: u8,
    /// +0x1B: flip-book frames per second (0: no animation).
    pub fps: u8,
    /// +0x1C: start each particle on a random frame.
    pub random_frame: bool,
    /// +0x1D: flip half the particles' spin.
    pub random_flip: bool,
    /// +0x1E: the particles are carried by the emitter's owner each frame
    /// rather than taking its velocity at birth.
    pub carried: bool,
    /// +0x1F: the aspect (0..3, 0x7F3944).
    pub aspect: u8,
    /// +0x20 (low nibble): 0 faces the camera, 1 lies in the world's XY
    /// plane, 2 XZ, 3 YZ; non-zero also puts the particles in the second
    /// list, drawn as quads (0x5C7E90) after the first (0x5C0E20).
    pub orientation: u8,
    /// +0x21: spawns at low detail too.
    pub low_detail: bool,
    /// +0x24/+0x28: on time and its spread; +0x2C/+0x30: off time.
    pub on_time: [f32; 2],
    pub off_time: [f32; 2],
    /// +0x34/+0x38: particles per unit of emission and the spread fraction.
    pub rate: [f32; 2],
    /// +0x3C/+0x40: life in seconds, spread.
    pub life: [f32; 2],
    /// +0x44/+0x48: speed, spread.
    pub speed: [f32; 2],
    /// +0x4C: the start angle's range, degrees.
    pub spin_range: f32,
    /// +0x50: the cone's half angle, degrees.
    pub cone: f32,
    /// +0x54/+0x58: the share of the emitter's velocity taken, spread.
    pub inherit: [f32; 2],
    /// +0x60: the box's centre, +0x70 its size (emitter space).
    pub offset: Vec3,
    pub extent: Vec3,
    /// +0x80: no spawning beyond this distance from the view.
    pub range: f32,
    /// +0x84: gravity (down, m/s²); +0x88: quadratic drag.
    pub gravity: f32,
    pub drag: f32,
    /// +0x8C: the on-screen size bound, pixels (0: 512).
    pub max_pixels: f32,
    /// +0x100: colour (R, G, B, A, 0..255) over the age fraction.
    pub color: Cubic,
    /// +0x140: column 0 the size (m), column 1 the spin (65536 a turn).
    pub size: Cubic,
    /// +0x5C: the car effect marker group its car emitters burst at
    /// (0x615CD0: render +0x548 + 8·group; 28 the purge's hood markers,
    /// 29 its wheel points, 10 the exhausts).
    pub marker_group: u32,
    /// +0x90: the colour keys' times (× 1/3 by 0x48B920), +0xA0 the four
    /// RGBA keys the +0x100 cubic runs through.
    pub key_times: [f32; 4],
    pub key_colors: [[f32; 4]; 4],
}

fn cubic(r: &[u8], at: usize) -> Cubic {
    Cubic(std::array::from_fn(|row| std::array::from_fn(|c| read_f32(r, at + row * 16 + c * 4))))
}

impl Description {
    pub fn parse(r: &[u8]) -> Description {
        let f = |o| read_f32(r, o);
        let v = |o| Vec3::new(f(o), f(o + 4), f(o + 8));
        Description {
            name: bundle::read_cstr(r, 0x180, 0x40),
            hash: read_u32(r, 8),
            texture: read_u32(r, 0xC),
            group: read_u32(r, 0x10),
            spherical: r[0x18] == 0,
            columns_log2: r[0x19],
            rows_log2: r[0x1A],
            fps: r[0x1B],
            random_frame: r[0x1C] != 0,
            random_flip: r[0x1D] != 0,
            carried: r[0x1E] != 0,
            aspect: (r[0x1F] as i8).clamp(0, 3) as u8,
            orientation: r[0x20] & 0xF,
            low_detail: r[0x21] != 0,
            on_time: [f(0x24), f(0x28)],
            off_time: [f(0x2C), f(0x30)],
            rate: [f(0x34), f(0x38)],
            life: [f(0x3C), f(0x40)],
            speed: [f(0x44), f(0x48)],
            spin_range: f(0x4C),
            cone: f(0x50),
            inherit: [f(0x54), f(0x58)],
            offset: v(0x60),
            extent: v(0x70),
            range: f(0x80),
            gravity: f(0x84),
            drag: f(0x88),
            max_pixels: f(0x8C),
            color: cubic(r, 0x100),
            size: cubic(r, 0x140),
            marker_group: read_u32(r, 0x5C),
            key_times: std::array::from_fn(|i| f(0x90 + i * 4)),
            key_colors: std::array::from_fn(|k| std::array::from_fn(|c| f(0xA0 + k * 16 + c * 4))),
        }
    }

    /// The flip-book's frame count.
    pub fn frames(&self) -> u32 {
        (1u32 << self.columns_log2.min(7)) * (1u32 << self.rows_log2.min(7))
    }
}

/// Every description of a bundle (`0x35021` chunks, version 4).
pub fn parse_descriptions(data: &[u8]) -> Vec<Description> {
    let mut out = Vec::new();
    bundle::walk(bundle::chunks(data, 0), &mut |c| {
        if c.id != DESCRIPTION_CHUNK {
            return;
        }
        let b = c.aligned(16);
        if b.len() < 16 || read_u32(b, 8) != 4 {
            return;
        }
        let count = read_u32(b, 0xC) as usize;
        for r in b[16..].chunks_exact(DESCRIPTION_SIZE).take(count) {
            out.push(Description::parse(r));
        }
    });
    out
}

/// The descriptions the game has loaded (0x830D08: GLOBALB's, then the
/// track's), by group.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub descriptions: Vec<Description>,
    /// The loaded emitters a car effect clones (0x48B8B0 walks the
    /// manager's list 0x8357E8 for those whose description is in a
    /// group): GLOBALB's emitter records (chunk 0x35020, state 3 — never
    /// run themselves), by description name hash, with their matrix.
    pub templates: Vec<(u32, Mat4)>,
}

impl Library {
    pub fn add(&mut self, data: &[u8]) {
        self.descriptions.extend(parse_descriptions(data));
    }

    /// GLOBALB's emitter records (chunk 0x35020) as car effect templates.
    pub fn add_templates(&mut self, data: &[u8]) {
        bundle::walk(bundle::chunks(data, 0), &mut |c| {
            if c.id == PLACED_EMITTERS {
                self.templates.extend(parse_placed(c.aligned(16)).into_iter().map(|p| (p.description, p.transform)));
            }
        });
    }

    /// 0x48B8B0: the emitters of a description group, cloned from the
    /// loaded ones in list order: (description, the emitter's matrix).
    pub fn group_templates(&self, group: u32) -> Vec<(usize, Mat4)> {
        if group == 0 {
            return Vec::new();
        }
        self.templates
            .iter()
            .filter_map(|&(h, m)| self.by_hash(h).filter(|&d| self.descriptions[d].group == group).map(|d| (d, m)))
            .collect()
    }

    /// The descriptions in a group, in list order (0x48B8B0 / 0x5E0630).
    pub fn group(&self, hash: u32) -> Vec<usize> {
        self.descriptions.iter().enumerate().filter(|(_, d)| d.group == hash).map(|(i, _)| i).collect()
    }

    pub fn by_name(&self, name: &str) -> Option<usize> {
        self.descriptions.iter().position(|d| d.name == name)
    }

    /// The first description in load order whose name hash (+0x08) is
    /// `hash` (0x48BC00, walking the list at 0x830D08).
    pub fn by_hash(&self, hash: u32) -> Option<usize> {
        if hash == 0 {
            return None;
        }
        self.descriptions.iter().position(|d| d.hash == hash)
    }
}

/// The track sections' placed emitters (chunk 0x35020).
pub const PLACED_EMITTERS: u32 = 0x35020;

/// An emitter placed in a track section. 0x491C70 reads a 0x35020 chunk
/// whose version (aligned +8) is 6: from aligned +0x10, as many emitter
/// instances as aligned +0xC, 0xD0 bytes each, used as they are. Each is
/// bound to the description its +0x0C names (0x48BC00, into +0x1C) and
/// started (0x48BC50); 0x491D50 unlinks them when the section goes.
#[derive(Clone, Debug)]
pub struct Placed {
    /// +0x0C: the description's name hash.
    pub description: u32,
    /// +0x10: 1 emitting, 0 waiting; any other state is filed apart by
    /// 0x48BC50 (state 2) and never runs.
    pub state: u32,
    /// +0x2C: the switch that must be held for it to run (0x492E10 →
    /// 0x5D7450, the list at 0x88A28C); 0 runs always. The shops' markers
    /// hold `FREE_ROAM`'s hash.
    pub switch: u32,
    /// +0x40: its matrix.
    pub transform: Mat4,
}

/// A 0x35020 chunk's emitters; `aligned` is its body from the first
/// 16-byte boundary (0x491C70's `+0x17 & ~0xF`).
pub fn parse_placed(aligned: &[u8]) -> Vec<Placed> {
    if aligned.len() < 0x10 || read_u32(aligned, 8) != 6 {
        return Vec::new();
    }
    let count = read_u32(aligned, 0xC) as usize;
    aligned[0x10..]
        .chunks_exact(0xD0)
        .take(count)
        .map(|r| Placed {
            description: read_u32(r, 0xC),
            state: read_u32(r, 0x10),
            switch: read_u32(r, 0x2C),
            transform: Mat4::from_cols_array(&std::array::from_fn(|i| read_f32(r, 0x40 + 4 * i))),
        })
        .collect()
}

/// One textured quad to draw (the renderer's 0x5C7E90 / 0x5C0E20 input).
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    pub texture: u32,
    pub corners: [Vec3; 4],
    pub uv: [[f32; 2]; 4],
    pub rgba: [u8; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_matches_keys() {
        // XEBO_ROAD_DirtA01's colour: 255 → 70 in red, 250 → 0 in alpha.
        let c = Cubic([
            [85.92, 0.0, 0.0, -682.2],
            [-22.29, 0.0, 0.0, 1028.0],
            [-248.6, 0.0, 0.0, -596.0],
            [255.0, 0.0, 0.0, 250.0],
        ]);
        let end = c.at(1.0);
        assert!((end[0] - 70.0).abs() < 0.1 && end[3].abs() < 0.5);
        assert_eq!(c.at(0.0)[0], 255.0);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn shares() {
        assert_eq!(share(10.0, 10.0, 15.0), None);
        assert_eq!(share(12.5, 10.0, 15.0), Some(0.5));
        assert_eq!(share(99.0, 10.0, 15.0), Some(1.0));
        assert_eq!(pair_surface(CAR_SURFACE, 20), 20);
        assert_eq!(pair_surface(CAR_SURFACE, CAR_SURFACE), CAR_SURFACE);
    }

    #[test]
    fn recolor_fits_the_keys_and_shifts_hue() {
        // XES_NOSBO_*'s keys (GLOBALB): the unshifted cubic is the stored
        // +0x100 one (147.19 at t = 0.5 for R).
        let mut d = Description { key_times: [0.0, 1.0, 2.0, 3.0], ..Default::default() };
        d.key_colors = [
            [130.0, 130.0, 135.0, 40.0],
            [140.0, 140.0, 150.0, 150.0],
            [150.0, 150.0, 160.0, 80.0],
            [125.0, 125.0, 125.0, 0.0],
        ];
        let c = recolor(&d, 0.0, 0.0, 0.0);
        assert!(
            (c.at(0.0)[0] - 130.0).abs() < 0.05
                && (c.at(0.5)[0] - 147.1875).abs() < 0.05
                && (c.at(1.0)[3]).abs() < 0.05,
            "{:?}",
            c.at(0.5)
        );
        // Full saturation, a third of a turn: grey-blue goes saturated, alpha kept.
        let c = recolor(&d, 0.33, 1.0, 0.0);
        let k = c.at(1.0 / 3.0);
        assert!((k[3] - 150.0).abs() < 0.05 && (k[0] - k[1]).abs() > 50.0, "{k:?}");
    }

    #[test]
    fn camera_sprites_stand_upright_and_do_not_spin() {
        // A marker icon (XEE_*_IconA_01): camera-facing, aspect 1 (1 × 0.5),
        // 0x5C0E20 / 0x5BB440: u along right, v along down, no angle.
        let mut d = Description { life: [10.0, 0.0], aspect: 1, spin_range: 360.0, range: 100.0, ..Default::default() };
        d.size = Cubic([[0.0; 4], [0.0; 4], [0.0; 4], [2.0, 90.0, 0.0, 0.0]]);
        d.color = Cubic([[0.0; 4], [0.0; 4], [0.0; 4], [255.0; 4]]);
        let lib = Library { descriptions: vec![d], ..Default::default() };
        let mut sys = System::default();
        let mut e = Emitter::new(0, Mat4::IDENTITY, 7);
        sys.emit(&lib, &mut e, -1.0);
        let q = sys.quads(&lib, Vec3::new(0.0, -5.0, 0.0), Vec3::X, Vec3::Z, Vec3::Y, 500.0);
        let q = &q[0];
        let at = sys.particles[0].position;
        // The texture's top left (u0, v0) at −right + up, its width (1)
        // across, its height (0.5) up.
        let tl = q.uv.iter().position(|uv| *uv == [0.0, 0.0]).unwrap();
        assert!((q.corners[tl] - (at + Vec3::new(-1.0, 0.0, 0.5))).length() < 1e-4);
        let br = q.uv.iter().position(|uv| *uv == [1.0, 1.0]).unwrap();
        assert!((q.corners[br] - (at + Vec3::new(1.0, 0.0, -0.5))).length() < 1e-4);
    }

    #[test]
    fn burst_counts_and_falls() {
        let mut d = Description {
            life: [1.0, 0.0],
            speed: [2.0, 0.0],
            gravity: 10.0,
            cone: 10.0,
            range: 100.0,
            ..Default::default()
        };
        d.size = Cubic([[0.0; 4], [0.0; 4], [0.0; 4], [1.0, 0.0, 0.0, 0.0]]);
        d.color = Cubic([[0.0; 4], [0.0; 4], [0.0; 4], [255.0; 4]]);
        let lib = Library { descriptions: vec![d], ..Default::default() };
        let mut sys = System::default();
        let mut e = Emitter::new(0, Mat4::IDENTITY, 7);
        // Rate 0 steps 0.1: a full share gives the first particle and
        // nine more.
        sys.emit(&lib, &mut e, -1.0);
        assert_eq!(sys.particles.len(), 10);
        let z0 = sys.particles[0].position.z;
        for _ in 0..30 {
            sys.update(&lib, 1.0 / 60.0, |_| None);
        }
        assert!(sys.particles[0].position.z < z0 + 1.0);
        assert!(sys.particles[0].velocity().z < 2.0 - 4.0);
        for _ in 0..40 {
            sys.update(&lib, 1.0 / 60.0, |_| None);
        }
        assert!(sys.particles.is_empty());
        let q = sys.quads(&lib, Vec3::new(0.0, -5.0, 0.0), Vec3::X, Vec3::Z, Vec3::Y, 500.0);
        assert!(q.is_empty());
    }
}

crate::persist!(Library { descriptions, templates });
crate::persist!(Description {
    name,
    hash,
    texture,
    group,
    spherical,
    columns_log2,
    rows_log2,
    fps,
    random_frame,
    random_flip,
    carried,
    aspect,
    orientation,
    low_detail,
    on_time,
    off_time,
    rate,
    life,
    speed,
    spin_range,
    cone,
    inherit,
    offset,
    extent,
    range,
    gravity,
    drag,
    max_pixels,
    color,
    size,
    marker_group,
    key_times,
    key_colors
});
crate::persist!(Placed { description, state, switch, transform });
impl crate::cache::Persist for Cubic {
    fn put(&self, w: &mut crate::cache::Writer) {
        crate::cache::Persist::put(&self.0, w)
    }
    fn get(r: &mut crate::cache::Reader) -> crate::cache::Got<Self> {
        crate::cache::Persist::get(r).map(Cubic)
    }
}
