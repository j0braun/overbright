# Evidence register and test rules

Two things keep what is known apart from what is assumed: a fixed set of tags on every claim about the
original game, and two rules that keep checking code away from the normal path.

## Tags

A claim about how `SPEED2.EXE` behaves carries one of these words, and means exactly this by it.

| Tag | Meaning |
|---|---|
| **observed** | Seen in the running original (under Wine or on a real machine): a screenshot, a log, a measured rate, a captured value. Name the run when it matters ("fixed 1/60"). |
| **traced** | Read in the original's code at a named address, followed to the end of the path that matters. |
| **inferred** | Reasoned from neighbouring code or behaviour; the function itself was not traced or the behaviour not seen. May be wrong. |
| **choice** | An agreed deviation or a clone decision with no native value. It is not a claim about the original. |
| **open** | Not known. |

When a claim has two bases, name both ("traced; observed under Wine"). The strongest one a reader can
check goes first. A claim is only as strong as its weakest part.

The older prose word "verified" means traced. Coverage maps use the new words. The source-code markers
(`// prov: inferred|open|choice <id>`, [provenance.md](provenance.md)) keep their three kinds; the tags
here add observed and traced for the docs.

## Where the check applies

`scripts/checks/evidence.sh` (about a second, docs only) fails on a claim row without a tag in:

- the coverage maps, [docs/original/coverage/](../original/coverage/README.md): every table row that starts
  with an address needs a tag word in its Evidence column;
- the table of differences you get out of the box in
  [native-deviations.md](../playing/native-deviations.md), section 2: every row needs a tag word in its
  Basis column (`choice` for the deviation itself; add `original traced` or `original observed` where
  the row also states what the original does and how that is known).

- prose in `docs/playing`, `docs/modding`, `docs/developing` and `docs/original`: a paragraph that says
  something behaves "like", "as in" or "matches" the original, or names a native value, needs a tag word in
  the same paragraph, written as `(tag: pointer)` where the pointer is a test, a trace note or a
  measurement script. Where nothing is known the tag is `open`; never write `observed` without a run.
  Headings are not checked.

Claims that predate the check and carry no tag may be listed in `scripts/checks/evidence-allow.txt` with a
reason. The list only shrinks: a new untagged row fails, and a line whose row is now tagged fails until
the line is removed. To tag a row, write the tag in its column; do not add a line to the list for new
work.

The check runs in `checks.sh fast` and `full` (step `evidence`), and `selfcheck.sh` proves it fails on an
untagged coverage row and an untagged deviation row.

## Test rules

1. **Test and audit code never changes the normal path.** Anything that exists to check, trace, probe or
   audit (harness hooks, probes, dumps, shot and flow drivers, counters) is off in a normal run and is
   reached only from its own switch or build. With it switched off, the game's frames, sounds,
   simulation state and timing are byte-identical to a build without it, and it adds no work to a
   frame. A check that needs a different path than the player's (a substitute screen, a stubbed
   device, a pinned clock) must say so and must not be taken as proof of the normal one.
2. **A visual change comes with same-pose before and after shots.** The shots are taken from the same
   start, the same camera pose, the same time of day and weather, the same resolution and settings,
   once on the base and once on the change, and they are shown side by side with the pixel difference.
   A change that should not alter the picture shows byte-identical shots. Shots of different poses or
   different scenes do not count, however similar they look.

Both rules are part of "How a change is checked" in [CONTRIBUTING.md](../../CONTRIBUTING.md); the checks
that carry them out are in [checks.md](checks.md).
