# High Precision Post's blur: what was measured (docs/hdr-post.md "Blur where it can be lit")

GPU timestamps per pass, `--bench racing 200` unless named, headless; the desktop GPU at 3840x2160 with
the player's 4K settings (SSAA 2x: scene 5431x3055, Full-Size Glow at 3840x2160, scaled prefilter);
the integrated GPU at 1920x1080, World Detail 3, the Medium (Half-Size Glow 960x540) and High (Full-Size
Glow 1920x1080) levels. Medians of interleaved runs (base, new, base, new ...), the range in brackets.
Other jobs shared the machine (load average 15–42).

## The glow's blur passes alone (a probe on a racing frame: the game's passes against compute variants)
ms per chain, 10 reps interleaved, prefilter + across + down (the compute variants: their own kernels).

| variant | desktop 4K (scaled) | desktop 1080p Full | integrated 1080p Full | integrated Half |
|---|---|---|---|---|
| the game's fragment passes (bilinear pairs) | **1.00** (0.99–1.09) | **0.22** | **9.33** | **1.13** |
| one compute dispatch, 32x32 tiles in workgroup memory, A and C read | 1.63 | 0.36 | 15.4 | 1.17 |
| the same with the prefilter in the kernel (unrolled taps) | 1.05 | 0.23 | 15.0 | 1.17 |
| two compute passes (a row, then a column, in workgroup memory) | 1.73–2.18 | 0.38–0.43 | 8.66–8.75 | 1.42–1.70 |
The bilinear pairs take two texels a fetch and their weights for free; the kernels spend that work as
loads and multiply-adds (192 VGPRs unrolled). Not kept.

## The share of the glow the blur can light (8 x 8 tiles; per 60 frames, racing / rain / free roam)
| | marked (A can be other than 0) | the pass across runs | the pass down runs |
|---|---|---|---|
| desktop 4K, Full-Size | 12–44 % | 23–51 % | 35–68 % |
| integrated 1080p, Full-Size | 17–22 % | 30–38 % | 50–60 % |
| integrated 1080p, Half-Size | 22–28 % | 40–49 % | 59–71 % |
4 x 4 tiles lit less (the pass across 1.25 instead of 1.49 ms at High) but their widening and quads cost
more than that (1.8 ms on the integrated GPU): not kept.

## After SSAA's resolve to the screen's size (docs/ssaa-screen-passes.md; the later tree)
Under SSAA with High Precision Post the glow's source is the image resolved to the screen's size and its
prefilter the 1× one (`fs_overbright_mrt_hdr_flag`). One build with the passes marked apart and the full
passes switched in by a measuring aid (not in the tree), interleaved, 150 frames, the player's 4K settings,
desktop GPU, load 33–39:

| | prefilter | need | across | down | glow chain | frame | runs |
|---|---|---|---|---|---|---|---|
| racing, full passes | 0.25 | | 0.32 | 0.35 | 0.92 | 17.69 (17.66–17.75) | 3 |
| racing, where lit | 0.23 | 0.06 | 0.09 | 0.15 | 0.53 | 17.23 (17.16–17.32) | 3 |
| night rain, full passes | 0.28 | | 0.33 | 0.37 | 0.98 | 21.17–21.24 | 2 |
| night rain, where lit | 0.27 | 0.08 | 0.16 | 0.23 | 0.74 | 20.99–21.04 | 2 |

With the driver's colour compression off (`RADV_DEBUG=nodcc`, the parity gates' picture setting; single runs
that also dumped frames) the clears of B and A write the whole 4K targets: 1.18 → 1.09 racing, 1.17 → 1.22
rain. Frames at those settings (racing, rain; 1080p High on both GPUs) identical to the build without the skip.

## Before → after, the earlier tree (the glow from the box of the scene under SSAA; "post: overbright (high precision)": prefilter, need, blur; GPU busy per frame)
| | glow chain | frame | runs |
|---|---|---|---|
| desktop 4K, racing | 1.088 (1.087–1.093) → 0.704 (0.702–0.705) | 18.81 (18.80–18.87) → 18.39 (18.36–18.44) | 4 / 4 |
| desktop 4K, race start | 1.100 → 0.835 (with the one-tile margin) | 20.65 → 20.37 | 3 / 3 |
| desktop 4K, night rain | 1.156 → 0.928 (with the one-tile margin); 1.146 → 0.886 (without) | 23.13 → 22.90; 22.94 → 22.73 | 3 / 3; 1 / 1 |
| integrated 1080p High, racing | 9.47 (8.74–10.47) → 5.25 (4.84–5.59) | 47.8 → 44.5 | 9 / 7 |
| integrated 1080p Medium, racing | 1.56 (1.50–1.64) → 1.03 (0.95–1.06) | 25.2 → 25.0 | 6 / 6 |
| integrated 1080p Medium, night rain | 1.56 (1.55–1.57) → 1.28 (1.27–1.47) (with the margin) | 28.3 → 28.3 | 3 / 3 |
The integrated GPU's runs are nearly all in its slow state (the build machine's memory contention: SSAO
occlusion 2.9–3.5 ms instead of about 1.0–1.3); the few fast-state runs at Medium gave the glow chain
1.42–1.54 (base) and 1.08–1.19 ms (this path), the same as the slow runs; no High run was in the fast state.
The one-tile margin dropped (exact: the taps sit on their own row; `need_covers_every_texel_the_blur_can_light`)
took 4K 0.741 → 0.708 and integrated High 5.35 → 4.76 ms (3 / 3 runs each).
At Medium the streak buffer's add (the pass that reads A) measures 0.27–0.28 → 0.34–0.37 ms with this path,
also with A cleared, kept, written whole by zeros, or written whole again by the full pass down (all
measured, 3 runs each, `post: streak add` split out): not the way A is written; the cause is open. Rain at
Medium shows no such difference (streaks 1.10 → 1.09).

## Exact
Frames dumped at the 4K settings and compared by md5 with the build before: racing frames 100–104 and 200
(in motion), a still (`NFSU2_BENCH_FREEZE=60`, frames 100 and 149), night rain frames 100–102 and 140, the
race start's frames 60 and 89; on the integrated GPU High and Medium frames 60–62 and 120: all identical,
except racing frame 103, which also differs between two runs of the build before (one step on 21 pixels).
The first form tested the flag on A itself; the driver then kept A in full precision and wrote two of its
channels with another rounding (one half-float step on a few texels: found by comparing the prefilter's
machine code); the flag now tests the prefilter's input.
