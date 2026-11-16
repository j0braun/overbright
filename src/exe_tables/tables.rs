//! Every table the clone reads from SPEED2.EXE, by address, type and count
//! (docs/exe-tables.md lists them with their users). Built once by
//! [`Tables::load`] from the checked executable.

use super::{Error, Exe, Rec, Result};
use crate::audio::car::skids::SkidRecord;
use crate::audio::car::transients::ShiftRec;
use crate::audio::engine_chain::{Rec80, Rec84};
use crate::garage::Shot;
use crate::render::native::car::lighting::RigLight;

/// The VP6 decoder's tables (0x7C4618..0x7C6668, 0x816708).
#[derive(Debug)]
pub struct Vp6Tables {
    pub ac_dequant: [i32; 64],
    pub dc_dequant: [i16; 64],
    pub zigzag: [u8; 64],
    pub coeff_group: [i8; 64],
    pub default_reorder: [u8; 64],
    pub ref_of_type: [u8; 10],
    pub dccv_update: [u8; 22],
    pub reorder_update: [u8; 64],
    pub runv_update: [u8; 28],
    pub runv_default: [u8; 28],
    pub ract_update: [u8; 396],
    pub dccv_lc: [i32; 30],
    pub vector_update: [u8; 34],
    pub mb_stats_presets: [u8; 960],
    pub mb_stats_default: [u8; 60],
    pub cat_table: [u8; 176],
    pub candidates: [i32; 24],
    pub bilinear: [i32; 16],
    pub token_base: [i32; 12],
    pub coeff_group_huffman: [i8; 64],
}

/// The tables, read once at start ([`super::init`]).
#[derive(Debug)]
pub struct Tables {
    // ---- car physics and driving
    /// Stick response over input −1..1 (graph 0x7E9E60 → 0x7E9228).
    pub steer_response: [f32; 21],
    /// The same in drift and URL races (0x7E9E9C → 0x7E9330).
    pub steer_response_drift: [f32; 21],
    /// Lateral force curves: row 0 in zero-filled data (0x86B34C), rows
    /// 1..6 at 0x7FC744 (graph 0x7FC7D4).
    pub lateral: [[f32; 6]; 7],
    /// The race tunes, rows 0x144 apart by layout: drift 0x7FC2E4, type 2
    /// 0x7FC33C, drag 0x7FC394.
    pub drift_tune: [[f32; 22]; 3],
    pub street_x_tune: [[f32; 22]; 3],
    pub drag_tune: [[f32; 22]; 3],
    /// The 0-60 / 0-100 mph time factors (0x7FC120 / 0x7FC1D8).
    pub zero_60_factor: [f32; 31],
    pub zero_100_factor: [f32; 31],
    /// The contact surface weight: surface table 0x800EE0 + 0x98·s, +0x34.
    pub surface_weight: [f32; 29],
    /// The chase camera's shake graph (0x7EC430).
    pub shake: [[f32; 4]; 5],
    // ---- drift scoring
    /// The multiplier levels' speeds (0x79DA78) and points (0x79DA90).
    pub drift_level_speed: [f32; 6],
    pub drift_level_points: [[i32; 4]; 4],
    /// The downhill virtual rivals (0x7FA5C0, 6 rows of 13 dwords).
    pub drift_virtual_scores: [(u16, [f32; 12]); 6],
    // ---- AI driving
    /// The AI steering lock and table rate by speed (0x7E9074, 0x7E909C).
    pub ai_lock: [f32; 10],
    pub ai_table_rate_speed: [f32; 10],
    /// The 16-bit arcsine (0x7EA628, 8-byte entries {u16 angle, f32 slope}).
    pub asin: [(u16, f32); 208],
    // ---- audio
    /// The collision level curve (0x7ECB78) and list 0's gains (0x7F34A0).
    pub collision_levels: [f32; 20],
    pub collision_gain: [u16; 289],
    /// The mix map's dB ↔ linear tables (0x7EDE88, 0x7EE7F0).
    pub db_to_lin: [i32; 602],
    pub lin_to_db: [i32; 512],
    /// Per sound index and level, the 12-byte entries of 0x7F2230: the
    /// engine entry (+0), the shift record (+4), the turbo record (+8).
    pub engine_levels: [[u8; 4]; 33],
    pub shifts: [[u8; 4]; 33],
    pub turbos: [[u8; 4]; 33],
    /// The shift sweeteners (0x7F3040, 12 bytes: the SWTN_CAR_<nn> bank's
    /// number, the CAR_SWTN and CAR_Sputter volumes).
    pub swtn: [(u8, i32, i32); 43],
    /// The engine entries' GIN names (the engine records' +0x0C less
    /// "GIN_" and ".gin").
    pub gins: [&'static str; 43],
    /// The shift records (0x7F2860, 0x60 bytes) and launch records
    /// (0x7F3248, 20 bytes); each engine record's launch record (+0x18).
    pub shift_recs: [ShiftRec; 16],
    pub launches: [(f32, i32, i32, i32, i32); 12],
    pub launch_index: [u8; 43],
    /// The skid tuning records (0x7F2F20, 0x48 bytes) and the skid class
    /// by sound index (0x458F00's switch).
    pub skid_records: [SkidRecord; 4],
    pub skid_class: [u8; 32],
    /// The car type's sound index (0x459240's switch).
    pub sound_index: [u8; 31],
    /// The engine and decel records (0x7F0DE0, 0x7F1AA8).
    pub engines: [Rec80; 43],
    pub decels: [Rec84; 37],
    /// The 15 scripted car-audio patterns (0x7F0070..0x7F08F8,
    /// {u16 rpm, u16 throttle}).
    pub patterns: [Vec<(u16, u8)>; 15],
    /// The music contexts' score events (0x7ECC10, dword pairs).
    pub score_events: [(i16, i16); 31],
    /// The trains (0x7ED798, 12-byte entries).
    pub trains: [(u32, u32); 24],
    /// The UTK decoder: reflection coefficients (0x819208), codebooks
    /// (0x819308), commands (0x819508, 12 bytes).
    pub utk_rc: [f32; 64],
    pub utk_codebooks: [[u8; 256]; 2],
    pub utk_commands: [(u8, u8, f32); 29],
    /// The layer III decoder: the Huffman trees (pointer list 0x7D0C30),
    /// big values' linbits (0x7D0CC0), the synthesis window (0x7D1600,
    /// each row twice).
    pub mp3_huff: [Option<Box<[i16]>>; 32],
    pub mp3_linbits: [u8; 16],
    pub mp3_window_rows: [[f32; 16]; 17],
    pub vp6: Vp6Tables,
    // ---- rendering
    /// The in-game and default car light setups (0x7F3F00, 0x7F3E10).
    pub in_game_rig: [RigLight; 3],
    pub default_rig: [RigLight; 3],
    // ---- garage cameras
    /// The part views (0x7F4608: two tables of 0x31 records of 0x3C bytes,
    /// cars then SUVs; the slot at +8, the view at +0xC): records 0..41.
    pub slot_shots: [[(u32, Shot); 42]; 2],
    /// The vinyl views (0x7F5D40, 7 records of 0x3C a class) and the SUV
    /// engine view (0x7F5D00).
    pub vinyl_shots: [[Shot; 7]; 2],
    pub suv_engine_shot: Shot,
    // ---- front end and career
    /// The world map's zoom records (0x7F42C8, 16 bytes).
    pub world_map_zooms: [(f32, f32, f32, u32); 6],
    /// Magazine / DVD covers and the DVD by rating (0x7FA3F0, 0x7FA490,
    /// 0x7FA4DC).
    pub magazine_lines: [(u32, u32); 20],
    pub dvd_lines: [(u32, u32); 10],
    pub rating_lines: [u32; 11],
    /// The part type names (pointers 0x80333C).
    pub part_type_names: [&'static str; 0x69],
    /// The slots' part types (0x802960, dwords), cost categories (set up
    /// by 0x501920 into 0x837D30) and vinyl slot categories by kind
    /// (0x501D70's switch 0x502020).
    pub slot_types: [u8; 0xAA],
    pub slot_categories: [u8; 0xAA],
    pub vinyl_categories: [u8; 28],
    /// The RANDOM opponents' car types (0x7F6DA4).
    pub opponent_cars: [u8; 26],
    /// The vinyl groups (0x546040's calls of 0x545930: kind, icon, label).
    pub vinyl_groups: [(u8, u32, u32); 28],
    /// Quick Race opponent names (pointers 0x7F6CD0).
    pub opponent_names: [&'static str; 53],
    /// EA Trax (0x7ECD80, 16 bytes: title, artist, album pointers).
    pub ea_trax: [(&'static str, &'static str, &'static str); 27],
    /// Where each EA Trax track plays (+0xC: "FE" or "IG").
    pub ea_trax_where: [&'static str; 27],
    /// The tuning parameters' names (0x50FE90's switch).
    pub tuning_names: [u32; 25],
    /// The graphics cards' scores (0x8005C0, 12 bytes: vendor, device,
    /// f32 score).
    pub card_scores: [(u32, u32, u8); 120],
    // ---- read by the fresh profile writer (career::fresh_payload)
    /// The Quick Race records' default cars by car type index (0x7FAA48,
    /// 0x7FAA4C).
    pub qr_default_cars: [u32; 2],
    /// The ranked rides' table as 0x77D3B0 fills it (0x8641A8, 0x12
    /// dwords: the ONLINE_CAREER_* name hashes, stock car indices, kept
    /// numbers).
    pub ranked_rides: [u32; 0x12],
}

/// Rows of `N` plain values `row_stride` bytes apart.
fn rows<T: bytemuck::Pod, const W: usize, const N: usize>(exe: &Exe, va: u32, row_stride: u32) -> Result<[[T; W]; N]> {
    let v: Vec<[T; W]> = (0..N as u32).map(|r| exe.array::<T, W>(va + r * row_stride)).collect::<Result<_>>()?;
    Ok(v.try_into().unwrap_or_else(|_| unreachable!()))
}

fn arr<T, const N: usize>(v: Vec<T>) -> [T; N] {
    v.try_into().unwrap_or_else(|_| unreachable!())
}

/// A garage view at `o` of a record ([`Shot`]: angle, steer, field of
/// view, target, eye).
fn shot(r: &Rec<'_, '_>, o: usize) -> Shot {
    let v = |o: usize| glam::Vec3::new(r.f32(o), r.f32(o + 4), r.f32(o + 8));
    Shot { car_angle: r.f32(o), steer: r.f32(o + 4), fov: r.f32(o + 8), target: v(o + 0xC), eye: v(o + 0x18) }
}

fn byte(va: u32, v: u32) -> Result<u8> {
    u8::try_from(v).map_err(|_| Error::Shape { va, what: "a value past a byte" })
}

/// One light of a setup (+0x20 + i·0x1C: flag, θ, φ, r, g, b, intensity).
fn rig(exe: &Exe, va: u32) -> Result<[RigLight; 3]> {
    exe.records(va + 0x20, 0x1C, |r| {
        Ok(RigLight {
            camera_relative: r.u32(0) != 0,
            theta: r.f32(4),
            phi: r.f32(8),
            colour: glam::Vec3::new(r.f32(12), r.f32(16), r.f32(20)),
            intensity: r.f32(24),
        })
    })
}

const PATTERNS: [(u32, usize); 15] = [
    (0x7F_0070, 73),
    (0x7F_0198, 40),
    (0x7F_0238, 27),
    (0x7F_02A8, 31),
    (0x7F_0328, 28),
    (0x7F_0398, 23),
    (0x7F_03F8, 20),
    (0x7F_0448, 68),
    (0x7F_0558, 57),
    (0x7F_0640, 35),
    (0x7F_06D0, 31),
    (0x7F_0750, 30),
    (0x7F_07C8, 48),
    (0x7F_0888, 28),
    (0x7F_08F8, 25),
];

impl Tables {
    pub fn load(exe: &'static Exe) -> Result<Tables> {
        let engines: [Rec80; 43] = exe.records(0x7F_0DE0, 0x4C, |r: &Rec<'static, '_>| {
            Ok(Rec80 {
                gin: r.str_ptr(0x0C)?,
                hybrid: r.u32(0x10) != 0,
                decel: r.u32(0x14) as usize,
                rpm_max: r.f32(0x1C),
                rpm_min: r.f32(0x20),
                vol_a: r.i32(0x24),
                vol_b: r.i32(0x28),
                d0: r.f32(0x2C),
                a1: r.f32(0x30),
                b1: r.f32(0x34),
                a0: r.f32(0x38),
                b0: r.f32(0x3C),
                fixed_a: r.f32(0x40),
                fixed_b: r.f32(0x44),
                cutoff: r.i32(0x48),
            })
        })?;
        let launch_index: [u8; 43] = exe.records(0x7F_0DE0, 0x4C, |r| byte(0x7F_0DE0, r.u32(0x18)))?;
        let gins = engines.map(|e| e.gin.strip_prefix("GIN_").and_then(|g| g.strip_suffix(".gin")).unwrap_or(e.gin));
        let level_entries = |off: usize| -> Result<[[u8; 4]; 33]> {
            exe.records(0x7F_2230, 48, |r| Ok(std::array::from_fn(|l| r.u32(12 * l + off) as u8)))
        };
        let lateral_rows: [[f32; 6]; 6] = rows(exe, 0x7F_C744, 24)?;
        let mut lateral = [exe.array::<f32, 6>(0x86_B34C)?; 7];
        lateral[1..].copy_from_slice(&lateral_rows);
        let skid_class = {
            let cases = exe.switch_returns(0x45_8F28, 3)?;
            let index: [u8; 32] = exe.array(0x45_8F34)?;
            let mut out = [0u8; 32];
            for (o, &i) in out.iter_mut().zip(&index) {
                *o = byte(
                    0x45_8F34,
                    *cases.get(i as usize).ok_or(Error::Shape { va: 0x45_8F34, what: "a case past the jump table" })?,
                )?;
            }
            out
        };
        let mp3_huff = {
            let ptrs: [u32; 32] = exe.array(0x7D_0C30)?;
            let mut starts: Vec<u32> = ptrs.iter().copied().filter(|&p| p != 0).collect();
            starts.sort_unstable();
            starts.dedup();
            let mut out: [Option<Box<[i16]>>; 32] = Default::default();
            for (o, &p) in out.iter_mut().zip(&ptrs) {
                if p != 0 {
                    let end = starts.iter().copied().find(|&s| s > p).unwrap_or(0x7D_0C30);
                    *o = Some(exe.vec::<i16>(p, ((end - p) / 2) as usize)?.into_boxed_slice());
                }
            }
            out
        };
        let i32_bytes = |va: u32, n: usize| -> Result<Vec<u8>> {
            exe.vec::<i32>(va, n)?.into_iter().map(|v| byte(va, v as u32)).collect()
        };
        let i32_signed = |va: u32, n: usize| -> Result<Vec<i8>> {
            Ok(exe.vec::<i32>(va, n)?.into_iter().map(|v| v as i8).collect())
        };
        let vp6 = Vp6Tables {
            ac_dequant: exe.array(0x7C_4618)?,
            dc_dequant: exe.array(0x7C_4C18)?,
            zigzag: arr(i32_bytes(0x7C_5218, 64)?),
            coeff_group: arr(i32_signed(0x7C_56F0, 64)?),
            default_reorder: exe.array(0x7C_5A30)?,
            ref_of_type: arr(i32_bytes(0x7C_5AB0, 10)?),
            dccv_update: exe.array(0x7C_5FC0)?,
            reorder_update: exe.array(0x7C_5FD8)?,
            runv_update: exe.array(0x7C_6018)?,
            runv_default: exe.array(0x7C_6034)?,
            ract_update: exe.array(0x7C_6050)?,
            dccv_lc: exe.array(0x7C_61E0)?,
            vector_update: exe.array(0x7C_6258)?,
            mb_stats_presets: exe.array(0x7C_62A8)?,
            mb_stats_default: exe.array(0x7C_6668)?,
            cat_table: exe.array(0x7C_5630)?,
            candidates: exe.array(0x7C_5528)?,
            bilinear: exe.array(0x81_6708)?,
            token_base: exe.array(0x7C_58F8)?,
            coeff_group_huffman: arr(i32_signed(0x7C_5928, 64)?),
        };
        let vinyl_groups = {
            let calls = exe.pushed_args(0x54_6040, 0x54_5930, 28)?;
            let v: Vec<(u8, u32, u32)> = calls
                .iter()
                .map(|a| match a.as_slice() {
                    [label, icon, kind] => Ok((byte(0x54_6040, *kind)?, *icon, *label)),
                    _ => Err(Error::Shape { va: 0x54_6040, what: "a vinyl group call without three arguments" }),
                })
                .collect::<Result<_>>()?;
            arr(v)
        };
        Ok(Tables {
            steer_response: exe.array(0x7E_9228)?,
            steer_response_drift: exe.array(0x7E_9330)?,
            lateral,
            drift_tune: rows(exe, 0x7F_C2E4, 0x144)?,
            street_x_tune: rows(exe, 0x7F_C33C, 0x144)?,
            drag_tune: rows(exe, 0x7F_C394, 0x144)?,
            zero_60_factor: exe.array(0x7F_C120)?,
            zero_100_factor: exe.array(0x7F_C1D8)?,
            surface_weight: exe.records(0x80_0EE0, 0x98, |r| Ok(r.f32(0x34)))?,
            shake: rows(exe, 0x7E_C430, 16)?,
            drift_level_speed: exe.array(0x79_DA78)?,
            drift_level_points: rows(exe, 0x79_DA90, 16)?,
            drift_virtual_scores: exe.records(0x7F_A5C0, 52, |r| Ok((r.u32(0) as u16, r.f32s::<12>(4))))?,
            ai_lock: exe.array(0x7E_9074)?,
            ai_table_rate_speed: exe.array(0x7E_909C)?,
            asin: exe.records(0x7E_A628, 8, |r| Ok((r.u16(0), r.f32(4))))?,
            collision_levels: exe.array(0x7E_CB78)?,
            collision_gain: exe.array(0x7F_34A0)?,
            db_to_lin: exe.array(0x7E_DE88)?,
            lin_to_db: exe.array(0x7E_E7F0)?,
            engine_levels: level_entries(0)?,
            shifts: level_entries(4)?,
            turbos: level_entries(8)?,
            swtn: exe.records(0x7F_3040, 12, |r| {
                let name = r.str_ptr(0)?;
                let n = name
                    .strip_prefix("SWTN_CAR_")
                    .and_then(|s| s.get(..2))
                    .and_then(|s| s.parse::<u8>().ok())
                    .ok_or(Error::Shape { va: 0x7F_3040, what: "a sweetener bank not named SWTN_CAR_<nn>" })?;
                Ok((n, r.i32(4), r.i32(8)))
            })?,
            gins,
            shift_recs: exe.records(0x7F_2860, 0x60, |r| {
                Ok(ShiftRec {
                    up_delay: r.f32(0x08),
                    up_drop: r.i32(0x0C),
                    up_drop_ms: r.i32(0x10),
                    up_add: r.i32(0x14),
                    up_add_ms: r.i32(0x18),
                    bump_ms: r.i32(0x1C),
                    bump: r.f32(0x20),
                    wob_vol: r.i32(0x24),
                    wob_vol_period: r.i32(0x28),
                    wob_vol_ms: r.i32(0x2C),
                    wob_rpm: r.i32(0x30),
                    wob_rpm_period: r.i32(0x34),
                    wob_rpm_ms: r.i32(0x38),
                    down_delay: r.f32(0x40),
                    down_drop: r.i32(0x44),
                    down_drop_ms: r.i32(0x48),
                    down_add: r.i32(0x4C),
                    down_blip_ms: r.i32(0x50),
                    down_fall: r.i32(0x54),
                    down_fall_ms: r.i32(0x58),
                    settle: r.f32(0x5C),
                })
            })?,
            launches: exe.records(0x7F_3248, 20, |r| Ok((r.f32(0), r.i32(4), r.i32(8), r.i32(12), r.i32(16))))?,
            launch_index,
            skid_records: exe.records(0x7F_2F20, 0x48, |r| {
                Ok(SkidRecord {
                    long_scale: r.f32(0x00),
                    long_hi: r.f32(0x04),
                    long_lo: r.f32(0x08),
                    lat_scale: r.f32(0x0C),
                    lat_hi: r.f32(0x10),
                    lat_lo: r.f32(0x14),
                    l3_up: r.i32(0x18),
                    l3_down: r.i32(0x1C),
                    l3_mix: r.f32(0x20),
                    wheel_threshold: r.f32(0x28),
                    hold: r.f32(0x2C),
                    l2_up: r.i32(0x30),
                    l2_down: r.i32(0x34),
                    l2_mix: r.f32(0x38),
                    off_side: r.f32(0x3C),
                    angle_in0: r.f32(0x40),
                    angle_in1: r.f32(0x44),
                })
            })?,
            skid_class,
            sound_index: arr(exe
                .switch_returns(0x45_930C, 31)?
                .into_iter()
                .map(|v| byte(0x45_930C, v))
                .collect::<Result<_>>()?),
            engines,
            decels: exe.records(0x7F_1AA8, 0x34, |r: &Rec<'static, '_>| {
                Ok(Rec84 {
                    gin: r.str_ptr(0)?,
                    vol_a: r.i32(0x04),
                    vol_d: r.i32(0x08),
                    rpm0: r.i32(0x0C),
                    rpm1: r.i32(0x10),
                    up: r.f32(0x14),
                    down: r.f32(0x18),
                    d0: r.f32(0x1C),
                    a1: r.f32(0x24),
                    b1: r.f32(0x28),
                    a0: r.f32(0x2C),
                    b0: r.f32(0x30),
                })
            })?,
            patterns: arr(PATTERNS
                .iter()
                .map(|&(va, n)| exe.records_vec(va, 4, n, |r| Ok((r.u16(0), byte(va, r.u16(2) as u32)?))))
                .collect::<Result<_>>()?),
            score_events: exe.records(0x7E_CC10, 8, |r| Ok((r.i32(0) as i16, r.i32(4) as i16)))?,
            trains: exe.records(0x7E_D798, 12, |r| Ok((r.u32(0), r.u32(4))))?,
            utk_rc: exe.array(0x81_9208)?,
            utk_codebooks: rows(exe, 0x81_9308, 256)?,
            utk_commands: exe.records(0x81_9508, 12, |r| Ok((r.u32(0) as u8, r.u32(4) as u8, r.f32(8))))?,
            mp3_huff,
            mp3_linbits: exe.array(0x7D_0CC0)?,
            mp3_window_rows: rows(exe, 0x7D_1600, 128)?,
            vp6,
            in_game_rig: rig(exe, 0x7F_3F00)?,
            default_rig: rig(exe, 0x7F_3E10)?,
            slot_shots: [
                exe.records(0x7F_4608, 0x3C, |r| Ok((r.u32(8), shot(r, 0xC))))?,
                exe.records(0x7F_4608 + 0x31 * 0x3C, 0x3C, |r| Ok((r.u32(8), shot(r, 0xC))))?,
            ],
            vinyl_shots: [
                exe.records(0x7F_5D40, 0x3C, |r| Ok(shot(r, 0xC)))?,
                exe.records(0x7F_5D40 + 7 * 0x3C, 0x3C, |r| Ok(shot(r, 0xC)))?,
            ],
            suv_engine_shot: exe.records::<_, 1>(0x7F_5D00, 0x3C, |r| Ok(shot(r, 0xC)))?[0],
            world_map_zooms: exe.records(0x7F_42C8, 16, |r| Ok((r.f32(0), r.f32(4), r.f32(8), r.u32(12))))?,
            magazine_lines: exe.records(0x7F_A3F0, 8, |r| Ok((r.u32(0), r.u32(4))))?,
            dvd_lines: exe.records(0x7F_A490, 8, |r| Ok((r.u32(0), r.u32(4))))?,
            rating_lines: exe.array(0x7F_A4DC)?,
            part_type_names: exe.records(0x80_333C, 8, |r: &Rec<'static, '_>| r.str_ptr(0))?,
            slot_types: arr(exe
                .vec::<u32>(0x80_2960, 0xAA)?
                .into_iter()
                .map(|v| byte(0x80_2960, v))
                .collect::<Result<_>>()?),
            slot_categories: arr(exe
                .initialised_dwords(0x50_1920, 0x83_7D30, 0xAA)?
                .into_iter()
                .map(|v| byte(0x50_1920, v))
                .collect::<Result<_>>()?),
            vinyl_categories: arr(exe
                .switch_returns(0x50_2020, 28)?
                .into_iter()
                .map(|v| byte(0x50_2020, v))
                .collect::<Result<_>>()?),
            opponent_cars: exe.array(0x7F_6DA4)?,
            vinyl_groups,
            opponent_names: exe.records(0x7F_6CD0, 4, |r: &Rec<'static, '_>| r.str_ptr(0))?,
            ea_trax: exe
                .records(0x7E_CD80, 16, |r: &Rec<'static, '_>| Ok((r.str_ptr(0)?, r.str_ptr(4)?, r.str_ptr(8)?)))?,
            ea_trax_where: exe.records(0x7E_CD80, 16, |r: &Rec<'static, '_>| r.str_ptr(12))?,
            tuning_names: arr(exe.switch_returns(0x50_FF40, 25)?),
            qr_default_cars: exe.array(0x7F_AA48)?,
            ranked_rides: arr(exe.initialised_dwords_with(0x77_D3B0, 0x86_41A8, 0x12, &|callee, pushed| {
                (callee == 0x43_DB50)
                    .then(|| pushed.and_then(|va| exe.cstr(va).ok()).map_or(0, crate::formats::hash::bin_hash))
            })?),
            card_scores: exe.records(0x80_05C0, 12, |r| Ok((r.u32(0), r.u32(4), r.f32(8) as u8)))?,
        })
    }
}

impl Tables {
    /// Each table's name and `Debug` text (what [`FIELD_PINS`] hashes).
    pub fn field_texts(&self) -> Vec<(&'static str, String)> {
        vec![
            ("steer_response", format!("{:?}", self.steer_response)),
            ("steer_response_drift", format!("{:?}", self.steer_response_drift)),
            ("lateral", format!("{:?}", self.lateral)),
            ("drift_tune", format!("{:?}", self.drift_tune)),
            ("street_x_tune", format!("{:?}", self.street_x_tune)),
            ("drag_tune", format!("{:?}", self.drag_tune)),
            ("zero_60_factor", format!("{:?}", self.zero_60_factor)),
            ("zero_100_factor", format!("{:?}", self.zero_100_factor)),
            ("surface_weight", format!("{:?}", self.surface_weight)),
            ("shake", format!("{:?}", self.shake)),
            ("drift_level_speed", format!("{:?}", self.drift_level_speed)),
            ("drift_level_points", format!("{:?}", self.drift_level_points)),
            ("drift_virtual_scores", format!("{:?}", self.drift_virtual_scores)),
            ("ai_lock", format!("{:?}", self.ai_lock)),
            ("ai_table_rate_speed", format!("{:?}", self.ai_table_rate_speed)),
            ("asin", format!("{:?}", self.asin)),
            ("collision_levels", format!("{:?}", self.collision_levels)),
            ("collision_gain", format!("{:?}", self.collision_gain)),
            ("db_to_lin", format!("{:?}", self.db_to_lin)),
            ("lin_to_db", format!("{:?}", self.lin_to_db)),
            ("engine_levels", format!("{:?}", self.engine_levels)),
            ("shifts", format!("{:?}", self.shifts)),
            ("turbos", format!("{:?}", self.turbos)),
            ("swtn", format!("{:?}", self.swtn)),
            ("gins", format!("{:?}", self.gins)),
            ("shift_recs", format!("{:?}", self.shift_recs)),
            ("launches", format!("{:?}", self.launches)),
            ("launch_index", format!("{:?}", self.launch_index)),
            ("skid_records", format!("{:?}", self.skid_records)),
            ("skid_class", format!("{:?}", self.skid_class)),
            ("sound_index", format!("{:?}", self.sound_index)),
            ("engines", format!("{:?}", self.engines)),
            ("decels", format!("{:?}", self.decels)),
            ("patterns", format!("{:?}", self.patterns)),
            ("score_events", format!("{:?}", self.score_events)),
            ("trains", format!("{:?}", self.trains)),
            ("utk_rc", format!("{:?}", self.utk_rc)),
            ("utk_codebooks", format!("{:?}", self.utk_codebooks)),
            ("utk_commands", format!("{:?}", self.utk_commands)),
            ("mp3_huff", format!("{:?}", self.mp3_huff)),
            ("mp3_linbits", format!("{:?}", self.mp3_linbits)),
            ("mp3_window_rows", format!("{:?}", self.mp3_window_rows)),
            ("vp6", format!("{:?}", self.vp6)),
            ("in_game_rig", format!("{:?}", self.in_game_rig)),
            ("default_rig", format!("{:?}", self.default_rig)),
            ("slot_shots", format!("{:?}", self.slot_shots)),
            ("vinyl_shots", format!("{:?}", self.vinyl_shots)),
            ("suv_engine_shot", format!("{:?}", self.suv_engine_shot)),
            ("world_map_zooms", format!("{:?}", self.world_map_zooms)),
            ("magazine_lines", format!("{:?}", self.magazine_lines)),
            ("dvd_lines", format!("{:?}", self.dvd_lines)),
            ("rating_lines", format!("{:?}", self.rating_lines)),
            ("part_type_names", format!("{:?}", self.part_type_names)),
            ("slot_types", format!("{:?}", self.slot_types)),
            ("slot_categories", format!("{:?}", self.slot_categories)),
            ("vinyl_categories", format!("{:?}", self.vinyl_categories)),
            ("opponent_cars", format!("{:?}", self.opponent_cars)),
            ("vinyl_groups", format!("{:?}", self.vinyl_groups)),
            ("opponent_names", format!("{:?}", self.opponent_names)),
            ("ea_trax", format!("{:?}", self.ea_trax)),
            ("ea_trax_where", format!("{:?}", self.ea_trax_where)),
            ("tuning_names", format!("{:?}", self.tuning_names)),
            ("card_scores", format!("{:?}", self.card_scores)),
            ("qr_default_cars", format!("{:?}", self.qr_default_cars)),
            ("ranked_rides", format!("{:?}", self.ranked_rides)),
        ]
    }

    /// The tables whose SHA-256 (of the `Debug` text) is not the one the
    /// supported build gives ([`FIELD_PINS`]); empty when all equal.
    pub fn verify(&self) -> Vec<String> {
        self.field_texts()
            .into_iter()
            .filter(|(name, text)| {
                FIELD_PINS
                    .iter()
                    .find(|p| p.0 == *name)
                    .is_none_or(|p| crate::fingerprint::sha256_hex(text.as_bytes()) != p.1)
            })
            .map(|(name, _)| name.to_string())
            .collect()
    }
}

/// The SHA-256 of each table's `Debug` text as the supported build (NFSU2
/// PC 1.2) gives it: hashes only, never the data. A modified SPEED2.EXE is
/// accepted when every table matches ([`super::load`]).
pub const FIELD_PINS: &[(&str, &str)] = &[
    ("steer_response", "97c9d4db9902554aea087dddb38927c577d3fcafcf8d75c6adc56c7642d267fd"),
    ("steer_response_drift", "7128c7f08971f38da4ebc05425c1cf5d996ca28208a674cc8e877eba817cf775"),
    ("lateral", "7991663ac51f4456b95eb62d1773f4cf61f80f13edae1a206ee85f8f5d52ae49"),
    ("drift_tune", "401f0f31a5cdbeecbd76212ef69956fbb6f6689f99c522b69a480ea9c425c6df"),
    ("street_x_tune", "7984df633f3cb5c20c8b45a2c0c55d1b0d88a85b5dc4d62458c223d8a1d45fad"),
    ("drag_tune", "9cd9f05d36b372dc01e3e831e10dd152604a057b1614bce08ae88f9373683075"),
    ("zero_60_factor", "d833f6a592bfb8c0bb150dc30fe380a7436287988f525a50726f105b3c0ab144"),
    ("zero_100_factor", "73f01dfd85960f77cdd0b9245a11b3d66c037f04a6a1d9e8e5679c77e1127959"),
    ("surface_weight", "91ed557543e740c60c16f2aefac0cd640f5ac5c6d06c0033b7fb7e3e5ea07179"),
    ("shake", "c345a386246dcceff899bbfa60539e53bf198d736026d7951734d4c83d64d10c"),
    ("drift_level_speed", "837d136655ec5d2ea0a008cbf52097578f54ec79e51906693a88b3ae9d16ae54"),
    ("drift_level_points", "74fa6692ddf3e140ac5a6a56616678f69e1892067cf8f4a74bb6d0de4d221c58"),
    ("drift_virtual_scores", "f89278c88a3d6f099a1bfabf10a24acdb0be9178a2e81299e80ff2451a9e7b0c"),
    ("ai_lock", "9028802df21e737e41ea7f42ff600b5a3653c3d1c355611071959e01e54dcbbb"),
    ("ai_table_rate_speed", "421219fef71828c9ffc462d2db2ce1326922bddca5ea94763142cf17ca86c3ee"),
    ("asin", "cb074f073140e07a3d11d762c5d9944ff03afb910a9f029b9e42b8171acf63aa"),
    ("collision_levels", "c9e4241381d60e8775b46304967f7fb65499b7789eccea91604df6a8e11ac956"),
    ("collision_gain", "826ab4d85747e1608702d68ee452c8a7da57424669473805d15fc5cd826851fb"),
    ("db_to_lin", "1687d241da9111cfdf54a74fbaa0c788a6a9e517d0a730300d318b7c9129c098"),
    ("lin_to_db", "6372738eb668a6ef985f2439c90d21df15d5243753c78e6f2d4aaeea2c3899c8"),
    ("engine_levels", "a016675968f97f5b493403b7df93c5a732c2a6899c3b5e482131f5b123509514"),
    ("shifts", "55246f364c7680edfc6379b6e2642d77b186948797d8e5254a4df01cbb4527dd"),
    ("turbos", "438bcae3eed895f3cdee23ffc9ffdf9a4fecbc6288670f7c1a18d02190ac0dac"),
    ("swtn", "427023369ba87d9c41308c55ff674e5a124e754ffaed91bac02bb1d1f1b41948"),
    ("gins", "3aa880fd813232949b0750d6b8547eed90f4b63ca79f720517289e96579bfa82"),
    ("shift_recs", "318553225da4f9ad099a74ba5a2cccba80275eff86646631d5c4613db71ec6be"),
    ("launches", "bcc7bd5ff3483bc3f6e2b82995f1377d5d04bd0d034970e9fbec85cdb57f32b9"),
    ("launch_index", "0ff776193525cd76e513c3054ba8e56347351a2489e8525ca54a492056849b6a"),
    ("skid_records", "097fc8100dbb72ad110979a639c4840a5ac10ca18f416011070cd97259226870"),
    ("skid_class", "270733b70c9be7b4521be2af0b18cf8a627d3c70d6cca483a72c36f78133e552"),
    ("sound_index", "faf9398e2f8fde6eafc921bc57de38d060e140ee9758b5aa473f62ae4ba86d43"),
    ("engines", "75a7ab2fe917ae6712b7bedd65ba00430677ae3ee708284f865fc49443dd5f7c"),
    ("decels", "64bed80613638fbfeaeeb7a0d0de2589ad7538221dc215cd868cc3f369fb0040"),
    ("patterns", "1a6ef5491f327ad50372d7925109d0e08fd758db62518590f005dea8006d9711"),
    ("score_events", "77092a0893be1d5f5dc7f0e77d2de9b904de579035db35ed64526769cadbca78"),
    ("trains", "0c4fc3abe255e5cf5272cf0193955eb07fd3457c8d17a3963a0ece5dbd79277a"),
    ("utk_rc", "4e5dcb5047da02d7dd593b829c045b56e9ce10ee5450c973379270e83ec9b7d0"),
    ("utk_codebooks", "1838ca39000c86ad35e785d4dc62d69649cadcb99ee100c46246d4c384772662"),
    ("utk_commands", "f1ab7742a1c507ec463a173d43eb25acd2a4165d7b44ae8a464df94222ca5e6e"),
    ("mp3_huff", "b39a7c5a253b1b5caefcc601fa723646733a14b970a0684a5dc2a2c32829412b"),
    ("mp3_linbits", "b7e81c641166b01f75da998ec47a11c5886979ecee7de7e49047f8c4b9f8520c"),
    ("mp3_window_rows", "208f70e46530512ed6cf409c2b56d3eb5cfbb6ad405611785218206a98dcb0cf"),
    ("vp6", "bba46113a64edcd76a2868a304e61407b4656b088cd5759dbc49b1abd33df836"),
    ("in_game_rig", "374b868341104910743c56cb18b8193b94256382a5aa229179eadea0a8c53e2d"),
    ("default_rig", "f6dae390943fdb39da0b1684c55f3f02d589bc36aadcfebe0f5646d882a19f80"),
    ("slot_shots", "638b8afafb5796a98d98faccd90da2243f0be7829284acd6f29d95ce636ae10e"),
    ("vinyl_shots", "6a2aa13b820f262a335cd6aabeef269ae54aa6b6e726e5a1ae15891f8137e260"),
    ("suv_engine_shot", "d1a4befd0479cd2f1d5d1ac2d40f05f36be6f742e4bc9869d270404218fc78c3"),
    ("world_map_zooms", "9daf14fc727953fe4649ccb7a43db7882e7c90ca61367ea873eacdd778a1c624"),
    ("magazine_lines", "79f55c8d8cd6ef1b232eb8f5bdb41af91ac11bf288206d269b5cab58ddefad92"),
    ("dvd_lines", "edde62f9ff0733849a36cf326598ceb4e3d5a7920aa107bc2e21d7da98afa72b"),
    ("rating_lines", "66daefe78d65c72894300bad8e015b67824036ea37374f7a2ed95cb89ba9b49d"),
    ("part_type_names", "003a85d02507726f664ca10a0818d2c59c4eba71c20c0c5930851f3cbf96b777"),
    ("slot_types", "5e4e90db4101788ab7e3260131d358060c0d6fb4264dafb0d999ccfd6c5c5184"),
    ("slot_categories", "ce74331b9579407b1e31d508146c1737ea8c91c94254b54290fd140c0fea81e8"),
    ("vinyl_categories", "36cf1219ce39477b12c9eb45ef1d13cc3d78d58bb18dda6aa2680ad8980a2063"),
    ("opponent_cars", "8755e296db5a66d29e85c43c672ac5971225977728f923ff87e13ed3bd424722"),
    ("vinyl_groups", "586c490810b912985b5af2656a4b439e48c92eebbcead5c29e930fc795479376"),
    ("opponent_names", "cadbe9328922ad1e910d1d97d7b1c8df9124cd583aac132ed2accc49c50c09e6"),
    ("ea_trax", "f50c95034a0d803977a372045c30a2a4f5d2681bd20b4372bf2406d9583109c7"),
    ("ea_trax_where", "b329477db993a2f8881121438cc6a3d485236c60fe2ffca8166299f4e9b90d09"),
    ("tuning_names", "b2207292939ee822cf26145efa5d3cf89e0aaf3f4595360f92895de625a33b86"),
    ("card_scores", "743fdc46be9929eeccb70a9397d09c9fc8f03eaf6bbc8ab6f4f92ba0e5d1423e"),
    ("qr_default_cars", "4bf4e88fcf270fedb7c9ab1126d20bf33ab7ab74816395f48391320127a49af2"),
    ("ranked_rides", "8040550a1a0f45e23596219fe4f1da5efbbf56492cdda571ac0e559fb5b90ea6"),
];

/// Read every address [`Tables::load`] reads, ignoring the results (the
/// fuzz target: any bytes must give errors, never a panic or a hang).
pub(super) fn probe(exe: &Exe) {
    for va in PROBE_VAS {
        let _ = exe.bytes(va, 4096);
        let _ = exe.vec::<u32>(va, 64);
        let _ = exe.cstr(va);
        let _ = exe.cstr_at_ptr(va);
        let _ = exe.switch_returns(va, 32);
        let _ = exe.pushed_args(va, 0x54_5930, 28);
        let _ = exe.records_vec(va, 0x4C, 43, |r| Ok((r.str_ptr(0x0C).map(str::len), r.f32(0x1C))));
    }
    let _ = exe.initialised_dwords(0x50_1920, 0x83_7D30, 0xAA);
    let _ = exe.initialised_dwords_with(0x77_D3B0, 0x86_41A8, 0x12, &|_, p| p);
}

const PROBE_VAS: [u32; 16] = [
    0x7E_9228, 0x7F_C744, 0x86_B34C, 0x7F_0DE0, 0x45_8F28, 0x45_930C, 0x7D_0C30, 0x7C_4618, 0x54_6040, 0x50_1920,
    0x50_2020, 0x50_FF40, 0x77_D3B0, 0x80_333C, 0x7E_CD80, 0x7F_4608,
];
