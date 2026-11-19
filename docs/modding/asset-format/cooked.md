# Asset format: the cooked form (the asset cache)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md)
apply.

The source form is the format; the **cooked form** is what a reader derives from it to load fast: the merged,
decoded model of one load unit in a flat binary encoding, plus build-specific prepared data. It lives in the asset
cache, is never shared, never part of a pack, and is always rebuildable from the source form. This page documents
it so tools can inspect and verify it; it is **not** a compatibility promise between Overbright versions (§6).

Prior art taken: Unreal's split between editor assets, per-platform cooked output and a disposable derived-data
cache (https://dev.epicgames.com/documentation/en-us/unreal-engine/using-derived-data-cache-in-unreal-engine);
Godot's import cache keyed by the source's hash and the importer's format version
(https://docs.godotengine.org/en/stable/tutorials/assets_pipeline/import_process.html). Kept from Overbright: the
asset cache's framing, writes, temporary files and pruning (docs/developing/asset-cache.md, `crates/ob-core/src/cache/**`).

## 1. Cook units

A cook unit is what one load needs, normally one load unit of the logical tree ([layout.md](layout.md) §2):
a region, a car, a scene, the global data, the front end, a sound-bank group. Each kind page names its cook unit
and model sections (section 7 of every kind page). A unit is cooked for one **layer stack** (base set + packs) and
one **target**.

| Target | Used by | Differences |
|---|---|---|
| `desktop` | the game, Studio and the CLI on Windows and Linux | BC texture blocks used as they are; payloads memory-mapped |
| `web` | the browser build (later) | no memory mapping; files in the browser's private storage (OPFS) |
| `web-nobc` | the browser build on an adapter without `texture-compression-bc` | BC textures decoded to RGBA8 on the CPU when cooked (WebGPU guarantees BC *or* the ETC and ASTC families, not BC, https://www.w3.org/TR/webgpu/#texture-compression-bc) |

## 2. Two layers inside the cache

1. **Model entries**: the merged decoded model of a cook unit (what the engine consumes: placements, records,
   vertices, texture records), in the cache's flat encoding (`crates/ob-core/src/cache/persist.rs`: little-endian, fields in
   declaration order, floats as bits, maps sorted, large arrays 16-aligned). Keyed by the **trace of its lookups** (§3) and the
   **layout hash** (§4). They survive Overbright updates that do not change the model's types.
2. **Prepared entries**: build-specific data derived from model entries (texture arrays as uploaded, merged vertex
   buffers, surface maps, the pipeline cache). Keyed by the build id as today (docs/developing/asset-cache.md "Names and
   versions") plus the model entry's digest. Not documented beyond their framing.

Today's track and car cache files hold both at once; splitting them is part of moving the engine onto the format.

## 3. Inputs and the cache key: a verifying trace

A model entry records **what the cook looked up and what it got** (the verifying-trace model of "Build Systems à la
Carte", Mokhov, Mitchell, Peyton Jones, ICFP 2018):

| Record | Meaning |
|---|---|
| lookup | A logical path the cook asked for, and the **ordered list** of `(layer, digest)` pairs that contributed to the merged result (bottom first), or "not found". |
| listing | For a directory listing or a wildcard: `(directory, layer)` → the digest of the names that layer holds there. |
| cross-unit reference | A reference into another unit is recorded as a lookup of the file that resolves it. |

plus the target, the layout hash, the cooker version and a global cache salt (bumped to disown entries a faulty build
wrote, as Bazel's `Action.salt`, https://bazel.build/remote/caching). An entry is used only when every record still
gives the same answer against the **effective** layer stack (after packs refused at mount, [layering.md](layering.md))
and the other fields match. Neither the stack order nor whole-pack digests are part of the key: toggling or
reordering a mod re-cooks only the units whose lookups it changes, and editing one texture of a pack re-cooks only
the units that read it.

Cooks get only a **recording handle** to the asset layer, so a lookup that bypasses the record cannot happen (a lint
checks it). A property test shuffles related and unrelated layers around a cooked entry and checks that hits and
misses come out as predicted and that every hit equals a fresh cook. A golden-output test cooks fixed
fixtures and pins each cook kind's output digest next to its cooker version, so a cooker change without a version
bump fails the build.

Checking digests must not mean reading every source file at each start. Per bundle, the cache keeps the bundle's
size, modification time and (Unix) inode and the digests of its entries as last verified ([layout.md](layout.md)
§5); while those match, the recorded digests are trusted, exactly as the asset cache trusts a game file's
size/time/inode today (`crates/ob-core/src/cache/format.rs` `SourceFile`). A folder-form pack (being edited) is checked per file
the same way.

## 4. The layout hash

Every type stored in a model entry declares its layout through the `persist!` macro (field names, field types,
recursively). The **layout hash** is the XXH3-64 of that description for all types of the entry. So:

- a change to a stored type changes the hash, and old entries are misses (no manual version bump to forget, the
  property the build id gives today);
- a change elsewhere in the code does not, and the entries survive the update (which the build id does not allow
  today: every update re-parses everything);
- `--verify-cache` prints the descriptions, and a test pins each section kind's hash so a change shows up in review
  (as `exe_tables` pins its tables, `crates/ob-formats/src/exe_tables/tests.rs` `FIELD_PINS`).

The **cooker version** (an integer per unit kind) is bumped when cooking logic changes without a type change (a
merge rule fixed, say).

## 5. Framing

The existing framing (docs/developing/asset-cache.md; `crates/ob-core/src/cache/format.rs`) with format 3:

| Offset | Size | Type | Field | Notes |
|---|---|---|---|---|
| 0x00 | 8 | `u8[8]` | `magic` | `NFSU2CCH` (kept: the framing is the cache's own) |
| 0x08 | 4 | `u32` | `format` | 3 |
| 0x0C | 8 | `u64` | `header_len` | |
| 0x14 | 8 | `u64` | `header_hash` | XXH3-64 of the header bytes |
| 0x1C | `header_len` | – | `header` | flat-encoded (below) |
| … | | | sections | each at a multiple of 256 from the file's start |

Header (flat encoding, fields in this order):

| Field | Type | Meaning |
|---|---|---|
| `kind` | string | The entry's kind: `model` or `prepared`, then the cook unit (`model/world/L4RA`). |
| `target` | string | §1. |
| `build` | u64 | The build id (prepared entries); 0 for model entries. |
| `layout` | u64 | The layout hash (model entries); 0 for prepared entries. |
| `cooker` | u32 | The cooker version. |
| `stack` | list of packId | For tools only: the layer stack the entry was cooked with, bottom first (not part of the key, §3). |
| `lookups` | list of (logical path, list of (packId, digest) or "not found") | §3. |
| `listings` | list of (directory, packId, names digest) | §3. |
| `bundles` | list of (sort, packId, name, size, digest) | Sources referenced by `extref` payloads (§5.1): `bundle`, `file` (folder form) or `external` (linked sets). |
| `layouts` | list of (section name, layout description text) | For tools: what each section holds. |
| `sections` | list of (name, offset, length, XXH3-64 per 4 MiB) | As format 2. |

### 5.1 Zero-copy payloads

Large payloads that are byte-identical to a stored entry of a source bundle (texture blocks, vertex and index
arrays, audio and movie bitstreams) are not copied into the cache. The model holds an **external reference**
instead:

| Offset | Size | Type | Field | Notes |
|---|---|---|---|---|
| 0x00 | 4 | `u32` | `bundle` | index into the header's `bundles` |
| 0x04 | 4 | `u32` | `pad` | zero |
| 0x08 | 8 | `u64` | `offset` | byte offset in the bundle file |
| 0x10 | 8 | `u64` | `len` | |

24 bytes, 8-aligned. At stage 0 ([import.md](import.md) §4.1) the same mechanism references payload ranges
(texture blocks, vertex and index arrays) inside the stored kept originals, so a cooked entry holds only what the
cook derived; this roughly halves today's track cache (139 MB for a 118.5 MB source) [guess]. `bundle` indexes the header's `bundles` list, whose entries are of three sorts: a stored
bundle entry's file (the common case), a file of a folder-form pack (a plain file, mapped whole), and, in a linked
base set only, an original file in the game folder (by path, size and SHA-256; [import.md](import.md) §4.2). On
desktop the reader maps each source once and hands out slices (the cache's `Bytes::Mapped`, `crates/ob-core/src/cache/bytes.rs`,
extended to these mappings); on the web it reads the range from storage. A reference into a deflated entry is not
allowed: such payloads are copied into the entry.

### 5.2 Writes, temporary files, pruning

Unchanged from docs/developing/asset-cache.md (background writer, one writer per file, temporary file renamed into place,
sweep, superseded entries, size limit). Model entries of other layouts and stacks are pruned like other builds'
files today; a stack the player switched away from keeps its entries until the size limit or the "unused for an
hour" rule removes them.

## 6. No promise across versions

- A reader MUST treat any cooked file it does not fully understand (format, layout, cooker, target) as a miss and
  rebuild from source. It MUST NOT migrate cooked files.
- Cooked files MUST NOT be distributed, put in packs or support files, or used as a source by any tool other than
  the cache's own inspector (`overbright-studio-cli cache inspect`).
- Equality of cooked and fresh data is tested exactly as today (`tests/world_cache.rs`: cache ≡ fresh), extended to
  every model section kind ([parity.md](parity.md) §3).

## 7. What the web build loads

The web build has no install folder and no memory mapping:

1. **Import in the browser**: the player selects their game folder (the File System Access API where available, a
   folder drop or `<input webkitdirectory>` otherwise); the importer, compiled to WebAssembly, runs in a worker and
   writes the base set's bundles into the origin's private storage (OPFS). Nothing is uploaded; the page's server
   never sees the files ([import.md](import.md) §11).
2. **Packs**: `.obpk` files dropped in or fetched from the author's own URL, stored in OPFS.
3. **Cooking** happens in a worker into OPFS, target `web` or `web-nobc`, with synchronous access handles instead of
   mappings.
4. **Storage** may be deleted by the browser: the page asks for persistent storage (`navigator.storage.persist()`)
   before importing, and a base set the browser evicted means "import again" ([import.md](import.md) §11).
5. **Memory**: a 32-bit WebAssembly memory holds at most 4 GiB; payloads are read on demand per unit instead of
   mapped. The shipping game's measured peak resident memory (1.3 GB; 2.1–2.7 GB with a texture pack,
   [performance.md](performance.md) §1) fits; the largest texture packs and the highest supersampling levels may
   not [guess].

The format itself has nothing web-specific: bundles are ZIPs, payloads are plain ranges, and every reader path goes
through the same VFS `Source` trait.
