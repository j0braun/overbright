//! Tables the clone reads from the player's own SPEED2.EXE (clone tooling;
//! docs/exe-tables.md).
//!
//! Overbright ships nothing of the original game: the numbers, names and
//! curves the original keeps in its executable are read from the player's
//! copy when the game starts, not copied into the source. At start
//! [`init`] maps `SPEED2.EXE` (found without regard to case,
//! [`crate::game_fs`]; read-only) and builds every table once
//! ([`Tables`]). The build the clone is traced on (size and XXH3-64 of
//! [`crate::qol::setup::KEY_FILES`]) is taken as it is. Any other file (a
//! no-CD or widescreen patch of it, another version) is read the same way
//! and then every table is checked against the SHA-256 the source pins for
//! it ([`Tables::verify`]; the hashes, never the data): all equal → a
//! modified copy whose game data matches, accepted with a note; any
//! different (or not readable at its address) → refused with a
//! plain-words message naming the version. Each
//! table is named by its virtual address, element type and count; the PE
//! section table turns the address into a file offset (data past a
//! section's raw size is the zero-filled part the loader allocates), and
//! every read is range-checked.
//!
//! **One process-wide holder.** The tables are used from many places —
//! the audio threads, the car physics, the movie and audio decoders, the
//! front end and the formats — so they live in one read-once holder
//! ([`tables`]) rather than a struct handed down through every call: the
//! game builds it in `Game::new` before anything runs, and the frame path
//! only reads memory that is already there (an atomic load). Tools and
//! tests that never call [`init`] get it built on first use from
//! [`crate::game_root`].
//!
//! Tables that are pure maths (a sine or arctangent table) are computed
//! from their formula instead ([`generated`]); a data test checks that the
//! computed values equal the player's executable byte for byte.

pub mod generated;
mod tables;
#[cfg(test)]
mod tests;

pub use tables::*;

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The executable's name in the game folder.
pub const EXE: &str = "SPEED2.EXE";

/// Why the executable's tables cannot be read.
#[derive(Debug)]
pub enum Error {
    /// No SPEED2.EXE in the game folder.
    Missing(PathBuf),
    /// SPEED2.EXE is there but cannot be read or mapped.
    Unreadable(PathBuf, String),
    /// SPEED2.EXE is not the build the clone knows and its data tables
    /// differ from that build's (another version or layout): its size,
    /// the version if known, the tables that differ.
    TablesDiffer { path: PathBuf, size: u64, version: Option<&'static str>, tables: Vec<String> },
    /// Not a Windows executable the section table can be read from.
    NotPe(PathBuf),
    /// A table's address range lies outside the executable's sections.
    Range { va: u32, len: usize },
    /// A table's contents are not what the clone expects (a string that
    /// does not end, a code pattern not found).
    Shape { va: u32, what: &'static str },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Missing(p) => {
                write!(f, "{}", crate::qol::setup::exe_help(p, &crate::qol::setup::ExeProblem::Missing))
            }
            Error::Unreadable(p, e) => {
                write!(
                    f,
                    "{} cannot be read ({e}).\n{}",
                    p.display(),
                    crate::qol::setup::exe_help(p, &crate::qol::setup::ExeProblem::Missing)
                )
            }
            Error::TablesDiffer { path, size, version, tables } => write!(
                f,
                "{}",
                crate::qol::setup::exe_help(
                    path,
                    &crate::qol::setup::ExeProblem::Differs { size: *size, version: *version, tables }
                )
            ),
            Error::NotPe(p) => {
                write!(
                    f,
                    "{} is not a Windows program.\n{}",
                    p.display(),
                    crate::qol::setup::exe_help(p, &crate::qol::setup::ExeProblem::Missing)
                )
            }
            Error::Range { va, len } => write!(f, "SPEED2.EXE table {va:#x} (+{len:#x}) lies outside the program"),
            Error::Shape { va, what } => write!(f, "SPEED2.EXE table {va:#x}: {what}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// One section of the PE image: where it lives in memory and in the file.
#[derive(Clone, Copy, Debug)]
struct Section {
    va: u32,
    virtual_size: u32,
    raw_offset: u32,
    raw_size: u32,
}

/// The mapped executable and its section table.
pub struct Exe {
    data: Data,
    sections: Vec<Section>,
}

enum Data {
    Mapped(memmap2::Mmap),
    Owned(Vec<u8>),
}

/// Which SPEED2.EXE the game folder holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edition {
    /// The build the clone is traced on (size and hash).
    Known,
    /// Another file whose every table equals the known build's
    /// ([`Tables::verify`]): a patched copy (no-CD, widescreen).
    Modified { size: u64 },
}

impl std::ops::Deref for Data {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Data::Mapped(m) => m,
            Data::Owned(v) => v,
        }
    }
}

impl Exe {
    /// Map `root`'s SPEED2.EXE and tell whether it is the known build (a
    /// file of another size or hash still opens; [`load`] checks its
    /// tables).
    pub fn open(root: &Path) -> Result<(Exe, Edition)> {
        let path = crate::game_fs::find(root, EXE).ok_or_else(|| Error::Missing(root.join(EXE)))?;
        let file = std::fs::File::open(&path).map_err(|e| Error::Unreadable(path.clone(), e.to_string()))?;
        // SAFETY: the game folder is only read; nothing in the process
        // writes the file while it is mapped.
        let map = unsafe { memmap2::Mmap::map(&file) }.map_err(|e| Error::Unreadable(path.clone(), e.to_string()))?;
        Exe::checked(Data::Mapped(map), &path)
    }

    fn checked(data: Data, path: &Path) -> Result<(Exe, Edition)> {
        let (_, size, known) = crate::qol::setup::KEY_FILES[0];
        let len = data.len() as u64;
        if len == size && Some(xxhash_rust::xxh3::xxh3_64(&data)) == known {
            return Ok((Exe::parse(data, path)?, Edition::Known));
        }
        match Exe::parse(data, path) {
            Ok(e) => Ok((e, Edition::Modified { size: len })),
            // Not even a section table: certainly not the supported build.
            Err(_) => Err(Error::TablesDiffer {
                path: path.to_path_buf(),
                size: len,
                version: None,
                tables: vec!["the section table".into()],
            }),
        }
    }

    /// This file's version name when the fingerprint knows it (SHA-256).
    fn version_name(&self) -> Option<&'static str> {
        crate::fingerprint::other_exe(EXE, self.data.len() as u64, &crate::fingerprint::sha256_hex(&self.data))
    }

    /// An executable from bytes, without the edition check (tests of the
    /// section reader, the fuzz target).
    fn from_bytes(bytes: Vec<u8>) -> Result<Exe> {
        Exe::parse(Data::Owned(bytes), Path::new(EXE))
    }

    fn parse(data: Data, path: &Path) -> Result<Exe> {
        let bad = || Error::NotPe(path.to_path_buf());
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
    /// (`B8 imm32`) or `xor eax, eax` (0).
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
    pub fn u8(&self, o: usize) -> u8 {
        self.b[o]
    }
    pub fn i8(&self, o: usize) -> i8 {
        self.b[o] as i8
    }
    pub fn u16(&self, o: usize) -> u16 {
        u16::from_le_bytes(self.at(o))
    }
    pub fn i16(&self, o: usize) -> i16 {
        i16::from_le_bytes(self.at(o))
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
    /// A NUL-terminated string stored inline at offset `o`.
    pub fn str_inline(&self, o: usize) -> Result<&'e str> {
        self.exe.cstr(self.va + o as u32)
    }
}

/// A table as its users name it: a `static` that reads as the table
/// (through `Deref`), served from [`tables`]. The source keeps the name,
/// type and address of each table and none of its data.
pub struct Table<T: ?Sized + 'static>(fn(&'static Tables) -> &'static T);

impl<T: ?Sized + 'static> Table<T> {
    pub const fn new(field: fn(&'static Tables) -> &'static T) -> Table<T> {
        Table(field)
    }
    /// The table ([`tables`]).
    #[inline]
    pub fn loaded(&self) -> &'static T {
        (self.0)(tables())
    }
}

impl<T: ?Sized + 'static> std::ops::Deref for Table<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        self.loaded()
    }
}

impl<T: std::fmt::Debug + ?Sized + 'static> std::fmt::Debug for Table<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.loaded().fmt(f)
    }
}

static EXE_MAP: OnceLock<&'static Exe> = OnceLock::new();
static TABLES: OnceLock<&'static Tables> = OnceLock::new();

/// Map `root`'s executable, read every table and accept it ([`Edition`]):
/// the known build as it is; another file only when [`Tables::verify`]
/// finds every table equal to the known build's, else
/// [`Error::TablesDiffer`]. The executable and the tables are kept for the
/// process (a check screen that calls this for another folder keeps one
/// more mapping).
pub fn load(root: &Path) -> Result<(&'static Exe, &'static Tables, Edition)> {
    let (exe, edition) = Exe::open(root)?;
    let path = crate::game_fs::find(root, EXE).unwrap_or_else(|| root.join(EXE));
    let exe: &'static Exe = Box::leak(Box::new(exe));
    let differ = |tables: Vec<String>| Error::TablesDiffer {
        path: path.clone(),
        size: exe.data.len() as u64,
        version: exe.version_name(),
        tables,
    };
    let tables = match (Tables::load(exe), edition) {
        (Ok(t), _) => t,
        (Err(e), Edition::Known) => return Err(e),
        (Err(e), Edition::Modified { .. }) => return Err(differ(vec![e.to_string()])),
    };
    if let Edition::Modified { .. } = edition {
        let bad = tables.verify();
        if !bad.is_empty() {
            return Err(differ(bad));
        }
    }
    Ok((exe, Box::leak(Box::new(tables)), edition))
}

/// Whether `root`'s SPEED2.EXE can be used, for `--check-game` and the
/// launcher's check screen: the known build without reading its tables,
/// any other file through [`load`].
pub fn check_folder(root: &Path) -> Result<Edition> {
    match Exe::open(root)? {
        (_, Edition::Known) => Ok(Edition::Known),
        _ => load(root).map(|(_, _, e)| e),
    }
}

/// Map the player's executable under `root`, check it and read every
/// table (the game's start, `Game::new`). Once per process: a later call
/// returns the tables already read.
pub fn init(root: &Path) -> Result<&'static Tables> {
    if let Some(&t) = TABLES.get() {
        return Ok(t);
    }
    let _t = crate::prof::span("exe_tables::init");
    let (exe, tables, edition) = load(root)?;
    if let Edition::Modified { size } = edition {
        log::info!("{} ({size} bytes)", crate::qol::setup::exe_modified_line());
    }
    let _ = EXE_MAP.set(exe);
    Ok(*TABLES.get_or_init(|| tables))
}

/// The fuzz target's entry ([`fuzz/targets.rs`]): parse any bytes as an
/// executable and read every table address the clone uses; errors are
/// fine, a panic or hang is not.
pub fn fuzz_probe(bytes: &[u8]) {
    let Ok(exe) = Exe::from_bytes(bytes.to_vec()) else { return };
    tables::probe(&exe);
}

/// The tables read at start ([`init`]). A program that never called
/// [`init`] (a tool, a test) reads them here on first use from
/// [`crate::game_root`]; that failing is fatal with the plain message.
#[inline]
pub fn tables() -> &'static Tables {
    match TABLES.get() {
        Some(&t) => t,
        None => tables_cold(),
    }
}

#[cold]
#[inline(never)]
fn tables_cold() -> &'static Tables {
    init(&crate::game_root()).unwrap_or_else(|e| panic!("{e}"))
}

/// The mapped executable ([`init`]), for string access by address.
pub fn exe() -> &'static Exe {
    tables();
    EXE_MAP.get().copied().expect("the executable is mapped with the tables")
}
