use super::*;

/// A bank's events: its programs and running instances.
pub struct Events {
    pub programs: Vec<Option<Arc<Program>>>,
    pub(super) names: Vec<(String, usize)>,
    pub(super) instances: Vec<(u32, Instance)>,
    pub(super) next: u32,
    pub(super) clock: f32,
    pub rng: Rng,
}

impl Events {
    pub fn new(bank: &Bank) -> Events {
        let file: Arc<[u8]> = bank.data.clone().into();
        let programs = (0..bank.modules.len()).map(|m| Program::new(bank, file.clone(), m).map(Arc::new)).collect();
        Events { programs, names: bank.events.clone(), instances: Vec::new(), next: 0, clock: 0.0, rng: Rng::default() }
    }

    /// The module an event name starts.
    pub fn module(&self, name: &str) -> Option<usize> {
        self.names.iter().find(|e| e.0 == name).map(|e| e.1)
    }

    /// Start an event (0x7358C0 → 0x721096, then the parameter block);
    /// `None` at the module's instance limit.
    pub fn play(&mut self, module: usize, params: &[i32]) -> Option<u32> {
        let p = self.programs.get(module)?.clone()?;
        let live = self.instances.iter().filter(|i| Arc::ptr_eq(&i.1.program, &p)).count();
        if live >= p.max as usize {
            return None;
        }
        let mut inst = Instance::new(p);
        inst.set_params(params);
        self.next = self.next.wrapping_add(1).max(1);
        self.instances.push((self.next, inst));
        if log::enabled() {
            log::row(self as *const _ as usize, self.name_of(module), self.next, params, 'p');
        }
        Some(self.next)
    }

    /// The game's per-frame parameter dispatch (0x735580).
    pub fn set(&mut self, id: u32, params: &[i32]) {
        if let Some(i) = self.get_mut(id) {
            i.set_params(params);
        }
        if !log::enabled() {
            return;
        }
        let module =
            self.instances.iter().find(|i| i.0 == id).and_then(|i| {
                self.programs.iter().position(|p| p.as_ref().is_some_and(|p| Arc::ptr_eq(p, &i.1.program)))
            });
        if let Some(m) = module {
            log::row(self as *const _ as usize, self.name_of(m), id, params, 's');
        }
    }

    pub(super) fn name_of(&self, module: usize) -> &str {
        self.names.iter().find(|e| e.1 == module).map_or("?", |e| e.0.as_str())
    }

    /// The game releasing its handle (0x735550).
    pub fn release(&mut self, id: u32) {
        if let Some(i) = self.get_mut(id) {
            i.release();
        }
    }

    pub fn alive(&self, id: u32) -> bool {
        self.instances.iter().any(|i| i.0 == id)
    }

    pub fn get(&self, id: u32) -> Option<&Instance> {
        self.instances.iter().find(|i| i.0 == id).map(|i| &i.1)
    }

    pub(super) fn get_mut(&mut self, id: u32) -> Option<&mut Instance> {
        self.instances.iter_mut().find(|i| i.0 == id).map(|i| &mut i.1)
    }

    /// Stop an instance at once (its voices too).
    pub fn kill(&mut self, id: u32, host: &mut dyn VoiceHost) {
        if let Some(k) = self.instances.iter().position(|i| i.0 == id) {
            self.instances[k].1.stop(host);
            self.instances.remove(k);
            if log::enabled() {
                log::row(self as *const _ as usize, "kill", id, &[], 'e');
            }
        }
    }

    pub fn kill_all(&mut self, host: &mut dyn VoiceHost) {
        for (_, mut i) in self.instances.drain(..) {
            i.stop(host);
        }
    }

    /// Run the library ticks that fall in `dt` seconds.
    pub fn advance(&mut self, dt: f32, host: &mut dyn VoiceHost) {
        self.clock += dt * 1000.0;
        while self.clock >= TICK_MS {
            self.clock -= TICK_MS;
            self.tick(host);
        }
    }

    pub fn tick(&mut self, host: &mut dyn VoiceHost) {
        let stems = super::super::mixer::stems::enabled();
        for (_, i) in &mut self.instances {
            if stems {
                let m = self
                    .programs
                    .iter()
                    .position(|p| p.as_ref().is_some_and(|p| Arc::ptr_eq(p, &i.program)))
                    .unwrap_or(0);
                let name = self.names.iter().find(|e| e.1 == m).map_or("?", |e| e.0.as_str());
                // The player's own engine instance (CAR p12 = 1) apart from the others' under
                // NFSU2_AUDIO_STEMS_SPLIT.
                let split = super::super::mixer::stems::split();
                let player = name == "CAR" && split && i.param(12) == Some(1);
                // The AI racers' one-event skids (their three levels equal, from one mix-map
                // copy's slots 3, 5, 4) apart from the player's two under the same split.
                let ai_skid = name == "FX_SKID"
                    && split
                    && i.param(0).is_some_and(|l| l > 0 && Some(l) == i.param(1) && Some(l) == i.param(2));
                super::super::mixer::stems::set(if player {
                    "CAR.player"
                } else if ai_skid {
                    "FX_SKID.ai"
                } else {
                    name
                });
            }
            i.tick(host, &mut self.rng);
        }
        if stems {
            super::super::mixer::stems::clear();
        }
        let me = self as *const _ as usize;
        for (id, i) in self.instances.iter().filter(|_| log::enabled()) {
            if i.dead {
                let m = self
                    .programs
                    .iter()
                    .position(|p| p.as_ref().is_some_and(|p| Arc::ptr_eq(p, &i.program)))
                    .unwrap_or(0);
                log::row(me, self.name_of(m), *id, &[], 'e');
            }
        }
        self.instances.retain(|i| !i.1.dead);
    }
}
