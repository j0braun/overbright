//! The cars' ground effects: the tyre smoke, the wheel spin, the drive
//! spray of puddles and deep water and the bottom-out sparks (each car's
//! update 0x5F5740, the first part, before the damage smoke and the
//! nitrous flame). **Verified** 2026-11-26 (REA decompile of 0x5F5740,
//! 0x61FB10 and 0x615CD0; the disassembly 0x5F57D0..0x5F5DBB and the
//! marker fill 0x639118..0x6395A0).
//!
//! Every update, per car within 150 m of the listener (0x60A1BB:
//! 0x5F1170, 0x7A5498 = 150, the distance handed in), [`GroundFx::update`]
//! fires the car effect kinds 1–5 through 0x61FB10. Kinds 1–5 do not use
//! the car's own emitter lists: 0x615CD0 takes the surface table's list
//! (0x883E68 − 0xE8 + (kind · 29 + surface) · 8), that is event kind − 1 of
//! the surface ([`SURFACE_EVENTS`]: 0 `_DR` / `_DRSL`, 1 `_DR`, 2 `_SK`,
//! 3 `_SP`, 4 `XEBO_`), whose emitters 0x5E0630 cloned from the loaded
//! templates of the named group (0x48B8B0). Each emitter bursts −intensity
//! (0x491E50: 1 + ⌊intensity · rate⌋ particles) at every marker of its
//! group ([`marker`]).

use super::*;

/// The surface table's tyre events (0x800EE0 + 0x98·s, event e at
/// +0x44 + 0xC·e: the group name, a pointer, then the 0..1 range's low and
/// high ends). An empty name (the pointer 0x783FE5) is `None`; its range
/// still gates the skid timer (event 2).
pub const SURFACE_EVENTS: [[(Option<&str>, f32, f32); 5]; 29] = [
    // 0 NONE
    [(None, 50.0, 100.0), (None, 50.0, 100.0), (None, 50.0, 100.0), (None, 50.0, 100.0), (None, 50.0, 100.0)],
    // 1 ROAD
    [
        (Some("XETT_ROAD_DR"), 20.0, 100.0),
        (None, 50.0, 100.0),
        (Some("XETT_ROAD_SK"), 5.0, 40.0),
        (Some("XETT_ROAD_SP"), 2.0, 200.0),
        (Some("XEBO_ROAD"), 1.0, 2.0),
    ],
    // 2 ROAD_WET
    [
        (Some("XETT_ROAD_DR"), 20.0, 100.0),
        (None, 50.0, 100.0),
        (Some("XETT_ROAD_SK"), 5.0, 40.0),
        (Some("XETT_ROAD_SP"), 2.0, 200.0),
        (Some("XEBO_ROAD"), 1.0, 2.0),
    ],
    // 3 ROAD_DRIFT
    [
        (Some("XETT_ROAD_DR"), 20.0, 100.0),
        (None, 50.0, 100.0),
        (Some("XETT_ROAD_SK"), 5.0, 40.0),
        (Some("XETT_ROAD_SP"), 2.0, 200.0),
        (Some("XEBO_ROAD"), 1.0, 2.0),
    ],
    // 4 ROAD_SMOKE_1
    [
        (None, 20.0, 100.0),
        (None, 50.0, 100.0),
        (Some("XEM_ROADSKBLUE"), 5.0, 40.0),
        (None, 2.0, 200.0),
        (Some("XEBO_ROAD"), 1.0, 2.0),
    ],
    // 5 ROAD_SMOKE_2
    [
        (None, 20.0, 100.0),
        (None, 50.0, 100.0),
        (Some("XEM_ROADSKYELW"), 5.0, 40.0),
        (None, 2.0, 200.0),
        (Some("XEBO_ROAD"), 1.0, 2.0),
    ],
    // 6 ROAD_SMOKE_3
    [
        (None, 20.0, 100.0),
        (None, 50.0, 100.0),
        (Some("XEM_ROADSKRED"), 5.0, 40.0),
        (None, 2.0, 200.0),
        (Some("XEBO_ROAD"), 1.0, 2.0),
    ],
    // 7 ROAD_BRIDGE
    [
        (Some("XETT_ROAD_DR"), 20.0, 100.0),
        (None, 50.0, 100.0),
        (Some("XETT_ROAD_SK"), 5.0, 40.0),
        (Some("XETT_ROAD_SP"), 2.0, 200.0),
        (Some("XEBO_ROAD"), 1.0, 2.0),
    ],
    // 8 DIRT
    [
        (None, 3.0, 30.0),
        (Some("XETT_DIRTRD_DR"), 35.0, 100.0),
        (Some("XETT_DIRTRD_SK"), 5.0, 20.0),
        (Some("XETT_DIRTRD_SP"), 1.5, 10.0),
        (Some("XEBO_DIRT"), 1.0, 2.0),
    ],
    // 9 GRAVEL
    [
        (None, 3.0, 30.0),
        (Some("XETT_DIRTRD_DR"), 35.0, 100.0),
        (Some("XETT_DIRTRD_SK"), 5.0, 20.0),
        (Some("XETT_DIRTRD_SP"), 1.5, 10.0),
        (Some("XEBO_DIRT"), 1.0, 2.0),
    ],
    // 10 ROUGH_ROAD
    [
        (Some("XETT_RRCSSW_DR"), 15.0, 100.0),
        (None, 50.0, 100.0),
        (Some("XETT_RRCSSW_SK"), 5.0, 20.0),
        (Some("XETT_RRCSSW_SP"), 2.0, 100.0),
        (Some("XEBO_ROAD"), 1.0, 2.0),
    ],
    // 11 COBBLESTONE
    [
        (Some("XETT_RRCSSW_DR"), 15.0, 100.0),
        (None, 50.0, 100.0),
        (Some("XETT_RRCSSW_SK"), 5.0, 20.0),
        (Some("XETT_RRCSSW_SP"), 2.0, 100.0),
        (Some("XEBO_ROAD"), 1.0, 2.0),
    ],
    // 12 STAIRS
    [
        (Some("XETT_RRCSSW_DR"), 15.0, 100.0),
        (None, 50.0, 100.0),
        (Some("XETT_RRCSSW_SK"), 5.0, 20.0),
        (Some("XETT_RRCSSW_SP"), 2.0, 100.0),
        (Some("XEBO_ROAD"), 1.0, 2.0),
    ],
    // 13 PUDDLE
    [
        (Some("XETT_PUDDLE_DRSL"), 3.0, 30.0),
        (Some("XETT_PUDDLE_DR"), 35.0, 100.0),
        (Some("XETT_PUDDLE_SK"), 5.0, 20.0),
        (Some("XETT_PUDDLE_SP"), 1.0, 100.0),
        (None, 0.6000000238418579, 3.0),
    ],
    // 14 DEEP_WATER
    [
        (Some("XETT_DEEPWATER_DRSL"), 3.0, 30.0),
        (Some("XETT_DEEPWATER_DR"), 35.0, 100.0),
        (Some("XETT_DEEPWATER_SK"), 5.0, 20.0),
        (Some("XETT_DEEPWATER_SP"), 1.0, 100.0),
        (None, 0.6000000238418579, 3.0),
    ],
    // 15 GRASS
    [
        (None, 3.0, 30.0),
        (Some("XETT_XETT_GRASS_DR"), 35.0, 100.0),
        (Some("XETT_GRASS_SK"), 5.0, 20.0),
        (Some("XETT_GRASS_SP"), 2.0, 200.0),
        (Some("XEBO_DIRT"), 1.0, 2.0),
    ],
    // 16 SIDEWALK
    [
        (Some("XETT_RRCSSW_DR"), 15.0, 100.0),
        (None, 50.0, 100.0),
        (Some("XETT_RRCSSW_SK"), 5.0, 20.0),
        (Some("XETT_RRCSSW_SP"), 2.0, 100.0),
        (Some("XEBO_ROAD"), 1.0, 2.0),
    ],
    // 17 WOOD
    [
        (None, 20.0, 100.0),
        (None, 50.0, 100.0),
        (None, 20.0, 20.0),
        (None, 20.0, 100.0),
        (None, 0.6000000238418579, 3.0),
    ],
    // 18 PLASTIC
    [
        (None, 20.0, 100.0),
        (None, 50.0, 100.0),
        (None, 20.0, 20.0),
        (None, 20.0, 100.0),
        (None, 0.6000000238418579, 3.0),
    ],
    // 19 GLASS
    [
        (None, 20.0, 100.0),
        (None, 50.0, 100.0),
        (None, 20.0, 20.0),
        (None, 20.0, 100.0),
        (None, 0.6000000238418579, 3.0),
    ],
    // 20 SOLID_WALL
    [
        (None, 20.0, 100.0),
        (None, 50.0, 100.0),
        (None, 20.0, 100.0),
        (None, 20.0, 100.0),
        (None, 0.6000000238418579, 3.0),
    ],
    // 21 SEE_THROUGH_WALL
    [
        (None, 20.0, 100.0),
        (None, 50.0, 100.0),
        (None, 20.0, 100.0),
        (None, 20.0, 100.0),
        (None, 0.6000000238418579, 3.0),
    ],
    // 22 PLANT
    [(None, 15.0, 100.0), (None, 50.0, 100.0), (None, 2.0, 100.0), (None, 5.0, 100.0), (None, 0.6000000238418579, 3.0)],
    // 23 POST
    [(None, 15.0, 100.0), (None, 50.0, 100.0), (None, 2.0, 100.0), (None, 5.0, 100.0), (None, 0.6000000238418579, 3.0)],
    // 24 PILLAR
    [(None, 15.0, 100.0), (None, 50.0, 100.0), (None, 2.0, 100.0), (None, 5.0, 100.0), (None, 0.6000000238418579, 3.0)],
    // 25 METAL_GRATE
    [(None, 15.0, 100.0), (None, 50.0, 100.0), (None, 2.0, 100.0), (None, 5.0, 100.0), (None, 0.6000000238418579, 3.0)],
    // 26 METAL
    [(None, 15.0, 100.0), (None, 50.0, 100.0), (None, 2.0, 100.0), (None, 5.0, 100.0), (None, 0.6000000238418579, 3.0)],
    // 27 CHAINLINK
    [(None, 15.0, 100.0), (None, 50.0, 100.0), (None, 2.0, 100.0), (None, 5.0, 100.0), (None, 0.6000000238418579, 3.0)],
    // 28 CAR
    [(None, 15.0, 100.0), (None, 50.0, 100.0), (None, 2.0, 100.0), (None, 5.0, 100.0), (None, 0.6000000238418579, 3.0)],
];

/// A surface's tyre events (row 0, NONE, for an index past the table).
pub fn surface_events(surface: u8) -> &'static [(Option<&'static str>, f32, f32); 5] {
    SURFACE_EVENTS.get(surface as usize).unwrap_or(&SURFACE_EVENTS[0])
}

/// A wheel as 0x5F5740 reads it from the car.
#[derive(Clone, Copy, Debug, Default)]
pub struct WheelIn {
    /// +0x390 + i: the wheel touches the ground.
    pub on_ground: bool,
    /// +0x394 + 4i: the surface under it.
    pub surface: u8,
    /// +0x3B4 + 8i: the slip the wheel exports (along, 0 while the tyre
    /// grips) and the speed across it, m/s.
    pub slip: f32,
    pub lateral: f32,
    /// +0x330 + 4i: the wheel's angular speed (rad/s; only its sign and
    /// zero are read).
    pub spin: f32,
}

/// The car as 0x5F5740 reads it.
#[derive(Clone, Copy, Debug, Default)]
pub struct CarIn {
    pub wheels: [WheelIn; 4],
    /// +0x430: |v|, m/s.
    pub speed: f32,
    /// +0x370..+0x37C: the bump stops' impulses this update, summed.
    pub bump: f32,
    /// The surface under the car's centre of mass (0x5E72F0 on +0x60);
    /// `None` when the query finds no ground.
    pub centre_surface: Option<u8>,
    /// The handed-in distance from the listener (0x5F1170), m.
    pub distance: f32,
}

/// The velocity a fire hands its emitters (0x61FB10's fourth argument,
/// else the car's +0x70).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FireVelocity {
    /// The car's velocity.
    Car,
    /// A vector in car space turned into the world by the body's rotation
    /// (0x441000 on +0xD0, its translation row zero), nothing added.
    Body(Vec3),
    /// A vector in the wheel's space turned by the wheel's matrix (+0x130 +
    /// 0x40·i), plus the car's velocity (+0x70).
    WheelPlusCar(Vec3),
}

/// One car effect fire (0x61FB10(kind, intensity, parameter, velocity)).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fire {
    /// 1 drive (event 0), 2 drive fast (event 1), 3 skid (event 2), 4 wheel
    /// spin (event 3), 5 bottom-out (event 4).
    pub kind: u8,
    pub intensity: f32,
    /// The surface whose list fires (the parameter's low word).
    pub surface: u8,
    /// Kinds 3 and 4: the wheel, whose marker group 5 + wheel replaces the
    /// emitters' own (the parameter's high word, 0x61FB10).
    pub wheel: Option<usize>,
    pub velocity: FireVelocity,
}

impl Fire {
    /// The surface event the kind's list is (kind − 1).
    pub fn event(&self) -> usize {
        self.kind as usize - 1
    }
}

/// What 0x5F5740 keeps per car between updates: each wheel's skid time
/// (+0x99C + 4i, s) and its last skid intensity (+0x3E4 + 4i, read by no
/// effect here).
#[derive(Clone, Copy, Debug, Default)]
pub struct GroundFx {
    pub skid_time: [f32; 4],
    pub skid: [f32; 4],
}

/// The skid's ramp by how long the wheel has slid (0x5F58BC..0x5F591A:
/// 0x7A5624 0.15, 0x7A5628 0.7, 0x7A562C 2, 4): × 0, 0.25, 0.5, 0.75, then 1.
pub fn skid_ramp(time: f32) -> f32 {
    if time < 0.15 {
        0.0
    } else if time < 0.7 {
        0.25
    } else if time < 2.0 {
        0.5
    } else if time < 4.0 {
        0.75
    } else {
        1.0
    }
}

fn ramp(x: f32, lo: f32, hi: f32) -> f32 {
    ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
}

impl GroundFx {
    /// One car update's fires, in the order 0x5F5740 makes them; `dt` is the
    /// game clock's step since the car's last update (+0x9AC).
    pub fn update(&mut self, car: &CarIn, dt: f32) -> Vec<Fire> {
        let mut out = Vec::new();
        let mut down = 0;
        for (i, w) in car.wheels.iter().enumerate() {
            if !w.on_ground {
                continue;
            }
            down += 1;
            let row = surface_events(w.surface);
            // The skid (event 2): the exported pair's length past the low
            // end, ramped by the time the wheel has slid that hard.
            let (name, lo, hi) = row[2];
            let slide = w.slip.hypot(w.lateral);
            if car.distance >= 150.0 || slide <= lo {
                self.skid_time[i] = 0.0;
            } else {
                self.skid_time[i] += dt;
                if name.is_some() {
                    let k = ((slide - lo) / (hi - lo) * skid_ramp(self.skid_time[i])).clamp(0.0, 1.0);
                    self.skid[i] = k;
                    if k > 0.0 {
                        // 0x5F592A..0x5F5975: a turning wheel throws the
                        // smoke along the car's axis, against its turn,
                        // at 50 m/s × the intensity; a still one hands
                        // the car's velocity.
                        let velocity = if w.spin == 0.0 {
                            FireVelocity::Car
                        } else {
                            FireVelocity::Body(Vec3::X * if w.spin > 0.0 { -50.0 * k } else { 50.0 * k })
                        };
                        out.push(Fire { kind: 3, intensity: k, surface: w.surface, wheel: Some(i), velocity });
                    }
                }
            }
            // The wheel spin (event 3): the rear wheels only (0x5F59AF /
            // 0x5F59B9 skip the parameters 0x50000 and 0x60000), within 40 m
            // (0x7A5600 is the skid's 150 m; 0x5F59A4 compares 40), the
            // exported slip itself (signed) past the low end.
            let (name, lo, hi) = row[3];
            if car.distance < 40.0 && i >= 2 && w.slip > lo && name.is_some() {
                let k = ramp(w.slip, lo, hi);
                if k > 0.0 {
                    out.push(Fire {
                        kind: 4,
                        intensity: k,
                        surface: w.surface,
                        wheel: Some(i),
                        velocity: FireVelocity::WheelPlusCar(Vec3::new(w.slip, w.lateral, 0.0)),
                    });
                }
            }
        }
        // With three wheels down or more: the drive events by the speed in
        // mph (× 2.237) on the surface under the car, and the bottom-out.
        if down > 2 {
            // 0x5E72F0 writes the surface into the wheel count's variable;
            // with no ground found the count stays (**inferred**: the query
            // leaves it untouched then).
            let surface = car.centre_surface.unwrap_or(down as u8);
            let row = surface_events(surface);
            let mph = car.speed * 2.237;
            let (n0, lo0, hi0) = row[0];
            let (n1, lo1, hi1) = row[1];
            let mut a = if n0.is_some() { ramp(mph, lo0, hi0) } else { 0.0 };
            let mut b = if n1.is_some() { ramp(mph, lo1, hi1) } else { 0.0 };
            if a + b > 0.0 {
                // Past a sum of 1 the event with the higher low end gives up
                // twice the excess (0x5F5C80..0x5F5CC2).
                let over = a + b - 1.0;
                if over > 0.0 {
                    if lo1 <= lo0 {
                        b = (b - 2.0 * over).max(0.0);
                    } else {
                        a = (a - 2.0 * over).max(0.0);
                    }
                }
                if a > 0.0 {
                    out.push(Fire { kind: 1, intensity: a, surface, wheel: None, velocity: FireVelocity::Car });
                }
                if b > 0.0 {
                    out.push(Fire { kind: 2, intensity: b, surface, wheel: None, velocity: FireVelocity::Car });
                }
            }
            // The bottom-out (event 4): the bump stops' sum (0x5F5D11..);
            // its name is not tested, an empty list fires nothing.
            let (_, lo, hi) = row[4];
            let k = ramp(car.bump, lo, hi);
            if k > 0.0 {
                out.push(Fire { kind: 5, intensity: k, surface, wheel: None, velocity: FireVelocity::Car });
            }
        }
        out
    }
}

/// The car's marker of a group as 0x639118..0x6395A0 file them, in car
/// space from the wheels' positions (the ride's physics block, 0x5B5360(1)
/// +0x10 + 0x30·i; the flag bits cleared, so never mirrored): 0 the origin,
/// 1 the front axle's middle (wheels 0, 1), 2 the rear's (2, 3), 3 the side
/// of wheels 0 and 3, 4 that of 1 and 2, 5..8 each wheel. Other groups
/// (the exhausts 10, the purge's 28 / 29) are not ground effect markers.
pub fn marker(group: u32, wheels: &[Vec3; 4]) -> Option<Vec3> {
    let mid = |a: usize, b: usize| (wheels[a] + wheels[b]) * 0.5;
    Some(match group {
        0 => Vec3::ZERO,
        1 => mid(0, 1),
        2 => mid(3, 2),
        3 => mid(0, 3),
        4 => mid(2, 1),
        5..=8 => wheels[group as usize - 5],
        _ => return None,
    })
}

/// The particles one burst of −`intensity` spawns from a fresh or burst
/// emitter at `rate` (0x491E50 with no wait: one, then one more each
/// 1 / rate of the intensity; a rate not over 0 waits 0.1).
pub fn burst_count(intensity: f32, rate: f32) -> usize {
    let wait = if rate <= 0.0 { 0.1 } else { (1.0 / rate).clamp(0.0, 240.0) };
    if intensity <= 0.0 {
        return 0;
    }
    let (mut left, mut n) = (intensity, 1);
    while wait <= left {
        left -= wait;
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rolling(surface: u8, slip: f32, lateral: f32) -> WheelIn {
        WheelIn { on_ground: true, surface, slip, lateral, spin: 30.0 }
    }

    fn car(wheels: [WheelIn; 4], speed: f32) -> CarIn {
        CarIn { wheels, speed, bump: 0.0, centre_surface: Some(wheels[0].surface), distance: 5.0 }
    }

    /// The table as the EXE holds it (rows ROAD and PUDDLE).
    #[test]
    fn surface_rows_match_the_exe() {
        assert_eq!(SURFACE_EVENTS[1][2], (Some("XETT_ROAD_SK"), 5.0, 40.0));
        assert_eq!(SURFACE_EVENTS[1][3], (Some("XETT_ROAD_SP"), 2.0, 200.0));
        assert_eq!(SURFACE_EVENTS[1][4], (Some("XEBO_ROAD"), 1.0, 2.0));
        assert_eq!(SURFACE_EVENTS[13][0], (Some("XETT_PUDDLE_DRSL"), 3.0, 30.0));
        assert_eq!(SURFACE_EVENTS[13][1], (Some("XETT_PUDDLE_DR"), 35.0, 100.0));
        assert_eq!(SURFACE_EVENTS[14][1], (Some("XETT_DEEPWATER_DR"), 35.0, 100.0));
        assert_eq!(SURFACE_EVENTS[2][2].0, Some("XETT_ROAD_SK"), "ROAD_WET smokes as ROAD");
    }

    /// The skid fires only after 0.15 s of sliding past 5 m/s, its
    /// intensity the slide's share of 5..40 m/s times the time ramp.
    #[test]
    fn skid_ramps_with_time() {
        let mut fx = GroundFx::default();
        let c = car([rolling(1, 0.0, 22.5); 4], 20.0);
        let dt = 1.0 / 60.0;
        let mut first = None;
        for step in 0..300 {
            let fires = fx.update(&c, dt);
            let skids: Vec<_> = fires.iter().filter(|f| f.kind == 3).collect();
            if !skids.is_empty() && first.is_none() {
                first = Some(step);
                assert_eq!(skids.len(), 4);
                assert!((skids[0].intensity - 0.5 * 0.25).abs() < 1e-5, "{}", skids[0].intensity);
            }
        }
        // 0.15 s at 1/60: the ninth or tenth update (the summed time's rounding).
        assert!(matches!(first, Some(8 | 9)), "{first:?}");
        assert!((fx.skid[0] - 0.5).abs() < 1e-5, "after 5 s of sliding the ramp is 1");
        // A gripping wheel (slide at the low end) starts over.
        fx.update(&car([rolling(1, 0.0, 5.0); 4], 20.0), dt);
        assert_eq!(fx.skid_time, [0.0; 4]);
    }

    /// The wheel spin: the rear wheels only, past 2 m/s of exported slip,
    /// within 40 m of the listener; thrown with the slip plus the car's
    /// velocity.
    #[test]
    fn wheel_spin_on_the_rear_wheels() {
        let mut fx = GroundFx::default();
        let mut c = car([rolling(1, 12.0, 0.0); 4], 1.0);
        let fires = fx.update(&c, 1.0 / 60.0);
        let spins: Vec<_> = fires.iter().filter(|f| f.kind == 4).collect();
        assert_eq!(spins.iter().map(|f| f.wheel.unwrap()).collect::<Vec<_>>(), vec![2, 3]);
        assert!((spins[0].intensity - 10.0 / 198.0).abs() < 1e-6);
        assert_eq!(spins[0].velocity, FireVelocity::WheelPlusCar(Vec3::new(12.0, 0.0, 0.0)));
        c.distance = 45.0;
        assert!(fx.update(&c, 1.0 / 60.0).iter().all(|f| f.kind != 4));
        // Locked wheels (negative slip) do not spin smoke.
        let mut fx = GroundFx::default();
        assert!(fx.update(&car([rolling(1, -12.0, 0.0); 4], 10.0), 1.0 / 60.0).iter().all(|f| f.kind != 4));
    }

    /// A puddle sprays by the car's speed: DRSL from 3 mph, DR from 35;
    /// dry road has no drive effect group with emitters, but fires kind 1
    /// (XETT_ROAD_DR, an empty group) from 20 mph.
    #[test]
    fn puddle_spray_follows_the_speed() {
        let mut fx = GroundFx::default();
        let slow = fx.update(&car([rolling(13, 0.0, 0.0); 4], 15.0 / 2.237), 1.0 / 60.0);
        assert_eq!(slow.len(), 1);
        assert_eq!((slow[0].kind, slow[0].surface), (1, 13));
        assert!((slow[0].intensity - 12.0 / 27.0).abs() < 1e-5);
        let fast = fx.update(&car([rolling(13, 0.0, 0.0); 4], 60.0 / 2.237), 1.0 / 60.0);
        let kinds: Vec<_> = fast.iter().map(|f| (f.kind, (f.intensity * 1000.0).round())).collect();
        // DRSL is 1 (clamped), DR 25/65; twice their sum's excess comes off
        // DRSL, the event with the lower low end.
        assert_eq!(kinds, vec![(1, 231.0), (2, 385.0)]);
        let faster = fx.update(&car([rolling(13, 0.0, 0.0); 4], 80.0 / 2.237), 1.0 / 60.0);
        assert_eq!(faster.iter().map(|f| f.kind).collect::<Vec<_>>(), vec![2], "DR alone from about 70 mph");
        // Two wheels down: no drive effects.
        let mut w = [rolling(13, 0.0, 0.0); 4];
        w[0].on_ground = false;
        w[1].on_ground = false;
        assert!(fx.update(&car(w, 20.0), 1.0 / 60.0).is_empty());
    }

    /// A hard landing's bump stops fire the bottom-out (XEBO_ROAD: sparks,
    /// a flash and smoke) by their sum's share of 1..2.
    #[test]
    fn landing_fires_the_bottom_out() {
        let mut fx = GroundFx::default();
        // (22.4 mph also fires the dry road's empty XETT_ROAD_DR.)
        let mut c = car([rolling(1, 0.0, 0.0); 4], 10.0);
        c.bump = 1.5;
        let fires: Vec<_> = fx.update(&c, 1.0 / 60.0).into_iter().filter(|f| f.kind != 1).collect();
        assert_eq!(fires, vec![Fire { kind: 5, intensity: 0.5, surface: 1, wheel: None, velocity: FireVelocity::Car }]);
    }

    #[test]
    fn bursts_spawn_one_then_one_per_rate_step() {
        assert_eq!(burst_count(0.2, 1.0), 1);
        assert_eq!(burst_count(1.0, 1.0), 2);
        assert_eq!(burst_count(0.5, 100.0), 51);
        assert_eq!(burst_count(0.35, 0.0), 4);
        assert_eq!(burst_count(0.0, 1.0), 0);
    }

    #[test]
    fn markers_from_the_wheels() {
        let w = [
            Vec3::new(1.3, 0.8, 0.3),
            Vec3::new(1.3, -0.8, 0.3),
            Vec3::new(-1.2, -0.8, 0.3),
            Vec3::new(-1.2, 0.8, 0.3),
        ];
        assert_eq!(marker(0, &w), Some(Vec3::ZERO));
        assert_eq!(marker(2, &w), Some(Vec3::new(-1.2, 0.0, 0.3)));
        assert!(marker(3, &w).unwrap().abs_diff_eq(Vec3::new(0.05, 0.8, 0.3), 1e-6));
        assert_eq!(marker(7, &w), Some(w[2]));
        assert_eq!(marker(10, &w), None);
    }
}
