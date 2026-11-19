# Tracing: AI lanes and the road network for custom races

How the original's AI racers and traffic pick their routes and lanes, which files carry that, and what a custom
race on its own route would need. The formats and the AI's behaviour are in
[docs/formats/ai.md "AI route layer"](../formats/ai.md#ai-route-layer-srcformatsai_routesrs-srcairoute) (route
file, walk, junction choice, speeds, free-roam post-load, "Race files against free roam", "Paths files per
race") and [docs/formats/career-1.md](../formats/career-1.md) (the `Paths` file, markers); the editor's custom
races in [docs/map-editor.md](../map-editor.md).

Marks: **verified** = read in SPEED2.EXE or checked against every file named; **inferred** = consistent with the
data, not traced; **open** = not known.

## Two networks

| | `Paths<track>.bin` | `Routes<track><F\|B>.bin` |
|---|---|---|
| Read by | 0x5D9AA0 | 0x57F554, chunks at 0x5DEB90 |
| Holds | road graph (nodes, segments, connections), checkpoint lists, barriers `0x3414D`, triggers `0x3414A` | control points with lanes (0–1 racing line, 2–9 traffic), junctions, traffic intersections |
| Used for | the race road through the checkpoints (0x5E5BC0), the race coordinate, barriers that close segments | where racers and traffic drive, which way they turn, how fast |
| Free roam | `PathsFreeRoam.bin`: triggers and barriers only | `RoutesFreeRoam.bin`: the whole region's network (L4RA: 855 records) |
| Per race | its own small graph around the race (verified) | the free-roam network cut for the race (verified, below) |

## What is traced

- **Route choice** (verified; ai.md "Start / road node search", "Walk"): a racer starts on the race record
  nearest to it whose race-coordinate range holds its own race coordinate; its walk follows control points and,
  at a junction, takes a type 2 (race link) always, refuses types 5, 7, 10 and links flagged "not for racers",
  takes a type 4 under an odds draw and types 0 / 1 by the skill row's odds (0x413CC0, 0x413E33..0x413EC9). So
  the junction types in the loaded file decide the course; the race road from `Paths` decides only the race
  coordinate and the lap.
- **Lane choice** (verified; ai.md "Speeds", "Path", traffic sections): racers drive lane 0 forward / lane 1
  backward (the racing line) with a lateral offset that blends back to the line; traffic drives lanes 2–5 /
  6–9; the speeds are rebuilt at race load from the racing line's geometry (0x5E9E90 → 0x5DDFD0 →
  0x5E8790), so the 100 m/s written in the files is never used for racers.
- **No pruning in races** (verified; ai.md): the game loads a race's route file as is; the race branch of the
  post-load (0x5E41B0's race half, 0x5E23E0, 0x5E1E90, 0x5E2760) never runs because 0x88A330 is never set.
- **A race file is the free-roam network with race bytes** (verified, byte comparison of the 120 race files of
  ROUTESL4RA against `RoutesFreeRoam.bin`; ai.md "Race files against free roam"): same records, same geometry
  and traffic lanes; different race coordinate, point flags, junction slots, racing-line laterals and speeds;
  junctions with the same type nibbles but their run-time types already decided; plus 103 race-only records.
- **The racing line is per course** (verified, same files): lane 0 / 1 laterals equal the free-roam ones at
  only 35 % of the shared points (75,445 of 213,196), while the forward and backward files of one track share
  their records (59 of 60 pairs) and their lane 0 / 1 bytes at 94 % of the points. So each course has its own
  authored line, shared by its two directions.
- **The record exclusion list** +0x44 (verified, 0x5E41B0): four track numbers a record is left out of.
- **Each race's road graph is local** (verified; ai.md "Paths files per race"): L4RA's 60 Paths files cover
  0.6–4.2 km boxes of a region about 5 km across; none holds the whole region; free roam has no road graph.

## What is inferred

- The authoring tool made each race file by running the race branch of the post-load offline on the free-roam
  network and the race's road: the low junction bytes in the race files are exactly the run-time types that
  branch computes (7 dead, 2 link, 3 end), and the game never needs to compute them again.
- **Custom races today**: the editor's custom race loads its base track's `Routes` file (`AiRoutes::load` with
  the base track's number). Where the custom route leaves the base race's course, the racers' walk still takes
  the base race's type 2 links (taken always), so AI racers tend to follow the base course there; traffic is
  unaffected (its lanes are the free-roam ones). Not measured; the `--simulate` warnings (wrong way, stuck)
  are the check.
- A custom route can only use roads in its base track's `Paths` graph; a route across the city needs a
  region graph (the 60 L4RA graphs overlap: 3,284 of their 3,923 distinct node spots are in two or more
  files, so a merged graph is plausible but not checked for consistent connections).

## What is open

- The exact pruning the race branch performs (0x5E41B0's race half, 0x5E23E0, 0x5E1E90, 0x5E2760, the bit-9
  paths of 0x5E27E0): traced only up to the record test at 0x5E42EB. Tracing it fully gives the generator for
  a custom race's route file.
- How the racing-line laterals (lanes 0 / 1) were authored: from the geometry (an apex line), by hand, or
  recorded. No rule found; a generated line needs its own check against the shipped ones.
- What the race-only records (`TrackRoutesA33`, `TrackRoutesA34` and `TrackRoutesA40` to `TrackRoutesA44`) are on the map, and whether a custom
  race may use them.
- Drag-race lanes and their fixed lane changes (ai.md "Drag") against a custom drag strip: not looked at.
- Barrier groups for a custom race of its own (roads closed only for it): needs new `0x3414D` barrier records,
  a group in the track bundle's `0x3412F` table and walls in the collision packs (docs/tracing/collision.md).

## What a custom race needs

1. **Now (works):** the base track's Paths, Routes, barriers and event set-up; the route through the
   checkpoints on the base track's graph. Good when the custom route stays on the base race's course or on
   roads its racers already reach.
2. **Its own route file** (proposal): start from `RoutesFreeRoam.bin` (plus the base race file's race-only
   records), then
   - keep every record (traffic still needs the whole neighbourhood);
   - per junction, set the run-time type bytes: 2 where the custom route continues through it, 7 where it
     leaves the route for racers, 3 at the route's end against its direction — by porting the race branch once
     traced, else by the same rules as the free-roam post-load with every road off the route barred;
   - per control point on the route: race coordinate (+0x08) = the distance along the custom race road,
     start flag (bit 11) near the grid, the lane flags the walk tests, the record's direction (+0x70 bit 1) and
     coordinate range (+0x38/+0x3C);
   - racing-line laterals: the base race's where the custom route runs on the same records in the same
     direction, else free roam's lane 0 / 1 (inferred to be a usable line; to be checked);
   - lane speeds: leave 100; the race load rebuilds them.
3. **A region road graph** for routes across the whole city: merge the region's Paths graphs, or build one
   from `RoutesFreeRoam.bin` (open which).

## Proposed lane delta format (for review)

```text
[ai]
lateral   <record id> <point> <lane 0..9> <fraction /128>      # move one lane at one control point
junction  <own record> <own point> <target record> <target point> type <k0> [<k1>]
exclude   <record id> <track number>                           # +0x44
```

A delta applies to the free-roam file and to every race file holding the record (geometry and traffic lanes
are shared, verified above); a custom race's generated file is written whole, not as a delta. Without a map mod
nothing changes.

## Test plan

1. **Generator round trip** (no game): rebuild each shipped race file's race bytes (junction types, flags,
   coordinates) from `RoutesFreeRoam.bin` and its Paths race road; count the bytes that differ, per file.
   The generator is ready when all 120 L4RA files match except the authored laterals.
2. **Base behaviour** (clone): `--simulate` a custom race that leaves its base course at one junction, with the
   base file and with a generated file; the generated one must have no wrong-way or stuck warning there.
3. **Original** (Wine, private display, null audio): replace one race's `Routes` file with the generated one
   built from its own checkpoints; the race must play as with the shipped file (positions per lap within the
   run-to-run spread).
4. **Native unchanged**: with no map mod, every race file loaded is the shipped one (file hash in the
   simulation stream header).
