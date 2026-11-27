# Asset format: run files

A run file holds one run of the game's simulation: every car's look, pose, wheels and lights at each 60 Hz tick,
the race's events, the results and a state hash, and, when asked for, each car's gear, engine speed and pedals
(the drive channels) and the state hash of every tick. It is one file for simulations, recordings and replays: the
map editor's Simulate tab plays it (docs/modding/map-editor.md, "Simulation stream"), and Studio's runs and ghosts
read the same file.

## 1. Scope

Overbright's own data; nothing of the original maps here (the original's replays are not files). A run file is
written by the game while it runs (`game … --simulate --stream <file>`, docs/modding/map-editor.md; or, for a
test window the player drives, `game … --record <file>`, 3.4) and read while it grows. It is not part of a pack: packs do not ship runs, and no other kind refers to one. Studio keeps the runs of a mod
in its editor folder (`.editor/runs/`, never exported) and lists them in its Runs workspace
(docs/modding/studio.md, "Runs").

## 2. Files and keys

One file per run, any name; `.obrun` by convention (the example is
[`examples/runs/race.obrun`](examples/runs/race.obrun)). Short name `run` (`run:<file stem>`, in Studio's lists
only). Within a file a car is named by its id: the player's slot 0, opponents 1.., traffic 100.., street racers
200...

## 3. Source form

Text (conventions §3.1: UTF-8, no byte-order mark, no NUL, LF line ends), not TOML: one record per line, the
record's tag first, its fields after it separated by one space, in the order below. The header is the first line,
`END` the last (a run still being written has none yet). Numbers are decimals as Rust writes them (positions to
1 cm, rotations to 1e-4, times to 1 ms); a list is `,`-separated (`-` when empty); a tick's cars are
`|`-separated; a name or the end's reason is the rest of its line.

### 3.1 Records

Generated from the code (crates/ob-asset-format/src/kinds/run.rs, `RUN`):

<!-- generated:records run -->
| Record | Field | Type | Req. | Meaning |
|---|---|---|---|---|
| `OBSIM`: The header, the file's first line: its version, then `key=value` pairs. | `version` | u32 | yes | The major version (`VER.2` when unknown). |
|  | `schema=` | schema id | no | `ob.run/1.<minor>`; streams written before the kind had none (read as 1.0). |
|  | `mode=` | enum | no | A custom race, or free roam from a spot. |
|  | `seed=` | u64 | no | The run's random seed. |
|  | `race=` | race id | no | The custom race's id (`-` in free roam). |
|  | `track=` | u16 | no | The base track. |
|  | `laps=` | u32 | no | The race's laps. |
|  | `checkpoints=` | u32 | no | The race's checkpoints. |
|  | `hash=` | u32 | no | How often `H` is written, in ticks (60 when absent; 1: every tick). |
| `C`: A car's first appearance. | `id` | u32 | yes | The car. |
|  | `kind` | enum | yes | Its role. |
|  | `name` | text | no | Its name (the rest of the line). |
| `L`: A car's look: before its first tick and again when it changes (a traffic slot taking another car). | `id` | u32 | yes | The car. |
|  | `model` | name | yes | The model's folder (`CARS/<model>`). |
|  | `camber` | f32 rad | yes | The front wheels' x-turn. |
|  | `parts` | list (`,`; `-` none) | yes | The fitted visual parts: slot and part hash in hex, in slot order. |
| `F`: One tick (the game's 60 Hz update) and every car in it. | `tick` | u64 | yes | The tick, from 0. |
|  | `time` | f32 s | yes | The race clock (negative before the green), or since the start in free roam. |
|  | `cars` | list (`|`) | no | Each car (the rest of the line), its fields as below. |
| `E`: An event of the tick before it. | `tick` | u64 | yes | The tick. |
|  | `time` | f32 s | yes | Its time. |
|  | `kind` | word | yes | `start`, `checkpoint`, `lap`, `finish`, `crash`, `stuck`, `wrong-way`; a reader skips an event of a word it does not know. |
|  | `car` | u32 | yes | The car. |
|  | `checkpoint` | u32 | yes | The checkpoint passed, or for a problem the next one (1-based; 0 none). |
|  | `value` | f32 | yes | A lap's or the finish's time (s), the speed a crash lost (m/s), how long stuck (s). |
| `H`: The state hash, every 60 ticks and after the last tick: the same seed gives the same hashes, so a replay or a second run shows the tick where it parted. | `tick` | u64 | yes | The last tick it covers. |
|  | `hash` | hex u64 | yes | FNV-1a 64 over every tick so far: per car its id and the exact bits of its position, rotation, velocity and spin (runs.md 3.2). |
| `R`: A racer's result at the end. | `car` | u32 | yes | The racer. |
|  | `place` | u32 | yes | 1-based; 0 did not finish. |
|  | `time` | f32 s or `-` | yes | The race time. |
|  | `best` | f32 s or `-` | yes | The best lap. |
|  | `laps` | list (`,`; `-` none) | no | Each lap's time. |
| `END`: The end, the file's last line. | `why` | text | yes | `finish`, `quit`, `limit` or `error <why>` (the rest of the line). |
| a car of `F` | `id` | u32 | yes | The car: the player's slot 0, opponents 1.., traffic 100.., street racers 200.. |
|  | `kind` | enum | yes | Player's slot, racer, traffic, street racer. |
|  | `x` | f32 m | yes | Position (world metres, to 1 cm). |
|  | `y` | f32 m | yes |  |
|  | `z` | f32 m | yes |  |
|  | `qx` | f32 | yes | Rotation, a unit quaternion (to 1e-4). |
|  | `qy` | f32 | yes |  |
|  | `qz` | f32 | yes |  |
|  | `qw` | f32 | yes |  |
|  | `speed` | f32 m/s | yes | Speed. |
|  | `lap` | i32 | yes | The lap under way, from 0. |
|  | `checkpoint` | u32 | yes | The race's checkpoints passed in this lap. |
|  | `place` | u32 | yes | 1-based; 0 outside a race. |
|  | `steer0` | f32 rad | no | The front wheels' steer, left and right (the wheels and lights follow together). |
|  | `steer1` | f32 rad | no |  |
|  | `roll0` | f32 rad | no | Each wheel's roll about its axle, 0..2π, table order. |
|  | `roll1` | f32 rad | no |  |
|  | `roll2` | f32 rad | no |  |
|  | `roll3` | f32 rad | no |  |
|  | `lift0` | f32 m | no | Each wheel's lift: the hub's offset from rest. |
|  | `lift1` | f32 m | no |  |
|  | `lift2` | f32 m | no |  |
|  | `lift3` | f32 m | no |  |
|  | `lights` | u8 | no | Bit 1 headlights, bit 2 brake lights. |
|  | `gear` | i32 | no | The drive channels: -1 reverse, 0 neutral, 1.. the gear. |
|  | `rpm` | f32 | no | The shown engine speed. |
|  | `throttle` | f32 | no | The pedals as the car took them, 0..1. |
|  | `brake` | f32 | no |  |
|  | `handbrake` | f32 | no |  |
|  | `steerin` | f32 | no | The steering input, -1 (left) .. 1 (right). |
|  | `nitrous` | u8 | no | 1 while the nitrous key is held. |
<!-- /generated -->

`schemas/run.schema.json` describes one line read as an object of its named fields, with `record` its tag (the
car fields of `F` as objects in `cars`). The check is `ob_asset_format::kinds::run::check_run`.

### 3.2 The state hash

`H` carries FNV-1a 64 (offset basis `cbf29ce484222325`, prime `100000001b3`) over the bytes of every tick so
far: for each car of the tick, in the tick's order, its id (u32) and the exact bits of its position (3 × f32),
rotation (x, y, z, w), velocity (3 × f32) and spin (3 × f32), all little-endian, then the tick (u64). It is
written every 60 ticks and after the last tick, before the results. The written poses are rounded; the hash is
not, so two runs whose files look the same but whose cars differ in the last bit still differ in their hashes.
The same seed gives the same file byte for byte, hashes included; a replay or a second run that parts from the
first shows it at the first `H` that differs.

### 3.3 Channels and recording

Besides the poses, a run file has two opt-in channels, asked for with `--run-channels drive,hash` (a
simulation's or a recording's; neither flag, and the file is as it was, `ob.run/1.0`, byte for byte):

- `drive`: each car's gear (-1 reverse, 0 neutral, 1..), shown engine speed (rpm), throttle, brake, handbrake,
  steering input and nitrous key, as trailing fields of the car in `F` (after the lights). They are what the car
  took in its last update, so the player's own inputs in a recording and the AI's in a simulation.
- `hash`: the state hash every tick instead of every 60 (the header says `hash=1`), so a second run's first
  difference is the tick it parted at.

A file with either channel says `schema=ob.run/1.1`; a reader of 1.0 skips the trailing fields and reads the rest
(§9). The channels are read from the car after each update and never written back: the run is the same with them
on or off (the same seed gives the same poses and the same `H` lines at the ticks both write).

`--record <file>` makes the window's test run (`--map-file … --map-race <id>` or `--map-at …`, docs/modding/map-editor.md)
write the same file as the player drives: one `F` per update, whatever the frame rate, in a race and in free roam;
a pause writes nothing; the file ends `finish` when the race does, `quit` when the window is closed and `restart`
when Studio's link starts the run over. The header's `seed=` is the game's own boot seed, which the run
does not depend on (the player's inputs decide it). Studio's Drive to record starts this and keeps the file as a run
(docs/modding/studio.md, "Runs").

### 3.4 Example

[`examples/runs/race.obrun`](examples/runs/race.obrun): a two-car race's first two ticks;
[`examples/runs/drive.obrun`](examples/runs/drive.obrun): a recorded drive's first ticks with the drive channels
and the hash of every tick (`ob.run/1.1`).

## 4. Layering

None: run files are not pack content.

## 5. Import

None: nothing of the original maps here.

## 6. Equality and verification

The writer and the reader are the map data's (`ob_map::tools::map::sim::format`); its tests write every record
and check it against `RUN`, and read the example and write it back byte for byte. The game's test
`a_simulated_race_streams_every_car_and_repeats_for_its_seed` runs a race twice with one seed and compares the
two files byte for byte. With no `--stream`, nothing is written and nothing is hashed: the game's own run is
unchanged.

## 7. Cooked form

None: read as written.

## 8. Errors

A run file is refused with the conventions' rules: `CONV.3` when the first line is not the header or its
`schema=` is malformed, `CONV.5` for a missing field, `CONV.6` for a value of the wrong type or range (and a second
header, or a record after `END`), `CONV.8` for a malformed name, part or hash, `CONV.12` for a list over its
limit. `VER.2` refuses an unknown major (the header's version, or `schema=ob.run/<major>.x`). The refusal names
the line and the field.

## 9. Versioning and extension

The header says `schema=ob.run/1.<minor>`; a stream from before the kind has no `schema=` and reads as 1.0. Unlike
a TOML kind (`VER.1`), a newer minor is read: a reader skips a record of a tag it does not know, the words after
the fields it knows (a car's too), an event word it does not know and an unknown `key=value` of the header. So a
minor version adds records, trailing fields and event words; a change in what a known field means is a new major.

## 10. Open points

- The spinners' own turn is not in a run yet.
- A recording does not carry the player's raw device (the key or the wheel): the pedals and steering are as the car
  took them.
- More events (a car's crash with another, the nitrous, the police in free roam) come as a minor version.

## 11. Sizes and timing

A custom race of four racers with traffic, run to its finish, is about 3.3 MB (one `F` line per tick, most of
the file).
