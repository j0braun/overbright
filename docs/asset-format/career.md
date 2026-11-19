# Asset format: career data, events, stages, tracks, magazines

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md), [layering.md](layering.md) and [residual.md](residual.md) apply and are not repeated.

This page defines how the career tables, the race-track records, the style-points table, the magazine data and the
track maps' calibration are stored. Field meanings come from `docs/formats/career-1.md` ("Career", "Sponsors",
"Messages / SMS", "World challenges"), `career-2.md` ("Trick table") and `career-3.md` (magazines); the tags
**verified**, **inferred** and **open** are theirs. How the career *evaluates* these tables (open events, stage ends,
unlocks) is engine code, not all of it traced yet; §10 lists what is open.

## 1. Scope

| Original data | Chunk | Kind (this page) |
|---|---|---|
| `GLOBAL/GlobalB.lzc` (unpacked; `GLOBAL/GLOBALB.BUN` is byte-identical in known editions): the career container, **first** copy (the game loads only the first, 0x512C00) | `0x80034A10` → `0x34A11` events, `0x34A12` shops, `0x34A14` brands, `0x34A16` milestones, `0x34A17` messages, `0x34A18` stages, `0x34A19` sponsors, `0x34A1B` world challenges, `0x34A1E` cash pickups, `0x34A1F` car unlocks, `0x34A1D` the string pool | `ob.career` |
| the same container | `0x34A15` performance packages, `0x34A1A` tuning unlocks, `0x34A1C` part costs | [cars.md](cars.md) |
| the **second** `0x80034A10` (8 events, 2 stages; the game loads only the first) | its children `0x34A15`, `0x34A1A`, `0x34A1C` | [cars.md](cars.md): the same items as the first container's where byte-identical (they are in the reference install) |
| the same | its other children (`0x34A1D`, `0x34A11`, `0x34A12`, `0x34A14`, `0x34A16`–`0x34A19`, `0x34A1B`, `0x34A1E`, `0x34A1F`) | residual, `kept` (no reader; not yet owned, [README.md](README.md) §3) |
| the race tracks | `0x34201` (131 × 0x128) | `ob.tracks`; the map fields `ob.track-maps` |
| the style-points tricks | `0x34A07` (39 × 0x30) | `ob.career` (`trick`) |
| `0x34202` (1,096 bytes) | – | residual, `kept` (no reader found) |
| `FRONTEND/MAGAZINES_FRONTEND.BIN`, `FRONTEND/MAGAZINES_SHOWCASE.BIN` | `0x30230`, `0x30231` | `ob.magazines` |
| the HUD mod's `map-overrides.tsv` (legacy pack) | – | `ob.track-maps` overrides |

Not here: the pixels of magazines and covers (`MAGAZINES_STREAMING_*.bin`, [textures.md](textures.md)); the route
triggers that place event markers, shops, photo spots, cash pickups and engage tips, and the markers that place shops
and start grids ([roads-ai.md](roads-ai.md)); the track bundle's world-animation instances that stand for the shops
(`0x37090`, [world.md](world.md)); message and label texts ([text.md](text.md)); movies ([movies.md](movies.md)); the
AI's upgrade presets, rating ranges and class curves ([cars.md](cars.md)); the engine's built-in tables (§1.1,
[engine-tables.md](engine-tables.md)).

### 1.1 Data or engine table

Career behaviour comes from data, from the engine's tables and constants, and from engine code. Only the first
row of the table is this page's.

| Fact | Where it lives |
|---|---|
| Events, stages, sponsors, brands, shops, milestones, cash pickups, car unlocks, messages, world challenges, tricks, track records, magazine setups, map calibration | **data**: the base set (this page) |
| Where events, shops, photo spots, cash pickups and tips are; start grids; shop positions | **data**: route triggers and markers ([roads-ai.md](roads-ai.md)) |
| Region 0's opponent pool (0x7F6DA4) and the Quick Race opponent names (0x7F6CD0); magazine, DVD and rating lines (`MAGAZINE_LINES`, `DVD_LINES`, `RATING_LINES`); World Map zooms | **engine tables** in the table holder (`opponent_cars`, `opponent_names`, `magazine_lines`, `dvd_lines`, `rating_lines`, `world_map_zooms`; `src/exe_tables/constants/frontend.rs:9`, `:19`, `:43`, `:57`, `:194`, `:230`) |
| The alternate and SUV opponent pools (0x7F6DC0, 0x7F6DDC); car-lot exclusions (0x7F7C50 / 0x7F7C48); Quick Race's always-offered cars (0x7F7C28 / 0x7F7C08); neighbourhood names (0x800C38); reputation by winning margin and reward class (0x7F7AC8); visual-rating steps (0x7FA230); minimap icons (0x7F4348); AI throttle caps (0x783A28) | **engine constants** written in the modules that use them (`src/frontend/career/events.rs:147`, `:151`; `lists.rs:7`–`14`; `triggers.rs:75`; `rewards.rs:53`; `src/frontend/screens/ingame_map.rs:142`; `src/ai/driver/mod.rs:104`) |
| Open-event picking (0x527D40), stage ends (0x532C70), sponsor offers (0x500610, 0x500670), race pay (0x532E60), car unlock checks (0x513C50), start cash (cheat bonuses) | **engine code**, not data |

Engine tables are built in and overridden only through [engine-tables.md](engine-tables.md) (`tables/` of mod packs and
community base sets); engine constants are not overridable in 1.0 and are candidates for the table holder
([engine-tables.md](engine-tables.md) §3.4).

### 1.2 Saves are not assets

Native profile saves keep the original's own format, so the original game still loads them
(`docs/formats/saves-options.md`). They are never part of a base set or a pack. They name career data by keys this
page keeps stable: events by `bin_hash(name)` (the visit table, 0x180 entries, and each stage record's win lists of
35 / 4 / 10), shops by hash (visit table), the signed sponsor by its name's hash, milestones and messages by hash
(milestone states, SMS inbox of 256), cash pickups by their `flag` index (a 256-byte table), the D-Day event by hash.
Therefore:

- no operation of this page may change an item's name, hash or `flag`, or add items (1.0 allows value merges only,
  §4), so a native save stays valid with any stack and without one;
- anything a pack *adds* later (events, shops, cars) is recorded only in the save **sidecar** beside the profile,
  with the stack's identity (pack id, version, content digest), while the native save keeps a native-valid fallback
  (the save sidecar, [layering.md](layering.md) §8). A save loaded without the pack shows "needs <pack>" and deletes nothing. The sidecar is user
  data like the save (docs/platform-dirs.md), not an asset; its format belongs to the saves documentation.

## 2. Files and keys

```text
global/career/
  career.toml          ob.career       events, stages, sponsors, brands, milestones, shops, cash pickups,
                                       car unlocks, messages, world challenges, tricks
  tracks.toml          ob.tracks       the race-track records (without the map fields)
global/maps/
  track-maps.toml      ob.track-maps   per track number: the minimap and World Map calibration
frontend/magazines/
  magazines.toml       ob.magazines    photo-shoot cameras, cover setups, environment maps, showcase
```

A pack may split its overrides of one kind over several files of the same kind in the same folder (e.g.
`global/career/economy.toml`); they apply in byte order of their file names ([layering.md](layering.md) §3.1). Two
files of one pack may hold entries for the same key, but MUST NOT set the same field of one item (`CRR.12`), so the
order never decides a value within one pack.

| Short name | Key | Notes |
|---|---|---|
| `event` | `event:<NAME>` (`event:S2_CIRCUIT_3`) | the name from the pool; the record's `+0x08` is `bin_hash(name)` in all 224 events (checked at import; an optional `hash` field holds it only where it differs, §5) |
| `stage` | `stage:<n>` | `+0x00`, 0–5 |
| `sponsor`, `brand`, `milestone`, `shop`, `message`, `challenge` | `<kind>:<NAME>` | names unique per table; the stored hash equals `bin_hash(name)` for every sponsor, brand, milestone and shop (checked) |
| `cash-pickup` | `cash-pickup:<flag>` | `+0x03`, the save's flag index; unique |
| `car-unlock` | `car-unlock:<car>` | the car type's name, or its hash when unknown; unique |
| `trick` | `trick:<index>` | `+0x04`, equal to the position in all 39 records |
| `track` | `track:<number>` (`track:4014`); a later record with the same number `track:<number>~<k>` (k = 2, 3, …) | numbers are unique except 4400, whose two records are identical |
| `track-map` | `track-map:<number>` | one per track number |
| `magazine-camera`, `magazine-setup`, `envmap`, `showcase-camera`, `showcase-light`, `showcase-setup` | `<kind>:<name or 0xHASH>` | by the record's hash |

Fields holding a `bin_hash` are **`nameref`**s ([conventions.md](conventions.md) §6.3): the name when known, else the
hash. The importer's names are listed in [roads-ai.md](roads-ai.md) §3.1.

## 3. Source form

The field tables below give, for each original record, every byte: a TOML field, a value derived from other fields,
a checked fill the encoder writes, or bytes kept only in the residual (as patches or a **shadow**, §6). Column **M** marks fields a mod pack may `merge` in 1.0 (§4).
"Req." is for base sets; a pack's merge entry holds its key and the fields it changes (layering §3.1). The schemas
check shape only, so they require just the keys; a reader checks a base set's required fields (CONV.5).

### 3.1 `ob.career` (`global/career/career.toml`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.career/1.0` |
| `event`, `stage`, `sponsor`, `brand`, `milestone`, `shop`, `cash_pickup`, `car_unlock`, `message`, `challenge`, `trick` | array<table> | – | no | `[]` | The tables, each in original order. |
| `extras` | table | – | no | – | conventions §9. |

**`event`** (0x88 bytes, `src/frontend/career/events.rs:60`):

| Field | Type | Unit | M | Req. | Default | Meaning (offset) |
|---|---|---|---|---|---|---|
| `name` | name | – | – | yes | – | `+0x00` (pool): the key; `+0x08` is its hash. |
| `movie`, `won_movie` | string | – | M | no | `""` | `+0x02`, `+0x04` (pool): movie names played before the race and after winning it ([movies.md](movies.md)). |
| `tier` | u8 | – | – | no | `0` | `+0x0C`: 3 a URL race opened by wins, 4 the stage's last URL race. |
| `suv` | u8 | – | – | no | `0` | `+0x0D`: 1 an SUV event (read as `== 1`). |
| `kind` | enum or u8 | – | – | yes | – | `+0x0F`: `circuit`, `sprint`, `street_x`, `special`, `drag`, `drift`; other values as integers. |
| `requirement` | u32 | – | – | no | `0` | `+0x10`: a URL race's requirement (written in hex). |
| `races` | array<table> | – | M* | yes | – | `+0x18`, count `+0x7E`, 1–4: `{ track = u16, variant = u8, laps = u8 }`; more than one is a tournament. The event's track, direction (`variant` bit 0) and laps are the first's. *A merge may change `laps` only (CRR.6). |
| `marker` | nameref | – | – | no | `"0x00000000"` | `+0x28`: the event marker's route trigger (`EV_nn`). |
| `purse` | u32 | $ | M | no | `0` | `+0x30`: the cash a win pays. |
| `category` | enum or u8 | – | – | yes | – | `+0x34`: `scripted` (0), `sponsor` (1), `url` (2), `normal` (3). |
| `traffic` | i8 | – | M | no | `0` | `+0x36`: the race's traffic level. |
| `stage` | u8 | – | – | yes | – | `+0x37`. |
| `goals` | array<table> | – | – | no | `[]` | `+0x38`, count `+0x35`, at most 8: `{ kind = u8, target = nameref }` (kind 4: entering the shop `target` wins). |
| `opponents` | array<table> | – | M* | no | `[]` | `+0x40`, count `+0x7C`, at most 5: `{ name = string (pool), mode = u8, skill = u8, car = nameref ("RANDOM" or a car type), difficulty = [u8; 3], block = i8 }`. *Replaced whole; a merge keeps the count (CRR.6). An event has goals or opponents, not both (CRR.5). |
| `throttle` | i8 | – | M | no | `0` | `+0x7D`: the AI racers' throttle level. |
| `always` | u8 | – | – | no | `0` | `+0x7F`: always open, up to the stage's cap. |
| `sender` | nameref | – | M | no | `"0x00000000"` | `+0x84`: the race dialog's sender label ([text.md](text.md)). |

Shadow bytes: `+0x06` (u16, **open**), `+0x0E`, `+0x14..+0x17` (`+0x14` is 100 or 0, **open**), `+0x2C` (u32, **open**,
set in one event), `+0x80..+0x83`, the three bytes after each goal's kind, and every byte of the goal / opponent area
(`+0x38..+0x7B`) past the counts, which holds old values in 135 events.

**`stage`** (0x50 bytes, `src/frontend/career/data.rs:105`):

| Field | Type | Unit | M | Req. | Default | Meaning (offset) |
|---|---|---|---|---|---|---|
| `stage` | u8 | – | – | yes | – | `+0x00`: the key. |
| `outrun_bank` | i16 | $ | M | no | `0` | `+0x02`: paid for an outrun win, taken for a loss. |
| `cover_cash` | i16 | $ | M | no | `0` | `+0x04`: a photo shoot's pay per visual-rating point. |
| `u_0x06` | u16 | – | – | no | `0x88` | **open** (0x88 in every stage). |
| `sponsors` | array<nameref> | – | – | no | `[]` | `+0x08`, count `+0x01`, at most 5: brand names. Slots past the count are zero (written as zero). |
| `sponsor_wins` | array<i16>[5] | – | – | no | zeros | `+0x1C`: normal wins before each sponsor is offered. |
| `u_0x26` | u16 | – | – | no | `0` | **open** (non-zero in one stage). |
| `final_event` | nameref | – | – | no | `"0x00000000"` | `+0x28`: the event that ends the stage (none for stage 1). |
| `u_0x2c` | array<u8>[4] | – | – | no | zeros | **inferred** race counts by type. |
| `caps` | array<i8>[6] | – | – | no | zeros | `+0x30`, `+0x31`, `+0x33`, `+0x34`, `+0x32`, `+0x37`: open events per slot (circuit, drag, drift, sprint, street X, SUV). |
| `u_0x35`, `u_0x36` | u8 | – | – | no | `0` | **open**. |
| `u_0x38` | array<u8>[8] | – | – | no | zeros | **open**. |
| `required_wins` | i8 | – | – | no | `0` | `+0x40`; also caps the stage's outrun wins. |
| `always_cap` | i8 | – | – | no | `-1` | `+0x41`: most always-open events at once (−1 no limit). |
| `skill_wins` | i16 | – | M | no | `0` | `+0x42`: wins the opponents' skill range spreads over. |
| `skill` | array<f32>[2] | – | M | no | `[0.0, 0.0]` | `+0x44`, `+0x48`: skill at none and at all of them. |
| `u_0x4c` | f32 | – | – | no | `10.0` | **open** (10.0 in every stage). |

Every byte of a stage record is a field, so its encoder rebuilds the chunk (§6).

**`sponsor`** (0x10, all bytes read): `name` (`+0x00` pool, key; `+0x08` is its hash), `purse` (u16 `+0x02`, $, M),
`races` (u8[4] `+0x04`: required race types), `bonus` (i16 `+0x0C`, $, M: signing bonus), `reputation` (i16
`+0x0E`: average points a win must have).

**`brand`** (0x44): `name` (text in `u8[32]` at `+0x00`, key; `+0x40` is its hash), `display` (text in `u8[32]` at
`+0x20`, the name shown). Shadow: bytes after each string's NUL (old text in 33 and 31 records).

**`milestone`** (0x40, `src/frontend/career/data.rs:216`):

| Field | Type | Unit | M | Req. | Default | Meaning (offset) |
|---|---|---|---|---|---|---|
| `name` | name | – | – | yes | – | `+0x00`, text in `u8[32]`, key; `+0x20` is its hash. |
| `kind` | u8 | – | – | yes | – | `+0x24`: 1 / 3 magazine, 2 / 4 DVD. |
| `stage` | u8 | – | – | yes | – | `+0x25`. |
| `cash` | i16 | $ | M | no | `0` | `+0x26`: the photo shoot's pay before the stage's per-rating share. |
| `cover` | nameref | – | – | no | `"0x00000000"` | `+0x28`: the cover (`<cover>_TITLE` its title). |
| `spot` | nameref | – | – | no | `"0x00000000"` | `+0x2C`: the photo-shoot route trigger. |
| `offer` | u8 | – | – | no | `0` | `+0x34`: 0 any shop, 1 the shop `offer_at`, 2 winning the event `offer_at`. |
| `required` | u8 | – | – | no | `0` | `+0x35`: needed by the stage's last URL race. |
| `offer_at` | nameref | – | – | no | `"0x00000000"` | `+0x38`. |
| `rating` | f32 | – | – | no | `0.0` | `+0x3C`: the visual rating a DVD asks for. |

`+0x30..+0x33` and `+0x36..+0x37` are zero in every record and written as zero. Bytes after the name's NUL (old text
in 16 records) are kept as patches (§5).

**`shop`** (0xA0, `src/frontend/career/data.rs:160`): `name` (text in `u8[32]` at `+0x00`, key; `+0x38` is its hash),
`movie` (text in `u8[24]` at `+0x20`), `trigger` (nameref `+0x3C`, the route trigger that stands for it), `scene_file`
(text in `u8[16]` at `+0x40`), `kind` (u8 `+0x50`: 0 garage … 6 unique parts), `announced` (u8 `+0x51`),
`requirement` (u8 `+0x9C`: 0 none, 1 an event won, 2 URL, 3 normal, 4 sponsor wins) with `requirement_value` (u32
`+0x74`), `stage` (u8 `+0x9D`), `u_0x78` (bytes (36) `+0x78`, **open**: not read; old text in 9 records; default 36
zero bytes). `+0x52..+0x73` and `+0x9E..+0x9F` are zero in every record and written as zero; bytes after a text
field's NUL (old text in 16 records) are kept as patches (§5).

**`cash_pickup`** (0x0C, all bytes read, `src/frontend/career/data.rs:51`): `cash` (u16 `+0x00`, $, M), `condition` (u8
`+0x02`), `flag` (u8 `+0x03`, key), `value` (u32 `+0x04`), `trigger` (nameref `+0x08`, its kind-19 route trigger).

**`car_unlock`** (0x0C, every byte a field): `car` (nameref `+0x00`, key), `u_0x04` (nameref `+0x04`, default
`"0x00000000"`: **inferred** the event for the other region setting; 0x513C50 reads `+0x08` because 0x570A40 is
non-zero), `event` (nameref `+0x08`, the event whose win frees it; 0 from the start).

**`message`** (0x14, `src/frontend/sms/table.rs:6`): `name` (pool `+0x00`, key; its labels `<name>_SUBJECT`,
`<name>_BODY`), `condition` (u8 `+0x02`), `category` (u8 `+0x03`, the mailbox), `urgent` (u8 `+0x04`), `icon` (u8
`+0x05`), `values` (u32[2] `+0x08`), `sender` (nameref `+0x10`, a label). Shadow: `+0x06` (0x88 everywhere), `+0x07`.

**`challenge`** (0x18, all bytes read, `src/frontend/sms/triggers.rs:19`): `name` (pool `+0x00`, key), `marker` (pool
`+0x02`, a route trigger's name), `stage` (i16 `+0x04`), `condition` (u8 `+0x06`), `count` (u8 `+0x07`), `message`
(nameref `+0x08`), `milestone` (nameref `+0x0C`), `time_s` (i32 `+0x10`, M), `kind` (u8 `+0x14`), `slots` (u8[3]
`+0x15`).

**`trick`** (0x30, `0x34A07` after 8 bytes of `0x11` fill, `src/drift.rs:15`; every byte a field, Eq2):

| Field | Type | Unit | M | Req. | Default | Meaning (offset) |
|---|---|---|---|---|---|---|
| `name` | nameref | – | – | yes | – | `+0x00`: the trick's label. |
| `index` | u8 | – | – | yes | – | `+0x04`. |
| `u_0x05` | u8 | – | – | no | `0` | **open**. |
| `continuous` | u8 | – | – | no | `0` | `+0x06`: points accumulate while fed. |
| `mask` | u8 | – | – | yes | – | `+0x07`: the races it exists in (1 other, 2 drag, 4 drift, 8 point to point, 0x10 street X). |
| `min_time` | f32 | s; points for drift tiers | M | no | `0.0` | `+0x08`: seconds fed before it counts (drift tiers: the points they announce at). |
| `timeout_s` | f32 | s | M | no | `0.0` | `+0x0C`. |
| `threshold` | f32 | points | M | no | `0.0` | `+0x10`: raw points to pass. |
| `cap` | f32 | points | M | no | `0.0` | `+0x14`: most points one run makes (0 none). |
| `max_multiplier`, `multiplier_step` | f32 | – | M | no | `0.0` | `+0x18`, `+0x1C`. |
| `u_0x20`, `u_0x24` | f32 | – | – | no | `0.0` | **open**. |
| `points` | f32 | points | M | no | `0.0` | `+0x28`: points per unit of rate. |
| `max_count` | i8 | – | M | no | `-1` | `+0x2C`: most runs that score (−1 no limit). |
| `u_0x2d` | u8 | – | – | no | `255` | **open** (255 everywhere). |
| `u_0x2e` | u16 | – | – | no | `0` | **open**. |

Example (shortened; [examples/career/career.toml](examples/career/career.toml)):

```toml
schema = "ob.career/1.0"

[[event]]
name = "S2_CIRCUIT_3"
kind = "circuit"
category = "normal"
stage = 2
races = [{ track = 4013, variant = 0, laps = 2 }]
marker = "EV_12"
purse = 1500
traffic = 1
throttle = 3
opponents = [
  { name = "RACER_A", mode = 3, skill = 0, car = "RANDOM", difficulty = [1, 2, 3], block = 0 },
]

[[stage]]
stage = 2
outrun_bank = 100
cover_cash = 100
sponsors = ["STREETGLOW", "EUROLITE"]
sponsor_wins = [1, 1, 0, 0, 0]
final_event = "S2_URL_3"
caps = [2, 2, 2, 2, 2, 3]
required_wins = 10
always_cap = 4
skill_wins = 16
skill = [0.0, 3.0]

[[trick]]
name = "0x7B3C1A20"
index = 0
continuous = 1
mask = 4
min_time = 0.1
timeout_s = 0.3
threshold = 50.0
points = 120.0
```

### 3.2 `ob.tracks` (`global/career/tracks.toml`)

`track` — one 0x128-byte record of `0x34201` (`src/frontend/career/tracks.rs:5`), in table order (the Quick Race
order). Every byte is a field here or in `ob.track-maps` (Eq2):

| Field | Type | Unit | M | Req. | Default | Meaning (offset) |
|---|---|---|---|---|---|---|
| `number` | u16 | – | – | yes | – | `+0x8A`. |
| `copy` | u8 | – | – | no | absent | The *k*-th record with this number (2, 3, …; key `track:<number>~<k>`); absent for the first. |
| `name` | string ≤ 31 | – | M | yes | – | `+0x00`, text in `u8[32]`: `(4014) Short Circuit #4`. |
| `path` | string ≤ 31 | – | – | yes | – | `+0x20`: `Location4\Track_4014`. |
| `region` | string ≤ 7 | – | – | yes | – | `+0x40`: `L4RA`, the unit of its routes ([roads-ai.md](roads-ai.md)). |
| `region_path` | string ≤ 31 | – | – | no | `""` | `+0x48`: **inferred** the region's folder (`Location4\RegionA`); not read. |
| `u_0x68` | u32 | – | – | no | `4` | **open**. |
| `location` | string ≤ 15 | – | – | no | `""` | `+0x6C`: **inferred** (`Location4`); not read. |
| `icon` | u32 | – | – | no | `0` | `+0x7C`: the Quick Race icon kind. |
| `drift_kind` | u32 | – | – | no | `0` | `+0x80`. |
| `u_0x84`, `u_0x85` | u8 | – | – | no | `1`, `0` | **open**. |
| `reversible` | u8 | – | – | no | `0` | `+0x86`: 1 runs both ways. |
| `u_0x87` | u8 | – | – | no | `0` | **open**. |
| `hidden` | u8 | – | M | no | `0` | `+0x88`: kept out of the Quick Race lists. |
| `u_0x89` | u8 | – | – | no | `0` | **open**. |
| `u_0x8c` | u16 | – | – | no | `number` | **open**; equals `number` in every record. |
| `u_0x8e` | u16 | – | – | no | `0` | **open**. |
| `u_0x90` | u32 | – | – | no | `0xB05E7F4C` | **open** (two values). |
| `flags` | u32 | – | – | no | `0` | `+0x94`: bits 0–3, 11–13 pick the AI speed mode. |
| `length_m` | u32 | m | – | no | `0` | `+0x98`: the route length shown. |
| `default_record_ms` | array<f32>[2] | ms | M | no | `[0.0, 0.0]` | `+0x9C`: the record shown when the profile has none, forward / backward. |
| `u_0xa4` | bytes (8) | – | – | no | zeros | **open**. |
| `u_0xb8`, `u_0xbc` | u32 | – | – | no | `0`, `0x1555` | **open**. |
| `u_0xc4`, `u_0xc8` | u32 | – | – | no | `1`, `1` | **open**. |
| `u_0xcc` | bytes (24) | – | – | no | 24 × `ff` | **open**. |
| `u_0xe4` | u32 | – | – | no | `0xFFFF` | **open**. |
| `u_0xe8` | u32 | – | – | no | `0` | **open**. |
| `traffic` | array<i8>[8] | – | M | no | zeros | `+0xEC`: traffic cars to preload by level and direction. |
| `start_gap_off` | array<u8>[2] | – | – | no | zeros | `+0xF4`: per direction, no start gap when set. |
| `start_kind` | i8 | – | – | no | `0` | `+0xF6`: the start scenes' kind. |
| `u_0xf7` | u8 | – | – | no | `0` | **open**. |
| `start_gap_m` | array<f32>[2] | m | M | no | `[0.0, 0.0]` | `+0xF8`: per direction, the gap past the start line in which traffic is not placed before the green. |
| `drag_fill_m` | array<f32>[2] | m | M | no | `[0.0, 0.0]` | `+0x100`: per direction, the drag set-up fill's step. |
| `traffic_with` | array<f32>[4] | – | M | no | zeros | `+0x108`: per level, the odds a placed traffic car goes the player's way. |
| `u_0x125` | bytes (3) | – | – | no | zeros | **open**. |

`+0xAC..+0xB7`, `+0xC0` and `+0x118..+0x124` are the map fields of `ob.track-maps` (§3.3).

```toml
schema = "ob.tracks/1.0"

[[track]]
number = 4014
name = "(4014) Short Circuit #4"
path = "Location4\\Track_4014"
region = "L4RA"
region_path = "Location4\\RegionA"
location = "Location4"
icon = 5
reversible = 1
flags = 0x0001
length_m = 2140
traffic = [12, 12, 8, 8, 6, 6, 4, 4]
start_kind = 1
start_gap_m = [60.0, 60.0]
drag_fill_m = [31.0, 31.0]
traffic_with = [0.5, 0.5, 0.5, 0.5]
```

### 3.3 `ob.track-maps` (`global/maps/track-maps.toml`)

One `track` entry per track number; the importer takes the values from the track's record (both copies of a
duplicated number hold the same values).

| Field | Type | Unit | M | Req. | Default | Meaning (offset in the track record) |
|---|---|---|---|---|---|---|
| `number` | u16 | – | – | yes | – | the key. |
| `left`, `top` | f32 | m | M | yes | – | `+0xAC`, `+0xB0`: world x at the minimap's left edge, world y at its top. |
| `size` | f32 | m | M | yes | – | `+0xB4`: the world size the map covers. |
| `zoom` | f32 | – | M | yes | – | `+0xC0`: the window is 0.3 / zoom of the map; > 0. |
| `window_left`, `window_top`, `window_size` | f32 | px (of 512) | M | no | `0.0` | `+0x118`, `+0x11C`, `+0x120`: the World Map widget's window on `TRACKMAP<n>`; size 0 = whole map, ≥ 0. |
| `window_fixed` | u8 | – | M | no | `0` | `+0x124`: the widget shows the window at once. |

The HUD mod's six calibration names map to `left` (`TrackMapCalibrationOffsetX`), `top` (`…OffsetY`), `zoom`
(`TrackMapCalibrationZoomIn`), `window_left` (`MenuMapZoomOffsetX`), `window_top` (`MenuMapZoomOffsetY`) and
`window_size` (`MenuMapZoomWidth`), `src/frontend/enh/map_pack.rs:19`. The window is three scalars, not an array, so a
pack can change one of them without restating the others (layering §3.2).

```toml
schema = "ob.track-maps/1.0"

[[track]]
number = 4701
left = 160.0
window_size = 200.0
```

(an override as the HUD mod's legacy pack gives it; a base set's entry has every required field).

### 3.4 `ob.magazines` (`frontend/magazines/magazines.toml`)

From `MAGAZINES_FRONTEND.BIN` (`0x30230`: three counts, then 64 camera slots of 0x38, 32 environment-map slots of
0x18, 32 setup slots of 0x68; `src/formats/magazines.rs:1`) and `MAGAZINES_SHOWCASE.BIN` (`0x30231`: three counts,
then 64 camera slots of 0x2C, 32 light slots of 0x0C, 32 setup slots of 0x38). The counts are the list lengths here.
Unused slots are zero in the first file; in the second every unused camera slot holds the same default camera
(params `[0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 45.0, 0.0, 0.0]`, hash 0). The importer checks both (else the file is `kept`).

| Field | Type | Req. | Meaning (offset in the slot) |
|---|---|---|---|
| `schema` | string | yes | `ob.magazines/1.0` |
| `camera` | array<table> ≤ 64 | no | `name` (nameref `+0x00`), `flags` (u8 `+0x04`: car lights), `car` (vec2 `+0x08`, M), `lens` (f32 `+0x10`, M), `target` (vec3 `+0x14`, car space, M), `eye` (vec3 `+0x20`, M), `extra` (vec2 `+0x2C`, M), `roll_deg` (f32 `+0x34`, M). `+0x05..+0x07` are 0 (checked). |
| `envmap` | array<table> ≤ 32 | no | `name` (nameref `+0x00`), `light` (nameref `+0x04`: the car's light material), `file` (string ≤ 15 `+0x08`: `FRONTEND\ENVMAPS\<file>.BIN`). |
| `setup` | array<table> ≤ 32 | no | `name` (nameref `+0x00`, `MAGAZINE_SETUP_<MAG>`), `counts` (u32[6] `+0x08`: how many of each list are drawn from), `backgrounds` (≤ 3, `+0x20`), `mastheads` (≤ 2, `+0x2C`), `models` (≤ 5, `+0x34`), `extra_layers` (≤ 3, `+0x48`, `<MAG>_XT_nn`), `cameras` (≤ 3, `+0x54`), `envmaps` (≤ 2, `+0x60`): nameref lists, trailing zero slots dropped. `+0x04` is 0 (checked). |
| `showcase` | table | no | `camera` (≤ 64: `name`, `params` f32[9] `+0x08`, M; `+0x04` is 0, checked), `light` (≤ 32: `name`, `u_0x04` u32, `u_0x08` u32, **open**), `setup` (≤ 32: `cover`, `counts` u32[4], `mastheads` ≤ 2, `extra_layers` ≤ 1, `cameras` ≤ 3, `lights` ≤ 2; `+0x04` is 0, checked). |

```toml
schema = "ob.magazines/1.0"

[[camera]]
name = "0x1F00A3C4"
flags = 0x05
car = [0.0, 1.5]
lens = 0.85
target = [0.25, 0.0, 0.5]
eye = [4.5, -2.25, 1.125]
roll_deg = 0.0

[[envmap]]
name = "0xC2082D5F"
light = "0x3E7A9C01"
file = "Showroom"

[[setup]]
name = "MAGAZINE_SETUP_EXAMPLE"
counts = [1, 1, 1, 0, 1, 1]
backgrounds = ["0x6A1B2C3D"]
mastheads = ["0x6A1B2C3E"]
models = ["0x6A1B2C3F"]
cameras = ["0x1F00A3C4"]
envmaps = ["0xC2082D5F"]
```

### 3.5 Limits

| What | Limit |
|---|---|
| Events, shops, milestones, messages, challenges, cash pickups, car unlocks per table | 4,096 (cash pickups: 256, one per flag) |
| Stages | 8 (the save's stage records) |
| Races / opponents / goals per event | 4 / 5 / 8 |
| Sponsors per stage | 5 |
| Pool strings (names, movies) | 63 characters each; 65,535 bytes of pool in all |
| Track records; track name / path | 4,096; 31 characters |
| Tricks | 256 |
| Magazine cameras / env maps / setups | 64 / 32 / 32 (showcase 64 / 32 / 32) |

## 4. Layering

| Kind | merge | add | replace | disable |
|---|---|---|---|---|
| `ob.career` | fields marked **M** (CRR.3 otherwise) | MRG.3 | MRG.3 | MRG.3 |
| `ob.tracks` | fields marked **M**; a merge of `track:<n>` applies to every record with that number (CRR.7) | MRG.3 | MRG.3 | MRG.3 |
| `ob.track-maps` | every field except `number` | MRG.3 | MRG.3 | MRG.3 |
| `ob.magazines` | camera and showcase-camera values marked **M** | MRG.3 | MRG.3 | MRG.3 |

Why so narrow: **M** fields are values the engine reads as numbers or names of other items; none is a key, a count,
a stage membership, a category or a rule input whose evaluation is not yet traced. Magazine setups are refused
because the cover pick draws on the game's random generator as many times as the counts say
(`src/formats/magazines.rs:204`), so changing them changes later draws. Lists are replaced whole (`races`,
`opponents`) under the conditions of CRR.5 and CRR.6. Clashes are reported per item and field (layering §4).
Wildcards: none. `disabled` is refused for every item of this page (MRG.3): an event, stage or shop that the engine
treated as absent would change the order and the counts a native save follows (§1.2).

**Mods off** gives exactly the base set's values: career, tracks, map calibration and magazines are the original's.

**Extension point (career events).** Once the career's evaluation of events is traced (§10, open point 1), a minor
version may allow `add` of `event` items keyed
`event:<pack>:<id>` (their hash `bin_hash("<pack>:<id>")` checked against the stack, CONV.15), with a reserved
`marker` route trigger added by the same pack ([roads-ai.md](roads-ai.md) §4, `ob.route-edits`), a reserved field
`races[].race` naming a custom race (`race:<pack>:<id>`) instead of a base track, and progress kept only in the save
sidecar (§1.2). Such packs run in a career profile of their own. Added events go after the originals
(layering §3.3), so the open-event picker's order over the original table stays as it is. Until then `add` is
refused and those names are reserved. The same applies later to shops, milestones, messages and challenges.

### 4.1 The HUD mod's calibration (legacy)

The HUD mod is mounted as the legacy pack `hud-mod` (layering §7). Its `map-overrides.tsv` (rows `<track> <field>
<value>`, `#` comments) becomes `global/maps/track-maps.toml` merge entries:

- each row sets the field of §3.3's mapping; the file is refused as a whole on an unknown field, a duplicate
  `(track, field)`, a non-finite value, `zoom ≤ 0` or `window_size < 0` (today's all-or-nothing parse,
  `src/frontend/enh/map_pack.rs:8`);
- a row is converted only when the pack holds the track's map picture (`TRACKMAP<n>` by hash or by name,
  `src/frontend/enh/map_pack.rs:53`), as today; other rows are dropped with a note;
- switching the pack on or off at run time recomputes the merged view from the base set's values, which is what
  `reapply` does today (`src/frontend/enh/map_pack.rs:87`).

## 5. Import

1. Unpack `GLOBAL/GlobalB.lzc` (JDLZ); check it equals `GLOBAL/GLOBALB.BUN` when that file exists (known editions:
   identical) so both residual maps point at the same items.
2. Take the **first** `0x80034A10`: read the pool (`0x34A1D`), then each table in the field tables' mapping. Strings
   are read up to the first NUL; pool offsets resolve to whole strings (all 1,594 references point at a string's
   start). Hashes stored next to names (`+0x08` of events, sponsors' `+0x08`, brands' `+0x40`, milestones' `+0x20`,
   shops' `+0x38`) are checked to equal `bin_hash(name)`; a mismatch stores the record's own hash in the item's
   optional `hash` field (hash32, written only then) and makes the edition `custom`.
3. Resolve hashes to names with the dictionary (the career's own names, the pool's strings, the car type names,
   [roads-ai.md](roads-ai.md) §3.1).
4. `0x34201` → `tracks.toml` and `track-maps.toml`; `0x34A07` → `trick` entries (after the 8-byte `0x11` fill);
   `0x30230` / `0x30231` → `magazines.toml`.
5. Residual nodes: the container `0x80034A10` (first) as `container`; its children of this page `decoded`, with the
   proof of §6 (`bytes` for stages, cash pickups and car unlocks; `patched` for milestones and shops, the bytes after
   text terminators as patches, [residual.md](residual.md) §3.2; `values` with a shadow for the string pool, events,
   sponsors, brands, messages and challenges); `0x34A15`, `0x34A1A`, `0x34A1C` as [cars.md](cars.md) says, in both
   containers; the second container as `container`, its other children and `0x34202` `kept` with reasons; `0x34201`,
   `0x34A07`, `0x30230`, `0x30231` `decoded` with proof `bytes`. A node whose patches would exceed RES.6's limit
   (1/16 of the node) falls back to `values`.

The importer runs with no pack mounted: today's `CareerData::load` applies the HUD mod's calibration when its pack is
active (`src/frontend/career/data.rs:350`); the Eq1 reference hash is taken before any pack.

## 6. Equality and verification

| Kind / item | Level | Proof | Shadow |
|---|---|---|---|
| `ob.career` stages, cash pickups, car unlocks | **Eq2** | the encoder rebuilds each chunk (`bytes`) | none |
| `ob.career` milestones, shops | **Eq2** | the encoder rebuilds each chunk except the bytes after text terminators, kept as patches (`patched`) | patches: 64 and 167 bytes (RES.6 allows 120 and 240) |
| `ob.career` string pool, events, sponsors, brands, messages, challenges | **Eq1** | the model from the original equals the model from the base set (`values`) | 47,668 bytes (six chunks; ≈ 18 KB deflated [guess]) |
| `trick` | **Eq2** | the encoder rebuilds `0x34A07` (fill + records) | none |
| `ob.tracks` + `ob.track-maps` | **Eq2** | the encoder writes each 0x128-byte record from both files | none |
| `ob.magazines` | **Eq2** | the encoder writes both files, unused slots as checked | none |

Why not `patched` for all of them (measured on the reference install): the string pool holds 218 strings no table
reads (2,243 of its 10,492 bytes; RES.6 allows 655), the events hold old values in unused race, goal and opponent
slots (2,258 non-zero bytes in 135 events; RES.6 allows 1,904), and the brands old text after their terminators
(1,477 of 2,380 bytes; RES.6 allows 148). Events, sponsors, messages and challenges name pool strings by their
offsets, so their encoders need the pool's layout, which only the pool's own bytes give. These six chunks keep a
shadow (residual.md §3.2); §10, open point 7, says what would bring the pool-dependent ones to Eq2.

**Eq1 model**: `CareerData` (`src/frontend/career/data.rs:5`) without the parts of other pages (`costs`, `packages`,
`stat_ranges`, `tuning`, `makes`, `suvs`), the track map (`HashMap<u16, Track>` and each record's `order`), the trick
list, `Magazines` and `Showcase`. Order is data: events keep their index (`Event::index`), which the open-event
picker and the World Map follow.

Tests: Eq1 / Eq2 on the reference install; the HUD mod conversion test (both forms give the same merged tracks);
a test that every **M** field merged to a new value changes only that field in the model; a save test
(`tests/native_save.rs`) on a stack with a career merge pack, checking the native file names only base items and
loads unchanged without the pack.

## 7. Cooked form

- **Cook units**: `global` for `ob.career` and `ob.tracks` (`global/maps/track-maps.toml` is in the same unit,
  because the track records carry the calibration); `frontend` for `ob.magazines`.
- **Model sections**: `career.events`, `career.stages`, `career.sponsors`, `career.brands`, `career.milestones`,
  `career.shops`, `career.cash_pickups`, `career.car_unlocks`, `career.messages`, `career.challenges`, `career.tricks`,
  `career.tracks`; `frontend.magazines`.
- **Zero-copy**: nothing large; every section is copied.
- **Prepared, not normative**: the World Map's event lists per stage, the open-event pools, the stage-goal rows of the
  statistics screens.

## 8. Errors

| Rule | Text |
|---|---|
| CRR.1 | Two items of one table with the same key; a `hash` field in a mod pack, or one equal to `bin_hash(name)` (it is written only when they differ). |
| CRR.2 | A string longer than its record field (§3.5), or with a character outside printable ASCII (the records hold single-byte text). |
| CRR.3 | A merge of a field not marked **M** (this kind's case of MRG.3; reported as CRR.3 with the field). |
| CRR.4 | `races` empty or with more than 4 entries; `opponents` over 5; `goals` over 8; `sponsors` over 5. |
| CRR.5 | An event with both `goals` and `opponents`. |
| CRR.6 | A merge of `races` that changes a track or variant or the number of races; a merge of `opponents` that changes their number. |
| CRR.7 | Two track records with the same number that differ after merging. |
| CRR.8 | `track-maps`: a non-finite value, `zoom ≤ 0`, `window_size < 0`, or a number with no track record. |
| CRR.9 | A legacy `map-overrides.tsv` with an unknown field, a duplicate row or a malformed line (the HUD mod's pack is not mounted). |
| CRR.10 | `magazines`: a list longer than its slots. |
| CRR.11 | (warning) A track whose `region` has no `world/<REGION>/` unit in the stack: its races cannot run and it is not offered. |
| CRR.12 | Two files of one pack set the same field of one item (§2). |

## 9. Versioning and extension

- 1.x minors may add optional fields with defaults: names for shadowed or `u_*` bytes, more **M** fields once
  their rules are traced, the event `add` of §4 and its reserved fields, the pool's unread strings (§10).
- Reserved: `event:<pack>:<id>` keys and `races[].race`.
- `[ext.<NAME>]` tables are allowed in every file of this page (conventions §9).

## 10. Open points

1. How open events, stage ends and unlocks are evaluated in full, and what breaks when events are added: the
   condition for every **M** list to grow, the save's capacities (visit table 0x180, win lists 35 / 4 / 10).
2. Event `+0x06`, `+0x14`, `+0x2C`; stage `u_0x06`, `u_0x26`, `u_0x2c`, `u_0x35`, `u_0x36`, `u_0x38`, `u_0x4c`;
   shop `+0x52..+0x73`, `u_0x78`; car unlock `u_0x04`; message `+0x06`.
3. The second career container (8 events, 2 stages): no reader found for its career tables; kept. Its performance,
   tuning-unlock and cost chunks are [cars.md](cars.md)'s.
4. `0x34202` (1,096 bytes, starting with a `0x11` fill, then `2`, a hash and `PARKLAND`): no reader found; kept.
5. Track record: the open fields of §3.2; which of a duplicated number's records the original's lookup (0x5D3E90)
   returns (Overbright keeps the last, `src/frontend/career/data.rs:348`; identical in known editions).
6. Trick `u_0x05`, `u_0x20`, `u_0x24`; showcase lights' `u_0x04`, `u_0x08`.
7. The string pool's 218 unread strings: kept as data (a list of `{ at, text }` beside the names, as the part
   database does, [cars.md](cars.md) §3.4), they would let one encoder lay out the pool and bring the pool, events,
   sponsors, messages and challenges to Eq2 (events then within RES.6 only if their unused slots are named too).

## 11. Sizes and timing

| | Original (reference install) | Base set |
|---|---|---|
| Career container, this page's chunks | 54,904 bytes (224 events, 6 stages, 20 sponsors, 35 brands, 30 milestones, 24 shops, 59 cash pickups, 24 car unlocks, 167 messages, 28 challenges, pool of 828 strings) | `career.toml` ≈ 250 KB [guess]; shadow 47,668 bytes; 231 bytes kept as patches |
| Second career container | 25,616 bytes | residual: 16,328 bytes of performance, tuning-unlock and cost chunks are [cars.md](cars.md)'s items; the rest kept |
| Track table | 38,776 bytes (131 records) | `tracks.toml` ≈ 80 KB, `track-maps.toml` ≈ 15 KB [guess] |
| Tricks | 1,880 bytes (39) | ≈ 8 KB in `career.toml` [guess] |
| Magazines | 12,712 bytes (37 + 42 cameras, 3 env maps, 16 + 14 setups) | `magazines.toml` ≈ 25 KB [guess] |
| `0x34202` | 1,096 bytes | residual, kept |

Import: well under 50 ms after the GlobalB unpack, which the global units share [guess].
