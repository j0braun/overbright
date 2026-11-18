use crate::*;

/// One encounter racer's draw (0x53F010's loop body): its car, looks,
/// skill row and seed.
pub(crate) type EncounterPick = (u8, std::collections::BTreeMap<u8, u32>, usize, u32);

/// What an encounter racer's draw is made against (0x4FEAF0's pool and
/// ratings for the player's car).
pub(crate) struct EncounterDraw {
    pub(crate) player_car: usize,
    pub(crate) ratings: Vec<f32>,
    pub(crate) player_rating: f32,
    pub(crate) pool: &'static [u8],
}

impl EncounterDraw {
    pub(crate) fn new(car_types: &[nfsu2::car::CarType], ai: &AiData, player: &Car) -> EncounterDraw {
        EncounterDraw {
            player_car: car_types.iter().position(|t| t.name.eq_ignore_ascii_case(&player.car_type.name)).unwrap_or(0),
            ratings: car_types.iter().map(|t| ai.bounds.scale(t.handling.performance_rating)).collect(),
            player_rating: ai.bounds.scale(player.car_type.handling.performance_rating),
            pool: nfsu2::opponents::car_pool(player.car_type.suv),
        }
    }

    /// 0x4FEAF0(record, 100 + i, 2 + i, 1, 0, 0, −1) (the player's car
    /// allowed, against its rating), a random paint (0x6371F0), the
    /// level-2 neon (0x6371B0(0x9A, 2)), the skill row (0x4FED00) and the
    /// seed. `avoid` (Outrun Variety only, a clone deviation; empty =
    /// native): a car type drawn from it is drawn again, at most 8 times,
    /// with the same rule.
    pub(crate) fn pick(
        &self,
        car_types: &[nfsu2::car::CarType],
        parts: &nfsu2::formats::parts::PartDb,
        ai: &AiData,
        difficulty: usize,
        rng: &mut nfsu2::opponents::GameRng,
        avoid: &[u8],
    ) -> Option<EncounterPick> {
        let rating = |c: usize| self.ratings.get(c).copied().unwrap_or(10.0);
        let mut c = nfsu2::opponents::pick_car(rng, self.pool, self.player_car, true, self.player_rating, &rating)?;
        for _ in 0..8 {
            if !avoid.contains(&c) {
                break;
            }
            c = nfsu2::opponents::pick_car(rng, self.pool, self.player_car, true, self.player_rating, &rating)?;
        }
        let mut ride = nfsu2::ai_looks::Ride::stock(parts, &car_types[c as usize]);
        ride.random_paint(rng);
        ride.random_slot(rng, 0x9A, 2);
        let mut row = ai.tuning.random_row(difficulty, rng.unit(1.0));
        // Check aid (not native): NFSU2_ENCOUNTER_CAR=NAME:ROW makes
        // every encounter racer that car type and skill row, after the
        // draws, for like-for-like comparisons with a native capture
        // (docs/outrun.md, Live comparison).
        if let Some((name, r)) = debug_env().encounter_car.clone().as_deref().and_then(|v| v.split_once(':'))
            && let Some(k) = car_types.iter().position(|t| t.name.eq_ignore_ascii_case(name))
        {
            c = k as u8;
            ride = nfsu2::ai_looks::Ride::stock(parts, &car_types[k]);
            row = r.parse().unwrap_or(row);
        }
        // No seed of the racer's own is drawn: per racer the original
        // makes the 26-car shuffle, the paint, the neon part and the skill
        // row (**measured**, the sequence log, docs/rng.md).
        Some((c, ride.installed, row, 0))
    }

    /// 0x532020(0) for one racer: its performance matched to the
    /// player's (0x5B7580, `match_to_human`; no draws), then dressed by
    /// the levels that leaves (0x6399E0(1, 1)). Returns the fitted parts.
    pub(crate) fn dress(
        car_types: &[nfsu2::car::CarType],
        parts: &nfsu2::formats::parts::PartDb,
        perf: (&[nfsu2::performance::Package], &[(String, Vec<u8>)], Option<nfsu2::ai::route::HumanPerf>),
        ai: &AiData,
        p: &mut EncounterPick,
        rng: &mut nfsu2::opponents::GameRng,
    ) -> std::collections::BTreeSet<u8> {
        let (packages, records, human) = perf;
        let ty = &car_types[p.0 as usize];
        let mut fitted = std::collections::BTreeSet::new();
        if let (Some(h), Some((_, rec))) =
            (human.as_ref(), records.iter().find(|(n, _)| n.eq_ignore_ascii_case(&ty.name)))
        {
            fitted = nfsu2::ai::route::match_to_human(&ai.presets, &ai.bounds, p.0 as usize, h, rec, packages);
        }
        let mut ride = nfsu2::ai_looks::Ride { db: parts, car: ty, installed: std::mem::take(&mut p.1) };
        ride.dress(rng, nfsu2::performance::state_levels(packages, &|id| fitted.contains(&id)));
        p.1 = ride.installed;
        fitted
    }
}
