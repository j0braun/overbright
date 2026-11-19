# Screen resolutions and aspect ratios

Overbright supports every common screen size and aspect ratio, from a 640 × 480 window to 5120 × 1440 and
portrait monitors. Choose them under **Options → Display**: Screen Mode, Resolution and Aspect Ratio.

| Aspect | Sizes checked |
|---|---|
| 4:3 | 640×480 (the smallest window), 800×600, 1024×768, 1280×960, 1600×1200 |
| 5:4 | 1280×1024 |
| 16:10 | 1280×800, 1440×900, 1680×1050, 1920×1200, 2560×1600 |
| 16:9 | 1280×720, 1366×768, 1600×900, 1920×1080, 2560×1440, 3840×2160 |
| 3:2 | 2160×1440, 2256×1504 |
| 21:9 | 2560×1080, 3440×1440, 3840×1600 |
| 32:9 | 3840×1080, 5120×1440 |
| 9:16 (portrait) | 1080×1920 |

The Resolution row offers only sizes your monitor can show, and follows the monitor when you move the window
to another one. Aspect Ratio **Stretched (Original)** is the original's look; **Wide** fits menus, HUD and
the 3D view to wide and tall screens ([Ultrawide](ultrawide.md)).

## What "supported" means

At every size and aspect ratio in common use, with every display option the settings offer, these are
correct (not only "a window opens"):

* the menus (the 640 × 480 FEng canvas), the in-game HUD, movies, loading screens, dialogs;
* the 3D world view (its field of view follows the option's rule), photo mode;
* mouse hit-testing (a click on a drawn widget acts on that widget).

The native rule still holds: the original's look stays available and byte-identical
(Aspect Ratio **Stretched (Original)**: the 640 × 480 canvas stretched over the target, 0x5C5030 /
0x5BF760, and the native Vert− projection, 0x5C7ED0; docs/playing/native-deviations.md, docs/playing/ultrawide.md,
docs/formats.md "Display modes and aspect"). Wide / tall layouts are the clone's default on top.

## How it works

How it works, what it costs, how it was checked and what is open: [Screen resolutions and aspect ratios: how it works](../notes/graphics/resolutions.md).
