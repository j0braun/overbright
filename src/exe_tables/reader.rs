//! A reader for the reference executable's data, for the data tests only
//! (docs/exe-tables.md, "Tests"): it maps a virtual address through the PE
//! section table and reads each table where the original keeps it, so the
//! tests can compare the clone's built-in tables with it. Nothing outside
//! the tests reads SPEED2.EXE.

use super::{Tables, Vp6Tables};
use crate::audio::car::skids::SkidRecord;
use crate::audio::car::transients::ShiftRec;
use crate::audio::engine_chain::{Rec80, Rec84};
use crate::garage::Shot;
use crate::render::native::car::lighting::RigLight;
use std::borrow::Cow;

/// Why a table cannot be read.
#[derive(Debug)]
pub enum Error {
    /// Not a Windows executable the section table can be read from.
    NotPe,
    /// A table's address range lies outside the executable's sections.
    Range { va: u32, len: usize },
    /// A table's contents are not what the reader expects (a string that
    /// does not end, a code pattern not found).
    Shape { va: u32, what: &'static str },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotPe => write!(f, "not a Windows program"),
            Error::Range { va, len } => write!(f, "table {va:#x} (+{len:#x}) lies outside the program"),
            Error::Shape { va, what } => write!(f, "table {va:#x}: {what}"),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// One section of the PE image: where it lives in memory and in the file.
#[derive(Clone, Copy, Debug)]
struct Section {
    va: u32,
    virtual_size: u32,
    raw_offset: u32,
    raw_size: u32,
}

/// The executable's bytes and its section table.
pub struct Exe {
    data: Vec<u8>,
    sections: Vec<Section>,
}

impl Exe {
    /// An executable from its bytes (its section table read).
    pub fn from_bytes(data: Vec<u8>) -> Result<Exe> {
        let bad = || Error::NotPe;
        let u16_at = |o: usize| data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(bad);
        let u32_at = |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or_else(bad);
        if data.get(..2) != Some(b"MZ") {
            return Err(bad());
        }
        let pe = u32_at(0x3C)? as usize;
        if data.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err(bad());
        }
        let count = u16_at(pe + 6)? as usize;
        let optional = u16_at(pe + 20)? as usize;
        let base = u32_at(pe + 24 + 28)?;
        let mut sections = Vec::with_capacity(count);
        for i in 0..count {
            let s = pe + 24 + optional + 40 * i;
            sections.push(Section {
                va: base.checked_add(u32_at(s + 12)?).ok_or_else(bad)?,
                virtual_size: u32_at(s + 8)?,
                raw_size: u32_at(s + 16)?,
                raw_offset: u32_at(s + 20)?,
            });
        }
        Ok(Exe { data, sections })
    }

    /// `len` bytes at virtual address `va`, from one section: the file's
    /// bytes, and zeros for the part past the section's raw data (memory
    /// the loader zero-fills).
    pub fn bytes(&self, va: u32, len: usize) -> Result<Cow<'_, [u8]>> {
        let range = || Error::Range { va, len };
        let end = (va as u64).checked_add(len as u64).ok_or_else(range)?;
        let s = self
            .sections
            .iter()
            .find(|s| va >= s.va && end <= s.va as u64 + s.virtual_size.max(s.raw_size) as u64)
            .ok_or_else(range)?;
        let at = (va - s.va) as usize;
        let raw = s.raw_size as usize;
        let file =
            |a: usize, b: usize| self.data.get(s.raw_offset as usize + a..s.raw_offset as usize + b).ok_or_else(range);
        if at + len <= raw {
            return Ok(Cow::Borrowed(file(at, at + len)?));
        }
        let mut v = vec![0u8; len];
        if at < raw {
            v[..raw - at].copy_from_slice(file(at, raw)?);
        }
        Ok(Cow::Owned(v))
    }

    /// `n` values of a plain type at `va`, little-endian as stored.
    pub fn vec<T: bytemuck::Pod>(&self, va: u32, n: usize) -> Result<Vec<T>> {
        let size = std::mem::size_of::<T>();
        let b = self.bytes(va, size * n)?;
        Ok(b.chunks_exact(size).map(bytemuck::pod_read_unaligned).collect())
    }

    /// `N` values of a plain type at `va`.
    pub fn array<T: bytemuck::Pod, const N: usize>(&self, va: u32) -> Result<[T; N]> {
        let v = self.vec::<T>(va, N)?;
        Ok(v.try_into().unwrap_or_else(|_| unreachable!()))
    }

    /// One value of a plain type at `va`.
    pub fn read<T: bytemuck::Pod>(&self, va: u32) -> Result<T> {
        Ok(self.vec::<T>(va, 1)?[0])
    }

    /// `N` records of `stride` bytes from `va`, each turned into a value by
    /// `f` from its bytes ([`Rec`]).
    pub fn records<'e, R, const N: usize>(
        &'e self,
        va: u32,
        stride: usize,
        f: impl for<'b> Fn(&Rec<'e, 'b>) -> Result<R>,
    ) -> Result<[R; N]> {
        let v = self.records_vec(va, stride, N, f)?;
        Ok(v.try_into().unwrap_or_else(|_| unreachable!()))
    }

    /// `n` records of `stride` bytes from `va` ([`Exe::records`]).
    pub fn records_vec<'e, R>(
        &'e self,
        va: u32,
        stride: usize,
        n: usize,
        f: impl for<'b> Fn(&Rec<'e, 'b>) -> Result<R>,
    ) -> Result<Vec<R>> {
        let all = self.bytes(va, stride * n)?;
        (0..n)
            .map(|i| f(&Rec { exe: self, va: va + (stride * i) as u32, b: &all[stride * i..stride * (i + 1)] }))
            .collect()
    }

    /// The NUL-terminated string at `va` (the executable's text is ASCII
    /// or Latin-1; at most 4096 bytes).
    pub fn cstr(&self, va: u32) -> Result<&str> {
        let s = self
            .sections
            .iter()
            .find(|s| va >= s.va && (va as u64) < s.va as u64 + s.raw_size as u64)
            .ok_or(Error::Range { va, len: 1 })?;
        let at = s.raw_offset as usize + (va - s.va) as usize;
        let end = s.raw_offset as usize + s.raw_size as usize;
        let tail = self.data.get(at..end.min(at + 4096)).ok_or(Error::Range { va, len: 1 })?;
        let n = tail.iter().position(|&c| c == 0).ok_or(Error::Shape { va, what: "a string without its end" })?;
        let b = &tail[..n];
        if b.is_ascii() {
            return Ok(std::str::from_utf8(b).unwrap_or_else(|_| unreachable!()));
        }
        // Latin-1 (accented names such as "Molé"): converted once and kept
        // for the process, like the tables that hold it.
        Ok(Box::leak(b.iter().map(|&c| c as char).collect::<String>().into_boxed_str()))
    }

    /// The string a pointer at `va` points to (`None` for a null pointer).
    pub fn cstr_at_ptr(&self, va: u32) -> Result<Option<&str>> {
        match self.read::<u32>(va)? {
            0 => Ok(None),
            p => self.cstr(p).map(Some),
        }
    }

    /// The values a compiled `switch` returns: `n` cases through the jump
    /// table at `table`, each case starting with `mov eax, imm32`
    /// (opcode 0xB8, imm32) or `xor eax, eax` (0).
    pub fn switch_returns(&self, table: u32, n: usize) -> Result<Vec<u32>> {
        self.vec::<u32>(table, n)?
            .into_iter()
            .map(|target| {
                let code = self.bytes(target, 8)?;
                // `pop r32` restoring saved registers may come first.
                let k = code.iter().take(3).take_while(|&&c| (0x58..=0x5F).contains(&c)).count();
                let code = &code[k..];
                match code[0] {
                    0xB8 => Ok(u32::from_le_bytes(code[1..5].try_into().unwrap())),
                    0x33 | 0x31 if code[1] == 0xC0 => Ok(0),
                    _ => Err(Error::Shape { va: target, what: "a switch case that does not load eax" }),
                }
            })
            .collect()
    }

    /// The immediate arguments of the first `calls` calls to `callee` in
    /// the code from `start`: `push imm32` (`68`) and `push imm8` (`6A`,
    /// sign extended) since the previous call, in push order. Other
    /// instructions between them must be ones [`code_len`] knows.
    pub fn pushed_args(&self, start: u32, callee: u32, calls: usize) -> Result<Vec<Vec<u32>>> {
        let code = self.bytes(start, 64 * calls + 64)?;
        let mut out = Vec::new();
        let mut args = Vec::new();
        let mut i = 0usize;
        while out.len() < calls {
            if i + 16 > code.len() {
                return Err(Error::Shape { va: start, what: "fewer calls than expected" });
            }
            let va = start + i as u32;
            match code[i] {
                0x68 => args.push(u32::from_le_bytes(code[i + 1..i + 5].try_into().unwrap())),
                0x6A => args.push(code[i + 1] as i8 as i32 as u32),
                0xE8 => {
                    let rel = i32::from_le_bytes(code[i + 1..i + 5].try_into().unwrap());
                    if va.wrapping_add(5).wrapping_add(rel as u32) == callee {
                        out.push(std::mem::take(&mut args));
                    } else {
                        args.clear();
                    }
                }
                _ => {}
            }
            i += code_len(&code[i..])
                .ok_or(Error::Shape { va, what: "an instruction the call reader does not know" })?;
        }
        Ok(out)
    }

    /// The dwords a straight-line initialiser at `start` stores into
    /// `[dest, dest + 4·n)`: the subset of x86 such start-up code uses
    /// (`xor r, r`, `mov r, imm32`, `mov [abs], r`, `mov [abs], imm32`,
    /// `mov edi, imm32` + `mov ecx, imm32` + `rep stosd`, `push` / `pop`
    /// of a register), up to its `ret`.
    pub fn initialised_dwords(&self, start: u32, dest: u32, n: usize) -> Result<Vec<u32>> {
        self.initialised_dwords_with(start, dest, n, &|_, _| None)
    }

    /// [`Exe::initialised_dwords`] for start-up code that also calls
    /// functions: `push imm32` keeps its value, `add esp, imm8` drops
    /// pushed values, and `call` sets eax to what `call` returns for the
    /// callee and the value pushed last (`None` refuses the call).
    pub fn initialised_dwords_with(
        &self,
        start: u32,
        dest: u32,
        n: usize,
        call: &dyn Fn(u32, Option<u32>) -> Option<u32>,
    ) -> Result<Vec<u32>> {
        let mut pushed: Vec<u32> = Vec::new();
        let mut mem = vec![None::<u32>; n];
        let mut reg = [0u32; 8];
        let mut va = start;
        let shape = |va| Error::Shape { va, what: "an instruction the initialiser reader does not know" };
        let store = |mem: &mut Vec<Option<u32>>, addr: u32, v: u32| {
            if addr >= dest && addr < dest + 4 * n as u32 && (addr - dest) % 4 == 0 {
                mem[((addr - dest) / 4) as usize] = Some(v);
            }
        };
        for _ in 0..4096 {
            // Up to 16 bytes (fewer at the end of a section), zero-padded.
            let mut c = [0u8; 16];
            let got = (1..=16).rev().find_map(|n| self.bytes(va, n).ok()).ok_or_else(|| shape(va))?;
            c[..got.len()].copy_from_slice(&got);
            let imm = |o: usize| u32::from_le_bytes(c[o..o + 4].try_into().unwrap());
            let len = match c[0] {
                0xC3 => return Ok(mem.into_iter().map(|v| v.unwrap_or(0)).collect()),
                0x50..=0x5F => 1,
                0x68 => {
                    pushed.push(imm(1));
                    5
                }
                0x83 if c[1] == 0xC4 => {
                    let k = (c[2] / 4) as usize;
                    pushed.truncate(pushed.len().saturating_sub(k));
                    3
                }
                0xE8 => {
                    let target = va.wrapping_add(5).wrapping_add(imm(1));
                    reg[0] = call(target, pushed.last().copied()).ok_or_else(|| shape(va))?;
                    5
                }
                0xB8..=0xBF => {
                    reg[(c[0] - 0xB8) as usize] = imm(1);
                    5
                }
                0x31 | 0x33 if c[1] >> 6 == 3 && (c[1] >> 3) & 7 == c[1] & 7 => {
                    reg[(c[1] & 7) as usize] = 0;
                    2
                }
                0xA3 => {
                    store(&mut mem, imm(1), reg[0]);
                    5
                }
                0x89 if c[1] & 0xC7 == 0x05 => {
                    store(&mut mem, imm(2), reg[((c[1] >> 3) & 7) as usize]);
                    6
                }
                0xC7 if c[1] == 0x05 => {
                    store(&mut mem, imm(2), imm(6));
                    10
                }
                0xF3 if c[1] == 0xAB => {
                    // rep stosd: [edi] = eax, ecx times.
                    for k in 0..reg[1] {
                        store(&mut mem, reg[7].wrapping_add(4 * k), reg[0]);
                    }
                    reg[7] = reg[7].wrapping_add(4 * reg[1]);
                    reg[1] = 0;
                    2
                }
                _ => return Err(shape(va)),
            };
            va += len;
        }
        Err(shape(va))
    }
}

/// The length of the instruction at the start of `code`, for the few
/// shapes [`Exe::pushed_args`] walks over (pushes, calls, `mov` between
/// registers and to `[reg + disp]`, `lea`); `None` for anything else.
fn code_len(code: &[u8]) -> Option<usize> {
    let modrm_len = |m: u8| -> Option<usize> {
        match m >> 6 {
            3 => Some(1),
            0 if m & 7 == 5 => Some(5),
            0 if m & 7 == 4 => None,
            0 => Some(1),
            1 if m & 7 == 4 => Some(3),
            1 => Some(2),
            2 if m & 7 == 4 => Some(6),
            2 => Some(5),
            _ => None,
        }
    };
    Some(match *code.first()? {
        0x50..=0x5F => 1,
        0x68 | 0xE8 | 0xE9 | 0xB8..=0xBF => 5,
        0x6A => 2,
        0x8B | 0x89 | 0x8D => 1 + modrm_len(*code.get(1)?)?,
        0xC7 => 1 + modrm_len(*code.get(1)?)? + 4,
        0xC6 => 1 + modrm_len(*code.get(1)?)? + 1,
        0x33 | 0x31 | 0x85 | 0x3B | 0x2B | 0x03 => 1 + modrm_len(*code.get(1)?)?,
        0x83 => 1 + modrm_len(*code.get(1)?)? + 1,
        0xC3 => 1,
        0xC2 => 3,
        0x90 => 1,
        _ => return None,
    })
}

/// One record of a table ([`Exe::records`]): typed reads at offsets.
pub struct Rec<'e, 'b> {
    exe: &'e Exe,
    va: u32,
    b: &'b [u8],
}

impl<'e, 'b> Rec<'e, 'b> {
    fn at<const N: usize>(&self, o: usize) -> [u8; N] {
        self.b[o..o + N].try_into().unwrap()
    }
    pub fn u16(&self, o: usize) -> u16 {
        u16::from_le_bytes(self.at(o))
    }
    pub fn u32(&self, o: usize) -> u32 {
        u32::from_le_bytes(self.at(o))
    }
    pub fn i32(&self, o: usize) -> i32 {
        i32::from_le_bytes(self.at(o))
    }
    pub fn f32(&self, o: usize) -> f32 {
        f32::from_le_bytes(self.at(o))
    }
    /// `N` f32 from offset `o`.
    pub fn f32s<const N: usize>(&self, o: usize) -> [f32; N] {
        std::array::from_fn(|k| self.f32(o + 4 * k))
    }
    /// The string the pointer at offset `o` points to ("" for null).
    pub fn str_ptr(&self, o: usize) -> Result<&'e str> {
        Ok(self.exe.cstr_at_ptr(self.va + o as u32)?.unwrap_or(""))
    }
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

/// Every table as the executable holds it (the tests' reference).
pub fn read_tables(exe: &'static Exe) -> Result<Tables> {
    {
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
                .map(|&(va, n)| {
                    let v = exe.records_vec(va, 4, n, |r| Ok((r.u16(0), byte(va, r.u16(2) as u32)?)))?;
                    Ok(&*Box::leak(v.into_boxed_slice()))
                })
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
