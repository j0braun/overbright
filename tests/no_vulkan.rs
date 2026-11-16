//! Without a Vulkan driver the game's hardware check (`game --probe`, what
//! the launcher shows) says so in plain words with the command that
//! installs one (`platform::vulkan`), and does not crash. The loader is made
//! to find no driver (`VK_ICD_FILENAMES` / `VK_DRIVER_FILES` naming a file
//! that does not exist); no display, no game files.

mod common;

#[test]
#[cfg(target_os = "linux")]
fn probe_without_a_vulkan_driver_names_the_package() {
    let tmp = common::Scratch::new("no-vulkan");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_game"))
        .arg("--probe")
        .env("VK_ICD_FILENAMES", "/nonexistent/no-driver.json")
        .env("VK_DRIVER_FILES", "/nonexistent/no-driver.json")
        .env("NFSU2_DATA_DIR", tmp.path())
        .env("NFSU2_CACHE_DIR", common::child_cache_dir())
        .env_remove("WGPU_BACKEND")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .output()
        .expect("run game --probe");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "exit code 1 without an adapter:\n{stdout}\n{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    let error =
        stdout.lines().find_map(|l| l.strip_prefix("error=")).unwrap_or_else(|| panic!("no error= line:\n{stdout}"));
    // The loader is installed on a machine that builds the game (the
    // driver is what is missing), or it is not (then that is said).
    assert!(
        error.starts_with("no Vulkan driver for this graphics card was found")
            || error.starts_with("the Vulkan loader (libvulkan.so.1) is not installed"),
        "{error}"
    );
    assert!(error.contains("Overbright draws with Vulkan") && error.contains("then start again"), "{error}");
    use nfsu2::platform::vulkan::{Family, family, install_command};
    let os = std::fs::read_to_string("/etc/os-release").ok();
    let (command, _) = install_command(os.as_deref().map(family).unwrap_or(Family::Other));
    let install = command.unwrap_or("install your distribution's Vulkan loader");
    assert!(error.contains(install), "{install:?} not in {error}");
    assert!(stdout.lines().any(|l| l == "ok=0"), "{stdout}");
}
