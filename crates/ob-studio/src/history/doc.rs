//! Documents and their items: a mod's state is the text of each of its files ([`State`]), and a file is a list
//! of keyed items, so two writers who changed different items of one file both keep their change and only the
//! same item changed two ways is a conflict ([`merge`]). A map mod's items are its details, each prop edit and
//! each race; a texture list's (`looks.toml`) are its textures; a file of any other kind, or one that does not
//! parse, is one item.

use std::collections::{BTreeMap, BTreeSet};

use ob_map::tools::map::format::{CustomRace, MapMod, PropEdit};
use ob_map::tools::map::store;

/// The text of each document of a mod, by its file name inside the mod's folder (`map.obmap`).
pub type State = BTreeMap<String, String>;

/// The one item of a file that has no items of its own.
pub const WHOLE: &str = "file";

/// A document's items in file order: (key, value). None: the file does not parse as its kind.
pub fn items(doc: &str, text: &str) -> Option<Vec<(String, String)>> {
    if doc == crate::looks::FILE {
        // One item per texture.
        return crate::looks::Looks::parse(text).ok().map(|l| l.items());
    }
    if doc != store::FILE {
        return Some(vec![(WHOLE.into(), text.into())]);
    }
    let m = MapMod::parse(text).ok()?;
    let mut out = vec![("details".to_string(), json(&(&m.name, &m.author, &m.description)))];
    let mut seen = BTreeSet::new();
    let mut push = |out: &mut Vec<(String, String)>, key: String, value: String| {
        // Two edits of one placement stay two items.
        let mut k = key.clone();
        let mut n = 1;
        while !seen.insert(k.clone()) {
            n += 1;
            k = format!("{key}#{n}");
        }
        out.push((k, value));
    };
    for p in &m.props {
        push(&mut out, prop_key(p), json(p));
    }
    for r in &m.races {
        push(&mut out, format!("race:{}", r.id), json(r));
    }
    Some(out)
}

/// A prop edit's item key: the added prop's id, or the original placement it changes.
fn prop_key(p: &PropEdit) -> String {
    match (p.add_id(), p.target()) {
        (Some(id), _) => format!("prop:add/{id}"),
        (None, Some(t)) => {
            format!("prop:{}/{}/{}/{} {} {}/{}", t.region, t.section, t.solid, t.at.x, t.at.y, t.at.z, t.nth)
        }
        (None, None) => "prop:?".into(),
    }
}

/// The document made of `items` (as [`items`] gives them). None: an item does not read back.
pub fn join(doc: &str, items: &[(String, String)]) -> Option<String> {
    if doc == crate::looks::FILE {
        return crate::looks::Looks::join(items).map(|l| l.write());
    }
    if doc != store::FILE || items.iter().any(|(k, _)| k == WHOLE) {
        return match items {
            [(k, t)] if k == WHOLE => Some(t.clone()),
            _ => None,
        };
    }
    let mut m = MapMod::default();
    for (k, v) in items {
        if k == "details" {
            (m.name, m.author, m.description) = serde_json::from_str(v).ok()?;
        } else if k.starts_with("prop:") {
            m.props.push(serde_json::from_str::<PropEdit>(v).ok()?);
        } else if k.starts_with("race:") {
            m.races.push(serde_json::from_str::<CustomRace>(v).ok()?);
        } else {
            return None;
        }
    }
    Some(m.write())
}

/// Which side keeps a conflicting item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prefer {
    Ours,
    Theirs,
}

/// `ours` and `theirs`, both made from `base`, as one state: every item only one side changed (or added, or
/// removed) is taken from that side. The conflicts are the items both changed differently, as `doc#key`; they
/// keep `prefer`'s version. A document that does not parse on every side is one item.
pub fn merge(base: &State, ours: &State, theirs: &State, prefer: Prefer) -> (State, Vec<String>) {
    let mut out = State::new();
    let mut conflicts = Vec::new();
    let docs: BTreeSet<&String> = base.keys().chain(ours.keys()).chain(theirs.keys()).collect();
    for doc in docs {
        let (b, o, t) = (base.get(doc), ours.get(doc), theirs.get(doc));
        let whole = |c: &mut Vec<String>| {
            if o == t || b == t {
                o
            } else if b == o {
                t
            } else {
                c.push(format!("{doc}#{WHOLE}"));
                if prefer == Prefer::Ours { o } else { t }
            }
        };
        let merged = match (o, t) {
            (Some(ot), Some(tt)) if o != t && b != o && b != t => {
                let bi = b.map_or(Some(Vec::new()), |b| items(doc, b));
                match (bi, items(doc, ot), items(doc, tt)) {
                    (Some(bi), Some(oi), Some(ti)) => {
                        let before = conflicts.len();
                        let m = merge_items(doc, &bi, &oi, &ti, prefer, &mut conflicts);
                        match join(doc, &m) {
                            Some(text) => Some(text),
                            None => {
                                conflicts.truncate(before);
                                whole(&mut conflicts).cloned()
                            }
                        }
                    }
                    _ => whole(&mut conflicts).cloned(),
                }
            }
            _ => whole(&mut conflicts).cloned(),
        };
        if let Some(text) = merged {
            out.insert(doc.clone(), text);
        }
    }
    (out, conflicts)
}

fn merge_items(
    doc: &str,
    base: &[(String, String)],
    ours: &[(String, String)],
    theirs: &[(String, String)],
    prefer: Prefer,
    conflicts: &mut Vec<String>,
) -> Vec<(String, String)> {
    let map = |v: &[(String, String)]| -> BTreeMap<String, String> { v.iter().cloned().collect() };
    let (b, t) = (map(base), map(theirs));
    let o = map(ours);
    let mut out = Vec::new();
    let mut pick = |k: &str, bv: Option<&String>, ov: Option<&String>, tv: Option<&String>| -> Option<String> {
        if ov == tv || bv == tv {
            ov.cloned()
        } else if bv == ov {
            tv.cloned()
        } else {
            conflicts.push(format!("{doc}#{k}"));
            if prefer == Prefer::Ours { ov.cloned() } else { tv.cloned() }
        }
    };
    for (k, ov) in ours {
        if let Some(v) = pick(k, b.get(k), Some(ov), t.get(k)) {
            out.push((k.clone(), v));
        }
    }
    for (k, tv) in theirs {
        if !o.contains_key(k)
            && let Some(v) = pick(k, b.get(k), None, Some(tv))
        {
            out.push((k.clone(), v));
        }
    }
    out
}

/// The item keys (`doc#key`) that differ between two states.
pub fn changed_items(a: &State, b: &State) -> Vec<String> {
    let docs: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    let mut out = Vec::new();
    for doc in docs {
        let (x, y) = (a.get(doc), b.get(doc));
        if x == y {
            continue;
        }
        let ix = x.map_or(Some(Vec::new()), |t| items(doc, t));
        let iy = y.map_or(Some(Vec::new()), |t| items(doc, t));
        match (ix, iy) {
            (Some(ix), Some(iy)) => {
                let (mx, my): (BTreeMap<_, _>, BTreeMap<_, _>) = (ix.into_iter().collect(), iy.into_iter().collect());
                let keys: BTreeSet<&String> = mx.keys().chain(my.keys()).collect();
                out.extend(keys.into_iter().filter(|k| mx.get(*k) != my.get(*k)).map(|k| format!("{doc}#{k}")));
            }
            _ => out.push(format!("{doc}#{WHOLE}")),
        }
    }
    out
}

/// An item's value: its JSON form.
fn json<T: serde::Serialize + ?Sized>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}
