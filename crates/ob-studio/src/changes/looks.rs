//! A mod's texture list ([`crate::looks`], `looks.toml`) as layering tables of the `texture` kind, beside the
//! map mod's placements and races ([`super::map`]): each texture an enabled mod paints is an item keyed by its
//! texture id, its one field `image` the content hash of the mod's image; the base item is the game's own
//! texture (`image = "game"`). The enabled mods' textures load in their order, the last the strongest
//! (docs/modding/studio-looks.md, "In the player's game"), so the merge answers which mod's paint the game shows.

use ob_asset_format::layering::{Entry, Layer, Op};

use super::{Clash, Row, Tables, compare, this_mod};
use crate::looks::{Looks, TextureId};

/// The kind's short name.
pub const TEXTURE: &str = "texture";
/// The field a texture's rows show: the image the game draws.
pub const IMAGE: &str = "image";
/// The base item's image: the game's own texture.
pub const GAME: &str = "game";

fn image(v: &str) -> toml::Table {
    [(IMAGE.to_string(), toml::Value::String(v.into()))].into_iter().collect()
}

/// One mod's texture layer.
pub fn layer(looks: &Looks, pack: &str) -> Layer {
    let entries = looks
        .entries
        .values()
        .map(|e| Entry { key: e.id.to_string(), op: Some(Op::Merge), disabled: None, fields: image(&e.image) })
        .collect();
    Layer { pack: pack.into(), dependencies: Vec::new(), entries }
}

/// The enabled mods' texture lists as one kind's tables (weakest first, with the game's textures they paint
/// as the base items).
#[derive(Clone, Debug, Default)]
pub struct Textures {
    pub base: Vec<(String, toml::Table)>,
    pub layers: Vec<Layer>,
}

impl Textures {
    /// The textures of `mods` (pack id, its list), weakest first.
    pub fn new(mods: &[(String, Looks)]) -> Textures {
        let mut t = Textures::default();
        for (pack, l) in mods {
            for id in l.entries.keys() {
                let k = id.to_string();
                if !t.base.iter().any(|(x, _)| *x == k) {
                    t.base.push((k, image(GAME)));
                }
            }
            t.layers.push(layer(l, pack));
        }
        t
    }

    fn tables(&self) -> Tables<'_> {
        Tables { kind: TEXTURE, base: &self.base, layers: &self.layers, rules: &super::map::RULES }
    }

    /// Every texture `yours` paints, against the game's and the other mods'.
    pub fn this_mod(&self, yours: &str) -> Vec<Row> {
        this_mod(&self.tables(), yours)
    }

    /// The textures `yours` paints that another mod paints too.
    pub fn clashes(&self, yours: &str) -> Vec<Clash> {
        super::clashes(&self.tables(), yours)
    }
}

/// Two states of one mod's texture list (before → after), texture by texture: `original` is the image before
/// (`game` when the mod did not paint it yet).
pub fn compare_looks(before: &Looks, after: &Looks) -> Vec<Row> {
    let items = |l: &Looks| -> Vec<(String, toml::Table)> {
        l.entries.values().map(|e| (e.id.to_string(), image(&e.image))).collect()
    };
    let mut rows = compare(TEXTURE, &items(before), &items(after));
    for r in &mut rows {
        // A texture is never the mod's addition: before the mod painted it, the game drew its own.
        r.added = false;
        for v in &mut r.values {
            v.original.get_or_insert_with(|| toml::Value::String(GAME.into()));
            if v.yours.is_none() {
                v.game = Some(toml::Value::String(GAME.into()));
            }
        }
    }
    rows
}

/// `looks` with the textures `keys` given back to the game (their entries taken out); None when none of them is
/// in the list.
pub fn reset(looks: &Looks, keys: &[&str]) -> Option<Looks> {
    let mut l = looks.clone();
    for k in keys {
        if let Ok(id) = k.parse::<TextureId>() {
            l.entries.remove(&id);
        }
    }
    (l != *looks).then_some(l)
}
