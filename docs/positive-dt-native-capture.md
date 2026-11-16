# Fresh native positive-dt component boundaries

Captured 2026-11-03 in an owned Wine copy of the verified English target
(SPEED2.EXE SHA-256 `8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`).
This supplies fresh original-game evidence missing from
`rest-component-refresh.md` and the retained frame-entry x2 recorder.
**No clone physics or scheduling changes are included.**

## What was recorded

The recorder runs on the native game thread, copying actual state at:

| Tag | Native boundary |
|---|---|
| 1 | `0x5B04E0` entry, before normal component traversal |
| 2 | `0x5B051B`, after normal traversal, immediately before `0x5ABB10` |
| 3 | return from rest test `0x5A5E30` to `0x5ABB33`, retaining EAX |
| 4 / 5 | before / after each visited bump helper `0x59E1B0` at `0x5ABB58`; x87 return retained |
| 6 / 7 | before / after `0x5A0D90` at `0x5ABBE9`, retaining actual zero substep dt |
| 8 | after `0x5ABB10`, before the next force-pass tail dispatch at `0x5B0520` |

Every record includes actual `0x7FC800` substep dt, `0x865150` frame dt,
`0x865188` simulation ticks, the live component list, full suspension
objects and their pointed-to records (including exact maximum compression),
wheel objects and contact normals, rigid-body pose/momenta/cached velocity,
engine/clutch state, controls, registers and FXSAVE state. The previous
compression for a normal traversal is its tag-1 value; suspension +0x20
is **rate**, not previous compression. Tag 2 gives the newly measured value.

The native comparison is ordered **equality**, not an inferred threshold:
`0x5ABB4B..0x5ABB56` compares current compression to record +0x14, tests
AH against `0x44`, and skips on parity. Only ordered equality reaches the
bump helper. Exact captured maxima are front `0.1600000113248825` and
rear `0.16500002145767212`. The instruction extract is retained.

## Results

| Run | Records | Complete step groups | Positive normal posts | Rest-triggered zero refreshes | Bump-triggered zero refreshes |
|---|---:|---:|---:|---:|---:|
| baseline, no teleport | 1,572 | 334 | 334 | 118 | 0 |
| landing, +1 m / -2 m/s | 2,932 | 678 | 678 | 110 | 0 |
| hard landing, +0.6 m / -12 m/s | 2,560 | 556 | 556 | 112 | 17 |

All runs have **zero dropped records, no incomplete step groups, fixed
TABLE pointers verified against the live list/object vtables, exact
suspension-record pointers verified, and successful hook restoration**.
The ordinary landing reached only approximately 0.084 m front / 0.052 m
rear compression. It is retained as a negative control, not relabeled as
bump coverage.

The hard landing has 39 bump-call pairs. Native step 134 reaches front
full compression during the normal component post (records 756 → 757).
The rest return is zero. Front bump returns are 1.7469496727 and
1.7537295818; the zero-time component post follows, then the next force
pass. Step 135 reaches rear full compression. Wheels 3 and 2 leave full
compression at steps 137 and 138. Both fronts leave at step 151, whose
complete sequence is `[1,2,3,8]`: no bump and no zero-time post.

This exposes the cached-velocity refresh, not just a clutch phase:

| Hard-landing record | Boundary | Body vertical momentum | Cached vertical velocity |
|---|---|---:|---:|
| 757 | normal post | -6.75891447 | -6.40655327 |
| 763 | after impulses, before zero post | -3.26074600 | -6.40655327 |
| 764 | after zero post | -3.26074600 | -3.09075427 |
| 765 | next force input | -3.26074600 | -3.09075427 |

Step 139 visits two fully compressed front suspensions and both bump
returns are zero, but still executes the zero-time traversal. This is
fresh positive-dt evidence for the no-impulse refresh case. Its rest
return is zero, so these callbacks are not being confused with rest hold.

The same continuous capture contains a native throttle change 0 → 1
between adjacent complete steps 112/113, before the impact, and 1 → 0 at
340/341. Gear and target gear remain 2 through the compression episode;
clutch phase is Engaging (2), with engine power 0.7. The bump zero posts
in this episode do **not** change the clutch phase or power. Therefore
this is **not** evidence of a gear/phase transition exactly coincident
with the compression boundary. Rest-driven phase changes elsewhere in
the trace are distinguished from bump-driven updates by the captured
rest return and bump-call sequence.

## Instrumentation and provenance

the maintainer's private captures retains the assembly, linked recorder bytes,
disassembly, Python driver/decoder, three complete losslessly compressed
raw streams, metadata, summaries, selected hard-landing decoded boundaries,
and a SHA-256 manifest. The assembly saves/restores GPRs, EFLAGS and full
x87/SSE FXSAVE state; in particular the bump helper's x87 return is not
consumed by logging. Raw FXSAVE bytes remain in every record. Exact original
and replacement hook bytes are retained in each run's metadata.

Code occupies verified zero padding at `0x782B00`; no executable file
was changed. Installation and restoration occur while the game is paused
through its menu and SIGSTOPped. The five hook sites are checked against
the exact executable bytes before installation. The recorder redirects
calls/returns, but the original component/rest/bump implementations run.
The temporary hooks and telemetry padding are restored byte for byte.
A process-local allocation holds the records; stopping the owned process
releases it. The private executable's file hash is unchanged afterward.

The runs use a Peugeot 206 in Quick Race. Teleports change only the native
rigid body's position and momenta, with the original mass used to convert
requested vertical speed to momentum. Every before/after byte and keyboard
transition is retained in metadata. Suspension travel, maximum compression,
engine/clutch phases, gears and wheel spins are **not** fabricated or
written to produce an expected result. The native simulation supplies
those states. Instrumentation and SIGSTOP can affect wall-clock timing;
actual native dt is retained rather than substituted with a claimed fixed
rate. This is not an uninstrumented trajectory or performance comparison.

Owned runtime paths:

- Wine: `$OB_PRIVATE/wine/runs/<run>`, display `:174` (stopped).
- Worktree: an internal run folder, based on `ae2dbbb`.
- Runtime logs/full decoded JSON: an internal run folder.

The original source, original game directory and user saves were not
written. The borrowed scene work and parent physics source are unchanged.

## Reproduction and verification

Use an owned copy as required by `capture.py`, the retained an assistant
`wine/fastgpu.sh` method, and `fastnav.py OWNED race` to reach a fresh
Quick Race. Read the driver before running it; it installs temporary
in-memory instrumentation. Run the baseline, ordinary landing and hard
landing in that order. Each finishes at the native pause menu; send Escape
to resume before starting the next driver invocation:

```
python3 capture.py OWNED baseline --seconds 1.2 --no-teleport
# Escape, then:
python3 capture.py OWNED landing --seconds 2.5 --drop 1 --vertical-speed -2
# Escape, then:
python3 capture.py OWNED hard-landing --seconds 2 --drop .6 --vertical-speed -12
```

Exact recorded initial states and writes, not these relative-drop commands
alone, define the preserved trial. New playback need not be bit-identical.
All native sessions must be coordinated with other GPU work and stopped
through the owned-prefix supervisor afterward.

Offline audit (no game, Wine or clone simulation required):

```
python3 $OB_PRIVATE/docs/evidence/positive-dt-native/test_decoder.py
```

This audits all three complete streams, verifies raw/compressed hashes,
record indices, actual pointer/list consistency, normal/zero/force group
completeness and actual substep dt. Summaries reproduce identically. It
also deliberately alters a target pointer while recomputing transport
hashes, and truncates a group while updating counts/hashes: both are
rejected for the relevant semantic failure. Input transitions are accepted
only across adjacent complete step groups. `git diff --check` passes.
No Rust build/test is claimed or required by this evidence-only change.

## Remaining work

This provides a non-circular original-game oracle for compression entry,
exit, impulse and zero-time cached-state refresh. It does not by itself
validate any proposed clone reordering. A regression must initialize the
relevant clone state from the actual captured boundary, then compare
normal post, bump/zero post and next-force inputs. Moving only an early
`post(0)` call is still not established as the complete component port.
Coincident gear-transition coverage, broader collision paths and ordinary
uninstrumented trajectory comparisons remain open. No FPS cap, tolerance
relaxation or contact-test weakening is introduced.
