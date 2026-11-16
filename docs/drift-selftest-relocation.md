# Drift self-test relocation premise

The integrated `--selftest` failed at its final drift ranking assertion:
player NAMERay scored 21610, ahead of AI scores 11807/9670/8736. Sorting the
highest score first was correct; the test assumed its artificially relocated
player would have fewer points without enforcing a non-drifting trajectory.

## Controlled comparison

A temporary diagnostic logged player score, pose,
velocity, spin, momentum and wheel state before updates and each relocation.
The same diagnostic executable was run with native camber and with only the
old unit-axis camber shortcut selected. No other source changed between those
runs. The temporary selector and trace code are **not** in the final patch.

| Observation | Native double-cross camber | Old shortcut |
|---|---:|---:|
| Before first relocation, t=90.0323 s | score 0, velocity 0, slip 0 | score 0, velocity 0, slip 0 |
| After third relocation, vertical velocity | 10.674041 m/s | 10.6740265 m/s |
| First observed positive points-list score | 5462 after 33 relocations | 56 after 27 relocations |
| At that observation, speed / slip | 41.62645 m/s / 23.57791° | 8.01327 m/s / −59.8855° |
| Final player score | 21610 | 56 |
| Full self-test | fails expected low-score rank | passes |

At native-camber relocation 39 the player's speed reached 76.19703 m/s.
The test moved XY by 50 m every 0.25 s, retaining old height, orientation, velocity,
angular momentum and wheel/contact histories. Even the previously passing
formula produced an airborne, rotating car and unintended drift points.
The corrected formula changes the subsequent unstable trajectory; the old
low-score outcome was not a sound precondition for a ranking test.

Production `Race::drift_frame` reads actual velocity, slip and yaw.
`DriftManager::angle_points` considers speed and slip (>5°), among its
other gates. The diagnostic supplies physically moving/slipping input, so
this failure does not demonstrate incorrect score ordering. Nor does this
investigation establish general native force parity or normal-driving
behavior under every condition.

## Test-only correction

The scripted moves now obtain road height and route heading and call the
existing `Drive::place`. This reconstructs the grounded pose/state while
preserving barrier groups, transmission mode, hold and stability settings.
The test asserts zero velocity, spin and angular momentum immediately after
placement, and **zero player drift points** in the results. The original
relative-ranking assertion and rival-scoring check remain intact. No
production scoring, physics, camber formula or tolerances are changed.

## Retained diagnostic evidence

Private workspace directory:
an internal run folder.

SHA256 values:

- `native-camber-trace.log`: `f4fef8057180266d5ffbcf044c193ca577394633853812b9459e63f19b33cb64`
- `old-camber-trace.log`: `bc3e6e84eb7c5552ab9f10c73dd1e45d1eb1d8cbc4e781fae3141da69a892a80`
- `trace-diagnostic.patch`: `60149b16553dc2156ed2c4bf4bba4e86e748d3bcff5dcbf465585ec6be4f6755`
- `game-trace-binary`: `3e2b51e23df1adb760fcf911bd28b8b54a93aadf1ea069f6b47ab7a47d559e16`

The retained private `run.sh` isolates XDG/TMP/cache and supplies the read-only
game-assets path. Diagnostic commands were `NFSU2_DIAG_DRIFT=1 game --selftest`
and the same with `NFSU2_DIAG_OLD_CAMBER=1`. Those variables only operate in
the retained diagnostic executable, not the final game.

## Final validation

`cargo build --release --bin game -j 2` and the complete `game --selftest`
passed with the final patch and native camber unchanged. Drift results were
Brian 11807, Kristian 9670, Eddie 8736, NAMERay 0. The AI scores are identical
to the failing native-camber diagnostic; only the test player's relocation
changed. All later self-test checks completed (`cash $6650, reputation 0,
stage 1, wins 3`). `git diff --check` passed.

- `fixed-selftest.log` SHA256: `739b4404d9c2c66982970db9f1d6bccf5c41564b0948a6516e212d9c6d497d34`
- Validated final game SHA256: `042932b98689470e664a245fb5d2fafefffb1f7ef196f671d536d655d5c5e796`

There are no remaining diagnostic selectors or trace hooks in source.
