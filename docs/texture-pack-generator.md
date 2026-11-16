# Upscaled world texture pack, generated locally (clone deviation, opt-in)

The graphics wave ("optional upscaled world texture pack, reviewed by eye, via the texture-pack path");
research report: *a non-AI upscale cannot create information that was never present; needs review*.
**Nothing is shipped.** `overbright-upscale` (or Graphics → Image → Generate Texture Pack)
reads the player's own world and car textures and writes a pack into the player's data folder (or a
folder the player chooses); the game uses it when World Texture Pack is On or when
`NFSU2_WORLD_TEXTURE_PACK` / `NFSU2_CAR_TEXTURE_PACK` name a folder (docs/texture-packs.md). Without it the world is the original, byte-identical (the loader
path is unchanged when the variable is unset; no generated texture is in the repo).

Loading image (docs/loading-image.md): `overbright-upscale --ui` also makes `texture-pack/ui`, the loading
screens' and the title screen's textures the HD UI pack lacks (PNG, `Category::UiPicture`), used only with HD UI Art on.

## What the original has (traced: nothing to trace)
SPEED2.EXE draws its DXT1/DXT3/DXT5/P8 world textures as they are in the files (formats.md,
"Textures"). There is no native higher-resolution art; everything below is an **invented**
enhancement that only replaces texel data. UVs, blend, alpha test (ref 95, 0x5BCC40), scroll,
draw order and the CPU-side size are untouched (`texture_pack::replacement`, `texture_layer_with_pack`).

## How to run
```sh
export NFSU2_ROOT=/path/to/Need\ for\ Speed\ Underground\ 2
overbright-upscale --out ~/nfsu2-world-pack            # all 8 regions, ~70 s
NFSU2_WORLD_TEXTURE_PACK=~/nfsu2-world-pack ./game       # use it (restart to change)
overbright-upscale --out ~/nfsu2-world-pack --sheet /tmp/review/top60          # 60 most visible
overbright-upscale --out ~/nfsu2-world-pack --sheet /tmp/review/sign --category Sign --top 24
```
`--out` defaults to `$NFSU2_WORLD_TEXTURE_PACK`. `--tracks L4RA,…` limits the regions; reruns reuse
every file whose source texels and generator version are unchanged (6 s), `--force` remakes all.
Files: `<HASH>.dds` (DX9 DDS, DXT1 or DXT5, full mip chain) and `overbright-upscale.tsv`
(every world texture: category, verdict, output size, format, ΔE, near-road area, source key).
Delete the folder (or unset the variable) to go back to the original.

Measured here (a 16-thread desktop CPU, machine shared with other jobs): **70 s** cold for all eight
regions (2225 world textures, 0.9 s of it loading), **504 MB** on disk (1598 textures upscaled;
they replace 36 MB of the original 66 MB). Locally installed upscalers were checked
(realesrgan-ncnn-vulkan, waifu2x, upscayl, Python torch/onnxruntime/cv2): none present; nothing was
downloaded. The method is pure Rust.

## Method (`src/upscale.rs`, non-generative, deterministic)
1. Decode the top level (the game's own decoder). Texels the game never shows (alpha ≤ 95 for
   alpha-tested art, alpha 0 for blended) get the colour of their shown neighbours (no black fringes).
   For opaque draws the alpha is **data** (worldreflect's reflection amount on roads) and is kept
   as a channel (`AlphaUse`).
2. DXT 4×4 seam softening where the step across a block edge is below a small threshold (real edges
   stay), per category.
3. Lanczos-3, wrap-around (the game samples with WRAP), ×4 or ×2 (the largest that fits the
   category's size cap).
4. Unsharp mask (σ = 0.5·scale) per category, then the **anti-ring clamp**: each output texel pulled
   into the min/max of the 2×2 source texels bilinear filtering would use (1.0 = no halos at all).
5. **Back-projection** (16 iterations, tent-spread corrections, kept in 0..1 and, for signs/alpha, in
   the local source range): the s×s box average of the result converges to the source texel, so the
   original's colours are kept at low frequencies. Checked per texture as CIE76 ΔE between the box-
   downscaled result and the original over the shown texels; a texture with mean ΔE > 1.0 or
   p99 > 4.0 is left out (verdict `Colour`: 30, mostly signs with saturated edges).
6. Masks: a 0/255 alpha stays 0/255 (thresholded after a smooth upscale: smooth contours at the new
   size); other alpha is upscaled like colour.
7. BC1 (opaque or 0/255 mask; punch-through mode) or BC3, own encoder (PCA endpoints + least-squares
   refinement; BC3 alpha picks the better of the 8- and 6-level modes), full mip chain to 1×1 with
   alpha-weighted 2×2 boxes. Sizes are whole 4×4 blocks (DXT-friendly).

| category (from `surface_maps::classify` + blend) | cap | deblock | sharpen | anti-ring |
|---|---|---|---|---|
| Road (worldreflect / road names) | 2048 | 0.06 | 0.4 | 0.85 |
| Sign (SGN/ADS/…: text-preserving) | 2048 | 0 | 0 | 1.0 |
| Building (walls, metal, glass, props) | 1024 | 0.04 | 0.35 | 0.8 |
| Cutout (alpha-tested fences, girders, railings) | 1024 | 0.04 | 0 | 0.9 |
| Decal (alpha-blended windows, grime, icons) | 1024 | 0.06 | 0 | 0.9 |
| Natural (foliage, grass, terrain) | – rejected by review | | | |
| Skip (glows, sky, SFX/ANM, water, emissive, additive) | – not processed | | | |

Also left out: under 16 texels a side (`Small`), flat colour (luma σ < 0.006, `Flat`: 108), 2× over
the cap (`Large`: 6), and four textures rejected by eye (`REJECTED`).

## Load path (`src/render/texture_pack.rs`, `render::texture_layer_with_pack`)
Lookup per world texture: `<HASH>.png` (a hand-made replacement wins), then `<HASH>.dds`, then
`<NAME>.png`. A DDS goes up as-is (BC1/BC3 with its mips) when the GPU has BC; otherwise it is
decoded and given an alpha-weighted RGBA8 mip chain. Oversized (> GPU limit or 4096), truncated or
non-DXT1/DXT5 DDS files fall back to the original. Pack textures go into the world texture arrays
(grouped by format/size like the native ones); with a world pack the arrays are rebuilt at load and
never written to the track cache (unchanged rule).

## Cost (reference desktop GPU, `--bench explore 300`, 3840×2160, Original settings, 2 runs each)
| | base | pack |
|---|---|---|
| VRAM peak (drm-memory-vram) | 1.44 / 1.50 GB | 2.09 / 2.05 GB (**+0.55–0.64 GB**) |
| GPU engine ms/frame | 2.48 / 2.40 | 2.48 / 2.45 (no measurable change) |
| world load | 224 / 163 ms | 386 / 402 ms (+~0.2 s: arrays rebuilt from the pack) |
| process RSS peak | 1.13 / 0.71 GB | 2.03 / 2.01 GB |
Raw: internal evidence. On laptops with 2 GB VRAM the pack does not fit
next to the game at 4K; it is meant for desktop GPUs.

## Review (by eye; the maintainer's private captures)
Sheets: per texture the original (left) and the pack (right), the 48×48-texel window with the most
detail, magnified bilinearly to 256 px (≈ 5 px per source texel, how they look up close at 4K);
cut-outs alpha-tested at 95, decals blended over a checker, roads' alpha ignored (it is the reflection
amount). `sheet-top60-01..05.jpg` (the 60 most visible upscaled textures: triangle area within 20 m of
roads, all regions; names in `sheet-top60-index.txt`), `sheet-{Sign,Road,Cutout,Decal}-01.jpg`.
* **Better**: road paint (lane lines, crosswalks, parking bays) and kerb edges are crisp without
  halos; sign/advert lettering and logos have cleaner edges; fences, girders, window frames and
  chain-link (cut-outs/decals) get smooth, sharp outlines; tiles and panel seams sharper.
* **Same**: smooth concrete, plaster, gradients (little to gain; kept since harmless).
* **Worse → rejected**: foliage, grass and terrain (all `Natural`, 107): noise-like photo art turns into
  sharper texel blobs — grainier and more pixelated than the GPU's bilinear magnification of the
  original. City-light panoramas TRN_CITYGRIDB_DM_/TRN_CITYGRIDC (blocky source, blocks sharpened),
  ARC_CONCFOUNDATION1 and RDP_ROADSTRIP_COBBLESTO (grainier, not more detailed).
* Not fixed (cannot be): stair-steps and JPEG-like noise baked into the 2004 art stay; nothing is invented.
A first version made road textures into flat Voronoi patches (their alpha, the reflection amount, was
treated as coverage and the colour filled from neighbours); the review caught it and `AlphaUse::Data`
plus the ΔE over all texels now guard it (`road_alpha_is_data_not_coverage`).

In game (4K, explore frame 300, top before / bottom after): `road-near-car-4k-before-after.jpg`
(crosswalk paint and asphalt grain: the clearest gain), `sign-billboard-4k-before-after.jpg` (the
Welcome to Bayview billboard: frame and lettering edges cleaner; the source's stair-steps remain),
`terminal-facade-4k-before-after.jpg` (façade: barely different). Verdict: a modest, honest gain,
largest on road markings and signs close to the camera at high resolutions; at 1080p and beyond
~20 m the mips make pack and original nearly the same.

## Tests
`upscale::tests` (categories, low frequencies kept, deblocked within limits, road alpha is data,
signs have no halos and stay within a texel, masks stay masks with no fringe and ±5 % coverage,
BC1/BC3 + DDS round trip, flat/small/large/skip/no-gain verdicts),
`render::texture_pack::tests::generated_dds_loads_with_its_mips_and_a_png_overrides_it`,
`…::world_layer_uses_bc_as_is_or_decodes_with_mips` (BC key with BC, RGBA8 + mips without, no leak
into the car domain).

## In the game: the row, the button, the cars, the VRAM cap
Graphics → Image has two rows after Texture Filtering:
* **World Texture Pack** (`enh.texture_pack`): Off (native) / On / Lean. Greyed ("Not Generated")
  until a pack exists in `<data>/overbright/texture-pack/world` (its manifest) and while one is being
  made; it applies when the city or a car loads next (the world arrays and car textures are built at
  load). On selects `texture-pack/world` for the world and `texture-pack/car` for the cars
  (`upscale_pack::selected`, read by `render::texture_pack::root` at each lookup).
  `NFSU2_WORLD_TEXTURE_PACK` / `NFSU2_CAR_TEXTURE_PACK` still win when set (and keep the row usable).
  Presets leave the row alone (the pack is the player's own; `Preset::apply` keeps it like the QoL rows).
* **Generate Texture Pack** (action row): Select starts `upscale_pack::toggle_job` — world then cars,
  on a background thread with half the CPU threads (at most 8) at nice 10; the detail line shows the
  phase and `done/total (%)`; Select again cancels between textures (the manifest is then not
  rewritten; files already written are valid replacements and are reused next time). Reruns reuse
  every file whose source and generator are unchanged. Done: "N world and M car textures, X MB".
  The bin `overbright-upscale` runs the same library code (`src/upscale_pack.rs`); with no folder it
  writes the same `texture-pack/{world,car}`; `--out DIR` alone keeps the old world-only behaviour,
  `--car-out DIR` adds the cars, `--sheet PREFIX --cars --car-out DIR` makes car review sheets.

Row Off = native: `root()` returns no scoped folder, the loader path is the old one (checked:
`--shot explore` at 3840×2160 byte-identical with no pack folder and with a pack folder present but the
row Off; `md5 eab0e1d5…` both).

### Cars (`upscale::car_category`, `upscale_pack::collect_cars`)
Sources: every car's `CARS/<CAR>/TEXTURES.BIN`, the shared `CARS/TEXTURES.BIN` and
`CARS/WHEELS/TEXTURES.BIN`, and GLOBALB's window glass (`car::car_texture_sources`; 2705 textures).
| category | what | cap | verdict (this install) |
|---|---|---|---|
| CarSkin | traffic cars' shipped `<CAR>_SKIN<n>` (alpha = gloss, kept as data) | 512 | 76 upscaled |
| CarRim | rim faces, `_TIRE`, spinners | 512 | 167 |
| CarMask | the rims' paint masks (`_WHEEL_INNER_MASK`, spinner `_MASK`, found by hash from the part DB, `car::wheel_composite_hashes`); kept even when flat | 512 | 126 (12 fail ΔE: those rims stay native) |
| CarDecal | sponsor decals `_RECT/_WIDE`, BADGING: sign rules (no sharpening, full clamp); flat colour with lettering in the alpha is not "flat" | 512 | 458 (3 fail ΔE) |
| CarPart | ENGINE, MISC, INTERIOR | 1024 | 118 |
| CarGlass | WINDOW*, REAR_DEFROSTER | 512 | 2 (rest flat) |
| CarLamp | opaque head/brake/centre lamp lenses | – rejected by review | 782 |
| Skip | additive glows/lamp glass, SHADOWFE/IG, NEON, DOORLINE* (skin layers), DUMMY_*, DAMAGE, and every racer skin | – | 961 |
Car pack: 947 textures, **155 MB** on disk, 18 s cold (16 threads), 1–3 s on reruns.

What stays untouched: **carbon fibre and chrome** (CARBONFIBER / CHROME live in GLOBALB with the
light materials, not in the sources: never in the pack; the carbon fix's material path is unchanged), the
**racers' composited skins** (`<CAR>_SKIN1` is painted at load from paint, vinyls and the 512² doorline
layers, `car::composite_skin`; its hash equals the shipped name, so the generator refuses any texture
with a racer's `_SKIN1`, `_WHEEL1`, `_SPINNER1` hash). Racer skins are therefore *not* upscaled: doing
it would mean upscaling the composite at every paint/vinyl change in the shops (≈0.15–0.5 s each) or
all vinyl layers (hundreds per car); left open.

**Painted rims** (racers): the rim is composited into `<CAR>_WHEEL1` at load (0x61DEA0 → 0x612EF0,
`car::composite_wheel`). With the car pack, `composite_wheel_pack` composites the pack's source and
mask (same size, 4×) with the same per-texel rule and an alpha-weighted mip chain; the renderer
uploads an RGBA8 texture's own levels when it has more than one (`render::texture_layer`; the
game's textures and native composites have one level, so native is unchanged). Without both files
(or without the pack) the native composite is made as before.

### Review (by eye; internal evidence)
* `sheet-CarSkin-01.jpg` (traffic skins: spoke and grille edges cleaner, colours held — **better**),
  `sheet-CarDecal-01.jpg` (sponsor lettering and logos crisp, no halos — **better**),
  `sheet-CarRim-01.jpg` (tyre tread and rim edges sharper — **better**), `sheet-CarPart-03.jpg`
  (engine bay, misc: **same** to slightly cleaner).
* **Worse → rejected**: the lamp lenses (`CarLamp`, 782). Their art is dithered; the upscale kept the
  chrome rings cleaner but turned the dither into a coarser, grainier pattern (also with sharpening off).
* In the garage scene (`main-dday-350z-rim-4k-off-on.jpg`, `--shot main_dday` at 4K, row Off | On):
  the painted rim's spokes, lettering and tread are a little crisper; paint colour unchanged.
  `explore-peugeot-4k-off-on.jpg` (chase view): nearly identical (badging a touch sharper).
  `model-sheet-350z-bbs-rim-off-on.jpg` (the maintainer's probe `model_sheets`, `--cols 3 --rows 2`): no visible
  difference at sheet scale — the mips the GPU samples there are the box-downscaled original.
  Verdict: a small gain, visible only close up (garage, photo mode); the world pack is the larger one.

### VRAM cap (`upscale_pack::cap`, `render::texture_pack::drop_levels`)
Lean, or On on a GPU under 8 GB, holds Building, Cutout, Decal (world) and CarPart to 2× the
original by dropping the DDS's top mip levels at load (categories from the pack's manifest); roads,
signs, rims and decals keep their scale. The memory size comes from the DRM node with the adapter's
PCI vendor/device (`mem_info_vram_total`, amdgpu/i915/xe; **inferred**: other drivers report nothing
and then only integrated adapters count as small). `--bench explore 300`, 3840×2160, reference desktop GPU,
machine shared (load 12–68), two runs each (internal evidence):
| | Off | On | Lean |
|---|---|---|---|
| VRAM peak (GB) | 1.47 / 1.50 | 2.37 / 2.09 | 1.84 / 2.02 |
| GPU engine ms/frame | 2.51 / 2.46 | 2.53 / 2.55 | 2.48 / 2.50 |
VmHWM: 1.13 GB Off, 1.96 GB On, 1.37 GB Lean. Lean saves roughly a third of the pack's video
memory; the peaks vary by ±0.3 GB between runs here. Load times on this loaded machine are noise
(255–900 ms for Off).

## Open
* Racers' composited skins (see Cars); a 2× composite at load would be the way, with its cost per
  paint change measured first.
* The world `Decal` category is judged flat by colour alone like before (car decals no longer are);
  regenerating the world pack with the alpha rule would add world decals with lettering in the alpha.
* VRAM detection on drivers without `mem_info_vram_total` (NVIDIA proprietary, Windows) falls back to
  "integrated = small"; Lean is the manual choice there.
* 30 world textures and 15 car textures fail the ΔE limit (saturated edges).
* No iGPU / laptop measurement.
