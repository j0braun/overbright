//! Addresses (docs/modding/studio.md, "Addresses"): every item and every screen of Studio has one, to copy, paste
//! into Find and jump to.
//!
//! * An item: `ob://<pack id>/<kind>/<key>[#<field>][?guard=<value>]`, as the asset format names items
//!   (`ob://harbour-edits/placement/L4RA/1407/12#position`).
//! * A screen (a view): `ob://<pack id>/@<view>[?<name>=<value>…]` (`ob://harbour-edits/@changes?step=14`); Studio's
//!   own screens use the pack id `studio` (`ob://studio/@home`). A view segment starts with `@`, which no kind
//!   name can.
//!
//! The address bar shows names ("Harbour edits › World › Bayview › Dome roof"); Ctrl+L shows the address itself.
//! [`Places`] keeps the places visited in one window for back and forward.

use std::fmt;

/// The scheme every address starts with.
pub const SCHEME: &str = "ob://";
/// The pack id of Studio's own screens.
pub const STUDIO: &str = "studio";
/// How many places back and forward keep.
pub const PLACES: usize = 50;

/// What an address points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A screen: its view name without the `@` and its query in order.
    View { view: String, query: Vec<(String, String)> },
    /// An item: its kind, its key's segments, a field and a guard.
    Item { kind: String, key: Vec<String>, field: Option<String>, guard: Option<String> },
}

/// One address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    /// The pack id (a mod's id, or [`STUDIO`]).
    pub pack: String,
    pub target: Target,
}

/// Why a text is not an address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotAddress(pub String);

impl fmt::Display for NotAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn is_id(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-' || *c == b'_')
}

/// A key segment or a value: letters, digits and `-_.:`, nothing that an address uses as a separator.
fn is_word(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b':'))
}

impl Address {
    /// A screen's address.
    pub fn view(pack: &str, view: &str) -> Address {
        Address { pack: pack.into(), target: Target::View { view: view.trim_start_matches('@').into(), query: vec![] } }
    }

    /// An item's address.
    pub fn item(pack: &str, kind: &str, key: &[&str]) -> Address {
        Address {
            pack: pack.into(),
            target: Target::Item {
                kind: kind.into(),
                key: key.iter().map(|k| k.to_string()).collect(),
                field: None,
                guard: None,
            },
        }
    }

    /// Read an address (spaces around it are dropped, as a pasted one may carry them).
    pub fn parse(text: &str) -> Result<Address, NotAddress> {
        let bad = |why: &str| NotAddress(format!("{:?} is not an address: {why}", text.trim()));
        let rest = text.trim().strip_prefix(SCHEME).ok_or_else(|| bad("it does not start with ob://"))?;
        let (path, query) = match rest.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (rest, None),
        };
        let (path, field) = match path.split_once('#') {
            Some((p, f)) => (p, Some(f)),
            None => (path, None),
        };
        let mut parts = path.split('/');
        let pack = parts.next().unwrap_or_default();
        if !is_id(pack) {
            return Err(bad("no mod id after ob://"));
        }
        let segs: Vec<&str> = parts.collect();
        let Some(first) = segs.first().filter(|s| !s.is_empty()) else {
            return Err(bad("nothing after the mod id"));
        };
        let pairs = |q: Option<&str>| -> Result<Vec<(String, String)>, NotAddress> {
            let Some(q) = q else { return Ok(vec![]) };
            q.split('&')
                .map(|kv| match kv.split_once('=') {
                    Some((k, v)) if is_id(k) && is_word(v) => Ok((k.to_string(), v.to_string())),
                    _ => Err(bad("a query part is not name=value")),
                })
                .collect()
        };
        if let Some(view) = first.strip_prefix('@') {
            if !is_id(view) || segs.len() != 1 || field.is_some() {
                return Err(bad("a screen is one @name"));
            }
            return Ok(Address { pack: pack.into(), target: Target::View { view: view.into(), query: pairs(query)? } });
        }
        if !is_id(first) {
            return Err(bad("the kind is not a name"));
        }
        let key: Vec<String> = segs[1..].iter().map(|s| s.to_string()).collect();
        if key.is_empty() || !key.iter().all(|k| is_word(k)) {
            return Err(bad("the item's key is missing or has a character an address cannot carry"));
        }
        if field.is_some_and(|f| !is_id(f)) {
            return Err(bad("the field is not a name"));
        }
        let q = pairs(query)?;
        let guard = match q.as_slice() {
            [] => None,
            [(k, v)] if k == "guard" => Some(v.clone()),
            _ => return Err(bad("an item takes only ?guard=")),
        };
        Ok(Address {
            pack: pack.into(),
            target: Target::Item { kind: first.to_string(), key, field: field.map(str::to_string), guard },
        })
    }

    /// Whether it names a screen.
    pub fn is_view(&self) -> bool {
        matches!(self.target, Target::View { .. })
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{SCHEME}{}/", self.pack)?;
        match &self.target {
            Target::View { view, query } => {
                write!(f, "@{view}")?;
                for (i, (k, v)) in query.iter().enumerate() {
                    write!(f, "{}{k}={v}", if i == 0 { '?' } else { '&' })?;
                }
            }
            Target::Item { kind, key, field, guard } => {
                write!(f, "{kind}/{}", key.join("/"))?;
                if let Some(x) = field {
                    write!(f, "#{x}")?;
                }
                if let Some(g) = guard {
                    write!(f, "?guard={g}")?;
                }
            }
        }
        Ok(())
    }
}

/// The breadcrumb's separator between names.
pub const CRUMB: &str = " › ";

/// The places one window went to, for back and forward (Alt+← / Alt+→, the mouse's side buttons, the pad's View
/// button). Selecting an item is a place; moving the camera is not.
#[derive(Clone, Debug, Default)]
pub struct Places {
    back: Vec<Address>,
    now: Option<Address>,
    forward: Vec<Address>,
}

impl Places {
    /// Go to `a`: the place before goes on the back list, the forward list is dropped. Going to the place one is
    /// at does nothing.
    pub fn go(&mut self, a: Address) {
        if self.now.as_ref() == Some(&a) {
            return;
        }
        if let Some(old) = self.now.replace(a) {
            self.back.push(old);
            if self.back.len() > PLACES {
                self.back.remove(0);
            }
        }
        self.forward.clear();
    }

    pub fn now(&self) -> Option<&Address> {
        self.now.as_ref()
    }

    pub fn can_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Step back; the place to show.
    pub fn back(&mut self) -> Option<&Address> {
        let prev = self.back.pop()?;
        if let Some(n) = self.now.replace(prev) {
            self.forward.push(n);
        }
        self.now.as_ref()
    }

    /// Step forward; the place to show.
    pub fn forward(&mut self) -> Option<&Address> {
        let next = self.forward.pop()?;
        if let Some(n) = self.now.replace(next) {
            self.back.push(n);
        }
        self.now.as_ref()
    }

    /// The places back, newest first (the list a right-click on ← shows).
    pub fn back_list(&self) -> impl Iterator<Item = &Address> {
        self.back.iter().rev()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: [&str; 7] = [
        "ob://studio/@home",
        "ob://harbour-edits/@world",
        "ob://harbour-edits/@changes?step=14",
        "ob://harbour-edits/@changes?step=14&mode=compare",
        "ob://harbour-edits/placement/L4RA/1407/12",
        "ob://harbour-edits/placement/L4RA/1407/12#position",
        "ob://harbour-edits/placement/L4RA/1407/12#position?guard=3f2a",
    ];

    #[test]
    fn addresses_round_trip() {
        for text in GOOD {
            let a = Address::parse(text).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(a.to_string(), text);
            assert_eq!(Address::parse(&a.to_string()), Ok(a));
        }
        let built = Address::item("harbour-edits", "race", &["harbour-edits:sprint"]);
        assert_eq!(Address::parse(&built.to_string()), Ok(built));
        assert_eq!(Address::view("x", "@home").to_string(), "ob://x/@home");
    }

    #[test]
    fn a_pasted_address_may_carry_spaces() {
        assert_eq!(Address::parse("  ob://studio/@home\n").map(|a| a.to_string()).as_deref(), Ok("ob://studio/@home"));
    }

    #[test]
    fn views_and_items_do_not_mix() {
        assert!(Address::parse("ob://m/@world").unwrap().is_view());
        assert!(!Address::parse("ob://m/race/m:sprint").unwrap().is_view());
        for bad in [
            "",
            "http://x/@home",
            "ob://",
            "ob://Big/@home",
            "ob://m/",
            "ob://m/@",
            "ob://m/@world/extra",
            "ob://m/@world#field",
            "ob://m/race",
            "ob://m/race/a b",
            "ob://m/race/x?step=1",
            "ob://m/@changes?step",
            "ob://m/race/x#Bad",
        ] {
            assert!(Address::parse(bad).is_err(), "{bad:?} read as an address");
        }
    }

    #[test]
    fn back_and_forward_walk_the_places() {
        let a = |v: &str| Address::view("m", v);
        let mut p = Places::default();
        assert!(!p.can_back());
        p.go(a("home"));
        p.go(a("world"));
        p.go(a("world"));
        p.go(a("races"));
        assert_eq!(p.back(), Some(&a("world")));
        assert_eq!(p.back(), Some(&a("home")));
        assert_eq!(p.back(), None);
        assert_eq!(p.forward(), Some(&a("world")));
        p.go(a("mod"));
        assert!(!p.can_forward(), "a new place drops the forward list");
        for i in 0..80 {
            p.go(Address::view("m", &format!("v{i}")));
        }
        assert_eq!(p.back_list().count(), PLACES);
    }
}
