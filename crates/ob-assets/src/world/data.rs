use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use glam::Mat4;

/// One solid placed in the world.
#[derive(Clone, Debug)]
pub struct Placement {
    /// The highest LOD the track has (what the main view draws).
    pub solid: usize,
    /// The LOD the views with cull base 0x1800 (the road reflection, view
    /// 4, and the cube faces 0x10–0x15) draw: 0x5D672A takes the info's
    /// third solid (+0x38), or its second (+0x34) for flag 0x0400, and
    /// draws nothing when that pointer is null. `None` when the track has
    /// no such solid.
    pub low_solid: Option<usize>,
    /// The info's second solid (+0x34, LOD B): what the main view draws
    /// instead of `solid` when g_WorldLodLevel (0x870CF4) is 0 or 1
    /// (0x5D6717: 0x5BE1D0 ≠ 2 takes +0x34, null draws nothing).
    pub mid_solid: Option<usize>,
    /// Local → world, in the game's Z-up coordinates.
    pub transform: Mat4,
    pub section: u32,
    pub flags: u16,
    /// The radius the scenery cull sizes the instance with (0x5D6580:
    /// info +0x3C plus 6, 0x7A47D4), about its position (instance +0x20).
    pub cull_radius: f32,
}

/// Keep All Loaded's ring (clone deviation): the outlines within this
/// many metres of the eye lend their visible-section lists.
pub const LOOKAHEAD_RING: f32 = 60.0;

/// Keep All Loaded's cache (clone deviation): which sections some native
/// list draws together, and the sets made so far by the outlines lending.
#[derive(Default)]
pub struct LookaheadCache {
    /// Per section, the visible lists (indices, in list order) that name it.
    pub lists_of: std::sync::OnceLock<HashMap<u32, Vec<usize>>>,
    /// Per section, its placements (indices, in placement order).
    pub placements_of: std::sync::OnceLock<HashMap<u32, Vec<usize>>>,
    /// Per section, the ground cells ([`LOOKAHEAD_CELL`]) its placements'
    /// boxes cover, sorted; each made the first time a look-ahead asks for
    /// that section (all of them at once cost a drive's first conflict
    /// about 5 ms in one frame).
    pub cells: std::sync::Mutex<HashMap<u32, Arc<[u64]>>>,
    pub sets: std::sync::Mutex<HashMap<Vec<u32>, Arc<[u32]>>>,
    /// World Detail Extreme's look-ahead for the last eye section:
    /// [`World::far_sections_at`] measures from the section's outline
    /// centre at a fixed ring, so its set is the section's alone; kept
    /// here it is not worked out again (the outlines scanned) every frame.
    pub far: std::sync::Mutex<Option<(u32, Arc<[u32]>)>>,
}

/// Keep All Loaded's ground cells, metres: two sections overlap when
/// their placements' boxes share a cell.
pub const LOOKAHEAD_CELL: f32 = 16.0;

/// Whether Keep All Loaded may add section `s` to a drawn set `set`
/// (`Err`: why not). The lists' copies of a place are not alternatives
/// by number — base n and detail n + limit, and a region's panoramas
/// x90–x99, are listed together — so what may be drawn together is taken
/// from the game's own lists: `s` is added only when every section of the
/// set is either drawn with it by some native list (`together`) or lies
/// apart from it on the ground (`overlap` false: no shared cell), so no
/// place is drawn twice in a way no native list does. The shared
/// sections (X / Y props and textures, 2400+, no placements drawn of
/// their own) always.
/// A region's panorama section (x90–x99 of a letter's numbers; the shared
/// sections 2100+ are none).
pub fn is_panorama(s: u32) -> bool {
    s / 100 <= 20 && s % 100 >= 90
}

pub fn lookahead_admits(
    set: &[u32],
    s: u32,
    together: &dyn Fn(u32, u32) -> bool,
    overlap: &dyn Fn(u32, u32) -> bool,
) -> Result<(), &'static str> {
    if s / 100 > 20 {
        return Ok(());
    }
    match set.iter().find(|&&x| x / 100 <= 20 && !together(x, s) && overlap(x, s)) {
        None => Ok(()),
        Some(&x) if x / 100 == s / 100 && (x % 100 >= 90) != (s % 100 >= 90) => {
            Err("panorama and detail of one region")
        }
        Some(&x) if x / 100 == s / 100 => Err("over a section of its region"),
        Some(_) => Err("over another region's section"),
    }
}
