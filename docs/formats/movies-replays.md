# Movies and replays

Part of [docs/formats.md](../formats.md), the traced behaviour of `SPEED2.EXE`; the tags
(**verified**, **inferred**, **open**) are explained there.

## Movies (src/movie.rs, crates/ob-formats/src/formats/vp6.rs; verified except where noted)

* **Where the career intro plays** (**verified**): the career start
  0x495A30 (after MC_Main's 0x461A18EE, and on Resume) goes to the crib
  when DDAY_EVENT_B is won (0x500B80(0xDD60E403)); else, while 0x86347C
  is 0, it calls 0x53D720("UI_StartCareer.fng"): game mode 0x838568 = 6,
  the queue 0x8382A0 (4 names × 0x40 bytes, index 0x83856C) cleared and
  "scene01" put in, 0x4A78E0 (pops GarageMain.fng when it is up),
  0x4C57C0 (loads PC_Loading.fng when it is not), IG_PlayMovie.fng pushed
  over the caller (0x5377B0; the test of 0x8384C4 is the FE manager made
  by the boot code at 0x5378A8, always set) and 0x52CDE0 (the joystick
  handlers). With 0x86347C set it starts D-Day at once (event A, or B
  once A is won; 0x5410F0, 0x5404B0). 0x86347C is written only by
  0x54AF90's mode 6 (0x54AFDA, after the city start); it is career
  +0x873C (0x85AD40 + 0x873C): a new career clears it (0x53292D in
  0x532840), the career save writes it (0x5145F3 in 0x514530) and the
  load puts it back (0x529307 in 0x529240), so a career whose movie has
  played resumes straight into the city (**verified** 2026-10-30; the
  earlier "session flag, never saved" reading missed the career-pointer
  accesses). UI_StartCareer tests it too (0x4AF165, see **Menus**).
  Clone: `Profile::intro_played` (native save byte `UNDER_WAY`), read by
  the career menu, `Game::career_intro` and `Game::explore` (the city
  start), set after mode 6's city start. **User bug fixed** (2026-10-30):
  the clone kept it per session and its career menu used "has a car" in
  its place, so after a restart a career saved during D-Day showed only
  the resuming "Start New Career"; that played the movie and the D-Day
  script again on the saved inbox, where SMS_INSTRUCTION was already
  read (0x5338A0 adds it once), so the screens opened on the mailbox
  (0x4976B0: not exactly one unread) and the car stayed held (state 5
  waits for +8, set only by reading an unread entry, 0x514DC0) if the
  player left without reading one. The original resumes such a save with
  the script over (0x495A30 → 0x5404B0 → 0x5404FE): **checked under
  Wine** (2026-10-31, wined3d, the player's own save: D-Day A, no car, 5
  unread): after the load 0x86347C = 1 and the script 0; UI_StartCareer
  shows Resume Career (focused), Start New Career, Load Game; Resume
  goes to the airport bay with no movie and no SMS screens, the
  free-roam HUD (Bank, minimap, the SMS envelope), the script 9 within
  10 s and still 9 three minutes on.
* **The other queuers** (**verified**, not ported): 0x53D640 (mode 1: an
  event record's movie name, +2 into the string pool 0x838428, then
  "DriftTut" after "Scene20"), 0x53D7B0 (mode 4: a name, then
  "UpperClass" after "Scene07"; 0x43B640), 0x557E00 (mode 2: a record's
  +0x20 name; IG_PlayMovie via 0x555D10 or 0x5377B0). After the queue,
  0x54AF90 runs per mode: 1 → 0x499B10, 2 → 0x499560, 4 → 0x5F41F0(0xB,
  0), 6 → D-Day as above and 0x86347C = 1.
* **IG_PlayMovie** (constructor 0x553950, vtable 0x79CB3C, handler
  0x55AFF0; **verified**): the constructor names the movie object
  0x6C6554EE after the current queue entry (0x4955C0 → 0x5379D0,
  0x50C9B0). The handler takes three messages: 0xC3960EB9 (the movie
  ended, or its file is missing — 0x566250 sends it when 0x6F909D fails)
  goes to the next name at once; Accept 0x406415E3 (the package's accept,
  the clone's Enter) and PAD_START 0xB5AF2461 (`,`) do the same only when
  the screen's timer +0x4C is above 0.5 (0x784260). The tick 0xC98356BA
  adds 0x865194 to the timer (**inferred** in the clone: it adds the
  frame's time), steps the subtitles (0x576470) and, once the queue is
  done (+0x50), in mode 6 with 0x8654A4 ≠ 3 switches to PC_Loading.fng
  (0x537990) and calls 0x54AF90. Back is not handled: Esc does nothing.
  The package: a black White16x16_noalpha image 863 × 676 at z 1000, the
  movie object 640 × 432 at the centre (z 100), and the "Subtitles" group
  (an empty string and two U2_Menu_Box images, shown or hidden by the
  subtitle code). The clone lays the package out with the FE renderer
  and draws the movie on the movie object's quad.
* **Drawing** (0x536860, **verified**): the movie's texture on the unit
  quad, u 0..1, v from height / 2^k − 0.001 to 0.001 (k rounds the
  texture height up) when the movie object's +0xC is set, which 0x566030
  sets for a name ending in '6' (".vp6"): VP6 frames are stored bottom
  up, so the quad flips them. The colour is the object's halved.
* **Subtitles** (**verified**): 0x553950 and the skip path start them
  (0x57DAB0: SUBTITLES\<name>, the "Subtitles_Text" string, the group's
  SHOW or HIDE script) only when 0x50D350 holds: region 0x570A40 = 5 or 2
  with 0x7F70D0 set, or 4, or 0xC. This EXE's region is the static 6
  (0x7FA73C; only read, by 0x570A40 / 0x570A50 / 0x5712B0 / 0x571390 /
  0x575FA2 / 0x57DBE0 / 0x57FC5E — no writer), so no subtitles are shown;
  the clone shows none. The same gate guards the third starter,
  0x4A8B96 (the PSA screen, "PSA", LS_PSAMovie). The step 0x576470 runs
  from IG_PlayMovie's tick (0x55B112) and 0x4C544F; with no subtitle
  started it has nothing to show (**inferred**: it was not decompiled).
  **The group's own state** (2026-10-30): in both packages the
  "Subtitles" group 0xD3F7189E (the line 0x599B8442 and two 900 × 300
  U2_Menu_Box backings 0x8BD49BCC) has a zero-length INIT chained to HIDE
  (0x16A259: colour (255, 255, 255, 0), y 242) — SHOW (0x1CA7C0) has
  alpha 255, y 222 — so with no subtitle code the group is hidden by its
  own chain; the clone lays the movie screens out with `chain_init` (it
  used to keep INIT's state and drew the empty dark box). 0x57DAB0 itself
  names the scripts "Show" / "Hide" (0x78FFD0 / 0x78FC44) on "Subtitles"
  (0x7A09EC), not the package's upper-case names (moot here). **The
  files** (SUBTITLES\<movie>, 0x57DAB0 → 0x57DA20, read whole): 8-byte
  records {u16 time, u16 (unused, junk in the files), u32 label hash},
  ended by time 0xFFFF (0x57DB67); e.g. Scene01 has 19 lines, the first at
  9; the label 0x001A20BA is the empty string. The time unit and the
  stepping are **not traced** (unused in this EXE).
  **Observed** (Wine, 2026-10-30): Start New Career went to the loading
  screen and the airport scene without showing Scene01 (the widescreen
  fix's SkipIntro is on in that install; whether it also skips Scene01 is
  **not checked**), so the movie screen itself was not compared. The
  original without the fix (frames every 3 s) also went from
  the Career menu to a dark CareerNis1 opening within 3 s with no movie
  frame seen (**observed**; a movie shorter than 3 s is not ruled out).
* **File name** (0x51C740, **verified**, traced and seen under Wine):
  "%sMOVIES\%s%s" with ".vp6" for FMVOpening, IndustEast, IndustWest,
  Suburban, UpperClass, THX_logo, THX_logo_fr and blank, else "_fr.vp6"
  (ealogo too). Nothing turns "_fr" into the installed language: 0x50C4A0
  (called at the end of 0x51C740) fopens the name and, when that fails and
  the CD drive letter 0x80057F is set, the same name on that drive (none
  seen under Wine); 0x566250 then asks the file layer for the one name
  (0x6F909D → 0x6F87EA; under Wine one more open under the Install Dir)
  and skips the movie when it is missing. The English data has
  <name>_en.vp6 where this builds <name>_fr.vp6, so the original plays
  none of those (Wine: ealogo, THX_logo_fr, PSA, scene01). The clone looks
  up the built name first, then the installed language's file, without
  case (`movie::path`; "Boot sequence"; docs/playing/native-deviations.md "Movie
  files").
* **Container** (0x700AFF, **verified**): chunks of a 4-byte tag and a
  little-endian size counting the 8-byte head. MVhd: +0x0C width, +0x0E
  height (u16), +0x10 frames, +0x14 largest frame, +0x18 rate, +0x1C its
  divisor (read as a u16); fps = rate / divisor in f32 (Scene01: 512 ×
  512, 4545 frames, 819175 / 32767 = 25). MV0K (key) and MV0F frames
  carry the VP6 frame directly (+8, size − 8 to 0x702E47). More than 16
  frames behind, the player drops chunks up to the next MV0K. The tags
  are compared big-endian (0x7013A8: 0x4D566864 MVhd, 0x4D563046 MV0F,
  0x4D56304B MV0K). The audio chunks go to the stream code: SCHl
  (0x72250B), SCDl (0x7228AF), SCCl / SCEl.
* **Audio** (**verified**): SCHl starting "GSTR" (0x72ABB0): tags from +8
  (0x72AAF8: tag, length, big-endian value; 0xFC padding, 0xFD / 0xFE no
  length, 0xFF the end, length 0xFF a 4-byte length), defaults channels
  1 (0x82), 48000 Hz (0x84), 0 samples (0x85), codec 10 (0xA0); the
  version is forced to 2 and the flags to 4, and the data is big-endian
  (+0x25). "PT" headers (0x727A46) default to 22050 Hz. SCDl: a sample
  count, an offset per channel counted from the end of the offsets, then
  the channels' data. Codec 10 is EA-XA revision 2, decoded in floats by
  0x734AFF (SSE; 0x734BF4 is the x87 twin): per 28 samples a byte with
  the filter (high nibble; coefficients 0x819668 {0, 0.9375, 1.796875,
  1.53125} and 0x819678 {0, 0, −0.8125, −0.859375}) and the shift (low
  nibble; nibble values 0x819688 = signed nibble × 2^(12 − shift), zero
  for shifts 13..15), then 14 bytes, high nibble first: s = h1·c1 +
  nibble + h2·c2. A first byte 0xEE (0x734A81) instead gives the two
  history samples and 28 samples as big-endian words. The history carries
  from block to block. Scene01: GSTR, 2 channels, 32000 Hz, 5720277
  samples, 4469 blocks.
* **Colour** (0x7004F7 / 0x6FFD4A, **verified**; which converter the
  movie texture uses among 0x6FFD4A's siblings is **inferred**): per
  value v the tables Y (v, v, v, 255), U (round(1.772 (v − 128)),
  round(−0.3441 (v − 128)), 0, 0), V (0, round(−0.7141 (v − 128)),
  round(1.402 (v − 128)), 0) in B, G, R, A order, rounded half away from
  zero (0x7004B9); a pixel is Y + U + V saturated to 0..255; one chroma
  sample covers 2 × 2 pixels. No post-processing: 0x700AFF sets level 0
  (0x702593(…, 0, 0)), so 0x70288E hands out the decoded frame.
* **VP6** (the codec behind 0x702C9D, C paths; **verified** as ported,
  tables at their addresses in crates/ob-formats/src/formats/vp6.rs). Header (0x707CF8,
  plain bits): inter flag, quantizer (6), separated flag; a key frame adds
  version (5; above 7 refused), profile (2), interlace (1, not ported),
  then a 16-bit coefficient offset when separated or in profile 0, the
  range coder at +2 or +4 (inter: +1 or +3), and from the coder rows,
  columns, display rows and columns (8 bits each) and 2 bits; profile ≠ 0
  adds the filter fields. Inter frames read the golden flag (and in
  profile ≠ 0 the deblocking bits). Last the Huffman flag. The range
  decoder (0x707BCA, 0x70D801 / 0x70D9E8): split = 1 + ((range − 1) × p
  >> 8), renormalised below 0x80; the even bit (0x70D87A, and the sign
  0x706A61) shifts exactly once, so its range can reach 256. Models
  (0x7077B2): key frames reset the vector models (immediates) and the
  macroblock statistics (0x7C6668); inter frames update the statistics
  (0x70F5AC: presets 0x7C62A8, deltas 0x70F4EA) and derive the type
  probabilities (0x70F32B), then the vector models (0x70F1F3); then the
  coefficient models (0x706F35; DC 0x7C5FC0, reorder 0x7C5FD8, runs
  0x7C6018 / 0x7C6034, AC 0x7C6050 in [ct][plane][group] order; key
  frames give unsent nodes the last value read) and the DC context
  probabilities (0x70F16E, 0x7C61E0). Tokens (0x707319, range coded; or
  0x70E9B0 with Huffman trees built from the same models by 0x706D62:
  0x706B27 / 0x706C67 frequencies, 0x70EE7C trees, 0x70EFBE 6-bit
  lookups, a big-endian bit reader, and runs of skipped DCs / first ACs
  per plane). Scene01 is Huffman coded. Dequantisation 0x70394D: DC
  0x7C4C18 × 4, AC 0x7C4618 × 4 (the chroma tables are equal). DC
  prediction 0x70E92E (left, above, else the plane's last DC per
  reference). The inverse transform 0x71DA07 (rows truncated to 16 bits,
  columns (x + 8) >> 4), or 0x71E136 ((c·q + 15) >> 5) when the
  end-of-block class (+0x5FC[index − 1], +1 from version 7) is below 2.
  Macroblock types 0x70DB59 with the vector candidates 0x70DE83 (12
  neighbours 0x7C5528), vectors 0x70DC4C, four-vector chroma averaged
  ((sum + 2) >> 2, negative (sum + 1) >> 2). Prediction 0x70DF7C:
  quarter-pel luma, eighth-pel chroma, the simple profile's bilinear
  filter (0x816708) in 1-D or 2-D (9 rows across, then down), each
  (a·t0 + b·t1 + 64) >> 7. After a frame the planes' 48 / 24-pixel
  borders are filled from the edges (0x704D87); a key frame or the golden
  flag copies the frame to the golden one.
  **Open** (refused by the clone): interlaced frames and profile ≠ 0
  with its bicubic and deblocking filters (0x704078, 0x70423C, 0x71FDDF);
  the film-grain pass (0x7033D4) is off (level 0). All 32 shipped movies
  are version 7, profile 0, progressive, separated (checked on every key
  frame).
* **Clone**: a new career (and Resume before D-Day B) plays Scene01 on
  IG_PlayMovie until it ends or Enter / `,` skip it after 0.5 s, then
  starts D-Day as before; the soundtrack streams to the mixer (stereo,
  resampled). Video follows the frame clock (**inferred**: the game's
  A/V sync is not traced); the movie ends when its last frame's time has
  passed.

## Replays (src/bin/game.rs; verified except where noted)

**The replay manager** (0x8650B4, 0x50EC bytes, constructor 0x57A870;
per-frame 0x581060 from the frame loop at 0x581398):

* A list of named snapshots (+0x98; 0x578FE0 finds the n-th of a name,
  0x579030 the n-th of all, 0x579060 frees one to +0x90). A snapshot is a
  serialisation of the game's objects (0x578CF0 opens a named section —
  "ReplayManager", "RaceParameters", … — and 0x571650 / 0x571780 /
  0x5716D0 write or read each field; 0x57C090 takes one, 0x57C1E0 restores
  one and resets the race cameras, 0x5FAF90 per player). Its +0x28 is the
  sim tick (0x865188, 4000 Hz) it was taken at, +0x34 the loop counter
  0x7FB724. Names: REPLAY_RACE_SNAPSHOT (the race's start; 0x5791A0
  restores its RaceParameters, 0x5ED6C0), REPLAY_MANAGER_STATE,
  LAP_SNAPSHOT, INSTANT_REPLAY / HIGHLIGHT_SNAPSHOT (0x5806C0, below),
  THE_PRESENT,
  DEMO_MODE_SNAPSHOT, DUMMY_SNAPSHOT (0x57C190: every 100 loops while
  recording or playing, a 16-byte hash of the state into the list at
  +0x40EC — up to 256, each at its loop number, 0x57AD90 — compared on
  playback: a desync check).
* Recording starts (0x57AAE0, once: +0x78 = −1) outside the front end
  and game states 2 / 7, with at least 1 MB free (0x43E910 − 0x400 ≥
  0xF8F40): the loop counter range +0x78 / +0x7C and the tick range +0x84
  / +0x88 both start now, 2 instant replays (+0xA8), 5 LAP_SNAPSHOTs
  (+0xB0), a 256 KB input buffer (+0x8C, 0x575030) and 8 snapshot buffers
  (0x579090 × +0xA4).
* INSTANT_REPLAY (0x5806C0, each recorded frame just before the
  LAP_SNAPSHOT's 0x57D070: 0x5810B9, 0x5810C7; **verified**): once
  0x7FB720 — whole 60ths of a second, 0x57A0F0 adding (carry 0x865154 +
  dt) × 60 through 0x75BED4 and carrying the rest — reaches +0x74 (0 in a
  new manager, 0x57A8F2; the race loader 0x60A690 builds one per race),
  +0x74 moves on 300 (5 s); with +0xB8 ≥ +0xA8 (2) held, the oldest
  INSTANT_REPLAY is dropped or turned into a HIGHLIGHT_SNAPSHOT (renamed,
  +0x2C / +0x34 the tick and loop now), then under 2 a new one is taken
  (0x57C090, appended to the list). It becomes a highlight only when its
  score +0x1D8 is above 0, the highlight limit +0xB4 is at least 1 and it
  beats the lowest-scored highlight held (+0xC0 of them; at the limit the
  lowest goes, else one taken after it, 0x579060). The score: 0x578EA0
  (reason, amount) adds the amount to every held INSTANT_REPLAY while
  recording; its callers name EBrake (0x407C36), Invalidate (−99,
  0x578EF0), CrossFinishLine (× 7.5, 0x5EA5DF), LosePosition / ClosePass
  (0x5EEE3E / 0x5EEE6B), FlattenFern, InDeepWater, InShallowWater (1.25),
  CarInAir (2.0), GoingSlow, CloseCars (× 0.5), Speed, CarSlidingSideways
  (1.0), CarCollision and SmokeableCollision (0x5F56B2..0x6051E0). But
  +0xB4 is written only as zero in the replay-manager routines
  (0x57xxxx-0x58xxxx; **verified**: that range holds three +0xB4 stores, all
  zero): the constructor 0x57A942 stores edx, zero from `xor edx,edx` at
  0x57A88A, and 0x57AB76 / 0x57AC35 in 0x57AAE0 store ebx, zeroed at 0x57AAF3
  and not otherwise written before those stores. The decompiled test in
  0x5806C0 keeps a highlight only when `score > 0`, `+0xB4 >= 1` and it beats
  the lowest one held (**verified**; the Ghidra decompile agrees). With +0xB4
  at 0, the `jle` at 0x580782 goes to 0x580825, the drop, which takes
  INSTANT_REPLAY index 0 (`FUN_00578FE0("INSTANT_REPLAY", 0)`, the first match
  in list order from the head at [this+0x98]; whether that is the oldest is
  not verified) and skips the keep (**verified**). The only code that names HIGHLIGHT_SNAPSHOT (string
  0x7A0984) is inside 0x5806C0 (pushes at 0x58071D and 0x5807EA). That routine
  has one direct caller, the call at 0x5810B9 in 0x581060 (**verified**).
  0x581060 has one direct caller, the call at 0x581398 in 0x5811D0, which loads
  the manager into ECX from 0x8650B4 at 0x581392 (**verified**). The manager is built by
  the constructor 0x57A870, called only at 0x60A6D4, and the eax that call
  leaves is stored to 0x8650B4 at 0x60A6E4 (**verified**). Its +0xB4 is written
  only as zero. The constructor stores edx into +0xB4 at 0x57A942, and edx is 0
  from `xor edx,edx` at 0x57A88A with no call in between (**verified**).
  0x57AB76 and 0x57AC35 in 0x57AAE0 store ebx, which is 0 from `xor ebx,ebx` at
  0x57AAF3, and no branch from outside the routine reaches either store
  (**verified**); that ebx survives the calls 0x5F0100 and 0x43E910 in between
  rests on the x86 convention, not on those callees. The only other store to
  +0xB4 in 0x57xxxx-0x58xxxx is a stack slot at 0x572D8E. 0x471F0A (eax = 0
  from 0x471ED8) and 0x471F6F (ebx = 0 from 0x471F3C, same convention) zero
  +0xB4 of a different object, not the manager: its constructor 0x471ED0 has
  one reference (`jmp` at 0x47C838, in factory 0x47C800), and its teardown
  helper 0x471F30 has one (`call` at 0x47C86B, in 0x47C840) (**verified**).
  **Verified**: no other store reaches the manager's +0xB4. Of the 88
  instructions that name 0x8650B4, two store it (the constructor result at
  0x60A6E4 and ebx at 0x60AA5A) and 86 load it; none of the loads is followed
  by a +0xB4 store before the register is reloaded, the manager is copied
  nowhere (the one candidate, 0x5EB0A5, stores the return of 0x5D9C00), and
  the only call it is passed to as an argument is 0x578EA0 (0x6050E4). The
  calls made with it in ECX go to replay-manager routines (0x57xxxx-0x58xxxx,
  whose +0xB4 stores are the zeros above) or to 0x4A4690, 0x4A50E0, 0x5F41F0
  and 0x601A70, which write no +0xB4 (REA decompile). So the PC game always
  drops INSTANT_REPLAY index 0 and takes no HIGHLIGHT_SNAPSHOT (that index 0
  is the oldest is not verified). **Open**:
  what the drop path removes. 0x578EA0 has seven callers (0x407A30,
  0x578EF0, 0x5EA540, 0x5EED20, 0x5F5590, 0x5F5740, 0x604750). The clone keeps
  the two (`Recording::instant_replay`, its 60 Hz counter `tick`) without
  scores. 0x75BED4 is the CRT's float-to-integer
  conversion (`_ftol2`: `fistp`, then the rounding undone toward zero),
  so it truncates (**verified**); **inferred**: none on a frame with a
  shift or reset (as the LAP_SNAPSHOT).
* LAP_SNAPSHOT (0x57D070, each recorded frame): when the time since the
  start (+0x84) reaches count (+0xBC) × interval (+0xC4, first 15 s) one is
  taken; with 5 held the interval doubles and every second one (the 2nd,
  4th) goes, so the kept ones stay on the new interval.
* Each frame (0x581060): playing (+0) while the loop counter is within
  +0x78..+0x7C; at +0x7C, not paused, it records again (+4). Paused
  (+0x14) holds the frame; slow motion (+0x18) holds three render frames of
  four (0x865148 & 3); a held frame (+0x1C) skips the simulation
  (0x5813A3). Playback reads the input buffer (0x580BAA) and replays the
  manager's events at their loop numbers (0x5718C0 → 0x4A7B30).
* The joylog (debug capture CaptureJoylog.jlg / ReplayJoylog.jlg,
  0x57D6F0, 0x4128-byte buffers 0x572FE0): channels (table 0x7FB43C,
  {id, name, flags}) 0 NONE, 1 CONFIG, 2 JOYEVENTS, 3 TIMESTEP (0x10), 4
  CHECKSUM, 5 LOOP_COUNTER, 6 QUEUEDFILE_STATUS (0x10), 7
  MOVIE_PLAYER_STATUS, 8 MEMORY_CARD, 9 TRACK_STREAMER (0x10), 10 RANDOM,
  11 REPLAY_SNAPSHOT_INFO (0x100), 12 REPLAY_SNAPSHOT_DATA (0x100), 13
  PRINTF. A record is a byte (length << 4 | channel) and that many bytes,
  least significant first (0x5730A0). 0x580890 rebuilds a loaded
  joylog's two snapshots (12-byte INFO records, then DATA) as
  REPLAY_RACE_SNAPSHOT and the manager's state. Joy events 0x83–0x8E
  (table 0x7FAF70) are the replay's: SKIP, PAUSE, PREVIOUS / NEXT_MARKER,
  PLAYBACK_SPEED, CAMERA_ANGLE, CAMERA_TARGET, HELP, HIDE_TIMELINE and
  the MARK / TEST / ERASE_DEMO ones.

**The control bar** (UI_ReplayControl, constructor 0x4C0AA0, tick
0x4C10B0):

* Markers: over the snapshot list in order (the order taken: 0x57C090
  appends at +0x9C), the first INSTANT_REPLAY and
  every LAP_SNAPSHOT except, once there is a marker, those under 20 s
  before the end (+0x88); each {time +0x28 / 4000, snapshot}. REPLAY_TIME_n
  goes at the backing's left + f × its width less half its own width, at
  the backing's height. The bar spans +0x84..+0x88.
* Entering takes THE_PRESENT (+0xD0 → 0x57C090) and skips to the last
  marker (the first when the race camera state's +0x40 is set).
* A skip (0x4A4BF0) pauses, notes the time (+0xD4, real ticks 0x8651A8)
  and the snapshot (+0xCC); the tick starts the streamer at the snapshot's
  position (+0x38, 0x5E7B00) and once 0.5 s (0x78E000) have passed and it
  is done (0x5E7A50) restores it (0x57C1E0), plays (+0x14 = 0) and shows
  the bar with RCSTATE 5. Skip back (0x4A4CA0): the last marker more than
  1 s before now (the clock 0x7FB718, or a waiting skip's time), else the
  first; forward (0x4A4DD0): the first after now, else nothing; both end
  slow motion. Back (0x4C1220) skips to THE_PRESENT and then leaves
  (0x4A50E0). Play / pause (0x4A4E80) only with loops left (+0x7C over
  0x7FB724); slow motion (0x4A4FB0) not while paused.
* The camera (0x5FAFE0 on the race camera director 0x8900B4): its index
  +0x21C steps through six: 0 automatic, 1–5 the chase views 0x802130[i]
  (2, 4, 1, 0, 5); leaving 0 skips index 1 when view 2 is showing. The
  director (0x5FB060) in RCSTATE 5, not paused, on index 0: track cameras
  first (mover 0x12 from 0x44FE80, see Track cameras), else a chase view drawn
  (0x43C1F0 × 5 over the weights 1 at 0x7A553C) when the index was just
  set back to 0 (+0x218, then not the view showing unless the draw runs
  past the last weight) or, after 4 s on a view (0x5EA160: real ticks since
  +0x214), each frame with chance 0.3 with the car at ≥ 10 m/s (+0x430);
  between 1.5 s (0x802148) and 4 s it holds.

**The clone**: at the first frame after the green light it snapshots what
a race frame changes (the player's CarState and chase camera; each
rival's CarState, progress, RacerRoute, SpeedTarget, AiDriver, last
AiControls, GameRng and Unstuck; the CarField pair records and world rng;
the knock-over props; the clock, finish hold, wrong-way flag and drift
managers), takes the LAP_SNAPSHOTs as the game does (by the frames' dt
summed) and the INSTANT_REPLAYs (the bar's first INSTANT_REPLAY marker
included), and records each frame's Controls and dt plus the shift and reset
keys. Replay builds the markers and skips to one; frames play at their
own dt, skips, Back, play / pause, slow motion and the camera director
follow the game; a restore re-poses the props. Keys, from the FE keyboard
map (below): Esc (Back), Enter / Num Enter (play / pause), C (camera), `,`
(slow motion), `[` / `]` (skips).
**Two entries** (2026-10-31, **verified** in 0x4C0AA0 / 0x4C10B0 /
0x4C1220 / 0x4A50E0 / 0x603E30 and **observed** under Wine, Quick Race
circuit 4014 and sprint 4123, `wine/compare/replay`): the control's
choices key on the game flow's +0x40 (0x430560 on 0x8900D8), which the
race's end handler 0x60AAB0 sets (0x60AE55, an object from 0x4F4190) for
every race type but 7 / 8 before it enters state 7 (post race), and which
0x60AA77 clears.
* From the **post-race menu** (+0x40 set): no RCSTATE 5 at entry and no
  THE_PRESENT (+0xD0 = 0); the skip goes to the **first** marker, the green
  light (observed: the bar at 0.12–0.69 s and the countdown's "GO!" on
  screen). Back (0x4C1220): no THE_PRESENT, so with no skip waiting it
  leaves at once (0x4A50E0: message 0xC, which state 7 ignores, manager
  +0x14 = 1, then 0x537990 back to the screen below): the Post Race menu
  comes back still on Replay, the world where the replay stood.
* From the **race's pause** (UI_pause's Replay, 0x4C2BD0 → class 0x4C2640;
  +0x40 clear): RCSTATE 5, THE_PRESENT, the **last** marker (observed ~5 s
  back, the INSTANT_REPLAY). Back skips to THE_PRESENT and then 0x4A50E0
  (pops UI_ReplayControl, 0x5F43E0(6), +0x14 = 0): a new UI_pause on
  "Resume Race" (observed). State 5's message 0xC (0x603E6C) goes to state
  7 with +0x40 set, else 6.
* Every skip shows PC_Loading (0x4A4BF0 → 0x5F43C0(3), observed ~0.5–1 s);
  the clone holds the frame instead (the loading screen's 200 ms rule).
* **The end**: the recording stops for good at the race's end (0x60AB6E →
  0x571840: manager +0x80 = 1), and at the last recorded loop (+0x7C) the
  replay holds, paused (REPLAY_PLAY showing, the bar full), with the bar
  up until Back or a skip (observed both ways; play / pause needs loops
  left, 0x4A4E80). **Verified** (round 2): the store is the manager's
  end of frame **0x578E20**, called after the simulation (0x5813CA,
  0x581426): it clears +0x10; while recording (+4) it sets +0x7C = loop +
  1 and +0x88 = the tick + _ftol(0x865150 × k + c) and clears +4; while
  playing (+0) it clears +0 and the held frame +0x1C and, when the loop
  counter is +0x7C − 1 (the last recorded loop just played), sets paused
  **+0x14 = 1 (0x578E8C)** and sends 0x7C8D5450 (0x55DDF0), which
  UI_ReplayControl's handler takes at 0x4D7C83 → 0x4D7CD1 (SHOW to
  0x47FF4E7C, Hide when the bar is hidden and +0x75 clear, then +0x75 =
  0): the bar comes back. 0x581060 then sees the loop at +0x7C with
  +0x14 set and keeps the replay playing, held. The clone pauses at its
  last frame as before.
* The director starts on the view showing (the player's), held until a
  track camera or a draw (observed: the far chase view, then route
  cameras); its time on the view (+0x214) restarts at each skip.
* **The bar's hide and Help take no PC key** (observed: P, H and every
  other key, arrows, keypad and F-keys, leave the control's +0x74 at 0
  and the bar up). So FENG_BUTTON0 (P, row 5) does not reach 0xC519BFBF
  on PC; the clone binds neither (it bound P before).
* **Audio** (code only; Wine runs silent): the replay drives the same
  sound objects as the race, from the replayed cars. One replay-specific
  gate is **verified**: SFXCTL_MasterVol's update 0x471970 (vtable slot
  at 0x78A0A0) sets its +0x1C (field f6) at 0x471BD1..0x471C1B to 1 when
  the audio system's +0x13 is set, the race camera state (0x8900D8) is 1,
  state 7 while 0x435950 on the NIS manager holds, or state 5 with the
  manager paused (+0x14, also set by every skip, 0x4A4C8C), else 0.
  **Verified** (round 2, the mix map data, all four maps alike): id
  0x60000006 (section 0, controller class 0, field 6) is the trigger of
  section 0's event control 0 {mode 2, V = 0xD8F0 = −10000 mB, attack
  0x011 (17 × 16.67 ms, curve 0), hold 0, release 0x8011 (17 × 16.67 ms,
  curve 8)}, and that event is an input of sub channel 0 of sections 1–8
  (the player car, the AI racers, the traffic, the world objects) and of
  no section-0 channel: while the flag holds the world's sounds fade to
  −100 dB over ~0.28 s and come back over ~0.28 s after it drops; the
  music, speech and front-end sounds (section 0) are untouched. Field f7
  (+0x20: race state 0x89E7B0 = 8 with 0x890118 in state 4 and no NIS
  scene) triggers events 4 (−240 mB) and 12 (+300). **Ported**:
  `mixmap::Scene::sound_hold` (a replay paused or a skip waiting).
  **Slow motion**: no audio code reads the manager's slow-motion flag
  (+0x18): of the loads of 0x8650B4 followed by a +0x14 / +0x18 / +0x1C
  access, the only one in the sound code is the +0x14 above (**verified**);
  the sounds follow the replayed cars, which move on one render frame in
  four, so the engine pitch steps with the rpm and no rate or pitch
  change is applied (the clone does the same). **Open**: which race
  speech and HUD-driven sounds re-fire in a replay; the EA Trax music runs
  on (section 0); its chyron in a replay is not traced.
* The input buffer (256 KB) can fill; a desync (checksum) leaves the
  replay as Back does (observed: after memory teleports in the race, the
  replay left at the teleport's loop). Neither happens in the clone.
**Inferred**: recording inputs and dt rather than the joylog channels; the
start at the green light (the game starts at 0x57AAE0 from 0x60B527 when
race status 4 is first seen, outside game modes 8 / 0x200); no
LAP_SNAPSHOT on a frame carrying a shift or reset (the key acted before
it); slow motion's frame phase; the camera draws on the game's shared
random state (outside the race simulation in the clone); the 0.5 s wait
without the streamer. **"GO!"** (**verified**, round 2; ported): 0x6000A0
in RCSTATE 5 gives the low features 0 (0x600186 → 0x60018B → 0x60052A)
but its tail 0x60052C still adds the high 0x20000, the countdown element
0x4F7340, while the state is not 7 or 6, a race is on (0x89E7B0 or
0x457BA0) and the player's +0x234 is 0; 0x4AB5B0 then asks 0x5EEA50,
which returns 4 ("GO!") while (now − race status +4) / 4000 ≤ +0xC
(0.9 s) — the restored green tick — whatever +8 (the 3-2-1, which a start
scene leaves 0). So a replay at its green light shows "GO!" for 0.9 s
(observed: circuit 4014, drag and drift, "GO!" at 0.70–0.78 s, gone by
1.06 s). The clone draws the HUD's countdown alone under the bar
(`hud::Input::replay`). **Open**: whether a live race after a start scene
shows "GO!" (0x5EEA50 says it should when 0x5FC910(4) sets +4; the clone
shows none there). A snapshot now
also holds each rival's physics hold (+0x734), the human's line mean and
last frame's velocities, without which a replay from the green light
drifted. The
simulation is deterministic for a given frame sequence, so the replay ends
bit-for-bit where the race did, from the start or a LAP_SNAPSHOT (test
`replay_reproduces_the_race`).

**Drift and drag replays** (round 2, observed under Wine, Quick Race,
`wine/compare/replay` 14–16; **verified** in
0x4C0AA0): the same bar, keys, markers and camera director. In a drag
race (0x89E7D8 or 0x890108) the constructor sends HIDE to 0x788C05E8 (the
"Lap" label), LapNumber (0x57F0E3A5) and NumberLaps (0xF2685238): the bar
shows the time alone (ported: `ReplayBar::drag`). A drift replay keeps
"Lap 1/3". Neither shows its race HUD (score, tach, drift panel); both
show "GO!" at the green light. The drag replay stayed on the chase view
(the car stood at the line); the drift replay used track shots of the
car.

**Verified** (UI_postrace 0x4BEE10, UI_ReplayControl class vtable
0x79055C, handler 0x4D7BD0): the post-race menu lists "Replay" (icon
0x84D1D864, 0x7840428A) after Race Stats outside game mode 4 when the
replay manager (0x8650B4) holds a recording (+0x50EC set, +0xBC > 0); the
clone offers it with a recording. Its action (0x4BED90) pushes
UI_ReplayControl.fng over the race, which sets the race camera state
(0x8900D8) to RCSTATE_INSTANT_REPLAY (5; 0x5F4250 at 0x4C0EFA); the HUD's
features (0x6000A0) are none while that state is not
RCSTATE_RACE_IN_PROGRESS (4), so the HUD is hidden. Each tick
(0xC98356BA): f = (time − start) / (end − start); REPLAY_CURRENT's centre
at ScrollBar_Backing's left + f × its width, ScrollBar_fill f × that
width from the left; LapNumber "%2d" the player's lap + 1 (at most the
laps), NumberLaps "/%2d", CurrentLapTime the time since the start
(0x577250). Keys: Accept (0x0C407210 → 0x4A4E80) plays / pauses
(REPLAY_PAUSE shows while playing, REPLAY_PLAY while paused), Back
(0x911AB364 → 0x4C1220) leaves, BUTTON1 (0xC519BFC0 → 0x4A5090) blinks
REPLAY_MOVE_CAMERA and changes the camera (0x5FAFE0); 0xB5AF2461 toggles
slow motion (0x4A4FB0, replay manager +0x18), 0x5073EF13 / 0xD9FEEC59
skip back / forward (0x4A4CA0 / 0x4A4DD0) and the action 0xC519BFBF (bound to P in the base notes, not traced; its name
BUTTON0 is **unverified**: 0x43DB50 hashes "BUTTON0" to 0xB8A7C6CB) hides the
bar (compare 0x4D7D3E, je 0x4D800D → Hide 0x4A5200, **verified**). 0xD97360E9 is
compared only at 0x4D7FC2 (its one occurrence in the file, **verified**).
Hide (0x4A5200) toggles +0x74 (**verified**): from 0 it sends HIDE and sets 1, from
1 it sends SHOW and sets 0, to the object at [esi+4] (class 0x47FF4E7C) through
0x537C10 with the strings "HIDE" (0x79053C) or "SHOW" (0x78F614). The H path
(0x4D7FDF) saves +0x74 in +0x75, sends HIDE by hash (0x43DB50 gives "HIDE" 0x16A259)
through 0x537C70, then calls Hide at 0x4D800F only while the bar is visible, and
Hide sends HIDE again by name (**verified** call sites; the receivers are not
traced). H leaves +0x74 set and never clears it (**verified**), the same flag P
toggles. The restore path at 0x4D7CD1 is entered for eax 0x7C8D5450 (je at
0x4D7C88) or 0xB4623F67 (its jne at 0x4D7CCB is not taken): it sends SHOW by hash
(0x43DB50 gives "SHOW" 0x1CA7C0), calls Hide when the bar is hidden and +0x75 is
clear, then clears +0x75 (**verified**). Back (0x4C1220) tail-jumps to Hide at
0x4C126F while visible (**verified**). The string DIACTION is absent from the file,
and none of its 6015 NUL-terminated identifiers hashes to 0xD97360E9 under 0x43DB50
(**verified**), so H → 0xD97360E9 is **inferred** (the Help key has no FE row). The
clone's replay keys bind neither H nor P (observed under Wine: no PC key reaches
either; see the two entries above); the restore is not modelled.
**Input buffer** (**verified**; the clone records none of it, it replays
from its own snapshots):
* Allocated (0x57AB9C) only when free memory less 0x400 is at least
  0xF8F40 bytes (else the manager's replay fields stay 0): 0x40000 bytes,
  set up by 0x575030(size, 5 channels, 0): +0 magic 0x12345678, +8 the
  write position in bits (starts after the 0x30-byte header and 5 ×
  0x24-byte channel slots, 0xE4 × 8), +0xC 0xE4, +0x10 the read position
  in bits, +0x14 the size, +0x18 the channel count, +0x20 writes and
  +0x24 reads, +0x28 the desync flag. Each channel slot (+0x30 + 0x24 ×
  channel): +0 the run-count width in bits, +4 the value read when out of
  step, +8 the last width, +0xC the run count, +0x10 the run counter's bit
  position, +0x14 the last value, +0x18..+0x20 the reader's run, count
  position and value. Widths / defaults (0x57ABBA): channel 0 8 / 0,
  1 4 / 1, 2 0 / 0, 3 8 / 0, 4 8 / 0.
* Write (0x575150, through the manager's 0x579140 while recording, +4):
  the same value at the same width on the channel adds one to the open
  run (rewriting its counter) until the count is 2^width − 1; else a new
  record — the channel number in 4 bits, a run count of 0 in the
  channel's width, the value in its width. Writes stop 0x44 bits short of
  the end; then the manager stops recording and sets +0x80.
* Read (0x575280, through 0x5790F0 while playing, +0, until +8): within
  the open run the run's value; else, while reads are behind writes, the
  next record's 4-bit channel, which must be this one; otherwise the
  channel's default and the desync flag, which the manager copies to +8
  (playback stops reading).
* Channels a race writes (each from its own caller in the frame): 1,
  8 bits, the frame's tick count (0x86513C; 0x5EA370, × 1/1800 s by
  0x576CF0); 4, 16 bits, the input
  event id of 0x5D61A0(0) (0x5E6C20); 2, 8 bits, a checksum byte
  (0x5FA740: the sum of three state bytes). Channels 0 and 3 have no
  writer. A playback checksum that differs (0x5FA7F9) sets the manager's
  +0xC, sends 0x5F41F0(12, 0) on 0x8900D8 and leaves the replay
  (0x4D8020 → 0x4C1220, as Back).
* Saving (0x57D400): file "REPLAY\%s" with the one name at 0x7FAB40,
  REPLAY0.BIN: chunk 0x39100 (0x50 bytes: 8, the track 0x89E7A0, 1, 0,
  then a 0x3F-character name), then per snapshot pool (0x57D200:
  DEMO_MODE_SNAPSHOT, REPLAY_MANAGER_STATE, CHECKSUM0..3) a chunk 0x39101
  per snapshot, then 0x39102 with the input buffer (its bits rounded up to
  4 bytes). The save runs from 0x57E1E6, inside 0x57E110 (the demo
  object's per-frame update, called from 0x581060 at 0x581127 with ECX =
  0x865564), when the demo object's +0x1C is set; 0x57E110 clears the flag
  after it. The demo object has nine code references (reads at 0x57A97F and
  0x58106D; calls with ECX = 0x865564 at 0x579355, 0x579365, 0x581121,
  0x58172F, 0x581BF0 and 0x77D940; and at 0x782230 a `mov ecx,0x865564`
  then a `jmp`, not a call). The stores to +0x1C traced in the demo object's
  routines are zeros, in 0x57AFB0 (the static initialiser, called from
  0x77D940) and 0x57E110. **Open**: whether the PC build ever saves a replay.
  0x57D5C0 stores its first argument into this+0x1C (0x57D61F,
  `mov [esi+0x1c],ebx`; the store is skipped only when both +0x1C and +0x14
  already hold the new values), and its 12 direct callers (0x57D680,
  0x57D6B0, 0x57E490, 0x57E4E0, 0x57E5C0, 0x57E810, 0x57EAE0, 0x57ECC0,
  0x57EE10, 0x5809D0, 0x580DA0, 0x5815D0) were not traced up to the demo
  object. 0x57D400 has no caller but 0x57E110. The name table at 0x7FAB40
  has one name, entry 0 = "REPLAY0.BIN" (0x79FEAC; entry 1 is NULL), and
  0x571980 clamps every slot to 0, so the file is always
  `REPLAY\REPLAY0.BIN`; the pool writer 0x57D200 has no caller but 0x57D400.
**2026-11-05: the automatic replay camera's director gate, the reset flag, the typed groups (NOS, Jump, Speed, Corner, Burnout, PowerSlide: the replay's main shots under Wine) and their weights are in [replay-cameras.md](../playing/replay-cameras.md); the type table 0x7EC1F8 is filled at runtime (observed), so "only the first filled" below describes the static image only.**
**Track cameras** (**verified**; parsed by
`crates/ob-formats/src/formats/camera_tracks.rs`, test `camera_tracks_parse`; driven by
`src/track_camera/` for the route cameras, test
`track_director_follows_a_route_camera`; **open**: the rest of the
selection, listed after the motion below). The director calls 0x44FE80 on the camera manager 0x828770
(0x5FB465), which takes a camera track either fixed (+0x1C34 ≥ 1: +0x1C5C,
at fraction +0x1C64) or that of the scripted scene 0x4324D0(1) (its
fraction +0x2C / +0x30, the track named "Track %d" of 0x827B14 in the
generic group 0x442980 keys, 0x447C00), else draws one (0x44F9A0: the
player's road section's group first, 0x4429C0, its tracks whose entry at
the car's section fraction (0x5D4550 of car +0x8FC) lasts ≥ 1.5 s
(0x44F7B0: to the end of the run of entries that chain, over the car's
speed) and passes the visibility test 0x448280, one at random; otherwise
over the typed groups, each weighted by its type's weight fn in the type
table 0x7EC1F8 — 0x14-byte {—, type (matched at +4), name, weight fn,
flag fn}, only the first ("NOS", 0x441AF0: 0.5 + 0.25 × 0x5925C0 while
nitrous is on, else 0; flag fn 0x404800) filled in the file — ≤ 0.1 is
none, ≥ 10 skips the visibility test, the type's weight shared among its
tracks, the last three drawn (the ring 0x828774, counter 0x828770) left
out, drawn with 0x43C1F0; the flag fn's result 0x828649 mirrors the
entries' y); a fraction outside 0..1 fails; the 0x88-byte camera entry is
the last whose start (+0x0C) is at or before it (0x442B20, 0x442D00).
The director makes a track camera only when that entry's type byte is
above 1 (0x5FB46C), as mover 0x12 (0x5F1FE0 → constructor 0x453C00,
vtable 0x788018, update 0x453F50).
* The data (loader 0x44A6C0 → 0x44A620): chunk 0x8003B200 route cameras
  (track bundle; L4RA: 35 groups, keyed {u16 road section, u16
  direction}, ≤ 3000, manager +0x18), 0x8003B201 generic (InGameRace /
  Drag / Drift / FreeRoam.bun, 38 groups keyed by id, ≤ 256, +0x10),
  0x8003B202 typed (InGameCommon.bun, 6 groups keyed by type, ≤ 100,
  +0x14). Each child 0x3B210: u32 key, u32 track count, then the tracks:
  a 0x20-byte header (+0x0C duration in seconds, +0x10 u8 entry count,
  +0x12 name) and count × 0x88-byte entries.
* An entry: +0 type (0, 0x12 or 0x13; L4RA 31 / 40 / 12), +1 blend from
  the previous shot, +2 Hermite paths (else straight lerps, 0x446180),
  +4 / +5 eye / look space, +6 / +7 collision pushes (0x44B040 /
  0x444530), +8 hold the car frame from the shot's start, +9 shake,
  +0x0C start fraction, +0x10 / +0x14 tangent scales, +0x18 / +0x24 eye
  start / end, +0x30 / +0x3C look start / end, +0x48 / +0x4C field of
  view (degrees), +0x50..+0x6C four start / end scalar pairs for the
  view's depth-of-field and shake fields, +0x70..+0x81 byte pairs.
* The track's fraction (0x442B70) by its group's kind: generic, the
  scripted scene's; route, the car's road-section fraction; typed, the
  time since the camera began over the track's duration.
* Motion (0x451B70 on a new entry, 0x453F50 each frame): t = (fraction −
  start) / (next start − start); eye and look are cubic Hermite curves
  (a t³ + b t² + c t + d, 0x574880) from the entry's two points with
  tangents from the neighbouring entries (0x450040: the end-to-end
  difference × the tangent scale, split by the two shots' lengths when
  they chain); space 0 transforms by the car's frame (matrix car +0x90,
  position +0x60), 2 adds the car's position, 3 transforms by the scene
  frame, 1 is world; the view looks from eye to look with up +z
  (0x5BA9B0), the field of view interpolated in degrees (0x43E6D0 ×
  0x417F5C2A to bin angle units, 0x4462A0).
* **The reset key's chain (2026-11-11, verified in the disassembly)**: the chain test is
  0x44A160 / 0x4434F0 (next type ≠ 0, tangent scales and lenses exactly
  equal, same type and spaces, points within 1e-5 inclusive; `chains`);
  chained tangents follow 0x450040 (`end_tangents`; fraction lengths
  0x442D70 = next start − start, 1.0 past the last); the record fraction
  is 0x5D4550's re-centred value, a number outside the record too
  (`record_fraction`); the route group comes from the car's own road
  record only, with the race coordinates built (`route_group_of`); the
  candidate loop takes 0x447910's clamped entry (`entry_at`); a held
  entry of type 1 is dropped without a draw, a picked type-1 track is not
  kept, and a draw with +0x224 clear first steps the shared generator
  once (0x5FB2EA) — all in `TrackDirector::direct_on`, which the replay
  director uses (src/bin/game/sim/session/replay.rs; its car record is
  the clone's human road-node stand-in, **inferred**), with the car test
  `eye_in_car` against the rivals and the drawn traffic in the blocked
  closure for the route and typed draws. The live race's draw
  (0x5FB060 outside a replay) is **open**: past the gate (player +0x234
  ≠ 0 — a NIS scene playing —, or race status 3 with 0x89E7B8 > 0, the
  demo; 0x82A3A4 reads 0 in the image and has no absolute writer) with
  the mover not of type 2–4 (none under a NIS) the unit is drawn every
  update while player +0x224 is clear — 0 from the constructor
  (0x5F1C6C, **likely**: ebx = 0), set only by 0x5FAF90 from the replay
  restore 0x57C1E0, cleared at 0x5FB5BD — and 0x44FE80 then draws and
  may build a track mover; porting the unit alone would leave those
  draws out. Open: the
  spline lens curve (0x4462A0 → _ftol of 0x574880 into camera +0xC4: the
  curve's end values come from locals of 0x451B70 whose conversion from
  the entry's focal lengths was not traced), and the ground step's origin
  height (+0x1C60's writer).
* **The clone** (`src/track_camera/`, `TrackDirector`; the route
  groups only). On automatic, each director frame takes the route group of
  the car's record: the race record covering the car's race coordinate
  whose id and flags bit 1 key a group (the race's direction first), the
  car's fraction along it ((coordinate − lo) wrapped by the road length,
  over the span). It keeps the held shot while its entry at that fraction
  is a track camera (mover above 1), else picks at random among the
  group's tracks whose entry there is a track camera and runs 1.5 s or
  more at the car's signed speed (0x44F7B0: from the fraction to the start
  of the first entry after the chain; 1000 s at rest). The eye and look
  are the Hermite paths (tangents: the entry's end-to-end difference times
  its tangent scales, 0x450040) or straight lerps, in their spaces (0 the
  car's frame, 1 world, 2 world axes about the car), up +z; the field of
  view is lerped in degrees; the frozen flag (+8) holds the car's frame
  from the shot's start. The track's view replaces the chase view while it
  shows; the chase views are picked only when none does.
  **Inferred**: which of several overlapping records is the car's (the
  first covering one with a group; the test's 448 is the verified case),
  the chain test's fov compare, a chained entry's tangents (taken as
  unchained), space 3 (the scene frame for the shop scenes, see Shops
  "Cameras"; read as world elsewhere), a spline
  entry's fov (lerped), the wrap of the fraction, and that a lapsed shot is
  re-picked with no hysteresis.
  **Open**: the fixed camera +0x1C34 (the scripted scene's track is
  ported for the shop scenes), the typed and
  generic groups (the NOS weight 0x441AF0 and the flag functions), the blend
  (+1), shake (+9), the cylinder push 0x44B040, the depth and shake fields
  (+0x50..+0x6C); the ground height of space 3 (0x4427C0) is ported (Scene and track camera lens).
* **Visibility, avoidance and re-picks** (round 2, **verified** in the
  disassembly; ported in `track_camera::visible`, `Drive::sight_blocked`,
  `TrackDirector::push_from`):
  - **0x448280** (entry, fraction, car; the route draw 0x44F9A0 calls it
    for each candidate, the typed draw for weights under 10): the entry's
    eye start (0x447A30(0)) taken through the **car's frame whatever its
    space** (0x401340, the sums at 0x4485B6..0x448663; so a world-space
    eye, space 1 — 34 of L4RA's 52 entries of type 0x12 / 0x13 — tests a point
    that is not the shot's eye) ends the sight test **0x5E7EE0** (car
    +0x60 → that point: under 30 m one piece, else _ftol pieces, each with
    the polygons 0x5E6FB0 gathers and 0x5D3930's crossing test, the box
    grown by 0.5); any crossing fails. **0x5F1500**(point, 0.5, car) fails
    when another active car of the world list has its centre (+0x50)
    within a squared distance of 5.5 and its body box (+0xE0..+0xF8)
    grown by 0.5 holds the point (**open** in the clone). Then, in the
    car's frame, against the view showing (its camera through car +0x18 →
    0x48B1E0, else the director's): d = its look − eye, e = the entry's
    look start − eye start (0x447A80(0)), normalised: e.y · d.y < 0 fails;
    e · d ≤ −0.5 fails; e · d ≥ 0.866: 1 / (|view eye in the car's frame|
    · tan(+0xC4 / 2)) and 1 / (|eye start| · tan(the entry's +0x48
    half-angle, 0x443D20)) must differ by more than half the larger
    (0x401050), then the motion test; 0 < e · d < 0.866: the motion test;
    e · d ≤ 0 passes. Motion: the view camera's velocity (+0x1E0) taken
    through the inverse car frame **as a point** (its translation added,
    0x4484DC), normalised, dotted with the entry's eye path (eye end −
    eye start, 0x447A30(1), 0x4011B0) normalised: above 0 passes.
  - **The mover's pushes** (0x453F50, by the track's group kind, 0x442B70:
    0 scene, 1 route, 2 typed): typed — mover +0x84 = 0x444530(view, car,
    0) (the push from the car; its sweep's hit) then 0x44B040; route —
    with entry +7 (and 0x82A3A4 < 11) 0x444530(view, 0, &look) (the push
    from the look point, no car sphere step, result dropped), then
    0x44B040(view, car) always; scene / none — entry +7 as route, entry +6
    → 0x44B040. So a route camera is never dropped for being blocked; its
    eye is pushed only when its entry asks (+7; one L4RA entry).
  - **Re-picks** (0x5FB060 in RCSTATE 5): nothing while paused or while a
    mover of type 2..4 runs; on index 0, unless a chase view picked under
    1.5 s ago holds (+0x214, +0x224), it draws again (0x44FE80) when the
    held track's entry at the fraction (0x442DE0) has type 0, when the
    0x12 mover's +0x84 is set (typed tracks only; 0x828655 = 1), when
    0x441B40(car) > 0 (car +0xB8 < 0, or the flight prediction 0x5EC3E0(3)
    over 1 / 1.5) or car +0xB8 < 0; with no track drawn it goes back to a
    chase view (0x5FA830). Ported (docs/notes/parity/replay-cameras.md): the airborne
    re-pick (0x441B40 > 0: upside down, or Jump's long predicted flight,
    `long_flight`) and the 1.5 s chase hold before a track
    (`ReplayCamera::gate`; the car test 0x5F1500 is `track_camera::eye_in_car`);
    **inferred**: the view's velocity as the eye's change over the clone
    frame, the piece count of 0x5E7EE0.
  - The clone's black frame (side-by-side 09) was a route shot the clone
    drew without this test; the original has no per-frame sight test
    either, so a drawn shot runs on whatever passes later. Whether the
    original's test refuses that very shot is not checked (**open**). A
    clone replay of 4014 from the green light after the port (round 2)
    shows track shots of the car
    throughout and no black frame.
The PC key behind each event: the events are
FE pad messages (the table values at 0x7F7D20 entries 6 to 8 are **verified** in this pass; the names are the base's bin-hash claim, not re-derived here): 0xB5AF2461 PAD_START,
0x5073EF13 / 0xD9FEEC59 PAD_LTRIGGER / PAD_RTRIGGER, 0xC519BFC0
PAD_BUTTON1, Back 0x911AB364 PAD_BACK (0xD97360E9's and Accept
0x0C407210's names not found; Accept is pad bit 4, below). The FE keyboard map gives each event its key (**partly verified** here: rows 4 and 6 of the descriptor table 0x7FDD10 have JOY ids 0x1E and 0x20 and keys 0x24 and 0x23 at +0x12, as the base says for row 6; the first row per JOY id, the JOY names and the DIACTION link were not re-traced): FENG_START `,` (row 4,
"Menu Button 2"), FENG_LTRIGGER / _RTRIGGER `[` / `]` (rows 9, 10),
FENG_BUTTON1 C (row 6), FENG_CANCEL Esc (row 13) and FENG_SELECT /
_ALTERNATE Enter / Num Enter (rows 11, 12). The `,` of row 4 and the other row keys here are base claims that this pass did not re-read. That a FENG_ event reaches the
pad message of the same name (PAD_START for FENG_START) is **inferred**:
the message side is **verified** in this pass: a press of pad bit 6 sends
0xB5AF2461 (the entry at 0x7F7D38, read as 3048154209, picked by the bit as `(&DAT_007F7D20)[bit]`; the case-6 branch of 0x54D4C0 sends
it to the focused object's response list when that has one, else to the
package's). The only indirect call in 0x5809D0 is `call ecx` at 0x580D79,
the decompiled final loop's (device, code, value, data) call; its records
come from 0x57B0C0 and 0x5793A0, with the handler and data at the list
entry's +0x10 and +0x14. **Open**: which node-list entry the START key (`,`) binds (the device list at 0x8655A0 + device * 0x1C8 that 0x5793A0 searches),
what its handler does, and whether it sets pad bit 6, so the link to
FENG_START stays inferred. The
dispatcher 0x54D4C0 reads pad bits 0..0x12 (4..0x12 in its main loop, 0..3
in a second loop); the table at 0x7F7D20 has PAD_LTRIGGER at 7 and
PAD_RTRIGGER at 8. The clone binds those keys. **Verified** (0x5C18F0..0x5C1954, the keyboard branch of the poll 0x5C1850,
taken when the device's type at +0x234 is 0x13): the poll reads the buffered
device data (vtable +0x28) and, per event, sets or clears one bit of the
64-bit mask at +8/+0xC through the shift helper 0x75D8E0. The bit is the
event's uAppData (+0x10 of the 0x14-byte DIDEVICEOBJECTDATA; the shift count
is its low byte), so a key's DIACTION app data is its bit in the device
mask. A release (dwData at +4 zero) sets the bit and a press clears it, so
the mask is active-low; 0x75D8E0 returns 0 for counts above 63. A press of
app data 3 or 4 also sets DAT_00870785 = 1 and DAT_00870784 = 0
(0x5C1903..0x5C190A). The poll's only caller is 0x5CF230. **Open**: what
reads the device mask and copies it into the FE pad state, and the step from
its bits to the FE pad bits below. The FE pad dispatcher (0x54D4C0,
**verified**) walks each pad's 0x88-byte state ({previous mask, current
mask}) and, for bits 4..0x12, sends on a press the message 0x7F7D20[bit]
— bit 4 Accept: 0x0C407210 to the focused object when one of its
responses takes it, else 0x406415E3 to the package; 5 PAD_BACK, 6
PAD_START, 7 / 8 PAD_LTRIGGER / RTRIGGER, 9..0x12 PAD_BUTTON0..9
(0xC519BFBF..0xC519BFC8) — while held 7 / 8 repeat 0x447315AF /
0x20AD4EB5 (0x7F7D50[bit]), and on release 0x7F7D78[bit] (bit 4:
0xC12E9E27); bits 0..3 are the directions (pairs from 0x7F7CFC, messages
0x7F7C90, auto-repeat after 0x140 then 200 ticks). 0xD97360E9 is not in
these tables: no pad bit sends it.
* **FE events on the PC keys** (**verified** unless marked; the clone's
  keys for them are Key::Start, Button0, Button3 and Quit in
  `crates/ob-menus/src/frontend/screens/`, bound in `fn key` in `src/bin/game.rs`;
  BUTTON2 shares `,` with START and has no Key of its own). Pad
  messages by bit (table 0x7F7D20): 6 PAD_START 0xB5AF2461, 9 PAD_BUTTON0
  0xC519BFBF, 11 PAD_BUTTON2 0xC519BFC1, 12 PAD_BUTTON3 0xC519BFC2. No pad
  bit is named QUIT (**Open**: the quit message hash is not identified in
  the EXE; see the QUIT item below). Releases (0x7F7D78[bit]): bit 4 0xC12E9E27 (at 0x7F7D88, the
  Accept release; corrected from 0x936A6A7F, which no pad table holds), bit 6
  0xEBFCDA65, 9 0xD4671F83, 11 0xDC7C41C5, 12 0xE086D2E6. Code sites (a grep
  of the disassembly for each immediate, checked against a byte search of the
  EXE): 0xB5AF2461 at 24 sites (22 compares, 2 pushes),
  0xC519BFBF at 3 (0x4D7D3E, 0x4F964C, and the 0x800 mask's base at
  0x52D1F9), 0xC519BFC1 at 1 (0x4E5636), 0xC519BFC2 at none.
  * START (`,`, row 4). Ported: the replay's slow motion (0x4D7C5B, START →
    0x4A4FB0; the clone takes comma while not paused). Traced, not ported:
    UI_PerformanceTuning_Setting (0x55A750; START at 0x55A76E → 0x55A91A:
    the flag at +0x150 against 0x8651A8, then code on the race state 0x8900D8,
    not traced); the Graph and NOS screens (0x5654E0; START at 0x5655E2 →
    0x565636: +0x54 against 0x8651A8, then 0x50FDD0); 0x5650D0 (START at
    0x565203 → 0x565358: a +0x150 test, then the table 0x8900AC indexed by
    0x83A64C; screen not identified); and two handlers that take START on
    the Accept path (0x406415E3): 0x4C53D9 (at 0x4C53E7, the package at
    0x836498) and 0x55AFFB (at 0x55B00F, [esi+0x4C] against 0x784260). The
    other START compares (0x498084, 0x4A7D9D to 0x4A834B, 0x4C22D7, 0x4C2480, 0x4C533E,
    0x4C5584, 0x4F3B90, 0x4FA82E, 0x50D4CD, 0x51DD45, 0x55D94C, 0x564B4F,
    0x564C2C) are not traced to their screens.
  * BUTTON0 (P, row 5; name unverified, see above). The replay's hide
    (0x4D800D → HIDE 0x4A5200) is **not** reached by P on PC (observed
    2026-10-31: P leaves the control's +0x74 at 0), so the clone binds no
    hide key. Not ported: 0x4F964C reads the
    flags at +0x14F and +0x150 and pushes 0xE1FDE1D1 and 0x541389E5 (the call
    is not traced); the 0x800 mask gives BUTTON0 on screens other than
    UI_PerformanceTuning_Master (0x52D1F9), and its receiver is not traced.
  * BUTTON2 (`,`, row 7). Not ported: its one compare (0x4E5636, in
    FUN_004E4A30, the vinyl-layer handler whose BUTTON1 is compared at
    0x4E5650 and Reorder Layers 0x9803F6E2 at 0x4E5648) goes to 0x4E585A, a
    test of the difference of [0x8651A8] and [0x83A9C8] against the float at
    0x7F942C, then pushes of 0xA0 and 0x53F9FD5C (not traced). The clone
    sends only Start for comma.
  * BUTTON3 (`.`, row 8). Not ported: no code compares or pushes
    0xC519BFC2 (**verified**: the EXE holds it only as pad-table entry
    0x7F7D50), so no code compare reacts to it. A bounded range check that
    covers it is not ruled out (**Open**; ordered compares sit near it). A reaction from FNG data is
    not traced.
  * QUIT (Q, row 24). Not ported: no pad-table bit is QUIT (**Open**: the
    quit message hash is not identified; the checks are below). **Verified**: JOY 0x32 is
    JOY_EVENT_FENG_QUIT (name 0x79FBC0; the JOY table index 28 at 0x7FABA8 is not re-checked), and
    it is the only one of the 83 descriptor rows (count 0x7FE770) with +8 =
    0x32: row 24 at 0x7FE010, codes 0xFF.. (byte +0 = 0xFF; the "any device kind" reading is not traced), match 0x571C80,
    bytes +0x10 = 1, +0x12 = 0x2F, +0x14 = 0, +0x16 = 1 (the names inversion, second key, value and flag are not traced; +0x16 gates rows in 0x57B0C0). Key 0x2F is the
    DIACTION entry with action id 0x2F (the id 0x2F entry is at index 16, address 0x7FDA60;
    name "Quit", semantic 0x81000410 (DIK 0x10 = Q is the DirectInput convention, not read from the binary). So
    the step from this row to a device-mask bit is not traced (**Inferred**: the stored value is one byte, not a bit). **Verified** event
    path (0x57B0C0, 0x5809D0): 0x57B0C0 gives each node of the device list
    its row (0x572130) and, when the match returns a code, stores the code
    (value << 8 | 0x32), the device, the node's +0x10 handler and its +0x14
    data; 0x5809D0 sets DAT_00865658[dev * 0x1C8 + 0x32] = value and calls
    handler(dev, 0x32, value, data). **Open**: the handler (node +0x10, set
    when the node is made, not traced here), so the FE pad bit JOY 0x32 feeds
    is not identified. The literal 0x865658 is read only by the accessor
    0x571A10, whose callers are 0x443AC0, 0x450F80, 0x454970, 0x456940 and
    0x5F3560 (**verified** from the callers list, not traced); the pad
    dispatcher 0x54D4C0 (label not verified) is not among them. A read through a device-record
    pointer (0x8655A0 + 0xB8) is not ruled out.
  Quit message, checks this pass (the marker stays open). None of these
  candidate hashes is an immediate in the disassembly or a 4-byte value
  anywhere in SPEED2.EXE (**verified**: a grep of each value finds 0 sites,
  a byte search finds 0 hits, and 0xB5AF2461 finds 24 immediates): bin_hash of
  PAD_QUIT 0x912342F6, FENG_QUIT 0xA47C7321, JOY_EVENT_FENG_QUIT 0x84BDD6B3,
  QUIT 0x001BC582, Quit 0x001C51E2, quit 0x002DDE02, PAUSE_QUIT 0x605FA0DF,
  FE_QUIT 0xC803B12C and MENU_QUIT 0xF48BC2F6 (**verified** by recomputation).
  bin_hash("PAD_" + button) gives 0xB5AF2461 (START), 0xC519BFBF (BUTTON0),
  0xC519BFC1 (BUTTON2) and 0xC519BFC2 (BUTTON3). The pad table 0x7F7D20 holds
  them at u32 indices 6, 9, 11 and 12 (**verified** for these four; the names
  are **inferred**). Four of the six calls to 0x50DA20 have an immediate pushed
  within five lines before them: 0xB5AF2461 (0x4A7E0F), 0xC407210 (0x543E3E),
  0x911AB364 (0x543ED9) and 0xC519BFC0 (0x56ACF4); the other two (0x4C23DD and
  0x564B74) have none (**verified**). The binder 0x5721A0 has one call site,
  0x50BBE5, in the routine the worker names 0x50BB10; that routine's start and
  its arguments are **not** re-checked. The dispatcher 0x5809D0 has nine call
  sites in two clusters, 0x5813BF to 0x58147B and 0x60A6E9 to 0x60A7A1
  (**verified** by grep); the callers 0x5811D0 and 0x60A690 are not re-run. The
  26 compares with 0x32 are **verified** by count; the roles given for the five
  read sites (0x582587, 0x586EE6, 0x586F42, 0x74846F, 0x7484A7) are **not**
  re-checked.
  Node path, from the disassembly (**verified**): row 24 is the only one of the
  83 rows with +8 = 0x32. The row search 0x572130 walks 0x20-byte rows from
  0x7FDD10, matches the code at +8 and the kind bytes at +0 (0xFF is a
  wildcard; with a zero device value only codes 1 and 2 match), and returns the
  row pointer or 0. REA lists its callers as the binder 0x5721A0, the node
  builder 0x579420 and 0x57B0C0. The constructor 0x579370 stores node+8 = code,
  node+0x10 = handler (its second argument) and node+0x14 = its third argument,
  and zeroes node+0xC, +0x18, +0x1C and +0x20. The builder 0x579420(device,
  code, handler, context) stores the row from 0x572130 at node+0xC. Its handler
  arguments are immediates such as 0x4078a0 (codes 0xA5 to 0xAD at 0x410480)
  and 0x5FA910 (code 0xBD at 0x606301); the handler bodies are not read. No
  push of 0x32 or 0x32 immediate lies within five lines before a call to
  0x579420, and no push of 0x32 is followed by a call to 0x579420 or 0x579370
  within twelve lines (**verified** by grep over 165 call sites). Calls whose
  handler comes from memory or a virtual call are not all traced.
  **Open**: the handler that the code-0x32 node holds (node+0x10), and so the
  message it passes, is not identified; the call that creates that node is not
  found.
  The menus ignore the five keys: `menu_key` returns before its sound or any
  screen code. The replay reads `,` itself (the replay block in `fn key`).
