# How releases are built

`scripts/release/package.sh` turns one commit into the two player archives, checks them and writes their
checksums. It writes only to the folder you give it; publishing (a tag, a release page, an upload) is done by
hand afterwards.

```
. scripts/local.env                       # NFSU2_ROOT: the game for `game --check-game` (optional)
SOURCE_URL=https://example.org/overbright \
  scripts/release/package.sh <commit> <out dir>
```

Requirements: everything `scripts/release/container/README.md` lists (podman, git, python3, binutils), plus
`rustc` (the icon), Wine and Xvfb (the Windows checks). Nothing is installed by the script; the only network
use is the container build's `cargo fetch` (a no-op when its crate cache is complete).

The last line is `PACKAGE: ok (...)` (exit 0) or `PACKAGE: FAIL (N checks)` (exit 1). The maintainer's
privacy scan of the archives is not part of this repository: elsewhere the run prints
`SKIP  privacy scan: maintainer tool, not in this checkout`, runs every other check, and when they all pass
ends `PACKAGE: ok, privacy scan not run (...)` with exit status 3. Without `NFSU2_ROOT` the
`game --check-game` step is skipped too (`SKIP` line).

## The build

`scripts/release/container/build.sh` (called by package.sh; BUILDING.md, "The release container") builds both
platforms from `git archive <commit>` in pinned container images: Linux in AlmaLinux 8 (glibc 2.28), Windows
cross-compiled with mingw-w64 in Ubuntu 22.04, Rust from the official archives pinned by version and SHA-256.
The shipped game has no developer harness. `--no-build` reuses `<out>/build` from an earlier run.

## What it makes

| `overbright-<version>-linux-x86_64.tar.gz` | `overbright-<version>-windows-x86_64.zip` |
|---|---|
| `overbright` (the launcher), `game`, `overbright-editor` (the map editor, a preview) | `overbright.exe` (the launcher), `game.exe`, `overbright-editor.exe` |
| `README.txt`, `LICENSE`, `THIRD-PARTY-NOTICES.md` | `README.txt`, `LICENSE.txt`, `THIRD-PARTY-NOTICES.md` (CRLF) |
| `THIRD-PARTY-LICENSES.txt` (crates + Rust std, Linux build) | `THIRD-PARTY-LICENSES.txt` (crates + Rust std + mingw-w64 runtime + libgcc) |
| `overbright.desktop`, `overbright.png` (256 px icon) | (optional, `DXC_DIR`: `dxcompiler.dll`, `dxil.dll`, `LICENSE-DXC.txt`) |

Each archive has one top folder named like the archive. `<version>` is `git describe --tags --always` of the
commit, without the `v` of a tag such as `v0.2.1`: tag the commit first (`git tag v0.2.1 <commit>`) to get a
readable version (`0.2.1`).

- **Not shipped:** the developer tools the build also makes (`car_check`, `inspect`, `world`, `frontend`,
  `dump_textures`, ...: developer tools) and **`overbright-upscale`**. The texture-pack generator is for
  players, but the game runs the same code itself (Graphics → Generate Texture Pack,
  `upscale_pack::generate`); the command line adds only review sheets (`--sheet`) and the UI
  pack, which needs a separate HD UI pack (docs/modding/texture-pack-generator.md, docs/original/loading-image.md). One fewer
  program to explain.
- **The `.desktop` file** is a template: an unpacked archive can live anywhere and a menu entry needs the
  full path, so README.txt says to copy it and set `Exec=`. The icon is drawn by
  `build_support/icon_art.rs` (our own art, the same as the window icon and the Windows `.ico`), rendered by
  `scripts/release/icon_rgba.rs`.
- **README.txt** (`scripts/release/player/README.txt`, one template, `@@linux` / `@@windows` sections): what
  Overbright is, that the player needs their own copy of the game, how to start, where settings, saves, logs
  and the cache live (docs/playing/platform-dirs.md), how to report a problem, not affiliated with EA, the licence
  and where the source code is.
- **THIRD-PARTY-LICENSES.txt** (`scripts/release/crate_licenses.py`): `cargo metadata --filter-platform <target>`
  of the commit (run in the build image, offline, default features = the shipping build) → the
  normal-dependency closure of the packages that hold the player's programs (`nfsu2`: the game and the
  launcher; `ob-editor`: the map editor); each crate's name, version, licence expression and
  whether it is linked or only used at build time (proc-macros), then every distinct licence text from the
  crates' own sources (LICENSE*, COPYING*, NOTICE*, ...; identical texts once, with their users). A crate
  without a file gets its licence's standard text (MIT: with the manifest's authors). The script fails when
  an expression cannot be met with GPL-3.0-or-later-compatible licences, a crate is not from crates.io, or a
  text is missing. Appended from the build image: the Rust standard library's notices
  (`COPYRIGHT-library.html` as text, with the licence texts it names) and, for Windows, the statically linked
  mingw-w64 runtime (Debian's copyright file of mingw-w64) and libgcc (GCC Runtime Library Exception 3.1).

## Version and source URL

`container/build.sh` passes `OB_VERSION`, `OB_COMMIT` and `OB_SOURCE_URL` to build.rs: `game --version`
prints `Overbright <version> (commit <sha>, linux-x86_64)`, the launcher's about screen shows the version
line, and the about screen shows the source URL from `OB_SOURCE_URL` (without one it says the build is unofficial,
and the package check "about screen: source link" fails). `SOURCE_URL` defaults to the literal placeholder `SOURCE_URL`; the script prints a NOTE then. The URL is built
into the binaries: `--no-build` keeps the one of the earlier build, so a changed `SOURCE_URL` needs a new build.

## The checks (each prints `ok` or `FAIL`; any FAIL fails the script)

| check | what it proves |
|---|---|
| `third-party licences <target>` | every crate's licence is GPL-3-compatible and has its text in the archive |
| `third-party licences <target>: the map editor's crates` | the list covers the editor's package too: egui, egui-winit and egui-wgpu are listed as linked |
| `listing <archive>` | the archive holds exactly the expected files (a tool binary, a stray file → FAIL) |
| `zip text files: CRLF` | Notepad-friendly text in the Windows archive |
| `version stamp <binary>` | all six binaries carry the version and commit of the archive's name |
| `check_linux.sh` (both Linux programs) | no x86 ISA note, glibc ≤ 2.28, NEEDED allow-list, no RPATH, no `/home/` |
| `check_pe.py --gui` (both exes) | system DLLs only, GUI subsystem, icon + version + manifest resources, no home paths |
| `game --selftest` / `--dev-harness` | the developer harness is not in the shipping game (usage error, rc 2) |
| `no harness strings` | the harness's flags and variables are absent (both games) |
| `game --version`, `game --check-game $NFSU2_ROOT`, `overbright-editor --version` | the unpacked Linux game and editor run with an empty HOME |
| `overbright --render about` | the launcher draws with an empty HOME, no GPU (`VK_ICD_FILENAMES=/nonexistent`) and no display |
| `nothing written to HOME` | those runs leave the player's home alone |
| Wine: `game.exe --version`, `overbright-editor.exe --version`, `overbright.exe --render about` | the Windows programs start (private prefix in `<out>/wine/`, private Xvfb: `PKG_DISPLAY` or a free one; `WAYLAND_DISPLAY` unset) |

`--plant home-binary|file` proves the checks bite: it plants a home path at the end of `game.exe` or a tool
binary into the Linux folder after assembling, and the run must end in `PACKAGE: FAIL`.

## Checksums

The out dir holds the two archives and `SHA256SUMS` (`sha256sum` format, one line per archive); the run's
`sha256:` line repeats them. Publish the archives together with `SHA256SUMS`; a player checks a download with
`sha256sum -c SHA256SUMS --ignore-missing`. `checks.txt` keeps every check line of the run.

## Windows: the DirectX shader compiler

`dxcompiler.dll` is **not bundled**: without it, DX12 uses FXC (slower pipeline builds, no multiview cube;
Vulkan is preferred anyway). To ship it, put `dxcompiler.dll`, `dxil.dll` (from Microsoft's
DirectXShaderCompiler release, `bin/x64/`) and its licence as `LICENSE-DXC.txt` in a folder **outside the
repository** and run with `DXC_DIR=<that folder>`; the script copies the three files next to
`overbright.exe` / `game.exe` and expects them in the listing. Check that DXC's licence terms allow
redistribution with a GPL program before doing so.

## Reproducibility

The archives are reproducible from the same files: sorted entries, every time stamp = the commit's time,
owner 0/0 without names, fixed modes, no gzip name/time (`scripts/release/archive.py`; re-making both
archives from one stage gave the same SHA-256). The text files, the icon and the about-screen render were
byte-identical across builds.

The binaries: the first two builds of one commit (one on a crate cache filled by an earlier commit, one
from an empty cache) differed, for three reasons, all fixed in `container/inside.sh`:
- mimalloc (C) puts `__DATE__`/`__TIME__` into the binary. GCC takes them from `SOURCE_DATE_EPOCH`, which
  was the commit time, and cargo does not rebuild C code when only that variable changes, so a cached
  build kept the date of the commit that filled the cache. Now a fixed `SOURCE_DATE_EPOCH` (1980-01-01) and
  a matching `-D` in the C flags (cargo watches those, so old caches rebuild once); C paths are mapped too.
- Windows: the linker's time stamp in the PE header (`-Wl,--no-insert-timestamp`).
- Windows: the COFF symbol table holds the names of rustc's raw-dylib import libraries, which are named
  after random temporary folders. The Windows programs are stripped after linking
  (`x86_64-w64-mingw32-strip --strip-all`, deterministic; `-C link-arg=-s` left the table in place): a
  Windows panic backtrace then shows addresses instead of function names (the panic message and its source
  location remain). The Linux programs keep their symbols.

Later, the Linux binaries still differed now and then: same code and data, but about 800 local symbol
names of the game crate (and 50 in the launcher) had another `.llvm.<number>` suffix, and with them the
build-id. The cause, found by rebuilding only the game crate's library in the container:
- ThinLTO gives each local symbol that one module shares with another a global name,
  `<name>.llvm.<module hash>`, where the hash is a digest of the module's bitcode.
- The game crate's bitcode (one codegen unit) comes in two variants. They differ in a single
  instruction: the order of the incoming values of one 12-way `phi` in
  `track_camera::route_director::chains`, a chain of `&&` comparisons. LLVM's jump threading
  pass builds that `phi`. Which LLVM step picks the varying order is open.
  The two orders mean the same thing and give the same machine code, but different bytes, so a different
  hash. Which variant a build gets does not depend on the caches, the out dir, the job count or address
  randomization (it also happened with ASLR off). Repeated runs of one command in one container gave
  both variants, about equally often over 19 compiles. The nondeterminism is inside the compiler, not in
  this project's build.
- Fix (`container/inside.sh`): `-C llvm-args=-use-source-filename-for-promoted-locals`. LLVM then names
  the promoted symbols after the module's source name instead of its hash. For rustc that is the codegen
  unit's name, `<crate>.<crate hash>-cgu.<n>`, which is unique and fixed for a given source and toolchain.
  The code and data of the binaries are unchanged (every section other than the symbol names and the
  build-id is byte-identical to the build without the option), so speed is the same. The cost: the
  symbol names are longer (game +50 KB, launcher +26 KB), and a crash backtrace shows those functions as
  `nfsu2::track_camera::route_director::chains.llvm.nfsu2_74012db766162aff_cgu_0` rather than
  `…::chains` (the demangler drops only a numeric suffix). Backtraces remain symbolized.
- Four symbols keep a numeric suffix. They come from the prebuilt standard library objects
  (`core`, `compiler_builtins`), so they are the same in every build.
- Checked: two builds of one source without the option got the two bitcode variants and two different
  binaries. With the option, these builds all gave the same SHA-256 for every Linux program: from empty
  caches with 4 and 8 jobs under different out dir paths (each variant came up once), on a warm target
  dir with touched sources, and with a warm crate cache and an empty target dir. The Windows programs are
  stripped and were never affected by this. They stay identical with the option.

## Linux: what a player needs

The requirements as README.md and the archive's README.txt state them, and how they were measured.

- **64-bit x86** (x86-64). No newer instruction set: `check_linux.sh` fails a binary with an "x86 ISA
  needed" note.
- **glibc 2.28 or newer.** The Linux programs are linked in an AlmaLinux 8 image
  (`container/Containerfile.linux`, glibc 2.28, the manylinux_2_28 base); `check_linux.sh` fails a binary
  that names a newer glibc symbol version. Before, they were linked on Ubuntu 22.04 (glibc 2.35), which
  shut out Ubuntu 20.04, Debian 11 and RHEL / AlmaLinux / Rocky 8 and 9.
- **Libraries the programs link**: glibc's own (`libc`, `libm`, `libdl`, `libpthread`), `libgcc_s`,
  `libudev.so.1` (systemd) and, for the game, `libasound.so.2` (ALSA). Every desktop install has them; a
  minimal or container image may lack `libasound2` / `alsa-lib` (then the game does not start, the
  launcher does).
- **Libraries loaded at run time**: the Vulkan loader `libvulkan.so.1` and a driver; on X11 `libX11`,
  `libX11-xcb`, `libXcursor`, `libXi`, `libxkbcommon`, `libxkbcommon-x11`; on Wayland
  `libwayland-client` and `libxkbcommon`. The X11 ones measured with `LD_DEBUG=files` on the launcher and
  the game's probe, the Wayland ones read from the programs' strings (no Wayland session was run); a
  desktop has them all.
- **A Vulkan driver**, Mesa 22 or newer for AMD and Intel graphics, or NVIDIA's own driver. The oldest
  driver that drew real frames here is lavapipe from Mesa 22.3 (Debian 12). Lavapipe from Mesa 21.2
  (Ubuntu 20.04) and 20.3 (Debian 11) is found but cannot draw: wgpu reports a limit of 3 storage
  buffers per fragment shader for it, the game's frame layout uses 4 (a wgpu validation error at the
  first frame). Whether the real-GPU drivers
  of Mesa 21.x (RADV, ANV) draw was not tested: no such hardware in a container.
- **Sound**: ALSA, directly or through PulseAudio's or PipeWire's ALSA plugin.

**No Vulkan driver.** On Linux the game draws only with Vulkan, so "no adapter" means no loader or no
driver. `crates/ob-core/src/platform/vulkan.rs` words it for the player and names the packages for the distribution
(`/etc/os-release`: apt, dnf, pacman, zypper; any other: "the Vulkan loader and Mesa's Vulkan drivers"):

> no Vulkan driver for this graphics card was found. Overbright draws with Vulkan: install one with
> "sudo apt install libvulkan1 mesa-vulkan-drivers" (Mesa 22 or newer; for an NVIDIA card, NVIDIA's own
> driver), then start again

(or "the Vulkan loader (libvulkan.so.1) is not installed. … install it and the driver with …"). The
probe (`game --probe`) gives it as its `error=` line, which the launcher shows as its Graphics card line
(state `no-vulkan` of `overbright --render`); the game prints it when its window gets no surface or
adapter ("could not open a window: …", which the launcher's failure screen shows after Play). Before,
the launcher showed wgpu's "No suitable graphics adapter found; noop not requested, vulkan
drivers/libraries could not be loaded, metal support not compiled in, …" and the game "Failed to create
surface for any enabled backend: {}". Started directly from a file manager the game has no terminal to
print into and shows nothing; the menu entry (`overbright.desktop`) starts the launcher. Tests:
`platform::vulkan` (the families and the line), `launcher::tests::no_vulkan_driver_says_what_to_install`,
the render baseline `tests/launcher/state-no-vulkan-1366x768@150.png`, and `tests/no_vulkan.rs` (the
built game's probe with the loader pointed at a driver file that does not exist).

### The distributions it was run on

`scripts/release/distros/run.sh <linux archive> <out dir>` (with `NFSU2_ROOT`; `DEV_GAME` = the harness
game of the same build, `container/build.sh --dev-harness`) runs the archive in a rootless podman
container of each distribution in `scripts/release/distros/images.txt`: the image as pulled (ldd,
`--version`) and the image with the packages a desktop has (the Vulkan loader and Mesa's software
driver lavapipe, Xvfb, the X11, xkbcommon, ALSA and udev libraries), where it runs `--version` of both
programs, `game --check-game` on the game folder mounted read-only, `game --probe` on lavapipe, the
harness game's headless shots `main` and `explore` (real frames, the menu and free roam), and the
launcher on an X server inside the container (one screenshot, the libraries it loaded). `--novk` adds
Ubuntu 24.04 without the Vulkan loader and with the loader but no driver (the lines quoted above). The containers get no
network, no devices (no sound, no GPU) and nothing of the host's display. Result per distribution:
works = every step passed.

| distribution | glibc | lavapipe (Mesa) | glibc 2.35 build (before) | glibc 2.28 build (now) |
|---|---|---|---|---|
| Ubuntu 20.04 | 2.31 | 21.2.6 | does not start (needs glibc 2.35) | starts, checks the game, finds lavapipe, launcher; lavapipe 21.2 cannot draw (above) |
| Ubuntu 22.04 | 2.35 | 23.2.1 | works | works |
| Ubuntu 24.04 | 2.39 | 25.2.8 | works | works |
| Linux Mint 22 (`linuxmintd/mint22-amd64`) | 2.39 | 25.2.8 | works | works |
| Debian 11 | 2.31 | 20.3.5 | does not start | starts, checks the game, finds lavapipe, launcher; lavapipe 20.3 cannot draw (above) |
| Debian 12 | 2.36 | 22.3.6 | works | works |
| Debian 13 | 2.41 | 25.0.7 | works | works |
| Fedora 40 | 2.39 | 24.1.7 | works | works |
| Fedora 41 | 2.40 | 25.0.7 | works | works |
| Fedora 45 (`fedora:latest`) | 2.44 | 26.2.6 | works | works |
| AlmaLinux 8.10 (RHEL 8 family) | 2.28 | 23.1.4 | does not start | starts, checks the game, launcher; its lavapipe fails its own `vkCreateInstance` (`vulkaninfo` too), so the launcher shows the no-Vulkan line: no frames |
| AlmaLinux 9.8 (RHEL 9 family) | 2.34 | 25.2.7 | the launcher starts, the game does not (needs 2.35) | works |
| Arch (stands in for SteamOS) | 2.44 | 26.2.7 | works | works |
| openSUSE Leap 15.6 | 2.38 | 23.3.4 | works | works |
| openSUSE Tumbleweed | 2.44 | 26.2.6 | works | works |

Every image as pulled lacks `libasound.so.2` (the Mint image excepted): a minimal install, not a desktop.
Ubuntu-based distributions (Linux Mint 20–22, Pop!_OS, elementary, Zorin) have their base's glibc and
Mesa; SteamOS 3 is Arch-based (its first releases had glibc 2.33, later ones newer), stood in for by the
Arch container. The same harness game built in both images (Ubuntu 22.04 and AlmaLinux 8) drew the same
pictures (`main`, `explore`, `race`: identical PNG bytes) with identical simulation state hashes
(`NFSU2_STATE_HASH`, 181 and 91 steps) in one Ubuntu 24.04 container. Not run: real GPUs and drivers,
Wayland sessions, distributions outside the table.

**What the older floor costs.** The Windows programs are built as before (the same `Containerfile` and
image); only Linux moved to `Containerfile.linux`. One code change: the profiler's thread id comes from
the `gettid` system call instead of glibc's `gettid()`, which needs glibc 2.30. Linked against glibc
2.28, the binaries name `libdl.so.2` and `libpthread.so.0` (in `check_linux.sh`'s allow-list: newer
glibc keeps them as compatibility libraries) and the older symbol versions of `exp`, `log`, `pow`
and `hypot`, which on a newer glibc run the same functions (only their error reporting for overflow
differs; inferred from glibc's sources, not measured). On glibc older than 2.35 the huge-page malloc
tunable that the game's start sets (docs/developing/developer-guide.md, "Malloc on huge pages") does not exist and
is ignored: the "system" allocator that machines under 10 GiB of RAM use then loads races more slowly;
the default allocator is unaffected (inferred, not measured on such a machine).
