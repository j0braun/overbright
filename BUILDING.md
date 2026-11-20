# Building Overbright

## What you need

- **Rust** 1.98 or newer (edition 2024), from rustup or your distribution. The release container pins the
  exact version (`scripts/release/container/Containerfile`).
- **Linux:** a C compiler (mimalloc, the default allocator, is C), the usual X11/Wayland, ALSA and udev
  development headers for winit, cpal and gilrs (on Debian/Ubuntu: `build-essential pkg-config
  libasound2-dev libudev-dev`), and a Vulkan driver to run the game.
- **Your own copy of the game** to run it and to run the data tests (see below). The repository contains no
  game data.

## NFSU2_ROOT

`NFSU2_ROOT` names the folder of your installed game, the one that holds `GLOBAL/`, `FRONTEND/`, `TRACKS/`
and the rest (`SPEED2.EXE` is not needed):

```bash
export NFSU2_ROOT="/path/to/Need for Speed Underground 2"
```

The game itself also finds the folder without it (the launcher's choice, `game --set-root`, the current
folder; see the README). The tests and the check scripts use `NFSU2_ROOT`. The check and perf scripts take it
from the environment, and also read machine-local settings from `scripts/local.env` when it exists
(git-ignored; copy `scripts/local.env.example`; optional, it can export `NFSU2_ROOT` for you).

The tests decide at **build** time whether the game is there (build.rs): after setting or changing
`NFSU2_ROOT`, the next `cargo test` rebuilds the test binaries.

## Building on Linux

```bash
cargo build --release                       # the shipping game and tools: target/release/game, overbright (launcher), overbright-editor, …
cargo build --release --features dev-harness --bins --examples
                                            # the game with the developer harness, the tools and the public examples
```

- `game` is the game, `overbright` the launcher; `inspect`, `world`, `car_check` and the other binaries in
  `src/bin/` are developer tools (docs/developing/developer-guide.md). `overbright-editor`, the map editor, is the crate
  `crates/ob-studio-gui`, a default workspace member: the commands above build it too, and `cargo build
  --release -p nfsu2` leaves it out (the game itself never links its UI library, egui).
- The **developer harness** (`--shot`, `--selftest`, `--flow`, `--bench`, `--playthrough`, the parity and
  audio-capture modes; `src/bin/game/dev/`) is the Cargo feature `dev-harness`, off by default. A build
  without it is the shipping game and answers those flags with a usage error
  (`scripts/checks/release_check.sh` checks that). The tests and the checks build with the feature.
- `--no-default-features` builds with the system allocator instead of mimalloc;
  `--features profile-tracy` adds Tracy profiling scopes.
- The developer probes (`devprobes/`) are part of the maintainer's private set and not in the public
  repository; with the set laid over a checkout they are a workspace member, checked with
  `cargo check --release -p devprobes`.
- `scripts/pgo.sh` makes an optional profile-guided build (docs/developing/developer-guide.md, "PGO").
- docs/developing/build-times.md has measured build times (cold, after a one-file change) and their method.

## Tests

```bash
cargo test --release --features dev-harness
```

- **Pure tests** need nothing and always run.
- **Data tests** read your game files through `NFSU2_ROOT`; without it they are reported as *ignored*, with
  the reason `needs NFSU2_ROOT (…)`, never as passed.
- **Private tests** replay recordings of the original that cannot be regenerated from the game files; they
  need the maintainer's private set (`OB_PRIVATE`) and are ignored with their reason otherwise.

docs/developing/testing.md has the details and the test kit. `scripts/checks/provenance.py` checks the provenance tags
(docs/developing/provenance.md); `scripts/checks/fuzz.sh` runs the parser fuzz smoke (docs/developing/fuzzing.md).

## The checks

The check harness builds, tests and renders the game headless and compares the result (docs/developing/checks.md). It
needs `NFSU2_ROOT` (through `scripts/local.env`) and writes everything into the output folder you give it:

```bash
scripts/checks/checks.sh fast <outdir>                      # build, tests, self-test, the fast shot set, the flows
scripts/checks/checks.sh full <outdir>                      # every shot target twice, matrix, fuzz, release check, …
CHECKS=full scripts/checks/gate.sh <outdir> <base commit or base dir> [extreme]
                                                            # the gate: checks + parity of this tree against a base
RATE=1 CHECKS=full scripts/checks/gate.sh …                 # plus the rate gate (30/60/120/200 Hz)
```

`scripts/checks/checks.sh fast` ends with one line, `CHECKS: PASS (fast, skipped: ...; N s)` or
`CHECKS: FAIL (...)`; `RELEASE=1` adds the shipping-build check to the fast set. All checks run headless (no
window on your desktop). The gate compares frames, audio and simulation state of
the new tree with the base and reports every difference; a change that is meant to change the output says so
and explains each difference (CONTRIBUTING.md).

## Without the private set

The maintainer keeps a private set of files that is not in this repository (test data such as saves of the
original, developer notes and tools). It is not needed to build, test or check a change; without it these are reported, never passed silently:

- `cargo test`: the private tests are *ignored* with the reason `needs OB_PRIVATE (…)` or
  `needs NFSU2_ROOT (…) and OB_PRIVATE (…)`; the checks count them apart from the game-file tests
  (`data tests ignored: …`) and pass.
- the self-test's resume steps from the original's saves, the shots that need a private save
  (`garage_neon`): `SKIP: … needs the maintainer's private set (OB_PRIVATE)`.
- the release check and `scripts/release/package.sh`: the maintainer's privacy scan is not in the repository
  (`privacy scan: maintainer tool, not in this checkout`); every other check runs, and the run ends with exit
  status 3 instead of 0.
- `scripts/tests/` (the importer tests) and `devprobes/`: not in the repository, recorded as skipped
  (`devprobes.sh` says so and exits 3).
- the gate's parity: the shots that need a private save (`shots: SKIP (…)`) and the `parity` scenario of the
  sim gate (`sim parity: SKIP (…)`); the last line counts them (`GATE: PASS: …; skipped: checks N, parity M`).
- `scripts/checks/halo.sh` and `scripts/map_xtest.sh`: their default saves are private: `SKIP`, exit status 3,
  unless you name a save of your own (`HALO_SAVE`, `SAVE:NAME` arguments).
- `scripts/checks/perfgate.sh`: the two "max" configs of its set need a play setup of your own
  (`PRESET_user` in `scripts/local.env`); without it they are left out and the verdict names them.

Some technical documents mention evidence, scripts or tools of the private set ("kept with the maintainer's
private evidence", "a private developer probe", "the private tool `parity.py`"); they name them without a path,
and the text states what was found.

## The release container (Linux and Windows builds)

The binaries that are shipped are built in a container, so they do not depend on the build machine
(scripts/release/container/README.md):

```bash
scripts/release/container/build.sh <commit> <out dir>                 # both targets from `git archive <commit>`
scripts/release/container/build.sh --linux <source dir> <out dir>     # a working tree
scripts/release/container/build.sh --windows --dev-harness --tests --fast <source dir> <out dir>
```

- Needs podman (rootless is fine), git, rsync, python3 and binutils. The first run builds the two images:
  Windows in an Ubuntu 22.04 image (pinned by digest, mingw-w64 from Ubuntu), Linux in an AlmaLinux 8 image
  (pinned by digest); both with Rust from the official archives pinned by version and SHA-256.
- **Linux:** `x86_64-unknown-linux-gnu` against glibc 2.28, so the binary runs on any x86-64 distribution
  with glibc 2.28 or newer (the distributions it was run on: docs/developing/building-releases.md, "Linux: what a player
  needs"). A binary built directly on a newer system usually needs that system's glibc.
- **Windows:** `x86_64-pc-windows-gnu`, cross-compiled with mingw-w64; GUI-subsystem programs with an icon,
  a version resource and a manifest; only system DLLs needed (`scripts/release/check_pe.py`).
- Output in `<out dir>/dist/linux/` and `<out dir>/dist/windows/`; `--dev-harness` adds a separate harness
  build for the checks (never shipped). Builds of the same commit are byte-identical.
- `scripts/release/package.sh` turns one commit into the two player archives and checks them
  (docs/developing/building-releases.md).

## Building on Windows

The supported way to get Windows binaries is the container's cross-build above; `scripts/checks/wine_smoke.sh`
runs the Windows harness build under Wine. A native build on Windows uses rustup with the MSVC toolchain and
the Visual Studio Build Tools, then `cargo build --release`; build.rs then compiles the resources with
`rc.exe`. The Windows builds have not yet been tested on real Windows hardware (docs/playing/platform-windows.md).
