# Contributing to Overbright

## The rules

### Native first

The default build is the original game, exactly: its rules, its timing, its look and its sound. "Native"
means checked against the original executable (`SPEED2.EXE`, PC 1.2), not remembered or guessed.

- Trace a behaviour in the executable (a disassembler or decompiler such as Ghidra) and, where it can be
  seen, confirm it in the running original (the original under Wine; docs/parity-workflow.md and
  docs/parity-harness.md describe the comparison tools).
- Every claim carries its status: **verified** (traced in the executable, with addresses), **observed**
  (seen in the running original), **inferred** (a reasoned guess) or **open** (not known). An inference is
  never written down as a fact. "Verified" means traced, not that the picture matches: visuals are judged
  against the original's frames.
- Code that rests on an inferred or open value carries a provenance tag, `// prov: inferred <id>` (or
  `open`), registered in docs/provenance.tsv; `scripts/checks/provenance.py` checks both sides
  (docs/provenance.md).
- A difference from the original that is not listed in docs/native-deviations.md is a bug.

### Every extra is opt-in

Anything that is not the original (graphics enhancements, quality-of-life settings, higher limits) is off by
default, can be switched off again, and goes back to the original with Restore Native. With every extra off,
frames, sound and simulation are byte-identical to the build without the extra; the gate checks this. A new
deliberate deviation of the default build is a design decision: it is added to docs/native-deviations.md with
the way back to the original behaviour. Enhancements keep the original's character (its dark, contrasty
nights, its colours, glow and grade; docs/look-direction.md) and do not brighten or flatten the image.

Performance work needs no switch when it does not change what comes out: the same frames, sounds and
simulation, measured. Tiny invisible rounding is allowed when it is reported.

### Any frame rate

The game is never locked to 30 or 60 fps. The world advances as the original's does at 60 fps, whatever the
display's rate, and drawing is never capped (docs/uncapped-fps.md). Gameplay must be identical at 30, 60, 120
and 200 fps. A frame cap is never a fix for a rate-dependent bug.

## How a change is checked

1. `cargo build --release --bins --examples --features dev-harness` and
   `cargo test --release --features dev-harness` (BUILDING.md). New logic comes with tests; data tests follow
   docs/testing.md (ignored with a reason without the game, never passing silently).
2. The checks and the gate (docs/checks.md): `CHECKS=full scripts/checks/gate.sh <outdir> <base>` builds the
   change and its base (a commit, e.g. the one your branch starts from, or a folder) and compares shots, flows,
   audio and simulation state. A change that should not change the output passes byte-identical. Without the
   maintainer's private set, the steps that need it are reported as skipped (BUILDING.md, "Without the private
   set"); that is expected.
3. A change that changes the output on purpose (a native fix, a new opt-in extra) lists every difference the
   gate reports and says why each one is expected. A native fix names the addresses it follows. Unexplained
   differences are bugs.
4. The rate gate (`RATE=1`, docs/checks.md "Rate gate") compares the simulation across 30, 60, 120 and 200 Hz;
   a change must not make a column that was identical differ.
5. Headless only: the checks never open a window on your desktop.

### Before you send a change

From a checkout with `NFSU2_ROOT` set (`scripts/local.env`), base = the commit your branch starts from. Times are
from the reference desktop (16 threads, warm compiler cache) on a public checkout:

| command | time | needs |
|---|---|---|
| `scripts/checks/gate.sh <outdir> <base> extreme` | ~20 min | a Vulkan GPU; the build and test steps of BUILDING.md |
| … with `RATE=1` (timing, input, audio, harness) | +4 min | |
| … with `FRESH=1` (the launcher) | +3 min | Xvfb, xdotool; mutter for the Wayland part |
| … with `WINE_SMOKE=1` (platform code, anything behind a `cfg`) | +6 min, more on a first run | wine, Xvfb, podman (the Windows build) |
| `scripts/checks/selfcheck.sh <outdir>` (when you change a check script) | ~1 min | python3 |
| `scripts/checks/checks.sh full <outdir>` (parsers, settings, resolutions, release scripts) | ~27 min | as above |

The last line says `GATE: PASS` or `GATE: FAIL`. A missing tool is a skip, not a failure: the Wine smoke
without wine, Xvfb or podman, `FRESH=1` without Xvfb or xdotool, the Wayland test without mutter, the lavapipe
check without Mesa's software Vulkan driver. Without the private set the steps that need it are skipped too
(BUILDING.md); every skip is named on the `CHECKS:` line, and the `GATE:` line counts them
(`; skipped: checks N, parity M`).

## Recording reverse-engineering findings

- **docs/formats.md**: what is known about the original's files and behaviour, by subject, with addresses and
  the verified / inferred / open status of each claim. It is the index of the chapter files in docs/formats/
  (add a section to the chapter of its subject, then rerun `scripts/docs/formats_index.py`). Topic documents
  (docs/*.md) go into more depth.
- **docs/symbols.tsv**: one row per address (address, kind, name, description, status, source). It is the
  single source of names for addresses: no duplicate addresses; use these names in code and docs.
- Evidence that is our own (notes, numbers, small tables) can go under `docs/evidence/<topic>/`. Raw
  recordings of the original stay out of the repository (below).

## What must never be committed

- Game data in any form: files of the game, extracted or converted assets, textures, sounds, movies, text
  dumps, tables copied from the executable (read them from the player's copy at run time; docs/exe-tables.md).
- Screenshots or video frames of the original or of Overbright drawing the game's assets.
- Save games and profiles (yours or anyone else's), memory captures and traces of the original.
- Private paths (home folders, machine names), personal names, local settings files (`scripts/local.env`).
- Code or assets taken from the original, from other games, or from mods.

Look over your diff for these before you send a change; every pull request is reviewed for them before it is
merged.

## Third-party code

Proven libraries and algorithms are preferred over home-grown ones when they fit. Allowed licences are those
compatible with GPL-3.0-or-later (for example MIT, BSD-2/3-Clause, Apache-2.0, Zlib, ISC, MPL-2.0, LGPL-3.0,
GPL-3.0, CC0). Not allowed: GPL-2.0-only, proprietary or unlicensed code. Record the name, version, URL and
licence in THIRD-PARTY-NOTICES.md and keep vendored code in its own file or folder with its header intact.

## Licence of contributions

Overbright is licensed under the GNU General Public License, version 3 or later. By contributing you agree that
your contribution is licensed under the same terms (GPL-3.0-or-later).

## Sign off your commits

Every commit must carry a `Signed-off-by` line with your name and an email you control. Add it with
`git commit -s`. By signing off you certify the [Developer Certificate of Origin 1.1](https://developercertificate.org/):
that you wrote the change or have the right to submit it under this project's licence. In this project that
also means: nothing copied from the original game, from its executable or from other projects' code without a
compatible licence. A check on every pull request (`.github/workflows/dco.yml`) fails when a commit has no
sign-off.
