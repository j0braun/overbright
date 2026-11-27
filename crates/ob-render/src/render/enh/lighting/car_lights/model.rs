use super::*;
/// What a car light is (0x615640's kinds: the head- and brake-light
/// markers; a brake light not braking is the tail light).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Head,
    Tail,
    Brake,
}

/// One lamp in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub position: Vec3,
    /// The way it shines (the marker's direction under the car's matrix).
    pub facing: Vec3,
    /// Linear colour × intensity.
    pub colour: Vec3,
    pub kind: Kind,
}

/// A drawn car's lights.
#[derive(Clone, Debug, PartialEq)]
pub struct Car {
    pub transform: Mat4,
    /// The car's box in its own axes (`Car::shadow_box`).
    pub min: Vec3,
    pub max: Vec3,
    pub lights: Vec<Light>,
    /// Render info +0x380 (the headlight pool's colour).
    pub head_colour: [u8; 3],
    /// The underglow's linear colour × alpha (`shadow::neon_rgb`) when the
    /// car has a lit, drawn `%s_NEON`.
    pub neon: Option<Vec3>,
}

/// The brake / tail lights' colour (linear; a red LED / bulb behind red
/// glass).
pub const TAIL_COLOUR: Vec3 = Vec3::new(1.0, 0.07, 0.03);
/// Tail and brake intensity (0x615640 draws the tail sprites at 0.85 and
/// braking at 16.85, clamped to 1; here a ratio of 3).
pub(in crate::render) const TAIL_LEVEL: f32 = 0.33;
pub(in crate::render) const BRAKE_LEVEL: f32 = 1.0;

impl Car {
    pub fn position(&self) -> Vec3 {
        self.transform.w_axis.truncate()
    }

    pub(super) fn rear(&self) -> Option<(Vec3, Vec3, Vec3, bool)> {
        mean_of(self.lights.iter().filter(|l| l.kind != Kind::Head))
            .map(|(p, f, c)| (p, f, c, self.lights.iter().any(|l| l.kind == Kind::Brake)))
    }

    pub(super) fn heads(&self) -> Option<(Vec3, Vec3, Vec3)> {
        mean_of(self.lights.iter().filter(|l| l.kind == Kind::Head))
    }

    /// The ground under the car's middle (its box's floor) and its half
    /// extents along its forward and side axes.
    pub(super) fn floor(&self) -> (Vec3, Vec3, Vec3, f32, f32) {
        let mid = (self.min + self.max) * 0.5;
        let c = self.transform.transform_point3(Vec3::new(mid.x, mid.y, self.min.z));
        let fwd = self.transform.x_axis.truncate().normalize_or(Vec3::X);
        let side = self.transform.y_axis.truncate().normalize_or(Vec3::Y);
        (c, fwd, side, 0.5 * (self.max.x - self.min.x), 0.5 * (self.max.y - self.min.y))
    }
}

/// The mean position and facing and the summed colour of `lights`.
pub(in crate::render) fn mean_of<'a>(lights: impl Iterator<Item = &'a Light>) -> Option<(Vec3, Vec3, Vec3)> {
    let (mut p, mut f, mut c, mut n) = (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 0.0);
    for l in lights {
        p += l.position;
        f += l.facing;
        c += l.colour;
        n += 1.0;
    }
    (n > 0.0).then(|| (p / n, f.normalize_or(Vec3::X), c))
}

/// A car's lights as 0x615640 lights its glow sprites: the left and right
/// headlights in the headlight colour, the left and right brake lights
/// (tail level, brake level braking), the centre brake light only braking;
/// the centre headlight never.
pub fn car(
    transform: Mat4,
    flares: &[crate::glow::CarFlare],
    headlight: [u8; 3],
    braking: bool,
    shadow_box: (Vec3, Vec3),
    neon: Option<Vec3>,
) -> Car {
    use crate::glow::{CENTRE_BRAKELIGHT, LEFT_BRAKELIGHT, LEFT_HEADLIGHT, RIGHT_BRAKELIGHT, RIGHT_HEADLIGHT};
    // HeadlightColor's clamp (0x5BDAB0: bytes / 128, ≤ 1): a near-white.
    let head = Vec3::from(headlight.map(|b| (f32::from(b) / 128.0).min(1.0)));
    let lights = flares
        .iter()
        .filter_map(|f| {
            let (kind, colour) = match f.hash {
                LEFT_HEADLIGHT | RIGHT_HEADLIGHT => (Kind::Head, head),
                LEFT_BRAKELIGHT | RIGHT_BRAKELIGHT if braking => (Kind::Brake, TAIL_COLOUR * BRAKE_LEVEL),
                LEFT_BRAKELIGHT | RIGHT_BRAKELIGHT => (Kind::Tail, TAIL_COLOUR * TAIL_LEVEL),
                CENTRE_BRAKELIGHT if braking => (Kind::Brake, TAIL_COLOUR * (0.5 * BRAKE_LEVEL)),
                _ => return None,
            };
            let facing = transform.transform_vector3(f.flare.direction).normalize_or(Vec3::X);
            Some(Light { position: transform.transform_point3(f.flare.position), facing, colour, kind })
        })
        .collect();
    Car { transform, min: shadow_box.0, max: shadow_box.1, lights, head_colour: headlight, neon }
}

/// The look constants (inferred, by eye on the night race start, the rain
/// and the neon district; `NFSU2_CARLIGHTS=k=v,…` overrides them by name).
#[derive(Clone, Copy, Debug)]
pub struct Look {
    /// Scattering per metre of dry night air at full beam (× the shared medium's thickness in rain,
    /// `air::air_density`).
    pub density: f32,
    /// The headlight cones' gain in the air.
    pub spot: f32,
    /// The cones' in-scatter toward an eye inside the beam fades out between these cosines of
    /// the angle between the light's way and the way to the eye: the forward-scattered aureole
    /// around a lamp the eye looks into is its glare, which the native glow sprite and the lens
    /// core draw (added again over the beam's length, oncoming fronts once went white).
    pub aureole: (f32, f32),
    /// The lamps' halos' gain in the air.
    pub point: f32,
    /// Henyey–Greenstein asymmetry of the air (forward scattering).
    pub g: f32,
    /// Lens streaks' gain (0 off).
    pub streak: f32,
    /// A tail / brake light on the road: gain and reach (m).
    pub tail_road: f32,
    pub tail_reach: f32,
    pub brake_reach: f32,
    /// The underglow on the road: gain and soft reach outside the car (m).
    pub neon_road: f32,
    pub neon_reach: f32,
    /// Other cars' headlights on a car's body: gain and reach (m).
    pub head_car: f32,
    pub head_car_reach: f32,
    /// Tail / brake / underglow on a car's body: gain.
    pub glow_car: f32,
    /// The lamps' halos in the air may add at most this much light
    /// on average over the screen (linear, after the soft knee); above it
    /// every halo is scaled down alike ([`halo_veil_scale`]), so a grid of
    /// cars facing the camera does not floodlight the scene.
    pub veil: f32,
    /// The cars' tail / brake / underglow light on the road summed
    /// over every car (Rec. 709 luminance of the glows' colours) is kept
    /// within this; above it every glow is scaled down alike
    /// ([`bound_glows`]).
    pub road_budget: f32,
}

pub const LOOK: Look = Look {
    density: 0.035,
    spot: 2.2,
    aureole: (0.6, 0.9),
    point: 0.3,
    g: 0.3,
    streak: 2.0,
    tail_road: 0.55,
    tail_reach: 4.5,
    brake_reach: 7.5,
    // At max settings in the rain the road washed out
    // around the car: 0.45 / 1.8 m added on top of the native underglow
    // pool (the car's %s_NEON quad) on a wet road was p95 +12, glow +0.6
    // over the row without it (look rule for highlights and glow: ±4 / ±0.2); 0.15 / 1.5 m: +3.7 / +0.2.
    neon_road: 0.15,
    neon_reach: 1.5,
    head_car: 1.4,
    head_car_reach: 24.0,
    glow_car: 0.6,
    // The race start's grid (22 lamps, several
    // within the halos' 6 m of the camera) was +8.6 mean / +15.6 p95 over
    // Original, +7 mean of it these halos (by elimination, `point=0`).
    veil: 0.0015,
    // The race start's 4 glows summed 1.75 (+0.5 mean, +3.4 p95
    // by elimination). One car's tail lights (≈ 0.1) stay as they were; a
    // braking car (≈ 0.36) or a grid of them light the road like ≈ 0.15.
    road_budget: 0.15,
};

/// [`LOOK`] with `NFSU2_CARLIGHTS` applied.
pub fn look() -> Look {
    static L: std::sync::OnceLock<Look> = std::sync::OnceLock::new();
    *L.get_or_init(|| {
        let mut l = LOOK;
        if let Ok(v) = crate::render::debug_env::nfsu2_carlights() {
            for kv in v.split(',') {
                let Some((k, v)) = kv.split_once('=') else { continue };
                let Ok(v) = v.trim().parse::<f32>() else { continue };
                match k.trim() {
                    "density" => l.density = v,
                    "spot" => l.spot = v,
                    "aureole_lo" => l.aureole.0 = v,
                    "aureole_hi" => l.aureole.1 = v,
                    "point" => l.point = v,
                    "g" => l.g = v,
                    "streak" => l.streak = v,
                    "tail_road" => l.tail_road = v,
                    "tail_reach" => l.tail_reach = v,
                    "brake_reach" => l.brake_reach = v,
                    "neon_road" => l.neon_road = v,
                    "neon_reach" => l.neon_reach = v,
                    "head_car" => l.head_car = v,
                    "head_car_reach" => l.head_car_reach = v,
                    "glow_car" => l.glow_car = v,
                    "veil" => l.veil = v,
                    "road_budget" => l.road_budget = v,
                    _ => log::warn!("NFSU2_CARLIGHTS: unknown {k}"),
                }
            }
        }
        l
    })
}
