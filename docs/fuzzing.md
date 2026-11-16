# Fuzzing the file readers

Every parser that takes bytes a player (or a mod, or a download) controls has
a fuzz target in `fuzz/targets.rs`: one plain function `fn name(&[u8])`. A
target must return for every input: no panic, no hang, no allocation above
`Limits::max_alloc` (256 MiB; the readers' checked `Reader` and `Limits`,
docs/io-robustness.md). Behaviour on valid files must not change; the gate
proves it byte-identical.

Two drivers run the same functions:

* **`tests/fuzz_smoke.rs`** (stable, in `cargo test`): a deterministic
  mutation engine (xorshift; bit flips, bytes set to 0 / 0xFF / 0x80, 16- and
  32-bit fields set to boundary values, truncation, block duplication,
  length-field inflation, lying chunk-header sizes) over seeds taken from the
  player's game files at test time. Two tests: `fuzz_smoke_builtin_seeds`
  (always) and `fuzz_smoke_game_seeds` (needs `NFSU2_ROOT`; ignored with the
  reason otherwise). Default budget 1 s per target (about 45 s in all);
  `scripts/checks/fuzz.sh` (the `fuzz` step of `checks.sh full`) gives 2 s.
* **`fuzz/`** (cargo-fuzz, libFuzzer, needs a nightly toolchain): one binary
  per target, `fuzz/fuzz_targets/<name>.rs`.

## Running long

```
# stable, the mutation engine, minutes per target
FUZZ_SECS=300 FUZZ_ITERS=100000000 FUZZ_ONLY=roads,schl nice cargo test --release --test fuzz_smoke fuzz_smoke_game -- --nocapture
# nightly, libFuzzer (seed the corpus from your own files, see below)
cd fuzz && cargo +nightly fuzz run roads corpus/roads -- -max_total_time=600 -rss_limit_mb=1024
```

Environment of the smoke test: `FUZZ_SECS` (per target), `FUZZ_ITERS`,
`FUZZ_SEED`, `FUZZ_ONLY=a,b`, `FUZZ_CALL_MS` (slow-call bound, 3000),
`FUZZ_DUMP=dir` (writes failing inputs), `FUZZ_REPLAY=n` (skips running the
first n inputs of a target: the failure line names the input number; the
sequence is deterministic per seed and target). A single request above 1 GiB
aborts the run after printing `FUZZ-ALLOC target=… input=…`; a call over 30 s
aborts with `FUZZ-HANG`.

## Adding a target

1. Write `pub fn my_parser(b: &[u8])` in `fuzz/targets.rs`; call every public
   entry point of the parser and use the results (`black_box`). A chunk-level
   parser starts with `chunk(b)` (byte 0 = the chunk's file offset mod 128).
2. Register it in `TARGETS` with `t!(my_parser, Seed::…)` (where its seeds come
   from: chunks of a bundle, a whole file, archive entries with a magic, text).
3. Add `fuzz/fuzz_targets/my_parser.rs` (copy another; it calls
   `targets::my_parser(data)`) and a `[[bin]]` entry in `fuzz/Cargo.toml`.
   `every_target_is_registered_once` checks the files exist.

## Corpus policy

Nothing from the game is committed. Seeds are read from the installed game at
test time (`NFSU2_ROOT`); a libFuzzer corpus is made by the player from the
same files and stays outside the repository (`fuzz/corpus`, `fuzz/artifacts`
are git-ignored). A regression test for a crash stores either a few header
bytes it needs (synthetic), or the recipe (target, seed, input number from the
smoke test: `FUZZ_ONLY=<target> FUZZ_REPLAY=<n>`), or builds the input from a
seed file at test time (`tests/fuzz_regressions.rs`).

## Recipes of finds whose input is game data

* `globalb`, panic in `handling::acceleration_curve` (a gearbox with fewer
  than three ratios): `FUZZ_SEED=1234567 FUZZ_ONLY=globalb FUZZ_REPLAY=56295`
  (seed 9 = the GLOBALB.BUN file). Fixed: the gear loop is bounded by the
  ratio and shift tables.
