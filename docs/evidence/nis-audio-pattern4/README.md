# ShopExit1 pattern 4 and shared player sound context

Follow-up to `../nis-audio-pattern`. This slice covers only ShopEnter1/ShopExit1,
not every authored scene and not full mixed waveform parity.

`inventory.py GAME_ROOT OUT.json` independently inventories all 35 original
`NIS/*EvtTrk.txt` files (including ten without audio), retaining their hashes,
147 commands across 25 tracks, and extracts all 15 dispatch tables from the
verified executable. IDs 1/2 belong to DynoEnter/Exit, 3/4 ShopEnter1/Exit1,
5–12 race intros, and 13–15 endings. All authored parameters are 1.0. Table 4
starts at 0x7F02A8 with 30 segments, selected by 0x46FAE6. Like pattern 3 it
uses four updates per segment; update 120 switches to idle while holding update
119's value, then idle subtracts 500 RPM / 15 throttle and clamps. Dispatch and
interpolation instructions are retained in the preceding pattern-3 evidence.

## Native capture

Owned display 175, private copied profile in
`$OB_PRIVATE/wine/runs/<run>`, PID 1452697.
Natural navigation: load copied the finished-career save → Career/Resume → Garage/Return to Explore
→ nearby Body Shop confirmation → natural ShopEnter1 → Body Shop/Escape/confirm
→ natural ShopExit1 → Explore → nearby Body Shop confirmation → natural entry.
No teleport, skip, pattern write, or user save mutation. Observer hooks are the
same seek/constructor/destructor hooks as the preceding fixture; the retained
script batches reads to retain each pattern update. Hook code is
`../nis-audio-pattern/seek-hook.bin`; all installed bytes were restored and
verified, and the owned session was stopped.

`natural-lifetime.json.zlib` retains the complete raw polling log and hook
records. `reconstruct.py` regenerates `pattern4-physics.bin` and
`natural-summary.json` from it. It validates vtables, context→Physics linkage,
120 consecutive exact pattern states plus idle, and context generations.
Physics+0xD8 is a **byte**; adjacent padding is not a boolean.

The same address (0x4D5FD40) is reused across generations, so address equality
alone is not lifetime evidence. Generation 1 is destroyed by shop entry.
Generation 2 starts for shop exit and remains through free roam and the next
shop entry. Its scripted byte clears during free roam, then sets on entry;
pattern 4 remains stored until the authored pattern-3 event replaces it.
Only after entry completes does destructor count advance to 2. Native pattern
4 has already reached idle in this natural, unskipped exit, so the nonterminal
resume case is a clone immediate-skip adaptation tested separately rather than
claimed as captured native behavior.

## Scope and remaining validation

The clone deliberately uses the observed 60-update/second duration at uncapped
render rates; this is not proof of native scheduling at alternate render rates.
Immediate Enter remains an authorized clone adaptation, including before the
native skip handler's 0.5-second gate. Scene event cursors are per-scene; pattern
state and controller history belong to the car sound context and survive the
observed Explore transition. FE destruction must discard them.

All 15 tables are inventoried, but only patterns 3/4 and their scene gates are
implemented by this slice. Dyno, multi-car intro, and ending scene lifetimes
still require bounded native validation before routing; other scripted commands
are not covered. The broader audio parity task remains open.

A further required validation is a matched **same-car native/clone engine-only
recording**, with music and other sounds disabled through supported settings and
configuration provenance retained (car/parts, engine entry, sample rate, sound
levels, scene, timestamps). Existing different-car mixed captures establish
normalized control-envelope equivalence and audibility, not waveform equality.

Before enabling intro IDs 5–12, recover native event ordering: authored text
can be nonchronological (IntroNis08 audio frames 90, 40, 100, …). The current
per-scene cursor is sufficient for the single-event shop scenes; its raw-file
ordering must not be generalized to intro scheduling without native evidence.

Static ordering landmarks: `0x432D00` computes the next 0x1C-byte event slot
from count+0x14 (`0x432D06–0x432D15`); audio fields are parsed at `0x432E8C`.
Event advancement is `0x4395F0` (called from `0x43AC37` and seek `0x43AFB3`),
and audio dispatch is `0x439547`. These landmarks alone do not establish any
post-parse sorting/tie behavior; retain that as a prerequisite. Exact unsorted
file contents remain in the verified assets; `inventory.json` retains each
file hash and original line/order/frame values, including IntroNis08 and25.

Capture reproduction uses a private output directory: copy `capture-lifetime.py`
and `../nis-audio-pattern/seek-hook.bin` there before running the recorder in
an owned 175 session. The recorder writes `seek-lifetime.json` beside itself;
do not run it in a retained evidence directory. `session.json` retains the
owned session identity, audio/sync paths, and final stopped status.

Clone regression distinguishes FE destruction from retained-world menus:
ExplorePause through actual Escape/Back must preserve a nonterminal pattern
and controller fields. Unsupported commands log incomplete scene audio rather
than crash or silently claim support. A new drive or different engine entry
clears the context even if decoded assets remain available. Destruction also
resets an ordinary-only context; it does not depend on whether a pattern ran.

## Validation

On base binary, with verified assets, private test data (`Game::new` test
isolation), and a private Cargo target:

- Five sequencer tests pass, including all 108 prior entry states, all 121 exit
  states, and 30/60/120/144/240/360 FPS duration/replacement checks.
- Full release library: 520 passed, 0 failed, 12 ignored.
- Full release game tests after the final context correction: 19 passed,
  0 failed, 14 ignored.
- Explicit real-asset `tests::shop_pattern_native_envelope_and_skip_reentry_lifetime`
  and `tests::shop_exit_pattern_and_shared_context_lifetime`: both pass. The
  latter covers actual `Game::step`, nonterminal immediate Exit skip, free-roam
  pause/resume, retained pattern before entry's event, FE destruction, production
  Explore reconstruction and same-engine-entry ordinary-only replacement.
- Native fixture reconstruction and original-assets inventory regenerate
  byte-identically; the retained manifest covers all evidence files.

Reproduce the focused game checks from the repository root:

```sh
NFSU2_ROOT='/path/to/verified/game' NFSU2_HEADLESS_ONLY=1 \
CARGO_TARGET_DIR='/path/to/private/target' cargo test --release --bin game \
  tests::shop_ -- --ignored
```

The new tests failed before the implementation (pattern 4 unsupported; game
Exit using ordinary RPM), then passed with the shared-context implementation.
No GPU timing or mixed-waveform equality claim is made.
