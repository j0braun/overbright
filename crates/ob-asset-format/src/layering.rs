//! Layering (layering.md): the layer stack (the base set and the enabled mod packs, checked against their
//! dependencies and conflicts, §2) and the merge of one kind's items through it (§3): `merge`, `add`, `replace`,
//! `disabled`, recursive sub-tables, lists replaced whole or merged by key, the base order kept and additions
//! appended, and the clash list (§4).
//!
//! A pack is all or nothing (conventions.md §11): a pack with any refused entry contributes nothing, and the
//! refusal names the pack, the item and the rule.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::manifest::Manifest;
use crate::{Refusal, Rule};

// ---- the stack ----------------------------------------------------------------------------------------------------

/// The mounted stack: pack ids bottom to top (the base set first), and the packs refused with their reasons.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stack {
    pub layers: Vec<String>,
    pub refused: Vec<(String, Refusal)>,
}

fn version_ok(req: &str, m: &Manifest) -> bool {
    semver::VersionReq::parse(req).is_ok_and(|r| r.matches(&m.version))
}

/// Builds the stack from the base set's id, the enabled packs strongest first (`order.txt`, or a test's explicit
/// list) and the installed packs' manifests. Refused (layering.md §6): a dependency missing, off, of another version
/// or above its dependent (`ORD.2`); the stronger of two packs in conflict (`ORD.3`); a pack whose dependency was
/// refused (`ORD.4`, naming it).
pub fn build_stack(base: &str, enabled_strongest_first: &[String], packs: &BTreeMap<String, Manifest>) -> Stack {
    let bottom_up: Vec<&String> = enabled_strongest_first.iter().rev().filter(|id| packs.contains_key(*id)).collect();
    let pos: HashMap<&str, usize> = bottom_up.iter().enumerate().map(|(i, id)| (id.as_str(), i)).collect();
    let mut refused: BTreeMap<String, Refusal> = BTreeMap::new();
    for (i, id) in bottom_up.iter().enumerate() {
        let m = &packs[id.as_str()];
        for (dep, req) in &m.dependencies {
            let why = match (packs.get(dep), pos.get(dep.as_str())) {
                (None, _) => Some(format!("{id} needs {dep}, which is not installed")),
                (Some(_), None) => Some(format!("{id} needs {dep}, which is off")),
                (Some(d), Some(_)) if !version_ok(req, d) => {
                    Some(format!("{id} needs {dep} {req}, installed is {}", d.version))
                }
                (Some(_), Some(&j)) if j > i => Some(format!("{id} needs {dep} below it")),
                _ => None,
            };
            if let Some(why) = why {
                refused.entry(id.to_string()).or_insert_with(|| Refusal::new(Rule::Ord2, why).in_pack(id));
            }
        }
        // The stronger of two packs in conflict is refused, so the weaker keeps working.
        for other in bottom_up[..i].iter() {
            let o = &packs[other.as_str()];
            let clash = m.conflicts.get(other.as_str()).is_some_and(|r| version_ok(r, o))
                || o.conflicts.get(id.as_str()).is_some_and(|r| version_ok(r, m));
            if clash {
                refused.entry(id.to_string()).or_insert_with(|| {
                    Refusal::new(Rule::Ord3, format!("{id} and {other} are never on together")).in_pack(id)
                });
            }
        }
    }
    // A pack whose dependency is refused for any reason is refused too (to a fixed point).
    loop {
        let mut more = Vec::new();
        for id in &bottom_up {
            if refused.contains_key(id.as_str()) {
                continue;
            }
            if let Some(dep) = packs[id.as_str()].dependencies.keys().find(|d| refused.contains_key(d.as_str())) {
                more.push((
                    id.to_string(),
                    Refusal::new(Rule::Ord4, format!("its dependency {dep} was refused")).in_pack(id),
                ));
            }
        }
        if more.is_empty() {
            break;
        }
        refused.extend(more);
    }
    let mut layers = vec![base.to_string()];
    layers.extend(bottom_up.iter().filter(|id| !refused.contains_key(id.as_str())).map(|id| id.to_string()));
    let mut refused: Vec<(String, Refusal)> = refused.into_iter().collect();
    refused.sort_by_key(|(id, _)| std::cmp::Reverse(pos[id.as_str()]));
    Stack { layers, refused }
}

// ---- items --------------------------------------------------------------------------------------------------------

/// An entry's operation (§3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Merge,
    Add,
    Replace,
}

/// How a list field merges (§3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKey {
    /// By a field of its elements.
    Field(&'static str),
    /// By position: a base element's key is its `index` in the base set's list; an element a pack adds is keyed
    /// `<pack id>:<n>`, its position among that pack's own additions.
    Index,
}

/// What a kind allows (its page's section 4).
#[derive(Clone, Copy, Debug)]
pub struct KindRules {
    pub add: bool,
    pub replace: bool,
    pub disable: bool,
    /// List fields merged by key instead of replaced whole: (field, key).
    pub keyed_lists: &'static [(&'static str, ListKey)],
}

impl KindRules {
    pub const ALL: KindRules = KindRules { add: true, replace: true, disable: true, keyed_lists: &[] };
}

/// One entry of a pack file: an item's key, its operation, `disabled`, and the fields it gives.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub key: String,
    /// `None`: the default (`merge`, or `add` for a key in the pack's own namespace).
    pub op: Option<Op>,
    pub disabled: Option<bool>,
    pub fields: toml::Table,
}

fn read_op(t: &mut toml::Table) -> Result<Option<Op>, Refusal> {
    match t.remove("op") {
        None => Ok(None),
        Some(toml::Value::String(s)) => match s.as_str() {
            "merge" => Ok(Some(Op::Merge)),
            "add" => Ok(Some(Op::Add)),
            "replace" => Ok(Some(Op::Replace)),
            other => Err(Refusal::new(Rule::Mrg3, format!("unknown operation {other:?}")).in_field("op")),
        },
        Some(_) => Err(Refusal::new(Rule::Conv6, "`op` is not a string").in_field("op")),
    }
}

fn read_disabled(t: &mut toml::Table) -> Result<Option<bool>, Refusal> {
    match t.remove("disabled") {
        None => Ok(None),
        Some(toml::Value::Boolean(b)) => Ok(Some(b)),
        Some(_) => Err(Refusal::new(Rule::Conv6, "`disabled` is not a boolean").in_field("disabled")),
    }
}

impl Entry {
    /// An entry from its table: `op` and `disabled` taken out, the rest are the item's fields.
    pub fn from_table(key: &str, mut t: toml::Table) -> Result<Entry, Refusal> {
        let op = read_op(&mut t)?;
        let disabled = read_disabled(&mut t)?;
        Ok(Entry { key: key.to_string(), op, disabled, fields: t })
    }
}

/// One pack's entries for this kind, in the order of its files and entries.
#[derive(Clone, Debug)]
pub struct Layer {
    pub pack: String,
    /// The packs this one lists in `[dependencies]` (may merge or replace what they added).
    pub dependencies: Vec<String>,
    pub entries: Vec<Entry>,
}

/// Who defined an item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Base,
    Added(String),
}

/// One merged item.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub key: String,
    pub fields: toml::Table,
    pub disabled: bool,
    pub source: Source,
    /// The keys of the elements of index-keyed lists, by field.
    pub list_keys: BTreeMap<String, Vec<String>>,
}

/// What two mod packs both touched (§4). The stronger layer's value wins; a clash is never an error.
#[derive(Clone, Debug, PartialEq)]
pub struct Clash {
    pub key: String,
    /// The field (`a.b`, `list[key].c`), or `None` for the whole item.
    pub field: Option<String>,
    pub what: ClashKind,
    pub weaker: String,
    pub stronger: String,
    /// The two values (for a field clash), weaker first.
    pub values: Option<(toml::Value, toml::Value)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClashKind {
    /// Both gave a value for the field.
    Field,
    /// Both added the item.
    Add,
    /// One disabled (or enabled) what the other changed.
    DisableVsChange,
}

/// The merged view of one kind.
#[derive(Clone, Debug, Default)]
pub struct Merged {
    pub items: Vec<Item>,
    pub clashes: Vec<Clash>,
    /// Packs refused while merging, with the first reason each.
    pub refused: Vec<(String, Refusal)>,
}

impl Merged {
    pub fn get(&self, key: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.key == key)
    }
}

/// Values compared the format's way (conventions.md §3.4): floats by their bits, everything else structurally.
/// Writers use it to drop a field equal to the lower layers' value.
pub fn same_value(a: &toml::Value, b: &toml::Value) -> bool {
    use toml::Value as V;
    match (a, b) {
        (V::Float(x), V::Float(y)) => crate::float::same_f64(*x, *y),
        (V::Array(x), V::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(a, b)| same_value(a, b)),
        (V::Table(x), V::Table(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same_value(v, w)))
        }
        _ => a == b,
    }
}

/// The merge state while layers apply.
struct State {
    items: Vec<Item>,
    index: HashMap<String, usize>,
    /// (item key, field path) → (pack, value) of the last mod pack that set it.
    writers: HashMap<(String, String), (String, toml::Value)>,
    /// item key → mod packs that changed its fields.
    changers: HashMap<String, Vec<String>>,
    /// item key → the mod pack that last set `disabled`.
    disablers: HashMap<String, String>,
    clashes: Vec<Clash>,
}

/// Whether `key` lies in `pack`'s own namespace (`<pack>:…`, possibly after a unit prefix: `L4RA/docks-remix:a12`).
pub fn in_namespace(key: &str, pack: &str) -> bool {
    key.split('/').any(|part| part.strip_prefix(pack).is_some_and(|r| r.starts_with(':')))
}

/// Merges one kind: the base set's items in their order, then each layer bottom to top. A layer with any refused
/// entry is left out whole and listed in [`Merged::refused`]; the others still apply.
pub fn merge(base: Vec<(String, toml::Table)>, layers: &[Layer], rules: &KindRules) -> Merged {
    let mut st = State {
        items: Vec::with_capacity(base.len()),
        index: HashMap::new(),
        writers: HashMap::new(),
        changers: HashMap::new(),
        disablers: HashMap::new(),
        clashes: Vec::new(),
    };
    for (key, fields) in base {
        let list_keys = rules
            .keyed_lists
            .iter()
            .filter(|(_, k)| *k == ListKey::Index)
            .filter_map(|(f, _)| {
                fields
                    .get(*f)
                    .and_then(|v| v.as_array())
                    .map(|a| (f.to_string(), (0..a.len()).map(|i| i.to_string()).collect()))
            })
            .collect();
        st.index.insert(key.clone(), st.items.len());
        st.items.push(Item { key, fields, disabled: false, source: Source::Base, list_keys });
    }
    let mut refused = Vec::new();
    for layer in layers {
        // All or nothing: apply to a copy, keep it only when every entry passed.
        let mut trial = State {
            items: st.items.clone(),
            index: st.index.clone(),
            writers: st.writers.clone(),
            changers: st.changers.clone(),
            disablers: st.disablers.clone(),
            clashes: st.clashes.clone(),
        };
        match apply_layer(&mut trial, layer, rules) {
            Ok(()) => st = trial,
            Err(r) => refused.push((layer.pack.clone(), r.in_pack(&layer.pack))),
        }
    }
    Merged { items: st.items, clashes: st.clashes, refused }
}

fn apply_layer(st: &mut State, layer: &Layer, rules: &KindRules) -> Result<(), Refusal> {
    let mut added_here = HashSet::new();
    for e in &layer.entries {
        let at = |r: Refusal| r.in_field(&e.key);
        let own = in_namespace(&e.key, &layer.pack);
        let op = e.op.unwrap_or(if own { Op::Add } else { Op::Merge });
        if (op == Op::Add && !rules.add)
            || (op == Op::Replace && !rules.replace)
            || (e.disabled.is_some() && !rules.disable)
        {
            let what = if e.disabled.is_some() && !rules.disable {
                "`disabled`"
            } else if op == Op::Add {
                "`add`"
            } else {
                "`replace`"
            };
            return Err(at(Refusal::new(Rule::Mrg3, format!("{what} is not allowed for this kind"))));
        }
        let existing = st.index.get(&e.key).copied();
        if let Some(i) = existing {
            // Merging or replacing what another pack added needs that pack below as a dependency.
            if let Source::Added(owner) = &st.items[i].source
                && op != Op::Add
                && owner != &layer.pack
                && !layer.dependencies.contains(owner)
            {
                return Err(at(Refusal::new(
                    Rule::Ord2,
                    format!("changes an item {owner} added without depending on it"),
                )));
            }
        }
        match (op, existing) {
            (Op::Merge | Op::Replace, None) => {
                return Err(at(Refusal::new(Rule::Mrg1, "no lower layer has this item")));
            }
            (Op::Add, Some(i)) => {
                let item = &st.items[i];
                match &item.source {
                    Source::Added(owner) if owner != &layer.pack && !added_here.contains(&e.key) => {
                        // Two packs adding the same key: the stronger definition wins, shown as a clash.
                        st.clashes.push(Clash {
                            key: e.key.clone(),
                            field: None,
                            what: ClashKind::Add,
                            weaker: owner.clone(),
                            stronger: layer.pack.clone(),
                            values: None,
                        });
                        let mut fresh = Item {
                            key: e.key.clone(),
                            fields: toml::Table::new(),
                            disabled: false,
                            source: Source::Added(layer.pack.clone()),
                            list_keys: BTreeMap::new(),
                        };
                        set_fields(st, &mut fresh, layer, &e.fields, rules, false)?;
                        fresh.disabled = e.disabled.unwrap_or(false);
                        st.items[i] = fresh;
                        added_here.insert(e.key.clone());
                    }
                    _ => return Err(at(Refusal::new(Rule::Mrg2, "a lower layer already has this item (use merge)"))),
                }
            }
            (Op::Add, None) => {
                let mut item = Item {
                    key: e.key.clone(),
                    fields: toml::Table::new(),
                    disabled: false,
                    source: Source::Added(layer.pack.clone()),
                    list_keys: BTreeMap::new(),
                };
                set_fields(st, &mut item, layer, &e.fields, rules, false)?;
                item.disabled = e.disabled.unwrap_or(false);
                st.index.insert(e.key.clone(), st.items.len());
                st.items.push(item);
                added_here.insert(e.key.clone());
            }
            (Op::Merge | Op::Replace, Some(i)) => {
                let mut item = st.items[i].clone();
                if op == Op::Replace {
                    // The whole item is this entry; absent fields take the schema's defaults, not the lower values.
                    for (path, (pack, _)) in st.writers.iter().filter(|((k, _), _)| k == &e.key) {
                        if pack != &layer.pack {
                            st.clashes.push(Clash {
                                key: e.key.clone(),
                                field: Some(path.1.clone()),
                                what: ClashKind::Field,
                                weaker: pack.clone(),
                                stronger: layer.pack.clone(),
                                values: None,
                            });
                        }
                    }
                    st.writers.retain(|(k, _), _| k != &e.key);
                    item.fields = toml::Table::new();
                    item.list_keys.clear();
                }
                set_fields(st, &mut item, layer, &e.fields, rules, op == Op::Merge)?;
                if !e.fields.is_empty() {
                    if let Some(d) = st.disablers.get(&e.key).filter(|d| *d != &layer.pack) {
                        st.clashes.push(Clash {
                            key: e.key.clone(),
                            field: None,
                            what: ClashKind::DisableVsChange,
                            weaker: d.clone(),
                            stronger: layer.pack.clone(),
                            values: None,
                        });
                    }
                    st.changers.entry(e.key.clone()).or_default().push(layer.pack.clone());
                }
                if let Some(d) = e.disabled {
                    if let Some(c) = st.changers.get(&e.key).and_then(|c| c.iter().rev().find(|c| *c != &layer.pack)) {
                        st.clashes.push(Clash {
                            key: e.key.clone(),
                            field: None,
                            what: ClashKind::DisableVsChange,
                            weaker: c.clone(),
                            stronger: layer.pack.clone(),
                            values: None,
                        });
                    }
                    item.disabled = d;
                    st.disablers.insert(e.key.clone(), layer.pack.clone());
                }
                st.items[i] = item;
            }
        }
    }
    Ok(())
}

/// Records that `layer` set `path` of `key` to `value`, and a clash when another mod pack had set it.
fn record(st: &mut State, key: &str, path: &str, layer: &Layer, value: &toml::Value) {
    let slot = (key.to_string(), path.to_string());
    if let Some((pack, old)) = st.writers.get(&slot)
        && pack != &layer.pack
    {
        st.clashes.push(Clash {
            key: key.to_string(),
            field: Some(path.to_string()),
            what: ClashKind::Field,
            weaker: pack.clone(),
            stronger: layer.pack.clone(),
            values: Some((old.clone(), value.clone())),
        });
    }
    st.writers.insert(slot, (layer.pack.clone(), value.clone()));
}

/// Applies an entry's fields to an item: sub-tables merged field by field (when `merge`), keyed lists by key,
/// everything else replaced whole.
fn set_fields(
    st: &mut State,
    item: &mut Item,
    layer: &Layer,
    fields: &toml::Table,
    rules: &KindRules,
    merge: bool,
) -> Result<(), Refusal> {
    for (k, v) in fields {
        if let Some((_, lk)) = rules.keyed_lists.iter().find(|(f, _)| f == k) {
            merge_list(st, item, layer, k, *lk, v).map_err(|r| r.in_field(k))?;
            continue;
        }
        let key = item.key.clone();
        let slot = item.fields.get_mut(k);
        match (slot, v) {
            (Some(toml::Value::Table(have)), toml::Value::Table(new)) if merge => {
                merge_table(st, &key, k, have, new, layer)
            }
            _ => {
                record(st, &key, k, layer, v);
                item.fields.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(())
}

fn merge_table(st: &mut State, key: &str, path: &str, have: &mut toml::Table, new: &toml::Table, layer: &Layer) {
    for (k, v) in new {
        let p = format!("{path}.{k}");
        match (have.get_mut(k), v) {
            (Some(toml::Value::Table(h)), toml::Value::Table(n)) => merge_table(st, key, &p, h, n, layer),
            _ => {
                record(st, key, &p, layer, v);
                have.insert(k.clone(), v.clone());
            }
        }
    }
}

/// Merges a keyed list (§3.2): an element with a known key is merged (or replaced), `disabled` hides it, `op = "add"`
/// appends; an unknown key without `add` is `MRG.4`.
fn merge_list(
    st: &mut State,
    item: &mut Item,
    layer: &Layer,
    field: &str,
    lk: ListKey,
    v: &toml::Value,
) -> Result<(), Refusal> {
    let new = v.as_array().ok_or_else(|| Refusal::new(Rule::Conv6, "a list was expected"))?;
    let mut list: Vec<toml::Value> = item.fields.get(field).and_then(|l| l.as_array()).cloned().unwrap_or_default();
    let mut keys: Vec<String> = match lk {
        ListKey::Index => {
            item.list_keys.get(field).cloned().unwrap_or_else(|| (0..list.len()).map(|i| i.to_string()).collect())
        }
        ListKey::Field(f) => list.iter().map(|e| e.get(f).map(element_key).unwrap_or_default()).collect(),
    };
    let mut own_added = keys.iter().filter(|k| k.starts_with(&format!("{}:", layer.pack))).count();
    let item_key = item.key.clone();
    for el in new {
        let mut t = el.as_table().cloned().ok_or_else(|| Refusal::new(Rule::Conv6, "list elements are tables"))?;
        let op = read_op(&mut t)?;
        let disabled = read_disabled(&mut t)?;
        let given = match lk {
            ListKey::Index => t.remove("index").map(|k| element_key(&k)),
            ListKey::Field(f) => t.get(f).map(element_key),
        };
        let pos = given.as_ref().and_then(|k| keys.iter().position(|have| have == k));
        match (op, pos) {
            (Some(Op::Add), Some(_)) => {
                return Err(Refusal::new(Rule::Mrg2, format!("element {} exists", given.unwrap())));
            }
            (Some(Op::Add), None) => {
                let k = match (lk, given) {
                    (_, Some(k)) => k,
                    (ListKey::Index, None) => {
                        let k = format!("{}:{own_added}", layer.pack);
                        own_added += 1;
                        k
                    }
                    (ListKey::Field(f), None) => {
                        return Err(Refusal::new(Rule::Conv5, format!("an added element needs `{f}`")));
                    }
                };
                if let Some(d) = disabled {
                    t.insert("disabled".into(), toml::Value::Boolean(d));
                }
                for (f, v) in &t {
                    record(st, &item_key, &format!("{field}[{k}].{f}"), layer, v);
                }
                list.push(toml::Value::Table(t));
                keys.push(k);
            }
            (_, None) => {
                return Err(Refusal::new(
                    Rule::Mrg4,
                    format!("no element with the key {}", given.unwrap_or_else(|| "(none)".into())),
                ));
            }
            (op, Some(i)) => {
                let path = format!("{field}[{}]", keys[i]);
                let target =
                    list[i].as_table_mut().ok_or_else(|| Refusal::new(Rule::Conv6, "list elements are tables"))?;
                if op == Some(Op::Replace) {
                    let keep = match lk {
                        ListKey::Field(f) => target.get(f).cloned().map(|v| (f, v)),
                        ListKey::Index => None,
                    };
                    target.clear();
                    if let Some((f, v)) = keep {
                        target.insert(f.to_string(), v);
                    }
                }
                merge_table(st, &item_key, &path, target, &t, layer);
                if let Some(d) = disabled {
                    record(st, &item_key, &format!("{path}.disabled"), layer, &toml::Value::Boolean(d));
                    target.insert("disabled".into(), toml::Value::Boolean(d));
                }
            }
        }
    }
    if lk == ListKey::Index {
        item.list_keys.insert(field.to_string(), keys);
    }
    item.fields.insert(field.to_string(), toml::Value::Array(list));
    Ok(())
}

fn element_key(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        other => crate::text::toml_inline(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Container, Manifest};

    fn t(text: &str) -> toml::Table {
        text.parse().unwrap()
    }

    fn entry(key: &str, text: &str) -> Entry {
        Entry::from_table(key, t(text)).unwrap()
    }

    fn layer(pack: &str, deps: &[&str], entries: Vec<Entry>) -> Layer {
        Layer { pack: pack.into(), dependencies: deps.iter().map(|s| s.to_string()).collect(), entries }
    }

    fn base() -> Vec<(String, toml::Table)> {
        vec![
            ("PEUGOT".into(), t("mass_kg = 1200.0\ntop = 1.0\n[engine]\nrpm_max = 7000\nidle = 900")),
            ("COROLLA".into(), t("mass_kg = 1100.0\ntop = -0.0")),
        ]
    }

    #[test]
    fn operations() {
        let m = merge(
            base(),
            &[
                layer(
                    "a",
                    &[],
                    vec![
                        entry("PEUGOT", "mass_kg = 1250.0\n[engine]\nidle = 950"),
                        entry("NEWCAR", "op = \"add\"\nmass_kg = 900.0"),
                    ],
                ),
                layer(
                    "b",
                    &[],
                    vec![entry("COROLLA", "op = \"replace\"\nmass_kg = 1000.0"), entry("PEUGOT", "disabled = true")],
                ),
            ],
            &KindRules::ALL,
        );
        assert!(m.refused.is_empty(), "{:?}", m.refused);
        let p = m.get("PEUGOT").unwrap();
        assert_eq!(p.fields["mass_kg"].as_float(), Some(1250.0));
        // Sub-tables merge field by field: the untouched rpm_max stays.
        assert_eq!(p.fields["engine"]["rpm_max"].as_integer(), Some(7000));
        assert_eq!(p.fields["engine"]["idle"].as_integer(), Some(950));
        assert!(p.disabled);
        // Replace drops the lower values.
        let c = m.get("COROLLA").unwrap();
        assert_eq!(c.fields.len(), 1);
        // Order: the base items in place, then additions.
        let keys: Vec<&str> = m.items.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, ["PEUGOT", "COROLLA", "NEWCAR"]);
        assert_eq!(m.get("NEWCAR").unwrap().source, Source::Added("a".into()));
        // b disabled what a changed.
        assert_eq!(m.clashes.len(), 1);
        assert_eq!(
            (m.clashes[0].what, m.clashes[0].weaker.as_str(), m.clashes[0].stronger.as_str()),
            (ClashKind::DisableVsChange, "a", "b")
        );
        // A higher layer enables it again.
        let again = merge(
            base(),
            &[
                layer("a", &[], vec![entry("PEUGOT", "disabled = true")]),
                layer("b", &[], vec![entry("PEUGOT", "disabled = false")]),
            ],
            &KindRules::ALL,
        );
        assert!(!again.get("PEUGOT").unwrap().disabled);
    }

    #[test]
    fn field_clashes() {
        let m = merge(
            base(),
            &[
                layer("a", &[], vec![entry("PEUGOT", "mass_kg = 1250.0")]),
                layer("b", &[], vec![entry("PEUGOT", "mass_kg = 1300.0\ntop = 2.0")]),
            ],
            &KindRules::ALL,
        );
        assert_eq!(m.get("PEUGOT").unwrap().fields["mass_kg"].as_float(), Some(1300.0));
        assert_eq!(m.clashes.len(), 1);
        let c = &m.clashes[0];
        assert_eq!((c.field.as_deref(), c.weaker.as_str(), c.stronger.as_str()), (Some("mass_kg"), "a", "b"));
        assert_eq!(c.values.as_ref().unwrap().0.as_float(), Some(1250.0));
        // Base versus a pack is not a clash.
        let one = merge(base(), &[layer("a", &[], vec![entry("PEUGOT", "mass_kg = 1250.0")])], &KindRules::ALL);
        assert!(one.clashes.is_empty());
    }

    #[test]
    fn refusals_keep_the_pack_out_whole() {
        let rules = KindRules::ALL;
        let one = |entries: Vec<Entry>, rules: &KindRules| merge(base(), &[layer("a", &[], entries)], rules);
        let rule = |m: Merged| {
            assert!(
                m.get("PEUGOT").unwrap().fields["mass_kg"].as_float() == Some(1200.0),
                "the refused pack changed nothing"
            );
            m.refused[0].1.rule
        };
        assert_eq!(
            rule(one(vec![entry("PEUGOT", "mass_kg = 1.0"), entry("NOPE", "mass_kg = 1.0")], &rules)),
            Rule::Mrg1
        );
        assert_eq!(
            rule(one(vec![entry("PEUGOT", "mass_kg = 1.0"), entry("NOPE", "op = \"replace\"")], &rules)),
            Rule::Mrg1
        );
        assert_eq!(rule(one(vec![entry("PEUGOT", "op = \"add\"\nmass_kg = 1.0")], &rules)), Rule::Mrg2);
        let no_add = KindRules { add: false, replace: false, disable: false, keyed_lists: &[] };
        assert_eq!(rule(one(vec![entry("PEUGOT", "mass_kg = 1.0"), entry("X", "op = \"add\"")], &no_add)), Rule::Mrg3);
        assert_eq!(rule(one(vec![entry("PEUGOT", "disabled = true")], &no_add)), Rule::Mrg3);
        assert_eq!(Entry::from_table("X", t("op = \"delete\"")).unwrap_err().rule, Rule::Mrg3);
        let r = one(vec![entry("NOPE", "x = 1")], &rules).refused[0].1.clone();
        assert_eq!((r.pack.as_deref(), r.field.as_deref()), (Some("a"), Some("NOPE")));
    }

    #[test]
    fn pack_scoped_keys() {
        // An entry in the pack's own namespace defaults to add; another pack changes it only as a dependent.
        let added = layer("docks-remix", &[], vec![entry("L4RA/docks-remix:a12", "solid = \"XO_CONE_01\"")]);
        let m = merge(
            vec![],
            &[added.clone(), layer("other", &[], vec![entry("L4RA/docks-remix:a12", "solid = \"X\"")])],
            &KindRules::ALL,
        );
        assert_eq!(m.items.len(), 1);
        assert_eq!(m.refused[0].1.rule, Rule::Ord2);
        let m = merge(
            vec![],
            &[added, layer("other", &["docks-remix"], vec![entry("L4RA/docks-remix:a12", "solid = \"X\"")])],
            &KindRules::ALL,
        );
        assert!(m.refused.is_empty());
        assert_eq!(m.items[0].fields["solid"].as_str(), Some("X"));
        assert!(in_namespace("race:docks-remix:sprint".split_once(':').unwrap().1, "docks-remix"));
        assert!(!in_namespace("L4RA/docks:a1", "docks-remix"));
        // Two packs adding the same new name: a clash, the stronger wins.
        let m = merge(
            vec![],
            &[
                layer("a", &[], vec![entry("NEWNAME", "op = \"add\"\nv = 1")]),
                layer("b", &[], vec![entry("NEWNAME", "op = \"add\"\nv = 2")]),
            ],
            &KindRules::ALL,
        );
        assert_eq!(m.items[0].fields["v"].as_integer(), Some(2));
        assert_eq!(m.clashes[0].what, ClashKind::Add);
    }

    #[test]
    fn keyed_lists() {
        const RULES: KindRules = KindRules {
            add: true,
            replace: true,
            disable: true,
            keyed_lists: &[("events", ListKey::Field("name")), ("slots", ListKey::Index)],
        };
        let b = vec![(
            "C".to_string(),
            t(
                "events = [{ name = \"e1\", v = 1 }, { name = \"e2\", v = 2 }]\nslots = [{ v = 10 }, { v = 11 }]\nplain = [1, 2]",
            ),
        )];
        let m = merge(
            b.clone(),
            &[layer(
                "p",
                &[],
                vec![entry(
                    "C",
                    "events = [{ name = \"e2\", v = 20 }, { op = \"add\", name = \"e3\", v = 3 }, { name = \"e1\", disabled = true }]\n\
                 slots = [{ index = 1, v = 111 }, { op = \"add\", v = 12 }]\nplain = [3]",
                )],
            )],
            &RULES,
        );
        assert!(m.refused.is_empty(), "{:?}", m.refused);
        let c = m.get("C").unwrap();
        let ev = c.fields["events"].as_array().unwrap();
        assert_eq!(ev.len(), 3);
        assert_eq!(ev[0]["disabled"].as_bool(), Some(true));
        assert_eq!(ev[1]["v"].as_integer(), Some(20));
        assert_eq!(ev[2]["name"].as_str(), Some("e3"));
        let slots = c.fields["slots"].as_array().unwrap();
        assert_eq!((slots[1]["v"].as_integer(), slots[2]["v"].as_integer()), (Some(111), Some(12)));
        assert_eq!(c.list_keys["slots"], ["0", "1", "p:0"]);
        // Unkeyed lists are replaced whole.
        assert_eq!(c.fields["plain"].as_array().unwrap().len(), 1);
        // A higher pack depending on p can address p's added element by its key.
        let m2 = merge(
            b.clone(),
            &[
                layer("p", &[], vec![entry("C", "slots = [{ op = \"add\", v = 12 }]")]),
                layer("q", &["p"], vec![entry("C", "slots = [{ index = \"p:0\", v = 13 }]")]),
            ],
            &RULES,
        );
        assert!(m2.refused.is_empty(), "{:?}", m2.refused);
        assert_eq!(m2.get("C").unwrap().fields["slots"][2]["v"].as_integer(), Some(13));
        assert_eq!(m2.clashes.len(), 1);
        let bad = merge(b, &[layer("p", &[], vec![entry("C", "events = [{ name = \"e9\", v = 1 }]")])], &RULES);
        assert_eq!(bad.refused[0].1.rule, Rule::Mrg4);
    }

    #[test]
    fn floats_compare_by_bits() {
        let (z, nz) = (toml::Value::Float(0.0), toml::Value::Float(-0.0));
        assert!(!same_value(&z, &nz));
        assert!(same_value(&toml::Value::Float(f64::NAN), &toml::Value::Float(f64::NAN)));
        assert!(same_value(&toml::Value::Table(t("a = [1.5, -0.0]")), &toml::Value::Table(t("a = [1.5, -0.0]"))));
    }

    fn pack(text: &str) -> Manifest {
        Manifest::parse(text.as_bytes(), Container::Unknown).unwrap().0
    }

    fn packs(list: &[(&str, &str, &str)]) -> BTreeMap<String, Manifest> {
        list.iter()
            .map(|(id, version, extra)| {
                (id.to_string(), pack(&format!(
                    "format = 2\nkind = \"mod\"\nid = \"{id}\"\nname = \"{id}\"\nversion = \"{version}\"\noverbright = \"*\"\norigin = \"own-work\"\nshare = \"local-only\"\n{extra}"
                )))
            })
            .collect()
    }

    #[test]
    fn stack_rules() {
        let installed = packs(&[
            ("docks-remix", "1.2.0", "[dependencies]\ncone-pack = \">=1.0\"\n[conflicts]\nold-docks = \"*\""),
            ("cone-pack", "1.1.0", ""),
            ("old-docks", "0.1.0", ""),
            ("addon", "1.0.0", "[dependencies]\ndocks-remix = \"*\""),
        ]);
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let s = build_stack("nfsu2-pc12-eu", &ids(&["addon", "docks-remix", "cone-pack"]), &installed);
        assert_eq!(s.layers, ["nfsu2-pc12-eu", "cone-pack", "docks-remix", "addon"]);
        assert!(s.refused.is_empty());
        // Dependencies above their dependents.
        let s = build_stack("b", &ids(&["cone-pack", "docks-remix", "addon"]), &installed);
        assert_eq!(
            s.refused.iter().map(|(id, r)| (id.as_str(), r.rule)).collect::<Vec<_>>(),
            [("docks-remix", Rule::Ord2), ("addon", Rule::Ord2)]
        );
        assert_eq!(s.layers, ["b", "cone-pack"]);
        // A dependency off; a refused dependency refuses its dependents too (ORD.4, naming it).
        let s = build_stack("b", &ids(&["addon", "docks-remix"]), &installed);
        assert_eq!(
            s.refused.iter().map(|(id, r)| (id.as_str(), r.rule)).collect::<Vec<_>>(),
            [("addon", Rule::Ord4), ("docks-remix", Rule::Ord2)]
        );
        assert!(s.refused[0].1.message.contains("docks-remix"));
        assert_eq!(s.layers, ["b"]);
        // A conflict refuses the stronger of the two.
        let s = build_stack("b", &ids(&["old-docks", "docks-remix", "cone-pack"]), &installed);
        assert_eq!(s.refused, [("old-docks".to_string(), s.refused[0].1.clone())]);
        assert_eq!(s.refused[0].1.rule, Rule::Ord3);
        // A dependency of the wrong version.
        let old =
            packs(&[("docks-remix", "1.2.0", "[dependencies]\ncone-pack = \">=2.0\""), ("cone-pack", "1.1.0", "")]);
        assert_eq!(build_stack("b", &ids(&["docks-remix", "cone-pack"]), &old).refused[0].1.rule, Rule::Ord2);
    }
}
