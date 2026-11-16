# NIS mover (mode 5): race-start scene cars driven physically

2026-11-04. SPEED2.EXE SHA-256 `8f6a9ae2…a297`. Code:
`src/drive/nis.rs` (`NisMover::set_target`, `CarState::step_nis`),
`src/bin/game.rs` (`scene_movers`). Earlier pieces: `race-start-wheelspin.md`
(wheel/burnout/torque events), `nis-mover-step-investigation.md` (frame
order, component traversal), `nis-target-rust-port.md` (0x5A44B0, bitwise).

## Native chain (verified = traced in the binary)

Per scene frame (the race-start scene runs at 30 frames a second):

1. **0x4309D0** NIS_PlaceSceneCar (verified): slot n's racer; target x / y =
   the channel's pose through the scene origin, z = the car's own (+0x68),
   heading = 0x43E6D0 of the turned forward axis. Slot count 0 (0x827BA4)
   places the car first (0x5FBD70); a car not on mode 5 is switched
   (0x5F46B0(5)); then 0x5A40B0 with the count; the count is incremented.
2. **0x5A40B0** NISMover_SetTarget (verified): mover +0x60 = the count
   *passed* (the slot count before its increment). Wrapped heading error;
   planar distance > 1.1 (0x7A19CC) or |error| > 15 (0x7A19D0) → 0x5FBD70 at
   the pose, target/history/heading reset, count restored. Else, when the
   incoming target differs from +0x70 by more than 0.001 (0x43D190) or the
   heading changed: pending +0x94 = 1, heading +0x90, target +0x70 = incoming
   (count < 4) or 0.25·history + 0.75·incoming (count ≥ 4), history +0x80 =
   incoming, and the steer (chassis +0x130): clamp(error × 25, cur ∓ 65 ×
   g_FrameTime) (0x401090, 0x7A19D4/0x7A19D8); above 3.2 m/s (0x7A19DC) the
   unsigned arccosine (0x4010E0) of forward·velocity within the same bounds;
   then ±45, stored as trunc(trunc(deg × 65536) / 360).
3. **0x5AB3D0** NISMover_Frame, in the world step (verified, `frame.asm`):
   body +0x150 = +0x154 = 1 (rest damping and angular damping on); a ground
   inertia (+0x158) is dropped (+0x88 divided by the ratio); chassis +0x194 =
   9.1 (not ported, unread here); 0x5A29B0 (hydraulic mounts, preloads
   0x598A00 = the ordinary mover's 0x5AB9A0 split); 0x5A44B0 with the count
   > 3 flag; pending cleared; n = trunc(dt / (1/70)) + 1 substeps of
   0x5AA490 (explicit Euler of body + chassis) and 0x5BB0E0; 0x5A2B70.
4. **Substep forces** 0x5A4420 → 0x5AB140 (verified): 0x5AB550 (per wheel:
   load chassis +0x50 along the body's up, at the mount − centre of mass,
   through 0x59BB10's caps; the tyre x/y forces are computed and then zeroed
   at 0x5AB806; slip diagnostics, skid flag above 6 m/s), 0x5A46B0 (authored
   torque, `race-start-wheelspin.md`), aero 0x59E630(0), 0x5A2FE0 (MeloMvr
   hydraulics), 0x5A3420 (fewer than two wheels down only).
5. **Substep posts** (verified): 0x5AB890 → 0x5AB1A0 → 0x5A3710 suspension:
   ground z under each mount (0x598D60 → 0x5E72F0 with its normal),
   compression (ground z − mount z) / (normal · up) + radius (−100 when
   n·up < 0.001), clamped to the travel (contact flag off below the
   extension limit); rate over max(substep, 0.0001); load =
   (max(x,0)·prog + 1)·spring·0.9·x + (bump | rebound)·0.9·rate + preload,
   ≥ 0, **no anti-roll bar**. Then RPM/gear from the spins (0x5AB1A0, not
   ported) and the spin overwrite (0x5AB890: forward speed / radius plus the
   authored burnout; fronts × brake multiplier).
6. **Body derivative** 0x59C1F0 (formats.md "rigid body"): dP = F − 8DP,
   dL = T − g(|ω|)L − 2DL, rest-damping D rate −10 / +0.1, yaw limit; v = P/m,
   ω = I⁻¹L (250 cap); 0x5BB0E0: x normalised, z = x × y normalised, y = z × x.

## Clone

`scene_movers` (each frame before the cut): the scene car's body/wheels
are restored to their pre-step state (the held physics step still runs for
the drivetrain revs, brake lights and AI; native: those cars are on mode 5,
**inferred** equivalent for what the clone draws/plays), then
`NisMover::set_target` and `CarState::step_nis` for the frame dt. At the cut
every car's mover is dropped (count back to 0). The Enter skip's seek still
re-places the cars (`scene_cars`), as NIS_SeekTo does. The mover steps
once per scene frame: a scene that holds the cars runs at 1/30 s frames or
longer (native 0x435F40; the clone's `display::SCENE_FRAME` limiter, which
applies with an uncapped frame rate too), so in game every frame steps with
its own dt. A caller stepping faster (tests at 60/240 Hz) accumulates up to
1/30 s first (`nis::SCENE_FRAME`): a 1/240 s target interval made
0x5A44B0's (target − body)/dt velocity oscillate where the channel stops
(seen in `intro_skip_grid` at 240 Hz before this rule).

## Observed under Wine

internal evidence: nis-mover/native-intro13-quickrace.tsv (`capture.py`): Quick Race,
Circuit, first track, Peugeot 206, forced scene 0x865024 = BinHash
("IntroNis13") variant 0; 493 polls of all four scene cars over the 6 s
scene. Scene car n = slot + 1: racer 1 → Car1 (the AI's scripted burnout:
fronts at 62.83 rad/s = 10 turns/s, rears 0, body rocked by the torque
events), racer 0 (the human) → Car4, which drives in 16.4 m and stops; Car3
drives in 12.7 m (peak 21 m/s on the direct-branch frames 3/4); Car2 stands.
The body trails the incoming target by one frame; the scene runs 30 counts a
second; the movers go back to mode 1 at the cut.

Replay (`drive::nis::replay`, flat ground, Peugeot physics for all four):
the clone's body vs the original's, after the first three extrapolated
frames — worst planar error 2.4 mm (car3), forward speed within 2.8 %
(car3, 2.0 % car4, ≤ 1.1 % for the near-stationary cars), the burnout front
wheels at 62.83 rad/s, the burnout's rocking (up-axis excursion) 0.0094 vs
the original's 0.0083. The test also checks the 1.1 m / 15° gate.

Visual (internal evidence: nis-mover/sbs-intro13-frame120-native-clone.jpg): the
original at scene count 121 (left; its AI cars were drawn at random: a
Focus does Car1's burnout) and the clone's `--shot qrstart` with
`NFSU2_INTRO_NIS=13 NFSU2_FRAMES=120 NFSU2_SHOT_STEP=1` (right; the new
`NFSU2_SHOT_STEP` runs the whole frame step so particles are made): the
same layout, Car1 spinning its fronts in tyre smoke at the line, Car3/Car4
parked from their drive-in. `NFSU2_NIS_PROBE=1` logs every scene car's
mover per frame (slot, count, pose, speed, wheels, controls).

## Race endings (player report, final race vs Caleb)

Design note: "after winning the last caleb race, my cars drift
animation glitched under the map". The final race is S5_CIRCUIT_36; its
ending scene is one of EndingNis01–04 (0x436840 case 1, the start line of
the circuit, origin at the line's ground turned 180°). The clone placed the
ending car at the channel's height above the origin (origin z + channel z,
constant 21.45 m), but that start line rises ~2 m across the scenes:
`NFSU2_PROBE=1 NFSU2_SHOT_EVENT=S5_CIRCUIT_36 NFSU2_ENDING_NIS=n --shot
ending` (old code) put the car origin 1.82 m under the road at the start of
EndingNis01 (road 23.27 m) and 0.4–0.9 m under through EndingNis01/02, and
2 m above the road in EndingNis03/04. Natively every bound scene car goes
through 0x439240 → 0x4309D0 (x / y and heading from the channel, z the
car's own) → the mode-5 mover, which follows the ground; EndingNis01's
winner was observed on mode 5 under Wine (the merged branch's `native-finish-rwd`
capture, `race-start-wheelspin.md`). The ending scene's player car and its
AI-winner companion now run `scene_mover_step` as the race start does; the
lowest wheel centre stays 0.21–0.29 m above the road through all four
scenes (`final_race_ending_keeps_the_car_on_the_road`, ignored: real assets
and isolated XDG). Not captured under Wine for this track (a stage-5 save at
the final race in the original would be needed); the mechanism is the
traced one.

`ending_keeps_ai_winner_in_its_channel_without_mutating_replay` now
checks the native contract: each ending car's centre of mass within 5 cm
and its heading within 1° of its channel (measured: AI winner 2.1 cm at
1/60 s steps, the mover stepping every other frame; player 0.0 cm). Its old
3-D placement check failed by 6.86 m only in height: the test's zero origin
has no road, and the mover keeps the car's own height (0x4309D0).

## Nitrous flames in scenes (for the flame work)

The event tracks carry the scene cars' flames as car effects, not as a
mover state: `CAR_TRIGGER_EFFECT <car> CARFX_NITRO|CARFX_NOS_BLOWOFF
<intensity> <param>` (event type 1) and `SEQ_CAR_NITRO <car> <intensity>`,
which 0x430DC0 expands from the four 0x1C-byte records at 0x7EA240 (read
from the executable; every record's time += start, +0x14 × intensity, car
set): torque (0, ½·i, ½) [ported], effect kind 10 (CARFX_NITRO) param 3
intensity 2·i at +2, the same with intensity 0 at +5, torque 0 at +10.
19 intro tracks and EndingNis02 use them (e.g. IntroNis06: 8 SEQ_CAR_NITRO,
4 CARFX_NITRO set-ups at 0 with intensity 0, 3 NOS_BLOWOFF; IntroNis13: 2
SEQ_CAR_NITRO on Car1). `shop_scene::wheels` now parses them
(`Kind::Effect`, `wheels::effect(events, slot, kind, frame)`), and
`scene_mover_step` stores the slot's CARFX_NITRO intensity in
`NisMover::nitro` (`state.nis`) each mover frame. The type-1 dispatch in
0x4392E0 → the car effect (0x61FB10 kind 10, see `Effects` in game.rs) is
**not traced** here; nothing fires the flame from it yet.

## SCREEN_FLASH (2026-11-05)

The white flashes of the race-start scenes IntroNis05 (frames 36, 71,
101, 131, 161, 200), 10, 22, 24, 25, 26, 27 (`SCREEN_FLASH 1 1.0f` lines).
Code: `src/screen_flash.rs`, `Scene::flashes`, `Drive::screen_flash`,
`rain_frame` (the tint pass), `PostFx` tint draw. Evidence:
`evidence/rating-flash/`.

**Traced (verified).**
* Parser 0x432D00: name 0x785E48 → type 10 (0x430B20, call 0x432D29),
  +0x08 the int, +0x14 the float (0x432E0C). Scene update 0x4395F0: clock
  +0x0C += dt; every event with time × 0.033333335 below the clock is
  dispatched (0x4392E0).
* 0x4392E0 type 10 (0x4394C5) ignores both values: for views 1–2 whose
  +0x08 is set (0x48B1E0), 0x5DBD80(1) on the view's effect controller
  (+0x60; nine 0xC-byte slot states at +0x04 and 0x58-byte records at
  +0x70, built by 0x5E0270 in 0x5C2FA0) → descriptor 0x8769DC (0x5DBDE0:
  one record, slot 0, callback 0x5D7A70, table 0, state 3) → 0x5D7730 →
  0x5D75F0: the record is copied, the slot's step (+0x08) kept below 6
  (else 0), the slot enabled, the callback run.
* 0x5D7A70: for step < 6 the amount (+0x80) = 0x800E54[step] = 1, 1, 1,
  0.7, 0.3, 0.1, colour 0x800E6C / 84 / 9C = 255, +0x7C = 128; from 6
  the amount 0 and the slot off.
* 0x5DBE90 → 0x5D7590(1/30) steps every enabled state-3 slot (+0x08 += 1,
  the callback again) **twice a drawn frame**: at the end of the screen
  passes (0x5CBAEC in 0x5CAAE0) and after the present (0x5D2C31 in
  Render_Present; the `jne` at 0x5D2BDE skips only the lost-device reset).
* Drawn by the tint pass 0x5C7870 (g_TintingEnable 0x870D04 and slot 0 or
  2 on, 0x5CB7CA): slot 0's colour / amount replace the rain's slot 2;
  per channel `c = ⌊((⌊255⌋/128 − 1)·amount + 1)·127.5⌋`, a full-screen
  quad blended DESTCOLOR / SRCCOLOR: dest × 2c/255. So the event's frame
  draws step 0 (× 1.992), the next frame step 2 (× 1.992), the next step 4
  (amount 0.3, × 1.294); then the slot is off: three frames, 0.1 s at 30
  frames a second; a brightening (saturating), not a white overlay.

**Observed under Wine** (IntroNis05 forced, Quick Race, wined3d;
`flash_capture.py`, `native-intro05-poll.tsv`): view 1's slot 0 reads
step 0 → 2 → 4 → 6 (amount 1, 1, 0.3, 0) on four consecutive frame counts
(0x865148, one per present), the controller's update count goes up 2 a
frame (656 in 328 frames), white, +0x7C 128; flashes 35, 35, 30, 30
frames apart as the track's 36, 71, 101, 131. The grabbed frames brighten
by a median × 2.22 and then × 1.31 (unsaturated pixels); the clone's
`--shot qrstart` (`NFSU2_INTRO_NIS=5 NFSU2_FRAMES=36..40`) × 2.28 then
× 1.36 over the same three frames (`sbs-intro05-flash-native-top-clone-bottom.jpg`:
native frames 34–38 above, clone 36–40 below).

**Clone.** `ScreenFlash::advance` fires on the scene update whose clock
passes the event's frame and steps 2 per scene frame (1/30 s of game time,
so any update or display rate gives the same three frames; the scene
limiter draws the race starts at 30 frames a second, as natively), and
keeps stepping after the scene ends. `rain_frame` hands the tint pass the
flash's multiplier over the rain's. The tint pass now draws a multiplier
above 1 as dest × min(m, 1) then dest × (1 + max(m − 1, 0)) (Blend
`AddScaledDest`): a blend constant is clamped to 1 on a normalised target,
so the clone's rain tint could never brighten either (a rain colour above
128 does natively: fixed for both). Reduced Flashing (opt-in,
docs/modern-extras.md) holds the amount to 0.25 (× 1.247).

**Inferred / open.** Which update of the scene fires the event (the clock
starts with the scene's first update; native ±1 frame not resolved by
the poll's start); the Enter skip's seek fires no passed flash in the
clone (0x4395F0 would dispatch skipped events if the seek runs through it:
not traced); Take Picture's flash (0x501250 → 0x5DBD80(1), same slot) is
not ported; the photo mode / replays have no SCREEN_FLASH.

## Open / inferred

* Shop and dyno scenes (ShopEnter1, ShopExit1, DynoEnter, CareerNis1)
  now run the mover too (observed under Wine: the CoM on the
  channel, the car on the floor and up the dyno ramp; within 0.09 m /
  0.22 m of the original's track, `p17-scene-follow-ups.md`). Open: after
  ShopExit1 the original's car stops within two frames, the clone's rolls
  on at its last mover speed; CareerNis1 not captured.
* 0x5A2FE0 hydraulic hops, 0x5A3420 airborne damping, aero 0x59E630(0), the
  top-speed limiter and 0x5AB1A0's RPM/gear (engine audio in scenes) are not
  ported; scene cars stay on four wheels at low speed (observed loads all
  positive), so these do not act in the captured scene.
* 0x598D60's per-wheel ground offset (chassis +0x1F8) is taken as 0.
* The order of force pass and posts inside one substep follows the traced
  traversal (integrate, posts, then forces for the next substep) but the
  first frame's priming pass (rate 0) is **inferred**.
* The native teleport also resets the AI controller (0x42CDC0/0x40D550,
  `nis-mover-step-investigation.md`); the clone keeps the grid route.
* Scene cars do not collide with each other while on the mover (native
  collisions during scenes not traced).
* Tyre smoke uses the skid flag (slip > 6 m/s); its emitter choice/rate is
  the existing **inferred** `particles::tyre_group`.
