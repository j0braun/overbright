//! The mix map (SOUND\MixMaps\*.mxb): the game's dynamic mixer, which
//! gives every car / world sound object its levels (slot 0x38) and
//! azimuths (slot 0x3C) as packed dwords (bits 0–14 the level 0..0x7FFF,
//! bits 16–31 the azimuth word). Traced in SPEED2.EXE (verified
//! unless marked):
//!
//! * Load 0x486920 (the file by 0x7EDE4C[k], k from 0x4869A0: drag
//!   (0x89E7D8 / 0x890108) 2 DRG, 0x89E7E1 3 URL, drift (0x89E7D9 /
//!   0x890104) 1 DFT, else 0), parse 0x467610 / 0x477C70, build 0x485FA0:
//!   per section (= sound object group 0..8, 0x82B884 +0xA8 lists) one
//!   copy (MixMapProc, vtable 0x78A6BC) per object instance (0x4837C0).
//! * Per frame 0x478FE0 → 0x467E70 per copy: the data procs (curves of a
//!   game value, 0x477C30), the controls (0x477BD0), the event controls
//!   (0x468D40, envelopes), the 3D controls (0x468580, distance / azimuth
//!   attenuation), the sub channels (sums, inline) and the master
//!   channels (0x467460 sum, 0x477AA0 the packed outputs).
//! * Levels are millibels: 0x467FF0 (mB → 0..0x7FFF) and 0x468050 (back).

use std::collections::HashMap;

mod curves;
mod envelope;
mod parse;
mod runtime;
mod scene;
mod tables;
mod volume;

pub use self::curves::*;
use self::envelope::*;
pub use self::parse::*;
pub use self::runtime::*;
pub use self::scene::*;
use self::tables::*;
pub use self::volume::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn db_conversions() {
        assert_eq!(db_to_lin(0), 0x7FFF);
        assert_eq!(db_to_lin(-602), 32730 >> 1);
        assert_eq!(db_to_lin(-10000), 0);
        assert_eq!(lin_to_db(0x7FFF), -1);
        assert_eq!(lin_to_db(0), -10000);
        assert_eq!(lin_to_db(0x4000), -602);
        // Round trip within a few mB.
        for mb in [-100, -600, -1275, -3000] {
            assert!((lin_to_db(db_to_lin(mb)) - mb).abs() <= 4, "{mb}");
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn curves_and_angles() {
        assert_eq!(curve(0, 0, false), 32766);
        assert_eq!(curve(0, 0x7FFF, false), 0);
        assert_eq!(curve(8, 0x1000, false), 0x6FFF);
        assert_eq!(curve(9, 0x1000, false), 0x1000);
        assert_eq!(asin16(0.0), 0);
        assert_eq!(asin16(1.0), 0x4000);
        assert!((asin16(0.5) as i32 - 0x1555).abs() <= 3, "{:x}", asin16(0.5));
        assert_eq!(asin16(-0.5), (0x10000 - asin16(0.5) as i32) as u16);
        // Ahead 0, the cross ≥ 0 side 0x4000, behind 0x8000.
        assert_eq!(azimuth([10.0, 0.0], [0.0, 0.0], [1.0, 0.0]), 0);
        assert_eq!(azimuth([0.0, -10.0], [0.0, 0.0], [1.0, 0.0]), 0x4000);
        assert_eq!(azimuth([0.0, 10.0], [0.0, 0.0], [1.0, 0.0]), 0xFFFF - 0x4000);
        assert_eq!(azimuth([-10.0, 0.0], [0.0, 0.0], [1.0, 0.0]), 0x8000);
    }

    /// The defaults: the volume fields full, the player's car 5 m from a
    /// chase camera, straight ahead.
    struct Still;
    impl World for Still {
        fn field(&self, _s: usize, _c: usize, id: u32) -> Option<i32> {
            let class = (id >> 4) & 0x7F;
            Some(match (id >> 28, class, id & 0xF) {
                (6, 3, 6) => 0,
                (6, 3, _) => 0x7FFF,
                (6, 8..=12, 0) => 0,
                (6, 8..=12, 1) => 5,
                (6, 8..=12, _) => 0,
                _ => 0,
            })
        }
    }

    /// Section 0 (the global objects) at the default volumes (menu music
    /// 0.7, the rest 1): the music objects' (classes 0 and 2) and
    /// SFXObj_FEHUD's (class 6) levels, as read from the original under
    /// Wine (sound5: [obj +0x58 + 4·slot] & 0x7FFF in the main menu).
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn global_levels() {
        for k in 0..4 {
            let map = MixMap::load(&crate::testkit::game_root(), k).expect("loading the mix map");
            let mut mix = Mix::new(map);
            let scene = Scene {
                master_vol: Some(master_vol_fields(1.0, [0.7, 1.0, 1.0, 1.0, 1.0, 1.0], false)),
                ..Default::default()
            };
            for _ in 0..60 {
                mix.frame(&scene, 1.0 / 60.0);
            }
            if k == 0 {
                assert_eq!([mix.level(0, 0, 0, 0), mix.level(0, 0, 0, 1)], [6447, 11598]);
                assert_eq!([mix.level(0, 0, 2, 0), mix.level(0, 0, 2, 1)], [10337, 10337]);
                let fehud: Vec<i32> = (0..7).map(|s| mix.level(0, 0, 6, s)).collect();
                assert_eq!(fehud, [4235, 6157, 6522, 6157, 6522, 4486, 3085]);
            }
        }
    }

    /// MasterVol f14 (0x471B75) and the player's weather wind (class 10,
    /// slot 0): 205 with the game music on, 2154 with it off or in the
    /// fill case, as read from the original at idle (sound comparison
    /// round 2).
    /// The master volume law (0x471970 / 0x45C210) against the original
    /// under Wine (2026-11-11, airport free roam at idle, the option
    /// floats written live): f14 = f1 = trunc((1 − game music · master ·
    /// c(master)) · 32767) at game music 1 read 0 / 21201 / 30748 / 32258 /
    /// 32643 at master 1 / 0.5 / 0.2 / 0.1 / 0.05 (0x7ED344): lowering the
    /// master raises the no-music fill as lowering the music does (native);
    /// and with game music 0 FX_WIND_Weather's level (class 10 slot 0, its
    /// p0) read 2154 / 759 / 132 at master 1 / 0.5 / 0.2.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn master_law_matches_the_original() {
        for (m, want) in [(1.0, 0), (0.5, 21201), (0.2, 30748), (0.1, 32258), (0.05, 32643)] {
            assert_eq!(master_vol_fields(m, [0.7, 1.0, 1.0, 1.0, 1.0, 1.0], false)[14], want, "master {m}");
        }
        let map = MixMap::load(&crate::testkit::game_root(), 0).expect("loading the mix map");
        for (m, want) in [(1.0, 2154), (0.5, 759), (0.2, 132)] {
            let mv = master_vol_fields(m, [0.0, 0.0, 0.0, 1.0, 1.0, 1.0], false);
            let mut mix = Mix::new(map.clone());
            let car = CarFields {
                pos: [0.0, 0.0],
                right: [1.5, -0.8],
                left: [1.5, 0.8],
                speed: 0.0,
                ctx_rpm: 1000.0,
                airborne: false,
                skid: 0,
                view_slew: 0x7D00,
            };
            let scene = Scene {
                listener: Some(Listener {
                    cam: [-5.0, 0.0],
                    cam_dir: [1.0, 0.0],
                    car: [0.0, 0.0],
                    car_dir: [1.0, 0.0],
                }),
                player: Some(car),
                master_vol: Some(mv),
                player_vols: player_vol_fields(m, 0.0, 0.0, 1.0, 1.0, 1.0, mv[14]),
                ..Default::default()
            };
            for _ in 0..60 {
                mix.frame(&scene, 1.0 / 60.0);
            }
            assert_eq!(mix.level(1, 0, 10, 0), want, "master {m}");
        }
    }

    /// The options screen's master slider (0x49A790 / 0x4FF480: value
    /// +0x30, step 0.05 at +0x38, range 0..1, the sum stored as an f32 and
    /// clamped at 0) from 1 down: its 21 positions.
    fn slider_positions() -> Vec<f32> {
        let mut p = vec![1.0f32];
        loop {
            let n = (p[p.len() - 1] - 0.05f32).max(0.0);
            if n == p[p.len() - 1] {
                return p;
            }
            p.push(n);
        }
    }

    /// Every slider position against the original (airport free roam,
    /// Rachel's 350Z at idle, the master float written live at each
    /// position; `docs/evidence/audio-levels/slider-sweep.txt`): the
    /// player's CAR block p4 = clamp(((HM +0x7C · level0) >> 15) − 0x7FFF)
    /// with HM +0x7C 12500, p14 = (wet B · level0) >> 15, FX_WIND_Weather's
    /// p0 (class 10 slot 0), with speech and game music 0 and the other
    /// volumes at their defaults, then at effects 0.85 and engine / tires
    /// 0.65 (the products' f32 truncations); and the pause menu's FE click
    /// (PlayCommonSample p1 = SFXObj_FEHUD slot 0) at seven positions.
    /// The engine's abk layer stops at 0.30 (p4 −31825) and the wind at
    /// 0.15 in both games: the graphs' own cut.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn slider_positions_match_the_original() {
        let pos = slider_positions();
        assert_eq!(pos.len(), 21);
        assert_eq!(pos[4].to_bits(), 0.799_999_95f32.to_bits());
        assert_eq!(pos[16].to_bits(), 0.199_999_84f32.to_bits());
        assert_eq!(pos[20], 0.0);
        const P4: [i32; 21] = [
            -25811, -26177, -26582, -27015, -27478, -27950, -28429, -28914, -29388, -29861, -30314, -30736, -31137,
            -31496, -31825, -32106, -32338, -32526, -32659, -32741, -32767,
        ];
        const P14: [i32; 21] = [
            2886, 2734, 2566, 2386, 2194, 1998, 1800, 1598, 1402, 1205, 1017, 842, 676, 527, 390, 274, 178, 100, 44,
            11, 0,
        ];
        const WIND: [i32; 21] =
            [2154, 2040, 1915, 1781, 1638, 1492, 1343, 1193, 1046, 900, 759, 629, 504, 393, 291, 204, 132, 74, 0, 0, 0];
        // The same with effects at its 4th position (0.85) and engine /
        // tires at their 8th (0.65), as the slider leaves them.
        const P4_65: [i32; 21] = [
            -28245, -28483, -28751, -29028, -29329, -29635, -29947, -30262, -30573, -30878, -31172, -31445, -31708,
            -31941, -32154, -32337, -32489, -32611, -32697, -32767, -32767,
        ];
        const P14_65: [i32; 21] =
            [1876, 1777, 1666, 1551, 1426, 1299, 1170, 1039, 910, 783, 661, 548, 439, 343, 254, 178, 115, 65, 29, 0, 0];
        const WIND_65: [i32; 21] =
            [1831, 1735, 1626, 1512, 1394, 1267, 1142, 1014, 889, 765, 646, 535, 429, 334, 248, 174, 112, 63, 0, 0, 0];
        let map = MixMap::load(&crate::testkit::game_root(), 0).expect("loading the mix map");
        let cases = [
            ([0.7f32, 0.0, 0.0, 1.0, 1.0, 1.0], [1.0f32, 1.0, 1.0], (&P4, &P14, &WIND)),
            ([0.7, 0.0, 0.0, pos[3], pos[7], pos[7]], [pos[3], pos[7], pos[7]], (&P4_65, &P14_65, &WIND_65)),
        ];
        for (vols, [effects, engine, tires], (p4s, p14s, winds)) in cases {
            for (i, &m) in pos.iter().enumerate() {
                let mv = master_vol_fields(m, vols, false);
                let mut mix = Mix::new(map.clone());
                let car = CarFields {
                    pos: [0.0, 0.0],
                    right: [1.5, -0.8],
                    left: [1.5, 0.8],
                    speed: 0.0,
                    ctx_rpm: 1000.0,
                    airborne: false,
                    skid: 0,
                    view_slew: 0x7D00,
                };
                let scene = Scene {
                    listener: Some(Listener {
                        cam: [-5.0, 0.0],
                        cam_dir: [1.0, 0.0],
                        car: [0.0, 0.0],
                        car_dir: [1.0, 0.0],
                    }),
                    player: Some(car),
                    master_vol: Some(mv),
                    player_vols: player_vol_fields(m, 0.0, 0.7, effects, engine, tires, mv[14]),
                    ..Default::default()
                };
                for _ in 0..60 {
                    mix.frame(&scene, 1.0 / 60.0);
                }
                let level0 = mix.level(1, 0, 0, 0);
                let p4 = ((((12500i64 * level0 as i64) >> 15) as i32) - 0x7FFF).clamp(-32767, 0);
                assert_eq!(p4, p4s[i], "p4 at master {m}, engine {engine}");
                // p14 scales by the reverb's wet B [0x82B070], read 5186 and
                // 5187 in this scene (its ramp's last unit).
                let p14 = |wet: i64| ((wet * level0 as i64) >> 15) as i32;
                assert!(
                    p14(5186) == p14s[i] || p14(5187) == p14s[i],
                    "p14 at master {m}, engine {engine}: {}",
                    p14(5186)
                );
                assert_eq!(mix.level(1, 0, 10, 0), winds[i], "wind at master {m}, effects {effects}");
            }
        }
        for (i, want) in [(0, 4235), (10, 1493), (14, 573), (16, 261), (17, 146), (18, 66), (19, 0)] {
            let mut mix = Mix::new(map.clone());
            let scene = Scene {
                master_vol: Some(master_vol_fields(pos[i], [0.7, 1.0, 1.0, 1.0, 1.0, 1.0], false)),
                ..Default::default()
            };
            for _ in 0..60 {
                mix.frame(&scene, 1.0 / 60.0);
            }
            assert_eq!(mix.level(0, 0, 6, 0), want, "FE click at master {}", pos[i]);
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn music_off_brings_the_wind_up() {
        let map = MixMap::load(&crate::testkit::game_root(), 0).expect("loading the mix map");
        for (game_music, fill, want_f14, want) in
            [(1.0, false, 0, 205), (0.0, false, 0x7FFF, 2154), (1.0, true, 0x7FFF, 2154)]
        {
            let mv = master_vol_fields(1.0, [0.7, game_music, 1.0, 1.0, 1.0, 1.0], fill);
            assert_eq!(mv[14], want_f14);
            let mut mix = Mix::new(map.clone());
            let car = CarFields {
                pos: [0.0, 0.0],
                right: [1.5, -0.8],
                left: [1.5, 0.8],
                speed: 0.0,
                ctx_rpm: 1000.0,
                airborne: false,
                skid: 0,
                view_slew: 0x7D00,
            };
            let scene = Scene {
                listener: Some(Listener {
                    cam: [-5.0, 0.0],
                    cam_dir: [1.0, 0.0],
                    car: [0.0, 0.0],
                    car_dir: [1.0, 0.0],
                }),
                player: Some(car),
                master_vol: Some(mv),
                player_vols: player_vol_fields(1.0, 1.0, 0.7, 1.0, 1.0, 1.0, mv[14]),
                ..Default::default()
            };
            for _ in 0..60 {
                mix.frame(&scene, 1.0 / 60.0);
            }
            assert_eq!(mix.level(1, 0, 10, 0), want, "game music {game_music} fill {fill}");
        }
    }

    /// SFXCTL_MasterVol f6 (`Scene::sound_hold`, a replay paused): event
    /// control 0 of section 0 (−10000 mB, 17-frame attack and release)
    /// takes the player's section down through its sub channel 0 and
    /// lets it back; the music (section 0) does not move.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn a_paused_replay_fades_the_world() {
        let map = MixMap::load(&crate::testkit::game_root(), 0).expect("loading the mix map");
        let mut mix = Mix::new(map);
        let car = CarFields {
            pos: [0.0, 0.0],
            right: [0.0, -0.8],
            left: [0.0, 0.8],
            speed: 10.0,
            ctx_rpm: 3000.0,
            airborne: false,
            skid: 0,
            view_slew: 0x7D00,
        };
        let mut scene = Scene {
            listener: Some(Listener { cam: [-5.0, 0.0], cam_dir: [1.0, 0.0], car: [0.0, 0.0], car_dir: [1.0, 0.0] }),
            player: Some(car),
            master_vol: Some(master_vol_fields(1.0, [0.7, 1.0, 1.0, 1.0, 1.0, 1.0], false)),
            ..Default::default()
        };
        let run = |mix: &mut Mix, scene: &Scene, n: usize| {
            for _ in 0..n {
                mix.frame(scene, 1.0 / 60.0);
            }
            (
                (0..16).flat_map(|k| (0..8).map(move |s| (k, s))).map(|(k, s)| mix.level(1, 0, k, s)).max().unwrap(),
                mix.level(0, 0, 0, 1),
            )
        };
        let (car_on, music_on) = run(&mut mix, &scene, 60);
        assert!(car_on > 1000, "{car_on}");
        scene.sound_hold = true;
        let (car_mid, _) = run(&mut mix, &scene, 6);
        let (car_off, music_off) = run(&mut mix, &scene, 30);
        assert!(car_mid < car_on && car_mid > car_off, "{car_on} {car_mid} {car_off}");
        assert_eq!(car_off, 0);
        assert_eq!(music_off, music_on);
        scene.sound_hold = false;
        let (car_back, _) = run(&mut mix, &scene, 60);
        assert_eq!(car_back, car_on);
    }

    /// The car stereo objects (CARSFX_TrafficStereo 0x30030, class 3 of
    /// the traffic section; CARSFX_Stereo 0x200F0, class 0xF of the AI
    /// section) read slot 0.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn stereo_slots() {
        let map = MixMap::load(&crate::testkit::game_root(), 0).expect("loading the mix map");
        let mut mix = Mix::new(map);
        for _ in 0..10 {
            mix.update(&Still, 1.0 / 60.0);
        }
        for (s, k) in [(3, 3), (2, 15)] {
            let l: Vec<Option<u32>> = (0..8).map(|slot| mix.packed(s, 0, k, slot)).collect();
            println!("section {s} object {k}: {l:x?}");
        }
        for s in [2usize, 3] {
            for k in 0..16 {
                let l: Vec<i32> = (0..8).map(|slot| mix.level(s, 0, k, slot)).collect();
                if l.iter().any(|&x| x != 0) {
                    println!("section {s} object {k}: {l:?}");
                }
            }
        }
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn player_levels() {
        let map = MixMap::load(&crate::testkit::game_root(), 0).expect("loading the mix map");
        assert_eq!(map.sections.len(), 9);
        let mut mix = Mix::new(map);
        for _ in 0..10 {
            mix.update(&Still, 1.0 / 60.0);
        }
        for k in 0..13 {
            let l: Vec<i32> = (0..8).map(|s| mix.level(1, 0, k, s)).collect();
            println!("player object {k}: {l:?}");
        }
        assert!(mix.level(1, 0, 0, 0) > 0x4000, "the engine is loud");
        for s in 0..9 {
            let ids: Vec<String> = mix.map.field_ids(s).iter().map(|i| format!("{i:08x}")).collect();
            println!("section {s} fields: {}", ids.join(" "));
        }
    }

    /// FX_Scrape's level: SFXObj_Collision's slot 9 (0x461CE0 →
    /// +0xD4) is section 5's master 1, base −600 mB plus sub channel 0
    /// (the section-0 hold and speech duck, the effects volume) and the
    /// scrape point's 3D control (4..30 m, view 0 by the distance to the
    /// player's car). Read under Wine (a free-roam audio capture, a line
    /// playing): {5812, 4096, azimuth, 0, level}; 5812 = −1501 mB.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn scrape_slot_nine() {
        let map = MixMap::load(&crate::testkit::game_root(), 0).expect("loading the mix map");
        let mv = master_vol_fields(1.0, [0.7, 1.0, 1.0, 1.0, 1.0, 1.0], false);
        let run = |speech: i32, at: [f32; 2]| {
            let mut mix = Mix::new(map.clone());
            let scene = Scene {
                listener: Some(Listener {
                    cam: [-5.0, 0.0],
                    cam_dir: [1.0, 0.0],
                    car: [0.0, 0.0],
                    car_dir: [1.0, 0.0],
                }),
                master_vol: Some(mv),
                collisions: vec![at],
                sfx: vec![(0, 0, 0x11, speech)],
                ..Default::default()
            };
            for _ in 0..60 {
                mix.frame(&scene, 1.0 / 60.0);
            }
            (mix.level(5, 0, 0, 9), mix.azimuth(5, 0, 0, 9))
        };
        assert_eq!(run(0, [1.0, 0.0]).0, 16365);
        let (l, az) = run(1, [1.0, -0.3]);
        assert!((5780..=5840).contains(&l), "{l}");
        assert!(az > 0 && az < 0x4000, "{az:x}");
        // Beyond 30 m: silent.
        assert_eq!(run(0, [40.0, 0.0]).0, 0);
    }

    /// The impact sounds' start volume: SFXObj_Collision's slot 0x28
    /// (0x461CE0 → the sound object's +0xC0) is slot 0x38 of the object's
    /// kind (+0x124 1..8 → slot 1..8, else 0) — section 5's master 0 for
    /// slots 0–5 (−600 mB), master 2 for 7–8 (−800 mB), slot 6 unbound
    /// (0) — each through its master's 3D control by the distance.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn impact_slots_by_kind() {
        let map = MixMap::load(&crate::testkit::game_root(), 0).expect("loading the mix map");
        let mv = master_vol_fields(1.0, [0.7, 1.0, 1.0, 1.0, 1.0, 1.0], false);
        let levels = |at: [f32; 2]| {
            let mut mix = Mix::new(map.clone());
            let scene = Scene {
                listener: Some(Listener {
                    cam: [-5.0, 0.0],
                    cam_dir: [1.0, 0.0],
                    car: [0.0, 0.0],
                    car_dir: [1.0, 0.0],
                }),
                master_vol: Some(mv),
                collisions: vec![at],
                ..Default::default()
            };
            mix.frame(&scene, 1.0 / 60.0);
            (0..9).map(|k| mix.level(5, 0, 0, k)).collect::<Vec<_>>()
        };
        let near = levels([1.0, 0.0]);
        // One pass gives the level (no 3D smoothing).
        assert!(near[..6].iter().all(|&l| l == near[0]) && near[0] > 0, "{near:?}");
        assert_eq!(near[6], 0, "{near:?}");
        assert!(near[7] == near[8] && near[7] > 0 && near[7] < near[0], "{near:?}");
        // −600 mB = 16365, −800 mB = 12999 (read: 1 m from the car).
        assert_eq!((near[0], near[7]), (16365, 12999));
        // Further off, the masters' own 3D control turns them down (40 m:
        // 6373 / 3175).
        let far = levels([40.0, 0.0]);
        assert!((0..9).all(|k| far[k] < near[k] || near[k] == 0) && far[0] > 0, "{near:?} {far:?}");
    }

    /// The collision objects while Rachel speaks: section 5's
    /// sub channel 0 (all three masters' input) sums section 0's event
    /// controls 0 and 2 (inputs 0xB2003000 / 0xB2003002); event control 2
    /// is −900 mB × f10 (the speech volume) on SFXObj_Speech f1 (0x4751A0:
    /// a line plays outside game state 3), attack 5 × 16.67 ms. So an
    /// impact 1 m off starts at 4617 (kind 7–8) / 5812 (1–5) instead of
    /// 12999 / 16365 — the levels read under Wine with and without the
    /// stage-2 save's queued kind-5 hint line playing.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn impacts_duck_while_rachel_speaks() {
        let map = MixMap::load(&crate::testkit::game_root(), 0).expect("loading the mix map");
        let mv = master_vol_fields(1.0, [0.7, 1.0, 1.0, 1.0, 1.0, 1.0], false);
        let levels = |speaking: bool| {
            let mut mix = Mix::new(map.clone());
            let scene = Scene {
                listener: Some(Listener { cam: [-5.0, 0.0], cam_dir: [1.0, 0.0], car: [0.0, 0.0], car_dir: [1.0, 0.0] }),
                master_vol: Some(mv),
                collisions: vec![[1.0, 0.0]],
                sfx: if speaking { vec![(0, 0, 0x11, 1)] } else { Vec::new() },
                ..Default::default()
            };
            for _ in 0..30 {
                mix.frame(&scene, 1.0 / 60.0);
            }
            (mix.level(5, 0, 0, 7), mix.level(5, 0, 0, 0))
        };
        assert_eq!(levels(false), (12999, 16365));
        assert_eq!(levels(true), (4617, 5812));
    }
}
