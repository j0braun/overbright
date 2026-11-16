//! Car renderers built ahead of the frame a car first shows
//! (docs/first-use-builds.md "Car renderers ahead"). A traffic car of the
//! drive's pool or an encounter racer gets its renderer in the first frame
//! that draws it (`for_each_drawn_car`); building one there took 2–3.6 ms
//! of that frame. The cars are known before they show: the pool is made
//! with the drive and the encounter racers are loaded and parked. Their
//! renderers are built here on a worker as soon as the cars exist, and the
//! first frame that draws one takes it when it is ready and was built for
//! what that frame would build it for ([`BuildKey`]); otherwise the frame
//! builds it in place as before. A renderer that has not drawn holds no
//! state, so the frame draws the same either way.
use crate::sim::car_id::CarId;
use crate::*;

/// What a car renderer is built from besides its car (`CarRenderer::new`):
/// the device, the window's format, the car texture sampler's filter (the
/// Texture Filtering row and the anisotropy) and the texture packs'
/// generation.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct BuildKey {
    device: u64,
    format: wgpu::TextureFormat,
    filter: wgpu::FilterMode,
    anisotropy: u16,
    packs: u64,
}

impl BuildKey {
    pub(crate) fn now(gpu: &Gpu, format: wgpu::TextureFormat, opts: &nfsu2::render::RenderOptions) -> BuildKey {
        let sampler = nfsu2::render::display::texture_sampler("car", opts.anisotropy);
        BuildKey {
            device: gpu.build_tag(),
            format,
            filter: sampler.mag_filter,
            anisotropy: sampler.anisotropy_clamp,
            packs: nfsu2::render::pack_reload::generation(),
        }
    }
}

type Built = Vec<(u64, BuildKey, CarRenderer)>;

static OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether renderers and the kept world are made beside the loads that
/// precede their use (the rivals' with their cars, the player's with a
/// preloaded car, `loads::kept`): with the programs made ahead, unless a
/// test switched it off to compare against the frame's or the load's own.
pub(crate) fn with_loads() -> bool {
    crate::loads::warm::ProgramsAhead::enabled() && !OFF.load(std::sync::atomic::Ordering::Relaxed)
}

/// `car`'s renderer as a frame would build it, with the key of what it
/// was built for; None when a texture pack changed during the build.
pub(crate) fn build(
    gpu: &Gpu,
    car: &Car,
    format: wgpu::TextureFormat,
    opts: &nfsu2::render::RenderOptions,
) -> Option<(BuildKey, CarRenderer)> {
    let key = BuildKey::now(gpu, format, opts);
    let r = CarRenderer::new(gpu, car, format, opts);
    (BuildKey::now(gpu, format, opts) == key).then_some((key, r))
}

/// Off (true) for a test comparing against the work done in place.
#[cfg(test)]
pub(crate) fn set_with_loads_off(off: bool) {
    OFF.store(off, std::sync::atomic::Ordering::Relaxed);
}

/// The renderers built ahead and the worker building them.
#[derive(Default)]
pub(crate) struct CarsAhead {
    /// The worker's batch, when one runs.
    job: Option<std::thread::JoinHandle<Built>>,
    /// The cars asked for (built, being built, or taken), by id key, with
    /// a handle telling whether the car is alive.
    asked: std::collections::HashMap<u64, std::sync::Weak<u64>>,
    /// Built and not yet taken.
    ready: std::collections::HashMap<u64, (BuildKey, CarRenderer)>,
    /// How many a frame took (a measuring aid; the tests read it).
    pub(crate) taken: u64,
}

impl CarsAhead {
    /// Whether a renderer built ahead waits for the car `id`.
    #[cfg(test)]
    pub(crate) fn has_ready(&self, id: u64) -> bool {
        self.ready.contains_key(&id)
    }

    /// The renderer built ahead for `id`, when it was built for `key`.
    pub(crate) fn take(&mut self, id: u64, key: &BuildKey) -> Option<CarRenderer> {
        match self.ready.remove(&id) {
            Some((k, r)) if k == *key => {
                self.taken += 1;
                Some(r)
            }
            // Built for other settings: dropped off the frame.
            Some((_, r)) => {
                std::thread::spawn(move || drop(r));
                None
            }
            None => None,
        }
    }

    /// A renderer built ahead elsewhere for the car `id` (a race's rivals,
    /// built beside their cars' loads), for the frame that first draws it.
    pub(crate) fn offer(&mut self, id: &CarId, key: BuildKey, renderer: CarRenderer) {
        self.asked.insert(id.key(), id.watch());
        self.ready.insert(id.key(), (key, renderer));
    }

    /// The cars that are gone are forgotten, their renderers dropped off
    /// the frame.
    fn sweep(&mut self) {
        let mut gone = Vec::new();
        self.asked.retain(|id, alive| {
            let keep = alive.strong_count() > 0;
            if !keep && let Some((_, r)) = self.ready.remove(id) {
                gone.push(r);
            }
            keep
        });
        if !gone.is_empty() {
            std::thread::spawn(move || drop(gone));
        }
    }

    /// How many are built and waiting (for the tests).
    #[cfg(test)]
    pub(crate) fn ready_count(&self) -> usize {
        self.ready.len()
    }
}

impl Game {
    /// Once a tick: the drive's traffic pool and encounter racers that have
    /// no renderer yet get one built on a worker (one batch at a time); a
    /// batch that has ended is put with the ready ones.
    pub(crate) fn cars_ahead_tick(&mut self) {
        let ahead = &mut self.render.cars_ahead;
        ahead.sweep();
        // Built in place meanwhile (the car showed first): dropped.
        let cars = &self.render.cars;
        let late: Vec<CarRenderer> = {
            let ids: Vec<u64> = ahead.ready.keys().copied().filter(|&id| cars.has_key(id)).collect();
            ids.iter().filter_map(|id| ahead.ready.remove(id)).map(|(_, r)| r).collect()
        };
        if !late.is_empty() {
            std::thread::spawn(move || drop(late));
        }
        if ahead.job.as_ref().is_some_and(|j| j.is_finished()) {
            let built = ahead.job.take().unwrap().join().unwrap_or_default();
            for (id, key, r) in built {
                if ahead.asked.contains_key(&id) {
                    ahead.ready.insert(id, (key, r));
                }
            }
        }
        if ahead.job.is_some() || !crate::loads::warm::ProgramsAhead::enabled() {
            return;
        }
        let Some(w) = self.platform.window.as_ref() else { return };
        if self.sim.drive.is_none() {
            return;
        }
        // The cars that exist, are not drawn yet (the next frame builds
        // those as before) and have no renderer, in drawing order.
        let mut cars: Vec<(&CarId, &std::sync::Arc<Car>)> = Vec::new();
        let race = match &self.mode {
            Mode::Race(r) => Some(r),
            _ => None,
        };
        let explore = matches!(self.mode, Mode::Explore);
        if let Some(e) = self.sim.encounters.as_ref() {
            cars.extend(e.cars.iter().filter(|c| !(explore && c.active)).map(|c| (&c.id, &c.car)));
        }
        // Free roam's traffic is drawn in free roam, the race's unless it is
        // paused (the start's countdown).
        let drawn = crate::render::frame::cars::traffic_drawn;
        if let Some(t) = self.sim.traffic.as_ref() {
            cars.extend(t.cars.iter().filter(|c| !(explore && drawn(c))).map(|c| (&c.id, &c.car)));
        }
        if let Some(t) = race.and_then(|r| r.racers.traffic.as_ref()) {
            cars.extend(t.cars.iter().filter(|c| t.paused || !drawn(c)).map(|c| (&c.id, &c.car)));
        }
        let ahead = &mut self.render.cars_ahead;
        let mut wanted: Vec<(u64, std::sync::Arc<Car>)> = Vec::new();
        for (id, car) in cars {
            if self.render.cars.get(id).is_none() && !ahead.asked.contains_key(&id.key()) {
                ahead.asked.insert(id.key(), id.watch());
                wanted.push((id.key(), car.clone()));
            }
        }
        if wanted.is_empty() {
            return;
        }
        let (gpu, format) = (w.gpu.clone(), w.config.format);
        let opts = nfsu2::render::RenderOptions::in_force();
        ahead.job = std::thread::Builder::new()
            .name("cars ahead".into())
            .spawn(move || {
                let _t = nfsu2::prof::span("car renderers ahead");
                let mut built = Vec::new();
                for (id, car) in wanted {
                    let key = BuildKey::now(&gpu, format, &opts);
                    let r = CarRenderer::new(&gpu, &car, format, &opts);
                    // A texture pack changed during the build: not kept.
                    if BuildKey::now(&gpu, format, &opts) == key {
                        built.push((id, key, r));
                    }
                }
                built
            })
            .ok();
    }
}
