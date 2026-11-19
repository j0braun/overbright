# Asset format: engine tables (the built-in tables and their overrides)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md) and [layering.md](layering.md) apply and are not repeated.

Rule prefix: `TBL`.

## 1. Scope

The original game keeps many numbers, curves and names in its executable: the race tunes, the steering and lateral
curves, the car sound records, the garage camera views, the opponent pools, the codec tables. Overbright carries
them in its own code, `crates/ob-formats/src/exe_tables/**` (docs/exe-tables.md): one process-wide `Tables` holder
(crates/ob-formats/src/exe_tables/tables.rs:40) with 65 tables built once by `Tables::builtin()` (:180) from three sources:

| Source | Module | What |
|---|---|---|
| Formulas | `exe_tables::generated` | Pure maths: arcsine, the mixer's dB curves (and the event VM's sine and the arctangent, used outside the holder) |
| Third-party decoder tables | `exe_tables::third_party` | The standard tables of the UTK, MPEG-1 layer III and VP6 decoders, from open decoders with their notices |
| Tuning constants | `exe_tables::constants::{driving, car_audio, frontend, garage}` | Handling and race curves, sound records, camera views, front-end facts, as named constants |

Nothing is read from `SPEED2.EXE` at run time, and the importer never reads it ([residual.md](residual.md) §5). So
**an imported base set contains no tables**: they are part of the program, identical whatever base set is mounted.
This page defines the one thing the format adds: **override files** with which a mod pack or a community base set
changes table values, the check that keeps them in the built-in shape, and the hook that applies them before the
engine first uses a table.

Not on this page: constants written elsewhere in the code (§3.4), and data that is in the game files (car records,
packages, AI presets: [cars.md](cars.md); career data: [career.md](career.md)). The EA Trax song titles, artists and
albums that the music caption and the jukebox show are this page's `ea_trax` table
(`crates/ob-formats/src/exe_tables/constants/frontend.rs:239`): engine constants, not text data ([text.md](text.md) §1).

**What a base set replaces.** A base set, imported or made by a community, provides the game's data files in this
format. The engine's built-in tables are part of the Overbright program; no base set contains or replaces them. A
community base set that needs other values (other cars, other tunes) carries override files of this page in its own
`tables/` ([layout.md](layout.md) §2); they apply at the bottom of the stack, before any mod pack (§4). An imported
base set has none (`TBL.7`).

## 2. Files and keys

```text
<mod pack or community base set>/
  tables/
    drift_tune.toml          ob.engine-table   one file per table, named after the table
    slot_shots.toml
```

| Item | Key | Notes |
|---|---|---|
| a table | `table:<name>` | `<name>` is the field name in `Tables` (`drift_tune`, `ai_lock`, …), the same name `FIELD_PINS` uses (crates/ob-formats/src/exe_tables/tables.rs:344). |
| one value inside a table | `table:<name>@<path>` | `<path>` is the entry's `at` joined with `/`: `table:drift_tune@0/4`, `table:slot_shots@0/3/1/fov`. |

`tables/` is a unit of its own ([layout.md](layout.md) §2); it has no kind folders and allows no wildcards.

## 3. Source form

### 3.1 `tables/<name>.toml` (`ob.engine-table/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.engine-table/1.0` |
| `table` | string `[a-z][a-z0-9_]*` | – | yes | – | The table's name; MUST equal the file name without `.toml` (`TBL.1`). |
| `op` | enum `merge`, `replace` | – | no | `merge` | `merge`: the `set` entries change parts of the table. `replace`: `value` is the whole table. |
| `value` | the table's shape | – | iff `replace` | – | The whole table (§3.2). |
| `set` | array of tables | – | iff `merge` | – | Entries, applied in file order. |
| `set[].at` | array of (u32 index or field name), ≤ 8 | – | yes | – | The path into the table: array and tuple positions from 0, struct fields by their Rust name. `[]` is the whole table. |
| `set[].value` | the shape at `at` | – | yes | – | The new value of that part. |
| `set[].guard` | the shape at `at` | – | no | – | The built-in value the author changed; when the running program's built-in value differs, the pack or community base set holding the file is refused (`TBL.5`; one stale entry refuses it whole, [conventions.md](conventions.md) §11). Studio always writes it. |
| `made_for` | `sha256:<hex>` | – | no | – | The built-in table's pin (`FIELD_PINS`) when the pack was made; a different pin is a **warning** ("made for another version of this table"), `guard` decides refusals. |

Two entries of one file MUST NOT address the same value or one inside another (`TBL.9`). Limits: one table per file;
a file ≤ 1 MiB; strings ≤ 63 characters of printable ASCII.

```toml
schema = "ob.engine-table/1.0"   # tables/drift_tune.toml: two numbers of the drift race tune
table = "drift_tune"

[[set]]
at = [0, 4]                      # row 0, column 4
value = 0.75
guard = 0.5                      # the built-in value it was changed from

[[set]]
at = [2, 10]
value = 1.25
guard = 1.0
```

```toml
schema = "ob.engine-table/1.0"   # tables/ai_lock.toml, replaced whole (made-up values)
table = "ai_lock"
op = "replace"
made_for = "sha256:9028802df21e737e41ea7f42ff600b5a3653c3d1c355611071959e01e54dcbbb"
value = [0.5, 0.45, 0.4, 0.35, 0.3, 0.25, 0.2, 0.175, 0.15, 0.125]
```

```toml
schema = "ob.engine-table/1.0"   # tables/slot_shots.toml: one garage view's field of view and eye
table = "slot_shots"

[[set]]
at = [0, 3, 1, "fov"]            # cars' table, record 3, the tuple's Shot, its field `fov`
value = 50.0

[[set]]
at = [0, 3, 1, "eye"]
value = [2.5, 1.5, 0.75]
```

Writers SHOULD use `merge` with only the changed entries and their guards (Studio does); `replace` is for a table
an author rewrites entirely.

### 3.2 Shapes: from the Rust types

A table's TOML shape is derived from its Rust type in `Tables`; the schema of a table is its type, so it cannot
drift from the code (§9). The override's values MUST have exactly the built-in shape (`TBL.3`):

| Rust type | TOML | Checked |
|---|---|---|
| `f32` | float, or `"bits:0x…"` (conventions §3.4) | NaN and infinity refused unless the built-in value at that place is one (`TBL.4`) |
| `u8` … `u32`, `i8` … `i32` | integer | the type's range (`TBL.4`) |
| `usize` | integer | 0 … 2³² − 1 and the table's own limit (`TBL.4`) |
| `bool` | boolean | – |
| `&'static str` | string | printable ASCII, ≤ 63 characters (`TBL.4`) |
| `Vec3` | `vec3` | – |
| `[T; N]` | array of exactly N | `TBL.3` |
| `&'static [T]` (the scripted audio patterns) | array of exactly the built-in length | `TBL.3` |
| tuple `(A, B, …)` | array, positional (TOML 1.0 arrays may mix types) | arity and element types |
| struct (`Shot`, `RigLight`, `SkidRecord`, `ShiftRec`, `Rec80`, `Rec84`) | inline table with the Rust field names | in `replace` and whole-struct values every field; through `at` one field |
| `Option<Box<[T]>>`, `Vp6Tables` | – | decoder tables only, never overridable |

Values that index another table are checked against it (`TBL.8`): `launch_index` → `launches`, `skid_class` →
`skid_records`, `engines[].decel` → `decels`, `opponent_cars` → the car types (racers, [cars.md](cars.md)),
`engines[].gin` → an engine recording present in the layer stack ([audio.md](audio.md)); the registry (§9) lists
every such relation with the shapes.

### 3.3 The tables and their classes

Generated from the code (§9); this draft shows the current 65. **Class**: `tunable` may be overridden; `fixed`
is refused in 1.0 (`TBL.2`) and may open in a minor version; `formula`, `decoder` and `derived` are never
overridable.

| Table | Rust type | Class | Read by |
|---|---|---|---|
| `steer_response`, `steer_response_drift` | `[f32; 21]` | tunable | src/drive/tables.rs |
| `lateral` | `[[f32; 6]; 7]` | tunable | src/drive/tables.rs, the AI and standings |
| `drift_tune`, `street_x_tune`, `drag_tune` | `[[f32; 22]; 3]` | tunable | src/race.rs |
| `zero_60_factor`, `zero_100_factor` | `[f32; 31]` (by type index) | tunable | src/handling/mod.rs (dyno times) |
| `surface_weight` | `[f32; 29]` | tunable | src/particles/contact_fx.rs |
| `shake` | `[[f32; 4]; 5]` | tunable | src/chase/** |
| `drift_level_speed`, `drift_level_points` | `[f32; 6]`, `[[i32; 4]; 4]` | tunable | src/drift.rs |
| `drift_virtual_scores` | `[(u16, [f32; 12]); 6]` | tunable | src/drift.rs |
| `ai_lock`, `ai_table_rate_speed` | `[f32; 10]` | tunable | src/ai/driver/mod.rs |
| `asin` | `[(u16, f32); 208]` | formula | src/ai/driver/math.rs, src/audio/mixmap |
| `collision_levels`, `collision_gain` | `[f32; 20]`, `[u16; 289]` | tunable | src/audio/world/collision.rs |
| `db_to_lin`, `lin_to_db` | `[i32; 602]`, `[i32; 512]` | formula | src/audio/mixmap/tables.rs |
| `engine_levels`, `shifts`, `turbos` | `[[u8; 4]; 33]` (by sound index) | tunable | src/audio/car/tables.rs |
| `swtn` | `[(u8, i32, i32); 43]` | tunable | src/audio/car/tables.rs |
| `gins` | `[&str; 43]` | derived (from `engines[].gin`, recomputed after overrides) | src/audio/car/tables.rs |
| `shift_recs`, `launches`, `launch_index` | `[ShiftRec; 16]`, `[(f32, i32, i32, i32, i32); 12]`, `[u8; 43]` | tunable | src/audio/car/transients |
| `skid_records`, `skid_class` | `[SkidRecord; 4]`, `[u8; 32]` | tunable | src/audio/car/skids.rs |
| `sound_index` | `[u8; 31]` (by type index) | tunable | src/audio/engine.rs |
| `engines`, `decels` | `[Rec80; 43]`, `[Rec84; 37]` | tunable | src/audio/engine_chain/records.rs |
| `patterns` | `[&[(u16, u8)]; 15]` | tunable | src/audio/car/scripted.rs |
| `score_events` | `[(i16, i16); 31]` | fixed (music contexts) | src/audio/trax/playlist.rs |
| `trains` | `[(u32, u32); 24]` | tunable | src/audio/world/enviro |
| `utk_rc`, `utk_codebooks`, `utk_commands` | UTK decoder | decoder | src/audio/codec/utk.rs |
| `mp3_huff`, `mp3_linbits`, `mp3_window_rows` | layer III decoder | decoder | src/audio/codec/mp3_tables.rs |
| `vp6` | `Vp6Tables` | decoder | crates/ob-formats/src/formats/vp6.rs, src/movie.rs |
| `in_game_rig`, `default_rig` | `[RigLight; 3]` | tunable | src/render/native/car/lighting.rs |
| `slot_shots`, `vinyl_shots`, `suv_engine_shot` | `[[(u32, Shot); 42]; 2]`, `[[Shot; 7]; 2]`, `Shot` | tunable | src/garage.rs |
| `world_map_zooms` | `[(f32, f32, f32, u32); 6]` | tunable | src/frontend/screens/ingame_map.rs |
| `magazine_lines`, `dvd_lines`, `rating_lines` | `[(u32, u32); 20]`, `[(u32, u32); 10]`, `[u32; 11]` | fixed (career texts and covers) | src/frontend/dday/hints.rs |
| `part_type_names` | `[&str; 0x69]` | fixed (solid names of parts) | src/car/model.rs |
| `slot_types`, `slot_categories`, `vinyl_categories` | `[u8; 0xAA]`, `[u8; 0xAA]`, `[u8; 28]` | fixed (the part database's structure and the native save's layout) | crates/ob-formats/src/formats/parts.rs |
| `opponent_cars` | `[u8; 26]` (type indices) | tunable (region 0's pool only, §3.4) | src/opponents.rs |
| `vinyl_groups` | `[(u8, u32, u32); 28]` | fixed (shop structure) | src/frontend/graphics/graphics_shop.rs |
| `opponent_names` | `[&str; 53]` | tunable | src/frontend/quick_race/race_setup.rs |
| `ea_trax`, `ea_trax_where` | `[(&str, &str, &str); 27]`, `[&str; 27]` | fixed (music, and the playlist a fresh native profile stores) | src/audio/trax/caption.rs (the music caption's title, artist and album), src/frontend/screens/jukebox.rs, src/frontend/career/native_map/fresh.rs |
| `tuning_names` | `[u32; 25]` | fixed (the 25 tuning parameters' labels) | src/frontend/screens/tuning.rs |
| `card_scores` | `[(u32, u32, u8); 120]` | fixed (hardware detection, not content) | src/frontend/settings/gpu_rating.rs |
| `qr_default_cars`, `ranked_rides` | `[u32; 2]`, `[u32; 0x12]` | fixed (written into fresh native profiles) | src/frontend/career/native_map/fresh.rs |

Why the `fixed` ones are refused: they define the structure the native save and the part database follow, or
values a fresh native profile stores; a pack must never change what goes into a native save (the save sidecar
holds pack references instead, [manifest.md](manifest.md)). The `formula` and `decoder` tables define how the base
set's data is decoded: changing them would change what every texture, sound and movie decodes to.

### 3.4 Constants outside the holder

Many traced constants are written in the modules that use them, not in `Tables`, and are **not overridable** in
1.0: e.g. the car-lot exclusions and Quick Race's always-offered cars by region (src/frontend/career/lists.rs:7–14),
the alternate opponent pool (the one region 6 uses) and the SUV pool (src/frontend/career/events.rs:147, :151), the
neighbourhood names and their bits (0x800C38, src/frontend/career/triggers.rs:75), the reputation table by winning
margin and reward class (0x7F7AC8, written as code in src/frontend/career/rewards.rs:53), the minimap icon names
(0x7F4348, src/frontend/screens/ingame_map.rs:142), the AI throttle caps (0x783A28, src/ai/driver/mod.rs:104), the
part-exclusion groups (crates/ob-formats/src/formats/parts.rs:34–39), the AI looks' chances, colour sets and paint matches
(src/ai_looks.rs:28, :52, :63), the nitrous curve (src/nitrous.rs:12), the hatch and closed-scoop type lists
(src/car/parts.rs:288, src/car/lod.rs:38), the shared car part files (src/car/assets.rs:240). Note that `opponent_cars` holds region 0's
pool only, so an override of it changes nothing on a European install until the region-6 pool joins the holder.
Moving a constant into `Tables` (with its pin) is how it becomes a table of this page (§9).

## 4. Layering

| Operation | Allowed | Notes |
|---|---|---|
| `merge` (`set` entries) | yes, for `tunable` tables | Each entry replaces the value at its path. |
| `replace` | yes, for `tunable` tables | The whole table. |
| `add`, `disabled` | no | A table cannot be added, removed or hidden; the schema has no such fields. |

**Order.** Overrides are applied to `Tables::builtin()` in stack order, weakest first: a community base set's
`tables/` first, then the lowest enabled pack, and so on up; within a file in entry order. A stronger layer's write
to a path replaces whatever weaker layers wrote there and inside it. A `replace` is a write at `[]`.

**Clashes** ([layering.md](layering.md) §4): two packs writing the same path, or paths one inside the other, clash;
the stronger wins and the launcher, Studio and `pack clashes` show both values per `table:<name>@<path>`.
A pack overriding a value that a community base set already overrides is not a clash (base set versus pack,
[layering.md](layering.md) §4).

**Where.** `tables/` in a mod pack or in a community base set ([layout.md](layout.md) §2). An imported base set has
no `tables/` folder (`TBL.7`): the importer never writes one, because the tables belong to the program, not to the
game files. A community base set's overrides follow every rule of this page (classes, guards, shapes) and apply at
the bottom of the stack, before any mod pack; with "Use mods" off they still apply, as part of the base set.

**Play and saves.** Every `tunable` table can change pictures, sound or simulation, so a mod pack with `tables/`
has `affects_play = true` ([manifest.md](manifest.md)) and is recorded with saves and replays; a community base
set's tables are part of the base set those record. Table values never enter a native save.

## 5. Import

Nothing. The importer reads game content only and skips executables ([residual.md](residual.md) §5); a base set
made by it has no `tables/` and no residual of the executable. Today's mod registries have no table overrides, so
no legacy conversion exists.

## 6. Equality and verification

**With an imported base set alone, the tables are the built-in ones, bit for bit** (Eq1 by construction): no
`tables/` is read, `Tables::builtin().with_overrides(&[])` returns `Tables::builtin()`, and every `FIELD_PINS` hash
(crates/ob-formats/src/exe_tables/tables.rs:344) is unchanged. Tests:

- `every_table_equals_its_pin` (crates/ob-formats/src/exe_tables/tests.rs:218) runs on the holder as installed by the game's start
  path with an empty stack, and a second test compares every float of `with_overrides(&[])` with `builtin()` by
  bits (`Debug` text alone does not show a NaN's payload);
- a fixture pack per class: a `tunable` override changes the table and the simulation hash; a `fixed`, `formula`,
  `decoder` or `derived` one is refused; wrong lengths, types, paths and guards are refused with their rules;
- round trip: Studio's "export as override" of a built-in table, read back, equals the built-in table by bits;
- determinism: the same stack gives the same tables, and the same per-tick state hash, at 30, 60, 120 and 200
  frames per second and on every platform (float parsing per conventions §3.4).

## 7. Cooked form

None. Override files are small and parsed at start (well under 1 ms for typical packs [guess]); the merged
`Tables` lives in memory for the process. The asset cache does not store it; its digest (xxh3-128 over the
canonical list of `(table, path, value bits)` after merging, empty for an imported base set alone) is part of the
identity that saves, replays and the simulation gates record.

**The hook.** Today `tables()` builds the holder on first use behind a `LazyLock`
(crates/ob-formats/src/exe_tables/mod.rs:70–76). With overrides:

```rust
// crates/ob-formats/src/exe_tables/mod.rs (sketch)
static TABLES: OnceLock<Tables> = OnceLock::new();

/// Called once by the game's start path, after the layer stack is mounted and before
/// anything reads a table.
pub fn install(tables: Tables) -> Result<(), AlreadyBuilt> { TABLES.set(tables).map_err(|_| AlreadyBuilt) }

/// Unchanged for every user (`Table<T>` statics): the installed tables, else the built-in ones.
pub fn tables() -> &'static Tables { TABLES.get_or_init(Tables::builtin) }

impl Tables {
    /// Apply merged overrides (weakest first); refuses anything §3 refuses.
    pub fn with_overrides(self, overrides: &[Override]) -> Result<Tables, Vec<TableError>>;
}
```

- **Before first use.** The start path merges `tables/*.toml` of the enabled stack, calls `install`, and only then
  starts the audio threads, the front end and the loads. If the holder was already built (something read a table
  first) and the override set is not empty, the start fails with `TBL.6`: a program error, caught by the fixture
  tests, never a silent mix of old and new values. Tools and tests that never install get the built-in tables.
- **Immutable afterwards.** Tables never change while a process runs: the frame path keeps reading plain memory,
  and the simulation stays deterministic. A changed stack means a new game process (Studio's game link already
  starts the game with an explicit `--mods` stack).
- `gins` is recomputed from the merged `engines` (as `builtin()` does, crates/ob-formats/src/exe_tables/tables.rs:207).

## 8. Errors

| Rule | Text |
|---|---|
| TBL.1 | Unknown table name, or `table` unlike the file name. |
| TBL.2 | The table is not overridable: its class (`fixed`, `formula`, `decoder`, `derived`) is named. |
| TBL.3 | Shape mismatch: a path that does not exist, an array of the wrong length, a value of the wrong type, an unknown struct field. |
| TBL.4 | A value out of its type's range, NaN or infinity where the built-in value is finite, a string too long or not printable ASCII. |
| TBL.5 | A `guard` unlike the running program's built-in value at its path. |
| TBL.6 | Overrides installed after the tables were first used, or installed twice (a program error). |
| TBL.7 | `tables/` in an imported base set (one with `import.toml`). |
| TBL.8 | A value that indexes another table or names an item is out of range or resolves to nothing. |
| TBL.9 | Two entries of one file address the same value, or one inside another. |
| TBL.10 | `op = "replace"` without `value`, a `merge` without `set`, or both `value` and `set`. |

## 9. Versioning and extension

- **Docs and code in step.** A registry in `exe_tables` (one entry per `Tables` field: name, class, shape
  descriptor derived from the type, reference relations, the original's address, the users) generates §3.3 between
  `<!-- generated:tables -->` markers and the reader's shape checks ([versioning.md](versioning.md) §6). A test fails
  when a `Tables` field has no registry entry, when `FIELD_PINS` and the registry name different tables, or when the
  page differs from the generated text. `FIELD_PINS` stays the pin of the values.
- **Minor versions** may move constants into `Tables` (new tunable tables, each with its pin), and open `fixed`
  tables whose save path exists. A table whose built-in shape changes (a fix finds another length) makes old
  overrides of it fail `TBL.3`; such a change ships with a rename or migration entry for paths where one is possible
  ([versioning.md](versioning.md) §4).
- **Add-on cars** ([cars.md](cars.md) §9): every table and constant keyed by type index (`zero_60_factor`,
  `zero_100_factor`, `sound_index`, the opponent pools, the lot and Quick Race lists, the hatch and scoop lists)
  needs a row per added car. The reserved route: such tables gain a `by_car` form, rows keyed by `car:<NAME>`
  instead of by position, filled for the original types from the built-in values and for added types from their
  packs (`[ext.OB_addon_car]` until then). Refused today.

## 10. Open points

- The class of each table is a first proposal: in particular whether `opponent_names`, `in_game_rig` /
  `default_rig` and the audio records should be `tunable` in 1.0, and whether `score_events` may open.
- Region 6's opponent pool and the other constants of §3.4 are outside the holder; overriding `opponent_cars` alone
  is region-dependent until they move.
- Candidates for the registry (§9) among the career constants that are plain Rust today (§3.4): the car-lot
  exclusions and Quick Race's always-offered cars (src/frontend/career/lists.rs:7–14), the alternate and SUV
  opponent pools (src/frontend/career/events.rs:147, :151), the neighbourhood names (src/frontend/career/triggers.rs:75)
  and the reputation reward table (0x7F7AC8, src/frontend/career/rewards.rs:53, today code rather than a table). Each
  needs a class; those that decide what a native save or a fresh native profile holds would be `fixed`.
- Value checks beyond shape and range (a monotonic curve, a sane steering lock) are not defined: any in-range value
  is accepted, as the original would hold it.
- The registry's reference relations (§3.2) are partly listed; the rest come with the registry.

## 11. Sizes and timing

| What | Size |
|---|---|
| The built-in tables | 65 tables; about 4,200 lines of constants in `crates/ob-formats/src/exe_tables/constants/` |
| Building them | about 0.1 ms (docs/exe-tables.md, `exe_tables_cost`) |
| A typical override file | 0.2–5 KB (a few entries with guards); a full `replace` of the largest tunable table (`slot_shots`) ≈ 30 KB [guess] |
| Parsing and applying a pack's overrides | < 1 ms [guess] |
| Imported base set | 0 bytes (no tables) |
