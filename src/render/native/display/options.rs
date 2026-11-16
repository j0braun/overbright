/// The renderer switches from the custom values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Display {
    /// g_CarEnvironmentMapEnable (0x870CB8, custom 0): 0 makes no cube at
    /// all (0x5BA4F4 skips creating 0x870858 / 0x87088C, 0x5B9920 skips the
    /// front end's copy) so the car effects' EnvMapTexture (0x5BCAE0) is
    /// null and samples black; 1–3 pick the face schedule
    /// ([`env_faces`]).
    pub env_rate: u8,
    /// g_CarEnvironmentMapUpdateData (0x870CBC, custom 1): the cube faces
    /// draw the world (0x5CAD86 → 0x5D6AE0) only when set; the sky
    /// (0x60F890) is drawn either way.
    pub env_world: bool,
    /// g_CarShadowEnable (0x870CC0, custom 2): 0 skips the shadow and the
    /// underglow (0x616D40 in the front end's car draw, 0x622113 in the
    /// game's).
    pub shadows: bool,
    /// g_CarHeadlightEnable (0x870CC4, custom 3): 0 skips view 9 (the
    /// headlight projector: 0x5CB200, its cull context 0x5B9D55) and the
    /// worldheadlight remap (0x5C597C).
    pub headlight: bool,
    /// g_WorldLodLevel (0x870CF4, custom 6) below 2: the main view draws
    /// every scenery instance's LOD B (0x5BE1D0 → 0x5D6717).
    pub world_lod_b: bool,
    /// g_FogEnable (0x870CD8, custom 12): FOGON for the main view's world
    /// effects (0x5CB0B1; the road reflection and the cube get 0,
    /// 0x5CAB22).
    pub fog: bool,
}

impl Display {
    /// Everything on: what the renderer did before these were read.
    pub const ALL: Display =
        Display { env_rate: 3, env_world: true, shadows: true, headlight: true, world_lod_b: false, fog: true };

    pub fn from_custom(custom: &[u8; 22]) -> Display {
        Display {
            env_rate: custom[0].min(3),
            env_world: custom[1] != 0,
            shadows: custom[2] != 0,
            headlight: custom[3] != 0,
            world_lod_b: custom[6] < 2,
            fog: custom[12] != 0,
        }
    }
}

impl Default for Display {
    fn default() -> Display {
        Display::ALL
    }
}

/// The cube faces (views 0x10–0x15) 0x5C397B renders this frame: the
/// row `frame % period[rate] + 6 · (rate − 1)` of the 6 × 6 table
/// 0x7FEEA0, periods 0x7FEE90 = {_, 6, 4, 1}; `frame` is 0x870818, which
/// 0x5D2C5E counts up once per frame. Rate 1 renders one face a frame,
/// rate 2 faces 0 and 4 every frame and one of 1, 2, 3, 5 in turn, rate 3
/// all six.
pub fn env_faces(rate: u8, frame: u32) -> [bool; 6] {
    const PERIOD: [u32; 4] = [0, 6, 4, 1];
    const ROWS: [[bool; 6]; 13] = {
        const O: bool = false;
        const I: bool = true;
        [
            [I, O, O, O, O, O],
            [O, I, O, O, O, O],
            [O, O, I, O, O, O],
            [O, O, O, I, O, O],
            [O, O, O, O, I, O],
            [O, O, O, O, O, I],
            [I, I, O, O, I, O],
            [I, O, I, O, I, O],
            [I, O, O, I, I, O],
            [I, O, O, O, I, I],
            [O; 6],
            [O; 6],
            [I; 6],
        ]
    };
    if rate == 0 {
        return [false; 6];
    }
    let rate = rate.min(3) as usize;
    ROWS[(frame % PERIOD[rate]) as usize + 6 * (rate - 1)]
}

/// Tick form (frame-rate independence): the faces due at tick `m`. 0x870818 counts once
/// per frame (0x5D2C5E) as a u32, so at the original's 60 fps tick m is its
/// frame m: [`env_faces`] of the counter's value (m mod 2³², as the u32
/// wraps). The cube faces due at m are rendered from S_m.
pub fn env_faces_tick(rate: u8, tick: u64) -> [bool; 6] {
    env_faces(rate, tick as u32)
}

/// The faces due for each tick a drawn frame spans (`ticks`: those run
/// since the last drawn frame; two per frame at 30 Hz, none for a frame
/// between ticks), in order, each to be rendered from its own S_m.
pub fn env_faces_span(rate: u8, ticks: std::ops::Range<u64>) -> Vec<(u64, [bool; 6])> {
    ticks.map(|m| (m, env_faces_tick(rate, m))).collect()
}

/// docs/uncapped-fps.md, "Environment cube on ticks": the faces a
/// drawn frame renders so that its cube is the one the original's frame
/// `drawn` samples, whatever the display rate. At its 60 fps the original
/// renders the faces [`env_faces_tick`] of each frame (counter 0x870818,
/// incremented once per frame at 0x5D2C5E, read at 0x5C3981 before the
/// increment, never reset) from that frame's car and camera; each face
/// render clears the face, so the cube after frame `drawn` holds, per
/// face, the render of the newest tick ≤ `drawn` at which it was due.
///
/// `last`: the newest tick already rendered into this cube (None: a new
/// cube, which gets `drawn`'s own faces alone, as the original's first
/// frame on a new cube; the rest stay black until due). `every`: the
/// Graphics page's Reflection Update Rate "Every Frame" (clone deviation):
/// all six faces every tick. Returns, oldest first, each tick in
/// (`last`, `drawn`] that is the newest due tick of a face, with those
/// faces; each face at most once (a face due at several of the ticks is
/// rendered once, from the newest: the same pixels). Ticks before
/// `drawn` − 5 are never needed: every face is due at least once in any
/// six consecutive ticks at every rate (periods 6, 4, 1). Empty for a
/// frame with no new tick (`last` = `drawn`) and at rate 0.
pub fn env_faces_due(rate: u8, every: bool, last: Option<u64>, drawn: u64) -> Vec<(u64, [bool; 6])> {
    if rate == 0 {
        return Vec::new();
    }
    let from = match last {
        Some(l) if l >= drawn => return Vec::new(),
        Some(l) => (l + 1).max(drawn.saturating_sub(5)),
        None => drawn,
    };
    let mut taken = [false; 6];
    let mut out: Vec<(u64, [bool; 6])> = Vec::new();
    for m in (from..=drawn).rev() {
        let due = if every { [true; 6] } else { env_faces_tick(rate, m) };
        let mut faces = [false; 6];
        for i in 0..6 {
            faces[i] = due[i] && !taken[i];
            taken[i] |= faces[i];
        }
        if faces.contains(&true) {
            out.push((m, faces));
        }
        if !taken.contains(&false) {
            break;
        }
    }
    out.reverse();
    out
}

#[cfg(test)]
mod tick_tests {
    use super::*;

    /// The cube as (face → the tick its image is from), after a drawn
    /// frame at `drawn` that rendered `env_faces_due(…, last, drawn)`.
    fn commit(cube: &mut [Option<u64>; 6], rate: u8, every: bool, last: Option<u64>, drawn: u64) {
        for (m, faces) in env_faces_due(rate, every, last, drawn) {
            for i in 0..6 {
                if faces[i] {
                    cube[i] = Some(m);
                }
            }
        }
    }

    /// The reference: the original at 60 fps, every tick drawn and its
    /// own faces rendered.
    fn reference(rate: u8, every: bool, first: u64, upto: u64) -> [Option<u64>; 6] {
        let mut cube = [None; 6];
        for m in first..=upto {
            let due = if every { [true; 6] } else { env_faces_tick(rate, m) };
            for i in 0..6 {
                if due[i] {
                    cube[i] = Some(m);
                }
            }
        }
        cube
    }

    /// A frame with no new tick renders nothing; a new cube renders the
    /// drawn tick's own faces; each face at most once, oldest tick first.
    #[test]
    fn faces_due_basics() {
        assert!(env_faces_due(1, false, Some(10), 10).is_empty());
        assert!(env_faces_due(0, false, Some(1), 10).is_empty());
        assert!(env_faces_due(3, true, Some(11), 10).is_empty());
        assert_eq!(env_faces_due(1, false, None, 8), vec![(8, env_faces_tick(1, 8))]);
        // 30 Hz: two ticks, two faces at rate 1.
        assert_eq!(env_faces_due(1, false, Some(6), 8), vec![(7, env_faces_tick(1, 7)), (8, env_faces_tick(1, 8))]);
        // Rate 2: faces 0 and 4 once (from the newer tick).
        let due = env_faces_due(2, false, Some(6), 8);
        assert_eq!(due.len(), 2);
        assert!(!due[0].1[0] && !due[0].1[4] && due[1].1[0] && due[1].1[4]);
        // Rate 3 and "Every Frame": all six from the newest tick only.
        assert_eq!(env_faces_due(3, false, Some(2), 8), vec![(8, [true; 6])]);
        assert_eq!(env_faces_due(1, true, Some(2), 8), vec![(8, [true; 6])]);
        // A long gap: never more than the last six ticks, all faces once.
        let due = env_faces_due(1, false, Some(0), 1000);
        assert_eq!(due.len(), 6);
        assert!(due.iter().all(|(m, _)| *m > 994));
    }

    /// The cube a frame samples is a function of its tick alone: at 60 /
    /// 30 / 120 / 200 Hz and an uneven display (several ticks a frame,
    /// stalls, frames between ticks), after every drawn frame the cube
    /// equals the 60 fps reference at that frame's tick, at every rate
    /// setting and with "Every Frame".
    #[test]
    fn cube_is_a_function_of_the_tick() {
        // Drawn ticks per display: the tick each frame's cube stands at.
        let mut schedules: Vec<(&str, Vec<u64>)> = vec![
            ("60", (1..=600).collect()),
            ("30", (1..=300).map(|k| 2 * k).collect()),
            ("120", (2..=1200).map(|k| k / 2).collect()),
            ("200", (10..=2000).map(|k| k * 3 / 10).collect()),
        ];
        let mut jitter = Vec::new();
        let (mut t, mut x) = (1u64, 0x9E37_79B9u64);
        while t < 600 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            // 0 (a frame between ticks), 1, 2, 3 ticks, or a stall of 9.
            t += [0, 0, 1, 1, 2, 3, 9][(x % 7) as usize];
            jitter.push(t);
        }
        schedules.push(("jitter", jitter));
        for rate in 0..=3u8 {
            for every in [false, true] {
                for (name, frames) in &schedules {
                    let first = frames[0];
                    let mut cube = [None; 6];
                    let mut last = None;
                    for &d in frames {
                        commit(&mut cube, rate, every, last, d);
                        last = Some(d);
                        // The new cube's first frame has its own faces alone (the
                        // original's cube created at `first`); from then on the
                        // reference.
                        assert_eq!(cube, reference(rate, every && rate != 0, first, d), "{name} rate {rate} every {every} tick {d}");
                    }
                }
            }
        }
    }

    /// 600 ticks = 600 frames of the per-frame schedule (a frame counter
    /// from 0); 30 Hz frames span two ticks and render both ticks' faces;
    /// a frame between ticks renders none.
    #[test]
    fn faces_per_tick_are_the_per_frame_schedule() {
        for rate in 0..=3u8 {
            let mut frame_count = 0u32;
            for m in 0..600u64 {
                assert_eq!(env_faces_tick(rate, m), env_faces(rate, frame_count), "rate {rate} tick {m}");
                frame_count = frame_count.wrapping_add(1);
            }
            for k in 0..300u64 {
                let span = env_faces_span(rate, 2 * k..2 * k + 2);
                assert_eq!(span, vec![(2 * k, env_faces(rate, 2 * k as u32)), (2 * k + 1, env_faces(rate, 2 * k as u32 + 1))]);
            }
            assert!(env_faces_span(rate, 7..7).is_empty());
        }
        // The u32 counter's wrap.
        assert_eq!(env_faces_tick(1, (1u64 << 32) + 5), env_faces(1, 5));
    }
}
