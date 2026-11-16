//! The main menu's garage scene read while the title screen is up: its
//! platform (SHOWROOM, read and parsed), the front end's light rigs and the
//! FE car as UI_Main asks for it, on a thread of its own and the loads'
//! pool. The menu's first tick read and parsed all of this itself (12–30 ms
//! of the main thread, and the files' reads: up to 0.6 s when the system
//! had dropped them). It takes the scene when it is done and is the one
//! that tick asks for (the same platform, the same car, parts and
//! performance parts) and makes the platform's renderer and the FE car's
//! then, as before (the GPU is left to the title's frames; the front end's
//! cube is the platforms' in the order they are shown, `fe_cube_texels`);
//! anything else (a scene not done yet: never waited for) is dropped and
//! the tick makes its own. The original reads SHOWROOM with the front end
//! at its start, before the boot screens (0x57FEFA).
use crate::*;

/// The scene being made, and what it is made for.
pub(crate) struct GarageAhead {
    platform: String,
    car: GarageCarKey,
    job: std::thread::JoinHandle<Option<(GarageScene, Car)>>,
}

impl GarageAhead {
    /// The scene is made (or its making ended).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn is_done(&self) -> bool {
        self.job.is_finished()
    }
}

impl Game {
    /// Starts the main menu's scene (once a run, with a window and a car
    /// to show; `NFSU2_NO_PRELOAD=1`: never).
    pub(crate) fn garage_ahead_start(&mut self) {
        if self.loads.garage_ahead.is_some() || self.render.garage.is_some() || !Preload::enabled() {
            return;
        }
        if self.platform.window.is_none() {
            return;
        }
        let (Some(name), installed, perf) = self.main_menu_ride() else { return };
        let platform = nfsu2::garage::PLATFORMS[1].to_string();
        let car: GarageCarKey = (name, installed, perf);
        let (root, caches, packages) =
            (self.game_data.root.clone(), self.game_data.caches.clone(), self.game_data.career.packages.clone());
        let (p, key) = (platform.clone(), car.clone());
        let job = std::thread::Builder::new().name("garage ahead".into()).spawn(move || {
            let _t = nfsu2::prof::span("garage ahead");
            // The parallel parts on the loads' own pool (`Preload::pool`):
            // the frame's parallel work never queues behind them. This thread
            // only waits; no pool thread waits for another.
            let pool = Preload::pool();
            std::thread::scope(|s| {
                let car = s.spawn(|| {
                    pool.install(|| Car::load_upgraded(&root, &key.0, &key.1, &packages, &|id| key.2.contains(&id)))
                });
                s.spawn(|| frontend_rig(&caches, &root, &p));
                let scene = pool.install(|| garage_platform(&root, &p).map(GarageScene::new));
                Some((scene?, car.join().ok()?.ok()?))
            })
        });
        if let Ok(job) = job {
            self.loads.garage_ahead = Some(GarageAhead { platform, car, job });
        }
    }

    /// The scene made ahead put in place for the tick's request when it is
    /// the one the tick would make, before the tick runs (then not fresh:
    /// it makes the platform's and the FE car's renderers, with the window
    /// and options in force then). Any garage request ends it.
    pub(crate) fn garage_ahead_take(&mut self) {
        let Some(request) = self.garage_fe.request.as_ref() else { return };
        let Some(a) = self.loads.garage_ahead.take() else { return };
        let fresh = self.render.garage.as_ref().is_none_or(|g| g.platform.name != request.platform);
        let wanted: Option<GarageCarKey> =
            request.car.as_ref().map(|n| (n.clone(), request.installed.clone(), request.perf.clone()));
        if !fresh
            || a.platform != request.platform
            || request.lights != request.platform
            || wanted.as_ref() != Some(&a.car)
        {
            return;
        }
        // Never waited for: a scene still being made is left to finish on
        // its own and the tick makes its own, as before.
        if !a.job.is_finished() {
            return;
        }
        let _t = nfsu2::prof::span("garage ahead: taken");
        let Ok(Some((mut g, car))) = a.job.join() else { return };
        let (name, installed, perf) = a.car;
        g.car = Some((name, installed, car, None));
        g.car_perf = perf;
        self.render.garage = Some(g);
        self.loads.garage_ahead_taken += 1;
    }
}
