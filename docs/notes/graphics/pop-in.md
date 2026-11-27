# Pop-in at World Detail Ultra / Extreme (2026-11-06)

Issue: pop-in clearly visible at World Detail Ultra
(Detail Distance 200 % + Keep All Loaded, LOD Fade on; the play setup,
3840 × 2160). At World Detail High with no Enhancements lines (native) the
native pops show as well; both are measured below.

Nothing here is native: the original's pops are the native lists and size
culls traced in docs/formats.md "Pop-in: what appears when" (lod180, 2026-11-01;
the clone's Original path matches them frame for frame there). The changes
are clone deviations inside World Detail Ultra / Extreme (`qol::pop_free`:
Keep All Loaded on); Original and every tier below Ultra are untouched.

## Measuring

Two instruments, both measuring aids (off unless the variable is set):

* **Pop meter** (`NFSU2_POP_STATS=1`, `crates/ob-render/src/render/pop_meter.rs`; report at
  the end of `--bench`, per-frame TSV with `NFSU2_POP_METER_LOG=file`,
  per-event lines with `NFSU2_POP_VERBOSE=1`). Every element entering or
  leaving the main view's picture while in the view frustum, by source:
  scenery size cull (props, lamps, signs; their flares follow), whole
  section swaps (visible-list change, look-ahead ring, far set), car LOD
  switch, car cull (size 4), traffic placed again while drawn, lamp light
  switched with its section (Dynamic Lamps / Modern Lighting), lamp shadow
  layer handed out. Size = projected bounding radius in output pixels
  (cull size × width / 512); "abrupt" = the change takes under 0.5 s.
  **Limit: frustum only, no occlusion or fog** — it counts what *could* pop.
* **Pixel truth** (`NFSU2_POP_ORACLE=1`: scenery and cars drawn whatever
  their size, cars at their top LOD; `NFSU2_G14_OFF=1`: Ultra / Extreme as
  before this work, for A/B on one binary). Frame dumps of the same fixed-clock
  drive compared with ImageMagick (`compare -metric AE -fuzz 6 %`): an arm
  against its oracle gives the content the cull leaves out; Ultra before
  against after gives every pixel this work changes; consecutive frames give
  the temporal change (pops as spikes over the local median).

Scripts (maintainer only, not in the repository):
`meter.sh`, `oracle.sh`, `spikes.sh`, `ab2.sh`, `pxstat.py`, `spstat.py`.

## Sources ranked (pop meter, `--bench drive 3000`, COROLLA, 3840 × 2160)

Abrupt events with a projected radius ≥ 2 px (≥ 32 px in brackets);
"gradual" = cross-faded over ≥ 0.5 s.

| source | Original | Ultra before | Ultra after | Extreme before | Extreme after |
|---|---|---|---|---|---|
| scenery size cull | 430 (430) | 258 (258) | 0 | (see note) | 0 abrupt, 246 gradual |
| section swap | 40 (40) | 36 (36) | 0, 36 gradual | 92 (92) | 0, 92 gradual |
| car LOD switch | 15 (15) | 15 (15) | 0 | 15 (15) | 0 |
| car cull (size 4) | 7 | 3 | 0 | 11 | 0 |
| traffic respawn | 0 | 1 | 0 | 1 | 0 |
| lamp light (Extreme) | – | – | – | 237 (237) | 0, 159 gradual |
| lamp shadow (Extreme) | – | – | – | 56 (56) | 0, 56 gradual |
| frames with an abrupt pop | 397 (13.2 %) | ~260 (8.7 %) | 0 | 922 (30.7 %) | 0 |

`--bench racing 3000` (play setup, 4K): before scenery 1874, sections 82,
car LOD 29, car cull 29; after 0 abrupt (sections 82 and 42 scenery
gradual). Note: the first Extreme-before scenery count (3275) included the
instances of sections leaving the set (a meter bug fixed before the after
runs; the after runs exclude them, the Ultra-before row above is the
corrected meter with `NFSU2_G14_OFF`).

**Pixel truth says most of these are not visible on the test drives.** The
meter's events are in the frustum but mostly behind buildings, in the fog
or a few pixels across:

* Scenery cull: Original against its oracle (everything drawn), 1080p,
  frames 1800–2399 of the drive: on average 14 pixels of the picture
  differ (at most 105); Ultra before: 0. Event 120 (7.8 km circuit, every
  20th frame): at most 97 pixels. The native 17 px size cull leaves out
  almost nothing visible on these routes.
* Section swaps: Original at frame 2172 swaps 11 sections, a ~2,300-pixel
  spike in the temporal change at 720p that Ultra does not have (the
  look-ahead ring draws them earlier; before this fix). Ultra before against Ultra
  after over the whole drive at 720p: no frame's temporal change differs
  by 100 pixels or more (largest 63, frames 2755–2773).
* The largest temporal spikes of every arm (20,000 pixels at 720p,
  frames 728–942) are the native animated chevron signs, not pops.
* Routes (`--bench racing 3600`, events 80 / 100 / 120 / 140, every 10th
  frame at 1280 × 720; internal evidence):

  | event | Original vs its oracle, mean / max px (frames ≥ 200 px) | Ultra before vs after, mean / max px (frames ≥ 200 px) |
  |---|---|---|
  | 80 (5.2 km) | 12 / 297 (1) | 2 / 65 (0) |
  | 100 | 59 / 296 (13) | 10 / 64 (0) |
  | 120 (7.8 km) | 20 / 94 (0) | 29 / 1,332 (5) |
  | 140 | 12 / 99 (0) | 10 / 743 (3) |

  The Ultra differences ≥ 200 px are section swaps caught mid-dissolve:
  a far hill's tree line (ev120 frame 2900) and a roof prop (ev140 frame
  460) drawn half by the screen-door pattern after this work where before they
  had already finished their 0.25 s fade
  (internal evidence,
  `ev140_f460_orig_before_after.jpg`: rows Original, Ultra before, Ultra
  after). The 4 × 4 dither stays visible for the 1 s of the dissolve
  without TAA (here SSAA 2×, 720p); `SECTION_FADE_SECONDS` is the knob.
* Look (LOOK RULE; 13 frames each, 320 × 180 luma): ev120 Original
  mean 48.1 / p5 11 / p95 135, Ultra before 48.4 / 12 / 134, after
  48.4 / 12 / 134; ev140 57.0 / 16 / 150, 56.9 / 16 / 151, 56.9 / 16 / 151.
  this work neither brightens nor flattens.

So the remaining pops the player sees are not reproduced by the benches'
routes; this work removes every abrupt transition the meter can see (each a
real, if mostly hidden, switch) so that none can show anywhere, at no
measurable cost.

## What changed (World Detail Ultra / Extreme only)

1. **Scenery drawn to a speck** (`render::main_min_size`,
   `qol::DETAIL_FULL_PX` = 1.5): the main view's size need is the smaller
   of 17 ÷ Detail Distance and a 1.5-pixel radius on the output
   (1.5 × 512 / width: 0.2 at 4K, where 200 % was 8.5 = a 64-pixel radius).
   Below one unit the CPU fade's size is not truncated (ftol would make
   it 1). The road reflection's enhanced capture keeps the percentage
   (`detail_min_size`). Water and the lamp flares follow the main view.
2. **Section swaps dissolve over 1 s** (`qol::SECTION_FADE_SECONDS`, with
   LOD Fade): a section joining or leaving the main or far set, and a
   section still fading out of one (its groups handed to the other set
   when it ends), cross-fades with the screen-door pattern over 1 s instead
   of 0.25 s (`FadeState::swaps`).
3. **Cars keep their top LOD while drawn and are drawn to a speck**
   (`car_lod` in bin/game.rs): the Car Geometry Detail's start LOD (A, or B
   at Low) down to 1.5 output pixels of a 3 m radius; no LOD switch, no
   size-4 cull. (Car Detail Distance still matters below Ultra.)
4. **Traffic fades at the despawn ring** (`ai::traffic::ring_presence`,
   `CarRenderer::set_presence`, `glow::car_sprites_faded`; with LOD Fade):
   a traffic car is drawn whole up to 200 m from the player and fades out
   (screen-door, its flares by intensity) to nothing at 250 m, the native
   placer's despawn / spawn distance (0x417BB0, `traffic::DESPAWN`); a car
   placed at 250–500 m therefore fades in as it comes nearer instead of
   appearing, and one re-placed far off has already faded. The native
   placement, pool and simulation are untouched.
5. **Lamp light and shadow fade** (`lamps::PopFades`, `POP_SHADOWS`; with
   LOD Fade and Dynamic Lamps / Modern Lighting): a lamp's added light
   follows its section's share over 1 s (light and Modern Lighting colour
   scaled; a section fading out keeps its lamps until 0); a lamp shadow
   fades in over 0.6 s from the frame its layer is handed out
   (world.wgsl reads `1 − fract(shadow.x)` as the shadow depth scale; a
   settled layer is a whole number: unchanged).
6. **World Detail help** now says Ultra / Extreme keep all of Bayview
   loaded ("Keep All Loaded" is part of them) and stop pop-in.

## Cost (reference desktop GPU, GPU busy ms per frame, 3000 frames, 4K)

| | before | after |
|---|---|---|
| drive, play setup | 9.54 mean / 9.58 median | 9.55 / 9.58 |
| racing, play setup | 9.94 / 10.09 | 9.89 / 10.05 |
| drive, Extreme (+ Modern Lighting 2 and a de-lit texture option since removed) | 14.55 / 14.52 | 14.48 / 14.41 |

Within run-to-run noise (budget ≤ +1 ms). Draw CPU 1.19 → 1.22 ms mean
(drive, user). `perfgate.sh run` against the stored baseline (main
99e726f, older than <base>; load average 19–45 at the start from other
agents) flagged CPU in racing-ultra-4K and rain-max-4K; a direct
interleaved A/B of <base>'s binary against this one on those two
configs (`perf.sh bench`, 5 + 5, 300 frames) passes: racing Ultra 4K GPU
9.08 → 9.07 ms, cpu_render 2.51 → 2.46 ms; rain max 4K GPU 28.19 →
28.12 ms, draw CPU p10 2.28 → 2.06 ms (no significant regression).

## Open

* The player's pop-in is not reproduced on the bench routes (drive, racing
  events 80 / 100 / 120 / 140): no visible pop at Ultra before this work that
  this work removes. A capture of where they see it (place, time of day,
  settings) would let it be measured; candidates not covered by the meter:
  pops in the road reflection / car cube (their own lists and culls), the
  native glow sprites' own fades, light probes' baking (one at a time near
  the car), lamp shadows taken away past 70 m (at once).
* No Wine side-by-side was redone: This page changes nothing on the Original
  path, and the Original pops are the native ones.
* Traffic's ring fade hides cars between 200 and 250 m that the original
  shows to ~270 m (size 4, in the fog); the car is always placed again
  from 250 m, so nothing beyond it stays.
