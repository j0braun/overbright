/// The vinyl layers, bottom to top (slots 0x40–0x43; "Bottom Layer" …
/// "Top Layer", labels and default icons VINYL_LAYER_1 … _4).
pub const VINYL_LAYERS: [u8; 4] = [0x40, 0x41, 0x42, 0x43];

/// The labels / default icons of the layers (0x548460: bin_hash
/// VINYL_LAYER_1 … _4).
pub const VINYL_LAYER_ICONS: [u32; 4] = [0x27B8_F0DD, 0x27B8_F0DE, 0x27B8_F0DF, 0x27B8_F0E0];

/// A layer's `i`th remap colour slot, `i` from 1 to 4 (0x50F440): layer
/// 0x40 has 0x4D–0x50, 0x41 0x51–0x54, 0x42 0x55–0x58, 0x43 0x59–0x5C.
pub fn vinyl_colour_slot(layer: u8, i: u8) -> u8 {
    0x4D + (layer - 0x40) * 4 + (i - 1)
}

/// Whether `slot` is one of `layer`'s colour slots (0x50EDE0).
pub fn is_layer_colour(layer: u8, slot: u8) -> bool {
    (0x40..=0x43).contains(&layer) && (1..=4).any(|i| vinyl_colour_slot(layer, i) == slot)
}

/// The NUMCOLOURS attribute (0x564B8CB6) a unique vinyl carries: one
/// that takes all four layers (0x50EE90, 0x50EF10).
pub const UNIQUE_COLOURS: u32 = 0x40;
pub const NUMCOLOURS: u32 = 0x564B_8CB6;
/// REMAP (0x039DD714): the vinyl takes colours (the browser goes on to
/// UI_Paint for them, 0x5664A0).
pub const REMAP: u32 = 0x039D_D714;
/// NUMREMAPCOLOURS (0x6212682B): how many colours it takes.
pub const NUMREMAPCOLOURS: u32 = 0x6212_682B;
/// NUM_DECALS (0xA77BDCFA) of a decal location part: its layout's slots.
pub const NUM_DECALS: u32 = 0xA77B_DCFA;
/// The vinyl colours' brand (0x50F5B0 for slots 0x4D–0x5C): bin_hash
/// VINYL.
pub const VINYL_BRAND: u32 = 0x03E8_71F1;

/// The vinyl groups the browser strips (0x546040, each 0x545930: a
/// part kind, its icon and label), in the screen's order. A group lists
/// the car's slot-0x40 vinyls of its kind that are unlocked (0x50F0D0,
/// 0x52A020) and is locked without any.
pub static VINYL_GROUPS: crate::exe_tables::Table<[(u8, u32, u32); 28]> = crate::exe_tables::Table::new(|t| &t.vinyl_groups);

/// A vinyl group's icon (0x50E190; the layer options show the fitted
/// vinyl's).
pub fn vinyl_group_icon(kind: u8) -> Option<u32> {
    VINYL_GROUPS.iter().find(|g| g.0 == kind).map(|g| g.1)
}

/// The Unique group's icon, every layer's while a unique vinyl is on
/// (0x522140).
pub const UNIQUE_ICON: u32 = 0x1B3A_8DD3;

/// An entry of UI_DecalMain's option tree (0x546A00 by the node shown,
/// screen +0x154): a decal slot, whose decals list through
/// UI_DecalsOverlay (0x546F50), or a node that opens a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecalItem {
    Slot(u8),
    Node(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecalOption {
    pub item: DecalItem,
    pub icon: u32,
    pub label: u32,
    /// The top level's options lock with their location part (0x52FF70:
    /// the location slot's first part for the car, 0x515500 in a career).
    pub location: Option<u8>,
}

const fn slot(slot: u8, icon: u32, label: u32) -> DecalOption {
    DecalOption { item: DecalItem::Slot(slot), icon, label, location: None }
}

const fn node(node: u32, icon: u32, label: u32) -> DecalOption {
    DecalOption { item: DecalItem::Node(node), icon, label, location: None }
}

/// "Slot 1" … "Slot 8".
const SLOT_LABEL: u32 = 0x89BB_9CEA;

/// A node's options (0x546A00). The top (0): Windshield (slot 0x69),
/// Rear Window (0x71), Left Door, Right Door (six slots each), Left and
/// Right Quarter Panel (a layout first); 0x52FF70 locks them by their
/// location parts 0x35–0x3A. The quarter panels offer Layout 1 (one
/// slot) and, unless the car wears a wide body (slot 6's part above
/// level 0), Layout 2 (two). The hood's node 0x10005 (Layout 1: slots
/// 0x61–0x64, Layout 2: 0x61–0x68, location 0x34) is coded but not on
/// the top level.
pub fn decal_options(node_id: u32, wide_body: bool) -> Vec<DecalOption> {
    let numbered = |first: u8, count: u8, icon: u32| -> Vec<DecalOption> {
        (0..count).map(|i| slot(first + i, icon + i as u32, SLOT_LABEL + i as u32)).collect()
    };
    match node_id {
        0 => vec![
            DecalOption { location: Some(0x35), ..slot(0x69, 0x52DE_D91D, 0x52DE_D91D) },
            DecalOption { location: Some(0x36), ..slot(0x71, 0xAC79_37B4, 0xAC79_37B4) },
            DecalOption { location: Some(0x37), ..node(0x10001, 0xDA88_B711, 0xDA88_B711) },
            DecalOption { location: Some(0x38), ..node(0x10002, 0xC9A9_67C4, 0xC9A9_67C4) },
            DecalOption { location: Some(0x39), ..node(0x10003, 0x2C71_0C4D, 0x2C71_0C4D) },
            DecalOption { location: Some(0x3A), ..node(0x10004, 0xFFA7_D360, 0xFFA7_D360) },
        ],
        0x10001 => numbered(0x79, 6, 0xFE95_7F48),
        0x10002 => numbered(0x81, 6, 0x2E40_EADB),
        0x10003 => {
            let mut v = vec![node(0x20006, 0x13AE_389C, 0xB348_F286)];
            if !wide_body {
                v.push(node(0x20007, 0x13AE_389B, 0xB348_F287));
            }
            v
        }
        0x10004 => {
            let mut v = vec![node(0x20008, 0x5CED_1BE2, 0xB348_F286)];
            if !wide_body {
                v.push(node(0x20009, 0x5CED_1BE1, 0xB348_F287));
            }
            v
        }
        0x10005 => vec![node(0x2000A, 0x154F_A42F, 0xB348_F286), node(0x2000B, 0x154F_A430, 0xB348_F287)],
        0x20006 => vec![slot(0x89, 0x13AE_389C, SLOT_LABEL)],
        0x20007 => numbered(0x89, 2, 0xC813_D5BB),
        0x20008 => vec![slot(0x91, 0x5CED_1BE2, SLOT_LABEL)],
        0x20009 => numbered(0x91, 2, 0x392F_21C1),
        0x2000A => numbered(0x61, 4, 0xFDE2_B3CF),
        0x2000B => numbered(0x61, 8, 0x4ADC_F090),
        _ => Vec::new(),
    }
}

/// The node a node returns to (the screen keeps each level's node,
/// +0x158 by level, 0x560030).
pub fn decal_parent(node_id: u32) -> Option<u32> {
    match node_id {
        0 => None,
        0x10001..=0x10005 => Some(0),
        0x20006 | 0x20007 => Some(0x10003),
        0x20008 | 0x20009 => Some(0x10004),
        0x2000A | 0x2000B => Some(0x10005),
        _ => Some(0),
    }
}

/// A layout node's location slot, its decal slots' first slot and the
/// NUM_DECALS it asks of the location part (0x55FEA0, 0x53A7D0): Layout
/// 1 is the medium shape (one slot on a quarter panel), Layout 2 the
/// small ones.
pub fn decal_layout(node_id: u32) -> Option<(u8, u8, u32)> {
    match node_id {
        0x20006 => Some((0x39, 0x89, 1)),
        0x20007 => Some((0x39, 0x89, 2)),
        0x20008 => Some((0x3A, 0x91, 1)),
        0x20009 => Some((0x3A, 0x91, 2)),
        0x2000A => Some((0x34, 0x61, 1)),
        0x2000B => Some((0x34, 0x61, 2)),
        _ => None,
    }
}

/// The first decal slot of a door, quarter panel or layout node (the
/// "remove all decals in this layout" range, 0x55FF80: eight slots).
pub fn decal_node_slots(node_id: u32) -> Option<std::ops::Range<u8>> {
    let first = match node_id {
        0x10001 => 0x79,
        0x10002 => 0x81,
        0x10003 | 0x20006 | 0x20007 => 0x89,
        0x10004 | 0x20008 | 0x20009 => 0x91,
        0x10005 | 0x2000A | 0x2000B => 0x61,
        _ => return None,
    };
    Some(first..first + 8)
}

/// Whether a slot holds a decal or a decal layout (0x556880, 0x530250).
pub fn is_decal_slot(slot: u8) -> bool {
    (0x61..=0x98).contains(&slot) || (0x34..=0x3E).contains(&slot)
}

/// Whether a slot is a paint slot UI_Paint's paint mode fills (0x561A50
/// cancels these).
pub fn is_paint_slot(slot: u8) -> bool {
    slot == 0x3F || (0x44..=0x4A).contains(&slot)
}
