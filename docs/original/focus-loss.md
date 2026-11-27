# Focus loss: the game in the background

What `SPEED2.EXE` does when its window loses the focus or is minimised, and how Overbright's
**In the background** row (Enhancements, Performance) matches it.

## The original

Traced in the window procedure and the message pump, then observed under Wine.

**Traced** (disassembly of SPEED2.EXE):

- **WM_ACTIVATE (6)**, Win_WndProc 0x5CCD70 → 0x5CCDB6. Inactive means LOWORD(wParam) == 0. When the audio
  manager (0x82B884) exists and the state changed (byte 0x8709E0), going inactive calls Music_OnPause
  0x4810F0(-1, 1). That holds the music and the speech streams. Then it calls AudioManager_Update 0x483B40(0).
  Coming back calls Music_OnFlowState 0x481210(-1), which resumes them.
- **WM_ACTIVATEAPP (0x1C)** → 0x5CCF4C. It sets g_AppActive 0x870988 = wParam and calls 0x5B7EF0, which resets
  the input devices' state blocks (0x874C48…, 0x874FB8…, 0x875328…). Held keys are let go.
- **WM_SIZE (5)** → 0x5CCE2D. It sets g_AppActive = !IsIconic(hwnd), so a minimised window is inactive too.
- **The pump** 0x5BF1D0, once per main-loop pass (0x5811D0). It first peeks for WM_QUIT. Then
  `if (Race_OtherHumans 0x5F0100 || g_AppActive)` it calls PeekMessage(PM_REMOVE), and otherwise
  **GetMessage, which blocks**. While the game is inactive, each pass waits for a window message. The sim, the
  drawing and the input stand still. A race with other human racers (LAN or online) keeps running.
- The input poll 0x5CFD70 skips the per-device update while g_AppActive is 0 (0x5CFDBB).

**Observed** (Wine 9, DXVK, the widescreen fix's borderless window; a notepad started in the game's Wine desktop
takes the focus; restored with ShowWindow(SW_RESTORE) + SetForegroundWindow; sound recorded to a file through
a private null sink). The samples are once a second
([evidence](../evidence/focus-loss/)):

| | Free roam | Quick Race (Circuit, racing) |
|---|---|---|
| Focused | sim clock 0x865188 +4130/s, world update counter 0x7FB724 +123/s (120 fps) | the same |
| Focus lost | 0x870988 = 0, 0x8709E0 = 1 within a second; 0x865188, 0x7FB724 and 0x8651A8 stand still for 60 s | the same, 15 s |
| Sound while away | no change in the recording (a flat level: the stream stopped) | the same |
| Notepad closed | one pass, +200 ticks (the window's messages); still inactive and minimised | — |
| Focus back | 0x870988 = 1, the clocks run at once at 4130/s; **the time away is dropped** (+300 ticks, not +240 000); sound back within a second | the same (+1566 ticks over the restore) |

The game window minimised itself when it lost the focus (the widescreen fix's borderless window). Whether the
plain full-screen game does the same was not checked (Alt+Tab out of exclusive full screen minimises it
anyway).

**Not traced:** D3D9 device-lost / Reset handling on the way back.

## Overbright

The row **In the background** (`enh.background`):

| Value | What it does |
|---|---|
| **Native** (Pause), the default | As the original (observed: docs/evidence/focus-loss/, the Observed table above). While the window is not the focused one or is hidden (minimised or covered: winit's `Focused` / `Occluded`), no frame is drawn and no tick runs: the event loop sleeps until the next window event (`ControlFlow::Wait`). Going in, every sound is held (`Mixer::hold_all`: silence out, nothing moves on) and the held keys are let go at once. Coming back, the time away is dropped (the frame clock starts from that moment) and the sound goes on from where it stood. An uncovered paused window is drawn again as it stood when asked (no time passes). |
| **Keep running** | The sim, the sound and the drawing go on. |
| **Draw less** | As Keep running, the drawing held to 30 fps (`extras::BACKGROUND_FPS`). It never goes lower: a drawn frame steps at most 3 race / 2 scene ticks, so under 20 / 30 fps time would be dropped. |

- Any frame rate: a pause runs no ticks, and the time away is dropped. The ticks after it are the same at
  30, 60, 120 or 200 Hz.
- Overbright has no races with other human players, so the original's exception has nothing to apply to.
- The map editor's and Studio's test game (`--map-mod` / `--map-file`, `--looks-live`) and scripted windowed
  runs (`NFSU2_KEEP_RUNNING` set: the `*_xtest.sh` scripts, `dyno_xvfb.sh`, the Wine smoke) always keep
  running. The headless harness (shots, benches, flows, the playthrough) has no window and never pauses.
- The row took over Power Saving's former Background step. An older `enh.power_saving N` line is read as
  Draw less plus Power Saving N − 1 (see [saves-options](../formats/saves-options.md)).

Code: `Game::background_pause` (src/bin/game/shell/frame_loop.rs), `about_to_wait` (shell/window.rs),
`extras::background`, `Mix::hold`. Tests: `focus_loss_pauses_and_resumes` (the game),
`a_held_mix_is_silent_and_stands_still`, `extras::tests` (the caps, the migration, the forced Keep running).
