use super::{Session, TickEvents, TickInput, race_drive};
use crate::{Controls, step_race, traffic_body, traffic_coast};

impl Session<'_> {
    /// The race's traffic after the start scene: unpaused, coasting to a
    /// stop through the countdown.
    pub(super) fn race_traffic(&mut self, dt: f32) {
        let (race, d) = race_drive(&mut self.race, &mut *self.drive);
        if let Some(t) = race.racers.traffic.as_mut() {
            t.paused = false;
            if race.timing.clock < 0.0 {
                let racers: Vec<nfsu2::ai::traffic::Other> =
                    race.racers.rivals.iter().map(|r| traffic_body(&r.car, &r.state)).collect();
                traffic_coast(
                    &d.world,
                    t,
                    &d.state,
                    nfsu2::ai::traffic::Other::half_box(&d.car.hull),
                    &racers,
                    1.0,
                    dt,
                );
            }
        }
    }

    /// The countdown (race clock below 0).
    pub(super) fn update_countdown(&mut self, dt: f32, input: &TickInput, events: &mut TickEvents) {
        let controls = input.controls;
        let (race, d) = race_drive(&mut self.race, &mut *self.drive);
        // The countdown: the player's car runs under the race
        // start's hold (neutral, full brake), so the engine
        // revs on the line (observed in a drag race under Wine:
        // gear 1, up to the limiter, 0 mph). In a drag race the
        // steering keys are lane events, not steering.
        // The rivals' drivers run too (the AI driver's
        // countdown pedals, 0x41D8D0: gas 1, brake 1, neutral),
        // so their engines rev on the line (**observed** in a
        // Quick Race drag under Wine: race status 3, each AI
        // racer at gear 1 with its rpm climbing to the limiter,
        // ≈ 6950, at 0 mph); the traffic stands braked
        // (`traffic_coast`; observed: its cars start from
        // about 0 m/s at the green).
        let held = Controls { steer: if race.drag { 0.0 } else { controls.steer }, ..controls };
        if race.racers.ai_routes.is_some() && input.ai.is_some() && !race.racers.rivals.is_empty() {
            // Their engine heat stays off (−1) on the line as
            // the player's does (`CarState::held`).
            for r in &mut race.racers.rivals {
                r.state.held = true;
            }
            step_race(d, race, held, input.ai, dt, &mut events.contacts);
            d.tick_clock(dt, &mut self.rng);
        } else {
            let sample = d.state.begin_world_update(dt);
            d.posed.extend(d.state.step_with_props(&d.car, &d.world, &mut d.props, held, dt, &mut events.contacts));
            d.state.finish_world_update(sample);
            d.impact_trigger();
            d.hold(dt, &mut self.rng);
        }
        race.update(d.state.position.truncate(), dt);
    }
}
