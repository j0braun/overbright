//! A bounds-checked cursor over file bytes, and the limits a parser holds
//! counts and allocations to.
//!
//! Every read returns `Result<_, FormatError>`: past the end is
//! [`FormatError::UnexpectedEof`], never a panic. A count taken from the
//! file is checked against the bytes that are really left
//! ([`Reader::count`]) and against a [`Limits`] maximum before anything is
//! allocated for it.
//!
//! Two ways to read:
//! * sequential: `u32()`, `bytes(n)`, `sub(n)`, ... advance the cursor;
//! * positional: `u32_at(at)`, `bytes_at(at, n)`, ... read at an offset
//!   from the reader's start and leave the cursor alone (records with
//!   fields at fixed offsets).
//!
//! Offsets in errors are relative to the reader the error came from plus
//! its `base` (the offset of a sub-reader inside its parent, so errors from
//! a sub-reader name the place in the outermost buffer).

use std::fmt;

/// What went wrong reading a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
    /// `need` bytes were wanted at offset `at`, but the data ends before.
    UnexpectedEof { at: usize, need: usize },
    /// A count or size from the file (`got`) is above what any shipped
    /// file uses by a wide margin (`max`, see [`Limits`]).
    LimitExceeded { what: &'static str, got: usize, max: usize },
    /// A value that cannot be right (`what`) at offset `at`.
    Invalid { what: &'static str, at: usize },
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::UnexpectedEof { at, need } => {
                write!(f, "unexpected end of data: {need} bytes wanted at {at:#x}")
            }
            FormatError::LimitExceeded { what, got, max } => write!(f, "{what}: {got} is above the limit {max}"),
            FormatError::Invalid { what, at } => write!(f, "invalid {what} at {at:#x}"),
        }
    }
}

impl std::error::Error for FormatError {}

pub type FormatResult<T> = Result<T, FormatError>;

/// Upper bounds for counts and sizes taken from a file.
///
/// The defaults sit at least 4x above the largest value in the shipped game
/// (NFSU2 PC, every `.BIN`/`.BUN`/`.LZC` under the game root and the BIG
/// archive, measured 2026-11-11 with a throw-away probe over every chunk
/// tree, compressed block, texture pack, mesh, parameter map and animation
/// bank):
///
/// | field | largest shipped value | where | default | margin |
/// |---|---|---|---|---|
/// | `max_unpacked` (codec output bytes) | 8,008,064 | GLOBAL/GlobalB.lzc (JDLZ) | 64 MiB | 8x |
/// | `max_texture_dim` (texels per side) | 1,024 | GLOBAL/GlobalB.lzc and others | 8,192 | 8x |
/// | `max_textures` (entries per pack) | 1,790 | CARS/MUSTANGGT/VINYLS.BIN | 16,384 | 9x |
/// | `max_count` (records of one kind: mesh vertices, materials, parameter-map values) | 15,618 vertices | TRACKS/STREAML4RC.BUN | 1,048,576 | 67x |
/// | `max_alloc` (bytes of one table or archive entry) | 25,595,192 (largest chunk body) | TRACKS/STREAML4RA.BUN texture pack | 256 MiB | 10x |
/// | `max_anim_values` (decoded floats of one animation bank) | 300,888 (one channel: 88,935) | NIS/Scene_EndingNis03_BundleB.bun | 4,194,304 | 13x |
///
/// Other maxima the probe saw, for reference: chunk nesting depth 4
/// (`bundle::walk` is iterative and needs no limit), 10 entries in
/// SDATA/sdat.viv, 363 frames and 22 channels per animation bank, a
/// compressed texture block of 1,398,172 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Bytes a decompressor may produce for one block.
    pub max_unpacked: usize,
    /// Width or height of one texture.
    pub max_texture_dim: u32,
    /// Texture entries in one pack.
    pub max_textures: usize,
    /// Records of one kind in one table.
    pub max_count: usize,
    /// Bytes one table (or archive entry) may take once parsed.
    pub max_alloc: usize,
    /// Decoded animation values (floats) of one bank, and of one channel.
    pub max_anim_values: usize,
}

impl Limits {
    pub const DEFAULT: Limits = Limits {
        max_unpacked: 64 << 20,
        max_texture_dim: 8192,
        max_textures: 16384,
        max_count: 1 << 20,
        max_alloc: 256 << 20,
        max_anim_values: 4 << 20,
    };

    /// `got` if it is at most `max`, else [`FormatError::LimitExceeded`].
    pub fn check(what: &'static str, got: usize, max: usize) -> FormatResult<usize> {
        if got > max { Err(FormatError::LimitExceeded { what, got, max }) } else { Ok(got) }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Limits::DEFAULT
    }
}

/// A cursor over `&[u8]`; every read is checked.
#[derive(Clone, Copy, Debug)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    /// Offset of `data[0]` in the outermost buffer (for error offsets).
    base: usize,
}

macro_rules! scalar {
    ($name:ident, $name_at:ident, $t:ty) => {
        #[doc = concat!("A little-endian `", stringify!($t), "` at the cursor.")]
        #[inline]
        pub fn $name(&mut self) -> FormatResult<$t> {
            let v = self.$name_at(self.pos)?;
            self.pos += std::mem::size_of::<$t>();
            Ok(v)
        }

        #[doc = concat!("A little-endian `", stringify!($t), "` at `at` (from the reader's start); the cursor stays.")]
        #[inline]
        pub fn $name_at(&self, at: usize) -> FormatResult<$t> {
            Ok(<$t>::from_le_bytes(self.array_at(at)?))
        }
    };
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0, base: 0 }
    }

    /// A reader whose errors report offsets from `base` (where `data`
    /// starts in the file).
    pub fn with_base(data: &'a [u8], base: usize) -> Self {
        Reader { data, pos: 0, base }
    }

    /// The whole buffer this reader covers.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// The cursor, from the reader's start.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Bytes from the cursor to the end.
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// The bytes from the cursor to the end (the cursor stays).
    pub fn rest(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    fn eof(&self, at: usize, need: usize) -> FormatError {
        FormatError::UnexpectedEof { at: self.base.saturating_add(at), need }
    }

    /// An [`FormatError::Invalid`] at `at` (from the reader's start).
    pub fn invalid(&self, what: &'static str, at: usize) -> FormatError {
        FormatError::Invalid { what, at: self.base.saturating_add(at) }
    }

    /// Move the cursor to `pos` (at most the length).
    pub fn seek(&mut self, pos: usize) -> FormatResult<()> {
        if pos > self.data.len() {
            return Err(self.eof(pos, 0));
        }
        self.pos = pos;
        Ok(())
    }

    /// Move the cursor `n` bytes on.
    pub fn skip(&mut self, n: usize) -> FormatResult<()> {
        if n > self.remaining() {
            return Err(self.eof(self.pos, n));
        }
        self.pos += n;
        Ok(())
    }

    /// `n` bytes at `at`; the cursor stays.
    #[inline]
    pub fn bytes_at(&self, at: usize, n: usize) -> FormatResult<&'a [u8]> {
        match at.checked_add(n) {
            Some(end) if end <= self.data.len() => Ok(&self.data[at..end]),
            _ => Err(self.eof(at, n)),
        }
    }

    /// `n` bytes at the cursor.
    #[inline]
    pub fn bytes(&mut self, n: usize) -> FormatResult<&'a [u8]> {
        let b = self.bytes_at(self.pos, n)?;
        self.pos += n;
        Ok(b)
    }

    /// `N` bytes at `at` as an array; the cursor stays.
    #[inline]
    pub fn array_at<const N: usize>(&self, at: usize) -> FormatResult<[u8; N]> {
        let b = self.bytes_at(at, N)?;
        let mut a = [0u8; N];
        a.copy_from_slice(b);
        Ok(a)
    }

    /// `N` bytes at the cursor as an array.
    #[inline]
    pub fn array<const N: usize>(&mut self) -> FormatResult<[u8; N]> {
        let a = self.array_at(self.pos)?;
        self.pos += N;
        Ok(a)
    }

    scalar!(u8, u8_at, u8);
    scalar!(u16, u16_at, u16);
    scalar!(i16, i16_at, i16);
    scalar!(u32, u32_at, u32);
    scalar!(i32, i32_at, i32);
    scalar!(f32, f32_at, f32);

    /// Three `f32` at `at`; the cursor stays.
    pub fn vec3_at(&self, at: usize) -> FormatResult<[f32; 3]> {
        self.bytes_at(at, 12)?;
        Ok([self.f32_at(at)?, self.f32_at(at + 4)?, self.f32_at(at + 8)?])
    }

    /// Three `f32` at the cursor.
    pub fn vec3(&mut self) -> FormatResult<[f32; 3]> {
        let v = self.vec3_at(self.pos)?;
        self.pos += 12;
        Ok(v)
    }

    /// A reader over the `n` bytes at `at` (errors keep naming offsets in
    /// this reader's buffer); the cursor stays.
    pub fn sub_at(&self, at: usize, n: usize) -> FormatResult<Reader<'a>> {
        let data = self.bytes_at(at, n)?;
        Ok(Reader { data, pos: 0, base: self.base.saturating_add(at) })
    }

    /// A reader over the next `n` bytes; the cursor moves past them.
    pub fn sub(&mut self, n: usize) -> FormatResult<Reader<'a>> {
        let r = self.sub_at(self.pos, n)?;
        self.pos += n;
        Ok(r)
    }

    /// A NUL-terminated string in a fixed field of `max` bytes at `at`,
    /// lossy UTF-8. The field may be cut short by the end of the data (as
    /// `bundle::read_cstr`), but `at` itself must be inside it.
    pub fn cstr_at(&self, at: usize, max: usize) -> FormatResult<String> {
        if at > self.data.len() {
            return Err(self.eof(at, max));
        }
        let field = &self.data[at..at.saturating_add(max).min(self.data.len())];
        let len = field.iter().position(|&c| c == 0).unwrap_or(field.len());
        Ok(String::from_utf8_lossy(&field[..len]).into_owned())
    }

    /// A fixed `n`-byte string field at the cursor (all `n` bytes must be
    /// there), cut at its first NUL, lossy UTF-8.
    pub fn fixed_str(&mut self, n: usize) -> FormatResult<String> {
        let field = self.bytes(n)?;
        let len = field.iter().position(|&c| c == 0).unwrap_or(n);
        Ok(String::from_utf8_lossy(&field[..len]).into_owned())
    }

    /// The bytes from the cursor up to the next NUL (the cursor moves past
    /// the NUL). A string without a NUL before the end is an error.
    pub fn cstr(&mut self) -> FormatResult<&'a [u8]> {
        let rest = self.rest();
        let Some(len) = rest.iter().position(|&c| c == 0) else {
            return Err(self.eof(self.data.len(), 1));
        };
        self.pos += len + 1;
        Ok(&rest[..len])
    }

    /// `n` records of `stride` bytes from the cursor: `n` checked against
    /// `max` and against the bytes that are left, before any allocation.
    pub fn count(&self, what: &'static str, n: usize, stride: usize, max: usize) -> FormatResult<usize> {
        Limits::check(what, n, max)?;
        match n.checked_mul(stride) {
            Some(bytes) if bytes <= self.remaining() => Ok(n),
            _ => Err(self.eof(self.pos, n.saturating_mul(stride))),
        }
    }

    /// The next `n` records of `stride` bytes as slices (count checked as
    /// in [`Reader::count`]); the cursor moves past them.
    pub fn records(
        &mut self,
        what: &'static str,
        n: usize,
        stride: usize,
        max: usize,
    ) -> FormatResult<std::slice::ChunksExact<'a, u8>> {
        self.count(what, n, stride, max)?;
        let b = self.bytes(n * stride)?;
        // `chunks_exact(0)` panics; a zero stride has no records to give.
        Ok(b.chunks_exact(stride.max(1)))
    }

    /// The whole `stride`-byte records from the cursor to the end, each as
    /// a reader (a cut-off tail is left out, as `chunks_exact`); the cursor
    /// stays. A zero stride gives none.
    pub fn whole_records(&self, stride: usize) -> impl ExactSizeIterator<Item = Reader<'a>> + use<'a> {
        let (base, pos) = (self.base, self.pos);
        let n = if stride == 0 { 0 } else { self.remaining() / stride };
        self.data[pos..pos + n * stride].chunks_exact(stride.max(1)).enumerate().map(move |(i, data)| Reader {
            data,
            pos: 0,
            base: base.saturating_add(pos + i * stride),
        })
    }

    /// `f` over every whole `stride`-byte record from the cursor (see
    /// [`Reader::whole_records`]), the first error ending it.
    pub fn map_records<T>(&self, stride: usize, f: impl FnMut(Reader<'a>) -> FormatResult<T>) -> FormatResult<Vec<T>> {
        self.whole_records(stride).map(f).collect()
    }

    /// `n` little-endian `u16` at the cursor.
    pub fn u16s(&mut self, what: &'static str, n: usize, max: usize) -> FormatResult<Vec<u16>> {
        Ok(self.records(what, n, 2, max)?.map(|c| u16::from_le_bytes([c[0], c[1]])).collect())
    }

    /// `n` little-endian `u32` at the cursor.
    pub fn u32s(&mut self, what: &'static str, n: usize, max: usize) -> FormatResult<Vec<u32>> {
        Ok(self.records(what, n, 4, max)?.map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
    }

    /// `n` little-endian `f32` at the cursor.
    pub fn f32s(&mut self, what: &'static str, n: usize, max: usize) -> FormatResult<Vec<f32>> {
        Ok(self.records(what, n, 4, max)?.map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: [u8; 12] = [0x01, 0x02, 0x03, 0x04, 0x00, 0x00, 0x80, 0x3F, b'a', b'b', 0, b'c'];

    #[test]
    fn scalars_at_the_boundary() {
        let r = Reader::new(&DATA);
        assert_eq!(r.u8_at(11), Ok(b'c'));
        assert_eq!(r.u8_at(12), Err(FormatError::UnexpectedEof { at: 12, need: 1 }));
        assert_eq!(r.u16_at(10), Ok(u16::from_le_bytes([0, b'c'])));
        assert!(r.u16_at(11).is_err());
        assert_eq!(r.i16_at(0), Ok(0x0201));
        assert!(r.i16_at(11).is_err());
        assert_eq!(r.u32_at(0), Ok(0x0403_0201));
        assert_eq!(r.u32_at(8), Ok(u32::from_le_bytes([b'a', b'b', 0, b'c'])));
        assert!(r.u32_at(9).is_err());
        assert_eq!(r.i32_at(4), Ok(0x3F80_0000));
        assert!(r.i32_at(usize::MAX).is_err());
        assert_eq!(r.f32_at(4), Ok(1.0));
        assert!(r.f32_at(9).is_err());
        assert_eq!(r.vec3_at(0).map(|v| v[1]), Ok(1.0));
        assert!(r.vec3_at(1).is_err());
        assert_eq!(r.array_at::<12>(0), Ok(DATA));
        assert!(r.array_at::<13>(0).is_err());
    }

    #[test]
    fn sequential_reads_advance_and_stop_at_the_end() {
        let mut r = Reader::new(&DATA);
        assert_eq!(r.u32(), Ok(0x0403_0201));
        assert_eq!(r.position(), 4);
        assert_eq!(r.f32(), Ok(1.0));
        assert_eq!(r.remaining(), 4);
        assert_eq!(r.u16(), Ok(u16::from_le_bytes([b'a', b'b'])));
        // A failed read leaves the cursor where it was.
        assert_eq!(r.u32(), Err(FormatError::UnexpectedEof { at: 10, need: 4 }));
        assert_eq!(r.position(), 10);
        assert_eq!(r.i16(), Ok(i16::from_le_bytes([0, b'c'])));
        assert_eq!(r.remaining(), 0);
        assert!(r.u8().is_err());
        let mut r = Reader::new(&DATA);
        assert_eq!(r.u8(), Ok(1));
        assert_eq!(r.i32(), Ok(i32::from_le_bytes([2, 3, 4, 0])));
        assert!(r.vec3().is_err());
        assert_eq!(r.array::<2>(), Ok([0, 0x80]));
        let mut r = Reader::new(&DATA[..12]);
        assert!(r.vec3().is_ok());
        assert!(r.vec3().is_err());
    }

    #[test]
    fn bytes_skip_seek() {
        let mut r = Reader::new(&DATA);
        assert_eq!(r.bytes(0), Ok(&[][..]));
        assert_eq!(r.bytes(12).map(|b| b.len()), Ok(12));
        assert!(r.bytes(1).is_err());
        assert_eq!(r.seek(12), Ok(()));
        assert!(r.seek(13).is_err());
        assert_eq!(r.position(), 12);
        r.seek(0).unwrap();
        assert!(r.skip(13).is_err());
        assert_eq!(r.skip(12), Ok(()));
        assert!(r.skip(1).is_err());
        assert_eq!(r.rest(), &[][..]);
        assert!(r.bytes_at(usize::MAX, 2).is_err());
        assert_eq!(r.bytes_at(12, 0), Ok(&[][..]));
        assert!(r.bytes_at(13, 0).is_err());
    }

    #[test]
    fn sub_readers_are_bounded_and_report_outer_offsets() {
        let mut r = Reader::with_base(&DATA, 100);
        r.skip(4).unwrap();
        let mut s = r.sub(4).unwrap();
        assert_eq!(r.position(), 8);
        assert_eq!(s.len(), 4);
        assert_eq!(s.f32(), Ok(1.0));
        assert_eq!(s.u8(), Err(FormatError::UnexpectedEof { at: 108, need: 1 }));
        assert!(r.sub(5).is_err());
        assert_eq!(r.sub(4).map(|s| s.len()), Ok(4));
        assert!(r.sub_at(8, 5).is_err());
        assert_eq!(r.sub_at(12, 0).map(|s| s.is_empty()), Ok(true));
        assert_eq!(r.invalid("x", 3), FormatError::Invalid { what: "x", at: 103 });
    }

    #[test]
    fn strings() {
        let r = Reader::new(&DATA);
        assert_eq!(r.cstr_at(8, 4).as_deref(), Ok("ab"));
        // The field is cut at the end of the data, as bundle::read_cstr.
        assert_eq!(r.cstr_at(11, 8).as_deref(), Ok("c"));
        assert_eq!(r.cstr_at(12, 8).as_deref(), Ok(""));
        assert!(r.cstr_at(13, 8).is_err());
        let mut r = Reader::new(&DATA);
        r.seek(8).unwrap();
        assert_eq!(r.fixed_str(4).as_deref(), Ok("ab"));
        assert!(r.fixed_str(1).is_err());
        r.seek(8).unwrap();
        assert_eq!(r.cstr(), Ok(&b"ab"[..]));
        assert_eq!(r.position(), 11);
        // "c" has no NUL before the end.
        assert!(r.cstr().is_err());
        assert_eq!(r.position(), 11);
    }

    #[test]
    fn counts_and_arrays() {
        let mut r = Reader::new(&DATA);
        assert_eq!(r.count("x", 3, 4, 10), Ok(3));
        assert!(matches!(r.count("x", 4, 4, 10), Err(FormatError::UnexpectedEof { .. })));
        assert_eq!(r.count("x", 11, 0, 10), Err(FormatError::LimitExceeded { what: "x", got: 11, max: 10 }));
        assert!(r.count("x", usize::MAX, 2, usize::MAX).is_err());
        assert_eq!(r.count("x", 5, 0, 10), Ok(5));
        assert_eq!(r.records("x", 5, 0, 10).map(|c| c.count()), Ok(0));
        assert_eq!(r.u16s("x", 2, 2), Ok(vec![0x0201, 0x0403]));
        assert!(r.u32s("x", 3, 3).is_err());
        assert_eq!(r.f32s("x", 1, 1), Ok(vec![1.0]));
        assert_eq!(r.u32s("x", 1, 1).map(|v| v.len()), Ok(1));
        assert_eq!(r.u32s("x", 0, 0), Ok(vec![]));
        assert!(r.u16s("x", 1, 1).is_err());
        let mut r = Reader::new(&DATA);
        assert!(r.u32s("x", 3, 2).is_err());
        assert_eq!(r.records("x", 2, 6, 2).map(|c| c.count()), Ok(2));
    }

    #[test]
    fn whole_records_leave_out_a_cut_off_tail() {
        let mut r = Reader::with_base(&DATA, 10);
        r.skip(1).unwrap();
        let recs: Vec<_> = r.whole_records(4).collect();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[1].u8_at(0), Ok(0));
        assert_eq!(recs[1].u8_at(4), Err(FormatError::UnexpectedEof { at: 19, need: 1 }));
        assert_eq!(r.whole_records(11).count(), 1);
        assert_eq!(r.whole_records(12).count(), 0);
        assert_eq!(r.whole_records(0).count(), 0);
        assert_eq!(r.position(), 1);
        assert_eq!(r.map_records(4, |r| r.u8_at(0)), Ok(vec![2, 0]));
        assert!(r.map_records(4, |r| r.u8_at(4)).is_err());
    }

    #[test]
    fn errors_print_and_convert_to_anyhow() {
        let e: anyhow::Error = FormatError::LimitExceeded { what: "textures", got: 9, max: 8 }.into();
        assert_eq!(e.to_string(), "textures: 9 is above the limit 8");
        assert!(Limits::check("x", 8, 8).is_ok());
        assert_eq!(Limits::default(), Limits::DEFAULT);
    }
}
