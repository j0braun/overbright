# Texture editor (modding tool)

A live texture browser and editor inside the game, for making Overbright packs (docs/texture-packs.md,
"Overbright packs"). It is a modding tool, not a game feature: off by default and unreachable until the game
is started with `--editor`. Without the flag the hotkey does nothing, no panel exists and the renderers note
nothing; native play is untouched. The original game has nothing like it.

Code: `src/tools/texed.rs` (the editor), `src/tools/overlay.rs` (the panel helpers it shares with the map
editor), `src/obpack/` (the pack format), `src/bin/game/enh/texed.rs` (the game's hooks).

## Use

1. Start the game with `--editor` (the launcher's game arguments, or `game --editor`).
2. Drive or open the screen whose textures you want; press **F7**. The panel opens at the left edge; the
   game's cursor is shown. F7 closes it again.
3. The list fills with the textures drawn while it is open (the world's main view and the cars): name,
   size, format and the Overbright pack that replaces it (`[id]`). Car textures start with `car`. Up / Down
   / Page Up / Page Down / Home / End or a click select; the wheel scrolls. **Find** filters by name or hash.
   **Freeze** stops adding textures; **Live** adds again.
4. Below the list: the selected texture's name, hash, size, format and mip count.
5. **Highlight** (or Enter, or a second click on the row) draws that texture with a magenta / cyan checker
   tint everywhere it is used; **Unlight** takes it away (closing the panel does too). A highlighted row
   starts with `*`.
6. **Export PNG** / **Export DDS** write the game's original texture to
   `<data>/overbright/texture-editor/exports/<NAME>.png|.dds` (`<NAME>` the texture name, or `0x<HASH>` when
   the name is not a safe file name). DXT textures export as DX9 DDS with their own blocks and mips; others
   as BC7 (DX10). The PNG is the top level, decoded.
7. Edit the exported file in any image editor (any size; keep the aspect ratio) and save it under the same
   name, PNG or DDS.
8. **Import** puts the newer of the two into the workspace pack and shows it at once (the hot-reload path
   the packs use: the world's array layer is rewritten, or the arrays laid out again for a new size).
   From then on every save of that file is imported again within about a quarter of a second, until the
   panel is closed or another pack is loaded. **Revert** takes the texture out of the pack: the original
   (or the next pack's) shows again.
9. The pack: **Name**, **Author** and **Licence** fields; **Save pack** writes them into `pack.toml`
   (priority 100, `source = "editor"`) and compiles the pack. The first Import makes the pack
   (`packs/<id>`, the id from the name) if none is loaded, and puts it first in `order.txt`, on.
   **Packs** lists the installed packs (on / off, name, id, or why one is refused); Enter on one makes it the
   workspace pack (its fields read, first in the order, compiled in the background); **Textures** goes back.

While the panel is open it takes the keyboard (the F keys still reach the game, key releases too, so a held
key is not stuck) and the clicks and wheel over the panel; a click elsewhere still reaches the game. Text
fields take typing after a click on them; Enter does nothing more in them, Escape or a click elsewhere
leaves them.

## What it writes

Only into Overbright's data folder (`platform::dirs::app()`, portable-aware): `packs/<id>/…`,
`packs/order.txt`, `texture-editor/exports/…`, and the compiled pack in the asset cache
(`<cache>/packs/<id>/`). Nothing is written into the game's folder or next to its files.

## How it works

* **The list.** While the panel is open (`tools::texed::recording`), the world renderer notes the texture
  slots of the main view's draws after culling (`WorldCore::record_views`, flip-books at their current
  frame; the sky dome not), and each car renderer that draws notes its textures (`CarRenderer::
  record_queued`). Once a frame the game maps the world's slots to their textures (`WorldRenderer::
  slot_hash`, the drive's `World::textures`) and the editor adds new ones, refreshing the shown list every
  half second. The rear view, reflections and shadows are not noted separately (their textures are the main
  view's or lower LODs of them).
* **Highlight.** The texture loader takes a stand-in ahead of every pack (`texture_pack::set_override`,
  through `pack_reload::highlight`): the original's top level, tinted, with its mips, published as a change
  of that texture so the renderers load it again. Taking it away publishes the change again.
* **Import.** The edited file goes to `packs/<id>/<world|car>/<NAME>.png` (or `.dds`), replacing the
  texture's earlier file in that folder; the pack is compiled (`obpack::sync`, PNG to BC7) on a worker
  thread; when it is done the loader's pack list is read again and the texture published as changed
  (`pack_reload::texture_changed`).
* **Hotkey.** F7 (F3 is the performance overlay, F9 to F12 may be the Mark Spot key, F12 the photo key).

## Limits and open points

* No preview image in the panel (the front end's quads draw only UI textures); highlight the texture to see
  it in place.
* UI (HUD, menu) textures are not listed: Overbright packs replace world and car textures only.
* No TexWizard export (it needs a writer for the game's texture-pack chunks).
* Compiling a large PNG for BC7 takes a moment (on a worker thread; the old texture shows until it is done).
* An Advanced-page switch for the tool is not there; `--editor` is the only way to turn it on.

## Tests

`tools::overlay::tests` (buttons, lists, text fields, clicks taken), `tools::texed::tests` (the checker tint,
exports, and the whole flow in a scratch folder: noted textures listed, export, an edited file imported into
a new pack and compiled, a second save re-imported, save pack, highlight on and off, revert, another pack
loaded), `render::…::pack_reload::tests::renderers_follow_changes_on_the_gpu` (the highlight replaces and
restores a world texture on the GPU; slots name their textures).
