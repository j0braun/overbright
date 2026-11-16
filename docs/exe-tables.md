# Tables from the player's SPEED2.EXE

Overbright ships nothing of the original game. The numbers, names and curves the
original keeps in its executable are read from the player's own `SPEED2.EXE` when the
game starts (`src/exe_tables/`), a new profile is written from the original's rules
(`src/frontend/career/native_map/fresh.rs`), and menu texts come from the player's
language file by their hash.

## The loader (`nfsu2::exe_tables`)

* `init(root)` — once, in `Game::new` before anything else loads: finds `SPEED2.EXE`
  through `game_fs` (any case), maps it read-only (memmap2) and builds every table
  (`load`). The edition rule:
  1. **the supported build** (size and XXH3-64 of `qol::setup::KEY_FILES[0]`, 4,788,224
     bytes, NFSU2 PC 1.2): taken as it is (the whole-file hash already proves every table);
  2. **any other file** (no-CD and widescreen patches change a few code bytes): read the
     same way, then every table — the code-held values and strings included — is checked
     against the SHA-256 of its `Debug` text pinned in `tables::FIELD_PINS` (hashes only,
     never the data; `Tables::verify`, 0.59 ms for all 65). All equal: accepted
     (`Edition::Modified`), with one log line and an Info line on `--check-game` / the
     launcher's check screen: "SPEED2.EXE is a modified copy of the supported version;
     its game data matches";
  3. **any table different or unreadable** (another version or layout, a broken section
     table): refused before anything runs with the plain-words message
     (`qol::setup::exe_help` / `exe_differs_line`): "this SPEED2.EXE is <version from the
     fingerprint> / an unknown version: its data tables differ from the supported 1.2
     release", naming the tables that differ, then the folder advice.
  `check_folder(root)` gives the same verdict for the check screens. The PE section reader
  and every read are a fuzz target (`fuzz/targets.rs` `pe_sections`).
* `tables() -> &'static Tables` — the one process-wide, read-once holder. The tables are
  used from many places (audio threads, physics, codecs, front end, formats), so a
  struct handed down every call path would rewrite dozens of signatures; the game
  builds the holder at start and the frame path only reads memory (an atomic load and a
  field). Tools and tests that never call `init` get it built on first use from
  `game_root()`.
* `Table<T>`: a `static` with the old constant's name and type, `Deref` to the loaded
  table, so users keep `NAME[i]`, `NAME.iter()`, `NAME.len()`; by-value uses read
  `*NAME`.
* `Exe`: `bytes(va, len)` through the PE section table (data past a section's raw size
  reads as the loader's zeros), `vec` / `array` / `read` of plain types, `records(va,
  stride, f)` with typed field reads, `cstr(va)` / `cstr_at_ptr(va)` (ASCII, or Latin-1
  converted once), and three small code readers for tables the original keeps in code:
  `switch_returns` (a jump table whose cases `mov eax, imm32`), `pushed_args` (the
  immediates pushed before each call to a function) and `initialised_dwords[_with]`
  (the dwords a straight-line start-up function stores). Every read is range-checked;
  a table whose shape is not the expected one is an error, not data.
* `exe()` — the mapped executable, for string access by address.

Cost: about 0.3 ms at start, see "Start-up" below.

## The tables

Loaded (L) from the executable or generated (G) from their formula. "Pin" = the data
test `exe_tables::tests::every_table_equals_the_old_constant` checks the SHA-256 of the
table's `Debug` text against the old constant's (evidence: `docs/evidence/exe-tables/
phase-a.txt`, the comparison run while the constants still existed: 95 equal).

| family | table (users) | VA | count / layout | |
|---|---|---|---|---|
| drive | STEER_RESPONSE, _DRIFT (drive/input.rs) | 0x7E9228, 0x7E9330 | 21 f32 | L |
| drive | LATERAL (drive/force.rs) | 0x86B34C (zeros), 0x7FC744 | 7 × 6 f32 | L |
| drive | atan16's table (drive/tables.rs) | 0x7EACB0 | 256 u8: trunc(atan((i+½)/256)/(π/4)·256) | G |
| race | DRIFT_TUNE, STREET_X_TUNE, DRAG_TUNE (race.rs) | 0x7FC2E4, 0x7FC33C, 0x7FC394 | 3 rows 0x144 apart × 22 f32 | L |
| handling | ZERO_60_FACTOR, ZERO_100_FACTOR | 0x7FC120, 0x7FC1D8 | 31 f32 | L |
| particles | pair_surface WEIGHT (contact_fx.rs) | 0x800EE0 + 0x98·s + 0x34 | 29 f32 | L |
| chase | SHAKE (chase/tuning.rs) | 0x7EC430 | 5 × 4 f32 | L |
| drift | LEVEL_SPEED, LEVEL_POINTS, virtual_scores TABLE | 0x79DA78, 0x79DA90, 0x7FA5C0 | 6 f32; 4×4 i32; 6 × 13 dwords | L |
| AI | LOCK, TABLE_RATE_SPEED (ai/driver) | 0x7E9074, 0x7E909C | 10 f32 | L |
| AI / mixmap | ASIN_TABLE (ai/driver/math.rs), ASIN (mixmap, first 192) | 0x7EA628 | 208 × {u16, pad, f32} | L |
| audio | collision LEVELS, GAIN (also stitch.rs) | 0x7ECB78, 0x7F34A0 | 20 f32; 289 u16 | L |
| audio | mixmap DB_TO_LIN, LIN_TO_DB | 0x7EDE88, 0x7EE7F0 | 602, 512 i32 | L |
| audio | car LEVELS / SHIFTS / TURBOS | 0x7F2230 | 33 × 4 × 12-byte entries (+0, +4, +8) | L |
| audio | SWTN | 0x7F3040 | 43 × 12 (bank name → its number, 2 i32) | L |
| audio | GINS | engine records +0x0C | 43 names less GIN_ / .gin | L |
| audio | ENGINES, DECELS (engine_chain/records.rs) | 0x7F0DE0, 0x7F1AA8 | 43 × 0x4C, 37 × 0x34 | L |
| audio | SHIFT_RECS, LAUNCHES, LAUNCH_INDEX (transients) | 0x7F2860, 0x7F3248, engines +0x18 | 16 × 0x60, 12 × 20, 43 | L |
| audio | skid RECORDS, class C (skids.rs) | 0x7F2F20; switch 0x458F28 / index 0x458F34 | 4 × 0x48; 32 | L |
| audio | sound index MAP (engine.rs) | switch 0x45930C | 31 | L |
| audio | scripted PATTERN1..15 (scripted.rs) | 0x7F0070..0x7F08F8 | {u16 rpm, u16 throttle} | L |
| audio | SCORE_EVENTS (trax/playlist.rs) | 0x7ECC10 | 31 dword pairs | L |
| audio | TRAINS (world/enviro) | 0x7ED798 | 24 × 12 | L |
| audio | event VM quarter sine (aems/instance.rs) | 0x8181D8 | 257 u16: min(65535, trunc(65536·sin(iπ/512))) | G |
| codec | UTK RC, CODEBOOKS, COMMANDS (utk.rs) | 0x819208, 0x819308, 0x819508 | 64 f32; 2 × 256; 29 × 12 | L |
| codec | layer III HUFF trees, LINBITS, WINDOW_ROWS (mp3_tables.rs) | pointers 0x7D0C30, 0x7D0CC0, 0x7D1600 | 32 trees; 16 u8; 17 rows (each twice) | L |
| codec | VP6: AC/DC_DEQUANT, ZIGZAG, COEFF_GROUP, DEFAULT_REORDER, REF_OF_TYPE, DCCV_UPDATE, REORDER_UPDATE, RUNV_UPDATE/DEFAULT, RACT_UPDATE, DCCV_LC, VECTOR_UPDATE, MB_STATS_PRESETS/DEFAULT, CAT_TABLE, CANDIDATES, BILINEAR, TOKEN_BASE, COEFF_GROUP_HUFFMAN | 0x7C4618…0x7C6668, 0x816708 | 20 tables (i32 rows read as bytes where the clone kept bytes) | L |
| render | IN_GAME_RIG, DEFAULT_RIG (car/lighting.rs) | 0x7F3F00, 0x7F3E10 (+0x20, 0x1C a light) | 3 lights | L |
| front end | WORLD_MAP_ZOOMS (ingame_map.rs) | 0x7F42C8 | 6 × 16 | L |
| front end | MAGAZINE_LINES, DVD_LINES, RATING_LINES (dday/hints.rs) | 0x7FA3F0, 0x7FA490, 0x7FA4DC | 20, 10 pairs; 11 | L |
| front end | PART_TYPE_NAMES (car/model.rs) | 0x80333C (stride 8) | 0x69 names | L |
| formats | SLOT_TYPES, SLOT_CATEGORIES, BY_KIND (parts.rs) | 0x802960; init 0x501920 → 0x837D30; switch 0x502020 | 0xAA, 0xAA, 28 | L |
| front end | OPPONENT_CARS (career/events.rs) | 0x7F6DA4 | 26 u8 | L |
| front end | VINYL_GROUPS (graphics_shop.rs) | 0x546040's calls of 0x545930 | 28 (kind, icon, label) | L |
| front end | NAMES (quick_race/race_setup.rs) | pointers 0x7F6CD0 | 53 | L |
| front end | EA_TRAX (+ where it plays) (jukebox.rs) | 0x7ECD80 | 27 × 16 | L |
| front end | TUNING_NAMES (tuning.rs) | switch 0x50FF40 | 25 | L |
| settings | CARD_SCORES (gpu_rating.rs) | 0x8005C0 | 120 × 12 (f32 score) | L |
| garage | SLOT_SHOTS, VINYL_SHOTS, SUV_ENGINE_SHOT (garage.rs) | 0x7F4608 (2 × 0x31 records of 0x3C; records 0..41), 0x7F5D40 (7 a class), 0x7F5D00 | view at record +0xC | L |

**Quarter sine (verified; fixed):** the quarter sine the clone carried
differed from SPEED2.EXE at six entries (11, 13, 15, 19, 20, 21: one less). The original's
stored table 0x8181D8 is exactly min(65535, trunc(65536·sin(iπ/512))) with no exception
(file bytes and a read of the running game agree; no code writes it, its only readers are
0x7254CF's two loads), so the clone's six values were its own error. The generated table is
now the formula alone (`SINE_CLONE_DEVIATION` removed); the data test compares all 257
entries with no exception, `quarter_sine_is_the_exact_formula` states the six values.

Small tables of the same families stay in the source for now (each under the scanner's
64-byte run): e.g. drive/tables.rs MAX_ANGLE, STEER_SPEED, LOCK_GAIN, the drift-assist
rows; ai/driver SPEED_*, TABLE_P/I/D; carfx SHIFT_BANKS, TURBO_BANKS; enviro PLANES*;
vp6 MC_TYPE; drift START_SPEED / KEEP_SPEED, AI_DIFFICULTY; garage SHOTS, PLATFORMS and
TOUR_SHOTS (the clone's tour values are rounded, not the executable's: 0.1998 for
0.19976 — loading them would change the tour, left for a native fix).

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

## Start-up

Measured 2026-11-12 (`exe_tables_cost`, best of 20, file in the page cache): mapping
SPEED2.EXE and its XXH3 edition check 0.235 ms, building every table 0.045 ms. The fresh
profile is written once, the first time a profile is created (not at start).
