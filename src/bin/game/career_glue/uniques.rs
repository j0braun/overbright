/// An option of UI_ChooseUniquePart (0x533D30's list).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum UniqueItem {
    /// A unique part for a slot.
    Part { slot: u8, hash: u32 },
    /// A performance unique, 0..8 (0x502240's index).
    Perf(u8),
}

/// A world challenge's visual slot number to the car slot (0x5020C0).
pub(crate) fn unique_slot(n: u8) -> Option<u8> {
    Some(match n {
        0xB => 9,
        0xC => 0xC,
        0xD => 0x1D,
        0xE => 0xF,
        0xF => 0xE,
        0x10 => 0xB,
        0x11 => 2,
        0x12 => 1,
        0x13 => 0x20,
        0x14 => 7,
        0x15 => 0x10,
        0x16 => 6,
        0x18 => 0x9F,
        0x19 => 0x9A,
        0x1B => 0x9C,
        0x1E => 0x22,
        0x21 => 0x1F,
        0x23 => 0xA0,
        0x24 => 0xA1,
        _ => return None,
    })
}

/// A world challenge's performance slot number to the performance
/// unique's index (0x502240).
pub(crate) fn perf_unique_index(n: u8) -> Option<u8> {
    Some(match n {
        1 => 3,
        2 => 7,
        3 => 5,
        4 => 2,
        5 => 1,
        6 => 0,
        7 => 8,
        8 => 6,
        9 => 4,
        10 => 9,
        _ => return None,
    })
}

/// A unique part's icon on UI_ChooseUniquePart by slot (0x50E640).
pub(crate) fn unique_part_icon(slot: u8) -> u32 {
    match slot {
        6 => 0x945E_9BC5,
        9 => 0x028F_7092,
        0xC => 0xBB03_4EA6,
        0x1D => 0x0294_D2A3,
        0x40..=0x43 => 0x1B3A_8DD3,
        _ => 0x74CE_8C0B,
    }
}

/// Its label (0x50E6F0; 0 none).
pub(crate) fn unique_part_label(slot: u8) -> u32 {
    match slot {
        6 => 0x945E_9BC5,
        9 => 0x028F_7092,
        0xC => 0xBB03_4EA6,
        0x1D => 0x0294_D2A3,
        0x40..=0x43 => 0xD6D1_15E4,
        _ => 0,
    }
}

/// The performance uniques' labels (0x50E790, item +0x2C) and icons
/// (0x50E820, item +0x0C).
pub(crate) const PERF_UNIQUE_LABELS: [u32; 10] = [
    0x758D_5E3C,
    0x73AD_2F72,
    0xA788_6F38,
    0x026E_EA5E,
    0xABD0_FF14,
    0x9C34_C75F,
    0xAA89_59AD,
    0xA788_4765,
    0xABCA_6AEF,
    0,
];

pub(crate) const PERF_UNIQUE_ICONS: [u32; 10] = [
    0xEA57_D2C4,
    0x01A2_9FFA,
    0x9701_BDE4,
    0xC15C_94E6,
    0x06EF_789C,
    0x1784_75E7,
    0xBAA2_3A28,
    0x502A_BEED,
    0x06E8_E477,
    0,
];
