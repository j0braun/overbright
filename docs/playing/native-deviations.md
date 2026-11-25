# Differences from the original game

Overbright plays like the original PC game (`SPEED2.EXE`, version 1.2) unless you change a setting. This page
lists every place where it knowingly differs, and every optional setting with the switch that controls it. If
you find a difference that is not listed here, it is a bug: please report it.

## 1. The rule

1. Out of the box, Overbright is the original game: its rules, its timing, its look and its sound.
2. Every extra is optional and off until you turn it on. You can turn it off again per row, per group, or all
   at once with **Restore Native** on the Enhancements page.
3. The few differences you get without changing anything are listed in section 2, each with the way back to
   the original's behaviour.
4. Speed-ups are allowed only when they change neither how the game plays nor how it looks (section 4).
5. Anything else that differs from the original is a bug (section 5).

## 2. Differences you get out of the box

| What | Original | This build | Why | Original behaviour | Basis |
|---|---|---|---|---|---|
| Display mode | First run is 640 x 480, full screen, image stretched; no aspect setting; vsync always off | The window or full-screen mode follows the monitor and the image keeps its aspect ratio. The first run currently opens a 1280 x 960 window; following the desktop resolution is planned | 640 x 480 stretched is unusable on current screens; it is neither gameplay nor art | Display page: Resolution 640 x 480, Aspect Ratio = Stretched (Original). A single "Original display mode" option that sets both is being added | choice |
| Extra resolutions and widescreen | Six modes, 640 x 480 to 1600 x 1200, 4:3 | Adds 1920 x 1080, 2560 x 1440, 3840 x 2160 and the monitor's own modes including ultrawide; Aspect Ratio "Wide" widens the view instead of stretching it | Modern displays | Aspect Ratio = Stretched (Original); pick one of the original's six resolutions | choice |
| Vsync Off Mode | Immediate presentation (tearing) | "No Tearing" (mailbox presentation) when vsync is off | Smooth uncapped output without tearing | Display page: Vsync Off Mode = Immediate | choice |
| World update rate | One world update per main-loop frame, at most 0.05 s long, so the original itself plays differently at different frame rates. One visible case: its weather update (0x61F130) is skipped for a frame of 0.009 s or less, so at its own 120 fps the rain amount, the wetness and the rain tint never change (docs/formats.md, Rain) | The world always advances in fixed 1/60 s steps at every display rate, following the original's 60 fps tick pattern; frames in between are interpolated; drawing is never capped. At 60 Hz this is exactly the original | The original at 60 fps is the reference; gameplay must be identical at 30, 60, 120 and 200 fps | None needed at 60 Hz. The optional Frame Rate Cap (Performance) only limits drawing | choice; original traced |
| Light trails when probes cost too much | One streak-buffer commit per main-loop frame, from that frame's scene | Exactly the 60 fps commits at any display rate (each tick's commit from that tick's own frame, drawn offscreen where no displayed frame stands on the tick). When those extra renders would cost the machine over a quarter of its time (measured, e.g. a GPU that cannot hold 60 fps), each displayed frame commits the ticks it took from its own scene: the same number of commits per second, the commit's scene up to one frame interval later than its tick (tiny invisible rounding, reported in docs/original/uncapped-fps.md "Streak buffer on ticks"). A steady 60 Hz needs no extra renders | Never lose frames to exactness on a slow machine | None needed at 60 Hz; the form is chosen from measured work | choice |
| High refresh rates | Tuned for 60 Hz | Uncapped drawing at any rate. Shop and garage drive-in and drive-out scenes are not held to 30 fps (their car still takes the 30 fps run's mover steps and is drawn between them); cut-scenes still are | Modern displays | Frame Rate Cap / Power Saving (Performance) | choice |
| Loading screens | Every loading screen is always shown, including the game start's | A loading screen is shown only when its load takes longer than 200 ms. The title screen is not a loading screen and is unchanged | Loads are much shorter on modern machines; a flashing screen helps nobody | No switch. An opt-in value in options.txt (`enh.boot_loading_min_ms`, no menu row) keeps the game-start screen up for at least that many milliseconds | choice |
| Skipping scenes | Movies and in-engine scenes follow their own skip rules | Enter skips every movie and in-engine scene | Player convenience | No switch yet; an off switch that restores the original's skip rules is planned | choice |
| Front-end animation clock | Menu scripts step trunc(frame seconds x 1200) ms per frame, at most 20 ms (0x5662D0, 0x558DAB): 1.2x real time at a steady 60 fps, slower below 60 fps (0.6x at 30) and at rates where 1200/fps is not whole (0.96x at 144) | Always 1.2x real time | The original at 60 fps is the reference; menus must behave the same at every display rate | None needed at 60 Hz | choice; original traced |
| Language | The language comes from the registry string `Language` (a missing value is English; docs/original/tracing/language.md); the movie file names are a separate constant (the reference executable asks for `_fr` movies) | The text language is English (`LANGUAGES/English.bin`) unless another is chosen: in the launcher (Settings › Advanced › Game language, kept in `language.txt`), or with `game --language <name>`. The choices are the installed languages among English, French, German, Italian, Spanish, Dutch, Swedish and Danish; Korean, Japanese, Chinese, Thailand and any other file are listed as not supported. The menus, fonts, credits, save-library texts and the text language's movies follow the choice. A copy without `English.bin` starts in its first supported language. A language whose file is missing is English. The Metric option's default and the number separators follow the language as SetLanguage sets them (French groups with a space and writes `,` as the decimal mark; German to Danish group with `.`). Two of SetLanguage's other effects are not followed: the default profile name's label and the speech bank (inferred) stay English's. Only English is checked against the original's sizes and checksums; another language's file is reported as "not verified". The movie file names are taken from the installed files as in Movie files | The original reads a Windows registry value that Overbright has no use for (there is no installer); the player's choice and the installed files take its place | Choose English, or delete `language.txt` | choice; original traced (SetLanguage 0x512360) |
| Region | The region byte 0x7FA73C is a constant of the executable (6, European, in the European PC 1.2 release); it picks the lot and Quick Race car lists, the random opponents' pool, the shared texture pack, the credits file, the lot's badges and the Best Buy code and vinyl | The region is read from the installed CARS folder: CIVIC and RSX present and CORSA and PEUGOT106 absent is region 0 (US), anything else 6; each table follows the region as the executable's do (docs/playing/region.md) | No executable is read; the car folders are what tells the two releases apart | None; the installed files decide | choice; original traced |
| Movie files | The reference executable builds `_fr` movie names (0x51C740, the constant "_fr.vp6" at 0x79B1EC; `<name>.vp6` only for blank, FMVOpening, the four neighbourhood flybys and THX_logo_fr) and skips a movie whose file is missing (0x566250). With English data it therefore skips the EA and THX logos, the PSA, the career's scenes, the tutorials and Scene15 (seen under Wine: the boot goes from blank.vp6 to FMVOpening, Start New Career finds no scene01_fr.vp6; docs/formats.md "Boot sequence") | The built name first; when that file is missing, this install's own name for it, matched without case: `<name>.vp6` without a suffix (the US edition's names; THX_logo.vp6 for THX_logo_fr), the text language's `<name>_<xx>.vp6`, `<name>_en.vp6`, then any one `<name>_<xx>.vp6` (another edition's language, the first by name). So the US, European and single-language editions' logos, scenes and tutorials play. A movie with none of these files is skipped as in the original, with one log line naming the files tried. The game-folder check (launcher, `--check-game`, start log, support file) reports the same resolution (`src/game_assets.rs`) | Each edition's executable names its own data's files; a mismatch between this build and the player's data is not the game's design | None; with `_fr` files installed Overbright plays them, as the original | choice; original traced and observed |
| Controller layout | DirectInput presentation: pad triggers and buttons as Windows and the original give them | Controller Layout defaults to Auto: standard gamepads use the Modern layout (analog triggers, Start pauses, shoulder buttons page the menus, pad button names); other devices stay Original | A gamepad that works properly out of the box | Controller group: Controller Layout = Original (this setting is a device setting; Restore Native leaves it alone) | choice |
| Gamepad hot-plug | Devices are read at start | A pad can be plugged in or out while the game runs | Modern expectation | None needed | choice |
| Mouse cursor | Shown for 2 s after a mouse move, also while driving and on the screens before the menus (the splash included) | Never shown while driving or before the menus; the system cursor is hidden over the window | Cleaner picture | None | choice |
| Audio device watchdog | None | The audio output is reopened if the device stops delivering | Device changes and sleep on modern systems | None | choice |
| Texture editor | None | Started with `--editor`, F7 opens a panel that lists the textures drawn, highlights one with a checker tint, exports originals and shows edited ones at once from an Overbright pack in Overbright's data folder (docs/modding/texture-editor.md) | Modding | Start without `--editor` (the default): the panel and its hotkey do not exist | choice |
| Performance overlay | None | F3 cycles a hidden / Minimal / Standard / Full overlay with FPS and frame times (hidden by default); Shift+F3 hides it | Measuring | Performance group: Performance Overlay; or never press F3 | choice |
| Diagnostics key and events | None | Ctrl+F8 writes the game's recent window, display, GPU and screen events to the log and saves a support file in the logs folder; the same events go to the log on a crash, a hang, or a lost GPU device (docs/troubleshooting.md). The key still reaches the game as before, and nothing the game simulates, draws or plays changes | Players' bug reports (a black screen, a start that ends at once) | None needed; never press Ctrl+F8 | choice |
| Font and 2D rendering | 640 x 480 canvas, original filtering | Text and 2D art may use modern filtering (sharper edges, better sampling) so that they look like the original at high resolutions; no UI or font upscaling is applied unless HD UI + Maps is on | Look like the original at today's resolutions | None | choice |
| First-run game detection | Registry-based install | Finds and checks the game folder on the first run; the game's saves are read and written in the original's profile format; extra settings go to a separate options file | Running on the player's own files | None | choice |

Kept as in the original on purpose: the master-volume curve (the original is
silent near 10 percent; an optional smooth curve is planned), the original's
Customize Display rows and their values, what each physics step does at 60 Hz,
and all gameplay data.

## 3. Optional settings

All optional settings live under **Options → Display → Enhancements**. Every
default below is the original's behaviour. The page shows how many rows of each
group differ from the original, a Compare switch to preview the original look, a
filter (all / changed / costly), **Reset Group to Native** for each group and
**Restore Native** for everything. Values are stored in the options file as
`enh.*` lines, written only when they differ from the original's.

Count, for developers: the page has **88 rows** in 8 groups: 87 stored options and 1 action
button (Generate Texture Pack). Of those options, 79 appear as an entry in the
F3 overlay's Deviations rows and the crash reports' "deviations on" line; the other 8 are the four
overlay diagnostics, Lighting Strength and the three HDR calibration values,
which have no entry of their own. The code's tests set every option away from
the original's value and assert **81** entries: the rows' own plus two extra entries from the
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

### 3.5 Performance (12)
| Option | Default | What it changes |
|---|---|---|
| Render Scale | 100% | Draws the 3D scene at 50 to 100% of the screen; the HUD stays sharp |
| Upscaling | Off | Temporal upscaler, AMD FSR 1 or FSR 2 tiers |
| Detail Distance | Native | Scenery detail and draw distance; the top tiers keep all of the city loaded and stop pop-in |
| Reversed-Z Depth | Off (row hidden unless on) | Legacy: no visible effect in any tested scene |
| Frame Rate Cap | Off | Caps drawing; the world rate is unaffected |
| Low Latency | Off | One frame in flight, input read after the display wait |
| In the Background | Native (pause) | Keep running, or keep running drawn at 30 FPS, while the window is not focused (the original pauses) |
| Power Saving | Off | Frame caps where nothing the player watches changes |
| Performance Overlay | Hidden | The F3 overlay's layout (Minimal, Standard, Full) |
| Overlay Position | Default | Overlay corner |
| Overlay Scale | 100% | Overlay size |
| Performance Log (CSV) | Off | Writes frame times to a CSV file |

### 3.6 Gameplay & Accessibility (19)
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
| Traffic Density | Native | More (1.5x) or Dense (2x) free-roam traffic; races keep the original's pool |
| Replay Cameras | Native | Extra replay angles and slow-motion steps |
| Photo Mode in Pause Menu | Off | Adds Photo Mode to the pause menu |
| Skip Intro Movies | Off | On: the opening movies (EA, THX, the warning screen, the intro) are passed as missing movie files are (0x566250); the title screen still follows unless Skip Title Screen is on (docs/original/start-and-menus.md). Also in the launcher |
| Skip Title Screen | Off | On: the profile step comes where the title screen stood, without the title screen and its 4 s before Enter is taken (0x4C564E); independent of Skip Intro Movies (docs/original/start-and-menus.md). Also in the launcher |
| Mark Spot Key | Off | A key (F9 to F12) writes a report of the spot you are looking at |
| Durable Saves | Off | Flushes profile writes to disk before and after the rename |
| Save Backups | 0 | Keeps the last N copies of a profile before each save |
| Crash Reports | Off | A crash writes a local report; nothing leaves the machine |

### 3.7 Controller (9)
| Option | Default | What it changes |
|---|---|---|
| Controller Layout | Auto (see section 2) | Original, Modern or Auto |
| Stick Dead Zone | 0% | Dead zone for sticks |
| Trigger Dead Zone | 0% | Dead zone for triggers |
| Stick Saturation | 100% | Where the stick reaches full deflection |
| Steering Linearity | 1.00 | Steering response curve |
| Rumble | Off | Low, Medium or High: the pad shakes on crashes, bumps and hard speed changes (the original has no rumble for XInput pads) |
| Pause on Disconnect | Off | Pauses a race or free roam when the driving pad is unplugged |
| Safe Axis Binding | Off | Capturing an axis on the Controls page keeps the roles of the other axes (the original clears them) |
| Quit with Both Sticks | Off | Pressing both sticks (L3 + R3) together on the main menu asks the quit question, as the Q key does (the original has no pad shortcut) |

### 3.8 Packs (4)
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

Map mods and the map editor (docs/modding/map-editor.md): with no map mod listed in `maps/enabled.txt`, worlds and races
are the original's; the editor is reachable only after a start with `--editor`.

## 4. Speed-ups that change neither the look nor the play

These need no switch: the same pictures, sounds and simulation come out, only
faster. Each is checked by comparing pictures and simulation state with a build
without the change.

- A cache of the converted world and car data, rebuilt automatically; once this build has written its own copy of
  an entry, other builds' copies unused for an hour are removed (docs/developing/asset-cache.md "Pruning").
- After the first frame, the files the next screens read (the boot movies, the main menu's car and showroom, the
  garage's platform, free roam's first files) are read ahead into the system's file cache on a low-priority
  thread that gives way to every load the player waits for (docs/developing/read-ahead.md; `NFSU2_READ_AHEAD=0` turns it off).
- A place's whole track is loaded at once. The original streams its sections into one fixed pool of 60000 KB (0x79DC5C; 43000 KB with two players, 36000 KB on L4RG), loads the sections ahead of the car first (the look-ahead 0x5E4F30: position + speed × 1.5 s, at most 100 m, inside trigger zones) and frees the oldest when it needs room. When its reads fall behind it stops the frame until the player's section is in (0x5E6C20), shows HUD_World_Loading in a race when a needed section would arrive in under 0.5 s (0x5DF820 / 0x5DF730), and draws a section only once it has loaded. With everything loaded none of these can happen; on a machine where the original's reads keep up (any current one) the frames are the same. Traced, not seen under Wine (**inferred** for the visible effect).
- Background and prefetch loading, with first-use graphics pipelines built before play, so transitions and first use do not stall.
- Less main-thread CPU per frame (menus, tuning screen and HUD no longer redo work every frame).
- Render-pass encoding spread over worker threads; a pipeline cache for the remaining pipelines.
- Faster texture pack loading through a cache of prepared arrays (only runs when a pack is on).
- Tiny invisible rounding is allowed where it is reported.

## 5. Known differences still being fixed

These still differ from the original. They are bugs, not choices (what you notice of each: README.md,
"Known issues"):

- **Random draws:** the draws are made at the original's places in its order (docs/original/rng.md): a
  race's set-up (line-up, looks, grid, world build, traffic) and free roam's entry match the
  original's draw for draw from the same state; still different are the front-end cars' render
  infos (one at Resume), some in-play audio draws and the traffic's route walk (docs/original/rng.md
  "Open"), so a session reaches those set-ups with another state and its picks still differ.
- **Physics:** a car held against a wall turns off it the same way as in the original, but leaves it
  sooner or later: from the second push of a hold the contacts' timing differs (docs/notes/parity/cornering-parity.md
  section 6).
- **AI:** free-roam encounter racers hit traffic cars about six times as often as in the original, a drag
  rival goes back to its lane more slowly after a shove and can be totaled by it, and Quick Race circuit
  traffic dodges and queues more (docs/original/coverage/, the gap lists).
