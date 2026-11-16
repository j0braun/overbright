use crate::*;

/// Values computed once a run for their key, by whichever thread asks
/// first; a thread asking while one is being computed waits for it (the
/// menus' preload and the load that needs it never compute it twice). A
/// failure is not kept.
pub(crate) type MemoCell<V> = std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<V>>>>;
pub(crate) struct Memo<K, V>(pub(crate) std::sync::Mutex<Option<HashMap<K, MemoCell<V>>>>);

/// G35: the run's caches of what the game files give (were statics in
/// the functions below), on `GameData::caches`; shared with the load
/// threads.
#[derive(Default)]
pub(crate) struct Caches {
    /// [`free_roam_routes`]'s networks.
    pub(crate) free_roam: Memo<(PathBuf, u32), nfsu2::formats::ai_routes::RouteFile>,
    /// [`encounter_routes`]'s routes.
    pub(crate) encounter: Memo<(PathBuf, u32, Option<(usize, bool, usize)>), nfsu2::ai::route::AiRoutes>,
    /// [`typed_cameras`]'s groups.
    pub(crate) typed_cameras: std::sync::OnceLock<std::sync::Arc<Vec<CameraGroup>>>,
    /// FrontB's light setups (`frontend_rig_hash`).
    pub(crate) rig_setups: std::sync::OnceLock<HashMap<u32, [nfsu2::render::lighting::RigLight; 3]>>,
    /// PathsFreeRoam.bin's reset zones (`free_roam_reset_zones`).
    pub(crate) reset_zones: std::sync::OnceLock<Vec<nfsu2::ai::route::ResetZone>>,
}

impl<K, V> Default for Memo<K, V> {
    fn default() -> Self {
        Memo::new()
    }
}

impl<K, V> Memo<K, V> {
    pub(crate) const fn new() -> Self {
        Memo(std::sync::Mutex::new(None))
    }
}

impl<K: std::hash::Hash + Eq, V> Memo<K, V> {
    pub(crate) fn get(&self, key: K, make: impl FnOnce() -> Result<V>) -> Result<std::sync::Arc<V>> {
        let cell = self.0.lock().unwrap().get_or_insert_with(HashMap::new).entry(key).or_default().clone();
        let mut slot = cell.lock().unwrap();
        if let Some(v) = slot.as_ref() {
            return Ok(v.clone());
        }
        let v = std::sync::Arc::new(make()?);
        *slot = Some(v.clone());
        Ok(v)
    }
}

/// RoutesFreeRoam.bin pruned as free roam loads it for career `stage`
/// (`RouteFile::finalize_free_roam_paths` with PathsFreeRoam.bin): the same
/// files give the same network, so it is made once a run (it took ~17 ms
/// at every free roam start, twice: the traffic's and the encounters').
pub(crate) fn free_roam_routes(
    caches: &Caches,
    root: &std::path::Path,
    stage: u32,
) -> Result<std::sync::Arc<nfsu2::formats::ai_routes::RouteFile>> {
    caches.free_roam.get((root.to_path_buf(), stage), || {
        let path = nfsu2::game_fs::path(&root, &format!("TRACKS/ROUTES{EXPLORE_REGION}/RoutesFreeRoam.bin"));
        let mut file = nfsu2::formats::ai_routes::RouteFile::parse(
            &std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?,
        );
        let paths = nfsu2::game_fs::path(&root, &format!("TRACKS/ROUTES{EXPLORE_REGION}/PathsFreeRoam.bin"));
        file.finalize_free_roam_paths(
            &std::fs::read(&paths).with_context(|| format!("reading {}", paths.display()))?,
            4000,
            stage,
        );
        Ok(file)
    })
}

/// The encounter racers' routes: [`free_roam_routes`] with the racing
/// speeds for the racers' best `class`, `suv` and the speed `mode`
/// (`AiRoutes::racing_speeds`, ~40 ms, a function of these alone), made
/// once a run for each.
pub(crate) fn encounter_routes(
    caches: &Caches,
    root: &std::path::Path,
    stage: u32,
    curves: Option<&nfsu2::ai::route::ClassCurves>,
    speeds: Option<(usize, bool, usize)>,
) -> Result<std::sync::Arc<nfsu2::ai::route::AiRoutes>> {
    let speeds = speeds.filter(|_| curves.is_some());
    caches.encounter.get((root.to_path_buf(), stage, speeds), || {
        let file = (*free_roam_routes(caches, root, stage)?).clone();
        let mut routes = nfsu2::ai::route::AiRoutes {
            file,
            track: 4000,
            backward: false,
            race_coords: false,
            resets: Vec::new(),
            blocks: Vec::new(),
        };
        if let (Some(curves), Some((class, suv, mode))) = (curves, speeds) {
            routes.racing_speeds(curves, class, suv, mode, None);
        }
        Ok(routes)
    })
}

/// The typed track cameras of GLOBAL/InGameCommon.bun (0x8003B202, the
/// camera manager +0x14: NOS, Jump, Speed, Corner, Burnout, PowerSlide).
pub(crate) fn typed_cameras(caches: &Caches, root: &std::path::Path) -> std::sync::Arc<Vec<CameraGroup>> {
    caches
        .typed_cameras
        .get_or_init(|| {
            let Ok(data) = nfsu2::game_fs::read(&root, "GLOBAL/InGameCommon.bun") else { return Default::default() };
            std::sync::Arc::new(chunks(&data, 0).find(|c| c.id == nfsu2::formats::camera_tracks::TYPED_CAMERAS).and_then(|c| parse_camera_tracks(c).ok()).unwrap_or_default())
        })
        .clone()
}
