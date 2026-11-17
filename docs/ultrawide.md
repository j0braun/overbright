# Ultrawide and super-ultrawide (clone deviation, 2026-11-04)

Approved design. Applies to Aspect Ratio **Wide** only (the
clone default); **Stretched (Original)** keeps the native stretched canvas
and Vert− projection untouched. Background, native facts and the general
wide-screen rules: docs/formats.md "Display modes and aspect".

## Native facts (verified, from the assembly; no new trace needed)
* 0x5C7ED0 (view projection): m00 from the camera angle (+0xC4) alone;
  h / w = target +0x18 / +0x14 (0x5C7F66..0x5C7F74) scales m11, so a
  screen wider than 4:3 loses height (Vert−). Views 1, 2, 4, 5 take the
  extra factor 0x7A27E8 / 0x7A27E0 (0x5C7F89).
* The FE / HUD 640 × 480 canvas is stretched over the target (0x5C5030).
* The original has no ultrawide handling; nothing was checked under Wine
  for this work (the native look is the stretched one, already documented).

## What the clone does
| Item | Behaviour | Code | Status |
|---|---|---|---|
| 3D views (driving, free roam, garage/shop, showcase, NIS, track/replay, magazine cover) | Hor+: every camera with `game_fov` goes through `display::view_aspect` — m11 as at 4:3, m00 × (4/3)/aspect, so the vertical angle is the same at 4:3, 16:9, 21:9, 32:9 and the centre 4:3 is the native picture. The resolution fixes (2026-11-12): narrower than 4:3 (5:4, portrait) the same rule, the 4:3 picture with its sides cropped (was the native Vert−, ≈ 161° vertical at 9:16; docs/resolutions.md). Culling uses the same matrix (frustum planes), so the sides are populated. | `render::camera::projection_between`, `display::view_aspect` | existed; checked at 4 sizes |
| HUD | Side elements anchored to a centred **16:9 safe area** by default; Enhancements row **HUD Layout: Centered / Full Width** sends them to the screen edges | `display::SAFE_SPREAD`, `UiFrame::hud_reach`, `Place::hud`, `frontend::element_place`, `qol::Enhancements::hud_full_width` (`enh.hud_full_width`) | new |
| Front end | Menus a centred 4:3 box; side elements (help-bar name / buttons, `Chyron_FE`) at the 16:9 safe area's edges (were at the screen / 2.4:1 edges); fills (dims, fades, letterbox, help-bar strip) still reach the screen edges; the 3D garage behind is Hor+ | `UiFrame::reach` | changed |
| Loading screens, movies | 4:3 picture pillarboxed (black flat quad under it) | `screens::Loading`, movie object | existed; checked |
| NIS (in-engine scenes) | Hor+ only adds picture at the sides; the authored 4:3 centre and vertical framing are kept (no crop); letterbox bars span the width | event / track cameras via `game_fov` | existed; checked |
| Front-end 3D scenes capped at 21:9 (the menus capped at 21:9 on wider screens) | The garage behind the menus (main, career, profile, Customize, car lot, parts / paint / perf shops) and the Showcase and shop-entry (`ShopEngage`) menus over the world show at most a centred 2.4 : 1 region; beyond it black bars, fading to clear over 4 % of the height at their inner edge, drawn under every 2D quad. Hor+ keeps the vertical angle, so the open region is exactly the 21:9 projection's picture (only post-pass taps at the bar edge see the hidden sides). 2.4 so 2560 × 1080 (2.37) and 3440 × 1440 (2.39) are uncapped; 3840 × 1080 bars 624 px (fade 43), 5120 × 1440 832 px (fade 58). 2D menus stay in the 16:9 safe area, inside the open region. Driving, free roam, races, NIS, shop drive-in scenes and replays stay full width. | `display::SCENE_CAP`, `scene_bars`, `set_scene_capped` (set per frame by `fe_scene_capped` in `bin/game.rs`), bars in `render::ui::record` | new |
| Mouse | Hit-testing inverts the same per-place reach (`frame.reach_x(place)`): `frontend::mouse::canvas_point`, `Game::mouse_point`, the `mouse_main` flow | | changed with the reach |

Reach (pixels the side elements move out of the centred 4:3 canvas):
`display::side_reach` = floor((min(w, h·16/9) − canvas w) / 2), the whole
width when the screen is within 1 % of 16:9 (1366 × 768). So 4:3, 5:4,
16:10 and 16:9 output is unchanged (byte/pixel-identical, below).

| Size | canvas x | FE / HUD Centered reach | HUD Full Width reach |
|---|---|---|---|
| 2560 × 1080 | 560 | 240 | 560 |
| 3440 × 1440 | 760 | 320 | 760 |
| 3840 × 1080 | 1200 | 240 | 1200 |
| 5120 × 1440 | 1600 | 320 | 1600 |

## Checks
* Tests: `render::display::tests::scene_cap_bars` (cap region and bars), `render::display::tests::ultrawide_reach` (reach table, to_pixels
  per place, mouse round trip), `modes_and_aspect` updated (16:9 safe
  area), `qol::tests::hud_full_width_round_trip_kept_by_presets`,
  options row order test (HUD Layout after HUD Scale), label constants.
* 1280 × 960 and 1920 × 1080: 14 shots (main, career, explore, race,
  qrstart, customize, shop, showcase, intro_nis, movie, options, loading,
  drift) pixel-identical to <base> (`compare -metric AE` 0) except
  within <base>'s own run-to-run noise (animated highlights: main,
  customize, options at 1280 × 960 differ base-vs-base by 50–1355 px as
  well); options_enhancements differs by the new row.
* Evidence: internal evidence, one sheet per size (main, career,
  explore, race start NIS, qrstart, customize, shop, showcase, intro NIS,
  movie, options, Enhancements page) and
  `hud_centered_vs_full_<size>.jpg` (explore and drift, Centered left,
  Full Width right) for 2560 × 1080, 3440 × 1440, 3840 × 1080,
  5120 × 1440 (the 3840 × 1080 and 5120 × 1440 sheets re-shot with the
  scene cap). `mouse_main`, `mouse_options`, `mouse_map`, `mouse_carlot`
  pass at 1920 × 1080, 3440 × 1440, 3840 × 1080.

## Open
* The FE scene cap is per screen kind (garage drawn, Showcase,
  ShopEngage); other menus drawn over the world (pause, World Map, SMS)
  stay full width (gameplay screens).
* At 32:9 the horizontal angle is wide (pure Hor+, as ThirteenAG's
  WidescreenFix); no cap or Field Of View compensation is applied.
* Not compared with the original + WidescreenFix under Wine at 21:9/32:9.
* 2-player HUDs (`2PHud*`) are tagged HUD but unchecked.
