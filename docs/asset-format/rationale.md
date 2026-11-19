# Asset format: design rationale and evidence

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md). Informative: the normative rules are on the
other pages; this page says why they are as they are, what evidence supports them, and where the format departs
from common practice on purpose.

Evidence is of three kinds: the **code** of other projects' format readers and writers (read at the commits below),
**specifications and standards**, and **research**. Code citations are `repository path:lines @ commit`.

| Key | Repository | Commit read |
|---|---|---|
| glTF | https://github.com/KhronosGroup/glTF | b0f245d |
| glTF-V | https://github.com/KhronosGroup/glTF-Validator | 602c540 |
| KTX | https://github.com/KhronosGroup/KTX-Software | 983695c |
| USD | https://github.com/PixarAnimationStudios/OpenUSD | 3ed9ded |
| OpenMW | https://github.com/OpenMW/openmw | eb2d2e7 |
| Godot | https://github.com/godotengine/godot | b070e63 |
| Bevy | https://github.com/bevyengine/bevy | 46479b9 |
| Factorio | https://github.com/wube/factorio-data | 5c03eaa |
| LUS | https://github.com/Kenix3/libultraship (Ship of Harkinian's archive layer) | 759b134 |
| DFU | https://github.com/Mojang/DataFixerUpper | b6c712e |

## 1. Our own format, made once from the player's copy

**Decision.** A one-time importer converts the player's own installation into a base set in Overbright's format;
everything else reads only that ([import.md](import.md)). The base set is an ordinary pack (`kind = "base"`).

**Evidence.** Ship of Harkinian extracts the player's ROM into its own ZIP archive and reads only archives
afterwards; mods are more archives of the same kind, and a later archive owns a path (LUS
`src/ship/resource/archive/ArchiveManager.cpp:163-166, 285-312`); an archive whose game version is not accepted is
not mounted (`Archive.cpp:38-98`). OpenGOAL validates the player's disc against an edition database and builds its
own data, re-deriving it after updates. Factorio's own game content is the mod `base` (Factorio `base/info.json`).
OpenTTD's original-data and free base sets coexist under one manifest format. OpenMW's template game shows a full
game on an engine without the original data, and also the cost of engine-hard-coded asset names, which this format
avoids with the required-items profile ([manifest.md](manifest.md) §5).

**Departure.** OpenGOAL keeps a copy of the extracted original for re-derivation, and users who deleted it broke
their updates (open-goal/launcher issue 638). The residual's rebuild invariant keeps the same ability without a
second copy ([residual.md](residual.md) §1), and the player's default is an owned base set that needs no game folder
([import.md](import.md) §4.2).

## 2. Stage 0 and graduation

**Decision.** The first import keeps every file byte for byte (stage 0); kinds graduate to decoded files one by one,
each proven ([import.md](import.md) §4.1).

**Why.** It separates plumbing from decoding: the engine moves onto the asset layer while reading bytes identical to
the original, so the gates prove the plumbing alone; each later graduation changes one kind and is proven alone.
Equality at stage 0 holds by construction.

**On demand, with every kind as the goal.** A kind graduates when a tool edits it or a base set needs it, not in one
block before anything else: each graduation is a parity proof and a risk to native play, so it is paid for when it
buys something. Kinds no tool edits yet stay in the original's own encoding inside Overbright's own bundles, and their
original parsers stay in the game until then; the format is honest about being only a container for those kinds. The
goal stays that every kind graduates (only then do the original's parsers leave the game, and only then can a
community base set or new content replace the kind), and every kind must have graduated before the engine-level work
that depends on it.

## 3. Open formats where exact, the original bitstream where not

**Decision.** Records, tables, meshes and textures in open forms (TOML, glTF 2.0, KTX 2.0 with the original blocks);
media bitstreams kept when an open form cannot hold the values exactly or would grow more than 3×
([conventions.md](conventions.md) §4.6).

**Evidence.** glTF's buffers, buffer views and `_`-prefixed custom attributes hold the original vertex bytes
unchanged (glTF `specification/2.0/Specification.adoc:1296-1356`); KTX 2.0 carries BC blocks as they are, and its
level alignment is lcm(block size, 4) with no extra padding allowed (KTX `lib/src/texture2.c:1867-1881`). The
original's decoded sound is unrounded `f32` (measured: 91 % of bank sample values have a fraction), which no integer
PCM format holds; decoded music would grow 7.85×; lossless video 7–11× ([audio.md](audio.md),
[movies.md](movies.md)).

## 4. Refuse what is not understood

**Decision.** Unknown keys, malformed values, duplicates and non-zero padding are refused; a pack with any refused
file is not mounted ([conventions.md](conventions.md) §3.2, §11).

**Evidence for.** RFC 9413 ("Maintaining Robust Protocols", 2023, https://www.rfc-editor.org/rfc/rfc9413.html):
"ignoring faulty or ambiguous input is almost always the incorrect solution" (§2.1), and deliberate intolerance
gives implementers better feedback (§5.1). LANGSEC (Sassaman et al., ;login: 2011): lenient parsing trades
consistent behaviour for interoperability; parser differentials cause bugs (Momot et al., IEEE SecDev 2016). JSON
parsers disagree on duplicates, numbers and nesting (Seriot, "Parsing JSON is a Minefield", 2016). OpenMW refuses
unknown record and subrecord types (`components/esm3/loadacti.cpp:36-38`, `esmreader.cpp:517-528`); KTX refuses
unknown keys in its reserved namespace while keeping vendor keys (`tools/ktx/validate.cpp:1491-1500`), which is the
model of `[extras]` and `[ext.*]` here; ProtoJSON rejects unknown fields by default.

**Practice that differs.** glTF loaders ignore unknown properties (the validator only warns,
glTF-V `lib/src/utils.dart:614-622`), I-JSON (RFC 7493 §4.2) and Avro ignore unknown members, OpenUSD passes unknown
metadata through (`pxr/usd/sdf/textParserHelpers.cpp:133-168`), and Godot drops unknown properties. They do so so
that newer files reach older readers. Here that need is met differently (§5): a file declares the lowest version
that reads it, so an older reader refuses only files that really use something new, with a message naming the
release that added it. Unknown keys then only ever come from typos and writer bugs, which should fail loudly.

**One parser.** All programs read through one TOML reader and one JSON reader, so they cannot disagree about an
input ([conventions.md](conventions.md) §3.4).

## 5. Versions: the lowest version that reads the file

**Decision.** `schema = "ob.<kind>/<major>.<minor>"` names the lowest minor that can read the file; minors only add;
readers refuse newer minors and check the declaration (`VER.6`); renames by alias; released names never reused
([versioning.md](versioning.md) §2).

**Evidence.** glTF's `asset.minVersion` (glTF 2.0 §3.2) states the same idea. OpenUSD's writers do it in code: new
binary files start at an old version and raise it only for value types that need it
(`pxr/usd/sdf/crateFile.cpp:168-177, 1071-1088, 1522-1540`); text files likewise
(`usdaFileFormat.cpp:48-124`); readers refuse what they cannot read (`crateFile.cpp:3540-3561`). Godot's text
resource writer starts at format 3 and raises it only for content that needs 4 or 5
(`scene/resources/resource_format_text.cpp:1730-1781`). Avro resolves renamed fields through reader aliases; Protocol
Buffers forbids reusing field numbers (`reserved`); Cap'n Proto and FlatBuffers forbid changing defaults — a changed
default is a major here too. Wikipedia's schema history shows renames are common (Curino et al., ICEIS 2008: about
16 % of operations), so renames are minors by alias, not majors.

**Practice that differs.** Godot's binary writer always writes the newest format; Bevy writes a meta version it never
checks (`crates/bevy_asset/src/meta.rs:27-58`). Both make old readers fail on files that did not need anything new.

## 6. Migrations kept, applied on read for packs

**Decision.** Every major's migration is kept forever as a chain; packs migrate in memory and stay unchanged on disk;
base sets migrate on disk once; rename tables apply atomically ([versioning.md](versioning.md) §4).

**Evidence.** Mojang's DataFixerUpper composes versioned fixes into one converter
(DFU `src/main/java/com/mojang/datafixers/DataFixerUpper.java:112-136`), but on a failed fix it logs and continues
with partial data (`:74-82`); here a failed migration refuses the file (`VER.3`). Factorio records applied migrations
in each save and never applies one twice; its own migrations swap names within one file, which is why a rename table
applies as a whole (`VER.5`). Cambria (Litt et al., Ink & Switch 2020) argues for translating on read, not on write,
which is the pack rule. OpenUSD and OpenMW drop very old versions after a deprecation; this format cannot, because a
base set may be the player's only copy.

## 7. Layering: per field, explicit, keyed

**Decision.** Strongest layer wins per field; `merge` must find its item, `add` must not; `disabled` instead of
delete; keyed lists; `revert` to the base value ([layering.md](layering.md) §3).

**Evidence.** OpenUSD resolves each field to the strongest opinion (`pxr/usd/usd/stage.cpp:6185-6200`), deactivates
instead of deleting with the strongest `active` opinion winning (`:1006-1017`), and composes child order weakest to
strongest, appending new names (`pxr/usd/pcp/composeSite.cpp:486-505`) — the order rule of layering §3.3. Whole-record
replacement (OpenMW `apps/openmw/mwworld/store.cpp:195-206`; Bethesda plugins) makes untouched copies of base values
override other mods ("dirty edits", https://tes5edit.github.io/docs/7-mod-cleaning-and-error-checking.html). Matching
list elements by position corrupts data when a list changes; keys fix it (Bohannon et al., "Boomerang", POPL 2008),
hence pack-added elements are keyed by pack, never by merged index. CSS's `revert-layer` is the model of `revert`.

**Departure.** OpenUSD's `over` on a missing prim and a list delete of an absent value are silent no-ops; here they are
errors (`MRG.1`, `MRG.4`), because a pack made for another base set must say so instead of quietly doing nothing
(RFC 9413 again).

## 8. All or nothing per pack, transitively

**Decision.** One refused file or one stale reference refuses the whole pack; dependents of a refused pack are refused
and told why ([conventions.md](conventions.md) §11, `ORD.4`). Legacy packs keep today's skip-and-count.

**Evidence.** OpenUSD does not open a layer that fails to parse and records it as an error in the layer stack
(`pxr/usd/sdf/usdaFileFormat.cpp:312-333`, `pxr/usd/pcp/layerStack.cpp:1833-1856`); Godot and Ship of Harkinian
refuse packs and archives as units; OpenMW aborts the whole load on any unknown record. Per-asset failure (Godot,
Bevy) suits editors, where a broken asset is visible; in play a partly applied mod changes the game without a word,
so the format refuses at pack level and Studio lists every failing item for repair.

## 9. Extensions: used, required, safe to copy

**Decision.** `extensions_used` / `extensions_required`; undeclared extension data refused; safe-to-copy flags for
tools that rewrite ([conventions.md](conventions.md) §9).

**Evidence.** glTF: required ⊆ used, an extension object not declared is an Error, a required extension that is never
used is an Error (glTF-V `lib/src/utils.dart:544-551`, `context.dart:199-216`). PNG's chunk property bits
(https://www.w3.org/TR/png-3/ §5.4) and EA IFF 85 ("do not edit a file you don't know how to create") separate what a
reader may skip from what an editor may copy. ext4's compat / incompat / ro-compat feature flags make writers
stricter than readers. RFC 9170 (2021): extension points rust shut unless exercised, so the conformance fixtures
include made-up "used" and "required" extensions that must load with a warning and be refused.

## 10. Digests and content identity

**Decision.** Algorithm-prefixed digests; SHA-256 for anything compared between machines, XXH3-128 for local caches;
list digests over defined bytes with a domain tag; size with published references ([conventions.md](conventions.md)
§7).

**Evidence.** OCI descriptors (`algorithm:encoded`, size checked first,
https://github.com/opencontainers/image-spec/blob/main/descriptor.md); multihash/multicodec (the name `xxh3-128` is
registered there); Git's tree hashing; xxHash's own README forbids it where collisions can be crafted. Git's move from
its original 160-bit hash to SHA-256 needed a format version and a repository extension: here a new algorithm is a schema minor.

## 11. The cooked cache: keyed by what was read

**Decision.** Model entries keyed by a verifying trace: for every path looked up, the ordered list of layers and
digests that contributed (or "not found"), checked against the effective layer stack, plus a layout hash generated from the stored types, a cooker version and a salt ([cooked.md](cooked.md) §3–4).

**Evidence.** "Build Systems à la Carte" (Mokhov, Mitchell, Peyton Jones, ICFP 2018): verifying traces record the
inputs a task used, giving minimal rebuilds and early cutoff. Dolstra's thesis (2006): a store path hashes all inputs
including the build script — the role of the cooker version and layout hash. Bazel's `Action.salt` disowns results of
a faulty build. Godot re-imports when the source hash, the destination hash or the importer version changes
(`editor/file_system/editor_file_system.cpp:670-746`); Bevy keys processed assets by BLAKE3 of source and
dependencies but has no loader version (`crates/bevy_asset/src/loader.rs:31-70`), so a loader change leaves stale
output — the gap the layout hash and the golden-output test close.

## 12. Bundles: aligned, stored ZIP files

**Decision.** One ZIP bundle per load unit; payload entries stored and 64-byte aligned for zero-copy mapping; text
entries deflated; identity by content digest, not bundle bytes ([layout.md](layout.md) §4).

**Evidence.** USDZ is a stored ZIP whose entries start at multiples of 64 bytes and are read by pointer into the
mapping (USD `pxr/usd/sdf/zipFile.cpp:687-724`, `usdzResolver.cpp:158-229`); Android's zipalign aligns stored entries
for `mmap` and its alignment extra field (0xD935) is the one used here; Apache Arrow recommends 64-byte buffer
alignment. Ship of Harkinian copies every entry out of its archive (LUS `O2rArchive.cpp:43-90`), which costs what
mapping saves. Reproducible-builds.org: sorted names and fixed times make archives reproducible, but deflate output
varies between implementations, so identity rests on the uncompressed content. One file per unit rather than tens of
thousands of small files: [performance.md](performance.md) §3.5.

## 13. Floats in text

**Decision.** Shortest round-trip digits in a fixed text form; a decimal's binary32 value defined as
round₃₂(round₆₄(x)), which is what common TOML and JSON libraries already compute; a `bits:` escape, required for the
two values where that differs from a direct parse ([conventions.md](conventions.md) §3.4).

**Evidence.** Ryū (Adams, PLDI 2018) and Steele & White (1990) for shortest output; Clinger (1990) for correctly
rounded input; Regan (2020) and an exhaustive test over all 2³² binary32 values (two values, ±7.038531e-26, read wrong
through binary64); Figueroa (1995) shows double rounding harmless for arithmetic, not for decimal parsing. TOML 1.0
defines floats as binary64, so defining the binary32 value through binary64 keeps every TOML reader conforming
without a custom hook; the two exceptions cost one escape each.

## 14. Lossless import: the rebuild invariant

**Decision.** Every original byte is either re-encodable from decoded items or kept ([residual.md](residual.md)).

**Evidence.** The lens laws (Foster et al., TOPLAS 2007): GetPut (putting back an unchanged view restores the
source) and PutGet; the residual is the lens's complement. Round-trip property tests are among the most valuable in
practice (Goldstein et al., ICSE 2024). EverParse (USENIX Security 2019): formats with exactly one encoding per value
make "parse is the inverse of serialize" provable — why padding must be zero and booleans 0 or 1.

## 15. Order and ids are the original's

**Decision.** Original names, hashes and positions are the keys; order is data ([conventions.md](conventions.md) §6.5).

**Evidence.** OpenMW and Bethesda plugins remap numeric ids by load order (OpenMW `components/esm/formid.hpp:13-27`,
`esmreader.cpp:88-99`), so a reference's meaning depends on the player's order — avoided here. Ids derived from the
original's own data are stable across machines and editions where the data agrees, and a guard catches where it does
not.

## 16. Performance before authoring convenience

The format is read by every player at every start; Studio is used by few. Where the two conflict — one bundle per unit
instead of loose files, stored payloads instead of compressed ones, binary arrays instead of TOML for large numeric
data, a cooked cache instead of parsing text at load — the player's load time wins, and authoring convenience is
provided by tools (`base export`, Studio) instead of by the stored form. The budgets and the gate are on
[performance.md](performance.md).
