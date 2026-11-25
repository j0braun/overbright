//! Authored CAR_AUDIO_PATTERN records (0x432E8C → command 7 at 0x439547).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub frame: f32,
    pub slot: usize,
    pub pattern: i32,
    /// Native stores the authored float; dispatch truncates it to an integer.
    pub parameter: f32,
}

pub fn parse(track: &str) -> Vec<Event> {
    track
        .lines()
        .filter_map(|line| {
            let words: Vec<_> = line.split("//").next()?.split_whitespace().collect();
            if words.len() < 6 || words[0] != "EVENT:" || words[2] != "CAR_AUDIO_PATTERN" {
                return None;
            }
            let number = |s: &str| {
                s.trim_end_matches('f')
                    .parse::<f32>()
                    .ok()
                    .filter(|v| v.is_finite())
            };
            let frame = number(words[1])?;
            let slot = words[3].parse::<usize>().ok()?;
            // Native rejects slot >= 8; zero has no valid bound car.
            if !(1..8).contains(&slot) {
                return None;
            }
            Some(Event {
                frame,
                slot,
                pattern: words[4].parse().ok()?,
                parameter: number(words[5])?,
            })
        })
        .collect()
}

/// The track's records in native order. The loaders (0x432AC0 / 0x432C00)
/// append every parsed record (0x1C bytes, frame at +0x04) and heap-sort the
/// whole array by frame (0x5D4100 with comparator 0x430DB0, ascending). That
/// sort is not stable, but only records of one frame can trade places, and
/// the audio records' observable effect depends only on the order of those
/// for the same slot on the same frame, which no shipped track has (test
/// `shipped_tracks_have_no_same_slot_audio_ties`); so a stable sort by frame
/// gives the same audio. (IntroNis08's authored audio frames are 90, 40,
/// 100, 180, 200, 230, 210.)
pub fn sorted(mut events: Vec<Event>) -> Vec<Event> {
    events.sort_by(|a, b| a.frame.total_cmp(&b.frame));
    events
}

/// One event-track cursor. Re-entry creates a fresh cursor; seeking resets
/// then advances it, as 0x43AF80 clears +0x0C/+0x10 before 0x4395F0.
#[derive(Default, Debug)]
pub struct Cursor {
    next: usize,
}
impl Cursor {
    pub fn advance<'a>(&mut self, events: &'a [Event], seconds: f32) -> &'a [Event] {
        let start = self.next;
        // 0x43960F / 0x43961E: event seconds strictly less than clock.
        while events.get(self.next).is_some_and(|e| e.frame * 0.03333333507180214_f32 < seconds) {
            self.next += 1;
        }
        &events[start..self.next]
    }
}

/// Scene-owned event clock and cursor, stepped at the native audio cadence.
/// Each audio update first dispatches the events the scene's event clock
/// (scene +0x130, the f32 sum of the frame times) has crossed, then the
/// sound contexts update (0x483B40). A context takes an event only if it is
/// scripted at that update (the context update clears the pending byte
/// either way, 0x479630 / 0x4795A6). The caller owns the car contexts'
/// patterns, which survive the scene.
///
/// The event clock adds the native frame lengths, not the ideal period:
/// 0x57EAE0 turns each frame into whole 1/1800 s ticks with a carry kept in
/// 1/4000 s units (`frame_clock::FrameClock`), so 1/30 s scene frames run
/// 60, 60, 60, 60, 61 ticks and an audio update after a start scene's cut
/// (two 1/120 s frames) 30 or 31. Every captured scene clock replays bit
/// for bit with this model (test `native_event_clocks_follow_the_frame_ticks`);
/// the 61-tick frames are why some events cross one update earlier than at
/// ideal pacing.
#[derive(Default, Debug)]
pub struct Run {
    clock: crate::audio::car::scripted::StepClock,
    seconds: f32,
    cursor: Cursor,
    /// The frame limiter's carry (0x865144) as the scene's frames leave it.
    pub frames: crate::frame_clock::FrameClock,
    /// Whether the scene held the cars at the end of the last frame: the
    /// frame cap (and so the audio period) of the frame that follows.
    pub held: Option<bool>,
    /// Audio updates run so far.
    pub updates: usize,
}

/// The native frames behind one audio update of `period`: one 1/30 s frame
/// while the scene holds the cars, else two 1/120 s frames (0x483B40 runs
/// the contexts once 15 ms passed). Returns the event clock after them.
pub fn native_update(frames: &mut crate::frame_clock::FrameClock, seconds: f32, period: f32) -> f32 {
    use crate::audio::car::scripted::SCENE_PERIOD;
    if period >= SCENE_PERIOD {
        seconds + frames.frame_limited(SCENE_PERIOD, SCENE_PERIOD).0
    } else {
        const FREE_FRAME: f32 = 1.0 / 120.0;
        let a = seconds + frames.frame_limited(FREE_FRAME, FREE_FRAME).0;
        a + frames.frame_limited(FREE_FRAME, FREE_FRAME).0
    }
}

impl Run {
    /// A run whose frame limiter starts with `carry` (the world's 0x865144
    /// when the scene starts: it is one global in the original).
    pub fn with_carry(carry: f32) -> Run {
        Run { frames: crate::frame_clock::FrameClock { carry }, ..Default::default() }
    }

    /// Advance by `dt` of scene time with native audio updates every
    /// `period` (`scripted::SCENE_PERIOD` while the scene holds the cars,
    /// else `FREE_PERIOD`); `update` gets each update's crossed events.
    pub fn advance(&mut self, events: &[Event], dt: f32, period: f32, mut update: impl FnMut(&[Event])) {
        for _ in 0..self.clock.advance(dt, period) {
            self.step(events, period, &mut update);
        }
    }

    /// One audio update of `period` now, whatever the accumulated time (the
    /// scene's last frame: under Wine the update in the frame whose clock
    /// passed the scene's length still ran scripted).
    pub fn step(&mut self, events: &[Event], period: f32, mut update: impl FnMut(&[Event])) {
        self.seconds = native_update(&mut self.frames, self.seconds, period);
        self.updates += 1;
        update(self.cursor.advance(events, self.seconds));
    }

    /// The scene's event clock (seconds).
    pub fn seconds(&self) -> f32 {
        self.seconds
    }
}

/// Apply one update's events to the context bound to `slot`, if that
/// context is scripted now.
pub fn dispatch(events: &[Event], slot: usize, scripted: bool, pattern: &mut crate::audio::car::scripted::Pattern) {
    if scripted {
        for e in events.iter().filter(|e| e.slot == slot) {
            pattern.start_id(e.pattern);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::car::scripted::{FREE_PERIOD, Pattern, SCENE_PERIOD};

    #[test]
    fn authored_records_are_parsed_and_invalid_slots_dropped() {
        let events = parse(
            "EVENT: 35.0 CAR_AUDIO_PATTERN 1 3 1.0f\nEVENT: 42 CAR_AUDIO_PATTERN 2 12 1.5f // kept\nEVENT: 43 CAR_AUDIO_PATTERN 8 3 1\nEVENT: 44 CAR_AUDIO_PATTERN 0 3 1\nEVENT: NaN CAR_AUDIO_PATTERN 1 3 1\n",
        );
        assert_eq!(
            events,
            vec![Event { frame: 35.0, slot: 1, pattern: 3, parameter: 1.0 }, Event { frame: 42.0, slot: 2, pattern: 12, parameter: 1.5 }]
        );
    }

    #[test]
    fn crossing_is_strict_once_and_fresh_cursor_replays_on_seek() {
        let events = sorted(parse("EVENT: 42 CAR_AUDIO_PATTERN 2 7 1\nEVENT: 35 CAR_AUDIO_PATTERN 1 3 1\nEVENT: 35 CAR_AUDIO_PATTERN 1 12 1"));
        assert_eq!(events.iter().map(|e| e.pattern).collect::<Vec<_>>(), [3, 12, 7], "frame order, ties as authored");
        let mut cursor = Cursor::default();
        let at = 35.0 * 0.03333333507180214_f32;
        assert!(cursor.advance(&events, at).is_empty());
        assert_eq!(cursor.advance(&events, at + 0.001), &events[..2]);
        assert!(cursor.advance(&events, at + 0.001).is_empty());
        assert_eq!(cursor.advance(&events, 2.0), &events[2..]);
        let mut reentry = Cursor::default();
        assert_eq!(reentry.advance(&events, 2.0), &events);
    }

    /// One scene's native records (`docs/evidence/nis-audio-patterns`).
    struct Native {
        scene: String,
        events: Vec<Event>,
        contexts: Vec<(usize, Vec<(f32, u32, u32, i32)>)>,
    }

    fn native() -> Vec<Native> {
        let text = &crate::testkit::private_str("docs/evidence/nis-audio-patterns/native-updates.txt");
        let mut out: Vec<Native> = Vec::new();
        let mut lines = text.lines().filter(|l| !l.starts_with('#'));
        while let Some(line) = lines.next() {
            let w: Vec<_> = line.split_whitespace().collect();
            match w[0] {
                "scene" => out.push(Native { scene: w[1].into(), events: Vec::new(), contexts: Vec::new() }),
                "event" => out.last_mut().unwrap().events.push(Event { frame: w[1].parse().unwrap(), slot: w[2].parse().unwrap(), pattern: w[3].parse().unwrap(), parameter: 1.0 }),
                "context" => {
                    let n: usize = w[2].parse().unwrap();
                    let rows = (0..n)
                        .map(|_| {
                            let r: Vec<_> = lines.next().unwrap().split_whitespace().collect();
                            let h = |k: usize| u32::from_str_radix(r[k], 16).unwrap();
                            (f32::from_bits(h(0)), h(1), h(2), r[3].parse().unwrap())
                        })
                        .collect();
                    out.last_mut().unwrap().contexts.push((w[1].parse().unwrap(), rows));
                }
                other => panic!("{other}"),
            }
        }
        out
    }

    /// Every captured update of every scripted context (intros 27 / 07 with
    /// the player and three racers, EndingNis02, DynoEnter, ShopEnter1,
    /// ShopExit1, EndingNis01 lost to an AI with all four racers,
    /// CareerNis1; patterns 1, 3–7, 11, 12, 14 started): the authored track
    /// sorted by frame, crossed against the native event clock, dispatched
    /// to the slot's context, one 0x46FA30 step: rpm / throttle bit for bit.
    /// A context whose first scripted update comes after the scene's first
    /// (the racers in an ending) loses what crossed before it: EndingNis01's
    /// pattern 13 for the AI winner is latched but never started.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn native_scene_updates_replay_bit_for_bit() {
        let scenes = native();
        assert_eq!(scenes.len(), 8);
        let mut started = std::collections::BTreeSet::new();
        for scene in &scenes {
            let events = sorted(scene.events.clone());
            let first = scene.contexts.iter().map(|c| c.1[0].0).fold(f32::INFINITY, f32::min);
            for (slot, rows) in &scene.contexts {
                let (mut cursor, mut pattern) = (Cursor::default(), Pattern::default());
                if rows[0].0 > first {
                    let lost = cursor.advance(&events, first);
                    dispatch(lost, *slot, false, &mut pattern);
                }
                for (i, &(clock, rpm, throttle, id)) in rows.iter().enumerate() {
                    let crossed = cursor.advance(&events, clock);
                    started.extend(crossed.iter().filter(|e| e.slot == *slot).map(|e| e.pattern));
                    dispatch(crossed, *slot, true, &mut pattern);
                    pattern.step();
                    let at = format!("{} slot {slot} update {i} (pattern {id})", scene.scene);
                    assert_eq!(pattern.rpm.to_bits(), rpm, "rpm {at}");
                    assert_eq!(pattern.throttle.to_bits(), throttle, "throttle {at}");
                }
            }
        }
        assert_eq!(started.into_iter().collect::<Vec<_>>(), [1, 3, 4, 5, 6, 7, 11, 12, 14]);
    }

    /// The native cadence: 30 updates a second while the scene holds the
    /// cars (the start scenes up to their camera cut, the shop / dyno /
    /// ending scenes throughout), each event clock step at least 1/30 s.
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn native_scene_updates_follow_the_scene_frame_cap() {
        for scene in native() {
            let first = scene.contexts.iter().map(|c| c.1[0]).fold((f32::INFINITY, 0, 0, 0), |a, b| if b.0 < a.0 { b } else { a });
            for (slot, rows) in &scene.contexts {
                let rows = &if rows[0].0 > first.0 { [&[first][..], &rows[..]].concat() } else { rows.clone() };
                // The scene's first frame: 1/30 s, or one 1/1800 s tick more
                // (0x57EAE0's tick rounding).
                assert!(rows[0].0 == SCENE_PERIOD || (rows[0].0 - 61.0 / 1800.0).abs() < 1e-6, "{} slot {slot}: first update at {}", scene.scene, rows[0].0);
                let capped = rows.iter().take_while(|r| r.0 <= 6.0).collect::<Vec<_>>();
                for w in capped.windows(2) {
                    assert!(w[1].0 - w[0].0 >= SCENE_PERIOD * 0.999, "{} slot {slot}", scene.scene);
                }
            }
        }
    }

    /// The clone's run: an event crosses on the first update whose event
    /// clock (the native frames' ticks, 60, 60, 60, 60, 61 from a zero
    /// carry) exceeds frame × f32(1/30), as under Wine: frame 60 crosses on
    /// update 60 (index 59), not 61 as a sum of ideal 1/30 s would; a run
    /// after a cut takes 60 updates a second.
    #[test]
    fn run_dispatches_on_the_native_update_at_any_render_rate() {
        let events = sorted(parse("EVENT: 0 CAR_AUDIO_PATTERN 1 5 1.0f\nEVENT: 60 CAR_AUDIO_PATTERN 4 6 1.0f\nEVENT: 30 CAR_AUDIO_PATTERN 2 11 1.0f"));
        for fps in [30, 60, 120, 144, 240, 360] {
            let mut run = Run::default();
            let mut seen = Vec::new();
            let mut updates = 0;
            for _ in 0..fps * 3 {
                run.advance(&events, 1.0 / fps as f32, SCENE_PERIOD, |crossed| {
                    seen.extend(crossed.iter().map(|e| (updates, e.pattern)));
                    updates += 1;
                });
            }
            assert_eq!(updates, 90, "FPS {fps}");
            assert_eq!(seen, [(0, 5), (29, 11), (59, 6)], "FPS {fps}");
            let mut after = 0;
            for _ in 0..fps {
                run.advance(&events, 1.0 / fps as f32, FREE_PERIOD, |_| after += 1);
            }
            assert_eq!(after, 60, "FPS {fps}");
        }
    }

    /// Every captured scene clock is the native frame clock: from the carry
    /// the scene started with (a multiple of 1/12000 s in the captures; the
    /// frames before the scene decide it), 1/30 s frames through 0x57EAE0
    /// while the scene holds the cars, then two 1/120 s frames an update
    /// (IntroNis27 after its cut, from the carry the cut's frame left). The
    /// sums match bit for bit; only IntroNis07's Wine hitches (a 63-tick
    /// frame among others) are outside the model, so it is checked up to
    /// its first.
    #[test]
    #[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
    fn native_event_clocks_follow_the_frame_ticks() {
        use crate::frame_clock::FrameClock;
        // `n` model updates of `period` from `clock` with the limiter's `carry`.
        let model = |clock: f32, carry: f32, period: f32, n: usize| {
            let (mut frames, mut clock) = (FrameClock { carry }, clock);
            (0..n).map(|_| { clock = native_update(&mut frames, clock, period); clock.to_bits() }).collect::<Vec<u32>>()
        };
        // The longest prefix of `rows` some carry reproduces (`skip` model
        // updates first: an ending's racers start on the second).
        let best = |rows: &[u32], from: f32, period: f32, skip: usize, step: f32| {
            (-12..=12)
                .map(|k| {
                    let m = model(from, k as f32 * step, period, rows.len() + skip);
                    m[skip..].iter().zip(rows).take_while(|(a, b)| a == b).count()
                })
                .max()
                .unwrap()
        };
        let mut checked = 0;
        for scene in native() {
            let first = scene.contexts.iter().map(|c| c.1[0].0).fold(f32::INFINITY, f32::min);
            for (slot, rows) in &scene.contexts {
                let clocks: Vec<u32> = rows.iter().map(|r| r.0.to_bits()).collect();
                let skip = usize::from(rows[0].0 > first);
                // Held (1/30 s) updates up to the first shorter step (the cut).
                let held = 1 + rows.windows(2).take_while(|w| w[1].0 - w[0].0 > 0.025).count();
                let ok = best(&clocks[..held], 0.0, SCENE_PERIOD, skip, 1.0 / 12000.0);
                // Wine hitches (frames over 61 ticks: IntroNis07's second
                // update, loads early in EndingNis01, CareerNis1 under machine
                // load) are outside the model: up to the first one.
                let regular = rows[..held].windows(2).take_while(|w| ((w[1].0 - w[0].0) * 1800.0).round() <= 61.0).count() + 1;
                let regular = if rows[0].0 * 1800.0 > 61.5 + 61.0 * skip as f32 { 0 } else { regular };
                if regular < held {
                    assert!(ok >= regular, "{} slot {slot}: {ok} < {regular}", scene.scene);
                    checked += ok;
                    continue;
                }
                // After the cut: two 1/120 s frames an update, from the carry
                // the cut's frame left (under Wine a frame ~0.17 ms long).
                let rest = if held < clocks.len() { best(&clocks[held..], rows[held - 1].0, FREE_PERIOD, 0, 1.0 / 48000.0) } else { 0 };
                assert_eq!(ok + rest, clocks.len(), "{} slot {slot}: held {ok}/{held}", scene.scene);
                checked += clocks.len();
            }
        }
        assert!(checked > 2000, "{checked}");
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn unscripted_context_drops_its_event() {
        let events = parse("EVENT: 0 CAR_AUDIO_PATTERN 4 6 1.0f");
        let mut p = Pattern::default();
        dispatch(&events, 4, false, &mut p);
        p.step();
        assert_eq!((p.rpm, p.throttle), (1000.0, 0.0));
        dispatch(&events, 3, true, &mut p);
        p.step();
        assert_eq!((p.rpm, p.throttle), (1000.0, 0.0));
    }

    /// The loaders' heap sort (0x5D4100) is unstable; the stable sort is
    /// equivalent for audio because no shipped track has two audio records
    /// for one slot on one frame.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn shipped_tracks_have_no_same_slot_audio_ties() {
        let root = crate::testkit::game_root();
        let dir = std::path::Path::new(&root).join("NIS");
        let mut tracks = 0;
        let mut commands = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if !path.to_string_lossy().ends_with("_EvtTrk.txt") {
                continue;
            }
            let events = sorted(parse(&std::fs::read_to_string(&path).unwrap()));
            tracks += !events.is_empty() as usize;
            commands += events.len();
            for w in events.windows(2) {
                assert!(!(w[0].frame == w[1].frame && w[0].slot == w[1].slot), "{path:?}");
            }
            assert!(events.iter().all(|e| (1..=15).contains(&e.pattern) && e.parameter == 1.0 && (1..=4).contains(&e.slot)), "{path:?}");
        }
        assert_eq!((tracks, commands), (25, 147));
    }
}
