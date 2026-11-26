# Coverage maps

A coverage map lists, for one subsystem of the original game, every function of SPEED2.EXE that
decides what the player sees, and says how far this game reproduces each one. It exists so that a
gap is visible before someone answers "that is native" from one traced path: a behaviour is only
as native as the least-covered function on its path.

One row per original function (or per tight group of helpers that only make sense together):

| Column | Meaning |
|---|---|
| Address | entry of the function in SPEED2.EXE |
| Name | the name in [`symbols.tsv`](../../symbols.tsv) (each map adds the names it needs there, no duplicate addresses) |
| What it does | one line, the original's behaviour |
| Ours | where this game does it (`file:function`), or `—` |
| Status | one of the words below |
| Evidence | what the status rests on: a trace (decompile / asm ranges), an observation under Wine, a harness or probe metric, a test |
| Risk | what a player could notice if the row is wrong: high / med / low / none, with a few words |

## Status words

| Status | Meaning |
|---|---|
| **PORTED** | traced in the original and reproduced value by value (constants, branches, order of random draws); differences only where the trace says they cannot show |
| **SIMPLIFIED** | traced, but ours knowingly differs: a branch left out, a constant or a test taken from a sibling function, a fallback not taken. The row says what differs |
| **GUESSED** | written from observation, a neighbouring function or reasoning, without a trace of this function (or of the part named). Matches may be accidental |
| **MISSING** | the original runs it and ours has nothing for it |
| **NOT NEEDED** | the original runs it but it cannot change what the player sees (debug output, data the retail files never trigger, a platform detail). The row says why |

A row may add **in change** when another branch is rewriting that area at the time of writing; the
status then describes the default branch as it is.

Within a row, the evidence words are the tags of the evidence register
([evidence.md](../../developing/evidence.md)): **traced**, **observed**, **inferred**, **choice** and **open**.
A row without one of them fails the evidence check.

The original's AI is frame-rate dependent: its think slots, avoidance proposals and contact tests
run per frame. A native number for an AI row is therefore taken with the original at a fixed 1/60 s
frame, this game's own world step, and is marked "fixed 1/60"; numbers from older real-time runs on
wined3d, at an unknown frame rate, are indicative only (see "A valid native measurement" in
[parity-harness.md](../../developing/parity-harness.md)).

## Maps

| Map | Subsystem |
|---|---|
| [traffic-ai.md](traffic-ai.md) | traffic cars: pool, placement, walk, planner, think, avoidance, intersections, stops, mover, lights, horn |
| [racer-ai.md](racer-ai.md) | AI racers |

Each map ends with a summary: counts per status and a ranked list of the gaps worth closing, each
with the player-visible effect expected and how to check it (for traffic: the [traffic parity harness](../../traffic-harness.md); for racers: the [parity harness](../../developing/parity-harness.md)).
