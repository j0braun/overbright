//! Distance fog, as SPEED2.EXE computes it.
//!
//! Traced in the executable:
//!
//! * 0x614310 samples the "Normal Fog" and "Night Normal Fog" parameter maps
//!   at the camera and blends them by the time-of-day factor (0x60FA90:
//!   "Normal Fog" gets `w`, "Night Normal Fog" `1 − w`). Each row holds
//!   eight floats: `f0` fog strength, `f1` fog start distance, `f2..f4` fog
//!   colour (0–255), and `f5..f7`, which this path does not use.
//! * 0x5FA670 smooths the sample: when the camera has moved less than the
//!   blend distance (50 units, the "Fog Blend Distance" default) since the
//!   last sample, it moves only `moved / 50` of the way to the new value.
//! * 0x5CAAE0 turns that into the effect constant
//!   `FogValue = (250, 1 / (250 - f1), min(1 - f0, 0.6))` and FOGCOLOR
//!   `0.75 × (f4, f3, f2)` (red from `f4`; see [`fog_color`]).
//! * world.efx (vs_1_1) computes
//!   `fog = clamp((FogValue.x - view_depth) * FogValue.y, FogValue.z, 1)`,
//!   and D3D blends `colour * fog + FogColor * (1 - fog)`.

use glam::Vec2;

use crate::formats::hash::bin_hash;
use crate::formats::parameter_map::ParameterMap;
use crate::world::World;

const FOG_FAR: f32 = 250.0;
const MAX_FOG: f32 = 0.6;
const BLEND_DISTANCE: f32 = 50.0;

/// FOGCOLOR from the row's colour fields `f2..f4`.
///
/// 0x614310 truncates each (`_ftol`) and packs them as `0x80000000 |
/// f4 << 16 | f3 << 8 | f2` (0x614595…0x6145C2, out +0x14): `f4` is red and
/// `f2` blue. 0x5CAAE0 then scales each byte by 0.75 (0x785510), truncates
/// again and sets the result as FOGCOLOR (0x5CB0D1…0x5CB199). Checked against
/// an apitrace of the original at the airport ("Normal Fog" row 12: f2..f4 =
/// 132, 74.8, 99 → D3DRS_FOGCOLOR 0xFF493763, i.e. (73, 55, 99); 99 smoothed
/// to just under 99 truncates to 98 → 73).
pub fn fog_color(f2: f32, f3: f32, f4: f32) -> [f32; 3] {
    let byte = |f: f32| ((f as i32 & 0xFF) as f32 * 0.75) as i32 as f32 / 255.0;
    [byte(f4), byte(f3), byte(f2)]
}

/// The constants the world shader needs.
#[derive(Clone, Copy, Debug)]
pub struct Fog {
    /// `(far, 1 / (far - start), minimum fog factor)`.
    pub value: [f32; 3],
    /// RGB in 0..1.
    pub color: [f32; 3],
}

impl Fog {
    /// The fog of a view whose track has no fog maps (the front end's
    /// garage scenes). 0x614310 returns without writing its output
    /// (0x89E850, .bss, so zero from boot) when the "Normal Fog" / "Night
    /// Normal Fog" objects are not bound, and 0x5CAAE0 still builds the
    /// constants from it: `FogValue = (250, 1/250, min(1 − 0, 0.6))`, FOGCOLOR
    /// black. Checked against an apitrace of the original's Quick Race car
    /// select: c8 = (250, 0.004, 0.6), D3DRS_FOGCOLOR 0xFF000000, FOGENABLE
    /// on. That holds only before the first game session: see
    /// [`Fog::frontend`].
    pub const UNSAMPLED: Fog = Fog { value: [FOG_FAR, 1.0 / FOG_FAR, MAX_FOG], color: [0.0; 3] };

    /// The fog of a front-end / garage view (**verified** in
    /// memory under Wine, two sessions): 0x614310 stops writing 0x89E850
    /// when the track's fog maps are unbound, and the struct survives the
    /// unload, so the front end draws with the last in-game sample — the
    /// zero struct ([`Fog::UNSAMPLED`]) only until the first game session
    /// after start. The constants are built from it as from any sample
    /// (0x5CAAE0; **likely**: the same path as the verified zero case).
    pub fn frontend() -> Fog {
        Fog::frontend_from(*LAST_SAMPLE.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// [`Fog::frontend`] with 0x89E850's row given (phase 4: the game's
    /// kept [`FogState::last_sample`] instead of the process-wide static).
    pub fn frontend_from(row: Option<[f32; 5]>) -> Fog {
        row.map_or(Fog::UNSAMPLED, Fog::from_row)
    }

    /// 0x5CAAE0's constants from 0x614310's output row (strength, start,
    /// f2, f3, f4).
    fn from_row([strength, start, r, g, b]: [f32; 5]) -> Fog {
        Fog { value: [FOG_FAR, 1.0 / (FOG_FAR - start), (1.0 - strength).min(MAX_FOG)], color: fog_color(r, g, b) }
    }
}

/// 0x89E850: 0x614310's last output row (None: the zero struct of a fresh
/// start). Process-wide like the original's; reset only by a new start.
static LAST_SAMPLE: std::sync::Mutex<Option<[f32; 5]>> = std::sync::Mutex::new(None);

/// Fog that follows the camera with the game's smoothing.
#[derive(Clone)]
///
/// 0x614310 smooths each map on its own (one 0x5FA670 object per map and
/// view: "Normal Fog" at 0x8A1E90, "Night Normal Fog" at 0x8A31F0), then
/// mixes the two smoothed rows with the time-of-day weight
/// ([`crate::sky::TimeOfDay::fog_weight`]): `normal·w + night·(1 − w)`.
/// In the shipped game `w` is always 1 (see [`crate::sky`]), so Bayview's
/// fog is "Normal Fog" alone.
///
/// When it rains (view +0x64 → +0x1F0 > 0) 0x614310 also samples "Night
/// Raining Fog" (0x8A1E30 + 0x30·(view − 1)), smoothed over the distance
/// the "Fog Blend Distance" map gives (0x8A3250 + 0x1C·(view − 1), field
/// 0; 50 without it), and mixes every field `rain·raining + (1 −
/// rain)·normal` (0x6146C0…); the colour is truncated after the mix and
/// handed to the rain's tint as well (0x613030, see
/// [`crate::weather::tint_multiplier`]).
pub struct FogState {
    /// Smoothed rows of "Normal Fog" and "Night Normal Fog": strength,
    /// start, r, g, b (the fields this path reads).
    current: [Option<[f32; 5]>; 2],
    last_position: [Vec2; 2],
    /// "Night Raining Fog", smoothed only on frames with rain.
    raining: Option<[f32; 5]>,
    raining_position: Vec2,
    /// The mixed row's f2, f3, f4 of the last frame with rain.
    pub rain_color: [f32; 3],
    /// The fog the main view's world was drawn with this frame (None
    /// with the fog off), for effects drawn later (the volumetric smoke).
    pub drawn: Option<Fog>,
    /// 0x89E850 as this state last wrote it (tick form: the static
    /// [`LAST_SAMPLE`] as a field; the static is still written, and read by
    /// [`Fog::frontend`], until the game keeps this one).
    last_sample: Option<[f32; 5]>,
}

#[cfg(test)]
impl FogState {
    /// The whole smoothing state as bits (tests).
    fn dev_bits_raw(&self) -> Vec<u32> {
        let mut v = Vec::new();
        for r in self.current.iter().chain(std::iter::once(&self.raining)) {
            v.extend(r.map_or([u32::MAX; 5], |r| r.map(f32::to_bits)));
        }
        v.extend(self.last_position.iter().chain([&self.raining_position]).flat_map(|p| [p.x.to_bits(), p.y.to_bits()]));
        v.extend(self.rain_color.map(f32::to_bits));
        v
    }
}

#[cfg(feature = "dev-harness")]
impl FogState {
    /// Dev harness (rate gate, `state_hash` `vis`): the smoothed rows as bits.
    pub fn dev_bits(&self) -> Vec<u32> {
        let mut v = Vec::new();
        for r in self.current.iter().chain(std::iter::once(&self.raining)) {
            v.extend(r.map_or([u32::MAX; 5], |r| r.map(f32::to_bits)));
        }
        v.extend(self.rain_color.map(f32::to_bits));
        v
    }
}

impl Default for FogState {
    fn default() -> Self {
        FogState {
            current: [None; 2],
            last_position: [Vec2::ZERO; 2],
            raining: None,
            raining_position: Vec2::ZERO,
            rain_color: [0.0; 3],
            drawn: None,
            last_sample: None,
        }
    }
}

const MAPS: [&str; 2] = ["Normal Fog", "Night Normal Fog"];

impl FogState {
    /// Sample at `position` with the shipped game's weight (1: "Normal Fog").
    pub fn update(&mut self, world: &World, position: Vec2) -> Option<Fog> {
        self.update_weighted(world, position, 1.0)
    }

    /// Sample at `position`; `normal_weight` is 0x60FA90's factor.
    pub fn update_weighted(&mut self, world: &World, position: Vec2, normal_weight: f32) -> Option<Fog> {
        self.update_rain(world, position, normal_weight, 0.0)
    }

    /// As [`FogState::update_weighted`], blended toward "Night Raining
    /// Fog" by `rain` (+0x1F0). The per-frame path over the tick form: one
    /// [`FogState::step`], then the draw at α = 0.
    pub fn update_rain(&mut self, world: &World, position: Vec2, normal_weight: f32, rain: f32) -> Option<Fog> {
        let s = self.step(world, position, normal_weight, rain);
        FogSample::drawn(s.as_ref(), s.as_ref(), 0.0)
    }

    /// 0x89E850 as this state last wrote it (None: never sampled).
    pub fn last_sample(&self) -> Option<[f32; 5]> {
        self.last_sample
    }

    /// Tick form (frame-rate independence): one tick of 0x614310 at the **tick**
    /// camera's ground `position` (each map smoothed by t = min(|p_n −
    /// p_{n−1}| / 50, 1)), blended toward "Night Raining Fog" by `rain`.
    /// -> S_n (None: the track has no fog maps, nothing written).
    pub fn step(&mut self, world: &World, position: Vec2, normal_weight: f32, rain: f32) -> Option<FogSample> {
        let mut rows = [[0.0f32; 5]; 2];
        for (i, name) in MAPS.iter().enumerate() {
            let map: &ParameterMap = world.parameter_maps.get(&bin_hash(name))?;
            let row = map.sample(position);
            let target = [row[0], row[1], row[2], row[3], row[4]];
            let current = match self.current[i] {
                None => target,
                Some(old) => {
                    let moved = position.distance(self.last_position[i]);
                    let t = if moved < BLEND_DISTANCE { moved / BLEND_DISTANCE } else { 1.0 };
                    std::array::from_fn(|k| t * target[k] + (1.0 - t) * old[k])
                }
            };
            self.current[i] = Some(current);
            self.last_position[i] = position;
            rows[i] = current;
        }
        let w = normal_weight;
        let mut mixed: [f32; 5] = std::array::from_fn(|k| rows[0][k] * w + rows[1][k] * (1.0 - w));
        if rain > 0.0
            && let Some(map) = world.parameter_maps.get(&bin_hash("Night Raining Fog"))
        {
            let blend = world
                .parameter_maps
                .get(&bin_hash("Fog Blend Distance"))
                .map_or(BLEND_DISTANCE, |m| m.sample(position)[0]);
            let row = map.sample(position);
            let target = [row[0], row[1], row[2], row[3], row[4]];
            let raining = match self.raining {
                None => target,
                Some(old) => {
                    let moved = position.distance(self.raining_position);
                    let t = if moved < blend { moved / blend } else { 1.0 };
                    std::array::from_fn(|k| t * target[k] + (1.0 - t) * old[k])
                }
            };
            self.raining = Some(raining);
            self.raining_position = position;
            mixed = std::array::from_fn(|k| rain * raining[k] + (1.0 - rain) * mixed[k]);
            self.rain_color = [mixed[2] as i32 as f32, mixed[3] as i32 as f32, mixed[4] as i32 as f32];
        }
        *LAST_SAMPLE.lock().unwrap_or_else(|e| e.into_inner()) = Some(mixed);
        self.last_sample = Some(mixed);
        Some(FogSample { row: mixed, rain_color: self.rain_color })
    }
}

/// The fog's part of a tick snapshot: 0x614310's mixed row (strength,
/// start, f2, f3, f4) and the rain tint's colour as it persists.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FogSample {
    pub row: [f32; 5],
    pub rain_color: [f32; 3],
}

impl FogSample {
    /// The fog a frame at α between ticks `prev` (S_{n−1}) and `cur` (S_n)
    /// draws: the row interpolated (`prev` exactly at α = 0), then
    /// 0x5CAAE0's constants and truncations. Whether there is fog at all
    /// is held at `prev`.
    pub fn drawn(prev: Option<&FogSample>, cur: Option<&FogSample>, alpha: f32) -> Option<Fog> {
        let p = prev?;
        let c = cur.unwrap_or(p);
        Some(Fog::from_row(super::tick::lerp_n(p.row, c.row, alpha)))
    }

    /// The rain tint's colour a drawn frame uses: truncated values, held at
    /// `prev` (a discrete value, §1.3).
    pub fn drawn_rain_color(prev: &FogSample, _cur: &FogSample, _alpha: f32) -> [f32; 3] {
        prev.rain_color
    }
}

#[cfg(test)]
mod tests {
    use super::fog_color;
    use super::*;
    use crate::render::native::fx::tick::{testkit, within};

    /// The per-frame code before the tick form (verbatim but for the
    /// static's write), the oracle.
    fn legacy_update_rain(
        s: &mut FogState,
        world: &World,
        position: Vec2,
        normal_weight: f32,
        rain: f32,
    ) -> Option<Fog> {
        let mut rows = [[0.0f32; 5]; 2];
        for (i, name) in MAPS.iter().enumerate() {
            let map: &ParameterMap = world.parameter_maps.get(&bin_hash(name))?;
            let row = map.sample(position);
            let target = [row[0], row[1], row[2], row[3], row[4]];
            let current = match s.current[i] {
                None => target,
                Some(old) => {
                    let moved = position.distance(s.last_position[i]);
                    let t = if moved < BLEND_DISTANCE { moved / BLEND_DISTANCE } else { 1.0 };
                    std::array::from_fn(|k| t * target[k] + (1.0 - t) * old[k])
                }
            };
            s.current[i] = Some(current);
            s.last_position[i] = position;
            rows[i] = current;
        }
        let w = normal_weight;
        let mut mixed: [f32; 5] = std::array::from_fn(|k| rows[0][k] * w + rows[1][k] * (1.0 - w));
        if rain > 0.0
            && let Some(map) = world.parameter_maps.get(&bin_hash("Night Raining Fog"))
        {
            let blend = world
                .parameter_maps
                .get(&bin_hash("Fog Blend Distance"))
                .map_or(BLEND_DISTANCE, |m| m.sample(position)[0]);
            let row = map.sample(position);
            let target = [row[0], row[1], row[2], row[3], row[4]];
            let raining = match s.raining {
                None => target,
                Some(old) => {
                    let moved = position.distance(s.raining_position);
                    let t = if moved < blend { moved / blend } else { 1.0 };
                    std::array::from_fn(|k| t * target[k] + (1.0 - t) * old[k])
                }
            };
            s.raining = Some(raining);
            s.raining_position = position;
            mixed = std::array::from_fn(|k| rain * raining[k] + (1.0 - rain) * mixed[k]);
            s.rain_color = [mixed[2] as i32 as f32, mixed[3] as i32 as f32, mixed[4] as i32 as f32];
        }
        s.last_sample = Some(mixed);
        Some(Fog::from_row(mixed))
    }

    fn bits(f: Option<Fog>) -> Option<([u32; 3], [u32; 3])> {
        f.map(|f| (f.value.map(f32::to_bits), f.color.map(f32::to_bits)))
    }

    fn state_bits(s: &FogState) -> Vec<u32> {
        let mut v = s.dev_bits_raw();
        v.extend(s.last_sample.into_iter().flatten().map(f32::to_bits));
        v
    }

    /// 600 ticks stepped = 600 frames of the per-frame code, bit for bit
    /// (rain on and off, a moving and jumping camera); α = 0 draws the old
    /// output; between ticks the row lies within the two snapshots.
    #[test]
    fn tick_form_is_the_per_frame_path() {
        let normal: [&[f32]; 4] = [
            &[0.3, 40.0, 132.0, 74.8, 98.99],
            &[0.8, 120.0, 10.0, 20.0, 30.0],
            &[0.1, 0.0, 255.0, 128.5, 3.3],
            &[0.55, 80.0, 60.0, 61.0, 62.0],
        ];
        let night: [&[f32]; 4] = [
            &[0.2, 10.0, 1.0, 2.0, 3.0],
            &[0.9, 200.0, 100.0, 90.0, 80.0],
            &[0.4, 50.0, 40.0, 50.0, 60.0],
            &[0.0, 0.0, 0.0, 0.0, 0.0],
        ];
        let raining: [&[f32]; 4] = [
            &[0.6, 30.0, 70.0, 80.0, 90.0],
            &[0.7, 60.0, 11.0, 12.0, 13.0],
            &[0.65, 20.0, 200.0, 190.0, 180.0],
            &[0.3, 90.0, 33.0, 44.0, 55.0],
        ];
        let blend: [&[f32]; 4] = [&[30.0], &[50.0], &[80.0], &[20.0]];
        let world = testkit::world(&[
            ("Normal Fog", normal),
            ("Night Normal Fog", night),
            ("Night Raining Fog", raining),
            ("Fog Blend Distance", blend),
        ]);
        for w in [1.0f32, 0.4] {
            let (mut new, mut old) = (FogState::default(), FogState::default());
            let mut prev: Option<FogSample> = None;
            for i in 0..600 {
                let p = testkit::path(i);
                let rain = if (i / 90) % 2 == 1 { 0.25 + (i % 90) as f32 / 120.0 } else { 0.0 };
                let want = legacy_update_rain(&mut old, &world, p, w, rain);
                let mut per_frame = new.clone();
                assert_eq!(bits(per_frame.update_rain(&world, p, w, rain)), bits(want), "frame {i}");
                let cur = new.step(&world, p, w, rain);
                assert_eq!(state_bits(&new), state_bits(&old), "tick {i}");
                assert_eq!(bits(FogSample::drawn(cur.as_ref(), cur.as_ref(), 0.0)), bits(want));
                let cur = cur.unwrap();
                if let Some(prev) = prev {
                    assert_eq!(
                        bits(FogSample::drawn(Some(&prev), Some(&cur), 0.0)),
                        bits(Fog::frontend_from(Some(prev.row)).into())
                    );
                    for a in testkit::ALPHAS {
                        let row = crate::render::native::fx::tick::lerp_n(prev.row, cur.row, a);
                        for k in 0..5 {
                            assert!(within(row[k], prev.row[k], cur.row[k]));
                        }
                        let d = FogSample::drawn(Some(&prev), Some(&cur), a).unwrap();
                        assert!(within(d.value[2], (1.0 - prev.row[0]).min(MAX_FOG), (1.0 - cur.row[0]).min(MAX_FOG)));
                    }
                }
                prev = Some(cur);
            }
            assert_eq!(new.last_sample(), Some(prev.unwrap().row));
        }
    }

    /// The front end draws with 0x89E850 as the last game
    /// session left it (zero only before any).
    #[test]
    fn the_front_end_keeps_the_last_in_game_sample() {
        use super::Fog;
        assert_eq!(Fog::frontend_from(None).value, Fog::UNSAMPLED.value);
        assert_eq!(Fog::frontend_from(None).color, [0.0; 3]);
        let f = Fog::frontend_from(Some([0.5, 50.0, 132.0, 74.8, 98.99]));
        assert_eq!(f.value, [250.0, 1.0 / 200.0, 0.5]);
        assert_eq!(f.color, fog_color(132.0, 74.8, 98.99));
    }

    #[test]
    fn fog_color_is_bgr_scaled_like_the_trace() {
        // "Normal Fog" row 12 at the airport; the original set 0xFF493763.
        let c = fog_color(132.0, 74.8, 98.99);
        let bytes = c.map(|v| (v * 255.0).round() as u32);
        assert_eq!(bytes, [73, 55, 99]);
    }
}
