# TexMod (.tpf) texture packs

Many HD texture packs for the game are TexMod packages (`.tpf`), made for the original running under
TexMod or uMod. Overbright converts them into its own texture-pack folders, so you can use them without
TexMod. The import is optional: with no pack on, the game draws exactly as without it.

- **Players:** add the `.tpf` on the launcher's **Mods** page ([texture-packs.md](texture-packs.md),
  "Adding texture mods in the launcher"). The launcher runs the conversion described here.
- **Developers and tools:** `examples/import_tpf` does the same from a terminal (below).

No pack content or converted image is in the repository: the tool reads your own download and your own game
files.

## Converting a package by hand

```sh
NFSU2_ROOT=<game> import_tpf <pack.tpf> <out>
```

Any `.tpf` imports the same way (for example Nexus "NFSU2 Redux HD Textures"). Packs made for uMod are `.tpf`
files too and use the same container. The report lists keys that match nothing: textures that are not in the
game files (for example composited car skins and vinyls, render targets) or that come from another game
version. BMP entries (TexMod's logged dumps) are read; JPG is not.

To use the result without the launcher, point the pack variables at it:

- `NFSU2_WORLD_TEXTURE_PACK` / `NFSU2_CAR_TEXTURE_PACK` take several folders separated like PATH (`a:b`); the
  first file found wins (`render::texture_pack::roots`), and the World Texture Pack row's folders follow.
- The World Texture Pack row (On / Lean) also uses `<data>/overbright/texture-pack/imported/{world,car}`
  before the generated upscale, which fills in the rest. The row is available when either exists. The HD UI
  pack is separate, and both work at once. Reload Packs When Changed watches every folder in the list.

## The .tpf format

Implemented in `crates/ob-formats/src/formats/tpf.rs` from retex's notes.

- A `.tpf` is a ZIP archive with every dword XORed with 0x3FA43FA4. Every entry is encrypted with
  traditional PKWARE ZipCrypto under TexMod's fixed 42-byte password and deflated. Entries are named
  `<EXE>_0x<HASH>.<dds|bmp|png>`; `texmod.def` holds `0xHASH|file` lines. The archive comment is the pack's
  comment.
- Source: retex `docs/texmod-format.md` (github.com/Depth118/retex, MIT OR Apache-2.0; clean-room notes from
  Texmod.exe). The code is this project's own (THIRD-PARTY-NOTICES.md).
- Checked: the unit test reproduces retex's TexMod CRC vector (0x48F4B3D9 for 1 KiB of `i % 256`), and a
  real package decrypts with matching ZIP CRCs for all 111 entries.
- TexMod's key is a CRC-32 (reflected 0xEDB88320, init 0xFFFFFFFF, **no final XOR**) over
  `width × height × bpp` bytes from the start of the locked level-0 surface (bpp: DXT1 0.5, DXT2–5 1, 32-bit
  4, unknown formats 0.5; the pitch is ignored).

## Matching TexMod keys to the game's textures

Code: `src/tpf_import.rs`. Overbright's packs are keyed by the texture's bin hash (the number the game
computes from the texture's name, entry +0x24). `tpf_import::collect` reads every texture pack (chunk
0xB3300000) in every `.BUN/.BIN/.LZC` of the game (10,167 textures, 2–3 s) and computes each one's TexMod
key. `CARS/*/VINYLS.BIN` is skipped: vinyls are composited into the skin before upload.

- DXT1/DXT3/DXT5 and 32-bit textures: their stored level 0 (`SPEED2.EXE` uploads it as stored).
- **P8 textures: the indices expanded through the BGRA palette, 4 bytes per texel** (the surface as
  A8R8G8B8). **Observed, not traced**: all 32 P8 matches of the pack below have this key and none has the
  raw-index key. The conversion itself was not found in `SPEED2.EXE` (D3DX is linked statically; the TPK
  upload path was not traced).

Which folder a texture goes to depends on its file: `TRACKS/` and `FRONTEND/PLATFORMS/` (garage and shop
scenes, drawn by the world renderer) go to `world/`, `CARS/` to `car/`, and GLOBAL's shared textures
(licence plate, carbon fibre, PC_LOAD) to both. A folder's file is only read for a texture its renderer
draws; PC_LOAD is a UI loading screen and used by neither. A key may match several bin hashes when their
pixels are identical (TRN_GRASSA and TRN_GRASSA_KT); all of them get the file. A bin hash whose pixels differ
between files would be flagged (none in this pack).

## The conversion

Code: `tpf_import::convert`, `match_tone`.

- DXT1/DXT3/DXT5 with whole 4 × 4 blocks → `<HASH>.dds` in the same format. Stored levels are kept byte for
  byte; missing lower mip levels are completed (alpha-weighted box chain, Overbright's BC encoder; DXT3 has
  its own encoder, `upscale::encode_bc2`). Levels over 4096, the loader's limit, are dropped (17 textures of
  1024 × 8192 → 512 × 4096). The loader takes DXT3 DDS (`Bc2RgbaUnorm`, decoded without BC support).
- Uncompressed, BMP, PNG, or sizes that are not whole blocks → `<HASH>.png` (RGBA8; the loader makes the mip
  levels), halved while over 4096. No upscaling and no colour-space change (UNORM, like D3D9).
- Images identical to the original (choice: format rule; the original's layout is in docs/formats/) (same size, texels within ±2) are skipped (20 in this pack).
- **Tone match** (the default; `--tone keep` keeps the pack as authored), so the game keeps its look: on the
  original's grid the replacement gets the original's mean colour and luma contrast. Low frequencies are
  scaled about their mean (gain 0.5 to 2), the pack's finer detail is kept on top, and the mean is trimmed
  after clipping. Where alpha is data (opaque draws: the roads' reflection amount, the garage floors'
  reflection) the replacement gets the original's mean alpha (scaled, then offset; an opaque original stays
  opaque; `--alpha keep` keeps it). Cut-out alpha (alpha test or blend: palms, fences, the waterfall) stays as
  authored. Every tone-matched texture is encoded again with Overbright's encoder (lossy, like the
  generator).
- `import-tpf.tsv`, beside the folders, lists per texture: the key, the matched name, hash and files, the
  original and output format and size, notes, luma and alpha before and after, correlation and flags.

## Results with a community HD world pack

- **Coverage:** 110 of 110 package textures matched. 82 world (roads and lane strips, road patches,
  sidewalks, grass, dirt, freeway walls and barriers, palms, hedges, brick and concrete walls, crates), 25
  garage and shop scenes (`FRONTEND/PLATFORMS`), 3 GLOBAL. 95 files written (92 world, 3 car), 20 skipped as
  identical to the original (choice: format rule; the original's layout is in docs/formats/). The pack has no sky texture, so the sky stays the original's. 4 cut-out
  textures have new outlines (two palms, fence spikes, an airport line: the alpha-test coverage differs by
  10–39 %, the author's art).
- **Memory:** the pack's arrays are cached and opaque DXT3/DXT5 art uploads as BC1 (lossless): 290 → 251 MB
  for the world folder ([texture-pack-cache.md](texture-pack-cache.md)).
- **Cost before the pack cache:** 297 MB of GPU memory for the replacements (they replace 16 MB); files
  278 MB. City load 0.3–0.95 s → 0.7–5.6 s (the world arrays were rebuilt, not cached, with any world pack;
  it varies with the page cache on a shared machine). GPU busy at 1280 × 960 in free roam 0.63 → 0.82
  ms/frame. Import: 16 s (6 s decrypt, 3 s scan, 6 s convert).
- **Look**, with the usual play settings: the frame's mean brightness stays within −2 to +2 % at all six
  places checked, ground crops −0 to +5 % (highway +5 %, hills +3.6 %: the pack's wider painted lines and
  finer asphalt under the headlights). The darkest 5 % are unchanged and contrast is the same or higher, so
  the picture is not flatter. Kept as authored, the ground is up to 7.5 % brighter (hills) and the roads'
  reflection amount falls by half (alpha 0.17 → 0.07). The reference is Overbright with the pack off, which
  is the original's picture; the original under TexMod with the pack was not run.

## Open

- The address of the P8 → A8R8G8B8 conversion in `SPEED2.EXE` (the key was observed, not traced).
- A side-by-side of the original under TexMod with the pack, under Wine (not run).
