//! `--verify-cache`.

use super::*;

/// `--verify-cache`: every track cache in the cache folder (or the tracks
/// named) against a fresh parse of the game's files. Per track a line; Err
/// when one differs.
pub fn verify(root: &Path, names: &[String]) -> anyhow::Result<Vec<String>> {
    let names: Vec<String> = if names.is_empty() {
        let dir = dir().ok_or_else(|| anyhow::anyhow!("the cache is off (NFSU2_NO_CACHE)"))?;
        let mut v: Vec<String> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        let n = e.file_name();
                        let (kind, build) = split_name(n.to_str()?)?;
                        (build == build_id()).then(|| kind.strip_prefix("track-").map(String::from))?
                    })
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    } else {
        names.to_vec()
    };
    let mut out = Vec::new();
    let mut bad = 0;
    for name in names {
        let sources = crate::world::World::sources(&name);
        let srcs: Vec<&str> = sources.iter().map(String::as_str).collect();
        let Some(file) = CacheFile::open(root, &TrackCache::kind(&name), &srcs) else {
            out.push(format!("{name}: no valid cache"));
            continue;
        };
        let cached = crate::world::World::from_sections(&file)
            .ok_or_else(|| anyhow::anyhow!("{name}: the cache does not decode"))?;
        let fresh = crate::world::World::parse(root, &name)?;
        let (a, b) = (fresh.to_sections(), cached.to_sections());
        let world_ok = a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x.0 == y.0 && x.1 == y.1);
        let gpu = crate::render::prepared::verify(&file, &fresh);
        let bytes: usize = a.iter().map(|s| s.1.len()).sum();
        let gpu_text = match gpu {
            None => "no renderer data".to_string(),
            Some(true) => "renderer data equal".to_string(),
            Some(false) => "renderer data DIFFERS".to_string(),
        };
        if !world_ok || gpu == Some(false) {
            bad += 1;
        }
        out.push(format!(
            "{name}: world {} ({bytes} bytes compared), {gpu_text}",
            if world_ok { "equal" } else { "DIFFERS" }
        ));
    }
    // The per-file caches: each against its parser's result now.
    if let Some(dir) = dir() {
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        e.file_name()
                            .to_str()
                            .filter(|n| n.starts_with("file-") && split_name(n).is_some_and(|(_, b)| b == build_id()))
                            .map(String::from)
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        let (mut equal, mut stale) = (0, 0);
        for f in files {
            let Some(header) = std::fs::read(dir.join(&f)).ok().and_then(|b| CacheFile::header(&b)) else {
                stale += 1;
                continue;
            };
            let Some(source) = header.sources.first() else { continue };
            let what = header.kind.strip_prefix("file-").and_then(|k| {
                ["solid-index", "solids", "textures"].into_iter().find(|w| k.starts_with(&format!("{w}-")))
            });
            let Some(what) = what else { continue };
            let Some(file) = CacheFile::open(Path::new(""), &header.kind, &[&source.rel]) else {
                stale += 1;
                continue;
            };
            match crate::car::reparse_for_cache(what, Path::new(&source.rel)) {
                Some(Ok(fresh)) if file.bytes("data").is_some_and(|d| d[..] == fresh[..]) => equal += 1,
                _ => {
                    bad += 1;
                    out.push(format!("{}: {what} DIFFERS", source.rel));
                }
            }
        }
        out.push(format!("per-file caches: {equal} equal to a fresh parse, {stale} stale or unreadable (not used)"));
    }
    anyhow::ensure!(bad == 0, "{}\n{bad} cache(s) differ from a fresh parse", out.join("\n"));
    Ok(out)
}
