# Waits and frame times in the game's log

Not native: SPEED2.EXE writes no such log. Nothing drawn, heard or simulated changes: the game reads
clocks, counters and its mode, and writes lines to its own log file (docs/playing/platform-dirs.md "The log
file": Linux `$XDG_DATA_HOME/overbright/logs/game-<UTC time>.log`, normally
`~/.local/share/overbright/logs/`; Windows `<Local AppData>\Overbright\logs\`; the newest 5 runs are
kept). Nothing is sent anywhere. The lines hold only times, counts, the game's own screen, movie and
region names and the cache folder's size: no paths, profile or car names, settings or hardware.

The lines are written at Info and reach the log file even while it keeps only warnings
(`qol::log_file::fact`), so every plain game run (the shipping game, the launcher's Play) has them.
With no log file and no `RUST_LOG` showing Info (the developer harness's runs, the tests) the hooks do
nothing but note keys.

## A session

A whole session (integrated GPU, 1920x1080, Original, a warm cache; the boot screens and the title
pressed through, a Quick Race circuit with its start scene, a restart, the race quit, the career's
garage to free roam, the window closed; some menu steps left out here):

```
    0.179 [INFO game] wait: start -> first frame (boot Blank) 161 ms (data loaded 38, window 137, audio 137; the process 9 ms old at main); cache 208 files 947 MB, pipeline cache 0.1 MB, prepare stamp no
    0.243 [INFO game] wait: boot Blank -> boot EaLogo: 7 ms from the key
    2.333 [INFO game] wait: boot Intro -> menu Splash: 21 ms from the key
    6.442 [INFO game] wait: menu Splash -> menu McMessage: 6 ms from the key
    7.210 [INFO game] wait: menu McMessage -> menu Main: 318 ms from the key
   15.264 [INFO game] wait: menu CarLot -> menu Dialog: 196 ms from the key
   17.119 [INFO game] wait: menu Dialog -> race: 1016 ms from the key; load of L4RA 466 ms (world and car 20, renderers and uploads 8, set-up 12, programs 1, the rest 0, rivals 425), world loaded ahead, car loaded ahead, rivals 0 of 3 loaded ahead, cache files 0 read 20 missing, 0 programs made, loading screen not shown; first frame 18 ms
   27.251 [INFO game] wait: race -> green light 10132 ms after the race's first frame (the start scene and the countdown)
   46.386 [INFO game] wait: race -> menu RacePause: 186 ms from the key
   46.386 [INFO game] drive: race, 29 s, 2191 frames, median 12.62 ms (79 fps), 1% low 39 fps (p99 25.55 ms), hitches over 2x median: 23 (1699 ms in all, 23 in the first minute), longest 313.6 ms at 3.9 s, 157.5 ms at 11.6 s, 150.3 ms at 4.6 s
   47.893 [INFO game] wait: menu RacePause -> menu Dialog: 24 ms from the key
   49.198 [INFO game] wait: menu Dialog -> race: 553 ms from the key; load of L4RA 230 ms (set-up 9, programs 2, the rest 0, rivals 219), world kept by the drive, car kept by the drive, rivals 0 of 3 loaded ahead, cache files 12 read 8 missing, 0 programs made, loading screen not shown; first frame 308 ms
   54.107 [INFO game] wait: race -> green light 4910 ms after the race's first frame (the start scene and the countdown)
   64.286 [INFO game] wait: race -> menu RacePause: 22 ms from the key
   64.286 [INFO game] drive: race, 15 s, 1051 frames, median 13.90 ms (72 fps), 1% low 41 fps (p99 24.68 ms), hitches over 2x median: 5 (199 ms in all, 5 in the first minute), longest 71.4 ms at 0.6 s, 42.4 ms at 0.9 s, 29.2 ms at 7.0 s
   75.246 [INFO game] wait: menu Crib -> menu Dialog: 28 ms from the key
   76.387 [INFO game] wait: menu Dialog -> free roam: 294 ms from the key; load of L4RA 13 ms (world and car 11, renderers and uploads 0, set-up 1, programs 1, the rest 0), world kept, car loaded ahead, cache files 10 read 10 missing, 0 programs made, loading screen not shown; first frame 256 ms
   88.782 [INFO game] wait: free roam -> menu ExplorePause: 21 ms from the key
   88.782 [INFO game] drive: free roam, 12 s, 708 frames, median 15.99 ms (63 fps), 1% low 33 fps (p99 30.33 ms), hitches over 2x median: 5 (240 ms in all, 5 in the first minute), longest 105.1 ms at 4.0 s, 34.7 ms at 0.6 s, 34.3 ms at 2.6 s
   89.926 [INFO game] wait: the game ran 90 s
```

Read here: the race's 1016 ms from the key hold a 466 ms load, of which 425 ms are the rivals' cars
(`rivals 0 of 3 loaded ahead`: none was taken from the menus' loads ahead, and 20 cache files were
missing: cars this cache had not seen), an 18 ms first frame, and 532 ms before the load began (the
old screen's frames and the menus' steps: not split further). The restart's 553 ms are the rivals
again (219 ms) and a 308 ms first frame. The green light came 10.1 s after the race's first frame with
the start scene played out, 4.9 s with it skipped.

## The lines

* **`wait: start -> first frame (<screen>) N ms`**, once, after the first frame the game draws itself
  (the boot's first movie, or the main menu with `--no-boot`; the game start's PC_Loading, shown only
  past 200 ms, comes before it): ms from `main` (`qol::waits::started`) to the end of that frame's
  drawing. In brackets the parts, each in ms from `main`: the game's data read (`Game::new`; the
  window's event loop is made beside its loaders, docs/original/start-and-menus.md), the window made (its
  graphics device, opened on a thread since the options were in force, taken or waited for here), the
  audio device opened (asked for since the start); then how old the process was when `main` began (the program's
  loading by the system; on Linux it includes the start's own re-exec for glibc's huge pages,
  `shell::startup`; Linux 10 ms steps, Windows the system clock's; left out where the system does not
  say). After the `;` the cache folder as the start found it, read on a thread of its own after the
  first frame: its files and their size, the graphics driver's pipeline cache kept there, and whether
  the launcher's prepare step's stamp is there (docs/developing/prepare.md). A few files mean a new cache: every
  load of that run builds its caches.
* **`wait: <from> -> <to>: N ms from the key`**, at the end of the first frame of every new screen (the
  mode; the menu, its kind, a boot movie's step, a replay), and of a drive loaded again on the same
  screen (`free roam -> free roam`). "from the key": since the last key, mouse button or controller
  button pressed (keys and the mouse as the window receives them, a controller's buttons as the
  game's tick reads them: up to one tick later). A key older than 20 s asked for nothing that comes now:
  "no key" (a movie or a scene that ended by itself). Screen names: `boot <step>`, `movie <name>`,
  `menu <kind or screen>`, `free roam`, `race`, `replay`.
  A drive's load (`drive_in`: free roam, a race, a test run) adds, after `;`:
  * `load of <region> N ms (…)`: the load's own time and its parts in ms: the world and the car (read,
    or taken from the menus' loads ahead, PC_Loading ticked meanwhile), the renderers and the uploads,
    the set-up (the old drive dropped, the HUD's textures, the barriers and placements), the drive's
    programs made behind the loading screen (docs/developing/first-use-builds.md), the rest of `drive_in`, and for
    a race the rivals' cars (`line_up_rivals`, after the load: taken from the menus' loads ahead, kept
    from the race before or read; with `rivals N of M loaded ahead` after the car, and `, K kept from the
    race before` when a Restart or a Next Race took K of the race before's cars). A restart with the same
    car keeps the drive: only the set-up, the programs and the rest (and the rivals);
  * where the world and the car came from: `kept` (in memory from the drive before), `loaded ahead`
    (by the menus, done), `loading ahead (waited for)` (the menus' load still running: the load
    waited for it), `loaded` (read in the load), `kept by the drive` (the same region and car: no
    load of either);
  * `cache files N read M missing`: the cache files opened from the load's start to the end of the
    first frame, on any thread (the traffic's and the encounter racers' cars loading meanwhile
    included), valid and missing or stale (missing ones are built from the game files and written
    back);
  * `N programs made`: pipelines built over the same time (0 when the menus made them ahead);
  * whether PC_Loading went up (only past 200 ms, docs/original/loading-image.md);
  * `first frame N ms`: from the load's last part to the end of the new screen's first frame: the rest
    of the tick the load ran in (a race's traffic and its set-up after the rivals, free roam's) and
    the first frame's own work (what it made on first use, its drawing).
  The wait a player saw is the "from the key" figure; the load and the first frame are its parts, the
  rest is the frames of the old screen before the load and the menus' own steps.
* **`wait: race -> green light N ms after the race's first frame (…)`**: the race's first frame to its
  clock passing 0: the start scene and the countdown, the original's own wait (Enter skips the scene).
  Time in the pause menu is left out and named; a restart starts it again; a race entered after its
  green light (back from the pause menu) has none.
* **`drive: <free roam | race>, …`**, when a drive's frames end (another screen, the pause menu
  included, or the game's end): seconds, frames, the median frame time, the 1 % low as the F3 overlay
  counts it (1000 / the 99th percentile frame time, index ⌊0.99 n⌋: MangoHud's reading, docs/notes/performance/overlay.md;
  "1 % of the frames were slower than this"), the hitches (frames over twice the median and over 4 ms) with their sum and how many
  fell in the first minute, and the three longest frames with when they ended (s from the drive's
  start). A frame is the interval from one window frame's start to the next's (the display's waits
  included). Not counted: the drive's first frame and the one after it (the load ran in it: it is the
  `wait:` line's), and frames while the window is in the background (unfocused or hidden) together with
  the first one back. Under 10 frames: no line. A drive longer than 262,144 frames (14.6 min at
  300 fps) is written in parts of that many (`(part N)`).
* **`wait: the game ran N s`**, at the window's exit.

## Reading a session

1. The newest `game-*.log` is the last run. `grep -E 'wait:|drive:'` gives the session.
2. Waits: about 0.1 s is the limit of feeling instant, about 1 s the limit of keeping the player's
   flow of thought (the usual response-time limits). A long "from the key" with a short load and a
   long "first frame" is the first frame's work (a new placement set, programs, uploads); a long load
   part names its step; `loaded` or `loading ahead (waited for)` where `loaded ahead` was expected is
   a missed load ahead; cache files missing are a first run, a changed game folder or build, or cars
   loaded for the first time (the traffic and encounter racers are drawn anew each run).
3. Drives: the median and the 1 % low against the display's refresh (vsync on: the median sits at the
   refresh period); hitches in the first minute after a load are streaming or first use, later ones
   are the game or the machine. Compare with an outside tool (MangoHud's log, PresentMon) the same way
   (its "1 % low" may be the mean of the slowest 1 %, which reads lower).
4. The `wait: start` line's cache state and the process's age tell a first start (new cache, cold
   disk) from a later one.

## Bounds, cost

* At most 20,000 such lines a run (`qol::waits::MAX_LINES`; a session writes a few hundred), then one
  line saying so: whatever a screen does, the file stays bounded (a few MB). The log's thread
  writes them; a failed write (a full disk) is dropped, never retried or kept (`qol::log_file`).
* Memory: 4 bytes per counted frame of the drive on screen, at most 1 MB (a part); freed at the
  drive's end. A summary is O(frames) (two selections and one pass), on the main thread at the
  drive's end or a part's: 0.05 ms for 10,000 frames, 0.5 ms for 100,000, 1.2 ms for a whole part.
* Per frame: the screen's discriminants compared, one `f32` pushed while driving; per screen change
  one line formatted. At the exit, after the window is hidden, at most 0.2 s for the log's thread to
  write the last lines. Measured: docs/evidence/perf-budget/waits-log-cost.txt.

## Code

`crates/ob-settings/src/qol/waits.rs` (the lines' logic, pure and unit-tested: `Tracker`, `Frames`, `summarize`, `Load`,
`cache_state`), `src/bin/game/shell/waits.rs` (the game's hooks: `Game::waits_frame` from `redraw`,
`waits_end` from `exit`), the load's parts in `loads/drive_in.rs`, the start's in `shell/cli.rs` and
`shell/window.rs`, the rivals' in `flow/race_setup.rs`, the keys in `shell/window.rs`'s event handler and
`input/pads.rs`, the cache counters in `cache/file.rs` (`open_counts`).

Open: a screen change inside one menu kind (a dialog's text, a list's page) is not a new screen; the
menus' loads ahead (their own times) and free roam's encounter racers (they join during the drive) are
not in the load's line; the frame times are the game's own frame starts, not the display's presents.
