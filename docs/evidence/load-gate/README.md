# Load-speed gate: noise alone passes, a planted slowdown fails

Both runs: `scripts/checks/loadgate.sh run`, 10 runs per arm, mode (warm, cold, 4 cores) and scene (boot, explore),
the arms interleaved, on the reference desktop (discrete GPU) while other jobs kept the 16 threads at a load average
of about 8 to 40 (each repetition waited up to 2 minutes for a load under 12, then went on); runs under
`nice -n 10 ionice -c3`. Every cold run's log: `cold: dropped 2096 MB, resident after 0.0 MB`.

- `aa.md`: the same binary in both arms (an A/A run): `LOADGATE: PASS: no load slower beyond its noise limit`;
  no measure flagged in any mode.
- `planted.md`: the new arm sleeps 20 ms inside the track's cache read (one line added after the
  `World::load: from the cache` span opens, in `src/world/parse.rs`; never committed):
  `LOADGATE: FAIL: slower: warm: explore World::load: from the cache; cold: explore World::load: from the cache;
  cores: explore World::load: from the cache`, shifts +18.5 / +20.5 / +20.6 ms, p 5.4e-06 (the smallest a 10 + 10
  test can give). The load totals did not flag on this shared machine (their noise was 30–170 ms); the stage did.

The same two runs on a quieter machine (load average 5–12; `aa-quiet.md`, `planted-quiet.md`):

- A/A: `LOADGATE: PASS: no load slower beyond its noise limit`.
- Planted: `LOADGATE: FAIL: slower: warm: explore World::load: from the cache; cores: explore World::load: from the
  cache`; with less noise the warm load total flags too (`explore load +20.3 ms`), and the verdict names the innermost
  stage that carries the shift (the cache read, +18.2 ms, not its parents `World::load` and `drive_in`). Cold, the
  stage's shift (+19.9 ms, p 0.00053) stayed under the cold mode's stricter Bonferroni threshold: reported as
  "slower, not proven".
