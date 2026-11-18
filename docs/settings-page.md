# The settings page (2026-11-06)

The settings rework, second half: one streamlined page for every clone setting, built on the settings model
(`docs/settings-model.md`). Nothing here is native — the original has no such page; it is a clone deviation on the
Enhancements entry of the Display options (agreed deviation "Enhancements page"; the settings rework plan). The original's own screens (Audio / Display / Gameplay / Player, Customize Display, the pause's) are
unchanged: same rows, values, look and flow. Defaults stay native; options.txt keeps every key.

* Code: `src/frontend/settings_page.rs` (rows, presets, 60 fps pick, detail lines, reset, strings),
  `options.rs` (`Category::Settings`, the `Ux*` row ids and labels, `OptionsScreen::key` / `quads`, 7 rows + a
  4-line detail on the page), `qol.rs` (`Preset::Extreme`, the page's preset names, `COMPARE`), `game.rs`
  (navigation, shots `settings_<group>`).
* Evidence: the maintainer's private captures (screens at 1920×1080, 3840×2160, 3440×1440; navigation sequence; preset costs).

## Structure

Options → Display → **Enhancements** (the hub, 7 rows shown, scroll bar). Its first rows are the **streamlined
graphics page** (section below); then:

| row | what it does |
|---|---|
| Graphics Mode … Rain on the Lens | the main page (below) |
| Advanced | opens the **Advanced page**: Compare, Show, then the group pages Display, Image Quality, Lighting & Effects, Weather & Water, Performance, Packs (value "Native" or "N Changed") |
| Gameplay & Accessibility, Controller | open the group page (not graphics, so they stay on the hub) |
| Restore Native | every enhancement back to the original's value |

On the Advanced page: **Compare** (Current Settings ↔ Original Look: the Original preset's graphics values in force
while on the pages, nothing changed or saved; the title reads "… - Original Look"; C / the pad's X toggles it on any
of these pages) and **Show** (All Settings / Changed Only / Costly Only, ≥ 0.5 ms here: filters the group pages).
Back: a graphics group page → the Advanced page on its row; Gameplay & Accessibility / Controller → the hub; the
Advanced page → the hub on Advanced.

A **group page** lists its rows in the settings model's order (`settings_model::MODEL`: shared rows — the
original's Customize Display row with the clone's tiers above —, enhancements, conveniences), plus the original's
Customize Display rows an enhancement needs, as the same setting (Particle System on Lighting & Effects after
Sparks & Fire; Rain Splatter on Weather & Water after Lens Drops), the HD UI pack's state under its row (Packs),
then **Reset Group to Native**. Display is also the HDR calibration screen (its test pattern shows, as the HDR work's
page). Every row of the old Enhancements page and every Graphics row is on exactly one group page (test
`every_row_on_one_page`); the old Graphics hub, its eight group pages and the HD UI page are no longer linked
(kept as code for their shots and tests; removal left to the maintainer's cleanup).

**Detail line** (4 lines under the 7 rows; selected row): the status first — origin ("Enhancement, off: as the
original." / "Enhancement." / "The original's setting; this value is native, the tiers above it are
enhancements." / "Enhancement over the original's setting." / "Convenience: no change to the picture."), when it
applies (at once / when the cars, the city, free roam load next / after a restart / "this tier after a restart"
for Modern + RT and Ray Traced SSR), the measured cost of the current tier ("Cost ~+0.93 ms at 4K (+0.23 ms at
1920x1080)", or "Saves ~…"; the tiers' `cost` hints, reference desktop GPU, scaled by the output's pixels) —, then the rule
in force from the settings model ("Overridden by Modern Lighting: Modern Lighting lights the street lamps
itself…"), then what the row and its tier do. Overridden / needing / blocked rows are greyed with the reason in
their value ("Overridden by FSR 2", "Needs Particle System") exactly as the settings model's `settings_model::shown`; their
stored value is kept, so switching the other row back restores it (the settings model's test `switching_back_restores_the_value`).

## Graphics mode and quality levels (the main page)

| row | values | notes |
|---|---|---|
| Graphics Mode | Original / Enhanced | Original = the Original preset (every graphics value native) and Customize Display as the original at the World Detail level: byte-identical to SPEED2.EXE's look. Enhanced = the level for the GPU class (`qol::Level::for_class`: Low on an integrated GPU, Medium elsewhere). Detected from the values (`qol::graphics_original`), not stored |
| Enhanced Quality | Low / Medium / High, "Custom" when the values are no level | Enhanced only. Choosing a level writes its rows (`qol::Level::apply`, ask nothing); a row changed on Advanced makes it Custom; choosing a level again overwrites |
| Upscaling | Off / Quality / Balanced / Performance | the temporal upscaler (`enh.upscale` 0–3); FSR 1 / FSR 2 stay on Advanced (an FSR value reads its name) |
| Anti-Aliasing | Native MSAA / TAA / SSAA 2x / 4x / 8x / 9x / 16x | `enh.post_aa` 0, 11, 6–10 (tiers the output cannot take skipped, as on Advanced); SSAA only with Upscaling Off (the existing rule greys the row under Upscaling); MSAA 2x–8x with alpha-to-coverage, FXAA, SMAA and the combination / sharpening rows on Advanced |
| Ray Tracing | Off / On | outside the levels: On = Modern Lighting "Modern + RT" and Screen-Space Reflections "Ray Traced"; Off gives the level's own reflections back. Greyed "Not Supported" where the device has no ray queries (`qol::rt_offered`); applies after a restart |
| HDR Output | Off / On | the Advanced row itself; with it on, **HDR Calibration** opens the Display group (the calibration screen) |
| Motion Blur | Off / On | Customize Display's Motion Blur (the original's speed blur); Camera / Per-Object on Advanced |
| Rain on the Lens | Off / On | Lens Drops (Enhanced only) |
| Advanced | | the Advanced page above |

In Original the Enhanced rows (quality, Upscaling, Anti-Aliasing, Ray Tracing, Rain on the Lens) are not listed;
HDR Output and Motion Blur stay (an output choice and the original's own setting). The page fits one screen in
Original; Enhanced lists 9 graphics rows and scrolls (the UI_Options package has room for 7 rows plus the 4-line
detail).

**The levels** are data (`src/qol/levels.rs`), the same on every GPU class. A level sets every value the presets
reset except the main page's choices (Upscaling and Render Scale; Anti-Aliasing with MSAA / filter / sharpening;
Motion Blur; Rain on the Lens; the two ray-traced tiers), which stay and are not compared:

| row | Low | Medium | High |
|---|---|---|---|
| Post-Processing | Native | High (half glow) | Ultra (full glow) |
| Enhanced Paint, Enhanced Reflections | On | On | On |
| Road Reflection | Very High | Very High | Ultra |
| Car Reflection | High (native) | Very High (256) | Ultra (1024) |
| Car Shadow | Ultra | Ultra | Ultra |
| LOD Fade | On | On | On |
| World Detail (draw distance) | Very High (150 %) | Very High | Ultra (200 %, keep loaded) |
| Car Geometry Detail | High (native) | High | Ultra (200 %) |
| Wet Car | On | On | On |
| Wet Roads | Off | On | On |
| Rain Detail | On | On | High |
| SSAO | Off | Medium (50 %) | Medium |
| Dynamic Lamps, Contact Shadows, Light Probes, Spray | Off | On | On |
| Lamp Shadows | Off | Low | High |
| Headlight Beams | Off | On | On |
| Volumetric Fog | Off | Low | High |
| Screen-Space Reflections | Off | Off | High |
| Particles | Original | Original | Ultra |
| Animated Water | Off | Off | On |
| Wet Road Interaction | Off | Off | On |
| Enhanced Glass | Off | Off | On |
| everything else (Sky, Sparks & Fire, Surface Detail, Smooth Car Shapes, Depth of Field, Ambient Detail, Modern Lighting 1–2, Next-Gen tiers …) | native | native | native |

Medium is the old High preset without its MSAA 8x (an Anti-Aliasing choice) plus Volumetric Fog Low; High is the
old Ultra preset without its SSAA 2x. Low keeps the original's car highlight (no light probes, no screen-space
reflections: `qol::scene_lights_car`); Medium, High and Ray Tracing light the car by the scene
(docs/car-scene-specular.md). Next-Gen tiers and every other row stay reachable on Advanced only.

**Level costs** (measured: `docs/evidence/settings-levels/levels.txt`; `--bench` explore / racing / race start /
rain at clock 162, 300 frames, Anti-Aliasing Native; the heaviest scene's GPU busy median ms; the detail line shows
the estimate for the output as the 60 FPS pick's `preset_ms` does):

| level | desktop 1080p | desktop 4K | integrated 1080p |
|---|---|---|---|
| Original | 0.70 | 2.60 | 15.0 |
| Low | 0.74 | 2.66 | 17.4 (per scene +1.0..+2.8: within Original + 3 ms) |
| Medium | 1.77 | 7.08 | 49.5 |
| High | 3.32 | 11.42 | 100.5 (rain; 64–70 in the other scenes) |

Re-measured on a later main (docs/perf-budget.md "Quiet-machine re-measure"): desktop 0.81 / 0.85 / 1.89 / 3.49 at
1080p and 2.72 / 2.77 / 7.32 / 11.73 at 4K (Original / Low / Medium / High), integrated 14.1 / 15.6 / 49.4 / 103.2 at 1080p
and 8.6 / 9.5 / 27.7 / 57.5 at 1366×768; the desktop figures were taken with another program busy on the GPU.

The integrated figures above are World Detail 5 (the bench's default: 8× MSAA, the bleach bypass, rain splatter),
what a player gets who raises everything. A fresh install on an integrated GPU runs World Detail 3 (the game's
first-run estimate, docs/perf-integrated-gpu.md "World Detail"); there the heaviest scene at 1080p is Original 5.8,
Low 7.7, Medium 28.4, High 67.7 ms, and at 1366×768 3.8 / 4.4 / 14.6 / 37.0 (docs/perf-integrated-gpu.md "The
Enhanced levels at World Detail 3"; measured under the machine's build load, so Medium and High at 1080p are upper
bounds). The detail line's estimate (`LEVEL_COSTS`, `preset_ms`) still takes the World
Detail 5 figures (**open**: a change of what the page shows, left to the settings work).

Low was first High Precision Post + Wet Roads + the 256 car reflection too: +4.3 / +6.3 ms (race start / rain) on
the integrated GPU; those three rows (+3.9 / +2.7, +2.7 in rain, ~+0.7) start at Medium. Medium matches the High
preset's cost (2.41 / 7.20 / 50.9 with its MSAA 8x), High is the Ultra preset's (3.97 / 13.07 with SSAA 2x)
without the SSAA.

**Option files.** Nothing new is stored: the mode and the level are found from the `enh.*` values, so any
options.txt loads with the same values and writes back byte-identical (test
`old_option_files_survive`). A preset's values (`qol::Preset`) read as: Original → Graphics Mode Original; Ultra → Enhanced, High (its
SSAA 2x shown on Anti-Aliasing); Balanced, High and Extreme → Enhanced, Custom (Balanced has fewer rows than Low;
the High preset lacks Medium's fog; Extreme has rows no level sets), each with its own values until a level is picked.
The presets stay defined (`qol::Preset`, `PRESETS`) for the checks (the gate's `extreme` sets, shot targets,
benches, the matrix) and the preset row's code (`RowId::GraphicsPreset`, the 60 FPS pick below), which is not listed on a
page.

**The launcher** offers Graphics Original / Enhanced with the same meaning: Enhanced writes the Low level (the same
on every GPU, so no probe is needed); a file at Medium or High reads "Enhanced Medium (set in game)", one matching no
level "Custom (set in game)" (`launcher::model::Graphics`).

## The old presets and "60 FPS"

The old preset row (no longer on a page):

**60 FPS on this machine** is the last entry. Choosing it applies, once, the costliest preset whose estimated
heaviest-scene GPU time at the current output fits **12.5 ms** (three quarters of a 60 Hz frame: room for the CPU,
frame spikes and unbenched scenes); Original when none fits (the detail line then says even Original needs
~X ms and points to a lower resolution or Upscaling). The row reads "60 FPS (High)" while the values are that
preset. It is a one-time choice of fixed values: no dynamic resolution, nothing changes while playing, no
upscaler is chosen for you (no "60 fps at 4K" preset). Estimates (`settings_page::preset_ms`):
High class — measured at 1920×1080 and 3840×2160, linear in pixels between and beyond (inferred off the two
sizes); Integrated — measured at 1920×1080, proportional to pixels (inferred elsewhere); **Mid — twice the High
class (inferred: no mid-class card measured, as the effects-tiers pass)**.

Measured (internal evidence, `presets-igpu.txt`; perf.sh bench, 2 runs × 300 frames,
`ORDER=alternate`, load average 30–64; heaviest of explore / racing / race start / rain, GPU busy median ms):

| preset | reference desktop GPU 1080p | reference desktop GPU 4K | reference integrated GPU 1080p |
|---|---|---|---|
| Original | 0.85 | 2.92 | 14.8 |
| Balanced | 1.23 | 5.62 | 18.1 |
| High | 2.41 | 7.20 | 50.9 |
| Ultra | 3.97 | 13.07 | 72.0 (Integrated-class Ultra, rain; race start not measured) |
| Extreme | 5.55 | 16.56 | = Ultra (same values on that class) |

Picks: reference desktop GPU — Extreme at 1080p, 1440p and 3440×1440, High at 4K; reference integrated GPU — Balanced at 1280×720,
Original at 1080p (14.8 ms > 12.5: over budget, said so). Test `fps60_follows_measured_costs`.

## Navigation (keyboard, gamepad, mouse)

Same menu keys as every options page (the pad sends them: d-pad, A / B, shoulders, X): Up / Down move (on the
page disabled rows can be focused too, so an overridden row's reason can be read; Left / Right do nothing there),
Left / Right change, Enter opens a group or presses Reset / Restore, Esc goes back (group → hub on its row → the
Display page; leaving the hub turns Compare off and resets Show). On a group page **[ / ]** (LTrigger / RTrigger)
open the previous / next group; **C** (Button1) toggles Compare on any page of it. Mouse: the rows and arrows are
the list's mouse targets as on every options page (hover, click, arrows). Every resolution: the UI_Options
package scales as on the other pages (4:3 panel centred on ultrawide; shots at 3440×1440).

## Strings and label ids

Main page labels 0xC1_04E0_65..6A (Graphics Mode, Enhanced Quality, Ray Tracing, Advanced, HDR Calibration, Rain on
the Lens; unused in this copy and the main checkout). New labels (`options::clone_label`), 0xC1_04E0_80..88, checked unused in this copy, the main checkout and the
parallel copies: 80 Compare, 81 Show, 82 Image Quality, 83 Lighting & Effects, 84 Weather & Water,
85 Gameplay & Accessibility, 86 Packs, 87 Reset Group to Native, 88 Nothing Matches the Filter (Display and
Performance reuse 0x57 / 0x46). Every other text the page adds is in `settings_page::text` (one table keyed by
`Text`, `{}` placeholders filled by `fill`), ready for a translation table; the rows' own help and tier texts stay
in `graphics_table` / `settings_model` as before.

## Tests

`settings_page::main_page::tests`: `the_main_page_follows_the_mode`, `levels_write_rows_and_custom_is_shown`,
`main_choices_step_their_lists`, `old_option_files_survive`, `ray_tracing_is_its_own_choice`; `qol::levels::tests`:
`levels_round_trip`, `custom_is_detected`, `levels_follow_the_car_highlight_rule`; the matrix's `levels` part
(`matrix_covers_every_tier`); shots `options_enhancements` (Original), `settings_main_enhanced`, `settings_advanced`,
`mouse_settings` (hub → Advanced → Compare → Image Quality); resolution matrix screens `settings_hub`,
`settings_main_enhanced`, `settings_advanced`.
`settings_page::tests`: `every_row_on_one_page`, `reset_group_restores_native` (Extreme + QoL values, every group
reset → options.txt identical to the defaults), `preset_row_cycles_with_60fps`, `fps60_follows_measured_costs`,
`rules_show_on_the_page`, `filter_lists_changed_rows`; `qol::tests::presets_follow_the_gpu_class` covers Extreme
(steps down only, no Next-Gen). The settings model matrix (`scripts/checks/matrix.sh`, gate `CHECKS=full`) covers Extreme's
tiers (all are rows' tiers).

## Open

* Rows a later cleanup may remove, now on Advanced only: Ambient Detail, Modern Strength, the FSR 1 upscaling modes,
  FXAA, SMAA + FXAA, the MSAA override and Sharpening (not decided; old files keep loading
  either way, the keys stay known).
* Enhanced lists 9 graphics rows on a page that shows 7 (scrolls); Original fits one screen.

* Mid-class 60 fps estimate is inferred (no card); the Integrated estimate off 1080p is inferred.
* "How it looks" is in words plus the live Compare; per-row before/after preview images are not made (no image
  assets may be shipped from the game; a generated preview would need the pack generator).
* Search by text is not offered (no text entry on a gamepad page); Show filters instead.
* The old Graphics / HD UI pages remain as unlinked code.

## Gate

* `CHECKS=full scripts/checks/gate.sh … <base> extreme`: CHECKS PASS (full, 451 s: build, 1256 tests passed / 0
  failed, selftest, 283 shots 0 failed and 0 new nondeterministic, flows, playthrough, showcase, the matrix 4 ×
  clean, provenance). Parity: Original images 1280×960 / 4K AE 0, 144 Hz window path AE 0, enhanced explore /
  racing Extreme / Extreme FSR AE 0, audio identical, five sim hashes identical; **shots: 27 of 29 AE 0,
  `options_enhancements` and `ui2d` differ (expected: both draw the Enhancements page, now the settings hub)**.
  Binaries differ (md5 of base and new).
* `perfgate.sh run` against `current` (20261106-0446-b7ee3bcf464b): **pass** (GPU −0.9..+0.4 %; rain max 4K VRAM
  +2.9 % reported only, CPU columns n.s.; load average 4–42). The page changes no render path.
