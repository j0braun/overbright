//! The three ways to try a race, in words: Test race, Drive here and
//! Simulate. Their names, the one sentence each that says who drives and
//! where it shows, the note on the AI-driven slot, the game-window strip
//! and the Help ▸ How to test your race page all come from here, so the
//! toolbar, the Simulate tab, the help page and the docs test read the
//! same words.
//!
//! Choices kept in one place each (change them here):
//! - the names stay Test race / Drive here / Simulate ([`TEST_RACE`],
//!   [`DRIVE_HERE`], [`SIMULATE`]);
//! - the player's slot stays in a simulation's results, with a note
//!   ([`SHOW_AI_SLOT`], [`SLOT_NOTE`]);
//! - a simulation has the traffic the race's career event has
//!   ([`TRAFFIC`]);
//! - Simulate runs the race once per click ([`RUNS`]).

/// One way to try the mod.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunMode {
    /// The button's name.
    pub name: &'static str,
    /// Who drives ("You drive").
    pub who: &'static str,
    /// What it does, in one sentence.
    pub what: &'static str,
    /// Its key ("" none).
    pub key: &'static str,
}

impl RunMode {
    /// "Test race: you drive." (the tab header's and the help page's lead).
    pub fn lead(&self) -> String {
        let mut who = self.who.chars();
        let first = who.next().map(|c| c.to_lowercase().to_string()).unwrap_or_default();
        format!("{}: {first}{}.", self.name, who.as_str())
    }

    /// The tooltip: the heading, then the sentence and the key.
    pub fn tooltip(&self) -> String {
        let key = if self.key.is_empty() { String::new() } else { format!(" ({})", self.key) };
        format!("{}\n{}{key}", self.lead(), self.what)
    }
}

pub const TEST_RACE: RunMode = RunMode {
    name: "Test race",
    who: "You drive",
    what: "Race the selected race yourself in the game window. Finishing it marks the race tested.",
    key: "F5",
};

pub const DRIVE_HERE: RunMode = RunMode {
    name: "Drive here",
    who: "You roam",
    what: "Drive freely in the game window, with this mod loaded, from the ground the camera looks at.",
    key: "Shift+F5",
};

pub const SIMULATE: RunMode = RunMode {
    name: "Simulate",
    who: "The AI drives",
    what: "The game races the selected race with the AI in every car, without a window. \
           Watch it here and see where cars got stuck.",
    key: "",
};

/// Simulate with no race chosen: the world around the camera.
pub const SIMULATE_WORLD: &str =
    "The game runs the traffic and street racers around the camera without a window. Watch them here.";

/// The three, in the order the help page lists them.
pub const ALL: [RunMode; 3] = [SIMULATE, TEST_RACE, DRIVE_HERE];

/// The player's slot is listed in a simulation's results (with
/// [`SLOT_NOTE`]); false would list the opponents only.
pub const SHOW_AI_SLOT: bool = true;

/// Under the results while the player's slot is listed.
pub const SLOT_NOTE: &str =
    "Your slot (AI) is driven by the AI, so its place and time are not yours. Use Test race to race it yourself.";

/// Beside the player's slot in the results.
pub const SLOT_ROW: &str = "AI-driven, not your time";

/// The traffic a simulation has.
pub const TRAFFIC: &str = "Traffic and opponents are the ones the race's career event has, as in Test race.";

/// Races per Simulate click (one; the seed picks which run).
pub const RUNS: u32 = 1;

/// The seed's tooltip.
pub const SEED_TIP: &str =
    "The same seed runs the same race again, car for car. Another seed changes the AI's and the traffic's choices.";

/// The strip over the view while the game window is open (drawn like
/// the hint line): its lead, what runs, then the `Esc` key chip and
/// [`STRIP_END`].
pub const STRIP_LEAD: &str = "Game window open";

/// What runs in the game window.
pub fn strip(race: Option<&str>) -> String {
    match race {
        Some(r) => format!("You are racing {r}"),
        None => "You are driving from the camera's spot".to_string(),
    }
}

/// After the `Esc` chip.
pub const STRIP_END: &str = "in the game ends it · You can keep editing here";

/// The help page's title (Help ▸ How to test your race).
pub const HELP_TITLE: &str = "How to test your race";

/// The help page: (heading, text) in order.
pub fn help() -> Vec<(String, String)> {
    let runs = if RUNS == 1 { "once" } else { "several times" };
    let mut v: Vec<(String, String)> = ALL.iter().map(|m| (m.lead(), m.what.to_string())).collect();
    v[0].1 = format!(
        "{} With no race selected, Simulate shows the world around the camera instead. {TRAFFIC} \
         It runs the race {runs} per click; the Seed picks which run.",
        v[0].1
    );
    v.push((
        "Reading a simulation".to_string(),
        format!(
            "Cars are boxes in the 3D view and arrows on the map: lime is your slot and the car you follow, light the \
             opponents, grey the traffic, amber the street racers. {SLOT_NOTE} Cars that got stuck, drove the \
             wrong way or crashed are added to the Validation tab with a Show button."
        ),
    ));
    v.push((
        "A good order".to_string(),
        "1. Simulate to find the spots where the AI has trouble and fix them. \
         2. Test race to drive it yourself; a finish marks the race tested. \
         3. Drive here to look around the route in the game."
            .to_string(),
    ));
    v
}

fn help_id() -> egui::Id {
    egui::Id::unique("editor.modes.help")
}

/// Ask the shell to open the help page (a panel cannot post actions).
pub fn request_help(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(help_id(), true));
}

/// Whether the help page was asked for (taken once).
pub fn take_help_request(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(help_id()).unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_mode_says_who_drives() {
        assert_eq!(TEST_RACE.lead(), "Test race: you drive.");
        assert_eq!(DRIVE_HERE.lead(), "Drive here: you roam.");
        assert_eq!(SIMULATE.lead(), "Simulate: the AI drives.");
        assert!(TEST_RACE.tooltip().ends_with("(F5)"));
        assert!(!SIMULATE.tooltip().contains("()"));
        let h = help();
        assert_eq!(h.len(), 5);
        assert!(h[0].1.contains("Seed") && h[0].1.contains("career event"));
        assert_eq!(strip(Some("Casino sprint")), "You are racing Casino sprint");
    }
}
