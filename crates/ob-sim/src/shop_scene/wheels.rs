//! NIS wheel controls: event parser/sequence 0x432D00 / 0x430EA0,
//! dispatch 0x4392E0, and NIS chassis post-step 0x5AB890.
//! Torque commands are retained for the future NIS body solver; the current
//! scene placement does not apply them to the driving engine or wheel speed.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub frame: f32,
    pub slot: usize,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Brake(f32),
    Additive(f32),
    /// 0x5A4090: chassis +0x258 / +0x25C. These drive body torque,
    /// not the engine/drivetrain, despite the event's name.
    Torque([f32; 2]),
    /// Event type 1 (CAR_TRIGGER_EFFECT, and SEQ_CAR_NITRO's two records
    /// at 0x7EA25C / 0x7EA278): a car effect kind (+0x0C; 10 CARFX_NITRO,
    /// 11 EXHAUST_SMOKE, 12 EXHAUST_BLOWOFF, 13 NOS_BLOWOFF by the name map
    /// 0x430C90), its intensity (+0x14) and parameter (+0x10). Parsed for
    /// the scene cars' flames; the dispatch (0x4392E0 type 1) is not traced
    /// here and nothing in this module consumes it.
    Effect { kind: u32, intensity: f32, param: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    pub brake: f32,
    pub additive: f32,
    pub torque: [f32; 2],
}

impl Default for Controls {
    fn default() -> Self { Self { brake: 1.0, additive: 0.0, torque: [0.0; 2] } }
}

impl Controls {
    /// 0x5AB890: speed/radius, driven-wheel additive in turns/s, then
    /// multiply the front wheels by brake lock. Intermediate drive splits
    /// only receive the additive while travelling forwards. |v| < .1 is 0.
    pub fn spin(self, speed: f32, radius: f32, rear_split: f32, wheel: usize) -> f32 {
        if radius <= 0.0 { return 0.0; }
        let speed = if speed.abs() < 0.1 { 0.0 } else { speed };
        let front_drive = rear_split < 0.2;
        let rear_drive = rear_split > 0.8;
        let driven = if front_drive { wheel < 2 } else if rear_drive { wheel >= 2 } else { speed > 0.0 };
        let spin = speed / radius + if driven { 360.0 * self.additive * 0.01745329238474369_f32 } else { 0.0 };
        if wheel < 2 { spin * self.brake } else { spin }
    }
}

pub fn parse(track: &str) -> Vec<Event> {
    let mut events = Vec::new();
    for line in track.lines() {
        let words: Vec<_> = line.split("//").next().unwrap_or_default().split_whitespace().collect();
        if words.len() < 5 || words[0] != "EVENT:" { continue; }
        let number = |s: &str| s.trim_end_matches('f').parse::<f32>().ok().filter(|v| v.is_finite());
        if words[2] == "CAR_TRIGGER_EFFECT" {
            // time, car, effect name, intensity, parameter.
            let kind = match words[4] {
                "CARFX_NITRO" => 10,
                "CARFX_EXHAUST_SMOKE" => 11,
                "CARFX_EXHAUST_BLOWOFF" => 12,
                "CARFX_NOS_BLOWOFF" => 13,
                _ => continue,
            };
            if let (Some(frame), Ok(slot), Some(intensity), Some(param)) = (number(words[1]), words[3].parse::<usize>(), words.get(5).and_then(|s| number(s)), words.get(6).and_then(|s| number(s)))
                && (1..8).contains(&slot)
            {
                events.push(Event { frame, slot, kind: Kind::Effect { kind, intensity, param } });
            }
            continue;
        }
        let (Some(frame), Ok(slot), Some(value)) = (number(words[1]), words[3].parse::<usize>(), number(words[4])) else { continue };
        // 0x432DD6 and 0x432F47 reject slots >= 8. Shipped tracks use 1..4.
        if !(1..8).contains(&slot) { continue; }
        let mut push = |frame, kind| events.push(Event { frame, slot, kind });
        match words[2] {
            "CAR_BRAKE_LOCK" => push(frame, Kind::Brake(value)),
            "CAR_BURNOUT_ADDITIVE" => push(frame, Kind::Additive(value)),
            "CAR_ENGINE_TORQUE" => {
                if let Some(second) = words.get(5).and_then(|s| number(s)) {
                    push(frame, Kind::Torque([value, second]));
                }
            }
            "SEQ_CAR_NITRO" => {
                // 0x430DC0, records at 0x7EA240 and 0x7EA294: only
                // +0x14 is multiplied by intensity; +0x18 stays 0.5.
                // The two intervening type-1 effect records are separate.
                push(frame, Kind::Torque([0.5 * value, 0.5]));
                // The nitrous flame on at +2 (2.0 x intensity), off at +5
                // (kind 10, parameter 3; read from the executable's table).
                push(frame + 2.0, Kind::Effect { kind: 10, intensity: 2.0 * value, param: 3.0 });
                push(frame + 5.0, Kind::Effect { kind: 10, intensity: 0.0, param: 3.0 });
                push(frame + 10.0, Kind::Torque([0.0; 2]));
            }
            "SEQ_CAR_BURNOUT" => {
                // 0x430EA0 copies ten start-relative records, then six
                // end-relative records. Preserve that order before sorting.
                push(frame, Kind::Additive(5.0));
                push(frame, Kind::Torque([0.1, -2.0]));
                push(frame + 3.0, Kind::Additive(10.0));
                for (offset, value) in [(3.0, -2.0), (8.0, 2.0), (12.0, -1.0),
                    (17.0, 1.0), (22.0, -0.5), (27.0, 0.5), (32.0, 1.0)] {
                    push(frame + offset, Kind::Torque([0.0, value]));
                }
                for (offset, value_at_end) in [(-20.0, 10.0), (-15.0, 8.0), (-10.0, 6.0)] {
                    push(frame + value + offset, Kind::Additive(value_at_end));
                }
                push(frame + value - 10.0, Kind::Torque([0.0; 2]));
                push(frame + value - 5.0, Kind::Additive(3.0));
                push(frame + value, Kind::Additive(0.0));
            }
            _ => {}
        }
    }
    events.sort_by(|a, b| a.frame.total_cmp(&b.frame));
    events
}

/// The latest intensity of car effect `kind` for `slot` at `frame` (0 before
/// any event): e.g. kind 10, the nitrous flame, on while above 0.
pub fn effect(events: &[Event], slot: usize, kind: u32, frame: f32) -> f32 {
    events.iter().take_while(|e| e.frame <= frame).filter(|e| e.slot == slot).fold(0.0, |v, e| match e.kind {
        Kind::Effect { kind: k, intensity, .. } if k == kind => intensity,
        _ => v,
    })
}

pub fn controls(events: &[Event], slot: usize, frame: f32) -> Controls {
    let mut result = Controls::default();
    for e in events.iter().take_while(|e| e.frame <= frame).filter(|e| e.slot == slot) {
        match e.kind {
            Kind::Brake(value) => result.brake = value,
            Kind::Additive(value) => result.additive = value,
            Kind::Torque(value) => result.torque = value,
            Kind::Effect { .. } => {}
        }
    }
    result
}

/// Return (end angular speed, integrated roll). Split at event boundaries
/// so refresh rate does not alter the scripted duration. `speed` is the
/// caller's mean forward speed over this pose interval; the existing pose
/// approximation remains distinct from the native wheel-speed calculation.
pub fn motion(events: &[Event], slot: usize, from: f32, to: f32, speed: f32, radius: f32, rear_split: f32, wheel: usize) -> (f32, f32) {
    let spin = controls(events, slot, to).spin(speed, radius, rear_split, wheel);
    if to <= from { return (spin, 0.0); }
    let mut cursor = from;
    let mut angle = 0.0;
    for e in events.iter().filter(|e| e.slot == slot && e.frame > from && e.frame < to && !matches!(e.kind, Kind::Torque(_) | Kind::Effect { .. })) {
        angle += controls(events, slot, cursor).spin(speed, radius, rear_split, wheel) * ((e.frame - cursor) / 30.0);
        cursor = e.frame;
    }
    angle += controls(events, slot, cursor).spin(speed, radius, rear_split, wheel) * ((to - cursor) / 30.0);
    (spin, angle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
    fn burnout_preserves_all_sixteen_native_event_records() {
        use crate::testkit;
        // Actual 0x1C-byte records from SPEED2.EXE VA 0x7EA2B0.
        // 0x430EA0 rebases the first ten on start, the last six on end.
        let raw = testkit::fixture("burnout-records", 1, |_| testkit::exe_bytes(0x7E_A2B0, 16 * 0x1C));
        let mut native = Vec::new();
        for (i, record) in raw.chunks_exact(28).enumerate() {
            let word = |offset| u32::from_le_bytes(record[offset..offset + 4].try_into().unwrap());
            let float = |offset| f32::from_bits(word(offset));
            let kind = match word(0) {
                5 => Kind::Additive(float(20)),
                6 => Kind::Torque([float(20), float(24)]),
                other => panic!("unexpected native record type {other}"),
            };
            native.push(Event { frame: 10.0 + if i < 10 { 0.0 } else { 152.0 } + float(4), slot: 1, kind });
        }
        native.sort_by(|a, b| a.frame.total_cmp(&b.frame));
        assert_eq!(native.len(), 16);
        assert_eq!(parse("EVENT: 10 SEQ_CAR_BURNOUT 1 152\n"), native);
    }

    #[test]
    fn torque_sequence_keeps_signed_body_controls_and_end_relative_reset() {
        let e = parse("EVENT: 10 SEQ_CAR_BURNOUT 1 152\n");
        for (frame, expected) in [(9.0, [0.0, 0.0]), (10.0, [0.1, -2.0]),
            (13.0, [0.0, -2.0]), (18.0, [0.0, 2.0]), (22.0, [0.0, -1.0]),
            (27.0, [0.0, 1.0]), (32.0, [0.0, -0.5]), (37.0, [0.0, 0.5]),
            (42.0, [0.0, 1.0]), (151.0, [0.0, 1.0]), (152.0, [0.0, 0.0])] {
            assert_eq!(controls(&e, 1, frame).torque, expected, "frame {frame}");
        }
        assert_eq!(controls(&e, 2, 18.0), Controls::default());
        // Shipped 30-frame sequences still contain the start+32 command:
        // do not replace native sorting with an invented duration clamp.
        let short = parse("EVENT: 0 SEQ_CAR_BURNOUT 4 30\n");
        assert_eq!(controls(&short, 4, 20.0).torque, [0.0, 0.0]);
        assert_eq!(controls(&short, 4, 32.0).torque, [0.0, 1.0]);
    }

    #[test]
    fn nitrous_flame_effects_from_sequences_and_trigger_events() {
        // IntroNis04's lines; 0x7EA25C / 0x7EA278 put SEQ_CAR_NITRO's flame
        // on at +2 (2.0 x intensity) and off at +5.
        let e = parse("EVENT: 0.0f    CAR_TRIGGER_EFFECT    3         CARFX_NITRO      0.0f                3\nEVENT: 8.0       SEQ_CAR_NITRO       3               1.0f\nEVENT: 90.0f    CAR_TRIGGER_EFFECT    3         CARFX_NOS_BLOWOFF      1.0f                3\n");
        assert_eq!(effect(&e, 3, 10, 9.0), 0.0);
        assert_eq!(effect(&e, 3, 10, 10.0), 2.0);
        assert_eq!(effect(&e, 3, 10, 12.9), 2.0);
        assert_eq!(effect(&e, 3, 10, 13.0), 0.0);
        assert_eq!(effect(&e, 2, 10, 10.0), 0.0);
        assert_eq!(effect(&e, 3, 13, 95.0), 1.0);
        // Effects are neither wheel controls nor rotation.
        assert_eq!(controls(&e, 3, 10.0).additive, 0.0);
        assert_eq!(motion(&e, 3, 0.0, 30.0, 0.0, 0.3, 1.0, 2).1, 0.0);
    }

    #[test]
    fn nitro_torque_temporarily_overrides_burnout_and_scales_only_first_axis() {
        let e = parse("EVENT: 10 SEQ_CAR_BURNOUT 1 152\nEVENT: 45 SEQ_CAR_NITRO 1 2\n");
        assert_eq!(controls(&e, 1, 44.0).torque, [0.0, 1.0]);
        assert_eq!(controls(&e, 1, 45.0).torque, [1.0, 0.5]);
        assert_eq!(controls(&e, 1, 55.0).torque, [0.0, 0.0]);
    }

    #[test]
    fn direct_torque_requires_both_parameters_and_does_not_drive_wheels() {
        let e = parse("EVENT: 4 CAR_ENGINE_TORQUE 1 -.5f 2f\nEVENT: 5 CAR_ENGINE_TORQUE 1 99\nEVENT: 7 CAR_ENGINE_TORQUE 1 0 0 // stop\n");
        assert_eq!(e.len(), 2);
        let c = controls(&e, 1, 6.0);
        assert_eq!(c.torque, [-0.5, 2.0]);
        assert_eq!(c.spin(0.0, 0.3, 1.0, 2), 0.0);
        assert_eq!(controls(&e, 1, 7.0), Controls::default());
        assert_eq!(controls(&e, 1, 3.0), Controls::default());
    }

    #[test]
    fn intro_burnout_sequence_uses_duration_for_the_fade_not_the_peak() {
        let events = parse("EVENT: 10.0 SEQ_CAR_BURNOUT 1 152.0f\n");
        assert_eq!(controls(&events, 1, 9.0).additive, 0.0);
        assert_eq!(controls(&events, 1, 10.0).additive, 5.0);
        assert_eq!(controls(&events, 1, 13.0).additive, 10.0);
        assert_eq!(controls(&events, 1, 147.0).additive, 8.0);
        assert_eq!(controls(&events, 1, 157.0).additive, 3.0);
        assert_eq!(controls(&events, 1, 162.0).additive, 0.0);
        assert_eq!(controls(&events, 2, 50.0).additive, 0.0);
    }

    #[test]
    fn native_driven_axles_and_stationary_awd_exception() {
        let c = Controls { brake: 0.2, additive: 2.5, ..Controls::default() };
        assert_eq!(c.spin(0.0, 0.3, 1.0, 0), 0.0);
        assert!((c.spin(0.0, 0.3, 1.0, 2) - 15.707963).abs() < 1e-5);
        assert!((c.spin(0.0, 0.3, 0.0, 0) - 3.1415927).abs() < 1e-5);
        assert_eq!(c.spin(0.0, 0.3, 0.0, 2), 0.0);
        for split in [0.2, 0.5, 0.8] {
            assert_eq!(c.spin(0.09, 0.3, split, 2), 0.0);
            assert!((c.spin(3.0, 0.3, split, 2) - 25.707963).abs() < 1e-5);
        }
    }

    #[test]
    fn rotation_across_event_boundaries_is_refresh_independent() {
        let events = parse("EVENT: 10 SEQ_CAR_BURNOUT 1 152\n");
        let total = |hz: usize| {
            (0..hz * 6).map(|i| motion(&events, 1, i as f32 * 30.0 / hz as f32,
                (i + 1) as f32 * 30.0 / hz as f32, 0.0, 0.3, 1.0, 2).1).sum::<f32>()
        };
        let reference = motion(&events, 1, 0.0, 180.0, 0.0, 0.3, 1.0, 2).1;
        for hz in [30, 60, 144, 240] { assert!((total(hz) - reference).abs() < 0.003); }
    }

    #[test]
    fn direct_controls_and_seek_do_not_replay_or_leak_other_slots() {
        let events = parse("// EVENT: 0 CAR_BURNOUT_ADDITIVE 1 99\nEVENT: 0f CAR_BRAKE_LOCK 1 .2f\nEVENT: 0 CAR_BURNOUT_ADDITIVE 1 2.5\nEVENT: 185 CAR_BURNOUT_ADDITIVE 1 0\nEVENT: 185 CAR_BRAKE_LOCK 1 1\n");
        assert_eq!(controls(&events, 1, 184.0), Controls { brake: 0.2, additive: 2.5, ..Controls::default() });
        assert_eq!(controls(&events, 1, 185.0), Controls::default());
        assert_eq!(controls(&events, 2, 5.0), Controls::default());
        assert_eq!(motion(&events, 1, 200.0, 200.0, 0.0, 0.3, 1.0, 2), (0.0, 0.0));
    }
}
