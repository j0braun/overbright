# Overbright on Windows: graphics API, shader compiler, pacing, manifest

No native counterpart: SPEED2.EXE is Direct3D 9 only. Everything here is the clone's own platform layer.
Library code: `src/render/core/backend.rs` (the pure decisions, unit-tested on Linux), `src/render/core/gpu.rs`
(`Gpu::new`, `new_instance`), `src/platform/{win,timer,icon}.rs`, `src/render/native/display/present.rs`
(`sleep_margin`), `src/upscale_pack.rs` (`lower_priority`), `build_support/{windows_res,icon_art}.rs`.
**[unverified on real Windows]** unless a Wine result is named; the Wine smoke (docs/checks.md "Wine smoke")
covers what Wine can show.

## Graphics API and adapter

| | Before | Now |
|---|---|---|
| Linux / macOS | `WGPU_BACKEND`, else `Backends::PRIMARY`; `request_adapter(HighPerformance)` | **unchanged** (same expression, same call) |
| Windows | the same: wgpu creates Vulkan, then DX12 instances and sorts all adapters by kind (discrete, integrated, other, virtual, CPU) with a stable sort, so ties went to Vulkan by enumeration order; one adapter, no retry | `WGPU_BACKEND`, else Vulkan + DX12. Every adapter that can present to the window is ranked by kind (the same order as wgpu's), then **Vulkan before DX12**, then enumeration order (`backend::adapter_order`). The device is requested from each in that order until one opens: a Vulkan driver that refuses the device falls back to the same GPU's DX12 (`GPU … did not open: …; trying the next`). |

So a discrete GPU always beats an integrated one whatever the API (a discrete GPU with only a DX12 driver
beats the iGPU's Vulkan; a software Vulkan never beats DX12 hardware), and for one GPU Vulkan is used, as on
Linux. The log lists every candidate (`GPU candidate: name (kind, backend, driver …)`) and the pick
(`GPU adapter: …`, unchanged).

## DX12 shader compiler

`backend::dx12_compiler`, logged once per instance:

1. `WGPU_DX12_COMPILER` (`dxc`, `fxc`, `staticdxc`, `auto`) when set: wgpu's own reading.
2. `dxcompiler.dll` **next to the exe** that loads and exports `DxcCreateInstance` (checked with
   `LoadLibraryExW(LOAD_WITH_ALTERED_SEARCH_PATH)` + `GetProcAddress`, then freed): `Dx12Compiler::DynamicDxc`
   with that full path (`DX12 shader compiler: DXC (…)`). The probe matters: with an explicit DXC path that
   fails to load, wgpu-hal 30 fails the whole DX12 instance (`instance.rs`, "Failed to load dynamic DXC")
   instead of falling back.
3. Else FXC (`d3dcompiler_47.dll`, part of Windows), with one warning: `DX12 shader compiler: FXC, the last
   resort (no dxcompiler.dll in <dir> | <dll> does not load: …); slower pipeline builds and shader model 5.1
   (no multiview cube)`. Before, `Auto` searched `dxcompiler.dll` on the DLL search path (current folder,
   PATH) and fell back to FXC silently.

`dxil.dll` is not required: DXC 1.8.2502 and later sign DXIL with their own validator (release notes: the
validator hash was open-sourced, #6846; 1.9 always uses the internal validator). An older DXC finds
`dxil.dll` next to itself (the altered search path). wgpu 30's minimum is DXC 1.8.2502.

**Where a packager gets DXC** (nothing is downloaded or bundled by the tree; the maintainer decides about
shipping): the DirectX Shader Compiler releases, <https://github.com/microsoft/DirectXShaderCompiler/releases>
(`dxc_<date>.zip`, `bin/x64/dxcompiler.dll` and `dxil.dll`), or the NuGet package `Microsoft.Direct3D.DXC`.
Licence: the release package's README lists `LICENSE-LLVM.txt` (University of Illinois/NCSA Open Source
License, GPL-compatible) for all files but `d3d12shader.h` (MIT) as of 1.8.2502 [read from nuget.org's
package page, 2026-11-12]; older releases put `dxil.dll` under `LICENSE-MS.txt` (Microsoft's proprietary
redistributable terms). Check the README of the exact zip before shipping, record it in
THIRD-PARTY-NOTICES.md, and put both DLLs next to `game.exe`. The `static-dxc` wgpu feature
(mach-dxcompiler) exists only for MSVC targets; the release build is `windows-gnu`.

## Optional features and their fallbacks

`Gpu::new` asks for `adapter.features() & wanted`, so a missing feature never fails the device request. One
warning names what the device goes without (the adapter lacks it, or `NFSU2_GPU_DROP` dropped it) and the
fallback each uses (`backend::FALLBACKS`, `backend::missing`), e.g. on DX12 under Wine:

`GPU (Dx12) runs without optional features, using fallbacks: multiview (environment cube drawn one face per
pass); pipeline_cache (no driver pipeline cache on disk (more first-use hitches))`

| Feature | DX12 in wgpu-hal 30 (`dx12/adapter.rs`) | Fallback (where it is checked) |
|---|---|---|
| `MULTIVIEW` (six views) | view instancing gives at most **4** views, and needs SM 6.1 (so never with FXC) | cube one face per pass (`world/build/buffers.rs`, `env_cube.rs`) |
| `PIPELINE_CACHE` | **never** (Vulkan only) | no disk cache (`core/pipeline_cache.rs`) |
| `TEXTURE_BINDING_ARRAY` + `PARTIALLY_BOUND_BINDING_ARRAY` | SM ≥ 5.1 and resource binding tier 3 | arrays filled with a stand-in (`world/textures.rs`) |
| `IMMEDIATES`, `DUAL_SOURCE_BLENDING`, `TEXTURE_COMPRESSION_BC`, adapter-specific format features | always | no texture arrays / two-pass smoke / CPU-decoded DXT / MSAA ≤ 4x |
| `EXPERIMENTAL_RAY_QUERY` | SM 6.5 + DXR tier | `rt::wanted` asks only on discrete Vulkan |
| `SUBGROUP` | SM ≥ 6.0 and the driver's `WaveOps` (so never with FXC: shader model 5.1) | the lamps' plain cluster loop, the same light (`registry::subgroup_clusters`, docs/lamps-shadows.md "The cluster loop") |

Wine: the DX12 set's fallbacks (multiview, pipeline cache) run without a panic on Vulkan with
`NFSU2_GPU_DROP=multiview,pipeline_cache`, and the image equals Linux's (smoke step `vk_fallback`, 0 pixels).
Under Wine's vkd3d with FXC the device also goes without `SUBGROUP` (the smoke's `dx12_main` log: `... subgroup
(the lamps' light-cluster loop per pixel (the same light))`), so the lamps' module takes the plain loop there;
that fallback draws the same pictures as the subgroup form (`tests::first_use::the_cluster_loop_forms_draw_the_same_and_are_ready`
with `NFSU2_GPU_DROP=subgroup`; the smoke's `explore` is Original, which has no lamps' module).
The DX12 device itself opens under Wine's vkd3d and logs the line above, but **nothing renders**: Wine's
FXC (d3dcompiler_47 on vkd3d-shader) rejects every shader with a sampler (`E5017: … not yet implemented
feature: Shader model 5.1+ resource array`; wgpu 30's DX12 backend binds samplers through an SM 5.1 sampler
heap array), and a shader compile error is fatal in wgpu (a panic). Real Windows FXC supports SM 5.1 resource
arrays; DXC avoids FXC altogether. So the DX12 path is **unverified** until it runs with DXC (under Wine:
`WINE_DXC_DIR`) or on a real machine.

## Frame pacing

- `display::wait_until` sleeps to within `sleep_margin` of the deadline and spins the rest. The margin is
  1 ms on Linux / macOS (unchanged) and **2 ms on Windows**: Windows sleeps wake up to about a millisecond
  late even with a 1 ms timer resolution or std's high-resolution waitable timer (Windows 10 1803+) **[gen]**.
  The cost is at most 2 ms of spinning per waited frame (cut-scenes' 1/30 s, `NFSU2_FRAME_CAP`); uncapped
  frames never wait.
- `platform::timer::TimerResolution::one_ms()`: `timeBeginPeriod(1)` (winmm, on the check_pe allow-list)
  for the guard's lifetime, `timeEndPeriod(1)` on drop; a no-op elsewhere. Held for the whole game run
  (game side: `shell/cli.rs`, see the game-side list). Windows 11 ignores it for a minimised/occluded
  window, which is fine (nothing is paced then). Gameplay is unaffected at any rate: the world runs on its
  fixed 1/60 s cadence (docs/uncapped-fps.md); only the wait's accuracy changes.

## Threads

The texture-pack generator's workers (`upscale_pack::lower_priority`, rayon's start handler): Linux nice 10
(as before), Windows `SetThreadPriority(THREAD_PRIORITY_BELOW_NORMAL)`, so the game's normal-priority
threads always win a core.

## Window

- Icon: `platform::icon::window_icon()` (64×64, the same drawing as the exe's icon resource,
  `build_support/icon_art.rs`); the window has to be built `.with_window_icon(…)` (game side). Without it
  the title bar and Alt+Tab show Windows' default icon (the taskbar may use the exe's resource).
- Alt / F10 menu loop: winit 0.30.13 passes `WM_SYSKEYUP` to `DefWindowProc` only when the window has a
  native menu (`event_loop.rs`, `GetMenu`), so Alt or F10 alone do not enter the modal menu loop (no frozen
  frame); no code needed. Alt+F4: winit hands `WM_SYSKEYDOWN` to `DefWindowProc`, which sends `WM_CLOSE`,
  i.e. `CloseRequested`, which the game already ends on (`window.rs`). Alt+Tab out of exclusive full screen:
  the OS minimises the window; the surface is reconfigured on the size/occlusion events **[unverified]**.

## Application manifest (`build_support/windows_res.rs`, RT_MANIFEST id 1, `game.exe` and `overbright.exe`)

- `supportedOS` Windows 10/11 (`{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}`): no version lies, current behaviour.
- `dpiAware true/pm` + `dpiAwareness PerMonitorV2, PerMonitor`: physical pixels from the first window and
  for every dialog (winit asks for the same at run time; the manifest wins and winit's call becomes a no-op).
- `longPathAware true`: paths past MAX_PATH where the system's LongPathsEnabled policy is on.
  Overbright's own file access does not depend on it: Rust's `std::fs` hands an absolute path that is too
  long to Windows in the `\\?\` form, which has no MAX_PATH limit. Checked under Wine 11 (`wine_smoke.sh`
  `l_list`, `l_hud`, `l_tpf`: a 340-character folder with accents); a real Windows machine with LongPathsEnabled
  off is still to be checked (docs/checks.md "Wine smoke").
- `activeCodePage UTF-8` (Windows 10 1903+): any ANSI API a dependency calls gets UTF-8.

`scripts/release/check_pe.py` now requires the manifest (RT_MANIFEST with PerMonitorV2 and longPathAware);
an exe without the manifest fails that line.
