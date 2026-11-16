# Instrumented race-ending evidence

Executable SHA-256: `8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.
Capture SHA-256 (`native-finish-rwd.json`): `60a3c2a9e814981d13767eb6b455dc033f4ca9db67d65ef81932344bb6bbe49e`.
Captured 2026-11-02 under private Wine/wined3d at :191.

240SX, Quick Race Resort Loop. Only the racer laps/most-laps and finish-line
tick are written to invoke the native finish handler; other ending entries'
flags are temporarily cleared to isolate EndingNis01. Native drivetrain,
human-winner, and memory gates are unchanged. This proves the isolated
native transition and wheel controls, not a naturally driven win or random
variant frequency. See ../../race-ending-selection.md for trace and limits.

`capture.py` and `probe.py` reproduce the relevant memory capture using an
already running private fastgpu session. Do not use a user/shared prefix.
The supplied JSON is the original successful capture, not a rerun.
