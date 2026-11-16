use crate::*;

impl Game {
    /// The FE car's hydraulics for 0x4A6030: the fitted slot 0xA1 part's
    /// level (FUN_00610300(0xA1) byte +5 >> 5) and, when the suspension
    /// category's level is not 0, its first gain (0x5A6590(5, 3) on the
    /// car's upgrade state +0x2F0) × the car type's +0x714 (front) and
    /// +0x718 (rear).
    pub(crate) fn garage_hydraulics(
        &self,
        car: Option<&str>,
        installed: &std::collections::BTreeMap<u8, u32>,
        perf: &std::collections::BTreeSet<u8>,
    ) -> (u32, [f32; 2]) {
        let Some(car) = car else { return (0, [0.0; 2]) };
        // 0x4A6030 reads the level only while 0x8363F8 is set: UI_ICEMAIN's
        // list entry 0x556940 clears it, then sets it for Hydraulics (slot
        // 0xA1, 0x556A41); Back (0x54FB80(…, 0) at 0x568B8B), the list's
        // exit 0x5605C0 (0x560648) and 0x8ACAA8DF (0x568A1B) clear it. So
        // the FE car pumps only in the Hydraulics list (Wine, the finished-career save:
        // still in the strip and the other lists, LVL 3 pumping in its
        // list).
        if !self.hydraulics_list_open() {
            return (0, [0.0; 2]);
        }
        let level =
            installed.get(&0xA1).and_then(|&h| self.game_data.parts.find(car, 0xA1, Some(h), None)).map_or(0, |x| x.level as u32);
        if level == 0 {
            return (0, [0.0; 2]);
        }
        let (mean, gains) = nfsu2::performance::level(&self.game_data.career.packages, &|id| perf.contains(&id), 5, 3);
        let lowering = match self.game_data.car_records.iter().find(|(n, _)| n.eq_ignore_ascii_case(car)) {
            Some((_, r)) if mean != 0.0 && r.len() >= 0x71C => {
                let f = |o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
                [gains[0] * f(0x714), gains[0] * f(0x718)]
            }
            _ => [0.0; 2],
        };
        (level, lowering)
    }

    /// UI_ICEMAIN's Nitrous Purge list is open (UI_IcePartsOverlay +0x110
    /// → +0x48 = 0xA2).
    pub(crate) fn purge_list_open(&self) -> bool {
        matches!(&self.mode, Mode::Menu(Menu::Parts(b)) if b.package == "UI_ICEMAIN" && b.sub.is_none() && b.part.is_some() && b.categories.get(b.category).is_some_and(|c| c.slot == 0xA2))
    }

    /// Test N2O Purge on or off (UI_IcePartsOverlay +0x116): on starts
    /// FX_PURGE (0x458B30), off sets its stop flag (0x458C40).
    pub(crate) fn purge_test_set(&mut self, on: bool) {
        if on == self.shop.purge_test {
            return;
        }
        self.shop.purge_test = on;
        // **Inferred**: the nitrous bank of the car shown, by the in-game
        // rule (0x465830: Nitrous_01 for nitrous level 2 or 3).
        let level = self.profile.as_ref().map_or(0, |p| {
            (12u8..=14).filter(|id| p.perf.contains(id)).map(|id| (id - 11) as usize).max().unwrap_or(0)
        });
        let Some(a) = self.audio.as_mut() else { return };
        if on {
            a.fe.purge_start(&mut a.mixer, &nfsu2::audio::carfx::nitrous_path(level));
        } else {
            a.fe.purge_end();
        }
    }

    /// g_GarageHydraulicsShown (0x8363F8): UI_ICEMAIN's Hydraulics list
    /// is open (the shops' and the Parts Storage's).
    fn hydraulics_list_open(&self) -> bool {
        matches!(self.preview_menu(), Some(Menu::Parts(b)) if b.package == "UI_ICEMAIN" && b.sub.is_none() && b.part.is_some() && b.categories.get(b.category).is_some_and(|c| c.slot == 0xA1))
    }

    /// The driven car's hydraulics level (car record +0x581: the fitted
    /// slot 0xA1 part's level, 0x61BE24), or the override 0x7E949C when 0
    /// or more (`NFSU2_HYDRAULICS`, a test aid standing for that global,
    /// −1 in the game).
    fn hydraulics_level(&self) -> u8 {
        if let Some(v) = debug_env().hydraulics.as_deref().and_then(|v| v.parse::<i32>().ok()).filter(|&v| v >= 0) {
            return v.min(3) as u8;
        }
        let (Some(p), Some(d)) = (self.profile.as_ref(), self.sim.drive.as_ref()) else { return 0 };
        if !d.car.name.eq_ignore_ascii_case(&p.car) {
            return 0;
        }
        p.installed.get(&0xA1).and_then(|&h| self.game_data.parts.find(&p.car, 0xA1, Some(h), None)).map_or(0, |x| x.level)
    }

    /// The human input's hydraulics for the frame (0x4078A0's events, then
    /// 0x407400): the keyboard's throttle / brake keys are its up / down
    /// axis, the steering keys left / right (live; the pads' sticks behind
    // prov: open game.hydraulics_controls
    /// the SELX / SELY events are **open**: their digital actions stand
    /// in). A mode on drops the throttle, brake and steering.
    pub(crate) fn hydraulics_controls(&mut self, mut controls: Controls, dt: f32) -> Controls {
        use nfsu2::pad::act;
        let held = self.held_actions();
        let on = |a: u8| held & (1u64 << a) != 0;
        let keys = nfsu2::hydraulics::Keys {
            bounce: on(act::HYD_BOUNCE),
            pressure: on(act::HYD_PRESSURE),
            ud: nfsu2::hydraulics::axis(on(act::THROTTLE), on(act::BRAKE)),
            lr: nfsu2::hydraulics::axis(on(act::STEER_LEFT), on(act::STEER_RIGHT)),
        };
        let level = self.hydraulics_level();
        let Some(d) = self.sim.drive.as_mut() else { return controls };
        let race = d.car.car_type.handling.race;
        let ctx = nfsu2::hydraulics::Context {
            barred: race.drift() || race.street_x(),
            mph: d.state.velocity.length() * 2.237,
            states: d.state.hydraulics.state,
        };
        controls.hydraulics = d.hydro.update(level, keys, &ctx, dt);
        if d.hydro.holds_driving() {
            controls.throttle = 0.0;
            controls.brake = 0.0;
            controls.steer = 0.0;
        }
        controls
    }
}
