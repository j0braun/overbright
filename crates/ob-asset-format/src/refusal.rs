//! A refusal (conventions.md §11): what was refused, where, and by which rule.

use crate::Rule;

/// Why a file, a value or a pack was refused. Names the rule and, when known, the pack, the logical path, the
/// line (text) or byte offset (binary) and the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub rule: Rule,
    pub pack: Option<String>,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub offset: Option<u64>,
    pub field: Option<String>,
    pub message: String,
}

impl Refusal {
    pub fn new(rule: Rule, message: impl Into<String>) -> Refusal {
        Refusal { rule, pack: None, path: None, line: None, offset: None, field: None, message: message.into() }
    }

    /// The pack the refused file belongs to (kept when already set).
    pub fn in_pack(mut self, pack: &str) -> Refusal {
        self.pack.get_or_insert_with(|| pack.to_string());
        self
    }

    /// The refused file's logical path (kept when already set).
    pub fn at_path(mut self, path: &str) -> Refusal {
        self.path.get_or_insert_with(|| path.to_string());
        self
    }

    pub fn at_line(mut self, line: u32) -> Refusal {
        self.line = Some(line);
        self
    }

    pub fn at_offset(mut self, offset: u64) -> Refusal {
        self.offset = Some(offset);
        self
    }

    /// The field, as a dotted path from the file's root; an outer name is put in front of an inner one.
    pub fn in_field(mut self, field: &str) -> Refusal {
        self.field = Some(match self.field.take() {
            Some(inner) if inner.starts_with('[') => format!("{field}{inner}"),
            Some(inner) => format!("{field}.{inner}"),
            None => field.to_string(),
        });
        self
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(p) = &self.pack {
            write!(f, "{p}: ")?;
        }
        if let Some(p) = &self.path {
            write!(f, "{p}")?;
            if let Some(l) = self.line {
                write!(f, ":{l}")?;
            }
            if let Some(o) = self.offset {
                write!(f, " @0x{o:X}")?;
            }
            write!(f, ": ")?;
        }
        if let Some(field) = &self.field {
            write!(f, "`{field}`: ")?;
        }
        write!(f, "{} ({})", self.message, self.rule.id())
    }
}

impl std::error::Error for Refusal {}
