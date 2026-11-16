# Provenance tags (2026-11-05)

Not native: review tooling. The docs say which values are **verified**
(traced), **inferred** or **open** (docs/formats.md, top); until now that
was prose only, so a code site resting on an inferred value was invisible
at review time. Now such a site carries a machine-checked tag:

```
// prov: inferred ai.traffic.LANE_MARGIN
// prov: open    game.ISP_TRY_MS
// prov: choice  <id>       (a deliberate clone choice with no native value)
```

(the comment's first word `prov:`, a kind, an id `module.item[-n]`), and
every id is a row of `docs/provenance.tsv`: `id <TAB> kind <TAB> doc[,doc]
<TAB> summary`. `scripts/checks/provenance.py [repo]` fails on a tag whose
id is not registered, a registered id without a tag in `src/`, a kind that
differs from the row, duplicate rows, malformed tags or rows, a row's doc
that does not exist. `--list` prints every tag and its site,
`--untagged` the `**inferred**` / `**open**` comment lines that have no tag
yet. It runs in `scripts/checks/checks.sh` (fast and full: step
`provenance`).

When a value gets traced: delete its tag and its row (the check then
passes again); when a new inferred value is added: tag it and add a row.

## Seeding (2026-11-05)

Seeded from the docs' inferred / open items that have a findable code
site: every `**inferred**` / `**open**` comment line in `src/` whose comment
block names an address (0xNNNNN…) or an item (the const / fn / struct
after it) that a docs/*.md passage marked inferred / open names too (same
or ±2 lines). Those sites were tagged (the tag on its own line above the
marked comment line, so the existing lines are untouched) and registered
with the matched docs (formats.md first). The counts are in the checker's
output line (`provenance: N ids (I inferred, O open, 0 choice), S tagged
sites, U untagged …`); the untagged lines are inferred / open notes whose
doc passage the seeding could not match by address or name — candidates
for the next pass, listed by `--untagged`.

The summaries are the comment lines as they were; the tags assert nothing
new (they mirror what the code and the docs already say).
