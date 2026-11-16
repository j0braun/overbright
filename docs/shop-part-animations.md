# Shop part-animation previews: spoiler on the trunk, Trunk Audio, Doors, split hoods

Player reports: "when opening the trunk in the shop, only
the trunk opens but the spoiler stays static", "trunk sound system in the
shop still seems not finished", "door preview also not finished".
Native captures: `$OB_PRIVATE/wine/runs/<run>/work` (display :188, the
finished-career save copied in, main-menu Customize on its tuned 350Z, an RX7 career).
Small copies: [evidence/shop-part-animations](evidence/shop-part-animations/).
Clone shots from the same save: `NFSU2_SHOT_SAVE=<the finished-career save> NFSU2_SHOT_KEYS=…
--shot custcars` (the native key sequence; see the README there).

## 1. What follows a posed part (Car_Draw 0x620AF0) — verified

* **Generic draw keeps three posed matrices.** After an animated model's
  matrix (B·P·B⁻¹·M, 0x6235A1…0x6243E5) the draw files it by slot
  (0x6243F4): slot 0xA (TRUNK) → [esp+0xC4], 0x11 (DOOR_LEFT) → [esp+0x60],
  0x12 (DOOR_RIGHT) → [esp+0x68]; all three are zeroed per draw (0x623052).
  Only the animated branch (channel not shut, 0x6234E2) stores, so a shut
  part leaves them 0. The last animated group of the slot wins.
* **Spoiler** (slot 0xC, its own draw 0x625A91): with the SPOILER marker
  (render info +0x2C04), group 0 is drawn at marker × [esp+0xC4] when that is
  set (0x625AE1…0x625D40), else marker × car; group 1 is always on the car
  (0x625AE9: `edi ≠ 0` skips); without the marker both groups are drawn on
  the car (0x626334), never posed. Nothing distinguishes hatch, SUV or
  saloon spoilers (SPOILER, SPOILER_HATCH, SPOILER_SUV are only model
  sources): the trunk channel is the hatch on hatchbacks.
* **Mirrors** (slot 0x20): above level 0 (ride +0x610, 0x62645D) group 0 is
  at the left marker × [esp+0x60] (0x626515) and group 1 at the right
  marker × [esp+0x68] (0x626A86) when set; stock mirrors (0x626F90) take
  [esp+0x60] / [esp+0x68] for groups 0 / 1 only while [esp+0x60] is set,
  else the car (**inferred** equal in the clone, which opens both doors
  together; the case "right posed, left not" is not reproduced).
* **Channel overrides** (0x623473…0x62355F): slot 0x36 (DECAL_REAR_WINDOW)
  takes channel 1 (trunk) when 0x60C900(car type) holds — types 0–3, 6–11,
  13, 15–20, 22, 24, 29 (hatchbacks and the SUVs); slot 0xE (HEADLIGHT)
  takes channel 0 on car type 8 (HUMMER), evaluated as group 1 when render
  info +0x2D48 is set — the hood's VERTSPLIT attribute ≠ 0 (0x6367D1).
* Ported: `car::animated_channel`, the spoiler / mirror `anim` in
  `Car::load_with`; render unchanged (part_world = B·P·B⁻¹·M · marker).
  Under Wine the 350Z's spoiler rises with the hatch on entering Trunk
  Audio; the doors carry the mirrors in all three styles. Tests:
  `spoiler_rides_on_the_open_trunk` (real 350Z assets: the spoiler's matrix
  is the trunk's pose × its marker, >0.2 m up when open, unposed when shut),
  `rear_window_decal_and_hummer_headlights_take_their_channels`.
* Not observed under Wine: a stock 240SX with a spoiler, a HUMMER, a rear
  window decal on a hatch (same code path, traced only).

## 2. Trunk Audio (UI_ICEMAIN modes 2–5, 0x5593F0)

Compared screen by screen (strip, layout list, Back, component strip,
component list, Back, exit). Fixed:

* **Components unlit** — the draw (0x630CEC) passes the trunk audio's group
  0 context ([esp+0xD8], built at 0x6306F6), not the main one: harmonic ×
  trunk value × 0.9 (0x7A65E8). The clone gave them harmonic 0 (black
  boxes); now `harmonic_anim = (1, 0.9)` like the layout's group 0.
* **Level-3 glow** — 0x6307A3…0x63085B (audio, part +5 & 0xE0 = 0x60):
  c0.rgb += (R, G, B)·I·1.5·value (bytes of render info +0x37C ÷ 256; 1.5 =
  0x7A65EC); the engine (0x630581…0x630641, ride +0x5C4 level 3) the same
  with +0x364 and 0.6 (0x7A65FC). +0x364 / +0x37C are the engine / trunk
  neon's pulsed colours (0x622AA8 / 0x622BF8), 0 without that neon part.
  Ported as `CarPart::harmonic_glow` + `CarRenderer::set_part_glow`
  (garage and showcase frames). Not seen under Wine (the finished-career car has no
  level-3 audio with trunk neon): traced only.
* **Layout preview empties the components** — 0x55C180 for slot 0x22 calls
  0x547480: each slot 0x23–0x2E takes 0x53AEB0's part (the slot's first
  part of level 0 that 0x52A020 passes; **inferred** that every level-0
  part passes). Under Wine the layout list shows TUNED LAYOUT without its
  amp and speakers; the clone now does (`Game::preview_ride`).
* **Visual Rating** — up from the opening of every UI_IcePartsOverlay list
  (layout list in modes 2 and 3, a component slot's list, and the
  specialties' lists, e.g. Window Tint on DARK BLACK) and gone after Back
  (Wine: strips without it). The clone showed it only after a step and kept
  it after Back. The neon list keeps its own rule (none until a step).
* **Install Part** — the trunk lists' help bar (the overlay's 0x409), was
  Continue.
* **Back from Trunk Audio** lands on the specialties' first item (Custom
  Gauges) as the hatch shuts (case 0 set up again), was Trunk Audio.
* The help bar's name stays the loaded profile's ("the finished-career save") in main-menu
  Customize (was "Player": the scratch profile's name).
* Matching already: the hatch per mode (cases 2–5 open, case 0 closes),
  camera (the 0x22 view on a list, base view with the car's angle kept on
  Back), layout order and ratings (TUNED 4.22, STANDARD 4.07, STOCK 3.92 on
  the same save), component order and names, sounds 0x53 / 0x52.
* Open: the scene's left lamp glow and overall car brightness differ (not
  part of this flow); the components' own glow with trunk neon, not
  captured.

## 3. Doors and split hoods

0x61F830 (traced): the doors' variant is the door style part's (+0x810)
ANIMSTYLE; ANIMSTYLE 1 (scissor) on a wide body above level 0 with a kind
plays WIDE<kind> (kind + 2); the FE sound is 0x4C / 0x4F for ANIMSTYLE 1,
else 0x50 / 0x4E, channel 2 only (in game 0x481500(1 / 0) both); hood 0x51
/ 0x4D, trunk 0x53 / 0x52 (in game 3 / 2, 5 / 4). The clone played the
regular door sound for scissor doors; fixed (`fe_rules::part_sound_fe` with
the FE car's door style).

Under Wine (Doors list, Open/Close = C): STOCK / SCISSOR / SUICIDE open both
doors with their own animation, the mirrors on them; a step to another
style or Back shuts the doors **at once** (next frame, also on the fitted
STOCK; the FE car is built again) with no closing animation. The clone
closed them with the animation and a close sound on Back; now it drops the
channels (`garage_request`, `garage_toggled`). Visual check of the three
styles: same poses, hinge sides and camera (clone `p1..p3`).

Split hoods (Split Hoods list, COBRA / COPPERHEAD): open, both halves; the
right half matches. **Open**: the left half (group 0, LEFT_HOOD_SPLIT, hinge
y +0.711, ≈103° about −x) has the same silhouette in both but is black in
the original and lit with its paint in the clone — whether the original
shows its underside (a different turn: A·R vs A·R⁻¹, or the rest matrix's
sense) or lights it in other axes is not traced. The STYLE22 speeds were
not compared (no STYLE22 hood on the 350Z).
