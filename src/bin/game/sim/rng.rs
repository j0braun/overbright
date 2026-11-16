//! The game's random-number routines on the shared generator (0x7EA624,
//! `Game::rng`) and the world generator (world +0xF8) in the original's
//! sites and order (docs/rng.md). The draws whose values the
//! clone does not read are in `nfsu2::rng_sites`.

use crate::*;

impl Game {
    /// The start-up's draws after the boot seed (0x57ED20's chain): the two
    /// views' lens set-up (0x57ED7C → 0x5C2FA0, 120 draws; the clone keeps
    /// view 1's), the input binding's draw (0x57ED90 → 0x579560, 1), then
    /// the traffic manager list's first build (0x4FE9F0 → 0x41AAF0 →
    /// 0x412840 with no world: 15 × 2). **Measured** on the pinned original:
    /// 151 shared draws from the process start to the main menu with the
    /// sound off (c1/d1 `boot`; the one main-menu car's `Rand_Int(8)` at
    /// 0x6383A5 is not ported: open). Made again when the window reseeds
    /// the generator from the clock.
    pub(crate) fn boot_draws(&mut self) {
        self.sim.traffic_paints = nfsu2::rng_sites::traffic_paints(&self.game_data.parts, &self.game_data.car_types);
        self.sim.lens = nfsu2::weather::LensSetup::boot(&mut self.rng);
        nfsu2::rng_sites::input_bind(&mut self.rng);
        nfsu2::rng_sites::model_slots(&mut self.rng, None, &self.sim.traffic_paints);
    }

    /// A world build's draws (World_Start 0x60A690 → the world's
    /// constructor 0x5FF4A0; every free-roam entry, race load and restart),
    /// in the original's order: the world generator's seed (0x5FF557), the
    /// traffic manager's slot reset with the world (0x4179F0 → 0x412840:
    /// 15 × {paint, shared, world}), the rain set-up of both views
    /// (0x5FF6A5 → 0x61E100: 3260 shared draws). Returns the world
    /// generator. **Measured**: ≈ 3341 shared draws a build (c1/d1 `entry`,
    /// `setup`). Not ported here: one `Rand_Int(8)` per car render info
    /// built (0x6383A5, 24–37 a build; which cars and when is open).
    /// The callers then make the career's slot-event picks (free roam,
    /// 0x527D40) and Traffic_AssignModels (0x412C9A) on the shared
    /// generator, then the speech library's re-seed ([`Game::speech_seed`]).
    pub(crate) fn world_build(&mut self) -> nfsu2::opponents::GameRng {
        let mut world = nfsu2::rng_sites::world_seed(&mut self.rng);
        nfsu2::rng_sites::model_slots(&mut self.rng, Some(&mut world), &self.sim.traffic_paints);
        if let Some(d) = self.sim.drive.as_mut() {
            d.weather.rain_init(&mut self.rng);
        }
        // The harness's traffic knob varies the world's stream.
        world.0 ^= traffic_seed();
        world
    }

    /// The speech library's init (0x486220) on the game's draw
    /// `Rand_Int(0xFFFFFFFF)`: at the audio start (0x4866F0, made at the
    /// first audio frame: nothing draws between the start-up and the main
    /// menu) and at every world build (MixMap_Select 0x486A2C). Only with
    /// the sound on (no audio, no draw: **measured**, c1 sound off).
    /// Where 0x486A2C falls among a build's other draws is **open**; the
    /// clone makes it after them.
    pub(crate) fn speech_seed(&mut self) {
        if let Some(a) = self.audio.as_mut() {
            if !a.speech_seeded {
                a.speech_seeded = true;
                a.streams.init_speech_library(self.rng.below(0xFFFF_FFFF));
            }
            a.streams.init_speech_library(self.rng.below(0xFFFF_FFFF));
        }
    }

    /// The audio start's speech init (0x4866F0) once, at the first audio
    /// frame.
    pub(crate) fn speech_start(&mut self) {
        if let Some(a) = self.audio.as_mut()
            && !a.speech_seeded
        {
            a.speech_seeded = true;
            a.streams.init_speech_library(self.rng.below(0xFFFF_FFFF));
        }
    }
}
