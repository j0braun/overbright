//! The flat format parsed data is stored in: `Writer`, `Reader`, `Persist` and the `persist!` macros.

use super::*;

// ---------------------------------------------------------------------------
// The flat format the cache stores parsed data in.
//
// Little-endian, field by field in declaration order; floats as their bits
// (so a value comes back bit for bit); maps sorted by key (so the same data
// always gives the same bytes, which `--verify-cache` relies on). Large
// plain-data arrays (vertices, indices) are one copy each way, texture
// bytes are not copied at all on the way in: they stay in the mapped file
// ([`Bytes`]).

/// What [`Persist::put`] writes into.
#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// Zero bytes up to a multiple of `n` (from the start of the blob,
    /// which the file keeps aligned to 256).
    pub fn align(&mut self, n: usize) {
        let pad = self.buf.len().next_multiple_of(n) - self.buf.len();
        self.buf.resize(self.buf.len() + pad, 0);
    }

    /// A length, then the bytes, 16-aligned.
    pub fn bytes(&mut self, b: &[u8]) {
        self.u64(b.len() as u64);
        self.align(16);
        self.buf.extend_from_slice(b);
    }

    pub fn pod<T: bytemuck::Pod>(&mut self, v: &[T]) {
        self.bytes(bytemuck::cast_slice(v));
    }
}

/// What [`Persist::get`] reads from: a blob, and the mapping it lies in
/// (when there is one) for [`Bytes`] to point into.
pub struct Reader<'a> {
    pub(super) data: &'a [u8],
    pub(super) pos: usize,
    pub(super) map: Option<(&'a std::sync::Arc<memmap2::Mmap>, usize)>,
}

#[derive(Debug)]
pub struct Corrupt;

impl std::fmt::Display for Corrupt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cache data does not decode")
    }
}

impl std::error::Error for Corrupt {}

pub type Got<T> = std::result::Result<T, Corrupt>;

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0, map: None }
    }

    /// `data` is `map[base..]`'s start: [`Bytes`] read from it point into
    /// the mapping.
    pub fn mapped(map: &'a std::sync::Arc<memmap2::Mmap>, base: usize, len: usize) -> Reader<'a> {
        Reader { data: &map[base..base + len], pos: 0, map: Some((map, base)) }
    }

    pub fn done(&self) -> bool {
        self.pos == self.data.len()
    }

    #[inline]
    pub fn take(&mut self, n: usize) -> Got<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(Corrupt)?;
        let s = self.data.get(self.pos..end).ok_or(Corrupt)?;
        self.pos = end;
        Ok(s)
    }

    pub fn u64(&mut self) -> Got<u64> {
        // `take(8)` gives exactly 8 bytes: the conversion cannot fail.
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    /// A count of items at least `min` bytes each: checked against what is
    /// left, so a bad count fails instead of allocating.
    pub fn count(&mut self, min: usize) -> Got<usize> {
        let n = self.u64()? as usize;
        if n.saturating_mul(min.max(1)) > self.data.len() - self.pos { Err(Corrupt) } else { Ok(n) }
    }

    pub fn align(&mut self, n: usize) -> Got<()> {
        let to = self.pos.next_multiple_of(n);
        self.take(to - self.pos).map(|_| ())
    }

    fn bytes_range(&mut self) -> Got<std::ops::Range<usize>> {
        let n = self.count(1)?;
        self.align(16)?;
        let start = self.pos;
        self.take(n)?;
        Ok(start..start + n)
    }

    pub fn bytes(&mut self) -> Got<&'a [u8]> {
        let r = self.bytes_range()?;
        Ok(&self.data[r])
    }

    /// Bytes pointing into the mapping (copied when there is none).
    pub fn shared_bytes(&mut self) -> Got<Bytes> {
        let r = self.bytes_range()?;
        Ok(match self.map {
            Some((m, base)) => Bytes::mapped(m.clone(), base + r.start..base + r.end),
            None => Bytes::from(&self.data[r]),
        })
    }

    pub fn pod<T: bytemuck::Pod>(&mut self) -> Got<Vec<T>> {
        let b = self.bytes()?;
        if b.len() % std::mem::size_of::<T>().max(1) != 0 {
            return Err(Corrupt);
        }
        Ok(bytemuck::pod_collect_to_vec(b))
    }
}

/// A type the cache can store: written field by field, read back equal.
pub trait Persist: Sized {
    fn put(&self, w: &mut Writer);
    fn get(r: &mut Reader) -> Got<Self>;
}

macro_rules! persist_le {
    ($($t:ty),*) => {$(
        impl Persist for $t {
            fn put(&self, w: &mut Writer) {
                w.buf.extend_from_slice(&self.to_le_bytes());
            }
            fn get(r: &mut Reader) -> Got<Self> {
                // Exactly size_of bytes taken: the conversion cannot fail.
                Ok(<$t>::from_le_bytes(r.take(std::mem::size_of::<$t>())?.try_into().unwrap()))
            }
        }
    )*};
}
persist_le!(u8, u16, u32, u64, i8, i16, i32, i64);

impl Persist for f32 {
    fn put(&self, w: &mut Writer) {
        self.to_bits().put(w)
    }
    fn get(r: &mut Reader) -> Got<Self> {
        u32::get(r).map(f32::from_bits)
    }
}

impl Persist for f64 {
    fn put(&self, w: &mut Writer) {
        self.to_bits().put(w)
    }
    fn get(r: &mut Reader) -> Got<Self> {
        u64::get(r).map(f64::from_bits)
    }
}

impl Persist for usize {
    fn put(&self, w: &mut Writer) {
        (*self as u64).put(w)
    }
    fn get(r: &mut Reader) -> Got<Self> {
        usize::try_from(u64::get(r)?).map_err(|_| Corrupt)
    }
}

impl Persist for bool {
    fn put(&self, w: &mut Writer) {
        (*self as u8).put(w)
    }
    fn get(r: &mut Reader) -> Got<Self> {
        match u8::get(r)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Corrupt),
        }
    }
}

impl Persist for String {
    fn put(&self, w: &mut Writer) {
        w.u64(self.len() as u64);
        w.buf.extend_from_slice(self.as_bytes());
    }
    fn get(r: &mut Reader) -> Got<Self> {
        let n = r.count(1)?;
        String::from_utf8(r.take(n)?.to_vec()).map_err(|_| Corrupt)
    }
}

impl<T: Persist> Persist for Vec<T> {
    fn put(&self, w: &mut Writer) {
        w.u64(self.len() as u64);
        for x in self {
            x.put(w);
        }
    }
    fn get(r: &mut Reader) -> Got<Self> {
        let n = r.count(1)?;
        (0..n).map(|_| T::get(r)).collect()
    }
}

impl<T: Persist> Persist for Option<T> {
    fn put(&self, w: &mut Writer) {
        match self {
            None => 0u8.put(w),
            Some(x) => {
                1u8.put(w);
                x.put(w)
            }
        }
    }
    fn get(r: &mut Reader) -> Got<Self> {
        match u8::get(r)? {
            0 => Ok(None),
            1 => T::get(r).map(Some),
            _ => Err(Corrupt),
        }
    }
}

impl<T: Persist + Default, const N: usize> Persist for [T; N] {
    fn put(&self, w: &mut Writer) {
        for x in self {
            x.put(w);
        }
    }
    fn get(r: &mut Reader) -> Got<Self> {
        let mut bad = false;
        let a = std::array::from_fn(|_| match T::get(r) {
            Ok(v) => v,
            Err(_) => {
                bad = true;
                T::default()
            }
        });
        if bad { Err(Corrupt) } else { Ok(a) }
    }
}

impl<A: Persist, B: Persist> Persist for (A, B) {
    fn put(&self, w: &mut Writer) {
        self.0.put(w);
        self.1.put(w);
    }
    fn get(r: &mut Reader) -> Got<Self> {
        Ok((A::get(r)?, B::get(r)?))
    }
}

impl<A: Persist, B: Persist, C: Persist> Persist for (A, B, C) {
    fn put(&self, w: &mut Writer) {
        self.0.put(w);
        self.1.put(w);
        self.2.put(w);
    }
    fn get(r: &mut Reader) -> Got<Self> {
        Ok((A::get(r)?, B::get(r)?, C::get(r)?))
    }
}

/// Sorted by key: the same map always writes the same bytes.
impl<K: Persist + Ord + std::hash::Hash + Eq, V: Persist> Persist for std::collections::HashMap<K, V> {
    fn put(&self, w: &mut Writer) {
        let mut keys: Vec<&K> = self.keys().collect();
        keys.sort();
        w.u64(keys.len() as u64);
        for k in keys {
            k.put(w);
            self[k].put(w);
        }
    }
    fn get(r: &mut Reader) -> Got<Self> {
        let n = r.count(2)?;
        let mut m = std::collections::HashMap::with_capacity(n);
        for _ in 0..n {
            let k = K::get(r)?;
            let v = V::get(r)?;
            m.insert(k, v);
        }
        Ok(m)
    }
}

impl Persist for Bytes {
    fn put(&self, w: &mut Writer) {
        w.bytes(self)
    }
    fn get(r: &mut Reader) -> Got<Self> {
        r.shared_bytes()
    }
}

macro_rules! persist_glam {
    ($($t:ty => $a:ty, $to:ident, $from:path);*) => {$(
        impl Persist for $t {
            fn put(&self, w: &mut Writer) {
                self.$to().put(w)
            }
            fn get(r: &mut Reader) -> Got<Self> {
                <$a>::get(r).map(|a| $from(&a))
            }
        }
    )*};
}
persist_glam!(
    glam::Vec2 => [f32; 2], to_array, glam::Vec2::from_slice;
    glam::Vec3 => [f32; 3], to_array, glam::Vec3::from_slice;
    glam::Mat3 => [f32; 9], to_cols_array, glam::Mat3::from_cols_slice;
    glam::Mat4 => [f32; 16], to_cols_array, glam::Mat4::from_cols_slice
);

/// `persist!(Type { a, b, c: pod })`: [`Persist`] for a struct, field by
/// field (`pod`: a `Vec` of plain data, written as one block). Invoked
/// next to the type, so private fields are reachable.
#[macro_export]
macro_rules! persist {
    (@put $w:ident, $v:expr, pod) => { $w.pod(&$v) };
    (@put $w:ident, $v:expr, ) => { $crate::cache::Persist::put(&$v, $w) };
    (@get $r:ident, pod) => { $r.pod()? };
    (@get $r:ident, ) => { $crate::cache::Persist::get($r)? };
    ($t:ty { $($f:ident $(: $how:ident)?),* $(,)? }) => {
        impl $crate::cache::Persist for $t {
            fn put(&self, w: &mut $crate::cache::Writer) {
                $( $crate::persist!(@put w, self.$f, $($how)?); )*
            }
            fn get(r: &mut $crate::cache::Reader) -> $crate::cache::Got<Self> {
                Ok(Self { $( $f: $crate::persist!(@get r, $($how)?), )* })
            }
        }
    };
}

/// `persist_enum!(Type { A, B })`: [`Persist`] for a fieldless enum, by
/// the variant's position.
#[macro_export]
macro_rules! persist_enum {
    ($t:ty { $($v:ident),* $(,)? }) => {
        impl $crate::cache::Persist for $t {
            fn put(&self, w: &mut $crate::cache::Writer) {
                let all = [$(<$t>::$v),*];
                // `all` lists every variant, `self` among them.
                let i = all.iter().position(|v| v == self).unwrap() as u8;
                $crate::cache::Persist::put(&i, w)
            }
            fn get(r: &mut $crate::cache::Reader) -> $crate::cache::Got<Self> {
                let all = [$(<$t>::$v),*];
                let i: u8 = $crate::cache::Persist::get(r)?;
                all.get(i as usize).copied().ok_or($crate::cache::Corrupt)
            }
        }
    };
}
