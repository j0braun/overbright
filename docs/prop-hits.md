# Knock-over prop hits: sound and behaviour against the original

The report: "hitting the yellow cones on the highway plays way too loud audio,
not sure if they behave like native at all". The "yellow cones" are the highway's yellow
impact barrels, kind `ZCS_Barrel_A` (L4RA: 227 placements, mostly in groups of 3–5 at the
gores; info: mass 0.9 t, inertia diagonal from the info, break impulse 0, restitution 0.005,
friction 0.3 held, material `WATER_DRUM`). The group measured is the one at 379.4 −1477 35.8
(3 barrels in the car's line). `devprobes prop_census` lists every kind and placement.

## 1. What the report describes, and why (first round of measurements)

Each barrel hit plays two things (both natively and in the clone):

* the impact: the collision sound object of class 2 (a prop in the pair, vtable 0x78BB08) plays
  a stitch record of `MB_Stich_Collision.abk` chosen by the volume byte (0x47A550:
  |J| × the prop's 1/m on the 0x479A20 curve; kind 7 with a car, 8 against the track; the
  WATER_DRUM group, `collision::prop_index`) at the start volume +0xC0, which SFXObj_Collision
  (0x461CE0) writes from mix-map section 5's level of the kind's slot at the object's point
  (slots 7–8 −800 mB: 12999 at 1 m) — already ported and verified (formats.md, Impact sounds);
* the water splash: a WATER_DRUM knocked loose (+0x505) and not yet splashed (+0x50C) starts
  three FX_ROADNOISE_TRANS requests (variant 3, ROADNOISE_00_MB sample 14, 1.99 s) when its
  object starts (0x46AE00 = vtable 0x78BB08 slot 8, called by the queue's flush at 0x4818A6),
  and each frame (0x481FD0, slot 9) sets p1 = clamp(+0xC0 · [0x7ECF90] >> 15, 0, 0x7FFF)
  ([0x7ECF90] = 17000), p2 = 2000 / 4000 / 7000.

**The clone's splash was 8 dB too loud** (`src/audio/world/env/splash.rs`, before: `level =
volume_byte · 0x7FFF / 0x7F · 17000 >> 15`, marked inferred): it took the impact's volume
byte scaled to 0x7FFF for +0xC0. Natively +0xC0 is the object's start volume — the same mix
level the impact's layers take (12999 at 1 m, less with distance), so a 127 hit splashes at
6743, the clone's at 16999 (+8.0 dB). The barrels' hits are always loud enough for byte 127
or close (|J| / m over 7.5 m/s), so every barrel hit in the clone splashed at about full
scale, three voices of a 2 s splash per barrel. And the clone started the splash outside the
five-object cap (it posted it from the physics), where natively it rides on the object's
start (0x4818A6 → 0x46AE00) and is dropped with it.

Also: natively +0xC0 is rewritten every frame from the object's copy (0x461CE0) and every
playing layer is set to its volume × V >> 15 again (0x47BB30 → 0x46CF70), the splash's p1
too (0x481FD0) — read under Wine: V is 0 on the start frame, 12999 next to the hit (the
clone's start level, verified), then falls as the car drives away (12615 at 4.6 m, 10912 at
7.4 m, 7126 at 12 m, 2365 at 21 m from the car). The clone held the first pass's level for
the layers' whole length (up to 2 s).

**Fixed** (all in the Original path, no option):
1. `collision::splash_level`: p1 = V · 17000 >> 15 with V the object's +0xC0.
2. The splash rides on the impact (`Impact::splash`) and starts with its object in the
   queue's flush (`Pool::flush_started`, `Game::mix_levels`), so the five-object cap drops it
   with the object.
3. A started object keeps its mix copy while its layers or splash play (`ImpactLive`) and
   follows the copy's level each frame (layers' gain, the splash's p1).

## 2. Measured (original under Wine, clone headless)

Hit loudness: the mix's loudest 50 ms RMS in the second after the first hit, and against
the engine (the RMS over the 0.5 s before it). Raw lines: the maintainer's private capture.

| speed | original (dBFS / vs engine) | clone before | clone after |
|---|---|---|---|
| 60 km/h | −12.5 / +4.5; second run −11.6 / +17.0 | −5.5 / +15.7 | −9.4 / +11.9 |
| 120 km/h | −14.9 / +7.4 | −7.8 / +21.4 | −11.3 / +17.8 |
| 200 km/h | −13.5 / +21.1 | −5.2 / +25.9 | −8.3 / +22.8 |

The "vs engine" column mixes two things: the original's engine level just before the hit
depends on the run (it kicked down to second at 60 and 120: −17 / −28.6 / −22.3 / −34.7 dBFS),
the clone's shot rolls in a fixed gear (−21.2 / −29.2 / −31.1 dBFS); the absolute level is the
better comparison. The fix takes 3–4 dB off the clone's hit; the clone is still 2–5 dB above
the original's, and its peak still reaches 0 dBFS (the original's −0.9 … −5.2). What remains
is not the sound law: the impact's start level (12999 near, both), its layers (the same
stitch records, layer volumes 3700–5700 both) and their per-frame level now match; the
clone's car pushes the barrels straight ahead and meets all three within 0.17 s (three
11-layer kind-7 records at full level on top of each other), where the original's car
deflects them and meets them 0.2–0.4 s apart (section 3).

The splash's p1 read in the original: 4240–4714 (V 8180–9090, a drum a few metres off);
the clone before: 16999 for every hard hit; after: V · 17000 >> 15 of the clone's level
(6743 at the hit, then following it).

## 3. The props' behaviour

Car speed loss per barrel contact (the frame's drop, km/h):

| speed | original | clone (same before and after: no physics change) |
|---|---|---|
| 60 | −4.4, −5.0, −7.2; second run −5.0, −9.4, −10.9 | −11.5, −10.8, −10.8 |
| 120 | −10.8, −25.3 | −8.0 (one barrel met; the first drop is the shot's roll-in) |
| 200 | −17.2, −42.9, −11.7 | −16.3, −36.0, −11.2 |

Barrels (distance travelled / highest lift / time to rest):

| speed | original | clone |
|---|---|---|
| 60 | 4.0 / 0.13 m / 1.5 s, 5.7 / 0.13 / 2.0 (one barely moved); second run 2.7–12.1 m, ≤ 0.23 m, 2.0–3.0 s | 11.1–14.9 m, ≤ 0.24 m, 1.3–1.4 s |
| 120 | 2.4–8.4 m, ≤ 0.12 m, ~2 s | 11.3 m, 0.12 m, 1.2 s |
| 200 | 11.9–35.1 m, ≤ 0.23 m, 2.5–3.7 s | 18.5 m and 81.6 m (lifted 1.76 m) |

Both tip the barrels over within ~0.5 s and let them roll; the car's loss per contact is of
the same size at 200 km/h and up to twice the original's at 60. The original's barrels leave
at 60–70° off the car's heading at 60 km/h (glancing), the clone's straight ahead (98°), and
the clone's car keeps pushing them (three to five contacts per barrel), so they travel 2–3×
as far and, at 200 km/h, one was thrown 1.76 m up (the original's never rose over 0.23 m).
The contact law itself is the verified one (0x5AF0F0 with the props' unturned inverse
inertia, drag-stage4-parity §7.1); the barrels' volume is the `ZCS_` one (48 points,
radius 0.76 / 0.66, half height 0.98); a lying barrel's centre rests 0.65–0.7 m above the
road in the original and 0.85–0.95 m in the clone. Not fixed: no native rule was isolated
(section 5). Respawn: none while the track is loaded in both (the made-already bit,
0x86AD78; formats.md "Knock-over props").

## 4. Method and tools

* Original: the maintainer's tool `native.py` with the scenarios `prop_barrels_{60,120,200}.toml`:
  free roam from the stage-2 save, the player teleported 23 m before the barrels; the
  original holds a teleported car still for ~20 frames and puts it back on the resume's
  heading, so at frame `speed_at` (30) the game is stopped (SIGSTOP) and the car set rolling
  (`[start] speed`, `gear`, `ratio`: body P and the car's velocity, the rows, the wheels'
  spin, the gear and the shaft and engine speed matched), throttle held, 300 frames at the
  cave's fixed 1/60 s; the mix to `native.wav` (fastgpu `--audio-file`), the prop bodies per
  frame through /proc (the maintainer's tool `proppoll.py`: the pool [0x86B374], vtable 0x7A26A0,
  rigid body +0x68: +0x20 position, +0x70 P, +0x168 1/m; +0x506 moving, +0x508 settle).
* Clone: the explore shot at the same place and speed in the same gear
  (the maintainer's private capture: `NFSU2_SHOT_AT`, `NFSU2_FX_SPEED`,
  `NFSU2_FX_GEAR`, `NFSU2_AUDIO_WAV`, `NFSU2_PROP_LOG=1`), and the parity flow
  (`parity.py clone`, the original's car state put on the clone's two frames before the hit,
  `[run] transplant`).
* the private tool `prophits.py OUT` / `scripts/clone_shot.py`: the hit frame (the car's first
  speed drop over 0.4 m/s a frame), the mix's RMS over the 0.5 s before the hit (the engine),
  the loudest 50 ms RMS and the peak of the second after it; per prop the distance travelled,
  the height reached and the time to rest.
* Check aid: `NFSU2_PROP_LOG=1` prints each tick's car (`PROPTICK`), each car–prop contact
  (`PROPCAR`), each record's first push and whether it plays (`PROPSOUND`), each woken prop
  (`PROPWAKE`) and each moving prop's pose (`PROP`), with the step count.

## 5. The contacts, traced end to end (second round)

Measured with a contact log inside the original (the maintainer's tool `contactlog.py` + `.s`, `[run]
contact_log` or `PARITY_CONTACT_LOG=1` in `native.py`: hooks on 0x5AF110's call of 0x594010,
on the prop body's vtable +0x50 and on the three calls of the pair query 0x5B3D30), and
compared per quantity with the maintainer's tool `propcompare.py` (native run dir or clone log).

**Rules (traced; the clone's code in `src/smackable/`):**
* *Shape:* the barrel's `ZCS_` volume (48 points), the car's `<CAR>_CV` hull; one convex
  distance query (0x5B33A0 / 0x595050, `narrow`). Rotation and placement of the barrel agree
  with the original's (pair transforms +0x120 / +0x1A0 read under Wine).
* *Pairs:* every body with +0x12 and +0x19 (a car, or a prop once 0x5ADFA0 has started it)
  sweeps (0x5FFB10, and again after each contact it takes part in, 0x5FFC78 → 0x5B23C0): the
  bodies whose x/y box meets its own pair with it (0x5A0350), an unhit prop woken first
  (0x5B3200). **So a moving barrel wakes and hits the next one** — the clone had no prop–prop
  pairs (fixed). 0x5B5060 makes no contact of two bodies both at rest (vtable +0x6C).
* *Point and normal (0x594010):* the midpoint of the pair's closest points +0xC0 / +0xD0 and
  their gap (two bodies with +0x12; under 1e-6 apart the positions' line, 0x787594). 0x5B2620
  moves both bodies back to the moment, applies the push-out offset and calls 0x5AF0F0 **without
  a new query**: the points are the last query's (time of impact 0x5B4BF0, push-out 0x5B4300 or
  detection) — read under Wine as 0.003 m apart at 60 km/h and 0.07–0.23 m at 200 km/h. The
  clone measured them again at the moment (fixed: `PairCache::last`), and its car–prop
  queries now pass the pair's 1 mm dirty test with the bodies' positions (0x598630).
* *Impulse (0x5AF0F0, unchanged):* each side's 1/m (vtable +0x5C; a prop's own 1/m, 0 while
  its break impulse is left; a car's min(1/m, 1/1.85)) and body-space inverse inertia unturned
  (+0x60; × 0.4 for a car out of crash mode); e and μ the larger of the two (+0x70 / +0x74,
  0x5941D0: car 0.1 / 0.3, barrel info 0.005 / 0.3); j ≥ 0.3 / the larger 1/m; friction μ·j ×
  min(slide, 1); B takes J, A −J (vtable +0x4C = 0x5B2080 for a prop: starts a resting one,
  takes its break impulse; then +0x50); a car pressed by a prop against a wall gets its z × 0.25
  (0x5AF8D8). Prop–prop: both props' terms, kind-8 sound on A's object (0x592D90 picks A,
  0x4598B0 gives 7 only with +0x128 bit 0x10). The level arm (0x5AF7B8 / 0x5AF854) needs record
  +0x10, set only by a car–car crash, and applies to car bodies: never for a prop pair.
* *Repeats:* at most 2 contacts per pair a frame (0x802084), 20 a frame (0x802080); no
  cool-down or class change after a hit (+0x14 classes are the body's, 0x86B0B0 table).
* *Flight and rest (unchanged, as ported):* Euler parts of 1/60 s, gravity 9.8, angular and
  rest damping, settle test 0x59E440 (0.95 damping under 2 m/s), stop 0x5A8DA0.

**The original's early-contact regime (open, not ported).** The hull support 0x5AFBB0 ranks
the vertices less the shape's offset +0x40 (the centre-of-mass offset, set by 0x5B5060), but
returns the raw vertex through the transform (0x5AFF2A, call 0x43C9C0, overwriting the corrected
point); vertices kept from the previous query are placed again correctly (0x5AAD30). Read under
Wine (PEUGOT, run n200e): the original's answers 0.564 m and 0.153 m are hull vertex 36 at
+R·(0.3, 0, 0.3) — the PEUGOT's centre of mass — while its other answers equal the clone's
query on the same transforms to 0.1 mm. With the warm-started vertex sets this makes the
original see an overlap where the shapes are still 7–8 cm apart, then resolve the contact
with the stale points: at 200 km/h in 4 of 5 runs (n200a/b/d/e) the first barrel leaves along
the heading (+7.5°) at ~26 m/s, knocks the other two, and the car loses 40–45 km/h; in n200f
(and n120a, all 60 km/h runs; n120b/e are early) the contact is clean and the barrel leaves 28–32° right, the car
losing 11–13 km/h — the regime the clone reproduces. Emulating the support offset alone in the
clone's car–prop queries (tried) gave the early contacts at 200 km/h but also 0.3 m early ones
at 60 km/h that the original never shows: the exact port needs the vertex-set history of the
query (+0xF0 / +0x104) and stays open. The car–track and car–car queries use the same support
natively (not examined).

**Measured** (native: runs n60a–c, n120a/b/e, n200a/b/d/e/f of `prop_barrels_*.toml`; n120c /
n200c left out: the car's line missed the group. Clone: the explore shot, `run_clone.sh`,
before = without this round's port, after = with it). Barrels in order along the road; "gap" = first touch
of barrel 2 / 3 after the car's first barrel contact; angle = the barrel's first motion off the
car's heading (− = right); speed drop over the frame of a car contact:

| | 60 native | 60 before | 60 after | 120 native | 120 before | 120 after | 200 native | 200 before | 200 after |
|---|---|---|---|---|---|---|---|---|---|
| angle, barrel 1 | −29.1 … −28.1° | −30.3° | −30.3° | −27.6 / +7.5° (×2) | −32.0° | −30.6° | −32.0 / −4.6 … +7.6° | −33.4° | −33.1° |
| angle, barrel 2 | −23.8 … −23.0° | −25.7° | −24.2° | by barrel 1: −20.7 … +12° | car, −52° | by barrel 1 | by barrel 1: −18 … −4.7° | not touched | by barrel 1 |
| gaps | 0.12–0.15 / 0.28–0.35 s | 0.13 / 0.28 | 0.13 / 0.30 | 0.05–0.13 / 0.13–0.37 | 0.18 / 0.22 | 0.15 / 0.22 | 0.03–0.12 / 0.07–0.08 | — | 0.15 / — |
| peak rise | 0.07–0.16 m (first round ≤ 0.23) | 0.13 | 0.14 | 0.12–0.29 | 0.31 | 0.13 | 0.06–0.46 | 0.30 | 0.31 |
| distance, barrel 1 | 3.96–4.61 m | 4.21 | 4.22 | 7.6–10.1 | 12.3 | 11.6 | 17.4–33.0 | 23.1 | 22.2 |
| distance, barrel 2 | 5.54–7.34 m | 6.39 | 6.30 | 0.3–9.8 | 0.47 | 0.16 | 6.9–16.0 | — | 0.0 |
| time to rest | 1.23–2.10 s | 1.28–1.73 | 1.30–1.62 | 2.0–2.8 | 2.48–3.52 | 2.25 | 2.9–3.8 | 3.45 | 3.25 |
| rest height (lying) | 0.64–0.73 m | 0.60–0.72 | 0.59–0.66 | 0.70–0.91 | 0.79–1.24 | 0.66 | 0.82–1.07 | 0.85 | 0.82 |
| car drop / barrel | 5.2–5.8, 6.8–7.2 km/h | 4.7, 6.0 | 4.7, 6.4 | 11.3 / 27.2–27.5 | 8.9, 15.0 | 9.3 | 13.1 / 39.6–45.2 | 11.5 | 11.8 |

At 60 km/h the clone was already in the original's ranges (its first contact is clean, normal
0.73 / 0.68 against the original's 0.71 / 0.69, |J| 4.5 against 4.4); at 120 and 200 km/h it
matches the original's clean-contact runs (n120a, n200f) and not the early-contact ones. The first round's
"straight ahead, 82 m, 1.76 m up" clone run does not reproduce without this round's port (the shot's
roll-in changed since).

**Hit loudness** (loudest 50 ms RMS in the second after the hit, dBFS): original −16.7 / −15.3 /
−15.7 (60), −15.9 / −15.1 / −16.1 (120), −17.2 / −15.9 / −14.6 / −14.8 / −14.2 (200); the first round's runs
−12.5 / −11.6, −14.9, −13.5. Clone before −9.7 / −10.7 / −11.5, after −9.1 / −10.9 / −11.5.
**Not met** (4–7 dB over these runs). The physics now makes the same hits at 60 km/h (same
contacts, |J| and volume bytes 82 / 98 natively against 84 / 106), so the gap is in the level:
in every second-round run (and the first round's n200) the original's collision objects start at V = 4617
(kind 7) and 5812 (kind 5), constant while the car drives away, against 12999 / 16365 in
the first round's n60b and in the clone — −9.0 dB of section 5's level that changes between boots of
the original. Which state of the mix map sets it is **open** (answered in section 6: Rachel's
queued hint line playing; without it the gap is gone at 120 / 200 km/h and +2.5 dB at 60).

## 6. The start level: Rachel's line ducks the hits (third round)

**Rule (traced, verified under Wine).** Section 5 of MAPOUTPUT (the collision objects) has three
masters (slots 0–5: master 0, −600 mB; slot 6: master 1; slots 7–8: master 2, −500 −300 mB),
each fed by its 3D control and sub channel 0; sub 0 sums section 0's event controls 0 and 2
(inputs 0xB2003000 / 0xB2003002, clamped −10000..0). Event control 0 is the sound hold (−10000
mB on MasterVol f6); event control 2 is **−900 mB × MasterVol f10 (the speech volume) while
SFXObj_Speech f1 = 1** (0x4751A0: the speech stream plays, 0x45A550 status 2, outside game
state 3 — free roam; f0 in a race takes other ducks). So while Rachel speaks every collision
object starts 900 mB down: 12999 → 4617 (kinds 7–8), 16365 → 5812 (1–5), and still falls
with distance (read: 4617 → 4153 as the car leaves). No other input of section 5 differs.

**Why the boots differed (measured).** The stage-2 save the scenarios load
(`stage1_win_streetx.sav`) carries a queued Rachel hint: career 0x8635BC +0xA8 slot {4000
ticks, kind 5} (posted at 0x52438E after a won event at stage 1, saved with the career).
0x551330 fires it 1 s of free-roam driving after the resume and a ~15 s line plays
(the maintainer's tool `speechpoll.py`: f1 = 1 from clock 12873; in run q120 back to 0 at 74133). The
scenario arms 25–40 s of wall time after the boot, so the hit fell inside the line in every
second-round run (4617) and after it in the first round's n60b (hit at clock 72000: 12999).

**Both levels at will.** `native.py` default: the line plays through the run → 4617 / 5812.
`PARITY_SPEECH_QUIET=1` (or `[start] speech_quiet = true`): the run is armed only once the
speech object's f0 / f1 are 0 and no hint is queued → 12999 / 16365 in all 9 runs (u60–u200c).

**The clone** already follows the rule: `mixmap::Scene::sfx` carries SFXObj_Speech f0 / f1
(`Game::mix_levels`, `Streams::speech_playing`) and the generic section 5 sums the same
inputs; test `mixmap::tests::impacts_duck_while_rachel_speaks` (4617 / 5812 exactly). No code
change: the clone's 12999 was the right level for the comparison it was measured in (no line
playing). **What a player normally hears: 12999** — the duck applies only while one of
Rachel's lines plays in free roam (a hint, an outrun line), in the original and the clone alike.

**Hit loudness, re-measured** (loudest 50 ms RMS in the second after the hit, dBFS; raw:
the maintainer's private capture; `prophits.py --power` = per-channel power,
the same within 0.1 dB, so the original's panning is not a factor):

| speed | original, no line (12999) | original, line playing (4617) | clone (12999) | clone − original |
|---|---|---|---|---|
| 60 | −11.6 / −11.9 / −12.0 | −15.0; second round −16.7 / −15.3 / −15.7 | −9.1 | +2.5 … +2.9 dB |
| 120 | −11.4 / −10.7 / −11.1 | −15.9 / −15.1 / −16.1 | −10.9 … −11.5 | −0.8 … +0.5 dB |
| 200 | −10.4 / −9.8 / −11.4 | −17.2 … −14.2 | −11.5 | −1.7 … −0.1 dB |

So the "4–7 dB louder" of section 5 was the original measured under Rachel's line. Left: at 60
km/h the clone's second barrel hit has volume byte 106 against the original's 87–100 (the
contact's |J|, physics — **inferred** cause of the +2.5 dB); at 200 km/h the original's hits
are the early-contact ones (the car loses 90–104 km/h against the clone's 37), section 7.

## 7. The early contact: the query's vertex history, walls and cars (third round)

**Traced (0x5B3D30 → 0x5B33A0, 0x5AFBB0, 0x5AAD30 / 0x5A1980).** The pair object keeps per
side up to four vertex indices (+0xF0.. count +0x100 for A, +0x104.. count +0x114 for B), their
points (+0x40.. / +0x80.., 16-byte stride) and a warm flag +0x118. Cold (+0x118 = 0): both start
at vertex 0 (0x5AAD30, offset-corrected), A's first point is A's support along B0 − A0 and B's
along the reverse; the pair turns warm only if they are ≥ 1e-5 apart (else 0 returns with the
flag still clear). Warm: the kept indices' points are rebuilt (0x5AAD30 → 0x5A1980: vertex −
shape offset +0x40, then the body transform +0x120 / +0x1A0: **correct**), 0x595050 measures
them; under 1e-5 → overlap at once, else the loop: new supports from 0x5AFBB0 (ranked with the
offset removed, but the point written is the raw vertex through the transform: **R·c off**),
a support index already in both sets ends it, a new one is appended (≤ 4, 0x595050 reduces the
sets, 0x598250 restores the previous sets when the distance did not shrink). The flag is
cleared only by 0x5B3D30 when a type-2 side's +0x1D0 / +0x1D4 state flips; otherwise the
history lives as long as the pair (the box-overlap pair of 0x5A0350). So an error appears in
the query of the frame in which a vertex is new to the set, and is gone the next frame.
The offset flag +0x3E / +0x40 is set per shape by 0x5B5060 (both type-1 shapes of a detected
pair: the body's vtable +0x1C, the centre-of-mass offset), 0x5AEDE8 and 0x5A191A.

**Measured** (contact log with the shapes, `PARITY_CONTACT_ALL=1` for every pair;
the maintainer's private capture): the PEUGOT's hull carries flag 1 and offset
(0.3, 0, 0.3) in its **car–track** queries too (shapes 0x00010201: hull vs polygon), and the
barrel's own hull flag 1 with offset (0, 0, 0.946) — larger than the car's, so a barrel's new
supports sit 0.95 m along its axis. Wall hit (crash_wall, 15 m/s, normal approach 0.244 m a
frame): the car–wall pair's first (cold) query answered 0.365 m where the next ones (0.407,
0.175) put it at ~0.66 m — 0.3 m short, the R·c error — but it did not touch; the overlap came
the frame the true gap went negative (0.175 → −0.07), i.e. **not early** in this run, which
agrees with crash_wall_tp (the clone hits on the same frame, at rest 0.05 m apart, world-
systems-parity.md §2). Car–car: **not measured** (no scenario with a car–car touch at speed).
Inferred: a frontal wall hit early by up to 0.3 m needs a fresh support in a query that would
otherwise be ≤ 0.3 m clear — at 15 m/s that is the cold query only when the pair forms
within 0.3 m; at high speed (0.93 m a frame at 200 km/h) more likely.

**Not ported (design).** An exact port is local to `narrow::query`: give each side an optional
world offset R·c (per shape: the car's centre-of-mass offset, a prop's own, 0 for polygons),
add it to the point of every support taken in that call (the cold start's first supports and
every loop support), never to points rebuilt from kept indices; keep +0x118's rule (warm only
after a ≥ 1e-5 cold answer, cleared only on the polygon state flip). Because the car's hull is
flagged in its track queries too, porting it for car–prop alone diverges from the original's
car–wall queries; porting it for all changes every hull contact (sim hashes everywhere). For
the record: the second round's try offset the car only; the barrel's 0.946 m offset was not known then.

## 7a. The support offset, ported (all hull queries)

**Traced.**
* *Who sets the offset* (+0x3E flag, +0x40 vector, type-1 hull shapes only): the hull list loader
  0x5A1760 clears both for every `_CV` / `ZCS_` volume (0x5A183B); the lookup 0x5A18D0 (key, vec)
  sets them from `vec` when given (0x5A191A). It has three calls: the car collision body's
  constructor 0x5AEC30 (0x5AED2F `<CAR>_CV`, 0x5AED5C a fallback name; `vec` = 0x5ED410, the centre
  of mass: rigid body +0x1F0, else the car's +0x20 record), which then writes the car's +0x20 record
  again (0x5AEDE8); and the prop rigid body's setup 0x5B1550 (0x5B1690: `vec` = rigid body +0x1F0,
  the info's centre of mass). Before each detection 0x5B5060 writes each type-1 side's offset from
  the body's vtable +0x1C (0x5B514C / 0x5B517F, flag = 1): the car body 0x593B70 → 0x5ED410, the prop
  body 0x5B1C00 → rigid body +0x1F0. So the offset is always the body's centre of mass. A car without
  a `_CV` volume and a prop without a `ZCS_` volume get a box (type 3, 0x598770; 0x5AED76 / 0x5B17D5),
  which has no offset; polygons (type 2), points (4) and type 5 have none. No wheel shape is a hull of
  this lookup.
* *The support* 0x5AFBB0 ranks the vertices less the offset (0x5AFC..: +0x20 cached dot) but writes
  the raw vertex through the body transform (0x5AFF2A overwrites the corrected point of 0x5AFEDB);
  0x5AAD30 → 0x5A1980 (rebuilt kept vertices) subtracts it. World error: +R·c.
* *The pair state* (0x5B33A0): per side up to four kept indices (+0xF0 / +0x104, counts +0x100 /
  +0x114), points (+0x40 / +0x80), warm flag +0x118. Cold: vertex 0 of each (true) gives the first
  direction; both first supports raw; their indices and points are written and +0xC0 / +0xD0 set; only
  a squared gap over 1e-5 sets +0x118 and runs the loop, else 0 returns with +0x118 clear (the next
  call is cold again). Warm: kept points rebuilt true, 0x595050; over 1e-5 the loop, else 0. The loop
  appends a support whose index is new (raw point), 0x598140 / 0x598250 save and restore the sets
  (+0x40..+0x118) around a step that does not shrink the distance. +0x118 is cleared only by
  0x5B3D30 on a type-2 side's +0x1D0 / +0x1D4 flip. The clone's `PairCache` holds the same: kept
  indices per side, cleared to cold the same way (`narrow::query`).
* *When a pair forms* (0x5A0350 from the sweep 0x5B23C0): when the bodies' boxes meet (0x592970), a
  box being the body's position ± its radius in x/y (0x5A9DF0). The car body's position is its centre
  of mass (vtable +0x08 = 0x593B50: rigid body +0x20, else car +0x60); its radius +0x1C and heights
  +0xD0 / +0xD4 come from 0x5AE530 over the shape's vertices as 0x5A1980 gives them, i.e. about the
  centre of mass (0x5AEC30 sets the offset first). **The clone's car box was about the car's origin
  with the reach from the origin** (PEUGOT: 0.58 m short at the front): its car–prop pairs formed a
  frame late, and with the offset ported the first (cold) query then came 0.30 m from the barrel and
  made an early contact at 60 km/h the original never shows. Ported: `Car::body_reach`, the box at
  the centre of mass for the car–prop sweep, the car pairs and the car's track gather.

* *The support's climb* (0x5AFBB0, traced from the asm 0x5AFBF1–0x5AFF50): the direction is turned
  into the shape's frame by the pair's inverse rotation (+0x160 / +0x1E0 = 0x43C9D0's transpose of the
  body's rotation, written by 0x598680; its fourth row, read under Wine, is 0: no translation), so
  the ranking equals the world's. From the vertex the side's buffer last reported (0x59FB60 clears
  both buffers, the cold start's 0x5AAD30(0) gives vertex 0; a warm call's rebuild loop leaves the
  last kept vertex; then each support's own) it takes that vertex when none of its links (record +8
  count, +0xA first into the `0x39202` list, aligned to 16 bytes, 0x5A1760) is strictly greater; for
  hulls of over 16 points the start queues its greater links and the first popped one that is such a
  local maximum wins (only the first vertex queues: the flag then reads 0x86B2D8, never written);
  a vertex with no links is skipped; failing all, a scan takes the first of the greatest. It reports
  the record's +0xC: the vertex's own index, or 0 for an unlinked one (PEUGOT_CV 12, 18, 30, 39, 40;
  34 of ZCS_Barrel_A's 48, all duplicates), which then enters the set as vertex 0 and is rebuilt as
  vertex 0. Each call draws one number from the shared generator 0x7EA624 for its visit stamp
  (0x43C1C0(0xFFFF) + 0x43BDF0): **not ported** (open: the draw shifts the shared generator).
* *Pair creation* (0x5A0350) clears the warm flag (+0x118 = 0): every pair starts cold.

**Ported** (`narrow::Side` / `narrow::Raw`, `narrow::query`, `HullLinks` from the volume parser,
`Car::hull_links`, `Prop::links`): every support taken in a call carries its side's R·c and is
the climb's vertex (with its reported index); points rebuilt from kept indices are true; for all
hull queries — car–track, car–prop, car–car, prop–prop, prop–track — detection, time of impact and
push-out alike. Replayed against the original's logged queries with the pair's kept sets
(`tests/track_l4ra.rs`, the contact log now writes +0xF0..+0x118): car–barrel at 200 km/h, 25
queries, and at 60 km/h, 20 queries, every gap within 0.2 mm and every kept set identical; car–car
19 of 19 within 0.05 mm. (Past those, a few 60 km/h time-of-impact queries flip between 0 and
3.2 mm: the cold / warm test is a squared gap against 1e-5, √ = 3.16 mm, where the original's
float32 and the clone's double part.) The earlier run without logged sets (12 queries, 3 off)
is superseded: its sets are unknown.

**Measured** (original: `prop_barrels_*` speech quiet, contact log, runs t60 / q120 / s200; clone:
`run_clone.sh` at 60 / 132 / 225 km/h so that the hit comes at the original's speed; raw:
the maintainer's private capture):

| | original | clone before | clone after |
|---|---|---|---|
| 60: first contact | clean, n 0.70/0.70, byte 84 | clean, byte 84 | clean, n 0.73/0.68, byte 84 |
| 60: second car contact | byte 94 | byte 106 | byte 105 |
| 120: km/h lost in 1 s | 30.2 (the roll's stale target gear, section 7b: 14.9) | 5.2 | 2.2 |
| 200: km/h lost in 1 s | 95 (90–104) (section 7b: 57.1) | 32.1 | 55.4 |
| crash_wall_tp | frame 196, 14.9 → 0.48 | 196, 15.02 → 0.48 | 196, 15.02 → 0.48 |

`barrels_at_60_kmh_stay_in_the_originals_range` now starts from the original's logged state a frame
before its first barrel query (the scenario's start drifts there through the original's hold and
roll, which the test's start did not reproduce; its ranges are unchanged): first barrel −36°.

The shots' lines are not the original's (the clone's explore shot does not replay the hold and
roll): at 120 the clone's car meets the barrel group further right and pushes one barrel; at 200 it
meets barrel 2 first. The original at 200 (s200): the first contact is the early one (gap 0.230,
normal 0.155 / 0.985 along the heading), one impulse of |J| 23.85 that takes 46 km/h in one frame
(= J / 1.85: the car's capped mass), the barrel then knocks the second and third (prop–prop 7.0 /
4.6 / 2.8), the third meets the car again (|J| 6.1, 0.13 s). The clone's equivalent needs the same
line: open, a transplanted start (the parity flow's `pose` does not carry the gear yet).

**Car–car** (original, `car_rear_end_200`: the player rolled at 200 km/h behind an encounter car;
`PARITY_CONTACT_ALL=1`): the encounter car's pursuit steered aside — gaps 0.408 (cold) 0.088 0.120 …
1.749 m, no contact. The FOCUS's hull carries (0.19, 0, 0.27). A rear-end contact was not staged.

**Hit against the mix** (speech quiet, the save's option volumes [1, 1, 0.7, 1, 1, 1, 1]; no music
object in either game's event list; bed = RMS of the 1 s before the first car contact, hit = the
loudest 50 ms of the 0.3 s after it; power downmix):

| speed | original bed / hit / hit − bed | clone after |
|---|---|---|
| 60 | −18.3 / −11.9 / +6.3 dB | −22.8 / −12.1 / +10.7 |
| 120 | −23.9 / −11.5 / +12.3 | −25.8 / −11.7 / +14.1 |
| 200 | −17.9 / −10.4 / +7.5 | −23.9 / −9.1 / +14.8 |

The hit itself is the original's (−12.1 / −11.7 against −11.9 / −11.5; the first byte 84 / 118 /
127 against 84 / 127 / 127). The excess over the bed is the **bed**: at equal gear and engine speed
the clone's mix runs 3 dB under the original's (second gear, 3700–3800 rpm: −22.3 / −22.9 against
−19.3 dBFS; ~6050 rpm: −22.5 against −19.5): an engine / road level finding for the audio balance,
not a collision one. Impact sounds in the 2 s: the clone 5 / 3 / 6, the original's records begun
6 / 6 / 13+ (**inferred** from its contacts): the clone starts no more or longer sounds; the play
rule (record bit 0 or volume ≥ 10, one started sound per prop: 0x46A670 / 0x592D90; five objects)
is the ported one.

**What else moves** (parity scenarios with the original's records; mean / end distance of the
clone's car from the original's, m; base | query port alone | box alone | both):
crash_wall_tp 0.3 / 0.1 in all four (hit frame 196); crash_wall 2.5 / 3.7 all; drag_s1 90.5 / 267.5
all; circuit_s1 0.4 / 0.9 | 0.4 / 0.1 | 0.4 / 0.9 | 0.4 / 0.2; sprint_s1 4.9 / 16.6 | 4.8 / 16.2 |
4.9 / 16.6 | 4.8 / 15.7; camera_shake 7.9 / 19.0 | 7.1 / 17.1 | 7.9 / 19.0 | 7.8 / 18.8;
the outrun on the finished-career save 130.3 / 81.1 | 130.4 / 125.3 | 130.3 / 81.1 | 126.1 / 71.9; circuit_traffic 6.3 / 5.1 |
83.2 / 217.1 | 85.7 / 249.5 | 93.6 / 426.0 (first half 3.2 | 1.7 | 1.9 | 2.0). In circuit_traffic the
base car runs 1–2 s behind the original's from frame ~1300 (25 m at frame 1950) and so meets the
traffic later; with the port the car is on the original's spot at frame 1975 and then meets a
traffic car at step 2019 / 2027 (a car–car contact the port makes, 0.03 / 0.10 m apart without it)
— a traffic car the clone placed and drives itself, not the original's (traffic is not replayed).
The box change alone parts from the base first at step 2111 and leads to the same kind of stop.

**Car–track queries against the original** (stage-1 circuit, throttle only, the player's hull
against the track, `PARITY_CONTACT_ALL=1` with the pair's kept sets and the polygon's points,
19 226 queries; replayed with one pair history per pair): with the offset and the climb 19 119
within 0.2 mm and 18 414 kept sets identical; without the offset 16 000 / 15 393; the scan instead of
the climb changes 2 of them. The misses are the 3.16 mm cold / warm boundary (0.0033 against 0)
and a few time-of-impact steps after it. Hull–floor contacts happen in normal driving and are
resolved like any track contact (0x5B5060 / 0x5B2620, the floors' material, formats.md "Contacts").
The first query of the circuit run that answers differently between base and port is such a
floor touch (step 918, the circuit_traffic parting at frame 431; 6.8 mm apart in the base).
Track polygons are prisms in the original (0x5A1AA0: the face and its copy 1 m behind it,
0x7A1880, point count doubled; the support 0x5A2070 takes only the face while +0x1D0 is clear,
0x593000 sets it while the body is in front of the prism's centre +0x1F0; 0x5B3D30 clears the
pair's warm flag when it flips). The clone keeps the face only; in this run feeding the prism
points changes no answer (the back face is never the support toward the hull), so not ported (open).
**The body's box and the ground pack** (traced): 0x5AE940 gathers with the body's box (+0x20 /
+0x28: 0x5A9DF0's centre of mass ± reach) into the context at body +0x60, which is car +0x5C4
(0x5AEC30 passes it to 0x5A9CF0); the wheels' height query 0x5E72F0 uses the same per-car context
(formats.md "Height query"), and 0x5E7110 picks its pack at the box's centre with no slack. So the
centre-of-mass box choosing the floor cache's pack is the original's; in this run it first moves
the state at step 2111.

## 7b. The barrels from the original's state (full transplant)

**The earlier native figures at 120 and 200 km/h were the harness's.** The scenarios' roll
(`[start] speed`, native.py) wrote the gear into the gearbox (+0x3C) and car +0x434 but left the
gearbox's target gear (+0x38) at the launch's 1st. The automatic choice 0x5AA970 (code −1) only
moves +0x38 one gear from the current one when a shift is due, and the shift's Changing state takes
+0x38 (**traced**, 0x5AA9B0..0x5AAAE3; the clone's `Shift::Changing` sets `gear = target_gear`). So
the first down-shift after the hit went from 6th straight to 1st (frame 57 at 200 km/h: input
shaft 2040 rad/s at 147 km/h) and the engine braked the car: "200: 95 (90–104) km/h lost in 1 s"
and "120: 30.2" above are that, not the barrels. The roll now writes +0x38 as well.

**Same start state on both sides.** `[run] transplant_full = true` puts the original player's exact
state before frame F's update on the clone's (plan `stance_at`, `Plan::stance_at`): the body (centre
of mass, rows, v, ω, L), the suspensions, the rest class, the gear, engine and input-shaft speed,
clutch, the four wheel spins, the target gear, the shown rpm (car +0x440, which the automatic
choice reads; without it the clone shifted down at once from the transplanted 6th) and the shift
state. Three original runs per speed, identical to the printed digit (the lockstep is
deterministic); raw numbers the maintainer's private capture. |v| in km/h:

| | original | clone |
|---|---|---|
| 200: lost in the 1 s after the first drop | 57.1 | 57.5 |
| 200: position error at 0.5 / 1 / 2 s | — | 0.21 / 0.33 / 0.76 m |
| 120: lost in the 1 s after the first drop | 14.9 | 37.3 |
| 120: position error at 0.5 / 1 / 2 s | — | 1.84 / 4.71 / 10.69 m |

The first drop at both speeds is a track contact at (379.5, −1482.8), 3 m short of the barrels (no
prop query before it on either side); its response differs by about 5 cm/s between the games, the
first difference over float noise. At 200 km/h the run stays together through the barrels. At 120
km/h the barrel pair's cold distance query decides it: on the original's transforms the clone's
query answers the original's gaps (0.920 / 0.488 / 0.066 / 0 m over frames 68–71, within 0.05 mm),
but on its own transform at frame 68 — 8 mm and 0.003 in the rows from the original's — it answers
0.177 m, and two frames later the car meets the barrel with |J| 15 where the original's contact
comes at frame 71 and costs it nearly nothing. The query is that sensitive at this pose (**observed**
in the clone's query on both transforms; the original's own answer at the clone's pose is
**inferred** to be the same, as its query is the clone's on every logged transform). So the 120
km/h difference is the track contact's 1 % before the barrels, amplified by the query: **open**
(which track polygon, and why its response differs by 1 %).

## 7c. The explore bench's wall: a held car, and the hull points rebuilt as the original's

**What the bench does.** `--bench explore` (the PEUGOT of a new profile) holds full throttle from the
free-roam start for 1200 frames and then times its frames. The car runs straight east into a wall (the
face through (1716.625, −892.25) and (1724.125, −850.125), normal (−0.9845, 0.1753, 0)) at 14.2 m/s
at frame 204, square to it. It then stays pressed against it at 0–0.3 m/s for 7–8 s, its front edge
(`PEUGOT_CV` vertices 21 and 31, one above the other) on the wall, yawing slowly, until it pivots off
and drives south along the next road into the wall at its end (frames 1100–1200), where it is held
again. Builds before and after the support rule of §7a part at the first hit (4 mm, 5 mm/s at frame
204); the 1200 frames then end at another moment of the second hold: 30.1 m in the 300 timed frames
before, 7.1 m with the rule (the "held" car of the perf rebaseline), 153 m with the port below.

**The original holds there too (observed).** Scenario `bench_wall_hold` (stage-2 save, the same
PEUGOT; put at the clone's pose 64 frames before the hit, rolled at 31 km/h, throttle held 10 s,
contact log): in three runs the car meets the wall at frame 113–114 at 11.4 m/s, stays at 0.1–0.3 m/s
against it, turns −10.2° → −16…−18° by frame 360 and leaves it 7–8 s after the hit (2.0–3.7 m/s at
frame 540, −92…−98° at frame 600). 51–60 push-outs per run, all of one 5 mm step except two of 2 and 3
steps. Raw numbers: the maintainer's private capture. So the hold, its length and the
pivot are native; which way the car pivots follows the sign of its yaw rate at the hit. From the full
transplant at frame 50 the clone holds as long but pivots the other way in all three runs: at the
2nd → 1st downshift (frames 60–64) the original's front wheels pull unequally (fx −6.45 / −6.63, slip
−0.0086 / −0.0051) and its yaw rate turns from +0.0007 to −0.0014 rad/s; the clone's stay equal
(the drivetrain's split to the wheels is not carried by the transplant, parity-harness.md; **open**).
From the transplant at frame 100 (after the shift) the clone follows: 0.4 / 5.2 / 3.9 m apart at 10 s.

**Rule ported: the kept vertices are rebuilt as the original rebuilds them (traced).** 0x5B33A0's warm
start rebuilds each kept vertex with 0x5AAD30 → 0x5A1980 (the vertex less the shape offset +0x40, a
float subtraction) and the pair's transform +0x120 / +0x1A0 through 0x43C9C0 → 0x441000: per axis
((row 2·z + row 1·y) + row 0·x) + row 3, row 3 the body's position (the centre of mass), x87, stored
as floats. The clone built the hull as origin + R·v. At this wall vertices 21 and 31 differ along the
normal by about 2e-5 m, under a float step at x ≈ 1717 (1.2e-4), so the order of the sums decides
whether they tie. On a tie 0x595050 reduces the kept pair to the first (its arg-max takes a strictly
greater value only, 0x597959–0x597A71), the climb 0x5AFBB0 starts from the last kept vertex and
stops there (no strictly greater link), that vertex is then new to the reduced set (0x5B3B70) and
enters R·c off (§7a), and the query answers an overlap: the push-out 0x5B4300 steps on until the car
has moved 0.32 m back (8 tries). The clone did that in three of six transplanted runs, the original
never. Ported: `narrow::body_point` (0x441000's order), `Car::hull_points` (a `_CV` hull: vertex less
the offset, about the centre of mass; the default box as before), the props' `ZCS_` shapes
(`smackable::body::shape_at`). Afterwards one 0.32 m push-out is left in two of the six runs (n1 from
both frames), an exact tie under the original's order too; that the original makes the same push-out
on such a tie is **inferred** (same rules; its rebuilt points are not logged to the bit). Test
`track_l4ra::wall_push_out_rebuilds_the_kept_edge_as_the_original` (the logged pose and kept sets).
Not ported: a fresh support's point is the raw vertex through the same transform (0x5AFF2A) where
the clone adds R·c to the true point in double precision (a rounding of the same size, **open**).

**The bench's timed frames** now start from a standing start at a fixed place on the straight south
of the start, along the road (`EXPLORE_LINE`), after the same 1200 frames (the traffic fill): 45 m in
the 300 frames with and without the port. The perf baseline's explore configurations need re-accepting
(new scene); the images gate's explore runs (`img-explore-*`, `win-explore`, the enhanced explore runs)
differ from a base without this change by construction.

## 7d. The 120 km/h run, contact by contact

Measured with the contact log extended for this (`PARITY_CONTACT_ALL=1`: every pair's queries and
contacts, the player's rigid body before 0x5AF0F0 and, through a hook on 0x5B2620's call at 0x5B2794,
after it; `PARITY_CONTACT_EXACT=1`: exact floats; docs/parity-harness.md). The rigid body read there:
centre of mass +0x20, momentum +0x70, angular momentum +0x80, velocity +0xA0, ω +0xC0 (the PEUGOT's
mass 1.055: momentum / velocity). Raw numbers: the maintainer's private capture.

**First difference: the frame after the transplant (observed, open).** From the full transplant at
frame 59 the next frame already differs beyond float noise: vertical speed −0.9107 against −0.9173 m/s,
ω.x 0.0024 against 0.0058 rad/s, the compression rates by 0.005–0.025 m/s (the compressions within
0.4 mm). It decays (1.7 mm/s at frame 62) and shows at every speed (60 km/h: rates 0.04–0.11 m/s, ω.y
0.022 rad/s). Transplanting at frame 62, the frame right before the wall, changes nothing below, so it
is not what parts the runs; which state the transplant does not carry is open.

**The track contact is a wall, not a floor or a kerb.** The 3 m-before-the-barrels contact is the
car's right front against a vertical wall polygon (corners (385.25, −1494.63, 36.17) … (379.5,
−1478.88, 35.69), 4.8 m high, normal (−0.939, −0.343, 0); the original's polygon carries its 1 m prism,
+0x1D0 = 0: face only). The car drifts into it at 11.6 m/s along the normal; the first contact takes
9.4 of momentum and 3.5 of angular momentum about z.

**Rule ported (traced 0x5B2620 → 0x5AF0F0 → 0x594010).** 0x5AF0F0's point is the midpoint of the
pair's closest points +0xC0 / +0xD0 as they stand (0x594010), and 0x5B2620 measures nothing between
the time of impact's search (0x5B4BF0) or the push-out (0x5B4300) and the impulse. So the point is the
search's **last** query's, an overlapping one taken after the clear query that fixes the moment: here
2.3 mm further along the car's path than the pose the car is moved back to. The clone took the points
of a new query at that pose (its props already used the last ones, section 5); for track pairs it
now takes the pair's last answer too (`CarState::resolve_track`). Car–car contacts go through the same
0x5B2620 but the clone's `collide::cars::contact_point` still measures again: not ported (open) —
tried, it is unmeasured against the original and moves the traffic test
`a_parked_car_coasts_inactive_on_its_old_walk` off its path (no paused car reaches a world load).
With the original's state before the impulse, the clone's impulse from that point is the original's to
the printed digit: Δp (−9.400659, −4.422943, 0.027363), ΔL (0.483334, −1.005673, 3.494544) (test
`wall_contact_takes_the_searchs_last_points`, which also replays the pair's 15 queries from its first:
all within 0.03 mm). Before the port the first contact gave Δv −8.917 against −8.911 m/s and ΔL.z
3.487 against 3.494.

**Next difference: the second contact's moment (observed, open).** After the first impulse the car
is still against the wall at the end of the span; the search of the second contact queries at 1/2,
3/4, … of the 5.5 ms left. At 3/4 the original answers 0.003174 m — 12 µm over the query's warm
boundary √1e-5 = 3.162 mm, under which it answers 0 (overlap) — so its moment is 3/4 (event time
0.004137 s); on that exact transform the clone answers 0.003166 (also clear), but on its own pose,
0.1 mm away, 0 (overlap), and its moment becomes 7/8 (0.00485–0.00517 s). This contact is the
minimum push (0.3 m/s), but being moved back to a different moment leaves the car 25 % instead of
6 % of the way through that span's change: 0.03 m/s and 0.02 rad/s apart, and the original then makes a
fourth contact in the next frame that the clone does not. From there the runs part (frame 63: ω.y −2.77
against −2.59 rad/s). The original's gaps near the boundary come out quantised (0.003174 four times in
one run: its float arithmetic at world coordinates near ±1500, steps of 30–120 µm); matching them needs
the query in the original's float order: **open**.

**The barrel query's 0.920 → 0.177 (observed, explained).** Same frame (68), same barrel (the first
ZCS_Barrel_A), the pair's cold query. Its true distance is 0.426 m on the original's transform and
0.429 m on the clone's; the query, with the support offsets (the car's R·(0.3, 0, 0.3), the barrel's
0.946 m along its axis, section 7a), answers 0.920 (kept sets A {35, 36}, B {10, 0}) or 0.175 (A {36},
B {0, 32, 10}). Moving the clone's car from the original's transform toward its own, the answer stays
0.920–0.922 up to 55–60 % of the 8 mm position difference and then drops to 0.174–0.177; the 0.003
rotation difference alone keeps 0.918–0.920. So 8 mm of position decide which vertices the climb
reaches; both answers are the original's own rule (its query is the clone's on every logged
transform), the difference is the pose.

| km/h lost in the 1 s after the first drop (3 runs each) | original | clone before | clone after |
|---|---|---|---|
| 60 (transplant at 94) | −0.5 / −0.9 / 2.2 | −1.5 / −0.9 / 3.8 | −1.5 / −0.7 / −0.6 |
| 120 (transplant at 59) | 15.7 / 15.7 / 15.1 (section 7b's run: 14.9) | 7.8 / 7.8 / 9.1 (37.3) | 9.5 / 9.5 / 9.1 (36.9) |
| 200 (transplant at 50) | 72.0 / 44.3 / 57.1 | 77.3 / 41.6 / 57.5 | 77.3 / 41.6 / 57.5 |

The original's three runs per speed are **not** identical this time (they were in section 7b): the
roll reaches the transplant frame in a different state run to run (the 120 km/h wall drop at frame 62,
63 or 64). Each column pair is therefore one run: the clone takes that run's state. Position error 1 s
after the drop (m, before → after): 60: 0.32 → 0.32, 0.12 → 0.13, 0.33 → 0.64; 120: 2.32 → 1.41 twice,
1.32 → 1.30 (section 7b's run: 5.02 → 4.95); 200: unchanged (0.79, 0.53, 0.33).

## 7e. The query in the original's float order (fourth round)

**Traced (0x5B33A0, 0x595050 0x595050–0x598126, 0x5AFBB0, 0x5A2070, 0x441000, 0x43D1E0, 0x43CF10,
0x43D0C0).** The game runs the x87 at 24-bit precision (formats.md "Floating point"), so every x87
operation rounds as an f32 one; what decides the bits is the order of the sums. The query works on
world coordinates (not about a local origin). Ported in `narrow` (`closest`, `query`, the helpers
`xzy` / `zyx` / …, `cross`, `normalize`, `across`, `along`):
* 0x5B33A0: the cold gap and its test against the float 1e-5 (0x7A1934) as (x² + z²) + y²; the
  result √ of the last squared gap; an overlap leaves the pair's closest points +0xC0 / +0xD0 as the
  last call left them (a warm query that overlaps at once returned (0, 0) points before).
* 0x595050: dot products summed per site in their own order (most (x + z) + y, the first point of an
  arg-min (x + y) + z or (y + x) + z, the gaps (y + z) + x, the planes and inside tests (z + y) + x);
  the parameters t = ∓((d·w)) / (d·d); the axis across a segment as d × (w × d) (two 0x43D1E0 calls,
  not w − d (w·d)/|d|²); faces normalised by 0x43CF10 (√((x² + y²) + z²), one reciprocal); points as
  (d·t) + p; every candidate's squared gap (z² + y²) + x², a triangle's from the scaled normal (not
  (low − plane)²); the fallback replaced only when strictly nearer; an overlap leaves the sets as
  they are.
* 0x5AFBB0 (section 8's open item): the direction turned into the shape's frame by the transpose in
  0x441000's order, each vertex less the offset ranked by (z + y) + x of its products, and the
  fresh support's point the stored vertex through the body's transform (0x5AFF2A) — `narrow::Body`.
  The polygon support 0x5A2070 ranks (x + z) + y. 0x441000 itself: x = ((r2.x·z + r1.x·y) +
  r0.x·x) + t.x, but y and z = ((r2·z + r0·x) + r1·y) + t (`narrow::body_point` had x's order on all three).

**Bit for bit** (`devprobes` `query_replay L4RA PEUGOT native-contacts.tsv…`: every query of the
player's hull, a barrel's hull or a track polygon the pair measured, run from the pair's logged state
before it — kept sets, warm flag, the previous closest points — on the logged transforms; the contact
log now records the z rows and that entry state). Nine transplanted runs (60 / 120 / 200 km/h × 3):

| pairs | queries | all bits equal: before | after |
|---|---|---|---|
| car–track | 7 319 | 1 772 | 7 319 |
| car–barrel | 568 | 80 | 568 |
| barrel–barrel | 1 049 | 128 | 1 049 |
| barrel–track | 17 775 | 1 622 | 17 762 |

"All bits" = the distance (pair +0x10), both closest points and the kept sets. The 13 left are
overlapping barrel–floor queries against prism polygons (+0x1D0 set, all six points): distance and
points equal, the vertex last appended differs (e.g. B {3, 0, 1} against {3, 0, 2}); **open**. Among
the equal ones: the 120 km/h wall's 0.003174 (0x3B500000, 12 µm over the warm boundary), every time.
Test `track_l4ra::wall_and_barrel_queries_match_the_original_to_the_bit` replays 289 of them
(the maintainer's private capture: the 120 km/h wall pair and the barrels at all three speeds).

**In the game** the hull side carries its `Body` only in the car's track detection at the span's end
(`CarState::track_event`); the time of impact and push-out still blend the hull's world points and
the offset linearly between the span's poses (not the original's per-fraction transform: **open**),
and the props' and cars' pairs rank on world points with the offset added (f32).

**The table again** (`[run] transplant_full`, tyre state carried, contact log on; three original runs
per speed, each paired with the clone from its own state; km/h lost in the 1 s after the original's
first drop; clone position error at 0.5 / 1 / 2 s after it):

| speed | original | clone (base) | error (base) | clone (port) | error (port) |
|---|---|---|---|---|---|
| 60, ×3 identical | −1.3 | −1.4 | 0.04 / 0.08 / 0.17 | −0.2 | 0.16 / 0.39 / 0.87 |
| 120, ×3 identical | 15.0 | 34.8 | 1.91 / 4.39 / 9.64 | 34.4 | 1.88 / 4.39 / 9.57 |
| 200, runs 1 and 3 | 39.7 | 40.0 | 0.22 / 0.31 / 0.55 | 29.5 | 2.39 / 4.42 / 7.36 |
| 200, run 2 | 30.6 | 16.5 | 1.52 / 3.28 / 6.97 | 30.5 | 0.07 / 0.11 / 0.17 |

**The first frame after the transplant is still not equal within float noise** (observed, all nine
runs, the same with and without the port). Frame +1, original / clone (run 1 of each speed, the
recorded fields `c0.*`; ω = `rb_spin`, wheel spins `spin_*` rad/s, engine `eng_w` and input shaft
`in_w` rad/s, forces `fx_*` and `load_rl` kN, compression rates `crate_*` m/s):

| field | 60 km/h, frame 95 | 120 km/h, frame 60 | 200 km/h, frame 51 |
|---|---|---|---|
| vel.z | −0.173234 / −0.173227 | −0.910703 / −0.910532 | −1.76568 / −1.68687 |
| rb_spin.x | −0.0399222 / −0.045568 | 0.00238336 / 0.00107455 | 0.458766 / 0.407798 |
| rb_spin.y | −0.456428 / −0.435395 | 0.0360033 / 0.0360543 | −2.70786 / −2.61365 |
| spin_fl | 64.2531 / 63.7856 | 114.425 / 114.401 | 182.954 / 181.993 |
| spin_rr | 63.0785 / 63.0753 | 114.44 / 114.419 | 186.351 / 186.237 |
| eng_w | 455.381 / 454.296 | 589.789 / 589.632 | 589.758 / 588.849 |
| in_w | 455.381 / 453.834 | 589.824 / 589.697 | 589.758 / 588.13 |
| clutch | 1 / 1 | 1 / 1 | 1 / 1 |
| fx_fl | 5.05963 / 4.9949 | 1.21891 / 1.21917 | 10.1048 / 9.9615 |
| fx_rl | 0.115852 / 0.137501 | −0.204836 / −0.204985 | 13.4202 / 12.6998 |
| load_rl | 8.80153 / 7.937 | 3.79951 / 3.82247 | 24.3402 / 22.0827 |
| crate_fl | 0.0949148 / 0.100321 | 0.0717551 / 0.0797256 | 2.06936 / 2.12764 |
| crate_rl | −0.00622977 / −0.00346824 | 0.0788466 / 0.0859276 | 3.18928 / 3.12139 |

At 120 km/h v.y is also 6e-3 m/s apart; at 60 and 200 km/h the transplant frame falls in a landing
(rear load 8–24 kN). With the clutch locked, the original's engine and input shaft are equal (60, 200 km/h) or
0.035 rad/s apart (120), the clone's 0.46 / 0.065 / 0.72 rad/s apart. The
drivetrain is the first quantity apart, as in the cornering cases (docs/cornering-parity.md §4: the
clutch's lock-up, the engine's power factor); so the 120 km/h runs part before any contact, and the
query port does not change that run (34.8 → 34.4 against 15.0). At 200 km/h the runs fork at the
barrels either way (two of three paired runs closer before, one after). **Open**: the drivetrain
state at the transplant.

## 7f. Car against car: the contact's point (fourth round)

**Observed** (original under Wine, contact log with every pair; scenarios `carcar_rear30` and
`carcar_side150`, two runs each; contacts in one of each: 64 into a PICKUP from behind at 102 km/h,
13 into an SUV from the side at 150 km/h): in all 77 car–car contacts (body types 0 and 0) the point
0x5AF0F0 takes (esp+0x90) is to the bit the midpoint of the pair's +0xC0 / +0xD0 (0x594010:
(the second point plus the first) · 0.5), and those are the closest points of the pair's last logged query — the time of
impact's search in 67, the push-out in 5, the detection in 5. The same path as a track pair
(0x5B2620 measures nothing between them and the impulse). The replay (`query_replay … PEUGOT,PICKUP`
/ `PEUGOT,SUV`) gives every car–car query of both runs to the bit (825 and 270) and every one of
the 77 points. **Ported**: `collide::cars::contact_point` takes the pair's last answer
(`PairCache::last`), measuring only a pair with none. Test
`track_l4ra::car_car_contacts_take_the_last_querys_points` (one rear and one side contact,
the maintainer's private capture).

**Observed on the way**: after the side hits the original's PEUGOT hull offset (+0x40, written from
the body's centre of mass before each detection, section 7a) reads (0, 0, 0.3), then (0, 0, 0.3333),
(0, 0, 0.3666) instead of (0.3, 0, 0.3): the car's centre of mass moves after a crash. The clone keeps
the car record's; **open** (which code moves it).

**The coasting test** (`a_parked_car_coasts_inactive_on_its_old_walk`) waited for a paused traffic car
to meet a world load somewhere in a 210 s race; any change of the contacts moved the race so that
none did (0 coasting frames). What it asserts is the original's (observed,
`docs/evidence/traf-3/native-paused-coast.txt`: car +0x77D set and +0x550 clear throughout; mode 0 at
one place for 2.6 s; mode 2 — the TrafficMover, 0x5F6CE0 → 0x408C40 → 0x5F46B0(0x5F4410) — moving at
its last speed 20.39 m/s along its walk; mode 4 when the world unloads, 0x4022D0(0)). The test now
makes that situation 8 s after the green: two moving placed traffic cars paused as the placer pauses
them (0x4020D0, `pause_traffic`, walk and path kept), the world under one marked not loaded the frame
before (so the next stream step sees +0x5C0 go 0 → 1), the other left in mode 0; it asserts the same
(inactive and unplaced while paused; the first coasting on a mover, moving; the second not moving).

## 8. Open

* The contact refresh 0x593710 (before and after each impulse) runs the whole post-step 0x5B04E0 with
  h = 0 (components, the clutch machine 0x5A1410, the rest test, the force pass); the clone refreshes
  only the suspensions, after the impulse. Where the bench wall's runs part next
  (docs/cornering-parity.md section 6; the centre of mass is now kept exactly, as the original's).

* The original's front wheels pull unequally at a downshift while the clone's do not (§7c; the
  transplant does not carry the drivetrain's split): it sets which way a car pressed square
  against a wall pivots off.
* A fresh support's point (0x5AFF2A, the raw vertex through 0x441000) and the support's ranking
  are the original's only where the hull side carries its `narrow::Body` (the car's track
  detection, §7e); elsewhere the true point plus R·c in single precision, ranked on world points.
* Thirteen overlapping barrel–floor queries against prism polygons append another vertex than the
  original (§7e).
* The first frame after a full transplant: the drivetrain differs at once (§7e).

* The support's visit stamp draws from the shared generator (one draw per support call): not ported.
* The 120 km/h run from the original's state: the query now answers the original's 0.003174 on
  the original's transforms (§7e); the run still parts at the first frame after the transplant.
* The original's car centre of mass moves after a crash (§7f); the clone's stays.
* The frame after a full transplant differs (vertical speed, compression rates): an uncarried
  state, open (section 7d).
* The bed: the clone's mix 3 dB under the original's at equal gear and rpm (section 7a).
* A prop–prop pair's A/B order (0x5A0350): the clone takes the lower prop index as A; it
  matters only for the sound object's prop and a push-out over 200 km/h (**inferred**).
* The break test loop of 0x5AF0F0 (a prop that takes part of a push, 0x5B2080's scale < 1,
  then the impulse is worked out again with the prop moving) is not ported for car–prop or
  prop–prop pairs (the barrels have no break impulse; `ZCS_WaterBarrel` has 0.1).
* The copy's level beyond ~15 m from the car runs ~1 dB above the original's +0xC0 (test
  tolerance: median 0.25 dB, 90th percentile 1.25 dB).
* The drum's splashed flag (+0x50C) is set when its sound is posted, natively at its
  object's start (a capped drum would splash at a later record); the sound object's life
  after its record ends is not traced (the clone keeps it while its layers play).
* The prop poll catches the bodies after the pool's 0.3 s rescans; a barrel's first two or
  three frames after its hit can be missing from `native-props.tsv`.
