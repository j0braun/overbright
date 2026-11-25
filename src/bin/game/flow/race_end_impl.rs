use crate::update::Flow;
use crate::*;

impl Game {
    /// An outcome left unsettled (Restart, 0x5F41F0(9)): no handler runs;
    /// a win stays filed in its stage record (0x513390 at the race's end)
    /// while its visit entry stays unwon (`Profile::filed`).
    pub(crate) fn drop_race_result(&mut self) {
        self.race_flow.result = None;
    }

    /// Settle a career race's outcome (0x54C950; see [`RaceResult`]): a
    /// win runs the win handler 0x541430 (cash and reputation only when it
    /// pays), a loss 0x528510 (the visit entry +6 = 4 unless won; Rachel).
    pub(crate) fn settle_race(&mut self) {
        let Some(r) = self.race_flow.result.take() else { return };
        let Some(event) = self.game_data.career.events.get(r.event).cloned() else { return };
        let Some(p) = self.profile.as_mut() else { return };
        if r.won {
            let stage = p.stage;
            // Without the payout gate 0x513430 does not run at all: no
            // points and no race counted (+0xE4).
            p.win_paid(&self.game_data.career, &self.game_data.root, &event, r.time, r.pays.then_some(r.reward));
            // The stage end's 0x528330: Rachel names the new stage 15 s of
            // driving later.
            if p.stage > stage {
                p.hints.stage_up(p.stage);
            }
        } else {
            p.lost(&event);
        }
        // Won (0x541430 → 0x54152D) or lost (0x528510 → 0x528553):
        // 0x5242D0 tells Rachel's hints.
        let stage_final = self.game_data.career.stages.get(event.stage as usize).is_some_and(|d| d.final_event == event.unlock);
        p.hints.race_over(event.unlock, r.won, event.category == career::Category::Scripted, event.stage, stage_final);
        let _ = p.commit();
    }

    /// The race is over (the race-over hold has run): the results, the
    /// standings and stats pages, the track record, then the next round,
    /// or the event's outcome and the post-race screens; the ending scene
    /// (called from `update_world`).
    pub(crate) fn finish_race(&mut self, end: RaceEnd) -> Flow {
        let RaceEnd { time } = end;
        let Mode::Race(race) = &mut self.mode else { unreachable!("finish_race outside a race") };
        race.end_unfinished();
        // The race's ending scene (see `play_race_ending`): its
        // marker, the start line on a circuit, else the finish
        // line (0x42FC60 with its flag clear).
        let ending = if race.route.loops {
            race.route.start_line.zip(race.route.start_heading)
        } else {
            race.route.finish_line.zip(race.route.finish_heading)
        };
        let ending = ending
            .filter(|_| race.test.is_none() && race.replay.playback.is_none())
            .map(|(at, h)| (self.game_data.career.events[race.event].round_track(race.round), at, h));
        // **Inferred**: a finished test run goes back to the
        // dyno menu (what the game does at its end is not
        // traced).
        if race.test.is_some() {
            self.mode = self.icon_menu(MenuKind::Dyno);
            return Flow::Stop;
        }
        let event = self.game_data.career.events[race.event].clone();
        let car = self.profile.as_ref().map_or_else(|| "240SX".to_string(), |p| p.car.clone());
        let car = self.sim.drive.as_ref().map_or(car, |d| d.car.car_type.name.clone());
        let name = self.profile.as_ref().map_or_else(|| "Player".to_string(), |p| p.name.clone());
        // Every racer, the player first, and the finishing order.
        let mut runs: Vec<(String, String, &Progress)> = vec![(name.clone(), car.clone(), &race.racers.me)];
        runs.extend(race.racers.rivals.iter().map(|r| (r.name.clone(), r.car.car_type.name.clone(), &r.progress)));
        let mut order = finish_order(&runs);
        // A drift race places by points (0x56DDA0 on the list
        // 0x864630: the stats' place 0x5160F7, the event's win
        // 0x528918 — rank 1 — and the race-over message
        // 0x4AB4B0), whatever the finishing order.
        let board = (!race.scoring.board.is_empty()).then(|| race.scoring.board.clone());
        if let Some(b) = &board {
            order.sort_by_key(|&i| nfsu2::drift::board_place(b, i).unwrap_or(usize::MAX));
        }
        let place_of = |i: usize| match board.as_ref().and_then(|b| nfsu2::drift::board_place(b, i)) {
            Some(p) => p,
            None => order.iter().position(|&o| o == i).unwrap_or(0) + 1,
        };
        let place = place_of(0);
        let ending = race_ending(ending, self.sim.drive.as_ref(), race, &order);
        // Each racer's points so far (the player first).
        let mut points = race.field.points.clone();
        points.resize(runs.len(), 0);
        for (i, p) in points.iter_mut().enumerate() {
            *p += screens::race_points(place_of(i));
        }
        // An event of more than one race (0x4C0050): each race's
        // tournament results then the standings on the summed
        // points (0x513EB0, highest first by 0x4A4730); before
        // the last, the next race.
        // A Quick Race (game mode 2) is one race on its mode
        // record's track: 0x4C0050 / 0x4C00C0 are false there
        // and UI_PostRace offers no Next Race outside a career
        // (0x4BEE10), so it never goes on to another round.
        let rounds = if race.quick { 1 } else { event.races.len() };
        let tournament = rounds > 1;
        // 0x4C0953: the standings list takes the tournament's
        // racers in its record's order (0x85BA64 +0x08, 12 bytes
        // each: under Wine the AI racers 100.. first and the
        // player, 0, last), each insert followed by 0x43E650, a
        // bubble pass that swaps only on more points
        // (0x4A4730), so ties keep that order.
        let mut by_points: Vec<usize> = (1..runs.len()).chain(std::iter::once(0)).collect();
        by_points.sort_by(|&a, &b| points[b].cmp(&points[a]));
        self.race_flow.standings = tournament.then(|| standings(race.round + 1 < rounds, &runs, &points, &by_points));
        let kind = if tournament { ResultsKind::Tournament } else { ResultsKind::Race };
        // A drift race's records take each racer's drift trick
        // (style manager slot 0, 0x516036).
        let drift_race = event.race_mode().drift();
        self.race_flow.stats = Some(post_race_stats(race, &runs, &order, drift_race));
        // A drift race ranks and shows by score (0x4BF140: a row
        // per node of the points list in its order, TIME_DATA
        // "%$.0f" the node's points; the virtual rivals of a
        // downhill track too).
        let scores = race.score();
        let (rows, player_row) = result_rows(race, &runs, &order, &board, place_of, &name, scores);
        track_record(race, &event, &self.game_data.career, self.profile.as_mut(), player_row, scores);
        if race.round + 1 < rounds {
            let mut field = race.field.clone();
            field.points = points;
            field.times.push(time);
            self.race_flow.next_round = Some((race.event, race.quick, race.from_map, race.round + 1, field));
            let results =
                PostRaceResults { kind, rows, opened: self.now(), scores: scores.is_some(), player: player_row };
            self.race_flow.chain.clear();
            self.keep_replay(Mode::Menu(Menu::Results(results, None)));
            self.play_ending(ending);
            return Flow::Stop;
        }
        // The event's time: all its races.
        let event_time = time + race.field.times.iter().sum::<f32>();
        // The winning margin (0x513FE0): one race on time, 1 −
        // the winner's over the runner-up's; more races on
        // points, the best over the next best − 1; with no
        // runner-up 100%.
        let margin = winning_margin(tournament, &board, &points, &by_points, &runs, &order);
        // A win: first place, or the most points over the races.
        let won = if tournament { by_points[0] == 0 } else { place == 1 };
        // The rewards (0x4A4240): the bank is 0x532E60 (the
        // event's purse, or a sponsor race's sponsor's), reputation
        // the winning margin's points (0x513FE0 → table 0x7F7AC8).
        let bank = self.profile.as_ref().map_or(event.reward, |p| p.race_bank(&self.game_data.career, &event));
        let reward = career::race_reward(&event, margin, bank);
        // The race's end (0x54CA70): a win is filed in its
        // stage record (0x513390) at once; the win or loss
        // handler waits for the post-race exit (0x54C950,
        // `RaceResult`). An event already won when the race
        // began (0x532EB0: +0xD28) has none; one started from
        // the World Map's list (0x5325F0: +0xD27) does not pay.
        let mut pays = false;
        self.race_flow.result = None;
        if !race.quick
            && let Some(p) = self.profile.as_mut()
            && !p.completed.contains(&event.index)
        {
            if won {
                p.filed.insert(event.index);
            }
            pays = !self.race_flow.map_start;
            self.race_flow.result = Some(RaceResult { event: event.index, won, time: event_time, reward, pays });
            let _ = p.commit();
        }
        // The post-race chain (0x4F3EA0): the results, then in
        // career after a win that pays (0x5287B0(0), 0x500DB0,
        // not 0x500DD0) the reward, then the post-race menu.
        let reward = (won && pays).then(|| Reward {
            reputation: reward.0,
            bank: reward.1,
            name: name.clone(),
            opened: self.now(),
        });
        let results = PostRaceResults { kind, rows, opened: self.now(), scores: scores.is_some(), player: player_row };
        self.race_flow.chain.clear();
        self.keep_replay(Mode::Menu(Menu::Results(results, reward)));
        self.play_ending(ending);
        Flow::Go
    }

    /// The race's ending scene, when it has a place and a winner.
    fn play_ending(&mut self, e: RaceEnding) {
        // The companion's renderer built during the race-over hold
        // (`CompanionAhead`), for the ending's first frame to take; one not
        // done yet is let go (the frame builds it in place).
        if let Some(a) = self.loads.companion_ahead.take()
            && let Some(c) = e.companion.as_ref().filter(|c| c.rival == a.rival && a.job.is_finished())
            && let Some((key, r)) = a.renderer.lock().unwrap().take()
        {
            self.render.cars_ahead.offer(&c.id, key, r);
        }
        if let (Some((track, at, heading)), Some(winner)) = (e.at, &e.handling) {
            self.play_race_ending(track, at, heading, winner, e.winner == 0, e.companion);
        }
    }
}

/// A racer's results row source: name, car, progress.
pub(crate) type Run<'a> = (String, String, &'a Progress);

/// The race's ending scene (`race_ending`): its place, the winner (0 the
/// player), the winner's handling, an AI winner's scene car.
struct RaceEnding {
    at: Option<(u16, Vec3, f32)>,
    winner: usize,
    handling: Option<nfsu2::handling::Handling>,
    companion: Option<SceneCar>,
}

/// The finishing order (indices into `runs`, the player first).
pub(crate) fn finish_order(runs: &[Run<'_>]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..runs.len()).collect();
    // A racer who dropped out takes the last place still
    // free when it happens (0x6076E0), so after the
    // finishers, the earliest drop-out last.
    // Racers the race's end stopped (0x608CB0) follow the
    // finishers in their race order then.
    let key = |p: &Progress| match (p.dropped, p.finished, p.ended) {
        (Some(_), Some(t), _) => (2, -t),
        (_, _, Some(place)) => (1, place as f32),
        (_, t, _) => (0, t.unwrap_or(f32::MAX)),
    };
    order.sort_by(|&a, &b| {
        let (ka, kb) = (key(runs[a].2), key(runs[b].2));
        ka.0.cmp(&kb.0).then(ka.1.total_cmp(&kb.1))
    });
    order
}

/// The ending scene's place and cars: the winner (0 the player), its
/// handling, and the AI winner riding along.
fn race_ending(ending: Option<(u16, Vec3, f32)>, drive: Option<&Drive>, race: &Race, order: &[usize]) -> RaceEnding {
    let ending_winner = order.first().copied().unwrap_or(0);
    let ending_handling = if ending_winner == 0 {
        drive.map(|d| d.car.car_type.handling.clone())
    } else {
        race.racers.rivals.get(ending_winner - 1).map(|r| r.car.car_type.handling.clone())
    };
    let ending_companion = ending
        .filter(|_| ending_winner != 0)
        .and_then(|_| race.racers.rivals.get(ending_winner - 1))
        .map(|r| SceneCar { rival: ending_winner - 1, ..SceneCar::from_rival(r) });
    RaceEnding { at: ending, winner: ending_winner, handling: ending_handling, companion: ending_companion }
}

/// A tournament's standings (on the summed points, `by_points`).
fn standings(next_race: bool, runs: &[Run<'_>], points: &[i32], by_points: &[usize]) -> PostRaceResults {
    PostRaceResults {
        kind: ResultsKind::Standings { next_race },
        rows: by_points.iter().map(|&i| (runs[i].0.clone(), runs[i].1.clone(), 0.0, points[i])).collect(),
        opened: 0.0,
        scores: false,
        player: by_points.iter().position(|&i| i == 0),
    }
}

/// The post-race stats pages.
fn post_race_stats(race: &Race, runs: &[Run<'_>], order: &[usize], drift_race: bool) -> screens::PostRaceStats {
    // The stats pages (0x515DB0) in finishing order, with each
    // racer's place at every lap's end (0x541B50).
    let lap_place = lap_places(&runs.iter().map(|r| r.2).collect::<Vec<_>>(), race.laps);
    // A drift race's records take each racer's drift trick
    // (style manager slot 0, 0x516036).
    let styles: Vec<Option<&nfsu2::drift::StyleManager>> =
        std::iter::once(race.scoring.style.as_ref()).chain(race.racers.rivals.iter().map(|r| r.style.as_ref())).collect();
    // The page opens on the first local player's (0x4FBBC0
    // pages to the results entry whose racer is 0).
    let mine = order.iter().position(|&i| i == 0).unwrap_or(0);
    screens::PostRaceStats {
        racers: order
            .iter()
            .map(|&i| {
                let (name, car, run) = &runs[i];
                let time = run.finished.unwrap_or(race.timing.clock);
                let best = run.lap_times.iter().copied().fold(f32::INFINITY, f32::min);
                screens::RacerStats {
                    name: name.clone(),
                    car: car.clone(),
                    time,
                    top_speed: run.top_speed,
                    average: if time > 0.0 { run.distance / time } else { 0.0 },
                    best_lap: if best.is_finite() { best } else { time },
                    laps: run.lap_times.iter().zip(&lap_place[i]).map(|(&t, &p)| (t, p)).collect(),
                    status: run.dropped,
                    shifts: (run.shifts, run.perfect_shifts),
                    drift: drift_race.then(|| {
                        styles[i]
                            .and_then(|s| s.trick(nfsu2::drift::DRIFT))
                            .map_or((0.0, 0.0, 0.0), |t| (t.banked, t.best, t.longest))
                    }),
                }
            })
            .collect(),
        index: mine,
        player: mine,
        circuit: race.route.loops,
        drag: race.drag,
        drift: drift_race,
        opened: 0.0,
        arrows: Default::default(),
    }
}

/// The results rows (a drift race's by its board) and the human's row.
fn result_rows(
    race: &Race,
    runs: &[Run<'_>],
    order: &[usize],
    board: &Option<Vec<nfsu2::drift::BoardEntry>>,
    place_of: impl Fn(usize) -> usize,
    name: &str,
    scores: Option<i32>,
) -> (Vec<(String, String, f32, i32)>, Option<usize>) {
    // 0x606CC0 writes the finish time (+0x198) only for status 1 at the
    // race's laps (0x606F92) and statuses 4–7 (0x606CFB's
    // table); a racer knocked out (3), with a blown engine (8)
    // or totaled (9) keeps 0, shown "0.00" (**observed** under
    // Wine, race11: a drag's blown engine).
    let time_of = |p: &Progress| match p.dropped {
        Some(3 | 8 | 9) => 0.0,
        _ => p.finished.unwrap_or(race.timing.clock),
    };
    let mut rows: Vec<(String, String, f32, i32)> = order
        .iter()
        .map(|&i| (runs[i].0.clone(), runs[i].1.clone(), time_of(runs[i].2), screens::race_points(place_of(i))))
        .collect();
    // The human's row (0x4BF290's HIGHLIGHT).
    let mut player_row = match &board {
        Some(b) => b.iter().position(|e| e.racer == Some(0)),
        None => order.iter().position(|&i| i == 0),
    };
    if let Some(b) = &board {
        player_row = b.iter().position(|e| e.racer == Some(0));
        rows = b
            .iter()
            .enumerate()
            .map(|(k, e)| {
                (
                    if e.racer == Some(0) { name.to_string() } else { e.name.clone() },
                    e.car.clone(),
                    e.score,
                    screens::race_points(k + 1),
                )
            })
            .collect();
    } else if let (Some(points), Some(row)) = (scores, rows.iter_mut().find(|r| r.0 == name)) {
        row.2 = points as f32;
    }
    (rows, player_row)
}

/// The race's track record and the profile's race totals.
fn track_record(
    race: &Race,
    event: &career::Event,
    data: &CareerData,
    profile: Option<&mut Profile>,
    player_row: Option<usize>,
    scores: Option<i32>,
) {
    // 0x551440 → 0x515990 → 0x502DF0: the race's record
    // (track, laps flag, backward) — the player's best lap
    // in ticks, or a drift track's score (record kind 2 by
    // prov: inferred game.update_world-3
    // the track's +0x94 bits). **Inferred**: the laps flag
    // 0x89E7BC is the route's loop.
    let track_no = event.round_track(race.round);
    if race.replay.playback.is_none()
        && let (Some(t), Some(p)) = (data.tracks.get(&track_no), profile)
    {
        // 0x515990's totals (profile +0xA00…): the place
        // is the results entry's +0 (the board's order).
        let style = race.scoring.style.as_ref().map_or(0, |s| career::style_score(s.total, 1.0));
        p.totals.race_done(
            career::record_kind(t.flags),
            player_row.map_or(0, |r| r + 1),
            race.racers.me.odometer,
            race.racers.me.dropped.unwrap_or(0),
            style,
        );
        let drift = career::record_kind(t.flags) == 2;
        let best = race.racers.me.lap_times.iter().copied().fold(f32::INFINITY, f32::min);
        let value = if drift {
            scores
        } else {
            race.racers.me.finished.map(|f| ((if best.is_finite() { best } else { f }) * 4000.0).round() as i32)
        };
        if let Some(v) = value {
            p.add_record(drift, track_no as i16, race.route.loops as i8, race.reversed as i8, v);
            let _ = p.commit();
        }
    }
}

/// The winning margin (0x513FE0), in percent.
fn winning_margin(
    tournament: bool,
    board: &Option<Vec<nfsu2::drift::BoardEntry>>,
    points: &[i32],
    by_points: &[usize],
    runs: &[Run<'_>],
    order: &[usize],
) -> f32 {
    if let (false, Some(b)) = (tournament, &board) {
        nfsu2::drift::board_margin(b)
    } else if tournament {
        let best = points[by_points[0]] as f32;
        let next = by_points.get(1).map_or(0.0, |&i| points[i] as f32);
        if next > 0.0 { (best / next - 1.0) * 100.0 } else { 100.0 }
    } else {
        let first = runs[order[0]].2.finished.unwrap_or(0.0);
        match order.get(1).and_then(|&i| runs[i].2.finished) {
            Some(second) if second > 0.0 => (1.0 - first / second) * 100.0,
            _ => 100.0,
        }
    }
}
