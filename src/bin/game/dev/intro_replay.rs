use crate::*;

/// A `sync` line of an INTRO_FRAMES replay (rep.py): the car set to the
/// original's logged state (centre of mass, rows, velocity, spin, wheel
/// spins, drivetrain, then optionally the suspensions, the wheel hop and
/// the rest class).
pub(crate) fn apply_replay_sync(d: &mut Drive, v: &[f32]) {
    let rows = [Vec3::new(v[3], v[4], v[5]), Vec3::new(v[6], v[7], v[8]), Vec3::new(v[9], v[10], v[11])];
    let rot = glam::Mat3::from_cols(rows[0], rows[1], rows[2]);
    d.state.orientation = glam::Quat::from_mat3(&rot).normalize();
    let com = glam::Mat3::from_quat(d.state.orientation) * Vec3::from(d.car.car_type.handling.centre_of_mass);
    d.state.position = Vec3::new(v[0], v[1], v[2]) - com;
    d.state.velocity = Vec3::new(v[12], v[13], v[14]);
    // Explicit synthetic/replay motion initialization; no native placement offset.
    d.state.publish_forward_speed();
    d.state.spin = Vec3::new(v[15], v[16], v[17]);
    d.state.angular_momentum = Vec3::new(v[24], v[25], v[26]);
    for i in 0..4 {
        d.state.wheels[i].spin = v[18 + i];
    }
    if let Some(t) = d.state.drivetrain.as_mut() {
        t.gear = v[22] as usize;
        t.target_gear = v[27] as usize;
        t.engine_w = v[23];
        t.shift = [
            nfsu2::handling::Shift::Disengaging,
            nfsu2::handling::Shift::Changing,
            nfsu2::handling::Shift::Engaging,
            nfsu2::handling::Shift::Locked,
        ][(v[28] as usize).min(3)];
        t.clutch = v[29];
        t.input_w = v[30];
        t.pending = v[31] != 0.0;
    }
    // The suspensions' compression and rate (+0x1C, +0x20),
    // then the wheel hop (car +0x46C..+0x478), when logged.
    if v.len() >= 40 {
        for i in 0..4 {
            d.state.wheels[i].compression = v[32 + i];
            d.state.wheels[i].compression_rate = v[36 + i];
        }
        // Logged at the end of the original's sub-step: the
        // first sub-step keeps that rate.
        d.state.last_sub = 0.0;
    }
    // The rest test's class (mover +0x60) when logged (field
    // 45); else a car the log shows standing exactly still
    // was held (class 2: no gravity, velocities zeroed).
    d.state.rest = if v.len() >= 45 {
        v[44] as u8
    } else if v[12..18].iter().all(|x| *x == 0.0) {
        2
    } else {
        0
    };
    if v.len() >= 44 {
        d.state.hop = v[40] as i32;
        d.state.hop_timer = v[41];
        d.state.hop_grip = v[42];
        d.state.hop_calm = v[43];
    }
}

/// INTRO_POSE / INTRO_KEYS on the `intro` shot (see there).
pub(crate) fn intro_drive(game: &mut Game) -> Result<()> {
    // INTRO_POSE="x,y[,z],heading°": set the car down there first
    // (at rest, 3 s to settle), to repeat a run measured in the
    // original from the same pose.
    if let Ok(pose) = std::env::var("INTRO_POSE") {
        let v: Vec<f32> = pose.split(',').filter_map(|s| s.parse().ok()).collect();
        // (x,y,z,heading°: at that height, for places off the airport's level.)
        anyhow::ensure!(v.len() == 3 || v.len() == 4, "INTRO_POSE=x,y[,z],heading");
        let (z, h) = if v.len() == 4 { (v[2], v[3]) } else { (30.0, v[2]) };
        let d = game.sim.drive.as_mut().unwrap();
        d.place(Vec3::new(v[0], v[1], z), h.to_radians());
        // The original's car +0x60 (what its logs give) is the centre of
        // mass; +0x50 is the body origin (0x5F5072: the placement's point
        // less the unrotated centre-of-mass offset). Put the clone's centre
        // of mass on the logged point.
        let com = glam::Mat3::from_quat(d.state.orientation) * Vec3::from(d.car.car_type.handling.centre_of_mass);
        d.state.position.x = v[0] - com.x;
        d.state.position.y = v[1] - com.y;
        // The particles too (the marker effects), as a frame does.
        for _ in 0..90 {
            game.advance_for_each(1.0 / 30.0, |g, dt| {
                g.particle_effects(dt, &crate::sim::TickSettings::take(&g.settings.custom));
                Ok(())
            })?;
        }
    }
    // INTRO_KICK="dz,v[,vz]" (at the start of key step INTRO_KICK_STEP,
    // default 0): lift the car by dz m and give it v m/s along its heading
    // (and vz up), no spin, the wheels untouched — the same write the Wine
    // logger makes to the original's rigid body (position +0x20, momentum
    // +0x70, angular momentum +0x80), for drops and top-speed runs.
    let kick = |game: &mut Game| {
        if let Ok(kick) = std::env::var("INTRO_KICK") {
            let v: Vec<f32> = kick.split(',').filter_map(|s| s.parse().ok()).collect();
            let d = game.sim.drive.as_mut().unwrap();
            let f = d.state.forward();
            d.state.position.z += v.first().copied().unwrap_or(0.0);
            d.state.velocity = Vec3::new(f.x, f.y, 0.0).normalize_or_zero() * v.get(1).copied().unwrap_or(0.0)
                + Vec3::Z * v.get(2).copied().unwrap_or(0.0);
            // Explicit synthetic/replay motion initialization; no native placement offset.
            d.state.publish_forward_speed();
            d.state.spin = Vec3::ZERO;
            d.state.angular_momentum = Vec3::ZERO;
        }
    };
    // INTRO_TOP=mph: the rigid body's top speed (+0x204, record +0x3A0) set
    // to that, as the Wine logger pokes it, to measure the limiter.
    if let Some(top) = std::env::var("INTRO_TOP").ok().and_then(|v| v.parse::<f32>().ok()) {
        game.sim.drive.as_mut().unwrap().car.car_type.handling.top_speed_mph = top;
    }
    let kick_step: usize = std::env::var("INTRO_KICK_STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    // INTRO_FRAMES=file: replay a frame-by-frame log of the original
    // (a code-cave logger in the original, lg.py ->
    // rep.py): `f dt keys` runs one frame of dt with those keys held
    // (up+down+left+right+space+nos, or -), `sync ...` sets the car to the
    // original's logged state first (centre of mass, rows, velocity, spin,
    // wheel spins, gear, engine speed). Each frame writes one line to
    // INTRO_FOUT in the logger's columns.
    if let Ok(path) = std::env::var("INTRO_FRAMES") {
        use std::io::Write;
        let mut out = std::fs::File::create(std::env::var("INTRO_FOUT").unwrap_or_else(|_| "/dev/stderr".into()))?;
        let mut k = 0usize;
        for line in std::fs::read_to_string(&path)?.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            match f.first().copied() {
                Some("sync") => {
                    let v: Vec<f32> = f[1..].iter().map(|x| x.parse().unwrap()).collect();
                    apply_replay_sync(game.sim.drive.as_mut().unwrap(), &v);
                }
                Some("sync_forces") => {
                    // Optional diagnostic capture state, separate from the legacy
                    // pose sync: wheel +0x120 (four averaged net torques), engine
                    // +0x80/+0x7C/+0x78/+0x88, drivetrain +0xEC, mover +0x60.
                    // This still does not restore every native simulation field.
                    anyhow::ensure!(f.len() == 11, "sync_forces requires ten values");
                    let v: Vec<f32> = f[1..].iter().map(|x| x.parse()).collect::<std::result::Result<_, _>>()?;
                    anyhow::ensure!(v.iter().all(|x| x.is_finite()), "non-finite sync_forces value");
                    let state = &mut game.sim.drive.as_mut().context("sync_forces without a car")?.state;
                    let t = state.drivetrain.as_mut().context("sync_forces without a drivetrain")?;
                    for (wheel, torque) in state.wheels.iter_mut().zip(&v[..4]) {
                        wheel.net_torque = *torque;
                    }
                    t.power = v[4];
                    t.reaction = v[5];
                    t.throttle = v[6];
                    t.limiter = v[7] != 0.0;
                    t.rear_share = v[8];
                    state.rest = v[9] as u8;
                }
                Some("f") => {
                    let dt: f32 = f[1].parse()?;
                    game.platform.keys.clear();
                    for key in f[2].split('+') {
                        game.platform.keys.insert(match key {
                            "up" => KeyCode::ArrowUp,
                            "down" => KeyCode::ArrowDown,
                            "left" => KeyCode::ArrowLeft,
                            "right" => KeyCode::ArrowRight,
                            "space" => KeyCode::Space,
                            "nos" => KeyCode::AltLeft,
                            _ => continue,
                        });
                    }
                    if f.len() >= 8 {
                        let v: Vec<f32> = f[3..8].iter().map(|x| x.parse().unwrap()).collect();
                        game.dev.replay_controls = Some(Controls {
                            throttle: v[0],
                            brake: v[1],
                            handbrake: v[2],
                            steer: -v[3],
                            nitrous: v[4] != 0.0,
                            ..Default::default()
                        });
                    }
                    // The original's logged frame time (a replay of its
                    // frames, off the clone's cadence by design).
                    super::ticks::off_cadence(|| game.update(dt))?;
                    let d = game.sim.drive.as_ref().unwrap();
                    let s = &d.state;
                    let r = glam::Mat3::from_quat(s.orientation);
                    let cm = s.position + r * Vec3::from(d.car.car_type.handling.centre_of_mass);
                    let w = &s.wheels;
                    let t = s.drivetrain.as_ref();
                    writeln!(
                        out,
                        "{k} pos {:.4} {:.4} {:.4} V {:.4} {:.4} {:.4} w {:.4} {:.4} {:.4} fwd {:.5} {:.5} {:.5} up {:.5} {:.5} {:.5} gear {} sh {} pend {} rpm {:.1} ew {:.3} clutch {:.3} steer {:.3} down {} spin {:.2} {:.2} {:.2} {:.2} slip {:.3} {:.3} {:.3} {:.3} load {:.3} {:.3} {:.3} {:.3} comp {:.4} {:.4} {:.4} {:.4} nos {} wall {} T {:.3} {:.3} {:.3} {:.3} fx {:.3} {:.3} {:.3} {:.3} lk {}{}{}{} recs {} {} share {:.2} near {:.4} thr {:.3} pow {:.3} hop {} L {:.4} {:.4} {:.4} fy {:.3} {:.3} {:.3} {:.3} sl {}{}{}{} vy {:.3} {:.3} {:.3} {:.3} circ {:.3} {:.3} {:.3} {:.3}",
                        cm.x,
                        cm.y,
                        cm.z,
                        s.velocity.x,
                        s.velocity.y,
                        s.velocity.z,
                        s.spin.x,
                        s.spin.y,
                        s.spin.z,
                        r.x_axis.x,
                        r.x_axis.y,
                        r.x_axis.z,
                        r.z_axis.x,
                        r.z_axis.y,
                        r.z_axis.z,
                        t.map_or(1, |t| t.gear),
                        t.map_or(9, |t| t.shift as u8),
                        t.map_or(0, |t| t.pending as u8),
                        s.rpm(),
                        t.map_or(0.0, |t| t.engine_w),
                        t.map_or(0.0, |t| t.clutch),
                        s.steer_degrees,
                        s.wheels_down(),
                        w[0].spin,
                        w[1].spin,
                        w[2].spin,
                        w[3].spin,
                        w[0].slip,
                        w[1].slip,
                        w[2].slip,
                        w[3].slip,
                        w[0].load,
                        w[1].load,
                        w[2].load,
                        w[3].load,
                        w[0].compression,
                        w[1].compression,
                        w[2].compression,
                        w[3].compression,
                        t.map_or(0, |t| t.nitrous),
                        s.wall_contact.is_some() as u8,
                        w[0].torque,
                        w[1].torque,
                        w[2].torque,
                        w[3].torque,
                        w[0].fx,
                        w[1].fx,
                        w[2].fx,
                        w[3].fx,
                        w[0].locked as u8,
                        w[1].locked as u8,
                        w[2].locked as u8,
                        w[3].locked as u8,
                        s.records.len(),
                        s.records.values().map(|r| format!("{:.2}", r.age)).collect::<Vec<_>>().join(","),
                        t.map_or(0.0, |t| t.rear_share),
                        s.pairs
                            .values()
                            .filter_map(|c| c.as_ref().and_then(|c| c.last_distance()))
                            .fold(9.0f32, f32::min),
                        t.map_or(0.0, |t| t.throttle),
                        t.map_or(0.0, |t| t.power),
                        s.hop,
                        s.angular_momentum.x,
                        s.angular_momentum.y,
                        s.angular_momentum.z,
                        w[0].fy,
                        w[1].fy,
                        w[2].fy,
                        w[3].fy,
                        w[0].sliding as u8,
                        w[1].sliding as u8,
                        w[2].sliding as u8,
                        w[3].sliding as u8,
                        w[0].lateral_slip,
                        w[1].lateral_slip,
                        w[2].lateral_slip,
                        w[3].lateral_slip,
                        w[0].circle,
                        w[1].circle,
                        w[2].circle,
                        w[3].circle,
                    )?;
                    k += 1;
                }
                _ => {}
            }
        }
        game.platform.keys.clear();
        return Ok(());
    }
    // INTRO_KEYS="up:2,down+left:1.5,...": hold those keys for
    // so many seconds each and log the car every 0.1 s.
    if let Ok(script) = std::env::var("INTRO_KEYS") {
        if std::env::var_os("INTRO_WHEELS").is_some() {
            let d = game.sim.drive.as_ref().unwrap();
            let h = &d.car.car_type.handling;
            eprintln!("hull {:.3?}", d.car.hull);
            eprintln!(
                "engine idle {} redline {} max {} shift {:?}",
                h.engine.idle_rpm,
                h.engine.redline_rpm,
                h.engine.max_rpm,
                nfsu2::handling::shift_points(h)
            );
            eprintln!(
                "car mass {} drag {} downforce {:?} preload {} inertia {:?} ground yaw x {} rear share {} ratios {:?} final {} {}",
                h.mass,
                h.drag,
                h.downforce,
                h.preload_downforce,
                h.inertia,
                h.unknown_270,
                h.gearbox.rear_split,
                h.gearbox.ratios,
                h.gearbox.final_drive_front,
                h.gearbox.final_drive_rear
            );
        }
        let mut t0 = 0.0f32;
        for (si, step) in script.split(',').enumerate() {
            if si == kick_step {
                kick(game);
            }
            let (keys, secs) = step.split_once(':').unwrap_or((step, "1"));
            game.platform.keys.clear();
            for k in keys.split('+') {
                game.platform.keys.insert(match k {
                    "up" => KeyCode::ArrowUp,
                    "down" => KeyCode::ArrowDown,
                    "left" => KeyCode::ArrowLeft,
                    "right" => KeyCode::ArrowRight,
                    "space" => KeyCode::Space,
                    "nos" => KeyCode::AltLeft,
                    _ => continue,
                });
            }
            let hz: f32 = std::env::var("INTRO_HZ").ok().and_then(|v| v.parse().ok()).unwrap_or(30.0);
            let n = (secs.parse::<f32>().unwrap_or(1.0) * hz) as usize;
            for i in 0..n {
                game.advance_for(1.0 / hz)?;
                if i % (hz as usize / std::env::var("INTRO_LOG_HZ").ok().and_then(|v| v.parse().ok()).unwrap_or(10))
                    .max(1)
                    == 0
                {
                    let d = game.sim.drive.as_ref().unwrap();
                    let f = d.state.forward();
                    // The centre of mass, as the original's car +0x60.
                    let cm = d.state.position
                        + glam::Mat3::from_quat(d.state.orientation)
                            * Vec3::from(d.car.car_type.handling.centre_of_mass);
                    eprintln!(
                        "{keys} t {:.2} pos {:.2} {:.2} {:.2} head {:.1} speed {:.2} rpm {:.0} gear {:?} wall {:?} contacts {} recs {} upright {} wb {:.3?} fz {:.4} vz {:.3} down {} ld {:.2} {:.2} {:.2} {:.2}",
                        t0 + i as f32 / hz,
                        cm.x,
                        cm.y,
                        cm.z,
                        f.y.atan2(f.x).to_degrees(),
                        d.state.velocity.length(),
                        d.state.rpm(),
                        d.state.drivetrain.as_ref().map(|t| t.gear),
                        d.state.wall_contact,
                        d.state.contacts.len(),
                        d.state.records.len(),
                        d.state.records.values().filter(|r| r.upright).count(),
                        glam::Mat3::from_quat(d.state.orientation).transpose() * d.state.spin,
                        f.z,
                        d.state.velocity.z,
                        d.state.wheels_down(),
                        d.state.wheels[0].load,
                        d.state.wheels[1].load,
                        d.state.wheels[2].load,
                        d.state.wheels[3].load
                    );
                    // INTRO_GPS: the GPS follower, the aid's point and its
                    // side in the car's frame (+ left), and the arrow's tip
                    // (its model −z row) across the view (+ right).
                    if std::env::var_os("INTRO_GPS").is_some() {
                        if let Some(g) = game.career_world.gps.as_ref() {
                            let side =
                                g.aid.map(|a| (a - cm).truncate().perp_dot(f.truncate().normalize_or_zero()) * -1.0);
                            let tip = gps_aid_transform(&d.camera, game.gps_aid_target()).0;
                            let axes = CameraAxes::look(d.camera.forward(), d.camera.up);
                            let across = (-tip.z_axis.truncate()).dot(axes.right);
                            eprintln!(
                                "  gps state {} follower {} aid {:.1?} side {:.1?} target {:?} tip across {:.3} cam {:.2?}",
                                g.state(),
                                g.follower.state,
                                g.aid,
                                side,
                                game.gps_aid_target().is_some(),
                                across,
                                d.camera.forward()
                            );
                        }
                    }
                    if std::env::var_os("INTRO_SUB").is_some() {
                        let w = &d.state.wheels;
                        eprintln!(
                            "  sub slp {:.2} {:.2} {:.2} {:.2} fx {:.2} {:.2} {:.2} {:.2} spin {:.1} {:.1} {:.1} {:.1} x {:.3} {:.3} {:.3} {:.3}",
                            w[0].slip,
                            w[1].slip,
                            w[2].slip,
                            w[3].slip,
                            w[0].fx,
                            w[1].fx,
                            w[2].fx,
                            w[3].fx,
                            w[0].spin,
                            w[1].spin,
                            w[2].spin,
                            w[3].spin,
                            w[0].compression,
                            w[1].compression,
                            w[2].compression,
                            w[3].compression
                        );
                    }
                    if std::env::var_os("INTRO_PAIRS").is_some() {
                        let mut near: Vec<_> = d
                            .state
                            .pairs
                            .iter()
                            .filter_map(|(k, c)| {
                                c.as_ref().and_then(|c| c.last_distance()).filter(|x| *x < 0.2).map(|x| (*k, x))
                            })
                            .collect();
                        near.sort_by_key(|p| p.0);
                        eprintln!("  pairs {near:.5?}");
                    }
                    if std::env::var_os("INTRO_WHEELS").is_some() {
                        let t = d.state.drivetrain.as_ref().unwrap();
                        let w = &d.state.wheels;
                        eprintln!(
                            "  eng {:.1} in {:.1} clutch {:.3} shift {:?} | rear spin {:.1} {:.1} slip {:.3} {:.3} sliding {} {} load {:.2} {:.2}",
                            t.engine_w,
                            t.input_w,
                            t.clutch,
                            t.shift,
                            w[2].spin,
                            w[3].spin,
                            w[2].slip,
                            w[3].slip,
                            w[2].sliding,
                            w[3].sliding,
                            w[2].load,
                            w[3].load
                        );
                        eprintln!(
                            "  T {:.3} {:.3} {:.3} {:.3} fx {:.2} {:.2} fy {:.2} {:.2} circle {:.2} {:.2} front load {:.2} {:.2} fx {:.2} {:.2} fy {:.2} {:.2} spin {:.1} {:.1} steer {:.4}",
                            w[2].torque,
                            w[3].torque,
                            w[0].torque,
                            w[1].torque,
                            w[2].fx,
                            w[3].fx,
                            w[2].fy,
                            w[3].fy,
                            w[2].circle,
                            w[3].circle,
                            w[0].load,
                            w[1].load,
                            w[0].fx,
                            w[1].fx,
                            w[0].fy,
                            w[1].fy,
                            w[0].spin,
                            w[1].spin,
                            d.state.steer_degrees.to_radians()
                        );
                    }
                }
            }
            t0 += n as f32 / hz;
        }
        game.platform.keys.clear();
    }
    Ok(())
}

/// Types cheat codes as at the title screen: a comma list, or "all"
/// (shots; the game's table `cheats::ACTIVE`).
pub(crate) fn type_cheats(codes: &str) {
    let mut input = nfsu2::frontend::cheats::Input::default();
    for code in
        nfsu2::frontend::cheats::CODES.iter().filter(|c| codes == "all" || codes.split(',').any(|x| x.trim() == c.text))
    {
        code.text.chars().for_each(|c| drop(input.char(&nfsu2::frontend::cheats::ACTIVE, c)));
    }
}
