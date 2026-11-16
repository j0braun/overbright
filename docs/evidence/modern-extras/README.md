# This page evidence: modern extras (docs/modern-extras.md)

Clone frames and numbers only (none of this is in SPEED2.EXE; nothing to
compare under Wine). reference desktop GPU, World Detail 5 (native 8x MSAA); the
machine was shared with other builds and Wine sessions (load
average 15–370 during these runs).

| file | what |
|---|---|
| `latency-final3..5.log` | `--bench latency 600` (1080p racing on the autopilot, display model FIFO 60 / 144 Hz, VRR 144 Hz, Low Latency off / on): input → present / photon, blocked time, frames shown, CPU parts |
| `fsr-bench-<size>.tsv` | the maintainer's private capture: GPU busy ms/frame, `--bench explore 600` at night, Original / TAAU 67-58-50 % / FSR 67-58-50 %, 2 runs |
| `fsr_vs_taau_1080p.webp` | racing frame 200 crops: top Original / TAAU Q / B / P, bottom Original / FSR Q / B / P |
| `fsr_vs_taau_4k.webp` | the same frame at 4K: Original, TAAU Quality, FSR Quality, FSR Performance |
| `replay_cameras.webp` | `--bench replay` with Replay Cameras Extended and Cinematic DOF, replay frame 120: Trackside, Helicopter, Wheel, Orbit |
| `hud_palettes.webp` | racing frame 200, minimap and tachometer: Native, Deuteranopia, Protanopia, Tritanopia |
| `hud_sizes.webp` | racing frame 200: native vs Minimap 130 %, Speedometer 80 %, Race Info 115 % |
| `parity-gates.txt` | `perf.sh parity` against the base binary (full run, then the image gates again after the UI shader fix) |

Smooth slow motion (`NFSU2_REPLAY_SMOOTH_PROBE=144 NFSU2_REPLAY_SLOW=1|0.5|0.25`,
`--bench replay 10`, NFSU2_BENCH_ENH="enh.replay_cameras 1"): numbers in
docs/modern-extras.md §3.
