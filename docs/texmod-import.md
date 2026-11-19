# TexMod (.tpf) texture packs (clone deviation, opt-in)

The NFSU2 HD packs are TexMod
packages (`.tpf`) made for the original running under TexMod/uMod. `examples/import_tpf` converts
one into the clone's texture-pack folders (docs/texture-packs.md). **No pack content or converted
image is in the repo**; the tool reads the player's own download and game files. Pack off (no
variable, World Texture Pack Off): the loader path is unchanged and the game is byte-identical.

## The format (from retex's notes, implemented in `crates/ob-formats/src/formats/tpf.rs`)
* A `.tpf` is a ZIP archive with every dword XORed with 0x3FA43FA4 and every entry encrypted with
  traditional PKWARE ZipCrypto under TexMod's fixed 42-byte password, deflated. Entries are named
  `<EXE>_0x<HASH>.<dds|bmp|png>`; `texmod.def` holds `0xHASH|file` lines. The archive comment is
  the pack's comment. Source: retex `docs/texmod-format.md` (github.com/Depth118/retex, MIT OR
  Apache-2.0; clean-room notes from Texmod.exe). Our code is our own (THIRD-PARTY-NOTICES.md).
  Checked: the unit test reproduces retex's TexMod CRC vector (0x48F4B3D9 for 1 KiB of `i % 256`);
  the real package decrypts with matching ZIP CRCs for all 111 entries.
* TexMod's key: CRC-32 (reflected 0xEDB88320, init 0xFFFFFFFF, **no final XOR**) over
  `width × height × bpp` bytes from the start of the locked level-0 surface (bpp: DXT1 0.5,
  DXT2-5 1, 32-bit 4, unknown formats 0.5; the pitch is ignored).

## The mapping to our textures (`src/tpf_import.rs`)
Our packs are keyed by the texture's bin hash (entry +0x24). `tpf_import::collect` reads every
texture pack (chunk 0xB3300000) in every `.BUN/.BIN/.LZC` of the game (10,167 textures, 2-3 s;
CARS/*/VINYLS.BIN skipped: vinyls are composited into the skin before upload) and computes the
TexMod key of each:
* DXT1/DXT3/DXT5 and 32-bit textures: their stored level 0 (SPEED2.EXE uploads it as stored).
* **P8 textures: the indices expanded through the BGRA palette, 4 bytes per texel** (the
  surface as A8R8G8B8). **Observed, not traced**: all 32 P8 matches of the pack below have this
  key and none has the raw-index key. The conversion itself was not found in SPEED2.EXE (D3DX is
  linked statically; the TPK upload path was not traced) — open.
Domains by file: TRACKS/ and FRONTEND/PLATFORMS/ (garage and shop scenes, drawn by the world
renderer) go to `world/`, CARS/ to `car/`, GLOBAL's shared textures (license plate, carbon fibre,
PC_LOAD) to both (a folder's file is only read for a texture its renderer draws; PC_LOAD is a
UI loading screen, not used by either). A key may match several bin hashes (identical pixels:
TRN_GRASSA and TRN_GRASSA_KT) — all get the file. A bin hash whose pixels differ between files
would be flagged (none in this pack).

## The conversion (`tpf_import::convert`, `match_tone`)
* DXT1/DXT3/DXT5 with whole 4×4 blocks → `<HASH>.dds` in the same format: stored levels kept byte
  for byte; missing lower mips completed (alpha-weighted box chain, our BC encoder; DXT3 got an
  encoder, `upscale::encode_bc2`); levels over 4096 (the loader's limit) dropped (17 textures of
  1024×8192 → 512×4096). The loader now takes DXT3 DDS (`Bc2RgbaUnorm`, decoded without BC).
* Uncompressed, BMP, PNG or sizes that are not whole blocks → `<HASH>.png` (RGBA8, the loader
  makes the mips), halved while over 4096. No upscaling, no colour-space change (UNORM like D3D9).
* Images identical to the original (same size, texels within ±2) are skipped (20 in this pack).
* **Tone match (default; `--tone keep` = as authored)**, for the look rule: on the original's
  grid the replacement gets the original's mean colour and luma contrast (low frequencies scaled
  about their mean, gain 0.5..2, the pack's finer detail kept on top, mean trimmed after
  clipping); where alpha is data (opaque draws: the roads' reflection amount, the garage floors'
  reflection) the original's mean alpha (scaled, then offset; an opaque original stays opaque;
  `--alpha keep` keeps it). Cut-out alpha (alpha test/blend: palms, fences, waterfall) stays as
  authored. Every tone-matched texture is re-encoded with our encoder (lossy, as the generator).
* `import-tpf.tsv` beside the folders: per texture the key, the matched name/hash/files, original
  and output format/size, notes, luma and alpha before/after, correlation and flags.

## Selecting it (docs/texture-packs.md)
* Players add packages in the launcher's Mods page (docs/texture-packs.md, "TexMod packages and the HUD
  mod"): the same conversion (`tpf_import::import_package`), one folder per package under `imported/<id>/`
  and a kept list the game reads; the paragraphs below are the tool's and the developers' way.
* `NFSU2_WORLD_TEXTURE_PACK` / `NFSU2_CAR_TEXTURE_PACK` now take several folders like PATH
  (`a:b`), first file wins (`render::texture_pack::roots`), followed by the row's folders.
* World Texture Pack row (one row, unchanged tiers): On/Lean now also use
  `<data>/overbright/texture-pack/imported/{world,car}` **before** the generated upscale (which
  fills in the rest); the row is available when either exists. The HD UI pack (UI domain) is independent:
  both work at once. The hot-reload watcher watches every folder in the list.

## A community HD world texture pack (TexMod), 2026-11-05
* Coverage: **110 of 110** package textures matched (0 unmatched): 82 world (roads/lane strips,
  road patches, sidewalks, grass, dirt, freeway walls/barriers, palms, hedges, brick/concrete
  walls, crates), 25 garage/shop scenes (FRONTEND/PLATFORMS), 3 GLOBAL. 95 files written
  (92 world, 3 car), 20 skipped as identical to the original. No sky texture is in the pack: the
  skybox stays the original's. 4 cut-out textures have new silhouettes (palms ×2, fence spikes,
  airport line: alpha-test coverage differs by 10-39 %, the author's art).
* Docs/texture-pack-cache.md: the pack-applied arrays are now cached and opaque DXT3/DXT5 art uploads as BC1 (lossless): 290 → 251 MB for the world folder.
* Cost (before the pack cache): 297 MB of GPU memory for the replacements (they replace 16 MB); files 278 MB. City load
  0.3-0.95 s → 0.7-5.6 s (the world arrays are rebuilt, not cached, with any world pack:
  existing behaviour; varies with the page cache, machine shared). GPU busy at 1280×960 free roam
  0.63 → 0.82 ms/frame. Import: 16 s (6 s decrypt, 3 s scan, 6 s convert).
* Look (internal evidence, the play setup's options): frame mean within
  −2..+2 % at all six places, ground crops −0..+5 % (highway +5 %, hills +3.6 %: the pack's wider
  painted lines and finer asphalt under the headlights), p5 unchanged, contrast (sd) equal or
  higher — not flatter. As authored the ground is up to +7.5 % brighter (hills) and the roads'
  reflection amount falls by half (alpha 0.17 → 0.07). Not compared with the original under Wine
  with TexMod (TexMod was not run, per the plan); the reference is the clone with the pack off,
  which is the native render.

## Other packs
Any `.tpf` (e.g. Nexus "NFSU2 Redux HD Textures") imports the same way:
`NFSU2_ROOT=<game> import_tpf <pack.tpf> <out>`; the report lists unmatched keys (textures not in
the files, e.g. composited car skins/vinyls or render targets, or another game version). BMP
entries (TexMod's logged dumps) are read; JPG is not. Packs made for uMod (`.tpf` too) use the
same container.

## Open
* The P8 → A8R8G8B8 conversion's address in SPEED2.EXE (key observed, not traced).
* A Wine side-by-side of the original under TexMod with the pack (not run).
