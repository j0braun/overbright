# Prepare: the first drive's work done ahead (`game --prepare`, the launcher's step)

Not native: SPEED2.EXE has nothing like it. It changes nothing that is drawn, heard or simulated;
it only does, before the game is started, work the game otherwise does during its first loads.
The game never needs it: without it the same work is done in the menus' background
(docs/first-use-builds.md "Ahead of the load") and, for what is still missing, by the load.

## What a first drive needs

Measured on empty caches (docs/perf-budget.md "Load times"): the track's parse into the asset
cache (docs/asset-cache.md), the world renderer's data (the texture arrays as uploaded), the cars',
and every shader program of the settings in force, which the graphics driver compiles once and
keeps (0.25 s at Original, up to 2.4 s at High on the reference desktop).

## `game --prepare`

No window (`shell::prepare`; like `--probe`, but it reads the game folder and options.txt, as the
game does). It makes, into the player's cache folder:

1. **the programs** of the settings in force: `enh::warm::drive_programs` (every pipeline and
   shader module a drive's frames take that depends on the settings and the window's format
   alone, the post chain's the boot screens draw with, and the FE car's floor reflection in the
   menus' garage scene) and the 2-D renderer's for the menus and the HUD, at the scene's sample
   count a drive's frames draw with (`display::drive_msaa_samples`: none under TAA or a temporal
   Upscaling tier, whatever the native Full Screen Anti-Aliasing; FSR 2's passes and FSR 1's
   included);
2. **free roam's world**: the track file parsed, the renderer's data, the renderer's own
   programs (`WorldRenderer::warm_drive`) and the world's effect objects' (`world_programs`), as
   the menus' load ahead does it (`Preload::world`); the current profile's car beside it;
3. **the first career race's region**, the same way, when it is not free roam's;
4. the cache writes flushed and **the stamp** written last: `prepared.txt` in the cache folder
   (the build id, the adapter and its driver, and the settings' program key:
   `graphics_table::program_key`, a digest of the Graphics rows' Enhanced tiers as options.txt has
   them; the Customize Display values are left out, since the game's first run sets them for the
   hardware after a first prepare).

The window's format the programs are made for is the one the game's window last chose on this
machine (`window-format.txt` in the cache folder, written when the window is made), else the
first of the window's preference order (8-bit BGRA, what a surface offers nearly everywhere). A
window that picks another format makes its own programs as before.

Output: `key=value` lines on stdout (`launch::prepare` in the library defines them, `launcher::prepare` reads them; other lines are ignored).

```
prepare=1
step=1/5 graphics device        (each step as it starts)
adapter=<name> (<backend>)
pipeline_cache=1                (0: the backend keeps none on disk)
step=2/5 shader programs
step=3/5 the city
step=4/5 the first race's track
step=5/5 saving
pipelines=<made in this run>
ok=1                            (error=<why>, ok=0 and exit code 1 otherwise)
```

**Per backend** (wgpu 30):

* **Vulkan**: wgpu's pipeline cache (`Gpu::pipeline_cache`, `render::core::pipeline_cache`) is
  the driver's own cache data, kept in the cache folder under wgpu's key for the adapter and
  written whole (`cache::replace_file`). A later start makes its pipelines from it. It is written
  again only when the driver's data has grown past what the driver held right after reading the
  file (taken once at the start: about 1 ms for a 10 MB file). Against the file's length, the same
  pipelines could look grown: Mesa writes its objects in its hash table's order, each aligned to 8
  bytes, so the same objects can come out 4 bytes longer after a restart, and a second prepare run
  wrote the file again (seen with subgroup operations on the device, whose pipeline keys differ;
  the check's step 2 failed in about half the runs). The writes run one at a time, each taking
  the driver's data when it runs, so an earlier save's smaller data cannot land after a later
  one's (two saves are made in a run: after the programs and at the end; the base failed the
  check once in six loaded runs with a first file short of objects, **inferred** to be that;
  `docs/evidence/perf-budget/prepare-cache-rewrite.txt`).
* **DX12**: wgpu has no pipeline cache there (`pipeline_cache=0`). The run still parses the
  tracks and makes every program once, which fills the driver's own shader cache where it keeps
  one; what wgpu itself does per pipeline at each start (the shader translation and the HLSL
  compile) is repeated at each start, in the background from the first frame on ("Ahead of the
  load"). **Not measured on a Windows machine** (open); under Wine the DX12 step runs only with a
  DXC given (the Wine smoke's `dx12_prepare`, expected to fail without one, as `dx12_main`).

**Stopped half way** (Skip, a crash, the power): every cache file is written to a temporary name
and renamed into place, and the stamp comes last, so there is no stamp and no half file; the next
run, or the game, makes what is missing. A second run on a prepared folder changes no file.

`game --probe` prints `prepared=1` when the cache folder's stamp is for this build, adapter and
driver and for the Enhanced Graphics rows options.txt has now, else `prepared=0`; and
`prepared.programs=<key>`, the stamp's key, when the stamp is for this build, adapter and driver.
A level changed since the prepare run (in the launcher's Graphics row or in the game) therefore
reads as not prepared.

## The launcher's step (`launcher::prepare`, `Screen::Prepare`)

* **Started by itself** once a session, when the hardware check says `prepared=0` for the game
  folder in use and the main screen is up: on a first run right after the folder is chosen, and
  again after an update of Overbright or of the graphics driver, or another graphics card. It runs
  **behind the main screen** (`Prepare::background`): no screen of its own, Play stays first, and
  the line beside Play says what it does ("Ready to play · getting ready in the
  background: shader programs (2 of 5)"). **Play stops it**: the cache files it wrote are
  whole and the driver keeps the programs it made; the game makes the rest as it does without the
  step. A player who spends the step's seconds in the launcher (a first run chooses the folder and
  shows the hardware check) starts the game prepared without waiting on a screen for it (it was
  an offered screen with Skip, 1–3 s on the reference desktop).
* **Started again** once a session the same way when the Graphics row saves another level than
  the one the hardware check found prepared (`Prepare::prepared_graphics`); setting the level back
  starts nothing (one start by itself per session in all).
* **Prepare again** on the main screen opens the step's screen: what it is for, a bar (the steps
  finished) and the step's words, and **Skip** all the time (Skip stops the child and goes to the
  main screen).
* Done or failed: one line and **Continue**. A failure is a note, never an obstacle ("The game
  works without it").
* The launcher only starts the game with `--prepare` and reads its lines; it makes nothing
  itself and opens no graphics device.

## Numbers (reference desktop, discrete GPU shared with another program; 3 runs each)

| | Original | High | Extreme |
|---|---|---|---|
| `--prepare` on empty caches (the game's and the driver's) | 0.6 s | 1.7–2.2 s | 1.9–2.1 s |
| again on the prepared folder | 0.35–0.56 s | 0.59–1.0 s | 0.65–0.70 s |
| pipelines made | 181 | 409 | 423 |
| first load after it, a new process (`load_ms`) | 258 ms | 397–425 ms | 408 ms |
| the same first load on empty caches | – | 1455–1491 ms | – |

The prepared first load is a warm one (warm loads of the same binary on the same shared GPU:
docs/perf-budget.md). The cache folder holds about 260 MB after it (the city's track file).

## Checks

* `scripts/checks/prepare.sh OUTDIR GAME` (a harness build; `PREPARE_OPTIONS=file` for an
  options.txt): on folders of its own, the first run ends with `ok=1`, the stamp and (Vulkan) the
  pipeline cache; the probe says `prepared=1`; a second run changes no cache file; a first load
  after it leaves the pipeline cache file as it is (every pipeline the load made was in it); a
  run killed during its second step leaves no stamp, the next run ends with `ok=1`, and
  `--verify-cache` passes; the same stamp with another Graphics level in options.txt (SSAO on)
  makes the probe say `prepared=0`. Passes at the defaults, High and Extreme. The self-check's
  stand-in game answers it (`levelstale`: a probe that ignores the level must fail the check).
* `launch::prepare::tests` (the lines), `launcher::prepare::tests` (the state), `launcher::tests::
  the_prepare_step_is_offered_once_skippable_and_can_be_asked_for` (started once behind the main
  screen, its line there at every size, Prepare again with Skip and Continue, every state's layout
  at every size),
  `the_prepare_step_is_offered_again_after_the_graphics_level_changed`, the renders `prepare-running`,
  `prepare-done`, `prepare-failed`.
* The Wine smoke: `prepare` (Vulkan through winevulkan: `ok=1`, then `prepared=1` from the
  probe) and `dx12_prepare`.

## Open

* A Windows machine: the time on DX12, and whether its drivers' own shader caches make a second
  start faster after it.
* Closed: the menus' own 3-D scene (the garage) and the boot screens draw nothing in the prepare
  run, but their programs are the ones it makes: measured on empty caches (the driver's shader cache and the
  game's cache folder new, Original, the integrated GPU, one run each), the boot screens' post chain
  and 2-D renderer took 13.7 and 9.6 ms of pipeline making without the prepare step and 2.0 and
  1.2 ms after it (a warm start's), the main menu garage's world pipelines 1224 → 86 ms (warm: 86) and
  its cars' 76 → 5.6 ms. The FE car's floor-reflection set is in the cars' programs, and in a load's
  warm-up (`car::warm_reflection_set`) for the shops' garage scenes. Checked in-process at every
  preset: the main menu's garage after the boot screens and every shop kind's garage scene after a
  free-roam load make no program (`tests::first_use`). A start with Enter held reaches the garage
  only after the title's 240 or so frames (about 0.2 s), by then the work ahead is done: measured
  (`--bench boot`, `NFSU2_BENCH_BOOT_MENU=1`, the integrated GPU, one run each, warm and empty caches,
  Original and High) the frame that makes the garage builds no pipeline.
* Regions beyond the city and the first race, and Surface Detail's maps, are made when first
  needed (a region is about 0.5 s of parsing and up to 250 MB of cache).
