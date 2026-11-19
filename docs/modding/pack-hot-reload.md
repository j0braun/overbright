# Reload Packs When Changed

With this setting on, changes to your texture packs show in the running game: no restart and no reload of
the city. It is meant for pack authors. It is optional and off until you turn it on; the original game has
nothing like it, so nothing here was traced or compared under Wine.

## Turning it on

**Options → Display → Enhancements → Advanced → Packs → Reload Packs When Changed** (the row after HD UI +
Maps). With it on:

- A changed, added or removed `<HASH>.png`, `<HASH>.dds` or `<name>.png` in a pack in use shows within about
  half a second.
- **Ctrl+R** reloads every texture of every pack, and checks the HD UI pack again.
- The **HD UI + Maps** row applies at once (textures and map calibration) instead of saying "Restart
  required".
- The **World Texture Pack** row, and a pack the generator finishes, apply to the world and the cars at once
  instead of at the next load.

With it off, packs are read as before: at the start (HD UI + Maps) or when a world or car loads (World
Texture Pack, `NFSU2_WORLD_TEXTURE_PACK` / `NFSU2_CAR_TEXTURE_PACK`), with the same result as a build
without the setting.

## Limits (open)

- Textures made once from a pack image and kept elsewhere keep the old image until their next load: the
  cars' shadow and neon textures, the race start's characters added to the world renderer
  (`add_shadow_texture`, `add_texture_slot`), the car pack's composited rims, Surface Detail normal maps (a
  car's map is dropped for a changed texture; the world's stays), and UI textures added outside the assets
  (the magazine cover's streamed layers).
- A replacement that decodes but has an unexpected aspect ratio is taken as the loader takes it at the start
  (the loader has no size rule beyond its limits).
- Removing a single HD UI file does not switch the pack off: the completeness check runs on index changes,
  folder changes and Ctrl+R only. That texture falls back to the original.

## How it works

The rest of this page is for developers. Code: `src/render/pack_reload.rs`. The setting is
`RowId::ReloadPacks`, label `0xC1_04E0_D4`, options.txt `enh.reload_packs 1`.

- **Watching.** A background thread ("pack watcher", started and stopped once a frame by
  `pack_reload::frame`) polls each folder in use every 250 ms (`TICK`): the HD UI pack's folder
  (`texture-pack` beside the game or `NFSU2_TEXTURE_PACK`, watched while the HD UI row is on, even while
  incomplete) and the world and car pack folders (`texture_pack::root_of`). It polls instead of using
  inotify: no new dependency, and it follows folders that appear, disappear or are symlinks. One poll
  (readdir + stat) on the reference desktop took 0.8 ms for 714 files, 1.9 ms for 1598 files and 1.1 ms for
  947 files (`NFSU2_PACK_WATCH_TIME=<dir> cargo test --release --lib listing_cost -- --nocapture`).
- **Waiting for a file to settle.** A file's size and modification time must stay the same for one whole
  tick before the change counts, so a file still being written waits.
- **Broken files.** A settled file must decode (`texture_pack::validate`, the loader's PNG / DDS limits,
  ≤ 4096 px). One that does not is tried again for 8 ticks (`RETRIES`, about 2 s), then left out with one
  warning; the texture in use stays (its cached replacement is not dropped). It is looked at again when it
  changes.
- **Publishing a change** (`publish`). The loader's cached replacement for that hash or name is dropped and
  an entry goes into a log with its domains (UI, world or car, by folder). Each renderer keeps the generation
  it has seen (`since`):
  - UI (`UiRenderer::sync_pack`, each frame before binding): the textures hit are unbound, so the frame's
    `bind` uploads them again (HUD, menus, fonts and maps all go through it).
  - World (`WorldRenderer::sync_texture_pack`, next to `sync_environment_size` in the game's frame): a
    replacement of the same shape is written over its array layer (`queue.write_texture`). A new size, a
    texture gaining or losing its replacement, or a full reload lays the texture arrays out again
    (`prepared::arrays_with_pack` + `TextureArrays::new`) and rebuilds the sky groups; geometry, draws and
    the track cache are untouched. Without texture arrays the slot's bind group is made again. This waits
    for a later frame while a render thread holds the core.
  - Cars (`CarRenderer::sync_texture_pack`, from `queue` or the draw entry): the textures hit are uploaded
    again (each is its own texture, any size).
- **Whole reloads.** A folder that appears or goes, a domain whose folder changes (the rows, a generated
  pack) or a change to the HD UI pack's index or calibration files (`hd-ui-pack.list`, `map-overrides.tsv`,
  manifests) reloads that domain as a whole. The HD UI pack's completeness is checked again
  (`hd_ui_pack::recheck`) and the map calibration is applied again over the game data's values
  (`map_pack::reapply`, which keeps them from the career load).
- While the texture-pack generator runs, the watcher waits (thousands of files are being written); the
  finished pack then reloads as one change.

### Tests

`render::pack_reload::tests`: file-name keys; the scanner's settling (settling, still changing, add, remove,
change and back); retries then one rejection, a file completed within the retries, a rejected file changing
again; listing filters; end to end in a child process with a world pack folder (publish for the world only,
the loader follows, a broken file keeps the texture in use, removal falls back to the original, name files,
the folder going reloads all, Ctrl+R, a renderer behind the log); the setting off does nothing. On the GPU
(opt-in, `NFSU2_PACK_TEST_GPU=1`): a world renderer rewrites a layer in place and lays out its arrays for a
new size and for the original again, with wgpu validation errors as panics. `map_pack` test: `reapply`
restores and applies again.
