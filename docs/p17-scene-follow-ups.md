# Scene cars on the NIS mover, event cameras 1–5, the dyno camera, a won ending's car sounds (2026-11-06)

Follow-ups of internal notes. **Verified** = traced in SPEED2.EXE;
**observed** = seen in the original under Wine (vanilla SPEED2.EXE, no
WidescreenFix, private copy `$OB_PRIVATE/wine/runs/<run>`, fastgpu on :170 with
`--audio-file`, the finished-career save's profile; raw captures and videos in
`<work dir>/p17-work/`). Evidence: `evidence/shop-scene-mover/`,
`evidence/ending-hold-audio/`.

## 1. Shop and dyno scene cars on the NIS mover — fixed

**Verified**: NIS_UpdateCars 0x439240 has no scene-kind test; every
bound slot goes 0x4309D0 → 0x5A40B0 → 0x5AB3D0 (mode 5).
**Observed** (`scenecap.py`, ~35 polls a second of the world's car list
0x890080 +0x1C: mode +0x28, rigid body +0x20.., mover count +0x60, view 1's
camera 0x8760E0 and the player's camera mover):
* **ShopEnter1** (IND_BODYSHOP_1, Garage → Return to Explore → Engage → Enter
  Shop): the car on mode 5 for the scene's 140 counts; it stands at its first
  key for ~40 counts, then drives in at up to 4.5 m/s; the body's centre of
  mass is the channel's point, its height the floor's (CoM z 4.20–4.23); at
  the end mode 2 (the shop's front end). `native-shopenter.tsv`.
* **ShopExit1** (Escape from the shop): mode 5 for 120 counts, out at up to
  3.2 m/s, slowing to 0.77 m/s at count 119; then mode 1 and the body stops
  within two polls (velocity 0, the place unchanged after). `native-shopexit.tsv`.
* **DynoEnter** (Garage → Performance Tuning / Dyno → Yes): mode 5 for 280
  counts, the car on the pit lane's floor (CoM z 43.046), up the ramp
  (43.27, 43.53) onto the rollers (43.628); the scene's end puts it on mode 1
  at the same place (CoM (−1682.216, −1957.092, 43.628), origin +0x50
  (−1682.070, −1957.343, 43.483)): no placement, as 0x437A00 reads.
  `native-dynoenter.tsv`. The camera mover during the scene: vtable 0x788018
  (the track camera); after it 0x788070 (the dyno camera).
* No scene when returning from the Garage to Explore (the car is placed
  outside at once). Part previews inside a shop are the front end's
  turntable car (FE scene), not a NIS slot.

**Clone (fix).** `shop_scene_frame` runs `scene_mover_step` for every scene
kind (it did for endings only): the channel's x / y and heading are the
mover's target, the car keeps its own height and its suspension follows the
ground; a new scene resets the slot (count 0 places the car, 0x43B172);
the scene's end drops the mover; DynoEnter's end keeps the car where the
mover left it (the clone re-placed it on the grid marker, `dyno_grid`,
removed). The Enter skip's seek stands the car at the scene's last key (as
NIS_SeekTo re-places a race start's cars). Before, the car's *origin* sat on
the channel at the channel's height (DynoEnter with a floor drop): its
centre of mass ran 0.29 m ahead of the original's, and DynoEnter ended
0.29 m off the original's place on the rollers.
**Measured** (`cmp.py`, the original's CoM turned into the origin):
DynoEnter mean 0.14 m / max 0.22 m in x / y (was 0.25 / 0.36 at the best
frame alignment), end 3 cm; ShopEnter1 0.05 / 0.09 m; ShopExit1 0.035 /
0.064 m. Test `shop_and_dyno_scenes_drive_the_car_like_the_original`.
~~Open: the origin's height is 8 cm under the original's~~ and ~~the car
keeps rolling after ShopExit1~~: both closed by the scene leftovers (§5 below).

## 2. Event cameras 1–5 and 0x83AA24 — resolved (no trigger)

**Verified**: 0x83AA24 (settings +0x20) is written through the settings
block's base: the profile constructor 0x5344B0 calls 0x5116E0 with
`ecx` = profile +0x24 (0x5344B3), which stores 0x42FF0000 = 127.5 at +0x1C
and +0x20 (0x51170B / 0x51170E); the profile load copies the saved value.
**Observed**: 127.5 / 127.5 at 0x83AA20 / 0x83AA24 before loading the finished-career save,
0.0 / 127.5 after (the finished-career save's Jump Camera is off); every fixture save holds
127.5 (test `settings_20_is_the_constructors_127_5`). So the gate at
0x6023DA passes. **Verified**: nothing in the PC game starts kinds 1–5:
0x6022F0's only direct caller is Car_Crash (kind 6, 0x5A0105); kind 1's
timer (director +0x9C, read at 0x602892) is written only by the
constructor (0 at 0x5F1842; the other +0x9C stores belong to other
objects); the key events 0x7F / 0x80 / 0x81 (0x602BE0 jump table 0x603598
via 0x6035EC: cases 0x602F4F kind 1 → mover 0x0D, 0x603098 kind 2 → 0x0E,
0x60320A kind 3 slow motion) have no row in the input descriptor table
0x7FDD10 (rows carry their event at +8; only 0x7D and 0x7E appear), only
the subscriptions 0x60617F..0x6061A7 (handler 0x605C60). The clone starts
none (unchanged behaviour; comments fixed); 0x4531A0 stays unported as dead
code in the PC game. **Not read live**: the director's +0x9C.

## 3. The dyno camera 0x15 in a 3D dyno run — verified, already native

**Observed** (`campoll.py`, 20 Hz through a whole Dyno Run, Results and
back; `native-dynorun-camera.txt`): the player's camera mover is the dyno
camera (vtable 0x788070, type 0x15 at +0x0C, distance +0x2C / +0x30 /
+0x34 7.2, +0x38 5.0, zoom +0x3C 1.0, pitch / yaw words +0x40 0x70A0 /
+0x50 0xE270, rates +0x48 / +0x58 0) from the Performance Tuning
menu on, through the pull and the results; the eye (−1675.7385, −1959.4895,
46.1347) and its forward (−0.882, 0.294, −0.368) do not move while the body
moves a few cm; +0xC4 8000. The eye looks at the car's origin (+0x50), not
its centre of mass. The clone's `dyno_camera` from the same frame is within
4 cm and 0.3° (test `dyno_camera_matches_the_dyno_run_under_wine`). The
"dyno run is the chart" note was stale: the clone draws the chart over the
3D car on the rollers with this camera.

## 4. A won ending's car sounds behind the results — ported, levels partly open

**Observed** (Quick Race Resort Loop, RX-7, `finish.py` writing the player's
laps and finish tick, then `sndpoll.py`: the player's sound objects'
parameter blocks at 20 Hz, `native-won-endingnis01-params.tsv`; a first run
gave EndingNis03, `native-won-endingnis03-events.tsv`):
EndingNis01 (550 counts) drifts the car for ~7 s, then holds it. The
player's FX_WIND (p1 level, p6 pitch 52 at rest → 803 at the finish's
speed, decaying, 159–260 in the second spin), FX_ROADNOISE (base p2 pitch
4018 → 1500), FX_SKID (p0–p2 1774 / 2507 / 1879, the slip levels p4 / p5 up
to 204 / 432 in the drift) are all updated behind Event Results; at count
~550, the scene's end, FX_SKID's p0–p2 and FX_WIND's p1 / p4 drop to 0 over
~0.1 s. The CAR instance and FX_TRAFFIC stay (traffic levels 0).
**Clone (fix).** `car_sounds` keeps the player's car effects (`car_sfx`)
and road / wind (`env`) running from the scene car while a race's ending
scene plays (`frame < length`) behind the post-race screens; after the
scene they are silenced (**inferred**: what lowers them natively is not
traced). Test `won_ending_keeps_the_player_car_sounds_until_the_scene_ends`.
**Open** (compared with `NFSU2_EVENT_LOG`): the original's wind / road
pitch reads car +0x430, which under the mover follows the spinning wheels
(it decays from the race's speed over 3 s and rises while the car stands
in the burnout); the clone's reads the body's forward speed (192 at the
start, back to 52 sooner). FX_SKID's slip goes to p5 in the original's
first drift and p4 in the second; the clone's to p4, up to 962. The
mover's 0x5AB1A0 / +0x430 export under a scene is not ported.

## Still open (not this work)
* DynoExit's trigger (0x436840 case 1 with mode 7 and race type 8): the
  Dyno Shop has a **Test Run** item, not driven here — a candidate.
* CareerNis1 on the mover was not captured (a new career); the car stands
  at its single key, the same code path.

## Checks (gate against <work dir>, CHECKS=full, extreme)
Checks PASS (1232 tests, selftest, 275 shots, the four flows). Parity:
images, window path, enhanced (extreme / extreme_fsr), audio and the
firstrace / firstdrag / postrace / parity sims identical; two intended
differences, both the career start scene's car now on its mover: the
`intro` shot (the 350Z stands where the original's does: bottom-strip RMSE
against the Wine `intro.png` 0.129, was 0.166;
internal evidence: shop-scene-mover/intro-native-base-new.jpg) and the playthrough
sim hash from step 76 (the playthrough passes).

## 5. 2026-11-06: this work's leftovers

Wine: vanilla SPEED2.EXE, private copy `$OB_PRIVATE/wine/runs/<run>` on :181
with `--audio-file`, the finished-career save's profile (Lancer), one instance; raw captures in
`<work dir>/p18-work/`; poller internal evidence: dyno-exit/carpoll.py
(the player's car 0x8900AC → +4: mode +0x28, the body's gravity +0x160,
centre of mass, velocity, the input's pedals, car +0x3E0..+0x43C).

1. **The 8 cm height — not a bug (measurement).** this work's clone runs used
   `player-settings/the finished-career save-20261104` (a Skyline) while the original ran the
   Wine copy's the finished-career save (a Lancer; = `tests/fixtures/native_saves/the finished-career save`).
   With the Lancer (centre of mass (0.29, 0, 0.145)) the clone's origin is
   on the original's height in all three scenes: mean dz −0.000 (ShopEnter1),
   0.000 (ShopExit1), 0.001 m (DynoEnter); frame 0 of DynoEnter 42.901 vs
   the Skyline's 42.818. No ride-height / suspension change. Test: the this work
   test now also checks the height (mean |dz| < 2 cm).
2. **The stop after ShopExit1 — native, already matched.** **Observed**
   (internal evidence: shop-scene-mover/native-shopexit-stop.tsv): on mode 1 the
   body's |v| goes 0.77 → 0.37 → 0.25 → 0 within three polls (~60 ms);
   then the body's gravity +0x160 = 0, velocity exactly 0 and the place
   frozen — the rest test's class 2 (Car_RestClassify 0x5992C0 via
   0x5A5E30: |v|, |ω| ≤ 0.7 class 1 momenta × 0.95, ≤ 0.25 class 2 zeroed,
   gravity 0; verified earlier, formats.md "Rest test"); the pedals read
   gas 0.003 / brake 0.004 (no hold, no brake). The clone (`rest_class`)
   does the same: the car stands still one frame after the scene, its
   centre of mass 0.4 mm from the original's (−546.6732, −1889.4449,
   4.2084). This page's "keeps rolling" was not measured past the scene's end
   (the `shopenter` shot stopped there; `NFSU2_AFTER_FRAMES=n` now runs
   on). Test: this work's test runs three frames on and checks velocity 0 and
   the place (< 2 cm).
3. **CareerNis1 — observed, matches.** A new career (my copy's NAME): the
   350Z on mode 5 under 0x346D604C, standing at centre of mass (1695.2516,
   −883.6426, 24.1127), forward (0.9817, −0.1903, −0.0018), camera mover
   vtable 0x788018; at the scene's end (count ~405) mode 1, the same
   place, velocity 0, gravity 0, camera 0x787F28
   (internal evidence: dyno-exit/native-careernis1.tsv). The selftest now checks
   the clone's car against it (< 0.1 m, forward within 2.6°), during and
   after the scene.
4. **DynoExit — triggered by the Test Run; ported.** **Observed**: Dyno
   Shop → Test Run (the finished-career save, Circuit) set 0x827E1C = BinHash("DynoExit")
   0xB7DEC9B3 and put the car on mode 5 for 180 frames (ticks 132187 →
   156117, 5.98 s) off the rollers and down the ramp at up to 19 m/s; then
   the test run's track 4705 loaded (race type 0x89E7B0 8 → 1, the dyno
   flag 0x89E7E6 kept); seen twice (internal evidence: dyno-exit/native-dynoexit.tsv).
   Quitting a test run to the garage and forcing its finish played no scene.
   **Verified**: the race flow's mode handlers are the table 0x80214C
   (entry 7 = 0x603EC0); case 1 with +0x44 set (0x60A878 at the race's
   start) and 0x4018F0 false sends a dyno session in race types ≠ 8 back to
   the tuning menu (0x603640(0), 0x601030(10)) but race type 8 — the dyno
   session's own race, which the Test Run ends — to +8 = 5 and
   0x5F41F0(4, 0), the ending flow whose pick 0x436840 case 1 gives
   DynoExit on track 0x1262 (4706). DynoExit's header: type 2 (DynoEnter's
   origin: the dyno's ForwardStartLine), 180 frames, one car.
   **Clone**: `SceneKind::DynoExit`; `exit_dyno_scene` on the Test Run
   when the car is on the dyno track (the scene line NIS_Select_End
   (0x100, 0x8000, 1)), its end starts the test run. Also **observed**: a
   test run's Return to Dyno reloads the session and plays DynoEnter again
   (mode 5 under 0xB3B2B197 for ~9 s) before the Dyno Shop; the clone now
   does (`dyno_enter_scene`). Test
   `test_run_drives_off_the_dyno_through_dyno_exit` (centre of mass against
   the original's: mean 0.11 m, worst 0.61 m at 19 m/s, poll timing; the
   test run after it; Return to Dyno → DynoEnter); shot `testrun_pause`
   runs DynoEnter out. **Open**: the dyno's mix inputs during DynoExit
   (race status not read), Dyno Run's reload after a test run (not
   observed: no scene played there in the clone).
5. **Wind / road speed: car +0x430 is |velocity|.** **Observed**
   (internal evidence: ending-hold-audio/native-p18-endingnis03-speed.tsv, a won
   Quick Race on Freemont, RX-7 → EndingNis03): car +0x430 = |body +0xA0|
   to 4 decimals on the mover (e.g. +0x42C 4.95, +0x430 5.46, |v| 5.46 in
   the scene's slide) and on mode 1 (braking 39.77 → 6.95); **verified**:
   the NIS mover's export 0x5A2B70 stores sqrt(v · v) of the body's
   velocity (+0xA0) and forward · v side by side (its car pointer +0x3F0 /
   +0x3EC). This page's "follows the spinning wheels" was an inference from the
   pitch; the EndingNis01 donut moves the body, so |v| stays up while the
   forward speed falls. **Clone**: `world_sfx_input` gives RoadNoise and
   WindNoise |velocity| (was the forward speed) — also in free roam and
   races, where they differ only when sliding or reversing. Test
   `road_and_wind_read_the_cars_whole_speed`. Gate against this work: `audio race`
   differs from 1.53 s for one second (difference RMS 0.1 on 6367: the
   race start's small sideways velocity); every other parity is identical.
   **Open**: FX_SKID's slip
   levels — in EndingNis03 the original's player FX_SKID p3 reached
   114–125 on one side and p4 / p5 ≤ 64 under the mover (p5 432 only
   while braking on mode 1 before the scene,
   `native-p18-endingnis03-params.tsv`); the clone's NIS tyre slip
   (0x5AB550 port in `nis_post`) → SFXCTL_Wheel is not compared value by
   value (up to 962 in EndingNis01).
6. **What lowers the ending-hold sounds — still open.** **Observed** again
   (EndingNis03): at the scene's 550th frame (~18.3 s) the player's two
   FX_SKID instances are destroyed (not just zeroed) and FX_WIND's p1 / p4
   go to 0, while the car stays on mode 5 at rest (gravity 9.8) and Event
   Results stays up. **Verified**: the scene-end routine 0x43BB10 acts
   only on types 3 (0x43B7A0 + 0x4995A0) and 4 (0x43B7A0, the player's
   driver vt+0x1C(0), 0x532E20); an ending (type 1) gets neither, so the
   cars are not handed back. What tears the skid objects down is not
   traced (candidates: the NIS audio state 0x4791B0 / 0x827C38, the mix
   map's NIS-object inputs). The clone keeps silencing them after the
   scene (**inferred**, unchanged).

