//! What runs and what the checks found, as the status bar, the bottom dock's Jobs and Problems tabs and Home show it
//! (docs/modding/studio.md, "Layout": the status bar's busy job line and problem counts).
//!
//! The jobs are the runs Studio starts: a test of the race in the game (the map editor's Test race and Drive here)
//! and a simulation (one run or a batch). The counts are the checks Studio already makes: the map editor's
//! validation while World or Races has the mod open, otherwise the Mod page's ready list with the map checks.

use ob_look::lamp::Mark;
use ob_studio::session::Level;

use super::home;
use super::shell::Studio;
use super::words;

/// One running job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    /// What runs, in the job's own words ("You are racing Casino sprint").
    pub word: String,
    /// Runs done (the running one counted) of all, for a batch.
    pub progress: Option<(usize, usize)>,
}

impl Job {
    /// The line the status bar and the Jobs tab show.
    pub fn line(&self) -> String {
        match self.progress {
            Some((done, all)) if all > 1 => format!("{} {done} of {all}", self.word),
            _ => self.word.clone(),
        }
    }
}

/// What the checks found: problems (they stop a test or the export) and warnings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub problems: usize,
    pub warnings: usize,
}

impl Counts {
    /// "Problems 0" and, when there are any, "· 1 warning" (the second part in the warning colour).
    pub fn words(&self) -> (String, Option<String>) {
        let w = (self.warnings > 0).then(|| home::plural(self.warnings, "warning", "warnings"));
        (format!("{} {}", words::PROBLEMS, self.problems), w)
    }
}

impl Studio {
    /// The jobs running now, oldest first.
    pub fn jobs(&self) -> Vec<Job> {
        let mut v = vec![];
        if let Some(w) = crate::editor::testplay::strip(&self.ctx) {
            v.push(Job { word: w, progress: None });
        }
        if let Some(h) = self.editor.as_ref() {
            let sim = h.app.panels.sim.borrow();
            if sim.running() {
                v.push(Job { word: words::SIMULATING.into(), progress: Some(sim.batch_progress()) });
            }
        }
        v
    }

    /// The Problems tab's lines outside World and Races: the Mod page's ready list, what is not ready or warns.
    pub fn problem_lines(&self) -> Vec<(Mark, String)> {
        let Some(open) = self.open_mod() else { return vec![] };
        let Some(d) = self.mod_page.data.as_ref().filter(|d| d.folder == open.folder) else { return vec![] };
        let mut v: Vec<(Mark, String)> = d
            .ready
            .iter()
            .filter(|p| !p.ok)
            .map(|p| (if p.level == Level::Warning { Mark::Warn } else { Mark::Error }, p.text.clone()))
            .collect();
        if d.check_warnings > 0 {
            v.push((Mark::Warn, home::plural(d.check_warnings, "map check warns", "map checks warn")));
        }
        v
    }

    /// What the checks found on the open mod (None: no mod open, or not checked yet).
    pub fn counts(&self) -> Option<Counts> {
        let open = self.open_mod()?;
        if self.editing()
            && let Some(s) = self.editor.as_ref().and_then(|h| h.app.state.as_ref())
        {
            let n = |l| s.session.issues.iter().filter(|i| i.level == l).count();
            return Some(Counts { problems: n(Level::Error), warnings: n(Level::Warning) });
        }
        self.mod_page.data.as_ref().filter(|d| d.folder == open.folder)?;
        self.mod_page.counts().map(|(problems, warnings)| Counts { problems, warnings })
    }
}
