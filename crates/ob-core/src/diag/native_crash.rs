//! A crash that is no Rust panic, in words (Windows): an access violation
//! inside a graphics driver or an injected overlay, an illegal instruction,
//! a DLL that failed. Windows' unhandled-exception filter writes one line —
//! the exception's code and name, its address and the module it lies in,
//! the thread and what it was doing ([`set_context`]: the game names the
//! shader module or pipeline it was making), and for a graphics driver
//! what to try ([`driver_hint`]) —
//! to the sink [`install`] was given (the game's log, the launcher's log),
//! then lets Windows go on as before (Windows Error Reporting, the
//! Application event the support file reads). Elsewhere nothing is
//! installed: Linux prints the signal itself and the support file reads
//! the logs.
//!
//! Not caught: a stack overflow and an abort (Rust ends those with a fast
//! fail, which skips every filter); Windows still records them in the
//! Application event log (code 0xc0000409), which the support file shows.

use std::sync::OnceLock;

type Sink = Box<dyn Fn(&str) + Send + Sync>;
static SINK: OnceLock<Sink> = OnceLock::new();
/// What the crashing thread was doing, asked on that thread
/// ([`set_context`]).
static CONTEXT: OnceLock<fn() -> Option<String>> = OnceLock::new();

/// A function the crash line asks, on the crashing thread, what that
/// thread was doing (the game: the shader module or pipeline it was
/// making). Once; later calls do nothing.
pub fn set_context(f: fn() -> Option<String>) {
    let _ = CONTEXT.set(f);
}

/// The crashing thread in words: its name and what it was doing.
#[cfg_attr(not(windows), allow(dead_code))]
fn thread_context() -> String {
    let t = std::thread::current();
    let mut s = format!("; on thread {}", t.name().unwrap_or("(no name)"));
    if let Some(doing) = CONTEXT.get().and_then(|f| f()) {
        s += ", ";
        s += &doing;
    }
    s
}

/// The name of a Windows exception or exit code (NTSTATUS), when it is one
/// a player meets.
pub fn status_name(code: u32) -> Option<&'static str> {
    Some(match code {
        0xC000_0005 => "access violation",
        0xC000_001D => "illegal instruction",
        0xC000_0094 => "integer divide by zero",
        0xC000_00FD => "stack overflow",
        0xC000_0135 => "a DLL was not found",
        0xC000_0139 => "an entry point was not found in a DLL",
        0xC000_0142 => "a DLL failed to start",
        0xC000_0409 => "fast fail (an abort, a stack overflow or a security check)",
        0xC000_041D => "an exception in a callback",
        0xC000_0374 => "heap corruption",
        0xC000_0602 => "fail fast exception",
        0x8000_0003 => "breakpoint",
        0xE06D_7363 => "C++ exception",
        _ => return None,
    })
}

/// The graphics driver a module file belongs to, when it is one: a crash
/// inside a driver is the driver's, usually while it compiles a pipeline or
/// presents.
pub fn driver_of(module: &str) -> Option<&'static str> {
    let m = module.to_ascii_lowercase();
    let m = m.rsplit(['\\', '/']).next().unwrap_or(&m);
    Some(match m {
        "amdvlk64.dll" | "amdvlk32.dll" => "the AMD Vulkan driver",
        "amdxc64.dll" | "amdxc32.dll" => "the AMD DirectX 12 driver",
        "nvoglv64.dll" | "nvoglv32.dll" | "nvgpucomp64.dll" => "the NVIDIA Vulkan driver",
        "nvwgf2umx.dll" | "nvwgf2um.dll" => "the NVIDIA DirectX driver",
        "igvk64.dll" | "igvk32.dll" | "igc64.dll" => "the Intel Vulkan driver",
        "igd12umd64.dll" | "igd12um64xel.dll" => "the Intel DirectX 12 driver",
        "vulkan-1.dll" => "the Vulkan loader (or a layer it loaded)",
        _ => return None,
    })
}

/// What the crash line adds for a crash inside a graphics driver
/// (`docs/troubleshooting.md` "A crash in the graphics driver").
pub fn driver_hint(module: &str) -> Option<String> {
    driver_of(module).map(|driver| {
        format!(
            "; inside {driver}: update the graphics driver; if it still happens, set Graphics to Original \
             (docs/troubleshooting.md, \"A crash in the graphics driver\")"
        )
    })
}

/// Installs the filter with this sink (once; later calls do nothing).
pub fn install(sink: impl Fn(&str) + Send + Sync + 'static) {
    if SINK.set(Box::new(sink)).is_err() {
        return;
    }
    #[cfg(windows)]
    // SAFETY: sets the process's top-level filter to a function that only
    // reads the exception record and calls the sink.
    unsafe {
        win::install();
    }
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    #[repr(C)]
    struct ExceptionRecord {
        code: u32,
        flags: u32,
        next: *mut ExceptionRecord,
        address: *mut c_void,
        parameters: u32,
        information: [usize; 15],
    }

    #[repr(C)]
    struct ExceptionPointers {
        record: *mut ExceptionRecord,
        context: *mut c_void,
    }

    type Filter = unsafe extern "system" fn(*mut ExceptionPointers) -> i32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetUnhandledExceptionFilter(filter: Option<Filter>) -> Option<Filter>;
        fn GetModuleHandleExW(flags: u32, name: *const u16, module: *mut *mut c_void) -> i32;
        fn GetModuleFileNameW(module: *mut c_void, name: *mut u16, size: u32) -> u32;
    }

    const FROM_ADDRESS: u32 = 0x4;
    const UNCHANGED_REFCOUNT: u32 = 0x2;
    const CONTINUE_SEARCH: i32 = 0;

    /// The filter installed before ours (0: none), called after ours.
    static PREVIOUS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    pub(super) unsafe fn install() {
        unsafe {
            let previous = SetUnhandledExceptionFilter(Some(filter));
            PREVIOUS.store(previous.map_or(0, |f| f as usize), std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// The module holding `address` and the offset into it.
    unsafe fn module_of(address: *mut c_void) -> Option<(String, usize)> {
        unsafe {
            let mut module = std::ptr::null_mut();
            if GetModuleHandleExW(FROM_ADDRESS | UNCHANGED_REFCOUNT, address as *const u16, &mut module) == 0 {
                return None;
            }
            let mut name = [0u16; 520];
            let n = GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) as usize;
            let path = String::from_utf16_lossy(&name[..n.min(name.len())]);
            let file = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
            Some((file, (address as usize).wrapping_sub(module as usize)))
        }
    }

    unsafe extern "system" fn filter(info: *mut ExceptionPointers) -> i32 {
        unsafe {
            if let Some(record) = info.as_ref().and_then(|i| i.record.as_ref()) {
                let module = module_of(record.address);
                let at = match &module {
                    Some((m, off)) => format!("{m}+0x{off:x}"),
                    None => format!("0x{:x}", record.address as usize),
                };
                let mut line = format!(
                    "native crash: exception 0x{:08x} ({}) at {at}",
                    record.code,
                    super::status_name(record.code).unwrap_or("unknown")
                );
                if record.code == 0xC000_0005 && record.parameters >= 2 {
                    let what = match record.information[0] {
                        0 => "reading",
                        1 => "writing",
                        8 => "executing",
                        _ => "using",
                    };
                    line += &format!(", {what} 0x{:x}", record.information[1]);
                }
                line += &super::thread_context();
                if let Some(hint) = module.as_ref().and_then(|(m, _)| super::driver_hint(m)) {
                    line += &hint;
                }
                if let Some(sink) = super::SINK.get() {
                    sink(&line);
                }
            }
            match PREVIOUS.load(std::sync::atomic::Ordering::Relaxed) {
                0 => CONTINUE_SEARCH,
                // SAFETY: the value is a `Filter` stored by `install`.
                p => std::mem::transmute::<usize, Filter>(p)(info),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn common_codes_have_names() {
        assert_eq!(super::status_name(0xC000_0005), Some("access violation"));
        assert!(super::status_name(0xC000_0409).is_some_and(|n| n.contains("fast fail")));
        assert_eq!(super::status_name(1), None);
    }

    #[test]
    fn a_crash_in_a_graphics_driver_says_what_to_try() {
        assert_eq!(super::driver_of("amdvlk64.dll"), Some("the AMD Vulkan driver"));
        assert_eq!(
            super::driver_of("C:\\Windows\\System32\\DriverStore\\AMDVLK64.DLL"),
            Some("the AMD Vulkan driver")
        );
        assert!(super::driver_of("nvoglv64.dll").is_some_and(|d| d.contains("NVIDIA")));
        assert!(super::driver_of("igvk64.dll").is_some_and(|d| d.contains("Intel")));
        assert!(super::driver_of("amdxc64.dll").is_some_and(|d| d.contains("DirectX 12")));
        assert_eq!(super::driver_of("game.exe"), None);
        assert_eq!(super::driver_of("ntdll.dll"), None);
        // Update the driver, then Graphics Original; no other graphics API
        // is suggested (the DirectX 12 path is untested).
        for module in ["amdvlk64.dll", "amdxc64.dll", "nvwgf2umx.dll", "vulkan-1.dll"] {
            let hint = super::driver_hint(module).unwrap();
            assert!(hint.contains("update the graphics driver") && hint.contains("Original"), "{hint}");
            let lower = hint.to_ascii_lowercase();
            assert!(!lower.contains("dx12") && !lower.contains("wgpu_backend"), "{hint}");
        }
        assert!(super::driver_hint("amdvlk64.dll").unwrap().contains("AMD Vulkan driver"));
        assert_eq!(super::driver_hint("kernelbase.dll"), None);
    }

    #[test]
    fn the_line_names_the_thread_and_what_it_did() {
        fn doing() -> Option<String> {
            Some("making a pipeline (x.rs:1)".into())
        }
        super::set_context(doing);
        let c = std::thread::Builder::new().name("programs".into()).spawn(super::thread_context).unwrap().join().unwrap();
        assert_eq!(c, "; on thread programs, making a pipeline (x.rs:1)");
    }
}
