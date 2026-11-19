//! A streaming section's light pack (container 0x80135000, loaded by
//! 0x48E0B0's 0x80135000 case): the street lamps and other lights that
//! light the cars (0x48E3D0), with a box tree to find the ones near a
//! point.
//!
//! * 0x135001 header (16-byte aligned): +0x08 u16 version 4, +0x0C the
//!   section number, +0x1C the light count.
//! * 0x135002 tree (16-byte aligned): +0x00 a pointer the loader points at
//!   +0x10, where 0x30-byte nodes start: f32 centre x, y, z at +0x00, i16
//!   +0x0E the child count (> 0) or minus the light count (a leaf), f32
//!   half extents at +0x10, i16 child or light indices from +0x1C.
//! * 0x135003 lights (16-byte aligned), 0x60 bytes each: u8 kind +0x04
//!   (0 ambient, 2 point, 3 spot), u8 +0x05, u8 on +0x07 (1), colour
//!   +0x0C (bytes red, green, blue), f32 position +0x10, outer radius
//!   +0x1C, spot direction +0x20, intensity +0x2C, inner radius +0x30,
//!   spot cone +0x34.

use glam::Vec3;

use crate::formats::bundle::Chunk;
use crate::formats::reader::{FormatResult, Reader};

fn vec3(r: &Reader<'_>, at: usize) -> FormatResult<Vec3> {
    r.vec3_at(at).map(Vec3::from)
}

pub const LIGHT_PACK: u32 = 0x8013_5000;

#[derive(Clone, Debug)]
pub struct WorldLight {
    pub kind: u8,
    pub sub: u8,
    pub on: bool,
    /// Red, green, blue (0–255).
    pub colour: [u8; 3],
    pub position: Vec3,
    pub outer: f32,
    pub direction: Vec3,
    pub intensity: f32,
    pub inner: f32,
    pub cone: f32,
}

#[derive(Clone, Debug)]
struct Node {
    centre: Vec3,
    half: Vec3,
    /// > 0: children; ≤ 0: minus the lights listed.
    count: i16,
    indices: Vec<u16>,
}

#[derive(Clone, Debug, Default)]
pub struct LightPack {
    pub section: u32,
    nodes: Vec<Node>,
    pub lights: Vec<WorldLight>,
}

/// The chunk body from its first 16-byte boundary (the loader's
/// `(header + 0x17) & ~0xF`).
fn aligned<'a>(c: &Chunk<'a>) -> &'a [u8] {
    let pad = (16 - c.offset % 16) % 16;
    &c.body[pad.min(c.body.len())..]
}

impl LightPack {
    pub fn parse(container: Chunk) -> LightPack {
        let mut pack = LightPack::default();
        for c in container.children() {
            let mut b = Reader::new(aligned(&c));
            match c.id {
                0x135001 if b.len() >= 0x10 => pack.section = b.u32_at(0xC).unwrap_or(pack.section),
                0x135002 if b.len() >= 0x10 => {
                    let node = |n: Reader<'_>| -> FormatResult<Node> {
                        let count = n.i16_at(0xE)?;
                        let listed = (count.unsigned_abs() as usize).min((0x30 - 0x1C) / 2);
                        Ok(Node {
                            centre: vec3(&n, 0)?,
                            half: vec3(&n, 0x10)?,
                            count,
                            indices: (0..listed).map(|i| n.u16_at(0x1C + i * 2)).collect::<FormatResult<_>>()?,
                        })
                    };
                    // Whole 0x30-byte nodes: none of the reads can fail.
                    pack.nodes = b.skip(0x10).map_or_else(|_| Vec::new(), |()| b.whole_records(0x30).filter_map(|n| node(n).ok()).collect());
                }
                0x135003 => {
                    let light = |r: Reader<'_>| -> FormatResult<WorldLight> {
                        let colour = r.u32_at(0xC)?;
                        Ok(WorldLight {
                            kind: r.u8_at(4)?,
                            sub: r.u8_at(5)?,
                            on: r.u8_at(7)? == 1,
                            colour: [colour as u8, (colour >> 8) as u8, (colour >> 16) as u8],
                            position: vec3(&r, 0x10)?,
                            outer: r.f32_at(0x1C)?,
                            direction: vec3(&r, 0x20)?,
                            intensity: r.f32_at(0x2C)?,
                            inner: r.f32_at(0x30)?,
                            cone: r.f32_at(0x34)?,
                        })
                    };
                    pack.lights = b.whole_records(0x60).filter_map(|r| light(r).ok()).collect();
                }
                _ => {}
            }
        }
        pack
    }

    fn inside(node: &Node, p: Vec3) -> bool {
        let d = (p - node.centre).abs();
        d.x < node.half.x && d.y < node.half.y && d.z < node.half.z
    }

    /// The leaf that holds `p` (0x5747F0): from the root, the first child
    /// whose box holds it, depth first, down to a node without children.
    pub fn leaf_lights(&self, p: Vec3) -> Option<impl Iterator<Item = &WorldLight>> {
        let root = self.nodes.first()?;
        if !Self::inside(root, p) {
            return None;
        }
        // Work budget: a tree visits each node at most once, so a walk that
        // needs more calls than twice the node count is a cycle (or a DAG
        // blown up by shared children) in a hostile file; the native routine
        // would loop forever there. Valid data never gets near it.
        let mut budget = self.nodes.len() * 2 + 16;
        let leaf = if root.count < 1 { Some(root) } else { self.descend(root, p, 0, &mut budget) }?;
        Some(leaf.indices.iter().filter_map(|&i| self.lights.get(i as usize)))
    }

    /// 0x574740.
    fn descend(&self, node: &Node, p: Vec3, depth: usize, budget: &mut usize) -> Option<&Node> {
        if depth > 32 || *budget == 0 {
            return None;
        }
        *budget -= 1;
        for &c in node.indices.iter().take(node.count.max(0) as usize) {
            let Some(child) = self.nodes.get(c as usize) else { continue };
            if Self::inside(child, p) {
                if child.count < 1 {
                    return Some(child);
                }
                if let Some(found) = self.descend(child, p, depth + 1, budget) {
                    return Some(found);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(centre: f32, half: f32, count: i16, indices: &[u16]) -> Node {
        Node { centre: Vec3::splat(centre), half: Vec3::splat(half), count, indices: indices.to_vec() }
    }

    /// A node naming itself ten times is 10^32 calls to the depth limit;
    /// the walk now ends (without a leaf) almost at once.
    #[test]
    fn self_referencing_node_is_bounded() {
        let pack = LightPack { nodes: vec![node(0.0, 10.0, 10, &[0; 10])], ..Default::default() };
        assert!(pack.leaf_lights(Vec3::ZERO).is_none());
    }

    /// Valid trees: the first matching leaf in depth-first order.
    #[test]
    fn first_leaf_in_depth_first_order() {
        let light = WorldLight { kind: 2, sub: 0, on: true, colour: [0; 3], position: Vec3::ZERO, outer: 1.0, direction: Vec3::ZERO, intensity: 1.0, inner: 0.0, cone: 0.0 };
        let pack = LightPack {
            nodes: vec![node(0.0, 10.0, 2, &[1, 2]), node(0.0, 5.0, -1, &[0]), node(0.0, 5.0, -1, &[1])],
            lights: vec![light.clone(), light],
            ..Default::default()
        };
        let got: Vec<_> = pack.leaf_lights(Vec3::ZERO).unwrap().collect();
        assert_eq!(got.len(), 1);
        assert!(std::ptr::eq(got[0], &pack.lights[0]));
    }
}

/// A streaming section's flare pack (container 0x80135100, loaded by
/// 0x48E0B0's 0x80135100 case; the section's record 0x883EC0[section]
/// keeps it at +0x08): the glow sprites of the street lamps, drawn by
/// 0x491860 (see `nfsu2::glow`).
///
/// * 0x135101 header (16-byte aligned): +0x08 u32 version (the loader
///   drops any but 3), +0x30 the box's minimum, +0x40 its maximum (the
///   frustum test 0x5BC310), +0x50 u16 the flare count, +0x52 a loaded
///   flag, +0x54 the section number, +0x58 the list head.
/// * 0x135102 flares (16-byte aligned), 0x30 bytes each, linked into the
///   list at +0x00 / +0x04: +0x08 a name hash, +0x10 the position, +0x1C
///   the reflection's ground cache (999 until 0x490920 fills it), +0x20
///   the direction, +0x2C the type (index into 0x7F3CC8), +0x2D flags.
pub const FLARE_PACK: u32 = 0x8013_5100;

#[derive(Clone, Copy, Debug)]
pub struct Flare {
    pub position: Vec3,
    pub direction: Vec3,
    pub kind: u8,
    pub flags: u8,
}

#[derive(Clone, Debug, Default)]
pub struct FlarePack {
    pub section: u32,
    pub min: Vec3,
    pub max: Vec3,
    pub flares: Vec<Flare>,
}

impl FlarePack {
    /// `None` for a pack the loader would drop (no header, version ≠ 3).
    pub fn parse(container: Chunk) -> Option<FlarePack> {
        let mut pack = None;
        let mut count = 0;
        for c in container.children() {
            let b = Reader::new(aligned(&c));
            match c.id {
                0x135101 if b.len() >= 0x58 => {
                    if b.u32_at(8).ok()? != 3 {
                        return None;
                    }
                    count = b.u16_at(0x50).ok()? as usize;
                    pack = Some(FlarePack { section: b.u32_at(0x54).ok()?, min: vec3(&b, 0x30).ok()?, max: vec3(&b, 0x40).ok()?, flares: Vec::new() });
                }
                0x135102 => {
                    let Some(p) = pack.as_mut() else { continue };
                    let flare = |r: Reader<'_>| -> FormatResult<Flare> {
                        Ok(Flare { position: vec3(&r, 0x10)?, direction: vec3(&r, 0x20)?, kind: r.u8_at(0x2C)?, flags: r.u8_at(0x2D)? })
                    };
                    p.flares = b.whole_records(0x30).take(count).filter_map(|r| flare(r).ok()).collect();
                }
                _ => {}
            }
        }
        pack
    }
}

crate::persist!(LightPack { section, nodes, lights });
crate::persist!(Node { centre, half, count, indices });
crate::persist!(WorldLight { kind, sub, on, colour, position, outer, direction, intensity, inner, cone });
crate::persist!(FlarePack { section, min, max, flares });
crate::persist!(Flare { position, direction, kind, flags });
