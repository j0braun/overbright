# Evidence: one wetness, one volume, fountain jets (2026-11-12)

Method as the earlier look measurements: `--shot` 1280 × 960 headless, `NFSU2_SHOT_ENH` per config (a `shoot.sh`
scenes and preset strings: "Modern" = `2high`, "Ultra" = `3ultra`, "Next-Gen max" = `6nextgen`), before =
the tree before the change, after = with it, both `--features dev-harness`. Luma Rec. 709 on sRGB codes, HUD masked
(the look tools' `lstats.py`). `wet.py` = frame Δmean / Δp5 / Δp95 / Δsat-of-bright against the scene's Original and,
for rain / rainharb / street, the road boxes' sheen / coverage / speckle; `jets.py` = the fountain's jet
region. `table-before.txt` / `table-after.txt` = every shot (63) of the run.

Dry and Original identity (md5 of the PNG, before / after):
```
street__2high      7df995ff0cc1 7df995ff0cc1   (Modern, dry: the native road path)
street__3ultra     bff91e586c28 bff91e586c28
street__o+wetroads edd5240ec8a3 edd5240ec8a3   (= street__0orig: Wet Roads on a dry road is native)
street__0orig      edd5240ec8a3 edd5240ec8a3
rain__0orig        9c9dd3b21118 9c9dd3b21118
fountain__0orig    889d564c3437 889d564c3437
```
(`street__6nextgen` differs: it has Volumetric Fog Next-Gen, the volume fix.)

Sheets (Original | before | after): `wet-rain-2high`, `wet-rainharb-2high` (Modern, SSR off),
`crop100-rainharb-2high` (100 %: no speckle), `fog3-neon`, `fog3-rain`, `fountain-water2-ng-before-after`
(Realistic | Next-Gen before | after). Numbers: docs/wet-roads-ssr.md "One wetness", docs/atmosphere.md
"One medium and its energy bound", docs/water.md.
