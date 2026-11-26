//! The placeable solids of a region, listed and searched: the prop browser's list in the editor and Studio
//! (their Browser tab) and `props solids` on the command line read the same functions, so the window and the
//! tool show the same solids for the same words.

use std::collections::HashSet;

use nfsu2::studio_api::map::edit::MAX_PICK_RADIUS;
use nfsu2::studio_api::world::{HIDDEN_FLAG, World};

/// A browser filter. The categories other than knock-over go by the
/// solid's name (the game's own naming: XC_CONE, XB_BENCH …); a solid
/// can be in more than one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    KnockOver,
    Barriers,
    Signs,
    StreetFurniture,
    Trees,
    All,
}

impl Filter {
    pub const ALL: [Filter; 6] =
        [Filter::KnockOver, Filter::Barriers, Filter::Signs, Filter::StreetFurniture, Filter::Trees, Filter::All];

    pub fn name(self) -> &'static str {
        match self {
            Filter::KnockOver => "Knock-over",
            Filter::Barriers => "Barriers",
            Filter::Signs => "Signs",
            Filter::StreetFurniture => "Street furniture",
            Filter::Trees => "Trees",
            Filter::All => "All in region",
        }
    }

    /// The name on a narrow browser.
    pub fn short(self) -> &'static str {
        match self {
            Filter::StreetFurniture => "Street",
            Filter::All => "All",
            f => f.name(),
        }
    }

    pub fn words(self) -> &'static [&'static str] {
        match self {
            Filter::Barriers => &["BARRIER", "BARRICADE", "JERSEY", "FENCE", "CONE", "BARREL", "BLOCK"],
            Filter::Signs => &["SIGN", "BILLBOARD", "ARROW"],
            Filter::StreetFurniture => &[
                "BENCH", "HYDRANT", "DUMP", "TRASH", "BIN", "MAIL", "PHONE", "METER", "LAMP", "LIGHT", "POLE", "NEWS",
                "TABLE", "CHAIR", "PLANTER", "BOLLARD",
            ],
            Filter::Trees => &["TREE", "PALM", "BUSH", "PLANT", "SHRUB", "HEDGE"],
            Filter::KnockOver | Filter::All => &[],
        }
    }

    /// Whether `e` belongs to this filter.
    pub fn matches(self, e: &Entry) -> bool {
        match self {
            Filter::All => true,
            Filter::KnockOver => e.knock_over,
            f => {
                let n = e.name.to_ascii_uppercase();
                f.words().iter().any(|w| n.contains(w))
            }
        }
    }
}

/// One placeable solid.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub solid: usize,
    pub name: String,
    pub knock_over: bool,
    /// Shown placements of it in the world (with the mod).
    pub placed: usize,
}

/// The solids a player can place: those the region places with a matrix
/// and small enough to pick, with their counts, by name.
pub fn entries(world: &World) -> Vec<Entry> {
    let knock: HashSet<usize> =
        world.smackables.iter().filter_map(|p| world.placements.get(p.placement)).map(|p| p.solid).collect();
    let mut placed = vec![0usize; world.solids.len()];
    let mut placeable = vec![false; world.solids.len()];
    for p in &world.placements {
        let Some(s) = world.solids.get(p.solid) else { continue };
        if p.flags & 0x0100 != 0 {
            continue;
        }
        let r = (glam::Vec3::from_array(s.bbox_max) - glam::Vec3::from_array(s.bbox_min)).length() * 0.5;
        if r > MAX_PICK_RADIUS || r < 0.05 {
            continue;
        }
        placeable[p.solid] = true;
        if p.flags & HIDDEN_FLAG == 0 {
            placed[p.solid] += 1;
        }
    }
    let mut out: Vec<Entry> = (0..world.solids.len())
        .filter(|&i| placeable[i])
        .map(|i| Entry {
            solid: i,
            name: world.solids[i].name.clone(),
            knock_over: knock.contains(&i),
            placed: placed[i],
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The entries passing `filter` and the search text (any word, case
/// ignored).
pub fn filtered<'a>(all: &'a [Entry], filter: Filter, search: &str) -> Vec<&'a Entry> {
    let words: Vec<String> = search.split_whitespace().map(|w| w.to_ascii_uppercase()).collect();
    all.iter()
        .filter(|e| filter.matches(e))
        .filter(|e| {
            let n = e.name.to_ascii_uppercase();
            words.iter().all(|w| n.contains(w.as_str()))
        })
        .collect()
}

/// Each filter's count of the entries matching `search`.
pub fn counts(all: &[Entry], search: &str) -> Vec<(Filter, usize)> {
    Filter::ALL.iter().map(|&f| (f, filtered(all, f, search).len())).collect()
}

/// The categories `e` is in, by name ("Barriers, Signs"); "Other" when
/// none.
pub fn categories(e: &Entry) -> String {
    let c: Vec<&str> = [Filter::Barriers, Filter::Signs, Filter::StreetFurniture, Filter::Trees]
        .into_iter()
        .filter(|f| f.matches(e))
        .map(Filter::name)
        .collect();
    if c.is_empty() { "Other".into() } else { c.join(", ") }
}

/// The details line's words for `e`.
pub fn details(e: &Entry) -> String {
    format!(
        "{} · {} · {} · {} placed in the region",
        e.name,
        categories(e),
        if e.knock_over { "knock-over" } else { "static" },
        e.placed
    )
}


/// The words of `Filter` as the command line takes them (`knock-over`, `barriers`, `signs`, `street`, `trees`,
/// `all`).
pub fn filter_word(f: Filter) -> &'static str {
    match f {
        Filter::KnockOver => "knock-over",
        Filter::Barriers => "barriers",
        Filter::Signs => "signs",
        Filter::StreetFurniture => "street",
        Filter::Trees => "trees",
        Filter::All => "all",
    }
}

/// The filter a word names ([`filter_word`]).
pub fn filter_from_word(w: &str) -> Option<Filter> {
    Filter::ALL.into_iter().find(|f| filter_word(*f) == w.trim().to_ascii_lowercase())
}

/// The nearest of `candidates` to `text` when it is a near miss (a typo, a missing letter), else None: what
/// "did you mean" offers. Case is ignored; a candidate that contains the text or is contained in it counts.
pub fn did_you_mean<'a>(text: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let t = text.trim().to_ascii_lowercase();
    if t.is_empty() {
        return None;
    }
    let limit = (t.chars().count() / 3).clamp(1, 3);
    candidates
        .into_iter()
        .filter_map(|c| {
            let l = c.to_ascii_lowercase();
            let d = if l.contains(&t) || t.contains(&l) { 0 } else { distance(&t, &l) };
            (d <= limit).then_some((d, c))
        })
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

/// The edit distance of two words: one insert, delete, change or swap of two neighbours is 1.
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=b.len() {
        d[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let same = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + same);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_near_miss_names_the_nearest_word() {
        let words = ["road", "barrier", "sign"];
        assert_eq!(did_you_mean("raod", words), Some("road"));
        assert_eq!(did_you_mean("barier", words), Some("barrier"));
        assert_eq!(did_you_mean("SIGNN", words), Some("sign"));
        assert_eq!(did_you_mean("zebra", words), None);
        assert_eq!(did_you_mean("  ", words), None);
    }

    #[test]
    fn the_command_line_words_name_the_filters() {
        for f in Filter::ALL {
            assert_eq!(filter_from_word(filter_word(f)), Some(f));
        }
        assert_eq!(filter_from_word("Barriers"), Some(Filter::Barriers));
        assert_eq!(filter_from_word("nope"), None);
    }
}
