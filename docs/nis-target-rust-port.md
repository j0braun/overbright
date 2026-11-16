# Native scene target arithmetic port

The specification is original0x5A44B0 and its64 retained native input/output
snapshots, documented in `nis-mover-step-investigation.md`. This step ports that
helper into Rust without changing the live scene loop. The existing scene loop
still needs native component ordering, contact state and mover lifecycle; using
this helper alone cannot establish or replace that work.

`drive::nis::TargetBody` retains separate momentum and cached velocity/spin,
plus the base and selected inverse inertia. Those distinctions are required by
the original helper and cannot be represented by treating CarState.velocity as
both values. Target application has two native branches: direct velocity/yaw
assignment with base-tensor momentum reconstruction, and capped impulse/moment
application that deliberately leaves the velocity caches unchanged. Both retain
position, orientation and vertical cached velocity. The direct setter rebuilds
all linear momentum from cached velocity times mass; it can change vertical
momentum if that cache was stale. The impulse path retains vertical momentum. The native
angle16 conversion is reused; no fixed frame interval or render cap is added.

Verification sequence:

1. A no-op implementation must fail against the original snapshots.
2. Implement both branches with original constants and float storage boundaries.
3. Compare all64 cases, all affected momentum/cache components and preserved
   pose, using the existing original-oracle arithmetic bound (2e-6 relative,
   2e-5 absolute). Do not loosen it to accommodate differences.
4. Run the library regression suite and retain exact results.

The fixture covers identity/90-degree orientation, both selected inertia states,
two positive frame intervals, wraparound headings, nonzero vertical motion/spin
and both impulse caps. It is not broad coverage of arbitrary tilted tensors or
near-boundary angle-table rounding. No fullscene or all-input bit-exact claim
follows from passing these cases. Runtime wiring stays open until the full
mode-5 frame path is established.

Validation: the no-op failed on native case0 (cached x velocity1 versus
1.2500285). Both branches then passed all64 original snapshots without changing
the tolerance. A subsequent review preserved native dx extended-precision
multiplication separately from stored dy, and the final complete library run
passed468 tests with7 ignored. Logs are retained in the private workspace's
review/nis-target-rust-{red,green,final}.log. The first attempted test compile
had an incorrect relative fixture path; it was corrected before the genuine
no-op failure was recorded. No game binary/package behavior changes follow from
this unwired helper.
