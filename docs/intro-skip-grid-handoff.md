# Race intro skip places every car at the authored cut

Pressing Enter during a moving race intro advanced its clock to the cut but
left its cars at their previous scene positions. The next update passed the
`run.frame < cut` placement guard, so the held countdown retained those old
positions. This reproduces the reported AI cars starting behind the line.

The fix invokes the existing slot-based `scene_cars` placement when seeking,
before changing the clock. Both sample times are the cut: this is a seek,
not simulated travel over the skipped interval. It therefore generates no
artificial velocity/spin and retains `set_scripted_spin`'s angular-momentum
consistency. Existing wheel phases survive; progress is placed on the road.
The controller route approximation is preserved.

## Native basis

The verified executable has SHA256
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.
Unmodified bytes for the relevant ranges are disassembled in the evidence
folder. `43B460` seeks to StartCountdown through `43AF80`; that function
resets car binding/first-placement fields, calls `439240` at `43AFDF`, then
stores the new clock. `439240` evaluates each bound car channel. The reset
first-placement flag causes `430A75..430A81` to call `5FBD70` at the requested
pose. Merely changing the clock omits this placement.

The existing original-Wine observation and trace are recorded in
`formats.md`, “Race start scene cars” (grid145/finish151). This task rechecked
the executable bytes and the clone integration; it did not make a fresh native
runtime capture. Full NISMover trajectories and native teleport/controller
reset lifecycle remain open. The fix retains existing ground-derived Z and
model-origin/centre-of-mass conversion; it does not claim complete scene parity.

## Paired reproduction

The asset-backed game integration uses the first career drag event,
STAGE_1_DRAG_1, track 4201 variant 2, private stock PEUGOT profiles, and the
normal `Game::key` path. For each scene/rate, event and seed are identical
between full intro and Enter at 0/10/50/95 percent of the pre-cut duration.

The initial moving IntroNis03 reproduction at 60 Hz, skipped at 10 percent,
left AI slots 2/3/1 behind the complete-intro cut poses by 9.537/18.211/13.397 m;
at green the gaps were 9.167/17.048/10.155 m. IntroNis25 was a stationary control
and did not show that large displacement. Red and green log hashes are retained.

The final regression selects moving IntroNis03 naturally with seed 4 and
stationary IntroNis25 with seed 0, asserting both selections. It runs at 60 and
240 Hz with four skip times. It checks immediate authored target XY for the
player and every rival, zero seek motion/angular momentum, preserved route
node/driver/control/RNG state, and matched AI cut/green poses and progress.
Authored channels specify centre-of-mass positions: raw model-origin offsets
of roughly 0.19–0.245 m are not target-placement errors. The regression converts
the state back to its physical centre before checking the authored target.

Run in private XDG directories with NFSU2_ROOT set to the original installation
and NFSU2_INTRO_NIS unset:

```
cargo test --release --bin game intro_skip_preserves_ai_grid_handoff -- --ignored --nocapture
```

This is a CPU integration test using real assets; no rendered/native visual
match is claimed. Other intro assets and the exact user save are not covered.

## Final validation

The explicit paired regression passed all 20 runs: 64 immediate player/AI
centre targets and 48 cut/green/progress/target comparisons had zero error.
No tolerance was relaxed. Route/controller input state remained unchanged on
the key, and all sampled racers remained upright through green.

The broader game-test binary had 17 passes, 10 ignored and one failure:
`wrong_way_street_x_on_keys` did not finish track 4608 variant 0 within its bot
budget, with zero wrong-way/text errors. A fresh isolated unmodified 03f3c60
worktree reproduces exactly that failure. This test never sends Enter; the
failure predates this patch. It remains open and was not weakened or relabelled
as passing. The integration branch must still run its normal package checks.
