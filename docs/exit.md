# The game's exit (2026-11-12)

## The original (**traced**, SPEED2.EXE)

WM_CLOSE (Win_WndProc 0x5CCD70 → 0x5CCE6C) sets the quit flag 0x864F48 and returns 0 — ignored
while PC_Loading is up (0x4A9330) or when the flag is already set. The main loop 0x5811D0 tests it
at 0x5811E4 / 0x581572 and returns; WinMain (0x580E80) calls 0x5CF970 — the audio stopped
(0x481080, 0x4839C0), the render and D3D objects released, 0x57D960 waiting for the async file
queue 0x86510C — then exit(0) (0x580E8C). The other setters of 0x864F48 (0x4B962D, 0x4C57AC,
0x4F016E, 0x4F440B, 0x55E00D, 0x5BF1F9) are the front end's quits (Quit Game) reaching the same
end. **Nothing is saved at quit**: the profile and the options are written at the save points and
when changed. The clone does the same (`Profile::commit` / options writes on change), so its exit
has nothing to write either. **Not ported**: the close ignored while PC_Loading is up (the
clone's window closes; nothing of the game runs for it).

## What the clone's exit waited for (measured, docs/evidence/exit/)

The shipping game took 3–8 s here (20–30 s, once over 30 s, on a loaded machine) from the
window's close to its end:

1. `cache::flush` at the end of `startup::main` joined every cache writer ("a first run's cache
   files are finished"), and each writer ended its file with `sync_all`: on a busy disk an fsync
   waits for the journal (`jbd2_log_wait_commit`), uninterruptibly — the process cannot end before
   it returns. A first run (an empty cache, as `tests/launcher_fresh.sh`) writes hundreds of MB.
2. The audio device's close (`Audio` dropped: the stream's thread joined): 2.7–3.0 s in some runs.
3. The rest (GPU teardown, the window, the threads' pools): 14–454 ms.

## Now

* Cache files are not synced (`cache::file::write_now`): every section is checked against its
  chunk hashes when a file is opened (`sections_intact`), so a file the system lost part of in a
  crash is found corrupt and made again; it is still given its name only when complete
  (`TempFile`: `O_TMPFILE` + link, or a named temporary file swept later). The pipeline cache
  (`replace_file`, small, its content not hash-checked by the clone) keeps its fsync.
* The window's exit waits at most `EXIT_FLUSH` (1 s) for the writers in flight
  (`cache::flush_within`); what is left is made again on the next start ("exit: N cache write(s)
  left for the next start" in the log). The headless modes still wait for all of them.
* `Game::exit` (shell/exit.rs): the window hidden at once, the log's last drive line and the run's
  length written (docs/waits-log.md; at most 0.2 s for the log's thread), the audio muted and left
  to the process's end (the system closes the device), then everything else dropped in its order as
  before (the renderer before the window's surface: the other order panics in wgpu-hal with a
  swap-chain image still held). Each part's time is logged (`exit: … at N ms`).

Measured after (loaded, 9 runs at 0.5 / 2 / 12 s into a session, cold cache): 0.08–0.31 s.
`tests/launcher_fresh.sh` (step h) and `tests/launcher_start.sh` pass (the gate's FRESH run).
