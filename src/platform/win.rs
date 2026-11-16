//! Windows-only calls (`cfg(windows)`; docs/platform-dirs.md): the Local
//! AppData known folder and the original's install folder in the registry.
//! Declared by hand against the documented signatures (no binding crate;
//! as `allocator.rs`' GlobalMemoryStatusEx).

use std::ffi::{OsString, c_void};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

/// FOLDERID_LocalAppData {F1B32785-6FBA-4FCF-9D55-7B8E7F157091} (KnownFolders.h).
const FOLDERID_LOCAL_APP_DATA: Guid =
    Guid { data1: 0xF1B3_2785, data2: 0x6FBA, data3: 0x4FCF, data4: [0x9D, 0x55, 0x7B, 0x8E, 0x7F, 0x15, 0x70, 0x91] };

#[link(name = "shell32")]
unsafe extern "system" {
    /// HRESULT SHGetKnownFolderPath(REFKNOWNFOLDERID, DWORD, HANDLE, PWSTR*).
    fn SHGetKnownFolderPath(rfid: *const Guid, flags: u32, token: *mut c_void, path: *mut *mut u16) -> i32;
}

#[link(name = "ole32")]
unsafe extern "system" {
    /// void CoTaskMemFree(LPVOID).
    fn CoTaskMemFree(pv: *mut c_void);
}

#[link(name = "advapi32")]
unsafe extern "system" {
    /// LSTATUS RegGetValueW(HKEY, LPCWSTR, LPCWSTR, DWORD, LPDWORD, PVOID, LPDWORD).
    fn RegGetValueW(
        hkey: *mut c_void,
        sub_key: *const u16,
        value: *const u16,
        flags: u32,
        ty: *mut u32,
        data: *mut c_void,
        data_len: *mut u32,
    ) -> i32;
}

/// The Local AppData known folder (`C:\Users\<name>\AppData\Local`), or
/// None when the shell has none.
pub fn local_app_data() -> Option<PathBuf> {
    let mut p: *mut u16 = std::ptr::null_mut();
    // SAFETY: documented call; the returned buffer is freed with
    // CoTaskMemFree whether the call succeeded or not, as the docs ask.
    unsafe {
        let hr = SHGetKnownFolderPath(&FOLDERID_LOCAL_APP_DATA, 0, std::ptr::null_mut(), &mut p);
        let out = if hr >= 0 && !p.is_null() {
            let mut n = 0;
            while *p.add(n) != 0 {
                n += 1;
            }
            Some(PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(p, n))))
        } else {
            None
        };
        CoTaskMemFree(p.cast());
        out.filter(|p| !p.as_os_str().is_empty())
    }
}

/// HKEY_LOCAL_MACHINE / HKEY_CURRENT_USER (predefined handles, sign-extended as in WinReg.h).
const HKEY_LOCAL_MACHINE: isize = 0x8000_0002_u32 as i32 as isize;
const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
const RRF_RT_REG_SZ: u32 = 0x0000_0002;
const RRF_SUBKEY_WOW6464KEY: u32 = 0x0001_0000;
const RRF_SUBKEY_WOW6432KEY: u32 = 0x0002_0000;
const ERROR_SUCCESS: i32 = 0;
const ERROR_MORE_DATA: i32 = 234;

/// The original's key and value: SPEED2.EXE 0x5B7700 opens
/// HKEY_LOCAL_MACHINE `Software\EA Games\Need for Speed Underground 2`
/// (0x7A30F0) and reads `Install Dir` (0x7A30E4).
const KEY: &str = r"Software\EA Games\Need for Speed Underground 2";
const VALUE: &str = "Install Dir";

fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// A REG_SZ value (REG_EXPAND_SZ expanded), or None.
fn reg_string(hkey: isize, flags: u32, key: &str, value: &str) -> Option<OsString> {
    let (key, value) = (wide(key), wide(value));
    let hkey = hkey as *mut c_void;
    let flags = flags | RRF_RT_REG_SZ;
    for _ in 0..3 {
        let mut len = 0u32;
        // SAFETY: a size query (no buffer), as documented.
        let rc = unsafe {
            RegGetValueW(
                hkey,
                key.as_ptr(),
                value.as_ptr(),
                flags,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut len,
            )
        };
        if rc != ERROR_SUCCESS || len < 2 {
            return None;
        }
        let mut buf = vec![0u16; (len as usize).div_ceil(2) + 1];
        let mut len = (buf.len() * 2) as u32;
        // SAFETY: `buf` holds `len` bytes.
        let rc = unsafe {
            RegGetValueW(
                hkey,
                key.as_ptr(),
                value.as_ptr(),
                flags,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &mut len,
            )
        };
        match rc {
            ERROR_SUCCESS => {
                let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                return (n > 0).then(|| OsString::from_wide(&buf[..n]));
            }
            ERROR_MORE_DATA => continue,
            _ => return None,
        }
    }
    None
}

/// The original's `Install Dir` as the registry holds it, in the order
/// tried: HKLM's 32-bit view (where a 32-bit installer's HKLM writes land
/// on 64-bit Windows, `WOW6432Node`), HKLM's 64-bit view, then HKCU.
/// Only the key, the value and HKLM are traced (0x5B7700); which view and
/// whether installers wrote HKCU are unverified, so these are candidates
/// that `qol::setup::find` checks like any other folder.
pub fn registry_install_dirs() -> Vec<PathBuf> {
    [(HKEY_LOCAL_MACHINE, RRF_SUBKEY_WOW6432KEY), (HKEY_LOCAL_MACHINE, RRF_SUBKEY_WOW6464KEY), (HKEY_CURRENT_USER, 0)]
        .into_iter()
        .filter_map(|(hkey, view)| reg_string(hkey, view, KEY, VALUE))
        .map(PathBuf::from)
        .collect()
}

// ---- Windows: DXC probe, thread priority, timer resolution --------------

#[link(name = "kernel32")]
unsafe extern "system" {
    /// HMODULE LoadLibraryExW(LPCWSTR, HANDLE, DWORD).
    fn LoadLibraryExW(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
    /// FARPROC GetProcAddress(HMODULE, LPCSTR).
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    /// BOOL FreeLibrary(HMODULE).
    fn FreeLibrary(module: *mut c_void) -> i32;
    /// DWORD GetLastError(void).
    fn GetLastError() -> u32;
    /// HANDLE GetCurrentThread(void) (a pseudo handle, never closed).
    fn GetCurrentThread() -> *mut c_void;
    /// BOOL SetThreadPriority(HANDLE, int).
    fn SetThreadPriority(thread: *mut c_void, priority: i32) -> i32;
}

#[link(name = "winmm")]
unsafe extern "system" {
    /// MMRESULT timeBeginPeriod(UINT).
    fn timeBeginPeriod(period: u32) -> u32;
    /// MMRESULT timeEndPeriod(UINT).
    fn timeEndPeriod(period: u32) -> u32;
}

/// LOAD_WITH_ALTERED_SEARCH_PATH: an absolute path's own folder is searched
/// for the library's dependencies (an older DXC's `dxil.dll` next to it).
const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;
/// THREAD_PRIORITY_BELOW_NORMAL (WinBase.h).
const THREAD_PRIORITY_BELOW_NORMAL: i32 = -1;

/// Whether the library at `path` loads and exports `symbol` (the library
/// is freed again; wgpu loads it itself).
pub fn library_exports(path: &std::path::Path, symbol: &str) -> Result<(), String> {
    let w: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let sym: Vec<u8> = symbol.bytes().chain(std::iter::once(0)).collect();
    // SAFETY: documented calls with NUL-terminated strings; the module is
    // freed on every path after a successful load.
    unsafe {
        let m = LoadLibraryExW(w.as_ptr(), std::ptr::null_mut(), LOAD_WITH_ALTERED_SEARCH_PATH);
        if m.is_null() {
            return Err(format!("LoadLibrary error {}", GetLastError()));
        }
        let f = GetProcAddress(m, sym.as_ptr());
        let out = if f.is_null() { Err(format!("no {symbol} export")) } else { Ok(()) };
        FreeLibrary(m);
        out
    }
}

/// The calling thread below normal priority (the texture-pack generator's
/// workers; Linux: nice 10).
pub fn lower_thread_priority() {
    // SAFETY: the pseudo handle of the calling thread.
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

/// THREAD_MODE_BACKGROUND_BEGIN (WinBase.h): the calling thread's CPU,
/// I/O and memory priority lowered.
const THREAD_MODE_BACKGROUND_BEGIN: i32 = 0x0001_0000;

/// The calling thread in background mode (the file read-ahead,
/// `cache::read_ahead`): its CPU, disk and memory priority lowered.
pub fn background_thread() {
    // SAFETY: the pseudo handle of the calling thread.
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
    }
}

/// timeBeginPeriod(1): true when granted.
pub fn timer_begin_1ms() -> bool {
    // SAFETY: plain call; TIMERR_NOERROR is 0.
    unsafe { timeBeginPeriod(1) == 0 }
}

/// timeEndPeriod(1), pairing a granted [`timer_begin_1ms`].
pub fn timer_end_1ms() {
    // SAFETY: plain call.
    unsafe {
        timeEndPeriod(1);
    }
}

// ---- Windows: a monitor's display modes --------------------------------

/// RECT (windef.h).
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// MONITORINFOEXW (WinUser.h): 104 bytes.
#[repr(C)]
struct MonitorInfoExW {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
    device: [u16; 32],
}

/// DEVMODEW (wingdi.h), the display form of its unions: 220 bytes.
#[repr(C)]
struct DevModeW {
    device_name: [u16; 32],
    spec_version: u16,
    driver_version: u16,
    size: u16,
    driver_extra: u16,
    fields: u32,
    /// The position / orientation / fixed-output union (16 bytes).
    position: [u32; 4],
    color: i16,
    duplex: i16,
    y_resolution: i16,
    tt_option: i16,
    collate: i16,
    form_name: [u16; 32],
    log_pixels: u16,
    bits_per_pel: u32,
    pels_width: u32,
    pels_height: u32,
    display_flags: u32,
    display_frequency: u32,
    icm_method: u32,
    icm_intent: u32,
    media_type: u32,
    dither_type: u32,
    reserved1: u32,
    reserved2: u32,
    panning_width: u32,
    panning_height: u32,
}

const _: () = assert!(std::mem::size_of::<DevModeW>() == 220 && std::mem::size_of::<MonitorInfoExW>() == 104);

/// DM_BITSPERPEL | DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY (wingdi.h).
const DM_BITSPERPEL: u32 = 0x0004_0000;
const DM_PELSWIDTH: u32 = 0x0008_0000;
const DM_PELSHEIGHT: u32 = 0x0010_0000;
const DM_DISPLAYFREQUENCY: u32 = 0x0040_0000;

#[link(name = "user32")]
unsafe extern "system" {
    /// BOOL GetMonitorInfoW(HMONITOR, LPMONITORINFO).
    fn GetMonitorInfoW(monitor: isize, info: *mut MonitorInfoExW) -> i32;
    /// BOOL EnumDisplaySettingsExW(LPCWSTR, DWORD, DEVMODEW*, DWORD).
    fn EnumDisplaySettingsExW(device: *const u16, mode: u32, devmode: *mut DevModeW, flags: u32) -> i32;
}

/// A monitor as the system reports it, read without winit's monitor calls
/// (which assert that every display mode carries its size, depth and
/// refresh fields, and unwrap GetMonitorInfoW).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MonitorModes {
    /// The device name (`\\.\DISPLAY1`).
    pub name: String,
    /// The desktop rectangle's size.
    pub desktop: (u32, u32),
    /// The sizes of the modes that carry a size (any refresh rate).
    pub sizes: Vec<(u32, u32)>,
    /// Modes listed without one of the size, depth or refresh fields.
    pub incomplete: usize,
}

/// The monitor `hmonitor`'s facts and display modes; None when the
/// system no longer knows the monitor.
pub fn monitor_modes(hmonitor: isize) -> Option<MonitorModes> {
    // SAFETY: zeroed plain-data structures with their size fields set, as
    // the calls require; the device name is the NUL-terminated array the
    // first call filled.
    unsafe {
        let mut info: MonitorInfoExW = std::mem::zeroed();
        info.size = std::mem::size_of::<MonitorInfoExW>() as u32;
        if GetMonitorInfoW(hmonitor, &mut info) == 0 {
            return None;
        }
        let r = info.monitor;
        let end = info.device.iter().position(|&c| c == 0).unwrap_or(info.device.len());
        let mut out = MonitorModes {
            name: String::from_utf16_lossy(&info.device[..end]),
            desktop: ((r.right - r.left).max(0) as u32, (r.bottom - r.top).max(0) as u32),
            ..Default::default()
        };
        const ALL: u32 = DM_BITSPERPEL | DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
        for i in 0..4096 {
            let mut mode: DevModeW = std::mem::zeroed();
            mode.size = std::mem::size_of::<DevModeW>() as u16;
            if EnumDisplaySettingsExW(info.device.as_ptr(), i, &mut mode, 0) == 0 {
                break;
            }
            if mode.fields & ALL != ALL {
                out.incomplete += 1;
            }
            let sized = mode.fields & (DM_PELSWIDTH | DM_PELSHEIGHT) == DM_PELSWIDTH | DM_PELSHEIGHT;
            if sized && mode.pels_width > 0 && mode.pels_height > 0 {
                out.sizes.push((mode.pels_width, mode.pels_height));
            }
        }
        Some(out)
    }
}
