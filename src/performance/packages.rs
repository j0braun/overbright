use crate::formats::bundle::{read_f32, read_u32};

#[derive(Clone, Debug)]
pub struct PerfPart {
    pub id: u8,
    pub name: u32,
    pub price: u32,
    pub brands: Vec<u32>,
    pub gains: [f32; 4],
    pub replaces: Vec<u8>,
    pub requires: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Package {
    pub category: u8,
    pub level: u8,
    pub parts: Vec<PerfPart>,
}

/// The shop's categories (0x530500 adds them in this order): category,
/// icon, label.
pub const CATEGORIES: [(u8, u32, u32); 9] = [
    (3, 0xC15C_94E6, 0x026E_EA5E), // Engine
    (7, 0x502A_BEED, 0xA788_4765), // ECU
    (1, 0x01A2_9FFA, 0x73AD_2F72), // Transmission
    (5, 0x1784_75E7, 0x9C34_C75F), // Suspension
    (2, 0x9701_BDE4, 0xA788_6F38), // Nitrous Oxide
    (8, 0x06E8_E477, 0xABCA_6AEF), // Tires
    (6, 0xBAA2_3A28, 0xAA89_59AD), // Brakes
    (0, 0xEA57_D2C4, 0x758D_5E3C), // Weight Reduction
    (4, 0x06EF_789C, 0xABD0_FF14), // Turbo
];

/// A category's cost category (0x5021C0): its unlocks are the cost
/// table's for that record at the package level (0x515360).
pub fn cost_category(category: u8) -> u8 {
    [6, 5, 4, 1, 9, 3, 8, 2, 7, 10].get(category as usize).copied().unwrap_or(0)
}

/// Parse chunk 0x34A15.
pub fn parse(body: &[u8]) -> Vec<Package> {
    body.chunks_exact(0x17C)
        .map(|r| Package {
            category: read_u32(r, 0) as u8,
            level: read_u32(r, 4) as u8,
            parts: (0..(read_u32(r, 8) as usize).min(4))
                .map(|k| {
                    let e = &r[0xC + k * 0x5C..];
                    let brands = (0..(read_u32(e, 0xC) as usize).min(8)).map(|i| read_u32(e, 0x10 + i * 4)).collect();
                    let ids = |at: usize, n: usize| {
                        (0..n).map(|i| read_u32(e, at + i * 4)).filter(|&v| v != u32::MAX).map(|v| v as u8).collect()
                    };
                    let (replaces, requires) = (ids(0x40, 5), ids(0x54, 2));
                    PerfPart {
                        id: read_u32(e, 0) as u8,
                        name: read_u32(e, 4),
                        price: read_u32(e, 8),
                        brands,
                        gains: std::array::from_fn(|i| read_f32(e, 0x30 + i * 4)),
                        replaces,
                        requires,
                    }
                })
                .collect(),
        })
        .collect()
}

/// A part by id, with its package (0x5000A0).
pub fn part(packages: &[Package], id: u8) -> Option<(&Package, &PerfPart)> {
    packages.iter().find_map(|p| p.parts.iter().find(|x| x.id == id).map(|x| (p, x)))
}
