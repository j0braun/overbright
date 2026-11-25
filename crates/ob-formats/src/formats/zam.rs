//! A car's part animations, `CARS\<CAR>\PARTS_ANIMATIONS.BIN` (loaded per
//! car at 0x61C68B; the "PartsAnimations" object 0x438070 keeps the car's
//! name hash at +0x24): the hood, trunk and door `_ZAM_` animations.
//!
//! * Chunk 0x00E34010 (loader 0x4348F0, 16-byte aligned body): an EAGL4
//!   animation bank, an ELF32 relocatable object (`.data`, `.symtab`,
//!   `.rel.data`; the relocated words are `.data` offsets). The symbol
//!   `__AnimationBank:::bank` points at the bank: +0x04 count, +0x10 the
//!   animation pointers, +0x14 their name pointers. Every bank holds one
//!   part animation as two channels, `<CAR>_ZAM_<NAME>_q` (rotation) and
//!   `_t` (translation). A channel: +0x00 type 0x1400000F, +0x08 u16 1,
//!   +0x0A u16 frame count, +0x0C the key block: +0x08 components per key
//!   (4 for `_q`: x, y, z, w; 3 for a moving `_t`, 0 for none), +0x0C the
//!   frame count, keys from +0x20 (raw floats, one key per frame at 30
//!   frames a second; **verified** from the data: 31 keys of a 0…−46° turn
//!   about y for the 240SX's HOOD_REG, its last key fully open).
//! * Chunk 0x00037100 (0x4381CE, 16-byte aligned): +0x04 the car's name
//!   hash, then from +0x10 21 row-vector 4×4 rest matrices, the pivots
//!   0x433F40 hands out by channel and animation: hood REG, FRONT, LEFT /
//!   RIGHT / FRONT / BACK_HOOD_SPLIT (+0x10…+0x150); left door REG,
//!   SCISSOR, SUICIDE (+0x190…); right door REG, SCISSOR, SUICIDE
//!   (+0x250…); trunk REG (+0x310); left door WIDE1–4 (+0x350…); right
//!   door WIDE1–4 (+0x450…).

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use glam::{Mat4, Quat, Vec3};

use super::bundle::chunks;
use super::reader::{FormatResult, Reader};

/// Chunk ids.
pub const BANK: u32 = 0x00E3_4010;
pub const PIVOTS: u32 = 0x0003_7100;

/// The animation suffixes 0x433AD0 builds names from (`<CAR>` + suffix,
/// hashed with 0x43DB80), by channel (0 hood, 1 trunk, 2 / 3 left / right
/// door) and animation; with the rest matrix 0x433F40 pairs each with
/// (index into [`PartAnimations::pivots`]).
pub const NAMES: [&[(&str, usize)]; 4] = [
    &[
        ("_ZAM_HOOD_REG", 0),
        ("_ZAM_HOOD_FRONT", 1),
        ("_ZAM_LEFT_HOOD_SPLIT", 2),
        ("_ZAM_RIGHT_HOOD_SPLIT", 3),
        ("_ZAM_FRONT_HOOD_SPLIT", 4),
        ("_ZAM_BACK_HOOD_SPLIT", 5),
    ],
    &[("_ZAM_TRUNK_REG", 12)],
    &[
        ("_ZAM_LEFT_DOOR_REG", 6),
        ("_ZAM_LEFT_DOOR_SCISSOR", 7),
        ("_ZAM_LEFT_DOOR_SUICIDE", 8),
        ("_ZAM_LEFT_DOOR_WIDE1", 13),
        ("_ZAM_LEFT_DOOR_WIDE2", 14),
        ("_ZAM_LEFT_DOOR_WIDE3", 15),
        ("_ZAM_LEFT_DOOR_WIDE4", 16),
    ],
    &[
        ("_ZAM_RIGHT_DOOR_REG", 9),
        ("_ZAM_RIGHT_DOOR_SCISSOR", 10),
        ("_ZAM_RIGHT_DOOR_SUICIDE", 11),
        ("_ZAM_RIGHT_DOOR_WIDE1", 17),
        ("_ZAM_RIGHT_DOOR_WIDE2", 18),
        ("_ZAM_RIGHT_DOOR_WIDE3", 19),
        ("_ZAM_RIGHT_DOOR_WIDE4", 20),
    ],
];

/// One part animation: its rotation keys and, when it moves, its
/// translation keys (car space), a key per frame.
#[derive(Clone, Debug, Default)]
pub struct Animation {
    /// Native channel header +0x0A, copied into instance +0x64 (0x6E2110).
    pub frames: u16,
    pub rotations: Vec<Quat>,
    pub translations: Vec<Vec3>,
}

impl Animation {
    /// Native duration in frames includes the terminal frame; sampling
    /// itself clamps at the last key (0x6E7801).
    pub fn length(&self) -> f32 {
        self.frames as f32
    }

    /// Native 0x6E7790 clamps key indices; 0x6E7300 → 0x6E4450
    /// interpolates quaternions linearly along the short arc, then normalizes.
    /// Exact keys are copied without normalization (0x6E7670).
    pub fn sample(&self, frame: f32) -> (Quat, Option<Vec3>) {
        let at = |n: usize| -> (usize, usize, f32) {
            let f = frame.clamp(0.0, (n.max(1) - 1) as f32);
            let i = f.floor() as usize;
            (i, (i + 1).min(n.max(1) - 1), f - i as f32)
        };
        let q = if self.rotations.is_empty() {
            Quat::IDENTITY
        } else {
            let (i, j, s) = at(self.rotations.len());
            let a = self.rotations[i];
            if s == 0.0 || i == j { a } else {
                let b = self.rotations[j];
                let b = if a.dot(b) <= 0.0 { -b } else { b };
                (a + (b - a) * s).normalize()
            }
        };
        let t = if self.translations.is_empty() {
            None
        } else {
            let (i, j, s) = at(self.translations.len());
            Some(self.translations[i].lerp(self.translations[j], s))
        };
        (q, t.filter(|t| t.abs().max_element() >= 1e-6))
    }
}

/// A car's part animations.
#[derive(Clone, Debug, Default)]
pub struct PartAnimations {
    /// The rest matrices (chunk 0x37100), column-major car space.
    pub pivots: Vec<Mat4>,
    /// By suffix (`_ZAM_HOOD_REG` …).
    pub animations: HashMap<String, Animation>,
}

impl PartAnimations {
    /// `CARS\<car>\PARTS_ANIMATIONS.BIN`; None when the car has none (the
    /// game then has no "PartsAnimations" for it and 0x433CD0 answers 1,
    /// shut, for every channel).
    pub fn load(root: &Path, car: &str) -> Option<PartAnimations> {
        let dir = crate::game_fs::path(root, &format!("CARS/{car}"));
        let file = std::fs::read_dir(&dir).ok()?.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("PARTS_ANIMATIONS.BIN"))?;
        let data = crate::game_fs::read_file(&file.path()).ok()?;
        match PartAnimations::parse(&data, car) {
            Ok(a) => Some(a),
            Err(e) => {
                log::warn!("{car} PARTS_ANIMATIONS: {e:#}");
                None
            }
        }
    }

    pub fn parse(data: &[u8], car: &str) -> Result<PartAnimations> {
        let mut out = PartAnimations::default();
        let prefix = car.to_ascii_uppercase();
        for c in chunks(data, 0) {
            match c.id {
                BANK => {
                    for (name, keys) in parse_bank(c.aligned(16)).context("animation bank")? {
                        let Some(rest) = name.strip_prefix(&prefix) else { continue };
                        let (base, kind) = match (rest.strip_suffix("_q"), rest.strip_suffix("_t")) {
                            (Some(b), _) => (b, 'q'),
                            (_, Some(b)) => (b, 't'),
                            _ => continue,
                        };
                        let a = out.animations.entry(base.to_string()).or_default();
                        // Paired q/t duration headers agree in all 29 shipped banks.
                        a.frames = keys.2;
                        match (kind, keys.0) {
                            ('q', 4) => a.rotations = keys.1.chunks_exact(4).map(|k| Quat::from_xyzw(k[0], k[1], k[2], k[3])).collect(),
                            ('t', 3) => a.translations = keys.1.chunks_exact(3).map(|k| Vec3::new(k[0], k[1], k[2])).collect(),
                            _ => {}
                        }
                    }
                }
                PIVOTS => {
                    let mut b = Reader::new(c.aligned(16));
                    let matrix = |m: Reader<'_>| -> FormatResult<Mat4> {
                        let mut v = [0f32; 16];
                        for (i, x) in v.iter_mut().enumerate() {
                            *x = m.f32_at(i * 4)?;
                        }
                        Ok(Mat4::from_cols_array(&v))
                    };
                    out.pivots = match b.skip(0x10) {
                        Ok(()) => b.map_records(0x40, matrix)?,
                        Err(_) => Vec::new(),
                    };
                }
                _ => {}
            }
        }
        if out.pivots.len() < 21 {
            bail!("{} rest matrices", out.pivots.len());
        }
        Ok(out)
    }

    /// Channel `channel`'s animation `variant` (0x433AD0's second
    /// argument) and its rest matrix.
    pub fn get(&self, channel: usize, variant: usize) -> Option<(&Animation, Mat4)> {
        let &(name, pivot) = NAMES.get(channel)?.get(variant)?;
        Some((self.animations.get(name)?, *self.pivots.get(pivot)?))
    }
}

/// The channels of one bank: (name, (components per key, keys, duration)).
fn parse_bank(elf: &[u8]) -> Result<Vec<(String, (u32, Vec<f32>, u16))>> {
    let e = Reader::new(elf);
    if e.bytes_at(0, 4).ok() != Some(b"\x7fELF") {
        bail!("not an ELF object");
    }
    // Fuzzing: a cut-off header or symbol table is an error, not a panic.
    anyhow::ensure!(elf.len() >= 0x34, "ELF header");
    let shoff = e.u32_at(0x20)? as usize;
    let shnum = e.u16_at(0x30)? as usize;
    let section = |i: usize| -> Result<(u32, u32, usize, usize, u32)> {
        let s = e.sub_at(shoff + 40 * i, 40).ok().context("section header")?;
        Ok((s.u32_at(0)?, s.u32_at(4)?, s.u32_at(16)? as usize, s.u32_at(20)? as usize, s.u32_at(24)?))
    };
    let sections: Vec<_> = (0..shnum).map(section).collect::<Result<_>>()?;
    let (_, _, data_off, data_len, _) = *sections.get(1).context(".data")?;
    let data = e.sub_at(data_off, data_len).ok().context(".data bounds")?;
    let &(_, _, sym_off, sym_len, sym_link) = sections.iter().find(|s| s.1 == 2).context(".symtab")?;
    let &(_, _, str_off, _, _) = sections.get(sym_link as usize).context(".strtab")?;
    let cstr = |b: &Reader<'_>, at: usize| -> String { b.cstr_at(at, usize::MAX).unwrap_or_default() };
    // Compared in place: a copy of every symbol's name to its NUL was
    // quadratic in a table without NULs.
    let named_bank = |at: usize| e.data().get(at..).is_some_and(|s| s.starts_with(b"__AnimationBank"));
    let bank = (0..sym_len / 16)
        .map_while(|i| Some((e.u32_at(sym_off + 16 * i).ok()?, e.u32_at(sym_off + 16 * i + 4).ok()? as usize)))
        .find(|&(name, _)| named_bank(str_off + name as usize))
        .map(|(_, at)| at)
        .context("no bank symbol")?;
    let u = |at: usize| -> Result<usize> { Ok(data.u32_at(at).ok().context("bank bounds")? as usize) };
    let count = u(bank + 4)?;
    // The game has at most 21 channels per bank; every entry needs a 4-byte
    // pointer in `.data`, so a larger count is corrupt (and a count past
    // data.len() / 4 can not be backed by the tables anyway).
    anyhow::ensure!(count <= 256 && count <= data.len() / 4, "animation bank: {count} animations");
    let (anims, names) = (u(bank + 0x10)?, u(bank + 0x14)?);
    (0..count)
        .map(|i| {
            let a = u(anims + 4 * i)?;
            let name = cstr(&data, u(names + 4 * i)?);
            let keys = u(a + 0xC)?;
            let (comps, frames) = (u(keys + 8)? as u32, u(keys + 0xC)?);
            let n = (comps as usize).checked_mul(frames).context("key count")?;
            // The keys must fit in `.data`: no allocation before the check
            // (a shared key block times many animations amplified it).
            anyhow::ensure!(n <= data.len().saturating_sub(keys + 0x20) / 4, "animation keys: {n} floats do not fit");
            let floats = data.sub_at(keys + 0x20, n * 4).ok().context("keys")?.f32s("animation keys", n, usize::MAX)?;
            Ok((name, (comps, floats, data.u16_at(a + 0xA)?)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> Option<std::path::PathBuf> {
        std::env::var_os("NFSU2_ROOT").map(Into::into)
    }

    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn authored_rx8_samples_match_original_evaluator() {
        let bytes=&crate::testkit::private("docs/evidence/fe-authored-animation/samples.bin");
        for (case,b) in bytes.chunks_exact(52).enumerate() {
            let f=|i:usize| f32::from_le_bytes(b[i*4..i*4+4].try_into().unwrap());
            let a=Animation { rotations:vec![Quat::from_xyzw(f(0),f(1),f(2),f(3)),Quat::from_xyzw(f(4),f(5),f(6),f(7))],..Default::default() };
            let q=a.sample(f(8)).0;
            assert!(q.abs_diff_eq(Quat::from_xyzw(f(9),f(10),f(11),f(12)),0.0000003),"native case {case}: {q:?}");
        }
    }

    #[test]
    fn orthogonal_quaternion_keys_take_native_negative_arc() {
        // 0x6E447C tests the FPU flags C0 | C3: zero dot takes the negative endpoint too.
        let a=Animation { rotations:vec![Quat::IDENTITY,Quat::from_xyzw(1.0,0.0,0.0,0.0)],..Default::default() };
        assert!(a.sample(0.25).0.abs_diff_eq(Quat::from_xyzw(-0.25,0.0,0.0,0.75).normalize(),1e-7));
    }

    #[test]
    fn native_rx8_duration_includes_terminal_frame() {
        let Some(root)=root() else { return };
        let a=PartAnimations::load(&root,"RX8").expect("RX8 animations");
        let mut lengths:Vec<_>=a.animations.values().map(|a| a.length() as u32).collect();
        lengths.sort();
        assert_eq!(lengths,vec![27,28,28,28,28,28,28,28,28,28,28,30,30,30,30,30,30,31,31,31,31]);
    }

    #[test]
    fn the_240sx_hood_turns_up_about_its_rear_hinge() {
        let Some(root) = root() else { return };
        let a = PartAnimations::load(&root, "240SX").expect("240SX animations");
        assert_eq!(a.pivots.len(), 21);
        assert_eq!(a.animations.len(), 21);
        let (hood, rest) = a.get(0, 0).unwrap();
        assert_eq!(hood.rotations.len(), 31);
        assert_eq!(hood.length(), 31.0);
        // Rest: the hinge at x 0.856, z 0.623, no turn.
        assert!((rest.w_axis.x - 0.856).abs() < 1e-3 && (rest.w_axis.z - 0.623).abs() < 1e-3);
        // The last key turns the hood's front (+x of the hinge) up.
        let (q, t) = hood.sample(hood.length());
        assert!(t.is_none());
        assert!((q * Vec3::X).z > 0.6);
        // The trunk opens its rear (−x of its hinge) up.
        let (trunk, _) = a.get(1, 0).unwrap();
        assert!((trunk.sample(trunk.length()).0 * Vec3::NEG_X).z > 0.6);
        // A scissor door moves its hinge.
        let (scissor, _) = a.get(2, 1).unwrap();
        assert!(scissor.sample(0.0).1.is_some());
    }
}
