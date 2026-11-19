# Runtime I/O robustness: the unwrap/panic audit (2026-11-05)

Not native: engineering hygiene. The original has its own failure paths
(the memory-card messages for saves, DirectSound's absence for audio);
where the clone's code could panic on a file, device or user-data error,
it now reports the error and goes on the way the success path would.
Success paths are unchanged (parity gate, `docs/developing/perf-gates.md`).

## Audited

Every `unwrap` / `expect` / `panic!` / `unreachable!` / slice index outside
tests in the code that touches files, devices or user data: `crates/ob-core/src/cache/`,
`src/audio/mixer/` (device open, watchdog), `src/audio/streams/`
(decoder threads), `crates/ob-formats/src/formats/native_save.rs`, `src/frontend/profiles/`,
`src/frontend/career/` (`Profile::from_native`: constant offsets, capped
counts, map lookups only), `src/frontend/options/` (options.txt),
`src/qol/` (saves, backups, crash reports, game-root file),
`src/hd_ui_pack.rs`, `src/frontend/map_pack.rs`,
`src/render/texture_pack.rs` + `src/upscale.rs` (`parse_dds`),
`src/surface_maps.rs`, `src/bin/game/enh/photo/`. The headless check tools in
`src/bin/game.rs` (shots, bench dumps, flows) are out of scope: they are
meant to fail loudly. Missing game files keep their existing clear error.

## Changed

| site | was | now |
|---|---|---|
| `profiles::with_current` (the current profile) | `CURRENT.lock().unwrap()`: a thread that panicked while holding it made every later profile access panic | poison ignored (`PoisonError::into_inner`): the payload is a whole fixed-size save in any case |
| `Settings::save` (options.txt) | `std::fs::write`: a crash or full disk mid-write left a cut-off file | `qol::saves::write_atomic` (temporary file + rename, as the profiles); a failure is logged (`options not saved to …`) and returned (callers ignore it as before; the options in memory stay) |
| photo mode writer (`photo.rs` `write_picture`) | a failed write was dropped silently; `image::save_buffer` panics on a buffer of another size after creating the file | the buffer size is checked first; a failed write or a dead writer thread is logged when joined (`written`) |
| `cache::spawn_writer` | `std::thread::spawn` panics without a thread | `thread::Builder::spawn` (same unnamed thread); without one the write is skipped with a warning, as a failed write |
| audio decoder threads (`streams.rs` NIS line, speech, song, score) | `std::thread::spawn` | `spawn_decoder` (same unnamed thread); without one the track is marked done (silent) with a warning |
| audio watchdog's old-stream drop (`Mixer::keep_alive`) | `std::thread::spawn` | `Builder::spawn`; without a thread the stream closes inline (a wait) with a warning |
| surface maps (`surface_maps::Builder::start`) | `.expect("surface maps thread")` | without a thread: a warning, no maps (as slots that get none) |

Kept, with a comment why they cannot fail: `cache.rs` `Reader::u64` /
`persist!` conversions (exactly N bytes taken), `persist_enum!` position
(every variant listed), the two `bytes(&e.name).unwrap()` (names from the
file's own header and index), the `/proc/self/fd` `CString`;
`profiles::with_current`'s `f.take()` (runs once);
`surface_maps` cache path parent; `map_pack::apply`'s `unreachable!`
(`parse` gives fields 0..=5). Already safe and left alone: the mixer's
device locks (all poison-tolerant), `Mixer::new` returning `None` without
a device (the game runs silent), `NativeSave::parse` (size, magic and
header checksum checked before any field is read), cache files (size,
build, source keys and per-4 MB hashes checked before use).

## Tests (failure paths)

* `cache::tests::an_unwritable_cache_folder_is_a_miss` — the cache folder
  blocked by a file: open is a miss, writes and `replace_file` are errors;
  a source gone while caching is an error.
* `frontend::profiles::failure_tests::*` — missing, empty, cut-off, one-byte
  and text profile files (and a folder in the file's place) are `read` /
  `read_cover` / `load` errors; an unwritable profiles folder makes
  `write`, `write_cover` and `delete` errors; a poisoned profile lock keeps
  working.
* `frontend::options::file_tests::*` — options.txt missing, not UTF-8,
  junk keys/values and cut off mid-line load as defaults (or the whole
  lines before the cut); the write leaves no temporary file and fails
  without a panic when its folder cannot be made.
* `audio::mixer::tests::no_audio_device_is_none` — no output device
  (`NFSU2_AUDIO_HOST=no-such-host`, in a child process): `Mixer::new()` is
  `None`, within the open wait.
* `photo::write_tests::a_picture_goes_to_the_first_free_name_or_fails_cleanly`
  (game binary) — names, a blocked folder, a wrong-size buffer and a dead
  writer thread.

## Open

Thread creation failing elsewhere (render workers, loaders joined with
`expect` in `game.rs`) is not handled: the game cannot run without them.
The render-side self-check cache (`postfx.rs`, `post-check` verdict) is
still written with a plain `fs::write` (a cut-off file reads as
"separate passes", a valid state): open.

## Fuzzing (2026-11-05)

`tests/fuzz_readers.rs`: every reader that takes bytes from a game file, a
save or options.txt, at its API boundary (whole files and the chunk-taking
parsers with the chunk ids their callers select): the chunk walker, JDLZ,
HUFF, TPK (+ the RGBA decode), solid lists (+ markers), scenery sections,
light / flare packs, texture animations, camera tracks, skinned models, the
track readers (streaming sections, visibility, parameter maps, world
animations), routes / roads / magazines / showcase, the GLOBALB readers
(car types, presets, light materials, tricks, AI bounds, part database,
particles), the career cost / tuning tables, native saves (parse, field
readers, `Profile::from_native`, the picture; and any payload behind a
valid header and checksum), options.txt (`Settings::from_text`), fonts,
FEng packages and responses, the language table, ABK / GIN / SCHL audio,
the VP6 container, EAGL banks, skeletons, scene characters, part
animations, VIV headers, DDS.

* proptest (dev-dependency, THIRD-PARTY-NOTICES.md): random bytes, any
  chunk header (id, size) over random bytes, options.txt lines of known
  keys with any value — into every reader.
* A seeded mutation engine (xorshift; `NFSU2_FUZZ_ITERS`, default 1000 per
  target, `NFSU2_FUZZ_SEED`, `NFSU2_FUZZ_ONLY=<target>`,
  `NFSU2_FUZZ_DUMP=<dir>` keeps failing inputs): a chunk's size set to a
  boundary (0, 1, 7, 0x7FFFFFFF, 0xFFFFFFFF, its body ± 1, twice …) or
  nudged, a u32 in a body set to a boundary (counts, offsets), cut-offs,
  bit flips, junk tails. Seeds: synthetic (a chunk tree, the
  the finished-career save fixture, an options.txt) always; with NFSU2_ROOT the
  real files (InGameCommon.bun, L4RA.BUN, GLOBALB.BUN, the Corolla's
  GEOMETRY.BIN and PARTS_ANIMATIONS.bin, FrontB.lzc and GlobalB.lzc and
  their decompressed chunks, MAGAZINES_SHOWCASE.BIN, English.bin, an AI
  path file, an ABK, a GIN, the first 2 MB of a VP6 movie, the first 24 MB
  of STREAML4RA.BUN), the chunks with each target's ids.
* The test binary's allocator names the target and prints a backtrace for
  any request past 1 GiB (no reader needs one), before the abort.
* Each input must return; a panic or a call over 5 s is a failure (all
  are listed at the end with the panic's site).

Runs: `cargo test --release` (1000 mutations per target, about 6 s), and
long runs with overflow checks and debug assertions on (a target dir of
its own: `CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true
CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`), 50 000 mutations per target
and seed: 9 seeds while fixing, then 4 more seeds after the last fix
(81–84, about 2.3 M mutated inputs each), all clean.

### Found and fixed

| reader | input | was | now |
|---|---|---|---|
| `camera_tracks::parse_camera_tracks` | a group's track count 0xE0000000 | `Vec::with_capacity(count)`: a 3.7 GB allocation, abort | capacity held to what the bytes can hold; the count still ends at the first truncated track (an error, as before) |
| `scenery::parse_scenery_section` | a header chunk shorter than 0x10 | slice panic (read of +0x0C) | error "scenery header" |
| `parameter_map::ParameterMap::parse` | a short header, a field offset or row stride past the values, 0 rows | slice panics; with 0 rows `sample` underflowed later (`rows.len() - 1`) | header length, the fields' offsets and every row's extent checked (no overflow) before any read; a map without rows is an error |
| `solid::parse_mesh` | a material whose first index + count wraps u32 | wrapped to a small count that passed the length check (release) / overflow panic (debug) | summed in u64 and checked against the index buffer → the mesh is skipped with the existing warning |
| `zam::parse_bank` (part animations) | an ELF object cut inside its header or symbol table | slice panic | error "ELF header"; the symbol scan stops at the table's end |
| `character::elf_data` / `Skeleton::parse` | the same; a bone count whose extent overflows | slice panic / overflow | errors ("ELF header", "skeleton bounds", checked arithmetic) |
| `eagl` key-block decoder | a block shift ≥ 64, a block with no keys | shift overflow (debug), `k − 1` underflow then index panic | errors "keyblock shift", "keyblock without keys" |
| `eagl` delta / key-block slots | slot base + component past 0xFFFF | u16 overflow (debug; release wrapped) | `wrapping_add` (the release result, unchanged) |
| `abk::Bank::parse` | a bank table running past the file | slice panics (count at +6, an entry) | the count read only when present, the table ends at the file's end |

Valid files parse exactly as before (every guard is on a path real data
never takes; the parity gate's images are unchanged).
