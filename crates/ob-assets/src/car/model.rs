//! Model types, slot tables and hull of a loaded car.

use super::*;

/// One drawn piece of the car.
#[derive(Clone, Debug, Default)]
pub struct CarPart {
    pub solid: usize,
    /// A wheel (rim) model: the rim draw (0x61797F, 0x627784) puts the
    /// rims' paint in place of MAGSILVER, MAGCHROME and 0x12049251. Its
    /// `PLAINNOTHING` meshes (the black barrel / inner backing behind the
    /// spokes) are drawn like the rest (no code names that material).
    pub is_wheel: bool,
    /// For wheels and brakes: which wheel (table order) it moves with, and
    /// whether it turns with the wheel's roll (brakes don't).
    pub follows_wheel: Option<(usize, bool)>,
    /// For a part that follows a wheel: the y of the point its pose turns
    /// about (the steer), where the model's origin lands after the wheel
    /// draw's face shift (0x620AF0 / 0x6162F0 multiply the frame in before
    /// it); `None` turns it about the wheel centre (record +0x04).
    pub pivot_y: Option<f32>,
    /// Part space → car space.
    pub transform: Mat4,
    /// The factor on the light context's harmonic (0x620AF0, 0x6304D4…):
    /// 1 for the main context; the trunk audio's group 0 (slot 0x22, render
    /// info +0x21B4) takes a copy scaled by the trunk animation's value
    /// × 0.9, 0 with the trunk shut. See [`AUDIO`].
    pub harmonic_scale: f32,
    /// Which scenes draw it: the trunk audio components are the fitted
    /// parts' models in the front end and placeholders in game (0x630C34).
    pub scene: Scene,
    /// Whether it takes the audio paint ([`Car::audio_material`]) in place
    /// of a paint reference (0x48D860 at 0x630908 and 0x630CC3).
    pub audio_paint: AudioPaint,
    /// An aftermarket mirror model (slot 0x20 above level 0): the mirror
    /// draw (0x6269D6 / 0x626F4B) puts render info +0x3A8 (the mirror
    /// paint, [`Car::mirror_material`]) or, without one, the body paint
    /// +0x384 (0x622F4B…0x622F5C) in place of light material 0x6BFA56DA.
    pub mirror_paint: bool,
    /// A slot paint drawn in place of one light material reference (0x48D860
    /// in the part's own draw): the roof scoop (slot 7, render info +0x1888,
    /// 0x6262D5 / 0x6263F5) and the spoiler (slot 0xC, +0x1928, 0x626367)
    /// put their paint (+0x3A4 / +0x3A0) in place of CARSKIN (0xD6D6080A),
    /// the exhaust (slot 0x10, +0x19A8, 0x624C2C) its paint (+0x394) in
    /// place of EXHAUST_TIP (0x3B3A1F8D). `None` without the paint (the
    /// spoiler and scoop then take the body paint +0x384, 0x622EF8…0x622F44,
    /// as CARSKIN does anyway; the exhaust keeps its own material).
    pub slot_paint: Option<(u32, u32)>,
    /// A spinner (the rim part's second model group, render info +0x1B58 /
    /// +0x1B78): it turns by the wheel's spinner angle
    /// ([`WheelPose::spinner`]) instead of its roll.
    pub spinner: bool,
    /// Moved by a part animation channel (0 hood, 1 trunk, 2 / 3 left /
    /// right door), with its solid's header +0x70 (0x488E80): see
    /// [`ANIMATED`] and [`crate::part_anim::ChannelPose::part_matrix`].
    pub anim: Option<(u8, Vec3)>,
    /// The model's group in its slot (render info +0x17A8 + 0x20·slot +
    /// 0x10·group): which half of a split hood it is (0x434190's fourth
    /// argument, the draw's group at 0x623540).
    pub anim_group: u8,
    /// Drawn only while channel 0 (the hood) is not shut (the engine,
    /// 0x620B22: 0x433CD0(…, 0) ≠ 1).
    pub hood_gate: bool,
    /// The light context's harmonic scaled by a channel's value × factor
    /// in place of `harmonic_scale` (the engine's group 0: hood × 0.6;
    /// the trunk audio's group 0: trunk × 0.9).
    pub harmonic_anim: Option<(u8, f32)>,
    /// With `harmonic_anim`, for a level-3 engine / trunk audio part
    /// (ride +0x5C4 / +0x618, part +5 & 0xE0 = 0x60): the factor k on the
    /// neon colour (render info +0x364 / +0x37C, R, G, B, intensity, each
    /// ÷ 256) that the context's c0 (HarmonicCoeff[0].rgb) gains as
    /// R·intensity·k·value (0x630581…0x630641: k = 0.6; 0x6307A3…0x63085B:
    /// k = 1.5). See [`crate::render::car::CarRenderer::set_part_glow`].
    pub harmonic_glow: Option<f32>,
    /// The carbon-fibre texture table the part's model is bound to
    /// (0x634B0B…0x634E69): see [`Carbon`]. A `Full` part also draws its
    /// `CARSKIN` references with the `CARBONFIBER` light material (render
    /// info +0x388; the generic part draw, 0x6231B3): see
    /// [`Car::part_light_material_for`].
    pub carbon: Carbon,
    /// A model of the generic part draw (slots 0–0x3E it draws itself) on a
    /// car with KIT_CARBON fitted (render info +0x3B8): its 0xA7366AE6
    /// light-material references take `CARBONFIBER` (0x6231E1…0x623456)
    /// instead of the paint.
    pub carbon_kit: bool,
    /// A decal location model (slots 0x34–0x3E): the eight textures its
    /// `DUMMY_DECAL1`…`8` take from its block of render info +0x13E8
    /// (0x61FF50; bound at 0x634AEC). See [`decal_block`].
    pub decals: Option<[u32; 8]>,
    /// Triangles (numbers, index / 3) of the model the draw leaves out:
    /// Fix Original Model Errors only ([`crate::model_fixes::hidden_triangles`],
    /// the PEUGOT106 antenna under a roof scoop). Empty by default.
    pub hidden_tris: Vec<u32>,
    /// The car LODs (bit n: LOD n, A = 0) this part draws at: each LOD is
    /// its own part with that LOD's model (render info +0x17A8 + 0x20·slot
    /// + 0x10·group + 4·LOD; a missing model draws nothing, 0x6230C6).
    /// None: every LOD (parts outside the slot table).
    pub lods: Option<u8>,
}

/// Which texture-remap table a part model draws with (0x634B0B…0x634E69):
/// the main one (+0x638), +0xAC8 (`Full`: 0xA7366AE6 and 0x3C84D757 →
/// `CARBONFIBRE`) or +0xF58 (`Partial`: 0xA7366AE6 only). 0x636850 builds
/// both from the main table, the skin keys (entries 0–2) all on the skin
/// (+0x2D8), entry 3 (0xA7366AE6) → `CARBONFIBRE` (0x7A8528, a GLOBALB
/// 512² DXT1) in both and entry 4 (0x3C84D757) → `CARBONFIBRE` in +0xAC8
/// only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Carbon {
    #[default]
    None,
    Full,
    Partial,
}

/// The scenes a part is drawn in (0x8654A4 = 3 is the front end).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scene {
    #[default]
    Both,
    FrontEnd,
    InGame,
}

/// A part whose model swaps a paint light material for render info +0x39C
/// (the audio paint, slot 0x48's material with a level-3 audio part, else
/// the body paint +0x384; 0x63909B).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AudioPaint {
    #[default]
    None,
    /// The trunk audio (slot 0x22): 0x5BE72F1C → +0x39C (0x630908).
    TrunkAudio,
    /// A trunk audio component: CARSKIN (0xD6D6080A) → +0x39C (0x630CBC).
    Component,
}

/// The trunk audio components' markers on the audio part's group 0 LOD A
/// model (table 0x8A4324, filled at 0x77F0C0: `MARKER_AUDIO_COMP_0` …
/// `_11`; looked up at 0x6350DC into render info +0x2184).
pub const AUDIO_COMP_MARKERS: usize = 12;
/// The component slots 0x23–0x2E (ride +0x61C + 4i; models at render info
/// +0x2274 + 0xC0·i, two groups of four LODs, 0x635440).
pub const AUDIO_COMP_FIRST: usize = 0x23;
/// Component i's size (0x630C42: 0 small, 1 medium, 2 large), which picks
/// its in-game placeholder.
pub const AUDIO_COMP_SIZE: [usize; AUDIO_COMP_MARKERS] = [0, 0, 1, 1, 2, 2, 0, 0, 0, 0, 1, 1];
/// The in-game placeholders (render info +0x2B74 + 0x18·(2·size + paint),
/// names 0x8A44FC.. from 0x7A8B38 …, loaded at 0x635729).
pub const AUDIO_COMP_EMPTY: [[&str; 2]; 3] = [
    ["AUDIO_COMP_EMPTY_SMALL_A", "AUDIO_COMP_EMPTY_SMALL_PAINT_A"],
    ["AUDIO_COMP_EMPTY_MEDIUM_A", "AUDIO_COMP_EMPTY_MEDIUM_PAINT_A"],
    ["AUDIO_COMP_EMPTY_LARGE_A", "AUDIO_COMP_EMPTY_LARGE_PAINT_A"],
];

/// The engine slot (render info +0x1948, slot 0xD's model groups): 0x620AF0
/// draws it only while animation channel 0 (the hood, render info +0x2D44
/// → 0x433CD0(…, 0) ≠ 1) runs, group 0 with the context's harmonic × the
/// channel's value (0x434610) × 0.6 plus, for a level-3 part, the engine
/// colour bytes +0x364..+0x367 (R, G, B, intensity, each /256) × that
/// factor on c0; group 1 with the main context.
pub const ENGINE: usize = 0x0D;
/// The hood's underside (slot 0x19: `<CAR>_KITnn_HOOD_UNDER`, or
/// `_HOOD_UNDER_LEFT` / `_RIGHT` as groups 0 / 1 for a split hood, the
/// part the hood's HOODUNDER attribute names, 0x61BCE0). The generic part
/// draw takes it, posed by the hood channel (chunk 0x34608), not by its
/// ride +0x838 flag but while the hood is not shut (0x623143 → case
/// 0x6233D9: [esp+0x30C] = 0x433CD0(anim, 0) ≠ 1, 0x620B56); its
/// PLAINNOTHING material draws it black (frame 20900 of a COBRA trace:
/// calls 9078 / 9109, the halves' undersides, c18 = c19 = 0).
pub const HOOD_UNDER: usize = 0x19;
/// The trunk audio slot (ride +0x618; models at render info +0x21B4,
/// 0x635201): group 0 lit by the context's harmonic × channel 1's (the
/// trunk's) value × 0.9, plus for a level-3 part the audio colour bytes
/// +0x37C..+0x37F × the value × 1.5; group 1 by the main context. With no
/// animation running (or none, +0x2D44 = 0) the value is 0.
pub const AUDIO: usize = 0x22;

/// The car render's purge set-up (0x634810 on the ride's slot 0xA2 part,
/// ride +0x818): kind 13 (render +0x4C0) takes the emitters of the group
/// its HOODEMITTER attribute names, kind 14 (+0x4C8) WHEELEMITTER's
/// (0x48B8B0: clones of the loaded emitters of that description group),
/// each kind's emitters recoloured by HOODHUE / HOODSAT / HOODLUM and
/// TIREHUE / TIRESAT / TIRELUM (0x48B920, into a colour slot of
/// 0x8A3F60). The markers: group 28 (render +0x628) gets the slot 0
/// LOD A model's (render +0x17A8) markers 0xE3DE69DB (HOODLEFT) and
/// 0x601A6CCE (HOODRIGHT, y mirrored); group 29 (+0x630) the points
/// WHEELLEFT (wheel 0's centre, x + its radius, y mirrored) and WHEELRIGHT
/// (wheel 1's, x + its radius) when those attributes are not 0. The
/// emitters' own group (description +0x5C) picks 28 (hood smokes) or 29
/// (wheel smokes) in 0x615CD0.
#[derive(Clone, Debug, Default)]
pub struct PurgeFx {
    /// HOODEMITTER, WHEELEMITTER (description group hashes; 0 none).
    pub groups: [u32; 2],
    /// (hue, saturation, luminance) shifts for kinds 13 and 14.
    pub hsl: [[f32; 3]; 2],
    /// Marker groups 28 and 29: car-space points with their mirror
    /// flags (x, y, z; 0x615CD0 negates that component of the emitter's
    /// axes).
    pub markers: [Vec<(Vec3, [bool; 3])>; 2],
}

/// The purge's hood markers in the slot 0 model (0x6363B3 / 0x6364A3).
pub const PURGE_HOOD_LEFT: u32 = 0xE3DE_69DB;
pub const PURGE_HOOD_RIGHT: u32 = 0x601A_6CCE;

/// The slots whose models make up the car's box (0x634810, jump table
/// 0x63681C over slots 0–0x33): each fitted slot's model groups 0 and 1,
/// LOD A (render info +0x17A8 + (slot·2 + group)·0x10), by the solid's
/// own header box (0x48D7E0: +0x20 / +0x30; zeros when the model has no
/// solid). The union starts empty (0x43D510) and is grown by 0.1 m
/// (0x784358) on every side.
pub const SHADOW_BOX_SLOTS: [usize; 17] = [0, 1, 2, 3, 4, 5, 6, 7, 9, 0xA, 0xB, 0xC, 0x2F, 0x30, 0x31, 0x32, 0x33];

/// A hull's reach (0x5AE530): its farthest point from the car's origin,
/// and its highest and lowest z, each taken from 0 (see
/// [`Car::hull_reach`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HullReach {
    pub radius: f32,
    pub top: f32,
    pub bottom: f32,
}

impl HullReach {
    /// The reach of `hull`, folded in the hull's order (bit-identical to
    /// the per-call folds it replaces).
    pub fn of(hull: &[Vec3]) -> HullReach {
        Self::about(hull, Vec3::ZERO)
    }

    /// The reach of `hull` about `centre`: 0x5AE530 folds the shape's
    /// vertices as 0x5A1980 gives them, less the shape's offset when it
    /// has one (0x5AEC30 sets the offset before it calls 0x5AE530).
    pub fn about(hull: &[Vec3], centre: Vec3) -> HullReach {
        let mut r = HullReach::default();
        for p in hull {
            let p = *p - centre;
            r.radius = r.radius.max(p.length());
            r.top = r.top.max(p.z);
            r.bottom = r.bottom.min(p.z);
        }
        r
    }
}

impl Car {
    /// The offset the hull's shape carries (shape +0x40 with +0x3E set):
    /// the car's centre of mass for a `_CV` hull (0x5AEC30 writes the
    /// car's +0x20 record's, 0x5B5060 the body's vtable +0x1C = 0x5ED410
    /// before each detection), none for the [`DEFAULT_HULL`] box (0x5AED76
    /// makes a box shape, type 3, which has no offset). The distance query
    /// places its fresh supports R·this off ([`crate::narrow::Raw`]).
    pub fn hull_offset(&self) -> Vec3 {
        let (lo, hi) = DEFAULT_HULL;
        let boxed = self.hull.len() == 8
            && self.hull.iter().all(|p| {
                (0..3).all(|k| p[k] == lo[k] || p[k] == hi[k])
            });
        if boxed || self.hull.is_empty() { Vec3::ZERO } else { Vec3::from(self.car_type.handling.centre_of_mass) }
    }

    /// The collision body's reach (0x5AE530, body +0x1C radius, +0xD0 /
    /// +0xD4 heights): about the centre of mass for a `_CV` hull (the
    /// shape's offset, [`Car::hull_offset`]), about the car's origin for
    /// the default box. The body's box is its position (the centre of
    /// mass, vtable +0x08 = 0x593B50) ± this radius in x/y (0x5A9DF0).
    pub fn body_reach(&self) -> HullReach {
        HullReach::about(&self.hull, self.hull_offset())
    }

    /// The hull's points in the world for a body transform: the rotation
    /// and the car's origin. A `_CV` hull's points are rebuilt as the
    /// original's query rebuilds them (0x5A1980: the vertex less the shape's
    /// offset, the centre of mass; then the body transform about the centre
    /// of mass, [`crate::narrow::body_point`]); the default box, which has
    /// no offset, as origin + R·v.
    pub fn hull_points(&self, origin: Vec3, rot: glam::Mat3) -> Vec<Vec3> {
        let c = self.hull_offset();
        if c == Vec3::ZERO {
            return self.hull.iter().map(|p| origin + rot * *p).collect();
        }
        let com = origin + rot * Vec3::from(self.car_type.handling.centre_of_mass);
        self.hull.iter().map(|p| crate::narrow::body_point(rot, com, *p - c)).collect()
    }

    /// The hull's reach, worked out at load. `hull` is public; a caller
    /// that changes it after load breaks this (checked in debug builds).
    pub fn hull_reach(&self) -> HullReach {
        debug_assert_eq!(self.hull_reach, HullReach::of(&self.hull), "Car::hull changed after load");
        self.hull_reach
    }
}

/// The car's convex hull and its vertex links: the `<CAR>_CV` volume in
/// its geometry, else the [`DEFAULT_HULL`] box (no links: a box shape's
/// support is not the hull's climb).
pub fn collision_hull(geometry: &[u8], name: &str) -> (Vec<Vec3>, crate::formats::collision::HullLinks) {
    let cv = format!("{name}_CV");
    let volume = bundle::chunks(geometry, 0)
        .filter(|c| c.id == crate::formats::collision::COLLISION_VOLUMES)
        .flat_map(crate::formats::collision::parse_collision_volumes)
        .find(|v| v.name == cv && !v.vertices.is_empty());
    if let Some(v) = volume {
        return (v.vertices, v.links);
    }
    let (lo, hi) = DEFAULT_HULL;
    let hull = (0..8)
        .map(|i| {
            Vec3::new(
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            )
        })
        .collect();
    (hull, Default::default())
}

/// The box the game gives a car without a `_CV` volume (0x5AED76):
/// x −2.217..2.165, y ±0.971, z −0.2..0.624.
pub const DEFAULT_HULL: ([f32; 3], [f32; 3]) = ([-2.217, -0.971, -0.2], [2.165, 0.971, 0.624]);

/// The slots a stock car fills (0x637050 skips 0x40–0x43, 0x61–0x98 and
/// does not store 0x19; slot 0x19, HOOD_UNDER, is filled by 0x61BCE0 from
/// the hood and drawn while the hood is open, [`HOOD_UNDER`]), less those
/// that are not body meshes: the paint
/// (0x3F, set from car type +0x84C), the license plate (0x21, drawn at
/// its markers below), the decal
/// placements (0x34–0x48, drawn only with a decal on them) and the
/// collision volume (0xA9).
pub fn draws_slot(slot: usize) -> bool {
    !((0x40..=0x43).contains(&slot)
        || (0x61..=0x98).contains(&slot)
        || slot == 0x3F
        || slot == 0x21
        || (0x3F..=0x48).contains(&slot)
        || slot == 0xA9)
}

/// Brake and wheel slots: the brakes 0x1B (all four drawn from it; the
/// stock part's second model group is the rear one; slot 0x1C is not
/// drawn), front wheels 0x1D, rear 0x1E.
pub const FRONT_BRAKES: usize = 0x1B;
/// The window tint slot (car +0x7F4; its part names the glass's light
/// material).
pub const WINDOW_TINT: usize = 0x99;
/// The lights slot (ride +0x80C): its part colours the headlight pool.
pub const LIGHTS: usize = 0x9F;
/// The roof scoop, spoiler and mirror slots, drawn at the base model's
/// markers (0x620AF0).
pub const ROOF_SCOOP: usize = 0x07;
pub const SPOILER: usize = 0x0C;
pub const MIRRORS: usize = 0x20;
/// The light material the aftermarket mirror models paint with (their
/// body meshes; e.g. MIRROR_POST_LEFT_STYLE03_A), which the mirror draw
/// swaps for the mirror or body paint (0x6269CF / 0x626F44). Its name is
/// not in the data.
pub const MIRROR_PAINT_REFERENCE: u32 = 0x6BFA_56DA;
/// The exhaust slot: its two model groups are drawn at every exhaust
/// marker (render info list +0x598, 0x6245C5).
pub const EXHAUST: usize = 0x10;
/// CARSKIN, the body paint reference (0xD6D6080A).
pub const CARSKIN_HASH: u32 = 0xD6D6_080A;
/// EXHAUST_TIP (0x3B3A1F8D), the exhaust models' paint reference.
pub const EXHAUST_TIP: u32 = 0x3B3A_1F8D;
/// The marker hashes 0x639843 files in the exhaust list: EXHAUST,
/// EXHAUST_LEFT, LEFT_EXHAUST, RIGHT_EXHAUST, 0xDF066A41 (name not found)
/// and EXHAUST_RIGHT.
pub const EXHAUST_MARKERS: [u32; 6] =
    [0xA5B2_8001, 0xA279_9C2B, 0xBCF8_A18B, 0xBD7C_F15E, 0xDF06_6A41, 0xF21B_E91E];
/// The license plate slot (0x802EF0 names it LICENSE_PLATE): its part's
/// models are drawn at the rear plate markers.
pub const LICENSE_PLATE: usize = 0x21;
/// REAR_LICENSE_PLATE, the marker 0x639805 files in the plate list
/// (render info +0x620).
pub const REAR_LICENSE_PLATE_MARKER: u32 = 0xC805_F7E0;
/// The paint slots whose colours the skin's marker texels take (0x61D7A0:
/// engine, spoiler, brake, exhaust, audio, 0x4B, 0x4C).
pub const PART_PAINT_SLOTS: [usize; 7] = [0x44, 0x45, 0x46, 0x47, 0x48, 0x4B, 0x4C];
/// The part type names 0x61BCE0 matches a part's hide attribute
/// (0xE80A3B62) against (the table 0x80333C, read up to its last name,
/// PERF_PART_AE_SPOILER_L3); the slot it hides is the name's index, through
/// the slot table 0x802DE8 whose entry i is slot i (so DECAL_HOOD, type
/// 0x25, hides slot 0x25).
pub static PART_TYPE_NAMES: crate::exe_tables::Table<[&str; 0x69]> = crate::exe_tables::Table::new(|t| &t.part_type_names);

/// The mirror markers' hashes (0x635048 / 0x63505A; their names are not
/// in the EXE): left at y > 0; the right one's matrix mirrors y.
pub const LEFT_MIRROR_MARKER: u32 = 0x9B97_1511;
pub const RIGHT_MIRROR_MARKER: u32 = 0x0EE8_7EC4;
