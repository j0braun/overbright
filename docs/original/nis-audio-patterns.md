# NIS car-audio patterns (CAR_AUDIO_PATTERN) — 2026-11-04

All 147 authored `CAR_AUDIO_PATTERN` commands (25 event tracks, IDs 1–15,
parameter always 1.0) now drive the car sound contexts natively, except
DynoExit's (no clone flow). Code: `src/audio/car/scripted.rs` (banks, step,
cadence), `src/shop_scene/audio.rs` (order, clock, dispatch),
`Game::engine_sound` / `Game::world_sfx_input` (binding, scripted gates).
Evidence: `docs/evidence/nis-audio-patterns/` (vanilla SPEED2.EXE under Wine,
no WidescreenFix). Earlier slices: `docs/evidence/nis-audio-pattern{,4}`.

## Traced (verified)
* **Event order.** The loaders (0x432AC0 / 0x432C00) append every record and
  heap-sort the whole track by frame (0x5D4100, comparator 0x430DB0). The
  sort is unstable, but no shipped track has two audio records for one slot
  on one frame (test), so a stable frame sort gives the same audio.
* **Crossing.** NISEvents_Advance 0x4395F0 adds the frame's dt to the event
  clock (scene +0x130) and dispatches records with frame × f32(1/30)
  (0x786460) **strictly** below it. The audio update follows in the same
  frame. Observed: every pattern start in the captures falls on the first
  update whose recorded clock passes the event (0 exceptions).
* **Dispatch.** 0x4392E0 type 0xD: slot → 0x827BA8 + 0x14·(slot − 1) →
  world car index → car +0x968 → context vtable +0x48 (player 0x469BC0, racer
  0x469980, both to 0x459550): command 7 latches ID +0xC4 / pending +0xC0.
  Both context updates (0x479610 / 0x479590) clear the pending byte after
  the controllers run, so an event reaching an unscripted context is lost.
* **Binding** (0x4328B0, already in formats.md): start scenes (type 0)
  slot n = the racer on grid slot n (0x432860; the 0x865040 / 0x865044
  swaps have no writer); types 2–4 (dyno, shops) grid slots then the human
  to slot 1 (0x4307F0); endings (type 1) the results order (0x430960), then
  the human to entry +6 on a win, +7 on a loss (all entries: 1 / 2), so
  slot 1 is the winner.
* **Scripted** (Physics +0xD8, one global test per update path):
  player path 0x484C50 (Physics vtable +0x14): NIS 0x827E88 +0x40 or +0x28,
  or +0x10 unless (Physics +0xD0 and race status ∈ {2, 3, 4}), or 0x82A3A4 >
  0. Racer path 0x47C490 (+0x3C): the same with "status ≠ 4" and no +0xD0
  test. Observed: the player is scripted in shop, dyno and ending scenes and
  in a start scene only up to the camera cut (last scripted update at clock
  5.987 / 5.989 s for a cut at 6.0 s); the racers through the whole start
  scene; during an ending NIS +0x28 is set and every context is scripted.
* **Banks** (0x46FA30): ID 2..15 via the jump table 0x46FF64, any other ID
  the default bank 1 (0x7F0070, 72 segments). Quarter-segment linear steps,
  idle −500 rpm / −15 after the last segment, clamps 1000..10000 / 0..100;
  writes ctx +0x48 rpm / +0x44 throttle. All 15 tables are compiled in and
  checked against the executable's bytes (test).
* **Cadence.** AudioManager_Update 0x483B40 runs the contexts only when more
  than 15 ms (0x787AF4) of real time passed since its last run. Every NIS
  scene caps the frame at 1/30 s while it holds the cars (0x435F40), so one
  update per 1/30 s; otherwise (1/120 s frames) one per two frames, 1/60 s.
  Observed: shop / dyno / ending / start-scene updates at 30 a second (event
  clock steps 1/30 s, or 61/1800 s when a frame ran over), ~60 after a start
  scene's cut. **Correction:** the 2026-11-03 shop captures were made with
  WidescreenFix's HighFPSCutscenes on (60 fps scenes); vanilla shop scenes
  run their patterns at half that speed, now ported (pattern 3 starts on
  ShopEnter1's 35th update and is cut by the scene's end after 104 + 2).

## Observed under Wine (2026-11-04, display :192, private copy)
10 851 scripted updates captured with an observer hook on both 0x46FA30
calls (bytes restored and verified; 0 lost), all bit-exact with the bank
model: IntroNis08 (racers: 12, 7, 11, 10, 7, 5), IntroNis27 (player 6,
racers 5, 11, 7), IntroNis07 (player 7, racers 6, 11, 5, 12), EndingNis02
(player win: 14), DynoEnter (1), ShopEnter1 (3), ShopExit1 (4). Patterns
started live: 1, 3–7, 10–12, 14. Not seen live: 2, 8, 9, 13, 15 (same code
path, banks checked byte for byte).

## Clone
* `audio::scripted`: `table(id)`, `Pattern::start_id`, `SCENE_PERIOD` /
  `FREE_PERIOD`, `StepClock::advance(dt, period)`.
* `shop_scene::audio`: `sorted`, `Cursor` (strict f32 crossing), `Run`
  (event clock = f32 sum of the native frames' ticks since the audio pass; the frame's period is the cap state at
  its start, `Run::held`), `dispatch` (unscripted contexts drop events).
* `Game::engine_sound`: every scene (CareerNis1 since the audio pass: scripted idle). Player slot:
  grid slot + 1 in a start scene, else `ShopRun::car_slot`; player scripted
  except in a start scene past its cut (`holds_cars`). Racers: one pattern
  per rival (`Audio::ai_patterns`, cleared with each race), scripted during
  the start scene; `world_sfx_input` feeds their rpm / throttle ÷ 100 to the
  AI engines. The ending scene's engine now plays behind the post-race
  screens (Menu mode).

## 2026-11-04: leftovers traced and captured
Evidence: `docs/evidence/s3-audio/` (vanilla SPEED2.EXE, private copy
a private Wine copy, display :202, `--audio-file`; recorder `record.py` of
`nis-audio-patterns`, all bytes restored, 0 lost). The new scenes' updates
are appended to the maintainer's private capture (`fixture.py`).

* **Event clock = native frame ticks (verified, all captures).** The scene
  event clock adds each frame's world dt, `ticks / 1800` from 0x57EAE0 with
  the limiter's period T (0x865558): 1/30 s frames give 60, 60, 60, 60, 61
  ticks (carry kept in 1/4000 s units), two 1/120 s frames 30 or 31 after
  a start scene's cut. With the carry the scene started with (a multiple of
  1/12000 s; it depends on the frames before), every steady capture replays
  bit for bit (IntroNis27 all 4 contexts incl. after the cut, EndingNis02,
  DynoEnter, ShopEnter1, ShopExit1; IntroNis07, EndingNis01 and CareerNis1
  up to their first Wine hitch: frames of 62–120 ticks under machine load).
  Clone: `FrameClock::frame_limited`, `shop_scene::audio::native_update`;
  `Run` sums those (seeded with the world's carry at the scene's start) instead
  of ideal periods, so an event lands on the native update (frame 60 on
  update 60, not 61). Test `native_event_clocks_follow_the_frame_ticks`.
* **ShopExit1's last update (verified).** Native: 120 scripted updates; the
  one in the frame whose clock passes the length still runs. The clone ended
  the scene in `update` before `engine_sound`; the ended run is now kept
  for that frame's audio update (`Game::scene_audio_tail`): 120 updates.
* **CareerNis1 (observed).** 403 scripted updates of the player's context
  (path 0x484C50) over its 405 frames (13.5 s; the 61-tick frames make
  403), all 1000 rpm / 0 throttle (no CAR_AUDIO_PATTERN in its track; three
  more contexts on the racer path likewise idle). The clone now runs the
  player's context scripted there too (was the ordinary engine).
* **An ending lost to an AI (observed).** Quick Race Resort Loop, RX-7,
  instrumented finish with an AI first (`lose.py`: the AI's then the human's
  finish-line tick / laps, as `finish.py`): Event Results over
  **EndingNis01** (the winner an RX-7). All four contexts run scripted at
  30 updates a second behind the results (513 player / 512 per racer
  updates in 17.3 s): the racers only from the scene's **second** update,
  so EndingNis01's frame-0 event for slot 1 (pattern 13) reaches the
  winner's context while it is not scripted: +0xC4 = 13 is latched but the
  pattern never starts (segments 0); every racer and the player idle at
  1000 rpm / 0 throttle. `sndwatch` during the hold: the four CAR
  instances (player + three racers), FX_ROADNOISE, FX_WIND, FX_SKID,
  FX_TURBO_01, FX_Weather and FX_TRAFFIC stay alive
  (`end-events.tsv`). Clone: in an ending the racers' contexts are scripted
  from the second update (`engine_sound`), the mix map runs the world
  scene with the racers (`mix_sounds`: the ending counts as driving; no
  traffic, **inferred**) and the racers' engines play behind the post-race
  screens (`car_sounds` → `OtherCars::update`, AI only): the winner where
  the scene puts it (`ShopRun::companion`), the others where the race left
  them (the kept replay race; none without a recording). The other world
  sounds of the hold (road noise, wind, skid) are **not** ported (open).
  Test `ending_lost_to_an_ai_idles_and_voices_the_racers`.
* **DynoExit (the Dyno Shop's Test Run plays it; docs/notes/parity/scene-follow-ups.md §5.4. The rest of this note is the earlier trace.)** The pick is
  0x436840 case 1 with track 0x1262, entered only from the race flow's mode
  7 (0x603EC0 case 1) with +0x44 set, no replay, and either no dyno session
  (0x89E7E6 clear) or race type 8 with 0x4018F0 false; a dyno session in
  any other race type (the Test Run's 1) clears +0x44 and returns to the
  tuning menu (0x603640(0), 0x601030(10)). The dyno session itself (race
  type 8, 0x526980) goes from its start scene's end straight to mode 10
  (case 3: 0x435950). Under Wine (the finished-career save): Garage → Performance Tuning /
  Dyno → DynoEnter (278 updates) → Dyno Run → chart → Dyno Results → Dyno
  Shop → Quit Tuning → "Quit to Garage?" Yes: no further scripted update
  and no scene (`dyno.json.zlib`). **Open**: no path into mode 7 with race
  type 8 was found, so DynoExit stays unrouted (the clone's
  `play_race_ending` already returns on track 0x1262).

## Open / deviations
* DynoExit: see above (no native trigger found). The sound heard on leaving
  the dyno is the garage's car swing, FE id 69 (L1, internal notes;
  ported).
* Ending scenes: the player's road / wind / skid sounds run behind the results
  until the scene ends (docs/notes/parity/scene-follow-ups.md §4, test
  `won_ending_keeps_the_player_car_sounds_until_the_scene_ends`); their pitch
  and slip levels are partly open there.
* Racers' slots 3+ in an ending: the results order binding is not used (no
  shipped ending has events for them).
* The scene's starting carry: the native one depends on the frames before
  the scene; the clone takes its world's (1/60 s cadence) carry.
* Wine hitches (long frames) are not modelled.
