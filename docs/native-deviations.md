# Native deviations

This is the single list of every place where the default build knowingly
differs from the original game (SPEED2.EXE), and of every optional extra with
the switch that controls it.

## 1. The rule

1. The default is exactly the original game: its rules, its timing, its look and its sound.
2. Every extra is opt-in, off by default, and can be turned off again (per row, per group, or all at once with **Restore Native** on the Enhancements hub).
3. The few deliberate deviations of the default build are listed in section 2 with the way back to the original behaviour.
4. Performance improvements are allowed only when the native setting does not change gameplay or look (section 4).
5. Anything else that differs from the original is a defect, not a decision (section 5).

## 2. Deliberate deviations of the default build

| What | Original | This build | Why | Original behaviour |
|---|---|---|---|---|
| Display mode | First run is 640 x 480, full screen, image stretched; no aspect setting; vsync always off | The window or full-screen mode follows the monitor and the image keeps its aspect ratio. The first run currently opens a 1280 x 960 window; following the desktop resolution is the agreed target (to be finished) | 640 x 480 stretched is unusable on current screens; it is neither gameplay nor art | Display page: Resolution 640 x 480, Aspect Ratio = Stretched (Original). A single "Original display mode" option that sets both is being added |
| Extra resolutions and widescreen | Six modes, 640 x 480 to 1600 x 1200, 4:3 | Adds 1920 x 1080, 2560 x 1440, 3840 x 2160 and the monitor's own modes including ultrawide; Aspect Ratio "Wide" widens the view instead of stretching it | Modern displays | Aspect Ratio = Stretched (Original); pick one of the six native resolutions |
| Vsync Off Mode | Immediate presentation (tearing) | "No Tearing" (mailbox presentation) when vsync is off | Smooth uncapped output without tearing | Display page: Vsync Off Mode = Immediate |
| World update rate | One world update per main-loop frame, at most 0.05 s long, so the original itself plays differently at different frame rates. One visible case: its weather update (0x61F130) is skipped for a frame of 0.009 s or less, so at its own 120 fps the rain amount, the wetness and the rain tint never change (docs/formats.md, Rain) | The world always advances in fixed 1/60 s steps at every display rate, following the original's 60 fps tick pattern; frames in between are interpolated; drawing is never capped. At 60 Hz this is exactly native | The original at 60 fps is the reference; gameplay must be identical at 30, 60, 120 and 200 fps | None needed at 60 Hz. The optional Frame Rate Cap (Performance) only limits drawing |
| Light trails when probes cost too much | One streak-buffer commit per main-loop frame, from that frame's scene | Exactly the 60 fps commits at any display rate (each tick's commit from that tick's own frame, drawn offscreen where no displayed frame stands on the tick). When those extra renders would cost the machine over a quarter of its time (measured, e.g. a GPU that cannot hold 60 fps), each displayed frame commits the ticks it took from its own scene: the same number of commits per second, the commit's scene up to one frame interval later than its tick (tiny invisible rounding, reported in docs/uncapped-fps.md "Streak buffer on ticks"). A steady 60 Hz needs no extra renders | Never lose frames to exactness on a slow machine | None needed at 60 Hz; the form is chosen from measured work |
| High refresh rates | Tuned for 60 Hz | Uncapped drawing at any rate. Shop and garage drive-in and drive-out scenes are not held to 30 fps; cut-scenes still are | Modern displays | Frame Rate Cap / Power Saving (Performance) |
| Loading screens | Every loading screen is always shown, including the game start's | A loading screen is shown only when its load takes longer than 200 ms. The title screen is not a loading screen and is unchanged | Loads are much shorter on modern machines; a flashing screen helps nobody | No switch. An opt-in value in options.txt (`enh.boot_loading_min_ms`, no menu row) keeps the game-start screen up for at least that many milliseconds |
| Skipping scenes | Movies and in-engine scenes follow their own skip rules | Enter skips every movie and in-engine scene | Player convenience | No switch yet; an off switch that restores the native skip rules is planned |
| Front-end animation clock | Menu scripts step trunc(frame seconds x 1200) ms per frame, at most 20 ms (0x5662D0, 0x558DAB): 1.2x real time at a steady 60 fps, slower below 60 fps (0.6x at 30) and at rates where 1200/fps is not whole (0.96x at 144) | Always 1.2x real time | The original at 60 fps is the reference; menus must behave the same at every display rate | None needed at 60 Hz |
| Language | The language and movie file names come from the executable's region byte (the reference executable is a French-region build that asks for `_fr` movies) | The language is taken from the installed game files (for example the `_en` movies and the installed language files) | The executable's region byte does not describe the player's data | None; the installed files decide |
| Movie files | The reference executable builds `_fr` movie names (0x51C740, the constant "_fr.vp6" at 0x79B1EC; `<name>.vp6` only for blank, FMVOpening, the four neighbourhood flybys and THX_logo_fr) and skips a movie whose file is missing (0x566250). With English data it therefore skips the EA and THX logos, the PSA, the career's scenes, the tutorials and Scene15 (seen under Wine: the boot goes from blank.vp6 to FMVOpening, Start New Career finds no scene01_fr.vp6; docs/formats.md "Boot sequence") | The built name first; when that file is missing, the installed language's file (`<name>_en.vp6`, THX_logo.vp6 for THX_logo_fr), so the English data's logos, scenes and tutorials play; a movie with neither file is skipped as in the original | A language mismatch between this build and the player's data is not the game's design | None; with `_fr` files installed the clone plays them, as the original |
| Controller layout | DirectInput presentation: pad triggers and buttons as Windows and the original give them | Controller Layout defaults to Auto: standard gamepads use the Modern layout (analog triggers, Start pauses, shoulder buttons page the menus, pad button names); other devices stay Original | A gamepad that works properly out of the box | Controller group: Controller Layout = Original (this setting is a device setting; Restore Native leaves it alone) |
| Gamepad hot-plug | Devices are read at start | A pad can be plugged in or out while the game runs | Modern expectation | None needed |
| Mouse cursor | Shown for 2 s after a mouse move, also while driving and on the screens before the menus (the splash included) | Never shown while driving or before the menus; the system cursor is hidden over the window | Cleaner picture | None |
| Audio device watchdog | None | The audio output is reopened if the device stops delivering | Device changes and sleep on modern systems | None |
| Performance overlay | None | F3 cycles a hidden / Minimal / Standard / Full overlay with FPS and frame times (hidden by default); Shift+F3 hides it | Measuring | Performance group: Performance Overlay; or never press F3 |
| Font and 2D rendering | 640 x 480 canvas, original filtering | Text and 2D art may use modern filtering (sharper edges, better sampling) so that they look native at high resolutions; no UI or font upscaling is applied unless HD UI + Maps is on | Look native at today's resolutions | None |
| First-run game detection | Registry-based install | Finds and checks the game folder on the first run; the game's saves are read and written in the native profile format; extra settings go to a separate options file | Running on the player's own files | None |

Not deviations (native on purpose): the master-volume law (the original is
silent near 10 percent; an optional smooth curve is planned as an extra), the
original's Customize Display rows and their values, the physics step contents
at 60 Hz, all gameplay data.

## 3. Extras (opt-in)

All extras live under **Display > Enhancements** (the settings hub). Every
default below is the original's behaviour. The hub shows how many rows of each
group differ from native, a Compare switch to preview the original look, a
filter (all / changed / costly), a per-group **Reset Group to Native** and a
global **Restore Native**. Values are stored in the options file as `enh.*`
lines, written only when they are not native.

Count: the hub has **87 rows** in 8 groups: 86 stored options and 1 action
button (Generate Texture Pack). Of those options, 78 appear as an entry in the
F3 overlay's Deviations rows and the crash reports' "deviations on" line; the other 8 are the four
overlay diagnostics, Lighting Strength and the three HDR calibration values,
which have no entry of their own. The code's tests set every option away from
native and assert **79** entries: the rows' own plus two extra entries from the
Detail Distance row (it covers three stored values). The Display page's Aspect
Ratio and Vsync Off Mode add two more entries outside the hub. The
legacy Reversed-Z row is hidden unless something switched it on.

### 3.1 Display (4)
| Option | Default | What it changes |
|---|---|---|
| HDR Output | Off | Sends an HDR signal (HDR10 or scRGB) where the display supports it |
| Peak Brightness | Auto | HDR calibration: display peak, 400 to 10000 nits |
| Paper White | Auto | HDR calibration: brightness of the original's white, 80 to 400 nits |
| SDR Gamma | Gamma 2.2 | How the original's image is decoded for HDR: 2.2, sRGB or 2.4 |

### 3.2 Image Quality (17)
| Option | Default | What it changes |
|---|---|---|
| Anti-Aliasing | Native (the original's own AA row) | FXAA, SMAA 1x, MSAA 2x/4x/8x, SSAA 2x/4x/8x/9x/16x. When a choice here or below overrides it, the original's Full Screen Anti-Aliasing row reads "Disabled - Enhancements" and keeps its saved value |
| MSAA Override | Auto | Sets the multisample count under the choice above (Off, 2x to 16x) |
| AA Post Filter | Auto | Adds FXAA, SMAA or SMAA + FXAA on top |
| Sharpening | Off | Contrast-adaptive sharpening of the final image (Low, Medium, High) |
| Texture Filtering | The original's value | Adds anisotropic filtering above the original's tiers |
| Post-Processing | Native | High-precision scene and screen passes (High, Ultra): smoother glow, no banding |
| LOD Fade | Off | Scenery and car detail changes cross-fade instead of popping |
| Enhanced Reflections | Off | Surface-aware car reflection shading and filtered road reflections |
| Road Reflection Res | The original's value | Higher road-reflection resolution (above the original's top tier) |
| Screen-Space Reflections | Off | Reflections of on-screen objects on wet roads and paint (High, Ultra; the ray-traced tier needs a restart) |
| Enhanced Car Paint | Off | Clear-coat, metallic flake and pearl shift per paint type |
| Enhanced Glass | Off | Windows and lamp covers with refraction and sharper reflection |
| Car Reflection Res | The original's value (128) | Larger car environment maps |
| Reflection Update Rate | The original's value | More faces of the car reflection updated per frame |
| Car Detail Distance | The original's value | Keeps detailed car models farther away |
| Smooth Car Shapes | Off | Rounds low-poly curves of cars at load (applies to cars loaded next) |
| Fix Original Model Errors | Off | Small load-time corrections of errors in car and rim models |

### 3.3 Lighting & Effects (17)
| Option | Default | What it changes |
|---|---|---|
| Lighting | Original (the baked light) | Modern lighting tiers lit by lamps, neon and signs with shadows; the top tier uses ray queries where the GPU can |
| Lighting Strength | Mid value (only offered with Modern Lighting) | From classic (baked look kept) to bold (deeper dark, stronger pools) |
| Ambient Occlusion | Off | Screen-space contact shadows |
| Shadow Quality | The original's value | Higher-resolution and softer car shadow tiers |
| Dynamic Lamps | Off | Street lamps light the road and cars (replaced by Modern Lighting when that is on) |
| Lamp Shadows | Off | Nearby lamps and headlights cast soft shadows |
| Contact Shadows | Off | Soft darkening right under each car |
| Headlight Beams | Off | Every car's headlights as a projected beam; Next-Gen adds light in the air, tail and underglow light |
| Light Probes | Off | Cars pick up the colour of nearby neon and lamps |
| Surface Detail | Off | Material roughness and normal-mapped response to lights |
| Volumetric Fog | Off | Light scattered in the night air |
| Sky | Original | A procedural night sky (moon, stars, clouds) |
| Particles | Original | Smoke as many soft lit puffs; same emitters |
| Sparks & Fire | Original | Lit, light-casting sparks and flames; same emitters and timing |
| Motion Blur | The original's value | Camera and per-object blur tiers above the original's |
| Depth Of Field | The original's value | Cinematic depth of field in scenes, replays, garage and photo mode; never while driving |
| Ambient Detail | Off | Litter, props, leaves, steam and moths; visual only |

### 3.4 Weather & Water (7)
| Option | Default | What it changes |
|---|---|---|
| Wet Roads | Off | Rain-driven ripples, puddles and streaked road reflections |
| Wet Car | Off | Cars get wet with rain and spray |
| Rain Detail | Off | Finer lit rain, ground splashes, wet-air glow (On, High) |
| Spray & Splashes | Off | Wheel spray and mist behind cars on wet roads |
| Wet Road Interaction | Off | Tracks, wakes, rooster tails, mirrored lights (includes Wet Roads and Spray) |
| Lens Drops | Off | Extra drops on the screen, wiped by speed |
| Animated Water | Off | Flowing fountains, pools and streams (On, Realistic, Next-Gen) |

Where and when it rains stays the original's.

### 3.5 Performance (11)
| Option | Default | What it changes |
|---|---|---|
| Render Scale | 100% | Draws the 3D scene at 50 to 100% of the screen; the HUD stays sharp |
| Upscaling | Off | Temporal upscaler, AMD FSR 1 or FSR 2 tiers |
| Detail Distance | Native | Scenery detail and draw distance; the top tiers keep all of the city loaded and stop pop-in |
| Reversed-Z Depth | Off (row hidden unless on) | Legacy: no visible effect in any tested scene |
| Frame Rate Cap | Off | Caps drawing; the world rate is unaffected |
| Low Latency | Off | One frame in flight, input read after the display wait |
| Power Saving | Off | Frame caps where nothing the player watches changes |
| Performance Overlay | Hidden | The F3 overlay's layout (Minimal, Standard, Full) |
| Overlay Position | Default | Overlay corner |
| Overlay Scale | 100% | Overlay size |
| Performance Log (CSV) | Off | Writes frame times to a CSV file |

### 3.6 Gameplay & Accessibility (18)
| Option | Default | What it changes |
|---|---|---|
| Field Of View | 0 | Degrees added to the player cameras (scene and replay cameras keep theirs) |
| HUD Scale | 100% | Size of the in-game HUD |
| HUD Layout | Centered | On screens wider than 16:9, Full Width pushes side elements to the edges |
| HUD Colours | Native | Colour-vision palettes (deuteranopia, protanopia, tritanopia) |
| Minimap Size | 100% | 70 to 150% |
| Speedometer Size | 100% | 70 to 150% |
| Race Info Size | 100% | 70 to 150% |
| Reduced Flashing | Off | Caps full-screen flashes |
| Lists Open on Installed | Off | Custom gauge and colour lists open on the fitted item |
| Outrun Variety | Off | A parked encounter racer that has been on the road gets a new car |
| Traffic Density | Native | More (1.5x) or Dense (2x) free-roam traffic; races keep the native pool |
| Replay Cameras | Native | Extra replay angles and slow-motion steps |
| Photo Mode in Pause Menu | Off | Adds Photo Mode to the pause menu |
| Skip Title Screen | Off | Title Screen: after the opening movies the profile step comes at once, without the title screen and its 4 s before Enter is taken (0x4C564E). Movies and Title Screen: the profile step at the start, without the opening movies too (docs/start-and-menus.md) |
| Mark Spot Key | Off | A key (F9 to F12) writes a report of the spot you are looking at |
| Durable Saves | Off | Flushes profile writes to disk before and after the rename |
| Save Backups | 0 | Keeps the last N copies of a profile before each save |
| Crash Reports | Off | A crash writes a local report; nothing leaves the machine |

### 3.7 Controller (8)
| Option | Default | What it changes |
|---|---|---|
| Controller Layout | Auto (see section 2) | Original, Modern or Auto |
| Stick Dead Zone | 0% | Dead zone for sticks |
| Trigger Dead Zone | 0% | Dead zone for triggers |
| Stick Saturation | 100% | Where the stick reaches full deflection |
| Steering Linearity | 1.00 | Steering response curve |
| Rumble | 0% | Controller vibration strength |
| Pause on Disconnect | Off | Pauses a race or free roam when the driving pad is unplugged |
| Safe Axis Binding | Off | Capturing an axis on the Controls page keeps the roles of the other axes (the original clears them) |

### 3.8 Packs (5)
| Option | Default | What it changes |
|---|---|---|
| HD UI + Maps | Off | Uses an installed HD interface and detailed-map pack (needs a restart) |
| Reload Packs When Changed | Off | Reloads pack files live when they change |
| World Texture Pack | Off | Sharper world and car textures: an imported pack or one generated from your files |
| Generate Texture Pack | Action | Builds the texture pack from your game files |

Settings that apply later: Smooth Car Shapes and Fix Original Model Errors on
the next car load; Traffic Density on the next free-roam load; the texture
packs when the city or a car loads next; HD UI + Maps after a restart.

Packs are never shipped: HD UI + Maps and imported texture packs are files the player provides, and
the generated packs are made on the player's machine from their own game files.

Not in the hub: `enh.boot_loading_min_ms` and `overlay.hz` exist only in the
options file. Some combinations are resolved for you (for example Modern
Lighting replaces Dynamic Lamps); the page says so and never changes the
stored value.

## 4. Performance changes that do not alter the native look or gameplay

These need no switch because the same frames, sounds and simulation come out,
faster. They are checked by comparing images and simulation state against the
build without the change.

- A cache of the converted world and car data, rebuilt automatically; once this build has written its own copy of
  an entry, other builds' copies unused for an hour are removed (docs/asset-cache.md "Pruning").
- After the first frame, the files the next screens read (the boot movies, the main menu's car and showroom, the
  garage's platform, free roam's first files) are read ahead into the system's file cache on a low-priority
  thread that gives way to every load the player waits for (docs/read-ahead.md; `NFSU2_READ_AHEAD=0` turns it off).
- A place's whole track is loaded at once. The original streams its sections into one fixed pool of 60000 KB (0x79DC5C; 43000 KB with two players, 36000 KB on L4RG), loads the sections ahead of the car first (the look-ahead 0x5E4F30: position + speed × 1.5 s, at most 100 m, inside trigger zones) and frees the oldest when it needs room. When its reads fall behind it stops the frame until the player's section is in (0x5E6C20), shows HUD_World_Loading in a race when a needed section would arrive in under 0.5 s (0x5DF820 / 0x5DF730), and draws a section only once it has loaded. With everything loaded none of these can happen; on a machine where the original's reads keep up (any current one) the frames are the same. Traced, not seen under Wine (**inferred** for the visible effect).
- Background and prefetch loading, with first-use graphics pipelines built before play, so transitions and first use do not stall.
- Less main-thread CPU per frame (menus, tuning screen and HUD no longer redo work every frame).
- Render-pass encoding spread over worker threads; a pipeline cache for the remaining pipelines.
- Faster texture pack loading through a cache of prepared arrays (only runs when a pack is on).
- Tiny invisible rounding is allowed where it is reported.

## 5. Known differences being fixed

The following still differ from the original. These are defects, tracked as such, not deliberate choices
(the player's view of each: README.md, "Known issues"):

- **Random draws:** most draws are made at the original's places in its order (docs/rng.md); the car
  render-info draws, the drivetrain's rerolls, the free-roam traffic fill and some audio draws are not yet
  (docs/rng.md "Open"). Line-ups, cars, traffic and encounters follow the original's rules but not its picks.
- **Rival launches:** the rivals leave a standing start about 10 % quicker (their rpm drops from the limiter at
  once; the original's falls about 5500 rpm/s with the wheels spinning; first gear 1–6 frames earlier):
  docs/race-ai-parity.md "Open", A.
- **Physics:** after a wall contact the pivot off the wall differs (collision code); in an engaging clutch the
  front wheels' force is about 5 % off for a frame (docs/cornering-parity.md section 6).
- **AI slip tracking:** reads the wheels' own slip, not the slip the scene mover writes back (0x4057C0,
  0x5A2B70; docs/native-fixes/slip-readers.tsv).
- **Replay cameras:** the Jump shot's flight prediction (0x5EC3E0) is not ported; Jump is chosen only for a car
  upside down (docs/replay-cameras.md).
- **Not yet compared under Wine:** a tournament's Next Race (Restart was), a save made after a long session
  loaded back into the original (docs/world-systems-parity.md section 5).
