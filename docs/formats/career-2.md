# Career, race types and rewards (2)

Part of [docs/formats.md](../formats.md), the traced behaviour of `SPEED2.EXE`; the tags
(**verified**, **inferred**, **open**) are explained there.

## Race types: drift, drag, street X (modes fork)

* **HUD package** (0x5F1983, verified): HUD_Drag in a drag race (0x89E7D8),
  HUD_Drift with the drift flag 0x89E7D9 or 0x89E7E2, HUD_Short_Track with
  0x89E7E3 (street X), else HUD_SingleRace. Features (0x5F33C0, 0x5F32B0):
  a drift race drops the split times (0x80) and adds the drift readouts
  (0x80000000) and builds the style element (0x20000, 0x4C85D0); a drag
  race adds the heat bar (0x20000000).
* **Trick table** (GLOBALB chunk 0x34A07, body 16-byte aligned, 39 × 0x30;
  verified): +0 name label, +4 index, +6 continuous, +7 race mask (1 other,
  2 drag, 4 drift, 8 point to point, 0x10 street X), +8 minimum time (the
  drift tiers: their points 1000/3000/7000/15000/20000/50000/100000), +0xC
  timeout, +0x10 raw threshold, +0x14 cap, +0x18 multiplier ceiling, +0x1C
  multiplier step, +0x28 points per unit, +0x2C max runs (−1 none). Entry 0
  "DRIFT" (continuous, 0.1 s, 0.3 s, 50, 120 points), 1 "CHAINED DRIFT",
  2 "BONUS ZONE", 3 "WALL IMPACT", 8..14 "Good Drift!" … "Drift King!".
* **Style manager** (0x56EBF0 per racer, human and AI — see "Drift
  event, end to end"; update 0x56FD20; verified):
  one trick per entry whose mask matches the race (0x56EBF0's mask).
  Feeding (0x56EAC0) a continuous trick moves idle → pending; each frame
  (0x56E930) a trick unfed past its timeout banks (running: banked +=
  trunc(points)) or fails (pending); while fed within 2·dt points += rate ×
  points-per-unit × multiplier × dt (capped) and raw += rate × points × dt;
  a pending trick past its minimum time with raw over its threshold runs.
  A banked/lost trick returns to idle the next frame (count++ and multiplier
  step when banked). The total (+0x9C) is Σ trunc(banked). The finish banks
  a running drift (0x56D720 once the racer's laps reach the race's).
* **Drift manager** (0x56C900, update 0x56ED50; verified unless noted):
  rate = difficulty (1 human; AI 1/1.25/1.5 by 0x89E7F4, 0x79DAF0) ×
  off-road factor (0x56CA90: with ≥2 wheels off surface 1, count ×
  min(1, speed / (0.25 top speed)), else 1) × proximity (0x56E0B0: 1 + Σ
  over other cars closeness(1 m→1, 20 m→0) × speed between 10 and 160 km/h)
  × angle points (0x56C950: (speed/8.333)² × |slip| when nothing is
  touched, progress has not gone back, |slip| > 5°, speed over the class's
  minimum: 0x79DA68 40/35/30/25 km/h, or 0x79DA58 50/45/40/30 km/h while a
  drift is not running or the car counter-steers (|yaw| > 0.1 against the
  slip)). Multiplier level 1..5 (0x56CB60): falls a level when slower than
  5 km/h + the level's speed (0/30/40/50/60 km/h) or going backwards, again
  every 1.5 s; rises when fast enough for the next and the running drift's
  points pass 0x79DA90[class][level−1] (class 0: 350/1400/4200/11200, 1:
  300/1200/3600/9600, 2: 250/1000/3000/8000, 3: 200/800/2400/6400). A wall
  hit (0x56FB30: closing faster than 3, kind 3) within 0.5 s with strength
  over 0.2 loses the running drift and resets the level to 1. Tiers
  (0x56CCE0) announce on Comment_DATA for 2 s (0x4AC120, 0x4AB340).
  The hit strength is the contact record's +0x60, the sum of its
  pushes' |J| (0x5A9689; see "Style tricks, end to end"); car +0x118 is
  the angular velocity's z (0x6047A2: car +0x110..+0x118 is ω); car
  +0x430 is |velocity|.
* **Drift event, end to end** (2026-10-29; **verified** in the binary
  unless marked; ported in game.rs / drift.rs, test
  `the_board_places_by_points_not_by_finish`, selftest `selftest_drift`):
  * Set-up. Quick Race (0x53F840, drift mode record): 0x89E7D9 = 1, race
    type 0x89E7B0 = 1; when the track record's +0x80 is 1 (the six "SH
    Drift" downhill tracks 4174–4179 = 0x104E–0x1053) laps 0x89E7BC = 1
    and AI racers 0x89E7C4 = 0; else the record's laps, and, offline
    (0x89E7E4 = 0) with 0x802228 set (it is 1 in the file and never
    written) and one human (record +0x0C = 1), the record's opponents
    (+0x0D); 0x53EEA0 then builds them. Career: the 43xx drift tracks'
    events list 3 opponents (2–3 laps), the downhill ones 0 (1 lap)
    (GlobalB 0x34A11: events 13, 40, 62 … on 43xx; 11, 12, 93, 95 … on 4174–4179).
    No team-drift mode was found offline (no string or branch looked
    for one beyond 0x890104 / 0x890114, the online twins: **not traced**).
  * **Grid** (verified 2026-10-30; ported `opponents::grid_slots`,
    `GridOrder`): the car placement 0x6065C0 asks 0x5EB5E0 for six
    starting positions (marker +0x0C = 0..5, else generated) and puts the
    racer whose info +0x03 is k (1-based) on position k − 1 (0x606490).
    +0x03 is numbered by 0x5264B0(order) after the racers are built: order
    1 the humans (info +0x04 = 1) 1.., then the AI (+0x04 = 2); order 2
    the AI first, the humans behind; order 3 as 1, then 0x4FEDE0 (one
    human, AI present) swaps the slots of racer 0 and racer
    0x43C1C0(AI + humans) (two humans: its own pairing, not ported);
    0x89E7D0[player] = the human's slot. Online / LAN (0x850078 = 1 / 4)
    branches and the debug slots 0x864FE0 / 0x864FE4 (0 in the file, no
    writer) are not ported. Callers: career 0x5322F0 at 0x53250F by the
    event's +0x0F through table 0x5325CC (circuit 3, sprint 3, street X
    2, special 1, drag 3, drift 2); Quick Race Circuit / Sprint 0x53F160
    3 (2 with 0x89E7E3, which they leave clear), Street X 0x53F380 2, Free
    Run 0x53F590 1, Drag 0x53F6A0 3, Drift 0x53F840 2, URL 0x53FA50 2;
    Outrun 0x526840 1; also 0x526B78 and the city starts 0x54042C /
    0x540720 / 0x54097D 1. So in **every drift race the player starts
    behind all the rivals** (slot 4 of 4 with three). The race start
    scene's human slot (0x431F10) is that slot. **Confirmed under Wine**
    (2026-10-30, Quick Race Drift, Stadium Drift 2, 3 laps, 3 opponents,
    Medium, Peugeot 206; memory at the grid): 0x89E814 = 4, list
    0x89E810 → racer 0 bytes 00 00 00 04 / +4 = 1 (the human, index 0,
    slot 4), racers 1–3 numbers 100–102 (+1 = 0x64..0x66), slots 1, 2, 3,
    +4 = 2; 0x89E7D0 = 4; the camera behind the player's car showed the
    rivals ahead.
  * Rivals' driver: 0x4260B0 (AIRacerController_Ctor) gives every
    controller-type-3 racer the DriftDriver 0x425E40 in a drift race (the
    same driver the player's car gets past the line). Its target speed
    comes from the controller's speed planner (slot 14, at most 31.29
    m/s); routes and racing line are the ordinary AI's (0x4032A0 one
    entry ahead in drift). Skill rows as in any race (event +0x08 +
    difficulty).
  * Scoring: 0x6016B0 (from 0x607784 for each racer whose status +0x10 is
    0, i.e. still racing) calls 0x5F7A10 in a drift race; 0x5F7A10 builds
    a style manager (0x56EBF0, racer +0x1D4), a misc style manager
    (0x56CE00, +0x1DC) and a drift manager (0x56C900, +0x1D8) for a local
    human (owner +0x14 → +4 = 1, +0x900 = 0) **or**, with 0x802228 set,
    an AI racer (owner kind 2), then runs 0x56ED50 every frame: the AI
    scores with the same scorer as the human, at rate × 0x79DAF0[0x89E7F4]
    (1 / 1.25 / 1.5; 0x56D7E0 sets +0x38 for kind 2, 1.0 otherwise). The
    finish (0x606CC0 → 0x5F7A10, laps +0x09 = 0x89E7BC) banks a running
    drift and fails a pending one (0x56D720); a done racer is no longer
    updated. 0x5F7A10 also keeps per-lap points at racer +0x13C + 4·lap
    (the banked total less +0x190; not used by the clone).
  * **Closeness bonus** (0x56E0B0, the drift manager's +0x3C; verified
    2026-10-30, ported: `drift::closeness`, `traffic_closeness` in
    game.rs, for the player and every rival): 1.0 (0x784250) unless the
    racer's DRIFT trick ([manager +0] = racer +0x1D4 → [0], state +0x14)
    is running (0); then 1 + Σ over the cars of the world list 0x890080
    from index +0x18 to +0x10 (the traffic; the racers are the first
    +0x18 and are skipped, so rivals never count) whose +0x28 is set
    (**not traced**; the clone takes traffic cars on a lane) of
    closeness × w: closeness = 1 − (clamp(d, 1, 20) − 1) / 19 (0x7FA70C
    = 1, 0x7FA710 = 20), d the 3D distance 0x43CE10 between the cars'
    +0x60; w = (clamp(v, 2.7778, 44.444) − 2.7778) / 41.667 × (1 − 0) + 0
    (0x7FA714 / 0x7FA718, 10 / 160 km/h; 0x7FA71C = 0, 0x7FA720 = 1),
    v = the **drifting** car's own speed (+0x430 of [manager +8]), not
    the other car's. A parked traffic car sits at z = −123456 (0x8025B8)
    and adds 0. Drift races have traffic only when the event's traffic
    level is set, as any race.
  * Standings list 0x864630 (built by 0x56D860 on the first 0x5F7A10 of
    the race, 0x20-byte nodes from 0x864640): +0x08 racer number, +0x0C
    class (0x5B5340), +0x10 score, +0x14 flag, +0x18 name, +0x1C car.
    One node per human (flag 1 for the local one) and, off the downhill
    tracks, one per AI racer (flag 1, score 0). On a downhill track
    (offline) no AI node but, with one player (0x89E7C0 = 1), three
    virtual rivals: names 0x4FE970(4), a car each 0x4FEAF0(…, 100 + i, 0,
    1, the human's car, 0, −1.0), score table 0x7FA5C0 (rows: track
    number, then 12 f32 by class × 3 + difficulty, the class the lowest
    of the humans') × 1, 0.95, 0.9, flag 0. 4174: 40000/60000/80000 …
    20000/30000/40000; 4178 up to 400000 (`drift::virtual_scores`).
  * Each frame (Race_UpdateState 0x6076E0 → 0x56DDE0 at 0x608C7A): every racer's node takes its style
    manager's DRIFT trick banked points ([+0x1D4]→[0]+0x2C; only banked
    drifts count, not the running one), then List_SortPass 0x43E650 with
    0x56CDE0 (true when a's score ≥ b's: highest first, stable), and
    0x4DA570 copies the first four nodes to the HUD (0x4C6EC0, when the
    element has feature 0x800000).
  * Standings_Compare 0x5EEC10 has no drift case (finish order, then
    progress): the race order +0xBD0 is still by progress, but the drift
    HUD and results never read it for the place.
  * HUD: the position element's drift branch (0x4EBB10) prints the place
    as the player's node index + 1 (0x56DDA0), the suffix, and "/count"
    of nodes (0x43BE80); it does not write +0x38 (yet the original shows
    "Time:" counting the race clock in a drift race, see below; the
    clone keeps RaceTime). It
    shows +0x44 with a countdown when race status +0x20 ≥ 0 (a beep each
    other second, 0x4EBAC0(0x862A0519)); see **Drift time-out** below
    for what sets +0x20 (ported). The leader
    board (0x4DA190, one human): 10 s of 0x4AAC50 then 2 s of 0x4AADC0,
    4 rows in drift (not the racer count), row k = node k: LeaderText
    "%d :", LBData "%$d" (0x78F744, grouped) the node's points (0x4AAC50) or its name
    (0x4AADC0), SetPlayerColor when the node's flag is set — so the AI
    rows too (the flag is 1 for them), Init for the virtual rivals.
  * The end: each racer finishes on its own laps (0x608C11 → 0x606CC0);
    the race is over when every racer is done (0x5F85C0) and the last
    hold has run, as in any race — the AI on track finish their laps.
    Place = node rank by points: the stats' place (0x5160F7), the
    race-over message (0x4AB4B0: rank, "last" label 0xEAC720D3 when the
    rank is the node count, else 0x7F66B4[rank]), the event's win test
    (0x5287B0 at 0x528918: event type +0x0F = 5 and one player → won iff the rank of racer 0 is 1),
    and the results (0x4BF140: RANKING_DATA_i, PLAYER_DATA_i name,
    CAR_DATA_i, TIME_DATA_i "%$.0f" points, highlighted when the flag is
    set). The margin (0x513FE0, type 5) is first ÷ second − 1 (× 100)
    over the nodes' scores; rewards and reputation then as any race
    (`career::race_reward`).
  * **Confirmed under Wine** (2026-10-29, Quick Race Drift, Stadium
    Drift 2, 1 lap, 3 opponents, Medium, Peugeot 206; lavapipe, memory
    read through /proc/pid/mem): the options offer Laps and Opponents
    (3); the start scene and grid show three AI cars on track; at the
    race 0x89E7B0 = 1, 0x89E7BC = 1, 0x89E7C0 = 1, 0x89E7C4 = 3,
    0x89E7D9 = 1, 0x89E7F4 = 1, 0x802228 = 1; the list 0x864630 holds 4
    nodes, racer numbers 0, 100, 101, 102 (class 3 each), the flag byte
    1 on all four. The rivals' node scores rise in steps as their drifts
    bank (72, 1,829, 2,731, 4,582 …) while the player stood at 0. The
    HUD showed "1st /4" with every score 0 although the player started
    last on the grid (ties keep list order, the human first), then
    "3rd /4" and "4th /4" as rivals banked; the leader board's rows
    "1 :" … "4 :" with the points "%$d" grouped ("4,582"), then the
    names (Arnel, Stacey, Brian, NAME) for about 2 s; a "Time:" row
    under it counted the race clock (2.40, 7.29, 12.24). **Not
    captured**: the results and rewards (the player's car could not be
    steered round the lap at the capture's frame rate, and the race
    ends only when every racer is done), whether SetPlayerColor rows
    look different (the rows all looked white at this size), and a
    downhill track's virtual rivals.
* **Drift HUD** (0x4C85D0, 0x4AB780, 0x4DA590; verified): BestDrift "%$d"
  best drift; SP_Drift_Title "DRIFT" and SP_Drift_Point "%$d" the drift's
  points (red 0xFFCF3030 when lost, orange 0xFFFF8020 with a bonus factor
  over 1, else 0xFFCAF48A); SP_DRIFT_TOTAL "%$d" banked, Highlight on a bank;
  drift_multiplier_text "X%$d"; NextSpeeed_Trap_Indicator "Gain %dX with:"
  (level + 1) or "MAX", SpeedTrap_Speed_DATA the next level's points or
  "----"; Angle_Indicator_DATA the |slip| (≤ 90, 0 in reverse/backwards)
  with 3rdperson_driftometer turned to angle + 90°; SlowMessage runs
  LosingMult / TooSlowToGain while the level falls / may not rise.
  Results show the score in TIME_DATA ("%$d", 0x4BF290).
  **Trick popups** (verified, not drawn by the clone): when the style
  manager reports tricks (0x56FE1C), 0x4DA6F0 hands the list to the style
  element (HUD +0x4C) when the HUD has feature 0x20000; 0x4C88E0 per
  report (0x2C-byte entries from 0x4C7E20): in a drift race DRIFT (index
  0) goes to the drift readout (+0x40 = 1, +0x38 = 0; Highlight via
  0x50CA00 when its state +0x14 is 4); any other trick updates its queued
  entry (state, +0x28 = state 4, points +0xC, the count +0x10 = round of
  +0x1C unless state 3, timer +0x18 = 0) when its points are not 0, else
  a new entry is queued — at the back with points over 0, at the front
  under 0. 0x4DA710 shows the first entry not yet shown (0x4AB780) and
  drops it after 0.5 s (1 s when it is the only one; 0x78E074 /
  0x78E078), or at once once shown in a drift race. **CHAINED DRIFT /
  BONUS ZONE** (verified): never fed — none of the 16 call sites of 0x56EAC0 names entry 1 or 2. The 15 calls outside the wrapper 0x56ED20 (which makes one more) and the 5 calls to 0x56ED20 name the slots 0 (DRIFT, fed by 0x56ED50), 15–26 and 28–38 (28–32 by the gear loop in 0x56F730 at 0x56F854); each call takes its trick from the style manager (the load bases were checked). xrefs show 0x56EAC0 and 0x56ED20 referenced only by those calls (no jmp, data or immediate reference in the disassembly).
  **Wrong-way countdown** (verified): the drift element's
  WrongWay_CountDown (element +0x70) is drawn by 0x4AB2B0 from the
  element's +0x8C: while over 0 it shows "%$1.1f" (0x790A48) of +0x8C
  less the seconds since +0x90, else it clears +0x8C and hides. +0x8C is
  only ever stored 0 (the constructor 0x4F6C7B, 0x4AB2F0), so the text
  never shows; the drift manager's own countdown (0x56DEF0, +0x2C) has no
  HUD reader. The clone keeps it hidden. **Trick popup queue**: only the
  style element reads it, and that is built in drift races alone
  (0x4C85D0); there the tricks of mask 4 are DRIFT (to the drift
  readout), CHAINED DRIFT / BONUS ZONE (never fed, above), WALL IMPACT,
  entry 4 and the tiers 8–14 (announced on Comment_DATA); every queued
  entry is dropped the frame it shows. The clone feeds DRIFT alone (the
  other feeders, 0x56EAC0's callers, are not ported), so the queue it
  would draw is expected to stay empty — **not drawn**. **DRIFT's entry (partly verified).** Slot i is built by 0x56EBF0 from table record i when the record's mask byte matches the race mask (test at 0x56EC91; the mask is 4 when 0x89E7D9 or 0x890104 is nonzero, from the chain at 0x56EC20-0x56EC8B), and i is stored as the node's item[3] (+0xC) at 0x56ECC1, so slot 0 has item[3] = 0. With 0x89E7D9 or 0x890104 nonzero, 0x4C88E0 takes the show-at-once branch (0x4C8931) for item[3] = 0, and it queues only when item[3] is nonzero or when both flags are zero (0x4C8928-0x4C89BF). Not verified: that slot 0 is DRIFT (record 0 was not read; the table base at [0x864624] holds 0x3D613D56 in the file, so the table is filled at run time), and whether a DRIFT node exists with both flags zero. 0x89E7D9 is written at 0x5323D4 (set to 1 when the race byte [esi+0xf] is 5 and [esi+0x34] is not 2, else cleared at 0x5323DD); that value 5 is the drift race is not verified. 0x890104 is 0 in the file and has no absolute-address store in the binary (its roughly 150 references are reads), so its branches are reached only if it is written indirectly. 0x4C85D0 is called from 0x4DA967 when 0x89E7D9 or 0x890104 is nonzero (verified), so the 'built in drift races alone' premise above is not exact. Not traced: the 0-4 / 8-14 drift-mask definition. **Element flag 0x20000 (+8)**: the element is the 0x88-byte object allocated at 0x5F19FA and stored at [racer+0x9a4] at 0x5F1A2C; 0x4F8BF0 stores 0 to its +8 at 0x4F8C69. 0x4DA6F0 has one direct caller, 0x56FE1C, which passes [racer+0x9a4] (0x56FE10); it returns unless [elem+8] & 0x20000 is set, and then calls 0x4C88E0. The +8 store is at 0x4CA65D in 0x4CA090 (ecx is the element, set at 0x6005BE and passed at 0x6005CB): it writes ebx, which is arg2 & 0xFDFBFFFF (reloaded from [esp+0x20] at 0x4CA4FF), where arg2 is the eax return of 0x6000A0 (its esi, moved at 0x60056C). The 0x20000 setters found are 0x5F351E (mov esi,0x20000 when the mode is not 7 and [this+0x234] is 0), 0x5F354F (or edi,0x20000 when 0x89E7D9 or 0x890104 is nonzero) and 0x600563 (or edi,0x20000 under the tests at 0x60053E-0x600563 with [eax+0x234] = 0; 0x600569-0x60056C return edi in edx, which reaches 0x4CA090 as arg3, not the +8 store). How the 0x5F351E and 0x5F354F words reach 0x6000A0's esi is not traced, and other 0x20000 immediates (for example 0x5EA8E7, 0x685AD6, 0x6CEFA1, 0x75DA97) were not examined, so this setter list is not exhaustive. WALL IMPACT (slot 3) is not among the slots the call sites name (see CHAINED DRIFT above).
* **Drag** (verified): the player's gearbox is manual (0x5EB440 → 0x5F6D60
  vt+0x34); a manual shift requests gear ± 1 (0x5AA970 with an explicit
  gear; the clone binds E/Q). The player's car in a drag race is driven by a
  DragDriver (vtable 0x785118, ctor 0x427F20, inside the DragCarController
  0x4299C0, vtable 0x785518): its update 0x4220A0 runs the AI driver
  (0x421030) for the steering and overrides gas/handbrake/brake with the
  player's (+0x67C/+0x680/+0x684). Lane changes (events 0xB8/0xBA left,
  0xB9/0xBB right, 0x4102B0) set the controller's request +0x7B4 (0 left, 1
  right, 2 none); its think 0x42DF10 shifts the path by ∓4 (0x41D720) and
  tracks the lane (0x40F4A0) until the car is within 0x783824 of it.
  **Lanes** (verified unless noted; ported, `src/ai/drag.rs`). The
  DragCarController is controller type 2, which 0x602A68 switches the
  player's car to while its input mode (+0x22C) is 3 (and back to type 1
  otherwise); it is the AI racer controller (0x4260B0) plus the
  DragDriver and the lane state (+0x7B4 request 2, window 0xFF/0xFF).
  AI racers keep type 3 (0x5F6D60), whose driver is the plain CarDriver
  0x425AB0 in a drag race (0x42614A; 0x425E40 in drift races), but
  their goal is "Win drag race" (vtable 0x7847D0, name 0x40CA90): its
  chooser (slot +0x38, 0x42D470) takes GetUnstuck, type 0x12 (vtable
  0x7859D4), "I just hit a wall (in a drag race)!" (0xF, 0x7858D4,
  0x403450) or 0x424560 / 0x4243B0 (both ctors traced: 0x424560 calls the
  base ctor 0x4234A0 with type 0xC, vtable 0x78588C; 0x4243B0 with type 1,
  vtable 0x785858, DAT_00865188 copied to +0x228 (a start tick: **Inferred**,
  its writers 0x57A180 and 0x601C26 are not traced) and a float argument
  (+0x21C; +0x22C = that + 2.0); the chooser's tests for 0xF / 0xC / 0x12
  are not traced, not ported) when they apply, else its default (slot +0x3C, 0x426EC0): AIActionDriveTrafficLane
  (type 7, vtable 0x785820), the traffic lane action. The goal's lane
  class (slot +0x20, 0x40CAA0) is 2 (traffic lanes; "Win road race"
  0x40CA60 gives 1, the racing line), so the action's update 0x4242D0,
  seeing the controller's lane (+0x30C: 0–1 class 1, 2–9 class 2) differ,
  calls 0x420ED0: a lane 2–5 is driven as it is, otherwise the nearest
  (0x40F4A0) becomes the lane with offset 0 and rate 0.151 (0x405410,
  0x7837D8). The path (0x420D40 → 0x41D530) skips 0x415B70's lane
  keeping in a drag race: per FuturePoint its offset moves from the car's
  own off the lane (0x60BFB0) towards the lane record's offset by rate ×
  the point's step (0x40FC80), at the lane's position (0x401C40). The
  action's speed (+0x1F0) is controller slot +0x7C (0x5A9F60: 1000 m/s,
  so the goal's speed rules), +0x1FC cleared, +0x1F8 = 2 (0x783724), and
  for an AI racer 0x40E390 (decompiled) clears +0x1FF and sets it only when
  the car's +0x14 → +4 is 2, 0x59FA40 (0 when the driver's +0x420 is 0 or
  0x7FFFFFFF, or FUN_005925C0's value is at most the first 0x7A1798 entry;
  else 1–3) is 0, and the controller at car +0x2C returns true from its slot +0xA0. That
  slot's target is not traced (0x416F70, which the clone's game.rs comment
  names, is no procedure in the analysis). The route update (0x428720) moves a racer on a lane 2–9 whose
  lane its record lacks (0x40AE90) or, for a car whose controller type is
  not 2, that ends before the list (0x40AA90) to a lane 1–3 away (which
  side first by 0x5EA510 < 0.5), the first 0x412500 allows (the lane
  exists and, for such a car whose own lane is gone, runs to the list's
  end or passes 0x40AF30(…, 25)); with none, driver kinds 1 and 3 take
  the nearest. In a drag race the route manager calls 0x411BE0 (at 0x42817C)
  and then, when the list count is 0, 0x426810 (at 0x42819C). 0x426810 (decompiled)
  takes a road point as its second argument. ECX is loaded with that point at
  0x426828 (not the route manager), the road index is its +0xC & 0x7FF, and
  0x5D89C0 gives its index in its road's 0x38-byte records ((ECX - table
  0x88A334[road] - 0x8C) / 0x38). 0x418A50 is called with the road index, that
  index and two local buffers, +0xAD is set to 0, and 0x422220 follows (bodies of
  0x418A50 and 0x422220 not traced, so that it rebuilds the list from the point is
  **Inferred**). **Inferred**: the per-frame cadence (see below). The player's lane
  think (0x42DF10, then the racer think
  0x42C0C0): with a window set, once the car's lane (0x40F4A0) is
  outside it and |offset| (0x419BC0) ≤ 0.5 (0x783824) the window and
  +0x7C0 clear; request 0 calls 0x41D720(−4), request 1 0x41D720(+4), and
  when the lane moved with no change under way (+0x7C0) the window becomes
  (the car's lane, 5) / (2, the car's lane), +0x7C0 = +0x7C4 = 1, +0x7CC
  = car +0x8FC; request 2 calls 0x40FDE0: the nearest lane outside the
  window (none: clear it and take the nearest), offset 0, the rate below.
  0x41D720(n) steps from the car's lane (0x40F4A0) up to |n| lanes while
  0x412500 allows (a traffic lane its record has), so a held key runs to
  the last lane; it sets (lane, 0, rate) with rate = table 0x81E370 at
  |velocity| × 2.237 mph (0x5767A0; built by 0x77A730 from 0x7E9C68:
  (0, 0.75), (100, 0.25)). The events (0x4102B0) act on a change of the
  held state (+0x790 / +0x791): pressing 0xB8/0xBA sets request 0
  (0xB9/0xBB 1) unless already so and clears the window and +0x7C0;
  letting go sets request 2, +0x774 = 1 and clears +0x7C0. Lane indices
  rise to the right on the drag tracks checked (L4RB: lanes 2, 4, 5 at
  −1.5, 5.4, 8.9 m off the racing line), so −4 is to the left. Clone:
  rivals drive `DragLane::ai_path`, the side coin flip drawn from the
  world's random numbers (verified: 0x428B0B calls 0x5EA510(1) on
  0x890080, i.e. 0x43C170 on 0x890080 +0xF8 — the clone's
  `CarField::rng`); the player's car is steered by an AI driver on
  `DragLane::player_path` (`Input::Drag`: its wheel angle, the player's
  pedals and gearbox), the steering keys standing for the lane events
  (binding **inferred**: the PC keyboard map, DIACTION table 0x7FCAC0,
  has no lane action. **Verified** how keys reach events: the keyboard
  poll 0x5C1850 turns the held DIACTIONs into a pad — action bits cleared
  in the device's 64-bit mask +0x8 / +0xC (active low), sticks in bytes
  +0x10..+0x13 — so the events come from the pad's event map.
  **Verified** event numbers and names: the {id, name} pairs in .data
  (0xB8 at VA 0x7FB118) give 0xB8 JOY_EVENT_DRAG_RACE_CHANGE_LANE_LEFT,
  0xB9 …_RIGHT, 0xBA …_LEFT_ANALOG, 0xBB …_RIGHT_ANALOG (names at 0x79EA38,
  0x79EA10, 0x79E9E4, 0x79E9B4). **Open**: the pad input behind them. The
  definitions (0xB4 … 0x128) come from the table parser 0x659522 (0x659CFF
  is inside it; caller 0x65A821) through 0x6581CC, but no uncompressed game
  file names them and the pad input names are hashes, so the physical binding is not
  traced; the steering keys stand in. **Lane room**
  (0x40FC80(from, lane, rate, to, point, margin, car); decompiled and
  checked in full; ported for the drag lane paths, `RacerRoute::lane_path`
  with `lane_room`): the offset moves from `from` towards `to` by rate × the
  point's vt+0x18 step without passing it, then is held to [lo, hi]: lo
  = point +0x18 (lane 0) or +0x1C.. (+4 × (lane − 2), lanes 2–5), else 0,
  plus margin; hi = +0x2C (lane 0) or +0x30.. , else 0, less margin; in
  no drag race (0x89E7D8 and 0x890108 clear), one of 0x890114 / the drift
  flag 0x89E7D9 / 0x890104 set, and car +0x1C set, crossed bounds (hi < lo)
  meet at their mean; otherwise lo = min(lo, 0) and hi = max(hi, 0), so 0
  is forced into the room (drag races, or no 0x890114 / 0x89E7D9 /
  0x890104 flag set, or a car with +0x1C clear). The
  result is min(hi, max(lo, moved)). The room fields are cleared by the
  FuturePoint set-up (0x40AD20) and refilled by 0x41FD50, called from the route
  manager 0x428030 after 0x418AE0 (0x4281C5, after 0x4281BD; **Inferred**: a
  call every frame (0x42A190 and 0x42A360 are each called only from
  0x42CAA0, complete; 0x428720 from 0x42A050 and 0x42ABD0, partial; none is
  traced to a per-frame update)): each point's
  own lane bounds (FuturePoint vt+0xC / vt+0x10 per lane) and, walking
  back from the list's end, a point's lo at least the next one's less
  1 (or 3) × a per-point value and its hi at most the next's plus it (3
  × above a ±0.12 measure; ±99999 sentinels). **Verified** from the
  disassembly (0x41FE50..0x42001C): the step is (next +0x54 − this +0x54) ×
  99999 (0x784360), three times that for lane 0 on a curve measure ≤ −0.12
  (0x7854E0, the high side) or ≥ 0.12 (0x785368, the low side); lo = max(own
  lo, next lo − step), hi = min(own hi, next hi + step); lane 0's own lo
  over 1 (hi under −1) with a width of 99999 or more becomes 0. With any
  distance between two points the pass never binds, so a point's room is
  its own: from the lane's position plus the point's lane adjustment (+0x04
  lane 0, +0x08 + 4·(lane − 2)) to the road's edges (`lane_room`).
  **Observed** under Wine (a stage-4 drag on 4213 backward, the rivals' walk
  points, controller +0x370 → +0x378 + 4k): record 270 point 6 (adjustments
  0) lanes 2–5 −6.466 / 24.542, −12.449 / 18.559, −17.943 / 13.065, −24.048
  / 6.960, the clone's `lane_room` the same within 0.01 m; point 8, whose
  lane 2 adjustment is −1.181, read 1.18 m off the clone's then (31 % of
  the native rivals' recorded walk-point adjustments were nonzero; the
  racer points now carry them, 0x41A180, docs/drag-stage4-parity.md 8.6,
  and `lane_path` takes the room off them). The
  margin (0x41D530, 0x41D5F9..0x41D60F): 0x420D40's 1 m plus the car's own
  road entry's +0x08 (planner +0x10 + 0x30·own index), its half extent
  across. In a drag race the bounds never pass 0, and once the clamp moves
  the offset against its way while it is not yet within 0.001 of the
  target, the rate is 0 for the rest of the path (0x41D68E..0x41D6DE). The
  racers' JustDrive path (Street X) clamps to the road's edges
  less a margin, passed as param_3 of 0x41D530; the Street X caller and
  its value (1 m) are **inferred**. In 0x41D530 param_3 (arg3) is read as
  [esp+0x34] at 0x41D60B and added to the walk's float (fVar1 + param_3,
  stored back at 0x41D60F); the 0x40FC80 consumer is **inferred**. 0x41D530
  has no 1.0 compare: its float compares are 0.001 (0x78435C, after an fsub
  at 0x41D6AC) and 0.0 (0x7D7D3C, after an fsub at 0x41D6C1 and an fmul),
  and it also loads the dword 0x3F000000 (f32 0.5) from 0x783808 for
  0x4198C0. The param_3 values, verified in the disassembly: 0x420940 pushes
  1.0 (0x3F800000) at 0x420A0F for the call at 0x420A16; the direct call at
  0x42BDD1 passes [0x78375C] = 1.0 (its routine is not confirmed as
  0x42B340); 0x420D40 passes [0x7837EC] = 1.0 when |rate - 999| > 0.001
  (0x7E8E68 = 999.0) and 0.0 otherwise, and 5.0 (0x40A00000) when the drift
  test holds: 0x89E7D8 and 0x890108 clear, one of 0x890114, 0x89E7D9 or
  0x890104 set, and car +0x1C nonzero (0x89E7E3 is not in it). The road-edge
  bound stays **inferred**. In 0x41D530 the rate drops to 0
  once the clamp holds the offset against its motion. **Lane change
  test** (0x412500(from, to, car), verified; ported, `drag::lane_allowed`):
  false unless `to` is 0 or 2–5 and the point's record has it (0x40AE90);
  for a car whose +0x24 ≠ 2 (0x412552; **observed** the drag-finish pass: the rivals' is 3, the player's 1 at
  the green and 2 by frame 600) that lane must run to the list's
  end (0x40AA90) or 0x40AF30(to, from, 25) hold; then, when both lanes are
  traffic lanes, the race is a drag race and the record has `from`, each
  lane stepped through towards `to` must exist, pass 0x40AF30(next, prev,
  −0.1) and the table 0x40B110; else true. These are not tests against
  other cars: 0x40AF30(a, b, d) is "lane a's end along the list (+0xAC +
  4·lane) minus lane b's ≥ d" for two traffic lanes (2–9), else true (decompiled: exactly that, with
  end(x) = *(obj +0x64) + 0xAC + 4·x);
  0x40B110(from, to, coord, 0x89E7A4, track +0x8A, racer) refuses a
  change listed for the track — 0x1073: 16 rows at 0x7E9728, 0x1074: 18
  at 0x7E98A8, 0x1075: 12 at 0x7E9A58, rows {from, to, start, end, flag,
  backward} of 24 bytes (`drag::BLOCKS`; the forward rows at 847–874 /
  849–873 and 988–1012 m, the backward ones at 1270–1295 and 1409–1434 m)
  — while the race coordinate (car +0x8FC) lies in [start − 80 (flag and
  an AI racer, owner kind +0x14 → +4 = 2; **observed** the drag-finish pass: the player's is 1), end] (decompiled: a row matches on from, to
  and backward, and the refusal is inclusive at both ends). 0x41D720 steps
  with 0x412500(lane,
  lane ± 1, car). The race coordinate is the race road's (**observed**:
  the player's +0x8FC on S4_H_DRAG_15's grid 378.05, the clone's
  `Progress::along` 377.9; the clone had taken the list distance, −20.6
  there, so its rows fell ~400 m early) and the 80 m only for the player's
  DragCarController (controller type 2; an AI racer's is 3): ported,
  `RacerRoute::race_coord`, `table_allows(.., player)`
  (docs/drag-stage4-parity.md §6.3). **Inferred**: the AI's run test
  applied only when its own lane is gone.
  The run test itself (0x40AA90, decompiled): a lane runs to the list's end
  when the last point's s (0 for an empty list) is at most its end (+0xAC +
  4·lane for lanes 2–5, +0xB0 for lane 0); lane 1 and the other indices never
  run.
  **Route rebuild** (verified, 0x428030): the branch that calls 0x411BE0
  (at 0x42817C) runs in a drag race (0x89E7D8 or 0x890108) or while the
  route manager's +0xAF is set. The non-race path with +0xAF clear also calls
  0x411BE0, at 0x428141 (when +0xAC is 0 and the lane type differs) and at
  0x428173 (when the lane pick fails), so the list is not simply kept there
  (0x411BE0 empties the list, see below). Apart from the early return at
  0x428241 (third argument 0), every path reaches the same tail: when the list
  count is 0, 0x426810 is called (at 0x42819C), and then 0x418AE0 and 0x41FD50
  run (at 0x4281BD and 0x4281C5). **Verified** (0x411BE0, decompiled): it sets
  the count +0x4 to 0 (the test at 0x428151
  reads it; 0 sends the build to 0x426810 at 0x42818A), sets +0xD0 to 0, frees
  the 40 slots +0x08..+0xA4 (each through its vtable's deleting slot), sets
  +0xAD / +0xAE to 0xFF, +0xC4 / +0xCC to −1, +0xC8 (a byte) to 0, +0xB0..+0xC0
  to 99999 and +0x354 to −99999, and first calls 0x5701E0(0) when +0xA8 is set.
  0x426810 (decompiled) builds from the road point its caller passes as the
  third argument of 0x428030 (the first argument is the car, which goes on to
  0x5D89C0 and 0x422220; the point's +0xC & 0x7FF is the road index for 0x418A50,
  body not traced; it clears +0xAD). The callers pass: 0x428720 the car's road
  point (+0x2C → +0xC), 0x42A190 the action's current point (+0xC), 0x42A360 the
  point 0x41AF90 returned for the car's position (kept between searches; not
  traced). **Callers of 0x428030** (complete, REA): 0x428720, 0x42A190 and
  0x42A360 (each decompiled). **Verified** (dis, REA): 0x42A190 and 0x42A360
  are each called only from 0x42CAA0 (at 0x42CB14 and 0x42CB1B; complete), and
  0x42CAA0 has no direct caller: it is slot 3 of the vtable 0x785230 (entry
  0x78523C; the code stores 0x785230 into [esi] at 0x41850D and 0x4266FF). 0x42A050
  has no direct caller either: it is slot 13 of the vtable (labelled TrafficController in this doc, label not re-checked)
  0x785428 (entry 0x78545C; the code stores 0x785428 into [esi] at 0x41E72D and
  0x4265AB). 0x428720 has four direct call sites: 0x429E6B, 0x42A128 (in
  0x42A050), 0x42ADA4 and 0x42AF4E. Ghidra lists the callers 0x42A050 and
  0x42ABD0 and reports one reference omitted (partial); the omitted one is 0x429E6B, which REA places in no procedure, while 0x42A128 lies in 0x42A050 and 0x42ADA4 and 0x42AF4E lie in 0x42ABD0 (an inference from REA's containment, not a Ghidra report). 0x42ABD0 is slot 3 of vtable 0x7852B0 (entry 0x7852BC) and
  is called only at 0x42C64F (in 0x42C0C0, body 0x42C0C0 to 0x42C65C; the "racer think" label is not verified; slot 3 of vtable
  0x785370, entry 0x78537C). 0x42C0C0 is called only at 0x42E0A2, which REA places in no procedure (its enclosing routine is unresolved; "after 0x42DF10" is address order only)
  (slot 3 of vtable 0x785518, entry 0x785524; REA has no procedure at
  0x42DF10). 0x42C950 is called at 0x42E777, 0x42E9AA and 0x42EDDF, and its
  body (0x42C950 to 0x42CA9F) makes no direct call to 0x42CAA0, 0x42A190 or 0x42A360 (its indirect calls through [edx+0x18] and [edx+0x20] are not traced).
  **Open**: the rate. The slot-3 thinks run through indirect calls (503 call instructions of the form call [reg+0xc] in the binary, a pattern count), and no dispatch loop has been traced to a
  per-frame update, so how often 0x42CAA0, 0x42A050 and 0x42ABD0 run is not
  yet known. **Verified** gate: 0x42CAA0 returns early, without calling 0x42A190 or 0x42A360, when the signed dword at 0x82A3A4 is above 0 (jg at 0x42CABA to the epilogue at 0x42CD50); otherwise both calls run once each, with no branch between them. The file image holds 65535 there, and the disassembly shows no store to 0x82A3A4 by its absolute address, so its runtime value is unknown.
  **Not ported**: the clone keeps its list.
  **Wall-hit / got-hit
  actions** (partly traced, **not ported**): the class-name table lists
  AIActionHitWallDragRace and AIActionIJustGotHitDragRace, not yet mapped
  to vtables. Type 0xF (vtable 0x7858D4): vtable slot 2, 0x4249C0
  (decompiled): the nearest lane (0x40F4A0) at rate 0.151 (0x3E1A9FBE) with
  offset 0, path 0x420D40, speed from controller slot +0x7C, +0x1FC = 0,
  +0x1FD = +0x1FE = 1. Its apply test 0x403450 (verified) returns 300 while
  a wall contact is live (0x5A0170 on each contact record) and −1
  otherwise. Type 1 (vtable 0x785858, ctor 0x4243B0: start tick
  DAT_00865188 at +0x228, window +0x22C = param + 2.0) and type 0xC (vtable 0x78588C,
  ctor 0x424560) are constructors only so far; type 0x12 (vtable 0x7859D4,
  slot 2 0x42DCB0) likewise. **Open**: their update and end logic, and the
  chooser conditions (0x42D470).
  **Verified**: the step of 0x40FC80 is the distance from the previous
  FuturePoint: vt+0x18 (0x40B050, in the three FuturePoint vtables
  0x7846F0 / 0x784A48 / 0x784AA0) returns, for a point with an object at
  +0x68, that object's vt+0x14; else, from its record (+0x5C / +0x5E), the
  record's vt+0x44 length plus +0x58. vt+0x14 (0x40AFF0) is next +0x54
  − own +0x54 when +0x6C is set.
  **Verified** (0x428720, decompiled): 0x40F4A0 is passed the action's current
  point (+0xC, the route's point), not the car's road point (+0x2C → +0xC, the point
  0x428720 passes on as 0x428030's third argument); 0x4249C0's call of 0x40F4A0
  is **open** (not checked).
  **Inferred**: inserted (junction-gap) points placed along their control
  point's direction, not the smoothed curve.
  **Human driver update** (0x41E2C0, DragDriver's base vtable 0x784FC8
  slot 1; verified): driver +0x2C0 is the transmission mode, not a launch
  state: slot 0x34 (0x407280) sets it from the racer's setting (3 → 3
  with the gear request +0x1E4 = the current gear, 1 → 1, 0 → 0, else 2)
  and copies it into transmission +0x60 for a human car (+0x28 = 1). The
  setting is racer info +0x20 (0x5F17F9 from the options' per-player
  Transmission byte 0x83AA79, which the options row 0x4B75D0 and every
  other writer only set to 0 or 1); in a drag race 0x5F6E0A forces 1 unless
  0x864FA8 is set (written at 0x4FEF65 / 0x580182 / 0x58177B, a replay or
  network state, not traced). So single-player play never reaches mode 3
  (clutch pedal: events 0x9F / 0xA0 set the clutch +0x2F4, 0x408180) or
  its gear-change state 4 (0x41E3B4: in mode 3 a gear change to or from
  reverse/neutral switches to 4, transmission mode 0, until 0x5ECD80
  clears). In a drift race (0x401950 on 0x89E7A0: not drag, a drift flag)
  with mode ≠ 3 it sets transmission +0x74 = handbrake (+0x210) > 0.6:
  the clutch held open (see Missed Shift). Ported (Drivetrain
  `clutch_held`).
  **Not found**: staging, a Christmas tree or a false start — the drag
  start is the common countdown.
  **Start, hold and gearbox** (2026-10-29; **verified** in the code and
  **observed** under Wine in a Quick Race drag, Bayview Bridge, stock 206,
  fresh profile, Transmission option at its default automatic):
  * The transmission: 0x5EB440 returns 1 (manual) when 0x89E7D8 is 1 and
    0x864FA8 is 0, else the racer's +0x20; the racer set-ups 0x5F3020 /
    0x5F3090 pass the same test to the input's vt+0x34 (0x407280: mode 3
    → +0x2C0 = 3 and request +0x1E4 = car gear +0x434; mode 1 → +0x2C0 =
    1, request = car gear; else request −1, +0x2C0 = 0 for 0, 2
    otherwise; a human car (+0x28 = 1) copies +0x2C0 into transmission
    +0x60). **Observed**: holding only the gas, the gear stayed 1st for
    the whole run (no automatic up-shift) until the engine blew.
  * The race start's hold (human driver +0x2B4): the driver vtable
    0x784FC8 slot 0x1C (0x410430) stores the flag and, when it is cleared,
    calls the input's vt+0x30. While it is set the human driver update
    (0x41E2C0, at 0x41E37E) writes request +0x1E4 = 1 (neutral) and calls
    0x5EB900(1.0) (full brake) after the pedal update 0x407A30, so the gas
    still reaches the engine. **Observed**: through the countdown the gear
    reads 1 (car +0x434), 0 mph, and with the gas held the rpm climbs to
    the limiter (≈ 6950 for the 206) and stays; shift-up presses during the
    hold leave the gear at 1. The caller that sets / clears the hold (the
    green light) is **not traced** (0x410430 has no direct caller; it is
    reached through the vtable).
  * The input's vt+0x30 (0x407310): with +0x2C0 = 1 or 3 the request
    becomes 2 (first gear), else −1 (automatic); in a drag race (0x89E7D8
    or 0x890108) it sets the launch window +0x2CC = int(0.25 × 4000 + 0.5)
    ticks (0x783B04, 0x784264, 0x784260). Besides the hold's release, the
    pedal update runs it whenever the request is 0 (reverse) and the pedals
    no longer ask for reverse (0x407E4E). **Observed**: at the green light
    the gear goes 1 → 2 (first) by itself and the HUD gear turns from N to
    an orange N for a frame, then 1.
  * Drag gas in neutral (0x407A30, decompiled): in a drag race with the
    car in neutral (+0x434 = 1), as in URL races, the gas moves at most
    +1/s and −1.5/s from last frame's (+0x20C); then, when the pedal
    (+0x2D4) is over 0.6, the rpm (car +0x440, or 0x7FBED8 when that is
    not 100) is under idle + 1000 (H +0x1F0 = record +0x300) and the gas is
    under 0.6, the gas is 0.6. **Observed**: the revs on the line rise
    smoothly from idle to the limiter over about a second of game time.
  * Clone: `CarState::held` (set on the grid, `Game::drive_to_grid`),
    released at the first green frame (`CarState::release_hold`); during
    the countdown the player's car runs (`step_with_props`, the steering
    zeroed in a drag race) so the engine revs; `CarState::drive_off` is
    vt+0x30 (`gear_request`); a manual gearbox now also takes reverse.
    Shots: `game --shot dragline` (the line, gas held), `draghud` (mid-run
    with shifts), `dragblown` (held at the limiter until the engine
    blows); side by side with the original in
    $OB_PRIVATE/wine/compare/drag91/. **Shift light** (fixed
    2026-10-30, **verified**): HUD_Drag's Shift_light (0x25CF2F74, a
    32 × 32 image at (0, −132)) has INIT (white, chained to PulseGreen) and
    three 600 ms colour scripts with no chain and flags 0 (they hold their
    end): PulseGreen 0x2A15F4B9 (base R0 G255 B0, alpha held to 320 ms,
    0 from 340 ms), PulseRed 0xA6797F23 (base grey 223 with key deltas
    B −223, G −223, R −95: R128 G0 B0; same alpha keys), PulseBlue 0x75A0BDB0 (R0 G128 B192).
    0x4C7090 runs the script (0x51CF80) unless 0x51D050 says it is the
    object's current one and not past its length (script +0x30 the name,
    +0x10 time ≤ +0xC length), then shows the light (0x50CA50), so each
    pulse restarts once done — a 600 ms blink. The clone ran a script only
    when the colour changed, so after one pulse the light sat at alpha 0
    and the white of an earlier INIT showed through in later shots; it now
    reruns the pulse as 0x4C7090 does (`Hud::drag_tach`).
    **Fixed**: the grid placement (`Drive::place`, `CarState::placed`)
    used to reset `manual` after `start_race`, so drag races ran on the
    automatic gearbox and the launch window never opened; the racer's
    settings now survive a placement (also the R reset).
  * Heat during the hold: 0x5A11F0 (decompiled) accrues heat whenever
    +0x8C is in [0, 2) and not (transmission +0x3C = 2 with +0x68 > 0),
    with no gear or race-state test; the engine constructor (0x5AA694)
    stores −1. **Observed** (engine object = [car +0x34] +0x48, polled
    under Wine): +0x8C reads −1 for the whole countdown while the engine
    sits at the limiter, and 0 from the first frames after the green light,
    then climbs at the limiter and reaches 2 (blown). The instruction that
    stores the 0 is **not traced** (no absolute store of 0 to +0x8C in the
    engine code was found). Clone: `CarState::release_hold` sets it.
  * **HEAD START** (trick 24 of GLOBALB 0x34A07, mask 0x4B, so also drag;
    **observed** as a popup at the right in a drag race): fed once by
    0x56F730 (at 0x56F7BF, 0x56ED20(0x18, 1.0, 0)) while the race status
    (0x890118) is 4, more than one racer counts in 0x89E7C0..CC, the race
    time (status +0x14 × 0.00025, whole ticks: 4000 is not past 1 s) is in (1.0, 1.5] s, the racer's place
    byte (+0xA) is 1 and its speed (+0x430) is over 2.2351 m/s (5 mph).
    The other drag-mask tricks are AVOIDED ACCIDENT, DRAFTING and Hydros.
    **Not ported**: HEAD START's detector; the clone builds the style
    manager in every race and in free roam (see "Style points and the
    nitrous refill") but feeds only REVERSE and SPRAY SHOW there, and draws
    no trick popups.
  * Lanes: **observed** the arrow keys (the steering keys) move the car one
    lane per press (Right moved it one lane right at 45 mph). Traffic:
    **observed** traffic cars on Bayview Bridge (Traffic Level Minimum)
    that the player has to change lanes around; ported 2026-10-30 (the
    drag placement 0x42E620, see Traffic → Drag placement; the fill before
    the race is open).
  * The rivals on the line (**observed** 2026-10-30 under Wine, Quick
    Race drag, Bayview Bridge, 3 opponents, world list cars 1–3): through
    the countdown (race status 3) each AI racer sits in gear 1 (neutral)
    at 0 mph with its rpm climbing from idle to the limiter (≈ 6950) —
    the AI driver's countdown pedals (gas 1, brake 1, neutral; see AI
    driver → Pedals) running on a live car — while the player's car,
    with no gas, idles at 800; at the green they drop into first (gear 2)
    from ≈ 6400 rpm: 0.84 s 3.6–3.9 m/s, 3.7 s 15.7–16.5 m/s in second
    (gear 3). The clone used to run only the player's car in the
    countdown, so its rivals left the line from idle (800 rpm at 0.3 s);
    it now runs the race step for every racer then (`step_race` with the
    traffic left out), with the rivals' engine heat held off (−1) like the
    player's and released at the green. The traffic waits: at the green
    its cars start from 0.7 m/s.
  * Race over by racer status (0x4C7F00, decompiled; the message is
    0xA7806DDB RaceOverMessage, the HUD's +0x20): 3 "You were knocked
    out!" 0x1B59940C, 8 "Blown engine!" 0x7449D26D, 9 "Totaled!"
    0x4BA0D22F, 11 0x668C7453, 12 0x4A8764A8, other unknown statuses "Bad
    HUD string." 0xA074449F (in game mode 8 / 0x200 status 8 and 9 add
    "Waiting for final race results..."); 1 (finished) in a race of more
    than one racer: 0x4AB4B0's place line ("You were last!" 0xEAC720D3
    when the place equals race status +0x24, else 0x7F66B4[place]: 1
    "Winner!", 2 "You finished second!" … 5 "You finished fifth!"), "Your
    time:" and the time ("%s\n%s %s"); with one racer "%s %s"; drift
    races (0x401910) and 0x42FC40 (**not traced**) a score instead.
    **Observed**: after the blow-up the tach and the position hide,
    "Blown engine!" shows at the centre, and Event Results list the player
    last with 0.00. Ported in `Hud::quads` (`Input::status`).
* **Nitrous** (verified unless noted; ported, `src/nitrous.rs`). The car's
  tank is the block at car +0x40 (+0x414.. = car +0x454..), filled by
  0x591CE0 at the car's set-up (0x59F9DA) and when its size no longer
  matches (0x5F6C8C): size and left = round(tuning +0x2A0 × 4000) ticks
  (record +0x3B0, seconds; tuning slider 16), +0x418 = size / +0x2A4
  shots (record +0x3B4; nothing reads +0x418), shot start +0x420 = 0.
  Stock cars have +0x2A0 = 0; the Nitrous package lerps it to +0x580 and
  sets the push +0x2AC (record +0x3BC; slider 17) from +0x58C (inline in
  0x5AC500). Seconds left (0x592490): left / 4000 − (clock − start) /
  4000 while a shot runs (whole ticks: a 2 s shot at 20 frames a second
  runs 40 frames; the clone's shot time summed 0.05 s steps in f32, fell
  just short of 2 s and ran 41 — now counted in ticks, round 9). Fraction (0x5925C0): left / size, past full
  1 + excess / size (0x7A1794 = 1). Stage (0x59FAB0 when idle, 0x59FA40
  while running, 0 otherwise): 1 + the highest i with fraction >
  0x7A1798[i] = {0, 1.4, 1.7}; 0x59FA40 stops an empty shot (0x592510:
  left = max(0, seconds left) × 4000, +0x418 = 0 with 0x7FBEDC = 1,
  start = 0). The style-point refill (0x592650 from 0x601737 under the
  Race Breaker flag 0x89E7E7, which is 1 in free roam and most races
  offline — corrected 2026-10-30, see "Style points and the nitrous
  refill") fills past full up to a fraction of 2, giving stages 2 and 3.
  Each frame the mover stores the stage in engine +0x84 (0x5ABD75) and
  0x594310 adds curve(rpm) × throttle × power × +0x2AC × 0x7A17A0[stage]
  (1.0 / 1.4 / 1.7) before the heat cut, the curve being engine +0x60:
  0x7FBF38 {0.3, 0.4, 0.45, 0.5, 0.55, 0.6, 0.7, 1.0, 0.7} over
  idle..redline (0x5AA65B); the grip set-up (0x5A5106) multiplies each
  wheel's +0x180 by 0x7A17AC[stage] (1.0 / 1.4 / 1.7). The key:
  JOY_EVENT_NITRO_BOOST 0xA3 / _ALTERNATE 0xA4 (the player's event
  handler 0x602BE0, jump table 0x603598) → 0x6034C6 (it runs before the
  frame's input write 0x5EB924, so its SPRAY SHOW test 0x5ECDC0 reads the
  previous world update's gas and +0x430: **observed**;
  `CarState::nitrous_key`): pressed, a shot
  starts when 0x5EB570 (0x59FAB0) allows (0x59FB00: start = clock,
  +0x418 = left), then SPRAY SHOW (trick 0xF, 0x56ED20) and a rumble
  (0x5727C0(8, pad, 1) with the pad option 0x83AA68); let go, a running
  shot (0x5EB540) stops (0x7FBEDC = 1). Controls row 7 (action 6, Left
  Alt / Right Alt). The gauge (element 0x4F6A90, hi 0x40000: NOS_Line
  +0x20, NOS_GameBreaker +0x24, NOS_Mask +0x28, NOS_Fill; colours +0x2C
  0x96005A7D, +0x30 0x9600B4FA) shows in a race for a car with a tank
  (0x401860 > 0) outside drift (0x401910) and Street X (0x401930)
  races (0x600231). **No nitrous in drift and Street X races**
  (**verified**, 2026-11-04, docs/streetx-nitrous.md): the race set-up's
  drift and Street X tunes (0x5A7760 / 0x5A77D0, from 0x5B4DD0 modes 3 and
  10) clear the tuned block's capacity +0x2A0 (0x5A77BB / 0x5A782B), so
  0x591CE0 fills a 0 s tank and the key (0x6034C6, which itself has no
  race-type test) starts no shot; under Wine a car with the Street
  Package had 8000 ticks in a circuit and 0 in Street X and drift, the
  key doing nothing there. Ported in `race::apply_tune`. In free roam (and the other `+0x28 == 0` cases of
  0x6000A0) 0x5F32B0 sets it (**verified**, 0x5F337C..0x5F33AE) when
  the gauges option (0x83AA64 + 0x28 × player) is on and the tank size
  (car +0x454) is above 0, with no drift / Street X exclusion in that
  second test (the first, 0x5F32FE, is the same test under it). The
  clone: `hud::features`. Its update 0x4C7B20 with one local human: empty →
  NOS_Line and NOS_GameBreaker hidden; in a drag race NOS_Line shown as
  a bar 0x4C66F0(…, f, 1, 45, −9) (NOS_GameBreaker hidden, or past full
  shown with the excess); elsewhere NOS_GameBreaker shown coloured +0x2C
  and NOS_Mask turned to −28 − (152 − f·152)° (past full: NOS_Line too,
  the excess, colour +0x30; 0x8364B0 = −angle). "Past full" is strictly
  f > 1 (0x4C7B20's x87 test; **verified** 2026-10-31, reports): a full
  tank (f = 1.0) keeps NOS_Line hidden and the plain colour +0x2C with
  the mask at −28° — the clone had taken f ≥ 1, so a full tank showed
  the bright 0x9600B4FA ring (side by side at the airport start,
  <work dir>). Ported: tank, key, torque,
  grip, gauge; the tuning sliders 16 / 17 reach the physics through the
  tuned record. **AI nitrous** (verified unless noted; ported for drag
  races, `CarState::ai_nitrous_ok`): a racer's driving action (lane
  0x4242D0, JustDrive 0x42B570, 0x42B9E3, 0x42F5EE) calls 0x40E390 each
  frame, which for a racer (car +0x14 → +4 = 2) with no shot running sets
  driver +0x1FF from controller slot +0xA0 (0x416F70); the AI update
  0x420810 then starts a shot (vt+0xA4, 0x59FB00) whenever none runs, and
  only an empty tank ends it (0x59FA40). 0x416F70 refuses with car +0x28
  = 4 or car +0x1C clear and in a drag race (0x89E7D8 / 0x890108) goes
  straight to 0x40D850: false when no shot runs and the seconds left
  (0x592490) are under the AI tuning row's +0x570 (0x81CB60 + 0x68 ×
  skill: 0.5, 0.5, 1, 1, 1, 1.5, 1.5, 2, 2, 0.5) or car +0x424 ≠ 4; false
  when any live contact record (body +0x18 count, 0x5A0830) is against a
  kind-3 body whose +0x60 → +0x78 is under 0.1 (0x5A0170, decompiled: the
  current record's bodies at +0x238 and +0x23C of the object at
  +0x64 (0x5A0830 selects the record); the clone's `ai_nitrous_ok` now tests
  the same through `upright_contact()` (ported; car contacts do not count);
  false when the mean |slip| (car
  +0x3B4 + 8i) of the wheels 0x592880 counts — rear share +0x1C0 under 0.2
  the front pair, over 0.8 the rear pair, else all — is over 2 m/s
  (0x402660, 0x78353C); then true when the gear (car +0x434) is at least
  0x7E8D90 = 2 (in a drag race the row's byte +0x588 + 1: 6, 6, 6, 6, 6,
  5, 4, 4, 4, 4) and 0x402590 is false, i.e. the rpm (+0x440) is
  **inside** [A, A + 0.95 (B − A)] (**corrected the outrun-pace pass, 2026-11-05**: 0x402590
  clamps the rpm into the band, max with A then min with the top, and
  returns 1 when the rpm differs from the clamp, 0x40261C..0x40264D; this
  entry and the clone had "outside", so the clone's AI shots ran only above
  95 % of the band and ended at every shift). For the physics mover (car
  +0x28 = 1) A is the gear's down-shift rpm (0x5946F0(gear, 0): +0x7C +
  per-gear +0xA8) and B its up-shift rpm (0x594680(gear, 0)); for the
  MeloMvr (car +0x28 = 3, 0x4025C8) A / B are the chassis' (car +0x34 →
  +0x24) down / up rpm +0x1C0 / +0x19C by gear; any other mover passes.
  The controller's think ends a running shot of any AI racer (owner kind
  2) when 0x40D850 fails on a non-full think (0x42AD24..0x42AD55 →
  0x592510, slot +0xA8 0x41E500 stamps +0x7AC) once slot +0xB4 (0x408430)
  allows: always in a drag race (0x89E7D8 / 0x890108), else after the row's
  +0x570 s (**verified** the outrun-pace pass; the clone ended drag rivals' shots only on an
  empty tank). 0x416F70's strategy outside drag races is traced (see
  Difficulty, "AI nitrous outside drag") and ported (2026-11-01):
  the request each frame (0x416F70 with 0x40D850), a shot started whenever
  none runs, ended once it has run +0x570 s and the request fails or when
  the tank empties, the end stamping +0x7AC. On the MeloMvr and the
  BeltMover a shot only drains the tank: the stage's readers (0x59FA40)
  are the physics mover (0x5ABD75, whose one caller is 0x5B72E0) and its
  grip set-up (0x5A5106) alone (verified from every call of 0x59FA40).
  Bench aid `NFSU2_BENCH_RIVAL_NOS=s` (not native) gives the rivals a
  one-shot tank: career event 180 with 6 s, the rivals fired at the green
  and again at 8.3 / 9.0 / 22.8 / 27.6 / 30.3 s. Not checked under Wine.
  **Inferred**: on the MeloMvr 0x40D850's gear is the chassis' and its
  rpm band is skipped; the lane speed is lane 0's FuturePoint speed;
  0x412220 is taken as "no lane-0 speed drops from the car's point to the
  list's end". **Inferred**: the player's DragCarController (also slot +0xA0 =
  0x416F70, also on the lane action) never sets +0x1FF: 0x40E390 asks
  slot +0xA0 only when the car's +0x14 → +4 is 2 (decompiled, with no shot running (0x59FA40 = 0) and
  slot +0xA0 true), and the
  player's car is not that kind (its drag branches test kind 1 at 0x5F6DE1
  and 0x4C70E3; the human's own record not traced). **Not ported**: the rumble, the AI's
  timed refill (0x601795). The style-point refill and SPRAY SHOW are
  ported (`nitrous::Tank::refill`, `style`). **Verified**: the shot
  clock 0x865188 (also the start tick the action ctor 0x4243B0 reads) is
  written only by the frame timer 0x57A0F0 (store 0x57A180: adds an integer
  tick count from its float timers 0x865150 / 0x865154), by the stores at
  0x601BC2 / 0x601C26 (context not traced), and by 0x576D11 (= 0xFA0,
  4000 ticks).
  **Verified**: its only resolved caller is the main loop 0x5811D0 (complete
  callers query), and 0x57EAE0 has no other reference in the image. The loop
  calls it once per iteration at 0x5812FD with 0x43BE20(prev, now) × 0.001
  (0x78435C = 0.001; ms to s); prev is edi (set at 0x5811E2 before the loop and
  at 0x58130F after the 0x43BDF0 read at 0x581305), now is the 0x43BDF0 read at
  0x5812E5. A conditional back-edge at 0x581578 (je 0x5811F4) returns to the
  loop head. 0x57A060 and 0x57A0F0 are called later in the loop body (0x5814F5,
  0x5814FA); the path from the physics call to them was not traced.
  **Verified** (body 0x57EAE0 to 0x57ECB3; all nine calls are direct): it does
  not call the mover 0x5B72E0. Its callees are 0x43BDF0, 0x43BE20, 0x75BED4,
  0x57E3C0 and 0x57D5C0 (complete callees query). The mover has no resolved
  direct caller and no textual reference anywhere in SPEED2.EXE. So this
  function does not step the world itself; whether the world advances
  downstream (through 0x57D5C0, whose callees are 0x573030, 0x57CB80 and
  0x6F5889, or through the object at 0x8650D8) is **open**, and the name
  "physics update" is not confirmed. What it does with dt (verified in the
  body): in the normal path (0x8650CC = 0) it stores dt at ring 0x8647A8 +
  (frame % 100) × 16, with +0 = raw dt (frame = the value read from 0x865148;
  its writers not traced). If 0 < dt < T (T = 0x865558, written at 0x576CCA as
  1.0 / argument and at 0x576CE5 from the dword at 0x7FB710 = 1/120; see
  **Frame limiter** below) it busy-waits on 0x43BE20 until 1000 × (T − dt) ms have elapsed and
  sets dt = T. It sets V = min(max(dt + 0x865144, 0.02), 0.06) when 0x7FB700 =
  0, else V = min(max(dt + 0x865144, T), 0.05) (0x7851C0 = 0.02, 0x7FB714 =
  3.0, 0x7875BC = 1/60). It sets ticks = int(V × 1800 + 0.5) (0x7A0624 = 1800,
  0x784260 = 0.5; 0x75BED4 is floor for non-negative input), carries
  0x865144 = V − int(ticks × 2.2222223 + 0.5) × 0.00025 (0x7A0AB0, 0x784268),
  and stores 0x86513C = ticks and 0x865194 = ticks / 1800 (0x7A0620 = 1/1800).
  The ring slot keeps raw dt (+0), padded dt (+4), 0x7FB70C (+8) and the old
  0x865194 (+0xC). When 0x8650D0 ≠ 0 it also calls 0x57D5C0 with ECX = the
  object at 0x8650D8 and stack arguments (ticks, 8, 3), then (frame & 15, 4, 5)
  (0x57D5C0 contains a ret 0xC at 0x57D67D; its use of ECX is not traced). When 0x8650CC ≠ 0 it takes ticks
  from 0x57E3C0(8, 3) (0x57E3C0(4, 5) is also called and its result dropped;
  0x57E3C0's only callee is 0x57E2B0, not traced), skips the ring, pad, clamp,
  carry and 0x57D5C0 steps, and still writes 0x86513C and 0x865194.
  0x43BE20(a, b) = max(0, b − a) × 2^[0x8284E4] × 48.332287 (0x828028), with a
  signed 32-bit difference; the code treats the result as ms (the spin compares
  it with 1000 × seconds), but the scale from counter ticks is not derived.
  0x43BDF0 passes a pointer to a 64-bit local to the import at 0x78311C, passes
  the value with [0x8284E4] to 0x75C600, and returns the low 32 bits (import
  identity and 0x75C600 not traced).
  **Open**: the readers of 0x865194 (at least 30 instructions in 0x443xxx to
  0x44Dxxx) and of 0x86513C, the use of the object at 0x8650D8, and whether the
  mover 0x5B72E0 takes V rather than dt (no instruction in 0x5B0000 to 0x5BFFFF
  references 0x865194, 0x86513C or 0x8650D8 by address; the mover body was not
  read). The runtime values of 0x8650CC, 0x8650D0, 0x7FB700 and 0x8284E4 are not
  traced (file-image values are not reliable). So the raw-dt rule in the clone's
  Handling section ("The step") is not confirmed by this marker.
* **Frame limiter** (2026-10-31; **verified** in the disassembly and
  Ghidra unless marked): the period T (0x865558) of 0x57EAE0 above.
  * The wait: the loop (0x5811D0) measures dt from the last frame's end of
    wait (prev re-read at 0x581305 after the call) to now; when 0 < dt < T
    (0x57EB2F / 0x57EB51) 0x57EAE0 spins on 0x43BDF0 / 0x43BE20 (no
    Sleep) until 1000 × (T − dt) ms more have passed (0x57EB74..0x57EB8C)
    and takes dt = T. So a frame starts T after the last one's start.
    Then V = clamp(dt + carry 0x865144, T, 3 × 1/60 = 0.05) with 0x7FB700
    ≠ 0 (its file value is 2; the runtime value is **not traced**), else
    clamp(…, 0.02, 0.06); ticks = round(V × 1800) with the carry, as above.
    The simulation's step is the frame's time V (there is no fixed step
    tied to T): the cap makes the frames longer, not the world slower.
  * T = 1/120 (0x7FB710 = 0.008333) from the start: 0x77DCE0 (a static
    initialiser, its one reference the table word 0x7E8528) and
    **0x576CE0** (FrameLimit_Default) copy it. **0x576CC0**(f)
    (FrameLimit_SetFps) sets T = 1 / f (0x784250 = 1.0). Its two callers:
    * **0x435F40** (NIS_SetFrameRate30), from the scene start 0x437910
      (both of its paths, 0x4379A3 / 0x4379EB): when the scene's property
      1 is set (0x4326B0(1): the scene's +0x38 list of {vtable, key +0xC,
      value +0x10} entries, value ≠ 0) every scene car slot (0x827BA8,
      20-byte stride to 0x827C48, index ≥ 0) of the world list
      (0x890080 +0x1C) gets controller 0 (0x5EBA40(0)), mover 5 when it
      has a mover (+0x5CC, 0x5F46B0(5)) and its body paused (+0x3C,
      0x593100), then 0x576CC0(30.0) (0x435FA3: push 0x41F00000): **T =
      1/30**.
    * 0x5FF698 in the world ctor 0x5FF4A0 (the one that sets 0x890080):
      with another human (0x89CF69, 0x8669F4 > 0 or 0x866A14 > 0: online
      / split-screen, see 0x5F0100) the "ExtrapolatedStateSlotPool" is
      made and T = 1 / [0x7A5734] = 1/30 for the whole session; the world
      dtor (0x5FF76A..0x5FF786) frees it and calls 0x576CE0. Not ported
      (the clone has no online / split-screen play).
  * Property 1 ("the scene holds the cars"): set by **0x43ACF0**
    (NIS_BindSceneCars, from the scene set-up 0x43B000 at 0x43B13C) when
    the scene header's +0x20 is non-zero and 0x434DC0(BinHash("ROOT"),
    0x827F60 from 0x77AD90) finds a skeleton named ROOT in the EAGL
    skeleton list 0x827C74 (filled by the 0xE34009 chunk loader 0x434E10, Anim_SkeletonLoad).
    Every scene header has +0x20 = 1 (all 42 `NIS\Scene_*_BundleB.bun`
    read) and the ROOT skeleton is in GLOBAL\GLOBALB.BUN (chunk
    0xE34009 at 0x775100, `__Skeleton:::ROOT`), so **every NIS scene**
    started through 0x43B000 runs at 30: CareerNis1, IntroNis01–28 (race
    starts), EndingNis01–04, ShopEnter1 / ShopExit1, DynoEnter / DynoExit
    (and DemoFlyIn, the Generic* scenes if played). **Inferred**: that
    GLOBALB's skeleton is registered whenever a scene starts (it is the
    always-resident global bundle). A scripted property set 0x438E70 (from
    0x456B0B, in 0x456940) can set or clear properties of a scene it
    starts (there: property 2 = 0); **open** what 0x456940 is.
  * The release, **0x437A00** (NIS_ReleaseCars): with property 1 set, it
    clears it, gives each scene car back (0x5F6D60), calls 0x5EF0A0 on
    0x890118 and tail-jumps to 0x576CE0 (0x437A6C): **T = 1/120**. Called
    from the scene's stop 0x439070 (0x4390FF; from 0x43B7A0 and the other
    stop paths 0x43B34E, 0x43B3EA, 0x43B5FF, 0x43B6F4) and from
    **0x439240** (0x439286), which the per-frame scene update 0x43ABC0
    calls after adding dt to the scene clock +0x2C: with property 2 set
    (0x43B000: scene types 0 and 2, race starts and the dyno scenes) and
    +0x30 < +0x2C (+0x30 = header +0x30 / 30, 0x437966; the start scenes'
    camera cut, IntroNis08's 180 frames; DynoEnter's equals its length).
    So a race start scene runs at 30 until its camera cut (the countdown's
    start) and at 120 after; CareerNis1 (type 4), the shop scenes (3 / 4)
    and the ending scenes (type 1, which play behind the post-race
    screens: those screens run at 30 too, the whole loop being capped)
    until their stop.
  * Not affected: the loading screen (a scene starts only after its load,
    0x43BAA0 → 0x43B8E0 → 0x43B000), movies (not NIS scenes; their loop
    is **not traced**), the front end and free roam / races outside scenes
    (1/120).
  * **Clone** (`display::SCENE_FRAME`, `display::frame_period`,
    `display::wait_until`, `ShopRun::holds_cars`, `Game::scene_frame_cap`):
    the window's frame waits until 1/30 s after the last frame's start
    while a scene holds the cars (types 0 / 2 up to header +0x30, the rest
    while the scene runs; an Enter-skipped scene (agreed deviation) ends
    the cap with it), sleeping to within 1 ms and spinning the rest (the
    original spins all of it), before input and the simulation; the
    scene and the world advance by the measured frame time (the game
    clock), as V does. The 1/120 cap outside scenes is **not ported**
    (agreed deviation, docs/native-deviations.md); `NFSU2_FRAME_CAP` stays an optional extra cap (the
    longer period wins). `--bench scene` measures the pacing (a stage 1
    circuit's start scene on the real clock; reference desktop GPU, 2026-10-31: 177–179
    capped frames, p50 33.334 ms, sd 0.04 ms on a quiet machine, p99 ≈ 39
    ms with other jobs' Wine sessions running; after the cut ≈ 2–3 ms
    frames, uncapped). A frame whose own work runs past 1/30 s is not
    made up for, as in the original (no wait, dt the frame's time).
    **Open**: confirmation under Wine (an attempt, 2026-10-31, to read
    0x865558 through a race start: the private DXVK copy died with an X
    error before the title, wined3d drew off-frame; not retried), and the
    original's pacing under vsync (the spin and the present interval together) was
    not measured under Wine; the clone's present mode is unchanged (Fifo
    with VSync on, which holds 30 fps frames for whole refresh intervals:
    even on 60 / 120 Hz, alternating on rates that are not multiples of
    30, as the original's would be).
* **Drag drop-out** (0x6076E0, verified): in a drag race (0x89E7D8 or
  0x890108) each racer whose racer +0x10 is clear (or with no race on,
  0x89E7B0) is tested each frame: a totaled car (+0x964) ends with status
  9, else a car of drivetrain kind 1 whose engine is blown (0x5ECE10 →
  0x5942D0: engine heat +0x8C exactly 2, see Engine heat) ends with status
  8 "Blown Engine" (0x606CC0(racer, place, 0, 8)). The caller's place is,
  first, the highest racer slot (1-based; the array at 0xBD0, bubble-sorted
  each frame by 0x5EEC90 outside states 0 and 8) whose occupant's place byte
  +0xC is clear, i.e. that slot index (inline search in 0x6076E0, the same
  rule as 0x5EEFC0, which returns it in AL; verified). Only when every
  occupant has a place does it fall back to the lowest place no racer holds
  (the same search as 0x5EEF80: counter EAX from 1, returned; verified).
  0x606CC0 stores a place of 1 or more as the place in the branch taken while
  0x89CF69 = 0, 0x8669F4 < 1 and 0x866A14 < 1 (verified; 0 means compute by
  0x5EEF80). Otherwise it sets the place only while +0xC is clear, by
  0x5EEFC0 for status 8 (statuses 1 and 6 use 0x5EEF80; verified). The clone
  ends the run at the clock and ranks drop-outs after the finishers, the
  earliest last. **Open**: this order is not verified in general. Verified:
  0x5EEC10 tests state 0x89E7B0 = 7 first; in state 7 it puts unplaced racers
  before placed ones (placed by place, unplaced by the float at +0x24), so
  each drop-out takes the count of unplaced racers, which falls with each
  drop-out (earliest last across frames; drop-outs in one frame are numbered
  in array order). Outside state 7 the comparator mixes the float at +0x20
  into the order, so unplaced-first is not guaranteed. Not traced: which
  state the drag runs in (state 7 is written only at 0x6071AE, in 0x607190,
  which 0x6076E0 calls on a flag at +0xC30), and a finisher placed after a
  drop-out (0x606CC0 status 1/6/7 paths).
  **Totaled** (car +0x964, a latch; verified):
  set only by the crash check 0x59FB80 (each push through 0x5A9630, track
  contacts included) in a drag race (0x89E7D8 or 0x890108), for each of
  the pair's bodies without +0x11 whose car (+0x130) has +0x1C set, when
  min(|J|, 40) ≥ 30 (0x7A1874) or else the body's vt+0x14 +0x38 is under
  −0.5 (0x787B5C, compared at 0x59FC56). The body's vt+0x14 (vtable
  0x7A2478, constructor 0x5AEC30, slot 5 = 0x593BC0; verified) returns
  [+0x68] + 0x30 when +0x5C is set, else [+0x130] + 0x90, the same
  +0x130 whose +0x1C the test reads; 0x5937C0 reads that block's +0x20
  row as a unit axis (dotted with a normal, clamped to 1), and 0x59B9C0
  writes +0xA0 and +0xC0..+0xC8 from argument vectors. **Verified**:
  +0x38 is a constant 0, so only the push test totals a car. The block is
  the car state's 4×4 orientation (car +0x90 = state +0x50, copied each
  frame from the rigid body's +0x30 at 0x5A5586; the +0x5C branch returns
  the body's own +0x30), rows of 16 bytes: +0x00 / +0x10 / +0x20 the axes
  (0x5937C0 reads +0x20 as the up axis) and +0x30 a translation row the
  body never fills — its position is +0x20, its save / load (0x59B700)
  writes +0x20 and the three axis rows only and then resets +0x3C / +0x4C
  / +0x5C to 0 and +0x60..+0x6C to (0, 0, 0, 1) (0x59B778), as does its
  identity set-up (0x43C780); no other store reaches body +0x68 or car
  +0xC8. So +0x38 (row 3's z) reads 0, never under −0.5. With car +0x958
  (the car's damage model, see Window texture) set it also calls
  0x63A4C0(1) (verified): the model's +8 flag is set and, on that change,
  0x633F00 re-reads the part damage states (0x610EB0 → +0xE0, 2 bits per
  row of the 15 at 0x803714, 36 bytes each, "Side Mirror Left" …) and
  applies each changed one (0x61FB10 with 0x803914[state]: 0, 0.1, 0.4;
  state 1 also 0x4031C0 on car +0x60), then the window textures
  (0x615290): visual damage, not ported (the clone draws none).
  Ported (CarState `totaled`, the push test alone; drop-out status 9
  before the blown test).
  **Done racers** (0x606CC0, verified): a racer that finishes or drops
  out in a drift or Street X race (0x89E7D9 / 0x890104 / 0x89E7E3 /
  0x89010C), or with status 9 or 8, stops colliding with cars (racer car
  +0x3C → body +0x190 = 0); ported. **Knocked Out** (status 3, 0x6079BF;
  verified, not reachable in the clone): only with 0x89E7B0 = 3, which
  0x53F249 / 0x53F477 (the latter also sets the street X flag) store in
  Quick Race (game mode 2) for race type 0 when the circuit record's lap
  knockout byte 0x83AAD4 is set (UI_QRModeOptions' Lap KO; ported 2026-10-29). The
  frame a racer completes a lap (+0x0D) with place (+0x0A) + laps done
  (+0x09) equal to the racer count, every racer behind it in the order
  (+0xBD0) still running is knocked out in its place. The leader board
  label (0x4AA9F0, jump table 0x4AAC28): 3 "Knocked Out" 0x5D82DBA2, 8
  "Blown Engine" 0x01E66364, 9 "Totaled" 0xB7B75185, 11 "Timeup"
  0x1CE129E0, 12 "Disconnected" 0xA7DB717F, else "Finished"; ported.
  **BLOWNENGINE** (0x484E78.., verified, not ported): a sound event, not
  a camera. The car's audio object (car +0x44 → +0x30, its kind +0x14 = 1)
  with its latch +0x6C clear, *0x8900D8 not 2, 3 or 7 and the engine blown
  (0x5ECE10) asks the audio stream manager (0x82BA30 → +8: idle by
  0x46C9A0 = 0, then 0x46C790 on +0x34 + 4) and queues event 8 named
  "BLOWNENGINE" (0x78C154, hashed by 0x43DB50) on its +0x28 (0x484510,
  callback 0x45D360; next to "Collision Stream QUeued"), then sets the
  latch. The clone wires no streamed audio events (see Audio).
* **Drag tachometer** (ctor 0x4F6500, HUD +0x34, update 0x4C7090 from
  0x4EC5CA; verified unless noted): built instead of 0x4F5100 in a drag
  race (0x4F8CF2), mask 0x10000060: RPM_fill, Drag_Turbo_Backing / _Lines,
  3rdPersonTachRPMNumbers and RPM_Redline (0x5ED9C0 with index −1: in a
  drag race with one local human its names are `DRAG_RPM_` + "10000_LINES"
  / "9000_LINES" / "8000_LINES" / "7000_LINES" by 0x60FE20's case pairs
  0–1 / 2–3 / 4–5 / 6–7, and `DRAG_RPM_` + "REDLINES_<9000 … 5500>" — not
  the `3RDPERSON_…_CUSTOM_nn` set, which is built only otherwise, so the
  needle, fill and turbo keep the package's own images; textures in
  GLOBAL/InGameDrag.bun; with no numbers texture both objects hide, +0xA4;
  **verified**, decompiled, and **observed**: the 206's dial reads 0–8 with
  a red band 6.5–8 — the clone used to put the round-dial
  `3RDPERSON_8500LINES_CUSTOM_00` on the drag arc, drawing huge stray
  numbers, and swept the needle like the round dial; fixed). RPM_Redline
  (flags 0x40000001, invisible until its element shows it) shows with the
  element. The needle angle (0x4AB210 with one local human) runs from the
  car record's +0x7FC to +0x800 (read through car +0x0C, the car type
  record, + 0x7F0 +0x0C / +0x10; +0x7F4 / +0x7F8 with more humans) by rpm /
  max rpm clamped to 0..1 (an end not past the start wraps through 360);
  **observed** in memory: the 206 has −43 / 43 (and 7 / 275), and
  Matrix2D_Rotate 0x51CEE0 takes it as for the round tach. 3rdPersonNeedle, 3rdPersonGear, ShiftMessage, speed and
  units, Drag_Position_Backing_03, Shift_light, RPM_1..12. 0x89E7C0 is
  the local human racer count (0x4AB52E compares 0x89E7C0 + C4 + C8 + CC
  with the racer count; the quick-race setups 0x4FEFF0… store 1, 0x52675B
  the mode's player byte), so in single player it is 1: the needle turns
  (0x4AB210) and the 12 RPM lights are never lit (lit 0xFFCCF48A, from
  the 10th 0xFFC80B0B, unlit alpha 0x64 / 0x7D only with more than one
  local human; not needed by the clone). RPM_Redline runs Blink while the last
  up-shift's rating (transmission +0x68, 0x5ECEB0) is negative, the rpm is
  over the redline (engine +4) and Over Rev! is not up; else Init.
  ShiftMessage on a gear change (+0x434 against the last) with a rating
  in (0, 1]: "Perfect Shift!" 0x0598B065 over 0.5, else "Good Shift!"
  0x27D2DD45; going up from a forward gear (*0x890118 = 4): "Short
  Shift!" 0x2303D76C at 0, "Over Rev!" 0x27D75CD8 under 0; transmission
  +0x5C > 0 (0x5ECE30) gives "Missed Shift!" 0x75941DC1. It runs ZoomIn
  and hides when that ends. Shift_light: hidden
  while shifting (+0x48) or with no rating more than max(500, window)
  under the up-shift rpm (0x594F30 flag 0, 0x594F10, 0x5ECD40; the rpm is the car's shown
  rpm +0x440 and the gear the element's +0x80, the car's +0x434 of its last update — stage-4 drag parity,
  docs/drag-stage4-parity.md: so the light is blue from max(500, w) under the point while a
  shift grades above 0 only from 0.95 w under it, "Short Shift!" under a blue light); else
  PulseRed at −1, PulseBlue up to 0.5 (not in reverse), PulseGreen over.
  The needle (object +0x20 = 3rdPersonNeedle, its own colour kept at
  +0x9C; one local human only): its own colour while the light is off,
  0xFF800000 at −1, 0xFF00FF00 over 0.5, and up to 0.5 in a gear other
  than +0x434 = 0 its colour blended toward (A 255, R 0, G 128, B 192) by
  t = (rpm − (up-shift rpm − w)) / (0.5 w), w = max(500, window), each
  channel clamped to 0..255 (0x4AB0B0; 0x4A9E30 packs four float-to-int
  conversions, through 0x75BED4, into an ARGB word, the first in the top byte).
  **Verified** (0x4C7090, decompiled): the light-off branch sets the own colour
  (+0x9C); in gear 0 (+0x434 = 0) up to 0.5 the game writes no tint, so the last
  one written stays; ported (`Hud::needle_color`).
  **HEAT_Bar** (element 0x4F69E0 at HUD +0x24, update 0x4C7AA0, verified,
  ported): 0x4C66F0(bar, h, 1, 105, 5) with one local human (25, 25
  otherwise), h = engine heat +0x8C clamped to 0..1 for drivetrain kind 1,
  else 0 (0x5ECCE0). With H the image's texture height (texture +0x46),
  f = 0x4A9D60(0, 1, (H − 105)/H, 5/H, h) (linear, clamped at the ends)
  becomes the top texture coordinate (object +0x48) and the height
  (1 − f)·H, the position moving by half the change so the bottom edge
  stays (0x517DB0, 0x517C10).
  **Missed Shift** (verified; unreachable in single player, so not
  ported): 0x5AA970 with an explicit gear,
  when transmission +0x60 is 3 and the gear differs from the current
  (+0x3C), accepts it only in clutch state 1 (open; +0x44) with no change
  pending (+0x48 → 1, +0x38 the gear); otherwise it clears +0x48 and stamps
  +0x5C = the game tick 0x865188 / 4000 s. While +0x5C is set the tidy-up
  (0x5A1639) holds engine power (+0x80) at 0 and clears +0x5C after
  0.45 s (0x7A18D0). +0x60 = 3 is set by 0x594920(x) (x > 0.95 also locks
  the clutch, state 3 and engagement 1), called from the race-start code
  at 0x41E41C with the clutch pedal +0x2F4 — only in transmission mode 3,
  which single-player play never sets (see Human driver update), so the
  clone's manual shift without refusals matches.
  **Held clutch** (transmission +0x74, cleared by its constructor
  0x5AFB14; verified, ported): in 0x5A1410 (mode ≠ 3, jump table
  0x5A16FC) the power is 1 instead of 0 / 0 / 0.7 in states 0 / 1 / 2,
  Changing does not move on to Engaging (the shift offsets are still
  re-rolled, 0x594720) and every state ends in 1 (clutch open, no torque
  through it, the clutch engagement left where it was).
  **Gear colour** (verified, ported): car +0x448 is the nitrous block's
  +0x408 (car +0x40), which the mover stores each frame as transmission
  state +0x44 = 3 (clutch locked; 0x5A593B, next to +0x3F4 = the gear);
  the drag tach gives 3rdPersonGear its own colour (+0x98) while locked,
  else 0xFFFF8000 (0x4C7386); the plain tach 0x4C68D0 0xFFC3C3C3 locked,
  else 0xFFFF8040 (0x4C6AA6). **Verified**: 0x594F30(rpm, gear, flag)
  without the flag gives the unquantised rating: early = the up-shift rpm
  (engine +0x84 + gear × 4) − rpm, w = 0x7FBF60[class × 9 + gear] × +0xE4;
  with w > 0, past the point 1 within (1 − 0.95) w (0x7FBF5C), else −1;
  before it 1 − early / (0.95 w), 0 below 0. Only with the flag does a
  rating over 0 become 0.5 (under 0.5) or 0.85.
* **Street X**: own tune (0x7FC33C, already ported), road surface forced
  (wheel surface 1), HUD_Short_Track; a done racer stops colliding with
  cars (0x606CC0, above); the Quick Race lap knockout can be on
  (0x53F477). **Racer path rate** (verified; ported, `RacerRoute::
  line_path`): the path build at 0x42B340 (each frame from its update
  0x42B570; the name AIActionJustDrive is **open**, no link to it is
  verified) gives its racing-line lane record (lane 0, offset
  0) the rate 0.2 (0x783680) in Street X (or 0x89010C), else 0.05
  (0x7E8E64), and builds through 0x420D40 → 0x41D530: the offset starts
  at the car's own off the racing line (0x60BFB0) and moves to 0 by rate
  × each point's step (0x40FC80), so a racer off the line rejoins it over
  20 m per metre of offset (5 m in Street X). The clone's racers drive
  this path in Street X (**verified**: the path is kept between frames and
  re-planned from the car by 0x41BFE0's test, 1.5 m / 4.5 m in drift races,
  or when the car's route has ended; the room clamp **inferred**, see
  Drag → Lanes). Elsewhere they drive it at 0.05 (0x7E8E64, read at 0x42B382; **verified**),
  wired in the clone since the contact normal turn got its X+0x64 gate
  (Contacts → Impulse). Before that gate the wiring made `game --selftest`
  fail (`rivals at 2052 s`): rival Philip, held against a barrier face, had
  the face's normal turned toward his centre of mass once it was 0.010 m
  past the plane, was pushed 1.55 m through the wall and fell with no floor
  below (**clone observation**; the game turns the normal only for faces
  whose byte 0 is nonzero). With the gate and the rate wired the self-test
  passes. Only the 0x420D40 branch of 0x42B340 is modelled; its 0x41D160,
  0x428DF0, 0x40FBE0 and 0x401D40 calls are **open**. The block after the path build (0x423FFC, in GetUnstuck's update 0x423E20,
  vt+0x38) is the racer-leaves-road step. Its gate exits to 0x42415B whenever a flag is set, the node is 0 or the key is 0 (**verified**); whether it does so on every frame
  in the shipped binary (the key paragraph below), so its normal, path B,
  braking and 0x402940 branches never run (**inferred**: the gate code is **verified**, but the key stores not
  verified below are **Inferred**; this replaces the
  earlier inferred reading). The path build is reached on every frame
  (0x423FF7, from both 0x423FD2 and the fall-through); 0x423FC0 loads 0x7E8E64
  (0.05) into edx on one branch, but 0x420D40 writes edx at 0x420DD2 before
  any read in its own body (its entry code 0x420D40–0x420DC8 does not use
  edx; 0x41D160 at 0x420DCA was not checked for an edx input).
  No Ghidra procedure starts at or contains 0x423E20 or 0x423FFC (REA reports
  none), so the block is read from speed2.dis. **Verified** (speed2.dis,
  re-read this run): the search key is the full dword [action+0x218] (pushed
  at 0x423E51), and 0x60C130 is called at 0x423E5A with ECX = the controller (esi), not the route. The gate
  at 0x423FFC–0x424032 sends the block to 0x42415B when the byte 0x89E7E3 or
  the dword 0x89010C (Street X), or the byte 0x89E7D9 or the dword 0x890104
  (drift), is nonzero, or when the search result is 0. 0x42415B sets
  [action+0x1FC] = 1 and falls into the tail, which exits at once on that
  flag. At 0x424038 the same gate sends the block there when the dword
  [action+0x218] is 0. Otherwise it calls 0x40DF50(car, 0) (pushes 0, then the
  car) and takes the 0x40DEE0 branch only when that result is 1 (0x424053) and
  0x60C000(entry) > 0 (0x424060–0x424068). The branch pushes 0.2 (0x783704),
  calls 0x40DEE0 and jumps to the tail at 0x424162. Every other exit runs the
  normal path at 0x424083. The normal path (**verified** call order,
  0x424083–0x4240C4) calls 0x60C060 with ECX = the controller (esi) and arguments (out, route,
  [esi+0x30C]) (0x424083–0x42408E), then 0x614890 with ECX =
  the dword [action+0x218] (0x424093–0x424099). It goes to the B path at
  0x4240F7 when 0x614890 is false or when the controller type [[car+0x14]+4]
  is 3 (0x4240AC–0x4240AF). Otherwise, when the flag [esp+0x38] is nonzero
  (0x4240B1–0x4240B7), it pushes 0.2 (0x783704) and 0.0 (0x783708) for the
  0x402940 call (0x4240B9–0x4240C4); when the flag is zero it goes to
  0x4240D4. The dot test reads the dword again at 0x424116. The 0x402940 body and
  the 0x60C060 body were re-read this run (REA decompile): FUN_00402940
  sets this+0x1F0 = max(0, |P| − arg3), with P the float triple at
  *(node+0x10), node = DAT_0089E970[route.+0x2C] ("point-B" is a label, not in
  the binary), and arg3 = 0.0 at this call (pushed at 0x4240C5; the 0.2 pushed
  at 0x4240C4 is a fourth stack argument, since the callee's rets are ret 0x10
  at 0x402A1F and 0x402A30, and the decompiled body does not use it), and
  this+0x1F4 = max(0.1, (d − elem.+0xC − route.+0xC) / route.+4) when
  |route.+4| > 0.01 (else 0x47C34F80 = 99999.0), with d = 0x60C000
  (route.+0x10 − elem.+0x10); FUN_0060C060 writes out[0] = bfb0(route, p) −
  bfb0(elem, p) (the same float p in both calls; ECX = route, then elem:
  0x60C07C, 0x60C088) and out[1] = route.+0x10 −
  elem.+0x10. The flag [esp+0x38] comes from 0x423F54–0x423F8A: 1 iff
  (route.+0xC + elem.+0xC − 2.8) < d, strictly (fcompp, then jnp keeps the
  byte at 1). The vt+0x7C call on [car+0x2C] was not re-read. Constructor (**verified**, re-read this
  run): 0x423C30 stores the vtable 0x785784 (0x423C54), +0x210 = the clock
  [0x865188] (0x423C64–0x423C72), +0x214 = arg3 (0x423C78), and +0x218 as one
  byte: sete cl, then mov BYTE PTR [esi+0x218],cl (0x423C8C–0x423C8F;
  0x423C9F–0x423CA2 on the other branch). The byte is 1 when |arg3 + 1.0| >
  0.001 (0x78435C, value 0.001), else 0. The base ctor 0x4234A0 (body to its
  ret at 0x423554) makes no reference to +0x210–+0x21F; its callees 0x40C960
  and 0x41D160 were not checked. So the gate, the search and 0x614890 read a
  dword whose bytes +0x219–+0x21B this chain does not write. The ctor's this
  comes from 0x440E50 (called at 0x426410 with [0x81CB6C]; not traced).
  **Verified** (this run, code): the pop routine at 0x440D70 zero-fills
  [esi+0x28] bytes (rep stos, 0x440D7C–0x440D93) only when [esi+0x14] has bit
  0x2 set. The pool used for GetUnstuck objects (pool flags 0x0B and size
  0x230 as the worker recorded them, not traced here) and the allocation at
  the six 0x423C30 call sites were not traced, so the zero extension of
  +0x219–+0x21B is **Inferred**. The exit does not depend on it: any key that
  is not an object pointer gives route = 0. The GetUnstuck objects are made
  only by the six 0x423C30 call sites (0x42643B, 0x427346, 0x42D1BF,
  0x42D51F, 0x42D7BF, 0x42DB23). Key stores: the ctor byte (sete, 0 or 1;
  0x423C8C and 0x423C9F, **verified**) and the dword FUN_00414930 returns
  (**verified** by REA decompile: 1, 2 or 3), stored by FUN_0041CE30 at
  0x41CE75 (**verified**; its vtable entry at 0x7859A8 is not checked).
  0x423BC0 (byte 1) and the serialiser sites 0x40E6A0–0x40EF67 are
  **Inferred** (not re-read this run) to move the same small values. The id
  table DAT_0089E970 is written only by 0x63A585 (stores 0), 0x63B5EA and
  0x63B64C (store the object pointer), and 0x63B71A (stores ebx, which is 0:
  xor ebx,ebx at 0x63B6C5) (**verified**; direct writes only). So a nonzero
  small key matches no slot, 0x60C130 returns 0, route = 0, and the gate exits
  to 0x42415B on every frame. A zero key can match a slot whose value is 0
  (0x63A585 creates such slots; whether a controller entry refers to one is
  not checked), but 0x423E9B skips the branch and 0x424038 exits for key 0.
  The earlier model
  of the key as an object-table node (key.+0x1C as an owner) is wrong: the
  key is not a node. Raw callers of 0x423C30 (**verified**): 0x42643B (arg3 = −1.0,
  pushed at 0x42642E, so the byte is 0; the earlier note that it never reaches
  the block does not follow, because the gate reads the whole dword), 0x427346
  (−1.0 in the earlier trace), 0x42D1BF, 0x42D51F, 0x42D7BF and 0x42DB23
  (stack values, meaning **open**). REA's caller list (0x4270E0, 0x42D0F0,
  0x42D470, 0x42D6F0, 0x42DA70) omits 0x42643B (its relationship coverage is
  partial). Other routines store to +0x218 as dwords (for example 0x41CE75,
  0x42511E, 0x425604, 0x425664, 0x426F99, 0x427034, 0x42869D, 0x447690,
  0x4B4D3B, 0x4EA9CE, 0x515FF1, 0x559E03, 0x5CF392–0x5CF3C0) and as byte 1
  (0x423BC0, 0x42F098); none is typed to a GetUnstuck object (**open**). The
  update reads the dword at 0x423E47, 0x423E9B, 0x424038, 0x424093 and 0x424116 (re-read this run). Builder
  0x6148F0 (the source of the table that 0x60C130 searches) has one raw
  caller, 0x428C71 (in 0x428720, per the earlier trace). Its REA callees
  (coverage complete): 0x4038D0, 0x403A40, 0x403D60, 0x403D80, 0x403DA0,
  0x404910, 0x40ADD0, 0x4123C0, 0x419BC0, 0x419C10, 0x43C370, 0x43E6D0,
  0x502FD0, 0x50D510, 0x5AACE0, 0x5B5340, 0x5BAE70, 0x5D5CE0, 0x5EC070,
  0x75C401, 0x75C461. Its keep and distance tests (an earlier tracer's
  decompile, not re-read here) use 0x5EC070 (byte [ECX+0x998]; +0x998 not
  traced), 0x404910 (max(125, v·s), with v from vtable slot +0x60, not
  identified), 0x403A40 (its F via 0x60BE40 and 0x5EC340, not traced; its
  speed term 0x4038D0 is partly read) and 0x5D5CE0 (not traced). The other
  callees are not traced here, and the clone builds no table of this kind. The
  road check 0x40DF50 (earlier decompile) has these REA callees, two call
  references omitted: 0x4036E0, 0x40CC40, 0x43C2B0, 0x43C370, 0x43CE50,
  0x43D090, 0x591FD0. None is traced here, so its road test is **open**.
  Common tail 0x424162–0x4241AE (**verified**, re-read this run): exit when
  [action+0x1FC] is nonzero or [action+0x1F0] >= 20.0 (0x783710); when P =
  [0x890118] is 0, set [action+0x1F0] = 20.0 (0x42419B); otherwise call
  0x402000 with ECX = P and set 20.0 when its result is above 10.0 (0x784560).
  0x402000 (REA, this run): (DAT_00865188 − [ECX+4]) × 0.00025 when [ECX+4] is
  neither 0 nor 0x7FFFFFFF, else −1.0. So P is a pointer whose +4 field is the
  stamp; what that field is, is **open**. 0x40DEE0 (earlier decompile,
  consistent with the raw branch): +0x1F4 = 0.2 and +0x1F0 = v − 0.2 × μ ×
  (9.8 + c × |v| / (2m)), with μ = (+0xFC + +0xDC) / 2, m = +0x110, c = +0x264
  + +0x268 and H = [car+0x20] (9.8 at 0x7FBEF0); the meanings of +0xFC, +0xDC,
  +0x264 and +0x268 are **open** (the clone's car.rs reads +0xDC as the rim
  size). Blocking (no code changed in this run, so the self-test was not
  rerun): (1) the search key, the dword [action+0x218] (its upper bytes and
  the other writers above): **open**; (2) the controller table: 0x6148F0 and
  its callees, the table 0x89E970 and the entries the clone does not build:
  **open**; (3) the road test in 0x40DF50 and its callees: **open**; (4) the
  spec fields +0xFC, +0xDC, +0x264 and +0x268 of H = [car+0x20]: **open**; (5)
  the normal path: 0x60C060, 0x614890 and 0x402940 (earlier trace) and the
  vt+0x7C method on [car+0x2C] (not identified): **open**; (6) the stamp at
  P+4 of the tail: **open**. A gate-only port would be unfaithful: (1) to (3)
  decide which branch runs at all, and (5) and (6) decide the rest. The
  clone's Unstuck keeps one reverse speed while it runs (driver.rs,
  UNSTUCK_SPEED), and game.rs passes line_path 0.2 in Street X and 0.05
  elsewhere (0x42B357; wired since the X+0x64 gate).
  0x42D32D / 0x42D92D (in two goal choosers next to 0x42D470) skip a block keyed on racer +0x14 → +0x902 and table 0x81CB60 +0x934 in Street X (not traced further): **open**. The other readers of 0x89E7E3 (decompiled where marked; port status from the clone's citations): 0x4032C9 (FUN_004032A0, decompiled): the look-ahead point is 3 ahead, or 1 in a drift or Street X race (0x89E7D9 / 0x890104 / 0x89E7E3 / 0x89010C); **ported**, `facing_point` is called with drift || street X (game.rs). 0x408513 (FUN_004084A0, decompiled): under the 0x89E7F8 gate (the catch-up option), with 0x89E7E3 and 0x89010C clear it takes the difficulty-indexed pair of curves, otherwise the fixed +0x2C0 / +0x318; **function ported** (route.rs, the street-X curves), but reached since 2026-10-29 in Quick Races with Catch Up on (game.rs `step_race`). **Traced**: 0x89E7F8 is written by the Quick Race set-ups (0x601E34, from the result of 0x5028F0 on the race record at 0x83AAB4; 0x5267BC and 0x5268A2 store esi; six writes from 0x53F225 to 0x53FB1E) and by the race-type defaults (0x4FEF8C to 0x4FF16C, values not checked here); it is read at 0x4084B9 and 0x41757B (and at 0x5F6771 and 0x5F9646, not traced). 0x5028F0 is the mode record's +0x05 (catch-up), 0x53F160 stores it. 0x4073E2 (FUN_00407370, decompiled): returns 0 in a drift or Street X race (also when 0x890104 or 0x89010C is set), else 1 when the global DAT_007E94A0 is set and the car's speed (× 2.237 mph) is at most 30 mph (param 0) or 10 mph (param 1); its only caller is FUN_00407400 (0x407400, decompiled): a recovery state machine on the racer's +0x2F9 (the last reader value), +0x2FA and +0x2FB (state 1 or 2) and the time stamp +0x300. State 2 sets +0x1EC to +0x1F8 from +0x304 and +0x308 (0.8 bands); state 1 sets +0x1FC to +0x208 from +0x30C and +0x310 and the flags of FUN_005ED480 (not traced further). **Not ported**: the clone's Unstuck drives the reverse speed only (`UNSTUCK_SPEED`), so this machine has no counterpart. 0x40E84B (FUN_0040E7C0, decompiled): the stuck test returns −1 or 500 (the priority of the unstuck goal); in drift or Street X races it skips the block of car-state and speed tests and tests only the loop over recent contacts (list 0x5A0830, kinds 0, 2 or 3 by 0x402A40, ages 0.4 s and 0.25 s, and a speed below 5 or FUN_004010C0(speed, 5.0, eps) non-zero), returning 500 when one qualifies; its callers are the goal choosers 0x42D0F0, 0x42D470, 0x42D6F0 and 0x42DA70. **Ported**: that skip (driver.rs `Unstuck::update`: the off-route and road-edge tests need neither drift nor Street X). The 2 s settle is FUN_005ED590 (the clone's SETTLE). **Open**: FUN_005EF000 and FUN_0060BF00 (the early −1 returns) and FUN_005EC0C0 (the branch that picks the off-route test over the class test; the clone always applies the off-route test) are not traced. The class split is not modelled: its test passes for controller type 3 at speed 25 or below and type 4 at 20 or below, and needs FUN_0040DF50(car, 1) to return 1; other types never pass it. 0x4209C7 (FUN_00420940, decompiled; the path begin: the plan, the three clears and +0x1FD = 1 run always, the rest only when FUN_005EC0C0 is non-null): it plans the path from the car (0x41D160(0, 0)), clears +0x1FC, +0x1F8 and +0x1FE, sets +0x1FD = 1, places the facing point (0x4032A0, then 0x4122C0 with +0x30C as its fourth argument) and sets +0x21 = 1 in drift or Street X (the four flags of 0x4073E2); it then runs 0x41D530 and sets +0x1F0 to −15 for controller type 3, else −6. **Open**: the readers of +0x21 are not traced; the clone's `line_path` with `fresh`, which game.rs sets while the unstuck runs, is the nearest counterpart. 0x423FFC (not a procedure in Ghidra; a block after the path build 0x420D40, skipped in Street X / drift): **open**. 0x42F505 (FUN_0042F330, decompiled): after the path build (0x420D40, called at 0x42F500), the four flags at 0x42F505–0x42F529 set bVar3. In Street X or drift the block that otherwise runs when bVar4 holds is skipped and the normal branch runs; that block sets +0x1F4 from +0x21C, +0x1F8 to 2.0 and +0x1F0 to the speed plus a quarter of the magnitude of the path's lateral value (0 when it is positive), floored at 20. **Traced**: FUN_0042F330 is AvoidAvoidable's update; bVar4 / bVar5 are its near / far: unless 0x42DEC0 (the goal's slot +0x20: 1 → 0x428FD0, 2 → 0x42BF00) found a plan, the object is ghosted (0x60BF80) or traffic (0x6148D0), +0x21C ≈ 0 gives near when 0x60C000's gap > 0, else the gap in +0x21C s (0x60C060, 0x5EC3B0) ≤ 3 gives far, else near; 0x60C130 returns the object's road entry. The clone's counterpart is `Actions::avoid` (see AvoidAvoidable). 0x4086B3 (inside FUN_004084A0): the second output (param 4) is computed outside Street X / 0x89010C, or in them when the tuning byte +0x420 is set (Ghidra shows it as cRam00000420): **open**, the clone does not read that table. 0x417531, 0x41767C gate on tuning bytes +0x422 / +0x423 (and 0x4174D3 on +0x421): traced, see AI route layer → Catch-up (the shipped bytes are 0). 0x40AA20 (FUN_0040AA20, decompiled) moves a value towards 0 by rate × dt: 0.2 (0x783CC0, verified) when 0x89E7E3 or 0x89010C is set, else 0.04 (0x783CBC, verified), and it stores 0 when it crosses 0. It is called from FUN_00422220 at 0x42265E, under the record's fade flags (+0x74 / +0x75 when the byte at [esp+0x13] is non-zero, +0x7C / +0x7D when it is zero; what that byte means, and the forward / backward names, were not traced). **Not ported**: the AI path walk FUN_00422220 is not ported (the clone reads the flags only in the standings walk, `src/standings.rs` `walk`), and the rest of that walk is not traced. 0x41CB4C changes nothing (both paths join). The race flow's 0x6074C3, 0x607FAA, 0x6084A0, 0x6087E5 are inlined copies of the done-racer rule (drift / Street X or status 8 / 9 → stop colliding; ported); 0x60907B sits under 0x89E7E7 (Race Breaker, online / LAN). 0x5EE078.. are replay records. **Not ported**: the open readers above (their effect on the AI); the path rate above is ported.
* **GetUnstuck's gate key and path replan** (0x423E20, 0x60C130, 0x423C30, 0x440D40, 0x420D40; verified unless noted). The gate's search key is the dword [action+0x218], pushed at 0x423E51. 0x60C130 (REA decompile) returns the controller's node whose 0x89E970[index] equals the key, else 0, and 0x424030–0x424032 (`test ebp,ebp; je 0x42415B`) exits when that result is 0, and 0x424038 (`mov eax,[edi+0x218]; test eax,eax; je 0x42415B`) exits when the dword key is 0 (**verified**). The factory 0x423C30 stores only the byte +0x218 (`mov BYTE PTR [esi+0x218], cl`: 0 when the minimum-run argument is within 0.001 of −1.0, else 1), and the base ctor 0x4234A0's own instructions write none of +0x218 to +0x21B (its callees 0x40C960 and 0x41D160 were not checked). The pool allocator 0x440D40 zeroes a block only when the pool's flag +0x14 & 2 is set (0x440D7C–0x440D93), so bytes +0x219 to +0x21B are stale pool contents unless that flag is set; the action objects' pool and its flag were not traced, so their value is **open**. The gate's outcome depends on them only if they reproduce a table value. The 0x89E970 entries are written at 0x63B5EA, 0x63B64C and 0x63B71A (their sources are **not traced**; 0x63A585 stores 0), so "the block never runs" is **inferred**, not verified. 0x89010C and 0x890104 have no store-form write in speed2.dis (every reference is a read; block copies are not ruled out). The path build 0x420D40 (REA decompile): with the replan test 0x41BFE0 false, it cuts the kept path with 0x41D160(car, v), v = max(30, speed × 0.5) (speed × 0.25 when 0x89E7D8 or 0x890108 is set; v = 0 with no car when car +0x30 → +0x1D9 is clear), then calls 0x41D530; with the test true, it calls 0x41D160(0, 0) and then 0x41D530, so the path is rebuilt with no car argument (0x41D160(0, 0)); what that call does with no car was not traced. The mode argument is 1.0 unless the rate is 999.0, and 5.0 when 0x89E7D8 and 0x890108 are clear, one of 0x890114 / 0x89E7D9 / 0x890104 is set, and car +0x1C is nonzero. The clone's `fresh` replans on every frame during the unstuck, which is not that test (**inferred** deviation; see the racer path rate above).
* **Outrun / Free Run** (Quick Race). **Verified**: UI_QRModeSelect's
  options (built at 0x4B2D60) each store a race type through 0x4B2C00
  (into 0x83AAB4, or 0x850088 in game modes 8 / 0x200): Circuit 0, Sprint
  1, Drag 2, Drift 3, URL 4, Outrun 8, Free Run 9, Street X 10. 0x4B2C00
  then sets the type's record (0x83AAB4 + 4 + type × 20) bytes +0x0C = 1
  and +0x0D = 3 (5 for URL; 2 and 2 in game mode 4; 0x502930 / 0x502960
  write 0x83AAB4 + 0x10 / 0x11 + type × 20, which 0x526730 copies to
  0x89E7C0 and, when that is 1, 0x89E7C4: the opponents). The record's
  +0x10 (0x83AAC8 + type × 20) is the traffic level (Traffic). Outrun (icon
  0x086C74CB, "Outrun" 0x613D3106) is only added in game modes 8, 0x200
  and 4 (0x850078), Free Run (0x713D99C1 / 0x6F2421B7) only outside them,
  so single-player Quick Race (game mode 2) lists Circuit … URL and Free
  Run. Outrun's accept (0x4B2CD0) clears 0x83AAB4 +0x114 / sets +0x118 =
  0xFF; Free Run's (0x4B2CF0) sets its record's track (0x502910: u16 at
  +0x18) to 4000, the free-roam city, and +0x1C to 0. The next screen
  (0x4B2A80) is UI_QRTrackSelect, except UI_QRCarSelect for Free Run
  (no track to pick) and UI_QRModeOptions for Outrun in game mode 4.
  Clone: Free Run drives off in free roam (track 4000's region) with the
  pause menu's "Return to Free Run" and no career triggers; quitting to
  the main menu ends it. **Verified**: the start — every race, Free Run
  included, is placed by 0x6065D6 → 0x5EB5E0 on the track's
  "ForwardStartingPosition" marker ("BackwardStartingPosition" when
  0x89E7A4 is set), the marker the clone's free-roam start reads.
  **Verified**: the file. The track loader (0x57F2E0, state 3) reads
  TrackPosMarkersFreeRoam.bin for a track id that is a multiple of 1000
  (0x57F400-0x57F411; format string 0x7A0C2C), else TrackPosMarkersAll.bin
  (0x7A0C04); the folder is the track record's +0x40 string; the result is
  kept at +0x14. 0x5EB5E0 looks up the name
  ForwardStartingPosition (0x7A5C80, hashed by 0x43DB50) among the records
  that 0x5D8480 appends from chunk 0x34146 (it checks dword 0x34146 at the
  buffer start, reads size at +4 and appends floor((size + 8 - (A - buf)) / 48)
  0x30-byte records from A = (buf + 0x17) & ~0xF; registered for tag 0x34146
  at 0x77E720, with 0x5D84E0 as its third argument), so track 4000's marker
  comes from that file. The clone's
  free_roam_start (src/frontend/career.rs) reads the same file,
  TRACKS/ROUTESL4RA/TrackPosMarkersFreeRoam.bin. **Inferred**: the generic
  loader passes the file to 0x5D8480 by its chunk tag (not traced through
  0x57CF00). **Verified**: 0x5D8530 and 0x5D85E0 keep only records whose
  dword at +0x08 is the name hash and, when DAT_00883DAC is set, whose dword
  at +0x24 equals the track record's short at +0x8A (0 means any). 0x5EB5E0
  then takes, per slot, the first such record whose +0x0C equals the slot
  index; with too few records, or no slot match, it generates grid positions
  instead. **Open**: the track id DAT_0089E7A0 is stored as 4000 at 0x540581
  and 0x5407E6, and which mode those stores serve was not traced;
  free_roam_start tests only the name (first match in file order), and the
  track-4000 value of +0x8A, the file's records and the ROUTESL4RA folder
  choice were not checked. No career events, shops or shoots in Free Run (game mode 2 has
  no career loaded). **Open**: Quick Race's Outrun (online / LAN /
  split-screen only) is not in the clone; career free roam's outruns are
  (see Outrun).
* **Where the city start puts the car** (**verified**, 2026-10-28): the
  career keeps a place at career 0x85AD40 +0x8744 (flag, = 0x863484),
  +0x8750 / +0x8754 / +0x8758 (x, y, z) and +0x8760 (u16 heading); the
  save writes and loads it with the career (0x5145F3 / 0x52930D). The
  city starts 0x5404B0 (0x540691) and 0x540790 (0x540927) copy it, when
  the flag is set, into the racer record's +0x91C flag / +0x920 position
  / +0x930 heading, and the car placement 0x6065C0 (0x606666) then puts
  the car there instead of the grid slot from 0x5EB5E0
  (ForwardStartingPosition, the airport). Writers: a new career clears
  the flag (0x532840); a career race engaged in the city (race dialog
  0x499B27, World Map 0x4E1284, debug 0x541690 → 0x541170) clears it
  and keeps the free-roam car's place (0x8900AC +4: +0x60..+0x68 and
  +0x40C); "Return to Explore Mode" from a race's pause or after it
  (0x4F39A0, 0x451E768E → 0x4A3DF0 → 0x513950, also 0x606740 case 0xB)
  sets it, re-keeping the car's place only in race state 7 (an outrun);
  entering the garage from the explore pause (0x4C2170 → 0x4A7C70 /
  0x4A7CC0 → 0x513B30), saving from it (0x4A7CF0 → 0x5139C0) and
  quitting to the main menu in a career (0x4A3E80 → 0x5139C0) set it and
  keep the car's place when in free roam (race state 0); the magazine
  reward (0x498E70 → 0x500AB0, kinds 3 / 4 only) keeps it too; so does
  **entering any shop** (Shop_EnterWithScene 0x499470 → 0x54CB00,
  0x54CC0C: flag 1 and the car's place at the trigger, before the
  drive-in scene): the car lot's and the garage's saves hold the
  trigger's place with the flag set (the original's car_lot.sav
  (−726.51, −875.20), garage.sav (654.11, −97.64)); leaving the garage
  (0x9A3B571D → 0x4AF607 / 0x4AF616, **after** its save point 0x4AF360,
  so a save made there holds the flag as it was) and Resume past the
  movie (0x495AD4) set the flag, then 0x5404B0.
  **Resume Career** (Career_Start 0x495A30, **verified**, checked under
  Wine 2026-10-31 with the original's own saves): DDAY_EVENT_B won
  (0x500B80(0xDD60E403)) → UI_CareerCrib.fng, whose Return to Explore
  Mode puts the car at the kept place; else the intro movie not played
  (0x86347C) → scene01 (0x53D720) and the airport start (a new career
  clears the flag, 0x532927, and its MC_Main save precedes the movie, so
  such a save holds flag 0); else 0x5410F0 with A, or B once A
  is won (0x524250 posts Rachel's hints for A only), the flag set and
  0x5404B0 at the kept place (the D-Day script then 9, 0x5404FE). Under
  Wine: the car-lot save (s01) resumed in the lot at (−722.38, −878.83),
  heading 0xF8C0 as saved, script state 9, no drive-out scene, the GPS
  reading "Unable to contact satellite"; the first-win save (s02, flag
  0) opened the garage and its exit stood the car at the race's engage
  place (462.42, −46.16). So per quit point: before the car lot (no save
  since Start New Career) the movie and the airport again; car bought
  and the lot's save accepted: the lot; the lot's save declined and the
  game left: as before the lot; Quit to Main Menu then Resume in the
  same session: where the car was (0x5139C0, memory only); after the
  garage or a race (saved): the garage, then the garage's trigger or the
  race's engage place. A save with the flag set and no place written
  would put the car at 0 (0x863490 is BSS); no save point allows it
  (**inferred**: not every save path was traced). 0x86501C, the start marker of 0x42FC60 / 0x430240
  / 0x431930, is never written by code (reads only; BSS), so it is 0.
  Clone: `Profile::explore_set` / `explore_pos` (saved as `explore`),
  written at those points (`Game::start_race`, ReturnToExplore,
  JumpToGarage, the garage's Back, the quit, `career_intro`), and
  `Game::explore` takes the saved place in a career (not Free Run) when
  no place is given; `Game::here` counts only a free-roam car (barrier
  set-up `free_roam`, race state 0), since city races share L4RA.
  Selftest: back at the engage place after the World Map race, and at
  the saved place out of the garage after Resume; `selftest_resume_points`
  resumes the original's four saves and plays each quit point (new
  career left before the lot, Quit to Main Menu in D-Day A, the car
  bought with the save and without, the garage, a race), the game left
  being a reload of the file. Fixed 2026-10-31 (issue: buying the
  first car and then leaving the game placed the car at the airport): the
  clone kept no place on entering a shop, so its car-lot save held the
  flag with no place and Resume fell back to ForwardStartingPosition;
  and the garage's Yes did not set the flag, so a race's save resumed at
  the airport out of the garage. **Open**: the outrun case (the clone's
  outruns stay in free roam). **Fixed 2026-10-31 (reports)**: the kept
  place is car +0x60, the centre of mass (0x500AB0, 0x513986, 0x513A1D
  read +0x60 / +0x64 / +0x68 and +0x40C); 0x5F5020 writes +0x60 = the
  point and +0x50 = the point less the unrotated offset, but once the body
  runs +0x50 is the point less the *turned* offset (**observed** under
  Wine: kept place written at heading 90°, Resume → +0x60 = (1695.25,
  −883.64) exactly, +0x50 = (1695.25, −883.89)). The clone kept its body
  origin (`Game::here`) and placed the origin at the point less the
  unrotated offset, so each resume moved the car by (R − 1)·offset; now
  `here` gives the centre of mass and `CarState::placed` subtracts the
  turned offset (test `placement_puts_the_centre_of_mass_on_the_point_at_any_heading`).
  **Inferred**: where the rigid body's constructor reads +0x60 is not
  traced.
* **Quick Race car select** (UI_QRCarSelect, class 0x4F9D90 in game mode
  2; **verified** except where noted): after the track (and straight from
  the mode for Free Run, 0x4B2A80) comes UI_QRCarSelect with the
  package's "Quick Race Car Select" title, the race type after "Race
  Mode:" and the filter's name on both "Filter type goes here" strings
  (0x4B2746: 1 "Stock Cars", 2 "My Cars", 4 "Career", 8 "SPONSOR CARS",
  0x10 "Ranked Cars", 3 "All Cars"). The list (0x4E22E0) holds the
  profile's car records of the filter (0x5162D0) that Quick Race has
  unlocked (0x534190); a filter with none falls back to All Cars
  (0x534860). Customize (0xA936C3A2) and 0x89782734 show only for filters
  4 and 8 (0x4B2310). Accept (0x4FC2E5) races, asking the transmission
  first when the player's Ask for Transmission (0x83AA6A) is on, except in
  drag races (0x83AAB4 = 2): 0x497D50 opens "Transmission:" (0x30003D27)
  with Manual (0x1B3A2485, message 0x5F5E3886) and Auto (0x61CB0720,
  0x1A2826E1), which set the player's Transmission byte 0x83AA79 to 1 / 0
  and start the race (0xBA5B5FCE); Back (0x1FAB5998) is not handled by
  the car select, so the dialog just closes. The race's player takes that
  byte (racer info +0x20, 0x5F17F9; manual forced in drag, 0x5EB440), and
  the clone's gearbox follows it. **Inferred**: the current setting's
  button starts selected. Filters (**verified**): the filter is the
  global 0x7F444C (initially 3, All Cars; written back to the profile's
  0x83AAC0 on each change); 0x4B2746 names 3 "All Cars" (0x6B68362C),
  else its highest bit's (0xE798A2F7, 0xE7AB43D3, 0xB4045365,
  0x26FECEE7, 0x1A23D5E6). The shoulder triggers switch it (0x4EED10 in
  game modes 2 and 4; online 8 / 0x200 and 0x20 have their own orders):
  PAD_RTRIGGER (0xD9FEEC59) steps All → Stock → My Cars → Career →
  Sponsor → All, PAD_LTRIGGER (0x5073EF13) the other way, skipping a
  filter with no unlocked car (0x497EE0 → 0x534860; Stock is never
  tested); a change rebuilds the list (0x4EEC90) and the buttons
  (0x4B2310). Clone (`next_qr_filter`, `[` / `]`): `quick_race_cars` (menus15: the
  lot's cars plus 0x529CD0's eight, see "Menus round 15") for All and Stock alike, the car
  driven stock; Back to the track list. **Verified** (0x497DA0, 0x497EE0,
  0x534860, 0x4E22E0, 0x5162D0, 0x534190, 0x610190): the unlock mask is 0x1F8
  when the game mode (DAT_00850078, **inferred**: its writes are immediate stores, 8 at 0x4F98DF, 0x4BC706, 0x4F9FC9, 0x4FC515 and 0x4FC7DD, 0x200 at 0x4AEE1A, 0x100 at 0x4FCB19, 2 at 0x4DF18B and 0x4FC061, 1 at 0x495A3D and 0x4C5033, 4 at 0x54045C, 0x10 at 0x4AECF6 and 0x20 at 0x4AED3A; the screens of the 1, 4, 0x10 and 0x20 stores were not traced) is 8 or 0x100 and 1
  otherwise, so mode 2 tests Stock (that mode 2 is Quick Race is not confirmed: the screens of the 0x4DF18B and 0x4FC061 stores were not traced). The list and the skip count
  both walk the profile's 91 car-record slots (+0x9BD8 of the caller's
  `this`) for records whose flags (+0x14) hold the filter's bits. The skip
  count passes a record without the Career bit (4) when 0x534190 (mask, car)
  does (DAT_00838464 set passes all), and a Career record when its byte
  +0x7F5 is set. The list instead passes each record that 0x534190 passes,
  Career records included, when the car type's +0x840 is not -1 and 0x5022C0
  on that value is true, when 0x610190 on the car type record is true, and
  when 0x516340 on the record's +0x8 is false or its +0x7F5 is set. 0x610190
  takes no argument: the list calls it with the car type record in ECX, and it
  is true when that record's byte +0x40 is set and FUN_0057CAD0 on that
  address returns nonzero. So My Cars
  (2) is offered only when the profile holds a passing record. The clone
  keeps no My Cars records (the garage notes, "Not in the clone", say so),
  and its Quick Race trigger handler (`src/bin/game.rs`, the only production
  call of `next_qr_filter`) passes `|_| false` as `has`, so it never offers
  the filter. **Verified** (REA decompile; the region global 0x7FA73C reads
  6 in the file, DAT_00838464 reads 0): the list is built from the profile's
  car records (the car type entry supplies the checks and is also the first
  argument of FUN_00511DB0, which builds each 0x930-byte list entry; that body
  is not traced). It walks the
  91 slots of this+0x9BD8 in slot order (0x5162D0) for each record whose
  +0x10 is set and whose +0x14 holds the filter's bits (the filter is the stack
  argument; the call sites load it from global 0x7F444C, and the values behind
  the All and Stock labels are not traced): filter 3 takes
  records with the Stock (1) or My Cars (2) bit, filter 1 only those with
  the Stock bit. A record's car type
  is the index its vtable slot 1 returns (-1 is skipped; 0x890 stride from
  DAT_008A1CCC). It passes when the type entry's +0x840 is not -1, 0x5022C0 accepts it,
  0x610190 holds, and 0x534190 accepts the car under the mode mask (1 for
  game mode 2), and when the car is not in the career slots (0x516340) or
  the record's +0x7F5 is set. 0x529F80 and 0x529CD0 build nothing: each is
  a per-car test, both called only by 0x534190, which returns 1 when
  DAT_00838464 is set and otherwise ORs them (mask bit 1: both; bits 2 and
  4: 0x529F80; bits 0x1F8: both). For a car whose id matches the first
  dword of one of the 12 entries at 0x84481C (stride 0x1C), 0x529F80 returns
  the AL left by 0x579D80, a void call to FUN_00576010(6, car) (that
  function's return value and body are not traced).
  Otherwise it fails only when the car is in the table at this+8 (0x516360;
  count at +0x540, stride 0x1C), V1 (the record's vtable slot 1) is not 0x15
  or 0x1B, or FUN_00576010(5, 3) is zero (FUN_00579D40 is then false),
  0x5023B0 on V2 (the slot 2 value) gives
  a nonzero pointer, and 0x500B80 on that pointer's +8 is false (0x500B80 and 0x5023B0 are
  not traced). 0x529CD0 needs the
  car in the table at this+8 with the same first dword as the record's +8,
  and passes when V1 is in the 8-id list at 0x7F7C28 (0, 1, 2, 7, 10, 25, 28,
  22); the list at 0x7F7C08 (0, 1, 2, 5, 10, 25, 28, 22) is for region 0.
  **Verified** removals only: 0x516340 (0x503680 on the career car slots at
  this+0x5954, 0x7F8 stride, id at +8, count at +0x812C) drops a car in a
  career slot unless the list record's +0x7F5 is set (0x4E22E0 checks that, not 0x516340); 0x5022C0 drops the +0x840
  values 5 and 18 (region nonzero) or 6 and 7 (region 0) while DAT_00838464
  is 0; 0x57CAD0, reached through 0x610190 once the name at +0x40 of the object in ECX is
  non-empty, returns 0 (so the car drops) for a name that 0x57CA20 does not
  return from the "bFile System" pool (created by 0x440DF0; the lookup 0x57C5C0
  and the length test 0x43DBB0 are not traced); on a found entry it
  sets that entry's +0x14 to 1 when its +0x18 is positive,
  else calls 0x579BA0(1) (ECX not traced). As far as the traced code shows,
  none of the three inserts a node into the list 0x4E22E0 builds: 0x5022C0 and
  0x516340 only read, and 0x57CAD0 writes only that pool entry's +0x14.
  **Open**: which slots hold which cars and their +0x10 and +0x14 bits, so
  what All and Stock hold for a profile, is not traced; FUN_00576010, the
  vtable targets of the records' slots 1 and 2, 0x500250, 0x4FFC90 and the
  +0x840 values, and the object each per-car test reads as `this` (its ECX), are
  not traced. The clone's `lot_cars` drops indices 5 and 18
  (`LOT_EXCLUDED`), the region-nonzero set read as indices (equal only if
  +0x840 is the index for those cars), and lists type order, not slot order,
  so it stays the stand-in until those are traced.
* **Online** (UI_Main; **verified** except where noted): its accept
  (0x4AECB0) stores the pad (0x50D2A0) in 0x83A9D8 and sends 0xD05FC3A3
  to UI_Main (0x55DDB0), whose handler (0x4F98B0) sets game mode 8 and
  starts the online flow (0x4EFFD0 → 0x4E3960, a state machine over
  UI_OLISPConnect, UI_OLAuthDNAS, UI_OLEALogin, UI_OL_WebOffer2,
  UI_OLSelectPersona, UI_OLCreateUser, UI_OLUseExisting, then
  UI_OLMAIN) on UI_OLISPConnect.fng (routine 0x49DE10, not traced
  further: Header_text hash 0xD5344356; hashes 0x1DB81DB6 and 0x69A8BC54
  follow the connection status). Its tick (0xC98356BA → 0x49DF20) polls;
  on -1 0x4B97D0 calls 0x558550 with a text looked up from a hash chosen
  by the 4-character code at 0x8667E0 ('-dif' 0x81273F29, '-cfg'
  0xB9BCDE4B, '-dev' 0x8645E0D4, '-ini' 0x82EC0F03, '-ath' 0xE097DE5F,
  '-lnk' 0x31D451C9, else 0xE156E946; template GenericDialog_LARGE.fng for
  '-ath', '-lnk' and the rest, string 0x783FE5 for the others); on -2 the
  hash 0x033CE3D4 (the English text is not in SPEED2.EXE). OK, Accept or Back
  leave (0x4A0140: the flow torn down,
  UI_Main pushed). Clone: UI_OLISPConnect, then the '-lnk' error, OK to
  the main menu. **Open** (partly traced): the report on a PC with no
  network and the 2 s wait (ISP_TRY_MS) are not settled. **Verified**
  (SPEED2.EXE): the tick 0x49DF20 returns only 0 or 1 (decompile), so the
  -1 and -2 tests in its message handler 0x4B97D0 (message 0xC98356BA)
  cannot be taken from the tick. While its +0x50 byte or DAT_008500CC is 1
  it returns 0 at once. While DAT_008500D0 is 0 it calls 0x585650 (it reads
  the key 'open' through 0x742B60; the tick discards its result, so no
  failure is reported from it), then 0x5031B0(2) (+0x50 becomes 2, the old
  +0x50 goes to +0x54, the +0x4C byte is cleared) and returns 1. For a
  result of 1 the handler calls 0x4B9690 on the object at DAT_00836370;
  that routine makes a virtual call on its +0x140 object and passes the
  results to 0x4B9650, 0x537990 and 0x43DCC0, none traced further, so the
  screen it leads to is not settled. While DAT_008500D0 is 1 the tick reads
  the key 'type' through 0x742B60, only when +0x54 is 1; when the value
  changes it sets Header_text to hash 0x69A8BC54 for type 0x10 and to hash
  0x1DB81DB6 otherwise. The tick body has no timeout of its own. The -2 text
  is not in SPEED2.EXE (ASCII or UTF-16). The code at 0x8667E0 has one
  reference, the read at 0x4B9812, and the only instruction that names it
  is that read, so there is no literal writer. The only 0x7D0 (2000)
  immediate in 0x4B0000-0x4BFFFF is the body of 0x4BA510 (mov eax,0x7D0;
  ret). That routine has no direct call, but its only reference is a
  code-pointer word at 0x79499C, so its use is not traced and ISP_TRY_MS
  stays inferred (clone src/bin/game.rs line 65).
  **With a network** (the original under Wine, <work dir> 2026-10-31: on_1–on_8,
  cn_1–cn_10, r3; gaps120 on1–on9) UI_OLISPConnect gives way to
  **UI_OLEALogin** (record 0x7F907C: factory 0x4E3C50, help hash 0xE720F694,
  mask 0x405; class 0x4D0690, vtable 0x795ED8, handler 0x4F0060): "EA Login"
  over Create New EA Account (0xED21B47F) and Use Existing EA Account
  (0x036E79B9; option class 0x79475C, whose Accept sets the flow's +4,
  0x4B9C50), over the garage (SHOWROOM), the ESRB notice hidden. In game
  mode 8 with 0x8654A4 = 3, 0x54E6F0 turns mask bit 0x1 (Quit Game) into
  0x2 (Logoff), so the bar reads Back / Continue / Logoff. Accept
  (0x0C407210 at 0x4F00C2), on either option, calls the connect helper
  0x4CFCB0(+0x154, screen, 0x144849C1): "Connecting to the EA Server..."
  (0x4B8EE0) with Cancel sending 0xCA4C311B. The helper's handler 0x4EFE40:
  0xCA4C311B → 0x4B8F40, "If you cancel connecting to the lobby server, you
  will be taken offline and returned to the main menu.^Are you sure you want
  to do this ?" (0xD857CE14, 0x558760: Yes 0xF770 → 0x2C0E485A, No 0x061C →
  0xCA4C3140, No focused); 0xCA4C3140 → "Connecting to lobby..." (0x0023E142)
  with Cancel; 0x6E57AA10 / 0x73C044EC close the dialog (0x4CFE10). Back on
  the screen (0x911AB364 at 0x4F0130) asks "Are you sure you wish to
  disconnect and stop playing Online?" (0xAFFF7C7A, 0x558830: Yes →
  0x2C0E485A, No nothing, No focused). The connection then fails: 0x49F780
  puts "You have lost your connection to the EA Server.  You will be
  returned to the Main Menu." (0x3F920248) up with OK (0x2C0E485A, which the
  base notify 0x55B5D0 turns into 0x4A0140 and UI_Main + GarageMain); the
  help bar shows again under it. Under Wine both options went straight to
  the server dialog and the notice came after 31 200–43 000 game ticks
  (8–11 s). **Clone** (`screens::EaLogin`, `Menu::EaLogin`): that flow, no
  real networking; the attempt fails after `EaLogin::CONNECT_MS` = 9 s
  (**inferred**, network-dependent). **Inferred**: the options' row places
  (measured on the original's frames: text at (−7, −156) + 36 a row, the
  row backing 0x66E29936 + row at (6.5, −146), HIGHLIGHT / UNHIGHLIGHT);
  UI_OLISPConnect's own length (ISP_TRY_MS). **Open**: UI_OLCreateUser /
  UI_OLUseExisting and everything after a successful login.
* **LAN** (UI_Main; **verified**): 0x4AEDE0 resets the pads, sets game mode
  0x200 and pushes UI_PC_LAN_ServerSelect.fng (class 0x4FB4C0) at 0x4AEE14,
  in the branch that tests message 0x0C407210. The name is also pushed at
  0x4A2EE0, so it has at least two code references (the earlier xref check
  missed 0x4AEE14). Its
  constructor opens the network (0x585650); if that fails, "Could not
  connect to Local Area Network" (0xA392D977); else "Looking for LAN
  servers ..." (0x43003B5B) with Cancel (0x541389E5, message 0x7345710F)
  over the server list, closed by Cancel or 5 s later (0x4FB610: timer
  now + 5.0; the list refreshes every 3 s, 0x4F3040). Back goes to
  UI_Main (0x5031B0(0)); BUTTON1 (0xC519BFC0) hosts on UI_PC_LAN.fng.
  **Verified**: UI_PC_LAN.fng (0x7990A4) is pushed at 0x4FB72B and 0x4FB7FF,
  each followed by the screen push 0x537990. Which screen code contains those
  two pushes is **open** (REA resolves neither address to a procedure), and
  which of the two BUTTON1 reaches is not traced. Clone: the search over an
  empty list, then Back.
  **The list** (the original under Wine, lan_1–lan_6): the
  constructor's 0x4F2EE0 binds fifteen slots to scroll_text_%d /
  scroll_racers_%d / High_Light_%d (scripts Highlight / UnHighlight) and
  0x4E8A00 fills an empty list with fifteen blank rows and updates the
  scroll bar (0x538510(1)): no handle, no arrows, the track only. The rows'
  High_Light_2..15 go from INIT to HIDE; the first row's band shows
  (**inferred**: which call leaves High_Light_1 at its INIT alpha 0x91).
  The screen stands on GarageMain (SHOWROOM) with the help bar (mask
  0x40000405: NAME, Host LAN Server, Back, Continue, Quit Game), which the
  search dialog covers. The refresh 0x4F3040 returns early while the list
  is unchanged and dismisses the dialog only on a change; the tick's timer
  path dismisses it once now > +0x900 (0x4FB8AE). Under wined3d the game
  clock runs at ~13 % of real time, which is why gaps120 saw the dialog
  "stay": with the clock read live it went after 5 s of game time. **Inferred**: that the network opens offline (no
  server answers). **Open**: the hosting screen's class and handler
  (UI_PC_LAN.fng), and joining.
