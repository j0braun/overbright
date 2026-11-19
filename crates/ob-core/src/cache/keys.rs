//! Cache keys: build id, source file stat, content and chunk hashes, file names.

use super::*;

/// The build id of the crate whose code makes a cache file
/// (build_support/build_id.rs): its sources, the lock file, its features
/// and the ids of the crates below it. Every cache call takes the owning
/// crate's id (`nfsu2::BUILD_ID`): it names the file and is checked in its
/// header, so the file goes stale with any change on its path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BuildId(pub u64);

impl BuildId {
    /// The id from its 16 hex digits (`env!("OB_BUILD_ID")`); 0 for any
    /// other text.
    pub const fn from_hex(hex: &str) -> BuildId {
        let b = hex.as_bytes();
        if b.len() != 16 {
            return BuildId(0);
        }
        let mut v = 0u64;
        let mut i = 0;
        while i < 16 {
            let d = match b[i] {
                c @ b'0'..=b'9' => c - b'0',
                c @ b'a'..=b'f' => c - b'a' + 10,
                c @ b'A'..=b'F' => c - b'A' + 10,
                _ => return BuildId(0),
            };
            v = v << 4 | d as u64;
            i += 1;
        }
        BuildId(v)
    }
}

/// A file's size, modification time (ns) and inode, or None if it cannot
/// be read (a cache over a missing file is never valid).
pub(super) fn stat(path: &Path) -> Option<(u64, u64, u64)> {
    let m = std::fs::metadata(path).ok()?;
    let mtime = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    #[cfg(unix)]
    let ino = std::os::unix::fs::MetadataExt::ino(&m);
    #[cfg(not(unix))]
    let ino = 0;
    Some((m.len(), mtime, ino))
}

pub(super) fn content_hash(path: &Path) -> Option<[u64; 2]> {
    let bytes = SourceBytes::open(path).ok()?;
    let h = xxhash_rust::xxh3::xxh3_128(&bytes);
    Some([h as u64, (h >> 64) as u64])
}

pub(super) fn chunk_hashes(data: &[u8]) -> Vec<u64> {
    use rayon::prelude::*;
    data.par_chunks(CHUNK).map(xxhash_rust::xxh3::xxh3_64).collect()
}

/// The cache file of `kind` written by this crate's build: `<kind>.<build id>.nfc`.
#[cfg(test)]
pub(super) fn file_name(kind: &str) -> String {
    file_name_of(kind, crate::BUILD_ID.0)
}

/// The cache file of `kind` written by the build `build`. The build id is
/// part of the name (and checked again in the header): two builds of
/// different sources never share a file, so neither finds the other's
/// stale and rewrites it.
pub(super) fn file_name_of(kind: &str, build: u64) -> String {
    let safe: String = kind
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();
    format!("{safe}.{build:016x}{NFC}")
}

/// A cache file's extension.
pub(super) const NFC: &str = ".nfc";

/// A cache file name's kind and build ([`file_name_of`]); None for a name
/// without a build id (the files of builds before the id was in the name)
/// or not a cache file.
pub fn split_name(name: &str) -> Option<(&str, u64)> {
    let (kind, build) = name.strip_suffix(NFC)?.rsplit_once('.')?;
    (build.len() == 16 && !kind.is_empty()).then(|| u64::from_str_radix(build, 16).ok().map(|b| (kind, b)))?
}
