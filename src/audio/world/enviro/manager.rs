use super::*;
use crate::audio::carfx::{Sink, close_bank, open_bank, run_bank};
use std::path::Path;

impl Enviro {
    pub fn new(root: &Path) -> Enviro {
        Enviro {
            sounds: CarFxSounds::new(root),
            gains: HashMap::new(),
            env_bank: None,
            main_bank: None,
            setup: None,
            fountains: Pool::new(Kind::Fountain, 0, 0),
            planes: Pool::new(Kind::Plane, 0, 0),
            trains: Pool::new(Kind::Train, 0, 0),
            started: 0,
            log: Vec::new(),
        }
    }

    /// The managers' set-up for a race or free roam (vtable +8): the
    /// states made afresh.
    pub fn set_up(&mut self, sink: &mut dyn Sink, setup: Setup) {
        if self.setup == Some(setup) {
            return;
        }
        self.silence(sink);
        let fountains = if (4000..4200).contains(&setup.track) { 5 } else { 0 };
        self.fountains = Pool::new(Kind::Fountain, fountains, FOUNTAINS_0.len() + FOUNTAINS_1.len());
        self.planes = Pool::new(Kind::Plane, 3, plane_table(&setup).len());
        self.trains = Pool::new(Kind::Train, 6, TRAINS.len());
        self.setup = Some(setup);
    }

    /// The states per section for the mix map (4, 7, 8) and each one's 2D
    /// position (its controller's).
    pub fn mix_positions(&self) -> [Vec<[f32; 2]>; 3] {
        let p = |pool: &Pool| pool.states.iter().map(|s| [s.pos.x, s.pos.y]).collect();
        [p(&self.fountains), p(&self.planes), p(&self.trains)]
    }

    pub fn silence(&mut self, sink: &mut dyn Sink) {
        close_bank(&mut self.env_bank, &mut self.sounds, &mut self.gains, sink);
        close_bank(&mut self.main_bank, &mut self.sounds, &mut self.gains, sink);
        for pool in [&mut self.fountains, &mut self.planes, &mut self.trains] {
            for s in &mut pool.states {
                *s = State::default();
            }
        }
        self.setup = None;
    }

    /// Whether any state plays (for the debug log).
    pub fn playing(&self) -> [usize; 3] {
        let n = |p: &Pool| p.states.iter().filter(|s| s.env.is_some() || s.main.is_some()).count();
        [n(&self.fountains), n(&self.planes), n(&self.trains)]
    }

    pub fn update(&mut self, sink: &mut dyn Sink, i: &Input, dt: f32) {
        self.started = 0;
        self.log.clear();
        let Some(setup) = self.setup else { return };
        if i.players.is_empty() {
            return;
        }
        open_bank(&mut self.env_bank, &mut self.sounds, &mut self.gains, sink, ENV_COMMON);
        open_bank(&mut self.main_bank, &mut self.sounds, &mut self.gains, sink, MAIN_MEM);
        let planes = plane_table(&setup);
        let find = |id: u32, hash: u32| i.points.iter().find(|p| p.id == id && p.hash == hash).copied();
        let nearest = |p: Vec3| i.players.iter().map(|q| q.distance(p)).fold(f32::INFINITY, f32::min);
        let fountain_at = |k: usize| -> Vec3 {
            if k < FOUNTAINS_0.len() {
                Vec3::from(FOUNTAINS_0[k])
            } else {
                Vec3::from(FOUNTAINS_1[k - FOUNTAINS_0.len()])
            }
        };
        // The managers (0x4647E0, 0x464410, 0x464600).
        let mut starts: Vec<(Kind, usize)> = Vec::new();
        let mut stops: Vec<(Kind, usize)> = Vec::new();
        for pool in [&mut self.fountains, &mut self.planes, &mut self.trains] {
            let kind = pool.kind;
            if pool.states.is_empty() {
                continue;
            }
            let entries = match kind {
                Kind::Fountain => FOUNTAINS_0.len() + FOUNTAINS_1.len(),
                Kind::Plane => planes.len(),
                Kind::Train => TRAINS.len(),
            };
            let locate = |e: usize| -> Option<AnimPoint> {
                match kind {
                    Kind::Fountain => Some(AnimPoint { id: 0, hash: 0, position: fountain_at(e), hidden: false }),
                    Kind::Plane => find(planes[e].0, planes[e].1),
                    Kind::Train => find(TRAINS[e].0, TRAINS[e].1),
                }
            };
            for e in 0..entries {
                if kind == Kind::Train && pool.dead[e] {
                    continue;
                }
                let Some(pt) = locate(e) else {
                    if kind == Kind::Train {
                        pool.dead[e] = true;
                    }
                    continue;
                };
                let d = nearest(pt.position);
                let r = kind.radius();
                if let Some(k) = pool.states.iter().position(|s| s.entry == Some(e)) {
                    if d >= r || pt.hidden {
                        pool.states[k].entry = None;
                        stops.push((kind, k));
                    }
                    continue;
                }
                if d >= r || pt.hidden {
                    continue;
                }
                let k = match pool.states.iter().position(|s| s.entry.is_none()) {
                    Some(k) => Some(k),
                    None => pool.states.iter().position(|s| {
                        let other = s.entry.and_then(locate).map_or(Vec3::ZERO, |p| p.position);
                        d < nearest(other)
                    }),
                };
                if let Some(k) = k {
                    if pool.states[k].entry.is_some() {
                        stops.push((kind, k));
                    }
                    pool.states[k].entry = Some(e);
                    pool.states[k].variant = (kind == Kind::Fountain && e >= FOUNTAINS_0.len()) as i32;
                    starts.push((kind, k));
                }
            }
        }
        for (kind, k) in stops {
            self.stop(kind, k);
        }
        let starts_now = starts.clone();
        for (kind, k) in starts {
            let pos = {
                let pool = self.pool(kind);
                let e = pool.states[k].entry;
                match kind {
                    Kind::Fountain => e.map(fountain_at),
                    Kind::Plane => e.and_then(|e| find(planes[e].0, planes[e].1)).map(|p| p.position),
                    Kind::Train => e.and_then(|e| find(TRAINS[e].0, TRAINS[e].1)).map(|p| p.position),
                }
            };
            self.start(kind, k, pos.unwrap_or(Vec3::ZERO), i.url);
        }
        // The objects (0x463BD0): position, then frame.
        for kind in [Kind::Fountain, Kind::Plane, Kind::Train] {
            let n = self.pool(kind).states.len();
            for k in 0..n {
                let entry = self.pool(kind).states[k].entry;
                let Some(e) = entry else { continue };
                if kind != Kind::Fountain {
                    let (id, hash) = if kind == Kind::Plane { planes[e] } else { TRAINS[e] };
                    let s = &mut self.pool_mut(kind).states[k];
                    if let Some(p) = find(id, hash) {
                        s.pos = p.position;
                    }
                    // +0xD0, the move since the last update, taken per 60 Hz
                    // frame (**inferred**: the original's frame).
                    s.moved = if dt > 0.0 { s.pos.distance(s.last) / (dt * 60.0) } else { 0.0 };
                    s.last = s.pos;
                }
                let d = {
                    let s = &self.pool(kind).states[k];
                    s.pos.truncate().distance(i.listener.truncate()) as i32
                };
                self.pool_mut(kind).states[k].doppler.update(d, dt);
                // A state the mix has no copy of yet reads 0 (0x45E390 with no slot).
                let mix = i.mix[kind as usize].get(k).copied().unwrap_or(Levels([[0; 8]; 16]));
                let before = self.started;
                self.frame(kind, k, &mix, d, i.url);
                if self.started > before || starts_now.contains(&(kind, k)) {
                    let name = ["fountain", "plane", "train"][kind as usize];
                    let slot = if kind == Kind::Plane && i.url
                        || kind == Kind::Fountain && self.pool(kind).states[k].variant == 1
                    {
                        1
                    } else {
                        0
                    };
                    self.log.push((name, e, d as f32, mix.level(0, slot)));
                }
            }
        }
        run_bank(&mut self.env_bank, &mut self.sounds, &mut self.gains, sink, dt);
        run_bank(&mut self.main_bank, &mut self.sounds, &mut self.gains, sink, dt);
    }
}
