//! The shader composer (docs: audit plan render §0 (a) 1): WGSL files with
//! exactly three kinds of directive line, nothing else rewritten.
//!
//! - `#include "name"` puts the named file's composed lines there (each
//!   file at most once per module; names resolve in the module's file
//!   table, [`super::registry`]).
//! - `#if FLAG` / `#if !FLAG`, `#else`, `#endif` keep or drop the lines
//!   between them; `FLAG` is a [`VariantFlag`] name, anything else is an
//!   error.
//!
//! A directive is a line whose first non-blank character is `#` (WGSL has
//! no such line). Every kept line is copied as it is, with a `\n` after it;
//! directive lines leave nothing. Errors name the file and line.

/// The closed set of names an `#if` may test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VariantFlag {
    /// Dynamic Lamps, Lamp Shadows, Contact Shadows and the rest of the
    /// lamps' module (clone deviations; docs/notes/graphics/lamps-shadows.md).
    Lamps,
    /// Modern Lighting, inside the lamps' module only.
    Modern,
    /// Modern Lighting's ray-query lines (Modern + RT).
    Mlrt,
    /// The draw's texture as a plain 2D texture with its own bind group.
    TextureBindingPlain,
    /// Texture arrays, the layer in the immediates (`TextureArrays`).
    TextureBindingArray,
    /// Bound arrays: texture arrays in one binding array.
    TextureBindingBound,
    /// The environment cube's six faces in one multiview pass.
    Multiview,
    /// The clip position `@invariant` (the depth pre-pass).
    InvariantPosition,
    /// The temporal pass's LOD bias on the main texture (`MIP_BIAS`).
    MipBias,
    /// The device's subgroup operations (wgpu `Features::SUBGROUP`): the
    /// lamps' light-cluster loop one cluster per subgroup at a time.
    Subgroups,
}

impl VariantFlag {
    pub const ALL: [VariantFlag; 10] = [
        VariantFlag::Lamps,
        VariantFlag::Modern,
        VariantFlag::Mlrt,
        VariantFlag::TextureBindingPlain,
        VariantFlag::TextureBindingArray,
        VariantFlag::TextureBindingBound,
        VariantFlag::Multiview,
        VariantFlag::InvariantPosition,
        VariantFlag::MipBias,
        VariantFlag::Subgroups,
    ];

    /// The name an `#if` writes.
    pub fn name(self) -> &'static str {
        match self {
            VariantFlag::Lamps => "LAMPS",
            VariantFlag::Modern => "MODERN",
            VariantFlag::Mlrt => "MLRT",
            VariantFlag::TextureBindingPlain => "TEXTURE_BINDING_PLAIN",
            VariantFlag::TextureBindingArray => "TEXTURE_BINDING_ARRAY",
            VariantFlag::TextureBindingBound => "TEXTURE_BINDING_BOUND",
            VariantFlag::Multiview => "MULTIVIEW",
            VariantFlag::InvariantPosition => "INVARIANT_POSITION",
            VariantFlag::MipBias => "MIP_BIAS",
            VariantFlag::Subgroups => "SUBGROUPS",
        }
    }

    pub fn from_name(name: &str) -> Option<VariantFlag> {
        VariantFlag::ALL.into_iter().find(|f| f.name() == name)
    }

    fn bit(self) -> u32 {
        1 << VariantFlag::ALL.iter().position(|&f| f == self).unwrap()
    }
}

/// A set of [`VariantFlag`]s: the ones a module is composed with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VariantFlags(u32);

impl VariantFlags {
    pub const NONE: VariantFlags = VariantFlags(0);

    pub fn with(self, flag: VariantFlag, on: bool) -> VariantFlags {
        if on { VariantFlags(self.0 | flag.bit()) } else { VariantFlags(self.0 & !flag.bit()) }
    }

    pub fn has(self, flag: VariantFlag) -> bool {
        self.0 & flag.bit() != 0
    }
}

impl FromIterator<VariantFlag> for VariantFlags {
    fn from_iter<I: IntoIterator<Item = VariantFlag>>(iter: I) -> VariantFlags {
        iter.into_iter().fold(VariantFlags::NONE, |s, f| s.with(f, true))
    }
}

/// A composer error at `file`:`line` (1-based).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposeError {
    pub file: String,
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.file, self.line, self.message)
    }
}

impl std::error::Error for ComposeError {}

/// A module's sources: (name, text), the first the root.
pub type Files<'a> = &'a [(&'a str, &'a str)];

/// `files[0]` composed with `flags`.
pub fn compose(files: Files, flags: VariantFlags) -> Result<String, ComposeError> {
    let (root, text) =
        *files.first().ok_or_else(|| ComposeError { file: String::new(), line: 0, message: "no source".into() })?;
    let mut out = String::with_capacity(files.iter().map(|(_, t)| t.len()).sum());
    let mut included = vec![root];
    compose_file(files, root, text, flags, &mut included, &mut out)?;
    Ok(out)
}

/// The directive in `line` (trimmed), if it is one.
enum Directive<'a> {
    Include(&'a str),
    If { flag: VariantFlag, negated: bool },
    Else,
    Endif,
}

fn parse_directive(t: &str) -> Result<Option<Directive<'_>>, String> {
    if !t.starts_with('#') {
        return Ok(None);
    }
    let (word, arg) = t.split_once(' ').map_or((t, ""), |(w, a)| (w, a.trim()));
    match word {
        "#include" => {
            let name = arg
                .strip_prefix('"')
                .and_then(|a| a.strip_suffix('"'))
                .filter(|n| !n.is_empty())
                .ok_or_else(|| format!("`#include` wants a quoted name: {t}"))?;
            Ok(Some(Directive::Include(name)))
        }
        "#if" => {
            let (negated, name) = match arg.strip_prefix('!') {
                Some(n) => (true, n.trim()),
                None => (false, arg),
            };
            let flag = VariantFlag::from_name(name).ok_or_else(|| format!("unknown flag `{name}`"))?;
            Ok(Some(Directive::If { flag, negated }))
        }
        "#else" if arg.is_empty() => Ok(Some(Directive::Else)),
        "#endif" if arg.is_empty() => Ok(Some(Directive::Endif)),
        _ => Err(format!("unknown directive: {t}")),
    }
}

fn compose_file<'a>(
    files: Files<'a>,
    name: &str,
    text: &str,
    flags: VariantFlags,
    included: &mut Vec<&'a str>,
    out: &mut String,
) -> Result<(), ComposeError> {
    let err = |line: usize, message: String| ComposeError { file: name.to_string(), line, message };
    // One entry per open `#if`: (taken now, an `#else` seen, its line).
    let mut stack: Vec<(bool, bool, usize)> = Vec::new();
    let active = |stack: &[(bool, bool, usize)]| stack.iter().all(|s| s.0);
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        match parse_directive(line.trim()).map_err(|m| err(n, m))? {
            None => {
                if active(&stack) {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            Some(Directive::If { flag, negated }) => stack.push((flags.has(flag) != negated, false, n)),
            Some(Directive::Else) => match stack.last_mut() {
                Some(s) if !s.1 => *s = (!s.0, true, s.2),
                Some(_) => return Err(err(n, "a second `#else`".into())),
                None => return Err(err(n, "`#else` without `#if`".into())),
            },
            Some(Directive::Endif) => {
                stack.pop().ok_or_else(|| err(n, "`#endif` without `#if`".into()))?;
            }
            Some(Directive::Include(inc)) => {
                let &(inc_name, inc_text) = files
                    .iter()
                    .find(|(f, _)| *f == inc)
                    .ok_or_else(|| err(n, format!("`{inc}` is not in the module's files")))?;
                if included.contains(&inc_name) {
                    return Err(err(n, format!("`{inc}` included twice")));
                }
                // Each file once per module, whether or not this branch
                // takes it (the include set does not depend on the flags).
                included.push(inc_name);
                if active(&stack) {
                    compose_file(files, inc_name, inc_text, flags, included, out)?;
                } else {
                    let mut sink = String::new();
                    compose_file(files, inc_name, inc_text, VariantFlags::NONE, included, &mut sink)?;
                }
            }
        }
    }
    match stack.last() {
        Some(&(_, _, open)) => Err(err(open, "`#if` without `#endif`".into())),
        None => Ok(()),
    }
}

/// `files[0]` as one file: every `#include` line not naming one of `kept`
/// replaced by the named file's lines (each file at most once), every
/// other line (`#if` lines, the kept includes) copied as it is. With the
/// includes a source had before its split into files kept, it is
/// the file before the split, what source-reading tests look into; for a
/// module without `#if`s or kept includes it is what [`compose`] makes.
pub fn inline(files: Files, kept: &[&str]) -> Result<String, ComposeError> {
    let (root, text) =
        *files.first().ok_or_else(|| ComposeError { file: String::new(), line: 0, message: "no source".into() })?;
    let mut out = String::with_capacity(files.iter().map(|(_, t)| t.len()).sum());
    let mut included = vec![root];
    inline_file(files, kept, root, text, &mut included, &mut out)?;
    Ok(out)
}

fn inline_file<'a>(
    files: Files<'a>,
    kept: &[&str],
    name: &str,
    text: &str,
    included: &mut Vec<&'a str>,
    out: &mut String,
) -> Result<(), ComposeError> {
    let err = |line: usize, message: String| ComposeError { file: name.to_string(), line, message };
    for (i, line) in text.lines().enumerate() {
        if let Some(Directive::Include(inc)) = parse_directive(line.trim()).map_err(|m| err(i + 1, m))?
            && !kept.contains(&inc)
        {
            let &(inc_name, inc_text) = files
                .iter()
                .find(|(f, _)| *f == inc)
                .ok_or_else(|| err(i + 1, format!("`{inc}` is not in the module's files")))?;
            if included.contains(&inc_name) {
                return Err(err(i + 1, format!("`{inc}` included twice")));
            }
            included.push(inc_name);
            inline_file(files, kept, inc_name, inc_text, included, out)?;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    Ok(())
}

/// The named section `name` of `text`: the lines between
/// `// BEGIN {name}` and `// END {name}` (each a whole line), as written.
/// A section is plain WGSL (another module takes it as it is): a directive
/// inside it is an error.
pub fn section<'a>(file: &str, text: &'a str, name: &str) -> Result<&'a str, ComposeError> {
    let begin = format!("// BEGIN {name}\n");
    let end = format!("// END {name}\n");
    let line_of = |at: usize| text[..at].matches('\n').count() + 1;
    let err = |line: usize, message: String| ComposeError { file: file.to_string(), line, message };
    let start = text
        .match_indices(begin.as_str())
        .map(|(i, _)| i)
        .find(|&i| i == 0 || text.as_bytes()[i - 1] == b'\n')
        .ok_or_else(|| err(0, format!("no section `{name}`")))?;
    let body_at = start + begin.len();
    let len = text[body_at..]
        .find(end.as_str())
        .ok_or_else(|| err(line_of(start), format!("section `{name}` without its end")))?;
    let body = &text[body_at..body_at + len];
    if let Some((i, _)) = body.lines().enumerate().find(|(_, l)| l.trim_start().starts_with('#')) {
        return Err(err(line_of(body_at) + i, format!("a directive inside section `{name}`")));
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(f: &[VariantFlag]) -> VariantFlags {
        f.iter().copied().collect()
    }

    #[test]
    fn if_else_nest_and_negate() {
        let src = "a\n#if LAMPS\nb\n  #if !MODERN\n  c\n  #else\n  d\n  #endif\n#else\ne\n#endif\nf\n";
        let files = [("x.wgsl", src)];
        assert_eq!(compose(&files, VariantFlags::NONE).unwrap(), "a\ne\nf\n");
        assert_eq!(compose(&files, flags(&[VariantFlag::Lamps])).unwrap(), "a\nb\n  c\nf\n");
        assert_eq!(compose(&files, flags(&[VariantFlag::Lamps, VariantFlag::Modern])).unwrap(), "a\nb\n  d\nf\n");
        assert_eq!(compose(&files, flags(&[VariantFlag::Modern])).unwrap(), "a\ne\nf\n");
    }

    #[test]
    fn include_once_in_place() {
        let files = [("a", "1\n#include \"b\"\n3\n"), ("b", "2\n#if MULTIVIEW\nm\n#endif\n")];
        assert_eq!(compose(&files, VariantFlags::NONE).unwrap(), "1\n2\n3\n");
        assert_eq!(compose(&files, flags(&[VariantFlag::Multiview])).unwrap(), "1\n2\nm\n3\n");
        let twice = [("a", "#include \"b\"\n#if LAMPS\n#include \"b\"\n#endif\n"), ("b", "2\n")];
        assert_eq!(compose(&twice, VariantFlags::NONE).unwrap_err().to_string(), "a:3: `b` included twice");
        let selfish = [("a", "#include \"a\"\n")];
        assert!(compose(&selfish, VariantFlags::NONE).is_err());
    }

    #[test]
    fn inline_keeps_the_conditionals() {
        let files = [("a", "1\n#include \"b\"\n#if LAMPS\n3\n#endif\n"), ("b", "2\n#if MULTIVIEW\nm\n#endif\n")];
        assert_eq!(inline(&files, &[]).unwrap(), "1\n2\n#if MULTIVIEW\nm\n#endif\n#if LAMPS\n3\n#endif\n");
        assert_eq!(inline(&files, &["b"]).unwrap(), "1\n#include \"b\"\n#if LAMPS\n3\n#endif\n");
        let twice = [("a", "#include \"b\"\n#include \"b\"\n"), ("b", "2\n")];
        assert_eq!(inline(&twice, &[]).unwrap_err().to_string(), "a:2: `b` included twice");
    }

    #[test]
    fn errors_name_file_and_line() {
        let e = |src: &str| compose(&[("w.wgsl", src)], VariantFlags::NONE).unwrap_err().to_string();
        assert_eq!(e("a\n#if NOPE\n#endif\n"), "w.wgsl:2: unknown flag `NOPE`");
        assert_eq!(e("#if LAMPS\na\n"), "w.wgsl:1: `#if` without `#endif`");
        assert_eq!(e("a\n#endif\n"), "w.wgsl:2: `#endif` without `#if`");
        assert_eq!(e("#if LAMPS\n#else\n#else\n#endif\n"), "w.wgsl:3: a second `#else`");
        assert_eq!(e("#define X 1\n"), "w.wgsl:1: unknown directive: #define X 1");
        assert_eq!(e("#include \"missing\"\n"), "w.wgsl:1: `missing` is not in the module's files");
        let nested = [("a", "#include \"b\"\n"), ("b", "x\n#if LAMPS\n")];
        assert_eq!(compose(&nested, VariantFlags::NONE).unwrap_err().to_string(), "b:2: `#if` without `#endif`");
    }

    #[test]
    fn flag_names_round_trip() {
        for f in VariantFlag::ALL {
            assert_eq!(VariantFlag::from_name(f.name()), Some(f));
        }
        assert_eq!(VariantFlag::from_name("lamps"), None);
    }

    #[test]
    fn sections() {
        let t = "x\n// BEGIN RTVIS A\na\nb\n// END RTVIS A\n  // BEGIN RTVIS B\n// BEGIN RTVIS C\n#if LAMPS\n// END RTVIS C\n";
        assert_eq!(section("w", t, "RTVIS A").unwrap(), "a\nb\n");
        assert!(section("w", t, "RTVIS B").is_err(), "an indented marker is not a section start");
        assert_eq!(section("w", t, "RTVIS C").unwrap_err().to_string(), "w:8: a directive inside section `RTVIS C`");
        assert!(section("w", t, "RTVIS D").is_err());
    }
}
