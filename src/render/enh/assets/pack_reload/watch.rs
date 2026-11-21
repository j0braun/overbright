use super::*;

/// A file's signature: size, modification time, and (on Unix) the inode and
/// change time, so a file replaced through a rename, or rewritten within the
/// modification time's granularity, still looks changed.
pub type Sig = (u64, Option<SystemTime>, u64);

/// A file whose modification time is this close to the scan that took it in
/// can still change without its signature showing it (a file system whose
/// times are coarse, a second write within the same tick of its clock): its
/// content is compared on later scans until it is older than this.
pub const RACY: Duration = Duration::from_secs(3);

/// What a scan found for a file, once it settled.
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    /// Changed or added, and decodes.
    Changed(String),
    /// Removed.
    Removed(String),
    /// Changed but does not decode after [`RETRIES`] tries: left out (the
    /// texture in use stays).
    Rejected(String, String),
}

/// A file as last taken in.
struct Seen {
    sig: Sig,
    /// Its content's hash (None: not read yet).
    hash: Option<u64>,
    /// Its content is still compared at each scan ([`RACY`]).
    racy: bool,
}

/// A change waiting to settle.
struct Wait {
    /// The file's signature now (None: gone).
    sig: Option<Sig>,
    /// Scans it has stayed so.
    quiet: u32,
    /// Failed decodes, and when the first failed.
    failures: u32,
    failed_at: Option<SystemTime>,
    /// The content changed under an unchanged signature.
    forced: bool,
}

/// How the scanner reads a file: `check` decodes it as the loader would and
/// gives its content's hash; `hash` only the hash (None: unreadable).
pub trait Reader {
    fn check(&mut self, name: &str) -> Result<u64, String>;
    fn hash(&mut self, name: &str) -> Option<u64>;
}

/// One folder's files as last seen, and the changes waiting to settle.
#[derive(Default)]
pub struct Scanner {
    /// None until the first scan (which only takes the baseline).
    files: Option<HashMap<String, Seen>>,
    pending: HashMap<String, Wait>,
}

fn racy(sig: &Sig, now: SystemTime) -> bool {
    sig.1.is_some_and(|m| now.duration_since(m).map_or(true, |age| age < RACY))
}

impl Scanner {
    /// Compare `listing` (the folder now, taken at `now`) with the last.
    /// Returns the settled events. A changed file is decoded at once (a save
    /// through a rename is whole when it appears) and shows if it decodes;
    /// one that does not yet (being written) shows once it has stayed the
    /// same for one more scan and decodes, retried [`RETRIES`] scans.
    pub fn scan(&mut self, listing: HashMap<String, Sig>, now: SystemTime, read: &mut dyn Reader) -> Vec<Event> {
        let Some(files) = self.files.as_mut() else {
            self.files = Some(
                listing.into_iter().map(|(n, sig)| (n, Seen { racy: racy(&sig, now), sig, hash: None })).collect(),
            );
            return Vec::new();
        };
        let mut out = Vec::new();
        // New differences start (or restart) waiting; a recent file whose
        // content changed under the same signature too.
        let mut names: Vec<String> = files.keys().chain(listing.keys()).cloned().collect();
        names.sort();
        names.dedup();
        let mut fresh = HashSet::new();
        for name in names {
            let sig = listing.get(&name).copied();
            if let Some(w) = self.pending.get_mut(&name) {
                if w.sig != sig {
                    *w = Wait { sig, quiet: 0, failures: w.failures, failed_at: w.failed_at, forced: false };
                    fresh.insert(name);
                } else {
                    w.quiet += 1;
                }
                continue;
            }
            let seen = files.get_mut(&name);
            match (seen, sig) {
                (Some(seen), Some(sig)) if seen.sig == sig => {
                    if !seen.racy {
                        continue;
                    }
                    let h = read.hash(&name);
                    seen.racy = racy(&sig, now);
                    // Changed under the same signature, or taken in by the
                    // baseline while recent (written after the loader read
                    // it, before the watcher started): shown as a change.
                    let moved = match (seen.hash, h) {
                        (Some(a), Some(b)) => a != b,
                        (None, h) => h.is_some(),
                        (Some(_), None) => false,
                    };
                    if moved {
                        self.pending
                            .insert(name.clone(), Wait { sig: Some(sig), quiet: 0, failures: 0, failed_at: None, forced: true });
                        fresh.insert(name);
                    }
                }
                (None, None) => {}
                (_, sig) => {
                    self.pending.insert(name.clone(), Wait { sig, quiet: 0, failures: 0, failed_at: None, forced: false });
                    fresh.insert(name);
                }
            }
        }
        // Settled: whole at once, or the same for one whole scan.
        let mut done = Vec::new();
        for (name, w) in self.pending.iter_mut() {
            let first = fresh.contains(name);
            if !first && w.quiet == 0 {
                continue;
            }
            match w.sig {
                None => {
                    // Gone: only once it stays gone (a save that removes the
                    // file before writing it again shows no original between).
                    if first {
                        continue;
                    }
                    if files.remove(name).is_some() {
                        out.push(Event::Removed(name.clone()));
                    }
                    done.push(name.clone());
                }
                Some(sig) if !w.forced && files.get(name).is_some_and(|s| s.sig == sig) => done.push(name.clone()),
                Some(sig) => match read.check(name) {
                    Ok(h) => {
                        let same = files.get(name).is_some_and(|s| s.hash == Some(h));
                        files.insert(name.clone(), Seen { sig, hash: Some(h), racy: racy(&sig, now) });
                        // Written again with the same content: nothing to reload.
                        if !same {
                            out.push(Event::Changed(name.clone()));
                        }
                        done.push(name.clone());
                    }
                    // Not whole yet on first sight: wait for it to settle.
                    Err(_) if first => {}
                    Err(e) => {
                        w.failures += 1;
                        w.quiet = 0;
                        let since = *w.failed_at.get_or_insert(now);
                        // As many tries, and as long as that many ticks
                        // (the watcher looks more often right after a change).
                        if w.failures > RETRIES && now.duration_since(since).unwrap_or_default() >= TICK * RETRIES {
                            // Kept out until it changes again (its content
                            // read again while recent). No hash kept: one
                            // read now could already be a newer, whole save.
                            files.insert(name.clone(), Seen { sig, hash: None, racy: racy(&sig, now) });
                            out.push(Event::Rejected(name.clone(), e));
                            done.push(name.clone());
                        }
                    }
                },
            }
        }
        for name in done {
            self.pending.remove(&name);
        }
        out.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
        out
    }

    /// Whether a change is waiting to settle (the watcher looks again sooner).
    pub fn waiting(&self) -> bool {
        !self.pending.is_empty()
    }
}

/// The scanner's reads of a folder's files.
struct Files<'a>(&'a Path);

impl Reader for Files<'_> {
    fn check(&mut self, name: &str) -> Result<u64, String> {
        let path = self.0.join(name);
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        if Key::of_file(name).is_some() {
            texture_pack::validate(&path)?;
        }
        Ok(xxhash_rust::xxh3::xxh3_64(&bytes))
    }
    fn hash(&mut self, name: &str) -> Option<u64> {
        std::fs::read(self.0.join(name)).ok().map(|b| xxhash_rust::xxh3::xxh3_64(&b))
    }
}

/// The pack files of `root` (None: it does not exist).
pub fn listing(root: &Path) -> Option<HashMap<String, Sig>> {
    let dir = std::fs::read_dir(root).ok()?;
    let mut out = HashMap::new();
    for entry in dir.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if Key::of_file(name).is_none() && !META.contains(&name) {
            continue;
        }
        let Ok(m) = entry.metadata().or_else(|_| std::fs::metadata(entry.path())) else { continue };
        if m.is_file() {
            #[cfg(unix)]
            let id = {
                use std::os::unix::fs::MetadataExt;
                m.ino() ^ (m.ctime() as u64).wrapping_mul(1_000_000_007) ^ (m.ctime_nsec() as u64).rotate_left(32)
            };
            #[cfg(not(unix))]
            let id = 0;
            out.insert(name.to_string(), (m.len(), m.modified().ok(), id));
        }
    }
    Some(out)
}

/// The HD UI pack's own files: a change re-checks the pack as a whole.
const META: [&str; 4] = ["hd-ui-pack.list", "map-overrides.tsv", "manifest.json", "map-manifest.json"];

// ------------------------------------------------------------- watcher

/// A watched folder: its domains and scanner.
struct Watched {
    domains: u8,
    exists: bool,
    scanner: Scanner,
}

#[derive(Default)]
pub(super) struct Watcher {
    folders: HashMap<PathBuf, Watched>,
    /// The folder each domain used at the last tick.
    roots: [Vec<PathBuf>; 3],
    /// The HD UI pack's completeness at the last tick.
    hd_installed: Option<bool>,
    warned: HashSet<PathBuf>,
    /// Until when the watcher looks every [`FAST_TICK`] (a change just seen
    /// or waiting, a folder just come: its files follow).
    fast_until: Option<std::time::Instant>,
}

impl Watcher {
    /// The watched folders that do not exist (yet): their coming is looked
    /// for between ticks, so a save into a new folder shows at once.
    fn missing(&self) -> Vec<PathBuf> {
        self.folders.iter().filter(|(_, w)| !w.exists).map(|(p, _)| p.clone()).collect()
    }
}

/// The poll interval right after a change ([`FAST_FOR`] long).
pub const FAST_TICK: Duration = Duration::from_millis(50);
pub const FAST_FOR: Duration = Duration::from_secs(2);

pub(super) static RUNNING: AtomicBool = AtomicBool::new(false);
pub(super) static STOP: AtomicBool = AtomicBool::new(false);
static FULL: AtomicBool = AtomicBool::new(false);
pub(super) static MAP_DIRTY: AtomicBool = AtomicBool::new(false);
pub(super) static HD_REQUESTED: AtomicBool = AtomicBool::new(false);

impl Watcher {
    /// The folders to watch now, by domain: the HD UI pack's (its folder
    /// even while incomplete, so its completion is seen) and the world and
    /// car packs'.
    fn roots(&self) -> [Vec<PathBuf>; 3] {
        let ui = if HD_REQUESTED.load(Ordering::Relaxed) {
            crate::hd_ui_pack::candidate_root().map(Path::to_path_buf)
        } else {
            None
        };
        [ui.into_iter().collect(), texture_pack::roots_of(Domain::World), texture_pack::roots_of(Domain::Car)]
    }

    /// One poll; the time to the next.
    pub(super) fn tick(&mut self) -> Duration {
        let soon = self.poll();
        let now = std::time::Instant::now();
        if soon {
            self.fast_until = Some(now + FAST_FOR);
        }
        if self.fast_until.is_some_and(|u| now < u) { FAST_TICK } else { TICK }
    }

    /// Whether something changed or waits to settle.
    fn poll(&mut self) -> bool {
        let mut soon = false;
        let full = FULL.swap(false, Ordering::Relaxed);
        if full {
            crate::hd_ui_pack::recheck();
            for w in self.folders.values_mut() {
                w.scanner = Scanner::default();
            }
            log::info!("texture packs: full reload");
            publish(ALL, None);
            MAP_DIRTY.store(true, Ordering::Relaxed);
        }
        // A generated pack being written: wait for the generator to finish.
        if matches!(
            crate::upscale_pack::job_status(),
            crate::upscale_pack::JobStatus::Running(_) | crate::upscale_pack::JobStatus::Cancelling
        ) {
            return false;
        }
        let roots = self.roots();
        // A domain whose folder changed (a row, a pack generated) reloads.
        let mut changed = 0u8;
        for (i, bit) in [UI, WORLD, CAR].into_iter().enumerate() {
            if roots[i] != self.roots[i] {
                changed |= bit;
            }
        }
        let first = self.hd_installed.is_none();
        self.roots = roots.clone();
        let mut wanted: HashMap<PathBuf, u8> = HashMap::new();
        for (i, bit) in [UI, WORLD, CAR].into_iter().enumerate() {
            for r in &roots[i] {
                *wanted.entry(r.clone()).or_default() |= bit;
            }
        }
        self.folders.retain(|p, _| wanted.contains_key(p));
        for (path, domains) in wanted {
            let w = self.folders.entry(path).or_insert_with(|| Watched {
                domains,
                exists: true,
                scanner: Scanner::default(),
            });
            w.domains = domains;
        }
        let mut hd_meta = false;
        for (path, w) in self.folders.iter_mut() {
            #[cfg(test)]
            slow_step();
            let Some(list) = listing(path) else {
                if w.exists {
                    w.exists = false;
                    w.scanner = Scanner::default();
                    changed |= w.domains;
                    hd_meta |= w.domains & UI != 0;
                }
                continue;
            };
            if !w.exists {
                w.exists = true;
                soon = true;
                changed |= w.domains;
                hd_meta |= w.domains & UI != 0;
            }
            #[cfg(test)]
            slow_step();
            let events = w.scanner.scan(list, SystemTime::now(), &mut Files(path));
            soon |= w.scanner.waiting();
            soon |= !events.is_empty();
            for e in events {
                match e {
                    Event::Changed(name) | Event::Removed(name) if META.contains(&name.as_str()) => {
                        hd_meta |= w.domains & UI != 0;
                    }
                    Event::Changed(name) | Event::Removed(name) => {
                        if let Some(key) = Key::of_file(&name) {
                            log::info!("texture packs: {} changed, reloading", path.join(&name).display());
                            publish(w.domains, Some(key));
                        }
                    }
                    Event::Rejected(name, error) => {
                        let file = path.join(&name);
                        if self.warned.insert(file.clone()) {
                            log::warn!(
                                "texture packs: keeping the texture in use, {} does not load: {error}",
                                file.display()
                            );
                        }
                    }
                }
            }
        }
        // The HD UI pack appeared, went or its index changed: check it
        // again; a change of completeness reloads the UI and the maps.
        if hd_meta || first {
            let was = self.hd_installed;
            if !first {
                crate::hd_ui_pack::recheck();
            }
            let now = crate::hd_ui_pack::installed();
            self.hd_installed = Some(now);
            if was.is_some_and(|w| w != now) || (hd_meta && !first) {
                changed |= UI;
                MAP_DIRTY.store(true, Ordering::Relaxed);
            }
        }
        if changed != 0 && !first {
            log::info!("texture packs: a pack folder changed, reloading");
            publish(changed, None);
            soon = true;
        }
        soon
    }
}

pub(super) fn spawn() {
    STOP.store(false, Ordering::Relaxed);
    RUNNING.store(true, Ordering::Relaxed);
    let started = std::thread::Builder::new().name("pack watcher".into()).spawn(|| {
        let mut w = Watcher::default();
        while !STOP.load(Ordering::Relaxed) {
            let next = w.tick();
            // Until the next tick, a missing folder coming wakes it sooner
            // (one stat each, every FAST_TICK).
            let missing = w.missing();
            let until = std::time::Instant::now() + next;
            loop {
                let left = until.saturating_duration_since(std::time::Instant::now());
                if left.is_zero() || STOP.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(left.min(FAST_TICK));
                if missing.iter().any(|p| p.is_dir()) {
                    break;
                }
            }
        }
        RUNNING.store(false, Ordering::Relaxed);
    });
    if let Err(e) = started {
        log::warn!("texture packs: no watcher thread: {e}");
        RUNNING.store(false, Ordering::Relaxed);
    }
}

/// Ctrl+R: every pack's every texture again (and the HD UI pack checked
/// again), at the watcher's next tick.
pub fn request_full() {
    FULL.store(true, Ordering::Relaxed);
}
