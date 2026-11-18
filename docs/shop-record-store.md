# Visual shops: the car record store (2026-11-04)

Follow-up to `docs/perf-shop.md` "Car record store" (the performance-shop save fix, performance
shop). Question: do the visual shops store the ride into the career car's
record as the original does, and what does the original do with the stale
derived slots of the player's a community save (RX7, IMPREZAWRX, ESCALADE,
LANCEREVO8: door sill right, hood under, wide-body decals)?

## Traced (SPEED2.EXE, **verified** by code)
* Every write of a part to a ride goes through Ride_SetPart **0x631D30**
  (ride +0x590 + 4·slot; slots 0x61–0x68 refused, slot 0x19 only read)
  and ends with Ride_RefreshSlots **0x61BCE0**, which rewrites the
  derived slots (`car::ride_slot_parts`: a wide body's KITW0n doors /
  panels / sills 0x11–0x16 and WIDEn decals 0x3B–0x3E, no wide body's
  first door parts, slot 0x19 by the hood's HOODUNDER 0x0D4B85C7 or the
  first part).
* All 18 calls of Career_StoreRide **0x503950** copy the career ride
  (shop 0x838980 + 0x940 = 0x8392C0) into the car record ([shop+0x2C]
  +0x18): performance 0x5213D4 / 0x521433 / 0x52147C / 0x5214CC /
  0x521AFA (after 0x639CF0(1, …)); the setup purchase 0x547580
  (paints, vinyls, decals: each item 0x631D30, then 0x547791); the
  generic visual fit Shop_InstallVisualPart **0x55C240** (0x55C35A; called
  by the body shop 0x5664A0, unique part 0x561730, gauges 0x563D10,
  decal main 0x568D40, parts list 0x56A310, 0x55E350); the rims
  0x55F2FC / 0x567C1B / 0x567CF9 and spinners 0x569780 / 0x569858 (0x1D
  and 0x1E by 0x631D30); the gauge colours 0x563E0F … 0x563FBA (0xA4,
  0xA6, 0xA5); the new stock record of the car lot 0x516C52. None of the
  visual paths runs 0x639CF0: the record takes the ride's upgrade state
  (+0x2F0) as it was built.
* 0x55C240(slot, part): 0x631D30; slot 0x1D also 0x1E; 0x22 → 0x5474B0
  (0x23–0x2E take 0x53AEB0's part); hood (9) with CARBONFIBRE 0x721AFF7C →
  0x50EC10 (vinyl layers of kind 0x14 off) and a hood of level ≥ 1 →
  0x50EC50 (0x61–0x68 to 0, refused by 0x631D30: no effect); slot 0x5D of
  level ≥ 1 → 0x50EC80 (vinyl kinds 0x0E, 0x0F, 0x15–0x1A off) without a
  wide body, 0x50ECE0 (every vinyl but kind 0x14 off) with one; a wide
  body over a 0x5D of level ≥ 1 → 0x50ECE0. Then 0x413440 (shown ride +0x50
  = copy), 0x4D8EC0, 0x503950, 0x55B1B0.
* Career_BuyPart 0x528A00 (all the visual purchases above): bank less
  0x514280's price (floor 0), owned bit 0x503D50, and the current slot's
  spend +0x7F0 grows by the price (formats.md "Total Spent On Car"
  corrected: it said only 0x547580 writes it).
* Loading: CarObject_BuildRide 0x516AC0 → 0x503880 fits every record slot
  by 0x631D30, so the **ride** always has correct derived slots; the
  **record** is not written back (0x503880 has no store). A load and
  resave keeps stale slots (the performance-shop fix's control save).

## Observed under Wine (vanilla, fastgpu :221, CC_BODYSHOP_1, a community save-latest)
Memory reads (peek.py) of the records 0x8406E4 + 0x7F8·i and the ride
0x8392C0, saves after each visit (internal fixture saves of a community career,
raw dumps (saves, memory, Wine runs) kept outside the repository;
summary internal evidence):
1. Load, garage, free roam: records unchanged (stale slots kept).
2. LANCEREVO8 (wide body kind 4, TRAP hood with HOODUNDER), hood ERASER
   (no HOODUNDER): record 0x09, 0x16, 0x3B–0x3E rewritten, 0x19 stays the
   first HOOD_UNDER part, 0x0D / 0x1B the performance styles, upgrade
   floats 0 → [3, 2.98, 3 ×8], category 9 fitted 0x3A/0x3D/0x40/0x43,
   spend +1000.
3. Car Select to ESCALADE (no wide body) and save: no record changes.
4. ESCALADE front bumper → STOCK ("install this part now", free): record
   0x01, 0x0D, 0x19 (CC559D1E → 9C070D81), 0x1B; category 9 keeps 0x3A
   (the ride's state on entry).
5. Next visit (new launch): the ride on entry has 0x3A off, float[9]
   2.25; ORION (level 1) bought: the record keeps that state.
6. RX7 (wide body, HOODUNDER hood, every derived slot stale), spoiler
   JORGEN: 0x0C, 0x0D, 0x16, 0x19, 0x1B, 0x3B–0x3E rewritten =
   the maintainer's probe `record_derived`'s expected values.
* A second shop visit in one session hung the original twice (frozen, FE
  clock 0 %); every visit above ran in a fresh launch.

## Clone
* `Profile::fit_visual` (0x55C240's fit and side effects),
  `Profile::store_visual` (0x503950 after a visual action: the part slots
  with 0x0D / 0x1B and the derived ones, category 9 from the body parts
  as on entering the shop — `Profile::build_shop_ride` on Enter Shop,
  `shop_ride` cleared on leaving — and the ten floats), shared
  `store_parts` with the performance-shop save fix's `store_ride`; `car::with_derived_slots`,
  `car::trunk_component`.
* Callers: `fit_browser_part` (body shop, rims, spinners, specialties,
  neon), the carbon toggle, the gauges, `take_unique`, `purchase_setup`;
  0x528A00's spend added to the browser, toggle and gauge purchases.
* A stale record loads and resaves unchanged; the ride the clone drives
  (`player_car`) and rates (`visual_rating_car`) has the derived slots,
  as the original's ride.
* Test `body_shop_installs_store_the_ride_as_the_original`: the four
  visits byte for byte (the car slot; other slots untouched), the control
  resave, and the stale ride's derived slots against the original's
  stored ones. Tool: a private developer probe (`record_derived SAVE…`).

## Open
* Main-menu Customize: what 0x503950 stores there (the record behind
  shop +0x2C outside a career) is not traced; the clone stores only the
  part slots (`scratch`).
* Paint / vinyl / decal purchases (0x547580) and rims were not run under
  Wine; their store is the same 0x631D30 + 0x503950 by code.
* The ride's state when no shop was entered first (a fit outside a
  shop visit: tests, shots) uses the parts before the action
  (**inferred** equivalent).
* The second-visit hang of the original under Wine (not investigated).
