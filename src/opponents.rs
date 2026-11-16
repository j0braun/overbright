//! A race's opponents: their cars and how good they are, as 0x5260C0 sets
//! them up from the event's opponent list (+0x40, +0x7C of them).
//!
//! Each opponent's skill (0–10) comes from its entry's source byte: 0 the
//! fixed skill byte, 3 the stage's range by career progress (0x5132E0),
//! otherwise the player's car's rating (0x5B69A0 on the player's
//! performance object). A "RANDOM" car is drawn by 0x4FEAF0 against that
//! skill; any other is the named car type.

use crate::formats::bundle::{chunks, read_f32, walk};
use crate::frontend::career::Event;

/// The game's random numbers (state 0x7EA624, statically 0x12345678).
/// At boot 0x57ECC0 (from the start-up 0x57ED20 chain at 0x57ED5C)
/// reseeds it through 0x43E6B0 with 0x43BDF0() + 0x12345678 —
/// QueryPerformanceCounter (IAT 0x78311C) shifted by 0x8284E4 — unless a
/// replay or online game (0x8650CC / 0x8650D0) supplies the seed. So
/// every boot of the original draws other rivals, names, grid slots and
/// cover choices (**verified** under Wine: three boots, no state within
/// 2·10⁶ steps of 0x12345678 at the main menu). The window seeds with
/// [`GameRng::boot`]; headless runs (shots, tests, flows) keep the static
/// value so they stay reproducible (check aid, not native).
#[derive(Clone, Debug)]
pub struct GameRng(pub u32);

impl Default for GameRng {
    fn default() -> GameRng {
        GameRng(0x1234_5678)
    }
}

impl GameRng {
    /// 0x43E6B0(seed, &state): the state one step past `seed`.
    pub fn seeded(seed: u32) -> GameRng {
        let mut r = GameRng(seed);
        r.advance();
        r
    }

    /// The boot seed (0x57ECC0): a high-resolution clock reading plus
    /// 0x12345678.
    pub fn boot(ticks: u64) -> GameRng {
        GameRng::seeded((ticks as u32).wrapping_add(0x1234_5678))
    }

    fn advance(&mut self) {
        let a = self.0 ^ 0x1D87_2B41;
        let b = (a >> 5) ^ a;
        self.0 = (b << 27) ^ b ^ a;
    }

    /// 0x43C1C0: the state modulo `n`, then a step.
    #[cfg_attr(feature = "dev-harness", track_caller)]
    pub fn below(&mut self, n: u32) -> u32 {
        #[cfg(feature = "dev-harness")]
        draw_log::note(self, 'I', n as f32);
        let r = self.0 % n.max(1);
        self.advance();
        r
    }

    /// 0x43C1F0: the state modulo 0x7FFFFFFF as a fraction of 2³¹, times
    /// `scale`, then a step.
    #[cfg_attr(feature = "dev-harness", track_caller)]
    pub fn unit(&mut self, scale: f32) -> f32 {
        #[cfg(feature = "dev-harness")]
        draw_log::note(self, 'F', scale);
        let r = self.0 % 0x7FFF_FFFF;
        self.advance();
        r as f32 * (1.0 / 2_147_483_648.0) * scale
    }
}

/// Developer harness only (unit RNG-T): with `NFSU2_RNG_LOG=<file>` every draw on a
/// [`GameRng`] appends `seq <TAB> generator address <TAB> I|F <TAB> argument <TAB> state before
/// <TAB> file:line` (the caller, through `#[track_caller]`), so the draws per call site and per
/// generator instance (the shared `Game::rng`, a race's `CarField::rng`, an encounter or
/// traffic stream) can be set beside the original's per-site counts
/// (tools/parity/rngsites.py). Off (one relaxed load) without the variable.
#[cfg(feature = "dev-harness")]
pub mod draw_log {
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};

    static LOG: OnceLock<Option<Mutex<(u64, std::fs::File)>>> = OnceLock::new();

    #[track_caller]
    pub(super) fn note(rng: &super::GameRng, kind: char, arg: f32) {
        let Some(log) = LOG.get_or_init(|| {
            let path = std::env::var_os("NFSU2_RNG_LOG")?;
            Some(Mutex::new((0, std::fs::File::create(path).ok()?)))
        }) else {
            return;
        };
        let at = std::panic::Location::caller();
        let mut g = log.lock().unwrap_or_else(|e| e.into_inner());
        g.0 += 1;
        let seq = g.0;
        // One unbuffered write a draw: nothing is lost when the process exits.
        let line = format!("{seq}\t{:x}\t{kind}\t{arg}\t{:08x}\t{}:{}\n", rng as *const _ as usize, rng.0, at.file(), at.line());
        let _ = g.1.write_all(line.as_bytes());
    }
}

/// The performance bounds the game copies from GLOBALB chunk 0x34A32 to
/// 0x86B3D0 (30 floats; the loader 0x599470 takes it from the 0x80034A30
/// container). +0x28 / +0x2C bound the performance rating, +0x70 / +0x74
/// a second range 0x5260C0 spreads by skill.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub rating: (f32, f32),
    pub spread: (f32, f32),
    /// The handling score's normalisers (0x5B54A0), by measure: 0 +0x38/+0x3C,
    /// 1 +0x40/+0x44, 2 +0x30/+0x34, 3 +0x48/+0x4C, 4 unused, 5 +0x50/+0x54,
    /// 6 +0x58/+0x5C, 7 +0x60/+0x64, 8 +0x68/+0x6C.
    pub score: [(f32, f32); 9],
}

impl Bounds {
    pub fn parse(globalb: &[u8]) -> Option<Bounds> {
        let mut out = None;
        walk(chunks(globalb, 0), &mut |c| {
            if c.id == 0x34A32 && c.body.len() >= 0x78 {
                let f = |o| read_f32(c.body, o);
                let pair = |o: usize| (f(o), f(o + 4));
                out = Some(Bounds {
                    rating: pair(0x28),
                    spread: pair(0x70),
                    score: [pair(0x38), pair(0x40), pair(0x30), pair(0x48), (0.0, 1.0), pair(0x50), pair(0x58), pair(0x60), pair(0x68)],
                });
            }
        });
        out
    }

    /// A rating on the 0–10 scale skills use (0x5B69A0 with its scaling
    /// flag): its place between the bounds, times 10.
    pub fn scale(&self, rating: f32) -> f32 {
        (rating - self.rating.0) / (self.rating.1 - self.rating.0) * 10.0
    }
}

/// The pools of car type indices random opponents come from (0x4FEAF0):
/// the SUVs (0x7F6DDC) when the event is an SUV event (+0x0D = 1), else
/// 0x7F6DC0 — 0x570A40 returns 0x7FA73C, which is 6 and never written, so
/// the game never takes the first pool (0x7F6DA4).
pub fn car_pool(suv_event: bool) -> &'static [u8] {
    if suv_event { &crate::frontend::career::OPPONENT_SUVS } else { &crate::frontend::career::OPPONENT_CARS_ALT }
}

/// Draw a random opponent's car (0x4FEAF0): the pool is shuffled (each
/// place swapped with a random one, 0x43C1C0), then, with a margin that
/// starts at 0.06 and doubles each pass, the first car (other than the
/// player's unless `allow_player`) whose scaled rating is at most the skill
/// plus the margin. `rating` gives a car type's scaled rating.
pub fn pick_car(rng: &mut GameRng, pool: &[u8], player_car: usize, allow_player: bool, skill: f32, rating: &dyn Fn(usize) -> f32) -> Option<u8> {
    if pool.is_empty() || (!allow_player && pool.iter().all(|&c| c as usize == player_car)) {
        return None;
    }
    let mut cars = pool.to_vec();
    let n = cars.len() as u32;
    for i in 0..cars.len() {
        let j = rng.below(n) as usize;
        cars.swap(i, j);
    }
    let mut margin = 0.03f32;
    loop {
        margin += margin;
        for &car in &cars {
            if !allow_player && car as usize == player_car {
                continue;
            }
            if rating(car as usize) <= skill + margin {
                return Some(car);
            }
        }
    }
}

/// An opponent's skill (0–10) by its entry's source (0x5260C0): 0 the
/// entry's skill byte, 3 the stage's range by progress (0x5132E0),
/// otherwise the player's car's scaled rating.
pub fn skill(source: u8, fixed: u8, stage: Option<StageSkill>, player_rating: f32) -> f32 {
    match (source, stage) {
        (0, _) => fixed as i8 as f32,
        (3, Some(s)) => s.at(),
        _ => player_rating,
    }
}

/// A stage's opponent skill range (stage definition +0x42 events, +0x44 /
/// +0x48 skills) and how far through it the career is (0x5132E0: the
/// count of the stage's completed entries).
#[derive(Clone, Copy, Debug)]
pub struct StageSkill {
    pub events: i16,
    pub low: f32,
    pub high: f32,
    pub done: u32,
}

impl StageSkill {
    /// The skill: from low to high by done / events while done ≤ events
    /// (and events is not 0), else high.
    pub fn at(&self) -> f32 {
        let events = self.events as f32;
        if self.done as f32 <= events && events != 0.0 {
            (self.high - self.low) * (self.done as f32 / events) + self.low
        } else {
            self.high
        }
    }
}

/// One opponent as 0x5260C0 lines it up.
#[derive(Clone, Debug)]
pub struct Entrant {
    pub name: String,
    /// Car type index.
    pub car: usize,
    /// Skill 0–10.
    pub skill: f32,
    /// The entry's skill source (+0x02).
    pub source: u8,
    /// The preset an entry's car names (any car but RANDOM: 0x637280 by
    /// the hash, GLOBALB 0x30220), whose parts it takes; a RANDOM car is
    /// dressed by 0x6399E0(1, 1) instead (`ai_looks`).
    pub preset: Option<u32>,
}

/// The event's opponents in entry order (0x5260C0): each one's skill
/// ([`skill`]) and car — a RANDOM one drawn by [`pick_car`] from the
/// event's pool with the player's car allowed, otherwise the preset whose
/// name hashes to the entry's car (0x637280: the boss cars CALEB_GTO,
/// NIKKI_MUSTANGGT, …), `car_by_hash` giving its car type. `ratings` are the car types'
/// scaled ratings; `player_rating` the player's car's.
pub fn line_up(event: &Event, rng: &mut GameRng, stage: Option<StageSkill>, player_car: usize, player_rating: f32, ratings: &[f32], car_by_hash: &dyn Fn(u32) -> Option<usize>) -> Vec<Entrant> {
    let random = crate::formats::hash::bin_hash("RANDOM");
    event
        .opponents
        .iter()
        .filter_map(|o| {
            let skill = skill(o.mode, o.skill, stage, player_rating);
            let car = if o.car == random {
                pick_car(rng, car_pool(event.suv), player_car, true, skill, &|c| ratings.get(c).copied().unwrap_or(0.0))? as usize
            } else {
                car_by_hash(o.car)?
            };
            Some(Entrant { name: o.name.clone(), car, skill, source: o.mode, preset: (o.car != random).then_some(o.car) })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_random_numbers_step_like_the_game() {
        let mut r = GameRng::default();
        assert_eq!(r.below(26), 0x1234_5678 % 26);
        let a = 0x1234_5678u32 ^ 0x1D87_2B41;
        let b = (a >> 5) ^ a;
        assert_eq!(r.0, (b << 27) ^ b ^ a);
    }

    #[test]
    fn picks_the_first_car_within_the_margin() {
        let mut r = GameRng::default();
        let pool = [1u8, 2, 3];
        // Every car rated 5: a skill of 2 needs the margin to reach 3.06+.
        let car = pick_car(&mut r, &pool, 0, true, 2.0, &|_| 5.0).unwrap();
        assert!(pool.contains(&car));
        // The player's car is skipped unless allowed.
        let mut r = GameRng::default();
        assert_eq!(pick_car(&mut r, &[7, 7, 9], 7, false, 10.0, &|_| 0.0), Some(9));
    }

    #[test]
    fn stage_skill_spreads_by_progress() {
        let s = StageSkill { events: 16, low: 0.0, high: 3.0, done: 8 };
        assert_eq!(s.at(), 1.5);
        assert_eq!(StageSkill { done: 20, ..s }.at(), 3.0);
    }
}

/// How 0x5264B0 numbers the grid slots (racer info +0x03, 1-based; the
/// car placement 0x6065C0 puts the racer with +0x03 = k on the k-th
/// starting position 0x5EB5E0 found, the marker with +0x0C = k − 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridOrder {
    /// 1: the humans (owner kind 1) first, then the AI (kind 2), each in
    /// racer order.
    HumansFirst,
    /// 2: the AI first, the humans behind them.
    AiFirst,
    /// 3: as 1, then 0x4FEDE0 swaps the slots of racer 0 and of a random
    /// racer (0x43C1C0 over all racers; with one human and no AI no draw).
    Shuffled,
}

impl GridOrder {
    /// The career's choice (0x5322F0 at 0x53250F, table 0x5325CC on the
    /// event's +0x0F): circuit, sprint and drag 3, street X and drift 2,
    /// special 1.
    pub fn career(kind: crate::frontend::career::EventKind) -> GridOrder {
        use crate::frontend::career::EventKind as K;
        match kind {
            K::StreetX | K::Drift => GridOrder::AiFirst,
            K::Special => GridOrder::HumansFirst,
            _ => GridOrder::Shuffled,
        }
    }
}

/// 0x5264B0 offline with one human (racer 0) and `ai` AI racers: each
/// racer's grid slot, 0-based (the human first). The online and LAN
/// branches (0x850078 = 1 / 4) and the debug slots 0x864FE0 / 0x864FE4
/// (0 in the file, never written) are left out.
pub fn grid_slots(order: GridOrder, ai: usize, rng: &mut GameRng) -> Vec<usize> {
    let mut slots: Vec<usize> = match order {
        GridOrder::AiFirst => std::iter::once(ai).chain(0..ai).collect(),
        _ => (0..=ai).collect(),
    };
    if order == GridOrder::Shuffled && ai > 0 {
        let r = rng.below(ai as u32 + 1) as usize;
        slots.swap(0, r);
    }
    slots
}

#[cfg(test)]
mod grid_tests {
    use super::*;

    /// 0x43E6B0 stores one step of the seed; the boot seed adds 0x12345678
    /// to the clock, so a zero clock gives the static state's successor.
    #[test]
    fn boot_seed_is_one_step_past_the_clock() {
        let mut r = GameRng::default();
        r.advance();
        assert_eq!(GameRng::boot(0).0, r.0);
        assert_ne!(GameRng::boot(1).0, GameRng::boot(2).0);
    }

    #[test]
    fn drift_starts_the_player_behind_the_rivals() {
        let mut rng = GameRng::default();
        assert_eq!(grid_slots(GridOrder::career(crate::frontend::career::EventKind::Drift), 3, &mut rng), vec![3, 0, 1, 2]);
        assert_eq!(grid_slots(GridOrder::HumansFirst, 3, &mut rng), vec![0, 1, 2, 3]);
        // Order 3 swaps racer 0 with a drawn racer; the slots stay a
        // permutation and only racer 0 and that racer move.
        for _ in 0..20 {
            let s = grid_slots(GridOrder::Shuffled, 3, &mut rng);
            let mut sorted = s.clone();
            sorted.sort();
            assert_eq!(sorted, vec![0, 1, 2, 3]);
            assert!(s.iter().enumerate().filter(|&(i, &k)| i != k).count() <= 2);
        }
        let before = rng.0;
        assert_eq!(grid_slots(GridOrder::Shuffled, 0, &mut rng), vec![0]);
        assert_eq!(rng.0, before, "no draw without AI");
    }
}
