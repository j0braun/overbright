//! Surface Detail (Graphics page, Lighting group; a clone deviation, opt-in,
//! docs/surface-maps.md): material-ID roughness / specular for the world's
//! textures, and normal maps derived on the player's machine from the player's
//! own game textures (nothing is shipped).
//!
//! SPEED2.EXE has no material model for the world: world.efx is
//! `tex · v0 · 2` (baked vertex light), worldreflect adds the planar road
//! reflection by the texture's alpha. So everything here is invented and
//! only feeds the clone's own light terms: the headlight projector's light
//! (view 9) and, when on, Dynamic Lamps, Headlight Beams, Wet Roads and SSR.
//!
//! * [`classify`]: a texture's material from its name, its draws' effect
//!   (worldreflect = a road surface: mesh flag 0x40) and its blend / alpha
//!   test. Cheap and robust: what the research report ranked first
//!   ("material-ID roughness gains more than per-pixel normals").
//! * [`normal_verdict`] / [`build_normals`]: a tangent-space normal map from
//!   the texture's luminance, high-passed (the baked, painted light of the
//!   art is low-frequency: it is removed before the gradient, so it does not
//!   emboss), soft-clipped and kept at a small strength. Textures whose
//!   detail is painted structure rather than surface grain (signs, windows,
//!   art with painted shadows) are rejected by a measure of the gradient's
//!   distribution, besides the class rules.
//! * The maps are made at [`MAP_SIZE`]² (RG8 snorm, full mip chain) and kept
//!   in the player's cache dir, keyed by the source texture's bytes and the
//!   generator's version ([`cache_key`]); [`Builder`] makes them on a
//!   background thread on first use.

use crate::formats::texture::{Blend, Texture};

/// The size of every generated normal map (one texture array).
pub const MAP_SIZE: u32 = 256;
/// Mip levels of a map (256 → 1).
pub const MAP_MIPS: u32 = 9;
/// Bumped whenever the generator's output changes (invalidates the cache).
pub const GENERATOR_VERSION: u32 = 2;

mod builder;
mod cache;
mod classify;
mod normals;
pub use builder::*;
pub use cache::*;
pub use classify::*;
pub use normals::*;
pub(in crate::render) mod gpu;

#[cfg(test)]
mod tests {
    /// The maps in the cache are keyed by [`super::GENERATOR_VERSION`], not
    /// by the build: the generator's output for a fixed picture is pinned
    /// with the version, so a change of the output without a version bump
    /// (which would let every build read the other's maps as its own)
    /// fails here. Bump the version and pin the new hash together.
    #[test]
    fn generator_output_is_pinned_to_its_version() {
        let (w, h) = (64u32, 48u32);
        let rgba: Vec<u8> = (0..w * h)
            .flat_map(|i| {
                let (x, y) = (i % w, i / w);
                let v = ((x * 37 + y * 11) ^ (x * y)) as u8;
                [v, v.wrapping_mul(3), (x * 4) as u8, 255]
            })
            .collect();
        let mut hash = xxhash_rust::xxh3::Xxh3::new();
        for strength in [0.5f32, 1.0, 2.0] {
            hash.update(&super::build_normals(&rgba, w, h, strength).bytes());
        }
        assert_eq!(
            (super::GENERATOR_VERSION, format!("{:016x}", hash.digest())),
            (2, "95c8ca642c9dd7e4".to_string()),
            "the surface map generator's output changed: bump GENERATOR_VERSION and pin the new hash"
        );
    }

    #[test]
    fn a_panicking_job_still_finishes_the_builder() {
        let jobs: Vec<(usize, super::Job<u32>)> =
            vec![(0, Box::new(|| Some(1))), (1, Box::new(|| panic!("bad pack"))), (2, Box::new(|| Some(3)))];
        let mut b = super::Builder::spawn(jobs);
        let mut got = b.wait();
        got.sort_by_key(|g| g.0);
        assert_eq!(got, vec![(0, Some(1)), (1, None), (2, Some(3))]);
        assert!(b.done());
    }

    #[test]
    fn temp_names_are_unique_per_call() {
        let p = std::path::Path::new("/x/abc.rg8");
        assert_ne!(super::temp_path(p), super::temp_path(p));
    }

    #[test]
    fn fit_pack_survives_mismatched_pixels() {
        let (v, w, h) = super::fit_pack(vec![0; 16], 2048, 2048);
        assert_eq!((v.len(), w, h), (16, 2048, 2048));
    }

    use super::*;

    fn info(name: &str) -> TextureInfo {
        TextureInfo {
            name: name.into(),
            blend: Blend::None,
            alpha_test: false,
            reflect: false,
            emissive: false,
            width: 128,
            height: 128,
        }
    }

    /// Deterministic noise in 0..1.
    fn noise(i: usize) -> f32 {
        let mut x = (i as u32).wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
        x ^= x >> 13;
        x = x.wrapping_mul(0xC2B2_AE35);
        x ^= x >> 16;
        (x & 0xFFFF) as f32 / 65535.0
    }

    fn grey(w: usize, h: usize, f: impl Fn(usize, usize) -> f32) -> Vec<u8> {
        let mut v = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let g = (f(x, y).clamp(0.0, 1.0) * 255.0) as u8;
                v.extend([g, g, g, 255]);
            }
        }
        v
    }

    #[test]
    fn classes_from_names_and_draws() {
        assert_eq!(classify(&TextureInfo { reflect: true, ..info("XYZ_01") }), Class::Asphalt);
        assert_eq!(classify(&info("ARC_SIDEWALK_A")), Class::Concrete);
        assert_eq!(classify(&info("OBJ_FENCE_CHNLINK")), Class::Metal);
        assert_eq!(classify(&info("ARC_OFFICEWINDOW_D_BK")), Class::Glass);
        assert_eq!(classify(&info("ORG_MAPLEA01")), Class::Rough);
        assert_eq!(classify(&info("TRN_IP_TRAINLOADINGBAYA")), Class::Rough);
        assert_eq!(classify(&info("ARC_A51CONCRETE2")), Class::Concrete);
        assert_eq!(classify(&info("TRN_FREEWAYLOD_A_DM")), Class::Rough);
        assert_eq!(classify(&info("ARC_WHOUSE_METAL_01")), Class::Metal);
        assert_eq!(classify(&TextureInfo { reflect: true, ..info("ARC_SIDEWALK_E") }), Class::Concrete);
        assert_eq!(classify(&TextureInfo { reflect: true, ..info("RDP_LANEA_YS_AA_KT") }), Class::Asphalt);
        assert_eq!(classify(&info("SGN_BACK_001")), Class::Sign);
        assert_eq!(classify(&info("ORG_PALMTREEBARK")), Class::Rough);
        assert_eq!(classify(&info("ARC_SKYDOME_A_LL")), Class::Skip);
        assert_eq!(classify(&info("ARC_PANARAMABUILDINGSB_")), Class::Skip);
        assert_eq!(classify(&TextureInfo { blend: Blend::Additive, ..info("ROAD") }), Class::Skip);
        assert_eq!(classify(&TextureInfo { emissive: true, ..info("ROAD") }), Class::Skip);
        assert_eq!(Class::Skip.packed(), 0);
        let p = Class::Asphalt.packed();
        assert_eq!(p >> 24, 1);
        assert_eq!((p & 0xFF) as f32 / 255.0, (0.62f32 * 255.0).round() / 255.0);
    }

    #[test]
    fn grain_gets_a_map_painted_shapes_and_flat_do_not() {
        let (w, h) = (128, 128);
        // Asphalt-like grain over a painted light gradient.
        let grain = grey(w, h, |x, y| 0.35 + 0.25 * (x as f32 / w as f32) + 0.12 * (noise(y * w + x) - 0.5));
        assert_eq!(normal_verdict(&info("ROAD_A"), Class::Asphalt, &grain, w as u32, h as u32), NormalVerdict::Map);
        // A sign: flat colour fields with sharp edges.
        let sign = grey(w, h, |x, y| if (x / 32 + y / 32) % 2 == 0 { 0.2 } else { 0.8 });
        assert_eq!(normal_verdict(&info("ROAD_B"), Class::Asphalt, &sign, w as u32, h as u32), NormalVerdict::Painted);
        let flat = grey(w, h, |x, _| 0.4 + 0.2 * x as f32 / w as f32);
        assert_eq!(normal_verdict(&info("ROAD_C"), Class::Asphalt, &flat, w as u32, h as u32), NormalVerdict::Flat);
        assert_eq!(normal_verdict(&info("BILLBOARD"), Class::Sign, &grain, w as u32, h as u32), NormalVerdict::Class);
    }

    #[test]
    fn painted_light_does_not_tilt_the_map() {
        // A pure low-frequency light gradient (baked art) with fine grain:
        // the mean normal stays straight up (the gradient is high-passed
        // away), the grain tilts single texels a little.
        let (w, h) = (128, 128);
        let img = grey(w, h, |x, y| 0.2 + 0.6 * (x as f32 / w as f32) + 0.1 * (noise(y * w + x) - 0.5));
        let m = build_normals(&img, w as u32, h as u32, 1.0);
        assert_eq!(m.levels.len(), MAP_MIPS as usize);
        assert_eq!(m.levels[0].len(), (MAP_SIZE * MAP_SIZE) as usize);
        let n = m.levels[0].len() as f32;
        let mean_x = m.levels[0].iter().map(|p| p[0] as f32).sum::<f32>() / n / 127.0;
        assert!(mean_x.abs() < 0.03, "mean x {mean_x}");
        let max = m.levels[0].iter().map(|p| (p[0] as f32).hypot(p[1] as f32) / 127.0).fold(0.0, f32::max);
        assert!(max > 0.05 && max < 0.75, "max tilt {max}");
        // Round trip through the cache's bytes.
        assert_eq!(NormalMap::from_bytes(&m.bytes()).as_ref(), Some(&m));
    }

    #[test]
    fn a_ramp_tilts_away_from_uphill() {
        // Height rising in +x within a bump: the normal's x is negative on
        // the bump's rising side.
        let (w, h) = (64, 64);
        let img = grey(w, h, |x, _| if (28..36).contains(&x) { 0.5 + 0.3 * ((x as f32 - 28.0) / 8.0 * std::f32::consts::PI).sin() } else { 0.5 } + 0.02 * noise(x));
        let m = build_normals(&img, w as u32, h as u32, 1.0);
        let s = MAP_SIZE as usize;
        let row = &m.levels[0][(s / 2) * s..(s / 2) * s + s];
        let rising = row[(29 * s) / w + 2][0];
        let falling = row[(34 * s) / w + 2][0];
        assert!(rising < 0 && falling > 0, "{rising} {falling}");
    }

    #[test]
    fn car_classes_from_texture_and_material() {
        assert_eq!(car_class("SKYLINE_TIRE", "RUBBER"), Class::CarTyre);
        assert_eq!(car_class("ADVAN_STYLE01_WHEEL", "RUBBER"), Class::CarTyre);
        assert_eq!(car_class("SKYLINE_TIRE", "MAGGUNMETAL"), Class::Skip);
        assert_eq!(car_class("SKYLINE_SKIN1", "GRILL"), Class::CarMesh);
        assert_eq!(car_class("GRILL_03", "GRILL"), Class::CarMesh);
        assert_eq!(car_class("SKYLINE_SKIN1", "REGPAINTBLUE"), Class::Skip);
        assert_eq!(car_class("SKYLINE_SKIN1", "METPAINTSILVER"), Class::Skip);
        assert_eq!(car_class("SKYLINE_SKIN1", "CHROME"), Class::Skip);
        assert_eq!(car_class("SKYLINE_SKIN1", "CLEARPLASTIC"), Class::Skip);
        assert_eq!(car_class("LANCEREVO8_SKIN1", "INTERIOR"), Class::CarInterior);
        assert_eq!(car_class("CARBONFIBRE", "CARBONFIBER"), Class::Skip);
        assert_eq!(car_class("CARBONFIBRE", "DEFAULT"), Class::Skip);
        assert_eq!(car_class("SKYLINE_BADGING", "DECAL"), Class::CarBadge);
        // Vinyls and other decals: native.
        assert_eq!(car_class("VINYL_FLAME01", "DECAL"), Class::Skip);
        assert_eq!(car_class("DEFAULTALPHA", "DECAL"), Class::Skip);
        assert_eq!(car_class("WINDOW", "WINDSHIELD"), Class::Skip);
        assert_eq!(car_class("BRAKE_STYLE00", "BRAKEDISC_DIFFUSE"), Class::CarMesh);
        assert_eq!(car_class("SKYLINE_ENGINE", "ALUMINUM"), Class::CarInterior);
        for c in [Class::CarTyre, Class::CarMesh, Class::CarBadge, Class::CarInterior] {
            assert!(c.params().normal > 0.0);
        }
    }

    /// An atlas: grain in the left half (a tyre's region), flat paint on
    /// the right. The map tilts only inside the region; outside it is flat
    /// (0, 0): the paint stays native. A region of drawn shapes alone is
    /// rejected; the cache bytes round trip.
    #[test]
    fn car_map_only_inside_its_regions() {
        let (w, h) = (128usize, 128usize);
        let img = grey(w, h, |x, y| if x < 64 { 0.4 + 0.15 * (noise(y * w + x) - 0.5) } else { 0.6 });
        let left = CarRegion {
            class: Class::CarTyre,
            tris: vec![[[0.0, 0.0], [0.5, 0.0], [0.0, 1.0]], [[0.5, 0.0], [0.5, 1.0], [0.0, 1.0]]],
        };
        let m = build_car_normals(&img, w as u32, h as u32, std::slice::from_ref(&left)).expect("a map");
        assert_eq!(m.levels.len() as u32, CarMap::mips(w as u32, h as u32));
        let at = |x: usize, y: usize| m.levels[0][y * w + x];
        assert!((4..60).any(|x| at(x, 40) != [0, 0]));
        // (The region grows a texel and wraps: the last column borders it.)
        for x in 70..124 {
            assert_eq!(at(x, 40), [0, 0], "x {x}");
        }
        assert_eq!(CarMap::from_bytes(&m.bytes()).as_ref(), Some(&m));
        // Paint only (no relief region over the grain): no map.
        let right = CarRegion { class: Class::CarMesh, tris: vec![[[0.6, 0.0], [1.0, 0.0], [0.6, 1.0]]] };
        assert!(build_car_normals(&img, w as u32, h as u32, &[right]).is_none());
        // Holes (alpha below the test's reference) are outside the region.
        let mut holed = img.clone();
        for p in holed.chunks_exact_mut(4) {
            p[3] = 0;
        }
        assert!(build_car_normals(&holed, w as u32, h as u32, &[left]).is_none());
    }

    #[test]
    fn pack_replacements_fit_and_key_apart() {
        let big = grey(2048, 1024, |x, y| noise(y * 2048 + x));
        let (b, w, h) = fit_pack(big, 2048, 1024);
        assert_eq!((w, h, b.len()), (512, 256, 512 * 256 * 4));
        let (b2, w2, h2) = fit_pack(vec![7; 64 * 64 * 4], 64, 64);
        assert_eq!((w2, h2, b2[0]), (64, 64, 7));
        assert_ne!(pack_cache_key(1, 1.0), pack_cache_key(2, 1.0));
        assert_ne!(pack_cache_key(1, 1.0), pack_cache_key(1, 0.8));
        let r = vec![CarRegion { class: Class::CarTyre, tris: vec![[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]] }];
        assert_ne!(car_cache_key(1, &r), car_cache_key(2, &r));
    }
}
