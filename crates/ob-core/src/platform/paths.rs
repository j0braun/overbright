//! Path forms (docs/playing/platform-dirs.md).

use std::path::{Path, PathBuf};

/// `p` made absolute for keeping (`--set-root`). Unix: `canonicalize` as
/// before (symlinks resolved; `p` itself when that fails). Windows: never a
/// verbatim `\\?\` path — `canonicalize` gives one, and a verbatim path
/// takes no `/`, so every later `root.join("GLOBAL/…")` would fail — the
/// canonical path with the prefix stripped when it means the same without
/// it ([`strip_verbatim`]), else `std::path::absolute`.
pub fn absolute(p: &Path) -> PathBuf {
    #[cfg(not(windows))]
    {
        p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
    }
    #[cfg(windows)]
    {
        let plain = || std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
        match p.canonicalize() {
            Ok(c) => match c.to_str().map(strip_verbatim) {
                Some(Some(s)) => PathBuf::from(s),
                _ => plain(),
            },
            Err(_) => plain(),
        }
    }
}

/// The Windows path `s` without its verbatim prefix, when the plain form
/// names the same file: `\\?\C:\a\b` → `C:\a\b`, `\\?\UNC\srv\share\a` →
/// `\\srv\share\a`; a path without the prefix is returned as it is. None
/// when only the verbatim form works: other verbatim forms (`\\?\Volume{…}`),
/// 260 characters or more (MAX_PATH), or a component Win32 would rewrite
/// (ending in `.` or a space, `.`/`..`, or a reserved device name).
pub fn strip_verbatim(s: &str) -> Option<String> {
    let plain = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        let b = rest.as_bytes();
        if b.len() < 3 || !b[0].is_ascii_alphabetic() || b[1] != b':' || b[2] != b'\\' {
            return None;
        }
        rest.to_string()
    } else {
        return Some(s.to_string());
    };
    if plain.encode_utf16().count() >= 260 {
        return None;
    }
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
        "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let start = if plain.starts_with(r"\\") { 2 } else { 3 };
    for c in plain[start..].split('\\').filter(|c| !c.is_empty()) {
        let stem = c.split('.').next().unwrap_or(c).trim_end();
        if c.ends_with('.') || c.ends_with(' ') || RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
            return None;
        }
    }
    Some(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_prefix_stripped_only_when_safe() {
        let ok = |a: &str, b: &str| assert_eq!(strip_verbatim(a).as_deref(), Some(b), "{a}");
        ok(
            r"\\?\C:\Program Files (x86)\EA GAMES\Need for Speed Underground 2",
            r"C:\Program Files (x86)\EA GAMES\Need for Speed Underground 2",
        );
        ok(r"\\?\d:\Games\NFSU2", r"d:\Games\NFSU2");
        ok(r"\\?\UNC\nas\games\NFSU2", r"\\nas\games\NFSU2");
        ok(r"C:\Games\NFSU2", r"C:\Games\NFSU2");
        ok(r"\\?\C:\", r"C:\");
        for bad in [
            r"\\?\Volume{0b1c}\Games",
            r"\\?\C:\Games\trailing.",
            r"\\?\C:\Games\trailing ",
            r"\\?\C:\Games\CON",
            r"\\?\C:\Games\nul.txt",
            r"\\?\C:\Games\..\x",
            r"\\?\C:",
        ] {
            assert_eq!(strip_verbatim(bad), None, "{bad}");
        }
        let long = format!(r"\\?\C:\{}", "a".repeat(300));
        assert_eq!(strip_verbatim(&long), None);
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_absolute_is_canonicalize() {
        let here = std::env::current_dir().unwrap();
        assert_eq!(absolute(Path::new(".")), here.canonicalize().unwrap());
        let missing = Path::new("no/such/folder/for/the/test");
        assert_eq!(absolute(missing), missing);
    }
}
