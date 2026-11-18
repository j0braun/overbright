# Texture pack cache and compression (clone deviation, opt-in path only)

Loads with any world pack were
slow (measured city loads of 0.3–0.95 s rising to 0.7–5.6 s) because the world texture arrays were rebuilt from the
pack's files on every load, never cached. This work changes only the pack path's speed and memory.
With no world pack (row Off, no variable) nothing here runs and the native track cache is used
as before, byte for byte. No SPEED2.EXE behaviour is involved (the texture packs are a clone feature).

## The pack-applied arrays' cache
* `WorldRenderer::new` (src/render/mod.rs): with a world pack, the upload-ready arrays with the
  pack applied (`prepared::arrays_with_pack`) are written after the first load to a file of their own
  beside the track's: `<cache>/track-<REGION>-pack.nfc` (`TrackCache::pack_file` / `store_pack`,
  src/cache/). It holds the same "gpu.meta" / "gpu.a*" sections as the native data (no geometry;
  the track's file has it) and a "pack.key" section. The track's own file keeps the native arrays,
  so turning the pack off finds them unchanged. There is one file per track, for the most recent pack setup.
* Validity: the file is a normal cache file over the track's game files (build id, sizes, times,
  content hash; every 4 MB chunk hashed when opened, read in parallel), plus **"pack.key"** =
  `texture_pack::fingerprint`: the loader version, the size limit, the World Texture Pack cap
  (Lean / low VRAM), every pack folder in order and, per folder, every file a slot of this track
  would try (`<HASH>.png`, `<HASH>.dds`, `<NAME>.png`) that exists, plus the folder's
  `overbright-upscale.tsv`, each with its size and modification time (ns). Each folder is listed
  once (`read_dir`), with no stat for each candidate: about 1 ms for L4RA with the imported pack. Adding, removing,
  renaming or rewriting a file any slot uses, or changing folder order or tier, gives a new key,
  and the arrays are rebuilt and the file is replaced. The TexMod import's tone mode (`import_tpf
  --tone`) is part of the files' bytes and times; there is no runtime tone setting to key on.
* Diagnostic subsets (`NFS_ONLY_TEXTURE`) neither read nor write it. With `NFSU2_NO_CACHE`, nothing is cached.
* Disk: L4RA with the imported pack: 234 MB. The folder's existing size limit applies
  (`NFSU2_CACHE_MAX_MB`, default 2048; oldest files are pruned first), so with large packs on several
  regions, native track files may be pruned and rebuilt sooner.

## Compression of the replacements (VRAM)
* **Opaque DXT3/DXT5 → BC1, lossless** (`upscale::opaque_to_bc1`, applied in
  `texture_pack::decode_dds` for every domain). When every texel of every level decodes to alpha 255, each
  colour block is already a 4-colour BC1 block. It is kept as it is (c0 > c1), or its end points are swapped
  and the index low bits flipped (c0 < c1: the same palette listed the other way), or its indices are set to 0
  (c0 = c1: all four colours equal, so BC1's transparent index is never used). Half the bytes, same
  texels. Checks: unit test `opaque_dxt_to_bc1_is_lossless` (CPU decode equal, all orders);
  the maintainer's probe `pack_vram` decodes every level of every converted file and compares them (all 35 equal); on the GPU
  the `--shot explore` and `--shot race` frames with the pack are byte-identical between the old
  binary and the new one, for both the build path and the cache path (md5 in this work's report). SSIM 1.0.
* The community world folder (internal evidence: the pack VRAM table): 35 of 89 DDS are
  opaque (29 DXT3, 6 DXT5); the pack's upload size goes from 290.4 to 250.9 MB (−13.6 %). L4RA's world arrays
  go from 255.1 to 233.9 MB, natives included.
* **Not done: PNG → BC.** The pack's two large PNGs (ARC_PRKG_CEILING 1024², ARC_SIDEWALK_GRASS_WH
  512×4096, opaque, 16.8 MB as RGBA8) encoded to BC1 with our encoder would save 14.7 MB, but measure
  luma SSIM 0.9915 / **0.9723** (PSNR 41.6 / 31.3 dB). The grass loses visible detail, so it fails the
  look rule. `NFSU2_PACK_BC1_PNG=1 pack_vram <folder> <out>` writes source/result crops. BC7 would
  keep the quality at a quarter of the size (12.6 MB saved, about 5 %), but needs a third-party
  encoder (none vendored). Left open.

## Measurements (the reference desktop; shared, so the load numbers are noisy)
internal evidence: loads SET 10 runs, 4 arms; with the pack, city loads (load + first frame, median) 502 → 364 ms first load, 322 → 160 menu → free roam, 396 → 216 race → free roam, 466 → 258 Quick Race circuit (pack off unchanged: 277 / 278 ms); cold page cache 1.1–2.7 s → 0.6–1.0 s.

## Tools
* `pack_vram <folder> [out]`: per file, the format and bytes before and after these steps,
  whether each step is lossless, and SSIM/PSNR for lossy ones.
* `NFSU2_TIMING=1`: spans "texture arrays: pack key / build / upload", "prepared::arrays: layers /
  levels"; `RUST_LOG=info` logs "world texture arrays: N arrays, X MB with the world pack (from the cache)".

## Open
* BC7 for opaque RGBA8 PNG replacements (third-party encoder).
* Upload (staging copy) is now the largest part of a cached pack load (50–280 ms, varies with load).
