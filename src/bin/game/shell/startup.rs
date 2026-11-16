//! The process's start: the allocator, glibc's malloc tuning, the crash
//! hook, the headless modes' data folders, then the command line.
use crate::*;
use super::command::{Command, TempData};

/// The game's allocator (`nfsu2::allocator`): mimalloc fast or lean, or
/// the system allocator, picked by the machine or NFSU2_ALLOC at the first
/// allocation. Without the default `mimalloc` feature, the system's.
#[cfg(feature = "mimalloc")]
#[global_allocator]
static ALLOCATOR: nfsu2::allocator::GameAlloc = nfsu2::allocator::GameAlloc;

/// glibc's malloc with transparent huge pages (`glibc.malloc.hugetlb=1`):
/// its heaps then grow in 2 MB steps backed by huge pages, where 4 KB
/// steps and faults stalled the loaders' threads (a rival's car load
/// ~450–580 ms instead of ~10–20; a race load 630–860 ms against 270–440,
/// at +15–20 MB peak; mimalloc: 470–550 ms at +425 MB). A tunable is read
/// only at the process's start, so the game starts itself again once with
/// it set (unless GLIBC_TUNABLES already names it, or NFSU2_NO_MALLOC_THP
/// is set). Nothing else changes; without THP (`never`) glibc ignores it.
/// Kept with mimalloc too, where glibc serves only the C libraries
/// (Vulkan driver, audio): race loads there 513–538 against 544–832 ms
/// and process CPU −0.1 ms a frame, peak ±30 MB (2026-10-31, noisy), for
/// one exec at start; for the system mode (small machines) it is the
/// measured fast path.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn malloc_with_huge_pages() {
    use std::os::unix::process::CommandExt;
    const TUNABLE: &str = "glibc.malloc.hugetlb";
    let current = std::env::var("GLIBC_TUNABLES").unwrap_or_default();
    if current.contains(TUNABLE) || debug_env().no_malloc_thp {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let mut args = std::env::args_os();
    let arg0 = args.next().unwrap_or_else(|| exe.clone().into_os_string());
    let tunables = if current.is_empty() { format!("{TUNABLE}=1") } else { format!("{current}:{TUNABLE}=1") };
    // exec returns only on failure: then the game runs on as it is.
    let _ = std::process::Command::new(&exe).arg0(arg0).args(args).env("GLIBC_TUNABLES", tunables).exec();
}

/// How long the window's exit waits for the cache writes in flight.
const EXIT_FLUSH: std::time::Duration = std::time::Duration::from_millis(1000);

pub(crate) fn main() -> Result<()> {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    malloc_with_huge_pages();
    // The start's clock for the log's waits (`qol::waits`).
    nfsu2::qol::waits::started();
    // The loaders parse on many threads; glibc would give each its own
    // malloc arena, and the parsed data freed later leaves them
    // fragmented (hundreds of MB over a long session). A few arenas do.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        unsafe extern "C" {
            fn mallopt(param: i32, value: i32) -> i32;
        }
        const M_ARENA_MAX: i32 = -8;
        // SAFETY: a plain glibc call, before any thread is started.
        unsafe { mallopt(M_ARENA_MAX, 4) };
    }
    // Windows GUI build: the starting terminal's console, if any.
    nfsu2::console::attach();
    // env_logger as before, with the crash report's log ring and panic
    // hook (QoL, clone deviation: reports are written only when the
    // Enhancements page's Crash Reports is on).
    nfsu2::qol::crash::install();
    #[cfg(feature = "mimalloc")]
    log::info!("allocator: {}", nfsu2::allocator::describe());
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = Command::parse(&args);
    if command == Command::Version {
        println!("{}", nfsu2::build_info::version_line());
        return Ok(());
    }
    if command == Command::Help {
        println!("{}", dev_hooks::USAGE);
        return Ok(());
    }
    // `--probe`: the launcher's hardware check (no window, no game files;
    // `shell::probe`).
    if command == Command::Probe {
        std::process::exit(super::probe::run());
    }
    // A flag the game does not know (the developer harness's, in a build
    // without it): the usage, exit code 2.
    if let Command::Unknown(flag) = &command {
        eprintln!("game: unknown option {flag}\n{}", dev_hooks::USAGE);
        std::process::exit(2);
    }
    // `--dev-harness`: this build has the developer harness (the checks
    // ask before they test a binary; a shipping build answers with the
    // usage error above).
    #[cfg(feature = "dev-harness")]
    if command == Command::DevHarness {
        println!("dev-harness: on");
        return Ok(());
    }
    // `--shot-list`: every `--shot` target of the check set, `name<TAB>group`
    // (the shot table; scripts/checks/shot_targets.py).
    #[cfg(feature = "dev-harness")]
    if command == Command::ShotList {
        dev::shot::targets::print_list();
        return Ok(());
    }
    // First run: the game folder (QoL; nothing changes when NFSU2_ROOT
    // names a complete install).
    match game_folder(&command) {
        Ok(true) => {}
        Ok(false) => return Ok(()),
        Err(e) => {
            nfsu2::console::fatal(&e.to_string());
            std::process::exit(2);
        }
    }
    // Several seeded playthroughs, each a child process of its own.
    #[cfg(feature = "dev-harness")]
    if let Command::PlaythroughAll(seeds) = &command {
        return playthrough::playthrough_all(seeds.as_deref());
    }
    // The headless modes' data folders (removed at the end).
    let _data = headless_data(&command);
    // --crash-test: a deliberate panic with Crash Reports on (in the data
    // folder's overbright/crashes), to check the report.
    if command == Command::CrashTest {
        nfsu2::qol::Enhancements { crash_reports: true, ..Default::default() }.apply();
        log::warn!("crash test: a log line before the panic");
        panic!("crash test");
    }
    if let Command::VerifyCache(names) = &command {
        return verify_cache(names);
    }
    let result = run(&command, &args);
    // A first run's cache files are finished before the process ends; the
    // window's exit waits at most `EXIT_FLUSH` for them (docs/exit.md: the
    // shipping game took 20-30 s to end on a busy disk; what is left is
    // made again on the next start). The original's quit saves nothing
    // (WM_CLOSE 0x5CCE6C sets 0x864F48, the loop 0x5811D0 ends,
    // 0x5CF970 tears down, exit(0)): docs/exit.md.
    if matches!(command, Command::Window { .. }) {
        let left = nfsu2::cache::flush_within(Some(EXIT_FLUSH));
        if left > 0 {
            log::info!("exit: {left} cache write(s) left for the next start");
        }
    } else {
        nfsu2::cache::flush();
    }
    if let Err(e) = &result {
        nfsu2::console::show_if_unseen(&format!("{e:#}"));
    }
    result
}

/// The data folder a headless mode works in (`XDG_DATA_HOME`), before any
/// thread is started; the one it made for itself is removed when the
/// returned guard drops.
#[cfg_attr(not(feature = "dev-harness"), allow(unused_variables))]
fn headless_data(command: &Command) -> TempData {
    #[cfg_attr(not(feature = "dev-harness"), allow(unused_mut))]
    let mut made = TempData(None);
    // The self-test keeps its profile and options in a directory of its
    // own, so runs side by side don't share (and overwrite) one save.
    // Screenshots likewise: a shot may start careers, play and save, and
    // must never touch the player's own save or options.
    #[cfg(feature = "dev-harness")]
    if matches!(command, Command::Shot(_)) && !debug_env().shot_real_save {
        // NFSU2_SHOT_DATA=<dir>: a prepared folder instead (e.g. a copy
        // holding a profile, for the Profile Manager's lists).
        let given = debug_env().shot_data.clone();
        let dir = given
            .clone()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join(format!("nfsu2-shot-{}", nfsu2::unique_tag())));
        if given.is_none() {
            made.0 = Some(dir.clone());
        }
        // SAFETY: set before any thread is started.
        unsafe { std::env::set_var("XDG_DATA_HOME", &dir) };
    }
    // The flows too: a folder of their own unless NFSU2_FLOW_DATA names
    // one (a copy of the profiles to replay); never the player's own.
    #[cfg(feature = "dev-harness")]
    if *command == Command::Playthrough {
        let dir = std::env::temp_dir().join(format!("nfsu2-playthrough-{}", nfsu2::unique_tag()));
        let _ = std::fs::remove_dir_all(&dir);
        made.0 = Some(dir.clone());
        // SAFETY: set before any thread is started.
        unsafe { std::env::set_var("XDG_DATA_HOME", &dir) };
    }
    #[cfg(feature = "dev-harness")]
    if matches!(command, Command::Flow(_)) {
        let given = debug_env().flow_data.clone();
        let dir = given
            .clone()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join(format!("nfsu2-flow-{}", nfsu2::unique_tag())));
        if given.is_none() {
            made.0 = Some(dir.clone());
        }
        // SAFETY: set before any thread is started.
        unsafe { std::env::set_var("XDG_DATA_HOME", &dir) };
    }
    #[cfg(feature = "dev-harness")]
    if matches!(command, Command::Selftest | Command::SelftestShowcase | Command::Bench(..) | Command::Photo(..)) {
        // A bench may take `--flow`'s NFSU2_FLOW_DATA (a copy of a save to
        // load with NFSU2_FLOW_LOAD).
        let flow_data = matches!(command, Command::Bench(..))
            .then(|| debug_env().flow_data.clone())
            .flatten();
        let given = flow_data.is_some();
        let dir = flow_data
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join(format!("nfsu2-selftest-{}", nfsu2::unique_tag())));
        if !given {
            made.0 = Some(dir.clone());
        }
        // SAFETY: set before any thread is started.
        unsafe { std::env::set_var("XDG_DATA_HOME", &dir) };
    }
    made
}
