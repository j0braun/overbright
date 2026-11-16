# Suspension loads after the existing zero-time component post

This is a bounded production correction to the load consumed by the next force pass. **The full normal/rest/bump/zero traversal is still not implemented with native ordering.** Rest classification, deferred suspension measurement, complete zero-time component updates and full trajectory replay remain separate work.

## Consumer and refresh gate

The native component list places each wheel before its suspension. That does not make the tyre-force pass consume the earlier suspension load: after the complete normal/rest/bump/zero traversal, `5B0530` reads each suspension's +0x24 at `5B0590..5B05A9`, calls `59D170`, and stores the camber-adjusted value at wheel +0x164 (`59D228`). The tyre pass uses that field at `5A84EA` and `5A8513`. The retained disassembly records this dependency.

Across every one of the expanded capture's 550 groups, a zero-time traversal occurs exactly when the observed rest result is nonzero or the moving-car branch visits a fully compressed suspension. A visited bump requests the traversal even if its impulse is zero. Counts are 112 rest, 15 bump and 423 normal-only controls. The fixture records actual next-force-entry load from event 8, rather than predicting it from a final suspension formula.

The exporter verifies that compression, rate, preload and on-ground state are unchanged between normal-post and next-force-entry for all 2200 wheel cases. Thirteen left-wheel loads change because the zero traversal reads the right neighbour's newly updated compression.

## Minimal production plan

1. Keep current measurement, rest, engine/clutch, bump impulse and integration scheduling.
2. Retain the bump helper's visit flag, not whether it returned a positive impulse.
3. At the existing suspension-load calculation, choose the current neighbour compression if an existing zero-post site was requested: nonzero `CarState.rest`, or a visited bump. Otherwise preserve the normal pass's sequential neighbour state.
4. Leave rate/preload/contact and camber calculations in their current locations. The selector plus existing raw load calculation supplies the force input implied by repeating the native load calculation with unchanged compression/rate/preload.
5. Validate against all 550 captured groups through the shared production selector, with the old selector failing the native next-force loads first.

This folds the repeated load calculation into its final result; it does not pretend to execute all native components twice. The current `CarState.rest` is the class computed by the existing clone rest test; this patch does not prove that classification or its timing matches native in every situation. The selector follows the existing zero-post sites rather than inventing a new rest decision.

## Remaining replay prerequisites traced statically

* Ground query: `59D274..59D285` follows wheel +0x20 to the physics view (full-car +0x40), then reads view +0x4 (full-car +0x44), then `5E7E80` forwards to `5E72F0`. The context's +0x8 is the selected pack (`5E7311`); +0x2BC and +0x2C0 hold recent polygon count/list (`5E7362..5E7384`). Those polygon pointees carry plane/flags and were not retained by the expanded capture. The subsequent `evidence/full-phase-inputs` capture retains them and verifies their source-face pointers; that does not by itself establish full phase replay. A repeat query cannot generally be replaced by a fresh default `GroundQuery`.
* Aero: the physics mover's aero pointer is **+0x50**, as `5ABDDB..5ABDF9` writes throttle (+0x10) and frame wheels-down share (+0x14). `5991CA..5991E2` writes catch-up (+0x08) and draft factor (+0x0C). Earlier exploratory +0x24 suggestions were wrong for this mover; +0x24 belongs to a different layout. The aero pointee must be captured for an authoritative full force replay.
* Momentum/cache: native bump calls accumulate authoritative momentum while cached velocities stay fixed until zero-post. The clone helper preserves cached inputs, but sums deltas then adds them once and has no independently stored linear momentum field. Its floating-point sum order remains different.
* Native normal-pre is already after integration. A whole `CarState::substep` cannot be seeded there and treated as the corresponding native component update. A full refactor must preserve the phase of wheel histories and input/gear transitions, not just match one load result.

No FPS limit, timestep cap or tolerance change is part of this correction. Ground-query repetition, externally requested native reset paths and the remainder of the component traversal are not claimed equivalent.

## Validation

The old shared production selector failed at native step 135 wheel 0: 38.538696 versus next-force load 38.334827. The corrected selector passes all 550 groups / 2200 wheel cases with unchanged 1e-5 absolute error. The exporter also verifies the captured Handling and live suspension-record bytes for every group. Fixture regeneration is byte-identical.

The release library run passed 460 tests (7 ignored); track/contact passed 26, including held-car and zero-impulse bump refreshes; high-refresh driving passed 1 with the existing reverse case ignored on this base. Commands and environment are retained in internal evidence: suspension-load-phase/validation.txt. No full-substep equivalence follows from these projected component tests.
