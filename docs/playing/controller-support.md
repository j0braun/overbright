# Controller support

Plug in a gamepad and play: Overbright sets up Xbox, PlayStation, Switch Pro, Steam Deck and 8BitDo pads with
modern controls, analog triggers as pedals and Start to pause. You can plug a pad in or out while the game
runs. Old DirectInput pads and wheels keep the original game's controls.

Tested with a virtual Xbox 360 pad only. Rumble, PlayStation and Switch pads and the Steam Deck's controls
have not been tried on real hardware yet.

## The two layouts

**Options → Display → Enhancements → Controller → Controller Layout** chooses how a pad is presented:

- **Original**: the pad as Windows and the original game present it through DirectInput. On a modern pad
  this feels broken (see "Why a modern pad needs the Modern layout" below).
- **Modern**: the layout below.
- **Auto** (the default): Modern for a pad the game's mapping database knows as a standard gamepad,
  Original for everything else, and Original for a pad with your own custom bindings from the Original
  layout.

The row shows what Auto chose: "Auto (Modern)", "Auto (Original)", "Auto (Mixed)" or "Auto" (no pad).

### Modern controls

| | Xbox names (PlayStation and others: the same positions) |
|---|---|
| Throttle / brake | RT / LT, analog |
| Steer | left stick (or d-pad left / right) |
| Nitrous | A |
| Handbrake | X |
| Look back | B |
| Camera | Y |
| Shift down / up | LB / RB |
| Pause | Start |
| World map | Back |
| Enter Race / Shop | d-pad up |
| Read SMS | d-pad down |
| In menus | A select, B back, X customise, Y button 2, LB / RB page lists or zoom the map, Back button 3, Start select |

PlayStation pads use Cross / Circle / Square / Triangle and R2 / L2 in the same positions. Switch Pro, 8BitDo
and other pads also go by position (so South is Nintendo's B).

What else the Modern layout adds:

- Holding the stick in a menu repeats the step.
- Help lines show your pad's buttons ("A: Select", "Cross: Select") instead of keyboard keys.
- On the profile name screen, d-pad or stick up / down change the last letter (A–Z, 0–9), right adds a
  letter and left deletes one.
- The Controls page shows your pad's button names ("Right Trigger", "A", "LB").

### Other controller settings

All under **Enhancements → Controller**. Each is off, or at the original's value, until you change it.

| Setting | Values | Default |
|---|---|---|
| Controller Layout | Auto / Modern / Original | Auto |
| Stick Dead Zone | 0–25 % | 0 % |
| Trigger Dead Zone | 0–20 % | 0 % |
| Stick Saturation | 100 / 95 / 90 / 85 / 80 % | 100 % |
| Steering Linearity | 1.00 / 1.25 / 1.50 / 2.00 / 0.75 | 1.00 (linear, as the original) |
| Rumble | Off / Low / Medium / High | Off |
| Pause on Disconnect | Off / On | Off |
| Safe Axis Binding | Off / On | Off |

- **Rumble** shakes the pad on bumps and hard speed changes, on rough ground, on a nitrous shot and on a gear
  change. The original has no rumble for pads.
- **Pause on Disconnect** opens the pause menu when the pad you drive with is unplugged in a race or free
  roam. Without it, the car keeps rolling.
- **Safe Axis Binding**: when you bind an axis on the Controls page, only the axis you moved most is bound;
  the original also clears the roles of the other axes it sees moving.
- **Restore Native** and the graphics presets leave Controller Layout alone, as they leave the resolution
  alone. Restore Native does put the other settings in this group back to the original's values.
- Your custom bindings are kept per layout. Switching the layout and then saving drops the other layout's
  custom bindings, as the original keeps only the attached pads' blocks.

### Rebinding

The Controls page rebinds buttons, axes and the d-pad. Rebinding the d-pad (POV) works in both layouts.

## Why a modern pad needs the Modern layout

This is what the original's own controls give a modern pad, and what Original still gives:

| Pad | Steer | Throttle / brake | Handbrake, nitrous, shift | Pause | Menus (Select / Back) |
|---|---|---|---|---|---|
| Xbox (045E) | left stick | **buttons** B / A; the triggers do nothing (the default combined pedal axis lRz does not exist on the XInput HID layout) | RB / LB, Start = shift up, Back = shift down | **none** (no pad pause in the original's defaults) | A / B |
| PlayStation (054C) | left stick | **right stick** up/down (lRz), Cross = throttle, Square = brake; triggers unused | R1 / L1 … | none | **Square / Cross** |
| Switch Pro (057E), Steam Deck / Steam Input (28DE), 8BitDo, generic | left stick | right stick up/down; West = brake, South = throttle | | none | **West / South** |
| DirectInput wheel | not tested (the DevType Driving comes only from the product table) | | | | |

Other things the original does with a pad, which Original keeps:

- Steering and pedals use the original's 8-bit values. At rest the steering reads 1/128 right and the
  throttle 2/255; a full stick reads 125/128 left and −1 right. There is no dead zone and no curve.
- In menus the stick moves one step per push and never repeats. L1 / R1 (the `[` `]` keys) are unbound on
  every pad without a preset, so world-map zoom and list paging are out of reach. The profile name can only
  be typed, so a pad can only accept "NAME".
- With two pads, buttons from both count; analog input comes from the first pad attached (**inferred**).
- The car on the front end's platform does not turn on any pad input or key. Only the mouse turns it: hold the
  middle button and drag sideways to turn the car, up or down to lower or raise the camera
  ([formats/front-end.md](../formats/front-end.md), "Turning the car").
- No rumble: the original's effects are DirectInput force feedback, which pads do not expose.
- Help lines show keyboard keys ("Enter: Select") whatever the device.
- Every menu screen is reachable: the pad's menu map gives the keyboard's arrows, Enter, Esc, C, `,` and `.`.

Fixed in both layouts: the d-pad works on pads whose driver reports it as a hat (Linux xpad; before, it did
nothing), and a third pad is taken when one of the first two is unplugged (before, it was never attached).

## How it works

The rest of this page is for developers. The original's pad model and addresses are in
[formats.md](../formats.md) **Gamepads and joysticks**. Code: `src/pad.rs` (the original's logic),
`src/pad/host.rs` (gilrs and the scripted pad), `src/pad/extras.rs` (the optional settings), game.rs
`pad_frame` / `controls`. Detection and hot-plug use gilrs, with the SDL_GameControllerDB built in and its
filters off.

### Traced in SPEED2.EXE

- **Axis capture 0x5CA730 / 0x5B8260** (decompiled): every axis past the limit is handed to 0x5B8260, which
  first clears every role on that axis. The first call takes the action and zeroes +0x33C, so later ones only
  switch roles off. That this clears other axes' roles is the original's behaviour, so it is kept. The base
  0x86F8B0.. is set at the capture's start (0x5B91D3) and after each frame. Overbright runs the capture once
  per 60 Hz tick of the game clock (independent of the frame rate), takes the base at the start, and ports
  the POV capture 0x5C1730 with 0x5B83C0's direction rule, including its quirk: a button bound in the same
  frame makes a held POV write 0 to its entry. Earlier builds had no POV capture.
- **Throttle 0x416DE0** (disassembly): the analog alternate (combined axis) × 1.07, except × 1 in neutral (car
  gear +0x434 = 1); the plain analog row × 1 (its × 2 / × 4 branch needs 0x404800, `xor al,al; ret`). In
  neutral the combined result goes through the graph 0x7EA004 (0x40C840, lerp 0x576750:
  (1 − t)·y0 + t·y1). Ported as `pad::driving(…, neutral)` with `NEUTRAL_THROTTLE`; game.rs passes the
  drivetrain's gear 1. The keyboard throttle (0 or 1) is not changed by the graph. Earlier builds applied the
  × 1.07 in neutral too.
- **Menu stick rows 0x571D60**: a press on crossing, deadline +0x20 = tick 0x865134 + 25, then a repeat when
  tick > deadline (+6). 0x865134 has **no writer**; under Wine it reads **0** in the main menu, the pause menu
  and free roam, so the original never repeats. Overbright's press-only Original layout is right; the repeat
  is a Modern-layout feature, counted in 60 Hz ticks of the game clock (25, then every 6).
- **Force feedback** ([formats.md](../formats.md)): centring spring 0x5C9160, surface damping / terrain
  0x5C9FE0, bump jolt 0x5C9770 (speed change > 4 within 0.15 s, or the wheels' +0x370.. sum > 0.05),
  0x5C9DA0 on surface 0xD. These are DirectInput force-feedback effects for wheels; a pad under DirectInput
  has no actuators, so the original gives a pad no rumble. The Rumble setting models the jolt (strong motor,
  by the speed change) and adds weak-motor rumble for surface 0xD (**inferred** to be "rough"), a nitrous
  shot and a gear change.
- **Which device drives** (partly traced): player p's device byte 0x864788[p] (0xFF = default 0x7F9414[p])
  is set by the event-1 handler 0x571A30, registered by 0x579560. What raises event 1 is **open**, so "the
  first attached pad drives" stays **inferred**.

### The settings

Options.txt keys `enh.pad_*`, written only when not at the original's value. Label ids: Controller Layout
0xC104E033, Stick Dead Zone …34, Trigger Dead Zone …35, Stick Saturation …36, Steering Linearity …37 (1.00 is
the original's 0x57C250 identity graph), Rumble …39, Pause on Disconnect …3A, Safe Axis Binding …3B; the group
label 0xC104E032 "Controller". The ids were checked unused in the tree.

- The presets leave the group alone (quality-of-life settings).
- The shaping (`PadOptions::shape`) is a pure function of each sample. The rumble model and the capture run on
  60 Hz world / clock ticks, and the menu repeat counts 60 Hz ticks of the game clock, so all of it is the same
  at 30, 60, 120 and 200 Hz (tests).
- A Modern pad's map is kept in the profile under its GUID with the last byte XOR 0x4D, so the Original and
  Modern maps never overwrite each other.

### Auto

Auto is a default for how a device is presented, like the display mode: documented and switchable.

- **Standard pad** (Overbright's own rule, `host::standard_pad`): gilrs's mapping (SDL_GameControllerDB, or a
  kernel driver with the evdev gamepad codes) gives South, East and the left stick, and the product is not one
  the original has a preset or type override for (`host::legacy_product`: the nine 0x7FE7F8 presets, which
  are Logitech Dual Action / Rumble Pad / WingMan, the EMS PS2 adapter, Saitek and the old Microsoft pad, and
  the 0x800588 wheel). Xbox, PlayStation, Switch Pro, Steam Deck / Steam Input and 8BitDo pads resolve to
  Modern; old DirectInput pads and wheels to Original. The scripted pad's `connect <id> <vid> <pid> legacy`
  forces a non-standard device.
- **Saved bindings keep working:** when the profile holds an Original block for the pad (its own GUID) that
  differs from the original's defaults, Auto presents the pad as Original (`Host::set_saved_blocks`, called by
  `apply_pad_maps` at start-up and on every profile load). The switch goes one way within a session (Modern →
  Original), so restoring a pad's defaults on the Controls page does not flip its layout mid-session.
- **No pad attached:** nothing changes (no pad state, no prompts; the state hashes of the checks are
  identical).
- Auto counts in the list of differences from the original ("modern controller layout", as Modern does)
  while it makes a pad Modern (`extras::resolved`).
- options.txt: Auto writes nothing, Original writes `enh.pad_layout 0`, Modern `enh.pad_layout 1`. A file
  without the line gets Auto.

### Tests

`pad::tests` (the neutral graph and × 1.07, POV capture and direction, original and safe capture, Modern
exact pedals and buttons, menu repeat at 30/60/120/200 Hz), `pad::host::tests` (stick repeat through the host
at 60–1000 polls/s, original press-only, layout switch, GUIDs, driver lost), `pad::extras::tests` (defaults
change nothing, options round trip, shaping and the steering curve, rumble pulse and fade), the `qol` round
trip, game.rs `pad_only_modern_layout_end_to_end` (real assets: the original's triggers do nothing, Modern
drives, Start pauses, B resumes, d-pad name entry, unplugging pauses). End to end with a **uinput** virtual
Xbox 360 pad read by gilrs (a developer probe, not in the public repository): detection, the Original and
Modern reports, the hat d-pad, unplugging, and the three-pad rescan.

## Open

- Rumble on a real pad (the uinput pad has no force feedback; gilrs's force feedback needs write access to the
  event node).
- PlayStation, Switch and Steam Deck on real hardware (the mapping comes from gilrs's SDL database; checked
  here only with the Xbox 360 ids).
- Which device drives (event 1 → 0x571A30), the second byte of the menu stick rows (+0x13), the device arrows'
  Blink, and the Controls page with a pad under Wine (no pad in the prefix).
- Bindings are per layout. No text entry other than the profile name (MC_Main) was found that would need pad
  input.
- The windowed-mode system cursor belongs to the mouse's own topic.
