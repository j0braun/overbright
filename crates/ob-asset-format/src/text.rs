//! Text files (conventions.md §3.1–3.3): UTF-8 without a BOM or NUL, LF or CRLF line ends, TOML 1.0, and the
//! `schema` key with its version checks (versioning.md §2).

use crate::{Refusal, Rule};

/// The default limit of a TOML file (conventions.md §10).
pub const MAX_TOML_BYTES: usize = 16 << 20;

/// The text of a file, checked: UTF-8, no byte-order mark, no NUL (`CONV.1`). The limit is checked first, before
/// anything is decoded (`CONV.12`, or the page's own rule given as `limit_rule`).
pub fn decode(bytes: &[u8], limit: usize, limit_rule: Rule) -> Result<&str, Refusal> {
    if bytes.len() > limit {
        return Err(Refusal::new(limit_rule, format!("{} bytes; the limit is {limit}", bytes.len())));
    }
    if bytes.starts_with(b"\xEF\xBB\xBF") {
        return Err(Refusal::new(Rule::Conv1, "the file starts with a byte-order mark"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|e| Refusal::new(Rule::Conv1, "not UTF-8").at_offset(e.valid_up_to() as u64))?;
    if let Some(at) = text.find('\0') {
        return Err(Refusal::new(Rule::Conv1, "the text holds a NUL character").at_offset(at as u64));
    }
    Ok(text)
}

/// The 1-based line of byte `offset` in `text`.
pub fn line_of(text: &str, offset: usize) -> u32 {
    text.as_bytes()[..offset.min(text.len())].iter().filter(|&&b| b == b'\n').count() as u32 + 1
}

/// A TOML document (`CONV.2` with the parser's line and column when it is not TOML). CRLF line ends are read
/// like LF.
pub fn parse_toml(text: &str) -> Result<toml::Table, Refusal> {
    text.parse::<toml::Table>().map_err(|e| {
        let mut r = Refusal::new(Rule::Conv2, e.message().trim().to_string());
        if let Some(span) = e.span() {
            let line = line_of(text, span.start);
            let col = span.start - text[..span.start].rfind('\n').map_or(0, |i| i + 1) + 1;
            r.message = format!("{} (column {col})", r.message);
            r = r.at_line(line);
        }
        r
    })
}

/// A schema id, `ob.<kind>/<major>.<minor>` (conventions.md §3.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaId {
    pub kind: String,
    pub major: u32,
    pub minor: u32,
}

impl SchemaId {
    /// Reads a schema id (`CONV.3` when malformed).
    pub fn parse(s: &str) -> Result<SchemaId, Refusal> {
        let bad = || Refusal::new(Rule::Conv3, format!("{s:?} is not a schema id (ob.<kind>/<major>.<minor>)"));
        let rest = s.strip_prefix("ob.").ok_or_else(bad)?;
        let (kind, version) = rest.split_once('/').ok_or_else(bad)?;
        let (major, minor) = version.split_once('.').ok_or_else(bad)?;
        let kind_ok = !kind.is_empty()
            && kind
                .split('-')
                .all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()));
        let num =
            |n: &str| (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then(|| n.parse::<u32>().ok()).flatten();
        match (kind_ok, num(major), num(minor)) {
            (true, Some(major), Some(minor)) => Ok(SchemaId { kind: kind.to_string(), major, minor }),
            _ => Err(bad()),
        }
    }
}

impl std::fmt::Display for SchemaId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ob.{}/{}.{}", self.kind, self.major, self.minor)
    }
}

/// A kind version this reader reads: the kind, its major, and the highest minor it knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Supported {
    pub kind: &'static str,
    pub major: u32,
    pub minor: u32,
}

/// The `schema` key of a TOML data file, checked against what the reader reads (conventions.md §3.3): missing or
/// of an unknown kind is `CONV.3`, an unknown major `VER.2`, a newer minor `VER.1`.
pub fn check_schema(doc: &toml::Table, supported: &[Supported]) -> Result<SchemaId, Refusal> {
    let id = match doc.get("schema") {
        Some(toml::Value::String(s)) => SchemaId::parse(s)?,
        Some(_) => return Err(Refusal::new(Rule::Conv3, "`schema` is not a string").in_field("schema")),
        None => return Err(Refusal::new(Rule::Conv3, "no `schema` key").in_field("schema")),
    };
    let mine: Vec<&Supported> = supported.iter().filter(|s| s.kind == id.kind).collect();
    if mine.is_empty() {
        return Err(Refusal::new(Rule::Conv3, format!("unknown kind ob.{}", id.kind)).in_field("schema"));
    }
    match mine.iter().find(|s| s.major == id.major) {
        None => Err(Refusal::new(
            Rule::Ver2,
            format!(
                "{id}: this reader reads major {} only",
                mine.iter().map(|s| s.major.to_string()).collect::<Vec<_>>().join(", ")
            ),
        )
        .in_field("schema")),
        Some(s) if id.minor > s.minor => Err(Refusal::new(
            Rule::Ver1,
            format!("{id} is newer than this reader knows (up to ob.{}/{}.{})", s.kind, s.major, s.minor),
        )
        .in_field("schema")),
        Some(_) => Ok(id),
    }
}

// ---- writing ----------------------------------------------------------------------------------------------------

/// A TOML basic string with escapes (never a multi-line string: conventions.md §3.2 keeps every character).
pub fn toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A TOML key: bare when it can be, quoted otherwise.
pub fn toml_key(k: &str) -> String {
    if !k.is_empty() && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        k.to_string()
    } else {
        toml_string(k)
    }
}

/// A value in inline TOML form (tables as inline tables). Floats are written as binary64 (§3.4).
pub fn toml_inline(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => toml_string(s),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => crate::float::write_f64(*f),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Datetime(d) => d.to_string(),
        toml::Value::Array(a) => format!("[{}]", a.iter().map(toml_inline).collect::<Vec<_>>().join(", ")),
        toml::Value::Table(t) => {
            if t.is_empty() {
                "{}".into()
            } else {
                let items: Vec<String> =
                    t.iter().map(|(k, v)| format!("{} = {}", toml_key(k), toml_inline(v))).collect();
                format!("{{ {} }}", items.join(", "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding() {
        assert_eq!(decode(b"a = 1\n", 100, Rule::Conv12).unwrap(), "a = 1\n");
        assert_eq!(decode(b"\xEF\xBB\xBFa = 1", 100, Rule::Conv12).unwrap_err().rule, Rule::Conv1);
        assert_eq!(decode(b"a = \"\xFF\"", 100, Rule::Conv12).unwrap_err().rule, Rule::Conv1);
        assert_eq!(decode(b"a = 1\0", 100, Rule::Conv12).unwrap_err().rule, Rule::Conv1);
        assert_eq!(decode(b"a = 1", 3, Rule::Man8).unwrap_err().rule, Rule::Man8);
    }

    #[test]
    fn toml_syntax() {
        assert_eq!(parse_toml("a = 1\r\nb = 2\r\n").unwrap()["b"].as_integer(), Some(2));
        let e = parse_toml("a = 1\nb = = 2\n").unwrap_err();
        assert_eq!((e.rule, e.line), (Rule::Conv2, Some(2)));
        assert_eq!(parse_toml("a = 1\na = 2\n").unwrap_err().rule, Rule::Conv2);
        // Hex, octal and binary integers read alike (conventions.md §3.2).
        let t = parse_toml("a = 0x0200\nb = 0o1000\nc = 0b1000000000\nd = 512").unwrap();
        assert!(["a", "b", "c", "d"].iter().all(|k| t[*k].as_integer() == Some(512)));
    }

    #[test]
    fn schema_versions() {
        let sup = [
            Supported { kind: "pack-migrations", major: 1, minor: 0 },
            Supported { kind: "handling", major: 1, minor: 3 },
        ];
        let doc = |s: &str| parse_toml(&format!("schema = \"{s}\"")).unwrap();
        assert_eq!(check_schema(&doc("ob.handling/1.3"), &sup).unwrap().to_string(), "ob.handling/1.3");
        assert_eq!(check_schema(&doc("ob.handling/1.4"), &sup).unwrap_err().rule, Rule::Ver1);
        assert_eq!(check_schema(&doc("ob.handling/2.0"), &sup).unwrap_err().rule, Rule::Ver2);
        assert_eq!(check_schema(&doc("ob.cars/1.0"), &sup).unwrap_err().rule, Rule::Conv3);
        assert_eq!(check_schema(&doc("handling/1.0"), &sup).unwrap_err().rule, Rule::Conv3);
        assert_eq!(check_schema(&parse_toml("a = 1").unwrap(), &sup).unwrap_err().rule, Rule::Conv3);
        for ok in ["ob.texture-set/1.0", "ob.pack-migrations/1.0", "ob.anim-banks/12.34"] {
            assert_eq!(SchemaId::parse(ok).unwrap().to_string(), ok);
        }
        for bad in ["ob.Tex/1.0", "ob.tex-/1.0", "ob.tex/1", "ob.tex/1.x", "ob./1.0"] {
            assert!(SchemaId::parse(bad).is_err(), "{bad}");
        }
    }
}
