# Audio levels and the master volume (2026-11-11)

Issue: some effects played louder or quieter than in the original, and a low main volume
made the audio sound broken. This work traced the
option volumes in SPEED2.EXE, measured the original under Wine (private copy
a private Wine copy, fastgpu `--audio-file`, the option floats written live with
`peek.py`) against the clone's offline mix in the same scene (career free
roam, Rachel's 350Z at the airport car park, idle and a scripted drive), and
fixed what differed. Evidence: the maintainer's private captures (tables, Wine
readings, the scripts that regenerate them; raw WAVs are not in the
repository).

## 1. The native volume chain (verified unless marked)

Options: seven floats 0..1 at 0x83AA28 (+0 master, +4 speech & movies, +8
menu music, +0xC game music, +0x10 effects, +0x14 engine, +0x18 tires),
reached through [0x82B884]+0x38. Readers (all 26 found by scanning for that
pointer): 0x471970, 0x45C210, 0x45DCC0, 0x461B60, 0x462240, 0x474FBD,
0x47AD49, 0x47B9AD, plus 0x525E20 (direct).

1. **SFXCTL_MasterVol 0x471970** (each audio update): c = curve 0 of
   (0x7FFF − trunc(master · 32767)) / 32767 (0x468160; a cosine fade),
   f0..f5 = trunc((1 − v · master · c) · 32767) for menu music, game music,
   speech, effects, tires, engine; f8..f13 = trunc(v · master · 32767);
   f14 (+0x3C, copied to 0x7ED344) = f1, or 0x7FFF in drag / drift / no
   EA Trax song (0x471B75..0x471BBB). Section 0's controls read f0..f5
   through curve 8 → mB: the music, speech, FE and every effect's "effects"
   sub channel.
2. **Player volume fields 0x45C210**: f0 speech, f1 menu, f3 effects, f4
   engine, f5 tires = trunc(v · master · 32767) (no curve); f2 = [0x7ED344],
   f6 = 0x7FFF − that. Section 1 (the player's sounds) reads them.
3. The mix map turns those into per-object levels in mB (0x467FF0 lin ← mB,
   0x468050 mB ← lin; below lin 50 → 0, below −10000 mB → 0).
4. Sound objects pass levels to their AEMS events as parameters; the graphs
   give voice volumes (kind 2: v · 1/32767 float, 0x72085B; kind 8: the
   second byte >> 8, 0x724F5D) and the library mixes each voice through the
   pan law (0x724F5D: byte/127 × gain × the voice's channel weight).
5. **Streams** (speech 0x46CB48, NIS 0x46CBAE): volume = trunc(level /
   32768 · 100) / 127 via 0x723E9D, which sets each of the stream's library
   voices' gain (0x723E9D → 0x724F5D). A stream's channels are library
   voices at the azimuths of the table 0x8B4680 (0x7259xx): one channel at
   0, two at 0xC000 / 0x4000 (read live: a song's voices at 49152 / 16384,
   weights 0/1 and 1/0; a centred voice 0.706 / 0.706).
6. **Movies** (0x531E00, first frame of the movie object): vol =
   clamp(trunc(byte · master · speech), 0, 127) (0x525E20) → 0x6FEFEF →
   0x723E9D(vol / 127). The byte is +0x1F of the movie's entry in the list
   0x8637BC, built from GLOBALB.BUN chunks 0x39020, found by name
   (0x51C580, case-blind; not found → 0xFF → −1 → silence). Read under
   Wine: ealogo* 40, THX_logo* 69, FMVOpening 105, PSA / Scene* / *Tut 127,
   Track11 / IndustEast / IndustWest / Suburban / UpperClass 104.
7. **Collision objects' fields 0x461B60** (first mix pass of an impact): f0
   / f1 / f2 flags by kind (table 0x461CC8: kinds 0,1 → f0 + f5; 4,5 → f1 +
   f6; 7,8 → f2), level = trunc(v · r) · 255 >> 15 × trunc(effects · master
   · 32767) >> 15 (r = 1 − min(distance to the player car, 40) / 40). Section
   5's two event controls (−750 / −720 mB, attack 0, hold 83 ms, release
   183 ms) are read by sections 0 and 1: a hard impact ducks the music,
   speech and the player's sounds.
8. **Engine +0x138** (0x46F460, the abk block's p21): a camera-distance
   low-pass, 725..24000 (23920 within 6.5 m); Rec80 +0x48 is not read.

There is no other master stage: the library's output is the plain sum
(no limiter, SndMix_Render 0x729574), and the master is not applied again
per voice.

### Measured under Wine: the master law is the original's

Idle, airport, option floats written live (`wine_sweep.py`), RMS of 4.4 s
windows (dBFS; in brackets relative to master 1):

| variant | master 1 | 0.5 | 0.2 | 0.1 | 0.05 |
|---|---|---|---|---|---|
| all (music 0, speech 0) original | −27.1 | −37.1 (−10.1) | −50.5 (−23.5) | silent | silent |
| all, clone | −28.4 | −37.5 (−9.1) | −55.6 (−27.2) | silent | silent |
| engine 0, original | −27.7 | −39.1 | −57.5 | −104 | silent |
| engine 0, clone | −29.7 | −38.8 | −55.6 | silent | silent |
| effects 0, original | −34.2 | −43.3 | silent | silent | silent |
| effects 0, clone | −34.3 | −43.3 | silent | silent | silent |
| game music 1, original | −20.0 | −27.5 | −46.1 | silent | silent |
| game music 1, clone | −18.1 | −27.6 | −45.2 | silent | silent |

The original itself goes digitally silent at master 0.1: master enters as
v · master · cos-fade(master) (−36 dB at 0.1) and the mix map's mB stages
then cut the quieter objects (lin < 50). f14 read at game music 1: 0 /
21201 / 30748 / 32258 / 32643 at master 1 / 0.5 / 0.2 / 0.1 / 0.05 — the
clone's arithmetic gives the same numbers (test
`mixmap::master_law_matches_the_original`). Every AEMS event's parameters
matched the original at master 1 / 0.5 / 0.2 (CAR p4 −25811 / −30314 /
−32338, p14 2886 / 1017 / 178; FX_WIND 53 / 18 / 2; FX_WIND_Weather
2154 / 759 / 132; CAR_Sputter 8850 / 3121 / 545; the skid blocks;
`liveparams-idle.txt`). So the "wind louder at low master" (the
audit's) and the stacked master laws are **native**.

What made a low master sound broken in the clone and not in the original
was elsewhere:
- **movies ignored every volume** : at master 0.1 a movie
  played at full scale over a silent game. Fixed: item 6 above.
- **speech / NIS lines 3 dB too loud** : mono streams skipped
  the centre pan weight 180/255. Fixed: item 5 (`mixer::stream_pan`).
- **impacts never ducked** the music, speech and the player's sounds, and
  more than five collision sounds could pile up. Fixed:
  items 7 and the cap below.

The CAR layer at master 0.2 (once open: "the original keeps it audible")
is **native**: read live, the original's CAR instance at master 0.2 has
all eight players stopped (state 0, kind 0xFF), exactly as the clone's
graph leaves them (instance memory dumped and compared player by player,
`scripts/instdump.py`); its last voice holds the ramp's last step (28)
until the stop. The "all − engine 0" difference of the table above was
other content (the rain and its THUNDERSTRM lines, the second effects
stream, §1b).

### 1b. The slider's positions, layer by layer (verified)

The options screen's master slider (0x49A790 builds it: value from
0x83AA28, max +0x78 1.0, min +0x7C 0, step +0x8C 0.05; 0x49A720 on a change
stores the value to 0x83AA28 and calls 0x469050) steps through 0x4FF480:
value = f32(value ± 0.05), clamped to [min, max]. From 1.0 down that is 21
positions: 1, 0.94999999, 0.89999998, 0.84999996, 0.79999995, …,
0.19999984, 0.14999984, 0.09999985, 0.04999984, 0 (the clone's
`options::change` steps the same f32 sum).

Measured at every position in both games (airport free roam, idle and
full throttle; speech and game music 0; internal evidence: `slider-sweep.txt`, scripts `orig_sweep.py`, `clone_sweep.sh`,
`cmp_params.py`, `table.py`): every player event's parameters are equal
(CAR p4 / p14 / p25, CAR_Sputter, FX_SKID, FX_TURBO_01, FX_WIND_Weather),
the GIN bytes follow the same truncation, the stream bytes the same law,
and the graphs cut at the same positions. Where the original itself goes
quiet (the first position without the layer):

| layer | cut at (slider) | why (native) |
|---|---|---|
| engine abk layer, idle (CAR players 1..7) | 0.30 (p4 −31825) | the CAR graph stops its players below its volume floor |
| engine abk rev-limiter layer (player 0, p25) | 0.15 (p25 280) | the same graph |
| accel GIN voice, full throttle (byte +0x108) | 0.55 | ((HM +0x80 · level0) >> 15) >> 8 = 0 (0x4740C0) |
| skids (FX_SKID) | 0.30 | graph |
| weather wind (FX_WIND_Weather) | 0.15 | graph |
| turbo whistle (FX_TURBO_01) | 0.05 | graph |
| score, effects streams | 0.10 | trunc(level / 32768 · 100) = 0 |
| pause-menu click (PlayCommonSample p1) | 0.05 | 4235 / 1493 / 573 / 261 / 146 / 66 / 0 at 1 / 0.5 / 0.3 / 0.2 / 0.15 / 0.1 / 0.05 |

So the engine "cut off" at a low master is the original's: at idle the
abk layer stops at 0.30, while driving the GIN voices go at about 0.55
(full throttle) and the abk layers by 0.15; the whole output is silent
from 0.10 (0.05 at full throttle). Test
`mixmap::slider_positions_match_the_original` holds all 21 positions'
p4 / p14 / wind levels at the default volumes and at effects 0.85 /
engine and tires 0.65 (the products' f32 truncations), and the click
levels. p14 scales by the reverb's wet B [0x82B070], read 5186 and 5187
in the same scene (the one-unit difference at master 0.35 with engine
0.65 is that, not the master law).

### 1c. Driving at a low master (verified)

The first drag (Peugeot 206) driven at full throttle through the gears
with a lift, at slider positions 1, 0.70 … 0.20, the original in real
time against the clone (the maintainer's private capture;
scripts `drive_rt.py`, `native_poll.py`, `envelope_rt.py`,
`layers2.py`). Per position the output's 15 ms envelope and, per engine
layer, the share of time it sounds and its on/off switches per second
agree within the drives' own difference:

| slider | abk layers 5 / 6 on (orig / clone) | accel GIN on, its byte | switches / s (abk 6, GIN) | output |
|---|---|---|---|---|
| 1 … 0.40 | 89–90 % / 83–90 % both | 88–95 %, byte 11 → 2 | ≤ 0.25 both | continuous |
| 0.30 | 85 / 57 % · 90 / 83 % | 92 % both, byte 1 | 0.45 / 0.35, 0.2 / 0.25 | continuous |
| 0.25 | 88 / 82 % · 90 / 74 % | 27 % / 24 %, byte 1 | 0.55 both, 0.7 / 0.9 | continuous, the lift −53 / −60 dBFS |
| 0.20 | 73 / 58 % · 83 / 50 % | 23 % / 12 %, byte 1 | 0.56 / 0.55, 0.4 / 0.5 | 10 % of windows under −50 dBFS (the lift) in both |

So in the original too the engine thins out while driving at a low
master: from 0.30 the accel GIN voice sits at volume byte 1 of 127 and
from 0.25 it is there only on the revs' peaks (about a quarter of the
time, switching on and off about once a second), and the abk layers
carry the sound; the engine is complete (every layer steady) down to
about 0.35. The stages after the parameters are the same in both: a
voice's gain changes ramp over the mixer's 16-sample block (0x728B48;
`mixer::gain::mix_gain`); a GIN voice at byte 0 keeps running silent
(the original's library voices read playing at gain 0) and comes back
at the next non-zero byte without a restart; an abk player the CAR
graph stops restarts its sample from the start when the graph starts it
again (the graph's own logic, the same program); the engine's volumes
are set once per audio update (AudioManager 0x483B40, every 60 Hz frame)
and the graphs tick every 20 ms in both.

## 2. Level balance, original vs clone (dB)

Same free-roam start, same key timeline (`wine_drive.py`, NFSU2_SOAK_KEYS
"0 up 6;9 down 2;12 up+left 3;16 up 3"), music and speech 0, master 1;
`drive-base.txt` / `drive-new.txt` (per second, RMS and 0–250 / 250–2000 /
2000+ Hz bands):

| category (seconds) | original RMS | clone before | clone after |
|---|---|---|---|
| idle bed (8) | −27.6 | −29.2 (−1.6) | −29.2 |
| idle, 4.4 s window | −27.1 | −28.4 (−1.3) | −28.4 |
| full throttle on the limiter (4, 5, 15) | −18.2 / −17.7 / −18.8 | −18.5 / −18.2 / −18.8 (−0.3 / −0.5 / 0.0) | same |
| launch, 1st–2nd gear (2, 3) | −15.3 / −16.3 | −17.4 / −18.2 (−2.1 / −1.9; low band −5 / −7) | same |
| lift-off / decel (6, 7) | −20.4 / −22.6 | −21.0 / −25.1 | same |
| speech line (D-Day, same line, stem) | −24 (round-12 reading, other line) | −16.4 … −18.3 | −19.4 … −20.7 (−3.0) |
| engine-only (effects 0) idle | −34.2 | −34.3 | −34.3 |
| score / music (game music 1, idle) | −20.0 | −18.1 (+1.9, a different piece) | same |

Within ±1.5 dB except the launch's low band and the speech (fixed). The
seconds where the two runs drove differently (11, 12, 19, 21: walls, steering)
are not level comparisons. Turbo, nitrous, skids, scrapes, traffic and FE
clicks did not occur comparably in this scene; their parameter blocks were
compared instead (all equal at rest). The launch row and the idle bed of this
table are superseded by §2b: measured again at the original's own audio
cadence, the engine has no launch or idle gap (the airport's ENV_STATIC
planes and the reverb stay open in formats.md "Sound comparison").

### 2b. The engine in real time at the original's cadence (observed)

**Why the earlier real-time readings were off.** The original under Wine
(fastgpu, wined3d) runs free roam at 73–92 fps, not at its 120 fps cap. The
AudioManager (0x483B40) runs the sound contexts only when more than 15 ms
(0x787AF4) passed since its last run: one update every second frame, so
36–46 updates a second instead of 60 (read live: ctx +0x48 changed 36.5 times
a game second at 73 fps; 57.7 at a fixed 1/120 s frame). The HybridMotor's
rpm-change filter (HM +0xD0, 0x45D0A0) is a change *per update*, so at
36.5 updates it read 1.55× the rpm slope / 60 (9.55 against the clone's 6.2
at the same 367 rpm/s), which raises the rec80 lerp t1 = |+0xD0| / rec80 +0x2C,
the accel GIN's B gain (+0xFC) and its volume byte (8.1 against 6.6 at
120 km/h), and the AI racers spun their wheels harder at the drag's launch
(FX_SKID p3 up to 1024 on all three, against ≤ 17 at 120 fps), which was
the launch's missing 1–8 kHz. A second artefact made the prop runs' beds
(docs/prop-hits.md) unequal: the original's car was given its gear without
the gearbox's own state and kicked down to first or second gear (the engine
on its 7000 rpm limiter), while the clone rolled in the gear asked for.

**Method** (scripts and measurements: docs/evidence/engine-bed/). `scripts/rt_free.py` (free roam) and
`drive_rt_f.py` (the first drag): the parity cave holds the game while the car is placed, then stays
in the frame-time call with every frame 1/120 s (no recording, no lock), so
the original runs its own 120 fps cadence (world 119.8 updates and the audio
~60 updates a game second) whatever the wall clock; the mix is recorded with
fastgpu `--audio-file` and placed by the sim clock (the game clock runs at
0.8× the wall clock, a stretch that leaves steady-state levels and spectra
alone). The car is placed on the highway straight north of the yellow
barrels (352.6 −1445.0 36.6, heading 107°, 390 m straight) and set rolling
in its gear (rigid body P, wheel spin, gear, input shaft and engine speed,
and the gearbox's gear request [car +0x30] +0x1E4, pending gear +0x38 and
+0x48 = 0 so it stays there) with the throttle key held; the clone runs the
same place, speed and gear from the explore shot with the same save
(`NFSU2_SHOT_SAVE`: the stage-2 save with speech 0, game music 0, manual
gearbox). Both runs then accelerate at full throttle in that gear: the
speed agrees within 0.6 km/h and the audio rpm (CAR p1 = HM +0xF8, the same
quantity on both sides) within 1–4 % over the whole run. Bands are the mono
mix's power in 0–125 / 125–250 / 250–500 / 500–1k / 1–2k / 2–4k / 4–8k Hz.

| run (window) | original total | clone total (Δ) | Δ per band, clone − original |
|---|---|---|---|
| 60 km/h, 2nd gear, full throttle (1.5–5.5 s; 72 → 105 km/h) | −21.5 | −21.2 (+0.3) | −2.3 +0.5 +0.2 +0.4 +0.4 +0.2 −0.1 |
| 120 km/h, 3rd gear (1.5–5.5 s; 128 → 150 km/h) | −21.3 | −20.5 (+0.8) | +2.5 +0.7 0.0 +0.1 +0.1 +0.1 −0.2 |
| 200 km/h, 5th gear (1.5–5.5 s; 203 → 210 km/h) | −20.7 | −20.4 (+0.3) | 0.0 −1.5 +0.5 +0.8 +0.3 +0.2 −1.6 |
| idle, engine volume only (1–6 s) | −30.4 | −30.3 (+0.1) | +0.2 −0.4 −0.5 +0.1 −0.4 −0.2 −0.2 |
| first drag, launch (0–2 s) | −15.0 | −16.2 (−1.3) | −1.9 −0.6 −5.1 −0.1 −4.1 −0.6 +0.6 |
| first drag, 2nd gear by audio rpm 1750 / 2000 / 2250 | −16.7 / −16.9 / −13.5 | −0.4 / 0.0 / −0.3 | 0–250 Hz within ±0.4 |

Per layer (means over the same windows; original read live with
`native_poll.py`, clone from the event log): accel GIN byte 7.8 / 6.1 / 6.7
against 8.7 / 6.6 / 5.0 at 60 / 120 / 200 km/h (decel GIN 0 both); the abk
block's p4 −25124 / −25015 / −25544 against −25162 / −24900 / −24576 (one
abk player sounds in the original, player 7 at gain 0.21–0.23); at idle p4
−27196 on both sides (test `engine_chain::idle_layer_level_is_the_originals`).
The 200 km/h p4 / p14 dips in the original are the impact duck of section 5
(the car brushing the barrier); the clone's run touched less. In the clone's
own stems at 120 km/h the player's abk layers are −22 dBFS, the accel GIN −34,
road noise −38.5, wind −37, the score −30 and the reverb −40.

**Verdict: the player's engine is native** — within ±0.8 dB in total and
±2.5 dB in any band at 60 / 120 / 200 km/h, at idle and through the drag's
gears; no engine stage was changed. The remaining small terms are
**observed**, not traced: at 120 km/h the original's +0xD0 read 4.6 against
the clone's 6.2 (B gain 0.126 against 0.135, the accel GIN −0.6 dB, a layer
12 dB under the bed: under 0.1 dB of the total), and the launch's first two
seconds (−1.3 dB) where the original's audio rpm leads the clone's by
0.1–0.2 s.

**The barrels (the report's question).** Hit levels from docs/prop-hits.md
(original −11.9 / −11.5 / −10.4, clone −12.1 / −11.7 / −9.1 dBFS) over the
beds above (full throttle in the gear the barrel runs use):

| speed | bed original | bed clone (no change) | hit over bed, original | clone |
|---|---|---|---|---|
| 60 km/h | −21.5 | −21.2 | +9.6 | +9.1 |
| 120 km/h | −21.3 | −20.5 | +9.8 | +8.8 |
| 200 km/h | −20.7 | −20.4 | +10.3 | +11.3 |

Computed from the beds: the 23 m barrel approach run in real time in both
games (`fb*` / `cb*` runs) gives the hit itself (original −10.8 / −12.0 /
−11.7, clone −10.6 / −8.9 / −10.2 dBFS) but not a bed — the hit comes 0.4–0.7 s
after the car is set rolling, while the original's throttle key is still
arriving (its CAR p3 is 0 for the first ~0.5 s). The earlier "hit over bed
+6.3 / +12.3 / +7.5 (original) against +10.7 / +14.1 / +14.8" compared the
original kicked down onto its limiter with the clone in gear.

## 3. What changed (all in the Original path; parity differs where listed)

| change | where | native source |
|---|---|---|
| mono streams (speech, NIS lines, mono movies) at 180/255 per side | `mixer::stream_pan`, `render_tracks`, the movie stream | 0x8B4680, 0x724F5D (read live) |
| movie sound volume = byte · master · speech / 127 | `mixer::movie_volume`, `movie::volume_table`, `Game::movie_tick` | 0x531E00, 0x525E20, 0x6FEFEF, GLOBALB 0x39020 |
| impact ducks (section 5 fields) and impact copies kept 0.3 s for the envelopes | `collision::mix_fields`, `Game::car_sounds` mix scene | 0x461B60, mix map section 5 |
| at most 5 collision objects play (nearest kept), 10 queued per update | `collision::Pool` | 0x481850, 0x481CD0, 0x7ECBC8 |
| engine abk p21 from the camera distance | `HybridMotor::engine_cutoff` | 0x46F460 |
| turbo spool one step per update (no dt), fed the pedal % (+0x54) and its ±30 slew (+0x44) | `carfx::Turbo::spool_tick`, `PlayerSfx::ctx44`, `CarInput::throttle` | 0x472770, 0x484C50 |
| FX_SHIFTING_01 p6 = bumper camera; CAR_Sputter p8 toggled by flame pops | `CarInput::bumper_cam`, `carfx::BACKFIRE` | 0x45E8C8, 0x459200, 0x469BC0, 0x45F780, 0x5EBF0E / 0x5F6059 |
| skid slip ratios 0 unless moving forward | `skids::front_ratio` / `rear_ratio` | 0x5ED4A0 / 0x5ED540 |
| device rate change rescales voices / engines / movie stream and reloads the FX graph | `Mix::set_rate`, `Audio::fx_rate` | (clone device handling) |
| one-shots dropped while no device stream runs | `Mixer::rendering` | (clone device handling) |
| every cpal sample format; extra channels silent | `open_stream_with`, `render_chunk` | 0x8B46E8 = 2 channels |

Check aids for per-layer comparisons: `NFSU2_AUDIO_STEMS_SPLIT=1` (with the
stems: the player's own CAR instance as `CAR.player`, the engine voices as
`GIN.accel` / `GIN.decel`), `NFSU2_AUDIO_SOLO=name,…` (only those sources
reach the offline mix; `-reverb` drops the reverb's output), and the event
log's `GIN` row carries HM +0xD0 × 1000, the B gain × 10000 and the context rpm
after its five values.

Check aids added: `NFSU2_AUDIO_STEMS=file` (per-source RMS every 0.5 s of an
offline capture: AEMS event names, `GIN`, `speech`, `music`, `reverb`,
`collision`), soak phases `explore` and `master:x`.

## 4. Audit items (2026-11-11)

| item | verdict |
|---|---|
| wind up at low master | native (f14 21201 at master 0.5 read in the original; levels equal) |
| stacked master laws, floors | native (original silent at 0.1 too); the CAR layer at 0.2 is stopped in the original too (§1b) |
| streams skip the pan law | real, fixed |
| movies ignore volumes | real, fixed |
| surround channels | real, fixed (front pair only) |
| 48 kHz before the device / queued one-shots | real, fixed |
| U16/I24/F64 silent | real, fixed (all cpal formats, a named warning otherwise) |
| bottom-out timer | native (ctx +0x34 is a running clock: 570.8 s read live) |
| engine cutoff | real, fixed (0x46F460; Rec80 +0x48 unused natively) |
| drive-by whoosh tier | native (ramp 0x7F0B40 y 0..1, read live) |
| skid ratio truncation | native; the forward-speed gate added |
| collision cap | real, fixed (5 playing / 10 queued) |
| collision start volume "inferred" | the mix-map path takes the copy's slot (verified); the impact duck fields were missing (fixed) |

## 5. Open
- The in-game score plays at the score byte v (**observed**). The score
  plays on the music manager's MPF stream (0x482A2A, stream object vtable
  0x78BB98, 0x29000 buffer; MPF slot 0 at 0x82B620); the game sets v = the
  score byte (+0xBC >> 8, 0x47B647; the Ambience level >> 8, fed by the
  effects volume and the master) through 0x47B820 (slot +0xC = v). Read in
  free roam (effects 1, master 1; the stage-2 save at its resume point and
  on the highway; the original at its own pace and at a fixed 1/120 s
  frame): +0xBC 10337, slot +0xC 40, the music object's context +0xB4 0,
  the playing node 0x82AFD4 110 (event 27 → stream entry 0), the stream's
  one library voice (2 channels) at gain 40 of 127, steady over 70 s. By
  the sound: the decoded stream of event 27 found in the recorded mix by
  cross-correlation (lag 0.4 ms, the same over three windows 15 s apart)
  fits at 0.31 / 0.31, 0.33 / 0.33, 0.31 / 0.30 per channel = 38–42 of
  127, so v (0.315), not 2v − 1 (0.622). An earlier reading of the same
  voice field at 79 / 39 / 19 / 27 for v = 40 / 20 / 10 / 14 (effects 1 /
  0.5 / 0.25, master 0.5) did not reproduce in six runs; the state it was
  read in (save, place, session time) is not known, so whether some state
  doubles the byte stays **open**. Clone: `streams::score`
  `in_game_score_volume` = v (the 2v − 1 port reverted), test
  `the_in_game_score_plays_at_the_score_byte`. The context (0x46B330 from
  0x47B260): in game the Ambience map's field 0 at the car (0x5FA550 /
  0x5F0B90 on the accessor 0x82C710, bound in free roam), **traced** and
  **observed twice** under Wine: at the stage-2 save's resume place (462.4,
  −46.2) row 0 → context 0, event 27; placed on the highway (352.6,
  −1445.0) row 5 → context 5, event 55 — the clone's map gives 0 and 5 at
  the same places (`the_score_context_is_the_ambience_map_at_the_car`).
  The context 4 read before was the explore shot's place: the harness
  starts free roam at the ForwardStartingPosition (1694.9, −883.5, map 4),
  not at the saved place the career start resumes at (0x495AD4 sets
  career +0x8744 = 1; the save's own flag byte is 0), so comparisons of
  the score at a resume point need the clone started through Career →
  Resume (docs/native-fixes/score-context.tsv).
- Driving bed against the original in real time: **closed** (§2b). The
  gaps read before (2nd gear −4.8 / −4.5 / −2.3 dB, the launch's low band)
  came from the original running at 73 fps under Wine (36.5 audio updates
  a second, HM +0xD0 1.55× too large) and from unequal rpm logs; at the
  original's 120 fps cadence, binned by the audio rpm (CAR p1) on both
  sides, the drag's 2nd gear is within −0.4 … 0.0 dB and the free-roam
  runs at 60 / 120 / 200 km/h within +0.3 … +0.8 dB. Still **observed
  only**: the original's HM +0xD0 at 120 fps (4.6 against 6.2 at the same rpm
  slope: the accel GIN −0.6 dB) and the first 2 s of the drag (−1.3 dB).
- The AI racers' skids (CARSFX_Skids on an AI racer: one event, 0x473BA0
  sets the count +0xC4 = 1, 0x4608D0 makes it from the player's FX_SKID
  record 0x82C500, 0x460A70 takes the mean of both sides' intensities and
  one surface from both sides and the previous update, 0x460C70 writes
  event 0 with mix-map class 5 slots 3 / 5 / 4 of the racer's section-2
  copy) are **played** (`PlayerSfx::ai_skid_update`, from the player's
  skid bank; the rival's SFXCTL_Skid output is its copy's skid field).
  Their input is the car's exported slip (car +0x3B4 + 8i, 0x5A5550: a
  wheel's +0x100, which the tyre pass 0x5A8290 sets to the slip while the
  tyre slides and to 0 while it grips; the wheel gates +0x3DC / +0x3E0
  take the wheels' own slip +0x118). At the first drag's green the
  rivals launch from the limiter and spin their driven wheels in the
  original as in the clone (car +0x3B4 9–13 m/s; wheel spins, clutch, rpm
  and speed equal frame by frame at 60 and at 120 fps, and at the first
  circuit's start), so the original's three AI events read p3 = 1024 while
  they spin: 0.2–0.55 s at 1/120 s frames (two real-time runs with the
  cave's fixed frame time and one lockstep run), about 1.0 s at 60 fps.
  The earlier "p3 ≤ 117 for 0.3 s" came from a sparse event poll, and the
  +7.5 / +10.3 / +4.8 dB from a clone replay at 1/60 s frames against a
  1/120 s recording. At the same cadence (bands 0–125 … 4–8 kHz, dB clone
  − original, the two runs; evidence/ai-launch/):

  | first drag (0.1 s windows) | total | 0–125 | 125–250 | 250–500 | 0.5–1k | 1–2k | 2–4k | 4–8k |
  |---|---|---|---|---|---|---|---|---|
  | 0.2–0.7 s, no AI skids | −1.0 / −1.5 | −0.1 / +0.4 | +0.8 / +1.1 | +3.2 / −2.2 | −3.9 / −6.4 | −8.4 / −8.4 | −5.8 / −4.9 | −5.1 / −3.4 |
  | 0.2–0.7 s, AI skids | +0.8 / +0.4 | −0.1 / +0.5 | +0.9 / +1.3 | +3.4 / −2.0 | +2.4 / −0.0 | +2.2 / +2.2 | +1.0 / +1.9 | +0.1 / +1.8 |
  | 0.2–0.7 s, the AI skids alone | −3.7 / −4.1 | | | | +1.4 / −1.0 | +1.9 / +1.9 | +0.2 / +1.1 | −1.3 / +0.4 |
  | 0–2 s, no AI skids | −1.3 / −0.6 | +0.6 / +0.9 | −3.4 / −0.5 | +0.8 / −2.0 | −1.1 / −5.2 | −5.5 / −5.7 | −3.1 / −2.5 | −2.6 / −1.4 |
  | 0–2 s, AI skids | −0.6 / +0.0 | +0.6 / +0.9 | −3.4 / −0.5 | +0.8 / −1.9 | +2.8 / −1.4 | +2.3 / +2.0 | +1.8 / +2.4 | +0.4 / +1.7 |

  The soloed AI skid layer (`NFSU2_AUDIO_SOLO=FX_SKID.ai`) alone carries
  the original's whole 1–8 kHz energy of the launch (third row): the
  sound confirms the inputs.
  **Open**: the clone's skid sound sits about 0.1 s later and runs about
  0.2 s longer than the original's in 0.1 s windows (its rivals' spin ends
  at 0.65 s in the replayed lockstep run, the real-time runs' at
  0.55–0.6 s; the graph's release after p3 falls is not compared), hence
  1–4 kHz +2 dB over 0–2 s; 125–250 Hz −3.4 dB in one run is there
  without the skids too (docs/native-fixes/ai-skids.tsv).
- Collision steal protection (+0x128 bit 0, a car with a sound object in
  the pair) is not modelled; the CAR block's p15 (native 0 at the airport,
  the clone's slot-0 azimuth 65304) and p17 (Engine +0x134, native 150,
  clone 0); the score "Volumes" ramp's hold / game-state gate.
- The idle bed with the effects volume on: the in-game score dominates it
  (the clone's score stem −26 … −31 dBFS at idle, played at 2v − 1 then);
  its level is settled above (v), its context choice at the resume point
  is open. The engine alone at idle is equal (§2b).
