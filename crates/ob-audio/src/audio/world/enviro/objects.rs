use super::*;

impl Enviro {
    pub(super) fn pool(&self, kind: Kind) -> &Pool {
        match kind {
            Kind::Fountain => &self.fountains,
            Kind::Plane => &self.planes,
            Kind::Train => &self.trains,
        }
    }

    pub(super) fn pool_mut(&mut self, kind: Kind) -> &mut Pool {
        match kind {
            Kind::Fountain => &mut self.fountains,
            Kind::Plane => &mut self.planes,
            Kind::Train => &mut self.trains,
        }
    }

    pub(super) fn play(bank: &mut Option<EventBank>, module: &str, block: &[i32]) -> Option<u32> {
        let ev = &mut bank.as_mut()?.events;
        ev.module(module).and_then(|m| ev.play(m, block))
    }

    /// The objects' start (slot 11): 0x463920, 0x462810, 0x463020.
    pub(super) fn start(&mut self, kind: Kind, k: usize, pos: Vec3, url: bool) {
        self.stop_events(kind, k);
        let s = &mut self.pool_mut(kind).states[k];
        s.pos = pos;
        s.doppler = Doppler::default();
        // 0x463068: the train's last position is its first.
        if kind == Kind::Train {
            s.last = pos;
        }
        self.start_event(kind, k, url);
    }

    pub(super) fn start_event(&mut self, kind: Kind, k: usize, url: bool) {
        let (env, main) = match kind {
            Kind::Fountain => (Self::play(&mut self.env_bank, "ENV_STATIC", &[4, 0, 0, 0, 0]), None),
            Kind::Train => (Self::play(&mut self.env_bank, "ENV_STATIC", &[0, 0, 0, 0, 0]), None),
            // 0x46286A: FX_MAIN_MEM in a URL race (game mode 4 / race type
            // 8 never offline), else ENV_STATIC variant 1.
            Kind::Plane if url => (None, Self::play(&mut self.main_bank, "FX_MAIN_MEM", &[0, 0, 0, 0, 0])),
            Kind::Plane => (Self::play(&mut self.env_bank, "ENV_STATIC", &[1, 0, 0, 0, 0]), None),
        };
        self.started += env.is_some() as usize + main.is_some() as usize;
        let s = &mut self.pool_mut(kind).states[k];
        s.env = env;
        s.main = main;
    }

    pub(super) fn stop_events(&mut self, kind: Kind, k: usize) {
        let s = &mut self.pool_mut(kind).states[k];
        let (env, main) = (s.env.take(), s.main.take());
        if let (Some(id), Some(b)) = (env, self.env_bank.as_mut()) {
            b.events.release(id);
        }
        if let (Some(id), Some(b)) = (main, self.main_bank.as_mut()) {
            b.events.release(id);
        }
    }

    /// The objects' stop (slot 12): 0x463A60, 0x462A40, 0x463180.
    pub(super) fn stop(&mut self, kind: Kind, k: usize) {
        self.stop_events(kind, k);
    }

    /// The objects' frame (slot 10): 0x463820, 0x462B70, 0x4631E0.
    pub(super) fn frame(&mut self, kind: Kind, k: usize, mix: &Levels, distance: i32, url: bool) {
        let s = &self.pool(kind).states[k];
        let (env, variant, moved, idle) = (s.env, s.variant, s.moved, s.env.is_none() && s.main.is_none());
        match kind {
            Kind::Fountain => {
                let (slot, p2) = if variant == 0 { (0, 0x1450) } else { (1, 0xFA0) };
                if let (Some(id), Some(b)) = (env, self.env_bank.as_mut()) {
                    b.events.set(id, &[4, mix.level(0, slot).clamp(0, 0x7FFF), p2, mix.az(0, slot).clamp(0, 0xFFFF)]);
                }
            }
            Kind::Train => {
                if let (Some(id), Some(b)) = (env, self.env_bank.as_mut()) {
                    b.events.set(id, &[0, mix.level(0, 0).clamp(0, 0x7FFF), 0x1000, mix.az(0, 0).clamp(0, 0xFFFF)]);
                }
            }
            Kind::Plane => {
                if moved < 0.3 {
                    self.stop_events(kind, k);
                    return;
                }
                if idle {
                    self.start_event(kind, k, url);
                }
                let (main, env, dop) = {
                    let s = &self.pool(kind).states[k];
                    (s.main, s.env, s.doppler.ratio(5) * 4096.0)
                };
                if let (Some(id), Some(b)) = (main, self.main_bank.as_mut()) {
                    let az = mix.az(0, 1);
                    let mut v = (distance as f32).min(1000.0);
                    if az > 0x4000 && az < 0xC000 {
                        v = -v;
                    }
                    let p4 = ((v * 2.6) as i32).clamp(-1000, 1000);
                    b.events.set(
                        id,
                        &[0, mix.level(0, 1).clamp(0, 0x7FFF), (dop as i32).clamp(0, 0x4000), az.clamp(0, 0xFFFF), p4],
                    );
                }
                if let (Some(id), Some(b)) = (env, self.env_bank.as_mut()) {
                    b.events.set(
                        id,
                        &[
                            1,
                            mix.level(0, 0).clamp(0, 0x7FFF),
                            (dop as i32).clamp(0, 0x1FA0),
                            mix.az(0, 0).clamp(0, 0xFFFF),
                        ],
                    );
                }
            }
        }
    }
}
