/// A shift record (0x7F2860 + 0x60 · n; the bank names and volumes are
/// `carfx::SHIFT_BANKS`). Times in ms unless marked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShiftRec {
    /// +8 the up-shift clunk delay (s); +0xC / +0x10 the rpm fall of the
    /// up-shift and its time; +0x14 / +0x18 the rpm added on engaging and
    /// the time it decays to the engine's; +0x1C / +0x20 the volume bump
    /// time and size (f32, × 32767 onto Engine +0xE0).
    pub up_delay: f32,
    pub up_drop: i32,
    pub up_drop_ms: i32,
    pub up_add: i32,
    pub up_add_ms: i32,
    pub bump_ms: i32,
    pub bump: f32,
    /// +0x24 / +0x28 / +0x2C the volume wobble's amplitude, period and
    /// decay time; +0x30 / +0x34 / +0x38 the rpm wobble's.
    pub wob_vol: i32,
    pub wob_vol_period: i32,
    pub wob_vol_ms: i32,
    pub wob_rpm: i32,
    pub wob_rpm_period: i32,
    pub wob_rpm_ms: i32,
    /// +0x40 the down-shift clunk delay (s); +0x44 / +0x48 the rpm fall
    /// and its time; +0x4C / +0x50 the throttle blip's rpm rise and time;
    /// +0x54 / +0x58 the fall after it; +0x5C (f32) the settle time per
    /// rpm of difference.
    pub down_delay: f32,
    pub down_drop: i32,
    pub down_drop_ms: i32,
    pub down_add: i32,
    pub down_blip_ms: i32,
    pub down_fall: i32,
    pub down_fall_ms: i32,
    pub settle: f32,
}


/// The 16 shift records (fields +8..+0x5C less the volumes; read from the
/// EXE).
pub static SHIFT_RECS: crate::exe_tables::Table<[ShiftRec; 16]> = crate::exe_tables::Table::new(|t| &t.shift_recs);

/// The launch records (0x7F3248 + 20 · n, n = engine record +0x18 for the
/// player, 0x45CA80; AI cars take 0x7F3284 or 0x7F32C0 at random):
/// {f32 the second ramp's top, rpm added, launch ms, settle ms, lift ms}.
pub static LAUNCHES: crate::exe_tables::Table<[(f32, i32, i32, i32, i32); 12]> = crate::exe_tables::Table::new(|t| &t.launches);

/// Engine record +0x18 (the launch record) for entries 0..42.
pub static LAUNCH_INDEX: crate::exe_tables::Table<[u8; 43]> = crate::exe_tables::Table::new(|t| &t.launch_index);
