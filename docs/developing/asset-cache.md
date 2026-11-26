# The asset cache: names, versions, writes, temporary files, the developer folder

What the game's files are parsed into (tracks, cars' models and textures) is kept in cache files
(crates/ob-core/src/cache/, framing in `crates/ob-core/src/cache/format.rs`). This page holds the rules that keep the cache correct when
several builds and several processes share a folder. No SPEED2.EXE behaviour is involved: the original has no
such cache. A single player with one build sees the same cache contents and the same pictures as before.

## Where
* `NFSU2_NO_CACHE=1`: off. `NFSU2_CACHE_DIR`: that folder, for every program.
* Otherwise `platform::dirs::cache()` (docs/playing/platform-dirs.md):
  * the shipping game (built without `dev-harness`): the player's folder, `~/.cache/overbright`
    (`<Local AppData>\Overbright\cache`);
  * the game built with `dev-harness` (shots, flows, selftest, benches, playthrough, parity and rate runs), the
    probes, and the tools in `src/bin` and the examples (they call `dirs::developer_program()` first): the
    **developer cache** `~/.cache/overbright-dev` (`$XDG_CACHE_HOME/overbright-dev`;
    `<Local AppData>\Overbright\cache-dev`; with `NFSU2_DATA_DIR`, `<dir>/cache-dev`). A developer run with no
    variable set cannot write into the player's cache;
  * tests: `test-cache` in the target folder (`dirs::test_cache_folder`; set by the test kit's `game()`, and
    used by every cargo test binary of a `dev-harness` build and by the library's own unit tests).
* The check scripts export `NFSU2_CACHE_DIR` to a folder of their run (checks.sh, perf.sh, matrix.sh,
  resolutions.sh, wine_smoke.sh, release_check.sh, the X tests). checks.sh and gate.sh also take the file list of
  the player's cache and data folders (path, type, size, time; read only) before and after the run and fail with
  "player folders" when anything there changed (`ck_player_snapshot` in scripts/lib/common.sh;
  `PLAYER_DIRS=` leaves the step out while the player plays on the same machine). One change is not counted: a
  game log (`logs/game-<start time>.log`) that was there before the run and has only grown, which is the
  player's own running game writing on; a game started by a check would make a log of its own. And while a
  game, launcher, Studio or map editor that uses the player's folders runs (a process named `game`,
  `overbright`, `overbright-studio` or `overbright-editor` whose environment has no `NFSU2_CACHE_DIR`, no
  `NFSU2_DATA_DIR` and no other `HOME` or `XDG_DATA_HOME` than the caller's, sampled every few seconds:
  `ck_player_watch`; a test's run under a fake home is not one), a change there is the
  player's own game at work: the run prints the differences and the program's path, lists "player folders" as
  a skip and does not fail on it.

## Names and versions
* A cache file is `<kind>.<build id>.nfc`. The build id is the one of the crate whose code makes the file
  (build_support/build_id.rs, each library crate's build.rs; `nfsu2::BUILD_ID` for the caches the library
  makes today): it hashes that crate's source files (`src/` without `src/bin`), `Cargo.lock`, the crate's
  enabled features and the build ids of the workspace crates it depends on, so a change anywhere on the
  file's path changes it and a change in a crate above does not (`every_input_changes_the_build_id`). Every
  cache call takes it (`CacheFile::open`, `cache::write`, `file_cached`, `TrackCache`). It is the cache's
  only version, and it is in the file's name **and** in its header (checked on open). So:
  * a build never reads a file another source tree wrote: a change of what a kind stores (a new field in the
    car solids, say) changes the build id without anyone bumping a number;
  * two builds of different sources never write the same file, so neither finds the other's entry stale and
    replaces it (the rewrite loop below).
* `FORMAT` (format.rs) is the framing's version (magic, header, section table), checked in the header.
* Files of other builds go once this build has its own file of the same kind and they have not been used
  for an hour (see Pruning); a kind this build has no file of stays until the size limit removes it.
* Before the build id was in the name, files were `<kind>.nfc`; such names are never opened now and are pruned
  like other builds' files.

## Entry kinds (audit)
| kind | file | version | in name | in header | layout check |
|---|---|---|---|---|---|
| track (world sections, renderer data) | `track-<REGION>.<build>.nfc` | build id | yes | yes (+ FORMAT) | the build id covers every source; `two_builds_never_share_a_file` |
| track with a texture pack | `track-<REGION>-pack.<build>.nfc` | build id + "pack.key" (loader version, pack files) | build id | yes | as above; pack key: docs/modding/texture-pack-cache.md |
| car solids | `file-solids-<file>-<path hash>.<build>.nfc` | build id | yes | yes | as track |
| car / shared textures | `file-textures-<file>-<path hash>.<build>.nfc` | build id | yes | yes | as track |
| part file solid index | `file-solid-index-<file>-<path hash>.<build>.nfc` | build id | yes | yes | as track |
| surface maps | `surface-maps/<key>.rg8` / `.none`, `car-<key>.*` | `surface::GENERATOR_VERSION` in the key | yes (key) | no header (size checked) | `generator_output_is_pinned_to_its_version`: the generator's output for a fixed picture pinned with the version |
| pipeline cache | `<wgpu pipeline cache key>` | wgpu's key (adapter, driver, wgpu) | yes | wgpu / driver validate | wgpu's |
| the window's format (`window-format.txt`) and the prepare run's stamp (`prepared.txt`), docs/developing/prepare.md | plain text | the stamp names the build id, the adapter and the driver | – | compared whole | `scripts/checks/prepare.sh` |
| post self-check verdict | `post-check-<hash>.<build>.txt` | build id; hash of adapter, driver, package version, shader texts, the passes and variants compared (`NFSU2_CHECK_STAGES`, `NFSU2_POST_CHECK=all`) | yes | text | `verdict_file_is_keyed_to_build_adapter_and_check`; in `cache::dir` (`NFSU2_CACHE_DIR` first): `verdict_file_lives_in_the_asset_cache_folder` |

## Writes
* A miss parses the game's files and hands the result to `cache::write`, which returns at once; the file is
  written on a background thread (`cache writer`), never on the loading or frame thread. The exit waits for
  writers at most `EXIT_FLUSH`.
* At most one writer thread per file and process (`file::submit`): a request while one runs is queued (a newer
  request replaces the queued one). A writer first takes the sources' keys and the sections' hashes; it writes
  nothing when this process wrote the same contents before, or when the file on disk already holds them (its
  header has the same build, kind, sources' keys and every section's length and hashes: another process
  finished it). A failed write is tried once a process. So a process writes a file's contents once, whatever
  number of loads missed it. `NFSU2_LOG=debug`-level lines `cache writes <file>: N written, N repeated, N already
  there, N superseded, N failed` are logged at the exit flush; every open logs hit or why it missed.

## Temporary files
* A file is written to an anonymous file (Linux `O_TMPFILE`) or to `<name>.<pid>-<hex>-<n>.<seq>.tmp`,
  locked by its writer either way, and renamed (or linked) into place. A `.tmp` name is never a cache file's
  name, so a temporary file is never read as an entry. A failed rename removes the temporary file.
* A writer killed between giving the anonymous file its temporary name and the rename leaves that name. The
  sweep (at the first open of a process and after every write; also in `surface-maps/`) removes a temporary
  file when no writer holds its lock **and** it is older than 10 minutes, or older than 30 s and its writer's
  process is gone (Linux: no `/proc/<pid>`). The lock is the authority: a live writer always holds it, and it
  goes with the process, so a reused pid only delays a removal (it reads as alive, and the 10 minutes apply);
  a writer in another pid namespace likewise. Windows: the pid is not checked (every writer there locks; age
  and lock decide), and a file another process has open is not removed.

## Pruning
* **Superseded entries** (`prune::prune_superseded`): once this build has its own file of a kind
  (`<kind>.<own id>.nfc`), every other build's file of that kind (`<kind>.<other id>.nfc`) and the old-named
  `<kind>.nfc` are removed when they have not been used for `SUPERSEDED_AFTER` (an hour: an open touches a file
  older than 10 minutes, so a build played within the hour, side by side with this one or switched back to,
  keeps its files). It runs after every write for the written kind, and once a run, on a writer thread at the
  first cache open, for every kind this build has a file of (the files it wrote in an earlier run). Whatever
  the folder's size: after an update the old build's track caches (122–139 MB each) and car files go at the
  next start instead of staying until the folder passes 2 GB. Never removed this way: this build's files,
  temporary files, a kind this build has no file of, anything that is not a plain file directly in the cache
  folder (the surface maps' folder, links), and nothing outside the cache folder. What a cache key covers is
  unchanged. The surface maps carry no build id (their generator's version is in the key): they stay with the
  size limit below (`cache::tests::superseded_entries_of_other_builds_go`).
* `NFSU2_CACHE_MAX_MB` (default 2048) after every write: temporary files count against it (never removed
  here). Other builds' files (and old-named ones) go first, then this build's, each the one **used** longest ago
  first: an open sets a file's time when it is older than 10 minutes, so eviction is by use, not by write.
* The surface maps' folder (`surface-maps/`) counts against the limit too: its files with bytes are evicted with
  this build's files by the same rule (a map read from the cache is marked as used, `cache::touch_file`); its
  empty `.none` files (a texture that gets no map: removing one frees nothing) and its temporary files stay. A
  surface map write does not prune by itself: the next cache file write does
  (`cache::tests::prune_counts_the_surface_maps`).
* Never removed: the file just written, and every file younger than 5 minutes (another process's file just
  written and not yet opened, or just opened). A file in use stays readable: on Unix its mapping keeps the
  removed inode, and Windows does not remove a mapped file. When everything left is that young the folder may
  stay over the limit until the next write.

## The rewrite loop this fixes
Two builds of different sources, one folder (a check run without its own cache folder in the player's cache):
each build's open found the other's file under the shared name `<kind>.nfc`, rejected it ("stale (build or
kind)": the header's build id) and wrote its own; the other build then did the same on its next load. The
career bot loads cars all the time, so each process rewrote the same 5–9 MB files hundreds of times (several
writer threads at once, each request its own thread), and up to 167 temporary files existed at once. With
the build id in the name the two builds have two files and each is written once. The removal of superseded
files keeps that: two builds used side by side keep both files while each was used within the hour; a build
that comes back after longer writes its files once again.

## Checks
* `cache::tests::two_builds_write_each_entry_once`: two builds' write requests for one kind from four threads
  (800 requests) against one folder: one write per build, nothing written for contents already on disk, no
  temporary file left.
* `superseded_entries_of_other_builds_go`: a sandbox folder with this build's, two other builds', an old-named,
  a temporary, a linked and a surface-map file of one kind: only the other builds' unused files and the
  old-named one of a kind this build has go.
* `two_builds_never_share_a_file`, `dead_writers_temporary_files_are_swept`, `prune_order_and_grace`,
  `temporary_files_do_not_pile_up`, `stale_and_corrupt_files_are_misses`.
* checks.sh / gate.sh: "player folders" (above).
