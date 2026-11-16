use super::*;

/// A file's signature: size and modification time.
pub type Sig = (u64, Option<SystemTime>);

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

/// One folder's files as last seen, and the changes waiting to settle.
#[derive(Default)]
pub struct Scanner {
    /// None until the first scan (which only takes the baseline).
    files: Option<HashMap<String, Sig>>,
    /// Name → (its signature now (None: gone), scans it has stayed so, failed decodes).
    pending: HashMap<String, (Option<Sig>, u32, u32)>,
}

impl Scanner {
    /// Compare `listing` (the folder now) with the last; `valid` decodes a
    /// settled changed file. Returns the settled events.
    pub fn scan(
        &mut self,
        listing: HashMap<String, Sig>,
        mut valid: impl FnMut(&str) -> Result<(), String>,
    ) -> Vec<Event> {
        let Some(files) = self.files.as_mut() else {
            self.files = Some(listing);
            return Vec::new();
        };
        let mut out = Vec::new();
        // New differences start (or restart) waiting.
        let names: HashSet<&String> = files.keys().chain(listing.keys()).collect();
        for name in names {
            let now = listing.get(name).copied();
            if now == files.get(name).copied() && !self.pending.contains_key(name) {
                continue;
            }
            match self.pending.get_mut(name) {
                None => {
                    self.pending.insert(name.clone(), (now, 0, 0));
                }
                Some(entry) if entry.0 != now => *entry = (now, 0, entry.2),
                Some(entry) => entry.1 += 1,
            }
        }
        // Settled: the same for one whole tick.
        let mut done = Vec::new();
        for (name, (now, quiet, failures)) in self.pending.iter_mut() {
            if *quiet == 0 {
                continue;
            }
            match now {
                None => {
                    if files.remove(name).is_some() {
                        out.push(Event::Removed(name.clone()));
                    }
                    done.push(name.clone());
                }
                Some(sig) if files.get(name) == Some(sig) => done.push(name.clone()),
                Some(sig) => match valid(name) {
                    Ok(()) => {
                        files.insert(name.clone(), *sig);
                        out.push(Event::Changed(name.clone()));
                        done.push(name.clone());
                    }
                    Err(e) => {
                        *failures += 1;
                        *quiet = 0;
                        if *failures > RETRIES {
                            // Kept out until it changes again.
                            files.insert(name.clone(), *sig);
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
            out.insert(name.to_string(), (m.len(), m.modified().ok()));
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
}

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

    pub(super) fn tick(&mut self) {
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
            return;
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
                changed |= w.domains;
                hd_meta |= w.domains & UI != 0;
            }
            let events = w.scanner.scan(list, |name| texture_pack::validate(&path.join(name)));
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
        }
    }
}

pub(super) fn spawn() {
    STOP.store(false, Ordering::Relaxed);
    RUNNING.store(true, Ordering::Relaxed);
    let started = std::thread::Builder::new().name("pack watcher".into()).spawn(|| {
        let mut w = Watcher::default();
        while !STOP.load(Ordering::Relaxed) {
            w.tick();
            std::thread::sleep(TICK);
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
