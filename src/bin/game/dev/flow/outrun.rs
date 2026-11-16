use crate::*;

/// One encounter car's row for the outrun logs (docs/outrun.md, "Live
/// comparison"): the same fields the native logger reads (goal type 9 / 6
/// / 5, action type, mover 1/3/4, target speed, nitrous timers).
pub(crate) fn outrun_log_row(game: &Game, frame: u32) -> Option<String> {
    let e = game.sim.encounters.as_ref()?;
    let d = game.sim.drive.as_ref()?;
    let o = e.outrun.as_ref();
    let i = o.map(|o| o.opponent).or(e.target).or_else(|| e.cars.iter().position(|c| c.active))?;
    let r = &e.cars[i];
    let goal = match (o.is_some_and(|o| o.opponent == i), r.controller.goal) {
        (true, Some(true)) => 5,
        (true, _) => 6,
        _ => 9,
    };
    let action =
        if r.unstuck.since.is_some() { 9 } else { r.controller.actions.as_ref().map_or(-1, |a| a.kind.type_id()) };
    let nos = r.state.nitrous.as_ref().map_or(-1.0, |t| t.running.unwrap_or(-1.0));
    let p = d.state.position;
    let q = r.state.position;
    Some(format!("{{\"name\":\"{}\",\"frame\":{frame},\"tick\":{},\"race\":{},\"mgr\":{},\"career\":{},\"car\":{i},\"goal\":{goal},\"act\":{action},\"mover\":{},\"susp\":{},\"v\":{:.3},\"tgt\":{:.3},\"pv\":{:.3},\"d\":{:.2},\"nos\":[{},{},{}],\"nos_run\":{nos:.3},\"t73\":[{},{}],\"pos\":[{:.2},{:.2},{:.2}],\"ppos\":[{:.2},{:.2}],\"leads\":{},\"gap\":{:.2},\"wd\":{},\"gear\":{},\"left\":{:.2}}}",
        r.car.car_type.name, d.world_clock.tick, if o.is_some() { 7 } else { 0 }, e.state, e.career, encounter_mover_kind(r), u8::from(r.controller.suspended),
        r.state.velocity.truncate().length(), r.controller.target_speed.max(r.driver_plan.1), d.state.velocity.truncate().length(),
        q.truncate().distance(p.truncate()), r.controller.nitrous.started, r.controller.nitrous.ended, r.controller.nitrous.flat_since,
        r.belt_block_ticks, r.belt_hold, q.x, q.y, q.z, p.x, p.y, o.is_some_and(|o| o.player_leads), o.map_or(0.0, |o| o.player.max(o.rival)),
        r.state.wheels_down(), r.state.melo.as_ref().map_or_else(|| r.state.drivetrain.as_ref().map_or(-1, |d| d.gear as i32), |m| m.gear as i32),
        r.state.nitrous.as_ref().map_or(-1.0, |t| t.remaining())))
    .map(|row| {
        // Contacts (check aid): the rival's last wall contact, its
        // collision partners (0 the player, 1.. encounter cars, 1000+
        // traffic) and the nearest traffic car.
        let field = game.sim.traffic.as_ref().map_or(&e.field, |t| &t.field);
        let partners = field.partners(i as u64 + 1);
        let near = game.sim.traffic.as_ref().map_or(f32::INFINITY, |t| t.cars.iter().filter(|c| c.lane.is_some()).map(|c| c.state.position.truncate().distance(q.truncate())).fold(f32::INFINITY, f32::min));
        let wall = r.state.wall_contact.map_or("null".to_string(), |w| format!("[{:.2},{:.2}]", w.x, w.y));
        let up = r.route.as_ref().map_or(String::new(), |rt| rt.upcoming(6).iter().map(|(a, b, s, v)| format!("[{a},{b},{s:.0},{v:.1}]")).collect::<Vec<_>>().join(","));
        let rs = r.route.as_ref().map_or(-1.0, |rt| rt.speed_at(rt.distance()));
        let plan = r.driver_plan.0.iter().take(8).map(|p| format!("[{:.1},{:.1}]", p.x, p.y)).collect::<Vec<_>>().join(",");
        let fwd = r.state.forward();
        let ctl = format!("[{:.3},{:.3},{:.3},{:.2},{:.2}]", r.controls.steer_degrees, r.controls.gas, r.controls.brake, fwd.x, fwd.y);
        format!("{},\"plan\":[{plan}],\"ctl\":{ctl},\"wall\":{wall},\"partners\":{partners:?},\"near\":{:.1},\"rs\":{rs:.2},\"up\":[{up}]}}", &row[..row.len() - 1], near.min(9999.0))
    })
}

/// The live comparison's keyboard driver (docs/outrun.md; check aid, not
/// native), the rule of `docs/evidence/outrun-live/bot.py`: look 12 m +
/// 0.6 s of speed along the path; steer while the target is over 0.03 rad
/// off the car's forward axis; throttle unless faster than √(8 m/s² · R) +
/// 2 for the tightest radius R in the next 100 m (headings over 10 m
/// chords), else brake; brake at the path's end.
struct OutrunBot {
    path: Vec<Vec3>,
    cum: Vec<f32>,
    i: usize,
}

struct BotKeys {
    left: bool,
    right: bool,
    gas: bool,
    brake: bool,
}

impl OutrunBot {
    fn new(path: Vec<Vec3>) -> OutrunBot {
        let mut cum = vec![0.0];
        for w in path.windows(2) {
            cum.push(cum.last().unwrap() + w[0].truncate().distance(w[1].truncate()));
        }
        OutrunBot { path, cum, i: 0 }
    }
    fn ahead(&self, mut n: usize, d: f32) -> usize {
        let s0 = self.cum[n];
        while n + 1 < self.path.len() && self.cum[n] - s0 < d {
            n += 1;
        }
        n
    }
    fn radius(&self) -> f32 {
        let (p, mut r, mut m) = (&self.path, 1e9f32, self.i);
        let wrap = |a: f32| (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        while m < p.len() && self.cum[m] - self.cum[self.i] < 100.0 {
            let b = self.ahead(m, 10.0);
            let c = self.ahead(b, 10.0);
            if c > b && b > m {
                let h1 = (p[b].y - p[m].y).atan2(p[b].x - p[m].x);
                let h2 = (p[c].y - p[b].y).atan2(p[c].x - p[b].x);
                let dh = wrap(h2 - h1).abs();
                if dh > 1e-3 {
                    r = r.min((self.cum[c] - self.cum[b]) / dh);
                }
            }
            m += 1;
        }
        r
    }
    fn keys(&mut self, pos: Vec3, vel: Vec3, fwd: Vec3) -> BotKeys {
        let p = &self.path;
        let d2 = |q: Vec3| q.truncate().distance(pos.truncate());
        while self.i + 1 < p.len() && d2(p[self.i + 1]) <= d2(p[self.i]) {
            self.i += 1;
        }
        let v = vel.truncate().length();
        let j = self.ahead(self.i, 12.0 + 0.6 * v);
        let t = (p[j] - pos).truncate();
        let err = (fwd.x * t.y - fwd.y * t.x).atan2(fwd.x * t.x + fwd.y * t.y);
        let mut k = BotKeys { left: err > 0.03, right: err < -0.03, gas: false, brake: false };
        if j + 1 >= p.len() || v > (8.0 * self.radius()).sqrt() + 2.0 {
            k.brake = true;
        } else {
            k.gas = true;
        }
        k
    }
}

/// `--flow outrun`: an outrun against the original's (docs/outrun.md, Live
/// comparison). The finished-career save resumed to free roam; once an encounter car
/// is out, the player is put `NFSU2_FLOW_BACK` (default 10) m behind it on
/// its travel at its speed with the throttle held (the native capture's
/// intervention, `engage.py`), the Enter Race key taken when the career
/// offers it (+0x1AE8 = 2); then the player brakes (`NFSU2_FLOW_OUTRUN=loss`,
/// handbrake held) or leaves the car standing. Each frame's row goes to
/// `NFSU2_FLOW_LOG` (JSON lines) until a second after the end.
pub(crate) fn flow_outrun(game: &mut Game) -> Result<()> {
    use std::io::Write;
    let dt = 1.0 / 60.0;
    // NFSU2_FLOW_SAVE: another native save file to resume (check aid: the
    // finished-career save for docs/outrun-ai-strength.md).
    let bytes = match std::env::var("NFSU2_FLOW_SAVE") {
        Ok(path) => std::fs::read(path)?,
        Err(_) => crate::dev::private::require(crate::dev::private::FINISHED_CAREER, "--flow outrun (career_complete.sav)")?,
    };
    let save = nfsu2::formats::native_save::NativeSave::parse(&bytes)?;
    let name = crate::dev::private::profile_name(&save);
    profiles::write(&name, &save)?;
    flow_resume_to_explore(game, name)?;
    let mut log = std::env::var("NFSU2_FLOW_LOG").ok().map(std::fs::File::create).transpose()?;
    let back: f32 = std::env::var("NFSU2_FLOW_BACK").ok().and_then(|v| v.parse().ok()).unwrap_or(10.0);
    let mut frame = 0u32;
    // One tick per step, drawn frames on the rate run's schedule
    // (else 1/60 s), the flow acting between ticks.
    let mut frames = super::super::ticks::Frames::new(60.0);
    let mut step = |game: &mut Game, frame: &mut u32| -> Result<()> {
        frames.tick(game)?;
        *frame += 1;
        if let (Some(f), Some(row)) = (log.as_mut(), outrun_log_row(game, *frame)) {
            writeln!(f, "{row}")?;
        }
        Ok(())
    };
    // An encounter car out and driving for 3 s, at least `NFSU2_FLOW_WAIT`
    // (default 40) s into free roam: the native captures engaged minutes
    // after the load, past the nitrous gap the load's tick reset imposes.
    let wait: f32 = std::env::var("NFSU2_FLOW_WAIT").ok().and_then(|v| v.parse().ok()).unwrap_or(40.0);
    let mut out = None;
    for _ in 0..60 * 120 {
        step(game, &mut frame)?;
        if let Some(i) = game
            .sim.encounters
            .as_ref()
            .filter(|e| e.state == 3 && e.clock > wait)
            .and_then(|e| e.cars.iter().position(|c| c.active && c.placed.is_some_and(|t| e.clock - t > 3.0)))
        {
            out = Some(i);
            break;
        }
    }
    let i = out.context("no encounter car out")?;
    let place = |game: &mut Game, back: f32| {
        let e = game.sim.encounters.as_ref().unwrap();
        let r = &e.cars[i].state;
        let v = r.velocity.truncate();
        let dir = v.normalize_or(r.forward().truncate()).extend(0.0);
        let at = r.position - dir * back;
        let speed = v.length();
        let d = game.sim.drive.as_mut().unwrap();
        let ground = d.world.road_height_below(at + Vec3::Z * 3.0).unwrap_or(at.z);
        d.state = CarState::placed(&d.car.car_type, &d.world, Vec3::new(at.x, at.y, ground), dir.y.atan2(dir.x));
        d.state.velocity = dir * speed;
        // Explicit synthetic/replay motion initialization (the native capture's momentum write).
        d.state.publish_forward_speed();
    };
    // NFSU2_FLOW_POSE (a text file of 12 numbers from a native challenge
    // row: the rival's position and velocity, then the player's): both cars
    // put there first, so the two games race on the same road from the
    // same start.
    let floats = |path: String| -> Result<Vec<f32>> {
        Ok(std::fs::read_to_string(path)?
            .split_whitespace()
            .map(|v| v.parse::<f32>())
            .collect::<Result<Vec<_>, _>>()?)
    };
    let pose: Option<Vec<f32>> = std::env::var("NFSU2_FLOW_POSE").ok().map(floats).transpose()?;
    let vec3 = |v: &[f32]| Vec3::new(v[0], v[1], v[2]);
    let put = |world: &nfsu2::world::World, car: &Car, at: Vec3, vel: Vec3| {
        let ground = world.road_height_below(at + Vec3::Z * 3.0).unwrap_or(at.z);
        let mut s = CarState::placed(&car.car_type, world, Vec3::new(at.x, at.y, ground), vel.y.atan2(vel.x));
        s.velocity = Vec3::new(vel.x, vel.y, 0.0);
        // Explicit synthetic/replay motion initialization (the native capture's momentum write).
        s.publish_forward_speed();
        s
    };
    if let Some(pose) = pose.as_ref() {
        let d = game.sim.drive.as_mut().unwrap();
        d.state = put(&d.world, &d.car, vec3(&pose[6..9]), vec3(&pose[9..12]));
        let e = game.sim.encounters.as_mut().unwrap();
        let r = &mut e.cars[i];
        let tank = r.state.nitrous;
        r.state = put(&d.world, &r.car, vec3(&pose[0..3]), vec3(&pose[3..6]));
        r.state.nitrous = tank;
        // A 13th number: the rival's tank left, ticks (native car +0x45C).
        if let (Some(&left), Some(t)) = (pose.get(12), r.state.nitrous.as_mut()) {
            t.left = left as i32;
        }
        // Rolling wheels, as the native car arrived on the MeloMvr (the
        // capture's rival had been on it for seconds; a fresh state's
        // spins are 0 and would brake it for two frames).
        let speed = r.state.velocity.length();
        for (w, t) in r.state.wheels.iter_mut().zip(&r.car.car_type.wheels) {
            w.spin = speed / t.radius;
        }
        r.state.melo = Some(nfsu2::melo::Melo::new(&r.car, &r.state, r.belt_rating));
        r.belt = Default::default();
        r.route =
            nfsu2::ai::route::RacerRoute::start(&e.routes, r.state.position, r.state.velocity.truncate().normalize());
        r.driver_plan = Default::default();
        e.road_prev.clear();
    }
    game.hold_next(KeyCode::ArrowUp, true);
    let mut engaged = false;
    'tries: for n in 0..6 {
        if pose.is_none() || n > 0 {
            place(game, back);
        }
        for _ in 0..36 {
            step(game, &mut frame)?;
            if game.sim.encounters.as_ref().is_some_and(|e| e.career == 2) {
                game.sim.encounters.as_mut().unwrap().engage();
                engaged = true;
                break 'tries;
            }
        }
    }
    anyhow::ensure!(engaged, "the career never offered the challenge");
    {
        // Check aid: the rival's class (its upgraded car's and the stock
        // type's) and its racing-line bytes at record 1095 (control
        // points 18; docs/outrun-ai-strength.md).
        let e = game.sim.encounters.as_ref().unwrap();
        let r = &e.cars[i];
        println!(
            "rival {} class {} (stock {}) record 1095 cp 18 line bytes {:?}",
            r.car.car_type.name,
            r.car.car_type.handling.performance_class,
            game.game_data.car_types.iter().find(|t| t.name == r.car.car_type.name).map_or(9, |t| t.handling.performance_class),
            e.routes.file.record(1095).map(|k| e.routes.file.records[k].points[18].lane_speed[..2].to_vec())
        );
    }
    // 0.4 s with the throttle held, as the native script, then the scenario.
    for _ in 0..24 {
        step(game, &mut frame)?;
    }
    let scenario = std::env::var("NFSU2_FLOW_OUTRUN").unwrap_or_default();
    let mut bot = (scenario == "bot")
        .then(|| -> Result<OutrunBot> {
            // NFSU2_FLOW_PATH: x y z per point (the native run's road path).
            let v = floats(std::env::var("NFSU2_FLOW_PATH").context("bot needs NFSU2_FLOW_PATH")?)?;
            Ok(OutrunBot::new(v.chunks_exact(3).map(vec3).collect()))
        })
        .transpose()?;
    let lead_at: f32 = std::env::var("NFSU2_FLOW_LEAD_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let lead_m: f32 = std::env::var("NFSU2_FLOW_LEAD_M").ok().and_then(|v| v.parse().ok()).unwrap_or(40.0);
    // NFSU2_FLOW_LEAD_EVERY: the lead repeated every that many seconds;
    // NFSU2_FLOW_END_AT: from that race clock no more leads and the bot
    // brakes (scen2.py does the same in the original).
    let lead_every: f32 = std::env::var("NFSU2_FLOW_LEAD_EVERY").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let end_at: f32 = std::env::var("NFSU2_FLOW_END_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(f32::INFINITY);
    let mut next_lead = if lead_at > 0.0 { lead_at } else { f32::INFINITY };
    if bot.is_none() {
        game.hold_next(KeyCode::ArrowUp, false);
        if scenario != "coast" {
            game.hold_next(KeyCode::Space, true);
        }
    }
    let mut ended = None;
    for n in 0..60 * 150 {
        let clock = game.sim.encounters.as_ref().and_then(|e| e.outrun.as_ref()).map(|o| o.clock);
        if let (Some(b), Some(clock)) = (bot.as_mut(), clock) {
            if clock >= next_lead && clock < end_at {
                // As scen.py: the player put on the rival's driver path,
                // the first point `lead_m` or more ahead (or its last), on
                // that leg's heading at the rival's speed; then a new road
                // path from there.
                next_lead = if lead_every > 0.0 { next_lead + lead_every } else { f32::INFINITY };
                let e = game.sim.encounters.as_ref().unwrap();
                let r = &e.cars[e.outrun.as_ref().unwrap().opponent];
                let pts = &r.driver_plan.0;
                let rp = r.state.position;
                let speed = r.state.velocity.truncate().length();
                if let Some(k) =
                    pts.iter().position(|q| q.truncate().distance(rp.truncate()) >= lead_m).or(pts.len().checked_sub(1))
                {
                    let q = pts[k];
                    let q0 = if k > 0 { pts[k - 1] } else { rp };
                    let h = (q - q0).truncate().normalize_or(Vec2::X);
                    let path = nfsu2::outrun::road_path(&e.routes.file, q, h, 3000.0, 2);
                    let d = game.sim.drive.as_mut().unwrap();
                    d.state = put(&d.world, &d.car, q, h.extend(0.0) * speed);
                    *b = OutrunBot::new(path);
                    println!("lead placed at {clock:.2}: {q:?}");
                }
            }
            let d = game.sim.drive.as_ref().unwrap();
            let mut want = b.keys(d.state.position, d.state.velocity, d.state.forward());
            if clock >= end_at {
                want = BotKeys { left: false, right: false, gas: false, brake: true };
            }
            for (k, on) in [
                (KeyCode::ArrowLeft, want.left),
                (KeyCode::ArrowRight, want.right),
                (KeyCode::ArrowUp, want.gas),
                (KeyCode::ArrowDown, want.brake),
            ] {
                game.hold_next(k, on);
            }
        }
        step(game, &mut frame)?;
        let e = game.sim.encounters.as_ref().context("encounters")?;
        // Check aid: NFSU2_FLOW_DUMP_AT=seconds dumps the rival's route list
        // and line once at that race clock.
        if let (Some(o), Some(at)) =
            (e.outrun.as_ref(), std::env::var("NFSU2_FLOW_DUMP_AT").ok().and_then(|v| v.parse::<f32>().ok()))
            && o.clock >= at
            && o.clock - at < dt
            && let Some(rt) = e.cars[o.opponent].route.as_ref()
        {
            println!(
                "dump at {:.2}: car {:?}\n{}",
                o.clock,
                e.cars[o.opponent].state.position,
                rt.debug_lists(&e.routes.file)
            );
        }
        if ended.is_none() && e.outrun.is_none() {
            ended = Some(n);
            println!("outrun over: {:?}", e.result.map(|r| (r.0, r.2)));
        }
        if ended.is_some_and(|m| n > m + 60) {
            break;
        }
    }
    game.platform.keys.clear();
    anyhow::ensure!(ended.is_some(), "the outrun did not end in 150 s");
    Ok(())
}
