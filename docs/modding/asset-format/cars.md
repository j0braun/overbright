# Asset format: cars (car types, handling, parts, performance, presets, car units)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md), [layering.md](layering.md) and [residual.md](residual.md) apply and are not repeated.

Rule prefix: `CRS`. Field meanings and their tags (**verified**, **inferred**, **open**) come from
docs/formats/cars-1.md ("Cars (in progress)", "Handling", "Data and parts"), docs/formats/cars-2.md and the
parsers cited; the addresses are the reference executable's.

## 1. Scope

| Original data | Where | Kind on this page | Parser today |
|---|---|---|---|
| Car types and handling: 46 records of 0x890 bytes | `GLOBAL/GLOBALB.BUN` chunk 0x34600 | `ob.car-type`, `ob.car-index` | src/car/record.rs:110, :126; src/handling/record.rs:236 |
| AI upgrade presets, one chunk per car type | GLOBALB 0x80034A30 → 0x34A31 (×46) | `ob.car-type` (`ai_preset`) | src/ai/route/mod.rs:444 |
| Rating bounds and class curves | GLOBALB 0x80034A30 → 0x34A32, 0x34A33 | `ob.performance` | src/opponents.rs:120, src/ai/route/mod.rs:228 |
| Preset rides (named cars with parts) | GLOBALB 0x30220 | `ob.car-presets` | src/car/record.rs:322 |
| Part database | GLOBALB 0x80034602 (children 0x34603–0x3460C) | `ob.part-db` | crates/ob-formats/src/formats/parts.rs:110 |
| Slot groups | GLOBALB 0x34607 | `ob.part-db` (`slot_groups`) | crates/ob-formats/src/formats/parts.rs:162 |
| Part cost table | career container 0x80034A10 → 0x34A1C | `ob.part-costs` | crates/ob-formats/src/formats/parts.rs:400 |
| Performance packages | career container → 0x34A15 | `ob.performance` | src/performance/packages.rs:42 |
| Tuning unlocks (what fitted parts open on the dyno) | career container → 0x34A1A | `ob.performance` | src/tuning/parse.rs:14 |
| The car folders | `CARS/<NAME>/`, `CARS/TEXTURES.BIN` | `ob.car-unit` (the unit manifest) | src/car/load.rs:163, src/car/assets.rs:240 |
| Resident copies of the shared part files' headers | `GLOBAL/CarHeadersMemoryFile.bin` | residual only (§5.4) | none (not read) |

`GLOBAL/GlobalB.lzc` is a JDLZ-compressed copy of the same chunk file: in the reference install its unwrapped
content is byte for byte `GLOBALB.BUN`. Overbright reads car types, parts and presets from `GLOBALB.BUN` and the
career container (with its own copy of the car types, for the makes and SUV flags) from `GlobalB.lzc`
(src/frontend/career/data.rs:252, :361, :417). Both files map to the same items (§5.1).

**Not on this page.** The car meshes (`GEOMETRY.BIN`, shared part groups, rims by brand): [meshes.md](meshes.md).
All pixels, including the vinyl images of `VINYLS.BIN` and the shared `CARS/TEXTURES.BIN`:
[textures.md](textures.md); this page only says which records name which textures (§3.8). Part animations
(`PARTS_ANIMATIONS.bin`): [animation.md](animation.md). The light materials (GLOBALB 0x135200): not yet owned by a
kind ([README.md](README.md) §3); parts name them by bare hash. Engine sounds per car: [audio.md](audio.md). Career events, car unlocks (0x34A1F), the
rest of the career container: [career.md](career.md). The AI driver tuning (GLOBALB 0x34B00): [roads-ai.md](roads-ai.md).
Numbers the original keeps in its executable (lot and Quick Race lists, opponent pools, the race tunes, the car
sound index …) are not data at all: they are the engine's built-in tables, [engine-tables.md](engine-tables.md);
§3.9 says which car lists are which.

## 2. Files and keys

```text
global/
  cars/
    index.toml               ob.car-index    the car types in table order (type index = position)
    types/<NAME>.toml        ob.car-type     one car type record and its AI upgrade presets
    presets.toml             ob.car-presets  preset rides, in order
    performance.toml         ob.performance  packages, tuning unlocks, rating bounds, class curves
  parts/
    db.toml                  ob.part-db      the part database and the slot groups
    db.lists.bin             layout u16      the attribute lists (one external array)
    costs.toml               ob.part-costs   the part cost table
cars/
  <NAME>/unit.toml           ob.car-unit     what the original CARS/<NAME>/ folder holds (meshes.md,
                                             textures.md, animation.md write the files it names)
  common/unit.toml           ob.car-unit     CARS/TEXTURES.BIN (textures.md)
```

| Item | Key | Notes |
|---|---|---|
| car type | `car:<NAME>` | The record's name (+0x000), e.g. `car:PEUGOT`. Names are unique (checked) and equal the name of the type's `CARS/` folder where it has one. The type hash `bin_hash(NAME)` (record +0x0D0) is derived; other kinds that store the hash (career unlocks, event opponents) refer by name and their encoders hash it. |
| car type order | position in `index.toml` | The **type index**: the engine's own tables are indexed by it (§3.9). Never changes in 1.0. |
| AI upgrade preset | (car, position) | Inside the car type file; the list is ordered (the nearest-rating search walks it). |
| preset ride | `preset:<NAME>` | +0x28 name, e.g. `preset:DDAY_PLAYER_CAR` (looked up by its hash). |
| part | `part:0x<GROUP>/0x<HASH>` | Group hash and part hash: the 12,167 parts of the reference install carry 12,160 distinct hashes (a few occur in two groups), so the hash alone is not a key; (group, hash) is unique (checked). |
| part group | its hash (`group[].hash`) | Named when the hash is a known name (60 of the 75 are car or folder names). |
| part cost category | `part-cost:<number>` | 1-based record number. |
| performance package | `perf-package:<category>/<level>` | Unique in the data (10 categories × 3 levels). |
| performance part | `perf-part:<id>` | Ids 0–0x43, unique across packages. |
| tuning unlock | `tuning-unlock:<part>/<param>` | Unique in the data. |
| car unit | `car-unit:<NAME>` | The folder name; `common` for the shared files. It stands for an original folder, so it is looked up without regard to ASCII case, and two keys differing only in case are refused (conventions §6.3, `CONV.13`). |

Order is data ([conventions.md](conventions.md) §6.5): every table here keeps the original's record order, and
the encoders write records in file order.

## 3. Source form

Field tables use the types of [conventions.md](conventions.md) §3.2, layout tables those of §4.1. Physical units
are the original's: the physics works in tonnes, metres and seconds, so forces are kN and torques kN·m
(docs/formats/cars-1.md "Handling"). Field names end in conventions §5.2's suffixes where one exists (`_tonne`,
`_knm`, `_rpm`, `_mph`, `_mps`, `_s`, `_deg`, `_bam16`); other units (t·m², kN/m, kN·s/m, kN/(m/s)²) are in the Unit
column, and lengths and positions are metres (conventions §5.1).

### 3.1 `global/cars/index.toml` (`ob.car-index/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.car-index/1.0` |
| `order` | array<string>, 1–255 | – | yes | – | Car type names; position = type index. Each names `types/<NAME>.toml` (`CRS.1`). |

```toml
schema = "ob.car-index/1.0"
order = ["PEUGOT", "FOCUS", "COROLLA"]   # a small community set; an imported base set lists all 46
```

### 3.2 `global/cars/types/<NAME>.toml` (`ob.car-type/1.0`)

One file per car type: the 0x890-byte record of chunk 0x34600, field for field, plus the car's AI upgrade
presets. **The engine consumes the record as bytes**: besides the handling parse, the upgrades
(src/performance/apply.rs:7), the performance uniques (:185), the dyno tuning (src/tuning/settings.rs:132), the
handling score (src/ai/route/mod.rs:480), the hydraulics and the shops read and rewrite record offsets directly
(src/bin/game/car_build.rs:18, :43, :71; src/bin/game/state.rs:147). So the cooked form of a car type is the
record itself, made by this layout's encoder from the merged TOML (§7), and the layout below is normative for
every byte.

**TOML structure.** Top-level fields, sub-tables `chassis`, `wheel.{fl,fr,rr,rl}`, `tyres.{front,rear}`,
`suspension.{front,rear}`, `gearbox`, `engine`, `aero`, `handling`, `brakes`, `nitrous`, `upgrades` (with
`unique`, `weight`, `gearbox.{1,2,3}`, `engine.{0,1,2,3}`, `tyres.*`, `nitrous`, `brakes`, `suspension.*`,
`ride_height`), `camera.{0..5}`, `split_camera.{0..5}`, `gauges`, and the list `ai_preset`. Fixed-size repeats are
sub-tables with fixed keys, never lists, so a pack can change one wheel or one camera field by field
([layering.md](layering.md) §3.2); those keyed by number (`camera`, `split_camera`, `upgrades.gearbox`,
`upgrades.engine`) are `map<K,V>` tables (conventions §3.2) that take exactly the keys listed.

**Defaults.** Every field's default is 0 (all bits zero), except the **derived** fields, whose default is
computed: `skin_name` = `name`; `geometry_path` = `CARS\<name>\GEOMETRY.BIN`; `geometry_lzc_path` =
`CARS\<name>\GEOMETRY.LZC`; `name_hash` = `bin_hash(name)`; `type_index` = the position in `index.toml`; each
wheel's `index` = 0, 1, 2, 3. All 46 records of the reference install equal their derivations (checked), so an
imported file holds none of them; a record that differs keeps the field explicitly (`CRS.12`). Writers omit fields
equal to their default ([layering.md](layering.md) §3.2).

**Text fields** (`name`, `skin_name`, `geometry_path`, `geometry_lzc_path`, `manufacturer`: `u8[N]` holding text):
printable ASCII, at most N − 1 bytes; the encoder writes the text, a NUL, then zero bytes to N (`CRS.2`). When a
record's bytes are not of that form (bytes after the NUL, no NUL), the field keeps its text up to the first NUL and the optional table `raw_text` holds the field's full N bytes (`raw_text = { name =
"hex:…" }`), which the encoder writes instead (base sets only; none in the reference install). So **every byte of
the record is representable** and the record encoder never fails: the engine reads these bytes raw, so a shadow
would not be enough. Hash fields (`stock_paint`, `u_0x850`, `name_hash`) are bare `hash32` strings (conventions
§6.6; `stock_paint` names a part of the part database, §3.4); the other `u32` fields are integers.

**Open fields** (`u_*`, conventions §8) are named by their offset in the smallest struct that holds them: the
record's offset for top-level fields and the one-off sub-tables (`chassis`, `engine`, `handling`, …), the struct's
offset inside the repeated structs (`wheel`, `tyre`, `suspension`, `gearbox`, `camera`). 46 open fields, 465 of
the record's 2,192 bytes, most of them zero in every record.

**Record layout** (0x890 bytes, alignment 4; the record array starts 16-aligned in the chunk, after 8 fill bytes
0x11):

| Offset | Size | Type | Field (TOML path) | Unit | Notes |
|---|---|---|---|---|---|
| 0x000 | 32 | `u8[32]` | `name` | – | Key. **verified** (looked up by name, src/car/record.rs:144) |
| 0x020 | 32 | `u8[32]` | `skin_name` | – | Base of `<skin_name>_SKIN1` (render-info remap 0x631E30). **verified**. Default: `name` |
| 0x040 | 32 | `u8[32]` | `geometry_path` | – | Non-empty = stock-car candidate (0x516500). **verified** as a test; the text is not used for loading. Default: `CARS\<name>\GEOMETRY.BIN` |
| 0x060 | 32 | `u8[32]` | `geometry_lzc_path` | – | **open** (no reader known). Default: `CARS\<name>\GEOMETRY.LZC` |
| 0x080 | 64 | `u8[64]` | `u_0x080` | – | **open**; zero in every record of the reference install |
| 0x0C0 | 16 | `u8[16]` | `manufacturer` | – | Lot logo `CARSELECT_MANUFACTURER_<it>` (0x511E60). **verified** |
| 0x0D0 | 4 | `u32` | `name_hash` | – | = `bin_hash(name)` in every record. Derived; written only when it differs |
| 0x0D4 | 4 | `f32` | `u_0x0d4` | – | **open**; one value in every record |
| 0x0D8 | 1 | `u8` | `u_0x0d8` | – | **open** |
| 0x0D9 | 1 | `u8` | `skin_count` | – | Traffic skin draw count (0x61BC30). **verified** |
| 0x0DA | 1 | `u8` | `lot_flags` | – | Bit 1: may be a stock car (0x516500). **verified**; other bits **open** |
| 0x0DB | 1 | `u8` | `u_0x0db` | – | **open** |
| 0x0DC | 1 | `u8` | `rim_size` | – | Rims fit when their attribute 0xCE7D8DB5 matches (0x539B80). **verified** |
| 0x0DD | 1 | `u8` | `rim_min` | – | Smallest rim the AI rim draw takes (0x632DF0). **verified** |
| 0x0DE | 1 | `u8` | `rim_max` | – | Largest rim of the unique-part list (0x533D30). **verified** |
| 0x0DF | 1 | `u8` | `u_0x0df` | – | **open** |
| 0x0E0 | 48 | `f32[12]` | `u_0x0e0` | – | **open**; the same twelve values in every record |
| 0x110 | 12 | `f32[3]` | `chassis.centre_of_mass` | m, car space | Tuning block +0x00 (the physics copies the record from here, 0x5AC500). **verified** |
| 0x11C | 4 | `f32` | `chassis.u_0x11c` | – | **open** |
| 0x120 | 192 | `wheel[4]` | `wheel.{fl,fr,rr,rl}` | – | Front left, front right, rear right, rear left; struct `wheel` below |
| 0x1E0 | 32 | `tyre` | `tyres.front` | – | Struct `tyre` below |
| 0x200 | 32 | `tyre` | `tyres.rear` | – | Struct `tyre` |
| 0x220 | 4 | `f32` | `chassis.mass_tonne` | t | **verified** (scaled by weight reduction, 0x5997F0) |
| 0x224 | 12 | `f32[3]` | `chassis.dimensions` | m | Length, width, height; **inferred** from values |
| 0x230 | 64 | `f32[16]` | `chassis.inertia_matrix` | t·m² | 4×4, row-major; the diagonal +0x230/+0x244/+0x258 (roll, pitch, yaw) **verified** and recomputed from mass and dimensions by 0x5AC500; the other entries **open** |
| 0x270 | 4 | `f32` | `chassis.ground_yaw_inertia` | – | Ground yaw-inertia factor (0x59B950; the code's `unknown_270`). **verified** |
| 0x274 | 12 | `f32[3]` | `chassis.u_0x274` | – | **open** |
| 0x280 | 32 | `suspension` | `suspension.front` | – | Struct `suspension` below |
| 0x2A0 | 32 | `suspension` | `suspension.rear` | – | Struct `suspension` |
| 0x2C0 | 64 | `gearbox` | `gearbox` | – | Struct `gearbox` below |
| 0x300 | 4 | `f32` | `engine.idle_rpm` | rpm | **verified** |
| 0x304 | 4 | `f32` | `engine.redline_rpm` | rpm | **verified** |
| 0x308 | 4 | `f32` | `engine.max_rpm` | rpm | **verified** |
| 0x30C | 4 | `f32` | `engine.inertia` | t·m² | Crank inertia (0x5A12A0). **verified** |
| 0x310 | 36 | `f32[9]` | `engine.torque_knm` | kN·m | Full-throttle torque over idle..max rpm (`Curve<9>`, 0x5764E0). **verified** |
| 0x334 | 36 | `f32[9]` | `engine.turbo_torque_knm` | kN·m | Added into the torque by the setup (0x5A6E40) also when stock; the code's `curve_334`. **verified** use, name **inferred** |
| 0x358 | 12 | `f32[3]` | `engine.braking_knm` | kN·m | Closed-throttle drag over idle..max rpm. **verified** |
| 0x364 | 12 | `f32[3]` | `engine.u_0x364` | – | **open** |
| 0x370 | 4 | `f32` | `aero.drag` | kN/(m/s)² | (0x59E630) **verified** |
| 0x374 | 8 | `f32[2]` | `aero.downforce` | kN/(m/s)² | Front, rear. **verified** |
| 0x37C | 4 | `f32` | `aero.preload_downforce` | – | Downforce share in the suspension preload (0x5AB9A0). **verified** |
| 0x380 | 4 | `f32` | `handling.steering_ratio` | – | (0x416BB0) **verified** |
| 0x384 | 4 | `f32` | `brakes.brake_knm` | kN·m | Foot brake per unit pedal (×4 at 0x5A8290). **verified** |
| 0x388 | 4 | `f32` | `brakes.handbrake_knm` | kN·m | (×10) **verified** |
| 0x38C | 4 | `f32` | `brakes.balance` | – | Front ×2(1−b), rear ×2b (0x5A4D90). **verified** |
| 0x390 | 8 | `f32[2]` | `handling.drift_grip` | – | Rear-grip bonus bases while sliding (0x5990D0). **verified** |
| 0x398 | 4 | `f32` | `handling.yaw_limit_on_knm` | kN·m | yaw-torque limit with Stability Control on (0x5A49B0 at 0x5A4C65) **verified** |
| 0x39C | 4 | `f32` | `handling.yaw_limit_off_knm` | kN·m | the same with Stability Control off; 0 = only while counter-steering. **verified** |
| 0x3A0 | 4 | `f32` | `handling.top_speed_mph` | mph | The rigid body's limiter (0x59C1F0). **verified** |
| 0x3A4 | 12 | `f32[3]` | `handling.u_0x3a4` | – | **open** |
| 0x3B0 | 4 | `f32` | `nitrous.capacity_s` | s | Tank (tuning slider 16). **verified** |
| 0x3B4 | 4 | `i32` | `nitrous.shots` | – | **verified** |
| 0x3B8 | 4 | `f32` | `nitrous.u_0x3b8` | – | **open** |
| 0x3BC | 4 | `f32` | `nitrous.power` | – | Shot push (tuning slider 17). **verified** |
| 0x3C0 | 4 | `f32` | `upgrades.unique.weight` | – | Performance uniques, 0x5997F0 (all **verified**): mass × (1 − this) |
| 0x3C4 | 4 | `f32` | `upgrades.unique.rear_split` | – | Replaces `gearbox.rear_split` |
| 0x3C8 | 4 | `f32` | `upgrades.unique.nitrous_power` | – | Added to `nitrous.power` |
| 0x3CC | 4 | `f32` | `upgrades.unique.brake_knm` | kN·m | Added to `brakes.brake_knm` |
| 0x3D0 | 4 | `f32` | `upgrades.unique.grip_front` | – | Added to `tyres.front.grip` |
| 0x3D4 | 4 | `f32` | `upgrades.unique.grip_rear` | – | Added to `tyres.rear.grip` |
| 0x3D8 | 4 | `f32` | `upgrades.unique.sliding_front` | – | Added to `tyres.front.sliding` |
| 0x3DC | 4 | `f32` | `upgrades.unique.sliding_rear` | – | Added to `tyres.rear.sliding` |
| 0x3E0 | 36 | `f32[9]` | `upgrades.unique.engine_torque_knm` | kN·m | Added to the torque |
| 0x404 | 36 | `f32[9]` | `upgrades.unique.ecu_torque_knm` | kN·m | Added to the torque |
| 0x428 | 36 | `f32[9]` | `upgrades.unique.turbo_torque_knm` | kN·m | Added to the torque and the turbo curve |
| 0x44C | 4 | `f32` | `upgrades.u_0x44c` | – | **open** |
| 0x450 | 4 | `f32` | `upgrades.weight.mass_factor` | – | Weight reduction (0x5A6B60). **verified** |
| 0x454 | 4 | `f32` | `upgrades.weight.com_x_shift` | m | Added to the centre of mass x. **verified** |
| 0x458 | 4 | `f32` | `upgrades.weight.ground_yaw_inertia` | – | Target of `chassis.ground_yaw_inertia`. **verified** |
| 0x45C | 4 | `f32` | `upgrades.weight.u_0x45c` | – | **open** |
| 0x460 | 192 | `gearbox[3]` | `upgrades.gearbox.{1,2,3}` | – | Transmission levels 1–3 (0x5A6BE0). **verified** |
| 0x520 | 256 | `engine_step[4]` | `upgrades.engine.{0,1,2,3}` | – | Struct `engine_step` below (0x5A6D10, 0x5A71D0). **verified** |
| 0x620 | 36 | `f32[9]` | `upgrades.turbo_torque_knm` | kN·m | Turbo target curve (0x5A6E40). **verified** |
| 0x644 | 12 | `f32[3]` | `upgrades.u_0x644` | – | **open** |
| 0x650 | 32 | `tyre` | `upgrades.tyres.front` | – | Tyre targets (0x5A7370). **verified** |
| 0x670 | 32 | `tyre` | `upgrades.tyres.rear` | – | **verified** |
| 0x690 | 4 | `f32` | `upgrades.nitrous.capacity_s` | s | Target of `nitrous.capacity_s`. **verified** |
| 0x694 | 4 | `i32` | `upgrades.nitrous.shots` | – | **inferred** (mirrors `nitrous.shots`; not read by 0x5AC500) |
| 0x698 | 4 | `f32` | `upgrades.nitrous.u_0x698` | – | **open** |
| 0x69C | 4 | `f32` | `upgrades.nitrous.power` | – | Set into `nitrous.power`. **verified** |
| 0x6A0 | 12 | `f32[3]` | `upgrades.aero_a` | – | With `aero_b`: added to downforce front/rear and preload by the body parts' gains (0x5A74B0). **verified** |
| 0x6AC | 4 | `f32` | `upgrades.u_0x6ac` | – | **open** |
| 0x6B0 | 12 | `f32[3]` | `upgrades.aero_b` | – | **verified** (see `aero_a`) |
| 0x6BC | 4 | `f32` | `upgrades.u_0x6bc` | – | **open** |
| 0x6C0 | 4 | `f32` | `upgrades.brakes.brake_knm` | kN·m | Brake targets (0x5A7150). **verified** |
| 0x6C4 | 4 | `f32` | `upgrades.brakes.handbrake_knm` | kN·m | **verified** |
| 0x6C8 | 4 | `f32` | `upgrades.brakes.balance` | – | **verified** |
| 0x6CC | 4 | `f32` | `upgrades.ground_yaw_inertia` | – | Suspension target (0x5A6FB0). **verified** |
| 0x6D0 | 32 | `suspension` | `upgrades.suspension.front` | – | Suspension targets (0x5A6FB0). **verified** |
| 0x6F0 | 32 | `suspension` | `upgrades.suspension.rear` | – | **verified** |
| 0x710 | 4 | `f32` | `upgrades.ride_height.com_drop` | m | Subtracted from the centre of mass z. **verified** |
| 0x714 | 4 | `f32` | `upgrades.ride_height.front` | m | Added to the front wheels' z (also hydraulics, tuning limits). **verified** |
| 0x718 | 4 | `f32` | `upgrades.ride_height.rear` | m | Added to the rear wheels' z. **verified** |
| 0x71C | 4 | `f32` | `upgrades.ride_height.u_0x71c` | – | **open** |
| 0x720 | 4 | `u32` | `camera_count` | – | At most 6 read (src/car/record.rs:94). **verified** |
| 0x724 | 4 | `u32` | `split_camera_count` | – | **inferred** (split screen only) |
| 0x728 | 8 | `u32[2]` | `u_0x728` | – | **open** |
| 0x730 | 96 | `camera[6]` | `camera.{0..5}` | – | Struct `camera` below (0x5ECCB0 → 0x441140). **verified** |
| 0x790 | 96 | `camera[6]` | `split_camera.{0..5}` | – | Split screen (0x8707DC == 3). **inferred** |
| 0x7F0 | 4 | `u32` | `gauges.u_0x7f0` | – | **open** |
| 0x7F4 | 8 | `f32[2]` | `gauges.needle_deg` | deg | Drag tachometer needle at 0 and max rpm with more than one local human. **inferred** |
| 0x7FC | 8 | `f32[2]` | `gauges.drag_needle_deg` | deg | The same with one local human (0x4AB210). **verified** |
| 0x804 | 4 | `u32` | `gauges.u_0x804` | – | **open** |
| 0x808 | 4 | `f32` | `gauges.u_0x808` | – | **open** |
| 0x80C | 4 | `u16[2]` | `gauges.u_0x80c` | – | **open**; type [guess] |
| 0x810 | 16 | `f32[4]` | `gauges.u_0x810` | – | **open** |
| 0x820 | 4 | `u32` | `gauges.u_0x820` | – | **open** |
| 0x824 | 4 | `f32` | `gauges.u_0x824` | – | **open** |
| 0x828 | 4 | `u32` | `gauges.u_0x828` | – | **open** |
| 0x82C | 4 | `f32` | `gauges.u_0x82c` | – | **open** |
| 0x830 | 12 | `u16[6]` | `gauges.u_0x830` | – | **open**; type [guess] |
| 0x83C | 4 | `u32` | `gauges.u_0x83c` | – | **open** |
| 0x840 | 4 | `i32` | `type_index` | – | = the record's position in the table in every record; −1 = not listed (the lists' test). Derived; written only when it differs |
| 0x844 | 4 | `u32` | `kind_code` | – | 0 racer, 2 traffic; non-zero fixes the performance class at 3 (0x5B4DD0). **verified**; 1 **open** |
| 0x848 | 4 | `u32` | `u_0x848` | – | **open** |
| 0x84C | 4 | `u32` | `stock_paint` | – | hash32: the stock paint part (slot 0x3F, 0x637050). **verified** |
| 0x850 | 4 | `u32` | `u_0x850` | – | hash32; **open** (a paint part hash [guess]) |
| 0x854 | 5 | `i8[5]` | `traffic_caps` | – | Per traffic zone, how many cars may take this model (0x412BE0). **verified** |
| 0x859 | 3 | `u8[3]` | `u_0x859` | – | **open** |
| 0x85C | 4 | `u32` | `u_0x85c` | – | **open** |
| 0x860 | 20 | `f32[5]` | `u_0x860` | – | **open** (five values like the zone caps [guess]) |
| 0x874 | 4 | `u32` | `u_0x874` | – | **open** |
| 0x878 | 4 | `u32` | `u_0x878` | – | **open** |
| 0x87C | 4 | `u32` | `u_0x87c` | – | **open** |
| 0x880 | 8 | `u8[8]` | `skin_table` | – | Skin byte table drawn by 0x61BC30. **verified** |
| 0x888 | 1 | `u8` | `u_0x888` | – | **open** |
| 0x889 | 1 | `u8` | `u_0x889` | – | **open** |
| 0x88A | 1 | `u8` | `suv` | – | Non-zero: an SUV (SUV events, 0x527D40). **verified** |
| 0x88B | 1 | `u8` | `u_0x88b` | – | **open** |
| 0x88C | 4 | `u32` | `composited_skin` | – | Non-zero: the car gets a `DUMMY_SKIN<n>` composite slot (0x61C290). **verified** |

Repeated structs (each struct's table covers its size exactly):

`wheel` (0x30 bytes; `wheel.fl`, `.fr`, `.rr`, `.rl` = record +0x120, +0x150, +0x180, +0x1B0; the z of `position`
is what the tuning's lowering moves, parameters 3 and 7):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 12 | `f32[3]` | `position` | m, car space | Wheel centre. **verified** |
| 0x0C | 4 | `f32` | `u_0x0c` | – | **open** |
| 0x10 | 4 | `f32` | `radius` | m | **verified** |
| 0x14 | 4 | `f32` | `width` | m | Tyre width an aftermarket rim is drawn at. **verified** |
| 0x18 | 4 | `u32` | `index` | – | = the wheel's number 0–3 in every record. Derived; written only when it differs |
| 0x1C | 4 | `f32` | `face` | m | y of the outer face (0x61FE20, 0x6162F0). **verified** |
| 0x20 | 4 | `f32` | `face_wide` | m | The same with a wide body (0x620AF0). **verified** |
| 0x24 | 12 | `f32[3]` | `u_0x24` | – | **open** |

`tyre` (0x20 bytes; `tyres.front` +0x1E0, `.rear` +0x200; `upgrades.tyres.*` +0x650, +0x670):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `f32` | `slip_angle_scale_deg` | deg | Lateral curve index = angle / scale · 6 (0x59D520). **verified** |
| 0x04 | 4 | `f32` | `load_scale` | – | **verified** |
| 0x08 | 4 | `f32` | `lateral_scale` | – | **verified** |
| 0x0C | 4 | `f32` | `grip` | × load | Friction circle (0x59D610). **verified** |
| 0x10 | 4 | `f32` | `sliding` | × load | (0x5A8290) **verified** |
| 0x14 | 12 | `f32[3]` | `u_0x14` | – | **open** |

`suspension` (0x20 bytes; +0x280, +0x2A0; `upgrades.suspension.*` +0x6D0, +0x6F0; used by the update 0x5A87A0):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `f32` | `progression` | – | k·(1 + progression·x). **verified** |
| 0x04 | 4 | `f32` | `spring` | kN/m | **verified** |
| 0x08 | 4 | `f32` | `bump` | kN·s/m | **verified** |
| 0x0C | 4 | `f32` | `rebound` | kN·s/m | **verified** |
| 0x10 | 4 | `f32` | `anti_roll` | kN/m | **verified** |
| 0x14 | 4 | `f32` | `max_compression` | m | **verified** |
| 0x18 | 4 | `f32` | `max_extension` | m | Negative. **verified** |
| 0x1C | 4 | `f32` | `u_0x1c` | – | **open**: not read by the suspension update 0x5A87A0, moved by the suspension upgrade (the code's `unknown_1c`) |

`gearbox` (0x40 bytes; `gearbox` +0x2C0; `upgrades.gearbox.1/2/3` +0x460, +0x4A0, +0x4E0, from which the
transmission upgrade copies the final drives, split, neutral shift, `u_0x04`, ratios and gear count by its rounded
level and moves the clutch towards level 3's):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `f32` | `neutral_shift` | – | Where between idle and redline the shift out of neutral sits (0x5AC7E0). **verified** |
| 0x04 | 4 | `f32` | `u_0x04` | – | **open** (the code's `unknown_2c4`) |
| 0x08 | 4 | `f32` | `final_drive_front` | – | **verified** |
| 0x0C | 4 | `f32` | `final_drive_rear` | – | **verified** |
| 0x10 | 4 | `f32` | `rear_split` | – | Drive torque share to the rear (0x594A40). **verified** |
| 0x14 | 4 | `f32` | `clutch_stiffness` | kN·m/(rad/s) | (0x594CC0) **verified** |
| 0x18 | 4 | `u32` | `forward_gears` | – | At most 7 read. **verified** |
| 0x1C | 4 | `f32` | `clutch_rate` | 1/s | (0x5949C0) **verified** |
| 0x20 | 32 | `f32[8]` | `ratios` | – | Reverse, neutral, 1st … 6th. **verified** |

`engine_step` (0x40 bytes; `upgrades.engine.0..3` at +0x520, +0x560, +0x5A0, +0x5E0). Fields no step reads (step
0's `delta_rpm`, steps 1–2's `torque_delta_knm` and `braking_delta_knm`, steps 1–3's `inertia_delta`) are data like
any other and are kept:

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 12 | `f32[3]` | `delta_rpm` | rpm | Idle, redline, max: added by the ECU at rounded level 1–3 (steps 1–3). **verified** |
| 0x0C | 4 | `f32` | `inertia_delta` | t·m² | Step 0: added to the crank inertia by the transmission. **verified** |
| 0x10 | 36 | `f32[9]` | `torque_delta_knm` | kN·m | Step 0: engine category; step 3: ECU. **verified** |
| 0x34 | 12 | `f32[3]` | `braking_delta_knm` | kN·m | Steps 0 and 3, as `torque_delta_knm`. **verified** |

`camera` (0x10 bytes; `camera.0..5` +0x730, `split_camera.0..5` +0x790; looked up by kind):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 2 | `i16` | `kind` | – | 0 far, 1 near, 2 bumper, 3 driver, 4 hood, 5 high. **verified** |
| 0x02 | 2 | `u16` | `tilt_bam16` | binary angle | Downward tilt. **verified** |
| 0x04 | 12 | `f32[3]` | `eye` | m, car space | Stored x, z, y; the TOML field is [x, y, z]. **verified** |

**AI upgrade presets** (`ai_preset`, a list in record order; GLOBALB 0x34A31, body = u32 count then 16-byte
records; the chunks follow the car types' order inside 0x80034A30):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `f32` | `rating` | – | The rating 0x5B6AE0's nearest search compares. **verified** |
| 0x04 | 1 | `u8` | `u_0x04` | – | **open**; non-zero in 58 of the 1,194 records [guess: uninitialised bytes of the original's tools] |
| 0x05 | 9 | `u8[9]` | `parts` | – | A 72-bit set, most significant bit of byte 0x05 first (bit *k* of the set = performance part id *k*); the TOML lists the ids whose bit is set, ascending (`CRS.4`). Ids 0–0x43 are read; bits 68–71 are zero in every record and kept as data. **verified** |
| 0x0E | 2 | `u16` | `u_0x0e` | – | **open**; non-zero in 30 records [guess: as `u_0x04`] |

**Limits.** One car type per file; text fields as above; `ai_preset` ≤ 1,000 entries (the reference install has
26–49 per racer type and 1 per traffic type).

**Example** (a base set's file, made-up values, shortened; the complete file is
`examples/cars/car-type-base.toml`):

```toml
schema = "ob.car-type/1.0"
name = "FOCUS"
manufacturer = "FORD"
lot_flags = 0x02
rim_size = 0x18
stock_paint = "0x1A2B3C4D"
traffic_caps = [5, 2, 2, 2, 2]
composited_skin = 1

[chassis]
centre_of_mass = [0.25, 0.0, 0.3125]
mass_tonne = 1.125

[wheel.fl]
position = [1.25, 0.75, 0.3125]
radius = 0.3125

[suspension.front]
spring = 30.0
u_0x1c = 0.125                  # open: not read by the suspension update

[engine]
idle_rpm = 900.0
max_rpm = 7250.0
torque_knm = [0.125, 0.15, 0.1625, 0.175, 0.1875, 0.1875, 0.175, 0.15, 0.125]   # idle..max rpm

[upgrades.engine.1]
delta_rpm = [0.0, 250.0, 250.0]  # ECU level 1: idle, redline, max

[camera.1]
kind = 1                         # near chase
tilt_bam16 = 0x0300
eye = [-4.5, 0.0, 1.75]          # car space x, y, z (stored x, z, y)

[[ai_preset]]
rating = 412.5
parts = [0, 3, 6, 9, 12, 15]
```

### 3.3 `global/cars/presets.toml` (`ob.car-presets/1.0`)

Chunk 0x30220: 0x338-byte records, read from the chunk body unaligned, 32 in the reference install. Text fields
follow §3.2's rule, with `raw_text = { car, name }` for bytes the text cannot hold, so every byte is representable.

| Offset | Size | Type | Field | Unit | Type in TOML | Req. | Default | Meaning |
|---|---|---|---|---|---|---|---|---|
| 0x000 | 8 | `u32[2]` | `u_0x00` | – | array<u32>[2] | no | [0, 0] | **open**: two values that differ per record [guess: pointers left in the file by the original's tools] |
| 0x008 | 32 | `u8[32]` | `car` | – | ref car | yes (base) | – | Text: the car type, as `car:<NAME>` (`CRS.5`). |
| 0x028 | 32 | `u8[32]` | `name` | – | string ≤ 31 | yes | – | Text: the key. |
| 0x048 | 4 | `i32` | `perf_level` | – | i32 | no | 0 | Performance level fitted (0x5A64B0: every category's packages of levels 1..it), or −1: `perf_parts` decide. |
| 0x04C | 0x2A8 | `u32[170]` | `parts` | – | map<`"0x00"`–`"0xA9"`, string> | no | every slot `stock` | Per slot: `"stock"` (stored 1), `"none"` (0) or a part hash (`hash32`). Absent slots are `stock`. |
| 0x2F4 | 0x44 | `u8[68]` | `perf_parts` | – | array<u8>[68] | no | 68 × 0 | Per performance part id: 255 unset, else fitted when non-zero. |

Record size 0x338, alignment 1 (read unaligned).

```toml
schema = "ob.car-presets/1.0"

[[preset]]                       # a mod pack: an original preset changed ...
name = "DDAY_PLAYER_CAR"
perf_level = 3

[preset.parts]
"0x3F" = "0x5E6F7A8B"            # slot 0x3F (paint): another paint part

[[preset]]                       # ... and a new one
op = "add"
name = "DOCKS_RIVAL_350Z"
car = "car:350Z"
perf_level = 2

[preset.parts]
"0x0C" = "0x0BADF00D"
"0x49" = "none"
```

### 3.4 `global/parts/db.toml` (`ob.part-db/1.0`) and `db.lists.bin`

The part database is a set of pools that refer to each other by index (crates/ob-formats/src/formats/parts.rs:1–13): parts point at
a string (their name), an attribute list and a model; lists point at attributes; models point at strings. The
engine consumes the pools as they are (`PartDb`), and two of its walks depend on positions inside them: the slot
search returns parts in database order (0x61B990), and a part's owned-bit in the native save is its position among
the parts that walk returns (src/frontend/career/native_map/context.rs:161). So the format keeps every pool in its
original order and with its original indices (**pool form**): each table below is one child chunk, record by
record. Tools show the resolved view (a part with its name, attributes and solids); the file stays in pool form.

The container's children follow each other in this order, each read from right after its 8-byte header (not
aligned): 0x34603, 0x34606, 0x3460C, 0x34605, 0x3460A, 0x3460B, 0x34604.

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.part-db/1.0` |
| `header` | array<u32>[15] | – | base sets | – | Chunk 0x34603. Entries 8, 10, 12, 14 = attribute, group, model and part counts (`CRS.6`); the others **open** (zero except entry 2). |
| `string` | array<table> | – | base sets, unless `string_pool` | – | Chunk 0x34606, the string pool, as elements `{ at = u32, text = string ≤ 1,000 }`: `at` = dword index of the string; `text` = its bytes as Windows-1252 (crates/ob-formats/src/formats/parts.rs:585 `cp1252`, a one-to-one mapping, so the bytes come back exactly); NUL-terminated and zero-padded to a dword. `at` increases; a gap is zero dwords. |
| `string_pool` | array descriptor | – | instead of `string` | absent | Chunk 0x34606 as it is (layout `raw_u8`), when it is not strings and zero fill (a non-zero byte after a terminator): the engine keeps the pool as bytes, so it must come back exactly. Not used in the reference install. |
| `lists` | array descriptor | – | base sets | – | Chunk 0x3460C (layout `u16`): the attribute lists as one array: at a list's index, a count, then that many attribute indices. Kept as one array because lists are shared (60 list offsets serve several parts) and one trailing `u16` belongs to no list. |
| `attribute` | array<table> | – | base sets | – | Chunk 0x34605, records `part_attribute`, as elements `{ key = hash32, key_name = string, value = u32 }`; position = attribute index. `key_name` is the key's name when known (tools only; 51 distinct keys in the reference install). |
| `model` | array<table> | – | base sets | – | Chunk 0x3460A, records `part_model`, as elements `{ named = u8, u_0x01 = u8, middle = u16, names = array<array<u32>[4]>[2] }`; position = model index. |
| `group` | array<table> | – | base sets | – | Chunk 0x3460B, one `u32` hash per group, as elements `{ hash = hash32, name = name }` (`name` for tools, when the hash is a known name); position = group index; hashes unique (`CRS.6`). |
| `part` | array<table> | – | base sets | – | Chunk 0x34604, records `part_record` in order, as elements with the fields of the layout below (`hash` and `group` as `hash32`; `kind` and `level` as integers). Key (`group`, `hash`). A mod pack's entry holds only the key and `attributes` (§4.3). |
| `part_tail` | bytes ≤ 13 | – | no | `"hex:"` | Bytes of 0x34604 after the last whole record (2 zero bytes in the reference install). |
| `slot_groups` | table | – | base sets | – | Chunk 0x34607: `defaults` (array of 170 `[hash32, hash32]`, records `slot_group_default`: per car slot, the two groups its parts come from; `0xFFFFFFFF` = the car's own name hash, 0x6101F0), `override` (array<table>, default `[]`: records `slot_group_override` after the 0x550 bytes of defaults, in order, as `{ car = hash32, slot = u32, groups = [hash32, hash32] }`), `tail` (bytes ≤ 15, default `"hex:"`: bytes after the last whole override; none in the reference install). |
| `extras` | table | – | no | – | conventions §9. |

`part_record` (chunk 0x34604, 0x0E bytes, alignment 1):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `hash` | – | The part's hash. **verified** |
| 0x04 | 1 | `u8` | `slot` | – | The part type; a car slot takes the parts whose type is the slot's (`SLOT_TYPES`, an engine table). **verified** |
| 0x05 | 1 | `u8` | `kind_level` | – | Bits 0–4 `kind` (kind within the type), bits 5–7 `level` (upgrade level, 0 stock); two TOML fields. **verified** |
| 0x06 | 1 | `u8` | `seed` | – | What model names continue from: 0 nothing, 1 the group hash, else the brand attribute 0xEBB03E66. **verified** |
| 0x07 | 1 | `u8` | `group` | – | Group index; the TOML holds the group's **hash** and the encoder writes its index. **verified** |
| 0x08 | 2 | `u16` | `name` | – | String index (dwords) of its name. **verified** |
| 0x0A | 2 | `u16` | `attribute_list` | – | Index into `lists`; 0xFFFF none. **verified** |
| 0x0C | 2 | `u16` | `model` | – | Model index; 0xFFFF none. **verified** |

`part_model` (chunk 0x3460A, 0x24 bytes, alignment 4; all **verified**):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 1 | `u8` | `named` | – | Non-zero: the names are string indices, else solid hashes (0 or 1 in every record). |
| 0x01 | 1 | `u8` | `u_0x01` | – | **open**; zero in all |
| 0x02 | 2 | `u16` | `middle` | – | String index of the middle part; 0xFFFF none |
| 0x04 | 32 | `u32[8]` | `names` | – | Two groups of four (LOD A–D), group by group; 0xFFFFFFFF none |

A named model's solid is `bin_hash_from(seed, "<middle><name>_<LOD letter>")` (crates/ob-formats/src/formats/parts.rs:325–360).

`part_attribute` (chunk 0x34605, 8 bytes, alignment 4):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `key` | – | The attribute key's hash |
| 0x04 | 4 | `u32` | `value` | – | Its value (an integer, a hash or a string index, by key) |

`slot_group_default` (chunk 0x34607, 170 records from its start, 8 bytes) and `slot_group_override` (after them,
0x10 bytes), alignment 4:

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `groups` | – | `slot_group_default`: the slot's two group hashes |

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `car` | – | The car type's name hash |
| 0x04 | 4 | `u32` | `slot` | – | The car slot |
| 0x08 | 8 | `u32[2]` | `groups` | – | The two group hashes for that car and slot |

`db.lists.bin` layout `u16`:

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 2 | `u16` | `value` | – | A list count or an attribute index. Size 2, alignment 2. |

**Mod-pack form.** A pack's `db.toml` holds only `[[part]]` entries with the key (`group`, `hash`) and an
`attributes` table (§4.3); every pool field is reserved to base sets (`CRS.7`).

```toml
schema = "ob.part-db/1.0"        # a mod pack: new colour values for one vinyl colour part

[[part]]
group = "0x2C8E7A11"
hash = "0x4F00A1B2"

[part.attributes]
RED = 200
GREEN = 40
BLUE = 40
```

**Limits.** Pools ≤ 65,535 records (the indices are u16; groups ≤ 256, u8); a string ≤ 1,000 characters; the TOML
file ≤ 16 MiB (the reference install's is about 2 MB [guess]).

### 3.5 `global/parts/costs.toml` (`ob.part-costs/1.0`)

Career chunk 0x34A1C: 0x28-byte records, one per cost category (86 in the reference install), in order.

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.part-costs/1.0` |
| `category` | array<table> | – | base sets | – | The records, layout `cost_record`, in order. |
| `category[].number` | u32 | – | yes | – | `+0x00`: the category, = position + 1 (checked). The key (`part-cost:<number>`). |
| `category[].level` | map<`"1"`–`"3"`, table> | – | base sets | – | Per level *l* (record `+0x04 + 12·(l − 1)`), the fields of `cost_level`: `points` (i16), `price` (i16), `unlock` (u8), `unlock_level` (i8), `unlock_value` (u32); each defaults to 0. |
| `extras` | table | – | no | – | conventions §9. |

`cost_record` (0x28 bytes, alignment 4):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `number` | – | The category |
| 0x04 | 36 | `cost_level[3]` | `level` | – | Levels 1, 2, 3 |

`cost_level` (12 bytes, alignment 4):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 2 | `i16` | `points` | – | Visual points (0x501860). **verified** |
| 0x02 | 2 | `i16` | `price` | $ | Price (0x514280); −1 none. **verified** |
| 0x04 | 1 | `u8` | `unlock` | – | 1 an event won, 2 normal, 3 URL, 4 sponsor wins in the stage (0x515360); 0 and 6 occur, **open**. |
| 0x05 | 1 | `i8` | `unlock_level` | – | The level the unlock test is asked for (0x515470). **verified** |
| 0x06 | 2 | `u8[2]` | `pad` | – | Zero in every record; not in the TOML (a non-zero byte becomes a patch, §5.2). |
| 0x08 | 4 | `u32` | `unlock_value` | – | The unlock's argument: an event hash, or a count in the low 16 bits. **verified** |

Example: `examples/cars/costs.toml`.

### 3.6 `global/cars/performance.toml` (`ob.performance/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.performance/1.0` |
| `package` | array<table> | – | base sets | – | Career chunk 0x34A15, records `perf_package` (30), in order; key (`category`, `level`). Fields `category` (u32: 0 weight, 1 transmission, 2 nitrous, 3 engine, 4 turbo, 5 suspension, 6 brakes, 7 ECU, 8 tyres, 9 aerodynamics), `level` (u32, 1–3), `part`. |
| `package[].part` | array<table> ≤ 4 | – | no | `[]` | Records `perf_part`; a keyed list, key `id`. Fields `id` (u32, 0–0x43), `name` (hash32: a bare hash of a `string` in the active language, conventions §6.6), `price` (u32, $), `brands` (array<hash32> ≤ 8), `gains` (array<f32>[4], −1 none), `replaces` (array<u32> ≤ 5, performance part ids), `requires` (array<u32> ≤ 2, likewise). All **verified** (docs/formats/cars-1.md "Data and parts"). |
| `tuning_unlock` | array<table> | – | base sets | – | Career chunk 0x34A1A, records `tuning_unlock` (62), in order; key (`part`, `param`). Fields as in the layout; `range` and `display` are array<f32>[2]. **verified** (src/tuning/mod.rs:1–12). |
| `bounds` | array<f32>[30] | – | base sets | – | GLOBALB 0x34A32. One table with two readers: the shops' stat bars (all 30, src/frontend/career/data.rs:443) and the AI (+0x28/+0x2C rating bounds, +0x30…+0x6C handling-score normalisers, +0x70/+0x74 skill spread; src/opponents.rs:120). **verified** |
| `class_curve` | map<`"0"`–`"3"`, table> | – | base sets | – | GLOBALB 0x34A33, one `class_curve` record per performance class 0 (fastest) to 3: `top_speed_mps` (f32), `u_0x04` (array<f32>[2]), `accel` (array<f32>[64]). **verified** (0x5A78F0). |
| `extras` | table | – | no | – | conventions §9. |

`perf_package` (0x17C bytes, alignment 4):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `category` | – | |
| 0x04 | 4 | `u32` | `level` | – | |
| 0x08 | 4 | `u32` | `part_count` | – | 0–4; the length of `part` |
| 0x0C | 368 | `perf_part[4]` | `part` | – | Slots past the count are zero bytes (checked; the encoder writes them) |

`perf_part` (0x5C bytes, alignment 4):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `id` | – | Read as `u8`; the high bytes are zero (checked) |
| 0x04 | 4 | `u32` | `name` | – | Language string hash |
| 0x08 | 4 | `u32` | `price` | $ | |
| 0x0C | 4 | `u32` | `brand_count` | – | 0–8; the length of `brands` |
| 0x10 | 32 | `u32[8]` | `brands` | – | Unused slots 0xFFFFFFFF (checked) |
| 0x30 | 16 | `f32[4]` | `gains` | – | −1 none |
| 0x40 | 20 | `u32[5]` | `replaces` | – | Used ids first; unused slots 0xFFFFFFFF (checked) |
| 0x54 | 8 | `u32[2]` | `requires` | – | Likewise |

`tuning_unlock` (0x18 bytes, alignment 4):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `part` | – | The performance part id |
| 0x04 | 1 | `u8` | `level` | – | 1–3 |
| 0x05 | 1 | `u8` | `param` | – | The tuning parameter it opens, 0–24 |
| 0x06 | 1 | `u8` | `kind` | – | The slider kind |
| 0x07 | 1 | `u8` | `u_0x07` | – | **open**; zero in all |
| 0x08 | 8 | `f32[2]` | `range` | – | The parameter's range at that level |
| 0x10 | 8 | `f32[2]` | `display` | – | The range the slider shows; −99.99 none |

`bounds` is one `f32[30]` (0x78 bytes). `class_curve` (0x10C bytes, alignment 4):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `f32` | `top_speed_mps` | m/s | |
| 0x04 | 8 | `f32[2]` | `u_0x04` | – | **open** |
| 0x0C | 256 | `f32[64]` | `accel` | m/s² | Acceleration over 0..top speed |

```toml
schema = "ob.performance/1.0"    # a mod pack

[[package]]
category = 3                     # engine
level = 2

[[package.part]]
id = 0x10
price = 2500
gains = [0.5, -1.0, -1.0, -1.0]

[[tuning_unlock]]
part = 0x10
param = 23                       # ECU torque curve
range = [-0.25, 0.25]

[class_curve.0]
top_speed_mps = 80.0
```

### 3.7 `cars/<NAME>/unit.toml` (`ob.car-unit/1.0`)

One per original `CARS/` folder, plus `cars/common/`. It records what the folder held, so the engine sees the same
files: in particular an **empty** file (the 15 traffic folders' `TEXTURES.BIN` and `VINYLS.BIN` are zero-length,
and the car load opens `TEXTURES.BIN` and fails without it, src/car/load.rs:192) is not the same as a missing one.

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.car-unit/1.0` |
| `name` | string | – | yes | – | The folder name, original spelling (`CRS.10`). |
| `role` | enum | – | yes | – | `car` (a car type's folder), `part-group` (EXHAUST, BRAKES, MIRRORS_*, SPOILER*, ROOF, AUDIO, PLATES), `wheels` (WHEELS: rims by brand), `common` (CARS/TEXTURES.BIN), `unreferenced` (a folder nothing loads: IMPREZA, LANCER, NEON, S2000, MIRRORS and SPINNER hold only a small `VINYLS.BIN` in the reference install). |
| `car` | ref car | – | iff `role = "car"` | – | The car type. A car type without a unit is valid (the European install has no CIVIC or RSX folder; nothing offers them there). |
| `loaded_with` | enum `cars`, `front-end`, `none` | – | no | – | For part groups and wheels: who loads them in the original (the 0x7FA978 list; PLATES by the front end). Informational: the engine's list is its own (§3.9). |
| `file` | array<table> ≤ 64 | – | yes | – | The folder's files, in name order (fields below). |
| `file[].original` | string | – | yes | – | The file's name in the folder, original spelling. |
| `file[].state` | enum `data`, `empty` | – | yes | – | `empty`: a zero-length file (the engine opens it and finds nothing). |
| `file[].kind` | enum `meshes`, `textures`, `vinyls`, `part-animations`, `other` | – | yes | – | The page that owns its content. |
| `file[].paths` | array<string> | – | for `data` files whose kind has graduated | – | The logical paths, relative to the unit, of the files the owning page writes for it: its top-level glTF or TOML files, which name their own payloads. One original file may give several (`PARTS_ANIMATIONS.bin` gives a bank list and a pivot file). Absent for `empty` files, and for a file whose kind is still at stage 0 in this unit ([import.md](import.md) §4.1: the engine reads the original file from the residual). |

The owning pages name the files: `GEOMETRY.BIN` → `meshes/GEOMETRY.BIN.0.gltf` (and `.1`, … for further solid
lists; [meshes.md](meshes.md) §2.1); `TEXTURES.BIN` → `textures/TEXTURES.toml` and `VINYLS.BIN` →
`textures/VINYLS.toml` ([textures.md](textures.md) §2); `PARTS_ANIMATIONS.bin` →
`anim/PARTS_ANIMATIONS.bin.banks.toml` and `anim/PARTS_ANIMATIONS.bin.pivots.toml` ([animation.md](animation.md)).

```toml
schema = "ob.car-unit/1.0"       # cars/TAXI/unit.toml
name = "TAXI"
role = "car"
car = "car:TAXI"

[[file]]
original = "GEOMETRY.BIN"
state = "data"
kind = "meshes"
paths = ["meshes/GEOMETRY.BIN.0.gltf"]

[[file]]
original = "TEXTURES.BIN"
state = "empty"
kind = "textures"
```

### 3.8 Vinyls, decals, paint: which records name which textures

No file kind of its own: vinyls, decals and paints are **parts** (§3.4) whose attributes name textures; the pixels
are [textures.md](textures.md)'s. A reference from a part to a texture is by the texture's hash, built as below
(src/car/skin.rs:84–175, src/car/parts.rs:224–258):

| What | Part slots | Attributes read | Texture (key `texture:0x…`) | Where the texture is |
|---|---|---|---|---|
| Vinyl layer 1–4 | 0x40–0x43 | `TEXTURE` (a string index), `REMAP` | `bin_hash("<CAR>_<TEXTURE>")` and `bin_hash("<CAR>_<TEXTURE>_MASK")` | the car unit's vinyl set (from `VINYLS.BIN`) |
| Vinyl colours | 0x4D + 4·layer + k (k = 0–3) | `RED`, `GREEN`, `BLUE`, `GLOSS` | – (values) | – |
| Decal placement | 0x34–0x3E | `SIZE`, `SHAPE` (SQUARE, RECT, WIDE) | – | – |
| Decal | 0x61 + 8·block + j | `NAME` (a hash) | `bin_hash_from(NAME, "_SQUARE" / "_RECT" / "_WIDE" / "")`; empty entries `DEFAULTALPHA` | the shared car textures [inferred] / global |
| Paint | 0x3F (body), 0x45–0x4C (part paints) | `0x6BA02C05` (light material), brand | – (a light material, not a texture) | – |
| Skin | – | record `skin_name`, `composited_skin` | `<skin_name>_SKIN1`, or the composited `DUMMY_SKIN<n>` | the car unit's textures / `cars/common` |
| Lights | 0x0E, 0x0F | `0x10C98090` (texture base) | `<base>_OFF`, `<base>_GLASS_OFF`, `<CAR>_CENTRE_BRAKELIGHT_OFF` | the car unit's textures |

A pack that wants a new vinyl adds its pixels under the car unit's textures (textures.md) and points an existing
vinyl part at them through `TEXTURE` (§4.3); adding vinyl *parts* waits with add-on parts (§9).

### 3.9 Car lists and per-car numbers: data or engine constant

| What | Data (this page) | Engine constant (built into the program) |
|---|---|---|
| Region | the base set's `[base] region` ([manifest.md](manifest.md)) | the detection rule the importer runs (crates/ob-core/src/region.rs:55) |
| Car lot | record `lot_flags` bit 1, `geometry_path`, `kind_code`; unlocks ([career.md](career.md)) | lot exclusions by region (CIVIC, RSX / CORSA, PEUGOT106; src/frontend/career/lists.rs:7) |
| Quick Race | as the lot | always-offered types by region (lists.rs:12); cheat cars |
| Random opponents | AI upgrade presets, `bounds` | pools by region: region 0 `opponent_cars` (an engine table), region 6 `OPPONENT_CARS_ALT`, SUVs `OPPONENT_SUVS` (src/frontend/career/events.rs:147, :151) |
| Traffic | `kind_code` = 2, `traffic_caps`, `skin_count`, `skin_table` | – (zones come with the tracks, [roads-ai.md](roads-ai.md)) |
| AI looks | parts, presets | part chances, vinyl colour sets, paint matches (src/ai_looks.rs:28, :52, :63) |
| Part exclusions | attributes `EXCLUDE_UG1`, `EXCLUDE_UG2`, `EXCLUDE_SUV` | which groups apply them (crates/ob-formats/src/formats/parts.rs:34–39) |
| Shared part files loaded with the cars | the units (`role = part-group`, `wheels`) | the list 0x7FA978 (src/car/assets.rs:240); the rims by brand from the parts |
| Fresh profile | `lot_flags`, `geometry_path`, `kind_code` | Quick Race default cars, ranked rides (engine tables) |
| Per type index | – | 0–60 / 0–100 factors and the car sound index (31 racer types), the hatch list (src/car/parts.rs:288), the closed-scoop list (src/car/lod.rs:38) |

Everything in the right-hand column is keyed by **type index**, which is why the order of `index.toml` cannot
change and why add-on cars need a car registry first (§9). Front-end car lists are not files of their own: the
engine builds them from the records and these constants.

### 3.10 How a car is assembled from the files

What a car load reads, in the engine's order (src/car/load.rs:11–230), as items of the format:

1. The car type `car:<NAME>` (its record bytes from the cooked `global` unit), with the fitted performance parts
   applied by `performance.toml` (upgrade levels, uniques) and, on the dyno, the tuning unlocks and sliders.
2. The part database: per slot 0x00–0xA9 the fitted part, or the stock part (the first of level 0 of the slot's
   type in the slot's groups, `slot_groups`); each part's model gives solid names.
3. Solids: the car unit's meshes, then the shared part groups' and the rims' (by the brand of the fitted rim)
   meshes, for the solids the parts name ([meshes.md](meshes.md)).
4. Textures, first one found wins: the car unit's textures, `cars/common`, `cars/WHEELS`, then the global
   textures (window glass) ([textures.md](textures.md)).
5. The skin: paint, vinyl layers and colours (§3.8), composited as `composited_skin` says.
6. Part animations of the car unit ([animation.md](animation.md)); light materials (global).
7. Cameras and gauges from the record.

A traffic car takes the same path; its unit's empty texture file contributes no textures, and its
`<NAME>_SKIN1` comes from `cars/common`.

### 3.11 `GLOBAL/CarHeadersMemoryFile.bin`

A resident copy of the start of each shared car part file, which the original keeps in memory so it can stream
single solids out of the large files (**inferred** purpose; Overbright does not read the file). Measured on the
reference install (338,816 bytes):

| Offset | Size | Content |
|---|---|---|
| 0x00 | 8 | zero |
| 0x08 | 4 | magic 0x53219999 (the same as the other `*MemoryFile.bin` files) |
| 0x0C | 4 | entry count (35) |
| 0x10 | 0x14 × count | entries: `u32` hash of the file's path (`bin_hash("CARS\\EXHAUST\\GEOMETRY.BIN")`, `CARS\\WHEELS\\GEOMETRY_<BRAND>.BIN`), `u32` offset of its copy in this file, `u32` the file's full size, `u32` the copy's size, `u16`, `u16` (**open**) |
| … | to 0x2D0 | zero |
| 0x2D0 | rest | the copies, back to back: each entry's copy equals the first (copy's size) bytes of the named file (checked for all 35: the eleven shared part groups' and the 24 rim brands' `GEOMETRY*.BIN`) |

So everything after the 720-byte index is a copy of other original files. The importer keeps the index in the
residual and records each copy as a `copy` node naming the file it copies (§5.4).

## 4. Layering

### 4.1 Operations per kind

| File kind | Item | `merge` | `add` | `replace` | `disabled` | Clash key |
|---|---|---|---|---|---|---|
| `ob.car-index` | the order | refused | refused | refused | refused | – |
| `ob.car-type` | `car:<NAME>` | the fields of §4.2 | refused (§9) | refused | refused | (car, field path) |
| `ob.car-type` `ai_preset` | the car's list | replaced whole | – | – | – | (car, `ai_preset`) |
| `ob.car-presets` | `preset:<NAME>` | yes (`car` excepted) | yes, new name (CONV.15) | yes | refused | (preset, field or slot) |
| `ob.part-db` | `part:0x<G>/0x<H>` | `attributes` only (§4.3) | refused | refused | refused | (part, attribute key) |
| `ob.part-costs` | `part-cost:<n>` | `level.<l>.*` | refused | refused | refused | (category, level, field) |
| `ob.performance` `package[].part` | `perf-part:<id>` | `name`, `price`, `brands`, `gains` | refused | refused | refused | (part id, field) |
| `ob.performance` `tuning_unlock` | `tuning-unlock:<part>/<param>` | `level`, `kind`, `range`, `display` | refused | refused | refused | (key, field) |
| `ob.performance` `bounds`, `class_curve.<k>` | the table / the class | yes (`bounds` whole, curves per field) | – | – | – | (field) |
| `ob.car-unit` | `car-unit:<NAME>` | refused | refused | refused | refused | – |

"Refused" is `MRG.3` with the field or operation named. **`disabled`**: [layering.md](layering.md) §3.1 lets each
kind say what it means; on this page it is refused for every item. The engine's own tables are indexed by car type
position (§3.9), the slot search and the native save's owned bits count part positions (§3.4), and the shops and
packages look parts and costs up by position and id, so an item treated as absent would move or break others (for
car types, §4.2).

Lists in these kinds are replaced whole except `package[].part` (keyed by `id`) and `[[part]]` / `[[preset]]` / `[[package]]` / `[[tuning_unlock]]` /
`[[category]]`, which are item lists keyed as §2 says. No kind on this page allows wildcard units.

### 4.2 Car value overrides (`ob.car-type` merges)

A mod pack's `global/cars/types/<NAME>.toml` holds the key and the fields it changes, nothing else
([layering.md](layering.md) §3.2): this is how Studio's tuning panel saves "yours" against "original", and
"reset" removes the field from the pack. The merged record is encoded and drives the physics, the dyno, the
ratings and the AI exactly as an original record would, at every frame rate.

**May be merged:** every named field of `chassis`, `wheel.*`, `tyres.*`, `suspension.*`, `gearbox`, `engine`,
`aero`, `handling`, `brakes`, `nitrous`, `upgrades.*`, `camera.*`, `split_camera.*`, `camera_count`,
`split_camera_count` (≤ 6), `gauges.needle_deg`, `gauges.drag_needle_deg`, `traffic_caps`, `manufacturer`, and
`ai_preset` (whole).

**Refused in 1.0** (`MRG.3`), with the reason:

| Fields | Why |
|---|---|
| `skin_name`, `geometry_path`, `geometry_lzc_path`, `name_hash`, `type_index`, every wheel's `index` | Identity and derived values; changing them breaks name and index lookups. |
| `kind_code`, `lot_flags`, `suv` | They decide the lot, Quick Race, SUV events and the stock list the native profile writer stores: a pack must not change what goes into a native save. |
| `stock_paint`, `rim_size`, `rim_min`, `rim_max` | They decide which part hashes a native save receives for the car. |
| `skin_count`, `skin_table`, `composited_skin` | No visible effect for traffic (every traffic car draws `<CAR>_SKIN1`), but a changed count changes the random draws at traffic loads; `composited_skin` changes the skin path. |
| every `u_*` field | Meaning unknown: a later name could give an override a meaning its author did not intend. |
| `disabled` | Hiding a type would shift the stock list of fresh native profiles and the lists the save layout follows. |

A merged value is checked on the merged record: no NaN or infinity (conventions §3.4 refuses them unless written
as `bits:`), `chassis.mass_tonne`, wheel radii and the rpm span (`max_rpm − idle_rpm`) greater than 0, `forward_gears` 1–6
(`CRS.3`). These checks apply to packs and community base sets, never to an imported set of a known edition.

**Saves.** Car values are never written into a native profile (the profile stores parts, not records), so an
override needs no fallback there; the pack is recorded in the save sidecar as `affects_play`
([manifest.md](manifest.md)).

### 4.3 Part attribute overrides (`ob.part-db` merges)

A pack sets attribute values of an existing part: `[part.attributes]`, keys as attribute key hashes (`"0x…"`) or
known names (`RED`, `TEXTURE`, …), values `u32`. Shared pool entries are never changed: when cooking, a part with
overrides gets a **private copy** of its attribute list, appended after the original lists, in which an existing
key's first occurrence takes the new value and new keys are appended; changed values get new attribute records
appended after the original ones. So every other part sharing the list or the attribute is untouched, and every
original index stays where it was (the owned-bit walk and the slot search see the same parts in the same order).

`TEXTURE` values are string indices: a pack that needs a new texture name adds it with
`[ext.OB_part_strings]` [proposal, §9]; until then it can only point at names already in the pool.

### 4.4 What waits for later (MRG.3 today)

Adding, removing, hiding or reordering car types; adding parts, models, groups or attribute lists; changing a
part's slot, level, group, name or model; adding packages, performance parts or cost categories; changing which
performance parts replace or require which; car units in packs. The extension points are in §9.

## 5. Import

### 5.1 Mapping

| Original node | Items | Encoder | Proof |
|---|---|---|---|
| `GLOBALB.BUN` 0x34600: 8 fill bytes 0x11, then 46 records | the 46 car types, `index.toml` | §3.2 layout (fixed 0x890-byte records) | `bytes` |
| 0x80034A30 → 0x34A31 ×46 | each car type's `ai_preset` | count + 16-byte records | `bytes` |
| 0x80034A30 → 0x34A32, 0x34A33 | `performance.toml` `bounds`, `class_curve` | fixed records | `bytes` |
| 0x30220 | `presets.toml` | 0x338-byte records | `bytes` |
| 0x80034602 → 0x34603 … 0x34604 | `parts/db.toml`, `db.lists.bin` | per child, record by record | `bytes` |
| 0x34607 | `db.toml` `slot_groups` | 8- and 16-byte records | `bytes` |
| first 0x80034A10 → 0x34A15, 0x34A1A, 0x34A1C | `performance.toml` `package`, `tuning_unlock`; `costs.toml` | fixed records | `bytes` |
| second 0x80034A10 → the same three chunks | the same items when byte-identical (they are, in the reference install), else `kept` | as above | `bytes` |
| 0x34601 (0x40-byte records {type, skin, colour name, …}; looked up by the car draws, results unused, docs/formats/ai.md "Skin per model"), 0x34608, 0x34609 | none | – | `kept` (no decoder) |
| `GlobalB.lzc` (after JDLZ): the same nodes | the same items | as above | `bytes`, when its content equals `GLOBALB.BUN`'s chunk for chunk; otherwise `CRS.11` |
| `CarHeadersMemoryFile.bin` | none | – | index `kept` (720 bytes); the 35 copies as `copy` nodes (§5.4) |
| `CARS/<NAME>/` | `cars/<NAME>/unit.toml` (+ the other pages' files) | – | – |
| `CARS/TEXTURES.BIN` | `cars/common/unit.toml` (+ textures.md) | – | – |

The career container's other children, and the chunk headers of every container, belong to the residual map as
[residual.md](residual.md) §3.1 says; the career page imports the rest of 0x80034A10.

### 5.2 Field rules

- Names come from the record text; the importer checks that names are unique, that `name_hash` = `bin_hash(name)`
  and `type_index` = position, and writes a derived field only when it differs (`CRS.12`).
- Text fields: when the bytes after the NUL are not all zero (or there is no NUL), the importer writes `raw_text`
  for the field and warns (`CRS.12`); car types and presets stay `bytes`-proven. None in the reference install.
- Floats are written as conventions §3.4 says (shortest round trip; `bits:` for NaNs; −0.0 keeps its sign). The
  reference records hold no NaN, infinity or −0.0.
- Unused slots (performance package parts, brands, `replaces`, `requires`) must hold the fill the encoder writes
  (zero parts, 0xFFFFFFFF ids); otherwise the bytes that differ are kept as patches (`patched`,
  [residual.md](residual.md) §3.2), or the node keeps a shadow when they exceed RES.6's limit. Cost-record pads
  likewise.
- Part groups are named from a dictionary of known names (the car types, the `CARS/` folder names: 60 of 75);
  attribute keys likewise (`key_name`).
- The 32 bytes right after the last car record are what the original reads as record 0x2E's name when it walks a
  shared group's owned bits (docs/formats/career-1.md: "0x6101F0(slot, 0x2E) hashes the name of car-type record
  0x2E — past the 46 records"); Overbright models that as "no car". The importer checks that those bytes (the next
  chunk's header in the reference install) name no part group (`CRS.13`).
- Car units: one per `CARS/` folder; `role` from the car types (a folder named like a type), the engine's shared
  part list, `WHEELS`, else `unreferenced`; each file's `state` from its size.

### 5.3 GLOBALB.BUN and GlobalB.lzc

Overbright today reads the car records from both files (physics, parts and presets from `GLOBALB.BUN`; the lot's
makes and the SUV flags from `GlobalB.lzc`, src/frontend/career/data.rs:361, :417). They are identical in the
reference install, and the importer proves it chunk for chunk. If an install differs (a modified game), the
importer takes `GLOBALB.BUN`'s car, part and preset tables and `GlobalB.lzc`'s career container, records the
difference, and marks the edition `custom` (`CRS.11`); after the move both readers use the same items. Which file
the original reads for each of these uses is [to verify].

### 5.4 `CarHeadersMemoryFile.bin` in the residual

Its 720-byte index is `kept`; each of the 35 copies is a prefix of another original file the base set can already
rebuild, so it is a node with `fate = "copy"` ([residual.md](residual.md) §3): `copy = { original =
"CARS/EXHAUST/GEOMETRY.BIN", offset = 0, size = 55488 }`, checked at import (`RES.5`). `GlobalMemoryFile.bin`,
`InGameMemoryFile.bin` and `FrontEndMemoryFile.bin` look alike [to verify per file]; they are not this page's.

## 6. Equality and verification

**Level Eq2 for every table on this page** (fixed records: the encoders are a few hundred lines and cheap), hence
Eq1: the cooked car records are the encoder's output, so the engine receives the very bytes it reads today, and
every model parsed from them (`CarType`, `Handling`, the upgraded and tuned records, `PartDb`, the presets, the
packages, the cost table, the unlocks, `Bounds`, `ClassCurves`) is equal by construction. The import proves:

1. per node: `encode(decode(node)) == node` (`bytes`). Car types, AI presets, presets and the part pools give
   every byte a field (`u_*`, `raw_text`, `string_pool`, tails), so their encoders cannot fail, as they must not:
   the engine keeps those as bytes. Cost records, packages and tuning unlocks have fill the parsed model does not
   hold (pads, unused slots, the high bytes of ids read as `u8`); an unusual fill there becomes patches of only
   that node (`patched`), or its shadow beyond RES.6's limit;
2. per unit: `H(models from the original) == H(models from the base set)` ([parity.md](parity.md) §2), for the
   models above and the raw `car_type_records`;
3. per car unit: the list of `CARS/<NAME>/` files with their states equals the unit manifest.

Tests (synthetic data, no game files): a record of 0x890 distinct-ish bytes (counter pattern, NaN and subnormal
patterns, −0.0 in float fields) decodes and encodes back unchanged, which proves the layout covers every byte
exactly once; the same for every record kind of §3.3–3.6; the part pools with shared lists, a trailing pool word
and string gaps; a pack's attribute override leaves every other part's attributes and every original index
unchanged; merges of car values change the dyno and the simulation hash, and an empty pack changes nothing.
Gates ([parity.md](parity.md) §3): the dyno and launch times at 30/60/120/200 fps, the shops' ratings, the career
playthrough, the fresh native profile byte for byte (`the_written_profile_is_the_originals`), shots of the
garage, car lot and race start.

## 7. Cooked form

**Cook unit `global`** (with the other global kinds). Model sections:

| Section | Content | Replaces today's read of |
|---|---|---|
| `car_records` | `Vec<(String, [u8; 0x890])>`, the merged records encoded, in type order | `car_type_records` / `parse_car_types` on GLOBALB (src/car/record.rs:110, :126) |
| `ai_presets` | `Presets` | src/ai/route/mod.rs:444 |
| `car_presets` | `Vec<Preset>` | src/car/record.rs:322 |
| `part_db` | `PartDb` (pools in order, private lists and attributes of overrides appended, slot groups) | crates/ob-formats/src/formats/parts.rs:162 |
| `part_costs` | `CostTable` | crates/ob-formats/src/formats/parts.rs:400 |
| `performance` | packages, tuning unlocks, bounds (`Bounds` and the stat ranges), `ClassCurves` | src/performance/packages.rs:42 and the others |

About 0.6 MB in all; nothing is large enough to be a zero-copy payload. **Cook unit `cars/<NAME>`**: the unit
manifest only; its meshes, textures and animations are cooked by their pages, and the assembled car (today's car
cache entry) stays build-specific prepared data, not normative.

## 8. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| CRS.1 | `index.toml` missing in a base set, a name in it without `types/<NAME>.toml`, or a type file not in it. |
| CRS.2 | A text field longer than its record field allows (N − 1 bytes) or with characters outside printable ASCII; a `raw_text` value whose bytes up to the first NUL differ from the text field, or `raw_text` in a mod pack. |
| CRS.3 | A merged car type that cannot run: NaN or infinite values, mass, wheel radius or rpm span not above 0, `forward_gears` outside 1–6, a camera count above 6 (packs and community sets). |
| CRS.4 | `ai_preset.parts` not ascending, repeated, or above 71. |
| CRS.5 | A preset whose `car` names no car type, or a slot key above 0xA9. |
| CRS.6 | Part database: an index out of range (group, name, list, model, string `at` not increasing), a list running past `lists`, group hashes not unique, (group, hash) not unique, `header` counts unlike the pools, or not exactly one of `string` and `string_pool` in a base set. |
| CRS.7 | A mod pack sets a field this page reserves to base sets (the part pools and record fields of §3.4; any field of `index.toml` or `unit.toml`). |
| CRS.8 | Cost table: `number` not equal to the position + 1 in a base set, or repeated. |
| CRS.9 | Performance: a repeated package (category, level), performance part id or tuning unlock (part, param); an id above 0x43; `param` above 24; `replaces` / `requires` naming no performance part. |
| CRS.10 | Car unit: `name` unlike its folder, `role = "car"` without a resolvable `car`, a car unit without `GEOMETRY.BIN` data or without any `TEXTURES.BIN` entry, a `data` file without `paths` once its kind has graduated, `paths` on an `empty` file. |
| CRS.11 | Import (warning): `GLOBALB.BUN` and `GlobalB.lzc` differ in a chunk of this page; the edition is `custom`. |
| CRS.12 | Import (warning): a derived field differs from its derivation, a text field needs `raw_text`, or a record holds fill the encoder does not write (unused package slots, cost pads); the field is written explicitly, or those bytes become patches (a shadow beyond RES.6's limit). |
| CRS.13 | Import (warning): the bytes after the last car record name a part group, so the original's past-the-end read would find a car; the edition is `custom`. |
| MRG.3 | (from [layering.md](layering.md)) Every operation §4 refuses, with the field named. |
<!-- /generated -->

## 9. Versioning and extension

- **Minor versions** may name open fields: a `u_*` key gets a name through a rename entry
  ([versioning.md](versioning.md) §4.3), and readers keep accepting the old key for the major version. They may
  also open refused merges once their save path exists (e.g. `rim_size` with the save sidecar), add derived-field
  checks, and add optional fields with defaults.
- **Add-on cars (the extension point).** Adding a car type needs: a type index after the base set's that never
  shifts an original one and is never 46 (0x2E), which the owned-bit walk uses for "a part of a shared group"
  (§5.2): an added car appended naively would take exactly that index; its own record, unit, parts
  (group, parts, models, strings and lists appended to the pools), AI presets and career unlock rows
  ([career.md](career.md)); a row in every engine table and constant keyed by type index (§3.9,
  [engine-tables.md](engine-tables.md) §9); save-safe identities in the save sidecar, the native save keeping a
  native car. Reserved for it: `ob.car-index` minor version with `[[added]] { pack, name }` entries listed after
  `order`; `[ext.OB_addon_car]` in a car type for its per-index engine rows until those become data; an `add`
  operation on `ob.part-db` for groups, parts, models, strings and lists keyed by `(pack id, local id)`
  ([conventions.md](conventions.md) §6.5). All are `MRG.3` until the car registry exists and the original's uses of
  the type index are traced.
- `[ext.OB_part_strings]` (proposal): strings a pack adds to the pool for its attribute overrides, keyed by pack.

## 10. Open points

- 46 open fields in the car record (465 of 2,192 bytes; §3.2), among them the suspension's `u_0x1c` (the code's
  `unknown_1c`: moved by the suspension upgrade, not read by the update), the gearbox's `u_0x04` (`unknown_2c4`),
  the gauge block 0x804–0x840, `u_0x850` (a second paint hash [guess]), `u_0x860` (five values like the zone caps
  [guess]), `u_0x87c` (racers and traffic differ). Types of the gauge fields are [guess].
- `kind_code = 1` (seen in the code, not in the data) is **open**; `split_camera_count` and `gauges.needle_deg` are
  **inferred**; `engine.turbo_torque_knm`'s name is **inferred**, its use **verified**.
- AI presets' `u_0x04` and `u_0x0e`, presets' `u_0x00`: values that look like left-over memory [guess].
- Part database: `header` entries other than the counts; model `u_0x01`; 15 of 75 group names and some attribute
  key names unknown (hashes are kept, names are tools' business).
- Chunks 0x34601 (colour names per type and skin; read, results dropped), 0x34608, 0x34609: kept, no decoder.
- `CarHeadersMemoryFile.bin` entries' two `u16` fields; which copy of the car records the original reads for each
  use (§5.3) [to verify]; the North American edition's folders and records (CIVIC, RSX) [to verify].

## 11. Sizes and timing

Reference install (European PC 1.2):

| Original | Size | Base set | Size [guess] |
|---|---|---|---|
| 0x34600 car types (46 × 2,192 + 8) | 100,840 B | `types/*.toml`, `index.toml` | ≈ 0.3 MB (≈ 6 KB a type; zero and derived fields omitted) |
| 0x34A31 ×46 AI presets (1,194 records) | 19,288 B | inside the type files | ≈ 50 KB |
| 0x30220 presets (32) | 26,368 B | `presets.toml` | ≈ 40 KB (stock slots omitted) |
| 0x80034602 part database (12,167 parts, 1,580 models, 4,636 attributes, 23,810 list words, 2,667 strings, 75 groups) + 0x34607 (170 + 18) | 349,204 + 1,648 B | `db.toml` + `db.lists.bin` (47,620 B) | ≈ 2 MB |
| 0x34A15 (30), 0x34A1A (62), 0x34A1C (86), 0x34A32, 0x34A33 | 11,400 + 1,488 + 3,440 + 120 + 1,072 B | `performance.toml`, `costs.toml` | ≈ 40 KB |
| the same in the second career container and in `GlobalB.lzc` | (copies) | none (same items) | 0 |
| 0x34601, 0x34608, 0x34609 | 7,444 B | residual (kept) | 7,444 B before deflate |
| `CarHeadersMemoryFile.bin` | 338,816 B | residual: index kept, the copies as `copy` nodes | 720 B |
| `CARS/` folders (63) and `CARS/TEXTURES.BIN` | 231 files, 757 MB | `unit.toml` ×64 (+ other pages) | ≈ 30 KB |

Shadow on this page after the first import: none for decoded nodes (every encoder exists from the start); kept
bytes as above. Import: under 0.1 s for all tables, the same again for the Eq2 checks [guess]; cooking `car_records`
and `part_db` from TOML ≈ 20 ms [guess], once per stack (cached).
