//! One medium (docs/atmosphere.md "One medium"): the night air's density
//! every volume effect reads. Nothing here is native (SPEED2.EXE's air is world.efx's per-vertex fog and
//! the horizon fog grid, applied unchanged); every value is **inferred** (the atmosphere's look).
//!
//! Before, five volume paths each had their own air and their own idea of what the rain does to it:
//! Volumetric Fog Next-Gen (`atmosphere::medium`, ×(1 + 1.3 rain)), Low/High (a separate cone pass,
//! ×(1 + 0.3 rain) on High), the Next-Gen car lights' air (`car_lights::Look::density`, ×(1 + 1.5 rain)),
//! Rain Next-Gen's haze (`rain_fx::HAZE_LOOK`, × rain) and Rain Detail's wet-air lamp halos. Now one
//! extinction and one rain response ([`Medium`]) and one energy rule:
//!
//! | effect | reads |
//! |---|---|
//! | Volumetric Fog Low / High / Next-Gen (froxels, `atmosphere::tier`): **the one volume** | [`Medium::extinction`], [`Medium::height`]; every light it takes (street lamps, neon, every car's headlight cone, the cars' tail / brake lamps; fewer on the cheaper tiers) × [`Medium::scatter_scale`] |
//! | Car Lights Next-Gen air (cones, halos) | its density × [`Medium::thickness`]; only with Volumetric Fog Off (the froxels, at any tier, hold its cones and lamps) |
//! | Rain Next-Gen haze | the rain's share of the medium ([`Medium::rain_share`]); none while the froxel medium runs (any Volumetric Fog tier); the cars' cones none with Car Lights Next-Gen (each light in the air once) |
//! | Rain Detail wet-air halos | none (the native glow sprite is the lamp's halo: no second glow) |
//! | Headlight Beams' road light | × [`Medium::beam_transmittance`] while the froxels run, any tier (what the air scatters does not reach the road) |
//!
//! The energy rules: what a light scatters into the air grows as √thickness in rain (the extinction as the
//! thickness: denser air shows every beam and halo more, never as a white-out), is bounded by the light's own
//! energy (single scattering with the medium's transmittance; a beam's scattered light is taken off its road
//! light), and the light a whole frame or a whole stretch of the view shares is a veil the native fog already
//! draws (the froxel pass's veil floors, `atmosphere.wgsl` `cs_floor` / `tile_floor`): the blacks stay black.

/// A headlight's mean way through the air to the road it lights (m): the beam lights the road from about
/// 5 m (HeadlightClipTex's near cut-off) out, its light falling with 1 / (1 + z² / 100) (inferred).
pub const BEAM_PATH: f32 = 12.0;

/// The air for a rain amount.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Medium {
    /// The haze's extinction at the ground (1/m).
    pub extinction: f32,
    /// Its height falloff (m).
    pub height: f32,
    /// The extinction against the dry air's (1 dry; 2.3 in full rain).
    pub thickness: f32,
}

/// The medium for a rain amount (0..1).
pub fn medium(rain: f32) -> Medium {
    let (extinction, height) = super::atmosphere::medium(rain);
    let (dry, _) = super::atmosphere::medium(0.0);
    Medium { extinction, height, thickness: extinction / dry.max(1e-9) }
}

impl Medium {
    /// The scale on a light's scattering gain against the dry air's, so that the light it scatters
    /// grows as √thickness in rain (the cells' extinction grows as the thickness): denser air shows
    /// more of every beam and halo, but less than its extinction grows (the rest of the drops'
    /// scattering is the narrow forward peak the native glow sprites draw). One rule for every light
    /// the medium takes: street lamps, signs, headlights, car lamps.
    pub fn scatter_scale(&self) -> f32 {
        1.0 / self.thickness.max(1e-6).sqrt()
    }

    /// The share of a headlight's light that crosses the medium to the road: the haze's extinction over
    /// [`BEAM_PATH`]. What the medium scatters out of a beam (its light in the air) is light the road does
    /// not get: Headlight Beams' road light × this while the froxel medium runs (energy conserved).
    pub fn beam_transmittance(&self) -> f32 {
        (-self.extinction * BEAM_PATH).exp()
    }

    /// The rain's share of the medium, 0..1 (0 dry, 1 in full rain): (thickness − 1) over its full-rain value.
    pub fn rain_share(&self) -> f32 {
        let full = medium(1.0).thickness - 1.0;
        if full <= 0.0 { 0.0 } else { ((self.thickness - 1.0) / full).clamp(0.0, 1.0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rain_scatters_more_but_bounded() {
        let (dry, wet) = (medium(0.0), medium(1.0));
        assert_eq!(dry.scatter_scale(), 1.0);
        // The light scattered (extinction × scale) grows as √thickness: more than dry, less than the extinction.
        let grow = wet.thickness * wet.scatter_scale();
        assert!(grow > 1.2 && grow < wet.thickness, "{grow}");
        assert!((grow - wet.thickness.sqrt()).abs() < 1e-5);
        // A beam keeps most of its light; rain takes more of it.
        let (t0, t1) = (dry.beam_transmittance(), wet.beam_transmittance());
        assert!(t0 < 1.0 && t1 < t0 && t1 > 0.8, "{t0} {t1}");
    }

    #[test]
    fn one_rain_response() {
        let (dry, wet) = (medium(0.0), medium(1.0));
        assert_eq!(dry.thickness, 1.0);
        assert!(wet.thickness > 2.0 && wet.extinction > dry.extinction && wet.height > dry.height);
        assert_eq!(dry.rain_share(), 0.0);
        assert!((wet.rain_share() - 1.0).abs() < 1e-6);
        assert!((medium(0.4).rain_share() - 0.4).abs() < 1e-5);
    }
}
