# Bump-stop velocity cache correction

The native boundary capture and formula replay in `positive-dt-clone-boundaries.md` identified a later-wheel impulse error. `CarState::substep` now uses the shared `bump_stop_impulses` helper: all fully compressed wheels read the same cached linear/angular velocity; their impulses accumulate before velocity refresh. The first hard-landing group's second wheel previously used the first wheel's already-updated linear velocity.

The helper retains each impulse's 1000 linear and 10 angular cap, wheel iteration order, the existing rest gate, and the refresh when a full-compression visit returns zero impulse. Component pass scheduling is unchanged. In particular, the clone still schedules this block at the following substep's start; this patch does not resolve the larger boundary-equivalence question.

## Native regression

`bump_stop_loop_matches_native_hard_landing` exercises the actual production helper across all 17 recorded bump groups / 39 calls. Its CSV contains native pre/post linear and angular momenta, actual cached velocities/spin/mass/normals, and hubs transformed by the captured body matrix. Generate it with (the scripts are kept with the maintainer's private evidence):

```
python3 replay_bumps.py --fixture
```

Expected group momentum changes are taken directly from the native raw snapshots. The test checks both linear and angular changes with absolute vector error below 1e-5; that threshold was not widened. A second regression checks separating/zero-impulse visits, absent visits, and per-wheel caps. The initial two-wheel native regression failed with the extracted old sequential loop: linear impulse Z was 3.1053867 instead of 3.4981685.

This is a bounded impulse-loop regression, not a full trajectory replay. The clone stores linear velocity rather than the original body's separate authoritative momentum and cached-velocity fields. The helper sums impulse deltas, then applies each total once, while native adds each impulse to the existing momentum. Consequently f32 accumulation order is not bit-identical. The retained native group regression bounds this difference for the recorded landing; it is not an all-state precision proof. No full burnout or rest/component parity claim follows.

Validation: `cargo test --release --lib --test tuning_physics --test high_refresh -j2` with the original asset root and isolated target/XDG paths passed: 458 library tests (7 ignored), 3 tuning tests, and 1 high-refresh test (1 ignored). Library coverage includes the contact regression and both new bump tests. The base binary still marks reverse high-refresh coverage ignored; this patch neither changes that status nor claims to validate later parent changes. Native decoder audit and 39-call Python replay also pass, and the CSV regenerates byte-identically. Logs are retained in the sibling `positive-dt-native-capture-runtime` directory: `bump-red.log`, `bump-final-tests.log`. No Wine or GPU run was needed for this offline patch.
