//! What a mod changes, value by value, and where each value in the player's game comes from (Studio's Changes
//! view, docs/modding/studio.md "Changes"): the original game's, yours, or another enabled mod's that wins over
//! yours. Built on the keyed merge with per-field attribution ([`layering::merge`], [`Item::from`]), so the rows
//! are the layering tables' own answer; [`map`] reads a map mod's documents into those tables.
//!
//! * [`this_mod`]: every value a mod gives, against the original and against the other mods in the game.
//! * [`compare`]: two states of one mod (a history step, a kept version), value by value.
//! * [`reset`]: a value given back to the lower layers (the original, or what another mod gives).

pub mod map;

use std::collections::BTreeSet;

use ob_asset_format::layering::{self, Item, KindRules, Layer, Merged};

/// Where a value in the player's game comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum From {
    /// The original game's (no mod gives it, or a mod gives the original value back).
    Original,
    /// This mod's.
    Yours,
    /// Another mod's, above this one in the player's game.
    Other(String),
}

/// One value of an item.
#[derive(Clone, Debug, PartialEq)]
pub struct Value {
    /// Its field path (`position`, `grid[1].heading_deg`, `disabled`).
    pub field: String,
    /// Where the value the game uses comes from.
    pub from: From,
    /// The value the game uses (None: the item has no such value).
    pub game: Option<toml::Value>,
    /// The value this mod gives (None: this mod does not give it).
    pub yours: Option<toml::Value>,
    /// The original game's value (None: unknown, or the item is an addition).
    pub original: Option<toml::Value>,
}

impl Value {
    /// This mod gives the value and another mod's is used instead.
    pub fn loses(&self) -> bool {
        self.yours.is_some() && matches!(self.from, From::Other(_))
    }
}

/// One item and its values, as the Changes view groups them.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The kind's short name (`placement`, `race`).
    pub kind: &'static str,
    pub key: String,
    /// The mod added the item (it has no original).
    pub added: bool,
    pub values: Vec<Value>,
}

impl Row {
    /// Where the item as a whole comes from: another mod when any of yours loses, else yours when any value is
    /// yours, else the original.
    pub fn from(&self) -> From {
        if let Some(v) = self.values.iter().find(|v| v.loses()) {
            return v.from.clone();
        }
        if self.values.iter().any(|v| v.from == From::Yours) { From::Yours } else { From::Original }
    }

    pub fn loses(&self) -> bool {
        self.values.iter().any(Value::loses)
    }
}

/// Which values a list shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    /// Every value this mod gives.
    Mine,
    /// Only the values of this mod that another mod's beats.
    Losing,
    /// Every value of the items this mod changes, the original ones too.
    All,
}

/// The rows `show` keeps (items without a kept value are dropped).
pub fn filter(rows: &[Row], show: Show) -> Vec<Row> {
    rows.iter()
        .filter_map(|r| {
            let values: Vec<Value> = r
                .values
                .iter()
                .filter(|v| match show {
                    Show::Mine => v.yours.is_some(),
                    Show::Losing => v.loses(),
                    Show::All => true,
                })
                .cloned()
                .collect();
            (!values.is_empty()).then(|| Row { values, ..r.clone() })
        })
        .collect()
}

/// One kind's tables: the original items, the enabled mods' layers bottom to top (weakest first; the one that
/// is `yours` included), and the kind's rules.
pub struct Tables<'a> {
    pub kind: &'static str,
    pub base: &'a [(String, toml::Table)],
    pub layers: &'a [Layer],
    pub rules: &'a KindRules,
}

/// The value at a field path of an item: `a.b`, `list[key].c` (a list keyed by index through the item's
/// `list_keys`, else an element whose key field has the key), `disabled`.
pub fn get(item: &Item, path: &str, rules: &KindRules) -> Option<toml::Value> {
    if path == "disabled" {
        return Some(toml::Value::Boolean(item.disabled));
    }
    let mut cur = toml::Value::Table(item.fields.clone());
    let mut list_field = String::new();
    for part in path.split('.') {
        let (name, key) = match part.split_once('[') {
            Some((n, k)) => (n, Some(k.trim_end_matches(']'))),
            None => (part, None),
        };
        cur = cur.as_table()?.get(name)?.clone();
        list_field = if list_field.is_empty() { name.to_string() } else { format!("{list_field}.{name}") };
        if let Some(k) = key {
            let list = cur.as_array()?;
            let i = match item.list_keys.get(&list_field) {
                Some(keys) => keys.iter().position(|x| x == k)?,
                None => {
                    let by = rules.keyed_lists.iter().find_map(|(f, by)| match by {
                        layering::ListKey::Field(kf) if *f == list_field => Some(*kf),
                        _ => None,
                    })?;
                    list.iter().position(|e| e.get(by).and_then(|v| v.as_str()) == Some(k))?
                }
            };
            cur = list.get(i)?.clone();
        }
    }
    Some(cur)
}

/// The top-level field paths of an item (and `disabled`), for "every value".
fn all_fields(item: &Item) -> Vec<String> {
    let mut v: Vec<String> = item.fields.keys().cloned().collect();
    v.push("disabled".into());
    v
}

fn from_of(item: Option<&Item>, path: &str, yours: &str) -> From {
    match item.and_then(|i| i.from_of(path)) {
        None => From::Original,
        Some(p) if p == yours => From::Yours,
        Some(p) => From::Other(p.to_string()),
    }
}

/// Every item `yours` changes in one kind, value by value, against the original and the player's game (all of
/// `t.layers` merged). An item the mod changes has a row; with [`Show::All`] its other values are there too.
pub fn this_mod(t: &Tables, yours: &str) -> Vec<Row> {
    let Some(yi) = t.layers.iter().position(|l| l.pack == yours) else { return Vec::new() };
    let game = layering::merge(t.base.to_vec(), t.layers, t.rules);
    // Yours as it is with only the layers up to it: the values it gives.
    let mine = layering::merge(t.base.to_vec(), &t.layers[..=yi], t.rules);
    let original = layering::merge(t.base.to_vec(), &[], t.rules);
    let mut rows = Vec::new();
    for item in &mine.items {
        let given: Vec<&String> = item.from.iter().filter(|(_, p)| p.as_str() == yours).map(|(f, _)| f).collect();
        if given.is_empty() {
            continue;
        }
        let in_game = game.get(&item.key);
        let orig = original.get(&item.key);
        let mut paths: Vec<String> = given.iter().map(|s| s.to_string()).collect();
        for f in all_fields(item) {
            if !paths.iter().any(|p| p == &f || p.starts_with(&format!("{f}.")) || p.starts_with(&format!("{f}["))) {
                paths.push(f);
            }
        }
        let values = paths
            .into_iter()
            .map(|path| {
                let is_yours = item.from_of(&path) == Some(yours);
                let game = in_game.and_then(|i| get(i, &path, t.rules));
                let original = orig.and_then(|i| get(i, &path, t.rules));
                let mut from = from_of(in_game, &path, yours);
                // Yours, but the original value: the game shows the original.
                if from == From::Yours
                    && let (Some(g), Some(o)) = (&game, &original)
                    && layering::same_value(g, o)
                {
                    from = From::Original;
                }
                Value {
                    from,
                    game,
                    yours: if is_yours { get(item, &path, t.rules) } else { None },
                    original,
                    field: path,
                }
            })
            .collect();
        rows.push(Row { kind: t.kind, key: item.key.clone(), added: orig.is_none(), values });
    }
    rows
}

/// A clash for the Clashes table: one value, what each layer gives (bottom to top, the original first), and
/// the winner.
#[derive(Clone, Debug, PartialEq)]
pub struct Clash {
    pub kind: &'static str,
    pub key: String,
    pub field: String,
    /// (pack, its value); the original first as ("", value).
    pub stack: Vec<(String, Option<toml::Value>)>,
    pub winner: From,
}

/// The values `yours` gives that another mod also gives, with every layer's value.
pub fn clashes(t: &Tables, yours: &str) -> Vec<Clash> {
    let game = layering::merge(t.base.to_vec(), t.layers, t.rules);
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for c in game.clashes.iter().filter(|c| c.weaker == yours || c.stronger == yours) {
        let field = c.field.clone().unwrap_or_else(|| "disabled".into());
        if !seen.insert((c.key.clone(), field.clone())) {
            continue;
        }
        let original = layering::merge(t.base.to_vec(), &[], t.rules);
        let mut stack = vec![(String::new(), original.get(&c.key).and_then(|i| get(i, &field, t.rules)))];
        for (i, l) in t.layers.iter().enumerate() {
            let upto: Merged = layering::merge(t.base.to_vec(), &t.layers[..=i], t.rules);
            let it = upto.get(&c.key);
            let gives = it.is_some_and(|it| it.from_of(&field) == Some(l.pack.as_str()));
            if gives {
                stack.push((l.pack.clone(), it.and_then(|it| get(it, &field, t.rules))));
            } else if l.entries.iter().any(|e| e.key == c.key) || l.pack == yours {
                stack.push((l.pack.clone(), None));
            }
        }
        out.push(Clash {
            kind: t.kind,
            key: c.key.clone(),
            winner: from_of(game.get(&c.key), &field, yours),
            stack,
            field,
        });
    }
    out
}

/// Two states of one mod's items (a history step's before and after, a kept version and now), value by value:
/// `original` is the earlier state's value, `yours` and `game` the later's. Items in neither are left out.
pub fn compare(kind: &'static str, before: &[(String, toml::Table)], after: &[(String, toml::Table)]) -> Vec<Row> {
    let keys: Vec<&String> = {
        let mut seen = BTreeSet::new();
        before.iter().chain(after).map(|(k, _)| k).filter(|k| seen.insert(*k)).collect()
    };
    let find = |s: &[(String, toml::Table)], k: &str| s.iter().find(|(x, _)| x == k).map(|(_, t)| t.clone());
    let mut rows = Vec::new();
    for k in keys {
        let (a, b) = (find(before, k), find(after, k));
        let fields: Vec<String> = {
            let mut seen = BTreeSet::new();
            a.iter().chain(b.iter()).flat_map(|t| t.keys().cloned()).filter(|f| seen.insert(f.clone())).collect()
        };
        let values: Vec<Value> = fields
            .into_iter()
            .filter_map(|f| {
                let (x, y) = (a.as_ref().and_then(|t| t.get(&f)).cloned(), b.as_ref().and_then(|t| t.get(&f)).cloned());
                let same = match (&x, &y) {
                    (Some(x), Some(y)) => layering::same_value(x, y),
                    (None, None) => true,
                    _ => false,
                };
                (!same).then(|| Value {
                    field: f,
                    from: if y.is_some() { From::Yours } else { From::Original },
                    game: y.clone(),
                    yours: y,
                    original: x,
                })
            })
            .collect();
        if !values.is_empty() {
            rows.push(Row { kind, key: k.clone(), added: a.is_none(), values });
        }
    }
    rows
}

/// `pack`'s layer with the value at `field` of item `key` given back to the lower layers: the field taken out
/// of the pack's entry (`disabled` too), and an entry left with nothing to give taken out whole. False when the
/// pack does not give that value.
pub fn reset(layer: &mut Layer, key: &str, field: &str) -> bool {
    let Some(i) = layer.entries.iter().position(|e| e.key == key) else { return false };
    let e = &mut layer.entries[i];
    let done = if field == "disabled" {
        e.disabled.take().is_some()
    } else {
        let top = field.split(['.', '[']).next().unwrap_or(field);
        e.fields.remove(top).is_some()
    };
    // An entry left giving nothing is taken out (an addition with no fields left is no addition).
    if done && e.fields.is_empty() && e.disabled.is_none() {
        layer.entries.remove(i);
    }
    done
}

#[cfg(test)]
mod tests;
