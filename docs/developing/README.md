# Building and testing

Pages for developers. Start with [BUILDING.md](../../BUILDING.md), [ARCHITECTURE.md](../../ARCHITECTURE.md)
and the [developer guide](developer-guide.md).

## Tests and checks

| Page | What it covers |
|---|---|
| [Testing](testing.md) | The three kinds of tests, test data from your own game files, private test data. |
| [Check harness](checks.md) | The checks every merge runs: build, tests, shots, flows, the rate check across frame rates. |
| [Test determinism](test-determinism.md) | Keeping tests independent of order, threads and timing. |
| [Fuzzing the file readers](fuzzing.md) | Fuzz targets for every parser that reads player-controlled bytes. |
| [Software-GPU check](software-gpu.md) | The startup check on Mesa's lavapipe driver. |
| [Build times](build-times.md) | How long a build takes and what the development profile saves. |

## Proving behaviour against the original

| Page | What it covers |
|---|---|
| [Parity workflow](parity-workflow.md) | Step by step: proving a simulation change against the original under Wine. |
| [Differential parity harness](parity-harness.md) | The harness that runs Overbright and the original from one start and compares them. |
| [Traffic parity harness](../traffic-harness.md) | Seeded headless traffic sessions compared with the original's statistics. |
| [Performance harness and gates](perf-gates.md) | The gates a performance change runs before and after. |
| [Evidence register and test rules](evidence.md) | The tags for claims about the original (observed, traced, inferred, choice, open), the check that enforces them, and two rules for test code and visual changes. |
| [Provenance tags](provenance.md) | How docs mark values as verified, observed, inferred or open. |

## How the program is built

| Page | What it covers |
|---|---|
| [Developer guide](developer-guide.md) | Tools, diagnostic switches, the early graphics rows, performance notes. |
| [The simulation's boundary](sim-session.md) | One world update is one call to `sim::Session::step`. |
| [Settings per frame and per tick](frame-settings.md) | Which settings are read once per frame or per world update. |
| [Settings model](settings-model.md) | Every setting's group, kind and rules; the conflict and redundancy audit. |
| [The settings page](settings-page.md) | The Enhancements page, its group pages and how they are tested. |
| [The asset cache](asset-cache.md) | Cache file names, versions, writes and temporary files. |
| [First use](first-use-builds.md) | Shader programs made before the frame that needs them. |
| [Prepare](prepare.md) | `game --prepare`: the first drive's work done ahead. |
| [Read-ahead](read-ahead.md) | Reading the next minutes' files ahead of time. |
| [Waits and frame times in the log](waits-log.md) | What the game's log records about waits and slow frames. |
| [Runtime I/O robustness](io-robustness.md) | How file and device errors are handled without a panic. |
| [Game-version fingerprint](game-fingerprint.md) | Recognising which edition and version of the game is installed. |

## Releases

| Page | What it covers |
|---|---|
| [How releases are built](building-releases.md) | `scripts/release/package.sh`: the player archives and their checks. |
