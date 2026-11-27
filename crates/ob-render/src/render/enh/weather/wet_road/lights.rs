use glam::{Mat4, Vec2, Vec3};

use super::trail_map::{LIGHT_SLOTS, MAX_BOXES};
use crate::render::enh::weather::spray::CarSample;

// ---------------------------------------------------------------------
// The cars' lights and wheels as the game hands them (read only).

/// A car light in the car's axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarLight {
    pub position: Vec3,
    pub direction: Vec3,
    /// Linear colour × intensity.
    pub colour: Vec3,
    /// 0 head, 1 tail / brake.
    pub kind: u8,
}

/// One car's extras this frame: its sim position (to match the drawn car),
/// lights and wheels.
#[derive(Clone, Debug, Default)]
pub struct CarExtra {
    pub at: Vec3,
    pub lights: Vec<CarLight>,
    /// Per wheel (front left, front right, rear left, rear right): the slip
    /// along and across the wheel (m/s) and whether it is down.
    pub wheels: [(f32, f32, bool); 4],
}

/// Tail light colour (the flare's red, **inferred**) and intensities
/// (the native 0.85 / braking 16.85, clamped to 1 for the sprite; here the
/// brake's step is kept as a ×2.6 brighter mirror).
const TAIL_COLOUR: Vec3 = Vec3::new(1.0, 0.06, 0.03);

/// A car's extras at a tick (built by the game's tick from the
/// car's state, no longer pushed by the draw's glow pass): its light
/// flares in the car's axes, the headlight colour (+0x380), the brake flag
/// and the wheels. None with the row off (`tier`: Wet Road Interaction as
/// the tick took it).
pub fn car_extra(
    transform: Mat4,
    flares: &[crate::glow::CarFlare],
    headlight: [u8; 3],
    braking: bool,
    wheels: [(f32, f32, bool); 4],
    tier: u8,
) -> Option<CarExtra> {
    if tier == 0 {
        return None;
    }
    let head = Vec3::new(headlight[0] as f32, headlight[1] as f32, headlight[2] as f32) / 255.0;
    let lights = flares
        .iter()
        .filter_map(|f| {
            use crate::glow::{CENTRE_BRAKELIGHT, LEFT_BRAKELIGHT, LEFT_HEADLIGHT, RIGHT_BRAKELIGHT, RIGHT_HEADLIGHT};
            // As 0x615640 lights them: the centre headlight never, the
            // centre brake light only while braking.
            let (kind, colour) = match f.hash {
                LEFT_HEADLIGHT | RIGHT_HEADLIGHT => (0, head * 1.1),
                LEFT_BRAKELIGHT | RIGHT_BRAKELIGHT => (1, TAIL_COLOUR * if braking { 1.3 } else { 0.5 }),
                CENTRE_BRAKELIGHT if braking => (1, TAIL_COLOUR * 0.65),
                _ => return None,
            };
            Some(CarLight {
                position: f.flare.position,
                direction: f.flare.direction.normalize_or(Vec3::X),
                colour,
                kind,
            })
        })
        .collect();
    Some(CarExtra { at: transform.w_axis.truncate(), lights, wheels })
}

/// A tick's extras (`all`): 64 at most.
pub fn add_extra(all: &mut Vec<CarExtra>, extra: Option<CarExtra>) {
    if let Some(e) = extra.filter(|_| all.len() < 64) {
        all.push(e);
    }
}

/// The extra nearest a drawn car (its sim position within 4 m of the drawn
/// one: the drawn pose is interpolated between steps).
pub fn match_extra<'a>(car: &CarSample, extras: &'a [CarExtra]) -> Option<&'a CarExtra> {
    let at = car.world.w_axis.truncate();
    extras
        .iter()
        .filter(|e| e.at.distance_squared(at) < 16.0)
        .min_by(|a, b| a.at.distance_squared(at).total_cmp(&b.at.distance_squared(at)))
}

/// The main view's camera for the lights' cull (world
/// view-projection, the projection's x scale).
#[derive(Clone, Copy, Debug)]
pub struct ViewCull {
    pub view_proj: Mat4,
    pub scale_x: f32,
}

impl ViewCull {
    /// Whether anything a light at `p` draws on the road can be on screen:
    /// its mirror image (`mirror`, under the road) or its own glow (`p`)
    /// in front of the eye and within the screen's width, widened by
    /// `margin` m (the tail's glow, the streak's width). Not up or down:
    /// a streak runs along the view, from the horizon to the bottom edge.
    pub fn sees(&self, p: Vec3, mirror: Vec3, margin: f32) -> bool {
        [p, mirror].iter().any(|q| {
            let c = self.view_proj * q.extend(1.0);
            c.w > 0.3 && (c.x / c.w).abs() <= 1.15 + margin * self.scale_x / c.w
        })
    }
}

/// The lights to mirror: the cars' (drawn pose) nearest the eye, as the
/// shader's (position, range), (direction, cos outer), (colour, kind).
/// With the main view's camera, only those whose mirror image or
/// glow can be on screen (the cars behind the eye or far to the side
/// left the shader's loop running for nothing).
pub(super) fn gather_lights(
    cars: &[CarSample],
    extras: &[CarExtra],
    eye: Vec3,
    view: Option<ViewCull>,
    max: usize,
) -> Vec<[[f32; 4]; 3]> {
    let mut out: Vec<(f32, [[f32; 4]; 3])> = Vec::new();
    for car in cars {
        let Some(e) = match_extra(car, extras) else { continue };
        let ground = car.contacts.iter().map(|c| c.z).sum::<f32>() * 0.25;
        for l in &e.lights {
            let p = car.world.transform_point3(l.position);
            let d = car.world.transform_vector3(l.direction).normalize_or(Vec3::X);
            let (range, cos) = if l.kind == 0 { (26.0, 0.25) } else { (12.0, -0.2) };
            let dist = p.distance(eye);
            if dist > 120.0 {
                continue;
            }
            if view.is_some_and(|v| {
                !v.sees(p, Vec3::new(p.x, p.y, 2.0 * ground - p.z), if l.kind == 0 { 1.0 } else { 3.0 })
            }) {
                continue;
            }
            out.push((
                dist,
                [p.extend(range).to_array(), d.extend(cos).to_array(), l.colour.extend(f32::from(l.kind)).to_array()],
            ));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().take(max.min(LIGHT_SLOTS)).map(|l| l.1).collect()
}

/// The nearest cars' bodies as boxes: (centre, half length), (forward xy,
/// half width, half height), (0).
pub(super) fn car_boxes(cars: &[CarSample], eye: Vec3) -> Vec<[[f32; 4]; 3]> {
    let mut near: Vec<&CarSample> = cars.iter().filter(|c| c.world.w_axis.truncate().distance(eye) < 60.0).collect();
    near.sort_by(|a, b| a.world.w_axis.truncate().distance(eye).total_cmp(&b.world.w_axis.truncate().distance(eye)));
    near.into_iter()
        .take(MAX_BOXES)
        .map(|c| {
            let ground = c.contacts.iter().map(|p| p.z).sum::<f32>() * 0.25;
            let at = c.world.w_axis.truncate();
            let f = c.world.x_axis.truncate().truncate().normalize_or(Vec2::X);
            [[at.x, at.y, ground + 0.62, 2.15], [f.x, f.y, 0.88, 0.42], [0.0; 4]]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(at: Vec3, speed: f32) -> CarSample {
        let world = Mat4::from_translation(at);
        CarSample {
            world,
            contacts: [
                Vec3::new(1.3, 0.8, 0.0),
                Vec3::new(1.3, -0.8, 0.0),
                Vec3::new(-1.3, 0.8, 0.0),
                Vec3::new(-1.3, -0.8, 0.0),
            ]
            .map(|c| c + at),
            velocity: Vec3::X * speed,
        }
    }

    /// Lights follow the drawn car (not the sim's pose), nearest first, up
    /// to the tier's count; a car without a matching extra has none.
    #[test]
    fn lights_follow_the_drawn_car() {
        let extra = CarExtra {
            at: Vec3::new(10.5, 0.0, 0.0),
            lights: vec![
                CarLight { position: Vec3::new(2.0, 0.6, 0.6), direction: Vec3::X, colour: Vec3::ONE, kind: 0 },
                CarLight { position: Vec3::new(-2.1, 0.6, 0.7), direction: -Vec3::X, colour: TAIL_COLOUR, kind: 1 },
            ],
            wheels: [(0.0, 0.0, true); 4],
        };
        let cars = [car(Vec3::new(10.0, 0.0, 0.0), 20.0), car(Vec3::new(50.0, 0.0, 0.0), 20.0)];
        let l = gather_lights(&cars, std::slice::from_ref(&extra), Vec3::ZERO, None, 16);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0][0][..3], [7.9, 0.6, 0.7]);
        assert_eq!(l[1][0][..3], [12.0, 0.6, 0.6]);
        assert_eq!(gather_lights(&cars, std::slice::from_ref(&extra), Vec3::ZERO, None, 1).len(), 1);
        assert!(match_extra(&cars[1], std::slice::from_ref(&extra)).is_none());
        // With the camera, a car behind the eye or far off to the
        // side mirrors nothing on screen; the one ahead keeps both lights.
        let proj = Mat4::perspective_rh(0.9, 16.0 / 9.0, 0.5, 1000.0);
        let cull = ViewCull {
            view_proj: proj * Mat4::look_to_rh(Vec3::new(0.0, 0.0, 1.5), Vec3::X, Vec3::Z),
            scale_x: proj.x_axis.x,
        };
        assert_eq!(gather_lights(&cars, std::slice::from_ref(&extra), Vec3::ZERO, Some(cull), 16).len(), 2);
        let behind = CarExtra { at: Vec3::new(-9.5, 0.0, 0.0), ..extra.clone() };
        let side = CarExtra { at: Vec3::new(3.5, 40.0, 0.0), ..extra.clone() };
        let others = [car(Vec3::new(-10.0, 0.0, 0.0), 20.0), car(Vec3::new(3.0, 40.0, 0.0), 20.0)];
        assert_eq!(gather_lights(&others, &[behind.clone(), side.clone()], Vec3::ZERO, None, 16).len(), 4);
        assert!(gather_lights(&others, &[behind, side], Vec3::ZERO, Some(cull), 16).is_empty());
        let b = car_boxes(&cars, Vec3::ZERO);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0][0][..3], [10.0, 0.0, 0.62]);
    }
}
