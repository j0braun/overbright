//! The front end's garage: GarageMain.fng (constructor 0x4EA900, factory
//! 0x4EB130 with view 1 and the FE car 0x8371B0), the 3D scene the menus
//! are drawn over — a platform scene from `FRONTEND\PLATFORMS\<name>.BIN`
//! with the car standing on one of its three turntable discs.
//!
//! * The scene file is an ordinary bundle: a solid list (BACKDROP,
//!   BACKDROP_FLOOR, PLATFORMBASE, DISC01–03), its texture pack, and an
//!   environment map pack (XPOS … ZNEG) that 0x5B9920 copies into the front
//!   end's static cube 0x87088C, which the FE cars reflect.
//! * The draw (0x4A7330, on GarageMain's messages 0x32C72D8F for view 1 and
//!   0xFC946BFA for view 4, the road reflection): view 1 gets BACKDROP and
//!   BACKDROP_FLOOR at the origin and each disc `i` at
//!   `Rz(platform) · T(DISCS[i]) · [Rz(car) for the car's disc]`; view 4
//!   gets BACKDROP and PLATFORMBASE (turned by the platform angle) with m22
//!   negated, so PLATFORMBASE is only seen in the reflective floor and discs
//!   (mesh flag 0x40, worldreflect).
//! * The car (0x4A6670): on disc `d`, at `Rz(platform) · DISCS[d]`, turned
//!   by the turntable angle (GarageMain +0x274) alone, its tyres on the
//!   disc 0.15 m up (0x7F45A8); the front wheels steered by 0x7F45A4.
//! * The camera (0x44CC30, set up by 0x445500 from [`SHOTS`]): a look-at
//!   from the eye to the target with z up (0x444DE0), its field of view
//!   view 1's (0x44D116 → camera +0xC4, the projection 0x5C7ED0).

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use glam::{Mat4, Vec3};

use crate::car::Wheel;
use crate::formats::bundle::{chunks, walk};
use crate::formats::hash::bin_hash;
use crate::formats::texture::Texture;
use crate::render::Camera;
use crate::world::{Placement, World};

/// The platform `name`'s file, FRONTEND/PLATFORMS/<name>.BIN (its name
/// matched without case), which [`Platform::load`] reads.
pub fn platform_file(root: &Path, name: &str) -> Result<std::path::PathBuf> {
    let dir = crate::game_fs::path(root, "FRONTEND/PLATFORMS");
    let want = format!("{name}.BIN");
    std::fs::read_dir(&dir)
        .with_context(|| format!("{}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(&want)))
        .with_context(|| format!("no {want} in {}", dir.display()))
}

/// The platform scenes by number (0x7F60E8; 0x4A7180 loads
/// `FRONTEND\PLATFORMS\%s.BIN` by these names).
pub const PLATFORMS: [&str; 9] = ["NONE", "SHOWROOM", "AUDIOSHOP01", "CARLOT01", "CRIB01", "MEGALOSHOP01", "PAINTSHOP01", "PARTSSHOP01", "PERFSHOP01"];

/// The three discs' centres on the platform (0x7F45AC, 12 bytes each; the
/// loop of 0x4A7330 runs up to 0x7F45D4).
pub const DISCS: [Vec3; 3] = [Vec3::new(0.0, 5.4, 0.0), Vec3::new(4.7, -2.7, 0.0), Vec3::new(-4.7, -2.7, 0.0)];

/// How high the car's tyres stand (0x7F45A8): 0x4A6670 puts each wheel's
/// contact point this far up, and the LOCK model this far over its disc.
pub const TYRE_HEIGHT: f32 = 0.15;

/// One of GarageMain's two camera set-ups (0x7F608C + 0x30·n; the
/// constructor takes record +0x55, 0, and 0x4D82D0 switches to record 1
/// for an SUV, car type +0x88A).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    /// +0x00: the turntable angle in degrees (GarageMain +0x274, through
    /// +0x9D·4).
    pub car_angle: f32,
    /// +0x04: the front wheels' steer in degrees (0x7F45A4).
    pub steer: f32,
    /// +0x08: the field of view in degrees (camera +0x80 → +0xC4).
    pub fov: f32,
    /// +0x0C: what the camera looks at (+0x90).
    pub target: Vec3,
    /// +0x18: the eye (+0xA0).
    pub eye: Vec3,
}

pub const SHOTS: [Shot; 2] = [
    Shot { car_angle: 305.772_9, steer: 30.0, fov: 32.0, target: Vec3::new(0.0, 4.0, 1.3), eye: Vec3::new(0.0, -4.0, 1.3997) },
    Shot { car_angle: 305.772_9, steer: 26.0, fov: 37.0, target: Vec3::new(0.0, 4.0, 1.4996), eye: Vec3::new(0.0, -4.0, 1.3997) },
];

/// The part views (0x7F4608: two tables of 0x31 records of 0x3C bytes, cars
/// then SUVs, picked by GarageMain +0x154): +0x00 a tour flag (the idle
/// tour's "Animate Camera1…7", records 42–48, which the clone leaves out),
/// +0x04 the name, +0x08 the part slot, then a [`Shot`] (+0x0C car angle,
/// +0x10 steer, +0x14 field of view, +0x18 target, +0x24 eye; +0x30 and
/// +0x34 are zero) and +0x38 a float (20, or 170 for the front bumper)
/// that 0x4A55E0 keeps at +0x23C+0x30 (**not traced** further).
pub static SLOT_SHOTS: crate::exe_tables::Table<[[(u32, Shot); 42]; 2]> = crate::exe_tables::Table::new(|t| &t.slot_shots);

/// 0x7F5D00, "Engine": the view 0x4A5510 gives slots 0x0D, 0x9B and 0x44
/// for an SUV when GarageMain +0x158 is set (the car type's +0x840 is 8,
/// set by 0x4D82D0 when a car change ends).
pub static SUV_ENGINE_SHOT: crate::exe_tables::Table<Shot> = crate::exe_tables::Table::new(|t| &t.suv_engine_shot);

/// The vinyl views (0x7F5D40, 7 records a class): 0x4A5510 gives slots
/// 0x40–0x43 the first for a vinyl of group 0x14 (part +5 & 0x1F) and
/// record `group − 0x15` for groups 0x16–0x1B, else slot 0x40's.
pub static VINYL_SHOTS: crate::exe_tables::Table<[[Shot; 7]; 2]> = crate::exe_tables::Table::new(|t| &t.vinyl_shots);

/// The view 0x4A5510(slot, suv, engine, part) finds: decal slots
/// 0x22–0x2E share 0x22's, vinyl slots 0x40–0x43 go by the vinyl's group
/// (`vinyl_group`, part +5 & 0x1F; None for no part), then the table's
/// record of that slot; None when it has none (0x4A55E0 then leaves the
/// view as it is).
pub fn slot_shot(slot: u32, suv: bool, engine: bool, vinyl_group: Option<u8>) -> Option<Shot> {
    if suv && engine && matches!(slot, 0x0D | 0x9B | 0x44) {
        return Some(*SUV_ENGINE_SHOT);
    }
    let class = usize::from(suv);
    let mut slot = slot;
    if (0x22..=0x2E).contains(&slot) {
        slot = 0x22;
    } else if (0x40..0x44).contains(&slot) {
        match vinyl_group {
            Some(0x14) => return Some(VINYL_SHOTS[class][0]),
            Some(g @ 0x16..=0x1B) => return Some(VINYL_SHOTS[class][usize::from(g - 0x15)]),
            _ => slot = 0x40,
        }
    }
    SLOT_SHOTS[class].iter().find(|(s, _)| *s == slot).map(|(_, shot)| *shot)
}

/// What GarageMain shows: the camera (its field of view, target and eye),
/// the car's turntable angle (+0x274) and the front wheels' steer
/// (0x7F45A4), with the camera blend and the car's turn under way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub camera: Shot,
    pub car_angle: f32,
    pub steer: f32,
    blend: Option<Blend>,
    turn: Option<Turn>,
    /// The platform's angle (0x8363F0) and the shown car's disc (+0x144;
    /// during a car change's turn the incoming car's, +0x140).
    pub platform_angle: f32,
    pub disc: usize,
    /// While a car change's platform turn runs ([`View::at`]): the disc the
    /// outgoing car rides out on (+0x144 until the turn ends, 0x4D84A2),
    /// where GarageMain's other FE car stays visible (+0xB64) until then.
    pub outgoing_disc: Option<usize>,
    change: Option<CarChange>,
    /// The idle tour's clock (0x837AA0, in seconds): when the view last
    /// changed (a part view 0x4C1D40, a reset 0x4C1FA0, a tour step
    /// 0x4A5BD0) or the constructor ran (0x4EB0AB); `None` until the
    /// first [`View::tick`].
    idle_since: Option<f32>,
    /// The idle tour is on (0x8363FC).
    touring: bool,
    /// When the tour's hold began (0x83592C): refreshed while the camera
    /// blends, so the next step waits [`TOUR_HOLD_SECONDS`] after a blend
    /// ends; the constructor's 0 (0x4EB0A5) lets the first step go at once.
    hold_from: f32,
    /// The camera mover's shortest blend (+0x150) as the last view set it.
    blend_min: f32,
}

/// The idle tour (0x4D82D0, 0x4D88C1…0x4D89D4; **verified**, Wine menus115
/// t0–t6 with 0x837AA0 poked back): once the front-end tick clock 0x8651A8
/// is more than 300 s (0x7F45D4; ticks × 0.00025) past 0x837AA0 the tour
/// starts (0x8363FC), and from then on, each time the camera's blend has
/// ended and 1200 frames of 0x7FB708 (60 a second; 20 s) have passed, the
/// next record of 0x7F4608 with its tour flag set ("Animate Camera1…7",
/// records 42–48; the index 0x836400 runs 0..=49 and wraps) is shown by
/// 0x4A5BD0: the camera blends as a part view's (0x451200 at the current
/// +0x150) and the car turns the short way to the record's angle and
/// steer. Only the mouse (0x4A5D90: wheel or a held button) and the part
/// views' 0x4C2100 / the magazine pose 0x4C2070 stop it; menu keys do not
/// reset its clock.
pub const TOUR_IDLE_SECONDS: f32 = 300.0;
/// 0x7F45D0: 1200 frames of 0x7FB708 (which counts 60 a second).
pub const TOUR_HOLD_SECONDS: f32 = 20.0;

/// The tour records (0x7F4608 records 42–48, flag 1, slots 0x40302010…16),
/// cars then SUVs.
pub const TOUR_SHOTS: [[Shot; 7]; 2] = [
    [
        Shot { car_angle: 125.2885, steer: 24.0, fov: 30.0, target: Vec3::new(0.1998, 4.0, 1.3596), eye: Vec3::new(-0.3001, -4.0, 1.23) },
        Shot { car_angle: 115.8436, steer: 33.0, fov: 20.0, target: Vec3::new(0.099, 4.0, 1.1585), eye: Vec3::new(-0.201, -4.0, 1.3216) },
        Shot { car_angle: 63.8488, steer: -20.0, fov: 30.0, target: Vec3::new(-0.0004, 4.0, 1.3996), eye: Vec3::new(0.0, -4.0, 3.1688) },
        Shot { car_angle: 248.8915, steer: -28.0, fov: 17.0, target: Vec3::new(-0.3004, 4.0, 0.9989), eye: Vec3::new(-0.4007, -4.0, 1.27) },
        Shot { car_angle: 359.0, steer: 25.0, fov: 33.0, target: Vec3::new(-0.1001, 4.0, 1.8384), eye: Vec3::new(-0.2003, -4.0, 3.5981) },
        Shot { car_angle: 233.1009, steer: -35.0, fov: 20.0, target: Vec3::new(-0.1004, 4.0, 0.9989), eye: Vec3::new(0.1993, -4.0, 1.27) },
        Shot { car_angle: 307.0, steer: 20.0, fov: 30.0, target: Vec3::new(0.1, 4.0, 1.2), eye: Vec3::new(0.0, -4.0, 1.27) },
    ],
    [
        Shot { car_angle: 125.2885, steer: 26.0, fov: 35.0, target: Vec3::new(0.1998, 4.0, 1.6595), eye: Vec3::new(-0.3001, -4.0, 1.27) },
        Shot { car_angle: 66.5241, steer: -33.0, fov: 25.0, target: Vec3::new(-0.3012, 4.0, 1.4582), eye: Vec3::new(-0.201, -4.0, 1.3199) },
        Shot { car_angle: 63.8488, steer: -30.0, fov: 35.0, target: Vec3::new(-0.0004, 4.0, 1.6994), eye: Vec3::new(0.0, -4.0, 3.1688) },
        Shot { car_angle: 248.8915, steer: -30.0, fov: 23.0, target: Vec3::new(-0.3004, 4.0, 1.2988), eye: Vec3::new(-0.4007, -4.0, 1.27) },
        Shot { car_angle: 359.0, steer: 33.0, fov: 35.0, target: Vec3::new(-0.1001, 4.0, 1.8384), eye: Vec3::new(-0.2003, -4.0, 3.5981) },
        Shot { car_angle: 233.1009, steer: -35.0, fov: 25.0, target: Vec3::new(-0.1004, 4.0, 1.3988), eye: Vec3::new(0.1993, -4.0, 1.2973) },
        Shot { car_angle: 307.0, steer: 20.0, fov: 36.0, target: Vec3::new(0.1, 4.0, 1.5999), eye: Vec3::new(0.0, -4.0, 1.27) },
    ],
];

/// The tour's record index (0x836400, a global: it carries on from one
/// GarageMain to the next; the file's 1).
static TOUR_INDEX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

/// A car change's platform turn (0x4D82D0 state 2, 0x4D843D…0x4D85ED):
/// the platform from +0x150 (0x8363F0 when it began) by f · 120° (up for
/// direction +0x148 = 0, down for 1), f = elapsed ÷ 0.5 s (+0x124),
/// linear; the new car on the next disc (+0x140, [`next_disc`]), which
/// becomes the shown disc (+0x144) when the turn ends.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CarChange {
    from: f32,
    up: bool,
    disc: usize,
    start: f32,
}

/// How long a car change turns the platform (0x4D8338: +0x124 = 0.5).
pub const PLATFORM_TURN_SECONDS: f32 = 0.5;

/// The disc the next car stands on (0x4D8370: the table {1, 2, 0} turning
/// up, {2, 0, 1} down, indexed by the current disc), which a 120° turn
/// that way brings to the front.
pub fn next_disc(disc: usize, up: bool) -> usize {
    [[1, 2, 0], [2, 0, 1]][usize::from(!up)][disc % 3]
}

/// The FE camera mover's blend (0x451200 → 0x44CE00 state 0): from the
/// camera as it was (+0xC0…) to the target (+0x100…) over +0x14C seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Blend {
    from: Shot,
    to: Shot,
    start: f32,
    duration: f32,
}

/// The car's turn to a part view (0x4C1D40 sets it up, 0x4D82D0 runs it
/// while the camera's +0x30 is set): angle +0x11C ± f · +0x120, steer
/// +0x12C + f · +0x130, f = elapsed ÷ +0x124 (the camera's +0x14C), up
/// to 1, linear.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Turn {
    angle: f32,
    delta: f32,
    steer: f32,
    steer_delta: f32,
    start: f32,
    duration: f32,
}

/// The shortest blend of a part view (0x4532B0: camera +0x150 = 0.3; 10
/// when 0x4A55E0 runs during the idle tour, which a menu's part view
/// does not: 0x4C2100 ends the tour first).
pub const PART_BLEND_MIN: f32 = 0.3;
/// The shortest blend back to the base view (0x453320: +0x150 = 2.5).
pub const BASE_BLEND_MIN: f32 = 2.5;

/// 0x44D190: how long the mover takes to `to` from `from`, with `dt` the
/// frame's time (0x865194) and `turn` the |current − target| car angle
/// the caller passes (0x4A55E0: +0x274 and the record's +0x0C; 0 back to
/// the base view): the largest of the turn, |Δfov| ÷ (50 dt), |Δtarget| ÷
/// (2.2 dt) and |Δeye| ÷ (2.2 dt) (0x4455D0; 0 for an unchanged one; the
/// two zero record floats +0x30 / +0x34 likewise), × 0.0055, at least
/// `min` (camera +0x150).
pub fn blend_duration(from: &Shot, to: &Shot, turn: f32, dt: f32, min: f32) -> f32 {
    let over = |d: f32, rate: f32| if d == 0.0 || dt <= 0.0 { 0.0 } else { d / (rate * dt) };
    let moved = |a: Vec3, b: Vec3| if a.abs_diff_eq(b, 0.001) { 0.0 } else { over(a.distance(b), 2.2) };
    let longest = turn.abs().max(over((to.fov - from.fov).abs(), 50.0)).max(moved(from.target, to.target)).max(moved(from.eye, to.eye));
    (longest * 0.0055).max(min)
}

/// 0x44CE00's ease: f · 180° − 90° as a binary angle (× 65536 ÷ 360,
/// truncated, 0x75BED4), its sine (0x43C2B0), (sine + 1) ÷ 2.
pub fn ease(f: f32) -> f32 {
    let binary = ((f * 180.0 - 90.0) * 65536.0) as i32 / 360;
    ((binary as u16) as f32 * std::f32::consts::TAU / 65536.0).sin().mul_add(0.5, 0.5)
}

impl View {
    /// As the constructor (0x4EA900) leaves it: camera record `record`
    /// (0x445500 from 0x7F608C + 0x30·record) with its car angle and steer.
    pub fn new(record: usize) -> View {
        let shot = SHOTS[record];
        // The mover's +0x150 before any view: 2.5 as read under Wine in
        // the garage (**inferred**: the constructor's value not traced).
        View { camera: shot, car_angle: shot.car_angle, steer: shot.steer, blend: None, turn: None, platform_angle: 0.0, disc: 0, outgoing_disc: None, change: None, idle_since: None, touring: false, hold_from: f32::NEG_INFINITY, blend_min: BASE_BLEND_MIN }
    }

    /// The view at `now` (seconds): the camera eased along its blend
    /// (0x44CE00: field of view, target and eye from the old values by
    /// [`ease`] of elapsed ÷ duration), the car turned and steered
    /// linearly over the same time (0x4D82D0).
    pub fn at(&self, now: f32) -> View {
        let mut v = *self;
        let fraction = |start: f32, duration: f32| if duration > 0.0 && now - start < duration { ((now - start) / duration).max(0.0) } else { 1.0 };
        if let Some(b) = self.blend {
            let s = ease(fraction(b.start, b.duration));
            v.camera.fov = s * b.to.fov + (1.0 - s) * b.from.fov;
            v.camera.target = b.from.target + (b.to.target - b.from.target) * s;
            v.camera.eye = b.from.eye + (b.to.eye - b.from.eye) * s;
        }
        if let Some(t) = self.turn {
            let f = fraction(t.start, t.duration);
            v.car_angle = t.angle + f * t.delta;
            v.steer = t.steer + f * t.steer_delta;
        }
        if let Some(c) = self.change {
            let f = fraction(c.start, PLATFORM_TURN_SECONDS);
            v.platform_angle = if c.up { c.from + f * 120.0 } else { c.from - f * 120.0 };
            v.outgoing_disc = (f < 1.0).then_some(self.disc);
            v.disc = c.disc;
        }
        v
    }

    /// A car change at `now` (GarageMain's +0x148 from the lot's
    /// Left / Right, 0x4A7900: 0 for Right 0xB5971BF1, 1 else; `up` is
    /// 0): the platform turns 120° that way and the new car stands on the
    /// next disc, while the old car rides out on its own disc
    /// ([`View::outgoing`]; GarageMain's two FE cars +0x4C / +0x50, the
    /// shown one by +0x54, 0x4D873A…0x4D87DF). The caller starts it only
    /// once the new car is loaded (state 1, 0x4D85F8: the loader's
    /// [+0x234]+0x2CD4) and plays the turn's sound (0x51DE40(5 or 4,
    /// 0x17)).
    pub fn change_car(&mut self, up: bool, now: f32) {
        let here = self.at(now);
        self.platform_angle = here.platform_angle;
        self.disc = here.disc;
        self.change = Some(CarChange { from: here.platform_angle, up, disc: next_disc(here.disc, up), start: now });
    }

    /// A car change's platform turn is still under way at `now`
    /// (GarageMain state 2, [`PLATFORM_TURN_SECONDS`]).
    pub fn changing(&self, now: f32) -> bool {
        self.change.is_some_and(|c| now - c.start < PLATFORM_TURN_SECONDS)
    }

    /// A part's view (0x4C2100 → 0x4C1D40 → 0x4A55E0) asked at `now`
    /// with frame time `dt`: the record's camera (0x4532B0 → 0x451200,
    /// [`blend_duration`] at least [`PART_BLEND_MIN`]), and the car turned
    /// the short way round from its angle (taken mod 360) to the record's
    /// (a tie turns it down) and steered to its steer over that time
    /// (0x4C1DCE…0x4C1EE1). No record: as it was.
    pub fn set_slot(&mut self, slot: u32, suv: bool, engine: bool, vinyl_group: Option<u8>, now: f32, dt: f32) {
        // 0x4C2100 ends the idle tour, 0x4C1D40 restarts its clock.
        self.touring = false;
        self.idle_since = Some(now);
        let Some(shot) = slot_shot(slot, suv, engine, vinyl_group) else { return };
        self.blend_min = PART_BLEND_MIN;
        self.show(shot, now, dt);
    }

    /// 0x4C1DCE…0x4C1EE1 / 0x4A5BD0: the camera blends to `shot` (at
    /// least +0x150) and the car turns the short way round from its
    /// angle (taken mod 360) to the shot's (a tie turns it down) and
    /// steers to its steer over that time.
    fn show(&mut self, shot: Shot, now: f32, dt: f32) {
        let here = self.at(now);
        let duration = blend_duration(&here.camera, &shot, here.car_angle - shot.car_angle, dt, self.blend_min);
        let current = here.car_angle.rem_euclid(360.0);
        let (up, down) = if shot.car_angle > current { (shot.car_angle - current, current + 360.0 - shot.car_angle) } else { (shot.car_angle + 360.0 - current, current - shot.car_angle) };
        let delta = if down > up { up } else { -down };
        *self = View {
            camera: here.camera,
            car_angle: here.car_angle,
            steer: here.steer,
            blend: Some(Blend { from: here.camera, to: shot, start: now, duration }),
            turn: Some(Turn { angle: here.car_angle, delta, steer: here.steer, steer_delta: shot.steer - here.steer, start: now, duration }),
            ..*self
        };
    }

    /// Back to the base view (0x4C2140 → 0x4C1FA0 → 0x453320(1)) at `now`:
    /// the camera blends to the one the constructor set (camera +0x40 …)
    /// over at least [`BASE_BLEND_MIN`]; the car's turn stops where it is
    /// (0x4D82D0 turns only while the camera's +0x30 is set, which
    /// 0x453320 clears).
    pub fn reset(&mut self, record: usize, now: f32, dt: f32) {
        let here = self.at(now);
        let to = SHOTS[record];
        // 0x4C1FA0 restarts the idle tour's clock (it leaves a tour on).
        self.idle_since = Some(now);
        self.blend_min = BASE_BLEND_MIN;
        let duration = blend_duration(&here.camera, &to, 0.0, dt, BASE_BLEND_MIN);
        *self = View { camera: here.camera, car_angle: here.car_angle, steer: here.steer, blend: Some(Blend { from: here.camera, to, start: now, duration }), turn: None, ..*self };
    }

    /// GarageMain's per-frame idle tour (0x4D88C1…0x4D89D4, see
    /// [`TOUR_IDLE_SECONDS`]); `suv` picks the record table (+0x154).
    pub fn tick(&mut self, now: f32, dt: f32, suv: bool) {
        let since = *self.idle_since.get_or_insert(now);
        if !self.touring && now - since > TOUR_IDLE_SECONDS {
            self.touring = true;
        }
        if !self.touring {
            return;
        }
        // 0x444DC0: the blend is done once its time is up.
        let blending = self.blend.is_some_and(|b| now - b.start < b.duration);
        if blending {
            self.hold_from = now;
        }
        if now - self.hold_from > TOUR_HOLD_SECONDS {
            self.hold_from = now;
            // The next flagged record (42–48) of 0..=49.
            let mut i = TOUR_INDEX.load(std::sync::atomic::Ordering::Relaxed);
            loop {
                i = if i >= 49 { 0 } else { i + 1 };
                if (42..=48).contains(&i) {
                    break;
                }
            }
            TOUR_INDEX.store(i, std::sync::atomic::Ordering::Relaxed);
            self.idle_since = Some(now);
            self.show(TOUR_SHOTS[usize::from(suv)][i - 42], now, dt);
        }
    }

    /// The turntable for this view: platform and disc as the constructor
    /// leaves them, the car at this view's angle.
    pub fn turntable(&self) -> Turntable {
        Turntable { car_angle: self.car_angle, platform_angle: self.platform_angle, disc: self.disc }
    }

    /// The outgoing car's turntable while a car change's turn runs (a view
    /// from [`View::at`]): its own disc (+0x144), the same platform angle
    /// and car angle (0x4A6670 reads GarageMain's +0x274 for either car).
    pub fn outgoing(&self) -> Option<Turntable> {
        self.outgoing_disc.map(|disc| Turntable { disc, ..self.turntable() })
    }

    /// The platform's own pose: the disc 0x4A7330 turns with the car's
    /// angle is +0x144 (0x4A74EF), the outgoing car's while the turn runs.
    pub fn platform_turntable(&self) -> Turntable {
        self.outgoing().unwrap_or(self.turntable())
    }
}

impl Shot {
    /// The garage camera: a look-at with z up (0x444DE0: right =
    /// (0, 0, −1) × forward, no roll while +0xB8 is 0) and view 1's
    /// projection of the field of view (`Camera::game_fov`).
    pub fn camera(&self) -> Camera {
        let fov = self.fov.to_radians();
        let mut camera = Camera::look_at(self.eye, self.target, Vec3::Z, fov);
        camera.game_fov = Some(fov);
        camera
    }
}

/// The turntable rotation 0x5BAE70 applies: degrees to a binary angle
/// (× 65536 / 360, truncated, 0x75BED4) and a turn about +z.
fn turn(degrees: f32) -> Mat4 {
    let binary = (degrees * 65536.0) as i32 / 360;
    Mat4::from_rotation_z((binary & 0xFFFF) as f32 * std::f32::consts::TAU / 65536.0)
}

/// Where the platform's models go in the clone's world renderer.
#[derive(Clone, Copy, Debug)]
struct Slots {
    discs: [Option<usize>; 3],
    base: Option<usize>,
}

/// A loaded platform scene.
pub struct Platform {
    pub name: String,
    /// The scene's models placed for the world renderer (see
    /// [`Platform::load`]).
    pub world: World,
    /// The environment map's six faces (XPOS, XNEG, YPOS, YNEG, ZPOS,
    /// ZNEG), when the file has them all.
    pub envmap: Option<Vec<Texture>>,
    slots: Slots,
}

/// The instance flags that keep a model in view 1 and out of view 4, or the
/// other way round (the world renderer's cull, `WorldView::culls`: view 1
/// drops `(flags ^ 0x60) & 0x1A`, view 4 `(flags ^ 0x60) & 0x58` and
/// 0x0400).
const BOTH_VIEWS: u16 = 0x60;
const MAIN_VIEW_ONLY: u16 = 0x60 | 0x0400;
const REFLECTION_ONLY: u16 = 0x62;

/// GLOBALB.BUN's share of every platform scene: its particle descriptions
/// and emitter templates, then the neon tube and glow sprite textures in
/// file order. It is the same for every platform and the game's data does
/// not change while it runs, so it is parsed once a run per game folder
/// instead of on every garage and shop entry (
/// an 8 MB read and three walks). A failed read is not kept.
struct GlobalbResident {
    emitters: crate::particles::Library,
    textures: Vec<Texture>,
}

/// How many times [`GlobalbResident::parse`] ran (the read-once test).
static GLOBALB_RESIDENT_PARSES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The textures of the front end's resident pack FrontB.lzc whose hash is
/// in `wanted`, in file order. The pack is resident in the front end (the
/// original loads it once), so each distinct pick is read, decompressed and
/// walked once a run per game folder instead of on every garage and shop
/// entry (~4-5 ms of JDLZ per entry here). A failed read is
/// not kept.
fn front_end_pack_picks(root: &Path, wanted: &[u32]) -> Option<std::sync::Arc<Vec<Texture>>> {
    type Memo = std::sync::Mutex<Option<HashMap<(std::path::PathBuf, Vec<u32>), std::sync::Arc<Vec<Texture>>>>>;
    static MEMO: Memo = std::sync::Mutex::new(None);
    let mut memo = MEMO.lock().unwrap_or_else(|e| e.into_inner());
    let memo = memo.get_or_insert_with(HashMap::new);
    let key = (root.to_path_buf(), wanted.to_vec());
    if let Some(p) = memo.get(&key) {
        return Some(p.clone());
    }
    #[cfg(test)]
    FRONT_PICKS_KEYS.lock().unwrap().push(wanted.to_vec());
    let picked = std::sync::Arc::new(front_end_pack_scan(root, wanted)?);
    memo.insert(key, picked.clone());
    Some(picked)
}

/// [`front_end_pack_picks`] without the memo: FrontB.lzc read, decompressed
/// and walked.
fn front_end_pack_scan(root: &Path, wanted: &[u32]) -> Option<Vec<Texture>> {
    let raw = crate::game_fs::read(root, "FRONTEND/FrontB.lzc").ok()?;
    let front = if raw.starts_with(b"JDLZ") { crate::formats::jdlz::decompress(&raw) } else { Ok(raw) }.ok()?;
    FRONT_PACK_READS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut picked = Vec::new();
    walk(chunks(&front, 0), &mut |c| {
        if c.id == 0xB330_0000
            && let Ok(ts) = crate::formats::texture::parse_texture_pack(c)
        {
            picked.extend(ts.into_iter().filter(|t| wanted.contains(&t.hash)));
        }
    });
    Some(picked)
}

/// The `wanted` sets [`front_end_pack_picks`] scanned (the test's keys).
#[cfg(test)]
static FRONT_PICKS_KEYS: std::sync::Mutex<Vec<Vec<u32>>> = std::sync::Mutex::new(Vec::new());

/// How many times [`front_end_pack_picks`] decompressed FrontB.lzc.
static FRONT_PACK_READS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl GlobalbResident {
    fn get(root: &Path) -> Option<std::sync::Arc<GlobalbResident>> {
        type Memo = std::sync::Mutex<Option<HashMap<std::path::PathBuf, std::sync::Arc<GlobalbResident>>>>;
        static MEMO: Memo = std::sync::Mutex::new(None);
        // Parsed under the lock: a second loader (the garage prefetch and
        // the frame) waits for the first one's value instead of repeating it.
        let mut memo = MEMO.lock().unwrap_or_else(|e| e.into_inner());
        let memo = memo.get_or_insert_with(HashMap::new);
        if let Some(r) = memo.get(root) {
            return Some(r.clone());
        }
        let r = std::sync::Arc::new(GlobalbResident::parse(root)?);
        memo.insert(root.to_path_buf(), r.clone());
        Some(r)
    }

    fn parse(root: &Path) -> Option<GlobalbResident> {
        let globalb = crate::game_fs::read(root, "GLOBAL/GLOBALB.BUN").ok()?;
        GLOBALB_RESIDENT_PARSES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut emitters = crate::particles::Library::default();
        emitters.add(&globalb);
        emitters.add_templates(&globalb);
        // NEONBLUR, the cars' neon tubes (render info +0x348): GLOBALB
        // is resident in the front end too; so are the glow sprites'
        // textures the FE car's light flares take in the Lights list
        // (0x61ACF0, `crate::glow::texture_hashes`).
        let neon = bin_hash(crate::shadow::NEON_TUBE_TEXTURE);
        let glows = crate::glow::texture_hashes();
        let mut textures = Vec::new();
        walk(chunks(&globalb, 0), &mut |c| {
            if c.id == 0xB330_0000
                && let Ok(ts) = crate::formats::texture::parse_texture_pack(c)
            {
                textures.extend(ts.into_iter().filter(|t| t.hash == neon || glows.contains(&t.hash)));
            }
        });
        Some(GlobalbResident { emitters, textures })
    }
}

impl Platform {
    /// Loads `FRONTEND\PLATFORMS\<name>.BIN` (any case). The models become
    /// placements drawn as 0x4A7330 draws them: BACKDROP in both views,
    /// BACKDROP_FLOOR and the discs in view 1 only, PLATFORMBASE in view 4
    /// only (the reflection pass flips each about its origin, which for
    /// these is 0x4A7330's m22 = −1).
    pub fn load(root: &Path, name: &str) -> Result<Platform> {
        let path = platform_file(root, name)?;
        let data = std::fs::read(&path)?;
        let mut solids = Vec::new();
        let mut textures = HashMap::new();
        let mut faces: [Option<Texture>; 6] = Default::default();
        let mut error = None;
        walk(chunks(&data, 0), &mut |c| match c.id {
            0x8013_4000 => match crate::formats::solid::parse_solid_list(c) {
                Ok(s) => solids.extend(s),
                Err(e) => error = Some(e),
            },
            0xB330_0000 => match crate::formats::texture::parse_texture_pack(c) {
                Ok(ts) => {
                    for t in ts {
                        match crate::formats::magazines::ENVMAP_FACES.iter().position(|f| bin_hash(f) == t.hash) {
                            Some(i) => {
                                faces[i].get_or_insert(t);
                            }
                            None => {
                                textures.entry(t.hash).or_insert(t);
                            }
                        }
                    }
                }
                Err(e) => error = Some(e),
            },
            _ => {}
        });
        if let Some(e) = error {
            return Err(e.context(format!("{}", path.display())));
        }
        // The scene's particle effects: like any bundle, the platform file's
        // descriptions (0x35021) join GLOBALB's resident ones (0x491C70 →
        // the list 0x830D08) and its placed emitters (0x35020; SHOWROOM's
        // one is the steam by the lamp, D4884E83 at (−3.57, 9.55, 0)) are
        // bound and started (0x48BC00, 0x48BC50). GarageMain's update
        // (0x4D82D0) runs them (Particles_Update 0x492E10 with the frame's
        // dt) and the frame draws them (0x48BDF0). Their textures come from
        // the front end's resident pack FrontB.lzc (SFX_SMOKE_B_CS).
        let mut emitters = crate::particles::Library::default();
        if let Some(resident) = GlobalbResident::get(root) {
            emitters = resident.emitters.clone();
            for t in &resident.textures {
                textures.entry(t.hash).or_insert_with(|| t.clone());
            }
        }
        emitters.add(&data);
        let mut placed_emitters = Vec::new();
        walk(chunks(&data, 0), &mut |c| {
            if c.id == crate::particles::PLACED_EMITTERS {
                placed_emitters.extend(crate::particles::parse_placed(c.aligned(16)));
            }
        });
        // Also the car effects' (GLOBALB's emitter records, cloned by
        // 0x48B8B0 for the FE car's purge test: SFX_SMOKE_B_CS).
        let wanted: Vec<u32> = placed_emitters
            .iter()
            .map(|p| p.description)
            .chain(emitters.templates.iter().map(|t| t.0))
            .filter_map(|h| emitters.by_hash(h))
            .map(|i| emitters.descriptions[i].texture)
            .filter(|h| !textures.contains_key(h))
            .collect();
        if !wanted.is_empty()
            && let Some(picked) = front_end_pack_picks(root, &wanted)
        {
            for t in picked.iter() {
                textures.entry(t.hash).or_insert_with(|| t.clone());
            }
        }
        let solid_by_hash: HashMap<u32, usize> = solids.iter().enumerate().map(|(i, s)| (s.hash, i)).collect();
        let mut placements: Vec<Placement> = Vec::new();
        let mut place = |model: &str, flags: u16| {
            let solid = *solid_by_hash.get(&bin_hash(model))?;
            let low_solid = (flags != MAIN_VIEW_ONLY).then_some(solid);
            placements.push(Placement { solid, low_solid, mid_solid: Some(solid), transform: Mat4::IDENTITY, section: 0, flags, cull_radius: 0.0 });
            Some(placements.len() - 1)
        };
        place("BACKDROP", BOTH_VIEWS);
        place("BACKDROP_FLOOR", MAIN_VIEW_ONLY);
        let discs = [place("DISC01", MAIN_VIEW_ONLY), place("DISC02", MAIN_VIEW_ONLY), place("DISC03", MAIN_VIEW_ONLY)];
        let base = place("PLATFORMBASE", REFLECTION_ONLY);
        let mut world = World::from_models(solids, textures, placements);
        world.emitters = emitters;
        world.placed_emitters = placed_emitters;
        let mut platform = Platform { name: name.to_string(), world, envmap: faces.into_iter().collect(), slots: Slots { discs, base } };
        let poses = platform.poses(&Turntable::default());
        for (slot, transform) in poses {
            platform.world.placements[slot].transform = transform;
        }
        Ok(platform)
    }

    /// The models' transforms for this turntable state (0x4A7330), by
    /// placement.
    pub fn poses(&self, t: &Turntable) -> Vec<(usize, Mat4)> {
        let platform = turn(t.platform_angle);
        let mut out = Vec::new();
        for (i, slot) in self.slots.discs.iter().enumerate() {
            if let Some(slot) = *slot {
                let spin = if i == t.disc { turn(t.car_angle) } else { Mat4::IDENTITY };
                out.push((slot, platform * Mat4::from_translation(DISCS[i]) * spin));
            }
        }
        if let Some(slot) = self.slots.base {
            out.push((slot, platform));
        }
        out
    }
}

/// GarageMain's turntable: the platform's angle (0x8363F0, which a car
/// change turns by ±120° over half a second, 0x4D82D0), the disc the car
/// stands on (+0x144) and the car's own angle (+0x274), all in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turntable {
    pub platform_angle: f32,
    pub disc: usize,
    pub car_angle: f32,
}

impl Default for Turntable {
    /// As the constructor leaves it: platform at 0 (0x4EACD5), the car on
    /// disc 0 (+0x144 = 0, given to the shown car's +0xB78), turned by the
    /// first shot's angle.
    fn default() -> Turntable {
        Turntable { platform_angle: 0.0, disc: 0, car_angle: SHOTS[0].car_angle }
    }
}

impl Turntable {
    /// The shown car's disc stands at the front, where the platform's
    /// first disc starts (a whole third turn of the platform from it).
    pub fn car_in_front(&self) -> bool {
        turn(self.platform_angle).transform_point3(DISCS[self.disc]).distance(DISCS[0]) < 0.05
    }

    /// The car's car → world transform (0x4A6670 with the hydraulics at
    /// rest, so all four tyres touch [`TYRE_HEIGHT`] and the car sits
    /// level): x, y the disc's centre turned with the platform; z the
    /// tyre height plus the wheels' mean `radius − centre z`; turned by the
    /// car's angle only.
    pub fn car_transform(&self, wheels: &[Wheel; 4]) -> Mat4 {
        let at = turn(self.platform_angle).transform_point3(DISCS[self.disc]);
        let lift = wheels.iter().map(|w| w.radius - w.position.z).sum::<f32>() * 0.25;
        Mat4::from_translation(Vec3::new(at.x, at.y, TYRE_HEIGHT + lift)) * turn(self.car_angle)
    }

    /// 0x4A6670 with the hydraulics' corner heights (car +0x184 + 4i,
    /// `crate::hydraulics::Garage`): each tyre's contact P_i is the wheel's
    /// x, y turned by the car's angle about the disc's centre, at z = 0.15
    /// + corner i; the axes are A = (p1 − p2) + (p0 − p3) and B = (p3 − p2)
    /// + (p0 − p1), each normalised, and C = A × B (not made orthogonal);
    /// the origin's z is the contacts' mean + the mean of radius − wheel
    /// centre z over |C.z|. All corners at 0 give [`Self::car_transform`].
    pub fn car_transform_on(&self, wheels: &[Wheel; 4], corner: [f32; 4]) -> Mat4 {
        if corner == [0.0; 4] {
            return self.car_transform(wheels);
        }
        let at = turn(self.platform_angle).transform_point3(DISCS[self.disc]);
        let r = turn(self.car_angle);
        let p: [Vec3; 4] = std::array::from_fn(|i| {
            let q = r.transform_point3(Vec3::new(wheels[i].position.x, wheels[i].position.y, 0.0));
            Vec3::new(at.x + q.x, at.y + q.y, TYRE_HEIGHT + corner[i])
        });
        let norm = |v: Vec3| if v.length() != 0.0 { v.normalize() } else { Vec3::X };
        let a = norm((p[1] - p[2]) + (p[0] - p[3]));
        let b = norm((p[3] - p[2]) + (p[0] - p[1]));
        let c = a.cross(b);
        let lift = wheels.iter().map(|w| w.radius - w.position.z).sum::<f32>() * 0.25;
        let z = p.iter().map(|q| q.z).sum::<f32>() * 0.25 + lift / c.z.abs();
        Mat4::from_cols(a.extend(0.0), b.extend(0.0), c.extend(0.0), Vec3::new(at.x, at.y, z).extend(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn part_views_follow_0x4a5510() {
        // Front bumper, cars and SUVs.
        let s = slot_shot(0x01, false, false, None).unwrap();
        assert_eq!((s.fov, s.steer), (24.0, 21.6723));
        assert_eq!(slot_shot(0x01, true, false, None).unwrap().fov, 30.0);
        // Decal slots share 0x22's record; vinyls go by group.
        assert_eq!(slot_shot(0x2B, false, false, None), slot_shot(0x22, false, false, None));
        assert_eq!(slot_shot(0x41, false, false, Some(0x14)), Some(VINYL_SHOTS[0][0]));
        assert_eq!(slot_shot(0x41, false, false, Some(0x17)), Some(VINYL_SHOTS[0][2]));
        assert_eq!(slot_shot(0x42, false, false, Some(0x01)), slot_shot(0x40, false, false, None));
        assert_eq!(slot_shot(0x0D, true, true, None), Some(*SUV_ENGINE_SHOT));
        assert_eq!(slot_shot(0x4B, false, false, None), None);
        // A view keeps the car where a part view left it on reset.
        let mut v = View::new(0);
        v.set_slot(0x1D, false, false, None, 0.0, 1.0 / 60.0);
        let angle = v.at(10.0).car_angle;
        assert!((angle.rem_euclid(360.0) - slot_shot(0x1D, false, false, None).unwrap().car_angle).abs() < 1e-3);
        v.reset(0, 10.0, 1.0 / 60.0);
        let w = v.at(20.0);
        assert!((w.camera.fov - SHOTS[0].fov).abs() < 1e-4 && w.camera.eye.abs_diff_eq(SHOTS[0].eye, 1e-4) && w.car_angle == angle);
        v.set_slot(0x4B, false, false, None, 20.0, 1.0 / 60.0);
        assert_eq!(v.at(30.0).car_angle, angle);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn part_views_blend_with_a_cosine_and_turn_the_short_way() {
        assert!(ease(0.0).abs() < 1e-6 && (ease(1.0) - 1.0).abs() < 1e-6);
        assert!((ease(0.5) - 0.5).abs() < 1e-4);
        // Base (305.77°) to the front wheel (31.22°): up through 360,
        // 85.4°, the turn the longest term at 60 frames a second:
        // 0.0055 · |305.77 − 31.22| = 1.51 s.
        let mut v = View::new(0);
        v.set_slot(0x1D, false, false, None, 0.0, 1.0 / 60.0);
        let to = slot_shot(0x1D, false, false, None).unwrap();
        let d = blend_duration(&SHOTS[0], &to, SHOTS[0].car_angle - to.car_angle, 1.0 / 60.0, PART_BLEND_MIN);
        assert!((d - 0.0055 * (SHOTS[0].car_angle - to.car_angle)).abs() < 1e-4, "{d}");
        let mid = v.at(d / 2.0);
        assert!((mid.car_angle - (SHOTS[0].car_angle + (360.0 - SHOTS[0].car_angle + to.car_angle) / 2.0)).abs() < 1e-3);
        assert!((mid.camera.fov - (SHOTS[0].fov + to.fov) / 2.0).abs() < 1e-3);
        let end = v.at(d).camera;
        assert!((end.fov - to.fov).abs() < 1e-4 && end.eye.abs_diff_eq(to.eye, 1e-4) && end.target.abs_diff_eq(to.target, 1e-4));
        // A car change turns the platform 120° over half a second and
        // brings the next disc to the front.
        let mut c = View::new(0);
        c.change_car(true, 0.0);
        assert_eq!((c.at(0.25).platform_angle, c.at(0.25).disc), (60.0, 1));
        // Both cars stand on their discs during the turn: the old one
        // rides out on disc 0 (which the platform turns with the car),
        // the new one in on disc 1; at the end only the new one.
        let mid = c.at(0.25);
        assert_eq!(mid.outgoing().map(|t| (t.disc, t.platform_angle)), Some((0, 60.0)));
        assert_eq!(mid.platform_turntable().disc, 0);
        assert_eq!(mid.turntable().disc, 1);
        assert_eq!(c.at(0.0).outgoing().map(|t| t.disc), Some(0));
        assert_eq!(c.at(0.5).outgoing(), None);
        assert_eq!(c.at(0.5).platform_turntable().disc, 1);
        assert_eq!(c.at(1.0).platform_angle, 120.0);
        let front = turn(120.0).transform_point3(DISCS[1]);
        assert!(front.x.abs() < 0.05 && front.y > 5.0);
        c.change_car(false, 2.0);
        assert_eq!((c.at(3.0).platform_angle, c.at(3.0).disc), (0.0, 0));
        // Back to the base view takes at least 2.5 s.
        v.reset(0, 5.0, 1.0 / 60.0);
        assert!(v.at(6.0).camera.eye != SHOTS[0].eye);
        assert!(v.at(7.5).camera.eye.abs_diff_eq(SHOTS[0].eye, 1e-4));
    }

    #[test]
    fn car_changes_wait_for_the_turn_so_the_car_ends_in_front() {
        // Steps at random moments (a few under the half-second turn): a
        // change asked during a turn waits for its end (GarageMain's
        // queued ride), so each turn is a whole third and the shown car
        // ends in front. Changing mid-turn would leave it off to a side.
        let mut v = View::new(0);
        let mut queued: Option<bool> = None;
        let mut rng = 0x2545_F491_u32;
        let mut t = 0.0;
        for _ in 0..400 {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            if rng % 7 < 3 {
                queued = Some(rng & 0x100 != 0);
            }
            if !v.changing(t)
                && let Some(up) = queued.take()
            {
                v.change_car(up, t);
            }
            t += 1.0 / 60.0;
            if !v.changing(t) {
                assert!(v.at(t).turntable().car_in_front(), "at {t}: {:?}", v.at(t).turntable());
            }
        }
        // Without the wait, a second change 0.25 s in leaves the car off
        // the front.
        let mut w = View::new(0);
        w.change_car(true, 0.0);
        w.change_car(true, 0.25);
        assert!(!w.at(2.0).turntable().car_in_front());
    }

    #[test]
    fn the_front_disc_faces_the_camera() {
        // Disc 0 stands between the eye and the target's far side, and a
        // 120° platform turn (0x4D82D0) brings disc 1 there.
        let shot = SHOTS[0];
        let ahead = (shot.target - shot.eye).normalize();
        assert!((DISCS[0] - shot.eye).normalize().dot(ahead) > 0.99);
        // (The table's discs are 5.4 and 5.42 m out, so to a few cm.)
        let turned = turn(120.0).transform_point3(DISCS[1]);
        assert!((turned - DISCS[0]).length() < 0.05, "{turned}");
    }

    #[test]
    fn the_car_sits_on_its_tyres_and_turns_with_its_angle() {
        let wheel = |x: f32, y: f32| Wheel { position: Vec3::new(x, y, 0.05), radius: 0.33, width: 0.2, face: y, face_wide: y };
        let wheels = [wheel(1.3, 0.75), wheel(1.3, -0.75), wheel(-1.3, -0.75), wheel(-1.3, 0.75)];
        let table = Turntable::default();
        let m = table.car_transform(&wheels);
        let at = m.w_axis.truncate();
        assert!((at - Vec3::new(0.0, 5.4, TYRE_HEIGHT + 0.28)).length() < 1e-4, "{at}");
        // 305.77° about +z: the nose (+x) points right and towards the eye.
        let nose = m.transform_vector3(Vec3::X);
        assert!(nose.x > 0.5 && nose.y < -0.8, "{nose}");
        // Every tyre's bottom at the tyre height.
        for w in &wheels {
            let bottom = m.transform_point3(w.position) - Vec3::Z * w.radius;
            assert!((bottom.z - TYRE_HEIGHT).abs() < 1e-4);
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn idle_tour_starts_after_300_seconds() {
        let mut v = View::new(0);
        v.tick(0.0, 1.0 / 60.0, false);
        v.tick(299.0, 1.0 / 60.0, false);
        assert_eq!(v.at(299.0).car_angle, SHOTS[0].car_angle);
        v.tick(301.0, 1.0 / 60.0, false);
        let end = v.at(330.0);
        let i = TOUR_INDEX.load(std::sync::atomic::Ordering::Relaxed);
        assert!((42..=48).contains(&i));
        let shot = TOUR_SHOTS[0][i - 42];
        assert!((end.car_angle.rem_euclid(360.0) - shot.car_angle).abs() < 0.01);
        assert_eq!(end.camera.fov, shot.fov);
        // A part view ends the tour.
        v.set_slot(0x40302010, false, false, None, 331.0, 1.0 / 60.0);
        assert!(!v.touring);
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn showroom_has_the_scene_models_and_an_envmap() {
        let root = crate::testkit::game_root();
        for name in &PLATFORMS[1..] {
            let p = Platform::load(&root, name).unwrap_or_else(|e| panic!("{name}: {e:#}"));
            assert!(p.envmap.is_some(), "{name}: no envmap");
            assert_eq!(p.world.placements.len(), 6, "{name}");
        }
    }

    /// GLOBALB.BUN's share of the platform scenes is
    /// parsed once a run, however many platforms load, and the kept value
    /// is what a fresh parse gives (emitters, templates, textures in order).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn platforms_parse_globalb_once_a_run() {
        use std::sync::atomic::Ordering;
        let root = crate::testkit::game_root();
        for name in PLATFORMS[1..].iter().chain(&PLATFORMS[1..]) {
            Platform::load(&root, name).unwrap_or_else(|e| panic!("{name}: {e:#}"));
        }
        assert_eq!(GLOBALB_RESIDENT_PARSES.load(Ordering::Relaxed), 1, "GLOBALB parsed once for every platform load");
        let picks = FRONT_PACK_READS.load(Ordering::Relaxed);
        for name in &PLATFORMS[1..] {
            Platform::load(&root, name).unwrap();
        }
        assert_eq!(FRONT_PACK_READS.load(Ordering::Relaxed), picks, "FrontB.lzc not read again for a platform seen before");
        // The kept picks are a fresh scan's, for every key a platform asked.
        let keys: Vec<_> = FRONT_PICKS_KEYS.lock().unwrap().clone();
        assert!(!keys.is_empty(), "no platform wanted FrontB textures");
        for wanted in keys {
            let kept = front_end_pack_picks(&root, &wanted).unwrap();
            let fresh = front_end_pack_scan(&root, &wanted).unwrap();
            assert_eq!(format!("{kept:?}"), format!("{fresh:?}"));
        }
        let kept = GlobalbResident::get(&root).unwrap();
        let fresh = GlobalbResident::parse(&root).unwrap();
        assert_eq!(format!("{:?}", kept.emitters), format!("{:?}", fresh.emitters));
        assert_eq!(format!("{:?}", kept.textures), format!("{:?}", fresh.textures));
        assert!(!kept.textures.is_empty() && !kept.emitters.descriptions.is_empty());
    }
}

/// Frontend view 4's reflected base pose (0x630EDB–0x630F17).
pub fn reflected_car_world(world: Mat4) -> Mat4 {
    let mut reflected = Mat4::from_scale(Vec3::new(1.0,1.0,-1.0)) * world;
    reflected.w_axis.z += 2.0 * TYRE_HEIGHT;
    reflected
}

#[cfg(test)]
mod reflection_plane_tests {
    use super::*;
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn reflected_base_and_mesh_bias_match_native_paired_world_view() {
        let bytes = &crate::testkit::private("docs/evidence/fe-reflection-plane/matrices.bin");
        let matrix = |offset: usize| Mat4::from_cols_array(&std::array::from_fn(|i| {
            f32::from_le_bytes(bytes[offset+i*4..offset+i*4+4].try_into().unwrap())
        }));
        let mut draw = reflected_car_world(matrix(0));
        // Car_Draw applies this after the frontend constructs the reflected base.
        draw.w_axis.z += 0.01;
        assert!(draw.abs_diff_eq(matrix(64), 0.000003), "actual {draw:?}; native {:?}",matrix(64));
    }
}

