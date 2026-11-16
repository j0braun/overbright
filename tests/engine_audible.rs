//! The player's engine is heard (the "no engine sound" report on the finished-career
//! save, 2026-11-01): `game --flow engine` checks free roam with a fresh
//! career, free roam with the finished-career fixture and a Quick Race on an offline
//! mixer (the CAR event playing, its levels above silence, the output's
//! RMS above −54 dBFS). Headless (NFSU2_HEADLESS_ONLY, no window, no
//! device); ignored without NFSU2_ROOT.
mod common;

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn engine_heard_in_free_roam_and_quick_race() {
    let root = common::game_root();
    let tmp = std::env::temp_dir().join(format!("nfsu2-engine-test-{}", std::process::id()));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_game"))
        .args(["--flow", "engine"])
        .env("NFSU2_ROOT", root)
        .env("NFSU2_HEADLESS_ONLY", "1")
        .env("NFSU2_FLOW_DATA", &tmp)
        .env("NFSU2_CACHE_DIR", common::child_cache_dir())
        .env_remove("NFSU2_AUDIO_WAV")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .output()
        .expect("run the game");
    let _ = std::fs::remove_dir_all(&tmp);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "flow engine failed:\n{stdout}\n{stderr}");
    for what in ["fresh career", "quick race"] {
        assert!(stdout.contains(&format!("engine {what}:")), "{what} measured:\n{stdout}");
    }
    // The finished-career save is the maintainer's (OB_PRIVATE): without the
    // private set the flow reports that step as skipped.
    let career = if cfg!(ob_private) { "engine finished career:" } else { "SKIP: --flow engine, the finished-career step: needs" };
    assert!(stdout.contains(career), "{career}\n{stdout}");
}
