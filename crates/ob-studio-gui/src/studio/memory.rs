//! The panels' sizes and folds, kept per workspace in the mod's editor folder (`.editor/studio-layout.toml`, never
//! exported with the mod), versioned. View ▸ Reset layout forgets them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

use super::layout::Memory;
use super::workspace::Workspace;

/// The file's name in the mod's editor folder.
pub const FILE: &str = "studio-layout.toml";
/// The file's version; a file of another version is not read (the rules' defaults apply).
pub const VERSION: i64 = 1;

/// Every workspace's kept panels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memories(pub BTreeMap<Workspace, Memory>);

fn key(w: Workspace) -> String {
    w.view().trim_start_matches('@').to_string()
}

impl Memories {
    pub fn get(&self, w: Workspace) -> Memory {
        self.0.get(&w).copied().unwrap_or_default()
    }

    pub fn set(&mut self, w: Workspace, m: Memory) {
        if m == Memory::default() {
            self.0.remove(&w);
        } else {
            self.0.insert(w, m);
        }
    }

    /// The file's text (written by hand, as the editor writes its view file).
    pub fn write(&self) -> String {
        let mut s = format!("version = {VERSION}\n");
        for (w, m) in &self.0 {
            s.push_str(&format!("\n[{}]\n", key(*w)));
            for (name, v) in [("left_w", m.left_w), ("right_w", m.right_w), ("bottom_h", m.bottom_h)] {
                if let Some(v) = v {
                    s.push_str(&format!("{name} = {:?}\n", v.round()));
                }
            }
            for (name, v) in
                [("left_folded", m.left_folded), ("right_folded", m.right_folded), ("bottom_folded", m.bottom_folded)]
            {
                if let Some(v) = v {
                    s.push_str(&format!("{name} = {v}\n"));
                }
            }
        }
        s
    }

    /// Read the file's text; anything it does not know (another version, a wrong value) is dropped.
    pub fn read(text: &str) -> Memories {
        let Ok(t) = text.parse::<toml::Table>() else { return Memories::default() };
        if t.get("version").and_then(toml::Value::as_integer) != Some(VERSION) {
            return Memories::default();
        }
        let mut out = Memories::default();
        for w in Workspace::ALL {
            let Some(s) = t.get(&key(w)).and_then(toml::Value::as_table) else { continue };
            let f = |n: &str| {
                s.get(n)
                    .and_then(|v| v.as_float().or(v.as_integer().map(|i| i as f64)))
                    .map(|v| v as f32)
                    .filter(|v| v.is_finite() && *v > 0.0)
            };
            let b = |n: &str| s.get(n).and_then(toml::Value::as_bool);
            out.set(
                w,
                Memory {
                    left_w: f("left_w"),
                    left_folded: b("left_folded"),
                    right_w: f("right_w"),
                    right_folded: b("right_folded"),
                    bottom_h: f("bottom_h"),
                    bottom_folded: b("bottom_folded"),
                },
            );
        }
        out
    }

    /// The file of the mod in `dir`.
    pub fn path(mod_dir: &Path) -> PathBuf {
        mod_dir.join(crate::tools::map::store::EDITOR_DIR).join(FILE)
    }

    pub fn load(mod_dir: &Path) -> Memories {
        std::fs::read_to_string(Self::path(mod_dir)).map(|t| Memories::read(&t)).unwrap_or_default()
    }

    pub fn save(&self, mod_dir: &Path) -> Result<()> {
        let p = Self::path(mod_dir);
        crate::qol::saves::write_atomic(&p, self.write().as_bytes(), false)
            .with_context(|| format!("writing {}", p.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kept_panels_read_back_per_workspace() {
        let mut m = Memories::default();
        m.set(Workspace::World, Memory { left_w: Some(250.0), bottom_folded: Some(true), ..Default::default() });
        m.set(Workspace::Races, Memory { right_folded: Some(true), right_w: Some(300.0), ..Default::default() });
        let text = m.write();
        assert!(text.contains("version = 1"), "{text}");
        assert_eq!(Memories::read(&text), m);
        // Reset: a default memory is not kept.
        m.set(Workspace::World, Memory::default());
        assert_eq!(m.get(Workspace::World), Memory::default());
    }

    #[test]
    fn another_version_or_a_bad_value_falls_back_to_the_rules() {
        assert_eq!(Memories::read("version = 2\n[world]\nleft_w = 250.0\n"), Memories::default());
        assert_eq!(Memories::read("not toml ["), Memories::default());
        let m = Memories::read("version = 1\n[world]\nleft_w = -4.0\nleft_folded = true\n");
        assert_eq!(m.get(Workspace::World), Memory { left_folded: Some(true), ..Default::default() });
    }
}
