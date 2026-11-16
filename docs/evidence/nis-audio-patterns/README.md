# CAR_AUDIO_PATTERN — native scripted updates (2026-11-04)

Vanilla SPEED2.EXE (sha256 8f6a9ae2…2a297), private Wine copy
`$OB_PRIVATE/wine/runs/<run>` (WidescreenFix off), fastgpu on display :192
with `--audio-file` (WAVs stay in the copy). See `docs/nis-audio-patterns.md`.

* `tick-hook.S` / `.bin`: observer code at 0x782C00 (zero .text padding);
  `record.py` redirects both `call 0x46FA30` sites (0x47C593 racer path,
  0x484CE4 player path) to it, the original call runs first, then one 32-byte
  record per update into a 64-entry ring at 0x8BB720 (count 0x8BB700; above
  the highest referenced BSS address 0x8BB5F4, verified zero): sim ticks,
  Physics, state/substep/segments, rpm, throttle, ctx pattern | +0xD8 |
  path, scene event clock (scene +0x130 via the list 0x827B88) and index.
  All patched bytes restored and verified after each run (`restored: true`,
  `lost: 0` in every retained log). The first run (`race1`) used an earlier
  hook on 0x47C593 only, without the clock.
* Captures (`*.json.zlib`: every record plus the world car list → grid slot):
  * `race1`: Quick Race, Peugeot 206, natural IntroNis08 (racers).
  * `race4`: Outer Ring, IntroNis27 (player on grid 4, racers 1–3).
  * `race6`: Resort Loop, IntroNis07, then an instrumented finish
    (`finish.py`: the player's laps and finish tick, as
    `../race-endings/capture.py`, no ending isolated) → player win →
    EndingNis02 behind Event Results.
  * `dyno1`: the finished-career save, Garage → Performance Tuning / Dyno → DynoEnter.
  * `dyno2`: Return to Explore beside the Body Shop → ShopEnter1 → Escape →
    ShopExit1.
* `model.py`: the 0x46FA30 bank model in f32; `check.py` re-checks all
  10 851 updates (bit-exact). `fixture.py` builds `native-updates.txt` (3 221
  rows: per scene the authored events and each context's event clock, rpm,
  throttle), replayed by `shop_scene::audio::tests` and the game tests.

Clone checks: `cargo test --release --lib shop_scene::audio audio::scripted`,
`cargo test --release --bin game shop_ race_start_scene_patterns
ending_pattern` (NFSU2_ROOT set).
