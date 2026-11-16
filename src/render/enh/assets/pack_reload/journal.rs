use super::*;

/// Log entries kept; a renderer further behind reloads everything.
pub(super) const LOG_CAP: usize = 4096;

/// A changed file's texture: its eight-digit hash name, or the texture
/// name it is a fallback for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Hash(u32),
    Name(String),
}

impl Key {
    /// The texture a pack file name stands for (`ABCDEF12.png`,
    /// `ABCDEF12.dds`, `ROAD_MAIN.png`), as the loader looks them up.
    pub fn of_file(file: &str) -> Option<Key> {
        let (stem, ext) = file.rsplit_once('.')?;
        if stem.is_empty() || (ext != "png" && ext != "dds") {
            return None;
        }
        if stem.len() == 8 && stem.bytes().all(|c| c.is_ascii_hexdigit()) {
            // The loader asks for `{hash:08X}`: lower-case names never match.
            return (stem.bytes().all(|c| !c.is_ascii_lowercase()))
                .then(|| Key::Hash(u32::from_str_radix(stem, 16).unwrap_or(0)));
        }
        (ext == "png").then(|| Key::Name(stem.to_string()))
    }

    /// Whether this is texture `hash` named `name`.
    pub fn matches(&self, hash: u32, name: &str) -> bool {
        match self {
            Key::Hash(h) => *h == hash,
            Key::Name(n) => n == name,
        }
    }
}

/// Domain bits of a log entry.
pub(super) const UI: u8 = 1;
pub(super) const WORLD: u8 = 2;
pub(super) const CAR: u8 = 4;
pub(super) const ALL: u8 = UI | WORLD | CAR;

fn bit(domain: Domain) -> u8 {
    match domain {
        Domain::Ui => UI,
        Domain::World => WORLD,
        Domain::Car => CAR,
    }
}

/// What a renderer has to redo.
#[derive(Debug, PartialEq, Eq)]
pub enum Pending {
    None,
    Some(HashSet<Key>),
    All,
}

impl Pending {
    pub fn any(&self) -> bool {
        !matches!(self, Pending::None)
    }
    pub fn hits(&self, hash: u32, name: &str) -> bool {
        match self {
            Pending::None => false,
            Pending::All => true,
            Pending::Some(keys) => keys.iter().any(|k| k.matches(hash, name)),
        }
    }
}

#[derive(Default)]
struct Log {
    /// (generation, domains, key: None for everything), oldest first.
    entries: VecDeque<(u64, u8, Option<Key>)>,
}

static GENERATION: AtomicU64 = AtomicU64::new(0);
static LOG: Mutex<Log> = Mutex::new(Log { entries: VecDeque::new() });

/// The newest change's generation (0: none yet). A renderer made now
/// starts from it.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// Publish a change (`key` None: the domains' every texture): the loader
/// forgets what it cached for it, the renderers see it from the next frame.
pub(crate) fn publish(domains: u8, key: Option<Key>) {
    match &key {
        None => texture_pack::invalidate(None, None),
        Some(Key::Hash(h)) => texture_pack::invalidate(Some(*h), None),
        Some(Key::Name(n)) => texture_pack::invalidate(None, Some(n)),
    }
    let mut log = LOG.lock().unwrap_or_else(|e| e.into_inner());
    let g = GENERATION.load(Ordering::Relaxed) + 1;
    log.entries.push_back((g, domains, key));
    while log.entries.len() > LOG_CAP {
        log.entries.pop_front();
    }
    GENERATION.store(g, Ordering::Release);
}

/// The changes for `domain` after generation `seen`, and the generation
/// they reach (to store once they are done).
pub(in crate::render) fn since(seen: u64, domain: Domain) -> (u64, Pending) {
    let now = generation();
    if now == seen {
        return (now, Pending::None);
    }
    let log = LOG.lock().unwrap_or_else(|e| e.into_inner());
    let now = log.entries.back().map_or(0, |e| e.0);
    // Fell out of the log: everything.
    if log.entries.front().is_some_and(|e| e.0 > seen + 1) {
        return (now, Pending::All);
    }
    let mut keys = HashSet::new();
    for (_, domains, key) in log.entries.iter().filter(|e| e.0 > seen) {
        if domains & bit(domain) == 0 {
            continue;
        }
        match key {
            None => return (now, Pending::All),
            Some(k) => {
                keys.insert(k.clone());
            }
        }
    }
    (now, if keys.is_empty() { Pending::None } else { Pending::Some(keys) })
}

// ------------------------------------------------------------- scanner
