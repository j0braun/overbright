# Reading the next minutes' files ahead (`cache::read_ahead`)

Not native: SPEED2.EXE reads its files when a load asks for them. Nothing here changes what is loaded,
drawn, heard or simulated: the loads read the same bytes from the same files; only where those bytes
come from changes (the system's file cache instead of the disk). `NFSU2_READ_AHEAD=0` turns it off.

## Why

After a reboot the game's files and the cache folder's are on the disk only. The waits that read them
then wait for the disk, one piece at a time: a movie's `std::fs::read` when it opens (also when Enter
skips it), a platform's file in the main menu's or the garage's first frame, the start's packages, the
pages of a mapped file touched one fault at a time. In the shipping game's real flow (the title's native
4 s hold included; measurements below) the menus' work ahead (`Preload`) already reads the city's track
cache and the player's car during the boot screens and the title, so what is left cold is the start
itself and the files each screen reads in its own first frame.

## What is read, in which order

One thread at the lowest priority (Linux: nice 19 and the idle I/O class, which the BFQ scheduler
honours; Windows: background mode, which lowers its CPU, I/O and memory priority) reads the queued files
from start to end in pieces of 512 KB, in the order queued, each file once a run. The list is queued
once, after the window's first frame (`Game::read_ahead_files`, `Game::read_ahead_after_start`):

1. the boot screens' movies still to come (`flow::movies::BootStep`; `--no-boot` has none), each read
   whole by the frame that opens it, also when Enter skips the one before;
2. the main menu's FE car (`Game::main_menu_car`) and its platform (FRONTEND/PLATFORMS/Showroom.BIN,
   `garage::platform_file`), which the main menu's first frame loads; a car's files are
   `car::read_ahead_files`: its GEOMETRY.BIN, the cache files of its solids and textures (its
   TEXTURES.BIN without one), its part animations, the cache files of the shared car, rim and GLOBALB
   texture packs;
3. with a career, the garage's platform (Crib01.BIN), which the garage's first frame loads;
4. what free roam's first frames read: its routes (RoutesFreeRoam.bin, PathsFreeRoam.bin), its minimap
   (TRACKMAP4000.BIN) and the drive's sound banks (`audio::banks::cache::DRIVE_BANKS`).

Not read ahead (each measured or reasoned below): the start's own files (`Game::new` reads its packages
side by side already; a reader beside it made it no faster after a reboot, 304 → 395 ms median, the
spreads overlapping); free roam's city and car (the menus' `Preload` reads them from the first frame on:
in every measured run they were in memory by the time the reader got to them, and on a first start the
reader would only read the track's game files a second time beside the parse); the career's other garage
slots' cars (only the garage's car select shows them); the rims' and body kits' shared part files (a car
load reads only the pieces its parts use); the encounter racers' and rivals' cars (drawn when the drive
starts; the menus predict them); a race's track (the player picks it).

## With the asset layer

A queued game file that the mounted base set holds (`game_fs::peek`, which counts no read) is read from its
bundle's mapping instead of the game folder: pieces of 512 KB, a piece already in memory skipped (`mincore`), the
others asked for in advance (`madvise` WILLNEED) and touched a page at a time, under the same giving way. A name the
set knows is absent is counted as not there. Nothing is opened in the game folder, so it works with the folder gone.

## Giving way

The reader never competes with a load the player waits for. Between two pieces it waits while
* a `read_ahead::Hold` is held: `Game::drive_in` holds one over every drive's load (free roam, a race,
  a test run);
* the frame on screen has run for longer than `BUSY_FRAME` (50 ms, measured from the frame's start after
  the frame cap's wait): a frame that loads something (a garage visit's first tick, a movie's open)
  reads alone;
and after a piece that took longer than `SLOW_PIECE` (20 ms: a busy disk) it pauses as long as the piece
took (at most 0.5 s), so it never holds more than half of a slow disk's time.
A file whose pages are all in memory (Linux: `mincore` over a mapping of it) is not read; elsewhere a
file in memory is read from memory (a copy, no disk access). The queue's end is logged
(`read ahead: queue done … ms after the first: N files, X MB read, Y MB in memory already`).

## Measured

After a reboot, interleaved off / on (docs/notes/performance/budget.md "A start after a reboot, the real flow";
tables in `docs/evidence/perf-budget/read-ahead.txt`): the boot movies' opens on Enter 80–306 → 16–18 ms
median, the main menu's first frame 180–359 → 48–94 ms, the garage's 70–107 → 34–44 ms, free roam's entry
within the spread; the start itself is unchanged (the reader starts after its first frame); with the files
in memory nothing gets slower. With the disk saturated by other programs nothing moved beyond the spread.

## Checks

`cache::read_ahead::tests`: a file read whole whatever its size, a missing one skipped; pages in memory
not read again (Linux); the reader gives way to holds and to a long frame and goes on after; queued files
read once each, in order, by one thread that ends with the queue. `tests/read_ahead_files.rs` (the game
files): a car's list names its game files with an empty cache and, once it was loaded, the cache files the
load wrote (`cache::file_cached_entry` and `file_cached` share `cache::file_kind`).
