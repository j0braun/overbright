//! Footer texts and package text helpers.

use crate::frontend::fe_ids;
use crate::frontend::ScreenState;

/// Footer texts the PC build fills with key names: the button glyph
/// placeholder goes, the label says which key.
pub(crate) fn footer(state: &mut ScreenState, package: &crate::frontend::feng::Package, accept: &str) {
    for o in &package.objects {
        match o.text.as_str() {
            t if t.starts_with("$JOY_EVENT_") => {
                state.hidden.insert(o.hash);
            }
            "Help" | "DEFAULT STRING" | "Event Handler" => {
                state.hidden.insert(o.hash);
            }
            "Select Event" | "Select Car" | "Select" => {
                state.text.insert(o.hash, format!("{}: {accept}", crate::pad::extras::accept_name()));
            }
            _ => {}
        }
    }
}

/// The graphics screens' Total Cost (group 0x9FF84A88, "00000"
/// 0x2F5E7DA3): in a career the setup's cost as `%$d` (0x5453C0 shows
/// the group), outside one hidden (0x545470).
pub fn total_cost(state: &mut ScreenState, total: Option<i32>) {
    match total {
        Some(t) => {
            state.text.insert(0x2F5E_7DA3, crate::frontend::grouped(i64::from(t)));
        }
        None => {
            state.hidden.insert(fe_ids::TOTAL_COST);
        }
    }
}

/// Set the text of the package's string authored as `authored`.
pub(crate) fn retitle(state: &mut ScreenState, package: &crate::frontend::feng::Package, authored: &str, text: &str) {
    for o in package.objects.iter().filter(|o| o.text == authored) {
        state.text.insert(o.hash, text.into());
    }
}
