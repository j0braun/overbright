//! What the player is told when no graphics adapter is found on Linux.
//!
//! On Linux the game and its hardware check (`game --probe`) draw only with
//! Vulkan (wgpu's primary backends; there is no OpenGL fallback), so "no
//! adapter" means one of two things: the Vulkan loader (`libvulkan.so.1`)
//! is not installed, or it is but no Vulkan driver for the graphics card is.
//! [`no_adapter_line`] says which, in plain words, with the packages to
//! install for the player's distribution (read from `/etc/os-release`).
//! The game prints it when its window gets no surface or adapter, and the
//! launcher shows it on its Graphics card line (the probe's `error=`).
//! docs/developing/building-releases.md, "Linux: what a player needs".

/// A distribution family, from `/etc/os-release` (`ID`, then `ID_LIKE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Debian, Ubuntu, Linux Mint, Pop!_OS, … (apt).
    Debian,
    /// Fedora, RHEL, AlmaLinux, Rocky, CentOS (dnf).
    Fedora,
    /// Arch, SteamOS, Manjaro, EndeavourOS, CachyOS, … (pacman).
    Arch,
    /// openSUSE Tumbleweed and Leap, SLE (zypper).
    Suse,
    /// Anything else, or no os-release.
    Other,
}

/// The family of an os-release text: `ID` first, then each `ID_LIKE` word.
pub fn family(os_release: &str) -> Family {
    let value = |key: &str| -> String {
        os_release
            .lines()
            .find_map(|l| l.trim().strip_prefix(key)?.strip_prefix('='))
            .map(|v| v.trim().trim_matches(|c| c == '"' || c == '\'').to_ascii_lowercase())
            .unwrap_or_default()
    };
    let id = value("ID");
    let like = value("ID_LIKE");
    for word in std::iter::once(id.as_str()).chain(like.split_whitespace()) {
        let f = match word {
            "debian" | "ubuntu" | "linuxmint" | "pop" | "elementary" | "zorin" | "raspbian" | "kali" => Family::Debian,
            "fedora" | "rhel" | "centos" | "almalinux" | "rocky" | "nobara" => Family::Fedora,
            "arch" | "steamos" | "holo" | "manjaro" | "endeavouros" | "cachyos" | "garuda" => Family::Arch,
            "opensuse" | "opensuse-tumbleweed" | "opensuse-leap" | "suse" | "sles" => Family::Suse,
            _ => continue,
        };
        return f;
    }
    Family::Other
}

/// The command that installs the Vulkan loader and Mesa's Vulkan drivers
/// (None: no command known for the family), and a note for other cards.
pub fn install_command(f: Family) -> (Option<&'static str>, Option<&'static str>) {
    match f {
        Family::Debian => (Some("sudo apt install libvulkan1 mesa-vulkan-drivers"), None),
        Family::Fedora => (Some("sudo dnf install vulkan-loader mesa-vulkan-drivers"), None),
        // Arch splits Mesa's Vulkan drivers by vendor.
        Family::Arch => (Some("sudo pacman -S vulkan-icd-loader vulkan-radeon"), Some("Intel graphics: vulkan-intel")),
        Family::Suse => (Some("sudo zypper install libvulkan1 libvulkan_radeon libvulkan_intel"), None),
        Family::Other => (None, None),
    }
}

/// The player's line for "no adapter": what is missing (`loader`: whether
/// `libvulkan.so.1` could be loaded) and what to install (`os_release`:
/// the text of /etc/os-release, if there is one). One line, no newline.
pub fn line_for(loader: bool, os_release: Option<&str>) -> String {
    let (what, install) = if loader {
        ("no Vulkan driver for this graphics card was found", "install one")
    } else {
        ("the Vulkan loader (libvulkan.so.1) is not installed", "install it and the driver")
    };
    let (command, note) = install_command(os_release.map(family).unwrap_or(Family::Other));
    let how = match command {
        Some(c) => format!("{install} with \"{c}\""),
        None => "install your distribution's Vulkan loader (libvulkan.so.1) and Mesa's Vulkan drivers".into(),
    };
    let note = note.map(|n| format!("{n}; ")).unwrap_or_default();
    format!(
        "{what}. Overbright draws with Vulkan: {how} ({note}Mesa 22 or newer; for an NVIDIA card, NVIDIA's own \
         driver), then start again"
    )
}

/// Whether the Vulkan loader can be loaded (Linux; true elsewhere, where
/// the loader is not what is missing).
pub fn loader_present() -> bool {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // SAFETY: dlopen with a NUL-terminated name; the handle is only
        // closed again (the loader's own constructor is all that runs).
        unsafe {
            let h = libc::dlopen(c"libvulkan.so.1".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if h.is_null() {
                return false;
            }
            libc::dlclose(h);
            true
        }
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        true
    }
}

/// [`line_for`] on this machine: the loader checked, /etc/os-release (or
/// /usr/lib/os-release) read.
pub fn no_adapter_line() -> String {
    let os =
        std::fs::read_to_string("/etc/os-release").or_else(|_| std::fs::read_to_string("/usr/lib/os-release")).ok();
    line_for(loader_present(), os.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_from_os_release() {
        // As the distributions' own files say it (the lines that matter).
        let cases = [
            ("ID=ubuntu\nID_LIKE=debian\n", Family::Debian),
            ("ID=debian\n", Family::Debian),
            ("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n", Family::Debian),
            ("ID=fedora\n", Family::Fedora),
            ("ID=\"almalinux\"\nID_LIKE=\"rhel centos fedora\"\n", Family::Fedora),
            ("ID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\n", Family::Fedora),
            ("ID=arch\n", Family::Arch),
            ("ID=steamos\nID_LIKE=arch\n", Family::Arch),
            ("ID=\"opensuse-tumbleweed\"\nID_LIKE=\"opensuse suse\"\n", Family::Suse),
            ("ID=\"opensuse-leap\"\nID_LIKE=\"suse opensuse\"\n", Family::Suse),
            ("ID=nixos\n", Family::Other),
            ("", Family::Other),
        ];
        for (text, want) in cases {
            assert_eq!(family(text), want, "{text:?}");
        }
        // VERSION_ID and other keys that start like ID are not ID.
        assert_eq!(family("VERSION_ID=\"22.04\"\nID_LIKE=debian\n"), Family::Debian);
    }

    #[test]
    fn the_line_names_what_is_missing_and_the_packages() {
        let ubuntu = "NAME=\"Ubuntu\"\nID=ubuntu\nID_LIKE=debian\n";
        let no_loader = line_for(false, Some(ubuntu));
        assert!(no_loader.starts_with("the Vulkan loader (libvulkan.so.1) is not installed"), "{no_loader}");
        assert!(no_loader.contains("sudo apt install libvulkan1 mesa-vulkan-drivers"), "{no_loader}");
        let no_driver = line_for(true, Some("ID=fedora\n"));
        assert!(no_driver.starts_with("no Vulkan driver for this graphics card was found"), "{no_driver}");
        assert!(no_driver.contains("sudo dnf install vulkan-loader mesa-vulkan-drivers"), "{no_driver}");
        let other = line_for(true, None);
        assert!(other.contains("install your distribution's Vulkan loader (libvulkan.so.1)"), "{other}");
        let arch = line_for(true, Some("ID=steamos\nID_LIKE=arch\n"));
        assert!(
            arch.contains("with \"sudo pacman -S vulkan-icd-loader vulkan-radeon\" (Intel graphics: vulkan-intel;"),
            "{arch}"
        );
        // One line: it goes into the probe's key=value output.
        for l in [no_loader, no_driver] {
            assert!(!l.contains('\n') && l.contains("NVIDIA's own driver") && l.contains("Mesa 22"), "{l}");
        }
    }
}
