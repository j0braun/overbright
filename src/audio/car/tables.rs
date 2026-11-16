/// The fade curve 0x7EF7F8 (512 dwords): trunc(32767 · cos(π/2 · k / 511))
/// (every entry checked against the EXE).
pub fn fade(k: usize) -> i32 {
    let k = k.min(511) as f64;
    (32767.0 * (std::f64::consts::FRAC_PI_2 * k / 511.0).cos()) as i32
}

/// The shift banks (records at 0x7F2860, 0x60 bytes: +0 the bank name,
/// +4 the up-shift volume, +8 the up-shift clunk delay (s), +0x3C the
/// down-shift volume, +0x40 the down-shift delay).
pub const SHIFT_BANKS: [(&str, i32, f32, i32, f32); 16] = [
    ("GEAR_SML_Base", 10000, 0.22, 8000, 0.1),
    ("GEAR_SML_Lev1", 14000, 0.18, 12000, 0.1),
    ("GEAR_SML_Lev2", 14000, 0.1, 12000, 0.08),
    ("GEAR_SML_Lev3", 14000, 0.03, 12000, 0.05),
    ("GEAR_MED_Base", 10000, 0.15, 8000, 0.1),
    ("GEAR_MED_Lev1", 14000, 0.13, 10000, 0.1),
    ("GEAR_MED_Lev2", 14000, 0.06, 12500, 0.06),
    ("GEAR_MED_Lev3", 14000, 0.03, 12000, 0.02),
    ("GEAR_LRG_Base", 14000, 0.25, 10000, 0.16),
    ("GEAR_LRG_Lev1", 14000, 0.15, 12000, 0.1),
    ("GEAR_LRG_Lev2", 14000, 0.07, 12000, 0.07),
    ("GEAR_LRG_Lev3", 14000, 0.02, 12000, 0.02),
    ("GEAR_TK_Base", 14000, 0.25, 12000, 0.2),
    ("GEAR_TK_Lev1", 14000, 0.2, 12000, 0.15),
    ("GEAR_TK_Lev2", 14000, 0.15, 12000, 0.15),
    ("GEAR_TK_Lev3", 14500, 0.1, 12000, 0.1),
];

/// Per sound index (0x459240) and transmission level, the shift record
/// (dword +4 of the 12-byte entries of 0x7F2230, read by 0x45E6A3 with the
/// sound context's transmission level +0xA4).
pub static SHIFTS: crate::exe_tables::Table<[[u8; 4]; 33]> = crate::exe_tables::Table::new(|t| &t.shifts);

/// The shift record a car plays.
pub fn shift_record(car_type: usize, transmission_level: usize) -> usize {
    SHIFTS.get(crate::audio::engine::sound_index(car_type)).map_or(0, |r| r[transmission_level.min(3)] as usize)
}

/// The nitrous bank (0x465830): Nitrous_01 for nitrous level 2 or 3,
/// else Nitrous_00 (the level is the sound context's +0xAC,
/// round(upgrade state float[2]) by 0x5995F0).
pub fn nitrous_bank(level: usize) -> &'static str {
    if matches!(level, 2 | 3) { "Nitrous_01_MB.abk" } else { "Nitrous_00_MB.abk" }
}

/// The shift-sweetener records (0x7F3040 + 12 · engine entry, 43 entries:
/// {bank name under sound\Engine\, the CAR_SWTN volume (+4), the
/// CAR_Sputter volume (+8)}; entries past 0x2B take record 0, 0x45E6DE /
/// 0x45F72F).
pub static SWTN: crate::exe_tables::Table<[(u8, i32, i32); 43]> = crate::exe_tables::Table::new(|t| &t.swtn);

/// The sweetener bank of an engine entry (CARSFX_Shift's second bank,
/// 0x465A2F: loaded when the record's name is not empty).
pub fn swtn_path(entry: usize) -> String {
    format!("SOUND/ENGINE/SWTN_CAR_{:02}_MB.abk", SWTN[entry.min(42)].0)
}

/// The CAR_SWTN "ShiftDisengage" block (0x4720D0 at the up-shift's start,
/// variant 0, rpm the context's; 0x4722B0 at the clutch engaging, variant
/// 1, rpm the Shifting's +0xA8), only at ≥ 7000 rpm (0x78ABF8) with the
/// sound flag 0x7FAAA8 (1): {variant, id 0..60, rpm 0..10000, volume
/// ((ramp(rpm) · record +4) >> 15 · level) >> 15 clamped 0..0x8013 (the
/// ramp 0x7F095C maps 4000..7000 rpm onto 26000..32767, so 32767 here;
/// level slot 0x38 index 3 / 2, the mix map's), the azimuth, 0, 0}.
pub fn swtn_block(variant: i32, id: i32, rpm: f32, entry: usize, level: i32) -> Option<[i32; 7]> {
    if rpm < 7000.0 {
        return None;
    }
    let t = ((rpm - 4000.0) / (7000.0 - 4000.0)).clamp(0.0, 1.0);
    let ramp = ((32767.0 - 26000.0) * t + 26000.0) as i32;
    let vol = ((ramp * SWTN[entry.min(42)].1) >> 15) * level >> 15;
    Some([variant, id.clamp(0, 60), (rpm as i32).clamp(0, 10000), vol.clamp(0, 0x8013), 0, 0, 0])
}

/// The shift event's volume (0x45E730): t = (trunc(ctx rpm +0x48) − 1500)
/// / (8000 − 1500), 0 when not positive and at most 1; k = trunc(512 − 512 · (0.1 +
/// 0.9 t)); level × ((fade(k) × record volume) >> 15) >> 15, with the
/// record's up (+4) or down (+0x3C) volume and the level from the sound
/// object's slot 0x38 (index 0 up, 1 down: the mix map's, `mixmap`; the
/// clone passes 0x7FFF). The event's parameter is clamped to 0..0x7FFF.
pub fn shift_volume(rpm: f32, up: bool, record: usize, level: i32) -> i32 {
    let (_, vol_up, _, vol_down, _) = SHIFT_BANKS[record.min(15)];
    let t = ((rpm as i32) as f32 - 1500.0) / (8000.0 - 1500.0);
    let t = if t > 0.0 { t.min(1.0) } else { 0.0 };
    let k = (512.0 - 512.0 * (t * 0.9 + 0.1)) as usize;
    let v = (fade(k) * if up { vol_up } else { vol_down }) >> 15;
    ((level * v) >> 15).clamp(0, 0x7FFF)
}

/// The turbo records (0x7F3338, 24 bytes: +0 the bank name, +4 the spool
/// whine's volume, +8 the blow-off volume of variant 1, +0xC that of
/// variants 2–3, +0x10 the full spool (40.0), +0x14 the spool lost per
/// tick off the throttle (0.5)). Every record has 40.0 and 0.5.
pub const TURBO_BANKS: [(&str, i32, i32, i32); 15] = [
    ("TURBO_SML1_0_MB", 6500, 24500, 16500),
    ("TURBO_SML1_1_MB", 6500, 24000, 18000),
    ("TURBO_SML1_2_MB", 8500, 22500, 15000),
    ("TURBO_SML2_0_MB", 6500, 22000, 15000),
    ("TURBO_SML2_1_MB", 7500, 22000, 15000),
    ("TURBO_SML2_2_MB", 6000, 18500, 14500),
    ("TURBO_MED_0_MB", 6500, 24000, 18000),
    ("TURBO_MED_1_MB", 9500, 24500, 18500),
    ("TURBO_MED_2_MB", 8000, 24500, 16500),
    ("TURBO_BIG_0_MB", 6000, 20000, 16000),
    ("TURBO_BIG_1_MB", 6000, 18000, 16500),
    ("TURBO_BIG_2_MB", 9500, 18000, 16000),
    ("TURBO_TRUCK_0_MB", 8500, 16500, 12500),
    ("TURBO_TRUCK_1_MB", 10500, 24000, 28500),
    ("TURBO_TRUCK_2_MB", 7250, 18000, 16500),
];
/// Turbo record +0x10 and +0x14.
pub const TURBO_FULL: f32 = 40.0;
pub const TURBO_DECAY: f32 = 0.5;

/// Per sound index and turbo level, the turbo record (dword +8 of the
/// 12-byte entries of 0x7F2230, read by CARSFX_Turbo's bind 0x45EB30 with
/// level = round(upgrade state float[4]) (0x5995F0)).
pub static TURBOS: crate::exe_tables::Table<[[u8; 4]; 33]> = crate::exe_tables::Table::new(|t| &t.turbos);

/// The turbo record a car plays at its turbo level.
pub fn turbo_record(car_type: usize, turbo_level: usize) -> usize {
    TURBOS.get(crate::audio::engine::sound_index(car_type)).map_or(0, |r| r[turbo_level.min(3)] as usize)
}

pub fn turbo_path(record: usize) -> String {
    format!("SOUND/TURBO/{}.abk", TURBO_BANKS[record.min(14)].0)
}

/// The bank paths.
pub fn nitrous_path(level: usize) -> String {
    format!("SOUND/NOS/{}", nitrous_bank(level))
}

pub fn shift_path(record: usize) -> String {
    format!("SOUND/SHIFTING/{}.abk", SHIFT_BANKS[record.min(15)].0)
}

/// Per sound index, the engine entry for each level (0x7F2230).
pub static LEVELS: crate::exe_tables::Table<[[u8; 4]; 33]> = crate::exe_tables::Table::new(|t| &t.engine_levels);

/// The engine entries' GIN names (SOUND/ENGINE/GIN_<name>.gin); 41 and 42
/// reuse entries 14's and 12's banks and recordings.
pub static GINS: crate::exe_tables::Table<[&str; 43]> = crate::exe_tables::Table::new(|t| &t.gins);
