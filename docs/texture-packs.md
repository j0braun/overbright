# Optional replacement textures

No replacement world/car artwork is included: the game shows the player's own original textures.

## TexMod packages and the HUD mod (the launcher's Mods page)

For players: in the launcher, **Mods…** (on the main screen) sets up texture mods from files you have:
* **Add package…** opens the folder browser, which lists folders and TexMod packages (`.tpf`). Enter on a
  package imports it; **Import all here** imports every package in the folder shown, or the folder itself
  when it is a folder of loose TexMod files (a `texmod.def` and the images it names). On Windows the
  "Windows dialog…" button picks the folder that holds the packages.
* The import reads your game's textures once (a few seconds), converts each package
  (`tpf_import::import_package`, the same conversion as `examples/import_tpf`, docs/texmod-import.md) and
  says per package how many of its textures replace the game's, how many are not in your game's files (left
  out) and how many are in a format Overbright cannot read; a package made for another game says so (its
  entries name another program than `SPEED2.EXE`), as does one none of whose textures is in your files
  (another version). Free disk space is checked first (twice the package's images).
* The list: name (the package's own comment, else its file name), textures and size, on / off, **Move up** /
  **Move down** (the first in the list wins where two replace the same texture), **Remove…** (asks first;
  deletes only Overbright's converted copy, never your `.tpf`).
* **HUD mod** (the game's HD UI + Maps pack; see "The HUD mod" below): **Add HUD mod…** opens the folder
  browser, which lists folders and archives. It takes the two downloads as players have them: the Upscaled
  HUD part's extracted folder and the map part's `.zip` (or its extracted folder). Choosing one part looks for
  the other beside it (both in one Downloads folder: one step; a folder that holds both: one step); else a
  line asks for the other. The HUD part comes as a 7-Zip archive, which the launcher does not open (it
  says so and asks for the extracted folder; the folder beside the archive is used when it is there). The
  conversion runs on a worker thread with its progress, into Overbright's folder, then the existing check
  runs on the result and **Use the HUD mod** (the game's own HD UI + Maps setting) is focused. **Choose
  converted folder…** still takes a pack converted earlier; **Forget folder** forgets the folder (it is not
  deleted).

Mods belong to their authors and stay on your computer; Overbright neither ships, links nor downloads any.
Reading a `.tpf` is the project's own code from public format notes (THIRD-PARTY-NOTICES.md).

How it is kept (`src/texture_mods.rs`):
* One folder per package: `<data>/overbright/texture-pack/imported/<id>/` with `world/`, `car/` (the pack
  folders below), `import-tpf.tsv` (the importer's report) and `package.txt` (what the launcher shows). An
  import writes into `imported/.import-<id>` and renames it into place only when it finished: a failed or
  interrupted import leaves no package (the leftover is removed the next time the list is read). Nothing is
  written into the game folder or next to the package.
* The list: `texture-mods.txt` beside `game-root.txt` (`# Overbright texture mods v1`, then `on <id>` /
  `off <id>` lines, first wins), written atomically; it moves with Overbright's folder (docs/platform-dirs.md).
* The game (`render::texture_pack::roots`): `NFSU2_WORLD_TEXTURE_PACK` / `NFSU2_CAR_TEXTURE_PACK` first (for
  developers), then the packages that are on, in the list's order, then the World Texture Pack row's folders.
  The list is read when a world or a car loads (at most every half second): a change shows at the next game
  start, race or garage load, and at once with Reload Packs When Changed. No list, or nothing on: the
  loader's path is the native one.
* Rendered check (`scripts/checks/texture_mod_shot.sh GAME OUTDIR`): a package made in a prepared data
  folder (`NFSU2_SHOT_DATA`; a 4 × 4 magenta PNG under the name of every city texture, listed on) changes
  the free-roam shot (82 % of the pixels, 64 % magenta; the car and the HUD untouched); listed off, and with
  no list, the shot is the plain one exactly. No game code is changed for it.
* The game's Graphics page agrees with the launcher: World Texture Pack is the pack Generate Texture Pack
  makes (and an earlier `imported/{world,car}` made by the import tool); its help says the launcher's
  packages apply on or off, ahead of it. HD UI + Maps is the launcher's "Use the HUD mod" (one setting,
  shown in both places); its help says the mod is set up in the launcher's Mods page.

## The HUD mod

Two third-party mods made for the original's "Binary" mod tool, converted together (`src/hud_mod.rs`):
* **The HUD part** (a 7-Zip archive, LZMA2, solid): `assets/userstart.end` (the installer's
  script), folders of DDS images (DXT1/3/5) under `assets/`, `ReadMe.txt`, `Changelog.txt`. The script's
  `bind_textures synchronize <file> TPKBlocks <collection> "<folder>"` lines name the folders; the optional
  `UnlimiterTextures` (for another mod) are left out.
* **The map part** (a ZIP, LZMA method 14; `formats::zip` with `lzma-rs`): `Binary/script.end`,
  `Binary/Map/*.dds` and `Binary/Map/{Default,Beta}/*.dds` (2048 × 2048 DXT5), `readme.txt`. The script's
  `replace_texture` lines name one image per track map (common and "Default"; "Beta" left out), its
  `update_collection GLOBAL\GLOBALB.LZC Tracks <n> <field> <value>` lines are the map calibration.
* The scripts are parsed as data and never run; an instruction the parser does not know refuses the
  download ("This is not the HUD mod: …"). Every image is named by the bin hash of its full name
  (`<HASH>.png`), decoded with the game's own DXT decoder, saved as PNG, read back and compared; the same
  name in two folders must be the same pixels (one PNG). Written: the PNGs, `hd-ui-pack.list`,
  `map-overrides.tsv`, `manifest.json`, `map-manifest.json`, the authors' notes (`ReadMe.txt`,
  `Changelog.txt`, `detailed-map-readme.txt`) and `hud-mod.txt` (made by the launcher).
* Into `<data>/overbright/texture-pack/.hud-mod-import`, renamed to `hud-mod` when finished (an older one
  is moved aside first and removed after; a folder there without `hud-mod.txt` is left alone). Free disk
  space is checked first (half the DDS bytes plus 64 MB). A failed conversion changes nothing.
* The result in plain words: "HUD mod set up: 595 HUD images and 115 maps converted, the maps calibrated
  for 27 tracks. The check finds it complete."; "HUD mod not set up: files are missing or damaged in … :
  <the files> … Download the mod again and extract all of it."; "This is not the HUD mod: …"; another
  version of the mods converts but the check says the game keeps its own HUD.
* Checked against the real downloads (on the maintainer's machine, outside the tree): the same 710 file
  names and decoded pixels as the earlier Python conversion. Tests use stand-ins made in the test
  (`hud_mod::tests`, `formats::zip::tests`, `launcher::tests::hud_mod_from_its_downloads`).

## By hand (developers) To use external PNG replacements without the launcher,
select one or both renderer-specific directories before starting the game:

```sh
NFSU2_WORLD_TEXTURE_PACK=/absolute/path/to/world-pack \
NFSU2_CAR_TEXTURE_PACK=/absolute/path/to/car-pack ./target/release/game
```

Keep your usual executable arguments. These explicit, independent opt-ins work
with **HD UI + Maps off**, and require neither its 710-file inventory nor map
calibration. World arrays, standalone world textures and later-added world
slots use the world directory. Car material textures and their shadow textures
use the car directory, including shadows drawn by the world renderer. UI and
map uploads never search either directory. The same hash can therefore have
different replacements for world and car rendering without leaking through the
process cache.

The Graphics page's **World Texture Pack** row selects the pack that **Generate Texture
Pack** makes in `<data>/overbright/texture-pack/{world,car}` without any variable; a set variable
wins over the row (docs/texture-pack-generator.md, "In the game").

Each variable may name several folders separated like `PATH` (`:`), searched in order, first file
wins; the row's folders follow them (docs/texmod-import.md: an imported TexMod pack in front of
the generated upscale, e.g. `NFSU2_WORLD_TEXTURE_PACK=~/packs/ineedspeed-hd/world:~/upscaled/world`).

Restart after changing a directory or its contents (or turn on **Reload Packs When Changed**, docs/pack-hot-reload.md, to see changes live). Unset the corresponding
variable (or set it to an empty string) to disable that scoped pack. Empty
scoped variables do not suppress an otherwise active legacy shared pack. Missing
pack directories are ignored with a warning. Within an existing scoped pack,
missing, corrupt or unsupported files fall back to original game pixels.
No texture list, material settings or geometry is imported automatically.

The earlier `NFSU2_TEXTURE_PACK` remains the shared legacy pack path. In the
**game**, it is governed by the saved **HD UI + Maps** setting and complete
combined-pack validation, including map calibration. Non-game asset tools may
still explicitly select an arbitrary shared pack with that variable. A scoped
world/car directory takes precedence over the shared pack for its renderer;
without a scoped directory, existing shared-pack behavior is preserved. To
restore completely native assets, disable HD UI + Maps and unset both scoped
variables, then restart.

Each file is named with the original texture's eight-digit **uppercase hexadecimal hash**, for example `ABCDEF12.png`. An exact original texture name plus `.png` is a fallback, for example `ROAD_MAIN.png`; names containing path separators are rejected. Hash files take precedence. A `<HASH>.dds` (DX9 DDS, DXT1, DXT3 or DXT5, sizes in whole 4×4 blocks, with its mip chain) is accepted after `<HASH>.png` and before the name fallback; it is uploaded as-is when the GPU supports BC formats (otherwise decoded with a generated mip chain). This is what `overbright-upscale` writes (docs/texture-pack-generator.md: an upscaled world pack made locally from the player's own files). Files live directly in the pack directory, with no manifest or recursive search. Existing texture inspection tools can identify the source hashes and names.

Supply a complete replacement of the original image, preserving its content layout, atlas regions, orientation and alpha intent. PNG alpha replaces pixel alpha; the original blend mode, alpha-test threshold, draw ordering, scrolling and UV coordinates stay unchanged. The original CPU-side dimensions and metadata also stay unchanged, preserving UI atlas coordinate calculations. Upscale the complete atlas rather than moving its elements. This feature replaces images only: it does not provide geometry, materials or new art.

The loader accepts PNG images up to the smaller of 4096 pixels per axis and the GPU's maximum 2D texture dimension, with a 128 MiB decoder allocation limit. Missing, corrupt, oversized or unsupported images fall back to the original texture. Invalid hash images may fall back to a valid name image. Complete RGBA8 mip chains are generated down to 1×1, including rectangular and non-power-of-two images. Mip filtering weights color by alpha to keep transparent texel colors out of glyph edges, then returns straight alpha for the existing blend modes. The full-resolution pixels stay unchanged. Filtering uses the existing renderer's color interpretation and sampler settings.

All domains share decoding and alpha-safe mip generation, with domain-specific cache keys. With a world or legacy shared pack enabled, prepared world GPU texture arrays are never read from or written to the native track cache; geometry still uses the cache. Since the pack cache they are cached in a file of their own per track (`track-<REGION>-pack.nfc`), valid while the pack's files a slot uses are unchanged (docs/texture-pack-cache.md), and opaque DXT3/DXT5 replacements upload as BC1 (lossless, half the memory). A car-only scoped pack does not disable the native world-array cache. This keeps original caches reusable when the relevant pack is disabled. Replacement lookups are cached within the process with bounded CPU memory; startup and memory use can still increase substantially with large packs, especially GPU memory because PNGs are uploaded as uncompressed RGBA8.

With HD UI + Maps in force the UI also looks in a second folder after the pack: the generated
loading-screen textures, `<data>/overbright/texture-pack/ui` (or `NFSU2_UI_TEXTURE_PACK`), made by
`overbright-upscale --ui` (docs/loading-image.md). The HD pack's own files win.

## Import a TexMod pack (.tpf)

`examples/import_tpf` converts a TexMod package into world/car folders keyed by bin hash (the TexMod import,
docs/texmod-import.md). DDS files may be DXT1, DXT3 or DXT5 (whole 4×4 blocks, full mip chain).

## Import a third-party HD HUD pack

The launcher converts the player's downloaded HD HUD mod into a pack (`src/hud_mod.rs`). The maintainer also
keeps a separate Pillow-based converter as a cross-check (`scripts/import_hud_pack.py`, part of the private set,
not in the public repository); both follow the rules below:

```sh
/usr/bin/python3 scripts/import_hud_pack.py /path/to/extracted-mod /path/to/new-png-pack   # private set only
```

It reads `assets/userstart.end` only to identify standard `bind_textures` source categories; it never executes the installer. Optional `UnlimiterTextures` are excluded. Full DDS texture names are hashed with the native `bin_hash` algorithm and become eight-digit uppercase hexadecimal PNG filenames, so truncated original name fields still match. The manifest retains full logical names. Identical images sharing a hash are deduplicated, and conflicting images or hash collisions with different pixels cause the entire import to fail. Every source's decoded RGBA pixels are compared against the saved PNG. `manifest.json` records DDS, RGBA and PNG SHA256 hashes, source mappings, counts and decoder version. Original `ReadMe.txt` and `Changelog.txt` are preserved.

The converter does not install new texture collections or modify UI layouts. Replacements become visible only when the clone loads and draws the matching original texture. Custom HUD textures intended for collections created by Binary may therefore remain unused in a standard game-data installation.

The HUD converter output alone is not a complete game HD UI + Maps pack;
that setting also requires the existing map import/calibration inventory. This
requirement does not apply to the new world/car directories.

## Validation and available artwork (2026-11-03)

The local downloads inspected for this change contained the supplied Upscaled
HUD archive/extraction, verified original game data and a save archive. No
world/car HD replacement pack was identified or installed. Loader support does
not itself increase texture detail; an actual replacement image is required.
No downloaded installer was executed and original game files were not changed.

Subprocess regression tests initialize HD UI off and check independent world,
car, combined, absent, empty, missing-directory and corrupt-file cases. They
check identical hashes across domains, repeated cached lookups, original
metadata, dimension limits, native fallback and car-only world-cache reuse.
The pre-change world-pack regression failed on native 2×2 output where the
explicit replacement was 8×2. Existing alpha-safe mip and legacy-pack tests
remain in place. Optional GPU upload smoke checks run with
`NFSU2_PACK_TEST_GPU=1 cargo test --release --lib render::texture_pack::tests -- --test-threads=1`.

Final verification: all 455 non-ignored library tests passed with the verified
original game assets (seven GPU tests remained explicitly ignored). All six
texture-pack tests also passed with the optional GPU upload checks enabled,
including independent scoped domains and legacy packs with unset/empty scoped
variables. These upload checks validate format/dimension compatibility; they
are not a visual review of a real HD world/car pack.
