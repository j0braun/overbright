# Notes

Investigation write-ups: how a difference from the original was found, measured and fixed, and what is
still open. They are background for developers; the player pages link to them where it helps. Each topic has
an index with one line per note.

| Topic | What the notes cover |
|---|---|
| [Parity](parity/README.md) | Races, driving, AI, world systems and props checked side by side against the original. |
| [Physics](physics/README.md) | Single fixes to the car simulation, each traced in the original's code. |
| [Outruns](outrun/README.md) | The outrun racers' strength, pace, recovery, speech and scheduling. |
| [Graphics](graphics/README.md) | The look target, the settings audit, car shading, reflections, pop-in, anti-aliasing passes. |
| [Audio](audio/README.md) | The sound effects audit and the nitrous stream. |
| [Performance](performance/README.md) | Frame budgets, GPU and CPU frame time, loading waits, integrated GPUs. |
| [Menus and shops](front-end/README.md) | Menu screens, shops, the garage and the post-race screens. |

The data behind the notes is in [../evidence/](../evidence/), one folder per topic. Pages that moved when
`docs/` was sorted into folders are listed in [MOVED.tsv](MOVED.tsv) (old path, new path).
