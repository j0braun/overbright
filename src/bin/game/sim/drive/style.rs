use crate::*;

/// What the style detectors read of the player's car (see
/// [`nfsu2::style::Detect`]); `tank_before`: the tank before the frame (a
/// shot running), so a shot that started in the frame is the player's
/// nitrous event (0x6034E6); `others`: the other cars' positions and
/// velocities for the draft. The race's part is the caller's.
pub(crate) fn style_detect(
    d: &mut Drive,
    others: &[(Vec3, Vec3)],
    tank_before: Option<bool>,
    dt: f32,
) -> nfsu2::style::Detect {
    let state = &mut d.state;
    let started = tank_before == Some(false) && state.nitrous.is_some_and(|t| t.running.is_some());
    let h = &d.car.car_type.handling;
    let up = glam::Mat3::from_quat(state.orientation) * Vec3::Z;
    // 0x56F10D: car +0x68 less the centre of mass's z less the ground
    // under the car (0x5E7E80; a copy of the car's ground-query state, so
    // prov: inferred game.style_detect
    // the physics' own is not stirred). **Inferred**: car +0x60 is the
    // clone's car-space origin.
    let mut query = state.ground.clone();
    let height = d
        .world
        .ground_for(&mut query, state.position)
        .map_or(0.0, |(z, _, _)| state.position.z - h.centre_of_mass[2] - z);
    let wheel_slip = driven_wheel_slip(&state.wheels, h.gearbox.rear_split);
    nfsu2::style::Detect {
        forward_speed: state.speed(),
        speed: state.velocity.length(),
        shot_started: started,
        gas: state.gas(),
        // The nitrous key reads the previous world update.
        key: Some(state.nitrous_key),
        wheels_down: state.wheels_down(),
        up_z: up.z,
        touching: !state.contacts.is_empty(),
        height,
        // 0x59FA20: none unless going forward faster than 1 m/s.
        slip: if state.speed() > 1.0 { state.slip_degrees().to_radians() } else { 0.0 },
        yaw_rate: state.spin.z,
        heading: state.heading().to_degrees().rem_euclid(360.0),
        draft: nfsu2::style::draft(state.position, others.iter().copied()),
        ghosted: state.ghost_timer > 0.0,
        wheel_slip,
        dt,
        hits: std::mem::take(&mut state.style_hits),
        near: state.near.clone(),
        hydro_kicks: state.hydraulics.kicked.iter().filter(|&&k| k).count(),
        hydro_moving: state.hydraulics.moving(),
        bounce_input: state.hydraulics.input.bouncing(),
        ..Default::default()
    }
}

/// The burnout detector's wheel slip (0x5F7753..0x5F77B1): the mean of the
/// driven wheels' slip as the car exports it (car +0x3B4 + 8i, read from
/// 0x5F7755; the wheel's +0x100, 0 while the tyre grips), summed as it
/// comes; the driven wheels are 0x592880's: the front pair under a rear
/// share of 0.2, the rear pair over 0.8, else all four.
pub(crate) fn driven_wheel_slip(wheels: &[nfsu2::drive::WheelState; 4], split: f32) -> f32 {
    let driven: &[usize] = if split < 0.2 {
        &[0, 1]
    } else if split <= 0.8 {
        &[0, 1, 2, 3]
    } else {
        &[2, 3]
    };
    driven.iter().map(|&i| wheels[i].exported_slip).sum::<f32>() / driven.len() as f32
}

/// Race status 4 in free roam (0x890118 → +0), which HANG TIME needs
/// (0x56F084): **observed** 4 under Wine in career free roam.
pub(crate) const ROAM_RACING: bool = true;

/// The player's free-roam style points (0x6016B0 runs for the free-roam
/// racer too; the Race Breaker flag reads 1 there, observed): detectors,
/// update, refill. Free roam is race type 0 and the racer never done.
pub(crate) fn roam_style_frame(
    roam: &mut (nfsu2::drift::StyleManager, f32, f32, nfsu2::style::Misc),
    d: &mut Drive,
    others: &[(Vec3, Vec3)],
    shortcut: Option<usize>,
    tank_before: Option<bool>,
    dt: f32,
) {
    let (style, base, clock, misc) = roam;
    *clock += dt;
    let detect = nfsu2::style::Detect { racing: ROAM_RACING, shortcut, ..style_detect(d, others, tank_before, dt) };
    nfsu2::style::detect(misc, style, &detect, *clock);
    style.update(dt, *clock);
    nfsu2::style::refill(style, base, d.state.nitrous.as_mut());
}

#[cfg(test)]
mod slip_tests {
    use super::driven_wheel_slip;

    /// The burnout detector reads the driven wheels' exported slip (car
    /// +0x3B4 + 8i): a gripping tyre's own slip does not count.
    #[test]
    fn burnout_reads_the_driven_wheels_exported_slip() {
        let mut wheels = [nfsu2::drive::WheelState::default(); 4];
        for w in &mut wheels {
            w.slip = 0.4;
        }
        assert_eq!(driven_wheel_slip(&wheels, 1.0), 0.0);
        wheels[0].exported_slip = 8.0;
        wheels[2].exported_slip = 2.0;
        wheels[3].exported_slip = 4.0;
        // Rear drive: the rear pair; front drive: the front pair; else all four.
        assert_eq!(driven_wheel_slip(&wheels, 1.0), 3.0);
        assert_eq!(driven_wheel_slip(&wheels, 0.0), 4.0);
        assert_eq!(driven_wheel_slip(&wheels, 0.5), 3.5);
    }
}
