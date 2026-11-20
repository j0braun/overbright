# Overbright on Windows

What Overbright does differently on Windows: which graphics API it uses, the DirectX 12 shader compiler, frame
pacing, threads, the window and the program manifest. The original game is Direct3D 9 only, so everything here
is Overbright's own.

Unless a Wine result is named, nothing on this page has been checked on a real Windows PC
(**unverified**). The Wine check ([checks.md](../developing/checks.md) "Wine smoke") covers what Wine can show.

## For players

- Overbright uses **Vulkan** on Windows, as on Linux. It tries DirectX 12 only when no Vulkan driver opens
  for your graphics card. If the game shows no picture, update your graphics driver so that Vulkan is
  available.
- A dedicated graphics card is always preferred over integrated graphics.
- The DirectX 12 path is untested. Under Wine it opens but draws nothing (see "DX12 shader compiler").
- The log file lists every graphics card it found and the one it picked.
- When the game crashes inside the graphics driver, its log says what to try:
  [troubleshooting.md](../troubleshooting.md) "A crash in the graphics driver".

## For developers

Code: `src/render/core/backend.rs` (the decisions, as pure functions unit-tested on Linux),
`src/render/core/gpu.rs` (`Gpu::new`, `new_instance`), `crates/ob-core/src/platform/{win,timer,icon}.rs`,
`src/render/native/display/present.rs` (`sleep_margin`), `src/upscale_pack.rs` (`lower_priority`),
`build_support/{windows_res,icon_art}.rs`.

### Graphics API and adapter

| | Before | Now |
|---|---|---|
| Linux / macOS | `WGPU_BACKEND`, else `Backends::PRIMARY`; `request_adapter(HighPerformance)` | **unchanged** (same expression, same call) |
| Windows | the same: wgpu creates Vulkan, then DX12 instances and sorts all adapters by kind (discrete, integrated, other, virtual, CPU) with a stable sort, so ties went to Vulkan by enumeration order; one adapter, no retry | `WGPU_BACKEND`, else Vulkan + DX12. Every adapter that can present to the window is ranked by kind (the same order as wgpu's), then **Vulkan before DX12**, then enumeration order (`backend::adapter_order`). The device is requested from each in that order until one opens: a Vulkan driver that refuses the device falls back to the same GPU's DX12 (`GPU … did not open: …; trying the next`). |

So a discrete GPU always beats an integrated one whatever the API (a discrete GPU with only a DX12 driver beats
the integrated GPU's Vulkan; a software Vulkan never beats DX12 hardware), and for one GPU Vulkan is used, as on
Linux. The log lists every candidate (`GPU candidate: name (kind, backend, driver …)`) and the pick
(`GPU adapter: …`, unchanged).

### DX12 shader compiler

`backend::dx12_compiler` decides, and logs its choice once per instance:

1. `WGPU_DX12_COMPILER` (`dxc`, `fxc`, `staticdxc`, `auto`) when set: wgpu's own reading.
2. A `dxcompiler.dll` **next to the exe** that loads and exports `DxcCreateInstance` (checked with
   `LoadLibraryExW(LOAD_WITH_ALTERED_SEARCH_PATH)` + `GetProcAddress`, then freed): `Dx12Compiler::DynamicDxc`
   with that full path (`DX12 shader compiler: DXC (…)`). The check matters: with an explicit DXC path that
   fails to load, wgpu-hal 30 fails the whole DX12 instance (`instance.rs`, "Failed to load dynamic DXC")
   instead of falling back.
3. Else FXC (`d3dcompiler_47.dll`, part of Windows), with one warning: `DX12 shader compiler: FXC, the last
   resort (no dxcompiler.dll in <dir> | <dll> does not load: …); slower pipeline builds and shader model 5.1
   (no multiview cube)`. Earlier builds let `Auto` search `dxcompiler.dll` on the DLL search path (current
   folder, PATH) and fell back to FXC silently.

`dxil.dll` is not required: DXC 1.8.2502 and later sign DXIL with their own validator (release notes: the
validator hash was open-sourced, #6846; 1.9 always uses the internal validator). An older DXC finds `dxil.dll`
next to itself (the altered search path). wgpu 30's minimum is DXC 1.8.2502.

**Where a packager gets DXC.** Nothing is downloaded or bundled by the repository; shipping it is a release
decision. Sources: the DirectX Shader Compiler releases,
<https://github.com/microsoft/DirectXShaderCompiler/releases> (`dxc_<date>.zip`, `bin/x64/dxcompiler.dll` and
`dxil.dll`), or the NuGet package `Microsoft.Direct3D.DXC`. Licence: the release package's README lists
`LICENSE-LLVM.txt` (University of Illinois/NCSA Open Source License, GPL-compatible) for all files except
`d3d12shader.h` (MIT) as of 1.8.2502 (read from nuget.org's package page); older releases put `dxil.dll` under
`LICENSE-MS.txt` (Microsoft's proprietary redistributable terms). Check the README of the exact zip before
shipping, record it in THIRD-PARTY-NOTICES.md, and put both DLLs next to `game.exe`. The `static-dxc` wgpu
feature (mach-dxcompiler) exists only for MSVC targets; the release build is `windows-gnu`.

### Optional GPU features and their fallbacks

`Gpu::new` asks for `adapter.features() & wanted`, so a missing feature never fails the device request. One
warning names what the device goes without (the adapter lacks it, or `NFSU2_GPU_DROP` dropped it) and the
fallback for each (`backend::FALLBACKS`, `backend::missing`). For example, DX12 under Wine:

`GPU (Dx12) runs without optional features, using fallbacks: multiview (environment cube drawn one face per
pass); pipeline_cache (no driver pipeline cache on disk (more first-use hitches))`

| Feature | DX12 in wgpu-hal 30 (`dx12/adapter.rs`) | Fallback (where it is checked) |
|---|---|---|
| `MULTIVIEW` (six views) | view instancing gives at most **4** views, and needs SM 6.1 (so never with FXC) | cube one face per pass (`world/build/buffers.rs`, `env_cube.rs`) |
| `PIPELINE_CACHE` | **never** (Vulkan only) | no disk cache (`core/pipeline_cache.rs`) |
| `TEXTURE_BINDING_ARRAY` + `PARTIALLY_BOUND_BINDING_ARRAY` | SM ≥ 5.1 and resource binding tier 3 | arrays filled with a stand-in (`world/textures.rs`) |
| `IMMEDIATES`, `DUAL_SOURCE_BLENDING`, `TEXTURE_COMPRESSION_BC`, adapter-specific format features | always | no texture arrays / two-pass smoke / CPU-decoded DXT / MSAA ≤ 4x |
| `EXPERIMENTAL_RAY_QUERY` | SM 6.5 + DXR tier | `rt::wanted` asks only on discrete Vulkan |
| `SUBGROUP` | SM ≥ 6.0 and the driver's `WaveOps` (so never with FXC: shader model 5.1) | the lamps' plain cluster loop, the same light (`registry::subgroup_clusters`, [lamps-shadows.md](settings/lamps-shadows.md) "The cluster loop") |

Checked under Wine:

- The DX12 set's fallbacks (multiview, pipeline cache) run without a panic on Vulkan with
  `NFSU2_GPU_DROP=multiview,pipeline_cache`, and the picture equals Linux's (Wine check step `vk_fallback`,
  0 pixels different).
- Under Wine's vkd3d with FXC the device also goes without `SUBGROUP` (the check's `dx12_main` log: `...
  subgroup (the lamps' light-cluster loop per pixel (the same light))`), so the lamps' module takes the plain
  loop there. That fallback draws the same pictures as the subgroup form
  (`tests::first_use::the_cluster_loop_forms_draw_the_same_and_are_ready` with `NFSU2_GPU_DROP=subgroup`; the
  check's `explore` shot is Original, which has no lamps' module).
- The DX12 device itself opens under Wine's vkd3d and logs the line above, but **nothing renders**. Wine's FXC
  (d3dcompiler_47 on vkd3d-shader) rejects every shader with a sampler (`E5017: … not yet implemented feature:
  Shader model 5.1+ resource array`; wgpu 30's DX12 backend binds samplers through an SM 5.1 sampler heap
  array), and a shader compile error is fatal in wgpu (a panic). Real Windows FXC supports SM 5.1 resource
  arrays, and DXC avoids FXC altogether. So the DX12 path stays **unverified** until it runs with DXC (under
  Wine: `WINE_DXC_DIR`) or on a real machine.

### Frame pacing

- `display::wait_until` sleeps to within `sleep_margin` of the deadline and spins the rest. The margin is 1 ms
  on Linux and macOS (unchanged) and **2 ms on Windows**: Windows sleeps wake up to about a millisecond late,
  even with a 1 ms timer resolution or std's high-resolution waitable timer (Windows 10 1803+) (**general
  knowledge**, not measured here). The cost is at most 2 ms of spinning per waited frame (cut-scenes' 1/30 s,
  `NFSU2_FRAME_CAP`); uncapped frames never wait.
- `platform::timer::TimerResolution::one_ms()` calls `timeBeginPeriod(1)` (winmm, on the check_pe allow-list)
  for the guard's lifetime and `timeEndPeriod(1)` on drop; elsewhere it does nothing. It is held for the whole
  game run (game side: `shell/cli.rs`). Windows 11 ignores it for a minimised or hidden window, which is fine
  (nothing is paced then). Gameplay is the same at any rate: the world runs on its fixed 1/60 s steps
  ([uncapped-fps.md](../original/uncapped-fps.md)); only the wait's accuracy changes.

### Threads

The texture-pack generator's workers (`upscale_pack::lower_priority`, rayon's start handler) run at a lower
priority: nice 10 on Linux (as before), `SetThreadPriority(THREAD_PRIORITY_BELOW_NORMAL)` on Windows. So the
game's normal-priority threads always win a core.

### Window

- **Icon:** `platform::icon::window_icon()` (64 × 64, the same drawing as the exe's icon resource,
  `build_support/icon_art.rs`). The window has to be built with `.with_window_icon(…)` (game side). Without it
  the title bar and Alt+Tab show Windows' default icon (the taskbar may use the exe's resource).
- **Alt / F10:** winit 0.30.13 passes `WM_SYSKEYUP` to `DefWindowProc` only when the window has a native menu
  (`event_loop.rs`, `GetMenu`), so Alt or F10 alone do not enter the modal menu loop (no frozen frame); no
  code needed.
- **Alt+F4:** winit hands `WM_SYSKEYDOWN` to `DefWindowProc`, which sends `WM_CLOSE`, that is
  `CloseRequested`, which the game already ends on (`window.rs`).
- **Alt+Tab out of exclusive full screen:** the system minimises the window; the surface is reconfigured on the
  size and occlusion events (**unverified**).

### Program manifest

`build_support/windows_res.rs` adds a manifest (RT_MANIFEST id 1) to `game.exe`, `overbright.exe`,
`overbright-editor.exe` and `overbright-studio-cli.exe` (with the icon and the version resource):

- `supportedOS` Windows 10/11 (`{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}`): no version lies, current behaviour.
- `dpiAware true/pm` + `dpiAwareness PerMonitorV2, PerMonitor`: physical pixels from the first window and for
  every dialog (winit asks for the same at run time; the manifest wins and winit's call does nothing).
- `longPathAware true`: paths longer than MAX_PATH where the system's LongPathsEnabled policy is on.
  Overbright's own file access does not depend on it: Rust's `std::fs` hands an absolute path that is too long
  to Windows in the `\\?\` form, which has no MAX_PATH limit. Checked under Wine 11 (`wine_smoke.sh` steps
  `l_list`, `l_hud`, `l_tpf`: a 340-character folder with accents). A real Windows machine with
  LongPathsEnabled off is still to be checked ([checks.md](../developing/checks.md) "Wine smoke").
- `activeCodePage UTF-8` (Windows 10 1903+): any ANSI API a dependency calls gets UTF-8.

`scripts/release/check_pe.py` requires the manifest (RT_MANIFEST with PerMonitorV2 and longPathAware); an exe
without it fails that line.
