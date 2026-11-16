# Street X keyboard test recovery

This changes only the test driver in `wrong_way_street_x_on_keys`. Production
physics, AI, contacts, scoring and race completion are unchanged.

The test drives every Street X track in both directions for two laps using
arrow keys, a ten-metre pursuit target, and a five-metre weave. It requires
completion within 300 simulated seconds, no false wrong-way flag while moving
along the road, and no incorrectly visible Wrong Way HUD text. All these
requirements remain unchanged, as do the rivals and weaving inputs.

## Established failure

A clean `03f3c60` baseline reproduces the track 4608 variant 0 timeout. This
predates and is independent of the Enter intro placement fix; the test never
sends Enter. With both native-backed cache consumers enabled, rival contacts
around 84.5–87 s turn the player away from its road target. It then reaches a
track wall at roughly 88 s. The bot keeps pressing forward throttle and steering
toward a target more than 100 degrees behind it. It has no reverse/recovery path.
The car remains upright, and 202 one-second samples are below 1 m/s. At the end,
it has 1.746 laps, with no false wrong-way or HUD-text events.

A same-binary diagnostic changed only the two cached-input selectors:

| Tyre speed | Drivetrain contact count | 4608 variant 0 |
|---|---|---|
|cached|cached|times out against wall|
|live|cached|finishes|
|cached|live|finishes|
|live|live|finishes|

Every run chose IntroNis12 for that case. These comparisons show sensitivity
to contact trajectories; they do not establish a defect in either native-backed
consumer or prove native collision parity. The actual AI-controller Street X
regression already passes; the failure is the simpler keyboard test driver
being unable to escape a contact. No production cache change was reverted.
Temporary selector hooks and per-second diagnostics were removed from source.
The retained comparison patch/log hashes permit review of that experiment.

## Test-only change and observed recovery

After one second below 1 m/s with a recent wall contact, the test bot holds the
normal reverse/brake key for two seconds and reverses its steering direction,
then resumes its existing pursuit. It never teleports, changes momentum,
injects lap progress or extends the race budget.

The affected run requests exactly one recovery at 89.271 s. At 90.271 s the actual
transmission is in reverse, the car moves at 3.51 m/s and has no contact; at
91.272 s it is still reversing at 5.70 m/s. It returns to first gear and completes
the two laps around 108 s. Every Street X track/direction finishes with zero
false wrong-way flags and zero incorrect text appearances. These timing and
contact results are clone diagnostics, not a claim of native collision parity.

Run with original assets and private XDG/TMP directories:

```
cargo test --release --bin game wrong_way_street_x_on_keys -- --nocapture
cargo test --release --bin game
```

Set NFSU2_ROOT explicitly: these existing asset-backed tests return early when
it is absent. `docs/evidence/streetx-bot-recovery` retains diagnostic excerpts,
source/log hashes and final validation counts. No GPU or native Wine session
was used for this test-driver correction.

Final asset-backed game test suite: 18 passed, 0 failed, 9 existing ignored.
The original 300-second budget, two-lap completion, and wrong-way/text
assertions are unchanged. `git diff --check` passes.
