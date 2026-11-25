//! Making a new map mod: the one way Studio's New mod dialog, the map editor's New map mod… and
//! `overbright-studio-cli pack create` make one. The name is checked, the folder derived from it (or given), and a
//! mod that exists already, a folder in use or a bad name is refused with words a player understands; nothing is
//! ever written over an existing mod.

use std::fmt;
use std::path::{Path, PathBuf};

use ob_map::tools::map::format::{MapMod, valid_id, valid_region};
use ob_map::tools::map::store;

/// The region a new mod is edited on unless another is given.
pub const REGION: &str = "L4RA";
/// The longest name a mod may have (characters).
pub const MAX_NAME: usize = 60;
/// The folder a mod without a name gets.
pub const UNNAMED: &str = "my-map";

/// Why a mod was not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// No name given (only spaces).
    NoName,
    /// The name is longer than [`MAX_NAME`].
    LongName,
    /// Another mod has this name already (its folder).
    NameUsed(String),
    /// The folder name has other characters than a-z, 0-9, - and _ (or is too long).
    BadFolder(String),
    /// A mod (or something else) is in this folder already.
    FolderUsed(String),
    /// Not a region of the game's files.
    BadRegion(String),
    /// The disk refused (the reason).
    Disk(String),
}

impl Refused {
    /// Its word in the command line's JSON (`error.reason`).
    pub fn reason(&self) -> &'static str {
        match self {
            Refused::NoName => "no_name",
            Refused::LongName => "long_name",
            Refused::NameUsed(_) => "name_used",
            Refused::BadFolder(_) => "bad_folder",
            Refused::FolderUsed(_) => "folder_used",
            Refused::BadRegion(_) => "bad_region",
            Refused::Disk(_) => "disk",
        }
    }

    /// A bad value (the command line's usage error), not a clash with what is on the disk.
    pub fn is_input(&self) -> bool {
        matches!(self, Refused::NoName | Refused::LongName | Refused::BadFolder(_) | Refused::BadRegion(_))
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refused::NoName => write!(f, "Give the mod a name"),
            Refused::LongName => write!(f, "The name is too long: at most {MAX_NAME} characters"),
            Refused::NameUsed(folder) => {
                write!(f, "You have a mod with this name already (in the folder {folder}): pick another name")
            }
            Refused::BadFolder(folder) => {
                write!(f, "\"{folder}\" is not a folder name: use a-z, 0-9, - and _ (at most 40)")
            }
            Refused::FolderUsed(folder) => {
                write!(f, "The folder {folder} is in use already: pick another name or folder")
            }
            Refused::BadRegion(r) => write!(f, "\"{r}\" is not a region of the game (as {REGION})"),
            Refused::Disk(e) => write!(f, "The mod could not be written: {e}"),
        }
    }
}

impl std::error::Error for Refused {}

/// A folder name from a mod's name ("Casino sprints" → casino-sprints; [`UNNAMED`] when nothing is left).
pub fn folder_from(name: &str) -> String {
    let mut f = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            f.push(c.to_ascii_lowercase());
        } else if (c == ' ' || c == '-' || c == '_') && !f.ends_with('-') && !f.is_empty() {
            f.push('-');
        }
    }
    let f = f.chars().take(40).collect::<String>().trim_end_matches('-').to_string();
    if f.is_empty() { UNNAMED.into() } else { f }
}

/// The mod a name (and an optional folder) would make, checked against the mods in `maps`: the folder to use.
/// A name is needed.
pub fn plan(maps: &Path, name: &str, folder: Option<&str>) -> Result<String, Refused> {
    if name.trim().is_empty() {
        return Err(Refused::NoName);
    }
    let folder = match folder.map(str::trim).filter(|f| !f.is_empty()) {
        Some(f) => f.to_string(),
        None => folder_from(name),
    };
    check(maps, name, &folder)?;
    Ok(folder)
}

/// Whether a mod `name` may be made in `folder` under `maps` (an empty name is allowed here: the map editor's
/// mods may have none).
pub fn check(maps: &Path, name: &str, folder: &str) -> Result<(), Refused> {
    let name = name.trim();
    if name.chars().count() > MAX_NAME {
        return Err(Refused::LongName);
    }
    if !valid_id(folder) {
        return Err(Refused::BadFolder(folder.to_string()));
    }
    let dir = maps.join(folder);
    let in_use = dir.join(store::FILE).exists()
        || dir.is_file()
        || std::fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_some());
    if in_use {
        return Err(Refused::FolderUsed(folder.to_string()));
    }
    if !name.is_empty() {
        for other in store::list(maps) {
            let Ok(m) = store::read_file(&maps.join(&other).join(store::FILE)) else { continue };
            if m.name.trim().eq_ignore_ascii_case(name) {
                return Err(Refused::NameUsed(other));
            }
        }
    }
    Ok(())
}

/// What a new mod starts from: nothing, one race to change, or the sample (Studio's Home, `pack create
/// --template`). The races and props are our own work: the sample race pack, its start lines facing down the road.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Template {
    /// Nothing changed yet.
    #[default]
    Empty,
    /// One sprint along the docks (start, nine checkpoints, finish).
    Sprint,
    /// One circuit downtown, three laps.
    Circuit,
    /// The sample race pack: three races with start gates, barrels and curve signs.
    Sample,
}

/// The sample race pack's map file.
pub const SAMPLE: &str = include_str!("templates/sample.obmap");

impl Template {
    pub const ALL: [Template; 4] = [Template::Empty, Template::Sprint, Template::Circuit, Template::Sample];

    /// Its word on the command line.
    pub fn word(self) -> &'static str {
        match self {
            Template::Empty => "empty",
            Template::Sprint => "sprint",
            Template::Circuit => "circuit",
            Template::Sample => "sample",
        }
    }

    pub fn from_word(w: &str) -> Option<Template> {
        Template::ALL.into_iter().find(|t| t.word().eq_ignore_ascii_case(w.trim()))
    }

    /// The race the template holds (the sample's race of that id).
    fn race(self) -> Option<&'static str> {
        match self {
            Template::Sprint => Some("harbour-line"),
            Template::Circuit => Some("downtown-loop"),
            _ => None,
        }
    }

    /// The mod the template makes, named `name`.
    pub fn map(self, name: &str) -> MapMod {
        let mut m = match self {
            Template::Empty => MapMod::default(),
            _ => MapMod::parse(SAMPLE).expect("the sample reads"),
        };
        if let Some(id) = self.race() {
            m.races.retain(|r| r.id == id);
            m.props.clear();
            m.author.clear();
            m.description.clear();
        }
        m.name = name.trim().to_string();
        m
    }
}

/// Make the mod `name` in `<maps>/<folder>`, edited on `region`: its `map.obmap` (nothing changed yet) and its
/// view's region. Returns its folder's path.
pub fn create(maps: &Path, folder: &str, name: &str, region: &str) -> Result<PathBuf, Refused> {
    create_from(maps, folder, name, region, Template::Empty)
}

/// [`create`], the mod starting from `template`.
pub fn create_from(
    maps: &Path,
    folder: &str,
    name: &str,
    region: &str,
    template: Template,
) -> Result<PathBuf, Refused> {
    check(maps, name, folder)?;
    let region = region.trim().to_ascii_uppercase();
    if !valid_region(&region) {
        return Err(Refused::BadRegion(region));
    }
    let m = template.map(name);
    let dir = maps.join(folder);
    std::fs::create_dir_all(&dir).map_err(|e| Refused::Disk(format!("{}: {e}", dir.display())))?;
    ob_core::platform::fs::write_atomic(&dir.join(store::FILE), m.write().as_bytes(), true)
        .map_err(|e| Refused::Disk(format!("{}: {e}", dir.join(store::FILE).display())))?;
    // The region the mod is edited on (the editor's view file; it writes the rest of it).
    let view = store::editor_dir(maps, folder).join(store::EDITOR_TOML);
    let text = format!("# Overbright map editor: this mod's view (not part of the mod).\nregion = \"{region}\"\n");
    ob_core::platform::fs::write_atomic(&view, text.as_bytes(), false)
        .map_err(|e| Refused::Disk(format!("{}: {e}", view.display())))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ob-studio-create-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_folder_comes_from_the_name() {
        assert_eq!(folder_from("Casino sprints"), "casino-sprints");
        assert_eq!(folder_from("  Dock  -- Remix! "), "dock-remix");
        assert_eq!(folder_from("Ünïcode ✓"), "ncode");
        assert_eq!(folder_from("!!!"), UNNAMED);
        assert_eq!(folder_from(&"a ".repeat(40)).len(), 39, "at most 40, no dash at the end");
    }

    #[test]
    fn the_templates_make_their_races() {
        let maps = temp("templates");
        for (t, races, props) in
            [(Template::Empty, 0, 0), (Template::Sprint, 1, 0), (Template::Circuit, 1, 0), (Template::Sample, 3, 66)]
        {
            assert_eq!(Template::from_word(t.word()), Some(t));
            let folder = t.word();
            let dir = create_from(&maps, folder, &format!("My {folder}"), REGION, t).unwrap();
            let m = store::read_file(&dir.join(store::FILE)).unwrap();
            assert_eq!((m.races.len(), m.props.len(), m.name.as_str()), (races, props, &*format!("My {folder}")));
            // Every start line faces its first checkpoint (within a quarter turn): the grid faces down the road.
            for r in &m.races {
                let s = r.start.unwrap();
                let to = r.checkpoints[0] - s.position.truncate();
                let h = s.heading.to_radians();
                assert!(to.normalize().dot(glam::Vec2::new(h.cos(), h.sin())) > 0.7, "{}: {to} {}", r.id, s.heading);
            }
        }
        assert_eq!(Template::from_word("nope"), None);
        let _ = std::fs::remove_dir_all(&maps);
    }

    #[test]
    fn a_new_mod_is_made_and_never_twice() {
        let maps = temp("twice");
        assert_eq!(plan(&maps, "Harbour edits", None).unwrap(), "harbour-edits");
        let dir = create(&maps, "harbour-edits", "Harbour edits", "l4ra").unwrap();
        let m = store::read_file(&dir.join(store::FILE)).unwrap();
        assert_eq!(m.name, "Harbour edits");
        assert!(m.props.is_empty() && m.races.is_empty());
        let view = std::fs::read_to_string(store::editor_dir(&maps, "harbour-edits").join(store::EDITOR_TOML)).unwrap();
        assert!(view.contains("region = \"L4RA\""), "{view}");
        assert_eq!(store::list(&maps), ["harbour-edits"]);
        // The same name again: its folder is in use.
        assert_eq!(plan(&maps, "Harbour edits", None), Err(Refused::FolderUsed("harbour-edits".into())));
        // The same name in another folder (any case): refused too.
        assert_eq!(plan(&maps, "harbour EDITS", Some("other")), Err(Refused::NameUsed("harbour-edits".into())));
        assert!(create(&maps, "harbour-edits", "", REGION).is_err(), "never over an existing mod");
        assert_eq!(store::read_file(&dir.join(store::FILE)).unwrap().name, "Harbour edits");
        let _ = std::fs::remove_dir_all(&maps);
    }

    #[test]
    fn bad_names_and_folders_are_refused_clearly() {
        let maps = temp("bad");
        assert_eq!(plan(&maps, "   ", None), Err(Refused::NoName));
        assert_eq!(plan(&maps, &"x".repeat(MAX_NAME + 1), None), Err(Refused::LongName));
        assert_eq!(plan(&maps, "Fine", Some("Bad Folder")), Err(Refused::BadFolder("Bad Folder".into())));
        assert_eq!(plan(&maps, "Fine", Some("../up")), Err(Refused::BadFolder("../up".into())));
        assert_eq!(create(&maps, "fine", "Fine", "L4 RA"), Err(Refused::BadRegion("L4 RA".into())));
        // A folder with other things in it is not taken.
        std::fs::create_dir_all(maps.join("busy")).unwrap();
        std::fs::write(maps.join("busy/notes.txt"), "mine").unwrap();
        assert_eq!(plan(&maps, "Busy", None), Err(Refused::FolderUsed("busy".into())));
        // An empty folder is.
        std::fs::create_dir_all(maps.join("empty")).unwrap();
        assert_eq!(plan(&maps, "Empty", None).as_deref(), Ok("empty"));
        for r in [Refused::NoName, Refused::FolderUsed("a".into()), Refused::BadFolder("A".into())] {
            let w = r.to_string();
            assert!(w.chars().find(|c| c.is_alphabetic()).unwrap().is_uppercase() && !w.ends_with('.'), "{w}");
        }
        assert!(store::list(&maps).is_empty(), "nothing was made");
        let _ = std::fs::remove_dir_all(&maps);
    }
}
