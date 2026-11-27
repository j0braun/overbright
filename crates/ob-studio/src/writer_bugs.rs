//! Known writer bugs: faults an older build wrote into a mod's files, which a later build recognises and mends
//! (docs/modding/studio-cli.md, "Edits and the history"). Every save records the build that wrote it
//! ([`crate::history::SAVED_BY`]), so the finding can name that build. The fix is the one `pack repair` makes, as one
//! undoable step; nothing here writes.

use crate::history::SavedBy;
use ob_map::tools::map::format::MapMod;

/// One fault an older build is known to have written.
#[derive(Clone, Copy)]
pub struct Bug {
    pub id: &'static str,
    /// What was written wrong, in words for a player.
    pub what: &'static str,
    /// What the repair does.
    pub fix: &'static str,
    /// Whether a map file's text has it.
    found: fn(&str) -> bool,
}

/// Every known writer bug, oldest first. A new one is added here with its test.
pub const KNOWN: &[Bug] = &[Bug {
    id: "laps-on-a-sprint",
    what: "a sprint or drift race with more than one lap, which the game does not read",
    fix: "the race runs its one lap",
    found: |text| matches!((MapMod::parse(text), MapMod::parse_repairing(text)), (Err(_), Ok((_, f))) if !f.is_empty()),
}];

/// A known bug found in a mod's file, and who wrote the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub id: &'static str,
    pub what: &'static str,
    pub fix: &'static str,
    /// The build that last wrote the files, as the mod recorded it (None: no record, an older build or another
    /// program wrote them).
    pub written_by: Option<String>,
}

impl Finding {
    /// The sentence a player reads.
    pub fn sentence(&self) -> String {
        match &self.written_by {
            Some(b) => format!("{b} wrote {}", self.what),
            None => format!("An older build wrote {}", self.what),
        }
    }
}

/// The known writer bugs `map_text` (the text of a mod's `map.obmap`) has; `saved_by`: the mod's record of its last
/// writer.
pub fn check(map_text: &str, saved_by: Option<&SavedBy>) -> Vec<Finding> {
    KNOWN
        .iter()
        .filter(|b| (b.found)(map_text))
        .map(|b| Finding { id: b.id, what: b.what, fix: b.fix, written_by: saved_by.map(|s| s.build.clone()) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BROKEN: &str = "overbright-map 1\nname = B\n\n[race]\nid = s1\ntrack = 4001\nmode = sprint\nlaps = 3\n";

    fn by(build: &str) -> SavedBy {
        SavedBy { build: build.into(), at: 1, docs: Default::default() }
    }

    /// Every known bug is found in its own fixture, names the writer when the mod recorded one, and a file that
    /// reads (or that has some other fault) has none.
    #[test]
    fn each_known_bug_is_found_and_names_its_writer() {
        let f = check(BROKEN, Some(&by("Overbright 0.5.0 (build aa)")));
        assert_eq!(f.len(), 1);
        assert_eq!((f[0].id, f[0].written_by.as_deref()), ("laps-on-a-sprint", Some("Overbright 0.5.0 (build aa)")));
        assert!(f[0].sentence().starts_with("Overbright 0.5.0 (build aa) wrote a sprint"), "{}", f[0].sentence());
        assert!(check(BROKEN, None)[0].sentence().starts_with("An older build wrote"));
        let fine = BROKEN.replace("laps = 3", "laps = 1");
        assert!(check(&fine, None).is_empty(), "a file that reads has no known writer bug");
        assert!(check("not a map", None).is_empty(), "no safe repair: not a known bug");
        let ids: std::collections::BTreeSet<_> = KNOWN.iter().map(|b| b.id).collect();
        assert_eq!(ids.len(), KNOWN.len(), "ids are unique");
        assert!(KNOWN.iter().all(|b| !b.what.is_empty() && !b.fix.is_empty()));
    }
}
