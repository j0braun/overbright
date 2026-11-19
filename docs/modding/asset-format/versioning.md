# Asset format: versions, compatibility promises, migrations, docs in step with code

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md)
apply.

Prior art taken: glTF's major/minor rules and required extensions
(https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#versioning); Minecraft's supported format ranges
(https://minecraft.wiki/w/Pack.mcmeta); Factorio's one-shot rename migrations recorded in saves
(https://lua-api.factorio.com/latest/auxiliary/migrations.html); VCMI's compatibility identifiers; Ship of
Harkinian's and OpenGOAL's regeneration of the extracted data after an update without asking for the original
again (https://github.com/HarbourMasters/Shipwright/blob/develop/docs/VERSIONING.md,
https://github.com/open-goal/launcher). Avoided: OpenGOAL's dependence on a kept copy of the original
(`iso_data`), whose deletion broke updates; the residual's rebuild invariant replaces it ([residual.md](residual.md)).

## 1. The version numbers

| What | Where | Form | Changes when |
|---|---|---|---|
| Manifest format | `pack.toml` `format` | integer (2) | the manifest's shape or the layering rules change incompatibly |
| Kind schema | every TOML file's `schema` | `ob.<kind>/<major>.<minor>` | §2 |
| glTF extensions | glTF `extensions` | `OB_<name>` + its own `version` field | as a kind schema |
| Bundle profile | [layout.md](layout.md) §4 | fixed for manifest format 2 | only with a new manifest format |
| `order.txt`, `active.txt` | first line | `v1` | their syntax changes |
| Required-items profile | `[base] profile` | `nfsu2/<n>` | the engine starts to require a new fixed item |
| Import epoch | `import.toml` `importer.epoch` | integer | the importer's output for the same original changes (new decoder, new encoder, a fix) |
| Cooked framing, layout hash, cooker version | cache files | [cooked.md](cooked.md) | any change; no promise |
| Overbright | `overbright` ranges in manifests | semver | each release |

## 2. Kind schema versions

- **Minor** versions only add: an optional field whose default reproduces the earlier behaviour exactly, a new enum
  value, a new optional file kind, a new allowed operation. Nothing is removed, renamed or re-interpreted.
- A minor MAY **rename** a field through an alias, as Avro's reader aliases do
  (https://avro.apache.org/docs/1.12.0/specification/#aliases): the new name's `x-since` is that minor, readers
  accept the old name forever and map it to the new one, writers write the new name, and a file holding both is
  refused (`CONV.4`). Giving an open field (`u_0x1c`) its meaning is such a rename. Renames are frequent in practice
  (about one schema change in six in Wikipedia's history, Curino et al., ICEIS 2008), so they must not force majors.
- A field name, once released, is **never reused** with another meaning, in any later version (Protocol Buffers'
  `reserved` rule): the kind's schema keeps a list of retired names.
- **Major** versions are everything else: a removed or renamed field, a changed type, unit or meaning, a changed
  default, a changed key.
- Each field and enum value carries the minor version that introduced it (`x-since` in the JSON Schema, a column
  in the generated field table).

### 2.1 The file declares the lowest version that reads it

A writer writes `schema = "ob.<kind>/<major>.<m>"` where `m` is the highest `x-since` among the fields and enum
values the file actually uses, not the newest version the writer knows. This is what glTF's `asset.minVersion`
expresses (glTF 2.0 §3.2) and what OpenUSD's binary and text writers do in code: they start at a low version and
raise it only when a value needs a newer one (`pxr/usd/sdf/crateFile.cpp`, `usdaFileFormat.cpp`; Godot's text
resource writer likewise). Readers check the declaration: a field or enum value newer than the declared minor is
refused (`VER.6`), so a faulty writer cannot produce files that new releases accept and old ones misread. So a file that uses nothing new stays
readable by older Overbright versions, and a file that does use something new is refused by them with a precise
reason ("needs Overbright 0.7 or newer: field `wet_grip_scale`, schema ob.handling/1.3") instead of being read
wrongly.

This deliberately differs from glTF, where readers ignore unknown properties: here unknown keys are refused
(conventions §3.2, `CONV.4`), because a misspelt key must never be a silent no-op, and because ignoring a
gameplay field would change play without a word.

## 3. Compatibility promises

| # | Promise |
|---|---|
| P.1 | **A base set is never stranded.** Every later Overbright reads every base set an earlier release wrote, directly or through a migration (§4), without the original game folder. |
| P.2 | **Packs keep working.** Every later Overbright reads every mod pack of every manifest format and kind major it ever read, migrating in memory (the files on disk stay as they are until their author saves them in Studio). A pack stops loading only when its own `overbright` range excludes the release. |
| P.3 | **Same data, same play.** A file read by a newer release gives the same merged values as before, unless a release note names a bug fix in a reader (which also bumps the import epoch or the kind's cooker version so caches rebuild). |
| P.4 | **Writers are conservative.** They write the newest major and the lowest sufficient minor (§2.1), and never rewrite a file its author did not ask to change. |
| P.5 | **Native saves stay native.** Profile saves keep the original game's own format; nothing of the asset format enters them (pack references go to a sidecar, [layering.md](layering.md) §8). |
| P.6 | **No promise for cooked data** ([cooked.md](cooked.md) §6). |
| P.7 | **Rule ids are stable**: a rule id keeps its meaning forever; a changed rule gets a new id. |

## 4. Migrations

### 4.1 Schema migrations (kind major N → N+1)

A pure function from a file of major N to major N+1 (and from N+1's in-memory model back is not required). Kept in
code for every major ever released (`assets::migrate::<kind>::v<N>_to_v<N+1>`), each with fixtures of real old files
(synthetic, no game data) and a test that the migrated file reads to the expected values.

- **Mod packs**: migrated in memory on every read; Studio offers "Update the pack's files" (a normal edit, undoable).
- **Base sets**: migrated on disk once, bundle by bundle: each bundle is rewritten to a temporary file and renamed
  into place, then `import.toml` is updated last. An interrupted migration resumes (each bundle's state is in
  `import.toml`'s journal). Space needed: about one bundle's size again (≈ 0.3 GB per bundle at most) [guess].
  The first start after an update is not slower than today's: a unit not yet migrated MAY be migrated in memory when
  it is first used, followed at once by a background re-cook and the bundle's rewrite. Migrations, like every other
  background job, pause themselves while the game loads or streams ([import.md](import.md) §5).

### 4.2 Import epochs (re-deriving a base set)

When an update raises the importer's epoch (a kind graduates from stage 0, a new decoder, a new encoder, a fixed
decoder; [import.md](import.md) §4.1):

Units still at stage 0 graduate from their kept bytes in the base set, never from the game folder
([import.md](import.md) §4.1); the steps below apply to units already decoded.

1. If the original folder is still there and its fingerprint equals the one in `import.toml`, the changed import
   units are re-imported from it.
2. Otherwise each affected original file is **rebuilt from the base set** ([residual.md](residual.md) §4: decoded
   items re-encoded where proven, kept bytes elsewhere), its `content_digest` is checked, and the new importer runs
   on the rebuilt file. The result is identical to importing from the original (the rebuilt content *is* the
   original's content, by digest).
3. Shadow copies whose encoder the new epoch proves are dropped ([residual.md](residual.md) §3.2).

Units whose importer did not change are not touched. After any on-disk migration of a base set, the rebuild check
([residual.md](residual.md) §4) runs again for every original file whose items changed: a migration that alters
decoded items must not silently break the rebuild invariant. The launcher shows it as "Updating your game data" with the
import's progress protocol ([import.md](import.md) §9).

### 4.3 Key renames

Item keys never change for original items (they are the original's names, hashes and indices). Keys Overbright
defines itself (language keys, unit names, pack-added ids) may be renamed only by a major version, with a rename
table `assets::migrate::renames` (`kind`, `from`, `to`, `since`) applied to pack references, `order.txt` and save
sidecars. A pack MAY carry `migrations.toml` at its root (`ob.pack-migrations/1.0`: `[[rename]] kind, from, to, since`) for keys it
added itself, so saves made with its older version still resolve. Applied renames are recorded in the save sidecar
and never applied twice (as Factorio does).

## 5. Deprecation

A kind major is **written** until the next major ships and **read** forever (P.1, P.2). Removing a migration is never
allowed. The manifest's `format` 1 (today's texture packs) is read forever as a legacy pack
([manifest.md](manifest.md) §6).

## 6. Keeping the documentation and the code in step

The specification is generated from, and checked against, the code that reads it, so the two cannot drift:

| Part of the spec | Single source in code | Generated / checked by |
|---|---|---|
| Field tables of TOML kinds | the Rust model types with `serde` + doc comments + `#[since = "1.2"]`-style attributes | a generator writes each page's field tables between `<!-- generated:fields ob.<kind> -->` markers |
| JSON Schemas (`docs/modding/asset-format/schemas/`) | the same types (`schemars`-style derivation [to verify: crate and licence]) | regenerated and diffed |
| Binary layout tables | `#[repr(C)]` + `bytemuck::Pod` structs with a derive that emits the table (offset, size, type, field) | regenerated and diffed; a test checks `size_of` and offsets |
| Rule ids and texts | one `Rule` enum (`id`, text, page) used by every refusal | the pages' rule tables generated; a test fails on a rule id in docs but not in code or the other way round |
| The required-items profile | the `assets::required` registry | `profiles/nfsu2-<n>.toml` regenerated; a test fails when the engine requests a fixed name outside the registry |
| Examples | `docs/modding/asset-format/examples/**` | every example validates against its schema, reads with the Rust reader, and writes back byte-identical |
| Conformance fixtures | `tests/fixtures/asset-format/` (synthetic packs, no game data): one valid pack per kind and one invalid file per rule id with the rule it must trigger | the reader's tests; usable by third-party tools |
| Version table | the readers' `SUPPORTED` constants | README's table generated |
| No game-folder access | the asset layer is the only code that opens game content; the game crate does not depend on the importer | a layering check (`scripts/checks/layering.sh` rule) fails on a planted game-path open or a dependency on the importer crate |

One check script (`scripts/checks/asset_format.sh`, part of `checks.sh`) regenerates everything into a temporary
folder, diffs it against the tree, validates examples and fixtures, and fails on any difference.

**Implemented so far** (`crates/ob-asset-format`, module `docs`): the rule registry and every page's rule table
(a test also fails on a rule id a page names that the registry lacks, and on a kind the README's registry names
that references cannot); the field tables of `pack.toml` and the schemas `pack.schema.json` and
`pack-migrations.schema.json`, made from the same field descriptions the reader checks files with; the manifest
examples read and write back byte for byte. The check runs as the crate's test `docs_in_step` (so in every
`cargo test`) and as `scripts/checks/asset_format.sh` (not yet a step of `checks.sh`). The rest of the table above
follows with the kinds' readers. A change to a
reader without its docs change, or the other way round, fails the check, so format changes and their documentation
land in the same commit.

## 7. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| VER.1 | A file's schema minor is higher than this reader knows (the message names the release that added it, when known). |
| VER.2 | A file's schema major is unknown (neither readable nor migratable). |
| VER.3 | A migration failed (the file is refused with the migration's own rule id). |
| VER.4 | A base set's import epoch is newer than this Overbright (an older release started on a newer base set): the launcher refuses and says which release made it. |
| VER.5 | A rename table, applied as a whole (all renames at once, so two names may swap), leaves two items with one key. |
| VER.6 | A file uses a field or enum value introduced after the minor version it declares (under-declared `schema`). |
<!-- /generated -->
