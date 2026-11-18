/// CARSFX_TrafficStereo's / CARSFX_Stereo's state (ctors 0x47F410 /
/// 0x4764B0, vtables 0x78BC80 / 0x78B608: the same slots 7–12).
#[derive(Default)]
pub(super) struct Stereo {
    /// +0xA0: this car plays a stereo; +0xB8 the ENV_STATIC variant.
    pub(super) on: bool,
    pub(super) variant: i32,
    /// +0xB0: the playing ENV_STATIC instance.
    pub(super) event: Option<u32>,
}

impl Stereo {
    /// The context's attach (0x469B20) runs the objects' slot 11: for
    /// CARSFX_TrafficStereo 0x476580. A traffic car (car +0x14 → +4 = 3)
    /// plays one with odds 1/3 (Rand_Int(3) = 0) unless it is a taxi
    /// (then +0xA0 = 0, no draw); the variant is 8 + Rand_Int(2) either
    /// way (ENV_COMMON_MB sounds 7 / 8, looped 1.86 s / 2.58 s).
    pub(super) fn attach(&mut self, taxi: bool) {
        if taxi {
            self.on = false;
        } else {
            self.on = crate::audio::collision::rand(3) == 0;
        }
        self.variant = 8 + crate::audio::collision::rand(2) as i32;
    }

    /// Slot 7 (0x4765F0), run for an AI racer's context when the world
    /// starts (its `Rand_Int(2)` is one of the nine the world build makes,
    /// [`crate::audio::collision::stereo_roll`]) (0x60A7B3 → 0x458850 → the AI manager's slot 5 0x463BB0 →
    /// the context's slot 4 0x469CD0 → 0x464BB0; round 14: the only direct
    /// call of 0x469CD0 found is 0x481F7D, so this path and the count per
    // prov: inferred audio.traffic.racer_roll
    /// racer are **inferred**, see formats.md): +0xB8 = 8 +
    /// Rand_Int(2); a racer (car +0x14 → +4 = 2) steps the session's
    /// counter 0x82CB6C (.bss, 0 at boot) to (+1) mod 6 and plays when it
    /// lands on 0.
    pub(super) fn racer_roll(&mut self, counter: &mut u32) {
        self.variant = 8 + crate::audio::collision::stereo_roll() as i32;
        *counter = (*counter + 1) % 6;
        self.on = *counter == 0;
    }

    /// Slots 9 (0x4635B0) and 10 (0x4636E0): with +0xA0 (and a car) the
    /// ENV_STATIC loop starts when slot 0x38(0) is not 0 and stops when
    /// it is; p1 = level, p2 = 0x1000, p3 = azimuth. Returns whether one
    /// started.
    pub(super) fn frame(&mut self, ev: &mut crate::audio::aems::Events, level: i32, azimuth: i32) -> bool {
        if !self.on {
            return false;
        }
        let mut started = false;
        match self.event {
            Some(e) if level == 0 => {
                ev.release(e);
                self.event = None;
            }
            None if level != 0 => {
                self.event = ev.module("ENV_STATIC").and_then(|m| ev.play(m, &[self.variant.clamp(0, 50), 0, 0, 0, 0]));
                started = self.event.is_some();
            }
            _ => {}
        }
        if let Some(e) = self.event {
            ev.set(e, &[self.variant, level.clamp(0, 0x7FFF), 0x1000, azimuth.clamp(0, 0xFFFF)]);
        }
        started
    }

    /// Slot 12 (0x463540): the instance goes.
    pub(super) fn release(&mut self, ev: Option<&mut crate::audio::aems::Events>) {
        if let (Some(e), Some(ev)) = (self.event.take(), ev) {
            ev.release(e);
        }
    }
}
