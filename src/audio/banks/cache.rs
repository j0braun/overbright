use super::abk::Bank;
use crate::audio::collision::{Stitch, parse_stitches};
use crate::audio::aems::Events;
use crate::audio::fe::Sample;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// The banks read and parsed so far, one per (root,
/// path) while anything holds it: every player, the traffic, the world's
/// sounds and the stitch banks share one copy of a bank instead of each
/// reading and parsing the file (MB_Stich_Collision.abk and InGameCommon.bun
/// were read five times at the audio's start). Weak entries: a bank nobody
/// holds any more is read again when asked (a car's engine banks do not
/// pile up over a session). The bank is the file's parse, unchanged.
type BankKey = (std::path::PathBuf, String);
static BANKS: std::sync::Mutex<Option<HashMap<BankKey, std::sync::Weak<Bank>>>> = std::sync::Mutex::new(None);

/// The bank at `path` under `root`, shared ([`BANKS`]); `None` when it
/// cannot be read or parsed (asked again next time, as before).
pub fn shared_bank(root: &Path, path: &str) -> Option<Arc<Bank>> {
    let key = (root.to_path_buf(), path.to_string());
    if let Some(b) = BANKS.lock().unwrap().as_ref().and_then(|m| m.get(&key)).and_then(std::sync::Weak::upgrade) {
        return Some(b);
    }
    // Read and parsed outside the lock (another thread's bank is not held
    // up by this one's file).
    let t = std::time::Instant::now();
    let bank = Arc::new(Bank::parse(crate::game_fs::read(root, path).ok()?)?);
    // NFSU2_BANK_LOG (measuring aid): each bank read, its time.
    if crate::env_set!("NFSU2_BANK_LOG") {
        eprintln!("bank read: {path} {:.2} ms", t.elapsed().as_secs_f64() * 1000.0);
    }
    let mut all = BANKS.lock().unwrap();
    let all = all.get_or_insert_with(HashMap::new);
    all.retain(|_, w| w.strong_count() > 0);
    // Two threads that read it at once: the first one kept is everyone's.
    if let Some(b) = all.get(&key).and_then(std::sync::Weak::upgrade) {
        return Some(b);
    }
    all.insert(key, Arc::downgrade(&bank));
    Some(bank)
}

/// The banks a drive's sounds will ask for first, read and parsed now
/// (on a load worker, behind the loading screen): the returned handles keep
/// them shared until the drive's sounds hold them themselves. The banks
/// are the same as a first use would read.
pub fn prewarm_banks(root: &Path, paths: &[&str]) -> Vec<Arc<Bank>> {
    paths.iter().filter_map(|p| shared_bank(root, p)).collect()
}

/// The last [`prewarm_drive_banks`]' banks, held until the next.
static PINNED: std::sync::Mutex<Vec<Arc<Bank>>> = std::sync::Mutex::new(Vec::new());

/// The in-game banks every drive's sounds open on its first ticks
/// ([`DRIVE_BANKS`]), read on the calling thread (a load worker) and held
/// until the next call, so the drive's first tick finds them parsed. One
/// drive's set at most is held beyond what the sounds hold themselves.
pub fn prewarm_drive_banks(root: &Path) {
    let banks = prewarm_banks(root, DRIVE_BANKS);
    *PINNED.lock().unwrap() = banks;
}

/// The banks of [`prewarm_drive_banks`]: the world's, the traffic's, the
/// scrapes' and the road's (the player car's own banks are its load's).
pub const DRIVE_BANKS: &[&str] = &[
    "SOUND/IG_GLOBAL/ENV_COMMON_MB.abk",
    "SOUND/IG_GLOBAL/FX_MAIN_MEM_MB.abk",
    "SOUND/IG_GLOBAL/FX_Misc_MB.abk",
    "SOUND/IG_GLOBAL/TRAFFIC_MB.abk",
    "SOUND/IG_GLOBAL/ROADNOISE_00_MB.abk",
    "SOUND/IG_GLOBAL/WIND_00_MB.abk",
    "SOUND/IG_GLOBAL/FX_Rain_MB.abk",
];

/// The car-effect banks' samples, decoded on first use.
pub struct CarFxSounds {
    banks: HashMap<String, Option<Arc<Bank>>>,
    cache: HashMap<(String, usize), Option<Sample>>,
    root: std::path::PathBuf,
}

/// The voices an event's sound players start: (list index, loops).
pub type Players = Vec<(usize, bool)>;

impl CarFxSounds {
    pub fn new(root: &Path) -> CarFxSounds {
        CarFxSounds { banks: HashMap::new(), cache: HashMap::new(), root: root.to_path_buf() }
    }

    fn bank(&mut self, path: &str) -> Option<&Bank> {
        let root = self.root.clone();
        self.banks.entry(path.to_string()).or_insert_with(|| shared_bank(&root, path)).as_deref()
    }

    /// A bank event's players: each sound player's template index (state
    /// +0x14, clamped into the list as node 27 does at 0x720DC9) and
    /// whether that sample carries a loop (tags 0x86 / 0x87). Repeats are
    /// dropped (**inferred**: the graph's use of a second player on the
    /// same sound is not traced).
    pub fn players(&mut self, path: &str, event: &str) -> Players {
        let Some(b) = self.bank(path) else { return Vec::new() };
        let Some(m) = b
            .events
            .iter()
            .find(|e| e.0.ends_with(event) || event.ends_with(e.0.as_str()))
            .map(|e| b.modules[e.1].clone())
        else {
            return Vec::new();
        };
        let list = b.sound_list(&m);
        let mut out: Players = Vec::new();
        for &p in &m.players {
            let at = m.code.1 + p;
            let idx = crate::formats::bundle::read_u32(&b.data, at + 0x14) as i32;
            let idx = idx.clamp(0, list.len().saturating_sub(1) as i32) as usize;
            let looped = list
                .get(idx)
                .and_then(|e| b.samples.get(e.1 as usize))
                .and_then(|s| s.as_ref())
                .is_some_and(|s| s.loop_length.is_some());
            if !out.iter().any(|o| o.0 == idx) {
                out.push((idx, looped));
            }
        }
        out
    }

    /// A bank's sample by its BNKl slot (what a sound list entry's +4
    /// names), decoded once.
    pub fn slot_sample(&mut self, path: &str, slot: usize) -> Option<(Sample, bool)> {
        let key = (format!("{path}#slot"), slot);
        let looped = self.bank(path)?.samples.get(slot)?.as_ref()?.loop_length.is_some();
        if let Some(s) = self.cache.get(&key) {
            return s.clone().map(|s| (s, looped));
        }
        let s = self.bank(path).and_then(|b| b.decode(slot).map(|(info, pcm)| Arc::new((info.rate, pcm))));
        self.cache.insert(key, s.clone());
        s.map(|s| (s, looped))
    }

    /// A bank's event graphs (`aems::Events`), made on first use.
    pub fn events(&mut self, path: &str) -> Option<Events> {
        self.bank(path).map(Events::new)
    }

    /// The sample at a list index of an event's first player list.
    pub fn sample(&mut self, path: &str, index: usize) -> Option<Sample> {
        let key = (path.to_string(), index);
        if let Some(s) = self.cache.get(&key) {
            return s.clone();
        }
        let s = self.bank(path).and_then(|b| {
            let m = b.modules.first()?;
            let slot = b.sound_list(m).get(index)?.1;
            b.decode(slot as usize).map(|(info, pcm)| Arc::new((info.rate, pcm)))
        });
        self.cache.insert(key, s.clone());
        s
    }
}

/// Both stitch banks and lists.
pub struct StitchBanks {
    banks: [Option<Arc<Bank>>; 2],
    lists: [Vec<u32>; 2],
    pub stitches: Arc<[Vec<Stitch>; 2]>,
    cache: HashMap<(usize, usize), Option<Sample>>,
}

/// InGameCommon.bun's stitch lists by root (parsed once a run, the
/// file read once; the stitch banks and the collision sounds share them).
static STITCHES: std::sync::Mutex<Vec<(std::path::PathBuf, Arc<[Vec<Stitch>; 2]>)>> = std::sync::Mutex::new(Vec::new());

/// [`parse_stitches`] of `root`'s InGameCommon.bun, shared; `None` when
/// the file cannot be read (asked again next time).
pub fn shared_stitches(root: &Path) -> Option<Arc<[Vec<Stitch>; 2]>> {
    let mut all = STITCHES.lock().unwrap();
    if let Some((_, s)) = all.iter().find(|(r, _)| r == root) {
        return Some(s.clone());
    }
    let s = Arc::new(parse_stitches(&crate::game_fs::read(root, "GLOBAL/InGameCommon.bun").ok()?));
    all.push((root.to_path_buf(), s.clone()));
    Some(s)
}

impl StitchBanks {
    pub fn load(root: &Path) -> Option<StitchBanks> {
        let bank = |n: &str| shared_bank(root, &format!("SOUND/IG_GLOBAL/{n}"));
        let banks = [bank("MB_Stich_Collision.abk"), bank("MB_Stitch_Whoosh.abk")];
        let list = |b: &Option<Arc<Bank>>| b.as_ref().and_then(|b| b.modules.first().map(|m| b.sound_list(m).into_iter().map(|e| e.1).collect())).unwrap_or_default();
        let lists = [list(&banks[0]), list(&banks[1])];
        let stitches = shared_stitches(root)?;
        Some(StitchBanks { banks, lists, stitches, cache: HashMap::new() })
    }

    /// A layer's sample: bank kind 0 / 1, the sound-list entry.
    pub fn sample(&mut self, kind: usize, index: usize) -> Option<Sample> {
        if let Some(s) = self.cache.get(&(kind, index)) {
            return s.clone();
        }
        let s = (|| {
            let slot = *self.lists.get(kind)?.get(index)?;
            let (info, pcm) = self.banks.get(kind)?.as_ref()?.decode(slot as usize)?;
            Some(Arc::new((info.rate, pcm)))
        })();
        self.cache.insert((kind, index), s.clone());
        s
    }
}

#[cfg(test)]
mod read_once_tests {
    use super::*;

    /// The audio's start makes four stitch banks
    /// (the FE player's, the world's, the traffic's, the drive-bys') and the
    /// collision sounds: each bank file and InGameCommon.bun read once, the
    /// same bank shared, the lists as a fresh parse gives them.
    // The read notes are a developer-harness aid (`game_fs::record_reads`).
    #[cfg(feature = "dev-harness")]
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn stitch_banks_and_collision_read_each_file_once() {
        let root = crate::testkit::game_root();
        crate::game_fs::record_reads();
        let held: Vec<StitchBanks> = (0..4).map(|_| StitchBanks::load(&root).expect("stitch banks")).collect();
        let collision = crate::audio::collision::CollisionSounds::load(&root).expect("collision sounds");
        let reads = crate::game_fs::recorded_reads();
        for f in ["SOUND/IG_GLOBAL/MB_Stich_Collision.abk", "SOUND/IG_GLOBAL/MB_Stitch_Whoosh.abk", "GLOBAL/InGameCommon.bun"] {
            assert!(reads.iter().filter(|r| r.as_str() == f).count() <= 1, "{f} read again: {reads:?}");
        }
        assert!(held.iter().all(|b| Arc::ptr_eq(b.banks[0].as_ref().unwrap(), held[0].banks[0].as_ref().unwrap())));
        // The shared parse is the file's.
        let fresh = parse_stitches(&crate::game_fs::read(&root, "GLOBAL/InGameCommon.bun").unwrap());
        assert_eq!(format!("{:?}", *held[0].stitches), format!("{fresh:?}"));
        let bank = Bank::parse(crate::game_fs::read(&root, "SOUND/IG_GLOBAL/MB_Stitch_Whoosh.abk").unwrap()).unwrap();
        assert_eq!(format!("{:?}", held[1].banks[1].as_deref().unwrap()), format!("{bank:?}"));
        drop(collision);
    }

    /// A bank nobody holds is read again (Weak entries); one held is not.
    // The read notes are a developer-harness aid (`game_fs::record_reads`).
    #[cfg(feature = "dev-harness")]
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn shared_banks_live_while_held() {
        let root = crate::testkit::game_root();
        const P: &str = "SOUND/IG_GLOBAL/FX_Misc_MB.abk";
        let warm = prewarm_banks(&root, &[P]);
        crate::game_fs::record_reads();
        let mut a = CarFxSounds::new(&root);
        let mut b = CarFxSounds::new(&root);
        let (pa, pb) = (a.players(P, "x"), b.players(P, "x"));
        assert_eq!(pa, pb);
        assert!(a.bank(P).is_some() && b.bank(P).is_some());
        assert_eq!(crate::game_fs::recorded_reads().iter().filter(|r| r.as_str() == P).count(), 0);
        drop((warm, a, b));
        let _c = shared_bank(&root, P).unwrap();
        assert_eq!(crate::game_fs::recorded_reads().iter().filter(|r| r.as_str() == P).count(), 1);
    }
}
