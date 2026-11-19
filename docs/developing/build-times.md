# Build times

How long the game takes to build today, and how much a development build profile saves. This is the
baseline for splitting the code into several crates: later measurements are compared against it.

## How it was measured

- One machine: 16 cores, 61 GB RAM, NVMe disk, Linux, rustc 1.98.0. No other builds running, no compile cache
  (`RUSTC_WRAPPER` empty), `nice`, all cores (`CARGO_BUILD_JOBS` unset). One long-running single-threaded
  process was busy on one core throughout.
- Each number is the **median of 3 runs**, wall time of the whole `cargo` command.
- **Cold:** the target directory removed before each run (dependencies included; the crate cache was already
  downloaded).
- **Incremental:** the target set built once, then one source file changed (an added exported function whose
  value changes each run, so its code is regenerated and the binaries are relinked) and the same command run
  again. One file per area:

  | area | file |
  |---|---|
  | formats | `crates/ob-formats/src/formats/bundle.rs` |
  | render | `src/render/atmosphere.rs` |
  | sim / AI | `src/ai/drag.rs` |
  | frontend | `src/frontend/cheats.rs` |
  | launcher | `src/launcher/font.rs` |
  | editor | `crates/ob-editor/src/editor/camera.rs` |
  | game binary | `src/bin/game/state.rs` |

- Target sets:
  - **loop:** the one binary you run after that edit (`--bin game`; `--bin overbright` for the launcher,
    `--bin overbright-editor` for the editor), with `--features dev-harness`;
  - **checks:** what `scripts/checks/checks.sh` builds (`--bins --examples --features dev-harness`);
  - **test:** `cargo test --features dev-harness --no-run` (the test binaries the checks run);
  - **check:** `cargo check --features dev-harness --bins` (dev profile, no code generation).

## Profiles compared

| name | settings |
|---|---|
| ship | `[profile.release]` as shipped: thin LTO, 1 codegen unit, not incremental |
| gate | `[profile.gate]`: inherits release (opt-level 3, same code paths), LTO off (`lto = false`, i.e. crate-local only), 16 codegen units, incremental |
| gate + mold | gate, linked with mold (`-C link-arg=-fuse-ld=mold`; a machine setting, not in the repo) |

Rust does no fast-math, so the gate profile computes the same results as the shipped one. Only speed and
size differ, and perf numbers must still come from the shipped profile.

## Cold builds (median of 3, seconds)

| build | ship | gate | gate + mold |
|---|---|---|---|
| checks set (`--bins --examples --features dev-harness`) | **256** (256, 269, 256) | **95** (95, 96, 94) | **95** (95, 94, 96) |
| release build (`cargo build --release --bins`, the shipped binaries) | **217** (225, 217, 198) | – | – |

Where the cold time goes (`cargo --timings`, checks set):

| unit | ship | gate |
|---|---|---|
| dependencies (before the library starts) | 23 s | 34 s |
| `nfsu2` library | 93 s | 40 s |
| `game` binary (starts after the library) | 117 s | 19 s |

In the shipped profile the `game` binary alone takes longer than the library: with thin LTO and one codegen
unit, it re-optimises the library's code at link time. The other binaries (editor 82 s, launcher 69 s, the
examples 40–67 s) do the same in parallel. Under the gate profile the binaries link in seconds.

## Incremental builds after a one-file change (median of 3, seconds)

| area changed | loop, gate | loop, gate + mold | checks set, gate | test build, gate | `cargo check` |
|---|---|---|---|---|---|
| formats | 20.0 | 20.5 | 23.7 | 39.2 | 6.2 |
| render | 19.3 | 19.3 | 22.9 | to measure | to measure |
| sim / AI | 19.4 | 20.6 | to measure | to measure | to measure |
| frontend | 20.2 | 19.5 | to measure | to measure | 6.0 |
| launcher (`--bin overbright`) | 16.8 | 16.5 | to measure | to measure | to measure |
| editor (`--bin overbright-editor`) | 16.8 | 16.6 | to measure | to measure | to measure |
| game binary (`src/bin/game`) | 17.2 | 16.5 | 18.6 | 37.9 | 5.6 |

- Every area of the library costs the same, about 19–20 s for the game: everything is one crate, so any edit
  recompiles the library (incrementally) and relinks every binary that uses it. The launcher and the editor
  are not faster to iterate on than render code, even though they are small.
- **Ship-profile incremental: to measure in a quiet window.** It is not incremental and has one codegen unit,
  so by the cold timings above a library edit costs at least the library plus the `game` binary,
  ≈ 93 + 117 s ≈ 3.5 min (an estimate from the cold `--timings`, not measured).
- mold makes no measurable difference, cold or incremental: linking is not the bottleneck.

## Binary sizes (shipped profile, `cargo build --release --bins`)

| binary | as built | stripped |
|---|---|---|
| `game` | 31.1 MB | 26.2 MB |
| `overbright` (launcher) | 10.5 MB | 8.5 MB |
| `overbright-editor` | 21.4 MB | 17.7 MB |

The gate profile's binaries are larger (for example `game` with the developer harness ≈ 39.7 MB as built),
because there is no cross-crate LTO. They are never shipped.

## The gate profile

`Cargo.toml` defines `[profile.gate]` (the settings in the table above). It is for development loops and
the pre-merge checks only:

```bash
cargo build --profile gate --features dev-harness --bin game        # → target/gate/game
cargo test  --profile gate --features dev-harness
```

How the checks use it (docs/developing/checks.md, "Build profile"):

- `scripts/checks/checks.sh` builds and tests with `--profile "$CHECKS_PROFILE"` (default `gate`) and runs
  the binaries from `$CARGO_TARGET_DIR/gate/`; `CHECKS_PROFILE=release` switches back to the shipped
  profile and `release/`. The scripts it calls with a binary get that path; their own defaults follow the
  same variable.
- `scripts/checks/gate.sh` runs its checks step with the gate profile. Its parity builds both the base and
  the candidate with `perf.sh build` and the shipped profile, so the parity compares like with like.
  `perfgate.sh`, `release_check.sh`, the fresh-start build, `pgo.sh` and the release container keep
  `--release`, so every perf number and every shipped binary still comes from the shipped profile.
- The fast-build environment some machines set (`CARGO_PROFILE_RELEASE_*` variables) gives `--release`
  builds the values the gate profile sets itself; the named profile has its own target folder, so its
  binaries never overwrite the `release/` ones.

## Still to measure (in a quiet window)

These were planned for the same round but not run, because the machine was needed for other work. Any timing
taken while other builds run is skewed.

- The empty cells above: the checks and test sets for every area, and `cargo check` for every area.
- Ship-profile incremental builds (loop, checks set) after a formats, launcher and game-binary edit.
- The launcher as its own crate, on a throw-away copy: the launcher modules moved into a separate crate on
  top of the library, with the game↔launcher contract (folders, probe, start errors, theme tokens) left in
  the library. The question is whether a launcher edit then rebuilds only the small crate, and whether
  library edits get slower or faster. The copy is prepared but not built.
- The later rounds of the crate split: after the first split tier, and after the core and formats crates.

The method above is enough to repeat every number: same commands, same files, three runs each, medians.
