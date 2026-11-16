# Weather package (clone deviations, opt-in)

Design note: "add better weather effects", "the car can be affected as well, like appearing wet".
Four rows in a new **Weather** group of the Graphics page (`frontend/graphics_table.rs`), all **Off**
by default (native, byte-identical). Nothing here exists in SPEED2.EXE. The original's weather
schedule is unchanged and is what drives everything below: where and when it rains, the rain amount,
the wetness and the fog curves. No new weather types.

| row | values | options.txt | presets |
|---|---|---|---|
| **Wet Car** | Off / On | `enh.wet_car 0/1` | Modern, Ultra |
| **Rain Detail** | Off / On / High | `enh.rain_detail 0..2` | Modern On, Ultra High |
| **Spray & Splashes** | Off / On | `enh.spray 0/1` | Modern, Ultra |
| **Lens Drops** | Off / On | `enh.lens_drops 0/1` | manual (a camera effect, taste) |

`qol::wet_car()`, `rain_detail()`, `spray()`, `lens_drops()`, `weather_fx()` (any of them on).
Label ids 0xC104E0A0..A4 (`options::GROUP_WEATHER`, `WET_CAR`, …).

## Native inputs (already traced, docs/formats.md "Rain", `src/weather.rs`; nothing new traced)
- Rain amount, weather +0x1F0: `rain += (target − rain)·dt`. The target comes from the "Rain" map and
  the clock cycle 0x6134F0 / 0x61E390.
- Wetness +0x509C, step 0x61F2BD: up by 0.05 of the gap a step while it rains, down by 0.0015.
  The game sets it to 1 at track load (0x61E100) and only uses it for the flare stretch.
- Cover zones +0x1E4 / +0x1E8 (0x61E710, route triggers of kinds 5 and 6). The native drops are hidden
  under a cover.
- World clock 0x7FB718 and the lens drops 0x5C7AC0 (rain_drop.efx, `render/rain.rs`).
- Street lamps: the light packs (`lamps::lamps_of`).

**Clone values** (not native, all **inferred** / by eye):
- `Weather::road_wet`: from Wet Roads. It follows +0x509C's rule but starts at 0 at load.
- `Weather::car_wet` (new): the same rates. It starts at 0 and aims at the rain amount rather than at
  1, so drizzle leaves a thinner film. It aims at 0 while the player is under a cover zone (+0x1E4).
- `wet_car::spray_step`: driving on a wet road adds film to the lower body. It aims at
  road_wet × smoothstep(4 → 20 m/s), rises at 0.02 a step and dries at 0.0015.

The in-game frame publishes these values (`wet_car::publish_weather`, next to `set_weather`, two hook
lines in game.rs). Front-end scenes (garage, showroom) never take them, so their cars stay dry.

## Wet Car (`render/wet_car.rs`, `car_wet.wgsl`)
Per car, the renderer keeps a `WetState`: its velocity from the drawn poses over the world clock, and
its spray film. When the film and the spray are both ≤ `WET_MIN` (0.002), the car is drawn with the
**native / Enhanced Paint pipelines**, so a dry car is the same image bit for bit (R2 lesson,
docs/car-raindrops.md). A wet car uses a third module: car.wgsl (or Enhanced Paint's) with the final
combine's result passed through `wet_layer` (`CarMode` = scene, paint, wet). `Draw` grew by 48 bytes
(`wet: array<vec4,3>`, 560-byte entries, zeros when dry). The native module only reads the old fields,
and Original stays md5-identical (below).

Draw classes come from the light material (`WetClass::of`):
- **Paint**: the paint types and carbon.
- **Glass**: windshield, windows, light covers.
- **Tyre**: RUBBER.
- **Trim**: plastics, mouldings, grille, chrome, rims.
- **None**: lights, decals, interior, radiator, discs. These are unchanged.

The layer works per pixel, on top of car.efx's own terms:
- **Darker**: part of the diffuse is taken off (paint 30 %, tyre 45 %, trim 30 % at full film).
- **Water film**: the same env cube along the water's normal, Fresnel of water (F0 0.02), plus a sharp
  highlight of the harmonic's dominant light. The lobe is widened by the normal's spread
  (Toksvig-style), so distant cars do not shimmer. Paint takes a small share, because its coat already
  reflects.
- **Beads** on paint and glass: a dome per drop in 16 mm cells (3–6 mm drops) with a darker refracting
  rim. Coverage grows with the film and drops away as the film dries, so beads appear and vanish
  gradually. Beads are blown off the front and top above 12–32 m/s. They fade to the film's mean once a
  drop is under ~2 px, so nothing turns into dots at a distance or under Sharpen.
- **Rivulets**: thin streaks along the flow (air backward with the speed, gravity down), moving with
  it, filtered to their mean when sub-pixel.
- **Hood dries faster**: film² over the engine, ahead of the front axle and facing up.
- **Spray film** wets the lower body (below the sill) and the whole tyre.
- **Glass**: the water's light is not scaled down by the glass's blend alpha.

## Rain Detail and Spray & Splashes (`render/weather_fx.rs`, `weather_fx.wgsl`)
Drawn by `WorldRenderer` right after the native particles, so the temporal pass also draws them late
(no ghosting). The blend is premultiplied over, the colour only (the alpha bleach mask is untouched).
Depth-aware by reading the scene's depth, which works for MSAA and for the late single-sample path.
Everything is camera-relative.
- **Rain**: up to 5000 (On) or 10000 (High) streaks × the native rain amount, none under a cover zone.
  The drops are world-anchored in a 26 × 26 × 14 m box around the eye and fall at 7–9 m/s with a light
  wind. Each is drawn as the path it covers in 1/30 s, plus the camera's motion over 1/60 s (capped at
  0.35 m, so it never reads as warp lines). Streaks are ~1–3 px wide and hidden behind cars and walls.
  They are lit by the 32 nearest street lamps (native weight², forward scatter) and the cars' headlight
  cones and tail lights. The 400 native drops stay as they are.
- **Splashes**: 1200 / 2500 × the rain amount. Each takes a random screen spot for 0.3 s, reads the
  surface there from the depth, and keeps it only if it faces up (road, roofs, puddles), with a world-
  uniform density. It draws a flash plus droplets on short arcs. Wet Roads' ripples are unchanged.
- **Wet-air glow**: a soft halo around each of the 32 nearest lamps × the rain amount.
- **Spray**: a CPU pool (≤ 6000) at the 60 Hz world cadence, behind every wheel of every drawn car on a
  wet road (`spray_rates`: none below 4 m/s or on a dry road). Droplets fly and fall. Mist slows to the
  still air, grows and lingers, lit by the lamps and the tail lights.
- **Lens Drops**: up to 60 extra drops through the native lens pass (0x5C7AC0's shader, in chunks of 20
  after the native 20). They land only while it rains and not under a cover, slide down when slow, and
  are blown outward and off at speed. They follow the native Rain Splatter switch.

Not done: drips from overhangs and bridges (no data on where the edges are; open). Lamp light shafts
come from the temporal tier's Volumetric Fog, which already thickens with the rain (×(1 + 1.5·rain)). Wet-to-dry road
transition: The wet-roads and SSR work's Wet Roads already fades with `road_wet`.

## Native byte-identity and dry identity
Checked by md5 of the PNGs, fast build, 1280 × 960:
- Original against this copy's base: explore, intro, qrcars, race, qrstart, drift, drag and
  the rain shot (`NFSU2_RAIN_CLOCK=170`) are **identical**.
- The player's settings with all four weather rows on against off, in **dry** explore, qrstart and intro:
  **identical**. A dry car and a dry road draw nothing new.
- Parity gates (`perf.sh parity`, full release, <base> against this copy; internal evidence: weather/gates.txt):
  - images: Original, 5 scenes × 1280 × 960 / 4K, AE 0.
  - window path at 144 Hz: AE 0.
  - Modern / Ultra (perf.sh's preset strings, dry scenes): AE 0.
  - audio: identical.
  - sim: firstrace, firstdrag, postrace, the playthrough (82 695 steps) and the parity scenario, state
    hashes identical.
  - shots: 14 of 15 identical. `options_graphics` differs only by the new "Weather" group row
    (intended; `options_graphics_base_new.webp`).
  - The gate ran on the build before the mist optimisation. That change only touches weather_fx (off:
    never made). The fast-build shot md5s above were rerun on the final code.

## Cost
internal evidence: weather/scripts/bench.sh runs `--bench drive 600` at clock 162 (night, heavy rain 0.70,
four cars). The numbers are GPU busy ms: the weather pass's own timestamp ("graphics: weather fx"),
median of 3 interleaved runs. The full release build was measured on the reference desktop GPU and on the 2-CU
RDNA2 iGPU, on a shared machine (raw: `docs/evidence/weather/cost/`).

| row | 1080p | 4K | iGPU 1080p |
|---|---|---|---|
| Original frame (GPU busy) | 0.82 | 2.67 | 18.5 |
| Wet Car | ±0.02 (car pass, noise) | ±0.03 | ≤ +1 (noise) |
| Rain Detail On | 0.05 | 0.24 | ~1.2 (est.) |
| Rain Detail High | 0.06 | 0.27 | 1.65 |
| Spray & Splashes | 0.12 | 0.23 | 2.31 |
| Lens Drops | 0 (within noise) | 0 | – |
| all four | 0.14 | 0.42–0.49 | 3.53 |
| all four over the player's settings | 0.14–0.40 | 0.48–1.53 | – |

- Spray's mist is fill rate. The first version (two noise octaves, 12 mist puffs/s a wheel) cost
  0.69 ms at 4K and 1.53 ms at the player's settings at 4K. One octave, an early discard and 8 puffs/s
  brought it to 0.23 / 0.48 (`cost/summary-r1.txt` before, `summary-r2-mist.txt` after).
- The all-frame GPU medians move with the other jobs' load (the r2 run overlapped the parity
  gate). The pass timestamps are the stable figure.
- Row costs in graphics_table.rs: Wet Car 0.02, Rain Detail 0.24 / 0.27, Spray 0.25, Lens 0.01 (4K).

### The lens drops without a copy of the frame (High Precision Post)
The native lens pass (0x5C7AC0) draws the drops over a copy of the frame so far, sampling the frame: natively a
full-screen copy, at the scene's size under supersampling (16.6 MP of half float at 4K with SSAA 2x: the pass
was 0.49 ms, 0.46 of it the copy). The drops cover a few % of it. With High Precision Post the scene and the
screen copy share the half-float format, so the copy is the frame texel for texel: `LensPass::copy_sources`
copies only the texels each drop samples (`drop_source_texels`: the drop's box x − s/4 ..= x + 3s/4 by
y − 3s/4 ..= y + s/4, s = 0.12 × size, the filter's neighbour and two texels more each way) into the screen
copy at their own places, and the drops are drawn onto the scene itself sampling it. Every drop reads the same
texels and blends over the same pixel as before: the same frame, byte for byte
(`tests::fog_rain_paths::lens_drops_in_place_draw_the_plain_frames`). Without High Precision Post (the native
look, 8-bit targets of two formats) and when the image is not the scene (the streak copy before it) the pass
runs as it was. 4K, SSAA 2x, the player's rows, rain at clock 162 (7 interleaved runs each): the copy 0.458
[0.456..0.460] ms → none, the drops 0.032 → 0.048 (the texels' copy in it): −0.44 ms a rain frame
(docs/perf-budget.md "Fog, volumes and rain: the same pictures for less").

## Tests
- `render::wet_car::tests`: `dry_car_stays_native`, `film_follows_the_native_rates`,
  `spray_needs_a_wet_road_and_speed`, `velocity_in_car_axes`, `classes_by_light_material`.
- `render::weather_fx::tests`: `spray_only_on_wet_roads_at_speed`, `counts_follow_the_rain`,
  `spray_follows_the_clock`, `lens_drops_wiped_by_speed`.
- `render::car::paint_tests::wet_module_wraps_the_final_combine` and
  `draw_uniform_packing_matches_car_wgsl` (560 bytes, wet at 512).
- `qol::tests::weather_rows_round_trip_and_native_default`, `enhancements_round_trip` (45 deviations).
- `frontend::options` row order with the Weather group, label ids.

## Tuning aids
`NFSU2_WEATHER_DEBUG=1` prints the frame's counts, the published weather, the cars and the camera
velocity.

## Open
- Wet values are global for all cars (the player's cover zone). Other cars' own cover is not tracked;
  their spray film is their own.
- Headlight colour in the rain lighting is a fixed warm white (the cars' pool colours are not carried
  into the registry).
- The look is judged against the clone's frames only (there is nothing in the original to compare).
  The constants in car_wet.wgsl and weather_fx.wgsl are by eye.
- Shots step the weather only 5 s before the still, so "after the rain" is clock 176 in explore (car
  film 0.24). Longer drying is in the tests.
- Not measured on a laptop GPU beyond the 2-CU iGPU line in the evidence README.

## Additions (2026-11-05, docs/wet-road-car.md)
- **Wet Road Interaction** row (0xC104E0AC, `enh.wet_interaction`) in this group: tyre tracks, wakes, the cars'
  lights mirrored in the wet road, puddle splashes, rooster tails, arch spray, body sheeting. It runs the spray
  pool and Wet Roads' surface while on.
- Splashes (Rain Detail) are no longer placed at depth edges, on or under cars, or under covers
  (`weather_fx.wgsl` `splash_ground`; player report "splash … overlayed on objects").
- The spray pool reuses its slots in turn when full (was `Vec::remove(0)`).

## The Next-Gen rain work Next-Gen (2026-11-05, docs/rain-nextgen.md)
- **Rain Detail Next-Gen** (`enh.rain_detail 3`): a simulated rain volume (three layers, gusting wind,
  speed blur, HG-lit by lamps and headlight cones, screen-space cover test), world-anchored splashes on
  every up-facing surface incl. cars, rain rings on Wet Roads' puddles, quarter-resolution rain haze
  with lamp halos and distance mist, rain impacts and sheeting on the car (with Wet Car), running and
  merging lens drops (with Lens Drops). On / High unchanged.

## The wet-driving pass (2026-11-06, docs/wet-road-car.md "the wet-driving pass")
- Rain Detail On / High splashes: 5–9 cm crowns (were 8–16 cm, doubled at 30 m: "popcorn"), 0.2 s, faded from
  15 m, lit by the lamps and cars (60 % of the flat ambient off), a ring flash instead of a dome. Next-Gen unchanged.
- With Wet Road Interaction on, the Spray row's mist joins the volume at half rate; the spray follows the rain
  amount (`wet_road::water_depth`): a damp road after the rain throws a thin mist.
