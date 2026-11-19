//! The first-run setup (the game folder) and `--verify-cache`.
use crate::*;

/// The first-run setup (QoL, clone deviation): `--set-root <dir>` keeps a
/// game folder for later runs and `--check-game [dir]` reports on one (both
/// then exit: `Ok(false)`); otherwise the game folder is found
/// (`qol::setup::find`: NFSU2_ROOT, the kept folder, the current folder,
/// the usual Steam / Wine / Lutris / Bottles / Heroic places), asked for on
/// a terminal when there is none, checked for the files Overbright needs,
/// and put in NFSU2_ROOT for the rest of the run. Only reads the game
/// folder. Runs before any thread is started.
pub(crate) fn game_folder(command: &super::command::Command) -> Result<bool> {
    use super::command::Command;
    use nfsu2::qol::setup;
    use std::io::IsTerminal;
    let report = |root: &std::path::Path, c: &setup::Check| {
        println!("game folder: {}", root.display());
        for (file, _, _) in setup::KEY_FILES {
            let path = setup::key_file_path(root, file);
            let state = if c.missing.iter().any(|m| m == file) {
                "MISSING".to_string()
            } else {
                let len = std::fs::metadata(&path).map_or(0, |m| m.len());
                let hash = if len < 16 << 20 {
                    std::fs::read(&path)
                        .map_or(String::new(), |b| format!(" xxh3 {:016x}", xxhash_rust::xxh3::xxh3_64(&b)))
                } else {
                    String::new()
                };
                format!(
                    "{len} bytes{hash}{}",
                    if c.different.iter().any(|d| d == file) { " (differs from the known release)" } else { "" }
                )
            };
            println!("  {file}: {state}");
        }
        if c.ok() && !c.different.is_empty() {
            println!(
                "Some files differ from the known release (another version or a modded install); Overbright may still run."
            );
        }
    };
    match command {
        Command::SetRoot(dir) => {
            let dir = std::path::PathBuf::from(dir.as_ref().context("--set-root <game folder>")?);
            let dir = nfsu2::platform::paths::absolute(&dir);
            let c = setup::check(&dir, true);
            report(&dir, &c);
            anyhow::ensure!(c.ok(), "{}", setup::help(Some(&dir), Some(&c)));
            nfsu2::qol::setup::save_root(&dir)?;
            println!("kept in {}", setup::saved_root_file().display());
            return Ok(false);
        }
        Command::CheckGame(dir) => {
            let dir = dir
                .as_ref()
                .map(std::path::PathBuf::from)
                .or_else(nfsu2::qol::setup::find)
                .context(setup::help(None, None))?;
            let c = setup::check(&dir, true);
            report(&dir, &c);
            // SHA-256 of the key files and the edition verdict.
            let fp = nfsu2::fingerprint::compute(&dir);
            print!("{}", fp.report());
            // L1: the same plain-words summary as the launcher's check screen.
            println!("summary:");
            for (mark, line) in setup::check_lines(&c, Some(&fp)) {
                let tag = match mark {
                    setup::Mark::Ok => "ok",
                    setup::Mark::Warn => "warning",
                    setup::Mark::Info => "note",
                    setup::Mark::Error => "ERROR",
                };
                println!("  {tag}: {}", nfsu2::qol::crash::scrub(&line));
            }
            // Every movie and menu file the game will look for.
            println!("game files the game looks for:");
            for line in setup::check_details(&c) {
                println!("  {line}");
            }
            return Ok(false);
        }
        _ => {}
    }
    let mut root = nfsu2::qol::setup::find();
    // No terminal prompt on Windows (the attached console belongs to the shell).
    if root.is_none() && !cfg!(windows) && std::io::stdin().is_terminal() {
        print!("{}\nGame folder: ", setup::help(None, None));
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        let dir = std::path::PathBuf::from(line.trim());
        if !line.trim().is_empty() && setup::check(&dir, false).ok() {
            nfsu2::qol::setup::save_root(&dir)?;
            println!("kept in {}", setup::saved_root_file().display());
        }
        root = Some(dir);
    }
    let Some(root) = root else { anyhow::bail!("{}", setup::help(None, None)) };
    // Sizes on every start; the hashes too when the folder was found
    // rather than given (a first run).
    let given = std::env::var_os("NFSU2_ROOT").is_some();
    let c = setup::check(&root, !given);
    anyhow::ensure!(c.ok(), "{}", setup::help(Some(&root), Some(&c)));
    if !c.different.is_empty() {
        log::warn!(
            "game files differing from the known release (another version or modded): {}",
            c.different.join(", ")
        );
    }
    if !given {
        log::info!("game folder {}", root.display());
        // SAFETY: set before any thread is started.
        unsafe { std::env::set_var("NFSU2_ROOT", &root) };
    }
    Ok(true)
}

/// `game --verify-cache [track …]`: the asset cache against a fresh parse
/// of the game's files (every cached track, or those named).
pub(crate) fn verify_cache(names: &[String]) -> Result<()> {
    for line in nfsu2::cache::verify(nfsu2::BUILD_ID, &nfsu2::game_root(), names)? {
        println!("{line}");
    }
    Ok(())
}
