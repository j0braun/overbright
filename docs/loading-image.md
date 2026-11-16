# Loading screen: the game start's PC_Loading and the HD loading and title pictures (2026-11-05)

Request: an HD loading-screen image ("yes, but only the image": no layout,
animation or fade changes). The first version showed the game start's loading screen always, as the
original; **correction: every loading screen, the start's included, follows
the 200 ms rule** — not shown at all when the load is done sooner. The HD splash work: the title screen's picture
in HD too (§3).

## 1. PC_Loading at game start (the 200 ms rule, a deviation)

**Traced** (`$OB_PRIVATE/dis/speed2.dis`):
* WinMain's tail (0x580E5C..0x580E76) and 0x5817E0 (0x581813) set the main loop's pending load
  step 0x865484 (`g_LoadHandler`, name inferred) to **0x57FF20** (`FrontEndLoader_Load`) and
  0x86548C to 0x7A07A8.
* 0x57FF20 closes `loading_boot.fng` (0x799FC8, 0x5379B0), sets 0x8654A4 = 1 and 0x864F88 = "Load",
  and at **0x57FF95** opens `PC_Loading.fng` (0x790734) with 0x555E90(…, 0) unless 0x52CF70 finds it
  up, then passes `Global\FrontEndMemoryFile.bin` (0x7A0DD4) to 0x57D910 (the front end's load;
  what 0x57D910 does is not traced further).
* The boot screens (0x4D9580: LS_BlankMovie, LS_EAlogo, …, UG_LS_Splash; formats.md "Boot sequence")
  are front-end screens, so they come after this load.

**Observed under Wine** (SPEED2.EXE 1.2, fastgpu.sh/DXVK, fresh profile, x11grab of the Wine
desktop window at 30 fps from its creation): PC_Loading (PC_load.tga, "LOADING", the bars) for
frames 18–21 (~0.13 s on this machine), then LS_BlankMovie's black, then FMVOpening
(internal evidence: loading-image/orig-wine-*.jpg, `timings.txt`). The original's start load is shorter
than the clone's 200 ms rule, which would therefore have hidden it.

**Clone** (`game.rs`: `BootLoading`, `run`, `resumed`, `redraw`), **deviation** (design correction
2026-11-05; CLAUDE.md): the start follows [`LOADING_DELAY_MS`] (200 ms) like every other load
(track loads, `LoadingGate`). `run` arms `BootLoading` for the windowed game (the headless modes
never open a window); `BootLoading::frame` gives a PC_Loading frame only once the start's work
still running with the window open (`resumed`: the window, then the audio device) has taken 200 ms,
its animation from that first frame; `redraw` keeps presenting it until that work is done. A start
done under 200 ms presents **no** PC_Loading frame: the boot screens start at the window's first
frame. The log says which (`game start: done after N ms, PC_Loading shown / not shown`).
* `enh.boot_loading_min_ms` (opt-in, options.txt only, no menu row yet; 0..30000, default **0 = the
  200 ms rule**): with a value, PC_Loading is up from the window's first frame for at least that
  long (this work's behaviour, with a minimum). Presets keep it (QoL).
* The clone's front-end load (`Game::new`: packages, textures, career data) runs before the window
  exists, so the 200 ms are counted over the start's remaining work (from just before the window is
  made to the audio device being open); the check is made as the window opens, before the audio
  device's one blocking open (inferred: a slow device open alone does not raise the screen).
  Moving `Game::new`'s load behind the window (the original's order) is a larger change, not done.
* Measured (private Xvfb :216 in its own PID namespace + bwrap, no sound devices, frame cap
  60, x11grab at 30 fps; internal evidence: loading-image/boot-window-*.jpg): min 0 → `done after 138 ms,
  PC_Loading not shown`: the first window frame is LS_BlankMovie's black, the next the EA logo, no
  PC_Loading frame in the 150 captured; min 1500 → `done after 1501 ms, PC_Loading shown`:
  PC_Loading frames 19–56, then LS_BlankMovie, then the EA logo. (before the correction: always
  shown, down after 83 ms.)
* Shot: `--shot boot_loading` (the same quads `present_loading` draws, 2 s after opening).

## 2. The loading screens' textures and the HD UI pack

`devprobes/src/bin/loading_textures [HD pack]` lists every texture of the loading packages and whether the
pack covers it, and what PC_Loading draws. With the player's pack
(`$OB_PRIVATE/packs/upscaled-hud-detailed-map`, 4× almost everywhere):

| texture | size | where | HD UI pack | generator |
|---|---|---|---|---|
| PC_LOAD C51CE6F1 (PC_load.tga, the picture) | 1024×1024 BGRA8 | PC_Loading, MC_Background, Loading | **no** | **4096×4096 PNG** (mean ΔE 0.05, p99 0.21) |
| U2_LOADING_BG 70EB2CCB | 256×256 DXT1 | Loading_Tips, Loading_Controller (not opened on PC) | no | 1024×1024 PNG (mean ΔE 0.48) |
| GENERIC_LOADING_BAR2 ADB6359F (the bars) | 32×32 | PC_Loading, loading_boot | yes (128×128) | – |
| U2_LOADING_CIRCLE 1BA21214 | 32×32 | PC_Loading, … | yes | – |
| UG_LOAD_LOGO 1C5DABAA | 512×64 | Loading_Controller | yes | – |
| NFSU_LOGO_CHROME C5982DE8 | 256×128 | loading_boot | yes | – |
| U2_MENU_ROUNDEDCORNER, U2_MENU_CORNER_WHITE, U2_SCROLLER_CORNER | 8–16 | various | yes | – |
| CHROMEGENERIC_LOADINGBA 20AFCB2C | 64×64 DXT1 | PC_Loading, loading_boot | no | Flat (nothing to gain) |
| WHITE16X16, U2_MENU_BOX, U2_MENU_BOX_WHITE | 16×16 | various | no | Flat |
| U2_LOADING_GRADIENT CAAB2870 | 32×8 | PC_Loading, Loading | no | Small (under 16 texels) |

"LOADING" is drawn with the font CONDUITMDITC_TT21I/TT14I, which the HD UI pack covers.

**Generator** (`upscale_pack::{collect_loading, generate_ui, ui_dir}`, `overbright-upscale --ui`
or `--ui-out DIR [--hd HDPACK]`; the plain run with no folder makes it too): the textures of
`LOADING_PACKAGES` (and, the HD splash work, `TITLE_PACKAGES`) that the HD UI pack does not have (its file wins: verdict `Covered`), upscaled
with `upscale::Category::UiPicture` (4× like the pack, up to 4096 — the UI loader's limit;
Lanczos-3, unsharp 0.35 with the anti-ring clamp at 1.0, back-projection, ΔE check; wrap-around as
the UI samples) into `<data>/overbright/texture-pack/ui/<HASH>.png` (lossless; the loader makes
the mips) with `overbright-upscale.tsv`. Non-generative, local, from the player's own files; nothing
is shipped. ~5 s. The Graphics page's Generate Texture Pack job does not run it (CLI only).

**Load path** (`render::texture_pack::generated_ui`): the UI domain looks in the HD UI pack first,
then in the generated `ui` folder (`NFSU2_UI_TEXTURE_PACK` names another), and **only while HD UI
Art is in force**; off, nothing changes (native, byte-identical). Pack hot reload does not watch the
`ui` folder (restart after generating). VRAM: PC_LOAD as RGBA8 with mips ≈ 85 MB while a loading
screen's texture is resident.

**Review** (`evidence/loading-image/`): 1080p, 3440×1440 and 4K boot shots before (HD UI off) and
after (HD UI on + pack + generated folder); 4K crops at 100 % and 200 % (face, logo): the logo's
edges and lettering are visibly cleaner, the face smoother with no halos or ringing; the 1/8-scale
colour difference over the 4K frame is 0.19/255 on average. Layout, timing and animation unchanged.

## 3. The title screen's picture in HD

Requirement: "EA logo -> THX logo -> intro movie and then the screen": UG_LS_Splash ("Press
Enter Key", `screens::Splash`, formats.md "Boot sequence"). It is **not** a loading screen and stays
exactly as it was (waits for Enter, the native 30 s return to the intro, no layout, timing, fade or
text change); only its picture gets HD, with HD UI Art on.

`devprobes/src/bin/loading_textures [HD pack]` now lists UG_LS_Splash and what it draws
(`Splash::quads` at 1 s, and at 6 s with the prompt up):

| texture | size | UG_LS_Splash draws | HD UI pack | generator |
|---|---|---|---|---|
| PC_LOAD C51CE6F1 (PC_load.tga, the whole picture) | 1024×1024 BGRA8 | yes | **no** | **4096×4096 PNG** (mean ΔE 0.051, p99 0.21, max 2.84) — the same file as PC_Loading's |
| U2_LOADING_GRADIENT CAAB2870 (the prompt's band) | 32×8 DXT3 | yes | no | Small (under 16 texels; a smooth ramp, nothing to gain) |
| WHITE16X16 E2A5C626 | 16×16 | named, not drawn (hidden objects) | no | Flat |
| CONDUITMDITC_TT14I / TT21I (fonts: "Press Enter Key", the copyright line) | – | yes | yes | – |

So the title screen is the same art as PC_Loading (and MC_Background, Loading): no new texture was
needed; the generator now also collects the title screen's package (`upscale_pack::TITLE_PACKAGES`,
read by `collect_loading`, so `overbright-upscale --ui` / `--ui-out` covers it by itself) and the
loading-image file is used wherever PC_LOAD is drawn. Generated with
`overbright-upscale --ui-out $OB_PRIVATE/packs/generated-ui --hd $OB_PRIVATE/packs/upscaled-hud-detailed-map`
(14 textures: 7 covered by the HD UI pack, 4 flat, 1 small, 2 upscaled: PC_LOAD and U2_LOADING_BG,
12.8 MB, 12 s).

**Review** (internal evidence: loading-image/splash-*; `--shot splash`, before = HD UI Art off, after = on
with the HD UI pack + the generated folder): 1080p frames; 4K crops at 100 % (logo, face)
and 200 % (eyes, the windshield's lettering, the prompt). The picture's edges (logo chrome and
lettering, hair, the car's lines) are smoother and cleaner with no halos or ringing; the source's
softness stays (nothing is invented); the prompt and copyright text are sharper from the HD UI
pack's fonts. The art is reused unchanged on PC_Loading (`--shot boot_loading` on/off checked at
1080p and 4K, as the loading image). Look rule (luma, Rec. 709, 0..255; Wine = `wine/compare/fedraw/orig/splash.png`,
SPEED2.EXE 1.2, 1280×960): Wine mean 78.09 / p5 10.86 / p95 205.64; clone off 78.22 / 10.86 / 206.68;
clone on 78.25 / 10.87 / 206.31 (MAE vs Wine: off 0.44, on 0.69 of 255). At 4K: off 58.70 / 0 / 191.26,
on 58.69 / 0 / 192.41 (pillarboxed). No brightening or flattening.

## Tests
`upscale_pack::tests::loading_ui_pack_fills_only_the_gaps`, `…::loading_textures_include_the_picture`,
`…::title_screen_textures_are_collected` (skip without NFSU2_ROOT),
`render::texture_pack::tests::generated_ui_folder_needs_hd_ui_art`,
`qol::tests::enhancements_round_trip` (the key, clamp, presets), game
`tests::boot_loading_waits_for_the_start_and_the_minimum`, `tests::boot_loading_follows_the_200_ms_rule`
(a start under 200 ms presents no PC_Loading frame; past it, up from its first frame; the
opt-in minimum shows it at once).
