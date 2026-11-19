# Tracing: static collision for moved, added and deleted scenery

What the original does with the city's static collision, and what a moved, added or deleted prop would need
for the game to hit it where it now stands. The format itself is in
[docs/formats/cars-1.md "Collision"](../../formats/cars-1.md#collision-verified-except-where-noted) and its
"Pack layout rules" section; knock-over props in the same chapter under "Knock-over props".

Marks: **verified** = read in SPEED2.EXE or checked against every shipped file; **inferred** = consistent with
the data, not traced; **open** = not known.

## What is traced

- **The game builds nothing at run time** (verified). 0x5D8150 only records pointers and counts for each chunk
  of a `0x80034130` pack: faces `0x34134` at pack +0x08 / count +0x0C (size / 32), `0x34135` at +0x10 / +0x14
  (size / 2), references `0x34136` at +0x18 / +0x1C (size / 4), cells `0x34137` at +0x20 / +0x24 (size / 4),
  and copies the `0x34131` header (16-byte aligned) to +0x30..+0x5B: section +0x30, two bytes +0x32 / +0x33,
  width +0x34, height +0x36, origin +0x38 / +0x3C, bounds minimum +0x40..+0x48 and maximum +0x50..+0x58. It
  writes 1 into the header's byte +2 in the file image (a loaded mark). Every list a query walks comes from the
  file as stored.
- **The walk** (verified, 0x5E0D40, the gather for car contacts): the query box's cells are
  `trunc((p − origin) / 32)`, clamped to the grid (0x5E0F0E..0x5E0F3C, the constants 1/32 at 0x7A5188 and 32 at
  0x7875B4); in each cell a 16-bit mask of the box's 8 m sub-cells, bit `4·row + column` (0x5E0E36..0x5E0F06,
  1/8 at 0x788164), is ANDed with each reference's mask (0x5E0F6F); a face's stored x/y bounds are the corners
  byte 1 names, `x << 13` times 1/65536 (0x784278), so the corner itself in metres. The barrier group byte is
  tested against the group table's +8 (pointer at 0x88FF94, 0x5E1037..0x5E104E).
- **Runtime marks in the face record** (verified): the gather sets flag 0x20 on each face it takes so a face
  listed in several cells is taken once (0x5E1078..0x5E107B), and clears it afterwards (0x5E1104). It takes at
  most 160 faces per gather (`cmp eax, 0xA0`, 0x5E1054); faces beyond that are not seen. The i16 at +6 is the
  game's cache slot. On disk both are clear: 0x20 is set on no shipped face and +6 reads 0xFFFF on all of them.
- **No link from a face to a placement** (verified): a face holds a barrier group, a corner selector, a
  surface, flags, a base height and its corners; nothing names a solid, an instance or a section of scenery.
  Knock-over props are the only scenery the game collides with by placement (their `0x34027` records and the
  kind's `ZCS_` hull; cars-1.md "Knock-over props").
- **Every derived byte of a pack is reproducible from its faces** (verified on all 599 packs of the eight
  shipped stream files): the cell grid, every cell's reference list in order, every sub-cell mask,
  the header's origin, size and bounds, and each face's base height and corner selector. The rules are in
  cars-1.md "Pack layout rules". So a writer that holds the face list can rebuild a pack the original reads
  exactly as it reads its own.

## What is inferred

- **Which pack holds a face.** A face appears in every pack whose drivable section it touches: on a sample of
  25 L4RA packs, every face of the stream file that overlaps a pack's section outline is in that pack (no
  exception), and each pack also holds faces up to 10–18 m outside its outline. About a third of L4RA's faces
  are stored in two to five packs. The outside margin is not one fixed distance (878 faces of other packs lie
  closer to a sampled outline than that pack's farthest own face), so the authoring tool's rule is open. The
  margin fits the height query's section choice, which keeps a pack until the car is 9 m plus its depth into
  the section away (0x5D7280), so a car can query a pack while outside its section.
- **The pack's section number** is the drivable section's number plus the region's detail count (L4RA: pack
  121 for outline 101, n = 20, as 0x5D3C20 maps numbers n..2n back by n); the 25 sampled packs all match their
  outline that way.
- **The faces are authored, not the render mesh.** Collision walls are tall quads (flag 0x04 reaches
  ±2048 m) and floors are road-level triangles; a building's render mesh is not reused. Not measured per
  building.

## What is open

- How to pick a building's own faces out of a pack: faces carry no owner, so they must be matched
  geometrically to the solid's transformed footprint. How many faces of neighbours (pavement, a wall shared
  with the next building) such a match catches has not been measured.
- The exact outside margin the authoring tool used for a face's packs (above).
- The surface byte a face of an added prop should carry beyond "the surface of the faces it replaces" (the
  surface ids' meanings are in src/surface_maps.rs; not checked per prop kind).
- Whether `0x34135` is read anywhere: it is empty or missing in every shipped pack and no reader of pack
  +0x10 / +0x14 was found.
- The byte the header holds at +3 (0 in every shipped pack).

## What a moved, added or deleted prop needs

| Prop | Collision today | Needs |
|---|---|---|
| Knock-over prop (cone, sign, barrel, …) | its own hull, placed by its `0x34027` record | nothing more: moved, added and deleted ones already collide at their new place |
| Building, wall, fence (baked faces) moved | stays at the old place | remove its faces in the old place, add faces at the new place, in every pack whose section is within reach of either place |
| Same, deleted | stays | remove its faces |
| Same, added (a copy) | none | add faces |

Removing and adding faces is safe for the game's readers as long as each touched pack is rebuilt by the
layout rules: the lists are plain indices, nothing else in the game points into a pack's faces, and the
runtime fields start clear.

Limits a writer must respect (verified unless marked):

- at most 65,535 faces and 65,535 references per pack (u16 indices and starts); the largest shipped pack has
  15,696 faces and 20,870 references (L4RG);
- at most 160 faces per gather: an added prop should add few faces (a box or a coarse hull), not its render
  triangles;
- x and y in i16 eighths of a metre (±4096 m), z within ±128 m of the face's base height;
- at most 255 barrier groups per track (u8); groups live in the track bundle's `0x3412F` table (L4RA: 125);
- floors wind counter-clockwise for the outline test (0x5D3340); a triangle repeats corner 2 as corner 3.

## Proposed delta format (for review)

A map mod's collision change, per stream file, as plain records the editor writes and the game applies before
it registers the packs:

```text
[collision]
remove  <pack section> box <x0> <y0> <z0> <x1> <y1> <z1> [walls|floors|all]
add     <flags> <surface> <group> <x y z> × 3 or 4
```

- `remove` drops the faces of that pack whose corners all lie inside the box (a moved or deleted building's
  old footprint, grown by a small margin); `walls` keeps the floors, so the road under a deleted kiosk stays.
- `add` faces are written into every pack whose outline lies within 20 m of the face (inferred margin above),
  with the surface of the removed faces unless given.
- Then each touched pack is rebuilt by the layout rules, and the result is checked: every pack still parses,
  reference and face counts within the limits, no face added more than once per pack.
- A pack set with any `add` or `remove` marks the mod as changing play; its check reports collision as
  "approximate" until the open points above are closed.

## Test plan

1. **Round trip** (no game): rebuild every shipped pack from its faces with the layout rules; all 599 must be
   byte-equal (done in this round with a standalone script; to become a unit test once the writer exists).
2. **Delete**: remove one building's walls in L4RA; drive the player's car through its old place in the
   original under Wine with the patched stream file and in the clone; neither may stop the car.
3. **Move**: move one detached building (a kiosk) 30 m along an open plaza; drive into the new place at
   20 and 60 km/h: both must stop the car at the new walls, and the old place must be free.
4. **Add**: copy a small building into an open lot; drive into it from four sides.
5. **Gather cap**: drive along the added walls with several cars around; the gather must never reach 160
   faces where the original never did (count logged in the clone).
6. **Sections**: repeat 3 on a building within 20 m of a section border, approaching from both sections.
