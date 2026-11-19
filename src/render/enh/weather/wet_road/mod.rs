//! Wet Road Interaction (clone deviation, opt-in; docs/notes/graphics/wet-road-car.md):
//! the cars interact with the wet road. Nothing here is native: car.efx and
//! worldreflect have no input for it (docs/notes/graphics/car-look.md "Spots like raindrops on a dry car",
//! docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections"), so the original draws a wet road the same with
//! or without cars on it. What it follows is native: the rain's amount
//! (weather +0x1F0), the road's wetness from the rain (`Weather::road_wet`,
//! the +0x509C rule from 0 at load), the world clock 0x7FB718, the cars'
//! drawn poses, their light markers and brake flag (0x615640, car +0x944)
//! and their wheels' slip (the sim's values, read only).
//!
//! * **Trail map**: a world-space Rgba16Float map around the camera (a torus:
//!   texel = fract(world xy / size), no copies when the camera moves; the
//!   strips that wrap are cleared). Every drawn car stamps each wheel's path
//!   since the last frame: R = the water its tyre pushed away (the drier,
//!   darker track), G = the wake's freshness, B = the closeness to the
//!   track × G, A = the wake's height. A per-texel multiply decays them by
//!   the world clock: the track refills in ~1 s in heavy rain and lasts
//!   longer on a drying road; the wake travels outward for ~1.5 s.
//! * **Road** (world.wgsl's Wet Roads branch, `wet_road_world.wgsl`, the
//!   main view only): the track takes the reflection away, the wake ripples
//!   the puddles, the cars' head and tail lights are mirrored in the wet
//!   road (anisotropic, long streaks at grazing angles; brake lights
//!   brighter while braking) with a red glow beneath the tail, and without
//!   SSR the nearby cars block the puddles' mirror (two boxes each).
//! * **Spray** (`weather_fx::Spray`, [`emit`]): rooster tails scaled by the
//!   speed and the wheels' slip, splashes through puddles, wake mist behind
//!   fast cars, spray thrown off the wheel arches, water sheeting off the
//!   body's rear edge.
//!
//! With the row off nothing of this is made; the bindings 12–14 of the
//! world views' group 0 hold placeholders and the shader's branch is never
//! taken (frame.wet.x = 0 on a dry road and in every view but the main one).

pub mod lights;
pub mod spray_emit;
pub mod trail_map;

pub use crate::render::enh::materials::world::puddle::{level, puddle_at};
pub use lights::*;
pub use spray_emit::*;
pub use trail_map::*;
