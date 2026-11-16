# Lamps and shadows evidence (docs/lamps-shadows.md)

## Before / after shots
1920 × 1080 `--shot`, detail 5, WebP q88, per scene three images:
`<scene>_native` (all off), `<scene>_lamps` (Dynamic Lamps), `<scene>_all`
(Dynamic Lamps + Lamp Shadows High + Contact Shadows).

| scene | what | how |
|---|---|---|
| night | free roam under the overpass, night | `NFSU2_RAIN_CLOCK=93 --shot explore` |
| street | lamp-lined circuit start (street lamps on poles) | `NFSU2_SHOT_EVENT=S4_URL_7 --shot race` |
| crowd | street race start from above, cars + crowd under lamps | `NFSU2_SHOT_EVENT=S2_STREET_4 --shot race` |
| neon | drag start in the city (neon / signs: not in the light packs, unchanged) | `NFSU2_SHOT_EVENT=S3_DRAG_5 --shot race` |
| start | Quick Race grid (City Core, signs) | `--shot qrstart` |
| garage | Quick Race car select (front end: no light packs, unchanged) | `--shot qrcars` |
| dusk | brightest dry sky ("daytime"; NFSU2 has no day) | `NFSU2_RAIN_CLOCK=19 --shot explore` |

Scripts: the maintainer's private capture, the maintainer's private capture, the maintainer's private capture
(paths point at the job's work dir).

## Native byte-identity (all three off), md5 of `--shot` 1280 × 960, <base> binary vs this build (ported onto main 2026-11-04)
explore, intro, qrcars, race, qrstart, drift, drag: **identical**.
main, career: differ, as the base binary differs from itself run to run
(the menu turntable; base vs base again 2.5 % / 2.5 % of pixels, base vs
new 2.4 % / 2.5 %). A first version with the lamp hooks inline in the
native shader functions changed 1, 3 and 2 pixels by 1–3 levels in
explore / intro / drift (driver code generation): hence the LAMPS blocks.

## Cost, `--bench explore 600`, night (clock 93), gpu busy ms/frame (mean of 2 runs)
reference desktop GPU (Vulkan, RADV), machine shared with other builds:

| | 1080p | Δ | 4K | Δ | iGPU 1080p | Δ |
|---|---|---|---|---|---|---|
| Off | 0.69 | | 2.44 | | 12.59 | |
| Dynamic Lamps | 0.71 | +0.03 | 2.47 | +0.03 | 16.24 | +3.6 |
| Lamps + Shadows Low | 0.84 | +0.16 | 2.72 | +0.28 | 21.28 | +8.7 |
| Lamps + Shadows High | 0.93 | +0.24 | 2.92 | +0.49 | 25.57 | +13.0 |
| Contact Shadows | 0.71 | +0.02 | 2.44 | +0.01 | 15.29 | +2.7 |
| All (Lamps + High + Contact) | 0.97 | +0.28 | 3.05 | +0.61 | 26.38 | +13.8 |

So Lamp Shadows over Dynamic Lamps: Low +0.13 / +0.25 / +5.0, High
+0.21 / +0.46 / +9.3 (1080p / 4K / iGPU). Their scenery draws (the
headlight layer each frame, new lamp layers) do not scale with the
output: about 0.09 (Low) / 0.13 (High) ms fixed + 0.16 / 0.33 ms per 4K
of pixels (the tiers' costs in `frontend/graphics_table.rs`).
iGPU: the reference integrated GPU, RDNA2 (2 CUs, RADV RAPHAEL_MENDOCINO),
`MESA_VK_DEVICE_SELECT='1002:164e!'`, measured, not estimated. On it the
per-pixel loops dominate (lamps +3.6, contact +2.7 ms): Dynamic Lamps and
Contact Shadows are Medium there, Lamp Shadows High.
