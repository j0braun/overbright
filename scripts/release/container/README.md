# Release builds (Linux and Windows)

The shipped binaries are built in a container, so the result does not depend on the build machine:

- **Linux** `x86_64-unknown-linux-gnu` on AlmaLinux 8 (`Containerfile.linux`, the manylinux_2_28 base):
  glibc 2.28 is the oldest glibc the game needs, and the default compiler flags add no "x86 ISA needed"
  note, so the binary runs on any x86-64 distribution with glibc >= 2.28 (X11 and Wayland; window
  decorations on Wayland come from the binary itself). The distributions it was run on:
  docs/building-releases.md, "Linux: what a player needs".
- **Windows** `x86_64-pc-windows-gnu`, cross-compiled with mingw-w64 (open source; no licence terms to
  accept). The game and the launcher are GUI-subsystem programs with an icon, a version resource and an application manifest (docs/platform-windows.md); the C runtime parts
  are linked statically, so it needs only system DLLs (`scripts/release/check_pe.py`).

Requirements: podman (rootless is fine), git, rsync, python3, binutils (readelf, strings). The first run
builds the two images and downloads the crates; later runs reuse them. `Containerfile` (Windows, and the
crate download): the Ubuntu 22.04 image pinned by digest, Rust from the official archives pinned by version
and SHA-256, mingw-w64 from Ubuntu. `Containerfile.linux` (Linux): the AlmaLinux 8 image pinned by digest,
the same Rust archives (same version and SHA-256), gcc 8 and the ALSA and udev headers from AlmaLinux. Each
image is tagged with its Containerfile's hash (`overbright-build:<hash>`, `overbright-build-linux:<hash>`).

```
scripts/release/container/build.sh <commit> <out dir>              # both targets from `git archive <commit>`
scripts/release/container/build.sh --linux  <source dir> <out dir> # a working tree (git-ignored files left out)
scripts/release/container/build.sh --windows --dev-harness --tests --fast <source dir> <out dir>
```

- The binaries land in `<out dir>/dist/linux/` and `<out dir>/dist/windows/` (shipping: no developer
  harness); `--dev-harness` adds `dist/<target>-dev/game[.exe]` for the checks (never shipped).
- `--fast` turns LTO off (check builds only). `JOBS=<n>` sets cargo's jobs and the container's CPU
  limit (default 4).
- The version stamp: `OB_VERSION` (`git describe --tags --always`) and `OB_COMMIT` (the full commit id;
  `-modified` when a working tree has uncommitted changes) reach build.rs; `game --version` prints them.
  An ordinary `cargo build` stamps the package version and `unknown`.
- Paths inside the binaries are remapped (`/src` -> `overbright`, the crate cache -> `cargo`; C code too).
- Reproducible: the same commit gives byte-identical binaries whatever the caches held (a fixed C build date
  for mimalloc's `__DATE__`, no PE time stamp, the Windows programs stripped of their COFF symbol table,
  ThinLTO's promoted symbols named after their codegen unit instead of a hash of its bitcode;
  docs/building-releases.md "Reproducibility").
- A target directory remembers which image filled it (`target-<target>.image`); a different image empties
  it first (cargo would keep C code compiled against the other image's glibc headers).
- After the build, `check_linux.sh` (ISA note, glibc <= 2.28, NEEDED allow-list, no RPATH, no home paths)
  and `check_pe.py --gui` (DLL allow-list, GUI subsystem, icon, version and manifest resources, no home paths) run
  on the player's two programs (the game and the launcher `overbright`); the script exits non-zero when one fails.
- The player archives are made from this build by `scripts/release/package.sh` (docs/building-releases.md).

On Windows itself, contributors build natively instead: rustup (MSVC toolchain) and the Visual Studio
Build Tools, then `cargo build --release`. build.rs then compiles the resources with `rc.exe`.
