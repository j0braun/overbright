//! The other cars' sounds: the AI racers' engines (CARSFX_AIEngine) and
//! the traffic cars' engines and whooshes (CARSFX_TrafficEngine,
//! CARSFX_TrafficWoosh), with the mix map's levels and azimuths
//! (sections 2 and 3) and the position objects' doppler.
//!
//! * **Doppler** (0x45E1F0 on each SFXCTL_3DCarPos update, 0x471DC0;
//!   the object's +0x6C = 1 from its ctor 0x45DD60, so the distance is
//!   field 0, the integer distance to the player's car): +0xA0 = that
//!   distance; with it and last frame's +0xA4 both ≥ 0 (both start at
//!   −1) the ratio is 343 / ((d − d′) / dt + 343) (0x7887FC), else 1; a
//!   4-sample running mean (0x5768B0(4), 0x576980) → +0xB4. A sound
//!   object reads it through 0x45E290(k): clamp((mean − 1) · T[k] + 1,
//!   0.8, 1.2) with T = 0x7F3760 {0.35, 0.33, 0.6, 0.3} (0.0 while
//!   paused).
//! * **AI engine** (vtable 0x78AEE0; ctor 0x473CE0: +0xC0 = (counter +
//!   100) % 600 − 300 per object, counter 0x82CB4C; attach 0x4610B0:
//!   SFXCTL_Engine → +0xB8, 3DCarPos → +0xB4; bank 0x465EA0: the engine
//!   record's +4, the SPU bank `CAR_xx_ENG_MB_SPU.abk`, under
//!   sound\Engine\). Start (slot 0x44, 0x461300): "CAR" (id record
//!   0x7EDE18, slot 0x82C538) with {clamp(record +8, 0, 60), 0 …} (0x6C
//!   bytes). Per frame (slot 0x48, 0x4610F0): p1 = clamp(trunc(rec +0x20
//!   + (rpm − 1000) / 9000 · (rec +0x1C − rec +0x20)), 0, 10000) with the
//!   Engine controller's rpm (its slot 0x34); p3 = clamp(trunc(Engine
//!   slot 0x38 · 10.24), 0, 0x400); p4 = p5 = clamp(((Engine +0xE0 ·
//!   level[0]) >> 15) − 0x7FFF, −0x7FFF, 0); p15 = the azimuth; p16 =
//!   clamp(trunc(doppler(2) · (+0xC0 + 16000) − 16000), ±0x3FFF); p17 =
//!   Engine +0x134; p21 = Engine +0x138 (0..25000).
//! * **Traffic engine** (vtable 0x78B3A8; attach 0x476680: the position
//!   object → +0xB4; TRAFFIC_MB.abk). Start (0x4621C0): FX_TRAFFIC
//!   (slot 0x82C668) with {variant, 0, 0, 0, 0}, the variant alternating
//!   0 / 1 per start (0x82CB38). Per frame (0x462240): +0x10 (field 3)
//!   = 32767 unless [0x82B884] +0x38 is set; for a car with +0x550 set:
//!   p1 = level[0], p2 = clamp(trunc(doppler(1) · 16383 − 16383),
//!   −0x3FFF, 0x3FFF), p3 = the azimuth, p4 = clamp(trunc(min(mph / 140
//!   · 1024, 1024)), 0, 0x3FF) (0 when not positive); else p1 = p3 = 0.
//! * **Traffic whoosh** (vtable 0x78B490; 0x475E10 / 0x47E8D0): with
//!   none running, when a human player's car is within 5 m (3D) of the
//!   traffic car (0x4755F0), the relative speed (0x462670: |the view
//!   car's velocity − the traffic car's| in x / y) is above 15 m/s
//!   (0x788870) and 3 s (0x787ADC) have passed since the last, the
//!   whoosh record of list 1: (counter % 6) + 9 − 6 · trunc(clamp(clamp((rel
//!   − 15) / 45, 0, 1), 0, 0.99) · 127 · −3/128) (0x78B070), counter
//!   0x82CB68; +0xC (field 2) := 1 for that frame. Per frame its level and
//!   azimuth are slot 2's; it is dropped when its layers end, or at once
//!   when the relative speed falls to 15 or below.
//!
// prov: inferred audio.traffic.module
//! **Inferred**: the AI Engine controller's rpm / throttle as the sound
//! context's (no shift transients for the AI: Engine +0xE0 = 0x7FFF,
//! +0x134 = 0, +0x138 = 24000 as for the player at rest); the AI engine
//! entry from the stock car (the AI's parts are not modelled); the pause
//! gates off; [0x82B884] +0x38 null; the dt of the doppler's +0x60 the
//! frame time; traffic cars always "+0x550" (active) and visible.
//! * **Traffic horn** (CARSFX_TrafficHorn, factory 0x47E6A0, ctor
//!   0x475820: vtable 0x78B428, +0xC6 = 1 — its only writer — +0xC0 =
//!   +0xC8 = 0, the global last-horn time 0x82BC50 = 0; update 0x47E6E0).
//!   With the context's +0x3C set: the fade (+0xD0, 0x46B100: time += dt,
//!   linear from +0xD8 to +0xDC over +0xD4, done +0xE8). With no horn
//!   (+0xB4): when the traffic car's +0x550 is set, the clock is past
//!   +0xC0 + 3 and +0xC8 + 3 (0x787ADC), +0xC6, the first human racer
//!   is above 15 m/s (0x475690, car +0x42C > 0x788870) and 0x475B30
//!   holds — the player's car (0x459A50 with one human: [0x8900B4] +4)
//!   within 20 m (0x78886C, 3D) of the traffic car, n = the traffic car's
//!   velocity (+0x70, z = 0) normalised (Vec3_Normalise 0x43CF10), d =
//!   the player − traffic offset (z = 0) normalised: n · d ≥ cos 2.2689
//!   (130°, 0x43C350 is a cosine) and the player's forward (+0x90)
//!   normalised · n ≤ cos 1.9199 (110°), oncoming; in a drag race
//!   (0x89E7D8 or 0x890108) n · d ≥ cos 3.5779 and no second test —
//!   then Rand_Float(1) < 0.3 (0x788884) or the clock past 0x82BC50 + 5
//!   (0x788878) starts it: variant 5 + Rand_Int(3), length 1.2 +
//!   Rand_Float(4 − 1.2) (0x788880 / 0x78887C) into +0xB8, +0xC5 = 1,
//!   0x4759C0: ENV_STATIC (record 0x82C510, ENV_COMMON_MB.abk) with
//!   {clamp(variant, 0, 50), 0, 0, 0, 0}, 0x82BC50 = +0xBC = the clock,
//!   the fade reset to 1 (0.001 s). Else +0xC8 = the clock. With a horn,
//!   once the clock passes +0xBC + +0xB8 (or +0xC5 is clear) and +0xC4
//!   is clear: a fade 1 → 0 over 0.15 s (0x462480); when it is done the
//!   horn stops (0x4624E0: +0xC0 = the clock). Per frame (slot 10,
//!   0x475920 → 0x462550): p2 = clamp(trunc(doppler(0) · 4096), 0,
//!   0x1FA0), p1 = clamp(trunc(slot 0x38(1) · fade), 0, 0x7FFF), p3 =
//!   clamp(slot 0x3C(1), 0, 0xFFFF) — class 2 of the mix map.
//!   The context's +0x3C is its attach (0x469B55; see Traffic contexts);
//!   the clock is the context's +0x34 (0x464C10 adds dt each frame).
// prov: inferred audio.traffic.module-2
//!   **Inferred**: +0xC4 clear (not traced). **Open**: TrafficHorn's
//!   field 1 for the mix map's section 3 (not fed).
//!
//! * **Car stereo** (CARSFX_TrafficStereo 0x30030, registry 0x7ED64C,
//!   factory 0x47F4E0, ctor 0x47F410, vtable 0x78BC80; the traffic
//!   context's mask 0x1F (0x476B30) makes it). The traffic manager
//!   (0x463E30) attaches a context to each of its cars within 70 m
//!   (0x78A414, Vec3_Distance to the nearest player's car 0x459A50) and
//!   releases it beyond; the attach (0x469B20) runs slot 11, 0x476580: a
//!   traffic car (car +0x14 → +4 = 3) whose type (car +0x10 → +0) is not
//!   TAXI / TAXI02 (0x1F / 0x22) gets +0xA0 = (Rand_Int(3) = 0), a taxi
//!   +0xA0 = 0; then +0xB8 = 8 + Rand_Int(2). Per frame slot 9 (0x4635B0):
//!   with +0xA0 and a car, a stereo playing (+0xB0) stops (0x45FA60(1))
//!   when slot 0x38(0) is 0; none playing starts ENV_STATIC (0x82C510)
//!   {clamp(+0xB8, 0, 50), 0, 0, 0, 0} when slot 0x38(0) is not 0 (the
//!   pool node 0x457E20, retried once after 0x735840). Slot 10 (0x4636E0):
//!   p1 = clamp(slot 0x38(0), 0, 0x7FFF), p3 = clamp(slot 0x3C(0), 0,
//!   0xFFFF), p2 = 0x1000 (no doppler), 0x735580. Release (slot 12,
//!   0x463540) frees the instance. ENV_COMMON_MB's ENV_STATIC plays sound
//!   7 for variant 8 and 8 for 9 (looped bass loops). The mix map binds
//!   it as class 3 slot 0 of section 3 (`mixmap::tests::stereo_slots`).
// prov: inferred audio.traffic.module-3
//!   **Inferred**: the clone's traffic car slot as the context's car, slot
//!   9 before slot 10 in a frame.
//! * **Racer stereo** (CARSFX_Stereo 0x200F0, the AI racers' mask 0x802D):
//!   the world's start (0x60A690) makes the contexts (0x486DB0 → each
//!   manager's slot 2) and, after the cars (0x5F4260), activates the
//!   managers (0x60A7B3 → 0x458850 → slot 5 0x463BB0: +0x1C = 1, each
//!   context's slot 4); an AI context with a car (0x469CD0, vtables
//!   0x78BF30 / 0x78BF88) runs its objects' slot 7 (0x464BB0) —
//!   0x4765F0: +0xB8 = 8 + Rand_Int(2), and for a racer (car +0x14 → +4 =
//!   2) 0x82CB6C = (+1) mod 6, +0xA0 = (it = 0). The counter is .bss (0 at
//!   boot) and never reset, so the 6th, 12th, … racer of the session plays
//!   one. Its slots 9 / 10 are TrafficStereo's, on the mix map's AI class
// prov: inferred audio.traffic.module-4
//!   15 slot 0 (section 2). **Inferred**: the roll as the clone's racer
//!   list grows (one world start per race), in the list's order.
//! * **Traffic contexts** (0x463E30, sound round 12). The traffic manager
//!   (vtable 0x78A3E0, audio +0xB4, made at 0x48578E with 0x463AD0(3, 6):
//!   class 3, cap +0x14 = 6) makes one context per traffic car of the car
//!   table at the world's start, at most 6 (0x476AF0 → 0x476990). Cars
//!   join its 16 slots as they are made (0x4690E0 → slot 4 0x463F10) and
//!   leave at the track's unload (0x469160 → slot 10 0x463EE0). Each
//!   frame, slot by slot, a car with a context releases it at 70 m or
//!   more (0x469B90); a car within 70 m without one takes the first
//!   context of the list whose +0x3C is clear (0x464350), if any: no
//!   priority, no stealing. Every object acts only while its context is
//!   attached: the engine starts FX_TRAFFIC at the attach (slot 11
//!   0x4621C0) and stops it at the release (slot 12 0x475790), its frame
//!   (0x462240), the whoosh (0x475E10), the horn (0x47E6E0) test context
//!   +0x3C, the bottom-out (0x47ED80) the car +0x30; the release frees the
//!   whoosh (0x462650), the horn (0x4624E0: +0xC0 = the context's clock),
//!   the knocks (0x462700) and the stereo (0x463540). The mix map's
// prov: inferred audio.traffic.module-5
//!   section 3 has one copy per context (0x485FA0). **Inferred**: a
//!   context lets go when the clone's pool car loses its lane (the native
//!   car keeps its slot), the doppler history starting again at an attach.

/// One AI racer this frame.
#[derive(Clone, Copy, Debug)]
pub struct AiInput {
    /// The engine entry (0x7F0DE0 index) and the sound context's rpm
    /// (1000..10000) and throttle (0..1).
    pub entry: usize,
    pub ctx_rpm: f32,
    pub throttle: f32,
    pub position: Vec3,
    pub mix: Levels,
}

/// One traffic car this frame.
#[derive(Clone, Copy, Debug)]
pub struct TrafficInput {
    pub position: Vec3,
    pub velocity: Vec3,
    pub mix: Levels,
    /// The wheels on the ground (car +0x390..+0x393) and the up vector's
    /// z, for CARSFX_TrafficBottomOut.
    pub down: [bool; 4],
    pub up_z: f32,
    /// The camera director's event kind nearest the car (0x459AC0 +0x58).
    pub event_kind: u8,
    /// The traffic car's identity (its pool slot): a new one is a new
    /// attach of the sound context (0x463E30).
    pub key: usize,
    /// Its car type is TAXI or TAXI02 (type index 0x1F / 0x22, car +0x10
    /// → +0): CARSFX_TrafficStereo never plays on them (0x476580).
    pub taxi: bool,
}

mod ai_engine;
mod doppler;
mod horn;
mod others;
mod stereo;
mod traffic_ctx;

use crate::audio::mixmap::Levels;
use glam::Vec3;

pub use ai_engine::ai_bank;
pub use doppler::*;
pub use horn::horn_aims;
pub use others::OtherCars;
#[cfg(test)]
use stereo::Stereo;
pub use traffic_ctx::{CONTEXT_RANGE, TRAFFIC_CONTEXTS, assign};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doppler_rises_on_approach() {
        let mut d = Doppler::default();
        assert_eq!(d.ratio(1), 1.0);
        // Closing at 30 m/s: 343 / (343 − 30).
        for k in 0..6 {
            d.update(100 - k, 1.0);
        }
        assert!((d.mean - 343.0 / 342.0).abs() < 1e-3, "{}", d.mean);
        let mut f = Doppler::default();
        for k in 0..6 {
            f.update(300 - 30 * k, 1.0);
        }
        assert!((f.ratio(2) - ((343.0 / 313.0 - 1.0) * 0.6 + 1.0)).abs() < 1e-4);
        assert_eq!(ai_bank(41), "SOUND/ENGINE/CAR_14_ENG_MB_SPU.abk");
    }

    #[test]
    fn contexts_go_to_the_first_cars_in_range() {
        // Ten cars, every one within 70 m: the six contexts go to the
        // first six by slot order, the rest wait (no stealing).
        let cars: Vec<(usize, f32)> = (0..10).map(|k| (k, 10.0 + k as f32)).collect();
        let (attach, release) = assign(&[None; 6], &cars);
        assert_eq!(attach, (0..6).map(|k| (k, k)).collect::<Vec<_>>());
        assert!(release.is_empty());
        // Car 2 drives off to 70 m: its context goes free and car 6 (the
        // first car in range without one) takes it the same frame.
        let held: Vec<Option<usize>> = (0..6).map(Some).collect();
        let mut cars = cars;
        cars[2].1 = 70.0;
        let (attach, release) = assign(&held, &cars);
        assert_eq!(release, vec![2]);
        assert_eq!(attach, vec![(2, 6)]);
        // A car gone from the pool lets its context go.
        let (attach, release) = assign(&[Some(3), None], &[(4, 80.0)]);
        assert_eq!((attach, release), (vec![], vec![0]));
    }

    #[test]
    fn every_sixth_racer_has_a_stereo() {
        let mut counter = 0;
        let on: Vec<bool> = (0..12)
            .map(|_| {
                let mut s = Stereo::default();
                s.racer_roll(&mut counter);
                assert!((8..=9).contains(&s.variant));
                s.on
            })
            .collect();
        assert_eq!(on.iter().filter(|&&x| x).count(), 2);
        assert!(on[5] && on[11]);
    }

    #[test]
    fn horns_aim_at_oncoming_players() {
        let (p, f) = (Vec3::ZERO, Vec3::X);
        // Oncoming, 10 m ahead, coming at the player: honks.
        assert!(horn_aims(Vec3::new(10.0, 2.0, 0.0), Vec3::new(-20.0, 0.0, 0.0), p, f, false));
        // Too far, or going the player's way, or already past: silent.
        assert!(!horn_aims(Vec3::new(25.0, 0.0, 0.0), Vec3::new(-20.0, 0.0, 0.0), p, f, false));
        assert!(!horn_aims(Vec3::new(10.0, 2.0, 0.0), Vec3::new(20.0, 0.0, 0.0), p, f, false));
        assert!(!horn_aims(Vec3::new(-10.0, 2.0, 0.0), Vec3::new(-20.0, 0.0, 0.0), p, f, false));
        // A drag race skips the heading test.
        assert!(horn_aims(Vec3::new(-10.0, 2.0, 0.0), Vec3::new(20.0, 0.0, 0.0), p, f, true));
    }
}
