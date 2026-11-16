//! Race tracks (chunk 0x34201).

/// A race track (chunk 0x34201, 0x128-byte records).
#[derive(Clone, Debug)]
pub struct Track {
    pub number: u16,
    /// The record's index in the table (0x5D3E90: base 0x883DA0 + i ×
    /// 0x128, count 0x883DA4): the Quick Race list's order (0x4CDED0).
    pub order: usize,
    /// +0x88: kept out of every Quick Race list (0x4987C0).
    pub hidden: bool,
    /// +0x7C: the Quick Race icon kind (0x4984B0: 0–4 a fixed icon, 5 the
    /// mode's own).
    pub icon: u32,
    /// +0x80: 1 for the drift tracks raced alone with traffic (the Quick
    /// Race options 0x4CD7A0: no Laps or Opponents, a Traffic Level).
    pub drift_kind: u32,
    /// +0x86 = 1: the track runs both ways (the Quick Race options offer
    /// Track Direction, 0x4B40B0).
    pub reversible: bool,
    /// e.g. "(4014) Short Circuit #4".
    pub name: String,
    /// e.g. "Location4\Track_4014".
    pub path: String,
    /// e.g. "L4RA".
    pub region: String,
    /// +0x94: flags (bits 0–3, 11–13 pick the AI speed mode, 0x5E9E90).
    pub flags: u32,
    /// Route length in metres (+0x98).
    pub length: u32,
    /// +0x9C / +0xA0: the record a track shows with none in the profile,
    /// forward / backward, in ms (0x502AA0: ×0.001 × 4000 + 0.5, ftol).
    pub default_record: [f32; 2],
    /// The minimap's frame (0x4CA840): world x at the map's left edge
    /// (+0xAC), world y at its top (+0xB0), the world size the map covers
    /// (+0xB4), and its zoom (+0xC0; the window is 0.3 / zoom of the map).
    pub map_left: f32,
    pub map_top: f32,
    pub map_size: f32,
    pub map_zoom: f32,
    /// The World Map widget's window on TRACKMAP<n> (0x498630, texels of
    /// 512): left +0x118, top +0x11C, size +0x120 (0 = the whole map).
    pub map_window: [f32; 3],
    /// +0x124: the widget shows the window at once, without the zoom
    /// (0x4B4B80 / 0x4CDD40).
    pub map_window_fixed: bool,
    /// Traffic cars to preload per traffic level and direction (+0xEC +
    /// 2·level + reversed, read by 0x4FE9F0; at most 16 are used).
    pub traffic: [i8; 8],
    /// Per traffic level, the odds that the placer (0x42E9F0) sets a
    /// traffic car going the way it walked, away from the player (+0x108 +
    /// 4·level; the track record is 0x883DAC).
    pub traffic_with: [f32; 4],
    /// +0xF6: the start kind the race start scenes are made for
    /// (0x42FFD0 against the NIS table entry's +8; 0x5D3E50 finds the
    /// record).
    pub start_kind: i8,
    /// Per direction (0x89E7A4), the gap past the start line within which
    /// the traffic placement (0x418110) refuses points before the green:
    /// +0xF8 + 4·direction, None when the byte +0xF4 + direction is set.
    pub start_gap: [Option<f32>; 2],
    /// Per direction, the drag race set-up fill's step (+0x100 + 4·direction,
    /// 0x42E4A0: its reach and its spacing; 31 m on the Bayview Bridge drags).
    pub drag_fill: [f32; 2],
}

impl Track {
    /// The name without its "(4014) " prefix.
    pub fn title(&self) -> &str {
        self.name.split_once(") ").map_or(&self.name, |(_, t)| t)
    }
}
