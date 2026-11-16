use super::*;

/// 0x45DF00 (single listener, mode 0x82BC14 = 0): a position object's
/// four fields from the listener globals 0x46D7E0 sets each frame: f0 =
/// trunc(distance to the player's car 0x82BEDC), f1 = trunc(distance to
/// the camera 0x82BF10), f2 = the azimuth from the car along its heading
/// 0x82C6C4, f3 = the azimuth from the camera along its look 0x82BECC
/// (all 2D, the game's x / y). With no camera (0x82BB40 null): −1, −1, 0,
/// 0 (0x45E0D9).
pub fn position_fields(at: [f32; 2], l: Option<&Listener>) -> [i32; 4] {
    let Some(l) = l else { return [-1, -1, 0, 0] };
    let dist =
        |a: [f32; 2], b: [f32; 2]| (((a[0] - b[0]) as f64).powi(2) + ((a[1] - b[1]) as f64).powi(2)).sqrt() as i32;
    [dist(at, l.car), dist(at, l.cam), azimuth(at, l.car, l.car_dir) as i32, azimuth(at, l.cam, l.cam_dir) as i32]
}

/// The listener globals (0x46D7E0 for player 0): the view camera's
/// position (camera +0x40) and its 2D look (+0x50, normalised), the
/// player car's position (+0x60) and 2D heading (+0x90, normalised).
#[derive(Clone, Copy, Debug, Default)]
pub struct Listener {
    pub cam: [f32; 2],
    pub cam_dir: [f32; 2],
    pub car: [f32; 2],
    pub car_dir: [f32; 2],
}

/// A car's inputs to the mix (its controllers' fields).
#[derive(Clone, Copy, Debug, Default)]
pub struct CarFields {
    /// The car (3DCarPos), its right and left wheel pairs' centres
    /// (3DRightWheelPos / 3DLeftWheelPos; **inferred**: the wheels'
    /// mean positions) in 2D.
    pub pos: [f32; 2],
    pub right: [f32; 2],
    pub left: [f32; 2],
    /// Speed (m/s), the sound context's rpm (+0x48, 1000..10000), whether
    /// a wheel is off the ground, SFXCTL_Skid +0x10 (0..0x7FFF).
    pub speed: f32,
    pub ctx_rpm: f32,
    pub airborne: bool,
    pub skid: i32,
    /// SFXCTL_Physics +0x1C, slewed by [`view_slew`].
    pub view_slew: i32,
}

/// SFXCTL_Physics +0x1C (0x470A60): toward 0x7D00 for the context's
/// view ids 0, 1, 5, 0xFA0 for 4, 0 for 2 / 3, at most 0x3FFF a frame.
pub fn view_slew(prev: i32, view_id: u32) -> i32 {
    let t = match view_id {
        0 | 1 | 5 => 0x7D00,
        4 => 0xFA0,
        _ => 0,
    };
    t.min(prev + 0x3FFF).max(prev - 0x3FFF)
}

/// SFXCTL_Physics' fields (0x470A60, slot 6): f0..f3 = |mph| / 30, 60,
/// 100, 140 · 32767 (clamped 0..0x7FFF), f4 = (10000 − ctx rpm) / 9000 ·
/// 32767, f5 = 1 below four wheels down (0x5EC200 × 0.25 < 1), f6 the
/// view slew, f7 / f8 the car's camera kind 6 / 7 flags (0 here), f9 the
/// race-progress field (+0x28, 0 here).
pub fn physics_field(c: &CarFields, f: u32) -> i32 {
    let mph = |d: f32| (((c.speed * 2.237).abs() as f64 * d as f64 * 32767.0) as i32).clamp(0, 0x7FFF);
    match f {
        0 => mph(0.033_333_335),
        1 => mph(0.016_666_668),
        2 => mph(0.01),
        3 => mph(0.007_142_857),
        4 => (((10000.0 - c.ctx_rpm) as f64 * 0.000_111_111_11 * 32767.0) as i32).clamp(0, 0x7FFF),
        5 => c.airborne as i32,
        6 => c.view_slew,
        _ => 0,
    }
}

/// What the clone gives the mix each frame: the listener, the view mode
/// (0x82BB48: 1 for the context view id 2, 2 for 4, else 0; 3–5 are
/// cinematic / scripted states, not used here), the player car (section
/// 1), the AI racers (section 2) and the traffic cars (section 3).
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub listener: Option<Listener>,
    pub view: u32,
    pub player: Option<CarFields>,
    pub ai: Vec<CarFields>,
    pub traffic: Vec<[f32; 2]>,
    /// The world objects' states (`audio::enviro`): the fountains
    /// (section 4), planes (7) and trains (8), each state's controller
    /// position (one copy per state).
    pub enviro: [Vec<[f32; 2]>; 3],
    /// The collision objects (section 5, SFXObj_Collision 0x50000, one
    /// copy per object): each one's scrape point, which its
    /// SFXCTL_3DScrapePos (0x50010, class 1) and SFXCTL_3DColPos (0x50000,
    /// class 0) read (the client's +0xD0 / +0xC0, 0x461B60: the sound
    /// object's +0x100; the impact point taken as the same, **inferred**).
    pub collisions: Vec<[f32; 2]>,
    pub paused: bool,
    /// Sound objects' own fields (kind 4 ids: object +4 + 4·f) that are
    /// not 0: (section, copy, class << 4 | f, value). The ducking
    /// triggers: CARSFX_Nitrous f0 (1 while FX_NITROUS runs, 0x472AF0 /
    /// 0x45F2A0), CARSFX_BottomOut f0 (1 on a knock's frame, 0x47EA30),
    /// CARSFX_TrafficWoosh f2 (1 on a whoosh's frame, 0x475E10),
    /// CARSFX_TrafficEngine f3 (0x462240: 32767).
    pub sfx: Vec<(usize, usize, u32, i32)>,
    /// SFXCTL_MasterVol's fields f0..f5 (section 0, class 0; see
    /// [`master_vol_fields`]); none: 0 (the full volumes).
    pub master_vol: Option<[i32; 16]>,
    /// The player's volume fields (0x45C210, group 1 class 3): f0 speech,
    /// f1 menu music, f3 effects, f4 engine, f5 tires = trunc(v · master ·
    /// 32767); f2 = [0x7ED344] (MasterVol f14), f6 = 0x7FFF − that.
    pub player_vols: [i32; 7],
    /// SFXCTL_MasterVol f6 (+0x1C, 0x471BD1..0x471C1B): 1 while the audio
    /// system's +0x13 is set, the race camera state is 1, state 7 with the
    /// NIS manager's 0x435950, or a replay (state 5) is paused (manager
    /// +0x14); else 0. Every map's section 0 event control 0 takes it as
    /// its trigger (mode 2, −10000 mB, attack and release 17 × 16.67 ms,
    /// release curve 8), and sub channel 0 of sections 1–8 (the cars, the
    /// traffic, the world objects) reads that event: those fade out while
    /// it holds; section 0 (the music, speech and front end) does not.
    pub sound_hold: bool,
    /// The player's SFXCTL_Tunnel fields f0..f3 (section 1 class 7,
    /// [`super::driveby::Tunnel::fields`]).
    pub tunnel: [i32; 4],
}

impl World for Scene {
    /// Sections: 0 the global objects (SFXCTL_MasterVol's fields 1 −
    /// volume: 0 at the default full volumes; the rest 0), 1 the player,
    /// 2 the AI racers, 3 the traffic. Unlisted fields of an existing
    /// object are 0 (its events idle: **inferred**).
    fn field(&self, s: usize, copy: usize, id: u32) -> Option<i32> {
        let ctl = id >> 28 == 6;
        let class = (id >> 4) & 0x7F;
        let f = id & 0xF;
        let l = self.listener.as_ref();
        if id >> 28 == 4 {
            let key = id & 0x7FF;
            if let Some(v) = self.sfx.iter().find(|e| e.0 == s && e.1 == copy && e.2 == key) {
                return Some(v.3);
            }
        }
        match s {
            0 => Some(match (self.master_vol, ctl && class == 0) {
                (_, true) if f == 6 => self.sound_hold as i32,
                (Some(m), true) => m.get(f as usize).copied().unwrap_or(0),
                _ => 0,
            }),
            1 | 2 => {
                let c = if s == 1 { self.player.as_ref()? } else { self.ai.get(copy)? };
                if !ctl {
                    return Some(0);
                }
                Some(match class {
                    0 => physics_field(c, f),
                    2 if f == 3 => c.skid,
                    // The volume fields (0x45C210, `Scene::player_vols`).
                    3 if s == 1 => self.player_vols.get(f as usize).copied().unwrap_or(0),
                    7 if s == 1 => self.tunnel.get(f as usize).copied().unwrap_or(0),
                    8 | 0xA => position_fields(c.pos, l)[f.min(3) as usize],
                    0xB => position_fields(c.right, l)[f.min(3) as usize],
                    0xC => position_fields(c.left, l)[f.min(3) as usize],
                    _ => 0,
                })
            }
            3 => {
                let p = self.traffic.get(copy)?;
                Some(if ctl && class == 0 { position_fields(*p, l)[f.min(3) as usize] } else { 0 })
            }
            // The collision objects' position controllers (vtables
            // 0x78AAF8 / 0x78AB30: the 3D position's fields, 0x45E100 →
            // 0x45DF00, of the point their slot 0x2C was given).
            5 => {
                let p = self.collisions.get(copy)?;
                Some(if ctl && class <= 1 { position_fields(*p, l)[f.min(3) as usize] } else { 0 })
            }
            // The world objects' position controllers (the 3D car
            // position's fields, 0x45E100 → 0x45DF00).
            4 | 7 | 8 => {
                let k = match s {
                    4 => 0,
                    7 => 1,
                    _ => 2,
                };
                let p = self.enviro[k].get(copy)?;
                Some(if ctl && class == 0 { position_fields(*p, l)[f.min(3) as usize] } else { 0 })
            }
            _ => None,
        }
    }
    fn view(&self) -> u32 {
        self.view
    }
    fn paused(&self) -> bool {
        self.paused
    }
}

/// The view mode 0x82BB48 for the context's view id (0x46D7E0).
pub fn view_mode(view_id: u32) -> u32 {
    match view_id {
        2 => 1,
        4 => 2,
        _ => 0,
    }
}

impl Mix {
    /// One frame for `scene`: copies per section (1 global, 1 player or
    /// none, one per AI racer and traffic car, none for the world's
    /// woosh objects, one per collision object, one per fountain, plane and train
    /// state), then the pass.
    /// A check aid: the master channels of section `s` copy `copy` that
    /// feed object class `k`, each with its inputs' values (word = value),
    /// the sub channels, controls and event controls they read expanded
    /// one level.
    pub fn trace(&self, s: usize, copy: usize, k: u32) -> String {
        let Some(sec) = self.map.sections.get(s) else {
            return String::new();
        };
        let Some(cp) = self.copies.get(s).and_then(|v| v.get(copy)) else {
            return String::new();
        };
        let mut out = String::new();
        let word = |x: u32| -> String {
            let v = self.chan_input(s, copy, x);
            let t = ((x >> 16) & 0xFF) as usize;
            let i = (x & 0xFF) as usize;
            let more = match (x >> 28, t == s) {
                (3, true) => sec
                    .subs
                    .get(i)
                    .map(|c| {
                        format!(
                            " [sub base {} in {}]",
                            c.base,
                            c.inputs
                                .iter()
                                .map(|&y| format!("{y:x}={}", self.chan_input(s, copy, y)))
                                .collect::<Vec<_>>()
                                .join(" ")
                        )
                    })
                    .unwrap_or_default(),
                (0, true) => sec
                    .controls
                    .get(i)
                    .map(|c| format!(" [ctl id {:x} dp {} in {:x}]", c.id, cp.dp[i], c.input))
                    .unwrap_or_default(),
                (0xA | 0xB, true) => sec.evts.get(i).map(|e| format!(" [evt {:x?}]", e.w)).unwrap_or_default(),
                _ => String::new(),
            };
            format!("{x:x}={v}{more}")
        };
        for (i, c) in sec.masters.iter().enumerate() {
            if (c.out >> 4) & 0x7F != k {
                continue;
            }
            out += &format!(
                "master {i} base {} = {} | {} | td {:?}\n",
                c.base,
                cp.master[i],
                c.inputs.iter().map(|&x| word(x)).collect::<Vec<_>>().join(" "),
                c.threed.iter().map(|x| cp.td.get((x & 0xFF) as usize).map(|t| (t.mb, t.az))).collect::<Vec<_>>()
            );
        }
        out
    }

    pub fn frame(&mut self, scene: &Scene, dt: f32) {
        let n = self.copies.len();
        for s in 0..n {
            let c = match s {
                0 => 1,
                1 => scene.player.is_some() as usize,
                2 => scene.ai.len(),
                3 => scene.traffic.len(),
                4 => scene.enviro[0].len(),
                5 => scene.collisions.len(),
                7 => scene.enviro[1].len(),
                8 => scene.enviro[2].len(),
                _ => 0,
            };
            self.set_copies(s, c);
        }
        self.update(scene, dt);
    }
}
