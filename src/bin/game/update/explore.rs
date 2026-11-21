use super::{Flow, TickControls};
use crate::sim::session::EncounterTick;
use crate::*;

impl Game {
    /// Free roam's update (no shop scene running): the encounters (their
    /// frame ran in the session: `tick`), the outrun's career
    /// consequences, the car (the session's second phase), the career's
    /// clock, Rachel, the SMS, the world challenge and the markers.
    /// `Flow::Stop` where the update ended early. `world_dt` is the
    /// world's step under an event camera's slow motion (0x5EA370; the
    /// session's `TickEvents::roam_dt`).
    pub(super) fn update_explore(&mut self, dt: f32, world_dt: f32, input: &TickControls, tick: EncounterTick) -> Flow {
        let TickControls { controls } = *input;
        let result_bank = self.explore_encounters(tick);
        self.sim_roam(world_dt, controls, result_bank);
        // 0x551330: a career's free roam (race type 0x89E7B0 = 0, so
        // not during an outrun) adds the frame time to the driving
        // time and sends the timed messages (0x529760); an urgent
        // one not yet shown opens the SMS screens (0x529710 → state
        // 0xD) unless a shop is being visited, and marks them shown
        // (0x514C70).
        let outrun = self.sim.encounters.as_ref().is_some_and(|e| e.outrun.is_some());
        // The D-Day intro (0x551330): in a career's free roam the
        // game clock runs; flow event 1 starts the script while the
        // player has no car of their own (0x513BC0); while it runs
        // it steps (0x5342A0) in place of the driving time.
        let career_roam = !outrun && !self.race_flow.quick_free_run && self.profile.is_some() && self.scenes.shop.is_none();
        if career_roam {
            self.explore_career_clock(dt);
        }
        if self.explore_speech(dt, career_roam) == Flow::Stop {
            return Flow::Stop;
        }
        if self.explore_sms(dt, outrun, career_roam) == Flow::Stop {
            return Flow::Stop;
        }
        self.explore_markers()
    }

    /// The career's side of the race status's free-roam frame (the
    /// frame itself is the session's, `Session::encounters_frame`): the
    /// profile's outrun timer, an outrun's start and end (the inbox,
    /// Rachel's lines and their draws on the shared generator, the bank), the outrun
    /// variety. Returns a won outrun's bank for the race-over message
    /// (the session's second phase writes it).
    fn explore_encounters(&mut self, tick: EncounterTick) -> Option<i32> {
        if let Some(timer) = tick.timer
            && let Some(p) = self.profile.as_mut()
        {
            p.outrun_timer = timer;
        }
        // The outrun's start (0x607190 → 0x532E40 → 0x529B60): the
        // condition-4 messages (SMS_OUTRUN_INTRO) arrive once.
        if tick.started
            && let Some(p) = self.profile.as_mut()
        {
            for m in self.game_data.career.messages.iter().filter(|m| m.condition == 4) {
                p.inbox.add(m.hash);
            }
        }
        // 0x532E40 → 0x511000: Rachel's outrun line (Speech_Stop
        // 0x45B490 first).
        if tick.started
            && let Some(p) = self.profile.as_mut()
        {
            let rng = &mut self.rng;
            let line = p.hints.outrun_start(&mut |n| rng.below(n));
            self.rachel_outrun(line);
        }
        // 0x5F84A0 → 0x541330: the career's bank and outrun wins;
        // a win's race-over message shows the stage's bank.
        let mut result_bank = None;
        if let Some(won) = tick.ended
            && let Some(p) = self.profile.as_mut()
        {
            let bank = p.outrun_result(&self.game_data.career, won, &mut self.rng);
            // 0x541330 → 0x5110C0(won): Rachel's line on the result
            // (its 0x533320 / 0x4F92A0 marker and HUD rebuilds: see
            // formats.md, Outrun).
            let rng = &mut self.rng;
            let line = p.hints.outrun_end(won, &mut |n| rng.below(n));
            let _ = p.commit();
            self.rachel_outrun(line);
            // Written into the encounters' result by the session's
            // second phase (`Session::step_roam`, before the car; the
            // outrun variety below reads no result).
            if won {
                result_bank = Some(i32::from(bank));
            }
        }
        self.outrun_variety_frame();
        result_bank
    }

    /// A career's free roam: the game clock, the D-Day start, the GPS and
    /// the neighbourhood zone.
    fn explore_career_clock(&mut self, dt: f32) {
        self.sim.clock_ms += dt * 1000.0;
        if self.profile.as_ref().is_some_and(|p| p.car.is_empty()) {
            self.career_world.dday.begin();
        }
        if let (Some(g), Some(d), Some(t)) = (self.career_world.gps.as_mut(), self.sim.drive.as_ref(), self.sim.traffic.as_ref()) {
            let f = d.state.forward();
            // The follower's searches start from car +0x60, the
            // centre of mass, and +0x90 (0x4188C0 → 0x40C440).
            let centre = d.state.position + d.state.orientation * Vec3::from(d.car.car_type.handling.centre_of_mass);
            g.update(&t.routes, centre, Vec2::new(f.x, f.y));
            // 0x5F3CC0 (the player's update): the GPS aid's
            // look-ahead, car +0x430 the speed, with the chase camera's
            // mover only; any other (the crash / jump camera) clears the
            // kept point (+0x114 = 0).
            if native_mover_type(&self.mode, Some(d)) == 1 {
                g.update_aid(&t.routes, d.state.position, d.state.velocity.length());
            } else {
                g.aid = None;
            }
        }
        // 0x5EB050's zone (player +0x9B0 / +0x9B4 / +0x9B8):
        // the kind-0x11 trigger holding the car, looked up while
        // there is none or every 8th frame (0x7FB724 & 7: the world
        // counter, 0 at world create); on a change
        // to a trigger, +0x9B8 = its `NEIGHBOURHOOD_` hash (0
        // when unnamed); leaving every zone keeps the last.
        if let Some(d) = self.sim.drive.as_ref() {
            let z = &mut self.sim.zone;
            if z.0.is_none() || self.pacing.clocks.world_counter % 8 == 0 {
                z.0 = self.sim.route_triggers.zone_at(d.state.position.truncate());
            }
            if z.0 != z.1 {
                if let Some(i) = z.0 {
                    z.2 = career::neighbourhood_hash(self.sim.route_triggers.zone_name(i));
                }
                z.1 = z.0;
            }
        }
    }

    /// The D-Day script's step or Rachel's hints, and the neighbourhood
    /// line.
    fn explore_speech(&mut self, dt: f32, career_roam: bool) -> Flow {
        let free = self.speech_free();
        // NFSU2_AUDIO_DEBUG: the intro script's state every 2 s.
        if debug_env().audio_debug && career_roam {
            static LAST: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);
            let mut last = LAST.lock().unwrap();
            if last.is_none_or(|t| t.elapsed().as_secs_f32() >= 2.0) {
                *last = Some(Instant::now());
                let speed = self.sim.drive.as_ref().map_or(0.0, |d| d.state.speed());
                eprintln!(
                    "intro: dday state {} running {} holds input {} speech free {free} gps {} speed {speed:.1} m/s clock {:.1} s",
                    self.career_world.dday.state,
                    self.career_world.dday.running(),
                    self.career_world.dday.holds_input(),
                    self.career_world.gps.is_some(),
                    self.sim.clock_ms / 1000.0
                );
            }
        }
        if career_roam && self.career_world.dday.running() {
            let (step, line) = self.career_world.dday.step(self.sim.clock_ms, free);
            self.say(line);
            match step {
                dday::Step::None => {}
                dday::Step::Prompt => self.career_world.dday_prompt = Some(self.sim.clock_ms),
                dday::Step::Instruction => {
                    if let Some(p) = self.profile.as_mut() {
                        p.inbox.add(dday::SMS_INSTRUCTION);
                    }
                    self.sms_popup_sound();
                    if let Some(m) = self.sms_open() {
                        self.mode = m;
                        return Flow::Stop;
                    }
                }
                dday::Step::GpsToA => self.gps_to_intro(career::DDAY_EVENT_A),
                dday::Step::Done => {
                    if let Some(p) = self.profile.as_mut() {
                        p.inbox.add(dday::SMS_INSTRUCTION);
                    }
                    self.gps_to_intro(career::DDAY_EVENT_A);
                }
            }
        } else if career_roam
            && let Some(p) = self.profile.as_mut()
            && let Some(fired) = p.hints.tick(p.drive_time + dt, dt, free)
        {
            // 0x551080 → 0x54BD40 → 0x53E0C0: Rachel's hints.
            let dday::Fired { fire, line } = fired;
            if debug_env().audio_debug {
                eprintln!("hints: fired {fire:?} line {line:X?} at drive {:.1} s", p.drive_time);
            }
            let p = self.profile.as_mut().unwrap();
            match fire {
                None => {}
                Some(dday::Fire::Message(m)) => {
                    p.inbox.add(m);
                }
                Some(dday::Fire::Reveal) => {
                    p.intro_races = true;
                    p.refresh_open(&self.game_data.career, &mut self.rng);
                }
                Some(dday::Fire::Complain { close }) => {
                    if close {
                        p.intro_closed = true;
                        p.refresh_open(&self.game_data.career, &mut self.rng);
                    }
                }
            }
            let _ = p.commit();
            self.refresh_event_triggers();
            self.say(line);
        }
        // 0x551080 → 0x525660 in career stage ≥ 2 (0x55117F), on a
        // driving step after the count: the neighbourhood line.
        // Rachel's freedom is read after this frame's fired line
        // prov: inferred game.update_world-2
        // started (**inferred**: the game's line starts in 0x53E0C0
        // before 0x5112B0 asks); the clone does it after the kind-0xF
        // post, which shares nothing with it.
        if career_roam
            && !self.career_world.dday.running()
            && self.profile.as_ref().is_some_and(|p| p.stage >= 2 && p.drive_time + dt > 0.0)
        {
            let (free, speech, zone) = (self.speech_free(), self.audio.is_some(), self.sim.zone.2);
            let p = self.profile.as_mut().unwrap();
            if p.hints.zone_changed(zone, free) {
                let line = p.neighbourhood_line(&self.game_data.career, &self.sim.route_triggers, speech, &mut self.rng);
                self.say(line);
            }
        }
        Flow::Go
    }

    /// The timed SMS and the world challenge's clock.
    fn explore_sms(&mut self, dt: f32, outrun: bool, career_roam: bool) -> Flow {
        if !outrun
            && !self.race_flow.quick_free_run
            && !self.career_world.dday.running()
            && let Some(p) = self.profile.as_mut()
            && p.sms_drive(&self.game_data.career, dt)
            && self.shop.visit.is_none()
        {
            p.inbox.mark_shown(&self.game_data.career.messages);
            self.sms_popup_sound();
            if let Some(m) = self.sms_open() {
                self.mode = m;
                return Flow::Stop;
            }
        }
        // 0x551330 → 0x5016A0 (with career +0x713C 0 or 9, **not
        // traced**): the world challenge's clock.
        if career_roam {
            self.tick_challenge(dt);
            if matches!(self.mode, Mode::Menu(_)) {
                return Flow::Stop;
            }
        }
        Flow::Go
    }

    /// The event triggers, the engage tips, the cash pickups and the
    /// markers.
    fn explore_markers(&mut self) -> Flow {
        self.refresh_event_triggers();
        // During an outrun (race state 7) 0x500B60 fails: no marker
        // is entered or engaged (0x5EB050, 0x602BE0) and the marker
        // effects are off (0x605790).
        if self.sim.encounters.as_ref().is_some_and(|e| e.outrun.is_some()) {
            if let Some(d) = self.sim.drive.as_mut() {
                d.effects.set_markers(&d.world.emitters, [None; 12], |_| None);
            }
            return Flow::Stop;
        }
        // The markers, in a career's free roam only (0x5EB050
        // behind 0x500B60: game mode 1, not racing; so not Quick
        // Race's Free Run): the first kind 13–15 route trigger
        // holding the car that stands for a marker (0x527810).
        // Another one (or none) leaves the last (0x5EAF40: its
        // marker goes, 0x513050) and enters the new (0x5EAFC0).
        // Nothing opens by itself: the Engage key does (`engage`).
        let at = self
            .sim.drive
            .as_ref()
            .filter(|_| !self.race_flow.quick_free_run && self.profile.is_some())
            .map(|d| d.state.position.truncate());
        // 0x5EB0F8 (the same player update, every frame, with no
        // world challenge running, 0x501690): the first engage-tip
        // trigger (kind 0x12) holding the car sends its message
        // and pays its +0x0C (0x532D50 → 0x529C50).
        if let Some(tip) =
            at.filter(|_| self.career_world.challenge.is_none()).and_then(|at| self.sim.route_triggers.tip_at(at)).map(|t| t.name)
            && let Some(p) = self.profile.as_mut()
            && p.sms_tip(&self.game_data.career, tip)
        {
            let _ = p.commit();
        }
        // 0x5EB12F (the same update, same gates): the first
        // cash-pickup trigger (kind 0x13) holding the car
        // (0x5283D0): its 0x34A1E record, if open (0x513890), pays
        // its cash and sets its flag (0x528380), the HUD's bank
        // element shows it (0x4AC1B0: +0x28) and FE sound 0x11
        // plays (0x458520: FE_COMMON's sample 17).
        if let Some(bank) =
            at.filter(|_| self.career_world.challenge.is_none()).and_then(|at| self.sim.route_triggers.bank_at(at)).map(|t| t.name)
            && let Some(p) = self.profile.as_mut()
            && let Some(cash) = p.collect_bank(&self.game_data.career, bank)
        {
            let _ = p.commit();
            self.hud_state.hud.bank_reward(cash as u32);
            self.fe_sample(0x11);
        }
        let found = at.and_then(|at| {
            self.sim.route_triggers
                .iter()
                .enumerate()
                .filter(|(_, t)| t.contains(at))
                .find_map(|(i, t)| self.marker_target(t.name).map(|m| (i, m)))
        });
        if found.map(|f| f.0) != self.career_world.marker_trigger {
            self.career_world.marker = None;
            self.career_world.marker_trigger = found.map(|f| f.0);
            if let Some((_, m)) = found {
                self.enter_marker(m);
            }
        }
        self.marker_effects();
        Flow::Go
    }
}
