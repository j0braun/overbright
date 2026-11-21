//! What Studio knows about the player's mods without opening one for writing (Home's rows, the status bar): read
//! through the mod's history (`ob_studio::history`), as its writer has it, unsaved steps included, without taking
//! its lock.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context as _, Result};

use crate::tools::map::history::{self, History};
use crate::tools::map::{MapMod, PropEdit, store};

/// How the mod's work stands on the disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Saved {
    /// Everything is in the mod's files.
    Saved,
    /// Unsaved steps found on opening: in the journal, not in the files, until restored or discarded.
    Behind,
    /// The last steps are in the journal (kept through a crash) and go to the files at the next quiet point,
    /// in a moment.
    Saving,
    /// The mod's files do not read.
    Error(String),
}

/// One mod as Home lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModSummary {
    /// Its folder name (its id in addresses).
    pub folder: String,
    pub dir: PathBuf,
    /// Its name as the mod gives it (the folder name when it has none).
    pub name: String,
    pub moved: usize,
    pub added: usize,
    pub hidden: usize,
    pub races: usize,
    /// It is on in the player's game (`enabled.txt`).
    pub in_game: bool,
    pub saved: Saved,
    /// Another program has it open for writing (that program's name): Studio shows it read-only.
    pub read_only_by: Option<String>,
    /// The last kept step's words ("Move Dome roof").
    pub last_step: Option<String>,
}

impl ModSummary {
    /// The number of changes (props and races).
    pub fn changes(&self) -> usize {
        self.moved + self.added + self.hidden + self.races
    }

    /// The row under its name: "2 races · 9 prop edits · on in your game".
    pub fn line(&self) -> String {
        let props = self.moved + self.added + self.hidden;
        let mut parts = vec![];
        if self.races > 0 {
            parts.push(plural(self.races, "race", "races"));
        }
        if props > 0 {
            parts.push(plural(props, "prop edit", "prop edits"));
        }
        if parts.is_empty() {
            parts.push("nothing changed yet".into());
        }
        parts.push(if self.in_game { "on in your game".into() } else { "off".into() });
        parts.join(" · ")
    }

    /// Home's "Changes" row.
    pub fn changes_text(&self) -> String {
        if self.changes() == 0 {
            return "None yet · the original everywhere".into();
        }
        let mut parts = vec![];
        for (n, one, many) in [
            (self.moved, "moved", "moved"),
            (self.added, "added", "added"),
            (self.hidden, "hidden", "hidden"),
            (self.races, "race", "races"),
        ] {
            if n > 0 {
                parts.push(format!("{n} {}", if n == 1 { one } else { many }));
            }
        }
        parts.join(" · ")
    }
}

/// "1 race", "3 races".
pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Read one mod's summary (`maps`: the mods' folder).
pub fn summary(maps: &Path, folder: &str) -> Result<ModSummary> {
    let dir = maps.join(folder);
    let edir = store::editor_dir(maps, folder);
    let h = History::open_read(&dir, &edir, ob_studio::MOD_DOCS, Arc::new(history::disk::Real))
        .with_context(|| format!("reading the history of {folder}"))?;
    let in_game = store::enabled(maps).iter().any(|f| f == folder);
    let read_only_by = h.read_only_by().flatten().map(|o| o.program.clone());
    let last_step = h.steps().last().map(|s| s.label.clone());
    let saved_state = if h.pending().is_some() { Saved::Behind } else { Saved::Saved };
    let text = h.doc(store::FILE).unwrap_or_default();
    let mut s = ModSummary {
        folder: folder.into(),
        dir,
        name: folder.into(),
        moved: 0,
        added: 0,
        hidden: 0,
        races: 0,
        in_game,
        saved: saved_state,
        read_only_by,
        last_step,
    };
    match MapMod::parse(text) {
        Ok(m) => {
            if !m.name.trim().is_empty() {
                s.name = m.name.trim().to_string();
            }
            for p in &m.props {
                match p {
                    PropEdit::Move { .. } => s.moved += 1,
                    PropEdit::Delete { .. } => s.hidden += 1,
                    _ => s.added += 1,
                }
            }
            s.races = m.races.len();
        }
        Err(e) => {
            let mut why = format!("{} does not read: {e}", store::FILE);
            if !ob_studio::cli::repairs(text).is_empty() {
                why.push_str(&format!(". {}", ob_studio::cli::repair_hint(&s.dir)));
            }
            s.saved = Saved::Error(why)
        }
    }
    Ok(s)
}

/// Every mod in `maps`, sorted by folder; one that does not read at all is left out (and logged).
pub fn all(maps: &Path) -> Vec<ModSummary> {
    store::list(maps)
        .into_iter()
        .filter_map(|f| match summary(maps, &f) {
            Ok(s) => Some(s),
            Err(e) => {
                log::warn!("studio: {f}: {e:#}");
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ob-studio-home-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_new_mod_reads_as_nothing_changed() {
        let maps = temp("new");
        std::fs::create_dir_all(maps.join("harbour-edits")).unwrap();
        std::fs::write(maps.join("harbour-edits").join(store::FILE), "overbright-map 1\nname = Harbour edits\n")
            .unwrap();
        let s = summary(&maps, "harbour-edits").unwrap();
        assert_eq!(s.name, "Harbour edits");
        assert_eq!(s.changes(), 0);
        assert_eq!(s.saved, Saved::Saved);
        assert!(!s.in_game);
        assert_eq!(s.line(), "nothing changed yet · off");
        assert_eq!(s.changes_text(), "None yet · the original everywhere");
        store::set_enabled(&maps, "harbour-edits", true).unwrap();
        assert!(summary(&maps, "harbour-edits").unwrap().in_game);
        assert_eq!(all(&maps).len(), 1);
        let _ = std::fs::remove_dir_all(&maps);
    }

    #[test]
    fn a_broken_file_is_an_error_row_not_a_crash() {
        let maps = temp("broken");
        std::fs::create_dir_all(maps.join("bad")).unwrap();
        std::fs::write(maps.join("bad").join(store::FILE), "not a map mod").unwrap();
        let s = summary(&maps, "bad").unwrap();
        assert!(matches!(s.saved, Saved::Error(_)), "{s:?}");
        let _ = std::fs::remove_dir_all(&maps);
    }
}
