//! Windows' own folder dialog, which the launcher's folder buttons open
//! first (`model::system_folder_dialog`; the in-app browser is the
//! fallback and the pad's way): `IFileOpenDialog` with `FOS_PICKFOLDERS`. Declared by hand
//! against the documented COM interfaces (ShObjIdl_core.h), as the tree's
//! other Win32 calls are (`platform::win`), so no new dependency.
//! Opened and cancelled by mouse under Wine (Wine's own dialog); not run on
//! Windows, and no automated test opens it.

use std::ffi::{OsString, c_void};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

#[repr(C)]
struct Guid(u32, u16, u16, [u8; 8]);

/// {dc1c5a9c-e88a-4dde-a5a1-60f82a20aef7}
const CLSID_FILE_OPEN_DIALOG: Guid =
    Guid(0xDC1C_5A9C, 0xE88A, 0x4DDE, [0xA5, 0xA1, 0x60, 0xF8, 0x2A, 0x20, 0xAE, 0xF7]);
/// {d57c7288-d4ad-4768-be02-9d969532d960}
const IID_IFILE_OPEN_DIALOG: Guid = Guid(0xD57C_7288, 0xD4AD, 0x4768, [0xBE, 0x02, 0x9D, 0x96, 0x95, 0x32, 0xD9, 0x60]);

const CLSCTX_INPROC_SERVER: u32 = 0x1;
const COINIT_APARTMENTTHREADED: u32 = 0x2;
const FOS_PICKFOLDERS: u32 = 0x20;
const FOS_FORCEFILESYSTEM: u32 = 0x40;
const SIGDN_FILESYSPATH: i32 = 0x8005_8000u32 as i32;

type Hr = i32;

/// IFileOpenDialog's vtable up to GetResult (IUnknown 0–2, IModalWindow
/// 3, IFileDialog 4–26 in declaration order).
#[repr(C)]
struct DialogVtbl {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    show: unsafe extern "system" fn(*mut c_void, *mut c_void) -> Hr,
    set_file_types: usize,
    set_file_type_index: usize,
    get_file_type_index: usize,
    advise: usize,
    unadvise: usize,
    set_options: unsafe extern "system" fn(*mut c_void, u32) -> Hr,
    get_options: unsafe extern "system" fn(*mut c_void, *mut u32) -> Hr,
    set_default_folder: usize,
    set_folder: usize,
    get_folder: usize,
    get_current_selection: usize,
    set_file_name: usize,
    get_file_name: usize,
    set_title: unsafe extern "system" fn(*mut c_void, *const u16) -> Hr,
    set_ok_button_label: usize,
    set_file_name_label: usize,
    get_result: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> Hr,
}

/// IShellItem's vtable up to GetDisplayName.
#[repr(C)]
struct ItemVtbl {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    bind_to_handler: usize,
    get_parent: usize,
    get_display_name: unsafe extern "system" fn(*mut c_void, i32, *mut *mut u16) -> Hr,
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> Hr;
    fn CoUninitialize();
    fn CoCreateInstance(
        clsid: *const Guid,
        outer: *mut c_void,
        ctx: u32,
        iid: *const Guid,
        out: *mut *mut c_void,
    ) -> Hr;
    fn CoTaskMemFree(p: *mut c_void);
}

/// `HRESULT_FROM_WIN32(ERROR_CANCELLED)`: Show's answer to a cancel.
const CANCELLED: Hr = 0x8007_04C7u32 as i32;

/// The dialog: the folder chosen, `Ok(None)` when it was cancelled,
/// `Err` when it could not be made or shown.
pub fn pick_folder(title: &str) -> Result<Option<PathBuf>, ()> {
    // SAFETY: documented COM calls; every interface obtained is released,
    // the returned string freed with CoTaskMemFree, COM uninitialised
    // when this call initialised it.
    unsafe {
        let init = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
        let mut dlg: *mut c_void = std::ptr::null_mut();
        let hr = CoCreateInstance(
            &CLSID_FILE_OPEN_DIALOG,
            std::ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IFILE_OPEN_DIALOG,
            &mut dlg,
        );
        let mut out = Err(());
        if hr >= 0 && !dlg.is_null() {
            let v = &**(dlg as *mut *const DialogVtbl);
            let mut opts = 0u32;
            (v.get_options)(dlg, &mut opts);
            (v.set_options)(dlg, opts | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM);
            let t: Vec<u16> = std::ffi::OsStr::new(title).encode_wide().chain([0]).collect();
            (v.set_title)(dlg, t.as_ptr());
            let shown = (v.show)(dlg, std::ptr::null_mut());
            if shown == CANCELLED {
                out = Ok(None);
            }
            if shown >= 0 {
                let mut item: *mut c_void = std::ptr::null_mut();
                if (v.get_result)(dlg, &mut item) >= 0 && !item.is_null() {
                    let iv = &**(item as *mut *const ItemVtbl);
                    let mut name: *mut u16 = std::ptr::null_mut();
                    if (iv.get_display_name)(item, SIGDN_FILESYSPATH, &mut name) >= 0 && !name.is_null() {
                        let mut n = 0;
                        while *name.add(n) != 0 {
                            n += 1;
                        }
                        out = Ok(Some(PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(name, n)))));
                        CoTaskMemFree(name.cast());
                    }
                    (iv.release)(item);
                }
            }
            (v.release)(dlg);
        }
        if init >= 0 {
            CoUninitialize();
        }
        out
    }
}
