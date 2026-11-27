//! Pieces of [`Hud::quads`], called in its order (see there).

use super::*;

impl Hud {
    /// 0x4AA1B0: laps and lap times.
    pub(super) fn lap_info(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { assets, package, input, lo, .. } = *f;
        let s = &mut self.state;
        let hide = |s: &mut ScreenState, name: &str| {
            s.hidden.insert(fe_hash(name));
        };
        let text = |s: &mut ScreenState, name: &str, t: String| {
            s.text.insert(fe_hash(name), t);
        };
        let label = |h: u32| assets.strings.get(h).unwrap_or_default().to_string();
        let has = |h: u32| package.objects.iter().any(|o| o.hash == h);
        // 0x4AA1B0: laps and lap times. In race state 7 (0x4AA202 →
        // 0x4AA352) the lap count goes: NumberLaps (+0x24), LapNumber
        // (+0x20), the first LapCounterBacking (+0x40), Lap_Time_Title
        // (+0x34), Lap_Time_Text (+0x38) and LapText (+0x30) hide, and only
        // the times show.
        if lo & 0x508 != 0 && input.outrun.is_some() {
            for n in ["NumberLaps", "LapNumber", "Lap_Time_Title", "Lap_Time_Text", "LapText"] {
                hide(s, n);
            }
            if let Some(h) = (0..=16).map(|n| fe_hash(&format!("LapCounterBacking_{n}"))).find(|&h| has(h)) {
                s.hidden.insert(h);
            }
            text(s, "CurrentLapTime", time_text(ticks(input.clock.max(0.0) - input.lap_start)));
            text(s, "BestLapTime", "--:--:--".to_string());
        } else if lo & 0x508 != 0 {
            match input.sprint {
                Some(done) => {
                    text(s, "NumberLaps", format!("{:3}%", (done * 100.0) as i32));
                    hide(s, "LapNumber");
                    text(s, "LapText", label(COMPLETED));
                }
                None => {
                    text(s, "NumberLaps", format!("/{:2}", input.laps));
                    text(s, "LapNumber", format!("{:2}", input.lap + 1));
                }
            }
            let since = input.last_lap.map(|(_, at)| input.clock - at);
            if input.clock > 0.0 {
                // The finished lap's time stays up for 3 s, then the
                // running one; its title and time show for 2 s.
                let shown = match (input.last_lap, since) {
                    (Some((t, _)), Some(d)) if d <= 3.0 => t,
                    _ => input.clock - input.lap_start,
                };
                text(s, "CurrentLapTime", time_text(ticks(shown)));
                match (input.last_lap, since) {
                    (Some((t, _)), Some(d)) if d < 2.0 => {
                        let best = input.laps_done > 1 && input.best_lap.is_some_and(|b| b >= t);
                        text(s, "Lap_Time_Text", time_text(ticks(t)));
                        text(s, "Lap_Time_Title", label(if best { BEST_LAP_TIME } else { LAP_TIME }));
                    }
                    _ => {
                        hide(s, "Lap_Time_Text");
                        hide(s, "Lap_Time_Title");
                    }
                }
            } else {
                text(s, "CurrentLapTime", label(NO_LAP_TIME));
                hide(s, "Lap_Time_Text");
                hide(s, "Lap_Time_Title");
            }
            text(s, "BestLapTime", input.best_lap.map_or_else(|| "--:--:--".to_string(), |b| time_text(ticks(b))));
        }
    }

    /// 0x4C85D0: the drift readouts.
    pub(super) fn drift_readouts(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { assets, input, now, .. } = *f;
        let s = &mut self.state;
        let hide = |s: &mut ScreenState, name: &str| {
            s.hidden.insert(fe_hash(name));
        };
        let text = |s: &mut ScreenState, name: &str, t: String| {
            s.text.insert(fe_hash(name), t);
        };
        let label = |h: u32| assets.strings.get(h).unwrap_or_default().to_string();
        // 0x4C85D0 (with 0x4AB780 and 0x4AB340): the drift readouts.
        if let Some(d) = &input.drift {
            text(s, "BestDrift", grouped_int(d.best));
            // The drift's popup: its name and points, the crash colour
            // once lost, the drift colour while it runs.
            if d.running || d.banked_now || d.lost {
                text(s, "SP_Drift_Title", label(fe_ids::DRIFT_LABEL));
                text(s, "SP_Drift_Point", grouped_int(d.points));
                let colour = if d.lost {
                    0xFFCF_3030
                } else if d.bonus {
                    0xFFFF_8020
                } else {
                    0xFFCA_F48A
                };
                s.color.insert(fe_hash("SP_Drift_Title"), argb(colour));
                s.color.insert(fe_hash("SP_Drift_Point"), argb(colour));
            } else {
                hide(s, "SP_Drift_Title");
                hide(s, "SP_Drift_Point");
            }
            text(s, "SP_DRIFT_TOTAL", grouped_int(d.banked));
            if d.banked_now {
                s.run(fe_hash("SP_DRIFT_TOTAL"), "Highlight", now);
                s.run(fe_hash("Style_TotalDrift_Title"), "Highlight", now);
            }
            text(s, "drift_multiplier_text", format!("X{}", grouped_int(d.level as i32)));
            if d.next < 1 {
                text(s, "SpeedTrap_Speed_DATA", "----".to_string());
                text(s, "NextSpeeed_Trap_Indicator", label(0xCC62_C8BE));
            } else {
                text(s, "SpeedTrap_Speed_DATA", grouped_int(d.next));
                let gain = label(0xE03E_F7A4).replace("%d", &(d.level + 1).to_string());
                text(s, "NextSpeeed_Trap_Indicator", gain);
            }
            s.color.insert(fe_hash("SpeedTrap_Speed_DATA"), argb(0xFFAA_8000));
            // 0x4DA590: the angle meter (the needle turned to angle + 90°),
            // and "too slow" while the level falls or may not rise
            // (0x4AB420 runs LosingMult / TooSlowToGain).
            text(s, "Angle_Indicator_DATA", format!("{}", d.angle as i32));
            s.angle.insert(fe_hash("3rdperson_driftometer"), (d.angle + 90.0).to_radians());
            let slow = fe_hash("SlowMessage");
            if d.falling || d.held {
                let script = if d.falling { "LosingMult" } else { "TooSlowToGain" };
                if self.groups.get(&slow) != Some(&script) {
                    s.run(slow, script, now);
                    self.groups.insert(slow, script);
                }
            } else {
                s.hidden.insert(slow);
                self.groups.remove(&slow);
            }
            // 0x4AB2B0: WrongWay_CountDown (element +0x70) shows "%$1.1f"
            // of +0x8C less the time since +0x90 while that is over 0, else
            // hides; +0x8C is only ever stored 0 (0x4F6C7B, 0x4AB2F0), so it
            // stays hidden.
            hide(s, "WrongWay_CountDown");
            // A tier's name shows for 2 s (0x4AC120, 0x4AB340).
            match d.announce {
                Some((name, since)) if since <= 2.0 => text(s, "Comment_DATA", label(name)),
                _ => hide(s, "Comment_DATA"),
            }
        }
    }

    /// 0x4AAFC0: the wrong-way sign.
    pub(super) fn wrong_way_sign(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { input, lo, now, .. } = *f;
        let s = &mut self.state;
        let hide = |s: &mut ScreenState, name: &str| {
            s.hidden.insert(fe_hash(name));
        };
        // 0x4AAFC0: the wrong-way sign fades in and out.
        hide(s, "WrongWay_CountDown");
        if lo & 0x4000 != 0 && input.wrong_way != self.wrong_way {
            s.run(fe_hash("WrongWayImage"), if input.wrong_way { "FADEIN" } else { "FADEOUT" }, now);
            self.wrong_way = input.wrong_way;
        }
    }

    /// 0x4C7F00: the race-over message.
    pub(super) fn race_over_message(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { assets, input, now, .. } = *f;
        let s = &mut self.state;
        let text = |s: &mut ScreenState, name: &str, t: String| {
            s.text.insert(fe_hash(name), t);
        };
        let label = |h: u32| assets.strings.get(h).unwrap_or_default().to_string();
        // 0x4C7F00: race over, "Your time: <time>" for a lone racer. Its
        // first frame runs RACEOVER on the message and, in race state 7,
        // 0x36819D93 on the backing instead of RACEOVER. An outrun's racer
        // status 6 shows "You Won the Outrun!", "Your time:" and in a career
        // "Bank Reward:" with the stage's bank ("%s\n%s %s\n%s %$d"),
        // status 7 "You Lost the Outrun!" and the time ("%s\n%s %s").
        if let Some(time) = input.finished {
            if !self.race_over {
                s.run(fe_hash("RaceOverMessage"), "RACEOVER", now);
                if input.outrun_result.is_some() {
                    s.scripts.insert(fe_hash("RaceOverBacking"), (0x3681_9D93, now));
                } else {
                    s.run(fe_hash("RaceOverBacking"), "RACEOVER", now);
                }
                self.race_over = true;
            }
            let time = time_text(ticks(time));
            let line = match input.outrun_result {
                Some((true, Some(bank))) => format!(
                    "{}\n{} {}\n{} {}",
                    label(crate::outrun::WON),
                    label(YOUR_TIME),
                    time,
                    label(crate::outrun::BANK_REWARD),
                    grouped(i64::from(bank))
                ),
                Some((true, None)) => format!("{}\n{} {}", label(crate::outrun::WON), label(YOUR_TIME), time),
                Some((false, _)) => format!("{}\n{} {}", label(crate::outrun::LOST), label(YOUR_TIME), time),
                // By the racer's status (0x4C7F00's switch on racer
                // +0x10): a drop-out's own line; a finish in a race of more
                // than one racer the place line (0x4AB4B0: "You were
                // last!" 0xEAC720D3 for the last place, race status +0x24,
                // else 0x7F66B4[place]: Winner!, You finished second! …)
                // over "Your time:" and the time ("%s\n%s %s"). Drift
                // races (0x401910) and the 0x42FC40 case show a score
                // instead (not ported here).
                None => match input.status {
                    // Case 1 in a drift race (0x401910, 0x4C80C7): the
                    // place line over "Your score:" (0x13640DE9) and the
                    // style manager's DRIFT trick banked points (racer
                    // +0x1D4 → [0] +0x2C, _ftol) as "%s\n%s %$d"; a lone
                    // racer "%s %$d". The 0x42FC40 case is not traced.
                    1 if input.drift.is_some() => {
                        let score = format!(
                            "{} {}",
                            label(fe_ids::TXT_YOUR_SCORE),
                            grouped(i64::from(input.drift.as_ref().map_or(0, |d| d.banked)))
                        );
                        if input.field > 1 {
                            let place = if input.place >= input.racers {
                                fe_ids::OBJ_EAC720D3
                            } else {
                                PLACE_LINES.get(input.place as usize).copied().unwrap_or(fe_ids::OBJ_A074449F)
                            };
                            format!("{}\n{}", label(place), score)
                        } else {
                            score
                        }
                    }
                    3 => label(0x1B59_940C),
                    8 => label(0x7449_D26D),
                    9 => label(0x4BA0_D22F),
                    11 => label(0x668C_7453),
                    _ if input.racers > 1 && input.drift.is_none() => {
                        // Outside drift races 0x4AB4B0 reads the finish
                        // order (racer +0x0C), not the place (+0x0A).
                        let order = if input.finish_order > 0 { input.finish_order } else { input.place };
                        let place = if order >= input.racers {
                            fe_ids::OBJ_EAC720D3
                        } else {
                            PLACE_LINES.get(order as usize).copied().unwrap_or(fe_ids::OBJ_A074449F)
                        };
                        format!("{}\n{} {}", label(place), label(YOUR_TIME), time)
                    }
                    _ => format!("{} {}", label(YOUR_TIME), time),
                },
            };
            text(s, "RaceOverMessage", line);
        } else {
            // Ready for the next race-over (an outrun's in free roam keeps
            // the same HUD).
            self.race_over = false;
        }
    }

    /// 0x4AB5B0: the countdown.
    pub(super) fn countdown(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { assets, package, input, hi, .. } = *f;
        let s = &mut self.state;
        let label = |h: u32| assets.strings.get(h).unwrap_or_default().to_string();
        // 0x4AB5B0: the countdown, 3, 2, 1 a step each (0x5EEA50; the step
        // is race status +0xC, 0.9 s from 0x5FC740), then "GO!" for a step
        // from the green light; each grows from 1 to 3 times its size and
        // fades in as sin(90° × the step's fraction).
        if hi & 0x2_0000 != 0 {
            let step_s = if input.countdown_step > 0.0 { input.countdown_step } else { 0.9 };
            // An outrun starts at its "GO!": 0x607190 sets race status +4
            // (the green tick) to now and +8 to 0, so 0x5EEA50 gives 4 for
            // +0xC (0.9 s, 0x5FC740's 0x7A5690 — free roam plays no scene
            // to change it) from the challenge; `clock` is its race time.
            let step = if input.finished.is_some() || input.no_countdown {
                None
            } else if input.clock < 0.0 {
                let left = -input.clock / step_s;
                Some(((left.ceil() as i32).clamp(1, 3), 1.0 - (left - left.ceil() + 1.0)))
            } else if input.clock < step_s {
                Some((4, input.clock / step_s))
            } else {
                None
            };
            match step {
                Some((n, f)) => {
                    let f = f.clamp(0.0, 1.0);
                    s.text.insert(COUNTDOWN, if n == 4 { label(0x535) } else { n.to_string() });
                    if let Some(o) = package.objects.iter().find(|o| o.hash == COUNTDOWN) {
                        let mut c = o.attributes.color();
                        c[3] = ((f * 90.0).to_radians().sin() * 255.0) as i32 as f32 / 255.0;
                        s.color.insert(COUNTDOWN, c);
                    }
                    s.size.insert(COUNTDOWN, glam::Vec2::splat(2.0 * f + 1.0));
                }
                None => {
                    s.hidden.insert(COUNTDOWN);
                }
            }
        }
    }

    /// 0x4EBB10: place, racers, race time and the hurry counter.
    pub(super) fn race_position(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { assets, input, lo, now, .. } = *f;
        let s = &mut self.state;
        let hide = |s: &mut ScreenState, name: &str| {
            s.hidden.insert(fe_hash(name));
        };
        let text = |s: &mut ScreenState, name: &str, t: String| {
            s.text.insert(fe_hash(name), t);
        };
        let label = |h: u32| assets.strings.get(h).unwrap_or_default().to_string();
        // 0x4EBB10 (offline, not drift): the place "%d" and its suffix, "/%d"
        // racers, the race time; the hurry counter hides. The race time
        // follows the race clock but holds for 3 s from the frame racer
        // status +0x0D is set: the frame the racer completes a lap it has
        // not done before (0x5F7C40 clears it each frame and sets it when
        // +0x08 passes +0x09 at the finish checkpoint).
        if lo & 0x200 != 0 {
            if input.laps_done > self.laps_seen {
                self.laps_seen = input.laps_done;
                self.lap_at = Some(now);
            }
            if self.lap_at.is_none_or(|at| now - at >= 3000.0) {
                self.race_time = input.race_time;
            }
            // The drift branch: Hurry_Counter shows the time left once
            // race status +0x20 ≥ 0 (0x577250 flags 0 of _ftol(t × 4000 +
            // 0.5) ticks), and each frame whose time left is within a
            // frame time (0x865150) above an even whole second (_ftol, so
            // 30, 28, …) queues "Hurry!" (0x862A0519) on Hurry_Text, which
            // the queue (0x4EB9F0, made by 0x4F4AC0: 4 slots, 1.5 s each)
            // shows for 1.5 s. The ctor hides both objects. 0x5F0100 on
            // 0x89CF48 (which skips the counter) reads the online counters
            // 0x89CF69 / 0x8669F4 / 0x866A14, false offline (A-frontend-1b
            // 2-010 (e)). 0x865150 is the frame's world step, 0 while
            // paused: the clone takes how far the hurry clock moved since
            // the HUD's last update (one 1/60 s tick at 60 fps, as native;
            // 0 between ticks and while paused), not the drawn frame's
            // wall time, so "Hurry!" queues once per even second at any
            // display rate and never on resuming from a pause.
            match input.hurry.filter(|_| input.drift.is_some()) {
                Some(t) => {
                    text(s, "Hurry_Counter", time_text((t * 4000.0 + 0.5) as i32));
                    if hurry_due(self.hurry_prev, t) {
                        self.hurry_until = Some(now + 1500.0);
                    }
                    self.hurry_prev = Some(t);
                }
                None => {
                    self.hurry_prev = None;
                    hide(s, "Hurry_Counter");
                }
            }
            match self.hurry_until {
                Some(until) if now < until => text(s, "Hurry_Text", label(fe_ids::TXT_HURRY)),
                _ => hide(s, "Hurry_Text"),
            }
            text(s, "RacePositionNumber", input.place.to_string());
            text(s, "RacePositionNth", label(nth(input.place)));
            text(s, "NumberRacers", format!("/{}", input.racers.saturating_sub(input.knocked_out)));
            // The drift branch of 0x4EBB10 writes the place, suffix and
            // count (the points list's) but not +0x38; the original under
            // Wine still shows "Time:" counting the race clock under the
            // leader board in a drift race (its writer not traced), so the
            // clone keeps the race time.
            text(s, "RaceTime", time_text(ticks(self.race_time)));
        }
    }

    /// 0x4C9210: a world challenge's time left.
    pub(super) fn challenge_timer(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { input, hi, now, .. } = *f;
        let s = &mut self.state;
        let text = |s: &mut ScreenState, name: &str, t: String| {
            s.text.insert(fe_hash(name), t);
        };
        // 0x4C9210 (element 0x4F86D0, HUD +0x64, high 0x4): a world
        // challenge's time left (0x85B944) above 0 as ftol(t × 4000 + 0.5)
        // ticks (0x577250 flags 4) on CurrentLapTime_2, which runs
        // Animate_RED under 10 s, else Animate, unless already running it
        // (0x51D050 / 0x51CF80).
        if hi & 0x4 != 0
            && let Some(t) = input.challenge.filter(|&t| t > 0.0)
        {
            let h = fe_hash("CurrentLapTime_2");
            text(s, "CurrentLapTime_2", time_text((t * 4000.0 + 0.5) as i32));
            let script = if t < 10.0 { "Animate_RED" } else { "Animate" };
            if s.scripts.get(&h).is_none_or(|&(name, _)| name != fe_hash(script)) {
                s.run(h, script, now);
            }
        }
    }
}

/// 0x4EBB10's "Hurry!" test: the time left `t` within the step 0x865150
/// above an even whole second (`_ftol(t)` even); the step is how far the
/// hurry clock moved since `prev` (the last update's), 0 without one.
pub(super) fn hurry_due(prev: Option<f32>, t: f32) -> bool {
    let step = prev.map_or(0.0, |p| (p - t).max(0.0));
    let n = t as i32;
    n % 2 == 0 && (t - n as f32).abs() < step
}
