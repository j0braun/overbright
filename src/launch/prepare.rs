//! `game --prepare`'s progress lines (docs/developing/prepare.md), as the launcher
//! reads them: `step=<n>/<of> <what>`, then `ok=1`, or `error=<why>` /
//! `ok=0`. The launcher's side of the run is `launcher::prepare`.

/// One line of the prepare run's output that the launcher shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// `step=<n>/<of> <what>`: step `n` (from 1) of `of` has started.
    Step { n: u32, of: u32, what: String },
    /// `ok=1`: all done.
    Done,
    /// `error=<why>`, or `ok=0`.
    Failed(String),
}

/// Reads one output line (`key=value`); None for anything else (the
/// game's own log lines, keys this launcher does not show).
pub fn parse_line(line: &str) -> Option<Line> {
    let (k, v) = line.trim().split_once('=')?;
    match k {
        "step" => {
            let (count, what) = v.split_once(' ').unwrap_or((v, ""));
            let (n, of) = count.split_once('/')?;
            let (n, of): (u32, u32) = (n.parse().ok()?, of.parse().ok()?);
            (n >= 1 && n <= of).then(|| Line::Step { n, of, what: what.trim().to_string() })
        }
        "ok" if v == "1" => Some(Line::Done),
        "ok" => Some(Line::Failed("the game could not finish preparing".into())),
        "error" => Some(Line::Failed(v.to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_read_and_others_skipped() {
        assert_eq!(parse_line("prepare=1"), None);
        assert_eq!(parse_line("adapter=Some GPU (Vulkan)"), None);
        assert_eq!(parse_line("[time] World::load: 3.0 ms"), None);
        assert_eq!(
            parse_line("step=2/5 shader programs"),
            Some(Line::Step { n: 2, of: 5, what: "shader programs".into() })
        );
        assert_eq!(parse_line("step=5/5"), Some(Line::Step { n: 5, of: 5, what: String::new() }));
        assert_eq!(parse_line("step=6/5 x"), None);
        assert_eq!(parse_line("step=0/5 x"), None);
        assert_eq!(parse_line("step=a/b x"), None);
        assert_eq!(parse_line("ok=1\r"), Some(Line::Done));
        assert!(matches!(parse_line("ok=0"), Some(Line::Failed(_))));
        assert_eq!(parse_line("error=no GPU adapter"), Some(Line::Failed("no GPU adapter".into())));
    }
}
