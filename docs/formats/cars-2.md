# Cars, handling, collision, cameras (2)

Part of [docs/formats.md](../formats.md), the traced behaviour of `SPEED2.EXE`; the tags
(**verified**, **inferred**, **open**) are explained there.

## Chase camera (verified except where noted)

* The camera factory 0x5F1FE0 builds type 1 for racing: constructor
  0x451660, vtable 0x787F28, update 0x453570.
* World push (**verified**, 2026-10-28; `chase::keep_out`): for kinds 0,
  1 and 5, after the shake, 0x453570 calls 0x444530: from the car body's
  centre (car +0x50 + rotated body +0x110, the box's middle; see the last
  step below) to the eye it gathers the world polygons around the
  segment (0x5E6FB0: midpoint, half-size (length + 1) / 2 + 0.5, with the
  car's barrier groups copied from car +0x8B4 / +0x8B8), then 0x444190:
  the nearest crossing of the segment with a polygon (0x5D3A00(1, 1, …,
  0.7, 1) → 0x5D3800 / 0x5D3700 / 0x5D35C0, either side) pulls the eye in
  to 0.7 m (0x7870A0) short of it; then up to three passes push the eye
  out of every polygon whose edges hold it at a distance d ∈ [0, r) on
  its normal's side by r − d, averaged (r 0.7, 0x78709C; walls the
  mover's vtable +0x30, 0x445A60: 0.7, or 3 looking back), skipping
  surfaces with the byte at 0x800EED + 0x98·s set (19, 21–28); 0x444390
  then turns the view so the car's centre keeps its screen position.
  0x444530's last step (verified, `chase::keep_out_lifted`):
  with a car, after the pushes, R = |body +0xF0 − body +0x110| + 1
  (0x5EC260 hands back the box min +0xE0 and max +0xF0, 0x5EC250 the
  box's middle +0x110, see below; 0x402CC0 reads +0xF0 − +0xE0 as the box's size,
  so +0xF0 is the maximum corner, not +0xE0 as noted before); an eye
  nearer the centre than R gets z = centre z + √(R² − its horizontal
  distance²), then 0x444390 turns the view. The box (**verified**
  2026-11-01, by 0x5AE530 and live reads of the player's 206 under
  Wine): the hull's vertices relative to the centre of mass (the shape's
  +0x40 = record +0x110, 0x5AEC30); body +0x110 is the middle of their
  bounds (0x5A1A00), **not** the centre of mass, and +0xE0 / +0xF0 their
  minimum / maximum with 0 included (the loop starts its extremes at 0).
  Live: (−2.2268, −0.8421, −0.1058) / (1.6394, 0.8440, 0.9465) / (−0.2937,
  0.0010, 0.4203), the same per car type in every car of the list. So R =
  |+0xF0 − +0x110| + 1 = 3.174 m on the 206 (the clone had |hull max −
  centre of mass| + 1 = 3.073). The sweep's centre (0x4445A5) is car +0x50
  — the car's origin, the centre of mass less the turned record offset
  (live: +0x60 − rows · (0.3, 0, 0.3)) — plus +0x110 turned by the rows
  (+0x90): 0.59 m further back and 0.12 m higher than the clone's centre
  of mass on the 206. Clone: `chase::body_box`, `Drive::push_camera`.
  Then 0x44B040(matrix, car, 1) at 0x453BDD (**verified**): not a
  push but a lift. 0x44AC10 walks the world list 0x890080 for a car (not
  the camera's own; flag 1: traffic only, owner kind [+0x14]+4 = 3; drawn
  +0x550; in the view's mask +0x5B4) whose origin +0x50 is under 12 m
  (0x7870C8) from the eye; outside flow state 6 (0x571260) a car moving
  over 15 m/s (0x7870C4) relative to the camera's car (+0x70) is passed
  over when neither the eye-to-car nor the own-to-car direction dots 0.8
  (0x7870C0) or more with that motion; the eye must be within R = |body
  +0xF0 − +0x110| + 1 of the origin and its z inside the z-range of the
  box's 8 corners (rows +0x90, origin +0x50) ± 0.5. While one is found,
  eye z = √(R² + 0.1 − dx² − dy²) + origin z; then 0x444390 re-aims at the
  camera car's +0x50 (0x7EB5C0 = the car's id, −1 without). Clone:
  `chase::lift_over_traffic`, called by `Drive::chase_view_lifted` after
  `push_camera` with the drawn traffic (free roam's, the race's unpaused;
  flow state 4 / 7 there, so the moving-car test runs) on every world
  update. The eye heights near walls were not re-measured against
  the original after the fix. The shake's noise clock is the world
  clock's tick (0x865188 × 0.00025, `ChaseCamera::set_clock`) and the
  nitrous flame (stage set, `carfx::purging` false) feeds
  `ChaseCamera::set_nitrous_flame`, both per world update.
* Each car type has its own camera table at record +0x720: two counts,
  then six 16-byte entries {i16 kind, u16 tilt (binary angle), f32 x,
  f32 z, f32 y} in car space, looked up by 0x5ECCB0 → 0x441140.
  * A second set at +0x790 is for split screen.
  * Kinds: 0 far, 1 near, 2 bumper, 3 driver, 4 hood, 5 high.
  * The 240SX's near chase is 4.7 m behind and 1.7 m up.
* Player camera setting → kind (0x502500): 0 → 2, 1 → 4, 2 → 1, 4 → 5,
  else 0. Setting 4 (Super Far) is the drift races' (verified): the
  drift set-ups (0x4FF0CC; career 0x532569 for event +0x0F = 5) set
  0x83AA74 = 4, every other set-up (0x4FEFE0, 0x4FF05F, 0x4FF146,
  0x4FF1BF, 0x53257E) turns a 4 into 3 (`Settings::race_camera`).
* Field of view per kind (0x7EB5B4): 13020 (71.5°) for the chase views and
  16384 (90°) for bumper, driver and hood. The eased angle goes to camera
  +0xC4.
* Projection (0x5C7ED0, **verified**): the camera's angle is scaled before
  its tangent is taken. For views 1, 2, 4 and 5:
  * `m00 = 1/tan(fov·1.1/2)` (0x7A27E4);
  * `m11 = −1/tan(fov·(1.1/0.8696)·(h/w)/2)` (0x7A27E8 / 0x7A27E0), where h
    and w are the target record's +0x18 / +0x14.

  Other views use factor 1 and h/w; views 9 and 10 use a fixed 0.6. On a 4:3
  screen the chase view is therefore 78.7° wide and 67.9° tall.
  **Modded captures** (2026-10-31): the Wine copies ran the
  widescreen fix with FixFOV = 1 (all but look112), which patches this
  function (0x5C7F60 / 0x5C7F71: views 1 and 4 take 1.0 for h/w, the
  mirror view 3 0.45, other views ≤ 10 1.0; 0x5C7FFA / 0x5C800C /
  0x5C8031: the y half-angle factor 0.5 → 0.43511 and the m00 / m11
  numerators 1.0 → hor3DScale (1 at 4:3) / 1.22). At 4:3 that equals the
  native m11 at the chase views' 13020 (71.5°) — free-roam edges match
  the vanilla original to the pixel — but not at other angles: the front
  end's turntable car was about 4 % taller in the modded captures (the
  turntable's front rim at y 726 / 765 against the vanilla original's
  716 / 753); hood / bumper (90°) were about 4 % flatter and the narrow
  scene lenses about 4 % taller. The clone matches the vanilla original
  (main menu rim 753 / 753). Near and
  far come from camera +0xBC / +0xC0, with far fixed at 15000 for view 1.
  `Camera::game_fov`.
* Tuning per kind is at 0x7EC610. It points to two 0xB0-byte records,
  which 0x446DC0 blends at the global 0x787154 = 0.5, so their average is
  used. Each record has:
  * ease times for the eye, look-at point, field of view and lean (+0x00);
  * lean, eye and look-at lag, each as scale / min / max vectors (+0x10,
    +0x40, +0x70);
  * ease times for the chase direction (+0xA0).
* Easing (0x5749F0): every frame, each coordinate refits a cubic from its
  value and slope to the target with zero slope, and moves dt/duration
  along it.
* The frame (0x445E00): for the chase kinds (vtable slot 9, 0x445A90) it
  faces the eased chase direction with z up. For the others it is the
  car's orientation. Its origin is the car's position.
* The chase direction (0x445C20): the velocity (mirrored when reversing)
  blended with the car's forward, from 0 at 8 m/s to 1 at 13 m/s (graph
  0x7EC420). Its vertical part fades out over 2 s. Of the mover's stamp +0x58, which 0x44DE90
  renews each update with car +0x5CC set and car +0x424 > 1 (verified, world-systems parity;
  docs/world-systems-parity.md, Cameras), not of the camera's life.
* Lag (0x44DE90): the car-space acceleration, averaged over 4 frames, ×
  scale and clamped. It is added to:
  * the eye;
  * the look-at point, which is (0, 0, x·tan(tilt) + z) for chase views
    and (x + 1, 0, z − tan(tilt)) for the others;
  * the lean, whose z is fixed at 1.

  The 4-frame average (0x446F10, called at 0x453767) reads car +0x80
  (camera +0x2C is the car, 0x5EAC80). The car's state block is car +0x40
  (0x56D393): its +0x20 position, +0x30 velocity and +0x3F0 speed
  (0x5A5550, 0x592170), +0x50 the 4×4 orientation (from the rigid body's
  +0x30), so +0x80 is state +0x40, between the velocity and the matrix.
  Its writer is 0x59B9C0, which the +0x40 scans did not cover: a vector
  helper writes the destination this +0x80 (`thunk_FUN_00441000(this +0x80,
  M, input)`, an affine transform of the 4th argument by M, where M is the
  matrix copied into this +0x30 (2nd argument) with the reciprocals of this
  +0x170 / +0x184 / +0x198 folded in; the stack layout of the diagonal is not
  fully resolved). Callers 0x5A2AD7 and 0x5A48D0 pass input = [X +0xD0] (X =
  [esi +0x30] or [esi +0x58]); 0x5A469D and 0x5ADC63 pass locals. At 0x5A2AD7
  `this` is ECX = [esi +0x20], and the input is the address X +0xD0 (a `lea`,
  the 4th argument; verified from the pushes). The callee's `this` is the car
  body B = [V +0x20] (verified: 0x5A2A70 is entered with ECX = V, and 0x5EBC10
  tail-jumps to it when [Q +0x28] == 3). 0x59B9C0 (verified) stores arg1 to
  B +0x20..0x28, arg3 (X +0x30) to B +0xA0..0xA8 and the input to B +0xC0..0xC8,
  passes arg2 (X +0x50) to a call to 0x4012B0 with B +0x30, and calls 0x43C9C0
  with destination B +0x80, a local at esp +0x80 and the input; the transform
  semantics of 0x43C9C0 and the helpers are not traced. 0x453767 (verified) calls
  0x446F10 with ECX = [camera +0x5C] and pushes [camera +0x2C] +0x80. 0x446F10
  (verified) is a ring filter: 16-byte entries at [ring +8], an index [ring +7]
  that wraps at the cap [ring +5], a fill count [ring +6] capped at that cap,
  running sums at ring +0x10..0x18, and a mean (sum times 1.0 / count, with 1.0
  at 0x784250) at ring +0x20..0x28. The cap is not traced, so "4-frame" is
  inferred. 0x453570 sets [camera +0x2C] = 0x5EAC80(camera +0x20) when camera
  +0x20 is non-null (verified). 0x5EAC80 (verified) returns [P +4] when byte
  P +0x24A is zero; otherwise, if [[0x890118] +0xBD0] is non-null, it returns the
  first dword of the object that pointer names. 0x5F1680 stores zero to [this +4]
  at 0x5F1A3E (verified on the path shown). So [camera +0x2C] is B only if P +4 is
  B for the player camera: **open**. That the input is the acceleration is
  **inferred**; the producer of X +0xD0 is not found. The
  state class does treat it as a vector of its own: the reset 0x59F890
  zeroes +0x30 (velocity) and +0x40 together before setting the matrix,
  and the serializer 0x59F530 writes +0x10, +0x20, +0x30, +0x40 (three
  floats each) and then the matrix +0x50. The vector stores at +0x40 in
  the physics range (0x5958CE, 0x596F38, 0x5980AA, 0x5B35E1, 0x5B513F)
  belong to other objects (0x5B35E1's has sub-objects at +0x224 / +0x228
  with a kind at +0x10, 0x5958CE's a table at +0xC4), not the state.
  Camera +0x2C comes from 0x5EAC80 on camera +0x20, the player object
  (constructor 0x5F1680, listed at 0x8900B4; not re-verified here): its +4, or the car at
  [[0x890118] +0xBD0] while its byte +0x24A is set. A scan of the
  whole executable for three-float stores at +0x40/+0x44/+0x48 of one
  base (any order, within 20 instructions) finds only zeroing
  constructors and resets in 0x580000–0x5C0000 (0x583258, 0x59F8A4,
  0x5A1832, 0x5B35E1, 0x5B7631, 0x5BB212, 0x5BB3ED); 0x59B9C0 is not among
  them. X's +0xD0 and +0xE0 are serialised as vectors by the CarState
  serializer 0x59F530, so X is likely the CarState.
* The chase kinds use the eased lean (x, y, 1) as up; the others use the
  car's up. Bumper and driver snap every frame.
* Shake (0x452ED0 → 0x441340, **verified** except S):
  * A row from the graph at 0x7EC430 is picked by |v| (+0x430) × 4 / S,
    S = [car +0x10] +0x20 (100 when car +0x10 is null). Car +0x10 is not
    0x5B06B0's set-up object: under Wine (circuit_s1_pinned, fields
    `setup_*`, `trap*`, docs/evidence/n-ai/) it points into a static array,
    0x890170 + 0x940 · slot (the racers' records; +0x00 the car type index,
    +0x18 the performance rating 196.70 / 195.92 / 179.79 = 0x5B4DD0's area),
    and its +0x20 read 73.7595 (206, player and rival), 67.0541 (240SX),
    75.1006 (MIATA) = exactly 55, 50 and 56 steps of 1.3410819 m/s (3 mph).
    That is **not** `handling::launch_times`' quarter-mile speed (41.5 / 41.4 /
    40.5), nor the clone's top-speed loop (0x5B5940 as `Handling::
    computed_top_speed`: 55.9 m/s for the 206, capped at the record's
    125 mph; 65.7 m/s uncapped). +0x1C (183.04 / 181.65 / 168.40) and the
    writer of +0x20 were open. **Settled** (**measured** write
    watch + **verified** formula): +0x20 is the info's performance object
    +0x10, written at 0x5B5CB5 in 0x5B5AC0 (the acceleration table's loop:
    v in 3 mph steps from 0 until the top gear passes its shift rpm, at
    most 92 steps), i.e. the gearing-limited top speed; the info is a copy
    (0x413440) of a stack-built one. The clone: `Handling::
    gearing_top_speed` (= `acceleration_curve`'s span after the race's tune,
    `handling::gearing_top_speed`) for S here, the typed replay camera's
    Speed weight and the belt's cap; the data test matches 11 stock cars
    bit for bit (`gearing_top_speed_matches_the_original`).
  * Four noise channels: sideways, up, pitch and yaw. Rates are 0x7EC480 ×
    the row's rate, and amplitudes are 0x7EC490 × the row's amplitude ×
    0x7EBE90[kind] (1.0) × 0.2 outside a drag race (0x89E7D8 / 0x890108).
    A burning nitrous shot (0x59FA40 ≠ 0 and 0x5ECDC0 = 0) adds 0.2 to
    the amplitude and takes the rate × 1.4.
  * The noise clock is the sim clock, (float)[0x865188] × 0.00025
    (`ChaseCamera::set_clock`; the clone's camera age until the game hands
    it).
  * The noise is six octaves of hashed value noise (0x441270, 0x4411B0).
  * The angles are in degrees, scaled to the field of view against 60°
    (0x4412E0).
  * The rough-surface set (0x7EC4A0 rates 3, 5, 7, 5.5; 0x7EC4B0
    amplitudes 0.007, 0.01, 0.6, 0.8): amplitude row[1] × Σ surface row
    +0x3C of the four wheels (car +0x394..+0x3A0; rows 0x800EC0 + 0x98·s)
    × 10 × 0.25 × 0x7EBEA8[kind] (1.0, no drag factor), rate row[3] × Σ
    row +0x38 × 0.25; added to the sway per channel (0x4414BD..0x4414F2).
    Asphalt's +0x3C is 0; dirt and grass 0.05, cobblestone 0.03 at rate 3.
* **Impact shake** (`chase::impact`, `Drive::impact_trigger`; **verified**
  in the disassembly, **observed** under Wine where marked). A hard change
  of the car's velocity jolts the player's view for 0.3–0.7 s.
  * Input: car +0x80 = (+0x70 − last frame's +0x520) / the world step,
    written for every car by Car_UpdateRaceEvents 0x604750 (m/s², world
    axes, once a world frame). World_UpdateControllers (0x609CF0) then
    loops over the human racers (0x60A12B, skipped in game flow state 5)
    and, for a car with a player (car +0x18), calls 0x572E60(player
    +0x10, car +0x80) at 0x60A154. The clone: `CarState::
    world_acceleration` after the player's world update (free roam, the
    race, the countdown's held step and the start scene's), not in the
    scene-driven paths (a new chase mover clears the slot after them).
  * Trigger 0x572E60: skipped when 0x5EA7D0 is true (player mover type 1
    with +0x14 set and the camera setting mapping to view kind 3 through
    0x502500 — no setting does, so never). w = car +0x80 × (0.7, 0.5,
    2.0) (0x7FB750), m = |w|; nothing below 250 (0x79DCDC); f =
    (min(m, 600) − 250) / 350; amplitude 0.08 + 0.08 f m, duration
    0.3 + 0.4 f s (0x79DCE4..0x79DCF0). It starts only when the amplitude
    is above the running shake's present curve value (0 when none runs):
    a bigger hit replaces a running shake, a smaller one only once it has
    decayed. With the player's event kind (+0x58, 0x8900AC[i]) 7 the
    amplitude is × 0.8 and the duration × 1.2. 0x572820 fills the slot
    0x865360 + i × 0x20: −amplitude · unit(w) (0x43CF90), the curve
    0x7FB7A0 (keys at 0x7FB36C: (0, 1), (0.1, 0.7), (0.3, 0.5), (0.5,
    0.4), (1, 0)), time 0, the duration, rate 17 (0x79DC70).
  * Step 0x5728D0, once a main-loop frame (0x5814C2, after the world
    update 0x581417 and the cameras 0x5814A3): time += g_FrameTime
    (0x865150, the world's step, slow motion included) / duration; at or
    past the last key (1.0) the slot's curve is cleared. A test flag
    0x8650C0 (no writer in the code) would start slot 0 with (0, 0, 0.3)
    for 0.6 s. Reset 0x5729F0: the world object's constructor 0x5FF4A0,
    0x5FF8A0 and Player_ChaseCamera 0x5FA830 with its reset flag (or no
    mover before). The clone resets on a placement (`Drive::place`) and
    the start scene's end (0x439070 → 0x5FA830(1)); the other callers of
    0x5FA830 are **not mapped** one by one (a shake lasts ≤ 0.84 s).
  * Offset 0x572870: the slot's vector × curve(time) while trunc(17 ·
    time) (0x75BED4) is even, else 0 — a strobe: at 60 fps a 0.56 s shake
    is on for 2 frames, off for 1–2.
  * Apply 0x572A60(view, matrix): only while the world's time scale
    (0x890080 +0x108, the slow motion's rate) ≥ 1 and for views 1, 2, 3
    (views 1 and 3 read slot 0, view 2 slot 1). Callers: the chase mover
    0x453570 at 0x453B55, right after its look-at (0x5BA9B0: view x =
    unit(forward × up), y = unit(forward × x) — down —, z = forward) and
    before the speed shake 0x452ED0 and the world push 0x444530, for all
    six kinds (chase, bumper, driver, hood; look-back too); the rear-view
    mirror 0x44BA80 at 0x44BE7B (view 3). Not the crash / jump camera,
    the NIS or the replay track cameras. With T = offset · V's rotation
    (the offset in the view's axes): V' = V · translate(T) · RotateX(a) ·
    RotateY(b) (D3DX row vectors, 0x63CED8), a = trunc(−1200 (T.y +
    T.z)), b = trunc(2000 T.x) binary angles (0x7FB740 / 0x7FB744; the z
    turn's factor 0x7FB748 is 0). So the eye moves by −offset (along the
    weighted acceleration) and the view pitches / turns by up to ~1°.
  * **Observed** (Wine, the parity cave at 60 fps; tools/parity
    scenarios crash_wall, impact_wall_bumper, impact_wall_150,
    prop_barrels_120; docs/evidence/impact-shake/): the slot starts on
    the frame of the contact (weighted 481, 456, 1317 and 386 m/s²), the
    view of that frame takes the full vector, the slot's time steps by
    the frame's ticks / duration and ends after 33 / 32 / 41 / 27 frames;
    view 1's eye and forward axis move as the rule gives (eye within
    7 mm of eye − offset, the turn within 0.09° of the rule's) in the near
    chase and the bumper view. A 4 m drop onto the highway at 60 km/h
    (impact_drop) stays below 250 (166 m/s² weighted): a landing shake was
    not observed.
  * The clone (`chase::impact::ImpactShake`, `Drive::impact`): the slot
    on the drive, triggered after the player's world update, the chase
    view applying it on the tick before the speed shake
    (`ChaseCamera::set_impact`), the world frame's end stepping it
    (`Drive::tick_clock`), the mirror's camera shaken by the tick's
    offset; the drawn camera blends the tick's shaken cameras between
    ticks (docs/uncapped-fps.md). The time scale and kind 7 are the
    event cameras' (`Impact::follow`): read after the slow motion's
    advance and again after the event rules, so the trigger sees the
    kind as of the frame before on the frame kind 7 starts (the clone
    runs the event rules after the cars' update; **inferred** harmless:
    kind 7 runs at time scale 0.4 for 8 s, longer than any shake).
* Not modelled: the look-back flip, and the 1.3× pull-out in special states (0x445AC0).

## Player camera movers (2026-10-29; verified unless marked)

* Camera_Factory (0x5F1FE0, player in ECX, args type and "keep the old
  mover" flag) frees or parks (+0x23C, +0x240) the player's mover +0x238
  and builds by type: 0 none; 1 the chase camera (0x451660, kind from the
  camera setting through 0x5EB410); 2 a fixed look-at (0x455B10: eye at
  the car + 4 m up); 3 0x44DBF0; 4..0x0E the event camera 0x4531A0
  (vtable 0x787D40); 0x0F / 0x10 the crash camera 0x44BEA0 (0x470 bytes,
  placed beside the road ahead from the racer's route, 0x41CE90; when
  the road sight test 0x5E7EE0 fails it builds a chase camera instead);
  0x11 0x44ED50 (vtable 0x787FC8); 0x12 the track camera 0x453C00; 0x13
  the target camera 0x4519E0 (vtable 0x787F78); 0x14 the showcase; 0x15
  the dyno camera 0x452600 (vtable 0x788070, field of view 0x32DC). It
  refuses a type equal to the current one (but 0x12), 0x11 in game flow
  state 5, and anything but 0x14 while the showcase is up (0x5012C0).
  Every mover's +0x20 is the player.
* **Look back** (`ChaseCamera::set_look_back`): the Look Back control
  (DIACTION 9, B / K) sends event 0x7E to the player (0x602BE0), held:
  player +0x249 = pressed, then the chase mover's vtable +0x14 (0x445A30)
  stores mover +0x51 (on a change it also sets 0x883E34 = 1 through
  0x5D7AF0, reader **open**). With +0x51 set, the player's +0x2B
  (0x5EB530 returns 1 when it is clear) picks one of two exclusive
  modes (the audio and camera parity 2026-11-05, re-read; the earlier note had both at once):
  * +0x2B set: the view keeps its own entry; 0x453570 multiplies the
    eased eye and look-at point by −1 in x and y (0x4539A5..0x4539BD:
    0x7842A0 = −1, else 0x784250 = 1) before the frame transform, so the
    eye is at once on the mirrored side of the car's origin; walls at
    0.7 m (0x787090);
  * +0x2B clear: the eye entry (0x445EB0) is 0x7EC400's row (−17.2, 0, 6)
    with tilt 0x787080 = 0x1450 (split screen 0x8707DC = 3: (−15.8, 0,
    4.4), 0x0DD4), not mirrored, eased; walls at 3 m (0x787094, 0x445A60).
  The player's +0x2B read 1 under Wine in free roam and a race, and every
  view looked back mirrored in the first frame (docs/world-systems-parity.md,
  Cameras); its writer (**verified**, gap sweep 2026-11-05) is the player's
  constructor 0x5F1680: it zeroes +0x20..+0x4F and sets +0x2B = 1 through
  the block's base (0x5F17A5, `mov byte [edx+0xb], 1`, edx = player +
  0x20; +0x2C = 0x755), and no other store to it was found, so the
  mirrored look-back is the only one the game uses and the 0x7EC400 row is
  dead in practice. `chase::LOOK_BACK_TABLE` = false (no behaviour change).
  Constants checked live in the running original (peek, 2026-10-29).
* **Own car hidden** (`ChaseCamera::hides_car`): 0x453570 sets view
  +0x280 to the car for the non-chase kinds unless the kind is 4 (hood)
  and not looking back; the car list's draw skips that car (0x60D584).
  So bumper and driver never draw the player's car, the hood only while
  looking back. **Inferred**: the car's shadow (queued apart, 0x6162F0)
  still draws.
* Race modes in 0x453570: in drift (0x89E7D9) and URL (0x89E7E2)
  races (or their online twins) the blended tuning's eye lag scale
  (+0x40) is zeroed; the shake's amplitude (0x7EBE90[kind], 1.0 for all)
  is ×0.2 except in a drag race (0x89E7D8). In URL races the
  eye entry is (−10, 0, 5) with tilt 0x82A4D0 (3003 live; the file holds
  1, a static initialiser sets it); a car whose +0x30 object answers its
  vtable +0x54 gets (−5, 0, 1.5) with tilt 0x82A4DC (910 live). The clone
  has no URL race (its flag is only Quick Race type 11), so that override
  is not ported.
* Event cameras (Player_StartEventCamera 0x6022F0(kind, …)): only while
  racing (race status 4), one player, not in a drift race, 5 s since the
  same kind (0.1 s for the keys): 1 → type 0x0D (key event 0x7F), 2 → 0x0E
  on a nearby object (0x455BD0; key 0x80), 3–5 slow motion only
  (0x5EA290(rate, seconds): 1/3 for 8 s, 1/3 for 2 s, 2 for 4 s; key 0x81
  is kind 3), 6 → the slow-motion camera 0x0F (0.4 for 8 s), 7 → 0x10
  (the same mover re-aimed; 0x5F1FE0 case 0x10). Kind 6 comes from
  Car_Crash (at 0x5A0105): an impact of 30 m/s or more (not in a drift
  race, the racer's +0x58 not already 6 / 7); Car_UpdateRaceEvents
  (0x604750) starts 0x0F / 0x10 itself too (0x604CD3 / 0x604FD3, with
  0x5EA290 from 0x7A5500 / 0x7A552C; its trigger is the in-air test of
  the earlier notes, not re-traced here). Both need player +0x24 ≠ 0. The events 0x7F–0x81
  have no row in the Controls menu (no default key, **inferred** from the
  DIACTION table's 18 actions).
* **Crash cameras and slow motion** (`src/event_camera.rs`,
  verified unless marked). Player +0x24 is the Jump Camera option
  (0x83AA20 = settings +0x1C, copied by the director's constructor at
  0x5F180B); kinds 1..5 need 0x83AA24 (settings +0x20, 127.5) instead. Each kind
  also needs 0x8020E8[any player's +0x58] clear (kinds 3..7 block) and
  ends a running kind 1 / 2 first (0x8020E0). Slow motion: 0x5EA290 sets
  the world's +0x108 rate, +0x10C seconds, +0x110 callback (0x6022E0 →
  0x5FFE80(1)); 0x5EA370 counts +0x10C down by the real frame time,
  then resets the rate to 1 and runs the callback, and returns frame
  time × rate — the world's step. 0x5FFE80 (end): kinds 1, 2, 6, 7
  restore the saved mover, every kind resets the slow motion, +0x5C =
  kind, +0x58 = 0. Triggers: Car_Crash (0x5A0095..0x5A0105) for a car it
  just put in crash mode — no drift race, car against car (0x592C70), the
  push ≥ 30 (0x7A186C), a human racer (0x431DD0), its director's kind
  not 6 / 7, and 0x5F1EF0: from the route node nearest the car
  (AIRoute_NearestNode on the car's route: car +0x2C's vt+0x24 = 0x418560,
  `lea eax,[ecx+0x68]` — the controller's own maintained node list, count
  +4, nodes +8, node +0x54 the distance; **verified**; the
  clone builds a fresh list from the car's node, which matched to 1 cm in
  3 of 4 captures — **inferred** equivalent, a drifted list is untraced), the
  first node ≥ 75 m further on (node +0x54) more than four nodes on
  (+0x644 / +0x648 = the two, else −1) — starts kind 6 (key off; refused:
  0x5EA650). Car_UpdateRaceEvents (0x604750), each update: kind 6 / 7 end
  in crash mode (+0x38 = 1) once +0x424 = 4 more than 0.5 s after the
  crash's start +0x548 (set by 0x5F6BF0 for a human car), out of crash
  mode with any wheel down (0x604A37); every eighth update ((counter + 3)
  & 7) kind 6 in crash mode more than 3 s after the crash ends and starts
  kind 7 (5 s since +0x84, the same gates; 0.4 for 8 s, mover 0x10). The
  same update's jump trigger of kind 6 (0x604ACC..0x604D14: the option
  value through four graphs 0x89CF18, 0x890150, 0x89E790, 0x89E824 and
  the flight prediction 0x5EC3E0; landing time in +0x544) is the "Jump
  Camera"; **not ported**, nor kinds 1..5. The clone: `Race::events`,
  the race frame stepped by the slow motion's world step, the crash test
  on `CarState::crash_hit`, the race road's points / distances as the
  route nodes (**inferred**), one world update a frame (**inferred**).
  **The crash / jump camera mover** (`event_camera::CrashShot`,
  `plan_shot`, `game.rs crash_camera`; verified in the disassembly unless
  marked). Camera_Factory case 0x0F fills the director's 0xA4-byte record
  (+0x268): +0x0C / +0x28 the car's progress +0x8F8 (in a race,
  0x88F2BE; else 0), +0x10 / +0x2C the road locator's +0x1C at the
  camera place (0x5DEDA0; else 1.0), +0x20 = 1, +0x30 0.5, +0x34 10000,
  +0x3C → 32 field-of-view points (20, 0x8020B8), +0x50 = 2 eye points
  (+0x54 → director +0x270 / +0x280, both the place) and 2 roll points
  (+0x58 → +0x2A0 / +0x2B0, zero), +0x84 0.2, +0x88 ±0.4, the rest 0.
  The place: node n = 0x5F1EF0's far node (−2 when that is ≥ 5 nodes on),
  the points 2 m inside its right (vt+0x10 − 2) and left (vt+0xC + 2)
  edges on the car's lane (0x4122C0); the one less far along the car's
  velocity (right: +0x88 = −0.4, left: +0.4), the second shot's place
  (+0x290) on the other side two nodes on from the car's; 0x41CE90 then
  needs the line car → place to cross every node's edge line from the
  car's to n − 1 (**inferred**: node +0x18 / +0x2C as the edges), else the
  sides swap. Both 1 m up, the first at least 1 m above the ground
  (0x5E72F0). The sight test 0x5E7EE0 from the car (+0x60, or its
  attached object's) to the place: blocked builds a chase camera
  (0x451660) instead. Case 0x10 (kind 7): the progress from now, the eye
  the second place raised to at least the car's height (while kind 6 runs
  and the car falls, 0x604D2F raises it every eighth update) and 1 m over
  the ground, +0x88 negated, the field of view 20 − 0.25·i (0x8020BC), the
  same sight test. The mover (0x44BEA0, 0x470 bytes) copies the record to
  +0x68 and sets the view's +0x280 / +0x284 = 0 (the car is drawn), a
  random +0x43C = ±(26 + rand 19) eased to 0 over 0.42 s (it only feeds
  +0xB4, which +0x46C = 0 never writes). Its update 0x44C350: the eye from
  the eye points at (progress − +0x0C) / (+0x10 − +0x0C) (both the
  place: it stands still); the field of view from the 32 points at that
  fraction × 31 (to a binary angle × 182.04, view +0xC4); dist = |eye −
  car +0x60| (≥ 1); the look-at point is the car +0x60 raised by
  tan(half view) × 0.2 × dist and moved along (eye − car)/dist × up by
  tan(half view) × +0x88 × dist (x, y only); the record's offset (+0x78,
  through car +0xD0, zero; **inferred**: the matrix's translation row
  +0x100 zero) and spring (0x444CF0, +0x60 / +0x64 zero) add nothing;
  the up vector from a roll of 0 (0x4440D0) is +z; Matrix_LookAt; view
  +0xB0 = dist, +0xBC = 0.5. The mover's vtable slot 9 (0x570A70)
  answers 1, so the rear-view mirror is off under it.
  **The jump trigger** (Car_UpdateRaceEvents every eighth update,
  0x604ACC..0x604D14): no kind 6 / 7 running, all wheels up (+0x424 = 0),
  director +0x248 clear (**inferred** always), speed +0x430 above graph
  0x89CF18 of the option value x = trunc(+0x24) & 0xFF; the flight
  prediction **0x5EC3E0**(graph 0x890150(x), &h, &t, stop 1): from the
  mean of the four wheels' contact points (wheel matrices +0x130 + 0x40·i
  at −radius, through +0x90 / +0x60; 0x5921C0 builds them from the wheel's
  place with z + body +0x340 + 4·i, its suspension offset: **observed**,
  `CarState::flight_start`), n = trunc(speed / 3 × window) steps
  (n < 1: nothing), dt = window / n; starting above the ground the last
  touch is −dt and t = dt with no fall, on it the fall is 9.8 + (record
  +0x374 + +0x378) × 0.3 × speed / mass; each step at p0 + (v + a·τ/2)·τ
  (τ = k·dt − last touch): at or under the ground (0x5E72F0) it stops
  once it has flown, else re-bases on the ground with v along it at the
  speed; above, h = max height, t = max τ, the fall 9.8. The landing is
  due (+0x544) at now + t × 0.5 when t ≥ 1, else now + 5 s. Then h >
  graph 0x89E790, t > graph 0x89E824, 0x5F1EF0 and the event gates start
  kind 6 (0.4 for 8 s, mover 0x0F). The graphs (static initialisers
  0x77ECC0..0x77ED2C, points 0x802404..): speed 40 → 20, window 3 → 0.6,
  height 2 → 0.5, airtime 1.5 → 0.3 over x 0..255 (option on: +0x24 = 127.5, x = 127 →
  30.0 m/s, 1.8 s, 1.25 m, 0.9 s). Under kind 6 out of crash mode, past
  the landing time (0x604E72): the event ends, the prediction runs again
  over 1 s and above 0.5 m kind 7 starts. Any wheel down ends a jump's
  kind (0x604A33). 0x4022C0 after the start is an empty function.
  **Observed** under Wine (Quick Race, the 206, option written
  255): lifting the car 6 m at 24.7 m/s started kind 6 at once (the
  place +0x270 set 60 m ahead), the mover stayed the chase camera there
  (sight blocked) and the kind ended when the wheels touched 1.9 s
  later. The mover's own pose with a clear sight line: **seen** in <work dir> (below).
  **Sound**: the track collision sample's volume byte +60 (≤ 127) while
  the car's director runs kind 6 (0x47A770) and the bottom-out knock
  (0x47EB77) read the kind; neither is changed here (see Sound rounds).
  The HUD: HUD_BuildFeatures 0x6000A0 reads +0x58 and gives no features
  for kinds 1, 6, 7 (**ported** <work dir>, `hud::Input::event_camera`). Replays: the clone's playback does not run the events and
  keeps its replay cameras (**inferred**: the original's replay director
  0x5FB060 owns the camera; kinds 1, 2 stamp +0x68 outside replays only).
  **Inferred** deviations: the human car's route (+0x2C) as a fresh racer
  route from the car (`RacerRoute::start`); the wheels' contact points
  from their static places. The chase camera's park and return: see
  **Chase camera parked under the event** below. The event camera
  0x4531A0 (kinds 1, 2) is not ported.
* **Crash / jump camera, 2026-11-04** (**verified** in the
  disassembly): the second shot's rise at 0x604D2F (car +0x78 below 0 and
  the place's z +0x298 below car +0x68 → +0x298 = +0x68) sits inside the
  every-eighth-update block of 0x604750 (the gate 0x604A40..0x604A56
  jumps to 0x6051E5 otherwise) and runs after the jump trigger, before the
  kind 6 → 7 change (0x604D61), so the second place is raised on those
  updates only and before case 0x10 reads it. The clone raised it every
  update after the change (`crash_camera`, `EventCameras::eighth`). With
  the fixed 1/60 s cadence the clone's world updates are the original's at
  60 fps, so the eighth-update counter is native now (was **inferred**).
  The drawn frames between two updates blend the camera; a change of
  camera mover (the crash camera starting, its second place, its end) is
  a cut and is no longer blended (the second place can stand under the
  30 m blend limit from the chase camera). Kinds 1..5: 0x83AA24 (settings
  +0x20) is 127.5 (0x5116E0 at 0x51170E through the settings block's
  base; the save; read live under Wine), so the gate passes, but no
  PC trigger exists (kind 1's timer +0x9C only zeroed, the keys
  0x7F..0x81 in no descriptor row; `p17-scene-follow-ups.md` §2).
  0x604A97..0x604AC7 (**verified**): fast (above graph 0x89CF18),
  fewer than four wheels down and |0x59FA20| (car +0x40's +0x3EC test, else
  0x591DA0) above 0.098 (0x7A650C) → 0x578EA0(2.0 (0x7A55D0), "CarInAir"
  0x7A6500) on the replay manager 0x8650B4: +2.0 on the score +0x1D8 of
  every held INSTANT_REPLAY (names compared with 0x79545C). The score only
  picks HIGHLIGHT_SNAPSHOTs, which the PC game never keeps (+0xB4 = 0, see
  Replays), so nothing to port.
* **Crash camera in free roam** (**verified** in the code, **observed**
  under Wine). Free roam runs with race status [0x890118] = 4 (read live,
  career free roam), one player (0x8900BC = 1), no drift flags, the
  director's +0x24 = 127.5: every gate of Player_StartEventCamera 0x6022F0
  passes, and Car_Crash 0x59FB80 (from the contact record 0x5A9630, its
  only caller 0x5AF78A) and Car_UpdateRaceEvents 0x604750 (the world
  update's car loop at 0x60A086, free roam included) run there. Observed
  (career free roam after the airport, Jump Camera on): a head-on hit on a
  traffic car (closing ~50 m/s) put the player in crash mode (+0x38 = 1)
  and started kind 6 in the same frame: slow motion 0.4 for 8 s, mover 7
  (0x0F), field of view 20°, the eye still 68 m ahead beside the road, the
  whole HUD gone; it ended once the car was down on four wheels 0.5 s of
  simulation (1.24 s real) after the hit, the chase camera back at once.
  Not started (observed): a wall hit at 33.5 m/s (no crash mode: the wall
  contact never reaches 0x59FB80), a low-speed side contact with a traffic
  car (no crash mode), head-on hits whose push stayed under 30 (no crash
  mode). The place: car +0x60 (1652.168, −667.480, 38.230), forward
  (−0.2077, −0.9782), velocity (1.314, 6.171, −1.072) gave +0x270 =
  (1644.49, −735.48, 37.28) and +0x290 = (1628.81, −686.52) — on the
  side the car's velocity *before* the hit picks ((−3.667, −17.263) at
  the frame's start), not the one after it (thrown back at (1.314,
  6.171)): Car_Crash runs inside the contact before its push and the
  factory's dot (0x5F2710..0x5F275E) reads car +0x70. The clone now
  places a Car_Crash shot with the velocity from before the update
  (`event_shot`; within 1 cm of both places; the update's start for the
  push's instant is **inferred**). The clone ran
  no event camera in free roam; now `SimSession::roam_camera` (a director
  per free-roam start, **inferred**) with the race's rules
  (`Session::roam_events`): the slow motion steps free roam's world
  (encounters, traffic, the car), the placement walks the human car's list
  on the free-roam routes, the record's progress 0 and end 1.0
  (**inferred**: 0x88F2BE clear outside a race), the HUD, the mirror, the
  engine's camera-distance filter, the collision sample's +60 and the reset
  key read free roam's kind. The jump camera follows from the same code
  path (not observed in free roam).
* **Crash camera under Wine** (fastgpu, Quick Race 4014 forward,
  the 206, Jump Camera on: +0x24 = 127.5). The player's rigid body
  (vtable 0x7A20CC: +0x20 position, +0x30 axes, +0x70 / +0xA0 momentum and
  velocity) was put head-on in a rival's path (`crash.sh`, `tele.py` in
  <work dir>), with a 70 Hz memory log of the director, the car,
  the view 0x8760E0 (+0x40 eye, +0x50 forward, +0x60 look-at, +0xC4 field
  of view) and the mover, and 30 fps frames. Strips:
  <work dir> (clear crash shot),
  `b3_strip.jpg` (sight blocked, then kind 7), `j2_strip.jpg` (jump
  camera, clear, eye under an overpass). Seen:
  * Kind 6 started on the frame of the hit (push ≥ 30, rival against the
    player), the slow motion 0.4 for 8 s of real time; the eye jumped to
    the place (+0x270) and stood still, 67–76 m away, field of view 20°,
    the view's +0xB0 = the distance; the look-at point matched 0x44C350's
    (tan(10°) × 0.2 × dist up, tan(10°) × 0.4 × dist sideways) to 1 cm.
    No flash, blur, letterbox or zoom: a still, long-lens shot.
  * **The whole HUD is hidden** while kind 6 / 7 runs, also when the
    sight test failed and the chase camera stayed (b3). Traced:
    HUD_BuildFeatures 0x6000A0 returns 0 for kinds 1, 6 and 7
    (0x6000E8..0x6000F4) after the mover-type cases 2 / 3 → 1 and 4 / 0x12
    → 0. (The +0x225 flag and 0x4022C0 around it are an empty stub.)
  * Kind 6 ended once all four wheels were down more than 0.5 s of
    simulation after the hit (a3: 0.98 s sim, 2.2 s real); the chase camera
    was back at once. Held in the air in crash mode (b3, z written each
    10 ms), kind 6 ran 3.1 s of simulation (7.8 s real), then kind 7 (0.4
    for 8 s again) took the second place (+0x290, 18 m from the car,
    raised to the car's height), and ended on four wheels.
  * A second hit 5 s (simulation) after the first started no camera.
  * Camera mover +0xC read 7 for the crash mover, 1 for the chase camera.
  **Placement** (the car's state at kind 6's start fed to the clone,
  `NFSU2_CRASH_AT`, `cmp.sh`): the place and the second place agree to
  1 cm in three captures of four (a3, b3, j2), the fourth (b1) one node
  off. What it took:
  * The route is the human car's list (car +0x2C, vt+0x24 = 0x418560 →
    +0x68), which **follows the car's heading**: turned from −x to +x on
    the start straight its list ran 655/5, 655/6, 476/2 … instead of 655/5,
    655/4, 655/3 (read live). The clone had a racer route in the race
    records' way. `RacerRoute::start_human` (mask 0x0800_0000, the way
    by the tangent; **inferred**: a fresh list from the car's node).
  * The list holds 0x418DA0's 8 m points in **every** gap of 16 m or more
    (node +0x74 = 1, s = the last + 8k, the last point's record and cp):
    0, 8*, 20.6, 28.6*, 38.7, 53.3, 61.3*, 70.3, 84.4 … at the grid. The
    clone filled gaps across junctions only (0x422220: no junction or a
    non-turn one goes to 0x4226E1 → 0x418C50 → 0x418DA0); that changes the
    AI racers' lists too (native; the smoothing pass bends those points).
  * The car's node is 0x4198C0 with flag 1: the nearest node, then on while
    it lies behind the car — the first node at or ahead of it (the clone
    took the node behind: every place 8 m short).
  * Until <work dir> the trigger's 0x5F1EF0 test read the race road's points
    (30–55 m apart), so on Quick Race straights it never placed; it now
    reads the same list.
  **Sight test**: the factory's own cache (0x5E0B50 at 0x5F2945) first
  answers the height at the place (0x5F2961, 0x5E7E80 → 0x5E72F0), so its
  pack is the place's, and it has **no barrier groups of its own** (+0x2F0
  = 0): the race's barrier walls (face group byte ≠ 0, on only for the
  racers' own gathers) are not seen. Pieces from the car to the place
  share the cache (0x5E68E0 keeps the pack within its margin). With the
  car's barriers the clone was blocked at b1 / j2 by such walls at the
  place (flags 0x12) where the original's camera ran; now all four
  agree (a3, b1, j2 clear; b3 blocked by plain walls, flags 0x16).
  **Chase camera parked under the event** (2026-11-06; **verified**
  in the disassembly, **observed** in the <work dir> captures re-decoded,
  `docs/evidence/crash-camera-park/`). Every start of kind 6 / 7 calls the
  factory with the keep flag (0x6022F0 case 5 / 6, 0x604CD3, 0x604FD3:
  `push 1`, unless game state 5): the current mover goes to player +0x23C
  (the one there to +0x240) and vtable +0x40 (0x4440A0 for the chase
  camera, vtable 0x787F28) detaches it from the view; nothing updates it.
  The new mover is the crash mover, or with the sight blocked a *fresh*
  chase camera (0x451660). The end 0x5FFE80 (cases 1, 2, 6, 7) →
  0x5FFDE0: the current mover freed, +0x23C current again (+0x240 back to
  +0x23C), and for a type-1 mover vtable +0x20 = 0x445B10 (kind, sets
  +0x52), +0x14 (the look-back, player +0x249) and +0x3C = 0x445A50
  (re-attach, 0x444040: view +0x280 / +0x284 restored, +0xBC = 0.5). The
  first update with +0x52 (0x453570 → 0x44DE90(…, +0x52)) snaps the eye
  +0x38, the look-at point +0x3C, the field of view +0x34 and the lean
  +0x44 to their targets, and through 0x445C20's second argument the
  chase direction +0x40 too; 0x5D7AF0 runs (0x883E34 = 1, reader open).
  Kind 7 starts only after 0x604D9C ended kind 6 in the same update, so
  the same chase camera is parked again (0x8020E8[6] = 1 would refuse it
  otherwise; 0x8020E0[6] = 0). Under Wine: a3 / b1 (clear shots) parked
  0x0EBB1188 in +0x23C for the event and had it back in +0x238 at the end,
  view 1 facing the car's heading on that first frame (−173.1° vs −173.0°,
  152.4° vs 152.5°; before the crash it lagged 2.7° / 7.8°) though the car
  had turned 16° / 5° meanwhile; b3 (blocked): the fresh chase mover
  0x04915AB0 for kind 6, the crash mover for kind 7, park 0x04915730
  throughout, back at the end facing the car (−4.9° / −4.9°) after a 140°
  turn. Clone: `Race::parked_chase` (`crash_camera`: parked at each start,
  `Drive::chase` a fresh `ChaseCamera` for a blocked shot, updated in the
  start frame; at the end `ChaseCamera::resume` (every ease snapped as a
  new camera's, the ground stamp aged by the parked time) and the
  restored camera updated in that frame); a blocked shot's fresh chase
  counts as its own mover (`Game::camera_mover` 0x1_0000 + kind), so its
  start and end are cuts. Before, the clone's chase camera kept easing
  under the event and came back with its lag. **Inferred**: the
  acceleration history restarts after the park (the clone differentiates
  the velocity; the native filter 0x446F10 is fed car +0x80). Test
  `chase::tests::a_parked_chase_camera_comes_back_snapped`.
  **Not compared**: the post-crash physics (the clone's car came down on
  four wheels sooner in the head-on harness, so its kind 6 ran 0.5 s
  of simulation where a3's ran 0.98 s; physics, not camera).
* The dyno camera 0x15: Player_UpdateCamera (0x600600) builds it in game
  state 6 when g_IsDynoRun is set and 0x510050 holds; its constructor
  places a fixed matrix from the car's +0xD0 and minus +0x50 with ease
  times 7.2 (0x40E66666) and look-at 5.0. Ported (`dyno_camera`, the
  chart drawn over the 3D car on the rollers); **observed** through a
  whole Dyno Run under Wine: mover vtable 0x788070 throughout, the
  eye still, looking at the car's origin +0x50 (`p17-scene-follow-ups.md`).

## Main-menu car and car textures (2026-10-28)

* **Main-menu car** (verified: 0x4DF1D7..0x4DF285, 0x515D00, 0x521110,
  0x500BD0, 0x541550): UI_Main's constructor builds the car it copies to
  0x8371B0 from the DDAY_PLAYER_CAR preset (0x637280) while 0x86347D
  (career +0x873D) is clear, else through 0x515D00(0) on 0x83A9D0: out of a
  career (UI_Main has set game mode 2) the car named by player 0's entry
  0x83AAB8 in the car list (0x503510, 0x5B entries at +0x9BD8, matched by
  +8). 0x86347D is set when a career car slot becomes current (0x541550,
  0x500BD0: +0x8740 = "CAREER_SLOT_%d"). The only writer of 0x83AAB8 seen,
  0x521110, stores the name of the car just customized when its session
  has no unsaved change (0x50E900 compares the 0xAA part slots of the
  list's car and the session's). 0x55BE00 (the front end's Back) calls it
  only when the screen it returns to is UI_OLCarSelect or UI_QRCarSelect
  (strings 0x794DF0, 0x78F354), so main-menu Customize sets it and the
  career's shops (back to UI_CareerCrib, 0x78F5A4) never do; 0x515D70
  (game mode 4 only) also writes it. The entry is in the block saved with
  the profile (0x5344E0 copies 0x83AAB4…), zero in a new one (0x52A230
  does not set it). In a career (0x850078 = 1) 0x515D00 uses the current
  slot's name 0x863480 instead. The clone keeps `Profile::menu_car`
  (saved) and shows it; with the flag set and no name, 0x503510 finds no
  car and the FE car keeps type −1 (0x610280(-1, …)) — what then shows is
  **open**, and the clone shows the career car.
* **Car textures**: GLOBALB's texture pack stays resident, so a texture none
  of a car's own packs holds is taken from it; the brakes' BRAKE_STYLE00
  (0xC6578694, GLOBALB TPK 0xB3300000) is one (the clone did this only for
  the shared parts before).
* **Car cull and depth by blend** (verified, 0x5BCD29..0x5BCD89): for the
  car effect (0x870910 +4 = 5) a draw whose texture is opaque (+0x56 = 0)
  sets CULLMODE 1 (NONE) and ZWRITEENABLE 1; a blended one CULLMODE 2 (CW)
  and ZWRITEENABLE 0 (car.efx's passes assign neither). CW is taken as
  the clone's back face: culling it keeps the body's outer skin, culling
  the other side shows the inside.
* **Minimap turn** (verified, 0x4CA840): with the map mode 2 (race state
  ≠ 0: race mode 0x83AA1B, else explore 0x83AA1C) a = 0x43E6D0(dir y,
  dir x) = 90° − heading, else 0. The map and the next object get −a, the
  north indicator 0x43E6D0(0, 1) − a + 270 ≡ −a (0x51CEE0; FEng angles
  turn clockwise), each marker its own direction's angle − a (so the
  player's arrow points up), and each marker's map offset (fx, fy) goes to
  (cos a·fx + sin a·fy, cos a·fy − sin a·fx) before the 0.55 pull-in. The
  minimap object (0x4EC950, built by 0x4ECC80 into 0x8364D4) holds +8
  0x90FE80F3 Track2000_map, +0xC 0x40CDC515 Minimap_Mask and +0x10
  0xF2A54430 Minimap_North_indicator (**verified**, 0x4EC9B2..0x4EC9DF): the
  frame ring TrackMapTargetRing (a group of four Minimap_Border quarters,
  each 128 × 128 at ±16 and turned 0/90/180/270°) does **not** turn. The
  clone turned the ring, so the quarters' seams (bilinear filtering of
  the arc's last texel against the transparent black beyond it draws a
  thin dark line where two quarters meet) swept round the ring as the car
  turned: the "weird dark lines". Now they stay at the four compass
  points, as in the game. Minimap_Mask (alpha 0.8 inside its circle,
  additive with black colour) turns instead, which shows nothing.
* **Minimap items** (**verified**, 0x4EC950, 0x4EC790, 0x4EC820, 0x4AC2B0,
  0x4CA6D0, 0x4DB900): each item is {type, id, position, angle, flags};
  its FE object comes from the pool 0x836560, filled by 0x4DB900 from the
  table 0x7F6828 ({format, type}: 3 AIRACERINDICATOR_%d, 1
  STARTLINEINDICATOR_%d, 6 CRIB, 4 PLAYERCARINDICATOR_%d, 7 PAINTSHOP,
  8 PARTSSHOP, 9 ICESHOP, 10 PERFORMANCESHOP, 11 CARLOT, 5 SHOWCASE, 12
  CIRCUITEVENT, 13 DRAGEVENT, 14 STREETEVENT, 15 DRIFTEVENT, 16
  SPRINTEVENT, 17 URLEVENT, 18 SUVEVENT, objects n = 0..7). The
  constructor adds type 4 for players 0 and 1, types 1/2 (flag 1) for the
  race's start and finish (0x883DAC, 0x89E7B4), a type-3 item per racer
  from 0x890080 +0x14 to +0x18 (the AI racers), with 0x89CF48's 0x5F0100
  the other humans, and in free roam (0x89E7B0 = 0) the route-trigger
  table (0x500EE0 → 0x4EC8C0). 0x4CA6D0 decides which show through the
  byte table 0x4CA7E4 (types 0–2 → never, 3 → the AI test, 4 → always,
  5–18 → the marker test): so the start/finish items never show (the
  clone showed a STARTLINEINDICATOR at the start line). An AI item (case
  0x4CA6F9) shows while its car (0x5F0E40) has +0x550 set and is not done
  (+0x998, 0x5EC070), and in a career only while no world challenge runs
  (0x4CA72E: 0x501690 on 0x85B940, its +8 = career +0xC08; not the D-Day
  intro, as noted before: the clone now shows them during the intro).
  +0x550 is the car's active flag (0x5F6D60 sets it as a car enters the
  world, 0x5F6E30 clears it), so active free-roam encounter racers show
  too (the clone: active encounter cars, not done, outside the intro). Its position is the car's +0x50/+0x54
  and its arrow turns like the player's (case 3 of 0x4AC2B0: the
  direction of +0x40C), less a. The clone: a race's rivals until they
  finish, and an outrun's encounter cars (0x607190 makes every AI car a
  racer and marks all but the challenged one done) while active and not
  done. **Outrun racers** (2026-10-31, race5): no rebuild is needed for
  them. The items name cars by id (car +0x0A, 0x4EC790(3, id)), and the
  career free-roam set-up 0x540790 builds its three encounter racers
  (0x53F010(3) → 0x601610, parked) into the world list's AI range (+0x14
  … +0x18) that 0x4EC950 walks, and the outrun manager's slots take those
  same cars (0x412F90); so they are listed whenever the HUD is built in
  free roam, and 0x4CA6D0 shows one only while it is active (+0x550:
  0x5F6D60 brings a parked one back, 0x5F6E30 parks it) and not done —
  the clone's rule (active, not done; `--shot outrun`). **Inferred**: that
  the free-roam HUD is built after 0x540790 (the order of the two in the
  load is not traced); not captured under Wine. +0x550 is the car's
  active flag (Car_SetActive 0x5ED5D0; World_DrawCars 0x6317A2 skips an
  inactive car).
* **Mirrored FE images** (2026-10-31): the panels' rounded corners
  are one sprite turned by their quaternions — top left 180° about z, top
  right 180° about x (a vertical mirror), bottom left 180° about y. The
  game builds the four corners through the matrix (0x51A1C0), so a mirror
  is exact. The clone's `frontend::push_box` reduces each image to a
  rectangle and an angle; the x-mirror's matrix carried ~1e-8 off the axes,
  which sent it down the turned path (angle −π, UVs swapped) and drew the
  corner a pixel off with the texture wrapped: MC_Main's and the LAN
  screen's "notch" (gaps row 6). Float noise up to 1e-5 of the length now
  counts as unturned (**verified** against gaps120 b1 / <work dir> lan_6).
* **Front-end quad submission** (**verified**; `render::ui`): every FE
  polygon (images 0x536A60, strings, movies, coloured images) is built by
  0x51A1C0 (corners −0.5…0.5 through the object's matrix, vertex 0 upper
  left, 1 upper right, 2 lower right, 3 lower left) and drawn by 0x51A5D0
  → 0x5C7EB0 → 0x5C5030 with a mode of 0 or 1 (never 0x61). 0x5C5030
  writes x / 320 − 1, 1 − y / 240 (0x79AC18, 0x79AC1C) as the vertex
  position, and, with a texture and any mode but 0x61, insets the UVs by
  half a texel: du = 0.5 / entry +0x44 (width), dv = 0.5 / +0x46
  (height); vertex 0 (+du, +dv), 1 (−du, +dv), 2 (−du, −dv), 3 (+du,
  −dv). fe.efx's vertex shader (vs_1_1: oPos = v0 · WorldViewProj, oT0 =
  uv + TextureOffset, the latter set only for scrolling textures by
  0x5BCC40) runs with WorldViewProj = identity: the FE frame (0x5CC03B…
  0x5CC09D) sets view 0's matrices to identity and 0x5BDDF0(identity, 0)
  uploads their product. So there is no half-pixel correction, and under
  Direct3D 9's pixel-centre rule (pixel i's centre on the window
  coordinate i, wgpu's on i + 0.5) the front end lands half a target pixel
  down and right of where the same clip coordinates fall in wgpu; the
  clone shifts its UI vertices by that half pixel. **Corrected**
  (2026-10-31): the clone used to shift the other way (up and
  left), which kept the texel phase but put every FE edge one pixel up
  and left of the original's (main-menu edges at x 941 / 1228 and y 126 /
  142 against 942 / 1229 and 127 / 143; the scene letterbox bars on rows
  0–118 and 839–958 without the last column). With the shift down and
  right they land on the original's pixels (bars rows 0–119 and 840–959,
  all 1280 columns; the HUD edges in free roam to the pixel). Its pixel shader is tex ×
  diffuse with rgb doubled (`add r0.xyz, r0, r0`). Sampler: fe.efx's
  techniques set D3DX states 165 / 166 (AddressU / V) = 1 WRAP and 169 /
  170 / 171 (Mag / Min / Mip) = 2 LINEAR (164 is its DiffuseMap); the
  clone now repeats and filters linearly between mips. Without the inset
  and the shift the clone drew light specks around glyphs ("MPH") and
  the speed digits' backing (bilinear taps on the texels beyond each
  glyph cell) and 2-pixel seams on the minimap ring; with them the specks
  are gone and each ring seam is 1 pixel. That remaining seam is the art
  (MINIMAP_BORDER 128 × 128: the arc's last opaque texel is column / row
  79, the next transparent, and the four quarters at ±16 around the
  ring's centre butt against each other there), so bilinear filtering
  leaves it in the game as well. The minimap's window (0x4CA912..
  0x4CA9AA) is not clamped to 0..1, so near the map's edge it wraps, as
  under the game's WRAP. Blend and alpha test (**verified**, 2026-10-31):
  0x5BCC40 fills the BLENDSTATE array 0x86EAD8 = {alpha test (entry byte
  0x55 == 1), ref 95, blending (byte 0x56 ≠ 0), src, dst} (0x5BCB80,
  0x5BCBD0) and uploads it to fe.efx, whose pass binds the five entries to
  D3DX states 4 / 10 / 13 / 6 / 7 (AlphaTestEnable, AlphaRef,
  AlphaBlendEnable, SrcBlend, DestBlend; pass at blob offset
  0xB8C..0xBCC). The effect sets no ALPHAFUNC: the device's GREATER (5,
  0x5C2480) holds. So byte 0 draws unblended and a tested texture keeps
  only alpha > 95. The clone does both (`render::ui`, pipeline 5 and the
  shader's discard). The map's opacity: destination alpha from the mask's 0.8
  alpha through additive blending (0.8 × 0.8 + 0, the alpha on the colour
  factors), so the world shows through about a third, as in the Wine
  capture; the clone draws it so (see the frame's alpha-only quad).
* **Front-end drawing against the original** (2026-10-31, `<work dir>`):
  pixel diffs of 19 static screens at 1280 × 960 against the original
  under Wine (wined3d on Xvfb, vanilla, WidescreenFix off; images in
  `$OB_PRIVATE/wine/compare/fedraw`: orig / before / after / heat).
  Causes found and fixed:
  * Mirrored and turned pieces (a 9-slice box's corners, e.g. the
    dialogs' U2_MENU_CORNER_WHITE at the upper right, size −x and 180°):
    0x51A1C0 builds the corners from the local −0.5…0.5 box in vertex
    order with (u0, v0) on the local upper left, and 0x5C5030 insets the
    UVs by half a texel per vertex. The clone turned such a piece by the
    mirrored axis and swapped its UVs, which pushed the inset outward and,
    the sampler wrapping, drew the far edge's texels as a 1-pixel line (the
    "notch" on the profile dialog) and set the curve one pixel off. It now
    keeps the corner's own UV and mirrors the box (`push_box`): the curve
    lands on the original's pixels and the line is gone.
  * Unblended, alpha-tested textures (above): U2_HIGHLIGHT_BOX (DXT1,
    byte 0x56 = 0, 0x55 = 1), the dialogs' and the selected icon's green
    frame, showed in the clone as an alpha-blended 7, 75, 192, 192, 192,
    75, 7 across its edge; the original's is 121, 194, 194, 194, 121 —
    now matched to the level.
  * The icon strips' options (clones of OPTION_N, 0x52F8D0) draw in their
    template's z bucket (100 in UI_OptionsMain, 10 in UI_Main / UI_pause),
    under the selection glow CATEGORY_ONLY (0xDF422345); the clone drew
    them on top of everything, so the selected additive icon was not
    dimmed by the glow (Options / Audio centre 174, 195, 142 against the
    original's 148, 169, 116; now 149, 170, 117).
  * The quick-race track strip's master ICON_MASTER (0x5435E0 from
    0x4E2F30 hides it with 0x50CA00) drew as a flat white box under the
    selected track (inside 140, 144, 135 against 69, 72, 64).
  * Coloured images (kind 9, 0x536C90) take their vertex colours from
    their own corner colours (SA dwords 21..36), not the object colour
    (only the loading screens' tip box; off screen at 4:3).
  * The pause strips (UI_pause) do not fade the icons near the strip's
    ends: 0x52F8D0 runs the fade (0x50D8D0 → 0x51FD70) only with the
    scroller's +0xE4 set, which 0x538B40 sets (from StopDim 0x538CB0 and
    three other callers). **Observed**: the pause's right icon at full
    alpha (143 / 157 against the clone's faded 132 / 145); which screens
    take that path is **inferred** from the frames.
  Checked and matching: glyph shapes, advance, kerning and baselines (no
  pixel off in the text of any screen), the half-pixel placement, the
  UV inset, the 9-slice edges, the button outlines. The vertex colour's
  halving (every draw path's `c / 2`, the shader's `add r0, r0, r0`)
  would drop an odd channel's last bit, but the frames keep it (strip
  box 92.4, 92.1, 91.8 against the clone's full colour 92.4, 92.1, 91.7
  and the halved 91.8, 91.8, 91.1): why is **open**; the clone keeps the
  full colour. **Open** (not drawing): the scene under the menus (the
  clone's showroom car and the garage scene in the graphics / paint
  shops), the dialog's dimming of the screen behind it (the clone's is
  black), the pulse phase of the selected row's text, and the 350Z
  turntable car.
* **Marker icons** (**verified**, 0x48BDF0, 0x5C0E20, 0x5B7DF0,
  0x5BB440): the marker effects' icons (`XEE_Event_*_IconA_01`, texture
  MODE_ICON_* 128 × 64, orientation 0, aspect 1 = 1 × 0.5, 3 m up) are
  the particles' first list. 0x48BDF0 hands 0x5C0E20 the position,
  colour, half extents (size × aspect / 2) and the flip-book cell; the
  angle it also stores (+0x50) is never read, so camera sprites do not
  spin. The axes are the view matrix's columns 0 and 1 (0x5B7DF0 copies
  them to 0x873370 / 0x8763C0): the look-at's right and down (0x5BA9B0).
  0x5BB440 writes p + wR + hD (u1, v1), p − wR + hD (u0, v1), p − wR − hD
  (u0, v0), p + wR − hD (u1, v0) and zeroes the quad when the colour's
  alpha is 0x14 or less. The clone had used the second list's corner
  order (u along −up) for them too, so the icons lay on their side and
  spun. Second list (orientation 1–3, e.g. the RingA ground wave): A, B
  are the identity's rows (1: X, Y; 2: X, Z; 3: Y, Z) turned by the angle
  — 1: A (c, s, 0), B (−s, c, 0); 2: A (c, 0, −s), B (s, 0, c); 3: A (0,
  c, s), B (0, −s, c) (the clone had 3's sign of s flipped) — with corners
  p − wA + hB (u0, v0), p − wA − hB (u1, v0), p + wA − hB (u1, v1), p +
  wA + hB (u0, v1).
* **In-game World Map** (the World Map key, M / F2, DIACTION 16, code
  0xC0; verified unless marked): 0x602BE0's case 0x602EF5 opens it only in
  a career with no race (0x500B60: 0x850078 = 1, 0x89E7B0 = 0, so not in
  an outrun), 0x85EF50 +0x1C clear and no screen request pending (career
  +0x7138, then set): flow event 0x12 → 0x60B3B0 saves the mode and
  pauses (mode 6) → 0x606740 opens UI_InGame_WorldMap (screen record
  0x7F827C: factory 0x4F9C10, constructor 0x4EE320, messages 0x4E08E0;
  the map widget 0x4E0440 at +0x4C). Back (0x911AB364) closes it the next
  tick (flow event 8, 0x5F4250) and free roam resumes. The map object
  TRACK2000_MAP (0x90FE80F3, 352.75 square at (116, 6)) shows
  TRACKMAP<0x89E7A0> through the zoom table 0x7F42C8 ({cx, cy, size,
  label} in texels of 512: Full Map (0, 0, 512), City Core (186, 175,
  307), Beacon Hill (147, 114, 223), Jackson Heights (23, 0, 233), Coal
  Harbor East (251, 299, 213), Coal Harbor West (132, 341, 171)); UV (cx,
  cy)/512 … (cx + size, cy + size)/512; `[` / `]` (0x5073EF13 /
  0xD9FEEC59, 0x4CBDA0) step it, wrapping, up to the stage's record
  (0x7F4328 by 0x862838: 1, 1, 2, 3, 4, 5, 5) and write its label to
  0x42FAAD5A. Icons (0x4B09E0): centre + (fx·W, fy·H), fx = ((x −
  left)/size − uc)/span, fy = ((top − y)/size − vc)/span, span = size/512,
  shown only strictly inside the map (0x496760), 11 × 11 at zoom 0 and 15
  × 15 zoomed; objects `<NAME>%d` (table 0x7F4348, format 0x78F6F4) from a
  per-type counter starting at 1 (AI racers 0; 0x835F38). Left / Right
  step the selection (group 0xE2848973 with "Pulse", 0x4B0850). The clone
  (`screens::InGameWorldMap`, `--shot ingamemap`, NFSU2_MAP_ZOOM /
  NFSU2_MAP_SELECT) does all of that; it hides the second "Full Map"
  string 0x632B4FA8 (flag 0x40, see below). **GPS** (**verified**,
  0x4E08E0): C (BUTTON1, 0xC519BFC0 → 0x4B10C0) with the GPS off and an
  icon selected (the widget's +0x14) asks "Are you sure you want to
  engage the GPS system?" (0x63477036; Yes 0xF770, No 0x061C; 0x5587A0);
  Yes (0x51123592) engages it on that icon's entry (0x529490) and sets
  +0x60, so the map closes the next tick (0xC98356BA → 0x5F4250(1)); No
  (0xB4623F67) refreshes the help bar. With the GPS on, C shows
  "Deactivating GPS" (0x12C23B2C, OK 0x0639; 0x558620) whose OK
  (0x34DC1BEC → 0x496CE0) clears it (0x5010E0). **Help bar**: the
  package's own footer ("blah 1", "blah", "Back", the $JOY_EVENT_
  glyphs, the second "Full Map") is flagged 0x40, and every package's
  init (0x54E4B0, "Init Package %s") hides flag-0x40 objects (visitor
  0x51C870) — the duplicate "Activate GPS" the clone drew. The PC help
  bar is UI_PC_Help_Bar (record 0x7F7EA8, constructor 0x551F90, global
  0x8384D8): slots `button%d` / `icon_%d` from 1 (0x551FF1; the append
  order is **inferred**); 0x54E6F0(mask) hides every slot, then gives
  each set bit, low to high (skipping 0x8000, 0x10000, 0x20000,
  0x80000000), the next slot with its label (0x52D340: 0x400 "Back"
  0x1C0787E6, 0x10000000 "Activate GPS" 0x43512519, 0x20000000
  "Deactivate GPS" 0x11159F62, …). The map's mask is its record's +0xC
  (0x18400) | 0x10000000 with the GPS off, 0x20000000 on (0x4B0FE0):
  button1 "Back", button2 "(De)activate GPS". The clone applies the
  flag-0x40 rule to this package and draws the help bar's slots only;
  the rest of UI_PC_Help_Bar (the player name / cash group 0x3B949CD0,
  "RealGroup" faded in for masks without 0x18000, OL_ICON_GROUP, the
  res-2 backings) are **open**; the flag-0x40 rule now holds for every
  package (see **Help bar in the career and in game**). Selftest:
  activate on a marker, deactivate.
  **No free destination** (**verified** 2026-11-01; player report
  "on the map you can't set your own destination"): the original has no
  free cursor, waypoint or pan. The handler 0x4E08E0 takes only Left /
  Right (marker step), Up / Down (legend focus), Accept (legend toggle),
  C / BUTTON1 (the GPS button 0x4B10C0), `[` / `]` (the fixed zoom records
  0x7F42C8), Back, the Yes / No / OK answers, the tick and the mouse
  (0x9803F6E2 → 0x4CBEE0, zones: markers, legend rows, zoom arrows only).
  The router's goal setter 0x413120 has one caller, GPS_Engage 0x529490,
  whose point is always a marker entry's route trigger GPS point
  (+0x34..+0x3C); GPS_Engage's callers are this handler (the selected
  marker, widget +0x14) and 0x532D90 (the intro event's shop). So a
  destination is any marker the map lists (events, shops, the garage /
  car lot, sponsor events, milestones, world challenges; not the AI cars
  or the player). **Observed** under Wine (fastgpu, fresh career at the
  airport, GPS on the car lot): clicks on empty map points, the right
  button, the wheel and the arrow keys changed nothing in the GPS
  (0x838590 unchanged) and opened no dialog; with the GPS off and nothing
  selected (Car Lot hidden) C did nothing; a click on the Car Lot marker
  asked "Are you sure you want to engage the GPS system?" and Yes closed
  the map with the GPS on the car lot (point (−723, −913, 17); hash
  0xEC17E618; the intro's engage 0x532D90 had set 0xDD60E402, its own
  hash path), the HUD arrow pointing and the minimap showing the target.
  The clone's shot gives the same hash and point. The clone already had every path (keys, pad, mouse, help
  bar). **Real input** (2026-11-01; `scripts/map_xtest.sh`: the
  clone windowed on Xvfb, xdotool keys and clicks; the original under
  Wine with the same saves — stage1_win_streetx (Car Lot, Performance
  Shop, Garage, Circuit, Drag, Drift, Sprint) and the finished-career save (stage 6,
  a finished career: Body Shop, Car Lot, Car Specialties, Graphics,
  Performance, Garage; no event markers in either game)): the legend
  rows, the opening selection, Right ×3 / Left (the focused row's group),
  Down (row focus, the group's first marker), Enter (the row's group
  hidden / shown), C, the marker click and hover, the help bar's button2,
  Deactivate and a new target, the hashes (0x838590: Circuit 0x4F7D2AC5,
  SH_BODYSHOP 0xB88BD8C1, CC_GRAPHICS 0xCB1B31D5) and the HUD after Yes
  (the 3D arrow, the minimap target; at the finished-career save's garage spawn, off the
  road, "Searching Connection" with a blinking "Unable to contact
  satellite", router state 1 / result 1 / last 4 from the first frame in
  both) all matched. Fixed: 0x4B10C0 passes the map's package (+4) to
  0x5587A0 / 0x558620 and 0x5587A0's eighth argument 1, so the question
  opens on **No** (C then Enter does not engage; the clone had Yes) and
  both dialogs stand over the map with its help bar hidden (the clone drew
  them over the bare world with the bar). Shot `mouse_map_gps` (Deactivating on a marker click with the GPS
  on, an empty-map click does nothing, a marker click → the question →
  Yes → the GPS and its route on that marker).
  **Items** (**verified**; 0x4E0440): the marker table's entries
  (0x500EE0) that pass 0x533120 (or a world challenge running, 0x501690),
  each by 0x4B0670 at its route trigger box's centre ((min + max)/2 of
  +0x20..+0x2C) with group 0x4965F0(type 0x500DE0) and object
  `<NAME><n>` (counters 0x835F38[15] all set to 1 by 0x4E0440; a sponsor
  event's icon, +0x34 = 1, gets texture 0xB90F50BB); then the active AI
  cars (+0x550, 0x4B0740, group 0xE, counter 0x835F70 = 0); then the
  player: group 0x10, PLAYERCARINDICATOR (0xDD9EF5FF — in the package),
  car +0x60 / +0x64, heading +0x40C·2π/65536. Types (0x500DE0): an event
  0x5002E0 (+0x0D → 7, URL +0x34 = 2 → 5, else by +0x0F: 1 → 4, 2 → 2, 4
  → 1, 5 → 3, others 0), a shop 0x500340 by +0x50 (1 → 9, 2 → 10, 5 →
  11, 3 → 12, 4 → 13, 0 → 14, else 10), a milestone 6, a challenge 0xF;
  groups 0x4965F0: 0–5 same, 7 → 6, 9 → 7, 10 → 8, 11 → 11, 12 → 9, 13 →
  10, 14 → 12, 6 / 8 / 15 → 13. 0x4B09E0 shows an item inside the map
  when it is the AI or player group or its group is not hidden.
  Selection: on opening the GPS target's marker (0x4B07B0 by 0x514160)
  with the GPS on and its group shown, else the first marker. Left
  (0x9120409E) / Right (0xB5971BF1) only with the GPS off (0x4AE930),
  0x4B0850(3 / 2, the focused row's group or −1): forward / back through
  the markers (not the cars), wrapping, those inside the map and in the
  group; a hidden group drops the selection; none found keeps it.
  **Legend** (**verified**; 0x4EE320): 0x496870 hides DATA_A..D_n for
  n = 1..14; then a row per group first met among the markers (in their
  order, at most 14), ICON_BACKING_n its focus button, 0x4B0D50 / 0x496A40:
  DATA_A the check box (0x496920: shown — texture 0x62BC0DFC, colour of
  PLAYERCARINDICATOR; hidden — 0xB90F50BB, colour of PAINTSHOPINDICATOR_1),
  DATA_B the group's `<NAME>1` current image (0x50C980 → 0x50C990, so the
  cross 0xB90F50BB when that first marker is a sponsor event: seen under
  Wine with the community save's Sprint row; clone fixed)
  and colour, DATA_C the type's label
  (0x4966A0: 0 0x268D063C … 14 0x9D53BD10), DATA_D (0xB90F50BB in the
  icon's colour) only when the row's first marker is a sponsor event; B and
  D at alpha 0x3C while hidden. Row 1 is focused (0x4B0E70): DATA_C runs
  Button_Highlight, the old one Button_UnHighlight or, hidden,
  Button_Disable (0x496B20 / 0x496C50); with the GPS off and the group
  shown the selection goes to its first marker (0x4B07F0). Up
  (0x72619778, 0x4B1060) goes to the row above, Down (0x911C0A4B) below,
  wrapping; none focused → the last row. Accept (0x0C407210 → 0x4B0EF0)
  flips the group's flag (0x496390; bytes 0x8635A9.. with 6 / 8 / 15
  sharing 0x8635AF / B1 / B8; set = shown) and, shown again, selects its
  first marker (GPS off) or the GPS target's (on); a selection in a hidden
  group is dropped. The flags are saved (career +0x8869, see "Native
  profile saves"); the clone keeps them in the profile
  (`Profile::map_hidden`, `Game::map_hidden` without a profile).
  There is no info panel for the selected icon on this screen: the
  handler sets no text but the zoom label. Selftest: legend, hide / show,
  Down, the player icon. `--shot ingamemap`.
  **WorldMapMain**: no package, string or label of that name is in the
  executable or the game files — nothing to port.
  **Open**: the AI racers' icon visibility in a career outside an outrun
  (none in the clone), the icon colours beyond the package's, the rest of
  UI_PC_Help_Bar, the mouse handler 0x9803F6E2 → 0x4CBEE0, the legend
  move sounds (0x50D180), the save of the hide flags.

## Hydraulics (2026-10-31, hydro; **verified** from the assembly unless marked; checked against the original under Wine)

Code: `src/hydraulics.rs` (input, physics, garage), `src/audio/hydraulics.rs`
(CARSFX_Hydraulics), `drive::CarState::hydraulics`, `style::hydraulics`,
`devprobes/src/bin/hydraulics_parts.rs`. Shots: `hydro`, `hydro_bounce` (free roam,
the original's key timelines; `HYDRO_LOG=1` prints every frame,
`NFSU2_HYDRO_CAR=350Z` a stock car of that type), `hydroshop`.

* **The part** (car specialties slot 0xA1, cost category 0x24; all 46 car
  types have the same four, `devprobes/src/bin/hydraulics_parts.rs`): NO HYDRAULICS
  (6411A0F4, level 0), LVL 1 / 2 / 3 HYDRAULICS (1DB68B76, 4EA9E8D7,
  7F9D4638) at $750 / $1100 / $1600, 100 / 200 / 300 visual points
  (Visual Rating 0.20 for level 1 in the original's Customize), each
  unlocked by its cost level (type 1, a shop unlock). Fitting it
  (0x61BE0E, in 0x61BCE0) stores part byte +5 >> 5 in the
  car record's +0x581 (constructor 0x610280 clears it; 0x631A80
  serialises it). The save keeps the fitted part like any other slot (the
  native profile's installed parts); nothing else is saved: the heights
  and inputs start at 0 with every drive. 0x42FD60 (IntroNis15) tests
  the record's level > 1 for an in-engine scene (not ported: scenes).
* **The shop**: the generic UI_IcePartsOverlay list (MINI_MASTER /
  PARTS_ICON), as the other specialties. Live (unlock-all byte 0x838464
  written to 1 in memory for the check): the original's Customize lists
  the four by name with the Visual Rating meter and no price; the clone's
  career shop lists them with their prices (`hydroshop`, 1280 × 960).
* **Input** (human input object, vtable 0x784FC8): handler 0x4078A0 for
  the events 0xA5…0xAE (registered by 0x410480, removed by 0x407050). It
  returns at once without a level (car +0x2A8 → +0x10 → +0x581, or the
  override g_HydraulicsLevelOverride 0x7E949C when ≥ 0; −1 in the image,
  no writer) or while not available (+0x2F9). BOUNCE 0xA5 (Controls row
  17, action 19, J / U) and PRESSURIZE 0xAA (row 16, action 22, H / Y):
  pressed, the sim clock (0x865188) goes to +0x300 when no key was held,
  and +0x2FA = the event; let go, with a key held and a mode on, the mode
  (+0x2FB) and +0x300 clear; +0x2FA = 0. Axes, value (byte − 128) ·
  −1/128 (0x784540, 0x78453C): bounce up/down (0xA6, 0xA8) +0x308,
  left/right (0xA7, 0xA9) +0x304, pressure up/down (0xAB, 0xAD) +0x310,
  left/right (0xAC, 0xAE) +0x30C only with level 2 or 3; after a pressure
  axis, both pressure axes under 0.2 (0x783B0C) clear the toggle +0x2FC.
  **Live** (keyboard): the throttle / brake keys (Up, W / Down, S) send
  up/down, the steering keys left/right, to both bounce and pressure axes:
  Up 1.0, Down −0.9921875, Left 1.0, Right −0.9921875, a pair together 0.
  The pads' bindings of the SELX / SELY / ANALOG events are **open** (the
  event map is not in an uncompressed file); the clone uses the same
  digital actions.
* **Each frame** (0x407400, from HumanDriver_Update 0x41E2C0):
  available (0x407370(limit)) with g_HydraulicsOn 0x7E94A0 (1, no writer),
  |car +0x430| × 2.237 under the limit, not drift (0x89E7D9 / 0x890104)
  nor street X (0x89E7E3 / 0x89010C). Not available at 30 mph after being
  so: both input sets (+0x1EC…, +0x1FC…), the mode and the axes +0x304,
  +0x308, +0x310 cleared (+0x30C kept). A key held over 0.22 s (+0x300 ×
  0.00025) enters its mode if available at 10 mph — pressure 1, bounce 2
  ((+0x2FA ≠ 0xAA) + 1) — and the time clears either way (live: the mode
  came 0.236 s after H). Mode 2: the bounce inputs +0x1EC + 4i are zeroed
  and set from the axes over ±0.8, up/down first: up → corners 0, 1 =
  value, down → 2, 3 = −value, left → 0, 3, right → 1, 2. They are only
  rewritten in mode 2, so letting go of the key and the arrow together
  leaves them on and the car bouncing (live, as coded). Mode 1: the pair
  from the pressure axes (up 3, down 0xC, left 9, right 6); with the
  toggle clear, a picked corner at rest (state 0) sets it to raise (1),
  else one up (state 1) to lower (−1); none, return. Then each picked
  corner's pressure input (+0x1FC + 4i) is 1 (raise) or 0. Corners: 0
  front left, 1 front right, 2 rear right, 3 rear left (live: Left raised
  0 and 3). While a mode is on, the throttle (0x416DE0: +0x2D0..+0x2D4),
  brake (0x407EB0: +0x2DC) and reverse (0x4077D0: +0x2E4) events are
  dropped and the steering input is 0 (0x41E341). **Inferred**: the
  clone zeroes the pedals while the mode is on (the native keeps them at
  0 until the next event).
* **Physics** (Mover_Hydraulics 0x5A51C0, the force pass 0x5B0530's last
  step, every sub-step; [mover +4] the controls, [mover +0x58] car +0x40):
  skipped while the car has a live contact ([[mover +0x5C] +0x3C] +0x18 >
  0). The bounce pair: corners 0 and 1 on → the rear (mask 0xC), 2 and 3
  → the front (3), a diagonal → none (0x20). Per corner: target =
  pressure × 0.2 (0x7FC09C), at least 0.2 (0x7FC08C) for a masked corner;
  the height (car +0x3F8 + 4i) moves at 0.5 m/s (0x7FC098 × the sub-step
  0x7FC800) up to the target (state 2), or down (not below 0, state 3);
  at the target, state 1 if above 0, else 0 (mover +0x74 + 4i,
  Car_HydraulicsState 0x5ED480). A kick needs a mask, its corners at 0.2,
  and no bouncing wheel's suspension (mover +0x28 + 4i) compression +0x1C
  under −0.01 (0x7FC090); then each bouncing wheel whose hub (+0x30 turned
  into world axes) moves along its ground normal (+0x150) at v_n ≥ 0,
  with v = body velocity +0xA0 + ω +0xC0 × r taken before any kick, gets
  J = (0.9 − 0.3 v_n) · bounce × mass (+0x164) along the normal, if
  positive, through RigidBody_ApplyImpulse 0x59BC70, and its byte car
  +0x408 + i is set. The mover's frame (0x5B72E0) clears those bytes and
  Mover_SetSuspensionMounts 0x5AB9A0 → Suspension_SetMount 0x59E0D0 sets
  each mount (+0x90) to the wheel point less the centre of mass with z
  less the height (live: mount z −0.005 at rest, −0.205 at 0.2). The
  MeloMvr has a twin (0x5A2FE0, from 0x5AB15B), not ported: its cars are
  AI-driven. **Open**: whether an AI controller's +0x1EC… can be non-zero
  (other controller classes have other fields there); the clone gives AI
  cars no hydraulics input.
* **Effects elsewhere**: Car_RestClassify 0x5992C0 gives class 0 (no rest
  hold) with a bounce input (Car_HydraulicsBounceInput 0x5ED2A0) or a
  corner moving; HANG TIME needs no bounce input; style BOUNCE (entry 33,
  two or more kicks a frame) and HYDROS (entry 38, 0.25 per moving corner
  without a bounce input) from 0x56F9C0. The chase camera (0x4535A5, view
  kinds 2 / 4) also branches on 0x5ED2A0 / Car_HydraulicsRaised 0x5ED310
  (a height or pressure above 0) — not ported, as that branch is not
  ported for its other case either (**open**). Hydraulics work in free
  roam and in races other than drift and street X, standing or under 30
  mph (modes from under 10 mph).
* **Live comparison** (fastgpu, a private Wine copy, career free
  roam, Rachel's 350Z; level given by writing 0x7E949C = 2; refs in
  <work dir> `refA2.json` / `refB2.json`; clone `hydro` /
  `hydro_bounce` with `NFSU2_HYDRO_CAR=350Z`): mode entry 0.23 s vs 0.22–
  0.23 s; the corners 0 → 0.2 in 0.40 s in both; front up: forward.z
  +0.084 (original) / +0.073 (clone, stock 350Z), all four up: the body
  +0.171…0.188 / +0.196 m, with the original's suspensions 1.1–1.4 cm more
  compressed (the rest hold's history); bounce: kicks start once the far
  pair reaches 0.2 in both, then about every 0.62 s (original, tuned car)
  / 0.54 s (clone, stock car), the body's first swing +0.18 / +0.13 m and
  the second +0.23 / +0.26 m. **Open**: the bounce period gap (the
  original's car is Rachel's tuned 350Z).
* **Garage** (Garage_HydraulicsUpdate 0x4A6030, each frame from
  0x4D82D0; the FE car object's +0x174 mode, +0x178 follow height, +0x17C
  pitch, +0x180 phase, +0x184 + 4i corners, +0x194 + 4i lifted flags,
  +0x1A4 + 4i states): the mode is the FE car's fitted slot 0xA1 part's
  level (FUN_00610300(0xA1) byte +5 >> 5, with 0x8363F8 and the car
  shown). Records g_GarageHydraulicsModes 0x7F6864 + 0x114 · mode (modes
  1–3, `hydraulics::GARAGE_MODES`): +0 the phase rate, +4 the corner speed,
  +8 the swing, +0xC the follow height, +0x10 the rest height, +0x14 the
  pitch factor (0), +0x18 / +0x1C a channel's clamp, +0x20 smooth (0: a
  channel is 1 above 0, else 0), +0x24 four channels (front, rear, left,
  right) × three terms {on, amplitude, frequency, phase, offset}: channel
  = Σ Sin16f(phase · frequency + term phase) · amplitude + offset. A mode
  change resets the phase; to or from mode 3 the corners first settle to
  the rest heights at 20 × swing per second (follow 0.15 unless going to
  3). Rest heights: +0x10, plus, when the suspension category's level
  (0x5A6590(5, 3) on the car's upgrade state +0x2F0) is not 0, its first
  gain × the car type's +0x714 (front corners) / +0x718 (rear). Targets:
  corner 0 = min(1, (left + front) − (right + rear) · 0.1) · swing + rest,
  1 = (right + front) − (left + rear) · 0.1, 2 = (right + rear) − (left +
  front) · 0.1, 3 = (left + rear) − (right + front) · 0.1; each corner
  moves toward its target at swing × speed per second. Mode 1 swings the
  front and rear, mode 2 also the sides, mode 3 pumps the front and rear
  smoothly (live: level 1 pitches, level 2 also rolls, level 3 pumps
  high; mode 3's record is written at start-up by 0x77D010 — the image
  holds zeros there, which the clone had taken: corrected 2026-11-01, see
  "Specialties: hydraulics level 3 and the purge"). Sounds (with the FE sound
  object, [0x82B884] +0x74): see sound-audit F19. The pose
  (0x4A6670): contacts at 0.15 + corner i, the axes from the four
  contacts as at a racer's placing, the wheels' matrices lowered by
  min(corner, follow) (`garage::Turntable::car_transform_on`).
  **Inferred**: the clone runs it in 1/60 s steps up to the menu's time.

## Specialties: hydraulics level 3 and the purge (2026-11-01, specials; player report on the finished-career save)

Player report (the finished-career save): "level 3 hydraulics in the shop not
working, also nitro purge". Wine: `$OB_PRIVATE/wine/runs/<run>` through
`fastgpu.sh` (:178, `--audio-file`), the finished-career save loaded, IND_CAR_SPECIALTY_2
entered from free roam (car moved to (732, −1175), Enter, Enter Shop);
captures in `<work dir>/specials-tmp` (`h3m.png` LVL 3 pumping,
`pum.png` / `pu20.png` Test N2O Purge, `ev_purge.tsv` / `ev_gpurge.tsv`
sound events). Clone shots `finished_specials_hydro3`, `finished_specials_purge`
(checks in the shot; the shot fails on a mismatch).

* **What failed** (clone, before): (1) LVL 3 HYDRAULICS listed, priced
  (1,600) and bought fine, but its garage motion stood still: the mode 3
  record of g_GarageHydraulicsModes (0x7F6864 + 3·0x114) is zeros in the
  image and the clone had taken those; (2) the FE car pumped wherever a
  hydraulics part was fitted, not only in the Hydraulics list; (3) the
  Nitrous Purge list's Test N2O Purge did nothing (no key, no help-bar
  click, no smoke, no sound); (4) in game the purge played FX_PURGE but
  showed no smoke (kinds 13 / 14 were open). Listing, prices and buying /
  fitting the purge and hydraulics worked (`finished_specials_*`).
* **Mode 3's record** (**verified**): the static initializer 0x77D010
  writes +0x10…+0x110 at start-up: rest 0.15 (0x78E024), pitch factor
  0.25, channel clamp −0.1…10, smooth (+0x20 = 1), front {on, 0.5, 2.25,
  0, 0.4}, rear {on, 0.5, 2.25, 17, 0.4} (the other ten terms off: {1,
  3.3, 40, 0}, {1, 0.4, 0, −1}, {1, 2.0, 0, 0}, {0.5, 2.25, 0/17, 0.4},
  {1, 2.123, 40, 0}, {−1, 0.4, 0, −1}); +0…+0xC from the image (rate 4,
  speed 4, swing 0.5, follow 0.15). So level 3 pumps front and rear 9 rad
  s⁻¹ apart by 17 rad (Wine: the Evo's nose and tail rise ~0.6 m in turn).
  Clone: `hydraulics::GARAGE_MODES[2]`; the shot measures the corners
  moving 0.03…0.63 over 4 s.
* **g_GarageHydraulicsShown 0x8363F8** (**verified**): 0x4A6030 reads the
  FE car's slot 0xA1 level only while it is set. UI_ICEMAIN's list entry
  0x556940 clears it and 0x8A1C68 (lights), then sets it for Hydraulics
  (jump table 0x556BD0/0x556BEC → 0x556A41) and 0x8A1C68 for Lights
  (0x556A29); Back (0x54FB80(item, 0) at 0x568B8B), the overlay's exit
  0x5605C0 (0x560648) and 0x8ACAA8DF (0x568A1B) clear both. Wine: the
  strip and the other lists show the car still, the list pumps with LVL 3
  highlighted. Clone: `Game::hydraulics_list_open` gates
  `garage_hydraulics`.
* **Test N2O Purge** (**verified**): the help bar's bit 0x80000 (and the
  hoods' 0x100000, the doors' 0x200000) goes through 0x52D180 to
  0xC539A4B0, sent only there; UI_ICEMAIN's handler (0x568750, at
  0x568BC2) forwards it to UI_IcePartsOverlay as 0xC519BFC0 (BUTTON1,
  also the C key). UI_IcePartsOverlay's handler 0x568D40 on 0xC519BFC0
  with the Nitrous Purge item (+0x110 → +0x48 = 0xA2): +0x116 toggles,
  on → FESound_StartPurge 0x458B30 (lets a running one go, "SND: FEPurge",
  FX_PURGE with {0, 0, 0, 0}), off → 0x458C40 (the event's stop flag
  +0xC = 1). Its release 0xD871B0A4 (0x7F7D78[10]) with 0xA2: +0x116 = 0
  and the stop — so C held purges, the help-bar click toggles. Each tick
  (0xC98356BA) with +0x116 and the list open (+0x114 clear) it fires
  0x4C1920(13, 1.0, 0) → CarRender_FireEffectFE 0x61FBB0 → 0x615CD0;
  leaving (+0x114 set) stops the sound and clears +0x116. The FE_MB
  player's update 0x4793D0 fills the purge event's block each frame:
  {SFXObj level slot 5 (vt +0x38(5), 0…0x7FFF), 0, the stop flag, 0x1000}
  (0x4794DD…0x479529). Wine: C held 2 s → FX_PURGE {0x62E, 0, 0, 0x1000}
  for 2.8 s; smoke from both hood corners and outward from both front
  wheels, gone ~0.3 s after the release. The split hoods' / doors'
  Open/Close on the same message (0x521050 on the item's channel; the
  doors first test 0x4C2030 and set 0x838559 / 0x83855A) are **not
  ported**. **Inferred**: the nitrous bank FX_PURGE is found in for the
  front end (0x458B30 loads none; the clone opens the car's by the
  in-game rule, Nitrous_01 for nitrous level 2–3). Clone: C / a pad's
  Menu Customise press toggles, their release ends it (`Game::
  purge_test_set`, `key_released`; `Pads::released`).
* **The purge smoke** (**verified** unless marked): the car render's
  set-up CarModel_Build 0x634810 reads the ride's slot 0xA2 part (ride
  +0x818): HOODEMITTER / WHEELEMITTER name description groups; 0x48B8B0
  clones, into the kind 13 list (render +0x4C0) and kind 14 (+0x4C8), the
  loaded emitters (manager list 0x8357E8) whose description is in the
  group — GLOBALB's emitter records (chunk 0x35020, state 3, with their
  matrix: e.g. XES_NOSBO_C_Hood_SmokeA_01 z axis (−0.453, 0.423, 0.785),
  XES_NOSBO_C_Wheel_SmokeA_01 (−0.247, −0.815, 0.524)). Each kind is
  recoloured by Emitters_Recolor 0x48B920 with HOODHUE / HOODSAT /
  HOODLUM (kind 13) and TIREHUE / TIRESAT / TIRELUM (14) into one of 12
  colour slots (0x8A3F60 + 0x40·i, in-use flags 0x8A05D4; render +0x2D49 /
  +0x2D4A): per description its four keys (+0xA0, RGBA) go to HSL
  (0x488270), hue + shift (clamped ±1, wrapped), saturation + shift
  (clamped ±1, result 0…1), luminance + shift (0…1), back to RGB
  (0x4883F0), alpha kept, and 0x487940 fits the cubic through them at
  +0x90 × ⅓; the emitter's +0x30 = the slot, which the draw 0x48BDF0
  uses instead of the description's +0x100. Markers: group 28 (render
  +0x628): with HOODLEFT ≠ 0 the slot 0 LOD A model's (render +0x17A8)
  marker 0xE3DE69DB (0x48D8F0), HOODRIGHT 0x601A6CCE with the y flag;
  group 29 (+0x630): WHEELLEFT (y flag) at 0x5B5360(1)'s +0x10 + (+0x20,
  0, 0), WHEELRIGHT at its +0x40 + (+0x50, 0, 0) (**inferred**: that block
  is the car type's record from +0x110, so the points are wheel 0's and
  1's centre plus the radius in x). A description's +0x5C is its marker
  group (the NOSBO hood smokes 28, the wheel smokes 29, XES_NOS_FlameA
  10). 0x615CD0(kind, amount, …): for each emitter of the kind and each
  point of its group: the emitter's axes with the point's flagged
  components negated (bit 0 of the point's x / y / z words), at the
  point, × the car's matrix; velocity the car's; burst −amount
  (0x491E50); kind 13 then runs 14. In game 0x5F5FF9 fires kind 13 while
  0x5ECDC0 is not 0 (the purge); in the front end 0x61FBB0 with the FE
  car's matrix and no velocity. Under Wine in free roam, Left Alt held
  standing: FX_PURGE {0x6ED, 1, 0, 0x1000} for 3.9 s; a second run's
  frames show the smoke at the front corners from behind.
  Clone: `car::PurgeFx` (set-up), `particles::{recolor, Library::
  templates, group_templates, System::palette}`, `PurgeSmoke` (0x615CD0)
  in `Effects::step` (in game, per frame as the nitrous flame) and in the
  garage scene (per 1/120 s of front-end time while the test is on:
  **inferred** rate, the original bursts per FE tick, 120 a second at
  its frame cap); the colour fit solves the cubic directly instead of
  inverting the power matrix (rounding only). `NOS_SHOT=1 NOS_PURGE=1
  game --shot explore` stands the car with Left Alt held (purge smoke).
* Wine also showed: TYPE 1 ALL NOZZLES is the finished-career save's fitted purge (level 0,
  price column 0; thin white jets from the hood corners and sideways from
  the front wheels); TYPE 3 RED highlighted: 300, Visual Rating 3.14,
  dense red clouds over the hood and out of both front wheels — the clone
  the same (`wcmp.png` vs `NFSU2_PURGE_SHOW=… --shot finished_specials_purge`);
  a help-bar click on Test N2O Purge started FX_PURGE and the smoke kept
  going with nothing held (the toggle); the Hydraulics list NO / LVL 1 /
  2 / 3 at 0 / 750 / 1,100 / 1,600, Visual Rating 3.34 on LVL 3 (clone
  the same). In free roam with Left Alt held standing the original shows
  SPRAY SHOW +100 and faint smoke at the front corners (`gp15.png`).
* **Open**: the pitch factor's reader (+0x17C, 0.25 in mode 3; no reader
  found in the garage code).
