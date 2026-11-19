# Career, race types and rewards (1)

Part of [docs/formats.md](../formats.md), the traced behaviour of `SPEED2.EXE`; the tags
(**verified**, **inferred**, **open**) are explained there.

## Career (verified except where noted)

* **Career chunk** — GlobalB 0x80034A10 (the game loads only the first,
  0x512C00): 0x34A11 events (0x88), 0x34A12 shops (0xA0), 0x34A14 sponsors
  (0x44), 0x34A16 magazine/DVD rewards (0x40), 0x34A19 (0x10), 0x34A1B
  (0x30), 0x34A1C (0x28), 0x34A1E (0x0C), 0x34A1F car unlocks (0x0C),
  0x34A1D the string pool; 0x34A15, 0x34A17, 0x34A18 and 0x34A1A go to
  their own loaders.
* **Event** — +0x00 name (pool offset, e.g. "S2_CIRCUIT_3"; some are
  empty), +0x08 the event's hash (car unlocks name it), +0x0F type (0 circuit, 1 sprint, 2 street X,
  3 special, 4 drag, 5 drift), +0x18 track number, +0x1A variant (0/1/2; bit
  0 reversed: 0x532EE3 copies the +0x7E races into race records +0 track,
  +4 variant & 1, +8 laps; the reverse flag 0x89E7A4 picks Routes<n>B and
  BackwardStartLine), +0x1B laps, +0x28 marker
  (`bin_hash("EV_nn")`), +0x30 purse (the cash a win pays), +0x34 category (3 plain, 1
  sponsor, 2 URL, 0 scripted), +0x37 stage (1..5). 224 events; stage 1
  has five raceable ones.
* **Car unlocks** (0x34A1F) — {u32 car type hash, u32 trigger, u32
  trigger}; a trigger matches an event's +0x08. Cars without a record are
  available from the start (Peugeot 206, Focus, Corolla, 240SX, Civic,
  Peugeot 106, Corsa). Display names: `bin_hash(CAR)` gives the make,
  `CAR_NAME_<CAR>` the model.
* **Tracks** (GlobalB 0x34201, 0x128-byte records; +0xEC traffic counts and +0x108 traffic odds, see "Traffic") — name, path
  ("Location4\Track_4014"), region ("L4RA"), +0x8A track number, +0x98
  route length (m).
* **Routes** (TRACKS/ROUTES<region>/Paths<track>.bin, 0x80034147, read at
  0x5D9AA0) — 0x34148 path nodes (24 bytes: x, y, u16 segment, …, f32
  distance to the finish), 0x34149 (0xDC), 0x3414A (variable), 0x3414C the
  region's checkpoint lists (0x110: u16 track, u8 count, u8 loop, u8 —,
  u8 width, u16 —, 33 × (x, y); a loop ends on its first point), 0x3414D
  (0x24).
* **Markers** (TrackPosMarkersAll.bin / TrackPosMarkersFreeRoam.bin,
  0x34146 after 16 bytes) — 0x30-byte records: u32 11, u32 11, u32 name
  hash, u32 0, f32 x, y, z, u32 0, u16 heading (the game's 16-bit angle),
  u16 —, u16 track (+0x0C is a starting position's grid slot 0–7, two rows
  of four; 0x5EB5E0 looks markers up by it, so the clone orders the grid by
  it — verified on 4001/4013). Names: Forward/BackwardStartingPosition (× 8, the
  grid), …StartLine, …FinishLine; free roam also places the shops (their
  hashes match the shop records) and the explore start grid.
  The marker's height is not used as is: 0x5EB5E0 puts each grid slot on
  the ground found by 0x5E72F0 at its x/y — the highest floor at or below
  the marker's z + 1 m, or, when there is none, the highest floor there at
  any height (a second pass with no limit). Some tracks' markers lie under
  the map (4702's at −20 m, its road at ~40 m); spawning at the marker's
  height put the cars inside the world (**verified**; the clone's
  `World::grid_height`).
* **Stages** (0x34A18, 0x50 each; current stage 0x862838, runtime records
  0x862018 + n × 0x104 with +0xFC unlocked, +0x100 the definition):
  +0x01 sponsor count (0 for stages 0 and 1, else 5), +0x08 sponsor brand
  hashes (their 0x34A19 records), +0x28 the event that ends the stage
  (DDAY_EVENT_B, none, S2_URL_3, S3_URL_8, S4_URL_7, and the last stage-5
  race), +0x40 wins required (3, 5, 10, 15, 20, 20). **Verified**: +0x40
  also caps the stage's outrun wins (runtime record +0xDC, 0x513D30; see
  Outrun), and +0x02 (i16) is the outrun's bank, paid for a win and taken
  for a loss (0x541330, 0x500C60): 100, 100, 100, 100, 250, 400. That +0x40
  is also the stage's required wins is the earlier note, not re-checked
  here. Winning the final
  event posts the stage-complete message (0x5242D0, message 7). The World
  Map lists a stage's events with stage-0 events in stage 1 and without the
  D-Day pair (0x4B1360). The stage end (0x532C70, from the win handler
  on the **won event's** stage record, +0x37, not the current one):
  unless the record's +0xFC is set, a stage with a final event completes
  when it is won (visit state 1, 0x500B80) and its media milestones are
  done (0x513490); a stage without one (stage 1) when **every** event
  whose +0x37 is the stage is won (0x5130A0 over the 0x34A11 table) —
  stage 1's five, not any five wins (the three stage-0 races,
  STAGE_0_CIRCUIT_0/1 and STAGE_0_SPRINT_0, do not count). +0xFC is then
  set and 0x528330 advances 0x862838 only when the stage is the current
  one. Verified (career audit 2026-10-31); the clone used to count five
  wins over stages 1 and 0 together.
* **Open events** (0x527D40, rebuilt after each win, sponsor signing and
  milestone): the career keeps up to 64 open events (career +0x71C8).
  None once the stage's final event is won. Open events not yet won stay;
  then each slot fills to its cap with a pick among the stage's other
  unwon, non-URL, non-always events, shuffled with the game's random
  (0x43C1C0: state % n, then state ^= 0x1D872B41, b = a >> 5 ^ a,
  state = b << 27 ^ b ^ a; seed 0x12345678). Slots (0x513600): 0..4
  normal (category 3, not SUV) circuit, drag, drift, sprint, street X;
  5 SUV events (event +0x0D = 1, only while driving an SUV, car type
  +0x88A); 6..10 sponsor races of those types. Caps (0x527AB0): stage def
  +0x30, +0x31, +0x33, +0x34, +0x32 and +0x37 (1 each in stage 1, 2 in
  stage 2, …); a sponsor slot's cap is how many of the signed sponsor's
  four races (+0x04, codes 1 circuit, 3 drag, 2 drift, 4 sprint, 5 street
  X) are of that type, less those won (0x5137C0). Then the always-open
  events (+0x7F, the H_ races) while fewer than stage def +0x41 are open
  (−1 no limit), and the URL races whose requirement holds (0x513510):
  event +0x0C = 3 needs (+0x10 >> 16) normal wins; +0x0C = 4 (the final)
  needs the stage's media milestones, byte +0x11 normal, +0x12 sponsor
  and +0x13 URL wins (S2_URL_3: 10, 3, 2). Wins are filed per stage by
  the win handler (0x541430 → 0x513390): category 3 and not SUV at stage
  record +0x00 (35), sponsor at +0x8C (4), URL at +0x9C (10). The stage
  ends (0x532C70) when its final event is won and its media milestones
  are reached. Verified by the maintainer's probe `career_walk` and the test
  `open_events_follow_the_stage_rules`.
* **Milestones** (0x34A16, 0x40 each): +0x00 name (STAGE_2_DVD_1,
  STAGE_2_SHOW_1_PICTURE), +0x20 hash, +0x24 kind (1/3 magazine, 2/4 DVD),
  +0x25 stage, +0x28 the cover's name, +0x2C the photo-shoot marker,
  +0x34 how it is offered, +0x35 needed by the stage's final URL race,
  +0x3C the visual rating it asks for. States at 0x862840 (8 bytes: hash,
  +4 offered = 2, +6 done = 1). A stage's DVDs are offered when the car's
  visual rating reaches +0x3C (0x5331D0); magazines when the world
  challenge of 0x34A1B naming them starts (0x533BF0; the same record's
  message arrives by 0x533B50; see **World challenges**). An offered milestone places a marker in
  the world (0x533320); its photo shoot (0x4CEDC0) marks it done
  (0x533690). The visual rating (0x528420 → 0x5234D0) sums the points of
  the car's parts over 0xAA slots (0x5142E0 → 0x501860, table 0x34A15)
  and steps one per 500 points up to 10 (table 0x7FA230). A stock car
  rates 0, so the stage 2 DVD (rating 1) needs visual parts from the body
  shop.
* **Shops** (0x34A12, 0xA0 each): +0x00 name, +0x20 scene, +0x38 hash
  (the free-roam marker of that name places it), +0x40 scene file, +0x50
  kind (0 garage, 1 graphics, 2 body, 3 performance, 4 car lot, 5 car
  specialties, 6 unique parts), +0x51 announced, +0x74 / +0x9C what opens
  it (0x512870: 1 an event won, 2/3/4 URL, normal, sponsor wins of the
  shop's own stage record, 0x862018 + 0x104 × +0x9D, **verified**
  0x512899…0x512900), +0x9D its stage (0x513110). The minimap icon is the kind's
  (table 0x7F4348: events 0–6, shops 7–11, garage 12, showcase 13). The
  engage dialog (UI_EngageShopDialog, 0x4998E0) titles the kind
  (0x4FFEA0), says `<NAME>_BODY` (0x512920; the garage 0x987EECDA) and
  offers "Enter Shop" / "Enter Garage". The City Core shops need
  DDAY_EVENT_B (see D-Day intro).
* **Body shop** (UI_GenericParts_Browser, screen 0x566440, constructor
  0x55E830; categories 0x54FAF0 → 0x545B60): front and rear bumpers,
  skirts, spoilers, hoods, roof scoops, head- and taillights, mirrors,
  exhaust tips (slots 1, 2, 0x0B, 0x0C, 9, 7, 0x0E, 0x0F, 0x20, 0x10), rims
  (0x1D), carbon fibre (0x5E), widebody kits (6); icons VISUAL_PART_*. A
  category without a part beyond the stock one is locked (0x545710, item
  +0x22). A slot lists (0x5449D0) the car's parts of the slot's type,
  carbon ones apart (attribute 0x721AFF7C), not the type-5 hoods, that
  pass 0x515500: stock, owned (0x503DB0), or, in a shop, unlocked by the
  cost table (0x515360: level 4 never; 1 the shop named by the value open,
  2/3/4 the normal / URL / sponsor wins, 0x5004D0 / 0x500520 / 0x5005A0,
  of stage record value >> 16 — 0x862018 + 0x104 × (value >> 16), not the
  current stage, **verified** 0x5153D0…0x51544D, jump table 0x515450:
  types 0, 5, 6 open — ≥ the low half). Under Wine (2026-10-31) a
  finished career at stage 6 (no wins in the current stage) showed every
  Engine package and every body category open but Wide Body Kits (no kit
  for its Lancer); the clone had locked 63 levels there by counting the
  current stage (fixed; test `finished_career_unlocks_by_the_parts_own_stage`). In the
  garage (mode 2) only stock and owned parts list. A part's price is its
  category's at its level (0x514280). **Verified** layout: the screen has
  two IconScrollerMenus (0x5435E0). The categories run across
  CATEGORY_MASTER as CATEGORY_ICON clones (the package's templates in
  order, 0x52EC30), stacked by template width plus a 40 gap (0x52ED90),
  the strip 480 wide; the parts run down MINI_MASTER as PARTS_ICON clones
  with texture 0x73C4EC2D, gap 20, the column 200 tall (constructor
  +0x104 / +0x110). Placement (0x52EEB0): the selected item on its master
  (scroll = −its offset, 0x5386B0), the others at 0.8 size keeping their
  leading edge and centred across, hidden outside the extent and faded
  near its ends (+0x2F, 0x50D8D0). Category mode (0x55EF90; help
  0x6109E4B5): LEFT / RIGHT, LOCK_MASTER SHOW on a locked category
  (0x51DF10), 0xDF422345 shown, Install hidden. Select on an open
  category (0x55EDE0; help 0x43A05263): the strip hides (0x52EBA0(0)),
  the parts fill (0x53A4C0) starting on the installed one, UP / DOWN,
  Install shown; the part's name (0xD28B9316, 0x50E530) and price
  (0x0380B632) show (0x545520). Title: "Body Shop" (0x796DC449), in the
  garage "Body Parts Storage" (0x25F0C994); the option title is the
  category's label. The shop opens on the remembered category, slot 1 the
  first time (0x545B60). Rims and carbon fibre are items that open
  more (+0x23). The screen flow (next 0x520930, back 0x520A90):
  UI_ChooseCustomizeCategory → UI_GenericParts_Browser (back to it, or
  the career garage from there); rims go UI_ChooseRimBrand →
  UI_Rims_Browser; carbon fibre stays in the browser with its own
  categories ("Carbon Fiber", 0x24911E04); Back returns to the body
  categories on it.
  * Rims (0x53A630): locked unless at least two rims are unlocked
    (0x539C30). UI_ChooseRimBrand (screen 0x554A70, an OPTION_MASTER icon
    menu whose fill is 0x546530, each brand added by 0x5463C0): 15 brands
    for cars, 9 for SUVs (car type +0x88A), each list ending with Stock
    (0x3B83203); see `RIM_BRANDS` / `SUV_RIM_BRANDS` (brand, icon,
    label). A brand's rims (0x539B80): part type 0x1C (the brand rims
    are the shared group 51) with attribute 0xEBB03E66 the brand and
    0xCE7D8DB5 the car's rim size (car type +0xDC: 23–26 for cars, 32 /
    34 for SUVs), unlocked; Stock takes the level-0 rims and is never
    locked, another brand without rims is (LOCK_MASTER shows, 0x51DF10).
    UI_Rims_Browser (0x56B360 → 0x5678F0, 0x55F350): the brand's rims
    grouped by database name into styles on OPTION_MASTER (icon
    0x0294D2A3, label the name's hash, 0x43DB50), each style's rims
    (the 16/17/18-inch sizes, attribute 0xEB0101E2) down MINI_MASTER as
    in the parts browser; it opens on the fitted rim's style.
  * The strip's locks (+0x22) are taken when it is built: 0x545B60 runs
    only from 0x54FAF0, which runs from the constructor (0x55E830 at
    0x55EA8A) and on leaving carbon fibre (0x567830); back from a category
    or after a purchase the items stay as they were (**verified**, all
    callers of 0x545B60 / 0x54FAF0; under Wine, the original's career at stage 2, Carbon
    Fiber stayed locked after a spoiler bought in the same visit). The
    clone latches them (`ShopState::strip_locks`, `Game::new_parts_browser`;
    docs/native-fixes/NP-FIX-1.tsv).
  * Owned bits of shared-group parts (0x503B10): a part whose group is
    no car's takes car index 0x2E, and 0x6101F0(slot, 0x2E) hashes the
    "name" of car-type record 0x2E — past the 46 records, which 0x636BF0
    leaves in place in GLOBALB.BUN's chunk 0x34600, so the bytes of the
    next chunk's header (01 46 03 00); no override names it, so the walk
    is the slot's default groups and a part outside them counts all of
    them: every spoiler of the 206's shared group 0xE8E9C8A4 is bit 80 of
    +0x27C (the original's save after buying JORGEN: only 0x286 bit 0
    set). So in the original one bought spoiler of that group marks all
    of them owned, the carbon ones too, and Carbon Fiber opens at the
    next build of the strip (**observed** under Wine: on that
    save the garage's Body Parts Storage lists JORGEN, ROCKER 5,
    AIRFORCE, ARROW, BARRACUDA, WILDFIRE, RIPPER, …, Carbon Fiber is open
    and its Spoilers list the carbon JORGEN; the list runs on to the
    unique level-4 spoilers SHOCKWAVE, PHOENIX, LYNX and EDOLIX, owned
    and installable (preview 0.99 → 1.69); `audit/native-play/npfix2/`).
    0x503D50 sets `1 << (n & 31)` (the shift wraps), so any parts whose n
    meet in one bit are owned together. The clone's purchases now go
    through `Profile::own_part`, which owns every part of the car whose
    saved bit (`owned_bits`) is the bought part's, as a save and load
    would (test `part_group_owned_bits::a_bought_spoiler_owns_its_whole_bit`: the bits
    saved after the stage-2 visit's purchases equal the original's
    `stage2_body.sav`). Shared bits by slot (`part_group_owned_bits::
    list_shared_owned_bits`, 46 car types): spoilers 0x0C (one bit for
    80 parts on the 206, 106, Focus, Corsa, A3, Golf, Escalade, H2,
    Navigator; 2 elsewhere, 31 cars), exhaust tips 0x10, 0x1B, 0x1C and
    side mirrors 0x20 (pairs; mirrors up to 40, 31 cars), rims 0x1D /
    0x1E (stock + a level-3 rim, all cars), vinyl layers 0x40–0x43 (n and
    n + 32, all cars), 0x4A and the vinyl colours 0x4D–0x5C (pairs),
    0x48 / 0x4B / 0x4C (8–9) and the decal slots 0x61–0x98 (5–6).
    **Open**: whether the original sets a bit when a stock (level 0) part
    is fitted (pairs with a stock part in 0x10, 0x1B, 0x1C, 0x1D, 0x1E,
    0x20 would then own the other part); the clone never buys a stock part.
  * Carbon fibre (0x545890; locked by 0x544F10 when none of hoods 9,
    spoilers 0x0C, mirrors 0x20, roof scoops 7 has a carbon part and
    neither the trunk nor the kits is unlocked): categories (0x545D40)
    Carbon Fiber Hoods / Spoilers / Roof Scoops / Side Mirrors, each the
    slot's carbon parts (0x5449D0 with 666, 0x544E70; locked when none),
    then two toggles: the trunk (slot 0x60, cost category 0x56) and the
    kits (0x5D, 0x55). Each slot holds "CARBON FIBRE NONE" (0xC8D313AC,
    level 0) and "CARBON FIBRE" (level 3); while the none part is on the
    toggle reads "Convert Trunk To Carbon Fiber" (0xF8111A17) / "Carbon
    Fiber Body Kits" (0xB229FA7E, icon 0x246DE0CB), else "Revert Trunk To
    Stock" (0x1036E38E, icon 0x8F5DC19F) / "Revert Kits To Stock"
    (0xE170F115, icon 0x2BDB0653); locked until the cost category's level
    3 unlocks (0x52A1B0). The toggles are plain option objects (vtable
    0x79BE3C: only a destructor); the accept is the parts browser's
    (0x5664A0, carbon mode +0x1D5, slot 0x60 / 0x5D). It first asks "Do
    you want to convert …?" with No focused (0x566686..0x566728 trunk,
    0x566895..0x5669E3 kits; No 0xB4623F67 → 0x5674D8; ported,
    docs/car-materials.md); Yes (0x51123592) goes on at 0x566F12: the
    slot's other part (the non-none one while none is on, else the none
    one), in a career bought first unless owned (0x521BF0 → 0x503DB0):
    0x528A00 takes its 0x514280 price (level 3: the category's level-3
    price; the none part, level 0: nothing) off the bank without a check,
    floored at 0, marks it owned (0x503D50) and adds the price to the
    car's money spent (+0x7F0); then it is fitted (0x55C240) and the
    categories rebuilt (0x545D40). In the clone (the money spent is not
    kept).
  (**verified** by decompile of 0x55C180 and 0x538770) The car preview with the highlighted part (ported in menus103,
  `Game::preview_ride`, see "Shop sub-screens, round 2")
  (0x55C180(slot, part): for slots 0–0xA9 a copy of the car (0x413440)
  takes the part (0x631D30; slot 0x1D also sets 0x1E, the rear rims;
  slot 0x22 also 0x547480), its model is rebuilt (0x4D8EC0(+0x50, 1, 0))
  and 0x55B1B0(0x8392C0, 0x8389D0) refreshes the stats); not in the
  clone, the selected item's pulse (0x538770: while the item's x is within ±3 of the strip's
  centre (0x51D630), a phase +0x3C grows 0.03 a frame and wraps past 2;
  the triangle wave w = phase (≤ 1) or 2 − phase gives f = min(w × the
  item's byte +0x1F × 1.1 + 127.5, 255) and, when the item's +0x21 is
  set, its size = (f × 0.000117647054 + 0.97) × its base size +0x24 /
  +0x28 (0x50CB00), so 0.985–1.0 of it).
* **Graphics shop** (all in the clone; `src/frontend/graphics/`):
  UI_ChoosePaintCategory (0x55A410 → 0x5571C0, options 0x548210): Paint
  (0x6A4C46A6, icon 0x550681A4; option +0x48/+0x4C = 2/0), Vinyls
  (0x78DEEA1C, 0x55778E5A; 0/1), Decals (0x4E933E83, 0xDA1DAE54; 1/2;
  locked in a career unless 0x529F10), then "Purchase Setup" (career,
  0x920DBB02) / "Install Setup" (0xD132A6FE), icon 0x911A4EDE. The option
  stores +0x48 / +0x4C in 0x83899C / 0x8389A0 (0x522060); the next screen
  is 0x520930 → 0x50E380 (0 UI_ChooseVinylLayer, 1 UI_DecalMain, 2
  UI_Paint), back 0x520A90. **Verified**:
  * The setup (cart): list 0x8389C8 of {part, slot}, 0 a removal. The
    graphics screens put their choices in it (paint 0x53AF30 per slot,
    vinyl 0x54FDC0: a unique one, or one over a unique one in the setup,
    clears all layers (0x547550), else the layer's items and its colours
    (0x53B0F0, colour slots 0x50EDE0); vinyl colour 0x5474E0 (slots
    0x4D–0x5C); removal 0x547800 (with a unique vinyl fitted all four
    layers). Total Cost (group 0x9FF84A88, "00000" 0x2F5E7DA3 = `%$d` of
    0x521540: 0x514280's price of each item with a part outside slots
    0x34–0x3E) shows in a career (0x5453C0), hides otherwise (0x545470).
    Purchase (0x547580): each item fits its part or takes the slot's off
    (a vinyl over a fitted unique one clears all layers first); in a
    career it costs 0x514280's price (bank not below 0), no ownership
    discount, and outside the paint slots 0x3F, 0x44–0x47, 0x49 marks the
    part bought; then the list empties. Purchase Setup (0x562BA0): empty
    → "Please make a selection to install before checking out!"
    (0x51E3F647), else 0xA0C16CAB / 0x07F24787, Yes buys. Back (0x562C60):
    career 0xD76E0B5A first, then with a setup 0xA6C0D334 (non-career
    0x511CCA10): Yes buys and leaves, No drops it. BUTTON1 (0xC519BFC0,
    not on the setup option): cancel changes (0x6EE64972 / 0x449A9DD9 /
    0x1A311489 → 0x561A50: vinyls; decals 0x61–0x98 and 0x34–0x3E;
    paints 0x3F, 0x44–0x4A).
  * UI_ChooseVinylLayer (0x555450 → 0x554CB0, options 0x548460 each
    0x522140): layers 0x40–0x43, label and default icon VINYL_LAYER_1…4
    (0x27B8F0DD…; "Bottom Layer" … "Top Layer"); a layer with a vinyl (the
    setup's, else the car's) shows its group icon (0x50E190 by kind), all
    the Unique icon 0x1B3A8DD3 while a unique one (NUMCOLOURS 0x564B8CB6 =
    0x40) is on or in the setup. "Reorder Layers" 0xD999B6E7 and arrows
    0xCF6F0439 / 0xABA93D3F show on a layer with a vinyl (0x5535E0).
    Select (0x563110): with a vinyl, Change / Remove / Cancel
    (GenericDialog_ThreeButton; 0x06D7902C, unique 0x80C151D5); else the
    browser. Triggers (0x5632A0, not with a unique vinyl): swap the layer
    with the one below / above (0x561AE0: each layer's items, the setup's
    else the car's (colours 0x4D–0x5B — the game's loop misses 0x5C) else
    a removal, move to the other layer, colours keeping their index
    0x50EF50). BUTTON1: the vinyl's colours when it has REMAP.
  * Vinyl browser: UI_GenericParts_Browser in graphics mode, title
    "Graphics Shop" 0xB6AAECF7 (0x545520), groups 0x546040 (28: kind,
    icon, label; order Tear Set, Tear, Stripe Set, … Contest, Sponsor; kind
    0x1C is in no group) each listing the car's slot-0x40 vinyls of the
    kind that the shop lists (0x545930; AD_BESTBUY skipped when 0x7FA73C
    is set — 0x570A40's region, the static 6 in this exe, never written:
    so AD_BESTBUY, kind 0x16, is never listed; verified), locked without any; it opens
    on the layer's vinyl's group. Select (0x5664A0): 0x54FDC0; with REMAP
    (0x39DD714) on to UI_Paint mode 1, else back to the layers.
  * UI_Paint mode 1 (0x53BD60): one strip item per NUMREMAPCOLOURS
    (PAINT_ICON_n, "Choose Vinyl Colors" 0xC034CE3D) over the layer's colour
    slots 0x4D + 4·layer + i (0x50F440); swatches the slot's parts of brand
    VINYL (0x50F5B0). A swatch asks "Choose this vinyl color?" (0x2A2C6AEF,
    more than one colour) or "Would you like to add this vinyl to your
    setup?" (0x31FBC167; checked against the bank: the vinyl's price plus
    the setup's, 0xD1AC60BE). The finish selector group 0x031A9C0A hides.
    Yes (0xD05FC3A3, 0x563AB5): with one colour the vinyl (0x54FDC0) and
    its colour (0x5474E0) go in the setup and on to the layers (0x55BD70);
    with more the swatch goes in the screen's list +0x160 (0x53C170: {slot,
    part}, replacing the slot's earlier pick) and the strip comes back
    (0x55CEA0(0), the preview showing the list), the colour item staying
    selected; no bank check. Back (0x911AB364, 0x563886): from the
    swatches of a many-colour vinyl the strip (0x55CEA0(1)); otherwise
    each listed colour into the setup (0x5474E0) and back out (0x55BE00).
    0x51123592 does the same with the vinyl first unless 0x838983 is set
    and goes on (0x55BD70). **Open**: which sender. Its 13 direct callers
    (procedure_callers) are 0x55BF90, 0x55BFC0, 0x55C530, 0x55C940, 0x55E530,
    0x55F090, 0x561D20, 0x562C60, 0x563420, 0x5664A0, 0x568750, 0x56A890,
    0x56AB10; 0x561D20 sends 0x51123592 to 0x5501F0(0), not to 0x55BD70. In the
    clone (`vinyl_pending`).
  * Paint mode: a slot that cannot be painted (0x50F310) says why on
    Select (0x73BC702E stock parts; 0xD286BE3A engine accents without the
    level-3 engine; 0xE7337E9D / 0xAD460262 / 0xF152BEFA carbon or aluminium
    spoiler / roof scoop / mirrors; 0x6BCBCEEA spinner paint on plain rims;
    brakes need performance part 0x28, 0x29 or 0x2B). A swatch: "Add this
    paint to your current selection?" (0xC4C5D271; price + setup within the
    bank). CASH (0x13C45E) shows the highlighted swatch's price `%$d` in a
    career (0x55CC60 / 0x55D2E0). Paint_text is the label of the finish's
    name + "_PAINT" (0x5484F0: GLOSS_PAINT "Gloss" …; verified); the trigger
    hints 0x9DE5BA07 / 0x7A1FF30D show on slots 0x3F, 0x45, 0x4B, 0x4C.
  * Decals: UI_DecalMain (0x554AD0 → 0x553100, options 0x546A00 by node,
    +0x154): top Windshield (slot 0x69), Rear Window (0x71), Left / Right
    Door (0x10001 / 0x10002: slots 0x79–0x7E / 0x81–0x86, "Slot n"), Left /
    Right Quarter Panel (0x10003 / 0x10004: Layout 1 0x20006 / 0x20008 one
    slot 0x89 / 0x91; Layout 2 0x20007 / 0x20009 two, not on a wide body,
    slot 6 above level 0); top items lock by their location part
    (0x52FF70: slots 0x35–0x3A, 0x515500). The hood (0x10005, slots
    0x61–0x68, stock hood only 0xD104AD36) is coded but not on the top
    level. A layout (0x55FEA0) empties its eight slots and puts the location
    part with that NUM_DECALS (0xA77BDCFA) in the setup, asking first when
    it changes the layout over decals (0x10B83327). BUTTON1 on a node
    removes its decals (0x30F7037F → 0x55FF80). A slot lists through
    UI_DecalsOverlay (0x56B910 → 0x568460, 0x546F50): "No Decal"
    (0x2B5ABCF4) then the slot's decals (category 0x4C, level 0 priced at
    level 1); Select puts it in the setup.
  * Decals open (0x529F10 → 0x529E90): some slot-0x61 decal's brand is no
    performance part's brand (0x502310 over chunk 0x34A15), or is one the
    career records list (0x529E90: the profile's kind-4 records through
    0x5162D0 — the career car slots, 0x7F8 bytes — that have their own car,
    +0x7F5, each up to 0x44 hashes from +0x6E0, 0 ending). +0x6E0 is the
    car record's (+0x434) list at +0x2AC: 0x503E10 (id, brand) sets the
    part's owned bit (+0x15C = +0x590) and appends the brand if there is
    room, for each performance part bought (0x521B10; a package's unowned
    parts, 0x54FE60); the brand is the one picked on
    UI_PerformanceBrandSelect (0x55CA25 → 0x521F00 fills 0x8389B4… from
    each brand option's +0x58 list at its +0x5C index). Never removed.
    The clone keeps it (`Profile::brands` / `slot_brands`).
  * Not in the clone: the car preview of the setup in the shop (the
    decals on the car are drawn, see Cars; DECAL_<LOC>_TEX0–7 are not
    traced). **Verified**: BUTTON1's PC key is `C` (keyboard
    uAppData 0x23, "Menu Customise"; the descriptor rows below). The clone
    binds it (`KeyCode::KeyC` in `src/bin/game.rs`; it kept Space until this
    trace). **Verified** (tables): the FE
    keyboard map 0x7FD7E0 is 32 entries (indices 0-31) of 0x28 bytes, then
    a zero entry at index 32 (action id at +0, 0x81000400 | DIK at +4, name
    pointer at +0xC, rest zero): arrows 0x1B-0x1E ("Menu Up / Down / Left /
    Right"), Enter 0x1F
    ("Menu Select"), NumEnter 0x20 ("Menu Select Alternate"), H 0x21
    ("Help"), Esc 0x22 ("Menu Back"), C 0x23 ("Menu Customise", DIK 0x2E),
    `,` / `.` 0x24 / 0x25 ("Menu Button 2 / 3"), `[` / `]` 0x26 / 0x27
    ("Menu L1 / R1"), Home / End 0x2C / 0x2D ("Menu Zoom In / Out"), P 0x2E,
    Q 0x2F ("Quit"), 0x30 ("Race Options"), 0x31-0x3E the rest; DIK 0x39
    (Space) is in none. Space is action 0x05 in the driving map (0x7FD330,
    entry 10, DIK 0x39; **Verified**). The JOY_EVENT table (id and name-pointer pairs from
    0x7FABA8; the FENG rows from 0x7FABE8) has FENG_START 0x1E, BUTTON0
    0x1F, BUTTON1 0x20 (entry 0x7FABF8), BUTTON2 0x21, BUTTON3 0x22,
    LTRIGGER 0x23, RTRIGGER 0x24, SELECT 0x25, QUIT 0x32, RACEOPTIONS 0x33.
    The offset that once pointed at `C` (BUTTON1 + 3 = 0x23) is only a
    pattern (QUIT and RACEOPTIONS are -3 from "Quit" 0x2F and "Race Options"
    0x30). The table settles it instead: the descriptor row of FENG_BUTTON1
    reads uAppData 0x23, and NumEnter (0x20) is the row of FENG_SELECT_ALTERNATE
    (0x26). **Traced**:
    0x5CFD70 polls each device (0x5CF230 ANDs each device's 64-bit mask into
    DAT_0086EAF0 / EAF4; the keyboard poll 0x5C1850, which clears bit 1 << app
    data for a pressed key, is **Verified** for the default mode (byte
    0x870768 = 0) (decompiled and checked in the disassembly, since the
    pseudocode loses the GetDeviceData count: device type 0x13; a buffered
    DIDEVICEOBJECTDATA with dwData 0 sets the bit, any other value clears it;
    the app data is its uAppData, the writer of which is not traced; with
    0x870768 nonzero it sets the mask to all ones and sets 0x87075C on an
    Escape press, dwOfs 1)) and returns that
    mask to 0x5809D0, which stores each
    event's value byte at DAT_00865658 + action + dev * 0x1C8 (**Verified**)
    and calls the handler callback (dev, action, value, data) (**Inferred**:
    the call is not traced) for the codes 0x57B0C0
    collected (code = action in the low byte, value above it).
    **Verified** (uAppData -> JOY event, the step that was open): binder
    0x5721A0 takes a JOY name (0x571AE0 compares it case-insensitively with
    the 223-entry JOY table at 0x7FABA8) and the device index, and 0x572130
    picks the first row of the descriptor table at 0x7FDD10 whose +8 dword is
    that JOY id and whose codes accept the layout code (0xFF is a wildcard; a
    layout code of 0 accepts only the rows whose JOY id is 1 or 2; **Verified**). The table has 83 rows of 0x20
    bytes: codes at +0..+7, the JOY id at +8, the match function at +0xC, an
    inversion byte at +0x10, the key (uAppData) at +0x12, a second key at
    +0x13, a value byte at +0x14 (the value while pressed; a 0 there, as in the
    file, reports 1; **Verified** in 0x571C80), and a flag at +0x16 (its
    reader is not traced). FENG_BUTTON1 (0x20) is row 6 (0x7FDDD0): key 0x23 ("Menu
    Customise", DIK 0x2E = C), inversion 1, no second key, match function
    0x571C80. 0x571C80 tests the key bit with 0x571B20 (state dword k >> 5, bit
    k & 31; the keyboard bit is active low, a press clears it, and the inversion
    makes a press true) and, when the value changes against the node's last
    value (+0x18, or the pending flag +0x1C), returns (value << 8) + the node's +8 dword (the JOY id, 0x20 for row 6).
    The other FE rows (**verified** from the table; every one has codes
    0xFF.., match 0x571C80 and inversion 1, and the first row per JOY id
    wins): FENG_START 0x24 `,` (row 4), FENG_BUTTON0 0x2E P (5),
    FENG_BUTTON2 0x24 `,` (7; row 22's P comes later), FENG_BUTTON3 0x25
    `.` (8; row 23's P comes later), FENG_LTRIGGER 0x26 `[` (9),
    FENG_RTRIGGER 0x27 `]` (10), FENG_SELECT 0x1F Enter (11),
    FENG_SELECT_ALTERNATE 0x20 Num Enter (12), FENG_CANCEL 0x22 Esc (13),
    FENG_PAD_UP / DOWN / LEFT / RIGHT 0x1B-0x1E the arrows (14-17) and
    FENG_QUIT 0x2F Q (24); rows 18-21 (the PAD_*_ALTERNATE events) are
    axis rows (match 0x571D60, codes 0x0A-0x0D), and FENG_HELP,
    FENG_CANCEL_ALTERNATE and FENG_RACEOPTIONS have no row. The clone binds
    Enter / Num Enter, Esc, the arrows, `[` / `]` and C. `,` (FENG_START, and
    also FENG_BUTTON2) is Key::Start, which the menus ignore; the replay reads
    `,` itself for slow motion. P, `.` and Q are Key::Button0, Key::Button3
    and Key::Quit, which the menus ignore too. P does not hide the replay's
    bar on PC (observed, see Replays). See **FE events on the PC keys** under the pad messages.
    So 0x5809D0 stores the value byte at DAT_00865658 + action + dev * 0x1C8
    (**Verified**; action 0x20 here) and calls the node handler with (dev,
    0x20, value, data) (**Inferred**: that call is not traced). The binder
    0x5721A0 reads the device's kind byte at 0x864788 + dev (0xFF takes the
    dword at 0x7F9414 + dev * 4 as the layout code) and passes it, with the
    device field at 0x865764 + dev * 0x1C8, to 0x572130; 0x5720E0 returns the
    device field if the codes 0x571FE0 lists for the kind hold it, else the
    first listed code. 0x571FE0 and the keyboard's dev index were not traced,
    so whether row 6 binds to the keyboard rests on that layout code being
    nonzero. The other keyboard rows
    give: Menu Up..Right (0x1B-0x1E) to PAD_UP..RIGHT (0x2A-0x2D); Enter 0x1F to
    SELECT 0x25; NumEnter 0x20 to SELECT_ALTERNATE 0x26; Esc 0x22 to CANCEL
    0x27; `[` / `]` 0x26 / 0x27 to LTRIGGER / RTRIGGER 0x23 / 0x24; `,` / `.`
    0x24 / 0x25 to BUTTON2 / BUTTON3 0x21 / 0x22; P 0x2E to BUTTON0 0x1F; C 0x23
    to BUTTON1 0x20. FENG_START (0x1E) also reads `,` (0x24), which is not
    traced further. **Open** (narrowed): the JOY event -> pad-bit step, the node
    handler set when the binding is made (its puVar7[4] is not traced here).
    **Inferred**
    (not re-verified): 0x55DE10 walks the FE list at object +0x4C (node +0x18 enabled, +0x14 region,
    +0x1C mask): for the first enabled node whose region 0x52D110 hits at
    (DAT_008763D4 - 320, DAT_008763D8 - 240), its mask (bits 1 and 2 first)
    goes through 0x52D180 and the message goes to 0x55DDB0. The uAppData ->
    JOY event step is **Verified** (above); the JOY event -> pad-bit step (the
    node handler, set when the binding is made) is **open**. FE messages
    from pad bits (0x52D180: 0x100 Select 0x0C407210, 0x400 Back 0x911AB364,
    0x800 BUTTON1 0xC519BFC0 on UI_PerformanceTuning_Master (else
    0xC519BFBF), 0x200 BUTTON1 if the screen test matches
    UI_CHoosePaintCategory.fng (else Back); BUTTON1 0xC519BFC0 for 0x1000,
    0x2000, 0x4000, 0x40000, 0x800000, 0x1000000, 0x10000000-0x40000000).
    The 0x200 test uses UI_CHoosePaintCategory.fng (0x79B93C); the screen
    table entry at 0x7F8688 (handler 0x55A410) spells it
    UI_ChoosePaintCategory.fng, and the name hash 0x505450 folds a-z to upper
    case, so both spellings hash alike.
    The JOY names are read from the table (BUTTON1 is
    `JOY_EVENT_FENG_BUTTON1`, name pointer at 0x7FABFC); the short labels
    Select, Back and BUTTON1 are **Inferred** shorthand, not checked against
    the hash inputs. The clone binds `C` (`KeyCode::KeyC` in
    `src/bin/game.rs`).
* **Vinyls on the car** (0x61D7A0 → 0x611F20, verified, in `car.rs`): the
  skin's layers 1–4 are the fitted vinyls bottom to top (a vinyl a lower
  layer has is skipped): image `<CAR>_<TEXTURE string>` (0x61D630) and its
  `_MASK`, both in CARS/<car>/VINYLS.BIN (HUFF blocks; the images are
  compression 0x80, 0x81 the unique / contest ones; their D3D format
  records say D3DFMT_P8 with a 0x400-byte palette like 0x08 for all
  24,071 0x80 and 1,840 0x81 images — verified, `tpk_block <file> 0`;
  a P8 block's palette offset is rebased with its pixels). With
  REMAP the texel's R, G, B (0–1) weigh the layer's first three colours
  (0x611B10: sum of channel × weight, truncated, clamped; alpha the
  largest); an empty colour slot k is 0xFF in byte k. Mask green ≥ 128
  replaces, below blends by m/128; then the doorline.

* **Car specialties shop** (UI_ICEMAIN, 0x560560 → 0x55C020, options
  0x5593F0 mode 0; title 0xDF86227A): Custom Gauges (slot 0xA3,
  UI_CustomHUDOverlay), Neon (0x53AA90), Window Tint 0x99, Lights 0x9F,
  Nitrous Purge 0xA2, Hydraulics 0xA1, trunk audio (0x53AAD0), Spinners
  0x1D, Split Hoods 9 (the kind-5 hoods), Doors 0xA0 (icons / labels in
  `SPECIALTIES`). The part categories go through UI_IcePartsOverlay:
  MINI_MASTER / PARTS_ICON with the part's name (0xD28B9316) and price
  (0x0380B632), as in the body shop; Custom Gauges (slot 0xA3: Flames,
  Griffin, … Bling) the same through UI_CustomHUDOverlay, with
  UI_ChooseCustomHUD's preview and colour steps (below) before a custom
  gauge's purchase. Neon (item
  slot 0x9E, icon 0x2929058, label 0xA420BE7C, 0x53AA90 mode 7) opens
  UI_CustomNeonMain (0x554C50 → 0x553500; slots 0x548020: Underglow 0x9A,
  Engine 0x9B, Trunk 0x9D, each through UI_NeonPartsOverlay; parts
  NO NEON, the colours, and the PULSE ones). Neon is locked (0x539FC0)
  while all three slots are (0x539D10): a slot with nothing beyond stock
  (0x539970); engine neon also until the level-3 engine (performance
  category 3) unlocks (0x52A120 → 0x515630); trunk neon also without a
  level-3 trunk neon part (0x50ED20) that passes the trunk layout's unlock
  (0x52A020 → 0x515500, slot 0x22). In the clone: those nine
  categories, the neon slots and their locks, and the trunk audio:
  the item (slot 0x22, 0x53AAD0; its constructor stores FUN_00539D10(0x22) at
  +0x22) opens mode 2 (below) when no layout is fitted or byte +5 of the fitted
  layout has no 0xE0 bits (0x55BFC0), else mode 3 under "TRUNK AUDIO" (0x2AADAC5): Audio Components
  (0x76167B7E / 0x28BF66F6, mode 4) and Trunk Layout (slot 0x22: STOCK,
  STANDARD, TUNED, CUSTOM LAYOUT). Mode 4 lists "Trunk Slot %d"
  (0x3F4B9E43) for the slots the fitted layout opens (0x610400): none
  without one; level 1 up to 0x24 (SUVs, car types 8–10: 0x26); level 2
  up to 0x26 (SUVs 0x28); level 3 up to 0x27 (car types 5, 0xD, 0xF,
  0x12, 0x16, 0x19, 0x1A, 0x1C, 0x1E: 0x28; SUVs 0x2E). The clone has modes 3 and 4
  (`audio_browser`). Mode 5 runs the mode 4 code (0x5593F0's cases 4 and 5
  are one block; 0x568750 sets it at 0x5689AD, not traced). Mode 2 (0x5593F0
  case 2, verified) is the Trunk Layout screen, title TRUNK AUDIO (0x2AADAC5), with
  only the slot-0x22 overlay (0xF7E5B04A / 0xC8D23393), added after
  0xA2285688 and 0xC318FF76 are hidden, then FUN_00556940. The trunk audio
  item (slot 0x22, 0x53AAD0) has vtable 0x79BE5C with slot 1 = 0x55BFC0,
  which runs on event 0xC407210: DAT_00838998 = 2 when slot 0x22 holds no
  part (FUN_00610300) or byte +5 has no 0xE0 bits, else 3; then FUN_0054F990
  and FUN_0055BD70 (the rebuild that reaches 0x5593F0 is not traced). The
  constructor stores FUN_00539D10(0x22) at +0x22: true when at most one
  slot-0x22 entry counts (threshold 1; FUN_00539970, the count, is not traced).
  Case 0 adds the item unconditionally, but whether +0x22 gates the select is
  not traced, so reaching mode 2 from the parts list is unverified.
  FUN_00556940 draws and sets: hides 0xDF422345; sets label 0xC318FF76 to
  0x56DA42E0; if the current item's slot (+0x58 then +0x48) is not 0xA3, hides
  0xA2285688 and 0xC318FF76; sets +0x169 = 1 and +0x7B = 0; stores the item's
  +0x58 field in +0x160 and calls FUN_00555F20 (not traced) when the item and that field are nonzero and
  FUN_0052CF70 returns 0; clears DAT_008A1C68 and DAT_008363F8 (1 for slots 0x9F and 0xA1);
  saves +0x28 into +0x158 and sets +0x28 to 0xD3E04E45 (slots 9, 0x99, 0x9F,
  0xA1, 0xA0, 0xA2), 0xA93E10E9 (0x23-0x2E) or 0 (0xA3); for slots 9, 0xA0 and
  0xA2 sets 0xC318FF76 to 0xBD620BB1 and calls FUN_0054FB80(item, 1); selects
  in UI_ICEMAIN.fng the entries of lists 0x55C171E2 and 0xF1E6850B whose +0x30
  is FUN_00505450(&DAT_0078FC44) (list +0x54, FUN_00517720, +0x10 = 0); sets
  DAT_0083854C = 0. FUN_004954E0 and FUN_00495520 call 0x50CA00 and 0x50CA50
  (both test bit 0x1 of +0x1c; read as show and hide, bodies not fully traced).
  FUN_00495460 calls 0x537A20 and 0x50C900 (not traced; label text unverified). The clone: `trunk_audio`
  (src/bin/game.rs). The fitted
  gauges pick the HUD's
  `_CUSTOM_nn` textures: nn is the slot-0xA3 part's HUDINDEX (0x4F56E6;
  Flames 1 … Invader 8; 0 with none).
  The split hoods list only kind 5 (0x539970: slot 9 keeps kind 5; the
  body shop's 0x5449D0 leaves kind 5 out); verified. The item's +0x54
  (−1 by default, 0 split hoods, 2 doors, 0x5596C9…) is the car preview's
  animation (0x5609FD → 0x4C18B0; with slot 9 or 0xA0 also 0x438030),
  not a filter.
* **Covers**: after a part is fitted the stage's DVDs whose rating the car
  reaches are offered, the others withdrawn (0x5331D0). An offered cover
  waits at its spot (milestone +0x2C), a route trigger (see Triggers):
  0x533320 walks the kind 13–15 triggers (0x5D9C80 mask 0xE000, no
  point) and pairs a trigger whose +0x30 hash is an offered milestone's
  +0x2C (0x501150) with it, as it pairs shops (+0x3C) and open events
  (0x4FFC50 by the event's +0x28). Driving there
  (into its trigger) takes the shot (0x533690): the clone shows "Magazine Cover Shot" and the
  cover's `<cover>_TITLE` ("Bayview's Best") instead of the camera screens.
  Verified end to end by the test `parts_reach_the_dvd_cover`.
* **Countdown**: 3, 2, 1 then GO, each a step of race status +0xC, 0.9 s
  (set at 0x5FC740; 0x5EEA50).
* **Post-race chain** (0x4F3EA0): UI_PostRaceResults (0x4FBB60), the
  URL tournament results / standings, UI_PostRaceReward (career),
  UI_SponsorPopup (0x4C1280), UI_Sponsorship_new, UI_PostRace. The
  results (0x4E9640 → 0x4D7240 → 0x4BF290) list the racers in finishing
  order: RANKING_DATA_n, PLAYER_DATA_n, CAR_DATA_n (CAR_NAME_<car>),
  TIME_DATA_n (0x4A4020); unused rows hide (0x4A41A0); the player's place
  runs HIGHLIGHT; offline the online timer hides. The clone follows
  results → reward → sponsor popup and sponsorship (when offered) →
  post-race.
* **Race stats** (UI_PostRaceStats, screen 0x4FD6C0 → 0x4FBBC0;
  **verified** except where noted): a page per racer in finishing order,
  the arrows 0x444969FD / 0x444969FE paging (0x4A4480: the left hides on
  the first racer, the right on the last, the first winning with one
  racer). 0x515DB0 builds each racer's results record (0x89 dwords from
  0x85036C): +0 place, +4 racer, +8 status, +0xC finish time, +0x10 best
  lap (0x5EE2C0; the finish time with no laps), +0x1C + 4·lap the lap
  times (racer +0x98 on), +0xF4 laps led, +0xC8 + 4·lap the place at
  each lap's end, +0x10C on a copy of racer +0x1A0 … +0x1D0. After the
  race 0x541B50 ranks the racers on their time at each lap (laps 1–21)
  into +0xC8 + 4·lap, a lap in first adding to +0xF4. The racer's stats
  (+0x1A0, kept each frame by 0x5F7420): +0x1A0 the top speed (the car's
  speed +0x42C, the player's taken with more than two wheels down), +0x1A4
  the average (distance driven, car +0x414, over the race clock / 4000),
  +0x1A8 / +0x1AC the 0–60 / 0–100 mph times. 0x4E9C00 fills the page:
  RACER_DATA `%2d   %s - %s` (place, name, car name), then the option
  list (0x552790: rows OPTION_NAME_n / OPTION_DATA_n from 1, laid out
  from TITLE_MASTER and DATA_MASTER by 0x51FF90 / 0x54F830: names'
  top-left at the title master's, the data's right edge at the data
  master's (0x4A4300), each row the title master's height lower; the
  names are authored hidden; unused rows INIT to HIDE): FINISH TIME
  (0x4BF9E0, the time), TIME BEHIND (0x4A4380: the leader "--:--.--"
  0x61C9B7A2, other finishers the difference with a sign; other
  statuses Knocked Out, Blown Engine, Totaled, Timeup, Disconnected or
  Finished), TOP SPEED and AVERAGE SPEED (0x4BFA30 / 0x4BFAA0, `%$3.1f
  %s` in mph × 2.237, or km/h × 3.6 when 0x83AA1A is 1), and unless the
  race is point to point (0x89E7B4, which also drives the HUD's `%3d%%`)
  BEST LAP TIME (0x4BFB90) and LAPS LED (0x4BFC20, `%d/%d` of the laps).
  On a circuit the lap chart (0x4D73B0 → 0x4BFD40, laps 1–10, or 11 on
  when paged) fills TIME_DATA_n `%d   %s` (lap, lap time) with its
  top-left down from TIME_MASTER's and POSITION_DATA_n `%d` centred on
  POSITION_MASTER, a master's height apart; point to point hides the
  headings HEAD_TEXT_2 / _3. Signed times (0x577250 flag 2 → 0x576D40):
  '+' or '-' takes a one-digit leading field's blank, or goes in front of
  a two-digit one and the last character drops. The distance driven
  (car +0x414) is the sum, every frame while racing (race state 4), of
  the car's movement since the last frame (+0x510), in the car update
  0x604840; reset at set-up (0x5A93BD).
* **Race stats by race kind and the results rows** (menus110,
  2026-10-31; **verified** in the code, **observed** under Wine on a Quick
  Race drag, Bayview Bridge, 3 opponents, the player's engine blown):
  * The screen opens on the local player's page: 0x4FBBC0 walks the
    results list 0x851044 and pages forward to the entry whose racer (+4)
    is 0 (Wine: "4 NAME - 206" first, only the left arrow).
  * 0x4E9C00 fills a page. The lap chart (0x4D73B0) only when not drag
    (0x89E7D8 / 0x890108) and not point to point (0x89E7B4); else
    HEAD_TEXT_2 / _3 hide. Rows, each an item whose vtable slot 3 writes
    label and data: not drift (0x89E7D9 / 0x890104): FINISH TIME
    (0x4BF9E0: record +0xC through 0x577250 flag 0), TIME BEHIND
    (0x4A4380), TOP SPEED, AVERAGE SPEED, then in a drag race PERFECT
    SHIFTS (0x4BFB10: "%$d/%$d" of record +0x128 / +0x124) and nothing
    more, else (not point to point) BEST LAP TIME and LAPS LED. Drift:
    SCORE (0x4BFD00, "%$d" of ftol(record +0x21C)), BEST DRIFT SCORE
    (0x4BFC60, "%$d" of +0x214), LONGEST DRIFT (0x4BFC90: +0x218 s ×
    4000 (0x784264) + 0.5 (0x784260) ticks, 0x577250 flag 0), TOP SPEED,
    AVERAGE SPEED. Labels: 0x680AC597 "PERFECT SHIFTS:", 0xE8AEBFAF
    "SCORE:", 0xD7D8CA79 "BEST DRIFT SCORE:", 0x5C17EC47 "LONGEST DRIFT:".
  * Record (0x515DB0): +0x10C… copies racer +0x1A0… (13 dwords), so
    +0x124 / +0x128 are racer +0x1B8 / +0x1BC. In a drift race (and with a
    style manager, racer +0x1D4) its slot 0 — the drift trick, 0x56EBF0
    builds slot i from table entry i — gives +0x21C = trick +0x2C (banked),
    +0x214 = ftol(trick +0x44, the best run), +0x218 = trick +0x40 (the
    longest run, s).
  * Shifts (0x5F7420, with ECX = racer +0x1A0): when the car's gear
    (+0x434) is above the last seen (+0x1CC; 1, neutral, at set-up
    0x5F7936) and not 0, shifts (+0x1B8) += 1, and perfect (+0x1BC) += 1
    when car +0x28 is 1 (the physics mover) and the transmission's last
    up-shift rating (car +0x34 → +0x4C → +0x68) is over 0.5; +0x1CC then
    takes the gear. Wine: the blown player "0/1" (neutral → first), the AI
    "0/3".
  * TIME BEHIND for a racer that dropped out shows its status (Wine:
    "Blown Engine"); FINISH TIME and the results' time are 0 for it
    (record +0xC = racer +0x198 is never set): Event Results list it
    "0.00" and, on the Time Behind phase, "-1:24.28" (0 − the winner's).
  * Results rows (0x4BF290): RANKING_DATA_n runs HIGHLIGHT on the row of
    the local human (racer type 1, not +0x900), not on row 1 (the clone
    highlighted row 1). Rows past the racers (0x4A41A0) hide
    RANKING / PLAYER / CAR / TIME_DATA_n only; the row backings (groups
    0x949F4946 + n, INIT → HIDE) are not touched there — yet under Wine
    rows 5–6 show no backing with four racers. The backings' INIT (alpha
    0x91) chains to HIDE (alpha 0); their 0xBDFD2DF9 response names
    FADEIN, which the groups do not have, so the start does nothing
    (0x5046B0 finds no script); only HIGHLIGHT on RANKING_DATA_n (its
    events send 0xABC08912 to the row's texts and backing) shows one:
    under Wine (menus110 s13) only the player's row has a backing, rows
    1–3 and 5–6 none (**verified**; the clone: `ScreenState::chained`).
  * Times under a second (0x576D40, **verified**): with no hours,
    minutes or seconds the text starts from the template "0.00"
    (0x7A0630; "0.--" 0x7A0628 without flag 4) and writes the
    hundredths — "0.64", "0.00", not "  .00" as the clone had; signed,
    the sign goes in front ("+0.00" with flag 8; without it the last digit
    drops, "+0.0").
  * Clone: `RacerStats::{status, shifts, drift}`, `PostRaceStats::{player,
    drag, drift}`, `PostRaceResults::player`, `Progress::{shifts,
    perfect_shifts, gear_seen}`; shots `drag_results`, `drag_stats`.
  * Rows' z (**verified**): the row item's layout (vtable
    0x795130 slot 4, 0x4A4300, called by 0x54F830 as the page fills)
    places the data through 0x51D2E0, which writes (x, y, the object's
    current z) with 0x5178D0 — into the position track of every script in
    the object's list +0x4C (the list 0x5046B0 searches by name, not only
    the running one) and into the object. Done at construction, the z is
    INIT's (1); the option_data FADEIN's keyless position track (z 100)
    then holds that z too, so the data draw in front of the right panel,
    white (Wine s17). The clone takes the authored z for any object whose
    position the screen sets (`place`, frontend/mod.rs).
  * The top bar's dot is the option list's scroll-bar handle: 0x552790
    builds its bar with prefix "scrollbar" (0x542AD0: scrollbar_Backing
    0x688A1551, _Handle 0x3FBB7A4E, _Arrow_1 / _2 0x444969FD / FE) and,
    with TITLE_MASTER and DATA_MASTER present, hides the handle and both
    arrows (0x50CA00: hidden flag | 0x400000; the FADEIN the handle
    answers 0xBDFD2DF9 with does not clear it). The paging (0x4A4480)
    shows the arrows again; nothing shows the handle (**verified**; Wine
    s17: no dot; the clone hides it).
  * Average speed (**verified**): 0x5F7420 sets record +4 = car +0x414
    (distance) / (race clock 0x890118 +0x14 × 0.00025 s) each call; it is
    called by 0x6016B0 from the race update 0x6076E0 only while the
    racer's status word +0x10 is 0 (0x606CC0 sets it when the racer ends)
    or with no race on (0x89E7B0 = 0). So a drop-out's average freezes at
    its drop: distance then / clock then — what the clone does (the Wine
    24.5 mph vs the clone's 26.8 were different runs).
* **Opponents** (verified data, set up by 0x5260C0): an event's +0x7C
  is its opponent count, the opponents 0xC bytes each from +0x40: u16
  driver name (career string pool, e.g. Alden, Tom, Russell), u8 skill
  source (+0x02: 0 the fixed skill at +0x03, 3 from 0x5132E0; 1 and 2 not
  traced), u8 skill, u32 car (bin_hash "RANDOM" for every event checked:
  0x4FEAF0 picks a car; else the car type), u8 × 3 per difficulty
  (+0x08…+0x0A, read at +0x08 + difficulty 0x89E7F4), u8 +0x0B. The skill
  (0–10) sets the AI car's performance between the bounds 0x59A460 gives
  (lerp by skill / 10, 0x5B6AE0). RANDOM cars (0x4FEAF0) come from a
  pool of car type indices: 0x7F6DA4 (26: 1B 13 0F 16 0D 0B 10 15 02 0E
  1E 1C 1A 04 1D 03 01 00 18 17 14 11 0C 19 05 12), 0x7F6DC0 (the same
  ending 07 06; when 0x570A40 holds), or the SUVs 0x7F6DDC (08 09 0A)
  when the player's car or the event (+0x0D) is one; shuffled, the first
  car (the player's type skipped unless param 4) whose scaled rating
  (0x5B69A0(1, −1) on record +0x10 after 0x610280 sets it) is at most the
  skill (param 7, or with param 7 < 0 0x5B69A0(1, −1) on racer info 0
  +0x20, the player's upgraded performance) plus a margin from 0.06
  doubling each pass is taken (0x4FEC50..0x4FECCC, re-traced 2026-11-04
  for docs/drag-drift-outrun-parity.md; an older line here said "a random
  0–1"). The clone: see **Opponent line-up** and **AI car looks**
  (presets, 0x637280).
* **Opponent line-up** (0x5260C0 with 0x4FEAF0, **verified**; the clone's
  `opponents` module):
  * Skill (0–10) by the entry's source byte +0x02: 0 the skill byte +0x03;
    3 the stage's range (0x5132E0 on the current stage record 0x862018 +
    n × 0x104, whose +0x100 is the 0x34A18 definition): from +0x44 to +0x48
    by wins / +0x42 while the wins are at most +0x42 (and it is not 0),
    else +0x48 — the wins being the stage record's normal (+0x00, 0x5004D0),
    sponsor (+0x8C, four) and URL (+0x9C, 0x500520) entries. Stages 0–5
    read (1, 0, 0), (5, 0, 1), (16, 0, 3), (28, 3, 4), (40, 4, 8), (47, 8,
    10). Any other source takes the player's car's scaled rating.
  * Car: an entry car of bin_hash "RANDOM" is drawn (0x4FEAF0, called with
    the player's car allowed) from the pool 0x7F6DC0 — 0x570A40 returns
    0x7FA73C, a static 6 nothing writes, so the first pool 0x7F6DA4 (region
    0's; docs/region.md) is never used in the European release — or the SUVs 0x7F6DDC when the event's +0x0D is 1. The pool is
    shuffled (each place swapped with place `random(n)`), then with a margin
    from 0.03 doubled before each pass, the first car whose scaled rating
    (0x5B69A0) is at most skill + margin is taken. Otherwise the entry's
    hash names a preset (0x637280 → 0x61C470 by the preset's name hash;
    no such preset leaves the record as it was): the career's boss and
    rival entries CALEB_GTO, NIKKI_MUSTANGGT, TOM_G35, AL_RX8,
    NIGEL_3000GT, MARCUS_CELICA, SCOTT_TT (the maintainer's probe `opponent_cars`).
    The clone used to look the hash up as a car type, found none and
    dropped those opponents; it now takes the preset's car type and parts
    (see **AI car looks**).
  * Rating: the racer's performance object (+0x10 of its racer info) keeps
    the car's rating at +8 (0x5B4DD0): the area between its acceleration
    curve and drag (the class's area, see the drivetrain section), not set
    when the class is fixed (+0x844). 0x5B69A0 scales it to 0–10 between
    the bounds at 0x86B3F8 / 0x86B3FC. Those bounds, with the rest of the
    30-float block at 0x86B3D0, are copied by 0x599470 from GLOBALB chunk
    0x34A32 (inside 0x80034A30): rating 144.12–613.90 (the stock Corolla's
    rating is exactly the minimum), and at +0x70 / +0x74 0.03–0.9236.
  * Random numbers (0x43C1C0): r = state mod n, then a ^= 0x1D872B41, b =
    a ^ (a >> 5), state = (b << 27) ^ b ^ a; 0x43C1F0 takes state mod
    0x7FFFFFFF as a fraction of 2³¹ times its argument. The state 0x7EA624
    is a static 0x12345678, but the start-up chain reseeds it once
    (0x57ED5C → 0x57ECC0: 0x43E6B0(0x43BDF0() + 0x12345678), the
    QueryPerformanceCounter reading, IAT 0x78311C; a replay or online game
    seeds it instead), and 0x581775 writes it again for replays / online
    play. So rivals, names and grid slots differ from boot to boot
    (**verified** 2026-11-04, see internal notes item 14). The
    clone's window seeds it from the clock (`GameRng::boot`); headless
    runs keep 0x12345678.
* **Tournaments**: an event's races are its +0x18 slots (u16 track, u8
  variant, u8 laps, up to four; 19 URL events have 2 or 3). The game keeps
  one 0xD24 record a race (0x85BAB8, current 0x85EF48, count 0x85EF4C:
  all the event's races, since 0x503080's "a race is left" compares the
  current record + 0xD24 against base + count × 0xD24). With more than one
  race (0x863470 set) the chain 0x4F3EA0 puts UI_PostRace_TournResults and
  UI_PostRace_TournStandings (0x4C0050 / 0x4C00C0) in place of
  UI_PostRaceResults after every race. **Verified**:
  * Points: a race place's points are the dword table 0x7F7B8C indexed by
    place − 1: 8, 6, 4, 3, 2, 1, 0 (0x513E70 for this race through
    0x513E30's place; 0x513EB0 sums them over the races whose record state
    +0x14 is 3 or 4).
  * TournResults (0x4F41F0 → 0x4EA490 → 0x4C0110 / 0x4D7400): rows sorted
    by place (0x4A4710); RANKING_DATA_n `%2d` the place (0x4C0300, the
    player's runs HIGHLIGHT), PLAYER_DATA_n, CAR_DATA_n (CAR_NAME_<car>),
    TIME_DATA_n, PLAYER_POINT_DATA_n `%d` this race's points.
    "Waiting for Server Authentication..." (0x1F105857) INITs to HIDE.
  * TournStandings (0x4F4610 → 0x4EA570 → 0x4C07A0 / 0x4D77A0): rows by
    summed points, highest first (0x4A4730); RANKING_DATA_n the rank
    (0x4C0A00), PLAYER_DATA_n, CAR_DATA_n, PLAYER_POINT_DATA_n the sum, no
    times. NEXT reads "Next Race" (0x23F359B8) while a race is left
    (0x503080), "Quit" (0x224936F4) when 0x5027B0's best-of count is
    decided (a per-mode table 0x7992D4; not reached by URL events in the
    clone). OverTime is online only.
  * Margin: 0x513FE0 with more than one race is on points, best / next
    best − 1; with no other racer (next best 0) it is 100%.
  The clone races the event's races in turn with these two screens after
  each; the reward and the win come after the last, on the summed time.
* **Race route and progress** (0x5E5BC0, verified): the paths file's
  road network is 0x34148 nodes (0x18: f32 x, y, u16 segment, f32 at
  +0x14), 0x34149 segments (0xDC: i16 first node +0x1C and end node +0x1E,
  exclusive; u8 connection counts +0x22 / +0x23; connections at +0x24 and
  +0x44, 4 × 8 bytes of u16 segment, u8 the direction it is entered in,
  made pointers by 0x5D5640; f32 length +0x64; i16 barrier count +0xBC and
  indices +0xBE into 0x3414D, 0x24-byte records closed by +0x10, which is 0
  on disk; 0x578070 opens them all and closes those whose name hashes
  (+0x11 count, from +0x14) name the race's `BARRIERS_<n>` or
  `PLAYER_BARRIERS_<n>` before the route is searched (0x57A660; ported in
  `career::load_route`; none of the 204 forward and backward routes
  changes with it)). A
  segment run in direction 0 goes first node to last and leaves by its
  +0x44 connections, direction 1 the other way by +0x24. The race route
  goes through the checkpoint list (backward variant: reversed) pair by
  pair: from the segment of the node nearest the first point (0x5D57B0)
  to that of the node nearest the second, by the depth-first search
  0x5D53E0 (at most 16 segments a pair, depth limit 10, bound 9999999
  shrinking to the best distance so far, no segment twice except the
  target, none behind a closed barrier, the shortest by segment length).
  The first pair's direction is 1 when the pair runs against the
  segment's node order (dot ≤ 0), later pairs carry on in the last
  segment's. Segments repeat only as the last one taken or, on a loop,
  the first; any other repeat fails the build. The list's +4 byte asks
  for alternate branches as well and +5 goes to route +0x20 as a float
  (default 128); neither is a checkpoint radius. Checked by
  `race_roads_match_the_track_lengths`: all 102 tracks build; a circuit's
  road is the record's +0x98 length, a sprint's or drag's start-line to
  finish-line run along it is too (4101: 4413 m against 4419; 4213: 1610
  against 1609). The road itself (0x5E48C0, verified; `RoadNet::race_road`): route
  +0x18 and +0x1C are set to 1 first, then per route segment its spline
  (+0xA8) is freed and rebuilt by 0x5D50A0: 0x57A310 type 2 with the
  0x865230 matrix, a uniform Catmull-Rom through (x, y, 0, w) control
  points (derivative matrix 0x865270; piece `trunc(t)`, 0x570650; past
  either end the end span carries on in a line, 0x570250 / 0x570520):
  a phantom, the segment's nodes in file order, a phantom. A phantom is
  the end node (+0x88 first, +0x90 last) plus the neighbouring route
  segment's lead-in (its second node less its first, or its last but one
  less its last, by its direction; on a loop the route wraps), else the
  end span mirrored. w is the arc length (0x5778E0: 16 chords a span);
  +0x70 = w at t = 1, +0x6C = w(n) − w(1). The segment's start is a
  fresh locator's (0x5DEDA0 on the stack, so +0x20 = distance) place for
  its driving start node on the splines built so far: 0 for the first,
  else the last end plus the gap. 0x5D52D0(start, 1): direction 0 sets
  +0x74 = start, +0x7C = 1, direction 1 +0x74 = start + length, +0x7C =
  −1; +0x84 = the end; each node's +0x14 = (w(t) − +0x70)·+0x7C + +0x74.
  The road's length (route +0x18, half +0x1C) is the last end. Checked:
  the rebuilt lengths match the files' exported +0x6C to ~0.01 m except
  where the neighbours differ, and 4013's road is 2201.2 m against the
  record's 2201 (the node polyline gave 2196). The polyline the clone
  keeps for drawing and steering is the nodes at their +0x14.
* **Road locator** (0x5DA340 → 0x5D9E40, verified; `RoadRoute::locate`):
  0x5DA340 re-locates when |Δx| + |Δy| from the last ask (+0x08) reaches
  +0x38 (`RoadRoute::relocate`). +0x38 is 0 from 0x5DEDA0 and from the
  car's 0x5DA300 (0x6042E7), so a racer's locator re-locates every ask;
  the car set-up sets 16 m (0x6043DD) only for a car whose owner kind
  (car +0x14 → +4) is 3, which is traffic (verified: 0x6065C0's third
  loop hands each car-table entry with +4 = 3 to 0x42F7A0, which passes it
  as the owner to 0x42E0B0 and on to 0x42C670; 0x604270 reads +4 at
  0x6043D4-0x6043DB; kinds 1 and 2 go through 0x606490 and fail the test).
  The clone's traffic (src/ai/traffic/) has no per-car `Locator`; its
  cars move by `TrafficLane` and `Slot` records. No hit (0x5D9ECE) zeroes
  the node, place and fraction and
  points +0x28 along +x, keeping +0x1C and +0x38. The node (+0x18) is
  kept unless the point is over 128 m from the last ask, else 0x5D57B0
  (route segments with splines whose bounds +0x98 / +0xA0 widened by the
  radius hold the point; the nearest node; no hit resets the locator to
  0). 0x5D5A10 moves it to the nearer end of the nearer piece either side
  (0x5D4E40: point-to-piece distance, the end nearest by projection). With
  node byte +0x0A ≠ 0xFF: twice the distance under +0x0B but at least
  +0x0A tries the node's links (+0x0C/+0x0E/+0x10), twice the distance
  ≥ +0x0B a 0x5D57B0 search within 128 m; a candidate wins when its
  0x5D5A10 distance + 1 + max(0, |0x5D5CE0(+0x14 Δ)| − 40)/4 is lower.
  The parameter starts at the node's (index − first + 1), or the last
  one for the same node, and 0x5774D0 refines it (miss = (q − P)·D/|D|,
  done under 0.05; step miss/slope, the slope |D| then the secant of the
  last two misses, capped at a bound from 1 halved on reversal; ≤ 998
  steps; type 2 is not clamped). +0x24 = (w(t) − +0x70)·+0x7C + +0x74,
  +0x20 = +0x24 / route +0x18, +0x10 the point, +0x28 the unit direction
  (negated on +0x7C < 0). A racer's car locator is car +0x8D8 (+0x8F8 the
  fraction). The clone's racers and standings use it; the bounds are the
  segment's node bounds: **verified**, 0x5D57B0 passes the segment's +0x98
  (min) and +0xA0 (max) corners to 0x43D410, which tests the point against
  [min − r, max + r] on x and y (r the radius).
* **Racer progress** (0x5F7C40, verified; `standings::Laps`, game.rs
  `Progress::update`): p = car +0x8F8 × 100. For each race line i (status +0xBB4 +
  4i, percent of the road, count +0xBF4) 0x5EE160 tests a crossing: a =
  0x5D5D20(p, line), b = 0x5D5D20(last p (+0x1C), line) (0x5D5D20: a −
  b, a loop's ±100 past ±50); none if |a| > 25, forward when b < 0 ≤ a,
  backward when a < 0 ≤ b. On the finish line (+0xBF7) forward adds a
  lap (+0x08, the most +0x09 and flag +0x0D), backward takes one off (not
  below +0x09 − 1). Each forward crossing sets +0x04 = i + 5·lap; a new
  best (+0x06, flag +0x0E) stamps split and lap times (racer +0x94 on).
  Then +0x14 = +0x20 = lap + 0x5EE9C0(p), +0x18 a 0.667-smoothed rate
  (not kept by the clone), +0x1C = p.
  * Lines (0x5EE630 → 0x5EE500, none in race types 7 and 8), appended in
    this order: ForwardStartLine (+0xBF6; default 5%), SplitTimeBoundary1
    (default start + 33 %), SplitTimeBoundary2 (start + 66), 3 (default
    −1: only with a marker), and on a point to point race (0x89E7B4)
    ForwardFinishLine (+0xBF7; default start + 95; the marker whose +0x0C
    is 0x89E7D4 when that is set), else +0xBF7 = +0xBF6. Backward
    (0x89E7A4): BackwardStartLine, SplitTimeBoundary3, then 2 at start +
    33 and 1 at start + 66 (the split names have no direction), and
    BackwardFinishLine. 0x5EE500 takes the first marker of the name (with
    +0x0C equal to the key when one is given): its percent is a fresh
    locator's fraction × 100; none: the default, past 100 less 100,
    unless negative (then no line). On a point to point race a split
    still exactly at start + 33 / + 66 (past 100 less 100) moves to start
    + 0.33 / 0.66 × (finish − start). No shipped TrackPosMarkersAll.bin
    has a SplitTimeBoundary marker, so the splits are always at their
    defaults (`RaceLines::with_splits`).
    0x5EE9C0: equal start and finish percents, (p − line, ±100 past ±50,
    below 0 + 100) / 100; else (p − start) / (finish − start), past 1
    less 1. All in the clone; the split times are `Laps::split`.
  * Start (0x5FC2B0): +0x08 lap −1, +0x04 / +0x06 −1, +0x09 0, +0x1C =
    p, then one 0x5F7C40 (crosses nothing). The green light (0x5FC910
    state 4) sets each racer's +0x30 (line 0's crossing tick) and +0x28
    to now, +0x06 = 0, and on a point to point race +0x08 = +0x04 = 0. So
    a circuit's grid, behind the line, makes lap 0 at its first crossing
    (no new most, no lap time) and lap n at the n-th after; a sprint's
    finish crossing makes lap 1.
  * Times: a forward crossing whose line + 5·lap passes +0x06 stores the
    time since line 0's crossing tick (+0x30) at +0x44 + 4·line (last at
    +0x58, best +0x6C), at the finish line also the lap's slot (+0x94 +
    4·lap, laps 0–20; running sums +0xE8 on), then that line's tick
    (+0x30 + 4·line) = now. On a circuit line 0 is the finish line, so a
    lap's time runs from the last lap's crossing (the first from the
    green light); a sprint's single lap from the green light.
  * Finish (0x608C11, each frame for every race type but 7, racers with
    status +0x10 clear): +0x09 equal to the race's laps (+0xBE8 =
    0x89E7BC, 1 for point to point, drag or 0x890108, 0x5FC7B4; none when
    0) → 0x606CC0(racer, 0, 0, 1): status 1, finish order +0x0C from
    0x5EEF80, finish +0x198 = the finish line's crossing tick (+0x30 +
    4·+0xBF7) − race start (+0x10), i.e. the race clock at the crossing.
    (0x5F7ED0 fills the laps not done when a racer is ended short of
    them.) The clone runs this from the green light only (cars are held
    in the countdown) and keeps the lap times in order for the stats
    and `lap_places`.
  * HUD (0x4AA1B0): LapNumber `+0x09 + 1` (the clone keeps it within the
    race's laps), NumberLaps `/` +0xBE8; point to point `%3d%%` of
    trunc(max(0, +0x14) × 100).
* **Standings** (`src/standings.rs`; **verified** except where noted):
  0x5EEC90 bubbles the racers' list (race status +0xBD0, kept from frame
  to frame) with 0x5EEC10 until nothing swaps, then a racer's place
  (+0x0A, the last one kept at +0x0B) is its index + 1. A racer goes
  ahead of the one before when both have finished (+0x0C) and it finished
  first, else when its progress in laps (+0x20) is greater (race type 7
  compares +0x24; the clone has no such race).
  * Reference lap (+0xC34, 0x804 bytes, built at race start by 0x605260 →
    0x5F6E80 after the racing lines' speeds are set, 0x57F957: 0x57A660
    then 0x60A690; read by 0x5EDDA0): +0 the lap's time, then 256 bins of
    (rate in laps a second, time at the bin's start). The walk starts at
    the racer point (flag bit 11) nearest the start line marker (+0xC10,
    ForwardStartLine / BackwardStartLine via 0x5EE500): 0x5E4090 with 40 m
    and mask 4 tries the race records then the other listed ones, each
    record's best (0x5E2E50) shrinking the radius. Each point of a record
    set for its direction (+0x74/+0x75, +0x7C/+0x7D; record +0x70 bit 1
    backward) is sampled at 0 m and 1 m along it (0x5DD300 / 0x5DD440) at
    the racing line's offset (+0x22 + direction, i8/128 of the wider half
    width), both placed on the race road (0x5DA340, then 0x5EE9C0: a
    circuit's fraction from the start line round, 0x5D5D20; a sprint's
    from start to finish); a wrap of more than half a lap adds 1 to the
    lower. The rate is the fraction between them over the 1 m at the
    racing line's speed (+0x2C + direction), kept only on one road segment
    (the locator's +8) with the road distance (0x5D5CE0: a − b, a
    circuit's within half its length) 0.25–4 times the straight one, and
    goes to the bin of the 0 m sample if that bin is still empty. The next
    point is 0x5DD560 (type 0, links on: a type 2 or 0 junction of the
    other direction bit, bit 23 flipping the direction, else the record's
    next point; each a racer point); the walk ends at a dead end, after
    10,001 points, or once the points walked add up to 3/4 of the road
    and the last 1 m sample lies 0–200 m past the finish. An empty bin
    takes the bin before's rate (the first 40 m/s over the road's length,
    0x88F2B0); the start times add 1/256 ÷ rate. 0x5EDDA0: laps < 0 read
    0; time = whole laps × lap time + bin start + (fraction − bin/256) ÷
    rate. Checked by `reference_laps_walk_the_racing_lines` (4001: 168
    bins, 66.6 s for 6278 m). **Inferred**: the locator (0x5D9E40) as the
    road's nearest point.
  * Gap (0x5F82E0, ticks, row − player, negative when the row is ahead):
    both finished, their finishes (+0x198) apart; one finished, the other's
    reference time to the end (at the race's laps, +0x09) added to the race
    clock; neither, their reference times at their progress apart. Each
    reference difference is ×4000 + 0.5, truncated. Race type 7 uses
    (+0x24 difference) × 4000.
  * Position element (0x4EBB10, offline, not drift): RacePositionNumber
    `%d` the place, RacePositionNth the suffix (0x4FFAA0: 10 its own
    label, else 0x7F70D8[place % 10]: th st nd rd th…), NumberRacers `/%d`
    (less the knocked out, +0x28, in race types 3 and 7), RaceTime `%s`
    the race clock, held for 3 s from each frame racer status +0x0D is set
    (0x5F7C40 clears it each frame and sets it when the lap count +0x08
    passes the most laps done +0x09 at the finish checkpoint, 0xBF7);
    Hurry_Counter hides.
  * Leader board (0x4DA190 → 0x4C6F30 rows; one human): 10 s of gaps
    (0x4AA9F0), then 2 s of names (0x4AA890). The player's row runs
    SetPlayerColor and shows the name; the others run Init and show the gap
    as 0x577250 flags 10 (`-`/`+` over the leading space, a two-digit first
    field shifting right), or once done (0x5EE260) the status: 3 Knocked
    Out, 8 Blown Engine, 9 Totaled, 11 Timeup, 12 Disconnected, else
    Finished. LeaderText_n `%d :` the place. 0x4AA7A0 colours rows online
    only.
  * In HUD_SingleRace the position group (0x1DD0 at 213, −224, its texts
    +59 down) and the lap group (0xDCFE9 at 215.5, −158.25, its texts −3)
    meet at the same y, but the position texts are bottom aligned (`Sj`
    8, 0x51C110) and the lap texts top aligned: "1st /4" stands above
    "Laps 1 /2" (Front end, Layout).
* **Race over** (0x60B3B0 case 3): the post-race chain starts when every
  racer has finished (0x5F85C0) and the tick counter 0x7FB720 (60 a
  second, 0x5FFF94) is past the last finisher's finish tick (+0x50, set by
  0x5EA540) plus its hold (+0x54). **Verified** (0x5EA540): the hold is
  6.0 s (0x7A54A4) when racer +0x58 is 6 or 7, else 3.0 s (0x7A54A8) in
  race types other than 7 and 8, and in race type 7 only in mode 4
  (`[0x850078]` = 4); none in race type 7 otherwise or in type 8. The
  seconds are × 60 (0x787590) ticks. Racer +0x58 is set to 6 (at 0x604CFA)
  and 7 (at 0x604FEB) in 0x604750, whose body contains 0x604840 (**Inferred** as the car update; its role
  is not traced).
  0x5EA540 runs once, at the finish, on the player object whose +0x58 is
  the running camera event kind (see **Crash cameras and slow motion**):
  6 / 7 are the crash and jump slow-motion cameras, so a finish under one
  holds 6 s. Ported (2026-10-30): `Race::hold` takes 6 s
  (`RACE_OVER_EVENT`) when the player's `EventCameras::kind` is 6 or 7 on
  the first frame after the finish, else 3 s (`RACE_OVER`). **AI racers
  have no hold** (2026-10-31, race3; verified, the disassembly): the
  racer-ended handler 0x606CC0 calls 0x5EA540 only when the racer's car
  has a player object (car +0x18; 0x60704E jumps past the call for 0),
  so only human players get a finish tick +0x50 and hold +0x54; 0x5F85C0
  ("all finished") and 0x5EEF30 (the latest finish tick) walk the player
  list 0x8900B4 (count 0x8900BC), not the racers, so an AI racer's finish
  never delays the race over (offline: the player's own finish + hold).
  Online / split-screen (0x89CF69, 0x8669F4, 0x866A14) 0x5F85C0 asks
  0x5F0890 instead (not traced). Out of a race (race type 0x89E7B0 = 0)
  0x60B5A3 also needs 0x457BA0 on G (0x850078 = 4 and 0x83AAB4 = 8). The
  same routine plays the ambient event "CrossFinishLine" (0x578EA0) at
  (racers within 30 m − 1) × 7.5 + 10 for the race's first finisher
  (player +0x1C +9 equal to race status +0xBE8) — not ported.
  **After the finish** (traced 2026-10-28, **verified** unless marked):
  while the race runs on after the player's finish no NIS scene serves it
  and no camera changes: the director 0x5FB060 runs track cameras only past
  its gate (+0x234 is the playing NIS scene's id, written only by 0x437910
  and cleared by 0x439070; +0x24A is set only by 0x5FFF60 in race state 7
  online / split-screen; 0x82A3A4 is never written by code; replay
  *0x8900D8 = 5; 0x89E7B8 only from the demo set-up 0x540096), so the
  player's chase mover (type 1) stays. (An earlier note here said no NIS
  scene follows the race at all: **wrong** — the ending scene EndingNis01–04
  plays once the race is over, see **Race end** below.) No slow motion or fade is started
  by the finish (0x606CC0's param 3 path, +0x974 / +0x978 = 2 s, is not
  taken: the finish calls it with 0). What changes is who drives:
  0x606CC0 for the local player's racer (car +0x18 set) puts its input in
  state 0 (0x605C80(0): every driving key handler removed; 9 online or
  split-screen) and gives it controller type 3 (0x5EBA40(3) → 0x4260B0,
  the AI racer controller, with the CarDriver 0x425AB0; 0x425E40 instead
  when 0x890114, 0x89E7D9 or 0x890104 is set and neither 0x89E7D8 nor
  0x890108). Its skill row (vtable +0x78 → 0x610520, racer info +0x901)
  is the human's −2 (0x6373F0 with 0xFFFFFFFE at 0x526028 / 0x526B41 /
  0x5324CF): the target speed takes row max(−2, 0) = 0 (0x41C849) and no
  jitter (0x41C6E0); the driver's throttle cap is 1 (0x425B44, class 1).
  The post-race chain then waits for every racer and the hold (above);
  0x60AAB0 starts it with no fade of its own. Racer +0x58 = 6 / 7 (0x604750)
  are a stuck-car reset state (camera mode 0xF / 0x10), not the finish.
  **The stop** (**verified** 2026-11-01, small171): at the end of
  0x606CC0 (0x60711C), for every racer whose car runs controller type 3
  — the AI racers and the local player just given it — 0x42B330 →
  0x42AA20(11) replaces the controller's goal (+0x6FC) with goal type 11
  **"Park"** (ctor 0x40CB80, vtable 0x784968, name 0x7849B8). Its chooser
  (slot +0x38 0x402780 → +0x3C 0x427060) proposes one action, **"Stop
  driving"** (vtable 0x78591C, base ctor 0x4234A0 with type 16, priority
  600): its update 0x42F6E0 keeps building the line path (0x428CE0 →
  0x428FD0 / 0x42BF00, else 0x420D40) and sets the speed request +0x1F0
  = 0, +0x1F4 = +0x1F8 = 0, +0x1FC = 0 (no line speed), +0x1FD = 1
  (pedals on). The goal's speed slot +0x34 (0x4204F0 → 0x41C990, the
  racing goal's is 0x4204D0 → 0x41C5E0) takes the action's +0x1F0 and
  the least with the goal's slot +0x4C (0x401D30: 0.0), so the CarDriver
  gets target 0 (vt+0x38) and brakes to a stand. The same call runs for
  a Lap KO knockout (0x6079BF, status 3) and a drift time-out. Under Wine
  (finish151): the player's car from 21 m/s to 0 in ~1.7 s, rivals from
  26 m/s in ~2.5 s. Clone: `racer_think`'s `parked` (target 0, pedals on,
  no GetUnstuck) for every rival with a finish and the player's car past
  the line (`Autopilot` with `Race::me.finished`); `NFSU2_BENCH_SPEEDS=1`
  logs the speeds after the finish (circuit bench: 26.5 m/s → 0 in 2.2 s,
  ~12 m/s², the original's ~12.4).
  Clone: `Autopilot` (game.rs) drives the player's car with the
  rivals' AI (`racer_think`, `SpeedTarget::human`) from the frame after
  the finish; shot `game --shot finish` (NFSU2_SHOT_MS after the line);
  selftest checks it. In a drift race the driver is the DriftDriver
  (below; ported). **Open**: a test run's end.
* **Race end** (2026-10-30; **verified** unless marked; every race type
  of the race flow — circuit, sprint, street X, URL, lap KO, drift, drag):
  * The racers still running when the race is over (0x60B3B0 case 3,
    above) are ended where they are (0x60AAB0 → 0x608CB0): in race order
    (+0xBD0) each racer with status +0x10 clear gets 0x606CC0(i, 0, 0, 1)
    — status 1, the next finish order (+0x0C), +0x19C = 1 — and 0x5F7ED0
    estimates its time: the race clock plus the reference lap's time from
    the racer's progress (+0x20) to the leader's laps done (+0x09) when
    the leader (order [0]) finished for real (status 1, +0x19C clear),
    else clock / max(progress / laps, 0.01), rounded to 0.01 s (0x577410);
    the laps not done share what is left equally, a new best lap kept. In
    Lap KO (race type 3) with race status +0xBEC clear (**verified**
    2026-10-30: +0xBEC / +0xBF0 are written only by RaceCommand_Apply
    0x4A7B30 for a race command of kind 0 — +0xBEC = the command's +0xC,
    +0xBF0 its player — and cleared at the set-up 0x5FC7E2, in 0x6076E0
    at 0x60791C and here at 0x608D6E; the commands come through
    RaceCommand_Queue 0x578F00, whose callers 0x49AD95 / 0x4C32BF /
    0x4C3D5F push kinds 3, 2 and 1 only (0x578F00, ECX = the replay
    manager, logs each command at its loop in the manager's list +0x40EC
    while recording, up to 256, then runs it through 0x4A7B30; with the
    list full it calls 0x571840, which only stops the recording for good,
    manager +0x80 = 1: **verified** 2026-10-31), so offline it is always 0) the
    order is walked from the last place: the leader finishes, the
    others still running are knocked out (status 3). Results order:
    finishers, then the ended racers in race order, then drop-outs (latest
    first). Clone: `Progress::end_short`, `Race::end_unfinished`.
  * The ending scene: every race flow set-up sets flow +0x44 = 1 (0x60A878,
    on all its paths). Entering state 7 (0x603EC0 case 1: 0x4A46E0 pushes
    the post-race chain's first screen unless 0x4A4B60) with +0x44 set, no
    replay running (0x8650B4 +0x14 = 0) and not on a dyno run (0x89E7E6
    clear, or race type 8) clears it and sets +8 = 5 with event 4
    (0x5F41F0(4, 0)) → loading: PC_Loading (0x790734, 0x6039A6), the NIS
    manager flagged (0x430430 on 0x827E88, flow +0x45 = 1), and once the
    streamer is idle (0x5D6170) 0x437200 picks the scene (0x436840 case 1:
    NIS table +0x04 count, 8-byte records at +0x25C — hash, variant +4,
    flags +5; track masks at +0x91C, u16 at +10 via 0x4300F0; EndingNis01
    only when the results winner's tuned rear torque split is > 0.8
    (0x42FED0; see `docs/race-ending-selection.md`); 0x42FF50 requires
    0x40 and selects 0x80 by whether that winner is human; 0x42FF20 any scene with 1,600,000
    bytes free in the NIS pool, else EndingNis04; one drawn by 0x43C1C0;
    the dyno track 0x1262 takes DynoExit) and 0x43BAA0 → 0x43B8E0 →
    0x43B000 → 0x437910 starts it. Origin (0x43B000 type 1 → 0x431990):
    the marker 0x42FC60 gives with its flag clear — the start line on a
    circuit, the finish line point to point — at the ground, heading +180°.
    Camera: the scene's generic camera group, "Track %d" of the entry's
    variant. The post-race screens stand over the scene. **Observed** under
    Wine (Quick Race, Resort Loop): the race-over message for 3 s,
    PC_Loading, then Event Results over EndingNis04 (the crowd round the car
    on the start line); a minute on +0x234 still names the scene.
    **Inferred**: the car keys at two scene frames each (car in place
    within ~3 s). **Not ported**: the scene's second car (`ZPM_Car2`) and
    its event track. Clone: `play_race_ending`, `IntroTable::ending_candidates`,
    shot `ending` / `ending_hold`, the maintainer's probe `nis_endings`.
  * UI_PostRaceResults' time column (0x4E9B8B, the tick): +0x70 counts the
    frame time; past +0x74 +0x68 flips and +0x74 is 1.75 s (0x78DFEC) for
    "Time Behind" (0xE2F9334D) or 4.5 s (0x78DFF0) for "Finish Time"
    (0xC97A2D22); 0x4BF0A0 rewrites header 0xA4EC03BE and the rows
    (0x4A4110: the first "--:--.--" 0x61C9B7A2, the others the signed gap,
    0x576D40 flag 2). Not in drift races, online or on a dyno run.
    Clone: `results_behind`.
  * After UI_PostRace (2026-10-30): "Return to Explore Mode" (0x4F39A0,
    0x3411395C → 0x35E413DD → 0x4A3E30 → 0x54C9E0; a tournament left
    early 0x451E768E → 0x4A3DF0 → 0x513950) sets career +0x8744 and the city
    start puts the car at the place kept when the race was engaged (race
    dialog 0x499B27, World Map 0x4E1284) — see **Where the city start puts
    the car**; ported (`Action::ReturnToExplore`, `Profile::explore_set`).
    The 6 s hold: see **Race over** (ported). The outrun's end 0x541330
    (see Outrun, "The end") is ported for the bank, the wins and the
    messages / magazine triggers; its call 0x5110C0 is Rachel's result
    line (see **Rachel's outrun lines**, ported); 0x533320 and 0x4F92A0
    (marker table and HUD rebuild) are not ported as calls: the clone
    builds its markers and HUD lists from the career state every frame
    (**inferred** equivalent; the minimap's AI items need no rebuild: the
    encounter racers are in the world list's AI range from the free-roam
    set-up, see **Minimap items**, Outrun racers). Under Wine the original's GPS shows "Unable to
    contact satellite" after the car lot's drive-out too (seen once, 15 s
    after the drive-out was skipped with Enter;
    the same as the clone's selftest note).
* **Drift driver** (CarDriver_DriftCtor 0x425E40, vtable 0x784F40, 0x250
  bytes, "DriftDriver::DoSnapshot" at slot 2 0x405A80; **verified**, ported
  as `ai::driver::DriftDriver`). 0x4260B0 builds it instead of the
  PsychoDriver 0x425AB0 (0x42614A–0x4261B3) when 0x890114, the drift race
  flag 0x89E7D9 or 0x890104 is set and neither drag flag (0x89E7D8,
  0x890108). It drives every AI racer of a drift race (controller type
  3: 0x4260B0 tests param +0x24 = 3 before the drift / drag flags) and
  the player's car past the finish (0x606CC0 → 0x5EBA40(3)); ported for
  both (`Rival::drift`, `Autopilot`). (An earlier note here said drift
  races field no AI opponents: that was an inference and is **wrong** —
  see "Drift event, end to end".) The slots that
  differ from the PsychoDriver: 1 the frame 0x416120, 7 (0x405B10) +0x23C
  "hold", 14 (0x405B60) the target speed +0x22C, at most 31.29 m/s
  (0x7844BC), 29 (0x405BA0) reads it, 30 (0x405BB0) 2.0, 33 (0x405BD0) 0.
  The base 0x423330 sets the gear request +0x1E4 = 1. The frame (tuning
  in degrees at 0x783A5C–0x783AC4, ×0x784244 to radians; v = car +0x430):
  - Look-ahead (+0x224/+0x228): with a path (driver +0x10, count byte),
    d1 = |0x404AF0 cross-track| · 0.75 + 0.04·v + 15 held to [15, 30],
    P1 = 0x415630(d1); d2 = 0.08·v + d1 held to [8, 12], P2 = 0x415630(d2),
    path yaw = angle(P2 − P1); without a path 0x403AF0: 10 m along the
    heading. A = the aim's yaw (0x43E6D0, wrapped by 0x405940: x −
    floor(x/2π + ½)·6.28318), H = heading +0x40C.
  - Backing up: |wrap(H − A)| over 120° (90° while the gear request is 0)
    or a negative target.
  - +0x23C set: brake 1, gas 1, wheels to 0 at once (0x405990(0, 1.0)),
    gear 1.
  - Drifting (not backing up, v ≥ 10 mph = 4.4703): travel yaw V, slip S
    = 0x591DA0 (heading16 − velocity16, in (−π, π]), its rate R =
    wrap(S − +0x230)/dt (+0x230 = S); M = clamp(0.03·v + 45°, 45°, 70°);
    c1 = clamp(wrap(path yaw − V) · clamp(−0.1·v, 0, 1), ±35°) — always
    0 for v ≥ 0; c2 = clamp(wrap(A − V) · clamp(0.05·v, 0, 2), ±45°); the
    goal D = clamp(c1 + c2, ±M) (+0x238); err = wrap(S − D); Q = 1.5·err −
    0.02·R; wheels towards −Q (0x405990). e = err, negated for D < 0; q =
    Q, negated for err > 0. Handbrake (+0x210) = clamp01((v − 8.94)·0.11185)
    · clamp01((−20° − e)·2.8648) · clamp01((15° − q)·5.7296). Target
    under 2 m/s (and no hold): brake 1, gas 0; else gas =
    clamp01(clamp01((target − v + 2.235)·0.11185) − 0.9·clamp01(0.11185·v)
    ·(e − 20°)), brake 0. Gear −1.
  - Otherwise (slow or backing up): |target| under 2: brake 1, gas 0;
    else x = clamp(|target| − v, ±1): gas x or brake −x. Handbrake 0;
    gear 0 backing up, else −1; wheels towards wrap(A − H) (negated
    backing up).
  - 0x405990(angle, dt): the wheel angle +0x1E0 (16-bit) moves towards
    `angle` at most 360°/s (0x783A68), held to ±45°.
  - Then, with a drift flag and no drag flag, for a full-physics car (car
    +0x28 = 1): byte +0x74 of car +0x34 → +0x4C is set while the handbrake
    is over 0.8 (0x7850EC), else cleared — **not ported** (its reader is
    not traced).
* **Triggers**: events, shops and photo spots are route triggers
  (chunk 0x3414A of the region's route files, loaded by 0x5D9B40 into
  the route data 0x88F298: records +0x43C, bytes +0x438, the first record
  of each kind 0..0x14 at +0x440, 0x5D5660). A record: +0x00 kind (sorted
  by it), +0x04 x, +0x08 y, +0x14 z, +0x20 the ground box (min x, y; max
  x, y at +0x28), +0x30 its name hash, +0x40 (i16) point count, +0x42
  (i16) record size, +0x44 the polygon (x, y). 0x5D9C00 finds a record of
  a kind holding a point: in the box (0x43D410), then in the polygon by
  the even-odd rule (0x43D890); 0x5D9C80 walks a kind mask. By name:
  0x4FFC50, the first kind 13–15 record (mask 0xE000) with that hash.
  Events (marker +0x28, `EV_nn`) and photo spots (milestone +0x2C) are
  kind 13, shops (+0x3C) kind 14; `EV_01` is an 8 × 8 m box at (374,
  1167), z 35. Verified; +0x0C / +0x10 (1, 0 for EV_01) not traced. The
  shops are also world triggers (the track bundle's 0x37090 chunks: the
  load handler 0x43A460 → 0x43A040 appends each chunk's data (16-aligned)
  to the list at 0x827EF0 + 0x38 (0x479130), the unload 0x438C60 →
  0x438470 unlinks it; the world's event system, 0x43A880 → 0x43A4A0
  per record: +0x04 the event type (0x439EE0), +0x0C flags, +0x20 /
  +0x24 the name hash (both, copied to the instance; +0x20 = 0 with
  +0x3C set starts it in state 3), +0x34 an optional script,
  +0x38–0x3B an hour window against 0x89E7A0 % 100, +0x40 a 4 × 4
  matrix — rotation rows, position at +0x70 (also read by 0x43A902) —
  copied to the trigger object's transform +0x50 (0x4012B0); verified.
  +0x80 / +0x90 are a world-axis box around it (in the data the
  position's z to 6 m above; **open**: its reader. Not in 0x43A400–0x43AFFF
  (no +0x80 / +0x90 read there). Verified: none of the four callers of the
  +0x90 getter 0x4018B0 (`lea eax,[ecx+0x90]`) tests a trigger's box.
  0x5FA0BF and 0x5FA21D (in 0x5F9CE0) take the record 0x5F8FF0 returns (a
  walk of the list at [0x890080] by the +0x0A id through 0x5F0E40; line 444
  cites that walk for car 0, via 0x5ED990(0); the record's type is not
  traced) and read its +0x70 (0x4018A0, `lea eax,[ecx+0x70]`) and +0x90
  (0x4018B0) as x, y in dot products with a heading from 0x401B20
  (±1.388889; 1.9290124 = 1.388889²), with no compare against a box's min
  or max. 0x60F57B (in 0x60F560) and 0x61B346 (in 0x61B2F0) read the +0x90
  of the vehicle (param_1; its +0x554 holds the "Car Lights" block) as
  sign-flipped light offsets (multipliers 0x784250 = +1.0, 0x7842A0 = -1.0).
  The box's reader is still **open** (none of these four). The 23-of-24
  count of kind-14 route-trigger names is base text, not re-checked here.
  A shop is not engaged by its world trigger but by the route trigger of
  its name (verified, below): CC_BODYSHOP_1's is a 20 × 20 m box at
  (−385.9, 38.7), 14 m from its world trigger's (−381.1, 53.3).
  - **Marker table** (career 0x85AD40, 64 entries of 0x18 bytes: +0 the
    route trigger, +4 an event, +8 a shop, +0xC a milestone, +0x10 a world
    challenge, +0x14 a flag; 0x500EE0 walks the used ones, in a career
    only, `[0x850078]` = 1). 0x533320 builds it (after 0x527D40): for each
    kind 13–15 route trigger in order, the first shop naming it (+0x3C,
    0x4FFE60) when open (0x513110, flag 0) and the first milestone whose
    spot it is (+0x2C, 0x4FFFC0) when on offer (0x501150: its record in
    0x862840 in state 2, not done); then each open event (career +0x71C8)
    at the trigger of its marker (0x4FFC50, flag 1); then the world
    challenges (career +0x7148 + 0x80, 0x500910) by the name of their
    string (0x512A10). The minimap and the world map build their icons from
    the table (0x4EC950 hands each entry to 0x4EC8C0; 0x4E0440 and
    0x4EE320 take each passing 0x533120 or 0x501690; the icon code itself
    is not read), so the icons stand at the route triggers.
  - **Shop open** (0x513110; also the test 0x515360 makes of a part a
    shop unlocks): its stage (+0x9D) reached, then, for an announced shop
    (+0x51), its visit flag set (the table career +0x7AFC count, +0x7B00
    entries of 8 bytes, the shop's +0x38 hash and a flag byte at +4 that
    must be 1, 0x5003B0), then its requirement (0x512870). The debug
    switch 0x838441 opens every shop. 0x512FF0 sets the flag, creating the
    entry (0x512F10), when the car enters the shop's trigger with the stage
    reached; an announced shop seen for the first time is put on the table
    at once (0x500E20 → 0x4ECCF0). In Bayview's data 18 of 24 shops are
    announced; at stage 1 only the car lot, the garage and Megalow show
    until the others are found.
  - **The car's trigger** (0x5EB050, the player's per-frame update, in a
    career out of a race, 0x500B60): the first kind 13–15 route trigger
    holding the car (+0x60) that stands for a marker (0x527810: an open
    event whose marker it is, 0x527790 over all events in order; else the
    first shop naming it, 0x4FFE60; else the first milestone whose spot it
    is, 0x4FFFC0; else a world challenge, 0x512A10). When that differs
    from the last (player +0x9AC), 0x5EAF40 leaves the last (a world event
    by its name, 0x438E30 with 0x89CCEC; its marker cleared, 0x513050, and
    the HUD prompt removed, 0x4DB820) and 0x5EAFC0 enters the new (a world
    event with 0x89CF2C; a shop is visited, 0x512FF0; then, if 0x532A60
    finds the entry valid, the HUD prompt, 0x4CA020 → 0x4C9170 with a kind
    from 0x4ABF70 for events, 0x4ABFE0 for shops, 0x11 a milestone, 0x12 a
    challenge, and the current marker, 0x532B20: career +0x8700 the
    trigger, +0x871C the event, +0x8724 the shop, +0x8728 the milestone,
    +0x8720 the challenge). 0x532A60: while a world challenge runs (career
    +0xC08 = 0x85B948: the challenge record 0x533BF0 starts, from the
    challenge engage 0x541130 and the challenge states 0x533D00; it is
    **not** the D-Day intro, as this note said before) only a shop whose
    +0x38, or a milestone whose +0x20, is the challenge's goal (its +0xC);
    else an event open (0x5007E0), a
    shop open (0x513110), a milestone on offer (0x501150) or a challenge
    active (0x5135A0). For a shop 0x532B20 then runs 0x514FD0, which, when
    open and the running challenge's goal, engages at once (the
    challenge object career +0xC00: +0x11C = 2, the marker copied to
    +0x8704…, state 0x11). Verified. So during D-Day the car lot and the
    garage are ordinary shop markers engaged with the key; the clone,
    which had them engage at once, now does so.
  - **Engage** (the input action ENGAGE, code 0xBF: descriptor row 46 at
    0x7FE2D0 reads DIACTION 15, "Enter Race / Shop", Enter or F1; 0x602BE0):
    in a career with the car in a trigger (+0x9AC) whose current marker
    can be engaged (0x527900: the shop open, the event open, the milestone
    on offer or the challenge active) and no engage under way (career
    +0x7138, 0x5004C0; +0x1C, 0x5012C0, **not traced**), it sets +0x7138
    (0x5004B0), copies the marker to +0x8704… (0x500B00) and enters state
    0x11, whose handler (0x606740) opens 0x498D00's dialog:
    UI_EngageRaceDialog for an event, UI_EngageShopDialog for a shop,
    UI_EngageEventDialog for a milestone or challenge. Driving in only
    prompts. Codes 0xC0 WORLD_MAP (state 0x12, UI_InGame_WorldMap), 0xC1
    LEGEND (state 0x13, UI_Status_Master) and 0xC2 SMS (0x501480) follow
    the same gates (rows 47–49, DIACTION 16, 18, 17). Verified. For 0xC2
    (read at 0x602D52): a career offline (0x500B60), the +0x1C flag of
    0x85EF50 clear and no engage under way (0x5004C0), then 0x5004B0(1)
    and 0x501480; no trigger is needed.
  - **The prompt** (verified; the element 0x4F83D0 at HUD +0x58, built
    in the HUD constructor at 0x4F9005 with the HUD package, mask (0, 2):
    free roam). Its objects in HUD_SingleRace: +0x20 0x50910822 (the
    D-pad, 3rdperson_DPAD_PS2), +0x28 0xA206A0B4 and +0x2C 0x0A729B1B
    (Engage_DPAD_Highlight), +0x34 0x3B1B624A (the mode icon,
    Mode_Icon_Circuit), +0x38 0x4B9B8F5A (Engage_SMS_mail), +0x3C
    0x668C9CD6 (a string, 0x537A20), and in its list only 0x99F665F4
    (Engage_Backing) and 0xB996A84C (Engage_DPAD_Backing). Eight slots of
    0x14 bytes at +0x60 (+0 state, +1 kind, +4 the time, +8 the objects,
    +0x10 a value). 0x4C9170 queues a kind (a new slot in state 1);
    0x4DAB80 drops one (state 6, then 0). The update 0x4DA9A0 sets the
    D-pad's texture to 0xEA570722 and hides +0x28, +0x2C, +0x34, +0x38,
    +0x3C each frame; with its mask on it steps the slots (0x4C9030 →
    0x4C8AC0: state 2 → 3 after 1 s, 3 → 4 after 4 s, sounds 0x4AB9F0;
    then the kind's drawing: 0x11 0x4C8DD0, 0x13 0x4ABAF0, 0x14 0x4ABA30,
    else 0x4C8B80) and shows one slot in state 2 or 3, a pending one
    (0x4C90B0) taking over by priority (0x4ABF20). 0x4C8B80: states 2 and
    3 (and 4, which it sets back to 3) set the D-pad's texture to
    0x6FFF0767 and the mode icon's to 0x4ABE50(kind) (1 0xE9638D3E, 2
    0xAAAB31E9, 3 0x00119144, 4 0x2521E5EB, 5–6 0x91703AEF, 7 0x3F6495C5,
    8 0xBF5AC5A9, 9–10 0x33B5791A, 0xB 0x8244F164, 0xC 0xB8038AC0, 0xD
    0xB802F10E, 0xE 0xC56D3032, 0xF 0xDC99AAFE, 0x10 0x3F40E75F, 0x11
    0xD1D9D8A4, 0x12 0xB0C0DCB5, 0x13 0xF95DD41E), show the icon and the
    highlight +0x2C and start, unless running, "pulse" on the highlight,
    "Blink" on the icon and "Pulse" on the D-pad (the engine hashes script
    names upper case: PULSE 0x03826A28, BLINK 0x0280164F, the objects'
    own); state 6 hides them, the D-pad back to 0xEA570722, and starts
    "Init" on the three. The clone (`hud.rs` `Prompts`,
    `Game::prompt_kind`) runs the eight-slot queue, its timed steps and
    its sounds (markers162: see **Engage prompt queue and sounds**); it
    draws nothing for kind 0x11 (0x4C8DD0, not read).
  - **The SMS slot** (kind 0x13, **verified**): the SMS element (HUD
    +0x7C, 8 bytes {last count, the prompt element}, built at 0x4F913E
    in a career, not online) runs 0x4DB8C0 every HUD update: unread
    (0x514D30(1)) 0 → 0x4DAB80(0x13) (state 6 drawn, slot cleared) and
    last = 0; more than last → 0x4C9170(0x13, unread), last = unread.
    0x4C9170 on a slot of that kind: from state 4 back to 2 (time, value,
    sound 0x51DE40(0xB, 0xF)); in 1–3 nothing; no slot → a new one in
    state 1. Steps (0x4C8AC0): 2 → 3 after 1 s (held while the D-Day
    script runs, 0x502420; the clone holds it too), 3 → 4 4 s after state 2 began, sounds
    0x4AB9F0(3 / 4); state 4 stays. 0x4DA9A0 picks the pending slot by
    priority (0x4ABF20: markers 4, encounter 9/10 3, SMS 2, 0x14 1): a
    higher one takes over a flashing SMS slot, which goes to 4 (rest);
    with none active a queued slot goes to 2. Drawing (0x4ABAF0):
    states 2–3 the D-pad 0x6FFF0767, the mail (+0x38) the icon of the
    newest unread entry's kind (0x4974A0; the loop stops above entry 0,
    so entry 0 is never looked at, and none → the envelope), mail and
    highlight +0x28 shown, count and mode icon hidden, "animation" on the
    mail, "Hide" on the count, "pulse" on the highlight, "Pulse" on the
    D-pad; state 4 the mail 0x4B9B8F5A, the D-pad 0xEA570722, mail,
    count and highlight shown with "static", "Init" on the D-pad, the
    count's text printf'd from 0x783FE5, an empty string; state 6 the
    D-pad back, the highlight hidden, "Hide" on mail and count, "Init"
    on the D-pad. **Open**: the icon's colour (0x535580 on the mail's
    "animation" script: it rebases a script track's colour keys by the
    difference between the given colour and the track's current one,
    +0x18..+0x24, after 0x519D40 / 0x52BDD0 by the track's +2 kind — the
    keys' layout is not traced, so the clone does not apply it). The
    draw order and the sounds are traced (markers162, **Engage prompt
    queue and sounds**): the slots draw in slot order, and the SMS slot
    plays FE ids 16 / 15.
    Checked by the self-test (queued under a shop's prompt, flashing,
    resting) and the `sms` / `sms_rest` shots.
  - **Not in the clone**: the engage event dialog for a milestone (the
    clone takes the photo shoot on Engage) and the world events. The world
    challenges: see **World challenges** below.
  The clone follows all of the above in `Game::marker_target`,
  `marker_valid`, `enter_marker` and `engage`, with `Profile::visited`,
  `shop_open` and `visit_shop`. Checked in
  `triggers_stand_for_shops_and_events` and by the self-test, which finds
  the body and graphics shops by driving in and engages with the key.
* **Messages / SMS** (`frontend/sms.rs`; **verified** except where
  marked). Table 0x34A17 (0x14 each, 167 records, 0x838454 / count
  0x838450, 0x5012F0): +0x00 name (string pool; labels `<name>_SUBJECT`
  and `<name>_BODY`, except names starting "SMS_CAR_UNLOCK" → SMS_CAR_UNLOCK_1's
  and "ENGAGE_TIPS_AUT" → ENGAGE_TIPS_AUTO_BODY, 0x5149C0 / 0x514B00 —
  the compares are strncmp of strlen − 1), +0x02 condition, +0x03
  mailbox (1 Inbox 0xC06E138A, 2 Game Tips 0xF8CB19A3, 3 Special Events
  0x06F8E5BF, 4 Rachel 0xE2442059, 5 Unlocks 0xEA33E5B6; table
  0x7F4438), +0x04 urgent, +0x05 icon kind, +0x08 / +0x0C values, +0x10
  sender label ("Rachel" 0x770AF86E…). Conditions and their senders:
  1 an event won whose hash is +0x08 or +0x0C, and 9 / 10 / 12 the
  sponsor (4 slots, +0x8C) / URL (10, +0x9C) / normal (0x5004D0) wins of
  stage record +0x08 >> 16 reaching +0x08 & 0xFFFF (0x501370) — all from
  0x529930 in the win routine 0x541430 (after 0x541690 filed the win,
  0x513390, before the stage end 0x532C70; the D-Day shop goals too,
  0x54CB00); 2 a shop entered whose +0x38 is +0x08 / +0x0C and 8 any
  shop (0x54CB00 → 0x529870); 3 free-roam driving time (career +0x8764,
  saved; 0x551330 adds the frame time in a career's free roam, race type
  0x89E7B0 = 0, race state 4) past +0x08 seconds (0x529760); 5 / 6 an
  outrun won / lost (0x541330 → 0x529BE0). Each arrives once (0x514CB0).
  **Inbox** (career +0x5D20 = 0x860A60): 256 entries of 0x14 (+0 the
  message, +4 read, +5 urgent shown, +6 deleted, +7 filed, +8/+0xC/+0x10
  a custom body / extra / subject from the magazine triggers),
  +0x1400 count, +0x1404 unread (add +1, read −1 once, delete −1 if
  unread; 0x514E80 recounts on load), +0x140C new flag. Add 0x529600 /
  0x529680, read 0x514DC0, delete 0x5014C0 (read and deleted), file
  0x514D80 (every read entry +7 = 1; on leaving the SMS screens,
  0x4977C0, or Back from a message opened alone). Saved with the career
  (0x5014F0 from 0x5146A4: name hash and the four flags; loaded by
  0x514E80, the hash looked up by 0x501310). Urgent: 0x529760 opens the
  SMS screens (0x529710 → state 0xD) when an urgent message's entry is
  not yet shown (+5), in free roam (race state 4, mode 6, no shop visit
  0x863444 / 0x86344C, career, offline), then marks them shown (0x514C70).
  **Keys**: Read SMS (DIACTION 17, Tab / F3, code 0xC2; 0x602BE0 → in a
  career's free roam, 0x500B60, and no engage under way) → 0x501480 →
  state 0xD → 0x606740 → 0x4976B0: exactly one unread → UI_EngageMessageDialog
  on it (0x514CF0), else UI_SMS_Mailbox on the Inbox.
  **UI_SMS_Mailbox** (0x4E1BF0, handler 0x4CCC10, refresh 0x4CC900): the
  mailbox's rows newest first, deleted ones never, an unfiled entry only
  in the Inbox, a filed one in its +0x03 mailbox; MESSAGE_TEXT_1..10 =
  "Subject: %s" (0x4B5DE3E9, 0x497760), ENGAGE_PHONE_ICON_n the kind's
  colour (0x4974A0 → 0x51D870) with the closed envelope 0x4B9B8F5A or,
  read, the open 0xEE28AC57 (0x4B2040), HIGH_LIGHT_n "Highlight" on the
  selected row; UP_ARROW / DOWN_ARROW by the selection (0x4B2190:
  none without rows, first → down only, last → up only); "Delete
  Message" 0x31CF7EC2 / "Read Message" 0x7D8C9EE7 "Show" with rows,
  "Hide" without. Keys: 0x5073EF13 / 0x9120409E and 0xD9FEEC59 /
  0xB5971BF1 the mailbox left / right (stopping at the ends, 0x4B2110),
  0x72619778 / 0x911C0A4B the rows (0x4CCB40), Accept → the message
  (0x497810), BUTTON1 (0xC519BFC0) → "Are you sure you want to delete
  this message?" (0xF0A6E6B5; Yes 0x01F23C75 → 0x4CCB10, an emptied
  list closes), Back (0x911AB364) → the game (0x4977C0).
  **UI_EngageMessageDialog** (0x497850, handler 0x497B00): reads the
  entry, "Subject: %s" in 0x2C167533, "From: %s" (0x9F29D70D) of +0x10
  in 0xFECED617, the body in Body_Text_1..10 (0x50DBB0, broken at the
  row width) scrolled by SideScrollBar; Accept (0x406415E3) → the game,
  Back → the mailbox (opened alone: files first and opens the message's
  own mailbox). Icons by kind (0x4974A0; FEng colour B, G, R, A): 1
  0x88C9990A (45, 200, 200), 2 0x8B2F36EE, 3 0x470222FD, 4 0xA9DFE648, 5
  0x5D9B6555, 6 0x756B5C1B, 7 0x55476A96, else 0x4B9B8F5A (14, 156, 14).
  **HUD**: see the engage prompt's SMS slot (kind 0x13) above. The clone
  saves the profile on leaving the SMS screens.
  The intro's messages (SMS_INSTRUCTION from the D-Day script,
  SMS_LOC_GO_TO_GARAGE and SMS_LOC_EVENTS_OUT_THERE from Rachel's hints)
  and the script's hold on the driving time: see **D-Day script** and
  **Rachel's hints** (verified, ported).
  **Magazine / unique-part triggers** (**verified**; `sms::MagTrigger`,
  `Profile::fire_triggers`): table 0x34A1B (0x18 each, 28 records,
  0x838424 / count 0x838420): +0x04 (i16) stage, +0x06 condition, +0x07
  count, +0x08 the message (0x501310), +0x0C a milestone (0x34A16 +0x20,
  0x4FFF80), +0x14 kind (1 visual, 2 performance, 4 a magazine), +0x15
  the visual part's slot. 0x533B50 (the last call of the win routine
  0x541430, after the stage end 0x532C70) fires each record of condition
  0, stage = 0x862838, whose count equals the stage's normal wins
  (0x5004D0); 0x533AA0(won) (from the outrun end 0x541330, after
  0x529BE0) on a win those of condition 2 whose count equals the stage
  record's outrun wins (+0xDC, 0x8620F4 + 0x104·stage). Both call
  0x5338D0 and, for kinds 3 / 4, 0x525400 with the milestone (Rachel's
  hint kind 0xE / 0xD: speech only, see **Rachel's hints**). 0x5338D0
  adds the message through 0x529680 (once, 0x514CB0) with the entry's own
  labels: a milestone found → body SMS_MAG_GENERIC_BODY_%d, subject
  SMS_MAG_GENERIC_SUBJ_%d (counter 0x864210 % 2 + 1, then +1), argument
  `<cover>_TITLE` (0x43DB80 continues milestone +0x28's hash); else kind 2
  → SMS_PERF_GENERIC_%d_BODY / _SUBJECT, kind 1 →
  SMS_VISUAL_GENERIC_%d_BODY / _SUBJECT with the slot's label (0x0B Hoods
  0x028F7092, 0x0C Spoilers 0xBB034EA6, 0x0D Rims 0x0294D2A3, 0x16 Wide
  Body Kits 0x945E9BC5, 0x3E Vinyls 0x78DEEA1C), by counter 0x86420C % 3
  (perf) or % 5 (visual) + 1, which then steps; other kinds the
  message's own labels. Labels are bin hashes (0x50B990 → 0x43DB50). The
  entry's +0x08 body / +0x10 subject / +0x0C argument replace the
  message's labels in 0x514B00 / 0x5149C0: snprintf (0x50C960) of the
  format label with the argument label, each "FENG: Default string
  error." (0xC01A6F63) when missing; e.g. "Unique Hoods". The counters
  and the three labels are saved in the clone (`smscounters`, the `sms`
  line's last three numbers). The 28 messages are condition 13, which no
  scan reads (only 0x5338D0 names them).
  **Other conditions** (**verified**): 4 (SMS_OUTRUN_INTRO) see the D-Day
  script; 7 (0x529A50, from 0x532B20, the engage of a marker, with its
  hash): a message whose +0x08 or +0x0C is the hash and whose milestone
  spot (0x34A16 +0x2C) is in state 2, not done — no message of the table
  has condition 7, so the clone has nothing to send; 14 (0x5299C0, from
  0x5331D0 when a stage DVD is offered anew, no state record yet): the
  message whose +0x08 is ftol(milestone +0x3C), the cover's rating
  (SMS_DVD_COVER_1..10; `Profile::offer_covers`); 15 (engage tips,
  ENGAGE_TIPS_*): the player's update 0x5EB050 (career, not racing, no
  world challenge 0x501690) every frame takes the first kind-0x12 route
  trigger holding the car (0x5D9C00(pos, 0x12, 0)) to 0x532D50 →
  0x529C50: the condition-15 message whose +0x08 is the trigger's hash,
  if not in the inbox (inbox +0x1410, compared with the trigger, is only
  ever cleared, 0x532901), arrives, and its +0x0C (25 or 50) goes to the
  bank (career +0x7134); 0x528990 then lists the tip triggers still
  unsent at career +0x600 (its reader is **not traced**). Clone:
  `RouteTriggers::tip_at`, `Profile::sms_tip`.
  **Body formatter** 0x512530 (**verified**): with 0x8383DC clear it widens
  the bytes to UTF-16 (0x43E000), else maps them through the double-byte
  table (0x50D3C0, `_` for unknown); the text is otherwise unchanged.
  **Mailbox scroll bar** (**verified** but for the arguments): the
  constructor 0x4E1BF0 makes "scrollerina_area" / SideScrollBar (rows) and
  "scroll_area" / ScrollBar (0x542CB0 → 0x542AD0 with byte 0 clear: along
  x). 0x538090(visible, total, pos) hides handle and arrows when total ≤
  visible or pos = 0, else 0x52E330 puts the handle's centre at the
  backing's start + h/2 for pos 1, its end − h/2 for the last, and start
  + h/2 + (L − h)/max(total − visible, 1)·(pos − 1) between. The
  scroller's call (1, 5 mailboxes, position from 1) is **inferred** from
  the other scrollers' (0x538993, 0x5444D6). 0x4CC900 runs "Hide" /
  "Show" on 0x54049A3D (Hide on the Inbox) and 0xD506A970 (Hide on
  Unlocks); neither object is in the package. `screens::scroll_bar_at`.
  **Sound** (**verified**): 0x529710 (the screens opened by the game: an
  urgent message, the D-Day instruction) plays 0x838119 = table 0x837FE0
  cell 27·11 + 16 (FOCUS on SMS_POPUP): sound 13.
  **Open**: the gates 0x5012C0 (0x85EF50 +0x1C) and 0x8900AC +0x234; the
  objects 0x54049A3D / 0xD506A970 (not in the package); the mailbox title
  0x980822B3 as the scroller's label is **inferred**; the reader of the
  tip list career +0x600. (FE_SOUND_ENGAGE_SMS is control 0xF of the
  sound table, the SMS slot's flash, FE id 16: see **Engage prompt queue
  and sounds**.) (The magazine offers come
  from a world challenge's start, 0x533BF0, ported: see **World
  challenges**; 0x525400 is speech only, see **Rachel's hints**.)
* **World challenges** (**verified** unless marked; `sms::MagTrigger`,
  `Profile::challenges_on_offer`, `Game::start_challenge` …,
  `screens::ChallengeDialog`; selftest `selftest_challenge`, shots
  `challenge`, `challenge_retry`, `challenge_hud`). The 28 records of
  0x34A1B are the world challenges as well as the magazine / unique-part
  messages: +0x00 the name (string pool; its bin hash keys the career's
  flag table career +0x7AFC/+0x7B00 = 0x86283C/0x862840, the table the
  shops and milestones use: +6 = 2 started, 1 done), +0x02 the start
  marker (a route trigger's name, kind 13–15), +0x10 the time limit (s),
  +0x14 the kind (1 visual, 2 performance, 4 the magazine shows; 3 none
  in the data), +0x15..+0x17 the part slots, +0x0C the goal: a shop's
  +0x38 (the unique parts: CC_BODYSHOP_1, CC_GRAPHICS, CC_PERF_SHOP) or a
  milestone's +0x20 (the 18 SHOW_n_PICTURE magazines). `examples/
  challenges.rs` lists them (e.g. STAGE_2_SHOW_1: 4 normal wins, 120 s,
  STAGE_2_SHOW_1_PICTURE; STAGE_2_VISUAL_1: 4 outrun wins, 60 s, Hoods).
  - **On offer** (0x527D40 → 0x527980, list career +0x7148, 32 slots,
    0x500910 counts): rebuilt with the open events; none while one runs
    (career +0xC08) or in stage 0; each record of the current stage not
    done whose condition's count reaches +0x07 (0 normal wins 0x5004D0,
    1 URL wins 0x500520, 2 outrun wins stage record +0xDC, 3 sponsor wins
    0x5005A0). The message comes when the count equals +0x07 (above),
    so the challenge is on offer from its message on. **Open**:
    0x527980's skip of STAGE_5_VISUAL_5 (0xDD15316E) when 0x86347D and
    the car record's +0x88A are set (not ported), 0x527D40's early
    returns on career +0x873F / +0x8738 / +0x873E, stage 5's 0x4CCE3E1
    and +0x78EC (not ported).
  - **Markers** (0x533320): the challenges on offer stand at their start
    markers (entry +0x10, type 15 0x500DE0: legend group 0xD, the
    showcase icon; marker effect XEE_WORLD_CHALLENGE). While one runs the
    table holds its goal alone (0x515060 → 0x500EA0: the shop by 0x4FFE20
    at its +0x3C trigger, else the milestone's spot +0x2C), and the open
    events list is empty (0x527D40 returns): no event, shop or other
    marker; 0x532A60 accepts only the goal. 0x527D40 copies the list
    aside and clears it (0x527D4B–0x527D84) before that return
    (0x527DA3), so the copy is lost: the rebuild after the challenge
    (its end 0x541970 → 0x533690 → 0x533320, 0x5419D0 → 0x5419B7 /
    0x541A0E, the give-up 0x533D00 state 7 at 0x533D17) draws every slot
    anew (Wine, the original's career at stage 2: 16 → 0 at the Accept, 16 with new draws
    after the shoot). A drop (0x5139C0 / 0x513B30) clears +0xC08 without
    a rebuild; the next city start fills the list. The clone: the
    profile's `challenge_running` (not saved), `Game::start_challenge` /
    `challenge_over`; test `tests::career_stages_1_2`. The minimap hides the AI cars
    in a career while one runs (0x4CA72E); 0x4CA6D0's marker test passes
    all with the GPS off and a challenge running. The engage tips
    (0x5EB0F8) wait while one runs.
  - **Engage** at the start marker: prompt kind 0x12; the key (0x527900:
    0x5135A0 finds it in the list) → state 0x11 → 0x498D00 →
    UI_EngageEventDialog (0x863454 set), set up by 0x4991B0: message
    0xDE075B62 = 0x515160 (ENGAGE_VISUAL_GENERIC_%d_BODY / PERF 1–3 /
    SHOWCASE 1–5 by 0x43C1C0, filled with ftol(+0x10 × 4000 + 0.5) ticks
    through 0x577250 flags 0, e.g. " 2:00.00"), subject 0x2C167533 =
    0x515250 (kind 1 0x48157D57, 2 0xA517FD7A, 3 0xBF178F39, 4 0xE5B2C887
    "Subject: Magazine Cover Opportunity"). Handler 0x498F00: Accept
    (0xD72F002A) → 0x541130 (GPS off: 0x4131F0, 0x838590 = 0) →
    0x533BF0, then 0x5F4250(1); Cancel (0x63857DE0) / Back → the game.
  - **Start** 0x533BF0 (on career +0xC00 = 0x85B940): the prompt removed
    (0x4DB820), +0x118 = the car's heading, +0 (failed) = 0, +8 = the
    record, +0x11C = 1, the name's flag +6 = 2, +4 = (float)+0x10, the
    goal milestone's state +4 = 2 (**the magazine offer**), markers
    rebuilt (0x533320, 0x4F92A0).
  - **Clock** 0x5016A0 (from 0x551330 when career +0x713C is 0 or 9,
    **not traced**): in state 1, +4 −= the frame time; ≤ 0 → 0x501600:
    +0 = 1, state 6, flow event 0x11 → 0x498D00, which with +0 set opens
    UI_EngageShopDialog (0x4998E0: message 0xC4DF3B44, Accept 0x039DF715
    "Retry", empty title). Its handler 0x499690: Accept → 0x5150D0
    (state 8, the car put at the start trigger's +4/+8/+0x14 facing
    +0x118, 0x5FBD70; flow 8/0xD); Cancel / Back → 0x501640 (state 7,
    the goal milestone's +4 = 0). The dialog's destructor 0x4993E0 →
    0x533D00: state 8 → 0x533BF0 again; state 7 → +8 = 0, markers
    rebuilt (the challenge stays on offer).
  - **HUD**: 0x6000A0 adds high 0x4 in state 1 (0x6004BD): element
    0x4F86D0 (Show_case_backing, CurrentLapTimeText_2, CurrentLapTime_2;
    update 0x4C9210) prints +4 > 0 as ftol(t × 4000 + 0.5) ticks
    (0x577250 flags 4) on CurrentLapTime_2, running Animate_RED under
    10 s, else Animate, unless running (0x51D050 / 0x51CF80). The bank
    (0x4A9EF0) hides Cash_Bank, Cash_data and Cash_header in state 1.
  - **Goal**: a milestone — its spot entered sets state 3 (0x532B20;
    the clock stops), the Engage key's UI_EngageEventDialog (milestone)
    → 0x498E70: a kind-3/4 milestone (0x5011A0) at once 0x541940 (reward
    0x541220: bank += milestone +0x26 + ftol(stage def +4 × the visual
    rating 0x528420 + 0.005); 0x4AC170 the HUD's cash popup; 0x5410A0
    files it in stage record +0xC4 and checks the stage end 0x532C70;
    the cover at 0x837FD8), 0x541970 (state 3 → 5, the name's flag +6 =
    1, state 0, +8 = 0), 0x533690, UI_MagazineReward; kind 1/2 the
    showcase UI_ShowCase_Preview, whose constructor 0x4CE770 makes the
    same calls. A shop — entered (0x532B20 → 0x514FD0, open): state 2,
    +0 = 0, flow 0x11 → UI_EngageShopDialog; its Accept → 0x54CB00: state
    4 and 0x533D30 lists the slots' unique parts (+0x0C..), then the
    unique-part screen (constructor 0x553210, vtable 0x79CAA8, handler
    0x561730): a part bought (0xD05FC3A3) or Back confirmed (0x20D0D219
    "If you don't choose a unique…", Yes 0x34DC1BEC) → 0x5419D0 (done
    as 0x541970).
  - **Clone**: all of the above (the unique-part screen, the showcase and
    the cash popup: see **UI_ChooseUniquePart**, **UI_ShowCase_Preview**,
    **Photo-shoot cash popup**) but: the milestone's engage dialog is the
    clone's OK dialog, and the reward is paid as it opens; 0x5410A0's
    stage list is not kept separately (the clone's `milestones` set); the
    challenge is not saved (career +0xC08 is a pointer; 0x514530 does
    not write +0xC00..; **verified**). Leaving free roam drops it: the
    quit to the main menu (0x4A3E80, the pause's OK 0x4A7CF0 → 0x5139C0)
    and Jump to Garage / the pause's Back to World Map (0x4A7C70 /
    0x4A7CC0 → 0x513B30) withdraw the goal milestone's offer (0x5003B0 →
    +4 = 0) and clear +0xC08 and +0xD1C (**verified** under Wine, variants
    round 2026-10-31: STAGE_2_SHOW_1 taken, 0x85B948 set and 0x85BA5C = 1;
    after the pause's Quit to Main Menu both 0). The clone did not, so a
    challenge taken before a quit or a profile load ran on with a goal the
    loaded profile no longer offered (playthrough seed 10); now
    `Game::drop_challenge`.
* **Sponsors** (0x34A19, 0x10 each): +0x00 name (string pool), +0x02
  sponsor-race purse, +0x04..0x07 required race types, +0x08 hash, +0x0C
  signing bonus. UI_Sponsorship_new (vtable 0x7975B4, built at 0x4DF736) is
  a SideScroll, not the icon strip: its five option templates sit off
  screen (y 299..376) and the refresh (0x495D30) puts DECAL_ICON_<name> on
  the one slot 0xB04669E3, SPONSOR_<name> in the title, +0x0C, the stage's
  URL purse (0x500A40) and +0x02 in the three numbers, SPONSOR_INFO_<name>
  under the required races, and SPONSOR_DETAILS_<name> into the text list
  TERMS_1..TERMS_10 (0x544700: lines broken by 0x520080 at the row width,
  `^` breaks, empty lines kept), scrolled by its own bar. A stage with
  sponsors offers them after a race (UI_SponsorPopup, then this screen;
  0x4A4610): from stage 2, with none signed (stage record +0xE8), a
  sponsor is offered once the stage's normal wins reach its count (stage
  def +0x1C, one i16 a sponsor: 1 from stage 2) and the stage's average
  reputation points a win (record +0xE0 / +0xE4, added by 0x513430 on
  each win but the D-Day pair) reach its +0x0E (0, 75, 150, 200, 250 per
  stage; 0x500610, 0x500670). The screen lists the offered ones
  (0x500670 counts the sponsors passing both tests; the screen, 0x4C1596,
  then lists that many from the stage's first, 0x5006D0 by index — the
  clone does the same). Sign adds the bonus; Back
  leaves the offer for the next race. Test
  `sponsors_are_offered_by_wins_and_average_points`.
  Verified by screenshot against the package.
* **First car, World Map, margin** (**verified**): the first car comes
  from the D-Day intro's car lot (see Career below; no free pick, the
  intro's events are won by its shop goals). The winning margin
  (0x513FE0): one race takes the results' first and second entries
  (+0xD08 / +0xD0C, times +0xC × 0.00025 s) as 1 − first / second, drift
  (kind 5) the scores as first / second − 1, more races the best points
  over the next best − 1 (0x513EB0); with fewer than two entries (no
  opponents) or a zero runner-up it is 100%, as also when the object's
  first byte is set. The object is the results record at career +0xD24
  (0x85BA64): the margin is called on it from 0x5330E0 (via 0x4A4250,
  `mov ecx,0x85ba64`) and from 0x541430 (`mov ecx,ebx`; ebx = career
  +0xD24, set at 0x54143D). The win routine 0x541690 (career at 0x85AD40
  as ECX) sets that byte with `mov [esi+0xD24],1` at 0x5416D6, after
  0x541170 and before the win handler 0x541430. Only the pending
  dispatcher 0x541800 calls 0x541690 (its ECX is the career object, passed
  by the career tick 0x551330's tail jump at 0x55142B), on DAT_00838448 or
  DAT_0083844C. Both are zero in .data's zero-filled tail (raw data ends at
  0x81D000); their only references are the dispatcher's compares and clears
  (0x54181A, 0x541829, 0x54182F, 0x541931), so 0x541690 does not run and
  its setter is dead. 0x532EB0 clears the byte: 0x541170 calls it at
  0x5411A0 with ECX = career +0xD24 (loaded at 0x54119A). (**open**: whether
  another writer sets the byte. Not traced: 0x528690, which 0x532790 calls
  at 0x532796 with the same pointer, and profile copies (0x52A470). So
  "only setter" and "100% unreachable" are not established.)
### Options (UI_OptionsMain → UI_Options; verified except where noted)

* **Main menu** (0x4B6230 adds the options, each handler 0x49A620… sets
  the category 0x83AA04): Audio 0, Display 1, Gameplay 2, Player 3 push
  UI_Options.fng; Controls 4 UI_Options_PC_Controller.fng, EA GAMES TRAX 5
  EA_Trax_Jukebox.fng, Credits 8 Credits.fng (0x4B6080). The clone builds
  the four UI_Options categories and Controls, EA Trax and Credits (below).
* **Settings block** = profile +0x24 (0x83AA04 for profile 0; profiles
  0xAB4C apart from 0x83A9E0, built by 0x5344B0 → 0x5116E0, the defaults):
  +0x14 rearview 1, +0x16 metric = language ≠ 0 (0x512360), +0x17 race map
  2, +0x18 explore map 2, +0x1C jump camera 127.5, volumes +0x24.. 1.0
  except menu music (+0x2C) 0.7, +0x4C audio mode 1 (stereo); per player
  (+0x60 + p × 0x28): gauges, position, lap info, score, feedback,
  leaderboard, ask-transmission all 1, camera (+0x10) 3 (Far), split
  (+0x14) 0, transmission (+0x15) 0 (auto), stability (+0x16) 1 (reads
  "Off"). Career difficulty (profile +0x250) and save prompts / autosave
  (0x844AD8 / 9) are untouched → 0. Display: resolution index 0x870D1C
  (reset 0 by 0x5CECA0), detail 0x87099C (0x5CDE50: trunc(0.2 × the card's score
  from the 120-row table 0x8005C0, 1 when unlisted, + 0.8 × the CPU tier,
  under 1400 / 1800 / 2400 MHz 1 / 2 / 3, else 4); 0 on XGI, Intel and the
  ATI 0x5834, at most 3 on the NVIDIA 0x0322 / 0x0312, 0 on the SiS 0x6330;
  `options::estimate_detail`, with the clone reading the OS's CPU clock
  where the game times it, 0x5CDD00). The clone saves
  them in options.txt (the game saves them in the
  profile). **First run** (verified, audit A-frontend-1): no
  "NotFirstTime" → 0x5CED70 calls 0x5CECA0 (Default Settings): the
  estimate, then 0x5CED44 → 0x5BE880 (Customize Display follows the
  detail, Vsync 0) and the caps clamp 0x5BE6A0; kept by the registry
  save 0x5BEEB0. Clone: `Settings::first_run_display` on a windowed run
  with no options.txt, saved at once (the display mode stays the
  clone's). Detail 0 (Intel) is a stored value: the slider (0x4FF550)
  shows it as 1 without writing it, Left there is "not changed", Right
  gives 2; the registry reload 0x5BEA30 keeps 0. At an end stop
  (Right at 5, Left at 1) 0x4FF480 reports "not changed" and 0x49B752
  skips the write and 0x5BEA00 (Customize Display kept).
* **Rows** (builders 0x4B6E60 / 0x4B7180 / 0x4B7310 / 0x4B7CD0; item text
  = vtable slot 3, change = slot 1 on 0x9120409E left / 0xB5971BF1 right):
  Audio: 7 volume sliders (step 0.05, 0–1) + Audio Mode; Display:
  Resolution (next available mode, no wrap; "640x480".."1600x1200"; the
  clone's longer list: Display modes and aspect),
  Level Of Detail (slider 1–5 step 1), Customize Display Settings and
  Default Settings (buttons; the latter asks "Reset to default
  settings?"); Gameplay: Jump Camera, Rearview Mirror, Speedometer Units,
  Career Difficulty (wrap 0–2), Explore / Race Map Mode (Off/Static/Rotate,
  wrap), Save (Autosave/Prompt/Off; 0x49B170 order, Autosave only with a
  profile, 0x502FE0); Player: Transmission, Stability Control, Camera
  (wrap 0–3), Gauges, Race Position, Lap Info, Split Time (wrap 0–4),
  Score, Leaderboard, Wheel Options (button), Feedback (disabled without a
  force-feedback device, 0x5B8250), Ask for Transmission.
* **Layout** (list 0x552790, 8 rows shown on UI_Options): rows step by
  TITLE_MASTER's height from its top-left (0x51FF90 / 0x54EF40); name
  top-left on that column; value centred on DATA_MASTER (0x52D810); arrows
  just outside DATA_MASTER (0x52DAD0), texture 0x9943D584; backing group at
  name + (276, 10); sliders (0x52DF00 / 0x540D30): base centred on
  DATA_MASTER, track = width − 2 × 46, knob and handle by the value's
  fraction. Selection clamps and skips disabled rows (0x5390F0); selected
  row HIGHLIGHT, others UNHIGHLIGHT, disabled DISABLE, arrow BLINK on a
  change (0x51E6C0). A move (0x50DB10) runs UnHighlight (vt+0x28) on the
  old row and Highlight (vt+0x24) on the new row only, nothing when the row
  is unchanged; the other rows keep the end state of the scripts they got
  when the list was built, and scrolling (0x52FDC0) only moves the items'
  objects. The clone times every other row's script from the screen's
  opening (`slider_list::draw_rows_n`, test
  `a_move_restarts_only_the_old_and_new_rows`). Back from Gameplay asks "turn off save prompts?" /
  "turn on Autosave?" when those changed (Yes leaves, No stays, 0x49C690).
* **Effects wired in the clone**: units (HUD 0x4C7159: × 3.6 "KM/H" or
  × 2.237 "MPH"; race stats "km/h"/"mph"), the HUD masks (0x6000A0 with
  0x5EA8F0 map ≠ Off, 0x5F32B0 gauges, 0x5EA8D0 score, 0x5F33C0 lap info /
  split ≠ Off / position / leaderboard), the camera (option → view kind
  0x502500; the camera key steps it, 0x502540: wrap at Far, Super Far in
  drift), the transmission (racer info +0x20; manual shifting). **Open**: map rotation (mode 2), stability,
  jump camera, rearview mirror rendering, volumes (no audio), the save
  prompt flow (0x4F37F0). **Custom display values in effect**
  (**verified** readers in the frame function 0x5CAAE0): Light Glow
  (0x870CE4, 0x5CB575), Light Trails (0x870CE0, 0x5CB587), Over Bright
  (0x870CFC, 0x5CB778), Enhanced Contrast (0x870D00, 0x5CB834), Motion
  Blur (0x870CDC, 0x5CB31F) and Horizon Fog (0x870D0C, 0x5CB7AB) drive
  the post passes (`Switches::from_custom`; a Level Of Detail only
  presets them, 0x5BE880); Road Reflection Detail (0x870CD4) 0 skips the
  road reflection view (0x5CAB25; 3 adds 0x5C7E50 in game, 0x5CAC4C,
  **open**); Particle System (0x870CEC) 0 spawns only the low-detail
  emitters (0x491E50). The rest (2026-10-28; `render::display`,
  `Display::from_custom`; **verified** unless marked):
  * Car Reflection Update Rate (g_CarEnvironmentMapEnable 0x870CB8):
    0 creates no cube (0x5BA4F4 skips the render-target cube 0x870858 and
    the front end's 0x87088C; 0x5B9920 skips copying the platform's
    faces), so 0x5BCAE0 binds a null EnvMapTexture and the cars reflect
    black, in game and in the front end (clone: a black cube,
    `WorldRenderer::clear_environment`, `car::black_envmap`); 0x5CACDC
    also skips the faces. 1–3 pick the faces drawn each frame (0x5C397B,
    `display::env_faces`): row `frame % P[rate] + 6·(rate − 1)` of the
    6 × 6 flag table 0x7FEEA0, periods 0x7FEE90 = {_, 6, 4, 1}, frame =
    0x870818 (+1 per frame at 0x5D2C5E): rate 1 one face a frame, rate 2
    faces 0 and 4 every frame plus one of 1, 2, 3, 5, rate 3 all six;
    the others keep their last image. Only in game (0x8654A4 = 6).
    Rate ≠ 0 also places the cube at the car (0x5D2937 → 0x61AB60).
    The counter (**traced**): one writer, `inc` at 0x5D2C5E /
    store 0x5D2C6B near the end of the frame's render 0x5D2860 (called
    from the main loop 0x5811D0 at 0x5814E6 / 0x58167A, and at 0x57FC59);
    0x5C3981 reads it during the frame's view setup, before the
    increment, so frame m (from 0) uses m. Nothing resets it (no world
    load, car change or camera cut; the other readers 0x431408, 0x5DAAF4,
    0x5DAB12, 0x5DF0CD, 0x5E7C5E, 0x5E8479, 0x60D56B, 0x61AD90 only
    read). The faces' eye (0x61AB60 → 0x5C3460): the view-1 mover's
    target car (vtable +0x44, else a car of 0x8A30F0 with +0xB64 set,
    position +0x910) plus 0x803E50..58, oriented by view 1's matrix
    (0x48B1E0(1) +0x40) — that frame's car and camera. At 60 fps one
    frame is one tick, so the clone schedules the faces per tick
    (`display::env_faces_due`, docs/uncapped-fps.md "Environment cube on
    ticks"). **Inferred**: what loop 0x57FC59 serves (a render outside
    0x5811D0, counted the same).
  * Car Reflection Detail (0x870CBC): each face drawn clears and draws
    the sky (0x60F890); only with it set the world too (0x5CAD86 →
    0x5D6AE0). **Open**: the cube's sky — in views 0x10–0x15 0x60F890
    draws the solid at 0x8A1F10 (scaled × 0.004525, when 0x802950 = 1),
    which the clone does not draw (its faces start black).
  * Car Shadow / Neon (g_CarShadowEnable 0x870CC0): 0 skips the shadow
    and the underglow in the front end's car draw (0x616D40 → 0x617417)
    and the game's (0x622113 → 0x622EEE); clone: `car_shadow_in_game` /
    the garage's shadow gated. Above 1 the projected-hull block
    0x621958 (**open**, see Car shadow).
  * Car Headlight (g_CarHeadlightEnable 0x870CC4): 0 skips view 9 (the
    headlight projector's draw 0x5CB200 → 0x5D6D30 and its cull context
    0x5B9D55) and the worldheadlight remap (0x5C597C); clone: no
    headlight term (`FrameUniform::headlight` zero). The headlight pools
    in the road reflection (0x6154A0 at 0x5CAC17) do not read it.
  * World Detail (g_WorldLodLevel 0x870CF4): 0x5BE1D0 returns 2 for
    levels 2 and 3, else (level == 1) — 1 or 0; the scenery cull 0x5D6717
    takes LOD A only when it returns 2, else every main-view instance
    draws LOD B (+0x34; nothing when null). Clone: `Placement::mid_solid`,
    `MAIN_LOD_B`. 0x5D2909 also writes 0x883DDC (2 for levels 0–1, 0
    for 2–3; jump table 0x5D2CB8), whose reader is **not found**.
  * Fog (g_FogEnable 0x870CD8): set into the world effect's FOGON
    (0x86EB28 +0x1C → +0x484, ID3DXEffect::SetInt, vtable +0x68) before
    the main view (0x5CB0B1); FOGON is 0 for the road reflection and the
    cube (0x5CAB22) and 1 at load (0x5D211F). Clone: no fog in the main
    view when 0 (`main_fog`; the clear keeps the fog colour). **Inferred**:
    that FOGON gates the fog — world.efx keeps "FogOn" only as a
    pass-state expression (its CTAB in a `tx` preshader, register set 2),
    not in a shader, i.e. a FogEnable state.
  * Texture Filtering (g_TextureFiltering 0x870D18): at effect load
    (0x5D1E30) BASEMINTEXTUREFILTER (+0x9C, 0x5D2096) = 1 point, 2
    linear, at 2: 3 anisotropic (2 on Matrox, vendor 0x102B);
    BASEMAGTEXTUREFILTER (+0xC4, 0x5BE570) the same but anisotropic only
    with D3DPTFILTERCAPS_MAGFANISOTROPIC; BASETEXTUREFILTERPARAM (+0xEC)
    = D3DCAPS9.MaxAnisotropy (0x86E634); the cap 0x876530 = (MINFLINEAR
    and MAGFLINEAR) + MIPFLINEAR + MINFANISOTROPIC (0x5D2226).
    Clone: `display::texture_sampler` (nearest / linear / linear ×16) for
    the world and car renderers. **Open**: a change applies to renderers
    built afterwards (the game reloads on 0x8709D1, not traced); the
    effects' MipFilter.
  * Vsync (g_VSyncOn 0x870D24): the present parameters' interval
    (0x5B9B13: D3DPRESENT_INTERVAL_ONE when set, IMMEDIATE 0x80000000
    when 0); clone: `PresentMode::AutoVsync` / `AutoNoVsync`, the surface
    reconfigured when it changes.
  * Car Geometry Detail (0x870CF8, **verified**, ported: `car::pick_lod`,
    `CarRenderer::set_lod`): 0x5D28E0 sets 0x8A1C10 / 0x8A1C14 = 1 at
    Low, 0 at High — the start LOD of the car draws (0x6316E0 passes them
    as args 11 / 10 of 0x620AF0 and 0x6162F0); see **Car LOD**.
  * **Open** (no clone feature): Crowds (0x870CD0: 0x5CB6C3 → 0x611070, 0x61D1D1,
    0x5BF6E0; no crowds in the clone); Depth Of Field (0x870CF0: targets
    0x5BB871, the pass 0x5D2992 / 0x5CCCE0, which never runs behind the
    chase camera); Full Screen Anti-Aliasing (0x870D08: the present
    parameters' MultiSampleType 1, non-maskable, with the option as the
    quality level, 0x5B9B3B; the glow sprites through the multisampled
    surface 0x86F2E0, made at 0x5BBA84 and resolved into the streak
    buffer by StretchRect, 0x5CBBB2..0x5CBCE5; the clone multisamples the
    in-game scene at 2^v samples, **inferred** — the quality level's
    sample count is the driver's — clamped to the adapter, see
    `display::msaa_samples` and `PostFx::multisampled`; the front end's
    garage scenes too since 2026-10-31, see Display modes and aspect); Tinting (0x870D04 → 0x8707D1: the
    tint pass 0x5C7870, which no dry-weather path enables, see Tint
    pass); Rain Splatter (0x870D10 with particles → 0x8707D2, 0x5C7AC0:
    the lens drops, see **Rain**). The Wheel Options sliders act on a wheel's DirectInput
  device (range, force feedback); the clone reads controllers (see
  **Gamepads and joysticks**) but no wheel's range or force feedback:
  **open**.
* **Controls** (UI_Options_PC_Controller, screen 0x4ECD10; **verified**
  except where noted): title "Controls" (0x04DC6DB5), "Keyboard
  Configuration" (0x5CF08EB7), "Primary" / "Secondary" (0x1B15F53C /
  0x37E740E0), Instructions_Label "Configure your settings" (0x8BAB66EF);
  0x4DBA50 adds 18 rows (name from each row's vtable slot 3): Steering
  (Left), Steering (Right), Throttle, Back / Reverse, Shift Up, Shift
  Down, Handbrake, Nitrous Boost, Camera, Look Back, Enter Race / Shop,
  World Map, Read SMS, Career Statistics, Reset Car, Hydraulics Pressure,
  Hydraulics Bounce, Pause; each key is the input map's (0x874C40 +
  0x370 × action, named by 0x4ACF40). Each row's vtable gives its action
  (slot 3 → 0x4ACF40(obj, action, column)): 3, 4, 1, 2, 7, 8, 5, 6, 10,
  9, 15, 16, 17, 18, 14, 22, 19, 12 in row order. The keyboard's names
  (0x5C20F0 → 0x5B8DA0) are DirectInput's for the action's first
  (Primary) or second (Secondary) DIACTION, "Shift" / "Alt" / "Ctrl"
  printed "Left …" in English. **Verified** default map (DIACTION table
  0x7FCAC0, 0x28-byte entries: action, DIKEYBOARD_ 0x81000400 | DIK):
  Left/A, Right/D, Up/W, Down/S, Left Shift/;, Left Ctrl/., Space/End,
  Left Alt/Right Alt, C/Insert, B/K, Enter/F1, M/F2, Tab/F3, T/F4,
  R/Home, H/Y, J/U, Esc/P (action 11, Q/=, and 25, 5, have no row).
  Rebinding: a cell's handler stores the action and column in the map
  (0x5C2110: +0x33C, +0x340) and sets the keyboard to report the next key
  (0x5B9130). The controllers' pages, the header and Restore Defaults:
  see **Gamepads and joysticks**. Clone: the map in options.txt (`key.<row>.<col>`), driving,
  shifting, nitrous, camera, reset and pause read it; Up / Down pick the row, Left
  / Right the column, Accept waits for the key ("..."). **Inferred**: the
  column keys, the "..." marking, DirectInput's English key names (the
  OS supplies them); the rows shown (7) on TITLE_MASTER / DATA_MASTER /
  DATA_MASTER2. Duplicates (**verified**, 0x5B8560, which the keyboard
  callback 0x5C16E4 calls with the key): the key is first cleared from
  every DIACTION entry that has it (any action, either column: key, GUID
  and flags zeroed), then stored in the action's first entry (column 0)
  or second (column 1, +0x340); if the first is then empty it takes the
  second's, and a second equal to the first is cleared — a key is never
  bound twice, and binding a row's own key to its other column moves it
  there (`Settings::bind`). A row with no key bound draws `---` in both
  cells: 0x5B8DA0 returns "" (0x783FE5) when no entry for the action
  names a key (**verified** on the path where 0x5C20F0 sees
  [this+0x234] == 0x13; its other path, 0x5B8AA0, is not traced), and
  0x4ACF40 then draws the string at 0x790B94, the bytes `---`, through
  0x537BF0 in place of the name (**verified**; 0x4ACF40 has no column or
  language branch). 0x537B40 passes the label to 0x43FBC0 as its text
  argument (**verified**); what 0x43E950 does with it is **not traced**.
  Column 0 has no match skip (**verified**: 0x5B8DDA-0x5B8DE7 skips only
  for column 1; 0x5B8DEF skips an empty key in either column), so an
  empty Primary moves on to the next entry for the action. That entry is
  the Secondary only if the table layout above holds, which is **not
  traced**. **Open**: the clone draws `---` in an empty Primary even when
  the Secondary is bound.
* **EA GAMES TRAX** (EA_Trax_Jukebox, screen 0x553C80): four rows of
  number, artist, title, album and ON/OFF from the profile's playlist
  (0x83ACB8, index and on/off per entry) over the track table 0x7ECD80
  (title, artist, album, "FE"/"IG"; 27 tracks). Each entry's state
  (**verified**, 0x53DEC0): 0 "Off" (0x9CCE9F86), 1 "Menu" (0x5278C50B),
  2 "Racing" (0x5C1B351C), 3 "All" (0x9CCE64C4); left (0x9120409E) steps
  it down wrapping 0 → 3, right (0xB5971BF1) up wrapping 3 → 0, blinking
  that row's arrow (+0x34 / +0x38) with a sound, and 0x479220 rebuilds the
  music masks (states 1 / 3 the front end's, 2 / 3 the race's). Clone: the
  table in order, Up / Down select a track (the list scrolling with it,
  the selected row HIGHLIGHT, others UNHIGHLIGHT — **inferred** as
  UI_Options' list), Left / Right step its state, saved in options.txt
  (`trax`). Defaults (**verified**, the profile reset 0x515AA0 at
  0x515B46): entry i = {i, "FE" → 1 Menu, "IG" → 2 Racing, "AL" → 3
  All, else 0} by the table's fourth field, so the six FE tracks Menu and
  the 21 IG ones Racing. The play mode is settings +0x54 (saved; the
  triggers flip it, see EA_TRAX_Jukebox). **Open**: playing music
  (this install has no PFData/MusicSFX.mpf).
* **Credits** (Credits.fng, screen 0x4B8800; **verified**): the lines of
  CREDITS\<prefix><language>.TXT (UTF-16; 0x4B8808: in English NA_ for
  region 0, UK_ for a European region (0x570A50); the region 0x7FA73C is 6
  in this executable, so UK_ENGLISH.TXT; a US install, docs/region.md,
  reads NA_ENGLISH.TXT). The
  Event Handler sends 0xE6E946B8 at 320 ms; each one clones CreditsArea
  with the next line (from the first again after the last) running
  RollCredit (5200 ms), whose own 0xE6E946B8 at 300 ms brings the next
  line and 0x29161540 at 5200 ms removes it. Back → UI_OptionsMain.
  **Timing** (**verified** and **observed** 2026-11-01, small171): those
  are package milliseconds, and the front end's clock runs at 1.2 × real
  time: 0x5662D0 gives the package manager 0x558C10 trunc(0x865194 ×
  1200 (0x78AAA0)) ms a frame, at most 20 ms (0x558DAB) unless game state
  0x8654A4 = 6 with a real dt. Under Wine (120 fps) the lines scroll
  156 px/s at 1280 × 960, 39 px apart (a line every 250 ms); the clone
  scrolled 130 px/s with the same spacing (300 ms a line). Fixed for every
  package: `frontend::FE_RATE` (layout, `animate`, `run_package`,
  `host_time` and the dialogs' DIM times run on package time); the
  credits' own schedule counts package time. **Inferred**: under 60 fps
  the original's 20 ms cap slows its front end; the clone keeps 1.2.
  Screens whose code times FE events in real ms: swept 2026-11-01
  by the clock each one reads. Real time, unscaled (verified in the
  disassembly): the icon scroller's ease (0x538950: 0x8651A8 and the frame
  dt 0x865194 into 0x574E40), the cursor's 2 s (0x50B460, 0x8651A8), the
  splash's 30 s / 4000 ticks (0x8651A8), the LAN search's 5 s (0x4FB60E,
  0x8651A8), the help bar's name / cash cycle (+0x48 by g_FrameDt × 4000),
  the post-race "Time Behind" cycle (0x4E9BD6: the world dt 0x865150) and
  the D-Day steps (0x5342A0, sim ticks 0x865188). Package time (scaled):
  the arrows' "not while it blinks" test (0x51D050 reads the BLINK
  script's own clock, +0x10 ≤ +0x0C, which the package manager advances),
  so a blink blocks a new one for 400 / 1.2 ms of real time
  (`ArrowBlink::step`, fixed). UI_OLISPConnect's 2 s and EA login's 9 s
  are **inferred** times and stay real.
* **Pause options** (2026-10-28; **verified** except where noted). The
  pause's Options (built by 0x4C2BD0 only when 0x4A7BF0 holds; option
  vtable 0x795510 → 0x4C2700 on Select 0x0C407210; also 0x53C870 and
  0x4A814F) pushes **UI_PauseOptionsMain.fng** (string 0x79071C,
  descriptor 0x7F81D4 {factory 0x4F47F0}, screen 0x4EB580, vtable
  0x797CD4, messages 0x4A7D60). Its options (0x4C3000): Audio, Display,
  Gameplay, Player — UI_OptionsMain's icons, labels and handlers
  (0x7918B8…0x7918D0 → 0x49A620…0x49A680, category 0x83AA04); Header_text
  "PAUSE OPTIONS" (0x1D7BB6C9), Back_button / Select_button "Back" /
  "Select". After the out animation (0xE1FDE1D1) Select pushes
  **UI_PauseOptions.fng** (0x78FA60, descriptor 0x7F81F0 {0x4EB650},
  screen 0x4D93C0, vtable 0x79650C, rows 0x4C4BD0 by category: 0x4C3370,
  0x4C36C0, 0x4C38F0, 0x4C4740); Back pushes UI_Pause.fng (0x79070C;
  UI_PerformanceDyno_MAIN.fng 0x7906F0 when 0x89E7E6); PAD_START
  (0xB5AF2461) and 0xC98356BA with +0x150 resume the game (0x5F4250(1)).
  UI_PauseOptions' rows: Audio the seven volumes (no Audio Mode); Display
  as UI_Options'; Gameplay Career Difficulty (only when 0x500B60: game
  mode 1 and race state 0), Jump Camera, Rearview Mirror, Speedometer
  Units, Explore / Race Map Mode, Save; Player (the paused player, the
  items 0x795660… read 0x8900AC / 0x8900B0): Transmission (not with
  0x89E7D8 or 0x890108, drag), Stability Control, Camera, Feedback (enabled
  by 0x5B8250), Gauges, then (not with 0x89E7E6, test run) Race Position,
  Lap Info, Split Time, Leaderboard, Score — no Wheel Options, no Ask for
  Transmission. Headers: Header_text "PAUSE OPTIONS", Header_text2 "AUDIO"
  / "DISPLAY" / "GAMEPLAY" / "PLAYER" (0x737D1B97, 0xEF1BB23B, 0xE8647E75,
  0xD9DC2F12). Its handler 0x4A8010: Back asks the Gameplay save questions
  (0x4A7F70) as UI_Options does, then (Display: 0x5BEEB0) pushes
  UI_PauseOptionsMain; the Customize Display button's Select pushes
  UI_PC_Customize_Options (0x4A82FA); 0xD05FC3A3 on Display resets it
  (0x5CECA0). Clone: `MenuKind::PauseOptions`, `options::Pause`,
  `rows_in`. Rearview Mirror and Explore Map Mode are left out (0x4C3A7E,
  0x4C3B3F) when 0x5026F0 on 0x83AAB4 is 2: 0x5026F0 returns byte +0x0C of
  the current type's mode record (0x83AAB4 + 4 + type × 20; +0x100 for
  type 12), the player count — its writer 0x502930 is called only from
  0x4B2C00 (UI_QRModeSelect: 2 in game mode 4, split screen, else 1) and
  0x4DF2B7 (1), and the profile reset 0x515800 sets 1 (**verified**). The
  clone has no split screen, so both rows are always listed. **PAD_START**
  (0xB5AF2461, the FE map's comma; **verified**): on UI_PauseOptionsMain
  (0x4A7D9D) it plays out (0x50DA20), sets +0x150 on the out animation
  and 0xC98356BA then resumes the game (0x5F4250(1)); on UI_PauseOptions
  (0x4A8344) Gameplay first asks the save questions (0x4A7F70 with
  0x33333333: Yes → 0x4A8164 sets +0x158 and resumes, No stays), then the
  same resume via +0x158; UI_PC_Customize_Options' handler (0x49C540)
  does not take it. Clone: `Game::pause_start` (resume = the paused race,
  or free roam as the pause's Back). **Open**: the 0x4A7BF0 test for
  offering Options; 0x571A70 (the PAD_START sender check).
* **UI_PC_Customize_Options** (0x78FA8C, descriptor 0x7F7E38 {0x4E36B0},
  screen 0x4CF660, vtable 0x795E04, rows 0x4B6510, messages 0x49C540;
  title 0x42ADB44C "Customize Display Settings" 0x381FE1E9). From Display's
  button on UI_Options (0x49C98A) or UI_PauseOptions; Back returns to
  UI_PauseOptions when the game state ([0x8900D8]) is 6, else UI_Options
  (0x49C57B). Rows in order, each listed only when its card cap
  (0x8764D0…) is nonzero: Car Reflection Update Rate (slider, 0x870CB8,
  cap 0x8764D0), Car Reflection Detail (Low/High, 0x870CBC), Car Shadow /
  Neon (slider 0x870CC0, cap 0x8764D8), Car Headlight (0x870CC4), Car
  Geometry Detail (Low/High, 0x870CF8), Crowds (0x870CD0), World Detail
  (slider 0x870CF4, cap 0x87650C), Road Reflection Detail (slider
  0x870CD4, cap 0x8764EC), Light Trails (0x870CE0), Light Glow
  (0x870CE4), Particle System (0x870CEC), Motion Blur (0x870CDC), Fog
  (0x870CD8), Depth Of Field (0x870CF0), Full Screen Anti-Aliasing
  (slider 0x870D08, cap 0x876520), Tinting (0x870D04), Horizon Fog
  (0x870D0C), Over Bright (0x870CFC), Enhanced Contrast (0x870D00), Rain
  Splatter (0x870D10), Texture Filtering (slider 0x870D18, cap 0x876530),
  Vsync (0x870D24, always). Sliders run 0 → cap step 1 (slot 17, e.g.
  0x49BA70); Low/High is 0xB55E7678 + (v ≠ 0) × 0xABCC822E (0x49BAF0);
  On/Off 0xCCFA + (v ≠ 0) × 0xFFFF3942. Toggles take Select as well as
  Left / Right (0x49BAB0), Vsync only Left / Right (0x49B8A0); each change
  sets 0x8709D1. Presets: the detail slider (0x49B76F → 0x5BEA00 →
  0x5BE880) and Default Settings (0x5CED44) set every value from the Level
  Of Detail, then 0x5BE6A0 holds each to its cap (`custom_presets`). The
  caps come from the Direct3D probe 0x5BE210 (vendor / device and caps
  tests). **Inferred**: the clone uses a fully capable card's caps
  (`CUSTOM_CAPS`), anti-aliasing 3 (the real cap is the non-maskable
  multisample quality count − 1, 0x5D2724). **Open**: the empty-label
  button 0x790BD8 added after the first row when 0x43DDF0(…, 0x5E) holds;
  0x5BE880's resolution change above detail 3 (0x5C0CE0); the ATI
  anti-aliasing clear (0x5BEA0F).
* **UI_Wheel_Options** (0x78FA74, descriptor 0x7F7E1C {0x4E3770}, screen
  0x4CFBA0, vtable 0x795E50, rows 0x4B8A10, messages 0x49CBB0): PLAYER
  "Player 1" / "Player 2" (0xD87133BC + (0x836364 ≠ 0)); three sliders on
  the player's block (0x83AA64 + 0x28 × player): Wheel Range +0x1C (0.1–1,
  step 0.1, 0x49B3C0), Road Feedback +0x20 (stored 2–20, the slider shows
  1 − (v − 2) / 18 and writes 20 − f × 18, 0x49B4D0 / 0x49B450), Overall
  Force Strength +0x24 (0–1, step 0.1); defaults 1.0, 12.4, 1.0
  (0x511793). Back → UI_Options (0x49CBF0). Opened by UI_Options' Wheel
  Options button (0x49C9A8, category 3).
* **Not reached on the PC build** (**verified**): UI_Trailers (0x78FAA8,
  descriptor 0x7F7EC4) is pushed only for options category 6 (0x49C96C),
  and no code writes 6 to 0x83AA04 (writers: 0x49A62A…0x49A70A set
  0–5, 7, 8; 0x4B60C3 5; 0x4EB47A 0). UI_OptionsController,
  UI_QRTrackOptions have no name string in SPEED2.EXE; MU_PostRaceConfirm's
  descriptor (0x7F8A78) has no factory.
* **Status screens** (Career Statistics; **verified** except where
  noted). The Career Statistics key (LEGEND 0xC1, DIACTION 18 — T / F4,
  the Controls row 13 — state 0x13, 0x606740 case at 0x6068D5) pushes
  **UI_Status_Master.fng** (0x799F70, descriptor 0x7F804C {0x4E1B80})
  when the game state is ≥ 4, under the same gates as the World Map key
  (a career's free roam, 0x500B60; no engage under way, 0x5004C0). The
  factory runs 0x528A70 on the career (0x85AD40) into the block 0x835C00
  (dword indices): [0] bank (career +0x7134); [1] races entered (career
  +0x872C, incremented by 0x541170 — the career event start from the
  marker 0x499B27, the World Map 0x4E1284 and 0x5416D1; zeroed by the new
  career 0x5329CB; saved 0x514579); the visit table (career +0x7AFC /
  +0x7B00, 8-byte entries hash + state, byte +6 = 1 won) looked up in the
  event table (0x8383E0, 0x88 each, key +8): an entry won with category
  +0x34 ∈ {1, 2, 3} counts [9] URL when +0x34 = 2, else by type +0x0F
  [4] Circuit (0), [5] Sprint (1), [6] Drag (4), [7] Drift (5), [8]
  Street X (2) (type 3 none), [2] their sum; every won event counts
  toward [0x1A] = won / event count (game completion); an entry that is a
  shop (0x8383F0, 0xA0 each, key +0x38) with +0x51 set and flag +4 = 1
  counts the region's (+0x9D) entered hidden shops (i16 at 0x835C34 +
  4r), all +0x51 shops their total (i16 at 0x835C36 + 4r); [3] = [2] /
  [1]; [0x15] parts unlocked = 0x515470 / 0x5017F0 (the cost table
  0x83845C's levels with unlock type 1–4, and those 0x515360(mode 4,
  category = record + 1, level = the level's signed byte +5) passes);
  [0x1B] stage (career +0x7AF8); [0x1C] / [0x1D], [0x1E] / [0x1F],
  [0x20] / [0x21] the stage's normal (0x5004D0), sponsor (the four slots
  +0x8C…+0x98 set) and URL (0x500520) wins against the goal table
  0x7F7BA8 (16 bytes per stage: (0,0,0), (5,0,0), (10,3,3), (20,3,5),
  (30,3,7), (35,3,9)); the milestones (0x83840C, 0x40 each, state from
  0x862840 byte +6 = 1): DVDs (+0x24 2 or 4) listed per stage (+0x25) at
  0x835C9C + 0x28·s (rating +0x3C, won), count at 0x835CBC + 0x28·s,
  [0x22] / [0x23] all won / all, [0x24] / [0x25] the stage's; magazines
  (1 or 3) [0x0A] / [0x0B]; [0x0C] sponsorships = stage records with
  +0xE8 set (seven); [0x77 + k] / [0x7F + k] each stage record's
  reputation (+0xE0) and its average over +0xE4 races, records 0 and 1
  summed into [0x78] / [0x80]; [0x26] region completion = (normal +
  covers won + URL + sponsor) / (their goals + the stage's covers).
  Screen 0x4E1A30 (vtable 0x795D3C, messages 0x4B1F20) embeds the page
  (0x555F60, mode 4) from the table 0x7F4428: UI_Status_Career,
  UI_Status_Region, UI_Status_DVD. Tabs (0x4CC5C0, text object
  0x3223A234): "Overall Career Statistics" (0xA3532050); while the stage
  0x862838 < 6 "Intro Status" (0xD56AFE23, stage 0) or "Stage %d - %s
  Status" (0x94D3715B with the region label 0xC08AF0B2 + stage) and "DVD
  Unlocks" (0x74E499B0). Left / Right (0x4B1E60) step the page, not past
  the ends; the arrows 0x444969FD / FE hide on the first / last page
  (0x4B1DF0). Select / Accept / Back play out and resume the game
  (0xC98356BA → 0x5F4250(1)).
  * UI_Status_Career (descriptor 0x7F8068 {0x4EEB10} → 0x4EE8B0, list
    "U2_Menu_Box" / "SideScrollBar", seven rows: label 0x566BF789 + k,
    value 0xA148528A + k, backing 0x28887216 + k, sliders
    Slider_base/_slider/_handle/_Beg/_End_<k+1> hidden by 0x512240(0);
    arrows 0x640F0F22 / 23 hidden at the top / bottom, 0x496F50).
    0x4E13C0's rows, value rows indented by "    " (0x797670): Overall
    Status (heading, 0x4CC4A0), Game Completion Status: (a slider row,
    0x4B1710 / 0x4B1680, [0x1A]), Bank:, Total Spent On Car: (the
    current slot's +0x7F0 when 0x86347D), Races Entered:, Overall Wins:
    (`%$d`, 0x4CC250), Win Percent: ([3] × 100, `%$.1f`, 0x4CC410); Races
    Won: Circuit, Sprint, Drift ([7]), Drag ([6]), Street X, URL; Rewards:
    Magazines Won:, DVD Covers Won: (`%$d/%$d`, 0x4CC2E0), Sponsorships
    Signed:; Total Reputation and Average Reputation Per Race: "Stage
    %d:" (0x8B84E940) rows (`%$.1f`, 0x4CC380) — stage 1, then stage
    k + 1 while record k is unlocked (0x500C00(k), +0xFC; k ≤ 5); Hidden
    Shops Discovered (when record 0 is unlocked): CSTATISTICS_REGION_NAME_k
    rows (entered/all) the same way, while stage record k's definition
    (+0x100) is set; Unlocks: Parts Unlocked (a slider row, [0x15]).
    Up / Down scroll the list (**inferred**: the list's keys).
  * UI_Status_Region (0x4B1C00 → 0x497040): at stage 0 the labels
    0x4D2F5C61 / 62 read "Go to Car Lot" / "Go To Garage" (0x50C900) and
    the values 0x317AD6F2 / F3 and the slider hide; else the slider
    (Slider_base … 0xBDCD0D7C…, 0–1, margin 46) at [0x26], 0x317AD6F2
    "%$.2f %%" of it × 100, F3 normal wins / goal, and with record 1
    unlocked F4 sponsor, F5 URL, F6 the stage's covers (`%$d/%$d`); with
    record 1 locked rows 3–5 (0x949F4949…, 0x4D2F5C63…, F4–F6) hide.
  * UI_Status_DVD (0x4B1CF0 → 0x497330): per DVD cover k of the stage
    the group 0x02A9AFED + k shows, 0x2734FB0E + k "DVD Cover #%d
    %$.1f Star Car" (0x35CCC129; k + 1, rating), 0xB62767F2 + k "Achieve
    %$.1f star rating" (0x030353CD), the tick 0x449A7D70 + k textured
    0x62BC0DFC (won) or 0xB90F50BB (0x50C990); "Not Available"
    (0x42E73D71) hides when there is a cover; the groups from the count to
    4 hide.
  * Stage records (2026-10-28, **verified**): record k (career +0x72D8
    + 0x104·k) is unlocked (+0xFC, 0x500C00) only by the stage end
    0x532C70, which the win handler 0x541430 runs on the won event's
    stage record (event +0x37, 0x54151F; also 0x5410A0): with a final
    event (def +0x28) once it is won (0x500B80) and every milestone of
    the stage with +0x35 set is (0x513490), else once every event of the
    stage is won (0x5130A0); it then calls 0x528330(k), the only writer
    of the stage (0x528342): stage + 1 when k is the current one. So
    record k is unlocked exactly when the career is past stage k
    (`unlocked[k] = k < stage`; a new career clears +0xFC, 0x500766).
  * Slider rows (**verified**, 0x4B17FF): the list draws a slider item
    (+0x90 & 1) with the slider at +0x94 and the value text "%$d %%"
    (0x790F54) of trunc(+0x94 × 100) (0x784258, 0x75BED4) — "57 %"
    (`status::slider_text`). (0x4B1680's own "%$d" buffer at +0xC is
    not drawn for slider items.)
  * Total Spent On Car (**verified**): the current car slot's +0x7F0
    (0x4E1526: 0x513C30's record when 0x86347D); the part purchase
    0x547580 adds each bought part's price (the cost-table price it also
    takes from the bank, 0x547747) in a career (0x850078 = 1), and so
    does Career_BuyPart 0x528A00 (the bank less 0x514280's price, floored
    at 0, the owned bit 0x503D50, then 0x503680(current slot) +0x7F0 +=
    price; the body-shop slots fix, **verified** by the a community save body-shop save, +1000 for a
    hood): the body shop 0x5664A0, rims 0x567A60, spinners 0x569680,
    gauges 0x563D10, decals 0x568D40, the parts list 0x56A310 and the
    unique part 0x561730 (level 4, price 0). The car lot's 0x516B90
    leaves it. Clone: `Profile::spent` / `CarSlot::spent`, saved
    (`spent`, `slotspent`).
  * Visual Rating (**verified**): when there is a player (0x8900AC),
    0x53D880 on the object 0x8637D0 (widget vtable 0x79AA98, built by
    0x5104D0: colours +0xD0 0xFFCAF48A, +0xD4 0xFF80FF00, +0xD8
    0xFFCF8030) sets +0x4C = 0x5234D0 of the player's car, clears +4 and
    runs the update 0x523660 on UI_Status_Career's package. 0x53DAB0
    found per star k < 10 the full star 0x6EC2F0DE + k and the halves
    REP_Half_Star_<2k + 1> / <2k + 2>, and the bar vr_base / vr_slider /
    vr_handle / vr_Beg / vr_End (0–1, margin 72). The update (nothing
    above 10, 0x784560): hides all 30 stars, shows the first
    floor(rating) full stars and, with the fraction ≥ 0.5 (0x784260),
    the next star's first half, each in +0xD0 (0x51D800), and sets the
    bar to the fraction (0x523AE3). The comparison stars (+0x50, colour
    +0xD4, with +4 set) do not run here. Clone: `rating_stars`
    (`StatusScreen::rating`). **Inferred**: the bar is UI_Options'
    slider class (initialised through its vtable slot 2); **open**: the
    bar parts' colour (+0xD0 on its +0x18…+0x20 objects).
  * (career pass, 2026-11-04, **verified** + Wine) 0x5004D0 counts the stage
    record's 35 normal slots, so [0x1C] never exceeds 35; Win Percent
    (0x4CC410) and the reputation rows (0x4CC380) print "%$.1f"
    (0x795D2C, grouped) through 0x50C960. The block compared field by
    field under Wine (internal notes item 12).
  Clone: `frontend::status` (`CareerStats::new` = 0x528A70 on the
  profile, `career_rows`, `StatusScreen`), `Menu::Status`, the key in
  free roam; races entered kept in the profile (`entered`). **Inferred**:
  the career's won events are the clone's `completed` set; the stage
  wins are `Profile::stage_wins`. **Open**: the open / close animations
  and sounds.
* **UI_ChooseCustomHUD** (0x799C20, descriptor 0x7F87A0 {0x554DB0 →
  0x553780}, vtable 0x79BEA8, handler 0x5644E0; `custom_hud`; **verified**
  except where noted): embedded (0x555D10 mode 4) by the Custom Gauges
  overlay's constructor 0x56C220 (UI_CustomHUDOverlay, vtable 0x79D980,
  handler 0x563D00) and removed by that handler (0x5379B0). The step is
  the global 0x838560: 1 while the overlay lists the gauges (0x56C220 sets
  it), 2 / 3 / 4 for the colours, 0 for the purchase.
  - Constructor 0x553780: finds U2_Paint_HighLight (+0x60, 0x9BA6D658),
    the arrows 0xD1CDA594 (+0xAC) / 0x9345E4A7 (+0xB0), hides the picker
    (0x53C570: +0x60, the arrows, group 0xDB285767 to "Hide"),
    Non_Interactive_Group (0xED03D1D3, the gauge preview) and "Redline";
    three 0x14-byte entries at +0x64 (+0x6C node, +0x70 count, +0x74
    index, set to 1); highlight offset (21, 130) at +0xA0 / +0xA4;
    +0x58 = −1; loads GLOBAL\HUD_CustomTextures_ALL.bin, whose callback
    0x50F640 sets +0x54 when loaded.
  - The overlay: 0x50F650 (from 0x56C220) copies the fitted gauge's
    HUDINDEX (0x311151F8) to 0x839BF0 and the fitted colour parts of
    0xA4 / 0xA6 / 0xA5 to 0x839C24 / 28 / 2C (their RED / BLUE / GREEN
    to 0x839BF4…); 0x548D30 sets 0x839BF0 to the selected gauge's
    HUDINDEX. On 0xC98356BA, with +0x54 set and 0x839BF0 ≠ +0x58, the
    chooser runs 0x549010 (textures `3RDPERSON_TURBOFILL/TURBODIAL/
    TURBOLINES_CUSTOM_%02d`, `TACH_FILL_CUSTOM_%02d`,
    `3RDPERSON_RPMDIAL_CUSTOM_%02d` on 0x6D5FE871 / 0x6D5ECE44 /
    0x19C99185 / 0x05D19F25 / 0xF0250DAC, and on 0x1D9051AA the
    `3RDPERSON_<n>LINES_CUSTOM_%02d` by the engine's +0x1F8: 10500 from
    10000, 9500 from 9000, 8500 from 8000, else 7500; Redline's texture
    by +0x1F4, the object staying hidden; MPH / KM/H on
    3rdPersonSpeedUnits) and shows Non_Interactive_Group: the preview of
    the selected gauge beside the list (`custom_hud::preview_quads`). The
    clone takes the stock car type's top rpm (**inferred**; what 0x5B5360(1)
    returns in the front end is **open**).
  - Accept in the overlay (0x406415E3 → 0x563D6C; the first one after it
    opens is swallowed, +0x129): a gauge with a HUDINDEX in step 1 sends
    0xFD7C68D8 to the chooser (0x53C4B0 shows the picker, title by
    0x5504E0, swatches 0x549290(0)) and the overlay's next 0xC98356BA sets
    step 2 and the package colours (0x537960) UI_ICEMAIN 0, the overlay 0,
    the chooser 0xFF. A stock gauge (HUDINDEX 0) does nothing: the step is
    1 from 0x56C220 (0x56C283) and 0x564159–0x564192 returns for HUDINDEX 0
    while the step is > 0 (**verified**); only the needle Accept
    sets 0.
  - Steps (0x5504E0 title on 0xDC0EBA9C): 2 "Choose Backing Color"
    (0x3A9CCF4E), 3 "Choose Dial Color" (0xDED7CA79), 4 "Choose Needle
    Color" (0x6A0B254C). Swatches 0x549290(k): the parts 0x5449D0 lists
    for slot 0xA4 (k = 0) or 0xA6 (k = 1, 2) — the twelve colour parts
    (WHITE, BLUE, OCEAN, GREEN, LIME, YELLOW, ORANGE, RED, PURPLE,
    LAVENDER, GREY, CREAM; the same list in 0xA4–0xA6) — each passing
    0x530950 on a PAINT_SWATCH_n (n from 1) coloured {BLUE, GREEN, RED,
    0xFF}, the rest up to 14 hidden; the selection is the entry's index
    (1 when out of range). 0x549290 also fetches the fitted part of the
    slot but drops it (0x54938B), so a step never starts on the fitted
    colour.
  - Left / Right (0x9120409E / 0xB5971BF1; a click on the arrows
    0x9803F6E2 through 0x52D110) → 0x548E50(∓1): index clamped to
    1..count, "Blink" on the arrow moved towards, UnHighlight / Highlight
    on the swatches, the highlight to the swatch's position + (21, 130),
    and 0x53C630(step, colour, commit): the step's two preview images
    (2: 0x05D19F25, 0x6D5FE871; 3: 0x1D9051AA, 0x19C99185; 4: 0xF0250DAC,
    0x6D5ECE44) take the colour with their own alpha, and the swatch's
    part goes to 0x839C24 / 28 / 2C. Accepting a step without a move keeps
    the earlier pick (the fitted part from 0x50F650, or an earlier move).
  - Accept 0x406415E3 (jump table 0x5647DC): 2 → 3 (0x549290(1)), 3 → 4
    (0x549290(2)), 4 → 0: the picker hidden, package colours UI_ICEMAIN 0,
    overlay 0xFF, chooser 0, and 0x9E382ECD to the overlay, which runs its
    purchase (0x563D6C with step 0): "already installed" (0x639) for the
    fitted stock gauge; in a career 0xD1AC60BE when the price (0x514280)
    exceeds the bank 0x861E74, else the confirm (0x61C, 0x9F00F07);
    outside one 0x51123592. 0x9F00F07 charges (0x528A00) and fits the
    gauge (0x55C240), and both fit 0x839C24 → slot 0xA4, 0x839C28 → 0xA6,
    0x839C2C → 0xA5 (0x631D30: car +0x590 + slot × 4), then +0x114 makes
    the next 0xC98356BA close the overlay and the chooser back to
    UI_ICEMAIN (0x251BF83E). The dialogs' Cancel (0xB4623F67) returns to
    step 4 (0x153E234 shows the picker again). The colours are the car's
    installed parts, saved with it (the clone's `Profile::installed`).
  - Back 0x911AB364: in 1 it goes on to UI_ICEMAIN; in 2 / 3 / 4 the
    step's images go white (0x53C630 with 0xFFFFFFFF, not committed), its
    index 1, and the step before (2 → 1: the picker hidden and every
    package colour 0xFF, the list again).
  - The purchase question: `Game::buy_custom_gauge` asks
    0x44B7167F (career) / 0xF5AC2B2A (outside one) with No focused, or
    shows the bank dialog, all over the picker with UI_CustomHUDOverlay
    dimmed (0x560B70); Yes is `fit_custom_gauge`. The "already installed"
    branch (0x5641BA) needs HUDINDEX 0, i.e. a step-0 overlay, which the
    stock gauge never reaches. **Open**: the chooser's picks are dropped
    when it returns to the list (the game keeps 0x839C24… while the
    overlay is up); the help bar (0x54E6F0, 0x409 in step 4) and mouse
    clicks are not ported; the overlay's own 0xE1FDE1D1, 0x81F0B71A,
    0x72619778 / 0x911C0A4B messages are not traced.
* **UI_QRModeOptions** (0x78F380, descriptor 0x7F858C {0x4FA550};
  `UIQRModeOptions`): the Quick Race Game Options after the track select
  (see UI_QRTrackSelect below); from the mode select only for Outrun in
  game mode 4 (split screen, 0x4B2A80).
* **Main-menu Customize** (0x4AED30; **verified** except where noted):
  game mode 0x850078 = 0x20, pushes UI_QRCarSelect.fng (class 0x4F9D90),
  titled "Customize Car Select" (0x61032CE5) with "Customize New Car"
  (0xC8054CE0) for a stock car (0x4B2310); accepting (0x4FC2E5) checks a
  free "my cars" slot (0x5165E0, else 0x91921A8D "over the limit") then
  0x552CD0(3, …) opens UI_ChooseCustomizeCategory.fng (class 0x556650,
  options 0x520D70): "Change Body Parts", "Change Performance Parts",
  "Change Paint / Vinyls" (not when 0x83898C is 1), "Change Interiors /
  Car Specialties", plus "Car Lot" (game mode 0x100) and "Performance
  Tuning / Dyno" (0x83898C 2–4, not game mode 0x200). 0x552CD0 sets
  0x83898C to its first argument (0x552D32), so Customize has all five.
  A free slot (0x5165E0): one of the garage 0x83AD90's 20 car entries
  (+0x544, 0x434 each) with +0x54C zero (the clone's `my_cars_free`; see
  "Native dialogs: My Cars limit and Custom Gauges bank"). The Dyno item (0x520D20, vtable 0x79B580 →
  0x55E7A0) asks "Do you want to enter the Performance Tuning / Dyno
  track?" (0x54B649F1; 0x843F5D58 when 0x8500D0 is 0x11), No staying;
  Yes (0x34DC1BEC in 0x55E530) goes, when the profile matches its
  snapshot of 0x552CD0 (0x52A4D0) and 0x83AA00 is set, to 0x5309F0(0,
  0x83AAB8's car, 2) — the career car 0x863480 in a career — else to the
  save prompt 0x4A3100(0x1141), which the clone does not have. Its Quit
  Tuning (0x564DE0) asks by game mode: career 0xE5A0D45E "Quit to
  Garage?" (0xB3FB59A9 "Quit to Performance Shop?" with 0x83A640 set),
  Car Lot 0xFC820867, else 0x40813F63 "Are you sure you want to quit
  tuning?"; Yes (0x34DC1BCF) leaves the screen (0x538CB0). Clone: the
  stock cars (as Quick Race's), then the five categories opening the body
  shop, performance, paint and specialties shops and the dyno on the car;
  Back to the main menu. The car is customized on an
  unsaved profile carrying the loaded profile's progress (stage, wins,
  milestones: the unlock tests read the global career, 0x515360 /
  0x500B80). Its part lists take 0x50F0D0's flags — 2 | 4 only in a
  career (0x850078 = 1; 2 alone with 0x83898C = 1), by 0x850088 for mode
  8, and 1 for any other mode, Customize included — so 0x52A020 asks
  0x529D70, the Quick Race rule (`Profile::part_listed`), instead of
  0x515500: the part's unlock level (+0x05 >> 5) is 0; or the cost
  table's unlock (0x515360, mode 4; the vinyl category 0x3D instead
  tests the cheat table alone, 0x579DB0); or, once the last stage's
  final event is won (0x86262C = stage record 5's definition, +0x28;
  0x500B80), a level 1–3 part whose cost level unlocks by a shop
  (0x501790: type byte 1) or any level-4 part. The cheat table 0x865930
  (0x579D90 by level, 0x579DB0 by hash) holds the codes typed at the
  title (0x57B8C0 → 0x579CC0, cleared at 0x575FE0 unless kept) and the
  unlock-all byte 0x838464 short-circuits 0x52A020; neither is set in the
  clone. Performance levels (0x52A120 flag 1 → 0x515630(4): the cost
  table alone) and the category locks (0x52A1B0 flag 1 → 0x515360) keep
  the career's test. Prices: the parts browser hides the part's price
  outside a career (0x546FC0: 0x850078 ≠ 1 → 0x4954E0 on 0x0380B632),
  and the graphics shops' Total Cost and swatch price follow 0x5453C0 /
  0x545470 behind the same test (0x546E92, 0x5484AD, 0x54FB17); the
  clone hides all three in Customize (`customize_lists_parts_by_the_quick_race_rule`).
  Nothing is charged in mode 0x20: 0x528A00 (the price off the career
  object 0x85AD40 +0x7134) does not test the mode, but each of its callers
  checked here does — the body shop's Select (0x566DD5), the carbon toggles
  (0x567055), the setup purchase (0x547580 at 0x54761B), the performance
  part / package questions (0x55A2AE / 0x55A03F) and the other shops'
  purchase questions (0x5641FB, 0x567D8A, 0x568E73, 0x56A41C) ask to buy
  (Yes 0x9F00F07, after the bank test) only when 0x850078 = 1, else to
  install (0x51123592: fitted, not bought, no bank test, no brand
  screen). The price-text sites found by the
  0x0380B632 xrefs (0x5456E0, 0x5469D8, 0x547318, 0x54744F, 0x547FF5,
  0x548DFD, 0x56249E, 0x56250C, 0x5685D2, 0x546FC0) hide the price unless
  0x850078 = 1 (checked at 0x5685C6, 0x546FC0 and 0x5623ED), else 0x4954E0
  hides it. **Verified** for the package list (UI_ChoosePerformancePackage;
  its handler 0x561D20 opens UI_BuyPerformanceParts.fng on Select 0x0C407210
  and plays PackageFadeIn on 0x8577E0A0): the total, 0x0380B632 at 0x56249E /
  0x5624B4, is one of these sites. Its block (in a routine Ghidra does not
  analyse; 0x561D20's body ends at 0x5620D0; the disassembly is the source)
  runs with ebx = 1 (0x5620F1, no write to ebx before 0x5623ED). 0x5623ED
  needs 0x850078 = 1 and 0x5623F9 needs 0x83898C ≠ 1, else 0x5624ED hides
  0x0380B632, 0x06E0C9A8, 0xBE8693E1 and 0x89CA4FDB (0x4954E0). The total is
  shown when > 0 (0x562491), else "n/a" (0xA4C02484, 0x5624A6). The career
  path also sets the discount label (0x06E0C9A8: 0x66559034 when the 0x458340
  item's +0x38 is 0x7FC8D0D0 at 0x56241D, else 0x6DA17A25), its amount
  (0xBE8693E1, 0x562460), and 0xEB3A688A from 0x861E74 (0x5624D1); the clone
  hides the discount lines (open). The clone hides the total on the scratch
  profile (`perf_package_menu`). **Open**: the writer of 0x83898C (read at
  0x562216, 0x5455E8 and elsewhere); the per-part price rows (no 0x2AF31879
  to 0x2AF3187C immediates in SPEED2.EXE); the total on levels 2 and 3
  (0x56229B sends level 2 to a car-stats refresh with no total; only ecx = 1
  reaches the total at 0x562331).
  Not in the clone: keeping customized cars ("My Cars",
  0x4FC473 → 0x502A40 on 0x83AAB4).

## Style points and the nitrous refill (2026-10-30)

**Verified** in the binary (objdump; REA's Ghidra provider did not start
here: no Java) and **observed** under Wine unless marked. Ported in
`src/style.rs`, `nitrous::Tank::refill` and game.rs (`style_frame`,
`roam_style_frame`).

* **Correction.** Earlier notes said only an online / LAN "Race Breaker"
  refills nitrous. That was wrong. The Race Breaker flag 0x89E7E7 is the
  race parameter block 0x89E7A0's +0x47, and the block's reset 0x5ED600
  (callers 0x53FC30, 0x53FD40, 0x5791EE, 0x57FF5B) sets it to 1. Its only
  other writers: the Quick Race drift set-up 0x53F840 (0x53F935) and the
  street X / type 5 set-up 0x53F380 (0x53F459) clear it, and 0x53FD40
  (0x53FE06) copies the online / LAN option 0x850099 ("Race Breaker",
  label 0xB82A976E; Yes / No 0xF770 / 0x61C; toggled by 0x4B33A5; default 1
  from the options block reset 0x5030E0 on 0x83A9D0 + 0x156B0 = 0x850080).
  0x53FD40 runs only from 0x601C90 with 0x86676C set (written by the
  network code 0x58B22F) and from 0x601D30 (the bitstream reader
  0x58C061). The Quick Race set-up 0x53FC30 (jump table 0x53FD08 on
  0x83AAB4) resets the block, then runs the type's set-up: Circuit, Sprint
  and type 7 0x53F160, Drag 0x53F6A0, Drift 0x53F840, URL 0x53FA50, types 5
  and 10 0x53F380, Outrun 0x526840, Free Run 0x53F590. So the flag is 1 in
  Circuit, Sprint, Drag, URL, Outrun and Free Run, and 0 in Quick Race
  Drift and street X. **Observed** (Wine, career free roam in Rachel's
  350Z): 0x89E7E7 = 1 and 0x850099 = 1. **Inferred**: career races keep
  the reset's 1 (no career writer was found).
* **Style manager per human racer** (racer set-up 0x5FC2B0, at 0x5FC3A6:
  for a local human, owner +0x14 → +4 = 1 and +0x900 = 0, in any race
  type, or for an AI racer in drift with 0x802228): style manager
  0x56EBF0 (+0x1D4), misc style manager 0x56CE00 (+0x1DC), and in drift
  also the drift manager (+0x1D8). Then racer +0x194 = the total
  (0x56C7E0(0), 0x5FC46A). The mask (0x56EBF0, 0x56EC20..0x56EC8B): 4
  drift (0x89E7D9 / 0x890104), 2 drag (0x89E7D8 / 0x890108), 8 point to
  point (0x89E7B4), 0x20 (0x89E7E5 / 0x836474, **not traced**), 0x10
  street X (0x89E7E3 / 0x89010C), else 0x40 with 0x89E7E1 set, or 1.
  **Observed**: in free roam the manager holds entries 15–38 (mask 1).
  The free roam racer's player chain is [[[0x8900AC] + 4] + 0x1C]; its
  [+0] is the car (tank at car +0x454: size, per-shot, left, start).
* **Each frame** (Racer_Update 0x6016B0, for every racer still racing,
  also in free roam): the misc manager's detectors 0x56FF40 (they run
  unless 0x5F0100 says other humans race and race status 0x890118 is not
  4), the style manager's update 0x56FD20 (0x601719), the drift scorer
  0x5F7A10 in drift, then the refill at 0x601737: with 0x89E7E7 set and a
  style manager, Δ = total (0x56C7E0(0): +0x9C, Σ trunc(banked) over the
  39 slots) − racer +0x194, +0x194 = total, and tank.0x592650(Δ / 15
  (0x7A5460) × 0.01 (0x78425C)): 1500 points fill a tank. Without a style
  manager, an AI racer (owner kind 2) gets a timed refill (0x601795..0x601863,
  traced 2026-10-30): when more than 1 s (0x7A56A4) has passed since
  racer +0x28 (set to the tick at the green light by 0x5FC9FD for every
  racer, then by each refill), the tank takes max(0, Δp) (0x401050 is
  max) × 0.01 × 0.5 (0x7A56A8), or × 0 when 0x4018F0 (0x89E7D8 drag or
  0x890108) — Δp = car +0x8F8 × 100 (the road percent) less racer +0x2C
  (0 from 0x5FC4B4), through 0x5D5D20 (on a looping road, +0x24 of
  0x88F298: +100 under −50, −100 over 50); then +0x2C = the percent, +0x28
  = now. Half a tank per lap of the road. Ported (`Race::ai_refill`).
* **0x592650 (Tank_Refill)**, f tanks: with L the size in seconds and F the
  fraction (0x5925C0). f > 0: a = min(1 − F, f) when F < 1, left =
  round((a·L + left_s) × 4000); then b = min(2 − F', f') past full, left
  += b × 1.0 (0x7A1794) × L. f ≤ 0: c = max(1 − F, f) when F > 1, then d =
  max(−F', f'). Each step rounds (+ 0.5, then 0x75BED4 truncates). So the
  tank holds up to twice its size; the stage (0x7A1798 {0, 1.4, 1.7})
  then reaches 2 and 3 (torque and grip × 1.4 / 1.7). **Observed**: a
  SPRAY SHOW (100 points) during a shot took left 16000 → 17067 in a 4 s
  tank (= 100 / 15 × 0.01 × 16000), and the total read 100.
* **Negative refills, end to end** (2026-10-31, run nosneg; **verified**
  instruction by instruction with REA / Ghidra and **observed** under
  Wine). Answers "does a negative trick only take the nitrous above one
  full tank?": **no**. It takes the excess first, then goes on below full,
  down to empty.
  * 0x592650 (thiscall on car + 0x40, so +0x414 = car +0x454 size, +0x41C
    = +0x45C left, +0x420 = +0x460 the shot's start tick). L = size ×
    0.00025 (0x784268), or 0 when the size is 0 / 0x7FFFFFFF. F = 0x5925C0
    = r / L, where r = 0x592490 = left × 0.00025 less (clock − start) ×
    0.00025 while a shot runs (clock 0x8651A8 when 0x5F0100, else
    0x865188). F is 0 when L ≤ 0, and past full it is 1 + (r − L) / 1.0 / L,
    which is the same r / L. The sign test at 0x59268A is `fcom 0.0`, then
    `test ah,0x41` / `jnz 0x592794`, so f ≤ 0 (f = 0 too) takes the
    negative path.
  * Step 1 (0x592794..0x592808) runs only when F > 1 (`fcomp 1.0`, jnz
    skips on F ≤ 1). c = max(1 − F, f) (fchs of F − 1; `fcom`, `jz` keeps
    1 − F when it is greater). Then left = trunc((1.0 × c × L + left ×
    0.00025) × 4000 + 0.5), F' = F + c and f' = f − c. Because c ≥ 1 − F,
    this step stops at one full tank.
  * Step 2 (0x59280A..0x59286C) runs whenever the remaining f' < 0 (`fcomp
    0.0`, `test ah,5` / `jp` skips on f' ≥ 0). That is with F ≤ 1 from the
    start, or after step 1 when f was larger than the excess. d =
    max(−F', f'), and left = trunc((d × L + left × 0.00025) × 4000 + 0.5).
    So left never goes below the part a running shot has already burned
    (live remaining ≥ 0): with no shot running, it never goes below 0.
    With f = 0 and F > 1, step 1 only rewrites left with itself.
  * Callers (Ghidra and the disassembly): only 0x60178B and 0x601847,
    both in Racer_Update 0x6016B0. 0x60178B passes the style Δ ÷ 15 × 0.01
    signed, with no clamp. 0x601847 (the AI timed refill) passes
    max(0, Δp) × 0.01 × 0.5, or × 0, so it is never negative.
  * The total can go down. A trick's bank (the stores at 0x56D652 in 0x56D630, 0x56D759 and 0x56E9B1)
    adds `+0x2C += ftol(+0x24)` with no clamp, and +0x2C is cleared only by
    0x56D660, which is called only from the manager's constructor
    (0x56ECCF). 0x56C7E0 sums the 39 slots with no clamp either. So a
    negative trick lowers the total, and
    the total can stay below 0. Racer +0x194 is set to the total every
    frame, negative values included (0x60176C), so later positive tricks
    refill by their own Δ (no debt is carried).
  * The negative entries in the trick table (the maintainer's probe `trick_table`):
    * TRAFFIC HIT (16): −50 × rate, threshold −30, mask 0x49, unlimited
      runs.
    * WALL IMPACT (3): −10, threshold −15, mask 4 (drift only). Its
      refill needs 0x89E7E7, which Quick Race drift clears; for career
      drift it is **inferred** to be set (see above).
  * **Observed** (Wine, career free roam, Rachel's 350Z, 4 s tank = 16000
    ticks). Tank left was **written by hand** before each run. The player
    was teleported at 25 m/s into a traffic car (`hit.py`, a
    maintainer tool). Logged left / total / +0x194:
    * Above full: 28000 → 27552 → 27072 → 26581 → 26080 for TRAFFIC HITs
      of −42, −45, −46 and −47 (total 0 → −180).
    * Just over full: 16800 → **15968** (−78: 800 off the excess, then 32
      below full) → 15029 (−88) → 14549 (−45).
    * Below full: 12000 → 11349 (−61).
    * Nearly empty: 300 → 0 (−30).
    * Empty: 0 stays 0 (−30, −31).
    * Racer +0x194 followed the total each time (−180, −391, −452, −513,
      −543).
    Each step equals Δ / 15 × 0.01 × 16000, rounded as above.
  * The clone's `nitrous::Tank::refill` and `style::refill` (from
    `style_frame` / `roam_style_frame`) already do this. The test
    `negative_style_points_drain_past_full_down_to_empty` pins these runs.
* **Feeders** (calls to 0x56EAC0(rate, x), or to its wrapper 0x56ED20(slot,
  rate, x), which feeds slot `slot` when it exists; x only goes to the
  trick's +0x48 / +0x4C record of the run's span, no points): in the misc
  update 0x56FF40 (offline always; online only with race status 4) →
  0x56EEF0 (LEAD LAP 17, CLEAN SECTION 18, CLEAN LAP 19), 0x56F030
  (HANG TIME 35), 0x56F4D0 (POWER SLIDE 34), 0x56F630 (360, 22), 0x56F180
  (AVOIDED ACCIDENT 21 and CLEARED ACCIDENT 27, slot +0x6C at 0x56F4AC —
  earlier notes said 27 had no feeder), 0x56F730 (HEAD START 24; GAINED
  1ST…4TH 32–29; DRAFTING 36; REVERSE 37), 0x56F970 (SHORTCUT 20),
  0x56FA70 (J-TURN 25), 0x56F9C0 (Bounce 33, Hydros 38); the contact
  callback 0x56FB30 (TRAFFIC HIT 16); BEST LAP 23 (0x4AA4AA), BURNOUT 26
  (0x5F782F), SPRAY SHOW 15 (0x60352D) and the drift manager's DRIFT 0
  (0x56EE80). Each detector's conditions: "Style tricks, end to end"
  below.
* **Where the points go.** The nitrous refill above; the race's stats
  record (0x515DB0 → +0x104 = the total, 0x515F53; +0x108 = max(owner
  +0x14 → +0x904, 1.0), 0x515F5C..0x515F7B: fcom 1.0, kept only when
  greater); the profile's career stats (0x515990 from 0x551524 /
  0x551559: per race type, +0xA18 + 12·type += _ftol(floor(+0x108) ×
  _ftol(+0x104)) via 0x4FE170 (_ftol 0x75BED4 truncates; the floor is its
  correction at 0x4FE19C), +0xA4C += 1 for type 1 (drag) when the
  result's status +8 (racer +0x10) is 9 "Totaled", and in split screen
  +0xA48 / +0xA44 the last race's score / win; which screen shows them is
  **not traced**). **Clone** (2026-10-31): `RaceTotals::race_done`
  with `career::style_score(total, 1.0)`; **inferred**: owner +0x14 →
  +0x904 is ≤ 1 for the single player (its writer not traced), so the
  factor is 1. **Observed** under Wine: a drag won by teleport (no tricks)
  left +0xA24 unchanged and its results record had +0x104 = 0, +0x108 = 1.0. Reputation (margin table, 0x513FE0) and
  cash (purse) do not read them.
* **HUD** (style element 0x4F7AD0, feature 0x20000, update 0x4DA900):
  objects Style_Point_Backing +0x28, Style_Points_Title +0x2C, SP_DATA
  +0x30, SP_Trick_Point +0x34, SP_Trick_Type +0x38, Tach_NOS_Gamebreaker
  +0x48, the drift objects and BestDrift. At construction +0x79 = 1 when
  the player's car has a tank and 0x89E7E7 is set, else SP_DATA hides
  (0x50CA00) and +0x79 = 0. Each frame 0x4DA900 hides SP_DATA unless
  +0x79, reads the total into +0x58, and outside drift runs the trick
  popups (0x4DA710) and 0x4AB700(total). **Observed**: no style box
  showed at rest in free roam (before and after the SPRAY SHOW).
  The clone builds the element in every race and free roam (`hud.rs`
  `STYLE`, `Popups`; 2026-10-30).
  **The feature path, resolved** (2026-10-30): 0x6000A0 returns
  the low features in eax (its esi) and the high in edx (its edi); the
  element +8 store (0x4CA65D) takes the low. The low 0x20000 comes from
  0x5EA8D0 alone: set when the player's HUD option byte 0x83AA67 + 0x28 ×
  player (the Score / style option, +3 of the block 0x83AA64) is on; it
  is called in the pre-green branch (0x6001FC) and the race branch
  (0x60046A, racer +0x28 = 0); that career free roam takes the race
  branch is **inferred** from the observed mask below. 0x5F34D0 (pre-green with 0x89E7E6) gives low 0x20000 only in
  drift (0x5F354F); its 0x5F351E (mode ≠ 7, +0x234 = 0) is the **high**
  word — the countdown element 0x4F7340 (0, 0x20000) — as is 0x600563.
  **Observed** (Wine, a new career, after the D-Day script, 2026-10-30):
  the options block 0x83AA64 read 1 in its first seven bytes; the HUD
  ([0x8900AC] +0x9A4) +8 = 0x120041 (low: minimap, 0x40, style 0x20000,
  0x100000) and +0xC = 0xC0082; the style element (HUD +0x4C) has mask
  0x20000 at +0x10 and those features at +0x1C. So outside drift the
  style element is live in free roam and races: 0x56FE1C → 0x4DA6F0
  forwards the trick reports, 0x4C88E0 queues them and 0x4DA900 →
  0x4DA710 shows them (the head entry not yet shown, through 0x4AB780:
  SP_Trick_Type +0x38 / SP_Trick_Point +0x34 get the trick's name and
  points (0x4AB780 read in outline only) — "%d" 0x78F744, or with a count over 1 "%d x %d" 0x790AC0 /
  0x790AB4, colours from element +0x60 / +0x64 / +0x68 by state and
  drift — and the script "animate" 0x790A9C; it is dropped 0.5 s or,
  alone, 1 s (0x78E074 / 0x78E078) after its +0x18 tick; with the queue
  empty and neither showing "animate" nor "fadeout" 0x790AA4 both are
  hidden; Tach_NOS_Gamebreaker +0x48 runs GLOW / GLOW_2 (0x796580 /
  0x796578) by +0x7A and is hidden without the gauges option byte
  0x83AA64). Not seen on screen under Wine: no trick scored in the
  attempts (a burnout and a slide against the airport wall; style total
  0), nor in a later run (Alt_L / burnout / a slide in free roam: the element's
  queue, element +0x20, stayed empty at sim-rate ≈ 0.1× real time, so
  a popup would last about 10 s real; the trick list was never fed).
* **Popups, ported** (**verified** from the code unless marked):
  0x4C88E0 (from 0x4DA6F0 when the element's mask has 0x20000): with the
  drift flag 0x89E7D9 / 0x890104 clear, or the trick's index (+0xC) not
  0, each report with points (+0x24) not 0 refreshes the queued node of
  its index (node +0x28 lost = state 4, +0x24 state, +0x0C points, +0x10
  ftol(multiplier) unless state 3, +0x18 = 0) or makes a node
  (malloc(0x2C), 0x4C7E20: +8 continuous (trick def +6), +0xC points,
  +0x10 ftol, +0x14 index, +0x1C the name (def +0), +0x18 0, +0x20 0,
  +0x28 lost; +0x24 is **not** written, so a new node's state is CRT heap
  memory: **open**, the clone takes it as not 5) at the back, or the
  front for negative points; in drift the index-0 trick goes to the drift
  element's one slot (+0x20..+0x48) instead. SP_DATA, Style_Point_Backing
  and Style_Points_Title exist in no HUD package, so 0x4AB700's total
  never shows; SP_Trick_Type / SP_Trick_Point exist in HUD_SingleRace
  (animate 600, fadeout / fadeoutCrash 900 ms) and HUD_Drag (animate
  2000 ms), not in HUD_Drift / HUD_Short_Track. GLOW / GLOW_2 are only
  named (0x4DA775..0x4DA8A6): nothing runs them.

### Style tricks, end to end (2026-10-30)

**Verified** in the binary (objdump of SPEED2.EXE; REA's Ghidra provider
did not start: no Java) unless marked. Ported in `src/style.rs`
(`Misc`, `detect`, `draft`), `drive::CarState::style_contact` and game.rs
(`style_detect`, `Race::style_frame`, `roam_style_frame`); tests in
`style::tests`. The trick table's values (points, threshold, multiplier
ceiling, runs) are the GLOBALB table's, dumped by the maintainer's probe `trick_table`.

* **Misc style manager** (0x56CE00, racer +0x1DC; +0 the racer, +4 its
  style manager): +0x08 / +0x09 / +0x0A clean flags (1), +0x0B / +0x0C
  edge latches, +0x10 best place, +0x14 / +0x18 the 360's count and mark,
  +0x1C J-turn stage, +0x1D / +0x1E / +0x20..+0x28 near-miss stage, jump
  flag, pair and its bodies, +0x2C / +0x30 the last hit's tick and
  strength (+0x34 kind, +0x38 traffic), +0x34 = −1 at first. "Since the
  hit" (0x56CFE0) is (now − +0x2C) × 0.00025 s, or 99999.9 (0x79DC20)
  with none. The detectors run once per Racer_Update (0x6016B0), i.e. per
  frame, not per physics step; the clone runs them once per frame after
  the frame's physics steps.
* **Contacts** (0x56FB30, called from each push 0x5A9630 at 0x5A9961 for
  each of the record's two cars whose racer has a misc manager; the record
  is the argument): needs record +0x60 > 3 (0x79DB34; +0x60 is the sum of
  the record's pushes' |J|, 0x5A9689) and the record made at most 0.1 s
  ago (+0x08). A record on a track polygon that is not upright (0x5A0170
  false) and not car against car (0x592C70), while the car has a wheel
  down (+0x424 > 0) and up.z (+0xB8) > 0.9 (0x7850F0), is skipped. Then:
  +0x60 > 10 (0x79DB24) clears +0x08, +0x09, +0x0A; +0x38 = 0; car against
  car: if either car's owner kind (car +0x14 → +4) is 3 (traffic), unless
  the racer is done (0x5EE260), TRAFFIC HIT at min(1, +0x60 / 40)
  (0x79DB30) — a one-off with −50 points and threshold −30, so it only
  scores (−50 × rate) past rate 0.6 — and +0x38 = 1; any car contact sets
  AVOIDED ACCIDENT's multiplier (+0x1C) back to 1 and +0x34 = 0; other
  records +0x34 = 3. Last, +0x2C = now and +0x30 = +0x60. The clone's
  `CarState::last_hit` is this record (drift's WALL IMPACT reads it; its
  strength was the closing speed before — now the summed |J|, the push the
  clone gives at 1.85 t). **Not ported**: prop contacts.
* **Sections and laps** (0x56EEF0): on a frame where racer +0x0E is set
  (0x5F7C40: the line count +0x04 passed its best +0x06) — CLEAN SECTION
  when +0x08 is set, else +0x08 = 1 and CLEAN SECTION's multiplier = 1;
  on racer +0x0D (a new lap: +0x08 passed +0x09) the same for CLEAN LAP
  with +0x09, then LEAD LAP when the racer leads (+0x0A = 1; in drift
  0x56DDA0, the points board), else LEAD LAP's multiplier = 1. CLEAN
  SECTION / CLEAN LAP climb to ×3, LEAD LAP to ×2 (table +0x18).
* **HANG TIME** (0x56F030): race status 4, racer not done, no hydraulics
  bounce input (0x5ED2A0; was misread as an AI recovery input), car +0x424 = 0 (no wheel down), no live contact
  (car +0x3C → +0x18), and — unless the trick is already running (state
  0) — up.z > 0.8 (0x7850EC) and the last hit over 0.3 s (0x79DB2C) ago;
  then the height (car +0x68 − the centre of mass's z, 0x5ED410 → +8, −
  the ground under car +0x60, 0x5E7E80) over 0.75 m (0x79DB48) feeds rate
  1. Continuous, timeout 0: the first frame unfed banks it.
  **Inferred**: car +0x60 is the clone's car-space origin.
* **Observed** (Wine, career free roam in Rachel's 350Z, 2026-10-30):
  race status [0x890118] = 4 and its clock runs, so HANG TIME and the
  burnout's distance (car +0x410 grew to 15.6 m) work in free roam too
  (`ROAM_RACING`); racer +0x0A = 1 and misc +0x10 = 1 there (so GAINED
  never fires); the misc manager at start: +0x08..+0x0A = 1, +0x34 = −1.
  A wall hit while reversing set misc +0x30 = 13.05, +0x34 = 3 and
  cleared +0x08..+0x0A (strength over 10). Not reached at the setup's
  5–9 fps: the tricks' own scoring (REVERSE / J-TURN need ≥ 30–40 mph in
  reverse).
* **POWER SLIDE** (0x56F4D0): |slip| (0x59FA20 on car +0x40: 0 unless
  the forward speed +0x42C ≥ 1) > 5° (0x79DB3C), |v| > 40 mph (0x79DB40),
  four wheels down: rate min(1, |v| · |slip| / 20) (0x79DB44), fed when
  there is no live contact; but a hit over 5 (0x79DB38) less than 0.1 s
  ago loses a running or pending one (0x56D5D0(1)) instead.
* **360** (0x56F630): heading = u16 car +0x40C × 0.00549316 (degrees),
  yaw = |car +0x118| (the angular velocity's z; car +0x110..+0x118 is ω,
  0x6047A2). Not four wheels down: count 0. Count 0: yaw > 2 (0x79DB4C)
  marks the heading, count 1. Count ≥ 1: yaw ≤ 1 (0x79DB50) → 0; count 1
  or 2 and the heading 170° (0x79DC3C) or more from the mark (0x56D030:
  min(d, 360 − d)) → count + 1, new mark. Count 3 scores "360" (400
  points, three a manager) and resets.
* **Near miss** (0x56F180): unless the car is ghosted (0x5EBFB0), at most
  30 mph (0x79DB64) or hit in the last 0.2 s (0x79DB68), which reset
  everything. Stage 0: walk the pair list 0x86B490 (+0x230 links) for a
  pair that stays (not 0x592E70), passes 0x598740(0) (**not traced**),
  neither body's +0x11, one car is this car (+0x130), both cars neither
  ghosted nor hidden (0x5EBFB0, 0x5EC070) and both with +0x550 set. With
  no wheel down: the cars' positions (+0x60) within 3 m (0x79DB54) and
  this car more than 1 m (0x79DB58) higher → jump (+0x1E = 1). Else: the
  pair's hull distance (+0x10, 0x5B3D30) under 1.5 m (0x79DB5C) and the
  bodies' relative speed (vt+0x0C) over 80 mph (0x79DB60). The first
  match is kept, stage 1. Stage 1: the pair gone (0x593060) → stage 2; it
  has a record (+0x244) → stage 0; it goes (0x592E70) or its distance is
  over 1.875 m → stage 2. Stage 2 feeds CLEARED ACCIDENT (jump, 1000
  points) or AVOIDED ACCIDENT (100, multiplier +1 each run up to ×5, back
  to 1 on any car contact) and resets. Any car, traffic or racer, counts.
* **Place** (0x56F730): HEAD START with race type ≠ 0, race status 4,
  more than one racer (0x89E7C0 + C4 + C8 + CC), race time (status +0x14
  × 0.00025) in (1, 1.5) s, place 1 and |v| > 5 mph (0x79DB6C). Past
  1.5 s: +0x10 = the place when 0; a place better than +0x10 (> 1) and
  under 5 becomes +0x10 and feeds GAINED (place 1 → 32 … 4 → 29; 28
  "GAINED 5TH" is never fed). DRAFTING with |v| > 20 m/s (0x79DB88) and
  car +0x4DC > 0.25 (0x79DB8C), at rate +0x4DC. REVERSE with +0x42C <
  −40 mph. A hit over 3 (0x79DB78) less than 0.3 s ago loses a running or
  pending REVERSE.
* **Draft** (car +0x4DC = 0x5EBD90, every car each frame from 0x609F00):
  Σ over the car list 0x890080 of 0x5EBC30(other), stopping at 0.9
  (0x7A55C4), at most 0.9. 0x5EBC30: the other car's |v| > 1 (0x8023D8);
  d = |other +0x60 − mine| within 60 m (0x8023EC); c = (other's velocity
  +0x70 normalised (0x43CF10)) · (other − me) / d; a = (c − 0.95) / 0.05
  (0x7A55C0) > 0; L = a × curve 0x89CF30(|v|) (0x5767A0, points from
  0x8023D0: (0, 0), (1, 0), (5, 10), (150, 60)); L > 1e-5; the draft is
  curve 0x8023F0 (0x5764E0: 6 values 1, 0.9, 0.75, 0.55, 0.3, 0 from
  0x802194 over 0..1 in steps of 0.2, linear between) at clamp(d / L, 0,
  1). **Inferred**: the clone's cars are the list (rivals, traffic,
  free-roam racers).
* **J-TURN** (0x56FA70): needs four wheels down and the last hit over
  0.1 s ago, else stage 0. Stage 0: +0x42C ≤ −30 mph (0x79DB80) → 1.
  Stage 1: |v| < 20 mph (0x79DB84) → 0; +0x42C > 20 mph → 2. Stage 2
  scores J-TURN (500) and resets.
* **BURNOUT** (racer stats 0x5F7420 at 0x5F771B, run first in
  Racer_Update; stats +0x28 = −1 from 0x5EDE20 / 0x5F73E0): while race
  status 4 and race type ≠ 7, with +0x28 −1 or 0: s = the mean of car
  +0x3B4 + 8i (signed, the wheels 0x592880 counts). +0x28 = −1 and race
  time < 5 s (0x7843E8): s > 0.5 (0x7A569C) → +0x28 = 0. +0x28 = 0 and s
  < 0.5: +0x28 = car +0x410 and BURNOUT is fed at rate +0x410 (10 points a
  unit, threshold 50). Car +0x410 is the distance driven in race status
  4: += dt × |v| for 0 < |v| < 1000 (0x604874). So a burnout from the
  start scores 10 points a metre driven by its end, once 5 m are passed.
* **SHORTCUT** (0x56F970, ported: `Misc::shortcut`): 0x5D9C00 on
  0x88F298 finds a kind-0xB trigger at car +0x8E0; its +0x1A marks it
  scored once. 0x5D59D0 clears every trigger's +0x1A from the 0x1600-byte
  object's ctor 0x5FF4A0, built by World_Start 0x60A690 at
  TrackLoader::LoadHandler step 6 (0x57F962): once per track load.
  **BEST LAP** (0x4AA444..0x4AA4B5, ported: `Misc::best_lap`); with race
  type 0x89E7B0 = 2 and 0x89E818 set, 0x4AA3B4 shows lap clock − 0x865188
  + 0x89E818 in element +0x2C and skips it; only 0x5400A8 stores 2 there
  (from 0x7FAA28, read by 0x5802F0 from the 0x8650CC record stream), so
  no type the clone makes. **Dead in play** (2026-10-31, race3; verified,
  the disassembly): 0x7FAA28 is 1 in the image's data, its only stores are
  0x53FF31 (1) and 0x5802F0, which reads back the value 0x57DEDD wrote to
  the same stream from 0x7FAA28 itself, so it never holds 2 and the branch
  never runs.
* **Ported** (2026-10-31, hydro; `style::hydraulics`): BOUNCE (entry
  33, style manager +0x84: two or more of the kick bytes car +0x408 + i,
  rate 1) and HYDROS (entry 38, +0x98: corners whose 0x5ED480 state, mover
  +0x74 + 4i, is 2 or 3, at count × 0.25, not with a bounce input
  0x5ED2A0) — see "Hydraulics".

## Difficulty: settings and effects (traced 2026-11-01)

**Settings that exist** (every writer and reader of 0x89E7F4, 0x83AC30
and 0x89E804 in the disassembly):
* **Quick Race Difficulty** (Easy / Medium / Hard): UI_QRModeOptions' row
  0x4B3930 (labels 0x7913C8), the mode record's +0x02 (records 0x83AAB4 +
  0x10 + 20·type, +0x100 for type 12, inside the profile; constructor
  0x52A230 Medium, the fresh profile's drift / Free Run records Easy). Each
  Quick Race set-up copies it to 0x89E7F4 (0x53F160, 0x53F380, 0x53F590,
  0x53F6A0, 0x53F840, 0x53FA50, 0x526730, 0x526840, 0x601E4B).
* **Career Difficulty**: one global option, not per event. Options →
  Gameplay "Career Difficulty" (UI_Options; UI_PauseOptions lists it only
  in a career's free roam, 0x500B60), wrapping 0–2 (0x49AF20 … 0x49AF81);
  profile +0x250 = 0x83AC30, saved with the profile (0x52A37A /
  0x5345D0), Medium on a new profile (0x515B15). Race_SetupFromEvent
  0x5322F0 copies it to 0x89E7F4 at every career race (0x53231B), and so do
  the city starts 0x5404B0 (0x540569) and Career_FreeRoamSetup 0x540790
  (0x5407CC). A change takes effect at the next race or city start. Events
  have no difficulty of their own: they carry per opponent a skill row
  for each difficulty (+0x08…+0x0A; Easy 0–3, Medium 2–7, Hard 5–9, mean
  1.1, 5.0, 8.0 over the 679 entries), the Block row (+0x0B, 1 in all)
  and per event the AI throttle level (+0x7D: 0 or 1 in the 34 drag
  events, 3 in the other 190; the maintainer's probe `difficulty_data`).
* **Catch Up**: the Quick Race record's +0x05 (0x89E7F8); every career
  set-up clears it (see Catch-up, AI route layer).
* **AI skill per racer**: racer info +0x901 (the skill row, read through
  the controller's slot +0x78, 0x402580 → 0x610520): a career entry's
  +0x08 + difficulty (0x5261F0), a Quick Race's or free roam's racer a row
  0x4FED00 draws for the difficulty (0x53EF63, 0x53F0BD); +0x902 the
  Block row (see Block), 1 for every AI racer.
* **AI throttle level** 0x89E804: a career event's +0x7D (0x5323F5), the
  Quick Race drag set-up's difficulty (0x53F6A0: 0, 1, 2), 0x5400C5 in
  the 0x53FEC0 set-up (not reached offline, **not traced**); no other
  set-up writes it, so the Quick Race circuit, sprint, drift, Street X,
  URL and the city keep the last value. The CarDriver of a racer's
  controller takes 0x783A28[level] = 0.93 / 0.97 / 1.0 / 1.0 as its
  throttle cap (0x425B52). Read 3 under Wine at a fresh boot's main menu
  (its writer then **not traced**) and in an Easy Quick Race circuit.
* **Traffic level**: the Quick Race record's +0x04, a career event's
  +0x36, free roam 3 (0x540790); not tied to the difficulty.
* **Traffic skill** (traffic controller +0x799, its slot +0x78 =
  0x41E780): 9 / 4 / 0 for Easy / Medium / Hard, set by
  Traffic_CreateOnPoint (0x42C6E3) and Traffic_PlaceCar (0x42C98E); −1
  from the constructor (0x426623). Read 9 on every traffic car of an Easy
  Quick Race under Wine. Which code reads a traffic controller's slot
  +0x78 is **open** (the racer readers found — nitrous 0x40D850, the
  goal choosers, the target speed 0x41C5E0, the catch-up — are not run
  for traffic; its slot +0x80 is 0x408C80 and +0x9C 0x500BF0); not ported.
* Nothing else: no difficulty in options.txt's native counterpart beyond
  these, no reader in the payouts or reputation (the event's purse +0x30
  is fixed), the wrong-way timer or the traffic density. 0x5F9657 writes
  0x89E7F4 (32 bits, beside 0x89E7F8) into the replay / network stream.

**Effects on races**: through the skill row (AI tuning 0x81CB60 + 0x68 ×
row): the route speed factor +0x538 (0.74 … 1.00), the speed offset +0x584,
the catch-up delay +0x580 and line-speed pair lerp, the pass / avoid odds
+0x530, the junction odds +0x524…+0x52C, nitrous +0x570 / +0x588; the
Quick Race catch-up scale tuning +0x94C + 4·(4·difficulty + class) (all
1.0 as shipped); the AI's drift rate 0x79DAF0 (1, 1.25, 1.5; 0x56D838);
the downhill drift board 0x7FA5C0 (0x56DB2E); the drift assist's damping
0x7FC2D0 (0x5AC317); the human's drag shift window outside the career
0x7A19F0 (×3, 1.5, 1; 0x5B6189 / 0x5B61DE). The AI cars' upgrades follow
the entry's skill source and the player's rating, not the difficulty.

**Clone** (2026-11-01): the race's difficulty used to be the car's
handling difficulty, never set, so 0 (Easy rows) in every career race and
free roam; the throttle cap was 0x783A28[difficulty] (0.93 in every career
race); the Block row was the difficulty. Now `nfsu2::difficulty::RaceGlobals`
(Game::race_globals) is written as the set-ups write 0x89E7F4 / 0x89E804
(career race, Quick Race, city start), the career entries' rows, the
encounter rows, the drift rate / board, the drift assist (the human's and
the rivals' handling) and the drag shift window read it, the AI throttle
cap reads the level, and each rival keeps its Block row (+0x0B / 1).
The Career Difficulty row (Options → Gameplay, pause in career free
roam) and the Quick Race Difficulty / Catch Up rows were already there,
saved as the game saves them (profile +0x250, the mode records).

**Checked under Wine** (fastgpu, fresh profile, Quick Race circuit 4014,
3 laps, 3 opponents, traffic None, Catch Up off, the human parked):
0x89E7F4 = 0 / 1 / 2, 0x89E804 = 3 (never the difficulty outside drag),
0x89E7F8 = 0, the career difficulty 0x83AC30 = 1 throughout; the rivals'
+0x901 rows {0, 1, 2} Easy, {3, 4, 4} Medium, {6, 7, 8} Hard, +0x902 = 1,
the human −2 / 0. First-lap times (race clock, the 1st / 2nd / 3rd AI
racer to complete it): original Easy 107.5 / 113.1 / 117.0 s, Medium
97.5 / 99.0 / 102.5, Hard 89.1 / 91.5 / 94.5; the clone (`NFSU2_QR_MODE=
circuit NFSU2_QR_DIFFICULTY=d NFSU2_QR_CATCHUP=0 NFSU2_QR_TRAFFIC=0
NFSU2_BENCH_NOGAS=1 NFSU2_BENCH_RIVALS=file --bench race`; its own row
draws {0, 0, 1}, {3, 3, 4}, {6, 6, 7}) Easy 107.5 / 108.0 / 112.0, Medium
98.5 / 101.5 / 107.5, Hard 96.0 / 96.5 / 98.5. The ordering and the Easy /
Medium times match; on Hard the clone's racers are about 6 s a lap slower
than the original's (the high rows' speed, not the difficulty's
wiring — the rows and globals read the same). Career events were not
raced under Wine for this; their rows follow 0x5261F0 (decompiled).

**The high rows' speed** (2026-11-01; the same Quick Race, rivals'
cars and rows forced equal: racer +0x901 written under Wine, the clone's
`NFSU2_BENCH_RIVAL_ROWS` / `NFSU2_BENCH_RIVAL_CARS` / `NFSU2_BENCH_SEED`,
per-frame logs `NFSU2_BENCH_AILOG`; cars type 4 / 3 / 0 = MIATA, 240SX,
PEUGOT, the clone's car-type order taken as the game's, **inferred**):
* **Cause 1, fixed: the junction draws.** 0x413CC0 draws its odds
  (0x413DCB type 4, 0x413E57 the retyped 0 / 1 by row +0x524…+0x52C) from
  the world's generator (0x5EA510 with ECX = 0x890080, +0xF8), shared by
  every racer in turn. The clone gave each racer's walk a default-seeded
  generator, so every racer drew the same numbers in every race; on 4014
  the draw at junction 199 (record 423 → the cut-through 896, retyped to
  type 0) was 0.851, above +0x528 for every row but 9 (0.85 at row 8), so
  no racer ever took it (about 2–3 s a lap). Its route data match: the
  original's memory at the race (444 junctions' flags, every point's lane
  speeds 0/1) equals the clone's processed file (`NFSU2_BENCH_ROUTEDUMP`).
  `racer_think` now hands the walk the race's world generator
  (`Race::cars.rng`).
* **Not the cause** (traced or measured): the speed factor +0x538 and the
  target speed 0x41C5E0 (the logged targets agree); the AI cars'
  performance (a Quick Race rival's handling block, car +0x20, 0x300
  bytes, equals the human's stock 206 for the rival 206); nitrous (Quick
  Race rivals' tank is empty: car +0x454 = 0, no shot all race, nor the
  refill below); the throttle cap (0x89E804 = 3).
* **Cause 2, fixed 2026-11-01 (see Racer movers → BeltMover): the mode-4 mover.** With the human parked at the
  start the rivals leave the loaded world about 30 s into the lap (car
  +0x5CC null) and drive on mode 4 (car +0x28 = 4, the path mover
  0x5A9040, update 0x59EC90: car +0x42C = (s·+0x24 + +0x20) / +0x10 of
  its path segment, no tyres) until about 95 s, then 3 again (read under
  Wine every 3 s). On mode 4 they run 2–4 m/s under the driver's target
  (0x230) where the clone's MeloMvr car tracks it, so the original's
  first lap is close to 84 / row factor at every row. The clone keeps the
  whole world loaded and has no mode 4 (see Racer movers); the segment
  speeds' writer and the loaded-world rule (section streaming) are not
  traced.
* **Measured** (first lap, s; original Wine runs / clone over six seeds,
  three cars each, mean): row 1 (Easy) 109.4 / 106.6; row 4 (Medium) 98.5
  / 98.1; row 6 90.6 (forced; 93.8–95.0 unforced) / 95.0 (median 93.2);
  row 8 88.0 / 91.8 (median 90.6). Before the fix the clone's rows 6 and 8
  read 94.2–95.9 and 90.8–95.2 (one seed: the draws did not vary). The
  remaining ±2.5 s follow mode 4 (faster than the original at low rows,
  slower at high), with seed-to-seed spread of ±5 s from the junction
  draws and start-pack contacts. **Open** too: a clone rival that hits a
  wall in the start pack can stay wedged for the race (GetUnstuck drives
  only the reverse speed). **Correction** (2026-11-01): 0x407400 is
  the human's hydraulics input (HumanInput_HydraulicsFrame, from
  0x41E2C0), not a racer recovery. The racers' recoveries are GetUnstuck
  (ported), AccidentRecovery's reset (ported), the think's route reset
  (0x42AF45: 0x428720 false — an empty walk list, or the car's place past
  its last point, 0x428C57…0x428CB3 — with the goal's slot +0x1C true
  (0x40CA10: goal type 3, Win road race) and 0x89E7B0 ≠ 7 → 0x5EBF70, car
  +0x96C = 1; **not ported**) and mode 4 far from the human (BeltMover).
  The seed-6 wedge (Quick Race 4014, any row, the Peugeot at (−292, 105)
  from 7 s: GetUnstuck reverses ~3 m in its 1.3 s, ends facing the route,
  drives into the same wall, again every 6 s) stays **open**: the car
  stays on its walk, so the route reset would not fire; the cause in the
  clone's GetUnstuck path (0x404CD0 / 0x41D530, the 0x423E20 edge branch)
  is not found, and the wedge was not reproduced in the original. **Round 2** (2026-11-01; `--bench race`, seed 6, AI log
  columns heading / forward speed / GetUnstuck / steer / contacts / the
  driver's look-ahead): the Peugeot leaves the road north at 5.8 s
  (heading 111°, the pack) into a pocket at (−289…−293, 100…110) between a
  wall to its west and the road 15 m south (the line at y ≈ 88). It is on
  the MeloMvr (mode 3) throughout, not on the belt. Each cycle: forward
  along heading −140…−175° into the west wall; it pivots left against the
  wall to about −85° (facing the road, nearly free) at 1–2 m/s; then the
  side contact (class 2/3, ≥ 0.4 s old, ≤ 5 m/s) and the 6 s cooldown
  start a GetUnstuck, which reverses it north 4–8 m back into the pocket
  until it faces the facing point (cos ≥ 0.9, about −140°), and the cycle
  repeats. While reversing the driver's look-ahead is at (−317, 105…112),
  north-west, parallel to the line: the path is planned from the car at
  its offset. GetUnstuck's own lane path is now ported (0x423E20 →
  0x420D40 on lane record +0x08 = 0.25 per metre, 0x783718, offset 0,
  first planned from the car as 0x420940's 0x41D160(0, 0) drops the path;
  `UNSTUCK_LANE_RATE`, `Unstuck::first_path`; the actions are not
  thought while it runs, since it is the running action): the cycle is
  unchanged. **Open**: why the original would leave the pocket (its
  trigger rules are the clone's; the reverse steering and 0x41D530's
  start offset in a fresh plan, and the gated block 0x423E95…0x423FD2,
  are the candidates); the original was not put in this pocket.
  **Round 3, fixed** (2026-11-01; Wine, a rival
  teleported by its rigid body (car +0x34 → +0x20) into the pocket at
  (−291.5, 102.5) heading −140°, standing, logged every tick:
  `wait_tp.py`, logs `w/a4.tsv`, `w/a6.tsv`):
  put there at race clock 7 s (in its start hold), the original never
  starts a GetUnstuck: a contact record against the west wall at the
  front-left corner (along +1.7 m, right +0.8 m) held 0.4 s, pushed each
  frame, at under 1 m/s and four wheels down, yet the action stays Pass /
  JustDrive; the pinned steering (record > 0.6 s old) turns the wheel to
  full left lock over a second (+45° at 9.2 s), the car pivots from −145°
  to −90° by 10.8 s at ~1 m/s, the record ends and it drives out south
  onto the road (out by 11 s, 4 s after the drop). Put there at 13 s (past
  the hold, 101's 11.5 s), it backs off at once: GetUnstuck (type 9,
  priority 500, target −15 m/s) reverses 2.3 m in 1.4 s with the wheel at
  +6…+16° (heading −143° → −164°) until a rear contact ends it (slot 7);
  forward into the wall again, a second GetUnstuck 6 s later, then the
  pinned pivot gets it out at 25 s. So the original cycles in this pocket
  too once the hold is over, but the seed-6 drop at 5.8 s falls inside the
  hold, where 0x40E7C0 returns −1 (0x5EF000). The clone had no hold in the
  stuck test (only in the junction walk and the actions); with it the
  clone's Peugeot (number 102, hold 11 s) pivots out the same way (heading
  −124° → −82° between 9 and 10 s, on the road by 12 s, lap normal).
  The mover stayed mode 3 throughout in both. **Open**: the clone's
  reverse ran 4–8 m north where the original's ends on a rear contact
  after 2.3 m (the obstacle behind, about (−287, 105), was not identified
  in the clone's collision); the route reset (0x42AF45) is still not
  ported.
* **AI nitrous outside drag** (traced, verified from the disassembly;
  **ported** 2026-11-01, `ai_nitrous_wanted` in game.rs; Quick Race
  rivals have no tank, see above; career rivals' tanks follow the player's
  nitrous level, `rival_fits`). The racer
  controller (vtable 0x785370) slot +0xA0 is 0x416F70: false for car
  +0x28 = 4 or no racer status; in drag straight to 0x40D850. Else, with
  st = the lap part past the line (wrap(100·car +0x8F8 − race status
  +0xBF8[+0xBF6]) by 0x5D5D20, +100 when negative), last = status lap
  +0x09 = laps − 1, end = last and 0.01·st > 0.75: with row byte +0x589
  (rows 5–9) a tank under 1.0 s (0x592490) refuses unless end or race type
  7; a running shot longer than row +0x574 (1.5 … 4 s; 2 s in type 7)
  refuses; with no shot the time since the last shot ended (+0x7AC) must
  pass row +0x568 (10 … 5 s), +0x56C on the last lap (5 … 2), 0 at the
  end. Then it wants the shot (0x40D850 decides) when the controller's
  lane (+0x30C) is 2–5, or v / (the car's FuturePoint lane speed, 0x40AFC0;
  else the road node's byte, 0x4024F0) < 0.8 (1.0 in type 7), or the
  FuturePoint lane speeds have not dropped from the car's point to the
  list's end (0x412220) for more than 1 s (timer +0x7B0). 0x420810 stamps
  +0x7A8 (slot +0xA4, 0x41E4F0) when it starts a shot; the think 0x42ABD0
  ends a shot that has run row +0x570 s (slot +0xB4, 0x408430) when
  0x40D850 fails (its light path) or 0x416F70 fails (its full path, every
  0.2 s on the racer's turn of 8), then stamps +0x7AC (slot +0xA8,
  0x41E500). The AI's timed refill (0x601795, under 0x89E7E7, which reads
  1 in this Quick Race): every second 0.5 × the lap fraction run since the
  last (0 when 0x89E7A0's 0x4018F0 holds) through 0x592650.
