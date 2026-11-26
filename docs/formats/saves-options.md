# Saves, options and clone deviations

Part of [docs/formats.md](../formats.md), the traced behaviour of `SPEED2.EXE`; the tags
(**verified**, **inferred**, **open**) are explained there.

## Native profile saves and the Profile Manager (2026-10-29; checked against the original under Wine)

Code: `crates/ob-formats/src/formats/native_save.rs` (container, payload offsets),
`src/frontend/profiles/` (folder, list, current profile, MC texts), `Profile::from_native` / `to_native` and `NativeContext` in
`career.rs`, `Settings::apply_native` / `write_native` in `options.rs`,
`McList` / `McMessage` in `screens.rs`, `McAct` and `mc_*` in game.rs.
Fixtures: `tests/fixtures/native_saves/*.sav` (written by SPEED2.EXE; maintainer only (private test set),
the tests are ignored without it, docs/developing/testing.md),
tests `tests/native_save.rs`, selftest `selftest_profiles`.

* **Where** (**verified**): 0x4A9C40 calls SHGetFolderPathA(0, 0x801C =
  CSIDL_LOCAL_APPDATA | CSIDL_FLAG_CREATE), CreateDirectoryA, appends "\" and
  the name at 0x7F66A0 (→ 0x78E8F4 "NFS Underground 2"), CreateDirectoryA,
  hands the root to the memory-card library (0x6FC01E → 0x6FCCFE) and counts
  its entries with FindFirstFileA "\*". Each profile is the file
  `<root>\<NAME>\<NAME>` (no extension); seen under Wine at
  `drive_c/users/<user>/AppData/Local/NFS Underground 2/NAME/NAME` with the
  widescreen fix's CustomUserFilesDirectoryInGameDir off. The library deletes
  with DeleteFileA (0x760B0C) and RemoveDirectoryA (0x760AE2). **Clone**:
  `$XDG_DATA_HOME/NFS Underground 2/<NAME>/<NAME>` (else
  `~/.local/share/…`): the XDG data directory is Linux's local-application
  data, and keeping the game's own folder name and layout lets a profile
  folder be copied between Windows/Wine and the clone unchanged.
* **Container** (**verified** by the reader 0x6FD3F0…0x6FD6E8, the header
  constructor 0x6FC226, and by the original loading a file written outside
  the game): header 0x1C bytes — +0x00 "20CM" (0x4D433032), +0x04 the file
  size, +0x08 section 1's size (8), +0x0C section 2's size, +0x10 / +0x14
  their checksums, +0x18 the checksum of header bytes 0…0x17; section 1 =
  the block type 0x10D and the payload size (0x4D9C15 / 0x4D9CBF; 0x4A9850
  loads only type 0x10D with the expected size); section 2 = the payload,
  always 0xD692 bytes (0x52A450 = 0x5B9690's 0x43E + 0xD254), so every file is
  54966 bytes. The checksum 0x6FE173 (via 0x6FCD1C): CRC-32 poly 0x04C11DB7
  (table 0x8145E0, the standard MSB-first table), register = NOT of the first
  four bytes read big-endian, then each further byte shifted in from the
  right (`r = (r << 8 | b) ^ T[r >> 24]`), result NOT; fewer than four bytes
  give 0. The reader sets error 8 on a bad size, magic or header checksum and 7 on a short
  read (+0x2E0); 0x4A9850 then posts 0xF35D144E instead of 0x461A18EE.
  The two sections' checksums are computed too, but a mismatch's error 8
  (0x6FD6E8 / 0x6FD718) is cleared at 0x6FD73C, so such a file loads
  (**verified**: three community saves edited without fixing the payload
  checksum load under Wine with their values). The clone does the same.
* **Payload** (writer 0x52A290 on G + 0x10 = 0x83A9E0, reader 0x5344E0 — the
  same order, **verified** byte for byte against a running original's
  memory): offsets are payload offsets.
  * 0x0000 garage 0x83AD90 (0x5166B0 / 0x516850; **verified** by the
    writer and, for the records named, by the fixtures). Every record is a
    car object with a vtable at +0 (not saved; 0x79B134 for the full ones)
    and one head saved from +4: +4 a dword (0 in every save), +8 the id
    hash (0x503510 / 0x503680 match it; 0 = free), +0xC the car's raw
    performance rating (0x516C42: 0x5B69A0(0, −1), not scaled; 0 until the
    car's object is built — the stock list's entries get it on the car lot
    or the Customize car select; the 206's 196.7, the Corolla's 144.12 =
    the rating minimum), +0x10 usable, +0x14 the Quick Race filter bits (1
    Stock, 2 My Cars, 4 Career, 8 Sponsor, 0x10 Ranked; 0x5162D0).
    0x0000: 48 stock entries (garage + 0x1C·i, the head only, 24 bytes;
    +0x14 = memory +0x18 the list index, −1 free), 0x480 their count
    (+0x540, 0x1F). 0x484: 20 "My Cars" records (garage +0x544, stride
    0x434, 0x430 bytes saved): head, car record (memory +0x18, 0x418),
    memory +0x430. The main menu's Customize makes one (0x5165E0 takes the
    first free; seen under Wine: filter 2, a generated id, the 206's car
    record with the new front bumper; the race record's +4 then names it).
    0x5844: the five career slots, 0x7F2 bytes each (memory +0x5954,
    stride 0x7F8, 0x516620): +0x00 the head (+4 = bin_hash("CAREER_SLOT_n"),
    filter 4), +0x14 the car record, +0x42C memory +0x430, +0x430 locked
    (+0x7F4), +0x431 has its own car (+0x7F5), +0x432 0x3C0 bytes from
    memory +0x434 (X) by 0x503F00, remapped: X+0…+0x15C as is, then
    X+0x15C (16), X+0x16C (0x100), X+0x26C (16), X+0x27C (16), X+0x28C
    (8), X+0x3BC (the money spent, 4), X+0x294 (8), X+0x2AC (the 0x44
    brand hashes), X+0x29C (16). Those are the owned-part bit sets
    (0x503D50 sets, 0x503DB0 tests; 0x503B10 picks the set by slot
    through table 0x503CAC: slot 7 → X+0x26C, 9 → X+0x294, 0xC → X+0x27C,
    0x1D / 0x1E → X+0x16C, 0x20 → X+0x28C, 0xA2 → X+0x29C, each + 4 ×
    (n >> 5), others X + 4 × the part's cost category (0x501D70); the bit
    n & 31, n the part's position among its slot's parts (0x61BA90
    counting with 0x61B990 for the part's own car; see "Owned visual
    parts" below); slots 0x3F, 0x44…0x47 and 0x49 keep no bit, 0x503D50 /
    0x503DB0's switch 0x503DFC) and X+0x15C the performance parts' bits (0x503E10).
    0x7FFE the slot count (+0x812C, 5). 0x8002: six records (garage
    +0x8130, stride 0x438, 0x434 bytes saved: head, car record, memory
    +0x430 and +0x434), filter 0x10 (Ranked) with full sets of parts.
    0x993A garage +0x9A80 (0; **open**).
  * The car record (**verified**; 0x503950 fills one from a ride): +0x00
    1 once filled, +0x04 0, +0x08 the car type's bin hash (bin_hash
    ("PEUGOT") for the 206), +0x0C + 4·slot the part in each of the 0xAA
    slots (the part's two 16-bit halves, 0 none; up to +0x2B4, so
    +0x258…+0x2B3 are slots 0x93…0xA9) — every slot, stock ones included;
    +0x2B4 the 0x94-byte upgrade state from the ride's +0x2F0 (ten
    category levels, +0x28 + 4i the performance-unique flags, +0x50 + id
    the fitted performance parts); +0x348 the dyno's five tuning settings
    of 0x29 bytes (object +0x360: 0x516A60 fills them with 100 through
    0x5A7940; 0x516AC0 copies setting n to the ride's +0x398; modes in
    Mode Select's order Circuit, Drag, Drift, Street X, URL Race; each 23
    slider bytes then the ECU and Turbo curves' 9 points, 0x59AFC0), then
    3 bytes of padding.
  * 0x993E career 0x85AD40 (0x514530 / 0x529240): eight stage records of
    0x101 (career +0x72D8, stride 0x104, 0x500FA0): +0x00 35 normal wins,
    +0x8C 4 sponsor wins, +0x9C 10 URL wins (each the event's hash +8, filed by
    0x513390 by category +0x34: 3 and not SUV, 1, 2), +0xC4 six dwords,
    +0xDC outruns won, +0xE0 reputation (f32), +0xE4 its race count, +0xE8
    five pointers saved as bin_hash of the name their first u16 indexes in
    the string table 0x838428 (+0xE8 the signed sponsor), +0xFC completed
    (a latch: the stage end check 0x532C70 sets it once the record's
    final event is won with the stage's milestones, or every event of a
    stage without one, and then calls 0x528330(stage), which advances
    the career only when that is the current stage; only the record reset
    0x500700 clears it; **verified**; the clone's `Profile::stage_done`),
    +0xFD the definition's +0x100 as a signed byte widened to a dword (the
    stage number; 0 with no definition). +0xC4 and +0xEC…+0xF8 are 0 in
    every save (**not traced**; the copy 0x511940 moves them with the rest).
    Then (0xA146…) the bank +0x7134, +0x8868 Jump to Garage, +0x872C races
    entered, +0x873E / +0x873F, the stage +0x7AF8, the visit table +0x7AFC
    count and +0x7B00 0x180 × (hash, 4 bytes: +4 visited, 0x512FF0; +6 state,
    1 won, 0x500B80; new entries zero, 0x512F10), +0x873C career under way,
    +0x873D own car, +0x8740 the current slot's name hash (before the car
    lot it is not a CAREER_SLOT hash), +0x8744 saved-place flag, +0x8750
    x, y, z, w, +0x8760 heading (u16, a turn = 0x10000), +0x8764 driving
    time (−1 in a new career), +0x8768 the hidden cash pickups' flags
    (0x100 bytes, see the markers section), the SMS inbox +0x5D20
    (0x5014F0: 256 × (name hash, read, shown, deleted, filed, body,
    argument, subject), +0x1400 count, +0x1404 unread, +0x1408 new), +0x8869
    the World Map legend's show flags (16 bytes, 1 shown; 0x496390's table
    0x49648C: group g → byte 0, 1, 2, 3, 4, 5, 7, 9, 0xA, 0xC, 0xD, 0xB,
    0xE, 0xF, group 13 also bytes 6 and 8), Rachel's hints +0x887C (0x190
    bytes, laid out in **Rachel's speech**; saved by the clone; the loader 0x529240
    copies them and then zeroes career +0x8898, +0x8984, +0x898C and
    +0x8994 — the hints' +0x1C, +0x108, +0x110, +0x118 — so the original's
    load-and-resave writes those four dwords as 0; **verified** in the code
    and by resaving community saves under Wine), and the D-Day event under way
    +0x8738 saved as its hash (0 none).
  * 0xC423 profile 0x83A9E0 (settings block S = profile +0x24 = 0x83AA04,
    defaults 0x5116E0): +0x2D8 (the EA Trax playlist, 27 × (index,
    state)), +0x48 (S+0x24…+0x5F: ten volume floats, S+0x4C the audio
    mode, S+0x54 the EA Trax play mode — 0 Sequential, 1 Random, 0x524130;
    0x565DA0 flips it on the jukebox's events +0x10C / +0x110, under Wine
    '[' and ']' — S+0x50, +0x58, +0x5C 0), +0x38 (S+0x14 rearview, +0x16
    metric, +0x17 race map, +0x18 explore map, +0x1C jump camera f32,
    +0x20 an f32 127.5 the race code reads for race kinds 2–5, 0x6023DA,
    with no option row), +0x84 (the two players' 0x28-byte settings), +0x28
    (S+4 f32 0.5, S+8 / S+0xC 0, S+0x10 a byte that makes the race HUDs
    load Chyron_IG.fng, 0x4AC76B / 0x4F8C58; their writers **not
    traced**), +0xD4 (the race record 0x83AAB4: +4 player 0's car id,
    +0x10 + 20·type the Quick Race records), +0x250 career difficulty,
    +0x254 the number Style_P1 shows (0x4EAAB1 "%s %d"; writer **not
    traced**), 0x844AD9 autosave, 0x844AD8 save prompts, +0xA0F4 (0),
    +0xA0FC the records and statistics (0xA50 bytes: 0x140 records of
    i16 track (0 free), i8 +2, i8 +3, i32 value — 0x502A50 finds (track,
    +3, +2), 0x502DF0 adds or betters one, 0x502AA0 / 0x502C50 the lowest /
    highest value of a (track, +3), the track's defaults 0x5D3E50
    +0x9C…+0xA4 when none; then totals 0x515990 adds after each race:
    +0xA00 f32 sum of racer +0xF8, +0xA08 firsts, +0xA0C races (1, 2, 4
    after Rachel's races), +0xA18 + 12·type three dwords, +0xA44…+0xA4C;
    the keys and totals: **Records** in "Save points, records" below), +0x258
    (0x80, 0 in every save; **open**), +0x00 the name (32 bytes). The loader
    sets profile +0x20 (a profile loaded) and then re-applies the settings
    (0x469050, 0x479220, 0x5BE6A0, the per-player input maps 0x571FC0).
  * 0xD221 the object 0x850080 (0x515BF0, 0x33 bytes): two dwords written
    as 0, +0x49…+0x4B (1, 1, 0), +0x58 and +0x80 five dwords each (0);
    the object's role is **not traced**.
  * 0xD254 the controller maps (0x5B9430 / 0x5B9580, **verified**): six
    blocks of 0xB5 bytes for the devices 0x874C40 + 0x370·k up to the
    count 0x870764 (zeros past it): +0x00 the device's DirectInput
    guidInstance (0x5B8120 → GetDeviceInfo; the loader gives a block to
    the device with that GUID, so the order does not matter) — a block whose
    device is not attached is lost at the next save (the writer has only
    the attached records; **verified** under Wine with a community save;
    the clone keeps only the attached controllers' maps), +0x10 device
    +0x344 (four axis ids), +0x20 +0x354 (their values), +0x30 +0x364
    (four flags), +0x34 +0x368 (0x5B8260 assigns an axis to a role by
    +0x33C), +0x35 device +0x18 (32 dwords: each button's action, 0x5B8470
    → 0x5B8545). The keyboard (GUID_SysKeyboard) has zeros past its GUID,
    and the keyboard map is **not** in the profile: remapping Steering
    (Left) to J under Wine changed no byte (options.sav vs mycar.sav).
* **What the clone maps** (`Profile::from_native` / `to_native`,
  `Settings::apply_native` / `write_native`): name, bank, stage, races
  entered, intro flags, driving time, the cash pickups' flags, the visit
  table (events won = state 1, milestones 1 / offered 2, world challenges 1,
  shops visited +4), the stage records (unlocked = below the stage,
  outruns, reputation, the signed sponsor, the three win lists), the
  legend's show flags (`Profile::map_hidden`, the In-game World Map's
  hidden groups), the D-Day event, the saved
  place, the five slots (locked, car, its parts against the stock ones,
  fitted / owned performance parts, uniques, brands, spent, the five dyno
  settings — `CarSlot::tuning`), the current slot (once the player
  has a car), the inbox; the options above, the EA Trax play mode included
  (`Settings::trax_random`, shown on the jukebox and flipped by its
  triggers). A Quick Race record or a player-settings record the clone did not
  change keeps its bytes (Free Run's record holds 0 laps, which the clone
  reads as 1). Every other byte is kept from the file the profile was
  loaded from (a new profile starts from the original's own fresh profile,
  the same bytes in three runs; since R8 written from its rules by
  `career::fresh_payload`, docs/original/exe-tables.md). A car the
  original has not put in a slot gets a record of the type hash and its
  stock parts only (paint and the rest of a ride's record **not traced**).
  A slot whose car type the clone does not know keeps its car flag and
  record as loaded, and no profile is written when the car types failed
  to load (`NativeContext::check_writable`).
  Since 2026-10-30 (profiles107) also the owned visual parts and My Cars
  (see below). Not kept natively and so lost on save: its random state
  0x7EA624 (not in the save), the trigger counters 0x864210 / 0x86420C
  (not in the save; globals whose only writers are 0x5338D0's increments
  0x533964 / 0x533A2D, so the original never resets them while it runs;
  the clone keeps them for the session and `Profile::load` carries them
  over a Resume or Load), the main-menu car
  when it is not a My Cars record or a stock car, and the total
  reputation (the clone sums the stage records'). The display options and the key map stay in the clone's
  options.txt (the original keeps the display options in the registry,
  0x5BEA30; the keyboard map is not in the profile).
* **Round-trip tests** (`tests/native_save.rs`): five saves written by
  SPEED2.EXE under Wine (career_start, car_lot, garage, mycar — a fresh
  profile after Customize fitted a front bumper to the 206 — and options —
  then the play mode Sequential, track 1 Racing, player 1 Manual,
  difficulty Hard, a key remapped) parse and rewrite byte for byte, load
  into the clone's profile and options and write back unchanged; options
  differs from mycar in exactly the four option bytes; the garage records
  sit where the writer puts them; slot tuning and legend flags reach the
  file and come back.
* **Community saves** (2026-10-31): 44 distinct profile files from
  playground.ru (EU and US builds, stage 0 to 100 %, edited ones too) are
  all the same 54966-byte format and load (three with a wrong payload
  checksum, which the original accepts: see the container above). The
  original under Wine loaded each and saved it again from the Profile
  Manager; its resave differs from the input only in the four hint dwords
  the loader zeroes, the checksums, and (one save) a controller block of a
  device not attached, and the clone's load and save (career and options)
  gives the original's resave byte for byte for all 44 (`finished_resaved.sav`, test `clone_rewrite_matches_the_originals_resave`;
  the community files stay out of the repo, no redistribution terms:
  `NFSU2_COMMUNITY_SAVES=dir` runs `community_saves_rewrite_as_the_original_resaves_them`
  on `<name>.sav` + `<name>.resave.sav` pairs). Found by them: the visit
  table's +4 of a won event or milestone is often 0 in a finished career
  (the finished-career save: 48 events, 3 milestones; won from the World Map's list or
  edited); the clone sets +4 only when it changes the entry's state
  itself, and keeps the file's otherwise.

* **Boot** (**verified** under Wine): one profile → it is loaded and "Load
  complete." shows after the title's Enter; two or more → MC_List "Load"
  with the names in name order; none → "Would you like to create a new
  profile?". The clone does the same after the splash (`boot_profile_step`).
* **UI_ProfileManager** (options 0x4C58B0, **verified**; as shown under Wine):
  Create, Load, Save, Delete, always all four. Each opens MC_Main through
  0x4A3100: Create mode 0x861 (operation 0x60), Load 0xC11 (0x10, callbacks
  0x3A2BE557 / 0x8867412D), Save 0x4A51 (0x50), Delete 0x831 (0x30). MC_Main's
  start 0x4F3570 dispatches the operation (table 0x4F36B8 → 0x4F3690): 0x10,
  0x30, 0x70 list the profiles (0x4E8CE0: memory-card state 8); 0x50 with a
  profile copies its name and goes to state 5, without one leaves at once
  (0x461A18EE); 0x40 / 0x60 the create flow (message 0x5A051729). The
  states (0x4E8D30): 1 "Creating a new profile will …" (0xD36620FE), 2
  "Would you like to create a new profile?" (0x02B641C0), 3 the name
  editor, 5 "Are you sure you want to save^the profile named %s?"
  (0x2A9BA6F9), 6 save (0x4D9AB0), 7 save over (0x4D9B80), 0x7000000 "You're
  over the limit of saved profiles…" (0xB786AF76; the limit is **not
  traced**). Observed flows: **Load** → MC_List → with a profile current
  "Loading a profile will result in the loss of any unsaved changes to the
  current profile. Are you sure you want to load a profile?" (0x81B1D73D,
  Yes / No, No focused) → "Load complete." / Continue → back to the Profile
  Manager (the main-menu car changes to the profile's). **Save** → state 5 →
  "Do you want to overwrite previously saved game?" (Yes / No, No focused) →
  "Save complete." / Continue. **Delete** → MC_List "Delete" → "Are you sure
  you want to delete NAME?" (Yes / No, No focused). **Create** with a
  profile → state 1 (No focused), Yes → the name editor (see MC_Main above).
  All in MC_Main.fng under the header "Save/Load" (0xB19F8B41). The
  library's own texts are memcard/LOCALE_<LANGUAGE>.loc ("LOCH", "LOCI",
  "LOCL" + count + offsets from the LOCL tag, UTF-16LE): 2 Yes, 3 No, 4
  Continue, 6 the overwrite question, 7 "Save complete.", 8 "Unable to save
  %1.", 9 "Load complete.", 10 "Unable to load %1.", 11 "Are you sure you
  want to delete %1?", 12 "Save game deleted.", 13 "Unable to delete %1."
  (what the original shows after a confirmed delete is **not** observed; the
  clone shows text 12). MC_List.fng: header 0x426C7B4D and title row
  0x48D4FCAE = the operation's name, rows option_name_1…8 (0xB2980BA8…, y
  −130…150) with backings 0x66E29936…3D, the title row's arrows
  0x44496A03 / 02 hidden.
* **Save points seen in the original** (Prompt, the default after profile
  creation): the car lot asks "Are you sure you want to save the profile
  named NAME?" right after a stock car is taken; the garage (UI_CareerCrib)
  has a Save item; the Profile Manager's Save. No save at Start New Career or
  at the D-Day garage goal. Ported 2026-10-31: see **The save points**
  below.
* **Leaving the garage** (**verified** and **observed** 2026-11-01,
  small171, a community stage-6 save with Prompt and Autosave on):
  "Return to Explore Mode?" Yes runs 0x4AF360: 0x52A4D0 compares the
  profile with the snapshot the garage's INIT took (0x4AF515 /
  0x4AF4C2, 0x52A470(0): only with save prompts on, profile +0xA108 =
  0x844AD8, and unless one is kept; the snapshot pointer 0x851148 was set
  in the garage and 0 in the city) and asks the save point (0x4A3100 mode
  0x151) only on a change, else 0x9A3B571D → the city. Under Wine: from
  Resume Career and from Jump to Garage, unchanged, straight to free roam
  (twice); after a Car Select in the garage, "Do you want to overwrite
  previously saved game?" and then "Are you sure you want to save the
  profile named Sawa?". The clone does the same (`LotDeal::LeaveCrib`,
  `changed_since_snapshot`; selftest: a change in the garage asks, none
  goes straight). **Open**: a reported case of a question the clone
  skipped was not reproduced; a change made before the garage's INIT
  (outside it) asks nothing in either game.
* **Help bar**: the profile name (EB406FEC) is the current profile's; the
  original showed the loaded profile's name there at once.

### Profile names, settings before a profile, owned parts, My Cars (2026-10-30, profiles107)

* **Profile names and case** (**verified**; `profiles::resolve` / `find`
  / `exists`, `McAct::SaveAsk`, `Profile::save`). The name editor's key
  handler 0x551280 takes Backspace, Enter, Escape and, through
  0x4FF310, letters (a–z, A–Z) and digits for the profile entry (kind
  +0x204 = 6; kinds 4 / 5 also take @ . _ / space , @ ., kinds 0 / 3
  Latin-1), up to 7 (+0x20C), **as typed**: no case is changed. No
  uniqueness check runs in the editor or at the "save the profile named"
  question; the memory-card library decides: the save (state 6, 0x4D9AB0
  → 0x6FBFC1) writes through the file layer 0x6F9BFC (CreateFileA on a
  temporary file, MoveFileA onto `<root>\<NAME>\<NAME>`; folders by
  0x6F975C), whose name lists compare with _stricmp (0x6F9AE6, 0x6F9988),
  on Windows' case-insensitive file system. Under Wine (a private Wine copy,
  :143): with the profile NAME on disk, Create → "name" → Yes asked "Do
  you want to overwrite previously saved game?" (the library's text 6);
  Yes wrote into NAME/NAME (same folder and file name, the payload's
  name now "name"; the help bar showed "name", MC_List the folder's
  "NAME"). So a name that differs from an existing profile only in case
  is that profile. The clone finds folders and the file inside
  case-insensitively (an exact spelling first), asks the overwrite
  question for such a name, and saves a loaded profile back into the
  folder it came from. Load, save, create (the overwrite question),
  delete and the boot all go through `profiles::resolve` / `find`; the
  original has no rename (a name typed over another's in any case is
  that profile, as above). What is shown: MC_List the folder's name (as
  under Wine), the help bar and the save question the payload's own name
  (profile +0, `NativeSave::name`). **Clone choice**:
  folders that differ only in case (possible on Linux only, e.g. "the finished-career save"
  and "the finished-career save" made by older clone builds) are one profile: listed once,
  as the folder whose profile file is the newest (equal times: the exact
  spelling, then name order), and every operation uses that folder; the
  others stay on disk untouched (nothing is merged or deleted
  automatically; deleting removes the chosen one, and the next newest is
  listed then). Tests: `profiles::case_tests`, `tests/profiles_case.rs`
  (own XDG_DATA_HOME), selftest `selftest_profiles`.
* **A profile under another name** (**verified** under Wine): the folder, the
  file and the payload's name (profile +0x00, 32 bytes) changed together, the
  container's checksums recomputed, nothing else. A finished career renamed so
  (`launcher::saves::store::Files::renamed`, the launcher's import and restore
  copies, rename and duplicate) as the only profile of a private Wine copy:
  "Load complete." and MIKE2 in the main menu's help bar. Test
  `tests/native_save.rs` `a_renamed_copy_loads_under_its_new_name` (the same
  file, by its SHA-256; Overbright lists and loads it under the new name and
  its own save writes the same bytes back).
* **Settings before a profile is loaded** (**verified**; `Settings::load`).
  The options other than the display ones live only in the profile: the
  profile object's settings start as 0x5116E0's defaults (every volume
  1.0 but S+0x2C 0.7, Random play mode, …; a new profile runs the reset
  0x515AA0 → 0x515800 again); the registry holds only the display values
  (0x5BEA30 / 0x5BEEB0, `settings.reg`); nothing loads the last profile
  early — the boot reaches the profiles only after the splash (one is
  loaded, several are listed). So the boot movies, the splash and the
  profile list play at the default volumes. The clone's options.txt
  holds only the display options and the key map, and the rest come from
  the current profile or the defaults (applying the last profile's
  options at boot would give a profile with the music at 0 a silent
  first loading screen and no EA Trax caption).
* **Owned visual parts** (**verified** by code, not yet against a save
  with parts bought; `NativeContext::owned_bits`, test
  `owned_parts_round_trip`): 0x503D50 / 0x503DB0 set / test a part's bit
  except in slots 0x3F, 0x44…0x47 and 0x49. 0x503B10: n = 0x61BA90(part,
  car, slot, 0, −1), the number of parts 0x61B990 walks before it (any
  hash, any level; the slot's first group, then its second, 0x6101F0),
  where car is the car type whose name hash is the part's group
  (0x610020 → 0x610140, its +0x840), else 0x2E — one past the 46 types;
  **inferred**: its name hash matches no group, so only the slot's
  shared groups count; then table 0x503CAC → 0x503C90: slot 7 X+0x26C, 9
  X+0x294, 0xC X+0x27C, 0x1D / 0x1E X+0x16C, 0x20 X+0x28C, 0xA2 X+0x29C,
  each + 4·(n >> 5); others X + 4·category (0x501D70); the bit n & 31.
  X = the slot's memory +0x434, saved by 0x503F00's remap
  (`native_save::OWNED_REMAP`). The clone sets the bits of the parts it
  owns and clears those of the parts it does not; bits that name no part
  of the car stay.
* **My Cars** (**verified** unless marked; `Profile::my_cars`,
  `career::MyCar`, `Game::qr_cars`; test `my_cars_load_and_write_back`):
  saved records are 0x430 bytes from memory +4 (+4 the id, +8 the rating,
  +0xC usable, +0x10 the filter bits, +0x14 the car record). Main-menu
  Customize (game mode 0x20, UI_QRCarSelect): Accept needs a free record
  (0x5165E0, +8 = 0), else the dialog 0x558580(…, 0x639, …, 0x91921A8D);
  then 0x552CD0(3, …, id): a stock car (0x516360) gets a new record, id
  bin_hash("TUNED_CAR_<index>") with filter 2 and usable 1 (0x52A720 →
  0x516B90), marked made (0x839BC2); a My Cars record is customized
  itself; 0x502A40 makes the id player 0's car (race record +4). Back to
  the car select (0x55BE00) runs 0x521110: a made record whose 0xAA
  parts all equal the stock car's (0x50E900) is cleared (+4…+0x10 0,
  +0x14 = 2, the car record zeroed) and player 0's car becomes the
  stock entry's id. Stock entries' ids are bin_hash("STOCK_<car>")
  (**inferred** from the fixtures: "STOCK_PEUGOT" = 0x959B1E16).
  UI_QRCarSelect lists the garage records whose filter bits meet the
  filter (All Cars 3 = stock 1 + My Cars 2); the clone lists the stock
  cars, then the My Cars records (**inferred** order), skips empty
  filters, and races a My Cars record with its parts. A record the clone
  makes holds the car type, every slot's part and the fitted performance
  parts (paint data and the rest **not traced**, zero). Delete Tuned Car, the Quick Race customize path, the Ranked
  cars and Back to the car select: see the next section.
* **Not done** (open): see the next section (2026-10-31) for the records,
  the save points, Delete Tuned Car and the Quick Race customize path.

### Save points, records, Delete Tuned Car, clone saves in the original (2026-10-31)

**Verified** (REA / objdump of SPEED2.EXE) unless marked; Wine runs on a
private Wine copy (wined3d, Xvfb :161).

* **A save the clone wrote loads in the original** (**verified** under
  Wine): the test `write_a_clone_save_for_the_original` (ignored; writes
  `$NFSU2_SAVE_OUT`) takes garage.sav and adds owned visual parts (rims,
  slot 7, 9, 0x40) with two of them fitted, two My Cars records
  (TUNED_CAR_0 a 206 and TUNED_CAR_1 a Golf, each with a non-stock front
  bumper, the Golf a performance part), player 0's car = TUNED_CAR_1, the
  intro played (career +0x873C), every shop visited and entered (visit
  table +4 / +6 = 5) and Rachel's hints changed (+4, +9, +0xA). The
  original booted it ("Load complete."), its main menu showed the Golf
  with the bumper, Customize's car select opened on it with both records
  and "Delete Tuned Car" on the help bar; the Profile Manager's Save then
  wrote a file equal to the clone's but for 9 bytes: Rachel's hints
  +0x108, +0x110 and +0x118 (career +0x8984 / +0x898C / +0x8994), dwords
  the original zeroes between load and save (the driving-time reading
  +0x110 among them; who clears them is **not traced**). Clone records
  made before this round lacked the paint: a new car record now holds the
  car type's stock paint (+0x84C) in slot 0x3F and the vinyl colours
  0xD0561E94, 0xD0561E8D, 0x44D28FAE, 0xD0561E8C in turn in 0x4D…0x5C, as
  the original's new records in car_lot.sav (240SX) and mycar.sav (206)
  do (**inferred** from those two: the setter is not traced; test
  `a_new_my_cars_record_has_the_stock_paint`).
* **The snapshot** (0x52A470 / 0x52A4D0 / 0x5032C0 on 0x83A9D0): 0x52A470
  copies the whole payload (0x52A290) to +0x16778 (0x851148) unless one
  is kept, only with save prompts on (0x83A9D0 +0xA108 = 0x844AD8) or a
  true argument; 0x52A4D0 writes the payload again, compares, frees the
  copy and says whether it differs (no copy: no); 0x5032C0 frees it.
  Taken (argument 0) by UI_Main's Options (0x4AECE0, also keeping the
  prompts option in 0x836360), UI_CareerCrib's INIT and its Back
  question's No (0x4AF515, 0x4AF4C2), the car lot's INIT (0x4EDADE), the
  shops' first screens (UI_ChoosePerformanceCategory 0x55C697,
  UI_ChoosePaintCategory 0x562D24, UI_GenericParts_Browser 0x566EFA,
  UI_ICEMAIN 0x5689A1), the online main (0x4BC77B) and 0x552CD0 (a
  customize session, with prompts on and the latch 0x838540 clear).
  Clone: `Game::take_snapshot` / `changed_since_snapshot` /
  `drop_snapshot` on `Profile::native_payload`, taken on those screens
  after Rachel's front-end line (the INITs run 0x5243E0 first, e.g.
  0x4AF502 before 0x4AF515: the line's hint flags are not a change); the
  city start and a race start drop a stale one.
* **The save points** (0x4A3100 with MC_Main operation 0x40 or 0x50;
  0x4F33A0: operation 0x40 with a profile → state 4, without one → state
  2 "Would you like to create a new profile?"; 0x50 → state 5; 0x4E8D30:
  state 4 → 5 when the memory-card object's +6 or +8 is clear (both 0
  peeked under Wine), state 5 → 6 with Autosave (0x844AD9) unless the mode
  has 0x200, state 6 → 7 (save over) when (no 0x100000, card +0xC clear and
  Autosave = card +6)). Observed under Wine on the Options' Back:
  **Prompt**: "Are you sure you want to save^the profile named NAME?"
  (No focused), Yes → "Do you want to overwrite previously saved game?"
  (No focused), Yes → "Save complete." (Continue); No at either → on
  (UI_Main). **Autosave**: no question and no message; the first autosave
  of the session still asked the overwrite question (card +6 was 0, 1
  after it), the next wrote at once. The points:
  - UI_OptionsMain's Back (0x4B6010): prompts on and (the profile changed
    since the Options opened or the prompts option differs from 0x836360)
    → mode 0x2042, then UI_Main; the snapshot dropped either way.
  - UI_CareerCrib (0x4AF3A0): Back's "Return to Explore Mode?"
    (0x60B42AFB, GenericDialog_SMALL) Yes → 0x4AF360: changed since the
    crib opened → mode 0x151, then the city start (0x461A18EE /
    0x8867412D → 0x5404B0).
    Back asks that only when the crib was entered from the city: opened
    by the career start (Resume / Load, 0x836324 set at 0x495A5C) it asks
    "Are you sure you want to quit to the main menu?" (0xCB5E3178, Yes
    0x30ED2368, as Back to Main Menu), No focused in both (0x4AF59F).
    **Observed** under Wine (garage.sav loaded, Escape; then with 0x836324
    cleared): the question over the garage scene alone, no strip.
  - The car lot (0x4ED8A0 state 6): after a car is taken, changed →
    mode 0x141, else 0x9A3B571D; then the city (the question under Wine
    right after the stock car).
  - The career shops' exit to the city (0x552C90, called from the
    performance category 0x55C846 / 0x55C891, the paint category 0x562D7B
    / 0x5630D3, 0x567717, UI_ICEMAIN 0x568CEF): changed → mode 0x41 with
    the callback 0x552C30 (0x83A9D8 from the MC result, city start).
  - UI_PostRace (0x4F39A0): in a career (0x850078 = 1) with prompts on
    after a race won (0x5287B0(0)): Return to Explore Mode (0x3411395C)
    and Quit to Main Menu's OK (0x30ED2368 → 0x24DD40CF) call 0x54CA40 on
    the career (**not traced**) and mode 0x441, then go on (0x35E413DD →
    0x4A3E30 / 0xD78D7BF1 → 0x4A3E80); no snapshot test.
  - Customize: 0x55BE00 back to the Quick Race / online car select
    (0x83898C 2 / 4) with a change → mode 0x2042 (then the car select);
    in main-menu Customize it only sets 0x838540; the car select's Back
    in game mode 0x20 (0x4B22C0) → mode 0x42 to UI_Main when 0x838540 is
    set, then clears it. The dyno from Customize (0x55E5B5, mode 0x1141)
    and the tuning's quit question (0x5648EB, 0x151) / Mode Select's
    "Are you sure you want to save your profile?" (0x5649E1, 0x51) are
    **not ported** here.
  - The Profile Manager's Save (0x4A51), the boot's create (0x21961) as
    before. Operations 0xB0 (0x4B5A5D, 0x523F59) and 0xC0 (0x565C26) are
    other memory-card jobs (**open**), 0x12141 (0x4F059D) is online.
  Clone: `Game::save_point` (`McAct::SpOverwrite` / `SpSave` /
  `Continue`, `AfterSave`), `leave_shop`, `post_race_save_due`
  (**inferred**: "won" is the clone's last race being a career event now
  won). Everything else only commits: `Profile::commit` brings the
  current profile's payload in memory up to date without the file (the
  original keeps the career in memory); `Profile::save` writes it;
  `Profile::load` reads the payload in memory first. So the clone no
  longer writes the file at its own points (race wins, purchases, SMS…):
  as in the original, progress reaches the disk at the save points
  above, the Profile Manager's Save, or never (Save option Off).
* **Delete Tuned Car** (UI_QRCarSelect, 0x4FC1F0 on BUTTON1 0xC519BFC0
  in game mode 0x20, help bit 0x40000 for a My Cars record not in a
  career slot): a My Cars record (0x5163A0) not in a slot (0x516340) and
  not in garage +0x9A8C's 12 entries (0x503590, **open** what they are)
  asks "Would you like to delete this customized car?" (0x367161F5; Yes
  0xF770 / No 0x061C → 0x51123592 / 0x1FAB5998, No focused); Yes →
  0x4EF090: the record cleared by 0x52A6D0 (as 0x521110), the list node
  removed, the selection moved first to the next car (0xB5971BF1) or the
  previous when it was the last (0x9120409E), the filter All Cars, the
  list refreshed (0x4EEC90, 0x4B2310); with one car left 0x4EF090 sets
  the filter and refreshes instead. Player 0's car (0x83AAB8) is not
  changed. Under Wine: the dialog as above, the 206's record gone, no
  save question then or on Back. Clone: `LotDeal::DeleteTuned`.
* **Customize from Quick Race** (game mode 2, BUTTON1 on UI_QRCarSelect,
  0x4FC1F0): not for a car in the 0x9A8C list or a career slot; a free My
  Cars record is needed (else the limit dialog); the filter becomes All
  Cars (3), then 0x552CD0(2, …, car) and 0x502A40 as main-menu
  Customize; its Back returns to the car select through the save point
  above. Main-menu Customize's Back now returns to its car select (the
  clone went to UI_Main), and the car select opens on player 0's car
  (0x4F9D90 keeps 0x83AAB8; under Wine it opened on TUNED_CAR_1). Clone:
  `Game::customize_car_select`, `customize_select`, `qr_customize`.
* **Ranked cars** (garage +0x8130, six records of 0x438, saved 0x434 from
  +4, filter 0x10): the online ranked cars. 0x4F9D90 turns filter 0x10
  into 3 outside the online modes 8 / 0x100 (and 4 / 8 into 3 in mode
  0x20), so offline nothing lists them; the clone keeps their bytes
  (test `ranked_cars_are_kept`). Their filling is online-only (**not
  traced** further).
* **Records** (profile +0xA0FC = 0x844ADC, `career::Record`): 0x551440
  after a race calls 0x515990(kind, track 0x89E7A0, backward 0x89E7A4,
  laps flag 0x89E7BC, split screen, results record) for player 0 (and
  player 1 in split screen); the kind from the track's +0x94 bits (1 → 3,
  2 → 1, 4 → 0, 8 → 2, else −1). 0x515990 adds per kind to +0xA18 +
  12·kind the result's 0x4FE170 (ftol of +0x104 × ftol(+0x108)) and to
  +0xA14 + 12·kind (result +0 = 1), +0xA4C on kind 1 with result +8 = 9,
  +0xA44 / +0xA48 in split screen, +0xA00 += result +0xF8, +0xA08 +=
  (result +0 = 1), +0xA0C += (result +0 ≠ 1); then 0x502DF0(kind, track,
  backward, laps flag, result): a found entry (0x502A50: track, +3 =
  backward, +2 = laps flag) is bettered when 0x502DA0 says so (kind 2: the
  entry < result +0x104, the drift score; else 0 < result +0x10 <
  0x7FFFFFFF and below the entry); a new one takes the first free entry
  (kind 2 needs +0x104 ≥ 0, else +0x10 neither 0 nor 0x7FFFFFFF); the
  value is result +0x140 (kind 2) or +0x10, the best lap in ticks of
  1/4000 s (UI_PostRaceResults' BEST LAP TIME 0x4BFB90 reads +0x10).
  Readers: 0x502AA0(track, backward) the lowest of any laps flag, none →
  ftol(track +0x9C (forward) / +0xA0 (backward) × 0.001 × 4000 + 0.5);
  0x502C50 the highest score (from 0). UI_QRTrackSelect (0x4CE170) shows
  them for the list node's direction (+0x10, 0), the career's event list
  (0x4B1360) for the event's (drift events, type 5, the score). Clone:
  `Profile::records` (loaded and written in their entries' places, the
  totals after the table kept as the file has them), `add_record` after
  every race (Quick Race and career; the laps flag taken as the route's
  loop, **inferred**), `best_time` / `best_score`; Quick Race's track
  select and the World Map's event time show them (they showed nothing /
  the clone's own per-event times, which were never saved). The totals
  +0xA00…: see **Race totals** under the career playthrough notes (the
  clone keeps them, `career::RaceTotals`).

## Display modes and aspect (clone deviation)

Agreed deviation: 4K and other modern monitor resolutions. Native facts first (**verified** from the assembly), then what
the clone does instead (`render::display`, `frontend::options`,
`render::ui`, `render::camera`, `bin/game.rs`).

* **Native resolution list.** Six fixed sizes, 0x800538 (widths 640, 800,
  1024, 1280, 1280, 1600) / 0x800550 (heights 480, 600, 768, 960, 1024,
  1200), read by 0x5BF620 through g_ResolutionIndex 0x870D1C.
  0x5BE5B0 clears the six flags 0x86E880 and sets each whose size the
  adapter lists among its X8R8G8B8 modes (IDirect3D9 vtable +0x18
  GetAdapterModeCount, +0x1C EnumAdapterModes, adapter 0x8709AC); vendor
  caps there: not ATI (0x1002) and not NVIDIA (0x10DE) up to 1280 × 1024,
  XGI (0x18CA) up to 1024 × 768, NVIDIA device 0x1A0 up to 800 × 600 and
  0x1F0 up to 1280 × 1024. 0x5BE850 (in 0x5BE6A0) moves the index down to
  an available mode. The row (0x49B600) steps to the next available index
  right (0xB5971BF1) or left (0x9120409E), no wrap, and sets 0x8709D1
  when it changed; its text (0x49B690) is the size string per index, in
  the same Options cycle widget as the other rows.
* **Present parameters** (0x86F870, filled 0x5B9AC8..0x5B9B72): back
  buffer 0x870980 / 0x870984 (the table size, or the client rectangle in
  windowed mode, 0x5B9A70), Windowed from 0x87098C, interval from
  g_VSyncOn (0x5B9B13), MultiSampleType 0x86F880 = 1 (NONMASKABLE) with
  quality 0x86F884 = the FSAA option when it is above 0 (0x5B9B3B). The
  FSAA cap 0x876520 is CheckDeviceMultiSampleType(A8R8G8B8,
  NONMASKABLE)'s quality count (0x5D2724). No refresh rate is set
  (FullScreen_RefreshRateInHz stays 0, the adapter's default).
* **Native aspect.** The FE / HUD canvas covers the target with x and y
  scaled apart (see Front-end quad submission); the player views keep m00
  and take m11 from h / w (0x5C7ED0, see Chase camera): on a screen wider
  than 4:3 the picture loses height ("Vert−") and the menus stretch.
* **Clone resolution list** (`display::mode_list`): the native six, plus
  1920 × 1080, 2560 × 1440, 3840 × 2160 always, plus every monitor mode
  winit reports (and the desktop size) of at least 640 × 480 — so
  ultrawide sizes (2560 × 1080, 3440 × 1440, 5120 × 1440 …) appear when
  the monitor has them — deduplicated and ordered by width then height as
  the native table is. The row walks it as 0x49B600 does (next size either
  way, no wrap). options.txt keeps the size (`resolution WxH`); an older
  file's index (never applied by the clone) is ignored. A fresh run opens
  1280 × 960 windowed, as before; Default Settings sets 640 × 480 (index 0,
  0x5CECA0).
* **Clone screen mode** (rows after the native four on Options > Display,
  same cycle widget; labels from `options::clone_label`; Alt+Enter toggles
  it): Windowed — the window's inner size in physical pixels is the
  resolution (a HiDPI scale change keeps it: `ScaleFactorChanged`); Full
  Screen — borderless on the window's monitor when the resolution is the
  desktop size, else that monitor mode at its highest refresh rate
  (exclusive; not on Wayland, which has none: borderless at the desktop
  size there, and wherever the monitor lacks the size). The surface always
  follows the window's real size.
* **Refresh and pacing.** The loop's step is the measured frame interval
  (no 60 Hz assumption); Vsync on presents with Fifo (no tearing; on a
  variable-refresh monitor each frame shows when done, up to the
  monitor's top rate). Vsync off (clone row "Vsync Off Mode"): Mailbox by
  default (no tearing, no wait, uncapped) or Immediate (native
  D3DPRESENT_INTERVAL_IMMEDIATE, tears), each falling back to the other,
  then Fifo. No frame cap is added.
* **Clone aspect** (row "Aspect Ratio", `display::AspectMode`; user,
  2026-10-31: "HUD elements should place according to the aspect ratio /
  resolution"):
  * Wide (default): the player views are Hor+ — wider than 4:3, the
    projection is the native one at h / w = 0.75 with m00 × (4/3) /
    aspect, so the centre 4:3 is the native picture (`display::view_aspect`;
    exactly native at 4:3; narrower than 4:3 — 5:4, portrait — the same
    rule, the 4:3 vertical angle with the sides cropped, the resolution fixes 2026-11-12;
    before, the native Vert− there grew the vertical angle to ≈ 161° at
    9:16 and the road under the camera fell inside the 0.5 m near plane).
    Every FE / HUD element keeps its 4:3 size and shape (the lens drops too, 0x5C7AC0's clip-space quads narrowed
    by the same m00 factor, `rain::drop_x_scale`): the canvas is
    scaled evenly by the screen height (by the width below 4:3) and sits
    in the largest centred 4:3 box (`display::canvas_rect`); each quad
    carries a `display::Place` (x, y ∈ −1 / 0 / 1) and is moved that many
    times the *reach* towards the left / right (top / bottom) edge
    (`display::UiFrame`). The reach is the box's distance to the screen's
    edge, but at most to the edge of a centred 16:9 safe area
    (`display::SAFE_SPREAD`, the ultrawide work 2026-11-04, replacing the earlier 2.4 : 1
    `MAX_SPREAD`): front-end side elements always, the HUD unless the
    Enhancements row HUD Layout is Full Width (`UiFrame::hud_reach`,
    `Place::hud`); at 16:9 and narrower the safe area is the whole screen
    (see docs/playing/ultrawide.md). Anchored elements keep
    their native margins to the edge (in canvas units, scaled with the
    screen). What lies off the canvas stays hidden: each run is scissored
    to the canvas as its place moves it.
  * **Fills reach the screen's edges** (the menus' overlays
    adapt to wide screens; `render::ui`, one rule from the
    package data, no per-screen list). A *fill* is an unturned quad that
    is flat (untextured but not a hairline, or a `White16x16*` image,
    `Place::fill`) or magnifies its texture at least 4 × on both axes
    (`FILL_MAGNIFY`: a dim, backing, gradient or strip drawn from a small
    texture — U2_Menu_Box's 16 × 16 is ×40 and more over the canvas;
    never a picture such as PC_load.tga or a movie frame, about ×1, which
    stay a centred 4:3 with the black under them). A fill that spans the
    canvas's width or height, tinted or not, or that touches the canvas
    edge its element is anchored to, is not clipped to the canvas, and
    each of its sides on a canvas edge goes on to the screen's edge,
    continued with that side's edge texels (up to 3 × 3 pieces: the quad
    itself unchanged, so what lies inside the canvas stays as it was).
    So the dialogs' dim (U2_Menu_Box at −215..855 × −85..565, every
    dialog, pause / quit / memory-card / LAN / EA-login question), fades,
    the movie's backing, the loading screens' black, the scene and
    race-start letterbox bars (and the top / bottom at 5:4) cover the
    screen at every aspect, 32:9 included.
  * **Side strips** (`frontend::side_strip`): in a front-end element moved
    to a side (not the HUD), a quad wholly in the opposite outer third of
    the canvas goes to that edge instead, and an unturned quad reaching
    from the left third into the right third stretches, its left end with
    the left edge and its right end with the right (`Place::stretch`).
    UI_PC_Help_Bar's backing strip (RealGroup: the name / cash box
    35..168, the long U2_Menu_Box 171..602 and the rounded ends, canvas
    19..617) so still joins the name box at the bottom left to the
    buttons at the bottom right; before, its right part stayed with the
    name and ended mid-screen. Content panels (lists, dialogs, option
    boxes, the icon strip, the SMS and World Map panels, the title bar,
    which do not span the canvas) stay unstretched and centred.
    Checked: every FE shot (checks.sh's and pause, dialogs, loading, SMS,
    world map, options, profile, memory card, LAN, EA login) at 1920 ×
    1080 and 2560 × 1080, a set at 1280 × 1024, 1920 × 1200, 2560 ×
    1440, 3840 × 2160 and 5120 × 1440; at 4:3 (1280 × 960) every shot
    is pixel-identical to <base> but for the shots' own run-to-run noise
    (the rule needs a boxed canvas; at 4:3 the reach is 0).
  * The place of an element — a package's top-level object and everything
    under it, decided once per object per frame (`frontend::layout` →
    `element_place`) — follows ThirteenAG's NFSU2 WidescreenFix (source
    `Frontend.ixx`, table `NFSUnderground2.WidescreenFix.dat`), which
    keeps the FE / HUD a centred 4:3 canvas and moves listed top-level HUD
    objects, keyed by their package position, to the left or right edge
    (FixHUD; hooks on the FE object draw 0x51B190, the minimap blips
    0x51D665, 0x4C6753 and the stop sign 0x50516B). Its table and the
    clone's rule agree on every listed object (tach (142, 195) and the
    drag tach (−109, −26), N2O, leaderboard, bank, outrun, too-slow,
    test-run module right/left as listed; minimap, SMS, drift info,
    wrong-way, satellite left), except EA Trax (override below). Rules:
    * HUD packages (`HUD_*`, `2PHud*`): by the element's position (centred
      coordinates; the package's, or the one the game's code sets): x
      below −106.7 → left edge, above 106.7 → right, else centred
      (thirds). y: a side element goes to the top / bottom past ±30, a
      centred one only past ±160 (so the countdown, split time, messages
      stay centred; the position box, lap and leaderboard top right; the
      tach bottom right; the drift score top left). Override: objects
      whose resource is `Minimap_*` (the map, mask, north arrow and every
      marker, which the code places round the map) bottom left with the
      map's ring.
    * `Chyron_IG` (EA Trax in game) top left; `Chyron_FE` bottom left.
    * `UI_PC_Help_Bar`: only the screen-level bar anchors to the screen —
      at the canvas foot (its scripted position below y 160) *with* its
      backing strip RealGroup drawn: buttons (x ≥ −100) bottom right, the
      profile name / cash bottom left. Whenever the screen's mask hides
      RealGroup (0x8000 or 0x10000, 0x54E6F0: the screen frames the
      buttons itself), the buttons belong to that screen's panel and stay
      centred with it, wherever its scripts put them: the slot scripts
      In_Game (in-game World Map, SMS inbox and message — Rachel's first
      SMS too — Career Status, the engage SMS dialog), Pause_Menu_Main,
      Pause_Options and Event_Results, and the garage World Map's Init
      layout (0x8401: buttons under the map panel's right end). One rule
      in `frontend::element_place` (the help bar's top-level object, its
      state's RealGroup), so the mouse's rectangles (recorded from the same
      quads) follow it. Before (2026-11-01) the In_Game and
      garage-map buttons went to the screen's corners, outside the panel.
      Checked at 1280 × 960, 1920 × 1080, 2560 × 1080, 3440 × 1440:
      in-game and garage World Map, SMS inbox / message / intro SMS,
      Career Status, pause, pause quit, pause options, quit game, rewards,
      magazines, cover, results, drift results, reward, engage, dyno
      results, standings, tournament; 4:3 pixel-identical to <base> (but
      the garage shots' run-to-run noise).
    * Every other FE package (menus, dialogs, lists, the world map, the
      SMS and engage dialogs, results, movies) stays a centred 4:3 box; the
      3D garage scene behind fills the screen (Hor+). The loading backdrop
      (PC_load.tga, 640 × 480) stays centred with black beside it (a black
      flat quad under it, `screens::Loading`), not cropped or stretched.
    * Quads drawn outside packages: `Place::CENTRE` unless set (the
      dyno graph, the letterbox bars, which fill); the rear-view mirror's
      quad (canvas (200, 15)–(440, 95)) takes `Place::TOP_CENTRE`.
    * A HUD scale (Enhancements) scales each part about
      `Place::origin` (its anchored edge, or its own middle on a centred
      axis).
    At 4:3 nothing moves: the shots are pixel-identical to the canvas
    stretched (checked against <base> at 1280 × 960: main, career,
    explore, race, qrstart, drift, drag, engage, ending, custcars, pause,
    loading).
  * Scale with the resolution (menus and overlays
    scale to higher resolutions): every 2D quad — FE menus,
    dialogs, help bar, HUD, mirror, loading screen and tips, movies,
    subtitles, SMS, world maps, and the clone's own F3 overlay (with its
    deviations rows) and toasts — is drawn on the canvas, so it
    scales by screen height / 480 (by width / 640 below 4:3); nothing is
    drawn at a fixed pixel size. The F3 overlay is anchored to its corner
    (top left by default, `Place (−1, −1)`; docs/notes/performance/overlay.md). Was
    fixed size before: the hairlines (the dyno graph's lines, one pixel
    natively, a D3D line strip) — now round(scale / 2) pixels, 1 up to 960
    lines, 2 at 1440p and 4K. Movies keep their 4:3 object, pillarboxed.
  * Text and art at any size: sampled as fe.efx samples them (bilinear),
    no sharpening or upscaling unless HD UI Art is on (see "2D
    enhancements"; the earlier glyph sharpening above 960 lines and the
    runtime Super-xBR copies are removed).
  * Stretched (Original): the native behaviour above (stretched canvas,
    Vert− views); kept as the original's look, and what Restore Native
    picks.
  `NFSU2_ASPECT=wide|stretched` sets it for `--shot` / `--bench`;
  `NFSU2_SHOT_SIZE=WxH` / `NFSU2_BENCH_SIZE=WxH` set their target size.
  **Open**: not compared with the original + WidescreenFix under Wine at
  16:9 (the placement follows its table, not a capture); the HUD's
  3D-projected pieces, if any are added later, need `Place` from the
  projected point (none drawn today); 2-player HUDs (`2PHud*`) unchecked.
* **What stays native-sized at any resolution:** the road reflection
  target (320 × 240, 0x5C21F0), the car environment cube (128) (both
  bigger only by the Graphics page's opt-in rows), the
  post passes' blur taps in texels; the full- and half-size post
  targets, depth and the multisampled targets follow the screen. The UI
  samples its textures bilinearly (canvas scaled up 4.5× at 2160p).
* **FSAA in the front end.** The game multisamples the back buffer
  itself, so every draw into it — the front end's 3D scenes and the
  FE / HUD quads — is multisampled. The clone now multisamples the garage
  scenes (car select, customise, lots: `draw_garage`, world and car
  passes at the option's samples, resolved before the post passes) as it
  did the game's scene; the FE / HUD quads are still drawn single-sampled
  after the post passes (**open**: only their turned or fractional edges
  would differ). Level → samples: under DXVK, NONMASKABLE quality q is
  2^q samples (dxvk `DecodeMultiSampleType`: `sampleCount = 1u <<
  MultisampleQuality`), which the clone's 2^v matches; a native D3D9
  driver's mapping stays its own (**inferred** for real hardware).
  `NFSU2_MSAA=n` multisamples the `--shot` garage scenes too.
* **Measured** (reference desktop GPU, `--bench`, 600 frames, COROLLA, mean ms per
  frame / GPU busy): drive 1920 × 1080 1.98 / 0.64 (8× MSAA 1.97 / 0.81),
  3840 × 2160 2.16 / 1.05 (8× 2.69 / 1.96), 3440 × 1440 2.78 / 0.98
  (8× 2.83 / 1.27); race 1920 × 1080 1.91 / 0.64 (8× 1.95 / 0.81),
  3840 × 2160 2.54 / 1.19 (8× 3.27 / 2.16).

## QoL (clone deviation): Enhancements page, saves, crash reports, first run (2026-10-31)

Agreed deviations, each **opt-in**: with the defaults
the clone behaves and renders as before (shots pixel-identical, bar the
Display page's new last row). Code: `crates/ob-settings/src/qol/` (`Enhancements`, `saves`,
`crash`, `setup`), `frontend::options` (rows, options.txt),
`frontend::profiles::write_to`, `chase.rs`, `render::display::frame_cap`,
`bin/game.rs` (`game_folder`, `overlay_quads`, `PerfOverlay`). Nothing here
is native; nothing was traced for it beyond the native facts cited.

* **Enhancements page.** Options > Display's last row "Enhancements"
  (button, after the clone's Display rows; also on the pause's Display)
  opens a UI_Options page of clone rows in the native cycle widget
  (labels `options::clone_label`, title "Enhancements"; Back returns to
  Display on that row). Left / Right step and wrap like the native rows;
  each change saves options.txt. Rows, native value first:
  * Field Of View: Native, then −10…+30 degrees by 5 — added to the player
    camera's angle (chase, hood and bumper views: `ChaseCamera`'s
    `game_fov`, so the culling scale follows). Scene, track, garage and
    replay cameras keep theirs.
  * HUD Layout: Centered (default) / Full Width — on screens wider than
    16:9 the HUD's side elements stay in a centred 16:9 safe area or go to
    the screen's edges (`enh.hud_full_width`; docs/playing/ultrawide.md). No
    effect at 16:9 and narrower or with Aspect Ratio Stretched.
  * HUD Scale: Native (100 %), 50–150 % by 10 — the in-game HUD's quads
    only (`qol::scale_hud`; menus, the letterbox and scene overlays
    untouched). Quads that touch or overlap (within 4 canvas px) form a
    part; each part scales toward the corner, edge midpoint or centre of
    the canvas third its box's centre lies in; quads spanning ≥ 90 % of
    the canvas stay. **Open**: parts that do not touch but belong
    together scale slightly apart; checked by eye in free roam (60 %) and
    a drift race (140 %) only.
  * Frame Rate Cap: Off (native, uncapped), 30, 60, 75, 90, 120, 144, 165,
    240 — the existing `NFSU2_FRAME_CAP` limiter (sleep then spin before
    the step); the environment variable still wins. The in-engine scenes'
    30 fps is native and separate.
  * Durable Saves: Off (native) / On — see Saves.
  * Save Backups: Off, 1, 3, 5, 10 — see Saves.
  * Crash Reports: Off / On — see Crash reports.
  * Lists Open on Installed: Off (native) / On — Custom Gauges and its
    colour steps open on the fitted gauge / colours (`enh.lists_open_installed`);
    see "List openings".
  * HD UI Art: Off / On — see "2D enhancements" (the old Vector Fonts
    row is folded into it).
  * Graphics (button): the Graphics page, see Graphics enhancements.
  * Restore Native (button, also last on the Graphics page): every
    deviation the options hold off at once — the rows above native, the
    Graphics page's rows native, and Display's Aspect Ratio Stretched and
    Vsync Off Mode Immediate (window mode and resolution stay).
  options.txt holds the page as `enh.fov`, `enh.hud_scale`,
  `enh.frame_cap`, `enh.durable_saves`, `enh.save_backups`,
  `enh.crash_reports` lines (the Controller group as `enh.pad_*` lines,
  docs/playing/controller-support.md "The settings": `enh.pad_layout`,
  `enh.pad_deadzone`, `enh.pad_trigger_deadzone`, `enh.pad_saturation`,
  `enh.pad_steer_curve`, `enh.pad_rumble`, `enh.pad_pause_on_disconnect 1`,
  `enh.pad_safe_bind 1`, `enh.pad_sticks_quit 1`; In the background as `enh.background`
  1 Keep running / 2 Draw less, Pause being native; Power Saving as `enh.power_caps` 1 Menus /
  2 Menus + Battery), written only when not native (a file with
  none reads as native; out-of-range values are held to the rows' ranges).
  An older `enh.power_saving N` line (0 Off, 1 Background, 2 + Menus, 3 + Battery) is still
  read: N ≥ 1 becomes `enh.background 2` (Draw less) and `enh.power_caps N − 1`; it is
  never written again (docs/original/focus-loss.md).
  The F3 overlay's "Deviations" rows list them (`Settings::deviations`:
  wide aspect, vsync-off mailbox, then the page's; "none" when native).
* **Saves.** The native writer goes through a temporary file and a rename
  (0x6F9BFC: CreateFileA on a temporary file, MoveFileA onto
  `<root>\<NAME>\<NAME>`); the clone already did (`<NAME>.tmp` beside the
  file). The bytes are unchanged by both options (`NativeSave::to_bytes`;
  test `qol::tests::saves_keep_the_native_format` writes the SPEED2.EXE
  fixtures through both and gets them back byte for byte). Durable Saves:
  the temporary file `fsync`ed before the rename and the folder after it;
  a failed rename removes the temporary file. Save Backups: before each
  write the existing file is copied unchanged to `<data home>/NFS
  Underground 2 Backups/<NAME>/<NAME>.<YYYYMMDD-HHMMSS UTC>[-n]` and the
  newest N kept. The backups sit beside the profiles' folder, not in it:
  the original lists every entry of its folder (FindFirstFileA "\*",
  0x6FC01E) as a profile. A backup loads in the original once copied back
  as `<NAME>/<NAME>`. **Open**: not re-checked under Wine this round (the
  bytes are the fixtures' own).
* **Crash reports.** `qol::crash::install` replaces `env_logger::init`
  (same `RUST_LOG` filter and output) and chains a panic hook after the
  default one. With Crash Reports on, a panic writes
  `<data home>/overbright/crashes/crash-<UTC time>.txt`: version, time,
  OS (`/proc/version`), GPU adapter (name, PCI ids, backend, driver), the
  deviations on, the panic message and place, a forced backtrace, and the
  last ≤ 200 log lines (warnings and errors are kept while the option is
  on, plus whatever `RUST_LOG` shows); the home folder reads `~`. Nothing
  is sent anywhere. `game --crash-test` panics on purpose with the option
  on.
* **First run** (`game_folder`, before any thread): `NFSU2_ROOT` when set
  (as before); else the folder kept by `game --set-root <dir>`
  (`<data home>/overbright/game-root.txt`, not options.txt; the text language is kept beside it in
  `language.txt`, or given with `--language`), the current
  folder, then the usual places (Steam libraries, `~/.wine`, Proton,
  Lutris, Bottles, Heroic and CrossOver prefixes'
  `drive_c/Program Files*/EA GAMES/Need for Speed Underground 2`,
  `~/Games/…`) — the first holding GLOBAL/GLOBALB.BUN and
  FRONTEND/FrontB.lzc. None found: on a terminal the clone asks for the
  folder (kept when complete), else prints how to give it and exits (code
  2). The folder is then checked for nine key data files (`setup::KEY_FILES`;
  SPEED2.EXE is not among them, docs/original/exe-tables.md):
  missing ones are listed with the same help and the clone exits; sizes
  are checked on every start, the XXH3 of the eight smaller ones (22 MB)
  when the folder was found rather than given, against the European
  1.2 install; a difference is logged as
  "another version or modded", not an error. `game --check-game [dir]`
  prints the report. The game folder is only read. The asset cache then
  builds itself on first use under the native loading screen (unchanged;
  a first free-roam load takes longer). **Open**: no folder picker in the
  front end (the FE's font and art come from the game files it is looking
  for); the terminal prompt and `--set-root` stand in.

## Graphics enhancements (clone deviation): the Graphics page (2026-11-01; Texture Filtering, SMAA / MSAA, Reversed-Z, Shadow Quality, Reflection Update Rate, Ambient Occlusion added gfx195)

**2026-11-04:** the flat page is now a Preset row (Original / Clean /
Modern / Ultra, recognised from the values, nothing new stored) and six
group pages (Image, Lighting, Reflections, Cars, Effects, Performance),
each value with its GPU cost class. After an image and cost audit,
Texture Filtering, Reflection Update Rate and Reversed-Z Depth left the
page (no visible effect over native Detail 5; still read from
options.txt and listed while on) and Render Scale above 100 % merged into
SSAA (same image). See docs/notes/graphics/graphics-settings-audit.md; the row
descriptions below still hold.

Agreed deviation: extra graphics features in a non-native QoL menu. Every row **opt-in**, every default native: with
the page native the renderer takes the same path as before — no pass, no
target, no size changes — and every shot is pixel-identical to <base> but
for the shots' own run-to-run noise (checked: all 70 shots of checks.sh
at 1280 × 960; every in-game shot — explore, race, drift, drag, hood,
bumper, replay, outrun … — identical to the pixel; the front-end shots
that differ differ by the same counts <base> against itself, ~1k or
~30k pixels, and match <base> exactly on a re-run), bar
options_enhancements (the new Graphics row). Nothing here is native or
traced; no art is added or repainted. Code: `qol` (values, caps),
`frontend::options` (rows), `render::enhance` + enhance.wgsl (the screen
passes), `render::postfx::PostFx::apply` (the hand-over), `render`
(`WorldRenderer::set_view_size`, `sync_environment_size`), `bin/game.rs`
(scene size, navigation).

* **The page.** Enhancements' "Graphics" row (button, before Restore
  Native; also on the pause's) opens a UI_Options page titled "Graphics"
  in the native cycle widget (Left / Right step and wrap; each change
  saves options.txt and is in force from the next frame). Back returns to
  Enhancements on its row. Rows, native first:
  * Render Scale: Native (100 %), 50, 67, 75, 85, 125, 150, 175, 200 % —
    the 3D scene (the game's world, cars, particles, glows, post passes;
    the garage scenes) drawn at that size of the screen's, then taken to
    the screen's: above 100 % a box filter over the source pixels each
    screen pixel covers (four bilinear taps; exactly 2 × 2 at 200 %:
    supersampling, which smooths the alpha-tested foliage, fences and
    rails that MSAA leaves jagged), below 100 % Catmull-Rom (nine bilinear
    taps, clamped). The FE / HUD, the movies and the F3 overlay stay at
    the screen's size, as do the views' aspect and the particles' pixel
    sizes (`pixels_per_metre` from the screen's height). Caps
    (`qol::effective_scale`): the scene's sides within the adapter's
    largest texture, at most 2 × 3840 × 2160 scene pixels and 8 × 3840 ×
    2160 pixel samples with MSAA (half / a quarter of those on an
    integrated GPU; no supersampling on a software adapter): the scale is
    held down to the cap, never below 100 %; steps the cap cannot give
    at the chosen resolution are not offered (at 3840 × 2160: up to
    125 %). The F3 line names the row's value.
  * Texture Filtering: Native / 2× / 4× / 8× / 16× Anisotropic
    — the sampler of the world's, the cars' and the car shadows' textures
    (`display::texture_sampler`) linear with that anisotropy instead of
    what Customize Display's Texture Filtering gives (custom 20: point,
    linear, or linear 16× at "Anisotropic", the World Detail 5 preset's
    — so at the default settings 16× is what native already draws).
    The UI's, the movies', the reflections' (clamped), the headlight
    projector's and the screen passes' samplers are left alone. In force
    from the next load, as the native row (the samplers are made with the
    renderers).
  * Anti-Aliasing (was Post Anti-Aliasing): Off / FXAA / SMAA 1x / MSAA
    2× / 4× / 8×. Off is native: Customize Display's Full Screen
    Anti-Aliasing (custom 14, `display::msaa_samples`; 8× at World Detail
    5) alone.
    * FXAA: FXAA 3.11 quality, preset 12, on the scene's image after the
      native post passes; on top of the native FSAA's samples if both are
      on.
    * SMAA 1x (gfx195; `render::smaa`, smaa.wgsl): Jimenez et al.'s three
      passes in the same place as FXAA — luma edges (threshold 0.1, local
      contrast adaptation 2), blending weights, neighbourhood blending —
      with the orthogonal area table (5 × 5 slots of 16², subsample
      offset 0) computed at start-up from AreaTex's formulas (no file
      shipped), corner rounding 25, searches up to 32 pixels each way
      stepped a pixel at a time with loads (instead of the bilinear
      two-pixel steps and the search table). No diagonal search (no
      diagonal table): SMAA High without `SMAA_DIAG_DETECTION`. Alpha
      (the UI's destination-alpha mask) is the pixel's own. On top of the
      native FSAA when that is on.
    * MSAA 2× / 4× / 8×: the scene's sample count instead of
      FSAA's (capped to what the adapter has, as FSAA's), through the same
      multisampled path (scene, depth, the streak buffer's copy, the
      resolve before the post passes; the horizon fog, shadows, particles
      and glows draw into it). And alpha-to-coverage on the alpha-tested
      draws (world.efx's ALPHAREF 95 test, the cars' too; foliage, fences,
      rails): the alpha's crossing of the reference, spread over the
      alpha's change across the pixel (`fwidth`), as that share of the
      samples, rotated per pixel (`coverage_mask` in world.wgsl and
      car.wgsl, pipelines `fs_*_coverage` with the `SAMPLES` override,
      `SceneMode::coverage`). It is a sample mask, not the hardware's
      alpha-to-coverage: the colour and its alpha (the bleach bypass
      mask) are written as the alpha test writes them. The native FSAA
      keeps the plain alpha test. The depth's readers (the ambient
      occlusion below) read sample 0 of the multisampled depth; nothing
      native reads the depth.
  * Sharpening: Off, 25–100 % — AMD FidelityFX CAS (its no-scale form) at
    the screen's size, last.
  * Road Reflection Res: Native (320 × 240, 0x5C21F0), Half Screen, Full
    Screen (of the scene's size; never below 320 × 240) — view 4's target
    (`ensure_reflection_target`); the reflection is sampled by screen
    position, so only its sharpness changes. Also the garage floor's.
  * Enhanced Reflections: Off by default (`enh.enhanced_reflections 1`).
    The road capture follows the main view's mesh LOD (including the
    native lower World Detail setting) and Detail Distance size threshold,
    in both CPU and vertex culling. Its reflection exclusions, visibility
    lists, per-instance flips and already-mirrored flares/pools are retained.
    The road shader derives each receiving triangle's normal from world
    position derivatives, applies an artistic Schlick weight (35% floor,
    native intensity at grazing angles), and samples a bounded nine-tap
    spatial filter (at most 1.25 capture texels). Texture and vertex alpha
    retain their authored reflection mask. This is a filtered native
    reflection, not a planar camera, SSR or ray tracing: original geometric
    placement limitations remain on slopes, bridges and offset props.
    No history is used. Higher mesh detail and eight extra filtered samples
    cost GPU time. Off retains the original capture and shading formula.
  * Enhanced Car Paint: Off by default (`enh.enhanced_paint 1`;
    2026-11-04). Clearcoat highlight, metallic flake and pearl/candy shift
    per NFSU2 paint type (the paint part's light material: REGPAINT* gloss,
    METPAINT* metallic, PEARL* pearl, CUSTOMPAINT_* candy, chrome/rims),
    with the cube looked up per pixel on those materials. A separate shader
    module (car.wgsl + car_paint.wgsl); off uses car.wgsl's native pipelines
    unchanged. See docs/notes/graphics/car-paint-enhanced.md.
  * Car Reflection Res: Native (128, 0x7FEE6C), 256, 512, 1024 (1024
    added gfx195) — the cars'
    environment cube and its depth (one pass per face or the multiview
    pass); remade on a change (`sync_environment_size`, from the next
    frame no render thread holds the world). The face schedule (Car
    Reflection Update Rate) stays the native option's unless the next
    row says otherwise.
  * Detail Distance: Native, 150, 200, 300 % — the main view's scenery
    size cull (17 px, 0x5D66A4; `render::main_min_size`) and the scenery
    lamps' flare pools (`fill_flare_pools`) divided by the factor: props,
    trees, signs, lamps and their glows are drawn that many times further
    (an instance of radius r from ~21.9 r at native, see **Pop-in**). The
    section lists and the cube's 32 px and LODs stay native. The road
    capture follows main detail only with Enhanced Reflections on.
    Added 2026-11-01.
  * Car Detail Distance: Native, 150, 200, 300 % — the car draws' screen
    size (0x48ACB0, `car_lod`) times the factor before the LOD pick and its
    not-drawn test (size < 4): every car LOD switch and the cars' far cut
    that many times further (traffic LOD B from ~30 m → ~60 m at 200 %).
    Cost of both rows (reference desktop GPU, `--bench explore|race 300`, GPU busy
    median of 3 runs each, 300 %): 1080p explore 0.62 → 0.62 ms, race
    0.71 → 0.71; 4K explore 1.95 → 1.95 / 1.95, race 2.20 → 2.14 / 2.17
    (noise); draw CPU 0.9–1.4 ms either way. Effect on the Wine-matched
    routes: Detail Distance 300 % adds 0–35 pixels a frame (distant lamp
    poles, barrier ends; crop `detail300_r1_180.png`,
    kept outside the repository), Car Detail 300 % puts a van 40 m away at LOD
    A instead of B (`crops/cardetail300_40m.png`, native left).
  * LOD Fade: Off / On (2026-11-01) — see **LOD Fade** below.
  * Keep All Loaded: Off / On (2026-11-01) — see **Keep All
    Loaded** below.
  * Reversed-Z Depth: Off / On — the game's scene (the main
    view, the cars, shadows, particles, glows, the race start's
    characters, the horizon fog; the garage scenes too) drawn with its
    depth reversed: the projection built with near and far swapped
    (`Camera::view_projection_depth`, the same frustum and clipping, built
    directly rather than as 1 − z of the native matrix, whose third and
    fourth rows would cancel in f32), GREATEREQUAL instead of world.efx's
    LESSEQUAL (`SceneMode::compare`), cleared to 0 (`clear_depth`; the
    main view's pass is the scene's only clear), the horizon fog grid's
    z (0.9995 − row·0.0001, already clip space) as 1 − z. The depth is
    Depth32Float either way, so the reversed range puts the float's
    precision where the native range has least: at 1 km with near 0.5 /
    far 20000 the native depth steps ~0.12 m, reversed ~0.1 mm. Nothing
    in the game uses a depth bias (the car shadow's pull toward the eye
    is a scale of its positions, `Shadow::toward_eye`, and works either
    way); the decal-like coplanar draws pass with GREATEREQUAL as with
    LESSEQUAL. The road reflection, the rear-view mirror, the cube and the
    cover shot keep the native depth (their own pipelines and targets;
    their ranges are short). The culling and the CPU projections (glows,
    shadow tubes, horizon fog placement) keep the native matrix. Live:
    the scene's pipelines are rebuilt when it changes (`set_samples`,
    the cars' `sync_scene`, the fog's).
  * Shadow Quality: Native / High / Ultra. There is no shadow
    map in the game: the car shadow is the car's `%s_SHADOWIG` texture on
    a 4 × 4 grid laid on the ground (see **Car shadow**), so its
    resolution and filtering are the art's. What High / Ultra change is
    the grid: each cell split 2 × 2 / 4 × 4 (`Shadow::refine`),
    positions and UVs interpolated, each new vertex at the native
    vertices' height above the ground (interpolated) over the ground
    queried at its own spot (a miss, or more than 1 m off, keeps the
    interpolated height). The native vertices, the texture, the colour,
    the strips kept and the pull toward the eye are unchanged, so on flat
    ground nothing moves; over kerbs and slopes the shadow follows the
    ground between the native vertices. In game only (the front end's
    grid is flat by design).
  * Reflection Update Rate: Native / Every Frame — the cube's
    faces: native is Customize Display's Car Reflection Update Rate's
    schedule (`env_faces`, table 0x7FEEA0: one face a frame, faces 0 and
    4 plus one other, or all six); Every Frame draws all six each frame
    (with the multiview pass where the device has it) whenever the cube
    is on (rate 0 still means no cube). The road reflection is drawn
    every frame natively already. Greyed on software adapters.
  * Ambient Occlusion: Off, 25–100 % — not in the game (its
    lighting is baked into vertex colours); `render::ssao`, ssao.wgsl.
    From the scene's depth alone: at half the scene's size, twelve taps
    on a per-pixel-rotated spiral within 1.2 m (view space from the depth
    and the projection; normal from the depth's nearer slopes), with a
    distance fall-off, reach capped at
    96 scene pixels, each tap occluding by the cosine of its elevation
    over the pixel's plane less a 0.15 bias (scale-free: a body near the
    eye occludes no more than a wall far off); a 4 × 4 depth-weighted
    blur at half size; then at the scene's size a 2 × 2 depth-weighted
    upsample whose factor multiplies the scene's colour (alpha kept)
    after the multisampled scene's resolve and before the native post
    passes and glows. "Before the fog": the darkening is weighed by the
    pixel's world.efx fog factor (clamp((x − viewZ)·y, z, 1)), so a
    fogged pixel loses only its surface's share (exact when the fog
    colour is dark, an approximation otherwise); the sky (depth at the
    far plane) and anything whose 1.2 m covers under two pixels is left
    alone. The game's scene only (not the garage scenes). Particles and
    other blended draws write no depth, so the occlusion of what is
    behind smoke shows through it (**open**: drawing it before the
    blended draws would need the scene split).
  * High Precision Post (2026-11-04): Off / Half-Size Glow /
    Full-Size Glow — the scene and the native screen passes (bleach
    bypass, overbright, light streaks, motion blur, lens drops, tint) on
    `Rgba16Float` targets with every native 8-bit saturation kept and the
    rounding dropped, the overbright glow at half or full size with one
    Gaussian of the native two rounds' variance, and a ±1-step dithered
    clip back to 8 bits; see docs/notes/graphics/hdr-post.md (evidence
    docs/evidence/hdr-post/). Off unless the adapter can render, blend,
    filter and resolve the half-float scene at every offered MSAA count
    (`render::hdr_supported`). `SceneMode::hdr`, `PostFx::scene_format`.
  * Restore Native.
  options.txt: `enh.render_scale`, `enh.anisotropy` (0, 2, 4, 8, 16),
  `enh.post_aa` (0 off, 1 FXAA, 2 SMAA 1x, 3 / 4 / 5 MSAA 2× / 4× / 8×),
  `enh.sharpen`, `enh.road_reflection`, `enh.car_reflection`,
  `enh.detail_distance`, `enh.car_detail`, `enh.lod_fade 1`,
  `enh.keep_loaded 1`, `enh.reversed_z` (0 / 1), `enh.shadows` (0–2),
  `enh.reflection_rate` (0 / 1), `enh.ssao` (percent), `enh.hdr_post` (0 off, 1 half, 2 full glow), written only when
  not native; out-of-range values held to the rows. F3's "deviations on"
  lists them ("render 150%", "FXAA" / "SMAA 1x" / "MSAA 4x", "sharpen
  50%", "road refl. half/full", "car refl. 256/512/1024", "detail dist.
  N%", "car detail N%", "aniso 8x", "LOD fade", "keep all loaded",
  "reversed-Z", "shadows high/ultra", "refl. every frame", "SSAO N%").
  Row order: Render Scale, Texture Filtering, Anti-Aliasing, Sharpening,
  Road / Car Reflection Res, Detail Distance, Car Detail Distance, LOD
  Fade, Keep All Loaded, Reversed-Z Depth, Shadow Quality, Reflection
  Update Rate, Ambient Occlusion, Restore Native (merge m146; the clone
  label ids 0xC1_04E0_25–29 for the gfx195 rows). LOD Fade combines
  with the others: its dither mask is ANDed with MSAA's
  alpha-to-coverage mask (`fs_*_coverage_fade`, `fs_alpha_coverage_fade`),
  its pipelines take the scene's `SceneMode` (sample count, Reversed-Z's
  GREATEREQUAL), the cross-faded car draws use the scene's projection,
  and the faded geometry is in the scene's colour and depth that SMAA
  and the ambient occlusion read. Software adapters (llvmpipe, WARP):
  the three reflection rows greyed (DISABLE), Render Scale without
  supersampling.
* **LOD Fade** (`qol::lod_fade`; `WorldRenderer::update_fades`,
  `FadeState`, the `_fade` entry points of world.wgsl and car.wgsl,
  `CarRenderer::set_lod_faded`, `car::pick_lod_kept`). Nothing here is
  native; off, no code of it runs and the frame is the native one
  (checked below). On:
  * Scenery (the main view only): each frame every instance of the drawn
    sections is sized as the native cull sizes it (0x5D64E0's
    `k · r / (d − r)`, truncated, without its behind test so turning the
    view fades nothing). An instance comes in when the size reaches the
    need (17 px ÷ Detail Distance, as natively) and goes out only below
    0.85 of it (`qol::LOD_FADE_KEEP`: the hysteresis band — at 17 px a
    sign of 1 m radius appears at ~22 m and goes again past ~26 m), its
    fade moving 1 / 0.25 s (`qol::LOD_FADE_SECONDS`) of the frame time
    (`FrameParams::time`, so a paused game holds it) toward in or out.
    The behind test stays in force at once (it never shows a pixel).
  * Whole sections: a section new to the drawn set (a visible-list
    change, or Keep All Loaded's ring) fades in the same way, and a
    section leaving it stays drawn, fading out, until its fade is 0. The
    leaving one draws the other half of the screen-door pattern (its
    fade is stored negative), so where a block is swapped for another
    copy of the place every pixel shows one of the two. A first frame or
    an eye jump over 30 m (a teleport, an event start) shows everything
    at once, as natively.
  * Cars (`car_lod_main` in bin/game.rs: the player, racers, encounter
    racers and traffic in the main view): the LOD is kept while the
    screen size stays within ×/÷ 1.1 of what picks it
    (`car::pick_lod_kept`), and a switch (including to and from not
    drawn, below 4) is drawn for 0.25 s as both LODs, the new one at its
    share and the old with the complementary pattern. The mirror, the
    shots and the front end keep `set_lod` (native, at once).
  * The lamps' flares (`fill_flare_pools`) follow their instance's fade
    (intensity × fade, last frame's; the road reflection's flares keep
    theirs).
  * GPU: one f32 per instance slot (111,352 in Bayview: 445 KB), a vertex
    attribute of the fade pipelines, written only where it changed. The
    main view then draws each group with pipelines that read the fade
    and drop the instances at 0 (native fragment shaders, early depth
    kept), or, for a group with an instance part-way, the `_fade`
    fragment shaders: a 4 × 4 Bayer screen-door as a sample mask, per
    sample with MSAA (the scene's sample count in the frame's `cull.w`) —
    alpha to coverage's effect without touching the alpha, which the
    destination-alpha blends and the bleach bypass mask read. No pass is
    added; the frame's size need is 0 then (the CPU decides).
    Pipelines are built the first frame the option is on (2 × 40 world,
    10 car per sample count; from the disk cache after).
* **Keep All Loaded** (`qol::keep_loaded`; `World::drawn_sections_at`,
  `World::lookahead_sections`, `world::lookahead_admits`; the main
  view, the road reflection, the cube and the flares take the widened
  set; the AI's and the ground's section tests stay native). The
  clone's loader already keeps every Bayview section resident on the GPU
  (1.08 M vertices, 111 k instances, 1,661 textures in one set of
  buffers since the first world renderer; the original streams them,
  **not ported**, see **Pop-in**), so the option allocates nothing and
  loads nothing (VRAM and load time unchanged: free roam load 119–160 ms
  either way here; the 200 ms loading screen rule is untouched). What
  it changes is the drawn set: the eye section's list, then — outline by
  outline from the nearest — the lists of the drivable outlines within
  60 m of the eye (`world::LOOKAHEAD_RING`), i.e. what the native list
  would add when the eye crossed into them. The lists' copies of a place
  are not alternatives by number (base n and detail n + 20 are listed
  together, and a region's panoramas x90–x99 with its detail: e.g. list
  101 = 101–104, 121–124, 199), so a section joins only when every
  section of the set it overlaps on the ground (16 m cells of its
  placements' boxes, `World::sections_overlap`) is drawn with it by some
  native list (`World::listed_together`) — so no two copies are drawn
  over each other in a way no native list does (**inferred** safe: no
  native list shows the pair overlapping; lod180's dropped draw
  distance added drivable sections with no such test and changed road
  pixels). Shared sections (X / Y, 2400+) always join. Offered per
  outline 54 sections on average, kept ~11 at 60 m (`examples/
  vis_lookahead`: 1.8 at 30 m, 27 at 100 m); cached per eye section and
  lending outlines. Alone it moves block swaps earlier and further (they
  still happen at once); with LOD Fade they fade.
* **Pop-in measured** (`NFSU2_POP_STATS=1` with `--bench`, a measuring
  aid; `NFSU2_POP_LOG=file` per frame; counted in the main view's
  frustum, not occlusion-tested: a scenery instance whose shown share
  jumps by half or more in one frame; a block = a section entering or
  leaving the drawn set with a group in the view; a car LOD switch at
  once). `NFSU2_BENCH_CAR=COROLLA --bench drive 3000` (50 s of a career
  sprint on the AI, 1,134 m through 12 sections, fixed clock, 1280 ×
  960):

  | | frames with a pop | scenery pops | blocks (median distance, within 100 m) | car LOD switches at once / faded |
  |---|---|---|---|---|
  | native | 682 of 3001 (22.7 %) | 736 (0.25 a frame, ≤ 4 in one) | 46 (213 m, 13) | 67 / 0 |
  | LOD Fade | 0 | 0 | 0 | 0 / 58 |
  | Keep All Loaded | 809 (27.0 %) | 912 | 55 (278 m, 7) | 67 / 0 |
  | both | 0 | 0 | 0 | 0 / 58 |

  Keep All Loaded alone draws more (so more scenery crosses its cull)
  and swaps blocks further away (13 → 7 within 100 m). Most scenery
  pops are a few pixels or hidden (frame 2215: 1 pixel against LOD
  Fade); the visible ones are block swaps (frame 1222: a lit tower
  swapped for another copy, 2,600 pixels). Frame strips (rows native,
  LOD Fade, Keep All Loaded, both; magnified crops):
  kept with the maintainer's private evidence: block_pop_f1222,
  car_lod_f404, scenery_f1320 / f2215, keep_loaded_f404_native_left,
  options_graphics_lod (the page). With LOD Fade 25 car switches in the
  first 1,230 frames were cross-faded at 1, 4 and 8 samples
  (`NFSU2_MSAA`): no validation error.
* **Cost** (reference desktop GPU, 1920 × 1080, the bench's own settings (8×
  MSAA), median of 3 runs' medians, ms a frame):

  | | explore GPU | explore draw CPU | race GPU | race draw CPU |
  |---|---|---|---|---|
  | native | 0.52 | 0.54 | 0.68 | 0.79 |
  | LOD Fade | 0.53 | 0.55 | 0.68 | 0.85 |
  | Keep All Loaded | 0.52 | 0.61 | 0.71 | 0.88 |
  | both | 0.53 | 0.64 | 0.71 | 0.95 |

  4K race GPU 2.25 → 2.29 with both. The fade state costs 0.06 ms of
  CPU a frame (`world: LOD fade`, worst 0.24), the look-ahead ~0.02 ms;
  the rest is the drawn set's own draws. A first try that drew every
  main-view draw with the dithering pipelines cost +0.14 ms GPU in the
  race (early depth lost), hence the two pipeline sets.
* **Off path unchanged** (checked): all 94 shots of checks.sh at 1280 ×
  960 against <base>'s own binary: every in-game shot identical to the
  pixel; the front-end shots that differ differ by <base>'s own
  run-to-run amounts (a second <base> run differs from the first by
  the same order), bar options_graphics (its eighth row is now LOD Fade
  where Restore Native was). Shots with both on: no panic or validation
  error (explore, race, qrstart, drift, drag, hood, bumper, replay,
  outrun, garage_neon, main, career). Tests: `qol::tests::
  lod_fade_and_keep_loaded_default_native`, `render::lod_fade_tests`,
  `car::shared_part_tests::car_lod_hysteresis_keeps_near_thresholds`,
  `world::lookahead_tests`, the Graphics page's rows test.
* **Open**: the road reflection's flares and the rear-view mirror do not
  fade; a car's shadow switches with the car's LOD gate at once; the
  fades move with the frame clock (`Game::seconds`), not checked in
  replays. Not compared with the original (nothing to compare: neither
  exists there).
* **Order at the frame's end** (`PostFx::apply`): native post passes as
  before, but the last ("to the back buffer") writes an image of the
  scene's size instead of the screen; then FXAA (scene size) → resample
  (screen size) → CAS, each only when asked, the last into the screen;
  then the UI. Alpha is carried through (the UI's destination-alpha
  textures read it as before).
* **Checked** (shots at 1920 × 1080 Wide, and Stretched; `NFSU2_MSAA=4`
  garage shots; the race-start, mirror (hood / bumper), replay and drift
  shots; all on at once; a live change mid-run in `--bench explore` /
  `race` with `NFSU2_BENCH_ENH_SWITCH`): no panic or validation error.
  Crops (native first, kept outside the repository):
  aa_explore_cmp1/2 (render scale, FXAA, CAS on a palm tree),
  roadrefl_*_cmp, carrefl_*_cmp, menu_graphics_*.
* **Cost** (reference desktop GPU, `--bench explore|race 300` with the bench's
  own top settings, 8× MSAA; GPU busy ms per frame from timestamps,
  median of the runs' medians: 3 runs at 1920 × 1080, 5 at 3840 × 2160;
  other jobs on the machine; `NFSU2_BENCH_ENH`):

  | option            | 1080p explore | 1080p race | 4K explore | 4K race |
  |-------------------|------|------|------|------|
  | native            | 0.97 | 1.14 | 1.78 | 2.24 |
  | Render Scale 50 % | 0.62 | 0.80 | 1.05 | 1.11 |
  | Render Scale 75 % | 0.80 | 0.96 | 1.25 | 1.42 |
  | Render Scale 150 %| 1.65 | 1.86 | (capped to 100 % at 8×: 1.80) | (2.24) |
  | Render Scale 200 %| 2.09 | 2.72 | — | — |
  | FXAA              | 1.05 | 1.22 | 1.89 | 2.39 |
  | Sharpening 50 %   | 1.05 | 1.22 | 1.91 | 2.38 |
  | Road refl. half   | 0.98 | 1.14 | 1.80 | 2.26 |
  | Road refl. full   | 0.99 | 1.15 | 1.82 | 2.32 |
  | Car refl. 256     | 0.98 | 1.15 | 1.79 | 2.25 |
  | Car refl. 512     | 1.02 | 1.19 | 1.80 | 2.29 |
  | all (150 %, FXAA, CAS 50, road full, car 256) | 1.74 | 2.17 | 2.06 | 2.61 |

  At 4K the sample cap holds Render Scale to 100 % with 8× MSAA; with
  `NFSU2_MSAA=1` 150 % (held to 141 %) takes explore from 0.82 to 1.56
  ms, with 4× from 1.14 to 2.45. Draw-call (CPU) cost: none of the rows
  adds draws (the reflections draw the same lists into bigger targets).
  Rounding: the scene's size is the screen's × scale rounded per side, so
  the garage scenes' aspect (from the scene's size) can differ from the
  screen's by up to ~0.1 % at the odd scales (67, 85 %): invisible.
* **gfx195 rows: checked** (2026-11-01). Native unchanged: with the page
  native every in-game shot of the check set (explore,
  race, drift, drag, hood, bumper, replay, outrun …) is identical to the
  pixel to <base>'s at 1280 × 960; the front-end shots differ only by
  the shots' own run-to-run noise (<base> against itself the same
  counts; finished_shop_checks' pulsing icon differs with the machine's
  load in <base> too), bar options_graphics (the new rows). Each row,
  and all at once, on the explore, race, drift, hood and main-menu
  garage shots (`NFSU2_SHOT_ENH`, 1920 × 1080; the shots draw the game's
  scene multisampled with the page's MSAA or `NFSU2_MSAA`) and in
  `--bench explore|race` at 1080p and 4K: no panic or validation error.
  Native-vs-on images (kept with the maintainer's private evidence) —
  msaa8_a2c_palm_fsaa8-vs-msaa8 (native 8× FSAA against the page's 8×
  with alpha-to-coverage: the palm's fronds smooth),
  msaa_palm_native-vs-msaa4, smaa_car_native-vs-smaa and
  fxaa_vs_smaa_car (the roof line), aniso_native16-vs-2x_* (the row's
  2× against the default 16×: the road far off blurs, so the sampler
  follows the row), reversedz_* (0.01–0.07 % of the pixels differ:
  overlapping leaf cards and decals at equal depth; no far z-fighting
  was visible with the native range on these shots — the gain is the
  precision headroom), shadows_ultra_race (≤ 12 levels: flat ground),
  refl_everyframe_1024_race, ssao50/100_* (contact darkening under the
  cars, at kerbs and the overpass beams; ~0.2–0.4 mean levels),
  all_explore, hood_/main_native-vs-msaa4_rz (the mirror and the garage
  with MSAA and reversed-Z), options_graphics(_end_values) (the page).
  The race shots' flag girl animates on the wall clock, so her pose
  differs between runs (not the rows).
* **gfx195 rows: cost** (reference desktop GPU, `--bench explore|race 300`,
  `NFSU2_BENCH_CAR=COROLLA`, the bench's settings: World Detail 5, so
  native is 8× FSAA and 16× anisotropic; GPU busy ms per frame from
  timestamps, median of 3 runs; other jobs on the machine):

  | row               | 1080p explore | 1080p race | 4K explore | 4K race |
  |-------------------|------|------|------|------|
  | native            | 0.60 | 0.78 | 2.28 | 2.97 |
  | MSAA 8× (+ A2C)   | 0.62 | 0.85 | 2.30 | 3.10 |
  | MSAA 4× (+ A2C)   | 0.45 | 0.60 | 1.35 | 1.80 |
  | SMAA 1x (+ 8× FSAA) | 0.76 | 0.98 | 2.80 | 3.51 |
  | FXAA (+ 8× FSAA)  | 0.61 | 0.85 | 2.40 | 3.11 |
  | Reversed-Z        | 0.57 | 0.82 | 2.27 | 2.97 |
  | Shadow Quality Ultra | 0.57 | 0.82 | 2.28 | 2.94 |
  | Reflection Every Frame | 0.57 | 0.82 | 2.27 | 2.94 |
  | … and Car Reflection 1024 | 0.60 | 0.81 | 2.30 | 3.02 |
  | Ambient Occlusion 100 % (own run, native 0.62 / 0.80 / 2.37 / 2.90) | 0.75 | 0.99 | 2.99 | 3.64 |
  | all (MSAA 8×, rev-Z, Ultra, every frame, AO 50 %) (same run) | 0.79 | 1.27 | 3.03 | 3.75 |
  | World Detail 3 (linear filtering) | 0.29 | 0.46 | 0.68 | 0.89 |
  | … with 16× Anisotropic | 0.31 | 0.46 | 0.68 | 0.89 |

  Differences under ~0.05 ms are the runs' noise. The ambient occlusion's
  passes at 4K: half-size depth 0.54 ms with the 8× scene's resolve
  before it, occlusion 0.20, blur 0.04, apply 0.17. Draw CPU: Shadow
  Quality Ultra adds ~0.1 ms (the ground queries, 169 per car); the
  others none.

* **Not taken** (candidates of the request):
  * A mip LOD bias with the anisotropy: wgpu samplers have none (it
    would mean `textureSampleBias` in every world / car shader), and a
    negative bias shimmers on the game's textures.
  * World draw distance (2026-11-01): built and measured, then
    dropped. Adding the lists of the drivable sections within 100–400 m
    of the eye (their panoramas x90–x99 left out) added a few distant
    buildings (50–650 px a frame on the six Wine-matched routes) but also
    changed road-surface pixels near the eye (**inferred**: base sections
    the eye's list shows as the region's panorama or leaves out, drawn
    over each other); keeping only what cannot overlap (detail copies
    n + limit of base sections the eye's list already draws) changed 0
    pixels on the same routes. So nothing safe is left to add. (gfx161's
    first try of a bias on the 17 / 32 px, dropped then, is the Detail
    Distance row now: it changes little on the shots' poses, but it is
    what moves the prop / lamp pops.) Fog distance: the fog is the art's
    (and hides the visibility edge). A fade-in for popping scenery: built
    later as LOD Fade (above: a per-instance state kept across
    frames on the CPU, nothing of it running when off); a wider drawn set
    as Keep All Loaded (with a test lod180's try did not have).
  * Shadows: the car shadow is the game's own texture on a quad
    (`shadow.rs`), its resolution the art's; no new content (Shadow
    Quality refines its grid only, see above).
  * SMAA's diagonal search and its T2x / S2x / 4x modes (temporal, or on
    MSAA's samples).
  * World / car texture upscaling: not offered (the UI's sharp bilinear,
    HD UI Art, is for flat UI art; on photographic, block-compressed
    world textures nearest-neighbour steps would show).
  * Particle density / glow quality: the emitters' counts and the glow
    sprites are the game's data; more would change the look.

## 2D enhancements (clone deviation): HD UI Art (2026-11-01; revised ui3; sharp wired for pack art)

Agreed deviation: font and 2D rendering may use modern techniques as long as the
result still looks native. The first version's upscaled font and UI art looked
artificial, so:

* **Off (default) = native sampling at every size.** Every front-end
  quad, glyphs included, samples its texture as fe.efx does
  (**verified**, see `UiRenderer::new`: technique "world" sets AddressU /
  V WRAP and Mag / Min / Mip filter LINEAR, blob at file 0x426E40;
  0x5CC1E5..0x5CC200 set stage 0's ADDRESSU / V WRAP before 0x5378D0
  draws the front end; the mirror's quad CLAMP, 0x5CC186..0x5CC1AE),
  with the half-texel inset (0x5C5030) and the D3D9 pixel-centre shift,
  so a texture magnified 4.5× at 2160p is plain bilinear, as the original
  draws it at that size. The clone's earlier extras on this path are
  gone: the runtime Super-xBR 2× / 4× copies of art above 2 px/texel
  (`render::ui_upscale`) and the glyph coverage remap above 960 lines
  (ui.wgsl mode −1). Hairlines still scale (round(scale / 2) px,
  "Display modes and aspect"); that is geometry, not a filter.
* **One row, HD UI Art** (Enhancements, after Crash Reports, before
  Graphics; Off / On; `enh.hd_ui 1`, written only when on; Restore Native
  turns it off). The Vector Fonts row is folded into it: an old
  `enh.vector_fonts` line is read and ignored, never written.
* **HD UI Art = the pack's art, sampled sharp bilinear** (hygiene,
  2026-11-04; before that `sharp` was unreachable and pack art was plain
  bilinear). With the row on and a complete pack installed
  (`hd_ui_pack::initialize`, chosen once at start), the pack's PNGs replace
  the game textures they name (`render::texture_pack`, `Domain::Ui`).
  `UiRenderer` notes which bound textures came from the pack (`hd`); a
  quad with such a texture that is not a fill (`FILL_MAGNIFY`) gets the
  clip.z flag `SHARP` (4) or, for alpha-blended and additive art,
  `SHARP_PREMULTIPLIED` (8), added to the alpha-test mode. ui.wgsl then
  uses `sharp` where the art is magnified on both axes (> 1 target pixel
  per texel, from the UV derivatives): per axis k = ⌊target pixels per
  texel⌋, the art magnified as if enlarged k× by nearest neighbour and that
  filtered bilinearly to the target — each texel its own flat colour, a
  linear step 1/k texel (1–2 target pixels) wide between neighbours; from
  level 0 with the sampler's wrap; premultiplied art without a dark fringe
  from transparent texels' colour. Below 2 px/texel (k = 1) it is bilinear;
  at and below 1 px/texel (the player's pack at 1080p and below for most
  art) nothing changes. Game textures the pack does not replace, glyph
  pages, movie frames and rendered pictures (`set_texture`, `set_rgba`)
  are never sharpened. No bake, no cache file, no new shapes.
* **Off is byte-identical**: with the row off no pack is active, the `hd`
  set stays empty and every quad keeps mode 0 / 2. Checked: shots
  `ui2d` and `options_hdui` at 3840 × 2160 with
  `NFSU2_TEXTURE_PACK` set and HD UI Art off are byte-identical to <base>
  (the maintainer's private captures). `examples/ui_sampling_check` (GPU) checks
  through the real renderer that a game texture without a pack samples as
  the fill path does, and (re-run with a one-texture pack) that pack art is
  sharpened (texel centres exact at 2160p) while the same art as a fill is
  not; `examples/ui_sharp_check.rs` checks the filter arithmetic on the CPU.
* **Dropped**: the potrace-style tracing and MSDF fonts / logos (the
  traced outline never matched the bitmap at 1×: best logo edge error
  0.032, fonts 0.060–0.067 mean |coverage − alpha|, with the corners
  rounded), the Super-xBR + back-projection bake, BC3 copies, the cache
  files `ui2d-vector.nfc` / `ui2d-hd.nfc` (removed from the cache folder
  at the first cache use, `cache::OBSOLETE`), and the examples
  ui2d_bake / ui2d_glyph_diff / ui2d_upscale_eval / ui_upscale_cmp.
* **Evidence**: captures of an interrupted run are kept
  outside the repository. Fresh private-branch checks and comparisons
  are described in internal notes.
* **Open**: no comparison against the original under Wine at those
  sizes for the Off path beyond the traced sampler state (the original at
  1280 × 960 was matched before); HD UI Art has no native counterpart.
