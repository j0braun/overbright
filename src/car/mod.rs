//! A car: its parts, textures and lighting materials, assembled from
//! `CARS/<NAME>/GEOMETRY.BIN`, the car texture packs and the global tables in
//! `GLOBAL/GLOBALB.BUN`.
//!
//! See docs/formats.md ("Cars") for the evidence behind each field.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use glam::{Mat4, Vec3};

use crate::formats::bundle::{self, id, read_cstr, read_f32, read_u16, read_u32, read_vec3};
use crate::formats::hash::{bin_hash, bin_hash_from};
use crate::formats::parts::PartDb;
use crate::formats::solid::{Solid, parse_solid_list};
use crate::formats::texture::{Texture, parse_texture_pack};
use crate::handling::{Handling, parse_handling};

mod assets;
mod lights;
mod load;
mod lod;
mod material;
mod model;
mod paint;
mod parts;
mod record;
mod skin;
mod wheels;

pub use self::assets::*;
pub use self::lights::*;
pub use self::lod::*;
pub use self::material::*;
pub use self::model::*;
pub use self::paint::*;
pub use self::parts::*;
pub use self::record::*;
pub use self::skin::*;
pub use self::wheels::*;

#[derive(Clone)]
pub struct Car {
    pub name: String,
    pub car_type: CarType,
    pub solids: Vec<Solid>,
    pub parts: Vec<CarPart>,
    pub textures: HashMap<u32, Texture>,
    pub light_materials: HashMap<u32, LightMaterial>,
    /// Generic texture hash used by the geometry → the texture to bind
    /// (the replacement table built by 0x6381A0 / 0x631E30).
    pub texture_remap: HashMap<u32, u32>,
    /// Same idea for light materials (`CARSKIN` → the paint's material).
    pub light_material_remap: HashMap<u32, u32>,
    /// The rims' paint material (render info +0x390), which the rim draw
    /// (0x61797F) puts in place of MAGSILVER, MAGCHROME and 0x12049251.
    pub rim_material: Option<u32>,
    /// The brakes' paint material (render info +0x398), in place of
    /// 0xD6640DFF and 0xA3186E2B in the brake models (0x62E05F).
    pub brake_material: Option<u32>,
    /// The front wheels' x-turn (radians; 0x620AF0 at 0x6270B2…0x627112):
    /// 7° (0x7A675C) × v, v = 0.5 per fitted performance part 0x24
    /// (Adjustable Camber Plates) and 0x23 (0x59A430 reads the upgrade
    /// state's part bytes, ride +0x340 + id), cut to the game's binary
    /// angle (trunc(7·v·65536) / 360). See [`WheelPose::tilt`].
    pub camber: f32,
    /// The audio paint (render info +0x39C, 0x638DE8..0x63909B): the slot
    /// 0x48 part's light material (attribute 0x6BA02C05) when the audio
    /// part (slot 0x22) is level 3 or more; `None` takes the body paint.
    pub audio_material: Option<u32>,
    /// The mirror paint (render info +0x3A8): slot 0x4C's light material
    /// with the mirrors (slot 0x20) above level 0; `None` takes the body
    /// paint (see [`CarPart::mirror_paint`]).
    pub mirror_material: Option<u32>,
    /// The body's convex collision hull (`<CAR>_CV`), car space; empty if
    /// the car has none, when the game uses [`DEFAULT_HULL`].
    pub hull: Vec<Vec3>,
    /// The hull's vertex links (0x5AFBB0's climb); empty for the default box.
    pub hull_links: crate::formats::collision::HullLinks,
    /// The hull's reach (0x5AE530), worked out once at load: its
    /// farthest point from the origin and its highest and lowest z, each
    /// from 0. See [`Car::hull_reach`].
    hull_reach: HullReach,
    /// The headlight pool's colour, R, G, B (render info +0x380, 0x635D62):
    /// the RED, GREEN and BLUE attributes (0 when missing) of the lights
    /// part (slot 0x9F, ride +0x80C), or 0x90AA90 (R 0x90, G 0xAA, B 0x90)
    /// when the car has none (0x635D9B).
    pub headlight_pool_color: [u8; 3],
    /// The underglow's colours (0x635B7C → 0x61FC30 on the slot 0x9A
    /// part, ride +0x7F8): see [`crate::shadow::Neon`].
    pub neon: crate::shadow::Neon,
    /// The underglow tubes (0x6357A0..0x635901 → render info +0x2C18,
    /// count +0x2C14): the car-space START / END marker pairs of
    /// [`crate::shadow::NEON_TUBE_PREFIXES`] in the LOD A group 0 models of
    /// [`crate::shadow::NEON_TUBE_SLOTS`]; empty without a slot 0x9A part
    /// (ride +0x7F8, 0x615A91: no tubes are drawn then).
    pub neon_tubes: Vec<[Vec3; 2]>,
    /// The engine neon (slot 0x9B, ride +0x7FC) and trunk neon (slot 0x9D,
    /// ride +0x804): their colours (0x635B9B / 0x635BF3 → 0x61FC30 into
    /// render info +0x35C / +0x374, the pulse into +0x364 / +0x37C by
    /// 0x622AA8 / 0x622BF8, each only with its slot's part) and their tubes
    /// (render info +0x2C64, count +0x2C60: ENGINE_NEON_START_i / _END_i,
    /// i < 5, of the slot 0x0D engine's LOD A model 0 (+0x1948 → +0xC);
    /// +0x2CFC, count +0x2CF8: MARKER_TRUNK_NEON_START_i / _END_i, i < 4,
    /// of the trunk layout's model, +0x21B4 → +0xC, **inferred** to be
    /// the slot 0x22 part's LOD A models). `None` without the slot's part
    /// (0x615A20 tests ride +0x7FC / +0x804).
    pub engine_neon: Option<(crate::shadow::Neon, Vec<[Vec3; 2]>)>,
    pub trunk_neon: Option<(crate::shadow::Neon, Vec<[Vec3; 2]>)>,
    /// The colours of the skin's seven part-paint marker texels (0x61D7A0,
    /// slots [`PART_PAINT_SLOTS`]): the fitted paint part's RED, GREEN and
    /// BLUE and GLOSS, `None` for the body paint (no part, or slot 0x48
    /// without a level-3 audio part).
    pub part_paints: [Option<[u8; 4]>; 7],
    /// The panel-line layer composited over the vinyls (0x612B80): the
    /// car name plus `_DOORLINE_WIDEBODY` with a wide body (slot 6 above
    /// level 0), else `_DOORLINE_KIT` with slot 1, 2 or 0xB above level 0,
    /// else `_DOORLINE`; its mask is that name plus `_MASK`.
    pub doorline: String,
    /// The box the shadow is laid over (render info +0x10 / +0x20, set by
    /// 0x634810): car space, min and max. See [`SHADOW_BOX_SLOTS`].
    pub shadow_box: (Vec3, Vec3),
    /// The light flares (render info list +0x3F0, built by 0x6204E0): see
    /// [`light_flares`].
    pub light_flares: Vec<crate::glow::CarFlare>,
    /// The hood, trunk and door animations
    /// (`CARS\<CAR>\PARTS_ANIMATIONS.BIN`, [`crate::formats::zam`]).
    pub animations: Option<crate::formats::zam::PartAnimations>,
    /// Which animation each channel plays for the fitted parts (0x61F830).
    pub part_styles: crate::part_anim::Styles,
    /// The exhaust markers, car space (the list at render info +0x598 that
    /// 0x639843 fills; see the exhaust draw). The nitrous flame bursts at
    /// their positions (**verified**: XES_NOS_FlameA's marker group 10 is
    /// render +0x548 + 8·10 = +0x598; 0x615CD0 takes only the marker's
    /// position, the emitter keeps its own axes).
    pub exhausts: Vec<Mat4>,
    /// The drawn wheels' x shifts (table order) of Fix Original Model
    /// Errors ([`crate::model_fixes::wheel_shift`]); zeros when off.
    pub wheel_shift: [f32; 4],
    /// The Nitrous Purge's smoke (car effect kinds 13 / 14), from the
    /// fitted slot 0xA2 part (render set-up 0x634810).
    pub purge: Option<PurgeFx>,
    /// The visual parts the car was built with (slot → part hash, the
    /// performance packages' visual parts included): with
    /// [`Car::load_with`] they build the same look again (a recorded run's
    /// cars, docs/modding/map-editor.md "Simulate"). Empty for a car
    /// loaded for its physics alone.
    pub look_parts: std::collections::BTreeMap<u8, u32>,
}

#[cfg(test)]
mod carbon_decal_tests {
    use super::*;

    /// 0x634B0B…0x634E69 with the kit carbon (0x5D) on: bumpers full; body,
    /// skirt and doors partial; the wide body and its doors full only with
    /// a wide body; the trunk full with its own carbon (0x60).
    #[test]
    fn carbon_tables() {
        let kit = |s: usize| s == KIT_CARBON;
        assert_eq!(carbon_for(1, &kit, false), Carbon::Full);
        assert_eq!(carbon_for(5, &kit, false), Carbon::Partial);
        assert_eq!(carbon_for(0x11, &kit, false), Carbon::Partial);
        assert_eq!(carbon_for(6, &kit, false), Carbon::None);
        assert_eq!(carbon_for(0xA, &kit, false), Carbon::Partial);
        let wide = |s: usize| s == KIT_CARBON || s == 6;
        assert_eq!(carbon_for(6, &wide, false), Carbon::Full);
        assert_eq!(carbon_for(0x12, &wide, false), Carbon::Full);
        let trunk = |s: usize| s == TRUNK_CARBON;
        assert_eq!(carbon_for(0xA, &trunk, false), Carbon::Full);
        assert_eq!(carbon_for(9, &trunk, true), Carbon::Full);
        assert_eq!(carbon_for(1, &trunk, false), Carbon::None);
    }

    /// 0x61FF50: a RECT placement's decals take NAME + `_RECT`; empty
    /// entries DEFAULTALPHA; the wide door slot 0x3B shares block 3.
    #[test]
    fn decal_block_names() {
        let attr = |slot: usize, key: u32| -> Option<Option<u32>> {
            match slot {
                0x37 if key == bin_hash("SIZE") => Some(Some(bin_hash("MEDIUM"))),
                0x37 if key == bin_hash("SHAPE") => Some(Some(bin_hash("RECT"))),
                0x37 => Some(None),
                0x79 => Some(Some(0x352D_08D1).filter(|_| key == bin_hash("NAME"))),
                _ => None,
            }
        };
        let b = decal_block(0x3B, &attr, true);
        assert_eq!(b[0], bin_hash_from(0x352D_08D1, "_RECT"));
        assert_eq!(b[1], bin_hash(DEFAULT_ALPHA));
        assert_eq!(decal_block(0x37, &attr, true), b);
        assert_eq!(decal_block(0x38, &attr, true), [bin_hash(DEFAULT_ALPHA); 8]);
    }
}

#[cfg(test)]
mod shared_part_tests {
    use super::*;

    /// Slot 8 (TOP): the G35 and 240SX draw `ROOF` (the sunroof) alone,
    /// and `FULLROOF` alone with a roof scoop fitted (0x62315B); the two
    /// Carbon parts draw with the `CARBONFIBER` light material (render info
    /// +0x388) where the generic part draw remaps to it (0x6231B3,
    /// 0x623451; docs/notes/graphics/car-materials.md): a carbon hood's CARSKIN ranges,
    /// with KIT_CARBON the bumpers' CARSKIN ranges and every generic part's
    /// 0xA7366AE6 ranges; never the spoiler (its own draw), never a stock
    /// car. Compared in the original's Body Shop (G35 DOUBLE-V carbon hood,
    /// carbon body kit): the maintainer's private captures.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn carbon_parts_take_the_carbonfiber_material() {
        let root = crate::testkit::game_root();
        let db = PartDb::load(&std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap()).unwrap();
        // (solid name, light-material reference) → drawn material name.
        let drawn = |c: &Car| -> Vec<(String, u32, String)> {
            let mut v = Vec::new();
            for part in &c.parts {
                let solid = &c.solids[part.solid];
                for m in &solid.materials {
                    let Some(&h) = solid.light_materials.get(m.light_material as usize) else { continue };
                    let name = c.part_light_material_for(part, h).map_or(String::new(), |l| l.name.clone());
                    v.push((solid.name.clone(), h, name));
                }
            }
            v
        };
        let carbon = |v: &[(String, u32, String)], f: &dyn Fn(&str, u32) -> bool| {
            v.iter().filter(|(n, h, _)| f(n, *h)).map(|(_, _, m)| m == "CARBONFIBER").collect::<Vec<_>>()
        };
        let stock = Car::load(&root, "G35").unwrap();
        assert!(drawn(&stock).iter().all(|(_, _, m)| m != "CARBONFIBER"), "stock G35");
        // DOUBLE-V carbon (hood with the CARBONFIBRE attribute).
        let hood =
            db.parts_for("G35", 9).into_iter().find(|p| db.attribute(p, CARBONFIBRE).is_some_and(|v| v != 0)).unwrap();
        let car = Car::load_with(&root, "G35", &std::collections::BTreeMap::from([(9u8, hood.hash)])).unwrap();
        let v = drawn(&car);
        let on_hood = carbon(&v, &|n, h| n.contains("HOOD") && h == CARSKIN_HASH);
        assert!(!on_hood.is_empty() && on_hood.iter().all(|&c| c), "carbon hood: CARSKIN → CARBONFIBER");
        assert!(
            carbon(&v, &|n, h| !n.contains("HOOD") && h == CARSKIN_HASH).iter().all(|&c| !c),
            "the rest keeps the paint"
        );
        // The carbon body kit (KIT_CARBON above level 0).
        let kit = db.parts_for("G35", KIT_CARBON).into_iter().find(|p| p.level > 0).unwrap();
        let car =
            Car::load_with(&root, "G35", &std::collections::BTreeMap::from([(KIT_CARBON as u8, kit.hash)])).unwrap();
        let v = drawn(&car);
        let bumpers = carbon(&v, &|n, h| n.contains("BUMPER") && h == CARSKIN_HASH);
        assert!(!bumpers.is_empty() && bumpers.iter().all(|&c| c), "kit: the bumpers' CARSKIN");
        let skirts = carbon(&v, &|_, h| h == 0xA736_6AE6);
        assert!(!skirts.is_empty() && skirts.iter().all(|&c| c), "kit: every 0xA7366AE6 range");
        assert!(
            carbon(&v, &|n, h| n.contains("DOOR") && h == CARSKIN_HASH).iter().all(|&c| !c),
            "no wide body: the doors keep the paint"
        );
        // A community save's Lancer (docs/notes/graphics/car-look.md "Chrome trunk flicker and dry-road reflection strength"): TRUNK_CARBON
        // fitted; its trunk drew the yellow gloss paint's material with the
        // carbon texture's alpha 1 (twice the skin's gloss weight), a
        // mirror. Now CARBONFIBER, as the original (Body Shop, same scene).
        let trunk = db.parts_for("LANCEREVO8", TRUNK_CARBON).into_iter().find(|p| p.level > 0).unwrap();
        let car =
            Car::load_with(&root, "LANCEREVO8", &std::collections::BTreeMap::from([(TRUNK_CARBON as u8, trunk.hash)]))
                .unwrap();
        let v = drawn(&car);
        let lid = carbon(&v, &|n, h| {
            n.contains("_TRUNK_") && !n.contains("UNDER") && !n.contains("AUDI") && h == CARSKIN_HASH
        });
        assert!(!lid.is_empty() && lid.iter().all(|&c| c), "carbon trunk: CARSKIN → CARBONFIBER");
    }

    /// were drawn together before, the closed roof z-fighting with the
    /// sunroof (seen in the original's Body Shop, docs/notes/graphics/model-fixes.md).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn top_draws_one_roof_group() {
        let root = crate::testkit::game_root();
        let db = PartDb::load(&std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap()).unwrap();
        for (car, index) in [("G35", 0x17), ("240SX", 3)] {
            let roofs = |c: &Car| -> Vec<String> {
                c.parts
                    .iter()
                    .map(|p| c.solids[p.solid].name.clone())
                    .filter(|n| n.contains("ROOF_A") && n.starts_with(car))
                    .collect()
            };
            let stock = Car::load(&root, car).unwrap();
            assert_eq!(stock.car_type.handling.type_index, index, "{car}");
            assert_eq!(roofs(&stock), [format!("{car}_KIT00_ROOF_A")], "{car} stock");
            let scoop = db.parts_for(car, 7).into_iter().find(|p| p.level > 0).unwrap();
            let fitted = Car::load_with(&root, car, &std::collections::BTreeMap::from([(7u8, scoop.hash)])).unwrap();
            assert_eq!(roofs(&fitted), [format!("{car}_KIT00_FULLROOF_A")], "{car} with a scoop");
        }
        assert!(top_group_drawn(8, 0, 0, 1) && !top_group_drawn(8, 1, 0, 1), "other cars: group 0 only");
        assert!(top_group_drawn(9, 1, 0x17, 1));
    }

    /// Rachel's 350Z (DDAY_PLAYER_CAR): its Racinghart rims come from
    /// CARS/WHEELS/GEOMETRY_RACINGHART.BIN (the brand-seeded name), drawn at
    /// all four wheels, and its spoiler from CARS/SPOILER at the SPOILER
    /// marker.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn glass_alpha_is_replaced_on_load() {
        let root = crate::testkit::game_root();
        let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let m = parse_light_materials(&globalb);
        let alpha = |n: &str| {
            let c = m[&bin_hash(n)].constants(0x180);
            (c.diffuse_min[3], c.diffuse_min[3] + c.diffuse_range[3])
        };
        assert_eq!(alpha("WINDSHIELD"), (0.3, 1.0)); // file: 0.697 / 0.082
        assert_eq!(alpha("WINDSHIELD_TINT_L3_PINK"), (0.5, 1.0));
        assert_eq!(alpha("HEADLIGHTGLASS"), (0.4, 0.8));
        // Not on 0x5BB5C0's list: the file's values.
        let mblack = alpha("WINDSHIELD_TINT_L1_MBLACK");
        assert!((mblack.0 - 0.959).abs() < 1e-3 && (mblack.1 - 0.704).abs() < 1e-3);
    }

    /// Left wheels (table 0 and 3) are the right-hand model turned 180°
    /// about z (0x5BAE70 with 0x8000 at 0x617863 / 0x62766E), not mirrored:
    /// their placement keeps a positive determinant and flips x, and the
    /// rim face still lands at the record's +0x1C.
    /// 0x620AF0's LOD pick and 0x48ACB0's size: the thresholds 40 / 30 /
    /// 20 in game, 40 / 20 / 10 for the front-end draw (traffic), nothing
    /// under 4 pixels, Low geometry starting at B.
    /// LOD Fade's hysteresis (clone deviation): a car keeps its LOD within
    /// 10 % of a threshold either way; away from one, and with no LOD
    /// yet, the native pick.
    #[test]
    fn car_lod_hysteresis_keeps_near_thresholds() {
        let h = 1.1;
        // 39 px: B natively; A kept (39 × 1.1 ≥ 40), C switches to B.
        assert_eq!(pick_lod_kept(39.0, 0, LodDraw::InGame, Some(0), h), Some(0));
        assert_eq!(pick_lod_kept(39.0, 0, LodDraw::InGame, Some(2), h), Some(1));
        // Far from a threshold: the native pick.
        assert_eq!(pick_lod_kept(35.0, 0, LodDraw::InGame, Some(0), h), Some(1));
        // Not drawn (below 4) kept at 4.2, drawn again at 4.5.
        assert_eq!(pick_lod_kept(4.2, 0, LodDraw::InGame, None, h), None);
        assert_eq!(pick_lod_kept(4.5, 0, LodDraw::InGame, None, h), Some(3));
        for size in [3.0, 9.0, 15.0, 25.0, 33.0, 45.0, 200.0] {
            let native = pick_lod(size as i32, 0, LodDraw::FrontEnd, false);
            assert_eq!(pick_lod_kept(size, 0, LodDraw::FrontEnd, native, h), native);
        }
    }

    /// 0x6211CB keeps an airborne car whose underside showed
    /// (0x60C330 record 10, two planes (∓0.07, 0, 1), offset 0.05) at C.
    #[test]
    fn an_airborne_car_seen_from_below_keeps_lod_c() {
        let wheels = [
            Vec3::new(1.3, 0.8, 0.3),
            Vec3::new(1.3, -0.8, 0.3),
            Vec3::new(-1.3, 0.8, 0.3),
            Vec3::new(-1.3, -0.8, 0.3),
        ];
        // The part sits at the wheels' mean, z 0.3.
        assert!(underside_visible(Vec3::new(0.0, 0.0, -5.0), wheels, true));
        assert!(underside_visible(Vec3::new(0.0, 0.0, 0.34), wheels, true));
        assert!(!underside_visible(Vec3::new(0.0, 0.0, 0.36), wheels, true));
        assert!(!underside_visible(Vec3::new(0.0, 0.0, 5.0), wheels, true));
        // The planes tilt along x: 20 m ahead (or behind) the eye must be
        // more than 0.07 · 20 − 0.05 below the part.
        assert!(!underside_visible(Vec3::new(20.0, 0.0, -1.0), wheels, true));
        assert!(underside_visible(Vec3::new(20.0, 0.0, -1.5), wheels, true));
        assert!(!underside_visible(Vec3::new(-20.0, 0.0, -1.0), wheels, true));
        // Outside the game (0x8654A4 ≠ 6) the byte is always set.
        assert!(underside_visible(Vec3::new(0.0, 0.0, 5.0), wheels, false));
        assert_eq!(airborne_lod(3, 0, true), 2);
        assert_eq!(airborne_lod(3, 1, true), 3);
        assert_eq!(airborne_lod(3, 0, false), 3);
        assert_eq!(airborne_lod(1, 0, true), 1);
    }

    #[test]
    fn car_lod_steps_down_with_the_screen_size() {
        assert_eq!(pick_lod(3, 0, LodDraw::InGame, false), None);
        assert_eq!(pick_lod(41, 0, LodDraw::InGame, false), Some(0));
        assert_eq!(pick_lod(40, 0, LodDraw::InGame, false), Some(0));
        assert_eq!(pick_lod(39, 0, LodDraw::InGame, false), Some(1));
        assert_eq!(pick_lod(25, 0, LodDraw::InGame, false), Some(2));
        assert_eq!(pick_lod(25, 0, LodDraw::FrontEnd, false), Some(1));
        assert_eq!(pick_lod(10, 0, LodDraw::FrontEnd, false), Some(2));
        assert_eq!(pick_lod(9, 0, LodDraw::FrontEnd, false), Some(3));
        assert_eq!(pick_lod(100, 1, LodDraw::InGame, false), Some(1));
        assert_eq!(pick_lod(10, 0, LodDraw::InGame, true), Some(0));
        // k = 256 / tan(fov / 2); 3 m radius: 30 m ahead at k 256 → 28 px.
        let s = screen_size(Vec3::ZERO, Vec3::Y, 256.0, Vec3::new(0.0, 30.0, 0.0));
        assert_eq!(s, 28);
        assert_eq!(screen_size(Vec3::ZERO, Vec3::Y, 256.0, Vec3::new(0.0, -10.0, 0.0)), 0);
        assert_eq!(screen_size(Vec3::ZERO, Vec3::Y, 256.0, Vec3::new(0.0, 4.0, 0.0)), 512);
    }

    /// A traffic car carries its B / C models as LOD variants (the body
    /// `_BODY_B`, the tyres `_TIRE_FRONT_B`), the wheels only down to C.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn traffic_car_has_lod_variants() {
        let root = crate::testkit::game_root();
        let car = Car::load(&root, "TAXI").unwrap();
        let at = |lod: u8| car.parts.iter().filter(|p| p.lods.is_none_or(|m| m >> lod & 1 != 0)).count();
        assert!(at(0) > 0 && at(1) > 0, "LOD A {} B {}", at(0), at(1));
        assert!(car.parts.iter().any(|p| p.lods == Some(2)));
        assert!(car.parts.iter().filter(|p| p.is_wheel).all(|p| p.lods.is_some_and(|m| m & 0b1000 == 0)));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn left_wheels_are_turned_not_mirrored() {
        let root = crate::testkit::game_root();
        let car = Car::load(&root, "PEUGOT").unwrap();
        let wheels: Vec<_> = car.parts.iter().filter(|p| p.is_wheel && p.lods.is_none_or(|m| m & 1 != 0)).collect();
        assert_eq!(wheels.len(), 4);
        for p in wheels {
            let (i, _) = p.follows_wheel.unwrap();
            let w = car.car_type.wheels[i];
            assert!(p.transform.determinant() > 0.0, "wheel {i} is a rotation");
            assert_eq!(p.transform.x_axis.x, if matches!(i, 0 | 3) { -1.0 } else { 1.0 });
            assert!((p.transform.w_axis.y - w.face).abs() < 1e-5);
        }
    }

    /// Rachel's car (DDAY_PLAYER_CAR, performance level 2): 0x5A64B0
    /// fits brake parts 38, 39, 40, then 41 (which replaces 40) and 42, so
    /// 0x6399E0 puts BRAKE STYLE02 (level 2, part 0x29) on slot 0x1B and
    /// all four brakes draw it.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn rachels_brakes_come_from_the_performance_parts() {
        let root = crate::testkit::game_root();
        let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let raw = crate::formats::jdlz::decompress(&std::fs::read(root.join("GLOBAL/GlobalB.lzc")).unwrap()).unwrap();
        let career = bundle::chunks(&raw, 0).find(|c| c.id == 0x8003_4A10).unwrap();
        let packages = crate::performance::parse(career.child(0x34A15).unwrap().body);
        let preset = parse_presets(&globalb).into_iter().find(|p| p.name == "DDAY_PLAYER_CAR").unwrap();
        let perf = preset.perf(&packages);
        assert!(perf.contains(&0x29) && !perf.contains(&0x28), "{perf:?}");
        let db = PartDb::load(&globalb).unwrap();
        let fitted = perf_visual_parts(&preset.installed(), &db, &preset.car, &packages, &|id| perf.contains(&id));
        assert_eq!(fitted.get(&0x1B), Some(&0xC657_8696), "BRAKE STYLE02");
        let car =
            Car::load_upgraded(&root, &preset.car, &preset.installed(), &packages, &|id| perf.contains(&id)).unwrap();
        let brakes = car.parts.iter().filter(|p| car.solids[p.solid].name == "BRAKE_STYLE02_A").count();
        assert_eq!(brakes, 4, "four wheels (0x620AF0 in game and in the showroom)");
        // Her performance parts include 0x24 (Adjustable Camber Plates)
        // but not 0x23: v = 0.5, 637 binary degrees (3.499°).
        assert!(perf.contains(&0x24) && !perf.contains(&0x23));
        assert!((car.camber.to_degrees() - 637.0 * 360.0 / 65536.0).abs() < 1e-4, "{}", car.camber.to_degrees());
    }

    /// The showroom's FE car takes its own physics object's record
    /// (0x4A6670 and 0x5F6830 through FE car +0x20 → 0x5B5360(1) →
    /// 0x5B4DD0), the upgraded one: Rachel's suspension lowers the body, so
    /// the wheel centres sit at z 0.18557 in the car (the main menu's
    /// apitrace brake origin) where the stock record has 0.17072.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn showroom_wheels_take_the_upgraded_record() {
        let root = crate::testkit::game_root();
        let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let raw = crate::formats::jdlz::decompress(&std::fs::read(root.join("GLOBAL/GlobalB.lzc")).unwrap()).unwrap();
        let career = bundle::chunks(&raw, 0).find(|c| c.id == 0x8003_4A10).unwrap();
        let packages = crate::performance::parse(career.child(0x34A15).unwrap().body);
        let preset = parse_presets(&globalb).into_iter().find(|p| p.name == "DDAY_PLAYER_CAR").unwrap();
        let perf = preset.perf(&packages);
        let stock = parse_car_type_tuned(&globalb, &preset.car, &[], &|_| false, None).unwrap();
        let upgraded = parse_car_type_tuned(&globalb, &preset.car, &packages, &|id| perf.contains(&id), None).unwrap();
        assert!((stock.wheels[0].position.z - 0.17072).abs() < 1e-4);
        for w in &upgraded.wheels[..2] {
            assert!((w.position.z - 0.18557).abs() < 2e-4, "{}", w.position.z);
        }
    }

    /// Rachel's 350Z has turbo level 2 (under Wine, D-Day free roam:
    /// ride +0x2F0 float[4] = 1.98, 0x5995F0(4) = 2), so the shift and rev
    /// flame pops (`carfx_pops`, 0x5EBE10 / 0x5F5F5F) fire on her car; a
    /// stock record has 0 and never pops.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn rachels_turbo_level_fires_the_pops() {
        let root = crate::testkit::game_root();
        let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let raw = crate::formats::jdlz::decompress(&std::fs::read(root.join("GLOBAL/GlobalB.lzc")).unwrap()).unwrap();
        let career = bundle::chunks(&raw, 0).find(|c| c.id == 0x8003_4A10).unwrap();
        let packages = crate::performance::parse(career.child(0x34A15).unwrap().body);
        let preset = parse_presets(&globalb).into_iter().find(|p| p.name == "DDAY_PLAYER_CAR").unwrap();
        let perf = preset.perf(&packages);
        let stock = parse_car_type_tuned(&globalb, &preset.car, &[], &|_| false, None).unwrap();
        let upgraded = parse_car_type_tuned(&globalb, &preset.car, &packages, &|id| perf.contains(&id), None).unwrap();
        assert_eq!(stock.turbo_level, 0);
        assert_eq!(upgraded.turbo_level, 2);
    }

    /// The x-turn (camber) after the steer, against the original's main
    /// menu (apitrace, part matrices from WorldViewProj over the body's):
    /// the front-right brake's axle (model y) at (−0.5, 0.8596, −0.1055)
    /// after two turns, the front-right rim's at (−0.5, 0.8644, −0.0529)
    /// after one; the steer 30°.
    #[test]
    fn camber_turns_after_the_steer() {
        let a = 637.0 * std::f32::consts::TAU / 65536.0;
        let pose = WheelPose { steer: 30f32.to_radians(), ..Default::default() };
        let at = Vec3::new(1.3658, -0.8125, 0.1856);
        let brake = pose.matrix_tilted(at, false, -2.0 * a).transform_vector3(Vec3::Y);
        assert!((brake - Vec3::new(-0.5, 0.8596, -0.1055)).length() < 2e-3, "{brake}");
        let rim = pose.matrix_tilted(at, true, -a).transform_vector3(Vec3::Y);
        assert!((rim - Vec3::new(-0.5, 0.8644, -0.0529)).length() < 2e-3, "{rim}");
        // The left wheel (0) turns by +angle, its model mirrored in y.
        let left = pose.matrix_tilted(at, false, 2.0 * a).transform_vector3(-Vec3::Y);
        assert!((left - Vec3::new(0.5, -0.8596, -0.1055)).length() < 2e-3, "{left}");
    }

    /// An aftermarket rim's matrix is A·[R]·F·B (row vectors) in both
    /// wheel draws: C = A·R (0x617A1F / 0x627993), D = C·F (0x617E72 /
    /// 0x627DDF), E = D·B (0x618107 / 0x62807B). A scales y by
    /// width / 0.184 and moves it −width/2; B moves y by −(p ∓ width/2),
    /// p = −record +0x1C (0x61FE20); F the wheel frame with its y zeroed.
    /// So the rim face lands at +0x1C and the tyre runs s × the model's
    /// width inward, on Rachel's car (wide body and all) as on stock.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn aftermarket_rim_is_a_r_f_b() {
        let root = crate::testkit::game_root();
        let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let preset = parse_presets(&globalb).into_iter().find(|p| p.name == "DDAY_PLAYER_CAR").unwrap();
        let car = Car::load_with(&root, &preset.car, &preset.installed()).unwrap();
        let wheels: Vec<_> = car.parts.iter().filter(|p| p.is_wheel && p.lods.is_none_or(|m| m & 1 != 0)).collect();
        assert_eq!(wheels.len(), 4, "0x620AF0's placement, the showroom's too");
        for part in wheels {
            let (i, _) = part.follows_wheel.unwrap();
            let w = car.car_type.wheels[i];
            // In game the wide body (slot 6, level 4) moves the face to
            // record +0x20 (k = 1 at 0x62714A).
            let face = w.face_wide;
            let (s, h, p) = (w.width / 0.184, 0.5 * w.width, -face);
            // Column-vector form of the row-vector product A·R·F·B.
            let a = Mat4::from_translation(Vec3::new(0.0, -h, 0.0)) * Mat4::from_scale(Vec3::new(1.0, s, 1.0));
            let r = if matches!(i, 0 | 3) { Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0)) } else { Mat4::IDENTITY };
            let f = Mat4::from_translation(Vec3::new(w.position.x, 0.0, w.position.z));
            let b = Mat4::from_translation(Vec3::new(0.0, -(if p < 0.0 { p + h } else { p - h }), 0.0));
            let native = b * f * r * a;
            for y in [-0.01f32, 0.0, 0.19] {
                let v = Vec3::new(0.1, y, 0.2);
                assert!(
                    (native.transform_point3(v) - part.transform.transform_point3(v)).length() < 1e-5,
                    "wheel {i} y {y}"
                );
            }
            // The steer turns it about the model origin after A·R, which
            // B puts at the tyre's middle.
            assert!((part.pivot_y.unwrap() - b.w_axis.y).abs() < 1e-5);
        }
    }

    /// In game (0x620AF0) each brake is M·F·T (row vectors): M =
    /// diag(1, −1, 1) on the left wheels (0x62E0CA, 0x62FA9D), F the frame
    /// with its y zeroed, T moving y by −(p ± v) (esp+0x4D0, 0x627416…),
    /// v = s·B, B the rim model's MARKER_BRAKE y (0x6390A6), else w/2.
    /// The showroom (0x631239 → 0x620AF0) does the same.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn in_game_brakes_sit_at_the_marker() {
        let root = crate::testkit::game_root();
        let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let preset = parse_presets(&globalb).into_iter().find(|p| p.name == "DDAY_PLAYER_CAR").unwrap();
        let rachel = Car::load_with(&root, &preset.car, &preset.installed()).unwrap();
        let stock = Car::load(&root, "350Z").unwrap();
        // 350Z_KIT00_FRONT_WHEEL_A: B 0.0837, s 1; RACINGHART_STYLE07_20_26_A:
        // B 0.1091, s = width / 0.184; Rachel's wide body: face +0x20.
        for (car, b, wide) in [(&stock, 0.0837f32, false), (&rachel, 0.1091, true)] {
            let brakes: Vec<_> = car
                .parts
                .iter()
                .filter(|p| matches!(p.follows_wheel, Some((_, false))) && p.lods.is_none_or(|m| m & 1 != 0))
                .collect();
            assert_eq!(brakes.len(), 4);
            for part in brakes {
                let (i, _) = part.follows_wheel.unwrap();
                let w = car.car_type.wheels[i];
                let left = matches!(i, 0 | 3);
                assert_eq!(left, w.position.y > 0.0);
                assert_eq!(part.transform.y_axis.y, if left { -1.0 } else { 1.0 }, "diag(1, -1, 1) on the left");
                let y = part.transform.w_axis.y;
                assert_eq!(part.scene, Scene::Both, "the showroom draws them as in game (0x620AF0)");
                let face = if wide { w.face_wide } else { w.face };
                let width = if wide { w.width } else { 0.184 };
                let v = width / 0.184 * b;
                let p = -face;
                let want = if p < 0.0 { -(p + v) } else { -(p - v) };
                assert!((y - want).abs() < 1e-4, "wheel {i}: {y} vs {want}");
                assert_eq!(part.pivot_y, Some(y));
                // The original's main menu (apitrace): Rachel's front
                // brakes at |y| 0.8125.
                if wide && i < 2 {
                    assert!((y.abs() - 0.8125).abs() < 1e-3, "{y}");
                }
            }
        }
    }

    /// Traffic types have no DUMMY_SKIN slot (+0x88C = 0), so a traffic
    /// car keeps the shipped `<CAR>_SKIN1` (128² DXT1 in CARS/TEXTURES.BIN)
    /// that its geometry names; racers are composited.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn traffic_skin_is_the_shipped_skin1() {
        let root = crate::testkit::game_root();
        let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let types = parse_car_types(&globalb).unwrap();
        assert_eq!(types.iter().filter(|t| t.traffic).count(), 15);
        assert!(types.iter().all(|t| t.composited_skin != t.traffic));
        let car = Car::load(&root, "PICKUP").unwrap();
        let skin = &car.textures[&skin_hash("PICKUP")];
        assert_eq!((skin.width, skin.height, skin.format), (128, 128, crate::formats::texture::PixelFormat::Dxt1));
    }

    /// 0x625AE1 / 0x6243F4: Rachel's 350Z's spoiler (group 0, at the
    /// SPOILER marker) is drawn on the trunk's posed matrix, so it rises
    /// with the trunk lid; shut, it stays at its marker.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn spoiler_rides_on_the_open_trunk() {
        let root = crate::testkit::game_root();
        let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let preset = parse_presets(&globalb).into_iter().find(|p| p.name == "DDAY_PLAYER_CAR").unwrap();
        let car = Car::load_with(&root, &preset.car, &preset.installed()).unwrap();
        let spoiler =
            car.parts.iter().find(|p| car.solids[p.solid].hash == bin_hash("SPOILER_STYLE02_A")).expect("spoiler");
        let trunks: Vec<_> = car
            .parts
            .iter()
            .filter(|p| {
                p.anim.is_some_and(|a| a.0 == 1)
                    && p.transform == Mat4::IDENTITY
                    && car.solids[p.solid].hash != bin_hash("SPOILER_STYLE02_A")
            })
            .collect();
        assert!(!trunks.is_empty(), "the trunk is animated");
        let (channel, origin) = spoiler.anim.expect("the spoiler follows the trunk");
        assert_eq!(channel, 1);
        assert!(
            trunks.iter().any(|t| t.anim.unwrap().1 == origin && t.anim_group == spoiler.anim_group),
            "on the trunk's own pose"
        );
        let anims = car.animations.as_ref().expect("350Z part animations");
        let mut c = crate::part_anim::Channels::default();
        assert!(c.poses(Some(anims), 0.0)[1][0].is_none(), "shut: no pose, the spoiler stays at its marker");
        c.request(Some(anims), &car.part_styles, 1, true, 1.0, 0.0);
        let pose = c.poses(Some(anims), 30.0)[1][spoiler.anim_group as usize].expect("open trunk");
        let posed = pose.part_matrix(origin) * spoiler.transform;
        let rest = spoiler.transform.w_axis.truncate();
        let moved = posed.w_axis.truncate();
        // The same point carried by the trunk's own matrix.
        assert!((pose.part_matrix(origin).transform_point3(rest) - moved).length() < 1e-5);
        assert!(moved.z - rest.z > 0.2, "the spoiler rises with the lid: {rest} → {moved}");
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn split_hood_draws_its_black_undersides_while_open() {
        // COBRA on the RX7 (hood.trace frame 20900, the maintainer's private notes
        // §L3.1): the hood's HOODUNDER (0x61BCE0) puts KIT24's
        // HOOD_UNDER_LEFT / _RIGHT (55 / 53 triangles, PLAINNOTHING: c18 =
        // c19 = 0, black) in slot 0x19; the generic draw poses them with the
        // hood's halves (groups 0 / 1) and draws them only while the hood is
        // not shut (0x6233D9).
        let root = crate::testkit::game_root();
        let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let db = crate::formats::parts::PartDb::load(&globalb).unwrap();
        let mut after = None;
        let cobra = std::iter::from_fn(|| {
            let p = db.find_after("RX7", 9, after)?;
            after = Some(p);
            Some(p)
        })
        .find(|p| db.string(p.name as u32).as_deref() == Some("COBRA"))
        .expect("COBRA");
        let installed = with_derived_slots(&std::collections::BTreeMap::from([(9u8, cobra.hash)]), &db, "RX7");
        assert!(installed.contains_key(&0x19), "the derived hood underside");
        let car = Car::load_with(&root, "RX7", &installed).unwrap();
        let under: Vec<_> = car.parts.iter().filter(|p| car.solids[p.solid].name.contains("HOOD_UNDER_")).collect();
        let names: Vec<&str> = under.iter().map(|p| car.solids[p.solid].name.as_str()).collect();
        assert_eq!(under.len(), 2, "both halves' undersides: {names:?}");
        for p in &under {
            let left = car.solids[p.solid].name.contains("_LEFT");
            assert!(p.hood_gate, "drawn only with the hood open");
            assert_eq!(p.anim.map(|a| a.0), Some(0), "posed by the hood channel");
            assert_eq!(p.anim_group, if left { 0 } else { 1 }, "the half's own pose");
        }
        let halves: Vec<_> = car.parts.iter().filter(|p| p.anim.is_some_and(|a| a.0 == 0) && !p.hood_gate).collect();
        assert!(
            halves.iter().any(|p| p.anim_group == 0) && halves.iter().any(|p| p.anim_group == 1),
            "the hood's two halves"
        );
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn tiburon_wheel_cap_edge_passes_the_alpha_test_with_wrap() {
        // The TIBURON wheel line is native:
        // the wheel's inner cap (PLAINNOTHING, flags 0x4080, black) is a
        // quad on u 0.752…1.0 of TIBURON_TIRE's alpha disc; car.efx samples
        // it WRAP (apitrace, every car draw) with the alpha test 95, so at
        // u = 1.0 bilinear takes half of the opaque column 0 and the edge
        // passes (ps: alpha = 2·t1.a·v0.a, v0.a = 0.5 for the material's
        // DiffuseMin.w 1 / DiffuseRange.w 0). The original's car select
        // shows the edge at the predicted pixels (column 751 of tib.png).
        let root = crate::testkit::game_root();
        let car = Car::load(&root, "TIBURON").unwrap();
        let wheel = car.solids.iter().find(|s| s.name == "TIBURON_KIT00_FRONT_WHEEL_A").expect("the wheel");
        let cap = wheel.materials.iter().find(|m| m.flags == 0x4080).expect("the cap range");
        let tris: Vec<_> = wheel.indices[cap.first_index as usize..(cap.first_index + cap.index_count) as usize]
            .iter()
            .map(|&i| wheel.vertices[i as usize])
            .collect();
        let quad: Vec<_> = tris.iter().filter(|v| v.uv[0] > 0.7).collect();
        assert_eq!(quad.len(), 6, "the cap quad");
        assert!(quad.iter().any(|v| v.uv[0] == 1.0), "the quad's edge at u = 1.0");
        let t = &car.textures[&wheel.textures[cap.texture_slot as usize]];
        assert_eq!((t.name.as_str(), t.width, t.height, t.alpha_test), ("TIBURON_TIRE", 128, 128, true));
        let rgba = t.decode_rgba();
        let alpha = |x: usize, y: usize| f32::from(rgba[(y * 128 + x) * 4 + 3]) / 255.0;
        let ref_a = 95.0 / 255.0;
        for y in 33..63 {
            // u = 1.0 → texel 127.5: half of column 127 and half of column 0.
            let wrap = 0.5 * alpha(127, y) + 0.5 * alpha(0, y);
            let clamp = alpha(127, y);
            assert!(wrap > ref_a && clamp <= ref_a, "row {y}: wrap {wrap} clamp {clamp}");
        }
        let s = crate::render::display::texture_sampler("car", crate::render::RenderOptions::in_force().anisotropy);
        assert_eq!(
            (s.address_mode_u, s.address_mode_v),
            (wgpu::AddressMode::Repeat, wgpu::AddressMode::Repeat),
            "car.efx's sampler 1 is WRAP"
        );
    }

    #[test]
    fn rear_window_decal_and_hummer_headlights_take_their_channels() {
        // 0x623473: slot 0x36 on a 0x60C900 type (240SX = 3) takes the
        // trunk, not on the RX8 (21); slot 0xE on the HUMMER (8) the hood,
        // as group 1 with a VERTSPLIT hood.
        assert_eq!(animated_channel(0x36, 0, -1, 3, false), (Some(1), 0));
        assert_eq!(animated_channel(0x36, 1, -1, 21, false), (None, 1));
        assert_eq!(animated_channel(0x0E, 0, -1, 8, true), (Some(0), 1));
        assert_eq!(animated_channel(0x0E, 0, -1, 8, false), (Some(0), 0));
        assert_eq!(animated_channel(0x0E, 1, -1, 22, true), (None, 1));
        assert_eq!(animated_channel(0x0A, 0, 1, 22, false), (Some(1), 0));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn dday_car_has_its_shared_parts() {
        let root = crate::testkit::game_root();
        let globalb = std::fs::read(root.join("GLOBAL/GLOBALB.BUN")).unwrap();
        let preset = parse_presets(&globalb).into_iter().find(|p| p.name == "DDAY_PLAYER_CAR").unwrap();
        let car = Car::load_with(&root, &preset.car, &preset.installed()).unwrap();
        let rim = bin_hash("RACINGHART_STYLE07_20_26_A");
        // Four, at the wide body's +0x20 (0x62714A) in game and in the
        // showroom alike (both 0x620AF0).
        assert_eq!(
            car.parts
                .iter()
                .filter(|p| p.is_wheel && p.scene == Scene::Both && car.solids[p.solid].hash == rim)
                .count(),
            4
        );
        let spoiler =
            car.parts.iter().find(|p| car.solids[p.solid].hash == bin_hash("SPOILER_STYLE02_A")).expect("spoiler");
        assert!((spoiler.transform.w_axis.x - -1.933).abs() < 0.01, "at the SPOILER marker");
        // The Mantis wide body (slot 6, level 4) hides the KIT00 bumpers,
        // body and skirt (0x61BCE0), and the exhaust tips sit at the wide
        // body's LEFT_EXHAUST / RIGHT_EXHAUST markers (0x639843).
        assert!(!car.parts.iter().any(|p| car.solids[p.solid].hash == bin_hash("350Z_KIT00_REAR_BUMPER_A")));
        assert!(car.parts.iter().any(|p| car.solids[p.solid].hash == bin_hash("350Z_KITW02_BODY_A")));
        let exhaust = bin_hash("EXHAUST_STYLE08_LEVEL1_A");
        let tips: Vec<_> =
            car.parts.iter().filter(|p| car.solids[p.solid].hash == exhaust).map(|p| p.transform.w_axis).collect();
        assert_eq!(tips.len(), 2);
        assert!(tips.iter().all(|t| (t.x - -1.985).abs() < 0.01 && (t.y.abs() - 0.393).abs() < 0.01));
        // The rear plate (slot 0x21's LICENSE_PLATE_STYLE01_A from
        // CARS\PLATES) at the wide body's REAR_LICENSE_PLATE marker, facing
        // back, with GLOBALB's LICENSE_PLATE texture.
        let plate = bin_hash("LICENSE_PLATE_STYLE01_A");
        let plates: Vec<_> = car.parts.iter().filter(|p| car.solids[p.solid].hash == plate).collect();
        assert_eq!(plates.len(), 1);
        assert!(plates[0].transform.w_axis.x < -2.0 && plates[0].transform.w_axis.y.abs() < 0.01);
        assert!(plates[0].transform.transform_vector3(Vec3::NEG_X).x < -0.9, "the plate's face looks back");
        assert!(car.textures.contains_key(&bin_hash("LICENSE_PLATE")));
        // The skin's mirror marker texels (white, PAINT_MIRROR 0x4C) take
        // the body paint when no mirror paint is fitted.
        let skin = &car.textures[&skin_hash(&preset.car)];
        let i = (509 * skin.width as usize + 485) * 4;
        assert_eq!(skin.data[i..i + 4], [20, 78, 14, 0x80], "METAL_L2_COLOR10, GLOSS in the alpha");
        // The fill's alpha is the paint's GLOSS (0x611F20); the panel
        // lines are the wide body's (0x612B80).
        assert_eq!(skin.data[3], 0x80);
        assert_eq!(car.doorline, "350Z_DOORLINE_WIDEBODY");
        // VOLTAGE headlights and REAPER brakelights name their own
        // textures (attribute 0x10C98090, 0x631E30); the centre brake
        // light is the car's CENTRE_BRAKELIGHT_OFF.
        assert_eq!(car.texture_remap[&bin_hash("HEADLIGHT_LEFT")], bin_hash("350Z_STYLE02_HEADLIGHT_OFF"));
        assert_eq!(car.texture_remap[&bin_hash("HEADLIGHT_GLASS_RIGHT")], bin_hash("350Z_STYLE02_HEADLIGHT_GLASS_OFF"));
        assert_eq!(car.texture_remap[&bin_hash("BRAKELIGHT_RIGHT")], bin_hash("350Z_STYLE04_BRAKELIGHT_OFF"));
        assert_eq!(car.texture_remap[&bin_hash("BRAKELIGHT_GLASS_CENTRE")], bin_hash("350Z_CENTRE_BRAKELIGHT_OFF"));
        let stock = Car::load(&root, &preset.car).unwrap();
        assert_eq!(stock.doorline, "350Z_DOORLINE");
        assert_eq!(stock.texture_remap[&bin_hash("HEADLIGHT_LEFT")], bin_hash("350Z_KIT00_HEADLIGHT_OFF"));
        assert_eq!(stock.texture_remap[&bin_hash("BRAKELIGHT_LEFT")], bin_hash("350Z_KIT00_BRAKELIGHT_OFF"));
    }
}

#[cfg(test)]
mod shading_trace_tests {
    use super::*;

    /// The stock 240SX on the Quick Race car select: every car.efx draw of
    /// the original's frame (apitrace over DXVK, 2026-11-04; VS c18..c23
    /// and PS c0/c1, distinct sets in internal
    /// evidence) has the constants the clone gives one of the
    /// car's front-end LOD A draws (0x5BD4B0 on the part's light material
    /// and the material record's flags), and every such clone draw's set
    /// occurs in the frame. Paint METPAINTSILVER, rims MAGSILVER, glass WINDSHIELD /
    /// WINDOWMASK; DULLPLASTIC and LICENSEPLATE also without the env term
    /// (mesh flag 0x100 clear). See docs/notes/graphics/car-look.md "Car paint, reflections and rims against the original".
    #[test]
    #[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]
    fn stock_240sx_constants_match_the_original_frame() {
        let root = crate::testkit::game_root();
        let trace: Vec<Vec<f32>> =
            crate::testkit::private_str("docs/evidence/car-shading/fe-240sx-car-efx.txt")
                .lines()
                .map(|l| l.split('#').next().unwrap().split_whitespace().skip(1).map(|v| v.parse().unwrap()).collect())
                .collect();
        assert_eq!(trace.len(), 21);
        let car = Car::load(&root, "240SX").unwrap();
        let mut clone: Vec<(Vec<f32>, String)> = Vec::new();
        for part in car.parts.iter().filter(|p| p.scene != Scene::InGame && p.lods.is_none_or(|m| m & 1 != 0)) {
            let solid = &car.solids[part.solid];
            if solid.normals.len() != solid.vertices.len() {
                continue;
            }
            for m in &solid.materials {
                let l = solid
                    .light_materials
                    .get(m.light_material as usize)
                    .and_then(|h| car.part_light_material_for(part, *h))
                    .or_else(|| car.light_material_for(bin_hash("DEFAULT")))
                    .unwrap();
                let c = l.constants(m.flags);
                let mut v: Vec<f32> = c.diffuse_min.iter().chain(&c.diffuse_range).copied().collect();
                v.extend_from_slice(&c.specular_min[..3]);
                v.extend_from_slice(&c.specular_range[..3]);
                v.extend([c.specular_power, c.envmap_power]);
                v.extend_from_slice(&c.envmap_min[..3]);
                v.extend_from_slice(&c.envmap_range[..3]);
                clone.push((v, l.name.clone()));
            }
        }
        let same = |a: &[f32], b: &[f32]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 2e-5);
        for t in &trace {
            assert!(clone.iter().any(|(c, _)| same(c, t)), "original draw {t:?} not among the clone's");
        }
        // The other way round only per material: the enumeration holds
        // parts the frame does not draw (the engine under the shut hood,
        // the unfitted decal and roof variants).
        for (c, name) in &clone {
            if name != "MAGGUNMETAL" {
                assert!(trace.iter().any(|t| same(c, t)), "clone draw {name} {c:?} not in the original frame");
            }
        }
        // The DULLPLASTIC ranges of BASE_A and TRUNK_A (flags 0x4080 in
        // 0xC180 meshes) without the env term, the others with it.
        let dull: Vec<_> = clone.iter().filter(|(_, n)| n == "DULLPLASTIC").map(|(c, _)| c[16] == 0.0).collect();
        assert!(dull.iter().any(|&off| off) && dull.iter().any(|&off| !off));
        assert!(clone.iter().any(|(_, n)| n == "METPAINTSILVER") && clone.iter().any(|(_, n)| n == "MAGSILVER"));
    }
}
