# Troubleshooting and bug reports

When Overbright does not start, shows a black screen or stops, a **support file** collects everything a
maintainer needs in one text file. Attach it to your report at
[github.com/j0braun/overbright/issues](https://github.com/j0braun/overbright/issues). Nothing is ever sent
anywhere by Overbright itself.

## Making the support file

Pick the first way that works for you:

1. **In the game:** press **Ctrl+F8** (on a black screen too). The file goes to the logs folder (below) and
   the log names it. Ctrl and F8 still reach the game, so pause first if Ctrl is bound to an action.
2. **From the command line, also when the launcher or the game closes at once** (it never shows a window, or the window vanishes):
   start the game program with `--support-file`. It opens no window and needs no working graphics driver.
   - Windows: press Win+R, type the full path of `game.exe` (in the folder Overbright is in) followed by
     `--support-file` and press Enter, for example `"D:\overbright\game.exe" --support-file`. A message box
     names the file and Explorer opens with it selected.
   - Linux: `./game --support-file` in a terminal in Overbright's folder; the path is printed.
   - A folder after the flag writes the file there: `game --support-file D:\reports`.

The file is named `overbright-support-<UTC date>-<time>.txt`. It is written to the logs folder; when that
folder cannot be written, next to the program, then to the temporary folder. The newest five are kept.

### What the file holds

- the version, the system (Windows version and build, or the Linux distribution, kernel and session), the
  processors and memory;
- Overbright's folders, whether they exist, portable mode, and the environment variables that change
  Overbright, wgpu, Vulkan, Mesa, DXVK or Wine;
- the game folder's check: each file Overbright needs with its size, the SHA-256 of the key files, the edition
  verdict and the mod files found (the same as `game --check-game`);
- the graphics adapters and drivers and the one the game picks (`game --list-adapters`), and the launcher's
  hardware check (`game --probe`), each asked in a process of its own with a 20 s limit: a driver that crashes
  or hangs while listing shows up as that (with its exit code, e.g. `0xc0000005 (access violation)`);
- made in the game (Ctrl+F8): the graphics adapter and the audio output in use (the sound device, its sample
  rate, its channel count and layout, e.g. `6 channels (5.1 …)`, and the sample format); the game's log has the
  same `audio:` line from each time the output opens;
- on Windows, the Application event log's recent crash entries for `overbright.exe` and `game.exe` (the
  faulting module and the exception code of a start that ended without a word);
- the settings files (options.txt, the kept game and texture-pack folders);
- the logs of the last starts: `launcher.log`, `launcher.old.log`, the newest three `launcher-*.log` and
  `game-*.log`, the newest two crash reports, and in the game the recent events (below).

Home folders are written as `~` and user names in paths as `<user>`. The file holds no saves and no game
files.

## Where the logs are

| File | Windows | Linux |
|---|---|---|
| Launcher log (from its first line; the run before as `launcher.old.log`) | `%LOCALAPPDATA%\Overbright\launcher.log` | `~/.local/share/overbright/launcher.log` |
| Game logs (the newest five) and the launcher's per-start logs | `%LOCALAPPDATA%\Overbright\logs\` | `~/.local/share/overbright/logs/` |
| Crash reports (Enhancements: Crash Reports) | `%LOCALAPPDATA%\Overbright\crashes\` | `~/.local/share/overbright/crashes/` |

In portable mode (a `portable.txt` beside the programs) all of these are in the `data` folder beside the
programs; `NFSU2_DATA_DIR` moves them too (docs/playing/platform-dirs.md).

## The game's recent events

The game keeps its last 256 events in memory: the start, the GPU adapters tried and the one chosen, the window
(made, its size, full screen or windowed, focus, hidden, resized, scale), the swap chain (configured; an image
that did not come: timeout, occluded, outdated, lost), the first image shown, every screen and load, frames
longer than 0.25 s and errors. A repeat of the same event is counted, not stored again. The list goes to the
game's log:

- on Ctrl+F8;
- on a panic (with the panic's message and call stack);
- when the main thread has not turned for 10 s (a hang, or a very long load), or no frame was drawn for 10 s
  while the window is shown; the end of such a stall is an event too;
- when the GPU device is lost (a driver reset, a removed GPU);
- when the window cannot be made.

Reading it for a black screen: `images shown 0` and `no image: …` lines mean the display never took a frame
(a driver, compositor or full-screen problem: try windowed, update the driver); images shown with a screen
that stays the same mean the game is drawing a black picture (report the screen named in the list); a
`stall` line means the game stopped working.

On Windows a crash that is no Rust panic (an access violation in a driver or an overlay that injects itself)
adds one line to the game's log, `native crash: exception 0x… (…) at <module>+0x…`, before Windows ends the
process. A stack overflow or an abort skips that line; Windows records it in the Application event log, which
the support file reads.

## What the game check says

The launcher checks the game folder you choose (the **Game folder** screen) and shows one verdict. Its title
is one of these:

| What the screen says | What it means | What to do |
|---|---|---|
| Ready to play | Every file the game needs is there and is the original release (version 1.2). | Use this folder. |
| Ready, some movies won't play | Some movies are not in the MOVIES folder. The game skips them, as the original does. | Use this folder; Details lists which ones. |
| Ready, some menu files are missing | A menu file is missing; some menus may not load. | Use this folder; if a menu fails, save a support file. |
| Ready, with changed game files | Some game files are not the original ones (a mod or another release). The game may look or play differently. | Use this folder; attach a support file to any report. |
| This copy isn't recognised | None of the game files match the release Overbright knows. Overbright asks before it uses the folder. | Choose another folder, or Use it anyway; a support file helps add the release. |
| The game's text file is missing | The copy has no `LANGUAGES/English.bin` (for example a copy with another language's text only). The game reads its menus and text from that file and does not start without it. | Choose a copy that has the file. |
| Some game files are missing | Files the game needs are not in the folder (an install or a copy stopped early). | Reinstall or copy the whole folder again, then Check again. |
| No game here | The folder is not the game's. | Choose the folder with GLOBAL, FRONTEND and TRACKS in it. |
| This is Overbright's folder | Overbright's own folder was chosen. | Choose the folder the game was installed to. |
| Overbright can't open this folder | Your account cannot read the folder. | Choose a folder you can open, or copy the game to one. |

The **Details** button lists the missing and changed files by name and the movies per group. The full list
(sizes, checksums, the file each movie plays from) is in `game --check-game` and in the support file.

## What players see when a start fails

- On Windows, the launcher and the game show a message box when they stop with an error and no terminal sees
  their output; the launcher's names its log.
- A game start that fails writes the reason to its log as an `[ERROR …]` line (no usable game folder, no
  window, an error that ended the game), and the launcher's failure screen shows the game's last output.

## For maintainers: reading a report

1. `summary` and `what happened`: who wrote the file and why.
2. `game folder`: a missing or unknown file explains most failed starts.
3. `graphics adapters` and `hardware check`: no adapter, a software adapter (llvmpipe, Microsoft Basic
   Render Driver) or a failed listing explain most black screens and crashes at the window.
4. `Windows crash events`: the faulting module of a start that ended at once (an overlay DLL, a graphics
   driver, a missing runtime: `0xc0000135`).
5. `logs`: `launcher.log` says how far the launcher came (a log without its last line `end: the launcher closed
   normally` stopped hard); the game's log has the events.

Code: `src/diag/` (`support.rs`, `events.rs`, `native_crash.rs`), the game's hooks in
`src/bin/game/shell/{startup,window,frame_loop,waits,support}.rs`, `src/bin/game/render/frame/mod.rs` and
`src/render/core/gpu.rs`.
