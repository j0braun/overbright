# Tracing round: cars

What a complete car is in `SPEED2.EXE`, what refers to a car and how, and which limits the original has on
the number of cars — the ground for designing add-on cars. Tags as in [docs/formats.md](../formats.md)
(**verified**, **inferred**, **open**). Addresses are `SPEED2.EXE` virtual addresses. The details of each
piece are in the format chapters; this page links them and adds what this round traced.

Sources: a static trace of the executable and a read of the reference data; nothing was run.

## 1. Summary for add-on car design

* A car is **one record in a fixed table of 46** (GLOBALB chunk 0x34600, 0x890 bytes each) plus files named
  after the record's name. The 46 is built into the program in at least eight places (§4); a 47th record is
  never read, and two code paths would crash or write past a fixed array if a car outside the 46 were named
  (§4.2). Add-on cars therefore need the engine's own car registry; they cannot be added as data to the
  original's tables.
* Most references to a car are **by name hash** (`bin_hash(name)`, record +0xD0): saves, unlocks, presets,
  the part database's groups, the slot-group overrides. That is good for add-ons: a stable name gives a
  stable id across packs. The **type index** (0–45) is used by fixed tables in the program (§5) and as
  the car's id at run time (record +0x840).
* Everything a car needs beyond its record is data keyed by its name: geometry (meshes, part LODs, the
  collision hull, markers), textures and the skin, vinyl images, part animations, the parts the part
  database lists for its group, the AI upgrade presets, an unlock record, logos and strings (§2).

## 2. What a complete car consists of

| Piece | Where (original) | Keyed by | Details |
|---|---|---|---|
| Car-type record | GLOBALB 0x34600, record *i* = type index *i* | position; name at +0x00, `bin_hash(name)` at +0xD0, id at +0x840 | [cars-1.md "Cars"](../formats/cars-1.md#cars-in-progress); handling, wheels, engine, gearbox, cameras, upgrades are fields of it |
| Geometry | `CARS\<name>\GEOMETRY.BIN` / `.LZC` (paths stored in the record, +0x40 / +0x60) | record | body and kit parts as solids `<name>_<part>_<LOD>` (LODs `_A`…`_D`), markers, the collision hull `<name>_CV` and wheel volumes `_W0n_CV` |
| Textures | `CARS\<name>\TEXTURES.BIN` | name | skin layers (`<name>_SKIN1`, `_DOORLINE*` and masks), badges, lights, rim, tyre |
| Vinyl images | `CARS\<name>\VINYLS.BIN` | name | read only for non-traffic types and only if present (§3.3) |
| Part animations | `CARS\<name>\PARTS_ANIMATIONS.BIN` (0x61C68B) | name | hood, trunk, doors ([cars-1.md, part animations](../formats/cars-1.md#cars-in-progress)) |
| Parts | GLOBALB 0x80034602 part database; group hash = `bin_hash(name)` | name hash | per slot the parts of the car's own group, then shared groups (§3.1) |
| Slot groups | GLOBALB 0x34607: 0xAA × {group 0, group 1} defaults, then 16-byte per-car overrides | name hash | §3.1 |
| AI upgrade presets | GLOBALB 0x80034A30 → one 0x34A31 chunk per type, in type order | position | §4.1 (fixed 46-entry table) |
| Unlock | career chunk 0x34A1F, 12 bytes {car hash, event (region 0), event (other regions)} | name hash | §4.2; [career-1.md](../formats/career-1.md) |
| Preset rides | GLOBALB 0x30220 (0x338 bytes; car named at +0x08) | name (hashed) | §4.2 |
| Engine sound | program table by type index (31 racer types) | index | §5 |
| Logos | front-end textures `CARSELECT_MANUFACTURER_<make>` (make = record +0xC0) and `SECONDARY_LOGO_<name>` (0x511E60) | strings | [front-end.md, car lot](../formats/front-end.md) |
| Display name | string `CAR_NAME_<name>`; the make from `bin_hash(<name>)` | name | [career-1.md](../formats/career-1.md) |
| Colour names | GLOBALB 0x34601, 64-byte records {type id, skin, …} looked up by 0x6101B0 | type id | read only for results records; the car draws drop the result |

Shared, not per car: light materials (GLOBALB 0x135200), the shared part files (`CARS\BRAKES`, `EXHAUST`,
`ROOF`, `SPOILER*`, `MIRRORS*`, `PLATES`, `AUDIO`, `WHEELS\GEOMETRY_<brand>.BIN`), decal textures
(`CARS\TEXTURES.BIN`), and the `DUMMY_SKIN<n>` composite targets.

### 2.1 Part slots

* 0xAA slots per car (`ride +0x590 + 4·slot`); the slot → part type table is in the program (0x802960)
  (**verified**, [cars-1.md "Part database"](../formats/cars-1.md#cars-in-progress)). The slot count is
  fixed: the per-slot loops end at 0xAA (e.g. 0x61C270), the save keeps 0xAA part words per car.
* Body / kit slots 0x00–0x3E draw through the generic part draw; decal placements 0x34–0x3E; paint 0x3F
  and part paints 0x45–0x4C; vinyl layers 0x40–0x43; decals 0x61–0x98; performance parts are separate
  (ride +0x2F0).
* A part draws the model `<group><middle><name>_<LOD>`; the group is the car's name for its own parts.
  Which slots a car can fill is decided by the parts that exist for its group (or a shared group) in the
  part database — no per-car slot list.

### 2.2 Wheels

Four 0x30-byte wheel records at record +0x120 (position, radius +0x10, tyre width +0x14, face y +0x1C,
face y with a wide body +0x20); rim size class at +0xDC, AI rim range +0xDD / +0xDE
(**verified**, `src/car/record.rs`). Rims are shared models (`CARS\WHEELS\GEOMETRY_<brand>.BIN`) scaled
to the wheel; the wheel draw and LOD rules are in [cars-1.md "Wheel draw"](../formats/cars-1.md#cars-in-progress).

### 2.3 Detail levels

Four LODs `_A`…`_D` per model, picked from the car's size on screen (tables 0x7A66E4 in game, 0x7A66F4 in
the front end and for traffic), with per-slot gates (wheels only up to LOD 2, some slots only at 0)
(**verified**, [ai.md "Car LOD"](../formats/ai.md)). A model with no solid for a LOD draws nothing at that
LOD (0x6230C6), so an add-on may ship fewer LODs and simply vanish earlier; generating or flagging them is a
tool decision, not a format one.

### 2.4 Collision

The car body collides as the convex hull `<name>_CV` from its GEOMETRY.BIN (chunk 0x80034020); without one
the program uses a fixed box (0x5AED76). Mass, inertia and the box edge lengths are record fields (+0x220…)
(**verified**, [cars-1.md "Contacts"](../formats/cars-1.md#contacts-verified-except-where-noted) and
"Chassis"). An add-on car needs a hull with the original's vertex layout (48-byte vertices, position at +16).

### 2.5 Vinyl space and logos

* The body is textured through `<name>_SKIN1`, which for racer types (record +0x88C ≠ 0) is replaced by a
  run-time composite `DUMMY_SKIN<n>` (0x61C290; string 0x7A866C) built by 0x61D7A0: paint fill, vinyl
  layers 1–4 (`<name>_<TEXTURE>` and `_MASK` from VINYLS.BIN), then the panel-line layer
  `<name>_DOORLINE[_KIT|_WIDEBODY]` (**verified**, [cars-1.md "Skin compositor"](../formats/cars-1.md#cars-in-progress)).
* The composite is DUMMY_SKIN's size (512²); a layer of another size is skipped. So a car's "vinyl space"
  is its body UVs into one 512² image; every vinyl of the car is authored for that car's UV layout
  (vinyls are per car, not shared). An add-on car with new UVs needs its own vinyl images.
* Seven marker texels in the skin carry part-paint colours (cars-1.md "Part-paint marker texels"): an
  add-on skin must keep those texel positions free.
* Decals are separate meshes (`%s_DECAL_*` models in slots 0x34–0x3E, textures `DUMMY_DECAL1`…`8`), not
  part of the skin.
* **Open:** how many `DUMMY_SKIN<n>` targets exist (the slot number is passed in by 0x61C290's callers);
  that bounds how many composited cars can be on screen.

### 2.6 Tuning and performance tables

The record holds the stock handling and the upgrade deltas (`upgrades` in the asset format); the career
container holds the performance packages, costs and the tuning unlocks (0x34A1A); the dyno settings per
car (five modes × 0x29 bytes) are saved with the car. All are per-record or keyed by part, so an add-on
car brings its own record values and uses the shared performance packages. The 0–60 / 0–100 display
factors are program tables by type index (§5).

## 3. How the original finds a car's data

### 3.1 Part groups (verified)

0x6101F0(slot, type): `h = bin_hash(record name)`; search the 0x34607 per-car overrides {u32 car hash,
u32 slot, u32 group 0, u32 group 1} (16 bytes, after the 0x550-byte default table; count = (size − 0x550)
/ 16) for (h, slot); else take the slot's two default groups, a group 0xFFFFFFFF standing for `h`. The
part for a slot is then the first part of the slot's type in the first group that has one. **Data only,
keyed by name hash**: an add-on car with its own name gets its own group and overrides without program
changes.

### 3.2 Name → type (verified)

0x610140(hash) walks the 46 records comparing +0xD0 and returns the record, or **NULL** when no record has
the hash (the loop bound is the constant 0x2E). Callers read +0x840 (the type id) from the result; the
owned-part bits use 0x2E when the part's group is no car (saves-options.md "Owned visual parts").

### 3.3 Per-car files (verified)

* Vinyls: the car loader builds `CARS\%s\VINYLS.BIN` (string 0x7A8D6C, 0x63AFCC) from the record's name,
  only when record +0x844 ≠ 2 (not traffic, 0x63AFBD) and only when the file exists (0x57CAD0). Folders
  under `CARS\` that are not car types are never read: the reference install has six such VINYLS.BIN
  files (IMPREZA, LANCER, NEON, S2000, SPINNER, MIRRORS, in the older version-4 layout; UG2's types are
  IMPREZAWRX and LANCEREVO8), and the original never opens them.
* Part animations: `CARS\%s\PARTS_ANIMATIONS.BIN` (0x7A867C, 0x61C68B).
* Geometry: the two paths stored in the record (+0x40, +0x60).

## 4. Limits on the number of cars

### 4.1 Fixed at 46 (verified)

| Where | What is fixed |
|---|---|
| 0x636BF0 (chunk 0x34600 loader) | fix-up loop over 0x189E0 bytes = 46 records; the chunk's size is not read |
| 0x610140, 0x61C670, 0x6372B7 | name-hash lookups end at 0x2E |
| 0x513C50 (car-lot unlock filter) | a 46-byte availability array on the stack (`rep stos` 11 dwords + 1 word), indexed by the unlocked car's id (+0x840) |
| 0x516537…0x5165C4 | the stock-car list (0x516500) walks 0x189E0 bytes |
| 0x41AAF0 | the traffic list walks 0x189E0 bytes (types with +0x844 = 2) into a fixed array of 0x920-byte entries in its owner object (capacity **open**) |
| 0x520755…0x5207CB | a name list `CARTYPENAME_ANY` + every type (with +0xD0) walks 0x189E0 bytes |
| 0x60934F, 0x6099B8 | a CRC-32 (0x43BCD0) over the 0x189E0-byte table (use **open**; **inferred**: a consistency check) |
| 0x59947C (chunk 0x34A31 loader) | AI presets go to a fixed 46-entry table at 0x86B0C8 (8 bytes: count, pointer); chunks past the 46th are dropped |

### 4.2 What would break with a car outside the 46 (verified unless marked)

* **Unknown name hash → NULL dereference.** 0x513C50 (car-lot unlocks) calls 0x610140 with each 0x34A1F
  record's car hash and reads `[result + 0x840]` without a NULL check (0x513CC7…0x513CCC); the preset ride
  setup 0x637280 does the same with the preset's car name (0x6372A0…0x6372C9: on a miss it reads
  `[0 + 0x840]`). An unlock or preset that names a car not in the table crashes the original.
* **Id ≥ 46 → stack overwrite.** The 46-byte array in 0x513C50 is written at `[esp + id + 0x10]`
  (0x513CD5): an id past 45 writes over the caller's stack.
* **Saves** name the car by `bin_hash(name)` (saves-options.md, car record +0x08), so a save holding an
  add-on car loaded without it meets the NULL path above (**inferred**: the load path's handling of a
  miss was not traced).
* **Program tables by index** (§5) have 31 or fewer entries; an index past them reads neighbouring data
  (**inferred** for each table; only the sound switch 0x459240 has a default).

### 4.3 Other counts

* Part database: group index is a byte (part +7, ≤ 256 groups; 75 used), models / names / attribute lists
  are u16 indices (**verified** field sizes, `src/formats/parts.rs`).
* Slots: 0xAA per car, program-wide.
* Racer vs traffic: record +0x844 (0 racer, 2 traffic); 31 racer types and 15 traffic types in the
  reference data; traffic per zone is capped by record +0x854.

## 5. Program tables keyed by type index

All **verified** in the cited code; an add-on car needs an engine-side value for each.

| Table | Address | Entries |
|---|---|---|
| Car-lot exclusions by region | 0x7F7C50 (CIVIC, RSX) / 0x7F7C48 (CORSA, PEUGOT106), 0x5022C0 | 2 |
| Quick Race always-offered cars by region | 0x7F7C28 / 0x7F7C08, 0x529CD0 | 8 |
| Random opponents' cars | 0x7F6DA4 | 26 |
| Quick Race default cars | 0x7FAA48 / 0x7FAA4C | 2 |
| Engine sound index | switch 0x459240 (default for unknown types) | 31 |
| 0–60 / 0–100 factors | 0x7FC120 / 0x7FC1D8 | 31 |
| Hatch types (rear-window decal on the trunk) | 0x60C900 | type list |
| Closed-roof types (sunroof replaced under a roof scoop) | 0x60C860 (3, 5, 0x12, 0x17) | 4 |
| HUMMER special cases (headlight on the hood) | type 8, in the part draw | 1 |
| Part exclusion groups (EXCLUDE_UG1 / UG2 / SUV) | hash lists 0x610030 / 0x6100B0, 0x61B680 | hashes |
| Region logos (secondary badge per region) | Corsa / Miata / Tiburon, 0x511E60 | 3 |

## 6. Open points

* The `DUMMY_SKIN<n>` pool size (§2.5).
* The traffic list's capacity in 0x41AAF0's owner (§4.1).
* The use of the car-table CRC (§4.1).
* How a profile load treats a car hash it does not know (§4.2).
* The record's open fields (asset format: 46 open fields, 465 of 2,192 bytes), among them `u_0x850`
  and the gauge block 0x804–0x840.
