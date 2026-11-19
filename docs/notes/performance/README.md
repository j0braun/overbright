# Performance notes

Measurements and faster ways to draw the same frames. The gates a change runs are in
[perf-gates.md](../../developing/perf-gates.md).

| Note | What it found |
|---|---|
| [Frame budget](budget.md) | The cost of every enhancement per scene and resolution, and the regression gate. |
| [GPU frame time](gpu.md) | The GPU profile per pass and cheaper render encoding. |
| [Main-thread CPU](main-thread.md) | Less CPU per frame on the main thread, same frames. |
| [Integrated GPU](integrated-gpu.md) | Frame time on a small laptop's integrated GPU with four cores. |
| [Loading and pacing](loading-pacing.md) | Shorter transitions, no shader builds mid-play, an optional power-saving cap. |
| [Performance overlay](overlay.md) | What each frame figure means, where the readings come from, the CSV log and the overlay's cost ([player page](../../playing/perf-overlay.md)). |
| [Car draw index](car-draw-index.md) | Each car draw's index passed as an immediate, so its record reads as one uniform value. |
