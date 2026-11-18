# Reload Packs When Changed

Clone deviation, opt-in: the HD UI pack reloads while the game runs.
Not a feature of SPEED2.EXE: nothing here was traced or compared under Wine; there is no native
counterpart. Default **Off** = native: packs are read at startup (HD UI + Maps) or when a world /
car loads (World Texture Pack, `NFSU2_WORLD_TEXTURE_PACK` / `NFSU2_CAR_TEXTURE_PACK`), byte-identical
to before (gate: see the unit report).

## The setting
Enhancements page, row **Reload Packs When Changed** (`RowId::ReloadPacks`, label `0xC1_04E0_D4`,
options.txt `enh.reload_packs 1`), after the HD UI row. With it on:

* the packs in use are watched: a changed, added or removed `<HASH>.png`, `<HASH>.dds` or
  `<name>.png` shows within about half a second, without a restart;
* **Ctrl+R** reloads every pack's every texture (and checks the HD UI pack again);
* the **HD UI + Maps** row applies at once (textures and map calibration), instead of "Restart
  required";
* the **World Texture Pack** row (and a pack the generator finishes) applies to the world and the
  cars at once, instead of at the next load.

## How (src/render/pack_reload.rs)
* A background thread ("pack watcher", started and stopped by `pack_reload::frame` once a frame
  from the game) polls each folder in use every 250 ms (`TICK`): the HD UI pack's folder
  (`texture-pack` beside the game or `NFSU2_TEXTURE_PACK`, watched while the HD UI row is on, even
  while incomplete) and the world and car pack folders (`texture_pack::root_of`). Polling, not
  inotify: no new dependency, and it follows folders that appear, disappear or are symlinks.
  Measured cost of one poll (readdir + stat, the reference desktop): 714 files 0.8 ms, 1598 files 1.9 ms,
  947 files 1.1 ms (`NFSU2_PACK_WATCH_TIME=<dir> cargo test --release --lib listing_cost -- --nocapture`).
* Debounce: a file's (size, mtime) must stay the same for one whole tick before it counts; a file
  still being written keeps changing and waits.
* Half-written / broken files: a settled file must decode (`texture_pack::validate`, the loader's
  PNG / DDS limits, ≤ 4096 px). One that does not is retried for 8 ticks (`RETRIES`, ~2 s), then left
  out with **one** warning; the texture in use stays (its cached replacement is not dropped). It is
  looked at again when it changes.
* A settled change is published (`publish`): the loader's cached replacement for that hash / name
  is dropped and an entry goes into a log with its domains (UI / world / car, by the folder). Each
  renderer keeps the generation it has seen (`since`):
  * UI (`UiRenderer::sync_pack`, each frame before binding): the asset textures hit are unbound, so
    the frame's `bind` uploads them again (HUD, menus, fonts, maps: all go through it).
  * World (`WorldRenderer::sync_texture_pack`, next to `sync_environment_size` in the game's frame):
    a replacement of the same shape is written over its array layer (`queue.write_texture`); a new
    size, a texture gaining / losing its replacement or a whole reload lays the texture arrays out
    again (`prepared::arrays_with_pack` + `TextureArrays::new`) and rebuilds the sky groups;
    geometry, draws and the native track cache are untouched. Without texture arrays the slot's
    bind group is remade. Deferred to a later frame while a render thread holds the core.
  * Cars (`CarRenderer::sync_texture_pack`, from `queue` / the draw entry): the textures hit are
    uploaded again (each is its own texture, any size).
* A folder that appears or goes, a domain whose folder changes (rows, generated pack) or the HD UI
  pack's index / calibration files (`hd-ui-pack.list`, `map-overrides.tsv`, manifests) changing:
  the domain reloads as a whole; the HD UI pack's completeness is checked again
  (`hd_ui_pack::recheck`) and the map calibration re-applied over the game data's values
  (`map_pack::reapply`, which keeps them from the career load).
* While the texture-pack generator runs, the watcher waits (thousands of files being written); the
  finished pack then reloads as one change.

## Limits (open)
* Textures made once from a pack image and kept elsewhere keep the old image until their next
  load: the cars' shadow / neon textures and the race start's characters added to the world
  renderer (`add_shadow_texture`, `add_texture_slot`), the car pack's composited rims, Surface
  Detail normal maps (a car's map is dropped for a changed texture; the world's stays), and UI
  textures added outside the assets (the magazine cover's streamed layers).
* A replacement that decodes but has an unexpected aspect ratio is taken as the loader takes it at
  startup (the loader has no size rule beyond its limits).
* Removing a single HD UI file does not disable the pack (the completeness check runs on index
  changes, folder changes and Ctrl+R only); that texture falls back to the original.

## Tests
`render::pack_reload::tests`: file-name keys; scanner debounce (settling, still-changing, add,
remove, change-and-back); retries then one rejection, a file completed within the retries, a
rejected file changing again; listing filters; end to end in a child process with a world pack
folder (publish for the world only, the loader follows, a broken file keeps the texture in use,
removal falls back to the original, name files, the folder going reloads all, Ctrl+R, a renderer
behind the log); the setting off does nothing. GPU (opt-in `NFSU2_PACK_TEST_GPU=1`): a world
renderer rewrites a layer in place, lays out its arrays for a new size and for the original again,
with wgpu validation errors as panics. `map_pack` test: `reapply` restores and re-applies.
