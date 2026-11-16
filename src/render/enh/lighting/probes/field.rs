use super::*;
/// Probe spacing along the roads (m).
pub const SPACING: f32 = 24.0;
/// Probe height above the road (a car body's middle).
pub const HEIGHT: f32 = 1.0;
/// A probe's face size (texels).
pub const SIZE: u32 = 16;
/// Probes farther than this from the car do not light it.
pub const REACH: f32 = 40.0;
/// Probes are baked within this distance of the player's car (the camera before).
pub const BAKE_RADIUS: f32 = 180.0;
/// Arriving where nothing near is baked: these within this radius at once.
pub const BURST_RADIUS: f32 = 70.0;
pub const BURST_MAX: usize = 48;
/// What a probe face does to the car's ambient light on that side
/// ([`ambient_terms`]): it replaces the face's hue where the probe
/// face holds some light (luminance LIT_LOW..LIT_HIGH, smoothstep; a dark
/// face changes nothing), each channel moved by at most HUE_CAP of the
/// face's luminance, and lifts its brightness by at most LIFT_CAP where the
/// probe is brighter than a lit street (LUMA_FLOOR .. 3 × LUMA_FLOOR).
pub const LIT_LOW: f32 = 0.04;
pub const LIT_HIGH: f32 = 0.15;
pub const LUMA_FLOOR: f32 = 0.2;
pub const HUE_CAP: f32 = 0.2;
pub const LIFT_CAP: f32 = 0.05;
/// Grid cell for the nearest-probe lookups (m).
pub(in crate::render) const CELL: f32 = 48.0;

/// A frame's bake ([`ProbeField::plan`]).
#[derive(Clone, Debug, PartialEq)]
pub enum BakePlan {
    Nothing,
    One(u32),
    Burst(Vec<u32>),
}

/// The probes of one region and what has been baked of them.
pub struct ProbeField {
    pub positions: Vec<Vec3>,
    /// Ambient cube per probe: +x, −x, +y, −y, +z, −z (face mean colours,
    /// 0..1 as drawn); None until baked.
    pub baked: Vec<Option<[Vec3; 6]>>,
    cells: HashMap<(i32, i32), Vec<u32>>,
    /// A drive's set-up gave up baked probes ([`ProbeField::release_seeing`]):
    /// the next plan bakes those near the car at once, as an arrival does.
    pub rebake: bool,
}

pub(in crate::render) fn cell_of(p: Vec3) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

impl ProbeField {
    pub fn new(positions: Vec<Vec3>) -> ProbeField {
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, p) in positions.iter().enumerate() {
            cells.entry(cell_of(*p)).or_default().push(i as u32);
        }
        ProbeField { baked: vec![None; positions.len()], positions, cells, rebake: false }
    }

    /// Probes along every record of a route file: the control points and
    /// points every [`SPACING`] between them, none within half a spacing
    /// of one already placed.
    pub fn along_roads(routes: &[RouteFile]) -> ProbeField {
        let mut placed: Vec<Vec3> = Vec::new();
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        let mut add = |p: Vec3, placed: &mut Vec<Vec3>| {
            let p = p + Vec3::Z * HEIGHT;
            let (cx, cy) = cell_of(p);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(list) = cells.get(&(cx + dx, cy + dy))
                        && list.iter().any(|&i| placed[i as usize].distance_squared(p) < (SPACING * 0.5).powi(2))
                    {
                        return;
                    }
                }
            }
            cells.entry((cx, cy)).or_default().push(placed.len() as u32);
            placed.push(p);
        };
        for r in routes.iter().flat_map(|f| &f.records) {
            for w in r.points.windows(2) {
                let (a, b) = (w[0].position, w[1].position);
                let n = ((a.distance(b) / SPACING).ceil() as usize).clamp(1, 64);
                for k in 0..n {
                    add(a.lerp(b, k as f32 / n as f32), &mut placed);
                }
            }
            if let Some(last) = r.points.last() {
                add(last.position, &mut placed);
            }
        }
        ProbeField::new(placed)
    }

    /// Probe indices within `radius` of `at`, nearest first.
    pub fn near(&self, at: Vec3, radius: f32) -> Vec<(f32, u32)> {
        let (cx, cy) = cell_of(at);
        let r = (radius / CELL).ceil() as i32;
        let mut out = Vec::new();
        for dx in -r..=r {
            for dy in -r..=r {
                if let Some(list) = self.cells.get(&(cx + dx, cy + dy)) {
                    for &i in list {
                        let d = self.positions[i as usize].distance(at);
                        if d <= radius {
                            out.push((d, i));
                        }
                    }
                }
            }
        }
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out
    }

    /// What to bake this frame around `at` (the player's car): a burst on
    /// arrival — probes in reach, none of them baked — of every unbaked
    /// probe within [`BURST_RADIUS`] (at most [`BURST_MAX`]); else the
    /// nearest unbaked probe within [`BAKE_RADIUS`]; else nothing. A baked
    /// probe is never baked again (with no probe in reach the
    /// arrival test held every frame, and the burst re-baked up to
    /// BURST_MAX baked probes a frame off the roads).
    /// After a set-up's switch ([`ProbeField::rebake`]) the probes it gave
    /// up within [`BURST_RADIUS`] are a burst too: the field round the car
    /// is then baked in the same frame a fresh load bakes it in.
    pub fn plan(&self, at: Vec3) -> BakePlan {
        let unbaked = |i: &u32| self.baked[*i as usize].is_none();
        let reach = self.near(at, REACH);
        if self.rebake || (!reach.is_empty() && reach.iter().all(|(_, i)| unbaked(i))) {
            let burst: Vec<u32> =
                self.near(at, BURST_RADIUS).into_iter().map(|(_, i)| i).filter(unbaked).take(BURST_MAX).collect();
            if !burst.is_empty() {
                return BakePlan::Burst(burst);
            }
        }
        self.near(at, BAKE_RADIUS).into_iter().map(|(_, i)| i).find(unbaked).map_or(BakePlan::Nothing, BakePlan::One)
    }

    /// Gives up every baked probe whose bake can see one of `bounds`
    /// (spheres): a probe's six faces reach `far` along their axes, so it
    /// draws what lies in the box of half-size `far` round it. The probes
    /// given up, in index order; [`ProbeField::rebake`] set when there are any.
    pub fn release_seeing(&mut self, bounds: &[(Vec3, f32)], far: f32) -> Vec<u32> {
        let mut out = Vec::new();
        for &(c, r) in bounds {
            for (_, i) in self.near(c, far * 3f32.sqrt() + r) {
                if self.baked[i as usize].is_none() {
                    continue;
                }
                let gap = ((c - self.positions[i as usize]).abs() - Vec3::splat(far)).max(Vec3::ZERO);
                if gap.length_squared() <= r * r {
                    self.baked[i as usize] = None;
                    out.push(i);
                }
            }
        }
        out.sort_unstable();
        self.rebake |= !out.is_empty();
        out
    }

    /// The ambient cube at `at`: the baked probes within [`REACH`] (at most
    /// four), weighted by inverse distance; None without one.
    pub fn sample(&self, at: Vec3) -> Option<[Vec3; 6]> {
        let mut sum = [Vec3::ZERO; 6];
        let mut total = 0.0;
        for (d, i) in self.near(at, REACH).into_iter().filter(|(_, i)| self.baked[*i as usize].is_some()).take(4) {
            let w = 1.0 / (d + 2.0) - 1.0 / (REACH + 2.0);
            let cube = self.baked[i as usize].unwrap();
            for (s, c) in sum.iter_mut().zip(cube) {
                *s += c * w;
            }
            total += w;
        }
        (total > 0.0).then(|| sum.map(|s| s / total))
    }
}

pub(in crate::render) const LUMA_WEIGHTS: Vec3 = Vec3::new(0.2126, 0.7152, 0.0722);

fn smooth(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The world axes in the ambient cube's face order (+x, −x, +y, −y, +z, −z).
const FACES: [Vec3; 6] = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z];

/// The car harmonic's colour (rgb) towards `d` (world axes).
pub fn harmonic_rgb(h: &[Vec4; 10], d: Vec3) -> Vec3 {
    (h[0]
        + h[1] * d.x * d.x
        + h[2] * d.y * d.y
        + h[3] * d.z * d.z
        + h[4] * d.x * d.y
        + h[5] * d.x * d.z
        + h[6] * d.y * d.z
        + h[7] * d.x
        + h[8] * d.y
        + h[9] * d.z)
        .truncate()
}

/// What the probe's face `probe` makes of the car's ambient light `native`
/// on that side (replacing the earlier add-on): the probe's hue at the
/// native luminance (each channel within ±HUE_CAP of it), lifted by at most
/// LIFT_CAP where the probe is brighter than a lit street, blended in by how
/// lit the probe face is. A dark face gives `native` back exactly.
pub fn face_ambient(probe: Vec3, native: Vec3) -> Vec3 {
    let lum_p = probe.dot(LUMA_WEIGHTS);
    let lit = smooth(LIT_LOW, LIT_HIGH, lum_p);
    if lit <= 0.0 {
        return native;
    }
    let lum_n = native.max(Vec3::ZERO).dot(LUMA_WEIGHTS);
    let hue = probe / lum_p.max(1e-6) - Vec3::ONE;
    let hue = hue * (HUE_CAP / hue.abs().max_element().max(HUE_CAP));
    let lift = 1.0 + LIFT_CAP * smooth(LUMA_FLOOR, 3.0 * LUMA_FLOOR, lum_p);
    let target = (Vec3::ONE + hue) * (lum_n * lift);
    native + (target - native) * lit
}

/// An ambient cube as terms of the car harmonic's quadratic (c0 + c1x² +
/// c2y² + c3z² + c4xy + c5xz + c6yz + c7x + c8y + c9z, world axes): per
/// axis d² (P + N) / 2 and, for d·|d| (P − N) / 2, its least-squares linear
/// fit over the sphere, 0.75 d (P − N) / 2. rgb only (w 0: the highlight's
/// strength stays as it is).
pub fn harmonic_terms(cube: &[Vec3; 6]) -> [Vec4; 10] {
    let mut h = [Vec4::ZERO; 10];
    for axis in 0..3 {
        let (p, n) = (cube[axis * 2], cube[axis * 2 + 1]);
        h[1 + axis] = ((p + n) * 0.5).extend(0.0);
        h[7 + axis] = ((p - n) * 0.375).extend(0.0);
    }
    h
}

/// The terms that turn the car's harmonic `native` (world axes)
/// into its probe-lit form: per face [`face_ambient`] − the native colour
/// there, fitted as [`harmonic_terms`]. The probe replaces the ambient it
/// models instead of adding to it, so the car's brightness stays within
/// LIFT_CAP of the native one and its colour within HUE_CAP.
pub fn ambient_terms(cube: &[Vec3; 6], native: &[Vec4; 10]) -> [Vec4; 10] {
    let delta: [Vec3; 6] = std::array::from_fn(|f| {
        let n = harmonic_rgb(native, FACES[f]);
        face_ambient(cube[f], n) - n
    });
    harmonic_terms(&delta)
}

/// A region's route files: free roam first, then every race's forward file.
pub fn region_routes(root: &std::path::Path, region: &str) -> Vec<RouteFile> {
    let dir = crate::game_fs::path(root, &format!("TRACKS/ROUTES{region}"));
    let mut names: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("Routes") && n.ends_with("F.bin"))
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names.insert(0, crate::game_fs::path(root, &format!("TRACKS/ROUTES{region}/RoutesFreeRoam.bin")));
    names.iter().filter_map(|p| std::fs::read(p).ok()).map(|d| RouteFile::parse(&d)).collect()
}

/// `NFSU2_PROBE_LOG=1`: the bake and the cars' probe terms in the log (a
/// check aid).
pub fn log_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| crate::render::debug_env::nfsu2_probe_log())
}

/// The arrival burst's form: 0 not read yet, 1 one wait for the GPU for the
/// whole burst, 2 a wait after each probe.
static BURST_SERIAL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// `NFSU2_PROBE_BURST_SERIAL=1` or [`set_burst_serial`] (a check and
/// measuring aid): the arrival burst waits for each probe's read-back
/// before it draws the next, instead of once for all of them. The field
/// baked is the same either way (the game test
/// `the_arrival_burst_bakes_the_same_field_with_one_wait`).
pub fn burst_serial() -> bool {
    use std::sync::atomic::Ordering::Relaxed;
    match BURST_SERIAL.load(Relaxed) {
        0 => {
            let on = crate::render::debug_env::nfsu2_probe_burst_serial();
            BURST_SERIAL.store(if on { 2 } else { 1 }, Relaxed);
            on
        }
        v => v == 2,
    }
}

/// Sets [`burst_serial`] for the process (the tests).
pub fn set_burst_serial(on: bool) {
    BURST_SERIAL.store(if on { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed);
}

/// The published field's baked cubes, one per probe (None: not baked
/// yet); None without a field. A check aid.
pub fn published_baked() -> Option<Vec<Option<[Vec3; 6]>>> {
    let field = FIELD.read().unwrap().clone()?;
    let baked = field.read().unwrap().baked.clone();
    Some(baked)
}

/// The field the in-game renderer bakes into, for the car lighting.
pub(in crate::render) static FIELD: RwLock<Option<Arc<RwLock<ProbeField>>>> = RwLock::new(None);

pub fn publish(field: Option<Arc<RwLock<ProbeField>>>) {
    *FIELD.write().unwrap() = field;
}

/// The blended ambient cube at `at` (None: probes off, none baked near;
/// `on`: Light Probes in the frame's settings).
pub fn cube_at(at: Vec3, on: bool) -> Option<[Vec3; 6]> {
    if !on {
        return None;
    }
    let field = FIELD.read().unwrap().clone()?;
    field.read().unwrap().sample(at)
}

/// The face means of a read-back cube (`SIZE`² RGBA8 per face, rows
/// `row_bytes` apart, faces `SIZE` rows apart).
pub fn face_means(data: &[u8], row_bytes: usize) -> [Vec3; 6] {
    std::array::from_fn(|f| {
        let mut sum = Vec3::ZERO;
        for y in 0..SIZE as usize {
            let row = &data[(f * SIZE as usize + y) * row_bytes..][..SIZE as usize * 4];
            for px in row.chunks_exact(4) {
                sum += Vec3::new(px[0] as f32, px[1] as f32, px[2] as f32);
            }
        }
        sum / (255.0 * (SIZE * SIZE) as f32)
    })
}
