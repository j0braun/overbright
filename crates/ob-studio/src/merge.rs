//! A pack file of a kind as layer entries (layering.md §3), for the keyed merge with per-field attribution
//! ([`ob_asset_format::layering::merge`], [`ob_asset_format::layering::Item::from`]) that Studio's tables, inspector
//! and Pack panel show.

use ob_asset_format::fields;
use ob_asset_format::kinds;
use ob_asset_format::layering::Entry;
use ob_asset_format::{Refusal, Rule};

use crate::registry::Kind;

/// The entries of one file of `kind` in `pack`, checked: a file of the kind's entries (`[[placement]]`) or one item
/// (a race). An entry adding an item is keyed by the pack (`<pack>:<id>`), a merge by its `key`; both after
/// `unit/` when the kind's items sit in a unit (`L4RA/1407/0`). The key, the guard and the file's `schema` are not
/// item fields.
pub fn entries(kind: &Kind, unit: &str, pack: &str, doc: &toml::Table) -> Result<Vec<Entry>, Refusal> {
    match kind.short {
        "placement" => kinds::check_placements(doc)?,
        "race" => kinds::check_race(doc)?,
        other => return Err(Refusal::new(Rule::Mrg3, format!("no file check for the kind {other}"))),
    }
    let items: Vec<toml::Table> = if kind.entries.is_empty() {
        vec![doc.clone()]
    } else {
        let list = doc.get(kind.entries).and_then(|l| l.as_array()).into_iter().flatten();
        list.filter_map(|e| e.as_table().cloned()).collect()
    };
    items
        .into_iter()
        .map(|mut t| {
            let local = if fields::is_addition(&t, "id") {
                format!("{pack}:{}", t.get("id").and_then(|i| i.as_str()).unwrap_or_default())
            } else {
                t.get("key").and_then(|k| k.as_str()).unwrap_or_default().to_string()
            };
            for k in ["key", "guard", "schema"] {
                t.remove(k);
            }
            let key = if unit.is_empty() { local } else { format!("{unit}/{local}") };
            Entry::from_table(&key, t)
        })
        .collect()
}
