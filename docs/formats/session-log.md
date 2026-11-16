# Session log (dated)

Part of [docs/formats.md](../formats.md), the traced behaviour of `SPEED2.EXE`; the tags
(**verified**, **inferred**, **open**) are explained there.

Comparison rounds and session reports, moved here from the subject sections on
2026-11-05 unchanged and in their original (chronological) order. Their facts
and addresses are the record; the index in docs/formats.md lists them with the
subject chapters and by address. New session-style notes go to the matching
subject chapter, not here.

## Front-end projection under Wine; comparison aids (2026-10-29)

* **The Wine captures' front end is squashed vertically (inferred: a
  capture artifact, not the game's look).** In t1.trace (Quick Race car
  select and main menu) the view's projection has m00 = 3.15343 and
  m11 = 3.84335 (identity-world draws, e.g. call 10639836), the eye at
  (0, −4, 1.3997) as the clone's. 0x5C7ED0 (Camera_SetProjection, the
  only caller 0x5D2860 at 0x5D2982) gives views 1, 2, 4, 5 x angle
  `ftol(fov16·1.1·0.5)` (0x7A27E4, 0.5 0x784260) and y angle
  `ftol(fov16·(1.1/0.869565)·(h/w)·0.5)` (0x7A27E8 / 0x7A27E0), h, w the
  view's target record +0x18 / +0x14 (records 0x86E768, stride 0x1C,
  built by 0x5BA220; record 0 from 0x870980 / 0x870984). The x angle
  gives fov16 = 5826 (32°, SHOTS[0]); the y angle then needs h/w =
  0.7206, not 0.75 — e.g. a 1274 × 918 client rectangle. 0x5B9A70 reads
  the window's GetWindowRect (0x783388) / GetClientRect (0x783384) in
  windowed mode (the captures run windowed, CreateDevice Windowed =
  TRUE), while 0x5D2623 sets 0x870980/4 from the resolution table
  0x5BF620 (1280 × 960 for index 3). In game (t2.trace) the ratio is 0.75
  exactly. The clone keeps h/w = 0.75 (full screen); for side-by-sides
  with the t1 captures use `NFS_FE_HW=0.72057`, which reproduces the
  trace's matrix to 5 digits.
* **Comparison aids** (game `--shot`, not game behaviour): `NFS_NO_UI=1`
  leaves out the front end's and HUD quads (the apitrace pre-post
  snapshots have none); `NFS_FE_HW=h/w` as above; `NFS_REFLECTION_PNG`
  (Road reflections). Side-by-sides: $OB_PRIVATE/wine/compare/fx70/.
* **Quick Race car select after fx70** (pre-post, `NFS_NO_UI=1
  NFS_FE_HW=0.72057`, against orig-qrcars-prepost.png): floor and ring
  within 3 levels; remaining: the steam on the left (ported in fe72, see
  "Front-end particles"), and the disc's wet-look light streaks are
  weaker in the clone. Main menu (final, against shots/t1/main.png): floor,
  backdrop, body and ring within 2–7 levels; the 350Z's brake discs
  behind the rims show in the original and not in the clone (fixed in
  fe72: the performance brakes' visual part, see "Brakes from the
  performance parts").
* **Sky remap: resolved.** 0x870D14 is 1 (the options object's
  static constructor), so the remap takes the table's pair at the index:
  sunrise in the first minute after a track load, then the cycle. See Sky,
  "Shipped behaviour".

## Session (2026-10-29): dialogs, race start scene, scene lens, turbo

* **Empty New Career dialogs** (player report "empty sms pops up";
  **fixed**). The FEng clock a screen hands `layout` is the time since the
  screen opened (`now - opened`), and every script start time in its
  `ScreenState` is on that clock. MC_Main (Save/Load: "Would you like to
  create a new profile?", the name editor, "Are you sure you want to save
  the profile named %s?"), the generic dialogs, the SMS mailbox, the
  options rows, the jukebox, the parts lists and the results rows passed
  the absolute open time (`self.opened`, ms since the game started)
  instead: their HIGHLIGHT / fade-in scripts never began, so the message
  and the focused button stayed blank once the game had run longer than
  the screen's age (shots, at front-end time 0, hid it). All those sites
  now pass times relative to the opening. Selftest: `check_texts_shown`
  renders the dialog or SMS 1.5 s after it opened, a minute into the
  game's clock, and requires its message and every button label among the
  drawn strings (`frontend::drawn_texts`) — MC_Main's three steps,
  SMS_INSTRUCTION, the car lot's questions, the GPS question and the
  garage's "Return to Explore Mode?".
  Flow in the original (0x4AF090, UI_StartCareer's Start New Career):
  always FE_ScreenOperation (0x4A3100) with mode 0x21961, i.e. MC_Main.
  **Observed** under Wine: with the profile made at boot, main menu Career
  goes straight into the airport scene (capture.py `intro`); the clone,
  which makes no profile at boot, shows UI_StartCareer then MC_Main.
* **Shop dialogs over free roam** (player report: leaving the garage or the
  car lot "shows the dialog but the world building with the car inside";
  **fixed**). A dialog is a screen pushed over the one that opened it; the
  front end's scene (GarageMain) is its own object and stays under it.
  The clone drew free roam behind every `Menu::Dialog` (the car parked
  inside the shop, where the drive-in left it). `garage_request` now keeps
  the last front-end scene (car, view, open parts) under a dialog or a lot
  deal (compare/nis79/clone-crib-explore.png). Leaving a shop: the city
  start 0x5404B0, with a shop being visited (0x863444 / 0x86344C) and
  0x570A70 on the NIS manager, asks for its exit scene (0x430580: manager
  +0x58 = 1, +0x5C = the shop's +0x38; 0x4305B0 / 0x4305C0 read and clear
  it in the track loader, 0x57F9D9 / 0x57FA87, and the world start
  0x60A816 / 0x60B462). So the original also loads the city (PC_Loading)
  and then plays ShopExit1, whose first shot is from inside the shop
  (eye scene (−2.2, 8.9, 0.44), y > 0 is inside) looking out through the
  open door at the car driving out: the clone's flow is the native one.
  **Not checked at run time**: the drive to the car lot under Wine stalled
  in the lot (input held) before the engage dialog.
* **Scene and track camera lens** (**measured** 2026-10-29 under Wine,
  CareerNis1, peek of view 1's camera: the player's mover
  `[[0x8900B4]+0x238]`, its camera `+0x1C`; camera +0x40 the eye, +0x60
  the look-at, +0xC4 the binary angle): a camera track entry's +0x48 /
  +0x4C is a focal length in mm, not an angle. f 50.031 → 6440 (35.4°),
  50.046 → 6437, 50.062 → 6436, 29.971 → 10206, 29.987 → 10199, 24.046 →
  12222: angle = 2·atan(15.965 / f) (16 mm is 0.2 % wider). The mover
  writes it to +0xC4 (0x4462A0 → 0x75BED4), and 0x5C7ED0 projects it ×1.1
  (`Camera::game_fov`). `track_camera::lens_angle`; it applies to every
  type-0x12 / 0x13 track camera (scenes and race route cameras). The
  chase mover's own +0xC4 read 13018 (71.5°). The conversion's code is
  **not traced**. 0x4427C0 (ECX = 0x828770; space-3 points through
  0x447A30 / 0x447A80, after the mirror flip of y): the point's z (in
  the scene frame) += ground under the point taken to the world by scene
  +0xE0 (0x441000) − ground under the frame's origin (+0x110, cached in
  +0x1C60 when +0x1C34 ≥ 1, else taken each call), each a 0x5E6FB0(p,
  1.0) / 0x5E72F0 query (0x442700); nothing when no ground is under the
  point. **Ported** (race6, `track_camera::to_world` with a ground query
  for the shop / ending scenes and the race start scenes; **inferred**:
  the clone's query is the highest walkable surface below 1 m over the
  point, and no step when the origin has no ground).
* **Scene letterbox** (2026-10-31, race4; **verified** in the
  disassembly unless marked; ported, `track_camera::letterbox`,
  `ShopScene::letterbox`, `Game::letterbox_quads`). A camera-track entry's
  0x88 bytes end in effect bytes, start / end pairs (the order of the
  debug print's START_ / END_ fields at 0x787800): +0x70 / +0x71 aperture,
  **+0x72 / +0x73 letterbox**, +0x74 / +0x75 intensity, +0x76…+0x7B tint,
  +0x7C…+0x7F gain, +0x80 / +0x81 bloom, +0x84 an audio trigger.
  0x451B70 (the mover's new entry) copies each pair into a mover channel
  (+0x4C aperture, +0x50 letterbox, +0x54 intensity, +0x60 bloom; the
  tint and gain through 0x574CA0 / 0x574B20), lerped like the lens; each
  frame 0x453F50 writes the letterbox channel × 0.125 (0x7870A4) × 0.01
  (0x78425C) to its camera's +0xC8 (0x454553…0x45456C). The files: every
  race start scene IntroNis01…28 holds 100 / 100 (a few tracks 0 / 0),
  CareerNis1 89 → 74 and 100, the shop, dyno, ending and DemoFlyIn scenes
  0. **Observed** under Wine (Quick Race circuit and sprint starts,
  race112): the start scene is drawn between black bars of 120 pixels at
  the top and the bottom of 1280 × 960 (**re-observed** without the
  widescreen fix, vanilla115, whose DisableCutsceneBorders = 1 zeroed the
  bars' width (0x5CBE9B / 0x5CBEB3 / 0x5CBF07 / 0x5CBF17, 640.0 → 0) in the
  modded copies: CareerNis1's bars 106 then 98 pixels (89 → 74, ×0.125 ×
  480 × 2 truncated: 106.8, 88.8 at their ends) and 120 later in the
  scene, full width, pure black) — 0.125 of the height each, so
  camera +0xC8 is one bar's share of the screen height; the shop drive-in
  has none. **The bars** (2026-10-31, race5; **verified**):
  0x5CBDD0, called by the frame's tail 0x5CBF50 (0x5CC206) after the
  post-FX quad and before the front end (0x5378D0), takes view 1
  (0x48B1E0(1)); with its camera's (+0x40) +0xC8 > 0, the view active
  (+8) and its target (+0x58 → +0x10) set, h = _ftol(+0xC8 × 480
  (0x797D5C)) and two FE quads (0x5C5030) with the texture
  WHITE16X16_NOALPHA (0x8707F0, loaded by 0x5B9918) and colour 0:
  (0, 0)–(640, h) and (0, 480 − h)–(640, 480) on the 640 × 480 canvas.
  The 3D view is drawn whole and covered — the projection (0x5C7ED0,
  aspect from the target's height / width) and the viewport (0x5BA3D0:
  0, 0, target size) never read +0xC8 — so the bars crop, they do not
  narrow the view. Ported: `Game::letterbox_quads` now truncates the
  bar to whole canvas pixels (60 at 100). HUD_FEATURE_LETTERBOX
  (feature index 25, low 0x2000000, name table 0x7F6728) is what 0x6000A0
  returns alone in race status 1 and 2 (0x6001AA…0x6001BE → 0x600525)
  and online with +0x24A (0x6002DE); 0x4CA090 masks it off (0xFDFBFFFF)
  before the HUD elements, so its reader is **open**.
* **Race starts and a finish under Wine** (2026-10-31, race4;
  **observed**, `a private Wine copy` with wined3d at about 0.15–0.2× game speed,
  Quick Race, Peugeot 206, Traffic Level Minimum, 3 opponents; sheets in
  compare/race112/orig-*-sheet.jpg). **Circuit** (Resort Loop) and
  **sprint** (track 4123): PC_Loading, the start scene between letterbox
  bars, the chase view with the tach alone (gear N, 0 mph; no minimap,
  laps, position or leader board), then at the green the full HUD as the
  rivals pull away; no 3-2-1 / "GO!" in either. **Drag** (Quick Race,
  the first drag track): a start scene too — letterboxed, the starter and
  a second girl on the checkered line — then the chase view with the drag
  tach (N) and the starter, then the green with the rivals' burnouts; no
  3-2-1. The clone skipped the scene for drag races (`race.drag` in
  `race_start_scene`, not from the binary: 0x436840 case 0 has no
  race-type test); it now plays it (compare/race112/sbs-drag-pregreen.jpg),
  and the race's traffic is filled on the scene's first frame (the set-up
  fill runs at the load in the game) and stands until the green (the
  ~12 m roll seen in race2 is **not ported**). **Sprint finish**
  (track 4123, the player last, driven by a steering script along the
  clone's race line): on the line "You were last!" / "Your time: 3:49.74"
  over the race with no tach or minimap (the race-over features 0x4 /
  0x20000), the chase camera unchanged and no letterbox; then PC_Loading;
  then Event Results (Name / Car / Finish Time, the last column cycling
  with Time Behind) over the ending scene's crowd; Continue opens Post
  Race (Restart, Results, Replay, Quit to Main Menu; Quit asks "Quit to
  Main Menu? All unsaved race progress will be lost.", OK / Cancel) over
  the same scene. The clone's flow is the same (`play_race_ending`,
  compare/race112/sbs-sprint-results.jpg). **Race-over message in the
  clone** (race5): `--shot finish` showed none because the shot drew the
  HUD's first frame at once, where the RACEOVER scripts start the message
  faded out; the shot now runs the HUD along with the frames past the
  line, and the clone's race shows "You were last!" / "Your time: …"
  over the race, tach and minimap gone, at the original's place and size
  (compare/race112/sbs-raceover-message.jpg: the original's sprint left,
  the clone's circuit right). That sprint ran in the rain at night: fin/01…27 under
  race4-tmp hold the original's wet road with the street-lamp and car
  light reflections (the reference for **The reflection's flares, mode
  2**, still **not ported**). Not captured this round: drift, street X,
  URL, lap KO and outrun starts and finishes (**open**). The a private Wine copy
  prefix's DirectInput key map matches the fixed one (Left steers).
* **Race starts and finishes under Wine, round 5** (2026-10-31, race5;
  **observed**, `a private Wine copy` with wined3d, Quick Race, Peugeot 206 on
  Auto, 1 lap; the player's car driven by `race5-tmp/follow.py` (an AI
  racer's trail) or `bot.py` on the clone's race line (`examples/
  race_line`); sheets in compare/race112/orig-*-sheet.jpg). **URL**
  (the first URL track, 5 opponents): PC_Loading, the
  letterboxed scene (the grid, the starter with two flares on the
  checkered line), the chase view with the tach alone (N, 0 mph), then at
  the green position / laps / leader board / minimap and the rivals'
  burnouts — the circuit pattern; no 3-2-1. Finish (last, 1:08.06): the
  leader board rows of the finished read "Finished"; on the line "You
  were last!" / "Your time: 1:08.06" fades in over the race with no tach
  or minimap and no letterbox, then Event Results (Name / Car / Finish
  Time cycling with Time Behind) over the ending scene's crowd, then Post
  Race. **Drift** (track 4312, 3 opponents): the letterboxed scene
  (starter, crowd, close-ups of the player's car), then the chase view
  with the tach **and the drift score panel** (Best / Gain 2X with / X1 /
  Total) before the green, no minimap; at the green the minimap,
  position, laps and the points leader board, "Too Slow!". End by time:
  with the others done, "Hurry!" and a counter over the race, then "Race
  Over. Time's Up." (racer status 11, 0x668C7453) over the race, then
  Event Results with a Score column
  (compare/race112/orig-drift-hurry-timesup.jpg). The time-out is
  traced and ported in round 6 (see **Drift time-out**). **Street X** (track 4603, 3 opponents): the letterboxed
  scene (an overhead of the grid, the starter with flares), the chase
  view with the tach alone, the full HUD and burnouts at the green; no
  3-2-1. The **drag** start was logged in memory (see Drag placement,
  Why they roll): race state 0 at the load, 2 through the scene, 3 from
  its end to the green, 4 after. Not captured: lap KO and outrun starts
  and finishes, and the street X finish (**open**: the bot stuck on a
  barrier; at 1:37 of race time, with the three rivals finished, no
  "Hurry!" had shown, unlike the drift race).
* **Lap KO under Wine, round 7** (2026-10-31, race7; **observed**,
  `a private Wine copy` with wined3d, Quick Race Circuit with Lap KO On (laps 3
  and 3 opponents forced, greyed), Peugeot 206 on Auto, the player left
  standing on the grid; montages `race7-tmp/kostart_m.jpg`,
  `kofin_m.jpg`). **Start**: as the circuit — the letterboxed scene (an
  overhead of the grid, the starter, close-ups of the cars), then the
  chase view with the tach alone; at the green the full HUD with "Laps
  1/3". **Knockout**: the player (racer 3 of the race status list,
  +0xBD0) went to status 3 once the rivals completed their first lap
  (race clock about 2:20; the leader's estimated finish 7:20.33, so ~2:27
  a lap) — no earlier — consistent with 0x6079BF (place +0x0A + laps
  done +0x09 = 4 at a lap's end knocks out the racers behind). "You were
  knocked out!" then fades in over the race with no tach or minimap and
  no letterbox; the human being done ends the race, and Event Results
  (over the ending scene's crowd) list 1 Stan 7:20.33 (0x5F7ED0's
  estimate: the leader finishes at Race_Over), 2 Carol, 3 Paul and 4 the
  player at 0.00 (knocked out, 0x608CB0's walk), the column cycling
  Finish Time / Time Behind ("--:--.--" for the leader, −7:20.33 for the
  others). **Not captured**: outrun (no Quick Race mode: the Quick Race
  modes are Circuit (with Lap KO), Sprint, Drag, Drift, Street X, URL and
  Free Run; outruns need career free roam) and the Street X finish
  (**open**).
* **Race flow at full speed under Wine** (2026-10-31, raceflow; `fastgpu.sh`
  DXVK on the GPU, game clock = real time, 30 fps recordings lined up with a
  memory log of race status [0x890118] +0/+4/+8/+0xC and the player's
  [0x8900AC] +0x234; Quick Race defaults, Peugeot 206 on Auto, Crowds on;
  strips in compare/raceflow). **Countdown after a start scene**
  (**verified** + **observed**): 0x5EEA50 alone would give "GO!" (it returns
  4 for +0xC from the green tick +4, which 0x5FC910(4) sets in every race),
  and the scene's cut does set +8 (StartCountdown → 0x5FC910(3)), so 3-2-1
  too — but 0x6000A0's tail adds the countdown element 0x20000 only when the
  player's +0x234 is 0 (0x600555…0x600563), and 0x437910 sets +0x234 to the
  scene's id until the scene stop 0x439070 clears it (0x439109) at the
  scene's end. Logged: +0x234 = 1 from the scene's start to 4.1 s (sprint),
  5.5 s (drag), 4.9 s (circuit), 5.2 s (drift), 4.2 s (Street X), 4.5 s
  (URL) past the green; no 3-2-1 and no "GO!" drawn in any of them; the
  start scene's crowd vanishes at that moment. Ported: `Race::scene_end`
  (the HUD's countdown comes back once the scene's length has run, so a
  scene ending within +0xC of the green would show the rest of "GO!").
  **+0xC with a scene** (0x437910): (header +0x2C − header +0x30) / 30 / 3,
  logged 0.889, 0.989, 1.078, 1.233, 1.311, 1.356, the green exactly three
  steps after the cut; ported (`Race::countdown_step`). **Enter in the
  scene** (observed, drag): state 2 → 3 on the key, the chase view and drag
  tach at once, the green three steps later, the starter carrying on from
  her cut pose, +0x234 kept to the scene's end; ported (Enter before the
  cut jumps the scene and its characters to the cut). The skip's handler
  is 0x5FA890 (see "Race start scene cars", Skip); stopping the scene's
  speech then is **inferred**.
  **Leader board phase** (verified): the element's constructor (0x4F63E7)
  sets +0x1BC = the sim clock and +0x1C0 = 0 (gaps) when the race HUD is
  built, and 0x4DA190 flips after 10 s of gaps / 2 s of names on 0x865188
  — not from the green. After a ~10 s scene the board opens on the names
  (circuit names to +2.x s then gaps; sprint names at +0.05 s). Ported
  (`Input::race_age` from the race's set-up); the clone has no load gap
  (0.5–0.7 s between the clock's reset to 4000 and the scene's start in the
  game), so its first flip comes up to ~1.3 s later. **Leader board rows**
  (**verified**, 0x4C6F30 → **0x4AAED0**): the leader, then a window round
  the player (one place above while more than one row is left, one below
  while any is), labelled with their places; URL as 5th of 6 shows 1, 4,
  5, 6 (observed). The clone showed places 1–4 without the player; ported
  (`hud::board_rows`). **Quick Race records of a new profile**
  (**verified**, 0x515800 on 0x83AAB4; **observed** in memory at 0x83AAC4):
  every type players 1, opponents 3, difficulty Medium, traffic Minimum,
  Catch Up on; sprint and drag 1 lap; circuit track 4021; drift traffic
  Moderate; URL 5 opponents; type 5 one opponent on Easy; Free Run no
  opponents and 0 laps; Street X track 4601, Easy, traffic None, 10 laps
  (its options screen shows Laps 10 / Opponents 3 / Easy). The clone gave
  every type 0x52A230's 3 laps / Medium; ported (`QrRecord::new_profile`).
  Matched (start): letterbox scene, then chase view with the tach alone
  (drift: tach + drift panel; drag: drag tach), minimap / position / laps /
  leader board at the green, positions and lap counter. The drift
  grid and the board's highlight: see "Race start scene cars" and "Leader
  board highlight and rows" below (grid145). Finishes and Lap KO: see "Finishes at full speed, round 151"; the
  outrun is **open** (not captured).
* **Race start scene cars** (grid145, 2026-10-31; **verified** in the
  disassembly unless marked, **observed** under Wine at full speed with a
  memory log of each racer's car +0x60 by its info +0x03).
  The grid placement itself was right (0x6065C0 puts the racer whose info
  +0x03 is k on 0x5EB5E0's position k − 1, the marker with +0x0C = k − 1;
  markers 0–3 are the front row, 4–7 the second): at the load (race
  state 0) a Quick Race drift on Stadium Drift 2 (4312) had the player
  (slot 4) on marker 3, (−1039.97, −643.58), the front row's far side,
  as the clone did. The **start scene** then moves the racers: the scene
  set-up 0x43B000 → NIS_AssignSceneCars 0x4328B0 (type 0, a race start)
  clears the eight scene car slots (0x827B98, 0x14 bytes each: +0x10 the
  racer index, −1 none) and NIS_SceneCarsByGridSlot 0x432860 gives slot k
  the racer whose info +0x03 is k (the debug 0x865040 / 0x865044 swaps,
  0 in the file, are not ported); NIS_BindSceneCars 0x43ACF0 binds each
  slot's `ZPM_Car<k>_t` / `_q` (a slot with no channel is dropped); each
  frame NIS_UpdateCars 0x439240 (until the scene clock passes the cut,
  header +0x30) evaluates them and NIS_PlaceSceneCar 0x4309D0 puts the
  racer at the channel's pose through the scene origin: x / y and the
  heading of the transformed matrix, z the car's own (+0x68); the first
  time by Car_PlaceAt 0x5FBD70, then through the mode-5 mover
  NISMover_SetTarget 0x5A40B0, which places it again when over 1.1 m or
  15° off and otherwise steers it there. Two scene frames a key, as the
  shop scenes. Observed (drift, IntroNis12 by its keys): Car1 stays at
  (−1039.90, −650.53), Car2 at (−1039.09, −647.00) (2.8 / 3.6 m behind
  the start line), Car3 rolls from 12.5 to 8.1 m behind it and the
  player, Car4, from (−1026.83, −647.00) to (−1033.63, −647.00) over
  3.7 s (57 keys), so the player starts behind the three rivals; the
  restarts' grid (−1033.63 / −1034.18, −647.0) is this end pose. Circuit
  (Resort Loop 4014, IntroNis03 by its keys): all four cars roll in from
  10–20 m back and end on the scene's front row (lateral −1.74, 1.54,
  −4.72, 5.28 m, 2.7 m behind the line), within 0.2 m of the markers.
  **Ported** (`Scene::racers` / `Scene::racer`, `scene_cars`, `Rival::slot`):
  every racer with a `ZPM_Car<slot>` follows its keys until the cut (a
  rival keeps the AI route walk set up on the grid: restarting it from a
  roll-in pose behind a sprint's road start lost racers);
  the clone's ends match within 0.3 m (the clone's car position is its
  centre of mass, 0.3 m off +0x60 on the grid as well). **Inferred**: the
  clone sets the pose each frame instead of steering the car to it.
  **2026-11-02:** wheel phase now survives these placements, with native
  brake/additive burnout events and wheel-speed rules; see
  `race-start-wheelspin.md` for the trace and remaining pose/FX/audio
  limitations. **Drift rivals in the countdown**
  (finish151, **verified**): every race-state change (0x5FC910) calls
  each world car's controller vt+0x14; the AI racer controller's
  (vt_AIRacerController 0x785370 slot 5, 0x429B70) calls its driver's
  vt+0x1C with state ≠ 4 (car +0x30 is the driver), and at state 4 also
  brake 0 (0x5EB900), handbrake 0 (0x5EB910) and gas 1 (0x5EB920); only
  with a rolling start (0x89E7B8 > 0, the demo set-up 0x540096) does
  state 3 release it (pedals 0 / 0 / 0.5 and the car given that speed).
  The DriftDriver's vt+0x1C is 0x405B10 (+0x23C, "hold": gas 1, brake
  1, wheels straight, neutral in its frame 0x416120), so a drift rival
  stands from the cut (state 3) to the green, as observed (grid145).
  The clone's drift rivals drove off at the cut (their DriftDriver had
  no hold set); ported (`Rival::drift` held from the set-up, released
  with the cars' hold at the green). (The ctor 0x425FC0 with +0x2B4 is
  the human driver base, vtable 0x784FC8, whose slot 0x1C is 0x410430.)
  **Skip** (**verified**, finish151): the key handler is
  **0x5FA890**. Each frame 0x5EA660 picks the player's input mode by
  the flow state (table 0x5EA7AC: flow states 2 and 3, the race's
  load and start, give mode 8; in a race on a dyno run 0x510050 /
  0x430350 / 0x430420 pick 8, 9 or 1) and calls Input_SetMode 0x605C80
  on a change; mode 8 binds JOY events 0xBF (Engage, Enter), 0xBE (skip)
  and 0xBD (pause) to 0x5FA890 (0x6062A8; mode 9 binds them to 0x5FA910,
  the dyno's 0x43B760). 0x5FA890 on 0xBE / 0xBF: in a race (0x89E7B0 ≠ 0)
  it tail-calls **NIS_SkipToCut 0x43B460 directly**; outside one in career
  (0x850078 = 1) 0x43BC30 then NIS_UpdateEnd 0x43BB10; on 0xBD with the
  event's +0x50 clear and 0x430560 false it posts flow message 7 (the
  pause). The race state then moves on the flow's per-frame message 3
  (posted by the main loop at 0x581272): RaceFlow_Message case 3 in race
  state 2 sees the scene's clock past StartCountdown (0x438760 →
  0x437720: the event's time < clock +0x2C, so the frame after the seek)
  and calls 0x5FC910(3) and 0x603640(4). So the skip does **not** test
  g_RaceStartSkipAllowed 0x7FAAB8. **Message 6** (case 0x603C60: with
  0x89E7B0 and 0x7FAAB8 (1) set, race state 3, 0x603640(4), then
  0x43B460) has no poster: none of the 35 GameFlow_PostNow (0x601030)
  calls, the GameFlow_PostWorldEvent (0x5F41F0) calls (0x5F4250 posts
  message 8 with its argument) or the inlined posts on the message pool
  0x8900DC (0x6036A0…, 0x60A8CB…) passes 6 (**verified** by the call
  sites' constants; a computed message id is only at 0x60AEB5, where it
  is 1). NIS_SkipToCut 0x43B460: NISSound_ButtonThrough 0x479200 (0x82BC49 = 1
  when the NIS sound object 0x40000050 exists), NIS_SeekTo 0x43AF80 to
  the StartCountdown event's time (every scene object's time, the scene
  car slots reset and placed at that time, clock +0x2C), and the traffic
  resumed (0x409A30). The scene is not stopped (NIS_SceneStop 0x439070
  is not called, so +0x234 stays to the scene's end, as observed). The
  speech: NISSound_Update 0x475330 acts on 0x82BC49 with 0x82BC4A through
  the speech stream (0x45AD40, debug text "Button THrough NIS Playing");
  that this stops the line is **inferred** (not traced to the end; no
  sound under fastgpu). Message 3 (0x603C49) is the other path: in race
  state 2, when the scene's clock is already past StartCountdown
  (NIS_StartCountdownPassed 0x438760 → 0x437720), state 3 and flow 4.
  0x439070 is the scene stop (renamed from NIS_SceneFrameStart): with the
  camera on (+0x50) it resumes a type-0 scene's traffic once, gives each
  human not on camera mover 1 a chase mover (0x5FA830(1)) and clears
  +0x50; then the scene cars go back (0x437A00), the player's +0x234 = 0
  and +0x48 = 0; called from the scene unloads 0x43B310 / 0x43B380 /
  0x43B570 / 0x43B640 and NIS_Stop 0x43B7A0.
* **Finishes at full speed** (grid145, partial). **Drag, blown engine**
  (observed under Wine, Quick Race drag, Up held from the green on Auto,
  `grid-tmp/drag2`): the car stays in 1st on the limiter, "Short Shift!"
  over the launch from idle, 4th of 4; at 6.4 s past the green racer
  status +0x10 = 8 and "Blown engine!" over the car with the HUD gone,
  then (~3 s) the ending scene and Event Results (the rivals' Finish
  Time / Time Behind alternating every ~3 s, the player's row 0.00 /
  −1:40.27 highlighted). The clone (`NFSU2_QR_MODE=drag --bench race`, W
  held through the countdown): 1st gear to 36 mph, "Blown engine!" over
  the car with no HUD at 5.8 s; the bench stops at the race's end, so the
  results were not compared; the inputs differ before the green (the
  bench revs through the countdown), so the launch and "Short Shift!" are
  not comparable. **Teleporting the car blocks the original under
  fastgpu**: rigid-body writes (car +0x3C → +0x68: +0x20 position, +0x30
  axes) walking 8 m every 0.15–0.35 s froze the game (g_SimTicks and
  g_RealTicks both stopped, the process still running) after 8 s and
  38 s; one 3.6 km jump to the finish froze it at once. So the sprint /
  circuit / street X / URL / drift finishes, Lap KO and the outrun were
  **not captured** this round (the earlier wined3d rounds' teleports did
  not freeze); finish151 reached them by writing the racer block instead
  (see "Finishes at full speed, round 151").
* **Finishes at full speed, round 151** (finish151, 2026-10-31; fastgpu,
  vanilla EXE, Quick Race defaults, Peugeot 206 on Auto, 30 fps recordings
  with a memory log of race status and every racer block,
  <work dir>). **The way in**: no teleport. 10 s past the green
  (race status +0x10) `fin.py` writes the player's racer block (found in
  +0xBD0 by its car, block +0) +0x08 / +0x09 = the race's laps (+0xBE8)
  and its finish line's tick (+0x30 + 4·+0xBF7) = now; 0x608C11 finishes
  it that frame (status 1, finish order 1) and the game runs on normally
  (no freeze in nine runs). The clone's side: `game --bench race` with
  `NFSU2_QR_MODE`, `NFSU2_BENCH_FINISH=s` (the same write: laps, most laps
  and progress, then the finish), `NFSU2_BENCH_THROUGH=1` (on through the
  post-race screens), `NFSU2_BENCH_LAPS=n@s`, `NFSU2_BENCH_NOGAS=1`,
  `NFSU2_QR_KO=1`; dumps at 30 fps (`NFSU2_BENCH_DUMP_EVERY=2`).
  **Observed and matched** (sprint, circuit, Street X, URL, drift): the
  HUD goes at the finish frame and the race-over message fades in
  ("Winner!" / "Your time: 10.12", drift "You finished third!" / "Your
  score: 0", later "You were last!" as the points list moves); 3.0 s on
  the race is over (race status 4 → flow 1 → 7), PC_Loading for a few
  frames (sprint ~1 s, Street X / URL two frames), then the ending scene
  with Continue alone for ~0.3 s and the Event Results panel fading in;
  the rivals ended short get 0x5F7ED0's estimate with the player as the
  leader (sprint 1:32.10–1:32.42 against the clone's 1:32.03–1:32.55 now);
  Finish Time / Time Behind cycle (Time Behind "--:--.--" on the leader;
  drift: Score, no cycle). **Fixed**: (1) the race-over place line read
  the place (+0x0A) — 0x4AB4B0 reads the finish order (+0x0C) outside
  drift (`Input::finish_order`); (2) a centred string's block height
  counts its lines with 0x50C080, where "\n" counts only after a space on
  its line, so "Winner!\nYour time: …" is one line high and stands half a
  line lower (36 px at 1280×960, observed) — `frontend::line_count`
  (**verified**); (3) UI_PostRaceResults' panel (0x8D7E0C53) showed at
  full alpha for 0.33 s before its fade-in: its INIT chain (0x16A259,
  alpha 0) now runs (`ScreenState::chained`); (4) the post-race screens'
  open time took the wall clock, so a bench (fixed clock) cycled the
  columns early (now `Game::now`). The cycle itself: the original's
  Finish Time phases 4.53–4.57 s, Time Behind 1.83 s (30 fps capture;
  0x4E9B8B adds 0x865150 per tick and skips the flip tick), the clone's
  4.50 / 1.73–1.77 (**open**: the ~0.06 s per phase; the tick's caller
  is not traced). **Lap KO** (Circuit, Lap KO on; the player's +0x08 /
  +0x09 = 1 at 1 s, standing): at 108 s the third-place racer's lap 1
  knocks out the fourth (its row "Knocked Out", position "1st /3"); at
  197.6 s the second-place racer's lap 2 knocks out the player ("You were
  knocked out!", the tach and minimap still shown, position, laps and
  board gone); 3 s later the leader is ended, Event Results with the
  knocked out at 0.00 (Time Behind −4:31.67). The clone's bench set the
  record before the track select reset it (fixed); its knockouts then
  match; **fixed**: NumberRacers leaves the knocked out out (race status
  +0x28, 0x4EBB10; `Input::knocked_out`). **The knocked-out player's HUD**
  (**verified** 2026-11-01, small171; corrects the line here before,
  which said the tach and minimap stay): the knockout (0x6079BF →
  0x606CC0(i, place, …, 3)) calls 0x5EA540 for the player too, so player
  +0x50 is the knockout's tick, and 0x6000A0 checks 0x5FFF60 first:
  +0x50 set in a race (race type ≠ 0, ≠ 7) with flow state 4 gives
  0x40004 (RACE_OVER 0x4 and MUSICLABEL 0x40000) — no GAUGES_1 / _3
  (0x20 / 0x40, the tach) and no TRACK_MAP (0x1). The finish151 frames
  (`k200.png`, `ko-strip.jpg`) show
  exactly that: "You were knocked out!" over the car, no tach, no
  minimap. The clone's message alone is native; no change. (The player
  +0x28 = 3 branch of 0x6000A0 is not reached while 0x5FFF60 holds.)
  **After the finish the cars stop** (observed): the
  player's car under the AI racer controller brakes to 0 within ~1.7 s
  (circuit, from 21 m/s) and stands; AI racers finishing for real do the
  same (sprint, three rivals from 26 m/s to ~0 in ~2.5 s). Traced and
  ported: the goal "Park" and its "Stop driving" (see **After the
  finish**, The stop). **Not captured**: the outrun (career free roam, not reached
  this round); drag: see below.
* **Leader board highlight and rows** (grid145; **verified**,
  **observed**). Ordinary boards (0x4AA9F0 gaps / 0x4AA890 names): the
  player's row runs SetPlayerColor on its backing (+0x1A4 row objects)
  and LBData, the others Init (HUD_SingleRace: SetPlayerColor gives
  LBData colour (0x8A, 0xF4, 0xCA) and the backing a solid black, Init
  (0xC0, 0xC0, 0xC0) and black at alpha 0x78). Under Wine (URL, 5th of
  6) the player's name is green-olive and its backing darker, matching
  the clone within a few units (compare/raceflow/12). Drift boards
  (0x4AAC50 / 0x4AADC0) script **the backing alone**, by the node's flag;
  LBData is never scripted there, so every row reads white (observed:
  Quick Race drift, "4th /4", all four rows white); the clone ran
  SetPlayerColor on LBData too and coloured every drift row green —
  fixed. **Rows** (0x4AAED0): the budget is +0x170 − 1 places above and
  below, the player's own row not counted, so 3rd or 4th of 6 lists five
  places (1, 2, 3, 4, 5 / 1, 3, 4, 5, 6); the rows show the first +0x170,
  and the element's set-up makes it 4 (0x4F61AF): 3rd of 6 shows 1, 2,
  3, 4 and 4th of 6 shows 1, 3, 4, 5 (the clone's `board_rows`, now
  verified).
* **Race8 checks** (2026-10-31). **Start-scene traffic in a clone race**
  (the `drag` shot's career drag, Bayview Bridge, logged per 0.5 s): ten
  cars filled about 49–51 m apart along the bridge, each from 19.2 m/s,
  stop 12.2 m on (one frame of 0.64 m plus 11.6 m) by 1.5 s and stand
  through the scene (hidden) and the countdown — the first at (879.2,
  129.1) → (869.8, 135.9), against race2/5's (872.4, 118.8) → (862.5,
  126.0) under Wine (12.2 m, the same heading; the lane differs, drawn at
  random). **Street X under Wine** (WidescreenFix loaded; track 4603, 1 lap, 3 opponents,
  Peugeot on Auto, `race8-tmp/sxbot.py`: race5's line bot with xy
  teleports by memory writes when stuck): the rivals finished by 1:02; at
  2:56 of race time the player had not finished, the HUD still showed the
  race (Laps 1/1, the three "Finished", no "Hurry!", no time-out) — as
  round 6 traced, only a drift race arms the time-out. The finish itself
  is still **open** (the teleports reach only some of the car's position
  copies; the car fell off the line). **Race9**: the authoritative
  position is the chassis rigid body (vtable 0x7A20CC, ctor 0x5A7F40:
  +0x20 the centre of mass, +0x30 the axes rows, +0x70 momentum, +0x80
  angular momentum, +0xA0 velocity, +0xC0 angular velocity), which
  **0x5A5550** (ECX the vehicle: +0x20 the rigid body, +0x58 the car
  state = car +0x40) copies each step into the car: state +0x20 (car
  +0x60) = body +0x20, then 0x592410(body +0x1F0), state +0x50 (car
  +0x90) = body +0x30 and state +0x90 = +0x50, the velocity +0x30 = body
  +0xA0 and the speed +0x3F0, +0xD0 = body +0xC0. Writing only the body
  (`race9-tmp/tp9.py`: found by its vtable near car +0x60, the offset to
  car +0x60 kept, the four motion vectors zeroed, the axes turned) moves
  the car for good. **Street X finish observed** (race9, vanilla,
  wined3d, Quick Race Street X, Industrial Park Track 3 (0.39 mi), 1 lap,
  3 opponents, Easy, Peugeot on Auto; the car walked along round 8's
  line 4603 by rigid-body teleports of ~10 m every ~3 s,
  `race9-tmp/sx9.py`): the lap counted (positions 4th → 2nd → 3rd as
  the HUD showed), and on crossing the line the HUD went and the camera
  left the chase view for a low side view of the car rolling on, with
  the race-over message "You finished third!" / "Your time: 37.35"
  (`compare/race116/a private Wine copy.png`); then Event
  Results (Name, Car, Finish Time: Mario 32.76, Mike 33.11, NAME 37.35,
  Carla 41.01; `a private Wine copy.png`) with Continue, and Post Race
  (Restart, the stats icon, Replay, Quit; `a private Wine copy.png`);
  Quit asks "Quit to Main Menu? All unsaved race progress will be lost."
  (strip `a private Wine copy.jpg`). The clone's `--shot ending`
  (now with `NFSU2_ENDING_KIND=streetx`) shows the same results columns;
  its 1-s hold renders in explore mode, so the race-over message and the
  finish camera are not in that shot (round 10: `--shot finish` with
  NFSU2_FINISH_KIND renders them in race mode; the side view is the
  camera after a teleport, **inferred**, see Outrun end under Wine,
  round 10).
* **Race finishes in race mode, round 10** (2026-10-31, race10). `game
  --shot finish` with `NFSU2_FINISH_KIND` = circuit (default) / sprint /
  streetx / drift / drag / url / ko renders the first career event of
  that kind 8 s past the green with the player's finish forced, the HUD
  run along for NFSU2_SHOT_MS (2 s) in race mode: the race-over message
  over the race, no tach or minimap, the chase camera on the autopilot
  (`race10-tmp/fin-m.jpg`). Against the Wine captures (race112 sprint /
  URL sheets, race116 Street X): circuit, sprint, street X and URL show
  "You were last!" / "Your time: …" at the original's place and size, the
  chase view kept (the original's sprint and URL finishes keep the chase
  view too; round 9's Street X side view came from a teleport, see
  **Outrun end under Wine, round 10**); lap KO "You were knocked out!"
  (0x1B59940C) alone. Drag ended the race at once in the clone: the shot
  had blown the engine (see **Race finishes, round 11**). **Fixed**: the drift race-over
  message. 0x4C7F00 case 1 (status 1, finished) in a drift race
  (0x401910 on 0x89E7A0) or when 0x42FC40 holds (**not traced**) prints,
  at 0x4C80C7, the place line (0x4AB4B0) over "Your score:" (0x13640DE9)
  and _ftol of the racer's style manager's DRIFT trick banked points
  (racer +0x1D4 → [0] +0x2C, as the drift standings read them) as
  "%s\n%s %$d" (0x795B14), or "%s %$d" (0x790980) when the racer counts
  0x89E7C0 + 0x89E7C4 + 0x89E7C8 + 0x89E7CC sum to 1 (a drift race with
  virtual rivals only). Outside drift and the dyno flag 0x89E7E6 it is
  "%s\n%s %s" (0x795B20) with "Your time:" (0xC2878EBC), or "%s %s"
  (0x794FD8) alone; on a dyno run with 0x4018F0, "Your time: <time>".
  The clone showed "Your time" in drift; now `hud::Input::field` (the
  racers, humans and AI) picks the form (`--shot finish`,
  NFSU2_FINISH_KIND=drift: "You finished second!" / "Your score: 0").
  The original's drift finish itself was not captured (round 5 ended by
  time-out).
* **Race finishes, round 11** (2026-10-31, race11).
  * **Drag finish** (**observed** under Wine, `a private Wine copy`, vanilla,
    wined3d, Quick Race Drag, Bayview Bridge (1.06 mi), Traffic None, 3
    opponents, Medium, Catch Up on, Peugeot 206; `race11-tmp/dragbot.py`:
    Up held, Left Shift at 6600 rpm, the racers read from race status
    0x890118 +0x24 / +0xBD0: racer +0x0C order, +0x10 status, +0x14
    progress, +0x198 finish in ticks of 1/4000 s, +0x19C the estimate
    flag). A real finish: on the line the HUD (tach, place, time) goes,
    the chase view stays and the car rolls on (the autopilot), "You
    finished second!" / "Your time: 41.92" fades in over the race; the
    rivals finish meanwhile (40.99, 42.34, 42.44, all for real, +0x19C
    0); after the hold the ending scene's crowd at the line, then Event
    Results (Name, Car, Finish Time) with the player's row highlighted
    (internal evidence: the drag finish strip and results frames,
    `sbs-drag-finish.jpg`). An earlier run's rivals: 40.50, 41.77, 43.73
    (the bot 45.10). **Blown engine** (no shifts, the gas held at the
    limiter): racer status 8, place byte 4 (the last free), +0x198 left 0;
    "Blown engine!" over the race for the hold, then (LOADING) Event
    Results: the rivals still running ended by 0x608CB0 with 0x5F7ED0's
    estimates (+0x19C = 1): 1:48.02, 1:57.13, 1:58.78 from progress
    ≈ 0.04–0.05 at ~5 s, and the player "0.00", highlighted; Time
    Behind "--:--.--", +9.11, +10.76, −1:48.02 (internal evidence: the blown-engine strip).
    So round 10's "rival times ~1:45 and 2:02" in `--shot finish`
    NFSU2_FINISH_KIND=drag were native: the shot held W without shifting
    (a drag forces manual, 0x5EB440), the engine blew (the clone's status
    8 / 9) and the race ended at once with the estimates. **0x606CC0**
    writes +0x198 only for status 1 at the race's laps (0x606F92: the
    finish line's crossing tick − race start) and statuses 4–7 (jump
    table at 0x606CFB); status 3 (knocked out), 8 and 9 keep 0. **Fixed**:
    the clone's results showed a dropped racer's drop time (7.99); now 0
    for status 3, 8, 9. The shot now drives the drag to its real finish
    (up-shifts at the shift point, the shot's traffic removed so the
    straight run is not totaled): "Winner!" / "Your time: 41.33", the
    rivals at 0.89–0.97 of the run then (a career event: other rivals and
    tuning than the Wine run's Quick Race, so the times are not compared).
  * **Drift finish** (**observed** under Wine, Quick Race Drift, Stadium
    Drift 2 (track 4312, 0.30 mi), 1 lap, 3 opponents, Medium, 206 on
    Auto; the car walked along the clone's race line by rigid-body
    teleports, `race11-tmp/dr11.py` / `tp11.py`): the three rivals (racers
    driving the track) finished at 24.81, 25.15, 25.39 s, arming the
    30-s time-out ("Hurry!" over the counter, "Too Slow!"); the player
    crossed at 35.11 (status 1, order 4: a real finish, not status 11).
    The race-over message is "You were last!" / "Your score: 0" (the place
    by points, 0x4AB4B0, over 0x4C80C7's "Your score:") over the race
    without tach or minimap, the chase view kept; then the ending crowd and
    Event Results with the Score column (Marcel 13,549, Simon 7,761, Phil
    4,300, NAME 0, the player's row highlighted)
    (internal evidence: the drift finish strip). The clone's `--shot
    finish` NFSU2_FINISH_KIND=drift shows the same two lines at the same
    place and size (`sbs-drift-finish.jpg`; its place differs as its race
    is forced at 8 s).
  * **Results highlight** (**verified**, 0x4BF290, the rows filler of
    UI_PostRaceResults, from 0x4D7240): for row n (1-based, racers 0x89E7C0
    + C4 + C8 + CC) HIGHLIGHT runs on RANKING_DATA_n when the row's
    racer's controller (+0x14) +4 is 1 (human) and +0x900 is 0. The clone
    always highlighted row 1; now the human's row (`PostRaceResults::human`;
    the tournament standings take the same, **inferred**: their filler is
    not traced).
  * **Flag girl flare lights** (**verified** in the code): 0x439780 (the
    props object [0x827B28], 0x114 bytes) is a light group: +8 its list
    links, +0x10 the name "Flag Girl Flare Lights", +0x30 on (the group's
    +0x28), +0x34 the light list, two slots of 0x68 bytes from +0x3C, each
    node +0 and its light record from +8 (props +0x44 + 0x68·i, the light
    packs' 0x60-byte layout: +4 kind = 2 (point), +5 = 2, +7 = 1 on), and
    0x48E380 links it into **0x835850** (tail 0x835854). 0x433090 sets +0x30
    = 1 and light i: +0x0C colour 0x80A991DF (r 0xDF, g 0x91, b 0xA9), +0x10
    the flare's tip p1, +0x1C outer radius 3.7, +0x30 inner 0 (+0x34 14000,
    +0x38 10: unused for a point light). 0x48E3D0, after the packs
    (0x48E550…0x48E5CB), walks 0x835850 and passes each light of a group
    with +0x28 set to **0x4893A0** like a pack light: weight 1 − d²/3.7², the
    colour × 0.75 × weight / 255 lobe towards the flare — so the cars within
    3.7 m of a flare (the grid's front pair) get a pink light from it. The
    group lives until the scene stops: **0x43B7A0** (from 0x43BB10 and race
    state 4's 0x60B403: scene stop 0x439070 (NIS_SceneStop), then **0x43B1F0** → 0x439930
    frees the props, unlinking the group; 0x43B240 does the same when the
    next scene starts). Under Wine [0x827B28] reads 0 in the race.
    Ported: `World::scene_lights` (set each frame by `flag_girl_lights` from
    the flares while the scene crowd lives, the last places kept, cleared
    with the crowd), gathered by `nearby` after the packs within its cap.
    **Inferred**: the clone's crowd lifetime standing in for 0x43B7A0.
    Wine frame of the drift countdown with the flares:
    internal evidence: the drift countdown frame.
* **Drift time-out** (2026-10-31, race6; **verified** in the
  disassembly unless marked; ported, `Race::time_out`, `HURRY_TIME`,
  `hud::Input::hurry`). Race status (0x890118, built by **0x5FC740**:
  +0x18 = 0x7FFFFFFF, +0x1C = 0, +0x20 = −1.0) +0x1C is a tick and +0x20
  a length in s. **Arming** (Race_OnRacerEnded 0x606CC0 at
  0x606FB4…0x607024): in a drift race (0x89E7D9 / 0x890104), not split
  screen (0x89CF69 clear, 0x8669F4 and 0x866A14 ≤ 0), when the racer
  ending is an AI racer (controller +0x14 → +4 = 2), or a human with two
  humans (0x8900BC = 2), with status 1 (finished) and place byte +0x0C =
  1 (the first to finish), and +0x1C unset (0x401820): +0x1C = its
  finish (+0x198) + the race's start tick (+0x10), +0x20 = **0x89E7DC**
  (race parameters 0x89E7A0 +0x3C, 30.0 from RaceParams_Reset 0x5ED600;
  no other writer: the only reference is 0x60701E and no method of
  0x89E7A0 writes +0x3C). A human who finishes first arms nothing. A
  second arming path (0x606ECD) is for networked humans (0x89CF50 table,
  0x5EFA10; +0x20 = 0x7A5698 = 30.0); 0x60AB83 zeroes both (in Race_Over
  0x60AAB0). **Ending** (Race_UpdateState 0x6076E0 at 0x607BAD): when
  +0x20 > 0, +0x1C set and (now − +0x1C) / 4000 ≥ +0x20, every racer
  with status +0x10 = 0 is ended by 0x606CC0(i, place, 0, 11): status 11
  ("Timeup"; "Race Over. Time's Up." on the race-over message, 0x4C835B),
  the place the lowest one free. No other race type arms it offline, so
  no other race has a "Hurry!" or a time-out (consistent with round 5's
  Street X run). **HUD** (0x4EBB10's drift branch; objects from
  **0x4F4AC0**: +0x40 Hurry_Text, +0x44 Hurry_Counter, both hidden there;
  +0x48 a message queue on Hurry_Text, 4 slots, 1.5 s hold): unless
  0x5F0100 on 0x89CF48 (**inferred** false offline), with +0x20 < 0
  Hurry_Counter hides, else it shows t = +0x20 − (now − +0x1C) × 0.00025
  as 0x577250 flags 0 of _ftol(t × 4000 + 0.5) ticks, and when n =
  _ftol(t) is even and |t − n| < the frame time 0x865150, "Hurry!"
  (0x862A0519) is queued (**0x4EBAC0**; when the queue is full it is
  emptied first). **0x4EB9F0** (each update with 0x865194) counts its
  hold (+0x10, 0x577320) and when out shows the next message for +4 =
  1.5 s; the object hides when nothing shows (or in game state 7, 2, 3
  for the object hash 0xC18C12FD). **Observed** (round 5's frames, race5
  `drift_fin`): counter + race time stays 54.33–54.35 (9.07 @ 45.26,
  7.50 @ 46.85, 1.05 @ 53.30, 0.29 @ 54.06), "Hurry!" over the counter
  most frames, then "Race Over. Time's Up." without tach or minimap and
  Event Results with the Score column. **0x606DD1** (race7, **verified**
  in the disassembly): 0x606CC0 turns a time-out (status 11) of a human
  (controller +0x14 → +4 = 1) in a drift race into status 1 (+0x10 = 1)
  when controller +0x900 is 0 and [0x89CF4C] +0x250 is 0 — but the block
  is reached only from 0x606D9B's split-screen / network test (0x89CF69
  set, or 0x8669F4 > 0, or 0x866A14 > 0); offline 0x606DB9 jumps past it
  to 0x606F51, so a single-player time-out stays status 11 (as round 5's
  run showed) and the clone needs no conversion. The clone
  takes two finishes in one frame by time, the player first on a tie
  (**inferred**).
* **Race start scene** (`Game::race_start_scene`, `shop_scene::IntroTable`;
  **verified** unless marked; **corrected** in Session crowd83: the
  single-player pick is 0x436840 case 0 over the table's first list, the
  six-entry list below is case 3, the multiplayer pick, and with Crowds off
  a single-player race keeps the 0.9 s step). The NIS table (chunk 0x80037050 → 0x37070 in
  GLOBAL\InGameCommon.bun, pointer 0x827AD4 set by 0x431870) holds six
  race start scenes (count +0x0C, 0x14-byte entries from +0x2BC):
  IntroNis08 (flags 0x4F, 1–4 racers, start kind 1), 12 and 13 (0x4F,
  1–4, kind 2), 20 (0x7F, 3–6, kind 3), 14 (0x43, 2, kind 5), 21 (0x43, 2,
  kind 4). 0x436840 case 3 (track loader, game sub mode not 8, 0x865010
  clear): 0x431F10 counts the racers and the human's grid-slot bits;
  0x430160 the track's mask list (+0x37C, count +0x14, when its flag is 1,
  else +0x91C / +0x18; a track not listed takes any scene); 0x430020 flag
  0x40, the slot bits within the flags, the racer count in [+6, +7];
  0x42FFD0 the track record's +0xF6 (`Track::start_kind`; tracks use 1, 2,
  3, 6, 7) equal to the entry's +8 (and kind 3 on a kind-2 entry for two
  racers or fewer when the entry is the string 0x785D54's scene, not
  ported). One is drawn at random (0x43C140 on 0x89D00C; the clone draws
  from a copy of its own sequence). With Crowds on (0x5BF6E0:
  g_CrowdsEnable 0x870CD0) it plays, its camera track "Track %d" by the
  entry's byte +0x0C + slot − 1; with Crowds off nothing plays and race
  status +0xC (the countdown step) = (mark 1 − mark 2) / 30 / 3 (0x436F4D;
  marks are the header's +0x2C / +0x30, e.g. IntroNis08 260 / 180, of 429
  frames; the clone keeps its 0.9 s step). The countdown (0x5EEA50): with
  +4 set (0x5FC910(4), the green light, which also zeroes +8) it gives 4
  for +0xC s; else from +8 (0x5FC910(3)) 3, 2, 1 a step each, then 4; the
  race update 0x6076E0 goes from state 3 to 4 when it gives 4.
  **Observed** (scenes75 frames): the scene's shots, then the chase view
  with the pre-green HUD (tach, gear N, no minimap) while the starter
  stands on the road, no 3-2-1 / "GO!", then the green light with the
  starter walking off. **Inferred** timeline (ported): the camera over
  frames 0..mark 2 (fraction = frame / mark 2), the chase view from mark 2,
  the green light at mark 1 — the window Crowds off fills with the 3-2-1.
  No HUD under the scene's camera (**inferred**). The clone holds every
  car on the grid during it (compare/nis79/sbs-racestart-*.png,
  clone-racestart-scene.png). **Open**: the starter and the crowd
  (skinned models 0x80134000, skeletons 0xB3300000 and the animation bank
  0xE34010 of the scene bundle, placed by its 0x37040 records), the scene
  cars (`ZPM_Car<n>` channels) and the event track (SEQ_CAR_NITRO,
  CAR_AUDIO_PATTERN, burnouts, hydraulics), and what race status +0xC
  holds when the scene plays (1.0778 seen).
  **Observed** again (2026-10-30, career circuit STAGE_1_CIRCUIT_1
  engaged from its marker, frames every 2 s, `a private Wine copy/shots/g_sheet*.png`):
  PC_Loading, the scene (starter and a second girl between the cars, then
  the grid), the chase view with the tach alone, no 3-2-1 / "GO!" in any
  frame, then the minimap, position and leader board with the green light
  as the rivals pull away — the same order as the Quick Race above. Only
  this one career circuit was captured; the other race types' starts were
  not (**open**: sprint, drag, drift, street X, URL, lap KO under Wine).
* **Minimap racers** (checked): the orange arrows ahead in the original's
  minimap are the AIRACERINDICATOR_n items (colour (207, 128, 48)); the
  clone draws them (FENG_DEBUG shows them); the grid order (0x5264B0,
  see "Drift event, end to end", Grid) decides who starts ahead.

## Session (2026-10-29): race start scene characters, scene pick, countdown

* **The single-player pick is case 0** (**verified**; corrects nis79). The
  track loader 0x4371C9 calls 0x436840 with case 3 only when 0x5F0100 on
  0x89CF48 says other humans race (`neg al; sbb; and 3`), else case 0.
  Case 0 (0x436887): 0x865010 clear; a forced scene 0x865024 / variant
  0x865028; track 0x1262 takes the string 0x78637C's scene; **Crowds off
  (0x5BF6E0 false): no scene at all** (and race status +0xC keeps 0x5FC740's
  0.9 s); else the NIS table's first list (count +0x00, 12-byte records from
  +0x1C: hash, variant +4, flags +5, racers [+6, +7], start kind +8; 42
  entries over IntroNis01–27, `IntroTable::race_list`), filtered by
  0x430070 (the track record's u32 mask at +4 for entries 0–31, u16 at +8
  from 32) and 0x42FE00 (flag 0x40, the humans' grid-slot bits from
  0x431F10 within the flags, the racer count, 0x42FCF0's start kind: equal,
  or kind 3 with a kind-2 IntroNis13 variant 0 under three racers, or kind
  1 with kind 2 for two players; 0x42FD60: IntroNis15 for a human on slot 2
  only when the car's +0x581 > 1; 0x863470 +0x0D = 1 keeps only IntroNis08 /
  12; 0x8900BC = 2 adds 0x42FDA0), one drawn by 0x43C1C0; the variant goes
  out with the hash (the camera track, "Track %d"). Resort Loop (4014, kind
  1, four racers, human on slot 1): IntroNis01 v0, 03 v1–3, 06 v0, 08 v2–3;
  **observed** under Wine: IntroNis03 (compare/crowd83/sbs-scene-end.png).
  Case 3's Crowds-off step (0x436E5E..0x436F4D, a table by name equal to
  the headers' marks: 08 260/180, 12 322/200, 13 265/180, 14 320/200, 20
  311/200, 21 275/180, else 260/180; step = gap / 30 / 3 into race status
  +0xC) is `shop_scene::crowds_off_marks`, unused offline.
* **Green light during a scene** (**verified**): the scene start 0x437910
  (scene +0x34 = length / 30, +0x30 = mark 2 / 30) sets race status +0xC =
  (mark 1 / 30 − mark 2 / 30) / 3 and queues the timed event named by
  0x827AFC = BinHash("StartCountdown") (0x435BC1) at mark 2 / 30 (0x435EA0:
  a {0x7860C8, key, time} entry on the scene's +0x38 list). The race flow's
  message 1 (0x603BEF) starts the countdown (0x5FC910(3), flow state 4)
  only when no scene object of 0x827B88 plays (+0x50, 0x432500); so the
  countdown starts at mark 2 and gives the green light three steps later,
  at mark 1. **Not traced**: the StartCountdown handler itself.
* **Characters** (`nfsu2::character`, `render::characters`; **verified**
  unless marked, see the module docs). Per scene bundle: texture packs
  0xB3300000, skeletons 0xE34009 (EAGL ELF, `__Skeleton:::Bip1x`: +0 id,
  +8 bone count, bones of 0x70: scale, parent, quaternion, translation,
  index, inverse bind 4×4), solid lists 0x80134000 with skinned meshes
  (mesh flags 0x4081; 60-byte vertex: position, normal, colour, uv, three
  weights, three palette indices as floats; material palette bytes
  +0x21..+0x34), the character bank 0xE34010 (0x0F compounds of 0x0B
  rotations, 0x14 vec3 key blocks, 0x15 scalars; 12 pose floats a bone)
  and the 0x37040 records 0x4377F0 lists (0x435E10), set up by 0x437BE0
  (vtable 0x785FB4): +0x0C flags, +0x10 matrix, +0x50 solids, +0x6C
  animation, +0x70 skeleton name hash, +0x74 rate (halved for a skinned
  character), +0x78 delay, +0x80 / +0x84 loop range. The controller
  (0x434B00) advances 30 × rate frames a second: 15 animation frames a
  second, looping (0x20) or over the range (0x40). 0x431400 evaluates the
  pose, 0x6E3B10 builds model-space bones and multiplies each by its
  inverse bind (0x43C990) for the skinned draw (render info +0xE8,
  0x436050). Type 0x14 (vtable 0x7C1DB0, eval 0x6E9780, table 0x6E89C0,
  constants 0x6E88F0) is 0x15 with vec3 items
  (`eagl::decode_keyblock_vec3`). The clone skins on the CPU and draws
  through world.efx after the cars (**not native**: the character effect
  and its lighting are not traced — a fixed half-Lambert; the
  CHARACTERSHADOW blob 0x433220 is not drawn; the flag girl's flare glows of
  0x4335C0 are, since race9, and their emitters since race10 — their
  lights are not, see **Pooled flares**). The characters play on to the scene's length after the
  green light (**observed**: the starter walks off). Side by sides:
  compare/crowd83/sbs-scene-end.png, sbs-pregreen.png, sbs-postgreen.png
  (IntroNis03; same girl, crowd, cars and camera; the post-green frame is
  matched by eye, the Wine run being slow).
* **Scene cars** (**open**): `ZPM_Car<n>_q/_s/_t` drive the cars in (e.g.
  IntroNis08's four from 17–26 m back to the line, IntroNis12's back row
  from 12–16 m to 8–9 m); 0x439A10 binds "_" + prefix + "_t" / "_q" / "_s"
  on a controller at rate 1 with flags 0x2A (loop), which would replay a
  77-frame drive-in every 2.6 s — so which object moves the racers, and how
  a car's slot maps to `Car<n>`, is not traced; the clone holds the grid.
  The event track's SEQ_CAR_NITRO / CAR_AUDIO_PATTERN / CAR_TRIGGER_EFFECT /
  CAR_HYDRAULIC_* / SEQ_CAR_BURNOUT events are not ported (SCREEN_FLASH is
  since the visual-rating parity: docs/nis-mover.md "SCREEN_FLASH", `screen_flash`).

## Front-end menu audit against the original (2026-10-29)

Captured under Wine (private copy of a private Wine copy on Xvfb :92, capture.py
`--only walk` + peek.py key/hold/shot) and compared side by side with
`game --shot` in $OB_PRIVATE/wine/compare/menus92 (`orig/`, `clone/`,
`sbs/`; `sbs.sh ORIG SHOT` redoes one). Checklist: internal notes.
**Verified** (REA / objdump) unless marked.

* **GarageMain stays under the main menu's screens.** Of the 121 uses of
  "GarageMain.fng" (0x78FCD8) the pushes are 0x555F20(name, 0xFF, 0) at
  0x49D9D5, 0x4B9405, 0x55B720, 0x55E06C (UI_Main's constructor
  0x4DF140 among their callers), the only pops (0x55DDB0) are
  MU_QuickRaceCarSelect's constructor 0x4D8F20 and destructor (split
  screen), the rest are 0x52CF70 "is it up" tests. So UI_StartCareer,
  UI_OptionsMain and its pages (UI_Options, the controller page, EA Trax,
  the credits), UI_ProfileManager and its memory card screens, and the
  Quick Race track select / options all stand on the showroom with
  UI_Main's FE car; under Wine they all show it (the clone drew the
  Career menu, the options and the profile manager on black). The scene
  is SHOWROOM in game mode 2 / 0x20 (UI_Main / Customize), the car the
  main menu's (`Game::main_menu_car`). Clone: `Game::garage_request`,
  `Game::frontend_scene`.
* **A dialog over the front end's scene keeps it alone.** The D-Day car
  lot's Back ("Rachel needs her car back.") raises a dialog while the city
  is still loaded: the clone drew both the world (its dialog rule) and
  the lot's garage, from different render threads, so they flickered.
  The front end draws no world (FEng screens over GarageMain); the clone
  now skips the world whenever the garage is drawn. **Not captured under
  Wine** (the car lot is only reachable by driving there).
* **UI_Options' scroll bar** (0x4B8470 → 0x539660(0) → 0x538090(1,
  +0x12C − 1, position)): shown on every options page with more than one
  row (both arrows and the handle at the first row's place), not only
  for Player as the clone had it. 0x538090 shows the handle and both
  arrows over what the screen's code hid (`screens::scroll_bar_at`).
* **Fresh profile defaults** (0x515AA0, the reset 0x534700 runs on a new
  profile): Career Difficulty Medium (+0x250 = 1, 0x515B15), Save Prompt
  (+0xA0F8 = 1, +0xA0F9 = 0). Under Wine a new profile shows Medium /
  Prompt (peeked 0x83AC30 = 1, 0x844AD8 = 1, 0x844AD9 = 0); the clone had
  Easy / Off.
* **UI_Options_PC_Controller** (constructor 0x4ECD10): "UnHighlight" on
  the header 0x4E11CF90; 0x4ACE00 counts the configured input devices and
  with one (keyboard only) hides the header's device arrows 0xD1CDA594 /
  0x9345E4A7; 0x4DBA50 builds the 18 rows through 0x4CB4B0: the name item
  (0x51E0A0, vtable 0x79B348) gets "Options_Backing_<n>" (row number
  from 1) and places it at the text's position + (276, 8) (0x51E100:
  less +0x1C / +0x20 = (−276, −8)); its Highlight / UnHighlight
  (0x51E140 / 0x51E030) run on the text and the backing; the key cells
  (0x51DFD0) are placed by their text's top left (0x52D6C0 → 0x51D4E0 /
  0x51D080) on the row's line (their strings are centred, justify 5, so
  the clone adds half the text height). The scroll bar 0x538090(1,
  +0x134 − 1, position). Under Wine: the selected row's dark backing,
  keys on the names' line, no header arrows, handle at the top.
  **Open**: the key names come from DirectInput (`Control`, `space`
  under Wine; the clone's own names), the selected row's text pulse.
* **EA_TRAX_Jukebox** (0x553C80, rows 0x54B320): the play mode
  0xCA74A2FA reads "Sequential" (0x4CA36B89) for 0x83AA58 = 0, "Random"
  (0xA9C9C8F7) for 1 (the clone showed the package's "RANDOM"; a fresh
  profile has 1; settings +0x54, saved in the profile). 0x565DA0 (the
  screen's update, called from 0x56B2E1) flips it when either of the
  screen's events +0x10C / +0x110 fires (0x52D110), playing a sound
  (0x458520 with 0x8381AB / 0x8381C6, not ported), then rebuilds the
  music masks (0x524130, 0x479220); under Wine '[' and ']' each flip it
  (peeked 0x83AA58), the clone's LTrigger / RTrigger keys. The row groups 0x8065767B + row get "static", the
  row arrows "Static" (dim) — the clone ran HIGHLIGHT / UNHIGHLIGHT on
  them and hid them; the grab arrows Grab_Arrow_1…4 (0x07CD00B5 + row,
  INIT → HIDE) show only on GRAB; scroll bar 0x538090(4, 27, position)
  with Arrow_1 off on the first track, Arrow_2 on the last. **Inferred**
  from the capture: the selected row's arrows run HIGHLIGHT (which row
  objects 0x795CB0's highlight reaches is not traced).
* **UI_ProfileManager**'s title (0x4EB700 at 0x4EB740): string
  0xBCB18F38 "Profile Manager" (the package's label reads "Options").
* **Body shop category strip** (UI_GenericParts_Browser, an
  IconScrollerMenu): its ScrollBar handle by the category and the end
  arrows (0x52FA80), and the parts column's scroller (0x4A1316E9,
  0x38E39AD7, 0xD82B4A08, 0x640F0F22 / 23, INIT → HIDE) off until a
  category is entered. The clone left the handle at its authored x and
  showed the column's bar.
* **Visual Rating widget in the shops**: ported and traced in the
  menus100 section below.
* **Slider knob vs handle** (**open**): UI_Options' SLIDER_HANDLE_n runs
  scripts that set its z to 1, the same as SLIDER_KNOB_n; under Wine the
  knob draws over the green handle, in the clone the handle (#165) over
  the knob (#222). The equal-z order follows the traversal's list order,
  which the clone takes as the package's order (**inferred** there).
* **The selected icon's green frame pulses** (under Wine it fades in and
  out over about 1.5 s); a still may catch it off.

## Shop sub-screens against the original (2026-10-30)

Captured under Wine (private copy $OB_PRIVATE/wine/runs/<run>, wined3d on
Xvfb :100, main-menu Customize with the Peugeot 206) and compared with
`game --shot <screen>_cust`; images in $OB_PRIVATE/wine/compare/menus100.
**Verified** (objdump of SPEED2.EXE; the REA Ghidra provider did not start)
unless marked.

* **Visual Rating widget** (`status::shop_rating_quads`,
  `Game::shop_rating_quads`). The object 0x8637D0 (constructor 0x5315C0)
  holds two widgets: +0 for UI_Status_Career (vtable 0x79AA98: find
  0x53DAB0, update 0x523660) and +0xDC for the shops (vtable 0x79B82C:
  find 0x53D900, update 0x54B010); 0x5104D0 sets their colours +0xD0
  (0xCA, 0xF4, 0x8A), +0xD4 (0x80, 0xFF, 0x00), +0xD8 (0xCF, 0x80, 0x30).
  0x55BCA0 on each screen change pushes UI_Menu_Asset_Reputation for
  UI_GenericParts_Browser, UI_ChoosePaintCategory, UI_Paint, UI_DecalMain,
  UI_ChooseSpinner, UI_Rims_Browser, UI_DecalsOverlay (0x557ED0) and calls
  0x55B1B0(0x8392C0, 0x8389D0), else pops it (0x53D8D0). 0x8392C0 is the FE
  car 0x838980's +0x940 (the car as fitted), 0x8389D0 its +0x50 (the
  preview ride the shop screens fit the highlighted item on: 0x55C180,
  0x55C200, 0x55C3E0, 0x55F350; copied back by 0x55C470); each of those
  calls 0x55B1B0 again. 0x55B1B0: +0x4C = 0x5234D0(car) and update with the
  flag +4 clear, then +0x50 = 0x5234D0(preview) and update with it set.
  0x54B010 with the flag: 0x53DA20(preview) and colour +0xD8 when the
  preview is lower, +0xD4 higher, +0xD0 equal. 0x53DA20(v): text 0x171D00CB
  = "%$.2f" (0x79BF3C) of v, slider value v − floor(v) clamped 0–1.
  0x523610 colours vr_handle, vr_Beg, vr_End (widget +0x20 / +0x24 /
  +0x28 = slider +0x18 / +0x1C / +0x20 = descriptor +0x10 / +0x14 / +0x18),
  the text and the star 0x6EC2F0DE (0x53D900 shows it). The bar: 0x540C20
  with flag 1 (s+4), margin 44 (descriptor +0x2C), range 0–1; its redraw
  0x4FF530 takes vtable 0x78F544 slot 7 (0x540EA0) when vertical: the caps
  hidden (0x50CA00), track = the base's width (0x51D740, before the parts'
  ±90° turn in the package) − 88; with the base's position (bx, by)
  (0x51D630) the knob at (bx + 1, by + track / 2 − 0.95·f·track, 7 higher
  once f > 0) and the handle 0.95·f·track wide at (bx + 4, by + track / 2 −
  width / 2 − 7) (0x51D4E0). Under Wine: "0.00" in the colour +0xD0 on the
  body shop, "0.12" in green on Front Bumpers' ACE, "0.15" in green on
  5Zigen's first rim style; the clone gives the same numbers.
  **Inferred**: which rim size UI_Rims_Browser previews on its strip (the
  fitted one in the style, else the first); the preview in UI_Paint is the
  highlighted swatch, in the parts browsers the highlighted part.
* **UI_StatBar / status widget colours**: the same 0x523610-style
  colouring in 0x523660 (0x523AC5: +0xD0 with the flag clear) applies to
  vr_handle, vr_Beg, vr_End on UI_Status_Career (the clone now colours
  them).
* **UI_GenericParts_Browser in a category** (0x55EDE0): hides 0xDF422345
  (the strip's highlight frame), shows Install, fills and previews, runs
  0x545520 (both bars: the strip's +0x114 and the column's "SideScrollBar"
  +0x16C, made by 0x542AD0 with flag 1 at 0x55E8F3; 0x538090(1, count,
  position)) and sets the help mask 0x401 | 8 ("Install Part"), | 0x10
  ("Install Vinyl") instead when 0x838990 = 2 (the graphics shop: 0x520430
  maps shop kind 1 → 2) and the category option 0x83899C = 0 (vinyls);
  leaving (0x55EF90) shows the frame again and puts the screen's mask
  (+0x20) back. Nothing hides the strip: under Wine its icons, handle and
  arrows stay; the column's up arrow is off on the first part (the
  IconScrollerMenu's arrow rule 0x52FA80; **inferred** that the column's
  are the SideScrollBar arrows). The clone dropped the strip's icons, kept
  the package's handle and both arrows, and showed "Continue".
* **UI_Rims_Browser** (refresh 0x5468B0): the title "%s %s" (0x794FD8) of
  the brand's name — 0x50E960(0x839BBC): the brand record's name (+0) from
  the table 0x838400 (0x4FFF40 by +0x40), else a switch of brand hashes to
  their label strings — and the style's ("5ZIGEN FN01R-C" under Wine; the
  clone showed "FN01R-C"); the part name 0xD28B9316 "%s %d\"" (0x79C1CC)
  of the name (0x610010; **open**: attribute 0x4732DA07 when present) and
  the size attribute 0xEB0101E2 (low byte, signed): "FN01R-C 15\"". The
  sizes column's bar 0x538090(1, +0x210, position) on +0x1B4. Under Wine
  the styles strip has its handle at the start, no left arrow on the
  first style and no column until a style is entered; the strip's icons
  stay in the sizes. **Open**: the column's down arrow is dim with three
  sizes (not traced); the car shows the previewed rims (car looks).
* **UI_Paint slots and swatches**: entering a slot (0x55CC60) hides the
  strip's frame 0xDF422345 (0x55CD2A), runs "FadeIn" on the swatch panel
  0xDB285767 (0x55CD45; its INIT chains to a zero-alpha script, so it is
  invisible on the slots) and sets the help mask 0x281 (Cancel Changes,
  Install paint, Quit Game; 0x55CE78); leaving (0x55CF05…) shows the
  frame, hides the highlight 0x9BA6D658 and runs "FadeOut" (0x55CF35). The
  slot strip's handle and arrows as the parts browser's (**inferred** for
  UI_Paint from the class; under Wine the handle at the start and no left
  arrow on Base Paint).
* **Open** (seen under Wine, not traced or not ours): the shops' garage
  camera keeps moving while a category screen is up (the graphics shop
  went from a rear close-up to a high rear view with no key pressed; the
  camera records' +0x00 "tour" flag is the candidate), and leaving a
  parts list or the rims does not reset it; the clone holds the view
  still. The paint swatch highlight looks like an enlarged swatch rather
  than a ring (not compared on the same swatch).

## Shop sub-screens, round 2 (2026-10-30)

Captured under Wine (private copy $OB_PRIVATE/wine/runs/<run>, wined3d on
Xvfb :132; main-menu Customize, Peugeot 206; the unlock-all byte 0x838464
poked to 1 through /proc/pid/mem so that vinyls and every performance part
list) against `game --shot <screen>_cust` (with `NFSU2_UNLOCK_ALL=1` where
the original had the byte set); side by sides in
$OB_PRIVATE/wine/compare/menus103 (orig left). **Verified** by objdump of
SPEED2.EXE (the REA Ghidra provider did not start: no JDK at its JAVA_HOME)
unless marked.

* **Unlock-all byte 0x838464**: 0x52A020 / 0x52A0A0 / 0x52A120 / 0x52A1B0
  return "unlocked" at once while it is set; the car lists read it too
  (0x5022C0, 0x52FFFF, 0x534121, 0x53B51E). Nothing in the exe writes it.
  The clone mirrors it as `career::UNLOCK_ALL`, set only by shots
  (`NFSU2_UNLOCK_ALL=1`), never in play.
* **The preview ride on the car** (`Game::preview_ride`; was "not in the
  clone"): the FE car shows its +0x50 ride. A parts browser in a category
  (and the rims' style strip) shows a copy of the car as fitted (+0x940,
  0x413440) with the highlighted part (0x55C180; slot 0x1D also 0x1E;
  slot 0x22's extra 0x547480 **open**). Elsewhere the ride is 0x55C470's:
  the copy, then each graphics-setup item fitted in turn (an item on a
  vinyl layer while 0x50EF10 holds first clears 0x40–0x43); it runs on
  leaving a category and the paint slots. UI_Paint fits its highlighted
  swatch on the ride as it is (0x55C200, from 0x55CC60 / 0x55D0E0 /
  0x55D2E0 / 0x55D420), the vinyl colours included. The shops' Visual
  Rating preview (0x55B1B0) rates that ride, so the graphics shop's
  categories show the setup's rating (under Wine 0.04 with Flames 1A in
  the setup). Under Wine: Tear Set 100 drawn on the 206 while highlighted.
* **Vinyl colours on a stock ride**: 0x637050 (stock ride) at 0x637100
  fills every layer's colour slots 0x4D + 4·layer + k with the parts named
  VINYL_L1_COLOR09 (grey 128), VINYL_L1_COLOR02 (white 250),
  VINYL_L2_COLOR11 (129, 130, 120), VINYL_L1_COLOR01 (black 5) (0x61B990
  by hash); read from the 206's ride under Wine (+0x940 and +0x50 slots
  0x4D–0x50 = D0561E94, D0561E8D, 44D28FAE, D0561E8C). The clone took an
  unset slot as empty (red / green vinyls); now `car::VINYL_STOCK_COLOURS`
  (`vinyl_layers`), and UI_Paint's vinyl colours open on that colour.
* **Vinyl Select asks first** (`SetupStep::Vinyl`): 0x566D80 (Select with
  +0x1D7, the vinyl browser) → 0x559250: in a career the vinyl's price
  (0x514280) plus the setup's (0x521540) against the bank (0x861E74), else
  "You do not have enough bank…" (0xD1AC60BE); then "Would you like to add
  this vinyl to your setup?" (0x31FBC167) or, for a unique vinyl
  (NUMCOLOURS 0x564B8CB6 = 0x40) while a unique one is fitted or in the
  setup (0x5215E0, 0x521590) or the setup has a vinyl on layers 0x40–0x42,
  0xEC8616FD; GenericDialog_MED (0x78F2F8), Yes 0xF770 → 0x51123592, No
  0x061C → 0x34DC1BCF, No focused (0x5587A0's 1). Yes → 0x566F12 →
  0x5670F9: 0x54FDC0, then with REMAP its colours (0x55BD70) else the
  layers (0x55BE00). The clone went straight on.
* **UI_Paint mode 1** (vinyl colours): with NUMREMAPCOLOURS < 2 UI_Paint's
  setup enters the slot at once (0x563C79: mode 1 and 0x839BB8's
  attribute "NUMREMAPCOLOURS" (0x79BE98) below 2 → 0x548640, 0x55CC60), so
  the swatches open on the slot's colour and the help is 0x281 (Cancel
  Changes, Install paint, Quit Game; 0x55CC60 serves both modes). The
  swatch questions (0x563420 at 0x5637D4; the paints' 0x55D4BF) use
  GenericDialog_SMALL (0x78FA24) with No focused, and check the bank only
  in a career (0x850078 = 1 at 0x563799). Under Wine the finish arrows
  CF6F0439 / ABA93D3F (UI_Paint +0x150 / +0x154, top-level, not under the
  finish group 0x031A9C0A) are not drawn in mode 1: the ctor 0x56AEC0
  hides both and only entering slot 0x3F / 0x45 / 0x4B / 0x4C shows them
  (**verified**, see "Native front-end fixes, part 1"). **Open**: the camera (under Wine a closer side view than the
  vinyl list's).
* **UI_ChooseVinylLayer** (refresh 0x5535E0): a layer with a vinyl (setup
  or car, 0x521680 / 0x610300), no unique on, SHOWs "Reorder Layers"
  0xD999B6E7 and the trigger arrows (0x495520 at 0x55371E; the label's FNG
  flag 0x40 starts it hidden, so the clone needs the show), and the help
  mask gets 0x800000 = "Repaint" (0xF667659C) when the vinyl has REMAP
  (0x5536B4). Under Wine: Repaint / Back / Continue / Quit Game on
  Flames' layer.
* **Decal lists** (UI_DecalsOverlay over UI_DecalMain): 0x556880 for a
  slot 0x34–0x3E or 0x61–0x98 sets the help mask 0x800421 (Decal Color,
  Back, Install Decal, Quit Game); the price hides outside a career
  (0x546FC0 in 0x546F50); the list's view (0x568540 at 0x5685EF) is
  0x4C2100 on the slot's location 0x50E5A0 (0x61–0x98 by eights →
  0x34–0x3A, else 0xA9: no record), Windshield → Front Window Decal.
  Under Wine the strip keeps its icons, handle at the start and no left
  arrow (the clone now runs the IconScrollerMenu bar on UI_DecalMain's
  strip, **inferred** class from the look), the column's handle at the
  top and no up arrow on "No Decal" (the overlay's SideScrollBar objects,
  same rule as the parts browser's).
* **UI_BuyPerformanceParts** shows 4 rows (+0x134 = 4 at 0x569E1D; the
  option list's default 7, UI_Options 8 at 0x4CF697) with its
  "scrollbar"; under Wine with the 12 engine parts listed, four rows and
  the bar. The window follows the selection a row at a time (**inferred**
  for 0x5390F0). While it is up the help is its own mask 0x409 (Install
  Part). The package screen outside a career: 0x5624F0…0x562520 hide
  0x06E0C9A8, 0xBE8693E1, 0x0380B632, 0x89CA4FDB (no "Total Cost:"), and
  under Wine its rows read "Not Installed" (**inferred** from the row
  class's 0x53B7C0). **Open**: the highlighted row's green text (the
  clone's stays white).
* **Seen under Wine, not traced** (**open**): while a category, decal
  list or vinyl list is up the camera keeps moving after the blend (the
  decal view kept closing in on the windshield; leaving the graphics shop
  for its exit question reset it to the base view); the screen under a
  dialog stays drawn (dimmed) in the original — the clone draws the
  garage only; the swatch question's backdrop hides UI_Paint's strip and
  title (0x53BD30 at 0x563803, not traced).
* **Specialties, Custom Gauges, Neon** (under Wine): UI_ICEMAIN's and
  UI_CustomNeonMain's strips show the handle at the start and no left
  arrow on the first item and keep their icons in a list (as the decal
  shop's, same **inferred** class); no Visual Rating on UI_ICEMAIN (not in
  0x55BCA0's list; the clone showed it); a neon list's help is Install
  Part (UI_NeonPartsOverlay's 0x409; the Custom Gauges list keeps
  Continue) and its highlighted colour is drawn on the car (**inferred**
  0x55C180 from 0x560790 / 0x560930). **Open**: the original shows the
  Visual Rating widget on a highlighted neon colour (0.14 on RED) though
  UI_CustomNeonMain is not in 0x55BCA0's list.

## Shops, round 3: first-entry movies and Rachel's shop lines (2026-10-30)

**Verified** (REA Ghidra and objdump of SPEED2.EXE) unless marked.

* **First-entry movie** (`Game::open_shop`, `Profile::entered`,
  `Shop::movie`). UI_EngageShopDialog's Accept (0x499690 → 0x499640),
  with a visited shop (0x863444, 0x86344C): 0x499470(flag) with flag =
  the shop record's +0x20 (a movie name, 0x18 bytes) is not empty and
  0x500460(shop) is false. 0x500460 looks the shop's hash (+0x38) up in
  the visit table (career +0x7AFC count, +0x7B00 8-byte entries) and is
  true when the entry's +6 is 5. 0x499470 keeps the flag at 0x836350 and
  the shop at 0x83634C, then 0x54CB00 enters the shop, whose first act
  is 0x512F10(hash) (find or add the entry) and +6 = 5 (0x54CB19). So the
  movie plays on the first entry only. The drive-in scene follows
  (0x836351 = 1 when one plays). Its end (0x43BBD9), or Accept with no
  scene (0x49967C), runs 0x4995A0: with 0x836350 set, 0x557E00(parent =
  UI_EngageShopDialog.fng when up (0x52CF70), else 0; shop): game mode
  0x838568 = 2, the movie queue 0x8382A0 cleared and +0x20 put in
  (0x43DC00, 0x40 bytes), IG_PlayMovie.fng pushed (0x555D10 / 0x5377B0);
  the queue's end runs 0x54AF90 case 2 → 0x499560: 0x528570 (the shop's
  front end) and 0x836350 = 0. Without the flag 0x4995A0 runs the same
  0x528570 at once. Movies in the data (chunk 0x34A12): CC_CAR_LOT_1
  "SCENE05" (Scene05_en.vp6), CRIB_1 "SCENE06"; the other 22 shops have
  none. The entry state is in the save (the visit table, `native_save`
  visits: byte +6 = 5). Clone: the dialog's Accept sets `shop_movie` and
  `Profile::entered`; `open_shop` plays the movie on IG_PlayMovie (mode
  2, skipped by Enter after 0.5 s as IG_PlayMovie does), whose end opens
  the shop (`shop_front_end`). The engage dialog's body also tests +6 =
  5 (0x512920: 0x15D6B438 instead of `<NAME>_BODY` once entered; ported,
  see "Shop previews and engage message"). Selftest: the D-Day car lot and garage entries now pass
  through SCENE05 / SCENE06; `--shot carlot_first`.
* **Under Wine** (the original, private copy $OB_PRIVATE/wine/runs/<run>,
  wined3d on Xvfb :141, memory read and written through /proc/pid/mem):
  a new career on the profile NAME, the car moved into CC_CAR_LOT_1's
  trigger during D-Day A. Before Accept the lot's visit entry read
  `01 00 00 00` (visited, +6 = 0); after Accept 0x836350 = 1, 0x836351 = 1
  and the entry `01 00 05 00`; once the drive-in ended the movie queue
  0x8382A0 held "SCENE05" with 0x838568 = 2 (the movie is black in this
  Wine build), then the car lot opened with Rachel's once-flag +9 = 1 and
  kind 4 posted in a second (hints slot {4000, 4}); the speech manager
  0x82BA30 is 0 with the sound off, so the line itself was not heard. A
  second entry (after taking the stock 206 and leaving): 0x836350 stayed
  0, no movie, and the kind-4 time +0x130 = the clock +0x11C (217638) —
  the later line (0x20, 0x20). CRIB_1's first entry: "SCENE06" queued in
  mode 2, then the crib with +0xA = 1 and 0x8641F8 = 1. Clone:
  `NFSU2_AUDIO_WAV=… game --shot carlot_first` (12.7 s: the movie's
  sound, then Rachel's line in the lot).
* **Rachel's shop lines** (`Hints::fe_line`; was "traced, not ported" in
  **Front-end lines**). 0x5243E0(kind), career only (0x850078 = 1). The
  callers (all on the screen's INIT 0x35F8620B unless marked; the class
  by its vtable and constructor): UI_CareerCarLot 4 (0x4EDACB), UI_CareerCrib
  0x11 (0x4AF502), UI_ChoosePerformanceCategory 1 (handler 0x55C530,
  vtable 0x79CD88, ctor 0x556C90; 0x55C68B), UI_ChoosePaintCategory 0xF
  (0x562C60, 0x79CDD8, 0x5571C0; 0x562D18), UI_ChooseVinylLayer 0xE
  (0x56AB10, 0x79CC3C, 0x554CB0; 0x56AB6A), UI_DecalMain 5 (0x56B3C0,
  0x79CA8C, 0x553100; 0x56B648), UI_ICEMAIN 0x12 while 0x838998 = 0
  (0x568750, 0x79D67C, 0x55C020; 0x568995), UI_GenericParts_Browser 2
  while 0x838990 = 0 (0x5664A0, 0x79D760, 0x55E830; 0x566EEE — not a
  purchase as noted before), UI_PerformanceTuning_Setting 0x1B on its
  first tick (0xC98356BA with +0x154 clear, 0x55AA3C). 0x838990 is set
  by 0x520430 from the shop kind: 1 → 2, 2 → 0, 3 → 1, 5 → 3, 6 → 6 (4
  none), so kind 2 speaks in the body shop only. The function: kind 0x11
  sets 0x8641F8; in a shop 0x8641F8 = (shop +0x50 = 0, the garage), and
  kind 1 in CC_MEGALOW (0x7FC8D0D0) becomes 0; with 0x8641F8 kinds 1, 2,
  0x12 become 0x11. Kind 4 with +9 clear and 0x11 with +0xA clear take
  their once-paths (`car_lot`, `crib`); later kind 4 goes on like the
  others, 0x11 says nothing. Kind 2 sets +0x28. Then a kind with its time
  +0x120 + 4·kind set (not 0, not 0x7FFFFFFF) and +0x11C − time ≤
  ftol(3600 × 4000 + 0.5) is skipped; else the speech stream stops
  (0x45B490), the line plays and the time is kept. Lines (table
  0x524FD4): FE_Screen (0x510F30, through 0x79AD08) with b 0x20: 0
  (0x400000), 1 (0x800000) while the kind's time is unset, else 1
  (0x401820 tests it), 2 8, 3 0x10, **4 0x20** (the car lot's later
  visits, 0x5245E1), 5 0x40 … 16 0x80000 as listed above, 18 0x2000000;
  0x11 none; 19–27 FE_Dynoshop (0x510F50, 0x79AD0C) 1 … 0x100 with b
  0x10. **Verified and ported (2026-11-12)**, the other three
  posts: UI_Paint's handler 0x563420 on its INIT (0x35F8620B, 0x56351A)
  posts 8 when 0x83899C = 2 (the Paint category; the vinyls' colours run
  UI_Paint with 0) — the "0x5632A0, message not identified" of the
  earlier note; 0x53B780, the action of the performance category item
  (vtable 0x79B5C4, built by 0x530410 with +0x48 the category, in the
  0x530500 order 3, 7, 1, 5, 2, 8, 6, 0, 4), on 0x0C407210: 0x838994 =
  +0x48, then 0x525050(+0x48) (table 0x5250E8: 0 → 0x10, 1 → 0xC, 2 → 9,
  3 → 7, 4 → 0xD, 5 → 0xA, 6 → 3, 7 → 6, 8 → 0xB); and
  UI_PerformanceTuning_Master's handler 0x55D920 on 0x0C407210 with the
  item's +0x22 clear: 0x83A648 = +0x48 (the 0x557690 order, 0–6), then
  0x525110 at 0x55D9CC (table 0x525188: 0x13, 0x14, 0x18, 0x19, 0x15,
  0x16, 0x17). Both reach only an open item (the strip 0x543D50 skips one
  with +0x22). The clone: `Game::option_line` from `Action::PerfCategory`
  / `Action::TuningCategory`, UI_Paint in `tick_fe_lines`; test
  `tests::career_stages_1_2`. UI_ICEMAIN's 0x838998
  = 0 is taken as the main specialties list (title 0xDF86227A,
  **inferred**). The clone calls `fe_line` once per screen opening
  (`SaveFlow::fe_screen`, set on the first frame of a line screen, kept
  under a dialog or memory-card message, cleared on any other screen):
  the INIT runs once, and a second call is not harmless — on the crib's
  first visit it stamped +0x120 + 0x11·4 after the snapshot (the first
  call returns at 0x5244F0 before the stamp), so the Yes of "Return to
  Explore Mode?" asked to save (found in a playthrough, fixed; test
  `tests::career_playtest`). **Verified under Wine (the original's career at stage 2)**: the
  performance shop's first visit asks to save on leaving without a
  purchase because opening the Engine option (0x53B780 → 0x525050(3) →
  0x5243E0(7)) stamps kind 7's time (+0x13C) after the category's
  snapshot (0x55C697); the original's snapshot (0x851148 → 0xD254 bytes)
  against the profile it then saved differs only there (0 →
  0x005489C3). A later visit within the hour skips the line and leaves
  the profile unchanged: no question. Ported: the clone asks
  on the first visit only (test
  `perf_shop_engine_asks_on_the_first_visit_only`).

## Career audit (2026-10-31; the original under Wine side by side)
See also internal notes (the differences list for review).
* **Win handler order** (0x541430, **verified** from the assembly): the
  event's visit entry +6 = 1; then, when career +0xD26 is set and +0xD27
  clear (writers: see **Career audit round 2**), the bank (+0x7134) gets 0x532E60 (or
  the event's +0x30 with no current event +0x8730) and the stage record
  **of the event's own stage** (+0x37 × 0x104 + 0x72D8) the margin's
  points (0x513FE0 → 0x513430: +0xE0 += 0x512B50's points, +0xE4 += 1,
  skipped for DDAY_EVENT_A / B); then 0x5337E0 (career +0x4210),
  0x529930 (the inbox's win messages), 0x513390 (file the win, on the
  same stage record), 0x532C70 (the stage end, ECX still that record),
  0x5242D0 (Rachel) and 0x533B50 (world challenges). The win is filed
  before 0x541430 too: a race's end 0x54CA70 (when 0x5287B0(0) says
  won), the shop goal 0x54CB00 and the debug 0x541690 call 0x513390
  first, so 0x529930's win-count conditions (0x501370: 9 sponsor, 10 URL,
  12 normal) count this win (SMS_UNLOCK_01: stage 1, 2 normal wins →
  on the second win).
* **Reward class** (0x512AE0): 6 for +0x34 = 2; by type (circuit 1, drag
  2, sprint 3, drift 4, street X 5, special 0) for +0x34 = 3 or 1 or an
  SUV event (+0x0D = 1); 0 otherwise (the scripted category 0). Table
  0x7F7AC8 (7 rows, class 1–7): every class 1/2/5% → 100/200/300 but
  class 4 (drift) 10/20/30%; below the first margin, or class 0, 0
  (0x7D7D3C). The clone gave the scripted category points by type.
* **Seen under Wine** (fresh profile NAME, wined3d, memory read through
  the peek loop; career base 0x85AD40):
  - New career: cash 0, stage 0, visit table {DDAY_EVENT_A: +6 = 2}
    (2 = the current event), D-Day script states 0 → 2 → 5 (SMS_INSTRUCTION,
    read) → 9 once driven; inbox SMS_INSTRUCTION.
  - Car lot (CC_CAR_LOT_1, 0xEC17E618): the engage dialog needs Enter
    (Cancel selected); A won (+6 = 1), the shop's entry +4 = 1, +6 = 5
    (0x54CB00), B current (+6 = 2); no message for the lot. Taking the
    stock 206 sets +0x873D and +0x8740 = 0x319A8CA1, then MC save
    question → overwrite → "Save complete.". SMS_LOC_GO_TO_GARAGE (Rachel's
    car-lot hint) arrived ~6 s after the drive-out.
  - Garage (CRIB_1, 0x55446B8F, trigger centre (654.2, −101.5)): B won,
    stage 0 record +0xFC = 1, stage 1, +0x873F = 1, SMS_LOC_EVENTS_OUT_THERE
    (condition 2 on CRIB_1). Back in the city the open events are
    STAGE_1_CIRCUIT_1, drag C7483590, drift B0DEF68B, STAGE_1_SPRINT_1,
    street X 1CA81722 — the clone's `refresh_open` gives the same five
    in the same order.
  - STAGE_1_SPRINT_1 (engaged at its marker (−514, 311), won at 39.74 s,
    runner-up 1:56.78): at the finish the stage 1 record already holds
    the win (0x54CA70) while cash and reputation are unchanged; the
    reward screen shows 300 points and 250; both are added only on
    leaving UI_PostRace (cash 250, record +0xE0 300.0, +0xE4 1); the
    visit entry goes 2 → 1 then; then the MC save question again; the
    open events drop the sprint (no other stage-1 sprint). The clone's
    margin (1 − 39.74 / 116.78 = 66%) gives the same 300.

## Career audit round 2 (2026-10-31; **verified** from the assembly unless marked)
* **The race-result object** career +0xD24 (0x85BA64; constructor 0x528690):
  +0 cleared, +1 a career race under way, +2 it pays, +3 started from the
  World Map's event list, +4 the outcome settled / no handler, +0x50 / +0x54
  the racers, +0x34E4 / +0x34E8 the human racers (0xD24 each). A race's
  start (0x541170, and the intro events' 0x5410F0) runs 0x532EB0(event):
  +1 = 1, +2 = 1, +4 = 0 — but an event whose visit entry +6 is already 1
  (won) gets +2 = 0, +4 = 1. 0x5322F0 (the race setup from an event)
  clears +3 at its end; the World Map's "race this event" Yes (0x4E1284 →
  0x541170, 0x5325F0) runs it and then sets +3 = 1. The flow's track switch
  (0x6037D0 events 9 Restart, 10 Next Race, 16 a race's start → 0x581A00
  "BeginGameFlowSwitchTrack" → 0x581830 → 0x58193A) runs 0x5322F0 again,
  so a restarted World Map race pays. 0x500DB0 = +2 && !+3, 0x500DD0 = +4.
  Seen under Wine at an engaged circuit's start: D24..D28 = 00 01 01 00 00.
  The circuit lost (4th): at the results and on UI_PostRace the visit entry
  still had +6 = 2 and D24..D28 were unchanged; Return to Explore Mode
  asked nothing, set +6 = 4, cleared +0xD25 and put the car back at the
  engage point; the cash stayed 250.
* **Settling** (0x54C950): when +1 is set and +4 clear: +4 = 1 on a win
  (0x5287B0(1)), then the win handler 0x541430 (cash and reputation only
  with 0x500DB0) or the loss handler 0x528510 (visit +6 = 4 unless won;
  0x5242D0). Callers: 0x54C9E0 (Return to Explore, then 0x513A50, +0x8730
  and +0xD25 cleared, +0x8744 = 1) and 0x54CA50 (Back to World Map 0x4A3DC0,
  Quit to Main Menu 0x4A3E80; +0x8879 = 1). UI_PostRace (0x4F39A0):
  Return to Explore Mode 0x3411395C and Quit's OK 0x30ED2368 → 0x24DD40CF
  each, in a career (0x850078 = 1) with the Save option's prompts on
  (0x844AD8) and the race won (0x5287B0(0)), settle first (0x4F39A0 sets
  +0x150 for the first) and open MC_Main (0x4A3100 mode 0x441, callbacks
  0x35E413DD / 0xD78D7BF1); else they send those at once. Back to World
  Map (0x33C42E03) settles with no save; Restart (OK 0xE1A57D51 →
  0x5F41F0(9)) and a tournament abandoned (0x451E768E → 0x4A3DF0) do not
  settle. The post-race chain (0x4F3EA0) is built before any of this:
  UI_PostRaceReward only for a win with 0x500DB0 and not 0x500DD0;
  UI_SponsorPopup / UI_Sponsorship_new by 0x4A4610 on the unsettled state
  (the stage record already holds the win, 0x54CA70).
* **0x5337E0** (in 0x541430): milestones (0x83840C, 0x40 each) with +0x34
  = 2 whose +0x38 is the won event and whose visit entry +6 is 1 get state
  +4 = 2 (offered) and the marker table is rebuilt (0x533320). No milestone
  in the shipped data has +0x34 = 2 (all are 3 or 4; the maintainer's probe `milestones`),
  so it does nothing; the clone leaves it out.
* **Visit-table +6 / +4** (career +0x7B00; 0x512F10 finds or adds):
  +6 = 1 won (0x541430), 2 a race started unless won (0x541170) or the
  intro event (0x5410F0), 3 its race dialog declined (0x499C00: 0x911AB364
  or Cancel → 0x499BB0 → 0x5138F0), 4 lost unless won (0x528510), 5 a shop
  entered (0x54CB00). +4 = 1 when the car enters a marker's trigger
  (0x512FF0; seen on events too). A milestone offered has +4 = 2 (0x5331D0,
  0x5337E0, 0x533BF0); its photo shoot sets +4 = 2 and +6 = 1 (0x533690).
  Only +6 = 1 (0x500B80) and 5 (0x500460, 0x51295B) are read by pattern
  search; 2–4 are kept for the save. Under Wine: Decline at the circuit's
  dialog wrote 01000300, its start 01000200. The clone's native save had
  milestone offers at +6 = 2; fixed (`career::visit`, `Profile::tried`).
* **Save points** (MC_Main's save operation 0x40: 0x4F33A0 → 0x4E8D30(4)
  → state 5 "Are you sure you want to save the profile named %s?", or 6
  (save at once) with 0x844AD9 Autosave when the mode lacks 0x200):
  the car lot's state 6 (0x4ED8A0 / 0x4EDD70; only when 0x52A4D0 finds the
  profile changed since the 0x52A470 snapshot — a car taken always does —
  mode 0x141, whatever the Save option); UI_PostRace above (mode 0x441,
  only with prompts on). Clone: `Game::save_point` (see **Save points,
  records, Delete Tuned Car**, which also ports the shops' 0x552CB3 and
  the crib's 0x4AF383). Other 0x4A3100 save callers, **not ported** here:
  0x4F059D (mode 0x12141, in an unnamed screen's handler; 0x836370 +4 ≠
  2), 0x552CB3 (0x41, callback 0x552C30), 0x55E5D0 (0x1141, the dyno),
  0x5648EB / 0x5649E1 (0x151 / 0x51, tuning), 0x4AF383 (0x151).
* **The car lot's engage** (0x514FD0): only a running world challenge's
  goal shop engages at once; during D-Day the lot needs the engage key
  (Enter), as the earlier Wine run saw. The clone matches.

## Menus round 6: dyno, Custom HUD, status (2026-10-31; checked against the original under Wine)

Side by sides in $OB_PRIVATE/wine/compare/menus112 (orig left).

* **Dyno entry** (**verified**): Customize's Performance Tuning / Dyno
  item (0x55E7A0) asks 0x54B649F1 in GenericDialog_MED (0x78F2F8), the
  crib's (0x4AF780) 0xB10AE56F in GenericDialog_Small (0x790D74); both
  0x5587A0 with 1: No focused (Wine d0: only the 3D scene behind it, no
  Car Shop strip). Customize then asks the profile save (memory-card
  flow, left out in the clone). Yes → 0x5309F0 → 0x526980(1): the dyno
  track 4706 is loaded (L4RG; it has no Paths file, its grid is the
  markers' ForwardStartingPosition, slot 0 (−1682.30, −1957.06, 43.23),
  heading 21844) with the car on the rollers; the DynoEnter scene plays
  (Enter skips; not ported). Once in, 0x60B52C (session flag 0x89E7E6 and
  0x83A7A4 via 0x510050) pushes UI_PerformanceTuning_Setting (0x799B10)
  when no mode is chosen (0x510060: 0x83A63C = 0xFF) and 0x83A655 is clear
  (0x510080; 0x83A655's role **open**), else UI_PerformanceDyno_MAIN
  (0x7906F0). The clone: `Game::enter_dyno` / `load_dyno_track`; the
  tuning menus (Mode Select, dyno menu, tuning, sliders, graphs, chart,
  results and their dialogs) draw over the track while `tuning_session`
  (`tuning_menu_over_world`); Quit Tuning drops the track; Dyno Run after
  a test run reloads the dyno track (0x83A7A4 = 1).
* **Dyno camera 0x15** (**verified**, constructor 0x452600 from
  0x5F2EFF, update 0x44F300): an orbit about the car: the car's +0xD0
  orientation and +0x50 position (taken by the constructor into +0x70),
  RotZ(yaw +0x50 = 0xE270), RotY(pitch +0x40 = 0x70A0 + 0x8000; clamped
  0x70A0…0x8150), a step of the distance +0x2C = 7.2 (zoom +0x3C, 5.0…7.2)
  and the axis swap 0x4000 / 0xC000; the rates +0x58 / +0x48 start 0 (and
  are zeroed while the dyno manager's state is 0), fov +0xC4 = 8000.
  **Measured** in memory under Wine (mover via player 0x8900B4 +0x238,
  vtable 0x788070, camera 0x8760E0): car (−1682.08, −1957.33, 43.39),
  eye (−1675.75, −1959.47, 46.04): 7.19 away, 21.6° up, 41.3° off the
  back to the car's right, looking at the car; +0xC4 = 8000. The clone's
  `dyno_camera`. **Open**: what writes the rates (no key seen to move it).
* **Mode Select's question** (**verified**): a mode's option (react
  0x564EB0, option vtable 0x79B794) asks "Tune settings for this race
  mode?" (0x6BB7037E; 0x5587A0 with "" — sized by length — and 1: No
  focused); Yes 0xD05FC3A3 is the accept 0x55A750 handles (mode, its
  settings, then the chart the first time / the dyno menu); No
  0x34DC1BCF (0x538CB0) returns to the modes. The clone asked nothing.
* **Dyno chart under wined3d**: the original crashed (winedbg) going on
  to UI_PerformanceDyno_Chart from the question's Yes (Wine d6, twice the
  load); the chart, results and dyno menu were not captured this round
  (**open**).
* **Custom HUD picker** (**verified** by the frames, Wine h3): the
  colour picker UI_ChooseCustomHUD is embedded in UI_CustomHUDOverlay
  (0x555D10 mode 4), so the overlay (header, strip, "Custom Gauges", the
  gauge list, the gauge's name) stays drawn under it. The clone drew the
  picker alone; it now keeps the overlay (`Game::hud_overlay`).
  **Open**: while the picker is up (0x838560 > 1) the overlay's handler
  sets +0x28 = 0xADA7304B (0x1B2794C7 in the list state) and the list's
  dots show grey with a hollow highlight; not traced, the clone draws the
  list as when it is active.
* **Status screens** (**verified** unless marked):
  * The page bar: the master's constructor 0x4E1A30 builds a scroller on
    "Scroll_Area" / "ScrollBar" (0x542CB0); its handle (ScrollBar_Handle
    0x3FBB7A4E on ScrollBar_Backing 0x688A1551) sits left, centre, right
    on the three pages under Wine (s0–s2); the clone places it with
    0x538090's rule for (1, pages, page + 1) (**inferred** arguments),
    then hides the end arrows (0x4B1DF0). It stayed at the package's
    right before.
  * Heading rows: the row fill 0x4B1780 colours the row's label
    0xFFCAF48A and its backing its own RGB at alpha 0xDC when a cell has
    style bit 2 (0x4CC4A0 → 0x4AEA60(text, 0, 2): "Overall Status",
    "Races Won", …), else the label 0xFFFFFFFF and the backing alpha 0x7D.
    The clone drew headings white.
  * Slider rows: 0x4B181C's 0x512240(1) shows the slider's +0xC…+0x18
    (base, knob, an unset slot, handle: 0x540C20's descriptor at
    0x4EE9DE), not +0x1C / +0x20 (Slider_Beg / Slider_End), which the
    constructor's 0x512240(0) hid: no end caps (the clone showed a green
    End cap). The descriptor-to-field order is **inferred** (consistent
    with the frames). Not checked: the region page's slider (the Wine
    profile is in the intro stage, "Go to Car Lot" / "Go To Garage").
* **Not reached on the PC build** (from round 5's notes, rechecked): the
  Options carry no Trailers icon (Wine o3–o6: Audio … Credits); Wheel
  Options is greyed with no wheel (Wine o2), so UI_Wheel_Options is not
  reachable here.

## Menus round 7: post-race career screens, stage-2 status, DIM (2026-10-31; checked against the original under Wine)

Side by sides in $OB_PRIVATE/wine/compare/menus112 (orig left; the
`pr_*` and `*_s2` files are this round's). The original ran career3's
clone-written stage-2 save (s2a; the original loads it) and won
S2_SPRINT_3 by moving the car through memory onto its finish line
(-755.2, -638.6) and driving over it ("Winner! Your time: 12.78").

* **Won career race, the chain under Wine** (frames, **verified** as
  seen): "Winner!" → Event Results (over the winner crowd scene) →
  Race Reward ("Reputation Points Won: 300", "Bank Won: 300") →
  UI_SponsorPopup "SELECT A SPONSOR!" (stage 2's first win) →
  Sponsorships (five on offer, STREETGLOW first) → "Want to sign on with
  EUROLITE?" (No focused; the sponsor screen stays under the dialog's
  dim) → Post Race (Return to Explore Mode …) → the profile-save question
  (No) → free roam with the Bank Cash popup (1,250 + 300 + 1,275 bonus =
  2,875). The clone ports each screen (UI_PostRaceResults,
  UI_PostRaceReward, UI_SponsorPopup, UI_Sponsorship_new, UI_PostRace);
  the clone's shots `reward` and `sponsor` match the frames' layout
  (values differ with the shots' set-up); `sponsor_popup` matches but for
  the pulsing frame (below).
* **Sponsorship strip bar and arrows** (**verified**): UI_Sponsorship_new
  (and UI_RewardsSponsor, 0x4DF6F0) are the icon screen class 0x552340
  (0x543AA0's scroller on OPTION_MASTER / "option_"), so their
  "ScrollBar" handle and end arrows follow the strip's rule (0x538090,
  0x52FA80): Wine pr04 / pr05, the handle at the left on the first of
  five and a quarter along on the second, the left arrow off on the
  first. The clone skipped both for its single-slot screens (handle at
  the package's right, both arrows on); now it applies them.
* **The lot's DIM under the stock / trade questions** (**verified**):
  after the car lot (0x4EDB2B, states 2–5 by the jump table 0x4EE15C)
  asks through 0x5587A0 (Take 0xDA8F3E96, trade-in 0x85215BC6, trade or
  browse 0x3192F5D1), 0x4EDCBC plays "DIM" (0x78FEF4) on the lot's Event
  Handler 0x47FF4E7C (0x537C10). Its DIM script (0x9E99, length 0) sends
  0xA301C015 to all (target 0xFFFFFFFC); the lot's objects answer it
  (MsgR) with their own DIM: an alpha track from the base to 0 over
  100 ms (UNDIM 0x3D8EABC / 0x9F218E63 brings it back). So the lot's
  menu is gone under the question while the 3D car stays — Wine
  menus107 c14, which round 6 left as **inferred**. The car lot's Back
  questions (0x4EDE8B / 0x4EDED8) play no DIM (the lot shows, c24); DIM
  there comes only on 0x96DDA9E6 (0x4EDF00). The clone: `Dialog::
  dim_under`, the lot under the question fading out in 100 ms (the
  trade-or-browse question too since menus113: the lot rides as a third
  outcome no button reaches). 0x560B70 plays the screen's Event Handler DIM the same
  way (the Custom HUD overlay's questions, 33 call sites push "DIM").
* **Garage's "Return to Explore Mode?"** (Wine e0): only the 3D garage
  and its car behind the dialog, No focused; the clone matches (no
  screen under, `ask_in`).
* **Status at stage 2** (**verified**, Wine s0 / s1 against the clone's
  `status` / `status_region` on the same save): every value matches
  (3 %, Races Entered 1, Overall Wins 5, Win Percent 500.0 — the save's
  own counts; Region 0.00 %, 0/10, 0/3, 0/3, 0/1). The region page's
  slider is UI_Status_Region's own (0x497040: a 0x44-byte slider 0x4FF430
  on Slider_base / _slider / _handle / _beg / _end, margin 46): it shows
  its caps (white knob left, green Beg and End), unlike the career page's
  rows; the clone matches.
* **Custom HUD picker grey list** (still **open**, narrowed): the
  overlay (0x56BF80) builds a vertical PARTS_ICON scroller at +0x50
  (0x5435E0, "SideScrollBar"); Select on a gauge (0x81F0B71A) pushes
  UI_ChooseCustomHUD and clears the scroller's +0x2F, which 0x52EEB0
  reads to fade each item's alpha by its distance (0x50D8D0) — so the
  items keep their last colour. That alone does not explain the grey
  dots and hollow box (Wine h3); not traced further.
* **Reward Options** (Wine r0, stage 2, no magazines yet): View
  Magazines locked (padlock) and View DVD Covers; the clone has the
  icons (0xFF293517 with `or_locked`). Magazine / DVD reward frames were
  not reached (no cover shoot in this save; **open**).
* **UI_SponsorPopup** (0x4C1280 → 0x4A5250): the screen sets the package's
  input mask from 0x83A9D8 (0x537960) and makes the icon B26B6D43 the
  current button (0x52CEB0 → 0x52CE50 → 0x52B3B0). The green frame
  DF422345's INIT chains to the looping 5A8E4EBE (1.2 s: full alpha to
  320 ms, out by 640 ms, off to 1000 ms); the clone pulses it as the
  package says. Wine pr03 showed no frame in three samples taken about
  3.6 s apart (= 3 loops, so one phase): **open** whether it shows.
* **Shop welcome text** ("welcome back" in the queue): the shop engage
  dialog shows `<SHOP>_BODY` ("Welcome to the Bayview Graphics Shop,
  South Market store.", Wine pr09 on a revisit of CC_GRAPHICS at stage
  2); the clone's ShopEngage shows the same label. No other "welcome
  back" text was found in the language table (only `SHOP_BLURB_*` labels,
  not referenced by name in the exe; their use is **open**).
* **Graphics shop camera under Wine** (observation only): about 90 s
  after entering, two frames 4 s apart differ by RMSE 3.5 %, the next two
  by 0.4 %; the camera's idle motion was not traced (the FE camera mover
  is passed in ECX to 0x4A5B70; its instance not found).

## Career audit round 3 (2026-10-31; **verified** from the assembly unless marked)
* **Event movies** (event record, 0x88 bytes): +0x00 u16 the name, +0x02
  u16 the movie before the race, +0x04 u16 the movie after a win, each
  an offset into the career string pool (0x838428). The clone read the
  name as a u32, so the eleven events with a movie had no name (S2_URL_1
  among them); fixed (`Event::movie`, `Event::won_movie`). In the data:
  before — STAGE_1_DRIFT_1 SCENE20, STAGE_1_DRAG_1 DRAGTUT,
  STAGE_1_SHORT_1 STREETXTUT, S3_DRIFT_7 ORGANICSTUT, S2_URL_1 SCENE17,
  S3_URL_4 SCENE18, S3_URL_6 SCENE09, S4_URL_4 SCENE10, S5_URL_4 SCENE11,
  S5_URL_8 SCENE13, S5_CIRCUIT_36 SCENE14; after — S2_URL_1 SCENE08,
  S5_URL_4 SCENE12 and the stages' last races S2_URL_3 SUBURBAN,
  S3_URL_8 INDUSTEAST, S4_URL_7 INDUSTWEST.
  - **Before** (the race dialog's Accept 0x499B50): with the event's +0x02
    name not empty (0x4FFDD0) 0x53D640 clears the queue 0x8382A0, puts
    the name in, then "DriftTut" after "Scene20" (0x43DCC0, a compare
    without case), game mode 0x838568 = 1, IG_PlayMovie.fng; the queue's
    end runs 0x54AF90 case 1 → 0x499B10 (0x541170, the race). Without
    one, 0x499B10 at once.
  - **After** (UI_PostRace's Return to Explore Mode, 0x4A3E30 →
    0x54C9E0, the only caller): the event's stage (+0x37) is read, the
    outcome settled (0x54C950), then 0x513A50(event stage ≠ career
    stage, buffer): nothing for a race from the World Map's list
    (+0xD27); the event's +0x04 name when it is not empty and the event
    is won (0x500B80); "Scene07" when the stage changed and is now 2
    (stage 1 has no last race); for S5_CIRCUIT_36 (0x04CCE3E1)
    "Scene15" won, else "Scene16". With a name 0x53D7B0 queues it
    ("UpperClass" after "Scene07"), mode 4, 0x43B640 on 0x827E88
    (**open**), IG_PlayMovie; the queue's end runs 0x54AF90 case 4 →
    0x5F41F0(0xB, 0), the same flow event the exit sends without a
    movie (0x4A3E5F). Back to World Map and Quit (0x54CA50), the race
    pause's Return to Explore Mode (0x451E768E → 0x5F41F0(0xB) →
    0x513950) and a tournament left early (0x4A3DF0) play nothing.
  Clone: `Game::engage_accepted` and `Game::explore_movie` on
  `MovieScreen` modes 1 and 4 (`race_event` = career +0x8730).
* **Quitting a race from its pause runs no handler** (corrects round 2's
  open item): UI_pause (0x4C2170) on 0x451E768E (Return to Explore
  Mode's OK, 0x4C2780) sends 0x5F41F0(0xB) and its flow (0x606740 case
  0xB) runs 0x513950 (+0x8730 = 0, +0xD25 = 0, +0x8744 = 1); Quit to
  Main Menu's OK 0x30ED2368 → 0x4A7CF0 (0x5F4250(6), 0x5139C0); Back to
  World Map 0x33C42E03 → 0x4A7CC0 (0x513B30). Only UI_PostRace calls
  0x54C9E0 / 0x54CA50 (0x4A3E30, 0x4A3DC0, 0x4A3E80). So the visit entry
  keeps +6 = 2 and nothing is paid or lost; the clone matched already.
* **A Restart after a win keeps the stage record's entry**: the race's
  end filed it (0x54CA70 → 0x513390: category 3 not SUV at +0x00, 35;
  category 1 at +0x8C, 4; category 2 at +0x9C, 10), Restart settles
  nothing, so the win counts for the stage's win counts, the sponsor's
  races and the messages while the visit entry is not won (the event
  stays open, 0x500B80). Clone: `Profile::filed` (stage records' lists
  in the save; read back as filed when the visit entry is not 1);
  the race's end files there, the win handler moves it to `completed`.
* **Tuning's save points** (UI_PerformanceDyno handler 0x5647F0): Quit
  Tuning's Yes (0x30ED2368) with 0x50FD10 (the tuning object ≠ its
  mode's working settings, 0x50FCA0, or any of the five working settings
  (+0x904) ≠ the car object's (+0x360)) and save prompts on (0x844AD8)
  asks 0x71851654 (0x5587A0, Yes 0xF770 / No 0x061C, No focused): Yes
  0x24DD40CF commits (0x50FAE0) and saves (0x4A3100 mode 0x151), No
  0x1FAB5998 commits and quits (0x53CC80); else it quits. Save Profile
  (0x5226D0: icon 0x73FE99B7, label 0x410258C3, added when 0x50FD10
  holds as the menu is built, 0x522961) asks 0x0D7DE8AB; Yes 0x51123592
  → 0x4A3100 mode 0x51. Clone: `Game::tuning_changed` (the clone keeps
  the working settings in `Profile::tuning`, the car object's as the
  dyno found them in `tuning_entry`), `LotDeal::QuitTuningSave`,
  `LotDeal::TuningSave`. **Open**: Customize's dyno (0x55E530 on
  0x34DC1BEC: changed since 0x552CD0's snapshot, or 0x83AA00 clear →
  0x4A3100 mode 0x1141, else 0x461A18EE); 0x83AA00 has only a clearing
  writer in code (0x5124F8), so the save question would always come
  (**inferred**; the clone's main-menu Customize keeps its car apart from
  the profile and asks nothing). 0x4F059D (mode 0x12141) is online.
* **Signing a sponsor goes on to UI_PostRace** (UI_Sponsorship_new's
  handler 0x4C1340 on Accept 0xD05FC3A3: 0x5112A0, 0x532D00(the
  sponsor at +0x58 → +0x48), then 0x4BFF80 on the post-race chain 0x8363DC, its next
  screen): the race is settled by UI_PostRace's exits as any other. The
  clone went to the city from the sign, so the win was never settled (no
  pay, the visit entry not won); fixed (`Action::Sign` → UI_PostRace).
* **The sponsors' reputation test reads the previous stage** (0x4A4610
  and UI_Sponsorship_new's 0x4C1576): 0x5005E0 (+0xE0 / +0xE4, 0 with no
  race) runs on 0x861F14 + 0x104 × stage, the record before the current
  one, while the win count (0x5004D0) is the current stage's. Under Wine
  (a clone stage-2 save, stage 1 at 1500 points in 5 wins): the first
  stage-2 win (S2_SPRINT_3, filed, not settled) brought UI_SponsorPopup
  and all five stage-2 sponsors (STREETGLOW, EUROLITE, EDELBROCK, TOYO,
  YOKOHAMA); the clone, testing stage 2's own (still empty) record,
  offered STREETGLOW alone. Fixed (`Profile::sponsor_offers`). The
  sponsor's react (0x4C14C0) asks "Want to sign on with %s?" (0x39DF9562
  with SPONSOR_<name>, 0x5129F0's "SPONSOR_%s"; Yes 0x88579775 / No
  0xBE503981, No focused) before the sign (0xD05FC3A3); added. Then
  under Wine: cash + the signing bonus at once (1250 → 2525), the stage
  record +0xE8 set, UI_PostRace; Return to Explore Mode settled the win
  (2825, record +0xE0 300 / +0xE4 1, one message more), asked the save
  question, and the city's open events were the 12 kept, then the three
  sponsor circuits, then S2_URL_1 — the clone's order now that the sign
  no longer rebuilds the list (0x532D00 does not; the settle's does).

## Career audit round 4 (2026-10-31; **verified** from the assembly unless marked)
* **Event markers and the engage key** (the stage-1 save with only
  STAGE_1_SPRINT_1 left, round 3's open item): the marker table (career
  +0, 64 × 0x18: trigger, event, shop, milestone, challenge, flag; built
  by 0x533320) held the sprint at its trigger 0x96021656 (kind 13, the
  8 × 8 m box (−518, 307)–(−510, 315), next to the Hotel Plaza garage,
  where Resume puts the car). The engage is the key: the free-roam
  trigger pass 0x5EB050 finds the kind 13–15 trigger under the car
  (0x5D9C80 mask 0xE000) that has a marker entry (0x527810), and only
  when it differs from the last one (+0x9AC) leaves the old (0x5EAF40)
  and enters the new (0x5EAFC0: shop visit 0x512FF0, then 0x532A60 — an
  event's entry passes while it is in the open list career +0x71C8,
  0x5007E0 on career +0x7148 + 0x80 — then the prompt 0x4CA020).
  Under Wine the car standing in the box showed the HUD's engage icon;
  Enter brought the race dialog (Frank, Sprint, 2.762 mi, purse 250).
  Round 3 had not pressed it: nothing was missing.
* **The stage change, seen under Wine** (s1z, the sprint won by dense
  teleports, 33.86 s): the race's end filed it (stage record 1 +0x00 4 →
  5) with cash and points unchanged; Race Reward 300 / 250; UI_PostRace's
  Return to Explore Mode settled it: cash 250 → 500, record 1 +0xE0 600
  / +0xE4 2, +0xFC = 1, stage 2, the visit entry +6 0 → 1, three
  messages (SMS_UNLOCK_01, SMS_UNLOCK_02, SMS_LOC_TEXT_BEACON_HILL), the
  shop BE4383FA open; the open list still held the won sprint. Then the
  save question, the overwrite question, "Save complete."; then the
  movie queue 0x8382A0 = "Scene07", "UpperClass" (mode 0x838568 = 4);
  then the city with the mailbox up (Beacon Hill first) and 13 stage-2
  open events (2 circuits, 1 drag, 2 drifts, 2 sprints, 2 street X, the
  four H_ races). The clone's settle of the same save
  (`WIN=1FD60BD2 career_saves s1z.sav`) gives the same cash, record,
  stage, messages and shops, and 13 events of the same slots after the
  city start; which circuits / drifts / sprints / street X are drawn
  depends on the session's random state.
* **When the open events are rebuilt** (round 3's open item): 0x527D40
  has three callers — 0x533320 (the marker table), 0x5291F0 (from
  0x533320 and the debug 0x541800) and the debug 0x541690 — and 0x533320
  runs from the world start 0x5412E0 (0x60A690, every free-roam load in
  a career: 0x500B60 = game mode 1 and no race 0x89E7B0; it also offers
  the DVD covers first, 0x5331D0), the outrun result 0x541330, the photo
  shoot 0x533690, milestone offers 0x533760 / 0x5337E0, a world
  challenge's start 0x533BF0 / give-up 0x533D00 / rewards 0x541970 /
  0x5419D0, Rachel's reveal 0x53E0C0, the D-Day step 0x5342A0 and the
  race load's 0x541A60 (only with its +0x156A8 = 1). The win handler
  0x541430 is not among them: a settled win stays in the list (and the
  marker table) until the next city start. 0x527D40 clears the list first
  and then returns early (stage 0 before the reveal, a world challenge
  running, the final event won), so those leave it empty. The native
  save does not keep the list (pointers; the clone reads it as empty).
  Clone: `Profile::win_paid` no longer rebuilds; `Game::explore` runs
  0x5331D0 (`offer_covers`) and the rebuild at every career city start;
  `Profile::outrun_result` rebuilds at its end. `Profile::win` (the
  walking helper of the tools and tests) still rebuilds after the win.
  Also fixed: a win without the payout gate (+0xD27, a World Map race)
  counted a race at the stage record's +0xE4 with 0 points; 0x513430
  does not run then (`settle_race` passes no reward).
* **Customize's dyno save point** (0x55E530 on 0x34DC1BEC): 0x83AA00 is
  profile slot 0's +0x20, "a profile is current" (1 under Wine with NAME
  loaded; written through the profile object, not by address; 0x512360
  fills in a default name only while it is clear). So with a
  profile the save question (MC_Main mode 0x1141) comes only when the
  car changed since 0x552CD0's snapshot (0x52A4D0 on 0x83A9D0), else
  0x461A18EE leaves at once; without one it always comes. Round 3's
  "would always come" is corrected. **Ported** 2026-11-04 (menus pass,
  observed under Wine: docs/save-points.md "Customize's dyno save point"). 0x4F059D
  (mode 0x12141) lies outside any analysed function; it is not ported
  (**open** whether it is reachable offline).

## Menus round 8: DVD reward, controls capture, sponsor signing, DynoEnter, vinyl-colour view (2026-10-31; checked against the original under Wine)

Side by sides in $OB_PRIVATE/wine/compare/menus113/sbs (orig left).
The original ran career3's clone-written stage-3 save (s3a) with
STAGE_3_DVD_1 (kind 4) set offered in the save's visit table and, in
memory, the milestone's visual rating (record +0x3C) set to 0 so the
offer stuck; the car moved through memory onto its spot (661, −537).

* **DVD cover shoot, kind 4** (**verified**, Wine w13–w16): the spot's
  engage gives an SMS "View Your DVD Cover" (Stan, "You made it on to the
  cover of a DVD. Go check it out!"), then UI_MagazineReward with the
  composed cover (Four on the Floor: background, masthead, model, the car
  and "DRIFT SUSPENSION SETUPS") and Continue; Accept asks "Do you want to
  save this DVD?" (No focused; the old **inferred** "DVD for kind 4" is now
  seen); No → free roam, "Bank Reward for DVD: 500" with the Bank Cash
  popup. The clone's `dvdreward` shot matches the layout (cover box and
  Continue); its random picks (model, camera) differ from this run's.
* **UI_MagazineSelect's MagThumb** (**verified**): see the
  UI_MagazineSelect entry; the clone showed no logo (it hid MagThumb).
  View DVD on a cover whose screenshot was never taken: "Unable to load
  NAME DVD 1." (Save/Load, Continue) and back to the list (Wine v3 / v4).
* **Crib reward screens over the garage** (**verified**, Wine v0 / v2):
  Reward Options and the magazine / DVD lists stand on GarageMain with
  the car (the crib pushed it; nothing pops it). The clone drew them on
  black; now `garage_request` includes Rewards, Covers and Signed
  Sponsors.
* **Controls key capture** (**verified**, Wine o5 / o6 / o8):
  Accept on a cell (0x4F93CC) sets +0x150, runs "Pulse" on the cell's
  text (0x51CF80(text, "Pulse", 1); the package's 1.2 s loop from grey to
  green) — the key's name stays — and puts "Press a button to use for
  this function" (0x23CF49AA) on Instructions_Label; after a key is
  taken (+0x14F, 0x4F989A) the label reads "Press enter to assign a new
  key or button" (0xFED0D3DF, 0x4F982B). The clone showed "..." in the
  cell and kept "Configure your settings"; it now pulses the cell and
  switches the label (`controls_capture` shot).
* **Sponsorship sign question** (**verified**): UI_Sponsorship_new's
  Select (0x4C14C0) asks "Want to sign on with %s?" (0x39DF9562; buttons
  0x88579775 / 0xBE503981, Yes 0xD05FC3A3, No and Back 0x34DC1BCF, No
  focused) only when the stage record's +0x100 (0x862100 + stage ×
  0x104) is empty, then FORWARDs 0x96DDA9E6; Yes (0x4C1462) → 0x5112A0,
  0x532D00 (sign the selected sponsor), 0x4BFF80 (the chain on to
  UI_PostRace). The clone signed at once and went back to the city; it
  now asks over the screen and goes on to Post Race. The name's source
  (0x5129F0 → 0x4FFA80) is not traced; the clone uses SPONSOR_<name>
  (**inferred**, matches "EUROLITE" under Wine).
* **The won race's chain in one shot** (`game --shot winchain`): a
  stage-2 career sprint won, then Accept through Event Results → Race
  Reward → SELECT A SPONSOR! → Sponsorships → sign question (Yes) → Post
  Race, each screen checked (`NFSU2_CHAIN_STEP=n` stops earlier). The
  sign question and Post Race match Wine pr06 / pr07.
* **Vinyl-colour camera** (**verified**): Wine a17 / a18 (Graphics Shop →
  Vinyls → Hoods → Hood Scoop; then its colour swatches): the colour
  swatches keep the vinyl list's view. The list's update (0x567A60 →
  0x50E580(slot, part) → 0x4C2100 → 0x4C1D40 → 0x4A5510) passes the
  highlighted part, whose kind (+5 & 0x1F) picks the vinyl view; the
  swatches (0x55CAD0 / 0x55D670) request the colour slot 0x4D.., which
  has no record, so the view stays. The clone requested slot 0x40 with no
  group (the generic vinyl record) — wrong for groups 0x14 (hoods) and
  0x16–0x1B; it now passes the part's group, and the `vinyl_colours`
  shot stands on the vinyl's view.
* **DynoEnter** (Wine e1–e8, about 8 s: the car comes down the main
  straight, through the garage and up onto the rollers; then the tuning
  menu): ported as `SceneKind::DynoEnter` (`Game::enter_dyno_scene`):
  the car and the camera track (the scene hash 0xB3B2B197's "Track 0")
  as the shop scenes; at the end the car is put back on the grid with the
  dyno camera and the first tuning menu opens. Type 2 takes its origin as
  a start scene (0x43B000: 0x431990 with the first flag, as type 0); the
  clone stands it on the dyno grid turned 90° more (**inferred** from
  the frames: the garage end matches; the early straight still looks at
  another part of the circuit — **open**). Shots `dynoenter`
  (NFSU2_SCENE_MS) and `dynoenter_end`.
* **Dyno Chart under wined3d** (**solved** in round 9: the crash was the
  WidescreenFix ASI's patch, see "Menus round 9"): the original crashes
  in the chart's construction (factory 0x55AF30 → 0x557A20 → 0x53D0B0:
  after the graph 0x5202C0 is built it returns into 0x53D13D, mid
  instruction — "illegal instruction"), from the Mode Select question's
  Yes as before and with a mode already chosen (0x83A63C poked 0) from
  the dyno menu's path alike. Chart, results and the dyno menu's frames
  remain uncaptured.
* **SELECT A SPONSOR! frame** (**inferred**, not re-captured): DF422345's
  INIT → the looping 5A8E4EBE is the same script as UI_Main's and
  UI_MagazineSelect's icon frame, which pulse under Wine (this round's v2
  shows UI_MagazineSelect's frame lit); Wine pr03's three samples 3.6 s
  apart fall on one phase of the 1.2 s loop (dark 640–1200 ms), so they
  do not show it off. The clone keeps the pulse.
* **Unused packages** (the maintainer's probe `packages` against the clone's sources;
  the exe's name strings and their references): online only (UI_OL*,
  PC_OL_*, UI_DateEntry 0x4BA3BB for the age entry, the "animating"
  dialogs — 0x526C50 picks GenericDialog_Animate_* only for the request
  "animating", 16 call sites; 0x4BB1FF / 0x4D43EF / 0x4E4D91 are server
  waits); no reference in code (IG_GenericDialog_LARGE, LS_Chinese_Health,
  LS_Demo_*, LS_LangSelect, MW_LS_*, UI_DebugTest, DEMO_ShopPreview,
  Demo_MiniMap_Legend, Loading_Controller, PC_ContextHelp,
  UI_DeleteProfile, UI_PostRaceWorldMap, UI_ShowCase_DPAD,
  UI_ShowCase_Magazine, WorldMapMain: no name string); HelpDialog_LARGE
  is only tested for (0x55B8C1, 0x52CF70) — 0x526C50 never returns it.
  **Open**: UI_DecalsOverlayInvis, the package UI_DecalMain's +0x16C
  names at construction and on its state changes (0x553147, 0x5600F5,
  0x5683FF, 0x56B87E) while UI_DecalsOverlay.fng is reached only through
  data (0x7F8768, 0x7FA1E4); the clone draws UI_DecalsOverlay (which
  matched the decal lists under Wine, menus103). The split-screen
  packages (2PHud*, 2P_PressStart 0x4FA433, MU_QuickRaceCarSelect) and
  UI_VirtualKeyboard (0x551ED3) are not checked for reach on the PC.

## Career audit round 5 (2026-10-31; **verified** from the assembly unless marked)

* **Sponsor money** (0x541430, 0x532E60, 0x532D00, 0x500C30, 0x5009E0):
  a sponsor is signed per stage (stage record +0xE8, 0x532D00 only when
  none is signed in the current stage: the next car slot 0x5284E0 and the
  signing bonus +0x0C, once). A won race pays (0x541430, payout gate
  +0xD26 / +0xD27) 0x532E60 — only in an event (0x863470) the player won
  (0x5287B0(0)): for a sponsor race (+0x34 = 1) with a sponsor signed in
  the **current** stage, that sponsor's +0x02, else the event's +0x30 —
  and files the points (0x513430, D-Day events skipped). With no current
  event (+0x8730 = 0) it pays the event's +0x30 and files no points. There
  is no contract-end payment: the stage records' +0xE8 are read only by
  0x500C30 (the purse shown, same rule as 0x532E60), 0x5009E0 (counts the signed
  sponsors), 0x528F81 (a stats fill that counts them too) and the debug
  0x541690. The
  engage dialog's purse is 0x500C30's: the clone showed the event's +0x30
  (fixed, `Engage::purse`).
* **Tournaments** (URL events with 2–3 races): the winner (0x528720 via
  0x5287B0) is the racer with the most points summed over the races in
  state 3 / 4 (0x513EB0; table 0x7F7B8C 8, 6, 4, 3, 2, 1, 0); a tie goes
  to racer id 0, the player. The clone's stable sort (player first) gives
  the same. Payout: one 0x532E60 for the event after its last race.
* **Photo shoots** (0x541940 → 0x541220): bank += milestone +0x26 +
  ftol(stage def +4 × visual rating 0x528420 + 0.005) (the clone's
  `cover_reward`), the HUD popup 0x4AC170, then 0x5410A0 on the current
  stage record: the milestone's hash goes in the record's six-dword list
  +0xC4 (first empty or equal slot; none past six) and the stage end
  0x532C70 runs — before 0x533690 sets the milestone's +6 = 1, so 0x513490
  does not count that milestone yet. The clone now writes +0xC4 in the
  native save (base order kept, new ones in hash order: **inferred**, the
  order shot is not kept) and runs the stage check there.
* **Outrun end** (0x541330): career only (0x850078 = 1); a win adds stage
  def +0x02 and counts +0xDC, a loss takes +0x02 off (floor 0); then
  0x529BE0 (SMS_OUTRUN_VICTORY / DEFEAT, each once: 0x514CB0), 0x533AA0
  (win only: condition-2 triggers equal to +0xDC), 0x5110C0 (Rachel's
  line; draws Rand(2) when already spoken), 0x533320, 0x4F92A0 (the
  HUD's marker list rebuilt). All in the clone.
* **The last stage** (0x532C70 → 0x528330): stage 5's definition has a
  final event (+0x28, S5_CIRCUIT_36); once it is won with the stage's
  media milestones, +0xFC is set and the career moves to stage 6
  (0x7AF8), which has no definition: 0x527D40 opens nothing and 0x513D30
  offers outruns without a cap (stage > 5). **An outrun's bank in stage 6**
  (**verified** 2026-11-01, small171): 0x541330 reads the stage record's
  +0x100 (career +0x72D8 + 0x104·stage, records 0..7), which 0x500700
  sets from 0x500190(stage) — the definition whose byte +0 is the stage
  among the 6 at 0x83841C (0x838418 = 6; stages 0..5) — or 0; with 0 a
  win adds 0 and a loss takes 0. **Observed** under Wine (a community
  stage-6 save, Sawa): records 0..5 +0x100 = the definitions, records 6
  and 7 = 0. So after the career's end an outrun pays nothing, as the
  clone's `outrun_result` (no stage-6 definition) already does. The clone stopped at stage 5
  (`check_stage` returned for stage ≥ 5); fixed. The clone's other stage
  users already take 6 (`outrun::offered`, `world_map_max_zoom`, the
  stage-up line, BARRIERS_CAREER<n>).
* **Under Wine** (wined3d, Xvfb :165, `a private Wine copy`): (1) the
  original's own stage-2 save
  at the city start: the marker table held the shops CC_CAR_LOT_1,
  CC_MEGALOW, CRIB_1 and UC_CAR_LOT only (the other stage-2 shops are
  announced, +0x51, and need driving into first) — the clone's
  `shop_open` gives the same four; no DVD offered (stock car, rating 0).
  (2) A clone-written stage-2 save with EDELBROCK signed (`SPONSOR=EDELBROCK
  PLACE=156,-646,12 career_saves`, s2b): loaded with the sponsor's record,
  the city start opened 17 events — 2 circuits, 1 drag, 2 drifts, 2
  sprints, 2 street X, the three S2_SPON_DRAG, the four H_ races, S2_URL_1
  — the clone's rebuild of the same save the same per slot. The
  S2_SPON_DRAG_3 dialog (Cancel focused; its message differs after a
  decline, a new random pick) showed "Purse: 875" (the sponsor's +0x02).
  Won by teleport: the race's end left the bank and points alone; Event
  Results filed it at the stage record's +0x8C; Race Reward 300 / Bank Won
  875 with the help bar on 2,175; Return to Explore Mode paid 3,050 and
  filed the points (+0xE0 600, +0xE4 2), visit +6 = 1, no message, then
  the save question. `WIN=E1A8ACB2 career_saves s2b.sav` gives 3,050, the
  same points and no message. The original's save after it
  (`career5-tmp/orig-s2-spondrag-won.sav`) loads in the clone with the
  same state.

## Menus round 9: the dyno chart and results, DynoEnter's origin, the Wine setup's code patches (2026-10-31; checked against the original under Wine)

Side by sides in $OB_PRIVATE/wine/compare/menus114 (orig left). The
original ran the a private Wine copy prefix (wined3d, Xvfb :160) with its
stage-3 save: garage (car moved through memory into CRIB_1's trigger box
(643.6…664.7, −112.1…−90.9), Enter engages) → Performance Tuning / Dyno →
Yes → DynoEnter (Wine de1–de30) → Mode Select (Circuit) → Yes → chart
(ch1–ch24) → Results (r1) → Dyno Shop (r2) → Dyno Run again (cj1–cj30).

* **The Wine setup patches the game's code** (**verified**, memory
  against the file): the game folder carries ThirteenAG's WidescreenFix
  (dinput8.dll ASI loader, SCRIPTS/NFSUnderground2.WidescreenFix.asi;
  run.sh forces `dinput8=n,b`). It rewrites 66 places in .text (list in
  <work dir>/wsfix_patches.txt: jumps into the fix,
  changed constants), among them:
  * 0x53D133..0x53D13A, inside UI_DynoChart_Build (0x53D0B0): the
    graph's position pointer and the call to 0x5202C0 become the floats
    79.0 / 60.0 (a pattern match gone wrong), so the code returns into
    0x53D13D (the last byte of the call, `FF EB`): the "illegal
    instruction" of rounds 6–8. Writing the file's bytes back
    (`write 0x53d133 c0a37f008bc8e882` through srv141's peek loop) after
    the launch makes the chart, the results and the dyno menu run. The
    renderer was never the cause.
  * 0x536A2F: FE object type 7 (FE_DrawMovie 0x536860, from
    FE_DrawObject 0x5370B0) draws its quad at ±0.6667 instead of the
    file's ±0.5, so FE movie objects are a third larger under this Wine
    setup than in the plain game. Earlier comparisons of FE movie objects
    against Wine need this in mind.
  * 0x453594 (ChaseCamera_Update 0x453570, mouse look) and others in the
    renderer (0x5BE858…0x5D2B75), 0x4FFA80 (the string lookup; gamepad
    button names) and 0x50516B / 0x50B460 / 0x511BD9 / 0x512570 (FE).
    None touches the FE camera mover (the shop camera's slow drift stays
    **open**; the fix's camera hook is the chase camera only).
* **Dyno chart** (**verified**, Wine ch1–ch24 / cj1–cj30 against the
  `dynochart` / `dynochart_end` shots):
  * The graph's rectangle (0x7FA3C0 / 0x7FA3C8) is x 79, y 73 (the file
    holds 79.0 then 73.0), 486 × 312; the clone had (73, 79), so its
    curves sat 6 units left and low. Fixed.
  * The curves and the sweep line are Direct3D lines: 0x50DE90 →
    0x50DCD0 → 0x5C5760, DrawPrimitiveUP type 3 (line strip) with one
    line, so one pixel wide at any resolution (1 px at 1280 × 960 under
    Wine). The clone drew 1-canvas-unit quads (2 px at 1280 × 960); it now
    draws hairlines (`HAIRLINE_UV`, one target pixel; the diamond rule's
    end pixels may differ slightly).
  * Chart_Legend: its INIT chains to HIDE, and only the pull's end runs
    Show (0x54A630): no legend during the pull under Wine; the clone drew
    the package's placeholder ("280 HP @7000 rpm") from the start. Fixed
    (`chained`).
  * The end: "141.6 ft-lbs @3,900 rpm", "138.0 bhp @6,250 rpm" for the
    stock 206 in the original and the clone alike; the curves overlay
    within a pixel.
* **Dyno results' rating bars** (**verified**, Wine r1): the
  constructor 0x557BE0 builds three sliders and 0x4A22A0 binds
  accel_ / top_speed_ / handling_ backing, handle and cur (range 0..1,
  pads 46, as the car lot's); 0x54A980 ends with 0x5103D0, which sets them
  to the shops' ratings 0x5B53B0 / 0x5B6A30 / 0x5B54A0 (clamped). The
  round-6 note ("never binds them") was wrong. The clone kept the
  package's bars; it now lays them out with the car lot's `lot_bar` from
  the shops' ratings of the car as fitted (which car record 0x5103D0
  reads is **inferred**; the three bars match Wine r1 to the pixel for the
  stock 206: about 0.1, 0.03, 0.3). The figures (6.32 Sec., 21.00 Sec.,
  125.0 MPH, the peaks) match.
* **Dyno Shop menu** (Wine r2): matches the clone's `dyno`.
* **DynoEnter's origin** (**verified**): 0x43B000 calls 0x431990 for
  scene type 2 with param_4 = 1 and no first flag (type 3 alone passes
  the flag), so 0x42FC60 finds "ForwardStartLine" (0x89E7A4 clear,
  0x86501C unset) and returns its +0x10 / +0x14 and +0x20 + 0x4000, which
  0x431990 turns by −0x4000: the start line's place and heading, as a race
  intro's. The clone stood the scene on ForwardStartingPosition slot 0
  turned 90° (round 8's **inferred** guess, wrong for the early straight);
  it now uses the dyno track's ForwardStartLine (`dyno_start_line`). The
  frames now follow Wine de2–de16 along the whole path (the main
  straight towards the bridge, the pits, the garage, the rollers); the
  clone's clock runs ahead of the slow wined3d frames.
* **UI_DecalsOverlayInvis** (narrowed, **open**): the decal screen's
  +0x16C is "UI_DecalsOverlayInvis.fng" (0x799C38; set at 0x553147,
  0x5600F5, 0x5683FF, 0x56B87E), and UI_DecalMain_OpenOverlay 0x556880
  pushes it embedded (0x555F20 → 0x555D10 mode 4) for slots 0x34–0x3E /
  0x61–0x98. Both names sit in the package table (0x7F8768: Invis and
  UI_DecalsOverlay share the factory 0x56B910). Drawn with its INIT
  states the Invis package does not match Wine's decal list (it shows
  Total Cost and no "No Decal" bar; menus103 decal_windshield), while
  UI_DecalsOverlay does, so the clone keeps UI_DecalsOverlay; how the
  Invis package's objects are set by its screen is not traced.
* **Split screen on the PC** (**verified**, not reachable offline): game
  mode 4 (the 2P packages: 2PHud*, 2P_PressStart at 0x4FA433 only in mode
  4, MU_QuickRaceCarSelect) is set only at 0x54045C when g_NumPlayers
  0x7FAA1C is 2; the file holds 1, 0x53FF19 sets 1 and only the replay
  stream reader 0x5801FC sets another value. No menu sets two players
  (the PC main menu: Career, Quick Race, Online, LAN, …; Wine y15 / y16).
* **UI_VirtualKeyboard**: its name (0x799D54) is referenced only by a
  name test (0x551ED3, after GarageMain and others in the 0x838528 check)
  and the package table entry 0x7F8554 (factory 0x4EFD40, hash
  0xCAF6D39E, which no code uses): not pushed on the PC (**verified** by
  the references; the PC profile name is typed).
* **Custom HUD picker's grey dots** (narrowed, **open**): the overlay's
  handler 0x563D00 sets the screen's +0x28 to 0xADA7304B while the picker
  is up (0x838560 > 1) and 0x1B2794C7 otherwise (also 0x56C37E); neither
  value is a script name of UI_CustomHUDOverlay / UI_ChooseCustomHUD, and
  the reader of +0x28 is not found.

## Menus round 10: the garage's idle tour, the Custom HUD list while picking, the magazine shoot under Wine, UI_DecalsOverlayInvis (2026-10-31; checked against the vanilla original under Wine)

Captures in $OB_PRIVATE/wine/compare/menus115/orig. The original ran
the a private Wine copy prefix with the WidescreenFix off (dinput8.dll renamed;
memory 0x536A2F = 00 00 00 3F, 0x566052 the file's bytes). Without the
fix the game keeps its profiles in the prefix's
`AppData\Local\NFS Underground 2\<NAME>\<NAME>` (the fix's ini had sent
them to `game\save`). The game clock runs slow under wined3d: 0x8651A8
(4000 a second) moved 6600 in about 10.5 s.

* **The garage's idle tour** (**verified**, Wine tour_t0…t30 and
  tour_later; ported: `garage::View::tick`, `TOUR_SHOTS`, shot
  `crib_tour`): 0x4D82D0 (0x4D88C1…0x4D89D4) turns the tour on (0x8363FC)
  once (0x8651A8 − 0x837AA0) × 0.00025 passes 300 (0x7F45D4) and
  0x510C30 is 0. 0x837AA0 is set by GarageMain's constructor (0x4EB0AB),
  a part view (0x4C1D40, 0x4C1D53), a reset (0x4C1FA0, 0x4C1FBD), a tour
  step (0x4A5BD0) and the PC mouse (0x4A5D90: the wheel 0x8763EC, or a held
  button 0x8763F7, which also turns the car by the mouse's dx / dy
  0x8763E4 / E8 × 0x784260); menu keys do not touch it. While the tour is
  on (and the constant byte 0x8363F4 is 0): while the camera blends
  (0x444DC0: +0x148 < +0x14C) 0x83592C follows 0x7FB708; when 0x7FB708 is
  more than 1200 (0x7F45D0) past it — 0x7FB708 counts frames at 60 a
  second (0x57A0AF), so 20 s after a blend ends — the index 0x836400 (file
  1) steps through 0..=49, skipping records of 0x7F4608 whose +0 flag is
  0, and 0x4A5BD0 shows the record (+8 slot, through 0x4A5510): the
  camera blends to it at the mover's current +0x150 (0x451200, duration
  0x44D190 with the car angle difference), the car turns the short way
  to its angle and steers to its steer (as a part view). The flagged
  records are 42–48, "Animate Camera1…7" (cars and SUVs, `TOUR_SHOTS`).
  0x4C2100 (a menu's part view) and 0x4C2070 (the magazine pose) end the
  tour; a reset (0x4C2140) does not. Under Wine, with 0x837AA0 poked 1.3
  million ticks back in the garage, the tour began at once: car angle
  305.8° → 125.29° (Animate Camera1) over the first blend, and kept
  running while menu keys moved the icons (tour_later). The camera
  actions 0x42–0x4A (0x4A5740 into GarageMain +0x23C; digital ±value/255,
  analog (value − 128)/128) read 0 under Wine (memory over 140 s: the
  rates, the camera and +0x274 unchanged before the tour), so the "slow
  drift" of the earlier rounds is the tour's slow blends (**inferred** for
  menus111's graphics-shop frames: the tour's clock there was not read).
  The clone: GarageMain's view now runs the tour (car class 0, as
  elsewhere); the tour index is a global (0x836400) in the clone too.
  `crib_tour` (the tour's first record settled) matches Wine tour_t30
  (Animate Camera1) in camera and car (compare/menus115/sbs/crib_tour).
  **Inferred**: the mover's +0x150 before any view (the clone takes 2.5,
  the value read under Wine in the garage).
* **Custom HUD picker's list** (narrowed): the value 0x563D00 stores at
  the screen's +0x28 is its help text (the screen record's +8, which
  MenuScreen::BaseNotify 0x55B73B opens as the Tip dialog): 0xADA7304B
  "… Change Color Left / Right … Select Highlighted Color …" while
  picking, 0x1B2794C7 "… Change Gauge Type … Select Gauge …" on the list
  (the decal screen's 0xB36E7C1E / 0xAADE3025 / 0x73332E68 the same way).
  It is not what greys the dots. While the picker is up (0x838560 > 1)
  0x563D00 returns before anything else, so the overlay's tick on
  0xC98356BA — the scroller's frame 0x538770 (layout 0x52EEB0, the move
  0x574E40, the highlighted item's size pulse 0x50CB00) — no longer runs:
  the list stays as the accept's frame left it (Wine h3: the highlighted
  dot 9 px below the box, mid-move). The overlay's DIM (PARTS_ICON alpha
  to 0 in 100 ms) is not it: only 0x560B70 / 0x562950 run DIM, before
  the overlay's dialogs. The dots' grey colour stays **open**; the clone
  draws the settled list (its last frame when the accept comes after the
  move has ended).
* **Magazine shoot (kind 3) under the vanilla game** (not captured):
  a stage-3 save with STAGE_3_SHOW_3_PICTURE (0xA60CE3D2) added to the
  visit table as (hash, 2, 0, 0, 0) (written with
  `native_save::set_visits`; <work dir>/s3a_mag.sav) and the car moved
  onto its spot (−1295, 797)
  gives the engage SMS "View Your Magazine Cover" ("You made it on to the
  cover of a magazine. Go check it out!", Continue; mag_sms). Keyboard
  Enter only pulses Continue; a mouse click on it runs 0x498E70: 0x541940
  pays the reward ("Bank Reward for Magazine: 500" on the SMS, Bank
  +500 per click), 0x8384C8 = "UI_MagazineReward.fng", flow event 8 / 4
  is queued and taken — but g_GameState 0x8654A4 stays 6 and the SMS stays
  (mag_stuck), on two launches. The WidescreenFix run of menus113 (DVD,
  kind 4) did reach UI_MagazineReward; why the vanilla game does not
  under this Wine setup is **open**. The clone's magazine path is the
  DVD's screen (UI_MagazineReward), so it stays as matched in menus113.
* **SponsorPopup's frame pulse at 0.1 s steps**: not captured. It needs a
  won career race; with the game clock about six times slow under wined3d
  and screenshots about 0.5 s apart, 0.1 s of game time cannot be
  sampled by frames (the frame object's colour would have to be read from
  memory). Its script is the menus' icon frame (traced).
* **UI_DecalsOverlayInvis** (narrowed): no code names UI_DecalsOverlay
  (its string 0x799C54 is only in the package table 0x7F8768); the four
  +0x16C stores all name the Invis package, so it is the one 0x556880
  pushes. Both packages are the screen 0x568460 (factory 0x56B910): it
  hides the price 0x0380B632 outside career (game mode ≠ 1), runs CALL
  (0x13C37B, the fade in) on the Event Handler when 0x7FA1FC is set (the
  decal-slot path 0x5683F0 sets it), and zeroes the package's four
  controller masks (0x537960 pattern: input stays with UI_DecalMain).
  With its INIT states the Invis package still shows "Total Cost:" (its
  group 0x371F1C93) and no list bar, unlike Wine's decal list (menus103,
  a WidescreenFix capture), while UI_DecalsOverlay matches it; what
  hides the group and shows the bar in the Invis package is **open**. The
  clone keeps UI_DecalsOverlay's layout (a deviation by package, equal by
  the frames).

    the time) and the fade steps send it (0x73A1A1, 0x73A26A). Who queues
    them: Sound round 10 (nobody).

## Career audit round 6 (2026-10-31; **verified** from the assembly unless marked)

All Wine runs of this round were taken with ThirteenAG's WidescreenFix **off**
(`a private Wine copy/game/dinput8.dll` renamed `dinput8.dll.widescreenfix-off`;
the save then lives in the prefix's `AppData/Local/NFS Underground 2/NAME`,
not `game/save`). Its 66 patch sites
touch FE object lookups and rendering (0x4FF9D0 callers, 0x5BE…–0x5D2…),
none of the career code (career base 0x85AD40). Rounds 1–5 ran with it on.
Saves: the maintainer's probe `career_saves` (new: `PARTS=<rating>` fits level-1
visual parts until the visual rating reaches it; `RATING=<r>` runs the city
start's DVD offers first; `SHOOT=<hash>` prints a shoot's pay; tag `m` = the
stage's first world challenge on offer; the NAMES dump lists milestones,
challenges, purses and senders).

* **Photo shoot engage** (0x532B20 → 0x5011C0; 0x498D00 → 0x4991B0): a
  milestone's spot entered sets the visit entry's +5 bit 0 the first time
  and, with no engage under way (0x861E78), engages at once (0x500B00,
  state 0x11): UI_EngageEventDialog with the milestone's subject and
  message by its kind (0x514910 / 0x514860: 1 "Subject: Take Photo For
  Mag Cover" / "You can now take a photo of your car and get on the cover
  of a magazine.", 2 the DVD's (0x99829679 / 0x1DEFB577), 3 / 4 "View
  Your Magazine / DVD Cover" (0x16549076 / 0xE0F2E528, 0x74B2A894 /
  0xA747EA86)), the Accept / Decline pair hidden and one button
  0xB8A7C6CE "Continue" (0xDE02CF84) shown. Its handler 0x498F00 takes
  only that button (→ 0x5010E0, 0x498E70). The sender (0x499160): "From:
  %s" (0x9F29D70D) of one name drawn by 0x4FE970 — for a challenge too.
  The clone showed an OK dialog with the cover's title and paid as it
  opened, engaged only on the key, and the challenge dialog had no sender:
  fixed (`ChallengeDialogKind::Milestone`, `Game::shoot_accept`,
  `Profile::spotted` saved at visit +5 bit 0, `Game::sender_name`).
* **The race dialog's sender** (0x4B569C → 0x512820): "From: %s" of the
  event's +0x84 label when set, else its first opponent's name (+0x40).
  No shipped event sets +0x84 but one. The clone left the package's "From:
  Simon": fixed (`Event::sender`).
* **Under Wine, magazine / DVD** (clone save `s2m`: stage 2, STREETGLOW,
  4 stage-2 normal wins, `PARTS=1.3` → rating 1.32, placed at the DVD
  spot): the city start offered STAGE_2_DVD_1 (visit +4 = 2) and listed
  the challenge STAGE_2_SHOW_1 (0x7148) — the clone's `RATING=1.32` city
  start gives the same (and the DVD's message). Engage at the DVD spot:
  the dialog above ("From: Carol"); Continue → UI_ShowCase_Preview with
  the bank already 3,725 → 4,357 (+632 = 500 + ftol(100 × 1.32)) and the
  stage record +0xC4 = [DVD]; Take Picture → Accept → the milestone's +6
  = 1, "Do you want to save this DVD?" → the city. The challenge marker
  (226, 53): UI_EngageEventDialog "Subject: Magazine Cover Opportunity",
  2:00.00; Accept → +0xC00 state 1, the record at +8, the goal milestone
  offered. Teleported to the spot (−606, −766): state 3 and the dialog came
  at once without a key ("From: Neil"); Continue → ShowCase (+632, 4,989;
  +0xC4 = [DVD, SHOW_1]; the challenge's flag +6 = 1, state 0) → Take
  Picture → Accept → "Do you want to save this magazine?" (kind 1). The
  clone's `SHOOT=` gives 632 for both; the flow now matches.
* **Under Wine, hidden event** (same save): driving into S2_H_CIRCUIT_6's
  marker (−469, 610) set its visit +4 = 1 and showed the prompt; no SMS,
  no map change; Enter → UI_EngageRaceDialog ("From: Patrick", Circuit,
  purse 750); Back → +6 = 3. As the clone (and markers98).
* **Under Wine, URL tournament** (clone save `s2d` placed at S2_URL_3,
  the stage's last event, 2 races of 4 laps): dialog "From: Ali", purse
  1,100. Race 1 won by teleport: Tournament Race Results (points 8 6 4 3 2
  1), Tournament Standings, UI_PostRace "Next Race"; the race's end filed
  nothing (stage record +0x9C kept S2_URL_1 / 2), cash 14,625. Race 2 won
  (a Restart in the middle kept the round): results 8 …, standings 16 / 12
  / 8 / 6 / 3 / 3; the end filed S2_URL_3 in +0x9C; Race Reward 300 / 1,100
  with the help bar on 14,625; Return to Explore Mode paid 15,725, record 2
  done (+0xE0 7,200, +0xE4 24), stage 3, three messages (SMS_UNLOCK_09,
  Jackson Heights, SMS_CAR_UNLOCK_3), the save questions, the SUBURBAN
  flyover. `WIN=1185EF08 career_saves s2d.sav` gives the same cash, points,
  stage and messages. The original's save after it:
  `career6-tmp/orig-s2-url3-won.sav`.
* **Under Wine, stages 3, 4 and 5 starts**: after the stage-2 end above
  the stage-3 city start opened 23 events (15 normal by kind 3 / 3 / 3 / 3 /
  3, the 8 S3_H_), no challenge, no DVD (rating 1.32 < stage 3's); the
  clone's rebuild of the original's own save gives the same 23 by kind and
  the same 8 hidden ones (the drawn circuits / drifts / sprints differ: the
  draw is session-random, round 3). Clone saves `s4a` and `s5a` loaded:
  30 events each (4 per kind, the 10 H_), the same kinds and hidden events
  as the clone's city start, 47 and 84 messages, no challenge, no DVD.
* **Under Wine, the final stage end** (clone save `s5d` at S5_CIRCUIT_36's
  marker, 5 laps × 7.2 km won by teleport): dialog "From: Caleb", purse
  10,000; the race's end filed the win (stage record 5) with cash 341,925
  unchanged; Event Results; Race Reward 300 / 10,000; Return to Explore
  Mode paid 351,925, record 5 +0xE0 17,100 / +0xE4 57 and done, career
  stage 6, one message (the event's, the 100th: the clone's
  SMS_END_OF_GAME_MESSAGE), the save questions. The stage-6 city start:
  no open event, the five shops BE4383FA, EC17E618, 55446B8F, 724F047D,
  7FC8D0D0 on the marker table, no challenge, outrun state +0x1AE8 = 1 —
  `WIN=04CCE3E1 career_saves s5d.sav` and the original's save loaded in
  the clone (`career6-tmp/orig-s5-final-won.sav`) give the same. The
  after-race movies (SCENE14's won movie, Scene15 / Scene16) were skipped
  by the key and not checked.
* **Not seen live this round**: the outrun encounter (no AI tuner car met
  the engage test 0x40C030 in ~15 min of teleporting behind the two
  placed racers; the HUD's state +0x1AE8 stayed 3 = no car).
* **Left**: the milestone dialog's Continue is drawn highlighted in the
  original (the INIT 0x35F8620B runs 0x52CEB0 on +0x54); the clone shows
  it unhighlighted (FadeIn only).

## The widescreen fix in the Wine captures (2026-10-31)

Until 2026-10-31 07:12 every Wine copy loaded ThirteenAG's widescreen fix
(game/dinput8.dll + SCRIPTS/NFSUnderground2.WidescreenFix.asi), which
patches 66 places in .text (list `<work dir>/wsfix_patches.txt`;
site → feature map `<work dir>/patchmap.md`, from the
patched bytes, the disassembly and the fix's source). It is now renamed
`dinput8.dll.widescreenfix-off` in the copies (a private Wine copy and
a private Wine copy still have it). The original without it (a private Wine copy,
wined3d, 1280 × 960) was captured again and set beside the clone in
`$OB_PRIVATE/wine/compare/vanilla115` (orig / clone / sbs).

* **What the fix changed at 4:3** (installed ini: FixHUD 1 or 0 by copy,
  FixFOV 1 except look112, DisableCutsceneBorders 1, RainDropletsScale
  0.5, HighFPSCutscenes 1, AutoFitFMV 1, FEScale 1, SkipIntro 1):
  * projection 0x5C7ED0 (FixFOV; see Chase camera → Projection): the
    chase views unchanged, other angles ±4 % in height, the mirror view's
    aspect 0.45;
  * HUD (FixHUD 1): the HudLayout entries moved, right-hand ones +12.3
    canvas units (+25 pixels), left-hand ones −2.3 (−5 pixels), through
    hooks at 0x51B190 / 0x51D665 / 0x4C6753 / 0x50516B;
  * the scene letterbox bars removed (0x5CBE9B…0x5CBF17);
  * every FE movie quad ×4/3 (0x536A2F: the ±0.5 pushes of 0x536860 →
    ±0.6667), and movies showed black;
  * lens drops ×0.5 (0x5C7CF5 / 0x5C7D13; see Lens drops);
  * in-engine scenes at 60 instead of 30 frames a second (0x435FA6, the
    30.0 that 0x435F40 pushes at 0x435FA3 for 0x576CC0);
  * the steering's zero test made unconditional (0x416D82 jp → jmp):
    no effect, the test (|angle| < 0.0, 0x783AF4) never holds (see
    Steering → Stored angle; round 2 below);
  * the NOS trail sampled by real time (0x61AD90), mouse free look on the
    chase cameras (0x453594 / 0x453B45), the cursor hidden while driving
    (0x50B460), the radar mask quad 640 × 480 → 2W × 2H (0x5C727D…, no
    visible change **inferred**), resolution / window / input / disc
    hooks (no picture change).
* **Re-captured** (vanilla, against the clone at 1280 × 960): the main
  menu (FE edges and the turntable car: match to the pixel after the FE
  half-pixel correction, see Front-end quad submission), free roam at the
  airport (3D edges identical to the modded capture; the HUD matches the
  clone to the pixel, the modded FixHUD 1 capture had the tach and Bank
  box 25 pixels right), CareerNis1 (letterbox bars present, see Scene
  letterbox), the boot intro movie (plays; extent matches the clone).
* **Statements from modded captures** that the vanilla original
  confirms: the race-start 120-pixel bars, the HUD positions with FixHUD 0,
  the chase-view geometry. Corrected: "Wine shows black for movies" (the
  fix's doing). Not re-captured (**open**): rain (weather107, already
  marked), the scene timing at 30 fps (nis79, scenes75 were taken at the
  fix's 60), the steering comparisons, the rear-view mirror (none was
  compared), the FE movie objects inside menus (none compared).
* **Round 2** (2026-10-31, vanilla2; a private Wine copy, wined3d, :169,
  fresh profile → Start New Career → free roam at the airport, memory
  read every frame sample; side by side in
  `$OB_PRIVATE/wine/compare/vanilla2`):
  * **Steering** (**verified**, static): the patched test at 0x416D77
    compares |angle| with the .rdata 0.0 at 0x783AF4, so the original
    never zeroes the angle and the fix changed nothing; the steering
    comparisons of physics rounds 2–6 (input mean, rate gain, drift
    steering) stand. Found on the way and ported: the angle is a u16
    binary angle (see Steering → Stored angle). Not re-driven in Wine.
  * **Scene frame rate** (**verified**, live): 0x865558 is the main
    loop's minimum frame time (0x57EAE0 pads dt up to it by busy-waiting;
    see the frame-time ring above). Static default 1/120 s (0x7FB710,
    0x77DCE0); NIS_SceneStart 0x437910 → 0x435F40 sets 1/30 (0x576CC0)
    when the scene has a kind-1 channel (0x4326B0(1), +0x10 set), and the
    scene's end 0x437A00 restores 1/120 (tail jump 0x576CE0); a track
    load with 0x89CF69 set or 0x8669F4 / 0x866A14 > 0 sets 1/30
    (0x5FF698, 0x7A5734 = 30.0) and the unload restores it (0x5FF786).
    Live: main menu 0.00833; CareerNis1 0.03333 from clock 2.19–2.55 to
    15.78–15.83, then 0.00833 in free roam; 0x8650CC = 0, 0x7FB700 = 2
    (so the step is clamp(dt + carry, 0x865558, 0.05)). The scene follows
    the world clock, not the real time: under Wine (about one frame a
    second, 0.05 s of clock each) CareerNis1 took 13.3 s of clock and
    133 s of real time. The fix's 60.0 therefore only changed how often
    scenes are drawn, never their timing, and the Wine captures (all under
    30 frames a second) were not affected. The clone's CareerNis1 ends
    between 13.4 and 14.0 s, and its frames at clock − 2.4 s match the
    original's (sbs-careernis1.png: the four shots' cameras agree, the
    first within a few frames). Race-start, shop and ending scenes run
    through the same 0x437910 / 0x437A00, so the same holds (not
    re-captured). **Open (decision)**: the clone draws scenes, and
    everything else, at the display's rate; the original caps at 120
    frames a second and at 30 during a scene with a kind-1 channel.
  * **Sky at the airport** (**settled**: time of day, not the renderer):
    the purple sky of the round-1 capture was taken in the sunrise →
    night fade (clock 60–80). Same pose, vanilla against the clone with
    `NFSU2_RAIN_CLOCK`: clock 19 (sunrise, index 0) top 120 rows mean RGB
    67/42/54 against 64/40/51; clock 93 (state 0, index 2, toggle 1)
    22/22/32 against 22/22/31.
  * **Rain** (**observed**): clock 154.8–156.1 at the airport, drops on
    the lens in both; native size (the clone has no 0.5 factor: 0.12 ×
    size, 0x785368); the largest drops about 60 pixels across in both.
    The weather107 note stands corrected.
  * **Rear-view mirror** (**observed** + gate **verified**): the original
    draws it with the Bumper and Hood cameras (camera setting 0 / 1):
    0x6000A0 adds HUD feature 0x1000 (0x6004B7) when the Rearview Mirror
    option (S+0x14, 0x83AA18) is on, racer +0x249 is 0 and the player's
    camera setting (0x83AA74 + 0x28 × player) is 0 or 1. Live (bumper,
    1280 × 960): a framed picture at about x 415–862, y 72–152 at the top
    centre, the view behind (RearViewMirrorCameraMover), black where the
    sky would be. The clone has no view 3 and draws no mirror: **open**
    (its camera mover, viewport, projection and HUD frame not traced).
  * **FE movie objects** (**verified**): only eight packages hold a movie
    object (FE type 7): IG_PlayMovie, LS_BlankMovie, LS_EAlogo,
    LS_PSAMovie, LS_THXMovie, MW_LS_IntroFMV, UG_LS_IntroFMV and
    UI_Trailers (unreachable on PC, see Options); no menu screen has one.
    The boot intro's was compared in round 1. Start New Career showed no
    movie in this run either (frames every second: Career menu, then
    the loading screen 1.6 s later).

## Menus round 11: dialog backdrops, the pauses, Post Race, the dyno camera's rates (2026-10-31; checked against the vanilla original under Wine)

Wine: a private copy (`$OB_PRIVATE/wine/runs/<run>`, vanilla game, no
WidescreenFix, wined3d, Xvfb :193); side by sides in
`$OB_PRIVATE/wine/compare/menus11/sbs` (original left, clone right,
1280 × 960).

* **Dialog backdrops (the screen under a dialog)** (**verified**): a
  dialog is pushed over the asking screen (ShowDialog 0x558030), which
  stays drawn; whether it dims is the asking code's and the package's:
  * IconScrollerMenu_StartDim 0x538C10 plays the package's Event Handler
    (0x47FF4E7C) script DIM (0x00009E99, fe_hash "DIM"). It is reached
    two ways: the icon menus' code sends 0x96DDA9E6 to IconScrollerMenu's
    handler 0x543D50 (0x54402D → 0x538C10) right after asking — 27
    senders: UI_postrace (0x4BEC69 … 0x4BEDFB, UI_InGameDialog 0x795000),
    the sponsor sign question (0x4C151A, 0x4C14C0), the pauses (0x4C25BE …
    0x4C28E0), and (screen **inferred** from the nearest named functions)
    0x4A3172, 0x4AF5F3 … 0x4AF80B (main menu / career), 0x55E819,
    0x560C1F, 0x56171A, 0x5629FF, 0x562BD6 (graphics shop), 0x564CF0 …
    0x564F06 (the dyno / tuning), 0x56BB00, 0x56BC50 (decal shop) — or the
    shop handlers
    call it directly after 0x5587A0 / 0x558440: UI_Paint 0x53BD30
    (0x5637C8, 0x563803; the paints 0x55D420; **verified**), the graphics
    shop 0x562F0B / 0x562F5D / 0x562FB2 (0x562C60, Back's questions,
    **verified** by the clone's port), the decal shop 0x56B5F3 / 0x56B7AC
    (0x10B83327 / 0x30F7037F, **verified**), and (screen **inferred** from
    the nearest named functions) 0x559C10, 0x568894, 0x568927, 0x568B66
    (UI_ICEMAIN), 0x55A139, 0x55A17F, 0x55C715 (performance categories),
    0x55A90F (Mode Select), 0x55F136, 0x55F17E (the parts browser leaving
    a category), 0x561801 (unique parts), 0x5649AC (dyno). The car lot's stock /
    trade questions play the same script by name (0x537C10(package,
    0x47FF4E7C, "DIM"), 0x4EDCBC).
  * What it does is the package's: the handler's DIM, where it has an
    event, sends 0xA301C015 at t = 0 to every object (target 0xFFFFFFFC,
    or 0 in UI_pause / UI_postrace / UI_PauseOptionsMain /
    UI_GenericParts_Browser / UI_Rims_Browser / UI_ChooseSpinner and the
    parts overlays); the objects that answer it (`MsgR`) run their own DIM:
    alpha tracks over 100 ms, groups and the strip template to 0, some
    images to 60 / 255. UI_ChoosePaintCategory, UI_ChooseVinylLayer,
    UI_ChoosePerformanceCategory, UI_ChoosePerformancePackage,
    UI_PerformanceTuning_Setting / _Master have a handler DIM with no
    event, UI_Sponsorship_new and UI_RewardsSponsor none: those stay as
    they are (Wine: the graphics shop under its install question, the
    sponsor screen under the sign question). The icon scroller's option
    clones copy their template's scripts and answer as it does.
  * UI_Paint's 0x53BD30 in a slot (+0x188) also plays "Dim" (0x78FEB8)
    on its group 0xDB285767, whose DIM sets the group's alpha to 0 at
    once: the strip and title go (Wine menus103 vinyl_colour_ask; the
    Visual Rating widget, its own package, stays).
  * The vinyl question (0x559250 → 0x5587A0 with GenericDialog_MED) calls
    no StartDim: the browser stays drawn under it (Wine menus103
    vinyl_ask_cust). Before asking it zeroes UI_GenericParts_Browser's
    package +0x1C…+0x28 (0x53A2C0; 0x53A2F0 sets 0xFF): its button masks,
    not drawn).
  * Wine (this round): free roam's "Quit to Main Menu?" over UI_pause:
    the pause menu faded out, only the scene under the dialog
    (`explore_pause_quit`); earlier rounds: the car lot's stock question
    (c14), "Restart race?" over UI_postrace (p2).
  * Clone: `frontend::with_dim(package, objects, age)` runs DIM on the
    named objects of that package from the dialog's opening inside the
    package's own script engine (the message route and the answers are
    the packages'), `frontend::dim_alpha` fades the scroller clones by
    their template's DIM; `Dialog::dim_under` names the package and the
    objects (the Event Handler; UI_Paint in a slot also 0xDB285767), set
    for the car lot's questions, UI_Paint's colour / paint questions and
    its bank notice, and the decal shop's layout / remove-all questions.
    The clone's other dialogs either draw no screen under them (the icon
    menus, whose DIM fades every object: the same frame) or a screen whose
    handler DIM has no event. The Visual Rating widget stays drawn under
    a dialog over the screens that show it (Wine vinyl_ask_cust /
    vinyl_colour_ask). The old DIM (every quad faded) compared game ms
    with a time in seconds, so it was at 0 at once.
  * **Open** (closed in menus12, below): the DIM's UNDIM (0x9F218E63 /
    script 0x03D8EABC) on the way back; the bank notice over the parts
    browser and the other shops.
* **Pause in free roam** (UI_pause, 0x4C2900) under Wine at the airport,
  stage 1: Return to Explore Mode, Options, Replay (greyed), Quit to Main
  Menu — the clone matches (`pause`, side by side `sbs_pause`); "Quit to
  Main Menu?" (UI_InGameDialog, OK / Cancel, Cancel focused) matches
  (`pause_quit`). The race's pause (0x4C2BD0) matches menus110 q0
  (`race_pause`, new shot: a Quick Race paused after its start).
* **Post Race** (UI_postrace, 0x4BEE10): the menu matches Wine menus112
  pr07 (`winchain`, side by side `sbs_postrace`); the 3D scene behind it
  differs (the post-race camera and crowd: race endings, not this
  screen). UI_PostRaceInfo.fng (0x78F070) is named only in the package
  table (0x7F4598): not used. There is no separate "Visual Rating" screen
  package: the rating shows as the shops' widget (VisualRatingBar) and as
  the milestone SMS / engage texts (SMS_VISUAL_GENERIC_%d,
  ENGAGE_VISUAL_GENERIC_%d), ported earlier.
* **Dyno camera rates** (**verified**, closes round 6's open item):
  0x44F2B0 (vtable 0x788070 +0xC) registers the label 0x446740 once
  (+0xB4) for JOY events 0x48 CARSEL_ORBIT_LEFTRIGHT, 0x49
  CARSEL_ORBIT_UPDOWN, 0x51 DEBUG_CAMERA_MOVE_CLOSER and 0x52
  DEBUG_CAMERA_MOVE_FARTHER (names from the (id, name) table 0x7FABA8).
  With the dyno manager's state ≠ 0: 0x48 sets the yaw rate +0x58 =
  −(v − 128) / 128 (0x784540 = 128, 0x787AF0 = 1/128), 0x49 the pitch
  rate +0x48 = (v − 128) / 128 (v the event's byte), 0x51 / 0x52 the zoom
  +0x3C = 0.98 / 1.02 while held (1.0 on release); state 0 zeroes them.
  The update 0x44F300 adds ftol(0x7FB70C × 128 × rate) to +0x50 / +0x40
  and multiplies the distance by +0x3C within 5.0 … 7.2. The PC's
  descriptor table 0x7FDD10 has no rows for 0x48 / 0x49 (rows 65 / 66
  give 0x51 / 0x52 keys 0x35 / 0x3D in the debug-camera block; whether
  that block is live in the dyno is **open**): the orbit takes a pad's
  analog axes (**inferred**), and the clone, keyboard-only here, keeps the
  camera still as the PC keyboard does.

## Menus round 12: UNDIM, the dim at every StartDim site, the dyno zoom keys (2026-10-31; checked against the vanilla original under Wine)

Wine: `$OB_PRIVATE/wine/runs/<run>` (vanilla, no WidescreenFix, wined3d,
own Xvfb :238); bursts and side by sides in
`$OB_PRIVATE/wine/compare/menus12` (orig / clone / sbs).

* **UNDIM** (**verified**): IconScrollerMenu_StopDim 0x538CB0 is
  StartDim's counterpart. It re-enables the screen (+0x7B = 1; with +0x14C
  set it restores the package's button mask, +0x1C = +0x28 = +0x20, log
  "StartInput … Mask (%d) to (%d)" 0x79B968), turns the strip's own fade
  back on (0x538B40: +0xE4 = 1 and every icon re-placed with its alpha by
  position, 0x52F8D0 → 0x50D8D0 / 0x51FD70; StartDim's 0x51FD20 had set
  +0xE4 = 0) and plays the package's Event Handler (0x47FF4E7C) script
  UNDIM 0x03D8EABC (fe_hash "UnDim"; same lookup as StartDim, 0x538D4F).
  The handler's UNDIM sends 0x9F218E63 to every object (target
  0xFFFFFFFC, or 0 where its DIM uses 0), at t = 0 in most packages, at
  t = 100 ms in UI_ChooseCustomizeCategory, UI_ChooseRimBrand and
  UI_Options; the objects answer (`MsgR` 0x9F218E63 → their UNDIM 0x03D8EABC:
  alpha 0 → base over 100 ms, e.g. UI_CareerCarLot's groups; UI_Paint's
  group 0xDB285767 back to 255 at once). Reached by:
  * 0x251BF83E to the screen (IconScrollerMenu_Handler 0x543D97 →
    0x543E6F), sent by the asking code when the dialog is answered: the
    pauses' Cancel (0x4C2207, 0x4C230D, 0x4C236B), the dyno (0x564497,
    0x564AE4), the graphics shop (0x562AEB), the decal overlay (0x56B9DA,
    0x56BEC9, 0x56BF4B), 0x4A323A, 0x4C13F3, 0x4F3B28, 0x4F98FE, 0x560D0B;
  * direct calls from the screens' answer handlers (27): the crib's No
    (0x43DA9FD0 / 0x1FAB5998 → 0x4AF4BB: StopDim, then "UnDim" on
    0x444969FD and 0x444969FE too), 0x4B60CD, 0x55A7A8, 0x55C593, 0x55C7E7,
    0x55E5FE, 0x55F337, 0x5617B2, 0x562CBA, 0x562FFD, 0x563064, UI_Paint
    (0x5634AF / 0x5639AC: StopDim, then in a slot "UnDim" 0x790CEC on
    0xDB285767; 0x563B18, 0x563BB3), 0x56488A, 0x56494E, 0x564AC3, 0x568811,
    0x569B5C, 0x56A90D, 0x56AB78, 0x56ADED, 0x56AE4D, 0x56B49B, 0x56B8AB;
    0x53A750 (strip off, 0x52EBA0(1), then StopDim; the rims browser
    0x568134) and its pair 0x53A730 (0x52EBA0(0), StartDim; 0x567E4B).
  * Wine (`pause_undim_burst`, explore pause, Cancel on "Quit to Main
    Menu?"): the help bar (Back / Continue) and the strip's icons are back
    at the answer, the rest of the pause fades in over the next frames
    (u09 → u12); `customize_dyno_ask_undim_burst` (Customize's "Do you want
    to enter the Performance Tuning / Dyno track?", No): the icons at once,
    the strip after a pause (its UNDIM event at 100 ms), then fading in.
  * Clone: `Game::undim` (`Undim`) follows a dialog with `dim_under` to its
    answer; while the answer's screen is the dimmed one (same package) it
    is drawn inside `frontend::with_dim_undim`: DIM from the ask, UNDIM on
    the same objects (UI_CareerCrib also 0x444969FD / FE,
    `Dialog::undim_under`) from the answer, in the package's own script
    engine; `dim_alpha` gives the scroller's clones their full alpha once
    UNDIM has started. New shot `pause_undim` (NFSU2_SHOT_MS after Cancel;
    15 / 60 ms match u09 / u11 in kind; the end state equals `pause`
    pixel for pixel). **Inferred**: the clone runs UNDIM on whatever answer
    goes back to the dimmed screen (the original's Yes paths that stay on
    the screen were not each traced); the Wine frames are not timed (the
    game runs slow under wined3d), so the 100 ms tracks are the data's.
* **The DIM at every StartDim site** (**verified** per site; the screen is
  drawn under the dialog and dims when the package's handler DIM has an
  event, else stays): `dialog_over(mode, dim, undim)` pushes a dialog over
  its last menu outcome (that screen's opening moved 2 s back so its
  FADEIN is over: the original never closed it) and sets `dim_under`.
  Wired: the pauses' and Post Race's UI_InGameDialog questions (0x4C25BE …
  0x4C28E0, 0x4BEC69 … 0x4BEDFB; `pause_quit` unchanged), the crib's
  questions (Return to Explore Mode? 0x4AF66B, Performance Tuning / Dyno
  0x4AF790, Back to Main Menu 0x4AF80B) and Customize's dyno question
  (0x55E819, UI_ChooseCustomizeCategory: Wine `customize_dyno_ask_dim_burst`
  shows it fading out), the dyno's Save / Quit Tuning questions (0x564D60,
  0x564DD0; Back's 0x564E99), Mode Select's "Tune settings for this race
  mode?" (0x564F06) and its "Quit to Garage?" (0x55A90F) — both over
  UI_PerformanceTuning_Setting, whose DIM has no event: the screen stays,
  as menus112 d4 (`tunemode_ask`, side by side `sbs_tunemode_ask`; before
  this the clone drew nothing under it) —, the performance package's
  notices and questions (0x55A139, bank notice 0x55A089 included), Return
  to Stock (0x55A17F), the individual parts' notices and questions (they
  all end at 0x55A3DE: "Dim" on UI_ChoosePerformancePackage's handler, no
  event), the unique-part leave question (0x561801, UI_ChooseUniquePart,
  with event), the graphics shop's exit / purchase questions (0x562F0B,
  0x562F5D / 0x562FB2; no event). Already wired: the car lot,
  UI_Paint (incl. its bank notice 0x5637A9 → 0x5637C8), the decal shop's
  layout / remove-all questions.
* **"Not enough bank" outside the paint screen** (**verified** per site):
  the vinyl browser's (0x5592CB → 0x5592D6, then returns: no dim; the clone
  now draws the browser under it, as under the vinyl question), the
  performance package's and parts' (dim, no event: the screen drawn
  unchanged), the parts browser's (0x566E1B: no StartDim), the rims
  browser's (0x567DC4 → 0x567E4B 0x53A730: dims), the ICE parts overlay's
  (0x568EBA → 0x560B70: dims), UI_BuyPerformanceParts' constructor path
  (0x56A463 → 0x562950) and the decal overlay's (0x56BB98 → 0x56BC50 and
  "DIM" 0x78FEF4 on the handler). The clone has no bank notice on the
  parts, rims, ICE or decal-overlay purchases (it has no buy question
  there), so only the first two apply. The decal shop's hood notice
  (0xD104AD36, 0x56B57C) returns without StartDim: drawn over the shop,
  undimmed.
* **Not wired, with the reason** (round 13 wired 0x4A3172 for the
  Profile Manager, 0x559C10, 0x55C715 / 0x568B66, 0x568894 / 0x568927 and
  0x5649AC): 0x4A3172 (FE_ScreenOperation's memory
  card operation sends 0x96DDA9E6 to the caller; the clone's save
  questions are MC_Main screens of their own), 0x5629FF / 0x562BD6
  (0x561730's other senders: screens the clone has no question on),
  0x559C10 (the spinner accept, slot 0x1D: the clone fits spinners without
  a question), 0x55C715 / 0x568B66 (the performance shop's and UI_ICEMAIN's
  exit question 0xD76E0B5A: the clone leaves those shops without asking —
  **open**), 0x568894 / 0x568927 (UI_ICEMAIN's notices 0x20B30CD8 /
  0x903854F0: not in the clone), 0x5649AC (the dyno's "save your profile
  now?" 0x71851654 after Quit Tuning: the clone asks it with the dyno
  menu already gone), 0x55F136 / 0x55F17E (0x55EF90's "already
  installed" / 0x5D336F96: the clone's parts browser has no such notice).
* **0x53A2C0 / 0x53A2F0** (**verified**, not visible): they are
  0x537960(UI_GenericParts_Browser, 0 / 0xFF) inlined — the package's four
  button masks (+0x1C current, +0x20 saved, +0x24, +0x28; StopDim's
  restore and the Q key's test 0x558B80 read +0x1C). The vinyl question
  zeroes them so the browser takes no buttons while it is up; nothing
  drawn reads them. The clone sends keys to the dialog only: nothing to
  port.
* **Dyno zoom keys** (**verified**, ported): the FE keyboard map 0x7FD7E0
  has the debug-camera block at entries 18–31; entry 22 is uAppData 0x35
  "Debug Camera In" (DIK 0x16, U), entry 23 0x3D "Debug Camera Out" (DIK
  0x18, O). Descriptor rows 65 / 66 (0x7FDD10) map them to JOY 0x51 / 0x52
  with kind byte 0xFF (the row search 0x572130 takes 0xFF as any device
  kind). 0x44F2B0 binds 0x48, 0x49, 0x51, 0x52 through 0x5794C0, which
  calls 0x579420 for both devices (0 and 1), so the keyboard's U / O reach
  the label 0x446740. Under Wine (`dyno_zoom_0` / `_u` / `_o`): U held
  zooms the dyno camera in on the Dyno Chart, O back out to the far end;
  on Mode Select (`modeselect_u_*`) and the Dyno Shop menu
  (`dynoshop_u_*`) nothing moves (the label's state test, ctx +0x60 →
  +0x1C → +0x1E0 → +4, is 0 there; which object that is stays **open**).
  Clone: `Game::dyno_zoom` (U / O on the Dyno Chart over the dyno track:
  distance × 0.98 / 1.02 within 5.0 … 7.2, `dyno_camera(state, distance)`),
  new shot `dynochart_zoom` (side by side `sbs_dyno_zoom`). **Inferred**:
  one step per 1/60 s (0x44F300 runs once per camera update; the
  original's rate is not traced), the distance kept until the dyno is
  entered again, U winning when both are held. The orbit axes 0x48 / 0x49
  have no keyboard rows (round 11): pad only.

## Stage 1 playthrough against the original (2026-10-31)

Details: internal notes. Seen under Wine (fastgpu) and **verified** in SPEED2.EXE:

* **UI_CareerCarLot's help bar in a career.**
  - 0x4B2310, the car lot's refresh, branches only on game modes 0x20, 2, 4, 8, 0x100 and 0x200.
  - In a career (mode 1) none of these branches run, so the screen record's mask (+0x20; 0x405 =
    Back, Continue, Quit Game) goes to 0x54E6F0 unchanged. 0x1000 is cleared for a car in a career
    slot (0x516340).
  - Under Wine the D-Day lot showed the bar, with the name and the cash taking turns.
  - Clone: `CarLot::help_mask` (UI_CareerCarSelect too).
* **The garage's "Return to Explore Mode?"** (0x4AF630, on 0x0C407210):
  - 0x5587A0(screen, GenericDialog_SMALL 0x78FA24, Yes 0x88579775, No 0xBE503981, 0x451E768E,
    0x43DA9FD0, 0x43DA9FD0, **1**, 0x60B42AFB), then StartDim 0x96DDA9E6.
  - The 1 focuses No, as seen under Wine. The clone had Yes focused.
  - **What stands under it** (Wine, stage-2 playthrough, frames 33 ms apart from before the key):
    the accept 0x0C407210 in the crib's handler 0x4AF3A0 first plays FORWARD (0xDE6EFF34 on the
    Event Handler: the objects' FADEOUT, then 0xE1FDE1D1 at 140 ms, when the option's action
    0x4AF630 asks). The strip, title, logo and help bar fade out over about 100 ms, and the
    question then stands over the garage scene alone, with nothing of UI_CareerCrib drawn (not
    dimmed). No brings the package back. Fixed: the clone drew the crib dimmed under it
    (`dialog_over`); it now draws no screen under it, as its other icon menus' questions do.
* **Race totals** (profile +0xA00 = 0x8454DC, 0x50 bytes; **verified** 2026-10-31, stage-2
  playthrough, in the code and under Wine):
  - At a race's load, World_Start 0x60A690 calls 0x541A60 (this = 0x83A9D0) when g_RaceType
    0x89E7B0 is set and 0x89E7E6 (dyno run) is not: +0xA04 += 1 and, for a known kind (the
    track's +0x94 bits, 1 → 3, 2 → 1, 4 → 0, 8 → 2), +0xA10 + 12·kind += 1. In split screen
    also +0xA40 and player 1's block. Under Wine at a sponsor circuit's load: 8 → 9 and +0xA34
    1 → 2; at a URL race's load only +0xA04 (the URL track has no kind).
  - After it, 0x551440 → 0x515990 (the results entry from 0x54CCB0 → 0x515DB0): +0xA00 += the
    entry's +0xF8 (an f32: car +0x410, dt × |v| while 0 < |v| < 1000 in race status 4), +0xA08
    += place 1, +0xA0C += place ≠ 1, +0xA14 + 12·kind += place 1, +0xA18 + 12·kind += 0x4FE170
    (the style score), +0xA4C on kind 1 with entry +8 = 9, +0xA44 / +0xA48 in split screen.
  - The per-kind triple is (loaded, won, style score).
  - The entry is filled at the finish: under Wine +0xA00 grew by 290.93 for a won circuit while
    car +0x410 read 313.68 after the results (the car drives on).
  - Clone: `career::RaceTotals` (raw 0x50 bytes, loaded and saved with the records):
    `race_loaded` in `start_round`, `race_done` with the race's `odometer` where the record is
    added. **Not ported**: the style score (+0xA18 + 12·kind; the clone's style total is not the
    entry's +0x104 × +0x108) and +0xA4C (entry +8 not traced). **Inferred**: Restart reloads and
    counts again.
* **Event movies:**
  - The drift's Accept queued SCENE20 then DriftTut (0x8382A0, mode 0x838568 = 1) on each Accept,
    with about 40 s of (black, headless) movie time.
  - The drag's Accept queued DRAGTUT, but the race's start scene came within 3 s. Why is **open**.

## Menus round 13: the shops' exit questions, UI_ICEMAIN's notices, the dyno's save question, the focused button, MC over its caller, the rims' question (2026-10-31; checked against the vanilla original under Wine)

Wine: `$OB_PRIVATE/wine/runs/<run>` through `fastgpu.sh` (DXVK on the
GPU, own Xvfb :213, no WidescreenFix), main-menu Customize with globals
poked by `peek.py` where a career is needed; images in
`$OB_PRIVATE/wine/compare/menus13` (orig / clone / sbs).

* **The focused button of a Yes / No question** (**verified**):
  0x5587A0's eighth argument goes to 0x558690 as the focused button
  (0x558760 passes its own the same way). Of the 0x5587A0 sites 59 push 1
  (No focused) and 11 push 0 (Yes; calls): "Reset to default settings?"
  (0x2BBCCA6B: 0x49B9C7, 0x4F9796), the stock car question 0x85215BC6
  (0x4EDCAC), 0x1231C38F (0x4DFEEC), 0xD5D46FDF (0x4F1B22, 0x55DED8),
  0xC4F4678E (0x498066), Quit Game 0x9DE6B485 (0x558BFB, 0x55DF1A) and the
  three-button 0x80C151D5 / 0x06D7902C (0x5631A0, 0x56325F); the save
  prompts' 0x22BC94AF / 0xF3568B86 (0x49C8E7, 0x49C92F, 0x4A7FB5,
  0x4A8002) and the transmission question (0x497D91) take a register. Under Wine the car lot's exit
  question (menus111 carlot_exit), the performance shop's and UI_ICEMAIN's
  exit questions, Quit Tuning and the dyno's save question light No. The
  clone had Yes focused on every `ask` but Mode Select's, the vinyl's and
  the crib's dyno question. Clone: `ask_focus(question)` (No unless one
  of the 0 texts; the save prompts' kept on Yes, **open**), used by
  `ask_in` and two-button `setup_dialog_in`; the selftest answers Yes with
  Left first (`answer_yes`).
* **The career shops' exit question** (**verified**, ported): the
  performance category's Back (0x911AB364 in 0x55C530) with 0x83898C = 0
  and its FADEOUT not running (0x537D50 on 0xDE6EFF34) asks 0xD76E0B5A
  "Are you sure you wish to exit back to Explore Mode?" (0x55C6E5;
  0x5587A0 over its own package, "" sized by length, Yes 0xD05FC3A3, No
  and Back 0x1FAB5998, No focused) and StartDims (0x55C715). Yes: flags
  +0x150 / +0x152, the package's masks to 0xFF, Shop_LeaveToCity
  (0x552C90). No: StopDim (0x538CB0), +0x150 = 0. In Customize
  (0x83898C ≠ 0) Back plays FADEOUT and 0xE1FDE1D1 goes to the
  categories (0x55BD70) or 0x55BE00. UI_ICEMAIN (0x568750) asks the same
  (0x568B2C, StartDim 0x568B66, 0x838970 = 0x1FAB5998) with game mode
  0x850078 = 1, 0x83898C = 0 and 0x838998 = 0 (its top list); Yes
  (0xD05FC3A3) sets 0x86461A / 0x864619 and the next 0xC98356BA leaves
  (0x552C90), No is StopDim. Wine (0x83898C poked to 0, and 0x850078 to 1
  for UI_ICEMAIN, in Customize): `perf_exit` — the strip dims, the scene
  dims, the help bar goes; `specialties_exit` — UI_ICEMAIN's title and
  strip fade out but the selected icon faintly; No brings both back.
  Clone: `Game::shop_exit_question` (`LotDeal::LeaveShop`, the save point
  when changed, then the city), `career_shop()`; shots `perf_exit`,
  `specialties_exit`.
* **The body shop's exit question** (traced, **not ported**): the generic
  parts browser's Back (0x5671EB) with 0x83898C = 0 and +0x1D7 / +0x1D5
  clear asks the same 0xD76E0B5A (0x56721E, No focused), zeroes the
  masks (0x53A2C0), turns the strip's fade off and plays MENUFADEOUT
  (0x79BE40) on its Event Handler instead of DIM. The clone's body shop
  leaves without asking (**open**: +0x1D7 / +0x1D5 not identified).
* **UI_ICEMAIN's notices** (**verified**, ported): its accept
  (0x0C407210) on Nitrous Purge (slot 0xA2) with no nitrous fitted
  (Perf_HasNitrous 0x50F180: perf ids 12–14) shows "Dude, you gotta
  install nitrous on your ride before^you can purge the system! Go get
  nitrous first!" (0x20B30CD8, 0x558440 with GenericDialog_SMALL),
  0x838970 = 0x34DC1BEC, StartDim (0x568894); OK is StopDim. On Split
  Hoods (slot 9) with any hood decal (slots 0x61–0x68, 0x610300) it warns
  "Warning! Installing a custom hood will remove any hood decals."
  (0x903854F0, "" sized by length), StartDims (0x568927), plays sound 8
  and still opens the list (0x556940). Wine (`purge_notice`, Customize's
  206): matches. Clone: before the browser's key mapping; shots
  `purge_notice`, `hood_notice` (the hood one **inferred** from the code:
  the list under the notice not captured).
* **The dyno's "save your profile now?"** (**verified**, ported): Quit
  Tuning's Yes is 0x30ED2368 (No / Back 0x34DC1BCF = StopDim); with
  Tuning_Changed (0x50FD10: the tuning object's five settings against the
  car's, 0x59AEA0) and save prompts (0x844AD8) 0x5647F0 asks 0x71851654
  over the dyno menu's own package (0x5649A2: Yes 0x24DD40CF commits and
  saves, mode 0x151; No 0x1FAB5998 commits and quits; Back 0x34DC1BCF,
  StopDim 0x56494E: the dyno menu again) and StartDims again (0x5649AC:
  the handler's DIM, still current, restarts at t = 0). Wine
  (`dyno_save_ask*`, the snapshot byte 0x83A544 poked): the quit question
  closes, for a few frames the menu shows again (its DIM restarting from
  full), then the save question over the dimmed menu, No focused; Back
  returns to the undimmed dyno menu. Clone: the save question is
  `dialog_over` the dyno menu with `Dialog::cancel` (Back's own outcome,
  the dyno menu); shot `dyno_save_ask`. The between-dialog frames are not
  drawn (the clone switches dialogs at once).
* **MC_Main over its caller** (**verified** for the Profile Manager,
  ported): 0x4A3100 stores the request and, with mode & 0xF = 1, sends
  0x96DDA9E6 to the caller (0x4A3172: StartDim) and pushes the MC package
  0x78F184 over it (0x555EC0(…, 0xFF, 0)); modes 2 / 3 replace the caller
  (0x537990). The Profile Manager's Create 0x861, Save 0x4A51, Load 0xC11
  and Delete 0x831 are mode 1: under Wine (`profile_load_list`) the
  Profile Manager stays under MC_List — its strip fades out, its title
  ("…e Manager" beside "Load"), the garage scene and car show through the
  list; no MC_Background. Clone: `Game::mc_over` keeps the menu
  (`mc_under`), drawn under the MC screens with its DIM while they are up;
  shot `profile_load`. **Open**: the other mode-1 callers (the shops' and
  dyno's save points 0x41 / 0x51 / 0x151 / 0x1141, Customize's 0x2042 is
  mode 2) still draw nothing under the MC screens; what the caller plays
  when the operation ends (UNDIM or FADEOUT) is not traced.
* **The rims' and spinners' size accept** (**verified**, ported): 0x559AC0
  (UI_Rims_Browser 0x568086 …, UI_ChooseSpinner 0x569A9B …): the fitted
  rim says "The part you are trying to apply is already installed!"
  (0xDA15D7E5); in a career an unowned rim above level 0 (+5 & 0xE0) that
  costs more than the bank "You do not have enough bank…" (0xD1AC60BE),
  else "Would you like to purchase this part?" (0x44B7167F, Yes
  0x09F00F07); owned, level 0 or outside a career "Would you like to
  install this part now?" (0xF5AC2B2A, Yes 0x51123592); GenericDialog_SMALL,
  No / Back 0x1FAB5998, No focused; then the strip's fade off
  (0x52EBA0(0)) and StartDim (0x559C10). The clone fitted at once; now
  `rim_question` → `LotDeal::FitPart` → `fit_browser_part` (the old
  accept). **Inferred**: the Yes handlers buy and fit as the parts
  browser does (0x567A60 / 0x569680 not re-traced).
* **Which state disables the U / O zoom** (**verified**): the label
  0x446740 tests mover +0x60 → +0x1C → +0x1E0 → +4, the chain 0x557B39
  uses to start the pull: the dyno manager (vehicle +0x1E0, update
  DynoManager_Update 0x56E6F0) and its state. 0 until the Dyno Chart's
  new run starts it (0x557B42 → DynoManager_StartPull 0x56D350: state 1,
  target gear 2), 1–4 during the pull, back to 0 when it stops (state 4 →
  0x56E690(1); 0x54A956 → 0x56E690(0) sets 3). With state 0 the label
  zeroes the rates and sets the zoom +0x3C = 1. Wine (`dyno_u_during_pull`
  / `dyno_u_after_pull`): U held for 2 s zooms during the pull, does
  nothing after it (frames identical); the distance stays zoomed on the
  Dyno Shop menu. Clone: `dyno_zoom` needs the chart's pull with stage ≠ 0
  (`dynochart_zoom` checks O does nothing once it has stopped).
* **vinyl_ask** (compared): the menus11 side by side set the career shot
  against the Customize capture (menus103 vinyl_ask_cust): Customize has
  no prices and its own garage camera, the career shop shows "Total Cost:"
  (traced, 0x9FF84A88) and the shop platform. `vinyl_ask_cust` against the
  same capture matches (sbs_va); left: the Visual Rating reads 0.04 there,
  0.00 in the clone (**open**: the previewed vinyl's share of 0x5103D0's
  rating). The career graphics shop's question was not captured.
* **Open** (seen this round): the scroller's neighbour icons stay faintly
  drawn under some dimmed screens in the clone (the dyno menu, the Profile
  Manager) where the original shows none (`frontend::dim_alpha`; the
  performance shop and UI_ICEMAIN match).

  - Stage 2 (movies render under fastgpu): S2_URL_1's Accept queued SCENE17 (mode 1, IG_PlayMovie
    pushed: 0x8384C4, the FE manager, is always set), yet 3 s later the race's start scene showed
    and the race ran at 11 s; Scene17_en.vp6 is 22.8 s and its frames are a montage, not that
    scene. So the event Accept movies did not play for DRAGTUT or SCENE17. IG_PlayMovie's ctor
    0x553950 names the movie object from the queue with no skip test (its 0x570A40 / 0x7F70D0
    tests only start subtitles). The upper-case name is not the cause: S2_URL_3's won movie
    "SUBURBAN" (mode 4) played (24.8 s, the Jackson Heights fly-over). **Open**; the clone still
    plays the Accept movies.
* **Engage dialog distance** (**verified**, Wine "3.313 mi" for S2_SPON_CIRCUIT_2, track 4004,
  5300 m): 0x4B5432 fild / fmul f32 0.000625 (0x791884) / fstp double, then Str_VFormat "%$f"
  (3 decimals) extracts the digits by ×10 and truncation and rounds the last up when the rest ×
  10 ≥ 5.0 (0x43EF92–0x43EFD8, 0x786E48). 5300 × 0.000625 rounds to exactly 3.3125 only with a
  single-precision product (the FPU's 24-bit mode under Direct3D 9, **inferred** as the
  reason); extended precision gives 3.3124999 and "3.312". Fixed: the clone printed "3.312"
  (Rust's round-half-even on an f64 product); it now multiplies in f32 and rounds half up
  (`screens::fixed3`).
* **"Bank Cash" after a race** (**verified**, Wine frames at 10 fps after the save's Continue):
  the new free-roam HUD shows "Bank Cash" (Cash_Bank's own package text) under the Bank box for
  about 2.7 s: Cash_Bank's INIT (0x001744B3) chains to CASH_ANIMATE (0x6287657C, 2.7 s) and no
  pickup has set its text. The clone's HUD follows INIT chains and shows the same
  (`NFSU2_SHOT_MS=1000 --shot explore`): no change needed.
* **Open-event refills and the always-open races** (**verified**): after S2_STREET_4 was won
  the original did not open S2_STREET_2, although the street slot's cap is 2 (stage def +0x32,
  read in memory). 0x527D40 re-adds the kept open events first, H_ races included, and counts a
  slot with 0x513770 → 0x513600, whose normal slots test only 0x4AE8B0 (+0x34 = 3, not SUV) and
  the type: H_STREET_5 fills the second street place. After a load the list starts empty and the
  H_ races are added after the slots, so they do not count (the reloaded city then had
  STREET_2 and STREET_3). The clone's rebuild already works this way; `career_saves WIN=` now
  rebuilds at load first, as the game does.

## Menus round 14: the body shop's exit question, MC over the save points' callers, DIM on icon menus, the save prompts' focus, the small rows (2026-10-31; checked against the vanilla original under Wine)

Wine: `$OB_PRIVATE/wine/runs/<run>` through `fastgpu.sh` (DXVK on the
GPU, own Xvfb :141, no WidescreenFix): main-menu Customize with globals
poked by `peek.py`, then career3's stage-2 save (s2a) for the sponsor
popup; memory read through the session (FE objects found by their hash
at object +0x10). Images in `<work dir>/menus14-tmp`.

* **The body shop's exit question** (**verified**, ported): the generic
  parts browser's Back (0x911AB364 → 0x5671DA): in a category (+0x1D9)
  back to the strip; else with 0x83898C = 0, not the graphics shop's
  groups (+0x1D7, set by GraphicsShop_FillGroups 0x546040 when 0x838990
  = 2) and not carbon fibre (+0x1D5, PartsBrowser_EnterCarbon 0x53A3E0 on
  item 0x5E; +0x1D4 is rims, 0x53A320 on item 0x1D) it zeroes the masks
  (0x53A2C0), asks 0xD76E0B5A (0x56721E: Yes 0xD05FC3A3, No and Back
  0x1FAB5998, No focused), sets 0x838970 = 0x1FAB5998, +0x1D2 = 0, the
  strip's +0x2F = 0 and hides its items (IconScroller_ShowItems 0x52EBA0(0):
  flags |= 0x400001 on the objects named by +0x18), and plays MENUFADEOUT
  (0x79BE40, fe_hash 0x9B90037C) on the Event Handler (0x567272), not DIM.
  The handler's MENUFADEOUT sends 0x821F58AE to every object (target 0)
  and 0xE1FDE1D1 to the code at 140 ms; the objects' own MENUFADEOUT fades
  them. No (0x1FAB5998 at 0x5674D8): the masks back to 0xFF, +0x2F = 1,
  0x52EBA0(1), +0x1D2 = 1, MENUFADE (0x79D798, 0x16DEFAA4: sends
  0xDDFC69A0) — in a category UNDIM instead. Yes (0x5676FE): +0x1DC = 1,
  0x83A9D8 = 0x50D2A0(…), Shop_LeaveToCity (0x552C90). Wine (0x83898C
  poked to 0 in Customize's body shop, `w_bodyexit` / `w_bodyno`): the
  strip and the browser gone, the right arrow, the Visual Rating widget
  and the column's dot left, No focused; No brings the screen back.
  Clone: `Dialog::dim_scripts` (MENUFADEOUT, MENUFADE) run through
  `frontend::with_dim_scripts`; the browser hides its strip items while
  `dim_script` is MENUFADEOUT; shot `bodyshop_exit`.
* **The save points' MC screens over their caller** (**verified** for the
  dyno, ported): FE_ScreenOperation (0x4A3100) with mode & 0xF = 1 sends
  0x96DDA9E6 to the caller and pushes MC_Main over it. The callers'
  handlers decide what that does: IconScrollerMenu's (0x543D50, called
  first by the performance categories 0x55C554, UI_ICEMAIN 0x5687C4 and
  the dyno 0x56482D) StartDims again; UI_GenericParts_Browser's handler
  (0x5664A0, vtable 0x79D768) has no case for it, so the body shop stays
  as its MENUFADEOUT left it. The shops' exit question's Yes (0x552C90,
  mode 0x41 when the profile changed, 0x52A4D0) and the dyno's save
  question's Yes (0x5649E1, mode 0x51) are mode 1. Wine (dyno in
  Customize, the tuning snapshot 0x83A544 poked, `wmc`): "Are you sure you
  want to save the profile named NAME?" over the dyno menu dimmed to
  nothing, the car still on the rollers. The clone dropped the dyno track
  before the save (`quit_tuning`); now the save point runs first
  (`AfterSave::QuitTuning`, the tuning committed before it). Clone: the
  answered dialog's screen is kept (`Game::save_under`) and drawn under
  the MC screens (`mc_under` with the dim's scripts); shots
  `dyno_save_mc`, `bodyshop_save_mc`. **Inferred**: the performance shop's
  and UI_ICEMAIN's (same code path, not captured). Also seen: entering the
  dyno from Customize after a change asks the save question over the
  scene alone (the Car Shop replaced, mode 2: 0x2042).
* **DIM on the icon menus' options** (**verified**, fixed): StartDim's
  IconScroller_FadeOff (0x51FD20) clears the strip's fade (+0xE4) and
  sets every option's colour from its item (+0x1C, FEObject_SetColor
  0x51D800); the handler's DIM sends 0xA301C015 and the options, clones of
  OPTION_N with its scripts, run its DIM: UI_PerformanceDyno_MAIN to
  alpha 0, UI_ProfileManager / UI_pause / UI_OptionsMain to 60, UI_Main
  and UI_CareerCrib to 0 / 40. The clone's icon menus took the master's
  alpha only, so the dyno menu's and the Profile Manager's neighbours
  stayed bright under a dialog; now `dim_alpha_of` (alpha by DIM alone,
  no fade by place). Wine: the dyno save question shows no icons
  (menus13 `dyno_save_ask`); under MC_List faint Profile Manager icons
  show through the "Load" bar (alpha 60).
* **The save prompts' focus** (**verified**): the register is 1 at every
  site: UI_Options' Back (0x49C8E7 / 0x49C92F, ebx = 1 from 0x49C893) and
  UI_PauseOptions_SaveQuestions (0x4A7F70: eax = 1 at 0x4A7F78; callers
  0x4A81F1, 0x4A8380) push 1 as 0x5587A0's eighth argument: No focused.
  Clone: `ask_focus` (was Yes).
* **Audio slider knob vs handle** (**verified**, matches): read in memory
  on UI_Options' Audio page: SLIDER_KNOB_1 z 1, colour (255, 255, 255,
  255); SLIDER_HANDLE_1 z 10 (INIT's), colour (34, 178, 99, 255): its
  FADEIN / HIDE tracks (z 1) never ran, the code places it
  (FEObject_SetTopLeft keeps the z). The clone already keeps INIT's z for
  objects the code places; the knob draws over the handle as under Wine.
* **The highlighted performance row's green** (**verified**, fixed):
  OptionRow_Highlight (0x52E130) moves the focus to the row's name first
  (0x52CE50 → Button_Highlight 0xABC08912), whose MsgR runs "Highlighted"
  (0xC2EA8AA0: colour 138, 244, 202, alpha 0 → 255 at 440 ms, held to
  760, 0 at 1200, looping), then plays Highlight on the name, data and
  backing; the queued message lands after it. Clone: `state.focus` on the
  highlighted row's name.
* **The Visual Rating widget on the neon / ICE lists** (**verified**,
  ported): Garage_PreviewPart (0x55C180) ends in RatingWidget_Update
  (0x55B1B0), which shows the widget (0x557ED0) when it is not up; it is
  called by NeonOverlay_Step (0x55CAD0), PartsOverlay_Step (0x560930), the
  decal overlay, the parts browser and the unique parts. Only a screen
  change pops it (FE_RatingWidgetForScreen 0x55BCA0). Wine (fresh
  session): the neon list opens on NO NEON without it, Down to RED shows
  0.10 — the clone's value (menus103's 0.14 came from that session's
  car). Clone: `Game::overlay_rating`, set by a step in the neon list or
  an ICE parts list (not Custom Gauges); shot `neon_rating_cust`.
* **The Visual Rating under the vinyl question** (**verified**, fixed):
  Ride_VisualRating (0x5234D0) sums Part_VisualPoints (0x5142E0) over
  the ride (Ride_SlotPart 0x610300: +0x590 + 4 × slot) and steps through
  the table 0x7FA230 (500 points a step): the clone's formula. Wine
  (fresh session, unlock-all 0x838464): Tear Set's vinyls 0.04 in the
  list and 0.04 under "add this vinyl to your setup?". The clone's preview
  ride read the dialog as the screen (0.00); now it follows the screen
  under a dialog (`preview_menu`).
* **SponsorPopup's frame pulse** (**verified**): Wine after S2_SPRINT_3
  won (s2a, the car moved onto the finish through memory): Event Results,
  Race Reward (300 / 300), "SELECT A SPONSOR!". 90 frames at 30 fps of
  the icon frame DF422345: on ~0.33 s, fading ~0.3 s, off ~0.36 s, back
  over ~0.2 s, period ~1.2 s — its script 0x5A8E4EBE (138, 244, 202; keys
  0 / 320 / 640 / 1000 ms, 1200 long, looping). The clone's popup sampled
  every 200 ms shows the same cycle.
* **UI_DecalsOverlayInvis** (**explained**): its objects are
  UI_DecalsOverlay's at the same places (the 120 option clones, column,
  bars, arrows), plus a "Total Cost" group (0x9FF84A88 with 0x1C663AA6 /
  0x2F5E7DA3) and the console footer strings (flags 0x40: hidden on the
  PC); some resource indices and the frame's z (10 vs 100) differ. So it
  looks like UI_DecalsOverlay; the clone keeps that layout. **Open**: the
  Total Cost group in a career's decal list (not captured).
* **The Custom HUD list's grey** (**narrowed**, open): at full speed the
  accept leaves the list at rest (menus112 h3's mid-move was the slow
  runner). All dots, the selected one too, read about alpha 60 / 255 over
  the column (background 130, dot 158) and the frame turns bright green.
  The dot objects themselves (PARTS_ICON clones 0x0937E8F1 of
  UI_CustomHUDOverlay, read in memory) are white with alpha 330, FADEIN
  done; no script of the overlay ends at 60. What greys them (a parent or
  a layer of UI_ChooseCustomHUD) is not traced.

## Menus round 15: Quick Race against the original, the Options pages, the dialogs' focus, the language character map (2026-10-31; checked against the vanilla original under Wine)

Wine: `$OB_PRIVATE/wine/runs/<run>` through `fastgpu.sh` (:152, fresh
profile NAME, no WidescreenFix), 1280×960; memory read with `peek.py`.
Images in `<work dir>/w` (original) and `c` (clone,
`NFSU2_SHOT_KEYS`, a new shot knob: menu keys sent to a shot's screen).

* **A new profile's Quick Race records** (**verified**, fixed): the
  profile reset 0x515AA0 runs 0x515800 on the block (profile +0xD4 =
  0x83AAB4) after the constructor's 0x52A230: every type 1 player, 3
  opponents, Medium, traffic 1, catch-up on, then Circuit's track 4021
  (0x515852), Sprint / Drag 1 lap (0x515845 / 0x515849), Drift traffic 2
  (0x51585C), URL 5 opponents (0x515894), type 5 1 opponent / Easy, Free
  Run 0 opponents / 0 laps, Street X Easy, traffic 0, track 4601, 10 laps.
  Memory on a fresh profile held exactly these. The clone started every
  type at 0x52A230's values (Street X showed 3 laps, URL 3 opponents, a
  downhill drift Minimum traffic); now `quick_race::new_profile_records`.
  This closes the "path A or B" question of "Traffic level": a new
  profile takes path B.
* **Mode select, track lists** (**verified**, match): 7 modes (Circuit,
  Sprint, Drag, Drift, Street X, URL, Free Run). Track lists on a fresh
  profile: Circuit 31 (2 open: Resort Loop, Outer Ring), Sprint 23 (1),
  Drag 10 (1), Drift 17 (2: Stadium Drift 2, Capilano Heights), Street X 8
  (1), URL 11 (1), in the clone's order; the panel's labels ("Best Time:"
  for sprint / drag, "Best Lap" and "0" for drift) and distances match.
  Moving in the list puts the track in the record with the direction
  forward; opening the list (0x4E2F30 at 0x4E303A) clears Lap KO — seen
  as Back from the options clearing it (Continue keeps it). The clone did
  both already.
* **Game Options** (**verified**): rows per type as `quick_race::rows`
  (Direction first on an open two-way track; Circuit Lap KO, Laps,
  Traffic, Opponents, Difficulty, Catch Up; Sprint / Drag Traffic,
  Opponents, Difficulty, Catch Up; stadium drift Laps, Opponents,
  Difficulty; downhill drift Difficulty, Traffic; Street X / URL Laps,
  Opponents, Difficulty, Catch Up). Ranges read in memory: Laps 1–10,
  Traffic 0–3, Difficulty 0–2, Opponents 0–3 (URL 0–5), all wrapping;
  Lap KO on sets 3 laps and 3 opponents and disables Laps and Opponents
  (Up / Down skip them). Fixed: (a) a disabled row shows no arrows (its
  arrows have no Disable script; 0x51E3F0 runs "Disable" on the row's
  +0x2C / +0x30 / +0x4C / +0x50 objects and its +0x24 group only); the
  clone left them in INIT and drew them. The mechanism (the arrows keep
  the list's unhighlight) is **inferred**, the look **verified**. (b) The
  scroll bar handle: at the top on row 1, 5/6 down on row 6 of 7, at the
  end on row 7 — 0x538090 with the row count; the clone passed count − 1.
* **The race honours the record** (**verified** in memory after the
  start): 0x89E7A0 track 4014, 0x89E7A4 reverse 0, 0x89E7A8 traffic 1,
  0x89E7B0 1 (3 only with Lap KO in game mode 2, 0x53F23F), 0x89E7BC 3
  laps, 0x89E7C4 3 opponents (4 racers on the HUD), 0x89E7F4 difficulty
  0, 0x89E7F8 catch-up 1 — the clone's `start_race` reads the same
  fields.
* **Car select** (**verified**, fixed): a fresh profile lists 206, Focus,
  Corolla, 240SX, 106, Corsa, Escalade, 350Z, GTO, Lancer Evolution VIII
  (All Cars). 0x534190 with the Quick Race mask 1 ORs 0x529F80 and
  0x529CD0; 0x529CD0 passes the car type ids in 0x7F7C28 (0, 1, 2, 7, 10,
  25, 28, 22; 0x7F7C08 with 5 for 7 is region 0's, the file's region is
  6). The clone listed the career lot's six; now
  `Profile::quick_race_cars` (the lot's plus those eight, type order).
  0x529F80's body is still only partly traced (**inferred**: it is the
  career unlock; 240SX and 106, without an unlock record, pass it).
  Customize's car select uses the same test (mask 1 outside game modes 8
  and 0x100), so its list changed the same way.
* **Transmission question** (**verified**, fixed): "Transmission:" over
  the car select dimmed (title, strip and bars dim, the car and garage
  behind), Auto focused. The clone drew it over black; now `dialog_over`
  with the select as the screen under (StartDim's look; whether 0x497D50
  calls StartDim is not traced). Shot `qrtrans` now runs the real Accept.
* **Loading and start**: the load showed no loading screen for long (the
  clone's 200 ms rule is the agreed deviation), then the start scene and
  the race; not compared frame by frame (race start scenes are the race
  endings / starts work's).
* **The dialogs' focused button** (**verified**, fixed): 0x5587A0 and
  0x558760 call 0x558690 with a constant 1 as its eighth parameter
  (0x5587F7 / 0x55877B `push 0x1`, the dialog's byte local_c); their own
  eighth argument goes to the ninth (local_1c, not traced). Under Wine
  "Reset to default settings?" (0x49B9C7, eighth argument 0) lights No as
  the sites pushing 1 do. menus13's reading (the eighth argument is the
  focus) was wrong; `ask_focus` is now No for every question.
* **"Reset to default settings?"** (fixed): over the Display page (rows
  unhighlighted, no StartDim) and the garage; the clone drew black.
  `menu_package` now names UI_Options' pages.
* **The language character map** (fixed): English.bin's string chunk has
  a 256-entry u16 map after its header (count 256 at +0x10): 0x81 ’, 0x82
  ™, 0x83 –, 0x84 ”, 0x86 …, 0x87 “, 0x88 „ (zero keeps the byte). Under
  Wine Options' "EA GAMES\x82 TRAX" shows "EA GAMES™ TRAX"; the clone
  dropped the byte (no glyph 0x82). The font code maps only 0x99 → ™ and
  0x9C → œ (0x50BC51, 0x51C3B5), so a conversion before it applies the map:
  0x50D3C0 (**verified** 2026-11-11, see "Native front-end fixes, part 1").
* **Options pages** (walked row by row against the clone): Options main
  (7 icons) match; Audio (8 rows, Audio Mode Stereo / Mono) match;
  Display (Resolution, Level Of Detail, Customize Display Settings,
  Default Settings) match, the clone adds Screen Mode, Aspect Ratio, Vsync
  Off Mode and Enhancements (its own); Gameplay (7 rows, defaults On, On,
  MPH, Medium, Rotate, Rotate, Prompt) match; Player (15 rows) match but
  **open**: Wheel Options is greyed under Wine without a wheel (the clone
  has it enabled; its enable test is not traced) and Wine's disabled
  Feedback row shows no backing band where the clone's does; Controls
  match but the key names (Wine's DirectInput names "Shift", "Control",
  "space" vs the clone's "Left Shift", "Left Ctrl", "Space": the OS
  provides them, **open**); EA GAMES™ TRAX match; Credits match but the
  scroll timing (known).
* **Graphics shop exit question** (**verified**, fixed; shot
  `graphics_exit_cust`): both sites name GenericDialog_SMALL.fng
  (0x562DD6 / 0x562F98 push 0x78FA24); the clone sized it by 0x526C50's
  rule (MED/LARGE box). The shop under it now keeps its selection
  (Decals) and Customize asks "install" (0x511CCA10) without the career's
  exit question first (`career_shop`, was `profile.is_some()`). Side by
  side with a private Wine copy `graphics_exit`: matches but the camera.
  **Open**: other 0x5587A0 sites that name a package where the clone
  sizes by rule were not swept.

## Screens round 160 (2026-11-01): 4:3 comparison fixes against the vanilla original under Wine

Wine: `$OB_PRIVATE/wine/runs/<run>` (fresh profile, main-menu Customize with
0x850078 / 0x83898C poked for the career shop screens) and
`$OB_PRIVATE/wine/runs/<run>` (the finished-career fixture
`the finished-career save` as profile the finished-career save), both through `fastgpu.sh`, no
WidescreenFix. Captures and side-by-sides in
`<work dir>/screens-tmp` (orig / cmp, REPORT.md).

* **Prices and cash** (**verified**): every price, the help bar's and the
  screens' cash print `%$d` (0x78F744: the number grouped by thousands,
  no currency sign; e.g. 0x5456DB, 0x547014, 0x5624CC); an owned part "0"
  (0x547032); two browsers use `%d` (0x5469D3, 0x54744A), the same below
  1,000. The clone printed "$1000"; now `grouped`.
* **Package discount row** (**verified**, 0x562405…0x562485, career and
  customize mode ≠ 1): 0x06E0C9A8 = "20% Discount:" (0x66559034) at
  CC_MEGALOW (the visited shop's +0x38 = 0x7FC8D0D0), else "10% Discount:"
  (0x6DA17A25), printed as a format ("%%" → "%"); 0xBE8693E1 the discount
  0x5415F0 returns (`%$d`) or "n/a" (0xA4C02484) at 0. Wine, a fresh 206's
  Street Package: "10% Discount: 150", "Total Cost: 1,350". Was hidden.
* **UI_BuyPerformanceParts' bar** (**verified**): the option list moves the
  bar with 0x538090(1, count, selection) (0x539324 …): one visible, the
  handle follows the selection and both arrows show from two rows.
* **Specialties lists' help**: UI_IcePartsOverlay's 0x409 (Install Part)
  while a list other than Custom Gauges is open (Wine, the finished-career save's
  Window Tint list); the clone said Continue.
* **Part names** (**verified** in the data): GLOBALB's part string pool is
  single-byte Windows-1252 ("BURGER KING\xAE", "EDGE\xAE ADVANCED\x99 SHAVE
  GEL"); the clone decoded UTF-8 and dropped ® / ™ (`parts::cp1252`).
* **Garage Car Select** (UICareerCribCarSelect, **verified**): the set-up
  0x4EE210 hides FILTERTYPE, CUSTOMIZE, JOY_EVENT(141) / (142) and
  Trade_Group (the "Trade - In Cost/ Reimburse:" bar); the constructor
  0x4F9AD0 pushes the Visual Rating widget (0x557ED0) and rates the
  entry's car (0x511E00 → 0x55B1B0), each step again (0x4B05DE; a locked /
  open placeholder pops it, 0x53D8D0); the bars (0x4962F0) rate the
  entry's car record (its performance parts, uniques and body aero, not
  the stock car); 0x4B0140 sets the scroll bar (0x538090(1, +0x58 count,
  +0x5C slot)); both logo arrows stay shown (Wine, slot 1 of 5).
* **Parts Storage** (the garage's Change Parts, customize mode 1,
  **verified**): UI_ChooseCustomizeCategory titled "Parts Storage"
  (0xE1497A54 on 0x42ADB44C, 0x556747) with Change Body Parts, Change
  Performance Parts and Change Interiors / Car Specialties (0x520D70 adds
  Paint / Vinyls only when 0x83898C ≠ 1 and the dyno only in modes 2–4);
  the performance categories titled "Performance Part Storage"
  (0xDCCD75F8, 0x556D81); in a career the packages are left out (0x556EF2:
  Choose Individual Parts and Return To Stock) and the parts list shows
  owned parts only (0x50F0D0's flags 2); UI_ICEMAIN titled "Car
  Specialties Storage" (0xF4B3B4FC, 0x54724B) without Custom Gauges
  (0x559737). Back returns to Parts Storage on the category, and from it
  to the garage on Change Parts. No exit question (customize mode 1). The
  clone opened the body parts browser directly.
* **World Map region entry** (**verified**): 0x4E0CE0 puts a region entry
  (0x4E0C30 → 0x4E0B50: table 0x7F4388 by stage, track 4500 + stage − 1,
  zoom window (163, 154, 350), (126, 93, 387), (17, 5, 480), whole map
  for 4 and 5) at the list's head, and the list opens on it and on each
  new stage: the stage's map, every row label and value "" (0x496EF0), no
  Select Event. An event (0x496D70): "Event Status" Completed / Not
  Completed, "Race Type:" the type alone (0x496D10: Circuit, Sprint,
  Street X, Drag, Drift, else "Untitled Race"), "Distance:" `%$.2f %s`
  (× 0.001 km, × 0.62137 mi by 0x83AA1A), then "Best Score:" (drift),
  "Best Time:" (sprint, drag) or "Best Lap:"; Select Event (0x2000000)
  joins the help (0x4CC140). Up / Down stop at the list's ends (0x4CC140
  returns there; the clone wrapped). The distance's source is **inferred**
  to be the track's +0x98 as Quick Race's.
* **Split hoods and door styles** (**verified**, `part_anim::Styles`):
  0x61F830 picks the hood variant from a kind-5 hood (ride +0x5B4): 4 with a
  non-zero VERTSPLIT attribute, else 2, and flags the car's `_STYLE22_HOOD`
  / `_STYLE22_HOOD_CF`; the doors' ANIMSTYLE of the door style (ride
  +0x810), SCISSOR (1) taking the wide body's WIDE<kind> (kind + 2). The
  draw (0x434190, group from 0x623540) plays a split hood's halves as
  2 + (g ≠ 0) or 4 + (g ≠ 1), L = length / (0.75 · speed) / 30, frame speed
  1 or (STYLE22) 0.75 / 1.25 × speed by half; a door closing from open
  keeps the variant it opened with (0x7EA610). `NFSU2_SHOT_FIT=slot:i`
  fits a part for shots. **Open**: not compared with a split-hood car
  under Wine.
* **Still open** (this round): the Custom HUD picker's grey dots (the
  overlay's PARTS_ICON clones have DIM / UNDIM / FADEIN / FADEOUT
  scripts between alpha 255 and 75 and two more pairs, 0x9326 /
  0x66EC937D, with the same keys; which one the picker runs is not traced);
  the garage camera's view per screen (idle tour, view blends) differs by
  timing in the captures.

## Screens round 2 (2026-11-01): fixes from the 1280×960 comparison against the vanilla original under Wine

Per-screen table in internal notes ("Screens round 2"); captures and side by sides in
<work dir>/screens2-tmp (orig/, cmp/, sheets/).

**Career World Map (UI_CareerWorldMap, the crib's).** Traced:
* The screen's arrows (0x4B1590, run after every step 0x4E0E50 / 0x4CC140 and by the list fill
  0x4E0CE0): first its SideScrollBar (bar at screen +0x19C, 0x542AD0 with "SideScrollBar" at
  0x4EE72A) by 0x538090(1, +0x228, 0x51F7F0 on the event list +0x138), +0x228 being the list's
  entries counted by 0x4E0CE0 (the region head 0x4E0C30 plus the events 0x4B1360); then, by the
  scrollers' ends, ScrollBar_Arrow_2 (+0x54) hidden on the last listed stage and ScrollBar_Arrow_1
  (+0x50) on the first, SideScrollBar_Arrow_2 (+0x58) on the event list's tail and
  SideScrollBar_Arrow_1 (+0x5C) on its head (the constructor 0x4EE690 looks them up at
  0x4EE798–0x4EE7DB). Wine: Stage 1 has no left arrow, Stage 5 no right one, the region entry no up
  arrow, the thin bar's handle at the top.
* The event list (+0x138, 0x5435E0 with "MINI_MASTER" / "EVENT_ICONS", vertical; extent +0x54 =
  273.0 and gap +0x60 = 20.0 set at 0x4EE842 / 0x4EE84C): one item per entry with the texture
  0x505450("U2_MINI_ICON") (0x4E0D9E), placed by 0x538770 → 0x52EEB0 as the parts column. The
  items' template z (130) is behind MINI_MASTER's U2_mini_icon_backing (z 10) and _highlight (z 1),
  so the selected dot shows dimmed through the backing (the parts browser's backing is at z 990,
  behind its items). Wine: three dots under the region entry's box; one above and three below an
  event's.
* The top scroller lists Stage n while 0x500C00(n) (stage record +0xFC) holds (0x4CBF80); a finished
  career (the clone's stage past 5) stops at Stage 5. **Inferred** for a stage-1 career, where the
  latch semantics (record n vs stage n) were not re-traced.
* GarageMain stays up under it (it is reached only from the crib): the garage and the help bar's cash
  show around the map, as under Wine.
* **Open**: the unselected dots' shade and size (the clone's are a little greyer and 3 px higher; the
  0.8 shrink of 0x52EEB0 about the leading edge, as in the parts column).

**Crib questions.** The crib's accepts (0x0C407210 in 0x4AF3A0) play the Event Handler's FORWARD
first (the objects' FADEOUT), so "Are you sure you want to quit to the main menu?" (0xCB5E3178,
0x4AF80B) stands over the garage scene alone, as Return to Explore Mode's does (Wine <work dir>
b_quit_q; the clone drew the menu dimmed under it). The crib's Save (0x4AF740 on 0x0C407210)
drops the snapshot (0x5032C0) and runs the save point 0x4A3100 with mode 0xC41 — not the Profile
Manager's 0x4A51: without 0x200, autosave on skips "Are you sure…" (Wine, the finished-career save: "Do you
want to overwrite previously saved game?" at once, No focused).

**Shot harness.** `finished_<screen>` now applies the save's own options as a real load does
(`Settings::apply_native`): the finished-career save is metric (km/h on the speedometer, km in the World
Map, as under Wine).

## Screens round 3C (2026-11-01): front end against the vanilla original under Wine

Per-screen table in internal notes ("Screens round 3C"); captures (`orig/`), clone shots
(`clone/`) and side by sides (`cmp/`, orig | clone | difference) in <work dir>/scrC-tmp.

* **Profile name entry** (MC_Main, **verified**): the text entry's white bar 0x557FE993 (1×18,
  U2_Menu_Box_White) has an INIT chained to 0x16A259, which parks it at (−10, −300), off the
  screen; no code brings it back. The caret is the field's own text: TextEntry_OnTick 0x540C00
  calls 0x540AC0 on every front-end tick 0xC98356BA, which writes the buffer (0x83A8B0; '*'s in
  state 5) plus "|" (0x79BFC4) while the counter +8 (`c + 1`, back to 0 at 0x5A, so 0..89) is
  under 45, else " " (0x7909A0): 45 of 90 ticks (**verified** 0x540B2A–0x540B47; the clone
  showed 44 until 2026-11-11). Under Wine (fastgpu, 120 fps cap) the caret is on 8 frames and off 7 at 20 fps
  sampling, ~0.75 s a cycle. The clone counts 0.12 a ms (one tick a frame at the original's
  120 fps cap: **inferred** — the tick rate under other frame rates is not traced).
* **MC_List scroll bar** (**verified**): MC_List_Ctor 0x4D6D50 builds its "ScrollBar" with
  0x542CB0, which hides (0x50CA00) the handle and both arrows (+0x78…+0x80); 0x4E5D40's own
  constructor hides ScrollBar_Arrow_1/_2, _Handle, _Backing and 0x9C7C3C73 for its list kind.
  The backing (0x688A1551, INIT chained to 0x16A259, alpha 0) comes up with the screen's
  0xBDFD2DF9 → 0x5B0D9106 fade. Under Wine (one profile, Delete) the backing shows and the
  handle and arrows do not; the clone had the handle at the bar's head and both arrows.
* **Shots**: `NFSU2_SHOT_DATA=<dir>` makes a shot use that folder as XDG_DATA_HOME (a copy with
  profiles, e.g. the Profile Manager's Load / Delete lists) instead of an empty temporary one.
* **Fixed later** (see "Screens fixes (scrfix)"): the EA Login dialogs' wrap (the renderer's
  own breaker 0x51C110 / 0x50BCE0 at the string's `Sw`, not 0x520080), their help bar, and the
  Controls key names (the clone's are the English build's; Wine's differ).
* **Observed under Wine**: Enter on MC "Load complete." right after a mouse move to the
  window's corner froze the vanilla game twice (game clock 0 %); not a clone matter.

## Screens round 3B (2026-11-01): career shops, dyno and garage against the vanilla original

Wine: `$OB_PRIVATE/wine/runs/<run>` through `fastgpu.sh` (DXVK, 1280×960, no
WidescreenFix), the finished-career fixture `the finished-career save` (LANCEREVO8,
km/h) and the early `stage1_win_streetx.sav` (206, mph, 275 cash). The shops
were entered for real from free roam: the player's car was moved onto each
shop's world trigger (`bin/tp.py`, every copy of its position) and the engage
dialog answered. Captures, side-by-sides and contact sheets in
`<work dir>/scrB-tmp` (orig / cmp / sheets, results.tsv). Per-screen
table in internal notes.

* **Visual Rating widget on entering a shop** (**verified**): only 0x557ED0 /
  0x55B1B0 push UI_Menu_Asset_Reputation, from 0x55BCA0 on a screen change
  (0x55BD70 / 0x55BE00) or a preview refit; entering a shop from free roam is
  not one, so the Body Shop's and the Graphics Shop's first screens show no
  widget until a list is opened (then it stays, Back included). The clone
  showed it at once (`rating_up`).
* **Prices of owned parts** (**verified**, 0x5456DB, 0x547014, 0x546F50):
  every list prints `%$d` of the part's cost (0x514280), owned or fitted
  alike; only a decal slot's "No Decal" hides the price (0x4954E0 on
  0x380B632). Wine: the fitted SPYDER bumper "1,000", DARK BLACK tint "200",
  XENON 8000k "550". The clone printed 0.
* **Rim brands** (**verified**, 0x546530): the strip opens on the fitted
  rim's brand (attribute 0xEBB03E66), Wine O.Z.
* **Scroller icons** (**observed**, Wine body shop / specialties): the
  unselected icons of the parts browser strip and column shrink about their
  own centres (the clone anchored them at the leading edge: 25 px off).
* **Performance shop** (**verified**): 0x530500 appends "Performance Tuning /
  Dyno" after Turbo in a career shop (0x850078 = 1, 0x83898C = 0), asking
  0x54B649F1; a category opens on Choose Individual Parts (0x556E70 selects
  entry 1 and pushes UI_BuyPerformanceParts), the strip's frame off there; Return
  To Stock's help mask 0x405 (0x5501C3); the rows' Purchased runs on the data
  only (0x530620 on +0x2C / +0x30; the name stays white, **inferred**
  mapping); the readouts in Nm / kW with 0x83AA1A = 1 (0x562540).
* **Dyno** (**verified**): UI_PerformanceDyno_MAIN's mask 0x4 while 0x83A7A4
  (Continue alone, 0x5573ED); 0x83A640 (`g_DynoFrom`) = 1 from the shop:
  Quit Tuning asks "Quit to Performance Shop?" (0xB3FB59A9, 0x564E16) and Yes
  returns to UI_ChoosePerformanceCategory (0x55A7B3, Wine: on Engine).
* **Tuning screens**: values printed as the game's bPrintf (0x43E950: digits
  truncated, the last rounded up at ≥ 5 of the rest; 8.125 → "8.13",
  `bprintf_fixed`); the sliders' list bar 0x538090(1, rows, selection) (Wine:
  bare track with one row); the graph titles "Torque ( %s )" with Nm / ft-lbs
  (0x5507B0, 0x550D3B), "RPM (x1000)" / "Duration (s)" (0x549FE0, 0x54A270),
  the curve's y numbers min + k·⌊(max − min)·0.2⌋ (0x54A13C), the nitrous
  x numbers 2c to the duration's max (0x54A270); the nitrous y step (fixed
  later, "Screens fixes (scrfix)").
* **Specialties** (**verified**, 0x54FB80): list help masks Test N2O Purge
  0x80409, Open/Close Hood 0x100409, Open/Close Doors 0x200409; the stock
  trunk's Trunk Layout opens with its list and the widget (Wine); UI_ChooseSpinner's
  strip hides its left arrow and column bar as UI_Rims_Browser.
* **UI_Paint**: no finish arrows while no slot is entered, and no swatch
  panel on a multi-colour vinyl's colour strip until a colour is entered
  (**observed**; what hides them is not traced).
* **Open**: Custom Gauges list order/content (the known Custom HUD family);
  a decal slot's list (original one entry per brand, alphabetical, no
  5ZIGEN; the clone lists both texture variants of each); the Lights list's
  preview lamps and the spinner preview (3D).

## Screens round 3A (2026-11-01): in-game and career screens against the vanilla original under Wine

Per-screen table in internal notes ("Screens round 3A"); captures, side by sides
(orig | clone | difference) and contact sheets in `<work dir>/scrA-tmp`
(`orig/`, `clone/`, `cmp/`, `sheets/`). Original: fastgpu (DXVK, 1280×960, no WidescreenFix),
saves garage.sav (stage 1 free roam), the finished-career save (World Map races, Quick Race with the
RX-7) and a fresh profile (the D-Day intro); finishes by finish151's racer-block write
(`bin/fin.py`). Clone: the same saves through `NFSU2_SHOT_SAVE`, `--bench race` dumps for the
Quick Race HUDs.

* **The pause's Player page, Feedback row** (**verified**, fixed): the row is its own class
  (vtable 0x791DB0, built at 0x4B80BD / 0x4C4971 with 0x5B8250's force-feedback flag). Its
  Disable (0x4B5FA0) runs "Disabled" (DISABLED, 0x06B55037) on the name and the data only
  (0x51CF80), not the list rows' 0x51E3F0 "Disable". UI_PauseOptions' Feedback objects carry
  both scripts; under Wine the name's current script read from memory was 0x06B55037 and
  "Feedback:" drew grey (0x9B) and still; the clone ran DISABLE (the HIGHLIGHT-like pulse).
  `options::draw_rows`.
* **HUD_Drag's turbo gauge** (**verified**, fixed): 0x4C6B80 with boost calls FEObject_Show
  (0x50CA50) on its six objects every frame, which clears an object's own invisible flag;
  HUD_Drag ships 3rdperson_Turbolines / TurboDial / TurboRed_1 / _2 with it set (flags
  0x40000001), so the clone's drag HUD on a turbo car (the stock RX-7) showed only the fill.
  Now the scale "+ 30 20 10", the red marks and the needle show as under Wine. `hud.rs`.
* **"Would you like to race this event?"** (fixed): the garage World Map stays drawn and
  dimmed under the question (Wine, finished-career save); the clone drew the garage alone
  (`menu_package` now names UI_CareerWorldMap, the ask goes through `dialog_over`).
* **Career Status' Visual Rating in the D-Day intro** (**verified**, fixed): UI_Status_Master
  passes the driven car's ride (0x4E1526 → player 0x8900AC → car +4 → +0x10) to 0x53D880 →
  0x5234D0, so with no car of the player's own it rates Rachel's DDAY_PLAYER_CAR (three and a
  half stars on a fresh career under Wine); the clone rated the empty profile car (none).
* **Shots** (no game change): `replay` started inside the race's start scene, so no recording
  and no bar showed (now from the green light, as `finish`); `intro` / `intro_prompt` /
  `intro_gps` stepped the HUD only at the render, so the D-Day phone prompt (kind 0x14) stayed
  in state 2 (phone icon) — with the HUD stepped every frame it rests (state 4, 0x4ABA30 sets no
  texture) and the D-pad shows the cingular icon 0xEA570722 as under Wine (the slots in memory:
  0x14 and 0x13 both state 4).
* **Open**: (1) the autosave overwrite question answered No: under Wine (finished-career save, autosave
  and prompts on) the post-race save point asked "Do you want to overwrite…" then, on No,
  "Are you sure you want to save the profile named the finished-career save?", and the autosave flag 0x844AD9 read
  0 afterwards (the MC result handler 0x4C6589 writes it); the clone goes on after the No.
  **Fixed 2026-11-04**: traced and ported, see docs/save-points.md. (2) The in-game World Map / minimap in the D-Day intro: the original
  (script run) lists Car Lot, Circuit, Sprint with the GPS on; the clone's career_start.sav
  shot has not run the script (state not matched). (3) The free-roam engage widget with the
  shop marker, the loading screen (too short at full speed to catch), sponsors, tournaments,
  rewards / unlock cards, the magazine / DVD shoots and the outrun were not reached this round.

* **Fixed later** (see "Screens fixes (scrfix)"): the Custom Gauges list's
  opening entry, a decal slot's list, the Lights list's lamps, the spinner
  preview, the nitrous y step, the Test Run pause. Still **open**: the
  carbon trunk's shade (3D).

## Screens fixes (scrfix, 2026-11-01): the open items of rounds 3B and 3C

Traced in SPEED2.EXE; compared with the vanilla original under Wine (fastgpu, DXVK,
1280×960, `$OB_PRIVATE/wine/runs/<run>`, the finished-career save, shops entered from free roam)
and the round 3B / 3C captures. Side by sides in <work dir>/cmp.

* **Custom Gauges list** (**verified**): UI_CustomHUDOverlay's constructor 0x56C220
  first runs the parts overlay's base 0x56BF80 (whose 0x5606F0 fills the list and
  selects the fitted gauge), then 0x53C200, which clears the list (0x5385D0), fills
  it again from 0x539970 (the slot's parts in the database walk, 0x61BA40 /
  0x61BA60, that pass 0x52A020(0x50F0D0(slot, part, 0x8389B0)); split hoods kind 5,
  spinners by car type) and selects the **first** item (0x5386B0(head)). So the
  list opens on STOCK at the top: under Wine STOCK, FLAMES, GRIFFIN, TABBY …, the
  walk order of `slot_walk LANCEREVO8 A3`. The clone opened on the fitted gauge
  (RETRO, the dots above it shown); now on the first, its column as the original's
  (no dots above the top entry). The picker's grey dots (0x838560 > 1) stay
  **open** as before: 0x563D00's entry into the picker only sets the packages'
  controller masks (0x537960: package +0x1C…+0x28 = 0 / 0xFF), not a colour.
* **A decal slot's list** (**verified**, UI_DecalsOverlay 0x568460 → 0x560260):
  "No Decal" (part 0), then 0x5204F0(slot, list, +0x112) for slots 0x61–0x98: the
  slot's parts in the walk order, no level or unlock test, kept when
  `(NAME ≠ BRAND) ≠ flag` — NAME the part's attribute 0x0019CBC0 (`"NAME"`,
  0x79AC30, hashed by 0x43DB50), BRAND 0xEBB03E66; the flag +0x112 is set by the
  constructor when the slot's decal (the setup's entry, 0x8389C8 list, when its
  part is non-zero, else the car's 0x610300 on 0x8392C0) has NAME = BRAND. Each
  brand has two texture variants, one with NAME = BRAND: the list shows the other
  (one entry per brand). Each brand must pass 0x53B510 (ECX 0x838980): in or out
  of a career 0x529E90 (a brand no performance part carries, 0x502310, or one in a
  career car slot's brand list +0x6E0), everything with g_UnlockAll; game mode 8
  uses 0x534120 (not ported). 5ZIGEN (a performance brand not yet bought) drops
  out; the list then starts at A'PEX INTEGRATION as under Wine. The list opens on
  the slot's decal (0x51F7B0) when listed, else on No Decal (+0x114 is 0).
* **Lights list lamps** (**verified**): entering a UI_IcePartsOverlay list calls
  0x54FB80(item, 1); for slot 0x9F (Lights) it sets g_FECarLightsOn 0x8A1C68 (the
  list's exits 0x560642 / 0x5569EE / 0x568A21 clear it; memory under Wine: 1 in
  the list, 0 on the strip). 0x61ACF0 draws the FE render infos' light flares
  (0x615640) only with it set and g_LightGlowEnable (0x870CE4) on (0x8026C8 and
  0x7F3D40 are never written). The FE render info has no car object, so the
  headlights draw at 1.0 in the render info's headlight colour (+0x380: the lights
  part's RED / GREEN / BLUE, the list's highlighted part being fitted on the
  preview), the brake lights at 0.85, the centre one at 0. The clone draws them
  (`glow::car_sprites`) in the garage scene with the neon, the glow textures
  (HEADLIGHTFLAREINNER / OUTER / HEADLIGHTGLOW) taken from GLOBALB like NEONBLUR.
  **Inferred**: drawn into the scene with the neon, not through the glow / streak
  pass (the original's halo is wider and bluer).
* **Spinner preview** (**verified** by the frames): the FE car is drawn by
  0x620AF0 (0x631239), which draws the rim part's group 1 (the spinner) too; the
  clone had marked the spinner in-game only. Under Wine the Spinners strip's
  preview wears the highlighted spinner's blades; now the clone's too (the blades'
  angle differs: their phase).
* **Nitrous graph's y step** (**verified**, 0x54A270 / 0x550850): the scale is the
  widget's +0x78 (0x550850: parameter 17's max, 1.0 when 0) × 100.0 (0x784258, at
  0x54A3D5); its _ftol picks the step (50 below 250, 200 above 499, else 100) and
  its inverse places TORQUE_DATA_c+1 at y0 − (H − 16) · c · step / scale. The
  clone used the bare max (≈ 1): only "0" showed; now "0" and "50" as under Wine.
* **FEng string wrapping** (**verified**): an FEString with a wrap width (`Sw`,
  +0x74, read at 0x53504E) is broken by the renderer itself: 0x51C110 draws, with
  the line widths from 0x50BCE0 and the block's line count from 0x50C080. A line
  adds each glyph's advance (+0xE) and, after its first character, the kerning
  after the previous character (0x6F45F0); once the width reaches the limit
  (limit ≤ width) with more text to come and a space seen, the line ends at that
  space (0x50BCE0 keeps the width before it; above 0 only) and the next starts
  past the spaces. No 16-pixel margin: that belongs to 0x520080, whose only caller
  is the sponsor terms' 0x544700 (it also kerns each character with itself). The
  renderer's flag that a line was broken by the width (0x50BCE0's last argument)
  is cleared once per string, and with it up the drawing ends a line as soon as
  its drawn width reaches the measured one (measured <= drawn, checked after
  every character): a line ending at a "\n" / "^" after the first wrap then
  breaks there and again at the "^": a blank line (the lobby-cancel question's
  blank line before "Are you sure…" under Wine). The measure (0x50BCE0) kerns
  from the line's second character, the drawing's counter (local_2c) only
  restarts at the string's start, so a line's first character is kerned against
  the character before it (the space the wrap skipped, or the "^"): when that
  kerning is negative the drawn width stays below the measure, the "^" breaks
  once and no blank line shows (**verified** by code; **observed**
  under Wine in a stage-4 playthrough: IND_GRAPHICS_BODY "…and new ^Vinyls. ^All
  parts…" has no blank line, " V" kerns; IND_PERF_SHOP_2_BODY's "Brakes
  performance upgrades. ^" has one). Of the 19 shops' `_BODY` texts only
  IND_GRAPHICS_BODY changes; UC_PERF_SHOP, IND_PERF_SHOP_1/_2, SH_BODYSHOP,
  SH_GRAPHICS, IND_BODYSHOP_1 and IND_CAR_SPECIALTY_2 keep their blank line;
  the retry text 0xC4DF3B44 and the race engage texts (ENGAGE_<event>,
  ENGAGE_<set>_RANDOM_n) have no "^" and no blank line
  (`engage_wrap_tests`). The block's height uses 0x50C080's count,
  without those blank lines (`line_count_wrapped`). The EA Login texts (Sw 720)
  now wrap as under Wine. The SMS body rows and the sponsor terms keep their
  0x520080-style breaker (**not re-traced** for the SMS rows).
* **Help bar under the EA Login notices**: UI_PC_Help_Bar is a package below the
  dialog's on the FE stack, so the dialog's backdrop darkens it (Wine ol_lost: the
  labels at 0.40 of white). The clone drew the bar after the dialog; it now draws
  it under the dialog when the bar stays up there.
* **Controls key names** (**verified**): 0x5B8DA0 returns DirectInput's object
  name for the key, remapped by the system language g_SystemLanguage 0x7FCA00
  (registry value "Language", table 0x7FCA08: "English US" / "English UK" → 0,
  then French, German …; −1 when no value matches): with 0, "Shift" → "Left Shift",
  "Alt" → "Left Alt", "Ctrl" → "Left Ctrl" (0x5B901A…); Dutch (5), German (2) and
  Swedish (6) have their own tables. Under Wine 0x7FCA00 is −1 (the prefix has no
  matching Language value; adding "English US" to the 32-bit key did not change it:
  **open** why) and Wine's DirectInput gives its X11 names ("Shift", "Control",
  "space"), so the original shows those. The clone keeps the English / US build's
  names (Windows' DirectInput "Shift" / "Ctrl" / "Space" after the remap: "Left
  Shift", "Left Ctrl", "Space"), as decided for the English build.
* **Test Run pause** (Wine, the finished-career save's IND_PERF_SHOP_2 dyno, Circuit): the
  Dyno Shop strip over the paused track, Resume Test Run, Restart, Tune Settings,
  Return to Dyno, Options, Quit Tuning, help bar Back + Continue: the clone's
  matches (`--shot finished_testrun_paused`); the track's tone under it differs (3D).
* **Vinyl colour strip frame** (**observed**): the strips' selection frame pulses
  fully on and off in the original (round 3B's paint strip captures alternate:
  frame on in r1 / r3 / r5, off in the others); b_vinyl_yes caught it off. The
  clone's shots sit at one phase: animation phase, not a difference
  (**inferred** that the vinyl colour strip pulses as the paint strip).
* **Not reached**: THE_UNIQUES_SHOP (only through a world challenge).
* **Open**: the carbon trunk's shade (the CARBONFIBRE remap's shading on the car;
  the clone's shows a bright band, the original's is dark).

## Native dialogs: My Cars limit and Custom Gauges bank (2026-11-11)

Traced in SPEED2.EXE (dis and REA/Ghidra); not re-run under Wine.
Tests: `profile::b5_tests` (the conditions), game tests
`native_dialogs::*` (the key path on a sandbox profile).

* **My Cars limit** (**verified**, dis 0x5165E0-0x516612,
  0x4FC2E5-0x4FC344, 0x4FCAF0-0x4FCB0E, 0x4FCC6A-0x4FCCA5).
  `MyCars_FirstFree` 0x5165E0 walks the garage's 20 records (`< 0x14`,
  +0x54C + 0x434·i) and is true at the first one whose id is 0; false
  when all 20 are used (`Profile::my_cars_free` = None).
  - UI_QRCarSelect's Accept (0x4FC2E5, once ready +0x1A7) in game mode
    0x20 (main-menu Customize, 0x4FC301) always tests, whatever the car.
    Full: 0x558580(screen, "" = the auto-sized GenericDialog 0x526C50,
    OK 0x639, accept and back 0x34DC1BEC, text 0x91921A8D), then
    0x50D180(4), and returns: the category screen is not opened.
    0x34DC1BEC (0x4FC2C6) only plays UNDIM on the screen: it stays on
    the car select.
  - Quick Race's Customize (BUTTON1 0xC519BFC0, 0x4FCAF0) tests only
    when the car is a stock entry (0x516360); a My Cars record is
    customized itself even with all 20 used. Full: the same dialog
    (0x4FCC6A), then the screen's event sound 0x50D150(8).
  - Clone: `Game::customize_car_select` (the dialog over the car select,
    `again`); since the save-points pass the Quick Race path skips the test for a My Cars
    record.
* **Not enough bank for a custom gauge** (**verified**, dis
  0x563D00-0x563DA1, 0x564096-0x564142, 0x564145-0x5642B9; strings
  0x78FA24 "GenericDialog_SMALL.fng", 0x79031C "UNDIM", 0x799AC0
  UI_ICEMAIN.fng, 0x79D820 UI_ChooseCustomHud.fng, 0x799A60
  UI_CustomHUDOverlay.fng, 0x799C20 UI_ChooseCustomHUD.fng). The
  overlay's handler (Ghidra starts it at 0x563D10; the code begins at
  0x563D00) runs the purchase on Accept / 0x9E382ECD (0x563D6C →
  0x564145) once the picker is at state 0 (0x838560, after the needle
  colour). With a career (0x850078 == 1) and a gauge with a HUDINDEX
  (0x61B510 of 0x311151F8 ≠ 0), not the fitted stock one (0x50F1B0 /
  0xDA15D7E5): the bank [0x861E74] is compared with the price (0x514280
  on 0x85AD40) **signed, `jge` buys** (0x56421D), so cash < price is
  short and cash = price buys (`career::short_of_bank`). Short:
  0x558580(screen, GenericDialog_SMALL.fng, OK 0x639, accept and back
  0xB4623F67, text 0xD1AC60BE), 0x838970 = 0xB4623F67 and the screen's
  DIM (0x560B70); nothing is bought. Its OK (0xB4623F67, 0x5640BD):
  UNDIM on UI_ICEMAIN, the overlay and UI_ChooseCustomHud (0x537C10 ×3);
  with 0x838560 = 0 the overlay (0) and the picker (0xFF) are shown again
  (0x537960) and 0x838560 = 4: the needle colour step, the picks
  (0x839C24 …) kept. With enough bank the handler asks the confirm
  (0x5587A0: 0x44B7167F, Yes 0xF770 → 0x9F00F07, No 0x61C); a stock
  gauge or no career asks 0xF5AC2B2A (Yes → 0x51123592).
  - Clone: `Game::buy_custom_gauge` shows the dialog over the picker and
    OK returns to `ChooseHud` at step 2 (the needle) with its picks.
    **Open / not ported**: the DIM under the dialog (the clone has no
    UI_CustomHUDOverlay screen of its own) and the confirm question
    before a purchase (the clone buys at once, as before).

## Failed loads remembered (2026-11-11; clone-side)

The original loads the garage platform and FE car (once per visit), the
mix map (per race kind, 0x486920) and the TRACKMAP texture (with the
track) once, and shows or plays without them when a load fails. The
clone asks for them from the frame, so a missing or broken file used to
be read (and the garage one logged) on every frame. `failed_load::
FailedLoads` keeps the failed requests by key; each failure is logged
once; a success, a changed key, or the owner's context built again tries
again. Loads that succeed are unchanged.

| loader | key | kept | tried again | test |
|---|---|---|---|---|
| garage platform (`garage_platform`) | platform name | 1, per thread | other name; the garage closed (next visit) | `failed_garage_tests` |
| garage FE car, part-change job, built ahead | (car, parts, perf parts) | 4 per visit (`GarageScene`) | next visit | `failed_garage_tests` |
| mix map (`load_mix`) | race kind 0 / 1 / 2 | 1 (the audio state) | another kind, then back | `failed_mix_tests` |
| TRACKMAP (`Assets::track_map`) | track number | 16 (`Assets`) | other numbers; a successful map load | `failed_track_map_tests` |

The music stream's failures are held off by its own back-off (
`streams::music_manager::Backoff`).

## Native front-end fixes, part 1 (2026-11-11; **verified** from SPEED2.EXE)

The clone's front end against the binary answers of audit/binary
of the front-end audit and the Wine answers. One row per fix in
docs/native-fixes/N-FE-1.tsv; tests in `src/frontend/native_fe_tests.rs`.

* **List windows and scroll bars.** The generic text list (ctor 0x542CB0:
  +0x18 rows, +0x1C count, +0x20 the window top's index from 1, +0x24
  current, +0x28 top item, +0x9C bar, +0x9E wrap, +0xA0 bar mode) steps with
  0x542E00 when +0x9E = 0: the current moves to the next enabled item (no
  wrap); Down moves the top one item when the new index ≥ rows + top; Up
  moves it when the selection goes above the top item. Its bar 0x5384A0
  calls 0x538090(rows, count, +0x20) — the window top, not the selection;
  the (1, count, current) branch needs +0xA0, which only the ctor writes
  (0, 0x542D71). MC_List (0x4D6D50, args 1, 0, 0, 1) and the EA Trax
  jukebox (list at +0x4C, 0x553CD2, args 1, 0, 0, 1; its 0x54B320 passes
  +0x64 / +0x68 / +0x6C) therefore place the handle by the top row; MC_List
  now keeps a sticky window (`McList::top`). The option lists (0x552790,
  one row visible) follow the selection as before.
* **FormatRaceTime.** `time_text` (0x577250 flags 0 → 0x576D40) is the one
  native function behind the HUD clocks, the World Map record (0x496D70,
  `push 0` at 0x496E9D), the Quick Race record and the results' finish
  times; the former `race_time` (seconds, rounded) and `format_race_time`
  (seconds × 100 in f32, truncated) were the same function and are merged
  into it. Seconds t / 4000, hundredths (t % 4000) × 1000 / 4000 / 10, all
  integer and truncating (0x5771C0). The signed variants stay separate:
  flags 2 (`signed_time`, last digit dropped) and flags 10
  (`signed_time_text`, flag 8 keeps it) are different flag sets.
* **"%$.1f" rounding.** 0x43E950 takes every %f through one digit loop that
  rounds the last digit half up (0x43EF92); `grouped_1`, `fixed_grouped`
  and the DVD ratings ("%$.1f" in 0x35CCC129 / 0x030353CD) now use
  `grouped_fixed`. PostRaceStats top / average speed (0x4BFA30 / 0x4BFAA0)
  print "%$3.1f %s" (0x7951F0) of the float product (× 3.6 0x7904FC or
  × 2.237 0x78426C): grouped, half up.
* **DVD covers.** A stage record has 4 cover slots (0x835C9C + stage ×
  0x28: 4 × {f32, u8}, count at +0x20; writer 0x528A70, reader 0x497330);
  `career::stats::DVD_SLOTS` = 4 caps the list (it was 5).
* **Name caret.** See "MC_Main name entry": 45 of 90 ticks.
* **UI_Paint finish arrows** (0xCF6F0439 +0x150, 0xABA93D3F +0x154): hidden
  by the ctor 0x56AEC0 (0x56B04E / 0x56B05A); entering a slot (0x55CC60)
  shows them only for 0x3F, 0x45, 0x4B, 0x4C (jump table 0x55CE88 by the
  byte table 0x55CE90; show 0x55CD95 / 0x55CDA8 after script 0x78F970);
  leaving (0x55CF0E) runs 0x790A40 and hides them; 0x563AFB / 0x563B96
  hide them too (which messages those are is not read).
* **Rim brand name** (0x50E960): the career brand table (0x838400, chunk
  0x34A14) first — its +0 name string, which for 5Zigen (0x352D08D1, the
  only rim brand there) is "5ZIGEN" — else a fixed switch of label
  hashes (equal to the third column of `RIM_BRANDS` / `SUV_RIM_BRANDS`;
  it also maps 0x365769E → 0x30B938BE, which no clone table lists).
  `screens::rim_brand_name` implements it; UI_Rims_Browser's title already
  did, and now the unique-part name (`unique_part_name`) does too.
* **Trick popup points** (0x4AB7A3): points / count through `_ftol`; a
  count of 0 gives the integer-indefinite low dword 0, so the popup reads
  "+0" (the clone printed 2,147,483,647). `hud::ftol`.
* **Language text** (0x50D3C0, ECX = the map 0x8383DC): bytes under 0x80
  are themselves; else v = map[b]: 0 keeps the byte, ≥ 0x80 is the
  character, 1..0x7F a double-byte lead whose next byte (always taken) under
  0x80 gives '_' and else indexes entry v·128 + d − 0x80 ('_' when 0); the
  text ends at the first 0 written. Without a map 0x43E000 widens. English
  has one lead entry (0x80 → 2) and no string with byte 0x80, Korean seven
  (985-entry map): only Korean text changes. The global 0x838530 (raw
  bytes when set) has no traced writer and is taken as clear.
* **Not changed, with the reason.** Opponents row wrap and the 4402 drag
  clamp (0x4B37F0 / 0x4B3750): already native in the base. Lap display at
  the finish: under Wine the element natively stops updating, the clone's
  clamp is equivalent. The bit-31 group skip of 0x52C5E0 (proposed to
  replace hud's `UNTRACED`): the skip only applies while the FE manager's
  byte +0 is 0, but its ctor 0x536180 sets it to 1 (0x536262) and no
  writer of 0 was found, so natively it looks bypassed — **open** until
  `[[0x8384C4]+8]` byte 0 is read under Wine in a race.

## Native front-end fixes, part 2 (2026-11-12; **verified** from SPEED2.EXE)

What part 1 left plus the Wine answers. One row per fix in
docs/native-fixes/N-FE-2.tsv; tests in `src/bin/game/tests/native_fe_part2.rs`,
`native_dialogs.rs`, `src/frontend/native_fe_part2_tests.rs`, `hud/tests.rs`.

* **Custom Gauges purchase** (UI_CustomHUDOverlay handler 0x563D10). The
  overlay's constructor 0x56C220 sets the step 0x838560 = 1; Accept on a
  gauge (0x563D6C → 0x564145) with the step > 0 opens the picker for a
  HUDINDEX ≠ 0 and does nothing for the stock gauge (0x564190 → 0x5644D9).
  The picker's needle Accept sets the step 0 (0x56455C) and sends
  0x9E382ECD, which reaches the purchase: in a career the bank dialog
  (0xD1AC60BE) or "Would you like to purchase this part?" (0x44B7167F, Yes
  0x9F00F07), outside one "Would you like to install this part now?"
  (0xF5AC2B2A, Yes 0x51123592), Yes / No 0xF770 / 0x61C with No focused;
  then 0x838970 = 0xB4623F67 and 0x560B70 plays DIM on the overlay's
  0x47FF4E7C. No / OK (0xB4623F67) undims and puts the picker on step 4.
* **Body shop question** (UI_GenericParts_Browser 0x5664A0; Select
  0x0C407210 and Accept 0x406415E3 share one block, so the mouse's
  MINI_MASTER Select asks too): 0x566D98 — fitted (0x50F1B0) → "already
  installed" (0xDA15D7E5); career, unowned (0x521BF0), level > 0 (+5 &
  0xE0): bank short → 0xD1AC60BE (GenericDialog_SMALL), else 0x44B7167F;
  otherwise 0xF5AC2B2A; the texts on the length-sized package (0x783FE5),
  masks off, strips' fade off, DIM on the browser (0x566748).
  `Game::body_part_question`. The ICE parts overlay's and the neon
  overlay's questions: ported (below, "Front end on the real-time clock").
* **Online** (UI_OLISPConnect, 0x4B97D0): 0x49DF20 returns only 0 / 1, the
  error dialog is unreachable; under Wine offline EA Login follows ≈ 0.13 s
  after the key with no ISP frame. The clone leaves at the next tick.
* **Race flow state** [[0x8900D8]] (SoundFX, 0x484E82): 3 in the start
  scene only, 4 from the countdown, still 4 through the 3 s race-over hold
  after the player's finish, 7 after it (`Game::race_flow_state`). The
  start scene's letterbox lasts through the frame at mark 2 (status 3 the
  frame after the clock passes header +0x30).
* **Shift light** (0x4C7090 at 0x4C784E): hidden while 0x5ECD80
  (transmission +0x48, `Drivetrain::pending`) is set.
* **Trick popup +0x24** (0x4C7E20 never writes it): a new popup takes the
  value of the last freed popup block (LIFO; 15 of 70 started with 5 under
  Wine), so a one-off popup after a faded one may not fade out. The LIFO
  allocator is **inferred** from the reuse pattern.
* **"Hurry!"** (0x4EBB10): the step 0x865150 is the frame's world step (0
  while paused); the clone uses the hurry clock's move since the HUD's last
  update instead of the drawn frame's wall time.
* **Front-end clock** (0x5662F9, 0x558DAB): unchanged; the original's
  per-frame truncation and 20 ms cap are rate dependences left out on
  purpose (equal at a steady 60 fps; docs/native-deviations.md).
* **Engage message** (0x512620 from 0x4B551B): none for stage 0, then SUV
  (9), sponsor sets, URL (12), kind sets; one 0x43C1C0(n) + 1 draw at the
  dialog's set-up, on the shared generator `Game::rng` (docs/rng.md: the clone's
  career generator is gone; docs/rng.md).
* **Default Settings**: above detail 3 a score-5 card gets 800 × 600
  (0x5BE9B6); the ATI FSAA clear needs Win9x; the CPU tier reads cpufreq
  `base_frequency` first and gives tier 1 to non-Intel/AMD vendors
  (0x5CDD00; the file-to-TSC match is **inferred**).
* **Garage base view** (0x4C2140(0, 1)): UI_DecalMain's Back from its top
  node (+0x170 at 0x5683C2 → 0x56B8D0) and UI_CustomNeonMain's first tick
  (+0x15B from 0x553547) and Back (0x56AA4B) return the camera
  (`menus::garage_view::screen_base_view_returns`; the garage side's call
  is pending in garage/request.rs).
* **Not changed, with the reason.** Cash_Reward with a kind outside 1–4
  (0x4AA05C → 0x4AA0A4) keeps its old text, shows and zooms: already
  native. Opponents wrap, lap display, stick repeat (0x865134 never
  written), the Detail slider at 0: native in the base (part 1).
  Scene skip: no switch exists yet and the scene keys are in
  input/dispatch.rs; the native rule (A-game-2 6-015) stays open. The
  group-skip byte `[[0x8384C4]+8]` +0 still needs its Wine read.

## Front end on the real-time clock (2026-11-12; **traced** from SPEED2.EXE)

One row per item in docs/native-fixes/FE-3.tsv; tests in
`src/bin/game/tests/fe_real_time_clock.rs`, `game_clock::tests::real_clock_timer_arithmetic`,
`frontend::widgets::help_bar::real_time_clock_tests`, `audio::trax::caption::real_time_clock_tests`.

* **Menu timers on the real-time clock 0x8651A8**: LAN's search, the
  splash's 30 s and "Press Enter", the cursor's 2 s, the EA Trax caption's
  GO / end, the lots' 0.3 s car request; the help bar's cycle on its own
  countdown. Table and addresses: docs/uncapped-fps.md "Menu timers on the
  real-time clock".
* **Specialties and neon purchase questions** (`Game::ice_part_question`).
  UI_ICEMAIN in a list (+0x169) forwards Select to its overlay
  (0x568952 → 0x55DDB0(0x0C407210, +0x160)); UI_IcePartsOverlay's handler
  0x568D40 (vtable 0x79BA1C, ctor 0x56BF80, descriptor 0x7F8928) and the
  neon's UI_PartsList 0x56A310 (UI_NeonPartsOverlay, ctor 0x553320) run one
  block for Select 0x0C407210 and Accept 0x406415E3 on a non-empty list
  (0x568E2F / 0x56A3CB): the fitted part (0x50F1B0) → notice 0xDA15D7E5; a
  career (0x850078 = 1), the part not owned (0x521BF0) and above level 0
  (+5 & 0xE0): the price 0x514280 over the bank 0x861E74 (`jge`) → notice
  0xD1AC60BE; else the question with Yes 0x09F00F07: 0x44B7167F, for the
  trunk layout (slot 0x22) 0xF58C0CD9 when the fitted layout is none or
  stock (0x50F150: 0x610300(slot) null or +5 & 0xE0 = 0), else 0x4FAFB114
  (0x266131CE + 0x294E7F46 for a part above level 0); otherwise the
  question with Yes 0x51123592: 0xF5AC2B2A, for slot 0x22 0x113EAB80 over a
  stock layout, 0x266131CE for a stock part over another, 0x8E4312B0 else.
  Notices by 0x558440, questions by 0x5587A0 (Yes / No 0xF770 / 0x61C, No /
  Back 0xB4623F67, focus 1 = No), all on "GenericDialog_SMALL.fng"
  (0x78FA24; not sized by the text as the body shop's); 0x838970 set and
  the overlay's own DIM (0x560B70 / 0x562950: 0x47FF4E7C of the overlay's
  package). Yes 0x09F00F07: 0x528A00 (buy), 0x55C240 (fit), 0x521290 (the
  install sound), 0x6691C08E to UI_ICEMAIN / UI_CustomNeonMain (the cash
  text); 0x51123592: the same without 0x528A00; then +0x114 / +0x115 make
  the next tick leave the list (0x8ACAA8DF). No / Back / OK: 0x560C50
  (UNDIM), the list stays. After Yes the list closes and the categories
  show again on the category (the overlay's tick 0x5692BD / 0x56A7E5 sends
  0x8ACAA8DF; UI_ICEMAIN 0x568A04 and UI_CustomNeonMain 0x56A8E6 show
  their strips), as in the body shop (0x5665AA → 0x55EF90(0), +0x1D9 = 0)
  and the rims / spinners (0x55FCF0(0): the sizes back to the styles);
  `fit_browser_part`, docs/native-fixes/parts-browser.tsv.
* **Custom gauge picker at step 0** (UI_ChooseCustomHUD): the needle's
  Accept (4 → 0, 0x56455C) sets the package colours (0x537960) UI_ICEMAIN
  0, UI_CustomHUDOverlay 0xFF, the chooser 0, so under the purchase's
  dialogs (`Game::buy_custom_gauge`) only the overlay shows, dimmed; their
  No / OK (0xB4623F67 → 0x5640BD, step 4, 0x153E234) shows the picker
  again (`ChooseCustomHud::hidden`, cleared on the next tick).
* **Garage before the tick's sounds**: GarageMain's update 0x4D82D0 (step
  15 of 0x5811D0, from 0x5663C0) and the camera movers (0x44EFE0, 17)
  before the AudioManager (0x483B40, 18); the hydraulics' sounds from
  0x4D8BB6 → 0x4A6030 in that update. docs/uncapped-fps.md "Front end out
  of the draw".
* **Exit**: WM_CLOSE 0x5CCE6C → 0x864F48, 0x5811D0 returns, 0x5CF970,
  exit(0); nothing saved at quit. docs/exit.md.
