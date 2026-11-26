//! "Try it once" (docs/launcher.md, Mods page): the game starts with one mod pack on for this run only and writes its
//! saves into a copy of the profiles, so the player's own saves stay as they are. The copy lives in `<app>/tries/`;
//! when the game ends the launcher says where it is and offers to keep or discard it.

use std::path::{Path, PathBuf};

/// One trial run's folders.
#[derive(Clone, Debug, PartialEq)]
pub struct Trial {
    pub pack: String,
    /// The folder that holds the profiles' copy (the game's data home for this run).
    pub dir: PathBuf,
}

/// `<app>/tries`.
pub fn tries_dir() -> PathBuf {
    crate::platform::dirs::app().join("tries")
}

/// Copies the folder `from` into `to` (files and folders only; links are skipped).
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let t = e.file_type()?;
        let dst = to.join(e.file_name());
        if t.is_dir() {
            copy_tree(&e.path(), &dst)?;
        } else if t.is_file() {
            std::fs::copy(e.path(), &dst)?;
        }
    }
    Ok(())
}

/// Makes the copy for a trial of `pack`: a new folder under `tries` with the profiles in `profiles` (the player's
/// profiles folder, read only here) inside it under the same name.
pub fn begin(pack: &str, tries: &Path, profiles: &Path) -> Result<Trial, String> {
    let stamp: String = crate::qol::saves::timestamp(std::time::SystemTime::now())
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let safe: String = pack.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    let dir = tries.join(format!("{safe}-{stamp}"));
    let name = profiles.file_name().ok_or("no profiles folder")?;
    let r = std::fs::create_dir_all(&dir).and_then(|()| {
        if profiles.is_dir() { copy_tree(profiles, &dir.join(name)) } else { Ok(()) }
    });
    if let Err(e) = r {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(format!("the saves copy could not be made in {}: {e}", dir.display()));
    }
    Ok(Trial { pack: pack.into(), dir })
}

impl Trial {
    /// The game's environment for this run.
    pub fn env(&self) -> Vec<(String, String)> {
        vec![
            (crate::mod_packs::TRY_VAR.into(), self.pack.clone()),
            (crate::platform::dirs::TRIAL_SAVES_VAR.into(), self.dir.display().to_string()),
        ]
    }

    /// Deletes the copy. Only a folder inside `tries` is ever removed.
    pub fn discard(&self, tries: &Path) -> Result<(), String> {
        if self.dir.parent() != Some(tries) {
            return Err(format!("{} is not a trial copy", self.dir.display()));
        }
        match std::fs::remove_dir_all(&self.dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", self.dir.display())),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_copy_is_made_apart_and_discarded() {
        let t = std::env::temp_dir().join(format!("ob-trial-{}", ob_core::unique_tag()));
        let profiles = t.join("real").join("NFS Underground 2");
        std::fs::create_dir_all(profiles.join("ANN")).unwrap();
        std::fs::write(profiles.join("ANN/ANN"), b"save").unwrap();
        let tries = t.join("tries");
        let tr = begin("docks", &tries, &profiles).unwrap();
        assert_eq!(std::fs::read(tr.dir.join("NFS Underground 2/ANN/ANN")).unwrap(), b"save");
        // The game writes into the copy; the player's file is untouched.
        std::fs::write(tr.dir.join("NFS Underground 2/ANN/ANN"), b"changed").unwrap();
        assert_eq!(std::fs::read(profiles.join("ANN/ANN")).unwrap(), b"save");
        let env = tr.env();
        assert!(env.iter().any(|(k, v)| k == "NFSU2_TRY_PACK" && v == "docks"));
        assert!(env.iter().any(|(k, v)| k == "NFSU2_TRY_SAVES" && Path::new(v) == tr.dir));
        assert!(tr.discard(&t.join("elsewhere")).is_err());
        tr.discard(&tries).unwrap();
        assert!(!tr.dir.exists());
        assert!(profiles.join("ANN/ANN").is_file());
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn no_profiles_is_an_empty_copy() {
        let t = std::env::temp_dir().join(format!("ob-trial-{}", ob_core::unique_tag()));
        let tr = begin("a b", &t.join("tries"), &t.join("none/NFS Underground 2")).unwrap();
        assert!(tr.dir.is_dir());
        assert!(tr.dir.file_name().unwrap().to_string_lossy().starts_with("a-b-"));
        let _ = std::fs::remove_dir_all(&t);
    }
}
