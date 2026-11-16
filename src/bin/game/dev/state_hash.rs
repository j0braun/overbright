//! `NFSU2_STATE_HASH=file` (parity gate, docs/perf-gates.md): after
//! every simulation step (`Game::step`, whichever mode drives it: flows,
//! the playthrough, the bench, shots) one line with the hash of the
//! simulation's state, split by component so a divergence names where it
//! began: the step's number and dt, then the hashes (xxh3, hex) of the
//! whole, the globals (mode, game RNG, frame counter, clocks, cash), the
//! player's car (body, drivetrain, pedals, camera, world clock, weather),
//! the race (clock, progress of every racer), the rivals (bodies, AI
//! controls, their RNGs) and the traffic (bodies, lanes). Floats are
//! hashed by their bits: identical means bit-identical.
//!
//! Off (the variable unset) the step pays one cached load of an
//! `Option`. Not native: a measuring aid.
//!
//! The rate gate (docs/checks.md "Rate gate"): after those eight
//! columns, unchanged in meaning (the base-vs-new sim gate compares them
//! with older binaries), come the v2 columns: `rate` (the cadence the
//! state picks next: R race, S a 1/30 s scene, F the front end without a
//! world), `gsim` (the globals without the drawn-frame counter: mode and
//! menu kind, every RNG stream, the game and world clocks, the
//! world-update counter, cash, the scene clock), `clk` (the front-end
//! clock `Game::now` / `seconds` and the stepped world time: the wall
//! clock outside a rate run), `enc` (free roam's encounters and outrun),
//! `hud` (the HUD's running state), `fe` (the front end's screen state and
//! the garage), `fx` (particles by key and position, the rain), `vis`
//! (overbright level, nitrous blur, tint, fog, horizon fog, the flare
//! blink, the screen flash), `envf` (the drawn-frame counter the env-cube
//! faces follow) and `aud`
//! (what has been posted to the mixer). scripts/checks/rate-columns.txt
//! says which of them each gate counts.
use crate::*;
use std::io::Write as _;
use xxhash_rust::xxh3::Xxh3;

struct Sink {
    out: std::io::BufWriter<std::fs::File>,
    step: u64,
}

fn sink() -> &'static Option<std::sync::Mutex<Sink>> {
    static SINK: std::sync::OnceLock<Option<std::sync::Mutex<Sink>>> = std::sync::OnceLock::new();
    SINK.get_or_init(|| {
        let path = std::env::var_os("NFSU2_STATE_HASH")?;
        let mut out = std::io::BufWriter::new(std::fs::File::create(&path).ok()?);
        writeln!(out, "step\tdt\tall\tglobal\tplayer\trace\trivals\ttraffic\t{}", V2.join("\t")).ok()?;
        Some(std::sync::Mutex::new(Sink { out, step: 0 }))
    })
}

/// Bits of the state fed to a hasher.
trait Feed {
    fn f(&mut self, v: f32);
    fn v3(&mut self, v: Vec3);
    fn q(&mut self, v: Quat);
    fn u(&mut self, v: u64);
}

impl Feed for Xxh3 {
    fn f(&mut self, v: f32) {
        self.update(&v.to_bits().to_le_bytes());
    }
    fn v3(&mut self, v: Vec3) {
        for c in v.to_array() {
            self.f(c);
        }
    }
    fn q(&mut self, v: Quat) {
        for c in v.to_array() {
            self.f(c);
        }
    }
    fn u(&mut self, v: u64) {
        self.update(&v.to_le_bytes());
    }
}

/// A car body's simulated state.
fn car(h: &mut Xxh3, s: &CarState) {
    h.v3(s.position);
    h.q(s.orientation);
    h.v3(s.velocity);
    h.v3(s.world_acceleration);
    h.v3(s.spin);
    h.v3(s.angular_momentum);
    h.f(s.rest_damping);
    h.u(u64::from(s.rest));
    h.f(s.handbrake_ramp);
    h.u(s.hop as u64);
    h.f(s.hop_timer);
    h.f(s.steer_degrees);
    h.f(s.last_gas);
    h.f(s.brake_input);
    h.f(s.rpm());
    let gear = s.melo.as_ref().map(|m| m.gear).or(s.drivetrain.as_ref().map(|d| d.gear)).unwrap_or(1);
    h.u(gear as u64);
    h.u(s.nitrous.map_or(u64::MAX, |t| t.left as u64));
}

fn ai(h: &mut Xxh3, c: &nfsu2::drive::AiControls) {
    h.f(c.steer_degrees);
    h.f(c.gas);
    h.f(c.brake);
    h.f(c.handbrake);
    h.u(c.gear as u64);
    h.u(u64::from(c.nitrous));
}

fn traffic(h: &mut Xxh3, t: &Traffic) {
    h.u(t.cars.len() as u64);
    for c in &t.cars {
        car(h, &c.state);
        h.u(u64::from(c.lane.is_some()));
        h.u(u64::from(c.stopped));
        ai(h, &c.controls);
    }
}

/// The state's component hashes: global, player, race, rivals, traffic.
pub(crate) fn components(game: &Game) -> [u64; 5] {
    let mut g = Xxh3::new();
    g.u(match &game.mode {
        Mode::Explore => 1,
        Mode::Race(_) => 2,
        Mode::Menu(_) => 3,
    });
    g.u(u64::from(game.rng.0));
    g.u(u64::from(game.pacing.frame_count));
    g.f(game.sim.clock_ms);
    g.u(game.profile.as_ref().map_or(u64::MAX, |p| p.cash as u64));
    let mut p = Xxh3::new();
    if let Some(d) = game.sim.drive.as_ref() {
        car(&mut p, &d.state);
        p.v3(d.camera.position);
        p.f(d.camera.yaw);
        p.f(d.camera.pitch);
        p.f(d.clock);
        p.f(d.weather.amount);
    }
    let (mut r, mut rv, mut tr) = (Xxh3::new(), Xxh3::new(), Xxh3::new());
    let race = match &game.mode {
        Mode::Race(race) => Some(race),
        _ => game.sim.paused_race.as_ref(),
    };
    if let Some(race) = race {
        r.f(race.timing.clock);
        r.f(race.racers.me.finished.unwrap_or(f32::NAN));
        r.f(race.racers.me.along);
        if let Some(l) = race.racers.me.laps.as_ref() {
            r.u(l.lap as u64);
            r.f(l.progress);
        }
        for v in &race.racers.rivals {
            car(&mut rv, &v.state);
            ai(&mut rv, &v.controls);
            rv.u(u64::from(v.rng.0));
            r.f(v.progress.along);
            r.f(v.progress.finished.unwrap_or(f32::NAN));
            if let Some(l) = v.progress.laps.as_ref() {
                r.u(l.lap as u64);
                r.f(l.progress);
            }
        }
        if let Some(t) = race.racers.traffic.as_ref() {
            traffic(&mut tr, t);
        }
    }
    if let Some(t) = game.sim.traffic.as_ref() {
        traffic(&mut tr, t);
    }
    [g.digest(), p.digest(), r.digest(), rv.digest(), tr.digest()]
}

/// The v2 columns after the eight legacy ones (`rate` is a letter, the
/// others hashes).
pub(crate) const V2: [&str; 10] = ["rate", "gsim", "clk", "enc", "hud", "fe", "fx", "vis", "envf", "aud"];

/// The v2 columns' hashes (V2 minus `rate`).
pub(crate) fn components_v2(game: &Game) -> [u64; 9] {
    use std::hash::Hash;
    let mut g = Xxh3::new();
    std::mem::discriminant(&game.mode).hash(&mut g);
    if let Mode::Menu(m) = &game.mode {
        std::mem::discriminant(m).hash(&mut g);
        if let Menu::Icons(i, k) = m {
            g.update(format!("{k:?}").as_bytes());
            g.u(i.selected as u64);
        }
    }
    g.u(u64::from(game.rng.0));
    g.f(game.sim.clock_ms);
    g.u(u64::from(game.pacing.clocks.world_counter));
    g.u(game.profile.as_ref().map_or(u64::MAX, |p| p.cash as u64));
    g.f(game.scenes.shop.as_ref().map_or(f32::NAN, |s| s.frame));
    if let Some(d) = game.sim.drive.as_ref() {
        g.u(u64::from(d.world_clock.tick));
        g.u(u64::from(d.effects.rng.0));
        g.f(d.effects.clock);
    }
    let race = match &game.mode {
        Mode::Race(race) => Some(race),
        _ => game.sim.paused_race.as_ref(),
    };
    if let Some(r) = race {
        g.u(u64::from(r.racers.cars.rng.0));
        g.u(r.autopilot.as_ref().map_or(u64::MAX, |a| u64::from(a.rng.0)));
        g.u(r.racers.traffic.as_ref().map_or(u64::MAX, |t| u64::from(t.field.rng.0)));
        if let Some(p) = r.replay.playback.as_ref() {
            g.u(p.at as u64);
            g.f(p.carry);
            g.f(p.clock);
            g.u(u64::from(p.frames));
            g.u(u64::from(p.paused));
        }
    }
    g.u(game.sim.traffic.as_ref().map_or(u64::MAX, |t| u64::from(t.field.rng.0)));
    let mut c = Xxh3::new();
    c.f(game.now());
    c.f(game.seconds());
    c.u(game.pacing.world_time.to_bits());
    let k = &game.pacing.clocks;
    for v in [k.tick, k.package_ms, k.real, u64::from(k.sim_clock), u64::from(k.world_counter)] {
        c.u(v);
    }
    let mut e = Xxh3::new();
    e.u(game.sim.zone.0.map_or(u64::MAX, |z| z as u64));
    e.u(game.sim.zone.1.map_or(u64::MAX, |z| z as u64));
    if let Some(x) = game.sim.encounters.as_ref() {
        e.u(u64::from(x.state));
        e.u(u64::from(x.career));
        e.f(x.timer);
        e.u(x.target.map_or(u64::MAX, |t| t as u64));
        e.u(u64::from(x.icon) | u64::from(x.start) << 1 | u64::from(x.forward) << 2);
        e.f(x.clock);
        e.f(x.race_clock_from);
        e.u(u64::from(x.rng.0));
        e.u(u64::from(x.field.rng.0));
        e.u(x.throttle as u64);
        e.update(format!("{:?}", x.result).as_bytes());
        if let Some(o) = x.outrun.as_ref() {
            e.u(o.opponent as u64);
            e.f(o.clock);
            e.u(u64::from(o.start_tick));
            e.f(o.player);
            e.f(o.rival);
            e.u(u64::from(o.player_leads));
        }
        for k in &x.cars {
            e.u(u64::from(k.active) | u64::from(k.held) << 1 | u64::from(k.recovery_pending) << 2);
            if k.active {
                car(&mut e, &k.state);
            }
            e.f(k.placed.unwrap_or(f32::NAN));
            e.f(k.done.unwrap_or(f32::NAN));
            e.u(k.belt_hold as u64);
            e.u(k.belt_block_ticks as u64);
            e.f(k.belt_rating);
        }
    }
    let mut h = Xxh3::new();
    let hs = &game.hud_state;
    h.update(format!("{:?} {:?} {}", hs.toast, hs.cash_popup, hs.rear_view_on).as_bytes());
    let (screen, text) = hs.hud.dev_state();
    screen_state(&mut h, screen);
    h.update(text.as_bytes());
    let mut f = Xxh3::new();
    let fe = &game.fe;
    f.update(
        format!(
            "{} {:?} {} {:?} {} {} {} {} {} {}",
            fe.main_selection,
            fe.help_hover,
            fe.rating_up.get(),
            fe.fe_hover,
            fe.options_prompts,
            fe.splash_seen,
            fe.boot_cash,
            fe.quitting,
            fe.undim.is_some(),
            fe.from_boot
        )
        .as_bytes(),
    );
    if let Mode::Menu(Menu::Icons(i, _)) = &game.mode {
        f.f(i.opened);
    }
    let gf = &game.garage_fe;
    f.update(
        format!(
            "{} {} {:?} {:?} {:?} {:?} {:?} {}",
            game.render.garage.is_some(),
            gf.view.is_some(),
            gf.lot.as_ref().map(|l| (l.0, l.1)),
            gf.crib,
            gf.type_shown,
            gf.parts_sounded,
            gf.toggled,
            gf.left
        )
        .as_bytes(),
    );
    // The garage scene as the tick leaves it for the draw (the snapshot the
    // frames between ticks blend: view, car place, hydraulics, part poses,
    // flares, particles) and the track-map widgets' hand-off and zoom.
    let gs = &game.interp.cur.garage;
    if let Some(v) = gs.view.as_ref() {
        f.update(format!("{:?} {} {}", v.camera, v.car_angle.to_bits(), v.platform_angle.to_bits()).as_bytes());
        f.u(u64::from(v.disc as u32));
        f.u(gs.key);
        f.f(gs.now);
        f.f(gs.camber);
        f.u(u64::from(gs.headlights));
        if let Some(m) = gs.car_world {
            m.to_cols_array().iter().for_each(|&x| f.f(x));
        }
        if let Some((corner, drops)) = gs.hydro {
            corner.iter().chain(drops.iter()).for_each(|&x| f.f(x));
        }
        for c in gs.poses.iter().flatten().flatten().flatten() {
            f.q(c.rotation);
            f.v3(c.moved);
            f.f(c.value);
        }
        f.u(gs.particles.len() as u64);
        for p in &gs.particles {
            f.u(u64::from(p.id));
            f.v3(p.position);
            f.u(p.life as u64);
        }
    }
    let widget = match &game.mode {
        Mode::Menu(Menu::WorldMap(m)) => Some(m.widget),
        Mode::Menu(Menu::QuickRace(q)) => Some(q.widget),
        _ => None,
    };
    if let Some(w) = widget {
        f.update(format!("{:?}", w).as_bytes());
    }
    let mut x = Xxh3::new();
    let mut v = Xxh3::new();
    // The env cube's tick schedule (0x870818): the tick's
    // schedule index, the faces due at it and the eye they are rendered
    // from (S_m). Before U8 this hashed the drawn-frame counter. Its own
    // column, so `vis` holds the per-update scalars alone.
    let mut ef = Xxh3::new();
    let index = crate::render::snapshot::env_index(game.pacing.clocks.tick);
    ef.u(index);
    let rate = nfsu2::render::display::Display::from_custom(&game.settings.custom).env_rate;
    let due = if nfsu2::qol::reflection_every_frame() && rate != 0 {
        [true; 6]
    } else {
        nfsu2::render::display::env_faces_tick(rate, index)
    };
    ef.u(due.iter().fold(0u64, |b, &f| b << 1 | u64::from(f)));
    if let Some(e) = game.interp.env[(index % 6) as usize].as_ref().filter(|e| e.index == index) {
        ef.v3(e.centre);
        ef.v3(e.axes.0);
        ef.v3(e.axes.1);
        ef.v3(e.axes.2);
        ef.f(e.time);
    }
    // The overbright level on the tick (the post's own no longer
    // steps once the frame hands it the tick's).
    v.u(u64::from(game.visuals.level.0.to_bits()));
    if let Some(d) = game.sim.drive.as_ref() {
        let sys = &d.effects.system;
        x.u(sys.particles.len() as u64);
        for p in &sys.particles {
            x.u(u64::from(p.key) | u64::from(p.description) << 32);
            x.u(p.life as u64);
            x.v3(p.position);
        }
        x.update(d.weather.dev_state().as_bytes());
        for b in d.fog.dev_bits().into_iter().chain(d.tint.dev_bits()) {
            v.u(u64::from(b));
        }
        v.u(u64::from(d.horizon_fog.dev_bits()));
        for b in d.motion.dev_bits() {
            v.u(u64::from(b));
        }
        v.u(u64::from(d.vis.blink.time().to_bits()));
        v.u(u64::from(d.vis.out.blink));
        v.f(d.screen_flash.amount());
    }
    let mut a = Xxh3::new();
    if let Some(au) = game.audio.as_ref() {
        for w in au.mixer.dev_posted() {
            a.u(w);
        }
    }
    [g.digest(), c.digest(), e.digest(), h.digest(), f.digest(), x.digest(), v.digest(), ef.digest(), a.digest()]
}

/// A front-end screen state, its maps in key order.
fn screen_state(h: &mut Xxh3, s: &nfsu2::frontend::ScreenState) {
    fn sorted<K: Ord + Copy, V>(m: impl Iterator<Item = (K, V)>) -> Vec<(K, V)> {
        let mut v: Vec<_> = m.collect();
        v.sort_by_key(|e| e.0);
        v
    }
    h.update(format!("{:?}", sorted(s.scripts.iter().map(|(k, v)| (*k, *v)))).as_bytes());
    let mut hidden: Vec<_> = s.hidden.iter().copied().collect();
    hidden.sort_unstable();
    let mut shown: Vec<_> = s.shown.iter().copied().collect();
    shown.sort_unstable();
    h.update(format!("{hidden:?}{shown:?}").as_bytes());
    h.update(format!("{:?}", sorted(s.text.iter().map(|(k, v)| (*k, v)))).as_bytes());
    h.update(format!("{:?}", sorted(s.image.iter().map(|(k, v)| (*k, *v)))).as_bytes());
    h.update(format!("{:?}", sorted(s.color.iter().map(|(k, v)| (*k, *v)))).as_bytes());
    h.update(format!("{:?}", sorted(s.base_color.iter().map(|(k, v)| (*k, *v)))).as_bytes());
    h.update(format!("{:?}", sorted(s.script_color.iter().map(|(k, v)| (*k, *v)))).as_bytes());
    h.update(format!("{:?}", sorted(s.angle.iter().map(|(k, v)| (*k, *v)))).as_bytes());
    h.update(format!("{:?}", sorted(s.size.iter().map(|(k, v)| (*k, *v)))).as_bytes());
    h.update(format!("{:?}", sorted(s.uv.iter().map(|(k, v)| (*k, *v)))).as_bytes());
    h.update(format!("{:?}", sorted(s.position.iter().map(|(k, v)| (*k, *v)))).as_bytes());
}

/// The cadence the state picks for its next update: R race (1/60 s), S a
/// scene that holds the cars (1/30 s), F the front end without a world.
fn rate_letter(game: &Game) -> char {
    if game.sim.drive.is_none() {
        'F'
    } else if game.scene_frame_cap() {
        'S'
    } else {
        'R'
    }
}

/// After a step: its line (no-op unless `NFSU2_STATE_HASH` is set).
pub(crate) fn record(game: &Game, dt: f32) {
    super::rate::count_tick();
    let Some(m) = sink() else { return };
    let c = components(game);
    let v = components_v2(game);
    let mut all = Xxh3::new();
    for x in c {
        all.u(x);
    }
    let mut s = m.lock().unwrap();
    let n = s.step;
    s.step += 1;
    let _ = writeln!(
        s.out,
        "{n}\t{:08x}\t{:016x}\t{:016x}\t{:016x}\t{:016x}\t{:016x}\t{:016x}\t{}\t{}",
        dt.to_bits(),
        all.digest(),
        c[0],
        c[1],
        c[2],
        c[3],
        c[4],
        rate_letter(game),
        v.map(|x| format!("{x:016x}")).join("\t")
    );
    let _ = s.out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(s: &CarState) -> u64 {
        let mut h = Xxh3::new();
        car(&mut h, s);
        h.digest()
    }

    /// The car hash is a function of the bits: equal states hash equal,
    /// one ulp of position or a sign of zero changes it.
    #[test]
    fn car_hash_follows_the_bits() {
        let a = CarState::new(Vec3::new(1.0, 2.0, 3.0), 0.5);
        let mut b = CarState::new(Vec3::new(1.0, 2.0, 3.0), 0.5);
        assert_eq!(of(&a), of(&b));
        b.position.x = f32::from_bits(b.position.x.to_bits() + 1);
        assert_ne!(of(&a), of(&b));
        let mut c = a.clone();
        c.velocity.z = -0.0;
        let mut d = a.clone();
        d.velocity.z = 0.0;
        assert_ne!(of(&c), of(&d));
    }

    /// Off unless NFSU2_STATE_HASH is set (the tests never set it).
    #[test]
    fn sink_is_off_by_default() {
        if std::env::var_os("NFSU2_STATE_HASH").is_none() {
            assert!(sink().is_none());
        }
    }
}
