# Race ending selection (2026-11-02)

Static trace uses the verified SPEED2.EXE (SHA-256
8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297),
REA/Ghidra 12.1.2 at 0x42FED0 and the full speed2.dis listings.

## Verified selection and binding

0x436840 case 1 walks ending records at NIS table +0x25C. It applies the
track mask (0x4300F0), drivetrain gate (0x42FED0), player flags
(0x42FF50), and memory gate (0x42FF20). At 0x436BC5 it calls 0x43C1C0
with the surviving count, consuming the shared random stream once.
There is no track-dependent reseed or private random-state copy.

0x42FED0 reads the **first results entry**, [0x851044], then its racer
id +4. 0x5F0E40 finds that car. Car +0x20 is its tuning block;
+0x1C0 is the rear torque split (record +0x2D0), including performance
upgrades. EndingNis01 requires this value strictly above 0.8. The old
claim that this unknown scalar belonged necessarily to the player was
wrong. It belongs to the winner, including an AI winner.

0x431DF0 builds the flags from that same results car. Offline, +0x1E
means its controller is human and player index +0x10 is zero. 0x42FF50
requires entry flag 0x40 and requires 0x80 exactly when a human wins.
The existing downhill exclusion remains. Thus a loss selects different
camera variants from a win; selecting 0xC0 unconditionally was wrong.

0x4328B0 type 1 (0x4329D8..0x432A62) binds the winner to the slot in
entry +6 and the player to +7 when an AI wins. On a human win the
player takes +6. The shipped entries contain slots 1 and 2. The clone
now passes first-results handling and human-win state to the selector,
uses the shared RNG, and binds the player's pose to that entry slot.

## Original runtime observation

Private Wine session :191 (wined3d), 240SX, Quick Race Resort Loop.
Evidence directory:
an internal run folder.
The successful capture and reproducer are preserved in
`docs/evidence/race-endings/` with hashes and instrumented scope. The
external capture is `native-finish-rwd.json`, with
`burnout-active.png` and `native-finish-rwd.png`.

This is an **instrumented transition**, not a naturally driven win:
the finish151 procedure writes the player's racer laps/most-laps and
finish-line tick, letting 0x608C11 and the native finish path build the
results. Other ending entries' flags were temporarily cleared to
isolate EndingNis01; its drivetrain/player/memory gates were unchanged.
The table was restored, Wine stopped, and the private save restored
with matching SHA-256. No original workspace or user save was modified.

The winning car's rear split was 1.0. There are 45 active mode-5 samples
with chassis brake lock +0x250 = 0.200000018 and burnout additive
+0x254 = 2.5. One sample's front angular speed is 4.883916 rad/s and
rear is 40.127541; rear minus front/0.2 is approximately 15.70796,
matching 2*pi*2.5. The values later reset to brake 1, additive 0,
and all four angular speeds 0. This matches the asset events at frames
0 and 185. The first active screenshot is a close camera view; it does
not establish smoke visual parity.

Earlier diagnostic captures are retained but are **not successful
winning-scene proofs**: writing finish flags directly left AI results
first. Their failure exposed the results-winner gate; a RWD player
alone is insufficient when the winning AI is FWD.

## Remaining limits

The original selection-only change left the AI winner absent on a loss;
the scene-owned-car follow-up below addresses that gap. A human win does
not select an AI runner-up: its second binding is the second human player,
which the single-player clone does not have.
Existing key-driven placement approximates the native mover-5 physical
controller; this work does not establish trajectory or smoke parity.
Scene event/wheel integration is handled separately. The PC memory
pool is still assumed to have room, as before. Matching the full native
random sequence depends on all other RNG consumers matching too.

Validation: the new selection regression first failed to compile due
to the missing selector, then passed after implementation; it covers
FWD, AWD, strict 0.8 boundary, upgraded 0.801 rear split, RWD, memory
fallback, no-candidate behavior, winner flags, and RNG consumption.
`cargo check --release --bin game` passed. Full integrated build,
selftest, and rendering checks belong to the parent integration pass.

## Wheel integration review correction (2026-11-03)

Integrated wheel phase/control commit and torque-parser commit
`8b02b8a` into this ending-selection branch. The shared scene update now
uses `run.car_slot` for wheel events as well as pose channels. The old
literal slot 1 would apply the winner's brake/burnout to a losing player
bound to slot 2.

The ignored asset-backed game regression
`loss_ending_uses_player_slot_for_wheel_controls` selects a native
EndingNis01 loss entry through the shared RNG, confirms slot 2 binding
and winner-only event records, then calls the real shared scene updater.
Before the correction it failed: front spin -0.33494565 versus rolling
-1.6747283, exactly the winner's brake multiplier 0.2. The corrected test
checks all four wheels against slot 2's unmodified rolling speeds.
Run with explicit `NFSU2_ROOT` and isolated XDG/TMP; an ordinary test run
skips this ignored regression. Focused integrated results are recorded in
internal evidence: race-endings/integration-validation.txt.

This integration does not implement the second scene car or active body
response to torque records. Shared ending/shop pose placement remains an
approximation, with body spin zeroed rather than reconstructed from the
scene yaw trajectory; turning skid and smoke fidelity remain unverified.
No uncapped gameplay or timestep policy changes are included.

## Scene-owned AI winner (2026-11-03)

Fresh disassembly of the verified executable is preserved in
internal evidence: race-endings/native-car-binding.asm. It corrects the earlier
assumption that every single-player win should show a second AI car:
0x432A06 binds human player 0 to entry +6, and 0x432A14 binds human
player 1 to entry +7 through 0x4307F0. That helper indexes the human
player array at 0x8900AC. These are not results-list indexes. In the AI
winner branch, 0x432A4C binds the already-resolved winning car to entry
+6 through 0x4326F0, then 0x432A59 binds human player 0 to entry +7.
Consequently the single-player implementation adds the AI winner only
on a player loss, without inventing an AI runner-up on a player win.

The results path snapshots that winning rival's exact fitted `Car` and
`CarState` before leaving the race. `ShopRun` owns the snapshot, so it
survives even when replay recording is disabled and does not alter the
rival kept for replay. This costs one CPU car-data clone per AI-won ending;
its GPU renderer is created lazily. The snapshot is released with the
scene. Slots come from entry +6/+7, and a missing companion channel does
not fall back to the player's channel. Both live rendering and headless
shots submit the companion, with its own wheel poses, environment/lighting,
shadow/underglow and headlight pool. Player wheel binding stays unchanged.

The added car uses the same channel-driven placement approximation as the
player, with its own slot's wheel events. This does not implement native
NISMover target forces, active body-torque response, rev patterns, or
per-car smoke/flare parity. Body spin remains zero in ending playback.

The ignored asset-backed regression
`ending_keeps_ai_winner_in_its_channel_without_mutating_replay` exercises
native EndingNis02 loss selection, distinct AI slot 1/player slot 2 poses,
exact car identity retention, unchanged race physics state, and survival
when a race without a recording is dropped. A subsequent human-win scene
must not retain/invent that AI companion. The existing ignored
`loss_ending_uses_player_slot_for_wheel_controls` remains the check against
winner burnout leaking onto the player. These offline checks establish
binding/ownership/update state, not actual pixels; render submission is
compiled and inspected but needs screenshot validation for visual claims.

The regression also runs the actual race-results dispatcher with the second
AI finishing first, verifies that rival is retained, and confirms subsequent
scene updates do not move the replay's saved rival. The normal dispatcher
starts a recording automatically; the no-recording ownership case is tested
separately by dropping the race through keep_replay.

Validation: `cargo check --release --bin game` passed. With verified
NFSU2_ROOT and private win-target/XDG/cache/TMP directories,
`cargo test --release --bin game ending_ -- --ignored --nocapture --test-threads=1`
passed both tests (2 passed, 0 failed, 18 filtered; 0.36 s test runtime).
No GPU, audio device, Wine run, or full native trajectory comparison was
performed. Logs: private review `win-companion-check.log` and
`win-companion-tests-final.log`. A preliminary expanded test incorrectly
expected the ordinary race update not to restart recording; inspection of
Recording::new corrected that test assumption before the final passing run.
