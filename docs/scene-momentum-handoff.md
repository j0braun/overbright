# Scene momentum handoff correction

The drag diagnostic on track4201, seed20261103, reproduces all three rivals rolling before green at60 and240Hz. At60Hz frame358, rival0 has zero scripted spin but momentum(-4.262683,-6.339113,-0.04888056). At frame359, after camera cut stops placement, physics derives spin(-2.9257383,-4.2093377,-0.018266737), without a contact impulse. Scene placement reset pose/spin but retained accumulated physics momentum.

`CarState::set_scripted_spin` sets momentum using the same rotated inertia tensor inverted by `spin_from_momentum`. Scene placement uses it after setting orientation. Scripted yaw is preserved; this does not zero arbitrary driving momentum, change forces or cap FPS.

Root validation: countdown rollover assertion passes at60 and240Hz, with three rivals and green reached. Unit test verifies rotated anisotropic inertia roundtrip in both ground/air modes and stationary momentum reset. Existing IntroNis13 wheel phase regression passes. Logs in workspace review:drag-rollover-momentum-60hz.log,drag-rollover-regression-final.log,drag-rollover-final-240hz.log,scene-momentum-unit.log,rollover-wheel-regression.log.

Scope: corrected internal state consistency in the existing scripted-placement approximation. Full native NISMover trajectories remain open; this does not prove every drag collision or track matches native. Player stays on grid during reproduction.

## Expanded countdown regression

The selectable diagnostic now accepts ROLLOVER_TRACK from its enumerated drag-mode list. All10 tracks(4201,4401,4402,4212,4213,4221,4222,4223,4404,4211) passed at60 and240Hz:20 asset-backed runs with three opponents, green reached, and no pregreen rollover. Seed20261103, player stationary. Track4201 also passed500Hz. Reproducer and per-run logs are review/drag-track-sweep.py and review/drag-track-sweep.json in the private workspace. This expands countdown coverage, not native race trajectory or every collision coverage.
