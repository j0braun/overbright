# GPS arrow kept out of reflections (2026-11-04)

Player report: "the GPS arrow should not be visible in the reflections". The player's settings
(the player's options file) have Screen-Space Reflections Full, Wet Roads and
Enhanced Reflections on.

## Native (traced, SPEED2.EXE)
- MARKER_GPS_AID is drawn by 0x5F3CC0 → 0x5EACA0 → 0x5C5940. Its only caller is World_DrawCars
  0x6316E0, at 0x63177A, and only for the view whose number (view +4) equals the player's
  ([player +0x18], compared at 0x631767). 0x5F3AF0 (called just before it) and the dyno rollers
  0x56E900 are behind the same test.
- The frame function 0x5CAAE0 draws view 4 (the road reflection) with 0x5D6AE0, 0x566410, 0x631260,
  0x630DF0, the pools 0x6154A0, 0x5C7E50, then the glows 0x491860 / 0x61AEA0 / 0x61ACF0 / 0x494270
  (block 0x5CAB03–0x5CACB5). The cube faces get 0x60F890 and 0x5D6AE0 (0x5CAD01–0x5CADE9). 0x6316E0
  is in neither, so **the arrow is never in the road reflection or in the car cube**.
- The particle draw Particles_Render 0x48BDF0 (marker icons and rings, the outrun challenge marker,
  smoke) is in neither block either: called at 0x5CAEC8 / 0x5CAED3 (the 0x6316E0 view-3 block) and at
  0x5CB541 / 0x5CB54C (the main view).
- Under Wine (fastgpu, DXVK; a new career's airport free roam with the GPS on the car lot): the arrow
  is above the screen centre and the rear glass and the road show no blue
  (`rear_window_before_after.jpg`, top left).

## The clone
- The planar road capture (native or Enhanced), the cube and the rear-view mirror already culled the
  arrow. Its placement goes in with view flags 0, and the 0x40 of those views culls it
  (`WorldView::culls`). Test: `gps_aid_is_drawn_in_the_main_view_only`.
- **Leak: SSR.** The trace reads the main view's colour and depth, where the arrow is. Car rays (the
  rear glass) and wet-road rays hit it. With the player's settings the 350Z's rear window showed the
  arrow (`rear_window_before_after.jpg`, middle left). It was not the glow sprites: still there with
  NFSU2_SSR_NOGLOW=1. With SSR off it is gone.
- **Fix (SSR only):**
  - `WorldCore::overlay_draws` holds the arrow's draws (the placement's groups, `World::gps_aid`).
  - `draw_wet_mask` draws them first into the SSR mask with `fs_overlay_mask` (world.wgsl). That
    writes no reflection term, the view depth, and tint.a = 1, and writes the mask's depth, so the
    road and cars behind the arrow keep the flag. It runs whenever the mask is drawn, also with Road
    Reflection Detail off (car SSR).
  - In `ssr.wgsl` `fs_trace`, a hit is no hit when an overlay is at or in front of the hit's depth,
    on the hit texel or one trace texel away (`overlay_hit`: bilinear lookup and resolved edges). The
    ray goes on past the arrow as past something thin. Misses keep the planar or cube image, which
    never holds the arrow.
- Native and SSR-off paths are unchanged: the overlay pass exists only in the SSR mask pass. The
  native `--shot intro` was byte-identical before and after (max diff 0). With the player's settings,
  the only pixels that changed were the rear window (bbox 549,558–669,623 of 1280×960).

## Other markers (checked)
- The event, shop and challenge markers, tips and banks are particle effects (`Game::marker_effects`,
  0x6054C0). The clone draws them in the main view only, as the original does. They write no depth,
  so SSR cannot hit them. A ray that hits the surface behind one could take its colour.
  - `--shot marker`, `prompt` (car in the trigger) and `shopmarker`, dry and in rain
    (NFSU2_RAIN_CLOCK=170), SSR on vs off: the diffs show only the reflected buildings and cars, with
    no marker shape on the road or the car.
  - **Open:** not excluded. Flagging additive particles would need their own mask draw. The scenes
    above show no leak.
- Route arrows and barriers are world scenery (native view flags), and the original reflects them as
  well.

## Evidence
`docs/evidence/gps-reflection/`:
- `rear_window_before_after.jpg`: the original; the clone native; the player's settings before / after;
  the same in rain.
- `intro_user_full_before_after.jpg`.

## Tests
- `render::native::world::reflection_tests::gps_aid_is_drawn_in_the_main_view_only`
- `render::native::world::reflection_tests::overlay_draws_are_the_placements_group_draws`
- `render::ssr::tests::overlay_is_left_out_of_traced_reflections` (GPU, ignored: a wet road reflects
  a wall; flagged as an overlay it does not)
- `traced_wall_reflects_on_the_mirror_road` now runs on a wet road. It had failed since the chrome-flicker fix's dry-road
  hold.
