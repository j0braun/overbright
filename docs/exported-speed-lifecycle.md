# Explicit exported forward-speed lifecycle

Ordinary physics now consumes the previous exported forward-speed scalar
throughout its substeps and contact re-steps, then publishes at mover exit.
Previously every `simulate` sampled current velocity, so a contact impulse
changed the tyre input before the native export boundary. In a real two-car
world regression the old re-step consumed−1.4191252 instead of the preceding
13.964559 export. Both that case and the post-motion callback regression fail
before this change and pass afterward.

Native evidence and its exact scope are in
[evidence/tyre-export-lifetime/README.md](evidence/tyre-export-lifetime/README.md).
Two untouched live captures establish3514 mover/export triples,276 contact
sequences, and one natural R/actionA1 human road placement. The new compact
fixtures copy3514 native exported vectors/scalars and one placement argument,
axis, velocity and scalar directly from those captures. Production arithmetic
matches all these raw values bit-for-bit, with no tolerance changes.

## Production boundaries and APIs

`CarState::publish_forward_speed` uses native PC24 `(vz*az + vy*ay) + vx*ax`.
Ordinary `simulate` calls it after its substeps and before returning to the
post-motion recovery callback; contact re-steps also publish when they finish.
Rewind, contact impulses, suspension/component refresh and bump/rest processing
do not republish. The force-loop consumer therefore retains the prior export.
The test observer's “entry” is immediately before that force loop, after frame
setup; it is **not** the native recorder's earlier mover-entry hook.

`set_exported_forward_speed` handles explicit scalar writers, and
`set_placement_speed` separately writes the argument and installs axis×(argument
+0.1). The native reset capture observes scalar20.959133→0→8→8, velocityaxis×8.1,
then the next mover retaining8 until its own export. Parameter8 versus velocity8.1
is intentional. Existing recovery route/pose choice and live `speed()` readers
are unchanged; this does not port the entire native teleport implementation.
The public getter exposes the retained scalar for writer/consumer regressions.

`speed()` still projects current velocity. `aero_down` and its current sampling
lifetime are unchanged. No cache-valid flag, lazy fallback, timing cap, control
threshold or test timeout was introduced. Ordinary race hold follows normal
physics export and never forcibly zeros a moving scalar.

## Direct writer audit

- **Constructors / generic `Drive::place`:** stationary state has scalar0.
  Nonzero callers must explicitly publish their intended initial condition.
- **Human, route-trigger, rival and encounter recovery:** use the native
  placement argument/velocity split. Their preexisting pose/controller resets
  and recovery parameter computation remain separate approximations.
- **Traffic placement:** uses lane speed as scalar and lane speed+.1 as velocity;
  the mover still initializes from the latter. No double+.1 is added.
- **Encounter spawn:** already installed lane-direction×(lane speed+.1); now
  also publishes the original lane argument. Existing route-direction versus
  quaternion-axis differences remain explicit.
- **Traffic / belt motion (`place_on_road`):** publishes the mover speed, which
  can differ from the translated pose's velocity projection. Ground/pose and
  belt movement retain existing approximations. The belt rebuild failure stops
  velocity and explicitly publishes that synthetic0. `Input::Belt` in physics
  is a genuine no-op and does not invent another publication.
- **Melo:** publishes at its existing5A2B70 write-back. `leave_melo` transfers
  drivetrain state and retains the just-published scalar for ordinary physics.
- **Intro, shop and ending poses:** converge through `scene_wheels`, which now
  publishes after their existing channel-derived motion. This is explicit
  integration with the existing NIS approximation, not full NISMover parity.
  Enter cut/cut seek still yields zero velocity/spin and publishes zero.
- **Dyno:** the deliberate post-settle reset clears the scalar (56D3D2).
  `Game::dyno_pose` only tilts the display proxy from the separately simulated
  `DynoPull`; it does not inject a new driving velocity. Exit scene motion
  passes through the scene publisher before ordinary driving resumes.
- **Synthetic motion:** camera/shot probes, benchmark/playthrough staged
  encounters, tests that deliberately install moving cars, track collision
  fixtures and the weight probe now explicitly seed a projection. They remain
  synthetic initial conditions, not native placement (+.1 is not added).
- **Older hit replay:** its log has no retained scalar; it explicitly seeds
  the reconstructed runtime projection. This remains an approximation and
  cannot serve as a native contact-entry cache oracle.
- **Retained native force fixtures:** supplied native scalars remain supplied;
  they are not overwritten by a runtime projection. Read-only AI driver tests
  that never run physics continue to read live speed and need no migration.
  Particle velocities and independent NIS primitive bodies are not CarState
  exports and were not modified.

Early-return audit: ordinary `simulate` has no rest or dt early return; only
Belt (no-op) and Melo (separate writer) return early. Native5B73C3 can skip its
integration loop but still reaches5B74E1 export. The scalar publication location
follows that static writer; the positive-dt captures do not establish zero-dt
component/clutch equivalence. Invalid dt behavior is outside this change.

## Validation

Private runtime: an internal run folder supplies
NFSU2_ROOT pointing to read-only verified assets, private XDG/TMP/cache, owned
`positive-dt-force-target` and2 build jobs. Source baselineb9935e1 includes the
cached-count, Enter fix, particle/HUD and StreetX test-driver work. Evidence
commits were applied separately before implementation.

```
cargo test --lib --test track_l4ra --test high_refresh --test tuning_physics --bin game -- --nocapture
cargo test --bin game intro_skip_preserves_ai_grid_handoff -- --ignored --nocapture
NFSU2_INTRO_NIS=13 cargo test --bin game race_start_wheel_phase_survives_scene_placement -- --ignored --nocapture
cargo run --bin game -- --selftest
cargo check -p devprobes --bin hit_replay -p devprobes --bin weight_probe
```

Results: library511 passed/12 existing ignored; game19 passed/12 ignored;
track26 passed; high-refresh1 passed/1 existing ignored; tuning3 passed.
Both explicit ignored scene tests pass. Enter's movingIntro03 and stationary
Intro25 paired runs at60/240Hz retain zero target/cut/green/progress gaps.
Full selftest passes (cash6650, stage1, wins3). Native cached-count drivetrain,
landing tyre-force, nonunit-axis camber, hub and suspension regressions remain
passing. Landing force maximum error stays7.6293945e−6 under its unchanged1e−4
bound. Six new library lifecycle tests cover actual mover callback, actual
contact re-step, actual placement consumption, explicit scripted/traffic input,
actual Melo handoff, and all raw projection/placement fixture rows. A real game
traffic-pose test distinguishes mover parameter from translated velocity;
existing recovery and scene integration tests now assert scalar publication.

The broader preserved force replay's128 supported /181 missing-brake cases are
not full-world validation and were not transplanted into this commit. This
change does not close native raw-matrix versus unit-quaternion fidelity,
complete force/component ordering, aero/count publication lifetime, or all
NIS/traffic/placement behavior. Positive-dt scalar lifetime is the verified slice.
