//! The command line read into a command of the registry and its arguments: the words name the command
//! (`props move` is `props.move`), one flag per argument the registry gives it (`--position 1,2,3`), each read
//! by the argument's type into the JSON value the command's request carries. Unknown words and flags are usage
//! errors, never ignored.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::registry::{self, Command, Field, Req};

/// The flags every command takes (beside its own arguments).
pub const COMMON: &[(&str, &str)] = &[
    ("mod", "the mod's folder (the one holding map.obmap)"),
    ("game", "the game folder (default: NFSU2_ROOT, else the one the launcher set)"),
    ("region", "the region the mod is edited on (default L4RA)"),
    ("dry-run", "show what an edit would change; change nothing"),
    ("json", "print one JSON document on stdout"),
    ("wait", "seconds to wait for Studio's answer when Studio has the mod open (default 10)"),
    ("no-inbox", "when another program has the mod open, fail instead of asking it"),
    ("help", "this help"),
];

/// The flags that take no value.
const SWITCHES: &[&str] = &["dry-run", "json", "no-inbox", "help"];

/// A parsed command line.
#[derive(Clone, Debug, Default)]
pub struct Line {
    /// The command (None: none named, help).
    pub command: Option<&'static Command>,
    /// Its arguments as JSON values, by the registry's names.
    pub args: BTreeMap<String, Value>,
    /// The common flags given (`dry-run` → "", `mod` → the folder).
    pub common: BTreeMap<String, String>,
    /// Words after `help`.
    pub help_for: Option<Vec<String>>,
}

impl Line {
    pub fn flag(&self, name: &str) -> Option<&str> {
        self.common.get(name).map(String::as_str)
    }

    pub fn switch(&self, name: &str) -> bool {
        self.common.contains_key(name)
    }
}

/// The command a list of words names (`["race", "point", "add"]`).
pub fn command_of_words(words: &[String]) -> Option<&'static Command> {
    registry::command(&words.join("."))
}

/// Read `argv` (without the program's name).
pub fn parse(argv: &[String]) -> Result<Line, String> {
    let mut line = Line::default();
    let mut words: Vec<String> = Vec::new();
    let mut flags: Vec<(String, Option<String>)> = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        i += 1;
        if let Some(f) = a.strip_prefix("--") {
            if f.is_empty() {
                return Err("`--` is not used: give every value after its flag".into());
            }
            let (name, value) = match f.split_once('=') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (f.to_string(), None),
            };
            flags.push((name, value));
            // A flag's value is the next word, unless the flag takes none (a switch, a yes/no argument).
            let (name, value) = flags.last_mut().unwrap();
            if value.is_none() && !SWITCHES.contains(&name.as_str()) && !is_bool_arg(&words, name) {
                let Some(v) = argv.get(i) else { return Err(format!("--{name} needs a value")) };
                *value = Some(v.clone());
                i += 1;
            }
        } else if a.starts_with('-') && a.len() > 1 && a.parse::<f64>().is_err() {
            return Err(format!("unknown flag `{a}` (flags are `--name value`)"));
        } else {
            words.push(a.clone());
        }
    }
    if words.first().is_some_and(|w| w == "help") {
        line.help_for = Some(words[1..].to_vec());
        return Ok(line);
    }
    if !words.is_empty() {
        line.command = Some(command_of_words(&words).ok_or_else(|| format!("unknown command `{}`", words.join(" ")))?);
    }
    for (name, value) in flags {
        if COMMON.iter().any(|c| c.0 == name) {
            if line.common.insert(name.clone(), value.unwrap_or_default()).is_some() {
                return Err(format!("--{name} given twice"));
            }
            continue;
        }
        let Some(cmd) = line.command else { return Err(format!("unknown flag `--{name}`")) };
        let Some(field) = cmd.args.iter().find(|f| f.key == name) else {
            return Err(format!("unknown flag `--{name}` for `{}`", words.join(" ")));
        };
        let v = value_of(field, value.as_deref())?;
        if line.args.insert(name.clone(), v).is_some() {
            return Err(format!("--{name} given twice"));
        }
    }
    if let Some(cmd) = line.command
        && !line.switch("help")
    {
        for f in cmd.args {
            if matches!(f.req, Req::Yes | Req::YesAs(_)) && !line.args.contains_key(f.key) {
                return Err(format!("`{}` needs --{}", words.join(" "), f.key));
            }
        }
    }
    Ok(line)
}

fn is_bool_arg(words: &[String], name: &str) -> bool {
    command_of_words(words)
        .and_then(|c| c.args.iter().find(|f| f.key == name))
        .is_some_and(|f| registry::type_name(&f.ty) == "bool")
}

fn numbers(text: &str, n: usize, what: &str) -> Result<Vec<f64>, String> {
    let v: Vec<f64> = text
        .split(',')
        .map(|x| x.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("{what}: expected {n} numbers separated by commas, got `{text}`"))?;
    if v.len() != n {
        return Err(format!("{what}: expected {n} numbers, got {}", v.len()));
    }
    Ok(v)
}

fn num(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// One argument's text read by its type into its JSON value.
fn value_of(field: &Field, text: Option<&str>) -> Result<Value, String> {
    let ty = registry::type_name(&field.ty);
    let name = format!("--{}", field.key);
    if ty == "bool" {
        return match text {
            None | Some("true") | Some("yes") => Ok(Value::Bool(true)),
            Some("false") | Some("no") => Ok(Value::Bool(false)),
            Some(t) => Err(format!("{name}: expected true or false, got `{t}`")),
        };
    }
    let t = text.unwrap_or_default();
    let json = || serde_json::from_str::<Value>(t).map_err(|e| format!("{name}: not JSON: {e}"));
    Ok(match ty.as_str() {
        "u8" | "u16" | "u32" | "int" => {
            Value::from(t.parse::<u64>().map_err(|_| format!("{name}: expected a whole number, got `{t}`"))?)
        }
        "f32" => num(t.parse::<f64>().map_err(|_| format!("{name}: expected a number, got `{t}`"))?),
        "vec3" if t.trim_start().starts_with('[') => json()?,
        "vec3" => Value::Array(numbers(t, 3, &name)?.into_iter().map(num).collect()),
        "mat3" if t.trim_start().starts_with('[') => json()?,
        "mat3" => {
            let v = numbers(t, 9, &name)?;
            Value::Array(v.chunks(3).map(|r| Value::Array(r.iter().copied().map(num).collect())).collect())
        }
        "json" => json()?,
        // A prop: its key as JSON, or `added:<id>`.
        _ if field.key == "key" => {
            if t.trim_start().starts_with('{') {
                json()?
            } else if let Some(id) = t.strip_prefix("added:") {
                serde_json::json!({ "added": id })
            } else {
                return Err(format!("{name}: expected `added:<id>` or the key as JSON, got `{t}`"));
            }
        }
        "enum" => {
            let words = enum_words(field);
            if !words.contains(&t) {
                return Err(format!("{name}: expected one of {}, got `{t}`", words.join(", ")));
            }
            Value::String(t.to_string())
        }
        _ => Value::String(t.to_string()),
    })
}

fn enum_words(field: &Field) -> &'static [&'static str] {
    registry::enum_words(&field.ty).unwrap_or(&[])
}
