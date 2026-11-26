# Asset format: the importer

Status: **draft 1 — stage 0 implemented** (`crates/ob-asset-format/src/import.rs`, the console program
`overbright-import`; §13), decoding units not yet. The game reads through a finished, verified base set by default (§13, "Reading through the asset layer"). Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md)
apply. Schema of the import record: [`schemas/import.schema.json`](schemas/import.schema.json).

The importer turns the player's own installed copy of the game into a base set, once. It is the only part of
Overbright that reads the original game's file formats; everything else reads the base set.

Prior art taken: an edition database with per-file hashes and an order-independent identity (OpenGOAL's extractor,
https://github.com/open-goal/jak-project; OpenTTD's base-set files with per-file checksums,
https://github.com/OpenTTD/OpenTTD/blob/master/docs/obg_format.txt; OpenRA's `IDFiles`,
https://github.com/OpenRA/OpenRA); the extracted archive stamped with the source edition and the importer version and
regenerated after an update (Ship of Harkinian, https://github.com/HarbourMasters/Shipwright/blob/develop/docs/VERSIONING.md);
provenance per item (OpenRCT2's `originalId`, https://github.com/OpenRCT2/objects).

## 1. Principles

- **Read only.** The importer never writes, renames or locks anything in the game folder.
- **Local only.** Nothing read or written leaves the player's machine (§11).
- **All or nothing.** A base set appears only when the whole import passed its checks (§5); an interrupted import
  resumes.
- **Verified.** Every item is proven equal to what Overbright's parser of the original gives (§5,
  [parity.md](parity.md)), and every original byte is either decoded or kept ([residual.md](residual.md)).
- **Once.** Afterwards the original folder is not needed (§4.2); the launcher says so, and keeps working if it is gone.

## 2. Input

The game folder the launcher already knows (found by `qol::setup::candidates` or chosen by the player,
`game-root.txt`, docs/playing/platform-dirs.md). Names are found without regard to case, as today (`crates/ob-core/src/game_fs.rs`).

The importer reads the **content folders** (`CARS`, `FRONTEND`, `GLOBAL`, `LANGUAGES`, `MOVIES`, `NIS`, `SDATA`,
`SOUND`, `TRACKS`, `CREDITS`, `SUBTITLES`, `memcard`) and skips everything else ([residual.md](residual.md) §5).
`SPEED2.EXE` is fingerprinted for the record only, as `crates/ob-core/src/fingerprint.rs` does today, and never read for content.

## 3. Editions

The importer knows **editions**: for each, a key, a name, the region, and the complete content-file manifest
(path, size, SHA-256 of every content file), plus an order-independent identity: the SHA-256 of the sorted list of
`<path>\t<size>\t<sha256>` lines, with paths upper-cased (case differs between installs).

| Key | Edition | State |
|---|---|---|
| `pc12-eu` | PC 1.2, European release (region 6) | known: the reference install; all content files' hashes recorded (today `crates/ob-core/src/fingerprint.rs` records nine) |
| `pc12-na` | PC 1.2, North American release (region 0) | expected (its `CARS` folder differs, crates/ob-core/src/region.rs); hashes to be contributed [open] |
| `custom` | anything else: another release, a patched or modded install | imported as found; no parity promise |

- The base set's id is `nfsu2-<key>` (`nfsu2-pc12-eu`); a `custom` import is `nfsu2-custom-<first 8 hex digits of
  its identity>`.
- A **custom** import runs the same decoders. Files a decoder refuses are kept whole in the residual. The launcher
  says plainly: "Your game files differ from the versions Overbright was checked against; it will run, but may not
  match the original (choice: format rule; the original's layout is in docs/formats/) exactly." It lists the differing files (`MODIFIED or unknown`, as `--check-game` does today).
- New editions are added to the table from a manifest of hashes (made by `overbright-studio-cli base fingerprint
  --manifest`; it holds names, sizes and hashes only, no game data) and a full import and parity run on that
  edition.

## 4. Import units

The work is cut into **import units**, normally one per original file (a region's `STREAM` file, a car folder, a
sound bank group), each with its own importer version. Units run in parallel, each:

1. reads its original file(s) (memory-mapped, as the parsers do today);
2. unwraps containers (JDLZ, HUFF, BIG4) and walks the chunk tree;
3. decodes with Overbright's existing parsers into the in-memory model the engine uses today;
4. hashes the model (**Eq1 reference hash**: the cache's flat encoding of the model, `crates/ob-core/src/cache/persist.rs`, hashed
   with XXH3-128);
5. writes the source-form files of its items into the staging bundles (the kind pages' import sections give the
   mapping);
6. records every byte's fate in the residual map, runs the kind's encoder over decoded nodes where one exists
   (**Eq2**), keeps bytes where it does not ([residual.md](residual.md) §3);
7. appends its result to the staging journal.

Two refinements:

- **One file, several units.** A chunk can belong to another unit than the rest of its file (the movie table in
  `GLOBALB.BUN` belongs to the `movies` unit). Each residual node belongs to exactly one unit; the file's map is
  written once, after every unit that owns one of its nodes has finished.
- **Identical content once.** Files or chunks whose unwrapped content is identical (`InGameCommon.bun` and the
  unpacked `InGameCommon.lzc`; `GLOBALB.BUN` and the unpacked `GlobalB.lzc` where they agree; the trigger tables
  repeated in every route file of a region) are imported once; each file's map points at the same items
  ([residual.md](residual.md) §3.1.1). Where an edition's copies differ, each is imported on its own and the kind
  page says which copy the engine reads for which use.

### 4.1 Stage 0 and graduation

Decoding every kind before anything reads the base set would put ~40 parity proofs in front of the first import. So
the importer works in **stages**, per kind, recorded per unit in `import.toml` (`state`):

- **`kept`** (stage 0): the unit's original files are in the residual as **whole-file `kept` nodes**, byte for
  byte, with their SHA-256, stored (not deflated) and aligned inside the unit's own bundle
  ([residual.md](residual.md) §3, [layout.md](layout.md) §5), so a load opens one file at every stage. Reading them through the asset layer gives exactly the
  original bytes, so the engine's existing parser for that kind runs on them unchanged: equality is by
  construction (the bytes are the original's), and no parity study is needed. A stage-0 import is a copy and a
  digest, bound by disk writes: under a minute on an SSD, about half a minute to two minutes on a hard disk or an SD
  card [guess; [performance.md](performance.md) §5].
- **`decoded`** (graduated): the unit's items are in the source form of their kind page, proven by Eq1 against the
  parser's result on the kept bytes, and the kept bytes become the residual's usual nodes (decoded, patched, kept,
  shadow).

A kind **graduates** through an import epoch ([versioning.md](versioning.md) §4.2): the new importer decodes the
kind from the kept bytes in the base set (never from the game folder), proves Eq1, and only then rewrites the unit; a
failed proof leaves the unit `kept` with a log line, never mixed data. While any kind is `kept`, the game still
contains that kind's original parser and runs it on bytes from the base set; when the last kind graduates, the
parsers leave the game and live only in the importer ([parity.md](parity.md) §3).

**Graduation is on demand.** A kind graduates when a tool edits it or a base set needs it (each graduation is costed
with that tool), not all kinds in one block. Until then the kind stays `kept`: the original's own encoding inside
Overbright's own bundles, read by the original's parser in the game. The goal is that **every kind graduates**; every
kind must have graduated before engine-level work that depends on it (baked data, new regions, community base sets),
since only a graduated kind can be replaced by content that is not the original's. A graduation rewrites only the
bundles of the units it touches (≈ 0.3 GB at most, so about 0.6 GB of free space while it runs).

### 4.2 Storage policies: the folder is not needed after the import

| Policy | What the base set holds | The game folder afterwards | Who makes it |
|---|---|---|---|
| **owned** (`[base] storage = "owned"`, the default) | everything: decoded items and kept bytes, in the base set's own files | **not needed**: it may be moved, patched or deleted | the launcher's import, for every player |
| **linked** (`[base] storage = "linked"`) | decoded items as above; files still at stage 0 are **references** to the game folder (`fate = "external"`: path, size, SHA-256) instead of copies | must stay unchanged; a changed, moved or missing file makes the set *changed since import* | developer builds, CI and test sandboxes only (`overbright-import --link`), to avoid a 1.8 GB copy per sandbox; never offered to players, not even when disk space is short (§10) |

- A release build never writes a linked set; when it finds one it refuses to mount it (`IMP.7`) and offers
  "Make independent" (copies the referenced files in; seconds on an SSD). So for players the promise holds: after
  the import the original folder is not needed.
- Linked references are checked like bundles ([layout.md](layout.md) §5): size, time and inode at each start, a
  background re-digest of changed files; a mismatch marks the set changed since import and the developer harness
  stops with the file named.
- A graduated kind never references the folder, in either policy.
- **The folder changes after the import.** For an owned set the folder no longer matters, and the launcher ignores
  in-place patchers and other tools writing into it. When it notices that the folder changed (size, time, inode of
  its files), it says so once and offers to bring the change in as a mod or to ignore it.

## 5. Steps

| Step | Work | Reference desktop [guess] |
|---|---|---|
| 1. Scan | List the content folders; sizes; free-space check (§10) | < 1 s |
| 2. Fingerprint | SHA-256 of every content file (parallel); edition lookup | 2–4 s (1.7 GB; disk-bound on a hard disk: ~15 s) |
| 3. Plan | Import units, their versions, what the journal already has (resume) | < 1 s |
| 4. Convert | §4 for every unit, in parallel | the kind pages' timings; sum in [parity.md](parity.md) §4 |
| 5. Verify | Units at stage 0 (`kept`): read every kept file back and compare its SHA-256. Decoded units: read every unit back **through the engine's reader** (the code the game uses), hash the model, compare with Eq1, and check every residual map's rebuild digest. Then check the required-items profile ([manifest.md](manifest.md) §5) | about the conversion's time again |
| 6. Finish | Bundle digests; `import.toml` written last; the staging folder renamed into place; `active.txt` set if it is the first base set | < 1 s |
| 7. Cook (required) | Cook the global and front-end units and the first region from the base set, so the first start after the import never cooks on the load path ([performance.md](performance.md) §3.3); then the existing Prepare step's shader work (docs/developing/prepare.md), optional as today. Afterwards the launcher cooks the remaining regions and the profile's cars in the background (pausable, as below) | a few seconds [guess] |

- **Play never waits for the import.** Until the base set has been renamed into place (step 6), the launcher
  starts the game without `--base`, and the game reads the game folder exactly as it does without the format. The
  import runs with progress and resumes after a stop.
- **Background work pauses for the game.** The import, the background cooks and verification lower their I/O
  priority where the system allows it, and also **pause themselves while the game loads or streams**: I/O priority
  alone did not prevent a measured 13 % slower cold load on a fast SSD.
- **Staging.** Everything is written under `<app>/base/.staging-<id>/`. Step 6 renames it to `<app>/base/<id>/`. A
  crash or Stop leaves the staging folder and its journal; the next run continues with the units not yet done
  (each unit's output is complete or absent, as cache writes are today).
- **Failure.** A unit that fails Eq1 (the read-back model differs) stops the import with the unit, the item and both
  hashes; no base set is made. This is an Overbright bug, never the player's fault, and the message says so.
- **Re-running** on the same folder with a complete base set present does nothing unless asked (`--force`), or the
  epoch rules of [versioning.md](versioning.md) §4.2 apply.

## 6. The import record (`import.toml`, `ob.import/1.0`)

```toml
schema = "ob.import/1.0"

[importer]
overbright = "0.5.0"
epoch = 1
finished = "2026-11-18T12:00:00Z"

[source]
edition = "pc12-eu"
verdict = "known"                     # known | custom
identity = "sha256:…"                 # §3
file_count = 1250
exe = { size = 4788224, sha256 = "sha256:8f6a9ae2…" }   # for the record only

[[source.file]]
path = "GLOBAL/GlobalB.lzc"
size = 5145778
sha256 = "sha256:ad015fdb…"
unit = "global"

[[source.skipped]]
path = "dinput8.dll"
reason = "not game content"

[[bundle]]
name = "global.obz"
size = 41234432
digest = "xxh3-128:…"

[[unit]]
name = "world-L4RA"
version = 1
state = "decoded"                     # kept | decoded (§4.1)
files = ["TRACKS/L4RA.BUN", "TRACKS/STREAML4RA.BUN"]
items = 41870
model = "xxh3-128:…"                  # the Eq1 reference hash
proof = { bytes = 1203, patched = 2, values = 88, kept = 12, copy = 0, fill = 640, external = 0 }   # residual nodes

[checks]
read_back = "pass"
rebuild = "pass"
profile = "pass"

[residual]
kept_bytes = 1048576
shadow_bytes = 41943040
```

| Field | Type | Req. | Meaning |
|---|---|---|---|
| `importer.overbright` | semver | yes | The release that made (or last re-derived) the set. |
| `importer.epoch` | u32 | yes | The import epoch ([versioning.md](versioning.md) §1). |
| `importer.finished` | date | yes | When step 6 ran. |
| `source.edition`, `verdict`, `identity`, `file_count` | | yes | §3. |
| `source.exe` | table | no | Size and SHA-256 of `SPEED2.EXE` if present: reports only. |
| `source.file[]` | table | yes | Every content file read: `path` (original spelling), `size`, `sha256`, `unit`. |
| `source.skipped[]` | table | no | Files not read, with the reason. |
| `bundle[]` | table | yes | Every bundle: `name`, `size`, `digest` ([layout.md](layout.md) §5). |
| `unit[]` | table | yes | Every import unit: `name`, `version`, `state` (`kept` or `decoded`, §4.1), `files`, `items`, `model` (Eq1 hash; for a `kept` unit the digest of its files), `proof`: counts of its residual nodes by outcome (`bytes`, `patched`, `values` for decoded nodes; `kept`, `copy`, `fill`, `external` for the others). |
| `checks` | table | yes | `read_back`, `rebuild`, `profile`: `pass` (a base set is only written when all pass). |
| `residual` | table | yes | Kept and shadow byte totals. |
| `journal[]` | table | no | Present only during an on-disk migration ([versioning.md](versioning.md) §4.1): per bundle `bundle` (name), `state` (`pending`, `written`, `done`), `from_schema`, `to_schema` (schema ids, optional). |

The record names no folder of the player's machine. The launcher keeps the game folder's path where it keeps it today
(`game-root.txt`).

## 7. Updates and re-derivation

After an Overbright update the launcher compares the base set's `importer.epoch` and unit versions with the new
importer's ([versioning.md](versioning.md) §4.2). Units at stage 0 graduate from their kept bytes (§4.1); for units
already decoded: changed units are re-imported from the original folder when its
identity still matches, otherwise re-derived from the base set itself through the rebuild invariant
([residual.md](residual.md) §4). A base set made by a newer Overbright than the one running is refused (`VER.4`).

## 8. Damage and repair

A bundle whose digest no longer matches (`LAY.3`) makes the base set **damaged**; the game does not start on it.
Repair, in order:

1. Re-import the damaged units from the original folder (if its identity matches).
2. Otherwise, re-import them from any folder holding the same edition (the launcher asks for one).
3. Otherwise the units cannot be restored: decoded items with a byte proof are kept only in their bundle. The
   launcher says which part of the game is affected and offers a fresh import.

Players who want to delete the original folder are told once that the base set is then their only copy and is worth
backing up like any other personal data (it is a plain folder).

## 9. Programs and progress protocol

- **`overbright-import`**: a console program that links the original's parsers (once every kind has graduated, the game itself no longer does). The
  launcher runs it as a child, as it runs `game --prepare` today, and reads its lines.
- **`overbright-studio-cli base <verb>`**: `import`, `verify` (re-check every bundle and the rebuild invariant),
  `ls` (the logical tree), `export <path|kind> --to <dir>` (source files as open files on disk, e.g. glTF and PNG,
  for outside tools; marked local-only), `fingerprint [--manifest]`, `check` (profile and rules; for community sets),
  `remove`. JSON output and exit codes as the CLI defines them (the command-line tool's documentation).
- **MCP**: base sets are readable resources; `base import` is a *run* command that asks for confirmation; there is
  no MCP tool that exports base-set files.

Progress lines on standard output (`key=value`; other lines are ignored), in the style of the prepare step:

```text
import=1
edition=pc12-eu known
space_needed=2600000000
step=2/7 fingerprint
progress=231/1250 files
step=4/7 convert
unit=world-L4RA done
progress=37/96 units
bytes=812000000/1700000000
eta_s=48
ok=1                       (or: error=<one line>, ok=0, exit code 1)
```

## 10. Disk space

The importer checks free space before step 4. When it is short, **the import stops** and says how much space is needed
and where ("needs 2.6 GB, 1.1 GB free on D:"); nothing is written. There is no fallback that keeps the player's data in
the game folder (a linked set, §4.2, is for developers only); the player frees space or moves the `base` location. The
need is the base set plus the staging overhead (none: staging is the final folder until the rename) plus the cache's
first cook. Per-kind sizes are on the kind pages; the totals are in [parity.md](parity.md) §4. The `base` location
can be moved to another drive (`locations.txt`, docs/playing/platform-dirs.md).

## 11. Legal and privacy notes

- The base set is a converted copy of the player's own installed game, for that player's use on their own machine.
  Overbright never ships, uploads, mirrors or downloads game data; the importer has no network access.
- A base set's manifest says `origin = "imported"`, `share = "local-only"`; Studio and the CLI refuse to export it,
  its residual or any pack derived from it as shareable ([manifest.md](manifest.md) §3, [layering.md](layering.md) §5).
- Support files and crash reports carry the import record's summary (edition, verdict, epoch, checks), never base-set
  content, file lists with the player's paths, or residual bytes.
- In a browser, the import runs inside the page in a worker and writes to the origin's private storage; the files
  are never sent to any server ([cooked.md](cooked.md) §7).
- In a browser, the base set is a **rebuildable copy, never the player's only copy**: the page asks for persistent
  storage (`navigator.storage.persist()`) before importing and says plainly that the browser may still delete its
  data; when it does, the recovery is to import again from the game folder, and "the original folder is no longer
  needed" is never shown there.

## 12. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| IMP.1 | The folder has no content folders (not a game folder), or a required file is missing (named). |
| IMP.2 | Not enough free space (needed, free, where). |
| IMP.3 | A unit's read-back model differs from the original's (unit, item, both hashes): an Overbright bug; nothing is written. |
| IMP.4 | A residual map's rebuild digest differs: an Overbright bug; nothing is written. |
| IMP.5 | The required-items profile is incomplete (a `custom` install missing files). |
| IMP.6 | The staging folder belongs to another edition or importer version than this run; it is discarded and the import starts over. |
| IMP.7 | A release build found a `linked` base set (it offers to make it independent), or a linked set's referenced file is missing or changed. |
<!-- /generated -->

## 13. What is implemented (stage 0)

`overbright-import import|verify|repair|fingerprint [--game <dir>] [--base <dir>] [--id <id>] [--force] [--manifest]`
(`crates/ob-asset-format/src/bin/overbright-import.rs`; `--game` defaults to `NFSU2_ROOT`, `--base` to `<app>/base`).
It prints the progress lines of §9 and ends with `ok=1`, or `error=<one line>` and `ok=0` (exit code 1).

- **Editions.** `pc12-eu` with all 1,250 content files (`crates/ob-asset-format/editions/pc12-eu.tsv`: path, size,
  SHA-256; made by `overbright-import fingerprint --manifest`, names and hashes only). Anything else is `custom`
  with its differing files (`MODIFIED`, `unknown`, `missing`) against the closest edition.
- **Units.** Every unit is at stage 0 (`kept`): each content file is one stored, aligned entry
  `residual/<ORIGINAL/PATH>.kept.bin` plus its file map `residual/<ORIGINAL/PATH>.map.toml` (`container = "plain"`,
  one `kept` node), in the bundle of its load unit (layout.md §5). `SDATA/sdat.viv` is read by two audio units and
  so goes in `global.obz`. Stored entries are aligned to 64 bytes, the bundle writer's alignment (the 256 of
  residual.md §2 is open).
- **Journal and resume.** `base/.staging-<id>/journal.txt` starts with the source identity, the epoch and the unit
  version; each finished bundle (written to a temporary name, flushed, renamed) appends `unit <name> <size>
  <digest>`. A rerun keeps the units whose bundles are still there at their size and writes the rest; a staging
  folder of another identity or version is discarded (`IMP.6`).
- **Checks.** Before writing: free space for the units still to do (`IMP.2`). After: every bundle re-read, its size
  and digest against the journal, every CRC, every kept file's SHA-256 against the original's (the stage-0
  rebuild check, `IMP.4`). `pack.toml` is checked by the manifest reader; `import.toml` is written last, then the
  staging folder is renamed into place and `active.txt` written if there is none.
- **Verify and repair.** `verify` re-checks a finished set (`LAY.3` per damaged unit); `repair` rebuilds the damaged
  bundles from the game folder when its identity still matches the record (the writer is deterministic, so the
  rebuilt bundle equals the recorded one).
- **Read only.** The game folder is opened for reading only; `SPEED2.EXE` is hashed for the record only. The
  reference install imports in about 50 s from and to an NVMe SSD (1.80 GB).
- Not yet: the launcher screen, the cook step (§5 step 7), decoding units (§4.1), linked sets (§4.2), pausing
  for the game.

### Reading through the asset layer

- **On by default when an import exists.** At start the game mounts the active base set (`<app>/base/active.txt`,
  else the only set there; `src/asset_layer.rs`, `crates/ob-asset-format/src/mount.rs`) and from then on takes the
  game's files from it: one read path (`crates/ob-core/src/game_fs/layer.rs`) behind `game_fs::read`,
  `game_fs::read_file`, `game_fs::open_file` / `open_reader` and the cache's `SourceBytes`, answered with a slice
  of the mapped bundle (stored, aligned: no copy). A name in a content folder (§2) that the set does not hold,
  and that the import's scan did not skip (`[[source.skipped]]`), was not in the install: the read is answered
  "no such file" from the set's index without asking the folder (the original asks for some such names and goes
  on without them: 0x432C00 builds `NIS\Scene_%s_EvtTrk.txt` for every scene and skips the track when the file
  is missing, and the reference install has no `Scene_EndingNis04_EvtTrk.txt`). Any other name is read from the
  game folder and counted.
  Without a set, or when it does not verify, nothing is mounted and every read is the game folder's, exactly as
  before. Every kind is still at stage 0, so the parsers are today's and see the original's bytes.
- **Verification on mount** (layout.md §5): `import.toml` read, its checks passed, its epoch not newer than this
  Overbright (`VER.4`); every bundle present at its recorded size; each bundle's `xxh3-128` digest compared with
  the record unless its size, modification time and inode match the stamp of an earlier verification
  (`base-verified-<id>.txt` in the cache folder); every kept file a stored, aligned entry of its recorded size
  (`LAY.3` otherwise). About 0.1 s for the reference install with the bundles in the page cache; a stamped
  mount ~20 ms. A refused set is logged (`asset layer: …; reading the game folder`) and the game folder used.
- **Files changed in the folder after the import** (a mod that replaces a file, such as an HD sky's
  `TRACKS/LOC4DYNTEX.BIN`): the set keeps serving its own copy; the mount compares every file's size with the
  folder's and logs the changed ones (`asset layer: … changed in the game folder since your own copy was made`).
- **`NFSU2_BASE`**: unset or `active` = the default above; `off` (or `0`) = the game folder always; a set id under
  `<app>/base` or a set's folder = that set. At exit a log line counts the reads from the set, the names the
  install does not have, and the game files read from the folder (each such name listed after it,
  `asset layer: not in base set: <NAME> (<n>x)`).
- **Sealed check**: `scripts/checks/sealed_copy.sh GAME SET OUT` runs the full playthrough twice, on the folder
  (`NFSU2_BASE=off`) and on the set with the game folder replaced, in a user + mount namespace, by a skeleton of
  unreadable sparse files (names, sizes and times kept), without capabilities: both runs pass, their state hashes
  are identical and the sealed run reads 0 game files from the folder. On the reference install: 22,639 reads
  from the set, 151 names not in the install, 0 from the folder, 308,107 steps identical.
- **Not yet**: the game folder is still the namespace: directory listings, existence checks and the cache's
  source keys look at it, so it must still be there; a lint on stray opens; read-ahead of bundles; the
  background re-digest; the launcher's switch.
- **The gate**: `ASSET_LAYER=1 scripts/checks/gate.sh …` imports `NFSU2_ROOT` into the gate's folder and runs the
  parity with the new binary on that set (the base binary reads the folder): every hash must match, and every new
  run must report reads from the set and none may read a game file from the folder (the names are listed). Without it the parity's runs have no base set (their data folders are
  fresh), so they read the game folder as before.
