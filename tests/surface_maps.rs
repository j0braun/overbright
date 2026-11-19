//! Surface Detail (docs/notes/graphics/surface-maps.md) on Bayview's own textures:
//! the classes of the most visible road, wall and plant textures, which of
//! them get a normal map, and that a map is small, flat on average and
//! round-trips the cache's bytes. Ignored without NFSU2_ROOT.
mod common;

use nfsu2::surface_maps::{self as sm, Class, NormalVerdict};

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn bayview_textures_classes_and_maps() {
    let root = common::game_root();
    let world = nfsu2::world::World::load(&root, "L4RA").expect("L4RA");
    let by_name = |n: &str| world.textures.values().find(|t| t.name == n).unwrap_or_else(|| panic!("{n}"));
    let cases: [(&str, bool, Class, Option<NormalVerdict>); 8] = [
        ("RDP_LANEA_YS_AA_KT", true, Class::Asphalt, Some(NormalVerdict::Map)),
        ("ARC_TILEWALLWHITE_LK", false, Class::Concrete, Some(NormalVerdict::Map)),
        ("ARC_WHOUSE_METAL_01", false, Class::Metal, Some(NormalVerdict::Map)),
        ("ARC_SIDEWALK_E", true, Class::Concrete, Some(NormalVerdict::Map)),
        ("ORG_GREENBUSH", false, Class::Rough, Some(NormalVerdict::Class)),
        ("SKY_SUNRISE_A", false, Class::Skip, Some(NormalVerdict::Class)),
        ("ARC_UC_HOTELNEWWINDOWS", false, Class::Glass, Some(NormalVerdict::Class)),
        // Painted parking bays: drawn shapes, rejected by the edge measure.
        ("RDP_PARKING_WL_AA_KT", false, Class::Asphalt, Some(NormalVerdict::Painted)),
    ];
    for (name, reflect, class, verdict) in cases {
        let t = by_name(name);
        let c = sm::slot_class(t, reflect, false);
        assert_eq!(c, class, "{name}");
        let rgba = t.decode_rgba();
        let v = sm::normal_verdict(&sm::TextureInfo::of(t, reflect), c, &rgba, t.width, t.height);
        assert_eq!(Some(v), verdict, "{name}");
        if v == NormalVerdict::Map {
            let m = sm::build_normals(&rgba, t.width, t.height, c.params().normal);
            let n = m.levels[0].len() as f32;
            let (mx, my) = m.levels[0].iter().fold((0.0, 0.0), |a, p| (a.0 + p[0] as f32 / 127.0 / n, a.1 + p[1] as f32 / 127.0 / n));
            assert!(mx.abs() < 0.05 && my.abs() < 0.05, "{name}: mean tilt {mx} {my}");
            let rms = (m.levels[0].iter().map(|p| (p[0] as f32 / 127.0).powi(2) + (p[1] as f32 / 127.0).powi(2)).sum::<f32>() / n).sqrt();
            assert!(rms > 0.03 && rms < 0.45, "{name}: rms tilt {rms}");
            assert_eq!(sm::NormalMap::from_bytes(&m.bytes()).as_ref(), Some(&m));
        }
    }
    // The reviewed rejections never become candidates.
    for name in sm::REJECTED {
        assert!(!sm::map_candidate(by_name(name), Class::Concrete), "{name}");
    }
}
