# The start's and the menus' waits

What a player waits for between the game's start and driving, outside a race's own load: the start to the
first frame, the boot screens and the title, the menus' steps, the garage's scene, the first frame of free roam.
Measured in the shipping game's own wait lines (docs/developing/waits-log.md). None of this is native behaviour: the
original reads its files and makes its objects when it needs them; here the same objects are made earlier or
off the frame that shows them, so nothing drawn, heard or simulated changes. The opt-ins are the Skip Intro
Movies and Skip Title Screen rows (below, and docs/playing/native-deviations.md).

## What a wait is made of (measured before this work)

The shipping game in a window on a private display, keys pressed as a player presses them, the wait lines'
"from the key" times; the main thread's state sampled every 4 ms from /proc (running, waiting for another thread,
blocked on the disk and in which call), and profiles of the main thread (samply). The table and the method are in
`docs/evidence/perf-budget/start-and-menus.txt`.

* **The start** (process start → the first boot screen's frame, 90–170 ms on a quiet machine): one chain on the
  main thread — the game's data (`Game::new`, 37–47 ms), the controllers, the window's event loop (44–59 ms on X11),
  the window, the Vulkan instance (the driver's libraries, 12 ms), the adapter and the device (8–172 ms), the swap
  chain, the 2-D renderer, the sound device (6–22 ms with a sound server; up to 2 s is waited for).
* **The title screen**: 4.0 s before Enter is taken, every start (0x4C564E: 16000 ticks of 0x8651A8 after the
  screen is up). Native.
* **The main menu's first frame** (after the profile message, 40–90 ms, up to 0.6 s when the system had dropped
  the files from its cache, 0.9 s on a first start): the garage scene's first tick read and parsed SHOWROOM's
  platform (11 ms), made its renderer, loaded the FE car and made its renderer; on a first start the scene's
  programs. The original reads SHOWROOM with the front end at its start (0x57FEFA), before the boot screens.
* **Free roam's first frame** at the Graphics rows' top tiers (4K, 130–330 ms): 136 ms of main-thread time, of
  it Surface Detail's map array zero-filled from the CPU (44 ms), Ambient Detail's volume buffers zeroed twice
  (31 ms) and the world's collision made by the drive's first placing (16–20 ms, at every setting).
* **A cache file older than ten minutes** had its time set at its open (the pruning's "used" mark) on the
  thread that opened it: a metadata change, which on ext4 waits for the journal's commit in progress, and a
  commit waits for the data being written (`data=ordered`): 0.1–0.2 s with a 150 MB write and fsync beside it,
  2.2 s once with the machine's disk busy, on three loading threads at once.

## What changed

### The start side by side (`shell::start_ahead`, `Gpu::ahead`, `Mixer::start`)

From the moment the options are in force (`Game::new_beside`'s `early`), the graphics device is opened on a
thread of its own and the sound device's stream is asked for (`Mixer::start`); the window's event loop is made on
the main thread while the front end's packages and the career's data load on theirs (`beside`). The window takes
the device when it is made (`GpuAhead::take`) and the sound when the window is up (`MixerOpening::finish`, which
waits what is left of the old 2 s from the start).

* **The same device.** `Gpu::ahead` picks the adapter as `Gpu::new` does with no surface. On Vulkan wgpu ranks
  the adapters by kind with a stable sort and only filters them by the surface, so when the first of all can show
  the window it is the first of those that can; on Windows the candidates are ranked by `backend::adapter_order`
  and the first one, opened, is the pick when it presents. `take` checks `is_surface_supported` and otherwise
  returns nothing: the window then opens a device as before (`Gpu::new` with the surface). The device's facts
  that the rest of the game reads (ray queries, the GPU class, the sample counts, HDR, the crash report's
  adapter) are set by `Gpu::publish` when the window takes it, at the point `Gpu::new` set them; the options
  that choose the device's features (the Ray Traced tier) are in force before the thread starts.
* **The same sound.** The stream renders the mixer's mix from its opening, as before: an empty mix is silence
  until the boot movie's first sound (the first boot screen, LS_BlankMovie, has none).
* `NFSU2_HEADLESS_ONLY` starts neither (it opens no window or sound device); `NFSU2_NO_START_AHEAD=1` turns it
  off (the old order).

### The main menu's garage scene during the title (`garage::ahead`)

When the boot reaches the title screen (`boot_screen(BootStep::Splash)`), a thread reads the main menu's scene as
its first tick would: SHOWROOM read and parsed (`garage_platform`), the front end's light rigs, and the FE car as
UI_Main asks for it (`Game::main_menu_ride`, the same choice `fe_car_ride` makes under UI_Main: the car, its parts
and performance parts), the parallel parts on the loads' own pool (`Preload::pool`: the title's frames never queue
behind them). Nothing of it touches the GPU. The menu's first tick takes it when it is done and the request is
that one (the same platform and car key) and is then not a fresh visit: it makes the platform's and the FE car's
renderers, as before, with the window and options in force then (the front end's cube follows the platforms in
the order they are shown, `fe_cube_texels`). A scene not done yet is never waited for; anything else drops it and
the tick makes its own; any garage request ends it. Made in the job, the renderers' GPU work shared the device
with the title's frames, and on a first start their pipelines waited for the programs being made ahead (the menu
then waited for the scene, 0.1 → 2.3 s once). `NFSU2_NO_PRELOAD=1` turns it off with the other work ahead.

### Free roam's first frame

* Surface Detail's array (`SurfaceMaps::new`) is zeroed on the GPU: copies from one zeroed buffer of sixteen
  top-mip layers, recorded and submitted at once, before any bind group holds it (the first-write hazard,
  docs/notes/performance/main-thread.md). The same zeros.
* `VolumeFx::new` no longer writes zeros into its accumulators and particles after `zeroed_buffer` has made
  them zero.
* The world's collision (`World::collision_ahead`) is made beside the world's load: on the menus' preload
  thread (`Preload::world`) and on the load's own world thread. Only from the game's collision packs (fixed by
  the load; the barrier switches are read through the shared `BarrierSwitches` when queried); a track without
  packs builds its stand-in from the placements, which the drive's set-up may change, on first use as before.

### Cache files marked as used off the loading threads (`cache::touch_later`)

The mark (`touch`, `touch_file`) is queued for one toucher thread and made within milliseconds, in the order
asked. The times the pruning reads ("used within the hour" for another build's entries, the oldest first under
the size limit) are the same: the cache's flush and the game's exit wait for the queue (`wait_touches`).

### Skip Intro Movies and Skip Title Screen (Enhancements, opt-in, default Off and Off)

Two independent On/Off rows (`movies.rs`, `Game::boot_screen`), both Off by default: the native start (the
boot movies, which Enter skips, then the title screen and its 4 s hold).

* **Skip Intro Movies** (`enh.skip_movies 1`): each boot screen (LS_BlankMovie, EA, THX, the warning screen,
  the intro) passes as a missing movie file does (0x566250): in the same tick, no file opened, no sound. The
  log says `boot: <name> skipped (Skip Intro Movies)` for each. The title screen follows unless Skip Title
  Screen is on.
* **Skip Title Screen** (`enh.skip_title 1`): the profile step where the title stood (its music starts as the
  title starts it); log `boot: the title screen skipped (Skip Title Screen)`.
* Both: the profile step on the start's first tick. A profile message or the profile list still asks for its
  key, as it does natively.

options.txt: an older file's `enh.skip_title 2` (the former single row's "Movies and Title Screen") reads as
both On and is written back as `enh.skip_title 1` + `enh.skip_movies 1` (`Settings::from_text`,
`Enhancements::set_skips`); `enh.skip_title 1` keeps its meaning. The launcher's main screen has the same two
values as two checkboxes ("Skip the intro movies", "Skip the title screen"), read from and written to the same
lines (docs/launcher.md "Start choices"), so the game's rows and the launcher always agree.

Observed (`docs/evidence/start-and-menus/skip-boot.txt`: the game windowed on a private display with no sound
device, no key pressed): both On, every boot screen passes in the start's first tick and the next screen is
the profile step (`wait: boot Blank -> menu McMain`); movies only, the title screen comes at once (`boot Blank
-> menu Splash`); an older `enh.skip_title 2` behaves as both; Off and title only play the EA and THX movies
first.

## The launcher's prepare step in the background

docs/developing/prepare.md "The launcher's step": offered by itself, the step now runs behind the main screen; the line
beside Play says what it does, Play stops it (the game makes the rest). "Prepare again" opens its screen as
before.

## Checks

`tests::start_and_menus` (Skip Intro Movies and Skip Title Screen each alone, both, and an older file's
`enh.skip_title 2`; the main menu's scene made during the title
taken by its first tick and drawing the same pictures as a scene the tick made), `render::enh::assets::surface::
gpu::tests` (the array zero in every layer and mip after it was filled with other bytes),
`render::core::upload::zeroed_tests` (a zeroed buffer reads zeros), `world::query::collision_ahead_tests` (the same
collision made ahead, on another thread, as on first use; none made ahead without packs), the cache's prune test
(a mark waited for), the launcher's prepare tests (offered behind the main screen, its line, every size) and the
launcher's fresh-machine run (tests/launcher_fresh.sh step p).
