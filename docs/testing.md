# Testing

Overbright's tests come in three kinds:

* **Pure tests** check logic on data written in the test. They always run.
* **Data tests** read the player's own copy of the game (`NFSU2_ROOT`): its
  files, or fixtures regenerated from them. The repository ships no game data.
* **Private tests** replay data that cannot be regenerated from the game:
  recorded traces of the original under Wine, the maintainer's saves. They read
  the private set (`OB_PRIVATE`).

A test that cannot run is reported as **ignored, with the reason**. It is
never reported as passed.

## Running

```
# Without the game: pure tests run, data tests are listed as ignored.
cargo test --release --features dev-harness

# With the game: everything runs.
export NFSU2_ROOT="/path/to/Need for Speed Underground 2"
cargo test --release --features dev-harness
```

The game binary's tests and the integration tests that use the harness's
library aids (`required-features` in Cargo.toml) need the `dev-harness`
feature; without it `cargo test` runs the library's tests only.

Without the game, the summary line of each test binary counts the data tests
as `ignored`, and each one is listed with the reason:

```
test bayview_loads_completely ... ignored, needs NFSU2_ROOT (the player's own game files)
```

`cargo test --release 2>&1 | grep -c 'ignored, needs NFSU2_ROOT'` gives the
number. `scripts/checks/checks.sh` counts the ignored data tests by reason
(`data tests ignored: G need the game files (NFSU2_ROOT), P the private set
(OB_PRIVATE), B both; ...`) and fails only on a skip the run could have
avoided: with `NFSU2_ROOT` set (the checks need it) a test ignored for the
game files alone **fails**; when the private set is here (`OB_PRIVATE`, or a
tree that holds it, `ck_private_here`) or `OB_REQUIRE=private` is set, a test
ignored for the private set (alone or with the game files) fails too. In a
public checkout without the set, P and B are a correct skip: the run passes
and states the counts.

### What "ignored" means

The decision is made when the tests are **built**: `build.rs` sets
`cfg(ob_game)` when `NFSU2_ROOT` is set and `cfg(ob_private)` when the
private set exists, and rebuilds when either variable changes. Rust's
standard test harness cannot ignore a test while it runs, so the choice is
made at build time. This works the same for `tests/*.rs`, for `mod tests`
inside `src/`, and for the game binary's test modules. A runtime harness
such as libtest-mimic would only cover `tests/` targets. The shipped binaries
do not change: only `#[test]` items use these flags.

* **Ignored** (`needs NFSU2_ROOT ...`): the tests were built without
  `NFSU2_ROOT`. Nothing was checked.
* **Failed** with the game set: a load error, a missing file, or a fixture
  that no longer matches its lock is a failure. A data test never returns
  early and never turns a load error into a skip (`.ok()?`).
* Tests marked with a plain `#[ignore = "..."]` (GPU probes, diagnostics)
  run only on request: `cargo test --release -- --ignored NAME`.

### Memory

Every game test (the game binary's `tests::*`) builds a `Game`: its world,
cars and the renderers' data. libtest runs as many tests at once as the
machine has cores. Measured on a 16-core machine (`cargo test` of the game
binary alone, 212 tests, peak resident memory from `/proc/<pid>/status`
VmHWM, a warm asset cache unless said):

| threads | peak | wall |
|---|---|---|
| 1 | 1.1 GB | 114 s |
| 6 | 2.3 GB | 66 s |
| 16 (default) | 4.4 GB; 6.1 GB with a cold asset cache | 55-65 s |
| 16, 9 memory slots | 3.2 GB | 77 s |

(The wall times vary by ±15 s with the machine's other load.) So about
0.3-0.5 GB per running game test, 1.1 GB for the largest. A machine with
many cores and little memory would still run out, so the game tests take
a **memory slot** before anything else (`tests::test_isolation::slot`,
taken by `Game::new` and by `exclusive()`): the number of slots is the
memory available when the first game test starts (`MemAvailable`), less
2 GB kept free, in 1 GB steps, at least one; `OB_TEST_GAME_SLOTS=N` sets
it. A 16 GB machine with 12 GB available runs at most 9 game tests at once
whatever its core count; with 30 GB available (the reference desktop) the slots
never bind and the run is as fast as before. `RUST_TEST_THREADS` is not
needed for `cargo test` to fit in memory.

## Writing a data test

```rust
mod common; // in tests/*.rs; inside src/ use crate::testkit

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn bayview_loads() {
    let root = common::game_root();
    let world = World::load(&root, "L4RA").unwrap(); // a load error fails the test
    ...
}
```

Rules:

* Get the game only through the kit: `game()`, `game_root()`. Do not read
  `NFSU2_ROOT` in a test, and do not use `nfsu2::game_root()`, which falls
  back to the current directory.
* Copy the `ignore = "..."` text exactly as shown above, because the check
  scripts count it. If `game()` is called from a test without the attribute,
  that test fails with a message that names the missing attribute.
* A test that needs both the game and the private set uses
  `#[cfg_attr(not(all(ob_game, ob_private)), ignore = "needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)")]`.
* Do not call `std::env::set_var` in a test. Test binaries run their tests
  on several threads, and `set_var` races every other thread's environment
  reads. Use a scoped setting instead:
  * The asset cache of a test: `test-cache` in the target folder
    (`platform::dirs::test_cache_folder`), set by the kit's `game()` and
    used by every cargo test binary built with `dev-harness`; never the
    player's cache nor the developer cache. `NFSU2_CACHE_DIR` comes first (the check scripts set it).
  * `nfsu2::cache::override_dir(dir)` returns a guard that points the asset
    cache at `dir` until it is dropped. The setting applies to the whole
    process, so such a test goes in a test binary of its own
    (`tests/world_cache.rs`).
  * `testkit::Scratch::new(tag)` makes a temporary folder for one test and
    removes it when dropped.

Where the kit lives: `tests/common/mod.rs` is one source file, compiled into
each place that uses it:

| From | How |
|---|---|
| `tests/*.rs` | `mod common;` |
| `src/**` unit tests | `crate::testkit` (declared `cfg(test)` in src/lib.rs) |
| game binary test modules (`src/bin/game/*.rs`) | `#[path = "../../../tests/common/mod.rs"] mod testkit;` inside the module |

### The player's folders are off limits

A test never reads or writes the player's own folders (`<data home>/overbright`,
`<data home>/NFS Underground 2`, `<cache home>/overbright`, docs/platform-dirs.md),
not even a file it removes again. When code under test has a default path
there (the launcher's start marker, for one), give it a way to take the path
from its caller and pass a folder of the test's own; a
process-wide `std::env::set_var` of `XDG_DATA_HOME` / `NFSU2_DATA_DIR` is no way
out, since the other tests of the binary run beside it on other threads. The
checks' tests step runs the suite with those folders masked
(`scripts/checks/home_mask.sh`) and fails when anything landed there.

## Fixtures from the game, and the fixture lock

The repository does not carry copies of game data. A test that needs a table
from the game regenerates it from the player's files:

```rust
let raw = testkit::fixture("burnout-records", 1, |_| testkit::exe_bytes(0x7E_A2B0, 16 * 0x1C));
```

* The first call per game runs the generator and stores the result in
  `$XDG_CACHE_HOME/overbright-test-fixtures/<game fingerprint>/<name>@<version>`
  (default `~/.cache/...`), outside the repository. The game fingerprint is
  the SHA-256 over the hashes of the files that `src/fingerprint.rs` checks.
* `tests/fixtures.lock` pins each fixture's SHA-256 and size:
  `name@version sha256 size  # source`. The pin is the hash of the file the
  repository used to carry. Every load, whether from the cache or freshly
  generated, must match it. A match proves that the generator reproduces
  exactly the bytes the test was written against, so the test keeps its full
  value. A mismatch **fails** the test and names the game build. If the
  player's game is another build or a modded install, the test fails with
  that message. It is not ignored.
* When a generator's output is meant to change, bump the version and the
  lock line in the same change.
* Shared generators live in the kit, for example `handling_peugot()`, the
  stock PEUGOT car type record from GLOBALB.BUN.

To add a fixture that replaces a committed file:

1. Write the generator, using the game's own parsers (`nfsu2::formats`,
   `nfsu2::car`, ...).
2. Add a lock line with `sha256sum` of the committed file and its size.
3. Replace the `include_bytes!` with the `fixture(...)` call and mark the
   test as a data test.
4. Run the test with the game. Only when it passes may the committed file
   be deleted.

Converted so far: `burnout-records@1` (src/shop_scene/wheels.rs) and
`handling-peugot@1` (src/drive/, src/drive/native_cached_count.rs).
The engine's tables are in the code (`nfsu2::exe_tables`, docs/exe-tables.md):
a test pins the SHA-256 of each table's `Debug` text, and the data tests that
compare them with the original read the reference executable through
`testkit::reference_exe()` (`$OB_PRIVATE/exe/SPEED2.EXE`, else the game
folder's; it must be the European PC 1.2 release's `SPEED2.EXE` without copy protection)
and carry the "game files and private set" ignore reason. `exe_bytes(va,
len)` reads a range of it for a fixture. The fresh
profile is written by `career::fresh_payload` and pinned the same way. The
old files stay in docs/evidence until a later release step deletes the G-class
files.

## Private data

```rust
#[test]
#[cfg_attr(not(ob_private), ignore = "needs OB_PRIVATE (private captures and saves)")]
fn replays_the_capture() {
    let raw = testkit::private("docs/evidence/cached-wheel-count/airborne.bin");
    ...
}
```

`private(path)` reads `$OB_PRIVATE/<path>`. Without `OB_PRIVATE`, it reads
from the repository itself for now, because the captures are still there.
When the private set is absent, the test is ignored with the reason. When the
set is present, a missing file fails the test. Without `OB_PRIVATE`, the
repository counts as the private set only while it still holds
`tests/fixtures/native_saves/` (build.rs): the public repository has none,
so there every private test is ignored with its reason. A test never `include_bytes!`
or `include_str!` a private file: the public tree would not compile.

The developer harness reads the maintainer's saves the same way at run time
(`src/bin/game/dev/private.rs`: `$OB_PRIVATE`, else the repository). A step
that needs one and does not find it prints
`SKIP: <step>: needs the maintainer's private set (OB_PRIVATE)`: the
selftest's resumes of the original's own saves are skipped and the selftest
goes on; `--flow engine` skips its finished-career step; the `finished_career_*` shots,
`garage_neon`, `--flow rain` and `--flow outrun` have nothing else to run
and exit with status 3. `scripts/checks/checks.sh` records these as
skipped (`summary.json` `skipped`), never as passed or failed.

That skip is for a harness built without the set (a public tree). A harness
built with it (build.rs set `ob_private`) whose set is gone or moved does not
skip: the step fails with `private file <path> is missing, but this harness was
built with the private set: ... set OB_PRIVATE to its folder` and the process
ends with status 1. Likewise `scripts/checks/checks.sh` accepts no such skip in
a tree that holds the private set (`ck_private_here` in scripts/lib/common.sh,
the rule of build.rs): there the step fails instead of being recorded as skipped.

## The checks

`scripts/checks/checks.sh` runs `cargo test --release --no-fail-fast --features dev-harness` with
`NFSU2_ROOT` set. It fails if any test failed, or if any data test was
ignored for lack of `NFSU2_ROOT`, and it prints the data-test ignore counts
(docs/checks.md).
