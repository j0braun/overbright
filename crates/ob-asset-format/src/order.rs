//! The two line-based files (conventions.md §3.6): `order.txt`, the player's order of mod packs and which are on
//! (layering.md §2), and `active.txt`, the base set in use (layout.md §1). The first line names the file and its
//! version; `#` starts a comment line; blank lines are ignored. Writers keep the player's comments and the order of
//! untouched lines.

use crate::names;
use crate::text;
use crate::{Refusal, Rule};

/// `order.txt`'s first line.
pub const ORDER_HEADER: &str = "# Overbright mods v1";
/// `active.txt`'s first line.
pub const ACTIVE_HEADER: &str = "# Overbright base v1";

/// One line of `order.txt` after the first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// A comment or blank line, kept as written.
    Text(String),
    /// A pack id; `on = false` for a `-` line (kept in its place, switched off).
    Pack { id: String, on: bool },
}

/// Something `order.txt` holds that is skipped, not refused (layering.md §2): the player's order survives a
/// removed pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// Line `line` lists `id` again.
    Duplicate { line: u32, id: String },
    /// Line `line` is not a pack id.
    BadLine { line: u32, text: String },
    /// A listed pack that is not installed.
    Missing { id: String },
}

/// `order.txt`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Order {
    pub lines: Vec<Line>,
}

impl Order {
    /// Reads `order.txt` (`ORD.1` without its first line, or of an unknown version). Duplicates and lines that are
    /// no pack id are reported and skipped.
    pub fn parse(bytes: &[u8]) -> Result<(Order, Vec<Notice>), Refusal> {
        let at = |r: Refusal| r.at_path("order.txt");
        let text = text::decode(bytes, text::MAX_TOML_BYTES, Rule::Conv12).map_err(at)?;
        let mut rows = text.lines().map(|l| l.strip_suffix('\r').unwrap_or(l));
        match rows.next() {
            Some(ORDER_HEADER) => {}
            Some(h) if h.starts_with("# Overbright mods v") => {
                return Err(at(Refusal::new(Rule::Ord1, format!("unknown version {:?}", h)).at_line(1)));
            }
            _ => {
                return Err(at(Refusal::new(Rule::Ord1, format!("the first line is not {ORDER_HEADER:?}")).at_line(1)));
            }
        }
        let mut order = Order::default();
        let mut notices = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (i, row) in rows.enumerate() {
            let line = i as u32 + 2;
            let t = row.trim();
            if t.is_empty() || t.starts_with('#') {
                order.lines.push(Line::Text(row.to_string()));
                continue;
            }
            let (id, on) = match t.strip_prefix('-') {
                Some(id) => (id.trim(), false),
                None => (t, true),
            };
            if !names::is_pack_id(id) {
                notices.push(Notice::BadLine { line, text: row.to_string() });
                order.lines.push(Line::Text(row.to_string()));
            } else if !seen.insert(id.to_string()) {
                notices.push(Notice::Duplicate { line, id: id.to_string() });
                order.lines.push(Line::Text(row.to_string()));
            } else {
                order.lines.push(Line::Pack { id: id.to_string(), on });
            }
        }
        Ok((order, notices))
    }

    /// The file's text (LF line ends).
    pub fn render(&self) -> String {
        let mut out = format!("{ORDER_HEADER}\n");
        for l in &self.lines {
            match l {
                Line::Text(t) => out.push_str(t),
                Line::Pack { id, on: true } => out.push_str(id),
                Line::Pack { id, on: false } => {
                    out.push('-');
                    out.push_str(id);
                }
            }
            out.push('\n');
        }
        out
    }

    /// The listed packs, strongest first, with whether each is on.
    pub fn packs(&self) -> impl Iterator<Item = (&str, bool)> {
        self.lines.iter().filter_map(|l| match l {
            Line::Pack { id, on } => Some((id.as_str(), *on)),
            Line::Text(_) => None,
        })
    }

    /// The packs that are on and installed, strongest first; listed packs that are not installed are reported.
    /// A pack installed but not listed is off (shown as new by the launcher).
    pub fn enabled(&self, installed: impl Fn(&str) -> bool) -> (Vec<String>, Vec<Notice>) {
        let mut on = Vec::new();
        let mut notices = Vec::new();
        for (id, enabled) in self.packs() {
            if !installed(id) {
                notices.push(Notice::Missing { id: id.to_string() });
            } else if enabled {
                on.push(id.to_string());
            }
        }
        (on, notices)
    }

    /// Moves a listed pack `by` places among the listed packs (negative: towards the strongest end); comment lines
    /// stay where they are. False when the pack is not listed or the move would leave the list.
    pub fn move_pack(&mut self, id: &str, by: isize) -> bool {
        let at: Vec<usize> = self.lines.iter().enumerate().filter(|(_, l)| matches!(l, Line::Pack { .. })).map(|(i, _)| i).collect();
        let Some(from) = at.iter().position(|&i| matches!(&self.lines[i], Line::Pack { id: have, .. } if have == id)) else {
            return false;
        };
        let to = from as isize + by;
        if to < 0 || to as usize >= at.len() {
            return false;
        }
        // The pack lines trade places; the comments between them keep theirs.
        self.lines.swap(at[from], at[to as usize]);
        true
    }

    /// Switches a pack on or off in its place; a pack not yet listed is added at the bottom (weakest).
    pub fn set_enabled(&mut self, id: &str, on: bool) {
        for l in &mut self.lines {
            if let Line::Pack { id: have, on: state } = l
                && have == id
            {
                *state = on;
                return;
            }
        }
        self.lines.push(Line::Pack { id: id.to_string(), on });
    }
}

/// Reads `active.txt`: the id of the base set in use (`None` when the file names none). A wrong first line or a
/// line that is no pack id is `LAY.5`.
pub fn parse_active(bytes: &[u8]) -> Result<Option<String>, Refusal> {
    let at = |r: Refusal| r.at_path("active.txt");
    let text = text::decode(bytes, 64 * 1024, Rule::Conv12).map_err(at)?;
    let mut rows = text.lines().map(|l| l.strip_suffix('\r').unwrap_or(l));
    if rows.next() != Some(ACTIVE_HEADER) {
        return Err(at(Refusal::new(Rule::Lay5, format!("the first line is not {ACTIVE_HEADER:?}")).at_line(1)));
    }
    for (i, row) in rows.enumerate() {
        let t = row.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        return match names::is_pack_id(t) {
            true => Ok(Some(t.to_string())),
            false => Err(at(Refusal::new(Rule::Lay5, format!("{t:?} is not a base set id")).at_line(i as u32 + 2))),
        };
    }
    Ok(None)
}

/// `active.txt`'s text for a base set.
pub fn render_active(id: &str) -> String {
    format!("{ACTIVE_HEADER}\n{id}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_example() {
        // layering.md §2 and layout.md §7.
        let text = "# Overbright mods v1\ndocks-remix\nhd-hud\n-old-hud\n";
        let (o, notices) = Order::parse(text.as_bytes()).unwrap();
        assert!(notices.is_empty());
        assert_eq!(o.packs().collect::<Vec<_>>(), [("docks-remix", true), ("hd-hud", true), ("old-hud", false)]);
        assert_eq!(o.render(), text);
        let (on, missing) = o.enabled(|id| id != "hd-hud");
        assert_eq!(on, ["docks-remix"]);
        assert_eq!(missing, [Notice::Missing { id: "hd-hud".into() }]);
        assert_eq!(parse_active(b"# Overbright base v1\nnfsu2-pc12-eu\n").unwrap().as_deref(), Some("nfsu2-pc12-eu"));
        assert_eq!(render_active("nfsu2-pc12-eu"), "# Overbright base v1\nnfsu2-pc12-eu\n");
    }

    #[test]
    fn comments_and_problems_survive() {
        let text = "# Overbright mods v1\r\n# my order\r\n\r\ndocks-remix\r\nDocks!\r\ndocks-remix\r\n";
        let (mut o, notices) = Order::parse(text.as_bytes()).unwrap();
        assert_eq!(
            notices,
            [
                Notice::BadLine { line: 5, text: "Docks!".into() },
                Notice::Duplicate { line: 6, id: "docks-remix".into() }
            ]
        );
        o.set_enabled("docks-remix", false);
        o.set_enabled("new-pack", true);
        assert_eq!(o.render(), "# Overbright mods v1\n# my order\n\n-docks-remix\nDocks!\ndocks-remix\nnew-pack\n");
    }

    #[test]
    fn refusals() {
        assert_eq!(Order::parse(b"docks-remix\n").unwrap_err().rule, Rule::Ord1);
        assert_eq!(Order::parse(b"# Overbright mods v2\n").unwrap_err().rule, Rule::Ord1);
        assert_eq!(Order::parse(b"").unwrap_err().rule, Rule::Ord1);
        assert_eq!(parse_active(b"nfsu2\n").unwrap_err().rule, Rule::Lay5);
        assert_eq!(parse_active(b"# Overbright base v1\nNFSU2\n").unwrap_err().rule, Rule::Lay5);
        assert_eq!(parse_active(b"# Overbright base v1\n# none yet\n").unwrap(), None);
    }
}
