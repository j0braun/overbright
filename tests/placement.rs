//! The car's placement (0x5F5020): the rigid body keeps the point as its
//! centre of mass at any heading. Under Wine (2026-10-31) a resume with the
//! kept place written to heading 90° left car +0x60 on the point exactly
//! once the body ran; the clone had placed the body origin at the point
//! less the *unrotated* offset, so its centre of mass was 0.19–0.38 m off.
mod common;

use glam::{Mat3, Vec3};

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn placement_puts_the_centre_of_mass_on_the_point_at_any_heading() {
    let root = common::game_root();
    let root = std::path::Path::new(&root);
    let world = nfsu2::world::World::load(root, "L4RA").unwrap();
    let car = nfsu2::car::Car::load(root, "350Z").unwrap();
    let com = Vec3::from(car.car_type.handling.centre_of_mass);
    assert!(com.x.abs() > 0.1, "the 350Z's offset is not trivial: {com:?}");
    // The airport road (the kept place of the Wine run).
    for deg in [0.0f32, 90.0, -10.9, 180.0] {
        let at = Vec3::new(1695.25, -883.64, 24.11);
        let s = nfsu2::drive::CarState::placed(&car.car_type, &world, at, deg.to_radians());
        let centre = s.position + Mat3::from_quat(s.orientation) * com;
        assert!((centre.truncate() - at.truncate()).length() < 1e-3, "heading {deg}: centre {centre:?}");
    }
}
