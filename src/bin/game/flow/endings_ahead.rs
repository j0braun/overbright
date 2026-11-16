//! The race endings' files read ahead (docs/perf-budget.md "The race end"):
//! the ending scene `play_race_ending` picks at the finish is one of
//! EndingNis01..04, and its load (the NIS table, the scene, the cameras, the
//! scene's characters' bundle parsed) took about 4 ms of the finish's frame.
//! They are pure readings of the game's files, so they are made on a thread
//! when a race starts, for all four, and the finish takes the one it picks
//! (the pick itself, its random draw included, is unchanged). A finish that
//! comes before the thread is done, or a reading that failed, loads in place
//! as before (its messages then printed as before).
use crate::*;

/// What the ending's load reads, for each of the four scenes.
#[derive(Default)]
pub(crate) struct Endings {
    pub(crate) table: Option<nfsu2::shop_scene::IntroTable>,
    pub(crate) cameras: Option<Vec<nfsu2::formats::camera_tracks::CameraGroup>>,
    /// By the scene's number (1..=4): the scene and its characters
    /// (None: the bundle missing or empty, as the load reads it).
    pub(crate) scenes: std::collections::HashMap<
        u8,
        (nfsu2::shop_scene::Scene, Option<std::sync::Arc<nfsu2::character::SceneCharacters>>),
    >,
}

impl Endings {
    /// Read on the calling thread (a worker's).
    pub(crate) fn read(root: &std::path::Path) -> Endings {
        use nfsu2::shop_scene::{IntroTable, Scene, SceneKind};
        let _t = nfsu2::prof::span("race endings ahead");
        let mut e = Endings {
            table: IntroTable::load(root).ok(),
            cameras: Some(nfsu2::shop_scene::load_cameras(root)),
            ..Default::default()
        };
        for n in 1..=4u8 {
            if let Ok(scene) = Scene::load(root, SceneKind::Ending(n)) {
                let crowd = crowd_of(root, &scene);
                e.scenes.insert(n, (scene, crowd));
            }
        }
        e
    }
}

/// The scene's characters as the ending's load reads them
/// (`NIS/Scene_<name>_BundleB.bun`; None when missing, unreadable or empty).
pub(crate) fn crowd_of(
    root: &std::path::Path,
    scene: &nfsu2::shop_scene::Scene,
) -> Option<std::sync::Arc<nfsu2::character::SceneCharacters>> {
    nfsu2::game_fs::read(root, &format!("NIS/Scene_{}_BundleB.bun", scene.kind.name()))
        .ok()
        .and_then(|b| nfsu2::character::SceneCharacters::parse(&b).ok())
        .filter(|c| !c.instances.is_empty())
        .map(std::sync::Arc::new)
}

impl Game {
    /// A race started: the endings read on a thread of their own.
    pub(crate) fn endings_ahead_start(&mut self) {
        let root = self.game_data.root.clone();
        self.race_flow.endings_ahead =
            std::thread::Builder::new().name("race endings".into()).spawn(move || Endings::read(&root)).ok();
    }

    /// The endings read ahead, when the thread is done (else None: the
    /// finish loads in place).
    pub(crate) fn endings_ahead_take(&mut self) -> Option<Endings> {
        match self.race_flow.endings_ahead.take() {
            Some(j) if j.is_finished() => j.join().ok(),
            Some(j) => {
                // Not done: left to end on its own.
                drop(j);
                None
            }
            None => None,
        }
    }
}
