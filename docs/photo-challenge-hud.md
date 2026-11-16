# Photo-challenge HUD regression

The reported missing countdown is a rendering-state bug, not a missing
challenge clock. `start_challenge` already loads the record's time limit,
offers the destination milestone, and restricts map markers to that goal.
The existing selftest exercises the start, countdown, timeout/retry and
arrival, but previously checked only the stored timer string.

`ScreenState::run` stores uppercase `fe_hash` script names. The timer
compared this against case-sensitive `bin_hash("Animate")` or
`bin_hash("Animate_RED")`, so it restarted the animation on every HUD
update. Both shipped scripts begin at alpha zero. The comparison now
uses the same hash convention as the renderer. Tests require visible
glyphs after multiple updates, for both the normal and under-ten-second
timer. The game flow selftest also checks emitted timer glyphs.

The reported incorrect marker icon has two related problems:

- The timed challenge's kind 0x12 assigned its texture only in prompt
  state 2, but the clone clears texture overrides each frame. State 3
  therefore reverted to the authored package texture. Native
  `0x4C8B80` assigns the texture with `0x50C990` and the object retains it.
  The clone now restores that texture in each active frame.
- The photo destination's kind 0x11 handler was explicitly empty.
  Native `0x4C8DD0` shows the mode icon and second highlight, runs the same
  Blink/Pulse scripts, and sets the photo icon in both active states.
  It returns state 4 to 3 and clears the visuals in state 6, as the
  shared marker handler does. The extra `0x5012C0` call reads a flag;
  its return value is unused. The clone now renders that handler.

Native icon hashes remain unchanged: 0xD1D9D8A4 (`ICON_SHOWCASE`) for the
photo spot, 0xB0C0DCB5 (`MODE_ICON_WORLD_CHALLEN`) for the timed challenge.
The regression checks emitted textured quads during the first second
and after the prompt transitions to state 3, allowing the native blink.

## Evidence and limits

internal evidence: photo-challenge-hud/static.json contains fresh disassembly from
SPEED2.EXE SHA256
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.
The timer's native script checks are `0x4C929C..0x4C92F0`.
The probe log records the actual shipped script hashes/tracks and icon
names. Red/green regression logs are retained beside it.

The standard `challenge_hud` shot samples 5000 ms, which is also a
zero-alpha instant of the red timer's 500 ms animation at the 1.2x FE
clock. A blank still at that instant alone is not proof of a live bug.
The failing multi-update glyph test establishes the bug; a shot at
5250 ms shows the corrected red digits. No native blink was disabled.

Fresh private Wine captures now show the challenge icon, the 2-minute
offer, the running countdown and the photo-spot arrival state. The
arrival used instrumented relocation, not a naturally driven route;
metadata records that limitation. The session was stopped and its prior
private save restored. Selected frames and capture metadata are retained
in the evidence directory.

Release binaries/examples, the combined 553-test suite (10 ignored),
selftest and the earlier 102-check scene/cache/driving/career run pass.
The ordinary suite does not include the explicitly ignored reverse-rate
failure. This change does not establish full photo-shoot parity:
saved pictures for kinds 1/2 and other cover rendering gaps remain open.
