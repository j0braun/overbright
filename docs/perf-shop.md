# Performance shop: individual parts, packages, brand screen (perf-shop round, 2026-11-04)

Issue: in the performance shop parts could not be bought one by one, only as packages,
and the brand screen was broken.

## Wine walk (vanilla SPEED2.EXE, fastgpu.sh, 1280×960, no WidescreenFix)
Saves: a community save (stage 4, LANCEREVO8, every part fitted, nothing
owned) and a control run (load, enter, exit, save, no purchase). Shop
CC_PERF_SHOP (player moved onto its trigger). Captures and saves are in
`<work dir>/perfshop-work/{wine,saves}`; side by sides in
the maintainer's private captures (left original, right clone fixed; `brand_part`
original / clone before / clone fixed).

Key by key: categories → Engine opens on Choose Individual Parts (the
parts list, 4 rows, CC at stage 4 lists the level 1–2 parts: 7 rows) →
Right to Street / Pro / Extreme (locked) / Return To Stock → "Re-Install
Stock Parts?" Yes → Left ×4 → a row → "Would you like to purchase this
part?" (No focused) → Yes:
* a part **with brands** opens UI_PerformanceBrandSelect; Select buys and
  fits it and the shop comes back **on Choose Individual Parts with the
  list on its first row** (`after_brand_buy`);
* a part **without brands** (Larger Diameter Downpipe, Differential) is
  bought at once and the **row stays highlighted** (`after_direct_buy`);
* a package (Pro: Cat Back, High Flow, Downpipe) asks "purchase and
  install this performance package?", its brand screen has one row per
  branded part (2), Select buys and the shop is back on Choose Individual
  Parts' first row. The replaced Performance Exhaust reads "Owned".
* Back (Esc) on the brand screen does nothing (pressed twice; the help bar
  has Continue / Quit Game only).
* Brand stepping: Right goes backwards through the part's brand list,
  Left forwards (Performance Exhaust: AEM → Right HP Racing → Jackson
  Racing → Neuspeed; Left back to Jackson).

## Traced
* 0x55C940 (UI_PerformanceBrandSelect): Select plays 0xDE6EFF34, its end
  0xE1FDE1D1 runs 0x521F00 (rows' brands → 0x8389B4… in row order), then a
  part 0x521B10 + 0x5213E0 or a package 0x54FE60 + 0x521350, then 0x54F990
  and 0x55BD70 (back: a new package screen).
* 0x521EA0 (row's arrows): 0x9120409E (Left) index + 1, past the end 0;
  0xB5971BF1 (Right) index − 1, below 0 the last. Formats.md had the pair
  swapped; Wine agrees with the code.
* 0x521B10: bank −= price; 0x503E10(part, slot 0) (owned bit +0x15C,
  brand appended to the first empty of 0x44 at +0x2AC — a 0 leaves it
  empty); car +0x7F0 += price; slot 0 := 0xFFFFFFFF.
* 0x54FE60: bank −= 0x5415F0's total; for each package part i not owned
  (0x530360): 0x503E10(part, slot i), +0x7F0 += full price; every slot i
  := 0xFFFFFFFF.
* 0x550340: one row per part with brands (first four), enabled flag
  `1 < count`; with fewer than two rows the hint group 0x6D017BC5 hides.
* So a purchase without the brand screen appends whatever its slot holds:
  0 at boot (nothing), 0xFFFFFFFF once read. **Verified** in the
  original's saves: downpipe first → no entry; Cat Back (AEM) then
  Differential → [0x9152, 0xFFFFFFFF]; the player's sequence → brands [HP
  Racing, Jackson, HP Racing, DC Sports], owned {15, 17, 19, 20, 21},
  spend +0x294 = 3000 (full prices; the package's 10 % off only in the
  bank).

## Brand screen layout (measured, mechanism **inferred**)
Rows 76 apart (title master height) from the data master's top; in a row
the logo (OPTION_IMAGE_n, 89.6×44.8) on top centred on the data master,
the brand name under it, the part name and the arrows halfway down both;
backings stay at their authored places (four always). Arrows only on the
highlighted row (and only with two brands or more, **inferred** from the
enabled flag). Bar: 0x538090(1, rows, sel): one row bare track, two rows
arrows and handle. The highlighted name pulses (its HIGHLIGHT, 1.2 s loop),
its brand green.

## Mouse (observed)
UI_BuyPerformanceParts: a move over a row highlights it (handle follows;
the readouts stay as they were); Enter then buys that row; a single click
on a row does nothing. Brand screen: a held click on the highlighted row's
arrows steps the brand as the keys do. The clone: `fe_mouse` arms for
both, `--shot mouse_perf`. Row hover on the brand screen and the bars'
arrows are **inferred** from the option list class (not exercised).

## Clone changes
`options::BrandSelect::quads` (layout above, bar, hint, mouse rows),
`BrandSelect::key` (direction), `Game::perf_brand_slots` (0x8389B4),
`perf_confirm` (slots, spend, the screen after the brand screen), Back
ignored, `PerfParts::quads` mouse rows. Shots: `brandselect_part`,
`brandselect_pkg`, `perf_part_ask`, `mouse_perf`. Test:
`perf_shop_individual_parts_packages_and_brands` (career_end_won.sav,
the Wine sequence, the written save fields).

## Car record store (2026-11-04)
Every performance shop action ends the same way: Shop_FitPart 0x5213E0
(0x5A6330(part, 1, 0) on ride +0x2F0), Shop_FitPackage 0x521350 (each
package part so), Return To Stock 0x521A30 (each category part
0x5A6330(part, 0, 0)) and the unique 0x521440 run 0x639CF0(1, 1, 1, 1),
0x4D8EC0(ride, 1, 0) and, the ride being the career car's (shop +0x940),
**0x503950** (car record +0x18): all 0xAA part slots from the ride
(0x610300: ride +0x590 + 4·slot, the part's first dword, 0 none) and the
ride's 0x94-byte upgrade state (ride +0x2F0 → record +0x2B4). A resave
without one leaves the record as loaded (the control save: floats 0).
* 0x639CF0(1, …) → 0x6399E0(refit 1, dress 0): category 9 is refitted
  (0x5A6890(t, {9}, 1)) to t = mean of 0x599610(8, 5, 0) (float / 3) —
  the floats the last 0x5B4FD0 wrote, i.e. **before** this action; then
  slot 0x0D (ENGINE level 3 / 2 / 0 by the engine, ECU and turbo packages,
  0x599640) and 0x1B (brakes by 0x28 / 0x29 / 0x2B), each followed by
  0x61BCE0; then 0x5B4FD0 → 0x5B4DD0 → 0x5AC500(·, ride +0x2F0) writes the
  ten floats (3 × 0x5A6590 per category).
* 0x61BCE0 derives slots: wide body (slot 6) of level ≥ 1, kind n (byte
  +5 & 0x1F) → 0x3B–0x3E `<CAR>_WIDE<n>_DECAL_{LEFT,RIGHT}_{DOOR,
  QUARTER}_RECT_MEDIUM` (0x61BF6B) and 0x11–0x16 `<CAR>_KITW0<n>_DOOR_`
  LEFT / RIGHT / PANEL_LEFT / PANEL_RIGHT / SILL_LEFT / SILL_RIGHT
  (0x61C02B); no wide body → 0x11–0x16 the slot's first part (0x61B990,
  hash 0 = any); hood (slot 9) → 0x19 the part its HOODUNDER attribute
  names. Loading (0x503880) builds the ride from the record and runs
  0x639CF0(0, 1, 1, 1): category 9 from the body parts (`fit_aero`).
* Readers of the stored state: 0x503880 copies it back into the ride on
  loading, where 0x5AC500 rewrites the floats from the fitted bytes before
  anything rates the car, so the saved floats feed nothing directly; the
  fitted bytes feed the physics (0x5AC500), the shop/dyno ratings and
  0x6399E0; the floats as recomputed are read by 0x5995F0 / 0x599610 (the
  nitrous level, the turbo sound 0x7F2230, the AI targets 0x6374C0,
  0x6399E0's refit). Visual rating 0x5234D0 reads the ride's slots (so
  the derived ones count: the wide-body sill is level 1).
* **Verified** against the original's saves (the performance-shop fix's Wine runs on a community save,
  slot 1 LANCEREVO8, wide body kind 4): run 1 (Engine to stock, Headers
  HP Racing, Exhaust Jackson, Pro package) → floats [3, 2.98, 3, 1.35, 3
  ×6], fitted +0x3A/0x3D/0x40/0x43 (refit to 1.0), ENGINE STYLE02
  (142C4B7D), BRAKE STYLE01, sill 09F68FFB, hood under 5E8E70A6, decals
  D51E8194…; run 2 (Engine to stock, downpipe, Cat Back AEM, Transmission
  to stock, Differential) → [3, 0.33, 3, 0.75, …], ENGINE stock. The clone
  now writes the same career slot byte for byte (0x7F2 bytes).
* Clone: `Profile::store_ride` (refit, `car::perf_visual_parts`,
  `car::ride_slot_parts` into `installed`; `record_levels` for the
  writer), called from `Game::perf_confirm` and the unique perf grant.
  Test `perf_purchases_store_the_ride_as_the_original` (fixtures
  internal fixture saves of a community career (control, after buys, unbranded)):
  both runs byte-identical, and the clone loading the original's save
  gives the same shop ratings and visual rating as after its purchases.
  Evidence: internal evidence.

## Open
* The strip's frame on Choose Individual Parts (the green frame round the
  icon): on in some captures, off in others (a pulse is likely; the clone
  keeps it hidden). Not settled in the performance-shop fix (two Wine restarts hung on the
  garage → free-roam load) nor in the performance-shop save fix (no Wine run: the record work
  was settled from the saves the performance-shop fix took).
* After a purchase the original's ride holds the **refitted** category 9
  (0x6399E0 refit = 1) until the ride is next built from the record
  (0x503880 fits it from the body parts again); the clone's handling and
  ratings always take the body parts' aerodynamics (`Game::fit_aero`).
  When the player's ride is rebuilt after leaving the shop is **open**;
  a reload of the save matches (test below).
* ~~The clone's own body-shop installs do not run 0x61BCE0's derived
  slots~~ — settled by the body-shop slots fix: every visual shop action now stores the ride
  as the original (`docs/shop-record-store.md`); the stale a community save records
  stay as loaded until the car's next shop action, as in the original.
* A package with no branded part: whether the brand screen opens empty
  (the clone buys at once, **inferred**).
