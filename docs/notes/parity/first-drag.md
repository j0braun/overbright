# First career drag: rivals tipping over (2026-11-04)

Issue: a rival tipping over, apparently in the first career drag
race. Earlier the November report triage (internal notes b) found tipping only
after a rival was totaled; this work reproduces it on the real start path
and finds why rivals were totaled so often.

## Reproduction (`--flow firstdrag`, `src/bin/game/dev/first_drag.rs`)

A fresh profile past the intro (`skip_intro`, Peugeot: stage 1 city start,
open list CIRCUIT_1, DRAG_1, …), the first open drag (STAGE_1_DRAG_1, the
Bayview drag 4201), the grid seed set (`NFSU2_FIRSTDRAG_SEEDS`, default 4),
the start scene played through, driven as the window drives it
(`Game::advance`, 1/30 s while the scene holds the cars, else 1/Hz;
`NFSU2_FIRSTDRAG_HZ`, default 60,144,0 = uncapped 1–4 ms frames), on the
player's keys: W held, Left Shift at 95 % of the red line, the arrow keys for
0.25 s when a traffic car is within 80 m ahead and 2.5 m aside (as the November report triage's
`drag_run`). Each grid: the whole race, then a fresh start left 5 s past the
green for the pause's Restart and that whole race again. Every world tick,
per rival: roll, pitch, up·z, drag lane, wheels down, contacts begun (by
kind: player / rival / traffic / wall, from the nearest body to the contact
point) and pushes, speed, crash mode, totaled, drop-out. Fails on a rival
**still racing** past 30° of roll or pitch, upside down, or with no wheel
down for over 30 ticks; post-total motion is reported (`up_after`).
`NFSU2_FIRSTDRAG_LOG=1` prints the per-tick rows.

## Finding: rivals rammed traffic, were totaled, and flipped (reproduced)

24 runs (seeds 0, 4, 7, 20261103 × 60 / 144 Hz / uncapped × first + Restart),
before the fix: **26 rival drop-outs (all status 9, totaled), 7 of them
ending upside down (up·z −0.87 … −1.00)**; no rival tipped while racing (roll
≤ 2.8°, pitch ≤ 10.6° — the launch squat). Almost every total was one
traffic push of 31–38 (the drag total threshold is 30), the rival on its lane
at full speed. Example (restart 60 Hz seed 4, rival 1 MIATA, lane 3):
50.7 m/s, then at 32.90 s a traffic push of 36 → 33.7 m/s, crash mode,
totaled; pitch 35° / 62° / 87° over the next 0.3 s, upside down (up·z −0.97)
at 33.5 s, no wheel down for over a second (the crash's vertical push at the
bumper, whole inertia in crash mode, the L cap of 10). The flipping itself
follows the traced crash response (formats.md, Crash 0x59FB80); what was not
native is the ramming. The rival changed lanes once in that race.

**Cause.** The rivals in a drag race drove the lane action only
(`DragLane::ai_path`, AIActionDriveTrafficLane). Natively their goal "Win
drag race" (chooser **0x42D470**, verified in Ghidra) proposes, after
GetUnstuck and the drag hit actions, **AvoidAvoidable**: 0x42BA20 sweeps the
path the car drives (car +0x30 → +0x10) for 3 s (0x428CE0) and takes the
first object met, priority 300 — no willingness draw in this chooser (the
road chooser's vt+0x9C); 0x40D5C0 between only honks / flashes at a human
object. Its update **0x42F330** (outside the start hold 0x5EF000) calls
0x42DEC0 with the goal's lane class (slot +0x20: 2 for drag) →
**0x42BF00**: the record's lane (the nearest, 0x420ED0, when not 2–5), then
for k = 1, 2, 3 the lanes lane−k / lane+k (lane−k first when the object's
+0x00 > 0), each through **0x428E70**: every step from the record's lane
allowed (0x412500), then the plan (lane, offset 0, rate 0.151, stamp now)
tested clear for 3 s (0x428DF0) and on success written to the record. With
none: a racer object gives the near / far speed, a traffic object (0x6148D0)
leaves the line speed (in a drag race +0x1F0 = the cap, 0x42F330) — so a
rival with every reachable lane blocked still rams.

**Fix** (`Actions::drag_avoid`, src/ai/actions/; called from
`racer_think`'s drag branch, src/bin/game.rs): the above, on the clone's
road entries and sweep (the same `sweep` / `clear` as the road
AvoidAvoidable, whose box stepping is itself **inferred**, see that doc).
After (same 24 runs): **8 drop-outs (7 totaled, 1 blown engine), 1 ending
upside down** (unchanged with the 1.2 lift below); lane changes 595 in
all, up to 17 for a rival in a race (before 82, up to 8). Some
back-and-forth between two lanes within a second is seen (no hysteresis in
0x42BF00 besides the 3 s clear test).

Also fixed: the crash's vertical push factor is 1.2 in a drag race, not 1.1
(0x5A0010..0x5A002D: 0x89E7D8 or 0x890108 → 0x7A1868 = 1.2, else 0x7A1864 =
1.1; verified in the disassembly). The comment had it right, the code used
1.1 always. It makes a crash in a drag slightly stronger (native).

## Open

* The drag chooser's other actions: type 0x12 (600, on controller +0x780),
  0xF "I just hit a wall" (0x403450), 0xC (0x403410 → 0x424560), and
  AvoidAvoidable's end test in a drag race (0x42BA80's 0x4140E0 /
  0x410A50): the clone ends it at the next think and sweeps again.
* 0x60BF80 in the near / far test (taken as false, as in the road port).
* Against the original: the rivals' totals with a *driving* player were not
  measured under Wine (the November report triage's 8 native runs had the player standing, so the
  traffic, placed round the player, never met the rivals). How often native
  rivals are totaled in the first drag stays **open**; the clone's rate is
  now 8 / 24 runs with this bot.
* The bot's player is totaled in about 10 of 24 runs (its lane changes are
  crude); not a rival matter.

Evidence: `docs/evidence/first-drag/` (`summary-before.txt` /
`summary-after.txt`: the 24 runs' per-rival records; `flip-before-*`: the
per-tick rows of the flipping total above).
