use super::*;

/// The flare pools 0x82CC30 (two lists of flare record pointers, 200
/// apiece; counts 0x82DAE8, capacities 0x7F3D44 = 200 and 50, a mask byte
/// per slot at 0x82D398). 0x48AB20(pool, mask) hands out the next record
/// (none when full); 0x48AC00, at the end of each world frame
/// (0x5CBAF1), empties both and runs the blink: 0x8358E4 += the frame time
/// (0x865150), and past 0.5 s (0x784260) it is zeroed and 0x82DAF0 is
/// inverted (0 / −1; BSS, so it starts off).
#[derive(Default)]
pub struct Pools {
    /// (flare, mask) per pool, in the order handed out.
    pub flares: [Vec<(Flare, u8)>; 2],
    /// LOD Fade (clone deviation): per pool, the flares' intensity
    /// factors by index (`add_markers_faded`; empty when every one is 1,
    /// as natively).
    fades: [Vec<(usize, f32)>; 2],
    /// 0x8358E4 and 0x82DAF0 as [`Pools::reset`] steps them.
    blink_clock: FlareBlink,
    /// 0x82DAF0 as this frame's sprites read it: pool 1's type 13 is drawn
    /// only while it is set.
    pub blink: bool,
}

/// Tick form (frame-rate independence): the flare blink of 0x48AC00, 0x8358E4 += the
/// frame time (0x865150), past 0.5 s zeroed and 0x82DAF0 inverted.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlareBlink {
    time: f32,
    on: bool,
}

impl FlareBlink {
    /// One tick of `dt` (the world dt the per-frame path hands
    /// [`Pools::reset`]). -> S_n: the blink tick n's frame draws.
    pub fn step(&mut self, dt: f32) -> bool {
        self.time += dt;
        if self.time > BLINK_HALF {
            self.time = 0.0;
            self.on = !self.on;
        }
        self.on
    }

    /// The blink a frame at α between ticks draws: a discrete value, held
    /// at S_{n−1} (§1.3).
    pub fn drawn(prev: bool, _cur: bool, _alpha: f32) -> bool {
        prev
    }

    /// 0x8358E4, s.
    pub fn time(&self) -> f32 {
        self.time
    }
}

/// 0x7F3D44: the pools' capacities.
pub const POOL_CAPACITY: [usize; 2] = [200, 50];
/// 0x784260: the blink's half period, s.
const BLINK_HALF: f32 = 0.5;
/// 0x803A70: the cosine past which a reflector (types 10–12) glows in
/// 0x490920's modes 3 and 4.
const REFLECTOR_COS: f32 = 0.96;

impl Pools {
    /// Dev harness (rate gate, `state_hash` `vis`): the flare blink's clock and state as bits.
    #[cfg(feature = "dev-harness")]
    pub fn dev_bits(&self) -> [u32; 2] {
        [self.blink_clock.time.to_bits(), u32::from(self.blink)]
    }

    /// The blink's tick state (tests).
    #[cfg(test)]
    pub(crate) fn blink_clock(&self) -> FlareBlink {
        self.blink_clock
    }

    /// 0x48AC00: empty the pools and step the blink by `dt`. The per-frame
    /// path over the tick form: one [`FlareBlink::step`], drawn at α = 0.
    pub fn reset(&mut self, dt: f32) {
        let on = self.blink_clock.step(dt);
        self.blink = FlareBlink::drawn(on, on, 0.0);
        self.clear();
    }

    /// The pools emptied for this frame's fill (a pure per-frame build;
    /// the tick form sets [`Pools::blink`] from its [`FlareBlink`]).
    pub fn clear(&mut self) {
        self.flares[0].clear();
        self.flares[1].clear();
        self.fades[0].clear();
        self.fades[1].clear();
    }

    /// 0x48AB20: false when the pool is full.
    fn add(&mut self, pool: usize, mask: u8, flare: Flare) -> bool {
        if self.flares[pool].len() >= POOL_CAPACITY[pool] {
            return false;
        }
        self.flares[pool].push((flare, mask));
        true
    }

    /// 0x433090's glow for one of the flag girl's flares (see
    /// `character::SceneCharacters::flare_glows`): pool 1, mask 3 (views 1
    /// and 2), type 14, flags 2 (0x433090: 0x48AB20(1, 3), +0x2D = 2,
    /// +0x2C = 0x0E).
    pub fn add_flag_girl_flare(&mut self, at: Vec3) {
        self.add(1, 3, Flare { position: at, direction: Vec3::ZERO, kind: 14, flags: 2 });
    }

    /// 0x5D6580's light markers (0x5D6830…0x5D69AB) for one scenery
    /// instance view 1 took (mask 1; view 2 would give 2): each marker of
    /// the solid drawn goes into pool 1 when its kind (+0x04) is 3, else
    /// pool 0, as type kind + 10 at the instance matrix × (marker − the
    /// solid's +0x70); kind 3 gets flags 4 and the direction (P − the
    /// instance's translation) in xy, normalised in 2D (0x43CE50: (1, 0)
    /// for zero), z 0; the others flags 2. A full pool skips the marker.
    pub fn add_markers(&mut self, transform: Mat4, markers: &[(u32, Vec3)]) {
        self.add_markers_faded(transform, markers, 1.0);
    }

    /// [`Pools::add_markers`] for an instance LOD Fade (clone deviation)
    /// has part-way in or out: its flares glow at `fade` of their
    /// intensity in the main view (the road reflection's keep theirs).
    pub fn add_markers_faded(&mut self, transform: Mat4, markers: &[(u32, Vec3)], fade: f32) {
        let first = [self.flares[0].len(), self.flares[1].len()];
        self.add_markers_inner(transform, markers);
        if fade < 1.0 {
            for pool in 0..2 {
                self.fades[pool].extend((first[pool]..self.flares[pool].len()).map(|i| (i, fade)));
            }
        }
    }

    fn add_markers_inner(&mut self, transform: Mat4, markers: &[(u32, Vec3)]) {
        let origin = transform.w_axis.truncate();
        for &(kind, local) in markers {
            let p = transform.transform_point3(local);
            let pool = usize::from(kind == 3);
            let (flags, direction) = if kind == 3 {
                let d = (p - origin).truncate();
                let len = d.length();
                let d = if len != 0.0 { d / len } else { glam::Vec2::X };
                (4, Vec3::new(d.x, d.y, 0.0))
            } else {
                (2, Vec3::ZERO)
            };
            self.add(pool, 1, Flare { position: p, direction, kind: (kind as u8).wrapping_add(10), flags });
        }
    }
}

/// 0x494270(view, 0, 0) in the glow pass (0x5CB61C): pools 0 and 1, the
/// slots whose mask has bit 1 (view 1's), skipping pool 1's type 13 while
/// the blink is off, each through 0x490920(view, flare, identity, 1.0, 0,
/// mode 3). Mode 3 is mode 0 without the screen-angle turn, except that a
/// flare of a type other than 13 and 14 (the reflectors, 10–12) glows only
/// within the player's headlights: with v = unit(P − car +0x60) and f the
/// car's forward row (+0x90; car 0x5F0E40(0x5ED990(0)), the first human),
/// it is dropped when v · f < 0.96 and drawn at intensity (v · f − 0.96) /
/// 0.04 (0x490D96…0x490E46).
pub fn pool_sprites(view: &View, pools: &Pools, car: Option<(Vec3, Vec3)>, out: &mut Vec<Quad>) {
    if view.front_end {
        return;
    }
    for (pool, list) in pools.flares.iter().enumerate() {
        for (n, (flare, mask)) in list.iter().enumerate() {
            if mask & 1 == 0 || (pool == 1 && flare.kind == 13 && !pools.blink) {
                continue;
            }
            let mut intensity = 1.0;
            if flare.kind != 13 && flare.kind != 14 {
                let Some((at, forward)) = car else { continue };
                let c = (flare.position - at).normalize_or_zero().dot(forward);
                if c < REFLECTOR_COS {
                    continue;
                }
                intensity = (c - REFLECTOR_COS) / (1.0 - REFLECTOR_COS);
            }
            if let Ok(k) = pools.fades[pool].binary_search_by_key(&n, |f| f.0) {
                intensity *= pools.fades[pool][k].1;
            }
            sprites_in(view, flare, Mat4::IDENTITY, intensity, None, false, out);
        }
    }
}

/// 0x494270(view 4, 0, 1) in the reflection (0x5CACB5): pool 1 alone, mask
/// bit 1, type 13 only while the blink is on, each through
/// 0x490920(view 4, flare, identity, 1.0, param 5 = 1, mode 2), which
/// probes the mirror height every time (param 5 = 1).
pub fn pool_reflection_sprites(
    view: &View,
    pools: &Pools,
    wetness: f32,
    ground: &mut dyn FnMut(Vec3) -> Option<f32>,
    out: &mut Vec<Quad>,
) {
    if view.front_end {
        return;
    }
    let streak = streak_factor(wetness);
    for (flare, mask) in &pools.flares[1] {
        if mask & 1 == 0 || (flare.kind == 13 && !pools.blink) {
            continue;
        }
        let h = mirror_height(flare.position, ground(flare.position));
        reflection_sprites(view, flare, Mat4::IDENTITY, h, streak, out);
    }
}
