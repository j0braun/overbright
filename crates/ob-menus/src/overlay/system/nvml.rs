//! NVIDIA GPU metrics through NVML (`libnvidia-ml.so.1`, part of the
//! NVIDIA driver), opened at run time with `dlopen` so the game neither
//! links nor needs it. The calls and their structs are NVML's documented
//! C API (nvml.h): `nvmlInit_v2`, `nvmlDeviceGetHandleByIndex_v2`,
//! `nvmlDeviceGetUtilizationRates`, `nvmlDeviceGetTemperature`,
//! `nvmlDeviceGetPowerUsage`, `nvmlDeviceGetClockInfo`,
//! `nvmlDeviceGetMemoryInfo`. Device 0 is read (**inferred**: the one
//! drawn on; a machine with two NVIDIA GPUs may show the other).
//! **Open**: not run on this machine (no NVIDIA GPU); untested.

use std::ffi::{c_char, c_int, c_uint, c_void};

type Handle = *mut c_void;

#[repr(C)]
#[derive(Default)]
struct Utilization {
    gpu: c_uint,
    memory: c_uint,
}

#[repr(C)]
#[derive(Default)]
struct Memory {
    total: u64,
    free: u64,
    used: u64,
}

pub struct Nvml {
    device: Handle,
    utilization: unsafe extern "C" fn(Handle, *mut Utilization) -> c_int,
    temperature: unsafe extern "C" fn(Handle, c_int, *mut c_uint) -> c_int,
    power: unsafe extern "C" fn(Handle, *mut c_uint) -> c_int,
    clock: unsafe extern "C" fn(Handle, c_int, *mut c_uint) -> c_int,
    memory: unsafe extern "C" fn(Handle, *mut Memory) -> c_int,
}

// SAFETY: NVML's calls are thread-safe (nvml.h); the handle is an opaque
// pointer owned by the library, used from the sampler thread alone.
unsafe impl Send for Nvml {}

impl std::fmt::Debug for Nvml {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Nvml")
    }
}

impl Nvml {
    /// The library opened and initialised, device 0's handle; None when
    /// absent or failing (no NVIDIA driver).
    pub fn open() -> Option<Nvml> {
        // SAFETY: dlopen / dlsym with NUL-terminated names; each symbol is
        // cast to its nvml.h signature; the library stays loaded (never
        // dlclosed) so the pointers stay valid.
        unsafe {
            let lib = libc::dlopen(c"libnvidia-ml.so.1".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if lib.is_null() {
                return None;
            }
            let sym = |name: &std::ffi::CStr| -> Option<*mut c_void> {
                let p = libc::dlsym(lib, name.as_ptr() as *const c_char);
                (!p.is_null()).then_some(p)
            };
            let init: unsafe extern "C" fn() -> c_int = std::mem::transmute(sym(c"nvmlInit_v2")?);
            let by_index: unsafe extern "C" fn(c_uint, *mut Handle) -> c_int =
                std::mem::transmute(sym(c"nvmlDeviceGetHandleByIndex_v2")?);
            if init() != 0 {
                return None;
            }
            let mut device: Handle = std::ptr::null_mut();
            if by_index(0, &mut device) != 0 {
                return None;
            }
            Some(Nvml {
                device,
                utilization: std::mem::transmute(sym(c"nvmlDeviceGetUtilizationRates")?),
                temperature: std::mem::transmute(sym(c"nvmlDeviceGetTemperature")?),
                power: std::mem::transmute(sym(c"nvmlDeviceGetPowerUsage")?),
                clock: std::mem::transmute(sym(c"nvmlDeviceGetClockInfo")?),
                memory: std::mem::transmute(sym(c"nvmlDeviceGetMemoryInfo")?),
            })
        }
    }

    pub fn read(&self) -> super::sources::GpuMetrics {
        let mut g = super::sources::GpuMetrics::default();
        // SAFETY: the functions from `open`, with out-pointers to locals.
        unsafe {
            let mut u = Utilization::default();
            if (self.utilization)(self.device, &mut u) == 0 {
                g.busy = Some(u.gpu as f32);
            }
            let mut v: c_uint = 0;
            // NVML_TEMPERATURE_GPU = 0.
            if (self.temperature)(self.device, 0, &mut v) == 0 {
                g.temp_c = Some(v as f32);
            }
            // mW.
            if (self.power)(self.device, &mut v) == 0 {
                g.power_w = Some(v as f32 / 1000.0);
            }
            // NVML_CLOCK_GRAPHICS = 0, NVML_CLOCK_MEM = 2; MHz.
            if (self.clock)(self.device, 0, &mut v) == 0 {
                g.core_mhz = Some(v);
            }
            if (self.clock)(self.device, 2, &mut v) == 0 {
                g.mem_mhz = Some(v);
            }
            let mut m = Memory::default();
            if (self.memory)(self.device, &mut m) == 0 {
                g.vram_used = Some(m.used);
                g.vram_total = Some(m.total);
            }
        }
        g
    }
}
