# Controller support (2026-11-11)

Requirement: "proper controller support is missing". This file is the assessment, what the original does (traced
in SPEED2.EXE), what the clone ports natively, and the opt-in Controller extras. Native model and addresses:
`docs/formats.md` **Gamepads and joysticks**; code `src/pad.rs` (native logic), `src/pad/host.rs` (gilrs and the
scripted pad), `src/pad/extras.rs` (the extras), game.rs `pad_frame` / `controls`.

## Status (assessment before this work, base binary)

How the clone behaved with a common pad plugged in on a fresh install, from the code and the scripted pad host
(`NFSU2_PAD_SCRIPT`). Everything below "Native" is what the original does with the same pad under DirectInput as
far as the presentation (which axes/buttons Windows shows) is inferred; the clone reproduces it faithfully, which
is exactly why it feels broken on a modern pad.

| pad family | detected | steer | throttle / brake | handbrake, nitrous, shift | pause | menus (Select / Back) |
|---|---|---|---|---|---|---|
| Xbox (045E) | yes (gilrs, SDL mapping db) | left stick | **buttons** B / A; the triggers do nothing (default combined pedal axis lRz does not exist on the XInput HID layout) | RB / LB, Start = shift up, Back = shift down | **none** (no pad pause in the native defaults) | A / B |
| PlayStation (054C) | yes | left stick | **right stick** up/down (lRz), Cross = throttle, Square = brake; triggers unused | R1 / L1 … | none | **Square / Cross** |
| Switch Pro (057E), Steam Deck / Steam Input (28DE), 8BitDo, generic | yes, presented as a 2004 four-axis twelve-button pad | left stick | right stick up/down; West = brake, South = throttle | | none | **West / South** |
| DirectInput wheel | n/a here (DevType Driving only via the product table) | | | | | |

* Detection / hot-plug: gilrs (SDL_GameControllerDB built in) with filters off; hot-plug works for the first two
  pads; a third pad was never attached after one of the first two was unplugged.
* Analog: native 8-bit chain, at rest steering reads 1/128 right and the throttle 2/255 (native quirk); a full
  stick reads 125/128 left / −1 right. No dead zone, no curve options.
* Menus: every screen takes the pad's FE map as the keyboard's arrows / Enter / Esc / C / , / . (so every screen
  the keyboard can drive, incl. the Enhancements hub, dialogs, shops, the world map, is reachable) **but** the
  FE stick gave one step per push (the audit called the repeat missing; this work found the original never repeats, below), L1 / R1 ([ ]) are unbound
  on every non-preset pad (world map zoom, list paging), and the profile name editor takes typed text only (a
  pad can only accept "NAME").
* Rebinding on the Controls page works for buttons and axes; POV (d-pad) capture was not ported; the analog
  `_ALTERNATE` throttle × 1.07 was applied in neutral too (native: × 1 when car +0x434 = 1).
* Driving with two pads: buttons OR-ed, analog from the first attached pad only (inferred).
* Force feedback / rumble: none (the original's effects are DirectInput FF, which pads do not expose).
* Help bar / footers: keyboard text ("Enter: Select") whatever the device.
* Disconnect mid-race: the car keeps the last report until the record detaches (then released); no pause.

## After this work

| pad family | Controller Layout **Original** (native) | Controller Layout **Modern** (what **Auto**, the default, gives a standard pad) |
|---|---|---|
| Xbox / Steam Deck / Steam Input | as before (native), plus the d-pad now works on pads whose driver reports it as a hat (Linux xpad: it did nothing before) | RT throttle / LT brake analog (exact 0..1), left stick steers (rest 0, full ±1), A nitrous, X handbrake, B look back, Y camera, LB/RB shift, Start pause, Back world map, d-pad up Enter Race/Shop, down Read SMS, left/right steer; menus A select, B back, X customise, Y button 2, LB/RB page ([ ]), Back button 3, Start select |
| PlayStation | native (right stick pedals, Square/Cross) | same positions: Cross / Circle / Square / Triangle, R2 / L2 |
| Switch Pro / 8BitDo / generic | native (four-axis twelve-button pad) | same positions (gilrs's SDL mapping by position; South = Nintendo's B) |

| screen / feature | before | after |
|---|---|---|
| every FE screen (hub, dialogs, shops, world map, Controls) | reachable, stick one step per push | same (native: verified the original never repeats); Modern: the stick repeats (25 then every 6 ticks of 60 Hz, game clock) and LB/RB page lists / zoom |
| profile name (MC_Main) | keyboard only | Modern: d-pad / stick up/down change the last letter (A–Z, 0–9), right adds one, left deletes |
| help footers ("Enter: Select") | keyboard text | Modern: the last-used pad's button ("A: Select", "Cross: Select") |
| Controls page | buttons / axes; POV capture missing | POV (d-pad) capture ported (0x5C1730); Modern shows the pad's names ("Right Trigger", "A", "LB") |
| hot-plug | third pad never attached | a waiting pad takes a freed channel (gilrs rescan) |
| disconnect while driving | car keeps rolling | Pause on Disconnect (extra) opens the pause menu |
| rumble | none (native) | Rumble Low/Medium/High (extra) |

### What was traced in SPEED2.EXE (2026-11-11)

* **Axis capture 0x5CA730 / 0x5B8260** (REA decompile): every axis past the limit is handed to 0x5B8260, which
  first clears every role on that axis; the first call takes the action and zeroes +0x33C, so later ones only
  switch roles off. The audit's "bug" is **native**; kept. The base 0x86F8B0.. is set at the
  capture's start (0x5B91D3) and after each frame. The clone now runs the capture once per 60 Hz tick of the
  game clock (rate independent), takes the base at the start, and ports POV capture 0x5C1730 with 0x5B83C0's
  direction rule (incl. the quirk: a button bound in the same frame makes a held POV write 0 to its entry).
  Extra: **Safe Axis Binding** (off = native) gives only the axis that moved most.
* **Throttle 0x416DE0** (disassembly): the analog alternate (combined axis) × 1.07 except × 1 in neutral (car
  gear +0x434 = 1); the plain analog row × 1 (its × 2 / × 4 branch needs 0x404800, `xor al,al; ret`); in neutral
  the combined result goes through the graph 0x7EA004 (0x40C840, lerp 0x576750: (1 − t)·y0 + t·y1). Ported as
  `pad::driving(…, neutral)` with `NEUTRAL_THROTTLE`; game.rs passes the drivetrain's gear 1. Keyboard throttle
  (0 / 1) is unchanged by the graph.
* **FE stick rows 0x571D60**: the press on crossing, deadline +0x20 = tick 0x865134 + 25, then a repeat when
  tick > deadline (+6). 0x865134 has **no writer**; under Wine it reads **0** in the main menu, pause menu and free
  roam (internal evidence), so the original never repeats — the clone's
  press-only behaviour was right; the repeat is now a Modern-layout extra on 60 Hz ticks of the game clock.
  (The audit's item is therefore not a native gap.)
* **Force feedback** (formats.md): centring spring 0x5C9160, surface damping/terrain 0x5C9FE0, bump jolt
  0x5C9770 (speed change > 4 within 0.15 s, or the wheels' +0x370.. sum > 0.05), 0x5C9DA0 on surface 0xD. These
  are DirectInput FF effects for wheels; a pad under DirectInput has no actuators, so the original gives a pad
  no rumble. The Rumble extra models the jolt (strong motor, by the speed change) and adds weak-motor rumble for
  surface 0xD (**inferred** as "rough"), a nitrous shot and a gear change.
* **Per-player device** (partly): player p's device byte 0x864788[p] (0xFF = default 0x7F9414[p]) is set by the
  event-1 handler 0x571A30 registered by 0x579560; what raises event 1 is **open**, so "the first attached pad
  drives" stays **inferred**.

### Options (Enhancements hub → Controller; options.txt `enh.pad_*`, written only when not native)

| row (label id) | values | native |
|---|---|---|
| Controller Layout (0xC104E033) | Auto / Modern / Original | **Auto** (design decision 2026-11-11, a device-presentation default; Original is the native value) |
| Stick Dead Zone (…34) | 0–25 % | 0 % |
| Trigger Dead Zone (…35) | 0–20 % | 0 % |
| Stick Saturation (…36) | 100 / 95 / 90 / 85 / 80 % | 100 % |
| Steering Linearity (…37) | 1.00 / 1.25 / 1.50 / 2.00 / 0.75 | 1.00 (linear, the original's: 0x57C250's identity graph) |
| Rumble (…39) | Off / Low / Medium / High | Off |
| Pause on Disconnect (…3A) | Off / On | Off |
| Safe Axis Binding (…3B) | Off / On | Off |

Group label 0xC104E032 "Controller". Ids checked unused in the tree.
Presets leave the group alone (QoL). The shaping (`PadOptions::shape`) is a pure function of each sample;
the rumble model and the capture run on 60 Hz world / clock ticks; the menu repeat counts 60 Hz ticks of the
game clock: the same at 30 / 60 / 120 / 200 Hz (tests). A Modern pad's map is kept in the profile under its GUID
with the last byte XOR 0x4D, so the native and Modern maps never overwrite each other (the profile keeps only
the attached pads' blocks, as the original: switching the layout and saving drops the other layout's custom map).

### Controller Layout Auto (the default; design decision 2026-11-11)

A device-presentation default, like the display mode: documented and switchable.

* **Auto** = Modern for a pad the mapping database knows as a standard gamepad, Original for anything else.
  Standard (**clone heuristic**, `host::standard_pad`): gilrs's mapping (SDL_GameControllerDB, or a kernel driver
  with the evdev gamepad codes) gives South, East and the left stick, and the product is not one the original has
  a preset or type override for (`host::legacy_product`: the nine 0x7FE7F8 presets — Logitech Dual Action / Rumble
  Pad / WingMan, the EMS PS2 adapter, Saitek, the old Microsoft pad — and the 0x800588 wheel). Xbox, PlayStation,
  Switch Pro, Steam Deck / Steam Input and 8BitDo pads resolve to Modern; old DirectInput pads and wheels to
  Original. The scripted pad's `connect <id> <vid> <pid> legacy` forces a non-standard device.
* **Saved bindings keep working**: when the profile holds an Original block for the pad (its own GUID) that
  differs from the native defaults, Auto presents it as Original (`Host::set_saved_blocks`, called by
  `apply_pad_maps` at start-up and on every profile load). The switch is one way within a session (Modern →
  Original), so restoring a pad's defaults on the Controls page does not flip its layout mid-session.
* **No pad attached**: nothing changes (no pad state, no prompts; the state hashes of the gate are identical).
* **Honest hub**: the row reads "Auto (Modern)", "Auto (Original)", "Auto (Mixed)" or "Auto" (no pad); Auto
  counts in the deviations list ("modern controller layout", as Modern does) while it makes a pad Modern
  (`extras::resolved`).
* **Restore Native and the presets leave it** (a device setting, like the resolution); Restore Native still
  puts the feel options (dead zones, saturation, linearity, rumble, pause on disconnect, safe binding) back to
  native, and those stay off / native by default.
* options.txt: Auto writes nothing; Original is now written (`enh.pad_layout 0`), Modern `enh.pad_layout 1`.
  A file from before the change (no line) gets Auto.
* For docs/native-deviations.md (lead's list): "Controller Layout defaults to Auto: standard gamepads get the
  Modern layout (analog triggers as pedals, Start pauses, exact analog, pad button names); Original (the
  DirectInput presentation) for legacy devices, wheels, pads with custom Original bindings, or by choice."

### Tests

`pad::tests` (neutral graph and × 1.07, POV capture and direction, native vs safe capture, Modern exact pedals and
buttons, FE repeat at 30/60/120/200 Hz), `pad::host::tests` (stick repeat through the host at 60–1000 polls/s,
native press-only, layout switch / GUIDs / driver lost), `pad::extras::tests` (defaults identity, options round
trip, shaping and the steering curve, rumble pulse and fade), `qol` round trip, game.rs
`pad_only_modern_layout_end_to_end` (real assets: native triggers do nothing, Modern drives, Start pauses, B
resumes, d-pad name entry, unplug pauses). End-to-end with a **uinput** virtual Xbox 360 pad read by gilrs
(the maintainer's probe `pad_probe`, internal evidence: the uinput logs, `vpad.py`, `vpad3.py`): detection,
native and Modern reports, the hat d-pad, unplug, and the three-pad rescan.

### Open

* Rumble on a real pad (the uinput pad has no FF; gilrs's ff needs write access to the event node).
* PlayStation / Switch / Deck on real hardware (the mapping comes from gilrs's SDL database; checked here only
  with the Xbox 360 ids).
* The per-player device rule (event 1 → 0x571A30), the second byte of the FE stick rows (+0x13), the device
  arrows' Blink; the Controls page with a pad under Wine (no pad in the prefix).
* Bindings are per layout; text entry beyond MC_Main's name (none other found in the clone's screens).
* Not in scope here: the windowed-mode system cursor (the mouse's own topic).
