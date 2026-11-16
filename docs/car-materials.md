# Car special materials: carbon fibre and chrome trim (2026-11-04)

The visual model review found the carbon parts light grey and flat and the FUSION chevrons flat white
(internal evidence, `g35_fusion_hood_wine_vs_clone.jpg`).
Those the visual model review crops came from two different scenes: the original's Body Shop and the clone's sheet tool,
which has its own lighting and env cube. This work compared both in the **same scene**: main menu →
Customize → G35 (and LANCER EVOLUTION VIII) → Car Shop → Body Shop. The original ran under Wine
(`fastgpu.sh`, own copy `a private Wine copy`, display :211, profile from `career_end_won`). The clone ran
with `NFSU2_SHOT_SAVE=tests/fixtures/native_saves/career_end_won.sav NFSU2_SHOT_KEYS=… --shot custcars`
and the same key path. Raw frames are in the work dir.

## Cause: carbon parts took the paint's light material (fixed, native path)
The texture side was already traced and ported (`Carbon`, 0x636850 / 0x634B0B…0x634E69). The light
material was not. **Verified** in the disassembly:
- 0x638DA9…0x638DBF: render info +0x388 = `LightMaterial_Find(bin_hash("CARBONFIBER"))`. The string is
  at 0x7A8D2C; GLOBALB's `CARBONFIBER` is 0x721AFDDC: diffuse 0.58 + 0.46·n·v, env 0.1165, alpha
  1 − 0.556·n·v. +0x384 is the paint, +0x38C the windshield tint.
- 0x634B0B…0x634E69 bind the texture tables and store the choice:
  - +0x3B0 / +0x3B4: front / rear bumper on +0xAC8, when KIT_CARBON is fitted.
  - +0x3BC: the hood, with its `CARBONFIBRE` attribute.
  - +0x3C4: the trunk, with TRUNK_CARBON.
  - +0x3C8: the wide body, and the doors on a wide body, with the kit.
  - +0x3B8: the body, the skirt and the plain doors on +0xF58, i.e. the kit fitted.
- The generic part draw (0x623071…; slot switch 0x623136, tables 0x630D68 / 0x630DA8):
  - [esp+0xBC] starts as the paint +0x384 (0x622F1D → [esp+0x178] → 0x6230AB). Slots 1, 2, 6, 9, 0xA,
    0x11 and 0x12 set it to +0x388 when their flag is set (0x623369, 0x62335A, 0x62341B, 0x623394,
    0x6233E5, 0x623207).
  - 0x6231B3: `0x48D860(model, CARSKIN 0xD6D6080A, [esp+0xBC])`.
  - 0x6231E1…0x623456: `0x48D860(model, 0xA7366AE6, +0x3B8 ? +0x388 : +0x384)`.
  - Spoiler, roof scoop and mirrors are drawn by their own code (0x6230DD's table skips them) and
    never get these remaps.

The clone drew every carbon range with the paint's material. The carbon texture is DXT1, alpha 1,
twice the composited skin's GLOSS weight (0x80). So the carbon got the paint's diffuse/specular/env
at double strength: light grey in the showroom, a yellow mirror on the a community save Lancer in game.
**Fix** (`car.rs`):
- `CARBONFIBER_MATERIAL`.
- `CarPart::carbon_kit`: generic-draw slot and KIT_CARBON fitted.
- `Car::part_light_material_for`: a `Full` part's CARSKIN, and with `carbon_kit` the 0xA7366AE6
  references, take CARBONFIBER.

That +0x3B8 equals "kit fitted" assumes a body or skirt model is always bound (slot 5 is). This is
**inferred**: the flag is only written by those bindings.

## Same-scene results (the maintainer's private captures, region means 0–255 R G B)
| scene | region | original | clone before | clone after |
|---|---|---|---|---|
| G35 DOUBLE-V CF hood (`g35_doublev_cf_hood_orig_before_after.jpg`) | hood left 100×50+430+500 | 103.5 101.6 106.5 | 96.7 94.5 103.3 | 104.1 102.4 106.3 |
| | scoop 100×40+610+480 | 212.6 195.7 218.3 | 213.7 194.2 225.8 | 212.7 195.9 217.7 |
| | hood 300×150+420+440 | 179.9 174.5 184.1 | 178.0 172.3 184.4 | 180.0 174.7 184.0 |
| G35 carbon body kit (`g35_carbon_kit_orig_after.jpg`) | front bumper 300×40+420+600 | 140.3 134.5 149.6 | – | 138.7 132.9 147.5 |
| | 200×40+400+560 | 192.8 187.9 203.8 | – | 192.5 187.4 203.6 |
| Lancer carbon trunk, rear (`lancer_carbon_trunk_orig_before_after.jpg`) | trunk 160×50+800+480 | 52.6 49.8 45.9 | 44.9 43.1 42.1 | 52.9 50.2 45.4 |
| | 100×40+700+470 | 92.6 85.4 62.9 | 90.8 84.5 62.0 | 92.2 85.5 61.3 |

The carbon now shows the original's yellowish weave sheen and agrees within about 1.5 levels.
internal evidence (the community-save Lancer, free roam) shows the player's car in free roam: before, a mirror trunk and
quarters; after, carbon.

Rachel's 350Z (DDAY_PLAYER_CAR, a hood with the CARBONFIBRE attribute) on the main menu
(`--shot main_dday`, the car-paint comparison's rectangles, original from internal evidence):
- hood 150×40+730+540, original − clone: before +1.6 +5.3 +2.5, after +1.4 +1.4 +1.3;
- car 600×260+330+450: before +0.8 +0.9 +0.5, after +0.7 +0.4 +0.3.

## Chrome trim and the CF spoilers: no bug in the native path
- FUSION (STYLE14) chevrons: texture `CHROME`, light material `CHROME` (diffuse 0, env 0.57). In the
  same Body Shop frame the clone matches the original (`g35_fusion_hood_orig_clone.jpg`). Only the
  UI's blink differs; the car pixels are identical before and after this fix.
- The flat white in the visual model review's sheets comes from the sheet tool's env cube and lighting, not the material.
- The CF spoilers / roof scoops (`CARS/SPOILER`, `ROOF` `*_CF_A`) reference light material
  `CARBONFIBRE2` (0xB57AEF2E). GLOBALB has no such material, so 0x48F6A0 returns DEFAULT in the
  original as well. The ROCKER 5 spoiler matched in the same scene.
- Enhanced Car Paint maps CHROME to its Chrome type and CARBONFIBER to Native (no layer). Carbon on
  Enhanced looks as on Original (`g35_doublev…` row, same frames). The chevrons with Enhanced Car Paint
  are in `g35_fusion_hood_orig_clone.jpg` (third tile).
- RAGE / SLINGBLADE / SWEEPER / ROADRUNNER were not shot one by one. They use the same CHROME material
  path as FUSION.

## Carbon Fiber → Convert Trunk / Body Kits asks first (ported)
UI_GenericParts_Browser's handler 0x5664A0 on Accept (0x406415E3) with +0x1D5 set (the carbon
fibre strip), **verified** in the disassembly; label texts from the game's language file:
- Trunk (slot 0x60, 0x566686..0x566728): with the "none" part 0xC8D313AC fitted (0x610300) "Do you
  want to convert your trunk to carbon fiber?" (0x2B950C36), in a career (0x850078 = 1) with that
  fitted part not owned (0x521BF0) "Do you want to purchase the ability to convert …" (0x3175F2B0);
  with the carbon trunk fitted "Do you want to revert your trunk back to stock?" (0xA645A29E).
- Body kits (slot 0x5D, 0x566895..0x5669E3): the same pair 0x9E274DAA / 0xB7C84DA4, or 0xEE2C85BB
  ("… This will cause some vinyls to be removed from your car.") when a vinyl on layers 0x40..0x43
  conflicts — with a wide body fitted (slot 6, level above 0) any vinyl kind (+5 & 0x1F) but 0x14,
  without one the kinds 0x0E, 0x0F, 0x15..0x1A (jump table 0x56787C / 0x567884); revert 0x660BDF52.
- 0x5587A0: Yes / No (0xF770 / 0x061C), Yes 0x51123592, No and Back 0xB4623F67, No focused (its 1);
  0x838970 = 0xB4623F67; the strip hidden (0x52EBA0(0)); StartDim (0x537C10 "DIM"); FE sound 8
  (0x51DE40).
- Yes (0x566F12 → 0x5670F6): bought in a career when not owned (0x528A00), fitted (0x55C240), the
  strip built again (0x545D40), the install sound (0x521290), the strip shown and UNDIM. No
  (0x5674D8): the strip shown again, nothing fitted.

Neither the question nor its answers call the camera (below, "The camera's turn to the rear"). Clone: `parts_toggle_key`,
`Game::carbon_question`, `Game::carbon_convert`, `LotDeal::Convert`. Test
`tests::native_fe_part2::carbon_convert_asks_first`. Not re-run under Wine for this change.

## The carbon hood's warning (ported)
Accept on the carbon fibre strip's Hood (slot 9, +0x1D5 set, +0x1D9 clear: no list open; 0x566786,
**verified**): a vinyl of kind 0x14 (+5 & 0x1F) on one of the layers 0x40..0x43 shows a one-button
warning first — "Warning! Installing a carbon fiber hood will cause any hood vinyls to be removed from
your car." (0xB3BBB11D), or "… hood vinyls and decals …" (0x9796DFB4) with a decal in one of the slots
0x61..0x68 (0x558440; OK 0x34DC1BEC). The browser plays MENUFADEOUT (0x537C10, "MENUFADEOUT" 0x79BE40)
and the hoods' list opens under it (0x566862 → 0x55EDE0, which sets +0x1D9 = list open); OK runs
0x5674D8 (strip and list shown, UNDIM 0x79031C). Clone: `Game::carbon_hood_warning`, `parts_step`.
Test `tests::native_fe_part2::carbon_hood_warns_about_hood_vinyls`.

## The camera's turn to the rear (open)
No path of the browser's handler 0x5664A0 for the carbon strip moves the garage camera: the convert
question, its Yes and No, and the strip's own Accept make no 0x4C2100 call. The handler's only delayed
view request is on its tick (0xC98356BA at 0x567645: +0x1D6 set and over 0.5 s since +0x1CC, then
0x50E580(slot, the list's part) → 0x4C2100), and nothing in the executable sets +0x1D6 but the
constructor's 0 (0x55E930): dead. The list's opening (0x55EDE0) requests the view of the highlighted
part (0x55EEE8); the trunk and the kits have no list. Where the turn seen under Wine comes from is
**open**.

## Seen on the way (open, not changed)
- Re-entering the original's car select lists the tuned car at the end.

## Tests
`car::shared_part_tests::carbon_parts_take_the_carbonfiber_material` (skips without NFSU2_ROOT):
- the stock G35 draws no CARBONFIBER;
- the DOUBLE-V CF hood draws its CARSKIN ranges with it, and the rest keeps the paint;
- with the carbon kit, the bumpers' CARSKIN and every 0xA7366AE6 range take it, and the plain doors
  keep the paint;
- the Lancer's TRUNK_CARBON lid takes it.

The probes the maintainer's probe `part_materials` (texture / light material / flags per range) and
the maintainer's probe `save_parts` (a native save's fitted parts) were used to find the materials.
