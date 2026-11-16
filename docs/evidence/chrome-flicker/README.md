# Chrome-flicker evidence (docs/chrome-flicker.md)
- (internal evidence) eight frames: a community-save Lancer (max settings, 144 Hz), 8 consecutive frames of the carbon trunk while driving off; left before (carbon drawn with the yellow gloss paint: mirror patches that change shape every frame), right after (CARBONFIBER).
- (internal evidence) flicker maps: frame 320 and the per-pixel mean |second temporal difference| ×8 (native, base max, fix max, fix max without Sharpen).
- matrix_moving_144hz.txt / matrix_static_144hz.txt: flicker.py over the trunk (300x120+490+600), every option dropped from the max set in turn, base vs fix (columns: frames, mean, std_mean, std_p99, delta_mean, delta>8, acc_mean, acc>16).
- dry_road_reflection.txt, dry_road_*_native_ssrbefore_ssrafter.jpg: the reflection contribution per Graphics row on dry roads.
- neon_ssr_streaks_before_after.jpg: neon_car shot; rows SSR Full alone / max; columns without SSR, before, after.
