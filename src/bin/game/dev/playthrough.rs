//! `--playthrough`: a headless career bot. It plays the clone's career from
//! Start New Career to the end-of-game message and the ending, as a player
//! would through the game's own key handlers, screens and finish code, and
//! logs every step. It stops with an error on a blocker: a soft-lock (the
//! same state for too long), a panic, an event it cannot enter or finish, a
//! stage that opens nothing, or a save that does not load back the same.
//!
//! Shortcuts it takes (bot deviations, not game changes):
//! - Drives to markers by putting the car in the marker's trigger (a
//!   teleport, as the original-side playthroughs did).
//! - In races it moves its car along the race's road (a teleport each
//!   update); the finish, results, reward, payout, save and unlocks run
//!   through the game's own code.
//! - In drift races, which a teleport cannot score, it adds drift points to
//!   its own score before the line (as the stage-1 round wrote a drift
//!   score in the original's memory).
//! - Skips movies and in-engine scenes with Enter (the agreed deviation).
//!
//! The save folder is a fresh one (XDG_DATA_HOME isolated, as `--flow`);
//! `NFSU2_PLAYTHROUGH_LOG=file` copies the log there.
//!
//! Variants (`NFSU2_PLAYTHROUGH_SEED=<n>`, n > 0; `NFSU2_PLAYTHROUGH_STYLE=
//! a,b,…` or `all` picks which, all by default with a seed): the bot's
//! choices drawn from the seed — the sponsor signed, the starting car,
//! races lost on purpose (standing until a rival is home, no drift points),
//! races restarted (from the pause, or Post Race after a loss), quits to
//! the main menu (free roam, mid-race, Post Race) and the game left (a new
//! `Game`, the boot's profile load: at each D-Day step, in shops, mid-race,
//! after a stage change) each resumed and checked against the save on
//! disk, outruns won and lost (the encounter car moved as the `outrun_won`
//! shot does), world challenges taken (won, timed out, retried) and their
//! unique parts, the paint shop, the performance shop, cars bought into
//! free slots, switched at the garage and traded in. The seed also seeds
//! the game's random draws (events, rivals) — a bot setting. With no seed
//! the bot plays the one fixed path. `--playthrough-all [seeds]` runs
//! several seeds (default [`ALL_SEEDS`]) as child processes, each in its
//! own save folder, `NFSU2_PLAYTHROUGH_JOBS` at once (4), logs in
//! `NFSU2_PLAYTHROUGH_DIR` (a temp folder).
use crate::*;

const DT: f32 = 1.0 / 30.0;

/// The seeds `--playthrough-all` runs without a list.
pub(super) const ALL_SEEDS: &str = "1,2,3,4,5,6,7,8";

/// The variants' names, as `NFSU2_PLAYTHROUGH_STYLE` lists them.
const STYLES: [&str; 11] =
    ["sponsor", "car", "lose", "restart", "quit", "leave", "outrun", "challenge", "paint", "cars", "perf"];

/// The bot's variants and their random draws (splitmix64 on the seed).
#[derive(Clone, Debug, Default)]
struct Style {
    seed: u64,
    state: u64,
    on: HashSet<&'static str>,
}

impl Style {
    fn from_env() -> Style {
        let seed = std::env::var("NFSU2_PLAYTHROUGH_SEED").ok().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        let list = std::env::var("NFSU2_PLAYTHROUGH_STYLE").ok();
        let on: HashSet<&'static str> = match list.as_deref() {
            Some("all") => STYLES.into_iter().collect(),
            None if seed != 0 => STYLES.into_iter().collect(),
            Some(l) => STYLES.into_iter().filter(|s| l.split(',').any(|x| x.trim() == *s)).collect(),
            None => HashSet::new(),
        };
        Style { seed, state: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03, on }
    }

    fn has(&self, what: &str) -> bool {
        self.on.contains(what)
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// With variant `what` on, true with probability `p`.
    fn chance(&mut self, what: &str, p: f64) -> bool {
        self.has(what) && (self.next() >> 11) as f64 / (1u64 << 53) as f64 <= p
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

struct Bot {
    t0: Instant,
    log: Option<std::fs::File>,
    /// The number of game updates run.
    frames: u64,
    /// The last progress fingerprint and the frame it changed.
    last_state: String,
    since: u64,
    /// The career's own state and the bot steps since it last changed.
    last_career: String,
    idle: u32,
    /// The stage at the last save/reload check.
    checked_stage: u8,
    races: u32,
    wins: u32,
    losses: u32,
    shops: u32,
    shoots: u32,
    reloads: u32,
    /// A shop kind's last fruitless visit, by stage: the bank and the
    /// part levels unlocked then (a visit is tried again once the bank has
    /// grown or another level has unlocked).
    fruitless_shop: HashMap<(u8, u8), (u32, usize)>,
    /// The browser categories and icon options opened in this shop visit.
    explored: HashSet<String>,
    /// The last event engaged (for the stuck report).
    last_event: Option<usize>,
    ending_seen: bool,
    /// The final race's Post Race was left by a quit or the game left:
    /// its exit (0x54C9E0 → 0x513A50), which queues Scene15, never ran.
    ending_skipped: bool,
    blockers: Vec<String>,
    /// The race (event, round) being driven, the bot's place along its
    /// road, and its updates.
    race_key: Option<(usize, usize)>,
    /// The kind of the shop last entered.
    shop_kind: Option<u8>,
    along: Option<f32>,
    race_frames: u64,
    /// Teleports in a row the race's locator did not follow.
    misses: u32,
    careful: u32,
    /// Driven by the AI racer controller up to this progress (laps).
    drive_to: Option<f32>,
    style: Style,
    /// This race is to be lost (variant `lose`), and the race update at
    /// which it is paused for a Restart, a quit or the game left.
    lose_this: bool,
    race_break: Option<(u64, RaceBreak)>,
    /// The D-Day steps (career intro states) the game was left at.
    left_at: HashSet<String>,
    /// Per stage: car lot visits, garage visits for a switch, outrun
    /// tries, challenge tries, performance shop visits.
    tries: HashMap<(&'static str, u8), u32>,
    /// A garage visit to switch cars is under way.
    want_switch: bool,
    /// The car lot's trade question: browse the other cars.
    browse: bool,
    /// The world challenge just taken is left to run out of time.
    challenge_wait: bool,
    /// The car the next car lot visit takes.
    lot_want: Option<String>,
    /// The performance shop visit is for nitrous.
    want_nitrous: bool,
    /// The car `fruitless_shop` was found with (a new car starts it anew).
    fruitless_car: String,
    counts: std::collections::BTreeMap<&'static str, u32>,
    /// Odd states met on the way (not blockers): logged and summed up.
    findings: Vec<String>,
}

/// What a race's planned break does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum RaceBreak {
    Restart,
    Quit,
    Leave,
}

impl Bot {
    fn race_frames_limit(&self) -> u64 {
        3500
    }

    fn count(&mut self, what: &'static str) {
        *self.counts.entry(what).or_default() += 1;
    }

    fn tried(&mut self, what: &'static str, stage: u8) -> u32 {
        let n = self.tries.entry((what, stage)).or_default();
        *n += 1;
        *n - 1
    }

    fn finding(&mut self, game: &Game, what: String) {
        self.say(game, &format!("FINDING: {what}"));
        self.findings.push(what);
    }

    fn say(&mut self, game: &Game, what: &str) {
        let p = game.profile.as_ref();
        let line = format!(
            "[{:7.1} s | f {:7}] {what} | mode {} | stage {} cash {} rep {} won {} open {}",
            self.t0.elapsed().as_secs_f32(),
            self.frames,
            flow_mode(&game.mode),
            p.map_or(0, |p| p.stage),
            p.map_or(0, |p| p.cash),
            p.map_or(0, |p| p.reputation),
            p.map_or(0, |p| p.completed.len()),
            p.map_or(0, |p| p.open.len()),
        );
        println!("{line}");
        if let Some(f) = self.log.as_mut() {
            use std::io::Write;
            let _ = writeln!(f, "{line}");
        }
    }

    fn step(&mut self, game: &mut Game, dt: f32) -> Result<()> {
        game.pacing.clock = game.pacing.clock.map(|c| c + f64::from(dt));
        // The ticks the span holds (1/30 s is two race ticks, one
        // scene tick).
        game.advance_for(dt)?;
        self.frames += 1;
        Ok(())
    }

    fn steps(&mut self, game: &mut Game, n: usize) -> Result<()> {
        for _ in 0..n {
            self.step(game, DT)?;
        }
        Ok(())
    }

    /// A key as the window gives it, through the input queue (
    /// `Game::input_at`): pressed at the start of the next tick, held
    /// through the step, let go at the start of the next step's first tick.
    fn tap(&mut self, game: &mut Game, code: KeyCode) -> Result<()> {
        let t = game.next_tick();
        game.input_at(t, InputEvent::press(code));
        self.step(game, DT)?;
        game.hold_next(code, false);
        self.step(game, DT)
    }

    /// A menu key after a short look (the screens' focus and fade times).
    fn menu(&mut self, game: &mut Game, key: Key) -> Result<()> {
        self.steps(game, 6)?;
        let code = match key {
            Key::Accept => KeyCode::Enter,
            Key::Back => KeyCode::Escape,
            Key::Left => KeyCode::ArrowLeft,
            Key::Right => KeyCode::ArrowRight,
            Key::Up => KeyCode::ArrowUp,
            Key::Down => KeyCode::ArrowDown,
            k => {
                game.menu_key(k)?;
                return self.step(game, DT);
            }
        };
        self.tap(game, code)
    }

    /// What counts as progress: the mode and the career's state.
    fn fingerprint(game: &Game) -> String {
        let p = game.profile.as_ref();
        let race = match &game.mode {
            Mode::Race(r) => format!("{} {} {:.0}", r.event, r.round, r.racers.me.along / 200.0),
            _ => String::new(),
        };
        format!(
            "{} {race} {:?} {:?}",
            flow_mode(&game.mode),
            p.map(|p| (
                p.stage,
                p.cash,
                p.completed.len(),
                p.inbox.entries.len(),
                p.inbox.unread(),
                p.sponsors.len(),
                p.milestones.len(),
                p.installed.len(),
                p.intro,
                p.car.clone()
            )),
            game.scenes.shop.as_ref().map(|s| s.frame as i32 / 30),
        )
    }
}

/// The car put in a trigger's box (a teleport): the marker test runs on
/// the next frame.
fn put(game: &mut Game, at: Vec3) {
    game.scenes.shop = None;
    if let Some(d) = game.sim.drive.as_mut() {
        d.place(at + Vec3::Z * 0.5, d.state.heading());
    }
    game.career_world.marker_trigger = None;
}

/// A profile's visual rating with its car (as `Game::visual_rating`).
fn rating_of(game: &Game, p: &Profile) -> f32 {
    let stock_paint = game.game_data.car_types.iter().find(|t| t.name == p.car).map_or(0, |t| t.stock_paint);
    nfsu2::car::ride_visual_rating(&game.game_data.parts, &game.game_data.career.costs, &p.car, stock_paint, &p.installed)
}

/// The visual points a part gives in a slot (0x5234D0's terms; none for
/// the vinyl colours, slot 0x1E and the stock paint).
fn slot_points(game: &Game, car: &str, slot: u8, hash: Option<u32>) -> i32 {
    // The part finds walk the whole part list: kept per car, slot and part.
    thread_local! {
        static KEPT: std::cell::RefCell<HashMap<(String, u8, Option<u32>), i32>> = std::cell::RefCell::new(HashMap::new());
    }
    let key = (car.to_string(), slot, hash);
    if let Some(v) = KEPT.with(|k| k.borrow().get(&key).copied()) {
        return v;
    }
    let v = slot_points_of(game, car, slot, hash);
    KEPT.with(|k| k.borrow_mut().insert(key, v));
    v
}

fn slot_points_of(game: &Game, car: &str, slot: u8, hash: Option<u32>) -> i32 {
    let slot = slot as usize;
    if (0x4E..=0x5C).contains(&slot) || slot == 0x1E {
        return 0;
    }
    let stock_paint = game.game_data.car_types.iter().find(|t| t.name == car).map_or(0, |t| t.stock_paint);
    let part = match hash {
        Some(h) => game.game_data.parts.find(car, slot, Some(h), None),
        None if slot == 0x3F => return 0,
        None => game.game_data.parts.stock(car, slot),
    };
    match part {
        Some(x) if !(slot == 0x3F && x.hash == stock_paint) => {
            game.game_data.career.costs.points(game.game_data.parts.category(slot, x), x.level).max(0)
        }
        _ => 0,
    }
}

/// What fitting `hash` in `slot` adds to the rating `now`: by the slot's
/// points, or (where a widebody kit takes the bumpers and skirts out) the
/// whole rating again.
fn rating_gain(game: &Game, p: &Profile, now: f32, slot: u8, hash: u32) -> f32 {
    if matches!(slot, 1 | 2 | 6 | 0x0B) {
        let mut ride = p.installed.clone();
        ride.insert(slot, hash);
        return game.visual_rating_of(&ride) - now;
    }
    let old = slot_points(game, &p.car, slot, p.installed.get(&slot).copied());
    let new = slot_points(game, &p.car, slot, Some(hash));
    (now + (new - old) as f32 / 500.0).min(10.0) - now
}

/// The part cost levels the career has unlocked (0x515360 over the cost
/// table, as the Career Stats' parts share counts them).
fn unlocked_levels(game: &Game, p: &Profile) -> usize {
    let mut n = 0;
    for (i, levels) in game.game_data.career.costs.categories.iter().enumerate() {
        n += levels
            .iter()
            .filter(|l| (1..=4).contains(&l.unlock) && p.part_unlocked(&game.game_data.career, i as u8 + 1, l.level.max(0) as u8))
            .count();
    }
    n
}

fn trigger_centre(t: &career::Trigger) -> Vec3 {
    ((t.min + t.max) * 0.5).extend(t.position.z)
}

fn shop_named(game: &Game, name: &str) -> Option<(usize, Vec3)> {
    let shop = game.game_data.career.shops.iter().position(|s| s.name == name)?;
    let t = game.sim.route_triggers.find(game.game_data.career.shops[shop].trigger)?;
    Some((shop, trigger_centre(&t)))
}

/// What a save carries, field by field, for the reload check.
fn save_summary(game: &Game, p: &Profile) -> Vec<(&'static str, String)> {
    vec![
        ("rating", format!("{:.2}", rating_of(game, p))),
        ("name", p.name.clone()),
        ("stage", p.stage.to_string()),
        ("cash", p.cash.to_string()),
        ("rep", p.reputation.to_string()),
        ("car", p.car.clone()),
        ("completed", format!("{:?}", p.completed.iter().collect::<std::collections::BTreeSet<_>>())),
        ("sponsors", format!("{:?}", p.sponsors.iter().collect::<std::collections::BTreeMap<_, _>>())),
        ("milestones", format!("{:?}", p.milestones.iter().collect::<std::collections::BTreeSet<_>>())),
        ("offered", format!("{:?}", p.offered.iter().collect::<std::collections::BTreeSet<_>>())),
        ("installed", format!("{:?}", p.installed)),
        ("perf", format!("{:?}", p.perf.iter().collect::<std::collections::BTreeSet<_>>())),
        ("inbox", format!("{:?}", p.inbox.entries.iter().map(|e| (e.message, e.read)).collect::<Vec<_>>())),
        ("visited", format!("{:?}", p.visited.iter().collect::<std::collections::BTreeSet<_>>())),
        ("stage_rep", format!("{:?}", p.stage_rep.iter().map(|(s, (r, n))| (*s, *r as i64, *n)).collect::<Vec<_>>())),
    ]
}

/// The fields that differ, each with both values.
fn summary_diff(a: &[(&'static str, String)], b: &[(&'static str, String)]) -> Vec<String> {
    a.iter().zip(b).filter(|(x, y)| x.1 != y.1).map(|(x, y)| format!("{}: {} vs {}", x.0, x.1, y.1)).collect()
}

pub(crate) fn playthrough(game: &mut Game) -> Result<()> {
    let log = std::env::var("NFSU2_PLAYTHROUGH_LOG").ok().map(std::fs::File::create).transpose()?;
    let mut bot = Bot {
        t0: Instant::now(),
        log,
        frames: 0,
        last_state: String::new(),
        since: 0,
        last_career: String::new(),
        idle: 0,
        checked_stage: 0,
        races: 0,
        wins: 0,
        losses: 0,
        shops: 0,
        shoots: 0,
        reloads: 0,
        fruitless_shop: HashMap::new(),
        explored: HashSet::new(),
        last_event: None,
        ending_seen: false,
        ending_skipped: false,
        blockers: Vec::new(),
        race_key: None,
        shop_kind: None,
        along: None,
        race_frames: 0,
        misses: 0,
        careful: 0,
        drive_to: None,
        style: Style::from_env(),
        lose_this: false,
        race_break: None,
        left_at: HashSet::new(),
        tries: HashMap::new(),
        want_switch: false,
        browse: false,
        challenge_wait: false,
        lot_want: None,
        want_nitrous: false,
        fruitless_car: String::new(),
        counts: Default::default(),
        findings: Vec::new(),
    };
    game.pacing.clock = Some(0.0);
    if bot.style.seed != 0 {
        // A bot setting: the game's draws (events, rivals) from the seed.
        game.rng = nfsu2::opponents::GameRng(bot.style.seed as u32 | 1);
        let mut on: Vec<_> = bot.style.on.iter().copied().collect();
        on.sort();
        bot.say(game, &format!("variants: seed {} with {}", bot.style.seed, on.join(",")));
    }
    let r = run(game, &mut bot);
    let p = game.profile.as_ref();
    bot.say(game, &format!(
        "summary: races {} (won {}, lost {}), shop visits {}, photo shoots {}, save reloads {}, ending seen {}, {:.1} s",
        bot.races, bot.wins, bot.losses, bot.shops, bot.shoots, bot.reloads, bot.ending_seen, bot.t0.elapsed().as_secs_f32()
    ));
    if !bot.counts.is_empty() {
        let c = bot.counts.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
        bot.say(game, &format!("variants done: {c}"));
    }
    for f in bot.findings.clone() {
        bot.say(game, &format!("finding: {f}"));
    }
    for b in bot.blockers.clone() {
        bot.say(game, &format!("blocker: {b}"));
    }
    r?;
    // The end: stage 6, the end-of-game message in the inbox, the ending
    // movie played, and no blocker on the way.
    let p = p.context("no profile at the end")?;
    let end_message = game
        .game_data.career
        .messages
        .iter()
        .find(|m| m.name == "SMS_END_OF_GAME_MESSAGE")
        .map(|m| m.hash)
        .context("SMS_END_OF_GAME_MESSAGE in the data")?;
    anyhow::ensure!(p.stage >= 6, "the career did not end (stage {})", p.stage);
    anyhow::ensure!(p.inbox.contains(end_message), "no end-of-game message");
    anyhow::ensure!(bot.ending_seen || bot.ending_skipped, "the ending movie (Scene15) did not play");
    anyhow::ensure!(bot.blockers.is_empty(), "{} blocker(s) on the way", bot.blockers.len());
    println!("playthrough passed: the career ended in {:.1} s", bot.t0.elapsed().as_secs_f32());
    Ok(())
}

fn run(game: &mut Game, bot: &mut Bot) -> Result<()> {
    // Main menu → Career → Start New Career (as a player: keys).
    game.mode = game.icon_menu(MenuKind::Main);
    bot.say(game, "main menu");
    game.act(Action::Career, MenuKind::Main)?;
    bot.say(game, "career menu");
    game.act(Action::NewCareer, MenuKind::Career)?;
    // MC_Main: Yes, the offered name, Yes to save.
    anyhow::ensure!(matches!(game.mode, Mode::Menu(Menu::McMain(_))), "MC_Main after Start New Career");
    bot.menu(game, Key::Left)?;
    bot.menu(game, Key::Accept)?;
    bot.menu(game, Key::Accept)?;
    bot.menu(game, Key::Left)?;
    bot.menu(game, Key::Accept)?;
    // The editor's first Enter does nothing (0x5512A9): the save question
    // is still up (No focused): Yes.
    if matches!(&game.mode, Mode::Menu(Menu::McMain(m)) if m.step == screens::McStep::Confirm) {
        bot.menu(game, Key::Left)?;
        bot.menu(game, Key::Accept)?;
    }
    let saved = game.profile.as_ref().is_some_and(|p| profiles::exists(&profiles::sanitize(&p.name)));
    bot.say(game, &format!("new career named (saved on disk: {saved}, in {})", flow_mode(&game.mode)));
    const LIMIT: u64 = 3_000_000;
    while bot.frames < LIMIT {
        let fp = Bot::fingerprint(game);
        if fp != bot.last_state {
            bot.last_state = fp;
            bot.since = bot.frames;
        } else if bot.frames - bot.since > 30 * 240 {
            anyhow::bail!(
                "soft-lock: no progress for 240 s of game time in {} (last event {:?})",
                bot.last_state,
                bot.last_event.map(|e| game.game_data.career.events[e].name.clone())
            );
        }
        // (With the races the bot started: a run of races lost on
        // purpose moves nothing in the career.)
        let career = game.profile.as_ref().map_or(String::new(), |p| {
            format!(
                "{} {} {} {} {} {} {} {:?} {}",
                p.stage,
                p.cash,
                p.completed.len(),
                p.races_entered,
                p.installed.len(),
                p.milestones.len(),
                p.inbox.entries.len(),
                p.intro,
                bot.races
            )
        });
        if career != bot.last_career {
            bot.last_career = career;
            bot.idle = 0;
        } else {
            bot.idle += 1;
            if bot.idle > 4000 {
                anyhow::bail!(
                    "soft-lock: the career has not moved for 4000 bot steps (in {}, last event {:?})",
                    flow_mode(&game.mode),
                    bot.last_event.map(|e| game.game_data.career.events[e].name.clone())
                );
            }
        }
        // The stage changed and the game saved: reload from the written
        // profile through the real load path, and compare.
        let stage = game.profile.as_ref().map_or(0, |p| p.stage);
        if stage != bot.checked_stage
            && matches!(game.mode, Mode::Explore)
            && game.scenes.shop.is_none()
            && !game.career_world.dday.running()
        {
            if stage >= 2 {
                if bot.style.chance("leave", 0.5) {
                    leave(game, bot, &format!("after the change to stage {stage}"))?;
                } else {
                    reload_check(game, bot)?;
                }
            }
            bot.checked_stage = stage;
            bot.say(game, &format!("stage {stage} begun"));
            continue;
        }
        let done = game.profile.as_ref().is_some_and(|p| p.stage >= 6);
        if done
            && (bot.ending_seen || bot.ending_skipped)
            && matches!(game.mode, Mode::Explore)
            && game.scenes.shop.is_none()
        {
            break;
        }
        match &game.mode {
            Mode::Explore => explore_step(game, bot)?,
            Mode::Race(_) => race_step(game, bot)?,
            Mode::Menu(_) => menu_step(game, bot)?,
        }
    }
    anyhow::ensure!(bot.frames < LIMIT, "frame limit reached");
    Ok(())
}

/// After a stage change has been saved: quit to the main menu, load the
/// written profile from disk through the Profile Manager's list (MC_List →
/// MC_Main's load, 0xC11), Resume Career, out of the garage; the career
/// must come back the same.
fn reload_check(game: &mut Game, bot: &mut Bot) -> Result<()> {
    if std::env::var_os("NFSU2_PLAYTHROUGH_POINTS").is_some()
        && let Some(p) = game.profile.as_ref()
    {
        // The visual points each slot can give with this car, by level.
        let mut total = 0;
        for slot in 0..0xAAusize {
            let best = game
                .game_data.parts
                .parts_for(&p.car, slot)
                .into_iter()
                .map(|x| (game.game_data.career.costs.points(game.game_data.parts.category(slot, x), x.level), x.level))
                .max();
            if let Some((pts, lv)) = best.filter(|b| b.0 > 0) {
                total += pts;
                println!("points: slot {slot:#04x} best {pts} (level {lv})");
            }
        }
        println!("points: total best {total}");
    }
    let before = game.profile.as_ref().map(|p| save_summary(game, p)).context("a profile")?;
    let written = written_summary(game)?;
    quit_to_main(game, bot)?;
    resume(game, bot, true, "stage change")?;
    // What the save point wrote against the career in memory (the clone's
    // in-memory profile keeps no stock parts; the written one names them).
    for d in summary_diff(&before, &written) {
        bot.say(game, &format!("in memory vs written: {}", d.chars().take(400).collect::<String>()));
    }
    Ok(())
}

/// The profile as the last save point wrote it.
fn written_summary(game: &Game) -> Result<Vec<(&'static str, String)>> {
    let name = game.profile.as_ref().map(|p| p.name.clone()).context("a profile")?;
    let ctx = career::NativeContext::get().context("native context")?;
    let disk = profiles::read(&profiles::sanitize(&name))
        .ok()
        .map(|s| Profile::from_native(&s, ctx))
        .context("the written profile")?;
    Ok(save_summary(game, &disk))
}

/// The bot's per-race and per-visit state after a quit or a new game.
fn forget(bot: &mut Bot) {
    bot.race_key = None;
    bot.shop_kind = None;
    bot.along = None;
    bot.drive_to = None;
    bot.race_break = None;
    bot.lose_this = false;
    bot.want_switch = false;
    bot.explored.clear();
}

/// Quit to Main Menu from where the game is (as a player: free roam's or
/// the race's pause by Escape, or Post Race's own option), OK on the
/// in-game dialog (on the left), then any save question answered Yes.
fn quit_to_main(game: &mut Game, bot: &mut Bot) -> Result<()> {
    let in_race = matches!(game.mode, Mode::Race(_));
    if matches!(game.mode, Mode::Explore | Mode::Race(_)) {
        bot.tap(game, KeyCode::Escape)?;
        let want = if in_race { MenuKind::RacePause } else { MenuKind::ExplorePause };
        if !matches!(game.mode, Mode::Menu(Menu::Icons(_, k)) if k == want) {
            let f = format!(
                "Escape in {} opened {} (not the pause)",
                if in_race { "a race" } else { "free roam" },
                flow_mode(&game.mode)
            );
            bot.finding(game, f);
            if in_race {
                anyhow::bail!("no race pause");
            }
            game.mode = game.icon_menu(MenuKind::ExplorePause);
        }
    }
    let (kind, actions) = icon_options(game).context("a menu to quit from")?;
    let i = actions
        .iter()
        .position(|a| matches!(a, Action::QuitToMainMenu))
        .with_context(|| format!("{kind:?} has no Quit to Main Menu: {actions:?}"))?;
    if let Mode::Menu(Menu::Icons(m, _)) = &mut game.mode {
        m.selected = i;
    }
    bot.steps(game, 6)?;
    game.act(Action::QuitToMainMenu, kind)?;
    bot.menu(game, Key::Left)?;
    bot.menu(game, Key::Accept)?;
    for _ in 0..20 {
        if matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))) {
            break;
        }
        answer_question(game, bot, true)?;
    }
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))),
        "quit to the main menu from {kind:?}: in {}",
        flow_mode(&game.mode)
    );
    bot.count(if in_race {
        "quit mid-race"
    } else if kind == MenuKind::PostRace {
        "quit at post race"
    } else {
        "quit in free roam"
    });
    forget(bot);
    Ok(())
}

/// From the main menu back into the career: the profile loaded from disk
/// through the Profile Manager's list (`from_disk`, as a new run: nothing
/// in memory) or the one in memory, then Resume Career. A profile loaded
/// from disk must be the written one.
fn resume(game: &mut Game, bot: &mut Bot, from_disk: bool, why: &str) -> Result<()> {
    let name = game.profile.as_ref().map(|p| p.name.clone()).context("a profile")?;
    let written = written_summary(game)?;
    let memory = game.profile.as_ref().map(|p| save_summary(game, p)).context("a profile")?;
    if from_disk {
        game.profile = None;
        game.mode = game.mc_list(McOp::Load);
        let Mode::Menu(Menu::McList(l)) = &game.mode else { anyhow::bail!("the load list") };
        let at = l
            .names
            .iter()
            .position(|n| n.eq_ignore_ascii_case(&profiles::sanitize(&name)))
            .with_context(|| format!("{name} in the load list {:?}", l.names))?;
        let at0 = l.selected;
        for _ in at0..at {
            bot.menu(game, Key::Down)?;
        }
        for _ in at..at0 {
            bot.menu(game, Key::Up)?;
        }
        bot.menu(game, Key::Accept)?;
        for _ in 0..10 {
            match &game.mode {
                Mode::Menu(Menu::McMessage(m, _)) => {
                    bot.say(game, &format!("load: {:?}", m.text));
                    if m.buttons.len() == 2 {
                        bot.menu(game, Key::Left)?;
                    }
                    bot.menu(game, Key::Accept)?;
                }
                Mode::Menu(Menu::McMain(_)) => bot.menu(game, Key::Accept)?,
                _ => break,
            }
        }
    }
    let loaded = game.profile.as_ref().map(|p| save_summary(game, p)).context("loaded no profile")?;
    let mismatch = summary_diff(if from_disk { &written } else { &memory }, &loaded);
    if !mismatch.is_empty() {
        let stage = game.profile.as_ref().map_or(0, |p| p.stage);
        bot.blockers.push(format!(
            "{why}: stage {stage} {} mismatch: {}",
            if from_disk { "save/load" } else { "resume" },
            mismatch.join("; ")
        ));
    }
    game.mode = game.icon_menu(MenuKind::Main);
    game.act(Action::Career, MenuKind::Main)?;
    game.act(Action::ResumeCareer, MenuKind::Career)?;
    let stage = game.profile.as_ref().map_or(0, |p| p.stage);
    if stage >= 1 && !matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Crib))) {
        let f = format!("{why}: Resume Career at stage {stage} opened {} (not the garage)", flow_mode(&game.mode));
        bot.finding(game, f);
    }
    bot.reloads += 1;
    bot.say(
        game,
        &format!(
            "{why}: resumed {name} from {} (stage {stage}): {}",
            if from_disk { "disk" } else { "memory" },
            if mismatch.is_empty() {
                "same".to_string()
            } else {
                format!(
                    "MISMATCH in {:?}",
                    mismatch.iter().map(|m| m.split(':').next().unwrap_or("")).collect::<Vec<_>>()
                )
            }
        ),
    );
    Ok(())
}

/// The game left where it is (the window closed, a crash): a new `Game`
/// as a new run makes it, the boot's profile step ("Load complete."),
/// Career, Resume Career. The career comes back as the last save wrote
/// it; what was not saved is lost (logged).
fn leave(game: &mut Game, bot: &mut Bot, at: &str) -> Result<()> {
    let Some(name) = game.profile.as_ref().map(|p| p.name.clone()) else { return Ok(()) };
    if !profiles::exists(&profiles::sanitize(&name)) {
        bot.say(game, &format!("the game not left {at}: no save yet"));
        return Ok(());
    }
    let written = written_summary(game)?;
    let memory = game.profile.as_ref().map(|p| save_summary(game, p)).context("a profile")?;
    bot.say(game, &format!("the game left {at} (in {})", flow_mode(&game.mode)));
    // NFSU2_PLAYTHROUGH_SAVES=dir: the save the game was left with, kept
    // (to load in the original).
    if let Some(dir) = std::env::var_os("NFSU2_PLAYTHROUGH_SAVES") {
        let n = bot.counts.get("game left").copied().unwrap_or(0);
        let to = std::path::Path::new(&dir).join(format!("{n:02}-{}.sav", at.replace([' ', '/'], "_")));
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::copy(profiles::file(&profiles::sanitize(&name)), &to);
    }
    let clock = game.pacing.clock;
    let t = Instant::now();
    *game = Game::new()?;
    game.pacing.clock = clock;
    game.mode = game.boot_profile_step();
    for _ in 0..20 {
        match &game.mode {
            Mode::Menu(Menu::Icons(_, MenuKind::Main)) => break,
            Mode::Menu(Menu::McMessage(m, _)) => {
                let text = m.text.clone();
                bot.say(game, &format!("boot: {text:?}"));
                answer_question(game, bot, true)?;
            }
            Mode::Menu(Menu::McMain(_)) => bot.menu(game, Key::Accept)?,
            Mode::Menu(Menu::Dialog(..)) => answer_question(game, bot, true)?,
            _ => bot.step(game, DT)?,
        }
    }
    anyhow::ensure!(
        matches!(game.mode, Mode::Menu(Menu::Icons(_, MenuKind::Main))),
        "the boot after leaving {at}: in {}",
        flow_mode(&game.mode)
    );
    let loaded = game.profile.as_ref().map(|p| save_summary(game, p)).context("the boot loaded no profile")?;
    let mismatch = summary_diff(&written, &loaded);
    if !mismatch.is_empty() {
        let stage = game.profile.as_ref().map_or(0, |p| p.stage);
        bot.blockers.push(format!("left {at}: stage {stage} boot load mismatch: {}", mismatch.join("; ")));
    }
    let diffs = summary_diff(&memory, &written);
    let lost: Vec<String> = diffs.iter().map(|d| d.split(':').next().unwrap_or("").to_string()).collect();
    for d in diffs.iter().filter(|d| !d.starts_with("installed")) {
        bot.say(game, &format!("left {at}: memory vs written {}", d.chars().take(600).collect::<String>()));
    }
    game.act(Action::Career, MenuKind::Main)?;
    game.act(Action::ResumeCareer, MenuKind::Career)?;
    forget(bot);
    bot.count("game left");
    bot.reloads += 1;
    bot.say(
        game,
        &format!(
            "left {at}: a new game in {:.1} s, {name} booted: {}, unsaved and lost: {lost:?}; Resume Career opened {}",
            t.elapsed().as_secs_f32(),
            if mismatch.is_empty() { "same as written" } else { "MISMATCH" },
            flow_mode(&game.mode)
        ),
    );
    Ok(())
}

/// A Yes / No question or an OK message: Yes (or the only button).
fn answer_question(game: &mut Game, bot: &mut Bot, yes: bool) -> Result<()> {
    let n = match &game.mode {
        Mode::Menu(Menu::Dialog(d, _)) => d.buttons.len(),
        Mode::Menu(Menu::McMessage(m, _)) => m.buttons.len(),
        _ => {
            bot.step(game, DT)?;
            return Ok(());
        }
    };
    if n == 2 {
        bot.menu(game, if yes { Key::Left } else { Key::Right })?;
    }
    bot.menu(game, Key::Accept)
}

/// The next thing to do in free roam.
enum Goal {
    Shop(usize, Vec3, &'static str),
    Shoot(u32, Vec3),
    Event(usize, Vec3),
    /// A world challenge's start marker (variant `challenge`).
    Challenge(usize, Vec3),
    /// The running world challenge's goal (its shop or photo spot).
    ChallengeGoal(MarkerTarget, Vec3),
    /// An encounter car challenged (variant `outrun`).
    Outrun,
}

/// The newest open shop of a kind (0 garage, 1 graphics, 2 body, 3
/// performance, 4 car lot, 5 specialties).
fn open_shop(game: &Game, kind: u8) -> Option<(usize, Vec3)> {
    let p = game.profile.as_ref()?;
    let (s, _) = game
        .game_data.career
        .shops
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind == kind && p.shop_open(&game.game_data.career, s))
        .max_by_key(|(_, s)| s.stage)?;
    let t = game.sim.route_triggers.find(game.game_data.career.shops[s].trigger)?;
    Some((s, trigger_centre(&t)))
}

fn next_goal(game: &mut Game, bot: &mut Bot) -> Option<Goal> {
    let p = game.profile.as_ref()?;
    if p.car != bot.fruitless_car {
        bot.fruitless_car = p.car.clone();
        bot.fruitless_shop.clear();
    }
    if p.intro == Some(career::DDAY_EVENT_A) {
        return shop_named(game, "CC_CAR_LOT_1").map(|(s, at)| Goal::Shop(s, at, "the car lot"));
    }
    if p.intro == Some(career::DDAY_EVENT_B) {
        return shop_named(game, "CRIB_1").map(|(s, at)| Goal::Shop(s, at, "the garage"));
    }
    // A world challenge under way: its goal (the marker table holds it
    // alone).
    if game.career_world.challenge.is_some() {
        return game.challenge_goal_marker().map(|(m, t)| Goal::ChallengeGoal(m, trigger_centre(&t)));
    }
    // The photo shoots on offer (the required DVDs first).
    let mut shoots = game.shoots();
    shoots.sort_by_key(|(h, _)| !game.game_data.career.milestones.iter().any(|m| m.hash == *h && m.required));
    if let Some((h, t)) = shoots.first() {
        return Some(Goal::Shoot(*h, t.position));
    }
    let stage = p.stage;
    // The variants' side trips.
    if stage >= 1 {
        let (free, cars, cash) = (p.free_slot().is_some(), p.car_slots().len(), p.cash);
        if free
            && bot.tries.get(&("lot", stage)).copied().unwrap_or(0) < 2
            && bot.style.has("cars")
            && let Some((s, at)) = open_shop(game, 4)
        {
            bot.tried("lot", stage);
            return Some(Goal::Shop(s, at, "the car lot (a car for the free slot)"));
        }
        if !free
            && bot.style.chance("cars", 0.03)
            && bot.tried("trade", stage) == 0
            && let Some((s, at)) = open_shop(game, 4)
        {
            return Some(Goal::Shop(s, at, "the car lot (a trade-in)"));
        }
        if cars > 1
            && bot.style.chance("cars", 0.05)
            && bot.tried("switch", stage) == 0
            && let Some((s, at)) = open_shop(game, 0)
        {
            bot.want_switch = true;
            return Some(Goal::Shop(s, at, "the garage (another car)"));
        }
        if bot.style.chance("outrun", 0.12) && bot.tried("outrun", stage) < 8 {
            return Some(Goal::Outrun);
        }
        let challenges = game.challenge_markers();
        if !challenges.is_empty() && bot.style.chance("challenge", 0.3) && bot.tried("challenge", stage) < 6 {
            let (i, t) = challenges[bot.style.below(challenges.len())];
            return Some(Goal::Challenge(i, trigger_centre(&t)));
        }
        if cash >= 5000
            && bot.style.chance("perf", 0.05)
            && bot.tried("perf", stage) == 0
            && let Some((s, at)) = open_shop(game, 3)
        {
            return Some(Goal::Shop(s, at, "a performance shop"));
        }
        if cash >= 2000
            && bot.style.chance("paint", 0.04)
            && bot.tried("paint", stage) == 0
            && let Some((s, at)) = open_shop(game, 1)
        {
            return Some(Goal::Shop(s, at, "a graphics shop (paint)"));
        }
    }
    let p = game.profile.as_ref()?;
    // A required DVD the rating does not reach yet: the body shop, then
    // the car specialties, then the graphics shop's vinyls (each again
    // once the bank has grown).
    let rating = game.visual_rating();
    let want = game
        .game_data.career
        .milestones
        .iter()
        .filter(|m| m.stage == stage && m.required && !p.milestones.contains(&m.hash))
        .map(|m| m.rating)
        .fold(None, |a: Option<f32>, r| Some(a.map_or(r, |a| a.min(r))));
    let events_left = !p.open.is_empty();
    if let Some(want) = want
        && rating < want
        && (!events_left || p.cash >= 3000)
    {
        for kind in [2u8, 5, 1] {
            if bot
                .fruitless_shop
                .get(&(stage, kind))
                .is_some_and(|&(c, u)| p.cash < (c * 2).max(c + 10_000) && unlocked_levels(game, p) <= u)
            {
                continue;
            }
            // The newest shop of the kind, found by driving in.
            let shop = game
                .game_data.career
                .shops
                .iter()
                .enumerate()
                .filter(|(_, s)| s.kind == kind && s.stage <= stage)
                .max_by_key(|(_, s)| s.stage)
                .map(|(i, _)| i);
            if let Some(s) = shop
                && let Some(t) = game.sim.route_triggers.find(game.game_data.career.shops[s].trigger)
            {
                return Some(Goal::Shop(
                    s,
                    trigger_centre(&t),
                    match kind {
                        2 => "a body shop",
                        5 => "a car specialties shop",
                        _ => "a graphics shop",
                    },
                ));
            }
        }
        // Every shop tried: nitrous (a performance shop), which opens the
        // specialties' Nitrous Purge (slot 0xA2 needs perf parts 12–14).
        let nitrous = [12u8, 13, 14].iter().any(|id| p.perf.contains(id));
        if !nitrous
            && p.cash >= 3000
            && bot.tried("nitrous", stage) < 2
            && let Some((s, at)) = open_shop(game, 3)
        {
            bot.want_nitrous = true;
            bot.fruitless_shop.remove(&(stage, 5));
            return Some(Goal::Shop(s, at, "a performance shop (nitrous, for the purge's rating)"));
        }
    }
    game.refresh_event_triggers();
    let starts = game.career_world.event_starts.as_ref().map(|(_, s)| s.clone()).unwrap_or_default();
    // URL and sponsor races once open, else the normal ones in turn.
    let pick =
        starts.iter().find(|(e, _)| game.game_data.career.events[*e].category == career::Category::Url).or_else(|| starts.first());
    pick.map(|&(e, t)| Goal::Event(e, t.position))
}

/// The variants' random breaks in free roam: a quit to the main menu or
/// the game left (at each D-Day step once, else now and then).
fn explore_break(game: &mut Game, bot: &mut Bot) -> Result<bool> {
    let Some(p) = game.profile.as_ref() else { return Ok(false) };
    let step = match p.intro {
        Some(i) => format!("D-Day {i:08X}"),
        None if p.stage == 0 => "D-Day".to_string(),
        None => String::new(),
    };
    if bot.style.has("leave") && !step.is_empty() && !bot.left_at.contains(&step) {
        bot.left_at.insert(step.clone());
        leave(game, bot, &format!("at {step}"))?;
        return Ok(true);
    }
    if p.stage >= 1 && bot.style.chance("leave", 0.01) {
        leave(game, bot, "in free roam")?;
        return Ok(true);
    }
    if p.stage >= 1 && bot.style.chance("quit", 0.015) {
        quit_to_main(game, bot)?;
        let disk = bot.style.chance("quit", 0.5);
        resume(game, bot, disk, "quit in free roam")?;
        return Ok(true);
    }
    Ok(false)
}

fn explore_step(game: &mut Game, bot: &mut Bot) -> Result<()> {
    // A drive-in, drive-out or the career's start scene: Enter skips it.
    if game.scenes.shop.is_some() {
        bot.steps(game, 3)?;
        if game.scenes.shop.is_some() {
            bot.tap(game, KeyCode::Enter)?;
        }
        return Ok(());
    }
    // The D-Day script holds the car until its messages are read.
    if game.career_world.dday.running() {
        return bot.steps(game, 15);
    }
    // A check aid: NFSU2_PLAYTHROUGH_EVENT=<name> races that event once
    // the career's city is reached (its race code, not the career's).
    if let Ok(name) = std::env::var("NFSU2_PLAYTHROUGH_EVENT")
        && game.profile.as_ref().is_some_and(|p| p.stage >= 1)
        && bot.races == 0
        && let Some(e) = game.game_data.career.events.iter().position(|e| e.name == name)
    {
        bot.say(game, &format!("check aid: straight into {name}"));
        return game.start_race(e, false, false);
    }
    // A world challenge left to run out of time (variant `challenge`).
    if bot.challenge_wait && game.career_world.challenge.is_some_and(|c| c.state == 1) {
        bot.challenge_wait = false;
        bot.say(game, "world challenge: waiting for its clock to run out");
        for _ in 0..4000 {
            bot.step(game, 0.25)?;
            if !matches!(game.mode, Mode::Explore) || game.career_world.challenge.is_none_or(|c| c.state != 1) {
                break;
            }
        }
        bot.count("challenge timed out");
        return Ok(());
    }
    if explore_break(game, bot)? {
        return Ok(());
    }
    let Some(goal) = next_goal(game, bot) else {
        let p = game.profile.as_ref();
        if p.is_some_and(|p| p.stage >= 6) {
            return bot.steps(game, 30);
        }
        // The car driven (a variant's purchase or switch) does not reach
        // the stage's required DVD with the parts on sale: the Focus the
        // fixed path drives (which does) in its place at the car lot.
        if let Some(p) = p
            && p.car != "FOCUS"
            && bot.tried("rating trade", p.stage) < 2
            && let Some((s, at)) = open_shop(game, 4)
        {
            let want = game
                .game_data.career
                .milestones
                .iter()
                .filter(|m| m.stage == p.stage && m.required && !p.milestones.contains(&m.hash))
                .map(|m| format!("{} {:.2}", m.name, m.rating))
                .collect::<Vec<_>>();
            let f = format!(
                "stage {}: the {} tops out at rating {:.2} with the parts the bot finds on sale, short of the stage's required {want:?}; traded for a Focus",
                p.stage,
                p.car,
                game.visual_rating()
            );
            bot.finding(game, f);
            bot.lot_want = Some("FOCUS".into());
            bot.say(game, &format!("to the car lot {} (a car that reaches the rating)", game.game_data.career.shops[s].name));
            put(game, at);
            bot.steps(game, 3)?;
            anyhow::ensure!(game.engage(), "the car lot: the Engage key did nothing");
            return bot.step(game, DT);
        }
        anyhow::bail!(
            "nothing to do in free roam: no open event, shoot or shop goal (stage {:?}, rating {:.2})",
            p.map(|p| p.stage),
            game.visual_rating()
        );
    };
    let (target, at, what) = match goal {
        Goal::Outrun => return outrun(game, bot),
        Goal::Shop(s, at, what) => (MarkerTarget::Shop(s), at, format!("{what} {}", game.game_data.career.shops[s].name)),
        Goal::Shoot(h, at) => (
            MarkerTarget::Shoot(h),
            at,
            format!(
                "photo shoot {}",
                game.game_data.career.milestones.iter().find(|m| m.hash == h).map_or("?", |m| m.name.as_str())
            ),
        ),
        Goal::Event(e, at) => {
            bot.last_event = Some(e);
            let ev = &game.game_data.career.events[e];
            (
                MarkerTarget::Event(e),
                at,
                format!("event {} ({:?} {:?}, purse {})", ev.name, ev.category, ev.kind, ev.reward),
            )
        }
        Goal::Challenge(i, at) => {
            (MarkerTarget::Challenge(i), at, format!("world challenge {:08X}", game.game_data.career.mag_triggers[i].name))
        }
        Goal::ChallengeGoal(m, at) => (m, at, format!("the world challenge's goal {m:?}")),
    };
    bot.say(game, &format!("to {what} at ({:.0}, {:.0})", at.x, at.y));
    put(game, at);
    for _ in 0..3 {
        bot.step(game, DT)?;
        if !matches!(game.mode, Mode::Explore) {
            break;
        }
    }
    if !matches!(game.mode, Mode::Explore) {
        // A milestone engages on arrival; an urgent SMS may come up.
        return Ok(());
    }
    // Two open events on one start trigger: the first in the event table
    // takes it (0x527790 walks the table in order), the other waits.
    if let (MarkerTarget::Event(want), Some(MarkerTarget::Event(other))) = (target, game.career_world.marker)
        && other != want
        && game.marker_open(MarkerTarget::Event(other))
    {
        let line = format!(
            "{} shares its start with {}, which takes the trigger: that one first",
            game.game_data.career.events[want].name, game.game_data.career.events[other].name
        );
        bot.say(game, &line);
        bot.count("shared start");
        bot.last_event = Some(other);
        anyhow::ensure!(game.engage(), "{}: the Engage key did nothing", game.game_data.career.events[other].name);
        return bot.step(game, DT);
    }
    if game.career_world.marker != Some(target) {
        let name = |m: Option<MarkerTarget>| match m {
            Some(MarkerTarget::Event(e)) => game.game_data.career.events[e].name.clone(),
            Some(MarkerTarget::Shop(s)) => game.game_data.career.shops[s].name.clone(),
            other => format!("{other:?}"),
        };
        let near: Vec<String> = game
            .career_world.event_starts
            .as_ref()
            .map(|(_, s)| {
                s.iter()
                    .filter(|(_, t)| (t.position - at).length() < 40.0)
                    .map(|(e, t)| format!("{} at {:?}", game.game_data.career.events[*e].name, t.position))
                    .collect()
            })
            .unwrap_or_default();
        anyhow::bail!(
            "{what}: not current at its trigger {at:?} (marker {:?} = {}, trigger {:?}, open starts near: {near:?}, challenge {:?})",
            game.career_world.marker,
            name(game.career_world.marker),
            game.career_world.marker_trigger,
            game.career_world.challenge
        );
    }
    if !game.engage() {
        // A world challenge carried over a quit or a profile load (career
        // +0xC08 is kept in memory; the load does not clear it, 0x514530
        // does not save it) whose goal the loaded profile no longer
        // offers: nothing to engage; its clock runs out (the retry).
        if let (Some(c), MarkerTarget::Shoot(_) | MarkerTarget::Shop(_)) = (game.career_world.challenge, target)
            && c.state == 1
        {
            let f = format!(
                "{what}: the running world challenge's goal does not engage after a quit or load (its offer was not saved); waiting for its clock"
            );
            bot.finding(game, f);
            bot.challenge_wait = true;
            return bot.step(game, DT);
        }
        anyhow::bail!("{what}: the Engage key did nothing");
    }
    bot.step(game, DT)
}

/// An outrun (variant `outrun`): on the longest road of the free-roam
/// routes (as the `outrun` shot), the encounter manager's first car once
/// placed is lined up 20 m ahead and challenged with the Engage key; then
/// (a bot shortcut, as the `outrun_won` shot) it is held 30 m behind for
/// 1 s and dropped 320 m back for a win, or put 320 m ahead for a loss.
/// The end, its pay and its triggers are the game's own.
fn outrun(game: &mut Game, bot: &mut Bot) -> Result<()> {
    let before = game.profile.as_ref().map(|p| (p.cash, p.inbox.entries.len())).unwrap_or_default();
    if let (Some(e), Some(d)) = (game.sim.encounters.as_ref(), game.sim.drive.as_mut()) {
        let f = &e.routes.file;
        let pick = bot.style.below(4);
        let mut roads: Vec<_> = f.records.iter().enumerate().collect();
        roads.sort_by_key(|(_, r)| std::cmp::Reverse(r.points.len()));
        if let Some((ri, r)) = roads.get(pick).or(roads.first()) {
            let c = r.points.len() / 2;
            let dir = f.direction(*ri, c, false);
            d.place(r.points[c].position, dir.y.atan2(dir.x));
        }
    } else {
        bot.say(game, "outrun: no encounter manager in this free roam");
        return bot.step(game, DT);
    }
    game.career_world.marker = None;
    let mut placed = None;
    for _ in 0..30 * 90 {
        bot.step(game, DT)?;
        if !matches!(game.mode, Mode::Explore) {
            bot.say(game, &format!("outrun: left free roam for {} while waiting", flow_mode(&game.mode)));
            return Ok(());
        }
        if let Some(e) = game.sim.encounters.as_ref()
            && e.state == 3
            && let Some(i) = e.cars.iter().position(|c| c.active)
        {
            placed = Some(i);
            break;
        }
    }
    let Some(i) = placed else {
        let e = game.sim.encounters.as_ref().map(|e| (e.state, e.career, e.timer));
        bot.say(game, &format!("outrun: no encounter car came in 90 s (manager {e:?})"));
        return Ok(());
    };
    let (heading, at) = {
        let d = game.sim.drive.as_ref().unwrap();
        (d.state.heading(), d.state.position)
    };
    let fwd = Vec3::new(heading.cos(), heading.sin(), 0.0);
    for frame in 0..12 {
        if frame == 0 {
            let d = game.sim.drive.as_mut().unwrap();
            d.state = CarState::new(at, heading);
            d.state.velocity = fwd * 20.0;
            // Synthetic initial/test motion explicitly establishes its exported scalar.
            d.state.publish_forward_speed();
            let e = game.sim.encounters.as_mut().unwrap();
            e.cars[i].state = CarState::new(at + fwd * 20.0, heading);
            e.cars[i].state.velocity = fwd * 20.0;
            // Synthetic initial/test motion explicitly establishes its exported scalar.
            e.cars[i].state.publish_forward_speed();
        }
        if frame == 1 {
            // The Engage key (0x602BE0 code 0xBF, as `dispatch`): no
            // marker current, so the waiting encounter's challenge.
            if !game.engage()
                && let Some(e) = game.sim.encounters.as_mut()
            {
                e.engage();
            }
        }
        bot.step(game, DT)?;
    }
    if game.sim.encounters.as_ref().is_none_or(|e| e.outrun.is_none()) {
        let e = game.sim.encounters.as_ref().map(|e| (e.state, e.career, e.target));
        bot.say(game, &format!("outrun: the engage did not start one (manager {e:?})"));
        return Ok(());
    }
    let win = !bot.style.chance("lose", 0.35);
    bot.say(game, &format!("outrun with encounter car {i}: to be {}", if win { "won" } else { "lost" }));
    let mut ended = false;
    for frame in 0..30 * 60 {
        if game.sim.encounters.as_ref().is_none_or(|e| e.outrun.is_none()) {
            ended = true;
            break;
        }
        if !matches!(game.mode, Mode::Explore) {
            break;
        }
        if frame <= 30 {
            let d = game.sim.drive.as_ref().unwrap();
            let (at, heading, v) = (d.state.position, d.state.heading(), d.state.velocity);
            let dir = Vec3::new(heading.cos(), heading.sin(), 0.0);
            let off = match (win, frame < 30) {
                (true, true) => -30.0,
                (true, false) => -320.0,
                (false, _) => 320.0,
            };
            let e = game.sim.encounters.as_mut().unwrap();
            e.cars[i].state = CarState::new(at + dir * off, heading);
            e.cars[i].state.velocity = if frame < 30 { v } else { Vec3::ZERO };
            // Synthetic initial/test motion explicitly establishes its exported scalar.
            e.cars[i].state.publish_forward_speed();
        }
        bot.step(game, DT)?;
    }
    let result = game.sim.encounters.as_ref().and_then(|e| e.result);
    let after = game.profile.as_ref().map(|p| (p.cash, p.inbox.entries.len())).unwrap_or_default();
    bot.count(if result.is_some_and(|r| r.0) { "outrun won" } else { "outrun lost" });
    if !ended || result.is_none_or(|r| r.0 != win) {
        let f = format!(
            "outrun meant to be {} ended {:?} (ended {ended})",
            if win { "won" } else { "lost" },
            result.map(|r| r.0)
        );
        bot.finding(game, f);
    }
    bot.say(
        game,
        &format!(
            "outrun over: {:?}, bank {} → {}, messages {} → {}",
            result.map(|r| (r.0, r.2)),
            before.0,
            after.0,
            before.1,
            after.1
        ),
    );
    Ok(())
}

/// Drive the race: the countdown, then the car moved along the race's road.
fn race_step(game: &mut Game, bot: &mut Bot) -> Result<()> {
    if game.scenes.shop.is_some() {
        bot.steps(game, 3)?;
        if game.scenes.shop.is_some() {
            bot.tap(game, KeyCode::Enter)?;
        }
        return Ok(());
    }
    let Mode::Race(r) = &mut game.mode else { return Ok(()) };
    if r.replay.playback.is_some() {
        anyhow::bail!("a replay is playing");
    }
    let key = (r.event, r.round);
    if bot.race_key != Some(key) {
        bot.race_key = Some(key);
        bot.along = None;
        bot.race_frames = 0;
        bot.misses = 0;
        bot.careful = 0;
        bot.drive_to = None;
        let e = &game.game_data.career.events[r.event];
        let mut line = format!(
            "race {} round {} ({:?}, {} laps, {:.0} m road, {} rivals)",
            e.name,
            r.round,
            e.kind,
            r.laps,
            r.route.road.length,
            r.racers.rivals.len()
        );
        bot.races += 1;
        // The variants: lose it, or break off part way.
        let stage = game.profile.as_ref().map_or(0, |p| p.stage);
        bot.lose_this = bot.style.chance("lose", 0.12);
        bot.race_break = None;
        let at = 20 + bot.style.below(120) as u64;
        let pick = if bot.style.chance("restart", 0.06) {
            Some(RaceBreak::Restart)
        } else if bot.style.chance("quit", 0.04) {
            Some(RaceBreak::Quit)
        } else if bot.style.chance("leave", 0.03) {
            Some(RaceBreak::Leave)
        } else {
            None
        };
        if let Some(b) = pick
            && bot.tried("race break", stage) < 6
        {
            bot.race_break = Some((at, b));
            line += &format!(", {b:?} at update {at}");
        }
        if bot.lose_this {
            line += ", to be lost";
        }
        bot.say(game, &line);
    }
    let Mode::Race(r) = &mut game.mode else { return Ok(()) };
    bot.race_frames += 1;
    if bot.race_frames % 400 == 0 {
        let lines = race_lines(&r.route);
        let line = format!(
            "race still on: clock {:.1} along {:.0} of {:.0} (bot {:?}) fraction {:.4} lines {:?} (finish {}) start_at {:.0} finish_at {:.0} finished {:?} dropped {:?} lap {:?} pos {:?}",
            r.timing.clock,
            r.racers.me.along,
            r.route.road.length,
            bot.along,
            r.racers.me.locator.fraction,
            &lines.at[..lines.count],
            lines.finish,
            r.route.start_at,
            r.route.finish_at,
            r.racers.me.finished,
            r.racers.me.dropped,
            r.racers.me.lap(),
            game.sim.drive.as_ref().map(|d| d.state.position)
        );
        bot.say(game, &line);
        if bot.race_frames > 4000 {
            anyhow::bail!("the race does not end: {line}");
        }
    }
    let Mode::Race(r) = &mut game.mode else { return Ok(()) };
    // A planned break once the race runs.
    if let Some((at, what)) = bot.race_break
        && bot.race_frames >= at
        && r.timing.clock >= 0.0
        && r.racers.me.finished.is_none()
    {
        bot.race_break = None;
        bot.say(game, &format!("race break: {what:?}"));
        match what {
            RaceBreak::Restart => return pause_restart(game, bot),
            RaceBreak::Quit => {
                quit_to_main(game, bot)?;
                let disk = bot.style.chance("quit", 0.5);
                return resume(game, bot, disk, "quit mid-race");
            }
            RaceBreak::Leave => return leave(game, bot, "mid-race"),
        }
    }
    let Mode::Race(r) = &mut game.mode else { return Ok(()) };
    if r.timing.clock < 0.0 || r.racers.me.finished.is_some() {
        // The rivals drift on to the race's end: the bot's lead too.
        if r.racers.me.finished.is_some() && r.scoring.drift.is_some() && !bot.lose_this {
            drift_points(r);
        }
        return bot.step(game, DT);
    }
    // A race to lose: the car stands until a rival is home (a drift
    // race: no points added), then goes on as usual.
    if bot.lose_this && r.scoring.drift.is_none() && !r.timing.first_done && bot.race_frames < 2400 {
        return bot.step(game, 0.25);
    }
    if let Some(to) = bot.drive_to {
        if r.racers.me.progress() < to && bot.race_frames < bot.race_frames_limit() {
            return bot.step(game, DT);
        }
        r.autopilot = None;
        bot.drive_to = None;
        bot.along = None;
        bot.careful = 12;
    }
    // A drift race scores the car's drifts, which a teleport does not do:
    // the bot's points (a shortcut) put it ahead of the best rival.
    if r.scoring.drift.is_some() && !bot.lose_this {
        drift_points(r);
    }
    // 100 m on from the bot's own place; when the race's locator has
    // fallen behind it (a road that passes near itself), back to the
    // locator's place in shorter steps.
    let length = r.route.road.length.max(1.0);
    let mine = bot.along.unwrap_or(r.racers.me.along);
    let mut lag = mine - r.racers.me.along;
    if r.route.road.loops {
        lag = (lag + length * 0.5).rem_euclid(length) - length * 0.5;
    }
    // Where it disagrees (a hairpin, a road over itself), 20 m on from the
    // locator's place until they agree again.
    // Each failure in a row jumps further (past a corner the road's line
    // cuts through a wall).
    // After a miss it goes on in 20 m steps a while (where the road comes
    // back by itself, a hairpin, a long jump lands on the earlier stretch);
    // misses in a row jump further (a corner the road's line cuts through
    // a wall).
    // Three misses in a row: the car on the AI racer controller for the
    // next 150 m (real driving past the spot).
    if lag.abs() > 20.0 && bot.misses >= 3 {
        if let (Some(routes), Some(d)) = (r.racers.ai_routes.as_ref(), game.sim.drive.as_ref()) {
            r.autopilot = Some(Autopilot::new(d, routes, d.car.car_type.handling.race.drift()));
            bot.drive_to = Some(r.racers.me.progress() + 150.0 / length);
            bot.misses = 0;
            let line = format!("teleports missed at {:.0} m: the AI controller drives on", r.racers.me.along);
            bot.say(game, &line);
            return bot.step(game, DT);
        }
    }
    let s = if lag.abs() > 20.0 {
        bot.misses += 1;
        bot.careful = 12;
        r.racers.me.along + 20.0 * (1 << (bot.misses - 1).min(4)) as f32
    } else if bot.careful > 0 {
        bot.misses = 0;
        bot.careful -= 1;
        mine + 20.0
    } else {
        bot.misses = 0;
        mine + 100.0
    };
    bot.along = Some(s);
    // Past a point to point road's end the finish line may still lie
    // ahead (its marker located beyond the last point): straight on.
    let road = &r.route.road;
    let target = if !road.loops && s > road.length {
        road.point_at(road.length) + road.direction(road.length) * (s - road.length)
    } else {
        road.point_at(s)
    };
    let dir = r.route.road.direction(s);
    if let Some(d) = game.sim.drive.as_mut() {
        // On the road's surface there (searched from a little above, as the
        // road climbs), at the car's height over the road now.
        let at = d.state.position;
        let lift = d.world.road_height_below(at + Vec3::Z * 3.0).map_or(0.5, |h| (at.z - h).clamp(0.0, 2.0));
        let ground = d.world.road_height_below(target.extend(at.z + 25.0)).unwrap_or(at.z - lift);
        d.state.position = target.extend(ground + lift);
        let _ = dir;
        if std::env::var_os("NFSU2_PLAYTHROUGH_DEBUG").is_some() && bot.race_frames % 400 > 390 {
            println!(
                "teleport: from {at:?} lift {lift:.2} to {:?} (s {s:.0}, along {:.0}) ground {:?} high {:?}",
                d.state.position,
                r.racers.me.along,
                d.world.road_height_below(target.extend(at.z + 25.0)),
                d.world.road_height_below(target.extend(1.0e6))
            );
        }
    }
    let t = Instant::now();
    bot.step(game, 0.25)?;
    if std::env::var_os("NFSU2_PLAYTHROUGH_DEBUG").is_some() && bot.race_frames % 400 > 390 {
        let pos = game.sim.drive.as_ref().map(|d| d.state.position);
        if let Mode::Race(r) = &game.mode {
            println!("  after: {pos:?} along {:.0}", r.racers.me.along);
        }
    }
    if t.elapsed().as_secs_f32() > 0.5 {
        let pos = game.sim.drive.as_ref().map(|d| (d.state.position, d.state.velocity));
        let props = 0;
        bot.say(game, &format!("slow race update: {:.2} s, car {pos:?}, props {props}", t.elapsed().as_secs_f32()));
    }
    Ok(())
}

/// The race's pause (Escape), Restart, any question answered Yes.
fn pause_restart(game: &mut Game, bot: &mut Bot) -> Result<()> {
    bot.tap(game, KeyCode::Escape)?;
    let Some((kind @ MenuKind::RacePause, actions)) = icon_options(game) else {
        let f = format!("Escape in a race opened {} (not the pause)", flow_mode(&game.mode));
        bot.finding(game, f);
        return Ok(());
    };
    let i = actions
        .iter()
        .position(|a| matches!(a, Action::Restart))
        .with_context(|| format!("the race pause has no Restart: {actions:?}"))?;
    if let Mode::Menu(Menu::Icons(m, _)) = &mut game.mode {
        m.selected = i;
    }
    bot.steps(game, 6)?;
    game.act(Action::Restart, kind)?;
    for _ in 0..10 {
        match &game.mode {
            Mode::Menu(Menu::Dialog(..) | Menu::McMessage(..)) => answer_question(game, bot, true)?,
            _ => break,
        }
    }
    bot.race_key = None;
    bot.count("restart mid-race");
    anyhow::ensure!(matches!(game.mode, Mode::Race(_)), "Restart from the pause: in {}", flow_mode(&game.mode));
    Ok(())
}

fn drift_points(r: &mut Race) {
    let best = r.scoring.board.iter().filter(|e| e.racer != Some(0)).map(|e| e.score).fold(0.0f32, f32::max);
    // The drift trick's banked points (the board's and the total's).
    if let Some(t) = r.scoring.style.as_mut().and_then(|st| st.tricks.get_mut(nfsu2::drift::DRIFT)).and_then(Option::as_mut) {
        let want = best * 1.5 + 2000.0;
        if t.banked < want {
            t.banked = want;
        }
    }
}

fn icon_options(game: &Game) -> Option<(MenuKind, Vec<Action>)> {
    match &game.mode {
        Mode::Menu(Menu::Icons(m, k)) => Some((*k, m.options.iter().map(|o| o.action).collect())),
        _ => None,
    }
}

fn menu_step(game: &mut Game, bot: &mut Bot) -> Result<()> {
    let Mode::Menu(m) = &game.mode else { return Ok(()) };
    match m {
        Menu::Movie(mv) => {
            let names = mv.queue.join(",");
            if names.to_ascii_lowercase().contains("scene15") {
                bot.ending_seen = true;
            }
            bot.say(game, &format!("movie {names} (Enter skips it)"));
            bot.steps(game, 20)?;
            bot.tap(game, KeyCode::Enter)?;
            bot.steps(game, 3)
        }
        Menu::SmsViewer(_) | Menu::SmsMailbox(_) => {
            let unread = game.profile.as_ref().map_or(0, |p| p.inbox.unread());
            let name =
                |h: u32| game.game_data.career.messages.iter().find(|m| m.hash == h).map_or(format!("{h:08X}"), |m| m.name.clone());
            let top =
                game.profile.as_ref().and_then(|p| p.inbox.entries.last().map(|e| name(e.message))).unwrap_or_default();
            bot.say(game, &format!("SMS screens ({unread} unread, newest {top}): Back"));
            bot.menu(game, Key::Back)
        }
        Menu::Dialog(d, _) => {
            let text = d.text.replace('\n', " ");
            // The car lot's trade question: trade the car driven, or
            // browse the others (variant `cars`).
            let yes = !(d.buttons.len() == 2 && bot.browse && d.buttons[1].contains("Browse"));
            bot.say(
                game,
                &format!(
                    "dialog {text:?} {:?}: {}",
                    d.buttons,
                    if yes { d.buttons.first() } else { d.buttons.get(1) }.map_or("", |b| b.as_str())
                ),
            );
            answer_question(game, bot, yes)
        }
        Menu::McMessage(m, _) => {
            bot.say(game, &format!("memory card {:?} {:?}", m.text, m.buttons));
            answer_question(game, bot, true)
        }
        Menu::McMain(_) => bot.menu(game, Key::Accept),
        Menu::Engage(e) => {
            let ev = &game.game_data.career.events[e.event];
            bot.say(game, &format!("race dialog {} (purse {}): Accept", ev.name, e.purse));
            bot.menu(game, Key::Left)?;
            bot.menu(game, Key::Accept)
        }
        Menu::ShopEngage(e) => {
            bot.explored.clear();
            bot.shop_kind = Some(game.game_data.career.shops[e.shop].kind);
            let s = game.game_data.career.shops[e.shop].name.clone();
            bot.say(game, &format!("shop dialog {s}: Accept"));
            bot.menu(game, Key::Left)?;
            bot.menu(game, Key::Accept)
        }
        Menu::Challenge(c) => match c.kind {
            screens::ChallengeDialogKind::Milestone { milestone } => {
                bot.say(game, &format!("photo shoot dialog {:08X}: Continue", milestone));
                bot.shoots += 1;
                bot.menu(game, Key::Accept)
            }
            screens::ChallengeDialogKind::Offer { challenge, .. } if bot.style.has("challenge") => {
                // NFSU2_PLAYTHROUGH_SAVES: the save on disk kept when it
                // already offers the challenge (to take it in the original).
                // A unique-part challenge (kinds 1, 2: offered after an
                // outrun or a race, before any save point): saved first, as
                // the garage's Save would (a bot step, with the aid only).
                if std::env::var_os("NFSU2_PLAYTHROUGH_SAVES").is_some()
                    && matches!(game.game_data.career.mag_triggers[challenge].kind, 1 | 2)
                    && let Some(p) = game.profile.as_ref()
                {
                    let _ = p.save();
                }
                if let (Some(dir), Some(name), Some(ctx)) = (
                    std::env::var_os("NFSU2_PLAYTHROUGH_SAVES"),
                    game.profile.as_ref().map(|p| profiles::sanitize(&p.name)),
                    career::NativeContext::get(),
                ) && let Ok(raw) = profiles::read(&name)
                    && Profile::from_native(&raw, ctx).challenges_on_offer(&game.game_data.career, false).contains(&challenge)
                {
                    let to = std::path::Path::new(&dir).join(format!(
                        "challenge-{:08X}-kind{}.sav",
                        game.game_data.career.mag_triggers[challenge].name, game.game_data.career.mag_triggers[challenge].kind
                    ));
                    let _ = std::fs::create_dir_all(&dir);
                    if !to.exists() {
                        let _ = std::fs::copy(profiles::file(&name), &to);
                    }
                }
                bot.challenge_wait = bot.style.chance("challenge", 0.3);
                bot.say(
                    game,
                    &format!(
                        "world challenge {:08X} (kind {}): Accept{}",
                        game.game_data.career.mag_triggers[challenge].name,
                        game.game_data.career.mag_triggers[challenge].kind,
                        if bot.challenge_wait { ", to time out" } else { "" }
                    ),
                );
                bot.count("challenge taken");
                bot.menu(game, Key::Left)?;
                bot.menu(game, Key::Accept)
            }
            screens::ChallengeDialogKind::Retry if bot.style.chance("challenge", 0.5) => {
                bot.say(game, "world challenge out of time: Retry");
                bot.count("challenge retried");
                bot.menu(game, Key::Left)?;
                bot.menu(game, Key::Accept)
            }
            _ => {
                bot.say(game, "world challenge dialog: declined");
                bot.menu(game, Key::Back)
            }
        },
        Menu::Showcase(v) => {
            if v.mode == screens::ShowcaseMode::Menu {
                if v.option < 3 {
                    return bot.menu(game, Key::Down);
                }
                bot.say(game, "showcase: Take Picture");
            }
            bot.menu(game, Key::Accept)
        }
        Menu::Cover(_) => {
            bot.say(game, "cover: Accept");
            bot.menu(game, Key::Accept)
        }
        Menu::CarLot(_) => car_lot_step(game, bot),
        Menu::Paint(_) => paint_step(game, bot),
        // A performance package's brands (UI_PerformanceBrandSelect): as
        // offered, Accept.
        Menu::BrandSelect(..) => {
            bot.say(game, "brand select: Accept");
            bot.menu(game, Key::Accept)
        }
        Menu::Results(r, reward) => {
            let me = r.player.and_then(|i| r.rows.get(i)).map(|x| (x.0.clone(), x.2, x.3));
            bot.say(
                game,
                &format!(
                    "results {:?}: player {me:?}, place {:?}, reward {:?}",
                    r.kind,
                    r.player.map(|p| p + 1),
                    reward.as_ref().map(|w| (w.reputation, w.bank))
                ),
            );
            bot.menu(game, Key::Accept)
        }
        Menu::Reward(w) => {
            bot.say(game, &format!("reward: rep {} bank {}", w.reputation, w.bank));
            bot.menu(game, Key::Accept)
        }
        Menu::SponsorPopup(_) => {
            bot.say(game, "SELECT A SPONSOR!: Accept");
            bot.menu(game, Key::Accept)
        }
        Menu::Stats(_) => bot.menu(game, Key::Back),
        Menu::Parts(_) => shop_step(game, bot),
        Menu::Icons(..) => {
            let (kind, actions) = icon_options(game).unwrap();
            let pick = |want: &dyn Fn(&Action) -> bool| actions.iter().position(want);
            let choose = match kind {
                MenuKind::PostRace => {
                    if bot.race_key.take().is_some() && game.race_flow.next_round.is_none() {
                        match game.race_flow.result.as_ref().map(|r| r.won) {
                            Some(true) => bot.wins += 1,
                            _ => bot.losses += 1,
                        }
                    }
                    let lost = game.race_flow.result.as_ref().is_some_and(|r| !r.won);
                    let stage = game.profile.as_ref().map_or(0, |p| p.stage);
                    let quit = bot.style.chance("quit", 0.06);
                    let left = !quit && bot.style.chance("leave", 0.04);
                    if quit {
                        quit_to_main(game, bot)?;
                        let disk = bot.style.chance("quit", 0.5);
                        resume(game, bot, disk, "quit at post race")?;
                    } else if left {
                        leave(game, bot, "at post race")?;
                    }
                    if quit || left {
                        if game.profile.as_ref().is_some_and(|p| p.stage >= 6) && !bot.ending_seen {
                            bot.ending_skipped = true;
                            bot.finding(game, "the final race's Post Race left by a quit or the game left: no Scene15 (it is queued only by Post Race's Return to Explore, 0x54C9E0 → 0x513A50)".into());
                        }
                        return Ok(());
                    }
                    if lost && bot.style.chance("restart", 0.4) && bot.tried("post race restart", stage) < 10 {
                        bot.count("restart after a loss");
                        pick(&|a| matches!(a, Action::Restart))
                    } else {
                        pick(&|a| matches!(a, Action::NextRace))
                            .or_else(|| pick(&|a| matches!(a, Action::ReturnToExplore)))
                    }
                }
                MenuKind::Sponsor => {
                    let signs: Vec<usize> =
                        (0..actions.len()).filter(|&i| matches!(actions[i], Action::Sign(_))).collect();
                    if bot.style.has("sponsor") && !signs.is_empty() {
                        bot.count("sponsor drawn");
                        Some(signs[bot.style.below(signs.len())])
                    } else {
                        Some(0)
                    }
                }
                // A performance shop (variant `perf`): a category, a
                // package bought, then out.
                MenuKind::PerfCategory => {
                    let open: Vec<usize> =
                        (0..actions.len()).filter(|&i| matches!(actions[i], Action::PerfCategory(_))).collect();
                    let nitrous = actions.iter().position(|a| matches!(a, Action::PerfCategory(2)));
                    if bot.want_nitrous && nitrous.is_some() && bot.explored.insert("perf/category".into()) {
                        nitrous
                    } else if !open.is_empty() && bot.explored.insert("perf/category".into()) {
                        Some(open[bot.style.below(open.len())])
                    } else {
                        return bot.menu(game, Key::Back);
                    }
                }
                MenuKind::PerfPackage => {
                    let open: Vec<usize> =
                        (0..actions.len()).filter(|&i| matches!(actions[i], Action::PerfPackage(_))).collect();
                    if !open.is_empty() && bot.explored.insert("perf/package".into()) {
                        bot.count("performance package");
                        if std::mem::take(&mut bot.want_nitrous) {
                            // The best package on sale.
                            open.last().copied()
                        } else {
                            Some(open[bot.style.below(open.len())])
                        }
                    } else {
                        return bot.menu(game, Key::Back);
                    }
                }
                // The graphics shop (UI_ChoosePaintCategory): Vinyls once
                // a visit, Purchase Setup while the setup holds any, then
                // Back (its exit question answered Yes).
                MenuKind::Graphics => {
                    if bot.style.has("paint") && bot.explored.insert("graphics/paint".into()) {
                        pick(&|a| matches!(a, Action::Paint))
                    } else if !game.shop.setup.is_empty() {
                        bot.explored.insert("graphics/bought".into());
                        pick(&|a| matches!(a, Action::PurchaseSetup))
                    } else if bot.explored.insert("graphics/vinyls".into()) {
                        pick(&|a| matches!(a, Action::Vinyls))
                    } else {
                        if !bot.explored.contains("graphics/bought") {
                            let p = game.profile.as_ref().context("profile")?;
                            bot.say(
                                game,
                                &format!(
                                    "graphics shop: nothing more to fit (rating {:.2}, bank {}): leave",
                                    game.visual_rating(),
                                    p.cash
                                ),
                            );
                            bot.fruitless_shop.insert((p.stage, 1), (p.cash, unlocked_levels(game, p)));
                        }
                        return bot.menu(game, Key::Back);
                    }
                }
                // UI_ChooseVinylLayer: each layer without a vinyl in turn
                // (into the vinyl browser), then Back.
                MenuKind::VinylLayer => {
                    let unique = game.fitted_unique() || game.setup_unique();
                    let key = |i: usize| format!("{kind:?}/{i}");
                    let free = |i: usize| match actions[i] {
                        Action::VinylLayer(l) => {
                            !unique && !game.layer_has_vinyl(l) && game.shop.setup.get(l).is_none_or(|h| h == 0)
                        }
                        _ => false,
                    };
                    match (0..actions.len()).find(|&i| free(i) && !bot.explored.contains(&key(i))) {
                        Some(i) => {
                            bot.explored.insert(key(i));
                            Some(i)
                        }
                        None => return bot.menu(game, Key::Back),
                    }
                }
                MenuKind::Crib if std::mem::take(&mut bot.want_switch) => pick(&|a| matches!(a, Action::CarSelect)),
                MenuKind::Crib => pick(&|a| matches!(a, Action::Explore)),
                // A shop's own option menus (rim brands): each open
                // option in turn, then Back.
                MenuKind::RimBrand => {
                    let key = |i: usize| format!("{kind:?}/{i}");
                    match (0..actions.len())
                        .find(|&i| !matches!(actions[i], Action::Unavailable) && !bot.explored.contains(&key(i)))
                    {
                        Some(i) => {
                            bot.explored.insert(key(i));
                            Some(i)
                        }
                        None => return bot.menu(game, Key::Back),
                    }
                }
                _ => None,
            };
            let Some(i) = choose else { anyhow::bail!("icon menu {kind:?} with {actions:?}: no choice") };
            bot.say(game, &format!("menu {kind:?}: {:?}", actions[i]));
            if let Mode::Menu(Menu::Icons(m, _)) = &mut game.mode {
                m.selected = i;
            }
            let t = Instant::now();
            bot.steps(game, 6)?;
            let t1 = t.elapsed().as_secs_f32();
            game.act(actions[i], kind)?;
            let t2 = t.elapsed().as_secs_f32();
            bot.step(game, DT)?;
            if std::env::var_os("NFSU2_PLAYTHROUGH_DEBUG").is_some() {
                println!(
                    "timing {kind:?}: 6 frames {t1:.3} s, act {:.3} s, a frame after {:.3} s",
                    t2 - t1,
                    t.elapsed().as_secs_f32() - t2
                );
            }
            Ok(())
        }
        other => anyhow::bail!(
            "a screen the bot does not know: {}",
            match other {
                Menu::LotDeal(_) => "LotDeal",
                Menu::WorldMap(_) => "WorldMap",
                Menu::InGameMap(_) => "InGameMap",
                Menu::Paint(_) => "Paint",
                Menu::Status(_) => "Status",
                Menu::Credits(_) => "Credits",
                Menu::Splash(_) => "Splash",
                Menu::BrandSelect(..) => "BrandSelect",
                _ => "other",
            }
        ),
    }
}

/// The car lots (variant `cars`, else the D-Day lot's second car): a
/// stock car into the free slot, a trade-in (of the car driven, or one
/// of the others browsed), or at the garage's Car Select another car.
fn car_lot_step(game: &mut Game, bot: &mut Bot) -> Result<()> {
    let Mode::Menu(Menu::CarLot(lot)) = &game.mode else { return Ok(()) };
    let cars: Vec<_> = lot.cars.iter().map(|c| c.name.clone()).collect();
    let (state, n, at) = (lot.state, lot.cars.len(), lot.selected);
    let wanted = bot.lot_want.is_some();
    let p = game.profile.as_ref().context("profile")?;
    let target = match state {
        LotState::CribSelect => {
            let others: Vec<usize> = (0..n)
                .filter(|&i| lot.cars[i].placeholder.is_none() && lot.cars[i].slot.is_some_and(|s| s != p.slot))
                .collect();
            if others.is_empty() {
                bot.say(game, &format!("car select {cars:?}: no other car, Back"));
                return bot.menu(game, Key::Back);
            }
            bot.count("car switched");
            others[bot.style.below(others.len())]
        }
        LotState::Browse => {
            let mine: Vec<usize> = (0..n).filter(|&i| lot.cars[i].slot.is_some()).collect();
            bot.count("car traded (browsed)");
            mine[bot.style.below(mine.len())]
        }
        _ if bot.lot_want.is_some() => {
            let want = bot.lot_want.take().unwrap();
            lot.cars.iter().position(|c| c.name == want).unwrap_or(0)
        }
        _ if p.stage == 0 && !bot.style.has("car") => 1.min(n - 1),
        _ => {
            if p.stage > 0 {
                bot.count(if state == LotState::Take { "car bought" } else { "car traded" });
            }
            // Not the car driven now (a trade of a car for the same).
            let pool: Vec<usize> = (0..n).filter(|&i| lot.cars[i].name != p.car || n == 1).collect();
            pool[bot.style.below(pool.len())]
        }
    };
    bot.browse = state == LotState::TradeOrBrowse && !wanted && bot.style.chance("cars", 0.5);
    bot.say(game, &format!("car lot {state:?} {cars:?}: {}, Accept", cars.get(target).map_or("?", |c| c.as_str())));
    for _ in at..target {
        bot.menu(game, Key::Right)?;
    }
    for _ in target..at {
        bot.menu(game, Key::Left)?;
    }
    bot.menu(game, Key::Accept)
}

/// UI_Paint (variant `paint`): a slot's swatches, a random swatch
/// installed into the setup, then Back to the graphics shop (which buys
/// the setup).
fn paint_step(game: &mut Game, bot: &mut Bot) -> Result<()> {
    let Mode::Menu(Menu::Paint(p)) = &game.mode else { return Ok(()) };
    let (vinyl, selected, slots, swatches) = (p.vinyl.is_some(), p.selected, p.slots().len(), p.swatches.len());
    if vinyl {
        return bot.menu(game, Key::Back);
    }
    match selected {
        None if bot.explored.insert("paint/slot".into()) => {
            let k = bot.style.below(slots.min(3));
            bot.say(game, &format!("paint: slot {k} of {slots}"));
            for _ in 0..k {
                bot.menu(game, Key::Right)?;
            }
            bot.menu(game, Key::Accept)
        }
        Some(_) if bot.explored.insert("paint/swatch".into()) && swatches > 0 => {
            let k = bot.style.below(swatches.min(20));
            bot.say(game, &format!("paint: swatch {k} of {swatches}, Install"));
            for _ in 0..k {
                bot.menu(game, Key::Right)?;
            }
            bot.count("paint installed");
            bot.menu(game, Key::Accept)
        }
        _ => bot.menu(game, Key::Back),
    }
}

/// In a shop's parts browser: fit the part that raises the visual rating
/// most (the cheaper on a tie) through the browser's keys; else open the
/// next category that opens its own (rims, neon, spinners, trunk audio);
/// else Back (out of the shop at its top).
fn shop_step(game: &mut Game, bot: &mut Bot) -> Result<()> {
    let Mode::Menu(Menu::Parts(b)) = &game.mode else { return Ok(()) };
    // UI_ChooseUniquePart (a world challenge's reward): one option, Accept.
    if b.package == "UI_ChooseUniquePart" {
        let (n, at) = (b.categories.len(), b.category);
        if n == 0 {
            bot.say(game, "unique parts: none listed, Back");
            return bot.menu(game, Key::Back);
        }
        let k = bot.style.below(n);
        bot.say(game, &format!("unique parts: {n} listed, option {k}"));
        for _ in at..k {
            bot.menu(game, Key::Right)?;
        }
        for _ in k..at {
            bot.menu(game, Key::Left)?;
        }
        bot.count("unique part taken");
        return bot.menu(game, Key::Accept);
    }
    // The game left in a shop now and then (variant `leave`).
    if bot.style.chance("leave", 0.003) {
        return leave(game, bot, "in a shop");
    }
    if b.part.is_some() {
        return bot.menu(game, Key::Back);
    }
    let (package, sub, categories, at_category) = (b.package, b.sub, b.categories.clone(), b.category);
    // The graphics shop's vinyl browser (a layer's): a vinyl goes into the
    // setup (bought by Purchase Setup), its price on top of the setup's.
    let vinyls = sub.is_some_and(|l| (0x40..=0x43).contains(&l)) && b.title == 0xB6AA_ECF7;
    let setup_total = if vinyls { game.graphics_total().unwrap_or(0) } else { 0 };
    let p = game.profile.as_ref().context("profile")?;
    let (cash, stage) = (p.cash as i32, p.stage);
    let now = game.visual_rating();
    let mut best: Option<(usize, usize, f32, i32)> = None;
    // The carbon fibre trunk and kits fit on Accept (a toggle): its part
    // stands as part `usize::MAX`.
    for (ci, c) in categories.iter().enumerate() {
        let screens::CategoryAction::Toggle(fit) = c.action else { continue };
        if c.locked || p.installed.get(&c.slot) == Some(&fit) {
            continue;
        }
        let gain = rating_gain(game, p, now, c.slot, fit);
        let level = game.game_data.parts.find(&p.car, c.slot as usize, Some(fit), None).map_or(3, |x| x.level);
        let cost = if p.owned.contains(&(c.slot, fit)) {
            0
        } else {
            game.game_data.career.costs.price(nfsu2::formats::parts::SLOT_CATEGORIES[c.slot as usize], level)
        };
        if gain > 1e-4 && cost <= cash && best.is_none_or(|(_, _, g, _)| gain > g + 1e-4) {
            best = Some((ci, usize::MAX, gain, cost));
        }
    }
    for (ci, c) in categories.iter().enumerate() {
        // Custom gauges take colours first (UI_ChooseCustomHUD): skipped.
        if c.locked
            || c.action != screens::CategoryAction::Parts
            || c.slot == 0xA3
            || bot.explored.contains(&format!("skip/{package}/{sub:?}/{}", c.slot))
        {
            continue;
        }
        for (pi, part) in c.parts.iter().enumerate() {
            let (hash, _, price, owned, installed) = part.clone();
            if installed || (!owned && price + setup_total > cash) {
                continue;
            }
            // Vinyls: the plain ones (no colours to pick, REMAP 0), not
            // the unique ones (which take all four layers).
            if vinyls
                && (game.unique_vinyl(hash) || game.part_attr(0x40, hash, graphics::REMAP).is_some_and(|r| r != 0))
            {
                continue;
            }
            let gain = rating_gain(game, p, now, c.slot, hash);
            let cost = if owned { 0 } else { price };
            if gain > 1e-4
                && best.is_none_or(|(_, _, g, pr)| gain > g + 1e-4 || ((gain - g).abs() <= 1e-4 && cost < pr))
            {
                best = Some((ci, pi, gain, cost));
            }
        }
    }
    let goto = |game: &mut Game, bot: &mut Bot, ci: usize| -> Result<()> {
        for _ in ci..at_category {
            bot.menu(game, Key::Left)?;
        }
        for _ in at_category..ci {
            bot.menu(game, Key::Right)?;
        }
        Ok(())
    };
    let Some((ci, pi, gain, cost)) = best else {
        // A category with its own screen not yet looked into.
        let key = |ci: usize| format!("{package}/{sub:?}/{ci}");
        if let Some(ci) = (0..categories.len()).find(|&ci| {
            !categories[ci].locked
                && categories[ci].action == screens::CategoryAction::Open
                && !bot.explored.contains(&key(ci))
        }) {
            bot.explored.insert(key(ci));
            bot.say(game, &format!("{package}: into category slot {:#x}", categories[ci].slot));
            goto(game, bot, ci)?;
            return bot.menu(game, Key::Accept);
        }
        if sub.is_none() && matches!(package, "UI_GenericParts_Browser" | "UI_ICEMAIN") {
            bot.say(game, &format!("{package}: nothing more to fit (rating {now:.2}, bank {cash}): leave"));
            if let Some(kind) = bot.shop_kind {
                bot.fruitless_shop.insert(
                    (stage, kind),
                    (cash as u32, unlocked_levels(game, game.profile.as_ref().context("profile")?)),
                );
            }
        }
        return bot.menu(game, Key::Back);
    };
    let c = &categories[ci];
    if pi == usize::MAX {
        bot.say(game, &format!("{package}: toggle slot {:#x} on for {cost} (rating {now:.2} + {gain:.2})", c.slot));
        goto(game, bot, ci)?;
        bot.menu(game, Key::Accept)?;
        // The convert question (0x5664A0 at 0x566686 / 0x566895): Yes.
        if let Mode::Menu(Menu::Dialog(d, _)) = &game.mode {
            bot.say(game, &format!("{package} dialog {:?}", d.text));
            answer_question(game, bot, true)?;
        }
        bot.shops += 1;
        bot.say(game, &format!("{package}: rating now {:.2}", game.visual_rating()));
        return Ok(());
    }
    bot.say(
        game,
        &format!("{package}: fit {} in slot {:#x} for {cost} (rating {now:.2} + {gain:.2})", c.parts[pi].1, c.slot),
    );
    goto(game, bot, ci)?;
    bot.menu(game, Key::Accept)?;
    let at = match &game.mode {
        Mode::Menu(Menu::Parts(b2)) if b2.part.is_some() => b2.part.unwrap(),
        Mode::Menu(Menu::Dialog(d, _)) => {
            // The category refuses first (NOS purge without nitrous): not
            // tried again this visit.
            let text = d.text.clone();
            bot.say(game, &format!("{package}: the category answers {text:?}; skipped"));
            bot.explored.insert(format!("skip/{package}/{sub:?}/{}", categories[ci].slot));
            return Ok(());
        }
        _ => anyhow::bail!("{package}: no part list after Accept (in {})", flow_mode(&game.mode)),
    };
    for _ in pi..at {
        bot.menu(game, Key::Up)?;
    }
    for _ in at..pi {
        bot.menu(game, Key::Down)?;
    }
    let installed_before = game.profile.as_ref().map(|p| p.installed.clone());
    bot.menu(game, Key::Accept)?;
    for _ in 0..3 {
        let Mode::Menu(Menu::Dialog(d, _)) = &game.mode else { break };
        bot.say(game, &format!("{package} dialog {:?}", d.text));
        answer_question(game, bot, true)?;
    }
    bot.shops += 1;
    if vinyls {
        // Into the setup, back on the layers: bought at Purchase Setup.
        anyhow::ensure!(
            game.shop.setup.get(categories[ci].slot) == Some(categories[ci].parts[pi].0),
            "{package}: the vinyl {} did not go into the setup (in {})",
            categories[ci].parts[pi].1,
            flow_mode(&game.mode)
        );
        return Ok(());
    }
    if game.profile.as_ref().map(|p| p.installed.clone()) == installed_before {
        bot.blockers.push(format!("{package}: the part {} was not fitted", categories[ci].parts[pi].1));
        if let Some(kind) = bot.shop_kind {
            bot.fruitless_shop
                .insert((stage, kind), (cash as u32, unlocked_levels(game, game.profile.as_ref().context("profile")?)));
        }
        return bot.menu(game, Key::Back);
    }
    bot.say(game, &format!("{package}: rating now {:.2}", game.visual_rating()));
    Ok(())
}

/// `--playthrough-all [seeds]`: each seed's `--playthrough` as a child
/// process (its own save folder), `NFSU2_PLAYTHROUGH_JOBS` at once, its
/// log in `NFSU2_PLAYTHROUGH_DIR`; a summary line per seed, an error when
/// any did not pass.
pub(crate) fn playthrough_all(seeds: Option<&str>) -> Result<()> {
    let seeds: Vec<u64> = seeds.unwrap_or(ALL_SEEDS).split(',').filter_map(|s| s.trim().parse().ok()).collect();
    anyhow::ensure!(!seeds.is_empty(), "--playthrough-all: no seeds");
    let dir = std::env::var_os("NFSU2_PLAYTHROUGH_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("nfsu2-playthrough-all-{}", nfsu2::unique_tag())));
    std::fs::create_dir_all(&dir)?;
    let jobs = std::env::var("NFSU2_PLAYTHROUGH_JOBS").ok().and_then(|j| j.parse::<usize>().ok()).unwrap_or(4).max(1);
    let exe = std::env::current_exe()?;
    let t0 = Instant::now();
    let mut queue = seeds.iter().copied();
    let mut running: Vec<(u64, std::process::Child, Instant)> = Vec::new();
    let mut done: Vec<(u64, bool, f32, String)> = Vec::new();
    loop {
        while running.len() < jobs
            && let Some(seed) = queue.next()
        {
            let log = std::fs::File::create(dir.join(format!("seed-{seed}.log")))?;
            let child = std::process::Command::new(&exe)
                .arg("--playthrough")
                .env("NFSU2_PLAYTHROUGH_SEED", seed.to_string())
                .env_remove("NFSU2_PLAYTHROUGH_LOG")
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?;
            println!("seed {seed}: started");
            running.push((seed, child, Instant::now()));
        }
        if running.is_empty() {
            break;
        }
        let mut i = 0;
        while i < running.len() {
            if let Some(status) = running[i].1.try_wait()? {
                let (seed, _, t) = running.remove(i);
                let log = std::fs::read_to_string(dir.join(format!("seed-{seed}.log"))).unwrap_or_default();
                let last = log
                    .lines()
                    .rev()
                    .find(|l| l.starts_with("playthrough passed") || l.starts_with("Error") || l.contains("summary:"))
                    .unwrap_or("")
                    .to_string();
                let ok = status.success() && log.lines().any(|l| l.starts_with("playthrough passed"));
                println!(
                    "seed {seed}: {} in {:.0} s: {last}",
                    if ok { "passed" } else { "FAILED" },
                    t.elapsed().as_secs_f32()
                );
                done.push((seed, ok, t.elapsed().as_secs_f32(), last));
            } else {
                i += 1;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    done.sort_by_key(|d| d.0);
    let failed: Vec<u64> = done.iter().filter(|d| !d.1).map(|d| d.0).collect();
    println!(
        "playthrough-all: {} seeds in {:.0} s, {} passed, failed {failed:?}; logs in {}",
        done.len(),
        t0.elapsed().as_secs_f32(),
        done.len() - failed.len(),
        dir.display()
    );
    anyhow::ensure!(failed.is_empty(), "seeds {failed:?} did not pass");
    Ok(())
}
