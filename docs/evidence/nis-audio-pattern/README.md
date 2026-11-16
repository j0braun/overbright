# ShopEnter1 authored engine pattern

Baseline: `b9935e15cab8339ef09cdea7efed8974007c6f27`. Original EXE SHA256:
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.

The clone now routes ShopEnter1's authored pattern 3 through its existing
engine sound chain. This changes sound-controller inputs, not drivetrain RPM
or vehicle motion. Other patterns, including ShopExit pattern 4, remain
unsupported. Exact normalized RPM/throttle matching is established below;
complete mixed-audio waveform parity is not claimed.

## Native command and sequencer

`Scene_ShopEnter1_EvtTrk.txt` contains `CAR_AUDIO_PATTERN 1 3 1.0f` at frame 35.
Parser `0x432E8C` stores slot, integer pattern and float parameter. Advancement
`0x4395F0` dispatches when event frame × float `0x786460` is **strictly less**
than the track clock. Seek `0x43AF80` clears the cursor before advancing, so
crossed events are dispatched again on a seek. Dispatch `0x439547` calls the
sound context's virtual +0x48 with command 7, pattern and truncated parameter.
Handler `0x459550` stores pending byte +0xC0, pattern +0xC4 and parameter +0xC8.
The pattern branch of `0x46FA30` does not use that parameter.

Pattern 3 selects table `0x7F0238` and 26 segments. Each audio update advances
one quarter-step; the terminal update changes to idle without evaluating the
endpoint. It holds 2557 RPM for that update. Idle then subtracts 500 RPM and
15 throttle points, clamping RPM to 1000..10000 and throttle to 0..100.
The scripted branch returns before ordinary gear/previous-input refresh.
Launch predicate `0x45CD30` rejects scripted Physics (+0xD8), which the clone
now represents explicitly. Other transient/engine processing remains in place.

`native-instructions.txt` retains the relevant original instructions. Ranges
were individually disassembled from their exact entry addresses with GNU
`objdump -d -Mintel --no-show-raw-insn --start-address=... --stop-address=...`;
this avoids misalignment after embedded jump-table data.

## Natural envelope capture

Owned prefix: `$OB_PRIVATE/wine/runs/<run>`.
Private display 175; copied the finished-career save; game PID 1261098. Navigation was
Title → load the finished-career save → Career → Resume → Garage → Return to Explore → nearby
Body Shop dialog → Enter Shop. No hooks, placement writes or forced scene entry
were used for this capture. The session was stopped afterward. Audio used a
private null sink.

`natural-entry.json` retains 108 distinct raw Physics/context snapshots from
1,619 read-only polls: 104 pattern updates and four idle-decay updates. It stops
before the Physics vtable changes; later reads of freed storage are excluded.
Physics has vtable `0x78A008`. The actual **derived player sound context** has
vtable `0x78BF88`, update `0x479610`; it is not the base `0x78BAA8` context.
Its controller update clears the pending command latch after processing.
`natural-physics.bin` concatenates those captured Physics records.

The 103 observed inter-update intervals span 1.71422386 seconds, about 60.09
updates/second with polling uncertainty. This establishes the capture's cadence,
**not** a native render-independent 60 Hz scheduler. The clone deliberately
preserves this captured duration while rendering at other rates. This duration
policy is an adaptation unless further native scheduling evidence establishes
otherwise.

## Skip, destruction and re-entry

`seek-lifetime.json` records actual handler/seek calls and context generations.
The four observer hooks are `0x43BC30` (skip attempt), `0x43AF80` (seek),
`0x48174D` (derived context construction) and `0x481760` (destruction).
`seek-hook.S` replays displaced instructions and preserves flags/registers;
it does not touch x87 state. All changed bytes were restored and byte-verified.
Owned PID 1288710/display 175 was stopped. An earlier installation attempt
rejected occupied scratch storage before installing hooks; the final recorder
uses a smaller verified-zero region. Failed-install and full polling logs are
retained privately in the workspace review directory.

Two early/late Enter attempts during the first entry produce two handler calls
but only one seek. Its caller is `0x43BC95`, scene ID 1, header hash `E39FF628`,
type 3, current time 1.8666656017 seconds and target 4.6666669846 seconds,
context generation 1. Static `0x43BC30` rejects scene time ≤0.5 seconds. The
context destructor then runs and the world becomes null. The early negative
attempt was therefore a gated skip, not evidence that the original cannot skip.

Returning to Explore constructs generation 2 at the **same address**. Its
ShopExit pattern 4 is observed. A later type-4 seek has scene ID 2, header hash
`25F2E1C4`, current time 3.233330965 and target 4.0 seconds. Natural re-entry in
generation 2 replaces pattern 4 with pattern 3, then destroys that context on
returning to the front end. Final counters are two seeks, two constructions,
two destructions and three skip-handler calls. The compact fixture retains
every generation/pattern/world/handler transition and hashes the original log.

The clone retains the player's **immediate** Enter skip, including before the
native half-second gate. A scene owns its audio clock and cursor; teardown
removes them and clears controller history while keeping decoded GIN assets.
Replacing a scene without an intervening sound update also resets its state.
Pre-event ShopExit pattern-4 behavior is outside this bounded change.

## Reproduction and validation

From this directory:

```sh
python3 reconstruct.py '/path/to/verified/SPEED2.EXE'
```

This verifies the EXE hash and table bytes, both captured vtables, all 108
states and context outputs, and the seek/context-generation assertions. It
regenerates `natural-physics.bin` byte for byte. To rebuild the observer code:

```sh
as --32 -o /tmp/nis-seek.o seek-hook.S
ld -m elf_i386 -Ttext=0x782b00 -o /tmp/nis-seek.elf /tmp/nis-seek.o
objcopy -O binary -j .text /tmp/nis-seek.elf /tmp/nis-seek.bin
cmp /tmp/nis-seek.bin seek-hook.bin
```

From the worktree, with `NFSU2_ROOT` pointing to the read-only verified assets
and `CARGO_TARGET_DIR` set to a private build directory:

```sh
cargo test --release --lib audio::scripted::tests
cargo test --release --lib audio::hybrid::tests
cargo test --release --lib shop_scene::audio::tests
NFSU2_HEADLESS_ONLY=1 cargo test --release --bin game \
  tests::shop_pattern_native_envelope_and_skip_reentry_lifetime \
  -- --ignored --exact --nocapture
```

The production game test checks every captured RPM/throttle state, actual
30/60/144/240 FPS updates, three Enter skips (0/30/90 ticks), fresh re-entry,
natural completion and real offline mixer output. Standalone cadence coverage
also includes 120 and 360 FPS. Tests first failed on the absent sequencer,
parser/cursor, scripted Physics route and production route before implementation.
The previous game route emitted -161.29 normalized RPM instead of scripted idle
1000 in the first scene update.

The retained clone smoke audio has 142,400 stereo frames at 48 kHz, RMS
0.084924878 and peak 0.380844623. It uses the test's PEUGOT; the native recording
uses the copied profile's car and includes other sounds/music. These recordings
are audibility evidence, **not** an isolated same-car waveform comparison.
Full recordings, PCM, command logs and test logs are in workspace
`review/nis-audio-pattern/`. `clone-audio-smoke.json` identifies the derived WAV.
