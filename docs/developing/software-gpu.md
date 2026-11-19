# Software-GPU check: lavapipe (2026-11-05)

Not native: a startup check on Mesa's llvmpipe Vulkan driver (lavapipe), as
a machine without a usable GPU, a VM or a CI runner starts the game.
Script: `scripts/checks/lavapipe.sh OUTDIR [BIN]` — `--selftest` and the
shots `main explore race` (`LVP_SHOTS`) on lavapipe, failing on a non-zero
exit, a panic or wgpu validation error, or an adapter that is not
llvmpipe (the log's new `GPU adapter:` line, `render::Gpu::new`). Each shot
is also compared with the default GPU's (reported only). The ICD comes from
`$LVP_ICD`, else `/usr/share/vulkan/icd.d/lvp_icd*.json`; without one the
check prints `LAVAPIPE: SKIP` and passes. `checks.sh full` (or `LAVAPIPE=1`)
runs it as step `lavapipe`.

The reference desktop has no system lavapipe; the check was run with Mesa
26.2.5's `vulkan-swrast` package (CachyOS znver4 build, signature checked,
extracted to a private folder, not installed) — `LVP_ICD=<that folder>/lvp_icd.json`.

## Result (evidence: docs/evidence/q1/lavapipe.txt)

All green: selftest passes (11 s), the three shots render without panics
or validation errors on `llvmpipe (LLVM 23.1.3, 256 bits) (Cpu, Vulkan)`;
the game's software-adapter rules apply (`qol::software_gpu`, the costly
Graphics rows greyed).

**Found, open:** the image differs from the GPU's in the light-streak
buffer (World Detail ≥ 3: Light Trails / Over Bright): the brake-light and
lamp glows come out larger and redder, with horizontal red bands — 0.5 %
of the explore frame, 1.2 % of the race frame by more than 4 %. Bisected:
`NFS_DETAIL=2` (no streaks) matches the GPU (87 px); with one streak frame
(`NFSU2_STREAK_FRAMES=1`, a new shot aid; shots settle 16) it matches
(81 px); from two frames on it diverges. Ruled out: the decay's ZERO /
CONSTANT blend and constant blends interleaved with additive passes
(`examples/blend_constant_check.rs`: identical texels on RADV and
lavapipe, ±1), every optional device feature (`NFSU2_GPU_DROP=bc,
binding_array,format_features,multiview,pipeline_cache,dual_source`, a new
check aid: no change), the fused post path, texture arrays, MSAA. Not yet
found: which state of the streak loop (decay → glow sprites → A·factor add →
Min) lavapipe carries differently from one frame to the next; whether it is
a lavapipe bug or undefined behaviour the clone relies on. The Original look
on real GPUs is unaffected.

## Check aids added

* `NFSU2_GPU_DROP=<feature,…>`: the device requested without those optional
  features (each has a fallback): `bc`, `binding_array`, `format_features`,
  `multiview`, `pipeline_cache`, `dual_source`, `immediates` (no world
  texture arrays; the cars read their draw values by instance index,
  docs/notes/performance/car-draw-index.md), `subgroup` (the lamps' light-cluster loop
  per pixel, docs/notes/graphics/lamps-shadows.md "The cluster loop").
* `NFSU2_STREAK_FRAMES=n`: a shot's streak-buffer settle frames (default 16).
* `examples/blend_constant_check`: the streak decay's blend on any adapter.
