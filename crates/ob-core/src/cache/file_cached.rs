//! Per-file caches.

use super::*;

// ---------------------------------------------------------------------------
// Per-file caches: what a parser makes of one game file (a car's models,
// a texture pack file), keyed on that file alone.

/// `parse`'s result for the game file `path`, from the cache when it has a
/// valid one (`what` names the parser), else parsed and left in the cache
/// (written in the background). `build`: the id of the crate the parser is
/// in ([`BuildId`]).
pub fn file_cached<T: Persist>(
    build: BuildId,
    path: &Path,
    what: &str,
    parse: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    if dir().is_none() {
        return parse();
    }
    let rel = path.to_string_lossy().into_owned();
    let kind = file_kind(path, what);
    let root = Path::new("");
    if let Some(file) = CacheFile::open(build, root, &kind, &[&rel])
        && let Some(mut r) = file.reader("data")
        && let Ok(v) = T::get(&mut r)
        && r.done()
    {
        return Ok(v);
    }
    let v = parse()?;
    let mut w = Writer::default();
    v.put(&mut w);
    write(build, root, &kind, &[&rel], vec![("data".into(), w.buf.into())]);
    Ok(v)
}

/// The kind of [`file_cached`]'s cache of the game file `path` made by the
/// parser `what`: `file-<what>-<file name>-<hash of the whole path>`.
pub fn file_kind(path: &Path, what: &str) -> String {
    let rel = path.to_string_lossy();
    format!(
        "file-{what}-{}",
        path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned())
            + "-"
            + &format!("{:016x}", xxhash_rust::xxh3::xxh3_64(rel.as_bytes()))
    )
}

/// Where the build `build` keeps its cache file of `kind` (whether there
/// is one or not); None with the cache off.
fn entry_path(build: BuildId, kind: &str) -> Option<std::path::PathBuf> {
    Some(dir()?.join(file_name_of(kind, build.0)))
}

/// The build `build`'s cache file of [`file_cached`]'s parser `what` over
/// `path`, when there is one (the file the next load of `path` reads
/// instead of `path`).
pub fn file_cached_entry(build: BuildId, path: &Path, what: &str) -> Option<std::path::PathBuf> {
    entry_path(build, &file_kind(path, what)).filter(|p| p.is_file())
}
