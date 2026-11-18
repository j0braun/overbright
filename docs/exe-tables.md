# The engine's tables

Overbright does not read `SPEED2.EXE`. Like other long-lived engine reimplementations
(OpenMW, OpenRCT2, OpenTTD and others), it reads the game's **assets** (models, textures,
sounds, text, tracks) from the player's installed files and carries the engine's **rules and
constants** in its own code. The numbers, curves and names the original keeps in its
executable are written in `src/exe_tables/` with the values of the reference executable, the
European PC 1.2 release (`SPEED2.EXE` without copy protection, 4,788,224 bytes, SHA-256
`8f6a9ae2…dfe2a297`). So any retail copy of the game, patched to 1.2, starts: with the
copy-protected disc executable, without any executable, of any region.

Three sources, one holder:

* **Formulas** (`exe_tables::generated`): tables that are pure maths, computed at first use.
* **Third-party decoder tables** (`exe_tables::third_party`): the standard tables of the
  audio and video codecs, taken from open decoders, each in its own vendored file with its
  notice (THIRD-PARTY-NOTICES.md), and put into the layout the decoders read.
* **Tuning constants** (`exe_tables::constants`): handling curves, sound records, camera
  views, front end facts, as named Rust constants with what each is and where the reference
  executable keeps it.

`exe_tables::tables()` builds them once on first use (about 0.1 ms; `exe_tables_cost`) into
one process-wide `Tables`; users read them through `Table<T>` statics with their old names
(`NAME[i]`, `NAME.iter()`, `*NAME`), and the frame path only reads memory that is already
there.

## Formulas

| table | formula | exceptions |
|---|---|---|
| quarter sine 0x8181D8 (257 u16; the event VM, aems/instance.rs) | min(65535, trunc(65536·sin(iπ/512))) | none |
| arctangent 0x7EACB0 (256 u8; drive/tables.rs) | trunc(atan((i+½)/256)/(π/4)·256) | none |
| mixer dB → linear 0x7EDE88 (602 i32; audio/mixmap) | floor(32768·10^((i−602)/2000)) | entry 0 is stored as 16384 (the formula gives 16385) |
| mixer linear → dB 0x7EE7F0 (512 i32) | max(0, #{i : db_to_lin[i] < (512+j)·32} − 1) | none |
| arcsine 0x7EA628 (208 × {u16 angle, f32 slope}; ai/driver/math.rs, mixmap) | 13 runs of 16 grid steps, run k from 1 − 2^−k in steps of 2^−(k+5) (the last run, k = 12, in steps of 2^−16 up to 1); angle = trunc(asin(x)·32768/π); slope = the secant to the next grid point / 2π, rounded to four decimals | 20 slopes are stored one step (0.0001) lower (`ASIN_SLOPES_ROUNDED_DOWN`); the original's rounding of those is not known |

**Quarter sine (verified; fixed):** the quarter sine the clone once carried differed from
SPEED2.EXE at six entries (11, 13, 15, 19, 20, 21: one less). The stored table is exactly the
formula (file bytes and a read of the running game agree; no code writes it);
`quarter_sine_is_the_exact_formula` states the six values.

## Decoder tables (third party)

| decoder | tables | from | how they are laid out |
|---|---|---|---|
| MicroTalk (UTK; audio/codec/utk.rs) | reflection coefficients (64 f32), codebooks (2 × 256), pulse commands (29) | vgmstream `src/coding/libs/utkdec.c` (ISC) | as they are |
| MPEG-1 layer III (audio/codec/mp3_tables.rs) | 32 Huffman trees, the linbits of tables 16..31, the synthesis window (17 rows of 16) | the ISO/IEC 11172-3 code tables and window D[0..=256] as FFmpeg's libavcodec holds them (`mpa_hufflens`, `mpa_huffsymbols`, `ff_mpa_huff_data`, `ff_mpa_enwindow`; LGPL-2.1-or-later) | a tree is built from the canonical codes depth first, 0 branch first (a node −n jumps n entries ahead on a 1), each followed by one 0; tables 0, 4, 14 are empty, 16..23 and 24..31 share a tree. Window row r, column c = −W[32c + r] / 2, W built from D as FFmpeg does (W[512 − i] = D[i], negated unless i is a multiple of 64) |
| VP6 (formats/vp6.rs) | 20 tables | FFmpeg's libavcodec `vp56data.c` and `vp6data.h` (LGPL-2.1-or-later) | most as they are; the zig-zag scan is generated; the coefficient bands with entry 0 = −1 (and capped at 3 for the Huffman profile); the macroblock type statistics grouped by context (the defaults' two counts swapped); the vector update probabilities interleaved per component (2 + 7 + 8); the DC context models by model, then plane; the motion candidates as (y, x); the bilinear taps (128 − 16k, 16k); the token bases with a trailing 0; the category table built from the token bases, extra bits and parse probabilities |

The LGPL-2.1-or-later permits distribution under the GPL version 3; Overbright is
GPL-3.0-or-later.

## Tuning constants

| family | table (users) | VA in the reference executable | count / layout | source |
|---|---|---|---|---|
| drive | STEER_RESPONSE, _DRIFT (drive/input.rs) | 0x7E9228, 0x7E9330 | 21 f32 | constant |
| drive | LATERAL (drive/force.rs) | 0x86B34C (zeros), 0x7FC744 | 7 × 6 f32 | constant |
| drive | atan16's table (drive/tables.rs) | 0x7EACB0 | 256 u8: trunc(atan((i+½)/256)/(π/4)·256) | formula |
| race | DRIFT_TUNE, STREET_X_TUNE, DRAG_TUNE (race.rs) | 0x7FC2E4, 0x7FC33C, 0x7FC394 | 3 rows 0x144 apart × 22 f32 | constant |
| handling | ZERO_60_FACTOR, ZERO_100_FACTOR | 0x7FC120, 0x7FC1D8 | 31 f32 | constant |
| particles | pair_surface WEIGHT (contact_fx.rs) | 0x800EE0 + 0x98·s + 0x34 | 29 f32 | constant |
| chase | SHAKE (chase/tuning.rs) | 0x7EC430 | 5 × 4 f32 | constant |
| drift | LEVEL_SPEED, LEVEL_POINTS, virtual_scores TABLE | 0x79DA78, 0x79DA90, 0x7FA5C0 | 6 f32; 4×4 i32; 6 × 13 dwords | constant |
| AI | LOCK, TABLE_RATE_SPEED (ai/driver) | 0x7E9074, 0x7E909C | 10 f32 | constant |
| AI / mixmap | ASIN_TABLE (ai/driver/math.rs), ASIN (mixmap, first 192) | 0x7EA628 | 208 × {u16, pad, f32} | formula |
| audio | collision LEVELS, GAIN (also stitch.rs) | 0x7ECB78, 0x7F34A0 | 20 f32; 289 u16 | constant |
| audio | mixmap DB_TO_LIN, LIN_TO_DB | 0x7EDE88, 0x7EE7F0 | 602, 512 i32 | formula |
| audio | car LEVELS / SHIFTS / TURBOS | 0x7F2230 | 33 × 4 × 12-byte entries (+0, +4, +8) | constant |
| audio | SWTN | 0x7F3040 | 43 × 12 (bank name → its number, 2 i32) | constant |
| audio | GINS | engine records +0x0C | 43 names less GIN_ / .gin | constant |
| audio | ENGINES, DECELS (engine_chain/records.rs) | 0x7F0DE0, 0x7F1AA8 | 43 × 0x4C, 37 × 0x34 | constant |
| audio | SHIFT_RECS, LAUNCHES, LAUNCH_INDEX (transients) | 0x7F2860, 0x7F3248, engines +0x18 | 16 × 0x60, 12 × 20, 43 | constant |
| audio | skid RECORDS, class C (skids.rs) | 0x7F2F20; switch 0x458F28 / index 0x458F34 | 4 × 0x48; 32 | constant |
| audio | sound index MAP (engine.rs) | switch 0x45930C | 31 | constant |
| audio | scripted PATTERN1..15 (scripted.rs) | 0x7F0070..0x7F08F8 | {u16 rpm, u16 throttle} | constant |
| audio | SCORE_EVENTS (trax/playlist.rs) | 0x7ECC10 | 31 dword pairs | constant |
| audio | TRAINS (world/enviro) | 0x7ED798 | 24 × 12 | constant |
| audio | event VM quarter sine (aems/instance.rs) | 0x8181D8 | 257 u16: min(65535, trunc(65536·sin(iπ/512))) | formula |
| codec | UTK RC, CODEBOOKS, COMMANDS (utk.rs) | 0x819208, 0x819308, 0x819508 | 64 f32; 2 × 256; 29 × 12 | third party |
| codec | layer III HUFF trees, LINBITS, WINDOW_ROWS (mp3_tables.rs) | pointers 0x7D0C30, 0x7D0CC0, 0x7D1600 | 32 trees; 16 u8; 17 rows (each twice) | third party |
| codec | VP6: AC/DC_DEQUANT, ZIGZAG, COEFF_GROUP, DEFAULT_REORDER, REF_OF_TYPE, DCCV_UPDATE, REORDER_UPDATE, RUNV_UPDATE/DEFAULT, RACT_UPDATE, DCCV_LC, VECTOR_UPDATE, MB_STATS_PRESETS/DEFAULT, CAT_TABLE, CANDIDATES, BILINEAR, TOKEN_BASE, COEFF_GROUP_HUFFMAN | 0x7C4618…0x7C6668, 0x816708 | 20 tables (i32 rows read as bytes where the clone kept bytes) | third party |
| render | IN_GAME_RIG, DEFAULT_RIG (car/lighting.rs) | 0x7F3F00, 0x7F3E10 (+0x20, 0x1C a light) | 3 lights | constant |
| front end | WORLD_MAP_ZOOMS (ingame_map.rs) | 0x7F42C8 | 6 × 16 | constant |
| front end | MAGAZINE_LINES, DVD_LINES, RATING_LINES (dday/hints.rs) | 0x7FA3F0, 0x7FA490, 0x7FA4DC | 20, 10 pairs; 11 | constant |
| front end | PART_TYPE_NAMES (car/model.rs) | 0x80333C (stride 8) | 0x69 names | constant |
| formats | SLOT_TYPES, SLOT_CATEGORIES, BY_KIND (parts.rs) | 0x802960; init 0x501920 → 0x837D30; switch 0x502020 | 0xAA, 0xAA, 28 | constant |
| front end | OPPONENT_CARS (career/events.rs) | 0x7F6DA4 | 26 u8 | constant |
| front end | VINYL_GROUPS (graphics_shop.rs) | 0x546040's calls of 0x545930 | 28 (kind, icon, label) | constant |
| front end | NAMES (quick_race/race_setup.rs) | pointers 0x7F6CD0 | 53 | constant |
| front end | EA_TRAX (+ where it plays) (jukebox.rs) | 0x7ECD80 | 27 × 16 | constant |
| front end | TUNING_NAMES (tuning.rs) | switch 0x50FF40 | 25 | constant |
| settings | CARD_SCORES (gpu_rating.rs) | 0x8005C0 | 120 × 12 (f32 score) | constant |
| garage | SLOT_SHOTS, VINYL_SHOTS, SUV_ENGINE_SHOT (garage.rs) | 0x7F4608 (2 × 0x31 records of 0x3C; records 0..41), 0x7F5D40 (7 a class), 0x7F5D00 | view at record +0xC | constant |
| career | QR_DEFAULT_CARS, RANKED_RIDES (native_map/fresh.rs) | 0x7FAA48; init 0x77D3B0 → 0x8641A8 | 2 u32; 6 × (kept number, name, stock car) | constant |

Label ids (magazine and DVD lines, tuning parameter names, vinyl groups, the ranked rides'
names) are written as their text id hashed with `bin_hash` (a `const fn`), e.g.
`h("TUNING_TYPE_FRONT_SPRINGS")`; the three vinyl icon ids no game file names stay numbers.

Small tables of the same kinds were already in the source before (drive/tables.rs MAX_ANGLE,
STEER_SPEED, LOCK_GAIN; ai/driver SPEED_*, TABLE_P/I/D; carfx SHIFT_BANKS, TURBO_BANKS;
enviro PLANES*; vp6 MC_TYPE; drift START_SPEED / KEEP_SPEED; garage SHOTS, PLATFORMS and
TOUR_SHOTS).

## Tests

* `exe_tables::tests::every_table_equals_its_pin` (no game data needed): the SHA-256 of
  each table's `Debug` text equals the value the reference executable gives (`FIELD_PINS`,
  and finer pins for the VP6 tables, the patterns and the generated sine and arctangent).
* With the reference executable (the maintainer's private set: `testkit::reference_exe()`
  reads `$OB_PRIVATE/exe/SPEED2.EXE`, else the game folder's `SPEED2.EXE`, and checks that
  it is the reference build): `formula_tables_equal_the_executable`,
  `decoder_tables_equal_the_executable` and `tuning_constants_equal_the_executable` read
  every table from the executable (`exe_tables::reader`, a PE section reader that exists
  in test builds only) and compare it with the built-in one entry for entry. A few other
  data tests compare single records the same way (scripted car-audio banks, map zooms,
  burnout records).
* Overbright starts and plays the same with no `SPEED2.EXE` in the game folder and with
  the copy-protected retail one: pictures, sound and simulation hashes are unchanged
  against the version that read the executable.

## Which executables work

Any, or none: Overbright never opens `SPEED2.EXE`. The game folder is recognised by its
data (`qol::setup::KEY_FILES`: GLOBAL/GlobalB.lzc, FRONTEND/FrontB.lzc, the TRACKS files
and the other key files). The fingerprint still names the executable in `--check-game`
and crash reports as information (docs/game-fingerprint.md), the copy-protected retail
one included (recognised by SafeDisc's sections `stxt774` / `stxt371`); it is never part
of the verdict.

## The fresh profile

`NativeSave::fresh(name)` starts from `career::fresh_profile()`, written once per process
by `fresh_payload` (module notes list each step with its address: the stock list
0x516500 — `STOCK_<name>` for each car type with +0x40, +0xDA bit 1 and +0x844 = 0 —,
My Cars, the career slots 0x516480 (all but the first locked), the six ranked rides
0x52A800 from the table 0x77D3B0 builds (ONLINE_CAREER_*), the career's stage numbers,
current slot `CAREER_0`, driving time −1, the legend, the EA Trax playlist (1 FE, 2 IG),
the settings block 0x5116E0, the Quick Race records 0x515800 / 0x515750, 0x515AA0's
difficulty and save prompts, the name, 0x850080, the keyboard block). The data test
`the_written_profile_is_the_originals` pins the SHA-256 of the written payload and file:
**byte for byte the original's** (the six ranked rides' ratings are the set-up's area
0x5B4DD0 over the curve 0x5B5AC0, computed with the game's own rounding: the table
resampled through a graph storing (n − 1) / span, single precision as the x87 runs
under Direct3D — `handling::acceleration_area`, which the physics uses, rounds
differently and is unchanged). **Inferred:** the u16 3 at +0xA of the first twelve Quick
Race entries (the record's constructor is not traced). The old file (SHA-256
e623c3ac…c1e3, 54,966 bytes) leaves the tree (private set:
`$OB_PRIVATE/saves/native_save_fresh.bin`, which `package.sh` uses as a scan reference).
`tests/native_save.rs` `new_profiles_after_bonus_codes_match_the_originals` checks two
profiles the original created under Wine against the clone's, byte for byte.

## Menu texts

The dialogs' English fallbacks (EA server lost, Dyno without a profile, nitrous purge,
engine / trunk neon, custom-tuned setups, new-profile warning) are gone: the text is
read by hash from the player's language file as every other menu text
(`Game::label(hash, "")`, memcard's `label`). All seven are in LANGUAGES/English.bin;
`strings::menu_text_tests` pins each by the SHA-256 of its text.

