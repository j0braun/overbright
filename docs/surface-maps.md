# Surface Detail: material-ID roughness and generated normal maps (clone deviation, opt-in)

The graphics wave / research report item 10 ("material-ID roughness gains more than per-pixel normals;
derive any maps on the player's machine from their own files"). One Graphics row, **Off by default**
(native, byte-identical: parity gates below). Nothing is shipped: the maps are made from the player's own
game textures and kept in the player's cache dir.

| row (group) | values | options.txt | code |
|---|---|---|---|
| **Surface Detail** (Lighting) | Off / Materials / Materials + Normals | `enh.surface_detail 0/1/2` | `surface_maps.rs` (CPU), `render/surface.rs` (GPU), world.wgsl LAMPS blocks (`surface_at`, `surface_spec`, `lamp_shade`, `wet_surface`) |

`qol::surface_detail()`; label id 0xC1_04E0_C0; presets: **none** (manual row; see Verdict).

## What the original has (traced: nothing)
world.efx is `tex · v0 · 2` (baked vertex light), worldreflect adds the planar capture by the
texture's alpha, worldheadlight adds view 9's projector (docs/formats.md, world.wgsl header). There
is no material, roughness, specular or normal input for the world in SPEED2.EXE. Everything below is
**invented** (a clone deviation) and only feeds the clone's own light terms.

## Materials (`surface_maps::classify`)
Per world texture slot at load, from the name and its draws (cheap, name-only; no pixels):
EA's name prefixes (Bayview L4RA: ARC architecture 836, SGN signs 319, OBJ 146, ORG plants 69, TRN
terrain 34, ADS adverts 26, RDP road pieces 24, SFX effects 17, ANM animated), `_`-token words
(SIDEWALK, METAL, WINDOW, BRICK, …), the draws' effect (most draws worldreflect = a road surface, mesh
flag 0x40), blend and alpha test. Classes and parameters (roughness, F0, map strength):
Asphalt 0.62/0.035/1.0, Concrete 0.78/0.03/0.8, Metal 0.38/0.45/0.5, Glass 0.06/0.05/–, Sign 0.42/0.045/–,
Rough (plants, terrain, cut-outs) 0.92/0.02/–, Generic (façades with painted windows and light)
0.72/0.03/–, Skip (additive/emissive, sky, glows, alpha-blended decals, water, panoramas): untouched.
Bayview: 1518 textures; Concrete 202, Asphalt 29, Metal 85, Glass 41, Sign 276, Rough 143, Generic 544,
Skip 197 (internal evidence: surface-maps/census-l4ra.tsv, `devprobes/src/bin/surface_census.rs`).

The class rides in the draw's immediates padding (`TextureImmediates._pad`: map layer, packed
material), read only by the lamps' module (`TEXTURE_ARRAY_SURFACE_WGSL`); needs texture arrays
(immediates) — without them the row does nothing. Main view only (`frame.cull.z` = 1/2, unused by the
native shaders); reflections, cube and shadows untouched. Uses (world.wgsl):
* **Specular** (GGX, Smith-Schlick, Schlick F, widened by the light's apparent size): the street lamps
  (the light packs, docs/lamps-shadows.md; the clusters are built while the row is on, Dynamic Lamps
  on or off), the native headlight projector (view 9) and Headlight Beams; × 0.6, ≤ 4 per light.
* **Wet Roads**: the class scales the wet roughness (concrete rougher, ×0.6–1.4 of asphalt's), and the
  lamps' highlights get the wet roughness (mirror-like streaks) and F0 ≥ 0.02 (water).

## Normals (`surface_maps::build_normals`, `normal_verdict`)
From the texture's top level: luma → **high-pass** (Gaussian σ = w/64 texels, 1.5–4, wrap-around: the
baked/painted light and large shapes removed before any gradient, so painted shadows do not emboss) →
soft clip at 2.5 × the band's RMS (paint lines and edges do not become cliffs) → Sobel → tilt
`SLOPE` 0.24 per band-RMS step (≈13° at strength 1) → 256² (bilinear), box mips, RG8 snorm.
Rejected (no map): classes without maps, alpha-tested cut-outs, < 32 texels, **Flat** (band RMS
< 0.012), **Painted** (gradient p99/median > 9: drawn shapes, not grain), and two façades rejected by
eye (`REJECTED`: painted windows embossed). Bayview: 283 candidates → 230 maps (50 Painted, 3 Flat).
Shader: cotangent frame from the position/uv derivatives (the world has no vertex tangents),
explicit-gradient sample (no implicit derivatives in branches: multiview-safe), specular AA (roughness
widened by log2 of the map's minification). The mapped normal drives the lamps' and beams' N·L, the
specular, the **relief** of the native headlight projector's light (ΔN·L / max(N·L, 0.25) × 1.5,
clamped 0.3–1.7: the headlights graze the road) and of the **baked light toward the lamps** lighting
the pixel (their weighted direction; × 1.5, clamped 0.6–1.4, faded in by their weight), and breaks up
the wet reflection (not in the puddles' mirror). `NFSU2_SURFACE_DEBUG=1` shows the normals' tilt.

## Cache and background build
`<cache dir>/surface-maps/<xxh3 of the texture bytes, palette, shape, format, strength, generator
version>.rg8` (175 KB, all mips) or an empty `.none` (judged: no map). `surface_maps::Builder` makes
them on a rayon pool from a background thread when Materials + Normals is first on; the renderer
uploads arrivals each frame (`SurfaceMaps::pump`) into one array (a layer per candidate). Headless
modes (shots, benches, flows, selftest) wait for the whole set before the first frame
(`set_blocking`). Bayview cold: +0.3–0.4 s wall (16 threads, machine at load ~90), warm: 0; cache 39 MB;
VRAM ≈ 50 MB (284 layers × 175 KB).

## Review (`devprobes/src/bin/surface_sheet.rs`, evidence/surface-maps/)
Sheets of the 50 most visible mapped Bayview textures (by area within 20–40 m of road surfaces;
per texture: albedo, a grazing lamp without / with the map, the map) — `sheet-map-01-24.jpg`,
`sheet-map-25-50.jpg` — and the 16 most visible rejected as Painted (`sheet-rejected-painted.jpg`).
Reviewed by eye: tiles, cobbles, stone walls, corrugated metal, kerbs and asphalt grain read as relief;
crosswalk/lane paint slightly raised (plausible); two façades with painted windows looked embossed and
were rejected; parking bays, lit walls and sign-like art are rejected by the edge measure.
In game (1920×1080, `scripts/scenes.sh`; pixels changed > 1 % vs the same settings without the row):
Materials 0.00–0.14 % (rain neon 0.14 %), Materials + Normals 0.17–0.66 % (tunnel 0.66 %, rain neon
0.65 %, user's options 0.16–0.57 %). Crops (top before, bottom after): `tunnel_road_off_vs_nrm.jpg`
(headlit asphalt grain), `rain_start_off_vs_nrm.jpg` (wet reflection broken up),
`rain_neon_off_vs_mat.jpg` (Materials: not visible by eye), `neon_user_vs_user_nrm.jpg`,
`night_off_vs_nrm.jpg`, `rain_crowd_user_vs_user_nrm.jpg`.

**Verdict (honest)**: subtle. NFSU2's night is baked into the vertex colours and the road textures are
magnified 15–25 px per texel near the car, so relief from the art's own grain shows mostly where a
dynamic light grazes (headlights on the road, tunnels) and on wet roads; Materials alone is barely
visible in these scenes (lamp glints on wet asphalt/metal). Hence no preset includes it; it stays a
manual row with its cost on the detail line.

## Cost (reference desktop GPU, `--bench explore 300` at night, 3 reps interleaved, "world: main view" pass)
| tier | 1080p | 4K | iGPU |
|---|---|---|---|
| Materials | +0.04–0.06 | +0.04 | not measured (est. ~10×) |
| Materials + Normals | +0.06–0.08 | +0.08–0.10 | not measured |
CPU: the lamp clusters (≈ Dynamic Lamps' build, ~0.03 ms) while the row is on; generation once per
texture (above). Raw: internal evidence: surface-maps/cost-explore.txt. The machine was heavily loaded
(load average 90–300 from other jobs); whole-frame medians not used.

## Native byte-identity
`scripts/perf/perf.sh parity`, base (this copy before this work, full release) vs this work (full release),
row off: **PARITY: all gates passed** — Original dumps of explore/race/racing/drive/rain at 1280×960
and 3840×2160 AE 0 (10 sets), the 144 Hz window path AE 0, Modern/Ultra AE 0 (the lamps' module
with Surface Detail off: g_surface.on = 0 leaves every lamp term as before), 15 shots AE 0, WAVs
identical, state hashes identical over firstrace, firstdrag, postrace, the playthrough (82 493 steps)
and the parity scenario (internal evidence: surface-maps/parity-base-vs-new.txt). How it is kept: every
Surface Detail line is inside world.wgsl's LAMPS blocks (the native module strips them); the extra
group-0 bindings 9–10 and the immediates' padding values are unread by the native shaders.

## Tests
`surface_maps::tests` (classes from names/draws, grain gets a map / painted shapes and flat do not,
painted light does not tilt the map, sign convention, cache bytes round trip), `tests/surface_maps.rs`
(Bayview's real textures: classes, verdicts, map statistics, reviewed rejections; skips without
NFSU2_ROOT), `qol::tests::enhancements_round_trip` (51 deviations), options page row order.

## Cars (2026-11-05)
Same row, **Materials + Normals** only (no new row or label id). Nothing in SPEED2.EXE: car.efx
lights per vertex (harmonic light, docs/formats.md); everything here is a clone deviation.

**What gets a map** (`surface_maps::car_class(texture, light material)`): car textures are atlases
(the skin holds paint, grille, trim, interior; the tyre texture holds rim and sidewall), so the
class comes from the draw's light material and the texture's name:
| class | strength | draws |
|---|---|---|
| CarTyre | 1.0 | RUBBER on *_TIRE / *_WHEEL (sidewall lettering, tread) |
| CarMesh | 0.8 | GRILL (skin's grille and vents, GRILL_0x), BRAKEDISC* |
| CarBadge | 0.6 | *_BADGING (DECAL), LICENSEPLATE |
| CarInterior | 0.7 | INTERIOR, DULLPLASTIC on the skin, ENGINE / AUDIO textures, other RUBBER |
| Skip (native) | – | paint (REG/MET/PEARL/CUSTOMPAINT, rims RIMPEARL/USER_RIMS/MAG*), CHROME, glass and lamp covers, lights, vinyls and other decals, CARBONFIBRE |

**Carbon rejected** (chrome-flicker and carbon fixes): the CARBONFIBER panels are glossy; the weave's map turned the a community save
Lancer's carbon hood into a hammered, blotchy highlight at driving distance
(internal evidence: the carbon panels, map off vs on), the
shimmer the chrome-flicker fix measured. Carbon stays native (car_class → Skip). Chrome (16² CHROME) has no texture to map.

**Generator** (`build_car_normals`, `car_map_for`): per texture the UV triangles of its relief draws,
by class, rasterised to a region mask (grown 1 texel, wrapping; texels below the alpha test's
reference 95 left out); outside texels take the masked mean before the high-pass (a region's border
is no edge); each region judged on its own texels (band RMS ≥ 0.012, edginess ≤ 24 for tyre / mesh /
badge, ≤ 13.5 otherwise) and tilted as the world's (`SLOPE` × strength / region RMS, soft clip 2.5
RMS); everything else (0, 0) = flat. At the source size (≤ 512), full mip chain, RG8 snorm.
Census of 5 cars: internal evidence: surface-maps-cars/census-maps.txt (36 regions mapped; engines of the
Lancer / RX-8 and the 350Z's interior rejected as Painted, the 350Z's MISC flat).
Cache: `<cache>/surface-maps/car-<xxh3 of source, regions, version>.rg8` (header w, h; `.none` when
no region passes). Jobs run on the shared rayon builder (`Builder::spawn`) when the car is first
drawn with the row on; maps are also kept in memory by key (`CarShared::surface_maps`), so traffic
of the same model reuses them; headless modes wait for them.

**GPU** (`render/car.rs`, `render/car_surface.wgsl`): the car texture group gets bindings 2–3 (map,
sampler); every texture binds a flat 1 × 1 stand-in until its map arrives. The surface modules are
car.wgsl (or Enhanced Paint's / Wet Car's form) with two hooks in `car_colour` and car_surface.wgsl
appended (`car_source_n`, CarMode's 4th field = row at 2): the mapped normal (cotangent frame from the
derivatives, explicit-gradient sample) re-weights the per-vertex light — diffuse × (1 + 1.5 Δ/harmonic
diffuse, 0.6–1.4), specular highlight × its ratio (0.25–2.5), the env cube direction turned by the
reflection's change. A flat texel returns exactly the native terms. Dynamic Lamps on cars keep the
vertex normal. Enhanced Glass draws (glass) are never mapped.

**Look** (internal evidence: surface-maps-cars/cars-01-05…jpg`, `cars-06-10…jpg`; per row: wheel off / on,
front off / on; the player's real options, `--shot explore`, 1280×960; the finished-career save = the player's Skyline save,
a community save = the player's Lancer): the tyres' tread and sidewall gain clear relief on every car; grilles,
badges and plates change little at these distances (night, harmonic light); paint and vinyls are
unchanged. Pixels changed (AE, whole frame): wheel 0.05–0.4 %, front 0.002–0.8 %.

## SSR sees the mapped normal
* **World**: with the row on (any tier) and texture arrays, the SSR mask pass uses the lamps' module
  (`WorldShared::mode_with(.., lamps)`, pipelines keyed by (Reversed-Z, surface)); its `wet_mask`
  takes `surface_at` first, so on a wet road the mask's normal and roughness carry the map's tilt and
  the material exactly as the planar Wet Roads term does; on a dry road the normal takes a third of
  the map's tilt (`SURFACE_SSR_DRY` 0.35). The dry-road strength hold (ssr.wgsl) and the GPS
  arrow's flag (`fs_overlay_mask`, unchanged) are untouched: only the normal/roughness the trace reads
  change. Off: the native module as before.
* **Cars**: the car mask module with the row on is car.wgsl + car_surface.wgsl + car_ssr.wgsl whose
  world normal is `surface_world_normal` (derivatives taken at the top, before the alpha test's
  discard; `ssr_mask_source(true)`).
User's options, 1920×1080, row 0 vs 2 (fuzz 2 %): explore 0.24 %, intro 0.18 %, qrstart 0.11 % of
pixels changed; no streaks, no arrow reflections, no validation errors.

## Texture packs
When a pack replaces a texture (`NFSU2_WORLD_TEXTURE_PACK`, `NFSU2_CAR_TEXTURE_PACK`, or the
shared HD pack), the map is made from the replacement's top level (PNG, or a DDS decoded), box-
downscaled to ≤ 512 (`fit_pack`: an HD texture's grain would alias into the map), same UVs; keyed by
the replacement's bytes (`pack_cache_key`, `PackSource`), so each pack has its own cached maps and
the original's stay valid. Checked: a 4× SKYLINE_TIRE.png car pack made a new 512² tyre map.

## The car Surface Detail cost and parity
internal evidence: surface-maps-cars/cost.sh.txt → `cost-explore-user-community.txt`: full release build, the player's
real options file (`the player's options file`: SSR 2, lamp shadows 2, …; Surface Detail is **off**
there, so "on" = that file + `enh.surface_detail 2`), the player's a community save (`NFSU2_FLOW_LOAD`),
`--bench explore 300`, 3 reps interleaved, medians; cold = no `surface-maps` cache (world + car maps
made), warm = cached. Load average ≈ 10.
| | 1080p off | 1080p on (warm) | 4K off | 4K on (warm) |
|---|---|---|---|---|
| GPU busy, ms/frame | 2.75 | 3.06 (+0.31) | 10.39 | 11.55 (+1.16) |
| world: main view | 0.49 | 0.68 (+0.19) | 1.67 | 2.34 (+0.67) |
| cars | 0.085 | 0.092 (+0.007) | 0.218 | 0.242 (+0.024) |
| SSR mask | 0.063 | 0.078 (+0.015) | 0.201 | 0.246 (+0.045) |
| VRAM peak, MB | 1228 | 1237 (+9) | 3530 | 3548 (+19) |
| first frame after load, ms | 54 | 83 warm / 441 cold | 70 | 94 warm / 486 cold |
Most of it is this work's world side under these options (the lamps' module already runs for the lamp
shadows; the normals and materials add to it); the car part is ≤ 0.03 ms and the SSR mask ≤ 0.05 ms.
CPU: "car surface maps" (job setup) 0.03 ms once per car; cold generation is on the builder thread.
Car cache: 16 MB for the 10 cars of the review (113 files incl. `.none`).

**Parity** (`perf.sh parity`, <base> vs this copy, both full release, row off):
**PARITY: all gates passed** — Original dumps AE 0 (10 sets), 144 Hz window path AE 0, Modern/Ultra AE 0
(4 sets), 15 shots AE 0, audio identical, state hashes identical (firstrace 4916, firstdrag 15559,
postrace 30779, playthrough 82412, parity 1753 steps): internal evidence: surface-maps-cars/parity-base-vs-new.txt.
Kept by: the native car module is car.wgsl alone (`car_source_n(.., false) == car.wgsl`); the car
texture group's extra bindings are unread by it; maps are only made with the row at 2; the world's
mask uses the native module with the row off.

**Tests**: `surface_maps::tests::{car_classes_from_texture_and_material,
car_map_only_inside_its_regions, pack_replacements_fit_and_key_apart}`,
`render::car::…::surface_module_hooks_car_colour_only_when_on`,
`world_source_strips_the_lamps` (SURFACE_SSR_DRY lamps-only), `mask_module_appends_to_the_native_car_shader`.
Probe: `devprobes/src/bin/car_surface_census.rs CAR… --maps` (classes, verdicts; `CAR_MAP_DIR` writes the maps).

## Open
* Grilles, badges and plates: relief is subtle under the night's harmonic light; no daylight scene.
* Dynamic Lamps on cars still light the vertex normal.
* No iGPU measurement.
* SSR's dry-road tilt factor (0.35) chosen by eye on the explore/intro shots only.

