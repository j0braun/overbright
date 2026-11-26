# Controller support

Plug in a gamepad and play: Overbright sets up Xbox, PlayStation, Switch Pro, Steam Deck and 8BitDo pads with
modern controls, analog triggers as pedals and Start to pause. You can plug a pad in or out while the game
runs. Old DirectInput pads and wheels keep the original game's controls.

Tested with a virtual Xbox 360 pad and a virtual force-feedback wheel only. Rumble, force feedback on a
real wheel, PlayStation and Switch pads and the Steam Deck's controls have not been tried on real hardware
yet.

## Wheels and force feedback

A racing wheel drives as in the original (traced: docs/formats/front-end.md, Wheels and Force feedback): its own steering (the **Wheel Range** slider on the Wheel
options page sets how much of the wheel's turn gives full lock), and the original's force feedback — a
centring spring at full lock and in the menus, a damper that fades with speed, the tyres' pull as a
steering force (its divisor is **Road Feedback**), the road surface as a vibration, a light wheel in
deep puddles, and short jolts on hard speed changes and bumps. **Overall Force Strength** sets the
strength (as in the original, anything from 0.4 up is full strength) and **Feedback** (Player options)
turns the effects off except the centring spring. This is on whenever a wheel with force feedback is the
driving device; there is no extra option. Without such a device nothing changes.

- Linux: the effects go to the wheel's event device (`/dev/input/event*`, which needs write access; desktop
  systems give it to the logged-in user for game controllers). Effects the driver does not offer are
  skipped: the mainline Logitech driver (hid-logitech, lg4ff) has only the steering force and autocenter,
  so the spring, damper, surface and jolts need a driver that offers them (new-lg4ff, hid-tmff2, the
  Fanatec driver).
- Windows: the effects go through DirectInput 8, as in the original (open: no trace note for the Windows force-feedback path): the game controller with the wheel's
  vendor and product id that reports force-feedback actuators, opened exclusive in the foreground (so the
  forces stop while the game window is in the background, as in the original). Not yet tried on a real
  wheel: the calls are tested against the original's parameters and the build runs under Wine, but no
  wheel driver has received them.
- Gamepads keep the Rumble extra; the original gives XInput pads nothing.

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
| Steering Linearity | 1.00 / 1.25 / 1.50 / 2.00 / 0.75 | 1.00 (linear, as the original; inferred: docs/formats/cars-1.md, Steering) |
| Rumble | Off / Low / Medium / High | Off |
| Pause on Disconnect | Off / On | Off |
| Safe Axis Binding | Off / On | Off |
| Quit with Both Sticks | Off / On | Off |

- **Rumble** shakes the pad on crashes, bumps and hard speed changes, on textured ground (kerbs, pavement,
  gravel), in puddles, on a nitrous shot and on a gear change. Every hit — a wall, another car, a sign or other
  knock-over prop — gives a clear pulse, light touches included, and harder hits a harder one. The original has no rumble for XInput pads (Xbox and most
  modern pads); it gives force feedback only to DirectInput devices that report force-feedback actuators. On
  Windows it uses the pad's motors through Windows.Gaming.Input (XInput pads); this was not yet tried on a real
  Windows PC.
- **Pause on Disconnect** opens the pause menu when the pad you drive with is unplugged in a race or free
  roam. Without it, the car keeps rolling.
- **Safe Axis Binding**: when you bind an axis on the Controls page, only the axis you moved most is bound;
  the original also clears the roles of the other axes it sees moving.
- **Quit with Both Sticks**: on the main menu, press both sticks down together (L3 + R3; LS + RS on Xbox
  pads) to get the game's own "Are you sure you want to quit?" question, the one the Q key opens, with
  Cancel focused. Nothing quits at once. It works with both layouts and any bindings, on the main menu
  only (not over a question, in another menu or in a race), and only after both sticks were let go on
  that screen: sticks already held when the main menu appears do nothing, and one stick alone does
  nothing. The original has no pad shortcut for quitting: its default pad maps bind no button to the
  quit question, which the Q key, the help bar's Quit Game button and closing the window reach.
- **Restore Native** and the graphics presets leave Controller Layout alone, as they leave the resolution
  alone. Restore Native does put the other settings in this group back to the original's values.
- Your custom bindings are kept per layout. Switching the layout and then saving drops the other layout's
  custom bindings, as the original (open: no trace note for pad-block saving) keeps only the attached pads' blocks.

### Rebinding

The Controls page rebinds buttons, axes and the d-pad. Rebinding the d-pad (POV) works in both layouts.

## Why a modern pad needs the Modern layout

This is what the original's own controls give a modern pad, and what Original still gives:

| Pad | Steer | Throttle / brake | Handbrake, nitrous, shift | Pause | Menus (Select / Back) |
|---|---|---|---|---|---|
| Xbox (045E) | left stick | **buttons** B / A; the triggers do nothing (the default combined pedal axis lRz does not exist on the XInput HID layout) | RB / LB, Start = shift up, Back = shift down | **none** (no pad pause in the original's defaults) | A / B |
| PlayStation (054C) | left stick | **right stick** up/down (lRz), Cross = throttle, Square = brake; triggers unused | R1 / L1 … | none | **Square / Cross** |
| Switch Pro (057E), Steam Deck / Steam Input (28DE), 8BitDo, generic | left stick | right stick up/down; West = brake, South = throttle | | none | **West / South** |
| Racing wheel (Logitech, Thrustmaster, Fanatec and the others SDL lists) | the wheel, through the original's wheel steering (Wheel Range, no rate limit) | a combined pedal axis is bound only on a wheel without separate pedal axes; otherwise bind the pedals on the Controls page, as in the original (open: no trace note for the wheel pedal default) | generic buttons | none | as a generic pad |

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
- No rumble: the original's effects are DirectInput force feedback. An XInput pad shows no actuators under
  DirectInput, so it gets nothing. An older DirectInput pad that reports actuators gets the damper, the road
  surface and the jolts, but not the steering force or the Feedback-off spring (see below).
- Help lines show keyboard keys ("Enter: Select") whatever the device.
- Every menu screen is reachable: the pad's menu map gives the keyboard's arrows, Enter, Esc, C, `,` and `.`.

Fixed in both layouts: the d-pad works on pads whose driver reports it as a hat (Linux xpad; before, it did
nothing), and a third pad is taken when one of the first two is unplugged (before, it was never attached).

## How it works

The rest of this page is for developers. The original's pad model and addresses are in
[formats.md](../formats.md) **Gamepads and joysticks**. Code: `src/pad.rs` (the original's logic),
`src/pad/host.rs` (gilrs and the scripted pad), `crates/ob-settings/src/pad/extras.rs` (the optional settings), game.rs
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
- **Force feedback** ([formats/front-end.md](../formats/front-end.md), "Force feedback"): DirectInput 8
  effects on any device that reports force-feedback actuators — a centring spring at full lock (0x5C9160), a
  speed damper (0x5C95A0), the tyres' force across the car as a constant force (0x5C9390), the road surface
  as a periodic effect (0x5C9FE0), a negative damper in a puddle above 23 m/s (0x5C9DA0), and a short square
  jolt (0x5C9770) when the speed changes by more than 4 m/s from one update to the next or the bump stops
  hit; jolts are at least 0.15 s apart. Feedback off and drag races keep only the spring. The console builds'
  pad vibration (0x5727C0 → 0x572620 → 0x4022C0) ends in a bare `ret` on the PC. The effect model is
  `nfsu2::pad::ff::Original`, stepped once per 1/60 s world tick by `Game::ff_tick` (`Host::ff_tick`: the
  driving path while the player drives, the front-end path otherwise; the gain from Overall Force Strength
  when driving starts, autocenter off on a wheel when the device opens) and played by
  `nfsu2::pad::ff_out` (Linux evdev: EVIOCSFF uploads, EV_FF play / stop / FF_GAIN / FF_AUTOCENTER;
  DirectInput ±10000 → evdev ±0x7FFF, a +X direction → 0x4000 as SDL and Wine map it; one actuator
  assumed, **inferred**: evdev does not say which axes take forces; Windows: `ff_out::dinput`, the
  original's DirectInput calls, below). The Feedback option is enabled while
  such a device drives (`Host::force_feedback`, 0x5B8250's question). The Rumble setting is Overbright's own model:
  the jolt on the strong motor (by the speed change, and by the wheels' suspension travel over 0.05 m in a
  tick, which stands in for the bump stops: **inferred**), and the weak motor on the road texture (0x5C9FE0's
  surface table: surfaces 8–11 and 15–17, amplitude / 2 × min(1, speed / 13 m/s); plain asphalt gives none),
  puddles (surface 13, the one the negative damper plays on), a nitrous shot and a gear change. The original
  has no nitrous or gear-change effect. Collisions: each contact's first push — the push that posts its impact
  sound (0x5A9630 → 0x4797B0) against a track polygon, another car or a knock-over prop — is a hit
  (`CarState::new_hit`, read only by the Rumble model). Its push |J| sets a strong-motor pulse of
  0.35 + 0.65 · √(min(|J|, 30) / 30) (30: the push that totals a car in a drag race, 0x59FB80), held 0.1 s and
  then faded like the jolt, so a glancing car or a sign, which barely change the speed, still rumble. The
  underbody on a floor-like polygon while the car stands on its wheels is left to the bump pulse. This is
  Overbright's own; the force-feedback model is not changed: the original's jolt 0x5C9770 is called only from
  the speed and bump tests of 0x406460 (0x406775, 0x4067A6), so wheels get no collision effect of their own.
- **Which device drives** (partly traced): player p's device byte 0x864788[p] (0xFF = default 0x7F9414[p])
  is set by the event-1 handler 0x571A30, registered by 0x579560. What raises event 1 is **open**, so "the
  first attached pad drives" stays **inferred**.

### The settings

Options.txt keys `enh.pad_*`, written only when not at the original's value. Label ids: Controller Layout
0xC104E033, Stick Dead Zone …34, Trigger Dead Zone …35, Stick Saturation …36, Steering Linearity …37 (1.00 is
the original's 0x57C250 identity graph), Rumble …39, Pause on Disconnect …3A, Safe Axis Binding …3B, Quit with Both Sticks …3C (`enh.pad_sticks_quit 1`); the group
label 0xC104E032 "Controller". The ids were checked unused in the tree.

- The presets leave the group alone (quality-of-life settings).
- Quit with Both Sticks (`Host::both_sticks`, called from `pad_frame`) reads the pads' raw stick buttons, not
  the mapped actions, so it is the same under Original and Modern and after rebinding (the default menu
  maps, 0x5C11B0's and Modern's, leave L3 and R3 unbound, so on those maps they do nothing else in menus;
  a preset pad of 0x7FE7F8 or a custom menu binding on a stick button keeps its action as well). It fires on the main menu under the
  same condition as the Q key (0x558B80: a screen with a help bar) and opens the same question
  (`Game::quit_question`). Off, it tracks nothing.
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
  the 0x800588 wheel, and every wheel `host::is_wheel` names). Xbox, PlayStation, Switch Pro, Steam Deck / Steam Input and 8BitDo pads resolve to
  Modern; old DirectInput pads and wheels to Original. The scripted pad's `connect <id> <vid> <pid> legacy`
  forces a non-standard device.
- **Saved bindings keep working:** when the profile holds an Original block for the pad (its own GUID) that
  differs from the original's defaults, Auto presents the pad as Original (choice: the Auto layout rule is Overbright's own) (`Host::set_saved_blocks`, called by
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
change nothing, options round trip, shaping and the steering curve, rumble pulse and fade, hit pulses by
strength), game.rs `rumble_pulses_on_every_crash_by_its_strength` (real assets, a scripted pad, Rumble High in
a race: a street sign knocked over, a light nudge of an opponent, a wall at 6 and at 22 m/s each pulse, harder
hits harder, the light ones under the speed jolt's 4 m/s; a smooth street records no hit), the `qol` round
trip, `pad::host::tests::both_sticks_together_once_released_first` (one pad's L3 + R3 fire once, only after
every pad's sticks were let go; one stick or another pad's single stick never; both layouts), game.rs
`both_sticks_quit_from_the_main_menu` (real assets: off does nothing, one stick does nothing, both open the
quit question with Cancel focused, sticks held when the menu appears and other menus do nothing; Original and
Modern), `pad_only_modern_layout_end_to_end` (real assets: the original's triggers do nothing, Modern
drives, Start pauses, B resumes, d-pad name entry, unplugging pauses). End to end with a **uinput** virtual
Xbox 360 pad read by gilrs (a developer probe, not in the public repository): detection, the Original and
Modern reports, the hat d-pad, unplugging, and the three-pad rescan.

Force feedback and wheels: `ff::tests` (the effect model), `ff_out` (units; `plays_on_a_device_node`, ignored,
plays one of each effect on `NFSU2_FF_NODE`), `host::tests::wheel_and_force_feedback` (a scripted G29 with
`connect 1 046d c24f ff`: wheel layout and axes, no default pedals, autocenter off, the front-end spring, the
gain), `host::tests::gilrs_finds_a_virtual_ff_wheel` (ignored, the same through gilrs), `drive::tests::
wheel_steering` (0x4061C0), game.rs `wheel_force_feedback_at_any_frame_rate` (real assets, a race: the
damper and the steering force play, the same calls on the same ticks at 30, 60, 120 and 200 fps). The two
ignored tests ran against a uinput force-feedback wheel (a developer script, not in the repository) that
logged every upload and play: spring, damper, constant, sine → square (released and made again), the
150 ms square jolt with its 20 ms fade, the negative damper, gain and autocenter all arrived. gilrs itself
uploads one rumble-style periodic effect (magnitude 0) on such a device.

## Open

- Force feedback on a real wheel (strength and direction of the steering force by feel; Linux drivers that
  lack effects), and on Windows at all (a DirectInput output is to come). The steering force's sign follows
  DirectInput's +X → evdev 0x4000 mapping (SDL, Wine), not checked on hardware.
- Which axes and buttons gilrs gives a real wheel (`Layout::Wheel` reads gilrs's left stick, LeftZ, right
  stick and RightZ, which is how gilrs names unmapped ABS_X / Y / Z / RX / RY / RZ).

- Rumble on a real pad (the uinput pad has no force feedback; gilrs's force feedback needs write access to the
  event node).
- PlayStation, Switch and Steam Deck on real hardware (the mapping comes from gilrs's SDL database; checked
  here only with the Xbox 360 ids).
- Which device drives (event 1 → 0x571A30), the second byte of the menu stick rows (+0x13), the device arrows'
  Blink, and the Controls page with a pad under Wine (no pad in the prefix).
- Bindings are per layout. No text entry other than the profile name (MC_Main) was found that would need pad
  input.
- The windowed-mode system cursor belongs to the mouse's own topic.
