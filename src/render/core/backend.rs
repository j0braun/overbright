//! Which graphics API and adapter the game runs on, the DX12 shader
//! compiler, and what an adapter lacks (docs/playing/platform-windows.md).
//! No native counterpart: SPEED2.EXE is Direct3D 9 only.
//!
//! The decisions are pure functions of what is available, so they are
//! tested on Linux; [`super::gpu::Gpu`] feeds them what wgpu reports.
//!
//! * Linux / macOS: unchanged (`WGPU_BACKEND`, else wgpu's primary
//!   backends and `request_adapter`).
//! * Windows: Vulkan and DX12 are both enumerated; [`adapter_order`] ranks
//!   every adapter that can present to the window by its kind (discrete
//!   GPU, integrated, unknown, virtual, software) and, within a kind,
//!   Vulkan before DX12 (Vulkan is what the Linux build renders with). The
//!   device is requested from each in that order until one succeeds, so a
//!   broken Vulkan driver falls back to the same GPU's DX12.

use std::path::{Path, PathBuf};

/// One adapter as the policy sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub backend: wgpu::Backend,
    pub device_type: wgpu::DeviceType,
    /// It can present to the window's surface (true without a surface).
    pub presents: bool,
}

/// The instance's backends: `WGPU_BACKEND` when set; else on Windows Vulkan
/// and DX12, elsewhere wgpu's primary set (as before).
pub fn instance_backends(windows: bool, env: Option<wgpu::Backends>) -> wgpu::Backends {
    match env {
        Some(b) => b,
        None if windows => wgpu::Backends::VULKAN | wgpu::Backends::DX12,
        None => wgpu::Backends::PRIMARY,
    }
}

/// An adapter kind's rank, best first: wgpu's own `HighPerformance` order
/// (wgpu-core `instance.rs` `get_order`), so where wgpu would pick a
/// discrete GPU this does too.
fn kind_rank(t: wgpu::DeviceType) -> u8 {
    match t {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::Other => 2,
        wgpu::DeviceType::VirtualGpu => 3,
        wgpu::DeviceType::Cpu => 4,
    }
}

/// A backend's rank within one adapter kind: Vulkan, then DX12, then the
/// rest.
fn backend_rank(b: wgpu::Backend) -> u8 {
    match b {
        wgpu::Backend::Vulkan => 0,
        wgpu::Backend::Dx12 => 1,
        wgpu::Backend::Metal => 2,
        _ => 3,
    }
}

/// The adapters to try, best first (indices into `c`): those that present,
/// by kind, then Vulkan before DX12, then the enumeration's own order.
pub fn adapter_order(c: &[Candidate]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..c.len()).filter(|&i| c[i].presents).collect();
    idx.sort_by_key(|&i| (kind_rank(c[i].device_type), backend_rank(c[i].backend), i));
    idx
}

/// The DX12 shader compiler and the one line that says which and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dx12Choice {
    /// `WGPU_DX12_COMPILER` set: wgpu's own reading of it.
    Env(String),
    /// DXC from this file (next to the game's exe).
    Dxc(PathBuf),
    /// FXC (`d3dcompiler_47.dll`, part of Windows): no usable DXC.
    Fxc(String),
}

impl Dx12Choice {
    pub fn compiler(&self) -> wgpu::Dx12Compiler {
        match self {
            Dx12Choice::Env(_) => wgpu::Dx12Compiler::from_env().unwrap_or_default(),
            Dx12Choice::Dxc(p) => wgpu::Dx12Compiler::DynamicDxc { dxc_path: p.to_string_lossy().into_owned() },
            Dx12Choice::Fxc(_) => wgpu::Dx12Compiler::Fxc,
        }
    }

    pub fn log_line(&self) -> String {
        match self {
            Dx12Choice::Env(v) => format!("DX12 shader compiler: WGPU_DX12_COMPILER={v}"),
            Dx12Choice::Dxc(p) => format!("DX12 shader compiler: DXC ({})", p.display()),
            Dx12Choice::Fxc(why) => format!(
                "DX12 shader compiler: FXC, the last resort ({why}); slower pipeline builds and shader model 5.1 \
                 (no multiview cube), see docs/playing/platform-windows.md"
            ),
        }
    }
}

/// The DX12 compiler: `WGPU_DX12_COMPILER` when set; else DXC when
/// `dxcompiler.dll` sits next to the exe (`exe_dir`) and loads (`loads`:
/// the library and its `DxcCreateInstance`); else FXC, saying why.
/// `dxil.dll` is not required (DXC 1.8.2502 and later sign with their
/// own validator); an older DXC finds it next to itself.
pub fn dx12_compiler(
    env: Option<&str>,
    exe_dir: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
    loads: impl Fn(&Path) -> Result<(), String>,
) -> Dx12Choice {
    if let Some(v) = env.filter(|v| !v.trim().is_empty()) {
        return Dx12Choice::Env(v.to_string());
    }
    let Some(dir) = exe_dir else {
        return Dx12Choice::Fxc("the exe's folder is unknown".into());
    };
    let dll = dir.join("dxcompiler.dll");
    if !exists(&dll) {
        return Dx12Choice::Fxc(format!("no dxcompiler.dll in {}", dir.display()));
    }
    match loads(&dll) {
        Ok(()) => Dx12Choice::Dxc(dll),
        Err(e) => Dx12Choice::Fxc(format!("{} does not load: {e}", dll.display())),
    }
}

/// The optional features the renderer asks for, each with what it does
/// without it (every one is checked where it is used).
pub const FALLBACKS: &[(wgpu::Features, &str, &str)] = &[
    (wgpu::Features::TEXTURE_COMPRESSION_BC, "bc", "DXT textures decoded to RGBA on the CPU"),
    (
        wgpu::Features::IMMEDIATES,
        "immediates",
        "no world texture arrays (one bind group per texture); car draws read their values by instance index",
    ),
    (
        wgpu::Features::TEXTURE_BINDING_ARRAY.union(wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY),
        "binding_array",
        "texture arrays filled with a stand-in instead of left unbound",
    ),
    (wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES, "format_features", "MSAA limited to 4x"),
    (wgpu::Features::MULTIVIEW, "multiview", "environment cube drawn one face per pass"),
    (wgpu::Features::PIPELINE_CACHE, "pipeline_cache", "no driver pipeline cache on disk (more first-use hitches)"),
    (wgpu::Features::DUAL_SOURCE_BLENDING, "dual_source", "half-size smoke composited in two passes"),
    (wgpu::Features::SUBGROUP, "subgroup", "the lamps' light-cluster loop per pixel (the same light)"),
];

/// The fallbacks in use: each [`FALLBACKS`] entry `wanted` holds whose
/// features `got` (the device's) lacks (any of them), plus multiview when
/// the device has fewer than the six views the cube needs (DX12 offers
/// four; 0 when multiview is not on the device).
pub fn missing(wanted: wgpu::Features, got: wgpu::Features, multiview_views: u32) -> Vec<(&'static str, &'static str)> {
    FALLBACKS
        .iter()
        .filter(|(f, name, _)| {
            if *name == "multiview" {
                return multiview_views < 6 || !got.contains(*f);
            }
            wanted.intersects(*f) && !got.contains(*f)
        })
        .map(|&(_, n, what)| (n, what))
        .collect()
}

/// [`missing`] as one log line (None: nothing missing).
pub fn missing_line(backend: wgpu::Backend, m: &[(&str, &str)]) -> Option<String> {
    if m.is_empty() {
        return None;
    }
    let list: Vec<String> = m.iter().map(|(n, w)| format!("{n} ({w})")).collect();
    Some(format!("GPU ({backend:?}) runs without optional features, using fallbacks: {}", list.join("; ")))
}

/// Whether `dll` loads and exports `DxcCreateInstance` (Windows; elsewhere
/// always an error: DX12 does not exist there).
pub fn dxc_loads(dll: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        crate::platform::win::library_exports(dll, "DxcCreateInstance")
    }
    #[cfg(not(windows))]
    {
        let _ = dll;
        Err("not Windows".into())
    }
}

/// The DX12 choice for this process (Windows; reads the environment and
/// the exe's folder).
pub fn dx12_for_this_exe() -> Dx12Choice {
    let env = std::env::var("WGPU_DX12_COMPILER").ok();
    let exe = std::env::current_exe().ok();
    let dir = exe.as_deref().and_then(Path::parent);
    dx12_compiler(env.as_deref(), dir, |p| p.is_file(), dxc_loads)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::{Backend::*, DeviceType::*};

    fn c(backend: wgpu::Backend, device_type: wgpu::DeviceType) -> Candidate {
        Candidate { backend, device_type, presents: true }
    }

    #[test]
    fn backends_linux_unchanged_windows_vulkan_and_dx12() {
        assert_eq!(instance_backends(false, None), wgpu::Backends::PRIMARY);
        assert_eq!(instance_backends(true, None), wgpu::Backends::VULKAN | wgpu::Backends::DX12);
        assert_eq!(instance_backends(true, Some(wgpu::Backends::DX12)), wgpu::Backends::DX12);
        assert_eq!(instance_backends(false, Some(wgpu::Backends::GL)), wgpu::Backends::GL);
    }

    #[test]
    fn vulkan_first_then_dx12_for_the_same_gpu() {
        // wgpu enumerates Vulkan before DX12; the order must not depend on it.
        assert_eq!(adapter_order(&[c(Dx12, DiscreteGpu), c(Vulkan, DiscreteGpu)]), vec![1, 0]);
        assert_eq!(adapter_order(&[c(Vulkan, DiscreteGpu), c(Dx12, DiscreteGpu)]), vec![0, 1]);
    }

    #[test]
    fn the_gpu_kind_beats_the_api() {
        // A discrete GPU without a Vulkan driver (DX12 only) beats the
        // integrated one's Vulkan; a software Vulkan never beats DX12 hardware.
        let o = adapter_order(&[c(Vulkan, IntegratedGpu), c(Dx12, IntegratedGpu), c(Dx12, DiscreteGpu)]);
        assert_eq!(o, vec![2, 0, 1]);
        assert_eq!(adapter_order(&[c(Vulkan, Cpu), c(Dx12, IntegratedGpu)]), vec![1, 0]);
        assert_eq!(adapter_order(&[c(Vulkan, VirtualGpu), c(Dx12, Other)]), vec![1, 0]);
    }

    #[test]
    fn only_adapters_that_present_and_ties_keep_order() {
        let mut v = [c(Vulkan, DiscreteGpu), c(Vulkan, DiscreteGpu), c(Dx12, DiscreteGpu)];
        v[0].presents = false;
        assert_eq!(adapter_order(&v), vec![1, 2]);
        assert!(adapter_order(&[]).is_empty());
        let none = [Candidate { presents: false, ..c(Dx12, Cpu) }];
        assert!(adapter_order(&none).is_empty());
    }

    #[test]
    fn dxc_next_to_the_exe_else_fxc_with_a_reason() {
        let dir = Path::new("C:/Games/Overbright");
        let ok = |_: &Path| Ok(());
        // Present and loading: DXC from that file.
        let d = dx12_compiler(None, Some(dir), |_| true, ok);
        assert_eq!(d, Dx12Choice::Dxc(dir.join("dxcompiler.dll")));
        assert!(matches!(d.compiler(), wgpu::Dx12Compiler::DynamicDxc { dxc_path } if dxc_path.ends_with("dxcompiler.dll")));
        // Missing: FXC, the line says so once.
        let f = dx12_compiler(None, Some(dir), |_| false, ok);
        assert!(matches!(&f, Dx12Choice::Fxc(w) if w.contains("no dxcompiler.dll")));
        assert!(matches!(f.compiler(), wgpu::Dx12Compiler::Fxc));
        assert!(f.log_line().contains("FXC, the last resort"));
        // Present but broken (wrong architecture, truncated): FXC, not a failed DX12 instance.
        let b = dx12_compiler(None, Some(dir), |_| true, |_| Err("bad image".into()));
        assert!(matches!(&b, Dx12Choice::Fxc(w) if w.contains("bad image")));
        // No exe folder: FXC.
        assert!(matches!(dx12_compiler(None, None, |_| true, ok), Dx12Choice::Fxc(_)));
        // The environment wins; an empty value does not count.
        assert_eq!(dx12_compiler(Some("fxc"), Some(dir), |_| true, ok), Dx12Choice::Env("fxc".into()));
        assert!(matches!(dx12_compiler(Some(" "), Some(dir), |_| true, ok), Dx12Choice::Dxc(_)));
    }

    #[test]
    fn missing_features_name_their_fallbacks() {
        let all = FALLBACKS.iter().fold(wgpu::Features::empty(), |a, (f, _, _)| a | *f);
        assert!(missing(all, all, 8).is_empty());
        assert_eq!(missing_line(Vulkan, &missing(all, all, 8)), None);
        // DX12 as wgpu 30 reports it: no pipeline cache, four views.
        let dx12 = all - wgpu::Features::PIPELINE_CACHE - wgpu::Features::MULTIVIEW;
        let m = missing(all, dx12, 4);
        let names: Vec<&str> = m.iter().map(|x| x.0).collect();
        assert_eq!(names, vec!["multiview", "pipeline_cache"]);
        let line = missing_line(Dx12, &m).unwrap();
        assert!(line.contains("Dx12") && line.contains("pipeline_cache (") && line.contains("one face per pass"));
        // Partially bound alone missing counts as the binding-array fallback.
        let m = missing(all, all - wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY, 8);
        assert_eq!(m.iter().map(|x| x.0).collect::<Vec<_>>(), vec!["binding_array"]);
        // Without immediates the line names the world's and the cars' fallbacks.
        let line = missing_line(Dx12, &missing(all, all - wgpu::Features::IMMEDIATES, 8)).unwrap();
        assert!(line.contains("immediates (") && line.contains("car draws"), "{line}");
        // DX12 through FXC (shader model 5.1: wgpu offers no wave operations):
        // the plain cluster loop, named.
        let fxc = dx12 - wgpu::Features::SUBGROUP;
        let names: Vec<&str> = missing(all, fxc, 4).iter().map(|x| x.0).collect();
        assert_eq!(names, vec!["multiview", "pipeline_cache", "subgroup"]);
        // Not asked for: not reported.
        assert!(missing(all - wgpu::Features::DUAL_SOURCE_BLENDING, all - wgpu::Features::DUAL_SOURCE_BLENDING, 8).is_empty());
    }
}
