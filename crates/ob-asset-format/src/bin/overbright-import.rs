//! `overbright-import`: the one-time import of the player's own copy of the game into an Overbright base set
//! (docs/modding/asset-format/import.md §9). A console program: the launcher runs it as a child and reads its
//! `key=value` lines; other lines are for people.
//!
//! ```text
//! overbright-import import [--game <dir>] [--base <dir>] [--force]
//! overbright-import verify [--base <dir>] [--id <id>]
//! overbright-import repair [--game <dir>] [--base <dir>] [--id <id>]
//! overbright-import fingerprint [--game <dir>] [--manifest]
//! ```
//!
//! `--game` defaults to `NFSU2_ROOT`; `--base` to the `base` folder in Overbright's own folder. The game folder is
//! only ever read.

use ob_asset_format::import::{self, Edition, Options, Outcome, Verdict};
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: overbright-import import|verify|repair|fingerprint [--game <dir>] [--base <dir>] [--id <id>] [--force] [--manifest]";

fn main() {
    std::process::exit(run(std::env::args().skip(1).collect()));
}

fn line(l: &str) {
    println!("{l}");
}

fn fail(msg: impl std::fmt::Display) -> i32 {
    println!("error={msg}");
    println!("ok=0");
    1
}

fn run(argv: Vec<String>) -> i32 {
    if argv.first().map(String::as_str) == Some("--version") {
        println!("{} importer", ob_core::build_info::version_line());
        return 0;
    }
    let mut verb = None;
    let (mut game, mut base, mut id) = (None, None, None);
    let (mut force, mut manifest) = (false, false);
    let mut it = argv.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--game" => game = it.next().map(PathBuf::from),
            "--base" => base = it.next().map(PathBuf::from),
            "--id" => id = it.next(),
            "--force" => force = true,
            "--manifest" => manifest = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return 0;
            }
            v if verb.is_none() && !v.starts_with('-') => verb = Some(v.to_string()),
            _ => {
                eprintln!("{USAGE}");
                return 2;
            }
        }
    }
    let game = game.or_else(|| std::env::var_os("NFSU2_ROOT").map(PathBuf::from));
    let base = base.unwrap_or_else(|| ob_core::platform::dirs::app().join("base"));
    let need_game = || game.clone().ok_or("no game folder: pass --game <dir> or set NFSU2_ROOT");
    match verb.as_deref() {
        Some("import") => {
            let game = match need_game() {
                Ok(g) => g,
                Err(e) => return fail(e),
            };
            // A killed run leaves the staging folder and its journal; the next run resumes.
            let opt = Options { force, ..Options::default() };
            match import::import(&game, &base, &opt, &line) {
                Ok(Outcome::Done { .. }) => {
                    line("ok=1");
                    0
                }
                Ok(Outcome::Stopped { units_done, units, .. }) => {
                    fail(format!("stopped after {units_done} of {units} units; run again to resume"))
                }
                Err(r) => fail(r),
            }
        }
        Some("verify") | Some("repair") => {
            let set = match set_dir(&base, id.as_deref()) {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let threads = Options::default().threads;
            if verb.as_deref() == Some("verify") {
                return match import::verify(&set, threads, &line) {
                    Ok(d) if d.is_empty() => {
                        line("ok=1");
                        0
                    }
                    Ok(d) => {
                        for (u, r) in &d {
                            line(&format!("damaged={u} {r}"));
                        }
                        fail(format!(
                            "{} damaged unit(s); `overbright-import repair` rebuilds them from the game folder",
                            d.len()
                        ))
                    }
                    Err(r) => fail(r),
                };
            }
            let game = match need_game() {
                Ok(g) => g,
                Err(e) => return fail(e),
            };
            match import::repair(&set, &game, &Options::default(), &line) {
                Ok(_) => {
                    line("ok=1");
                    0
                }
                Err(r) => fail(r),
            }
        }
        Some("fingerprint") => {
            let game = match need_game() {
                Ok(g) => g,
                Err(e) => return fail(e),
            };
            let mut scan = match import::scan(&game) {
                Ok(s) => s,
                Err(r) => return fail(r),
            };
            if let Err(r) = import::fingerprint(&mut scan, Options::default().threads, &|_| {}) {
                return fail(r);
            }
            if manifest {
                print!("{}", Edition::manifest_of(&scan));
                return 0;
            }
            let v = Edition::lookup(&scan, &import::editions());
            line(&format!("files={} bytes={}", scan.files.len(), scan.total()));
            line(&format!("identity=sha256:{}", import::hex(&scan.identity().unwrap_or_default())));
            match &v {
                Verdict::Known(e) => line(&format!("edition={} known", e.key)),
                Verdict::Custom { differing } => {
                    line("edition=custom custom");
                    for (p, why) in differing {
                        line(&format!("differs={p} {why}"));
                    }
                }
            }
            line("ok=1");
            0
        }
        _ => {
            eprintln!("{USAGE}");
            2
        }
    }
}

/// The base set to check: `--id`, else `active.txt`, else the only one there.
fn set_dir(base: &Path, id: Option<&str>) -> Result<PathBuf, String> {
    if let Some(id) = id {
        return Ok(base.join(id));
    }
    if let Ok(b) = std::fs::read(base.join("active.txt"))
        && let Ok(Some(id)) = ob_asset_format::order::parse_active(&b)
    {
        return Ok(base.join(id));
    }
    let sets: Vec<PathBuf> = std::fs::read_dir(base)
        .map_err(|e| format!("{}: {e}", base.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("import.toml").is_file())
        .collect();
    match sets.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(format!("no base set in {}", base.display())),
        _ => Err(format!("several base sets in {}: pass --id", base.display())),
    }
}
