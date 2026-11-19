# Random numbers: generators, draw sites, order

The original's generators and the draw sites the clone ports.
Source of the native facts: the audit's port list (all 206 direct call sites of the draw functions
classified, per-site counts measured on the pinned original, c1 / d1 / s2) and the sequence log
below (every draw in order); the addresses were re-read in SPEED2.EXE where this work relied on
them. Marks: **traced** (the code), **measured** (the running original), **open**.

## The sequence log

`tools/parity/rngsites.py patch-seq COPY` patches a private Wine copy's SPEED2.EXE as `patch` does
(a counter per call site) and in addition routes every call of the shared, world, state-pointer,
seed and speech draw functions through a gate that appends (the call's return address, the shared
generator's state before the draw) to a log (4 MB in a tail added to the last section; the gates
in a page appended to that section's file data: code in `.text`'s own file slack made the game
fail its garage load). `rngdraws.py OUT --seq` dumps the log (`seq.pkl`) with each snapshot's index
in it. The runs behind this page (stage-1 save, the original's 60 fps lockstep): free roam entry
with the sound off and on, a circuit (4F7D2AC5) and a drag (C7483590) from free roam with the
sound on, the shared generator pinned at the race dialog.

Clone side: `NFSU2_RNG_LOG` (every draw with its caller) and `NFSU2_RNG_PIN=<place>:<hex>`
(`dev-harness`: the shared generator set at `entry`, `world`, `race` or `race_world`), so a
clone run starts a routine from the original's state and the two sequences are compared site by
site. **Compared** this way (sound off, stage-1 save, `docs/evidence/rng-seq/`): from the race
dialog's pinned state to the start scene's pick, a circuit (4F7D2AC5), a sprint (1FD60BD2) and a
drag (C7483590) match the original draw for draw on both generators: the line-up and its
dressing, the grid, the world build, the render infos, the traffic fill (tries, lane picks,
placements), Traffic_AssignModels and the scene pick (3555, 3555 and 3551 shared draws; 366, 81
and 287 world draws). Free roam's entry, from the encounter set-up to the first wind gust (3586
shared draws, the fill's world draws), matches too. What still makes a session's picks differ
is the state the clone reaches these routines with: the draws listed under "Open".

## Generators

| Original | Clone |
|---|---|
| G1 shared 0x7EA624 (0x43C1C0 / 0x43C1F0) | `Game::rng` only. The career generator `Profile.rng` is gone (career picks, engage messages, sender names, neighbourhood lines draw on `Game::rng`); the weather's own generator is gone (lens set-up at start-up, rain set-up at each world build, gusts and lens drops on the update all on `Game::rng`). |
| G2 world [0x890080] +0xF8 (0x5EA4F0 / 0x5EA510), seeded by `Rand_Int(0x7FFFFFFF)` at each world build (0x5FF557) | One stream per world. Races: `Race::racers.cars.rng` (the race field), lent to the race's traffic around its think and fill. Free roam: the world's car field (`Traffic::field` while traffic is loaded, else `Encounters::field`): the traffic's placer and lanes, the encounter racers' spawn searches and thinks draw from it. The traffic's own `Traffic::rng`, the encounters' own spawn generator and the extra seeds (free roam 2, race 4) are gone. |
| G3 particles (0x7F3940 + emitters) | `Effects::rng`, render side, unchanged (open: A-game-1). Never read by the simulation. |
| G4 speech library (0x819AB4) | seeded from `Game::rng.below(0xFFFFFFFF)` at the audio start (0x4866F0) and at an in-game audio load that selects the mix map (MixMap_Select 0x486A2C, `Game::audio_load_early`), only with sound (`Game::speech_seed`). **Measured**: at the first free-roam entry and at a drag race's load from free roam, after the line-up and before the world seed; none at a circuit race's load from free roam. Taken as the mix map's kind (drag 2, drift 1, else 0) changing (**inferred**: what makes the load skip the step is not traced). |
| G5 AEMS | `audio::aems::Rng`, unchanged (open: A-audio-1). |
| G1 in the audio | the tick's sounds (`Game::audio_sounds`, only with sound) draw on `Game::rng`: `audio::collision::enter_game_rng` / `leave_game_rng` hand the game's state to the audio's generator for the window and back. Sites on it: turbo 0x45EED1 (`turbo.rs`), stitch layer start 0x45B280 (`stitch.rs`: road thumps, drive-by whooshes, bottom-out knocks, traffic contexts), stereo 0x4765A7/BD/D8 (`stereo.rs`; the racers' roll 0x4765F5 is the world build's, below), horns 0x47E78F/CE/EE (`others.rs`), bottom-out (`bottom_out.rs`), EA Trax's Random pick 0x46B86F / 0x46BA2F (`Playlist::next`; `Streams.rng` is gone). |
| G1, in-game audio set-up | with sound, at the world build (`nfsu2::rng_sites::audio_setup`, values unused): the accel transitions 0x45CADE (3 × `Rand_Int(2)`) and in a race the traffic sound's 0x464217 (`Rand_Int(14)`), with the speech re-seed when it is made, else after the rain set-up; CARSFX_Stereo's 9 × `Rand_Int(2)` (0x4765F5) after the fill, which the racers' stereo objects take (no draw of their own in the tick any more); SfxCtlEngine's 4 × {`Rand_Int(100)` 0x46F5AB, `Rand_Float(100)` 0x46F5BC} after Traffic_AssignModels (**measured**, the sequence log). The cover rewards' car-part stitch layers (`career_glue/cover_rewards.rs`) draw on `Game::rng` too. |
| — | **Left (open)**: native G1 audio draws the clone still does not make: physics bind 0x45D885 (`Rand_Float(700)`, 5 in 600 racing frames), hybrid 0x47099E, rain 0x47488A, knock 0x47EB9C, AI contexts 0x476C9B/AB/BB, the traffic sound's other branches 0x464097 / 0x464207 and its adds during play (none seen in these runs). The drivetrain's own sequence (`handling/drivetrain/gearbox.rs`), only where no world step lends the world generator. A traffic lane's walk generator (`lane_stream`): the original's route extension draws its junction odds (0x413C8E) and the planner's lane check its side (0x428B0B, 0x5EA510(1.0)) on the world generator; the clone's lanes draw both on a stream of their own, derived without a draw. |

## Sites ported

* **Start-up** (`Game::boot_draws`; traced order 0x57ED5C seed → 0x57ED7C → 0x48CD60 → Views_Init
  0x5C2FA0 → 0x61E160 × 2 (lens 0x6140A0: 2 × 20 × {F(1.5), F(0.1), F(1.0)}, view 1's kept) →
  0x57ED90 → 0x579560's tail `Rand_Int(0x80000000)` (0x5795BB) → the traffic list's first build
  0x4FE9F0 → 0x41AAF0 (once a process, flag +0x90C) → 0x412840 with no world: 15 × {paint
  `Rand_Int(270)` 0x63720F, `Rand_Int(0xFFFF)` 0x41289C}. 151 draws (**measured** 151 sound off;
  the sequence log gives this order). Then, with the sound, the speech library's start 0x4866F0
  and EA Trax's first pick 0x46BA2F, and the main menu's car render info 0x6383A5 (`Rand_Int(8)`,
  `Game::menu_car_draw`; at the end of the first audio tick with sound, else right after the
  start-up's draws). The window path reseeds from the clock first, then makes them again.
* **World build** (`Game::world_build`; every free-roam entry, race load, restart, test run),
  **measured** in order by the sequence log: world seed 0x5FF557; 0x4179F0 → 0x412840 with the
  world (15 × {paint, shared 0xFFFF, world 0xFFFF 0x4128B7}); rain set-up 0x5FF6A5 → 0x61E100 →
  0x613060 on view 1 then view 2 (2 × 1630). Then (`Game::render_infos`, `traffic_fill_plan`,
  `Game::traffic_models_draws`): the render infos of the player and the other racers (0x6383A5,
  one each: 4 with three rivals or encounter racers); the set-up's traffic fill (0x42F7A0 →
  0x42E2C0, 0x42E4A0 in a drag; world generator) with one render info for each car it places; in
  free roam the career's slot-event picks (0x527D01); with sound CARSFX_Stereo's rolls;
  Traffic_AssignModels (0x412C9A); with sound SfxCtlEngine's draws; the placed traffic cars'
  render infos again (as many as the fill placed); in a race NIS_PickScene case 0 (0x436A26).
  The render info's value (+0x3F8) has no reader (**verified**, the disassembly), so only the
  draw is made. With sound, the in-game audio load's draws (speech re-seed, 0x464217 in a race,
  0x45CADE × 3) come right before the world seed when the speech library is re-seeded, else right
  after the rain set-up. Before the world build: free roam's encounter set-up (0x540790 →
  0x53F010(3)); a race's line-up and dressing (0x526980) and grid (0x4FEE1C).
  The fill now runs in the load, as the original's does (it ran on the race's first frame), and
  free roam's entry has it too (the clone placed free roam's cars only through the placer before):
  0x42E2C0 with no race road built tests a point in 2D against the player and the cars placed so
  far, with no +0x70 flips (`fill_race`'s `road`); the drag fill makes no human-lane draws at the
  set-up (none is listed yet: no 0x410CF7 / 0x410D1C in the log). A placement draws no
  `Rand_Int(0x7FFFFFFF)` for the new lane (the clone drew one; the log shows none), nor does an
  encounter racer's pick (the clone drew a seed of its own per racer).
* **Line-up and looks** (`Game::lineup_dressed`): a race's set-up (0x5260C0) picks and dresses each
  entrant in turn, its 26-car pick (0x4FEC15) then its parts and vinyls (0x6374C0, 0x632DF0,
  0x632ED0), before the next entrant's pick (**measured**; the clone picked all, then dressed).
  Free roam's encounter set-up picks all three and then dresses them (0x53F010 → 0x532020), as
  the clone does; Quick Race is taken as the same (0x53EEA0 → 0x532020, **inferred**).
* **Open events at a load**: the original makes no slot-event pick at a profile load or at Resume
  (**measured**: none between the start-up and the city start); the open list (career +0x71C8,
  event-record pointers) is not saved, and the city start's rebuild (0x527D40) makes the first
  picks. The clone no longer fills them at the load or at Resume.
* **Camera director** (0x5FB060 → 0x5FB2EA `Rand_Float(1.0)`; `Game::director_draw`, once per
  world update, before the frame's rain) while the player's NIS id (player +0x234: set by 0x437910
  at a scene's start, cleared by the scene's end 0x439070) is set. **Measured** on the pinned c1
  with `[0x8900AC] +0x234` read every frame (`rngdraws.py`, internal evidence):
  set from the scene's first frame through the countdown and 325 frames past the green, one draw
  on each of those frames (678), none after. The clone: while its start scene runs, then on the race
  clock up to the scene's end (`Race::scene_end`, the clone drops the scene run at the green mark);
  its first race draws 677 (340 before the green, 337 after). Shop drive-in likewise (s2: 140).
  Drive-out, career start and ending scenes: not measured (open).
* **Engage message** (0x512620), challenge message (0x515160), sender name (0x4FE970) on the shared
  generator; **shop drive-in pick** 0x4370E3 `Rand_Int(1)` (measured one draw).
* **Outrun: the other encounter cars' goal 5**: while the manager races (state
  4, race type 7) every encounter car out besides the challenged one thinks as an outrun racer
  (goal 5 on the leader, `outrun_racer_think`), no longer as goal 9: its world-generator draws (the
  speed refresh of 0x41C5E0's race-state-7 branch, the route's junction decisions on the world
  generator, the race actions' odds) move from `goal9_think`'s sites to `encounter_racer_think`'s,
  in the same car order (the manager's slots), after the challenged car's when it has the lower
  slot. No draw site is added in the generator's other users.
* **Traffic placer pass** (**verified**, the decompile of 0x42F910 / 0x42F800 / 0x42E9F0 and the
  asm): when the placer's timer runs out, 0x42F910 first draws the spacing for each human racer
  (0x418240 → 0x4182A5 `Rand_Float(max − min)`; none outside every traffic zone or while 0x4112A0
  finds a jam) — on every pass, whether or not a car is then tried (the clone drew it per tried
  car); its 0x5F0E40(−1) entry is taken as none (R20's 4 floats + 1 int a placing pass leave no
  room for it, **inferred**). Then per tried car 0x42F800: 0x42F812 `Rand_Int(humans)` (which
  racer), 0x42F852 `Rand_Float(1.0)` (the teleporter odds) — both **added** (the clone drew
  neither), `nfsu2::ai::traffic::placer_try_draws` — 0x42E790 draws nothing (no teleporters in
  the retail routes), then 0x42E9F0: 0x42EA23 `Rand_Float(1.0)` (against), 0x42EAE7
  `Rand_Float(1.0)` under 24.5865 m/s (ahead / behind), the walk 0x415130 (0x4153D2, per branch
  outside coordinate races), and per try 0x410CA0's lane picks, then the new lane's
  `Rand_Int(0x7FFFFFFF)` — unchanged. Sites: `sim/traffic/think.rs` `traffic_place_pass` (spacing
  moved to the pass start; 0x42F812 / 0x42F852 added), `ai/traffic/place.rs`.
* **Honk / light flash 0x40D5C0** (**verified**, the decompile; `nfsu2::rng_sites::honk`):
  right before the constructor of the action proposed at the human's car (0x614890; not traffic
  0x6148D0; 0x60BED0 taken as false for the player's stock car, **inferred**), by the own car's
  kind (car +0x24): traffic (4) always, an AI racer (3) when 0x403820 keeps the human's entry (not
  wholly behind it: gap > −(both +0x0C) − 0.5 m), never otherwise. World generator, in order:
  F(0.4), F(0.2), F(0.4), F(1.0), F(1.0) (horn under p: 1 when the human entry's +0x04 is over
  50 m/s or, for a traffic controller, under −3 m/s; else 0.75), then F(1.0) only after a horn —
  5 or 6 draws. Sites: traffic "Avoid Cars" proposal 0x426CFF / 0x426E4F (`TrafficLane::think`,
  before the action; the two are the same code in the choosers of traffic goal 7, vtable 0x7848A0
  slot +0x38 = 0x426C20, and goal 8, vtable 0x784900 slot +0x38 = 0x426D70: the same calls in the
  same order, the same type / priority tests (0x11 ≤ 600 → 0x4149E0, else 2 ≤ 300 → 0x4145B0), and
  the same −1.0 arguments (0x783648 / 0x783630 / 0x783634); the placer 0x42C816 gives goal 8 to a
  car whose placement record +0x18 is set, else goal 7, and both share the default 0x426EC0. In
  every chooser the draws come after the proposal's search and before the action is allocated
  (0x440E50) and built, and neither the constructors 0x424B40 / 0x423CC0 / 0x4243B0 nor the searches
  0x4145B0 / 0x4149E0 draw, so the draw needs no other place in the think); IJustGotHit 0x42D1F5 / 0x42D7F5 (`Actions::choose`), 0x42DB59 (`choose_pursuit`);
  AvoidAvoidable 0x42D274 / 0x42D874 (`choose`), 0x42DBD8 (`choose_pursuit`) before the vt+0x9C
  draw (`willing`), 0x42D68C (`drag_avoid`). Nothing reads the stored values (0x5F61A0 returns
  before them: +0x925 is only ever written 0), so the draws are all that is ported.
* **Drivetrain rerolls** 0x594720 (**verified**; **observed** bit for bit, corner_launch20 run 1
  frame 65: from the original's world state 0x67B5B54F the offsets −18.9767 / −9.51665 / −19.1812
  and the state after, 0x4BD7ACF5, `docs/evidence/native-sim/shift-reroll.txt`): the three shift
  offsets +0x78 / +0x7C / +0x80 each Rand_Float(hi − lo) + lo on the world generator (0x5EA510),
  in that order. The call path: 0x5A1410's clutch machine (the drivetrain's vt+0x1C, vtable
  0x7A2510) in state 1 (0x5A1634) unless +0x48 = 0 and 0x594EA0() ≠ 0; the integrator's end
  0x5B04E0 runs every component's vt+0x1C (0x5B0509), then 0x5ABB10, whose rest test (0x5A5E30:
  class 1 / 2) or bump-stop hit re-runs them with h = 0 (0x5A0D90, 0x5A0DB6): a car standing makes
  2 rerolls (6 world draws) a frame at 60 fps (**measured**, gdb watches; the clone the same, the
  evidence file), 1 expected at 120 fps. The clone's world step lends its car field's generator
  (the race field's, free roam's traffic or encounter field's) to the drivetrains around every
  car's motion and contact re-step (`handling::with_world_rng` in `drive::collide::step`); the
  drivetrain's own sequence is left only where no world step runs (a lone free-roam car with no
  traffic or encounters, the dyno, tests). A replay's frames draw as the live ones (the replay
  restores the generator with its snapshots). The replay manager test 0x594731..0x594743
  (**verified**): [0x8650B4] +0xA0 is the snapshot Replay_SerialiseAll (0x57BFF0) walks — set at
  0x57C001, cleared at 0x57C01B / 0x57C07A — and its +0x41 is the restore flag (0x57C1F2 sets it in
  the restore 0x57C1E0, 0x5791EA / 0x579206 around 0x5791A0): a drivetrain set up while a snapshot is
  restored takes offsets 0 and draws nothing. The clone's restore copies the drivetrain's state and
  draws nothing either.
* **Collision hull support** 0x5AFBF1: `nfsu2::rng_sites::hull_support_start` for the collision
  code (not wired: the physics unit's files and the per-query mapping are open).

## Open

* The front-end cars' render infos other than the main menu's (0x63A660: the career garage's car
  at Resume, **measured** one; the shops', the car lots', the turntable's rebuilds 0x4D87C0..): not
  made.
* The career garage's car render info at Resume (one draw, **measured**) is one of the front-end
  cars above; it is the one draw between the start-up and free roam's entry the clone does not
  make.
* Drivetrain rerolls (0x594720): ported in the clutch machine, see "Sites ported". Open: (a) the
  gearbox set-up's reroll (constructor 0x5AF9E0 at 0x5AFA9F, three world draws per car mover built,
  0x5B608F in the mover's construction; the clone's drivetrain starts with its own offsets and draws
  nothing there); (b) the contact refresh 0x593710 runs the clutch machine with h = 0 before and
  after every impulse (0x5B2620 → 0x5B04E0), so a car in state 1 during a contact rerolls there too;
  the clone runs no drivetrain update in a contact (a port of the whole refresh was measured and
  left out of the default build, docs/cornering-parity.md section 6: it moves the car's motion after
  a push as well, and the captures showed no gain); (c) the order of a frame's rerolls among the world
  generator's other users (the cars' motion against the traffic's think).
* `pick_lane`'s frequency (the clone ≈ 6.6 a frame driving vs the original's ≈ 22 a placement).
  The placer's order inside a pass is settled ("Traffic placer pass" above).
* Audio draws (`audio::collision::rand`, trax): on `Game::rng` (the audio's posted events are a
  function of the ticks, rate gate `aud`); the native audio draws the clone lacks (see the table)
  are open.
