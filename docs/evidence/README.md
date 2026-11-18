# Evidence

The measurements behind claims in the docs: one folder per topic, named after the doc or the piece of work that
cites it (`docs/evidence/<topic>/`). Each file here is cited by a doc, a test or a tool; a doc names the file (or
the folder) next to the claim it supports.

What the files are:

* `README.md` / `README.txt`: what the folder holds, how it was recorded and which doc reads it.
* `*.txt`, `*.tsv`, `*.md`: final tables and measurements (frame times, levels, positions, counts), the
  original's side recorded under Wine with memory reads and Overbright's side with the developer harness.
* `*.py`, `*.sh`: the scripts that made a table, kept where a doc tells the reader to rerun them.
* `SHA256SUMS`: checksums of the captures a test replays.

Captures of the original game (its memory, traces, screenshots) and other raw recordings are not published:
a test that needs one reads it through the test kit's private set and is reported as ignored without it
(docs/testing.md). A doc that cites such a file calls it internal evidence (or the maintainer's private
captures, or kept with the maintainer's private evidence) without a path; a command that runs one names the
script by its file name only. The same holds for the maintainer's probes and tools (`devprobes/`, `tools/`),
which are private too: prose names them by file name only ("the maintainer's probe `pad_probe`", "a private
developer probe (`pad_probe`)", "the private tool `parity.py`").
