# Asset format: the residual (original bytes kept until they are understood)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md)
apply. Only base sets made by the importer have a residual.

## 1. Why

The player's original folder may be gone after the import, yet later Overbright versions will decode more of the
original data (fields marked **open** today, chunks no parser reads, better meanings). They must be able to do that
from the base set alone. So the importer guarantees one invariant:

> **Rebuild invariant.** For every original file the importer read, the base set holds enough to rebuild the
> file's *decoded container content* byte for byte: each byte is either produced again by a proven encoder from
> the decoded items, or kept as it was in the residual.

"Decoded container content" means after removing the original's compression (JDLZ, HUFF) and splitting its
archives (VIV/BIG4): the bytes the original's own loaders see. The compressed form is not rebuilt (Overbright has no
JDLZ/HUFF compressor and needs none); its digest is recorded so the file can still be identified.

A rebuilt file exists only in memory or in a temporary file in the cache folder while a newer importer re-derives
from it ([import.md](import.md) §7). It is never written into a pack, shown as a file, or sent anywhere.

## 2. Files

For each original file `<ORIGINAL/PATH>` (its path in the game folder, original spelling):

```text
residual/<ORIGINAL/PATH>.map.toml     the file map: structure and the fate of every byte
residual/<ORIGINAL/PATH>.kept.bin     the kept bytes, concatenated, 16-aligned (absent when nothing is kept)
```

Both live in the bundle of the load unit that reads the original file, under `residual/` ([layout.md](layout.md) §5;
originals several units read go in `global.obz`), so a load opens one file at every stage: maps and small kept bytes
deflated; every stage-0 kept file stored and 256-aligned.

## 3. The file map (`ob.residual-map/1.0`)

```toml
schema = "ob.residual-map/1.0"
original = "GLOBAL/GlobalB.lzc"
original_size = 5145778
original_digest = "sha256:ad015fdb24e3bfce3aff8bcdfbbef3cb981b524115884a8eed4bf3ce4a2d6e90"
container = "jdlz_chunks"
content_size = 0x10000
content_digest = "xxh3-128:00000000000000000000000000000000"

# Illustrative offsets (career.md §5): the first career container and two of its children, a tricks chunk with a
# byte proof, a chunk nothing reads yet, and an alignment fill.
[[node]]
path = "0x80034A10"
offset = 0x0
size = 0x9000
fate = "container"

[[node]]
path = "0x80034A10/0x00034A11"
offset = 0x8
size = 0x4000
fate = "decoded"
items = ["event:*"]
proof = "values"
kept = { file = "GlobalB.lzc.kept.bin", offset = 0, count = 16384, layout = "raw_u8" }

[[node]]
path = "0x80034A10/0x00034A1D"
offset = 0x4008
size = 0x4FF8
fate = "decoded"
items = ["event:*", "sponsor:*", "shop:*"]
proof = "values"
kept = { file = "GlobalB.lzc.kept.bin", offset = 16384, count = 20472, layout = "raw_u8" }

[[node]]
path = "0x00034A07"
offset = 0x9000
size = 0x790
fate = "decoded"
items = ["trick:*"]
proof = "bytes"

[[node]]
path = "0x00034202"
offset = 0x9790
size = 0x470
fate = "kept"
kept = { file = "GlobalB.lzc.kept.bin", offset = 36864, count = 1136, layout = "raw_u8" }
reason = "no reader found"

[[node]]
path = "pad"
offset = 0x9C00
size = 0x6400
fate = "fill"
fill = 0x00
```

| Field | Type | Req. | Meaning |
|---|---|---|---|
| `schema` | string | yes | `ob.residual-map/1.0` |
| `original` | string | yes | The file's path in the game folder, original spelling, `/`-separated. |
| `original_size` | u64 | yes | Size of the original file. |
| `original_digest` | digest | yes | `sha256` of the original file. |
| `container` | enum | yes | `plain`, `chunks` (the bundle chunk container, docs/formats/world.md "Chunk container"), `jdlz`, `huff`, `jdlz_chunks` (a JDLZ-compressed chunk file), `viv` (BIG4 archive), `text`. |
| `content_size` | u64 | yes | Size after unwrapping (= `original_size` for `plain`, `chunks`, `text`). |
| `content_digest` | digest | yes | `xxh3-128` of the unwrapped bytes. The rebuild check compares with it. |
| `node` | array of tables | yes | Covers `[0, content_size)` exactly once, in offset order (`RES.1`). |
| `node.path` | string | yes | Chunk ids from the root (`0x80034A10/0x00034A11`), an archive entry's name, `pad`, or `records[a..b]` for a run of fixed records. |
| `node.offset`, `node.size` | u64 | yes | Range in the unwrapped content, including the chunk header when the node is a whole chunk. |
| `node.fate` | enum | yes | `decoded`, `kept`, `fill`, `external` (linked sets only), `container` (a parent whose children are nodes of their own; its header bytes are rebuilt from the children's ids and sizes), `copy` (the bytes equal a range of another original file of the same import, e.g. the `*MemoryFile.bin` files that repeat the starts of other files). |
| `node.external` | table | if external | `{ path = "<path in the game folder>", size, sha256 }`: in a **linked** base set only ([import.md](import.md) §4.2), a stage-0 file that stays in the game folder. |
| `node.count` | u32 | no | For a run node (3.1.1): the number of sibling chunks it covers. Default 1. |
| `node.wrap`, `node.wrap_size`, `node.wrap_digest` | enum `jdlz`/`huff`, u64, digest | for compressed nodes | §3.1 "Compressed nodes". |
| `node.copy` | table | if copy | `{ original = "<path>", offset, size }`: the source range, in that file's unwrapped content. Rebuilt from that file (which must itself rebuild). |
| `node.items` | array<string> | if decoded | The items made from it. |
| `node.proof` | enum | if decoded | `bytes`, `patched` or `values` (3.2). A `values` node MUST also have `kept`; a `patched` node MUST have `patch`. |
| `node.patch` | array descriptor | if patched | Pairs of (offset in the node: `u32`, byte: `u8`, pad `u8[3]`), layout `byte_patch`: the few bytes where the encoder's output differs from the original (old, unread bytes in otherwise re-encodable records). |
| `node.kept` | array descriptor | if kept | Where the bytes are ([conventions.md](conventions.md) §4.4). |
| `node.fill` | u8 | if fill | The fill byte. |
| `node.reason` | string | no | Why it was kept. |

### 3.1 Containers

`chunks`: a node per chunk; container chunks (bit 31 of the id set) have `fate = "container"` and children; id-0
padding chunks and alignment fills between chunks are `fill` nodes (or `kept` when not one repeated byte). Fill
*inside* a leaf chunk (the `0x11` runs before an aligned payload) is part of that chunk's node: its kind's encoder
writes it, and the kind page says how. `viv`: one `container`
node per archive entry, named by the entry's name, then that entry's own nodes. `jdlz` / `huff` /
`jdlz_chunks`: the map describes the unwrapped content; the compressed stream is identified only by
`original_digest`.

**Compressed nodes.** Some chunks are compressed on their own inside an uncompressed file (menu packages in chunk
0x30210, compressed texture blocks). Such a node has `fate = "container"` and `wrap = "jdlz"` or `"huff"`; its
`wrap_size` and `wrap_digest` (`xxh3-128`) identify the compressed bytes, and its children describe the unwrapped
bytes, offsets counted from the start of the unwrapped data. The node covers only the compressed bytes; what lies
around them in the same chunk (a hash word before them, fill after them) are ordinary sibling nodes. Rebuilding
yields the unwrapped bytes in the compressed bytes' place; a file containing such a node therefore rebuilds to its
**expanded content**, and its map's `content_size` / `content_digest` describe
that expanded form (the compressed form is identified by `original_digest`, as for whole-file compression).

### 3.1.1 Node granularity

- **Stage 0.** A unit not yet graduated ([import.md](import.md) §4.1) has, per file, a map with
  `container = "plain"` and one `kept` node covering the **original file byte for byte**, compression included (or
  one `external` node in a linked set): no node map below it until a decoder exists, so today's parsers read exactly
  the original bytes. Unwrapping (JDLZ, HUFF, BIG4) starts when the unit graduates. Every stage-0 kept file is
  stored, not deflated, and 256-aligned in its unit's bundle, so it can be memory-mapped like any payload and no load
  pays for inflating it ([performance.md](performance.md)); cooked entries reference its payload ranges instead of
  copying them ([cooked.md](cooked.md) §5.1).
- **Whole-file nodes.** A kind stored by passthrough (a sound bank image, an engine recording, a movie's streams)
  MAY describe its whole file, or a whole archive entry, with one `decoded` node and `proof = "bytes"`: the
  passthrough payload *is* the bytes.
- **Run nodes.** A run of sibling chunks with the same id and the same fate (the 53,020 solids of the reference
  install's 2,937 solid lists, their padding fills) MAY be one node with `count = n` (the number of sibling chunks it covers, the id-0 padding chunks between them included): `path` names the first chunk,
  `size` covers the whole run, and the kind's encoder rebuilds the run including its fills. This keeps the maps
  small.
- **Shared items.** One decoded item may be named by nodes of several files: the importer imports identical content
  once (`InGameCommon.bun` and the unpacked `InGameCommon.lzc`; `GLOBALB.BUN` and the unpacked `GlobalB.lzc` where
  they agree; route files of one region carrying identical trigger tables), and each file's map points at the same
  items ([import.md](import.md) §4).

### 3.2 Proof levels

- **`bytes`**: the kind's encoder, run on the decoded items, gives back exactly the node's bytes (checked at import,
  for every node). Such a node keeps nothing.
- **`patched`**: the encoder gives back the node's bytes except at a few positions (bytes no reader uses: old
  name tails, unused fill); those bytes are kept as patches. The node keeps only the patches, not a full shadow.
- **`values`**: the import proved that the decoded items equal what the original's parser gives
  ([parity.md](parity.md) §2), but no encoder exists yet. The node's bytes are kept too (**shadow copy**), so the
  rebuild invariant holds. When a later importer version adds the encoder and proves it on the kept bytes, a
  migration drops the shadow ([import.md](import.md) §7).

A node without a decoder is `kept`. A `values` node keeps its bytes as a shadow next to its decoded items, a cost
that shrinks with each encoder ([parity.md](parity.md) §4 lists them).

### 3.3 Binary layouts

`raw_u8`: one byte per element (kept bytes, shadow copies).

`byte_patch` (8 bytes, 4-aligned):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `offset` | byte | position inside the node; strictly increasing within one node |
| 0x04 | 1 | `u8` | `value` | – | the original byte |
| 0x05 | 3 | `u8[3]` | `pad` | – | zero |

## 4. Rebuild

To rebuild a file: walk its nodes in order; for `decoded` + `bytes`, run the kind's encoder on the items; for
`decoded` + `patched`, run the encoder and apply the patches; for `kept` and `values`, copy the kept bytes; for
`copy`, copy the range from the other file's rebuilt content; for `fill`, write the byte; for `container`, write the
chunk header from the children. The result MUST have `content_size` bytes and `content_digest` (`RES.3`); otherwise the base set is
damaged.

## 5. What is not imported

The importer reads only game content. It skips, and lists in `import.toml` as skipped: executables and libraries
(`*.exe`, `*.dll`, `*.asi`), mod loaders' folders and settings (`SCRIPTS/`, `scripts/`, loader `*.ini`), the
installer's leftovers (`*.bat`, `*.ico`, read-me and file-list texts at the root), and anything outside the
game's content folders. `SPEED2.EXE` is never read for content: the engine's numbers are in Overbright's own code
(docs/exe-tables.md). Unknown files inside content folders (an add-on car from another tool, say) are imported
when a decoder accepts them and kept whole otherwise, and make the import's edition `custom` ([import.md](import.md)
§3).

## 6. Privacy and sharing

The residual holds the player's own game bytes. It never leaves the player's machine: no shareable pack, export,
support file or crash report includes it, and Studio offers no way to open it as a file. The pack check refuses any
residual chunk's bytes in a mod pack not marked `from-game-files` / `local-only` (`MRG.5`,
[layering.md](layering.md) §5).

## 7. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| RES.1 | Nodes do not cover the content exactly once, are out of order, or a node's range runs past `content_size`. |
| RES.2 | A `values` node without kept bytes; a `kept` node whose descriptor is out of range. |
| RES.3 | A rebuilt file's size or digest differs from the map. |
| RES.4 | A residual file in a mod pack, or a map naming a file outside the game's content folders. |
| RES.5 | A `copy` node whose source range lies outside its file, or copy nodes that refer to each other in a cycle. |
| RES.6 | Patches out of order, outside the node, or a `patched` node whose patches exceed 1/16 of its size (then it is a `values` node with a shadow). |
| RES.7 | An `external` node in an owned base set, or in any mod pack. |
<!-- /generated -->

## 8. Sizes

Reference install (1,250 content files, 1.80 GB): at **stage 0** the residual holds every file once, stored:
≈ 1.8 GB, 1.0× the folder ([performance.md](performance.md) §4); once every kind has graduated it
holds ≈ 15 MB of chunks no code reads plus ≈ 3 MB of shadow ([parity.md](parity.md) §4) [guess]. Kinds stored losslessly by construction (texture blocks copied as they are, vertex and index
buffers copied as they are, audio and movie bitstreams kept as they are) need no shadow once their small headers
have encoders.
