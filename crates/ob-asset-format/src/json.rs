//! A small JSON value with ordered objects: what the schema generator writes, the standard TOML-to-JSON mapping
//! (conventions.md §12) and the canonical form of RFC 8785 (JSON Canonicalization Scheme), which the content
//! digest hashes (manifest.md §4).

/// A JSON value. Objects keep their keys in insertion order.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn str(s: impl Into<String>) -> Json {
        Json::Str(s.into())
    }

    /// An object from `(key, value)` pairs, in this order.
    pub fn obj<K: Into<String>>(pairs: impl IntoIterator<Item = (K, Json)>) -> Json {
        Json::Object(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    /// An array of strings.
    pub fn strs<S: AsRef<str>>(items: impl IntoIterator<Item = S>) -> Json {
        Json::Array(items.into_iter().map(|s| Json::str(s.as_ref())).collect())
    }

    /// Appends a key to an object (no-op on other values).
    pub fn push(&mut self, key: &str, value: Json) {
        if let Json::Object(v) = self {
            v.push((key.to_string(), value));
        }
    }

    /// The standard TOML-to-JSON mapping: tables → objects (keys in the table's order), arrays → arrays, date-times →
    /// strings in their TOML form.
    pub fn from_toml(v: &toml::Value) -> Json {
        match v {
            toml::Value::String(s) => Json::Str(s.clone()),
            toml::Value::Integer(i) => Json::Int(*i),
            toml::Value::Float(f) => Json::Float(*f),
            toml::Value::Boolean(b) => Json::Bool(*b),
            toml::Value::Datetime(d) => Json::Str(d.to_string()),
            toml::Value::Array(a) => Json::Array(a.iter().map(Json::from_toml).collect()),
            toml::Value::Table(t) => Json::from_table(t),
        }
    }

    pub fn from_table(t: &toml::Table) -> Json {
        Json::Object(t.iter().map(|(k, v)| (k.clone(), Json::from_toml(v))).collect())
    }

    /// The value as indented text (two spaces, one key or element per line; the form the schemas use), with a
    /// final line feed.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.write_pretty(&mut out, 0);
        out.push('\n');
        out
    }

    fn write_pretty(&self, out: &mut String, depth: usize) {
        let pad = |out: &mut String, d: usize| out.extend(std::iter::repeat_n(' ', 2 * d));
        match self {
            Json::Array(a) if !a.is_empty() => {
                out.push_str("[\n");
                for (i, v) in a.iter().enumerate() {
                    pad(out, depth + 1);
                    v.write_pretty(out, depth + 1);
                    out.push_str(if i + 1 < a.len() { ",\n" } else { "\n" });
                }
                pad(out, depth);
                out.push(']');
            }
            Json::Object(o) if !o.is_empty() => {
                out.push_str("{\n");
                for (i, (k, v)) in o.iter().enumerate() {
                    pad(out, depth + 1);
                    write_string(out, k);
                    out.push_str(": ");
                    v.write_pretty(out, depth + 1);
                    out.push_str(if i + 1 < o.len() { ",\n" } else { "\n" });
                }
                pad(out, depth);
                out.push('}');
            }
            other => other.write_compact(out, false),
        }
    }

    /// The canonical form of RFC 8785: no white space, object keys sorted by their UTF-16 code units, numbers
    /// in ECMAScript's shortest form, strings with the minimal escapes. Non-finite numbers have no JSON form
    /// and are written as `null`.
    pub fn canonical(&self) -> String {
        let mut out = String::new();
        self.write_compact(&mut out, true);
        out
    }

    fn write_compact(&self, out: &mut String, canonical: bool) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Int(i) if canonical => out.push_str(&es_number(*i as f64)),
            Json::Int(i) => out.push_str(&i.to_string()),
            Json::Float(f) if canonical => out.push_str(&es_number(*f)),
            Json::Float(f) if f.is_finite() => out.push_str(&crate::float::write_f64(*f)),
            Json::Float(_) => out.push_str("null"),
            Json::Str(s) => write_string(out, s),
            Json::Array(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write_compact(out, canonical);
                }
                out.push(']');
            }
            Json::Object(o) => {
                let mut items: Vec<&(String, Json)> = o.iter().collect();
                if canonical {
                    items.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
                }
                out.push('{');
                for (i, (k, v)) in items.into_iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(out, k);
                    out.push(':');
                    v.write_compact(out, canonical);
                }
                out.push('}');
            }
        }
    }
}

/// A JSON string with the minimal escapes (`"`, `\`, and control characters; `\u00XX` in lower-case hex where no
/// short escape exists), as ECMAScript's `JSON.stringify` and RFC 8785 write it.
fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A number as ECMAScript's `Number.prototype.toString` writes it (RFC 8785 §3.2.2.3).
fn es_number(x: f64) -> String {
    if !x.is_finite() {
        return "null".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    let sci = format!("{:e}", x.abs());
    let (mant, exp) = sci.split_once('e').expect("`{:e}` has an exponent");
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp.parse::<i32>().expect("exponent") + 1;
    let mut out = String::from(if x < 0.0 { "-" } else { "" });
    if k <= n && n <= 21 {
        out.push_str(&digits);
        out.extend(std::iter::repeat_n('0', (n - k) as usize));
    } else if 0 < n && n <= 21 {
        out.push_str(&digits[..n as usize]);
        out.push('.');
        out.push_str(&digits[n as usize..]);
    } else if -6 < n && n <= 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-n) as usize));
        out.push_str(&digits);
    } else {
        out.push_str(&digits[..1]);
        if k > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if n - 1 < 0 { '-' } else { '+' });
        out.push_str(&(n - 1).unsigned_abs().to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecmascript_numbers() {
        // RFC 8785 Appendix B's examples.
        let cases = [
            (0x0000_0000_0000_0000u64, "0"),
            (0x8000_0000_0000_0000, "0"),
            (0x0000_0000_0000_0001, "5e-324"),
            (0x7FEF_FFFF_FFFF_FFFF, "1.7976931348623157e+308"),
            (0x4340_0000_0000_0000, "9007199254740992"),
            (0x444B_1AE4_D6E2_EF50, "1e+21"),
            (0x444B_1AE4_D6E2_EF4F, "999999999999999900000"),
            (0x3EB0_C6F7_A0B5_ED8D, "0.000001"),
            (0x3EB0_C6F7_A0B5_ED8C, "9.999999999999997e-7"),
            (0x41B3_DE43_5555_5555, "333333333.3333333"),
            (0xC1B3_DE43_5555_5555, "-333333333.3333333"),
        ];
        for (bits, text) in cases {
            assert_eq!(es_number(f64::from_bits(bits)), text, "{bits:#X}");
        }
    }

    #[test]
    fn canonical_form() {
        // RFC 8785 §3.2.3's sorting example (keys compared as UTF-16 code units).
        let j = Json::obj([
            ("\u{20ac}", Json::str("Euro Sign")),
            ("\r", Json::str("Carriage Return")),
            ("\u{fb33}", Json::str("Hebrew Letter Dalet With Dagesh")),
            ("1", Json::str("One")),
            ("\u{1f600}", Json::str("Emoji: Grinning Face")),
            ("\u{80}", Json::str("Control")),
            ("\u{f6}", Json::str("Latin Small Letter O With Diaeresis")),
        ]);
        assert_eq!(
            j.canonical(),
            "{\"\\r\":\"Carriage Return\",\"1\":\"One\",\"\u{80}\":\"Control\",\"\u{f6}\":\"Latin Small Letter O With \
             Diaeresis\",\"\u{20ac}\":\"Euro Sign\",\"\u{1f600}\":\"Emoji: Grinning Face\",\"\u{fb33}\":\"Hebrew Letter \
             Dalet With Dagesh\"}"
        );
        let t: toml::Table = toml::from_str("b = [1, 2.5, true]\na = { y = \"\\u001f\", x = -0.0 }").unwrap();
        assert_eq!(Json::from_table(&t).canonical(), "{\"a\":{\"x\":0,\"y\":\"\\u001f\"},\"b\":[1,2.5,true]}");
    }

    #[test]
    fn pretty_form() {
        let j = Json::obj([("a", Json::Int(1)), ("b", Json::Array(vec![])), ("c", Json::strs(["x\\y"]))]);
        assert_eq!(j.pretty(), "{\n  \"a\": 1,\n  \"b\": [],\n  \"c\": [\n    \"x\\\\y\"\n  ]\n}\n");
    }
}
