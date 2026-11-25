# Texture packs

Overbright ships no replacement art: the game shows your own original textures. You can add texture mods you
already have, or make your own packs.

- **Players:** add TexMod packages, TexWizard packs and the HD HUD mod on the launcher's **Mods** page (next
  section).
- **Modders:** make an Overbright pack, a folder of PNG or DDS files with a `pack.toml` ("Overbright packs"
  below). The in-game [texture editor](texture-editor.md) writes one for you.
- **Developers:** point the game at folders with environment variables ("By hand", near the end).

Mods belong to their authors and stay on your computer. Overbright neither ships, links nor downloads any.
Reading a `.tpf` or a TexWizard pack is the project's own code, written from public format notes and source
(THIRD-PARTY-NOTICES.md, "Format references").

## Adding texture mods in the launcher

Open the launcher's **Mods** page (the Mods row on the main screen, or the page bar). It lists Overbright packs
first, then the imported packages, in the order the game uses them, each with its own switch. **Use mods**
turns every mod off for the game and gives them back when you turn it on again; the list is kept, and what
was on is kept in `mods-paused.txt`.

What you can add:

| What you have | Made with | Choose |
|---|---|---|
| A TexMod package, `<name>.tpf` | TexMod / uMod | the file |
| A folder of loose TexMod files (`texmod.def` and the images it names) | TexMod's extract | the folder |
| A TexWizard pack: a folder with `meta.json` and `textures.bin` | TexWizard Packer or Binary | the folder |
| A `.zip` holding one or more TexWizard pack folders | (a download) | the file; each pack in it is added |
| A game folder with TexWizard installed (`scripts/TexWizard.json`, `TexturePacks/`) | TexWizard | every pack it loads |
| An Overbright pack folder | the texture editor, or by hand | the folder |
| The HD HUD mod (HUD part and map part) | Binary | its two downloads (below) |
| A map mod | the [map editor](map-editor.md) | the `.zip` or folder |

`.7z` and `.rar` downloads are not opened: extract them first. Images inside packages may be DDS (DXT1, DXT3,
DXT5, uncompressed), BMP or PNG (TexMod), or any texture format of the game's own files (TexWizard).

### Adding a package

- **Add a mod…** opens a folder browser that lists folders, TexMod packages (`.tpf`) and TexWizard packs
  (folders and `.zip` files). Enter on a package imports it. **Import all here** imports every package in the
  folder shown, or the folder itself when it is a folder of loose TexMod files or a TexWizard pack. In a game
  folder with TexWizard, or its `TexturePacks` folder, it takes the packs TexWizard loads, in TexWizard's order
  ("TexWizard packs" below). Dropping files and folders on the launcher does the same. On Windows the
  "Windows dialog…" button picks the folder that holds the packages.
- The import reads your game's textures once (a few seconds) and converts each package. Then it tells you, per
  package, how many of its textures replace the game's, how many are not in your game's files (left out) and
  how many are in a format Overbright cannot read. It also says when a package was made for another game (its
  entries name another program than `SPEED2.EXE`), or when none of its textures is in your files (another
  version). Free disk space is checked first (twice the package's images).

### The list

- Each row shows the name (the package's own comment, else its file name), the number of textures, the size
  and a switch.
- **Move up** / **Move down** appear only when two packs that are on change the same textures. The first in
  the list wins. An Overbright pack always comes before an imported one, so between those the page says which
  is used.
- **Remove** (X, or Space on a pad or keyboard) asks first. It deletes only Overbright's converted copy, never
  your `.tpf`.

A change shows at the next game start, race or garage load, or at once with
[Reload Packs When Changed](pack-hot-reload.md).

### The HUD mod

The HD HUD mod is the game's **HD UI + Maps** pack: sharper HUD, menus and a detailed map.

- **Add HUD mod…** opens the folder browser, which lists folders and archives. Choose the two downloads as you
  have them: the HUD part's extracted folder and the map part's `.zip` (or its extracted folder).
- Choosing one part looks for the other beside it. Both in one Downloads folder, or a folder that holds both,
  takes one step; otherwise a line asks for the other part.
- The HUD part comes as a 7-Zip archive, which the launcher does not open. It says so and asks for the
  extracted folder; the folder beside the archive is used when it is there.
- The conversion runs in the background with its progress, into Overbright's folder. Then the usual check runs
  on the result and **Use the HUD mod** (the game's HD UI + Maps setting) is focused.
- **Choose converted folder…** takes a pack converted earlier. **Forget folder** forgets the folder; it is not
  deleted.

The messages you may see:

- "HUD mod set up: 595 HUD images and 115 maps converted, the maps calibrated for 27 tracks. The check finds it
  complete."
- "HUD mod not set up: files are missing or damaged in … : <the files> … Download the mod again and extract
  all of it."
- "This is not the HUD mod: …"
- Another version of the mods converts, but the check says the game keeps its own HUD.

### The game's Graphics page

The game and the launcher agree:

- **World Texture Pack** is the pack that **Generate Texture Pack** makes (and an earlier
  `imported/{world,car}` made by the import tool). Its help text says the launcher's packages apply whether it
  is on or off, ahead of it.
- **HD UI + Maps** is the launcher's "Use the HUD mod": one setting, shown in both places. Its help text says
  the mod is set up on the launcher's Mods page.

### A replaced game file (for example `LOC4DYNTEX.BIN`)

Some mods for the original replace a file in the game folder, such as `TRACKS/LOC4DYNTEX.BIN` (the city's sky
and a few other shared textures). Overbright reads the game files from your game folder, so such a file works
as it does in the original: keep a copy of the original file, put the mod's file in its place and start as
usual. The launcher's game check then says that a game file is not the original one, which is only a note:
choose **Use this folder**; **Details** lists the file under "Changed". The first track load after the swap
takes a little longer while the cache is made again. To go back, put the original file back.

The Mods page does not take a replaced game file. If the same mod also comes as a TexMod package or a
TexWizard pack, add that one instead: the game folder stays original and the mod can be switched on and off.

## Overbright packs

Overbright's own replacement-texture format: what the texture editor writes and what an author can publish
directly. TexMod packages and TexWizard packs are only imported (the launcher converts them).
Code: `src/obpack/` (`obpack::Pack`, `obpack::sync`, `obpack::list_in`).

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

- `<id>` is the folder's name: lower-case letters, digits and `-`, at most 64 characters.
- Subfolders below `world/`, `car/` and `any/` are searched, at most 8 deep. The key is the file name without
  its extension, not the path. Files other than `.png` / `.dds` (any case) are ignored. Folders and files
  starting with `.` are skipped. At most 65,536 files.

### Naming files: texture names or hashes

A file name (without extension) is either

- `0x` and 1 to 8 hex digits (`0xBEEF`, `0x5FB8BCD1`), or exactly 8 hex digits (`5FB8BCD1`, any case): the
  texture's hash, the number the game computes from its name; or
- anything else: a texture name, hashed exactly as written, the way the game hashes names (`bin_hash`,
  `SPEED2.EXE` 0x43DB50). The game's texture names are upper case; a lower-case file name is another name.

A texture's name and its hash are the same key. Matching is by hash only, so a name the game stores cut short
still matches when the file carries the full name's hash.

### The manifest: `pack.toml`

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

Unknown keys and tables are ignored, so later fields do not break older readers. Text fields lose control
characters.

### Refused packs

A pack is listed with its reason and never used when:

- its id is not valid;
- `pack.toml` is not TOML, is larger than 64 KB, has no `format` or a wrong one, has no `name`, has a priority
  out of range or an unknown `compress`;
- two images in one domain folder stand for one texture (`ROAD.png` and `ROAD.dds`, or a name and its hash);
- folders are nested too deep, or there are too many files.

A single image that does not decode is left out with a warning in the log (the editor shows it); the rest of
the pack is used.

### Order

There is one list, and the first pack that has a texture wins (as for the launcher's packages).
`packs/order.txt`:

```
# Overbright packs v1
on night-roads
off signs-hd
```

Packs it names come first, in its order; `off` ones are not used. Packs it does not name follow, on, by
`priority` (higher first), then by id. The editor puts its pack first.

The loader's whole order is: the developer variables (`NFSU2_WORLD_TEXTURE_PACK`, `NFSU2_CAR_TEXTURE_PACK`),
then the Overbright packs, then the launcher's imported TexMod and TexWizard packages, then the World Texture
Pack row's folders. With no pack folder, or nothing on, the loader reads nothing and works as the original.

### Formats and compiling

| File | Accepted | Compiled to |
|---|---|---|
| PNG (8-bit, up to 4096 × 4096, 128 MB decoded) | yes | BC7 DDS (DX10) with its full mip chain (alpha-weighted 2 × 2 boxes), when the size is a whole number of 4 × 4 blocks; otherwise, or with `compress = "none"`, the PNG as is (RGBA8, mips made at load) |
| DDS DX9 FourCC DXT1 / DXT3 / DXT5 | yes | as is |
| DDS DX10, DXGI BC1 / BC2 / BC3 / BC7, UNORM or SRGB (71–78, 98, 99) | yes | as is (SRGB uploaded as the same bits; the renderer samples it as UNORM, like the game's own textures) |
| other DDS formats, arrays, cube maps, TGA, JPG, KTX2 | no | left out with a reason |

- Each changed image is compiled once. `stamps.tsv` keeps each texture's source, size, time and encoder, and
  an image is built again only when one of them changed. Files of removed images are removed.
- The loader asks for packs at most every half second while a world or a car loads. The first load after you
  install a big PNG pack compiles it; the launcher can compile ahead with `obpack::sync`, which reports
  progress.
- BC7 uses `block_compression` 0.11 (MIT; a CPU port of Intel's ISPC texture compressor kernels) with its
  "fast" settings. A PNG's colours change by a few levels at most in smooth areas. Without BC support on the
  graphics card, the BC7 levels are decoded on the CPU (the same crate).
- A replacement changes pixels only. The original texture's record (blend, alpha test, scrolling, UVs) stays,
  as for every pack.

## TexWizard packs

TexWizard (R-033, GPL-3.0) is a script for the original game that replaces textures by name without changing
game files. Overbright reads its packs from TexWizard's public source (`TexWizard.h`, release 1.3) and wiki.
Code: `crates/ob-formats/src/formats/texwizard.rs` (reading) and `src/texwizard_import.rs` (conversion).

- **A pack** is a folder with `meta.json` and `textures.bin`. `meta.json` is
  `{"textures": [["ORIGINAL", "NEW"], …]}`; TexWizard Packer adds `"links"`, the files the originals come
  from, which the game does not use. Each name is a texture name, hashed with the game's string hash
  (`bin_hash`, `SPEED2.EXE` 0x43DB50), or `0x` and the hash in hex (read up to the first other character;
  only a lower-case `0x`). jsoncpp's default reader parses it: comments and trailing commas are allowed, and
  Overbright's reader takes them too. A later pair for the same original wins.
- **`textures.bin`** is a chunk file holding texture packs (chunk 0xB3300000) in the game's own format
  (`texture::parse_texture_pack`; a whole-file JDLZ is unpacked). Checked against the TexWizard 1.3
  release's example pack for Underground 2 (outside the repository): one texture pack, one 256 × 256 DXT1
  texture whose stored hash is the string hash of its name in `meta.json`.
- **What TexWizard does in the original** (from TexWizard's code; its effect in the original under Wine was
  not run): it replaces the game start's call of the global loader (`LoadGlobalChunks`) with one that also
  loads every pack's `textures.bin`, and every call of `GetTextureInfo` (0x4901D0, verified) with one that
  looks an original's hash up in the pairs and asks for NEW's instead. NEW is any loaded texture: usually the
  pack's own, but it may be another texture of the game.
- **Its hook addresses.** In the `SPEED2.EXE` this project traces (entry point 0x75BCC7, the one TexWizard
  checks) there are 81 calls of 0x4901D0, as TexWizard patches. Its 42 sites below 0x536000 are the same; the
  other 39 and its loader addresses (call 0x57ED99, loader 0x57FB40) are 16 bytes lower than this
  executable's (call 0x57EDA9 of 0x57FB50, which loads `GLOBAL\GLOBALB.LZC`; inferred from the disassembly).
  TexWizard's table was made for another 1.2 executable of 4,800,512 bytes; the traced one has 4,788,224
  bytes, the same code moved by 16 bytes from there on. Per site: docs/evidence/texwizard/hook-sites.tsv.
- **Which packs, in which order:** `scripts/TexWizard.json` in the game folder, `{"packs": [FOLDER, …],
  "autoScan": true}` (folders relative to the game folder), then, unless `autoScan` is false, every folder
  under `TexturePacks/` holding a `meta.json` (recursively; a folder already listed is not loaded twice). A
  later pack's pair replaces an earlier one's, so TexWizard's last pack wins. The launcher imports them last
  first, and Overbright's list (the first wins) keeps TexWizard's priority. TexWizard scans in the file
  system's order; the launcher uses the names' order (case ignored), the order NTFS lists them.
- **The conversion:** each pair puts NEW's pixels under ORIGINAL's hash, in `world/` and/or `car/` by the file
  ORIGINAL comes from (as for TexMod: `TRACKS/` and `FRONTEND/PLATFORMS/` world, `CARS/` car, `GLOBAL` both).
  NEW is taken from the pack's `textures.bin`, else from the game's own files. DXT1/3/5 textures stay DXT
  (`<HASH>.dds`, stored levels kept, missing ones completed); P8 and 32-bit ones become PNG. The same tone
  match, size limit and "same as the original" skip apply as in the [TexMod import](texmod-import.md). The
  result line counts: pairs whose ORIGINAL is not in the game's files ("not found", left out), pairs whose
  NEW is in neither ("name a texture the pack does not hold", left out), and images that cannot be
  converted. Textures in `textures.bin` that no pair names are not used (TexWizard does not use them either;
  they are listed in the report).
- **Differences from TexWizard in the original:**
  - The original draws NEW with NEW's own texture record (its alpha test, blend mode and scrolling).
    Overbright replaces the pixels only and keeps ORIGINAL's record, as for every texture pack here.
  - HUD, menu and loading-screen textures in a pack are converted but not drawn: these packs cover world and
    car textures only, and the UI uses the HUD mod.
  - Vinyl images (`CARS/*/VINYLS.BIN`) are not matched.
  - A NEW held by another TexWizard pack than its own is not found: each pack is converted on its own.
  - **Open:** which texture the game returns when a `textures.bin` holds the same name as an original
    without a pair (not traced; such textures are left out).

## How the HUD mod is converted

Two third-party mods made for the original's "Binary" mod tool are converted together (`src/hud_mod.rs`):

- **The HUD part** (a 7-Zip archive, LZMA2, solid): `assets/userstart.end` (the installer's script), folders
  of DDS images (DXT1/3/5) under `assets/`, `ReadMe.txt`, `Changelog.txt`. The script's
  `bind_textures synchronize <file> TPKBlocks <collection> "<folder>"` lines name the folders. The optional
  `UnlimiterTextures` (for another mod) are left out.
- **The map part** (a ZIP, LZMA method 14; `formats::zip` with `lzma-rs`): `Binary/script.end`,
  `Binary/Map/*.dds` and `Binary/Map/{Default,Beta}/*.dds` (2048 × 2048 DXT5), `readme.txt`. The script's
  `replace_texture` lines name one image per track map (common and "Default"; "Beta" left out); its
  `update_collection GLOBAL\GLOBALB.LZC Tracks <n> <field> <value>` lines are the map calibration.
- The scripts are read as data and never run. An instruction the parser does not know refuses the download
  ("This is not the HUD mod: …").
- Every image is named by the bin hash of its full DDS name (`<HASH>.png`, eight upper-case hex digits), so
  names the original stores cut short still match; the manifest keeps the full names. Images are decoded with
  the game's own DXT decoder, saved as PNG, read back and compared with the source pixels. The same name in
  two folders must be the same pixels (one PNG); different pixels under one hash fail the whole import.
- Written: the PNGs, `hd-ui-pack.list`, `map-overrides.tsv`, `manifest.json` (DDS, RGBA and PNG SHA256
  hashes, source mappings, counts, decoder version), `map-manifest.json`, the authors' notes (`ReadMe.txt`,
  `Changelog.txt`, `detailed-map-readme.txt`) and `hud-mod.txt` (made by the launcher).
- It goes into `<data>/overbright/texture-pack/.hud-mod-import` and is renamed to `hud-mod` when finished. An
  older one is moved aside first and removed afterwards; a folder there without `hud-mod.txt` is left alone.
  Free disk space is checked first (half the DDS bytes plus 64 MB). A failed conversion changes nothing.
- The conversion adds no new texture collections and changes no UI layout. A replacement shows only when
  Overbright loads and draws the matching original texture, so custom HUD textures meant for collections
  that Binary creates may stay unused with standard game data.
- The HUD images alone are not a complete HD UI + Maps pack: that setting also needs the map images and their
  calibration. This does not apply to world and car packs.
- Checked against the real downloads (outside the repository): the same 710 file names and decoded pixels as
  an earlier, separate converter. Tests use stand-ins made in the test (`hud_mod::tests`,
  `formats::zip::tests`, `launcher::tests::hud_mod_from_its_downloads`).

## For developers

### How imported packages are kept

Code: `src/texture_mods.rs`.

- One folder per package: `<data>/overbright/texture-pack/imported/<id>/` with `world/` and `car/` (the pack
  folders), `import-tpf.tsv` or `import-texwizard.tsv` (the importer's report) and `package.txt` (what the
  launcher shows, with `kind=TexMod` or `kind=TexWizard`).
- An import writes into `imported/.import-<id>` and renames it into place only when it has finished, so a
  failed or interrupted import leaves no package (the leftover is removed the next time the list is read).
  Nothing is written into the game folder or next to the package.
- The conversion: `tpf_import::import_package` for TexMod and `texwizard_import::import_pack` for TexWizard,
  both through `tpf_import::import_images`, the same conversion as `examples/import_tpf`
  ([texmod-import.md](texmod-import.md)).
- The list: `texture-mods.txt` beside `game-root.txt` (`# Overbright texture mods v1`, then `on <id>` /
  `off <id>` lines, the first wins), written atomically. It moves with Overbright's folder
  ([platform-dirs.md](../playing/platform-dirs.md)).
- The game (`render::texture_pack::roots`) uses `NFSU2_WORLD_TEXTURE_PACK` / `NFSU2_CAR_TEXTURE_PACK` first,
  then the packages that are on, in the list's order, then the World Texture Pack row's folders. The list is
  read when a world or a car loads (at most every half second). With no list, or nothing on, the loader works
  as the original.
- Rendered check (`scripts/checks/texture_mod_shot.sh GAME OUTDIR`): a package made in a prepared data folder
  (`NFSU2_SHOT_DATA`; a 4 × 4 magenta PNG under the name of every city texture, listed on) changes the
  free-roam shot (82 % of the pixels, 64 % magenta; the car and the HUD untouched). Listed off, and with no
  list, the shot is exactly the plain one. No game code is changed for it.

### Launcher API for Overbright packs

- `obpack::packs_dir()`, `obpack::compiled_root()`; `obpack::list_in(packs)` → every pack folder with on/off
  and the pack or why it is refused, in the loader's order; `load_order` / `save_order` (on/off, order);
  `Pack::manifest` (name, author, licence, priority, version, description), `Pack::index` (textures per
  domain, ignored files), `obpack::winner` (which pack supplies a texture).
- `obpack::install_folder(src, packs)` (checked, staged, renamed into place; copies only the manifest, the
  images and preview/README/LICENSE), `obpack::remove(packs, compiled, id)`,
  `obpack::sync(pack, out, progress)`.

### By hand: the pack variables

To use PNG replacements without the launcher, name one or both folders before starting the game:

```sh
NFSU2_WORLD_TEXTURE_PACK=/absolute/path/to/world-pack \
NFSU2_CAR_TEXTURE_PACK=/absolute/path/to/car-pack ./target/release/game
```

Keep your usual arguments.

- **Scope.** The two variables work with **HD UI + Maps** off and need neither its 710-file inventory nor
  the map calibration. World arrays, standalone world textures and world slots added later use the world
  folder. Car material textures and their shadow textures use the car folder, including shadows the world
  renderer draws. UI and map uploads never search either folder. So the same hash can have different
  replacements for world and car drawing without leaking through the process cache.
- **Several folders.** Each variable may name several folders separated like `PATH` (`:`), searched in
  order; the first file wins. The World Texture Pack row's folders follow them. For example, an imported
  TexMod pack in front of the generated upscale:
  `NFSU2_WORLD_TEXTURE_PACK=~/packs/ineedspeed-hd/world:~/upscaled/world`.
- **The row.** The Graphics page's **World Texture Pack** row selects the pack that **Generate Texture Pack**
  makes in `<data>/overbright/texture-pack/{world,car}`, with no variable. A set variable wins over the row
  ([texture-pack-generator.md](texture-pack-generator.md), "In the game").
- **Changes.** Restart after changing a folder or its contents, or turn on Reload Packs When Changed to see
  changes live. Unset a variable, or set it to an empty string, to switch that pack off. An empty variable
  does not switch off an otherwise active legacy shared pack. Missing pack folders are ignored with a
  warning. Inside an existing pack, missing, corrupt or unsupported files fall back to the original pixels.
  No texture list, material settings or geometry is imported.
- **The legacy shared pack.** `NFSU2_TEXTURE_PACK` remains the shared pack path. In the game it is governed
  by the saved **HD UI + Maps** setting and the full check of the combined pack, map calibration included.
  Asset tools other than the game may still select any shared pack with it. A world or car folder takes
  precedence over the shared pack for its renderer; without one, the shared pack works as before. To go back
  to the original textures completely, switch HD UI + Maps off, unset both variables and restart.

File names in these folders:

- The original texture's eight-digit **upper-case hexadecimal hash**, for example `ABCDEF12.png`.
- A `<HASH>.dds` (DX9 DDS, DXT1, DXT3 or DXT5, sizes in whole 4 × 4 blocks, with its mip chain) is taken after
  `<HASH>.png` and before the name. It is uploaded as it is when the graphics card supports BC formats, else
  decoded with generated mip levels. This is what `overbright-upscale` writes
  ([texture-pack-generator.md](texture-pack-generator.md): an upscaled world pack made on your machine from
  your own files).
- An exact original texture name plus `.png` as a fallback, for example `ROAD_MAIN.png`. Names with path
  separators are rejected. Hash files take precedence.
- Files live directly in the folder: no manifest, no subfolders. Existing texture inspection tools show the
  hashes and names.

What a replacement must be:

- A complete replacement of the original image, with the same layout, atlas regions, orientation and alpha
  intent. Upscale the whole atlas; do not move its parts.
- PNG alpha replaces the pixel alpha. The original blend mode, alpha-test threshold, draw order, scrolling
  and UV coordinates stay. The original's CPU-side size and metadata also stay, so UI atlas coordinates are
  computed as before. This replaces images only: no geometry, materials or new art.
- PNGs up to the smaller of 4096 pixels per side and the graphics card's largest 2D texture, with a 128 MiB
  decoder limit. Missing, corrupt, oversized or unsupported images fall back to the original texture; an
  invalid hash image may fall back to a valid name image.

How they are loaded:

- Full RGBA8 mip chains are made down to 1 × 1, also for rectangular and non-power-of-two images. Mip
  filtering weights colour by alpha, so transparent texels' colours stay out of glyph edges, then returns
  straight alpha for the existing blend modes. The full-size pixels stay unchanged. Filtering uses the
  renderer's existing colour interpretation and sampler settings.
- All domains share decoding and the alpha-safe mips, with cache keys per domain. With a world or legacy
  shared pack on, the prepared world texture arrays are never read from or written to the original track
  cache (geometry still uses it). They are cached in a file of their own per track
  (`track-<REGION>-pack.nfc`), valid while the pack files a slot uses are unchanged
  ([texture-pack-cache.md](texture-pack-cache.md)), and opaque DXT3/DXT5 replacements upload as BC1
  (lossless, half the memory). A car-only pack keeps the original world-array cache, so the original caches
  stay usable when a pack is off.
- Replacement lookups are cached in the process with bounded CPU memory. Start-up time and memory can still
  grow a lot with large packs, especially GPU memory, because PNGs are uploaded as uncompressed RGBA8.
- With HD UI + Maps in force, the UI also looks in a second folder after the pack: the generated
  loading-screen textures, `<data>/overbright/texture-pack/ui` (or `NFSU2_UI_TEXTURE_PACK`), made by
  `overbright-upscale --ui` ([loading-image.md](../original/loading-image.md)). The HD pack's own files win.

### Tests

- `obpack::tests`: manifest round trip and refusals, keys by name and hash, domains and `any/`, bad packs
  refused, order and priority, compiling to BC7 / PNG / DDS with stamps, DX10 round trip and refusals, the
  loader's roots, create / put / install / remove.
- `render::…::texture_pack::tests::overbright_pack_reaches_the_world_layer_as_bc7`: a pack in a data folder
  reaches the world renderer's layer as BC7, decoded without BC.
- The fuzz target `obpack` (`fuzz/targets.rs`: manifest, order, keys, DX10).
- `formats::texwizard::tests` (JSON as jsoncpp reads it, the hash of names, pairs, a pack written and read
  back, TexWizard's pack order), `texwizard_import::tests` (pairs land under the original's hash; NEW from the
  game's files; a missing NEW), `texture_mods::tests::a_texwizard_pack_installs` (folder and ZIP, the
  launcher's install path) and, with your game files,
  `texture_mods::tests::the_launcher_imports_a_texwizard_pack_for_the_players_game` (a pack made in the test
  from two of the game's own textures). No pack content is in the repository.
- The pack variables: tests in child processes start with HD UI off and check separate world and car
  folders, both combined, absent, empty and missing folders and corrupt files; the same hash in both domains,
  repeated cached lookups, the original metadata, size limits, the fallback to the original, and the world
  cache kept with a car-only pack. A test from before the world folder existed fails on the original 2 × 2
  output where the replacement was 8 × 2. The alpha-safe mip and legacy-pack tests stay.
- Optional GPU upload checks: `NFSU2_PACK_TEST_GPU=1 cargo test --release --lib render::texture_pack::tests --
  --test-threads=1`. They check format and size compatibility, including separate domains and legacy packs
  with unset or empty variables; they are not a visual review of a real HD world or car pack.
