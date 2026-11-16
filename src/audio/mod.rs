//! The game's own audio: its sound banks and engine recordings decoded as
//! SPEED2.EXE does, played through a small software mixer.
//!
//! * [`abk`] — the "ABKC" banks (event modules + EA-XA samples).
//! * [`aems`] — the banks' event graphs, run as the audio library runs them.
//! * [`gin`] — the granular engine recordings ("Gnsu20") and their player.
//! * [`fe`] — which front-end sound a menu event plays.
//! * [`collision`] — the impact sound a contact record plays.
//! * [`carfx`] — the nitrous, turbo and gear-shift sounds of the player's car.
//! * [`engine`] — which recording a car's engine uses.
//! * [`hybrid`] — the engine volume chain (HybridMotor) and its two voices.
//! * [`skids`] — the skid controllers feeding the FX_SKID graphs.
//! * [`transients`] — the shift blip and launch / tip-in machines.
//! * [`mixmap`] — the mix map (.mxb): every sound object's levels and azimuths.
//! * [`mixer`] — the output stream (cpal), headless-safe.
//! * [`stitch`] — stitch records played by a sound object (thumps, knocks, whooshes).
//! * [`roadwind`] — the player car's road noise, wind and bottom-out sounds.
//! * [`scrape`] — the collision objects' FX_Scrape (level from the mix map, section 5 slot 9).
//! * [`traffic`] — the AI racers' engines and the traffic cars' engines and whooshes.
//! * [`enviro`] — the planes, trains and fountains.
//! * [`driveby`] — the player car's drive-by whooshes at tunnel / zone triggers.
//! * [`reverb`] — the FX graphs (`SOUND\FXEDIT\*.fx`) and the reverb zones' levels.

pub mod banks;
pub mod codec;
pub mod aems;
pub mod car;
pub mod engine;
pub mod fe;
pub mod fe_rules;
pub mod gin;
pub mod mixer;
pub mod mixmap;
pub mod others;
pub mod reverb;
pub mod music {
    pub use super::trax::*;
}
pub mod speech;
pub mod streams;
pub mod trax;
pub mod abk {
    pub use super::banks::abk::*;
}
pub mod ealayer3 {
    pub use super::codec::ealayer3::*;
}
pub mod microtalk {
    pub use super::codec::utk::*;
}
pub mod mp3_tables {
    pub use super::codec::mp3_tables::*;
}
pub mod schl {
    pub use super::codec::schl::*;
}
pub mod carfx {
    pub use super::banks::bank_events::*;
    pub use super::banks::cache::*;
    pub use super::banks::sink::*;
    pub use super::car::nitrous::*;
    pub use super::car::player::*;
    pub use super::car::tables::*;
    pub use super::car::turbo::*;
}

// Moved modules: the old paths stay as aliases.
pub mod engine_chain;
pub mod util;
pub mod world;
pub mod hybrid {
    pub use super::engine_chain::*;
}
pub mod traffic {
    pub use super::others::*;
}
pub mod transients {
    pub use super::car::transients::*;
}
pub mod hydraulics {
    pub use super::car::hydraulics::*;
}
pub mod scrape {
    pub use super::car::scrape::*;
}
pub mod scripted {
    pub use super::car::scripted::*;
}
pub mod skids {
    pub use super::car::skids::*;
}
pub mod stitch {
    pub use super::banks::cache::StitchBanks;
    pub use super::banks::stitch::*;
}
pub mod roadwind {
    pub use super::world::env::*;
}

// Moved modules: the old paths stay as aliases.
pub mod collision {
    pub use super::world::collision::*;
}
pub mod driveby {
    pub use super::world::driveby::*;
    pub use super::world::tunnel::*;
}
pub mod enviro {
    pub use super::world::enviro::*;
}
