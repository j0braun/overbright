//! Studio's workspaces and the rail that lists them (docs/modding/studio.md, "The rail").
//!
//! Home sits at the top; the MAKE workspaces follow in the order they shipped; SHARE (Mod) and Settings sit at the
//! bottom. Only the workspaces this version has are listed (no greyed slots); a shipped workspace never moves and
//! its Ctrl+number is never given to another.

use super::words;

/// One workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Workspace {
    Home,
    World,
    Races,
    /// The game's assets by name: models and textures, where each is used.
    Browser,
    /// The mod's recorded runs: play one back, its telemetry, compare two.
    Runs,
    Mod,
    Settings,
    /// The Changes page (Ctrl+D): not on the rail; it opens over World and goes back to where it was opened.
    Changes,
    /// The Data page (Ctrl+G): not on the rail either; it opens over World and goes back to where it was opened.
    Data,
}

/// Where a workspace sits on the rail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// Home, at the top.
    Top,
    /// MAKE: the workspaces that make things, in the order they shipped.
    Make,
    /// SHARE, anchored at the bottom.
    Share,
    /// Settings, last.
    Bottom,
}

/// What a workspace's main area is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A page on the slab, without toolbar or docks (Home, Mod, Settings).
    Page,
    /// A tool workspace: toolbar, left panel, a view, the right and bottom docks.
    Tool,
}

impl Workspace {
    /// Every workspace in rail order (top to bottom).
    pub const ALL: [Workspace; 7] = [
        Workspace::Home,
        Workspace::World,
        Workspace::Races,
        Workspace::Browser,
        Workspace::Runs,
        Workspace::Mod,
        Workspace::Settings,
    ];

    /// Its name on the rail, in the address bar and in Find.
    pub fn name(self) -> &'static str {
        match self {
            Workspace::Home => words::HOME,
            Workspace::World => words::WORLD_2,
            Workspace::Races => words::RACES,
            Workspace::Browser => words::BROWSER,
            Workspace::Runs => words::RUNS,
            Workspace::Mod => words::MOD,
            Workspace::Settings => words::SETTINGS,
            Workspace::Changes => words::CHANGES,
            Workspace::Data => words::DATA,
        }
    }

    /// The key after Ctrl that opens it (never reused for another workspace).
    pub fn ctrl_key(self) -> &'static str {
        match self {
            Workspace::Home => "1",
            Workspace::World => "2",
            Workspace::Races => "3",
            Workspace::Browser => "4",
            Workspace::Runs => "5",
            Workspace::Mod => "0",
            Workspace::Settings => ",",
            Workspace::Changes => "D",
            Workspace::Data => "G",
        }
    }

    /// What it is for, in one line (Home's "what each place does").
    pub fn what(self) -> &'static str {
        match self {
            Workspace::Home => words::YOUR_MODS_AND_WHERE_TO_START,
            Workspace::World => words::MOVE_ADD_AND_HIDE_PROPS_IN,
            Workspace::Races => words::MAKE_A_RACE_START_CHECKPOINTS_FINISH,
            Workspace::Browser => words::FIND_THE_GAME_S_MODELS_AND,
            Workspace::Runs => words::PLAY_A_RECORDED_RUN_BACK_READ,
            Workspace::Changes => words::EVERYTHING_YOUR_MOD_CHANGES_AND_WHICH,
            Workspace::Data => words::YOUR_PROPS_AND_RACES_AS_A,
            Workspace::Mod => words::ITS_DETAILS_WHETHER_IT_IS_READY,
            Workspace::Settings => words::STUDIO_S_OWN_SETTINGS,
        }
    }

    pub fn section(self) -> Section {
        match self {
            Workspace::Home => Section::Top,
            Workspace::World
            | Workspace::Races
            | Workspace::Browser
            | Workspace::Runs
            | Workspace::Changes
            | Workspace::Data => Section::Make,
            Workspace::Mod => Section::Share,
            Workspace::Settings => Section::Bottom,
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Workspace::World | Workspace::Races | Workspace::Runs => Kind::Tool,
            _ => Kind::Page,
        }
    }

    /// Its view segment in an address (`ob://<mod>/@world`): always starts with `@`, which no kind name does.
    pub fn view(self) -> &'static str {
        match self {
            Workspace::Home => "@home",
            Workspace::World => "@world",
            Workspace::Races => "@races",
            Workspace::Browser => "@browser",
            Workspace::Runs => "@runs",
            Workspace::Mod => "@mod",
            Workspace::Settings => "@settings",
            Workspace::Changes => "@changes",
            Workspace::Data => "@data",
        }
    }

    pub fn from_view(v: &str) -> Option<Workspace> {
        Workspace::ALL.into_iter().chain(Workspace::PAGES).find(|w| w.view() == v)
    }

    /// The pages that are not on the rail (opened by key, address or a button).
    pub const PAGES: [Workspace; 2] = [Workspace::Changes, Workspace::Data];

    /// The rail's entry that shows as current while this is open (Changes: World).
    pub fn on_rail(self) -> Workspace {
        match self {
            Workspace::Changes | Workspace::Data => Workspace::World,
            w => w,
        }
    }

    /// The workspace `step` places further down the rail (wrapping round), as Ctrl+PgDn / RB go.
    pub fn step(self, step: i32) -> Workspace {
        let n = Workspace::ALL.len() as i32;
        let i = Workspace::ALL.iter().position(|w| *w == self.on_rail()).unwrap_or(0) as i32;
        Workspace::ALL[(i + step).rem_euclid(n) as usize]
    }

    /// The workspace Ctrl+`key` opens.
    pub fn for_ctrl_key(key: &str) -> Option<Workspace> {
        Workspace::ALL.into_iter().chain(Workspace::PAGES).find(|w| w.ctrl_key() == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_order_and_keys() {
        let sections: Vec<_> = Workspace::ALL.iter().map(|w| w.section()).collect();
        assert_eq!(
            sections,
            [Section::Top, Section::Make, Section::Make, Section::Make, Section::Make, Section::Share, Section::Bottom]
        );
        let keys: Vec<_> = Workspace::ALL.iter().map(|w| w.ctrl_key()).collect();
        for (i, k) in keys.iter().enumerate() {
            assert!(!keys[..i].contains(k), "Ctrl+{k} twice");
        }
        // The shipped workspaces' keys are fixed: a later workspace takes a new one.
        assert_eq!(Workspace::for_ctrl_key("2"), Some(Workspace::World));
        assert_eq!(Workspace::for_ctrl_key("3"), Some(Workspace::Races));
        assert_eq!(Workspace::for_ctrl_key("0"), Some(Workspace::Mod));
        assert_eq!(Workspace::for_ctrl_key("4"), Some(Workspace::Browser));
        assert_eq!(Workspace::for_ctrl_key("5"), Some(Workspace::Runs));
    }

    #[test]
    fn stepping_wraps() {
        assert_eq!(Workspace::Home.step(1), Workspace::World);
        assert_eq!(Workspace::Home.step(-1), Workspace::Settings);
        assert_eq!(Workspace::Settings.step(1), Workspace::Home);
    }

    #[test]
    fn views_start_with_at() {
        for w in Workspace::ALL {
            assert!(w.view().starts_with('@'));
            assert_eq!(Workspace::from_view(w.view()), Some(w));
        }
    }
}
