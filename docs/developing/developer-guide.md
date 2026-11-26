# Developer guide: tools, diagnostic switches, the early graphics rows, performance notes

These sections were the long form of the old README. The README now covers playing; building is in
BUILDING.md, the code layout in ARCHITECTURE.md. Everything below is for developers.

## Run

```bash
export NFSU2_ROOT="/path/to/Need for Speed Underground 2"

cargo run --release --bin game                       # the game: menus, career, explore, races, Quick Race
# The headless modes (--shot, --bench, --selftest, --flow, --playthrough) are the developer harness,
# the Cargo feature `dev-harness` (off by default: the shipping game does not have them).
cargo run --release --features dev-harness --bin game -- --bench explore 600 # headless: load time, then ms per frame (update, draw, GPU) and the draw's CPU by part over 600 frames, after 20 s of free roam (traffic on the road); also `race`, `racing`, `drive` (see Performance)
NFSU2_TIMING=1 cargo run --release --features dev-harness --bin game -- --shot explore x.png  # load phase timings on stderr
cargo run --release --features dev-harness --bin game -- --bench transitions  # how long free roam -> race, race -> free roam, garage and shop exits take
cargo run --release --bin game -- --verify-cache     # the asset cache against a fresh parse of the game's files
cargo run --release --bin inspect -- L4RA            # parse and print stats
cargo run --release --bin world                      # fly around (WASD, Q/E, Shift, right mouse to look)
cargo run --release --bin world -- --cam -1405.9,250,20 --look 180,5 --shot out.png
cargo run --release --bin world -- --car 240SX       # drive: W throttle, S brake (hold to reverse), A/D steer, Space handbrake, C camera, R reset
cargo run --release --bin world -- --car 240SX --drive-seconds 2 --shot out.png
cargo run --release --bin car_check -- 240SX         # how a car assembles, what is missing
cargo run --release --example drive_probe -- 240SX  # the full model through a few manoeuvres in Bayview
cargo run --release --example reverse_probe -- square # into a wall square on, back out, drive off
cargo test --release --features dev-harness          # includes a test against the real data
```

While flying, press `P` to print the current camera as `--cam … --look …`
arguments. While driving, the window title shows speed, gear and rpm, and
`C` steps through the game's cameras: bumper, hood, near chase, far chase,
high chase.

Diagnostic switches for `world`:

* `NFS_RACE=drag|drift|streetx|url` drives with that race type's handling
  (tune, shift grading, drift assist, steering, input; see docs/formats.md,
  "Race modes"). `NFS_DIFFICULTY=0..2` sets the race difficulty (default 0).
* `NFS_ALL_SECTIONS=1` draws every section and ignores visibility.
* `NFS_ONLY_FLAGS` / `NFS_SKIP_FLAGS` filter placements by flag value
  (comma-separated hex).
* `NFS_ONLY_NAME=PAN_` draws only solids whose name contains the given text.
* `NFS_DRIVE_STEER=-1..1` with `--drive-seconds` steers for the last second.
* `NFS_CAMERA_VIEW=0..5` picks the camera to start in (0 far, 1 near — the
  default, 2 bumper, 3 driver, 4 hood, 5 high).
* `NFSU2_ALLOC=fast|lean|system` picks the allocator (default: by the
  machine, see Performance, "Allocator"); `scripts/pgo.sh` makes an
  optional profile-guided build (Performance, "PGO").

## Graphics options (clone deviations)

Options > Display > Enhancements > **Graphics** holds opt-in graphics
enhancements; each one starts at the native value (choice: the rule of the Enhancements page), and with all of them
native the frames are the same as without the page. Restore Native (on
both pages) turns everything back.

* **Render Scale** 50–200 %: the 3D scene drawn smaller or larger than
  the screen, then filtered to it (box filter when supersampling,
  Catmull-Rom when upscaling). Menus and HUD stay at screen size. Capped
  by the GPU's texture limit and a pixel budget (at 4K up to 125 %).
* **Anti-Aliasing**: FXAA, SMAA 1x, MSAA 2x/4x/8x, or **SSAA 2x/4x/8x/9x/16x**.
  SSAA multiplies scene pixels, then downsamples before drawing the HUD.
  16x at 4K means 15360 × 8640 internally when the GPU and budget allow it.
  **AA: MSAA Override** independently chooses Preset, Off, or supported
  2x/4x/8x/16x MSAA. **AA: Post Filter** independently chooses Preset, Off,
  FXAA, SMAA, or SMAA + FXAA. These combine, followed by optional sharpening.
  Preset follows the main Anti-Aliasing row; Native on that main row uses
  the original AA setting unless an explicit MSAA override replaces it.
  The original Full Screen Anti-Aliasing row then reads
  **Disabled - Enhancements** and preserves its saved value.
  Unsupported choices are skipped; saved/capped modes show "limited" and
  Render Scale shows the effective supersampling percentage. GPU texture
  limits and memory budgets still apply, particularly to extreme combinations.
  See [the AA guide](../playing/settings/ssaa.md). Defaults remain native; no FPS cap is added.
* **Sharpening** 25–100 %: AMD CAS, applied last.
* **Road Reflection Res**: the wet-road reflection at half or full screen
  size instead of 320 × 240.
* **Car Reflection Res**: the cars' environment cube at 256 or 512
  instead of 128.
* **Detail Distance** 150–300 %: props, trees, signs, lamps and their
  glows drawn that many times further (the native 17-pixel size cut
  divided). Whole sections still appear where the game's visibility
  lists put them: that pop-in is the original's own (checked against it
  frame by frame, docs/formats.md "Pop-in").
* **Car Detail Distance** 150–300 %: the cars' detail levels (and their
  far cut) switch that many times further away.

The F3 overlay's Deviations rows list the ones that are on.
`game --bench … ` takes them from `NFSU2_BENCH_ENH="enh.render_scale 150;enh.post_aa 1"`
(options.txt lines; `NFSU2_BENCH_ENH_SWITCH="frame:…"` changes them
mid-run), `game --shot …` from `NFSU2_SHOT_ENH` (after `NFSU2_SHOT_PRESET=Original|Balanced|High|Ultra|Extreme`
and `NFSU2_SHOT_LEVEL=Low|Medium|High`, a Graphics page preset or Enhanced level, as `NFSU2_BENCH_PRESET` /
`NFSU2_BENCH_LEVEL`; the README's screenshots are `--shot race` and `--shot explore` at 1920×1080, Original vs Ultra).
`NFSU2_BENCH_CAMTRACK=file` (comparison aid) puts the camera and the car
at one logged pose per bench frame (eye, forward, up, the game's view
angle, car position and axes), e.g. the original's camera logged under
Wine, to render the same path. What each one costs, and
which candidates were left out and why: docs/formats.md, "Graphics
enhancements".

## Performance

`game --bench <scenario> [frames]` runs the window's own frame headless on
a 1280×960 target, at a fixed 60 Hz step and a fixed clock, so a build's
frames come out the same run to run. Scenarios: `explore` (free roam
after 20 s, 16 traffic cars), `race` (a stage 1 circuit from its start
scene), `racing` (a circuit with three rivals and traffic from the green
light, the player's car on the AI racer controller), `drive` (a sprint
with traffic driven the same way: 3600 frames cover 1.55 km and 15
sections). It prints, per frame, the update, the draw (wall and the main
thread's CPU), the wait for the GPU and the whole frame (mean, median,
p95, p99, max), the CPU by part with each part's worst frame, the GPU time
by part from timestamp queries (where the device has
`TIMESTAMP_QUERY` and `TIMESTAMP_QUERY_INSIDE_ENCODERS`), the whole
process's CPU and the peak memory. `NFSU2_BENCH_DUMP=dir` writes every
`NFSU2_BENCH_DUMP_EVERY`-th frame as PNG, to compare two builds;
`NFSU2_BENCH_GPU=0` leaves the timestamps out; `NFSU2_BENCH_AUDIO=1` times
the game's sounds on an offline mixer; `NFSU2_TIMING=1` prints the
load's phases.

### Allocator

The game's global allocator is `nfsu2::allocator::GameAlloc` (default
feature `mimalloc`; `--no-default-features` builds with the system
allocator only). A global allocator cannot be swapped once it has handed
out memory, so the mode is picked at the process's first allocation
(before `main`, without allocating: `getenv`, then `/proc/meminfo` on
Linux, `sysctl hw.memsize` on macOS, `GlobalMemoryStatusEx` on Windows)
and kept; every block goes back to the allocator that gave it. The log
(`RUST_LOG=info`) and every `--bench` print the choice, e.g. `allocator:
fast (auto: 61.9 GB RAM, 41.7 GB available)`.

* `fast` (default): mimalloc v3 with its defaults.
* `system`, which a small machine gets: less than 10 GiB of RAM (every
  "8 GB" laptop reports 7.2–7.8 GiB, an integrated GPU sharing it) or
  less than 4 GiB available at start (fast's peak over the transitions
  is ~2.4 GB). On Linux/glibc it is glibc's malloc with huge pages (the
  re-exec below), which measured both the leanest and as fast to load.
* `lean`, only on request: mimalloc with `purge_delay` 0, no THP, no
  large OS pages, arenas committed on demand (a `MIMALLOC_*` variable of
  the same option wins). It saves little and loads slowest, so the
  auto choice does not use it; harder settings (`PAGE_FULL_RETAIN=0`,
  64 MB arenas, reclaim on free) were worse (1190–1301 MB, loads to 1.5 s).
* `NFSU2_ALLOC=fast|lean|system` forces one; `auto` or anything else
  lets the machine decide. `MIMALLOC_*` variables work as usual.

The glibc huge-page re-exec (below, "Malloc on huge pages") stays for
every mode on Linux/glibc: with mimalloc glibc's malloc still serves the
C libraries (Vulkan driver, audio). In fast mode, with vs without it:
racing loads 513–538 / 544–832 ms, process CPU 3.02–3.12 / 3.20–3.30 ms
a frame, peak 1342–1363 / 1335–1406 MB (noisy machine, three runs each).
`M_ARENA_MAX` 4 stays as well. mimalloc builds with `cc` on Linux,
Windows (MSVC and GNU) and macOS; the probe has a branch for each and
reports nothing elsewhere (then fast). Measured on Linux only; on
Windows and macOS the system allocators' speed and memory with this
game are **not measured** (small machines get them by the same rule).

Measured 2026-10-31 (a high-end desktop GPU, the machine shared with other build jobs, load
average 33–49; one binary, modes alternated, three runs; peaks from
VmHWM, the transitions' from rusage):

| | fast | lean | system |
|---|---|---|---|
| `racing` load, ms | 407 / 259 / 237 | 478 / 309 / 258 | 373 / 233 / 224 |
| `racing` peak, MB | 1717 / 1385 / 1381 | 1245 / 1227 / 1226 | 943 / 929 / 924 |
| `explore` load, ms | 260 / 186 / 201 | 367 / 222 / 226 | 307 / 167 / 198 |
| `explore` peak, MB | 628 / 624 / 620 | 487 / 491 / 501 | 447 / 449 / 449 |
| `transitions` free roam → race / race → free roam, ms | 118/131, 97/65, 91/72 | 203/123, 192/110, 141/94 | 138/105, 93/87, 97/83 |
| `transitions` peak, MB | 2451 / 2346 | 2166 / 2183 | 1133 / 1630 |
| draw CPU, ms a frame (`racing` / `explore`) | 1.04–1.22 / 0.66–0.73 | 1.11–1.23 / 0.66–0.72 | 1.12–1.21 / 0.70–0.74 |

Frame times are the same in all three. On this Linux machine fast buys
nothing over glibc with huge pages, at +0.45 GB a race and up to +1.3 GB
over the transitions; its case is the other platforms, where no huge-page
trick exists (an earlier round measured glibc without it: race loads twice as
long). **Inferred**, not measured: that Windows' and macOS's allocators
stall like glibc's without huge pages.

### PGO

`scripts/pgo.sh [frames]` (optional; `NFSU2_ROOT` set) makes a
profile-guided build: an instrumented `game` (`-Cprofile-generate`) runs
the benches `explore`, `race`, `racing`, `drive` (600 frames each) and
`transitions`, `llvm-profdata` merges the profiles, and `game` is built
again with `-Cprofile-use`. Result: `$CARGO_TARGET_DIR/pgo/game`
(default `target/pgo/game`). It builds in `pgo-gen/` and `pgo/` beside
the normal output and sets `RUSTFLAGS` only for itself, so plain `cargo
build` is unchanged and portable. Needs `rustup component add llvm-tools`
(rustc's own `llvm-profdata`; a system one of another LLVM version cannot
read rustc's profiles), the game data and a GPU for the training. Plain
rustc flags, no `cargo pgo`: Linux and macOS as is, Windows from Git Bash
or MSYS2 (**not tried** on those two). Takes ~8 min here.

Measured 2026-10-31 (loaded machine, alternated, three runs; trained and
measured on the same benches):

| ms a frame | normal | PGO |
|---|---|---|
| `explore` draw, main thread CPU | 0.81–0.88 | 0.71–0.76 |
| `race` draw CPU | 1.30–1.39 | 1.17–1.19 |
| `racing` draw CPU | 1.30–1.48 | 1.23–1.30 |
| `drive` draw CPU | 1.30–1.37 | 1.09–1.15 |
| process CPU (all threads), `explore` / `race` / `racing` / `drive` | 1.85–1.97 / 2.92–3.10 / 3.05–3.35 / 2.92–3.10 | 1.63–1.75 / 2.57–2.63 / 2.78–2.95 / 2.55–2.62 |
| loads, frame wall times | within noise | within noise |

So the CPU work drops 7–15 % (main thread) and 8–15 % (whole process);
frames here are GPU- and machine-bound, so the wall times did not move,
and loads (I/O, threads, allocation) are within noise. The frames are
the same bytes (`drive` dumps every 100 frames: identical). Worth it for
CPU-bound laptops as a release step; not needed for development builds.

### Loading

Loads are CPU-bound (the page cache serves the data at GB/s), so the
work is in parsing less and in parallel (none of it changes what is
drawn or simulated):

* **Parallel parse.** `World::load` parses the 393 stream sections, the
  collision packs and the shared texture files side by side and merges
  them in file order (first solid/texture of a hash wins, last
  flip-book wins, as before); the big files are mapped, not read.
* **Shared data once.** Texture pixels are shared (`cache::Bytes`), so the
  shared car and rim packs and GLOBALB's are held once for every car
  (they were copied into each car, ~37 MB a traffic car). The shared files,
  texture packs and part-file indexes are parsed once a run even when cars
  load in parallel (single-flight caches in `car.rs`).
* **Asset cache** (`crates/ob-core/src/cache/`, `src/render/native/world/prepared.rs`): the parsed
  world, each car's models and texture packs and the shared part files'
  indexes, plus the world renderer's upload-ready data (the vertex and index
  buffers, the texture arrays' levels with rows padded to 256 bytes), in
  `$XDG_CACHE_HOME/overbright` (`NFSU2_CACHE_DIR` to move it,
  `NFSU2_NO_CACHE=1` to turn it off, `NFSU2_CACHE_MAX_MB`, default 2048, to
  cap it). Built from the first load and written on a background thread
  (on Linux an anonymous `O_TMPFILE` linked in only once complete and
  synced, elsewhere a locked `*.tmp` renamed into place, so a killed process
  leaves nothing half-written; stale unlocked `*.tmp` files older than 10
  minutes are swept at the first open and on every prune); later loads map
  it. A file is used only if
  its format, the build id (a hash of the library's sources) and every source
  file's size and time match (a changed time with the same XXH3-128 content
  is rewritten and used) and every 4 MB of it matches its XXH3-64 (read and
  hashed by up to 8 threads of positioned reads at once: a cold Bayview open
  ~200 → 60–85 ms on this NVMe; the mapping itself is not populated);
  anything else is parsed from the game's files as without a cache. The
  format is the clone's own flat little-endian one (not rkyv): plain-data
  arrays are one copy, texture pixels and GPU data are not copied (they
  point into the mapping, whose pages are released once uploaded). Each
  copy is stored once: the world's vertices and indices are the renderer's
  buffers (`w.vertices`, `w.indices`), and a texture whose blocks the
  renderer's arrays hold verbatim keeps no section of its own (`w.tdmap`
  says where its levels lie; its bytes are gathered only if something on
  the CPU reads them). `--verify-cache` compares every cached track (every
  field of the world, and the renderer data) and per-file cache with a
  fresh parse. Bayview (L4RA) is 139 MB (208 MB before the copies went), a
  car's files 1–10 MB.
* **Track loads keep the region.** A car change, a garage or shop exit and
  free roam ↔ city races reuse the loaded world and its renderer, reset to
  the state a fresh load gives (`World::reset_to_loaded`,
  `WorldRenderer::reset_placements`; tested against a fresh load); only the
  car is loaded again. PC_Loading goes up only when a load runs over 150 ms
  (the original shows it for every track load; docs/formats.md, "Track loads
  and their loading screen").
* **Uploads behind the loading screen.** The uploads are submitted and
  waited for before the load ends, so the first frame is an ordinary frame.
* **Loads ahead** (`Preload` in `game.rs`; `NFSU2_NO_PRELOAD=1` turns it
  off). In the menus with no drive (and in the menus after a race in
  another region), free roam's world, its renderer (uploads done) and the
  player's car as free roam loads it are loaded on threads of their own;
  a car changed in the menus is loaded again. While UI_EngageRaceDialog is
  up at a marker whose race is in another region, that region and the
  car as the race loads it. `drive_in` takes (or waits for) them instead
  of loading; they are the same calls with the same arguments (the car
  keyed on its name, parts, packages and tuning), so the drive is the
  same: `--bench explore|racing` dumps after 200 main-menu frames match
  pixel for pixel with and without the preload
  (`NFSU2_BENCH_MENU_FIRST=1`). The preload's parallel parts run on a pool
  of their own, and the world texture arrays go up through staging
  buffers of their own (`queue.write_texture` copied ~100 MB holding the
  queue's lock, stalling the menu's frames meanwhile to 10–19 ms; now
  4–8 ms on the loaded machine, 3–4 ms without a preload).
* **Rivals and encounter racers ahead** (`RivalsAhead`, `EncountersAhead`
  in `game.rs`; off with the rest by `NFSU2_NO_PRELOAD=1`). While a
  career race's UI_EngageRaceDialog is up (any region), its rivals' cars
  load on threads: the line-up and their looks are drawn on a copy of the
  random numbers as the race's start will draw them. In the menus that go
  on to career free roam (the front end with no drive, the post-race
  screens, a race's pause, a shop's screens, the garage, and the screens
  after them) the encounter racers' cars load the same way, predicted
  again whenever the generator or the player's car moves
  (docs/developing/first-use-builds.md "Encounter racers ahead"). A load is taken only for the same car, parts and packages
  the race or free roam then asks for; anything else loads as before, so a
  wrong guess costs time, never a difference (`NFSU2_BENCH_ENGAGE=n`
  runs `racing`/`drive` through n frames of the dialog: dumps identical
  with and without the preload). The encounter racers' cars are now
  joined at the end of free roam's load instead of in the frame that
  places the first of them, which stalled free roam's first second by
  25–60 ms.
* **Free roam's routes once a run.** RoutesFreeRoam pruned for the career
  stage (~17 ms, made at every free roam start for the traffic and again
  for the encounter racers) and the encounter racers' racing speeds
  (~40 ms) are memoized by their inputs (stage; class, SUV, speed mode);
  the main menu's preload makes them for the profile's stage.

Measured 2026-10-29 on an integrated-GPU laptop (quiet: load average 3–5),
three runs each, against the build before this work:

| | before | no cache | cold cache (first run) | warm cache |
|---|---|---|---|---|
| `--bench explore`: load / first frame (ms) | 475–496 / 72–116 | 401–446 / 23–30 | 435–469 / 22–32 | 293–331 / 20–30 |
| `--bench race`: load / first frame (ms) | 504–666 / 138–150 | 460–495 / 36–39 | 502–534 / 34–47 | 351–371 / 34–43 |
| peak memory (VmHWM) | 833–849 MB | 487–496 MB | 632–660 MB | 407–410 MB |
| `World::load` (wall / thread CPU) | 206 / 144 ms | | | 59 / 15 ms |

`--bench transitions` (warm cache; the call, then its first frame):

| | before | after | PC_Loading |
|---|---|---|---|
| free roam → city race | 110–130 ms | 67–85 ms | not shown |
| race → free roam | 196–230 ms | 122–150 ms | not shown |
| out of the garage | 116–125 ms | 90–111 ms | not shown |
| out of a shop | 118–127 ms | 90–112 ms | not shown |
| out of a shop with a part fitted (car reloaded) | 508–569 ms | 140–163 ms | not shown |

`--bench transitions` with the loads ahead and the memoized routes
(2026-10-30, a high-end desktop GPU, the machine shared with other build jobs, load average
22–33; three runs each, alternated with the build before):

| | before | after |
|---|---|---|
| free roam, first load (no menus before it) | 271–317 ms | 249–256 ms |
| main menu → free roam (before: no preload, as `NFSU2_NO_PRELOAD=1`) | 125–153 ms | 31–33 ms |
| first load after the main menu (`NFSU2_BENCH_MENU_FIRST=1`) | 247–348 ms | 61 ms + 11 ms first frame |
| race → free roam (city) | 124–149 ms | 18–36 ms |
| out of the garage / a shop | 81–94 ms | 12–20 ms |
| out of a shop, a part fitted | 115–126 ms | 48–52 ms |
| free roam → another region's race (before: at once) | 65–72 ms | 38–44 ms |
| that race → free roam (before: at once) | 134–144 ms | 32–39 ms |

The preload takes ~150–190 ms of menu time (the Engage dialog's ~25–70 ms).
Frame times in `--bench explore|race|racing|drive` are unchanged within the
machine's noise (nothing per frame changed); all shots are identical to the
build before (`compare -metric AE`: 0).

Third round (2026-10-31; a high-end desktop GPU, measured when the other
jobs' load had dropped to a load average of 4–5; three runs each, base
and new binaries alternated, each with its own `NFSU2_CACHE_DIR` (two
builds sharing one cache rewrite each other's files, the build id
differs), the first run of each discarded):

* **AI car dressing.** `ai_looks::Ride::parts` walked a slot's parts with
  `PartDb::find_after`, which searches from the start each time
  (quadratic): ~100 ms of CPU for the 8 encounter racers at every free
  roam start, more for a race's rivals. `PartDb::parts_for` walks them
  once in the same order (tested against the `find_after` walk for every
  slot of nine cars).
* **Encounter racers off the load.** Their 8 cars and their routes' racing
  speeds (the `lane_speed` bytes) load on threads of their own; the
  manager joins them (`Encounters::ready`) just before it places its
  first racer. Until then it reads only the network's points, which the
  speeds leave as they are, and every reader of the cars sees none placed.
  The draws come first as before, and a loaded car's type is `car_types`'
  record (the same parse with no upgrades), so the speeds' class and SUV
  test read that.
* **Traffic beside the encounters.** Free roam's traffic loads on a thread
  while the encounter racers are set up (its seed drawn first, as before;
  its cars are `Arc`s now).
* **Renderer build.** The solids' and the instance groups' bounding
  spheres are made on the pool, and the world texture arrays' staging
  copies run per array and in 1 MB pieces in parallel; the draw list
  itself (its texture slots are numbered in first-use order) stays serial.
* **Pipeline cache on disk.** Vulkan's pipeline cache is handed to
  every world, car, post and UI pipeline and written to the asset cache's
  directory (`wgpu_pipeline_cache_vulkan_<vendor>_<device>`, temporary
  file and rename) after a load that grew it; wgpu and the driver check
  the data is theirs (driver version included) and start empty
  otherwise. `NFSU2_NO_PIPELINE_CACHE=1` turns it off. Mesa keeps its own
  shader cache, so on the reference desktop it matters only when that is cold:
  with `MESA_SHADER_CACHE_DISABLE=true` the first load and its first
  frame take 220–235 ms without it and 150–155 ms with it.
* **World animations.** The views tested a moved group (the 22 animated
  pieces' groups, knocked props) against no planes, so every animated
  piece was recorded and drawn in all eight views (main, road reflection,
  six cube faces) wherever it was. Each instance's spheres are kept now
  and updated as it moves (the same values the build puts round the
  groups), and a moved group is tested instance by instance. The moved
  placements' matrices go up in runs of adjacent slots under one lock,
  and the animation channels are sampled without allocating.
* **Shadow textures at the load.** A traffic car's or rival's shadow and
  underglow textures were uploaded in the frame it first came on the
  road; they go up when the traffic and the rivals load.

| `--bench transitions` (ms: the call / with its first frame) | before | after |
|---|---|---|
| free roam, first load (no menus before it) | 258–263 / 269–275 | 143–153 / 154–164 |
| free roam → city race | 161–180 / 166–186 | 102–106 / 106–110 |
| race → free roam (city) | 82–84 / 84–86 | 9–17 / 10–18 |
| out of the garage | 72–80 / 73–81 | 5–6 / 5–7 |
| out of a shop | 70–73 / 71–73 | 4–5 / 4–5 |
| out of a shop, a part fitted | 95 / 95–96 | 31–33 / 31–33 |
| main menu → free roam, at once | 169–180 / 171–182 | 90–99 / 92–101 |
| main menu → free roam, preloaded | 93–95 / 94–97 | 20 / 22–23 |
| free roam → another region's race, at once | 327–343 / 339–354 | 207–253 / 214–260 |
| that race → free roam, at once | 204–207 / 206–210 | 99–112 / 101–114 |
| free roam → another region's race, preloaded | 168–176 / 179–187 | 70–83 / 77–91 |
| that race → free roam, preloaded | 123–129 / 125–131 | 24–25 / 26–27 |

Per frame (1200 frames of `explore`, 3600 of `drive` and `racing`, two or
three runs each): free roam's draw CPU 0.58–0.61 → 0.56–0.57 ms, its
frame 0.76–0.77 → 0.73–0.74 ms, the whole process 1.28–1.32 → 1.28–1.30
ms a frame; the world animations' step and their placements' upload are
0.01 ms each on the main thread. Driving and racing over the sections at
speed show no hitch on a quiet machine before or after (frame max
2.7–3.9 ms, p99 1.6–1.8 ms, none over 16.7 ms; worst draw CPU 1.8–2.1
ms); the spikes seen earlier (frame max 30–50 ms) were the other jobs'
load (load average 50–80), not the game's work. All shots and the
`explore`, `drive` and `racing` dumps (every 100th frame) are identical to
the build before (`compare -metric AE -fuzz 3%`: 0).

What is left: the world renderer's draw list (~70 ms CPU, built on the main
thread each new world), the GPU copies of ~100 MB (~70 ms on the iGPU), the
encounter racers' route set-up (~60 ms, every free roam start), and
streaming sections while driving (the whole track is still loaded at once).

Fourth round (2026-10-31; a high-end desktop GPU, the machine shared with other build jobs,
load average 40–60, so load times are within noise; base and new binaries
alternated, three runs each, each with its own `NFSU2_CACHE_DIR`).
`NFSU2_BENCH_DETAIL=1..5` runs a bench at that World Detail preset.

* **Race start crowd.** The start scene's characters (CPU-skinned every
  frame, the race's whole start scene) looked each corner's texture up in
  a hash map (~50k lookups a frame); now once per run of corners, and a
  vertex shared by corners of a range is posed once. Same values, same
  order. Main thread's draw CPU in `race` 1.96 → 1.23 ms a frame (the
  crowd 1.5 → 0.74 ms), the process 3.47 → 2.70 ms; `racing` p95
  2.3–2.5 → 1.6–1.8 ms, `drive` p95 1.9–2.2 → 1.4 ms. (Skinning the
  characters on the pool instead halved the main thread's CPU but made the
  frame's wall time worse on the loaded machine, 5–8 → 12–14 ms, so it
  stays on the main thread.)
* **Environment maps off** (custom 0 = 0, World Detail 1–2): the cube was
  cleared to black with six passes every frame; it is cleared once and
  kept until a face is drawn again. GPU at World Detail 1: 0.28 → 0.22 ms
  a frame (`drive`). Low detail otherwise does what it says: no cube, no
  shadows, no particles, no post passes but the copy to the back buffer
  (GPU 0.22 ms against 0.86 at detail 5).
* **Load/store ops.** The post chain's replacing full-screen passes
  start with `LoadOp::DontCare` (they write every pixel; nothing is read
  in), and the cube's depth is transient and discarded. `LoadOp::Clear`
  there instead cost RDNA ~0.04 ms a frame (fast-clear work), so it is not
  used. Gains are for tile-based GPUs (Apple); none here.
* **Draw list.** The world renderer's groups (spheres, instance
  matrices, the materials' draws with each texture as a hash) are built on
  the pool; the texture slots are then numbered in one walk in first-use
  order, as before. "groups and draws" thread CPU 24–25 → 19–22 ms; wall
  times within noise here.
* **Post views (R8).** The post targets' views are made with the targets,
  not every frame.
* **Backend.** `WGPU_BACKEND=vulkan|dx12|metal` picks the backend
  (default unchanged: the first adapter of Vulkan, Metal, DX12).

Measured, not changed: rain at the airport (clock 162, rain 0.70) costs
GPU +0.07 ms a frame in `drive` (tint 0.04, lens drops 0.06 where shown,
particles +0.01) and no measurable CPU; world animations 0.02–0.03 ms main
thread; the AI racers' dressing ~9 ms ("Encounters::new: picks") once per
free roam start.

`--features mimalloc` (then opt-in): race loads (`racing`) 1015–1052
→ 508–550 ms on the loaded machine (two rival car loads stalled ~450 ms
each with glibc's allocator, ~20–180 ms with mimalloc; `GLIBC_TUNABLES`
mmap/trim thresholds and arena limits did not help), but peak memory 908 →
1340 MB (`MIMALLOC_PURGE_DELAY=0`: 1250 MB and slower); frame times the
same. mimalloc is now the default, see "Allocator" below; PGO: see
"PGO" below.

All shots, and the `explore`, `race`, `racing` and `drive` dumps (also at
World Detail 1 and 3), are identical to the build before (`compare -metric
AE`: 0, also without fuzz).

Fifth round (2026-10-31; a high-end desktop GPU, the machine shared with other build jobs,
load average 38–49; base and new binaries alternated, three runs each,
each binary with its own `NFSU2_CACHE_DIR`; medians, ranges in brackets).
Against the round-4 build, the base build's frames cost the main thread +0.05
(explore) to +0.14 ms (race) of draw CPU: the flare pools and the road
reflection's flares (0.04–0.06 ms, now their own part, "world: flare
pools, reflection flares") and the start scene's crowd; GPU, loads and
memory the same. Rain at the airport (clock 162) costs GPU +0.08 ms and
draw CPU +0.02–0.11 ms over `drive`; World Detail 1 0.14 ms GPU.

| ms (MB) | base explore | new | base race | new | base racing | new | base drive | new | base rain | new |
|---|---|---|---|---|---|---|---|---|---|---|
| load | 381 [321–408] | 268 [250–283] | 860 [833–951] | 453 [372–467] | 732 [730–954] | 511 [388–533] | 976 [794–1016] | 549 [399–600] | 944 [855–1415] | 543 [455–591] |
| draw, main thread CPU | 0.86 | 0.84 | 1.38 | 1.14 | 1.40 | 1.26 | 1.21 | 1.12 | 1.32 | 1.17 |
| process CPU (all threads) | 1.92 | 1.88 | 2.85 | 2.62 | 3.15 | 2.92 | 2.68 | 2.53 | 2.93 | 2.65 |
| frame mean / p95 / p99 / max | 2.3 / 4.6 / 8.9 / 18 | 2.3 / 4.5 / 8.8 / 22 | 3.2 / 5.7 / 9.2 / 22 | 3.5 / 6.6 / 12.5 / 21 | 4.0 / 9.0 / 16 / 21 | 4.2 / 7.5 / 14 / 25 | 3.5 / 7.4 / 14 / 17 | 3.5 / 7.4 / 12 / 20 | 3.6 / 7.4 / 12 / 21 | 4.0 / 8.7 / 14 / 28 |
| GPU | 0.72 | 0.72 | 0.93 | 0.94 | 0.95 | 0.95 | 0.85 | 0.86 | 0.94 | 0.94 |
| first frame | 34 | 35 | 45 | 48 | 31 | 48 | 42 | 52 | 39 | 49 |
| peak memory | 432 | 426 | 912 | 914 | 915 | 906 | 912 | 905 | 916 | 905 |

Frame wall times, their p99/max and the first frames are within the
machine's noise here (the same binary's p99 moved 6–16 ms run to run);
the CPU columns are not. `--bench transitions`: free roam → race 544–743
→ 171–260 ms, "out of a shop, a part fitted" 225–253 → 152–183 ms; the
rest within noise; peak 1094–1320 → 1163–1272 MB.

* **Malloc on huge pages.** A race load's rival cars stalled in glibc's
  malloc (one car's "shared parts and the rest" 580 ms wall for 50 ms of
  CPU; 160 ms / 8 ms with mimalloc): its heaps grow page by page and the
  loaders' threads (four arenas, `M_ARENA_MAX`) wait on it.
  `glibc.malloc.hugetlb=1` grows them in 2 MB huge-page steps; a tunable
  is read only at a process's start, so `main` starts the game again once
  with it added to `GLIBC_TUNABLES` (Linux/glibc only; not when the
  variable already names it, nor with `NFSU2_NO_MALLOC_THP=1`; glibc
  ignores it where THP is `never`). Race loads 730–980 → 450–550 ms,
  free roam 381 → 268 ms, at the same peak memory (±10 MB).
* **mimalloc** (then `--features mimalloc`), measured with the above:
  loads about the same as glibc with huge pages (race 467, racing 445,
  drive 507, explore 287 ms), frames the same, but peak memory +170 MB in
  free roam (602), +430 MB in races (1336–1350) and +0.7–1.2 GB over the
  transitions (1955–2549 MB). That round recommended leaving it opt-in;
  it was made the default with a fallback for small machines
  (2026-10-31, "Allocator" below).
* **Crowd skinning.** The start scene's characters are skinned straight
  into the frame's vertex list (no corner list per model, the scratch
  kept between frames), each vertex shaded once instead of at each of
  its ~4 corners, a range whose texture is not loaded is not posed, and
  the bone blend runs in SIMD lanes (`Vec3A`: the same multiplies and
  adds lane by lane). The same values: a bit checksum of every IntroNis
  scene's corners over 300 frames is unchanged (`cargo test --release
  character::cost -- --ignored --nocapture`). "characters" 0.74 → 0.50 ms
  a frame in `race`, 0.40 → 0.26 in `racing`, 0.27 → 0.18 in `drive`.
* **Audio.** `NFSU2_BENCH_AUDIO=1` runs a bench with the game's sounds on
  an offline mixer and times a frame's worth (800 frames at 48 kHz) after
  each step: 0.09–0.12 ms median, 0.12–0.18 mean (≈1% of a core) with up
  to 29 voices and the reverb in `racing`, 36 voices in `explore`; its
  p99 (0.9–2.6 ms) and max are the machine's preemption (wall time; the
  CPU column is printed too). The mixer test (`mixer::cost`: 24 voices
  with moving gains and Simple_Tunnel) takes 0.7% of a core. Changed: the
  engine's granular blocks are filled in place (no allocation in the
  callback), and a song's decoder reserves the whole song (the header's
  sample count) before it starts, so the buffer is never moved while the
  callback waits on its lock. The sound is the same: the `explore` shot's
  WAV capture is byte-identical. (The `race` shot's capture differs run
  to run in the base build too, 80% of samples after 0.4 s: the speech
  and music decoders run on threads and the offline mix outruns them.)

All shots (main career explore intro qrcars race qrstart drift drag ending
engage) and the `explore`, `race`, `racing`, `drive` and rain dumps are
identical to the base build's (`compare -metric AE`, with and without `-fuzz
3%`: 0). One `main` shot differed once in a 252×124 area at the menu's
top; three more runs of each build all match the base build's first (run-to-run
timing, not the change).

Sixth round (2026-10-31; a high-end desktop GPU, the machine shared with other build jobs,
load average 15–29, so absolute times are inflated and noisy; the base and
new binaries alternated, three runs each with their own
`NFSU2_CACHE_DIR`; medians).

* **Rivals loaded ahead** at the Engage dialog and **encounter racers
  loaded ahead** in the menus before free roam (see "Loading" above).
* **No stall after free roam starts.** `Encounters::ready` joined the
  encounter racers' car loads in the frame that places the first of them,
  usually free roam's first few frames: 27–61 ms frames on the reference desktop.
  They are joined at the end of `explore` now, so free roam's first
  second has no frame over 11 ms; the time moved into the load, and the
  loads ahead above take it out again.
* `--bench transitions` prints each transition's worst frame in the
  second after it, and has two new rows: a city race after its Engage
  dialog has been up, and back to free roam after the post-race menu has
  been up. `NFSU2_BENCH_ENGAGE=n` runs `racing`/`drive` from free roam
  through n frames of the event's Engage dialog.

| `--bench transitions`, ms (the call / with its first frame) | base | new |
|---|---|---|
| free roam → city race, from its Engage dialog | 99 / 106 (no load ahead) | 17 / 22 |
| city race → free roam, from the post-race menu | 14 / 19, then a 51–61 ms frame | 8 / 10, worst frame after 2.1 |
| city race → free roam, at once (no menu) | 14 / 19, then a 51–61 ms frame | 82 / 84, worst frame after 3.4 |
| main menu → free roam, preloaded | 21 / 26 | 23 / 27, worst frame after 2.7 |
| free roam, first load (no menus) | 157 / 179, then a 27 ms frame | 200 / 215, worst frame after 3.7 |
| free roam → another region's race, preloaded | 43 / 52 | 31 / 39 |
| that race → free roam, preloaded | 29 / 33 | 27 / 30 |
| out of the garage / a shop | 6 / 8, 16 / 19 | 8 / 9, 17 / 18 |
| into the garage (its scene's first frame) | 0 / 35 | 0 / 38 |

Frame times are unchanged within the noise (nothing per frame changed;
medians of three 600-frame runs, ms): explore frame 1.09 → 1.21 (draw
CPU 0.71 → 0.72, GPU 0.60 → 0.54), race 1.58 → 1.39, racing 1.47 → 1.57,
drive 1.28 → 1.46, replay 1.46 → 1.41, rear-view mirror (`NFSU2_VIEW=1`
drive) 1.51 → 1.40, 4× MSAA drive 1.24 → 1.29, hydraulics explore 1.12
→ 1.13, 3840×2160 drive 2.43 → 2.43 (GPU 2.01 both); the scene's 30 fps
cap holds (33.33 ms). Peak memory the same (448 / 936 MB). Startup to
the main menu: 0.5–0.7 s for the process either way (`Game::new`
150–235 ms). All shots and the `explore`, `racing`, `drive` and
menu-first `explore` dumps are identical to the base build's (`compare -metric
AE`: 0; 14 front-end shots with timed animations differ run to run in
the base build itself by the same amounts); the CAMERA check (`NFSU2_BENCH_CAR=
COROLLA --bench drive`) drives the same 136 m.

Open: the garage's first frame (~35 ms: the save-point snapshot 0x52A470
~10 ms of `to_native` twice, the FE car's load and renderer ~9 ms, the
platform 1–6 ms); the HUD layout (0.13–0.15 ms a frame) and the start
scene's crowd skinning (0.18 ms in `drive`) are the largest CPU parts left;
the post chain (~0.27 ms GPU at 1280×960) the largest GPU part.

Third pass (2026-10-29), on an integrated-GPU laptop's iGPU (Vulkan,
Mesa 26), the machine shared with other build jobs: medians of three runs,
old and new binaries alternated, ranges in brackets.

| ms per frame | explore before | explore after | race before | race after | racing before | racing after | drive before | drive after |
|---|---|---|---|---|---|---|---|---|
| update mean / p95 / max | 0.72 / 0.96 / 12.2 | 0.82 / 1.04 / 3.7 | 0.15 / 0.34 / 1.1 | 0.15 / 0.39 / 1.5 | 0.51 / 0.72 / 4.2 | 0.55 / 0.76 / 4.0 | 0.53 / 0.79 / 2.6 | 0.61 / 0.91 / 4.1 |
| draw, main thread CPU | 7.29 [7.04–7.74] | 3.04 [2.93–3.12] | 6.87 [6.73–7.98] | 1.89 [1.82–1.90] | 8.53 [8.40–9.24] | 2.60 [2.29–2.64] | 7.50 [7.44–7.56] | 2.50 [2.50–2.56] |
| draw wall mean / p95 / max | 6.9 / 9.4 / 16.2 | 3.5 / 4.7 / 7.9 | 7.1 / 11.1 / 16.2 | 3.7 / 6.4 / 10.9 | 8.6 / 11.4 / 22.4 | 5.5 / 8.8 / 14.3 | 7.2 / 9.7 / 19.4 | 3.7 / 4.6 / 8.1 |
| GPU (timestamps) | 5.58 [5.53–5.86] | 5.08 [5.04–5.21] | 5.49 [5.44–11.4] | 4.44 [4.42–4.87] | 8.20 [5.74–11.1] | 4.83 [4.81–10.9] | 6.04 [6.02–6.58] | 5.17 [5.05–5.18] |
| frame mean / p95 / max | 15.0 / 17.7 / 33.4 | 11.5 / 13.4 / 18.3 | 14.6 / 19.2 / 25.0 | 10.3 / 14.6 / 17.5 | 19.3 / 27.4 / 37.0 | 13.7 / 16.9 / 22.0 | 15.1 / 17.7 / 28.0 | 11.5 / 13.0 / 16.5 |
| load / first frame | 1205 / 184 | 1098 / 100 | 1073 / 198 | 888 / 107 | 1163 / 202 | 1042 / 114 | 1030 / 195 | 872 / 109 |

The whole process's CPU per frame (all threads) stays about the same
(explore 8.1–9.8 before, 8.9–9.2 after; racing 9.1–10.5, 9.5–11.9): the
encoding moved off the main thread rather than away. Peak memory is
830–855 MB either way (+8 MB for the culling data). Startup to the main
menu (~0.6–0.7 s) and the front-end screens (~0.6–0.7 s a shot) are much
as they were; they are dominated by the car's own data.

What changed (none of it changes the image; see docs/formats.md,
"Renderer performance", for the rules relied on):

* **Exact culling on the CPU.** Each view records only the draws that can
  put a pixel on screen: sections and instance groups wholly outside its
  planes, and groups whose every instance the vertex shader's size cull
  drops (tested with a margin), are left out. The road reflection is
  culled against bounds of its own (each prop flipped about its origin),
  the cube faces against their 64 m far plane as well (a frame's
  candidates found once for all six). Bounds scale by a matrix's largest
  singular value (3% of Bayview's placements are not scaled rotations).
  The cube went from ~1,060 to ~260 draws a frame in `racing`.
* **Recording off the main thread.** The world's views and the cube are
  recorded and encoded on render threads (blocking workers of the
  renderer's own) from the renderer's fixed part (`WorldCore`); the cars'
  pass and the rest are encoded there too, and submitted in order while
  the main thread records the post passes and the UI, whose own encoding
  also runs there. The swap chain's image is taken only after the scene
  is recorded.
* **Post passes.** Motion blur reads the scene instead of a copy of it,
  the streak buffer's decay reads no texture, and each "copy, then add
  the streak buffer" is one pass: a full-screen pass less in the normal
  frame, two with motion blur. All of them are built at start (motion
  blur's used to compile mid-drive, the first time the car went fast).
* **Shaders.** The world shaders lost a discard that could never be
  taken, so the early depth test holds; the headlight projector's texture
  is read only where its light falls.
* **Cars.** A part's matrix and lighting are made once for all its draws,
  the draws of parts the LOD leaves out get no values, and the lights'
  section test is a bitset; every car renderer shares car.efx's shader,
  layouts and pipelines, so a traffic car coming onto the road no longer
  compiles shaders (its worst frame went from ~7–13 ms to ~1 ms).
* **Loading.** The world and the player's car load side by side, the
  world pipelines are built in parallel, the texture arrays go up one
  copy per mip level for all layers, the bounds are computed in parallel.

Optional features: the texture arrays need `IMMEDIATES` (else a bind
group per texture, as before); the GPU timing needs the timestamp
features (else it is left out). Nothing else depends on the device.

What remains: the post chain is still about half of the GPU's frame on
the iGPU (the bleach bypass's three full-screen passes and overbright's
seven half-size ones read each other's 8-bit output, which only an
emulated 8-bit conversion could fold into fewer passes, not exactly on
every GPU); the UI's two full-screen alpha passes; car draws (≈50 per
car) could share per-part data on the GPU and go by fewer, larger
uniform writes; each garage platform still builds its own world
pipelines (~20 ms per front-end platform change).

