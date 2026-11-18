# Career, race types and rewards (3)

Part of [docs/formats.md](../formats.md), the traced behaviour of `SPEED2.EXE`; the tags
(**verified**, **inferred**, **open**) are explained there.

## Rewards gallery, dyno menus, Next Race, package discount (ui2 fork)

* **Next Race** (**verified**): UI_PostRace (0x4BEE10) offers "Next Race"
  in career when 0x513FA0 holds: a following race record exists
  (current 0x85EF48 + 0xD24 inside base 0x85BAB8 + count × 0xD24) and the
  player's results record (0x503320(0), +8 status) is 1. The chain
  0x4F3EA0 puts TournResults, TournStandings, then UI_PostRace after every
  race (the reward only once the event is won), so between a
  tournament's races the career goes to UI_PostRace and "Next Race"
  (0x4BEC90 → 0x5F41F0(10)) starts the next one. Quick Race (game mode
  2) has no rounds (**verified**): its race is the mode record's one track
  (0x83AAB4 + 4 + type × 20, track at +0x18), 0x4C0050 / 0x4C00C0 are
  false outside game modes 1, 4, 8 and 0x200, and UI_PostRace adds Next
  Race only in a career, so the chain is UI_PostRaceResults then
  UI_PostRace; the clone races a multi-race event's first race alone.
* **Jump to Garage?** (explore pause, 0x4C2900) needs the byte 0x8635A8
  (career +0x8868, saved by 0x514530); no instruction stores to it directly
  (it is in .bss; the flag setter 0x496390 writes 0x8635A9..B8 only).
  Yet the original's own saves hold 1 from the first stage-1 save on (0 in
  the car lot's: <work dir> s01 0, s02 / s06 1), and with such a save loaded
  its explore pause lists Return to Explore Mode, **Jump to Garage?**,
  Options, Replay, Quit to Main Menu (seen under Wine, variants round
  2026-10-31). The clone offers it once DDAY_EVENT_B is won, when its save
  writer sets the byte (the setter itself is **open**, the rule
  **inferred** from the saves).
* **Package discount** (UI_ChoosePerformancePackage handler 0x5620E0,
  **verified**): in a career shop (0x850078 = 1 and 0x83898C ≠ 1; the
  crib's Change Parts enters with 0x83898C = 1 via 0x4AF680, shops with 0
  via 0x520430) the package branch shows the discount line: 0x06E0C9A8
  "10%% Discount:" (0x6DA17A25) or "20%% Discount:" (0x66559034) when the
  visited shop (0x86344C, record +0x38) is CC_MEGALOW (0x7FC8D0D0);
  0xBE8693E1 the amount saved and 0x0380B632 the total, `%$d` (thousands
  grouped, no currency sign) or "n/a" (0xA4C02484) when not above 0.
  Otherwise the four (with "Total Cost:" 0x89CA4FDB) hide. The price
  (0x5415F0): the parts neither fitted (upgrade state +0x50+id) nor
  owned (car record +0x590 bit; the clone has no owned-but-unfitted set)
  summed; more than one such part pays 90% (80% at CC_MEGALOW), integer
  /100; the purchase 0x54FE60 charges that.
* **Reward options** (UI_CribRewardOptionsMain, 0x4AFB70): View Magazines
  locks when no magazine reward is reached (0x510D50(1): reward records
  0x34A16 of kind 1/3 marked reached), DVDs likewise (kind 2/4), Signed
  Sponsors when no stage has a sponsor (0x5009E0).
* **UI_MagazineSelect** (0x55B530 → 0x557F30; items 0x523F70; selection
  0x54B160): 0x838578 = 1 magazines / 2 DVDs. One item per reached reward
  of the kind, in record order, icon 0xFF293517 / 0x597DF2A7, label
  `<cover>_TITLE` (a cover is bin_hash "MAGAZINE_SETUP_<MAG>"). Title
  0xF7333312 "Magazines" / 0x653B26C4 "DVD covers"; 0x010CBA23 "View
  Magazine" / "View DVD"; "mag name" 0x9E302291 the title, "mag body"
  0x3554DEB1 `<cover>_BODY`; MagThumb (hidden by the constructor 0x550DE0) is given the selected item's cover (milestone +0x28) as its texture — the FrontB texture `MAGAZINE_SETUP_<MAG>` (128 × 32, the logo) — and shown on every tick 0xC98356BA (0x54B2A2 → 0x495500; **verified**, Wine menus113 v2). Accept (0x53DE70) stores the
  cover at 0x837FD8 and opens UI_MagazineView.
* **Cover pose and view (2026-11-01, <work dir> "mag"; `draw_cover`,
  `Game::open_cover`)**:
  * **UI_MagazineView shows the saved file** (**observed** under Wine,
    the finished-career save copy, Garage → View Rewards → View Magazines): Accept on a
    cover runs the memory-card library's load of `<Name> Magazine <n>` /
    `<Name> DVD <n>` and tells "Load complete." (then the view with that
    file's picture, pixel-identical to the community file `the finished-career save Magazine
    1` decoded) or "Unable to load the finished-career save Magazine 1." (Continue: back on
    UI_MagazineSelect, the cover still selected). No new shot is
    composed there. The clone composed a new shot on view (with its
    misplaced car); now `Game::open_cover` reads the file
    (`profiles::read_cover` → `native_save::read_picture`, alpha forced
    opaque, **inferred**) and binds it under SCREENSHOTHASH (shot
    `magview`; `NFSU2_SHOT_COVER_FILE` takes an original's file).
    The library calls behind it (op, MC_Main mode) are **not traced**.
  * **The FE car's place at the shoot** (**verified**): 0x523D10 poses
    the car of 0x4A7830(0), which is GarageMain.fng's FE car (0 when
    GarageMain is not up; UI_MagazineReward's constructor 0x565760 finds
    GarageMain.fng at 0x565A2C and hands it the career car first, 0x565A1F
    / 0x565A27). 0x4C2070 sets only the car's turntable angle and steer
    and the FE camera; the platform angle 0x8363F0 and the disc are left,
    so the car stands on the platform's front disc, centre (0, 5.4), tyres
    at 0.15 (0x7F45A8), as GarageMain shows it (`Turntable::car_transform`).
    The cameras are in that world: every MAGAZINES_FRONTEND camera's eye
    is at y = −4 and its point at y = 4.48 (dump: `examples/mag_cameras`),
    as GarageMain's own shots. The clone drew the car at the origin with
    the body's origin on the floor: 5.4 m nearer and sunk (the player's
    "weirdly placed"). **Projection**: view 1's 0x5C7ED0 widens the lens
    as for GarageMain's shots (×1.1 across, `Camera::game_fov`); the clone
    took the lens unwidened. **Roll** (**verified**, 0x444DE0): the
    camera's +0x34 (mover +0x138 via 0x4455A0) over 1e-5 builds
    0x5BA8C0(forward, ftol(roll · 65536) / 360), a column-convention
    rotation (m[1] = xy(1−c) − zs; c = Cos16 0x43C370, s = Sin16 0x43C2B0)
    multiplied on the left of the look-at basis (columns right, down,
    forward), so the camera turns by +roll about forward: the clone's
    `PhotoCamera::up` (checked against two original covers of camera
    0x05582DE8, roll 28.2°: Wine menus113 `dvdreward` and `the finished-career save DVD 2`).
    Side by side (research/mag-compare): the car's place, size and tilt
    now approximate the original's Four on the Floor cover. **SHADOWFE
    ported (2026-11-03)**: instrumented native reward entry and an on/off/
    restored capture establish the three order-2/effect-13 strips between
    earlier and later car buckets, with RGB-only alpha blending and depth
    test but no depth writes. The clone uses this queue split only for
    reward covers; authored background/foreground layers stay in place.
    Evidence and remaining composition limits (including a measured car
    position difference): `docs/evidence/cover-shadow/README.md`.
    **Mesh-height followup (2026-11-03)**: the reward mesh draw now applies
    the native copied matrix's +0.01 world-z translation (0x62132D), while
    the shadow and base pose stay unchanged. Paired live-state/draw evidence
    in `docs/evidence/cover-placement` does not resolve the earlier xy difference: the sessions lack comparable
    retained platform angles. No xy compensation is applied.
    Shared-draw followup: all car render entrypoints now apply the same mesh
    bias (traffic independently does so at 0x61684C); see
    `docs/evidence/car-draw-height`.
    The selected environment's light rig is now used; full paint,
    reflection and composition parity remains open. Picks stay the
    clone's draws (**inferred**).
  * The composition is the 640 × 480 screen whatever the window's shape
    (shots at 2560 × 1080 and 3840 × 2160 keep the cover's aspect).
* **UI_MagazineView** (0x554ED0 → 0x553BF0, messages 0x565B60): the
  cover image 0xD1CC9C37 in a black frame. The image is a 320 × 432
  screenshot ("SCREENSHOTHASH", streamed by the 0x832DAC loader) taken at
  the photo shoot; the magazines' art (`<MAG>_BG_nn`, `_MASTHEAD_01/02`,
  model and `_XT_nn` layers) is in FRONTEND/MAGAZINES_STREAMING_*.bin and
  MAGAZINES_FRONTEND.BIN chunk 0x30230 (0x5108A0 → 0x838570;
  `formats::magazines`, **verified** layout): u32 camera count 37, u32 3
  (environment maps), u32 setup count 16; cameras from +0x0C, 0x38 bytes:
  u32 name hash, u8 flags (bits 1 / 2 / 4 / 8 turn the FE car lights 0 /
  2 / 3 / 1 on, 0x4C1860), f32 +0x08 / +0x0C to the FE car object
  (0x4C2070: +0x274 / +0x254, +0x25C and 0x7F45A4), then ten floats to the
  FE camera mover (0x4A5B70 → 0x4533B0 → 0x451200): +0x10 → +0x100, the
  point looked at +0x14..+0x1C → +0x110, the eye +0x20..+0x28 → +0x120,
  +0x2C / +0x30 → +0x130 / +0x134, the roll +0x34 → +0x138 (0x4455A0).
  The mover eases from its current values with a cosine (0x44CE00, state
  0) and its view (0x444DE0) looks from the eye at the point, right =
  (0, 0, −1) × forward, rolled about forward when the roll is over 1e-5.
  Setups from +0x110C, 0x68 bytes: u32 the cover (bin_hash
  `MAGAZINE_SETUP_<MAG>`), six counts at +0x08, choice lists +0x20 ×3
  backgrounds, +0x2C ×2, +0x34 ×5, +0x48 ×3, +0x54 ×3 cameras, +0x60 ×2
  environment maps. UI_MagazineView's constructor (0x565A8E → 0x523D10)
  finds the setup of the cover 0x837FD8 (0x510790, else the first),
  picks one of each list (0x5105C0: the first under a count of 2, else
  random 0x43C1C0), poses the FE car at the picked camera (0x4C2070, the
  lights by the flags), loads FRONTEND\ENVMAPS\%s.BIN of the picked
  environment map and the layers (0x523BD0). The clone renders the
  career car at the picked camera into a 320 × 432 texture shown in
  MagImage (test `magazines_frontend_has_37_cameras_and_16_setups`;
  `game --shot cover`). **Verified**: +0x08 is the FE car's turntable
  angle in degrees, set as both current (+0x254) and target (+0x274), so
  it does not turn (0x4C1D40 wraps the target by 360 and turns the
  shorter way); +0x0C likewise sets +0x25C and 0x7F45A4 together (their
  difference, +0x130, is 0); +0x10 is the mover's angle (+0x100 → +0x80),
  written to the camera's +0xC4 × 65536 / 360 (0x44D116), the field of
  view the projection (0x5C7ED0) applies across, so the clone takes it
  as horizontal; the picks are bRandom (0x43C1C0, `GameRng`: seed % n,
  seed 0x7EA624 from 0x12345678), the four lists before the cameras
  drawing only with a count of 2 or more, the cameras and environment
  maps whenever their count is not 0. **Verified**: the turn about +z.
  The angle is set at 0x4C20B5 (current +0x254 and target +0x274 both get
  the pick) and read at 0x4C1DCE by 0x4C1D40's turn controller. The
  transform that turns the car is FUN_004A6670 (the car's body routine,
  called from 0x4D82D0): it reads the target +0x274 at 0x4A68D2, wraps it to
  [0, 360) (360 at 0x78450C, 0 at 0x7D7D3C), multiplies by 65536 (0x784238),
  converts with FUN_0075BED4 and divides by 360 (the magic multiply at
  0x4A69B0). That units value drives FUN_005BAE70 (cos / sin about +z,
  0x5BACA0), which gives the second rotation (local_390) of the four wheel
  points; the body basis at car +0x920 (FUN_004012B0) is built from those
  points. So the body turns about +z by the target angle (the +z axis is from the
  earlier pass and was not re-traced here). The angle is set by FUN_004C2070
  (0x4C2070). Its A1 is the second stack argument: FUN_00523D10 pushes its
  [esi +0x08] at 0x523DC1, then pushes a 0 at 0x523DC2 (the first argument), and
  calls at 0x523DC4. FUN_004C2070 pushes its own 0 at 0x4C2084 and calls FUN_004C1FA0
  at 0x4C208F; that callee pops the 0 with ret 0x4 at 0x4C2024, so [esp +0xC] at
  0x4C2094 is A1. A1 is stored to the target +0x274 at 0x4C209C and to the current
  +0x254 at 0x4C20B5. REA lists 0x523D10 as the only caller of 0x4C2070.
  **Verified**: FUN_004A6670 reads only the target, at 0x4A68D2 (no +0x254 access
  in its body).
  **Verified** (structure, not the axis): the global 0x8363F0 turns the per-wheel
  table offsets before the body position is built. FUN_004A6670 is called from
  FUN_004D82D0 at 0x4D8BC8 and 0x4D8BDA with P = [car +0x4C] or [car +0x50] and
  table index I = [P +0xB78]. At 0x4A66FD it loads the global, multiplies by 65536.0
  (0x784238), calls FUN_0075BED4 (not traced) and divides by 360 (magic 0xB60B60B7,
  checked on sampled values). The angle goes to FUN_005BAE70 (0x5BAE70), which
  calls FUN_005BACA0 (0x5BACA0) and FUN_0063CEEB (0x63CEEB, not re-traced). The two
  output pointers are both the frame slot [esp +0x100] (lea at 0x4A6816 and
  0x4A681E), so the matrix contents and the +z axis are **Open**. The table x and y
  (0x7F45AC + 12 I, 0x7F45B0 + 12 I) and the base z ([esi +0x20] - [esi +0x18] plus
  0.15 from 0x7F45A8) go through the matrix at 0x4A682E to 0x4A68B6. P +0x910,
  P +0x914 and P +0x918 are stored at 0x4A6E79, 0x4A6E7F and 0x4A6E85 from
  [esp +0x44], [esp +0x48] and [esp +0x4C], after FUN_004012B0 (0x4012B0) at
  0x4A6E68. The link from the rotated x and y at [esp +0x20] and [esp +0x24] to
  those stores was not traced, and [esp +0x4C] (0.25 times [esp +0x3C] plus a stack
  value) is not confirmed as the average wheel height. The exit at P +0x10 == -1
  skips this (0x4A6680 to 0x4A669C).
  **Verified** (writers): the global is written at 0x4D85DC (car +0x150 + 120 *
  input when car +0x14C equals ebx) and at 0x4D85ED (car +0x150 - 120 * input
  otherwise), where input is the float at [esp +0x14] and 120.0 is at 0x78AF80.
  Both writes are in FUN_004D82D0 (no ret between 0x4D82D0 and 0x4D8BDA). Each
  path has one conditional step: minus 360 when at least 360 (store at 0x4D870F),
  and plus 360 when below 0 (store at 0x4D8734). It is stored 0 at 0x4EACD5, with
  car +0x150 set from ebx and car +0x13C set to 1. **Open**: input is 1.0 at
  0x4D8443 or a value stored at 0x4D848D, and which branch runs in the front end
  after the pick is not traced, so the front-end value of the global is not
  established. That earlier pass changed no code. The recovered cover renderer now
  uses the existing GarageMain turntable transform (see Cover pose and
  view above); the dynamic platform state during every shoot is still open.
  **Inferred** (h / w): the
  shot's 320 × 432 (0x4889F0 → FUN_005CE520) is stored as the capture
  object's own size, and its texture is the 512 × 512 "ScreenCap" (0x48B220).
  0x5C7ED0's body reads the view's near / far (+0xBC, +0xC0, or 15000 for
  type 1), +0xC4 and two x87 angles (cot θa for x, −cot θb for y); as the
  body reads them, none is the shot's size, and the source of the two x87
  angles is not traced. **Verified** (black): the back buffer is cleared to
  colour 0 with flags 7 after each Present (0x5D2860's Clear, vtable +0xAC).
  The front-end light rig is the light setup item below. **Open** (the
  clone's seed draws): the
  game calls 0x43C1C0 from 102 direct call sites (counted in speed2.dis);
  which of them this view reaches, and in what order, is not traced, so the
  clone's picks are not the game's.
  **Layers** (**verified**): 0x5105C0's first four picks are texture
  hashes (`<MAG>_BG_nn`, `<MAG>_MASTHEAD_nn`, the model
  `<MAG>_<NAME>_nn`, `<MAG>_XT_nn`; all in MAGAZINES_STREAMING_HIGH.bin)
  that 0x523BD0 streams into 0x863780 (+0x10..+0x1C; +0x0C once all are
  in). UI_MagazineBack's tick (0x54B0F0) puts the background on its image
  "MagazineBack" (512² at (84, 35)); UI_MagazineReward's (0x55B270 is
  the handler of its vtable 0x79D838; corrected 2026-10-28 — the EXE
  never names UI_ShowCase_Magazine) the masthead on "MagazineMast"
  (0xB6638240), the model on "MagazineChick" (0x82227F6D) and the extra on
  "MagazineExtra" (0x824FA72F), all 512² at (84, 35) — 0x495400 + show
  0x495500, else hide 0x4954C0. The clone took them from
  UI_ShowCase_Magazine (no chick, the extra 512 × 256 at (86, 166)) until
  then; now the model layer draws too. The shot: 0x4889F0(320, 432) and 0x488B50 arm the capture, which
  the frame's end (0x5D2BA8 → 0x5C3130) takes from the back buffer, x
  W/2 − w/2 − 10 .. W/2 + w/2 + 3 and y H/2 − h/2 − 4 .. H/2 + h/2 − 6
  with w = 320 · W × 0.0015625, h = 432 · H × 0.0020833 — at 640 × 480
  the 333 × 430 pixels from (150, 20), exactly the layers' top-left
  corner — into the 512² "ScreenCap" texture. The clone composes that
  screen at 640 × 480 (background, car, masthead, extra) and binds the
  rectangle (`CoverView::layer_quads` / `CROP`, `game --shot cover`).
  The copy (**verified** in the code, the filter's result **likely**):
  CreateRenderTarget(512, 512, A8R8G8B8, lockable), StretchRect(back
  buffer, rect, target, NULL, D3DTEXF_NONE), LockRect(READONLY): point
  sampled, the back buffer's alpha with it (`native_save::picture`; the
  clone stretched bilinearly and wrote alpha 255). The reward capture
  keeps the shot's alpha (`cover_texel`, the shop fixes).
  The layers come from MAGAZINES_STREAMING_HIGH only (**verified**): the
  stream object 0x863780 picks its pack from table 0x7FA294 (HIGH, LOW)
  by its +0, and its one constructor call (0x565888) passes 0; the LOW
  pack is the other stream object's (0x8364F4, index 1, 0x499E6C).
  Earlier static analysis incorrectly concluded that the shot was taken
  on view and never stored. The finished-career save Wine observations above contradict
  that conclusion: Reward Options loads the saved picture. The following
  capture path exists, but its reachability during saved-cover viewing
  remains **open**. UI_MagazineView
  arms the capture on 0x35F8620B (0x565BDA: 0x4889F0(320, 432, +0x4C,
  "SCREENSHOTHASH"), once, +0x50) and binds it to MagImage (0x5BA040 →
  0x523C80); UI_MagazineReward's tick likewise ("ScreenShot",
  0x55B4E8 / 0x488B50) after its third frame (+0x74 > 2) unless
  GenericDialog_SMALL is up; 0x832DAC is the capture. This trace alone
  does not establish the saved-picture load path. **Inferred**: UI_MagazineBack draws behind the car. The frame's order
  (**verified**, 0x5D2860): the views' projections (0x5C7ED0) and the
  render-state reset (0x5C2440) first; BeginScene (vtable +0xA4), views 1–3
  through 0x48F7A0 when active, 0x5CAAE0 unless DAT_00864F48, 0x5BC190,
  EndScene (+0xA8); then 0x5BCFE0, 0x5C32D0 (the stream / screenshot TGA
  writer, not a draw), the cover capture (0x5C3130) when armed, Present
  (+0x44) and the black Clear. Not closed: which pass draws UI_MagazineBack's
  quad and the FE car's 3D draw inside that block (0x5CAAE0's per-view
  passes and 0x5BCFE0 are not traced), so the FNG flush's order against the
  car is not established. The clone's draw_cover draws the background first,
  then the car, then the masthead and extra.
  **Environment map** (**verified**): the chunk's u32 at +0x04 (3) counts
  entries at +0xE0C (after 64 camera slots), 0x18 bytes: u32 hash, u32 a
  light material hash, the file name (16 bytes) — Showroom, ElaborareGT,
  Showroom (the two Showrooms differ in the light material). 0x523D10
  finds the setup's pick (+0x68, 0x510750 by hash; none → nothing
  loaded), sets FE car 0's light material from +0x04 (0x4A7990 →
  0x48A2D0: render info +0x904, **inferred**: the objects at +0x4C/+0x50
  of the 0x4A7830 result are not yet traced to the 0x63B860 nodes),
  loads FRONTEND\ENVMAPS\<name>.BIN (a
  texture pack `...\EnvironmentMaps\<name>.tpk`), waits for it and runs
  0x5B9920, which (with 0x870CB8 on) copies the textures XPOS, XNEG,
  YPOS, YNEG, ZPOS, ZNEG into faces 0–5 (D3D +X, −X, +Y, −Y, +Z, −Z) of
  the static cube 0x87088C, the front end's EnvMapTexture. The clone
  loads the picked file's six faces (`magazines::load_envmap`) into a
  cube (`render::car::static_envmap`) the cover car reflects. Camera
  +0x0C (**verified**) is the front wheels' steer in degrees: 0x4C2070
  sets the FE car's target +0x25C and its current 0x7F45A4 (16 by
  default; 0x4D8B09 eases it), and the FE car's wheel matrices (0x4A6E8B,
  the loop over four wheels at +0x910, +0x30 apart) turn wheels 0 and 1
  by 0x7F45A4 × 65536 / 360 about +z (0x5BAE70 → 0x5BACA0: cos, sin /
  −sin, cos); the clone steers the cover car's front wheels by it.
  **Light setup** (lookup and resident table **verified**, cover-time snapshot **open**): the +0x04
  hash names a light setup. 0x4A7990 takes it through 0x48A2D0, which
  searches the 0x74-byte records of the loaded platform setup table
  (0x82DADC entry, 0x82DAE0 count) by their u32 hash (compare at 0x48A2F2,
  stride at 0x48A2F7), returning the record or 0x7F3E10 (the default rig)
  when none matches (0x48A300). 0x4A7990 stores the pointer in render info
  +0x904 of the objects at +0x4C and +0x50 of its 0x4A7830 result and in
  car +0x6C (0x4A79B8, 0x4A79C5, 0x4A79CB). The table is set by the chunk
  check at 0x48A280 (chunk id 0x3B700: 0x82DADC = chunk + 8, 0x82DAE0 = the
  chunk's u32 at +0x04 divided by 0x74, by the multiply at 0x48A294); the
  check at 0x48A2B0 (chunk id 0x3B700) clears both (0x48A2C0, 0x48A2C5).
  The loader is indirectly registered at 0x7FA8DC in the handler table
  0x7FA858..0x7FA8E8 dispatched by 0x570D80. A private native process read
  on 2026-11-03 found all ten FrontB setups resident, including exact matches
  for every magazine environment's light hash. The precise screen at that
  read was not recorded; it is not a cover-time draw capture. See
  `evidence/cover-lighting/README.md` for the snapshot and instruction chain.
  The clone now selects `EnvMap::light` from that table, independently of
  cube loading, instead of substituting `IN_GAME_RIG`. Both Showroom cube
  records have distinct light hashes and are kept distinct. Unknown hashes
  return `DEFAULT_RIG`, matching 0x48A300. **Verified**: with no environment
  map (0x510750 returns 0 for the picked hash, so the je at 0x523E4F skips the
  only call of 0x4A7990, at 0x523E5B; 0x523D10 is its only caller), 0x523D10
  itself stores nothing into +0x904. The stores 0x4A7990 makes (0x4A79B8 and
  0x4A79C5, into +0x904 of the objects at +0x4C and +0x50 of the record 0x4A7830
  returns) take 0x48A2D0's value, which is 0x7F3E10 (the default rig) when no
  setup in its table has the key. The constructor 0x63B860 sets +0x904 to
  0x7F3E10 at 0x63B89B, and 0x63121C reads +0x904 and passes it to 0x620AF0.
  **Not traced**: that the cover's car is an object 0x63B860 builds.
  The clone now distinguishes a missing environment record from a failed
  cube load; a failed cube does not discard the selected light rig.
  **Undefined**: the value +0x904
  holds at draw time when 0x523D10 stores nothing. 0x7F3E10 holds only if no
  later writer ran: 0x4A7180 (the platform setup, stores at 0x4A727E and
  0x4A728B), an earlier cover's envmap store (0x4A79B8, 0x4A79C5) and others
  (0x412713, 0x42010A, 0x4EAE91, and more) can change it. The clone draws a
  cover with no matching environment record using `DEFAULT_RIG` (the
  constructor's value), retaining this existing approximation.
* **UI_RewardsSponsor** (0x4ED840 → 0x4DF6F0): the signed sponsor of each
  stage 0..=current (0x4AFD00, record 0x862018 + n × 0x104 + 0xE8), icon
  DECAL_ICON_<name>, shown by the sponsorship screen's 0x495D30 (URL
  bonus of the current stage); options do nothing, Back to the reward
  options (0x495C40).
* **Dyno** (**verified**; the Save row's state labels **inferred**, see Left out): the crib's tuning
  option (0x4AF780) asks 0xB10AE56F; yes → 0x495AF0 → 0x5309F0 (career
  car 0x863480) → UI_PerformanceDyno_MAIN (0x55D800 → 0x55A470, options
  0x5227F0): Tune Settings (0x53C8A0 → UI_PerformanceTuning_Master), Test
  Run (0x53C8D0, copies the tuning record 0x83A540 +0x14 to 0x83A611 and
  drives), Dyno Run (0x53C840 → UI_PerformanceDyno_Chart), Mode Select,
  Save Profile (only when 0x50FD10; left out), Quit Tuning (0x564DE0:
  "Quit to Garage?" 0xE5A0D45E when entered from the garage). The tuning
  packages are in GLOBAL/InGamePerformanceTuning.bun (now loaded).
  UI_PerformanceTuning_Master (0x55AA50, options 0x557690): Suspension
  (params 0–8), Drivetrain (9–15), Aerodynamics (21–22), Tires & Brakes
  (18–20), ECU (23), Turbo (24), Nitrous (16–17), locked unless a
  parameter is tunable (0x5225D0). Tunable (0x59B090 / 0x59AE50): career
  chunk 0x34A1A (24-byte records: part id, level, param, kind, range
  min/max, display min/max) has a record for a fitted part; the highest
  level wins; gear params 10–15 need param − 8 < forward gears + 2
  (tuning +0x1C8). ECU, Turbo and Nitrous open the graph / NOS screens
  (below).
* **Mode Select** (**verified** unless marked): UI_PerformanceTuning_Setting
  (factory 0x55D860 → 0x55A630, options 0x557420, messages 0x55A750):
  Circuit (0xE9638D3E / 0x93FDEE74), Drag (0xAAAB31E9 / 0x35102EDF),
  Drift (0x00119144 / 0xD7162CFA), Street X (0x13A4CEEB / 0x30DA61E1),
  URL Race (0x3F6495C5 / 0x75F903F4), option +0x48 its index. The dyno
  state 0x839C40: +0x900 the tuning object (0x83A540, 0x50FE20: car
  object +0x384), +0x904 five working settings, +0x9FC the mode
  (0x83A63C; 0xFF none, set again by 0x53CD08). The dyno menu's Mode
  Select (0x5648FA) stores the working settings back to their mode
  (0x50FC20); a mode's accept (0xD05FC3A3) makes it the mode and loads
  its settings into the tuning object (0x50FC60), then opens
  UI_PerformanceDyno_Chart the first time (mode was 0xFF) or the dyno
  menu; Back asks "Quit to Garage?" with no mode, else returns. Storage:
  the car object (vtable 0x79B134, 0x516B30) keeps five 0x29-byte
  settings at +0x360 (0x516A60 makes them through 0x5A7940, all 100);
  0x516AC0 builds a physics car with settings n copied to car +0x398.
  The car object is the player garage's for the career car (0x5309F0 /
  0x528610: 0x503510 finds the car id among the garage 0x83AD90's 91
  car objects at +0x9BD8, matching +0x08), so the clone keeps five
  settings per career slot (`CarSlot::tuning`, saved at the slot's car
  record +0x348; see "Native profile saves"). Which settings a race
  loads (0x528610's mode, through the car object's 0x516AC0): a career
  race by the event's type (0x532401, table 0x5325B4: drag 1, drift 2,
  street X 3, circuit / sprint / the rest 0 — a URL race too, the
  category is not read, so no race loads URL Race's), free roam 0
  (0x540691), the test run the dyno's mode (0x526980).
  The tuning object (**verified**): the dyno menu (0x55A470 → 0x50FE20,
  when 0x83A540 is 0) builds the player's car through the car object's
  vtable +0xC (0x516CA0 → 0x516AC0) with settings 0, so its sliders
  (+0x14 = car +0x398) start as Circuit's before a mode is chosen; the
  sliders edit them only. Entering (0x50F800) copies the car object's
  five settings (+0x360) to the working ones (+0x904). 0x50FC20 /
  0x50FC60 (tuning object → working / working → tuning object) and
  0x50FAE0 (working → the car object, the mode's from the tuning object)
  do nothing with mode 0xFF, so edits made before the first choice are
  dropped when a mode's accept loads its settings. Mode Select
  (0x55A590): with a mode chosen and its working settings not the
  tuning object's (0x59AEA0), "Your current custom-tuned setup has
  changed. Would you like to store it?" (0xA2323032; Yes 0x2B5DFEF6 →
  0x50FC20, No 0xB4EDEB6D), then UI_PerformanceTuning_Setting. Save
  Profile (0x5226D0 option, react 0x564D70; shown when 0x50FD10: the
  tuning object ≠ the saved settings of the mode, or any working ≠
  saved) asks "Are you sure you want to save your profile?" (0x0D7DE8AB;
  Yes 0x51123592 → the memory card flow 0x4A3100(0x51)). Quit Tuning's
  yes (0x30ED2368): with 0x50FD10 and 0x844AD8, "One of your
  custom-tuned setups has changed. Would you like to save your profile
  now?" (0x71851654): Yes 0x24DD40CF commits (0x50FAE0) and saves
  (0x4A3100(0x151)), No 0x1FAB5998 commits and quits, else it quits
  (0x53CC80: tuning object 0, mode 0xFF). The clone keeps the tuning
  object apart (`Game::tuning_object`), asks Mode Select's question, and
  commits on quit, saving the profile then. **Left out**: Save Profile
  and the quit's save question (the clone has no memory card flow; it
  commits as both answers do). 0x844AD8 is the Options' Save-row flag
  (0x49B170, **verified**): the row steps through three states, (0, 0),
  (1, 0), (1, 1), with 0x844AD8 set in the last two and 0x844AD9 only in
  the last (0x4A7FC2 reads it). The quit's question is gated by 0x50FD10
  and 0x844AD8 alone (0x56496C), so it is asked in both. **Inferred**: the
  states are Off, Prompt, Autosave in that order (their labels are not
  traced).
* **Test Run** (0x53C8D0, **verified** as far as traced): on 0x0C407210
  it resets a tuning set (0x59AFF0(−1)), copies the tuning object's
  sliders (0x83A540 +0x14) to 0x83A611, clears 0x83A7A4 / 0x83A644 and
  starts the drive (0x5B7EE0(0), 0x608CB0), the player's race type
  (0x83A9D8[0x83A64C]) from the option's flags: 1 → 0, 2 → 1, 4 → 2,
  8 → 3, none → −1; the slider screen's options carry those flags from
  0x83A7A0 (0 → 1, 1 → 2, 2 → 4, 3 → 8, else 0). The tuning session
  is a race of its own: entering the dyno (0x5309F0) runs 0x526980(1),
  and the race reload (0x581830) runs 0x526980(0x83A7A4) while the
  session flag 0x89E7E6 (set by 0x526980) is on — 0x83A7A4 is 1 for the
  dyno chart (0x557B0A) and 0 after Test Run. 0x526980(0) sets the race
  parameters 0x89E7A0 by the mode 0x83A63C (table 0x526C0C), one racer
  (+0x20 = 1), race type 1, not reversed: Circuit and URL Race
  0x4FEF80 (track 4705 "Speedway Industrial #5", 10 laps), Drag
  0x4FEFF0 (4709 "SW Drag Tune", the drag flag, 1 lap), Drift 0x4FF070
  (4708 "SW Drift/StreetX Tune", the drift flag, 10), Street X 0x4FF0E0
  (4708, the street X flag, 10); no mode (or 0x526980(1)) 0x4FF160, the
  dyno (4706 "Speedway Dyno Testing", race type 8, 0 laps) — all L4RG.
  The car is built with the mode's settings and the dyno's sliders
  (0x83A611) copied over. Pausing in the session (0x6067A0) opens the
  last tuning screen made (0x83A650, stored by 0x55A470 / 0x55A630 /
  0x55AA50: the dyno menu after Test Run), and the pause options' Back
  (0x4A7F24) opens UI_PerformanceDyno_MAIN instead of UI_Pause; in a drag
  test run the HUD's Accept (0x4E9A51) re-copies the sliders and restarts
  (0x5F41F0(9)). The clone drives the run as a race with no opponents
  and no event. Esc pauses it under the test run's menu (**verified**
  2026-11-01, 0x557360: with 0x83A644 = 0 the dyno menu's options are
  0x53C9F0's — Resume Test Run 0x12BB5EA2 / 0x105149CB (0x53C7E0:
  0x5F4250(1)), Restart 0xB295A6B6 / 0x80833FC2 (0x564C90: UI_InGameDialog
  0x0972A44B, OK 0x0639 → 0xE1A57D51, Cancel 0x541389E5 focused), Tune
  Settings, Return to Dyno 0xFE1E723E / 0x18D5A136 (0x564D00: 0x455C3F80,
  Yes 0xF770 → 0x526ADEFC, No 0x061C focused; 0x526ADEFC at 0x564A36 sets
  0x83A644 = 1 and reloads in dyno mode, 0x5F4250(10)), Options
  0x520DE4E3 / 0xB34993C9 (0x53C870: UI_PauseOptionsMain), Quit Tuning;
  the help bar 0x404; under Wine the "Dyno Shop" menu over the paused
  run). Clone: `MenuKind::Dyno` with a paused test run. Resume with slider
  edits made meanwhile (**verified**): Resume Test Run (0x53C7E0)
  only takes the help bar down and resumes (0x5F4250(1)); no refit, so the
  run goes on with the car as it was. The edits reach the car at its next
  refit (0x5F6C30(1): Start on a tuning screen, Default on rows 9–15, a
  Restart's re-copy). The clone resumes the run as it was.
  **Inferred**: a finished test run goes back to the dyno menu. Traced:
  with the session flag the finished racer (0x600406, +0x28 = 0) takes
  0x5F34D0's mask — the one a car racing in free roam takes (0x6001EC),
  without the 0x80 / 0x140000 of a finished race (0x60041C) — so the
  game shows no finish. Traced too: the post-race chain 0x4F3EA0 queues
  UI_PostRaceReward and UI_PostRace only while 0x89E7E6 is clear (reads at
  0x4F3FC6 and 0x4F4083), so a finished test run gets no reward and no
  Next Race. It queues UI_PostRaceResults unless the game mode is 1 in a
  tournament (0x863470 set) with more than one race (0x85EF4C), 8 with
  0x850098 = 1, or 4 with 0x83AAB4 = 8. When it queues no screen it runs
  0x502FF0(0), 0x50C8D0 and 0x503040(1), then posts event 0xB to the
  object whose pointer is the global at 0x8900D8 (0x4F40FA loads it into ECX;
  0x4F4100 to 0x4F4104: push 0, push 0xB, call 0x5F41F0). **Verified**:
  0x5F41F0 takes a node from 0x440E50 (given the global at 0x8900DC) and
  appends it (id at +8, arg at +0xC) to the list at ECX+0xC / +0x10; it does
  not run the processor. The processor 0x600FB0 unlinks the nodes one by one,
  sets +0x14 to 1 while a node runs, and calls PTR_FUN_0080214C[mode](id, arg)
  with ECX = the object (mode at +0). Id 1 sets the mode to the arg first; id 2
  clears it after. It loops until the list is empty, so a node posted during a
  handler runs in the same drain. Table entry 7 (u32 at 0x802168) is 0x603EC0,
  so in mode 7 event 0xB runs its case 0xB (Ghidra decompile): 0x43B640(),
  0x603640(0), then 0x601030(0x16, arg), where arg is the event's arg (0 for
  this post). The switch has no case 0x16 (default: return 0), so in mode 7
  that state does nothing there. (Inferred: the object at 0x8900D8 is the race
  manager, since the store at 0x60A89D follows +0x44 = 1 at 0x60A878, and
  0x603EC0 reads that field.) **Not verified**: which object the processor runs
  on for 0x8900D8; its callers are 0x5811D0, 0x601030, 0x603670, 0x60A690 and
  0x60AAB0, and only 0x601030 was traced (it runs the processor at once when
  +0x14 is 0). **Not traced**: the mode a test run is in (no mov to 0x850078
  lies between 0x4FCB19 and 0x54045C), what 0x43B640 and 0x603640 do, and which
  code handles state 0x16 in other modes, so whether the results screen comes
  first and where the finished run goes stay open; the dyno menu return stays
  inferred.
  Also traced: the race manager's state entry (0x603EC0, by state − 1
  through 0x604250 / 0x604224): entering state 1 in the tuning session
  outside the dyno with +0x44 set resets every racer (0x5F1FE0(0, 0)
  over 0x8900B4), runs 0x603640(0), queues state 10 (0x601030(10, 0)
  on the manager's event list) and re-copies the sliders (0x5100B0:
  0x59AFF0(−1), 0x83A540 +0x14 → 0x83A611, as Test Run does); in the
  dyno or outside the session it goes to state 5 instead; state 3 in the
  dyno (race type 8) queues state 10 again when 0x435950(0x827E88)
  answers. The +0x44 flag of the object stored to 0x8900D8 at 0x60A89D is set
  to 1 by the routine at 0x60A878 (verified). Case 1 of 0x603EC0 reads it and
  clears it on some paths (decompile; byte stores at 0x603F1D, 0x603F51,
  0x603FA6). A second init sets +0x44 to 1 at 0x6010F4 (in the routine at
  0x601097, which stores the result of 0x440DF0 in 0x8900DC); whether that is the
  same object is not traced. **Not traced**: what raises +0x44 at a run's end.
  Other +0x44 stores in 0x5F0000 to 0x60FFFF (for example 0x5F16A9 and a WORD
  store at 0x60F66E) and the BYTE store at 0x5030FF were not traced either, so
  the return stays inferred.
* **Slider screens** (**verified** rows and labels): UI_PerformanceTuning_
  Sliders / _Drivetrain (factory 0x55DB20 → 0x55AC20, rows 0x557790 by
  0x83A648): suspension 0x549B50 (3, 7, 0, 4, 1, 5, 2, 6, 8), drivetrain
  0x549C30 (gears 10–15, then 9), aerodynamics 0x549DD0 (21, 22), tyres
  & brakes 0x549E10 (19, 20, 18); a row only when tunable (0x549860 →
  0x59B090); end labels "Low"/"High" 0x26033125 / 0xE66708F3,
  "Soft"/"Hard" 0xE66D2A8F / 0xE666E852, "Less"/"More" 0xE6692B0A /
  0xE669E1C6, "Top Speed"/"Acceleration" 0xCEBFAA36 / 0x95A7073D,
  "Weak"/"Strong" 0xE66F30DB / 0x36C30E70, "Rear"/"Front" 0x001C0CC9 /
  0x02CBDC28; names by 0x50FE90 (e.g. 0x2430E6BE "Steering Ratio",
  0x6A71E798 "Final Drive Ratio", 0x2308F06C "Brake Bias"); 6 rows on
  screen, 7 on the drivetrain's. A row (0x522B00, vtable 0x79B7C0) holds
  a slider (+0x6C, vtable 0x79AE5C) set up by 0x530DA0: min / max
  0x5B0BE0 / 0x5B0C10 (both gated by 0x59B090; max 1 when 0), step 0x522570 = (hi − lo)/30 of the
  0x5B2990 range (0x786460), value the current one (0x5B3E50). Left /
  Right (0x9120409E / 0xB5971BF1 → 0x564F20 → 0x4FF590) move the value
  one step, held to min / max, then 0x5B4680(value, parameter, 0, 1) on
  the dyno car stores it and the row reads the stored value back
  (`tuning::nudge`). The gear chart (0x549C30): labels 0xAEA3C019 + k (k
  0..7) read `%d` of k × 35, or k × 60 when 0x83AA1A is 1. Rows go down
  the list's running y (+0x100) by +0x128 + +0x108 per row, and its
  +0x120 by the same step (0x549860, **verified**; +0x12C is also read and stored back, its role not traced).
  The list's constructor 0x552790 (**Inferred**: the worker's read, not re-checked) zeroes +0xFC, +0x108 and +0x128, and the
  row setups 0x549B50 / 0x549C30 store to none of them, so where the step
  comes from is **open**. Checked: 0x551080 (a base-class update; its only
  direct caller is 0x5513D7) copies the global 0x865188 into +0x108 (and
  0x8651A8 into +0x118) on every call as integer bits; a seed copy comes
  first when the slot is 0 or 0x7FFFFFFF. Those globals are runtime tick
  counts (written at 0x576D11 as 4000, at 0x57A180, 0x601BC2 and 0x601C26;
  the file holds 981219947 at 0x865188, which reads as about 0.001), and
  0x549860 reads +0x108 as a float (fadd at 0x549A8D and 0x549AA7), so this
  path gives a step near zero for any realistic count, not 40. Every store to +0x128 in 0x54xxxx to 0x57xxxx is the constructor's store at 0x5528D2 (`mov [esi+0x128],ebx`, the only store in 0x54xxxx to 0x57xxxx, **verified** by a grep; ebx's zero is not traced), so the traced step is 0 if that zero holds and the origin of the 40 px is **open**; the clone's 40 px stays **inferred**.
  TIP_TEXT_ only outside the gears. The row's highlight
  (0x549860, **verified**; corrected): the +0x4C object's +0x24 is set to
  0x9943D584 for the selected row (flag [ebp+0x14] nonzero; **Inferred** name) and to 0x374F6465 for the others, and the
  +0x50 object's +0x24 is set to 0x9943D584 for the selected row (0x549A14
  to 0x549A56) and to 0x760BDC9D for the others; each write ORs
  0x400000 into the object's flags at +0x1C
  (what the hashes name is not traced). The handle's travel (0x530DA0,
  **verified** for the code): the travel +0xA8 is 0x51D740's first output
  for the base object (+0x78, the object 0x530D30 stores there; 0x5445B0
  formats its integer argument with "%d" (0x7845D4), combines it with [this+300]
  and looks the result up in [this+4]; the key-to-name step is not traced)
  less 2 × (+0xAC). 0x51D740 (**verified** pseudocode) reads the base's type
  at +0x18: for 1, 7 or 9 it returns the floats [base+0x2c]+0x38 (width, the
  first output) and [base+0x2c]+0x3C (height); for 2 it returns both scaled by
  0x50BEC0 / 0x50BF40 (on [base+0x64]; 0x50BF40 also takes [base+0x70]) when 0x51BD70 of [base+0x24] is nonzero,
  and otherwise leaves the outputs unwritten (the caller's local is then
  undefined); other types return 0.0, and a null base writes nothing. The
  Tuner base's type is not traced, so its width is **open**; the clone reads
  the FNG object's size as the 1/7/9 width at unit scale. Its unit is the
  record's float (**inferred** UI pixels, as the clone places objects; the
  writer of +0x38 is not traced). 0x530D30 (0x54998D) and 0x530DA0 (0x5499A5)
  run inside 0x549860, before its store of +0xAC = 4.0 (0x549AB5, 0x40800000 = 4.0, **verified**);
  the constructor FUN_00522B00 (**verified**: it also writes the vtable 0x79B7C0 at +0x00) stores 46.0 at 0x522BA5 (0x42380000, **verified**), so that
  first call gives width − 92. Then 0x549860 recomputes the travel inline at
  0x549AC2–0x549AE3 (the same 0x51D740 call on [edi+0x78], then `fsubr`)
  as width − 8 (**verified**). No branch between 0x5499A5 and 0x549AB5 skips the store (**verified**: the
  conditional jumps in that range land between 0x5499D2 and 0x549A61, the one
  unconditional jump lands at 0x549A56, and there is no ret in the range), so
  the width − 92 value is overwritten at 0x549AE3 (**verified**). Between the
  store and the recompute at 0x549AC2 only the virtual call at 0x549ABF runs
  (**verified**: it calls [eax+0x10] with eax = [edi]); its target is not
  traced, and whether it reads +0xA8 is not traced. The later 0x530DA0 calls at
  0x565286, 0x5652C5 and 0x565427 exist (**verified** call sites); whether they
  run on this slider after 0x549860 and see +0xAC = 4.0 is **inferred** (not
  traced, and other writes to +0xAC are not checked), so they give width − 8
  only then. How the value maps onto the travel is not traced. The travel is
  **verified** (0x530DA0 and 0x530D30 pseudocode: +0xA8 = w − 2 × (+0xAC), with
  w = 0x51D740's first output on +0x78). 0x530DA0 also sets, from its pseudocode
  (write targets and defaults **verified**; the callees' meaning is not traced):
  min +0x94 (0x5B0BE0, or 0.0 when FUN_0059B090 is 0), max +0x90 (0x5B0C10, or
  1.0 when FUN_0059B090 is 0 or the value is 0.0), step +0xA4 (0x522570), value
  +0x9C (0x5B3E50, or 0.0 when FUN_0059B090 is 0) clamped to [min, max], and
  +0xB4 / +0xB8 from 0x5B2BE0 (1.0 when FUN_0059B090 is 0 or the value is 0.0) /
  0x5B2BB0 (0.0 when FUN_0059B090 is 0). The clone's tuning rows put the handle
  at 60 + t × half with half = (width − 8) / 2 (`screens::TuneSliders`, now the
  steady travel; the half-travel reach is the clone's **inferred** choice), and
  the value-to-x map is **open** (the table at 0x79AE5C is written at +0x6C by
  FUN_00522B00, not at the +0x00 vtable slot, which holds 0x79B7C0; the slots
  listed below are not checked against the vtable used at 0x549ABF; its slots
  are 0x512220, 0x4FF590 (the step, from 0x564F20), 0x540C20, 0x5122B0, 0x526DA0, 0x526F90, 0x5273B0, 0x540EA0, 0x540D30; none read yet). The options' place_slider uses the 46 margin. **Inferred**: the clone's 40 px spacing
  (the step values), the highlight colour (on the name; no code constant found: the only 0xFFFFD200 dword, at 0x41E61C, is the rel32 of the call at 0x41E61B to 0x41B820 (0x41B820 − 0x41E620 = −0x2E00; **verified**), and the three 3-byte FF D2 00 hits (0x3CFDEB, 0x3D062F, 0x3D09E5) sit in 16-bit tables; the six 00 D2 FF hits were not examined), and that
  the screens' unit conversion cancels out. 0x5B2810 with flag 1 maps the
  mean limits onto the display range (0x5B09B0) through 0x401790 (not
  traced) and widens a zero-width mean by 0.01 when an opener exists
  (**verified**); the clone works unconverted.
* **Graph / NOS screens** (**verified** unless marked;
  UI_PerformanceTuning_Graph / _NOS, both factory 0x55DB80 → 0x55AD80 →
  0x5578D0, messages 0x5654E0) make one widget by the category 0x83A648:
  4 a curve widget (0x53CFD0, vtable 0x79BEC0, 0x30 bytes, 455 × 241)
  for parameter 23 (ECU), 5 the same for 24 (turbo), both at (65, −49);
  6 the nitrous widget (0x523060, vtable 0x79B804, 0xA8 bytes). On a
  test run (0x83A644 = 0) the Graph package's options take the
  race-type flags as the slider screens do. The curve widget's set-up
  (0x550590): an axis from the 0x5B09B0 limits rounded out to 50s, its
  box at (x − 292, y + 213) (0x549FE0, labels TORQUE_DATA_n), then a
  point per curve point (0x549E60: PULLPOINT_n, GRAPHBAR_n / _B_n /
  _C_n; a slider of the 0x5B2990 range with step (hi − lo) × 0.1,
  0x784358). Keys: Left / Right pick the point (0x522EC0, wrapping;
  "UnHighlight" / "Highlight" scripts), Up / Down (0x72619778 /
  0x911C0A4B) move it a step (0x522F20 → 0x5101E0 / 0x510240, held to
  the range) and store it (0x522510 → 0x5B4680(v, p, point, 1)). The
  nitrous widget (0x550850): one slider for parameter 16, step (hi −
  lo) × 0.1 (0x54A1A0), on Left / Right (0x523220) stored by
  0x5B4680(v, 16, 0, 1), then 16 and 17 read back (the writer moves
  17). Back (0x911AB364) plays 0xDE6EFF34 and returns to
  UI_PerformanceTuning_Master (0x55DDB0 0x731BF236). The clone
  (`screens::TuneGraph`) does the keys and steps. The drawing (**verified** unless marked; ported as `screens::curve_bars`). The point's sub-object (node +0x24, which the walker 0x531280 passes as `this`) holds the value bar (+0x4, from the point's value at +0x0C), the lo bar (+0x8, from +0x10), the hi bar (+0xC, from +0x14) and the handle (+0x10). Its column x (+0x14) is copied from the widget's +0x10 and its baseline (+0x18) from the widget's +0x14 (**verified**, 0x549F51 to 0x549F62). Its width (+0x1C) and height scale (+0x20) are copied from stack slots read at 0x549F5A and 0x549F5E; they are the pitch (455 × 0.11111111 at 0x8637C4, computed at 0x77D5D0 with 0x799398 = 0.0, **verified**; loaded at 0x549EBC) and the widget's +0x1C (241, loaded at 0x549EB9, **verified**), carried to the stores at 0x549F5A / 0x549F5E through stack slots (a tracer read of 0x549EC9 to 0x549ED8, not re-checked). Each bar's top is baseline − scale × (v − min) / span (a zero span counts as 1), and the handle is centred on the column at the value bar's top. The bars are drawn from the traced inputs (the curve-input block below). **Inferred**: the bars' colours, their draw order and the handle as a plain rectangle sized from PULLPOINT_k; the game draws the package's GRAPHBAR / GRAPHBAR_B / GRAPHBAR_C / PULLPOINT sprites, whose mapping to +0x4 / +0x8 / +0xC / +0x10 is not traced. Traced (**verified**): the widget's vtable
  0x79BEC0 holds 0x53D040, 0x550590, 0x522E40, 0x531280, 0x549E60,
  0x53D020, 0x53D030, 0x522EC0 in that order; slot 0x531280 walks the
  widget's circular list headed at +0x20 and reads floats at each node's
  +0x18 / +0x1C. The line routine 0x50DCD0 (the clone's `line_quad`) has
  three callers at 0x50E032, 0x50E0E8 and 0x50E152, none inside those
  routines, so the curve is not drawn there with it. Ghidra has no
  procedure at 0x549E60, so that routine is read from the disassembly
  (**verified**): its body calls 0x50B990 four times, each with the value
  at [esi+0x2c] and one label format: PULLPOINT_%d (0x549ECF),
  GRAPHBAR_C_%d (0x549EE5), GRAPHBAR_B_%d (0x549EF7) and GRAPHBAR_%d
  (0x549F0D); the point's value (+0x0C, argument 2), its range (+0x10 /
  +0x14, arguments 3 / 4), its index (+0x48, the widget's +0x2C counter, which the widget sets to 1 at 0x53D00E, so the names run from _1)
  and its step (+0x20 = (arg4 − arg3) × 0.1) are stored on it, and the
  point joins the tail of the widget's circular list at +0x20 (the first
  one also goes to +0x28). The names are formatted by 0x50B990 (0x43FBC0
  formats; 0x43DB50 returns the key, not traced) and attached to the
  point's sub-object at +0x24 by 0x53CF70 (PULLPOINT), 0x53CF10
  (GRAPHBAR_C), 0x53CEB0 (GRAPHBAR_B) and 0x53CE50 (GRAPHBAR): each looks
  up its first argument (0x52CF00, not the key) and calls 0x517F80 on that
  result with the key as its argument; the result goes to the point's +0x28
  (a fallback 0x52C580 runs via the global 0x8384C4; neither helper traced
  further). The walker 0x531280 (slot 3) calls 0x531050 once per point,
  with the sub-object as `this` and the point's range, value and the
  widget's +0x8 / +0xC floats (stored at the point's +0x18 / +0x1C). The
  setup is **verified**; what 0x531050 does with the bars and the handle
  is **verified**: the helpers are 0x51D080 / 0x51D180 (min / max corner as centre ∓ half size), 0x51D390 / 0x51D2E0 (move the min / max corner keeping the size, kinds 1, 7, 9), 0x51D740 (size) and 0x495590 / 0x525DC0 (set the width / height, keeping the other). **Curve inputs** (`tuning::curve_graph`, `tuning::curve_axis`, `Tuner::range_raw` / `display_limits` / `to_display`; **verified** unless marked). The loop 0x550590 runs idx 0 to 8 (`inc edi`, `cmp edi,9`, `jl`, 0x55078D to 0x550791) and calls the point routine through the widget's vtable slot 4 with the value from 0x5B3E50(P, idx, 1), the low from 0x5B2BB0(P, idx, 1) and the high from 0x5B2BE0(P, idx, 1). The point routine stores the value at +0x0C (0x549F83), the low at +0x10 (0x549F89) and the high at +0x14 (0x549F80). The wrappers 0x5B0BE0 / 0x5B0C10 return the LO / HI output of 0x5B09B0 (verified by their argument layout); 0x5B2BB0 / 0x5B2BE0 are the LO / HI of 0x5B2990 (a tracer read). If the parameter is not tunable (0x59B090 false) the game gives value 0, low 0 and high 1 (0x5506EF; the high also becomes 1 when its call returns 0, 0x5506ED).
The value (0x5B3E50): base + t × (hi − lo) of the widened raw range (0x5B2990 with flag 0, `Tuner::range`), with base = this[0x9C + 4·idx] for the ECU and this[0xC0 + 4·idx] for the turbo and t = `Settings::t`; then the display scale. The low and high (0x5B2990 with flag 1): the raw ends (0x5ACA60, then the ECU / turbo cap 0x5ACE90 and the kind rule; the 0.01 widening not yet applied), each through the display scale, swapped into order when high < low, and high + 0.01 when equal (`Tuner::range_raw`, `Tuner::to_display`).
The display scale (0x5B2810 with flag 1, `Tuner::to_display`, ECU and turbo): the native value is clamped into the native limits N (0x5ACA60 for the parameter at point 0; N_hi + 0.01 when N_lo = N_hi and an opener exists) and mapped linearly by 0x401790 (clamp, then lerp; **verified**) onto the display limits D: D_lo + (clamp(v) − N_lo) / (N_hi − N_lo) × (D_hi − D_lo).
The display limits D (0x5B09B0 with point 0 and flag 1, `Tuner::display_limits`): N, unless the opening record gives a display value at +0x10 / +0x14 (−99.99 = none). A native end is converted by × 1000, then × 0.7375621 unless the metric flag is set (the car's +4 in 0x5B09B0 equal to 1; **inferred** to be the Units option, `Settings`' `metric` in the clone). Then the pair is swapped into order and the high widened by 0.01 when they are equal.
The axis (0x550590, the widget's +0x8 / +0xC; 0x5505C2 to 0x550671): min = 50 × ⌊trunc(D_lo) / 50⌋ clamped at 0 (0x550606 to 0x550644; D_lo = 0x5B0BE0(P, 0, 1), 0 when untunable); max = 50 × (⌊trunc(H) / 50⌋ + 1) (0x550647 to 0x550671; H = 0x5B0C10(P, 0, 1), 1 when that is 0 or untunable). `tuning::curve_axis` implements it. The float-to-int step 0x75BED4 truncates (**verified**: the `fistp` with the residue fix-up).
Kind 0 (0x5B2990) takes the 'down' branch when 0x59B6D0(P) ≤ 0; 0x59B6D0 returns −1 for P 3 / 7 / 9 and 1 otherwise (0x7842A0 = −1.0), the same as the clone's `down` test (the sign **verified**; the min / max reading of 0x401050 / 0x401070 is the existing port's and was not re-checked). A point whose kind 0x5B2990 does not write (kinds other than 0 to 4) leaves its output slots unwritten (**verified**: no store on those paths), so the game reads an unset slot there; the clone keeps the untunable fallbacks for such a point (**the clone's choice**).
The drawing (0x531050 per point, **verified**; the walker 0x531280 passes the node's low (+0x10), high (+0x14), value (+0x0C), min (+0x18) and max (+0x1C) in that order): each bar is the pitch wide (the sub-object's +0x1C), its height the scale 241 (+0x20) × (v − min) / span (span 0 counted as 1), its left edge the column (+0x14), its bottom edge the baseline (+0x18, 164), and the handle (+0x10, sized by the package) is centred on the column at the value bar's top. The columns are x = −227 + k × 50.5556 (the widget's +0x10 advances by the pitch after each point, 0x549FB0 to 0x549FCC); the baseline is y = 164 (widget +0x14 = 213 + (−49)). The clone draws these in `screens::curve_bars` at the screen centre (320, 240), and the nitrous widget keeps its own lines. The tests are `tuning::curve_tests`.
* **Tuning screens' other messages** (**verified**, 2026-11-01;
  the slider screen 0x5650D0, the graph / NOS screen 0x5654E0):
  * Sounds (0x458520 in 0x5650D0): Up 0, Down 1, Right 8, Left 9,
    BUTTON1 0xB; Back 0x51DE70(_, 10), BUTTON2-type Tip 0x51DE70(_, 2)
    (`fe_rules::tuning`).
  * BUTTON1 0xC519BFC0 (the help bar's 0x800 "Default" in the tuning
    session; keyboard C): the selected row's parameter (row +0x54 →
    +0x68), or the graph widget's (+0x50 → +0x4: 23, 24 or 16), goes back
    to the middle: 0x50F9B0 → 0x59B160 (`add ecx, 0x14; jmp 0x59AFF0` on
    the tuning object 0x83A540) — one byte for 0–22, all nine for 23 / 24 —
    and 17 resets 16 too. For 9–15 (and always on the graph screen) the
    car is refitted (0x50FDA0: the sliders to 0x839C40 +0x3A8 and
    0x639CF0; 0x50FDD0 → 0x5F6C30(1)); the rows re-read their values
    (0x530DA0 over the list), which 0x5B3E50 computes from the bytes.
    Clone: `Settings::restore`, `Game::restore_tuning`.
  * Tip 0xC519BFC3 (the help bar's 0x400000 "Tip", 0x52D180 maps that
    bit to it): a one-button dialog (0x558440) with the parameter's hint
    0x510120(p): 0/4 0xD7C2416A, 1/5 0x6AD0772F, 2/6 0xC4553F70, 3/7
    0xF51BD160, 8 0x184AB1A3, 9 0x2EA43025, 10–15 0xE2E98554, 18
    0x76E721C7, 19/20 0xC7E6E649, 21/22 0x9DCC6E8B; on the graph screen by
    category 4 0xDACF52C1, 5 0xCD085170, 6 0x46B41BB8. Reached by the
    mouse (see "Front-end mouse"): the help bar's Tip (bit
    0x400000 → 0x52D180 → 0xC519BFC3) and a click on a row's "Tip" mark
    (0x565060). Ported (`Game::tuning_tip`; the dialog's button label OK is
    **inferred**). The keyboard key that raises pad bit 0x400000 is still
    not traced.
  * PAD_START 0xB5AF2461 while not in the dyno chart (0x83A7A4 = 0) and
    not on the same tick (+0x150 ≠ 0x8651A8): refit (0x5F6C30(1)), hide
    the help bar, 0x5F4250(1) (restart the test run with the new
    settings, **inferred** from 0x5F4250's use at the chart's entry).
    Corrected: 0x5F4250(p) is 0x5F41F0(8, p), and with 1 it is the
    call Resume Test Run (0x53C7E0) and the pauses make (resume the game),
    so Start applies the edits to the running car and resumes a paused test
    run; it does not restart it. Ported (`Game::tuning_start`; the clone's
    refit rebuilds the car's handling as the run's load does, **inferred**
    equivalent of 0x639CF0; with no run paused nothing happens,
    **inferred**). 0x0C407210 (Select) calls the row's vtable +0x24 and
    0x9803F6E2 the graph widget's +0x24: not traced.
* **Tuning screens' layout and numbers** (**verified** unless marked,
  2026-11-01; compared with <work dir>'s frames of the original under
  Wine, <work dir>/dyno-tmp/full-m2…m7; supersedes the
  **inferred** 40 px rows, handle travel and highlight colour of "Slider
  screens" above):
  * Row geometry (the option list 0x552790): 0x51FF90 reads TITLE_MASTER
    (+0x5C) and DATA_MASTER (+0x60): +0xFC / +0x100 the title box's top
    left, +0x10C / +0x110 its size, +0x108 its height, +0x11C / +0x120 the
    data box's top left, +0x114 / +0x118 its size. 0x549860 builds each
    row at +0xFC (+0x10C, +0x114, +0x11C passed to 0x522B00 → 0x51E1C0)
    and then adds +0x128 (0) + +0x108 to +0x100 and +0x120: rows are the
    title master's height apart (42.1 on the sliders, 29 on the
    drivetrain; the earlier note that 0x551080 feeds +0x108 was about
    another object). The list's name prefixes are read by 0x544560 /
    0x5445B0 with the row's index: OPTION_NAME_ (+0x2C), OPTION_DATA_
    (+0x80, the value), OPTION_BACKING_ (+0x24), LEFT_ARROW_ / RIGHT_ARROW_
    (+0x4C / +0x50), SLIDER_BASE_ / _KNOB_ / _HANDLE_ / _BEG_ / _END_
    (+0x78 / +0x7C / +0x84 / +0x88 / +0x8C), SLIDER_B_HANDLE_ (+0xB0),
    OPTION_DATA_B_ / _C_ (+0x5C / +0x60), TIP_TEXT_ (+0x64, rows other than
    9–15).
  * Placement 0x530AB0 (row slot +0x10): `off` = −6 when the row has both
    end labels, `bc` = the row's +0xBC (0x549860's fifth argument: 10.0 for
    the final drive in 0x549C30, else 0). Name at (row x, row y + bc − 3);
    left arrow's top left at the data box's (x, y + bc + off), right arrow
    at the data box's right less its width; the base from data x + arrow
    width + 9 to the right edge less arrow width + 9 (its width set by
    0x495590), same top; OPTION_DATA_B_ at the left arrow's left,
    OPTION_DATA_C_ with its right at the right arrow's right, TIP_TEXT_
    centred on the base, all with their top at the base's bottom; the
    value at (data x − 50, data y + bc − 3).
  * Fill 0x5273B0 (slider vtable 0x79AE5C slot +0x18; slider fields at
    row +0x6C): with span = max − min (0 → 1), travel = base width − 2 ×
    +0xAC (4.0), SLIDER_HANDLE_ is the green bar from base x + 4 + (lo −
    min)/span × travel, (value − lo)/span × travel wide, SLIDER_B_HANDLE_ the
    red bar from there, (hi − value)/span × travel wide (its "ToLesser"
    script run), both at base y + 5; SLIDER_KNOB_'s top left at (value x −
    4, base y + 4). min / max are the display limits (0x5B0BE0 /
    0x5B0C10), lo / hi the range (0x5B2BB0 / 0x5B2BE0), value 0x5B3E50
    (0x530DA0).
  * Text 0x53CD40 (row slot +0xC): the name "%s (%s)" with the unit of
    0x50FFB0(p, the car's +4): 3 / 7 "cm" 0x5E1F61EE or 0x6382EDE0 (no
    English string), 10 "KM/H" 0xCBBC8BF8 or "MPH" 0x8569AB44, 17 / 23 / 24
    "Nm" 0x5B81D613 or "ft-lbs" 0xACB8DFF3; the value "%$2.1f" for 10–15,
    else "%$2.2f" with TIP_TEXT_ set to "Tip" 0xCBBCB1C5.
  * Arrows: 0x549860's fourth argument is 0 at every tuning call
    (0x549B50 … 0x549E10), so the left arrow takes 0x374F6465
    (U2_MENU_MINUS) and the right 0x760BDC9D (U2_MENU_PLUS); the row's
    HIGHLIGHT / UNHIGHLIGHT scripts show them on the selected row only.
  * Display numbers (0x5B09B0 / 0x5B2810, flag 1): a native end with no
    display value in the opener is converted per parameter: gears 10–15
    to road speed (0x59B5D0 with the flag, the same radius × rpm × 2π/60 /
    final drive / ratio as without it) then × 3.6 (km/h) or × 2.237 (mph);
    3 / 7 × 100 (cm) or × 39.37 (in); 23 / 24 × 1000 (× 0.7375621 unless
    metric); the rest unchanged. A value maps linearly from the native
    limits (gears: from the limits' speeds, in order) onto those. The
    block 0x59B5D0 reads is the tuning object's car, which the sliders
    tune in place, so a gear's speed follows the tuned final drive. Clone:
    `tuning::slider_view`, `tuning::unit_label`.
  * The tuning master (UI_PerformanceTuning_Master) stays drawn under a
    category's screen (the frames show its header and strip over the
    panel; 0x55DDB0 pushes the category screen and Back fades the
    master's BUTTON_MASTER back in, 0x5650D0).
  * Graph bars: 0x549E60 formats PULLPOINT_%d, GRAPHBAR_C_%d,
    GRAPHBAR_B_%d, GRAPHBAR_%d in that order and the setters take them so
    (from the disassembly's stack slots): +0x4 value = GRAPHBAR_k, +0x8 low
    = GRAPHBAR_B_k, +0xC high = GRAPHBAR_C_k, +0x10 handle = PULLPOINT_k.
    The clone now sizes those package sprites, so the bars carry the
    package's colours (teal, the "Maximum" red) instead of the old
    **inferred** gold / grey.
  * Nitrous widget (0x523060, vtable 0x79B804; set up at (0, 0) by
    0x5578D0 with parameter 17, so 0x550850 puts its origin at (−148,
    122)): fields +0x70 value 17, +0x74 value 16, +0x78 max 17, +0x7C /
    +0x80 range hi / lo of 17, +0x84 max 16, +0x88 / +0x8C range hi / lo of
    16; objects +0x90 0x11495CD6 (the line), +0x94 0x9C90D375 (the dot),
    +0x98 0x01183698 (vertical), +0x9C 0xD35A7923 (horizontal), +0xA0 /
    +0xA4 0xD1CDA594 / 0x9345E4A7 (the arrows). The draw 0x5312D0 (slot 3):
    the line from (rLo16 / max16 × W, rHi17 / max17 × H) to (rHi16 / max16
    × W, rLo17 / max17 × H) above the origin, its length the distance and
    its rotation 0x43E6D0's angle × 360/65536 (0x51CEE0), centred between;
    the vertical H tall at value 16, the horizontal W wide at value 17 from
    the origin's x, the dot where they cross. W / H are +0x18 / +0x1C: the
    constructor stores 455 / 241 but the frames under Wine match the two
    markers' own sizes 319.3 / 202.1, which the clone uses (**inferred**:
    which code sets them). The widget's slider (+0x20) is drawn by 0x5273B0
    about SLIDER_BASE_1 (its place there and its 4.0 margin **inferred**
    from the frames).
* **Dyno chart and record use the tuned car** (**verified** 2026-11-01):
  the chart (0x557A20 → 0x53D0B0) draws from 0x8900AC[0x83A64C] → +0x4 →
  +0x20, the player racer's tuning block (+0x1F0 idle, +0x1F8 max,
  +0x200 torque), the racer the dyno race builds with the mode's
  settings and the sliders copied over; the record 0x523340 reads the
  tuning object's car ([state] +0x10 → +0x10, 0x5B5360(1) → +0x30), whose
  block the screens' writer 0x5B3FB0 tunes in place (it writes through
  the tuning object +0xC with the uniques undone and redone, 0x599DA0 /
  0x5997F0). The 0-60 / 0-100 times are that set-up's +0x24 / +0x28,
  written only by 0x5B06B0 inside the set-up 0x5B4DD0 (its one caller,
  0x5B4F08), so they come from the last set-up, not from the slider edits
  since; when that set-up re-runs (0x5B5360 rebuilds when +4 is −1) is
  **open**. Clone: `Game::dyno_handling` (the upgraded record tuned with
  the dyno's settings) for the chart lines and the record (before
  2026-11-01 both used the untuned car).
* **The car on the dyno's rollers** (2026-11-01; issue: the
  car floated, the animation was stiff, the wheels did not turn):
  * **Rollers** (**verified**): the dyno manager (made only in race type 8,
    0x5FC4E5) holds four objects at +0x40 (0xD0 each, 0x56D060: the model
    XO_DYNOROLLERA_1A_01 by name hash, 0x79DC24; a solid of STREAML4RG).
    0x56D1B0 (at the manager's creation 0x5FC4FF and each pull's reset,
    0x56E708) puts roller i (wheel i) at the car's centre of mass (+0x60) +
    forward × (wheel x ± 0.26 − centre-of-mass x; + for wheels 0, 2): no
    sideways part, on the centre line; z = the floor under it (0x5E72F0)
    − 0.175; matrix the car's (0x4012B0). The car record [car +0x20]: +0
    centre of mass (the finished-career save's Lancer (0.29, 0, 0.145)), +0x10 + 0x30·i the
    wheels. Each pull frame (0x56E6F0, state ≠ 0) roller angle +0xC0 −=
    wheel i's spin (car +0x330 + 4i, rad/s) × dt. Draw 0x56E900 → 0x56E1E0
    (four times, in the car pass after the GPS aid, 0x63178D): RotateY(angle
    × 10430.378) × the car's matrix at the roller's centre. Clone:
    `World::dyno_rollers` (four hidden placements), `dyno::Rollers`,
    `Game::seat_on_dyno` (placed at the dyno track's load, standing through
    DynoEnter as under Wine).
  * **The pulled car** (Wine, the finished-career save's Lancer, car +0x60 / +0x90 / +0x330
    read 10× a second through a Dyno Run): the body stayed within 4 cm of
    its place and 3 mm of its height, nose up ≤ 0.6° under the pull and down
    0.9° under the brake; all four wheels spun up together (gear 2 at 1 s:
    17.4 rad/s, 1950 rpm; 47.7 → 93 rad/s in third in 1.6 s; top 162 rad/s
    at about 6.7 s), the car's speed +0x430 = wheel spin × radius (no
    slip). The physics' velocity runs free while 0x5A5C30 overwrites the
    body's x/y each sub-step with a point sprung to the held place (k 150,
    damping 15, × body +0x168 × the sub-step). The world runs on behind
    UI_PerformanceDyno_Results (the sim clock and the spin-down went on
    there); in the Dyno Shop menu it stood. Clone: the chart's
    `DynoPull` (settled on its springs first, aero off as the hold's
    0x5A49B0, `CarState::aero_off`) is carried over to the drawn car
    (`Game::dyno_pose`: wheel turn, spin, compression change, body tilt
    and rise about the centre of mass, the drivetrain for the sound; the
    rollers turn by the wheels' spin), kept spinning down behind the
    results (`Game::dyno_spin`); the dyno camera keeps the car's rest
    frame (`dyno_camera`). Clone measured (the maintainer's `dyno_motion.sh`): 1 s
    17.5 rad/s / 1950 rpm, nose up 0.34°, rise < 1 mm. The third-gear
    pull was 1.75× slower than the original's until the dyno boost (see
    "The pull's power" under "Dyno chart") was ported (2026-11-01).
  * **DynoEnter** (Wine frames 2026-11-01: the car on the pit lane's floor
    and up the ramp onto the rollers): the scene car is driven physically
    (mover 5, 0x5A40B0: target place / heading, teleported only past 1.1 m
    or 15°), so it stands on the floor. The clone placed it at the scene's
    heights, above the floor before the ramp (it floated); then
    (2026-11-01) it kept the scene's place and heading and stood the car on
    the floor under its wheels. The scene-camera follow-ups: the car runs the NIS mover itself (as
    every scene's, observed under Wine; `p17-scene-follow-ups.md` §1) and
    stays where it ends on the rollers (no re-placement on the grid).
* **Tuning in the physics** (`tuning::apply`, applied to the player's car
  in `drive_in`; **verified** unless marked): the car's set-up 0x5B4DD0
  runs 0x5B4790 on the tuning object (car +0x374: +0x8 upgrade state,
  +0xC tuning block, +0x14 the 41 slider bytes, +0x40 the bases) when
  0x59B0F0 counts a tunable parameter. Slider bytes: 0–22 one per
  parameter, ECU (23) points at +0x17, turbo (24) at +0x20; byte 0–200,
  t = clamp((b − 100)·0.01, −1, 1) (0x59AEF0), stored as
  round(t·100 + 100.5) (0x59AF90); new settings are all 100 (0x5A7940;
  0x59AFF0 resets one, −1 all). Fields (0x59B210, record offsets):
  0 +0x284, 1 +0x288/+0x28C, 2 +0x290, 3 wheels +0x128 (+0x30 each), 4
  +0x2A4, 5 +0x2A8/+0x2AC, 6 +0x2B0, 7 wheels +0x188, 8 +0x380 (steering
  ratio), 9 +0x2C8/+0x2CC, 10–15 +0x2C0 + 4p (gears 1–6), 16 +0x3B0, 17
  +0x3BC (nitrous), 18 +0x38C (brake balance), 19 +0x1EC/+0x1F0, 20
  +0x20C/+0x210 (tyre grip), 21/22 +0x374/+0x378 (downforce), 23 torque
  +0x310 + 4i, 24 turbo +0x334 + 4i. Base (0x5A7DB0): the pair's mean for
  1, 5, 19, 20; stock − current (first wheel) for 3, 7; torque − turbo
  for 23; else the field. 0x5B4790: bases of the tunable 0–22 (zero
  otherwise); each tunable 0–22 set to base + (hi − lo)·t through
  0x5B4680 (clamp 0x5B3EF0, which stores the clamped slider back, then
  the writer 0x5B3FB0, then re-clamping the sliders it moved: every
  gear, the nitrous partner, all curve points) — gears go raw to the
  writer in this pass and through 0x5B4680 in a second; then 23 and 24
  point by point (bases of all 9 first) straight to 0x5B2C10 without
  spreading; finally, if the torque total moved by more than 0.001,
  0x59B3D0 adds the difference back evenly (ninths).
  Range lo..hi (0x5B2990) by the opener's kind (+0x06), r = its range,
  L..H the hard limits, c the centre: 4 [r0, r1]; 1 [max(L, c + r0),
  min(H, c + r1)]; 2 [L, min(H, c + (H − L)·r1)] (3/7/9: [max(L, c −
  (H − L)·r1), H]); 3 [max(L, c + (H − L)·r0), min(H, c + (H − L)·r1)];
  0 by 0x59B6D0 (−1 for 3/7/9, else 1): [max(L, c − r1), H] or [L,
  min(H, c + r1)]; lo = hi gets hi + 0.01. c (0x5A7A00): the stored
  base; 0/4, 2/6, 21/22 their larger, 3/7 their smaller. Hard limits
  (0x5ACA60, block = record +0x110, stock record of the type): min/max
  of 0/4 block +0x174, +0x194, +0x5C4, +0x5E4; 2/6 +0x180, +0x1A0,
  +0x5D0, +0x5F0; 3/7 −(+0x604), −(+0x608), 0; 8 +0x270, +0x5BC; 9
  +0x1B8, +0x3D8; 21/22 +0x264, +0x268 and each + (+0x590/+0x594) +
  (+0x5A0/+0x5A4); 23 per point +0x200 and +0x200 + +0x420 + +0x4E0; 24
  +0x224 and +0x510; then widened by the opener (0x59B170: kinds 2/3
  L += (H − L)·r0, H += (H − L)·r1; 0/1 L += r0, H += r1; 4 L = min(L,
  r0), H = max(H, r1)); 23/24 not below 0. Pairs 1/5 (0x5A7CC0) and
  19/20 (0x5A7B90): per member stock (+0x178/+0x198, +0xDC/+0xFC) to
  upgrade target (+0x5C8/+0x5E8, +0x54C/+0x56C), each widened, the mean
  of the two. 16 is [1, 11] unwidened, 17 [0.1, 0.9], 18 [0, 1]. Gears
  (car's own block, unwidened): the ratio for 111.757 m/s to that for
  1.1176 m/s (0x59B5D0: radius / v × rpm(+0x1F4)·0.10471967 / final
  drive, radius front +0x50 below 0.2 drive bias +0x1C0, rear +0x80
  above 0.8, else between). Curves (0x5ACE90): H no higher than the
  largest base, and at the point's rpm (idle + (max − idle)·i/8) no more
  power than the base curve's peak (0x5AC990, 50 rpm steps).
  Writers (0x5B3FB0): direct for 0, 2, 4, 6, 8, 18, 21, 22; 9 to both
  fields; 3/7 stock − value per wheel; 1, 5, 19, 20 at (clamp(v, L, H) −
  L)/(H − L) of the mean limits, each member that share of its own
  lo..hi; gears 0x5B3070 → 0x5B2EE0 (a rise pushes the next lower gear
  to ≥ 0.05 above, a fall the next higher to ≤ 0.05 below, recursively;
  what a gear cannot take beyond its range comes back; gears past the
  car's zeroed); nitrous 0x5B3110 (the value to its field, the partner
  base − (its span)·(v − base)/(this span)); curves 0x5B2C10 (one point
  clamped; ECU torque = point + turbo for all nine, turbo moves torque
  with it; nothing written when the point equals its base). The clone
  does all of this (`tuning::apply`). 0x5B2C10's spreading (the tuning
  screens' path, flag 1; **verified**): d = v − the point's base and the
  remainder r = d; the neighbours i+1, i−1, i+2, i−2, … i±8 (then i+1
  again), skipping those off the curve, each take d × the next entry of
  0x7FC674 {0.25, 0.25, −0.25 × 6, 0} — not past r on r's side — and
  after nine moves −r, each clamped into its own range with r += what it
  moved, at most 20 moves while |r| > 1e−7 (0x7A035C); so the curve keeps
  its total (`tuning::nudge`). **Verified** (0x5B3FB0, 0x5B4032–0x5B4098):
  the pair writer clamps x = min(H, max(v, L)) on the mean limits and
  divides f = (x − L) / (H − L) at 0x5B407C with no guard, then stores
  each member lo + f·(hi − lo). A zero mean span has x = L, so f = 0 / 0,
  a NaN under the x87's masked invalid operation, and both members take
  NaN. The clone's pair branch of `write` (src/tuning.rs) divides the same way.
  The no-opener case (**verified as undefined in the game**): with no
  opener, 0x5B2990 returns at once. 0x59AE50 searches the table at 0x86B340
  (24-byte rows; the parameter byte at +5; a nonzero byte at the row's first
  pointer + 0x50 + the value at [this+8]) and finds none, so 0x5B29B1
  jumps to 0x5B2B9C and the routine returns (`ret 0x14`) without writing
  either output. In 0x5B2EE0 those outputs are its own stack slots: the low
  limit is its parameter slot, which still holds p (so p's bits as a float),
  and the high limit is an uninitialised local (`local_4` in the Ghidra view,
  the slot handed to 0x5B2990 as its high output). 0x5B2F1B reads that local
  on every path, so a pushed gear without an opener is clamped to [p's bits
  read as a float, the leftover stack value]. The game's result therefore
  depends on undefined stack contents. The clone keeps its documented choice:
  it leaves the value unclamped on this path (a divergence, since the
  leftover stack value is not modelled). Not traced: whether a real car
  pushes a gear without an opener (which car data fills the table at 0x86B340 is not traced; the table is written by 0x59AE30 (0x59AE34, **verified**), whose only direct caller is 0x512D5C (**verified** by a grep for direct calls), which is not decompiled).
* **Dyno chart** (**verified** unless marked): Dyno Run (0x53C840) opens
  UI_PerformanceDyno_Chart (factory 0x55AF30 → 0x557A20, messages
  0x54A810). 0x53D0B0 builds the graph 0x5202C0 at (73, 79), 486 × 312,
  axes 0..10000 rpm × 0..600 (metric 813.49 Nm 0x444B5F69 / 447.42 kW
  0x43DFB5C0; the units option 0x5EA8C0). Lines from the engine (idle
  +0x1F0, max +0x1F8, the 9 torque values +0x200, read with 0x5764E0):
  32 points (max − idle) / 31 apart, torque × 1000 N·m (× 0.7375621
  ft-lbs), power torque × 1000 × rpm × 0.00010471204 kW (× 1.3410221
  bhp); colours 0xFFAA8000 torque, 0xFF1A95AF power. After a run
  (0x83A658 > 0) the first run's record (0x83A734, 0x6C bytes: max rpm
  +0x68, torque values +0x34) adds two lines with alpha 0x82. Labels
  `%$d` of 0, ⅕ top … top (truncated): bin_hash_from(0xEEC8134F, "1".."6")
  power (left), (0x22B57EB7, …) torque (right); 0xE2A8254C "Power ( %s )"
  (0x06285E80), 0x2345F69E "Torque ( %s )" (0x38D548D2), units "bhp"/"kW",
  "ft-lbs"/"Nm". Draw (0x50DE90): the line reaching the highest rpm
  sets the sweep `first + (last − first) × p` (p = graph +0x34); segments
  up to it (the last one cut), y up from the bottom edge (0x4FE050
  clamps to the axes), a yellow 0xFFFFFF00 line at the sweep while p < 1.
  p (0x56D450) is the dyno's rpm / (max rpm − 200), clamped 0..1 (0 in
  state 2 unless the car's gear +0x434 is 4). The dyno manager (vehicle
  +0x1E0, update 0x56E6F0, state +4) runs the car itself (+0x434 its
  gear, as the shifting shows): state 1 releases the brake (0x5EB900(0)),
  shifts to a target gear (vt+0x28) and revs it at full gas, raising the
  target at 3000 rpm (0x79DB9C) until gear 4; 2 gives full gas
  (0x5EB920(1)) and feeds the car's rpm (+0x440; the tweak 0x7FBED8 when
  it is not 100) into a moving average (0x576980, +0xC; its mean +0x18 is
  the rpm), at p = 1 shifts down (vt+0x28(−1)) for 3, which lifts off and
  brakes above gear 2 until the speed (+0x430 × 2.237) is 0, then 4; the
  sweep's pace is the simulated engine's. At p = 1: group 0x11BF9934 "FULL" (enters "FadeIn"), the run is
  recorded (0x523340: peaks 0x5B5820 torque / 0x5B56F0 power over idle..max
  in 50 rpm steps, strictly-higher wins; records below), MAX_TORQUE_DATA / MAX_HP_DATA "%$3.1f %s^@%$d rpm"
  (0x76F2BC60 / 0x43AEEF78; "n/a" when not above 0), Chart_Legend "Show"
  (0x54A630). Accept (0x406415E3) ends the run early (0x56E690) and plays
  0xDE6EFF34 on 0x47FF4E7C; 0xE1FDE1D1 then opens
  UI_PerformanceDyno_Results. Records (0x523340, 0x6C bytes): +0x00 the
  computed top speed (0x5B5940), +0x08 / +0x0C the 0-60 / 0-100 times
  (dyno car +0x24 / +0x28 × 4000 + 0.5 ticks), +0x34 the torque values,
  +0x58 / +0x5C peak torque / power, +0x60 / +0x64 their rpm, +0x68 max
  rpm; the first run (count 0x83A658 = 0) to 0x83A734, else the current
  0x83A65C moves to 0x83A6C8 unless its power is 0; the new run is the
  current. UI_PerformanceDyno_Results (0x55AF90 → 0x557BE0, filled by
  0x54A980): rows CURRENT / PREV / FR, `<row>_DATA_1`..`_5`: 0-60 and
  0-100 ("%s %s", "Sec." 0x29F4BF03), top speed ("%$3.1f %s", × 3.6
  "KM/H" 0xCBBC8BF8 / × 2.237 "MPH" 0x8569AB44), MAX_TORQUE_DATA (× 1000,
  "Nm" 0x5B81D613 / × 0.7375621 "ft-lbs" 0xACB8DFF3, rpm + 0.5
  truncated), MAX_HP_DATA ("kW" 0x5B81D5BA / × 1.3410221 "bhp"
  0xCBBC6512); a time of 0 or 0x7FFFFFFF or a figure not above 0 reads
  "n/a" (0xA4C02484). The chart's Accept (0x54A810) records a run still
  going (0x523340), stops the dyno (0x56E690(0)) and plays 0xDE6EFF34 on
  0x47FF4E7C; its end (0xE1FDE1D1) switches to UI_PerformanceDyno_Results
  (0x5377B0, 0x7FA234). 0xC98356BA is not a way back: it re-installs the
  joystick handlers (0x52CDE0) and drops the help bar. The results
  (0x53D520): Accept / Back play 0xDE6EFF34, then Accept switches to
  UI_PerformanceDyno_MAIN (0x7FA21C) and Back to the chart (0x7FA230).
  The dyno manager (made by 0x56E620 only in the dyno race, race type 8:
  the average 0x579F30(20), so 20 samples): its start 0x56D350 puts the
  car in neutral at rest (+0x434 = 1, +0x440 / +0x42C / wheel speeds
  +0x330.. 0), the automatic gearbox, the average reset to 0 (0x5769E0:
  every sample, count 0) and asks for gear 2 (target +8 = 2); state 1
  shifts to the target with the gas off, in the target gear gives full gas,
  pushes 0 into the average and raises the target once the rpm (0x4018C0)
  reaches 3000, and goes to 2 in gear 4 — the third forward gear (0 R,
  1 N); 4 stops it (0x56E690(1)). The average (0x576980): count up to 20,
  sum += x − the replaced sample, mean = sum / count. Each frame the four
  rollers (placed under the wheels by 0x56D1B0, ±0.26 m, 0.175 m down)
  turn by the wheel speeds × dt. In state 0 (stopped) the progress is the
  last mean's, so a chart reopened from the results shows the finished
  pull. The clone ports the manager (`dyno::DynoPull`, stepped by the
  chart each frame; the run recorded when the progress reaches 1).
  The manager is the car's controller in the dyno race: the vehicle
  update 0x6016B0 (from 0x607784) runs it in place of the driver's input
  for race type 8 (type 7 runs 0x5FC520), once per world frame with that
  frame's time 0x865150 (the world step 0x601A70's, 0x58134E); nothing
  in it holds the body (**verified**). The chart takes no Back
  (**verified**: 0x54A810 handles only 0x406415E3, 0xC98356BA and
  0xE1FDE1D1 and calls no base handler), so the way out is Accept to the
  results; the clone ignores Back there. **Open** (clone side): the manager
  is stepped at 1/60 s, but the game steps it once per world frame with that
  frame's time (0x865150, see the vehicle update above). The clone's car is
  driven by its own physics on a flat floor (`World::flat`; its movement
  is drawn on the dyno's car, see "The car on the dyno's rollers"). Traced: in race type 8 (only) 0x5A4420
  sets the four floats +0x1F8..+0x204 of the car's object at +0x1C, one
  per wheel position (+0x210 + 0x10·i), to 0x56D4B0 on the manager (car
  +0x2C → +0x1C → +0x1E0): the highest of its four rollers' profiles
  (0x56D090: d = (p − roller +0x70) · roller +0x80; within the roller's
  half size r (0x611040), max(0, √(r² − d²) − 0.215), else 0). What reads
  those floats — so whether the rollers hold the body — is not traced
  (**open**; the clone's floor is flat and nothing holds the car).
  **The pull's power** (**verified** 2026-11-01): the force pass
  0x5B0530 opens (0x5B053D–0x5B0564) with: if the mover's hold byte
  (mover +0x84, `[esi+0x7C]` with esi = mover + 8) is set and the car's
  dyno manager (`[esi+0x54]` → +0x1C → +0x1E0) is in state 2, the
  engine's power factor (`[esi+0x40]` = mover +0x48 → +0x80) = 1.5
  (k_DynoPullPower 0x7A1A28, .rdata, read nowhere else). It runs at the
  start of every sub-step, after the clutch tidy-up 0x5A1410 put the
  power back to its state's (1 locked), so the whole pull runs at 1.5:
  Engine_Torque 0x594310 gives T(rpm)·scale·throttle·1.5, and the
  closed-throttle drag term −(1 − throttle·power)·drag(rpm) turns into
  +0.5·drag(rpm). Nothing else in the physics reads the race type or the
  dyno flag (0x89E7B0 is read in 0x59..0x5B only by 0x5A4420; 0x89E7E6
  not at all). The hold is set by the pull's reset (0x56D350 →
  0x599210(1)). Under Wine (the finished-career save's Lancer, Circuit settings, the
  fixture the finished-career save, car +0x434 / +0x440 / +0x330 read every
  ~13 ms): first gear 1.43 s, second 0.83 s, third 5.12 s (2659 →
  8848 rpm), 3437 / 5015 / 6570 / 7833 / 8871 rpm at 1 / 2 / 3 / 4 / 5 s
  into third. The clone without the boost: third 8.97 s; with it
  (`CarState::dyno_boost`, set by `DynoPull` in state 2): 1.40 / 0.88 /
  5.13 s, the rpm and wheel spin in third within 1% (tests/dyno_pull.rs).
  The short gears differ by up to 0.05 s: the manager asks for a shift
  two of its frames after raising the target, and the clone steps it at
  1/60 s where the original ran ~120 frames a second under Wine. The 0-60 / 0-100 times (record +0x08 / +0x0C)
  read the dyno car's +0x24 / +0x28: its set-up object (the car +0x10;
  +0 the type index, +4 the class, +0x30 the handling, +0x374 the tuning
  object, +0x55C the acceleration curve) +0x14 / +0x18, written by
  0x5B06B0, which the set-up 0x5B4DD0 runs after the acceleration rating:
  a standing start at full throttle in 1/60 s steps (at most 999,999):
  rpm = v / r · ratio · final drive · 9.549304 (r and the final drive
  blended by the rear torque share, from first gear); while rpm < the
  gear's up-shift rpm (0x5AC7E0) − 100 (0x7FC294) the push is torque(rpm)
  · ratio · final drive / r / mass, else a shift step with no push, the
  clutch s (from 1) moving −3/s (0x7FC298[0]) down to 0, then +1.5/s
  (0x7FC2A0) to 1, where the next gear comes in — and a shift step always
  hands the next step to a push at the current gear (its flag cleared
  there); a = push − v²·drag/mass, x += (v + a·h/2)·h, v += a·h; the first
  t with v · 2.237 ≥ 60 × the type's factor (0x7FC120, 31 types) is +0x14,
  ≥ 100 × 0x7FC1D8 is +0x18, x ≥ 400.5 m (0x7A20C0) gives +0x1C = t and
  +0x20 = v; it stops when all four are non-zero. The results print them
  with 0x577250 (flags 0) and "Sec." (`handling::launch_times`,
  `DynoRun`). The results' ratings bars: 0x54A980 ends with 0x5103D0 (on
  the screen), which sets each bar (+0x54 acceleration 0x5B53B0, +0x98 top
  speed 0x5B6A30, +0xDC handling 0x5B54A0, on the dyno car's set-up
  0x839C60) to its rating, the old value to +0x2C, clamped to the bar's
  max +0x24 / min +0x28, and redraws it (vtable 0x78F544 +0x18 →
  0x4FF530 → 0x540D30). But the results screen (0x557BE0) never binds its
  bars (no 0x540C20: their objects stay null, their range 0..0), so every
  value clamps to 0 and the redraw touches nothing (0x51D740 / 0x51D080 /
  0x51D390 return on a null object): the bars keep the package's look, as
  the clone draws them (**verified**).
* Compressed texture packs: a P8 texture's palette offset moves with its
  pixels inside the block (fixes MAGAZINES_STREAMING_HIGH).
* **D-Day intro** (**verified**): a new career (0x532840: cash from the
  bonus codes, five locked car slots CAREER_SLOT_0..4 made by 0x516480 —
  0x7F8-byte records in the player garage 0x83AD90, +0x7F4 locked, +0x7F5
  has its own car — and the first opened, 0x5284E0) is at stage 0. The
  career start (0x495A30, after Start New Career's MC_Main flow sends
  0x461A18EE, and on Resume) goes to the garage once DDAY_EVENT_B is won,
  else makes DDAY_EVENT_A (or B once A is won) the current event (career
  +0x8738, 0x5410F0) and starts the city (0x5404B0). Until the car lot
  gives a car the player drives Rachel's car: the preset DDAY_PLAYER_CAR
  (GLOBALB 0x30220, 0x338-byte records read by 0x637280: +0x08 car type
  name, +0x28 preset name, +0x48 performance level — 0x5A64B0 fits every
  category's packages of levels 1 to it (level by level, categories 0–9,
  each part through Perf_FitPart(part, 1, 0): a part whose required parts
  (+0x54) are off stays off, and the parts it replaces (+0x40) come off —
  so Rachel's level 2 has brake part 41, not 40), −1 means the per-id flags at
  +0x2F4 — and +0x4C a part hash per slot, 0 none, 1 stock, slots 0x19
  and 0x61–0x68 skipped): a 350Z at level 2. The D-Day events are not
  races: +0x35 counts goals of 8 bytes from +0x38 (kind byte, hash at
  +0x3C); kind 4 is a shop. Entering a shop (0x54CB00) wins the current
  event when the shop is its goal (0x4FFDE0; 0x541430: no purse, no
  reputation, category 0 is filed nowhere): A's goal is the car lot
  CC_CAR_LOT_1, which then hands on to B (goal CRIB_1, the garage, which
  opens with A won). B is stage 0's final event, so stage 1 follows
  (0x532C70). The spawn point (career +0x8750.., 0x863490: x, y, z, the
  heading at 0x8634A0) is used by the city start (0x5406BE, 0x540936)
  only with career +0x8744 (0x863484) set, which a new career's D-Day
  start (0x54AF90 mode 6) does not set: the intro starts at the track's
  free-roam start, as in the clone (**verified**).
* **D-Day script** (`frontend/dday.rs`, **verified** unless marked):
  career +0x713C (0x861E7C: +0 state, +4 the state's start tick, +8 a
  message read, +9 the SMS screens closed). The career reset (0x532A43)
  zeroes it; the city start (0x5404E4..0x5404FE) sets it to 9 (done)
  when the career's intro movie has already played (0x86347C = career
  +0x873C, saved with the career: see **Movies**; the movie's own D-Day
  start runs 0x5404B0 before setting 0x86347C, 0x54AFD5 / 0x54AFDA), so
  the script runs once, in the drive right after the movie, and a career
  saved during D-Day resumes with it over. A race started from the world
  map also sets 9 (0x5325F0, **inferred** role). The engage tips have no
  script gate: the player update 0x602660 calls 0x5EB050 every frame,
  whose tip lookup (0x5EB0F8 → 0x532D50 → 0x529C50) runs in a career's
  free roam (0x500B60) with no world challenge (0x501690); the inbox's
  +0x1410 that 0x529C50 compares with the trigger is only ever zeroed
  (0x532901), so it never blocks one (**verified**). **Timing**: states
  3, 5 and 6 wait for Rachel (0x525A00), so with speech the script lasts
  as long as her lines (UG2_Speech entry 0x27: the scene's line 9 10.2 s
  over the career NIS's 13.2 s, state 3's line 38 12.8 s, 5 → 6 line 10
  6.2 s, 6 → 7 line 20 4.8 s). The clone with audio (`--flow newcareer`,
  `NFSU2_AUDIO_WAV=-`, the start scene watched): state 3 2.0 s after the
  scene, 6 at 15.5 s, 7 at 21.95 s, 9 at 22.0 s, the car held until
  then; the original under Wine with a speech object was seen still in
  state 6 20 s on (2026-10-30, the HUD agent's capture), which fits.
  With no speech object (the original with its audio driver off, the
  clone's headless shots) nothing waits: about 6 s. Flow event 1
  (0x60B3B0 → 0x513BC0 → 0x5023F0) starts an idle
  script in a career with no race and no car of the player's own yet
  (career +0x873D, set by 0x500BD0 when a car slot becomes current).
  0x551330 (career free roam, race state 4) steps it (0x5342A0) instead
  of counting the driving time (0x5513AD), on the clock 0x865188; the
  speech test 0x525A00 is true without a speech object (0x82BA30 +0x2C);
  states 3–6 wait for it and speak (ported 2026-10-30, see **Rachel's
  speech**). States: 1 → 2 queuing the HUD prompt kind 0x14 with value 2
  (0x4CA670); 2 → 3 after 2 s; 3 → 5 (4 is skipped) adding
  SMS_INSTRUCTION (0x5338A0, once) and opening the SMS screens (0x529710
  → state 0xD; one unread → the viewer, which reads it: 0x514DC0 sets
  +8 while the script runs); 5 → 6 once 2 s have passed, +8 is set and
  the SMS screens have closed (flow event 2 in state 0xD, 0x6069C4 →
  0x497730 → 0x5024F0 sets +9); 6 → 7 after 2 s: the marker table
  rebuilt (0x533320, which during the intro holds only its goal) and the
  GPS to DDAY_EVENT_A's goal (0x532D90); 7 → 8 → 9 on the next frames:
  input mode 2 back (0x605C80(2)), the markers again, SMS_INSTRUCTION if
  still missing, the GPS to A again (a no-op when it already aims there).
  While it runs (0x502420: state not 0 or 9): the player's input mode is
  0xA (0x5EA6F2 → 0x605C80: only pause 0xBD, skip 0xBE and Read SMS 0xC2
  bound, so the car takes no driving input; 0xBE → 0x534290 skips to the
  end (0x534210) only with 0x864FA4 set, which nothing writes); the HUD
  features are the script's alone (0x60031E: from 0x40000, which 0x4CA090
  masks off; state 2 high 2 (0x502440), above 2 high 2 unless state 5
  with +8 set (0x502460), from 6 low 1, the minimap (0x502490), above 8
  low 0x60 (0x5024B0, unreachable)); the SMS slot's flashing holds in
  state 2 (0x4C8AC0); the minimap's marker test (0x4CA74D) shows nothing
  before state 7 (0x5024D0) and then car lots (icon type 11) only. The
  prompt kind 0x14 (0x4ABA30, priority 1): its slot's state 2 lasts its
  value (2 s): the D-pad 0x6FFF0767 with "Pulse"; state 3 the texture with
  "Init"; from 4 s "Init" (no sounds for this kind). **Open**: Rachel's
  voice lines (0x510ED0 / 0x510F30 on the speech object at states 3, 5,
  6, 7 and in 0x53E0C0), the SMS slot against the 0x14 slot beyond
  priority. Clone: `Game::dday`, the selftest drives it (the instruction
  2 s in, read and closed, the GPS on the car lot, the car held) and the
  `intro_prompt` (state 2: the phone prompt, kind 0x14), `intro_gps`
  (the script run through) and `intro` shots. `intro` matches the Wine
  capture of the original ($OB_PRIVATE/wine/capture.py: Rachel's SMS
  dismissed, then 20 s standing): the script is over, so the HUD is free
  roam's (0x6000A0's `+0x28 == 0` branch): Bank (high 0x80000, career),
  minimap with the GPS (low 1), tach / speedo, nitrous (see Nitrous), the
  engage widget idle (its X explore icon — the phone icon belongs to the
  prompt kind 0x14 while the script runs) and MARKER_GPS_AID. Side by
  side, 2026-10-29: everything the original shows is there. The car
  used to creep ~19 m forward at idle in gear 1 (≈2.2 m/s) while the
  script holds the input; what holds it in the original is the rest test
  (see **Rest test**, ported 2026-10-29): gas 0 with all wheels down and
  under 0.25 m/s / rad/s, every sub-step zeroes its momenta and the next
  step has no gravity. The clone's `--shot intro` now reports 0.00 m.
* **Career start scene** (`shop_scene::SceneKind::Career`,
  `Game::start_career_scene`; **verified** 2026-10-29 unless marked): the
  in-engine airport fly-over a new career opens with, between the intro
  movie and Rachel's SMS, is the NIS `Scene_CareerNis1` ("Career Start
  NIS", header hash 0x346D604C, type 4, 405 frames). The city start
  0x5404B0, with the career's intro movie not yet played (0x86347C
  clear) and the D-Day script idle (0x861E7C = 0), flags the NIS manager
  (0x4305D0: 0x827E88 +0x60 = 1; 0x4305E0 reads it, 0x4305F0 clears it at
  the track loader's state 8, 0x57FA68). The track loader's state 6
  (0x57F9C4) then runs 0x437270, which picks the scene with 0x436840(4,
  …): case 4 takes 0x865598 (BinHash("CareerNis1"), set by the static
  initialiser 0x77D8E0) while 0x500AA0 (career +0x713C = 0) holds, else a
  random shop scene of the table. Its origin (type 4, 0x43B000 → 0x42FC60
  → 0x431990) is the start marker: 0x86501C is never written, so the
  marker named `ForwardStartLine` (`BackwardStartLine` backward;
  `ForwardFinishLine` / `BackwardFinishLine` with 0x89E7B4 set), its
  heading + 0x4000 there and − 0x4000 in 0x431990. In free roam that
  marker is at (1698.2, −884.2) heading 259.8°; the scene's car (two equal
  keys of `ZPM_Car1_t` / `_q`: (0, −2.95, 0), turned 89.2°) is then exactly
  ForwardStartingPosition's slot 0 (1695.2, −883.6, heading 349.8°), so the
  car stands where the city start put it. The camera is the generic
  camera group keyed 0x346D604C, "Track 0": six type-0x12 entries with
  world-space (space 1) eye and look paths — the "Welcome to Bayview" sign,
  the terminal, the view through its windows, the car. Its event track
  sends ANIM_POSITION (0.14 at once at frame 43, then 1.0 over 1800 frames
  at 44) to 0xA167F030; no world-animation instance of L4RA has that key
  (+0x20 / +0x24), so it is left out (**open**: what it addresses). The
  end (0x43BB10, type 4): 0x43B7A0, the player input's vtable +0x1C(0),
  and 0x532E20 (0x532D90 with BinHash("DDAY_EVENT_B"): the GPS to B's goal
  while B is the intro's event). The D-Day script begins only after it
  (the clone steps the script only with no scene running). The original
  under Wine (2026-10-29, `capture.py` with frames every 3 s) shows the
  same shots, no HUD during the scene, then the chase view with the phone
  prompt and the SMS; the clone matches (`--shot intro_nis`,
  NFSU2_SHOT_MS into the scene). 0x484AF4 (in the speech code, on the
  speech object 0x82BA30) compares a scene hash with CareerNis1's and
  plays speech event 9, then 0x5258C0 on Rachel's hints (0x8635BC): that
  is Rachel's call over the scene. **Corrected 2026-10-29**: the speech
  is in the install, inside `SDATA/sdat.viv` (see **Speech and music
  archive**); 0x484510 sets NIS_Select_End = (0x400, 0x4000, 1)
  (0x45A720) and queues NIS stream kind 9 (0x46CA60); that tuple picks
  record 0 of `nisaudio` entry 0x4024 (keys 10, 14, 0), a 13.24 s
  stereo EA-XA line at 0x40AD00 in `speech/nisaudio.big`. Clone:
  `Game::start_career_scene` plays it (`Streams::play_career_nis`).
* **The airport's invisible wall** (2026-10-29): driving straight ahead
  from the start the car stops dead about 20 m on, by the planter: a
  collision face of pack 240 (face 1412, barrier group 0, wall flag)
  spanning (1716.6, −892.3)..(1724.1, −850.1) from z −2048 to 2048. The
  original under Wine stops at the same place (0–1 mph, wheels spinning),
  so it is native; the way out is the GPS's, to the left. Steering and
  reverse work there in the clone. `--shot intro_route` drives the intro
  from the start to the car lot with the keys alone, on the GPS route
  (about 3.7 km, ~7 min of game time), and fails if it does not get the
  car lot's engage dialog; `INTRO_KEYS="up:2,down+left:1.5"` on the
  `intro` shot holds keys and logs the car.
* **Rest test** (**verified** 2026-10-29; `CarState::rest_class`):
  0x5ABB10 (each sub-step after the Euler step, the drive model's mover,
  from 0x5B04E0; **corrected** round 11: 0x5AB0D0 is the MeloMvr's)
  runs Car_RestHold 0x5A5E30(car, body, timer, &class, mover)
  on Car_RestClassify 0x5992C0(car, body, last class): 0 when a
  hydraulics bounce input (controls +0x1EC…+0x1F8, 0x5ED2A0) is on or a
  hydraulics corner state (mover +0x74 + 4i, 0x5ED480, type-1 movers) is 2
  or 3 (see "Hydraulics"); else,
  with (gas controls +0x20C ≤ 0.3, or gear +0x434 = 1 neutral, or wheels
  down +0x424 < 1, or +0x428 < 3), |v| (body +0xA0) ≤ 0.7, |ω| (+0xC0)
  ≤ 0.7, (|body +0x38| ≤ 0.15 and |+0x48| ≤ 0.15, the forward and side
  axes' z) or no wheel down, not recently hit (0x5EBFB0: car +0x974 tick
  within +0x978 s, the ghost timer), and a wheel down or a track contact
  with |n.z| > 0.7 (0x5ED230 → 0x5A01C0: either side a track polygon,
  class 3, whose |+0x60 → +0x78| is over 0x7844D8 = 0.7 — a floor, not a
  wall, **verified**) or class 2 last time:
  2 when |v| and |ω| ≤ 0.25, else 1. Class 1 scales momenta +0x70 / +0x80
  by 0.95, class 2 zeroes them (and, with a wheel mover, its wheel spins
  +0x28 when wheels are down), both body +0x160 (gravity) to 0 for 2,
  else 9.8 (0x7FBEF0; 0x7FC084 in mode 1); then 0x5A0D90 again; class 2
  also sets body +0x12C = 1.0 (0x5AB126, the MeloMvr's 0x5AB0D0, which
  passes no wheel mover). The drive mover's call (0x5ABB2E) passes
  itself, so its wheel spins go × 0.95 / to 0 with the momenta (ported
  round 11; before, the clone left them). **Open**: +0x428 (left out: taken as ≥ 3), the
  wheel states (taken as never 2/3), body +0x12C, the 1 s at-rest timer
  (0x5EBF70 after 4000 ticks with a wheel mover).
* **Driving vs the original, round 2** (2026-10-31, physics2; private
  Wine copy $OB_PRIVATE/wine/runs/<run> on :163, `donut.py` /
  `brake.py` / `steer.py` / `slip.py` there; clone `INTRO_POSE`,
  `INTRO_KEYS`, `INTRO_WHEELS`, 25 Hz):
  * **Input** (**observed**): in this Wine copy the Left arrow does not
    steer (the front wheel frame +0xB0/+0xB4 does not move), A does; Right
    and D steer right. Left-hand tests use A. Cause (round 3): see below.
  * **Donut** (full lock left + full gas from rest in the airport bay,
    (1692.30, −879.92), heading −11°): the original also donuts. It grips
    for ~2.2 s (to 9.4 m/s), breaks loose, then spins its rear wheels at
    ~66 rad/s (~7900 rpm, 1st gear, gear +0x434 = 2) and keeps rotating
    after the steering is released (8.2–10 s: 89°/s at 2.5 m/s; the clone
    81°/s at 2.3 m/s). So the sustained donut is native. Mean yaw rate /
    speed per window, original / clone after the fix: 0–1 s 34 / 40°/s;
    1–2 s 112 / 120°/s, 7.8 / 8.0 m/s (before: 101°/s, 6.6 m/s); 2–4 s
    161 / 151°/s; 4–6 s 104 / 121°/s; 6–8 s 141 / 149°/s.
  * **Grip turn** (same inputs from (1693.73, −859.06), −55°): speed at
    1.6 / 2.0 s original 8.03 / 9.73 m/s, clone before 7.15 / 8.07, after
    8.68 / 9.90 (the rolling-constraint fix above).
  * **Slip** (**observed**): +0x118 and +0x16C hold the same value; a
    rolling driven wheel keeps about −0.1 m/s of slip under full gas in a
    straight line (0x59D360 takes +0x118 / (r · dt 0x7FC800) out of the
    spin rate, after 0x5A12E0 has moved it).
  * **Braking** (full gas to 25 m/s from (1720.31, −831.95), 78.8°, then
    the brake key): original 25.19 → 0 m/s in 1.76 s over 21.97 m; clone
    25.13 → 0.2 m/s in 1.76 s over 20.6 m.
  * **Steering at speed** (gas to 20 m/s from (1732.64, −773.30), 77.9°,
    then Right + gas): yaw ~60°/s in both before both cars glance off the
    same wall at ~(1742, −740) (22.3 → 19.1 m/s / 22.8 → 19.0 m/s).
  * **Open**: the 240SX, COROLLA and an SUV (needs the car lot in the
    original), slides at speed, airborne and landing, the AI cars.
* **Driving vs the original, round 3** (2026-10-31, physics3; Wine copy
  $OB_PRIVATE/wine/runs/<run> on :163, Quick Race → Free Run (its car
  select lists 206, Focus, Corolla, 240SX, 106, Corsa, Escalade, 350Z,
  GTO, Evo; Free Run starts at (1695.17, −883.59), −10.2°); scripts there:
  `qrboot.py` (boot to the main menu, then serve peek), `turnto.py`,
  `donut.py` (key steps, logs every wheel), `inlog.py` / `inlog2.py` (the
  human input object, found as the object whose +0x2A8 is the car,
  vtable 0x784FC8). Clone: `--shot intro` with `INTRO_CAR=<name>` (Quick
  Race's Free Run in that stock car), `INTRO_POSE`, `INTRO_KEYS` (now with
  the running time), `INTRO_LOG_HZ`, `INTRO_HZ=25`; the original's key
  timeline is replayed from its log):
  * **Left arrow** (**verified**): the default map 0x7FCAC0 binds steer
    left (action 3) to Left (DIK 0xCB) and A, as the clone does. In the
    Wine prefix, Wine's saved DirectInput action map for this format
    (`HKCU\Software\Wine\DirectInput\Mappings\<user>\Keyboard\{F19DD539-CB04-4126-908B-CF28FEFC69BD}`,
    the format at 0x86FA00, genre 0x1000000) had semantic 0x810004CB
    stored as object 0x2404 (J): J steered left (the action bytes at
    0x865658 + 0x1C8 × player, axis 0x8F: Left 128, A 0, J 0). Wine
    applies a saved map in BuildActionMap; the bad entry came from an
    earlier rebinding in that prefix and was copied into most agent
    prefixes (ai109, car109, drag109, …; menus100/103 are clean). It
    keeps itself: Wine writes the map back on every SetActionMap, so a live
    `wine reg add` is lost when the game exits. Fixed in a private Wine copy by
    editing `prefix/user.reg` (`"810004cb"=dword:0000cb04`) with Wine
    stopped. Not native.
  * **Donut** (full gas + A from rest at the Free Run start, mean yaw rate
    / speed per second, original / clone): 240SX 27/27, 80/87, 120/118,
    147/151, 154/148, 155/166, 133/133, 154/181, 127/117 °/s, speed within
    0.4 m/s; Corolla 27/28, 74/76, 109/109, 109/116, 109/117, 125/135,
    143/137, 101/114, 79/86; Escalade 29/30, 60/61, 84/85, 89/87, 85/84,
    86/95, 108/120, 90/114, 55/62.
  * **240SX straight** (from (1711.45, −825.30), 77.8°: 6 s gas, 3 s
    coast, brake): speed at 2/4/6 s 7.6/16.1/21.8 vs 8.0/16.3/22.1 m/s;
    coast 21.8 → 16.0 m/s over 3 s in both (1.9 m/s²); brake 16 → 0 in
    about 1.2 s in both. Handbrake alone (Space) from 13 m/s: 13.0 →
    3.9 m/s in 1.75 s (5.2 m/s²) vs 13.2 → 4.2; both stay straight.
  * **240SX slalom** (14 m/s, A 0.6 s, Right 0.56 s): with the input mean
    the front wheel angle follows the original's (23, 27, 29, 30, 32,
    33° vs 23, 28, 30, 31, 32, 34°, 0.08 s apart; before, 34° at once)
    and the yaw builds over ~0.4 s in both. The clone still turns ~8° more
    to the left before the reversal takes (−57° vs −66°): its steering
    reverses one frame later in the replay, and its rear axle tips past
    the friction circle (both rear wheels sliding) where the original's
    stays at 0.96 of it. **Open** (marginal).
  * **240SX wall glance** (14.4 m/s into the east wall of the airport
    straight at ~45°): speed 14.4 → 9.4 vs 14.6 → 9.0 m/s at the hit. The
    original then slid along the wall, turned back to the wall's heading
    (33° → 77° in 2 s) and sped up to 11.7 m/s; the clone turned into the
    wall (heading −90°) and stopped at 1.5 m/s, its rear wheels spinning
    at 9 m/s of slip on the engine's speed. Cause: the wall drive above
    (the original's front wheels got 35% of the torque). Now: 30° → 79°,
    9.0 → 11.2 m/s, path within 0.5 m.
  * **Corolla against a kerb** (from (1703.75, −844.79), 79°): both stop
    against the same planter (1703.5, −841.1) within 1 s, same yaw. Then
    reversing (Down) from rest 8 s later: original 2.05 m/s after 0.24 s
    (all four wheels driven, front −1.12 kN·m each: the wall drive), then
    2 m/s²; clone 0.96 m/s after 0.4 s (rear only, wheelspin): its wall
    records died 0.3 s after the last impulse while the original's +0x120
    was still above 0 after 8 s at rest. **Corrected in round 4**: the
    original's record dies at rest too; a new one is made as the reverse
    starts (see round 4).
  * **Engine on a wall** (240SX, full gas in 2nd against the wall,
    original): rpm 5950 → 4820 in 0.45 s with the rear wheels sliding at
    5–8 m/s of slip; the clone's engine held ~6000 rpm before the wall
    drive (the sliding force balanced the drive torque); with it the
    front takes torque and the car leaves the wall as the original does.
  * **Not done** (**open**): nitrous (Quick Race's stock cars have none),
    airborne and landing, traffic hits, the assists' on/off cases (both
    runs had Stability Control off, car +0x464 = 1), AI racers' physics.
* **Driving vs the original, round 4** (2026-10-31, physics4; same Wine
  copy and Free Run; `klog.py` / `rlog.py` in <work dir> log the
  car with its collision body's +0x120 / +0x124, the record count
  0x86B24C and the first records of the list [0x890080] + 8):
  * **Logged position** (**verified**, live + REA): car +0x60 is the
    centre of mass, +0x50 the body origin: at rest +0x60 − +0x50 =
    rot · (0.25, 0, 0.24) for the Corolla, its centre-of-mass offset
    (handling +0x110). The placement 0x5F5020 writes +0x50 = the point
    less the unrotated offset (0x5F5072–0x5F5095), as the clone's
    `CarState::placed` does. The `intro` shot's `INTRO_POSE` now puts the
    clone's centre of mass on the given point and its log prints the
    centre of mass, so both logs give the same point. Earlier rounds
    compared the original's centre of mass with the clone's origin (and
    placed the clone's centre of mass off by (rot − 1) · offset): their
    paths carry up to ~0.3 m of that.
  * **Contact records** (**verified**): a record (constructor 0x5A9440,
    list [0x890080] + 8, record +0x64 the pair) lives while
    0x592BE0 holds: (clock 0x865188 − record +0xC) × 0.00025 < 0.3 s
    (0x7FBEE0), +0xC being the last push (0x5A9630, called only from the
    impulse 0x5AF0F0 at 0x5AF78A; the constructor sets +8 and +0xC). The
    collision step 0x5FFA60 (0x5FFD50–0x5FFD7C) drops the expired ones
    through 0x5B2280 → 0x5AE480 (the pair stays, only its record +0x244
    goes; 0x5A9580 takes 1 off the +0x120 / +0x124 count with the same
    0x5A0170 test as the constructor). Car_RestHold 0x5A5E30 clears the
    collision body's +0x19 at rest class 2 (0x5A5F44; classes 0 and 1 set
    it) and 0x5FFA60 sweeps only bodies with +0x12 and +0x19 set
    (0x5FFB00), but every pair still runs 0x5B5060; a pair's hull
    distance is computed again only when it is dirty (+0x220): set when
    its shapes change (0x5985A0) or when either body's position has moved
    by more than 0.001 in a component since the last time (0x598630,
    orientation not compared); otherwise its last distance and touch flag
    +0x14 stay (0x5B3D30). **Ported in round 5** (`narrow::distance_at`):
    a turn on the spot under 1 mm keeps the old answer, as in the
    original.
  * **Corolla against a wall, then reverse** (live, two runs). Gas held
    5 s against the wall north of the airport bay ((1700.61, −843.74),
    77.8°): one record, its summed push |J| (record +0x60) 89 after 5 s,
    the front wheels driven (wall drive). Keys off: it dies 0.3 s after
    the last push in the original too (record count 0x86B24C 0 from
    then; the first run's 8 s claim above was wrong). Down 3 s later: between
    the last sample at rest (gear still 1st) and the first moving one
    (0.15 s later, 1.72 m/s, reverse) a new record is made with one push of |J| = 0.555 (= 0.3 m/s × 1.85 t, the
    impulse floor) along the wall's normal (−0.16, −0.99, 0.018) — the
    same 0.555 in both runs, on a polygon body different from the push's;
    then the wall drive puts −0.83 / −1.55 kN·m on the front / rear
    wheels until the record dies 0.3 s later: 0 → 1.72 m/s in 0.15 s,
    2.55 m/s at 0.45 s. The clone (same pose and keys) makes no contact
    at the reverse start: 0.10 → 1.39 m/s in 0.5 s, rear wheels only. The
    push phase matches (heading 77.8° → 80.3° / 79.7°). **Open**: why
    the woken car meets the wall approaching (v·n < 0) in its first
    frames; candidates are gravity coming back after the rest class
    (n.z = +0.018 makes a drop an approach) and the body's pitch under
    the reverse torque.
  * **Full gas on the airport straight** (from rest; original / clone
    with the same pose and key timeline): Corolla from (1720.01,
    −842.67), 77.6°: 1.68 / 1.80 m/s at 1 s, 6.81 / 6.93 at 3 s, 12.98 /
    13.12 at 5 s, 19.18 / 18.93 at 7 s, 20.98 / 21.04 at 8 s; 1→2 at
    ~7.05 s (6757 rpm at 19.1 m/s) / 7.08 s. 350Z from (1721.21,
    −835.95), 77.6°: 2.01 / 1.80 at 1 s, 8.77 / 9.03 at 2 s, 17.10 /
    17.36 at 3 s, 25.46 / 25.48 at 5 s, 31.27 / 31.43 at 7 s, 37.33 /
    37.44 at 9 s; 1→2 at ~3.37 s (6859 rpm, 18.1 m/s) / 3.28 s, 2→3 at
    ~6.03 s (6591 rpm, 28.8 m/s) / 6.08 s. Both glance the same wall
    near 6–7 s. **Observed**: under full gas the original's heading
    creeps ~1° left over the first 3 s (77.6° → 76.8°, both cars); the
    clone holds 77.6° (**open**).
  * **Stability Control on** (**verified**, REA): car +0x464 is read at
    only three places: the rear-wheel factor 0x59D4A0 through 0x59D68F
    and 0x5A84BD (above, ported) and 0x5A33F0 in 0x5A3390, which over
    30 mph (0x784478) with it set multiplies the value stored at +0x140
    by 0x7A1998 = 1.0 — no effect. So "on" only drops the 0.85 rear
    factor; the clone takes the option for the player and every AI
    racer. Not measured live.
  * **Not done** (**open**): nitrous, airborne and landing, traffic hits,
    AI racers' physics, top speed (the airport straight ends near 45
    m/s), gear changes of the remaining cars, the 240SX slalom margin,
    drift races.
* **Driving vs the original, round 5** (2026-10-31, physics5; same Wine
  copy and Free Run; scripts in <work dir>: `olog.py` (pose, gear,
  wheel torques, records, raw car/body/wheel dumps), `fast.py` (one line
  per game frame: heading, car +0x110 = ω in the world, the first record's
  |J| sum +0x60 and points +0x20 / +0x30), `dlog.py` (drivetrain state
  +0x44, clutch +0x14, input +0x4C, engine +0x14, throttle / power,
  rigid body [mover +0x20] +0x150 / +0x90); clone `run.sh` = the `intro`
  shot with `INTRO_CAR` / `INTRO_POSE` / `INTRO_KEYS`):
  * **Frame rate** (**observed**): in these sessions the original under
    Wine steps 20 frames a second (the clock 0x865188 moves 200 ticks a
    frame), not 25 as rounds 2–4 assumed; the comparisons below use
    `INTRO_HZ=20`. Collisions run once a frame, so wall contacts depend on
    it (the 23° wall hit below differs by 3° of heading between 20 and
    25 Hz in the clone).
  * **Coasting to a stop** (fixed; see **Car modes**): Corolla, 1.6 s of
    gas then keys off on the airport straight heading south. Original:
    clutch locked down to the idle input speed (2.2 m/s in 1st), then the
    anti-stall opens it (state 0, clutch 0.78 → 0.18 in one frame, state 1
    held while the input stays under idle) and the car stops on the rest
    damping (D climbing 0.1/s): 4.73 m/s at 2 s, 2.21 at 3.5 s, 1.01 at
    4.5 s, 0 at 4.9 s. The clone kept the rest damping off above
    0.15 m/s, so the car sat at the idle input speed: the slipping clutch
    pushed it back over idle each time the anti-stall began to open it
    (2.25 m/s forever). Now 4.79 / 2.18 / 1.01 / 0 at the same times, the
    stop within 0.15 m. Northward (from 9.1 m/s): original 7.21 / 5.34 /
    3.53 / 1.95 / 0.74 m/s at 4.5 / 5.5 / 6.5 / 7.5 / 8.5 s, clone 7.27 /
    5.45 / 3.71 / 2.07 / 0.88, stopped at 9 s in both.
  * **Contact pairs** (ported): the 1 mm dirty test, as round 4 traced it
    (0x598630 with 0x43D190, eps 0x3A83126F = 0.001, ≤ in every component;
    position only), on every query of the step: detection 0x5B51BB, time
    of impact 0x5B4CED, push-out 0x5B4635; the bodies' positions are the
    car's centre of mass (the collision body's box +0x20..+0x2C is centred
    on car +0x60, live) and the polygon's fixed point. Props against the
    track too; prop–car pairs still measure every frame (**open**).
  * **Heading drift under gas** (**observed**, not a model difference):
    on the airport straight the original's heading creeps toward the east
    side whichever way it drives (north: 79.50° → 79.23° over 6.5 s of
    gas; south: −102.36° → −101.89° over 3.5 s), and also while braking;
    from (1722.40, −832.69) only 0.27°, where round 4's start gave 0.8°.
    The wheels' world forces (+0x130) are symmetric to 1 N and the ground
    normals tilt about 0.003 sideways there; the yaw needs under 1 N·m. The
    clone holds its heading to 0.05°. **Open** (below the resolution of the
    logged forces; most likely the road faces under each wheel).
  * **Reversing away from a wall** (**open**, narrowed): in the original
    the reverse-start push (|J| 0.555, the impulse floor) is made at the
    front corner's upper hull vertex (record +0x20 z 24.35, 0.43 m over the
    centre of mass, round 4) on the wall face itself — round 4's normal
    (−0.16, −0.99, 0.018) is the push's sum +0x50, friction included, not
    a leaning face; the clone has only the two vertical barrier faces
    (pack 240 faces 1417 / 1418) there. Reverse torque pitches the nose
    down, which moves that upper vertex forward. The clone comes to rest
    13 mm from face 1418 and its nose-down pitch closes 0.1 mm of that, so
    it makes no contact; the original's rest gap to that face is not known
    (another run rested 6.2 mm from its wall, pair +0x10).
  * **Hitting a wall at 20–25°** (**open**): Corolla into the east wall of
    the airport straight. At 10.3 m/s (from (1743.11, −693.52), 59.4°)
    original / clone heading 59.7 / 61.9° one frame after, 67.7 / 70.7° at
    0.25 s, 74.1 / 77.4° at 0.65 s; speed along the wall 10.3 / 10.7 m/s
    at 0.65 s, 11.5 / 12.8 at 1.4 s. At 3.9 m/s (from (1737.64, −758.96),
    57.3°) the original stays wedged at 64–68° crawling at 1.8 m/s (engine
    at idle, all four tyres on their friction circles pushing into the
    wall, +0x170 = 1, lateral −5.6 / −2.2 / −0.7 / −3.1 kN) while the clone
    turns along the wall (79°) and drives off at 4–5 m/s. Found: the
    clone's first push lands on the front corner's upper hull vertex (arm
    z 0.47; the car is pitched 1.5° nose down, which puts that vertex 8 mm
    nearer the wall) and kicks a roll of −0.92 rad/s (original −0.53, its
    first record point at the lower vertex, z +0.14, and a larger first
    |J|: 7.4 against 4.0 + 0.8); the clone then rocks on two diagonal
    wheels for 0.4 s and its yaw grows to 0.79 rad/s (original peaks at
    0.63 and decays). Which vertex the original's distance query returns
    for this nearly vertical edge is not traced.
  * **240SX full gas** (from rest at (1709.34, −847.79), 78.3°, 11.5 s,
    up the airport straight and the road beyond; `dlog.py`): original /
    clone speed at 1 / 2 / 3 / 5 / 7 / 9 / 11 s 2.59 / 2.91, 6.40 / 6.77,
    11.05 / 11.52, 17.87 / 18.08, 23.78 / 23.97, 27.92 / 28.00, 31.86 /
    31.90 m/s. The launch: clutch state 2 from the first gas frame, +0.15
    a frame (both), locked after 7 frames in both; the clone gains about
    0.02 m/s a frame more while it slips (input shaft 72.6 against 71.1
    rad/s at the sixth frame), so it leads by 0.14 m/s at the lock, and
    1st gear then pulls about 2% harder (+3.86 against +3.81 m/s from 1 to
    2 s). Shifts: 1→2 asked at 3.99 s with the engine at 6384 rpm a frame
    before (15.5 m/s), clone at about 3.88 s, 6304 rpm (15.3 m/s); 2→3 at
    8.44 s, 6432 rpm (27.2 m/s), clone about 8.27 s, 6275 rpm (26.7 m/s).
    The up-shift point is redline − 100 = 6400 plus the random −100..0
    of 0x594720, tested at the start of a frame (0x5ABC00), so the engine
    can pass it by one frame's climb (about 30 rpm here): both cars'
    shifts lie in that native range.
  * **Not done** (**open**): nitrous, airborne and landing, traffic hits,
    AI racers' physics, top speed (the 240SX was still pulling 0.95 m/s²
    at 31.9 m/s in 4th), more cars' shift points, the 240SX slalom
    margin, drift races.
* **Driving vs the original, round 6** (2026-10-31, physics6; same Wine
  copy and Free Run at 20 frames a second; `flog.py` in <work dir>
  logs every game frame: centre of mass, heading, v, vz, car +0x110 (ω,
  world), the rotation rows and every contact record (made, last push,
  point +0x30, |J| sum +0x60)):
  * **Collision rate in the comparisons** (**found**): the clone advances
    every caller by 1/60 s frames (`drive::STEP`), each running its own
    collision step, so `INTRO_HZ=20` still met walls three times per
    original frame; rounds 3–5 compared 60 Hz collisions with the
    original's 20. `INTRO_STEP_HZ=20` now makes the shot's frames 1/20 s
    (`game.rs` `step_len`), one collision step per frame as 0x5FFA60 runs.
  * **Impulse inverse inertia** (**verified**, ported; see Contacts →
    Impulse): body space, applied to world vectors unturned. Corolla into
    the airport straight's east wall at 3.9 m/s (pose of round 5): the
    original's first frame sums |J| 2.519 (one push at the front right
    corner's upper hull vertex, record +0x30 (1740.931, −755.195, 24.306))
    and the clone now 1.968 + 0.557 = 2.525 at the same vertex
    ((1740.931, −755.198, 24.306)); with the world-turned inertia it was
    1.652 + 0.557. The vertex choice matched already (the round-5 note that
    the original takes the lower vertex there came from a later push).
  * **Wall hits at 20 Hz** (original / clone, heading ° and speed m/s):
    3.9 m/s at 22°: 2.2 s 60.6 / 60.6, 3.04 / 2.98; 2.8 s 63.7 / 65.2,
    1.83 / 2.18; 3.4 s 65.1 / 66.8, 1.85 / 1.82; 4.4 s 67.1 / 69.0, 1.89 /
    2.13 — both now stay wedged crawling along the wall (round 5: the clone
    turned to 79° and drove off at 4–5 m/s). 7.5 m/s at 20° (owall2): 4 s
    70.3 / 71.5, 6.86 / 7.19; 5 s 76.4 / 76.7, 7.70 / 8.02; 6 s 79.1 /
    79.4, 10.05 / 10.07 (before: 73.3 / 79.3, 7.47 / 9.71 / 12.22).
    10 m/s at 23° (ofast): heading 64.2 / 66.4 at 4.2 s, 72.7 / 78.1 at
    4.6 s; speed 9.4 / 9.0 at 4.2 s but 9.4 / 11.1 at 4.6 s and 10.7 /
    14.5 at 5.4 s. **Open**: in the clone the first push (upper vertex,
    roll −0.82 rad/s) tips the car onto its outer wheels — its centre of
    mass rises 0.4 m within 0.3 s while the driven wheels spin up in the
    air (5300 rpm), and it lands at 4.5 s and jumps 9.2 → 12.2 m/s.
  * **16.6 m/s at 21°** (new; from rest at (1713.737, −807.874), 58.76°,
    0.5 s idle then 7 s gas): the original's first frame sums |J| 11.58
    with its last push at the lower vertex (z +0.155 over the centre of
    mass), ω (−0.35, −0.92, 0.62) after it and (0.08, 0.10, 0.82) a frame
    later; its centre of mass stays within 8 mm of 23.863 throughout;
    heading 59.6 / 61.6 / 66.7 / 71.1 / 75.3 / 77.9 / 79.5° at 6.05 /
    6.10 / 6.20 / 6.30 / 6.50 / 6.85 / 7.30 s, speed 15.4 → 15.2 → 18.8
    m/s along the wall, then about 1.1 of |J| a frame while it scrapes on.
    The clone: 7.57 at the upper vertex plus a 0.557 push at the same
    vertex 1 ms later, roll −1.9 rad/s, the centre of mass up 0.46 m by
    6.2 s, a roof vertex (33) on the wall at 6.3 s, and it leaves the wall
    at 84° (**open**). Before the hit the clone also accelerates faster
    from this start (13.84 against 13.29 m/s at 5 s; the start is on a
    different surface, z 24.05).
  * **Support functions** (**verified**, not ported — no difference found
    on the convex `_CV` hulls): the hull's support 0x5AFBB0 climbs from
    the last support vertex (pair cache; vertex 0 on a cold start) through
    the vertex links of the `_CV` volume (vertex record +8 link count,
    +0xA first link into the `0x39202` list, +0xC index, +0xE visit stamp
    from 0x43C1C0 + 0x43BDF0, +0x20 the cached dot): a vertex none of whose
    links is strictly greater wins; for hulls of over 16 points the first
    vertex's greater links are queued (0x86B2D8, never written, stops
    queueing after it); if none of them is a local maximum, a scan takes
    the first of the greatest. The reported index is the record's +0xC
    field, which is 0 for the points with no links (COROLLA_CV: 10 of 46,
    e.g. (0.266, ±0.824, 0.412)), so the pair cache then refetches vertex
    0. The polygon support 0x5A2070 is the first of the greatest over the
    face's corners. The pair's closest points +0xC0 / +0xD0 that 0x594010
    averages are left by the last query that found a candidate: an
    overlapping query (0x59811A) writes none.
  * **Reversing away from a wall** (**open**, unchanged): at 20 Hz the
    clone still reverses from the kerb pose of round 4 without the
    original's reverse-start push: 0.50 / 1.31 m/s at 0.15 / 0.45 s
    against 1.72 / 2.55.
  * **Not done** (**open**): nitrous, airborne and landing, traffic hits,
    AI racers' physics, top speed per car, the 240SX slalom, drift races.
* **Driving vs the original, round 7** (2026-10-31, physics7; Wine copy
  a private Wine copy on :163, Free Run, Corolla, 20 frames a second; these runs
  were taken with ThirteenAG's WidescreenFix still loaded (game/dinput8.dll,
  renamed to dinput8.dll.widescreenfix-off afterwards): none of its 65
  patched sites lies in the physics —
  the nearest are 0x5BE858..0x5D2B75, the options/post-FX/present code.
  `flog2.py` in <work dir> reads the clock, the car, the head
  record and its pair's count (+0x240) in one request and drops samples
  whose clock moved during it):
  * **Torn samples in rounds 5–6** (**found**): `flog.py` read the record
    in a second request, after the next frame's collisions, so each "first
    frame" summed two frames. The 16.6 m/s run's 11.58 was frame 1 + 2: the
    original's first frame is two pushes (pair +0x240 = 2), 7.88 in all,
    the last at the upper front-right vertex (+0.42 m over the centre of
    mass); clone 7.15 + 0.86 = 8.01 at the same vertex. The lower-vertex
    push is frame 2's.
  * **Wall hit at 15 m/s, 21°** (from (1717.313, −801.109), 57.82°, 1 s
    idle then gas; original / clone frame after the hit): V (4.415, 13.155,
    0.112) / (4.442, 13.233, 0.108); ω (−0.29, −0.78, 0.56) / (−0.18, −0.57,
    0.55); up (−0.015, 0.036) / (−0.017, 0.036). Frame 2: pushes 2.71 /
    2.28, ω (0.08, 0.11, 0.72) / (0.08, 0.17, 0.68); then the yaw rate
    0.77 / 0.74, 0.81 / 0.81, 0.79 / 0.81, 0.71 / 0.70, 0.51 / 0.51, 0.40 /
    0.39; two pushes a frame in both while scraping. Heading / speed after
    the hit at +0.1, 0.3, 0.5, 1.0, 1.5 s: 62.9 / 63.1°, 71.2 / 71.4°,
    74.9 / 74.7°, 78.1 / 78.1°, 79.5 / 79.5°; 13.77 / 13.81, 13.95 /
    13.64, 14.09 / 13.78, 15.08 / 14.74, 16.58 / 16.18 m/s; the centre of
    mass stays within 6 mm of 23.866 in both (before the fix: vz 0.51 m/s
    after frame 1, 0.46 m up and onto the outer wheels). Cause: the
    suspension refresh above.
  * Round 6's runs again (clone now / before; original): 10 m/s at 23°
    heading 66.0 / 66.4 (64.2) at 4.2 s, 73.0 / 78.1 (72.7) at 4.6 s; speed
    9.18 / 9.0 (9.4), 9.12 / 11.1 (9.4), 10.46 / 14.5 (10.7) at 4.2 / 4.6 /
    5.4 s. 7.5 m/s at 20°: 70.2 / 76.3 / 79.2° (70.3 / 76.4 / 79.1) and
    6.84 / 7.64 / 9.53 m/s (6.86 / 7.70 / 10.05) at 4 / 5 / 6 s.
  * The contact loop (**verified**): one 0x5AF0F0 call is one impulse —
    its loop repeats only while the bodies' vt+0x4C scale is below 1, and
    both the car body and the track polygons return 1.0 (0x5AA070); the
    denominator and impulse are as ported (cross product 0x43D1E0 = a × b,
    matrix 0x441000 on the diagonal body-space 0.4·I⁻¹; live rigid body
    +0x170 = (2.8055, 0.6374, 0.61), +0x1B0 with the ground yaw 0.2675,
    +0x158 = 1); the polygon's 1/m 0 (0x5B7690), matrix 0x86B450 zero,
    restitution 0.001 (0x5AA060; the car's wins). Pair +0x240 is zeroed
    for every pair at the step's start (0x5FFB5C) and counts its
    resolutions; at most 2 (0x802084).
  * **Still open**: reversing away from the wall (0.51 / 1.32 m/s at 0.15 /
    0.45 s against 1.72 / 2.55, unchanged — the rest gap at the planter);
    nitrous, jumps and landing, traffic hits, AI racers' physics, top speed
    per car, the 240SX slalom, drift-race physics (not reached this round).
* **Driving vs the original** (2026-11-04, after the physics merge;
  internal notes, evidence in the maintainer's private captures): 240SX
  and 206 launch, 38 m/s to ~99 mph, slalom at 60 mph, 2 m drop, brake from
  60 mph and a 6.6 mph wall bump, replayed frame by frame at ~120, 31–90 and
  20 fps. **One world update per frame** (**verified**, ported): the main
  loop 0x5811D0 calls 0x609CF0 once with the frame time 0x865150, so the
  player's steering 0x416BB0 and its running means 0x576AB0 run once a
  frame; the clone split frames over 16.7 ms into 1/60 s world steps and
  steered ~10 % slower there (240SX slalom heading +5.2° at 2 s, now
  +1.2°). Open: the slide state +0x160 (0x5A2F41 / 0x59D3D4) in long
  slides, clutch engagement at extreme slip after a teleport, the 0.05 s
  frame-time cap.
* **Driving vs the original, round 15** (2026-10-31, physics15; Wine copy
  a private Wine copy on :203 through `fastgpu.sh`; rounds 13/14's cave logger
  and replay, tools in <work dir>: `lg.py`, `rep.py`, `cl.sh`,
  `cmp.py`, `cmpw.py`, `summ.py` (per-segment heading / speed differences
  at 0.5 / 1 / 2 / 4 / 8 s, against round 14's replays), `hopaddr.py` (now
  also logs mover +0x60, the rest class), `tl.py` (the logger plus a
  traffic car C2 and a `hit2` placement), `t2h.py` (a `tl.py` log in
  the maintainer's probe `hit_replay` rows, with the player's gear, clutch state,
  engine and input shaft), `walk.py` (the mover's component list);
  logs `h1` Corolla handbrake turns, `n1` / `n2` 206 nitrous (career
  save), `t1` 206 into a traffic Panel Van; round 14's `c2`, `x2`, `f1`,
  `x1`, `z1` and round 13's `d1` replayed again):
  * **Suspension rate over the sub-step that moved the car** (**verified**,
    ported; `CarState::last_sub`): the mover's sub-step (0x5AA490, the
    Euler step) ends with the components' updates (0x5B04E0 → vt+0x1C in
    list order), so 0x5A87A0 measures each suspension at the sub-step's
    *end* and divides the change by that sub-step's length (0x7FC800).
    The clone measured at the next sub-step's start and divided by the
    sub-step about to run; the two differ when a frame's sub-step count
    changes (ftol(dt / 0.0044) + 1 at 0x5B7354: a 1/120 s frame takes two
    of 0.00417 s, a 0.00889 s frame three of 0.00296 s). On a road seam
    the clone's damper rate came out 0.309 instead of 0.2225 m/s, the
    rear load dipped to 1.50 instead of 2.05 kN, and the Corolla's left
    rear wheel locked under the handbrake (the lock test 0x59D410 is
    marginal there: brakes 4.69 against r · road + 0.1 · |spin| ≈ 0.6 +
    4.05). A measurement outside the sub-step loop keeps the rate (0x7FC800
    = 0 at 0x5ABBDF, 0x5B62BB, 0x5B6DE5, 0x5B7098): the clone's
    `refresh_suspension` and the replay's sync now set `last_sub` = 0.
  * **Anti-roll partner order** (**verified**, ported): 0x5A87A0's
    anti-roll term reads the other side's suspension (+0x18) +0x1C as it
    stands; the mover's list (read live, `walk.py`: rigid body, then W0,
    S0, W1, S1, W2, S2, W3, S3, engine, drivetrain; S0 ↔ S1, S2 ↔ S3) runs
    the left wheels before the right, so a left wheel sees the right one's
    compression of the previous sub-step and a right wheel the left one's
    new one. The clone used the new value for both.
  * **Handbrake, Corolla `c2`** (round 14's open item): the second rear
    wheel now keeps rolling as in the original (both lock only the right
    rear); heading +0.04° at 0.36 s, +0.45° at 1.0 s, at most +1.6° (2.3 s;
    round 14: −0.5° at 0.4 s, −7° at 2.1 s); synced at 2537 / 2545 the
    heading stays within 0.13° through 3.4 s. Fresh runs `h1`: an 18 m/s
    handbrake turn to the right and a 12 m/s one to the left lock both rear
    wheels on the same frame as the original; the right one runs into the
    kerb at 0.5 s (chaotic after), the left one is within 0.7° until the
    release and 3° 1.4 s later. Round 13's `d1` handbrake segment cannot
    be used: its sync lines have no wheel-hop state, and the original had
    a hop running there (rear torque split 0.65 / 0.35).
  * **The clutch state machine runs twice per sub-step while the rest test
    holds the car** (**verified** in the binary and the log; only the order
    was ported in round15; the engine/clutch second pass is restored in the
    2026-11-02 rest-components correction below): 0x5ABB10 (from 0x5B04E0 after
    the list) runs the rest test 0x5A5E30; when it holds the car, or a
    bump stop fires, it sets 0x7FC800 = 0 and calls 0x5A0D90, which runs
    every component's vt+0x1C again — the velocities from the momenta, the
    suspensions (rate kept), the engine's tidy-up 0x5AA770 (heat and turbo
    over 0 s) and the clutch states 0x5A1410. So at rest in gear with no
    gas the stall test cycles Engaging → Disengaging → (Changing →
    Engaging in the second pass) with a period of two sub-steps: the 206
    log `x2` shows five frames of (Engaging, 0) and five of (Disengaging,
    0.0125) switching on each three-sub-step frame. The clone ran the
    drivetrain's post once, after the rest test, and cycled through three
    states, so a launch could find the clutch disengaging and start its
    ramp a frame late. `substep` now runs `Drivetrain::post` before the
    rest test, as the list does. The second pass (dt 0, when the test holds
    the car or a bump stop fires) was tried and matched `x2` frame for
    frame (below), but with it the gentle head-on bump test
    (tests/track_l4ra.rs `cars_bump_and_part`, two 240SXs at 4 m/s) ended
    with both cars stopped, held (class 2), and a zero distance-query result,
    so it was left out. **Corrected 2026-11-02:** the hulls were separated:
    the native query's initial squared-distance threshold 1e-5 can return
    zero for separated supports below about 3.16 mm. The reproduced fixture
    has a strict 1.67 mm separating plane. The test now checks actual contact
    and strict geometric separation; the native query is unchanged. The
    engine/clutch zero-time post is restored at rest and bump-stop sites,
    including a full-compression stop moving upward (no impulse). Native
    dispatch, fresh machine-code threshold probe, recovered original x2
    capture, red/green tests and remaining component-order limits are in
    `docs/rest-component-refresh.md` and `docs/evidence/rest-components/`.
  * **206 clutch-slip remainder** (round 14's open item): with the second
    pass in (not kept, above) the clutch ramp, the lock frame (222) and the
    at-rest cycle were frame-exact; as kept (without it) the launch is
    +0.00 / +0.00 / +0.01 / +0.03 / +0.04 m/s at 0.5 / 1 / 2 / 4 / 8 s;
    with it:
    launch speed +0.00 at 0.5 / 1 s, +0.02 at 2 s, +0.03 at 4 s (round 14:
    −0.22 / −0.45 at 2 / 4 s), heading within 0.15°. **Open** (small): at
    the lock frame the original's front wheels still slip 0.31 m/s, the
    clone's 0.21, and the original's engine sits 1–4 rad/s over idle for
    eight frames (the clone's 0.3).
  * **Re-steps after a resolved contact** (item 2): the clone already
    re-runs the mover for the span left (`step_world`, since round 5). What
    it did not model (**verified** in the binary, ported): the player's
    input update (steering 0x416BB0, pedals, the reverse latch) runs once a
    frame before the physics, so a re-step must not steer or move the pedal
    ramps again (`CarState::restep`, `frame_pedals`); the wheel hop
    0x5B6310 counts the frame's dt (0x865150) on every mover call
    (`frame_dt`); the nitrous shot's time is the clock's (0x592490), so a
    re-step adds none. In 0x590000–0x5C0000 the frame dt is read only by
    0x5B6310, 0x5AA970's code −2 path (not the automatic gearbox's −1) and
    0x5A40B0 (called from the AI at 0x430AC5). Effect
    on the wall runs (`x1`, `z1`, `d1`): under 0.5° in the first second
    after a hit; the runs part chaotically afterwards either way. **Not
    found**: whether the original's first sub-step of a frame uses forces
    computed at the end of the previous frame: 0x5B04E0 ends with `jmp
    [vt+8]` = 0x5B0530 (the force pass), so the forces are gathered at
    each sub-step's end, and Mover_UpdateFrame does not run the force pass
    before its first sub-step (0x5B72E0..0x5B73DE: 0x5AB9A0, 0x5A49B0,
    0x5ABC00, 0x5A4D90, 0x5B6310). If so, a frame's first sub-step
    integrates forces from the last frame's inputs and pose (steering, the
    collision push-out). The clone computes them at each sub-step's start.
    **Open** (not ported; the replays above show no frame-level lag).
  * **Resting gap 3.4 mm** (item 3; **narrowed**, still **open**): the
    shape term is not a margin. 0x5985A0 (new shapes) and 0x5B3D30 set
    pair +0x221 from "shape type 2 with +0x1D0 set and +0x1D4 > 0", and a
    change of that flag only clears the cached simplex (+0x118 = 0, a fresh
    start from vertex 0 of each shape); 0x5B33A0 never reads +0x1D4. Its
    thresholds are on squared distances: a fresh start returns 0 when the
    two vertex-0 supports are within 1e-5 (3.16 mm), a cached simplex
    returns 0 at once when 0x595050 gives ≤ 1e-5 for it, and a round whose
    0x595050 gives exactly 0 ends with 0. 3.4 mm (1.16e-5) is just over
    that threshold, so a sub-millimetre difference in the hull or the
    polygon (or the x87's extended precision against the clone's doubles)
    decides whether the pair touches. Not measured live this round.
  * **Nitrous** (item 5; **verified** live): the career save
    `orig-win1.sav` (made by the original) with the level-1
    nitrous package's part (perf id 12) fitted and owned by the clone's
    profile writer (`career_saves NITROUS=1:out.sav`), loaded in the
    original into the Wine copy's own profile: 206, N2O gauge on. The tank
    is car +0x454 size (8000 ticks = 2 s), +0x458 the shot's start amount,
    +0x45C left, +0x460 the start tick (the "+0x40 block" of round 14 is
    embedded at car +0x40). A 1.218 s shot left 3128 ticks (8000 − 4872),
    the second ran out after 3168 ticks (≥ 3128 at the next mover call).
    Replayed (`INTRO_NOS=1`, synced before each shot, the key aligned to
    the logged start tick): speed within 0.01 m/s through the first shot
    (8.23 → 14.79 m/s) and 0.02 m/s through the second (17.57 → 25.37),
    the 1→2 shift on the same frame, the second shot ending one frame later
    in the clone (its first shot was released a frame early: the
    logger's key-up delay, not the tank). The engine's stage (+0x84) is the
    tank's at the mover call (0x5ABD75), stage 1 throughout. **Refill**: not
    reproduced (`n2`: the handbrake slides scored no style points; the
    tank stayed empty), so the refill rests on <work dir>'s observation (Style
    points and the nitrous refill). The first test drained the tank
    without the logger; it was refilled for `n1` by writing +0x45C = 8000.
  * **Traffic hit** (item 4; **verified** live): `tl.py` logs the traffic
    car with the player (mode +0x28, +0x60..+0xBC, spin +0x110) and
    `hit2` places it (mover +0x0C x, +0x10 y, +0x14 heading, +0x18 speed
    and car +0x40C the u16 heading); its model is car +0x0C → name
    (PANELVAN). Run `t1`: the van, placed slowly ahead, went to the MeloMvr
    (mode 3) 0.25 s later without a contact; the 206 hit it from behind at
    15.68 against 6.11 m/s. `hit_replay` from 0.1 s before (A's drivetrain
    now synced from the log, B on the MeloMvr with `B_GAS=1`, **inferred**:
    the traffic's own gas is not logged): the hit on the same frame, after
    it A 10.47 / 10.47 and B 11.44 / 11.45 m/s, yaw rates −0.22 / −0.26 and
    −0.47 / −0.45 rad/s; with `B_GAS=0` A 10.30, B 11.25. After the hit the
    van's heading parts (its MeloMvr steering is not replayed): A's heading
    2.34 / 1.74° at 0.16 s, 1.68 / 0.72° at 0.4 s.
* **Driving vs the original, round 14** (2026-10-31, physics14; Wine copy
  a private Wine copy on :191 through `fastgpu.sh`, Quick Race Free Run, auto;
  round 13's cave logger and replay with the changes below; tools in
  <work dir>: `lg.py`, `rep.py`, `cl.sh`, `cmp.py`, `cmpw.py`
  (chosen columns side by side), `hopaddr.py`; logs `x2` 206, `f1` Focus,
  `c2` Corolla, and round 13's `d1` / `x1` / `z1` replayed again):
  * **Replay sync fixed** (tooling, not physics): a `sync` line now also
    carries each suspension's compression and rate (+0x1C, +0x20) and the
    wheel hop's four fields, and a teleport is synced one frame later (on
    the teleport frame the car's copy +0x60.. still holds the old velocity
    and spin; the logger writes the rigid body). Before, the clone kept the
    compression of its pose before the jump, so the first step after a
    sync gave a huge damper rate and bounced the car up to 1.8 m/s; it then
    came to rest (class 2) in the air on two wheels. That artefact made
    most of round 13's launch and handbrake differences:
    * **206 launch** (round 13's "3–5 % more front torque"): front torque
      now within 1 % (4.07 / 4.23 kN·m slipping, 0.563 / 0.563 locked) and
      the speed +0.06…0.13 m/s over 10 s (round 13: +0.3…0.6). The rest is
      in the clutch-slip phase: at the lock (frame 222) the original's
      front wheels still slip 0.095 m/s and the engine sits 1 rad/s over
      idle, the clone's slip is 0. **Open** (small).
    * **Handbrake turn, Corolla `d1`**: heading within 0.24° through 0.9 s
      (round 13: +4.8° at 0.5 s), speed within 0.25 m/s; the second rear
      wheel locks one frame after the first (original: four).
  * **The launch wheel hop** (**verified**, ported: `CarState::wheel_hop`,
    0x5B6310, called by Mover_UpdateFrame 0x5B72E0 at 0x5B734F after
    0x5A4D90, each mover call): the player is driver type 1 (car +0x24,
    read live), so it runs in free roam; its state is the car object at
    +0x46C (count), +0x470 (step timer), +0x474 (grip), +0x478 (calm time)
    (the drivetrain's +0x34 and the mover's +0x58 are car +0x40; the
    0x5B6310 decompile reads them as +0x42C..+0x438 of that). Gate: driver
    type 1–3, not ghosted (0x5EBFB0), car +0x42C (forward speed) > 0, URL
    flag 0x89E7E2 and the online flags 0x890100 / 0x890110 clear. Off and
    with the race status *0x890118 = 4 (read 4 in Quick Race Free Run;
    the clone: not held on the line, **inferred** for the countdown) under
    15 mph (0x7FC114): the largest max(0, min(load / 3 · slip, slip))
    (+0x164, +0x118; 3 = 0x7FC110; both from the force pass after the
    frame's last integration, i.e. the end state: The drag-physics parity / the drag-traffic pass sub-step
    capture, ported as `CarState::end_loads` / `hop_loads`, docs/drag-stage4-parity.md
    §6.6) over 0.5 (0x784260) starts it: g =
    graph 0x7FC924 (points 0x7FC0CC: 0 → 1, 5 → 0.8, 9 → 0.9, 12.6 →
    0.833, 17.1 → 0.72, 25 → 0.65; 0x40C840), grip = g / k (k the shift
    bonus table 0x7FC2A8 by layout and class), count = trunc((1.5 − g) · 6
    + (wheel index & 1)) (0x785110, 0x7FC11C), timer 0.5 (0x7FC118). On:
    with |heading − travel| (0x591DA0; 0 under 1 m/s) ≤ 0.3 (0x7FC108), a
    wheel slipping 2.5 m/s or more (0x7FC104) runs the timer down by the
    frame's dt (0x865150) and each time it passes 0 the count drops by one
    (timer back to 0.5); 1 s (0x7FC10C) with none ends it; past 0.3 rad it
    ends at once. While on: every wheel's grip (+0x180) × 1 (0x7FC100),
    then FL and RR × 1 − (1 − grip) · 0.35 (0x7FC0FC) on an even count and
    × 1 − (1 − grip) · 0.65 on an odd one, FR and RL the other way round;
    counts 1 and 2 also fix the differential's split (0x594A40 on car
    +0x46C). Checked in the 206 log `x2`: a 4.1 m/s front slip after a
    teleport at rest gave count 4, grip 0.8755 (g at slip 8.02 m/s, k = 1),
    the front grips 0.9564 / 0.9191 as above, the end after 1.004 s calm.
    A dry standing start (slip under 0.4 m/s) does not start it; the
    Corolla's spinning launches did (counts 3–4). The re-steps after a
    resolved contact run 0x5B6310 again with the frame's dt (**not
    modelled**, the clone has no re-steps).
  * **The tyres' slip angle is the game's table arctangent** (**verified**,
    ported: `atan16`; 0x5A835D calls 0x43C570(vx, vy)): |y| / |x| (or the
    inverse) × −256 (0x786DB8) truncated (0x75BED4) indexes the byte table
    0x7EACB0, × 32; then the quadrant. So a slip angle under 1/256 reads 0
    (no side force from a gripping tyre) and larger ones come in 32-unit
    (0.0031 rad) steps. With the clone's float `atan2` every small yaw was
    taken out at once; with the table a yaw left by the clutch bite stays.
    **Straight-line yaw after a standing start** (round 13's open item):
    Focus `f1` original +0.35° over 8 s, clone +0.30° (before: 0.00); 206
    `x1` +0.35 / +0.27; Corolla from a real rest (`c2`, 15 s) +0.64 /
    +0.70. From the logger's hover start (the car dropped from the
    teleport pose at the gas) the Corolla's drift has the opposite sign
    (`d1` −0.6 / +0.67, `c2` −0.78 / +0.59; the 350Z `z1` −0.33 / −0.5):
    which rear wheel spins first in the clutch-slip phase is chaotic there
    (the original's RR, the clone's RL, from the first frames as the rear
    loads come on), and the yaw follows it. **Open**.
  * **Handbrake turn, Corolla `c2`** (hop 0, clean): both lock the right
    rear wheel on the same frame; the original's left rear keeps rolling
    with the clutch locked for 0.25 s and more (its lock test brakes 5.05
    against r · road + 0.1 · |spin| ≈ 1.0 + 4.05 is marginal), the clone's
    locks four frames later and the clutch opens (stall test): heading
    −0.5° at 0.4 s, then after the release the clone's yaw rate falls
    faster (rear lateral speed 4.27 / 4.46 m/s, more load on the inner
    rear): −1.4° at 1.5 s, −7° at 2.1 s. Not caused by the arctangent (the
    same with float atan2). **Open**.
  * **Resting gap against the planter** (round 13's 3.4 mm, pair +0x221,
    shape +0x1D0 / +0x1D4): not reached this round; **open**.
  * **Traffic hits frame by frame** and **nitrous runs**: not done;
    **open**. For nitrous the tank is on the car object +0x40: +0x414
    capacity, +0x418, +0x41C the remaining ticks (×0.00025 s), +0x420 the
    start tick (0x591CE0 fills it from the record, 0x592490 is the time
    left, 0x59FAB0 / 0x59FB00 start a shot, 0x59FA40 its stage by
    0x7A1798, 0x592510 ends it); a Quick Race car without the part also
    has no nitrous power in its engine record (+0x2AC, read by 0x594310),
    so poking the tank alone gives no torque (**inferred**, not tried).
  * **CARSFX_Stereo on the AI racers** (check of Sound round 12's chain):
    World_Start 0x60A690 calls 0x458850 on the sound manager [0x82B884] at
    0x60A7B3, which (only with 0x7FAAA8, the sound device up) calls slot 5
    of the nine objects at +0xA8; slot 5 0x463BB0 sets +0x1C and calls
    [vt+0x10] on each element of its list (+0x10, next +4). Round 12
    reads those elements' vtable as 0x78BF30 / 0x78BF88 (slot 4 =
    0x469CD0). Those addresses lie inside the car contexts' tables
    0x78BF00 / 0x78BF58 (+0x30, no RTTI word between), whose slot 16
    (+0x40) is 0x469CD0 (0x78BEB0's slot 16 is 0x4022C0, so the tables do
    run to 17+ slots). The one direct call of slot 16 found is 0x481F7D
    in 0x481F10 (CollSfx_Update in symbols.tsv; slot 1 of 0x78BF00 /
    0x78A780 / 0x78BEB0, one argument),
    when 0x7FAAA8 and the context's +0x132 are set (+0x132 set by slot 0
    0x46A540 at 0x46A5E6 / 0x46A650, cleared at 0x46A3BD, 0x481AD5,
    0x481B0B with the active count 0x82B950). So 0x469CD0 → 0x464BB0 →
    0x4765F0 is reached from the context's slot 1 (**verified** static);
    whether the World_Start path also reaches it (list element type) is
    **not confirmed**, and how often slot 1 runs per racer (the counter
    0x82CB6C steps once per call) is **open**. Not checkable under Wine
    here: with no sound device 0x7FAAA8 = 0 (read live) and hooks on slot
    16 and 0x4765F0 saw no calls in a Quick Race. The clone keeps one roll
    per racer at the race start (**inferred**).
* **Driving vs the original, round 13** (2026-10-31, physics13; Wine copy
  a private Wine copy on :173 through `fastgpu.sh` (DXVK on the GPU, full speed,
  120 frames a second, game clock = real time), Quick Race Free Run, auto;
  tools in <work dir>):
  * **Method: a code cave logs every mover call** (`cave.s`, `lg.py`). The
    drive mover's vtable slot 0x7A25EC (Mover_UpdateFrame 0x5B72E0, the
    only reference; thiscall, one float dt, `ret 4`) is pointed at a stub
    in the .text padding (0x782B00; .text ends at 0x782AF1, raw size to
    0x783000). For the player's mover (ECX = [0x8BBE00]) and while
    [0x8BBE0C] is set it allocates 16 MB once through the VirtualAlloc IAT
    slot 0x7831F4 and copies a table of {address, length} regions (.data
    padding 0x8BBE20…, past .data's virtual size 0x8BB5F8) into a 2 KB slot
    per call, then jumps to 0x5B72E0: the car +0x50..+0x140, +0x424..
    +0x444, the human input +0x1E0..+0x2F0, each wheel +0x20 and +0xB0..
    +0x190, the suspensions +0x1C, engine and drivetrain, rigid body
    +0x20..+0x90, 0x865188 and 0x865150, plus the stack dt. No tearing, no
    missed frames; read back after the run. `rep.py` turns a log into the
    clone's `INTRO_FRAMES` (one frame per game clock with the original's
    dt, the analog pedals and steering of the human input, and `sync` lines
    with the full state at the start and after each teleport: centre of
    mass, rows, velocity, spin, angular momentum, wheel spins, gear /
    target / shift state / clutch / input shaft / pending, engine speed);
    `cl.sh` replays it (`--shot intro`, `INTRO_CAR`, `INTRO_FRAMES`,
    `INTRO_FOUT`) and `cmp.py` lines the two up frame by frame.
  * **Found under Wine** (**verified**, logs): (a) the mover is called
    **several times in one frame** (same 0x865188): the first call with
    the frame's dt, then once per resolved contact for the span left
    (e.g. 0.00303 s, then 1.2e-5 s); 2–9 % of all calls in the runs below.
    (b) Frame dt jitters 0.00833 / 0.00889 / 0.01–0.015 s at the 1/120
    cap. (c) Fields: drivetrain +0x3C the gear (+0x38 the target, +0x44
    the shift state 0–3, +0x48 pending; car +0x434 = +0x3C), car +0x440 is
    a smoothed rpm (engine +0x14 is the engine speed), human input +0x1E0
    the stored u16 wheel angle, +0x2C8 steering, +0x2D4 / +0x2DC / +0x2EC
    raw throttle / brake / handbrake (+0x20C / +0x214 / +0x210 the pedals
    after the reverse swap), PlayerSteering is embedded at input +0x220.
    The idle pedals read 1/255 in this setup (no pad attached).
  * **Zero-length re-steps** (**found**, ported): the suspension update
    0x5A87A0 writes its damper rate +0x20 only when the sub-step 0x7FC800
    is over 0.0001 s (0x5A8877), else the last rate stays. The clone
    divided by the step: a 1.2e-5 s re-step gave inf loads and NaN spin
    (seen replaying the 350Z wall run, frame 813). Now
    `WheelState::compression_rate` keeps the rate.
  * **Broad drive, 3 cars** (60 s each: launch and shifts on the airport
    straight (1724.23, −837.16, 80.78°), braking turn, hard corner at 20
    m/s, handbrake turn at 15 m/s, a run from rest, round 7's wall at 21°
    then reverse, a 2 m kick at 15 m/s, round 8's slalom; teleports
    between; logs `d1` Corolla, `z1` 350Z, `x1` Peugeot 206 (FWD; the RX-8 is not in this build's Quick Race list)). Clone /
    original at the same frames:
    * Launch and gears: Corolla 2.40 / 2.31 m/s 0.6 s after the bite, then
      +0.10…0.21 m/s through 9.5 s (22.32 / 22.21), shifts on the same
      frames (1st→2nd at 19.0 m/s); 350Z within 0.08 m/s through 8.8 s
      (34.65 / 34.67), shifts on the same frames. Clutch ramp identical
      once the replay synced the drivetrain's pending flag (before, a sync
      artefact).
    * **206 launch** (**open**): the clone pulls ahead in 1st, 2.19 / 1.87
      m/s 0.6 s after the bite, 10.40 / 9.81 at 4.1 s, then a steady +0.3…
      0.4 m/s; with the front slips equal (−0.010 / −0.011) its front-wheel
      torque is 3–5 % higher (0.729 / 0.694 kN·m at 3 m/s, 0.933 / 0.902 at
      8 m/s). Not traced (the RWD cars do not show it).
    * Straight-line yaw (**open**): the original's heading drifts after a
      standing start, −0.6° over 8 s (Corolla, 80.78 → 80.17°), −0.3°
      (350Z); the clone holds it within 0.03°. It starts at the clutch
      bite: the original's roll kick (L.x −0.14) leaves L.z −0.0107 that
      decays slowly; the clone's yaw momentum returns to 0 within 0.1 s.
      Not traced.
    * Braking turn, slalom, kick landing: heading within 0.5–0.9°, speed
      within 0.2 m/s (the 2 m kick: 10.17 / 10.06 m/s 0.5 s after landing,
      Corolla).
    * Handbrake turn at 15 m/s (**open**, the largest): the original locks
      the second rear wheel 3–5 frames after the first (both locked at
      0.04 s), its clutch opens (stall test) and the engine drops to idle;
      in the clone the second rear wheel keeps 36 rad/s for ~0.15 s more
      (clutch locked, engine 450 rad/s): heading +2° after 0.25 s, +4.8° at
      0.5 s, +10° at 1.0 s (Corolla); 350Z +1.3° at 0.6 s. The coupling's
      Δ torque (0x594DB0 / 0x594850 / 0x594A40 decompiled) matches the
      clone's code as read; the difference is not found.
    * After crashes the runs part (hard corner into the planter at 20 m/s:
      chaotic, both stop within 1 m of each other only for the Corolla).
  * **Reverse from the planter** (round 8's open item; 350Z pushed at
    (1698.14, −850.44) 77.14°, 4 s gas, 4 s rest, Down): original 1.23 /
    2.01 / 3.02 m/s at 0.13 / 0.25 / 0.5 s and 4.9 m/s² after, clone 0.58 /
    0.94 / 1.73 and 3.2 m/s² (open ground: both 3.3 m/s²). Cause
    (**verified** in the log): on the first reverse frame the original's
    new pair measures an overlap, pushes out 23 mm and makes a wall record,
    so the wall drive (rear share 0.65, see Wall drive) sends torque to the
    front wheels for the record's 0.3 s (front T −1.1…−1.46 kN·m); with
    four driven wheels the rears never break loose. The clone (resting at
    3.4–8.2 mm, no contact) spins its rears (slip −0.6…−1.3 m/s) for the
    whole reverse. Synced to the original's resting pose, the clone still
    measures 3.4 mm where the original's fresh pair measures 0: **open**
    (candidate: the shape term 0x5985A0 / 0x5B3D30 carry in pair +0x221
    from shape +0x1D0 / +0x1D4, which zeroes the cached simplex +0x118;
    not traced into 0x5B33A0's supports).
  * **The stale +0x14** (**found**; measurement log `MEAS=1`: a second
    stub at 0x782C00 on the three calls of 0x5B3D30, 0x5B463D push-out,
    0x5B4CF5 time of impact, 0x5B51C2 detection, logging dirty, +0x14,
    +0x15, +0x10 and the reference positions): while the car presses the
    planter, the time of impact's bisection (0x5B4BF0 → 0x5B3D30 at
    0x5B4CF5) measures the pair at intermediate poses; when its last dirty
    query lands on an overlapping pose, +0x14 = 1 stays, and the next
    frame's detection (0x5B51C2) takes it without measuring because the car
    moved under 1 mm since that query (e.g. frames 646–647: TOI d 0.0037,
    then overlap, then the detection "touch, not dirty"). That is the
    contact reported at a distance the end pose does not have. Then the
    car went still (rest class 2) and every frame's detection, eight TOI
    queries and twenty push-out tries all answered from that one cache
    (none dirty), until it slept overlapping by a few mm.
  * **Push-out on a car at rest** (**verified**, ported): the resolution
    0x5B2620 adds the push-out offset to a body only when its vt+0x64 is
    true (0x5B26D9, 0x5B2723), and the push-out 0x5B4300 moves a body's
    test position only then (0x5B4588, 0x5B45C8). For the car collision
    body (vtable 0x7A2478) vt+0x64 is 0x5AEE90: !0x5ED200(car), which is
    true for mover mode 1 or 3 when the mover's vt+0x14 (0x5B04A0: +0x60,
    the rest test's class, 0x5ABB10 passes mover +0x60 to 0x5A5E30) is 2.
    So a car asleep is never pushed out. Clone: the track and car-pair
    push-outs skip a car whose `rest` is 2. (The clone's car did not reach
    this state in the planter run, so the reverse gap above stays.)
  * **MeloMvr leftovers** (ported): the brake flag is the car update's
    (0x5F62C0 from 0x6049D9) on the driver's +0x214 whatever the mover,
    so a racer on the MeloMvr now notes its brake (`CarState::simulate`,
    before: its brake lights froze); the AI racers' engine sound context
    takes the driver's gas (car +0x30 → +0x20C, SFXCTL_Physics 0x484C50),
    not the drivetrain's throttle (stale on the MeloMvr). There are no
    reverse lights in the game (0x615640 knows six light hashes; see Brake
    flag). **Open**: CARSFX_Stereo on the AI racers (its slot 7 comes
    through vtable slot +0x40 of 0x78BF00 / 0x78BF58 (0x469CD0), caller
    not found).
  * **Not done** (**open**): traffic hits frame by frame (needs a race
    with traffic and a second car in the cave's table; HIT2 with +0x40C
    as round 12 noted); nitrous in the broad runs (no nitrous fitted in
    Quick Race, the Alt presses did nothing in both).
* **Driving vs the original, round 12** (2026-10-31, physics12; Wine copy
  a private Wine copy on :163, Quick Race circuit Resort Loop, Traffic Level
  Maximum (byte 0x83AAC8 + 20 × type, type 0x83AAB4 = 0), 3 opponents,
  Peugeot 206 (PEUGOT), auto; tools in <work dir>: `tlog.py`
  logs the player and a second car per frame (mode +0x28, +0x60 / +0x70,
  rows +0x90…, spin +0x110, the player's wheel spins), `HIT=` teleports
  the player's rigid body behind a car (body +0x20 pos, rows +0x30 /
  +0x40 / +0x50 and their copy +0xD0…, +0x70 momentum = mass +0x164 ×
  v, wheel spins +0x28), `HIT2=` moves a TrafficMover car ahead of the
  player (mover +0x0C x, +0x10 y, +0x14 heading, +0x18 speed — and car
  +0x40C, the u16 heading the mover reads back: without it the body spins
  41 rad/s for a frame); `rlog.py` logs the racers' mode, hold, wheels
  down and distance; the maintainer's probe `hit_replay` replays a logged hit in the
  clone from one frame on). The world list is [0x890080] +0x10 (count)
  with pointers from +0x1C (+0x18 is the racers' count).
  * **AI racers' mover** (**found**, ported; `racer_mover` in game.rs,
    `CarState::human_hit` / `leave_melo`): the AI racer controller's
    update 0x42C0C0 (before its think 0x42ABD0) keeps a physics hold at
    controller +0x734 (ticks, counting down through 0x577320: −
    trunc(dt × 4000), 0 when used up). Before the green (0x5EEF00 false,
    not drag 0x89E7D8 / 0x890108) the hold is trunc(5 × 4000 + 0.5) =
    20000 (0x783518). While it runs the racer is on the physics mover
    (mode 1). When it runs out: with fewer than four wheels down (car
    +0x424) it gets 4000 ticks more; else, where the world under the car
    is loaded (+0x5CC), the car takes 0x5F4410's mode — for a racer (kind
    3) mode 3, the MeloMvr, since the controller's slot +0x50 (0x41E510:
    hold ≠ 0 and ≠ 0x7FFFFFFF) is false; 1 in drag races (0x89E7D8,
    0x890108) and drift races (0x401950), or with 0x8650AC set (0 live).
    A finished racer (car +0x1C and 0x5EE260) is put on mode 1. Any
    resolved contact calls the controller's slot +0x38 (0x5B2620 → body
    slot +0x48 0x593950 → 0x5F4970(other car or 0, 0)): the racer's
    0x402450 gives mode 1 and a fresh 20000-tick hold (0x78349C) when the
    other car has +0x18 set — only the human's car (live: +0x18 =
    0x10790928 for the player, 0 for racers and traffic) — else
    0x5F4410's mode (1 while a hold runs, else 3); no loaded world → 4.
    Live: all three racers mode 1 with the hold counting from 5 s at the
    green, mode 3 from the frame it ran out (1.36 s → 0 over 1.36 s of
    race clock), and mode 3 for the rest of the 20 s logged (up to 305 m
    ahead, world loaded); racers 300 m+ away whose sections are not
    loaded are mode 4 (0x5A9040; the clone loads the whole world, so it
    has no mode 4). **Clone before**: every racer on the physics mover all
    race. Now: `racer_mover` runs the hold per frame before the think
    (the driver's mover field follows: 3 → the PID's mode 0), a racer
    whose hold runs out takes `Melo::new` with its rating, a contact with
    the human's car switches it back inside the collision step (before
    the impulse, as 0x5B2620 calls slot +0x48 first, and for the rest of
    the frame) and the race step restarts its hold. The way back takes
    the MeloMvr's gear and rpm into the drivetrain (the wheel spins are
    written back each frame); that the physics mover's set-up (0x5B5CD0)
    reads them from the car as the MeloMvr's does (0x5A2A70) is
    **inferred**. `CarState::rpm` reads the MeloMvr's rpm while on it.
    Not ported: a finished racer touched by a non-human car with no hold
    (0x5F4410 → 3 until the next update's mode 1).
  * **Traffic hit → MeloMvr** (**verified** live): a TrafficMover car
    (mode 2, vtable 0x78456C) the player hit (h6, an overlapping
    placement) went to mode 3 (vtable 0x7A2540) in the same frame (m2 →
    m3 between two logged frames; the player 16.16 → 11.45 m/s), as the
    clone's
    `traffic_settle`. In a run right after a 110 m teleport (h3) the
    player pushed a TrafficMover car for 0.7 s (it held exactly 14.50
    m/s, kinematic) before it switched: 0x408ED0 switches only with car
    +0x5CC (the section's collision pack) set, which streams in after a
    jump; the clone's world is always loaded, so this does not arise.
  * **Traffic hits, impulses frame by frame** (**open**): the clean
    rear-end at known speed was not captured (the HIT2 placement without
    +0x40C overlapped the cars; the racers' traffic placement re-places
    cars when the player is teleported off the race coordinate). Next:
    HIT2 with +0x40C on a straight, then `hit_replay`.
  * **Bump-stop totals** (**verified**; nothing to port): [mover +0x58]
    is car +0x40 (the drive mover's constructor 0x5B5CD0 stores its arg
    + 0x40 at 0x5B5D41), so the totals are car +0x370 … +0x37C. Mover_
    UpdateFrame 0x5B72E0 zeroes them at the frame's start (0x5B731B);
    0x5920C0 zeroes one wheel's when its state byte changes. Their only
    reader is the force feedback 0x406460 (0x4066B4): the four summed,
    over 0.05 a jolt (0x5C9770) of min(1, sum / 4), at most one per
    0.15 s per pad (0x827AA4). The clone has no force feedback.
  * **Wall-reverse contact** (narrowed, **open**): 0x5B3D30 measures a
    pair only when dirty (+0x220); 0x598630 marks it dirty only when
    either body's position (vt+8) moved more than 0.001 in a component —
    the orientation is not compared — and 0x5985A0 only for new shapes.
    Otherwise the pair's +0x10 distance and touching flag +0x14 stay from
    the last measurement, and 0x5B5060 takes a contact from +0x14 alone.
    So a stale +0x14 = 1 from a measurement at another pose can report a
    contact at a 5–11 cm end distance; which measurement set it in round
    8's runs is not traced. The clone ports the same cache
    (`narrow::distance_at`, round 5).
  * Steering: not touched (the vanilla round 2 agent re-measures it).
* **Driving vs the original, round 11** (2026-10-31, physics11; Wine copy
  a private Wine copy on :163, Quick Race Free Run, 240SX, auto; tools in
  <work dir>: `klog2.py` = round 10's `klog.py` plus `SLOW=rate`,
  `RECS=1` and the wheel spins / world step per frame; `cmp.py` lines a
  Wine log up with a clone log at the first touchdown):
  * **Sub-steps in the original** (method): the world's slow-motion rate
    ([0x890080] +0x108, seconds +0x10C = 1e9, callback +0x110 = 0; see
    World_FrameTime 0x5EA370) poked to 1/12 makes the world step
    0x865150 4.17 ms, so each Wine frame (0.05 s real) is one 1/240 s
    sub-step and the reader sees every sub-step. The clone's match:
    `INTRO_HZ=240 INTRO_STEP_HZ=240` (`INTRO_SUB=1` logs slip, Fx, spin and
    compression per wheel). The per-frame set-up (gears, brakes, yaw
    limit) then runs every sub-step in both.
  * **Bump stops** (**found**, ported; `CarState::substep`): the drive
    mover's post-step 0x5ABB10 (from Mover_UpdateComponents 0x5B04E0 at
    the end of every sub-step, after the suspensions measured the new
    pose) runs the rest test 0x5A5E30 and, when that returns 0 (class 0),
    for each suspension whose compression +0x1C equals the record's
    maximum (+0x14) Suspension_BumpStop 0x59E1B0: the hub (+0x30: the rest
    point +0x90 with the compression added to z, car space, turned by the
    body) closing speed c = −n · (v + ω × r) along the wheel's ground
    normal (+0x150); c ≥ 0 gives J = 0.225 · mass (body +0x164) · c · n at
    the hub through RigidBody_ApplyImpulse 0x59BC70 (momentum capped at
    1000, its moment at 10), unless 0x86B36C (0 live). The totals go to
    [mover +0x58] +0x330 + 4·i (car +0x370 …; read only by the force
    feedback 0x406460, see round 12; nothing to port). After a bump or a hold: 0x7FC800 = 0 (the following force
    pass only; the next sub-step sets it again before its derivatives, so
    the wheels' slip term 0x59D360 is unaffected) and 0x5A0D90. The
    MeloMvr's twin is 0x5AB0D0 / 0x5A3E90 (0.2 × mass). Round 10 had
    taken 0x5AB0D0 for the drive model's and so found no bump stop.
    The clone applies them at the start of the next sub-step, right
    after measuring the compressions, before any force (same order).
  * **Landing at 15 m/s, gas held** (2 m kick, at (1730.77, −801.91),
    −100.33°, sub-step logs): through the spin-up (fronts −52 / −51 kN
    for 8 sub-steps, 0 → 39 rad/s; rears −43 → −20 kN) the two match
    within 0.2 m/s in vz and 0.1 rad/s in pitch. The original's fronts
    reach full compression ~14 sub-steps after touchdown and the bump
    stops turn vz −2.67 → −0.80 and pitch +0.88 → −0.11 rad/s within two
    rows; the clone had none, kept nosing down to fz −0.064 and then hit
    the ground with the hull. Now, at 20 Hz on round 10's run (j1,
    (1718.67, −836.67), −95.57°): forward z −0.034…+0.027 / −0.037…+0.029
    (before −0.062…+0.095), vz per frame −2.45, +0.56, +0.80 / −2.25,
    +0.61, +0.82, no wheelie (fronts keep 1.1–1.4 / 0.3–1.3 kN), speed
    13.21 / 13.28 m/s 0.45 s after touchdown and 14.59 / 14.66 at 0.75 s.
    The 2 m drop at 1.8 m/s is unchanged (lowest z 23.978 / 23.96).
  * **Rest test's wheels** (ported): see Rest test — the drive mover's
    call passes the mover, so its wheel spins follow the momenta.
  * **Planter at 45°** (round 8's slalom, (1708.42, −851.10), 78.09°, Up
    4.1 s, A / Right 0.7 s each): the hit at ~6.0 s takes 12.64 → 9.41 /
    12.25 → 9.30 m/s, then 8.31 / 8.44 (6.2 s), 6.92 / 7.19 (6.4 s);
    heading 124.1 / 123.3°, contact point (1712.55, −802.14) /
    (1712.53, −802.26). Matched (round 8's 13.3 → 9.4 / 13.1 → 8.3 is
    gone with the fixes since).
  * **Catch-up +0x468 of the human** (**verified**): 1 unless split screen;
    see Race catch-up in the physics.
  * **Not done** (**open**): traffic hits and AI racers live — Quick Race
    Free Run has no traffic (the world list [0x890080] +0x18 / +0x1C
    holds the player and three parked AI cars at z −123456) and this
    copy's free roam loads none (base bug), so both need a race session
    set up in the Wine copy; the wall-reverse contact query (round 8).
* **Driving vs the original, round 10** (2026-10-31, physics10; Wine copy
  a private Wine copy on :163, Quick Race Free Run, 240SX, auto; the logger
  `klog.py` in <work dir> (a peek loop with writes, `boot2.py`)
  logs the car each frame with the wheels' +0x118 slip, +0x11C, +0x124,
  +0x164 load and the suspensions' (mover +0x28..+0x34) +0x1C / +0x20 /
  +0x24, and can *kick* the car: rigid body [mover +0x20] +0x20 z += dz,
  +0x70 P = m·(v·forward), +0x80 L = 0; and `TOP` pokes tuning +0x290.
  Clone: `--shot intro` with `INTRO_KICK=dz,v[,vz]` at key step
  `INTRO_KICK_STEP` and `INTRO_TOP=mph`; the log line now has vz, the
  wheels down and the four loads. Under Wine the frames run 0.05 s with
  some 0.1 s ones (two steps between reads), so per-frame alignment is
  ±1 step):
  * **Drops** (**verified**): 1 m at 2 m/s (bay, (1709.44, −876.84),
    −100.1°): vz at touchdown −0.892 / −0.867 (original / clone), then
    −0.032 / −0.016, +0.304 / +0.316, +0.254 / +0.258, +0.161 / +0.163;
    loads 11.9 / 11.5 / 8.6 / 8.7 against 12.5 / 11.4 / 8.4 / 9.5 kN; the
    coast after within 0.03 m/s. 2 m at 1.8 m/s: vz −6.07 → −2.88 / −6.03
    → −3.17, lowest z 23.978 / 23.96, bounce +0.68…0.75 / +0.64…0.84.
    2 m at 11 m/s with gas, landing on one rear wheel (−946.33,
    −99.89°): the next frame vz −2.611 / −2.635, loads 25.5 / 27.4 /
    21.2 / 18.4 against 26.6 / 27.4 / 21.2 / 20.4; both fronts then reach
    full compression (0.16 m, 14.11 kN) in both; the original lifts its
    nose a frame sooner (fz +0.016 against +0.006, then within 0.007).
  * **Landing at 15 m/s** (**open**, mismatch): 2 m, gas held, wheels
    still slipping from the kick (front −12.4, rear −6…−9 m/s at
    touchdown). The clone noses down further and then wheelies: forward
    z down to −0.062 (fronts at full compression 3 frames) and up to
    +0.095 with the fronts unloaded for 7 frames; the original's forward
    z stays within −0.024…+0.027 and its fronts keep 0.6–1.5 kN. Speed
    after the landing within 0.1 m/s (15.0 → 12.45 in both). The
    original's landing frames fall in a 0.1 s frame, so the sub-step
    where the wheels spin up (the front's braking at touchdown, 43 kN
    per wheel in the clone) is not seen; the suspension (0x5A87A0, no
    bump stop for the player: 0x5A3E90 is the MeloMvr's, vtable
    0x7A2538), the inertia (rigid body +0x170 / +0x1B0 diagonals 0.4596,
    2.2463, 2.3675 / 5.2084 = the clone's) and the force point (wheel
    +0x140 = the contact, 0.472 m below the centre of mass at rest,
    0x5B0530) match. Not traced further.
  * **Limiter** (**verified**): tuning +0x290 poked to 50 mph, gas from
    rest on the airport straight: both hold 23.16–23.17 m/s (51.9 mph)
    from 6.3 s; speeds through the climb within 0.15 m/s, gears at the
    same times. So a car's top speed is its record's +0x3A0 + 1.9 mph
    unless the drag curve tops out first (the computed top speed, see
    "Computed top speed"), which uses the drag and torque checked live
    up to 31 m/s (round 9) — **not** run to the real top speed on a
    road (no straight long enough found).
  * **Over-speed kick** (40 m/s in 1st from rest): the shift to 2nd at
    rear slip −7.5 m/s (see Automatic gears, fixed); after it speed
    within 0.2 m/s and position within 0.1 m over 1.1 s.
  * **AI racers** (**traced**): they step through the same `step_cars`
    / 0x5ABC00 as the player with the driver's inputs; the physics
    differences are the catch-up (now ported, above) and the launch
    wheel hop (open). Not compared live.
  * **Draft** (ported): the drag × (1 − car +0x4DC) (aero +0x0C,
    0x599180 → 0x59E681); the clone had the draft for the style points
    only.
  * **Not done** (**open**): traffic hits (both cars), the wall-reverse
    contact and the 240SX planter at 45° (round 8), a ramp jump on the
    road (the kicks stand in for it), top speed on a road.
* **Driving vs the original, round 9** (2026-10-31, physics9; Wine copy
  a private Wine copy on :163, 20 frames a second; `flog4.py` with car +0x120..+0x128
  (angular acceleration) and per wheel +0x28 spin, +0x118 slip, +0x124,
  +0x108 added; clone `--shot qrstart` with `NFSU2_QR_MODE=drift|sprint|drag`,
  `NFSU2_QR_CAR`, and `INTRO_POSE` (now `x,y[,z],heading`) / `INTRO_KEYS`
  from the green light; `--shot intro` with `INTRO_NOS` (the level-1
  nitrous parts) and the key `nos` (Left Alt)):
  * **Drift race** (Quick Race Drift, Stadium Drift 2, 240SX, auto, from
    the grid at rest, the original's key frames replayed): gas 2.6 s, A
    0.8 s, Right 0.7 s, gas. Straight: speed within 0.02 m/s and position
    within 0.07 m over 2.6 s. Turn and counter-steer (after the steering
    fixes above): heading within 0.8° at the reversal (3.4 s), within 4° by
    4.3 s; speed within 0.3 m/s; position within 0.9 m until the wall at
    ~4.4 s. Before: the inside wheel held 38.9° against 41.7°. The front
    outside wheel slides for 2–3 frames at ~2.9 s in some runs of the
    original (sl 1, slip −0.66 m/s, +0x124 −0.79 kN, Fy 8.5 against 11
    kN) and not in others; the clone does the same, so runs differ by a
    few degrees from there (**observed**: chaotic, not a mismatch). The
    undriven front wheels push forward (+0x124 0.7–2.4 kN) as the clone's
    mirrored rolling constraint has them. The original's restart grid
    moves by ~0.5 m between restarts ((−1033.63, −647.00) and (−1034.18,
    −646.86)); the clone's grid for this race puts the player at (−1040.46,
    −643.58) (**open**, the drift grid layout is queued separately).
  * **Nitrous** (**verified**): Quick Race Customize fits a single part,
    "Dry Shot of Nitrous Oxide" (part 12, the level-1 package), on a fresh
    profile. 240SX in Free Run on the airport straight from (1724.23,
    −837.16), 80.78°: gas 2.05 s, gas + Left Alt 3.05 s, gas 1.5 s. The
    shot runs 40 frames (2.0 s) and lifts the acceleration from 4.4 to
    8.2 m/s² in 1st; speed within 0.16 m/s through 6.5 s (23.65 / 23.58
    m/s at 4.0 s, 28.86 / 28.75 at 6.5 s), position within 0.4 m. Before
    the tick fix the clone's shot ran a frame long (+0.4 m/s).
  * **Not done** (**open**): top speed per car (the longest straight in
    the race road networks is ~175 m; the airport straight ~230 m),
    jumps and landing, traffic hits, AI racers' physics against the
    player's, the wall-reverse and 45° planter items of round 8.
* **Driving vs the original, round 8** (2026-10-31, physics8; Wine copy
  a private Wine copy on :163, Free Run, 20 frames a second, clone `INTRO_HZ=20
  INTRO_STEP_HZ=20`; `flog4.py` in <work dir> logs per frame the
  car, the records near it with their pair, that pair's +0x10 / +0x14 /
  +0x15 / +0x220 / +0x240 / +0x20, the car's collision body (+0x19,
  +0x120, its start state +0x6C / +0xB0 / +0xC0 from 0x593220) and, with
  `WH=1`, the wheels (+0xB0/+0xB4 angle, +0x160, +0x170, +0x128, +0x164);
  `INTRO_PAIRS=1` prints the clone's pair distances):
  * **Steering rate boost** (**found**, ported; see Player steering): the
    240SX slalom from (1716.25, −827.20), 74.58°, 3.75 s gas, then A 0.5 s,
    Right 0.85 s, A 0.9 s, Right 0.55 s. Front-left wheel angle per frame,
    original / clone before / after: 14.0 / 9.2 / 14.0, 30.0 / 18.6 /
    30.0, 36.6 / 28.2 / 36.1°; at the reversal 45 → 25.8 / 32.4 / 25.8 →
    5.7 / 20.2 / 5.1°. Heading at the first peak 109.2 / 111.4 (s2 run) /
    110.0°; through the rest of the run within 2°, speed within 0.5 m/s
    (round 3's "8° more before the reversal" was this). In a run whose key
    press landed a frame late in the original, the clone matches with its
    key a frame later too (the logger cannot see when within a frame the
    game reads the key).
  * **Reversing away from a wall** (**open**, narrowed): Corolla pushed
    against the planter north of the airport bay, keys off, 3–4 s, then
    Down. At rest the car's body sleeps at once (+0x19 = 0 the frame the
    keys go off) with its wall pair at 6.5–8.2 mm (the TOI's last query;
    the clone rests at 7.8 mm, its first reverse frames measure 9.5 mm).
    Two outcomes in six runs: (a) a contact on the first reverse frame
    with the floor push 0.555 at the rest contact point (r4–r6, round 4),
    on a **new** pair (another address than the push phase's: the woken
    body's track polygons are made again), the frame's start state +0xB0
    then the rest pose: |v| 0.75 / 1.23 / 1.66 / 2.01 / 2.16 m/s in the
    first five frames; (b) no contact for 4–5 frames — the old pair not
    measured at all (+0x20 frozen while the car moved 7 cm) — then a
    contact (record, pair count 1) with no push (separating), the wall
    drive engaged for the record's 0.3 s: 0.12 / 0.24 / 0.35 / 0.49 /
    0.63 / 0.81 / 1.03 m/s (r3; r1 the same). The clone (no contact):
    0.13 / 0.26 / 0.37 / 0.51 / 0.65 / 0.80 / 0.94, matching (b) until its
    contact. In both the pair's end-of-frame distance is 5–11 cm when the
    contact is counted, so it is not measured at the end pose; which
    query reports it (a recycled pair's stale +0x14 / +0x220, or the
    woken body's polygon rebuild) is **not traced**. A pair made by
    0x5A0350 is dirty and untouching only when 0x5985A0 sees new shapes.
  * Steer-in at 16 m/s (240SX, A 0.7 s from 78.09°): heading per frame
    within 0.5° of the original (115.6 / 116.1° after 0.7 s). That run
    then hit the planter at (1712.5, −802.1) at ~45°: speed 13.3 → 9.4
    m/s / 13.1 → 8.3 (open, small; the clone's path was 0.3 m west).
  * **Not done** (**open**): nitrous, jumps and landing, traffic hits (this
    copy's free roam has no traffic), AI racers' physics, top speed per
    car, drift-race physics.
* **Opening and driving vs the original** (2026-10-30; private
  Wine copy $OB_PRIVATE/wine/runs/<run> on :131, `opening.py` shoots the
  boot and the career start, `samp.py` / `accel.py` / `wspin.py` /
  `pursue.py` drive it and read the car through peek):
  * Order (**observed**): Start New Career → the splash-art LOADING
    screen (Rachel, "LOADING" and its bars; no controls list) → (the
    intro movie: the modded Wine build showed black for movies, so it
    could not be compared; the original without the widescreen fix
 plays them: the boot intro's picture spans rows 150–810
    of 1280 × 960 between bars of grey 17, as the clone's `--shot
    boot_intro` (151–807, 17)) → CareerNis1 → chase view with the phone prompt (state
    2) → Rachel's SMS → free roam. The clone's order is the same. Wall
    time cannot be compared: under wined3d the game runs at 0.1–0.4×
    (frame steps capped near 0.04–0.08 s; the clock 0x8651A8 advances
    with the simulation: distance = ∫v over it).
  * Car at rest (**observed**): after the SMS the car stands at
    (1695.25, −883.64, 24.11), speed 0, gear +0x434 = 2 (1st), rpm +0x440
    = 800; the clone reports 0.00 m moved (the rest test is ported).
  * Player car in memory (**observed**): player [0x8900B4], car = player
    +4: +0x60 position, +0x70 velocity, +0x90 the orientation rows, +0x430
    speed, +0x434 gear (0 R, 1 N, 2 = 1st), +0x440 rpm (shown; engine +0x14
    is the crank speed in rad/s); +0x34 the mover (+0x38… wheels, +0x48
    engine, +0x4C drivetrain); wheel +0x28 spin, +0x108 torque, +0x138
    load, +0x160 sliding, +0x16C slip; engine +0x9C the turbo needle (0 →
    +0xA4 over ~1.5 s at full gas; not read by the torque 0x594310).
  * Full gas from rest on the airport straight (1725.5, −836.6, heading
    78°), original / clone (25 Hz steps, `INTRO_POSE`, `INTRO_KEYS`):
    1→2 at ~7100 rpm (≈20.5 m/s) / ~7200 rpm (≈20.2); 2→3 at 7320 rpm
    (≈34 m/s) / 7210 rpm (≈35); at 7.9–8.0 s 45.7 m/s after 206 m / 44.7 m/s after 211 m.
    1st gear: both run on the rear friction circle (+0x170 = 1), Fx =
    1.575 × load (1.832 × 0.85, the Stability Control factor above).
    Original / clone 8.58 / 8.59 m/s² from 4.2 to 12.8 m/s, speed against
    distance within 1%, wheel loads within 0.05 kN (before: the
    clone 10.3 m/s² without the factor, then 8.1 with the factor but
    81% of the downforce). The original's earlier "load swing" was a kerb.
    2nd gear 5.9 / 5.8 m/s²; 3rd 3.95 / 3.8; 45.7 / 45.8 m/s at 8 s
    (206 / 214 m; the original starts ~0.1 s late after the key).
  * HUD (**observed**, `--shot intro` against the original's standing
    frame): the 25–26 px offset of the right-hand elements (and 3–4 px of
    the left ones) came from the Wine copy's ThirteenAG widescreen fix
    (`SCRIPTS/NFSUnderground2.WidescreenFix.ini`, `FixHUD = 1`). With
    `FixHUD = 0` (a private Wine copy, now the setting there) the Bank text and
    the tachometer line up horizontally to 1 px; the tachometer ring sits
    ~4 px lower in the clone (**open**, small). Earlier notes: The
    package positions in the original's memory equal the clone's (tach
    group 5164D4EA (142, 195, 100), bank group 3F8644CA (193, −193.5,
    83.5)) and every FE transform found centres at (320, 240) (0x536F70,
    0x51B3C4, 0x5368D4, 0x536AA2, 0x536CD2, 0x50B4EE). **Open**.
  * While driving (after ~30 s) the original showed an SMS envelope next
    to the phone / explore icon and stage-0 race icons on the minimap
    (0x86347F = 1). The clone's intro shots did not arm Rachel's hints
    (they skip `career_intro`); they now do (`Hints::dday_a`), so the
    reveal comes 15 s in as in the real flow. The envelope's message is
    **open** (not read; no SMS is due by the hint table then).
* **Stage 0's races** (**verified**; `dday::Hints`, `Profile::intro_races`
  / `intro_closed`): the open-event build 0x527D40 opens nothing in
  stage 0 until career +0x873F (0x86347F) is set, nor while a world
  challenge runs, nor in stage 0 while DDAY_EVENT_B is current or once
  +0x873E (0x86347E) is set (both saved, 0x514585 / 0x529299; cleared by
  the career reset 0x532A30). Making DDAY_EVENT_A current (0x5410F0 →
  0x524250) posts hint kind 2 in 60000 ticks (15 s of free-roam driving
  after the script) and kind 3 in 160000, each unless queued. Kind 2
  (0x53E0C0 case 2): +0x873F = 1, the marker table and the HUD rebuilt
  (0x533320, 0x4F92A0): stage 0's three races (CA207583, CA207584,
  1BCB7AB0) open, with Rachel's line (ported, **Rachel's speech**) — no SMS. Each
  of those races over, won (0x541430 → 0x54152D) or lost (0x54C950 →
  0x54C9C0 → 0x528510 → 0x52855A), runs 0x5242D0(event, won): kind 1 in
  4000 ticks unless queued (DDAY_EVENT_A won sets the counter +4 to 3).
  Kind 1 (case 1) counts Rachel's complaints 0 → 1 → 2 → 3, each a voice
  line (0x510F10, ported); the third sets +0x873E, closing the races
  again and, with 0x513D30 / 0x500CB0 / 0x513E00, the outrun encounters,
  which stage 0 otherwise offers while the player drives Rachel's car
  (0x86347D clear). SMS_LOC_GO_TO_CAR_LOT_NOW (condition 12, stage 0, 2
  normal wins) is her written warning. The first outrun's start (0x607190
  → 0x532E40 → 0x529B60) adds the condition-4 message SMS_OUTRUN_INTRO.
  Kinds 3, 5, 7, 10 are speech only (ported). **Open**: the
  user-remembered "message from another racer" is not in the table (every
  intro message's sender is Rachel, 0x770AF86E; of the 168 messages
  only eight stage 2–5 photo-shoot ones, condition 13, come from others:
  Chuck, Kristian, Scott, Serena, Stefan — so it would be speech, not
  traced); whether the hint counter
  (+4) and queue are saved. Selftest: the reveal 15 s in, three
  complaints closing races and outruns, SMS_OUTRUN_INTRO.
* **Rachel's complaints after stage 0's races: timing, the pause, the
  race exit** (2026-11-01; **verified** in the code and **observed**
  under Wine, sound on: two stage-0 circuits won by teleporting
  through them, a memory log of the hint slots 0x863664, the count
  0x8635C0, the driving time 0x8634A4 and the speech / NIS streams'
  library records, and the WAV). The "angrier" chain is not timed: it is
  one complaint per stage-0 race (0x5242D0 at the settle, kind 1 in 4000
  ticks unless one is queued), Time (4, 1), (4, 4), (4, 8) by the count,
  the third closing stage 0's races and outruns (0x86347E); the written
  one is SMS_LOC_GO_TO_CAR_LOT_NOW (condition 12, stage 0, 2 wins; not
  urgent), filed by the second win's settle (seen: inbox 5 → 6 at
  Return to Explore Mode, entry = table record 65). No reminder comes
  from idling or ignoring the GPS: kinds 2 / 3 (15 s / 40 s of free
  roam) are the only timed intro lines. Observed, both races: kind 1
  posted at UI_PostRace's Return to Explore Mode (before the save
  question), the line 1.0–1.2 s of driving after the city start (wall
  3.8 s after "No", the load included). **The race exit purges the
  streams**: 0x606740 case 0xB (0x5F41F0(0xB), param 0 → 9) calls
  0x481210(9), which for flow modes 3 / 6 / 9 with the speech banks
  loaded (speech object 0x82BA30 +0x14, set by 0x45A7C0; read 1 in game)
  runs 0x736610 and 0x46C9D0 on the NIS stream (+8) and the speech
  stream (+4): 0x7227CB stops each and its queued request is freed. Under
  Wine the ending scene's NIS line (its record +0x14C = 1 through the
  results and UI_PostRace) went to 0 as the exit left. The clone kept
  that line (EndingNis03's, 54 s) playing into free roam, so Rachel was
  "busy" (0x525A00) and the complaint waited for it (`--flow intro_nag`
  before the fix: no complaint in 40 s). **The pause holds the lines**:
  0x4810F0 (the flow's pause, every mode but 0xD SMS, 0x11–0x13) holds
  the music and calls 0x46C8D0: speech and NIS stream objects +0x2C = 1
  and their library rate 0 (0x7230A7(h, 0)); 0x481210's end (0x46C930)
  sets +0x2C = 0 and the rate 0x1000. Observed: a kind-3 line paused 2 s
  in for 6 s ended 24.5 s after it fired against 18.4 s unpaused, silent
  in the WAV while paused and audible after; the driving time stood still
  in the pause (128.362 → 128.454 s over 15 s of wall time) and a slot
  of 40000 ticks fired after 10.09 s of driving, the pause not counted,
  nothing fired by the pause or the resume. Clone: `Streams::hold_speech`
  (`Game::speech_held`: the pause menus, where the music is `Paused`, not
  the SMS screens, World Map or a movie) and `Streams::purge_speech`
  (`Game::explore_after_race`, leaving a race's drive). **Inferred**: a
  new line started while held (none is posted in the pause) and the
  flow modes behind 3 / 6 / 9 other than the race exit. Test: `--flow
  intro_nag` (`NFSU2_AUDIO_WAV=-` for the speech checks).
* **Rachel's hints** (career +0x887C = 0x8635BC, **verified**): 0x5243E0
  (career only) posts timed entries into eight slots {ticks, kind,
  value} at +0xA8 (0x5111F0). Kind 4 once (+9): the car lot's INIT
  (0x4ED9E0 on 0x35F8620B, 0x4EDACB) with ftol(1.0 (0x7993BC) × 4000 +
  0.5) = 4000 ticks; kind 0x11 (or 1, 2, 0x12 while 0x8641F8 is set)
  once (+10): UI_CareerCrib's INIT (0x4AF3A0, 0x4AF502), kinds 6 (4000)
  and 10 (120000). 0x551330 runs 0x551080 only while the script is not
  running, with the driving time; above 0 s it counts every entry down
  by the driving time's step in ticks (0x54BD40) and fires the soonest
  due one when the speech object is free (0x53E0C0): 4 sends
  SMS_LOC_GO_TO_GARAGE (0x51B30CEC), 6 SMS_LOC_EVENTS_OUT_THERE
  (0x83914C51), both through 0x5338A0; the rest are speech only. So the
  garage hint arrives a second of driving after leaving the car lot, the
  events one a second after leaving the garage. Clone: `Profile::hints` (saved with the career; the lines: **Rachel's speech**)
  (selftest: SMS_LOC_GO_TO_GARAGE after the lot).
  **No stage gate** (2026-11-01; **verified** in 0x5243E0,
  0x551330, 0x54BD40, 0x53E0C0 and the loader 0x529240): the gates are
  0x850078 = 1 (career), the once flags +9 / +0xA, and for the count
  0x551330's free-roam state (0x8654A4 = 6, 0x8900AC +0x234 = 0, 0x89E7B0
  = 0, *0x8900D8 = 4, the D-Day script not running). The loader copies
  the slots as saved (it zeroes only +0x1C, +0x108, +0x110, +0x118, none
  of them a gate: +0x110 = 0 means "no reading", step 0). So what the
  saved object holds decides, whatever the stage: the original's finished
  career (the finished-career save: +1, +8, +9, +0xA, +0xC set, no slot queued)
  says no first-career line on Resume, and a stage-6 career whose object
  is all zeros gets the crib's FE_Screen (0x100000, 0x10) and kinds 6 /
  10 (and 4 at the car lot) like a new one. the finished-career save (2026-11-01
  report) is such a file: its content is the clone-written "the finished-career save" of
  2026-10-30 21:17 (a build from before the hints were saved, object all
  zeros; merged into the finished-career save folder on 2026-10-31), so the
  first-career lines it plays are what the original plays with it.
  **Observed** under Wine (2026-11-01, sound on, Career →
  Resume → Return to Explore Mode, reading 0x863664): the finished-career save
  kept all eight slots empty through the garage and a minute of free
  roam; a copy of the player's file had (4000, 6), (120000, 10), (4000, 4)
  queued at load, unchanged by the garage (+0xA set), kinds 6 and 4 gone
  by the spawn and kind 10 about 30 s later — as in the clone (`--flow
  resume`, NFSU2_FLOW_LOAD / NFSU2_FLOW_DATA). Whether each line was
  audible in the WAV was not checked (the slots fired with 0x7FAAA8 = 1).
  Test `a_finished_career_says_no_first_career_hints`.
  **Milestone hints** (0x525400, **verified**, `Hints::milestone`;
  called by the magazine triggers of kinds 3 / 4 — 0x533B50 / 0x533AA0,
  with 0x4FFF80's milestone, whether or not the message exists — and by
  0x5331D0 for every DVD whose rating the car reaches, 0x5332A0): a
  magazine (milestone +0x24 1 / 3) whose cover (+0x28) is not in the
  list +0x2C (count +0xA4; 0x5112F0 looks one up) is added and kind 0xE
  posted in ftol(1.0 × 4000 + 0.5) = 4000 ticks (0x7993BC, 0x784264,
  0x784260); a DVD (2 / 4) not in the list, while +0x28 is set (by
  0x5243E0(2), called at 0x566EEE in a shop's purchase flow; 0x551080
  clears it on each driving step), is added, kind 0xD posted in 4000
  ticks, +0x18 set and the flag +0xD + ftol(+0x3C) set. 0x53E0C0 plays
  both as speech only (0xE: 0x511240's table 0x7FA3F0 → 0x510FC0; 0xD:
  0x511260's table 0x7FA490 → 0x510FE0, then +0x18 cleared): **no offer
  comes through Rachel's hints**. Magazines (milestone +0x34 = 3) are
  offered (state record +4 = 2) by a world challenge's start (0x541130
  → 0x533BF0: the challenge's +0xC milestone; see **World challenges**),
  DVDs (+0x34 = 4) by 0x5331D0; 0x533760 (entering a shop,
  from 0x54CB00: +0x34 0 any shop, 1 the shop whose +0x38 is the
  milestone's +0x38) and 0x5337E0 (a win, from 0x541430: +0x34 2, the
  event's +8 = milestone +0x38, its record's +6 set) offer the others —
  no milestone in the data has +0x34 below 3 (the maintainer's probe `milestones`).
  The photo shoot (0x533690) sets +4 = 2 and +6 = 1 (done); 0x501150 (on
  offer) is +4 = 2 and +6 ≠ 1. The clone keeps the eight slots (0x5111F0
  drops a post when all are used); kind 0xF, 0x525660 and the lines: see
  **Rachel's speech**.
* **GPS** (0x838590; `dday::Gps`, `ai::gps`; **verified** unless
  marked): +0 the target's hash, +0x10 the point, +0x20 / +0x24 the map
  point, +0x28 / +0x29 flags (+0x29 engaged). 0x529490 (engage) on a
  marker entry {trigger, event, shop, milestone, challenge}: a running
  search is dropped first when on (0x4131F0 on the router 0x81CBC0); the
  point is the route trigger's **GPS point** +0x34 / +0x38 / +0x3C (three
  i32, read with fild — a point on the road, not the trigger's position:
  the car lot's trigger stands at (−727, −877, 18), its GPS point is
  (−723, −913, 17); `Trigger::gps`), the map point its +4 / +8; then
  0x413120(player car 0x5F0E40, point) → 0x40C440(car +0x60, the car's
  forward row +0x90, point, mode, 1, owner car): mode 2 for owner kind
  (car +0x14 → +4) 1, 1 for 3 (traffic), else 0 (the player's kind 1 is
  **inferred**; kinds 1 and 2 are the player and racers); +0x29 set; the
  hash: a shop's +0x38, else the event's +8, the milestone's +0x20, the
  challenge's name. 0x5010E0 clears it (router, +0x29, the point, the
  hash). 0x501100 (state): +0x29 clear → 4 (off); else router +4: 1 → 0
  (searching), 2 → 3 (found, +0x28 = 1), else 4; then router +8 (the
  last result) overrides: 3 → 2 ("Unable to find a path", 0x31139B65),
  4 → 1 ("Unable to contact satellite", 0x155409E8). 0x529420: on
  (0x4AE900 / 0x4952B0: states 0–3) and aimed at a point within 0.001
  per axis. 0x532D90(event): when it is the current event (career
  +0x8738): its first goal's shop (0x4FFE20 by goal +0x3C), that shop's
  trigger (0x4FFC50), engaged always for DDAY_EVENT_B, else unless
  0x529420. Callers: the D-Day script (A), the world's start 0x60A690 →
  0x5412E0 (B, in a career with no race: after the car lot the GPS turns
  to the garage). Cleared wholly (router, +0x29, point, hash) by: a world
  challenge's engage (0x498FAC → 0x541130), a milestone's engage
  (0x498FFA → 0x5010E0), entering a shop (0x54CB00, which also sets the
  event status 5), the World Map's Deactivate (0x496CE0), 0x602E8F (an
  online case, **not traced**). **A race's start** (0x541170 from the
  race dialog 0x499B27, the World Map 0x4E1284, debug 0x5416D1) clears
  the router only (0x5411B0 → 0x4131F0): the follower hears code 0, the
  GPS stays engaged (+0x29, target, point) but reads off (4), so the
  World Map offers Activate GPS afterwards (`Gps::drop_route` in
  `start_race`).
  * **Router** (0x81CBC0; `ai::gps::Router`): +0 state (0 idle, 1 start
    nodes, 2 search, 3 done), +4 result (1 running, 2 found, 3 no path,
    4 no start node), +8 the last finished result, +0x10 start, +0x20
    direction, +0x30 goal, +0x40 the start pass's record, +0x44 the open
    list (count +0x16C4, at most 0x9F), +0x2C4 the closed list {point,
    direction, g} (count +0x16C8, at most 0x4FF), +0x16CC the start
    nodes made, +0x16D4 the mode, +0x16E0 the node that reached the goal
    (+0x17A0 set), +0x17A4 the owner. 0x40C440 starts only an idle
    router (state 1, +4 = 1; +8 kept). 0x4131F0: the owner's vt+0x4C(0,
    0) when one is registered, then everything zeroed (+0x40 = −1). The
    world update (0x609D32) runs 0x4268A0 every frame: state 1 → 0x422B90
    and state 2 → 0x422F90 (only with +0x16D0, the start's fifth argument,
    = 1; both callers 0x413120 / 0x4188C0 pass 1) in the same call, each
    stopping once 0.6 ms of real time has passed since the call's start
    (see **Frame budget** below); state 3: found → the node
    copied for the owner, +4 = 2; else a dead node (0x4201D0(0, 0,
    goal, 2)) and +4 = 3 with start nodes made, 4 without; +8 = +4; the
    owner's vt+0x4C(code, &node) with code 1 found, 3 failed; the router
    idle again (+0x17A4 kept). The free-roam graph is the route file
    already loaded for traffic (RoutesFreeRoam.bin) after the free-roam
    post-load (AI route layer; **verified**: from the intro start the
    search then starts backward, g 3436.3 m, as the original — test
    `free_roam_intro_route`; on the raw file it started forward): the race records
    (+0x70 bit 6, +0xC 0) in id order (0x5D9440 → 0x88D218 / 0x88D21C).
  * **Start nodes** (0x422B90): for each record whose box (+0x60..+0x6C,
    0x43D410, no margin) holds the start, its control points from 0 in
    steps of max(1, ⌊distance to the start × 0.02 (0x7851C0)⌋) (_ftol):
    the first with tangent · direction ≠ 0, usable in the mode (0x5D8BC0:
    point flags bit 11 / 25 / 27 for mode 0 / 1 / 2), the start within
    its stretch (along 0..length, across −left..right) with a 5 m margin
    (0x783D3C) and its height within 4 m (0x783D44) gives two nodes, one
    each way (forward = dot > 0 first), each 0x4201D0 then 0x420270; then
    the next record. Done: the open list sorted, state 2.
  * **Node** (0xC0 bytes, pool 0x81CBB8): +0 start point, +4 point, +8
    g (metres), +0xC h, +0x10 goal, +0x20 three successors {point, way,
    junction or 0xFFFF}, count +0xB8, +0x38 the decisions (u16: junction
    & 0x1FFF, 0x2000 taken; at most 0x3F, count +0xB9), +0xBA reached,
    +0xBB start way, +0xBC way (1 forward). 0x4201D0: h = the straight
    x, y distance to the goal (100000 without a point, 0 when reached),
    then the successors. **Extend** 0x420270: from g0 walk point by point
    until a branch: over 10000 m (0x784500) past g0, a point not usable
    or a junction against the way (bit 18 ≠ way) of type 3 → no point;
    the goal on the point's stretch (0x413250: across (0x40CD00: past the
    right +0x16 / left +0x14 half width) < 0, along (0x40CE10: past its
    length ahead, past the previous point's behind) < 0, height within
    4 m) → reached; among the junctions against the way a forced one
    (0x5DCBF0: type 2; mode 2 also 8; mode 1 reads the high nibble) is
    followed (g += (target − point) · the way's direction, 0x40CBC0; the
    way flips with bit 23), traffic ones (0x5DCBA0 7 (mode 0 also 8),
    0x5DCC40 5 / 10) are ignored, any other stops the walk; else g += the
    segment and the next point (0x401CC0). **Successors** 0x41AE40: a
    junction with the way (bit 18 = way) of type 3 → none at all; a
    forced one against it → that one alone (no junction recorded); others
    not ignored → one each; then the next point straight on. **Expand**
    0x41AD40: a copy of the parent, the successor's point and way, g +=
    (successor − parent point) · the parent's direction, the decisions of
    the earlier successors recorded not taken and its own taken.
  * **Search** 0x422F90: take the open list's last (it is kept sorted by
    falling g + h, 0x40C4D0's bubble sort: the least, ties to the later
    one); reached → it is the result; else each successor expanded and
    extended, dropped when an open (same point and way) or closed entry
    has g ≤ it, else taken out of the closed list, put in the open list
    (over an open entry, or appended while < 0x9F) and the list sorted;
    then the node's {point, way, g} closed (< 0x4FF). It ends with a
    result or an empty open list (state 3). Clone: the car lot from the
    intro's start is found in one frame, 138 points, g 3009 m (straight
    2429 m; `selftest`).
  * **Frame budget** (**verified** by trace): 0x4268A0 reads the clock
    once at its entry (0x43BDF0: QueryPerformanceCounter, IAT 0x78311C,
    shifted right by 0x8284E4) and passes it to 0x422B90 and 0x422F90.
    0x43E5C0(t0) = (0x43BDF0's reading now − t0, clamped ≥ 0) × 2^[0x8284E4]
    × [0x828028]; 0x43BD90 sets 0x828028 = 1000.0 (0x784550) /
    QueryPerformanceFrequency (IAT 0x783118) and the shift 0x8284E4 the
    least for which 2^31 shifted counts span more than 60000 ms (0x786D84
    = 2^31, 0x786D80 = 60000.0). So 0x43E5C0 returns the real milliseconds
    since t0, a float. The test (`fadd 0.4; fcomp 1.0; test ah, 0x41; jp`
    exit) stops once elapsed + 0.4 > 1.0, i.e. after 0.6 ms. It sits at the
    top of each loop pass: 0x422BB6 before each race record of the start
    pass (after `record < count`, 0x88D218), 0x422FC3 before each node the
    search takes (after the found flag +0x17A0; the loop runs while the
    open count +0x16C4 > 0), so the step that crosses 0.6 ms completes and
    a start pass that used the time leaves the search no node that frame.
    The calls at 0x422F1F / 0x4232BE only keep the longest step's time in
    0x827ABC / 0x827AC0 (statistics, no effect). The router 0x81CBC0 is the
    only one: the GPS engage (0x529490 → 0x413120) and the follower's
    recalculation (0x42A360 → 0x4188C0, mode by owner kind: player 2,
    traffic 1, else 0) share it, one search at a time. Whether the
    original runs 0x42A360 for AI drivers is **open**; the clone's AI
    never searches (its racers and encounters follow their own route
    walk, `ai::route`, `ai::outrun_search` with a distance budget), so the
    player's GPS is the router's only user.
    **Clone (deliberate choice, `ai::gps::FRAME_WORK`)**: real time makes
    the frame a search ends on depend on the machine and its load (a
    preempted thread is enough: the off-road search of the shot
    `intro_gps_error` took a second frame in about one run in two under
    load), which the fixed 1/60 s tick does not allow. The clone counts
    work units instead, checked at the same two places: one per race
    record, control point tested, point an extension walks, node taken,
    successor made, open or closed entry compared and comparison of the
    open list's sort (where the time goes), 700000 a frame. Measured over
    the GPS destinations (212 route-trigger GPS points) from 9 starts
    (the intro garage, the free-roam start, 6 trigger points, one off-road)
    plus a goal on no road from each, on the networks of career stages
    1–5 (9585 searches; least of five runs per step, the probe
    `search_cost_survey`; per stage in docs/evidence/gps-budget/): 0–1122
    nodes, start pass ≤ 28 µs, whole search p50 127–302 µs, p90
    928–2054 µs, max 4.7 ms (stage 4, 956 nodes, 8 frames of 0.6 ms);
    1797 searches (19 %) take over 1 ms, so a search may not simply finish
    in its first frame. Work: up to 6.76 M units a search, about 850 units
    per µs; the median 0.6 ms slice bought 700 k units (p10 444 k, p90
    906 k). With 700000 a search takes the same number of frames as the
    original's 0.6 ms on those unloaded times in 88 % of the searches
    (±1 frame in 11 %, mean 1.55 frames against 1.57); a node count alone
    matched in at most 69 %. The lists' caps
    bound a step (≤ 3 successors × (an extension walk of at most 10 km +
    0x9F open + 0x4FF closed entries + 0x9F² sort comparisons); the largest
    seen 67842 units, 45 µs), not the number of steps: a node re-enters
    the open list with a lower g, and once the closed list is full (never
    seen: at most 1122 nodes) nothing is closed.
  * **Follower** (driver +0x3E4 state, +0x3F0 the node, +0x400 its goal;
    `ai::gps::Follower`): the router's owner callback 0x41EA30 sets the
    state to the code and copies the node. 0x42A360 (the player's route
    follower): with a state, the car within 30 m across and 4 m in height
    of the node's goal — the GPS point (the node's +0x10 is the router's
    goal +0x30) — drops it (state 0) and clears the router: the GPS reads
    off (4) though +0x29 stays set. In state 1 / 2, unless the car is
    within its look-ahead points' span and lateral room (**not ported**:
    the clone tests every frame), 0x41AF90 walks the route from its start
    point and way along the decisions (forced links taken, a matching
    decision consumed, its 0x2000 taking it; a type-3 link with the way
    goes straight on) to the first point whose stretch holds the car
    within 5 m across and along — none (the goal's stretch reached first)
    → **recalculation**: 0x4188C0 starts a search from the car (+0x60, its centre of mass, with
    +0x90; the clone passed the body origin until 2026-10-31) to +0x400
    (mode by owner kind), which an idle router only takes; the state
    stays 1 meanwhile, so the minimap shows no search group during it;
    found → the look-ahead (0x428030) rebuilt and state 1. **State 3**
    (the last search failed; **verified** 2026-10-29, gps93): 0x42A360
    zeroes the route point +0x3E8 without walking and so falls to the
    same 0x4188C0 — a failed search is started again every frame the
    router is idle (0x40C440 keeps +8, so the GPS reads "Unable to
    contact satellite" / "Unable to find a path" until a search
    succeeds); there is no timer. The clone returned in state 3, so a
    GPS engaged off the road never recovered (`Follower::update`, test
    `a_failed_search_is_tried_again`). **After the car lot** (original
    under Wine, 2026-10-29: Rachel's car driven to CC_CAR_LOT_1, the stock
    206 taken, saved): the city start (0x5404B0 → 0x60A690 → 0x5412E0)
    and the drive-out's end (0x43BB10 type 4 → 0x532E20) engage the GPS to
    CRIB_1 (hash 0xDD60E403, point (649, −168, 15)); ShopExit1 leaves the
    car at (−724.79, −876.51, 18.30) facing −x — the clone's (−724.79,
    −876.51, 18.02) — where no race record's stretch is within 5 m, so the
    minimap reads "Unable to contact satellite" in the original too. The
    router meanwhile stays in state 1 / result 1 / last 4 with its start
    (+0x10) following the car each pass (owner the player's car, mode 2);
    driven south off the lot, the search that started at (−726.8, −897.4,
    18.3) found the garage (state 0, result 2, last 2). Selftest: "GPS
    after the lot's drive-out" (state 1, follower 3, router running) and
    "GPS on the road by the lot" (found).
  * **Minimap** (0x4DA640 → 0x4C8410, minimap element +0x20 0xD80C97E1,
    +0x24 GPS_Search_Group, +0x28 GPS_SEARCH_Text, +0x2C
    GPS_SEARCH_ERROR, +0x30 0x697D2BCA, two Minimap_GPS_Selector.tga
    images; engaged in a career): +0x2C shows with the state's error text
    (states 1, 2), else hidden; the search group shows in states 0–2
    unless the player's (car +0x24 = 1) follower is in state 1 or 2;
    0x697D2BCA at the map point (0x5010C0 → 0x4CB150 → 0x4CAE60: the
    markers' window and turn, within 10 windows while on, pulled to
    0.55; sound 0x458520(0x12) on reaching 3); 0x4C8410 shows +0x30 in
    every state it runs in (state 4 falls to its last branch too). **No
    route line**: neither 0x4DA640 nor its callees (0x4C8410, 0x4CA840,
    0x500400, 0x500B60, 0x50CA00 / 0x50CA50) read the router or a
    follower's route; the router's only readers are 0x501100 and
    0x42A360's family. The marker test (0x4CA6D0 from 0x4CA77B) with the
    GPS on: every marker in stage 0, else only the one at the GPS point.
    Shots: `intro_gps` (found: no group), `intro_gps_error` (an engage
    from a point on no road: "Searching Connection" with "Unable to
    contact satellite"). **Open**: the clone draws the GPS icon below the
    map where the lot's marker sits left of it in `intro_gps` (both from
    near points; the icon's frame offset is not traced).
    The orange triangle on the minimap's left edge (`--shot intro`) is
    not a marker: it is Minimap_North_indicator (0xF2A54430, colour
    0xCF8030 at alpha 0.78), turned by (0x43E6D0(0, 1) − a) + 270 = −a
    (0x4CAB5C..) about the map's centre, so it sits west on a north-up
    map. The car lot's item (CARLOTINDICATOR_n, Minimap_Icon_Safe_House,
    teal 0x52A3A3) is the disc inside the GPS group's blue ring below the
    map: the GPS aims at the lot, so both stand at its pulled-in point
    (corrected 2026-10-30 from Wine captures with FixHUD=0; the earlier
    note took the triangle for the lot).
  * **Minimap items, checked 2026-10-30** (**verified** by REA unless
    marked; Wine: wined3d, FixHUD=0, `$OB_PRIVATE/wine/runs/<run>/shots/h`):
    - **Range** (0x4CA840, 0x4CA9F6..): an item at or past 1 window from
      the centre is hidden; past 10 instead only in free roam (0x89E7B0 =
      0) with the GPS on (0x501100 in 0–3), stage 0 (0x862838) or a world
      challenge running (0x501690). Shown items further than 0.55 are
      pulled in to 0.55 unless flag bit 0 (the race's start / finish items)
      hides them; bit 1 always hides. The clone used 10 in every free roam
      (`Game::map_view`); a career past stage 0 with the GPS off now drops
      markers past one window. No profile: stage taken as 0 (**inferred**).
    - **Turn**: 0x43E6D0(a, b) is atan2(b, a) as a binary angle (0x4000 =
      90°). Every shown item gets 0x43E6D0(dir.y, dir.x) − a, dir from
      0x4AC2B0 (0x43C350 is sine, 0x401100 sine of x + π/2): cars (cases
      3, 4) (cos h, sin h) of car +0x40C, so 90° − h like the player's
      arrow; markers (cases 5–18) (cos θ, sin θ) of the item's +0x14 (0
      for 0x4EC8C0's items), so 90° − a. The clone turned only cars; the
      marker textures are round, so it shows only on non-round icons.
    - **Scripts** (0x4AC620, from 0x4EC8C0 and 0x4CB110 ← 0x533320): an
      entry's item runs "Static" / "Normal" / "Dramatic" by the entry's
      +0x14 (0 shops, milestones, challenges; 1 events). Only
      SHOWCASEINDICATOR_n and AIRACERINDICATOR_n carry those scripts, so
      the photo-shoot / challenge icon takes Static's colour (B, G, R
      25, 195, 210 against INIT's 45, 180, 190); ported (`hud::minimap`).
    - **Per frame** (0x4AC460): event items (types 12–18) take texture
      MINIMAP_ICON_CIRCUIT (0x25559384) with their own colour (0x50CCC0 →
      0x5054D0 → 0x51D800) — the package's indicators already use it — and
      a sponsor event (route trigger +0x34 = 1) 0xB90F50BB with colour
      0x495570(…); an AI item's alpha is 0xFF when its car's record (car
      +0x1C, 0x5EEF70) has byte +0xA = 1, else 0x80. +0xA is **inferred**
      to be the race position (0x403C75 compares two players' +0xA
      against the lap count; 0x4AC062 indexes by +0xA − 1); **open**, not
      ported.
    - **Marker table in the intro**: after the D-Day script (0x861E7C =
      9) the original's table (0x85AD40) still held only the car lot (the
      shop at +8) until hint kind 2 rebuilds it (0x533320, 0x4F92A0); its
      driving-time counter (career +0x8764) read 7.0 s after ~90 s of
      wall time under Wine (slow frames), so the shot's events (Circuit,
      Sprint) are a matter of time, not a mismatch. The World Map later
      listed Car Lot, Circuit, Sprint, as the clone does.
    - **World Map** (UI_InGame_WorldMap, M) matches the capture: legend
      rows, the player's arrow, an opponent car's orange arrow,
      "Deactivate GPS"; with the GPS on no selection ring shows on the lot
      because the GPS target hash is the event's (DDAY_EVENT_A, 0x532D90)
      and 0x4B07B0 / 0x514160 compare the marker's shop hash — the clone
      does the same (`InGameWorldMap::new`).
    - **Ring seams and glyph specks**: side by side at 6× (N and E joins,
      "MPH", the speed digits) the clone's 1-pixel seams and clean glyph
      edges match the original's pixel for pixel. 0x5C5030's stencil path
      (Minimap_Mask 0x40CDC515 / 0x1149C7DA writing stencil INCR, and
      +0x71 'r' / 's' textures tested against it) runs only with 0x8709A0
      set, which 0x5BE534 sets for vendor id 0x102B (Matrox); on other
      GPUs the destination-alpha path the clone ports is the one used.
    - **Tach / HUD position**: with FixHUD=0 the tach and speedometer
      region aligns at offset (0, 0) (best of ±3 × ±6 px); the reported
      ~4 px drop and the ~27 px right shift of the right-hand HUD in the
      older captures came from the widescreen fix's FixHUD=1.
    - **Outrun**: the challenged car's AIRACERINDICATOR_n is listed and
      drawn (`--shot outrun`: the car 20 m ahead sits under the player's
      arrow at this zoom; range 1 in an outrun, race state 7). **Open**:
      0x4EC950 adds AI items only when the HUD is built; the encounter
      racers are in its AI range from the free-roam set-up (see **Minimap
      items**, Outrun racers), so they show while active — not captured
      under Wine.
    - **GPS rings**: the two Minimap_GPS_Selector images (0x4F649313 /
      0x4F649314) pulse out of phase in the original (two captures
      differ); their INIT chains to 0x58878504, a script neither object
      has, and no code or package response found runs PULSE (0x03826A28)
      on them; **open** (the clone draws them still).
  * **MARKER_GPS_AID** (player +0x110, made by 0x5F3E20 with
    MARKER_DIRECTION_AID at +0x10C; **verified**, ported: `dday::Gps::
    update_aid`, `dday::aid_matrix`, `World::gps_aid`): 0x5F3CC0 places
    it only in free roam (0x89E7B0 = 0), for the player (car +0x24 = 1),
    with the chase camera (player +0x238 → +0xC = 1: the camera mover's
    type, which 0x5F1FE0 compares with the type it builds; type 1 is
    0x451660) and the driver's follower (car +0x2C) in state 1 or 2:
    0x5F1650 gives its look-ahead route (driver +0x4B0, with a route at
    +0x3E8); 0x419B20(car +0x60, &s, …, vt+0x1C()) finds the nearest
    node (0x4198C0) and writes s, the distance along it (0x40AE30) —
    vt+0x1C's value is only 0x419B20's last argument; then 0x419D20(out,
    s + 0x40A2A0(), 0, −1, node) takes the point that far along (the
    stretch from 0x40AB10, lerped by the nodes' +0x54 distances,
    0x43D0C0; none past the end). 0x40A2A0: driver +4 → car +0x430 (the
    speed) × 1.5, at least 100 m. Found → drawn by 0x5EACA0 and kept at
    +0x120 (+0x114 set); not found → drawn at the kept point while +0x114;
    any other state clears +0x114. The model: 0x5F3E20 takes the solids
    MARKER_DIRECTION_AID and MARKER_GPS_AID by bin hash (0x43825817,
    0x493600) from GLOBAL/InGameCommon.bun's solid list (MARKER_GPS_AID
    spans x ±0.97, y ±0.34, z −2.59…0.64: its tip on −z). Its matrix
    (0x5EACA0): the camera (player +0x238 → +0x1C) matrix at camera +0 is
    world → view, view x right, y down, z forward; 0x5BAFA0 inverts it
    (transposed rotation, translation −t·Rᵀ; 0x63CED8 multiplies) and
    0x441000 applies a 4×4 to a point (row vectors). h = camera +0xC4 >> 1
    (>> 2 in split screen, mode 4 with 0x83AAB4 = 8); t = 0x43C370(h)·10
    (0x43C370 = 0x43C2B0's sine of h + 0x4000, so cos h; 10.0 at
    0x7A5560); a = −ftol((i16)h · 0.5) (0x7A5564); (s, c) = 0x43C390(a)
    (sine, cosine of a binary angle, 2π/65536 = 9.58738e−5); P = the view
    point (0, s·t, c·t) in the world — ahead of the camera and above the
    view's centre. z = unit(P − point), x = unit((0, 0, 1) × z), y =
    unit(z × x) (0x43D1E0 is a × b, 0x43CF10 normalizes); rows x, y, z, P
    (0x5C5940 draws the solid with it), so the arrow's tip points at the
    look-ahead point. The clone: the solid is a hidden placement in the
    always-drawn section, given the matrix each frame (`intro_gps` shows
    it pointing along the route). Checked 2026-10-29 against the original
    under Wine (memory read of player +0x120 / the follower's node): the
    matrix and model axis were right; the clone's arrow pointed right
    because its route ran south (forward start, g 3009 m) where the
    original's runs north (start way 0, g 3436.3 m, aid point (1725,
    −787)) — the raw route file still held junctions the free-roam
    post-load (0x5E9E90) retypes or drops; with that ported the intro
    shot's arrow matches ($OB_PRIVATE/wine/compare/gps69/
    cmp_after.png). **Approximated**: the look-ahead route
    (0x428030's window along the GPS node) is the clone's polyline of the
    whole route (`Node::points`), the nearest node by plain distance and
    the car projected on its stretch (0x4198C0 / 0x40AE30 bodies not
    traced). **Fixed** (2026-10-29, gps93; issue: no GPS arrow at
    the airport): only the `--shot` render placed the solid; the
    window's draw never did, so in play the arrow stayed hidden. Both now
    take `Game::gps_aid_target` (free roam, no scene, no outrun, follower
    1 / 2) and `gps_aid_transform`. The original under Wine shows it from
    the D-Day script's GPS (player +0x114 = 1, +0x120 = (1725.2, −787.3,
    23.75) about 20 s after Rachel's SMS). Enter does not skip the career
    fly-over in the original (frames every 2 s after Enter: the scene
    went on); the clone's skip is an agreed deviation and ends the scene as
    its own end does. No smoothing: the point is taken again each frame
    (kept only while none is found), the matrix built from it at once.
    **Fixed** (2026-11-01; issue: in the intro at the
    airport the GPS arrow pointed right, the original's left): the clone
    kept free roam's traffic pool, and with it the route network pruned
    by the stage's barriers (0x578070 → 0x5E9E90), across free-roam
    entries, so a new career after a later career's free roam routed over
    that stage's network. Stages 4 and 5 close the way north at the
    airport: the route started forward (south, g 3008.6 m) and the arrow
    pointed right; stages 0–3 start backward (north, g 3436.3 m) as the
    original. The original loads track 4000 on every free-roam entry; the
    clone now loads the pool again when the stage differs (`Traffic::
    stage`). Original under Wine (new career, player +0x120 with the car
    +0x60 / +0x90): (1725.2, −787.3) from (1695.3, −883.6) facing (0.98,
    −0.19), then along the airport road north (5 more samples, test
    `the_intro_aid_points_as_the_original`: the clone's bearing within 1°;
    the selftest plays a stage-5 free roam before the new career).
    **Open**: MARKER_DIRECTION_AID (+0x10C) and its placer.
* **Career car lot** (UICareerCarLot, constructor 0x4F9930 from 0x4FC0B0,
  vtable 0x795CF4, handler 0x4ED9E0; **verified**): no prices — cars are
  taken or traded. The list (0x4DF9C0 in list mode 1) holds the STOCK_<car>
  records of 0x516500 (car types with a geometry path +0x40, bit 1 of
  +0xDA and +0x844 = 0) in car type order (the sort 0x43E650 with the
  always-true 0x40CB20 keeps it), less CIVIC and RSX (0x5022C0: table
  0x7F7C50, as 0x570A40 is non-zero), less cars whose 0x34A1F unlock event
  (+8; 0x513C50) is an unwon event; the first is selected. At a new
  career: PEUGOT, FOCUS, COROLLA, 240SX, PEUGOT106, CORSA. States
  (0x495F00): an open slot without a car → "Do you want this stock %s?"
  (0xDA8F3E96; Yes puts it in the first such slot, 0x541550 → 0x5348F0);
  else one car → "Do you want to trade-in your %s ^for this stock %s?"
  (0x85215BC6, 0x534960 replaces the current slot's car and parts); else
  0x3192F5D1 with "Trade this car?" (0xFB75E78F) and "Browse My Other
  Cars" (0x2699CF8E, also Back): state 5 lists the player's cars
  (0x4DF9C0 mode 4) to trade one. After a deal: the save (state 6) and
  back to the city (0x9A3B571D → 0x5404B0). Back: with CAREER_SLOT_0
  still carless, "Rachel needs her car back. You have to select a new car
  before you leave." (0x272EBEBA), else "Are you sure you wish to exit
  back to Explore Mode?" (0xD76E0B5A). Signing a stage's sponsor opens the
  next slot (0x532D00 → 0x5284E0). The crib's Car Select
  (UICareerCribCarSelect, 0x4F9AD0 on UI_CareerCarSelect.fng, factory
  0x4FC110 in table 0x7F7F6C; **verified**) lists every slot (0x4E0160
  over 0x5162D0 filter 4): a locked one (slot +0x7F4) as a placeholder
  with +0x930 and +0x931 set, an open one without a car (+0x7F5 clear)
  with +0x931 only. The refresh (0x4B0140) writes SLOT_TITLE "Slot %d"
  (0x93475966); a car shows CARBADGE and BOTTOM_GROUP and hides
  SPONSOR_LOCKED_TEXT; a placeholder hides those two and shows "OPEN FOR
  SPONSOR CAR" (0xCA89507E, image 0xDA665928) or "LOCKED" (0xC9F7F7E5,
  image 0xF05704AC). Accept (0x4DFC50): locked → "You'll have to win some
  more races!" (0x2415BE66), open → "Pick up your FREE sponsor car^ at
  the local car lot." (0xCF3B2D70), the car driven → "You're already
  driving your %s!" (0x7EE84E4E), all OK dialogs (0x558580); another car
  → "Do you want to drive your %s instead of your %s?" (0x1231C38F,
  Yes 0xD05FC3A3 selects the slot, 0x500BD0; No stays). The number
  (**verified**) is the screen's +0x5C, "%s %d" (0x790EF4): 1 at the
  constructor (0x4F9B17) with the selection on the list's head (0x4E03A6),
  +1 / −1 per step (0x4B052F / 0x4B0587), so the list position from 1 (the
  slot's number, every slot being listed in order). The placeholder's
  image goes on MANUFACTURERLOGO (**verified**: the screen's +0xBC, found
  by 0x4EE210 with 0x537A70, which also shows it and puts CARBADGE at
  +0xC0; 0x4B0140 sets the image only when the texture exists,
  0x4901D0); a car's logo is 0x511E60(type, 1), its badge 0x511E60(type,
  0). The screen opens on slot 1 (**verified**: 0x4E0160 puts +0x54 on
  the list's head and the constructor does not move it), as the clone
  does.
  Each slot keeps its car's parts
  (slot +0x434 owned, 0x528A00).
* **World Map list** (**verified**): UI_CareerWorldMap opens on stage 1
  (constructor 0x4EE690: +0x224 = 1); the top scroller steps between the
  unlocked stages (0x4E0E50, stopping at the ends; stage record +0xFC,
  0x500C00), each step rebuilding the event list (0x4E0CE0): a region
  entry (0x4E0C30, "UI_CRIB_REGION_%d_COMMENT", table 0x7F4388; **not in
  the clone**), then 0x4B1360 — every event of the stage in career record
  order (stage-0 events with stage 1, the D-Day pair left out, at most 71),
  each with its won flag (0x500B80). Clone: Left / Right the stage, Up /
  Down the event. Accept (**verified**; 0x0C407210 or 0x406415E3 in the
  handler 0x4E0F60, vtable 0x795D14) on any event — the list is filtered
  by neither lock nor won flag; only the region entry at position 1
  (0x51F7F0 on the list +0x138) is skipped — asks "Would you like to race
  this event?" (0xEC9488BB, 0x5587A0); Yes 0x51123592 races the item's
  event (data +0x34, made by 0x4B1200; 0x541170 / 0x5325F0 on 0xC98356BA),
  No 0xB4623F67 stays; Up / Down are 0x72619778 / 0x911C0A4B (0x4CC140),
  Left / Right 0x9120409E / 0xB5971BF1 (0x4E0E50).

## Race types: Restart and Next Race keep the field (verified; seen under Wine)

* **The track switch** (**verified**, assembly): the flow's state-0 handler 0x6037D0 stores
  messages 9, 10 and 0x10 in 0x7FAA08 and starts the switch (0x581A00, "BeginGameFlowSwitchTrack" →
  0x581830). Restart is message 9 (the pause's and UI_PostRace's OK 0xE1A57D51: 0x5F41F0(9) at
  0x4F3BA6, also 0x4BFFDF, 0x4E9A75, 0x5DEBC9); Next Race is 10 (0x4BEC90: 0x54CAF0 → 0x541590 moves the
  current race record 0x85EF48 on by 0xD24, then 0x5F41F0(10)). Restart does not move the record.
* 0x581830: a dyno run (0x89E7E6) runs 0x526980; a career (0x850078 = 1) with an event (0x863470) runs
  Race_SetupFromEvent 0x5322F0(event, 0x7FAA08 == 0x10) (0x58192E); a career without one the free-roam
  set-up 0x540790; any other game mode (Quick Race, 2) no set-up at all.
* 0x5322F0 always sets the race globals (the difficulty 0x89E7F4; the track 0x89E7A0, variant 0x89E7A4
  and laps 0x89E7BC from the current record [0x85EF48]; the race-type flags; 0x89E804). Only with its
  second argument (0x53245A..0x532529) does it run the player's racer (0x528610, 0x6373F0), 0x601610,
  the line-up with the rivals' looks and performance (0x5260C0), the traffic pool (0x4FE9F0) and the grid
  slots (0x5264B0, racer info +0x03). So Restart and Next Race keep the line-up, the looks, the
  performance and the grid slots; Restart races the event's current race again, Next Race the next one.
  A Quick Race's set-up (0x53FC30 → 0x53EEA0) runs only from its menus, so its Restart keeps all of it.
* **Seen under Wine** (fastgpu.sh; the racer list [0x89E810], 0x940 bytes a racer, count 0x89E814): a
  Quick Race circuit (track 4014, three opponents) restarted three times from the pause: the four
  racers' records identical but for one byte (+0x8F8), grid slots 2 / 1 / 3 / 4 each time, the same cars
  and colours on the grid; a stage-1 career circuit started from the World Map (track 4013) restarted
  three times over two starts: identical the same way, 0x7FAA08 = 9 each time; the same event started
  again from the World Map (no restart) drew a new line-up (other cars, the player on slot 2 instead
  of 3). Evidence: docs/evidence/perf-budget/restart-line-up.txt.
* **Next Race seen under Wine** (fastgpu.sh, the `stage5_win1` save, stage 5's first URL tournament
  S5_URL_1, event +0x08 = 0xF874A549, three races of three laps): race 1 on track 4712 started with
  0x7FAA08 = 0x10 and the race record 0x85EF48 = 0x85BAB8; after the race's results and the standings
  the Post Race menu read Next Race, Return to Explore Mode, Restart, Race Stats, Replay, Quit to Main
  Menu; Next Race posted message 10 (0x7FAA08 = 0xA), moved the record to 0x85C7DC (+0xD24) and started
  track 4714 with the six racer records [0x89E810] identical to race 1's but for +0x8F8, grid slots
  (+0x03) 6, 2, 3, 4, 5, 1 both times (and so after a Restart of race 1 before it). To finish race 1
  the harness put the player's rigid body (+0x20..+0xE0) on the last rival's of 0.2 s before, every
  20 ms, so the race's own lap count and finish ran; nothing of the field was written. A Quick Race URL
  race is a single race in the original too: its Post Race menu has no Next Race. Evidence:
  docs/evidence/perf-budget/restart-line-up.txt. Test
  `tests::race_again::next_race_takes_the_url_tournament_to_its_second_track_with_its_field`.
* **Open**: whether a restarted tournament race's own record (+0x14 state) counts a finish from before
  the restart in the points (0x513EB0) is not traced.
* Clone: `Field::looks` and `Field::slots` (kept by `line_up_rivals`), `RaceFlow::restart`
  (`start_race_from`), docs/native-fixes/race-again.tsv. The race's rivals' cars are taken again by the
  set-up for the same load keys (`Loads::raced_cars`; docs/perf-budget.md "A race raced again").

## Hidden races, engage tips and cash pickups (2026-10-30; checked against the original under Wine)

**Verified** (REA decompiles of SPEED2.EXE) unless marked. Checked in the
original (Wine copy, new career at the airport, memory read and written
with /proc/pid/mem): the World Map, a tip and two pickups (below).

* **Hidden races** (event +0x7F ≠ 0: the 32 `S<n>_H_*` events and
  STAGE_0_CIRCUIT_0). Their marker is in the marker table like any open
  event (0x533320 does not test +0x7F) and in the world (the trigger, the
  Engage key, its XEE_EVENT_* effect from 0x605790), but never on a map:
  0x532C30 (the event test of 0x533120) is false for +0x7F ≠ 0; else the
  event must be in the open list (career +0x71C8, 0x5007E0 by +0x08) and,
  for an SUV event (+0x0D = 1), a career slot must hold an SUV of the
  player's own (0x5278A0: slot +0x7F5 and car type +0x88A). 0x533120 (a
  marker entry: shop → 0x513110's tests, event → 0x532C30, milestone →
  0x501150, challenge → 0x5135A0) is the filter of the in-game World Map
  (0x4E0512), its legend (0x4EE4A9) and the minimap's marker test
  (0x4CA7C0); the minimap's item builder 0x4EC8C0 drops failing events
  before an item is made, so with the GPS on (0x4CA6D0's stage-0 / GPS
  target branch) they are not shown either. Nothing changes +0x7F: being
  found, an SMS (ENGAGE_TIPS_HIDDEN only pays its tip) or a win do not put
  them on a map; a won event leaves the open list. The front-end World
  Map list (UI_CareerWorldMap, 0x4B1360) does not test +0x7F (a track
  list, not a map). **Under Wine**: at the intro the open list held
  CA207584 (circuit), 1BCB7AB0 (sprint) and STAGE_0_CIRCUIT_0 (CA207583,
  +0x7F = 1), all three in the marker table; the World Map showed Car Lot,
  Circuit and Sprint with one circuit icon, none at STAGE_0_CIRCUIT_0's
  marker (353, −485). (The original opens those stage-0 events during the
  D-Day intro; the clone's `refresh_open` opens none then: a difference
  outside this work, **open**.) Clone: `Game::event_on_map` filters
  `world_entries` (minimap) and `map_icons` (World Map and legend).
* **Engage tips** (the in-world SMS markers): route triggers of kind 0x12
  (56 in ROUTESL4RA's 0x3414A; boxes at z 0), each named by a
  condition-15 message (+0x08; 42 messages ENGAGE_TIPS_* and
  ENGAGE_TIP_CAREER_PROG, mailbox 2 Game Tips, icon 9, +0x0C the cash: 25,
  or 50 for FILTER, HIDDEN, TRLBRK, ORGANIC; CAREER_PROG is urgent). There
  is no stage gate: every tip whose message is unsent is live from the
  start of a career. The player update 0x5EB050 (career free roam, no
  replay, no world challenge 0x501690) each frame takes the first kind-0x12
  trigger holding the car (0x5D9C00(car +0x60, 0x12, 0)) to 0x532D50 →
  0x529C50 (see SMS): the message arrives once (0x514CB0 finds it in the
  inbox afterwards; 0x529600 adds it, unread, new flag set), its +0x0C
  goes to the bank (career +0x7134) and 0x528990 relists the tips at
  career +0x600 (128 pointers): each kind-0x12 trigger whose message
  exists and is unsent (0x514E10). That list is only read by the marker
  effects (0x605790 kind 1, XEE_GAME_TIP: the nearest listed trigger under
  200 m, at its +4 / +8 / +0x14, z 0 → the ground). No map shows them (the
  minimap and World Map take only the marker table, 0x500EE0). Their
  one-time state is the inbox itself (saved with it). **Under Wine**: 42
  tips listed at the intro (one per message, Rachel's SMS being no tip);
  the car moved into ENGAGE_TIPS_DRAFT's box (1711, −986): bank 0 → 25,
  inbox 1 → 2 at once, the list 41, nothing more while staying inside; no
  minimap icon near the tips. The look of XEE_GAME_TIP was not made out in
  the lavapipe shots (**not confirmed visually**). Clone:
  `Profile::sms_tip` (existing), `Profile::tip_pending`,
  `Game::marker_effects` (kind 1).
* **Cash pickups** ("banks"): career chunk 0x34A1E (59 records of 12
  bytes at 0x838430, count 0x838434, loaded at 0x512D82): +0x00 (u16) the
  cash, +0x02 the condition (1: the stage 0x862838 above +0x04; 2: the
  visit-table entry +0x04 in state 1, 0x500B80; else always), +0x03 the
  flag index, +0x08 the name of its kind-0x13 route trigger (59, 8 × 8 m
  boxes). The data: two $50 always (flags 0, 1), $50 from stage 1 (flags
  2–14, +0x04 = 0) and stage 2 (15–28), $100 from stage 3 (29–36), $200
  from stage 4 (37–48), $400 from stage 5 (49–58); no condition 2 in the
  data. 0x5EB050 (same gates as the tips) takes the first kind-0x13
  trigger holding the car to 0x5283D0: the first record naming it, if
  0x513890 (the condition, and flag career +0x8768 + index clear) →
  0x528380: bank += cash, flag = 1, 0x5141B0 relists the open pickups at
  career +0x800 (256 pointers, same test), and with a player (0x8900AC)
  0x4AC1B0 on its HUD (+0x9A4): the bank element (HUD +0x80, 0x4F4F10:
  Cash_Bank, Cash_data, Cash_header, Show_case_backing; mask high
  0x80000) gets +0x28 = the cash, and PlayFESound 0x11 (0x458520:
  FE_COMMON's CommonSample 17). The element's update 0x4A9EF0 (from
  0x4EC5A0), while shown (mask, and no challenge clock 0x501680), prints
  "%s %$d" with 0x5D10F6AF and the cash on Cash_Bank, runs Cash_Animate
  on it, clears +0x28 (+0x2C = 1) and writes the bank on Cash_data
  (0x500F90). The XEE_BANK effect (0x605790 kind 2) stands at the nearest
  listed pickup under 200 m; no map shows them. The flags (career
  +0x8768, 256 bytes, 0x8634A8) are saved by 0x514530 (0x514685: 0x40
  dwords) at payload `native_save::BANK_FLAGS` = career block + 0xC30
  (between the drive time and the inbox) and loaded by 0x52938E; a new
  career clears them (0x532A21). **Under Wine**: at the intro the list
  held exactly the two always pickups (flags 1, 0 at (501, −808) and
  (−690, −890)); the car moved into each: bank 25 → 75 → 125, flags
  0x8634A8 = 01 01, the list emptied, and the HUD showed "Bank Reward: 50"
  under the Bank box (the label carries the colon). The pickup's effect
  was not made out in the shots, the sound not heard (headless, muted):
  both **not confirmed** in the original beyond the code. Clone:
  `career::Bank`, `RouteTriggers::bank_at`, `Profile::{bank_open,
  collect_bank, banks}`, `Hud::bank_reward`, `Game::fe_sample`.
* Also found: the World Map legend's show flags (career +0x8869, 16
  bytes, 0x8635A9) are saved right after the inbox (0x5146A9;
  `native_save::LEGEND_SHOWN`); the clone saves them (`Profile::map_hidden`).
* **Second pass (2026-10-30, markers103; side by side with the original
  under Wine, wined3d on Xvfb, car moved by writing every copy of its x, y
  in memory, the D-Day stage-0 career).** Same pose in both
  (`NFSU2_SHOT_AT=x,y,z,deg` on `--shot marker`, `NFSU2_SHOT_HUD_MS=n`
  the HUD n ms after the update that started its scripts; examples
  `marker_spots` and `marker_effects` list the triggers and XEE groups):
  * Hidden race: at (317.5, −535) heading 131° the original shows
    STAGE_0_CIRCUIT_0's XEE_EVENT_CIRCUIT icon (slot 0 of 0x890020 held
    at d² 17095) and no minimap item for it, while STAGE_0_CIRCUIT_1 and
    STAGE_0_SPRINT_0 are on the minimap; the clone matches.
  * Engage tip: ENGAGE_TIPS at (1713, −846) seen from 30 m (slot 10,
    XEE_GAME_TIP: ICON_INFO_SIGN, a red "i", and its ground ring); the
    clone's icon stands at the same place and size.
  * Cash pickup (501, −808), driven through: bank 0 → 50, flag byte 1 of
    0x8634A8 set, the bank list (career +0x800) down to one, and "Bank
    Reward: 50" right-aligned under the Bank box. **Fixed**: the clone
    drew that text at the screen's middle, because no script ran on
    Cash_Bank. 0x4A9EF0 starts it by name through 0x51CF80 (object,
    name, restart), which finds the object's script (list +0x4C, hash
    +0x30) by FEHashUpper (0x505450: the bin hash with a–z folded to
    A–Z), and the packages store upper-case names (CASH_ANIMATE
    0x6287657C, not "Cash_Animate" 0x6BF88E9C). The clone's
    `ScreenState::run` hashed the name as given, so every mixed-case
    name (Cash_Animate, ZoomIn, FadeIn, Highlight, UnHighlight, Blink,
    Init, Pulse*, Show, FadeOut) silently ran nothing; it now hashes
    upper-case, as do the HUD's script-length lookup and the two
    menus' script tests. CASH_ANIMATE (2.7 s) fades the text in at
    (283, −163) and out from 2.5 s. Other visible effects of the fix:
    the drag ShiftMessage stays up for its ZoomIn (1.8 s) instead of
    vanishing at once, the shift light pulses, the magazine cash popup
    zooms in and fades, the showcase preview's options un-highlight, the
    dyno chart's group runs FadeIn.
  * Not markers, seen on the way: the long white searchlight beams in
    the sky are the world animations of `XO_VISTA_SPOTLIGHT_1A_DW_00`
    (see the self-playing world animations above), drawn by the clone
    since world106; side by side at (−775, −775) they stand where the
    original's do, with the same additive look
    (`$OB_PRIVATE/wine/compare/world106/C-beam-crop.png`). The car
    lot's own XO_SHOPBEAM_CARLOT_1A / 1B are 10 m cones that point down
    onto the lot.
* Tests: `tests/native_save.rs` `cash_pickups_pay_once_and_save_their_flag`;
  selftest `selftest_markers` (tip once with its effect, a pickup once with
  its effect, HUD and save round trip, stage 2's hidden races on neither
  map while the others are on both).

## Post Race and tournament standings (2026-10-31; verified)
* **UI_postrace's remembered option**: the menu is built (0x4D71CB →
  0x4BEE10) and then opens on its screen record's +0x10 (0x50B8A0 → the
  scroller's vt+0x20, 0x538A40: n > 0 selects option n − 1), which the
  scroller handler writes on each move (0x543F47 / 0x543F99 → 0x50B8C0)
  and only Quit to Main Menu resets (0x4A3E80 → 0x50B8C0(0)).
  **Observed**: after a Restart from Post Race, the next Post Race opened
  on Restart. Clone: `Game::post_race_selection`.
* **Standings ties** (0x4C0953): the tournament's racers are added in the
  record's order (0x85BA64 +0x08, 12 bytes each; under Wine the AI racers
  100… first, the player 0 last), each add followed by 0x43E650 with the
  compare 0x4A4730 (swap only when the later has more points), so ties
  keep the record's order — S3_URL_4 under Wine: Gabriel and Andrew on 10
  listed Gabriel (racer 100) first. Clone: the standings start from the AI
  racers in order, then the player.
* **The last race's after-movie**: 0x4A3E30 → 0x54C9E0 → 0x513A50 picks
  "Scene15" (S5_CIRCUIT_36 won; the event has no +0x04 movie) and 0x53D7B0
  queues it (mode 4) before the save questions; seen in memory at
  0x8382A0 under Wine.

## Cheat codes (2026-10-31, cheats; traced in SPEED2.EXE and checked under Wine)

Wine: `$OB_PRIVATE/wine/runs/<run>` through `fastgpu.sh` (:151, profile NAME,
no WidescreenFix), 1280×960, memory read with `peek.py`, codes typed with
`xdotool type` on the title screen. DXVK lost its device (submission
-13) on the way into the main menu in three runs, with and without codes,
so the comparison runs used `--wined3d` (wined3d's Vulkan renderer).
Images in `<work dir>/cheats-tmp` (`w_*.png`, `all_list.png`,
`sp_list.png`, `vg_all.png`, `w2_*.png`).

* **Input** (**verified**): the manager is the global 0x865930 (built at
  start-up, 0x77DCA0 → 0x575780). UG_LS_Splash, the "Press Enter Key"
  title screen, turns its input on in its constructor (0x4A90F0 →
  0x579CA0 at 0x4A9251) and off in its destructor (0x4A92B0 → 0x579CB0),
  so codes are typed only there (memory: +0x04 is 1 on the title, 0 once
  the attract movie or the profile step takes over). WM_CHAR (the window
  procedure's case 0x102, 0x5CCD7B) calls 0x55DBE0, which gives the
  character to the front end and then, with the input on, to 0x57B8C0:
  the byte goes into a 32-byte ring (+0x14, index = count & 31) and the
  count (+0x34) steps; every code no longer than the count is compared
  with the ring's last characters, a–z without case. A match runs
  0x579CC0. There is no time limit; the ring just keeps the last 32
  characters. The title screen's own idle time-out (the attract movie)
  ends the input; the codes typed before stay.
* **The table** (0x7FB960, 20 entries of 0x34, **verified** in the file
  and in memory): +0x00 the code, +0x20 kind, +0x24 value, +0x28 group,
  +0x2C a value copied to the manager's +0x08 (all 0; nothing reads +0x05
  / +0x08), +0x30 active, +0x31 keep, +0x32 enabled. Entries 0–3 are file
  data; 0x575780 writes 4–19.

  | Code | Kind, value | Effect |
  |---|---|---|
  | needperformance1 / needperformance2 | 2, 1 / 2, 2 | performance level 1 / 2 in main-menu Customize (0x579DA0 in 0x529E40 and 0x52A120, shop flag 1) |
  | gimmevisual1 / gimmevisual2 | 1, 2 / 1, 2 | both value 2: level-2 visual parts in Customize (0x579D90 in 0x529D70) |
  | gimmechingy, wannacapone, wantmyd3, shinestreetbright, davidchoeart, tunejapantuning, yodogg, opendoors | 6, hash of SPONSOR_CHINGY / _CAPONE / _D3 / _SHINESTREET / _DAVIDCHOE / _JAPANTUNING / _SNOOP_DOGG / _THE_DOORS | that sponsor car in Quick Race (0x579D80 in 0x529F80) |
  | needmybestbuy, goforoldspice, gottaedge, gottahavebk, gotmycingular | 7, hash of AD_BESTBUY / _OLDSPICE / _EDGE / _BURGERKING / _CINGULAR | that vinyl (category 0x3D, the Sponsor group) in a career's shops (0x515500 flag 4) and in Customize (0x529D70) |
  | regmybank | 5, 2 | $200 bonus (0x865128 = 200) |
  | regmebaby | 5, 3 | the RX-8 and Skyline in Quick Race (0x579D40: car types 0x15 / 0x1B in 0x529F80) and the contest vinyls (category 0x44, group Contest) everywhere (0x579D70 in 0x515360) |
  | ordermebaby | 5, 4 | $1000 bonus (0x86512C = 1000) |

  needmybestbuy's enabled byte is cleared by 0x575780 when the region
  0x7FA73C is not 0 (6 in this file; read 0 in memory), so in this build it
  does nothing — and the vinyl browser drops AD_BESTBUY in that region
  anyway. Kind 5 value 0 (0x8383C8 = 1) has no code.
* **On a match** (0x579CC0, **verified**): only an enabled entry acts. The
  first time it plays the menu's SELECT sound (0x51DE70(_, 8): column 0,
  FE sound 4); entries of the same non-zero group would turn off (0x576060;
  every group is 0); the entry turns active; a kind-5 entry runs 0x5760B0;
  the manager's +0x05 = 1 and the count 0. No message shows. 0x5760B0 sets
  the bonus amount and, when the career in memory has not played its
  intro (0x86347C = 0) and its bank (0x861E74) is 0, adds it at once
  (seen: 200 after regmybank then ordermebaby on the title, the second
  not paid as the bank was no longer 0). A new career's cash is the two
  amounts' sum (0x532918 → 0x5760A0, career +0x7134).
* **Lasting** (**verified**): nothing is saved. The active flags (+0x30)
  and the bonuses are only in memory and stay until the game quits:
  0x575FE0 (from 0x5817C4) clears only entries without the keep byte
  (+0x31), and every entry has it.
* **Sponsor cars** (**verified**): the garage 0x83AD90 holds eight records
  at +0x9A84 (0x5163C0 over the names at 0x7F7C6C: CHINGY, CAPONE, D3,
  SHINESTREET, DAVIDCHOE, JAPANTUNING, SNOOP_DOGG, THE_DOORS), pointed to
  by slots 73–80 of the 91 at +0x9BD8: id hash("SPONSOR_<name>"), +0x10 1,
  +0x14 8 (the SPONSOR CARS filter's bit), +0x18 the preset ride's name
  hash (GLOBALB 0x30220: Navigator CHINGY, Hummer CAPONE, Eclipse D3,
  IS300 SHINESTREET, Corolla DAVIDCHOE, G35 JAPANTUNING, Escalade
  SNOOP_DOGG, GTO THE_DOORS). Quick Race's 0x529F80 finds the id in that
  table (0x84481C, 12 slots) and returns its code's flag (0x579D80).
  Under Wine with every code: the trigger stepped All → Stock → SPONSOR
  CARS (My Cars and Career skipped), which listed Navigator, H2, Eclipse,
  IS 300, Corolla, G35, Escalade, GTO in that order, each on its preset
  (the Chingy Navigator: white with the Chingy vinyl, red neon), the help
  bar Back / Continue / Quit Game (no Customize). All Cars gained the RX-8
  and the Skyline: 206, Focus, Corolla, 240SX, 106, Corsa, Escalade, RX-8,
  350Z, GTO, Skyline, Lancer Evolution VIII. Main-menu Customize's car
  select offers only All / Stock.
* **Vinyls** (**verified** under Wine): with every code, Customize's vinyl
  groups showed Racing Set, Racing, Tribal Set, Tribal, Hoods, After Market,
  Body, Contest and Sponsor open; Sponsor listed four (Burger King first;
  no Best Buy). With only gottaedge and regmybank, Sponsor listed only Edge
  and Contest was locked.
* **Clone**: `frontend::cheats` (the table, the ring, the queries, the
  game's flags `cheats::ACTIVE`); the title screen feeds typed text to it
  (`Game::cheat_char`, from winit's key text while `Menu::Splash` is up)
  with the SELECT sound; `Profile::part_unlocked` (0x44), `part_listed`
  (0x3D by hash, the visual level in Customize), `Game::perf_unlocked`
  (the performance level in Customize), `Profile::quick_race_cars` (RX-8,
  Skyline), `Game::sponsor_cars` (filter 8, the car shown and raced as its
  preset), `career::start_cash`. Shots: `qrsponsor` (the SPONSOR CARS list),
  `vinyls_sponsor` (the Sponsor group; `NFSU2_CAT=26` Contest);
  `NFSU2_CHEATS=all|code,code` types codes for any shot. **Inferred**: C
  (Customize) does nothing on a sponsor car (the help bar drops it).
  **2026-11-04 (`docs/cheats.md`)**: a new profile's bank depends on
  MC_Main's question — the boot's create (no profile current) keeps the
  career in memory (Wine: saved with 200 after regmybank + ordermebaby),
  the Profile Manager's create over a current profile resets it (1200);
  the clone now follows (`cheats::creation_cash`, `Game::boot_cash`), and
  gimmevisual2 alone opens nothing (0x576010 reads the first entry).
  **Open**: the
  rating bars of a sponsor car (the original drew them almost empty) are
  not compared in detail.

## Race lines and the post-race exit (2026-11-01)

Issue: the spectators' shouting could still play after the race.

The race's crowd, starter and flag-girl voices are not AEMS events (no flare,
starter or countdown sound exists in the HUD or scene code): they are the NIS
stream lines, the start scene's (NisAudio_Request 0x484510 kind 2, the starter
and the crowd, 8–18 s) and the ending scene's (kind 6, the crowd at the line;
the race endings' records of entry 0x4024 last 48–54 s). Both play on the
speech object's (0x82BA30, vtable 0x789D00) NIS stream, +8; Rachel's lines on
its speech stream, +4.

* **When they stop** (**verified** in the code; every direct caller of the
  stream stop 0x7227CB, of 0x46C9D0 and of 0x482BC0 checked):
  * a new NIS request purges a busy line first (0x484510 at 0x484739);
  * **0x481210 with 3, 6 or 9** (the speech object's +0x14 set, i.e. its
    streams up, 0x45A7C0): 0x46C9D0 on +8 and on +4 (stop, queued request
    freed), then 0x46C930. Its callers with those values: **UI_PostRace's
    destructor** 0x4E9540 (vtable 0x797A78, built by 0x4F3940 → 0x4E94C0;
    slot 2 is UI_PostRace_Notify 0x4F39A0) with 3 — every way out of the
    post-race menu; the PAUSE state's handler 0x606740 on message 8 with
    the leave reason (0x5F4250), and on message 0xB with 9 (0 → 9);
  * the leave reasons UI_Pause gives (0x4C2170): Resume / Back 1, Restart
    (0xE1A57D51) 3, 0x30F32A49 4; Quit to Main Menu OK (0x4A7CF0), Back to
    World Map (0x4A7CC0) and Jump to Garage (0x4A7C70) 6; Return to
    Explore Mode (0x451E768E) is flow message 0xB (→ 9). 1 and 2 only
    release the streams (0x46C930); 4 neither purges nor releases (**not
    ported**: the lines stay held until the next release);
  * GB_NOS's end (0x484F20, kind 1 only); the end of the line's data.
  Race end (0x606CC0, 0x60AAB0), the ending's load and the results do not
  touch the streams: the ending's line plays through Event Results and Post
  Race. No volume fade: the stop is immediate.
* **Pause** (**verified**): 0x4810F0 (the flow entering PAUSE, 0x60B3B0
  messages 7 / 0xD / 0x11–0x13; replay control; focus loss) holds the music
  (state 5) and both streams (0x46C8D0: rate 0, +0x2C = 1), and the music
  manager's +0x13 = 1 keeps them held each frame (0x46CB00); 0x481210
  releases them (0x46C930: rate 0x1000).
* **Observed under Wine** (fastgpu with `--audio-file`, Quick
  Race Drag Bayview Bridge driven by a bot; the line found in the WAV by
  cross-correlation with the clone's decode of each 0x4024 record, with a
  ±100 ms lag search per 0.5 s window; the speech object read live):
  EndingNis02 / 03's line starts ~6 s after the finish (stream kind 6,
  volume 52) and plays through Event Results and Post Race; left to run it
  plays to its end (51.5 s) over Post Race; **Restart** OK from Post Race
  and **Quit to Main Menu** OK from Post Race each stop it within one
  0.5 s window (≤ 0.3 s after the key, the capture's ~130 ms latency
  included). The race's pause sets both streams' +0x2C (held) and Resume
  clears it. An earlier "stop" 22 s into the line was a 0.6 ms jump of
  the stream's position (the correlation came back at the new lag).
  Escape during a start scene skips the scene (no pause there).
* **Clone** (`lines_purged_by`, `Streams::purge_lines`, `Game::lines_held`,
  `Streams::hold_lines`): the post-race menu's exits (Return to Explore /
  Free Run, Restart, Back to World Map, Quit to Main Menu, Next Race) and
  the race / explore pause's Restart, Quit, World Map, Jump to Garage and
  Return to Explore purge the NIS and speech lines; the pause menus (and a
  dialog over them) hold them. The clone had neither: a race's ending line
  went on for up to ~50 s into free roam, the front end or the restarted
  race. **Open**: whether Race Stats / Replay from Post Race close
  UI_PostRace (the clone keeps the line there); leave reason 4.
* **Test**: `selftest_race_lines_end` (in `--selftest`; alone `--flow
  racelines`): a new career's first race (stage 0) won and left through Post
  Race by Enter, then Esc / Resume three times — the ending's line gone in
  free roam and not back after any resume; the start line held in the
  pause; gone 3 s after the pause's Restart, Quit to Main Menu and Return
  to Explore, and after Post Race's Return to Explore, Restart and Quit to
  Main Menu.
