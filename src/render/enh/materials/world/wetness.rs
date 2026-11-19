//! One wetness (docs/notes/graphics/reflections.md "One wetness"): what the rain has done to
//! the world this frame, the single source every wet effect reads. Nothing here is native: SPEED2.EXE
//! draws the road the same rain or not (docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections"); the inputs are native (the rain's amount,
//! weather +0x1F0) or follow a native rule (`Weather::road_wet`, +0x509C's rule from 0 at load).
//!
//! Before, Wet Roads, Wet Road Interaction, the spray, the wet car's spray film, Rain Next-Gen's rings
//! and the rain haze each derived their own wetness from the raw values (three different puddle rules:
//! `level`, `√level`, and the haze's raw road wetness). Now:
//!
//! | reader | reads |
//! |---|---|
//! | Wet Roads (`world_wet.wgsl`) + the SSR mask (its weight) | [`Wetness::film`] (`wetness_film`, same formula in WGSL) and [`Wetness::puddles`] (`wetness_puddles`) |
//! | Wet Road Interaction (trail map, splashes, spray emitters) | `film`, [`puddle_at`](super::puddle::puddle_at) |
//! | Spray (rates) | `film` |
//! | Wet Car (spray film) | `film` |
//! | Rain Next-Gen rings | `film` → `wetness_puddle` (the same field) |
//! | Rain haze (wet air weight) | `film` |
//!
//! The WGSL twin is `shaders/enh/weather/wetness.wgsl` (included by the world and the weather modules).

/// The rain amount from which the film is full (Bayview's rain maps reach 0.7).
pub const FULL_RAIN: f32 = 0.6;

/// The scene's wetness this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wetness {
    /// The rain's amount (weather +0x1F0), 0..1.
    pub rain: f32,
    /// The road's wetness from the rain (`Weather::road_wet`), 0..1.
    pub road: f32,
    /// The water film (standing water) on the surfaces, 0..1: the road's wetness, thinner in a
    /// drizzle and once the rain stops (× (0.35 + 0.65 · min(rain / 0.6, 1)); **inferred**: Wet Roads'
    /// level × (0.35 + 0.65 rain) and the spray's own `water_depth` (full from 0.6: Bayview's
    /// rain maps reach 0.7, so the old level never reached 1 in a shower) made one).
    pub film: f32,
    /// The puddles' share of the field's threshold: √film (they dry more slowly than the film;
    /// Wet Road Interaction's rule, now the only one).
    pub puddles: f32,
}

impl Wetness {
    pub fn of(rain: f32, road_wet: f32) -> Wetness {
        let rain = rain.clamp(0.0, 1.0);
        let road = road_wet.clamp(0.0, 1.0);
        let film = road * (0.35 + 0.65 * (rain / FULL_RAIN).min(1.0));
        Wetness { rain, road, film, puddles: film.sqrt() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_is_zero_and_the_film_follows_the_rain() {
        assert_eq!(Wetness::of(0.0, 0.0), Wetness::default());
        assert_eq!(Wetness::of(1.0, 0.0).film, 0.0);
        let full = Wetness::of(1.0, 1.0);
        assert!((full.film - 1.0).abs() < 1e-6 && (full.puddles - 1.0).abs() < 1e-6);
        assert_eq!(Wetness::of(0.7, 1.0).film, full.film);
        let drizzle = Wetness::of(0.2, 1.0);
        assert!(drizzle.film < full.film && drizzle.puddles > drizzle.film);
        assert_eq!(Wetness::of(3.0, 2.0), full);
    }

    /// The WGSL twin uses the same formula (text check: the constants of `wetness_film`).
    #[test]
    fn wgsl_twin_has_the_same_rule() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/render/shaders/enh/weather/wetness.wgsl"));
        assert!(src.contains("(0.35 + 0.65 * min(max(rain, 0.0) / 0.6, 1.0))"), "film rule");
        assert_eq!(FULL_RAIN, 0.6);
        assert!(src.contains("sqrt(max(film, 0.0))"), "puddle rule");
    }
}
