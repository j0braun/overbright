# The simulation's boundary: `sim::Session::step` (2026-11-13)

A tick of the world's simulation is one call:

    Session::step(&mut self, input: &TickInput) -> TickEvents

(`src/bin/game/sim/session/`). `Game::update_world` (src/bin/game/update/mod.rs)
is the glue around it: it runs the tick's input phase, builds the
`TickInput` and lends the `Session`, calls `step`, and hands the
`TickEvents` to the audio and the flow at the place their consumers acted
before. Nothing in `Game::step` moved: update → overlay_update →
preload_frame → fe_tick_before_sounds → audio_frame → particle_effects →
fe_tick, then `advance_tick`'s snapshot capture. The Phase 4 tick hooks sit
in that glue, beside the session, never inside it.

## The types

`Session<'a>`: the state the simulation owns, lent for one tick (a view:
the race lives in `Mode::Race`, which the front end's flow switches; moving
it into an owned session is a separate state split).

| field | what |
|---|---|
| `drive: &mut Option<Drive>` | the player's car, the world, the camera |
| `race: Option<&mut Race>` | the race running (`Mode::Race`'s) |
| `scene: &mut Option<ShopRun>` | the scene holding the cars (race start / ending, shop drive-in / out, career start, dyno) |
| `crowd: &mut Option<SceneCrowd>` | the race start scene's characters |
| `encounters: &mut Option<Encounters>` | free roam's encounters and outrun |
| `traffic: &mut Option<Traffic>` | free roam's traffic |
| `roam_style: &mut Option<(StyleManager, …)>` | the free-roam racer's style manager |
| `rng: &mut GameRng` | the shared generator (0x7EA624; docs/original/rng.md's first generator): every draw of the tick in its old order |

`TickInput<'a>` (≤ 104 bytes): `dt`, `world` (`WorldMode::{Menus, Explore,
Race}`, the mode as the simulation reads it), `controls` (the input phase's
driving controls), `lap_info` and `camera` (the play setup's options the rules
read), `settings` (the tick's `TickSettings`), `ai: Option<&AiData>` and `route_triggers: &RouteTriggers` (shared,
immutable data), and `roam: Option<&RoamInput>`: what free roam reads
from the career and the data (the profile, the career data, the trick
definitions, the units option, whether a world challenge runs).

`TickEvents` (≤ 112 bytes): `sounds: Vec<SimSound>` (no allocation while
empty), `race: Option<RaceTick>` (`Go`, `Stop`, `Finished { time }`,
`ReplayLeft`), `scene: Option<SceneTick>` (a shop scene's
frame `Ran`, or `Ended(run)`: its end is the flow's), `encounters:
Option<EncounterTick>` (free roam's encounter frame: the outrun timer, an
outrun started, an outrun ended and won or not) and `contacts:
nfsu2::drive::ContactSounds` (the impacts and scrape pushes the tick's
collision steps posted, in order).

What `step` runs, in the order `update_world` ran it: the ending scene
dropped outside the menus and the world animations (`world_anims`), the
camera director's draw (`director_draw`), then in a race the race's
tick (start scene, traffic, countdown / replay frame / live race, the
status clock, the race-over hold); in free roam with a shop scene the
scene's frame (`scene_frame`); in free roam without one the first
phase, the encounter manager's frame (`encounters_frame`); in the
menus the race's ending scene behind the post-race screens
(`ending_scene`).

**Free roam is a two-phase tick.** `step` runs the encounter
frame and returns its `EncounterTick`; the glue (`update::explore`) then
runs the outrun's career consequences in their old order (the profile's
outrun timer, the SMS_OUTRUN_INTRO inbox messages, Rachel's start line
with its hint draw on the shared generator, the outrun result with its draw on it, Rachel's
end line with its draw, the save, the speech, the outrun variety) and
calls `Session::step_roam(input, result_bank)`: a won outrun's bank onto
the race-over message, then the player's car, the traffic and the
encounter racers (`step_explore`) and the free-roam style manager. The
rest of the free-roam update (the career's clock, the GPS and the zone,
the D-Day script, Rachel's hints, the neighbourhood line, the SMS, the
world challenge, the tips, the cash pickups and the markers) is the
career's per-tick rules and stays in the glue after `step_roam`.

Why two phases: the hint draws and the outrun result's draw on the shared
generator depend on the career's state (`Profile::hints`, the stage's
bank table), not on anything the session asked for, and they come between
the encounter frame and the car's step (`step_explore` draws on the shared generator, the
world clock). The session cannot answer them from its own requests, so
the glue answers between the phases. The speech dependency (`say` changes
`speech_free`, which the neighbourhood line reads later in the tick) does
not cross the boundary: every speech request and every read of the
speech's state is in the glue after `step_roam`, in the old order (the
speech's state is the audio's, which no session copy could mirror: a
line's end depends on the stream's playback).

Draw order of a free-roam tick on the shared generator (before → after the two phases): the
encounter frame (no draw; the encounter racers' own generator) → session
phase 1; Rachel's outrun start line (`Hints::outrun_start`), the result
(`Profile::outrun_result`), Rachel's end line (`Hints::outrun_end`) →
glue between the phases, same order; the outrun variety (the racers' own
generator) → glue between; `step_explore` (`Drive::step` / the world
clock's `tick_clock`) → session phase 2; Rachel's hints'
`refresh_open` and the neighbourhood line (`neighbourhood_line`) → glue
after, same order. The director's draw needs a scene (none in this path).

**Shop scenes.** `Session::scene_frame(dt, camera)` is the
scene's step (the old body of `Game::shop_scene_frame`): the clock, the
door messages, the car on its NIS mover, the companion car, the camera,
and at the end the car off its NIS mover and a fresh chase view. Its end
comes back as `SceneTick::Ended(run)`; `Game::shop_scene_end` acts on it
where `shop_scene_frame` did (Enter opens the shop, DynoEnter seats the
car and opens the dyno menu, DynoExit starts the test run, a type-4 scene
aims the GPS; the run then goes to the audio's tail on a stepping frame).
`Game::shop_scene_frame` stays as the glue wrapper (lend the session,
step, act on the end). Its calls outside the tick are flow calls at a
scene's start, a dt-0 placement frame: `start_shop_scene`,
`start_career_scene`, `play_race_ending` (flow/scenes.rs),
`dyno_enter_scene`, `exit_dyno_scene` (dyno/session_impl.rs), and the
Enter skip's seek in the input phase (input/dispatch.rs, its end can open
the shop there); and the game's tests (src/bin/game/tests/scenes.rs). They are not
ticks and stay calls of the same session step through the wrapper.

**Contact sounds.** The library's process-wide queues
(`audio::collision`'s impacts, cap 64; `audio::scrape`'s pushes, cap
256) are now queue types (`collision::Pending`, `scrape::Pushes`) held per
session: the collision step posts into the `nfsu2::drive::ContactSounds`
it is handed (`step_world`, `step_world_with_post_motion`,
`CarState::step_with_props`, `step_cars_posting`; `step_cars`,
`CarState::step` and `step_ai` drop theirs). The session's steps post into
`TickEvents::contacts`; the glue appends them to `SimSession::contact_sounds`
right after the step (and after `step_roam`); the showcase's drive posts
there directly, the dyno pulls keep theirs on the pull (`DynoPull::sounds`)
and the dyno's tick appends them after each step; `impact_sounds` drains
`SimSession::contact_sounds`. Dropping the oldest at the cap gives the same
queue whether posts arrive one by one or in batches
(`drive::collide::sounds` test), so the queues hold what they held. The session module names none of the
game, the renderer, the audio, the window, the HUD or the front end
(`scripts/checks/layering.sh` rule 4).

## Paths from the simulation to the rest of the game

Source in the simulation → the path before this boundary → consumer.
"Event" is the `TickEvents` field it travels in now; "state" means the
consumer reads simulation state after the tick (a pull, not an event:
unchanged); "left" means not converted (reason below the table).

| # | source | path before the boundary | consumer | now |
|---|---|---|---|---|
| 1 | live race: the crash camera starts (`events.crash`) | `audio.soundfx.crash_camera = true` written from the update | `stream_sounds` → `SoundFx::Collision` | event `sounds: SimSound::CrashCamera`, set on the audio by the glue right after `step` (nothing between read it) |
| 2 | replay: a skip asks to leave | `self.end_replay()` called at the end of the replay's frame | flow (`Game::end_replay`) | event `race: ReplayLeft`, acted on by the glue right after `step` |
| 3 | race over: all finished, hold run | `self.finish_race(RaceEnd { time })` at the end of the race's update | flow (`Game::finish_race`, its `Flow`) | event `race: Finished { time }`, acted on by the glue right after `step` |
| 4 | race update's end (`Flow::Go` / `Stop`) | return value of `update_race` | the camera's velocity at the update's end (`tick_camera_motion`) | event `race: Go / Stop` |
| 5 | race start scene ends at the green, ending scene dropped outside the menus | `self.scenes.shop = None`, `scenes.crowd = None` | the flow's scene state | inside the session (the scene is lent: `scene`, `crowd`) |
| 6 | the camera director's draw | `nfsu2::rng_sites::director(&mut self.rng)` | the shared generator | inside the session (`rng` lent) |
| 7 | style reports (race's `scoring.style.reports`, free roam's `roam_style`) | taken by `style_reports_update` in the HUD's update | HUD (H2) | state (kept: the HUD's update reads both after the tick; the race's reports are part of the race the replay snapshots copy, so an event would not remove a reach) |
| 8 | impacts, scrapes (library physics: `drive/collide`, `smackable`) | process-wide queues `audio::collision::PENDING`, `audio::scrape::PUSHES` | `impact_sounds` (drain) | event `contacts` → `SimSession::contact_sounds` → `impact_sounds` |
| 9 | the exhaust pop (`Effects::step`) | `audio::carfx::BACKFIRE` atomic | CAR_Sputter | left: `Effects::step` runs in `particle_effects`, after the tick's sounds, outside `update` |
| 10 | particles, sparks, smoke (`fx::drain`, `particles::drain`) | queues drained by the effects glue | `particle_effects` | left: outside `update` (the boundary does not move `particle_effects`) |
| 11 | screen flash, crash-camera flag, hydraulics kicks, engine state | drive / race fields | render, audio (`engine_sound`, `car_sounds`) | state |
| 12 | camera cuts | `view_history_key` (scene shot, parked chase, mover type) | `capture_tick` / `frame_poses` | state |
| 13 | the snapshot, the state hash | `capture_tick`, `dev_hooks::record_state` read the state | harness, draw | state |
| 14 | free roam: outrun start / end | profile inbox, hint draws on the shared generator, `rachel_outrun` (speech), `profile.commit` | career, speech | event `encounters` (the encounter frame is the session's phase 1; the career's side runs in the glue between the phases; the bank goes back in as `step_roam`'s input) |
| 15 | free roam: D-Day script, hints, the neighbourhood line | `say`, `sms_popup_sound`, `sms_open` → `mode`, `refresh_event_triggers` | speech, FE sound, flow | glue (the career's rules after `step_roam`; no read-back crosses the session) |
| 16 | free roam: SMS, the world challenge | `sms_popup_sound`, `sms_open`, `tick_challenge` → `mode` | FE sound, flow | glue (as 15) |
| 17 | free roam: tips, cash pickups, markers | `profile.commit`, `hud.bank_reward`, `fe_sample(0x11)`, `enter_marker`, `marker_effects` | career, HUD, FE sound | glue (as 15) |
| 18 | shop / dyno / career scene frame (`shop_scene_frame`) | `open_shop`, `dyno_menu`, `start_test_run`, `gps_to_intro` at the scene's end | flow | event `scene: SceneTick::Ended` (the step is `Session::scene_frame`, the end `Game::shop_scene_end`; the dt-0 calls at a scene's start go through the same wrapper) |
| 19 | input phase | `fe_mouse`, `photo_tick`, hydraulics' FE sounds, `rumble_tick`, the look-back on `d.chase` | FE, pads | glue (input, not simulation) |
| 20 | showcase / FE lines | `say`, `take_snapshot`, `part_stitch` (audio), `render.cars` part poses | speech, save, audio, renderer | glue (front end) |

Left outside the boundary: the exhaust pop (9: `Effects::step` runs in
`particle_effects`, outside `update`), the particle and spark queue (10:
`particles::post_contact` is posted by the same collision steps, but its
consumer is `particle_effects` after the tick's sounds, the harness posts
into it directly, and its cap clears the whole queue on overflow, which
batching per tick would not reproduce without replaying each post; moving
it needs `particle_effects` on the session's side, a step of its own).
The chase camera's Field Of View offset (`chase/camera.rs`,
`ChaseCamera::view` reads `qol::fov_offset()`) is a settings global read
inside the tick (the race's and free roam's chase views) and also at
draw time (every chase view): taking it through `TickInput::settings`
needs the library's chase API to take the offset, which touches the draw
side too; left for its own step.

The HUD's views: `Race::drift_hud`, `Encounters::hud` and
`Progress::status_label` built the HUD's view structs inside `sim/**`; they
are now `hud_glue::views::{drift_hud, outrun_hud, status_label}`, reading
the plain simulation state. Rule 4 of `scripts/checks/layering.sh` run over
all of `sim/**` (not only `sim/session/**`): 64 hits before the two-phase free roam, 59
after; the rest are 27 glob imports of the crate root (`use crate::*`) and
`nfsu2::render` / `nfsu2::audio` / `crate::render` / `crate::dev_hooks` /
`Mode` reaches in the drive's clock and camera, the effects
(`sim/effects`, the renderer's particle and spark types), the event
camera's `native_mover_type(&Mode)`, `sim/rng.rs`'s `impl Game` and the
replay's camera type.

Before the two-phase free roam: why the free-roam paths (14–17) stayed in the glue: there the simulation and
the career are interleaved within the tick and the order is observable.
The outrun's start and end draw Rachel's hints on the shared generator between the encounter
manager's frame and the car's step (`step_explore` draws on it too), and
`say` changes `speech_free`, which the neighbourhood line reads later in
the same tick. Returning those as events would move the hint draws after
the car's or the line's start after that read. Converting them needs the
career's per-tick rules (hints, inbox, SMS) as session state or a
two-phase tick; both are logic-shaped steps of their own.

## Order statements

* **Random draws:** the session draws on the lent `GameRng` in exactly the
  statements and order the update drew before (the director, the drive's
  clock / hold, `race_frame`, the replay's cameras); nothing moved across
  a draw.
* **Side effects:** the events are produced in the order the side effects
  happened and consumed in that order right after `step` (the crash
  camera's flag, then `finish_race` / `end_replay`); nothing between the
  old call site and the end of `step` reads what the consumer writes.
* **Borrows:** the session's references are disjoint fields of `Game`
  (`sim.drive`, `mode`'s race, `scenes.shop`, `scenes.crowd`, `rng`, and
  shared `sim.ai`, `sim.route_triggers`); the settings are read before the
  step (they do not change in a tick). `TickInput::settings` is the
  tick's `sim::TickSettings`, taken once per tick after the tick's input
  (docs/developing/frame-settings.md).
