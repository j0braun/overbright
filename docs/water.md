# Animated Water (clone deviation, opt-in)

The fountains' water is a static texture, in the native game as well. One Graphics row,
off by default (native, byte-identical):

| row (Graphics → Effects) | values | options.txt |
|---|---|---|
| **Animated Water** | Off / On / Realistic / Next-Gen | `enh.water 0/1/2/3` |

`qol::animated_water()` (any tier), `qol::water_tier()`; label id 0xC104E07C
(`options::ANIMATED_WATER`; the tiers' names are strings, no new label ids). Presets: Ultra → On
(cost below); Original / Clean / Modern unchanged; Realistic (section "Realistic" below) is
manual only, in no preset (perf.sh's Extreme keeps `enh.water 1`); so is Next-Gen (the next-gen water, section
"Next-Gen" below).

## What the original does (verified: traced, docs/formats.md "Texture animation and scrolling")
SPEED2.EXE has no water system. Water is ordinary world textures with the texture entry's
scrolling (0x5BCC40 → 0x5B7D50/0x5B7D90 → 0x48B450, entry +0x52/+0x58/+0x5A/+0x5C) and the
flip-books of chunk 0xB0300100, both already in the clone's native path (world.wgsl
`texture_offset`, `TextureRef::Animated`):

| texture | format, blend | native motion | where |
|---|---|---|---|
| `ANM_WATERA_` (+ `_001`…`_013`) | 128², DXT3, opaque | flip-book 14 frames at 12 fps; frame 0 scrolls v −0.0156/s, the others −0.0234/s | fountain basins, the waterfall pools, the UC fountain steps; the industrial park's canals and one UC pond (reflective meshes, below) |
| `TRN_WATERFALL_A` | 64×128, DXT3, alpha blend, order 5 | scroll v +0.9375/s | every fountain's and waterfall's falling water |
| `TRN_WATERFALL_A_LL` | 256², DXT3, alpha, order 5 | scroll v −0.586/s | pipe outflows (XO_WATERPIPEA, XO_GRAINPIPEB), the hills' waterfall |
| `TRN_WATER_SURFACE_A_LL` | 256², DXT1, opaque | scroll v +0.176/s | the hills' streams (sections 124, 136) |
| `ARC_POOL_A_DM` | 128×64, DXT1, opaque | none | swimming pools |
| `TRN_OCEANA_DM` | 64×256, DXT3, alpha | none | the panoramas' far ocean (left native) |

So the "static" look is native: a slow scroll and a 14-frame loop on a low-resolution texture,
no lighting response, no splash. (Not re-observed under Wine in this work: the textures' scroll
values and the flip-book are the verified native path the clone already draws.)

## Water surfaces (`$OB_PRIVATE/devprobes/src/bin/water_census.rs [TRACK]`; asset test `bayview_water_census`)
Bayview, L4RA (top LOD; centres in game coordinates, Z up). L4RD holds the same set (the race
copy of the city); L4RB / L4RG only the panorama ocean; L4RC, L4RF, L4RH, L4RR none.

| place | solids (placements) | water | centre(s) |
|---|---|---|---|
| City Core plaza fountains | XO_FOUNTAINBASEC (4) + XO_FOUNTAINWATC (4), BASEC_2A + WATE (1) | basin + 4-tier cascade + centre jet | (−785, −285), (−526, −27), (603, −176), (−1501, 1142); (−432, −112) |
| Casino round fountains | XO_ROUNDFOUNTAINA (5) + XO_FOUNTAINWTRA arcs (28 incl. park/plaza) | basin + arcing jets | (−326, −547) group |
| Jackpot / casino jets | XO_FOUNTAINBASEB (2) + XO_UC_FOUNTAINWATN (4) | basin + 4 jets | (−154, −615), (−337, −701) |
| Beacon Hill fountain steps | XB_UC_FOUTAINBASE (2), XO_UC_FOUNTAINWATN (11), XO_UC_FOUNTAINBASE (1) | stepped pools + 11 jets | (−827, 714), (−869, 812) |
| Waterfalls / pools | XO_WATERFALLA (15), XO_WATERFALLB (33), XO_WATERFALLPOOLA (5) | small falls into pools | (−570, −150), (−100, −580), (−200, −280), (−1214, 496) |
| Park fountain | XO_PARKFOUNTAIN (no water texture) + XO_FOUNTAINWTRA (5) | arcs into a dry basin | (−114, 1494) |
| Pipe outflows | XO_WATERPIPEA (2), XO_GRAINPIPEB (4) | falling water | (−449, −1540), (−124, −1409); (−856, −1639) … |
| Swimming pools | XB_POOLA (2), XB_POOLB (2), XB_POOLC (1), TRN_MODERNHOTELFPOOL (2) | still pool | (−2046, 2323), (384, −141), (−2072, 2020), (−1180, 1218), (−3112, 2231), (−477, 317) |
| Hill streams / waterfall | TRN_SH_TERRAINA_NR_CHOP_A4_*, A15/A16_* | stream surface + falls | (−2650, 2200), (−2140, 1660) |
| Canals (industrial park), UC pond | TRN_IP_TERRAINA_CHOP_I3/I15/I17, TRN_UC_TERRAIN_CHOP_C6_8 | `ANM_WATERA_` on **reflective** meshes | (−480, −1500), (−1249, 475) |
| Far ocean | PAN_HILLRIDGE etc. | `TRN_OCEANA_DM` | panoramas |

What `water::build` takes (top LOD / LOD B): 141 / 130 placements, 7007 / 4444 triangles, 110 / 106
with falling water, 687 / 645 landings.

## What On does (`src/render/water.rs`, `water.wgsl`)
* **Replaces the water draws of the main view only.** `Draw::water` marks the draws whose texture
  is a water texture (`water::kind_of`; not on a reflective mesh); `WorldRenderer::set_hide_water`
  (set each frame by `water_prepare` in game.rs, = the row) makes the main view's draw list leave
  them out. The road reflection, the car cube and the rear-view mirror keep the native water.
  The reflective canals and pond, the far ocean and the LOD stand-ins stay native (their draw is
  worldreflect with the road reflection, Wet Roads and SSR; replacing it would drop that).
* **Same triangles.** At the first use the water materials of every placement (top LOD and LOD
  B) are copied to world space (`water::build`, deterministic; the census example loads and builds in 0.15 s). Each frame `water::pick`
  takes the records the native main view would draw: drawn section, LOD, not hidden
  (`placement_hidden`) or culled by the main view's flags (`WorldView::Main.culls`), the scenery
  size cull 0x5D64E0 with the main view's size need, inside the view's side planes.
  `NFSU2_WATER_PLAIN=1` draws them with the native shading alone at the shots' time 0: against Off
  it differs in 0.004–0.19 % of pixels (fuzz 3 %, the resampled texture array and the sheets'
  blend order), i.e. the copy is the native water.
* **Surfaces** (basins, pools, streams; opaque, depth written, LESSEQUAL as world.efx): the native
  texel (flip-book frame and scroll at the world clock) × v0 × 2 as the base, sampled through a
  refraction-like shift by the water's slope (0.045 uv per unit slope). The slope is
  **flow-mapped noise** (three octaves at 0.9 / 0.37 / 0.16 m, two phases half a period apart,
  each advected along the flow and reset at its zero weight): basins flow out radially from their
  centre (0.25 m/s), streams along the world direction of their native scroll (0.8 m/s, from the
  triangles' uv gradients), pools are still (half the waves). **Splash rings** around each landing
  (three rings per landing, widening to 1.4 m, fading; 0.22 m wavelength) and white water
  (churn) within ~0.6 m of it; **rain rings** (the wet-roads and SSR work's ring cells) scaled by the rain amount.
  Shading: crests lift the base up to +14 % (nothing where the art is dark), Fresnel (F0 0.02)
  reflection of the **car cube** (the native camera-relative lookup, or world axes with Enhanced
  Reflections; the fog colour when the cube is off), **lamp glints** (the eight nearest
  light-pack lamps, the lamps-and-shadows work's native weight², Blinn-Phong 180, reaching to twice the lamp radius at a
  quarter), fogged as world.efx. Alpha as native (tex.a · v0.a).
* **Sheets** (falling water; the native SRCALPHA / INVSRCALPHA, no depth write): the native layer
  as it scrolls plus a second one falling 1.43× faster, shifted across (the loop no longer
  repeats), streak noise moving with the fall, the bottom 22 % of the fall brighter and broken
  into drops, a wet sheen (lamp glints, a little of the cube), clamped to 1 as native.
* **Landings**: where a sheet's bottom (lowest 8 % of its fall) meets the water, clustered on a
  1.25 m grid per placement, strength by how much lands there, colour = 2 · texture mean · v0.
* **Spray and mist** (the GPU particles work's approach: billboards soft against the scene's depth, lit per corner by
  the lamps; but analytic, no simulation: a fountain is a steady emitter): per landing within
  70 m (the nearest 32) 24 mist puffs (rising, spreading, fading over 2–3.4 s) and 24 drops
  (ballistic arcs, 1.2–2.8 m/s up, streaked along their velocity). Drawn where the GPU particles work's puffs are:
  into the scene after the native particles, or with TAA / upscaling in the late pass at the
  output's size (no ghosting).
* **Clock**: everything moves on the world clock 0x7FB718 (`d.clock`), less the lag of an
  uncapped frame between two updates (`draw_between`; `water::frame_clock`), so shots are
  deterministic (two runs at `NFSU2_RAIN_CLOCK=120` md5-identical, 120.5 differs) and frames
  above 60 Hz move smoothly.

## The casino square's arcing jets
A screenshot (Animated Water Off): the casino square's big arcing jets over the round basins
"look like a frozen texture". View: the casino round fountains (XO_ROUNDFOUNTAINA ×5 around
(−326, −547)) with their arcs, XO_FOUNTAINWTRA_1A_LL_00 (28 placements in Bayview: 23 here and at
the jackpot basin (−337, −701) and the plaza fountain (−432, −112), 5 at the park fountain).
* **The jets** (`$OB_PRIVATE/devprobes/src/bin/solid_uv.rs XO_FOUNTAINWTRA_1A_LL_00`): one material of 72 triangles,
  `TRN_WATERFALL_A` (64×128 DXT3, alpha blend), local extent 28 × 4.3 × 6.9 m; **v runs along the
  arc** (mean |dP/dv| = (0.92, 0.04, 0.32): the jet's length), u across it, v spanning
  −0.37…1.29 (1.7 repeats over 28 m). Placements scale the jet non-uniformly (det 0.05–1.55).
* **The original animates them** (verified by the trace, docs/formats.md "Texture animation and
  scrolling"): the texture's entry has scroll kind 1, v speed +0.9375/s, so 0x48B450's
  TextureOffset moves the texture along the jet at about 0.94 / 0.06 ≈ 16 m/s; no flip-book
  (no 0xB0300100 entry for it). Not re-observed under Wine (the scroll rule and the data are
  traced; nothing was unclear).
* **The clone animates them the same way, Off included: no native bug.** world.wgsl adds
  `texture_offset(frame.clip.y)` with the texture's own scroll (`scroll_params`), the time being
  the game's seconds; the window path's dumps 0.1 s apart (`--bench explore` with a fixed camera
  track, the game clock running) show the streaks moving along every arc
  (`arcs_casino_off_on_*`, left column). Why it reads as frozen: `TRN_WATERFALL_A`'s streaks run
  along v, so a scroll along v slides the streaks along themselves; with 1.7 repeats stretched
  over 28 m the pattern changes little frame to frame. That is the original's look (and the same
  in the clone's shots, which draw the native water at time 0).
* **Animated Water covered them already** (they are `Kind::Sheet`), but as falls: their flow
  coordinate was the height, so both the spout and the landing counted as "where it lands". Now
  (`water::sheet_shape`): a sheet whose top lies inside its v range (15–85 %), with both ends
  well below the top (≥ 40 % of its height, ≥ 0.5 m) and reaching more than 2.5× its height in
  its own unscaled geometry is an **arc**: its flow coordinate is v, toward the end the native
  scroll carries the texture to (−speed_v · dP/dv; the low-v end for +0.9375). Exactly the 28
  XO_FOUNTAINWTRA placements qualify (`bayview_water_census`); falls, lips and the cascades keep
  the height. On an arc:
  - the second layer is three times finer along the jet and modulates the native one (detail,
    not a blur); streaks move with the flow;
  - **pulses**: noise along the jet moving toward the landing makes it thicker and thinner (alpha
    0.62–1.24×);
  - **shimmering edges**: the outer 20 % of u breaks up with fast noise and sparkles;
  - it thins and breaks into drops over the last third before it lands;
  - **splash rings, white water, mist and drops where it lands** (the landings now at the landing
    end only: 609 landings in Bayview instead of 687);
  - **drops breaking off at the apex** (`water::arc_apex`, `apex_drop` in water.wgsl): per arc
    within 70 m (nearest 24) 40 drops, starting within ±1.2 m of the top with the jet's
    horizontal velocity there (from its reach and heights under gravity, ballistic: inferred) plus
    scatter, falling and fading over 0.7–1.3 s, streaked along their velocity.
* Evidence (window path, `--bench explore 54`, camera tracks `track_casino.txt` = the player's view
  across the square, `track_close.txt` = one basin close; frames 6–54, 0.1 s apart; Off | On):
  `arcs_casino_off_on_0.1s.webp`, `arcs_close_off_on_0.1s.webp` (animated),
  `arcs_casino_off_on_strip_0.1s.webp`, `arcs_close_off_on_strip_0.1s.webp` (4 frames stacked).
* Cost of the player's view (`--bench explore 600` on `track_casino`, 3840×2160, 3 runs each, reference
  desktop GPU): Off 2.48 ms, On 2.63 ms (+0.15), the water passes 0.23 ms: inside the row's 0.62 ms.
* Parity gates against main (fast builds): `PARITY: all gates passed` (Original
  dumps, window path, Modern/Ultra, 15 shots AE 0; audio identical; state hashes identical over
  firstrace, firstdrag, postrace, the playthrough (82453 steps) and the parity scenario).
* Tests: `arcs_and_falls` (an arc's shape, flow direction, apex and jet; a fall and a lip are
  not arcs), `bayview_water_census` (the 28 arcs, one apex each, nothing else).

## Realistic (the row's top tier: fountain water closer to a modern game)
Clone deviation, opt-in, manual only. Nothing native to compare with: SPEED2.EXE has no water system
(above). `src/render/water_hq.rs` + `water_hq.wgsl` (appended to `water.wgsl`, whose module, pipelines
and uniform for On are untouched; the tier writes one more uniform). The same triangles as On, the
same pick (`water::pick`), the same clock; only the drawing differs. Passes, into the scene before the
temporal pass (`Water::draw_surfaces` hands over to `water_hq::Hq::draw`), each with its own GPU
timestamp (`graphics: water wet / copy / water / water sheets`):

1. **Wet stone and underwater caustics** (one screen rectangle over the near water bodies; per pixel
   the scene point from the depth, looked up in each body's **wet field**: a 96 × 96 top-down
   distance field to the water's outline with the level of the nearest water, built at the first use
   from the flat surface triangles, `wet_fields`, two chamfer sweeps; 0.12–0.6 m cells; bodies over
   57 m across (the hill streams) get none). Within a ragged 0.35–1.3 m of the edge and up to 0.7 m
   above the water the stone is darker (×0.58) and glossy (Fresnel of the cube, the lamps' glints, from
   the depth's normal); under the water (inside the outline, below the level) moving caustics
   brighten it (added light: sky-ish by the daylight plus the lamps). One pass (`rgb + dst · a`).
   Up to 16 bodies within 90 m.
2. **The scene behind the water** copied at half size (colour, and linear depth in an R32Float): one
   texel of each 2 × 2 block, with MSAA its first sample (the multisampled colour target is now also
   `TEXTURE_BINDING`; nothing drawn changes). A full-size copy, or an MSAA resolve, cost 3–4× as much
   at 4K with the native 8x MSAA (0.85 → 0.3 ms) with no visible gain under the ripples.
3. **Surfaces**: flow-mapped waves as On plus fine capillary ripples, splash rings ×1.25, rain rings at
   two scales (by the rain amount); **refraction** of the copy by the slope, checked against the copy's
   depth (no foreground leaks); **absorption** by the path under the water (σ = 1.1, 0.5, 0.4 /m, pools
   0.6×), scattered body colour = the native shading (2 · tex · v0, the art's tint and bake) — the art
   keeps a 40 % share (the pools' lamps, the fountains' tint); the basins' floors lie a few cm under the
   water plane in the art, so the path is at least the basin's depth (0.45 m, pools 1.4 m); where
   nothing is modelled under the water a **virtual floor** at that depth (the native texture, caustics);
   **screen-space reflections** of the copy (16 growing steps to ~70 m + 5 bisections, faded at the
   screen's edge and with distance; marched only where the Fresnel term exceeds 0.035), the car cube
   where the march misses, Fresnel (Schlick, F0 0.02,
   held to 0.62 at grazing: rippled water); the lamps' glints (sharp 420 + soft 50); white water where
   the sheets land and where the art is brighter than its mean (its flowing highlights); light caught
   in the crests (as On). Opaque, depth written (as the native draw).
4. **Sheets and jets**: On's layers and alpha (the shape), then premultiplied: aerated water (white,
   the native bake's colour and the lamps) where the art is white, at the landing and along fine fast
   streaks (motion streaks), at least 30 %; elsewhere glassy (14 % of a tint, the scene through it); a
   Fresnel rim of the cube, the lamps' glints, sparkles running down. (A refracting version needed a
   second copy of the scene after the surfaces: +0.3–0.9 ms at 4K for a distortion hardly seen on
   thin jets; dropped.)
5. **Spray**: On's billboards, 80 per landing instead of 48 (24 mist, 56 drops).

**Cost** (GPU busy ms/frame, `--bench explore 600`, reference desktop GPU with the native 8x MSAA, tiers
interleaved, 2 runs each, medians; internal evidence, raw internal evidence;
B = the jackpot basin, C = the City Core fountain, N = the Beacon Hill steps (11 jets, the worst
case), X = no water in view):

| view | 1080p Off | On | Realistic | Δ Realistic − Off | 4K Off | On | Realistic | Δ Realistic − Off |
|---|---|---|---|---|---|---|---|---|
| B | 0.627 | 0.682 | 0.798 | **+0.17** | 2.262 | 2.374 | 2.900 | +0.64 |
| C | 0.687 | 0.814 | 0.969 | **+0.28** | 2.600 | 2.725 | 3.498 | +0.90 |
| N | 0.637 | 0.866 | 1.102 | **+0.47** | 2.332 | 2.991 | 3.986 | +1.65 |
| X | 0.635 | 0.622 | 0.619 | 0 | 2.335 | 2.354 | 2.346 | 0 |

Inside the plan's budget (≤ +0.5 ms at 1080p with a fountain on screen). Row cost
`WATER_REALISTIC_COST_4K` = 1.65 (the worst case; integrated GPU ~15 ms there, inferred ×10 as in
the audit). Steps that brought N at 4K from +2.6 to +1.65 ms (by the per-pass timestamps): the copy
at half size, one texel per block (0.85 → 0.3 ms); one wet pass instead of two (0.5 → 0.17); no
second copy and no refraction in the sheets; the lamps in one loop; the march only where it shows.
None of these is visible in the crops (the refraction and reflections are distorted by the ripples
anyway); no shimmer in the frame sequences. CPU: the fields are built once, at the tier's first use (the asset test
`bayview_wet_fields`, the world's load included, runs in 0.14 s); per frame a sort of the picked
records.

Not done (open): planar or ray-traced reflections on the water (the road capture's plane is the
road's; RT traces the world's low LOD for the SSR pass, not for this one), the reflective
canals and pond and the far ocean (left native: their worldreflect draw keeps the road reflection,
Wet Roads and SSR; replacing it would lose that), shadows on the water, spray colliding with cars.
Every constant is by eye against the clone's own frames.

## Next-Gen (the row's fourth tier, past Realistic)
Clone deviation, opt-in, manual only, `enh.water 3` (the row's tier names are strings: **no new label
id**). Nothing native to compare with (SPEED2.EXE has no water system, above); every constant is by eye
(**inferred**). `src/render/water_ng.rs` (CPU: charts, allocation, slots, mist; GPU half `Ng`),
`water_ng_sim.wgsl` (compute, standalone), `water_ng.wgsl` (drawing, appended to `water.wgsl` +
`water_hq.wgsl`). It builds on Realistic: its wet stone, its scene copy, its surface shading (moved into
`surface_hq(SurfIn)`, a pure refactor: Realistic draws the same) and its pick. On's spray billboards are
off at this tier.

**Jets as simulated water.** Every falling sheet and arc of the art (a connected part of a record's
sheet triangles, `water_ng::charts`: 702 in Bayview, built in 0.016 s, the maintainer's probe `water_jets`) becomes
a **chart**: across (u) × along (v, from the source to where it lands) → world position, 16 × 64 texels
rasterized from the triangles. The source end is the end the native scroll carries the texture away from
(TRN_WATERFALL_A +0.9375/s: high v; TRN_WATERFALL_A_LL: low v) — that is the art's own flow: the falls
fall, and the City Core / plaza centre columns (whose v runs up the column) rise. Each chart is timed by
gravity (`v² = v_top² + 2g·Δz` along its middle line; v_top = 1.2 m/s at a fall's lip, the apex's
ballistic speed for an arc) into a 32-entry table. Per frame (`allocate`) the records within 60 m get up
to 48 Ki particles (nearest first, ∝ area / (1 + d/6)², 64 minimum per chart; 128 entries), and every
landing within 35 m 96 splash drops. `cs_particles` places them analytically on the world clock (no
state: shots are deterministic): each runs its chart's path cycle after cycle, in **strands** (ropes of
water; spacing across half the spacing along), present where the art's alpha is (its ragged edges and
breaks move with the water), aerated by the art's brightness along its streak, the breakup (past 55 % of
the path 70 % leave the sheet with their own velocity: drops) and the edges; an arc is a **round jet**
(a disc of 3–8 cm around the ribbon's middle line, widening as it breaks up). Sheets over 80 m² (the
hills' falls) and LOD B get no particles. Every sheet now keeps its native mesh, and the particles draw
only the water that leaves it (section "The jets as the art's sheets plus drops" below); the paragraph
on the screen-space fluid that follows, and the sections "Water polish" (its point 1), "The Next-Gen jets'
brightness" and "The Next-Gen jets as water, not white tubes", describe the earlier drawing, kept as the
record of what was measured.

**Screen-space fluid** (replaced; van der Laan, Green, Sainz 2009; Green, GDC 2010), at the scene copy's half
size: spheres stretched along their motion across the view (the streak of 1/30 s) write their nearest
view depth (an early depth test on the sprite's front plane, no per-fragment depth) and their summed
chord (thickness, aerated thickness, the art's brightness) — both cut against the copy's depth; a
separable bilateral filter (world radius 6 × the nearest particle radius, ≤ 10 px a side) makes one
surface of them; a half-size shading pass: normals from the smoothed depth (bilinear over the valid
texels, the nearer neighbour each way), refraction of the scene copy (taken again, colour only, over the
jets' rectangle after the basins and far sheets are drawn: the jets refract them), absorption
(0.55, 0.16, 0.1 /m), aeration (white: the art's bake, the sky, the lamps), Fresnel reflection of the
cube, the lamps' glints (sharp 600 + soft 60) and **their light through the water** (forward scattering:
a jet in front of a lamp glows); then an upsample into the scene (premultiplied, tested against the
full-size depth). Drawn before the temporal pass (with TAA it is part of the jittered scene).

**Basins with a wave simulation.** The nearest 6 fields within 60 m (Realistic's wet fields) get a slot:
a 128² heightfield, a damped wave equation (0.7 m/s, damping 3 % a step, walls where there is no water
and between levels more than 5 cm apart) stepped at 1/60 s on the world clock; a slot that comes into
reach is warmed up over 150 steps (2.5 s; `schedule`, also when the clock runs back), so a shot at a
clock is the same every run. **Driven by the particles that land in it** (`cs_impulse`: each particle
whose cycle ends in a step pushes the water down by r³ · speed, fixed-point atomics: the sum is
order-free) and by **rain** (12 hashed drops a step at the rain amount); foam is made where it is hit and
churned, ping-ponged and fading. Drawn as a **generated grid** per basin (96² quads over the field,
displaced by the height, the quads where the water is, steps between levels left to the art), with the
art's own uv and vertex colour rasterized per field cell (`field_attrs`: the bake and the flip-book stay
where the original has them), the simulation's slope and foam in Realistic's surface shading (its
reflections, refraction, absorption, caustics, plus the flow ripples and capillaries finer than the
cells); the native lips of a simulated basin (its surface triangles that do not lie flat) stay as
Realistic draws them.

**Mist** (`render::volume_fx`, the volumetric smoke's module, unchanged): 14 puffs a landing within 45 m (24 landings),
analytic on the clock (rising, spreading, thinning over 2.2–3.6 s), thin and white (`MIST_LOOK`), lit by
the eight nearest lamps (Modern Lighting's falloff when it is on), in a 160 × 160 × 32 grid of 0.35 m;
drawn where the spray was (before the temporal pass, or after it with TAA).

Passes (timestamps `graphics: water wet / copy / water ng sim / water / water ng copy / spheres / filter /
shade / composite / mist`). Diagnostics: `NFSU2_WATER_NG_DEBUG=1` (counts), `NFSU2_WATER_NG_VIEW=1..4`
(normal, thickness, aeration, depth), `NFSU2_WATER_NG_LOOK=k=v,…` (tuning).

**Cost** (GPU busy ms/frame, `--bench explore 600` on the orbit tracks, reference desktop GPU, default settings
(native 8x MSAA), tiers interleaved; internal evidence, raw internal evidence):

| view | 1080p Off | Realistic | **Next-Gen** | Δ NG − Off | 4K Off | Realistic | **Next-Gen** | Δ NG − Off (Realistic's) |
|---|---|---|---|---|---|---|---|---|
| B (jackpot basin, 4 jets) | 0.622 | 0.787 | 1.094 | **+0.47** | 2.201 | 2.828 | 3.359 | **+1.16** (+0.63) |
| C (City Core, cascade + column) | 0.684 | 0.917 | 1.363 | **+0.68** | 2.556 | 3.475 | 4.298 | **+1.74** (+0.92) |
| N (Beacon Hill, 11 jets, worst case) | 0.648 | 1.122 | 1.479 | **+0.83** | 2.287 | 4.100 | 4.701 | **+2.41** (+1.81) |
| X (no water in view) | 0.590 | 0.589 | 0.570 | 0 | 2.178 | 2.161 | 2.166 | 0 |

Against the plan's budget (≤ +1.5 ms at 4K with a fountain on screen): B inside, **C +0.24 ms over, N
+0.9 ms over** (N is over in Realistic too). An earlier single run of C measured +1.57; the table is the
median of two interleaved runs on a shared machine. Per pass (C, 4K): Realistic's wet 0.30 + copy 0.40,
sim 0.04, surfaces + grid 0.18, second copy 0.14, spheres 0.30, filter 0.11, shade 0.14, upsample 0.21,
mist < 0.01. Row cost `WATER_NEXTGEN_COST_4K` = 2.4 (the worst case). CPU: the charts and per-field
attributes are built at the tier's first use (16 ms for the charts); per frame an allocation over the
picked records and 336 mist puffs.

Steps (C at 4K, by the per-pass timestamps): first version +2.37 ms (fluid 1.65); shading at half size
and an upsample (composite 0.55 → shade 0.14 + upsample 0.21), the depth pass without per-fragment depth
(early test) and the second copy colour-only (0.36 → 0.14): +1.70; particle budget 64 Ki → 48 Ki and the
filter ≤ 10 px (spheres 0.35 → 0.30, filter 0.19 → 0.11): +1.57 in that run (+1.74 in the table's). The half-size shading is softer by
construction; the 100 % crops (`*_crop_100pct.png`) show no blocks or halos (the cover is the filtered
thickness, the upsample bilinear and depth-tested), the frame sequences no shimmer.

**Parity**: Off / On / Realistic draw as before (no On/Realistic code path changed except the shared
surface function and two layout visibilities). `scripts/checks/gate.sh … <base checkout> extreme`
(2026-11-05, internal evidence): base and new binaries
(differ); checks PASS, 28 shots 0 differ, window path, enhanced Modern / Ultra / Extreme AE 0, audio and five
simulation hashes identical: **PARITY: all gates passed**. (After the gate only the row's cost hint
constant changed, 1.5 → 2.4.)

**Evidence** (the maintainer's private captures, `scripts/evidence.sh`): `<view>_off_realistic_nextgen_night_rain.webp`
(C R N P W B L S, Off | Realistic | Next-Gen, rows night clock 120 and rain clock 162, 1280 × 960 at half
size); `<C|B|N|R>_motion_realistic_nextgen_0.1s.webp` (clock 120.0–120.7); `B_user_settings.jpg`
(the play setup: TAA, SSR Ultra, Volumetric Fog…; window path, `--bench explore`); crops.
Two runs of a shot are md5-identical. Also shot without panic / validation: MSAA 4x, TAA, Modern
Lighting.

**Self-assessment (honest, against Cyberpunk 2077 / GTA V / FH5 fountains).** What reads as next-gen:
the water is now 3D — jets have volume and refract what is behind them, highlights move over a curved
surface, they break into drops and white water where they land, the basins move with real waves from the
impacts and the rain, foam gathers where the water falls, mist rises, the lamps glint in and glow through
the water. Where it is short of the references: (1) the shapes are still the 2004 art's (wide curtains
and ribbons): GTA V / Cyberpunk author jets as thin columns with crowns, here the casino arcs became
round jets but the umbrella curtains (Beacon Hill) stay curtains, and at night they read glassy-dark with
white streaks rather than the references' bright aerated cores; (2) the smoothed depth gives a slightly
soft, "gel" look in 200 % crops (a narrow-range filter or curvature flow would be crisper); (3) no
planar/RT reflection at the water height: reflections are Realistic's screen-space march (at the water's
own height, the cube where it misses); (4) the jets are lit by the light-pack lamps and the cube only —
not by car head/tail lights, neon or Modern Lighting's clusters (the mist takes Modern Lighting's lamp
falloff); (5) the mist is subtle and only near landings; no drips running down the stone; (6) the City Core
fountain is 0.24 ms and Beacon Hill's 11 jets 0.9 ms over the 4K budget (table). It is a clear step past Realistic in motion and depth;
it is not yet at Cyberpunk's level of authored detail.

Tests: `render::water_ng::tests`: `shaders_validate` (naga: the compute module, the drawing module in
both depth variants, Realistic's module with the shared function), `a_curtain_charts_from_its_source_down`
(source, landing, coverage, area, the fall's time under gravity, the table), `particles_are_shared_by_distance`,
`slots_warm_up_and_keep`, `mist_is_deterministic_and_rises`, `the_rect_covers_the_jets`, `bayview_charts`
(asset: every fountain kind, the 112 arc parts, deterministic); `water_tier_tests::animated_water_has_a_realistic_tier`
(Off / On / Realistic / Next-Gen ⇔ 0–3), `qol::tests::enhancements_round_trip` (3 parsed, 9 held to 3).

Credits (algorithms only; no third-party code vendored, so no THIRD-PARTY-NOTICES entry): screen-space
fluid rendering — W. van der Laan, S. Green, M. Sainz, "Screen Space Fluid Rendering with Curvature
Flow", I3D 2009, and S. Green, "Screen Space Fluid Rendering for Games", GDC 2010; heightfield water —
M. Müller, "Fast Water Simulation for Games Using Height Fields", GDC 2008; the mist through the volumetric smoke's
volume_fx (its credits in docs/smoke-volumetric.md).

Open: planar / ray-traced reflections at the water height; car lights, neon and Modern Lighting's
clusters on the jets; the hill
falls and LOD B keep the mesh; spray does not react to cars.

## Water polish (fine highlights under every AA mode, cost at 8x MSAA, look check)
Clone deviation work on the Next-Gen tier (opt-in, manual). Off draws as before (nothing water is
built); On and Realistic share two exact edits (lamps out of reach skipped, the wet stone's noise skipped
where it adds nothing) and Realistic the reflection march's clip-space stepping (the same points up to
float rounding). Nothing native to trace (SPEED2.EXE has no water system); every number below is measured on the
clone (reference desktop GPU, 4K, explore at night, clock 120, orbit tracks; internal evidence).

**1. The fine highlights (the FSR 2 masks pass's open item).** the FSR 2 masks pass found the jets' fine specular detail gone under TAA,
TAAU and FSR 2 but kept by SSAA 2x and read it as a temporal-resolve problem. It is not: the default
(native 8x MSAA, no temporal pass) loses it exactly as TAA does (crops `C_aa_modes_before_after_100pct.webp`).
Ablations at SSAA 4x showed the highlights are not the surfaces' SSR, lamp glints or cube (removing each
changed nothing): they are the **screen-space fluid** of the jets, and its look depended on the
render size. The bilateral depth filter's radius is the world radius (6 × the particle radius) capped at
10 px of the half-size fluid targets; the world radius is over the cap at every size in these views, so
the smoothing *in the view* doubled at half the resolution: SSAA 4x (2160-line targets) kept the drops'
bumps and their small sharp highlights, native / TAA / FSR 2 Native AA (1080 lines) smoothed them into a
few large blobs, FSR 2 Quality (720 lines) into fewer still. Jitter, mip bias, the reactive mask and the
pass order play no part (a reactive 0.8 / 1.0 changed nothing in the FSR 2 masks pass for that reason).
*Fix* (`water_ng.wgsl` `blur`): the cap is a share of the view's height, `FILTER_CAP_PX` = 10 px at a
2160-line target (5 px at 1080, 3.3 px under FSR 2 Quality at 4K), so every mode smooths the same angular
size — SSAA 4x's. Within 10 m of the eye it widens back toward the old 10 px (`× max(1, 10 / z)`): there a
drop spans more pixels than the cap and the drops would show as single beads (the close-up look of the
old build is kept). Cheaper too (fewer taps: filter 0.14 → 0.09 ms).

Against SSAA 4x (the reference, which the fix leaves as it was), City Core orbit `track_C`, frames
300–323 (motion, ~11 m/s camera), crop of the cascade (2800 × 1200 px at 4K); SSIM / FLIP (NVIDIA FLIP,
LDR) / temporal error (mean |Δframe − Δref| × 255), and Rec.709 luma of the crop (metrics):

| mode | before SSIM / FLIP / temporal | **after** | luma mean / p5 / p95 / p99.5 before → after (ref 100.0 / 33.9 / 192.5 / 245.6) |
|---|---|---|---|
| native 8x MSAA (default) | 0.660 / 0.217 / 19.6 | **0.888 / 0.100 / 7.9** | 102.5 / 35.0 / 195.2 / 251.4 → 101.3 / 34.5 / 193.4 / 244.8 |
| TAA | 0.661 / 0.219 / 19.5 | **0.881 / 0.108 / 8.4** | 102.5 / 35.1 / 194.6 / 244.4 → 101.3 / 34.6 / 192.7 / 238.0 |
| FSR 2 Native AA | 0.653 / 0.218 / 19.5 | **0.880 / 0.102 / 8.3** | 101.9 / 34.9 / 194.3 / 244.7 → 100.5 / 34.4 / 192.5 / 237.7 |
| FSR 2 Quality | 0.605 / 0.251 / 22.5 | **0.790 / 0.147 / 12.2** | 103.4 / 35.6 / 195.8 / 247.4 → 101.4 / 35.1 / 192.5 / 234.2 |
| SSAA 2x (the player's) | 0.763 / 0.162 / 14.2 | **0.943 / 0.064 / 4.9** | 101.2 / 34.6 / 193.7 / 243.2 → 100.5 / 34.3 / 192.9 / 240.3 |

Every mode now shows the reference's fine, sharp highlights (`C_aa_modes_before_after_100pct.webp`,
`_200pct.webp`; motion `C_motion_f2q_taa_msaa_before_after.webp`: FSR 2 Quality, TAA, 8x MSAA, before |
after per row, 0.05 s steps) and the frame-to-frame error vs the reference halves (the old large blobs
swam over the jets). Look rules (crop): Δmean −0.7 … −2.0 toward the reference, p5 −0.5, p95 −1.8 … −3.3,
no lifted blacks, no added glow; the brightest blobs (p99.5) get smaller, not dimmer per highlight.
FSR 2 Quality keeps them but coarser: its fluid targets are 720 lines at 4K, so at 200 % the highlights
are dotted 3 × 3 blocks (open: fluid targets at the display's half size under upscaling, ~+0.3 ms there).
Native-size GPU time of the C view (gpu busy median, one run each): MSAA 4.31 → 4.29, TAA 2.93 → 2.84,
FSR 2 Q 3.98 → 3.99, Native AA 5.96 → 5.88, SSAA 2x 4.61 → 4.57 ms.

**2. Cost at the defaults (8x MSAA).** perf.sh's water scene (Beacon Hill orbit N, 4K, 3 runs interleaved,
`pbench.sh`): Off 2.242 / Next-Gen 4.482 → **4.342** ms GPU busy median, i.e. **+2.24 → +2.10 ms** (budget +1.5); per pass (v1 → final, N): water 0.71 → 0.67, filter 0.14 → 0.09, shade 0.17 → 0.14, the rest unchanged (wet 0.33, copy 0.38, second copy 0.15, particles 0.37, composite 0.27, mist 0.2). Exact changes: lamps out of reach skipped in the glints / lamp terms / the jets'
forward scattering (their contribution is exactly 0), the wet stone's noise skipped past its widest reach,
the screen-space reflection's march stepped in clip space (`clip(p + r t) = clip(p) + clip(r) t`, one
matrix product per pixel instead of one per step; the same points up to rounding), the fluid filter's
smaller cap. **Still over the 1.5 ms budget** (open). What was measured on the way, for the next unit:
* The "8x MSAA decompression" reading of the effects-performance pass / the final effects pass does not hold. Reading the multisampled targets costs
  **per fetch, not per pass**: the half-size copy reading depth for only a 64-px-wide strip cost what no
  depth read at all costs (0.24 vs 0.23 ms; full read 0.38); dropping the colour read or the depth read
  each saves ~0.15 ms. It is the bandwidth of the 8-sample surfaces, paid by every pass that touches them:
  wet stone +0.10, copy +0.37, second copy +0.11, composite +0.13, the mist's march +0.08 over the same
  passes under TAA (City Core view).
* A hardware resolve of the 8x colour costs 0.20 ms at 4K (`NFSU2_WATER_EXP_RESOLVE`-style experiment,
  not kept): more than the copy's colour read it would replace.
* The composite's depth test in hardware (read-only depth attachment, `frag_depth`) instead of loading the
  8x depth: 0.27 → 0.27 ms (its cost is blending into the 8 samples), not kept.
* A shared 1x linear depth could only serve the wet pass and the first copy (the composite needs the depth
  after the basins are drawn): one full read replaces at most the wet pass's (≤ 0.1 ms).
* Content costs at N (8x MSAA, clock 120): the basins' grid shading 0.78 of the 0.96 ms "water" pass
  (screen-space reflections 0.18, lamps 0.18 before the skip, flow-mapped waves 0.13), the particles'
  passes 0.42 (raster and overdraw of 48 Ki stretched sprites: discarding every thickness fragment saved
  nothing in the pass), the mist (volume_fx) 0.2.
Under the player's own anti-aliasing (SSAA 2x, or FSR 2) the tier is inside its budget.

**3. Look check vs modern games (fountains, harbour).** Judged at the City Core, casino, jackpot and Beacon Hill orbits (night, clock 120)
against what Forza Horizon 5 / Cyberpunk 2077 / GTA V fountains show: moving jets with volume, sharp
moving highlights, white water at the landings, rippling basins, mist. After this work the far and middle
views (≥ 10 m) have the fine moving sparkle at every AA mode, which was the biggest visible gap (the jets
read as smooth gel without it). Remaining gaps, not fixed here (each needs its own work, measured above):
(1) **close-up "bead" look** at the Beacon Hill curtains (within ~5 m the 48 Ki particles show as single
large drops with dark rims, as in the base: the depth filter cannot reach a drop's size there with its
10 taps; a sparse kernel that does was tried and aliases into dots and stripes, `N_closeup_base_g11c.webp`
shows the base and this work there: mean |Δ| 0.3 / 255); a narrow-range filter (Truong & Yuksel 2018) or curvature
flow is the known fix; (2) the umbrella curtains stay curtains (the 2004 art's shapes); (3) the
**harbour**: Bayview's sea (PAN_OCEAN, ANM_WATERA_ on a reflective panorama mesh), the industrial canals
and the UC pond are drawn by the native worldreflect path (road reflection, Wet Roads, SSR) at every tier,
not by Animated Water — moving them to the water tiers would lose that reflection path; not changed (open);
(4) no planar / ray-traced reflection at the water height; (5) cost at 8x MSAA (above). Look rules: only
the jets' highlight scale changed (table above); no lift, no new glow, the sodium / neon colours in the
water are the scene's (refraction / SSR of the copy).

Tuning aid (not an option): `NFSU2_WATER_SRC_DIR=dir` loads `water.wgsl` / `water_hq.wgsl` /
`water_ng.wgsl` from `dir` when the tier's pipelines are made (shader experiments without a rebuild).
Tests: `water::tests::shader_source_is_built_in_by_default`, `jets_filter_cap_is_a_share_of_the_view`;
the naga validation of every module (`water_ng::tests::shaders_validate`).

## The Next-Gen jets' brightness
The audit saw Next-Gen turn the casino arcs into grey tubes (jet region p95 190 vs Realistic 234,
internal evidence: crop-fountain). Causes in `fs_fluid_shade` (water_ng.wgsl), seen in the debug views
(`NFSU2_WATER_NG_VIEW=2/3`): the jets are thick everywhere (thickness saturated) but their aeration came
only from the particles' breakup (median 0.47), so most of each arc was clear water refracting the dark
scene; their light was the art's at mip 2 × 0.9 (median 0.79); and the cube's Fresnel reflection
**replaced** the body toward each round jet's rim (`mix(body, refl, F)`), a dark night cube there: the
shading of a dark-rimmed tube. Now the aeration is at least what the art's brightness says (Realistic's
rule, `0.2 + 0.55 · smoothstep(0.3, 0.8, lit)`), the light is Realistic's `mix(lit, 2 · bake, 0.25)` (the
bake taken as full; the particles carry only lit), and the reflection takes the light of the clear part
only (`body · (1 − F (1 − white_a)) + refl · F`). Jet region (y 220..480, 1280 × 960) p95: Realistic 234.0,
Next-Gen 190.4 → **225.3** (limit: within 10). The sim, its steps and the particles are unchanged.
**Looks**: white, bright arcs now, but still wide, smooth ropes without the streaks and gaps of the art
(they read as white tubes more than as water at this distance): the width comes from the particles'
screen-space size and the depth filter, a rendering redesign beyond this fix. Recommendation (the
settings unit's call): drop Next-Gen from the row, keep Realistic.

## Native byte-identity (`scripts/perf/perf.sh parity`, base = this copy before this work, both fast builds)
`PARITY: all gates passed` (2026-11-04, load average 22–26): Original dumps of explore / race /
racing / drive / rain at 1280×960 and 3840×2160 AE 0; the window path at 144 Hz AE 0; Modern and
Ultra (perf.sh's preset strings, without the water) AE 0; 15 shots AE 0, no panic / validation;
explore and race WAVs identical; state hashes identical over firstrace (4810 steps), firstdrag
(17547), postrace (29949), the playthrough (82695) and the parity scenario (1681). By
construction: Off never builds the mesh or the GPU half, `hide_water` stays false (the draw list is
the native one), and `Draw::water` is only read when it is set. Raw: `water-work/parity/`.

## Cost (GPU busy ms/frame, `--bench explore 600`, reference desktop GPU, Off vs On interleaved; internal evidence)
The camera orbits a fountain (`scripts/orbit.py`, 600 frames, eye 18–20 m out): C = the City
Core fountain (basin, 4-tier cascade, jet), N = the Beacon Hill steps (11 jets, stepped pools,
the worst case found), X = the default explore route (no water in view). 4 runs per arm
(one 4K Off run of C hung in loading with all threads in futex waits and was dropped: an Off run,
the shared asset cache being rewritten by another agent at the time; not reproduced). Shared
machine (load average 20–57: other builds, playthroughs and benches).

| view, size | Off frame | On frame | Δ frame | water passes (timestamps) |
|---|---|---|---|---|
| C 1920×1080 | 1.100 | 1.335 | +0.24 | 0.29 (surfaces + sheets) + spray |
| C 3840×2160 | 2.585 | 2.653 | +0.07 | 0.50 |
| N 1920×1080 | 0.989 | 1.439 | +0.45 | 0.56 |
| N 3840×2160 | 2.274 | 2.889 | **+0.62** | 1.09 |
| X 1920×1080 | 1.007 | 0.914 | noise | 0 (nothing picked) |
| X 3840×2160 | 2.381 | 2.319 | noise | 0 |

The frame grows by less than the passes take: the native water draws they replace are left
out of the main view. Row cost (`WATER_COST_4K`, the Graphics page's class): 0.62 ms at 4K, Med;
an integrated GPU (×10, inferred as in the audit) ~5 ms with a fountain filling the screen,
nothing elsewhere. Ultra preset: On. CPU: one pass over 141 records per frame (not measurable
against the noise). Raw: `water-work/cost/`.

## Evidence (the maintainer's private captures)
* `*_off_on_day_night_dusk.webp`: per view Off | On, rows day (sunrise sky, clock ≈ 1 s), night
  (`NFSU2_RAIN_CLOCK=120`), dusk (420, the sunset sky). Views (`scripts/shoot.sh`,
  `NFSU2_SHOT_LOOK` eye → target, `--shot explore`): C City Core fountain (−785, −285), R casino
  round fountains (−326, −547), N Beacon Hill steps (−827, 714), P park fountain (−114, 1494), W
  waterfall pools (−570, −150), B jackpot basin (−154, −615), L hill pool (−2072, 2020).
* `C_motion_off_on_night.webp`: eight frames 0.1 s apart (clock 120.0–120.7). The Off column is
  still because the shot harness draws the native water at time 0 (`FrameParams::time` 0 in
  shots); in the game the native flip-book and scroll move with the frame time.
* `B_on_user_settings_night.webp`: the jackpot basin On with the play setup
  (the play setup's options file: SSAA 2x, SSR Ultra, Volumetric Fog, …), no
  panic / validation.
* `cost.txt`: the cost runs' summary.
* `scripts/`: `shoot.sh`, `crops.sh`, `orbit.py` (bench camera tracks), `cost.sh`. Shot aid added
  in game.rs: `NFSU2_SHOT_LOOK=ex,ey,ez,tx,ty,tz[,fov]` (the shot's camera at the eye looking at
  the target; unset: unchanged).

## Evidence, Realistic (the maintainer's private captures, `scripts/evidence.sh`)
* `<view>_off_on_realistic_night_rain.webp`: per fountain view (C City Core, R casino round
  fountains, N Beacon Hill steps, P park fountain, W waterfall pools, B jackpot basin, L hill pool,
  S hill stream) Off | On | Realistic, rows night (clock 120) and rain (clock 162), `--shot explore`
  at 1280 × 960, half size.
* `C_motion_on_realistic_0.1s.webp`, `B_motion_on_realistic_0.1s.webp`: eight frames 0.1 s apart
  (clock 120.0–120.7), On | Realistic.
* `cost.txt`: the cost runs. The MSAA path (aa_msaa 4), TAA and the play setup
  (the play setup's options file + `enh.water 2`) shot without panic / validation.

`render::water::tests`: `water_textures_by_name`, `animation_time_is_the_world_clock`,
`ranges_merge_where_they_touch`, `mips_halve_to_one_texel`, `bayview_water_census` (asset: the
fountains, waterfalls and pools found, sheets ⇔ landings, reflective meshes left out, built twice
identical), `pick_follows_the_view` (asset: in front / behind / hidden / LOD B / section);
`qol::tests::enhancements_round_trip` (off by default, writes nothing, 39 deviations); options row
order (`graphics_page_steps_and_restores`: Animated Water after Particles in Effects).
`render::water_hq::tests`: `wet_field_measures_the_edge` (inside 0 at the level, a metre out
of a side and a corner), `big_bodies_and_sheets_have_no_field`, `wet_fields_are_deterministic`,
`half_floats`, `bayview_wet_fields` (asset: the basins and pools have fields, ≤ 256, deterministic,
water and margin in each); `frontend::graphics_table::water_tier_tests::animated_water_has_a_realistic_tier`
(Off / On / Realistic ⇔ `enh.water` 0 / 1 / 2); `qol::tests::enhancements_round_trip` (2 parsed,
9 held to 2).

## Open
* Every look constant (flow speeds, ring sizes, spray counts, the second sheet layer, glints) is
  by eye against the clone's native frames; there is nothing native to compare a deviation with.
* The reflective canals and pond and the far ocean keep the native look (see above).
* On: no SSR on the water (the SSR mask holds worldreflect draws and cars only); the water reflects
  the car cube (captured at the car, so far from it approximate). Realistic marches its own
  screen-space reflections (above).
* Spray is not in the road reflection or the mirror; it does not collide or react to cars.
* The Ultra preset turns it on; Modern does not (it is cheap, but the preset's settings-audit set is
  Modern's).

## The Next-Gen jets as water, not white tubes
The shared-medium fix's floor on the aeration (`0.2 + 0.55 · smoothstep(0.3, 0.8, lit)`, the art being white along every arc) made
each jet white water through and through, full cover from 2 cm of chord: opaque white tubes (the 4K Next-Gen
showcase). Now (water_ng.wgsl): the aerated share is the particles' own breakup through a curve
(`smoothstep(0.2, 0.7)` per particle in `fs_fluid_thick`: white streaks along the flow, clear water between), the
cover ramps over 6 cm of chord, white water reflects at most 0.9 of its light, and the lamps' light through the water
is counted once (in the white water's light; clear water shows the lamp's own sprite). Jet region
(the maintainer's private capture, 1280 × 960): Original + Next-Gen p95 225.3 → 224.8 (Realistic 234.0,
within 10; before 190.4), p75 207.9 → 170.2 (the tubes' fill gone), largest ≥ 250 region 350 → 215 px
(Realistic 325). At a common play setup p95 235.5 (Realistic 240.3); the largest ≥ 250 region (1028 px) is the jets'
landing spray in front of the basin's lamp, where the particle wall is thick (before 506 px; Realistic shows the lamp
through thin sheets) — open: the width / density of the landing particles, the rendering redesign still open.

## The jets as the art's sheets plus drops (Next-Gen redraw)
At the top tier the fountains read as wrong in every view of the casino square, the park fountain and the
Beacon Hill curtains: thick, smooth, grey-white **ropes and tubes** with hard silhouettes where the original
has thin, streaky, see-through, tinted arcs; the curtains as **white flame-like blobs**; a wall of spray in
front of the basin lamp. Causes, in the screen-space fluid (the paragraph "Screen-space fluid" above):
(1) the particle budget is shared by every jet in reach (48 Ki over the casino's 23 arcs), so the
particles' radius grows (`radius / √density`, up to 25 cm) and is never under 1.5 px of the half-size target
(10 cm at 30 m in 1080p): each arc becomes a tube 20–40 cm thick; (2) the bilateral depth filter turns the
particles into one smooth surface, so the art's streaks, gaps and ragged edges cannot survive; (3) the
cover reaches 1 within a few centimetres of chord, so a tube is opaque; (4) the landing spray is a thick
particle wall that takes the lamp's light. A tier above Realistic that cannot keep the art's streaks loses to
it at every distance; the fluid could not.

**Now** (`water_ng.wgsl`, `water_ng_sim.wgsl`, `water_ng.rs`, `water_hq.wgsl`'s shared `sheet_hq`):
* **The jets are the art's sheets** (every record keeps its native mesh), shaded by Realistic's sheet
  shading with the Next-Gen look (`SheetLook`): the clear water between the white streaks covers 42 % (Realistic
  14 %) and shows 55 % of the art's own light (its bake and tint: the original's cyan / lavender water, where
  Realistic's nearly clear glass leaves the streaks standing alone as white stripes), the aerated water stays
  near the art's tint (5 % toward neutral white, Realistic 25 %), and past the breakup point (the lower half of
  each fall or arc) the sheet thins where its water leaves it as drops. Realistic passes its old values and draws
  exactly as before (its frames are byte-identical).
* **The drops** (`vs_drop` / `fs_drop`, one pass into the scene where the fluid's five passes were): the
  same analytic particles on the world clock, but only the share that has left the sheet (`misc.z`: the
  breakup past 55 % of the path, the landings' splash drops whole). A particle is one drop of 2–6 mm near the
  eye and stands for a small cluster farther out (×3.5 from 4 m to 30 m); it is drawn at its size when that is
  over 0.7 px, otherwise at 0.7 px with its cover scaled down by the area ratio (a far drop fades, it never
  grows into a blob), smeared along its motion across the view over a **fixed 1/120 s exposure** (the same
  streak at any frame rate) with its cover spread over the streak, soft against the scene depth (5 cm).
  **Lit with a bound**: the art's light in the nearest jet's hue × 0.75, the lamps on it × 0.35, the lamps
  behind it seen through it (forward scattering, cos⁶) × 0.5, the sky × 0.2 — never more than 0.85 of the
  light that reaches it (art + lamps + sky) and never over 0.85 per channel, premultiplied over the scene: the
  spray in front of a lamp is never brighter than the lamp's own light and never stacks past it, whatever the
  target's format (no reliance on an 8-bit clamp).
* **Unchanged**: the charts, the particles' paths and timing, the basins' wave simulation driven by the
  landing particles, the mist, the pick, the reach (60 m), the clock. Removed: the fluid's depth, thickness,
  filter, shading and upsample passes, the second scene copy and the half-size fluid targets.

**Targets** (luma, Rec. 709 on sRGB codes, in a box around each fountain, HUD masked; areas in 1080p pixels;
`docs/evidence/fountains/numbers.txt`, scripts in `docs/evidence/fountains/scripts/`): night (clock 97) and
rain (162), the driving camera at the casino square from 10 / 30 / 80 m, a low passing view, the City Core,
jackpot, Beacon Hill and park views, 1080p and 4K, the Original / each tier / Extreme / Extreme with Next-Gen /
a player's own top options (Next-Gen, Modern Lighting, HDR post, SSR, MSAA).

| target | measure | before → after (a player's top options; Next-Gen alone in brackets) | Original |
|---|---|---|---|
| no large saturated region | largest connected ≥ 250 region ≤ the Original's | casino 10 m 4K 4082 → 1294; casino 30 m 4K 195 → 321; the casino at the player's own view 4K 1668 → 564, 1280 × 960 259 → 853; Beacon Hill 913 → 2696; park 996 → 316; City Core 176 → 944 | 8154; 324; 1713; 2826; 3143; 522; 2678 — met everywhere |
| not brighter | box mean ≤ Original + 5 | casino 30 m 4K 92.3 → 93.7; City Core 86.9 → 93.3; park 88.6 → 85.0; Beacon Hill 120.6 → 117.6 (115.2 → 116.3) | 101.3; 98.5; 91.7; 108.2 — **Beacon Hill +9 over** (Realistic +21) |
| broken, streaky water, not tubes | fine detail (mean \|Y − 3 × 3 mean\| over bright pixels) ≥ Original's | casino 10 m 3.05 → 6.04; 30 m 4K 7.06 → 9.59; Beacon Hill 2.57 → 6.40; park 2.28 → 4.50; City Core 3.64 → 4.80 | 5.06; 9.42; 5.17; 3.18; 4.50 — met |
| no added shimmer / popping | mean \|ΔY\| tick to tick, share of pixels changing by > 40 | casino 10 m 3.66 → 6.38, 1.2 → 3.3 %; Beacon Hill 10.85 → 5.44, 5.9 → 0.5 % | Realistic 6.96, 4.1 %; 5.79, 1.0 % — within Realistic's |

The tube look's numbers: the fine detail roughly doubles at every view and now exceeds the Original's; the
smooth ropes' tick-to-tick change was low only because a tube does not move (the art's streaks scroll along
the jets as in the original).

**The window's format.** A desktop that hands the game a 16-bit float surface makes the last passes blend
unclamped. The fountains at a player's top options at 4K, rendered into Rgba16Float and into Bgra8Unorm targets
(read back clamped): before and after, 0.17–0.23 % of pixels differ, at most 56–86 levels on single isolated
pixels; the fountain box's mean, p95 and largest saturated region are the same in both formats (1668 / 1668
before, 688 / 688 with the redraw at a slightly brighter clear-water setting than the final one). So the ropes and the spray wall were the fountain itself, not the surface format;
the drops' bound does not depend on the format.

**Tiers.** Next-Gen is now Realistic's jets plus what Realistic does not have: drops breaking off and splashing
(lit, energy-bounded), the simulated basins with waves and foam from them and the rain, the volumetric mist,
and sheets that keep the art's tint. It beats Realistic close up (≤ 30 m: the landings, the basins, the tint)
and equals it beyond the particles' reach (≥ 60 m both draw the same sheets). At the casino from the driving
camera the difference is subtle; the tier earns its cost mainly at the basins.

Cost and evidence: below ("Cost of the redraw"). Tests: `water_ng::tests::drops_and_sheets_keep_their_rules`
(the exposure, the bound, both tiers' sheet looks), `shaders_validate` (the new passes in both depth variants);
the depth filter's test went with the filter.

Open: the City Core cascade still reads as striped sheets over stone (the original's glowing, near-opaque
cascade is closer to a real film of water there; Realistic and Next-Gen share the sheet shading); Beacon Hill
is 9 levels brighter than the original (it was 12, Realistic 21); the drops do not take car lights or neon;
TAA was not exercised by the shot harness (its frames came out identical to the ones without it).

**Cost of the redraw** (GPU ms/frame at 4K, `--bench explore 600` on orbit tracks, base and redraw
interleaved, 2 runs each, medians, a shared machine: `docs/evidence/fountains/cost.txt`): the water passes at
the casino square 2.37 → 1.90 (Next-Gen alone) and 2.82 → 2.06 (a player's top options), at Beacon Hill
2.39 → 2.19 and 3.00 → 2.41; the frame 4.71 → 4.44, 21.08 → 20.37, 4.49 → 4.28, 13.92 → 13.48. The sheets'
pass grows (every record keeps its mesh: 0.25 → 0.99 ms at the casino) and the drops cost 0.06–0.09 ms, while
the fluid's passes (spheres 0.78–0.85, composite 0.20–0.28, second copy 0.14–0.28, filter, shade) are gone.
Nothing is drawn or computed with no fountain in view (0.000 ms; frame 2.309 → 2.305).
