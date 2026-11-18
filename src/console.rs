//! Where the game's text output goes, and fatal start errors.
//!
//! On Windows the shipped game is a GUI-subsystem program: started from
//! Explorer it opens no console window, so it has no standard output at
//! all. [`attach`] (called first thing in `main`) then attaches to the
//! console of the program that started it when there is one (a terminal:
//! `game --version`, `--check-game`), so those flags still print. When no
//! one can see the output (no console, nothing redirected), a fatal error
//! is shown in a message box instead ([`fatal`], [`show_if_unseen`]).
//!
//! Everywhere else (Linux, and the Windows console-subsystem builds such as
//! the developer harness) these do nothing beyond printing to stderr.

#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether stderr reaches anyone (a console or a redirect); set by [`attach`].
#[cfg(windows)]
static OUTPUT_SEEN: AtomicBool = AtomicBool::new(true);

/// Attaches to the parent's console when the process has no standard
/// output (Windows GUI subsystem); a no-op elsewhere. Call before anything
/// prints and before any thread starts.
pub fn attach() {
    #[cfg(windows)]
    {
        // SAFETY: plain Win32 calls on the process's own standard handles,
        // before any other thread exists.
        let seen = unsafe { win::attach_parent_console() };
        OUTPUT_SEEN.store(seen, Ordering::Relaxed);
    }
}

/// A fatal start error: printed to stderr, and on Windows also shown in a
/// message box when no one sees stderr.
pub fn fatal(msg: &str) {
    eprintln!("{msg}");
    show_if_unseen(msg);
}

/// Shows `msg` in a message box on Windows when stderr reaches no one (a
/// GUI-subsystem start from Explorer or a shortcut); a no-op otherwise.
pub fn show_if_unseen(msg: &str) {
    #[cfg(windows)]
    if !OUTPUT_SEEN.load(Ordering::Relaxed) {
        win::message_box("Overbright", &format!("Overbright could not continue.\n\n{msg}"));
    }
    #[cfg(not(windows))]
    let _ = msg;
}

/// Windows' version and build number (`RtlGetVersion`: the real one, not
/// the compatibility value `GetVersionEx` gives); for the launcher's log.
#[cfg(windows)]
pub fn windows_version() -> Option<(u32, u32, u32)> {
    win::version()
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    type Handle = *mut c_void;
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_WRITE: u32 = 2;
    const OPEN_EXISTING: u32 = 3;
    const MB_OK: u32 = 0;
    const MB_ICONERROR: u32 = 0x10;
    const MB_SETFOREGROUND: u32 = 0x1_0000;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
        fn GetStdHandle(which: u32) -> Handle;
        fn SetStdHandle(which: u32, handle: Handle) -> i32;
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut c_void,
            disposition: u32,
            flags: u32,
            template: Handle,
        ) -> Handle;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(owner: Handle, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }

    fn valid(h: Handle) -> bool {
        !h.is_null() && h as isize != -1
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// True when stderr reaches someone afterwards. Handles that are
    /// already set (a console-subsystem build, or output redirected to a
    /// file or a pipe) are kept as they are.
    pub(super) unsafe fn attach_parent_console() -> bool {
        unsafe {
            if valid(GetStdHandle(STD_ERROR_HANDLE)) || valid(GetStdHandle(STD_OUTPUT_HANDLE)) {
                return true;
            }
            if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
                return false;
            }
            // Attached: make sure stdout/stderr point at the console (std
            // asks for the handle on every write).
            if !valid(GetStdHandle(STD_OUTPUT_HANDLE)) || !valid(GetStdHandle(STD_ERROR_HANDLE)) {
                let name = wide("CONOUT$");
                let out = CreateFileW(
                    name.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_WRITE,
                    std::ptr::null_mut(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                );
                if !valid(out) {
                    return false;
                }
                SetStdHandle(STD_OUTPUT_HANDLE, out);
                SetStdHandle(STD_ERROR_HANDLE, out);
            }
            true
        }
    }

    /// RTL_OSVERSIONINFOW (wdm.h).
    #[repr(C)]
    struct OsVersionInfo {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        service_pack: [u16; 128],
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(info: *mut OsVersionInfo) -> i32;
    }

    pub(super) fn version() -> Option<(u32, u32, u32)> {
        let mut v = OsVersionInfo {
            size: std::mem::size_of::<OsVersionInfo>() as u32,
            major: 0,
            minor: 0,
            build: 0,
            platform: 0,
            service_pack: [0; 128],
        };
        // SAFETY: a documented call filling a structure whose size field is set.
        (unsafe { RtlGetVersion(&mut v) } == 0).then_some((v.major, v.minor, v.build))
    }

    pub(super) fn message_box(caption: &str, text: &str) {
        let (caption, text) = (wide(caption), wide(text));
        // SAFETY: two NUL-terminated UTF-16 strings that outlive the call.
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_OK | MB_ICONERROR | MB_SETFOREGROUND);
        }
    }
}
