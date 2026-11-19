# Uncapped FPS: what SPEED2.EXE does above 60 Hz (2026-11-04)

## Result

* **The original never steps the world by the raw frame time.** 0x57EAE0
  (FrameLimit_Wait) turns the measured frame time into whole 1/1800 s
  ticks with a carry kept in 1/4000 s units, and the world (0x5EA370 →
  player mover 0x5B72E0) advances by `ticks / 1800`. A steady rate
  therefore gives a repeating tick pattern, not a constant dt.
* Per second of world time, driving forward is the same at every rate in
  both games (launch spread ≤ 1 % at 1–3.8 s).
* A start **from rest** is rate dependent in the original, and in a
  discrete way: at rest in 1st the gearbox cycles between clutch-open (0)
  and engaging (2) with a period set by the tick pattern. A brake press
  that lands on "2" first lurches forward and reverses ~0.3 s late. This is
  the whole "reverse timestep sensitivity" of internal notes.
* The clone already had the same clutch machine; it differed only because
  it stepped by the raw interval (at 120 fps it never left state 0, the
  original is 50 % in state 2). **Fix:** `nfsu2::frame_clock::FrameClock`
  (0x57EAE0's arithmetic) now gives `Game::redraw` its dt. With it the
  clone shows the original's cycle and both reverse outcomes at 60, 120,
  144 and 240 fps. Rendering stays uncapped; no limiter was added.

## Native rules traced

| Address | What | Status |
|---|---|---|
| 0x5812E5..0x5812FD | main loop: raw = 0x43BE20(prev, now) × 0.001 → 0x57EAE0 | verified (disassembly) |
| 0x57EAE0 | `V = min(max(raw + carry, T), 3 × 1/60)` (0x7FB700 ≠ 0; T = 0x865558), `ticks = trunc(V·1800 + 0.5)`, `q = trunc(ticks·2.2222223 + 0.5)`, `carry 0x865144 = V − q·0.00025`, `0x86513C = ticks`, `0x865194 = ticks·(1/1800)` | verified (disassembly; REA decompile ev_e60ffd40…) |
| 0x5813D4..0x581417 | driving path: dt = 0x5EA370() → 0x601A70 → 0x865150, world update 0x609CF0(dt) | verified (disassembly) |
| 0x5EA370 / 0x576CF0 | world dt = ticks (0x86513C) × 1/1800 × time scale +0x108 (1 outside slow motion) | verified (disassembly) |
| 0x5B7354..0x5B73D8 | mover: ⌊dt / 0.0044⌋ + 1 sub-steps of dt/n (0x7FC070), sub-step → 0x7FC800 | verified (earlier, re-read) |
| 0x59D3DE | sub-ms wheel-slip guard (dt > 0.001) | earlier unit; not reached at ≤ 500 fps with the tick clock (500 fps: 4, 3 ticks, sub-step ≥ 1.67 ms) |
| live | 0x8650CC = 0, 0x7FB700 = 2, T = 1/120 in free roam | observed |

Consequences (computed from the rule, confirmed live as mover dt / 0x86513C):
60 fps 30, 29, 31 ticks; 120 fps 15, 16, 14; 144 fps 12/13 (37 per 3);
240 fps 8, 7, 7 — so at 240 the world gets 22 ticks per 12.5 ms, **2.2 %
less world time than real time**, while the 4000 Hz clock 0x865188
(advanced by q) stays on real time. With the stock T = 1/120 the original
cannot go above 120 fps (V ≥ T); the 144/240 rows are the original with
its period changed, as the clone has no limiter.

## Method (original)

Its own private Wine copy, fastgpu on Xvfb :182, fresh
profile → free roam (Rachel's 350Z, DDAY_PLAYER_CAR), teleport to
(356.6, −478.4) heading −119.4° (the game re-places the car there from
(361, −462)), wait until at rest. The machine (load 40) could not hold a
real 240 fps, so the whole game was run at a **fixed frame rate in game
time**: the call at 0x5812EC (0x43BE20, the measured ms) was pointed at a
stub returning 1000/rate, T = 0; every system then sees exactly that frame
time (not only the mover, unlike the rejected dt-override experiment in
internal notes). Keys (Up / Down via xdotool) were timed on the
game clock 0x865188. A code cave on the mover vtable slot 0x7A25EC (data
0x8BBE00, buffer VirtualAlloc'd by the cave) copied per frame the mover dt,
car +0x60.. (position, velocity, orientation), +0x430.. (speed, gear,
rpm), drivetrain +0x14 (clutch), +0x38.. (target, gear, shift, pending,
input ω), engine +0x14/+0x78.., input pedals and the timing globals. All
patches were restored after each run (vtable, call, T = 1/120). Runs repeat
to 3 decimals (f120r/rb/rc). Tools and the per-run table:
the maintainer's private captures (cap.py, rec.s, dec.py, ana.py,
native-runs.csv, rate_probe.rs); raw captures kept with the maintainer's private evidence.

## Tables (t = world seconds after the first frame with the pedal down)

**Reverse from rest, brake held** — forward speed m/s at 1 s:

| fps | original: at-rest shift cycle | original: press on 0 | original: press on 2 | clone (FrameClock): cycle | clone: press on 0 | clone: press on 2 | clone before (raw dt): cycle |
|---|---|---|---|---|---|---|---|
| 60 | 0 | −3.159 / −3.130 | – (never 2) | 0 | −3.13..−3.14 | – | 0 |
| 120 | 2,2,2,0,0,0 | −3.181 (×4), −3.176 | −2.267 | 2,2,2,0,0,0 | −3.14..−3.16 | −2.25..−2.33 | **0 only** |
| 144 | 2 | – (never 0) | −2.315 / −2.323 | 2 | – | −2.21..−2.32 | 2 |
| 240 | 2,2,0 | −3.184 | −2.174 / −2.209 / −2.205 | 2,2,0 | −3.18 | −2.15..−2.26 | 0,2 (period 2) |

At 2 s: original −6.47 (press on 0) / −5.45..−5.60 (on 2); clone −6.43..−6.49 / −5.27..−5.52.
360 / 500 fps (clone only, no native counterpart): cycles 0,2 / 2,0, same two outcomes.

**Launch, full gas** — forward speed m/s:

| fps | original 1 s | 2 s | 3 s | 3.8 s | clone 1 s | 2 s | 3 s |
|---|---|---|---|---|---|---|---|
| 60 | 6.220 | 15.410 | 23.94 | 29.03 | 5.98 | 16.59 | 24.96 |
| 120 | 6.16–6.24 | 15.26–15.43 | 23.73–23.89 | 28.83–28.98 | 6.01 | 16.61 | 24.43 |
| 144 | 6.159 | 15.266 | 23.81 | 28.90 | 6.03 | 16.63 | 24.53 |
| 240 | 6.18–6.20 | 15.28–15.33 | 23.77–23.91 | 28.87–29.00 | 6.05 | 16.62 | 24.63 |

The rate spread is ≈ 1 % in both games. The original-vs-clone launch
difference (6.2 vs 6.0 at 1 s, 15.4 vs 16.6 at 2 s) is the same at every
rate and is not a rate effect (driving parity). In the original
the later brake at "4 s" lands ~0.09 s earlier in world time at 240 fps
(keys on the real-time clock, world 2.2 % slow); this is the clock rule
above, not physics.

## Design decision: the world runs as the original at 60 fps

> "the original does not run at those high framerates, so it should work
> like at 30 or 60 fps" (design requirement). Rendering stays uncapped;
> no frame cap.

The measurements above show why: above 60 fps the original's own result
depends on the rate (tick pattern, at-rest clutch cycle), and above 120 fps
it never runs at all. So the clone's world no longer follows the display:

* **Tick clock** (`nfsu2::frame_clock::TickClock`, `Game::advance` while a
  world is loaded; 2026-11-12): real
  time is counted in whole units of 1/36 000 s (exact for 30/50/60/72/75/
  90/100/120/144/200/240 Hz; the window converts its measured nanoseconds
  with a carried remainder, the harness's f32 seconds round with a carried
  remainder). The world advances by ticks; each tick's rate is picked from
  the state before it, inside the catch-up loop: `RACE` (1/60 s, stepped as
  the original's 60 fps frame through `FrameClock` with `RAW_60`: 30, 29,
  31 ticks of 1/1800 s and the 1/4000 s carry) or, while a scene holds the
  cars (race start and ending scenes, the career's start, the dyno;
  docs/postrace-jitter.md), `SCENE` (1/30 s, the limiter's 60, 60, 60, 60,
  61). The carry and so the 3-cycle's phase run on from boot through every
  rate switch; the accumulator stays in real time across a switch. A tick
  is taken while the accumulator holds at least the period less 2 ms
  (`SNAP_UNITS`; only for frames that long themselves, so a 60 Hz
  display's jitter does not alternate 0 and 2 ticks, and a 75 Hz one does
  not take every tick early). At most 3 RACE / 2 SCENE ticks a drawn frame;
  the time beyond is dropped, so below 20 fps the world slows down as the
  original's 3/60 s cap makes it. A 30 Hz display takes two RACE ticks a
  frame (no slow-frame branch any more: before, a frame of 25 ms or more
  was one variable update of its own length, which made 30 fps a
  different world). The sequence of (rate, dt) is therefore a function of
  the tick states only: the world state at tick n is the same at any
  display rate (`frame_clock` tests; the rate gate, docs/checks.md). The
  at-rest clutch cycle at every rate is the 60 fps one (the original
  rests in clutch state 0 at 60 fps, so a reverse press starts at once:
  −3.13..−3.16 m/s after 1 s, as measured in the original).
* **Front end without a world**: ticks at RACE on the same
  tick clock, the same accumulator and carry as the world path (the
  original runs 0x57EAE0 every iteration, the menus included, and its carry
  and tick phase run on through menu → load → race → menu:
  the clock audit). Before, each drawn frame was one update of its own
  length with the accumulator zeroed.
* **Interpolation** (`nfsu2::interpolate`, `Game::draw_between`): a drawn
  frame between updates shows the state `alpha` = accumulator / the next
  tick's period of the way from before the last update to after it (0
  when the accumulator is within the 2 ms snap on a frame the snap applies
  to: a steady 60 Hz display draws exactly the tick states). Blended: every drawn
  car's position (lerp) and orientation (slerp), wheel compression, roll
  and spinner angles (the turn the wheel's spin made), steering angle and
  hydraulic mounts — the player, race rivals and race traffic, free-roam
  traffic and encounter cars, a shop scene's companion — and the chase /
  world camera (position, yaw the short way, pitch, fields of view, up).
  A jump over 30 m (teleport, reset, camera cut) or a changed car list is
  not blended. Now the blend is computed from two tick snapshots
  into the frame's own copies ("Tick snapshots" below); nothing is swapped
  into the simulation any more. Also blended now: the native
  particles (by id), the rain's drops and lens drops, the world
  animations and knock-over props, photo mode's rig, the dyno's rollers
  ("Drawn objects between ticks" below). **Not blended**
  (they move at the 60 Hz updates): the enhanced smoke / sparks / spray
  (on ticks too), skid marks (laid on the tick, they do not move), the HUD's
  numbers (stepped, the default), the garage's own scene.
* **Input** is read once per world update (held keys and pads at the
  update; menu key events still act when they arrive). Added latency: the
  drawn state trails the newest update by up to one update, so at most
  1/60 s (16.7 ms) more than stepping on every frame, and a key press waits
  up to 1/60 s for the next update, as on the original at 60 fps. A tap
  shorter than one update between two updates is not seen by the held-key
  read (the same as the original's once-a-frame polling at 60 fps).
* **The harness on the tick cadence** (2026-11-12;
  `src/bin/game/dev/ticks.rs`): `--shot`, `--selftest`, `--flow`,
  `--playthrough` and the game's tests step the world only by ticks, as the
  window does. `Game::advance_ticks(n)` takes n ticks (each at the rate the
  state picks, its dt the tick clock's next: `TickClock::begin`/`tick`);
  `Game::advance_for(s)` the ticks s seconds of real time hold (1/60 s is
  one race tick, 1/30 s two race ticks or one scene tick, 0.5 s 30 race
  ticks; the part of a tick left over is carried in the tick clock's
  accumulator); `advance_for_each` / `step_for` add a closure or the
  window's whole step per tick. The flows that draw frames (`firstrace`,
  `firstdrag`, `postrace`, `menus`, `garage`, `scenes`, `outrun`) drive
  `ticks::Frames`: drawn frames on the display schedule, the world ticked
  through the window's tick path one tick at a time, so every input and
  check happens at a tick index, between ticks (inputs keyed on ticks by
  default, not only in rate runs); their keys go through the input queue
  at that tick (`Game::input_at`, "Input on ticks": `tap_next`,
  `hold_next`; the playthrough's taps too), as the window's do. The front
  end without a world is ticked the same way (it is on ticks too).
  The bench's `NFSU2_BENCH_HZ` frames run on the exact integer schedule
  (1/36 000 s units); its audio is on ticks too: in a rate run each tick's frames
  rendered through the capture after its sounds (`audio_tick_end`),
  outside one pumped per drawn frame, at `NFSU2_BENCH_HZ` exactly the
  frames of the ticks that frame ran (the audio tick clock's), so no tick
  is rendered twice or not at all. `Game::update` checks (dev-harness and
  test builds) that every dt is one of the tick clock's (29, 30, 31, 60,
  61 of 1/1800 s); the only exception is the replays of the original's own
  measured frames (the parity flow, INTRO_FRAMES; `ticks::off_cadence`).
  The default `--bench` (one 1/60 s step a frame, `bench_step`; 1/60 s is
  a RACE tick's dt bit for bit) is unchanged, so the 60 Hz bench images
  and audio stay the reference.

## Game time on ticks (2026-11-12)

Every clock game state reads advances in `Game::update` (once per tick,
`Pacing::clocks`, `nfsu2::game_clock::GameClock`), from that tick's
quantised dt, never from the wall clock or a drawn frame:

| value | step per tick | resets | pause | address |
|---|---|---|---|---|
| `tick` | +1 | never | runs | (the clone's tick index) |
| `package_ms` (package clock) | `_ftol(f32 dt × 1200)`: 20, 19, 20 at RACE, 40 per SCENE tick | never | runs (0x558C10 applies it in pieces of ≤ 20 ms out of a running world, `last_pieces`) | 0x5662D0 → 0x558C10 |
| `real` (1/4000 s) | round(dt × 4000): 67, 64, 69 | never | runs | 0x8651A8, 0x57A060 |
| `sim_clock` (1/4000 s) | round(dt × 4000) when the world ran | 4000 at world create and teardown | frozen | 0x865188, 0x57A0F0, 0x576D00 |
| `world_counter` | +1 when the world ran | 0 at world create, 100 000 at teardown and boot | frozen | 0x7FB724, 0x5FF54D / 0x5FF7CD / 0x5FF7D2 |

"The world ran": the front end without a world (0x581469 sets 0x865150 =
0x865194), a race, free roam or a scene; not a menu over a loaded world
with no scene stepping (the pause: 0x601A70(0); **inferred** for the
in-game SMS / map / status screens and shops over a loaded world, which the
clone also does not step). A reset inside a tick also skips that tick's
step (0x576D00 zeroes 0x865150). World create = `drive_in_loaded` (every
drive-in, a restart included: **inferred**), teardown = `drop_drive` and the
career resume's world release.

`Game::now()` (ms) = `package_ms` / 1.2 (`FE_RATE`), so a screen's script
time 1.2 × (now − start) is exactly the package clock (F3; before: 1.2 ×
wall ms, 1200 package ms a second where the original makes 1180 at 60 fps).
`Game::seconds()` = `now()` / 1000. The screens, menu timers (F1: ISP, EA
Login, LAN, splash, help cycle), the showcase / save timing (F2), the
EA Trax chyron (F10, was `Instant`) and the HUD packages (`overlay_update(now)` on the tick)
all read it. The developer harness may pin it (`Pacing::clock`,
NFSU2_SHOT_CLOCK); rate runs never pin it.

### Menu timers on the real-time clock (2026-11-12)

The original's C++ menu timers count the real-time clock 0x8651A8 (67, 64,
69 per RACE tick, 200 per 3 ticks = exactly real time), not the package
clock (20, 19, 20 ms: 59/60). On `now()` they fired about 1.7 % late (LAN
after 305 ticks instead of 300) at every display rate. Now each stamps and
tests `Game::real_now()` = `GameClock::real_tick`: 0x8651A8 as the tick in
progress reads it (`real` at the tick's start, set before the tick's input,
whose key handlers open the screens; the draw after the tick reads the same
value, as the original's steps 12–24 all run before its own 0x57A060, step
25). Still tick-derived (no wall clock); a clock the harness pins gives its
own time. Compares are the x87 ones against the f32 constants with 0.00025
as the f32 0.000250000012 (`game_clock::real_over` `>`, `real_at_least`
`>=`, exact in an f64): the scripts' own start times stay on the package
clock.

| timer | native (**traced**) | clone now | before |
|---|---|---|---|
| UI_PC_LAN_ServerSelect search dialog | ctor 0x4FB60E: +0x900 = _ftol((0x8651A8 × 0.00025 + 5.0) × 4000 + 0.5); tick 0x4FB8AE: gone when 0x8651A8 > +0x900 | `fe.lan_search_until`, `real > until`: 300 / 301 ticks after the open's | 6000 package ms: 305 |
| UG_LS_Splash back to the intro | +0x50 = 0x8651A8 (0x4A9225); tick 0x4C55F8: (0x8651A8 − +0x50) × 0.00025 > 30.0 (0x784478) | `Splash::opened_real`: 120 000 ticks (1800) | 30 000 ms: 1831 |
| UG_LS_Splash "Press Enter" | +0x4C = 0x8651A8 at the package's screen-up message (0x4C55CF, sent at script time 0: the open, **inferred** that it is handled in the open's iteration); tick: 0x8651A8 − +0x4C > 0x3E80 (integers) | `Splash::shown_real`: > 16 000 (241 ticks) | 4000 ms: 245 |
| help bar name / cash cycle | not 0x8651A8: its own countdown +0x48 −= _ftol(0x865194 × 4000) (0x577320: 66, 64, 68, truncated), switch at ≤ 0, +0x48 = _ftol(t × 4000 + 0.5) (3000 / 20 000; 0x537F60, 0x51DDA0) | `HelpCycle::step` on the tick's dt in `menu_tick`: name 46, cash 304 ticks | 750 / 5000 ms: 46 / 306 |
| FE cursor shown after a move | FE_DrawCursor 0x50B460: 0x83A9CC = 0x8651A8 on a move (0x50B492); drawn while (0x8651A8 − it) × 0.00025 ≤ 2.0 (0x7844C0) | `Mouse::moved_real`, `cursor_shown`: up to 7999 ticks | 2000 ms |
| EA Trax caption *_GO, then gone | Chyron_SetText +0x4C = 0x8651A8 (0x4AC8E6); tick 0x4DB9C0: > 7.2 s (0x78E09C) → Chyron_Go 0x4CB1D0 (*_GO, +0x55); a later tick > 7.2 + 1.0 (0x4CB207) ends it | `audio::music::Chyron::step` in `update`: 28 800 / 32 800 ticks | 7200 / 8200 ms in the draw |
| the lots' car asked 0.3 s after the last Left / Right (crib 0x4B04C0 +0xB8, car lot +0xE0, QR car select +0xC4) | ticks 0x4E0005 / 0x4EE026 / 0x4FCD2E: (0x8651A8 − it) × 0.00025 ≥ 0.3 (0x78758C) | `GarageFe::crib_real`: ≥ 1200 ticks (18) | 300 ms: 19 |
| the career World Map's map widget hand-off 0.3 s after a change | +0x22C = 0x8651A8 (0x4CC1D9 selection, 0x4E0EE4 stage, the constructor); tick 0x4E12DF: while set (0x401820), (0x8651A8 − it) × 0.00025 ≥ 0.3 (0x784268, 0x78758C: `fcomp; test ah, 1; jne`) → 0x498600 (widget +0xC = the selected track) and +0x22C = 0, else the widget's tick 0x4CDD40 | `WorldMap::widget` (`MapWidget::step` in `menu_tick`): 18 ticks | 0.3 s of `now()`, decided in the draw: 19 |
| UI_QRTrackSelect's map widget hand-off 0.3 s after a move | +0x14C = 0x8651A8 (0x4E32E8, in the step 0x4E3270); tick 0x4EFC30: the same compare, 0x498600 at 0x4EFC72 | `QuickRace::widget`: 18 ticks | 19 (the first track handed at the opening: **inferred**, unchanged) |
| the map widget's zoom | 0x4CDD40: a handed track not shown loads and shows (0x4B4B80: +0x28 = _ftol((0.5 + 0.5) × 4000 + 0.5) = 4000); each of its ticks +0x28 −= _ftol(0x865194 × 4000) (0x577320: 66, 64, 68; 0 stays 0); r = +0x28 × 0.00025 ≤ 0.5 (0x78DBCC) → uv at t = 1 − r / 0.5 (0x4CDE88) | `MapWidget::count`, `map_zoom_count`; not counted while a hand-off waits | 1 − (1 − package seconds since the hand-off) / 0.5, counting on through a waiting hand-off |

Not ported (no clone counterpart, or not a menu): the demo screens
LS_Demo_Legal / PSA / ESRB / Warning (handlers 0x4C5460 4 + 6 s, 0x4C54B0
4 s; descriptors 0x7F8B74…0x7F8BC8, not in the boot list); the online
screens (EA Login's 0x7F942C 0.75 s at 0x4D51EA, the messenger, lobby, game
room, UI_OLCarLot); MC_Main's and the post-race screens' 0x89D05C
(network time); UI_GenericParts_Browser's 0.5 s preview check (0x567645: while +0x1D6 is set
and (0x8651A8 − +0x1CC) × 0.00025 > 0.5, 0x784260, clear it and preview,
0x50E580(slot, part) or 0x4C2100(0x1D) for the rims) is never armed in the
shipped game (**traced**: +0x1D6 has four references in the whole
disassembly, all reads or zero stores — the constructor 0x55E930 and the
tick 0x567683 — and +0x1CC is only zeroed, 0x55E904 / 0x55EA06): a move
in the list previews at once (0x55EBB0 → 0x55C180), as the clone does;
docs/native-fixes/parts-browser.tsv); UI_VirtualKeyboard's 0.75 s (0x83A9C8
stamped by its constructor 0x4E3300, compared ≥ 0.75 s, 0x7F942C, by
UI_OptionList_Notify 0x556030 at 0x556206 on message 0x81017864; listed
here before as UI_QRTrackSelect's: the screen has no clone counterpart).
The track-map widgets' hand-offs and zoom are in the table above
(tick-stepped `MapWidget`, docs/native-fixes/FE-SNAP.tsv). Race-side users of
0x8651A8 (finish timers, HUD, the camera director, nitrous) belong to the
race, not this table.

Wall time (`Instant`, `now_ms(started)`) is read only for presentation:
the loading gate and PC_Loading's dots while a load blocks the tick
(loads/), the boot PC_Loading, the frame pacer / VRR probe / battery and
power cap, the perf overlay, the photo mode's "Saved" status line (it
follows the save threads), the save-file time stamps (`SystemTime`), the
boot RNG seed (0x57ECC0 seeds from the clock natively), log-only timers
(`NFSU2_AUDIO_DEBUG`, the garage preview log).

The drawn-frame count `frame_count` (0x870818) is presentation only (the
env-cube faces' rotation, U8); `INTRO_STEP_HZ` is gone (one world
step per tick).

## Input on ticks (2026-11-12)

The original polls its input once per main-loop iteration, after the frame
clock (0x43BE20 → 0x57EAE0) and before the world update (0x5809D0, step 13
of 0x5811D0; the clock audit's "the order inside one main-loop
iteration"). The clone's window events are queued (`input::queue`,
`Platform::input`) in arrival order, each with its **due tick**, and
applied at the start of that tick — first thing in `Game::update`, before
anything of the tick (and before the tick decides whether the world runs),
which is what a key between two ticks did before. So what an input does
depends only on the tick it falls on, never on where in a drawn frame it
arrived.

* **Queued:** key presses (with the typed text: MC_Main's name editor,
  the splash screen's cheat input; the system's auto-repeat presses only
  type text, as before), key releases (modifiers are held keys, so they
  are queued too), the clicking button's press / release, the wheel and
  the cursor's moves. Everything a key does is queued with it, the F3
  overlay layouts and Alt+Enter included (they read Shift / Alt from the
  queued held keys, so they stay in order with them).
* **Immediate (presentation only):** resize, move, scale factor and the
  monitor; focus and occlusion (read only by Power Saving's cap and a
  swap-chain log line: no game state); the close request (ends the event
  loop; nothing of the game runs for it); the redraw. The quit question's
  Quit Game (`quit_app`) is set on a tick and ends the event loop right
  after (`about_to_wait`; before, only a key's own handler checked it, so
  a click or a controller on Quit Game waited for the next key).
* **Due tick** (`queue::due_tick`): an event belongs to the tick whose real
  time window holds its arrival (tick k covers [k·T, (k+1)·T) of the tick
  clock's accumulated time): due = next tick + ⌊(acc + since) / T⌋, at most
  the frame cap (3 RACE / 2 SCENE), with `acc` the tick clock's accumulator
  after the last drawn frame and `since` the time from that frame's start to
  the arrival (winit's dispatch time: winit gives no event time stamps).
  At 60 Hz that is the next tick; at 30 Hz a frame runs two ticks and an
  event from the frame's second half goes before the second of them, as at
  60 Hz; at 120 / 200 Hz a frame often has no tick and the event waits for
  the next. Exceptions (unit tests `a_jittered_display_…`): an event in the
  last 2 ms of a window whose tick a frame took early (the clock's snap)
  goes with the next tick; after a stall past the cap (dropped time) it goes
  with the next frame's first tick.
* **Latency:** at most one tick (16.7 ms) from arrival to effect at ≥ 60 Hz
  (none against the original at 60 fps, which polls at the frame start:
  the event applies at the first tick the next frame runs); at 30 Hz it is
  the same tick as at 60 Hz (up to one 30 Hz frame after the arrival). No
  other buffering.
* **Mouse:** sampled once a tick (`fe_mouse`); moves coalesce to the newest
  point (hover and hit tests read it); a press keeps the point it arrived at
  (`MouseState::press_at`), so a click acts there even when the cursor
  moved on before the tick; the newer point is the next tick's move.
* **Repeat (I2):** the scroll arrows' gate runs on the real-time clock
  0x8651A8 (`GameClock::real`), **traced**: 0x538216..0x53823A acts on an
  arrow press when (0x8651A8 − +0x38) × 0.00025 > 0.0625, +0x38 stamped
  with 0x8651A8 at the click (0x52E493 / 0x52E4BC): more than 250 units,
  the 4th RACE tick after a click (67 / 64 / 69 per tick; exactly 250 never
  occurs, so the x87 precision does not matter). Before: ≥ 62.5 ms of the
  package-clock `now()` (the same ticks at RACE, but not the native
  clock). The held button acts once a press (`held_fired`, no timer). The
  controller's FE stick repeat (`FeRepeat`, Modern layout only; native
  passes tick 0) counts the game clock's 60 Hz ticks, already tick-derived
  . The keyboard has no held-key repeat in the clone (the system's
  repeats are dropped, as before); whether the original repeats held FE
  keys (0x54D4C0's direction auto-repeat "after 0x140 then 200", formats.md)
  and on which clock is **open**.
* **Pads:** polled once per tick already (`pad_frame`, the tick's input
  phase, right after the queued keys), hot-plug unchanged.
* **The injection point** (window and harness):
  `Game::input_at(&mut self, tick: u64, event: InputEvent)` — `event`
  applied at the start of tick `tick` (`GameClock::tick`, the index of the
  update it goes before; a tick already run means the next one), in the
  order queued. The window uses `Game::input_now(event)` (= `input_at` with
  the due tick). `InputEvent::{KeyDown { code, text, repeat }, KeyUp,
  CursorMoved, Button, Wheel}`; `InputEvent::press(code)` is a key press
  without text (a harness press also holds the key until its `KeyUp`).
## Audio on ticks (2026-11-12)

The game posts its sounds once per tick, after the update (`Game::step` →
`audio_frame` → `audio_sounds`), and every audio timer that feeds game
state reads the **audio tick clock** (`Audio::tick_frames`,
`Game::audio_tick_begin` at the start of `update`): the mixer frames of the
ticks so far, each tick its real-time length at the mix rate — a RACE tick
1/60 s, a SCENE tick 1/30 s (48 kHz: 800 / 1600; 44.1 kHz: 735 / 1470;
other rates whole frames with the remainder carried in 1/36 000 s units, so
the sum is exact). Not the tick's game dt (30/29/31 or 60/61 × 1/1800 s:
that would give 800/773/827 and run a scene's audio 1/300 ahead of the
device, which plays real time).

| timer | clock now | before | feeds |
|---|---|---|---|
| speech / NIS line "over" (`within_length`: decoded length + 0.5 s, 30 s while decoding), its pause shift (`hold_speech`) | tick frames (`LineClock::Mixer`) | `Instant` | `speech_free` / `speech_playing` / `nis_busy`: Rachel's lines, the D-Day script, hints, SMS, the backup line's cleanups |
| EA Trax / score hold-off after a failed piece (1 s / 5 s; no native counterpart) | tick frames | `Instant` | the next song pick |
| a line "queued, not yet playing" (decoder not finished) | offline mix: none — lines decode inline (`Streams::set_inline_decode`) | decoder thread | `speech_free` |
| mixer gain ramps, stop fades, track playback | the mixer's own frames | same | output only |
| device watchdog, stream open timeout (`device/cpal.rs`) | wall clock | same | output only (the device) |
| `NFSU2_AUDIO_DEBUG` lines | wall clock | same | log only |

A track's end (`track_playing`) still follows the mixer's playback: offline
the capture renders each tick's frames after its sounds (`dev_hooks::audio_tick_end`,
and the pump waits for the decoders), so it is a function of the ticks; on a
device it follows the device as the original's stream state does. Real-time
onset bound (plan §1.5): a sound posted at tick n plays at most one drawn
frame after the tick's time.

Run-to-run differences found (rate gate `aud nd`): two hash-map iterations
that ordered voice starts (the AI engines' banks `OtherCars::banks`, now
by path; the scrape objects, now in creation order), the threaded line
decode read at a nondeterministic tick, and a line's samples visible before
its "done" (now set under the same lock).

## HUD on ticks (2026-11-12)

The original updates its HUD once per main-loop iteration: 0x600910 →
0x600600 → the features 0x6000A0 and the widget updates 0x4EC5A0, step 16
of 0x5811D0 — after the world update (13), the race flow (14) and the
package clock (15), before the audio manager (18) and the render (24)
(**traced**, the clock audit's "the order inside one main-loop
iteration"). The clone's HUD did all of that in the draw (`overlay_quads`
from `prepare_frame`), once per drawn frame. Now:

* **Update on the tick** (`Game::overlay_update`, called in `Game::step`
  right after `update`, i.e. after the world and the package clock, before
  the tick's sounds): the style reports taken (H2), the cash popup's count
  , `Hud::step` — the features, every element's objects, scripts,
  texts, the prompt queue and its sounds, the trick popups, the board phase,
  the lap hold `lap_at`, the "Hurry!" window, the outrun meter's frame
  count, the drag shift message — and the rear-view mirror's
  request. The HUD reads `Game::now()`, the package clock, which
  advances only on ticks. The "Hurry!" test's step is the previous
  tick's hurry clock (`hurry_prev`), exact at every rate.
* **Draw = layout** (`Game::overlay_layout(&self)`, `Game::hud_layout`,
  `Hud::layout(&self)`, `letterbox_quads`, `replay_quads`): the quads from
  the state the last tick left; no writes, sounds, RNG or clock reads.
  Between ticks the HUD shows the last tick's state, stepped (owner
  default; no interpolation). Drawing it twice or not at all changes
  nothing (`hud_glue::tests::drawing_the_hud_changes_nothing`,
  `frontend::hud::tests::layout_reads_the_last_step`).
* **Sounds**: the engage prompt's sounds the update queues are posted
  at the end of the tick's sound list (`audio_sounds`), so the capture
  renders them in that tick's frames and `aud` has them at that tick. At
  60 Hz the posting order is the one the draw had (right after the tick's
  other sounds); the capture plays them one tick (16.7 ms) earlier than
  before, as the original's order (16 before 18) has them.
* **Pause tick** (**traced**): under the pause menu (and its options)
  the HUD update runs with the pause features (low 0x40000, high 0,
  `Hud::pause_step`): the cash popup does not count, the outrun meter's
  group turns INACTIVE and its shown-frame count +0x44 resets unless in an
  outrun (race type 7). Other elements' pause behaviour follows their mask
  bits in the original and is not traced one by one: the clone leaves
  them as the last update did (**open**).
* **Harness**: `Game::hud(now)` / `Game::overlay_quads(now)` (dev-harness
  and tests only) still do update + sounds + layout in one call for shots
  that step the HUD at times of their own; flows driven by `update` alone
  do not step the HUD (as before: they drew nothing). The rate harness's
  draw-side updates (`rate::drawn`) lay the HUD out only.
* Tests: `hud_glue::tests::hud_state_and_quads_are_the_same_at_every_rate`
  (a career circuit race, gas held from tick 0, 1500 ticks through
  `Game::advance` at 30, 60, 120, 200 Hz and a 4–40 ms jitter with stalls:
  the state hash's `hud` column and the HUD quads identical at every tick
  reached), `frontend::hud::tests::pause_tick_resets_the_outrun_meter_outside_an_outrun`.

## Front end out of the draw (2026-11-12)

What the drawn frame stepped until now steps once per tick. In the
original GarageMain's update runs in the front end's tick (0x4D82D0 from
0x5663C0, step 15 of the loop 0x5811D0) and the camera movers at step 17
(0x44EFE0), both before the AudioManager (0x483B40, step 18). So
`Game::step` runs `fe_tick_before_sounds` — the garage's request and scene
(`garage_tick`, with the FE car's hydraulics sounds 0x4D8BB6 → 0x4A6030
played there, no longer by the next tick's input phase) and the dyno
camera — right before `audio_frame`, and `fe_tick` — photo mode's close
(the clone's own, no native step) and the hit map (a layout, no sound) —
last:

    self.preload_frame();
    self.fe_tick_before_sounds();   // garage (step 15), dyno camera (17)
    self.audio_frame(dt);           // AudioManager (18)
    self.particle_effects(dt);
    self.fe_tick();                 // photo close, hit map

The garage's part / turn / swing / hydraulics sounds go out with the tick
that made them: one tick (800 frames at 48 kHz) earlier than the first front-end-on-ticks step had
them (it kept the old draw-side timing to stay byte-identical at 60 Hz;
this is the attributed difference in the garage and menu flows' audio).
Test: `tests::fe_real_time_clock::garage_sounds_go_out_with_their_tick` (fails with the
old order).

| was in the draw | now on the tick | |
|---|---|---|
| photo mode's close (`photo_close_update`, F8), first in `draw` | first in `fe_tick` | |
| the mouse's hit map: what the last drawn frame laid out (`mouse::begin_frame` in `prepare_frame`) | `fe_map_tick`: the same layout (`replay_quads`' bar, photo mode's strip or `menu_quads`) run on the tick's state with recording on, then recording off | the next tick's `fe_mouse` tests a click (at the point it arrived) against the layout of the state it is applied to; the draw's layouts record nothing (`record_rect` / `set_slider_rows` now record only while recording) |
| hovers `help_hover`, `fe_hover`: `Cell`s | plain `FeState` fields, written by the tick's mouse | they were already set by `fe_mouse`; the `Cell`s are gone |
| the garage's request (`garage_update`, F4: lot swing, view tick, FE car swing, part / turn / swing sounds, `garage_fe.last`) | `garage_tick` | the draw reads `garage_fe.request` |
| the garage scene (`draw_garage`, F5–F7): platform load, FE car change and its builds on threads taken when done, builds ahead, hydraulics and their sounds, hood / trunk / doors (`part_anim` requests and the pose settle), the particles and their generator draws | `garage_scene_tick` (garage/draw.rs), from `garage_tick` | the draw (`render_garage`) writes only the renderers (GPU caches) |
| the dyno camera (`dyno_camera_update`, F9) | `fe_tick_before_sounds` | |

**Loads.** A garage car built on a thread (`GarageCarJob`, the shipping
game) is taken only on a tick (`garage_take_built` in the tick), so the
state machine observes completion at a tick boundary. The deterministic
headless modes (shots, the selftest, flows, rate runs) turn `GARAGE_ASYNC`
off (shell/cli.rs), so there a car asked on tick m is built in tick m at
every rate; the window keeps building on threads. (Whether the shipping
game takes the threaded path depends on the FE car having a renderer, a
GPU cache the draw makes: shipping only, not a harness path.)

**Harness.** Paths that call `Game::update` directly (the shots, the
old-style flows, the playthrough) never ran these draw-side steps unless
they drew, and still do not: they call `garage_request()` (request and
scene step) themselves, and a shot's still runs `draw_garage` =
`garage_scene_tick` + `render_garage` at its own request time. `rate::drawn`
no longer runs the map or the garage's request (both on the tick).

**Tests** (src/bin/game/tests/fe_ticks.rs): the garage scripted (a body
shop list, the Hydraulics list with a level-2 part pumping) at 30 / 60 /
120 / 200 Hz and jittered, drawn every frame: the same front-end state
(hovers, garage request and FE state, the scene's car, hydraulics, poses
and particles, FE sounds posted, the RNG) at every tick two schedules
share; drawing twice or not at all changes nothing; a click on UI_Main's
arrow (the cursor moved off before the tick) steps the strip at its tick
at every rate.

## Tick snapshots (2026-11-12)

The draw is a function of two captured tick states and α
(`src/bin/game/render/snapshot.rs`):

* **Capture** (`Game::capture_tick`, at the end of every tick in
  `advance_tick`; "use, then step": S_n holds what tick n's frame draws
  with). `TickSnapshot`: the drawn cars' ids and poses (`CarPose`: position,
  orientation, steering, wheel compression / roll / spin / spinner, the
  hydraulic mounts) in `drawn_car_states` order, the drive's camera (already
  shaken), the camera mover, the scene crowd's frame, the tick's dt and the
  `cut` flag. Ring `Snapshots { prev, cur }` (`Game::interp`): the slots
  swap and the newest is refilled in place, so the steady state allocates
  nothing. Size: 40 B per car pose + 8 B per id + ~70 B fixed; a race with
  3 rivals and 10 traffic cars is ~0.7 KB per slot. Measured: 0.25 µs per
  tick with 20 cars in free roam (`tests::snapshots::capture_cost_per_tick`),
  the slots' capacities unchanged over 2000 captures.
* **Frame values** (`Game::frame_poses(α)` → `FramePoses`, a pure read,
  built before photo mode and the frame's layout run, where the swap-in
  was): the cars as drawn are copies of the tick state with the blended
  pose applied (`nfsu2::interpolate::pose_at`: exactly S_{n−1} at α = 0,
  exactly S_n at α ≥ 1, `CarPose::blend` between); the camera
  (`camera_at`, same ends); the crowd's frame; the carried particles
  (nitrous flame, purge) moved by drawn × newest⁻¹; the water's lag
  (1 − α)·dt. The draw reads them through `FrameView::poses`,
  `SceneGame::poses`, `DrawnCars` and the frame's own camera
  (`FrameDraw::camera`, `SceneCtx::camera`; the TAA jitter is set on it).
  Deleted: the swap-in / swap-out in `draw_between`, `Pacing::draw_alpha`,
  `Water::lag` (now `water::View::lag`), `particle_owner_transforms`,
  `drawn_cars()` (`&mut` list), `Interp`'s four `prev_*` fields.
* **Cut rule** (`frame_poses`, one place): α ≥ 1, no drive, or no
  S_{n−1} → the tick's values; a car: count differs from S_{n−1} (all
  cars), a jump over 30 m (teleport, reset to the road, respawn), another
  car id at its index in S_{n−1} (new: before, a recycled index within
  30 m was blended across two cars), a write after the tick (the harness's
  frozen car); the camera and crowd: mover change (`TickSnapshot::cut` =
  `view_history_key` differs between the two ticks: crash / jump camera,
  each shot of a scene's camera track, scene start and end, back to the
  chase camera), camera jump over 30 m or camera kind change, crowd frame
  stepping back or over 4 frames; the camera also where it was written
  after the tick (`FramePoses::camera_for`: photo mode's camera), which
  then is drawn as written (the dyno camera steps on the tick too,
  so it is in the snapshots).
  Not a cut: a change made outside the ticks (the harness's set-up steps,
  a load, a menu action that moves a car or the camera) — before each tick
  `refresh_snapshot` makes S_n the state the tick starts from, so the
  frames after it blend from where the world stood (the old interpolation
  took its "before" at each tick's start; without this the first ticked
  frame of a bench drew S_n and the history effects carried the
  difference for ~40 frames). A world load makes a new drive (new car id); replay seeks and the smooth slow motion keep
  their rule (`smooth_slow_update` replaces S_{n−1}'s poses and camera by
  the recorded frame's).
* **Still stepping in the draw** (not this unit's; inputs from the
  snapshot pair where they take a pose or the camera): the
  HUD's, the menus' and the garage's layout are pure reads of the tick
  state (`overlay_layout`, `menu_quads`, `render_garage`): the frame draws
  them as S_n, not as copies in the snapshot (shots set state and draw
  without a tick, so captured quads would be stale there; follow-up).
  Writes left: `apply_display_mode` (presentation); photo mode's
  `photo_begin`/`photo_end` (its mode swap: the paused race shown as the
  race); `DrawWrites::drive`: the effects' GPU halves and prepares (their
  simulations step on the tick, "Enhanced effects on ticks"),
  `water_prepare`, `tint.update`, `fog.update_rain`/`fog.drawn`,
  `fill_flare_pools` (blink step), `flag_girl_lights`, `horizon_fog`,
  `motion_blur_frame(&mut d.motion)`; the env-face counter;
  (particles, rain, world animations and smackables: blended,
  "Drawn objects between ticks" below). The drawn set

  `water_prepare`, `fill_flare_pools` (the pools' per-frame fill),
  `flag_girl_lights` (the tint, fog, flare blink, horizon fog and
  blur level step on the tick, "Native visuals on ticks");
  the shaders' clocks `seconds` / `blur_time`; particles, rain,
  world animations and smackables at tick positions. The drawn set
  of traffic is the tick's (`traffic_drawn`: placed and `active`), so an
  inactive car stays undrawn whatever its pose.
* **Exactness**: at 60 Hz (the harness draws α = 1) the frame values are
  the tick's own, byte-identical. Between ticks two differences from
  before, both by design (checked on the gate's 144 Hz window benches:
  with the old α = 0 blend put back the 3D scene is identical to base in
  every dump, the minimap alone differs): α = 0 frames now draw S_{n−1} bit for bit (was
  `CarPose::blend(…, 0)`, whose slerp and `up` renormalisation round), and
  the HUD's minimap / speed layout reads the tick state instead of the
  blend (it ran after the swap-in).
* **144 Hz windowed check** (Xvfb, NFSU2_FRAME_CAP=144, free roam,
  reversing then driving 20 s; `NFSU2_FRAME_TRACE`, a dev-harness aid:
  per frame the wall time, ticks, α, the drawn and the tick's player
  position): the drawn positions equal the old blend's `prev·(1−α) +
  cur·α` on the same frames to 2 ulp (α is logged with 6 decimals), so the
  stutter pattern (second differences of the drawn positions) is the
  base's by construction; it is dominated by the frame times of the loaded
  machine (docs/evidence/snapshots/win144.txt).

## Draw clocks (2026-11-13)

Every time value a shader or a draw-side animation reads is the tick
snapshots' blended by α (`src/bin/game/render/clocks.rs`):

| clock (`DrawClocks`) | tick value (`TickClocks`, in `TickSnapshot::clocks`) | readers |
|---|---|---|
| `seconds` (f32) | `Game::seconds()` (package clock / 1000; the harness's pin) | neon pulse (0x6227DE: underglow, tubes, its light), car lights and occluders, the world's and the cars' shaders (`FrameParams::time`, `car_lod_main`), car air, glow quads, the garage's scene |
| `blur` (f64) | `Pacing::world_time` | the clone's motion blur / depth of field, FSR 2's blur |
| `world` (f32) | `Drive::clock` (0x7FB718) | the water's drawn clock (`water::View::lag` = tick clock − drawn, exact), the wet roads' ripples (`set_weather`), the Next-Gen atmosphere's time |

* `time_at(prev, cur, α)`: exactly S_{n−1}'s at α = 0 and S_n's at α ≥ 1
  (bit for bit), `min(prev + (cur − prev)·α, cur)` between (monotonic, so
  also across a tick boundary), f64; a clock that ran back or has no
  S_{n−1} is S_n's. The same at every display rate at equal (tick, α).
* Cuts: a clock written after the tick (a load's new world clock; refreshed
  into S_n before the next tick, `refresh_snapshot`) is drawn as written; the
  harness's pinned clock (`Pacing::clock`: shots, the non-rate benches'
  per-frame clock) is the frame's own time and drawn as it is; under smooth
  slow motion the clocks take the tick clock's α (`Pacing::slow_tick_alpha`),
  not the replay's.
* The draw has no other clock: `FramePrep::seconds` is gone, `FrameView`
  carries only the pacing's frame count, focus and visibility (no
  `Pacing`), `FrameInputs::clocks` replaced `time` / `blur_time`, and
  `FramePoses::clocks` replaced `water_lag`. Before: the shaders read
  `Game::seconds()` (the package clock, stepping per tick: not
  smooth above 60 Hz), the blur `world_time − (1 − α)·dt` (with the slow
  motion's α), the water `clock − (1 − α)·dt` (f32, not exactly S_{n−1}
  at α = 0), the wet roads and the atmosphere the tick's world clock.
* Not blended (states, not times): the HUD's and menus' layout time
  (`now`), the garage request's time, the flare pools' blink,
  the screen flash, time of day and weather (world state), and the
  steppers keyed on the tick's world clock — the Next-Gen water's
  simulation (`water::View::clock`) and the wet car's spray
  (`wet_car::publish_weather`: one step per clock change; a blended clock
  would step it per drawn frame). Per-frame counters left: the env-cube
  faces (U8), the enhancement histories' frame indices (TAA, the depth of
  field's noise phase, presentation; the motion blur's noise phase counts
  ticks now).
* 60 Hz in the harness (α = 1) is byte-identical: each clock is the live
  value.
* Measured (docs/evidence/draw-clocks/): the α = 0 rate dumps at 120 /
  200 Hz against 60 Hz (racing and explore benches, Original and Extreme
  without TAA / volumetric fog) are unchanged in count (every frame still
  differs, from the per-frame visuals and the env faces: before
  this unit the shaders' clock was already the tick's at α = 0); what
  changes is the frames between ticks: the clocks move on every drawn frame
  above 60 Hz (`tests::draw_clocks`), where they stood still for a whole
  tick before. Capture: 24 B more per snapshot, 0.22 µs per tick with 20
  cars (unchanged). Image soak (3 runs × drive, race, racing, explore,
  rain at 1280×960): identical.

## Environment cube on ticks (2026-11-13)

The cars' environment cube (views 0x10–0x15) is rendered on the ticks,
not on the drawn frames. The original renders the faces of row
`0x870818 % P[rate] + 6·(rate − 1)` of 0x7FEEA0 each frame (rate 1: one
face, rate 2: faces 0 and 4 and one of 1, 2, 3, 5, rate 3: all six) from
that frame's car (+0.95 m) and main camera; its counter 0x870818 is
incremented once per frame (0x5D2C5E), read before the increment
(0x5C3981) and never reset (docs/formats.md, "Car Reflection Update
Rate"). At 60 fps a frame is a tick, so:

* **Schedule index** of tick count t = t − 1 (`snapshot::env_index`; the
  first tick's frame reads 0). The phase counts ticks from boot through
  menus and loads, never reset (before: drawn frames from boot, an
  offset against the ticks that differed per run — 555 in the racing
  bench, 1201 in the 60 Hz explore bench; invisible at rate 3, the
  Detail 5 default, where all six faces render each tick).
* **Eye per tick**: `TickSnapshot` capture writes `EnvEye { index,
  centre = car + 0.95 m Z, axes = CameraAxes::look(camera), time =
  seconds() }` of the drive into a six-slot ring (`Snapshots::env`, by
  index mod 6; a Copy array, no allocation; the whole capture measured
  0.35 µs per tick with 20 cars on a loaded machine, `capture_cost_per_tick`).
* **Draw** (`render/frame/scene.rs` `environment_cube`): the frame stands
  at tick `drawn` (the newest at α = 1, else the one before: the S_{n−1}
  the frame starts from) and renders `display::env_faces_due(rate,
  every, last, drawn)`: for each face due in (last, drawn] the newest
  due tick, oldest first, each from that tick's eye; `last` =
  `WorldRenderer::env_tick` (the cube's bookkeeping; None for a new,
  remade or cleared cube, which renders `drawn`'s own faces only, as the
  original's first frame on a new cube). A frame with no new tick renders
  none. Each face is rendered at most once a frame (a face due at both
  ticks of a 30 Hz frame is rendered from the newer: the same pixels, as
  every face render clears it), and only the last six ticks can matter
  (every face is due in any six ticks). The cube a frame samples is so
  a function of its tick alone: `cube_is_a_function_of_the_tick` checks
  it at 60 / 30 / 120 / 200 Hz and an uneven display, rates 0–3, with and
  without "Every Frame". The frame on the newest tick at α = 1 (the
  harness's 60 Hz, the default bench) uses its own car and camera (= S_n,
  and any harness write after the tick, as before), so 60 Hz is
  byte-identical.
* **Enhanced options**: Reflection Update Rate "Every Frame" renders all
  six faces each **tick** (was each drawn frame); Car Reflection Res
  only sizes the cube (a remake resets `env_tick`); Enhanced Reflections'
  world-axis basis is unchanged. Both are rate-independent at their
  setting.
* **Photo mode**: its camera (written after the tick) orients the faces
  while it is open, as before; the faces still follow the ticks.
* **Not in the eye** (the frame's values are used): the visible-section
  list and the fog (the fog is per tick now). They differ from S_m's
  only for the older tick of a frame spanning two (30 Hz, stalls), i.e.
  at a section boundary or while the fog eases.
* **Cost**: at 30 Hz a frame renders the faces of two ticks (rate 1:
  two faces, rate 2: four, rate 3 / "Every Frame": still six, from the
  newer tick); per second the same as at 60 Hz. At 120 / 200 Hz only
  frames with a new tick render faces (before: every frame, so the cube
  cost fell by 2× / 3.3×).
* **Rate gate**: the `envf` column hashes the tick's schedule index, the
  faces due at it and its eye (before: the drawn-frame counter); the
  rate harness writes a digest of the cube's six faces per dumped tick
  (`NFSU2_RATE_DUMP/env.tsv`, `WorldRenderer::environment_digest`, the
  cube readable under the dev-harness feature only).

## Enhanced effects on ticks (2026-11-13)

The Enhanced effects that simulate or accumulate step once per tick, from
the tick's own state, in the tick hook `Game::enh_fx_tick`
(src/bin/game/render/enh/fx_tick.rs, called at the end of `Game::update`,
so the window's ticks and the harness's spans step them alike); a drawn frame reads what the ticks left and applies
the ticks' commits to its GPU caches in order. Before, each stepped in the
draw (keyed on the drawn frame, its blended poses, its camera or its
count). Classes (audit plan phase4-rate §1.4): **blend** = drawn
`lerp(S_{n−1}, S_n, α)` (exactly S_n at α = 1, S_{n−1} at α = 0),
**stepped** = drawn as the newest tick left it, **history** = a GPU buffer
committed once per tick in tick order (exact at tick frames, the newest
commit between them).

| item | effect | CPU half (tick) | GPU half (draw, cache) | class |
|---|---|---|---|---|
| 1 | Wet Car | `CarRenderer::tick_wet` (the car's `WetState`, its last two ticks' `WetParams`) | `frame_wet`: `WetParams::blend` by the frame's α (`wet_car::publish_alpha`) | blend |
| 2 | spray pool, camera velocity, the eye's spray (`SPRAY_HIT`) | `WorldRenderer::enh_tick` → `WeatherSim` (`Spray::advance` on the world clock) | `WeatherFx::prepare` reads `WeatherSim`; spray volume built per frame | stepped |
| 3 | Lens Drops | `lens_tick` (after the spray, with the tick's hit) | `lens_frame` (read) | stepped |
| 4 | wet-road trail map | `TrailSim::commit` (stamps, decay, window per tick) | `WetRoad::apply` per pending commit, then `road_values` | history |
| 5 | Cinematic smoke | the feeder's ticks (as before) + the rain smoothed per tick (`Feeder::wet`, `Tick::wet`) and each tick's own time (`Tick::time`) | `GpuSmoke2::simulate` replays the ticks; the volume grid per frame | history |
| 6 | High / Ultra GPU smoke | the feeder's ticks with their own time | `GpuSmoke::simulate` replays them | history |
| 7 | Sparks & Fire | `Sim::record`/`step` in the world update (as before), S_{n−1} kept (`Sim::prev`, ids) | `Sim::drawn(α)`; the draw no longer clears the sim | blend |
| 8 | Ambient Detail | `AmbientDetail::tick`: debris, grit, the volume's emits and steps (generator seeded by the tick count, was the drawn-frame count) | `prepare`: instances, uniform, the pool fed the queued steps, export / build | stepped (debris), history (volume pool) |
| 9 | VolumeFx particle pool | steps queued only by ticks (as 8; was one per drawn frame, 1e-4 s between ticks) | `ParticlePool::export` replays them | history |
| 12 | Sky Next-Gen panorama rows | `SkyTick` per tick (its eye, clock, rain, sky pair) | `NightSky::plan_rows` / `update`: per tick a quarter (phase `tick % 4`) from that tick's values, one uniform slot each; the first visible frame every row from the newest tick | history |

The lists the draw's glow pass pushed into statics (Wet Road Interaction's
`CarExtra`, Ambient Detail's `CarIn`) and the cars' renderers pushed while
queuing (the spray's `CarSample`) are built by the tick hook from the tick
state, the same cars in the same order; the glow pass's lists keep their
one-tick lag (the draw's push was a frame late), so a steady 60 Hz frame
reads what it read.

* **Exactness**: `NFSU2_ENH_HASH=file` (dev-harness) writes `Game::enh_digest`
  per tick. Racing bench in the rain (NFSU2_RAIN_CLOCK=162) with every
  effect above on: 30, 120, 200 Hz and jitter:7 identical to 60 Hz at all
  360 ticks (`tests::enh_ticks` the same on free roam, offscreen).
* **60 Hz picture** (racing bench, each effect alone, base vs new at
  60 Hz): Wet Car, Sparks & Fire (no contact in the bench) and the sky
  (clock 100 s, night) bit-identical; both smoke tiers identical on the
  racing bench, the High / Ultra smoke ±2 LSB noise in explore's first
  frames (gate `enh-explore-extreme`: each tick's own time instead of the
  frame's time summed back, 1 ulp); the spray (133 px), lens drops,
  trail map (300 px) and Ambient Detail (131 px) differ slightly
  (attributed: the trail map, spray and lens no longer reset when a bench
  or shot jumps the clock between two draws, the lens sim restarts with
  its generator on a jump; Ambient Detail's generator counts ticks).
  Before / after crops: the maintainer's private captures.
* **Cost**: the hook runs ~0.95 ms a tick in the rain at Extreme (spray
  and trail 0.73, Ambient Detail 0.2, the car lists < 0.01): the same work
  (same particle counts) the draw did before, now on the tick; at 200 Hz
  it runs 60 times a second instead of 200.
* **Cost after PERF-RAIN (Phase 6, 2026-11-13)**: measured with the bench's
  `prof::acc` spans (`tick: enhanced hook`, `tick:   spray step`, …), the
  work did not grow with the move: the draw before the move spent it in
  `  particles` on the main thread (`upload_particles` →
  `prepare_weather_fx`), 7d1bfab spends the same on the main thread in the
  tick (5 000 puffs + 4 000 mist particles × ~10 cars, two `exp` per car in
  reach: `Spray::step`'s ride on the cars' wake 0.44 ms, `Spray::hit` 0.11
  ms with a list of every puff a tick). Now: the ride of each puff and the
  draw's `volume_particles` run in chunks on the render job pool
  (`render/enh/weather/par.rs`, `chunks_mut`: the calling thread and three
  jobs take chunks from one counter, the caller then waits for every job
  inside a chunk; a job the pool starts later finds the call closed and
  touches nothing), joined inside the call, before anything reads the
  state; each element is worked by one thread with the serial loop's
  function, so the state is the serial one bit for bit. `Spray::hit` skips
  a puff out of its reach before making it (same sum, same order, no
  list). Hook 0.60 → 0.21 ms a tick (Extreme, rain), the spray step 0.44 →
  0.18, the hit 0.11 → 0.01; the draw's volume particles 0.23 → 0.12 ms.
  `NFSU2_ENH_HASH` identical to 7d1bfab's successor bc8467c at 30 / 60 /
  120 / 200 Hz and jitter:7 (racing and the rain drive, Extreme and a play
  setup), and over 20 repeated rain runs.
* **Not converted here** (next section): LOD fade, lamp pop fades, RT cache
  eviction, water time, WGSL clocks and the history passes.

## Enhancement effects on ticks (2026-11-13)

Clone-only (opt-in enhancements; the Original preset reads none of it).
Library half `src/render/enh/tick_views.rs`, game half
`src/bin/game/render/enh/tick_views.rs`.

* **Tick views.** Each tick's end records (tick index `GameClock::tick`,
  the game clock's seconds, the world clock, the drive's camera) in a ring
  of four (`Snapshots::recent`, `Game::record_tick_view` from
  `capture_tick`; no allocation once filled). Each drawn frame publishes,
  before the world is planned (`Game::publish_tick_views` in
  `draw_frame`), the ticks taken since the last published frame, each with
  the sections drawn from its eye (`drawn_sections_at`, `far_sections_at`);
  the newest takes the drive's camera as the frame finds it (a camera the
  harness wrote after the tick), a tick taken without a record (the plain
  bench's `step`) the live values. It also publishes the frame's place on
  the tick axis (n − 1 + α) and its lag on the world clock ((1 − α)·dt).
  Without a drive nothing is published and every effect steps per draw as
  before (shots, the front end's scenes). The garage's scene drawn while a
  drive stays loaded (a shop or the crib opened from free roam) sets the
  drive's frame aside for its own draw (`tick_views::SetAside` in
  `render_garage`): the drive's cameras and sections are not the
  platform's, and LOD Fade stepped with them faded the whole platform out,
  leaving the car on the clear colour
  (`tests::garage_drive_ticks::garage_ignores_a_loaded_drives_ticks`).
* **Simulations and schedules, one step per tick** (`TickGate`: each
  effect steps every published tick it has not stepped, oldest first, with
  that tick's camera, sections and time; 30 Hz steps two a frame, 200 Hz
  none on most frames; a frame between two ticks draws what the last step
  left):
  - LOD Fade (`WorldRenderer::update_fades` → `fades_step`): the
    per-instance fades, the sections fading out, the section cross-fades.
  - The lamps' section fades (`PopFades`), the shadow layers' hand-out
    (`assign_shadow_layers`: the budget of new layers is per tick) and the
    shadows' fade-in times (`POP_SHADOWS`), one gate (`LampState::gate`).
  - The traced scene's car cache (`rt::scene`): its clock counts ticks;
    models unseen for 600 ticks go, at most 4 new models a tick.
  - The Next-Gen water: the jets' particle allocation and the
    simulated basins (the steps' impulses) are decided at the newest tick's
    eye and view and kept between ticks (`Water::tick_pick`); its steps were
    already keyed on the world clock (one per 1/60 s of it).
  - Light probes: at most one new bake a tick.
* **Clocks:** the rain ripples' (Wet Roads), the wet cars' and the
  Next-Gen atmosphere's noise clock is the world clock less the frame's lag
  (`tick_views::drawn_clock`), as the water's already was: exactly the
  tick's clock on a tick's own frame, smooth between.
* **Histories, per drawn frame by nature:** TAA / temporal
  upscaling, the Next-Gen atmosphere's froxel history and the volumetric
  particles' temporal resolve accumulate drawn frames; their per-frame
  weight is taken over the frame's span s in ticks (`FrameSpan`):
  new-frame weight 1 − (1 − w)^s, history keep h^s (TAA: the motion term is
  per tick, velocity / s). Exactly the old weight at s = 1 (any span within
  1/1000 of a tick), so a steady 60 Hz is the old path bit for bit. The
  jitter index stays a drawn-frame counter (a pure function of the frames
  drawn); the previous transforms are the last drawn frame's, built from
  the snapshot pair and α. What remains rate-dependent is the sub-pixel
  jitter phase at a given tick and the number of samples averaged: the
  bound is measured on the rate gate's `hist` image class.
* **FSR 2** keeps its per-frame accumulation (its weights count
  samples, as AMD's reference does; no time constant to scale): left as
  is, inside the `hist` bound.
* **Motion blur**: the motion over a fixed shutter, from `blur_time`
  = the snapshot pair's time; its motion and noise phase now come from the
  ticks too ("Rate independence: what the gate guarantees", "Motion blur on
  ticks"; before, from the last drawn frame, which differed between rates).
* **No auto-exposure / eye adaptation exists** in the clone (the photo
  grade's exposure is a fixed gain), so there is nothing to tick there.
* **Test:** `tests::tick_effects::tick_stepped_effects_are_the_same_at_every_rate`
  (free roam, throttle held from the first tick, LOD Fade + keep-loaded,
  Dynamic Lamps + High lamp shadows, Next-Gen water, probes, drawn
  offscreen at 30 / 60 / 120 / 200 Hz and `jitter:7`): the digest of the
  tick-stepped state (`tick_views::digest`, `Water::tick_digest`) after the
  frame that took tick n is the same as at 60 Hz at every common tick.

## Drawn objects between ticks (2026-11-13)

Everything the world view draws that moves on the tick now has its S_{n−1}
in the snapshot (`TickSnapshot`: `drive_key`, `particles`, `owners`, `rain`,
`posed`, `photo`, `rollers`; filled by `Game::capture_drawn_objects` at the
capture) and is drawn by `Game::frame_objects` (in `frame_poses`) as a pure
function of S_{n−1}, the tick's live state (S_n) and α. Each is exactly
S_{n−1} at α = 0 and the live tick state at α ≥ 1 (the harness's 60 Hz
frames: α = 1, so every shot and 60 Hz bench is unchanged).

| kind | before (stepped on the tick, drawn at it) | drawn between ticks |
|---|---|---|
| native particles and sparks (`Effects::system`, one system for smoke, sparks, glows, nitrous flames, purge, markers, placed emitters, the flag girl's flares) | `render/native/particles.rs` drew `d.effects.system.particles` (only the nitrous / purge flames re-posed) | `nfsu2::particles::between`: paired by `Particle::id` (new, clone only: counted up per spawn in `System::emit`, never read by the simulation); in both ticks: the blend of the two positions and of the life (colour / size curves), the flip-book frame held at S_{n−1}; born in tick n: S_n, size × α (`System::quads_scaled_where`); died in tick n: not drawn; moved over 30 m: S_n. Carried particles (`FLAME_KEY` + owner): blended in the owner's frame (S_{n−1} in S_{n−1}'s owner transform, S_n in S_n's) and put in the owner's drawn pose — the flames' rule, generalised (a newborn keeps the flames' drawn × newest⁻¹) |
| falling rain (`Weather::drops`, 0x613A80) | `rain_quads_at` from the live drops | `nfsu2::weather::RainSnapshot::between`: a drop falling in both ticks at the blend of both its positions (the streak's head and tail); wrapped round the drop box (its older position moved over 1 m): S_n; started falling: S_n (a zero-length streak); stopped: not drawn; the stretch blended, the "ahead" colour held |
| lens drops (0x614100 / 0x5C7AC0, `Weather::splats`) | `rain_frame` from the live splats | running in both ticks (timer counting down): place and timer (→ alpha 3·timer/life) blended; (re)started in tick n: S_n with its strength × α; ended: not drawn. The Lens Drops enhancement's own drops (`weather_fx::lens_drops`) step on the tick ("Enhanced effects on ticks") |
| world animations (searchlights, planes, cranes, trains, sails: `World::step_loops`) and knock-over props (`step_with_props` / `step_world`), both in `Drive::posed` | `place_markers(&d.posed)` | `nfsu2::interpolate::transform_at`: scale and translation blended, rotation slerped; a placement S_{n−1} had not moved stood at its rest transform (`World::placements[p].transform` = the prop's `rest`); over 30 m (a prop put back, a loop's restart): S_n. No line of src/smackable/** changed: the props unit's collision changes merge cleanly; the blend reads only `Drive::posed` (filled by the drive's step as before) |
| photo mode's rig (`Photo::rig`, moved in `photo_tick` from the held keys / sticks / drag) | `photo_begin` → `photo_camera` from the live rig | `Rig::between` (each value blended), unless the rig was set after the tick (a key: the live rig) |
| the dyno's rollers (`Rollers::angles`, `turn_by` per tick) | `prepare_frame` → `transforms()` | `Rollers::angles_at`: each the way its wheel's spin turned it (`Rollers::spins`, new: the last `turn_by`'s), as the cars' wheels (over half a turn a tick at a fast pull) |

Listed from the code and left at the tick's state, with the reason:
the cars' wheels, steering, suspension and hydraulics (already in
`CarPose`); the scene crowd and the flag girl (the crowd frame);
the GPS aid (placed from the frame's camera); flip-book textures, neon and
water clocks (the shader clocks); the garage's own scene (FE car on the
turntable, its hood / trunk / door animations, its marker particles): its
own snapshot, "The garage scene between ticks" below; skid marks (they do
not move).

**Cut sources** (one rule per kind, in `frame_poses` / `frame_objects`):
`drive_key` differs (a world load, a car change: the particle ids and
placements start anew) → all of them live; a particle or placement over
30 m, a drop over 1 m → S_n; photo rig set after the tick → live. **Refresh**:
`refresh_snapshot` (before every tick) re-captures all of them into S_n
unconditionally (a copy costs what a comparison would), so a change made
outside the ticks (the harness's set-up, a shot's state, a replay seek,
which rebuilds `posed`) is where the next tick's blend starts.

**Cost**: the capture and the refresh copy ~85 particles, the 400 drops,
~15 placements: 2.5 µs each per tick in free roam with 20 cars on an idle
machine (`tests::snapshots::capture_cost_per_tick`; 5.5 / 6.3 µs under the
gate's load), allocation-free once the buffers have their size.

**Tests**: library — `particles::between::tests` (ends, pairing by id, the
dead / newborn / jumped, carried), `weather::tests::rain_between_ticks_…`
(240 raining ticks: the snapshot draws the weather's own streaks and lens
drops at each tick; α = 0 / 1 the ticks', α = 0.5 within),
`interpolate::transform_at_tests`, `photo::between_tests`,
`dyno::rollers_between_tests`; game — `tests::snapshots::drawn_objects_are_
the_ticks_at_the_ends_and_between_inside` (free roam in the rain: α = 0 is
S_{n−1}'s particles / streaks / lens drops / placements bit for bit, α = 1
the live state, α = 0.5 inside) and `…_the_same_at_every_display_rate`
(the same drive through `ticks::Frames` at 30 / 60 / 120 / 200 Hz: the
tick state equal at every common tick, and the drawn objects equal at α = 0
and 0.5 of every tick and at every drawn frame's (tick, α)). The state
hash's `fx` column (particles' key, description, life, position; the
rain's running state) was already identical at every schedule in every
flow ("fail" before the harness ran on the tick cadence); the blend adds no hashed tick state.

**Images**: the gate's 144 Hz window benches differ from before by design
(the frames between ticks now show the particles — here the tyre smoke
around the car — at their blended places, newborn ones scaled in); every
60 Hz image, shot and enhanced dump is byte-identical.

## The garage scene between ticks

The front end's garage (GarageMain's scene under the menus: the showroom,
the crib, the shops, the car lots) steps on the tick
(`garage_tick`) and is drawn by `render_garage` with its own camera. Its
tick state now has a snapshot beside the world view's
(`TickSnapshot::garage`, `src/bin/game/garage/snapshot.rs`):

* **Capture** (`Game::capture_garage`, in `capture_tick` and, before
  each tick, `refresh_snapshot`): `GarageSnapshot` = the view as drawn at
  the tick (`GarageRequest::view.at(now)`: the camera's field of view,
  target and eye, the car's angle and steer, the platform's angle and the
  car's disc), a hash of the platform and the FE car shown (name, parts,
  performance parts), the request's time, the car's place on the turntable
  (`GarageScene::car_world`), the hydraulics' corner heights and wheel
  drops, the hood / trunk / door poses (`car_poses`), the camber, the
  Lights list's flares and the scene's particles (`GarageScene::fx`).
  ~0.5 KB fixed + 48 B a particle; the particle buffer is reused.
* **Frame values** (`Game::frame_garage(α)`, in `frame_poses` →
  `FramePoses::garage`, handed to `render_garage` as `drawn`): the view's
  values blended (`fx::tick::lerp`: exactly S_{n−1} at α = 0, inside
  [S_{n−1}, S_n]), the car's place made again from the blended turntable
  and corner heights (`Turntable::car_transform_on`), the wheel drops and
  the part poses blended (rotations slerped), the camber and the flares held
  at S_{n−1}, the particles by id as the world's
  (`nfsu2::particles::between`: newborn ones grow in, dead ones go).
* **Cuts** (the tick's own drawn, as at α = 1): no garage in S_{n−1}
  (entering the garage, the shop drive-in's handover to the menus), another
  platform or FE car (a car change, a part change's new car, a lot's
  step), the car on another disc (a car change's platform turn starting),
  a request or scene written after the tick (a harness's own
  `garage_request`), no garage now (leaving: nothing drawn).
* **Exactness**: α ≥ 1 (every frame of the harness at 60 Hz, every shot:
  `draw_garage` passes no frame values) draws the tick's own state through
  the unchanged path, byte-identical. α = 0 frames draw S_{n−1}'s values
  bit for bit, so `render_garage` draws tick n − 1's picture pixel for pixel
  (`tests::garage_between::garage_images_at_alpha_zero_are_the_ticks`, on
  120 and 200 Hz schedules while the camera blends to a part's view);
  a 30 Hz frame k (α = 0 after tick 2k) is the 60 Hz window's frame 2k.
* **Smoothness** (`tests::garage_between::garage_frames_are_the_ticks_and_the_same_at_every_rate`,
  the body shop's list opened: the car turns to the part's view and the
  camera blends), per drawn frame while the car turns: at 120 Hz the
  turntable stood still on 19 of 37 frames and jumped 2.40–3.03° on the
  others (the tick's own state, what every frame drew before), now it turns
  1.20–1.52° on every frame; at 200 Hz still on 44 of 62, now 0.72–0.91° on
  every frame; the camera's eye likewise (120 Hz: 0.00014–0.00252 m a
  frame, none still; the spread is the blend's easing). The state at every
  (tick, α) two schedules share (30 / 60 / 120 / 200 Hz, jittered) is the
  same.

## The shops' drive-in and drive-out between ticks

The original runs every in-engine scene under its 1/30 s limiter (0x437910
→ 0x435F40), so its NIS mover (0x4309D0 → 0x5A40B0) takes one step per
30 fps frame: frame times of 60, 60, 60, 60, 61 ticks of 1/1800 s (its sim
clock in the drive-in capture, docs/evidence/shop-scene-mover/, counts
133, 133, 133, 133, 136 of 1/4000 s per mover count). The shops' and the
garage's drive-in / drive-out scenes are not held to 30 fps here
(docs/native-deviations.md "High refresh rates"), so they tick at RACE
(30, 29, 31). Before, `scene_mover_step` let those ticks build up to
1/30 s, stepped with what had built up and threw the rest away: three
ticks a step, so the car moved 20 times a second while the scene camera
glided (at 60 fps and above).

Now the car has its own 30 fps schedule (`nis::ScenePace`, only for a
scene with a shop): each step's frame time from a `FrameClock` limited to
1/30 s (60, 60, 60, 60, 61) and its target at the scene frame those steps
reach, so the steps are those of a 30 fps run and a function of the scene
alone, the same at any display rate. A step is taken once the world's
scene time reaches the start of the frame it ends (one step ahead), and
the tick snapshot captures the car between the pose before its newest
step and after it at the world's scene time (`CarPose::drawn`, used by the
capture, the refresh, `frame_poses` and the live frame), so the car moves
on every drawn frame and stands where the camera's time puts it. The
simulation writes nothing for the draw. The 61-tick frames make the
schedule 1/1800 s per five steps longer than real time (0.33 %, as the
original's scene clock); the scene still ends on the world's time.
Cut-scenes (race starts, endings, the career's start, the dyno) are
unchanged: under the 1/30 s ticks every tick is one step.

**Tests**: `tests::scenes::shop_scene_car_takes_the_30_fps_steps_at_any_frame_rate`
(ShopEnter1 / ShopExit1 at 30 / 60 / 120 / 200 fps: the same steps bit for
bit, the limiter's frame times, no drawn frame standing still while the
scene moves the car); `shop_and_dyno_scenes_drive_the_car_like_the_original`
(the path per mover count against the original's).

## Native visuals on ticks (2026-11-13)

The original's per-frame visuals step once per tick (`Game::visuals_tick`
at the end of `Game::step`, `src/bin/game/render/visuals.rs`), at the
camera the tick's frame draws with (photo mode's swapped in as the frame
swaps it), on the ticks whose frame draws them: the world's scene
(`world_under` without a garage request, or the tuning menus over the
dyno), the flare blink with Light Glow, the horizon fog with its switch,
the blur with the native speed blur; the overbright level on the ticks
whose frame runs the screen passes (the world's or the garage's scene).
S_n (`TickSnapshot::vis`, `overbright`; ~100 bytes, copied) holds what
tick n's frame draws with; a frame takes `Game::frame_vis(α)`
(`FramePoses::vis`): continuous values interpolated (S_{n−1} exactly at
α = 0), the flare blink and the rain colour held at S_{n−1}, S_n at α ≥ 1,
across a new drive or a camera cut. The draw writes none of them (the
drawn fog for the smoke is the scene's own, `WorldScene::fog_drawn`).

| effect | native order | S_n |
|---|---|---|
| overbright 0x876848 | 0x5BC110 after the draw (0x5D2B6A) | the level before the tick's step (verified, the clock audit's row 24) |
| nitrous blur 0x870900 | 0x5CC470 steps, then uses (0x5CB32F) | after the step |
| screen tint (slot 3) | read by the screen passes at 0x5CB920 (+0x188, +0x1CC), sampled by 0x5DBE90 → 0x5D7800 at 0x5CBAEC (and 0x5D2C31), both after it | the sample of the tick before (**traced**; the clone sampled and used it in the same frame: one tick later now, attributed) |
| fog | 0x614310 at 0x5CB03D, before the world's draw | after the step |
| horizon fog visibility | 0x5DB910 (from 0x5C7480 at 0x5CB81C), then the grid | after the step |
| flare blink | 0x48AC00 at 0x5CBAF1, the frame's end | the per-frame code's: stepped by the sim clock's f32 difference |

Each keeps its own smoothing distance (tint 50, fog 50 and the rain fog
the "Fog Blend Distance" map, horizon fog 20). A still (`--shot`, drawn
without a tick) steps them once at its camera and draws at once, as every
frame did before (`render::visuals::still_step`).

Consequences at 60 Hz (attributed): the tint one tick later; the
harness's ticks without a drawn frame (bench warm-ups, `step_for` before a
draw) now step the visuals as the window's ticks do; a scene's 1/30 s tick
steps them once (the original's scene loop runs at 30 fps), not once
per drawn 60 Hz frame. Open native differences (not changed here): the
blink's dt is the world dt 0x865150 in the original (the clone's sim-clock
difference includes the tick's own dt); the nitrous blur's slot 1 is set by
0x5DBE90 (0x5DBF44 / 0x5DBF50) after the blur used it (0x5CB32F), so the
original's level steps on the previous frame's nitrous state (**inferred**
from the listing).

## Streak buffer on ticks

The light-streak buffer C (decay by (120, 90, 170)/255, the glow sprites,
then the overbright pass's A × the level's add factor, 0x5CBB10 /
0x5CCB9A) commits once per **tick**, not once per drawn frame
(`src/bin/game/render/streak.rs`; the commit-only chain
`PostFx::commit_tick` in `src/render/native/post/mod.rs`). The original runs
the streak pass once per loop iteration, the scene loop's 1/30 s
iterations included, so at 60 fps commit n is a function of tick n's frame
alone: its glows (drawn with its camera, depth-tested against its scene),
its overbright level and A, made from its scene.

* **Slots.** A commit is keyed on the newest tick a draw stands after
  (`GameClock::tick`, `FramePoses::tick`; bookkeeping `RenderState::streak`).
  The frame drawn after tick n commits slot n with what it draws, as the
  60 Hz frame after tick n does (α = 0 in the window and the rate runs:
  S_{n−1}; the harness's `draw()` on the tick: S_n). A draw with no new
  tick commits nothing. Scene ticks (1/30 s) commit once per scene tick.
  Shots: unchanged (16 commits of the still).
* **Exact form (probes).** A slot no drawn frame stands on exactly is
  committed by a **probe**: the 60 Hz frame of that tick drawn offscreen at
  α = 0 — the world's scene and the post chain up to the commit (decay, that
  frame's glow sprites, A from that scene), no output, no UI, not
  presented, not counted. It is drawn where the 60 Hz frame would be:
  between two ticks of one drawn frame (`Game::probe_between_ticks`) and,
  for a frame between ticks, before that frame (`Game::draw_at`). The
  probe's Glows pass gives the commit its tick's glow list, camera and
  depth. A frame between ticks (0 < α < 1) shows the scratch B(α) =
  sat(q(k ⊙ C) + G(α) + A(α)·f) over the committed buffer (C kept and put
  back); B(1⁻) = C_n, and B(0⁺) already carries one more decay than
  C_{n−1} (frames between ticks have no reference in the original).
  Exact: every α = 0 frame is the 60 Hz frame of its tick, at any rate.
* **Cheap form.** A frame commits the slots it missed itself — as many as
  the ticks it took, at most 3 — each with its own glows and A; a frame
  that commits shows what it committed, a frame without a new tick the
  scratch. The number of commits per second is the 60 fps one at every
  rate (trail length and decay per second unchanged); only the commit's
  scene is the frame's instead of its tick's: the commit samples the scene
  up to one frame interval after its tick (the sub-tick phase), so a
  moving light's trail samples sit up to that much further along its path.
  A row in docs/native-deviations.md.
* **Which form.** The harness (shots, flows, the rate gate, the tests: a
  deterministic clock) is always exact; the gate's 144 Hz window bench too
  (it takes no wall-clock samples). The temporal pass (TAA / temporal
  upscaling: a history per drawn frame a probe would feed) is always cheap.
  The window chooses from measured work (`Governor`, fed once per frame in
  the frame loop while the trail buffer is in use: a scene's frames, not
  the menus'): the cost c of one scene render is the larger of its GPU
  time and the main thread's rendering per render. The GPU time comes from
  a timestamp pair (where the device has timestamp queries; requested at
  device creation) around exactly what a probe draws: the first one in a
  command buffer of its own submitted before every other of the frame (the
  world's views and the environment cube, encoded on render threads, go
  first), the second written by the post chain right after the streak
  buffer's commit (`PostFx::set_commit_stamp`), so a drawn frame's output,
  screen passes, photo grade and UI are not in it. The main thread's
  rendering is the frame's draw and probes, without its ticks' simulation
  (a probe simulates nothing) and its waits for the swap chain image and
  the frame before. Without timestamps, when the frame is not held by
  vsync at a known refresh or by a frame cap, the frame's interval per
  render counts too (the machine's throughput). Exact mode spends up to one
  render per tick on probes, so their predicted load is c / tick period.
  c is the **median** of each quarter second's frames, smoothed over 0.5 s:
  the typical render. Hitches (streaming, a loaded machine's scheduling, a
  load inside a frame) are rare frames and do not move it however long they
  are; a time-weighted mean lets them dominate (in a window at 700 fps
  uncapped a few 30–60 ms frames a second put the predicted load over the
  threshold and the trails sat in the cheap form, the dwell doubling on
  each return). A frame over 4 ticks long, or whose own work is (a load
  inside it), is no sample. Cheap above 30 %, exact again only under 18 %
  (the cost of all a probe draws: 4K Original on a strong GPU is 13–15 %,
  1080p Extreme 16–17 %, both kept exact; 4K Extreme 60 %, cheap).
  The drawn frames take their own share on top: one render per frame
  shown, c / the display's period (the monitor's refresh with or without
  vsync, or the frame cap's when longer; the bench's `NFSU2_BENCH_HZ`
  schedule). Probes plus frames over 90 % is cheap too, exact again only
  under 75 % (the frames' output, screen passes and UI are not in c, so a
  tenth is left for them): a small GPU whose frames fill most of each
  refresh has no room for a probe, although the probes alone look cheap
  against the tick (an integrated GPU at 1366 × 768 Original and 144 Hz:
  probes 21–30 % of the tick, frames 66–112 % of the refresh;
  docs/perf-integrated-gpu.md). With the period unknown (uncapped, no
  refresh read) only the probe load counts, as before. All this
  after at least 1 s in exact and 2 s in cheap (the cheap dwell doubles, up
  to 32 s, when exact did not hold for 10 s); the first decision at the end
  of the first quarter second in a scene. A frame over 2.5 ticks long (a
  hitch) commits its ticks itself even in exact mode: no burst of probes.
  The developer knob `NFSU2_STREAK_FORM=exact|cheap|auto` (dev-harness
  builds) forces a form everywhere, or (`auto`) lets the bench's frames
  choose as the window's do; with any of them the bench prints a
  `streaks` line (probes, the cost timer's GPU time per render). No menu
  option.
* **The decision on synthetic machines** (`render::streak` tests): GPU-bound
  without vsync at 40 / 75 / 140 fps → cheap (one change); vsync 60 with
  missed frames → cheap; a 50 Hz display, a 30 fps cap, 144 and 200 Hz and
  uncapped at 0.7 / 2.5 ms a render, all with headroom → exact, never left
  (50 Hz and the 30 cap also through hitches); a fast machine uncapped
  (1.4 and 2.75 ms frames) with a 30 ms hitch every hundredth frame and a
  frame with a load in it first → exact, never left (with and without
  timestamps; the previous rule went cheap); vsync at exactly 60 and 120 Hz
  → no probe drawn, no change (a 60 Hz machine using 13 ms a frame goes
  cheap once); a starved machine → cheap within a second, at the start or
  when the load arrives; a 0.5 s stall → no change; a load between the two
  thresholds → no change; the dwell and its doubling; the knob; frames
  filling most of a 144 Hz refresh (4.6 / 5.0 ms a render against a 60 Hz
  tick, 7.4 ms against a 30 Hz one; vsync and mailbox) → cheap within
  1.5 s, one change, while the same renders with the refresh unknown stay
  exact and a strong GPU at 144 Hz (1 / 2.3 ms) never leaves exact
  (`frames_filling_the_refresh_go_cheap`).
* **The cost timer against a probe's cost** (`--bench explore` at
  `NFSU2_BENCH_HZ=30`: every frame spans two ticks, one probe a frame in
  the exact form, none in the cheap; free roam on a strong desktop GPU).
  Predicted: the timer's GPU time per scene render (exact
  run / cheap run). Measured: the GPU time a drawn frame takes in the exact
  run minus the cheap run's (the bench's timestamps, every pass and the
  gaps between submissions).

  | | predicted | measured |
  |---|---|---|
  | 1920 × 1080 Original | 0.55 / 0.61 ms | 0.45 ms |
  | 1920 × 1080 Extreme | 2.57 / 2.81 ms | 2.85 ms |
  | 3840 × 2160 Original | 2.12 / 2.55 ms | 1.81 ms |
  | 3840 × 2160 Extreme | 9.82 / 10.53 ms | 10.97 ms |

  The timer's earlier span (from the cars' command buffer to the end of
  the post passes, the world's views and the cube outside it) read 0.51,
  2.28, 1.26 and 9.04 ms there. Its GPU test:
  `streak_ticks_cost_timer_ends_at_the_commit` (the timestamp written once,
  at the commit, by the commit alone and by a full chain).
* **A switch** (`tests::streak_ticks::a_switch_to_the_cheap_form_is_not_visible`,
  free roam at 144 Hz, Original, switched at a tick): the frames before it
  are the exact run's, the committed slots stay the same after it, and the
  pictures differ from the exact run's only near the moving lights (at most
  0.09/255 mean, 7.8 % of the pixels at 320 × 240), while consecutive frames
  differ by up to 8.1/255 in both runs: the switch is no jump (the largest
  frame-to-frame change is the same with and without it).
* **Exactness** (exact form): the GPU exact-path tests (`cargo test --release
  --features dev-harness --lib streak_ticks -- --ignored`; a commit alone
  over its own scene then the next tick's frame = the 60 Hz path's two
  frames, over two different scenes, bit for bit); the game test
  `tests::streak_ticks` (free roam, Original, light trails on: every α = 0
  frame at 30 / 120 / 200 Hz = the 60 Hz frame of its tick, image and
  committed buffer; with uneven 1–4 ms frames the buffer after every slot
  = 60 Hz's); the rate gate's `orig` image class: identical at 30 / 120 /
  200 Hz and counted (`images@orig fail` in scripts/checks/rate-columns.txt).
  The `step` and `hist` classes differed at 120 / 200 Hz by the motion
  blur (also with the light trails off); identical and counted since the
  blur takes its motion from the ticks ("Rate independence: what the gate
  guarantees").
* **Cost of exact mode on a strong machine** (racing bench, Original, the
  bench's frames back to back at the display's schedule, `auto`; probes
  per second = the ticks no frame lands on: 45 at 75 Hz, 30 at 90 Hz, 48 at
  144 Hz): 1080p, 0.45 ms GPU a scene render → 20 / 13 / 22 ms of GPU per
  second of play (2.0 / 1.3 / 2.2 %), predicted load 5–8 %; 4K, 1.9 ms →
  85 / 56 / 94 ms per second (8.5 / 5.6 / 9.4 %), predicted load 17–18 %:
  exact in every frame. At 60 Hz no probe is drawn at all.
* **Starved machine** (4K with 16x supersampling in Original, the racing
  bench at a 75 Hz schedule, 30 s, 4 interleaved runs each; median frame
  time of the runs, range of the means): the per-frame commits 21.4 ms
  (20.9–25.0); exact form always 29.7 ms (30.4–48.6: +39 %, one probe per
  tick on a 20 ms frame); the measured choice 21.9 ms (21.0–23.6): cheap
  after its first 18 frames (11 probes), predicted probe load 130–150 %.
  The machine was heavily loaded by other work, so the runs scatter by
  ±10 %; the measured choice is within that of the per-frame commits. The
  first frame between ticks allocates the scratch's copy of the buffer
  (530 MB at 4K × 16x supersampling: a one-off 0.4 s frame there; 33 MB at
  4K without supersampling).
  Re-measured on a later main with the machine otherwise idle but another program busy on the
  GPU (docs/perf-budget.md "Quiet-machine re-measure"): GPU busy 21.3 ms a frame; frame mean 34.0 ms
  with the measured choice (cheap after 9–10 frames, 6 probes), 38.4 ms with the cheap form forced;
  the frame times there carry the other program's queue and do not compare with the figures above.

## Rate independence: what the gate guarantees

The rate gate (`perf.sh rate`, docs/checks.md "Rate gate") runs ten flows at
30, 60, 120 and 200 Hz and on a jittered display (`jitter:7`: 4–40 ms frames
and a 120 ms stall every 4–6 s) and compares every state-hash column per tick
with 60 Hz, the offline mix byte for byte and the racing bench's α = 0
frames per image class. It now **counts everything** and runs by default
(`RATE=strict` in gate.sh and checks.sh full; `RATE=0` leaves it out;
evidence in the maintainer's private capture):

* **State: 600 of 600 cells identical** — the 15 columns × 10 flows × 4
  schedules, jitter included: gameplay, clocks, HUD, front end, audio,
  particles, visuals and the env-cube schedule are the same at every tick at
  every display rate. Every column is `fail` at every schedule
  (scripts/checks/rate-columns.txt).
* **Audio**: every flow's offline mix (rendered per tick) is byte-identical at
  every schedule.
* **Images, exact** (any difference fails): `orig` (Original), `step`
  (Extreme, another Anti-Aliasing setting, no volumetric fog) and `hist`
  (Extreme, with the volumetric fog's history, the light streaks, the motion
  blur): every α = 0 frame at 30 / 120 / 200 Hz is the 60 Hz frame of its
  tick, pixel for pixel.
* **Images, bounded**: `taa` (Extreme with TAA). The temporal pass
  accumulates drawn frames by nature (its sub-pixel jitter sequence and the
  number of frames averaged depend on the frames drawn; its weights are
  scaled to the frame's span, "Enhancement effects on ticks"), so its frames
  differ between rates; measured on the racing bench (100 α = 0 frames, 10 s):
  mean |Δ| per frame up to 1.49/255 at 120 / 200 Hz and 1.78/255 at 30 Hz,
  the largest single channel difference 247 (edges the history resolves
  differently). It counts only over its bound, mean |Δ| ≤ 2.5/255 per frame
  (`TAA_MEAN`, scripts/perf/rateimg.py). FSR 2's history is the same kind and
  not gated (its own sample counting, "Enhancement effects on ticks").
* **Not covered** by the gate: frames between ticks (no reference in the
  original; their bound is the interpolation tests' — every drawn value lies
  between S_{n−1} and S_n, `tests::snapshots`), the real-time audio onset
  (≤ one frame interval), the window's cheap streak form under load (below),
  the depth of field's noise phase (it counts drawn frames; it runs only in
  scenes, which the image classes do not draw).

**Jittered runs end on the same tick.** The benches ran a number of frames
and postrace's ending scene a span of drawn frames' time; a jittered run's
stalls drop real time past the tick clock's 3-tick cap, so those runs ended
2–4 ticks short and every column's end hash differed (the 60 cells the gate
lacked). The benches now end on a tick count (`NFSU2_RATE_TICKS`, the rate
gate passes RATE_SECS × 60; the frame that would pass the last tick is cut to
end on it, at α = 0: `dev/rate.rs`), postrace's ending scene in a rate
run after 8 s of the scene's 1/30 s ticks (241 ticks; the same tick as
before at every steady rate; the flow's own displays outside a rate run
keep their drawn-time end, so its sim hash is unchanged). ratediff.py no longer excuses a jittered run that ends
short.

**Motion blur on ticks** (`render/enh/blur_ticks.rs`). The clone's motion
blur (Camera / Per-Object) took a frame's motion from the last drawn frame
(the camera and car poses one drawn frame earlier, scaled by the shutter over
that interval) and its noise phase from a drawn-frame counter: at 120 /
200 Hz the α = 0 frames blurred over half / a third of a tick with another
noise phase, so every enhanced frame differed from the first frame after the
warm-up on (the `step` and `hist` classes from tick 539; worst 260 348 px,
max diff 143). Now each tick's end records what the blur reads of the drawn
state at that tick (the view's key, the camera, the cars' transforms by slot,
the blur clock; a ring of four, `Snapshots::blur`, only while the blur is on)
and the number of ticks the blur drew on so far; a frame on a tick takes the
record one tick before its own (no blur where the 60 Hz frame of that tick
drew none: a cut, a new view, the first tick) and that phase, a frame between
ticks the records before its two ticks blended by α. At a steady 60 Hz the
tick before is the last drawn frame, so the window draws as before; the
harness's plain steps record nothing and keep the last-drawn-frame path
(shots, the default benches: byte-identical). Measured: `step` and `hist`
identical at 30 / 120 / 200 Hz (100 of 100 frames each).

## Clone changes

* `crates/ob-core/src/frame_clock.rs`: 0x57EAE0's arithmetic (T = 0, cut at 3/60 s), with
  unit tests against the native tick patterns.
* (superseded by the fixed cadence above) `src/bin/game.rs` `Game::redraw`: dt from the frame clock instead of
  `interval.min(0.1)`. Below 20 fps the world now slows as the original
  does (3/60 s cut, was 0.1 s). A frame under 1/3600 s (no tick) does not
  step the world; its time stays in the carry (clone choice: the original
  only reaches dt = 0 with its limiter off, and it divides by dt in places).
  Shots, `--bench` and the selftest step fixed dt and are unchanged.
* `tests/high_refresh.rs`: drives through the frame clock; the ignored
  "reversing is the same" test is replaced by
  `reversing_from_rest_follows_the_native_cycle` (cycle per rate and both
  outcomes vs the native values). It fails on the old raw-dt stepping at
  120 fps (cycle 0 only).
* `src/drive/`: not changed.

## Open

* The clone's game clock and race timers use the world dt; in the original
  the 4000 Hz clock 0x865188 advances by q (real time). With the fixed
  60 fps cadence both average real time (90 ticks per 3 updates); per
  update they differ by under 1/1800 s. Which timers read 0x865188 vs
  0x865194 is not traced.
* The clone still splits a frame longer than 1/60 s into several outer
  steps (`STEP`, game.rs `step_len`); the original makes one mover call per
  frame (sub-steps only). Matters only for frames of 25 ms or more (the
  native per-frame path); not changed.
* The original's behaviour under a real display rate (vsync / uneven
  frames) was not measured; the rule above takes any raw time, so jitter
  only changes the tick pattern.
* Car-to-car parity of the absolute values (launch 1–2 s) belongs to the driving-parity check.
