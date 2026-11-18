# Where Overbright keeps its files, finding the game, the log file

`src/platform/` (and the log file). No native counterpart except where an address is given.

## Folders (`platform::dirs`)

One module builds every path Overbright writes. `platform::dirs::resolve(os, env, known_folder)` is a pure
function, so the Windows rules are unit-tested on Linux.

| Folder | Linux / Unix | Windows |
|---|---|---|
| data home (the profiles' parent) | `$XDG_DATA_HOME`, else `$HOME/.local/share`, else `.` | `<Local AppData>\Overbright` |
| profiles | `<data home>/NFS Underground 2/<NAME>/<NAME>` | `<Local AppData>\Overbright\NFS Underground 2\<NAME>\<NAME>` |
| Overbright's folder (`app`, `config`: options.txt, game-root.txt) | `<data home>/overbright` | `<Local AppData>\Overbright` |
| crash reports / logs / marks / perf CSVs / screenshots / texture pack | `<app>/crashes`, `logs`, `marks`, `perf`, `Screenshots`, `texture-pack` | the same under `<app>` |
| asset cache (`cache`), the player's game | `$XDG_CACHE_HOME/overbright` (set, not empty), else `$HOME/.cache/overbright` | `<Local AppData>\Overbright\cache` |
| asset cache, every other program (developer cache) | `$XDG_CACHE_HOME/overbright-dev`, else `$HOME/.cache/overbright-dev` | `<Local AppData>\Overbright\cache-dev` |

- The name is `dirs::APP` (`overbright`); Windows' own folder is `dirs::WINDOWS_APP` (`Overbright`, unchanged).
  The original's profile folder (`NFS Underground 2/<NAME>/<NAME>`) is the original's own name and is not renamed.
- `<Local AppData>` is `SHGetKnownFolderPath(FOLDERID_LocalAppData)`, `%LOCALAPPDATA%` when that fails.
- Windows: Overbright never writes into the original's save folder. The original keeps its profiles in
  `%LOCALAPPDATA%\NFS Underground 2` (0x4A9C40, SHGetFolderPathA CSIDL_LOCAL_APPDATA); Overbright's copy of that
  layout is one level down, inside its own folder.
- Overrides, every OS:
  - `NFSU2_DATA_DIR` (set, not empty): everything in that one folder (profiles' folder, Overbright's files flat,
    `cache/`). Portable mode ("Portable mode" below) uses the same layout in `data/` beside the programs.
  - `XDG_DATA_HOME` / `XDG_CACHE_HOME` (on Windows: set and not empty) keep Linux's layout. The developer harness
    isolates its runs through `XDG_DATA_HOME` (`--shot`, `--selftest`, `--flow`, `--bench`, `--playthrough`;
    `scripts/checks/*.sh`), so it works the same on Windows.
  - The cache's own switches come first: `NFSU2_NO_CACHE`, `cache::override_dir` (tests), `NFSU2_CACHE_DIR`.
  - **The player's cache folder is the shipping game's alone.** `dirs::cache()` gives the player's folder only to
    a build without `dev-harness`. The game built with `dev-harness` (shots, flows, the selftest, benches, the
    playthrough, the parity and rate runs) and the probes default to the developer cache (`dirs::developer_cache`:
    `overbright-dev` / `cache-dev`; with `NFSU2_DATA_DIR`, `<dir>/cache-dev`), and so do the tools in `src/bin` and
    the examples, which call `dirs::developer_program()` first. So a developer run without `NFSU2_CACHE_DIR` cannot
    write into a player's cache. Tests default to `test-cache` in the target folder (`dirs::test_cache_folder`: the
    test kit's `game()` sets it; every cargo test binary of a harness build and the library's unit tests use it).
    `NFSU2_CACHE_DIR` still comes first everywhere; the check scripts set it to a folder of the run (docs/testing.md).
- The post self-check's verdict (`render/native/post/fused.rs`) is kept in the cache's default folder
  (`dirs::cache()`, not `NFSU2_CACHE_DIR`, as before; so a developer build keeps it in the developer cache). Before it read `XDG_CACHE_HOME` even when empty
  and `%LOCALAPPDATA%` / `~/Library/Caches` on its own; those edge cases now follow the cache's rule.
- Crash reports, Mark This Spot reports and the log file show the home folder as `~`: `$HOME` and `%USERPROFILE%`.
  Mark's command lines write `$HOME` / `%USERPROFILE%`.

The test kit's fixture cache is `<cache home>/overbright-test-fixtures`, beside the player's cache rather than
inside it.

## Portable mode

A file `portable.txt` or a folder `portable` beside the running executable (`dirs::is_portable`,
`dirs::portable_base`; looked up once per process, `dirs::refresh_portable` looks again) makes the install
portable: `dirs::layout` is `resolve_portable(…, Some(<program folder>))`, which lays everything out in
`<program folder>/data` as `NFSU2_DATA_DIR` does (data home, Overbright's folder flat, `cache/`; the
developer cache `cache-dev/`). `NFSU2_DATA_DIR` still comes first; the cache's own switches still decide the
cache. Without the marker every function returns what it did before, byte for byte (`resolve` is
`resolve_portable(…, None)`; the tests compare them).

- **Kept paths.** In a portable install the game folder (`game-root.txt`) and the HUD mod folder
  (`texture-pack-root.txt`) are written relative to the program folder when both are on the same drive
  (`dirs::store_path`: the same drive letter or share on Windows, compared without case; the same file system
  elsewhere), e.g. `../Games/Need for Speed Underground 2`, else absolute. A relative path read back is taken
  from the program folder (`dirs::load_path`, `..` applied to the text), not from the current folder. Outside
  portable mode both are the identity.
- **The driver's shader cache** (Linux and other Unix): the game sets `MESA_SHADER_CACHE_DIR` and
  `__GL_SHADER_DISK_CACHE_PATH` to `data/cache/driver` before any thread starts, unless the environment sets
  them (`dirs::driver_cache_vars`). Windows' drivers keep theirs where they decide (outside Overbright's control).
- **The developer harness**: its isolated runs (shots, flows, the selftest, the playthrough) call
  `dirs::ignore_portable`, so they keep their data in the folder they chose even in a portable install.
- **Turning it on**: the launcher's About page (`dirs::make_portable`: writes `portable.txt`, nothing else) or
  the file by hand. Nothing is migrated: copy Overbright's folder (and on Linux `~/.local/share/NFS Underground
  2`, into `data/NFS Underground 2`) to take settings and saves along; the cache rebuilds itself.

### Everything Overbright reads or writes outside its own folder

| What | Where (default) | Portable | Code |
|---|---|---|---|
| options.txt, game-root.txt, texture-pack-root.txt, texture-mods.txt | `<app>` | `data/` | `dirs::config`, `qol::setup`, `texture_mods::list_file` |
| profiles, backups | `<data home>/NFS Underground 2[ Backups]` | `data/NFS Underground 2[ Backups]` | `frontend::profiles::root`, `dirs::data_home` |
| asset cache, pipeline cache (`wgpu_pipeline_cache_*`), surface maps, the post self-check's verdict | `dirs::cache` | `data/cache/` (`data/cache-dev/` for developer builds) | `cache::dir`, `render::core::pipeline_cache` |
| logs (game and launcher), crash reports, marks, perf CSVs, screenshots | `<app>/logs`, `crashes`, `marks`, `perf`, `Screenshots` | under `data/` | `dirs::logs` … `dirs::screenshots` |
| texture packs, imported mods, the converted HUD mod | `<app>/texture-pack` | `data/texture-pack/` | `dirs::packs`, `texture_mods`, `hud_mod` |
| launcher.lock | `<app>` | `data/` | `launcher::dirs::lock_file` |
| driver shader cache | the driver's (`~/.cache/mesa_shader_cache*`, `%LOCALAPPDATA%\<vendor>\…`) | Linux: `data/cache/driver`; Windows: unchanged | `dirs::driver_cache_vars` |
| read only: the registry's `Install Dir`, `%ProgramFiles%`, the Wine/Steam/Lutris places | — | unchanged (only read) | `qol::setup::candidates`, `platform::win` |
| read only: the original's saves (`%LOCALAPPDATA%\NFS Underground 2`, the import) and the home folder (the browser) | — | unchanged (only read) | `launcher::dirs::original_saves`, `home` |
| temporary folders | the system's temp folder | tests and the developer harness only | — |

Tests: `platform::dirs` unit tests (the layout on both OSes, the marker, kept paths, the driver variables) and
`tests/portable_mode.rs` (the game binary linked into a program folder with the marker and run under a fake
HOME / LOCALAPPDATA / APPDATA / TMPDIR: nothing written outside `data/`; `--set-root` kept relative and found
again from another folder; without the marker the old place and bytes).

`qol::setup::save_root` / `save_texture_pack` apply `dirs::store_path` and `saved_root` / `saved_texture_pack`
apply `dirs::load_path` themselves, so `setup::find` (the kept folder first) and every caller get the resolved
folder. **Open:** not checked on a real Windows machine or a removable drive.

## Paths (`platform::paths`)

`absolute(p)` (`--set-root`): Unix `canonicalize` as before. Windows: `canonicalize` returns a verbatim
`\\?\C:\…` path, which takes no `/`, so `root.join("GLOBAL/GLOBALB.BUN")` would fail; the prefix is stripped when
the plain path names the same file (`strip_verbatim`: drive and UNC forms, under 260 characters, no component
Win32 would rewrite), else `std::path::absolute`. **[unverified on Windows]**

## Finding the original on Windows (`qol::setup::candidates`)

`setup::find` order is unchanged: `NFSU2_ROOT`, the kept `game-root.txt`, the current folder, then the candidates;
the first that looks like the game wins. Windows candidates (`windows_candidates`, in order):

1. the registry's `Install Dir`, read from HKLM's 32-bit view (`WOW6432Node`), HKLM's 64-bit view, then HKCU;
2. `%ProgramFiles(x86)%\EA GAMES\Need for Speed Underground 2`, `%ProgramFiles%\EA GAMES\…`;
3. Overbright's exe folder and its parent (unpacked into or next to the install).

Traced in SPEED2.EXE: 0x5B7700 calls RegOpenKeyExA (IAT 0x783008) with
`HKEY_LOCAL_MACHINE` (0x80000002), `Software\EA Games\Need for Speed Underground 2` (0x7A30F0), KEY_READ, then
RegQueryValueExA (IAT 0x783004) for `Install Dir` (0x7A30E4). The same key is opened
from HKLM at 0x5BEA30 (the options' registry values). **Unverified:** the view a real install wrote (the exe is
32-bit, so on 64-bit Windows its HKLM reads land in `WOW6432Node` [gen]), and whether any installer or repack
wrote HKCU. That is why each value is only a candidate checked like any other folder. The Windows code
(`src/platform/win.rs`) is declared by hand against the documented signatures (no new dependency) and has not
been compiled for Windows yet.

`cfg(windows)` items: `platform::win` (`local_app_data`, `registry_install_dirs`, the extern blocks for
shell32 `SHGetKnownFolderPath`, ole32 `CoTaskMemFree`, advapi32 `RegGetValueW`), `platform::dirs::layout`'s known
folder call, `platform::paths::absolute`'s Windows branch, `qol::setup::candidates`' Windows branch.

## The log file (`qol::log_file`)

A plain game run (no arguments but `--no-boot`) also writes `<app>/logs/game-<UTC time>.log` and keeps the newest
5 (the oldest `game-*.log` are deleted before the new one is made). Lines: warnings and errors always, plus what
`RUST_LOG` shows (the crash report's ring keeps the same lines), each with the seconds since start; a panic's
message and call stack are added and flushed (at most 0.5 s wait) before the default hook ends the thread. One
`[INFO game] display: …` line is always written at the window's creation (the adapter and driver, the swap chain's
format, colour space, present mode and size, and the formats the surface offers), whatever the log level, and
so are the `wait:` and `drive:` lines (what the player waited for and the drives' frame times, docs/waits-log.md). A thread of its own
(`log-file`) does all file work; the logging threads only hand a line to a channel. Home folders are scrubbed to
`~`. Not written: the developer harness's runs, `--check-game`, `--set-root`, `--verify-cache`, `--crash-test`,
the tests, or with `NFSU2_LOG_FILE=0`.

## The game's files, any case (`game_fs`)

SPEED2.EXE opens its files through Win32 (case-blind on NTFS) and asks for upper-case names
(`LANGUAGES\ENGLISH.BIN`); installs spell them in many ways (the reference one: `LANGUAGES/English.bin`,
`GLOBAL/GlobalB.lzc`). Every game-file path the library builds goes through `game_fs`:

- `game_fs::path(root, "GLOBAL/GlobalB.lzc")` → the real path; `find` → `Option`; `read` / `read_to_string`.
  `/` and `\` both separate.
- The exact spelling first: when it exists the result is `root.join(rel)` itself (`read` just opens it, no
  extra call). Otherwise each component is matched without ASCII case against its directory's listing; a
  directory is listed once per process and kept (the game folder is never written). Two names differing only
  in case: the exact one, else the smallest. No match: the literal `root.join(rel)`, so errors name the file as
  asked for.
- Never outside `root`: a `..`, `.` or empty component, a leading separator or a `:` (drive, prefix) is refused
  (`path` then names a file that cannot exist under `root` and logs a warning).
- `qol::setup::check` / `looks_like_game`, `fingerprint::compute` and `viv::find_ci` use it, so a complete install
  spelled in any case passes the game-folder check. The game binary's own sites are listed for a later step.

## Replacing our own files on Windows (`platform::fs`)

Saves, options.txt, the asset cache and the pipeline cache are written to a temporary file and renamed over the
old one. Unix: a plain `rename` (unchanged). Windows: `rename_replace` retries for up to 500 ms (1, 2, 4 … 64 ms
steps) while the target is held by someone else (`ERROR_SHARING_VIOLATION` 32, `ERROR_LOCK_VIOLATION` 33,
`ERROR_ACCESS_DENIED` 5: antivirus, cloud sync, the original reading a profile). A cache file still mapped by a
`CacheFile` in use (the "retimed" rewrite) cannot be replaced at all; `rename_replace_aside` then renames it to a
temporary name (`<name>.<tag>.<seq>.tmp`, which the stale-file sweep removes once nothing maps it) and puts the new
file in its place; the mapping keeps reading the old data. Windows has no directory fsync: a durable write gets the
file's `sync_all` (FlushFileBuffers) only. The `cfg(windows)` tests (`platform/fs/tests.rs`) run under Wine in the
container build; Wine does not model every NTFS rule (see the Windows file-replacement report), so the proof is a real Windows machine.
