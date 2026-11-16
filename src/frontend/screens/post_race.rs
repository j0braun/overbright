use crate::frontend::fe_ids;
use super::*;

/// The points for a race place (1-based; table 0x7F7B8C read by 0x513E70
/// and 0x513EB0): 8, 6, 4, 3, 2, 1, then none.
pub fn race_points(place: usize) -> i32 {
    [8, 6, 4, 3, 2, 1, 0].get(place.wrapping_sub(1)).copied().unwrap_or(0)
}

/// Which post-race results screen (the chain 0x4F3EA0): a single race's,
/// or with more than one race in the event (0x4C0050 / 0x4C00C0) the
/// tournament's race results then its standings in place of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResultsKind {
    Race,
    /// UI_PostRace_TournResults (0x4F41F0 → 0x4D7400): the race's places,
    /// times and points (0x513E70).
    Tournament,
    /// UI_PostRace_TournStandings (0x4F4610 → 0x4C07A0 / 0x4D77A0): the
    /// racers by summed points (0x513EB0, sorted by 0x4A4730), RANKING
    /// the rank; NEXT reads "Next Race" while a race is left (0x503080).
    Standings {
        next_race: bool,
    },
}

/// UI_PostRaceResults (screen 0x4FBB60, set up by 0x4E9640): a row per
/// racer in finishing order (0x4D7240 → 0x4BF290): RANKING_DATA_n the
/// place (the player's runs HIGHLIGHT), PLAYER_DATA_n the name,
/// CAR_DATA_n the car's CAR_NAME_<car> label, TIME_DATA_n the time
/// (0x4A4020); the rows past the racers hide (0x4A41A0). The tournament
/// screens add PLAYER_POINT_DATA_n (`%d`); the standings have no times.
#[derive(Clone)]
pub struct PostRaceResults {
    pub kind: ResultsKind,
    /// Name, car type, time (s), points, by place (standings: by points).
    pub rows: Vec<(String, String, f32, i32)>,
    pub opened: f32,
    /// A drift race: TIME_DATA_n shows the score in the time's place,
    /// `%$d` of the racer's points at the final lap (0x4BF290: results
    /// record +0x198 + 4 × laps).
    pub scores: bool,
    /// The local player's row, whose RANKING_DATA_n runs HIGHLIGHT
    /// (0x4BF290; 0x4C0300 / 0x4C0A00 on the tournament screens).
    pub player: Option<usize>,
}

impl PostRaceResults {
    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        let name = match self.kind {
            ResultsKind::Race => "UI_PostRaceResults",
            ResultsKind::Tournament => "UI_PostRace_TournResults",
            ResultsKind::Standings { .. } => "UI_PostRace_TournStandings",
        };
        let Some(package) = assets.package(name) else { return Vec::new() };
        let mut state = ScreenState::default();
        // UI_PostRaceResults' panel group (0x8D7E0C53): its INIT chains to
        // 0x16A259, alpha 0, until the event handler's INIT chain
        // (0x13C37B) sends BDFD2DF9 at 320 ms and the panel's 5B0D9106
        // fades it in from 40 to 320 ms of that (**observed** under Wine,
        // finish151: the ending scene with Continue alone, then the panel
        // fading in; the clone showed the panel at full alpha first).
        if self.kind == ResultsKind::Race {
            state.chained.insert(0x8D7E_0C53);
        }
        footer(&mut state, package, "Next");
        let points = self.kind != ResultsKind::Race;
        let times = !matches!(self.kind, ResultsKind::Standings { .. });
        // UI_PostRaceResults alternates its time column (0x4E9B8B: +0x70
        // counts the frame times, past +0x74 +0x68 flips): "Finish Time"
        // (0xC97A2D22) with each racer's time (0x4A4020) for 4.5 s
        // (0x78DFF0, the first period, 0x4F3C55), then "Time Behind"
        // (0xE2F9334D) for 1.75 s (0x78DFEC), the header 0xA4EC03BE
        // (0x4BF0A0) and the rows by 0x4A4110: the first "--:--.--"
        // (0x61C9B7A2), the others the signed difference (0x576D40 flag
        // 2). Not in drift races (0x89E7D9 / 0x890104), online or on a
        // dyno run (0x89E7E6).
        let behind = self.kind == ResultsKind::Race && !self.scores && results_behind((now - self.opened) / 1000.0);
        if self.kind == ResultsKind::Race && !self.scores {
            let header = if behind { 0xE2F9_334D } else { 0xC97A_2D22 };
            state.text.insert(fe_ids::HEADER_TEXT_3, assets.strings.get(header).unwrap_or("").to_string());
        } else if self.scores {
            // 0x4A3FDD: in a drift race (0x89E7D9 / 0x890104) HEADER_TEXT_3
            // (0xA4EC03BE, string at 0x7904BC) reads "Score" (0xC03F9F19;
            // seen under Wine, S3_DRIFT_6's Event Results).
            state.text.insert(fe_ids::HEADER_TEXT_3, assets.strings.get(fe_ids::OBJ_C03F9F19).unwrap_or("Score").to_string());
        }
        let leader = self.rows.first().map_or(0.0, |r| r.2);
        for n in 1..=6 {
            let field = |f: &str| fe_hash(&format!("{f}_DATA_{n}"));
            match self.rows.get(n - 1) {
                Some((name, car, time, score)) => {
                    // The race's place is `%2d` on the tournament results
                    // (0x4C0300), the rank `%d` elsewhere (0x4C0A00).
                    let rank = if self.kind == ResultsKind::Tournament { format!("{n:2}") } else { n.to_string() };
                    state.text.insert(field("RANKING"), rank);
                    state.shown.insert(field("RANKING"));
                    state.text.insert(field("PLAYER"), name.clone());
                    state.text.insert(field("CAR"), assets.strings.label(&format!("CAR_NAME_{car}")));
                    if times {
                        let shown = if self.scores {
                            crate::frontend::hud::grouped_int(*time as i32)
                        } else if behind {
                            if n == 1 {
                                assets.strings.get(fe_ids::NO_LAP_TIME).unwrap_or("--:--.--").to_string()
                            } else {
                                signed_time(crate::standings::ticks(*time - leader))
                            }
                        } else {
                            // FormatRaceTime 0x577250 flags 0 on the ticks.
                            crate::frontend::hud::time_text(crate::standings::ticks(*time))
                        };
                        state.text.insert(field("TIME"), shown);
                    }
                    if points {
                        state.text.insert(field("PLAYER_POINT"), score.to_string());
                    }
                }
                None => {
                    for f in ["RANKING", "PLAYER", "CAR", "TIME", "PLAYER_POINT"] {
                        state.hidden.insert(field(f));
                    }
                }
            }
        }
        // The row backings (groups 0x949F4946 + n − 1) INIT (alpha 0x91)
        // → HIDE (alpha 0); their 0xBDFD2DF9 response names FADEIN, which
        // the groups do not have, so nothing shows them but the HIGHLIGHT
        // RANKING_DATA_n sends them (0xABC08912): only the local player's
        // row has a backing (Wine, menus110 s13).
        state.chained.extend((0..6).map(|n| 0x949F_4946u32 + n));
        if let Some(n) = self.player.filter(|&n| n < self.rows.len().min(6)) {
            state.run(fe_hash(&format!("RANKING_DATA_{}", n + 1)), "HIGHLIGHT", 0.0);
        }
        if let ResultsKind::Standings { next_race: true } = self.kind {
            state.text.insert(fe_hash("NEXT"), assets.strings.get(fe_ids::NEXT_RACE_LABEL).unwrap_or("").to_string());
        }
        // Offline 0x4E9640 runs HIDE on the online timer ("45 sec") and
        // SHOW on the restart group; the "disconnected" line's INIT chains
        // to HIDE. The standings' OverTime is online only (0x4EA570).
        state.hidden.insert(0x9B8C_9D7A);
        state.hidden.insert(0xC18C_12FD);
        state.hidden.insert(fe_hash("OverTime"));
        // The tournament results' "Waiting for Server Authentication..."
        // line INITs straight to HIDE and nothing shows it offline
        // (0x4F41F0).
        state.hidden.insert(0x1F10_5857);
        layout(assets, package, &state, now - self.opened)
    }
}

/// Whether UI_PostRaceResults shows "Time Behind" `t` s after it opened:
/// 4.5 s of finish times, then 1.75 s of gaps, over and over (0x4E9B8B).
pub fn results_behind(t: f32) -> bool {
    let t = t.max(0.0) % (4.5 + 1.75);
    t >= 4.5
}

/// One racer's page of UI_PostRaceStats: the results record 0x515DB0
/// builds from the racer (place, status, finish time, best lap +0x10, the
/// stats +0x10C on) and the per-lap places 0x541B50 ranks after the race.
#[derive(Clone, Debug)]
pub struct RacerStats {
    pub name: String,
    pub car: String,
    /// The finish time, s.
    pub time: f32,
    /// Top and average speed, m/s.
    pub top_speed: f32,
    pub average: f32,
    pub best_lap: f32,
    /// Each lap's time and the place at its end (0x541B50 ranks the racers
    /// on their time at each lap; a lap in first counts as led).
    pub laps: Vec<(f32, u32)>,
    /// The racer's status when it dropped out (record +8: 3 knocked out,
    /// 8 blown engine, 9 totaled …); `None` for a finisher.
    pub status: Option<u16>,
    /// Up-shifts and perfect ones (record +0x124 / +0x128 = racer +0x1B8 /
    /// +0x1BC).
    pub shifts: (u32, u32),
    /// A drift race's drift trick (style manager slot 0, racer +0x1D4):
    /// banked points (+0x2C → record +0x21C), the best run's points
    /// (+0x44 → +0x214, truncated) and the longest run, s (+0x40 → +0x218).
    pub drift: Option<(f32, f32, f32)>,
}

impl RacerStats {
    pub fn laps_led(&self) -> usize {
        self.laps.iter().filter(|l| l.1 == 1).count()
    }
}

/// UI_PostRaceStats (screen 0x4FD6C0 → 0x4FBBC0): a page per racer in
/// finishing order, paged with the arrows (0x4A4480). 0x4E9C00 fills it:
/// RACER_DATA `%2d   %s - %s` (place, name, car), then the stats as
/// option_name_n / option_data_n rows from 1 (0x54F830): FINISH TIME,
/// TIME BEHIND, TOP SPEED, AVERAGE SPEED, and on a circuit (0x89E7B4 clear)
/// BEST LAP TIME and LAPS LED; on a circuit the lap chart (0x4D73B0 →
/// 0x4BFD40) fills TIME_DATA_n `%d   %s` (lap, lap time) and
/// POSITION_DATA_n `%d` for laps 1–10, while point to point hides its
/// headings HEAD_TEXT_2 / _3.
#[derive(Clone)]
pub struct PostRaceStats {
    pub racers: Vec<RacerStats>,
    pub index: usize,
    /// The first local player's page, where the screen opens (0x4FBBC0
    /// pages to the results entry whose racer is 0).
    pub player: usize,
    /// Not point to point (0x89E7B4 clear).
    pub circuit: bool,
    /// A drag race (0x89E7D8 / 0x890108): no lap chart, PERFECT SHIFTS.
    pub drag: bool,
    /// A drift race (0x89E7D9 / 0x890104): the drift rows.
    pub drift: bool,
    pub opened: f32,
    /// The page arrows' blink (0x4A4480, [`ArrowBlink`]).
    pub arrows: ArrowBlink,
}

mod post_race_stats {
    pub const LEFT: u32 = crate::frontend::fe_ids::POST_RACE_STATS_LEFT;
    pub const RIGHT: u32 = crate::frontend::fe_ids::POST_RACE_STATS_RIGHT;
    pub const TITLE_MASTER: u32 = crate::frontend::fe_ids::TITLE_MASTER;
    pub const DATA_MASTER: u32 = crate::frontend::fe_ids::DATA_MASTER;
    pub const TIME_MASTER: u32 = crate::frontend::fe_ids::TIME_MASTER;
    pub const POSITION_MASTER: u32 = crate::frontend::fe_ids::POSITION_MASTER;
}

/// An image's box (0x51D080 / 0x51D740): top left and size.
pub(crate) fn master_box(package: &crate::frontend::feng::Package, hash: u32) -> Option<(Vec2, Vec2)> {
    let o = package.object(hash)?;
    let (p, s) = (o.attributes.position(), o.attributes.size());
    let size = Vec2::new(s.x, s.y);
    Some((Vec2::new(p.x, p.y) - size * 0.5, size))
}

impl PostRaceStats {
    /// Left / Right page (0x4F3DE0 → 0x4A4480: the arrow that way
    /// BLINKs from 0, or HIDEs on the first / last page, the other goes
    /// STATIC).
    pub fn key(&mut self, key: Key, now: f32) {
        match key {
            Key::Left if self.index > 0 => {
                self.index -= 1;
                self.arrows.step(false, now, true);
            }
            Key::Right if self.index + 1 < self.racers.len() => {
                self.index += 1;
                self.arrows.step(true, now, true);
            }
            _ => {}
        }
    }

    /// `metric`: the speeds in km/h (0x83AA1A: × 3.6, "km/h"), else mph.
    pub fn quads(&self, assets: &Assets, metric: bool, now: f32) -> Vec<Quad> {
        let Some(package) = assets.package("UI_PostRaceStats") else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Back");
        // The option list's constructor (0x552790), with TITLE_MASTER and
        // DATA_MASTER present, hides its "scrollbar" (0x542AD0) handle
        // and arrows (0x50CA00 sets the hidden flag, which the handle's
        // FADEIN does not clear); the page arrows are shown again by the
        // paging (0x4A4480), the handle never: no dot on the top bar.
        state.hidden.insert(fe_ids::OBJ_3FBB7A4E);
        let Some(r) = self.racers.get(self.index) else { return Vec::new() };
        let label = |h: u32| assets.strings.get(h).unwrap_or("").to_string();
        let ticks = |s: f32| (s * 4000.0) as i32;
        state.text.insert(
            fe_hash("RACER_DATA"),
            format!("{:2}   {} - {}", self.index + 1, r.name, assets.strings.label(&format!("CAR_NAME_{}", r.car))),
        );
        // The rows (0x4E9C00), each an item whose text routine (vtable
        // slot 3) fills the label and the data. TIME BEHIND (0x4A4380): the
        // leader "--:--.--", a finisher the signed gap, a racer that
        // dropped out its status.
        let leader = self.racers.first().map_or(r.time, |l| l.time);
        let behind = match r.status {
            Some(status) => label(crate::frontend::hud::status_label(status)),
            None if self.index == 0 => label(fe_ids::NO_LAP_TIME),
            None => signed_time(ticks(r.time) - ticks(leader)),
        };
        let (unit, per) = if metric { (label(0xE74B_7E13), 3.6) } else { (label(0xE74B_86F8), 2.237) };
        // 0x4BFA30 / 0x4BFAA0: the speed × 3.6 (0x7904FC) or × 2.237
        // (0x78426C), stored as a float, printed "%$3.1f %s" (0x7951F0):
        // grouped, the last digit half up (0x43E950).
        let top = (0x5D30_4669, format!("{} {unit}", grouped_1(r.top_speed * per)));
        let average = (0x5F38_E63E, format!("{} {unit}", grouped_1(r.average * per)));
        let mut rows = Vec::new();
        if self.drift {
            // 0x4BFD00 SCORE: "%$d" of the banked points; 0x4BFC60 BEST
            // DRIFT SCORE "%$d"; 0x4BFC90 LONGEST DRIFT: the seconds
            // × 4000 + 0.5 as a time (0x577250 flag 0).
            let (score, best, longest) = r.drift.unwrap_or_default();
            rows.push((0xE8AE_BFAF, crate::frontend::grouped(score as i64)));
            rows.push((0xD7D8_CA79, crate::frontend::grouped(best as i64)));
            rows.push((0x5C17_EC47, crate::frontend::hud::time_text((longest * 4000.0 + 0.5) as i32)));
            rows.push(top);
            rows.push(average);
        } else {
            let time = if r.status.is_some() { 0 } else { ticks(r.time) };
            rows.push((0xF9D7_F4C2, crate::frontend::hud::time_text(time)));
            rows.push((0xAB44_ED8B, behind));
            rows.push(top);
            rows.push(average);
            if self.drag {
                // 0x4BFB10: "%$d/%$d", perfect / all up-shifts.
                rows.push((
                    0x680A_C597,
                    format!(
                        "{}/{}",
                        crate::frontend::grouped(r.shifts.1.into()),
                        crate::frontend::grouped(r.shifts.0.into())
                    ),
                ));
            } else if self.circuit {
                rows.push((0xAF69_43CB, crate::frontend::hud::time_text(ticks(r.best_lap))));
                rows.push((0xADBF_D917, format!("{}/{}", r.laps_led(), r.laps.len())));
            }
        }
        // The list (0x552790, 0x51FF90) lays its rows out from TITLE_MASTER
        // and DATA_MASTER: the names' left edge on the title master's, the
        // data's right edge on the data master's (0x4A4300), each row the
        // title master's height below the last (0x54F830).
        let title = master_box(package, post_race_stats::TITLE_MASTER);
        let data_box = master_box(package, post_race_stats::DATA_MASTER);
        for n in 1..=10 {
            let (name, data) = (fe_hash(&format!("option_name_{n}")), fe_hash(&format!("option_data_{n}")));
            match rows.get(n - 1) {
                Some((h, text)) => {
                    state.text.insert(name, label(*h));
                    state.text.insert(data, text.clone());
                    // The names are authored hidden (flag 0x10000000).
                    state.shown.insert(name);
                    if let (Some((t, ts)), Some((d, ds))) = (title, data_box) {
                        let step = ts.y * (n - 1) as f32;
                        state.position.insert(name, Vec2::new(t.x, t.y + step));
                        state.position.insert(data, Vec2::new(d.x + ds.x, d.y + step));
                    }
                }
                // Their INIT chains to HIDE.
                None => {
                    state.hidden.insert(name);
                    state.hidden.insert(data);
                }
            }
        }
        let lap_time = master_box(package, post_race_stats::TIME_MASTER);
        let lap_place = master_box(package, post_race_stats::POSITION_MASTER);
        for n in 1..=10 {
            let (time, place) = (fe_hash(&format!("TIME_DATA_{n}")), fe_hash(&format!("POSITION_DATA_{n}")));
            match r.laps.get(n - 1).filter(|_| self.circuit && !self.drag) {
                Some((t, p)) => {
                    state.text.insert(time, format!("{n}   {}", crate::frontend::hud::time_text(ticks(*t))));
                    state.text.insert(place, p.to_string());
                    state.shown.insert(time);
                    // 0x4BFD40 / 0x52D810: the time's top left at the
                    // time master's, the place centred on the position
                    // master; each row the master's height lower.
                    if let (Some((t, ts)), Some((p, ps))) = (lap_time, lap_place) {
                        state.position.insert(time, Vec2::new(t.x, t.y + ts.y * (n - 1) as f32));
                        state.position.insert(place, Vec2::new(p.x + ps.x * 0.5, p.y + ps.y * (n - 1) as f32));
                    }
                }
                None => {
                    state.hidden.insert(time);
                    state.hidden.insert(place);
                }
            }
        }
        // The masters only place the rows.
        for m in [
            post_race_stats::TITLE_MASTER,
            post_race_stats::DATA_MASTER,
            post_race_stats::TIME_MASTER,
            post_race_stats::POSITION_MASTER,
        ] {
            state.hidden.insert(m);
        }
        if !self.circuit || self.drag {
            state.hidden.insert(fe_hash("HEAD_TEXT_2"));
            state.hidden.insert(fe_hash("HEAD_TEXT_3"));
        }
        // 0x4A4480: the left arrow hides on the first racer, the right on
        // the last (the first wins when there is one).
        let last = self.racers.len().saturating_sub(1);
        if self.index == 0 {
            state.hidden.insert(post_race_stats::LEFT);
        } else if self.index == last {
            state.hidden.insert(post_race_stats::RIGHT);
        }
        self.arrows.apply(&mut state, [post_race_stats::LEFT, post_race_stats::RIGHT], self.opened);
        layout(assets, package, &state, now - self.opened)
    }
}
