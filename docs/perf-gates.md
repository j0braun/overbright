# Performance harness and parity gates (2026-11-04)

The gates every wave-D unit runs before and after a change: one command for
the numbers (`perf.sh bench` + `perf.sh stats`), one for parity
(`perf.sh parity`), one for a CPU profile (`perf.sh profile`). Method and
the measured profile behind it: `$OB_PRIVATE/research/perf-playbook-2026-11-04.md`
§1, §5. Scripts: `scripts/perf/`. Baseline: internal notes.
2026-11-06: the regression gate's accepted baseline (`perfgate.sh`, `$OB_PRIVATE/perf-baselines/current`) is now
`20261106-0446-b7ee3bcf464b` (main + the effects-tiers pass), replacing the frame-budget gate seed (99e726f); docs/perf-budget.md "The Next-Gen tiers — costs, tiers, GPU classes".

Not native: measuring aids only. Every switch is an environment variable,
off by default, and costs one cached bool/`Option` load when off (proved
below: frames, audio and flows identical, numbers within noise).

## The one-command workflow (a D unit)

```
. scripts/local.env       # NOT for measured binaries: perf.sh build unsets it
P=scripts/perf/perf.sh; export PERF_WORK=<your work dir>
$P build <base copy> $PERF_WORK/bin/game-base     # full release profile (thin LTO, cgu 1)
$P build <your copy> $PERF_WORK/bin/game-new      # also builds bin/pixdiff
$P parity $PERF_WORK/parity $PERF_WORK/bin/game-base $PERF_WORK/bin/game-new   # "PARITY: all gates passed"
$P bench  $PERF_WORK/ab base=$PERF_WORK/bin/game-base new=$PERF_WORK/bin/game-new   # SET, REPS, FRAMES
$P stats  $PERF_WORK/ab --base base --new new     # compare.md; exit 1 on a REGRESSION
$P summary $PERF_WORK/ab new                      # summary.md (noise, MDE), top.md (top passes/parts)
```
Results go to internal notes<unit>/` (compare.md, summary.md, top.md,
gates.txt, the load average). Run benchmarks alone (no builds, no parity
runs of your own at the same time).

## Instrumentation (environment variables)

| variable | what | cost when off |
|---|---|---|
| `NFSU2_BENCH_JSON=file` | `--bench` writes the run as JSON: every frame's update/draw wall + thread-CPU, throttle, whole frame, GPU busy (timestamps); process CPU/frame; **per-thread CPU/frame** (from `/proc/self/task/*/schedstat`, the main thread apart from the helpers named like it, the 4 render workers, …); CPU parts (`prof::acc`) and GPU passes (timestamps) per frame; load, first frame; VmHWM; **own VRAM/GTT peak** (amdgpu `drm-memory-*` in `/proc/self/fdinfo`, sampled every 30 frames in-process) and the gfx engine time/frame from the same fdinfo; adapter, driver, enhancement lines | an `Option` checked once per bench, one `is_some` per frame |
| `NFSU2_TRACE=file.json` | Chrome/Perfetto trace: every `prof::span` (the load phases: `Game::new`, `drive_in`, `World::load`, `WorldRenderer::new: texture arrays`, …) with begin, length, thread and thread CPU, and in `--bench` each frame's update / draw / throttle. Open in ui.perfetto.dev: the loads' critical path across threads (the loading and pacing pass's timeline) | `spans_on()` (cached bool) per span, as `NFSU2_TIMING` before |
| `NFSU2_TIMING=1` | unchanged: the spans on stderr | — |
| `NFSU2_BENCH_FRAMELOG=file.tsv` | `--bench` writes one line per timed frame: update / draw / throttle wall, the bench's update and draw thread CPU, ticks run, the main thread's CPU clock over the frame, the other threads' and the render workers' CPU, pipelines and modules built, the camera's section, traffic and encounter cars, the frame's `prof::acc` parts over 0.25 ms (the tick's phases included: `tick: update`, `tick:   sim step`, `tick: audio`, `tick: particles`, `tick: snapshot before / capture`, …); `scripts/perf/frametail.py file.tsv` gives p50 / p95 / p99 / max and the slow frames with their causes (docs/perf-budget.md "CPU frame time and hitches"). Reading every thread's clock costs the main thread about 0.1 ms a frame, outside the update and draw columns | an `Option` checked once per bench, one `is_some` per frame |
| `NFSU2_BENCH_FRAMELOG_MIN_US=n` | the frame log's parts from n µs on (default 250; 0: every part, for one part's distribution over a run, as the sim step's in docs/perf-budget.md "CPU frame time and hitches") | one cached read per logged frame |
| `--bench boot` | the start up to its first frames, without a window: the process's age when the game's data is loaded (the loader, `Game::new`), the GPU device, the first boot screen's first frame, Enter every frame to the title screen's first frame, then each boot screen's own first frame (docs/perf-budget.md "Load times") | a harness scene |
| `--bench menu [frames]` | the main menu's frame times (update + draw, median / p99 / longest) while the loads and programs made ahead run behind it, and after; `taskset` for a small machine | a harness scene |
| `NFSU2_NO_PROGRAMS_AHEAD=1` | the drive's programs are not made ahead in the menus (docs/first-use-builds.md "Ahead of the load"): the loads make them, as before; the other arm of `--bench menu` and `--bench transitions` | one cached bool a frame |
| `NFSU2_BENCH_TRANSITIONS_SHOPS=1`, `NFSU2_BENCH_TRANSITIONS_RACE=1` | `--bench transitions` runs free roam's first load and then only the shops' transitions (with the part fitted in the shop's menus, those menus paced at two frames in flight as the window's), or only the race's (free roam → race and back, the race after its Engage dialog), for short measuring series | developer harness only |
| `NFSU2_BENCH_TRANSITIONS_KEPT=1` | `--bench transitions` runs free roam's first load and then only Quick Races one after the other in a region, the world kept for the next: a drift track, a URL circuit and a city circuit each from its car select (the region loaded ahead), then another track of the region with the same car and with another (docs/ambient-detail.md "When the props are placed"), for short measuring series | developer harness only |
| `NFSU2_PROBE_BURST_SERIAL=1` | Light Probes' arrival burst waits for the GPU after each probe (the form before the one wait for the whole burst): the A/B of a first frame on one binary, and the reference of the game test that compares the baked field | one cached byte per burst |
| `NFSU2_STATE_HASH=file` | after every `Game::update` (flows, playthrough, bench, shots alike) one line: step, dt bits, xxh3 of the whole state and of its components **global** (mode, game RNG, frame counter, clock, cash), **player** (body pose/velocity/momenta, rest state, drivetrain gear/rpm, pedals, nitrous, camera, world clock, rain), **race** (clock, every racer's progress/lap/finish), **rivals** (bodies, AI controls, RNGs), **traffic** (bodies, lanes, controls). Floats by their bits. `scripts/perf/hashdiff.py A B` names the first step and component that differ | one `OnceLock` load per update |

| `NFSU2_SHOT_CLOCK=s` | `--shot` with the game's clock pinned at `s` seconds (as the bench pins it): the front-end shots that animate on the wall clock (main, career, options, worldmap, ending) come out byte-identical run to run, so they can be gated (`NFSU2_SHOT_CLOCK=10 SHOTS="main career options worldmap ending" GATES=shots perf.sh parity …`) | one `env::var` per shot, read once |
| `NFSU2_ONE_SUBMIT=1` (the SSR / blur / submit pass) | the scene's command buffers are not submitted on their own but with the frame's post and UI in one `queue.submit` (same buffers, same order; AE 0 measured): to tell submission-boundary gaps (other processes' GPU jobs interleave there) from command-buffer boundaries | one cached bool per frame |
| `NFSU2_SSR_FULL=1` | SSR's scene-sized copy and composite over every pixel (before the SSR / blur / submit pass), not only where the mask was drawn (A/B) | one cached bool per SSR frame |

Existing switches the gates use: `NFSU2_BENCH_DUMP(_EVERY)`, `NFSU2_BENCH_HZ`
(the window path's interpolated frames), `NFSU2_BENCH_ENH`, `NFSU2_BENCH_GPU=0`
(no timestamps), `NFSU2_AUDIO_WAV` (offline mixer), `NFSU2_RAIN_CLOCK`,
`NFSU2_WEATHER_HELPERS=n` (the weather's element-wise steps on n render-pool jobs besides the
calling thread, 0 = serial; same values either way; docs/perf-integrated-gpu.md).

## Perf.sh bench: the benchmark set

`SET=quick` (5 configs: racing Orig/Ultra 1080p, explore, race start, racing
Orig 4K), `standard` (default, 19: explore/racing/race/drive/rain ×
Original/Modern/Ultra at 1080p + explore/racing × Original/Ultra at 4K),
`full` (the performance research's 30-config matrix), `igpu` (reference integrated GPU via
`MESA_VK_DEVICE_SELECT=1002:164e`), `loads` (`--bench transitions`).
Presets are the exact `Preset::apply` lines of the performance research's `matrix.sh`. Each label
gets its own asset cache (warmed first, not recorded) and XDG dir. Arms run
**adjacent per config, order rotated each repetition** (ABBA…), so slow
drifts of the machine (other jobs' Wine sessions, clocks) hit both arms
alike. Default `REPS=5`, `FRAMES=600` (fixed 60 Hz step, pipelined, 2 frames
in flight).

## Perf.sh stats: the statistics

* **Unit = the run.** Frames in a run are autocorrelated and share one
  machine state, so the per-frame series are reduced to one value per run
  (median GPU busy, p95, CPU ms/frame per thread group, p99 draw CPU, frame
  wall p50/p99, load, first frame, VRAM, RAM) and runs are compared.
* **GPU from timestamps, CPU from thread CPU.** Both exclude waiting for
  other processes; wall-clock columns (`frame_p50/p99`, loads) are kept but
  marked load-sensitive.
* **Test:** exact two-sided Mann-Whitney U on the run values (normal
  approximation with tie correction when tied). With 5 + 5 runs the smallest
  p is 0.008; with 3 + 3 it is 0.10, so **3 runs can never flag anything —
  use ≥ 5**.
* **Flag rule:** REGRESSION (exit 1) / improvement only when p < 0.05 **and**
  |Δ| > max(tolerance, 2 × noise) **and** |Δ| > the metric's absolute
  floor. Noise = 1.4826 × MAD / median of the runs, pooled over both arms.
  Tolerances: GPU median 2 %, CPU 2–3 %, p95/p99 3–10 %, loads 10 %, VRAM
  1 %, RAM 2 % (`METRICS` in `perfstat.py`). "not significant" = a large
  but unproven change: repeat with more runs.
* **MDE** (`summary.md`): the smallest change the run count can detect,
  1.15 × 2.8 × noise × √(2/n) (α 0.05 two-sided, power 0.8).
* A stored baseline (`--baseline internal notes)
  is indicative only (another time and load); gates are A/B.
* `perfstat.py selftest` checks the exact U distribution and the verdicts.

## Achievable precision on this machine (measured)

Measured from the baseline run (internal notes;
19 configs × 5 runs, load average 9–28 from other jobs' Wine sessions and
builds). Noise = run-to-run 1.4826 MAD / median; MDE = smallest change a
5-vs-5 A/B detects (×0.71 for 10-vs-10).

| metric (per run) | noise, median over configs [worst] | MDE n=5 typical | use for |
|---|---|---|---|
| GPU busy median (timestamps) | 0.7 % [12.5 % explore/rain Original 1080p, 3.5 % racing Modern] | **1.4 %** (≈0.01 ms at 1080p, 0.03–0.2 ms at 4K) | every GPU change; small ones at 4K |
| GPU busy p95 | 2.7 % [34 %] | 5.5 % | spikes |
| own VRAM peak / RAM peak | 1.3 % / 0.7 % | 2.6 % (≈30 MB) / 1.3 % (≈20 MB) | memory units |
| draw CPU p10 (least contended frames) | 4.0 % [19 %] | **≈8 %** (≈0.05 ms) | main-thread CPU changes |
| draw CPU median | 6.7 % [20 %] | ≈14 % | |
| main thread / render workers / process CPU ms/frame (means) | 11.6 / 12.2 / 12.5 % [26–35 %] | ≈24 % | only large CPU changes, or REPS=10–20 |
| update (simulation) CPU | 27 % [64 %] | ≈54 % | not usable at n=5 (0.05–0.24 ms, quantised) |
| scene load ms | 13 % [65 %] | ≈26 % | loads: use `SET=loads` with REPS ≥ 10, and the trace |
| frame wall p99 | 36 % [95 %] | — | not usable on this shared machine |

So on this machine: **GPU costs to ≈1–2 %**, memory to ≈2 %, the main thread
to ≈8 % via the p10 statistic (≈24 % via means), loads to ≈25 %. The CPU
noise is cache/SMT contention from the other jobs' processes (thread CPU
excludes preemption but not a slower core); a quiet machine should reach the
GPU numbers for CPU too — re-measure the table then. GPU clocks follow the
total load: at 1080p Original the GPU idles most of the frame, which is
where its worst noise is; use 4K (or pinned clocks, root) for small GPU
effects. False positives: the rule flags at p < 0.05 per comparison, so a
set of ~200 comparisons can show one by chance (observed: 1 of 209 in the
JSON-on vs off run, a load time the JSON code cannot touch). A flag on a
metric the change cannot affect, or a single one, is confirmed by rerunning
that config with `REPS=10` before it counts.

## Perf.sh parity: the gates

| gate | what | pass |
|---|---|---|
| images | Original, `--bench` explore/race/racing/drive/rain dumped every 100 frames (600) at 1280×960 and 3840×2160 | `pixdiff` AE 0 (every pixel byte-equal) |
| window | the window path's frames: `NFSU2_BENCH_HZ=144` (world at the 60 fps cadence, frames between interpolated by `draw_between`), racing and explore, 720 frames, every 37th | AE 0 |
| enhanced | Modern and Ultra, racing and explore at 1080p | AE 0 unless the unit is approved for a diff: `ENH_MAX_PIXELS`, `ENH_MAX_DIFF`, the report lists pixels and max channel diff per frame |
| shots | 15 standard shots (`SHOTS=` to change; main, career, options, worldmap, ending are left out: they animate on the wall clock and differ run to run with one binary) | AE 0, no panic/validation; a differing shot is re-run twice with BASE and counted only if BASE repeats itself |
| audio | `NFSU2_AUDIO_WAV` of the explore and race shots (offline mixer) | byte-identical WAV |
| sim | `NFSU2_STATE_HASH` over `--flow firstrace`, `firstdrag`, `postrace`, `--playthrough`, and the parity scenario circuit_s1 (`--flow parity`, plan `scripts/perf/plans/circuit_s1.plan` from the parity harness's native run; its clone.tsv rows too) | hash files identical (`hashdiff.py` names the first step/component otherwise). A base binary from before the harness has no hashes: the gate then compares the flows' last log lines and the parity rows |

`ENH_PRESETS` (default `modern ultra`) and `ENH_SCENES` (default
`racing explore`) choose the enhanced gate's runs; `extreme` /
`extreme_fsr` add the enhancements newer than this work's presets (particles,
water, wet car, rain detail, spray, lens drops, motion blur / DOF tiers,
beams, light probes, glass, Surface Detail, smooth cars, World Detail
Extreme; FSR 1 instead of TAA): `ENH_PRESETS="extreme extreme_fsr"
ENH_SCENES="racing explore rain" GATES=enhanced perf.sh parity …`
(`docs/perf-main-thread.md`, the main-thread port).

2026-11-05: `SET=d3` (race start, racing, explore × Original and
`extreme`, 1080p) and `extreme` as a bench preset (`PRESET` part of a
config); `perf.sh profile … extreme|extreme_fsr`; `perf.sh profile` builds
into one target dir per source tree (`target-prof-<hash>`, as `build`:
the same cargo-mtime hazard); `callers.py` works (it was a stub): the
callers chain of a self-time function; `getenvcount.c`, an `LD_PRELOAD`
shim counting `getenv`s per name (per-frame environment reads). Never edit
`perf.sh` while one of its runs is going: bash reads the script as it
runs.

2026-11-05: `user` as a preset (bench configs `scene-user-WxH`, the
enhanced gate's `ENH_PRESETS=… user`): the player's own Enhancements values
(player-settings, the GPU research's USER_ENH); `SET=d11` (racing / explore / rain / race
start × Ultra / Extreme / user at 4K, racing user / Ultra / Original 1080p);
`gate.sh … extreme` takes `GATE_ENH_PRESETS` (default `extreme extreme_fsr`).

2026-11-05: `SET=d4` (explore, racing, rain, race start × Original,
Ultra, Extreme × 1080p, 4K: the GPU profile set) and `SET=d4ab` (racing /
explore / race start × Original / Extreme at 1080p, racing Ultra /
Extreme at 4K: The GPU encoding pass's A/B set); internal notes OUT
LABEL` lists every GPU pass of a bench set with its share per config.

2026-11-05: `SET=gate` (the regression gate's 8 configs,
`scripts/checks/perfgate.sh`), `CONFIGS="cfg …"` instead of a set, presets
`ultra26` (Preset::Ultra of 99e726f), `user` (the player's play setup), `max`
(user + every newest row at its top) and any `PRESET_<name>` (`@MAX@` =
MAX's lines), the scene `water` (explore orbiting the Beacon Hill fountain
steps), `IGPU=1`, `ORDER=alternate`; `perfstat.py gate`; `budget.sh` /
`budget.py` (the frame budget). `docs/perf-budget.md`.

2026-11-05: `NFSU2_VRAM_REPORT=file` (a `--bench` writes the GPU
allocator's live textures/buffers by label, allocated/reserved totals and
the process VRAM/GTT at its end), `NFSU2_GPU_BLOCK_MIB=min,max|perf`
(device memory block sizes; default 4,32; `perf` = wgpu's 128–256) and
`NFSU2_RT_POOL=0` (no shared transient targets) for A/B
(`docs/perf-gpu.md` "Render-target memory").

`pixdiff` (`examples/pixdiff.rs`): files or directories by name, decoded
RGBA8, per file differing pixels / max channel diff / mean abs diff, exit 1
over the limits. Not covered: a real swap chain's acquire/present (the
bench renders offscreen; a windowed run on this desktop is forbidden) —
open, see below.

## Perf.sh profile: naming the top functions and passes

`perf.sh profile OUT SRC SCENE [PRESET] [FRAMES]` builds the same codegen
with `debug = line-tables-only` (`$PERF_WORK/target-prof`), records samply
(the performance research's user-only build; `scripts/perf/samply-perf_group.rs.patched` is the
one-line patch, `perf_event_paranoid` = 2 forbids kernel samples) at 900 Hz
while the bench writes its JSON, then prints per thread the top self /
inclusive functions (`prof.py`, first 4 s = load skipped) and the run's
GPU passes and per-thread CPU. `perfstat.py top OUT` lists the top GPU
passes and CPU parts per config of a bench set. Load critical path:
`NFSU2_TRACE=t.json game --bench transitions` → Perfetto. What a thread waits
for (its workers, the GPU's fence, the submit call) per load stage and in the
first frame: `scripts/perf/blocksample.py run OUT.tsv -- game --bench …` with
`NFSU2_TIMING=1`, then `blocksample.py stages OUT.tsv…` (Linux; it reads its
own child's `/proc` entries about every millisecond). Tracy
(`--features profile-tracy`) stays possible but there is no capture tool on
this machine.

## Zero cost when off (proof)

* **By construction.** With the variables unset: `prof::span` checks one
  cached bool (`spans_on`, as `enabled()` did); `Game::update` calls
  `state_hash::record`, which loads one `OnceLock<Option<…>>` and returns;
  the bench checks `bench_json.is_some()` / `trace_frames` once per frame.
  No allocation, no syscall, no change to any value the simulation or the
  renderer reads.
* **Parity, base (copy before the harness) vs new, both full release**
  (`baseline-2026-11-04/parity-base-vs-new.txt`): Original dumps of the 5
  scenes at 1280×960 and 3840×2160 (70 frames) AE 0; window path at 144 Hz
  (40 frames) AE 0; Modern/Ultra racing and explore (28 frames) AE 0; WAVs
  of the explore and race shots byte-identical; firstrace, firstdrag,
  postrace, playthrough logs identical with wall times normalised (the
  playthrough: 82 078 frames, same cash/rep/events), the parity scenario's
  clone.tsv identical. Shots: 17 identical; career, options, worldmap differ
  — and differ the same way between two runs of ONE binary (they animate on
  the wall clock), so they are not this work's; they are out of the default list.
* **State hash determinism** (`parity-new-vs-new.txt`): two runs of the
  new binary give identical hash files over firstrace (4 810 steps),
  firstdrag (17 449), postrace (29 232), playthrough (82 078) and the
  parity scenario (1 681); a racing bench with JSON + trace on gave the
  same hashes as one with everything off.
* **Numbers** (`zero-cost-base-vs-new.md`): 209 comparisons of base vs new
  (both without JSON, 5 runs each, interleaved): no regression, no
  improvement (198 same, 11 large-but-not-significant, all on noisy
  wall/CPU metrics in both directions). JSON on vs off
  (`json-cost-new-vs-newjson.md`): 192 same, 1 load-time flag and 1 RAM
  improvement — the RAM one is a read-point difference (the JSON's VmHWM was
  read before the end; fixed after the run: `vm_hwm_end_kib`).

## Determinism notes
* **RADV DCC:** on this machine (Mesa 26.2.5, reference desktop GPU) one pixel of the
  rain scene's 8x-multisampled frames (1280×960, (442, 400)) comes out up to
  3 off in ~2 % of frames, any frame, base vs base — never with
  `RADV_DEBUG=nodcc` (0 of 400), which gives the majority image every time;
  nor does it depend on the game's threads (all render jobs run inline: the
  same rate) or uploads (every `write_buffer`/`write_texture` byte-identical
  between good and bad runs). A driver issue in lossless compression, so
  `perf.sh parity` runs with `RADV_DEBUG=nodcc` (`PARITY_RADV_DEBUG=` keeps
  DCC); benches keep the driver default. Evidence:
  internal notes.
* **First-frame race (fixed in the main-thread pass):** see `docs/perf-main-thread.md` §1.

## Open
* Windowed swap-chain present cost (wgpu #9937): needs a present path on a
  private display; not done (headless-only rule; Xvfb would measure a
  software present).
* Clocks are not pinned (`power_dpm_force_performance_level` needs root):
  at 1080p Original the GPU idles most of the frame and its clock follows
  the other jobs' load, which is why GPU deltas there need ≥ 5 runs and
  4K for small effects.
* ~~iGPU set not run~~ — measured since on the reference integrated GPU (offscreen; it drives
  no display here), pinned to four cores: docs/perf-integrated-gpu.md.
* ~~The front-end shots main, career, options, worldmap and ending are not
  deterministic~~ — done in the main-thread pass: `NFSU2_SHOT_CLOCK=s` pins the clock for a
  shot; with it the five repeat byte for byte (two runs of one binary,
  internal notes). Without it (the default) shots run
  on the wall clock as before.
* Shots have no state-hash use beyond `update` (they call `update()` too,
  so `NFSU2_STATE_HASH` works there, unverified for determinism).
