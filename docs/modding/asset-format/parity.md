# Asset format: the native guarantee (equality levels, proofs, gates, risks)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md).

## 1. The guarantee

> With "Use mods" off and a base set imported from a **known edition**, Overbright draws the same pictures, plays
> the same sound and simulates the same game, bit for bit, as the same build reading the original files did.

Reading the game through the format is a change of plumbing, never of behaviour. A `custom` edition
([import.md](import.md) §3) is imported with the same decoders but carries no promise.

## 2. Three equality levels

| Level | Name | Definition | Proven |
|---|---|---|---|
| **Eq1** | values | For every import unit, the in-memory model the engine uses, decoded from the original, equals the model loaded from the base set: `H(model from original) == H(model from base set)`, where `H` is XXH3-128 over the model's flat encoding (`crates/ob-core/src/cache/persist.rs`; order-sensitive, floats as bits). | At every import (step 5) and in CI on the reference install. |
| **Eq2** | bytes | For a residual node, the kind's encoder run on the decoded items gives back the original bytes exactly ([residual.md](residual.md) §3.2). | At every import, per node. |
| **Eq3** | behaviour | The game running on the base set reproduces the recorded baselines of the game running on the original files: frames, audio, simulation. | The gates (§3), in CI and before every merge that touches a reader. |

Every encoder is also tested on its own with generated items, both ways: `decode(encode(x)) == x` and, on the
reference install, `encode(decode(b)) == b` — the two laws of a well-behaved lens (Foster et al., "Combinators for
bidirectional tree transformations", TOPLAS 2007), with the residual as the complement that makes the second law
hold where no encoder exists yet. Round-trip properties are among the most valuable property tests in practice
(Goldstein et al., ICSE 2024).

Eq1 is what makes the engine's inputs identical; Eq3 is what proves nothing else changed (load order, timing, a
consumer that read original bytes outside the parsers); Eq2 is what makes the import lossless for the future
([residual.md](residual.md) §1).

**What the model must contain.** Eq1 only proves what is in the model. Every place where the engine reads original
bytes *after* parsing (texture blocks and vertex data taken straight from the mapped file, streaming by file
offsets, a decoder fed with raw stream bytes) must become a payload of the model, referenced zero-copy from a
bundle ([cooked.md](cooked.md) §5.1). Moving the engine onto the format starts with listing those readers (the
~116 `game_fs` call sites and the stray direct opens) and ends with a check that no game code opens a game-folder
path ([versioning.md](versioning.md) §6).

## 3. The gates

The existing gates are the proof; the format adds no new kind of check, only a second data source for them. Today's
outputs are stored as baselines (`scripts/checks/baselines.sh`: the frames, shots, WAVs and state hashes below, from
the original files, with the binary that made them), and `ASSETS=base baselines.sh check` runs the same set read from
the base set; a change meant to move an output re-records them in the same change with every changed item mapped to
its reason (docs/developing/checks.md "Output baselines", "Re-bless rule"):

| Gate | What is compared | Notes |
|---|---|---|
| Shots | Every `--shot` scenario, every resolution of the resolution matrix: PNG hashes | docs/developing/testing.md; also the launcher's renders |
| Audio | WAV captures of the audio scenarios (to files only; nothing is played on a device) | docs/original/audio-levels.md |
| Simulation | Per-tick state hash (`NFSU2_STATE_HASH`), parity traces, the career playthrough, at 30, 60, 120 and 200 frames per second | the world updates at 1/60 s at every display rate |
| Loads | RNG draw order in line-ups, world build and traffic fill; load results | the original's draw order is already pinned by tests |
| Cache | cooked ≡ fresh for every model section kind | `tests/world_cache.rs` extended |
| Import | Eq1 for every unit; the rebuild invariant for every file; the profile | `overbright-import --verify` on the reference install |

**How a kind moves.** First the whole engine reads the base set at **stage 0** ([import.md](import.md) §4.1): every
file kept byte for byte, today's parsers running on bytes from the asset layer. Because the bytes are the original's,
the gates must reproduce today's baselines exactly; that proves the plumbing (paths, case, read order, streaming)
before any decoding changes. Then each kind **graduates**: its decoded files replace its kept bytes, Eq1 proves the
values, and every gate must again reproduce its baseline bit for bit — and the kind's load benchmarks must not be
slower ([performance.md](performance.md) §6). Kinds graduate **on demand**, when a tool or a base set needs them
([import.md](import.md) §4.1); the goal is that every kind graduates. During the move the developer harness can run either source
(the asset layer's switch `NFSU2_BASE`: `off`, or a base set's folder or id; a run that read the base set says so
in its log with the layer's exit line `asset layer: ... from base set`, which the gate scripts require); when every kind has graduated, the shipping game no longer links
the original's parsers (they live in the importer), and a layering check fails if game code depends on them.

## 4. Per kind: level, proof, size

Measured on the reference install (1,250 content files, 1.80 GB); base sizes and times are estimates from the kind
pages [guess ±15 %]. "Shadow" is what the residual keeps twice until an encoder exists.

| Kind (page) | Stored as | Level reached at import | Original → base | Shadow | Import [guess] |
|---|---|---|---|---|---|
| Textures ([textures.md](textures.md)) | KTX 2.0 with the original blocks; P8 as indices + palette; images from compressed packs with Zstandard | Eq2 for plain packs and for the unwrapped blocks of compressed packs (compressed nodes) | 717 MB → ≈675 MB | ≈1.3 MB | 10–20 s |
| Solids ([meshes.md](meshes.md)) | glTF 2.0, vertex and index bytes unchanged (3 layouts) | Eq2 (encoder for headers, tables, fills) | 420 MB → ≈382 MB payload + 14 MB deflated JSON | 0 | 3–6 s |
| Animation ([animation.md](animation.md)) | `.eagl` passthrough + TOML; cameras, scenes, pivots TOML | Eq2; event tracks Eq1 | 6.7 MB → ≈7.5 MB | 38 KB | < 1 s |
| World ([world.md](world.md)) | TOML + `.bin` in the original record layouts | Eq2 for placements, sections, visibility, lights, props, animation; parameter maps Eq1 | 25 MB → ≈13 MB + 12.4 MB kept (chunks no code reads) | 0.1 MB | ≈1 s |
| Collision ([collision.md](collision.md)) | TOML header + `.bin` arrays | Eq2; prop volumes Eq1 | 16.5 MB → ≈16.2 MB | 0.43 MB | ≈0.2 s |
| Roads and AI ([roads-ai.md](roads-ai.md)) | TOML + `.bin` | Eq2; checkpoint lists Eq1 (unread tails differ per file) | 31 MB → ≈22 MB (identical copies once) | 1.1 MB (≈20 KB deflated) | < 1 s |
| Cars ([cars.md](cars.md)) | TOML records, every byte a field | Eq2 | 0.5 MB → ≈2.5 MB | 0 | < 0.1 s |
| Career ([career.md](career.md)) | TOML | stages, cash pickups, car unlocks Eq2; milestones and shops `patched`; the pool, events, brands, sponsors, messages and challenges Eq1; tracks, tricks, magazines Eq2 | 0.1 MB → ≈0.4 MB | ≈48 KB | < 0.1 s |
| Menus and fonts ([ui.md](ui.md)) | TOML object trees, glyph tables | Eq2 on the unpacked packages and fonts | 2.1 MB → ≈25 MB TOML (≈3 MB deflated) | 0 (compressed nodes, residual §3.1) | ≈1 s |
| Text ([text.md](text.md)) | TOML, encodings recorded; Korean kept as bytes | Eq2 on every table and file | 3.8 MB → ≈7 MB (≈2 MB deflated) | 0 | < 0.2 s |
| Audio ([audio.md](audio.md)) | codec passthrough: bank images, engine recordings, speech and music streams as they are + TOML indexes | Eq2 (the payload is the bytes); `.csi/.idx/.evt/.mpf/.mxb` Eq1 | 282 MB → ≈282 MB | 36 KB | ≈1 s |
| Movies ([movies.md](movies.md)) | VP6 in IVF + the sound stream and chunk order | Eq2: every `.vp6` rebuilds byte for byte | 261 MB → ≈261 MB | 0 | 1–2 s |
| Engine tables ([engine-tables.md](engine-tables.md)) | built into the engine; packs override | an empty override set keeps every pin | – | – | – |

**Totals** [guess]: base set ≈ **1.7 GB** (the original's content folders: 1.80 GB), of which ≈ 15 MB kept bytes
of chunks no code reads and ≈ 3 MB shadow. The asset cache stays within its limit (2 GB by default) as today; model
entries are small because large payloads are referenced, not copied. **Import** at stage 0 (the first import every
player runs) is a copy plus digests: under a minute on an SSD, about half a minute to two minutes on a hard disk or an
SD card. A fully graduated import on the reference desktop: about
20–35 s of conversion plus as long again for the read-back verification, plus 2–4 s of fingerprinting: **≈ 1 min**;
a laptop with a hard disk: a few minutes (read-bound) [guess]. After the import the original folder (1.8 GB) is no
longer needed.

**Encoders still to write** before a first import reaches these levels: the texture pack framing, solid lists,
camera tracks, scene headers, collision, routes and roads, car records, menu packages, fonts, string tables and the
other text files, reverb presets, movie chunk order (each kind page's section 6 lists them; prototype encoders on
the reference install already rebuilt the menu packages, fonts, string tables, text files and movies byte for
byte, and the record layouts of world, cars and collision were checked byte by byte).

**Known value-level issues the move must settle first** (from the kind pages): today's texture parser drops 12
vinyl images of version-4 packs (textures.md); two HUD packages name a font no file holds and the engine falls back
to the first entry of a randomly seeded map, so four HUD strings can change font between runs today (ui.md); the
section table's first entry overlaps others in two regions so one scenery section is placed twice today
(world.md). Each is fixed against the original under Wine before its kind's baseline is recorded.

## 5. Risks to the guarantee, and what holds them

| # | Risk | Held by |
|---|---|---|
| R.1 | **Order changes.** A writer or reader that sorts, deduplicates or iterates a hash map changes item order, and with it draw and cull order, "first match wins" lookups and RNG draws during loads. | Order is data (conventions §6.5); Eq1's encoding is order-sensitive; the load and RNG gates. |
| R.2 | **Floats through text** lose a bit (double rounding when read through binary64). | Conventions §3.4 (a decimal's binary32 value defined as round₃₂(round₆₄(x)), shortest round-trip writes, `bits:` for the two values that need it); a test over every binary32 bit pattern class and an exhaustive nightly run. |
| R.3 | **Raw-byte consumers** outside the parsers read original files later (streaming by offset, read-ahead, decoders). | §2 "What the model must contain"; the no-game-path check; the gates. |
| R.4 | **Facts read from the folder**, not from a file's content: the region (from the `CARS` folder's contents, `crates/ob-core/src/region.rs`), the installed languages (`src/qol/setup.rs`), case-insensitive names. | The importer runs the same detection and records it (`[base] region`, `languages`); the engine reads those fields; Eq1 covers them. |
| R.5 | **Decoder state** across block boundaries, loop points and seeks, when audio is stored as decoded PCM. | The audio page fixes what is stored (whole decoded streams vs. blocks with predictor state); the audio gate. |
| R.6 | **Stale cooked data** after a reader change. | Layout hash + cooker version + input digests ([cooked.md](cooked.md) §3–4); `--verify-cache`. |
| R.7 | **Unknown or modified editions** give other values. | No promise for `custom`; the launcher says so; parity runs only on known editions. |
| R.8 | **Timing**: slower loads (text parsing) shift what is visible when, where the engine's behaviour depends on load completion. | Cooked entries make loads as fast as today; the free-roam and race load budgets stay in the perf gates; any load-order dependence is a bug the load gate shows. |
| R.9 | **Meaning drift** between a field's documentation and its use (a field documented as one thing, consumed as another). | Field tables generated from the code ([versioning.md](versioning.md) §6); Eq1 compares values, not documentation. |
| R.10 | **Lossy shortcuts** creeping into the importer (a colour converted, a quantised value rounded). | Conventions §5.2 (convert only if exact both ways); Eq1 fails on any lost bit. |
| R.11 | **Lookup history.** The original searches loaded texture packs in a most-recently-used order, so its answer could depend on history if one view held two different textures under one hash. | Fixed search paths per view (textures.md); an import check refuses differing duplicates per view (TEX.20); four cases are checked under Wine before textures move. |
| R.12 | **Positions that are behaviour.** Car type index (engine tables are indexed by it), part database positions (they decide the owned-part bits in native saves), section and bank order, first- or last-match rules for repeated names. | Order is data; append-only additions; pages refuse re-pooling and sorting; the save round-trip tests. |
| R.13 | **Float evaluation order** in decoders and maths moved between modules (three EA-XA decoders evaluate in three different orders today). | Passthrough storage keeps decoding where it is; per-payload decode hashes in the audio gate. |
| R.14 | **Random draws counted by data.** Counts in the data decide how many random draws a choice takes (magazine cover setups, open-event shuffles). | Merges that change such counts are refused until the original's handling of them is traced (MRG.3); the load gate's draw order. |
| R.15 | **Bytes the engine reads raw.** Some records are read and rewritten in place (car records, sound bank images read at file offsets). | Their cooked form is the encoded original layout; per-record Eq2. |

