# Optional replacement textures

No replacement world/car artwork is included: the game shows the player's own original textures.

## Overbright packs (the own format)

Overbright's own replacement-texture format. TexMod packages and TexWizard packs stay import-only (the
launcher converts them, below); an Overbright pack is what the texture editor (docs/texture-editor.md)
writes and what an author can publish directly. Code: `src/obpack/` (`obpack::Pack`, `obpack::sync`,
`obpack::list_in`).

### Layout

```
<data>/overbright/packs/<id>/         installed pack (<data>/packs/<id>/ in a portable install)
  pack.toml                           the manifest (optional: without it the folder is a bare pack)
  world/  ROAD_MAIN.png  0x5FB8BCD1.dds  signs/…   world textures (tracks, shop and garage scenes)
  car/    …                                        car textures
  any/    …                                        both (a key in world/ or car/ wins over any/)
  preview.png, README.txt, LICENSE.txt             optional, copied on install
<cache>/packs/<id>/                   compiled, never distributed (rebuilt when missing)
  world/<HASH>.dds|png  car/<HASH>.dds|png  stamps.tsv
```

* `<id>`: the folder's name, lower-case letters, digits and `-`, at most 64 characters.
* Subfolders below `world/`, `car/` and `any/` are searched (at most 8 deep); the key is the file stem, not
  the path. Files other than `.png` / `.dds` (any case) are ignored. Folders and files starting with `.` are
  skipped. At most 65,536 files.

### Keys: texture names or hashes

A file stem is

* `0x` and 1 to 8 hex digits (`0xBEEF`, `0x5FB8BCD1`), or exactly 8 hex digits (`5FB8BCD1`, any case):
  the texture's hash;
* anything else: a texture name, hashed as the game hashes names (`bin_hash`, SPEED2.EXE 0x43DB50),
  exactly as written. The game's texture names are upper case; a lower-case file name is another name.

A texture's name and its hash are the same key. Matching is by hash only, so a name the game stores
truncated still matches when the file carries the full name's hash.

### Manifest: `pack.toml`

```toml
format = 1                    # required; 1 is the only format; a higher one is refused ("update Overbright")
name = "Night Roads"          # required, at most 80 characters
author = "Someone"            # or authors = ["A", "B"]
licence = "CC-BY-4.0"         # or license = …; an SPDX id is recommended
priority = 0                  # -1000000..1000000: higher first among packs the player has not ordered
version = "1.0.0"             # optional
description = "…"             # optional
compress = "bc7"              # bc7 (default) or none (keep PNGs as RGBA8)
source = "editor"             # informational: what made the pack
```

Unknown keys and tables are ignored (so later fields do not break older readers). Text fields lose control
characters.

### Refused packs

A pack is listed with its reason and never used when: its id is not valid; `pack.toml` is not TOML, larger
than 64 KB, has no or a wrong `format`, no `name`, a priority out of range or an unknown `compress`; two
images in one domain folder stand for one texture (`ROAD.png` and `ROAD.dds`, or a name and its hash);
folders are nested too deep; there are too many files. A single image that does not decode is left out (a
warning in the log; the editor shows it), the rest of the pack is used.

### Order

One list, the first pack that has a texture wins (as for the launcher's packages). `packs/order.txt`:

```
# Overbright packs v1
on night-roads
off signs-hd
```

Packs it names come first, in its order, `off` ones unused; packs it does not name follow, on, by `priority`
(higher first), then by id. The editor puts its pack first. The loader's whole order: the developer
variables (`NFSU2_WORLD_TEXTURE_PACK`, `NFSU2_CAR_TEXTURE_PACK`), then the Overbright packs, then the
launcher's imported TexMod / TexWizard packages, then the World Texture Pack row's folders. No pack folder,
or nothing on: the loader's native path, nothing read.

### Formats and compiling

| file | accepted | compiled to |
|---|---|---|
| PNG (8-bit, up to 4096 × 4096, 128 MB decoded) | yes | BC7 DDS (DX10) with its full mip chain (alpha-weighted 2 × 2 boxes), when the size is a whole number of 4 × 4 blocks; otherwise, or with `compress = "none"`, the PNG as is (RGBA8, mips made at load) |
| DDS DX9 FourCC DXT1 / DXT3 / DXT5 | yes | as is |
| DDS DX10, DXGI BC1 / BC2 / BC3 / BC7, UNORM or SRGB (71–78, 98, 99) | yes | as is (SRGB uploaded as the same bits; the renderer samples it as UNORM, as the game's own textures) |
| other DDS formats, arrays, cube maps, TGA, JPG, KTX2 | no | left out with a reason |

Compiling runs once per changed image: `stamps.tsv` keeps each texture's source, size, time and encoder, and
an image is built again only when one of them changed (removed images' files are removed). The loader asks
for packs at most every half second while a world or a car loads; the first load after installing a big PNG
pack compiles it (the launcher can compile ahead with `obpack::sync`, which reports progress). BC7 uses
`block_compression` 0.11 (MIT; a CPU port of Intel's ISPC texture compressor kernels) with its "fast"
settings; a PNG's colours change by a few levels at most in smooth areas. Without BC support on the GPU the
BC7 levels are decoded on the CPU (the same crate). The replacement changes pixels only: the original
texture's record (blend, alpha test, scrolling, UVs) stays, as for every pack.

### For the launcher (API)

* `obpack::packs_dir()`, `obpack::compiled_root()`; `obpack::list_in(packs)` → every pack folder with on/off
  and the pack or why it is refused, in the loader's order; `load_order` / `save_order` (on/off, order);
  `Pack::manifest` (name, author, licence, priority, version, description), `Pack::index` (textures per
  domain, ignored files), `obpack::winner` (which pack supplies a texture).
* `obpack::install_folder(src, packs)` (checked, staged, renamed into place; copies only the manifest, images
  and preview/README/LICENSE), `obpack::remove(packs, compiled, id)`, `obpack::sync(pack, out, progress)`.

### Tests

`obpack::tests` (manifest round trip and refusals, keys by name and hash, domains and `any/`, bad packs
refused, order and priority, compile to BC7 / PNG / DDS with stamps, DX10 round trip and refusals, the
loader's roots, create / put / install / remove), `render::…::texture_pack::tests::
overbright_pack_reaches_the_world_layer_as_bc7` (a pack in a data folder reaches the world renderer's layer
as BC7, decoded without BC), and the fuzz target `obpack` (fuzz/targets.rs: manifest, order, keys, DX10).

## Texture packages and the HUD mod (the launcher's Mods page)

For players: the launcher's **Mods** page (the Mods row on the main screen, or the page bar) sets up texture
mods from files you have. It lists Overbright packs first, then the imported packages (the loader's order),
each with its own switch; **Use mods** turns every mod off for the game and gives them back when turned on
again (the list is kept; what was on is kept in `mods-paused.txt`). The order shows (Move up / Move down)
only when two packs that are on change the same textures; an Overbright pack always comes before an imported
one, so between those the page says which is used. **Add a mod…** also takes an Overbright pack folder
(`obpack::install_folder`) and a map mod.
The formats it takes:

| What you have | Made with | Taken as |
|---|---|---|
| A TexMod package, `<name>.tpf` | TexMod / uMod | the file |
| A folder of loose TexMod files (`texmod.def` and the images it names) | TexMod's extract | the folder |
| A TexWizard pack: a folder with `meta.json` and `textures.bin` | TexWizard Packer or Binary | the folder |
| A `.zip` holding one or more TexWizard pack folders | (a download) | the file, each pack in it |
| A game folder with TexWizard installed (`scripts/TexWizard.json`, `TexturePacks/`) | TexWizard | every pack it loads |
| The HD HUD mod (HUD part and map part) | Binary | its two downloads (below) |

`.7z` and `.rar` downloads are not opened: extract them first. Images inside packages may be DDS (DXT1,
DXT3, DXT5, uncompressed), BMP or PNG (TexMod), or any texture format of the game's own files (TexWizard).

* **Add a mod…** opens the folder browser, which lists folders, TexMod packages (`.tpf`) and TexWizard
  packs (folders and `.zip` files). Enter on a package imports it; **Import all here** imports every
  package in the folder shown, or the folder itself when it is a folder of loose TexMod files or a
  TexWizard pack; in a game folder with TexWizard, or its `TexturePacks` folder, it takes the packs
  TexWizard loads, in TexWizard's order (see "TexWizard packs"). Dropped files and folders do the same. On
  Windows the "Windows dialog…" button picks the folder that holds the packages.
* The import reads your game's textures once (a few seconds), converts each package
  (`tpf_import::import_package` for TexMod, `texwizard_import::import_pack` for TexWizard; both through
  `tpf_import::import_images`, the same conversion as `examples/import_tpf`, docs/texmod-import.md) and
  says per package how many of its textures replace the game's, how many are not in your game's files (left
  out) and how many are in a format Overbright cannot read; a package made for another game says so (its
  entries name another program than `SPEED2.EXE`), as does one none of whose textures is in your files
  (another version). Free disk space is checked first (twice the package's images).
* The list: name (the package's own comment, else its file name), textures and size, a switch; **Move up** /
  **Move down** when two packs collide (the first in the list wins where two replace the same texture);
  **Remove** (X, Space on a pad / keyboard; asks first; deletes only Overbright's converted copy, never your
  `.tpf`).
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
Reading a `.tpf` or a TexWizard pack is the project's own code from public format notes and source
(THIRD-PARTY-NOTICES.md, "Format references").

How it is kept (`src/texture_mods.rs`):
* One folder per package: `<data>/overbright/texture-pack/imported/<id>/` with `world/`, `car/` (the pack
  folders below), `import-tpf.tsv` or `import-texwizard.tsv` (the importer's report) and `package.txt`
  (what the launcher shows, with `kind=TexMod` or `kind=TexWizard`). An
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

## TexWizard packs

TexWizard (R-033, GPL-3.0) is a script for the original game that replaces textures by name, without
changing game files. Read from its public source (`TexWizard.h`, release 1.3) and wiki; implemented in
`crates/ob-formats/src/formats/texwizard.rs` (reading) and `src/texwizard_import.rs` (conversion):
* **A pack** is a folder with `meta.json` and `textures.bin`. `meta.json` is
  `{"textures": [["ORIGINAL", "NEW"], …]}` (TexWizard Packer adds `"links"`, the files the originals come
  from, which the game does not use). Each name is a texture name, hashed with the game's string hash
  (`bin_hash`, SPEED2.EXE 0x43DB50), or `0x` and the hash in hex (read up to the first other character;
  only a lower-case `0x`). jsoncpp's default reader parses it: comments and trailing commas are allowed,
  and our reader takes them too. A later pair for the same original wins.
* **`textures.bin`** is a chunk file holding texture packs (chunk 0xB3300000), the game's own format
  (`texture::parse_texture_pack`; a whole-file JDLZ is unpacked). Checked against the TexWizard 1.3
  release's example pack for Underground 2 (outside the tree): one texture pack, one 256 × 256 DXT1 texture whose stored hash is the string hash of its name in `meta.json`.
* **What the game does with it** (TexWizard's code; its effect in the original under Wine was not run):
  it replaces the game start's call of the global loader (`LoadGlobalChunks`) with one that also loads
  every pack's `textures.bin`, and every call of `GetTextureInfo` (0x4901D0, verified) with one that looks
  an original's hash up in the pairs and asks for NEW's instead. NEW is any loaded texture: usually the
  pack's own, but it may be another texture of the game. In the SPEED2.EXE this project traces (entry
  point 0x75BCC7, the one TexWizard checks) there are 81 calls of 0x4901D0, as TexWizard patches; its 42
  sites below 0x536000 are the same, the other 39 and its loader addresses (call 0x57ED99, loader
  0x57FB40) are 16 bytes lower than this executable's (call 0x57EDA9 of 0x57FB50, which loads
  `GLOBAL\GLOBALB.LZC`; inferred from the disassembly). TexWizard's table was made for another 1.2
  executable, of 4,800,512 bytes; the traced one has 4,788,224 bytes, the same code moved by 16 bytes from
  there on. Per site:
  docs/evidence/texwizard/hook-sites.tsv.
* **Which packs, in which order**: `scripts/TexWizard.json` in the game folder, `{"packs": [FOLDER, …],
  "autoScan": true}` (folders relative to the game folder), then, unless `autoScan` is false, every folder
  under `TexturePacks/` holding a `meta.json` (recursively; a folder already listed is not loaded twice).
  A later pack's pair replaces an earlier one's, so TexWizard's last pack wins; the launcher imports them
  last first, and Overbright's list (the first wins) keeps TexWizard's priority. The scan's order is the
  file system's in TexWizard; the launcher uses the names' order (case ignored), the order NTFS lists them.
* **The conversion**: each pair puts NEW's pixels under ORIGINAL's bin hash, in `world/` and/or `car/` by
  the file ORIGINAL comes from (as for TexMod: TRACKS/ and FRONTEND/PLATFORMS/ world, CARS/ car, GLOBAL
  both). NEW is taken from the pack's `textures.bin`, else from the game's own files. DXT1/3/5 textures stay
  DXT (`<HASH>.dds`, stored levels kept, missing ones completed), P8 and 32-bit ones become PNG; the same
  tone match, size limit and "same as the original" skip as the TexMod import. Counted in the result line:
  pairs whose ORIGINAL is not in the game's files ("not found", left out), pairs whose NEW is in neither
  ("name a texture the pack does not hold", left out), images that cannot be converted. Textures of
  `textures.bin` that no pair names are not used (TexWizard does not use them either; listed in the report).
* **Differences from TexWizard in the original**: the original draws NEW with NEW's own texture record (its
  alpha test, blend mode and scrolling); Overbright replaces the pixels only and keeps ORIGINAL's record, as
  for every texture pack here. HUD, menu and loading-screen textures in a pack are converted but not drawn
  (the scoped packs are world and car only; the UI uses the HUD mod). Vinyl images (CARS/*/VINYLS.BIN) are
  not matched. A NEW held by another TexWizard pack than its own is not found (each pack is converted on
  its own). Open: which texture the game returns when a `textures.bin` holds the same name as an original
  without a pair (not traced; such textures are left out).
* Tests: `formats::texwizard::tests` (JSON as jsoncpp reads it, the hash of names, pairs, a pack written and
  read back, TexWizard's pack order), `texwizard_import::tests` (pairs land under the original's hash; NEW
  from the game's files; a missing NEW), `texture_mods::tests::a_texwizard_pack_installs`
  (folder and ZIP, the launcher's install path) and, with the player's game files,
  `texture_mods::tests::the_launcher_imports_a_texwizard_pack_for_the_players_game` (a pack made in the
  test from two of the game's own textures). No pack content is in the tree.

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
* Checked against the real downloads (outside the tree): the same 710 file
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
