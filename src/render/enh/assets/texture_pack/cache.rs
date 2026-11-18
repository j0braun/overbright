use super::*;

/// Domain, hash, name, size limit, the pack folders and the cap in force.
type CacheKey = (Domain, u32, String, u32, Vec<PathBuf>, u32);
#[derive(Default)]
struct Cache {
    entries: HashMap<CacheKey, Option<Arc<Replacement>>>,
    bytes: usize,
}

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

/// docs/pack-hot-reload.md: forget the cached replacements a changed
/// file may stand for (an eight-digit hash name or a texture name, in any
/// domain and folder); `None` forgets them all.
pub(in crate::render) fn invalidate(hash: Option<u32>, name: Option<&str>) {
    let Some(cache) = CACHE.get() else { return };
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if hash.is_none() && name.is_none() {
        cache.entries.clear();
        cache.bytes = 0;
        return;
    }
    let mut freed = 0;
    cache.entries.retain(|k, v| {
        let hit = hash == Some(k.1) || name.is_some_and(|n| n == k.2);
        if hit {
            freed += v.as_ref().map_or(0, |r| r.bytes.len());
        }
        !hit
    });
    cache.bytes = cache.bytes.saturating_sub(freed);
}

/// The texture editor's stand-ins (docs/texture-editor.md: the highlight),
/// ahead of every pack; none while the editor is off.
static OVERRIDES: Mutex<Option<HashMap<(Domain, u32), Arc<Replacement>>>> = Mutex::new(None);
static ANY_OVERRIDE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Put (Some) or take away (None) the editor's stand-in for texture `hash`.
pub(in crate::render) fn set_override(domain: Domain, hash: u32, r: Option<Replacement>) {
    let mut o = OVERRIDES.lock().unwrap_or_else(|e| e.into_inner());
    let map = o.get_or_insert_with(HashMap::new);
    match r {
        Some(r) => {
            map.insert((domain, hash), Arc::new(r));
        }
        None => {
            map.remove(&(domain, hash));
        }
    }
    ANY_OVERRIDE.store(!map.is_empty(), std::sync::atomic::Ordering::Relaxed);
}

pub(in crate::render) fn replacement(
    domain: Domain,
    t: &crate::formats::texture::Texture,
    max_dimension: u32,
) -> Option<Arc<Replacement>> {
    if ANY_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed)
        && let Some(r) = OVERRIDES.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&(domain, t.hash)))
    {
        return Some(r.clone());
    }
    let mut roots = roots(domain);
    if roots.is_empty() {
        return None;
    }
    roots.extend(generated_ui(domain));
    // The VRAM cap (G7b2): generated textures of the bulk categories held
    // to 2x the original, by dropping their top mip levels.
    let cap = if domain == Domain::Ui { None } else { crate::upscale_pack::cap() };
    let mut cache = CACHE.get_or_init(|| Mutex::new(Cache::default())).lock().unwrap_or_else(|e| e.into_inner());
    let key = (domain, t.hash, t.name.clone(), max_dimension, roots.clone(), cap.unwrap_or(0));
    if let Some(r) = cache.entries.get(&key) {
        return r.clone();
    }
    // Serialize decoding to bound temporary memory even for parallel world arrays.
    let result = load_first(&roots, t.hash, &t.name, max_dimension)
        .map(|(root, r)| match cap {
            Some(_) => {
                let n = crate::upscale_pack::capped_levels(
                    crate::upscale_pack::manifest_categories(root).get(&t.hash).copied(),
                    r.width,
                    t.width,
                    cap,
                );
                drop_levels(r, n)
            }
            None => r,
        })
        .map(Arc::new);
    let bytes = result.as_ref().map_or(0, |r| r.bytes.len());
    if cache.bytes + bytes > 128 * 1024 * 1024 || cache.entries.len() >= 16384 {
        cache.entries.clear();
        cache.bytes = 0;
    }
    cache.bytes += bytes;
    cache.entries.insert(key, result.clone());
    result
}

/// The first folder (in order) that has a replacement, and it.
/// docs/texture-pack-cache.md: what the replacements for the
/// textures `slots` (hash, name) depend on, as text, for the pack-applied
/// arrays' cache: the loader's version, the size limit, the cap, every
/// folder in order, and per folder every file the loader would try for a
/// slot that exists (each folder listed once, not a stat per candidate),
/// with its size and modification time, and its manifest (the cap's
/// categories). A file added, removed, renamed or rewritten, another folder
/// order or tier, gives other text. The tone mode (`import_tpf --tone`) is
/// in the files' bytes, so their times. None without a pack.
pub(in crate::render) fn fingerprint(domain: Domain, slots: &[(u32, &str)], max_dimension: u32) -> Option<String> {
    use std::fmt::Write;
    let roots = roots(domain);
    if roots.is_empty() {
        return None;
    }
    let cap = if domain == Domain::Ui { None } else { crate::upscale_pack::cap() };
    let mut out = format!("tp1b-pack-cache 1\nmax {max_dimension}\ncap {}\n", cap.unwrap_or(0));
    let stamp = |p: &Path| -> Option<(u64, u64)> {
        let m = std::fs::metadata(p).ok()?;
        let t = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        Some((m.len(), t))
    };
    let mut wanted: Vec<String> = slots
        .iter()
        .flat_map(|&(hash, name)| {
            let mut v = vec![format!("{hash:08X}.png"), format!("{hash:08X}.dds")];
            if !name.is_empty() && !name.contains(['/', '\\']) && name != "." && name != ".." {
                v.push(format!("{name}.png"));
            }
            v
        })
        .collect();
    wanted.sort();
    wanted.dedup();
    for root in &roots {
        let _ = writeln!(out, "root {}", root.display());
        // File names as the file system matches them (case-blind on Windows).
        let fold = |n: &str| if cfg!(windows) { n.to_ascii_lowercase() } else { n.to_string() };
        let names: HashMap<String, String> = std::fs::read_dir(root)
            .ok()?
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .map(|n| (fold(&n), n))
            .collect();
        for n in
            wanted.iter().map(String::as_str).chain([crate::upscale_pack::MANIFEST]).filter_map(|n| names.get(&fold(n)))
        {
            let (len, t) = stamp(&root.join(n))?;
            let _ = writeln!(out, "{n} {len} {t}");
        }
    }
    Some(out)
}

pub(super) fn load_first<'a>(
    roots: &'a [PathBuf],
    hash: u32,
    name: &str,
    max_dimension: u32,
) -> Option<(&'a PathBuf, Replacement)> {
    roots.iter().find_map(|root| load(root, hash, name, max_dimension).map(|r| (root, r)))
}

/// `r` without its first `n` mip levels (block-compressed only: a hand-made
/// PNG is kept as it is).
pub(super) fn drop_levels(r: Replacement, n: u32) -> Replacement {
    let Some(format) = r.format.filter(|_| n > 0 && n < r.mips) else { return r };
    let skip: usize = (0..n).map(|m| format.level_size((r.width >> m).max(1), (r.height >> m).max(1))).sum();
    Replacement {
        width: (r.width >> n).max(1),
        height: (r.height >> n).max(1),
        mips: r.mips - n,
        format: Some(format),
        bytes: r.bytes[skip..].to_vec(),
    }
}
