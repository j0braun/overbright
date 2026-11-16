# Wet Roads + Screen-Space Reflections: before / after (2026-11-04)

Clone shots only (a deviation: nothing to capture in the original).
1920 × 1080 `--shot`, detail 5, WebP q88. Per scene four images:
`_native` (both rows Off), `_wet` (`enh.wet_roads 1`), `_ssr` (`enh.ssr 2`),
`_wetssr` (both).

| scene | shot | env |
|---|---|---|
| rain | explore, night shower | `NFSU2_RAIN_CLOCK=170` |
| dusk (dry) | explore, sunrise/dusk sky, no rain | `NFSU2_RAIN_CLOCK=19` |
| night (dry) | explore, night, no rain | `NFSU2_RAIN_CLOCK=93` |
| racestart | qrstart | – |

NFSU2 has no daytime; "dusk" is the brightest dry sky (clock 19). The
wetness is 1 after a track load (native), so dry scenes show small puddles.
`zoom_rain_car.webp` (native over wet+SSR): the player's car, exhaust and
lamp light reflected in the puddles (the native capture holds no car).
`zoom_racestart_cars.webp`: the rival's wheel and body reflected.
`debug_rain_mask_confidence.webp`: `NFSU2_SSR_DEBUG=1` (R mask weight,
G ray confidence, B mask surface still in front).

## Native byte-identity (both Off) vs <base>, 1280 × 960, md5
explore, intro, qrcars, race, qrstart, drift, drag, ending: identical.
main, career: differ, as <base> differs from itself run to run
(ref vs ref 440 / 144 pixels, ref vs new 703 / 345, ref2 vs new 263 / 490).

## Cost, `--bench explore 600`, rain clock 170, gpu busy ms/frame (mean)
reference desktop GPU (Vulkan, RADV), machine shared with other builds (load ~30):

| size | Off | Wet Roads | SSR Half | SSR Full | Wet + SSR Half |
|---|---|---|---|---|---|
| 1920 × 1080 | 1.03 | 1.07 | 1.55 | 1.57 | 1.53 |
| 3840 × 2160 | 2.36 | 2.33 | 3.56 | 4.34 | 3.74 |

iGPU class: the reference integrated GPU, RDNA2 (2 CUs, RADV RAPHAEL_MENDOCINO),
forced with `MESA_VK_DEVICE_SELECT='1002:164e!'`, 1920 × 1080:
Off 12.63, Wet Roads 14.33 (+1.7), SSR Half 27.12 (+14.5), SSR Full 33.05
(+20.4), Wet + SSR Half 28.05. SSR is an Ultra-tier option (not for
iGPUs); Wet Roads is cheap everywhere.
