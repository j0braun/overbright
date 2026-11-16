# Wet-driving evidence (docs/wet-road-car.md)

Clone frames only (no native counterpart for the spray; the reference is the clone's Original path at the same
`--bench drive` frame, which the look direction matched to the original's luminance). Fast release build, reference desktop GPU,
1920 × 1080 unless noted. "before" = the base (with the look direction), "after" = this work; both with the player's play
setup (perf.sh USER) + Wet Road Interaction High, Rain Detail High, Wet Car, Spray, Lens Drops. Heavy rain:
clock 162 (rain 0.70); "damp": clock 196 (the rain stops by frame ~300, road wetness 0.93 at frame 510,
0.69 at 810).

| file | what |
|---|---|
| heavy_chase_0230/0510/0810/1110_*_orig_before_after.webp | Original / before / after (half size): 33 mph launch, 56 mph between rivals, 65 mph drafting three cars with oncoming traffic, the post-crash camera on the splashes |
| heavy_bumper_0510/0810_*.webp | the bumper camera, same frames |
| damp_chase_0510/0810_*.webp | the rain stopped / drying: thin mist instead of the shower's rooster tails |
| motion_heavy_chase_780_840_before_after.webp, motion_heavy_bumper_780_840_before_after.webp | animated, every 3rd frame (look rule E7, motion: no popping, no shimmer, no lattice) |
| consec_before/after_801_804_crop100.png | four consecutive frames at 100 % |
| crop100/200_4k_0810_before/after.png | 3840 × 2160 (SSAA 2x) at 100 % / 200 %: the white wall and the march's lattice before, gone after |
| look-drive.txt | `tools/look/lstats.py` per frame: Original, rows off, before, after |
| look-spots.txt | The look direction spots (`tools/look/cshot.sh`), the row and the presets vs Original, before and after |
| cost.txt | `perf.sh bench` A/B at 4K (max, max without the row, max with the row On), 3 reps × 600 frames |
| gate.txt, perfgate.txt | `scripts/checks/gate.sh … <base> extreme` (CHECKS=full): PASS; perfgate vs the frame-budget gate baseline (99e726f): FAIL on rain-max draw_cpu_p10 +51 % at load 230 (the interleaved A/B against <base> in cost.txt: draw CPU p10 2.37 → 2.27, median 3.30 → 2.81 ms) |
