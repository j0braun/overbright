# Ambient Detail: street props, blown debris, tyre dust, steam, moths: how it works

The technical part of [Ambient Detail: street props, blown debris, tyre dust, steam, moths](../../playing/settings/ambient-detail.md): how it works, what it costs, how it was checked and what is open. The player part (the rows and what they do) is on that page.

## Overview

Design goal: "the street
feels alive at night". Nothing here exists in SPEED2.EXE and **nothing was traced**: it is visual only. The simulation,
the native particle system, the cars and every native draw are untouched; the effect reads the cars' drawn state (pose,
velocity, wheel slip / ground contact, headlight colour, brake flag) and writes nothing back. All look constants are by
eye (**inferred**). Original stays the default and byte-identical (see "Parity").

| row (Graphics → Effects) | values | options.txt | label id |
|---|---|---|---|
| **Ambient Detail** | Off / On / Next-Gen | `enh.ambient_detail 0..2` | 0xC104E0B8 (`options::AMBIENT_DETAIL`; unused in main and in the wave's copies — the sparks-and-fire work took 0xD8) |

Not in any preset (the presets reset it to Off: a preset + the row reads as Custom), so the Modern / Ultra / Extreme
presets render exactly as before. The effects-tiers pass's look audit (docs/notes/performance/budget.md "The Next-Gen tiers — costs, tiers, GPU classes"): R1–R6 met (Δmean −0.03) but the
props change ≤ 0.04 % of the frame at the chase camera, so the row stays manual; Next-Gen costs +0.37 ms at 4K
(the max setup) and +7.8 ms on the reference integrated GPU.

## What each tier does

**On** — street props (`render::ambient_props`, ported and finished from the earlier clutter prototype
`NFSU2_CLUTTER`):
* Placement at load, deterministic (SplitMix64 hash per triangle, rayon per triangle in order: the same list every run;
  test `bayview_placement_deterministic`): pavements = the Concrete slots named SIDEWALK / CURB / KERB / PAVE / COBBLE /
  COURTGD (not the `RDP_` road strips: RDP_ROADSTRIP_COBBLESTO is a driven road), gutters = the worldreflect Asphalt
  within 1 m of a pavement triangle. **Nothing on the lanes or the racing line.** Densities 0.10 / m² (pavement), 0.55 /
  m² (gutter); leaves lie in drifts of 3–10. Bayview: 336 719 props, 508 drain grates (248 steaming), 174 road-grill
  vents, placed in about 0.1 s of CPU (0.27–0.45 s before the slot decisions were kept per texture; "When the props
  are placed").
* **Low-poly 3D meshes made in code** (`ambient_props::meshes`, nothing from EA or a mod): drink cans (12-sided, some
  crushed flat), glass bottles (green / brown / clear with a label), paper cups with sleeve and lid, cardboard boxes
  (corrugation, tape), crumpled paper balls (a jittered icosphere, flat-shaded creases), leaves and newspaper / flyers as
  curled double-sided cards (leaf outline, midrib and veins; print columns and a photo block); flat decals for oil
  stains and **cast-iron drain grates** set along the kerb (oriented by the pavement's distance field, 0.4 m off it).
* Drawn at the **end of the main view's world pass** (`WorldCore::draw_pass_faded`; no reflection, mirror or cube
  view): depth-tested and depth-written (SSAO, contact shadows and the temporal pass treat them like the world), cards'
  outlines with alpha-to-coverage under MSAA (a hard test otherwise, smoothed by TAA / SSAA), decals blended without a
  depth write; the target's alpha (the bleach-bypass mask) is kept. CPU culling: 32 m cells round the eye, the drawn
  sections, the frustum; props **shrink** to nothing between 42 and 60 m (no pop). ≤ 32 768 instances, 7 meshes, ≤ 14
  draws.
* Shading (`ambient_detail.wgsl`): world.efx's `colour × baked v0 × 2` from the triangle under the prop (it is as bright
  as the road around it), less on faces turned down, **plus the scene's direct lights**: the 16 nearest light-pack lamps
  with their falloff (Modern Lighting on: its physical windowed falloff, as `volume_fx`), each near car's headlight cone
  (the car's headlight colour) and tail lights (brighter braking); wrapped diffuse, light through leaves and paper, a
  Blinn-Phong lobe for cans, bottles, lids and the wet grate. Wet road (`Weather::road_wet`): paper and leaves soak
  darker, everything glossier. Then the frame's native fog.

**Next-Gen** — everything of On, plus (`render::ambient_detail`):
* **Blown leaves and paper**, simulated on the CPU per drawn frame (steps ≤ 1/60 s; visual only) within 40 m of the eye:
  each car's air (`car_air`: a side wash pushing outward along its body, the trailing wake dragging air along and up,
  fading with height; **inferred**) plus the wind (volume_fx's base wind, **inferred**, with slow gusts up to ~3.4 m/s
  now and then). A card lifts when the air beside it exceeds its threshold (leaf 2.3, paper 3.0 m/s × 0.85–1.45 per
  prop), then flies with drag toward the air (heavier vertically: ~1 m/s fall), lift ∝ |air|², flutter and tumble, and
  lands on the real ground (`ambient_props::Ground`: the road and pavement triangles in a 2D grid, the highest under it —
  a leaf blown off the kerb lands on the road), skids, lies flat again and **settles where it landed** (it stays moved;
  ≤ 3000 moved props, the farthest settled ones out of sight go back). Test `leaf_lifts_and_settles`.
* **Grit** chips flicked up by spinning tyres (slip > 0.3) and by any tyre on gravel, ballistic, bouncing (≤ 600).
* **Moths** round the 6 nearest street lamps within 28 m: 7 each on erratic orbits 0.35–0.9 m from the light, wings
  flapping, lit through by the lamp's colour.
* Through the volumetric module (`volume_fx`, docs/notes/graphics/smoke.md "Volumetric effects and the volumetric Cinematic smoke"; its **own** grid of 128 × 128 × 32 voxels of
  0.5 m and its `ParticlePool`, marched one step coarser than the smoke — `VolumeFx::set_coarser`, the only addition
  to volume_fx besides COPY_SRC on its particle buffer for a readback test):
  * **Tyre dust** behind each wheel on the ground of a car within 35 m on a **dry** road (none above road_wet 0.15):
    thin lit puffs rising slowly, more with speed and wheel slip, ×3 on gravel / dirt (`is_gravel`).
  * **Steam** from the steaming drain grates and Bayview's road grills (RDP_ROAD_GRILL_WH, found with
    the maintainer's probe `ambient_census`; no manhole texture exists in Bayview): the 10 nearest within 45 m, each breathing on its
    own slow pulse, buoyant, drifting with the wind, stirred by the cars' wakes (volume_fx's wake field), lit by the
    lamps, headlights and tail lights with self-shadowing and multiple scattering. On the first frame the vents have
    been steaming for 4.5 s already (warm-up steps), so a load or a still never shows bare vents.
  * Composited with the other particles (`WorldRenderer::draw_ambient`, after the Cinematic smoke): into the scene before
    exposure / grade / HDR, in the temporal pass's late path with TAA (no ghosting).

## When the props are placed

* **The slot decisions once per texture** (`ambient_props::SlotUses`): which list a material's triangles go to (pavement,
  gutter road, other road piece, grill, none) and whether they are loose ground read only the texture's name, blend,
  cut-out flag and size and the solid's worldreflect flag, so the decision is made once per (texture, worldreflect)
  instead of once per material of every placement (the name tests were ~0.2 s of the city's ~0.3 s). Same triangles in
  the same order: `kept_slot_decisions_give_the_same_props` compares them with the decisions made per material.
* **What the props are made from** (`ambient_props::SurfaceInputs`, `AmbientDetail::made_for`): of a world, the
  surfaces read the solids and textures (as loaded for the world's life) and each placement's hidden flag, solid,
  section and transform. A placement whose solid has no slot of use gives nothing whatever its state, so the record
  keeps the solids of use and, for every placement drawing one, its hidden flag, section and transform (bit for bit).
  Equal, the props made again would be the same list.
* **Made ahead and kept**: the menus' preload makes the props for the world as loaded (`WorldRenderer::warm_drive` on
  the preload's thread). A drive's load then sets up the race's or free roam's barriers (`World::set_up_barriers`, the
  backward race's mirrored barriers, `World::set_track`'s world animations) and its warm-up (`AmbientWarm::run`, a job
  of its own beside the world renderer's) keeps the props if the world still gives them, else makes them again for
  the drive's world. The first frame (`ensure_ambient`) checks once more and makes them in place only if the world
  changed after the warm-up (no path found that does). In the city (L4RA, and L4RD) no scenery group, mirrored
  barrier or world animation holds a surface of use (2 316 of 48 349 placements draw one; all 146 set-ups its groups
  name give its surfaces as loaded, `surface_inputs_follow_the_barriers`), so every city drive draws the props made in
  the menus. On a drift track (L4RC: 22 of 40 set-ups), some drag strips (L4RB: 6 of 30) and the URL circuits (L4RG:
  12 of 46; no pavement there, but the ground the debris lands on changes) a race switches road pieces on and the
  load makes them again.
* **Kept for the next drive**: a world kept for the next drive (the last race's region: the drive itself for the same
  car, its world and renderer for another car, or the world kept beside the menus after a car change, made ready there
  or taken at once) carries the props the last drive drew. The new drive checks them the same way
  (`WorldRenderer::new_drive_ambient` once its barriers are set up, then the warm-up and the first frame), so a Quick
  Race on another drift track, drag strip or URL circuit of the region draws its own props, placed by the load; where
  the surfaces are as before (every city drive) the props are kept as they are, with the debris they moved. The check
  is the record's comparison (no placement). The other things a kept world carries were checked the same way
  ("Kept-world caches" below).
* `tests::first_use::ambient_props_made_ahead_reach_the_first_frame`: after the menus' work ahead, free roam and a city
  circuit place no props in their load or first frames, a drift track's load places them once and its frames none;
  each time the drawn props equal props placed then for the drive's world.
* `tests::first_use::a_kept_world_draws_the_next_drives_props`: Quick Races one after the other on L4RC's drift tracks
  4312 → 4311 (same car) → 4313 (another car) → 4314 (after a car change, the world made ready in the menus) → 4315
  (after a car change, at once), on L4RG's URL circuits 4701 → 4712 and on two city circuits: each drift and URL load
  places the props once and its frames none, the two city circuits none; each time the drawn props, vents and ground
  (`AmbientDetail::placement_digest`) are those placed then for the drive's world. Before this check the second to
  fifth drift races and the second URL circuit drew the last race's.

## Kept-world caches

What a world kept for the next drive carries that is made from the barriers or the sections shown, and what each does
with a new drive's barriers (checked in the code; the water by a data test):

| cache | made from | a new drive on a kept world |
|---|---|---|
| Ambient Detail's props, vents, ground | the surfaces' placements: hidden flag, section, transform | checked against the record, placed again when it differs (above) |
| Animated Water's mesh (`Water::for_new_drive`, the menus' kept water) | each water placement's flags and transform when built | the same for every drive: no barrier set-up, mirrored barrier or world animation of any region touches a water placement (`ambient_props::tests::water_is_the_same_at_every_barrier_set_up`) |
| Modern Lighting's loaded emitters (`modern_light_bold::load_emitters`) | the placements not hidden **as loaded** | made from the world as loaded on every path (a new renderer, a kept one's reset, the menus' kept world), so the same as a fresh load's; by design they do not follow a race's barriers |
| the renderer's instances (hidden flag, cull values, bounds) | the placements' flags and transforms | rewritten for the drive's set-up (`update_placement`; a kept renderer's `reset_instances` first) |
| Lamp Shadows' layers (a lamp's depth map, drawn once when it gets a layer) | the scenery shown when drawn | a set-up's switch gives up each held layer whose view holds a placement it shows, hides or moves and the layers draw; drawn again at the next step, the others kept (docs/notes/graphics/lamps-shadows.md, `tests::kept_world::lamp_layers_follow_a_kept_drives_barriers`). In the city the switched barriers are blended (in no layer); on the drag strips, URL circuits and drift tracks road pieces, walls and markers switch under the lamps |
| Light Probes' field (baked one probe at a time, kept for the renderer's life) | the scenery shown (each placement's hidden flag and transform, the race direction's culling) and the fog when baked | at every drive's start each placement is checked against the record of the last start (`WorldRenderer::new_drive_probes`); a probe whose bake box (± `ENV_FAR` round it) holds a switched placement's sphere before or after is given up, those within the burst radius baked again in the first frame (as a fresh load's arrival), the rest one a tick; a race direction other than the one baked under gives up the probes that see the direction-only groups. The fog maps are the world's (no set-up switches them; `reset_to_loaded` restores the same). Free roam ↔ a city circuit gives up none (no placement it switches that the environment view draws lies in a baked probe's box); URL 4701 → 4703 gives up 26–30, drag 4222 → 4223 13 (`tests::kept_world::light_probes_follow_a_kept_drives_barriers`: probe by probe equal to the field made afresh; before, 19 and 12 probes differed) |
| the traced scene (Modern + RT, process-wide per region) | every placement but the markers, hidden or not, at its transform then | not per kept world: the same for a fresh load of the region in the process; a backward race's mirrored barriers are not followed (open, with the traced scene's rework) |
| the environment cube, LOD Fade's and the lamps' section fades | the sections shown each frame | redrawn or stepped every tick (a teleport snaps the fades): nothing carried past a few ticks |

## Code

| file | what |
|---|---|
| `crates/ob-render/src/render/ambient_props.rs` | placement, surfaces, drains / grill vents, `Ground`, the meshes; tests |
| `crates/ob-render/src/render/ambient_detail.rs` | the row's state: car channel (`push_car`), air / wind model, debris / grit / moth simulation, instances, uniform, pipelines, the volume (dust, steam); tests |
| `crates/ob-render/src/render/ambient_detail.wgsl` | the props' shader (materials, lights, fog) |
| `crates/ob-render/src/render/mod.rs` | hooks: `WorldCore::ambient_draw`, `WorldRenderer::{ambient, ensure_ambient, draw_ambient}`, the plan in `plan_views`, the draw in `draw_pass_faded` |
| `crates/ob-render/src/render/volume_fx.rs` | additive: `set_coarser`, COPY_SRC on the particle buffer |
| `src/bin/game.rs` | hooks: `ensure_ambient` before `water_prepare` (window and shot), `draw_ambient` after the Cinematic smoke (3 sites), `ambient_detail::push_car` in `glow_quads`' per-car closure |
| `src/qol/`, `src/frontend/options/`, `src/frontend/graphics_table.rs` | the row |
| the maintainer's probe `ambient_census` | texture census (vent candidates; `NEAR=x,y` the textures round a spot) |

Diagnostics: `NFSU2_AMBIENT_STATS=1` (placement, nearest vents / lamps, per-second instance / moved / grit counts),
`NFSU2_AMBIENT_DEBUG=1` (props magenta, decals green), `NFSU2_VFX_DEBUG=1` (the volume's).

## Cost

GPU busy ms/frame, reference desktop GPU RADV, `--bench … 600`, perf.sh's **Ultra** preset (SSAA 2x: the 4K output's scene is
5431 × 3055) + the row, two runs each (`scripts/cost.sh`, `costs.txt`; the racing pair was measured before the last dust / grit tuning —
dust opacity 0.07 → 0.14, grit only above slip 0.3 — not re-measured):

| scene (4K) | Off mean / p95 | **On** mean / p95 | **Next-Gen** mean / p95 |
|---|---|---|---|
| explore (free roam, night) | 7.10 / 7.73 | 7.11 / 7.82 (**+0.01**) | 7.30 / 7.99 (**+0.20**) |
| racing (4 cars + traffic, dust and debris throughout) | 9.06 / 9.81 | 9.10 / 9.87 (**+0.04**) | 9.52 / 10.40 (**+0.47**) |

Budget ≤ +0.5 ms at 4K: met (racing +0.47, the worst case measured; run-to-run noise of a frame mean ±0.1 ms). Next-Gen's
share is almost all the volume: racing — pool 0.04, build 0.04, march 0.29, composite 0.07 ms. Steps: the first version
(grid 160×160×40 / 0.4 m … then 128×128×32 / 0.5 m, 96 march steps, the smoke's march resolution) cost +0.56 ms in
racing; 56 steps alone −0.02 ms (march 0.41 → 0.39); one step coarser march (`set_coarser(1)`: ¼ instead of ⅓
resolution at this height, then ⅕) −0.10 ms (march 0.39 → 0.29) with no visible change on thin dust and steam (soft
media, bilateral upsample; crops below). The props themselves: world main view +0.01–0.03 ms; CPU: placement once, about 0.1 s on
its thread plus the scatter's rayon workers (0.27–0.45 s before "When the props are placed"), per frame gather + debris simulation well under 0.1 ms at the measured counts (≤ 720 instances, ≤ 240
moved, ≤ 600 grit). Memory: ~24 MB CPU (336 k placed props, the ground grid), 2 MB instance buffer, the volume's own grid and
pool (not measured).

## Evidence (the maintainer's private captures)

1920 × 1080, perf.sh's Ultra preset as the base; captured with `scripts/spots.sh` (explore shots, clock 93 s = night,
170 s rain) and `--bench racing` dumps.
* `spot_{kerb,gutter,plaza,neon,street,rain}_orig_ultra_on_nextgen.webp` — 2 × 2: Original | Ultra / Ultra + On |
  Ultra + Next-Gen, at the prototype's spots (two free kerb cameras, three chase-camera spots, rain). A still renders one frame,
  so it shows no dust or flying debris; the steam is warmed up.
* `close_ultra_nextgen.webp`, `close_crop100.webp`, `close_crop200.webp` — a low camera over the gutter: a can, leaves,
  paper at 100 / 200 %.
* `steam_side_ultra_nextgen.webp`, `steam_top_ultra_nextgen.webp` — a steaming drain at the plaza (the side view's plume
  rises from a drain behind the low wall: see Open), the top view's grate and plume.
* `moths_crop100.webp` — moths round a lamp under the overpass (1–3 px specks at 5 m: subtle by design).
* `racing360_ultra_nextgen.webp` (Ultra | Next-Gen, frame 360), `racing_motion_nextgen_frames360-383.webp` (every second
  frame, 75 %), `racing_frames380-383_100pct.webp` (four consecutive frames, 100 %): debris and grit kicked up by the
  rivals; no shimmer or popping of the props between consecutive frames.
* `costs.txt`, the gate log, the gate log; scripts in `scripts/`.

## Self-assessment (honest, against FH5 / NFS Heat-Unbound / GT7 / Cyberpunk 2077 street dressing)

What works: the props are real geometry (cans read as cans at 2–5 m, leaves as leaves, grates as iron grates), sit only
in gutters and on pavements, take the street's own baked light plus the lamps' and headlights' direct light and the
wet look, and never pop (they shrink out at 42–60 m). Leaves and paper do lift in a passing car's wash and in gusts,
tumble and settle on the real ground. Steam rises from grates as a lit, self-shadowed, wind-drifted volumetric plume. The
whole thing is deterministic, TAA / SSAA-stable in the consecutive-frame crops, and inside the 0.5 ms budget.

Against FH5 / NFS Heat-Unbound / Cyberpunk it is **not at their level**, and it is honest to say the visible gain at the
game's chase camera is small:
1. **Scale and art.** At the chase camera the props are a few pixels each (real-world sizes); the references dress
   streets with authored, photo-textured assets, larger clutter (bins, bags, pallets, cones, newspaper boxes), decals
   and wet grime. Procedural low-poly props with shader materials look clean but simple at 200 %.
2. **Tyre dust is barely visible** in the captures on Bayview's asphalt at race speeds (deliberately thin; heavier on
   gravel). It does not yet give the "dust trail" impression of Heat / FH5's dirt roads.
3. **Debris** reacts to cars and wind but the leaves' motion is a per-card model (no aerodynamic orientation, no
   collisions with cars or each other, no rolling cans); a strong gust can keep many leaves skittering for several
   seconds (127 still moving after 3 s at the race start in the bench).
4. **Lighting**: direct light from 16 lamps + 8 cars, no Modern Lighting clusters and no shadows of their own.
5. Steam vents are few (Bayview has no manhole textures; 174 grills + 248 procedural drains over the whole city) and one
   plume was seen rising from a drain hidden behind a wall.
In short: a sound, cheap, deterministic foundation that makes close views and stops livelier; the "next-gen" impression
at speed needs authored props and stronger, surface-aware dust (follow-ups).

## Parity

Off: `push_car` returns at once, no `AmbientDetail` is made (`ensure_ambient` checks the row), the world pass's plan is
None (one uncontended lock a frame), `draw_ambient` draws nothing; volume_fx's additions do not change the smoke's output
(`coarser` 0, a buffer usage flag). `scripts/checks/gate.sh … <base checkout> extreme`
(2026-11-05, the gate log): base and new binaries
(differ); checks PASS (1132 tests, selftest, 28 shots, first race / drag, post race, playthrough); 28 shots 0 differ,
Original dumps AE 0, 144 Hz window path AE 0, enhanced Modern / Ultra / Extreme AE 0, audio identical, five simulation
hashes identical: **PARITY: all gates passed**.

Ported onto <base> (2026-11-05; main had the shadow-softness pass, the SSR / blur / submit pass since): `gate.sh … <base checkout>
extreme` with CHECKS=full (the gate log): base binary vs new
f57ddcf120fb18b862f5fa9d8f0912b6 (differ); checks PASS (1171 tests, selftest, 275 shots, determinism, first race / drag,
post race, playthrough); 29 parity shots 0 differ, Original / 144 Hz / enhanced (Extreme) AE 0, audio identical, five
simulation hashes identical: **PARITY: all gates passed**. volume_fx's Look / View API was already the smoke integration's in <base>,
so the volume keeps its look unchanged (`scene` 2.2: the scene's light pickup; `near`, `shadow` from `volume_fx::LOOK`;
the main view's native fog via `View::fog`). Ultra / Next-Gen stills at the kerb and street spots match <base>'s.

## Tests

`render::ambient_props::tests`: `distance_inside_and_out`, `rng_is_deterministic`, `names`, `kinds_and_meshes`,
`ground_picks_the_surface_under`, `bayview_placement_deterministic`, `kept_slot_decisions_give_the_same_props`,
`surface_inputs_follow_the_barriers`, `water_is_the_same_at_every_barrier_set_up` (asset; ignored without NFSU2_ROOT);
`tests::first_use::ambient_props_made_ahead_reach_the_first_frame`, `tests::first_use::a_kept_world_draws_the_next_drives_props`
(the game binary; asset and GPU);
`render::ambient_detail::tests`: `car_air_pushes_out_and_drags_behind`, `wind_is_calm_mostly_and_gusts_sometimes`,
`leaf_lifts_and_settles`, `frustum_keeps_what_is_ahead`, `off_by_default`; `gpu_tests::steam_pool_exports` (GPU,
ignored). `qol::tests::enhancements_round_trip` and `frontend::options` tests take the new row.

## Credits (algorithms; no third-party code vendored, so no THIRD-PARTY-NOTICES entry)

* SplitMix64 finaliser: G. Steele, D. Lea, C. Flood, "Fast Splittable Pseudorandom Number Generators", OOPSLA 2014.
* pcg2d hash: M. Jarzynski, M. Olano, "Hash Functions for GPU Rendering", JCGT 9(3), 2020.
* The volume (dust, steam): docs/notes/graphics/smoke.md "Volumetric effects and the volumetric Cinematic smoke"'s credits (Wrenninge 2013, Hillaire 2015/2016, Schneider 2015,
  Henyey–Greenstein 1941, Jimenez 2014, Bridson 2007).

## Open

* Next-Gen's volume shares nothing with Particles Cinematic's: with both on, two grids are built and marched (the effects-tiers pass:
  one shared grid, the emitters appended to the smoke's particle buffer).
* Props are lit by the 16 nearest lamps + 8 cars' lights directly, not by Modern Lighting's full clustered list, and
  cast no shadows of their own (SSAO / contact shadows ground them).
* Steam can come from a drain grate hidden under scenery (the gutter test sees road triangles, not what covers them).
* Moving debris writes no motion vectors of its own (**inferred** from the draw's place in the world pass; not checked
  under TAA with a moving camera).
* Placement is Bayview-only tested; other tracks (none in NFSU2's career) untested.
* The row takes effect in game at once (placement on first use: a one-off hitch when switched on mid-game, behind the
  menu it was switched in: the settings' warm-up, `warm_for_settings`).
