//! NIS car effects: the event track's CAR_TRIGGER_EFFECT lines and the
//! effect records SEQ_CAR_NITRO expands to (parser 0x432D00, dispatch
//! 0x4392E0, per-update firing 0x43966D..0x439772; **verified**
//! 2026-11-04, REA / disassembly, the maintainer's private notes).
//!
//! * A CAR_TRIGGER_EFFECT line (event type 1, 0x430B20) is `car kind
//!   intensity parameter`: record +8 the car (1..7; 8 or more → −1),
//!   +0xC the kind through the name map 0x430C90, +0x14 the intensity
//!   (float), +0x10 the parameter (int).
//! * SEQ_CAR_NITRO `car intensity` at frame f (0x430DC0) copies four
//!   0x1C-byte templates (0x7EA240, 0x7EA25C, 0x7EA278, 0x7EA294), each
//!   at f + its time with +0x14 × the intensity: a body torque at f (6:
//!   0.5, 0.5 — `wheels`), **CARFX_NITRO on at f + 2 with intensity 2**,
//!   **CARFX_NITRO off at f + 5** (intensity 0), the torque off at f + 10.
//! * The dispatch keeps, per scene car slot, the last record of each of
//!   the kinds 10 CARFX_NITRO, 11 CARFX_EXHAUST_SMOKE, 12
//!   CARFX_EXHAUST_BLOWOFF and 13 CARFX_NOS_BLOWOFF (0x827D38 / 0x827DB8 /
//!   0x827E28 / 0x827CD0 + 12 · slot: kind, intensity, parameter); every
//!   scene update, for each slot bound to a car, a kind whose intensity is
//!   above 0 fires 0x61FB10(kind, intensity, parameter, 0) on that car —
//!   the same car-effect call as the driving nitrous flame (0x5F5F5A), so
//!   the flame bursts with −intensity at the exhaust markers.

/// One effect record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    /// Scene frame (the event track's time).
    pub frame: f32,
    /// The scene car (1..7).
    pub slot: usize,
    /// The car effect kind (0x430C90): 10..=13 are kept by the dispatch.
    pub kind: u8,
    pub intensity: f32,
}

/// CARFX_NITRO's kind.
pub const NITRO: u8 = 10;

/// The kinds the dispatch keeps, by name (0x430C90's 10..13).
fn kind(name: &str) -> Option<u8> {
    Some(match name {
        "CARFX_NITRO" => NITRO,
        "CARFX_EXHAUST_SMOKE" => 11,
        "CARFX_EXHAUST_BLOWOFF" => 12,
        "CARFX_NOS_BLOWOFF" => 13,
        _ => return None,
    })
}

pub fn parse(track: &str) -> Vec<Event> {
    let mut events = Vec::new();
    for line in track.lines() {
        let w: Vec<_> = line.split("//").next().unwrap_or_default().split_whitespace().collect();
        if w.len() < 5 || w[0] != "EVENT:" {
            continue;
        }
        let number = |s: &str| s.trim_end_matches('f').parse::<f32>().ok().filter(|v| v.is_finite());
        let (Some(frame), Ok(slot)) = (number(w[1]), w[3].parse::<usize>()) else { continue };
        if !(1..8).contains(&slot) {
            continue;
        }
        match w[2] {
            "CAR_TRIGGER_EFFECT" => {
                if let (Some(k), Some(intensity)) = (kind(w[4]), w.get(5).and_then(|s| number(s))) {
                    events.push(Event { frame, slot, kind: k, intensity });
                }
            }
            "SEQ_CAR_NITRO" => {
                if let Some(v) = number(w[4]) {
                    events.push(Event { frame: frame + 2.0, slot, kind: NITRO, intensity: 2.0 * v });
                    events.push(Event { frame: frame + 5.0, slot, kind: NITRO, intensity: 0.0 * v });
                }
            }
            _ => {}
        }
    }
    events.sort_by(|a, b| a.frame.total_cmp(&b.frame));
    events
}

/// The slot's kept intensity of `kind` at `frame` (0 before any record).
pub fn intensity(events: &[Event], slot: usize, kind: u8, frame: f32) -> f32 {
    events.iter().take_while(|e| e.frame <= frame).filter(|e| e.slot == slot && e.kind == kind).last().map_or(0.0, |e| e.intensity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_car_nitro_lights_the_flame_for_three_frames() {
        let e = parse("EVENT: 0.0f    CAR_TRIGGER_EFFECT    2         CARFX_NITRO      0.0f                3\nEVENT: 4.0       SEQ_CAR_NITRO       4               1.0f\nEVENT: 150.0f    CAR_TRIGGER_EFFECT    4       CARFX_NOS_BLOWOFF\t   1.0f\t    3\n");
        assert_eq!(e.len(), 4);
        assert_eq!(intensity(&e, 4, NITRO, 5.9), 0.0);
        assert_eq!(intensity(&e, 4, NITRO, 6.0), 2.0);
        assert_eq!(intensity(&e, 4, NITRO, 8.9), 2.0);
        assert_eq!(intensity(&e, 4, NITRO, 9.0), 0.0);
        assert_eq!(intensity(&e, 2, NITRO, 100.0), 0.0);
        assert_eq!(intensity(&e, 4, 13, 151.0), 1.0);
    }
}
