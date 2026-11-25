# Front end, menus and input

Part of [docs/formats.md](../formats.md), the traced behaviour of `SPEED2.EXE`; the tags
(**verified**, **inferred**, **open**) are explained there.

## Front end (verified except where noted)

The menus are FEng packages: FRONTEND/FrontB.lzc (137), GLOBAL/GlobalB.lzc
(19) and GLOBAL/GLOBALA.BUN (2). A package chunk is 0x30203 (plain) or
0x30210 (a u32 name hash, then a JDLZ or HUFF block).

* **Package** — four-character tags `tag u32, size u32`: `FEn\xE7`
  { `PkHd` (name at +0x18, then the file path), `TypS`, `Res\xCC` {
  `RsNm` NUL-separated names, `RsRq` u32 count + 24-byte requests {u32 hash,
  u32 name offset, u32 kind (0 untextured, 1 texture, 2 font), 3 × u32} },
  `Obj\xCC` { `Butn`, `FOb\xEA`* } }.
* **Object** (`FOb`) — `ObjD`, then `Scrp`* and `MsgR`*. `ObjD` and
  `Scrp` use two-character tags with a u16 size:
  * `Ot` kind (1 image, 2 string, 4 list, 5 group, 7 movie, 9 coloured
    image, 11 simple image, 12 multi image), `Oh` name hash, `OP` {id,
    name hash, flags, resource index}, `PA` parent id, `If` image flags.
  * `SA` the animatable state, 17 dwords: colour as integers B, G, R, A;
    pivot; position; rotation (quaternion); size — plus the texture
    rectangle for images (21 dwords) and corner colours for coloured images.
    The canvas's y points down and an angle turns **clockwise** on screen:
    0x51CEE0 builds the row-vector matrix [[cos, sin], [−sin, cos]], and
    UI_Sponsorship_new's SideScroll_Arrow_1 (top, −90°) must point up.
    Verified: with this sign the minimap frame's rotated arcs close into a
    ring and the turbo needle rests near 0.
  * Strings: `St` UTF-16 text as authored, `SH` label hash (`bin_hash` of
    the label, looked up in the language table), `Sj` justification, `Sl`
    leading, `Sw` wrap width, `Sb` (**verified**, 0x51C110: 1 centres each
    line on the origin and 2 ends it there; 4 centres the block
    vertically, else 8 ends it there — see Layout).
  * Flags: bit 0 is the runtime hidden bit (0x50CA00 sets it, with
    0x400000 "changed", down a group; 0x50CA50 clears it; the renderer
    skips such objects, 0x51C670; 6 shipped objects, all in HUD_Drag, have
    it set). 0x10000000 (**verified**) is not a hide bit: the `ObjD`
    reader (0x555150, test at 0x5552EC) puts every object with it in the
    package's button list (+0x90), the objects the pad moves between; the
    screens hide the buttons they do not use themselves (e.g. MC_Main's
    0x4A3460: 0x50CA00 and +0x1C |= 0x4000000, the bit that takes a button
    out of the focus order). **Verified**: nothing in the draw path tests
    0x10000000. The renderer's skip (0x51C670) reads only +0x1C bit 0, and
    the queue and renderer code in 0x51A000–0x51C8FF tests no other bit of
    +0x1C but 0x40 (0x51C870, which runs 0x50CA00 on the object and sets
    0x400000). So the game draws such a button unless the screen's code
    hid it. The clone's `hidden()` matches: only bit 0 or the screen's
    own hidden list hides, and `ScreenState::shown` keeps a bit-0 object
    on, standing in for 0x50CA50. 0x100000 marks an
    instance of a library object (0x5186C0 copies it in). A group with bit
    31 is skipped by the traversal (0x52C5E0) unless the renderer's first
    byte is set (**verified**): the renderer's constructor (0x536180, at
    0x536262) and the FEng manager's (0x5372F0, at 0x53738D, copying it to
    every package's +0x14) set it to 1, and no other write was found, so
    such groups draw — the clone ignores the bit.
* **Scripts** — `Sh` {name hash, length ms, flags, track count}; then per
  track `FI` {param type (4 vector, 6 colour), size, interpolation, —,
  length}, `To` (first `SA` dword it drives), `Kd` (the base value at time
  −1, then {time ms, delta} keys, linear); `Sc` the script chained after;
  `EV` timed messages. Names are `bin_hash`: INIT runs when a package
  opens.
  * Runtime (**verified**; `frontend::animate`, `feng::sample`). The
    reader (0x5515F0) keeps a script as +0xC length, +0x10 time (ms),
    +0x14 flags, +0x18 chain, +0x1C track count, +0x20 tracks (0x38 bytes:
    +0 param, +1 size, +2 interpolation, +3 loop mode / bit 7 "done", +4
    24-bit length, +7 first `SA` dword, +8 the base key, +0x30 the key
    list), +0x28 events, +0x30 name; an object's current script is +0x54,
    and the reader sets it to INIT (0x1744B3) directly.
  * Clock: the manager's update (0x558D89) ticks every package's objects
    (0x54CE70) in steps of at most 20 ms, then pumps the queues (0x541FD0,
    0x5559D0). 0x54CE70 adds the step to the time; at or past the length
    a chained script evaluates its end (0x541DC0), moves to the chain
    (+0x54) with the time carried over (t − length) and re-bases its
    tracks (0x517720); without a chain the flags & 3 pick: 0 hold (time =
    length + 1, then no more evaluation, so values the screen's code set
    stay), 1 loop (t mod length, re-based), 2 t mod twice the length; a
    zero-length script ends on its first step. A finished held script is
    not evaluated again (0x54D09F).
  * Tracks (0x541DC0 → 0x535500): interpolation 0 holds the key at or
    before t (0x519D40), 1 and 3 interpolate (0x52BDD0), others are not
    evaluated; the shipped packages use 1 everywhere but one track (3).
    A track with bit 7 of +3 is skipped; 0x517720 clears it when a script
    starts and 0x535500 sets it after evaluating a keyless track, so a
    keyless track writes its base once (the packages mark keyless tracks
    with it; none with keys). 0x52BDD0: 0x5052B0 finds the first key at
    or after t (else the last); f is (t − previous) / (key − previous) as
    a float (1 with no previous key or t at or past the key); the loop
    mode (+3 & 0x7F) 1 runs past the last key towards the first, reached
    at length + the first key's time (and before the first key from the
    last), 2 reflects t past the length. Values are the base plus the
    delta: floats a + (b − a)·f (0x505150); colours (and ints) a +
    ftol((b − a)·f + 0.5) (0x5051A0; ftol truncates); rotations slerp
    (0x519B00: −b for a negative dot, nlerp above 0.999, a 16-bit angle
    — a float here) and compose delta · base (0x4FE6B0, Hamilton), not
    add. Before, the clone added quaternion deltas (halving a rotation's
    animated angle: HUD_SingleRace's wobble, UI_ChooseCustomHUD's needle),
    rounded colour steps, held loop tracks past their last key and ended
    scripts only past their length.
  * Message response type 0 (0x52C7D0) sets the object's script to the
    one named by arg 1 (0x5178B0 → 0x517720) and its time (+0x10) to 0 —
    the "entry count" read for +0x10 under Message responses is the
    script's time.
  * Interpolation 3's re-basing (0x517720, **verified**, `rebased`): when
    a script starts on an object (0x5178B0, 0x51CF80, a chain step or a
    loop in 0x54CE70) every track's done bit is cleared and a track with
    interpolation 3 or 4 gets its first key's delta (the key list at
    +0x30) set to the object's current value minus the track's base
    (param 1 int, 2–4 floats, 5 quaternion via 0x4FE6B0 — operand order
    **inferred** —, 6 colour ints via 0x505580). **Open**: the other key
    list used when the track's +0x2C byte is set (0x4FE890).
  * Timed messages (`EV`, **verified**, `run_package`): each step
    0x54CE70 fires the script's events in [old time, new time), the new
    time + 1 when it equals the length (0x534C80), so a zero-length
    script's events fire on its first step and a loop's every cycle. The
    target picks the route: 0 queues the message with value 0 (broadcast),
    −4 / −5 / −6 with that value, −1 calls the host's +0x18 at once, any
    other value is an object id (`OP` +0) resolved by 0x517FE0; message
    0x1B3909AA moves the focus instead (0x52B3B0). The queue is pumped
    after the step (0x541FD0, below).
  * Screen in (**verified** for the menus): the packages' "Event Handler"
    string (0x47FF4E7C) runs INIT (0 ms) → CALL, whose events broadcast
    0xBDFD2DF9 at 0 ms and send 0x35F8620B to the screen at its end
    (180 ms in UI_Main and the dialogs, 320 ms in UI_pause). The menu
    objects answer 0xBDFD2DF9 with their FADEIN script, so their INIT →
    HIDE is followed by the fade in (the open item of the previous note;
    the clone follows HIDE now). IconScrollerMenu's handler (0x543D50)
    sets its ready byte (+0x130) on 0x35F8620B; the dialogs focus their
    Cancel_Button then (below). **Open**: what reads +0x130 (the input
    gate during the fade).
  * Screen out (**verified** for the handler 0x495B10, which switches to
    UI_CareerCrib.fng): on accept (0x0C407210), or back (0x911AB364) when
    0x404800 is false, the screen runs FORWARD on the
    Event Handler (0x537C10 → 0x51CF80, unless 0x537CD0 finds it running);
    FORWARD broadcasts 0xAADD0187 (the objects' FADEOUT) at 0 ms and sends
    0xE1FDE1D1 to the screen at 140 ms (320 ms in UI_pause), on which
    the screen switches package (0x537990) — IconScrollerMenu (0x543D50)
    runs the chosen option's action then. The Event Handler's own accept
    response does the same ([0x301 FORWARD] [0 FORWARD] [0x501]).
    **Not ported**: the clone switches screens at once; its screens do not
    run FORWARD or wait for 0xE1FDE1D1.
  * Objects the screen's code shows (**inferred** stand-in): an object
    whose zero-length INIT chains to a non-looping script (HIDE, or
    UI_pause's slot 6048A18B) and that no package message reaches keeps
    INIT's state in the clone (`ScreenState::chain_init` off), standing in
    for screen code such as 0x51DDA0 (Fade_In / Fade_Out on the cash and
    player name, 0x537E70) that the clone's screens do not all port. The
    HUD and the shop engage dialog set `chain_init` (fully native).
* **Message responses** (`MsgR`; **verified** except where noted). The
  tag test at 0x5555C9 hands the body to 0x54D1E0, a block of two-character
  tags: `Mi` (u32 message hash) starts a node (hash at +0xC, count +0x10,
  entries at +0x14), and `Ri`, `Ru`, `Rt` fill the next entry's three
  dwords: type, arg 1, arg 2 (12 bytes). `MC` (0x52AE10) and `Rs` (0x517630)
  are not traced, nor the entry array's allocation. The FOb reader's call
  (third argument 0) registers the node with 0x4951B0 (0x512120 otherwise);
  with the package's flag at +0x18 set, a node the manager already holds for
  that message is emptied and reused (0x5046D0 finds it).
  * Queue: 0x51AA20 appends {target, message, parameter, value, flag} to the
    manager's queue (+0x411C / +0x4120, count +0x4118). The pump (0x541FD0)
    routes by the value: 0 broadcasts to the objects of that hash in every
    screen, then to each screen's per-message list (0x5367F0); -2 to the
    objects of that hash in every screen; -3 to the object of that hash in
    the target's list; -4, when the target is one of the screens, does the
    -3 effect (the object of that hash in the screen's list) and then also
    delivers to that screen's per-message list (0x5367F0, block 0x54219E);
    -1 and -5 are host callbacks at +0x10C (vtable +0x18 and +0x1C); -6 sets
    the input flag (+1) for message 0x59BED120 and clears it for 0x5D4CE32D;
    any other value is an object, sent through 0x5367F0.
  * Object lookup (0x5367F0): the object's node list (+0x3C; next at +0x4,
    hash at +0xC) is searched, and the first match runs (0x52C7D0). Kinds 4
    and 6 try 0x51B000 and 0x52C760 first.
  * Entries (0x52C7D0; a MsgR entry's dwords are type, arg 1, arg 2): 0x1
    queues message arg 1 with value arg 2 (through 0x517FE0 unless -4 or
    -1, so the value picks the route); 0x2 and 0x3 queue arg 1 with value
    -1 and -5; 0x101 sets the input flag (+1) to arg 1 == 1; 0x105 queues
    0x51AF50(object, 1, the message's flag word), 0x106 (object, 1, 0xFF),
    and 0x107 and 0x108 the same with flag 0; 0x200 queues kind 3 (0x51AB20)
    with arg 1 as a name string (copied by 0x519DE0 into the node at +0xC,
    its hash at +0x10) and the object's flags, then clears the object's
    flags +0x1C to +0x28; 0x201 queues kind 2 with the object's flags and
    clears them (0x51AC10; it also copies arg 1 as the name, as 0x519DE0 does), 0x202 does so for each set bit of the message's
    flag word, 0x203 queues kind 1 (0x51ADB0) with the object's flags, sets
    every object's flags (+0x1C and +0x28) in the manager's list at +0xE4 to
    its +0x20 copy and then clears the object's own, 0x204 with no bits.
  * Focus (0x52B3B0, which 0x100 and 0x103 call): with the flag set, the old
    focus gets 0x55D1E635 and the new one 0xABC08912, each sent to the object
    and then as the -5 callback; the focus is stored at +0x8C either way.
    0x100 with arg 1 of 0 clears the focus silently; otherwise the object
    0x517FE0 finds for arg 1 takes it, and nothing changes when it finds
    none. 0x102 stores, under the object's hash (+0x10), the +0xC word of its
    +0x8C child (0 when none) in a 32-entry ring at the manager's +0x5150
    (next slot +0x514C; 0x50B300 clears an older entry with the same key).
    0x103 focuses the object that 0x517FE0 finds for the value that 0x50B360
    returns for the object's hash, else does what 0x100 does with arg 1.
  * 0x2C0 pushes a copy of the object's +0xC name onto a stack of 16 slots
    of 0x100 bytes at the manager's +0x414C (count +0x4148; 0x51B070, a full
    stack does nothing); 0x2C1 pops the top slot when the stack is not empty
    and queues it as kind 3 for the object (0x51AB20); 0x2C2 empties the
    stack (0x50B390).
  * 0x0, when the dispatcher's context argument is non-null: finds the node
    named by arg 1 (0x5046B0: the manager's list at +0x4C, hash at +0x30),
    passes it to 0x5178B0 (which stores it at +0x54 of the object it is
    called on — the object's current script — and, for a non-null node,
    runs 0x517720, which re-bases the script's tracks, see Scripts), and
    sets the script's time +0x10 to 0 (**verified**: +0x10 is the time
    0x54CE70 advances).
  * 0x300 (skips when the receiving object's current script, +0x54 →
    +0x30, is not arg 1; **corrected**, it is not a screen id), 0x301
    (skips when it is) and 0x500 (always) skip with 0x504490: from the next entry it
    counts nested 0x300/0x301 (+1) and 0x501 (-1) and stops at the first 0x500
    or 0x501 at depth 1; the dispatcher resumes after that entry. So a skipped
    0x300 or 0x301 runs the part after the 0x500 (an if/else: then, 0x500,
    else, 0x501), a 0x500 jumps past its 0x501, and 0x501 does nothing.
  * The second queue (+0x412C / +0x4130, count +0x4128) holds the kinds 1,
    2, 3, 4 and 8 queued by 0x51ADB0, 0x51AC10, 0x51AB20 and 0x51AF50.
    0x5559D0 drains it from the head (**verified**), dispatching on each
    node's kind bits. Bit 0 (kinds 1 and 3) pops the target package when its
    +0x18 is not negative: a parent that is not in the manager's list is
    logged as invalid and cleared; the node's flags are OR-ed into the
    parent's; 0x5362F0 flushes the package; the objects whose parent is the
    target then take its parent (kind 1) or the object 0x555870 returns
    (kind 3). Bit 1 (kinds 2 and 3) walks the list headed at 0x8384C4 +0x10
    to the first entry that 0x43DCC0 returns 0 for, then calls 0x555870 (both
    not traced further). Bit 2 (kind 4) sends message 0x334C5493 with value
    -4 to the target's parent, bit 3 (kind 8) to the first object whose
    parent is the target. 0x51AE80 unlinks the last kind 2 or 3 node in the second queue whose
    name hash (+0x10) is the hash of its argument (1 when found); 0x5362F0,
    the unload of a package in the manager's list, deletes the second-queue
    nodes whose object is the package and the first-queue messages whose
    object is the package or whose value 0x517FE0 resolves (each logged as
    unsafe), then releases its objects and itself (0x50B240, 0x52C510 and the
    destructor; not traced further).
  * Value 0 (**verified**, 0x541FD0): for every package, its own node list
    (+0x5C) and then each object in its per-message table (+0x6C count,
    +0x70 entries {message, —, count, objects}) that has a node for the
    message (0x5367F0), so every object answering it. The clone runs the
    pump over the one package it draws (`run_package`); responses 0, 1, 2,
    3 and 0x300 / 0x301 / 0x500 / 0x501 are modelled, the rest not.
  * The menus' INIT → HIDE objects are shown by 0xBDFD2DF9 (the Event
    Handler's CALL, Scripts above). The pad's accept 0x0C407210 and back
    0x14DD0705 come from 0x54E030 (**not routed** through the clone's
    pump); left is 0x9120409E and right 0xB5971BF1 (the dialog buttons'
    0x100 responses and 0x499690 / 0x499C00); the focus pair comes from
    0x52B3B0. The senders of 0x406415E3 and 0x936A6A7F are **open**.
  * Engage dialogs (**verified**, `DialogFocus`): UI_EngageShopDialog
    (descriptor 0x7F8148, factory 0x4CE6B0 → 0x4B5300 → 0x4998E0, handler
    0x499690) and UI_EngageRaceDialog (0x7F812C, 0x4E34A0 → 0x4CE710 →
    0x4B53C0, handler 0x499C00) focus Cancel_Button (0x63857DE0) on
    0x35F8620B; left focuses Accept_Button (0xD72F002A), right Cancel;
    accept acts on the focused button. The shop dialog's constructor sets
    only the message (0x4A04E7C), title (0x2EDBDE5) and Accept's label
    (0x54B17DC "Enter Shop" when the shop's +0x50 is set, else 0x67F55A89
    "Enter Garage"; after a failed timed run 0x85B940, "Retry" and
    0xC4DF3B44), so the package's third button (group B1000CC8, "Yes"
    B8A7C6CE, INIT → HIDE with no FADEIN response) stays hidden; the clone
    drew it (an "OK" between the two). The focused button's text answers
    Button_Highlight with HIGHLIGHT (unless still in INIT, [0x301 INIT]),
    whose 0 ms event highlights its box group.
  * The intro's goal shop (0x514FD0) posts flow event 0x11
    (0x5F41F0(0x11, …)), the same as the engage key (0x602BE0): in the
    world mode (handler 0x60B3B0) it stores 0x11 at +0x4C and switches to
    mode 6 (0x603640(6)), whose event 1 (0x606740, sub-switch on +0x4C at
    0x606771 → 0x606862) calls 0x498D00, which pushes UI_EngageShopDialog
    for the shop (0x86344C, set from 0x863464). So the dialog shows; the
    game does not go straight into the shop.
  * Shop drive-in / drive-out and the shop doors (`shop_scene`,
    `formats::world_anim`, `formats::eagl`; **verified** unless marked):
    * The dialog's handler 0x499690: Cancel_Button (0x63857DE0) posts flow
      event 8 (0x5F4250(1)); Accept_Button (0xD72F002A) runs 0x499640 →
      0x499470: 0x54CB00 enters the shop, then the NIS manager 0x827E88
      gets a shop entry (0x430570: +0x50 = 1, +0x54 = shop +0x38; 0x827ED4
      = marker 0x863444 +0x30, the shop's world trigger), 0x437270 picks
      the scene (0x436840 case 4: the table entries with +6 & 1 for an
      entry / & 2 for an exit, +5 & 0x40, region 4000, a random one of
      them) and loads it (0x435BE0, "NISMemory"), and flow event 2 plays
      it. With no scene 0x4995A0 opens the shop's front end at once
      (0x557E00; the car lot 0x528570). **Inferred**: the scene's end
      leads to 0x4995A0 too (its caller not traced). The game ships one
      scene of each: `NIS\Scene_ShopEnter1` / `Scene_ShopExit1`.
    * Scene bundle: chunk 0x80037020 → 0x37030 {+0x00 hash, +0x04 name,
      +0x18 type (0 intro NIS, 2 DynoEnter, 3 shop entry, 4 shop exit;
      0x43B000 reads it), +0x28 three equal lengths (140 / 120 frames)};
      0xE34010 banks with `ZPM_Car1_q` / `_s` / `_t` (the car) and three
      `XO_Spotlight…` channels (not drawn, **open**). Car keys: 71 / 61,
      exactly half the lengths; the clone plays two scene frames a key
      (**inferred** from that and from the door timing: so the car meets
      the door with its slats up).
    * Origin (0x43B000 → 0x431990): type 3 takes the marker named by the
      shop's hash (0x431930 with 0x827EDC), type 4 with career +0x713C
      clear the start marker 0x86501C (0x42FC60; **inferred**: the shop's
      on the way out; but 0x86501C is never written, so 0x42FC60 would
      read "ForwardStartLine" — 0x500AA0 is career +0x713C = 0 — and
      **open** is what +0x713C holds during a shop visit); x / y the marker's, z the ground (0x5E7E80), turned
      about z by the marker's heading (the ∓0x4000 of 0x431930 / 0x42FC60
      cancel in 0x431990). In TrackPosMarkersFreeRoam.bin the car lot's
      marker sits exactly on its door (the slats at scene x 0, y 0).
    * Cameras (**verified**, 2026-10-28): the scene banks hold none; a
      scene's camera is a camera track of the generic group (chunk
      0x8003B201 in InGameFreeRoam / InGameRace .bun) keyed by the scene
      header's hash (+0x00; ShopEnter1 0xE39FF628, ShopExit1 0x25F2E1C4,
      every NIS scene has one). While a scripted scene runs
      (0x4324D0(1)), 0x44FE80 takes that group (0x4307C0 → 0x442980),
      the track "Track %d" of 0x827B14 (the scene's +0x4C, set at
      0x439014) and the fraction scene +0x2C / +0x30; the shop tracks
      are type 0x12 entries in space 3, the scene frame +0xE0 (0x43B000:
      the 0x431990 origin, the car's). Clone: `Scene::camera_track` /
      `Scene::camera` (`track_camera::camera_in` with the scene frame)
      replace the chase view while the scene plays; test
      `the_shop_scenes_have_their_cameras`. **Inferred**: track 0 for
      the shop scenes (their groups hold only "Track 0"), the fraction
      as the clone's frame ÷ length, and that the director's gate
      (0x5FB060: +0x234, +0x24A, 0x82A3A4, 0x89E7B8 with state 3, a
      replay) lets the scene track show during the visit; after the
      scene a fresh chase view (0x5FA830 → 0x5F1FE0 builds a new mover).
      The height 0x4427C0 adds to space-3 points: see Scene and track
      camera lens (ported). **Open**: the fades.
    * The scene's end (**verified**, 2026-10-28): 0x43BB10 ends it once
      the clock +0x2C passes +0x34 (the header's +0x28 / 30 s, 0x437963):
      type 3 → 0x43B7A0 then 0x4995A0 (the shop's front end), type 4 →
      0x43B7A0, the player input's vtable +0x1C(0) and 0x532E20
      ("DDAY_EVENT_B"). 0x43B7A0 → 0x43B1F0 → 0x437A00 → 0x5F6D60 per
      scene car: the human controller back (0x5EBA40(1)) and its physics
      mover (0x5F46B0(0x5F4410())), with no placement and the velocity
      kept — during the scene the car is driven physically by mover 5
      (0x4309D0 → 0x5A40B0, teleported only when 1.1 m or 15° off its
      key, z its own). 0x439070 gives every racer whose camera mover is
      not type 1 a new chase mover (0x5FA830(1) → 0x5F1FE0(1, 0) and its
      vtable +0x38, 0x445B20: the eases snapped): no blend from the scene
      camera. ShopExit1's car ends 4.8 m out at ~0.7 m/s, so the near
      chase (4.7 m) puts the eye at the door and the far chase (5.8 m)
      behind it; the chase view's world push (Chase camera) keeps it out.
      Clone: the car keeps its last scene velocity; `Drive::chase_view`.
      **Observed** (IND_BODYSHOP_1): the original's car slows to
      0.77 m/s by count 119 and, on mode 1, stops within two frames (what
      stops it is **not traced**; the clone's rolls on).
    * Seen in the original (2026-10-29, scenes75: drove Rachel's 350Z
      from the airport to CC_CAR_LOT_1 under Wine and accepted the
      engage dialog, "Would you like to enter the Shop?", Cancel
      selected first): the drive-in shows no HUD at all (minimap, tach,
      bank, SMS icon hidden; the gate in 0x6000A0 / the HUD's draw is
      **not traced**; the clone hides the HUD while a shop scene runs),
      opens on a high top-down shot of the car on the lot's tiles, then
      low shots along the car and a wheel close-up, then the car lot's
      front end (~30 s at the emulator's half speed).
      **Fixed** (2026-10-29): the entry's "field of view" is a
      lens focal length (see **Scene and track camera lens**); the first
      shot now matches (compare/nis79/sbs-shopenter-first.png). Was:
      the clone's first drive-in shot sits much farther from
      the car than the original's (scenes75 sbs-shopenter-first.png).
      The track mover writes view 1's camera +0xC4 straight from the
      entry's lerped field-of-view curve (0x4462A0 → 0x75BED4 at
      0x454111), so 0x5C7ED0's view-1 widening (×1.1, h/w) applies,
      while `track_camera::camera_in` takes the value as a vertical angle
      in degrees; the curve's units (0x442B70 / 0x442C80) are not traced; the
      space-3 height (0x4427C0) is traced and ported (see Scene and track camera lens).
    * Event track `Scene_<name>_EvtTrk.txt` (0x432D00; 0x1C-byte events:
      +0x00 type, +0x04 time, +0x08 message 1 = DOOR_POSITION (0x430D00),
      +0x14 / +0x18 two floats): ShopEnter1 shuts the door at 0 and opens
      it from 21 over 140; ShopExit1 opens it at 0 and shuts it from 40
      over 70. 0x438CA0 sends message 0x827F40 ("WAM_NIS_DOOR_CONTROL_MSG")
      to every world-animation instance whose +0x20 or +0x24 is 0x827ED4;
      0x435660(position, duration): no duration (|d| < 1e-6) sets each
      entity controller's frame +0x64 to its length +0x60 (position 1) or
      0 at once, else plays (+0x10 = 1) at rate +0x40 = length ÷ duration;
      0x435140 advances 30 × rate × instance speed frames a second.
      **Inferred**: the direction (0x435660 sets none; the clone moves to
      the position's end).
    * Doors: world animations (chunks 0x37080 entities, 0x37090
      instances, 0x43A4A0; see `formats::world_anim`). L4RA has 23
      "DoorAnim" instances keyed by their shop's trigger (CC_CAR_LOT_1
      0xFFCC6A4B at −712, −873), each four slats `XO_DoorPieceA…D_1a_RB_00`
      with 91-frame channels (frame 0 shut, slats stacked 1.5 m apart; 90
      all rolled up at 6 m). The slats' own scenery instances are all
      hidden (flag 0x10), which is why the clone drew no doors: they are
      drawn only by the world animation. The clone adds a placement per
      slat (always-drawn section, **inferred**) posed instance × channel
      pose, shut until a scene's message.
    * Channels (`formats::eagl`; factory 0x6E2950, evaluate vtable +0x18):
      0x0F groups children (+0x08 count, +0x0A frames, +0x0C children);
      0x0A / 0x0B delta-coded floats / quaternions (0x6E5460 / 0x6E5570,
      0x6EFA20, 0x6EFE40); 0x15 u16 keys with u8 deltas in blocks plus
      constants (0x6E82D0, 0x6E78F0); linear between frames. Pose slots
      12–14 scale, 16–19 rotation, 20–22 translation (**inferred** from
      the data).
    * Self-playing world animations (`World::add_loops`, `step_loops`):
      0x43A4A0 makes each instance (nothing while 0x865018 or 0x827ADC
      is set; record +0x38..+0x3B a track-number window against 0x89E7A0
      % 100; a non-zero +0x30 adds flag 0x40 to +0x0C, 0x43A72C). Per
      entity 0x438860 makes a render node (0x638020), links it under its
      parent's (0x6345C0: node +0xE0 parent, the parent's child list
      +0x08, its count +0xD8), sets its local +0x50 to the entity's rest
      matrix and, unless the entity's hash is the tree's (the root), a
      model (0x61D060). It binds the channels the entity's +0x28 bits
      ask for (1 `_t`, 2 `_q`, 4 `_s`, 0x4350C0), each writing its
      compound header's frame count (+0x0A, 0x6E2110) as the length
      +0x60; with none the player is freed. The player (0x435010, rate
      +0x40 = 1): speed +0x5C = record +0x10 (0 → 1), start delay +0x68
      = +0x14 (flag 0x80), pause +0x6C = +0x18 (flag 0x100), flags +0x34
      |= record +0x0C (0 → 0x28); 0x40 takes the range +0x1C / +0x1E;
      0x200 sets the pause to r × ½ × length ÷ 30 ÷ speed and 0x400 the
      frame to r × length, where r = 0x43C170 on a fresh seed 0x69BABE69
      each time, (0x69BABE69 mod 0x7FFFFFFF) × 4.656613e‑10 ≈ 0.826; a
      record with +0x20 / +0x24 (the doors) drops 0x08 and adds 0x810.
      The root's local is the instance matrix (0x43A7A7). Playing (+0x10
      = 1) when +0x20 = 0 and +0x3C = 0.
      Update 0x435980 → 0x435140, 30 × rate × speed frames a second:
      the start delay, then the pause while the frame is short of the
      end (the overshoot carries over, fmod 0x43C240); 0x08 forward,
      with 0x1000 backward; 0x20 without 0x800 wraps by the span and
      restarts the pause, else the frame stops at the end; neither 0x08
      nor 0x10 → 0x08; 0x08 with 0x10 drops 0x10; 0x10 alone goes there
      and back (+0x08), 0x800 stopping it (+0x10 = 2) at either end and
      the pause restarting at the start. Then, unless 0x2000, only when
      the node's box passes 0x48CE10 (and no 0x827EEC / 0x827EC8 /
      0x827E98), the channels are evaluated (0x4316B0: `_q`, `_s`, `_t`,
      then 0x6E3B10) and bone 1's matrix becomes the node's local
      (0x40EFE0, dirtying it and its children, 0x6147F0). A node's world
      (0x60BB20) is local × parent's world (0x43C990 → 0x4013F0,
      row-major), the root's its local.
      L4RA has 22 such instances: the three vista searchlights
      `XO_VISTA_SPOTLIGHT_1A_DW_00` (beams 986 m tall, speed 0.025, flags
      0x28, at (−459, −470), (−232, −448), (81, −448)), a bridge
      searchlight, six jumbo planes, the tower cranes, the graffiti trains
      and the pub's sails. As with the doors their entities' own scenery
      instances are hidden (0x10, in no scenery group).
      The pose (0x4316B0 → 0x6E3B10, `Animation::bone_pose`): the
      buffer starts as the "ROOT" skeleton's (list 0x827C74, found by
      name 0x434DC0; 0x6E3C70 copies each 0x70-byte bone's scale +0x10,
      quaternion +0x20, translation +0x30). Read from the running
      original: two bones, both scale 1, rotation identity, translation
      0, bone 1 under bone 0. Channels write floats at their slots (12
      a bone: scale 0–2, quaternion 4–7, translation 8–10); each bone is
      S · R(q) · T (0x6E2D60, the quaternion not normalised) times its
      parent's; the node takes bone 1. So a missing channel keeps the
      skeleton's identity, not the entity's rest matrix (which is the
      node's local only until the first update): the vista
      searchlight 0x49C5218F (at −459, −470; only `_q`, slots 16–19)
      turns about the instance's origin at scale 1; the other two
      (0x49C52190 / 91) have all three channels on bone 0 (slots 0–10).
      The clone had posed missing channels from the rest matrix (that
      beam scaled up to ×1.4 and 23 m off).
      **Inferred**: samples past the last key are the last key; dt is
      the frame's seconds (the caller of 0x435980 is not traced); the
      bone product order (0x811E34, not traced; with the shipped data
      one of the two bones is always identity). The track window
      (`world::track_window_admits`, **verified** 0x43A4C1…0x43A526:
      r = 0x89E7A0 % 100 in [+0x39, +0x38] or [+0x3B, +0x3A]; L4RB's
      instances 3, 5, 7 and 10 set it, 20–29 or 0: free roam and drag
      tracks 4221–4223): the world is loaded before its track is known,
      so the windowed instances are loaded inactive with hidden
      placements and `World::set_track` (from the drive-in, with the
      barriers) makes them for an admitting track — shown, played, heard
      (`anim_points` skips inactive ones) — keyed by `World::anim_track`
. **Not ported**: the creation's generator draws (flags 0x400
      / 0x200, 0x43A5EE…0x43A60C). The
      doors' length is the frame count, 91 (0x4350C0 → 0x6E2110; fixed
      2026-10-31, reports: the clone had used frames − 1).
    * **Audio update (2026-11-03)**: ShopEnter1's authored CAR_AUDIO_PATTERN 3
      now drives the sound Physics context through the existing engine chain.
      Its 104 quarter-step updates and idle decay match a natural native
      capture; see `docs/evidence/nis-audio-pattern`. The clone retains the
      captured 60 Hz duration independently of render rate. Other patterns
      (including ShopExit pattern 4), scene audio gates and fades remain open.
      DynoEnter is implemented; the historical DynoExit gap is separate from
      this bounded audio change.
      **Update (2026-11-04)**: all banks 1–15, native event order, slot
      binding, scripted gates and the vanilla 30-updates-a-second scene
      cadence (the 60 above was WidescreenFix's HighFPSCutscenes) — see
      `docs/original/nis-audio-patterns.md`.
* **Layout** (**verified**, `frontend::layout`) — the renderer walks the
  groups (0x52C5E0; a group with alpha 0 skips its children), gives each
  group a context (0x51B7F0) and queues each object (0x51A970: alpha 0 is
  not queued) for its type's draw (0x5370B0: 1 image, 2 string, 7 movie,
  9 coloured image).
  * Transform (0x51B0B0, row vectors): v · S(size) · T(−pivot) · R(quat)
    · T(pivot) · T(position), the pivot and rotation only when the
    quaternion is not (0, 0, 0, 1) (0x4FD980 builds R); then · the
    parent group's world matrix. The canvas is that plus (320, 240)
    (0x536F70), so (0, 0) is the centre of the 640 × 480 canvas, which
    covers the whole window with x and y scaled apart (**verified**: the
    cursor maps to it as (x − left) · 640 / client width, (y − top) ·
    480 / client height, 0x5BF760, and a projected screen point as x /
    width · 640, y / height · 480, 0x4EC34C), so on a wide window the
    front end stretches (`render::ui`, the clone's "Stretched" aspect; its
    default keeps each element at its 4:3 size, centred or moved to its screen edge, see Display modes and aspect). A
    pivot is the point a rotation turns about (Minimap_North_indicator
    sits 72 above the map's centre with pivot (0, 72): it circles it).
  * Per resolution (**verified**, FE_DrawQuad 0x5C5030): each canvas
    vertex goes to clip space as x · 0.003125 − 1 and −(y · 0.0041666 −
    1) (constants 0x79AC18 = 1/320, 0x79AC1C = 1/240, 0x784250 = 1.0;
    0x5C5083..0x5C51E6), the line drawer 0x50DCD0 the same. Neither reads
    the window or back-buffer size, so the menus and the HUD have one
    layout in 640 × 480 units for every resolution and aspect, stretched
    over the whole target; the only size-dependent steps are the
    cursor's mapping (0x5BF760) and a projected world point's
    (0x4EC34C), both into the same canvas. No package or font is chosen
    by resolution (the strings 1024x768, 1280x960, 1280x1024 are the
    options' mode names).
  * Colour: per channel (own × parent + 0x80) >> 8 on the 0..255
    integers.
  * Order: an object goes to bucket ftol(its z + the parent's world z)
    clamped to 0..255 (0x51B850); buckets draw from 255 down (0x537249),
    each in fill order (**verified**): the traversal (0x536720 → 0x52C5E0)
    queues each object (0x51A970) as {object, key} at renderer +0x114
    (count +0x110), the key the pivot plus position through its view
    matrix, z; it skips alpha 0 and keys not above 0.0 (0x7D7D3C). The
    queue is sorted by the key (0x5114F0: stable LSD radix over its four
    bytes, ascending) and drawn from its end (0x50C2C0 → 0x51C670 →
    0x51B850, which appends to the bucket's list). So within a bucket the
    larger key draws first and equal keys draw in reverse queue order.
    The clone (`layout`): bucket, then key, then reverse depth-first
    order. The view matrix is the identity (**verified**: 0x536530 passes
    0x51A970 the matrix the manager's slot +0x3C fills, vtable 0x79BCE0 →
    0x50C490 → 0x43C780). The key matrix (**verified**, 0x52C5E0): the
    traversal hands a top-level group the identity and the view
    (0x536762 / 0x53675D) and a top-level object the view alone; a group
    builds its quaternion's matrix (0x504A50) with the translation row
    pivot + position + (−pivot through it) (0x52C68E), times the parent's
    (0x5187A0) — **no size**, unlike the draw transform — and hands that
    to its child groups and that × view to its objects (0x52C738). So the
    key is the z of pivot plus position through the groups' size-free
    matrices (`key_matrix`), and the clone applies the cut (keys ≤ 0 are
    not drawn; a NaN would pass). **Verified** (0x52C5E0): the queue order is
    depth first in each list's order: the traversal walks a group's child
    list (+0x64, next at +4) in turn, recurses into a kind-5 child where it
    stands and queues each other child (0x51A970) as it reaches it. Quick
    Race's highlight glow
    (DF422345, z −83 under its group's 80) is cut in its INIT state; the
    game draws it because INIT is zero-length and chains to the pulse
    0x5A8E4EBE, which puts it at z 16 and loops (chains to itself). The
    clone follows zero-length chains natively, the menus' INIT → HIDE
    objects coming back through the package's FADEIN messages (Scripts,
    "Screen in"); see the stand-in there for objects no message reaches.
    With the cut, LAN's progress cap 36D4B782 (z −15) no longer draws.
  * Images: the quad ±0.5 through the matrix (so centred on the position,
    size wide), texture corners from the state (0x536A60 → 0x51A1C0).
  * Strings (0x536F70 → 0x51C110): in font pixels through the matrix
    (size scales them). Lines step by ftol(`Sl` × the font's leading
    scale) + the font's line height; the block height is lines × step for
    `Sj` & 5, else the steps between lines plus one line height
    (0x50BF40). x starts at −width / 2 (`Sj` 1), −width (2) or 0; y at
    −height / 2 (4), −height (8) or 0. Each glyph (0x51BFC0) is
    max(width, 4) × height at the pen + its x offset, y + its y offset +
    the font's y offset, over its texture cell widened by one texel all
    round; the pen adds the kerning after the previous character
    (0x6F45F0) then the advance; widths (0x50BCE0) count the same.
    Fonts by name hash (0x51BDC0, table 0x7F918C): the Korean and Chinese
    Conduit fonts take a y offset of 18–23 and leading scale 2 or 1; the
    rest 0 and 1. Font 0x9583AA1A is drawn × 1.6 when 0x7F70D0 is 8 or 9
    (not modelled: English only).
    The font a string draws in (verified): 0x50C300 cuts the package
    resource's name to its base name (0x50C250: after the last `\` or
    `/`, before the last `.`), upper-cases it and stores its bin hash;
    0x51BD70 finds the loaded font of that hash, else follows the one
    alias pair at 0x7F9184 (0x9583AA1A → 0x5B9D88B9, ConduitMdITC_TT21i),
    else returns NULL, and the string draw (0x536F70 → 0x53709A) and the
    measures (0x51D0DB … 0x51D8F2) then skip the string. The four
    `Font_ImpactHud` labels of HUD_Drift and HUD_Short_Track name a font
    no bundle ships, so they are never drawn (`Assets::font_for`).
  * Text is the label's string unless flag 2 (then `St`, 0x536F70).
  * **Open** (seen in `game --shot` main, quickrace, options_display,
    carlot, parts, crib, pause, results; `NFSU2_SHOT_MS` sets the front
    end's time and `NFSU2_SHOT_SIZE=WxH` the target): UI_QRTrackSelect's
    track picture is not drawn (the slot's white template now follows its
    scripts and no longer shows; the screen's code that sets the track's
    image is not traced); faint dark specks at glyph
    cell edges (the widened cells sampling neighbouring glyphs) are not
    compared with the game's sampler state.
  * The HUD's position row ("1st /4", RacePositionNumber `Sj` 0xA and
    RacePositionNth / NumberRacers `Sj` 8 at −224 + 59) is bottom aligned
    over the lap row (LapText / LapNumber / NumberLaps `Sj` 0 at −158.25 −
    3), which hangs from just below it.
* **Resources** — textures by `bin_hash` of the upper-case name without
  extension (TPKs in FrontB, GlobalB, GLOBALA, LANGUAGES/LanguageTextures);
  fonts likewise (`ConduitMdITC_TT21i` is in GLOBALA, the rest in
  LANGUAGES/English.bin). The untextured image's request hash is
  `bin_hash("(UNTEXTURED SIMPLE IMAGE)")`.
* **Strings** (LANGUAGES/English.bin, 0x39000) — u32 16, u32 count, u32
  table offset, u32 text offset; a 256-entry character map (applied by
  0x50D3C0, see "Native front-end fixes, part 1"); the table
  {u32 hash, u32 offset} sorted by hash; NUL-terminated Latin-1 text.
* **Fonts** (0x30201; **verified** against the renderer, 0x51BDC0,
  0x51BFC0, 0x6F45F0) — name at +0, texture name at +0x100, `FNTF` at
  +0x200: +0x0A glyph count, +0x0C flags (0x40000: 16-byte glyphs, else
  12), +0x14 glyph table, +0x18 kerning table, +0x24 i32 line height,
  +0x40 / +0x44 1 / texture size (+0x12 is not the line height: 16
  against 20 for ConduitMdITC_TT14i). Glyph (16 bytes): u16 code, u8
  width, u8 height, u16 x, u16 y, u8 —, i8 x offset, i8 y offset, u8
  kerning count, u16 first kerning pair, i16 advance. Kerning pair n at
  table + 4 + 4n: u16 previous character, i8 adjust, u8 —.
* **Menus** — every menu is an IconScrollerMenu (vtable 0x79C0FC) in its
  own package; each option is `0x51F680(icon texture hash, label hash, 0)`
  with an action vtable whose second entry handles the accept message
  0x0C407210. The option sets and their conditions:
  * UI_Main (0x4AEE30): Career → UI_StartCareer (0x4AED80), Quick Race →
    UI_QRModeSelect (0x4FC050, game mode 2), Online, LAN, Customize (mode
    0x20), Profile Manager → UI_ProfileManager, Options → UI_OptionsMain
    (mode 0x10).
  * UI_StartCareer (0x4AF110, title set to "Career"): with a saved career
    (career +0x20, 0x502FE0) whose intro movie has played (0x86347C,
    career +0x873C, saved) "Resume Career" + "Start New Career", saved but
    not yet played "Start New Career" acting as resume (the resume vtable
    0x790CC0, which "Resume Career" has too; the new career's is
    0x790CB8), without one "Start New Career"; always "Load Game"
    (0x790CC8). **Verified** 2026-10-30. Resume (0x495A30) sets game mode
    1 and goes on from the career's state.
  * UI_CareerCrib (0x4AF820): Return to Explore Mode (Yes/No "Return to
    Explore Mode?"), World Map (only when 0x862838 > 1), Car Select →
    UI_CareerCarSelect, Change Parts, Performance Tuning / Dyno (Yes/No
    "Do you wish to Performance Tune / Dyno your car?"), View Rewards →
    UI_CribRewardOptionsMain, Save, Back to Main Menu (Yes/No "Are you
    sure you want to quit to the main menu?").
  * UI_pause in explore (0x4C2900) and in a race (0x4C2BD0), UI_postrace
    (0x4BEE10): see `screens::explore_pause_menu`, `race_pause_menu`,
    `post_race_menu` for the exact sets and conditions (World Map only for
    races started from it, 0x85BA67; Restart; Return to Explore Mode in a
    career; Options; Quit to Main Menu).
* **Garage scene** (GarageMain.fng; `garage.rs`, `draw_garage`): the 3D
  scene the front-end menus stand on. **Verified** unless marked.
  * Who pushes it: UI_Main (descriptor 0x7F7DC8, factory 0x4ED6B0 →
    constructor 0x4DF140) sets game mode 2, builds its icons (0x4AEE30),
    copies a ride into the FE car 0x8371B0 (0x413440 at 0x4DF264) and
    pushes GarageMain.fng when it is not up (0x555F60), else hands the
    garage the new car (0x4D8C70). The ride: the preset DDAY_PLAYER_CAR
    (0x637280) while the byte 0x86347D (career +0x873D) is clear, else
    0x515D00(0, ride, 0). **Inferred**: +0x873D (set by 0x500BD0 and
    0x541550, cleared by the new career 0x532840) means the player has a
    car of their own and 0x515D00 gives it; the clone shows the profile's
    car when it has one, else the preset. 0x8371B0 itself starts empty
    (static constructor 0x40C910: 0x610280(−1, …)).
  * The empty turntable after boot (gaps row 7): under Wine (live
    reads) 0x8371B0 holds the preset's type (0x16) as soon as UI_Main is
    up and the car shows 10–20 s later; wined3d runs the game clock at
    ~13 % of real time, so the original's asynchronous car load is slow
    there, not empty by design. **Inferred**: the load path after
    0x4D8C70 is not traced.
  * Platform: the front end's load (0x57FE85) opens SHOWROOM.BIN when its
    first screen (0x8384C8) is UI_Main.fng or none; otherwise a visited
    shop's scene file (0x863444 and 0x86344C set, `FRONTEND\PLATFORMS\%s`
    with the shop record's +0x40), else CRIB01.BIN once (0x8635B9, set by
    0x53CCD0 and 0x55A7D8), else SHOWROOM. 0x4A7180(from, to) swaps them
    later (crib 1 → 4 at 0x4AFAD0, back 4 → 1 at 0x4AF464). A platform file
    holds a solid list (DISC01–03, flag 0x40; BACKDROP; BACKDROP_FLOOR,
    flag 0x40; PLATFORMBASE), its texture pack and an environment pack
    XPOS … ZNEG, which 0x5B9920 copies into the static cube 0x87088C
    (called by 0x4A7180 after its load; **inferred** for the first load,
    whose caller is not traced). The clone gives the car that cube.
    Without the faces (**verified** in the code): 0x5BCAE0
    binds 0x87088C without a null test; it exists whenever 0x870CB8 ≠ 0
    (0x5BA4F4: 256², A8R8G8B8, managed), and a missing face copies
    DEFAULTALPHA's 1024 zero bytes over the face's first row only. So the
    cube is the last platform's (top rows zeroed), or its creation
    contents (black, **likely**) before any. A car renderer now starts
    black (`car::start_envmap`; the clone had a made-up night-sky
    gradient). `garage_car_renderer` keeps the front end's one cube
    across platforms (`fe_cube_texels`): a platform without its six faces
    leaves the previous platform's cube with each face's first 1024 bytes
    zeroed (`car::envmap_without_faces`; the shop fixes). Every shipped platform has
    all six faces (`showroom_has_the_scene_models_and_an_envmap`).
  * Model handles: 0x493600(hash) on GarageMain +0x70 PLATFORMBASE, +0x88
    BACKDROP, +0xA0 / +0xB8 / +0xD0 DISC01–03, +0xE8 LOCK (string
    0x797C18; not in Showroom.BIN), +0x100 BACKDROP_FLOOR: it stores the
    hash, links the handle into the list 0x82DBC0 and resolves the model
    (0x48DF90, 0x48D780).
  * Draw (0x4A7330(msg, mirrored), from the message handler 0x4D8D20:
    0x32C72D8F → view +0x220 = 1, 0xFC946BFA → view 4, the road
    reflection; nothing while +0x1C8 is set). Matrices are row-vector
    (0x5BAE70 = src × Rz, Rz = [cos, sin; −sin, cos] from 0x5BACA0, the
    angle `(int)(deg·65536)/360`). View 1: BACKDROP and BACKDROP_FLOOR at
    the identity; disc i at `Rz(car) · T(disc i) · Rz(platform)` (the car
    angle +0x274 only on the car's disc +0x144, which also gets the light
    context 0x48F600 from the light setup +0x6C); LOCK on the disc of each
    car slot whose flag (+0x5C, +0x64) is set, 0.15 up. View 4: BACKDROP
    and `Rz(platform)` PLATFORMBASE, both with m22 = −1, so PLATFORMBASE is
    seen only in the worldreflect floor and discs. Disc centres (0x7F45AC):
    (0, 5.4, 0), (4.7, −2.7, 0), (−4.7, −2.7, 0). The clone draws the
    models with the world renderer, the passes picked by instance flags.
  * **Garage scenes: no weather** (verified; checked under Wine). The
    frame function 0x5CAAE0 updates and draws the rain only in game mode
    6: `cmp [0x8654A4], 6; jne 0x5CB4AB` at 0x5CB40D skips the call of the
    weather update 0x61F130 at 0x5CB47B (its only caller), which draws the
    drops (0x613A80 at 0x61F1E7), applies the rain tint (slot 2, 0x5D75F0
    at 0x61F1DB), steps the lens drops (0x614100 at 0x61F211) and moves
    the drops (0x61E710 at 0x61F3E8); and 0x5CB7EE sets the lens drops'
    draw flag 0x8707D2 (read at 0x5CB9A9 before 0x5C7AC0) only in mode 6.
    The front end, where the main menu's and the shops' garage scenes
    are drawn, is mode 3, so none of it runs there whatever the weather
    object holds. Under Wine (main menu, 0x8654A4 = 3, Rain 0x870D10 = 1,
    the frame period 0x865558 written to 1/60): with view 1's weather
    (`*(0x832E50 + 0x64)`) amount +0x1F0 and target +0x5094 written to
    1.0 and the debug override 0x8A1D38 to 1, the wetness +0x509C stayed
    0 for 6 s (the update would raise it by 0.05 a frame) and the garage
    picture changed no more than between two dry captures (RMSE 0.0351
    vs 0.0350, the menu's animation): no drops, no tint. The clone's
    garage renderer (`render_garage`) is never given a weather
    (`WorldRenderer::set_weather` is the in-game main view's), and a
    renderer without one draws no weather effects (`plan.rs`, `fx_view`):
    no rain drops, splashes, haze, spray or wet-road trails, even when a
    drive left a shower published (`wet_car::scene`). The FE car's wet
    film was already off (`lighting.is_frontend()`), and the post's rain
    tint, lens drops and the rain's volumetric terms run only in game
    frames. Test: `tests::garage_weather::garage_draws_no_rain_left_by_a_drive`.
  * The car (0x4A6670(car, disc), per car from 0x4D82D0): x, y the disc
    centre turned by the platform angle 0x8363F0; turned by +0x274 alone;
    each tyre's contact at 0.15 (0x7F45A8) plus the hydraulics height
    (0x4A6030, parts slot 0xA1; 0 without), the body levelled on the four
    contacts and lifted by their mean `radius − centre z` (wheel data from
    car +0x20 → 0x5B5360; **inferred**: the car type's wheel table, which
    the clone uses, as 0x5B4FD0 is not traced); wheels 0 and 1 turned by
    0x7F45A4 about +z.
  * Camera (0x44CC30 on view 1; 0x445500 with record +0x55 = 0 of the
    0x30-byte table 0x7F608C): +0x00 car angle 305.77°, +0x04 front-wheel
    steer (0x7F45A4) 30°, +0x08 field of view 32°, +0x0C target (0, 4,
    1.3), +0x18 eye (0, −4, 1.3997), then two zeros (camera +0xB4, +0xB8);
    record 1 (SUVs, car type +0x88A): 26°, 37°, target z 1.4996. The update
    0x44CE00 builds a look-at from eye to target with z up (0x444DE0) and
    writes the field of view to camera +0xC4 as `(int)(fov·65536)/360`, so
    view 1's projection (0x5C7ED0, `Camera::game_fov`) applies. Camera
    state 1 (the constructor's) moves the eye only on the mouse's lift
    rate (y −6.5..−0.1, z 1.2..5.2; see **Turning the car** below);
    state 0 blends to a new record.
  * Update (0x4D82D0, on message 0xC98356BA and at the end of the
    constructor): the car turns only on the mouse's turn rate (+0x248,
    0x4A5D90; **Turning the car** below) or in the idle camera tour (after 300 s of 0x8651A8 − 0x837AA0 times
    0.00025 against 0x7F45D4; shots from 0x7F4608, the next when 0x7FB708
    has moved on by the integer 1200 at 0x7F45D0); a car change turns the
    platform by ±120° over 0.5 s and, when it ends, picks camera record 1
    for an SUV (0x4533B0). 0x458520(0x49) / 0x458560(0x49) run as the car
    angle changes / holds (**inferred**: a turntable sound's start and
    stop).
  * Which scene and menus (`Game::frontend_scene`, `Game::garage_request`;
    verified unless marked): UI_CareerCrib (0x4AF820) checks GarageMain.fng
    is up (0x52CF70) and, when CRIB01.BIN is not loaded (0x577EC0), swaps
    SHOWROOM for CRIB01 (0x51BD10, 0x4A7180(1, 4)), whose light setup it
    also gives the cars (render info +0x904, GarageMain +0x6C). In a shop
    the front end loads the shop record's scene (+0x40; CarLot01,
    PartsShop01, PaintShop01, PerfShop01, AudioShop01, MegaloShop01;
    0x57FE85). The shops' screens find GarageMain through 0x4A7830
    (GarageMain.fng, else GarageMain_2P's car 0/1) to move its view, so it
    is up under them; the car lots and the shops' category icon menus
    between the crib and those screens are drawn over it too
    (**inferred**: nothing traced pops it on the way). The clone draws the
    garage under UI_Main, UI_QRModeSelect, the crib, the car lots (the
    selected car, stock), the customize categories, the parts, rims and
    paint browsers, UI_ChooseRimBrand, UI_ChoosePaintCategory,
    UI_ChooseVinylLayer, the performance category / package menus and
    UI_PerformanceBrandSelect, with the player's car (the profile's, else
    the DDAY preset). Leaving for the city clears the visited shop
    (0x541B22 / 0x500B40 zero 0x863444 and 0x86344C; **inferred** where).
  * Part views (`garage::SLOT_SHOTS`, `slot_shot`, `View`): 0x4C2100(slot,
    part, player, flag) → 0x4C1D40 (GarageMain, `[esp+…]` slot and part) →
    0x4A55E0 → 0x4A5510(slot, class +0x154, +0x158, part) picks a record
    of 0x3C bytes from 0x7F4608 (2 × 0x31: cars, SUVs; +0x00 tour flag,
    +0x04 name, +0x08 slot, +0x0C car angle, +0x10 steer, +0x14 fov,
    +0x18 target, +0x24 eye, +0x30/+0x34 zero, +0x38 a float 20 or 170
    kept at GarageMain +0x23C+0x30, **not traced**). Slots 0x22–0x2E use
    0x22's (Audio); 0x40–0x43 the vinyl table 0x7F5D40 by the vinyl's group
    (part +5 & 0x1F: 0x14 → record 0, 0x16–0x1B → 1–6, else slot 0x40's);
    for an SUV with +0x158 set (car type +0x840 = 8, set when a car change
    ends) slots 0x0D, 0x9B and 0x44 use 0x7F5D00 ("Engine"). No record:
    the view stays. The camera blends to the record (0x4532B0: camera
    +0x30 = 1, rate +0x150 = 0.3, or 10 during the idle tour); the car
    turns the short way from +0x274 to the record's angle (+0x254) and the
    steer 0x7F45A4 to its steer (+0x25C) over the camera's +0x14C seconds
    (0x4D82D0 while camera +0x30 is set). 0x4C2140(player, flag) →
    0x4C1FA0 → 0x453320(flag) blends back to the base camera (camera
    +0x40 …, the constructor's record) and leaves the car's angle and
    steer. **Blend (verified, ported: `garage::View::at`, `set_slot`,
    `reset`, `blend_duration`, `ease`)**: 0x451200 keeps the camera's
    current field of view, target, eye (+0x80 / +0x90 / +0xA0 → +0xC0 /
    +0xD0 / +0xE0) and takes the record's (+0x100 / +0x110 / +0x120,
    record +0x30 / +0x34 → +0x130 / +0x134), then 0x44D190(p10, p11) sets
    the duration +0x14C = 0.0055 (0x78710C) × the largest of |p10 − p11|
    (0x4A55E0 passes +0x274 and the record's angle; 0x453320 0, 0),
    |Δfov| ÷ (50 dt), |Δ+0x130|, |Δ+0x134| ÷ (50 dt) and |Δtarget|,
    |Δeye| ÷ (2.2 dt) (0x4455D0; 0 for unchanged ones), dt the frame time
    0x865194, at least +0x150 (0.3 for a part view; 2.5 back to the base,
    0x453320). 0x44CE00 (state 0) adds dt to +0x148, f = +0x148 ÷ +0x14C
    (1 at the end or with no duration), s = (sin(f·180° − 90°) + 1) ÷ 2
    (a binary angle, 0x75BED4 / 0x43C2B0), and lerps field of view,
    target, eye (and +0xB0 / +0xB4) by s; state 2 when done. The car
    (0x4C1D40: +0x124 = camera +0x14C; +0x11C the angle, +0x120 the
    shorter of the up and down distances from the angle mod 360 to the
    record's, a tie going down, +0x138 the way; +0x12C / +0x130 steer and
    its difference; the mouse's rates +0x248 / +0x24C zeroed) turns
    linearly: f = +0x128 ÷ +0x124, angle = +0x11C ± f · +0x120, steer =
    +0x12C + f · +0x130 (0x4D8A85…0x4D8B09), only while camera +0x30 is
    set, so a reset (0x453320 clears it) stops the turn where it is.
    `NFSU2_SHOT_MS=250 game --shot parts_hood` shows a view mid-blend. Slot 0x44 (engine paint) also opens the hood (0x61F830 on the
    FE car, GarageMain +0x160/+0x15C), closed again on the next view.
    Callers: the parts list screen (0x553320, MINI_MASTER / PARTS_ICON)
    → 0x53BA10 with its category's slot (+0x110 +0x48); UI_Paint's
    swatch scrolls (0x55CAD0, 0x55D670) with the paint slot's (0x5606F0
    and 0x560930 the same way, their screen not identified); UI_Rims_Browser (0x55F350, 0x55F900, 0x55FCF0) and
    others (0x560D40, 0x5612B0, 0x567A60, 0x569680) with 0x1D (Front
    Wheel) and the rim; 0x50E580(slot, part) at 0x55EEE8 / 0x5676B8; UI_DecalsOverlay
    0x568460 with 0x50E5A0's slot; the vinyl screen 0x5664A0. Resets
    (0x4C2140(0, 1), all callers): 0x554CFC (UI_ChooseVinylLayer's
    constructor 0x554CB0, vtable 0x79CC3C), 0x55F06D
    (UI_PartsBrowser_LeaveCategory), 0x55FE8D (RimsBrowser_LeaveSizes,
    after its own 0x4C2100(0x1D)), 0x5606C1 (UI_ICEMAIN_LeaveList),
    0x56785F (UI_GenericParts_Browser on 0xE1FDE1D1 when 0x83898C ≠ 0 or
    +0x1D3 / +0x1D4 / +0x1D5 / +0x1D7), 0x568A44 (UI_ICEMAIN on
    0x8ACAA8DF), 0x568D0D (UI_ICEMAIN with 0x86461A and 0x83898C or
    0x838998 set), 0x56A814 (the neon list 0x56A310), 0x56A903
    (UI_CustomNeonMain on 0x8ACAA8DF), 0x56AAD5 (UI_ChooseVinylLayer's
    tick after 0xE1FDE1D1 with +0x15B) and 0x56B8D0 (UI_DecalMain on
    0xE1FDE1D1 with +0x170). None of them, nor anything else, sets the
    car's angle +0x274 or the steer 0x7F45A4 back (their only writers are
    the constructor 0x4EAF1B, the part views / tour 0x4D8A74…0x4D8B6E and
    the magazine pose 0x4C209C), and UI_Paint, the car selects, the
    category menus and the performance screens have no reset. **Wine
    (the finished-career save, main-menu Customize, full speed)**: back from a body
    part's list and from the rim sizes the camera blends to the base view
    (2.5 s) while the car stays as the part view left it up to Customize
    Car Select (side-on, 0x7F45A4 = −24 after the rims); back from a
    paint swatch the paint camera itself stays, up to Car Select (steer
    16); after Performance Tuning / Dyno the front end's new GarageMain
    shows the constructor's view (steer 30). The clone
    (`GarageViewPhase`, `Game::garage_view_returns`; **fixed 2026-11-01**,
    it kept a rims view's camera at Car Select and reset the camera on
    entering any parts strip): a parts list in UI_GenericParts_Browser and
    a UI_Paint slot's swatches take the slot's view, UI_Rims_Browser 0x1D's
    for each style shown; the returns above blend back to the base view
    (a list closed to its strip in UI_GenericParts_Browser,
    UI_Rims_Browser, UI_ICEMAIN, UI_CustomNeonMain; UI_GenericParts_Browser
    left outside a career shop or from carbon fibre / vinyls; UI_ICEMAIN
    left outside a career shop; UI_ChooseVinylLayer entered); every other
    menu leaves the view as it is. **Open**: UI_DecalMain's and
    UI_ChooseVinylLayer's flagged resets (+0x170, +0x15B) and 0x86461A are
    not traced or ported. Selftest `selftest_garage_returns`. It blends as
    below. `--shot parts_bumper`, `parts_hood`, `paint_base`
    show part views; `crib`, `carlot`, `parts`, `perf`, `paint` the
    scenes (the shop shots set the visited shop of that kind).
  * Car change (ported, `garage::View::change_car`, `next_disc`;
    verified unless marked): Left / Right in a car lot (0x4E003E /
    0x4E0070 / 0x4E0099 → 0x4A7930 / 0x4A7900 / 0x4A7960) set GarageMain
    +0x148 = 0 for Right (0xB5971BF1), 1 else; 0x4D82D0 then starts state
    2: +0x124 = 0.5 s, +0x150 = the platform angle 0x8363F0, the next
    disc +0x140 from {1, 2, 0} (+0x148 = 0) or {2, 0, 1} by the current
    +0x144, and each frame 0x8363F0 = +0x150 ± f · 120 (0x78AF80; + for
    0), f = +0x128 ÷ +0x124 linear; at the end +0x144 = +0x140. GarageMain
    keeps two FE cars (+0x4C / +0x50, the shown one by +0x54): state 1
    waits for the loader ([+0x234]+0x2CD4) and only then starts the turn
    (0x4D861F…0x4D8679) and, in the same update (0x4D873A…0x4D87DF),
    flips +0x54, builds the loaded car into the other slot (0x63A660),
    puts it on the next disc (+0xB78 = +0x140) and shows it (+0xB64 = 1);
    the old car stays shown on its own disc (+0x144) and rides out with
    the platform until the turn ends (0x4D84CF: +0xB64 = 0). The disc
    0x4A7330 turns with the car's angle is +0x144 (0x4A74EF), the old
    car's during the turn; both cars use GarageMain's angle +0x274
    (0x4A68D2). Ported: `Game::lot_load` / `fe_car_swing` hold the turn
    (`GarageRequest::hold`) until the scene has the new car
    (`GarageScene::incoming`), which then goes on the next disc while the
    old one rides out (`GarageScene::outgoing`, `View::outgoing`); before,
    the turn started at the ask and the old car was moved to the new disc
    (gone from the front at once) until the new car's build landed
    mid-turn. Test `tests::garage_lot_turn` (build delays 0–3 s, 30–200
    Hz; the developer harness's NFSU2_DEV_CAR_BUILD_DELAY_MS gives every
    FE car build that latency). Which of Left / Right the clone's lot maps to the next car is
    **inferred** (Right = the next one). `NFSU2_SHOT_MS=250 game --shot
    carlot_next`.
  * **Fast switching** (verified; Wine at full speed, UI_QRCarSelect,
    0x8363F0 read every 30 ms): the lots never ask GarageMain for a car at
    the key. Each step stores the tick clock 0x8651A8 (crib 0x4B04C0 at
    +0xB8, UI_CareerCarLot 0x4AFFA0 at +0xE0, UI_QRCarSelect 0x4E2590 at
    +0xC4, which Customize's car select shares) and the screen's tick
    (0x4DFFEC, 0x4EE00D, 0x4FCD1D) hands the selected ride (0x511E00) to
    the loader (0x4D8EC0 → GarageMain 0x4D8C70 → 0x4D8030) once
    (now − it) · 0.00025 ≥ 0.3 (0x784268, 0x78758C), then clears it. The
    loader (GarageMain +0x234): 0x4D8030 cancels a load in flight (+0x2CD0,
    0x63AE10 on +0x2CC4) and starts the new one, unless the old car is
    still held by a swing (0x836408 with 0x83640C clear), when the ride is
    queued (+0x23D0, +0x2CD8 = 1; a later one overwrites it); 0x4D8180
    marks it loaded (+0x2CD4); 0x4D82D0 state 1 swaps it in (0x4D8220) and
    swings (state 2) when its car type differs (+0x2CE0, 0x4D8030: ride +0
    ≠ the shown one's), else goes straight to state 3; state 2's end
    loads the queued ride (state 1, 0x4D8030(+0x23D0)). So the old car
    stays until the new one is in, every swing turns a whole 120°, and
    the last car asked for ends in front: five Lefts 0.14 s apart gave one
    swing 0.53 s after the last key; Rights 0.45 s apart gave three
    swings back to back (0x8363F0 240 → 0 → 120 → 240). The clone did
    the swing at each key and restarted it from wherever the platform
    was, so fast keys left the car off the front disc (and a filter change
    could index a car of the new list); now `Game::garage_request` waits
    the 0.3 s on every lot, holds a car asked for during a swing until it
    ends (the last one wins) and keeps the shown car itself
    (`garage_crib_shown`), swings only for another car type (a crib
    placeholder always: 0x4A7930 / 0x4A7960), and `View::changing`
    reports a swing under way. The clone's car load is in the frame
    (`draw_garage`), so its state 1 lasts no time. Tests: the selftest's
    `selftest_lot_hammer` (Quick Race's and Customize's car selects and
    the D-Day car lot: 160 random steps 0–0.6 s apart, the car in front
    at every frame at rest, the selected car shown at the end) and
    `garage::tests::car_changes_wait_for_the_turn_so_the_car_ends_in_front`.
    **Open**: 0x4E8530 / 0x4F2E41 (vtable 0x797A38, a 0x4F9D90 subclass
    built by 0x4FB320) do the same on +0xC4; which screen it is was not
    found; a QR filter change's own load (0x4EED10) is not traced (the
    clone waits the 0.3 s for it too, **inferred**).
  * **A lot's first car swings too** (observed; internal notes):
    0x8363F0 at the boot's main menu (350Z) 120, at UI_QRCarSelect's first
    car (206) 240, back on the main menu 0 — every FE change of car type
    turns the platform +120°, the lot's constructor's car included. The
    clone had excluded the lots from that swing; `Game::garage_request`
    now swings their first car of another type as well.
  * **Turning the car** (**verified**; ported: `garage::View::drag_step`,
    `View::drag_input`, `DragInput`, `mouse_grid`; tests
    `garage::drag_tests`, `tests::garage_drag`). On PC only the mouse
    turns the car on the platform: hold the **middle button** and drag.
    - Input (0x4A5D90, GarageMain's tick 0xC98356BA after its update
      0x4D82D0): g_Mouse (0x8763D0, polled once a frame by 0x5BF870)
      keeps the cursor on a 640 × 480 grid of the whole client area
      (0x5BF760: GetCursorPos less the client origin, × 640 / width and
      × 480 / height, truncated) and its move since the last frame (+0x14
      / +0x18 = 0x8763E4 / 0x8763E8). While the middle button is held (+0x27 =
      0x8763F7) the turn rate +0x248 = 0.5 (0x784260) × dx and the lift
      rate +0x24C = 0.5 × dy; the idle tour stops where it stands
      (0x8363FC = 0) and its clock restarts (0x837AA0). The release
      (+0x29) zeroes the rates (+0x240 is the constructor's 1). The wheel
      (+0x1C) turns nothing: it (and the frame after it, 0x83640D) only
      stops the tour and restarts its clock, except with
      UI_GenericParts_Browser up. Nothing is read while the reward
      screens' flag 0x838578 is set (0x510C30: UI_MagazineSelect for
      magazines / DVDs, set by 0x53DDC0 / 0x53DE00, cleared by 0x53DE40;
      **not observed**: a fresh profile under Wine has no rewards). The
      test for GenericDialog.fng (zero the rates) matches no dialog the
      menus show (they are the sized GenericDialog_SMALL / _MED /
      _LARGE): under Wine the car turned under the quit and the
      profile-save dialogs.
    - Update (0x4D82D0, the next frame): unless a flagged part view
      locks it (+0x167, below), 0x4A5660 hands a changed rate to the
      camera (0x4454A0 / 0x4454E0: state 1 unless it blends; the lift
      into camera +0x144); any rate ends a part view (0x453320(1): back
      to the base view over at least 2.5 s, camera +0x30 cleared) and the
      tour (0x453380), and restarts the tour's clock. With no part view
      and no tour the car turns by `dt · 60 · +0x248` degrees (0x865194
      the frame time), kept within 0..360; the steer stays. The camera
      mover in state 1 (0x44CE00) moves the eye z by `dt · 60 · +0x24C ·
      0.1` every frame, clamped to 1.2..5.2 (y to −6.5..−0.1).
    - So: a drag to the right raises the car's angle (+0x274), to the
      left lowers it; down lifts the camera, up lowers it. At 60 fps a
      640-wide unit (2 px of a 1280-wide window) turns the car 0.5° and
      lifts the eye 0.05. The original scales this with its frame time:
      at its own 120 fps cap it is half (Wine, main menu: 100 units →
      25.3° at 1/120 s, 50.4° with 0x865558 at 1/60; 50 units up → eye z
      −1.27 / −2.5). The clone steps it on its 60 Hz ticks, so a drag
      turns the car by the same angle at 30, 60, 120 and 200 fps (the
      60 fps figure); the drawn frames between ticks turn it the short
      way across 360°. The turntable sound's pitch follows |+0x248|
      (0x7EC858, see the FE car turn in audio.md).
    - Where (Wine, a fresh profile; keyboard, mouse and a virtual Xbox
      360 pad): it turned on the main menu, Quick Race's mode, track,
      options and car select, the Car Shop's categories, the Body Shop's
      strip and parts list and the Profile Manager, and under the quit
      and save dialogs: everywhere GarageMain is up. The career's crib,
      car lots, shops and performance screens stand on the same
      GarageMain (**inferred** from the code; not reached under Wine).
      In a part view a drag returned the camera to the base view (camera
      +0x30 1 → 0, a 2.5 s blend) while the car turned at once. No key
      (arrows, WASD, Q / E, keypad, Home / End, Page Up / Down, Insert /
      Delete, `[` `]`), no pad input (both sticks, d-pad, buttons,
      triggers; the pad moved the menu) and neither the left nor the
      right button turned it.
    - The pad path that exists is a console one: GarageMain's +0x23C
      block registers 0x4A5740 for the JOY events 0x40–0x4A (0x7F45D8;
      JOY_EVENT_CARSEL_ORBIT_LEFT … _INOUT, names at 0x7FAD80: digital
      ±value / 255, analog (value − 128) / 128 into the same rates), but
      none of the PC's 83 input descriptors (0x7FDD10) emits them.
    - **Not ported**: the part-view lock +0x167 (0x4C2100's fourth
      argument, 1 from 0x50E580, 0x55CE10 and 0x5676D8: the mouse is
      ignored until that view's turn ends); the clone lets a drag end
      those views at once.
  * **Open**: in a
    car change the old car staying on its disc (GarageMain's two FE cars
    +0x4C / +0x50 by +0x54), the sounds (0x51DE40(5 / 4, 0x17)), the
    states 1 / 3 waiting on the car's loader (+0x2CD0 / +0x2CE0) and
    0x4D84E3's camera record switch for an SUV (car type +0x88A, 0x4533B0
    with 0x7F6088 + 0x30·class) are not ported, and it uses camera class
    0 and +0x158 = 0 even for an SUV; the
    part previews the shop screens fit on the FE car (0x55C180 / 0x55C200
    before each 0x4C2100) — ported in menus103 (`Game::preview_ride`); the views
    of the decal (0x568460 → 0x50E5A0), vinyl (0x5664A0, 0x567A60,
    0x569680), neon, audio (UI_ICEMAIN) and specialties screens, whose
    calls are listed but not mapped to the clone's menus; whether the
    dyno, tuning, rewards and UI_StartCareer screens keep the garage (the
    clone does not draw it there); a car lot's cars of a crib Car Select
    are shown stock; fog, near and far planes of the front-end view;
    GarageMain.fng's own FNG objects (CarName_P1, PlayerName_P1,
    Style_P1, 0x13C45E) are not drawn; the LOCK model's file. The DDAY
    preset's shared parts (Racinghart rims, spoiler) are drawn (see
    "Shared part models"); `--shot main_dday` shows it, `--shot main_kit`
    adds a roof scoop and post mirrors.
* **Icon scroller** (0x538950 update, 0x52F8D0 per option, 0x543AA0 /
  0x543850 construction with "OPTION_MASTER", "LOCK_MASTER",
  "BUTTON_MASTER", "option_" and width 480): options sit at OPTION_MASTER's
  y, spaced by the icon width plus a 40 gap; the selected one is full size,
  the others × 0.8, centred vertically; outside a 480-wide strip around
  the master they are hidden, and within 0.8 × the icon width of its ends
  faded (0x50D8D0). The scroll offset eases to −(selected position) along
  a cubic Hermite over 0.3 s (0x574E40). No wrap at the ends (0x51FA00).
* **Arrow blink** (verified unless marked; `screens::ArrowBlink`): the
  packages' left / right arrows (ScrollBar_Arrow_1 / _2, 0x444969FD / FE)
  carry a BLINK script (400 ms: colour to green, size 26 → 32 and back,
  chained to STATIC 0x7AB70D67; INIT looks as STATIC). A step runs it
  on the arrow that way, in two ways:
  - from 0 at every step (0x51CF80 flag 1): UI_QRCarSelect's step
    0x4E2590 (0x5426D0(package, "ScrollBar_Arrow_1" / "_2", "Blink", 1)),
    also Customize's car select; UI_PostRaceStats' paging 0x4A4480
    (BLINK, or HIDE on the first / last page, the other STATIC, by
    0x537C70); the Quick Race track select (**observed** under Wine, three
    Rights 0.15 s apart restart it three times; its call site not found);
  - not while it runs: IconScroller_UpdateArrows 0x52FA80 (Right: the
    left arrow +0x80 STATIC, the right +0x84 BLINK; Left: 0x527670 STATIC
    and 0x495810 BLINK on +0x48), each only when 0x51D050 (script +0x30 is
    that name and its time +0x10 ≤ length +0x0C) says it is not running:
    the icon menus (UI_Main etc.; Wine: three Rights 0.15 s apart, one
    400 ms blink) and, by their class, the parts browsers' category
    strips and UI_Paint's slot strip (**inferred** for UI_Paint).
  The screens' refreshes leave a running BLINK alone (0x4B2310,
  0x4BE440, 0x4CE170 check 0x51D050 before STATIC). UI_CareerCarLot's
  step 0x4AFFA0 and the crib's 0x4B04C0 name "ScrollBar_Arrow_1" / "_2"
  too, which their packages lack (theirs are Scroll_Bar_Arrow_*), so
  nothing blinks there (**inferred** from the packages; 0x5426D0's
  fallback 0x52C580 not traced). Already ported before: the option rows
  (0x51E6C0), EA Trax, the custom HUD pickers (0x548E50 / 0x55D420).
  **Open**: the Quick Race filter header's arrows (LTrigger / RTrigger),
  the controller page's device arrows (0x4ACAA0 / 0x4ACBF0), the online
  screens' (0x4F0370 …, 0x4FAA60, 0x4FCBC1) and HUD blinks (0x4C8BAD,
  0x4C7090) are not ported.
* **Dialogs** (0x5587A0; the package by 0x526C50, see **Dialog packages**):
  the message 0x1E2640FA; one centred button (group 0xB8A7C6CE, label
  0xDA473D31) or two (0xB8A7C6CC / 0xB8A7C6CD, labels 0xDA473D2F /
  0xDA473D30), shown by clearing their hidden flag; the chosen button runs
  HIGHLIGHT (a 1.2 s looping pulse), the other UNHIGHLIGHT. Strings wrap at
  their `Sw` width by the renderer's own breaker (0x51C110 / 0x50BCE0; see
  "Screens fixes (scrfix)"; 0x520080 is the sponsor terms' only).
* **Dialog pushed over its screen** (menus111, **verified** for the car
  lot): ShowDialog 0x558030 (from 0x5587A0 / 0x558580 → 0x558690, arg 1
  the parent package) queues 0x555D10 op 0x16 with the dialog package over
  the parent, which stays on the stack and drawn in its current state. The
  car lot's Back (0x911AB364 in the handler at 0x4EDE00) asks "exit back
  to Explore Mode?" (0x4EDE8B) or "Rachel needs her car back."
  (0x4EDED8) at once, parent the lot's package, with no FORWARD: the lot
  shows under the dialog's dim (Wine menus107 c24; the clone's
  `Dialog::under`, shot `carlot_exit`, side by side compare/menus111).
  The stock-car question (0x4EDC86) has the same parent, yet under Wine
  (c14) neither the lot nor its car shows: **inferred** that it comes
  after the lot's FORWARD fade-out (not traced). The shops' questions
  (the clone's `setup_dialog`) take the "No" outcome as the screen under
  (the graphics shop under its install question, a private Wine copy
  graphics_exit; **inferred** for the other shops, each asking from its
  handler with its own package as parent). The car lot's bar
  (0x4F9AD0 → 0x542AD0 with "Scroll_Bar", 0x798F38; UI_QRCarSelect's
  class 0x4F9D90 uses "ScrollBar", 0x79648C): Scroll_Bar_Handle
  0x95043B2D, _Arrow_1 / _2 0x42AA46BC / BD — the clone looked for
  ScrollBar_* in UI_CareerCarLot and left the handle at its authored
  right end and both arrows on (fixed; Wine c13: first car, handle left,
  no left arrow). The logo frame 0xDF422345 there INITs into a 1.2 s
  alpha pulse (0x5A8E4EBE), so its absence in a capture is the phase.
* **Engage dialog** (UI_EngageRaceDialog, 0x4B5390): subject and race type
  by category and type (jump tables 0x4B56D8 / 0x4B56F0, e.g. "Sponsored
  Drag"), distance = track +0x98 × 0.000625 miles, purse 0x500C30 (event
  +0x30, sponsor events from the sponsor table), message 0x512620:
  "ENGAGE_<event>" for stage-0 events, else a random
  "ENGAGE_<TYPE>_RANDOM_n" (11–18 variants by set).
  The race's map (**verified**): the set-up 0x4B53C0 hides the map object
  0x0F365871 (0x4B5648 → 0x50CA00) and loads TRACKS\TRACKMAP<track>.bin
  (0x791864; event +0x18 the track; "TrackMapMemory" 0x5E8680, loader
  0x57CF00 with callback 0x4B53A0, which sets dialog +0x50 and registers
  the pack, 0x4901D0); +0x58 = hash TRACKMAP<track> (0x50B990). The
  handler 0x499C00 on the tick 0xC98356BA (0x499D13): loaded, the object
  takes texture +0x58 (0x537A70 → 0x50C990) and shows (0x50CA50); else it
  stays hidden. The package's own texture (the city map) is never shown.
  **Observed** under Wine (2026-10-30; career stage 1,
  STAGE_1_CIRCUIT_1, track 4013, a screenshot of a private Wine copy): the map
  is the whole TRACKMAP<track> texture, uncropped — Bayview with the
  race's route outlined — exactly as the clone draws it (side by side
  `map_sbs.png`); the TRACKMAP files hold that one 512 × 512 texture and
  no bounds. The texts (0x4B5410–0x4B548E): distance "%$f %s" of track
  +0x98 × 0.000625 (0x791884) — miles whatever the units — with "mi"
  (0xAC3A4F4C) or, metric (0x83AA1A), "km" (0xB5D640B1); the game's
  formatter Str_VFormat (0x43E950) gives %f without a precision 3 decimals
  (0x43EDFD), so "1.376 mi"; the purse "%$d" (0x78F744) of 0x500C30 with
  no currency sign ("250"). The clone had "1.38 Miles" and "$250"; fixed
  (`Engage::quads`, `screens::fixed3`). Also seen: the engage key opens
  the dialog again only after the car leaves the trigger and drives back
  in; Decline is focused once the dialog is up.
* **Post-race reward** (UI_PostRaceReward, 0x4A4240): "Bank Won" is
  0x532E60: the event's purse (+0x30), but a sponsor race (+0x34 = 1)
  with a sponsor signed in the current stage (stage record +0xE8) pays
  that sponsor's +0x02 (stage 2: 825–900) instead of the event's 1000;
  the win handler 0x541430 adds the same to the bank (career audit
  2026-10-31); "Reputation Points Won" comes from the winning margin
  (0x513FE0: % ahead of the runner-up, 100 with nobody else) through table
  0x7F7AC8 by reward class (0x512AE0): 1/2/5% (drift 10/20/30%) → 100,
  200, 300 points.
* **Cash** — the wallet is career +0x7134 (0x861E74); a new career starts
  with 0x865128 + 0x86512C (0x53291D), which only the bonus codes set
  ($200 / $1000, 0x5760B0): 0 otherwise.
* **HUD** (HUD_SingleRace, built at 0x4F8BF0; `src/frontend/hud/`).
  Objects are found by `fe_hash` (0x505450: `bin_hash` of the name in
  upper case). Each element lists its objects and a feature mask (low,
  high words): tach 0x4F5100 and turbo 0x4F5A70 (0x10000060, 0), position
  0x4F4AC0 (0x200), laps 0x4F5C40 (0x508), wrong way 0x4F6420 (0x4000),
  split 0x4F6030 (0x80), race over 0x4F7170 (0x4), style points 0x4F7AD0
  (0x20000), leader board 0x4F6130 (0x800000), minimap 0x4F73E0 (0x1),
  heat 0x4F69E0 (0x20000000), drift 0x4F6C00 (0x80000000), nitrous
  0x4F6A90 (0, 0x40000), countdown 0x4F7340 (0, 0x20000), free-roam
  events 0x4F83D0 (0, 0x2), and in career free roam region names
  0x4F4E70 (0x1), cash reward 0x4F5060 (0, 0x80) and bank 0x4F4F10
  (0, 0x80000); the outrun meter 0x4F87E0 (0, 0x40, HUD_FEATURE_GETAWAY_METER,
  update 0x4DABD0, see Outrun) is built in every HUD.
  - **Hide all at build** (**verified** 2026-10-31, run wrongway, in the
    disassembly and in the original's memory under Wine): outside a drag
    race (0x89E7D8 / 0x890108) and with 0x89E7E5 and 0x836474 clear (**not
    traced**), the constructor 0x4F8BF0 first calls 0x52CFD0(package, 0):
    0x52CF00 finds the package by name (0x519E50 over the lists at
    0x8384C4 → +8 → +0xE4 / +0xF4, else 0x51BB70's loaded resource +0x18),
    and 0x504880 walks its objects (+0x4C list, down every group through
    0x504830) with the visitor 0x79AC00, whose 0x51CEB0 hides each one
    (0x50CA00; it shows instead with the visitor's byte +4 set). Then
    0x4CA090(…, 0xFFFFFFFF, 0) and (…, 0, 0) run every element's
    show/hide, and only the elements' objects (and Leader_bottom, which
    0x4CA090 shows outside drift races with the lap info off) are shown
    again; an object no element claims stays hidden for the whole race.
    The hidden bit is each object's own: in memory (Quick Race Street X,
    Industrial Park Track 2, lap 6) HUD_Short_Track's LeaderBoardGroup,
    Position and the tach group read flags 0x40000001 while their rows,
    numbers and needle read 0x40000000 and draw. HUD_Short_Track (and
    HUD_Drag) hold a String "Wrong Way" (WrongWayText, 0x159ABF82,
    INIT colour alpha 0x96) that no code names (the exe has no
    "WrongWayText" string or hash): in memory it reads 0x40000001
    (hidden), so the original never draws it in a Street X race. The
    clone drew it through every Street X race from the green light (the
    a recurring "WRONG WAY" report); `Hud::quads` now hides every
    drawing object no shown element claims (with the shown elements'
    group members), outside drag races. Comparing the clone's hides with
    that memory read left one difference, SplitTimeText (shown in the
    clone, hidden in the original at that moment: the split element's own
    update, not this rule). **Open**: whether WrongWayText shows in a
    drag race (no hide-all there; not checked under Wine).
  - Every frame 0x6000A0 builds the features; when they change 0x4CA090
    (masking off 0xFDFBFFFF) tells each element, which shows or hides all
    its objects (0x4C6690). A race: 0x100000 | minimap | laps | split |
    leader board | wrong way (not race type 1 without 0x89E7BC) | style |
    tach, position only with more than one racer, high 0x80 | 0x20000
    (racing), nitrous while the car has any.
    **Before the green light** (race status 3, one player; verified
    2026-10-29, scenes75, against the original's Quick Race start under
    Wine): 0x6000A0 gives only the score (0x5EA8D0), with the gauges
    option the tach (0x5F32B0's bit), the nitrous gauge (a tank, not
    drift / street X) and the drag heat bar, plus high 0x20000 — no
    minimap, lap info, position or leader board; they appear at the
    green light (`hud::features` with the race clock below 0).
    **Observed** (scene and timeline ported 2026-10-29, see **Race start scene** in Session nis79): in that Quick Race (with the race-start NIS and
    its starter) the original showed no 3-2-1 digits and no "GO!": race
    status +8 (the countdown's start tick, 0x5EEA50) stayed 0 and +0xC
    read 1.0778 (peek), so the starter scene starts the race without
    the countdown; the clone has no starter scene and keeps the
    countdown. The turbo gauge (0x4C6B80) shows only while the car's
    engine +0x98 (car mover +0x48) is above 0 (0x5ECED0, full-physics
    cars); **ported** (2026-10-29): engine +0x98 is set once when the
    engine is built (0x5AA6D4: 0x457B00, the largest of the nine turbo
    curve points, physics +0x334, from 0; +0x9C the boost read for the
    needle), so the gauge shows only for a car whose turbo curve has a
    positive point (`hud::Input::turbo`; a stock 206 none, a stock Lancer
    Evo yes, compare/nis79/clone-qrstart-*.png).
    **Turbo needle** (**verified**, ported 2026-10-31, hud112): the gauge's
    update 0x4C6B80 (objects +0x20..+0x34 from 0x4F5A70: Turbofill,
    Turbolines, TurboDial = the needle, TurboRed_1/_2,
    Drift_Angle_Backing2), while its mask is on, shows them with boost
    and hides them without, then turns +0x28 by 0x51CEE0 to 0x4AA160(b,
    a, c) = (c − a)(b + 1)/2 + a (less 360 over 360; under 0 it becomes
    360 − v, sic), (a, c) = (−14°, 222°), or (158°, 96°) with 0x89E7D8 /
    0x890108; b = engine +0x9C (0x5ECED0, 0 for a mover kind ≠ 1). The
    boost (0x5A0F40, from the engine's tidy-up 0x5AA770 after its limiter
    test, dt 0x7FC800; nothing while +0x98 ≤ 0): full = +0x98 / 0.19,
    vacuum = −0.25 full, t = (rpm − idle)/(max − idle) (engine data +0 /
    +8), throttle = +0x80 × +0x78 (0x5942F0; 0 at the limiter +0x88).
    State +0xA0 = 0: throttle ≥ 0.3 → 1 (no update that frame), else
    boost = min(1, 2t) × vacuum. 1: throttle ≥ 0.3 → boost = min(full,
    boost + t·min(1, throttle)·2·dt); else, off the limiter, → 2 with
    +0xA8 = dt, +0xA4 = boost. 2 (blow-off): x = (+0xA8 + 1.1)²; v =
    ClampF(+0xA4/full/+0xA8 × 0.015 × Sin16f(6.5x² + π/2) + vacuum, 2 ×
    vacuum, +0xA4); +0xAC the last turning point (set to v when 0; when v
    lies between boost and +0xAC: |boost − +0xAC| ≥ 0.15 → +0xAC = boost,
    else done); +0xA8 += dt; boost = v; done or +0xA8 > 0.5 → state 0,
    +0xAC = 0. Clone: `handling::Turbo`, `hud::turbo_needle`,
    `hud::Input::boost`. At idle the needle rests at 0 (104°) as in the
    original (compare/hud112/A_airport-gauges.png; before, the clone left
    it at the package's angle, above 0).
    **Red zones** (**verified** in the data, 2026-10-31): Redline /
    RPM_Redline (`3RDPERSON_REDLINE_<rpm>`) and 3rdperson_TurboRed_1/_2
    (`3RDPERSON_TURBORED_1/_2`) are white textures tinted by the package
    colour 0xC8750B0B. FrontB.lzc holds them with TPK byte 0x56 = 1
    (alpha); the in-game bundles (InGameCommon .lzc/.bun for the
    REDLINE_<rpm> set, InGameRace / InGameFreeRoam / InGameDrift for
    TURBORED) with 0x56 = 2 (additive), and in a race or free roam the game
    has the in-game bundles loaded, so the red adds onto the dial: light
    red (≈ 210, 127, 106 in the original's frame) instead of the clone's
    dark red (≈ 120, 38, 36). Clone: `Assets::in_game` keeps the in-game
    copy of each texture the two differ in (12: these and SFX_SMOKE_B_CS)
    under an alias, and `Hud::quads` uses it (compare/hud112).
    Over (0x5FFF60): 0x4 alone.
    Free roam: minimap, style, tach, nitrous with a tank (0x5F32B0); high 0x2 | 0x80, plus 0x80000 in a
    career. The custom-HUD options at 0x83AA64 + 0x28 × player gate each
    bit (set at run time; the clone takes them as on) and 0x83AA1A picks
    km/h.
  - Tach 0x4C68D0: speed "%3d" of |car +0x430| × 2.23694 (MPH 0x8569AB44)
    or × 3.6 (KM/H 0x8569A25F); gear letter from 0x7F66A4 (R, N, 1…),
    coloured 0xFFFF8040; the needle turns 235° × max rpm / dial × rpm /
    max rpm (0x4AA0E0, 0x51CEE0) on a 10500/9500/8500/7500 dial picked by
    0x60FE20; the shift light comes on past the up-shift rpm less 0.95 ×
    the gear's shift window (0x5ECD40, 0x594F10). The window is the
    class row of 0x7FBF60 × the drivetrain's +0xE4 (car +0x34 → +0x4C),
    which 0x5B6130 sets from 0x7A19F0[difficulty] only for a human in a
    drag race outside the career; everywhere else it is 1.0 (read live
    under Wine in a Quick Race circuit: +0xE4 1.0, +0xF0 class 3, up-shift
    rpm 6400; **fixed** 2026-10-31, reports: the clone scaled every
    window by the difficulty table with its never-set difficulty 0, ×3, so
    the light came on ~1100 rpm early — at 5390 rpm in 2nd where the
    original shows none;
    `handling::drag_shift_scale`, `Handling::shift_window_scale`). The car's HUD textures
    (0x5ED9C0, GLOBAL/HUD_CustomTextures): `3RDPERSON_<dial>LINES_CUSTOM_nn`,
    `3RDPERSON_REDLINE_<rpm>`, `TACH_FILL_CUSTOM_nn`,
    `3RDPERSON_RPMDIAL_CUSTOM_nn` and the turbo gauge's.
  - Custom gauge colours (**verified**, 0x4F5100, the non-drag tach built
    at 0x4F8D7C; the drag tach 0x4F6500 reads none): the car's parts in
    slots 0xA4 / 0xA5 / 0xA6 (0x610300) give {BLUE, GREEN, RED, 0xFF}
    (attributes "BLUE" / "GREEN" / "RED", +0x90 / +0xA0 / +0xB0), all
    0xFF (white) with no part; 0x51D870 sets 0xA4's on tach_fill and
    3rdperson_Turbofill, 0xA5's on 3rdPersonNeedle and
    3rdperson_TurboDial, 0xA6's on 3rdPersonTachRPMNumbers and
    3rdperson_Turbolines (`hud::Input::gauge_colors`,
    `custom_hud::HUD_TINTS`).
  - Laps 0x4AA1B0: "LapNumber" "%2d" lap + 1, "NumberLaps" "/%2d"; point
    to point "%3d%%" done with "LapText" = "Completed". "CurrentLapTime"
    shows "--:--.--" before the start, the finished lap's time for 3 s
    after each lap, else the running lap; "Lap_Time_Title"/"_Text" show 2 s
    after a lap ("LAP TIME", or "BEST LAP TIME" from the second lap when it
    was the best).
  - Times (0x577250 → 0x576D40): the clock runs at 4000 ticks/s; "SS.CC"
    under a minute, "MM:SS.CC" under an hour, a zero leading digit printed
    as a space; with no whole seconds "0.CC" (template 0x7A0630, race9).
  - Countdown 0x4AB5B0: 3, 2, 1 a step each (0x5EEA50), "GO!" (0x535) for
    a step after the green light; each grows from 1 to 3× and fades in as
    sin(90° × the step's fraction).
  - Wrong way 0x4AAFC0 runs FADEIN / FADEOUT on "WrongWayImage". Race over
    0x4C7F00 runs RACEOVER and prints "Your time: <time>" for a lone racer.
    Bank 0x4A9EF0: "Cash_data" = "%$d" ('$' groups thousands with ',').
  - Script names on objects are `bin_hash` of the upper-case name (FADEIN
    0x5B0D9106, RACEOVER 0x657A31F6). Many HUD objects' INIT has length 0
    and chains to HIDE; the HUD follows that chain (the menus do not: their
    screens fade such objects back in through package messages not traced).
  - Minimap 0x4CA840: the map image takes `TRACKMAP<track>`
    (TRACKS/TRACKMAP<n>.BIN; free roam is track 4000). Its texture window
    is 0.3 / zoom wide, centred on the player at u = (x − left) / size,
    v = (top − y) / size, from the track record (+0xAC left, +0xB0 top,
    +0xB4 size, +0xC0 zoom). Markers (start line, event indicators
    `<TYPE>EVENTINDICATOR_n`, …) sit at their offset from the player in
    window units × the map's size, pulled in to 0.55 and dropped past 1 (10
    in free roam); unused ones stay where the package parks them, off
    screen (see "Minimap items" for which show: never the start line). The player arrow turns 0x43E6D0(sin h, cos h) = 90° − heading.
    The map texture blends by destination alpha (TPK byte 0x56 = 5) against
    Minimap_Mask (the clone too, see the alpha-only quad at 0x5CBA9F). 0x4DA640
    hides the Filter group, GPS search and 0x697D2BCA outside those states.
  - Leader board 0x4DA190 (one local player) → 0x4C6F30 / 0x4AA9F0: a row
    per racer in race order, "LeaderText_n" = "%d :", "LBData_n" = the
    name, SETPLAYERCOLOR on the player's "LBBacking_n" and "LBData_n";
    rows past the racer count are hidden. 0x4CA090 runs LAPINFOON/OFF on
    "LeaderBoardGroup" and "Leader_bottom" with feature 0x400 and
    LEADERON/OFF on "BestTimeCounter" with 0x800000.
  - Clone's choices: one racer (no opponents yet), HUD index 0, MPH, the
    custom-HUD options on with a north-up map (option 1); the race-over message holds for the finish's hold (3 s, see Race over) before the rewards. Wrong way
    (racer +0x248, 0x600720 each frame with its last value): the heading
    110° or more off the road's direction (10° less once wrong); above
    10 mph (speed × 2.237) the direction of travel too, and a new wrong way
    needs all four wheels down (car +0x424); at 10 mph or less the last
    value stands. The road's direction (**verified**, ported) is the car's
    road locator (car +0x8D8, the race-road locator 0x5D9E40 run each
    frame by 0x5DA340): car +0x900 / +0x904 is its +0x28 driving
    direction (the spline's tangent at the nearest point, negated on a
    segment run backward, +0x7C < 0), car +0x8F0 its node (+0x18); no
    node → not wrong way. Before the angle test 0x5DED20 looks up route
    triggers of kinds 0–3 at the locator's last point (+0x08, 0x5D9C00):
    kind 0 → not wrong way; kind 1 → its +0x0C / +0x10 direction replaces
    the road's (racer +0x108 = 1); kinds 2 / 3 → 0x5FBE10, which (z
    +0x14 = 0, or car +0x5CC set and car z +0x68 below it) asks a reset
    (car +0x96C = 1) for kind 2, or for kind 3 not named PCDEMO places the
    car at the trigger facing its direction (0x5FBD70) — either way not
    wrong way. No route file has a kind 0–2 record; kind 3 is PCDEMO in
    every Paths file (no effect) and one unnamed 30 × 20 m zone at (835,
    −1214) in ROUTESL4RD Paths4401/4402/4404/FreeRoam. **Ported**
    (2026-10-31, race3; `AiRoutes::blocks`, `ResetZone::parse_blocks` /
    `place`): the caller is the player's update 0x602660 (0x60267E), and
    0x600720 runs the lookup on both paths (no road node: 0x600730; with
    one: 0x600798); 0x5DED20 tries 0x5D9C00(point, kind, 0) for kinds 0,
    1, 2, 3 and returns the first hit; 0x5D9C00 walks the kind's records
    (manager +0x440 + 4·kind to +0x444 + 4·kind, each +0x42 bytes long),
    box +0x20 / +0x28 (0x43D410) then polygon +0x44 of +0x40 points
    (0x43D890). The point is the locator's +0x08, which 0x5DA340 updates
    only once the car has moved more than the locator's +0x38 (Manhattan)
    from it. The L4RD zone: +0x04 (835.05, −1214.87), +0x0C (0.26, −0.96),
    z +0x14 = 0, name +0x30 = 0, box (827.1, −1212.2)–(859.9, −1193.3),
    five points: the car is put just south of it, facing south (out of
    it), at 8 m/s. 0x5FBD70(pos, 8, heading, 1) = 0x5F52D0, the locator
    on the new place (0x5DA340), the controller's vt+0x30(1) and vt+0x34,
    car +0x974 = the tick and +0x978 = 0x7A55C8 or 0x7A55CC (by 0x7D7D3C),
    0x5F6CE0. The clone tests in races only (the race's Paths file; the
    car's place for the locator's point, **inferred** within +0x38) and
    drops the car on the road below; free roam's PathsFreeRoam zone is
    not tested, nor 0x5F6CE0 / the controller resets. **Free roam**
    (2026-10-31, race5; **verified**): the world update runs 0x602660 for
    every human racer (0x60A257, over 0x8900B4 / count 0x8900BC) with no
    race-mode test, so 0x600720's lookup runs in free roam too, over the
    triggers the loader (0x57F4C0, when 0x89E7B0 = 0 or no Paths%d file)
    reads from `Tracks\Routes%s\PathsFreeRoam.bin` (0x7A0BC4), %s the
    track record's +0x40 (0x883DAC). Free roam is region L4RA (the
    clone's `EXPLORE_REGION`), whose PathsFreeRoam holds one kind-3
    trigger, named PCDEMO (0x728558D7 at (−1080, 288)): no effect. The
    L4RD zone is only in ROUTESL4RD's files, read when an L4RD track is
    loaded (its races 4401 / 4402 / 4404), so free roam has no reset
    zone and the clone needs no test there. The clone's former
    stand-in, the road polyline's piece at the car's road distance, broke
    past a circuit's end (the locator runs on past the last spline: 4607
    at 895 m of 887 m took the closing piece, which runs backward) and
    showed a false WRONG WAY there. Car +0x900 is also read by the reset
    (car frame 0x6012C0: with car +0x96C set, a human racer's car without
    +0x900 of its racer goes to 0x6009B0, then +0x96C = 0): on race
    coordinates the kept speed is max(speed +0x42C, 0) × max(car +0x900 ·
    forward +0x90, 0), capped at 0x8021CC; a kind-4 route trigger at the
    car places it at the trigger facing its direction; else, with the
    human's road node (car +0x2C → +0x0C) within 150 m, it goes on that
    node facing its direction (reversed by the race direction bit,
    0x5FBEE0), else to the oldest of its racer's four saved places
    (0x5FBD70; ring at racer +0xC0, headings +0x100, index +0xBC). The trigger
    branch places at the trigger's +0x04 / +0x08 with z 0 (0x600B37),
    heading Atan2_16(+0x0C, +0x10), speed 8 m/s (0x8021CC); the node
    branch at the node's control point (AIRoute_PointGeometry, lateral 0)
    facing its tangent, or the backward one (0x401AA0) when the sign is
    −1, with the kept speed (Car_PlaceOnRoad); the saved place at rest.
    Kind-4 triggers exist only in ROUTESL4RA (in every Paths file of it).
    The places are saved (0x602AEE–0x602BD6) on every 32nd frame
    (0x7FB724 & 31) or while +0xBC is 0: when the car is over 16 m
    (0x79C68C, 2D) from the place at the count and there is ground under
    it (0x5E7E90: the probe from z 99999.1), the count steps on and that
    place takes car +0x60 and the heading +0x40C — all four the first
    time. The reset key (**verified**): JOY event
    0xA1 in the human player's handler 0x602BE0, in races and free roam
    alike (no race-type test), calls 0x6009B0 directly when no engage runs
    (0x5012C0 on 0x85EF50 +0x1C), the key is down, not (car +0x24 = 1, car
    +0x30 ≠ 0 and its +0x2B4 ≠ 0; meaning **open**) and 0x5F3F10 allows
    it: no blocking event camera (0x8020E8[player +0x58], kinds 6 / 7
    excepted), |v| × 2.237 ≤ 60 mph, not the first 3 s of race status 4
    (except race type 7), not within car +0x978 (2.0 s) of the last
    stamped placement (+0x974). So free roam resets like a race (kept
    speed, kind-4 zone, node within 150 m, saved place — not a 1 m lift)
    and saves its places too. Clone: `racer_reset::{reset_key,
    may_reset}`; the key handler (src/bin/game/input/dispatch.rs,
    `reset_key_allowed`, the shop fixes) gates both paths: the controller condition
    is the human driver's race-start hold (car +0x30 is the driver /
    input block: 0x41E37E reads its +0x2B4 and brakes through 0x5EB900 on
    the same object; 0x410430 writes it — `CarState::held`; car +0x24 = 1
    for the player through the countdown, **observed** the drag-finish pass), the blocking
    kinds 3–7 of 0x8020E8 (bytes 0,0,0,1,1,1,1,1), the racing clock from
    the green. The stamp: 0x5FBD70 with its flag clear writes +0x974 = 0
    (0x5FBDF2), with it set the tick and +0x978 = 2.0 (0x5FBDD9); the
    reset's own placements (0x600DAC / 0x600DF1) and the kind-3 block
    (0x5FBE98) pass 1, the racer set-up (0x60406F) 0 (`Drive::stamp_placement`,
    `Drive::place` clears it). Free roam (`reset_in_free_roam`): the
    kind-4 zones of PathsFreeRoam.bin, the human-flagged node of the
    stage's free-roam network within 150 m, else the saved places, which
    free roam now saves each update (`Drive::roam`). Ported (`reset_to_road`, `Progress::save_place`, the zones
    `AiRoutes::resets` from Paths<track>.bin) with these stand-ins
    (**inferred**): the human's road node is the AI racer controller's
    route node when one drives the car, else the human search (mask
    0x400, see AI route layer, Start); the car's point is its position;
    the trigger's z is its own (+0x14). **Not ported**: when 0x5EAED0
    is false (always outside drag races on 4400–4404 / 4709) a line test
    between the node and the car (0x5E7EE0 / 0x5E8120 on the world's
    polygons) that, when blocked, takes 0x40F180's nearest clear node
    (flags 0x1020, the race coordinate) and its record's direction; the
    drag lane offset (AI_RequestLane, 0x5E2DB0).
    **Confirmed under Wine** (2026-10-31, run wrongway, `fastgpu.sh`,
    racer = 0x8900AC, flag racer +0x248): driving Quick Race Street X
    (Industrial Park Tracks 2 = 4602 and 3) with keys along the road, with
    a weave and at up to 40 m/s, never set the flag (four runs, about 9
    minutes, the flag polled about 40 times a second); a U-turn set it at
    once. The constants: 0x7A557C = 110°
    (heading), 0x7A5580 = 110° (travel), 0x7A5584 = 10° (the slack, added
    to the angle once wrong), 0x7A5588 = 10 mph, speed |car +0x42C| ×
    2.237 (0x78426C). The original's logged car places (every frame of
    one 150 s run on 4602) through the clone's `RoadRoute::relocate` gave
    the same driving direction and road distance at every sample (none
    over 3 m or 45° apart). The WRONG WAY the player saw in the clone's
    Street X was not this flag but HUD_Short_Track's WrongWayText (see
    HUD, "Hide all at build"). `wrong_way_street_x_on_keys` runs the
    career's first Street X (STAGE_1_SHORT_1) on every Street X track and
    direction, through its start scene with the rivals, driven on keys at
    60 fps: no false flag and no "Wrong Way" string drawn.
    Tests: `wrong_way_street_x` drives every Street X track both ways
    (two laps on the AI racer controller, then a U-turn);
    `wrong_way_circuits_and_sprints` (ignored, ~8 min) does the circuits
    and sprints: no false WRONG WAY in 130 runs, and the AI racer
    controller finishes all of them. It used to lose the route on 15
    (circuits 4002, 4011, 4044, 4088 forward, 4081, 4083, 4085 backward;
    sprints 4107, 4124, 4126 forward, 4103, 4104, 4106, 4122, 4127
    backward): not a junction choice but the restart after leaving the
    road (see AI route layer, "Road node follow / re-search": the
    record's direction, 0x40F750). `ai_keeps_the_race_route` repeats four
    of them; `rivals_keep_the_race_route` runs four full races with
    opponents (every rival finishes, none drives back along the road). Driving
    into an event's marker trigger makes it current and the Engage key
    opens its engage dialog (see Career, "Triggers"). Not built: the style points update (its objects stay
    hidden); the "Circuit Stats" panel and the showcase "Preview" label
    belong to no element found and are hidden.
* **Car lot** (UI_CareerCarLot, screen 0x4F9D90; refresh 0x4B2310): in a
  career (game mode 1) the package keeps its "Car Lot" title and "Select
  Car" button. "ManufacturerLogo" takes `CARSELECT_MANUFACTURER_<make>`
  (0x511E60; the make is car record +0xC0, 0x610180) and "CarBadge"
  `SECONDARY_LOGO_<car>`, else `GENERICPLACEHOLDER` — by region (this
  executable's 6, European; English): the Corsa (type 7) VAUXHALL / CORSA,
  the Miata (4) MX5, the Tiburon (11) COUPE (`region_logos`); the rating bars come
  from 0x497CD0. Quick Race (mode 2) titles it "Quick Race Car Select"
  (0x5AC1FB59). The clone draws the selected car turning on black behind the
  screen; the game's lot scene is not built.
* **World Map** (UI_CareerWorldMap, screen 0x4EE690): a top scroller of
  the unlocked stages, "Stage %d" (0x0A96B270, 0x4CBF80) on the
  0xC8E97C9C label; a vertical event scroller (MINI_MASTER / EVENT_ICONS, a
  U2_MINI_ICON per event, 0x4E0CE0); five info rows, labels 0x4D2F5C61+
  and data 0x23EAE8B9+; and a map widget (screen +0x1F4; **verified**,
  ported: `WorldMap::widget`, `map_zoom`, `map_window_uv`). Its vtable
  0x78F8C8 holds only the destructor 0x4B4B60; the class (constructor
  0x4985A0's family, also UI_InGame_WorldMap's and 0x4E2FCF's) is driven
  by the screen: the constructor hides the map object 0x90FE80F3
  (0x50CA00) and sets +0x22C = now, as do a new selection (0x4CC140 at
  0x4CC1D9) and a new stage (0x4E0E50 at 0x4E0EE4). The tick (0x4E0F60 on
  0xC98356BA): with +0x22C set (0x401820), 0.3 s after it, 0x498600(the
  selected item's track info) sets widget +0xC = the track (record +0x8A)
  and +0x22C is cleared; otherwise the widget's tick 0x4CDD40 runs: a
  track other than the loaded one loads TRACKS\TRACKMAP%d.bin
  (0x57CF80), then shows the object (0x50CA50) with TRACKMAP<n> and
  starts the zoom (0x4B4B80): window size (track +0x120) 0 → the whole
  map; +0x124 set → the window at once; else a countdown +0x28 of
  ftol(1.0 × 4000 + 0.5) ticks (0x577320 steps it) with the whole map,
  and while it is ≤ 0.5 s, 0x498630(1 − r/0.5). 0x498630(t): s =
  sin(t·π/2) (0x43C350), UV corners (0x5179D0 5 / 6) (x·s, y·s)/512 and
  (x·s + 512 − (512 − size)·s, …)/512 with (x, y, size) = track +0x118 /
  +0x11C / +0x120 (e.g. 4001: 168, 194, 270). The same track keeps its
  zoom. The widget's clock is in seconds (the 1.0 s countdown in 0.25 ms
  ticks, 0.3 s hand-over delay) for every screen that has one — World Map
  and Quick Race track select alike; the clone's screens tick in ms, so
  both convert (`WorldMap::changed` / `widget` / `map_zoom` take seconds;
  the World Map passed ms and zoomed at once). The clone loads the stage's
  maps at once. The event scroller
  (0x4E0CE0): each item (vtable 0x79AFE4, data from 0x4B1200, vtable
  0x790EFC) gets the texture U2_MINI_ICON (0x50C990) — the same icon for
  every event; the clone no longer marks events on the map (that was not
  the game's). **Open**: the scroller's layout (0x5435E0 on MINI_MASTER /
  EVENT_ICONS: how many rows show).
* **MC_Main** (**verified** for Start New Career; `screens::McMain`):
  0x4A3100(screen, a, mode, callback, its argument, …) stores its
  arguments (0x836518, 0x83651C, mode 0x836514, 0x836524 …) and opens
  MC_Main.fng — mode & 0xF = 1 pushes it, 2 or 3 switches to it; the
  screen (factory entry 0x7F9144 → 0x4F3320, vtable 0x797DFC, handler
  0x4FD570 over 0x4FB8F0) runs the operation mode & 0xF0 on its start
  (0x4F3570; 0x10/0x30/0x70 0x4E8CE0, 0x20 0x4A9590, 0x40/0x60 message
  0x5A051729 → 0x4F33A0, 0x50, 0x80, 0xA0, 0xB0, 0xC0, 0xD0 — only 0x60
  is ported). Start New Career (0x4AF090) passes mode 0x21961 and no
  callback: operation 0x60 (create a profile), 0x100 (the exit message
  goes to the caller, 0x4A30A0), 0x1000, 0x20000 (new career; 0x800 not
  traced). The steps, the texts and keys are in `McMain`'s comment: with
  a profile current (0x83A9E0 +0x20) "Creating a new profile will …"
  (Yes: 0x532840 new career, 0x52A470 keeps a copy of the profile that
  0x5347D0 restores on cancel), else "Would you like to create a new
  profile?"; Yes/No with No focused, Back ignored; then the name editor
  (PC_NAME_ENTRY, field 0x5B9D88B9, "NAME", ≤ 7 letters / digits, Enter
  0x0D with text → 0xDA5B8712, Escape 0x1B → 0xC9D30688, Backspace
  drops one; 0x551280 / 0x4FF310); then "Are you sure you want to save
  the profile named %s?" — Yes saves (0x4D9AB0), No goes on unsaved
  (0x1000); both leave with 0x461A18EE, on which UI_StartCareer starts the
  career. Leaving pops MC_Main and sends the caller 0x251BF83E
  (0x4A31D0; L2: mode & 0xF = 2, e.g. the Options' / Customize's Back
  0x2042, switches to the stored next screen instead and tells the
  caller nothing; with no profile operation 0x40 asks to create one
  first — internal notes L2.1). The byte 0x844AD9 that skips the save question
  (**verified**) is the Options' Save row at Autosave (0x49B170; its text
  0x49B210: 0x844AD9 "Autosave" 0x1B7ABF8E, else 0x844AD8 "Prompt"
  0x1B7ABF8F, else "Off"): the state setter 0x4E8D30 turns state 5 into
  6 (save unasked, 0x4E8DDD) when it is set and the operation lacks
  0x200, as Start New Career's does; the clone follows (`McMain.autosave`
  from the settings). The 0x2000000 Yes/No's handler (0x4F37F6) sets it
  (Yes) or clears it (No) and goes to state 5, but no caller asking with
  that tag was found. The starting text (**verified**, 0x5564EE): "NAME"
  unless the game language 0x7F70D0 or the system one 0x7FCA00 (table
  0x7FCA08: "Korean" → 8) is Korean, then empty. 0x4FF3B0 copies it to
  both the edit buffer (0x83A8B0, length 0x83A9B0) and the restore copy
  (0x83A7B0); typed characters (WM_CHAR → 0x55DBE0 → 0x551280) are
  appended to the buffer — nothing clears it on the first key, so "Ray"
  typed at once gives "NAMERay" (the player backspaces "NAME" away); the
  key counter 0x83A9B8 (editor +0x208, zeroed by 0x4FF3B0, counting every
  character 0x551280 gets — Enter, Escape and Backspace too) only makes a
  0x0D that is the very first key do nothing (0x5512B9); the clone counts
  the same (`McMain::keys`), so an Enter straight away leaves "NAME". The editor takes keys once the screen's state byte (+0x10,
  1 → 2 → 3 on two ticks 0xC98356BA, 0x54E5C0) sets 0x83A9C0. **Open**: the
  other operations (load, save, delete, EA login) and the save's
  progress dialogs (states 4, 6, 7); the profile files and the Profile
  Manager: see "Native profile saves and the Profile Manager" below.
* **Open** — dyno, the online flow past its first screen; the World Map
  event scroller's layout (see World Map). The
  Jump to Garage and Next Race conditions are traced: 0x8635A8 is read by
  the explore pause menu at 0x4C29D1 (in UI_pause, 0x4C2900; the Jump to
  Garage item is added when the byte is nonzero: test al, al, then je at
  0x4C29D8 skips it), and 0x513FA0 is
  called only by UI_postrace (0x4BEE10); it holds when the current event
  pointer (+0x34E4 of the object it is called on) is set, is not -0xD24, is not the table's
  last 0xD24-byte record (table at +0x54, count +0x34E8) and the record
  0x503320(0) returns has state 1 at +8. The World Map item shows when
  0x862838 (career +0x7AF8, the current stage) is above 1.

### Boot, customize extras, magazine reward, loading tips (2026-10-28)

* **Game start's PC_Loading** (**verified**, docs/original/loading-image.md): WinMain makes the front
  end's load the first load step (0x580E6C: 0x865484 = 0x57FF20); 0x57FF20 closes loading_boot.fng
  and opens PC_Loading.fng (0x57FF95) before the front end's files, so the start always shows it
  ahead of the boot list below (observed under Wine: ~0.13 s, then LS_BlankMovie). The clone shows
  it under the 200 ms rule like every loading screen (not at all when
  the start is done sooner; `BootLoading`; `enh.boot_loading_min_ms` opt-in minimum).
* **Boot sequence** (**verified** unless marked; `BootStep`, `game.rs`):
  0x4D9730 builds the list object 0x836488 (0x4D9580) from the names at
  0x7F6620 (or the same ones at 0x7F65E8, by 0x570A40's record +8):
  DiscErrorPC, MC_Bootup, LS_BlankMovie, LS_EAlogo, LS_THXMovie,
  LS_PSAMovie, UG_LS_IntroFMV, UG_LS_Splash, MC_Background, UI_Main
  (empty names skipped). DiscErrorPC is passed when the disc check
  0x5BF550 succeeds; each screen's end calls 0x4C4F50, which takes the
  next name, shows MC_Background only when the memory-card state
  0x836498 +0x20 is 0 or 0xB (0x4A9590), and for UI_Main with 0x83648C
  set starts the career instead (0x50D2A0, 0x5410F0, 0x5404B0). The
  screens (descriptors 0x7F8A94…0x7F8B3C): LS_BlankMovie 0x4A8AD0 →
  0x4A8A30 "blank" and LS_EAlogo ("LS_EALogo.fng" 0x799A38) 0x4A89D0 →
  0x4A8930 "ealogo", both on movie 0x58BCF5B6; LS_THXMovie 0x4A8D90 →
  0x4A8CC0 "THX_logo_fr" (0x790878) on 0x8BA694A3; LS_PSAMovie 0x4A8C60
  → 0x4A8B30 "PSA" on 0xF2754103; UG_LS_IntroFMV 0x4C52D0 → 0x4A8860
  "FMVOpening" on 0x72CF9F38. Handlers: 0x4C53A0 (blank, EA, THX) goes
  on only at the movie's end (0xC3960EB9); the PSA's 0x4C53D0 also on
  Accept (0x406415E3) / Start (0xB5AF2461) when 0x836498 +5 is set
  (**open**: what sets it; the clone never skips it); the intro's
  0x4C5330 on Accept / Start at any time (to MU_Main_PC.fng instead when
  0x836490 is set). UG_LS_Splash (0x4C5500 → 0x4A90F0, handler 0x4C5580):
  `screens::Splash` — "start_click" reads "Press Enter Key" (0x1F182080,
  set after 0xEEEB248E), LicenseBlurb and three objects hidden; the
  package's 0x35F8620B (sent at 0 by 0x02DDF58C's INIT) stores the time;
  4 s later (16000 ticks of 0x8651A8, 1/4000 s) "FadeIn" on 0x02DDF58C and
  0x4C46F86A and 0x836495 = 1 (at once in the constructor when already
  set); Accept / Start then play the START sound (0x51DE70 event 9) and go
  on; 30 s after opening (× 0.00025 > 30.0, 0x784268 / 0x784478) the intro
  comes back (0x4C5080 with UG_LS_IntroFMV.fng, list position moved
  there). **Open**: MC_Bootup and
  MC_Background (the clone loads the profile directly), the splash's
  Chyron message (0x55DDB0(0x69696969, "Chyron.fng")). `game --no-boot`
  starts on UI_Main; `--shot boot_ealogo|boot_thx|boot_psa|boot_intro|splash`.
  **Movie files** (**verified**: traced and seen under Wine). 0x51C740
  builds the name, "%sMOVIES\%s%s" (0x79B1DC, prefix 0x783FE5 empty, 0x40
  bytes): ".vp6" (0x79B1D4) for FMVOpening, IndustEast, IndustWest,
  Suburban, UpperClass, THX_logo, THX_logo_fr (0x7907E8, 0x79B224, 0x79B218,
  0x79B20C, 0x79B200, 0x79B1F4, 0x790878) and blank (0x79082C), "_fr.vp6"
  (0x79B1EC) for every other name; "ealogo" (0x790804) has a branch of its
  own that takes the same string. The names are compared without case
  (0x43DCC0). No region, language or registry value is read: the suffix
  is a constant of this build (0x7FA73C = 6, whose record 0x7FA7B8 names
  "FRANCE", is not read by these functions). The screens pass constant names: "ealogo"
  (0x4A8930), "THX_logo_fr" through "%s" (0x4A8CC0), "PSA" (0x790848,
  0x4A8B30). 0x566250 (the message 7 handler, the one caller of 0x51C740)
  reads the volume byte (0x51C580), asks the file layer whether that one
  name exists (0x6F909D → 0x6F87EA: a request from 0x6F7B34 on the default
  device, 0x6F78DC, for a name without ':' or a leading separator; waited
  for by 0x6F75DD), plays it (0x566030) if so, else sends the movie's end
  0xC3960EB9 at once (0x55DC60 on 0x8637B0), so the screen is skipped. No
  other name is tried. Under Wine (vanilla EXE, no widescreen fix, the
  player's MOVIES byte for byte, a fresh profile, frame period 1/60,
  frames at 4 a second, WINEDEBUG=+file): blank.vp6 opens; 1.05 s later
  MOVIES\ealogo_fr.vp6, THX_logo_fr.vp6 and PSA_fr.vp6 are each looked
  up twice (0x50C4A0's fopen of the relative name, then the file layer
  under the Install Dir) and not found, and
  FMVOpening.vp6 opens 4 ms after the first of them: no EA logo, no THX,
  no PSA, the intro from the first second on, the title at about 63 s and
  FMVOpening again 92 s after the first (the title's 30 s). Start New
  Career asks for scene01_fr.vp6, not found, and the airport scene
  follows. With the same files copied to ealogo_fr.vp6, THX_logo_fr.vp6
  and PSA_fr.vp6 the three play (3.2, 6.0 and 23.8 s) before FMVOpening.
  With the player's widescreen fix and its ini (SkipIntro = 1) the four
  names after blank are asked as "…SkipThis_fr.vp6" (the fix hooks them;
  not found): only blank.vp6 plays and the title is up after about 1 s. The English data
  holds ealogo_en/_dn/_du/_sw.vp6, PSA_en.vp6, THX_logo.vp6 and
  SceneNN_en / *Tut_en.vp6: this executable opens none of them, so the
  logos, the PSA, the career's scenes and the tutorials do not play with
  it. The US 1.2 English edition names its movies without any suffix
  (ealogo.vp6, PSA.vp6, Scene01.vp6, DriftTut.vp6, …, with a movies.tag).
  The clone asks for the built name first
  (`movie::file_name`), then this install's own name
  (`game_assets::resolve_movie`: `<name>.vp6`, THX_logo.vp6 for
  THX_logo_fr; the text language's `<name>_<xx>.vp6`; `<name>_en.vp6`;
  any one `<name>_<xx>.vp6`; a deliberate deviation,
  docs/playing/native-deviations.md "Movie files"), so the US, European and
  single-language data play; a screen with none of these files ends in the
  tick it opens (`movie::path`, `movie_tick`) and logs the names tried. **Open**:
  how an install shows these logos in English with this data; **inferred**:
  another build of the executable (one built with "_en") or other files,
  neither at hand.
  **Observed (2026-11-04)**: with each `*_en.vp6` copied to `*_fr.vp6`
  in a Wine copy's MOVIES, the career's movies play there: Scene14 on the
  final race's Accept, Scene15 after the final win's save questions
  (internal notes).
* **Quit key** (**verified** 0x558B80, the window's character handler;
  seen under Wine on UI_Main): 'Q' or 'q', once the splash was passed
  (0x4A8C50 → 0x836494), in the front end (0x8654A4 = 3; or 6 with
  0x571260(0x865480)), when the top screen's help-bar record (0x52CF00,
  +0x1C the button mask) is non-zero: 0x5587A0 with no package (size
  rule), "Are you sure you want to quit?" (0x9DE6B485), Quit Game
  (0x1C101778 → 0x919B1DD9) and Cancel (0x541389E5 → 0x42CA53FC, also
  Back), Cancel focused, over the screen. Quit Game ends the game
  (**observed**: the process exits; the message's handler not traced).
  The help bar's Quit Game button (0x55DE9B: mask bit 1; bit 2 Logoff asks
  0xD5D46FDF) asks the same. Escape on UI_Main does nothing in the
  original (Wine, twice); so in the clone (2026-10-31, selftest): Back on
  UI_Main is ignored, Q asks to quit, closing the window exits; Escape is
  Back on every other screen.
  Clone: `LotDeal::QuitGame`, shot `quitgame`.
* **UI_ChooseSpinner** (**verified**; `Game::spinner_browser`): factory
  0x56C100 → 0x569510 (vtable 0x79D8A8, fill 0x560D40, messages 0x569680).
  UI_ICEMAIN's handler 0x568750 on the Spinners item (slot 0x1D, built by
  0x53A9E0 like the other kinds, lock 0x539D10(0x1D)) sets 0x838998 = 8
  and the next screen (0x520A10 → 0x50E3D0) is UI_ChooseSpinner; its fill
  is the rims browser's for the brand "SPINNER" (0x539B80 with
  bin_hash("SPINNER") = 0xC8D1A75E, the car's rim size): styles by name on
  OPTION_MASTER (icon 0x0294D2A3), sizes down MINI_MASTER, opening on the
  fitted rim's style when it is a spinner. Back (0x520A90) returns to
  UI_ICEMAIN with 0x838998 = 0. The clone listed every rim under Spinners
  before. **Open**: the spinner paint mode (+0x214, 0x9F00F07 /
  0x51123592).
* **UI_ChooseUniquePart** (**verified** unless marked;
  `Game::unique_part_screen`, `Game::unique_reward`; selftest
  `selftest_unique_part`, shots `unique_part`, `unique_perf`): descriptor
  0x7F8730, factory 0x554B30 → 0x553210 (vtable 0x79CAA8: messages
  0x561730, refresh +0x10 0x5618C0, fill +0x14 0x53ABE0). The shop front
  end 0x528570 takes THE_UNIQUES_SHOP's record (kind 6 → 0x50E2E0, 0x520430
  sets 0x838990 = 6) whenever the running world challenge is in state 4
  (career +0xD1C, set by 0x54CB00 on entering the shop whose +0x38 is the
  record's +0x0C), whichever shop that is. The list (0x533D30, through
  0x539AA0) walks the record's +0x15..+0x17 (0 and −1 skipped): with +0x14
  kind 1 (visual) 0x3E the vinyls (slot 0x40 by 0x61BA40(car, 0x40, 0, −1),
  part byte +5 = 0x98: kind 0x18, level 4), 0xD the rims (0x61BA40(car,
  0x1D, 0, 4): one per name (0x610010, 0x43DCC0), a later one of the same
  name replacing it when its 0xCE7D8DB5 is the car's +0xDC and no less
  than the kept one's, and its 0xEB0101E2 is larger but no more than the
  car's +0xDE; 0x53ABE0 then keeps those whose 0xCE7D8DB5 is the car's
  +0xDC), else slot 0x5020C0(n) (0xB→9, 0xC→0xC, 0x16→6, …) at level 4
  (0x61BA40(car, slot, 0, 4) is 0x61B990 with the level); with kind 2
  (performance, +0x151 = 1) the unique 0x502240(n) (1→3, 2→7, 3→5, 4→2,
  5→1, 6→0, 7→8, 8→6, 9→4, 10→9) unless the car has it (car record +0x2F4
  + 4i). Items: a part 0x5210B0(icon 0x50E640(slot), label 0x50E6F0(slot),
  slot, part) — 6 0x945E9BC5, 9 0x028F7092, 0xC 0xBB034EA6, 0x1D
  0x0294D2A3, 0x40–0x43 icon 0x1B3A8DD3 / label 0xD6D115E4, else icon
  0x74CE8C0B and no label; a performance unique icon 0x50E820(i) (item
  +0x0C: the performance category icons) and label 0x50E790(i) (+0x2C:
  "Weight Reduction", "Transmission", … "Tires"). The refresh titles it
  "Unique Part Select" (0x9986A6E3 on 0x42ADB44C) and writes the selected
  item's name on +0x134: a performance unique's label; a part's
  LANGUAGEHASH (0x4732DA07) string or its name, rims "%s %s %d\"" (0x79D7D4:
  the brand by 0x50E960 of 0xEBB03E66 — **inferred** as the rim brand's
  label — the name and 0xEB0101E2). The answer 0xD05FC3A3 fits the part
  (0x55C240; slot 0x1D also 0x1E) and buys it (0x528A00: 0x514280 prices
  level 4 at 0) or adds the performance unique (0x521440: car +0x2F4 + 4i =
  1, the car rebuilt), then 0x5419D0 (state 5, the name's flag +6 = 1,
  state 0, the record cleared, markers rebuilt) and +0x150 = 1. Back
  (0x911AB364) asks 0x20D0D219 ("If you don't choose a unique…") on
  GenericDialog_SMALL: Yes (0x34DC1BEC) → 0x5419D0 and +0x150, No
  (0x1FAB5998) stays (0x538CB0). With +0x150 set the next tick
  (0xC98356BA) commits the car (0x54FD50) and leaves for the game
  (0x5404B0). Data: STAGE_2_VISUAL_1 hoods (6 for the 206), STAGE_3_VISUAL_2
  rims (5), STAGE_4_VISUAL_3 spoilers (8), STAGE_4_VISUAL_4 vinyls (5),
  STAGE_5_VISUAL_5 wide bodies (4), the three PERF challenges 3 each.
  The refresh 0x5618C0 previews the selected item on the FE car
  (0x838980): a part by 0x55C180(slot, part) — the shown ride +0x50 a copy
  of the car's +0x940 (0x413440) with the part fitted (0x631D30; slot
  0x1D also sets 0x1E, the rear rims), rebuilt (0x4D8EC0) and the stats
  refreshed (0x55B1B0) — and a performance unique by 0x521490(i), which
  sets the shown ride's flag (+0x50 + 0x318 + 4i), nothing the car shows
  (`Game::garage_request`, shot `unique_part`). What performance uniques
  do: see **Performance uniques**. **Open**: whether fitting the rims also
  fits 0x1E (0x55C240, not traced).
* **UI_MagazineReward** (**verified** unless marked; `CoverView::reward`):
  the photo shoot's engage dialog accept (0x498E70): a milestone of kind 3
  or 4 (0x5011A0) takes the shot (0x533690), keeps the car's place
  (0x500AB0) and opens UI_MagazineReward (0x50C8D0, game state 4); kinds 1
  and 2 open UI_ShowCase_Preview over the dialog (0x537990; see
  **UI_ShowCase_Preview**).
  The screen (descriptor 0x7F7FC0, factory 0x56B150 → 0x565760, vtable
  0x79D838, messages 0x55B270) hides the three layers, shows "Continue"
  (0xDE02CF84) on 0xB8A7C6CF and the button 0x69BA0838, loads
  UI_MagazineBack, poses the car (0x523D10(0)); Accept (or a click on the
  button) asks (0x55B230) "Do you want to save this magazine?"
  (0x03312F33; "… DVD?" 0xE9641A65 unless 0x838578 = 1): Yes (0x11223344)
  takes the "ScreenShot" capture three ticks later and leaves, No
  (0x11223345) leaves (0x54B0A0: layers freed, 0x838578 = 0, 0x5404B0).
  The clone draws the composed shot at `CROP` under the package
  (**inferred**: the game draws the 640 × 480 composition live). DVD for
  kind 4 is **inferred** (who sets 0x838578 here is not traced).
  **The saved cover file** (**verified** 2026-11-01, small171): Yes
  (0x11223344) sets +0x74 = 1; three ticks later the tick captures the
  shot (0x4889F0(320, 432, +0x6C, "ScreenShot") into the 512² capture
  texture of 0x832DAC, 0x488B50 sets its save flag +0x28) and once the
  capture is done (+0x29, 0x488B80) 0x523F00 hands it to the memory-card
  library: 0x4A95B0(op 2, name, 0x832DB4, the capture's texture
  0x5BA040(1)) and MC_Main (0x4A3100 mode 0xCB1, messages 0x172176E8 /
  0x36FAD7, either leaves through 0x54B0A0). The name: 0x523C80 finds the
  milestone (0x838408 records of 0x40 bytes at 0x83840C, +0x28 = the
  cover at 0x837FD8) and 0x510AF0 builds the library's 8-character name
  (the profile name 0x83A9E0 padded with '_' to 7, then the index as a
  digit or letter); 0x523CC0 → 0x510B80 shows it as "DVD %d" (0x79ACC8)
  for kinds 2 / 4 or "Magazine %d" (0x79ACBC) for 1 / 3, n the count of
  that kind among the records up to and including this one. On the PC
  the file is `<LOCALAPPDATA>\NFS Underground 2\<NAME> DVD <n>\<NAME>
  DVD <n>` (and `<NAME> Magazine <n>`), beside the profile's folder:
  **observed** in the original's own files (community saves, e.g.
  `a community save DVD 1`, `Sawa DVD 10`, `Sawa Magazine 20`; DVD 2 missing where
  that cover was never saved) and "Unable to load NAME DVD 1." for an
  unsaved one. **Format** (**verified** against those files): the
  profile's memory-card container (`formats::native_save`: "20CM", size,
  section sizes 8 and 0x100000, three CRCs, block type 0x10D) around
  0x100000 bytes, the 512 × 512 capture texture with the 320 × 432 shot
  stretched over it, rows top down, B, G, R, A (A8R8G8B8 in memory; the
  alpha bytes are whatever the back buffer held). 1,048,612 bytes. The
  profile list leaves these out (**observed** under Wine with 25 of
  them: MC_List showed the profile alone), but the boot then shows the
  list instead of loading the one profile (**observed**; the clone's
  `profiles::boot` counts the files, `list_in` skips picture-sized ones:
  the original's own filter is **inferred**/not traced). Clone:
  `LotDeal::SaveCover` → `Game::save_cover` writes
  `profiles::cover_name` with `native_save::picture` (the clone's 333 ×
  430 `CROP` read back after the cover's draw, stretched bilinearly —
  **inferred** filter — alpha 255) under the profiles' root (the clone's
  XDG data dir, never the game folder), then "Save complete." (or "Unable
  to save") and free roam. **Open**: UI_ShowCase_Preview's Yes (the
  magazine shot of kinds 1 / 2, "ScreenShot" five ticks later) writes
  none in the clone (it composes no picture there); UI_MagazineView now
  loads the file (see **Cover pose and view**).
* **UI_ShowCase_Preview** (**verified** unless marked;
  `screens::ShowcasePreview`; selftest in `selftest_customize_extras`,
  shots `showcase`, `showcase_picture`, `showcase_cycle`,
  `showcase_showoff`): the photo shoot of a milestone of
  kind 1 or 2 (half the SHOW_n_PICTUREs). Constructor 0x4CE770 (vtable
  0x78F998, handler 0x4CF170): 0x541970(0) ends a running world challenge,
  0x541940 pays the reward, 0x510C50 / 0x510C60 set the reward flag
  (0x510C30) by kind, MagazineMast / MagazineExtra hidden, +0x54 = 3. The
  options: texts 0x95AD0949..4C ("Control Car", "Show Off", "Cycle
  Cameras", "Take Picture"), boxes 0xC2EA8DE7..EA; INIT (0x35F8620B) runs
  Highlight on the selected pair and enters the menu state 0x4CEA70 (the
  handler's +0x50): Down 0x911C0A4B / Up 0x72619778 move within 0..3
  (UnHighlight on the old pair; 0x51CDB0 only toggles mouse input), Accept
  0x406415E3 runs 0x4B5C70 (Control Car, state 0x4B5A70), 0x4B5D80 (Show
  Off, 0x4B5AD0, the car's controller off, 0x5EB900(0.8)), 0x4B5EA0 (Cycle
  Cameras: 0x510710 / 0x4B5730) or 0x4CF460 (Take Picture). The menu's
  buttons (0x49A460): B8A7C6CC / 69BA0835 and B8A7C6CD / 69BA0836 hidden,
  B8A7C6CE / 69BA0837 "Select" (0x79AD657F). Take Picture: the options
  hidden (0x49A1F0) and 0xF9F1F5D0, "Back" (0x001335F0) on the second
  button, "Accept" (0x4F876B6F) on the third, FadeIn on 0x2253BD97,
  MagazineMast / MagazineExtra FadeIn (0x499E60), state 0x4CEDC0: Back →
  the menu again (0xF9F1F5D0 shown, 0x49A460), Accept → 0x533690 and "Do you want to save this
  magazine?" (0x03312F33, or "… DVD?" 0xE9641A65 when the flag is not 1)
  on GenericDialog_SMALL: Yes (0x11223344) takes the screenshot five
  ticks later, No (0x11223345); both leave (0x4B5880(1), 0x499FA0: FORWARD
  on 0x47FF4E7C, the screen closed after 5 ticks, flow event 2; it shuts
  the hood, doors and trunk (0x499F10), clears 0x836352, gives the player
  the chase camera again (0x501290(0)) and clears 0x85EF50 +0x1D). The
  menu has no Back. The picture state's Left / Right / Up / Down call
  0x4B57B0 only when 0x50D510 answers non-zero, and it returns 0: they
  do nothing.
  * **The camera** (**verified**; `chase::showcase_view`,
    `formats::magazines::Showcase`, `Game::showcase_frame`; shots
    `showcase`, `showcase_cycle`): the constructor loads
    FRONTEND\MAGAZINES_SHOWCASE.BIN (0x499EC0; chunk 0x30231 → 0x5108E0 →
    0x838574), sets 0x85EF50 +0x1D (0x5012E0(1)) and the player's camera
    to type 0x14 (0x501290(1) → 0x5F1FE0(0x14); while +0x1D is set
    0x5FA830 asks for 0x14 too; the same call frees the 12 map marker
    effects, 0x6022B0, and stops their update until the leave's
    0x501290(0) → 0x5F0C90, `Game::markers_off`): the mover 0x452520 (vtable 0x7880D8,
    update 0x44EF60 → 0x446400). Then 0x4B57B0(0, 0): the set (+0x78) is
    the setup whose hash is the cover 0x837FD8 (0x510840; 0x541940 has
    just set it to the milestone's +0x28), else the first (0x510880(0));
    0x510680 picks into +0x80..+0x90 with Rand_Int: the masthead (+0x18,
    of count +0x08), the extra (+0x20, count +0x0C), the camera (+0x24,
    count +0x10; its index +0x8C) and +0x30 (count +0x14), each list
    drawing only with a count of 2 or more (the last two with any count);
    0x4B5730 finds the camera (0x5107D0) and, when view 1's first mover
    (0x48B1E0(1) +0x44) is type 0x14, hands it the camera's nine floats
    (0x4463C0 → mover +0x2C, +0x30, +0x34, +0x40..+0x48, +0x50, +0x54,
    +0x58); 0x510800's look-up of +0x90 is dropped. Cycle Cameras
    (0x4B5EA0): 0x510710 steps +0x8C, wrapping past count − 1 to 0, then
    0x4B5730. File: u32 42 cameras, u32 1, u32 14 setups; cameras from
    +0x0C, 0x2C bytes (hash, 0, nine floats); +0xB0C one 0xC-byte entry
    (0xC2082D5F); setups from +0xC8C, 0x38 bytes (cover = bin_hash
    `MAGAZINE_SETUP_<MAG>`, 0, counts 2 / 0 / 3 / 1, mastheads ×2, extra
    ×1, cameras ×3, +0x30 ×2). The mover (0x446400): the car's matrix rows
    0 and 1 with z cleared and the ground normal N under the car (0x5E7E80)
    as row 2; row 0 = row 1 × N, row 1 = N × row 0, both normalised; the
    point looked at = car + (+0x40..+0x48) in that frame; a = +0x2C, p =
    +0x30 (set to 3 when under 3, which every stored one is) in degrees,
    as binary angles (ftol(deg × 65536) / 360, 16 bits; sin / cos
    0x43C390), d = +0x34; the eye = the point + d·(cos a, sin a, sin p) in
    the frame; Matrix_LookAt with up +z, then the world push 0x444530 (its
    third argument 0; the clone pushes as for the chase view, **inferred**
    for that argument). 0x44EF60: the field of view (camera +0xC4) is +0x50
    degrees as a binary angle, the focus +0xB8 / +0xB4 = +0x58 / +0x54 (0
    in every camera). Constructor defaults (replaced once 0x4B5730 runs):
    40.7°, −1.42°, 4.84, (0.71, 0.15, 1.26), 45°.
  * **Control Car / Show Off** (**verified**; `screens::ShowcaseMode`,
    selftest `selftest_customize_extras`, shot `showcase_showoff`): both
    let the world run (0x529590 on 0x85EF50: +0x1C = 1, flow event 2; Back
    stops it, 0x5295C0) and show only their own option (0x49A2E0 /
    0x49A3A0 hide the other three texts and boxes), the first two buttons
    hidden, "Back" (0x001335F0) on the third (message 0x911AB364); Back or
    0xB4623F67 returns to the menu (0x49A460). Control Car (0x4B5C70, state
    0x4B5A70) first shuts what Show Off opened (0x499F10); the player
    drives under the showcase camera. Show Off (0x4B5D80, state 0x4B5AD0):
    0x836353 = 1, the car's controller off (0x5EBA40(0)) and its input
    +0x214 = 0.8 (0x5EB900); Up toggles the hood (+0xE9, channel 0), Down
    the trunk (+0xEB, channel 1), Left / Right (0x9120409E / 0xB5971BF1)
    the doors (+0xEA, channels 2 and 3), each 0x61F830(car +0x554, channel,
    open, 1.0); its Back gives the player the car again (0x5EBA40(1)),
    clears 0x836353 and leaves the parts as they are.
  * **The masthead** (**verified**; shot `showcase_picture`): Take Picture
    runs 0x499E60: 0x510940(1, 1) on the stream object 0x8364F4 (pack 1 of
    0x7FA294, MAGAZINES_STREAMING_LOW, the only pack holding the showcase
    mastheads), 0x523BD0(0, masthead +0x80, 0, extra +0x84) streams them
    (+0x14 = 0x836508, +0x1C = 0x836510), and FadeIn on MagazineMast
    (0xB6638240) and MagazineExtra (0x824FA72F). The tick 0xC98356BA puts
    each loaded one on its image (0x495400 / 0x495500, once 0x836500 =
    +0x0C is set) and hides the others (every extra list holds 0). Back
    (0x4B5880(0)) hides both, runs 0x78F9D4 on them and frees the stream
    (0x510A70). The frames 0xF9F1F5CD..CF have no code reference (only
    0xF9F1F5D0 has, 0x4CEF3D / 0x4CF564), so the package's own state
    stands, as in the clone.
  * **Open**: UI_Showcase_DPAD (its name 0x799F0C has no code reference);
    Show Off's input +0x214 (0.8) and controller 0 (0x5EB940; the clone's
    car gets no controls); 0x836353's readers (0x5CFC80, 0x5EA6BA); the
    audio components the showcase flag 0x836352 switches (the clone keeps
    the placeholders in game); the picks are the clone's draws of Rand_Int
    (**inferred**, as for the covers).
* **Photo-shoot cash popup** (**verified**; `hud::Input::cash_reward`,
  `Game::cash_popup`; selftest in `selftest_challenge`, shot
  `cash_popup`): 0x541220 (the reward) calls 0x4AC170(kind, +0x26, the
  rating share) when the HUD exists (0x8900AC); with the HUD's cash
  element (+0x78, 0x4F5060: "Cash_Reward", mask high 0x80) it sets
  0x8364A8 = kind, 0x8364A0 / 0x8364A4 the two amounts, 0x8364AC = 4 and
  0x83649C. The element's update 0x4A9FF0 counts 0x8364AC down; at 0 it
  prints "%s%s: %$d" on Cash_Reward — "Bank Reward for Magazine"
  (0xABA1E56F, kinds 1 / 3) or "Bank Reward for DVD" (0x2AB10D61, kinds
  2 / 4), " " when the language (0x4FF970) is 1 else "", the sum — shows
  it, runs ZoomIn and clears 0x83649C. The clone counts the HUD updates
  in career free roam.
* **Loading tips** (traced, not ported): Loading_Tips (descriptor 0x7F8C00,
  factory 0x4D9480 → 0x4C4C90, vtable 0x790758: +8 the tick 0x4F4850)
  hides 0x03C44A2C, 0x3B35A98E, 0xB08E7468. Its first tick (+0x50) picks
  the tip (0x4C4D40) into +0x58; a tip with an image (+4) loads it first
  (0x4D9520, then 0x4D94D0 finds the screen and shows it), else 0x4A8480
  shows it at once: +8 on 0x03C44A2C, +0xC on 0x3B35A98E, and with +4
  0xB08E7468 shown and a script on 0x47FF4E7C; +0x54 = the time. The tip
  (0x4A8590) is a random record (0x43C1C0) of the 16-byte table 0x7F63E8
  (to 0x7F65E8) whose mask word has a bit of the context (0x4C4D40): 4 in
  game mode 8, 0 outside a career (mode ≠ 1); in a career's shop (game
  state 1 with 0x863444 / 0x86344C) 1 | 8 << the shop's +0x50 kind (0x401
  for the record 0x7FC8D0D0, CC_MEGALOW); in the city or a race (game
  state 4 / 5) 2, 0x2002 past the first stage's first win, | 0x800 when
  0x89E7E1 or | 0x1000 when 0x42FC40 holds, both only with the flow
  object up. Who opens Loading_Tips on PC: nobody found — "Loading_Tips.fng"
  (0x790744) has no code reference but its descriptor and 0x4D94D0's
  lookup, no hash of the name is pushed, and no load captured under Wine
  showed it (see Loading_Tips below); the clone shows PC_Loading.
* **Track loads and their loading screen** (**verified** unless marked;
  `screens::Loading`, `Game::drive_in` → `begin_loading`; selftest
  `selftest_dialogs_loading`, shots `loading`, `loading_late`): the game
  flow object (0x8900D8, made by 0x601080 at 0x57F2A5; message queue
  0x601030 (post and dispatch) / 0x5F41F0 (post), dispatcher 0x600FB0:
  message 1 sets the state +0 to its argument, then the state's handler
  from the table 0x80214C runs; 0x603640(n) posts 2 (leave the old state),
  1 (enter n), 3). States: 0 NONE 0x6037D0, 1 LOADING 0x6038F0, 2 0x603B00,
  3 0x603BD0, 4 0x60B3B0 (race in progress), 5 0x603E30, 6 0x606740, 7
  0x603EC0, 8 0x603D20 (**inferred** by order: the names RCSTATE_NONE,
  _LOADING, _CAMERA_FLYBY, _PRERACE, _RACE_IN_PROGRESS, _INSTANT_REPLAY,
  _PAUSE at 0x7A5A10… for states 0–6).
  0x5F43C0(kind) sets the flow's +8 = kind and posts message 4, which
  NONE turns into the LOADING state (byte table 0x6038D0). The kinds: 1
  from the main loop's track load (0x57F96F, after 0x60A690), 2 from
  0x5DF820 in the race state (a streaming wait: 0x5DF730's figure against
  0x7A47CC), 3 from the replay control 0x4A4BF0 (UI_ReplayControl.fng,
  REPLAY_PLAY), 4 from the state 6 handler on message 0xD (0x606AC7), 5
  from the state 7 handler (0x603FAA). LOADING (jump tables 0x603AE8 /
  0x603AD0): message 1 with +8 = 2 opens HUD_World_Loading.fng (0x7A643C,
  0x555E90(…, 0)) unless it or PC_Loading is up, or PC_Loading instead
  when 0x4AE8D0 on 0x83A9D0 and 0x5EFA10 on 0x89CF4C hold; +8 = 3, 4, 5
  PC_Loading.fng (0x790734; 5 also 0x430430 on 0x827E88, +0x45 = 1, +0x46
  = 0); +8 = 1 opens nothing. Message 2 (leaving) closes loading_boot
  (0x799FC8), HUD_World_Loading (0x558F10) and PC_Loading (0x4A9310) and
  sets +8 = 0. Message 3: 0x5E86F0 on 0x88EB00 unless +8 = 2; with +8 = 5
  it waits for 0x5D6170, then 0x437200 and 0x43BAA0 until 0x827EA4 clears,
  then state 7 (0x603640). Message 5 goes to its argument's state unless
  +8 = 5. The kind-1 screen comes from the loaders themselves: the track
  loader (TrackLoader::LoadHandler 0x57F2E0) at its step 1 (0x57F333) and
  the region loader (RegionLoader::LoadHandler 0x57DC70) at its step 1
  (0x57DCC9, unless 0x864FA8) call 0x4C57C0, which opens PC_Loading.fng
  when 0x52CF70 does not find it up; 0x57FF95 (not traced further) does
  the same, and IG_PlayMovie switches to it after the career movie
  (0x55B18F, 0x537990). PC_Loading has no screen class (no descriptor):
  its own scripts run (no screen class, so every zero-length INIT goes on
  with its chain at once): the controls group AF4A85EB's INIT (alpha 255)
  chains to 0x0016A259 (alpha 1, its strings drawn at 0), and the Event
  Handler's INIT chain 0x0013C37B (1200 ms) sends 0x6B93C93D at 600 ms
  (the controls group and "LOADING" run 5079C8F8: alpha 255 − 254 = 1 for
  the group), 0x9D4D17AA at 1000 ms (the tip strings 30E5F8D4/5 run
  5079C8F8) and 0x59BED120 at 1200 ms to target −6 (0x542070: FEngine
  +1 = 1, its input on; 0x5D4CE32D from the CB1AA27F response clears
  it). So the screen is the backdrop C51CE6F1, "LOADING" and the five
  dots (50D98304..08, each a 1600 ms looping alpha track): **the PC
  controls list never shows** (**verified** under Wine 2026-10-29, burst
  of ~11 grabs/s: the Quick Race load, the career start and Quit to Main
  Menu all show only the backdrop, "LOADING" and the dots growing and
  shrinking). The loaders are steps on the main loop (TrackLoader::
  LoadHandler queues its files with a completion callback and returns),
  so the package animates over the load. There is **no minimum time**:
  no script or code holds it — it went up and down with the load
  (lavapipe: the Quick Race load ~4 s, the career start ~4 s and then
  the scene01 movie slot (skipped here) before the airport NIS, Quit to
  Main Menu ~0.5 s before the front end's fade-in). The career start
  opens it from 0x53D720 (with "scene01" queued and IG_PlayMovie pushed
  over it). **Clone** (`screens::Loading` with `chain_init`,
  `Game::drive_in` → `begin_loading` / `present_loading`): each race /
  free-roam start (career start, quick race, free roam ↔ race) is a
  track load; PC_Loading goes up, the world and car load on a worker
  thread while the main thread keeps drawing the package (its dots
  animate), and it comes down after the load. **Deliberate difference
  (agreed deviation):** the original shows PC_Loading for
  every track load; the clone puts it up only once a load has run 150 ms
  (`LOADING_DELAY_MS`, `LoadingGate`; always before a new world's
  renderer is built), so a load that keeps the region's world in memory
  (free roam ↔ city races, garage and shop exits, a car change: see
  `World::reset_to_loaded`) or reads the asset cache switches straight
  over. The load itself (the song stopped, the state reset) happens
  either way. The game start's PC_Loading is exempt (docs/original/loading-image.md). **Not native (performance, 2026-10-30):** the clone loads
  ahead (`Preload` in game.rs): free roam's world, its renderer and the
  player's car while the menus are up, another region's race while its
  Engage dialog is up; `drive_in` takes the result instead of loading.
  The Engage dialog also loads the race's rivals' cars (`RivalsAhead`:
  the line-up 0x5260C0 and the AI dressing 0x6399E0 drawn on a copy of
  the random state 0x7EA624 as it stands, nothing drawing before them),
  and the menus that go on to career free roam (the front end with no
  drive, the post-race screens, a race's pause, a shop's screens, the
  garage, and the screens after them) load the encounter racers' cars of
  0x53F010(3) (`EncountersAhead`: its draws on a copy as it stands,
  nothing drawing before them; docs/developing/first-use-builds.md "Encounter racers
  ahead"). The race or free roam takes a car loaded ahead
  only when its key (car, parts, performance packages) is the one it asks
  for, and loads any other itself, so a wrong guess costs time, never a
  difference. The encounter racers' cars are joined at the end of free
  roam's set-up (`explore`), not in the frame that places the first one
  (a 25–60 ms stall in free roam's first second before; the game loads
  them with the track). Same calls, same arguments, same drive (bench
  dumps identical, also through the Engage dialog,
  `NFSU2_BENCH_ENGAGE=n` with and without `NFSU2_NO_PRELOAD=1`); only the
  time differs. **Open**: the shop exit and
  race → city loads were not captured (the same track loader opens it);
  the clone's quit to the main menu loads nothing and shows no
  PC_Loading (the original's ~0.5 s); HUD_World_Loading (the clone does
  not stream, so kind 2 never happens), kinds 3–5 (replay, 0x606740
  message 0xD, 0x603EC0).
* **Dialog packages** (**verified** unless marked; `screens::dialog_package`,
  `help_dialog_package`; the game's `dialog`, `ask`, `ask_cars`,
  `setup_dialog`; selftest `selftest_dialogs_loading`, unit test
  `thresholds_of_0x526c50`, shot `dialog_sized`): the generic dialog
  (0x558030 → 0x526C50 at 0x558072, result to the record's +0x324) picks
  its package from the request record: a requested name (+0x324) other
  than "" / "animating" (0x7902C8) / "3button" (0x7902D4) as is; with the
  help flag (+0x32D) HelpDialog_SMALL, or HelpDialog_MED for more than
  four lines ('\n' and '^'); else strlen(text, the record's +0) × the
  line height of CONDUITMDITC_TT21I (0x51BD70 finds the font record by
  bin hash 0x5B9D88B9 in the list 0x863768, aliases 0x7F9184; +0x24 is
  the FNTF header's +0x24 line height as a float, 0x51BDC0 at 0x51BEF3):
  "3button" GenericDialog_ThreeButton, under 2561 SMALL, from 5122
  LARGE, else MED, each GenericDialog_Animate_* when "animating" was
  asked. TT21I's line height is 32 (US data): SMALL up to 80 characters,
  LARGE from 161. Of the 0x5587A0 / 0x558690 / 0x558580 call sites 46
  name no package ("" at 0x783FE5), 24 GenericDialog_SMALL.fng
  (0x78FA24), 10 UI_InGameDialog.fng (0x795000), 4 GenericDialog_MED.fng
  (0x78F2F8), one each IG_GenericDialog_SMALL.fng (0x795484, 0x4C150B),
  GenericDialog_Small.fng (0x790D74), GenericDialog_LARGE.fng (0x790038).
  Checked against the clone's dialogs: no name (the size rule, in the
  front end and in game alike) at 0x55F117 / 0x55A1B7 / 0x55A1EB /
  0x55F144 (shop notices and questions), 0x4DFEC6 / 0x4EDC86 / 0x4EDED8 /
  0x4DFDD0 / 0x4DFE21 / 0x4EDE8B (car lot), 0x4E0FE0, 0x55A5DD, 0x49C8C2 /
  0x49C90A / 0x4A7F87 / 0x4A7FD8 (quit questions), 0x497D67
  (transmission), 0x4B1151 / 0x4B1110 (GPS on / off), 0x4FB5E8 (LAN
  search); GenericDialog_SMALL named at 0x4AF63F, 0x4AF7DF, 0x49B9A1 /
  0x4F9770, 0x56B772, 0x56B5B9, 0x55B260 (magazine) — the clone keeps
  SMALL there (`ask_in`, `setup_dialog_in`). The clone's in-game
  IG_GenericDialog_SMALL choice is gone (it was inferred). **Inferred**:
  the clone's other dialogs (shop "not enough bank", "already installed"
  for performance, …) whose push sites were not found also follow the
  rule; the text measured is the clone's UTF-8 character count (the game
  counts Latin-1 bytes). **Open**: which requests ask for "animating" (16
  push sites, e.g. 0x4A1731, 0x4BB1FF, 0x55BAFE) — the clone has none;
  help requests (+0x32D); IG_GenericDialog_SMALL's request 0x4C14C0 (a
  Yes / No on 0x39DF9562 when 0x862100 + 0x104 × 0x862838 is 0, then
  0x55DDB0(0x96DDA9E6)) is not a clone dialog. IG_GenericDialog_MED /
  LARGE, MC_List and UI_Deleteprofile have no code reference to their
  names (only the descriptor tables 0x7F8404, 0x7F83E8, 0x7F9124 /
  0x7F4564, 0x7F90EC). UI_InGameDialog.fng is requested by name at
  0x4BEC5D, 0x4BECEF, 0x4BED6F, 0x4BEDEF, 0x4C25B2, 0x4C2622, 0x4C275F,
  0x4C27D2, 0x4C28D4, 0x564CBF: **open** which in-game questions these
  are (not traced; the clone has no UI_InGameDialog). PC_ContextHelp: the
  package exists (one object) but SPEED2.EXE holds no string
  "ContextHelp" at all, so no code opens it by name.
* **Chyron (the EA Trax song caption)** (traced; ported 2026-10-29 as
  `audio::music::chyron_quads`, drawn over the menus and the HUD when a
  song starts, without the 4 s delay of 0x4A9230): not a news
  ticker. 0x4AC9F0(track record) — gated by 0x7F68B4 (set at 0x499E0C and
  0x565B44, cleared by the showcase constructor 0x4CE805 and 0x565856,
  which also hide the chyron through 0x4AC9B0) — hides the one up
  (0x8364E0) and calls 0x4AC950(title, artist, album) with the record's
  +0 / +4 / +8; the music player calls it as a song starts (0x47B02D,
  0x47B115, 0x47B20F, on the EA Trax table 0x7ECD80 + 16 × song; a test
  entry 0x4ACA40 shows "Dumb" / "Nirvana" / "In Utero"). 0x4AC950 takes
  Chyron_FE.fng (0x790014) when 0x8654A4 = 3 (the front end), else
  Chyron_IG.fng (0x790B28), pushes it when not up (0x555F60(0xE, name)),
  and 0x4AC880 writes title 0xF5387816, artist 0x11DB9E17, album
  0x48097852; 0x4AC7D0 picks the scripts by the longest of the three:
  up to 17 characters SHORT / SHORT_GO, up to 35 MID / MID_GO, else
  LONG / LONG_GO (+0x58 / +0x5C). With no delay (0x8364DC ≤ 0) it runs
  the first script on 0x47FF4E7C at once (+0x54 = 1, +0x4C = the time,
  0x8364E0 = it); else +0x50 = the time and the tick 0x4DB9C0 (vtable
  0x790B38 +8, message 0xC98356BA) starts it once the delay (seconds,
  0x4AC940; 4 from 0x4A9230) has passed, then clears the delay. 7.2 s
  (0x78E09C) after it started 0x4CB1D0 runs the *_GO script (+0x55); 1 s
  (0x78E0A0) later still +0x54 clears. The chyron screen (constructor
  0x4AC730) itself pushes Chyron_IG.fng (0x4022C0) when 0x83AA14 is set
  and the game state is not 3. Chyron_FE is one of the packages 0x49FFA0
  treats as an overlay. **Open**: the music player and its song choice
  (0x47AF..), 0x83AA14, the scripts' motion (the "scroll" is the
  package's SHORT/MID/LONG animation, not code).

## Front-end help bar, car select, track select (2026-10-29, checked against the original under Wine)

Side-by-sides (main, qrcars) and the capture steps (Quick Race mode /
track select) are kept with the maintainer's private evidence. **Verified** (REA decompiles of SPEED2.EXE) unless marked.

* **Screen records** (0x7F7DC8 UI_Main … 0x7F915C, stride 0x1C): +0 the
  package name, +4 the factory, +8 the help text hash, +0xC the help-bar
  mask, +0x10 the remembered option (1-based, 0 at start; 0x50B840 finds a
  record by name, 0x50B8A0 reads +0x10, 0x50B8C0 writes it). Masks: UI_Main
  5, UI_QRModeSelect / UI_QRTrackSelect / UI_OptionsMain / UI_StartCareer /
  UI_ProfileManager / the shops' screens 0x405, UI_QRCarSelect 0x1405,
  UI_InGame_WorldMap 0x18400 (the full table is `screens::help_mask`).
* **UI_PC_Help_Bar** (record 0x7F7EA8, factory 0x552290 sets 0x8384DC,
  constructor 0x551F90, global 0x8384D8). The constructor appends six slots
  to its list (+0x4C) in order 1..6: `button%d` (a string object) and
  `icon_%d` (an image); so button1 is the bottom right (223, 224), button2
  the top right, button3 / button4 the top row to the left. 0x54E6B0 turns
  the bar on (+0x6C = 1, 0x5429A0(1)); UI_Main's constructor calls it at
  0x4DF2ED; 0x54E6C0 turns it off (six in-game callers). Every screen's
  base constructor (0x551DF0) calls 0x54E6F0(record +0xC) while the
  package is up. 0x54E6F0 (only while on): writes the profile name into
  `EB406FEC` (0x537B90 = sprintf into a string object, "%s"), hides every
  slot, then gives each set bit low to high (skipping 0x8000, 0x10000,
  0x20000, 0x80000000) the next slot with label 0x52D340(bit): 1 "Quit
  Game", 4 "Continue" (the fallback), 0x400 "Back", 0x1000 "Customize",
  0x10000000 "Activate GPS" … (`screens::help_labels_in`). With neither
  0x8000 nor 0x10000 it shows "RealGroup" (0x3B949CD0: the name, the cash
  and their backings) and its children. The cash string's INIT chains to
  HIDE; only the career (game mode 1, 0x537E70 → 0x51DDA0) fades name and
  cash in turn. Clone: `Game::help_bar_quads` (main menu, Career, QR mode
  / track / car select, Options, Profile Manager, Customize categories).
  Main menu: Continue / Quit Game; QR car select: Customize / Back /
  Continue / Quit Game — as in the original. The career's screens (name /
  cash alternation), the in-game layouts and the flag-0x40 rule: see
  **Help bar in the career and in game** below. **Open**: OL_ICON_GROUP
  (masks 1 / 2 online), the res-2 backings A444D16E…, the mouse.
* **UI_Main's first option** (0x4AEE30): the record's +0x10, set to 2
  (Quick Race) when 0; IconScrollerMenu_Handler writes the 1-based index
  back on every move (0x543F47 / 0x543F99). So a fresh run opens on Quick
  Race (clone: `Game::main_selection`, per session like the record).
  The fresh profile's car is DDAY_PLAYER_CAR (g_CareerHasOwnCar clear,
  0x4DF140) — the clone already did that (`--shot main_dday`); `--shot
  main` uses a profile with a Peugeot.
* **Icon strip scroll bar** (IconScroller +0x38, "ScrollBar" set up by
  0x542AD0 at 0x543AF9): 0x538090(1, total, position) at 0x538993 /
  0x538B28 (args from +0x28 − 1 and +0xC8; measured against the original:
  Quick Race, 2 of 7, sits 1/6 along, so total = the option count). 0x538090
  hides handle and arrows when total ≤ visible or total 0, else shows
  them (the handle only with a position) and places the handle (0x52E330: the start + h/2 for the first,
  end − h/2 for the last, else start + h/2 + (L − h)/(total − visible) ·
  (position − 1)). Then 0x52FA80 (verified 2026-10-29): Arrow_1 (scroll
  bar +0x48) hides on the first option, else Arrow_2 (+0x4C) on the last,
  else both show — the clone used to show the left arrow on the mode
  select's first option. Clone: `IconMenu::quads`.
* **UI_QRCarSelect** (class 0x4F9D90, refresh 0x4B2310):
  - OL_CarMode_Group (the "Race Mode:" panel) is hidden in every package
    but UI_OLCarLot.fng (0x4F9D90; the online modes 8 / 0x100 with the
    Ranked filter show it). The clone showed it.
  - ScrollBar_Arrow_1 / _2 (the logo box's left / right arrows) hide on the
    list's first / last car (0x4F9D90 and 0x4B2310, flags +0x1A5 / +0x1A6);
    the list scroll bar is 0x538090(1, count +0x60, position +0x64).
  - The help mask is +0x20 (0x1405) less Customize (0x1000) in game mode
    0x20, for filters 4 and 8, and for a car in a career slot (0x516340);
    mode 0x20 adds 0x40000 (Delete Tuned Car) for a tuned car not in a slot.
  - Rating bars: 0x497BF0 builds three sliders (base 0x4FF430, vtable
    0x78F544) with 0x540C20 from {package, `<x>_backing`, `<x>_handle`, 0,
    `<x>_cur`, 0, 0, min 0, max 1, value 0, step 0, pad 46}; 0x497CD0 sets
    each (0x4FF550 clamps to [min, max]) to Perf_AccelRating /
    Perf_TopSpeedRating / Perf_HandlingRating of the car and calls vtable
    +0x18 (0x4FF530 → +0x20 = UI_Slider_Layout 0x540D30): track = backing
    width − 92; handle top left (f·track − hw/2 + 46 + bx, bh/2 + by − hh/2
    − 1); `_cur` f·track wide (0x495590) at (bx + 46, bh/2 + by − hh/2 + 1)
    (0x51D080 / 0x51D390: an image's top left is position − size/2). The
    clone scaled the 52-px authored fills instead (Handling a quarter of
    the original's).
  - **Open**: the logo box's green focus frame is brighter in the clone.
* **UI_QRTrackSelect** (constructor 0x4E2F30, refresh 0x4CE170, race type
  0x4988C0): "Track Number DEBUG" (0x53C956C4) hidden (the clone showed
  "1 of 98"); race type = the mode's label alone (g_RaceOptions 0 Circuit,
  1 Sprint, 2 Drag, 3 Drift, 4 URL Race, 9 Free Run, 10 Street X); distance
  "%$.2f %s" of the track's +0x98 metres (int) × 0.001 (× 0.62137, "mi",
  imperial; "km" metric); record label "Best Time:" (sprint, drag), "Best
  Lap" (drift, "%$d" of the best score, 0x502C50), else "Best Lap:", value
  FormatRaceTime(best, 0) (0x502AA0: the profile's best for track and
  direction, else the track default via 0x5D3E50; 0 prints "0.00"
  (0x7A0628), under a minute "ss.cc" with a leading space, then " m:ss.cc").
  Arrows hide at the list's ends.
  - **The list** (**verified**, `quick_race::track_list`; 2026-10-29):
    0x4CDED0 walks the track table (0x5D3E90: base 0x883DA0, 0x128 bytes,
    count 0x883DA4) and adds (0x4B4C10) every track 0x4987C0 lists for the
    race type: +0x88 clear and the type's bit of +0x94 (0 bit 0, 1 bit 2,
    2 bit 1, 3 bit 3, 4 bit 13, 5 bit 12, 8/9 bit 4, 10 bit 11, 12 bits 0|2).
    A node (0x30 bytes) is locked (+0x08) unless 0x5156F0(1, track): the
    cheat byte 0x838464, or 0x5143C0 — some career event (0x4FFCF0 over
    0x8383E0, 0x88 each) whose first race on the track (0x514340) has race
    byte +0x1A bit 1, or that is won (its +0x08 in the state table
    0x862840 with +6 = 1). Then 0x43E650 with 0x4987A0 (a stable pass
    that moves an open node ahead of a locked one): open tracks first, the
    locked after, each in table order. A fresh profile's Circuit list:
    4014 Resort Loop, 4001 Outer Ring, then the 29 locked (4011 Freemont,
    4012 Providencia, …) — the capture's first two icons. Names are
    TRACK_NAME_<n> / REVERSE_TRACK_NAME_<n> (0x516280; 0x79B118 /
    0x79B100), not the record's "(4014) Short Circuit #4". The screen opens
    on the record's track (0x502910 +0x08) when it is listed and open,
    else on the first, whose number the record takes (0x4CDED0); Lap KO
    is cleared on opening (0x4E303A). Left / Right (handler 0x4EFA10) stop
    at the ends; each move writes the track and direction 0 into the
    record (0x498740); Accept does nothing on a locked track.
  - **Icons** (0x4984B0): locked 0xF05704AC; by track +0x7C 0 0xB5A633A0,
    1 0x79CEAD40, 2 0x4B938E3C, 3 0x4FDE6FEB, 4 0x6BA31820, 5 the mode's
    Track_Icon_Drag / _Drift / _U_Racing_League / _Short_Track; else (and
    for a missing texture, 0x4CDED0) 0x74CE8C0B. They run on an
    IconScroller (0x5435E0 with ICON_MASTER 0x1CF6F873 and TRACK_ICON
    0xF5FF3DBC templates, gap 40 at screen +0xBC); the slot backing is
    687FDA31 (U2_CarSelect_icon_backing). Clone: `screens::QuickRace`.
  - **Map**: the widget (screen +0x118, vtable 0x78F8C8, the World Map's,
    tick 0x4CDD40) shows TRACKMAP<n>; the screen's tick hands it the
    selected node (0x498600) once 0.3 s passed after a move's stamp +0x14C
    (0x4EFC30). **Inferred**: the opening's track is handed at once (the
    capture's first frame shows TRACKMAP4014 unzoomed; how it gets there
    is not traced). QR_TRACK_%d_COMMENT goes to 0x258E6A24, which the
    package lacks.
  - **Highlight** (**verified** under Wine, 2026-10-29): the selected
    slot's U2_Icon_Highlight (0xDF422345; GlobalB U2_ICON_HIGHLIGHT, P8
    128×64) is not touched by this screen's code (0x4E2F30, 0x5435E0; the
    screens that run its "Animate" by hand are others: 0x4B03BD, 0x4DFCEE,
    0x4EDA6A, …); its INIT (length 0, z −82) chains to Animate (0x5A8E4EBE,
    1200 ms, chained to itself: alpha 255 to 320 ms, down to 0 at 640 ms,
    0 to 1200 ms, z 1), so it pulses over the dark backing. Eight shots of
    the original 0.15 s apart (kept with the maintainer's private evidence) show the green frame on and off; the
    earlier capture (qr66/qrtrack.png) was taken in the off phase. The
    clone draws the same (no change). The best times (0x502AA0 / 0x502C50, per track and
    direction) are not kept (the clone shows 0.00).
* **UI_QRModeOptions = "Quick Race Game Options"** (**verified**,
  2026-10-29; `quick_race::QrOptions`): the next screen after the track
  for every type (0x498240, jump table 0x498270 with both entries
  0x498269 → 0x7F4460) and Back's (0x4B2AF0 → UI_QRTrackSelect); its
  Continue goes to UI_QRCarSelect (0x498220). Class 0x4EF3F0 on the
  option list (0x552790), factory 0x4FA550; title 0x47597606 in game mode
  2. Rows (0x4E2C70 by type): Circuit 0x4CCF60, Sprint 0x4CD2D0, Drag
  0x4CD530, Drift 0x4CD7A0, URL / 5 / Street X 0x4B47B0 — see
  `quick_race::rows` for the orders and conditions (Track Direction only
  on +0x86 = 1 tracks; no Traffic on drags 4211–4223; Drift's +0x80 = 1
  tracks take Traffic and no opponents). Items: Track Direction 0x4B40B0
  (vtable 0x791608: Forward 0x9E4C4648 / Reverse 0x22F254AF, record
  +0x0C and 0x89E7A4), Lap KO 0x4B3FA0 (0x7915C0; on → laps 3,
  opponents 3), Laps 0x4B3D30 (0x791530: 1–10, record +0x0A), Traffic
  Level (0x791458: None / Minimum / Moderate / Maximum, +0x04), Opponents
  0x4B3750 (0x791380: 0 … 3, 5 for URL, 2 on drag 4402, less players − 1,
  +0x01; the constructor stores 2 when Quick Race's drag on 4402 holds
  more, nothing else is clamped; key handler 0x4B37F0: Left wraps only
  below the lowest, Right only above the highest, so Left from above the
  highest steps down), Difficulty 0x4B3930 (0x7913C8: Easy / Medium / Hard, +0x02),
  Catch Up (0x791410, +0x05). Laps and Opponents disable while Lap KO is
  on. The record (0x83AAB4 + 0x10 + 20 × type; constructor 0x52A230:
  players 1, opponents 3, difficulty 1, traffic 1, catch-up 1, laps 3).
  The list's scroll bar: 0x538090(1, count − 1, 1-based position)
  (0x539337 …).
* **Quick Race set-up** (**verified**, 0x53F160): human racers +0x00,
  AI racers +0x01 (0x89E7C4), traffic +0x04 (0x89E7A8), difficulty +0x02
  (0x89E7F4), catch-up +0x05 (0x89E7F8), race type 3 for a circuit with
  Lap KO in game mode 2, laps +0x0A (0x89E7BC), track +0x08, backward
  +0x0C (0x89E7A4); the opponents 0x53EEA0(n): names 0x4FE970 (n of the
  53 at 0x7F6CD0, Rand_Int(53) up to 100 tries each), cars
  0x4FEAF0(record, 100 + i, 2 + i, 1, the player's car, 0, −1) — the pool
  for the player's car, its type allowed, against its rating — with skill
  rows 0x4FED00(difficulty). Clone: the record races as an event in a
  reused slot (`Game::quick_event`), Lap KO's knockout in `Race::knock_out`
  and the catch-up into the racers' speed (`CatchUp` from the race).

## Front-end particles, performance brakes, shadow pull (2026-10-29)

Side-by-sides are kept with the maintainer's private evidence. **Verified** (REA
decompiles of SPEED2.EXE) unless marked.

* **Front-end particles.** A platform file is an ordinary bundle: its
  emitter descriptions (0x35021) join the resident list 0x830D08 after
  GLOBALB's (0x491C70) and its placed emitters (0x35020) are bound by name
  hash and started (0x48BC00, 0x48BC50). SHOWROOM.BIN holds one,
  D4884E83 `XEFE_Showroom_SmokeA_01` at (−3.57, 9.55, 0), state 1, no
  switch: 10 particles a second, life 6 s, speed 0.75 ± 60 %, cone 60°
  (mode 0), gravity −0.3 (rises), drag 0.2, texture SFX_SMOKE_B_CS
  (0x0DE39984), which only FrontB.lzc and InGameCommon hold (the front
  end's resident pack is FrontB). GarageMain's update FEGarage_UpdateCarTurn
  (0x4D82D0, at 0x4D8A48 while its +0x1C8 is 0) runs Particles_Update
  (0x492E10) with the frame's dt, and the frame's particle draw
  (Particles_Render 0x48BDF0, 0x5CAEC8 / 0x5CAED3) shows them: the steam
  plume left of the car on every SHOWROOM screen. Clone: `Platform::load`
  fills the platform world's `emitters` / `placed_emitters` (GLOBALB's
  descriptions, then the file's; the missing textures from FrontB.lzc) and
  `draw_garage` runs and draws them (`GarageScene::fx`); a `--shot` starts
  as the scene stands `NFSU2_SHOT_MS` after the menu asked (at most 10 s).
  The other platforms' emitters are loaded the same way (not compared).
* **Particle direction, mode 0** (+0x18 = 0; 0x492539…0x492633,
  correcting the earlier "every way"): the three turns about x, y, z are
  each `rand · cone − cone / 2` degrees (+0x50; [esp+0x50] = +0x50,
  [esp+0x58] = +0x50 · 0.5 at 0x4920FF / 0x492148, × 65536 (0x784238)
  through `_ftol`, / 360 into the 16-bit angle), applied to the emitter's
  z axis: a jitter of ±cone/2 per axis. The clone drew a full turn per
  axis, so the steam spread along the floor instead of rising.
  **Inferred**: the order the turns compose in, the exact draws.
* **Brakes from the performance parts.** 0x6399E0 (run by 0x639CF0 —
  0x4D80F2 … 0x521410, whenever a ride's parts or performance change, and
  from GarageMain when 0x8A1CD0 is set) sets, with ride +0x2F0 the
  performance block (fitted flags at +0x50 + id, 0x599620): slot 0x1B
  (ride +0x5FC) to the car's part of level 1 when part 0x28 (the level-1
  brakes' third) is fitted, else 2 with 0x29, else 3 with 0x2B, else level
  0 (0x61BC90: the first part of the slot at that level, 0x61B990; none
  keeps the slot); and slot 0x0D (+0x5C4, ENGINE) to level 3 when the
  level-3 engine, ECU and turbo packages (categories 3, 7, 4; 0x599640:
  every part of the package fitted, or no such package) are all on, else
  2 for the level-2 ones, else 0. Then 0x61BCE0 refreshes the shown slots.
  The brake draw (0x62DF85…0x62E091) takes all four brakes from slot
  0x1B's models (+0x1B08 + 0x10·group + 4·lod): front group 0; rear
  group 1 when the part is level 0 (the stock brakes' rear model), else
  group 0 (BRAKE STYLE01–03 have one group); slot 0x1C is never drawn.
  Rachel's 350Z (performance level 2: parts 38, 39, then 41 replacing 40,
  42 — see the preset note) so shows BRAKE STYLE02 (C6578696, cross-drilled
  disc and caliper), as the original's main menu does; the clone showed
  the stock disc. Clone: `car::perf_visual_parts` (in `Car::load_tuned`
  for every in-game car, and for the front-end car in
  `Game::garage_request` with the profile's or the preset's parts),
  `Preset::perf` now fits as 0x5A64B0 does. Test
  `rachels_brakes_come_from_the_performance_parts`. **Remaining**: the
  disc is darker than the original's (its lighting, not traced).
* **Shadow pull**: see Car shadow (player 0's camera mover type).
* **Sky cycle**: see Sky, "Shipped behaviour" — 0x870D14 is 1 at run
  time (the options object's static constructor), the cycle runs, and the
  clone now runs it by default.
* **Quick Race car select, floor reflection (view 4), checked against the
  original's render target** (d3dretrace of t1.trace under Wine with
  `-S 10639750`, the 320 × 240 target after the car's opaque parts:
  wine/compare/fe72/qrcars-reflection-view4-orig-left-clone-right.png).
  The clone's image matches it in content and placement: the flipped
  BACKDROP, PLATFORMBASE's hazard ring (ARC_STRIPE_BLACK_YELLOW; the
  "white ring" in earlier `NFS_REFLECTION_PNG` dumps was only the
  vertices' alpha 0 shown as transparent, the colour is right) and the
  mirrored car. The weaker wet-look streaks on the floor are therefore
  **not** a missing pass: the differences left are the bright spots the
  original's image has under the car and bottom left (likely its blended
  car parts and the two alpha-tested fans drawn first, texture 0x630C880,
  still **open**) and one highlight the clone has on the right; the
  worldreflect formula itself is unchanged.

## Help bar in the career and in game, package init, in-game dialogs (2026-10-29)

Checked against the original under Wine (a private Wine copy,
capture.py with PEEK; side-by-sides kept with the maintainer's private evidence:
`orig/` the original, `clone/` the clone, `before/` / `after/` the clone's
front-end screens before and after the flag-0x40 rule). **Verified** (REA
decompiles and the objdump) unless marked.

* **Package init hides flag-0x40 objects, in every package.** 0x54E4B0
  ("Init Package %s") runs the visitor 0x51C870 (vtable 0x79AB94) over the
  package: an object flagged 0x40 gets 0x50CA00 (flags |= 0x400001 — the
  hidden bit, down a group's children too) and loses 0x10000000. On PC that
  drops the console footers ("Select", the $JOY_EVENT_ glyphs) and the
  screens' own name / cash group (e.g. UI_CareerCrib's group 000E53DA,
  flags 0x40000040, holding EB406FEC / EB3A688A): the clone drew "$0" there
  on the garage and the shops. The clone applies it in `layout` (`hidden`:
  flags & 0x41, unless the screen's code shows the object). The screens'
  base constructor 0x551DF0 writes the package's own EB406FEC only online
  (0x8500D0 ≥ 0xC).
* **The career's name / cash alternation** (`screens::HelpCycle`): see the
  doc comment there. 0x537E70 (from the bar's constructor 0x551F90 and
  from 0x54E6F0 in game mode 1 for a mask with bit 1 and not bit 2) →
  0x51DDA0(name, cash, 0.75, 5.0): +0x34 name, +0x38 cash, +0x3C the one
  shown, +0x40 0.75 s, +0x44 5.0 s, +0x48 the countdown in 0.25 ms ticks
  (× 4000 + 0.5, 0x577320 counts it down by g_FrameDt × 4000); Fade_In on
  the name, Fade_Out on the cash; texts "%s" of the profile name and
  "%$d" of 0x861E74 (grouped, no '$': the HUD's Bank shows "0" the same
  way under Wine). The tick 0x537F60 (on 0xC98356BA, handler 0x55BBDD)
  swaps them when the countdown runs out: the cash for 5.0 s, the name for
  0.75 s, each switch Fade_Out / Fade_In (0x54C20A66 / 0xBCC00F05, 320 ms).
  The cycle starts once (only while +0x3C is 0) and runs on across
  screens. g_GameMode (0x850078): 2 from UI_Main's constructor (0x4DF18B),
  1 at the career's start (0x495A3D, 0x4C5033), 0x20 Customize, 8 online,
  0x200 LAN; the clone keeps it in `Game::game_mode`.
  **Inferred**: the cash shows 5 s and the name 0.75 s (the constants'
  order is as traced; no capture of a career screen with the bar yet —
  the D-Day intro has none reachable without driving to the car lot).
* **The bar on every screen with a record mask** (`Game::help_screen`):
  the clone drew it only on a few front-end screens; now on every menu
  whose record (0x7F7DC8 …) has a mask, the in-game ones included
  (UI_Pause / UI_PauseOptionsMain / UI_PostRace 0x30404, UI_PauseOptions
  / UI_PostRaceStats 0x10400, UI_EngageMessageDialog 0x18404,
  UI_SMS_Mailbox 0x1A500, UI_PostRaceResults / TournResults 0x80010004,
  UI_PostRace_TournStandings 4, UI_PostRaceReward 0x404, UI_SponsorPopup /
  UI_Sponsorship_new 0x400). Under Wine the pause shows Back / Continue,
  the mailbox Read Message / Delete / Back, the message Continue / Back.
  **Open**: UI_EngageMessageDialog has a second record (0x7F8BE4, mask
  0x12400: Back, Delete); which one a message from the mailbox uses is
  not traced (the clone uses the first's).
* **In-game slot layouts.** 0x5428C0(slot, mask) runs, for each labelled
  slot, a script on its text and icon (0x51DBF0, which also keeps the name
  at slot +0x20): with 0x10000 "In_Game" (0xC3EB04AF) for 0x8000, else
  "Event_Results" (0xFCC020D2) for 0x80000000, else "Pause_Options"
  (0x2B5A03A8) without 0x20000, else "Pause_Menu_Main" (0xA95DB9D5);
  without 0x10000 "Init". In_Game puts slot 1 bottom left and slot 2 bottom
  right (the World Map's Back / Deactivate GPS); Pause_Menu_Main puts them
  in the pause panel (Back left, Continue right). 0x5429A0(on) shows /
  hides the slots and RealGroup.
* **Help bar labels**: 0x52D340 picks 0x800's label by g_IsDynoRun
  (0x89E7E6, read at 0x52D43F), not 0x8E26CC. 0x52D180 maps the help-bar
  bits to FE messages; 0x400000 "Tip" → 0xC519BFC3 (Pad_Button4).
* **HelpDialog_SMALL / _MED / _LARGE**: MenuScreen::BaseNotify (0x55B73B)
  on Pad_Button4 (0xC519BFC3), when the package's control mask takes it and
  the screen's help hash (+0x28, the record's +8) is set, opens 0x558AD0 —
  a request with +0x32D set, which 0x526C50 sizes into HelpDialog_SMALL /
  MED (more than four lines) — with that text (console help with
  $JOY_EVENT_ glyphs). On PC no keyboard row binds FENG_BUTTON4 or
  FENG_HELP, so only a mouse click on the help bar's "Tip" slot reaches it;
  the clone has no mouse, so it has no help dialogs. Back (0x911AB364)
  closes them (0x4A3330 sends it to both packages).
* **GenericDialog_Animate_SMALL / MED / LARGE**: 0x526C50 picks them for a
  request whose style (+0x324) is "animating" (0x7902C8). Every caller
  passing it is an online wait ("Searching...", "Retrieving stats from the
  server.", "Joining game '%s'...", 0x4A1700, 0x4BB0F0, 0x4BCD84, 0x4D4240,
  0x4E4D5E, 0x4F17D5, 0x55B73B …): not used offline.
* **Loading_Tips**: its name (0x790744) is read only by its own texture
  callback 0x4D94D0 (0x51BD10 finds the package; registered by 0x4D9520
  through 0x494D30 for image 0xB08E7468). No code pushes it by name; its
  factory 0x4D9480 builds only with 0x836480 set (0x4A83F0, from
  0x537860). The PC loading screen is PC_Loading (0x790734, 0x4A9310).
  **Inferred**: Loading_Tips is never shown on PC (none of the four
  loads captured under Wine on 2026-10-29 showed a tip).
* **UI_Showcase_DPAD**: its name (0x799F0C) has no reference in the code.
* **UI_InGameDialog** (`Game::in_game_question`, `Game::in_game_dialog`):
  the in-game options ask first through 0x5587A0 with
  "UI_InGameDialog.fng", OK 0x0639 / Cancel 0x541389E5, the focus on
  Cancel (the original under Wine: Cancel highlighted, Left moves to OK);
  OK sends the option's message, Cancel 0xB4623F67. UI_pause: Back to World
  Map 0x4C2560 ("Back to the World Map?" 0x33C42E03, racing
  0xE761C8B8 "…^All unsaved race progress will be lost."), Restart
  0x4C25D0 (0x0972A44B / racing 0xAA50AA02), Jump to Garage 0x4C2730
  (0xC9BA9E3E), Return to Explore Mode in a race 0x4C2780 (0x60B42AFB /
  racing 0x07CD17B0), Quit to Main Menu 0x4C2820 (offline: racing
  0xE15B558A, else game mode 1 0x6F4796A6, else 0x30ED2368; online
  variants for 8 / 0x200). "Racing" is the race camera 0x8900D8 in state
  6 with +0x24 = 4, which holds in free roam too: the explore pause's
  Quit asks "Quit to Main Menu?^All unsaved race progress will be lost."
  under Wine, and its Return to Explore Mode resumes at once. UI_postrace:
  Return to Explore Mode 0x4BEC20 (0x6C328734 "…tournament progress will
  be lost." while 0x503080 has a race left, else straight on with
  0x3411395C), Restart 0x4BECC0 (0xAA50AA02), Quit to Main Menu 0x4BED40
  (0xE15B558A), Back to World Map 0x4BEDC0 (0x33C42E03). The clone used
  GenericDialog_SMALL "Are you sure you want to quit to the main menu?"
  (0xCB5E3178) and asked nothing for the others. Shot `pause_quit`.
* **The explore pause's "Replay"** (0x4C2900): after Options (shown when
  0x4A7BF0 holds) and outside game modes 4 / 8 / 0x200 it adds "Replay"
  (0x84D1D864 / 0x7840428A, class 0x4C2640, vtable 0x7954F0), greyed
  (+0x20) unless the replay manager 0x8650B4 holds a recording (+0x50EC,
  +0xB8 > 0, 0x88F1A8 clear); Accept (0x4C26B0) checks the same. The
  original's pause under Wine: Return to Explore Mode, Options, the greyed
  Replay, Quit to Main Menu. The clone adds it as unavailable (free roam
  holds no recording). **When it is enabled** (2026-10-31, **verified**
  in 0x57AAE0 / 0x60B527 and read under Wine in a career's free roam):
  recording starts only when the flow sees race status 4 (once, flow
  +0x3C), outside game modes 8 / 0x200, and 0x57AAE0 needs a race type
  (0x89E7B0 ≠ 0, ≠ 7), not split screen (0x89E7C0 ≠ 2) and 1 MB free
  for the input buffer. Free roam has race type 0 (read: 0x89E7B0 = 0,
  race status 4, manager +0x78 = −1, +0xB8 = 0), and a race reloads the
  track (a new manager), so the explore pause's Replay never has an
  INSTANT_REPLAY: always greyed. (0x88F1A8, the streamer's held temp
  buffers, greys it too.) Jump to Garage's flag 0x8635A8: see **Jump to
  Garage?** (offered after D-Day; its setter is open).
* **Generic dialogs' header**: the header 0x426C7B4D ("Memory Card") and
  its icon 0xC26A8D5C INIT into HIDE; only the memory-card screens show
  them (0x4BEBEB, 0x4D6FD4). The clone's `Dialog` now follows the INIT
  chains (the dialog group's own INIT → HIDE comes back with the Event
  Handler's FADEIN), so no "Memory Card" title on other dialogs.
* **SMS mailbox rows**: the list (0x51F2F0 add, refresh 0x538510 → its
  vtable +4) hides a slot past the items whole; the clone hid the text and
  icon but drew the HIGHLIGHT_n backing (the original under Wine shows no
  backing under empty rows).
* **In-game World Map legend focus**: 0x4B0E70 → 0x496C50 / 0x496B20 /
  0x496BC0 send the row's text (row +0x14, Data_NN) the *messages*
  Button_Highlight / Button_UnHighlight / Button_Disable (0x51CDD0); the
  text's MsgR runs Highlight (0x249DB7B7) / Disabled (0x06B55037) on
  itself and forwards them to the row's bar group and DATA_A. The clone ran
  a script named Button_Highlight, which the objects lack, so the focused
  row showed no green bar (the original shows it on Car Lot).
* **World under the in-game screens**: the World Map key's map, the SMS
  screens and the dialogs raised in game draw over the paused city (the
  original under Wine); the clone drew them on black. 2026-11-04: the
  post-race Sponsorships page too, and the world is not dimmed under any of
  them (frame 0x5D2860 has no FE gate; Wine pause frames identical outside
  the panel). See `docs/notes/front-end/fe-backdrops.md`.
* **Open** (left from the menus audit): the career name / cash capture;
  the mailbox's second message record; the pause's Jump to Garage flag;
  the mouse. (The World Map's selection ring with the GPS on is resolved:
  see the In-game World Map's selection pulse in docs/formats/cars-2.md.)

## Gamepads and joysticks (2026-10-31; **verified** from the assembly unless marked)

Clone: `src/pad.rs` (the native model), `src/pad/host.rs` (gilrs and the
synthetic script), game.rs `pad_frame`, `controls`, `controls_cells`;
the maintainer's probe `pad_probe` lists what the game sees.

* **Enumeration and channels.** 0x5D0320 (the input system 0x870760's
  constructor) → 0x5D0080: DirectInput8Create (IID_IDirectInput8A 0x7BFDA0)
  into 0x8709EC, EnumDevices(DI8DEVCLASS_GAMECTRL 4, 0x5CEF10, —,
  DIEDFL_ATTACHEDONLY). 0x5CEF10 skips guidProduct 0x0802050D / 0x0805050D
  and the product names "Nostromo n40 Gamepad" / "Nostromo n50 Speedpad"
  (0x7A4154 / 0x7A413C), else 0x5CD200: a device already on a channel is
  skipped (GetDeviceInfo, guidInstance compare), at most two channels
  (0x86FFB4[2]); CreateDevice, SetDataFormat(c_dfDIJoystick2, 0x7BFDE0:
  0x110 bytes, 164 objects), SetCooperativeLevel(0x870990, exclusive |
  foreground 5), EnumObjects(0x5C8670): every axis object gets
  DIPROP_RANGE −10000..10000 (by id) and nothing else (**no DIPROP_DEADZONE
  or saturation**), objects with DIDOI_FFACTUATOR count into 0x86FF80[ch]
  (max 2). guidProduct → 0x870728 + 16·ch; the type 0x5BFB70 (the per-product
  table 0x800588: 044F:04B2 → driving 0x16, 06A3:FF0C → gamepad 0x15, else
  the DIDEVICEINSTANCE type's low byte) sets 0x86FF88 (joystick 0x14),
  0x86F8A8 (gamepad 0x15) or 0x86E5BC (driving 0x16; autocenter off,
  DIPROP_AUTOCENTER). A failed open releases the channel (0x5BFA60).
* **Polling and unplugging.** Each frame (0x5D2C70..0x5D2C93): 0x5C0FA0
  detaches records whose channel lost its handle (+0x230 = −1, mask all
  ones, analog 0, 0, 0x80, 0x80, count 0x870764 − 1), 0x5CA6A0 gives a
  channel with no record the first free record of six (0x874C40 + 0x370·k;
  0x5C1500), 0x5CFA20 polls: every 32nd call re-enumerates **only when
  0x8364E8 is set — no code writes it** (static search; so the original
  does not pick up a pad plugged in later: **inferred**), then 0x5C8710 per
  channel: Poll, Acquire on failure (and DIPROP / actuators on for FF
  wheels), GetDeviceState into 0x86FFC0 + 0x110·ch; DIERR_INPUTLOST
  (0x8007001E) releases the channel. On channel 1 with the EMS PS2 adapter
  (0B43:0003, 0x8005B0) lX, lY, lZ, lRz all at 10000 read as 0. The two
  states are copied to 0x870760 +0. Clone deviation (hot-plug support):
  gilrs's Connected / Disconnected attach and detach at once (hot-plug),
  and an attached pad gets the profile's block for its GUID.
* **The record** (0x874C40, 0x370 bytes; keyboard 0x5C1090 type 0x13): +8 a
  64-bit action mask, active low; +0x10..+0x13 the analog bytes (throttle,
  brake, steering, combined); +0x18 / +0x98 / +0x118 / +0x198 four tables of
  32 action ids (buttons 0–15, then POV k's down, up, left, right at
  16 + 4k): the driving map, the FE map and two more (no reader found that
  picks one; the poll clears the bits of all four); +0x218 the map mode
  (0x5CF330); +0x230 the channel; +0x234 the type; +0x33C the action being
  bound, +0x340 its column; +0x344 four axis offsets (roles: 0 steering, 1
  throttle, 2 brake, 3 the combined pedal axis), +0x354 their signs,
  +0x364..+0x367 their flags, +0x368 the pedals are combined; +0x36C
  dwPOVs. The Controls kind (0x5B8730): 0x13 → 1 keyboard, 0x16 → 2 wheel,
  0x14 / 0x15 → 3 joypad.
* **Defaults** (0x5C1500 → 0x5C11B0; also Restore Defaults). Attach:
  axis[0] = lX, axis[3] = lY, signs 1, −1, −1, 1, flags off, combined on.
  0x5C11B0: GetCapabilities (+0x36C = dwPOVs, 0x87078C =
  DIDC_FORCEFEEDBACK); with axes: steering lX, sign 1, on; the combined axis
  (sign 1, flag 3 and +0x368 on): a wheel lY when it has lY and no lRz / lZ,
  or slider 0 when it has slider 0 and lY and no lZ (else none: its pedals
  are bound by hand); any other device lY when it has fewer than four axes,
  else lZ for the EMS adapter, else **lRz**. Then the buttons (tables 1–2's
  first 16 cleared): a product in the preset table 0x7FE7F8 (0x5B81C0, nine
  entries of 0xB0: guidProduct, 20 driving, 20 FE entries — Logitech CA03,
  C216, C21A, C218, C20A, EMS 0B43:0003, 05FD:3002, Saitek FF0C, Microsoft
  0027; `pad::PRESETS`) gets its entries and nothing else; else by
  dwButtons: under 6 — buttons NOS, e-brake, shift down, shift up, look
  back; 6–7 — brake, throttle, NOS, e-brake, shift down, shift up, look
  back; 8 and more — brake, throttle, look back, camera, NOS, e-brake, shift
  down, shift up, hydraulics pressure, hydraulics bounce. FE: Select, Back,
  Customise (BUTTON1), Button 2, Button 3. Every POV (tables 1 and 3):
  driving (brake, throttle under 6 buttons, else none), steer left, steer
  right; FE Menu Down / Up / Left / Right (tables 2 and 4). No pause, no
  pad start in the generic defaults.
* **The poll** (0x5C1850, a controller record, not while 0x870768 waits
  for a binding): each of buttons 0–15 held (byte bit 7) clears the bits of
  its four table entries; each POV (low word ≠ 0xFFFF): a = v × 0.01 × π ×
  1/180, cos a = sin(a + π/2) < −0.1 down, > 0.1 up, sin a < −0.1 left,
  > 0.1 right, each clearing its four entries' bits. Analog bytes from
  the copied state at the role's offset: steering (flag 0) 0x80 −
  (i8)(ftol(v × −0.0127)) × sign, else 0x80; combined (+0x368 and flag 3)
  0x7F − (…) × sign into +0x13 with +0x10 = +0x11 = 0; else +0x11 the
  brake (flag 2) and +0x10 the throttle (flag 1) the same way (0 when off),
  +0x13 = 0x80. _ftol truncates: ±10000 × −0.0127f gives ∓126 (126.9999…),
  so a full stick is 0x02 / 0xFE, not 0x01 / 0xFF.
* **JOY rows** (0x7FDD10; the bit index: mask 0–63, the analog bytes from
  64 = +0x10): STEER (row 25, 0x571CD0: action 3 alone 0x00, 4 alone 0xFF,
  else 0x80), STEER_ANALOG (row 40, bit 0x50), THROTTLE (26, action 1,
  0xFF), THROTTLE_ANALOG (36, 0x40), _ALTERNATE (37, 0x58 lower half),
  BRAKE (27, action 2), BRAKE_ANALOG (38, 0x48), _ALTERNATE (39, 0x58 upper
  half), EBRAKE (28, action 5, 0xFF), NITRO (33, 6), SHIFT (29 / 30, 7 /
  8), LOOKBACK (31, 9), POV_CHANGE (32, 10, press), PAUSE (34, 12), RESET
  (45, 14), ENGAGE / MAP / SMS / LEGEND (46–49, 15–18), HYD (50, 53: 19,
  22). The analog rows (0x5795D0): the byte **XOR the row's +0x10 (1)**,
  through the device context's graph of type +0x15 (3) — 0x57C250 makes
  every type's graph (0, 0), (128, 128), (128, 128), (255, 255) with dead
  band 0 and nothing else writes them: the identity — then half 1
  (0x7F − v) × 2 below 0x7F, half 2 (v − 0x80) × 2 above 0x80, 0xFE → 0xFF;
  sent when it changes. The FE stick rows 18–21 (0x571D60): the combined
  byte (0x58) and the steering byte (0x50), XOR 1, below 0x40 / above 0xBF
  (a wheel 0x64 / 0x9B, 0x5C0F70) give PAD_UP / DOWN / LEFT / RIGHT
  _ALTERNATE: the crossing sends the press with +0x20 = tick 0x865134 +
  25, held past it a press whenever the tick is above +0x20 (then tick +
  6) — but 0x865134 has no writer and reads 0 under Wine in the menus,
  pause and free roam (the maintainer's private captures), so the
  original never repeats: one press per push (**verified**; the clone's
  `FeRepeat` at tick 0; the Modern layout's repeat is a clone extra).
* **The handlers** (the human input 0x41E060 registers them with 0x579420):
  0x407750 steering: STEER at +0x2C4, STEER_ANALOG at +0x2C5; the digital
  one unless 0x80; 0xFF → −1, else (v − 128) × −1/128 (+0x2C8, positive
  left) → 0x41DFC0 (0x416BB0's response curve; a wheel 0x4061C0).
  0x416DE0 throttle: digital +0x2D0, analog +0x2D1 = min(v, 255) (the × 2
  / × 4 at 0x416E8A needs 0x404800, which returns 0), alternate +0x2D2 =
  min(v × 1.07, 255), × 1 when the car's gear +0x434 is 1 (neutral); the
  digital unless 0, else the larger; /255 → +0x2D4; in neutral then
  through the graph 0x7EA004 (0x40C840: (0, 0) (0.575, 0.437) (0.797,
  0.620) (0.894, 0.746) (0.95, 0.866) (1, 1), `pad::NEUTRAL_THROTTLE`;
  ported). 0x407EB0
  brake the same without the 1.07 (+0x2DC). 0x407F40 e-brake: 0x99 → +0x2E8
  /255. REVERSE 0x97 / 0x98, EBRAKE_ANALOG 0x9A, SHIFT*_ALTERNATE and
  DISENGAGE_CLUTCH have no row on the PC. So with a pad attached, at rest
  the steering reads 0x81 (−1/128), the throttle 2/255 (0x7E: (0x7F − 0x7E)
  × 2 × 1.07) and the brake 1/255 (its byte 0 XOR 1): ported as is. Full
  left reads 125/128, full right −1. Clone: `pad::driving` (keyboard and
  pad actions in one held set; the analog report from the first pad —
  the per-player device assignment 0x571FC0 is **not traced**).
  **The race's input object (measured + traced):** the human input
  object (car +0x30) is a new object at the countdown; from its first frame its
  brake byte +0x2D9 = 1 (event 0x95; +0x2D8 = +0x2DA = 0), written only by the
  handler 0x407EB0 (registered at 0x41E0F1..0x41E11D through 0x579420, whose
  registration re-sends a pending device state, 0x579370) and the reset
  0x4076E8; brake = max(+0x2D9, +0x2DA) × 1/255 (0x407F24) while +0x2D8 = 0.
  Measured (+0x214): countdown brake 1.0 (the start's hold), gas 0.00298; from
  the green brake 1/255 = 0.00392157, kept while no key (the mover's pedal
  check 0x5ABC00 still clears it below half travel when the gas is ahead).
  Clone: `update_race_live` floors the player's brake at 1/255 from the
  green. **Open**: which device state sends 0x95 = 1 at the registration (with
  no pad attached), and whether free roam's input object has it (free-roam
  runs above read idle pedals of 1/255 too, not ported).
* **Controls screen** (UI_Options_PC_Controller, 0x4ECD10, handler
  0x4F94A0..): 0x4ACE00 lists the records with a channel (0x835930, 12-byte
  entries kind / index) and shows the header's arrows (+0xDC / +0xE0,
  "Static") when there is more than one. Up on the first row selects the
  header (+0x64 = 0, "Highlight" 0x78F228, 0x4F9434); Down on the header
  the first row (0x4F954A); Left / Right (0x9120409E / 0xB5971BF1) on the
  header change the device (0x4ACAA0, wrapping, "Blink" on the arrow,
  **not ported**) and send 0xAF0BBD92, which rebuilds the rows by kind:
  0x4DBA50 keyboard ("Keyboard Configuration" 0x5CF08EB7), 0x4DC9F0 joypad
  (0xF91FBA4D, "Joypad Configuration"), 0x4DDC10 wheel (0x14D4BD3B) — the
  same 18 rows and actions each (pad cells 0x4ADB30..0x4AE050 →
  0x4ACF40(action, column, device)). Names (0x5C20F0 → 0x5B8AA0 for a
  controller): column 0 of steering / a pedal bound to an axis names the
  axis (GetObjectInfo's tszName), column 1 then the first button; else the
  first (column 0) or second (column 1) button — DirectInput's "Button n"
  as "Button n+1" in English (another language's 0x7A345C prefix through
  its own string) — then a POV entry (the POV's name); "" → `---`. The
  axis / POV names are the driver's: the clone's "X Axis", "Z Rotation",
  "Hat Switch"… are **inferred** (Windows' HID names). Binding (Accept,
  0x5C2110: +0x33C = action, +0x340 = column, 0x870768 = 1): 0x5CA730 each
  frame — Esc (0x87075C) cancels; the first button 0–15 pressed since the
  last frame → 0x5B8470; for actions 1–4 any of lX, lRx, lY, lRy, lZ, lRz,
  slider 0, 1 (that order) moved more than 1000 (500 on a wheel) from the
  base 0x86F8B0.. (set at the capture's start, 0x5B91D3, and after each
  frame) → 0x5B8260(offset, ±1) for **each** (the first takes the action,
  the later ones only leave their roles: native); every POV not
  centred → 0x5C1730 (the direction by 0x5B83C0: the larger of |cos|, |sin|
  of the angle; the action takes POV entry 16 + 4k + dir with 0x5B8470's
  rule — after a button bound that frame it writes 0 there; ported the controller-support pass).
  0x5B8470: the action leaves every driving entry (one kept for column 1
  unless the action is 1–4 or bound to an axis), then the button's entry
  takes it. 0x5B8260: the axis leaves every role using it; throttle (1) /
  brake (2) take it with the direction, and when the other pedal is on the
  same axis the other way the two become the combined axis (sign −dir for
  the throttle, dir for the brake; +0x367, +0x368 on), else the role's flag
  on and +0x368 off; steering left (3) / right (4) take it as role 0 with
  sign −dir / dir. Restore Defaults: BUTTON1 (0xC519BFBF, C) → "Reset Keys
  To Default" dialog (0x2BBCCA6B, GenericDialog_SMALL, OK 0x0639 / Cancel
  0x541389E5); OK (0xEFEDCBA0) → 0x4ACD90: the keyboard 0x5D0000, a
  controller 0x5C11B0. Clone: `ControlsScreen` (`device`, `header`,
  `restore`), game.rs `controls_cells`.
* **The profile** (0x5B9430 / 0x5B9580; payload 0xD254, the end of the
  payload): six blocks of 0xB5 bytes, one per record in record order up to
  the count 0x870764 (zeros past it): +0x00 guidInstance (0x5B8120), +0x10
  +0x344..+0x353 (axes), +0x20 +0x354.. (signs), +0x30 +0x364..+0x367,
  +0x34 +0x368, +0x35 the driving table as 32 dwords. The FE tables are not
  kept. The loader gives each block to the record with its GUID. The clone
  writes the blocks the same way when it has one (the file's keyboard block
  first, as the file has it), with gilrs's device uuid as the GUID (a
  Windows guidInstance cannot be made; **clone deviation**), and leaves the
  section alone otherwise.
* **What a modern pad is** (**inferred**: Windows' drivers, not the game):
  an Xbox pad (045E) through the XInput HID driver — lX / lY, lZ the
  triggers (left +, right −), lRx / lRy, ten buttons A B X Y LB RB Back Start
  LS RS, POV 0 the d-pad: the default combined axis lRz does not exist, so
  the pedals are buttons (A brake, B throttle) until bound by hand; a
  PlayStation pad (054C) through the HID driver — lX / lY, lZ / lRz the
  right stick, lRx / lRy L2 / R2, Square Cross Circle Triangle L1 R1 L2 R2
  Share Options L3 R3 PS (14): the right stick is the pedals (up throttle),
  Square brake, Cross throttle, and in the menus Square Select, Cross Back;
  other pads as a four-axis, twelve-button pad of the time (the Logitech
  Dual Action's layout; a real Dual Action, C216, gets its preset). No dead
  zone (as the original sets none; gilrs's filters are off).
* **Force feedback** (**verified** in the disassembly, **observed** under
  Wine with a virtual FF wheel where marked; the model is
  `nfsu2::pad::ff::Original`, played by `nfsu2::pad::ff_out`). DirectInput 8 effects
  on any device that reports FF actuators; no XInput (no import, string or
  GUID). The console builds' pad vibration (0x5727C0 → 0x572620 →
  0x4022C0) ends in a bare `ret` on the PC.
  * **Set-up.** The opener (0x5CD200) sets the cooperative level 5
    (exclusive | foreground, observed); the axis callback 0x5C8670 counts
    the DIDOI_FFACTUATOR objects into 0x86FF80[ch] (at most 2). A driving
    device (0x86E5BC) gets DIPROP_AUTOCENTER off (0x5CD3E7). The gain:
    when the player's input object starts (0x41E264..0x41E2AD, if the
    channel has a device) 0x5C9130 → 0x5C00C0 sets DIPROP_FFGAIN to
    `min(100, _ftol(Overall Force Strength × 255)) × 100`, the minimum
    unsigned: every strength from 0.4 up, and a negative one, gives 10000
    (observed 10000 at the default 1.0). The FF object *0x870760 keeps
    8 slots per channel: conditions +0x2498, constant +0x24E8, periodic
    +0x2538; 0x5BFD30 fills its angle table +0x25F0 with
    `_ftol(sin(d · π_f32 / 180) × 1000)` for d = 0..359 (270° → −999).
  * **When.** 0x406460 runs from the human driver's update (0x41E2C0 →
    0x41DFC0, after the steering) once per world update 0x609CF0, so once
    per drawn frame, when car +0x28 = 1, the car has a player, 0x870760
    exists, the channel has a device and the game state *0x8900D8 is 4.
    Its arguments: the steering value 0x41DFC0 returns (the shaped input,
    which also feeds the running mean 0x576AB0) and the channel. Another
    caller, 0x4220CE, passes steering 0 for the driver object at +0x690 of
    a type not traced (**open**). Outside state 4 the input poll
    (0x5CFE40..0x5CFFBA) stops the constant, periodic slot 3 and the
    damper, resumes, ends the puddle damper, and plays the spring at
    7000 (0x1B58; observed) — stopped instead when channel **0** is a
    gamepad-class device (the test reads 0x86FFB4 / 0x86F8A8 without the
    channel index).
  * **Feedback off** (player block +4, 0x83AA68 = 0, read by 0x5EB430) or
    **a drag race** (0x4018F0 on 0x89E7A0: +0x38 = 0x89E7D8 the drag
    flag, or its online twin 0x890108): stop the constant (0x5CD860),
    periodic slot 3 (0x5CD9E0) and the damper (0x5BFE80), resume
    (0x5CDBC0), end the puddle damper (0x5CD920), then the spring: 10000
    at full lock (|steer| = 1.0 exactly), else 7058 (0x1B92), both
    stopped instead on a gamepad-class device (0x5BFDA0(ch, 2)). The speed
    of the call is not stored. Observed by writing 0x83AA68 = 0 / 1.
  * **Driving**, in order (0x4064DE..0x406927):
    1. Spring (condition slot 0, GUID_Spring, offset 0, coefficients =
       saturations): 10000 at full lock, else stopped (0x5BFE10); no class
       test here, so a gamepad-class device gets it too. Observed 10000.
    2. Constant force (constant slot 0), only when the front tyres' sum
       ≠ 0 (exactly 0 leaves the last level playing) and not on a
       gamepad-class device: L = clamp(_ftol(sum / [this +0xC] × 10000),
       ±10000), 0 when |v| ≤ 1 m/s; sent through 0x5C9560(ch, L, 270) =
       0x5C9390(ch, table[270] × L / 1000) = −999·L / 1000 (int).
       The sum is 0x59DB00's field +4 of the two front wheels (car +0x34 →
       +0x38, +0x3C): +0xB4 · +0x124 + +0xC4 · +0x128 = sin δ · Fx +
       cos δ · Fy, the tyre's force across the car (kN). [this +0xC] is
       the steering object's copy of **Road Feedback** (player block
       +0x20, copied at 0x41E25E; default 12.4), so Road Feedback is the
       force's divisor. Observed: 0 standing, small straight, ±7000…full
       in turns with the sign following the steering (Wine's numbers
       saturate, so −999 vs −1000 is not visible there).
    3. Damper (condition slot 1, GUID_Damper, saturation 10000):
       _ftol((1 − min(1, v / 13)) × 7058.8237), v = |car +0x70| (observed
       within ±25).
    4. Both front wheels off the ground (car +0x390 and +0x391 = 0: wheel
       +0x11C, the contact flag the export 0x5A57FC copies into the
       mover's +0x350 + i = car +0x390 + i): suspend (0x5CDA20: flag
       +0x2220, stops and remembers spring, constant, damper, puddle
       damper, periodic slot 3; the jolt is not touched). A front wheel
       down again: resume (0x5CDBC0) with the last parameters — the
       constant force through 0x5C9560 with the angle +0x2BBC, which has
       no writer (**inferred** 0, so level 0 until the next call).
    5. Jolts (periodic slot 0, GUID_Square, 150 ms, period 75 ms, fade to
       0 over 20 ms; 0x5C9770). Δ = 0x827AAC[ch] (the last call's speed)
       − v: when |Δ| > 4, M = _ftol(min(1, (Δ − 4) / 15) × 10000) (a rise
       gives Δ < −4 and a negative M below −10000, not clamped). Then the
       bump stops: car +0x370..+0x37C summed (each 0x5ABB10's total of
       0x59E1B0's 0.225 · mass · closing speed over the frame's
       sub-steps, zeroed at the mover's frame start 0x5B731B) over 0.05:
       M = _ftol(min(1, sum / 4) × 6666.67). Both are spaced on the sim
       clock: t = 0x865188 × 0.00025, fire when t − 0x827AA4[ch] ≥ 0.15
       (then 0x827AA4 = t); a negative gap sets 0x827AA4 = 0, and the bump
       jolt then fires all the same (0x406718..0x406727), the speed jolt
       does not. 0x5C9770 also needs 150 ms of wall clock (ftime,
       +0x2B90 / +0x2B98) since its last pass, and plays even while
       suspended. The trigger was observed (0x827AA4 stamped after a poke
       of 0x827AAC), but Wine refuses GUID_Square on the virtual device, so
       the jolt was not seen on the wire.
    6. Road surface (periodic slot 3, 0x5C9FE0, infinite): from the two
       front surfaces car +0x394 / +0x398 (wheel +0x114; 1 in drag and
       Street X races, 0x59CF75), jump tables 0x4069E4 / 0x406A0C (names
       from the surface table 0x800EC0 + 0x98·s, +0x20): the first sets
       (amplitude, period) — 8 DIRT (130, 80), 9 GRAVEL (110, 80), 10
       ROUGH_ROAD (70, 90), 11 COBBLESTONE (100, 100), 15 GRASS (100, 40),
       16 SIDEWALK (80, 100), 17 WOOD (80, 140), others (0, 0); the second
       adds its pair and picks the wave: 8 / 9 sine, 10 / 11 square, 15
       triangle; 16 / 17 replace the sums with their own pair and square;
       others sine and nothing added. Magnitude = _ftol(amp / 2 ×
       min(1, v / 13) × 39.2157), period = per / 2 ms, at most 150. Waves
       0x7BFCD0.. (0 sine, 1 square, 2 triangle, 3 / 4 sawtooth up / down,
       unused); a new wave releases and re-creates the effect. Observed:
       sine, 50 ms, 1568 at speed with one wheel on 16.
    7. Puddle damper (condition slot 2, 0x5C9DA0): a front wheel on
       surface 13 (PUDDLE) and v > 23: stop the damper and the spring
       (remembered), play GUID_Damper with coefficients −10000, saturation
       10000: the wheel goes light. While it plays, 0x5C9160 and 0x5C95A0
       do nothing. Otherwise 0x5CD920 stops it and restarts the remembered
       two. Not reached under Wine.
    8. 0x827AAC[ch] = v.
  * **Upload rule** (0x5C9160, 0x5C95A0, 0x5C9390, 0x5C9FE0, 0x5C9DA0):
    nothing while suspended or without actuators; parameters are uploaded
    only when they differ from the last upload (or the effect is created);
    an effect that does not play is started (Start(1, 0)). The jolt is
    started on every pass. 0x86F86C (tested by the constant and periodic
    functions) is only ever written 0.
  * **The DirectInput calls** (**verified** in the disassembly; the clone's
    Windows output `pad::ff_out::dinput` makes the same calls). Every
    DIEFFECT (dwSize 0x38): flags DIEFF_OBJECTOFFSETS | DIEFF_CARTESIAN
    (0x12), sample period 0, gain 10000, DIEB_NOTRIGGER, repeat 0, start
    delay 0, cAxes = the actuator count, axes DIJOFS_X, DIJOFS_Y (data
    format c_dfDIJoystick2, 0x7BFDE0: 0x110 bytes, 0xA4 objects).
    Conditions (0x5C04F0): direction {1, 1}, no envelope, one DICONDITION
    per axis {offset, coefficients ±c, saturations s, deadband 0}, the
    second {0, 8000, 8000, 8000, 8000, 500}, infinite. Constant (0x5C0210):
    direction {1, 0}, an all-zero DIENVELOPE, lMagnitude, infinite.
    Periodic (0x5C8D30): direction {1, 0}, magnitude as a DWORD (a negative
    jolt stays its bit pattern), offset 0, phase 0, period in µs; the road
    surface infinite with a zero envelope, the jolt 150000 µs, period 75000,
    fade time 20000. GUIDs 0x7BFC90 Spring, 0x7BFCA0..0x7BFCD0 SawtoothDown,
    SawtoothUp, Triangle, Sine, 0x7BFCE0 Square, 0x7BFCF0 ConstantForce.
    Updates are SetParameters(DIEP_TYPESPECIFICPARAMS | DIEP_ENVELOPE =
    0x140) (0x5C06B0, 0x5C03A0, 0x5C8EE0). Start (0x5BFEF0): Acquire, and
    only if it succeeds Start(1, 0). Stop (0x5BFF70): Acquire (result
    unused), Stop. Release of a slot (0x5C0000): Acquire, Unload (the COM
    object is not released). The opener: SetDataFormat, then
    SetCooperativeLevel(window 0x870990, 5), EnumObjects, and
    SetProperty(DIPROP_AUTOCENTER, DIPH_DEVICE, 0) on a driving device.
    The clone sets a two-axis data format with X and Y at the same offsets
    (only the actuator offsets matter to the effects), finds the game's
    window for the cooperative level, and matches the device by guidProduct
    = MAKELONG(vid, pid) among attached FF game controllers.
  * **Device class.** A gamepad-class device (0x15, 0x86F8A8) gets the
    damper, road surface, jolts and the full-lock spring, but no constant
    force and no Feedback-off or front-end spring. Under DirectInput an
    XInput pad reports no actuators and gets nothing (**inferred** from
    Windows' XInput HID driver).
  * **Frame rate.** Δ is per call, so per drawn frame in the original. The
    clone's world updates on fixed 1/60 s ticks with the 60 fps tick
    pattern, and the model steps once per tick, the sim clock and the
    real-time clock (as ftime) read on ticks: it behaves as the original
    at 60 fps at every display rate. At 60 fps the jolts come at most
    every 10 updates: 9 updates are 600 sim ticks, but 600 × 0.00025 less
    the stamp kept as a float is just under the float 0.15.
  * **Unused**: 0x5C99E0 (sine, 65 ms, periodic slot 1) and 0x5C9BC0
    (square, 100 ms, slot 2) are only replayed by the resume paths;
    nothing starts them.
  * **Open**: what the front-end path does in the pause (whether the
    driver update runs there); the order within the world update between
    the tyre forces 0x406460 reads and the mover; the +0x690 driver.
* **Wheels** (**verified** in the disassembly; not observed under Wine:
  no wheel in the prefix). 0x41DFC0 steers through 0x4061C0 instead of
  0x416BB0 when the channel is a wheel (0x5C1010: DI8DEVTYPE_DRIVING, the
  driver's type or 0x800588's override). The steering object (input +0x220)
  holds player block +0x19 at +4 and Wheel Range (+0x1C) at +8 (0x41E23E..
  0x41E255). 0x4061C0(x, dt): x = clamp(x, −R, R), u = 2 · (x + R) / 2R − 1,
  s = graph 0x7E9E88 (values 0x7E92D8: −1, −0.8, −0.615, −0.483, −0.388,
  −0.288, −0.22, −0.161, −0.111, −0.057, 0, …) at u. Unless +4 is set, the
  lock is 0x405DC0's with the wheel's angle graph 0x7E9EEC (0..160 m/s →
  0x7E91A4: 45, 15, 11, 8, 7, 7…) in place of 0x7E9ED8: lock = graph
  0x7E9F00(|mean +0x20|) · angle(v) · (graph 0x7E9F14(v) · (1 − (gas + 1 −
  (brake + handbrake) / 2) / 2) + 1); with +4 set it stays 45. No
  slow-down after a wall. The drift flag +0x10 as 0x405DC0 (on above 15°
  slip at 13.41 m/s, off under 5°). A counter-steer (s · slip < 0) while
  drifting or with |slip| > lock takes lock = min(|slip|, 45) — the whole
  slip, not 0x7E9F5C's share; else at |mean| ≥ 0.5 lock = max(lock, +0x80
  − 0 · dt), and +0x80 is not written here. The angle clamp(lock ·
  ratio(handling +0x270) · s, ±45) goes straight to the word +0x7C
  (_ftol(a · 65536) / 360): no rate limit (0x4060E0 is not called) and no
  drift ratio boost. It returns s (the FF's steering and the mean 0x576AB0
  take it). Player block +0x19 has no writer found besides the profile
  load (**inferred** 0). The clone: `CarState::steer_wheel`, chosen by
  `Controls::wheel` (the Wheel Range) when the driving device is a wheel.
  Which products count as wheels: 0x800588's 044F:04B2 and, standing in
  for the type each vendor's driver reports (**inferred**), SDL 2's wheel
  list (`pad::host::WHEELS`). A wheel is presented in DirectInput's axis
  order (`Layout::Wheel`: lX the wheel, lY, lZ, lRx, lRy, lRz raw), so
  0x5C11B0's wheel defaults apply: the combined pedal axis only on a wheel
  without lZ / lRz, else the pedals are bound on the Controls page.
* **Synthetic input** (clone): `NFSU2_PAD_SCRIPT=<file>`, lines `<s>
  connect <id> [vid pid]`, `disconnect <id>`, `axis <id> lx|ly|rx|ry|lt|rt
  <v>`, `button <id> <name> 0|1` on the game clock; the shots poll it at
  0 (`--shot controls_pad`, `controls_header`). The controller-support pass fed gilrs a uinput
  Xbox 360 pad (internal evidence, the maintainer's probe `pad_probe`).
* **Open**: the per-player device assignment (player p's device byte
  0x864788[p], 0xFF = default 0x7F9414[p]; set by 0x571A30, the handler
  of event 1 registered for players 0 and 1 by 0x579560 — what raises
  event 1 is not traced; 0x571FC0 sets the player's layout code table
  0x865764 + 0x1C8p from 0x571FE0); the device arrows' Blink; the other
  two record tables' readers; under Wine the Controls pages with a pad
  were not captured (no pad in the prefix). The Controller extras:
  `docs/playing/controller-support.md`.

## Shop previews and engage message (2026-10-31, the finished-career save)

Traced in SPEED2.EXE and checked under Wine (fastgpu, a copy of the player's
finished career, `tests/fixtures/native_saves/career_complete.sav`, maintainer only (private test set)).

* **Engage message** (Shop_EngageMessage 0x512920, from the dialog setup
  0x4998E0): kind 0 (the garage) 0x987EECDA; a shop whose visit-table entry
  (0x862840, count 0x86283C, 8 bytes each) has +6 = 5 — set by the first
  Accept (0x54CB00) — the generic 0x15D6B438 "Would you like to enter the
  Shop?"; otherwise `<NAME>_BODY` (the first-visit text: "Welcome to the
  Bayview Body Shop, … You have unlocked …"), 0x15D6B438 when that label is
  missing. The finished-career save's table (read from memory under Wine, 305 entries) holds 22
  shops in state 5 (all but UC_CAR_LOT and THE_UNIQUES_SHOP), and the
  original asked the generic question at IND_BODYSHOP_2,
  IND_CAR_SPECIALTY_2 and IND_GRAPHICS. The clone loaded the state
  (`Profile::entered`) but always showed `_BODY`; `ShopEngage::message` now
  follows 0x512920.
* **Previews** (all 0x55C180 Garage_PreviewPart on the car's copy unless
  noted; each also asks 0x4C2100(slot) for the slot's view):
  * UI_GenericParts_Browser lists: entry 0x55EDE0, step 0x55EBB0. In the
    graphics shop's vinyl lists (+0x1D7) they call Garage_PreviewVinyl
    0x55C380(layer 0x8389A8, part) instead: the vinyl on the shown ride as it
    is (car + setup), first clearing 0x40–0x43 when a vinyl on the ride
    (0x50EF10), a setup vinyl (0x521590) or the highlighted one is a unique
    (attribute 0x564B8CB6 = 0x40; **inferred** that the third test's part is
    the highlighted one), else refitting the setup's vinyls and colours
    (0x40–0x43, 0x4D–0x5C, 0x50E170). The clone previewed vinyls on the car
    without its setup; now as 0x55C380.
  * UI_IcePartsOverlay (UI_ICEMAIN: Window Tint, Lights, Nitrous Purge,
    Hydraulics, Split Hoods, Doors, Trunk Layout, the audio components):
    entry 0x5606F0 (the fitted part selected, previewed when it has a part)
    and step 0x560930 (PartsOverlay_Step). Not Custom Gauges (its own
    UI_CustomHUDOverlay). The clone had neither the preview nor the view
    for these lists. Under Wine, Window Tint's list turned the car to the
    tint view and DARK GREEN tinted the windows.
  * Neon lists: NeonOverlay_Step 0x55CAD0 (preview and view). Under Wine
    entering the Underglow list already moved the camera (**inferred**: the
    entry also calls 0x4C2100; its function is not traced); RED lit the
    floor red. The clone had no view request for the neon lists.
  * UI_ChooseSpinner's style strip (handler 0x569680, +0x214 clear): the
    fitted rim (+0x20C) when the style is its name's (0x43DB50(0x610010)),
    else the style's first size, on 0x1D and 0x1E, view 0x4C2100(0x1D). The
    clone previewed spinners only inside a style's sizes.
  * UI_Paint (UI_Paint_EnterSlot 0x55CC60): entering a slot sets +0x18C, so
    the fill UI_Paint_Fill 0x548640 takes the finish (+0x174) from the
    slot's setup item (list 0x8389C8) or fitted part (attribute 0xEBB03E66)
    and opens on that swatch; then Base Paint with no swatches, or a
    spoiler paint not Gloss, refill on Gloss (0x50F5B0 is the default
    finish a strip step sets). Under Wine the finished-career save's red opened on Metallic with
    the car still red; the clone opened on Gloss's first swatch (silver).
* UI_Paint on a finish with no paints to list (every paint of the finish
  still locked; the triggers step to it like any other, 0x55D0E0): the fill
  leaves the swatch list +0x158 empty, and the empty branch 0x55D28F hides
  U2_Paint_HighLight 0x9BA6D658 and the price 0x13C45E and shows the lock
  0x23A43844. Accept (0x406415E3) in the slot then compares the list head
  with itself (0x563707..0x563715) and returns: no sound (0x50D150 comes
  after), no question, no bank check. The mouse path 0x55D420 asks only on a
  press over a drawn swatch, and none is drawn. **Verified (code)**, both
  paths; not observed under Wine. The clone did the same until Accept, which
  indexed the empty list and ended the game.
* Checks: `--shot finished_shop_checks` (every open list of the body shop,
  carbon fibre, the specialties and neon, the rim and spinner strips, base
  paint, vinyls, a decal slot; then every shop's message), `finished_shop` with
  `NFSU2_SHOT_SHOP=<name>` (a shop's dialog), `NFSU2_SHOT_SHOP` on any shop
  screen (its platform). **Open**: 0x560930's part animation calls
  (0x4C18B0 on the category's channel; 0x438030 for Split Hoods and Doors)
  are not ported; the neon list's entry function.

## Engage prompt queue and sounds; the SMS marker flow (2026-11-01; checked against the original under Wine)

**Verified** (REA decompiles of SPEED2.EXE) unless marked. Checked in the
original (fastgpu, the `stage1_win_streetx.sav` career, free roam): the
prompt slots read from memory (player 0x8900AC → +0x9A4 the HUD → +0x58
the prompt element → +0x60 the slots) and frames at 30 fps.

* **The queue** (element 0x4F83D0, eight slots of 0x14 bytes at +0x60:
  +0 state, +1 kind, +4 g_SimTicks when state 2 began, +0x10 value).
  0x4C9170(kind, value): a used slot of the kind restarts only from rest
  (4 → 2, the time, the value, the sound below); in 1–3 nothing; else the
  first free slot gets {state 1, kind, value}. 0x4DAB80(kind): the first
  slot of the kind is set to 6, drawn (0x4C9030(0)) and freed.
  0x4DA9A0 (the HUD update 0x4EC5A0 runs it at 0x4EC673, before the SMS
  element 0x4DB8C0 at 0x4EC6CD), with the element's mask on: the reset
  (D-pad 0xEA570722, icon / mail / count / both highlights hidden), then
  0x4C9030 on every slot in slot order (kind 0 → state 0; states ≥ 2:
  the step 0x4C8AC0, then the draw by kind: 0x11 0x4C8DD0, 0x13 0x4ABAF0,
  0x14 0x4ABA30, else 0x4C8B80), so a later slot's draw wins (the D-pad's
  texture and script). Active = the first slot in 2 or 3; pending =
  0x4C90B0, the first used slot of the highest priority 0x4ABF20 (kinds
  1–8, 0xB–0x12: 4; 9, 10: 3; 0x13: 2; 0x14: 1). No active: the pending
  slot, unless in 4, goes to 2 (time, sound) and runs again; none: the
  reset again. Active and a pending one of a higher priority: the active
  SMS slot goes to 4, any other to 6 (drawn, freed), and the pending one
  to 2 (time, sound); else the active one runs again.
* **Steps** (0x4C8AC0): 2 → 3 when (now − +4) / 4000 > 1 s (kind 0x14:
  its value, 2 s), the SMS's held while the D-Day script runs (0x502420);
  3 → 4 past 4 s; each through 0x4AB9F0 (not for 0x14). 0x4C8B80 sets a
  state-4 slot back to 3 before drawing, so a marker's or the encounter's
  prompt never rests and its sounds end at the step to 3.
* **Sounds** (0x51DE40(event, control) = PlayFESound(0x837FE0[event·27 +
  control]); the controls' names at 0x7F9500): flashing (state 2) plays
  FOCUS on FE_SOUND_ENGAGE_PHONE (0xD, FE id 14), on
  FE_SOUND_ENGAGE_SMS (0xF, id 16) for the SMS slot; the step to 3 plays
  FE_SOUND_ENGAGE_DPAD (0xE, id 15) for every kind; the step to 4 nothing
  (0x4AB9F0(4)); kind 0x14 none (each site skips it). FE_SOUND_SMS_POPUP
  (0x10, id 13) is the SMS screens' own (0x529710). Sites: 0x4C9208
  (0x4C9170), 0x4DAAB0 / 0x4DAB49 (0x4DA9A0), 0x4ABA01 / 0x4ABA13 /
  0x4ABA1B (0x4AB9F0).
* **Callers.** A marker: 0x5EAFC0 (the car enters a marker trigger, the
  entry valid by 0x532A60) → 0x4CA020 → 0x4C9170(kind, 0); 0x5EAF40
  (leaving) and 0x533BF0 (a world challenge starts) → 0x4DB820 →
  0x4DAB80. The random encounter: 0x413030 runs every frame of the
  encounter manager's TRYING_TO_ENGAGE (0x42EE8D) → 0x500D20 → 0x4DB890:
  a car to challenge queues 9 (every frame; no effect while used), none
  drops it; 0x602E79 drops it when the outrun is engaged. The D-Day
  script: 0x5342C9 → 0x4CA670 → 0x4C9170(0x14, 2), never dropped. The
  SMS element 0x4DB8C0: see **Messages / SMS**.
* **Draws.** 0x4C8B80 (markers, encounter): 2 — D-pad 0x6FFF0767, the
  icon's texture 0x4ABE50(kind); 3 — the D-pad alone; both show the icon
  (+0x34) and the second highlight (+0x2C) and start, unless running,
  "pulse" / "Blink" / "Pulse" on highlight / icon / D-pad; 6 — icon and
  highlight hidden, D-pad 0xEA570722, "Init" on the three. 0x4ABA30
  (0x14): 2 the D-pad 0x6FFF0767 with "Pulse"; 3 with "Init"; 4, 6
  "Init". 0x4ABAF0 (SMS): as in **Messages / SMS**, plus the colour:
  when "animation" is not running on the mail, 0x5046B0 finds that
  script's track, 0x504A00(0), and 0x535580(0, colour) evaluates the
  track (0x519D40, or 0x52BDD0 for kinds 1 / 3) and adds the colour less
  that value to the track's base (+0x18..+0x24, B, G, R, A), then clears
  bit 7 of +3. The mail's ANIMATION (0x375D11BF in HUD_SingleRace: 800
  ms, looping, alpha keys +0xFF / 0 / +0xFF over the base 0x1FE: a blink)
  so blinks in the kind's colour — green (14, 156, 14) for the envelope
  of kinds 8+ (the tips are kind 9). **Observed** under Wine: the green
  envelope blinking next to the lit D-pad after an engage tip.
* **Under Wine** (memory, g_SimTicks): the garage marker entered by a
  teleport: its slot (kind 0xC) state 2 at 143838, 3 at 147937 (+1.02 s);
  the resting SMS slot (state 4, slot 0) drawn first, the marker (slot 1)
  after: the D-pad lit with the garage icon and the envelope beside it.
  A drag event's marker: kind 2, 2 → 3 after 1.02 s, then state 3 for
  good; Enter opened UI_EngageRaceDialog ("Incoming Mobile Message",
  within two frames at 30 fps), the slot unchanged (the HUD update does
  not run under the dialog); Escape back to free roam, the slot still 3:
  no new sound. An engage tip (FC25E6EE, (619, −175)) driven through at
  ~7 mph: bank +25 and unread 6 → 7 in the same frame, the SMS slot 4 → 2
  at once (0x4C9170 from rest: id 16), 3 at +1.02 s (id 15), 4 at +4 s;
  the car kept driving, no pause, no camera change, no dialog (a tip is
  not urgent but ENGAGE_TIP_CAREER_PROG). The tip's own effect was not
  looked at (teleports).
* **Sound events under Wine** (`fastgpu.sh --audio-file`, `sndwatch.py`,
  the same career): an engage tip → PlayCommonSample {0x10, 0x108B,
  0x1000, 0} (FE id 16, 554 ms), 1.0 s later {0xF, …} (id 15); the
  garage marker → id 14 then id 15; Enter there → UI_EngageShopDialog
  with id 5 (its "ready", [BACK][0], already ported); Escape → id 5.
  Nothing else (no sound at the trigger, the tip's cash or the prompt's
  end).
* **No other marker sounds** (searched): see docs/notes/audio/sound-audit.md H5–H7.
* **Clone** (`hud::Prompts`; `Hud::take_sounds` → `Game::fe_sound`):
  the queue, its steps, priorities, takeovers, draws in slot order and
  sounds as above; the marker's kind is queued / dropped when the
  current marker changes (`Input::engage`, kept under the in-game
  screens, where the HUD does not update), the encounter's queued while
  `Input::encounter` and dropped otherwise (**inferred**: the clone
  drops it also when the manager leaves state 3 without an engage, which
  the game does not), 0x14 once when the D-Day prompt starts. The mail's
  colour by `ScreenState::script_color`. Before: the markers had no
  queue or sounds; the SMS slot had no sounds and no colour; a marker's
  prompt kept the D-pad over the SMS slot whatever the order.
  Checks: `hud::tests::engage_prompt_queue`, the shot `sms_tip`
  (`NFSU2_SHOT_MS`; `NFSU2_AUDIO_DEBUG` prints each FE id played), the self-test's
  SMS steps. **Open**: 0x4C8DD0 (kind 0x11's draw).

## Front-end mouse (2026-11-01; traced in SPEED2.EXE, checked against the original under Wine)

* **Device** (**verified**): g_Mouse 0x8763D0, a DirectInput mouse
  created at 0x5D2839 (0x5BF6F0: data format 0x7BFDC8, cooperative level
  6) and polled once a frame by 0x5BF870 (GetDeviceState, 0x14 bytes).
  0x5BF760 reads GetCursorPos less the window's top left, × 640 / client
  width (0x797D60) and × 480 / client height (0x797D5C), truncated: +0x4 x,
  +0x8 y in canvas units; +0x14 / +0x18 the move since the last frame;
  +0x1C the wheel (lZ). Buttons: +0x21 left held, +0x22 pressed (edge
  down), +0x23 released (edge up); right +0x24..+0x26; middle
  +0x27..+0x29. Under Wine (fastgpu, xdotool) the memory follows: x 0x147 /
  y 0x197 for the window point (654, 814), +0x21 1 while held, then 0. A
  press shorter than a frame can be lost (GetDeviceState is immediate).
* **Swap flag** (**verified**): +0x20 (0x8763F0) makes the right
  button the clicking one. Its only writer is the mouse object's
  constructor 0x5C8610 (run once from the static initializer 0x77E580,
  table 0x7E8610, on 0x8763D0): +0x20 = GetSystemMetrics(SM_SWAPBUTTON)
  != 0. Nothing in the options or the registry touches it.
* **Dispatch** (**verified**): the front end's tick 0x5663C0 sends
  0xC98356BA to the screens, then 0x9803F6E2 when 0x5BF950 sees a button
  held, pressed or released, the wheel or a move. MenuScreen_BaseNotify
  0x55B5D0 drops it while the screen's text entry (+0xC) is up (0x54E5C0
  swallows every message) and when the package's pad mask (+0x1C) is 0;
  else the screen's slot +8 gets it. About fifty handlers compare
  0x9803F6E2; each screen class does its own hit tests on the cursor less
  (320, 240) (0x50CD70). 0x50CD00(x, y, pos, size) and 0x52D030 test
  pos ≤ p ≤ pos + size on both axes (**both edges inclusive**, corrected
  since). 0x52D030 / 0x52D110 (press) / 0x52D0A0 (**held**) read the
  object's rectangle through 0x51DA00, which has **no visibility test**:
  hidden arrows and frames still take clicks (0x537A70 finds an object
  only if it is an image). The wheel helper 0x50CDA0: lZ < 0 is +1 (down
  / on), > 0 is −1 everywhere.
* **Cursor** (**verified**; seen under Wine): 0x5378D0, after the front
  end is drawn, calls 0x50B460(x, y) when 0x7F91C4 and 0x8384C4 are set,
  no movie plays (0x8383AC, g_MoviePlaying: 1 at the movie start 0x566052,
  0 at its end 0x5661B3; corrected 2026-11-01, round 1 read it as "no
  load") and 0x7FA29C is set (initialised 1; cleared only by the Take
  Picture paths 0x4CEE11 / 0x55B2B1, set again by 0x4B59F5 / 0x523F26).
  0x7F91C4 is initialised 1 and never written (its only references are
  the reads at 0x5378D0, 0x537940, 0x566410), so the gate is the front
  end's existence (0x8384C4, set at 0x5378A8) and the movie flag.
  0x50B460 notes a move (0x838498 / 0x83849C → 0x83A9CC = now) and draws
  only within 2.0 s of the last move (0x7844C0) or always while
  g_GameState 0x8654A4 is 3 (the front end; under Wine the arrow stays on
  every front-end screen, in the dyno it does not). The sprite is
  PC_CURSOR (0x6B6A294F, 32 × 32, GlobalB / FrontB) at (x − 12, y − 6)
  in its own size, vertex colour 0xFF808080. The system cursor is hidden
  in full screen (0x5D269F SetCursor(0), ShowCursor(0) when 0x87098C = 0).
  **At boot (observed** under Wine, 2026-11-01, memory reads with
  the mouse moving every 2 s): through the boot movies g_GameState is
  already 3 and 0x8383AC is 1 (no arrow on any frame); the arrow first
  appears on UG_LS_Splash as soon as the movies end (0x8383AC 0), before
  "Press Enter Key" fades in, and stays on the front end; the attract
  movie sets 0x8383AC again. **In free roam (observed):** g_GameState 6,
  0x8383AC 0: a move shows the arrow over the driving view, gone 2 s
  later.
* **Help bar** (UI_PC_Help_Bar, 0x55DE10, **verified**): while the bar is
  on (+0x6C) a click on an enabled slot (button%d / icon_%d, +0x18
  enabled, +0x14 the object) reads its bit (+0x1C): bit 2 (Logoff) plays
  0x4B (the sound audit's F18) and asks 0x545C5292; bit 1 (Quit Game)
  asks 0x9DE6B485 "Are you sure you want to quit?" (Quit Game / Cancel; seen
  under Wine); any other goes through 0x52D180 to the screen (0x55DDB0):
  4…0x80, 0x2000000…0x8000000 → 0x406415E3 (accept; when the screen's
  current object has one, 0x0C407210), 0x100 → 0x0C407210, 0x200 → Back
  (BUTTON1 on UI_ChoosePaintCategory), 0x400 → Back 0x911AB364 (seen under
  Wine: Back on the Quick Race mode select returns to UI_Main), 0x800 →
  BUTTON1 0xC519BFC0 with the tuning master up else BUTTON0 0xC519BFBF,
  0x1000 / 0x2000 / 0x4000 / 0x40000 / 0x800000 / 0x1000000 /
  0x10000000…0x40000000 → BUTTON1, 0x400000 → 0xC519BFC3 (Tip),
  0x80000 / 0x100000 / 0x200000 → 0xC539A4B0 (no clone key: not ported).
  The bar's own handler 0x55DFB0 runs the hover 0x52D500 without a click:
  "Init_Pulse" (0xA3E4657B) on an enabled slot newly hovered (+0xC), the
  slot's +0x20 script on leaving. **Traced** 2026-11-01 (small171): the
  slots (0x551FA0 for k = 1..6, 0x120-byte nodes, vtable 0x79B2EC) hold
  +0x10 = `button<k>` (0x79C924), +0x14 = `icon_<k>` (0x79C91C) — the hit
  and hover object —, +0x18 enabled, +0x20 the leave script's name; the
  constructor sets bar +0x60 = Init_Pulse (0x5521FE); 0x5428C0(slot,
  mask) without 0x10000 runs "Init" on both and copies "Init" to +0x20,
  bar +0x60 = Init_Pulse; with 0x10000 0x51DBF0 runs the layout script
  on both and copies its name, and +0x60 is the layout's pulse: In_Game
  0x339F3277, Event_Results 0xDD0F9CDA, Pause_Menu_Main 0x40AAAC1D,
  Pause_Options 0x1D39C830 (each icon's 1200 ms looping script). Ported:
  `screens::help_hover_script`, `Game::help_hover` (the slot whose icon
  is under the cursor and since when) → `help_bar_in`'s hover.
* **Scroll bar** 0x538140 (**verified**; drag and track click
  **observed**): set up by 0x542AD0(pkg, prefix, vertical, proportional,
  mouse off) on `<prefix>_Backing` +0x40, `_Handle` +0x44, `_Arrow_1`
  +0x48, `_Arrow_2` +0x4C. Each call: +0x3C = −9; unless mouse off (+3) a
  press on the handle starts a drag (+2, grab offset +0x14 / +0x18 =
  |handle's top left − cursor|), else a press on the backing pages
  (0x52E680: before the handle 10, after it −10); the release ends the
  drag; while dragging 0x52E4F0 moves the handle with the cursor (clamped
  to the track) and gives ±1 once the cursor less the grab offset is more
  than the step (+0x34) from the handle's home (+0xC / +0x10), one step a
  message; not dragging, 0x52E480 gives −1 / +1 for a press on Arrow_1 /
  Arrow_2 (sounds 0x83804C / 0x838067, the table's LEFT / RIGHT on column
  0), at most once in 0.0625 s (+0x38). Layout 0x538090(visible, total,
  pos): the fixed handle (0x52E330) step = (track − handle) /
  max(total − visible, 1), home the handle's **centre** (pos 1: start +
  h/2; pos = total: the end − h/2); the drag compares the top left with
  that centre, so picking the handle up can step −1 at once when h/2 > the
  step (native quirk); the layouts leave a dragged handle alone. Most
  screens take ±1 as one step and ±10 as three (0x7FA1A8 = 3).
  **Sound (observed, sndwatch on the Quick Race track select):** an arrow
  click played one id 3 (as the Right key); a track click that moved three
  tracks played nothing: the screens' step functions are silent, only the
  bar's arrows sound.
* **Icon strips.** IconScrollerMenu (0x552360; UI_Main, the shops' strips)
  builds its strip with 0x543AA0 (vtable 0x79C0FC), whose mouse slot is
  0x538B90: its bar (+0x38) only, 1 or −10 one on, −1 or 10 one back;
  **no wheel, no hover, no icon rectangles** (verified; corrects round
  1). A click on the selected icon accepts it under Wine (round 1,
  **observed**); the code path is **open**. The list scroller 0x54EB90
  (class ctor 0x542CB0, vtable 0x795CB8; MC_List, EA Trax, LAN, SMS, the
  online lists) acts only with the cursor in its box (+0x8C / +0x94, from
  its backing): the wheel steps (0x5430D0); its slots (+0x18…+0x24, items
  shown) are hit-tested: with +0xA8 = 0 a hover on another slot selects
  it (0x538540) and a press posts +0xA4 when set; with +0xA8 = 1 a press
  on another slot selects it and one on the current slot posts +0xA4; then
  its bar (+0x34: ±1, 10 / −10 a page of slots, 0x543560 / 0x5435A0).
  The constructor writes +0xA4 / +0xA8 = 0 / 0; only UI_SMS_Mailbox and
  the online screens (game room, lobby, messenger, find results, feedback,
  friend dialogue) write 0x0C407210 / 1.
* **Option lists** (0x52FC50 from 0x556030, **verified**): with no click
  and no wheel a move selects the **enabled** row (+0x28) whose box (the
  title master's size at the row's place, 0x51FEB0) holds the cursor
  (0x50DB10: UnHighlight / Highlight); outside every row but inside
  DONE_BUTTON (+0x68, 0xD79B07A0) the row is unhighlighted, +0x54 = 0
  and the focus goes to the button (0x52CED0; not ported). A click goes
  only to the selected row's slot +0x8 (a click on another row does
  nothing, the hover having selected it): the generic row (0x52DA00, also
  every Options slider row) Left on LEFT_ARROW_, Right on RIGHT_ARROW_ or
  its +0x30 object, sound 0x837FE0 [type][4 / 5]; no drag on the sliders
  (observed: a press on the knob and dragging did nothing; holding the
  arrow gave one step); a tuning slider row (0x565060) also posts the Tip
  0xC519BFC3 on its TIP_TEXT_. No wheel. The list's own scroll bar runs
  on the **tick** (0x5444A0 on 0xC98356BA, bar +0x6C): ±1 moves the
  selection (0x539380 / 0x5390F0) and blinks its arrow; a track click
  does nothing (observed on Quick Race's Game Options; dragging the handle
  down moved three rows).
* **Tuning widgets** (**verified**): the curve (0x522FA0, vtable 0x79BEC0
  slot +0x24): a move whose x (less 320) lies inside a point's column
  (+0x38, + its width +0x40) makes it the current point (slots +0x18 /
  +0x14: UnHighlight / Highlight); the wheel moves the point (slot +0x20 =
  0x522F20, −1 below 0, +1 above). The nitrous widget (0x531500, vtable
  0x79B804 slot +0x24): a click on 0xD1CDA594 steps parameter 16 down,
  on 0x9345E4A7 up (slot +0x1C = 0x523220, sounds 0x83804C / 0x838067);
  the wheel likewise.
* **Dialogs**: under Wine a move over a button focuses it and a click
  answers it (the profile question, the quit question, the tuning quit
  question); the dialog class's handler is not traced (**inferred**
  mapping: the button's group rectangle).
* **Parts browsers** (**verified**; the body shop **observed**):
  - UI_GenericParts_Browser 0x5664A0: the strip's "ScrollBar" (+0x114)
    ±1 → 0x54FA00 (Left / Right), pages three; the column's
    "SideScrollBar" (+0x16C), the wheel overriding it, → 0x55EBB0 (Up /
    Down), pages three; the button **held** over MINI_MASTER 0x38E39AD7
    (+0x1E0) sends Select 0x0C407210 (under Wine: "Would you like to
    install this part now?"). Under Wine a track click went Front Bumpers
    → Spoilers, a drag back stepped back, the wheel and the down arrow
    stepped one part.
  - UI_Rims_Browser 0x567A60 / UI_ChooseSpinner 0x569680: on the styles
    0x543D50 first; a press on the current style's icon enters its sizes
    (0x55FB10 / 0x5614C0) unless locked (+0x22); +0x7A (strip +0x2E,
    "selection moved") re-previews the rim. In the sizes a press on the
    strip's ScrollBar objects returns at once; a press on the current icon
    or MINI_MASTER → 0x559AC0 (the size's questions) unless locked. Then
    the "SideScrollBar" (+0x1B4) ±1 (the rims' only in the sizes, the
    spinners' in both) else the wheel → 0x55F750 / 0x561150; no pages.
  - UI_IcePartsOverlay 0x568D40 and UI_PartsList 0x56A310 (the neon
    list): SideScrollBar (+0xB4) or wheel → Up / Down, pages three;
    MINI_MASTER held → Select. UI_CustomHUDOverlay 0x563D00: ±1 or wheel
    (0x55D670), no pages; MINI_MASTER held → **Accept** 0x406415E3.
    UI_DecalsOverlay 0x56B970: wheel, else ±1 → 0x560110; no pages.
  - UI_ChooseVinylLayer 0x56AB10: 0x543D50, then 0xCF6F0439 / 0xABA93D3F
    → 0x5632A0(PAD_LTRIGGER / PAD_RTRIGGER), the layer reorder.
  - UI_Paint 0x563420: on the slots (+0x188 = 0, +0x189 set) 0x543D50;
    in a slot 0x55D420: a move (no press) over a swatch (0x5308C0, list
    +0x158 less (320 + 152, 240 + 165)) selects it, previews it on the car
    (0x55C200) and centres U2_Paint_HighLight; a press on the current
    swatch plays 0x8380B8[+0x30] and asks 0xC4C5D271 (GenericDialog_SMALL);
    the finish arrows 0xCF6F0439 / 0xABA93D3F → 0x55D0E0(PAD_LTRIGGER /
    PAD_RTRIGGER) with "Blink" (no +0x174 check, unlike the keys).
  - UI_ChooseCustomHUD 0x5644E0: 0xD1CDA594 / 0x9345E4A7 → 0x548E50(∓1).
  - UI_MagazineView 0x565B60: BACK_BUTTON 0x1C7FCF8B hover runs
    0xA3E4657B (Init_Pulse) else 0x5BAB0FC0, a press sends Back.
    UI_MagazineReward 0x55B270: a press on 0x69BA0838 (+0x7C) asks to save
    (0x55B230), hover Init_Pulse / Init.
  - UI_ShowCase_Preview 0x4CF170: corner buttons 0x69BA0835..37 (+0xB0..,
    enabled +0xD4..): a press on an enabled one → 0x49A1C0(k), the state's
    message +0xD8[k] (menu: button 2 Accept; Control Car / Show Off:
    button 2 Back; Take Picture: 1 Back, 2 Accept; k from 0); hover
    Init_Pulse (0x49A110). The menu state 0x4CEA70: a press on
    HIGHLIGHT_1..4 (0xC2EA8DE7..) runs that option, a hover highlights it
    (0x49A050).
* **Car and track selects** (**verified**; the Quick Race ones
  **observed**): UI_CareerCarSelect 0x4DFC50 (ctor 0x4F9AD0, bar
  "Scroll_Bar" +0x60), UI_CareerCarLot 0x4ED9E0 (ctor 0x4F9930, +0x88),
  UI_QRCarSelect 0x4FC1F0 (ctor 0x4F9D90, "ScrollBar" +0x6C) and
  UI_QRTrackSelect 0x4EFA10 (+0xC0): the bar's −1 / +1 step Left / Right,
  a track click three; no hover, no wheel, no click on the icons (a click
  on the next track's icon did nothing). UI_QRCarSelect first takes a
  press on the filter arrows 0xCF6F0439 / 0xABA93D3F (with +0x1A7 set):
  "Blink" on it and 0x4EED10(PAD_LTRIGGER / PAD_RTRIGGER) (the keys do
  not blink them). The crib's steps skip the direction store +0x1A8 the
  keys make (the platform swing may use the last key's way, **inferred**).
  UI_PostRaceStats 0x4FBE20: a press on ScrollBar_Arrow_1 / _2 →
  0x4F3DE0(Left / Right). UI_QRModeSelect (0x4B2B30) and
  UI_QRModeOptions (0x4EF620) have no mouse case of their own (the icon
  strip, the option list). UI_PostRaceResults (0x4E9A20), UI_PostRaceReward
  (0x4D7350), the tournament standings / results (0x4EA6A0 / 0x4F4370)
  and UI_postrace have **no mouse case** (only the help bar works there).
* **Options, controls, EA Trax, profiles** (**verified**; observed under
  Wine where noted): UI_Options_PC_Controller 0x4ED140: a move over a key
  cell (0x4CB6B0 over the cells' boxes) selects it (0x4DEC40), over
  DONE_BUTTON drops it and focuses the button, over the header +0xE4
  drops it and highlights the header (observed); a press on the current
  cell starts the capture as Accept does (0x5C2110) but without its
  "Pulse" call (under Wine the cell still pulsed: **open**); presses on
  +0xDC / +0xE0 step the device (0x4ACAA0); no wheel, no bar.
  EA_Trax_Jukebox 0x56B1D0 → 0x565DA0: the play-mode arrows +0x10C /
  +0x110 (0xCF6F0439 / 0xABA93D3F) flip 0x83AA58 with their trigger's
  sound (observed); outside reorder mode its list scroller (0 / 0: hover
  selects, observed); then per row: a press on its left / right arrow
  steps the **selected** track (0x53DEC0), a press in the row sends Select;
  in reorder mode a band above / below the list scrolls Up / Down with no
  button. MC_List 0x4E8FC0: the list scroller (0 / 0). UI_DateEntry
  0x55E170 → 0x544170: hover focuses a field, arrows ±1.
* **Maps, SMS, status** (**verified**; observed where noted):
  UI_InGame_WorldMap 0x4E08E0 (ignored while +0x61, a GPS dialog) →
  0x4CBEE0 over its zone list: markers (kind 0, the icons), legend rows
  (kind 1, ICON_BACKING_n) and the zoom arrows (kind 2,
  0xCF6F0439 data 3 / 0xABA93D3F data 2 → 0x4CBDA0). A press: a marker of
  a shown group is selected (0x4967F0) and the GPS button works
  (0x4B10C0: engage or deactivate question); a row is focused and toggled
  (0x4B0E70 + 0x4B0EF0); an arrow zooms. A cursor in a zone other than
  the last ([0]) makes it the last and focuses the row or, with the GPS
  off, the group shown and the marker on the map, selects the marker;
  outside all zones the focused row is dropped (+0x5C = 0) but [0] stays
  (observed: leaving and coming back did not refocus). No wheel.
  UI_CareerWorldMap 0x4E0F60 (the handler, not a tick): ScrollBar_Arrow_1
  / _2 → the stage (0x4E0E50); the "SideScrollBar" (+0x19C) or the wheel
  → 0x4CC140 (Up / Down, pages three); MINI_MASTER held → Select.
  UI_SMS_Mailbox 0x4CCC10 (while +0x1AC): the rows' list scroller
  (Select / 1); clicks on PC_LTrigger_Icon / PC_RTrigger_Icon step the
  mailbox (observed; that code is **open**). UI_Status_Master 0x4B1F20:
  ScrollBar_Arrow_1 / _2 → the page (observed); UI_Status_Career 0x4B1B60:
  0x640F0F22 / 23 → 0x4B1A50 (Up / Down, observed). UILANServerSelect
  0x4FB680: the list scroller (0 / 0). Online screens (game room, lobby,
  rankings, messenger, find results, feedback, view career, friends,
  UI_OLCarLot) are traced in the symbols only (online is not ported).
  UI_ReplayControl 0x4D7BD0 → 0x4D7B30: click-to-show the hidden bar, then
  skip back / slow motion / skip forward / camera / back / hide buttons
  (play / pause 0xCBEDA229 is not tested): ported (see the clone's notes
  below). 0x544900 is a list widget's message function (owner **open**).
* **Clone** (`frontend::mouse`, `src/bin/game/input/mouse/`): the window's
  events are sampled once a frame; actions go through the keyboard's path
  (`Game::menu_key`), so keys and pads are unchanged; a scroll bar's
  steps, its pages and the wheel's steps go through it **muted**, and an
  arrow click plays the bar's LEFT / RIGHT sound itself. Hit tests use the
  last frame's drawn rectangles per package object (groups as their
  children's union) and, where the game's hit test ignores visibility, the
  hidden images' too (`rect_any`); each object's wide-screen placement
  (`display::Place`) maps the cursor, so clicks land at 16:9, 21:9 and 4K.
  The scroll bars record their layout (`scroll_bar_at` → `mouse::Bar`) and
  a dragged handle stays under the cursor. The swap flag follows
  SM_SWAPBUTTON on Windows (elsewhere the left button clicks).
  **Real window events** (2026-11-01): winit's CursorMoved
  (physical pixels, the same space as the surface: no HiDPI scaling),
  MouseInput and MouseWheel feed `Game::mouse_event`; the frame's update
  samples them. A press and its release inside one frame (a quick tap,
  tap-to-click) count as held for that frame (clone choice; the
  original's immediate GetDeviceState could lose it). Fixed since: the help bar's Quit Game slot (bit 1) now opens the quit
  question (it sent the 'Q' key into the screen, which ignored it); MC_Main's
  Yes / No and its messages ("Save complete." Continue) take hover and
  click as the dialogs do (MC_Main's handler 0x4FB8F0 has no mouse case;
  where the original handles them is **open**, the behaviour is
  **observed**); UI_InGame_WorldMap's own help bar (Back, Activate /
  Deactivate GPS) takes clicks as the screens' bar does (it was drawn but
  not hit-tested). The cursor (`Game::cursor_shown`): none while a movie
  plays (0x8383AC) or a load runs; always on the front end with no world
  loaded (g_GameState 3, the splash included, as the original); the menus
  over the world (pause, maps, the dyno) and the replay keep the 2 s rule.
  **Agreed deviations:** no cursor while driving (free
  roam, races, the in-engine scenes; the original shows it 2 s after a
  move, observed above), and the system cursor is hidden over the window
  in every mode (the original hides it in full screen only).
  `scripts/mouse_xtest.sh` drives the windowed game on a private Xvfb
  display (bwrap: no sound devices, no session sockets, its own
  XDG_DATA_HOME) with xdotool at 1280 × 960, 1920 × 1080, 2560 × 1080
  Wide and Stretched and a 1.5 HiDPI scale, and checks the game's
  `NFSU2_INPUT_LOG` (per frame with mouse activity: the edges, the
  cursor drawn, the screen and its selection; `NFSU2_INPUT_PROBE` adds
  objects' window points).
  Shots: `mouse_main`, `tune_tip`, `mouse_drag` (UI_Main's handle dragged
  2.5 steps, a track click back), `mouse_parts` (a track click pages
  three, the wheel steps a part, MINI_MASTER selects), `mouse_paint`
  (swatch hover and click), `mouse_options` (row hover, arrow click),
  `mouse_controls` (cell hover, click captures), `mouse_lists` (MC_List
  hover, EA Trax hover / arrow / play mode), `mouse_carlot` (track arrow,
  car track click, filter arrow), `mouse_map` (legend focus and drop,
  zoom arrow), `mouse_status` (page arrow); each passes at 1280 × 960,
  2560 × 1080 and 3440 × 1440. **Inferred / open in the clone:** the
  wheel steps are muted like the bar's (the step functions' silence is
  observed for the bar only); the Showcase corner buttons are taken as
  "enabled" when drawn; the controller cells' boxes are the column
  masters' width a row tall; MC_List's, the jukebox's and the mailbox's
  slots are their rows' backing / highlight objects; the strips' wheel box
  is not used (IconScrollerMenu has no wheel). Ported 2026-11-01
  (small171): the help bar's hover pulse (above); UI_MagazineReward's
  button and UI_MagazineView's BACK_BUTTON (Init_Pulse while hovered,
  "Init" / 0x5BAB0FC0 otherwise) and UI_ShowCase_Preview's corner buttons
  (0x49A110(k): Init_Pulse 0x78F9C8 on +0xC8[k] = 0x69BA0835 + k, "Init"
  on the one before) through `Game::fe_hover` and the screens'
  `quads_hover`; UI_ReplayControl's buttons (0x4D7BD0 → 0x4D7B30, objects
  from the constructor 0x4C0AA0 at +0x54…+0x6C): 0xAC6BDBC6 skip back,
  0x8A10C890 slow motion, 0xA6B6DD4A skip forward, 0x9C0AE9AA camera,
  0x8A094D85 Back, 0x8A0AB225 hide (unless +0x50); a click with the bar
  hidden (+0x74) shows it (0x50C600 / the button flags → 0x4A5200);
  play / pause 0xCBEDA229 (+0x58) is not tested (`Game::replay_click`,
  the keys' own paths; the replay records its objects for the mouse).
  Not ported: DONE_BUTTON's focus, the filter / finish arrows' Blink on
  a click, the jukebox's reorder band, UI_DateEntry and the online
  screens. **Inferred**: the replay's +0x50 latch (set by a held button,
  0x8763F3 / F6) is not modelled.

## Shop previews through the real loop (2026-11-01)

The earlier checks (`--shot finished_shop_checks`) set the shop screens up
directly. This round ran the windowed clone on a private Xvfb display
(bwrap, no sound devices or session sockets) with the player's finished career
(`the finished-career save`) as the profile, booted it, resumed the career and
drove the menus with real key events (XTEST): the maintainer's `preview_xtest.sh`
(`NFSU2_DEV_SHOP=<shop>` puts the car in that shop's trigger once free
roam is up; a test aid). The original ran beside it under Wine
(`fastgpu.sh`, the same save). Every body shop list (bumpers, skirts,
spoilers, hoods, roof scoops, headlights, taillights, mirrors, exhaust
tips, wide body, carbon fibre), the rims (brands, styles, sizes),
spinners, the specialties (neon, window tint, lights, split hoods, doors,
trunk audio), the paints, vinyls with colours and decals previewed the
highlighted part on the FE car, Back without buying put the car's own
back and a bought part stayed. Found and fixed:

* **Engine and trunk neon were not drawn** (0x615A20, **verified**): the
  neon draw handles four sets: the underglow (ride +0x7F8, tubes +0x2C18,
  colour +0x358, width 0.045, shortened), the engine (+0x7FC: tubes
  +0x2C64, `ENGINE_NEON_START_i` / `_END_i`, i < 5, on the engine's (slot
  0x0D) model 0; colour +0x364), the cabin (+0x800, +0x2CB0; no shop list
  fits it, not ported) and the trunk (+0x804: `MARKER_TRUNK_NEON_START_i`
  / `_END_i`, i < 4, on the trunk layout's model, +0x21B4; colour +0x37C;
  **inferred** that +0x21B4 is the slot 0x22 part's model). The engine set
  draws only while the hood's channel is not shut and the trunk set while
  the trunk's is not (0x433CD0 on the car's animation +0x2D44; the trunk's
  also always in the front end, 0x8654A4 = 3, where a shut channel gives
  fade 0), each at the channel's open fraction (0x434610): a tube 0.0175 /
  0.0225 wide (0x7A6740 / 0x7A6748) in the colour, then one 10× / 3.5× as
  wide (0x7A6600 / 0x7A65F0) at alpha trunc(A · 0.3 / 0.35) (0x7A6604 /
  0x7A65F4), neither shortened. The colours are each slot's part's RED …
  ALPHA2 (0x635B9B / 0x635BF3 → 0x61FC30 into +0x35C / +0x374) with the
  underglow's pulse (0x622AA8 / 0x622BF8, the same gate).
* **Engine and trunk neon notices** (UI_CustomNeonMain's handler 0x56A890,
  Select on an open category, **verified**; Wine: both on the finished-career save):
  Engine Neon with the car as fitted (0x8392C0) below a level-3 engine
  (slot 0x0D) says "You need to install all level 3 engine, ECU and turbo
  upgrades to access engine neon." (0xD649BE08); Trunk Neon below a
  level-3 trunk layout (slot 0x22) "You must upgrade your trunk to CUSTOM
  LAYOUT in order to install trunk neon." (0xA9E966D7); GenericDialog_SMALL
  (0x558440), OK back to the strip. The clone opened the lists.
* **Open/Close Hood / Doors** (UI_IcePartsOverlay's BUTTON1, 0xC519BFC0 in
  0x568D40, **verified**; the help bar already showed it): on the Split
  Hoods list the hood's channel, on Doors the doors' (category +0x54, 2
  also 3), opens when shut and shuts when open (0x4C18B0 → 0x521050 →
  0x4C1860), nothing while it moves. Under Wine the opened hood shut again
  on the next part (0x560930 → 0x438030). The clone did nothing. **Not
  ported**: the doors' deferred toggle while GarageMain +0x13C = 1
  (0x4C2030, 0x838559 / 0x83855A).
* **Rim and spinner paint** (**verified**): the rims are not painted by a
  light material colour (slot 0x49's material 0x5AE50DDF is the same for
  every colour) but by compositing (0x61D7A0 → 0x6343B0 / 0x61E050 →
  0x61DEA0): for a type with a DUMMY_SKIN slot, the front rim's texture base
  (attribute 0x10C98090) + `_WHEEL` under + `_WHEEL_INNER_MASK` is painted
  with slot 0x49 into `DUMMY_WHEEL%d` (render info +0x588, 0x61C2FB), the
  rim's `SPINNER_TEXTURE` under + `_MASK` with slot 0x4A into
  `DUMMY_SPINNER%d` (+0x58C), and the remap table sends the sources there
  (0x631EC6: +0x23C / +0x240 → +0x29C / +0x2A0). 0x61DEA0 needs the dummy,
  source and mask all 32-bit or all not and of one size; the paint is
  RED | GREEN << 8 | BLUE << 16 | GLOSS << 24, white without a part. The
  32-bit path 0x612EF0: per texel with m = the mask's byte 0, m = 0 keeps
  the source, else 0x611B10 weighs the source by 1 − m/255 and the source
  tinted by the paint (0x611A40: each byte × the paint's × 1/255,
  truncated) by m/255, the source's alpha kept. The palettized 0x612D20
  (sixteen levels, the mask's palette byte 0 >> 4) is not used on PC (the
  dummies are 32-bit). Under Wine yellow powder coat turned the 350Z's
  rims yellow; the clone drew `<rim>_WHEEL` unpainted (`car::composite_wheel`,
  `<CAR>_WHEEL1` / `<CAR>_SPINNER1`). When the composite does not apply
  the rim still draws the dummy (**verified**): the remap entry
  is written whatever 0x61DEA0 returns (0x631EB7…0x631EDB source,
  0x631F44…0x631F56 value = render info +0x588 / +0x58C; the four callers
  ignore the result), so the dummy shows as loaded or as last composited
  (`car::wheel_remap_target`; the clone kept the source). **Open**: a
  missing dummy (the remap then names no texture; the clone keeps the
  source); the dummy's contents after an earlier composite of another rim.
* **The finish label** (Paint_text 0x5484F0, **verified**): the label is
  the finish's hash continued with `_PAINT` (0x548591, 0x43DB80 with
  0x79C324) for every finish, so the rims show RIM_PAINT "Powder Coat"
  (Wine, the 350Z); the clone knew only Gloss, Metallic and Pearlescent.
* **Back without buying** (**verified** under Wine, the 350Z): Back from a
  parts list puts the car's own part back at once (the spoiler previewed
  on GRAVEDIGGER was gone 0.4 s after Back, on the strip again), likewise
  Back from a paint slot (Pearlescent purple back to the car's green). In
  the clone (this copy) every category reverted on Back from its list, by
  key and by a mouse click on Back: body parts, rims (style strip → brands),
  carbon, base and rim paint, vinyls, decals, underglow and engine neon,
  window tint, lights, nitrous purge, hydraulics, spinners, split hoods,
  doors; the report of a part staying until the shop was left did not
  reproduce here.
* Checks: the private windowed test `preview_xtest.sh run <dir> <display>` (each list
  stepped with real keys and checked against the FE car the draw loaded,
  `NFSU2_PREVIEW_LOG`; then Back must restore the car's own part at once;
  a locked category is noted), the shots as before.

## List openings (2026-11-04): which Customize lists open on the installed item

Issue: selecting custom gauges should, like the other parts, start at the one already
installed. Traced in SPEED2.EXE; confirmed under Wine (fastgpu, DXVK,
1280×960, no WidescreenFix, a private Wine copy on :189, profile the finished-career save,
main-menu Customize on the RX-7). Frames in
the maintainer's private captures; slot reads from the FE ride 0x8392C0 (+0x590 + 4·slot, 0x610300).

Every list selects with 0x5386B0 (IconScroller_SetSelected: the list offset −(item position),
so the selection sits on the column's fixed highlight row and the rows above it show; the scroll
follows from the selection alone). What each list's fill passes to it:

| List | Fill | Opens on | Status |
|---|---|---|---|
| Body shop / carbon part lists | 0x53A4C0 → 0x538710 | the fitted part (0x8392C0) when listed (carbon: with CARBONFIBRE 0x721AFF7C), else the head | verified (code) |
| Vinyl layer's group list | 0x53A4C0 (+0x1D7) | the car's layer vinyl (0x8392C0), else the setup's (0x521860), when of the group's kind; else the head | **verified** (Wine) |
| Specialties lists (tint, lights, purge, hydraulics, hoods, doors, trunk) | 0x5606F0 | the fitted part, else the head (slot 0x1D always the head) | verified (code) |
| **Custom Gauges** | 0x56C220 → 0x53C200 | **the head (STOCK)**, whatever is fitted | **verified** (Wine) |
| Custom gauge colour steps | 0x549290 | the step's index (1, the first swatch); the fitted colour is read and dropped | **verified** (Wine) |
| Neon lists | 0x553320 → 0x53BA10 | the fitted neon, else the head; then the slot's view (0x4C2100) | verified (code) |
| Decal slot | 0x560260 | the setup's decal when non-zero; else (no entry, or "No Decal" in the setup) the car's; else "No Decal" | verified (code) |
| Paint swatches | 0x55CC60 → 0x548640 | the setup's / fitted paint's finish and swatch | **verified** (Wine: Gloss, the dark green, row 3) |
| Rim / spinner sizes | 0x55F900 / 0x5612B0 | the fitted rim when its brand and style match, else the head | verified (code) |
| Rim brand strip | 0x546530 | the remembered brand, seeded with the fitted rim's | verified (earlier) |
| UI_BuyPerformanceParts | 0x5627A0 | the first row (0x539660(0)) | verified (code) |

* **Custom Gauges does not jump in the original.** UI_CustomHUDOverlay's constructor 0x56C220
  first runs the parts overlay base 0x56BF80, whose 0x5606F0 selects the fitted gauge, then
  0x53C200, which clears the list (0x5385D0), refills it and selects the head; nothing after it
  (0x50F650, 0x548D30, the handler 0x563D00) selects again. Under Wine: GRIFFIN bought with a
  GREEN backing (0x8392C0 slot 0xA3 → part 0x31BC9B04 GRIFFIN, 0xA4 0x02DDC8F0), the list
  reopened on STOCK and Accept on GRIFFIN opened "Choose Backing Color" on WHITE. The clone
  already does both (`PartsBrowser::key`, `ChooseCustomHud::new`). The player's request is
  therefore a deviation from the original; it is **not** implemented (design decision).
* **Vinyls (fixed)**: the column reads the FE ride, which holds the bought parts and not the
  graphics setup (Wine: with a vinyl only in the setup, 0x8392C0 slot 0x40 read 0). Under Wine
  Tear Set item 3 bought, item 1 then put in the setup: the layer's list reopened on item 3.
  The clone took the setup's first; now `Game::vinyl_list_current` (car, then setup) for the
  column and the group strip's seed.
* **Decals (fixed from the trace, not seen under Wine)**: "No Decal" put in the setup made the
  clone reopen on No Decal; 0x560260 leaves its setup loop on a zero part and takes the car's
  decal. `Game::decal_list_current`.
* **Strip memory** (**observed**, not ported): the icon strips write their screen record's +0x10
  (g_ScreenRecords 0x7F7DC8, 0x50B8C0) on a move. UI_GenericParts_Browser's record is shared by
  the body shop (it held 9 after a move to Hoods) and the graphics shop's vinyl groups (3 after
  Stripe Set, matched against the group's kind by 0x51F7B0); leaving the body shop reset it (1
  on reopening, Front Bumpers), while the vinyl groups reopened on the remembered group even
  with the layer emptied. The clone seeds the vinyl strip from the current vinyl each time and
  the body shop from its first category. **Open**: when the record is reset (−1 / 1), and the
  shared record's effect across the two shops.
* Tests: `list_opening_tests` (screens.rs), `steps_open_on_the_first_swatch_not_the_fitted`
  (custom_hud.rs), selftest `selftest_list_openings` (specialties incl. Custom Gauges, neon,
  vinyl car-over-setup, decal setup / removal).
* **Lists Open on Installed** (2026-11-04, **clone deviation**, opt-in; player: "keep the
  custom gauge and an option to fix that"): an Enhancements-page row (after Crash Reports),
  Off = native, `enh.lists_open_installed` in options.txt, listed in the deviations, reset by
  Restore Native. On: the Custom Gauges list opens on the fitted gauge (STOCK when none or
  stock is fitted; `PartsBrowser::opening_part`, skipping the 0x53C200 head select), and each
  UI_ChooseCustomHUD step opens on its fitted colour's swatch with the preview in that colour
  (`ChooseCustomHud::open_on_fitted`; a colour outside the palette keeps swatch 1). The only
  other lists the table shows opening on the head regardless of the install are
  UI_BuyPerformanceParts (a menu row, not an installed part) and 0x5606F0's slot 0x1D (the
  spinners, which the clone opens through the rim browser on the fitted rim already); both are
  unchanged. **Open** (not traced for the option): whether 0x1D's head select is ever visible. Keyboard and mouse open
  the list through the same `PartsBrowser::key(Accept)`. Back out of a colour step still resets
  that step to swatch 1 as natively (0x911AB364). Off is byte-identical (no code path
  changes). Tests: `lists_open_on_installed_moves_the_gauges_cursor` (screens.rs),
  `open_on_fitted_puts_each_step_on_its_colour` (custom_hud.rs),
  `lists_open_installed_row_toggles_and_restores` (options.rs),
  `lists_open_installed_round_trip_and_native_default` (qol.rs); selftest: the specialties
  lists with the option on (Custom Gauges on the fitted gauge), and `selftest_custom_hud`'s
  steps on swatch 1 off / on the fitted backing and dial colours on. Evidence: internal evidence, `u3b-enhancements-row-off.jpg` (the row, Off).

## Language selection (traced in SPEED2.EXE; verified unless marked)

* **System language** (0x5B7700, the start-up's registry read, called
  from 0x5B7A80): it opens HKLM `Software\EA Games\Need for Speed
  Underground 2` (0x7A30F0; RegOpenKeyEx with write access first, then
  read-only) and queries the string value `Language` (0x7A30A4,
  0x5B7959). Found: the string is compared (0x75DD6F, the compare also
  used for StreamingInstall's "off") with the names of table 0x7FCA08
  (12-byte records {name, language, second code}): English US 0, English
  UK 0, French 1, German 2, Italian 3, Spanish 4, Swedish 6, Danish 7,
  Dutch 5, Korean 8, Chinese (Traditional) 9, English China 9, Japanese
  10, Thai 11; the match's language goes to g_SystemLanguage 0x7FCA00
  (0x5B79A6). Not found (the query fails): 0x7FCA00 = 0, English
  (0x5B79B4, EDI is 0 there). Present but matching no name: 0x7FCA00
  keeps its initial −1. The record's third dword (0 US, 5 UK, 6 French,
  7 German, 8 Italian, 9 Spanish, 5 the Nordic and Dutch, 2 Korean, 4
  and 11 Chinese, 3 Japanese, 12 Thai) has no reader in the code
  (**open**; its values look like the region codes of 0x7FA73C,
  **inferred**). **Inferred**: under Wine a prefix whose `Language` holds
  a name not in the table gives the −1 seen for the controls' key names.
* **Game language** (0x7F70D0, initial −1): 0x512520, run once at start
  (0x57FBFC, after the global bundles load), calls SetLanguage 0x512360
  with 0x5B7B00 = 0x7FCA00, or 0 when it is −1. So the language comes from
  the registry value alone: no file is probed, the executable's region
  byte is not read, and a missing or unknown value is English.
* **Language table** (0x7F7020, 0x10-byte records up to 0x7F70D0): {id,
  name, file, font sets}: 0 ENGLISH `LANGUAGES\ENGLISH.BIN`, 1 FRENCH, 2
  GERMAN, 3 ITALIAN, 4 SPANISH, 5 DUTCH, 6 SWEDISH, 7 DANISH (font sets
  0x7F6E60), 8 KOREAN (0x7F6ED0), 9 CHINESE (0x7F6FB0), 10 JAPANESE
  (0x7F6F40); every lookup walks the table for the id and takes record 0
  when none matches (0x4FF780, 0x512360), so Thai (11) loads English.
* **SetLanguage** 0x512360(id) (nothing when unchanged): unloads the old
  language's text (0x580EA0 on 0x8383CC) and its loaded font sets
  (0x490150 on record +0xC +0x00 / +0x40 / +0x20, flags +0x60 / +0x68 /
  +0x64), stores the id, then 0x4FF780(1, 0x8654A4 = 3, 0x8654A4 = 6, 1):
  the record's file read whole (0x57CF00 with mode 7, the request flags
  from 0x57D910 against 0x7F6DE4, kept at 0x8383CC, waited for with the
  last argument) and the font sets loaded (0x494D60(set, 8, …), each a list
  of eight texture hashes, **inferred** to be the font pages); then
  0x468F30(id) on 0x82B884 (**inferred**: the speech bank by language,
  docs/formats/audio.md), the metric default 0x83AA1A = (id ≠ 0), the FE
  refresh 0x50C7E0 when the FE exists, the number separators 0x43CB20(a,
  b, 3) — English and 8+: '.' and ','; French: ',' and ' '; German to
  Danish: ',' and '.' (**inferred** which is the decimal mark; Overbright's `frontend::format::Separators`) — and, when
  0x83AA00 is clear, the default name buffer 0x83A9E0 from label
  0xCF4F80C2 (0xC01A6F63 when missing); last it tail-calls 0x4A95F0(id).
  LANGUAGES\LANGUAGETEXTURES.BIN (0x79AA64) is loaded with the front end
  (0x4FF75B), not per language.
* **Readers of the game language**: 0x4FF970 returns it; new settings'
  metric flag +0x16 = (language ≠ 0) (0x50261D, 0x511711); label
  choices by language (0x511EB9, 0x511F9B), by region and language
  (0x50D35A, 0x553A49); Korean / Chinese text scale for one font hash
  0x9583AA1A (0x536FCA); the profile name's starting text (0x5564EE, with
  the system language). The system language 0x7FCA00 itself is read by
  the controls' key names (0x5B8C4E..0x5B901A) and the name editor.
* **Debug cycler** (0x527530, registered as a key handler at 0x52CE2D,
  gated by 0x8383C8, set only at 0x57612D): steps to the next id below 14
  whose file exists (0x57CAD0) and calls SetLanguage (**inferred**: a
  developer key, unreachable in normal play).
* **Not read by the executable**: LANGUAGES\Labels.bin, Largest.bin and
  the agree.* files (no string names them).

## Shop menu animations (traced in SPEED2.EXE; checked against the original under Wine at 60 fps)

* **Scroll ease.** Both scroller classes ease their offset with 0x574E40 (cubic Hermite, length 0.3 s): the screens'
  IconScrollerMenu (0x543AA0 sets +0xB4 = 0.3 at 0x543BDF) and the parts browsers' strip and column (0x5435E0 mallocs
  the ease and sets its length 0.3; used by UI_GenericParts_Browser 0x55E830, the rim, decal, ICE and neon
  browsers). The update 0x538770 places the items at the offset (0x52EEB0) and steps the ease; a new selection
  (0x52EE50) sets the target to minus the item's place and restarts from the current value and speed (mode 2);
  filling the scroller (0x5386B0) puts it at rest on the item. Clone: `ScrollEase`
  (src/frontend/widgets/scroll_ease.rs), used by `IconMenu` and `PartsBrowser::step` (stepped on the tick).
  Observed: a strip step slides with an ease over about 150 ms of visible motion at 60 fps.
* **Accept on the Car Shop (FORWARD).** 0x543D50 on 0x0C407210, for an option neither locked (+0x22) nor direct
  (+0x45): 0x51FD20 (the strip's fade by place off), 0x52FC00 (input off), 0x50DA20 sends 0x0C407210 to the
  package. UI_ChooseCustomizeCategory's Event Handler answers it with FORWARD (0xDE6EFF34, 140 package ms; 0x301:
  unless already playing it): 0xAADD0187 to every object (their 0xBCBFCC87: alpha to 0 over 100 ms) and
  0xE1FDE1D1 to the screen at 140, on which 0x543D50 runs the option's action. Observed: the strip, title and frame
  fade over five frames and the Body Shop comes up about 130 ms after the key (Wine, 60 fps: fade from the key's
  frame, Body Shop 7 frames later). Most icon menus' handlers answer 0x0C407210 with FORWARD (UI_Main, UI_StartCareer,
  UI_CareerCrib, UI_pause, UI_postrace, UI_QRModeSelect, the customize, paint, rim and vinyl categories and others;
  not UI_OptionsMain or UI_ChoosePerformanceCategory). An option built "direct" (+0x45 = 1) gets the screen's
  handler called with accept at once instead (FORWARD still plays): UI_Main's LAN and Profile Manager (0x4AEF5B,
  0x4AEFE9), the crib's Explore / Resume Career, Tuning and Main Menu (0x4AF870…0x4AFA67), the pause and post-race
  exits (0x4BEEC3…0x4C2F50, 0x5229E1, 0x53CC09), the customize categories (0x520E0C…0x520F07; the Car Shop's own
  handler still switches on 0xE1FDE1D1, observed), the performance package's options (0x556ED1, 0x557089) and the
  Quick Race modes (0x55747A…0x557602). Observed: UI_Main's Customize fades over six frames and Car Select comes up
  eight frames after the key. Clone: `IconMenu::forward`, `accept_plays_forward`, `DIRECT_OPTIONS`.
* **Opening a parts list (ASSLESS).** 0x55EDE0 plays ASSLESS (0x66EC937D) on UI_GenericParts_Browser's Event
  Handler (0x537C10): 0x9F2182F0 to every object, which the column's objects (group 0x4A1316E9, frame 0xD82B4A08,
  bar, arrows, handle, the PARTS_ICON items) answer with their own ASSLESS (alpha up from 0 over 100 ms), and
  0x84FBF415 to the screen at 100 ms. The column's scroller hides its master MINI_MASTER when built (0x5435E0 →
  0x50CA00), so only the PARTS_ICON clones show. Clone: `ScreenState::played`. Observed under Wine at 60 fps: the
  column comes in over about four frames (alpha 0.52, 0.72, 0.93, 1 of full on the frames after the key); nothing
  grows, and the bright spot left of the car is the garage's own light, there before the key.
* **The selection glow** (CATEGORY_ONLY 0xDF422345) runs its looping Animate (1200 package ms: lit, fades out
  over 320, dark, back over 200) whatever the strip does; a step does not restart or hide it (Wine: a step at the
  end of the lit phase fades the glow while the strip slides, the pulse's own timing).
