# Test determinism (2026-11-04)

Test infrastructure only: no game behaviour changes (the library hooks below
are inert unless a test enters them).

## The flakes and their cause

Seen 2026-11-04 in loaded full `cargo test --release` runs, passing alone:
`tests::wrong_way_street_x` and `tests::rivals_keep_the_race_route`
("Darren 0.27 laps back"); the drag/drift/outrun parity saw a parallel test reset a global
Enhancements flag.

libtest runs every test of a binary on its own thread, side by side, in one
process. State that lives process-wide is therefore shared by whatever
tests run at the time:

| state | where | who wrote it | who read it |
|---|---|---|---|
| current profile `profiles::CURRENT` | src/frontend/profiles/ | every test whose game commits or saves a career (`Profile::commit/save` → `set_current`): the shop tests' a community save, `FIRST`, `DriftOrderTest`, `SHOP_EXIT_AUDIO_TEST`, … | `Game::new` (`Profile::load`, `Settings::load` → the profile's options), the menus |
| data folder `XDG_DATA_HOME` (profiles on disk, options.txt) | `profiles::data_home`, `Settings::path` | `Game::new` set it once per process; `rivals_hold_the_line_and_launch_at_full_throttle` re-pointed it mid-run for the whole process; profile/option saves | `Game::new` (`profiles::boot` loads a lone profile on disk; `Settings::load_text`) |
| the Enhancements' atomics (`qol`: ~50 values), `display` atomics, output size, photo mode, `UNLOCK_ALL` | src/qol/, src/render/display.rs, src/render/enh/post/blur_dof.rs, src/frontend/career/ | `Settings::apply_enhancements` — run by **every** `Game::new` — and tests that put values in force (the ignored nitro-flame GPU tests, the library's reflection / HDR / DOF tests) | rendering, world streaming (`keep_loaded`), model fixes, menus |

So a race test started in whichever profile another test had left current —
its car, stage, parts and options — and so drove a different car against a
different field. **Observed** (`NFSU2_TEST_SHARED_STATE=1`, the old shared
state, 20 runs of the game test binary, machine load 50–230 on 16 threads):
the two race tests started in 13 different profiles (a community save LANCEREVO8
stage 4 with 43–51 parts, a community save RX7 with 58, a community save ESCALADE, `NAME` stage 6,
`DriftOrderTest`, `SHOP_EXIT_AUDIO_TEST`, …; never none) and the four races
of `rivals_keep_the_race_route` ended in 10–11 distinct outcomes out of 20; with the sandbox the same
binary gave **1** outcome in 20 runs and always no profile. The 20 shared
runs did not fail themselves (the margins hold for these profiles; the
probe `race_checks_in_a_foreign_profile_probe` runs the checks in three of
them: all pass), so the exact profile and order behind the two reported
failures is **not reproduced**; what is shown is that their inputs were not
fixed, and now are. The unchanged binary passed 20/20 runs with 16 test
threads and 10/10 with 4 under the same load. Evidence:
`docs/evidence/test-determinism/` (per-run summaries; the outcome lists are internal evidence).

Other process-wide state checked and left as is (no test reads it in a way
that varies): `career::NativeContext` (set once from the same data), the
asset cache (keyed by build id and source stat/hash, written atomically),
`GARAGE_ASYNC` (shots only), profiling/trace statics, the audio statics
(capture off in tests), `drive::PAIRS_MADE` counters. **Open**: a game test
without `game.clock` runs its menus on the wall clock (`Game::now`), so
menu timings under heavy load can differ; no failure seen from it.

## The fix

* **Per-test sandbox** (`profiles::sandbox`, library, `#[doc(hidden)]`):
  `sandbox::enter(parent)` gives the calling thread its own empty data
  folder and its own current profile; `data_home()` (and so
  options.txt (`Settings::path`), backups) and `current_name/save,
  set/clear/update_current` use them on that thread only. The folder is
  removed when the thread ends. Never entered by the game: outside a
  sandbox the code paths are the old ones (one thread-local lookup).
* **Game tests** (`src/bin/game/tests/test_isolation.rs`, `cfg(test)`):
  `Game::new` enters the sandbox for its test thread (once) and takes a
  *shared* hold on the process-wide settings (an `RwLock`, held in a
  thread-local until the test's thread ends, also after a panic). A test
  that changes Enhancements, photo mode, output size or other display
  globals calls `test_isolation::exclusive()` first (done in
  `reported_issues::offscreen_window` and the nitro-window test): it waits for
  the running game tests to end, holds new ones off, and the native
  defaults are put back when it ends. `rivals_hold_the_line…` no longer
  re-points `XDG_DATA_HOME` for the process.
* **Library tests** (`qol::test_globals()`, `cfg(test)`): a mutex guard for
  the tests that put Enhancements / photo mode in force or assert their
  defaults (qol `hdr_output_values`, `enhancements_round_trip`, the GPU
  reflection tests, `hdr_passes_build_and_run`, the DOF test); defaults
  restored on drop. The GPU ones no longer need to run serially.
* New tests: `game_tests_start_from_their_own_sandbox` (another test's
  thread saves a PEUGOT profile and options; a game built after it has no
  profile, the 240SX and default options; fails without the sandbox),
  `profiles::tests::sandboxes_keep_threads_apart`; the ignored probe
  `race_checks_in_a_foreign_profile_probe`. The two race tests print the
  profile they start in.

## Rules for new tests

* Build games with `Game::new()`; never set `XDG_DATA_HOME` in a unit
  test (`tests/*.rs` integration binaries may: one process per file).
* Before changing a process-wide value (an Enhancements row and
  `apply_enhancements`, `qol::set_*`, `display::set_*`, photo mode,
  `UNLOCK_ALL`, a new global flag such as the drag/drift/outrun parity's Outrun Variety), call
  `test_isolation::exclusive()` (game tests) or hold
  `qol::test_globals()` (library tests). Add a new global's default to
  `test_isolation::reset_globals` / `TestGlobals::drop`.
* `NFSU2_TEST_SHARED_STATE=1` turns the game tests' sandbox off, to
  reproduce the old behaviour.
* Read a drawn frame back only after `WindowState::settle`: the frame's
  last command buffers are submitted from a worker (`FrameEnd`), so a
  read-back queued right after `draw` can copy the target before them and
  return the frame before. On a loaded machine this made the pictures of
  `tests::first_use`'s car-change, kept-water and rivals tests differ
  between two identical runs (the previous frame read: the Engage dialog
  in place of the race's first frame); `tests::first_use::picture`.

## Proof (2026-11-05)

* 30 consecutive full `cargo test --release` runs with the fix: **30/30
  exit 0, 0 failed** (45 test binaries, ~881 tests each, 13–21 s per run,
  1-minute load average 19.8–77.7 on 16 hardware threads from the other
  builds and Wine sessions): `full30-summary.txt`. (One run counts
  880 passed: a child process's line interleaved with the lib binary's
  output, not a missing test.)
* The game test binary 20× sandboxed: 20/20 passed, every race of
  `rivals_keep_the_race_route` the same outcome in all 20 (`iso20`), vs
  10–11 outcomes in 20 with the shared state (`shared20`).
* `--test-threads=1` still gives each test its own thread (the sandbox
  holds): 57 passed.
