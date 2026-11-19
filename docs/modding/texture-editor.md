# Texture editor

A texture browser inside the game for making your own texture packs. You can see which textures are on
screen, export them, edit them in any image editor and see your version in the game at once. The result is
an Overbright pack ([texture-packs.md](texture-packs.md), "Overbright packs").

It is a modding tool, not a game feature, and the original game has nothing like it. It exists only when
you start the game with `--editor`. Without that flag the hotkey does nothing and the game plays exactly as
without the tool.

## Making a pack, step by step

1. Start the game with `--editor`: add it to the launcher's game arguments, or run `game --editor`.
2. Drive to, or open, the screen whose textures you want, and press **F7**. The panel opens at the left edge
   and the game's cursor appears. F7 closes it again.
3. The list fills with the textures drawn while the panel is open (the world's main view and the cars). Each
   row shows the name, size, format and the Overbright pack that replaces it (`[id]`). Car textures start
   with `car`. Select with Up / Down / Page Up / Page Down / Home / End or a click; the wheel scrolls.
   - **Find** filters by name or hash (the number the game computes from a texture's name).
   - **Freeze** stops adding textures; **Live** adds them again.
4. Below the list you see the selected texture's name, hash, size, format and number of mip levels.
5. **Highlight** (or Enter, or a second click on the row) draws that texture with a magenta and cyan checker
   wherever it is used. **Unlight**, or closing the panel, takes it away. A highlighted row starts with `*`.
6. **Export PNG** or **Export DDS** writes the game's original texture to
   `<data>/overbright/texture-editor/exports/<NAME>.png` or `.dds`. `<NAME>` is the texture name, or
   `0x<HASH>` when the name is not a safe file name. DXT textures export as DX9 DDS with their own blocks
   and mip levels, others as BC7 (DX10). The PNG is the top level, decoded.
7. Edit the exported file in any image editor. Any size works; keep the aspect ratio. Save it under the same
   name, as PNG or DDS.
8. **Import** puts the newer of the two files into your pack and shows it at once. From then on, every save
   of that file is imported again within about a quarter of a second, until you close the panel or load
   another pack. **Revert** takes the texture out of the pack, and the original (or the next pack's version)
   shows again.
9. Fill in **Name**, **Author** and **Licence**, then press **Save pack**. This writes `pack.toml`
   (priority 100, `source = "editor"`) and compiles the pack.

The first Import makes a pack (`packs/<id>`, the id taken from the name) if none is loaded, switches it on
and puts it first in `order.txt`. **Packs** lists the installed packs (on or off, name, id, or why one is
refused). Enter on one makes it the pack you work on: its fields are read, it goes first in the order and is
compiled in the background. **Textures** goes back to the list.

### Keyboard and mouse while the panel is open

The panel takes the keyboard. The F keys still reach the game, and so do key releases, so a held key does
not get stuck. Clicks and the wheel over the panel go to the panel; a click elsewhere still reaches the game.
A text field takes typing after you click it. Enter does nothing more in a text field; Escape or a click
elsewhere leaves it.

## What it writes

Only into Overbright's data folder (`platform::dirs::app()`, which follows portable mode): `packs/<id>/…`,
`packs/order.txt`, `texture-editor/exports/…`, and the compiled pack in the asset cache
(`<cache>/packs/<id>/`). Nothing is written into the game's folder or next to its files.

## Limits

- The panel shows no preview image (the menu's drawing only takes UI textures). Highlight the texture to see
  it in place.
- UI textures (HUD, menus) are not listed: Overbright packs replace world and car textures only.
- No export to TexWizard (that needs a writer for the game's texture-pack chunks).
- Compiling a large PNG to BC7 takes a moment. It runs on a worker thread and the old texture shows until it
  is done.
- There is no switch on the Advanced page; `--editor` is the only way to turn the tool on.

## How it works

The rest of this page is for developers.

Code: `src/tools/texed.rs` (the editor), `src/tools/overlay.rs` (the panel helpers it shares with the map
editor), `src/obpack/` (the pack format), `src/bin/game/enh/texed.rs` (the game's hooks).

- **The list.** While the panel is open (`tools::texed::recording`), the world renderer notes the texture
  slots of the main view's draws after culling (`WorldCore::record_views`; flip-books at their current frame;
  not the sky dome), and each car renderer that draws notes its textures (`CarRenderer::record_queued`).
  Once a frame the game maps the world's slots to their textures (`WorldRenderer::slot_hash`, the drive's
  `World::textures`) and the editor adds new ones, refreshing the shown list every half second. The rear
  view, reflections and shadows are not noted separately: their textures are the main view's, or lower
  detail levels of them.
- **Highlight.** The texture loader takes a stand-in ahead of every pack (`texture_pack::set_override`,
  through `pack_reload::highlight`): the original's top level, tinted, with its mip levels, published as a
  change of that texture so the renderers load it again. Taking it away publishes the change again.
- **Import.** The edited file goes to `packs/<id>/<world|car>/<NAME>.png` (or `.dds`), replacing the
  texture's earlier file in that folder. The pack is compiled (`obpack::sync`, PNG to BC7) on a worker
  thread. When that is done, the loader's pack list is read again and the texture is published as changed
  (`pack_reload::texture_changed`). This is the same live-reload path the packs use: the world's array layer
  is rewritten, or the arrays are laid out again for a new size.
- **Hotkey.** F7, because F3 is the performance overlay, F9 to F12 may be the Mark Spot key and F12 is the
  photo key.

### Tests

`tools::overlay::tests` (buttons, lists, text fields, clicks taken), `tools::texed::tests` (the checker
tint, exports, and the whole flow in a scratch folder: noted textures listed, export, an edited file imported
into a new pack and compiled, a second save imported again, save pack, highlight on and off, revert, another
pack loaded), `render::…::pack_reload::tests::renderers_follow_changes_on_the_gpu` (the highlight replaces
and restores a world texture on the GPU; slots name their textures).
