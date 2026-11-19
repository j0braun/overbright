# Tracing round: HUD layout per resolution

How `SPEED2.EXE` places the in-game HUD, as the base for HUD positions as data with anchors (the native
values must come out identical at every resolution). Tags as in [docs/formats.md](../../formats.md).

## Where the positions come from (verified)

* The HUD is FEng packages: HUD_SingleRace, HUD_Drag, HUD_Drift, HUD_Short_Track (picked by race type,
  0x5F1983; `frontend::hud::package_for`), the split-screen 2PHudTop / 2PHudBot (and their _DRAG, _DRIFT,
  _OUTRUN variants), Chyron_IG. Every position, size, pivot and rotation is package data in 640 × 480
  canvas units with (0, 0) at the centre (docs/formats/front-end.md "Layout": transform 0x51B0B0, canvas
  offset (320, 240) at 0x536F70, z buckets 0x51B850).
* Code adds no fixed offsets to package positions. What code changes at run time:
  * shows / hides elements by feature mask (0x4CA090, 0x4C6690) and runs package scripts that move
    groups (LAPINFOON / LAPINFOOFF on LeaderBoardGroup and Leader_bottom, LEADERON / LEADEROFF on
    BestTimeCounter);
  * minimap items: at the map object's position plus the item's offset in map-window units times the
    map's size, pulled in to 0.55 (0x4CA840; `hud::minimap`); the player arrow at the map's centre;
  * engage prompt slots and their scripts (0x4C8AC0, 0x4ABAF0; career-1 "Messages / SMS");
  * world-anchored icons: a world point projected and mapped to the canvas as x / width · 640,
    y / height · 480 (0x4EC34C);
  * the rear-view mirror is one FE quad in the canvas (docs/formats/world.md "Rear-view mirror").

## Per resolution (verified)

* FE_DrawQuad 0x5C5030 maps every canvas vertex to clip space with constants only: x · (1/320) − 1,
  −(y · (1/240) − 1) (0x79AC18, 0x79AC1C, 0x784250; 0x5C5083..0x5C51E6); the line drawer 0x50DCD0 does the
  same. The window and back-buffer size are not read.
* So the original has a single HUD layout: the 640 × 480 canvas stretched over the whole target at every
  resolution and aspect ratio; on 16:9 every element is 4/3 wider than authored and sits proportionally
  further out. There is no safe area, no anchoring and no per-resolution package, font or texture (the
  strings 1024x768 / 1280x960 / 1280x1024 are display-mode names).
* The only size-dependent steps map *into* the canvas: the cursor (0x5BF760: (x − left) · 640 / client
  width, (y − top) · 480 / client height) and projected world points (0x4EC34C).
* A data model that reproduces native exactly: canvas positions as stored, anchor "stretch" (the canvas
  scaled by (w / 640, h / 480)). The clone's Stretched aspect is that; its Wide default (16:9 safe area,
  Centered / Full Width) is a documented deviation (docs/playing/ultrawide.md, docs/notes/graphics/resolutions.md).

## Inferred / open

* **Open**: the split-screen HUD (2PHud*) — how the two packages map to the two halves (a per-viewport
  canvas or package positions authored per half) was not traced.
* **Open**: the HUD font fallback that picks a random font when a font is missing (a native bug handled
  as its own fix).
* **Inferred**: Custom HUD (HUD_CustomTextures_nn, colours) changes textures and colours only, not
  positions (no position writes found in the custom-HUD code paths cited in front-end.md).
