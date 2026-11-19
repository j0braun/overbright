# Asset format: performance (budgets, how the cooked form meets them, the gate)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md). Normative where it says MUST.

Every player reads this format at every start and every load; few people author it. **Where player-facing
performance and authoring convenience conflict, performance wins**, and convenience is provided by tools (Studio,
`overbright-studio-cli base export`) rather than by the stored form. The choices made on that basis are listed in §7.

Numbers below are medians of at least 5 runs on the **reference desktop** (an 8-core / 16-thread desktop CPU,
NVMe SSD, ext4, Linux; discrete GPU) unless stated; "4c" pins the process to 4 cores of the same CPU, which is not a
laptop. Today's numbers come from docs/notes/performance/budget.md and the measurements behind it; prototype numbers come from
stand-alone benchmarks on copies of the reference install's data. Guesses are marked.

## 1. Today's baseline (what the format must not lose)

| What | Today | Source |
|---|---|---|
| Start → first frame / title, warm | 65–74 / 81–125 ms (discrete GPU); 69 / 85 ms (integrated GPU) | docs/notes/performance/budget.md |
| Start → driving, warm, fastest path | 192–209 ms (discrete) / 328–349 ms (integrated) | measured |
| Menu → free roam, warm | 60–131 ms (world preloaded in the menus) | docs/notes/performance/budget.md |
| Free-roam load stages, warm | `World::load` 22–23 ms (the mapped 139 MB track cache, nothing parsed); `Car::load_tuned` 20–21 ms; `Game::new` 22–23 ms; `WorldRenderer::new` 73–177 ms (mostly texture upload) | docs/notes/performance/budget.md |
| Races, warm | menu → Quick Race 134–487 ms; free roam ↔ race 20–118 ms | docs/notes/performance/budget.md |
| 4 cores | first free-roam load 150 ms; menu → free roam 101; Quick Race 110; free roam → race 27; no faster beyond 4 cores | measured |
| Cold OS cache (SSD) | start → first frame 238 ms, → title 340; first free-roam load 375 ms (track cache 110 ms) | docs/notes/performance/budget.md |
| Empty asset cache | first free-roam load 0.5–1.9 s; 0.40–0.43 s after the Prepare step | docs/notes/performance/budget.md, docs/developing/prepare.md |
| Region parse from the original files (cache miss) | 0.69 s | docs/notes/performance/budget.md |
| Streaming hitches while driving | none: the region stays loaded; worst frame CPU 4.7–5.2 ms | measured |
| The 200 ms rule | a loading screen shows only after 200 ms; every wait is under it on the discrete GPU at every level and on the integrated GPU at Original and Low | docs/original/loading-image.md, docs/notes/performance/budget.md |
| Memory | resident 0.6–0.7 GB in benches, 1.3 GB shipping game, 2.2 GB on a first start, 2.1–2.7 GB with a texture pack; video memory 0.6 GB (1080p Original) to 3.2 GB (4K, every option) | docs/notes/performance/budget.md |
| Disk | the original's content folders 1.80 GB; asset cache ≤ 2 GB (limit), 122–139 MB per region track file | docs/developing/asset-cache.md |
| Not measured yet | hard disks, SATA SSDs, Windows, Steam Deck | – |

## 2. Budgets (normative)

**Rule: non-inferiority.** With a base set of a known edition, every measure in this table MUST be at least as good as
today's on the same machine, within the margins of §6. Where today has no number, today's build is measured first on
that hardware and becomes the bound before any kind moves.

| Measure | Reference desktop | Low end: integrated GPU, 4 cores | SATA SSD, hard disk, Steam Deck-class | Web build |
|---|---|---|---|---|
| Mount the stack (stat every bundle, read central directories) | ≤ 2 ms | ≤ 5 ms [guess] | measure, then bound | ≤ 50 ms [guess] |
| Unit open on a cooked hit: region / car / `Game::new` | ≤ 23 / 21 / 23 ms (warm) | ≤ today's 4c values | measure, then bound | region ≤ 1 s from browser storage [guess] |
| Start → title, warm / cold | ≤ today (§1) | ≤ today | measure, then bound | – |
| Menu → free roam, Quick Race, free roam ↔ race | ≤ today, and every wait under 200 ms where it is today | ≤ today (150 / 101 / 110 / 27 ms at 4c) | measure, then bound | – |
| Cook on a cache miss, one region | ≤ 0.69 s (today's parse) | ≤ today's parse there | – | – |
| Streaming hitch while driving | none new: no load work on the frame thread | same | same | same |
| Peak resident memory | ≤ today + 2 % | same | same | fits 4 GiB of WebAssembly memory |
| Disk | base set ≤ 1.25 × the original folder; asset cache within its limit | same | same | browser storage quota |
| First start after import | ≤ today's first load after Prepare (0.40–0.43 s): the import's cook step has cooked the units it needs ([import.md](import.md) §5) | ≤ today | – | – |
| First start after an update | ≤ today's first start after an update (migrations in memory, then in the background; [versioning.md](versioning.md) §4.1) | ≤ today | – | – |

A **two-core** configuration (the low end with half the cores) is bounded the same way: today's build is measured
first, and its numbers become the bound.

## 3. How the cooked form meets them

### 3.1 A cache hit parses nothing and copies nothing

A model entry is flat arrays in the original record layouts or other fixed layouts, cast in place
([cooked.md](cooked.md) §2); large payloads are external references into stored bundle entries (§5.1 there).
Measured on 139 MB of 32-byte records: casting the mapped file costs 0.01 ms, building an owned `Vec` costs 14–36 ms
— today's whole `World::load` budget. So readers MUST NOT decode, parse or copy anything on a hit beyond what today's
cache does.

### 3.2 Compression only where it pays

Decode speed, single core, of payloads cut from the reference install (ratio · GB/s); memcpy for comparison runs at
tens of GB/s:

| Payload | zstd-3 | zstd-19 | deflate-6 (zlib-rs) | LZ-family fast codec (lz4) |
|---|---|---|---|---|
| World texture blocks (45 MB) | 0.67 · 1.3 | 0.63 · 0.9 | 0.67 · 0.6 | 0.82 · 8.7 |
| World geometry (43 MB) | 0.34 · 1.3 | 0.29 · 1.0 | 0.37 · 0.8 | 0.51 · 3.9 |
| Car textures, decoded (67 MB) | 0.07 · 3.7 | 0.06 · 5.0 | 0.07 · 3.2 | 0.11 · 7.3 |
| TOML text (11.6 MB) | 0.25 · 1.5 | 0.20 · 2.3 | 0.23 · 1.2 | 0.40 · 4.8 |

Today's decoders: JDLZ 1.9 GB/s, HUFF 0.1–3.5 GB/s; one car's textures (8.8 MB) 5.6 ms on one core.

- **World payloads are stored** (texture blocks, vertex and index arrays, collision and route arrays): compressing them
  with zstd-3 would add about 35 + 33 ms of decoding to every warm region load on one core (about 10 + 10 ms on four)
  to save about 30–75 ms only on a cold NVMe read, which the read-ahead (§3.4) already hides.
- **Texture images that the original kept compressed** (vinyls, car textures, the HUD, magazines) are KTX 2.0 with
  Zstandard supercompression ([textures.md](textures.md)): 0.73–0.82× of the original's JDLZ/HUFF size, and faster
  to decode than today's decoders (about 2.4 ms per car instead of 5.6 ms on one core). Decoding runs on the load's
  worker threads, per mip level.
- **Text is deflated** in bundles: it is read only when cooking (§3.3), where inflating 11.6 MB of TOML costs 12.7 ms
  next to 180 ms of parsing.
- **Audio and movies are stored** and read in place (passthrough bitstreams are already compressed).

### 3.3 Text is parsed only when cooking, and cooking is never inside a load

Measured, one core: typed TOML parsing 64 MB/s (3.8 µs per placement record); a 48,000-record `placements.toml` 180
ms; glTF JSON 600 MB/s. A region's text cooks in a fraction of today's 0.69 s parse, but a load must not pay it:

- The importer and the Prepare step cook the global and front-end units and the first region, so the first start
  after an import never cooks on the load path ([import.md](import.md) §5).
- A mod stack is cooked when it is mounted (the launcher, before the game starts), never inside a load. A change to a
  folder-form pack being edited re-cooks only the units that read the changed files ([cooked.md](cooked.md) §3).
- Cooking runs in parallel per file. The game's readers use typed deserialisation; `toml_edit` (which keeps
  formatting, 34 MB/s) is for Studio's writers only. Both read floats by the same rule
  ([conventions.md](conventions.md) §3.4).

### 3.4 Read in the order the load needs, with parallel readers

Cold reads are decided by contiguity and parallelism, not by the number of files. Measured on NVMe, 41 MB in 1,000
pieces: contiguous in one file 108 ms, the same pieces at random places in one file 412 ms, as separate files 365
ms; 139 MB read by one reader 366 ms, by four readers 163 ms. Therefore:

- **Entry order** inside a bundle follows the unit's load order (stream sections in the order the region loads them,
  then the kinds as the loader reads them), while the central directory stays sorted by logical path for binary
  search ([layout.md](layout.md) §4).
- Before a load, the reader issues an explicit **read-ahead** of every range the load will touch, merged and sorted
  by offset, with at least four parallel readers of 4 MiB pieces (today's cache integrity pass does this work for the
  track file and halves page faults). It MUST NOT rely on page faults in random order, and MUST NOT use
  `MADV_WILLNEED` / `MADV_SEQUENTIAL` on whole bundles (measured 2× slower cold).
- Loads use at least four worker threads; today's loads gain nothing beyond four cores.

### 3.5 Few files

Warm, per file on Linux: 7.6 µs to open, read and close (10,000 files: 97 ms on one thread; the same bytes from one
mapped bundle: 34–55 ms). The central directory of a 10,000-entry bundle parses in 0.07 ms. On Windows, Microsoft
Defender scans a file synchronously when it is opened
(https://learn.microsoft.com/en-us/defender-endpoint/microsoft-defender-endpoint-antivirus-performance-mode), and
closing written files has been measured at 1–10 ms each (https://gregoryszorc.com/blog/2021/04/06/surprisingly-slow/);
the per-open read cost is not published. So a player's base set is **one bundle per load unit**, and a game start
opens at most a few dozen files. Readers find entries by binary search on the sorted central directory, never by a
linear scan, and compute each entry's data offset from the central directory alone (writers write identical local
and central extra fields, [layout.md](layout.md) §4), so mounting never touches local headers.

### 3.6 Verification costs nothing on a normal start

XXH3-128 hashes 34 GB/s per core; verification is bound by the disk (≈ 0.11 s per 100 MB cold with four readers on
the reference NVMe; the whole 1.7 GB base set ≈ 1.9 s there, ≈ 3.4 s on a SATA SSD and ≈ 14 s on a hard disk
[estimates]). Bundles are therefore hashed only at import and when a bundle's size, time or inode changed
([layout.md](layout.md) §5), in the background, never on the load path. Background work (verification, warm-up
cooks, imports, migrations) pauses itself while the game loads or streams: lowering its I/O priority alone did not
help on a fast SSD's default scheduler, where a background digest made a cold load 13 % slower. Idle I/O priority is
kept as a best-effort extra.

### 3.7 Invalidation is narrow

Model entries are keyed by the files actually read ([cooked.md](cooked.md) §3–4): an Overbright update that does not
change a model's types keeps its entries (today every update re-parses every region: 0.69 s each); editing one
texture of a pack re-cooks only the units that read it. Prepared entries (renderer data) are rebuilt per build as
today and by the Prepare step.

### 3.8 GPU-ready data and memory

Texture blocks are uploaded as stored (BC1/BC2/BC3 as the original has them); vertex and index arrays are uploaded
from the mapping. Mapped pages are file-backed and reclaimable, so mapping a bundle does not raise the committed
memory the way reading it into heap buffers would.

### 3.9 The web build

No memory mapping: bundles and cooked files live in the browser's private storage (OPFS) and are read with
synchronous access handles in a worker, range by range. A pure-Rust Zstandard decoder is 3.5–4.5× slower than
libzstd, which is another reason payloads are stored. Without the `texture-compression-bc` feature the cook decodes
BC blocks to RGBA8 once ([cooked.md](cooked.md) §1), at four times the video memory. The web budgets in §2 are
provisional until a web build exists.

## 4. Disk

| | Size |
|---|---|
| Original content folders (reference install) | 1.80 GB |
| Base set, all kinds graduated ([parity.md](parity.md) §4) | ≈ 1.7 GB [guess ±15 %] |
| Base set at stage 0 ([import.md](import.md) §4.1): every kept file stored and aligned, text deflated | ≈ 1.8 GB (1.0× the folder) |
| Asset cache | within its limit (2 GB default), as today; model entries are small because payloads are referenced |
| Budget | base set ≤ 1.25 × the original folder; the importer checks free space first ([import.md](import.md) §10) |

## 5. Import time

Fingerprinting is SHA-256 at 2 GB/s per core (≈ 2–4 s for 1.8 GB here); a stage-0 import is a copy plus digests
bound by disk writes (under a minute on an SSD, about half a minute to two minutes on a hard disk or an SD card;
measured 9–27 s on NVMe drives for the whole set); a fully graduated import adds decoding and the
read-back verification (≈ 1 min on the reference desktop, a few minutes on a hard disk [guess];
[parity.md](parity.md) §4).

## 6. The gate: no kind moves if it loads slower

Each kind's move to the format ([parity.md](parity.md) §3) passes this gate in addition to the equality gates.

- **Both sources in one binary.** The developer harness reads each kind either way (`--assets original|base`), and the
  two arms run interleaved, so machine noise affects both alike.
- **Benchmarks.** Existing: `--bench transitions` (menu first), `--bench boot`, `--bench explore` / `race`, the load
  set of the perf gates (`SET=loads`, at least 10 repetitions; load wall time varies by about 13 % between runs,
  docs/developing/perf-gates.md), per-stage spans from `NFSU2_TRACE` compared on thread CPU time. New: `--bench units` (open each
  kind's unit from a cooked hit to a ready model, warm, cold and on 4 cores), `--bench cook <unit>` (the miss path),
  a mount-and-verify bench, and the format micro-benchmarks (decode, parse, central directory).
- **Pass rule.** A one-sided non-inferiority test: the upper bound of the 95 % Hodges–Lehmann confidence interval of
  (new − old) over at least 10 runs per arm stays within the margin:

  | Measure | Margin |
  |---|---|
  | Warm stage, thread CPU | max(1 ms, 3 %) |
  | Warm transition, wall time | max(5 ms, 5 %) |
  | Cold load | max(20 ms, 5 %) |
  | Peak resident memory | max(20 MB, 2 %) |
  | Disk | the §4 budget |

- **Configurations** for every kind: discrete GPU at 1080p and 4K, integrated GPU, 4 cores, cold OS cache; a hard
  disk, a SATA SSD, Windows and a Steam Deck-class device as soon as the hardware is in the test set (their today
  numbers are measured first, §2).
- A kind that fails stays where it is (stage 0 or the original-reading path) until it passes; the gate's report names
  the stage that got slower.

## 7. Where performance won over convenience

| Choice | Convenience given up | Recovered by |
|---|---|---|
| One bundle per load unit instead of loose files | browsing the base set in a file manager | `base ls`, `base export`, Studio's asset browser; folder form for developers ([layout.md](layout.md) §3) |
| Payloads stored, not compressed | ≈ 30 % smaller world data on disk | – (disk stays within budget) |
| Large numeric data in `.bin` arrays with layout tables, not TOML | hand-editing collision faces, route points | Studio's editors; `base export` to open formats |
| A cooked cache instead of reading text at load | none for players; a cook step for authors | cooking at mount and in the background, re-cooking only what changed |
| Entry order by load order, not alphabetical | – | the central directory stays sorted for lookups |
| Original record layouts kept in `.bin` arrays | arrays are less self-describing than TOML | layout tables on every kind page; generated docs ([versioning.md](versioning.md) §6) |
