# Expanded native boundary capture and suspension-load diagnostic

This continues `positive-dt-clone-boundaries.md`. It does not relocate production component passes or claim a full `CarState::substep` replay.

## What was missing, and what was captured

The first capture retained component objects but omitted the engine-record pointee used immediately by `5AA770`, the full drivetrain, mover history and source handling record. internal evidence audits the earlier capture's actual address coverage; its output is `audit.json`.

A fresh private native session on display 174 retained those dependencies. The expanded recorder keeps all original boundaries and adds full car (0x600-byte window), mover (0x200-byte window), drivetrain (0x100 bytes), live engine/gearbox records, four wheel records, and the car's source record. The windows describe bytes captured, not asserted C++ object sizes. There are 27 retained regions and 10816 used bytes in each 16384-byte record. Pointer checks verify the live owner and pointees at every boundary.

`expanded-landing` has 2530 records, 550 complete groups, 112 rest-triggered zero refreshes and 15 bump-triggered zero refreshes, with zero dropped records. The initial control is a +0.6 m position perturbation and -12 m/s vertical velocity expressed through actual native mass/momentum; the metadata records every write and throttle key transition. Compression, handling, clutch and wheel values were not injected. The source executable SHA256 is unchanged, all hooks restored, and the owned Wine/Xvfb session is stopped.

Reproduction uses the retained launcher with the owned prefix, followed by (the scripts are kept with the maintainer's private evidence):

```
python3 fastnav.py WINE_PREFIX race
python3 capture.py WINE_PREFIX /PRIVATE/OUTPUT/expanded-landing --seconds 2 --drop .6 --vertical-speed -12
python3 decode.py /PRIVATE/OUTPUT/expanded-landing
```

The capture pauses the game before disabling logging, then validates complete groups. Actual native dt and ticks are retained; reproduction is not promised bit-identical. Runtime navigation/capture logs are in the sibling `positive-dt-boundary-replay-runtime` directory. `test_decode.py` reproduces the summary and rejects a rehashed changed live engine pointer and a rehashed truncated group.

## A measured zero-time suspension effect

`5A87A0` preserves compression rate when dt is zero (`5A8877..5A8897`), but still recomputes spring/damper/preload/anti-roll load (`5A8AA0..5A8B22`). In the normal component traversal, each left suspension reads its right neighbour's previous compression. After all four suspensions update, the repeated zero-time traversal reads current neighbours on both sides.

At captured step 135:

| Suspension | Normal-post / zero-pre load | Zero-post load |
| --- | --- | --- |
| 0 | 38.5386962891 | 38.3348274231 |
| 1 | 23.4488868713 | 23.4488868713 |
| 2 | 43.5734596252 | 42.9173660278 |
| 3 | 43.8518676758 | 43.8518676758 |

Compression, rate and preload are unchanged between those zero boundaries. The load differences follow the neighbour term, rather than elapsed time or a gear transition. These are raw suspension loads at +0x24, **before** the tyre's camber multiplier; `WheelState.load` normally stores the later tyre load, so the two are not silently equated.

## Actual CarState component diagnostic

`WheelState::suspension_load` extracts the existing production arithmetic without changing operation order or pass scheduling. The ordinary tyre loop calls it before applying camber. The `native_zero_refresh_recomputes_suspension_loads` test constructs a CarState and seeds every wheel compression/rate read by that component from the native snapshots. Handling comes from the captured native car record; its two suspension records are byte-checked against the live suspension-record pointees. Preload comes from native suspension +0x88.

The test executes the shared production component calculation in native wheel order for both traversals, comparing all 60 wheel results from the 15 bump groups to native first-pass and zero-pass loads at absolute error below 1e-5. Its fixture is generated only from raw input/output snapshots (the scripts are kept with the maintainer's private evidence):

```
python3 fixture.py
cargo test --release --lib native_zero_refresh_recomputes_suspension_loads
```

This is an explicit boundary projection: pose, ground queries, aero, tyre camber/forces, engine/clutch, rest decisions and body integration are excluded because this component does not read them. Default values in the constructed CarState are not treated as captured native history. The diagnostic manually invokes the second calculation; production still lacks that second suspension traversal. Passing this test therefore proves the component calculation and the observed missing traversal's effect, not that production scheduling is corrected.

## Remaining boundary work

Native order remains normal component post → rest → full-compression impulses → optional zero component post → next-force input. Event 8 at 5B0520 is immediately before tail dispatch to 5B0530, not a render-frame marker. The clone measures suspension at its next substep start, performs its deferred bump block, then computes suspension loads/forces and integrates. Seeding a whole clone substep at native normal-pre would incorrectly start another integration from an already integrated native pose.

The capture's normal-post body has authoritative momentum and refreshed velocity. Between bump calls the momentum changes while cached velocity stays fixed; zero-post refreshes velocity. CarState has separate angular momentum/spin but stores only linear velocity. The bounded bump helper holds cached velocity while summing impulses, then applies the sum once. Native accumulates each impulse into existing momentum. That rounding/order distinction remains documented; no tolerance was widened and no complete state/precision equivalence is claimed.

For a full next-force replay, the missing ground-query context at car+4 (pack/section/recent floor history), aero-component frame setup, collision-pair history and exact native-to-clone phase projection still need mapping. The full-car snapshot retains pointers to some of these objects, not their pointees. A future capture should follow those specific pointers after tracing their layout. It should not replace absent state with CarState::new defaults, infer counter timing from rendered frames, or relocate passes on the strength of the bounded impulse/load diagnostics alone.

Validation: the targeted native-load diagnostic passed; the full library run passed 459 tests with 7 ignored. `validation.txt` retains commands/environment, and `SHA256.json` covers the raw capture, recorder, decoder and fixtures. The decoder negative controls and fixture byte-for-byte regeneration also passed.
