//! Parameter maps: named tables of values that vary across the city ("Normal
//! Fog", "Night Normal Fog", "Horizon Fog Visibility", …). They live in the
//! track's resident file (`L4RA.BUN`) under container 0x8003B600, one
//! 0x8003B601 per map:
//!
//! ```text
//! 0x0003B602 header   u32 name hash, f32 min x, min y, max x, max y,
//!                     u32 node count, u32 value count, u32 stride, u32 fields
//! 0x0003B603 types    u32 per field (0 = float)
//! 0x0003B604 offsets  u32 byte offset per field inside a value row
//! 0x0003B605 values   value count × stride bytes
//! 0x0003B608 tree     node count × [u16; 4] quadtree (absent: one value)
//! 0x0003B607 tree     node count × [u8; 4], the same quadtree with byte
//!                     entries ("Night Raining Fog"). 0x5FA3C0 keeps it at
//!                     map +0x18 (0x3B608 at +0x1C); 0x5FA4F0 walks +0x18
//!                     with 0x5F0940 when present, else +0x1C with
//!                     0x5F0A40: the same halving and child order
//! ```
//!
//! Lookup (SPEED2.EXE 0x5F0A40 for 0x3B608, 0x5F0940 for 0x3B607): start at node 0 with the header box. A node
//! whose first entry is 0 is a leaf, and its second entry is the value row.
//! Otherwise descend into child `[0]` (x < mid, y < mid), `[1]` (x < mid,
//! y ≥ mid), `[2]` (x ≥ mid, y < mid) or `[3]` (x ≥ mid, y ≥ mid), halving
//! the box. Verified on all 13 Bayview maps: node counts match the header,
//! every child index is in range, every leaf value index is below the value
//! count.

use std::collections::HashMap;

use anyhow::{Context, Result};
use glam::Vec2;

use super::bundle::{self, Chunk};
use super::reader::{FormatResult, Limits, Reader};

pub const PARAMETER_MAPS: u32 = 0x8003_B600;
const MAP: u32 = 0x8003_B601;
const HEADER: u32 = 0x0003_B602;
const OFFSETS: u32 = 0x0003_B604;
const VALUES: u32 = 0x0003_B605;
const TREE: u32 = 0x0003_B608;
const TREE_B: u32 = 0x0003_B607;

#[derive(Clone, Debug)]
pub struct ParameterMap {
    pub name_hash: u32,
    pub min: Vec2,
    pub max: Vec2,
    /// Value rows, each with one float per field (in field order).
    pub rows: Vec<Vec<f32>>,
    tree: Vec<[u16; 4]>,
}

impl ParameterMap {
    fn parse(map: Chunk<'_>) -> Result<ParameterMap> {
        let h = Reader::new(map.child(HEADER).context("map header")?.body);
        super::bundle::ensure_len(h.data(), 0x24, "map header")?;
        let value_count = h.u32_at(0x18)? as usize;
        let stride = h.u32_at(0x1C)? as usize;
        let field_count = h.u32_at(0x20)? as usize;
        let offsets = Reader::new(map.child(OFFSETS).context("field offsets")?.body);
        let values = Reader::new(map.child(VALUES).context("values")?.body);
        // Fuzzing: the header's counts and the field offsets checked
        // against the chunks before any read (a short header or a lying
        // count panicked); at least one row (`sample` takes one).
        anyhow::ensure!(value_count >= 1, "map without values");
        anyhow::ensure!(field_count.checked_mul(4).is_some_and(|n| n <= offsets.len()), "field offsets: {field_count} fields");
        let field_offsets = (0..field_count).map(|f| Ok(offsets.u32_at(f * 4)? as usize)).collect::<FormatResult<Vec<_>>>()?;
        let last_field = field_offsets.iter().copied().max().unwrap_or(0);
        let need = (value_count - 1).checked_mul(stride).and_then(|n| n.checked_add(last_field)).and_then(|n| n.checked_add(4));
        anyhow::ensure!(field_count == 0 || need.is_some_and(|n| n <= values.len()), "values: {value_count} rows of {stride} bytes");
        // Stride 0 (or no fields) satisfies the length check for any row
        // count and `rows` would be 4 billion Vecs. A row with fields owns
        // `stride` bytes of `values`, so the count is at most len / stride
        // (+1: the last row only needs its last field); without fields the
        // rows are empty and one is as good as many.
        anyhow::ensure!(field_count == 0 || stride > 0, "values: zero stride with {field_count} fields");
        let value_count = if field_count == 0 { value_count.min(1) } else { value_count };
        // Fields may share bytes (equal offsets), so rows x fields is not
        // bounded by the values chunk: by the limit instead.
        Limits::check("parameter map values", value_count.saturating_mul(field_count), Limits::DEFAULT.max_count)?;
        let rows = (0..value_count)
            .map(|row| field_offsets.iter().map(|&off| values.f32_at(row * stride + off)).collect::<FormatResult<Vec<f32>>>())
            .collect::<FormatResult<_>>()?;
        let tree = match (map.child(TREE_B), map.child(TREE)) {
            (Some(t), _) => Reader::new(t.body).whole_records(4).filter_map(|n| n.array_at::<4>(0).ok()).map(|n| n.map(u16::from)).collect(),
            (None, Some(t)) => Reader::new(t.body)
                .whole_records(8)
                .filter_map(|n| Some([n.u16_at(0).ok()?, n.u16_at(2).ok()?, n.u16_at(4).ok()?, n.u16_at(6).ok()?]))
                .collect(),
            (None, None) => Vec::new(),
        };
        Ok(ParameterMap {
            name_hash: h.u32_at(0)?,
            min: Vec2::new(h.f32_at(4)?, h.f32_at(8)?),
            max: Vec2::new(h.f32_at(0xC)?, h.f32_at(0x10)?),
            rows,
            tree,
        })
    }

    /// The value row in effect at ground position `p`.
    pub fn sample(&self, p: Vec2) -> &[f32] {
        &self.rows[self.row_at(p).min(self.rows.len() - 1)]
    }

    fn row_at(&self, p: Vec2) -> usize {
        if self.tree.is_empty() {
            return 0;
        }
        let (mut lo, mut hi) = (self.min, self.max);
        let mut node = 0usize;
        // Bounded by the node count so corrupt data cannot loop forever.
        for _ in 0..self.tree.len() {
            let n = self.tree[node];
            if n[0] == 0 {
                return n[1] as usize;
            }
            let mid = (lo + hi) * 0.5;
            let (right, up) = (p.x >= mid.x, p.y >= mid.y);
            if right { lo.x = mid.x } else { hi.x = mid.x }
            if up { lo.y = mid.y } else { hi.y = mid.y }
            node = n[match (right, up) {
                (false, false) => 0,
                (false, true) => 1,
                (true, false) => 2,
                (true, true) => 3,
            }] as usize;
            if node >= self.tree.len() {
                break;
            }
        }
        0
    }
}

/// All parameter maps in a track file, by name hash.
pub fn parse_parameter_maps(track_bun: &[u8]) -> Result<HashMap<u32, ParameterMap>> {
    let mut maps = HashMap::new();
    for container in bundle::chunks(track_bun, 0).filter(|c| c.id == PARAMETER_MAPS) {
        for map in container.children().filter(|c| c.id == MAP) {
            let m = ParameterMap::parse(map)?;
            maps.insert(m.name_hash, m);
        }
    }
    Ok(maps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn map(value_count: u32, stride: u32, field_count: u32) -> Vec<u8> {
        let mut h = vec![0u8; 0x24];
        h[0x18..0x1C].copy_from_slice(&value_count.to_le_bytes());
        h[0x1C..0x20].copy_from_slice(&stride.to_le_bytes());
        h[0x20..0x24].copy_from_slice(&field_count.to_le_bytes());
        let mut body = chunk(HEADER, &h);
        body.extend(chunk(OFFSETS, &[0; 4]));
        body.extend(chunk(VALUES, &[0; 4]));
        chunk(MAP, &body)
    }

    fn parse(bytes: &[u8]) -> Result<ParameterMap> {
        ParameterMap::parse(bundle::chunks(bytes, 0).next().unwrap())
    }

    /// Stride 0 with a huge row count used to build 4 billion row Vecs.
    #[test]
    fn zero_stride_with_huge_count_is_an_error() {
        assert!(parse(&map(0xFFFF_FFFF, 0, 1)).is_err());
        // No fields: rows are empty, and not 4 billion of them.
        assert_eq!(parse(&map(0xFFFF_FFFF, 0, 0)).unwrap().rows.len(), 1);
        // A sane map is unchanged.
        assert_eq!(parse(&map(1, 4, 1)).unwrap().rows, vec![vec![0.0]]);
    }
}

crate::persist!(ParameterMap { name_hash, min, max, rows, tree });
