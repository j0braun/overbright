//! The player car's drive-by whooshes: CARSFX_DriveBy (registry 0x7ED488,
//! id 0x100C0, factory 0x47E070 → ctor 0x474930, vtable 0x78B028) fed by
//! its SFXCTL_Tunnel (0x7ED318, id 0x10070, ctor 0x45D950, vtable
//! 0x78A064; attached at DriveBy +0xB0 by 0x4619F0).
//!
//! * **The zones**: the route data's triggers of kinds 5, 6, 7 (masks
//!   0x20 / 0x40 / 0x80) and 0x14 (mask 0x100000), found by 0x5D9C80
//!   (kinds ascending, record order within a kind, from after the last
//!   hit) with a height test: a hit counts when its +0x14 is 0 or not
//!   below the car's z (car +0x68), else the walk goes on.
//! * **The tunnel update** (0x483250 per frame, which first calls
//!   0x471080): the cooldowns +0x94 / +0xA4 run down by the frame time
//!   (not below 0). The point ahead = the car's position + its 2D
//!   heading (+0x90, normalised) × forward speed (+0x42C) × 0.3; its zone
//!   kind +0x74 is 5 / 6 / 7 by the first of masks 0x20, 0x40, 0x80 that
//!   holds it, else 2. Not inside a zone (+0x70 clear), with a zone ahead,
//!   when the cooldown is under 0.01 or the zone is not the last one
//!   (+0x98), and the ramp 0x7F0B40 (0 → 1 over speed +0x430 18 → 45
//!   m/s, 0x77C5B0) above 0.01: the entry flag +0x7C, cooldown 3 s, the
//!   point (+0x80, z = the car's + 10), the ramp's value +0x90. Inside
//!   (+0x70) a kind 5 zone (+0x78 = 5) with no zone ahead (+0x74 = 2),
//!   cooldown under 0.01 (or +0xA8 set, never), ramp above 0.01: the exit
//!   flag +0x9C, cooldown +0xA4 3 s, +0xA8 = 0, ramp +0xA0. Then the car's
//!   own zone at its position (masks 0x20, 0x40, 0x80, 0x100000 in turn,
//!   +0x78 = 5 / 6 / 7 / 0x14 the last kind tried): inside → +0x70 set
//!   (entering also picks a reverb, 0x45DB90: not ported), else clear.
//! * **The whooshes** (0x474A20): on the entry flag, the first free of two
//!   slots (+0xB4 / +0xB8; none free → none) starts whoosh stitch record
//!   3 · (15 − trunc(+0x90 · −0.0234375)) + n (n = a counter mod 3:
//!   0x82CB54 when the zone ahead is kind 5, else 0x82CB58), block slot 0;
//!   on the exit flag, two slots +0xE4 / +0xE8, record 3 · (18 −
//!   trunc(+0xA0 · −0.015625)) + n (0x82CB50), block slot 4. The stitch
//!   objects are the whoosh list's (0x45B1F0 on the manager's +0xD0 list 1,
//!   0x46CDD0). Per frame (0x47E0D0) each running one takes the object's
//!   level (slot 0x38) and azimuth (slot 0x3C) of its block slot — the
//!   entry whooshes' azimuth only for the newest one (+0x114) — and is
//!   released when its layers have ended.
//!
// prov: inferred audio.driveby.module
//! **Inferred**: the tunnel update runs before the DriveBy update in a
//! frame; the car's world z as the clone's position z.
//! The reverb zones (SFXObj_Reverb, `sound\fxedit\*.fx`) read this
//! tunnel's `inside` / `inside_kind` (`super::reverb::ReverbCtl::frame`).
//! **Not ported**: the pause's pitch word, the object's ducking field.

use super::tunnel::{CarIn, Tunnel};
use crate::audio::carfx::Sink;
use crate::audio::stitch::{StitchBanks, StitchVoice};
use std::path::Path;

struct Whoosh {
    voice: StitchVoice,
    azimuth: i32,
}

/// CARSFX_DriveBy: two entry and two exit whoosh slots.
pub struct DriveBy {
    pub tunnel: Tunnel,
    stitches: Option<StitchBanks>,
    entry: [Option<Whoosh>; 2],
    exit: [Option<Whoosh>; 2],
    /// Index of the newest entry whoosh (+0x114).
    newest: Option<usize>,
    /// 0x82CB54 / 0x82CB58 / 0x82CB50.
    counters: [i32; 3],
    /// Whooshes started (probe and selftest count).
    pub started: usize,
}

/// The two DriveBy record formulas (0x474A6E.., 0x474B9B..).
pub fn record(base: i32, ramp: f32, scale: f32, counter: &mut i32) -> usize {
    let n = *counter % 3;
    *counter = n + 1;
    (3 * (base - (ramp * scale) as i32) + n) as usize
}

impl DriveBy {
    pub fn new(root: &Path) -> DriveBy {
        DriveBy {
            tunnel: Tunnel::load(root),
            stitches: StitchBanks::load(root),
            entry: [None, None],
            exit: [None, None],
            newest: None,
            counters: [0; 3],
            started: 0,
        }
    }

    /// One frame: the tunnel, then the starts (0x474A20) and the running
    /// whooshes (0x47E0D0) with the object's levels (`level(slot)`,
    /// `azimuth(slot)` for block slots 0 and 4).
    pub fn update(&mut self, sink: &mut dyn Sink, car: &CarIn, level: impl Fn(usize) -> (i32, i32), dt: f32) {
        self.tunnel.update(car, dt);
        if self.tunnel.entry {
            if let Some(slot) = self.entry.iter().position(|w| w.is_none()) {
                let c = if self.tunnel.ahead == 5 { 1 } else { 0 };
                let r = record(15, self.tunnel.entry_ramp, -0.023_437_5, &mut self.counters[c]);
                if let Some(v) = self.stitches.as_mut().and_then(|s| StitchVoice::start(s, 1, r)) {
                    self.entry[slot] = Some(Whoosh { voice: v, azimuth: level(0).1 });
                    self.newest = Some(slot);
                    self.started += 1;
                }
            }
        }
        if self.tunnel.exit {
            if let Some(slot) = self.exit.iter().position(|w| w.is_none()) {
                let r = record(18, self.tunnel.exit_ramp, -0.015_625, &mut self.counters[2]);
                if let Some(v) = self.stitches.as_mut().and_then(|s| StitchVoice::start(s, 1, r)) {
                    self.exit[slot] = Some(Whoosh { voice: v, azimuth: level(4).1 });
                    self.started += 1;
                }
            }
        }
        let (l0, a0) = level(0);
        let (l4, a4) = level(4);
        for (k, w) in self.entry.iter_mut().enumerate() {
            if let Some(x) = w.as_mut() {
                if self.newest == Some(k) {
                    x.azimuth = a0;
                }
                if !x.voice.update(sink, l0, x.azimuth, dt) {
                    *w = None;
                }
            }
        }
        for w in self.exit.iter_mut() {
            if let Some(x) = w.as_mut() {
                x.azimuth = a4;
                if !x.voice.update(sink, l4, x.azimuth, dt) {
                    *w = None;
                }
            }
        }
    }

    /// Off the road: every whoosh stops.
    pub fn silence(&mut self, sink: &mut dyn Sink) {
        for w in self.entry.iter_mut().chain(self.exit.iter_mut()) {
            if let Some(mut x) = w.take() {
                x.voice.stop(sink);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::world::tunnel::speed_ramp;
    use glam::Vec3;

    #[test]
    fn records_and_ramp() {
        assert_eq!(speed_ramp(18.0), 0.0);
        assert_eq!(speed_ramp(45.0), 1.0);
        assert!((speed_ramp(31.5) - 0.5).abs() < 1e-6);
        let mut c = 0;
        let r: Vec<usize> = (0..4).map(|_| record(15, 1.0, -0.023_437_5, &mut c)).collect();
        assert_eq!(r, [45, 46, 47, 45]);
        let mut c = 0;
        assert_eq!(record(18, 0.5, -0.015_625, &mut c), 54);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_zone_ahead_starts_one_whoosh() {
        let root = crate::testkit::game_root();
        let t = Tunnel::load(&root);
        assert_eq!(
            t.zones.iter().map(|z| z.len()).collect::<Vec<_>>(),
            [39, 79, 42, 0],
            "zones of kinds 5, 6, 7, 0x14"
        );
        let z = t.zones[0][0];
        let mut tunnel = Tunnel::new(t.zones.clone());
        // 20 m short of the zone's centre, heading at it at 40 m/s: the
        // point ahead (12 m) is short; 10 m short it is inside.
        let centre = (z.min + z.max) * 0.5;
        let car = |d: f32| CarIn {
            position: Vec3::new(centre.x - d, centre.y, z.position.z - 1.0),
            forward: Vec3::X,
            forward_speed: 40.0,
            speed: 40.0,
        };
        let fires: Vec<bool> = [60.0, 5.0, 4.0, 3.0]
            .iter()
            .map(|&d| {
                tunnel.update(&car(d), 1.0 / 60.0);
                tunnel.entry
            })
            .collect();
        // Once while outside with the zone ahead; inside, none.
        assert_eq!(fires.iter().filter(|&&f| f).count(), 1, "{fires:?}");
        let mut s = StitchBanks::load(&root).expect("stitch banks");
        assert!(StitchVoice::start(&mut s, 1, 56).is_some(), "whoosh record 56 exists");
    }
}
