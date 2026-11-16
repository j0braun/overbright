# Player nitro coverage investigation

Baseline40db6b5; production effects are unchanged. User's car/situation remains unknown. The retained test-profile log has an empty career-car field and cannot identify the driven vehicle.

## Marker/configuration coverage

`nitro_marker_coverage` inventories29 nontraffic car types using original GLOBALB/geometry, `PartDb` lookup, and production `shown_slots`. It covers stock, each offered part in slots1/2/5/6/11/16 separately, plus every widebody/exhaust pair. These are4,064 explicit fit maps (some are visually equivalent to stock), not all possible combinations or proof every part is buyable at a given career stage. All have one or more exhaust markers and finite matrices. It mirrors the loader's marker walk; it is an asset/configuration audit, not a native live binding oracle.

`inventory-summary.json` hashes GLOBALB and all29 geometry files. `marker-inventory.jsonl` retains exact part hashes and transforms. The original verified assets are read-only.

## Loaded-car particle stress

`nitro_frame_coverage` uses actual `Car::load_with`, original L4RA emitter descriptions, and stock/last-widebody+last-exhaust configurations on Corolla, Hummer, Lancer, Mustang and RX7 (9 actual fits; Hummer has no nonstock widebody). It supplies an active-shot burst explicitly; there is no real tank/key path in this probe. Emitter seed123 is synthetic and retained for reproducibility. Each fit runs20,000 updates per cadence:60Hz,10Hz, and a240/30/10/10Hz repeating transition.

All60Hz cases have particles and quads throughout. Single-exhaust cases have2 fully empty updates and13 no-quad updates per20,000 at10Hz; transition cases have1 empty and5 no-quad updates. The dual-exhaust Mustang cases have none. `invisible` in the log means **no positive-alpha CPU quad**, not an actual rendered pixel measurement. This demonstrates occasional frame gaps, not the player's persistent absence.

## Native arithmetic boundary

Retained original instructions support short-lifetime disappearance being possible.492161..168 computes life(+3C)*spread(+40);492195..1A0 subtracts that spread from base;492951..96C adds random*spread.492BD3..BFC stores life*1024;493072..084 subtracts update ticks and frees when the signed result is nonpositive. Original flame values are life.2 and spread.8, giving roughly0.04–0.2 seconds. Thus100ms can expire all samples in a burst. This is static arithmetic evidence, not proof that original gameplay schedules emitter/update/draw exactly as the clone at10Hz, nor identical RNG/quantization across machines. No lifetime or spawn-rate fix is justified by this stress alone.

## Real input/tank test

`tests::player_nitro_car_frame_coverage` is an ignored asset-backed diagnostic. It uses Game::step, throttle/NOS input, releases/retriggers and ordinary finite tank depletion. The first garage attempt is retained in `runtime-garage-blocked.log`: the Mustang never reached the flame speed threshold because it hit the garage wall; tank consumption in purge mode does not count as flame activation. This was a fixture failure, not a reproduced product failure.

The revised fixture uses a controlled flat course, preserving original car, upgraded handling, and emitter assets, to isolate input/tank/frame-rate transitions from route collisions. The revised run completed18 cases across9 stock-body cars, with1,431 active non-purge frames, zero empty flame systems and zero absent CPU quads. All tanks depleted normally. It is not a natural career drive or a GPU visibility test.


## Reproduction and decision

Use the verified asset root recorded in `runtime-summary.json`, private XDG data/cache directories, and a private Cargo target. Commands:

```sh
cargo run --release -p devprobes --bin nitro_marker_coverage > marker-inventory.jsonl
cargo run --release -p devprobes --bin nitro_frame_coverage > frame-survival.log
NFSU2_HEADLESS_ONLY=1 cargo test --release --bin game tests::player_nitro_car_frame_coverage -- --ignored --exact --nocapture
```

These commands require `NFSU2_ROOT`; this test uses Game::new().unwrap() and actual car/world loads, with no no-assets early return. `runtime-summary.json` retains exact tested binary/source hashes and environment. Build jobs2, private target agents/outrun-cache-followup/target. Runtime test elapsed3.19seconds,1pass/0fail; this is a focused diagnostic, not a full game suite.

**No persistent missing player flame reproduced. No effects change proposed.** CPU quads do not establish GPU visibility through bodywork, depth testing or a particular user camera. The new coverage contains no GPU screenshot or native live scheduling capture. Prior controlled Peugeot screenshots are separate evidence and do not resolve the player-specific report. A GPU comparison using the player's actual car/body/camera, if supplied, remains especially useful; the known long synthetic stress gaps do not explain sustained absence.

Parent integration normalized trailing whitespace in the disassembly and terminal blank lines in the retained runtime log; instruction bytes, observations and counts are unchanged. Original copies remain in evidence commit6058431.
