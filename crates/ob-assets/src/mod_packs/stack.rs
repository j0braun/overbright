//! The texture layers the game reads, as one stack of mod packs (layering.md §7). Every mod kind the game reads
//! textures from is one layer of it, strongest first: the packs switched on in `order.txt` and the plain map mods
//! (`tools::map::texture_roots`), then the Overbright texture packs, the texture packages the launcher keeps, the
//! imported and generated World Texture Pack folders, and the HUD mod. The kinds keep their own lists and their own
//! order among themselves; nothing is moved or rewritten on disk. With none of them in use the stack is empty.

use std::path::{Path, PathBuf};

/// What a layer of the stack is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    /// A mod pack of `<app>/mods` (a `.obpk` or a folder).
    Pack,
    /// A plain map mod of `maps/` with textures.
    MapMod,
    /// An Overbright texture pack of format 1 (`packs/<id>`).
    TexturePack,
    /// An imported TexMod / TexWizard package.
    TexMod,
    /// The World Texture Pack row's imported folder.
    Imported,
    /// The World Texture Pack row's generated upscale.
    Upscale,
    /// The HUD mod (HD UI art).
    HudMod,
}

impl LayerKind {
    /// The words the launcher uses for the kind.
    pub fn label(self) -> &'static str {
        match self {
            LayerKind::Pack => "mod pack",
            LayerKind::MapMod => "map mod",
            LayerKind::TexturePack => "texture pack",
            LayerKind::TexMod => "texture package",
            LayerKind::Imported => "imported texture pack",
            LayerKind::Upscale => "generated upscale",
            LayerKind::HudMod => "HUD mod",
        }
    }
}

/// One mounted layer: where its texture images for one renderer are read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub id: String,
    pub kind: LayerKind,
    pub dir: PathBuf,
}

/// The name of the folder `n` levels above `p` (the pack or mod folder a texture folder sits in).
fn up(p: &Path, n: usize) -> String {
    p.ancestors().nth(n).and_then(Path::file_name).and_then(|s| s.to_str()).unwrap_or_default().to_string()
}

fn classify(dir: &Path) -> Layer {
    let own = super::ROOTS.read().unwrap_or_else(|e| e.into_inner());
    if let Some((id, _)) = own.iter().find(|(_, root)| dir.starts_with(root)) {
        return Layer { id: id.clone(), kind: LayerKind::Pack, dir: dir.to_path_buf() };
    }
    drop(own);
    // maps/<folder>/textures/<world|car>
    Layer { id: up(dir, 2), kind: LayerKind::MapMod, dir: dir.to_path_buf() }
}

/// Every texture layer for the world (`car` false) or the cars, the strongest first (the first folder that has a
/// texture wins). The one place the game's texture loader asks.
pub fn texture_layers(car: bool) -> Vec<Layer> {
    let mut v: Vec<Layer> = crate::tools::map::texture_roots(car).iter().map(|d| classify(d)).collect();
    let d = if car { crate::obpack::Domain::Car } else { crate::obpack::Domain::World };
    // <compiled>/<id>/<world|car>
    v.extend(
        crate::obpack::enabled_roots(d)
            .into_iter()
            .map(|dir| Layer { id: up(&dir, 1), kind: LayerKind::TexturePack, dir }),
    );
    // <packages>/<id>/<world|car>
    v.extend(
        crate::texture_mods::enabled_roots(car)
            .into_iter()
            .map(|dir| Layer { id: up(&dir, 1), kind: LayerKind::TexMod, dir }),
    );
    let imported = crate::upscale_pack::imported_dir(car);
    for dir in crate::upscale_pack::selected_all(car) {
        let kind = if dir == imported { LayerKind::Imported } else { LayerKind::Upscale };
        let id = if kind == LayerKind::Imported { "imported-textures" } else { "upscale-art" };
        v.push(Layer { id: id.into(), kind, dir });
    }
    let mut seen = std::collections::HashSet::new();
    v.retain(|l| seen.insert(l.dir.clone()));
    v
}

/// The HUD mod's layer, which the loader reads when no other layer is in use.
pub fn hud_layer() -> Option<Layer> {
    crate::hd_ui_pack::active_root().map(|d| Layer { id: "hud-mod".into(), kind: LayerKind::HudMod, dir: d.to_path_buf() })
}

/// The folders of [`texture_layers`], in the same order.
pub fn texture_stack_dirs(car: bool) -> Vec<PathBuf> {
    texture_layers(car).into_iter().map(|l| l.dir).collect()
}
