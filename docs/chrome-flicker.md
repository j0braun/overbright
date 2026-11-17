# A community save Lancer "chrome trunk" flicker and dry-road reflection strength (2026-11-04)

Issues:
- on a community save's Lancer the chrome trunk's reflection flickered;
- dry roads showed too much reflection.

## Cause of the flicker: the carbon trunk drawn as a mirror (docs/car-materials.md)
The save (a community save; the maintainer's probe `save_parts`) fits these parts to the
LANCEREVO8:
- KIT_CARBON (0x5D) and TRUNK_CARBON (0x60);
- the TRAP CARBON FIBRE hood;
- the APOCALYPSE wide body;
- REGPAINTYELLOW gloss paint.

There is no chrome paint. The "chrome trunk" is the carbon trunk. The clone drew carbon with the
paint's light material at the carbon texture's alpha 1, twice the skin's gloss weight. With the yellow
gloss paint (Enhanced Car Paint: Gloss, SSR roughness 0.02) that is a mirror. Its reflections of the
moving scene (car cube and SSR) changed shape every frame (internal evidence: eight frames of the community-save trunk at 144 Hz).

The original draws carbon with `CARBONFIBER`: env 0.12 and a dull sheen (docs/car-materials.md, traced
and compared in the same Body Shop scene). With that fix the mirror patches are gone.

## Measurements (`--bench explore` with a community save)
New bench aids:
- `NFSU2_BENCH_HZ=n`: frames at that display rate through the windowed loop's cadence + interpolation.
- `NFSU2_BENCH_FREEZE=n`: the car and camera held from frame n on.
- `custom.N V` in NFSU2_BENCH_ENH / NFSU2_SHOT_ENH.

The metric is `scripts/flicker.py` (luma: per-pixel temporal std; mean |second temporal difference|
"acc", which cancels steady motion). Maps come from `scripts/flicker_map.py`.

Trunk lid 200×36+540+612, driving off, acc_mean (acc>16 %):

| setting | 60 Hz | 144 Hz | 240 Hz |
|---|---|---|---|
| native, before / after | 2.73 / 2.54 | 1.42 / 1.36 | 0.85 / 0.81 |
| max setup, before | 6.57 (8.9 %) | 3.95 (4.1 %) | 2.24 (1.6 %) |
| max setup, after | 5.98 (7.8 %) | 3.67 (3.6 %) | 2.09 (1.4 %) |

At 144 Hz, dropping one row at a time from max (after):

| row dropped | acc_mean |
|---|---|
| Sharpen 100 | 1.98 |
| SSAA 2x | 3.20 |
| car reflection 1024 | 3.55 |
| SSR | 3.53 |
| SSAO | 3.49 |
| Enhanced Car Paint, Enhanced Reflections, Reflection Rate, lamp shadows, Wet Roads | ≈ 3.67 (no change) |

Car frozen (`FREEZE=100`), whole trunk, std_mean:
- native 0.13;
- max before 0.52, after 0.45;
- max without Sharpen 0.18.

The env cube schedule is not a factor: Reflection Rate High refreshes every face every frame, and
dropping the row changed nothing. Interpolation is not a factor either: frozen 60 vs 144 Hz is the
same.

**Remaining:** Sharpen 100 is the largest amplifier of the shimmer that is left. CAS sharpens the
512² carbon weave and the edges as they move. That is the row's own trade-off and the CAS pass is
shared by everything, so it is left as is. A lower Sharpen setting removes most of it. **Open:** the
original's carbon trunk in motion was not captured under Wine (only in the Body Shop).

## Dry-road reflection strength
Natively the road always reflects: worldreflect at the texture's authored alpha, wetness never read
(docs/wet-roads-ssr.md).

Measurement (the maintainer's `road_reflection.py`, 1280×960 shots, lower half):
- contribution = shot − the same shot with `custom.7 0` (road reflection off; shots now honour custom.7);
- mean over the pixels where any variant reflects;
- ratio to native.

| row (alone) | explore | qrstart | intro (night) |
|---|---|---|---|
| Road Reflection Full | 1.00 | 0.99 | 1.00 |
| Enhanced Reflections (Schlick weight) | 0.48 | 0.75 | 0.75 |
| Wet Roads (dry road: native path) | 1.00 | 1.00 | 1.00 |
| **SSR Full, before** | **1.24** | **1.26** | **1.48** |
| SSR Full, after | 0.87 | 1.09 | 1.15 |
| max setup, before | 1.17 | 0.66 | 1.06 |
| max setup, after | 0.54 | 0.46 | 0.66 |

`dry_road_reflection.txt` has the share of pixels changed by more than 4 levels. The before and after
masks differ slightly; the ratios are within each run.

Only SSR raised the strength. The traced scene colour is brighter than the planar capture it
replaces. It is not the glow sprites (NFSU2_SSR_NOGLOW: same) and not the fog removal (off: same).

**Fix (`ssr.wgsl` fs_trace):**
- On a dry road (`ssr::Mask::wet`: Wet Roads' uniform not on this frame) a road ray's traced colour is
  held to the planar term's luminance at that pixel.
- Darker detail stays as traced.
- Hits on cars stay as traced. They add cars the native capture lacks, which is what keeps SSR
  slightly above 1 in qrstart and at night (**inferred** from where the excess sits).
- A wet road (real wetness) is unchanged and may go beyond native.

## SSR neon streaks (from the neon unit)
With SSR Half / Full, the neon underglow showed as vertical magenta streaks behind the car.

- **Cause:** the trace added the glow sprites at the hit. Road rays reaching up under the car hit the
  neon tube, an additive mesh outside the mask, and picked up the underglow's sprites. The
  vertically stretched filter then drew them as streaks. The native capture has no cars and none of
  their glows.
- **Fix:** no glow at hits whose mask texel is a car's, and glow only from a 2–4 m path on. The world's
  lamp glows are further off, and the composite keeps the planar's bright flares.
- **Result:** `neon_ssr_streaks_before_after.jpg` (rows SSR alone / max; columns without SSR, before,
  after). The streaks are gone and the pool is as without SSR.

## Native stays native
- With every row off, `--shot explore / intro / qrcars / qrstart` are identical to the base build
  (check lines in the report).
- The carbon fix changes only carbon parts, which is native behaviour.

## Tests
- `car::shared_part_tests::carbon_parts_take_the_carbonfiber_material` (the a community save trunk's lid included).
- `render::ssr::tests::traced_glow_skips_cars_and_short_paths`.

**Open:** no automated GPU test pins the trunk's temporal variance. The bench aids above reproduce it:

```
NFSU2_FLOW_DATA=<dir with NFS Underground 2/a community save/a community save> NFSU2_FLOW_LOAD=a community save NFSU2_BENCH_HZ=144 NFSU2_BENCH_ENH="…" NFSU2_BENCH_DUMP=… NFSU2_BENCH_DUMP_FRAMES=300-347 game --bench explore 347
```

then `scripts/flicker.py 200x36+540+612 frames…`.
