# Making a release

`scripts/release/package.sh` turns one commit into the two player archives and checks them. It writes only
to the folder you give it; publishing (a tag, a release page, an upload) is done by hand afterwards.

```
. scripts/local.env                       # NFSU2_ROOT: the game for --check-game and the embedded-data scan
SOURCE_URL=https://example.org/overbright \
  scripts/release/package.sh <commit> <out dir>
```

Requirements: everything `scripts/release/container/README.md` lists (podman, git, python3, binutils), plus
`rustc` (the icon), numpy for python3 (the embedded-data scan), Wine and Xvfb (the Windows checks). Nothing
is installed by the script; the only network use is the container build's `cargo fetch` (a no-op when its
crate cache is complete).

The archives' leak checks search for the maintainer's private save names (`scripts/release/families.private.txt`,
part of the private set; also found as `$OB_PRIVATE/scripts/release/families.private.txt`). Without that file
they are not searched, and a run whose other checks all pass ends `PACKAGE: ok, private terms NOT LOADED` with
exit status 3 instead of 0: what a public checkout can reach. A release needs exit 0.

## What it makes

| `overbright-<version>-linux-x86_64.tar.gz` | `overbright-<version>-windows-x86_64.zip` |
|---|---|
| `overbright` (the launcher), `game` | `overbright.exe` (the launcher), `game.exe` |
| `README.txt`, `LICENSE`, `THIRD-PARTY-NOTICES.md` | `README.txt`, `LICENSE.txt`, `THIRD-PARTY-NOTICES.md` (CRLF) |
| `THIRD-PARTY-LICENSES.txt` (crates + Rust std, Linux build) | `THIRD-PARTY-LICENSES.txt` (crates + Rust std + mingw-w64 runtime + libgcc) |
| `overbright.desktop`, `overbright.png` (256 px icon) | (optional, `DXC_DIR`: `dxcompiler.dll`, `dxil.dll`, `LICENSE-DXC.txt`) |

Each archive has one top folder named like the archive. `<version>` is `git describe --tags --always` of the
commit, without the `v` of a tag such as `v0.2.1`: tag the commit first (`git tag v0.2.1 <commit>`) to get a
readable version (`0.2.1`).

- **Not shipped:** the developer tools the build also makes (`car_check`, `inspect`, `world`, `frontend`,
  `dump_textures`, ...: maintainer tools) and **`overbright-upscale`**. The texture-pack generator is for
  players, but the game runs the same code itself (Graphics → Generate Texture Pack,
  `upscale_pack::generate`); the command line adds only review sheets (`--sheet`) and the UI
  pack, which needs a separate HD UI pack (docs/texture-pack-generator.md, docs/loading-image.md). One fewer
  program to explain; it can be added to the allow-lists later if players ask for it.
- **The `.desktop` file** is a template: an unpacked archive can live anywhere and a menu entry needs the
  full path, so README.txt says to copy it and set `Exec=`. The icon is drawn by
  `build_support/icon_art.rs` (our own art, the same as the window icon and the Windows `.ico`), rendered by
  `scripts/release/icon_rgba.rs`.
- **README.txt** (`scripts/release/player/README.txt`, one template, `@@linux` / `@@windows` sections): what
  Overbright is, that the player needs their own copy of the game, how to start, where settings, saves, logs
  and the cache live (docs/platform-dirs.md), how to report a problem, not affiliated with EA, the licence
  and where the source code is.
- **THIRD-PARTY-LICENSES.txt** (`scripts/release/crate_licenses.py`): `cargo metadata --filter-platform <target>`
  of the commit (run in the build image, offline, default features = the shipping build) → the
  normal-dependency closure of the `nfsu2` package; each crate's name, version, licence expression and
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
| `listing <archive>` | the archive holds exactly the allow-listed files (a tool binary, a stray file → FAIL) |
| `zip text files: CRLF` | Notepad-friendly text in the Windows archive |
| `version stamp <binary>` | all four binaries carry the version and commit of the archive's name |
| `check_linux.sh` (both Linux programs) | no x86 ISA note, glibc ≤ 2.28, NEEDED allow-list, no RPATH, no `/home/` |
| `check_pe.py --gui` (both exes) | system DLLs only, GUI subsystem, icon + version + manifest resources, no home paths |
| `game --selftest` / `--dev-harness` | the developer harness is not in the shipping game (usage error, rc 2) |
| `no harness strings` | the maintainer's save names and the harness's flags and variables are absent (both games) |
| `scan.py archive` text/strings | every shipped file (text line by line, binaries through their ASCII and UTF-16 strings) against the privacy families of `families.txt` + `families.private.txt` (home paths, Windows user folders, private names and save names, maintainer wording, machine names, private mods). Allowed exceptions: `allow-archive.txt` |
| `scan.py archive` embedded game data | no run of ≥ 64 bytes in any shipped file equals bytes of the player's game (`NFSU2_ROOT`, every file; 32-byte fingerprints every 16 bytes in executables and small files, every 32 in files < 4 MiB, every 1024 above), the native save fixtures or `~/.local/share/NFS Underground 2` (the maintainer's saves). Generic runs any program has are allowed and counted (`generic_run`: pieces of the CRC-32 tables, step tables such as deflate's code tables, runs of < 24 non-zero bytes, the MS-DOS stub / version-resource names); `scan.py archive DIR --game-ref ... --list` lists them. numpy is needed (about 25 s for 1.7 GB of game files) |
| `game --version`, `game --check-game $NFSU2_ROOT` | the unpacked Linux game runs with an empty HOME |
| `overbright --render about` | the launcher draws with an empty HOME, no GPU (`VK_ICD_FILENAMES=/nonexistent`) and no display |
| `nothing written to HOME` | those runs leave the player's home alone |
| Wine: `game.exe --version`, `overbright.exe --render about` | the Windows programs start (private prefix in `<out>/wine/`, private Xvfb: `PKG_DISPLAY` or a free one; `WAYLAND_DISPLAY` unset) |

`--plant home-text|home-binary|file` proves the checks bite: it plants a home path into README.txt, a home path
at the end of `game.exe`, or a tool binary into the Linux folder after assembling, and the run must end in
`PACKAGE: FAIL`. `--no-build` reuses `<out>/build` (seconds instead of a release build).

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

## The clean repository

The public repository starts with one commit of the public tree; no earlier commit is carried over. The
private files (`scripts/release/private.list`) go into a private companion repository. Both are made by
`scripts/release/make_clean_repo.sh` from a commit of the development repository. They stay local: the script
adds no remote and pushes, publishes and deletes nothing. It refuses a target folder that is not empty or that
lies inside the source repository.

```
scripts/release/make_clean_repo.sh <source repo> <public dir> <private dir> \
    --name "Overbright Developers" --email overbright@localhost [--url <source url>] \
    [--rev <commit>] [--date <ISO date>] [--message-file <file>] [--work <dir>] [--target-dir <dir>]
```

- **Public repository**: the files of `<commit>` (default `HEAD`) minus private.list (`public_tree.py --rev`:
  the blobs byte for byte with their modes, no line-ending conversion; the root `Cargo.toml` leaves the private
  workspace member `devprobes` out of `members`, and the root `Cargo.lock` that member's `[[package]]` block,
  which cargo would otherwise drop on the first build without `--locked`). It is a new repository on branch `main` with one commit,
  "Overbright: first public source" plus a short body (and a `Source:` line when `--url` is given), or the
  text of `--message-file`. Author
  and committer are `--name` / `--email`. The date is `--date`, by default the source commit's date. Git's
  global and system configuration are not read, so the same input gives the same commit id.
- **Private companion repository**: the private.list files at their paths, plus `PRIVATE-README.md`, one commit.
  The README says how a developer uses it: overlay it on a public checkout with rsync, which gives the full
  development tree (the private tests, the harness's saves, the gate's rename lists), or point `OB_PRIVATE`
  at it (docs/testing.md).
- Environment instead of options: `OB_AUTHOR_NAME`, `OB_AUTHOR_EMAIL`, `OB_SOURCE_URL`, `OB_COMMIT_DATE`.

The verify step runs at the end, or again with `--verify <source> <public> <private>`. It prints PASS / FAIL
lines (also in `<work>/verify.log`, by default `clean-repo-work` next to the public folder) and ends with
`CLEAN REPO: PASS|FAIL`:
- each repository has one commit, only the branch `main`, and no remote;
- no file of the public repository is on private.list;
- the overlay: public + private = the source commit, file for file, by mode and SHA-256 (the work-tree files
  too). The only allowed differences are the public `Cargo.toml`, which must equal the source's with the
  private members left out, and the public `Cargo.lock`, which must equal the source's without those members'
  package blocks (a lock that still holds one fails, and so does one that lost any other package). The proof
  line prints a digest of both listings and the packages left out;
- `cargo metadata` in the public repository WITHOUT `--locked` (the resolve of an outside developer's first
  plain `cargo build`) leaves its work tree unchanged: a lock cargo would rewrite fails (and is put back, so
  verify changes nothing). It runs with `--no-build` too;
- `scan.py text` (scopes tree, public and all) and `scan.py binary` on the public repository: 0 not allowed,
  0 failing. This needs `families.private.txt`, so run the script from the full tree;
- every object in the public repository's `.git` (`git cat-file --batch-all-objects`: packed and loose,
  decompressed) and every other `.git` file, searched for the private pattern families. A blob of a tracked
  text file is judged as the text scan judges it (allow.txt). Binary blobs are also searched for the fixed
  leak strings. Trees and the other `.git` files have no exceptions, and commits only the exact co-author
  credit line `COMMIT_CREDIT_LINES` (clean_repo.py), so the author identity is checked too;
- `cargo build --release`, `cargo build --release --features dev-harness --bins --examples` and
  `cargo test --release --features dev-harness` in the public repository, with `NFSU2_ROOT` and `OB_PRIVATE`
  unset and its own cache and XDG folders under `<work>`. The tests must pass, and every ignored test must
  give its reason. Afterwards the public work tree must be unchanged. `--no-build` skips these checks. The
  target dir is `--target-dir`, else `$CARGO_TARGET_DIR`, else `<work>/target`;
- the source repository is untouched: a fingerprint of its `.git` (every file's size and time) and `HEAD`,
  taken by make, is unchanged.

**Re-author later.** While neither repository is published, one command redoes both single commits with the
real identity (and the source URL in the public message):

```
scripts/release/make_clean_repo.sh --reauthor "Real Name" real@address https://<public url> <public dir> <private dir>
scripts/release/make_clean_repo.sh --verify <source repo> <public dir> <private dir>
```

The tree and the date stay the same. The old commit is removed from `.git` (reflog expired, `gc --prune=now`).
A repository with more than one commit, a remote, another ref (a tag) or uncommitted changes is refused, and
then neither repository is changed.

**The old `.git`** stays as the private backup of the development history. The script never moves or deletes
it. Moving it out of the development folder is done by hand, after the verify step has passed.

`scripts/checks/selfcheck.sh OUT cleanrepo` runs the script on a small synthetic source. A good run must pass
and give the same commit id when made again, and re-authoring must work. Each of these must fail: a private
name planted in a public file, a target folder that is not empty, re-authoring a repository that has a
second commit or a tag, and a public `Cargo.lock` that still holds the private member's block (the overlay and
the plain resolve both fail) or lost a public package (the overlay fails). The synthetic source is a real
two-member cargo workspace, so the plain-resolve line runs there too.

## Changes after the first release

The public repository's history goes on there: every later change reaches it as a pull request that is
reviewed and merged on GitHub. `scripts/release/sync_pr.sh` makes one from a commit of the development
repository:

```
scripts/release/sync_pr.sh --branch perf/batch-e --title "<title>" --body-file <pr body.md> [--rev <commit>] \
    [--message-file <commit body>] [--repo j0braun/overbright] [--base main] [--work <dir>] [--no-push]
```

- **What goes in:** the commit's public tree, selected exactly as for the clean repository (`public_tree.py
  --rev`: private.list applied, the private workspace members left out of `Cargo.toml` and `Cargo.lock`),
  copied over a new branch of a fresh clone of the public repository (files the commit no longer has are
  deleted on the branch). The branch is one new commit on top of the public base: no commit of the
  development repository and none of its history leaves it.
- **The checks before the commit** (any FAIL stops it): no branch file on private.list; `scan.py text`
  (scopes tree, public, all) and `scan.py binary` at 0 on the branch's tree; the development folder's old name
  in no file name or content; no private pattern in the commit message; a plain `cargo metadata` leaves the
  tree unchanged. The scans need `families.private.txt`: run it from the full development tree.
- **The commit:** author and committer `j0braun <52372377+j0braun@users.noreply.github.com>`, `git commit
  -s` (the DCO check of `.github/workflows/dco.yml`), the co-author line of `clean_repo.py`'s
  `COMMIT_CREDIT_LINES`. The push and `gh pr create` use gh's credentials. The script refuses the base
  branch's name and a branch that exists already; it never merges and never pushes to the base.
- `--no-push` stops after the commit and leaves the clone in the work folder (default `sync-pr-work` next
  to the development repository) for a look.

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
driver. `src/platform/vulkan.rs` words it for the player and names the packages for the distribution
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
tunable that the game's start sets (docs/developer-guide.md, "Malloc on huge pages") does not exist and
is ignored: the "system" allocator that machines under 10 GiB of RAM use then loads races more slowly;
the default allocator is unaffected (inferred, not measured on such a machine).

## What a human must still do

1. Tag the commit and run the script with `SOURCE_URL` set to the public repository (the placeholder is in
   README.txt otherwise). The public repository must hold the exact source of the commit (GPL-3.0 §6): the
   private files (`scripts/release/private.list`) are not part of it, so publish the scrubbed tree, then build
   from that commit.
2. Test on real Windows 10/11 (Wine does not prove DX12, DPI, SmartScreen, NTFS, real drivers:
   docs/platform-windows.md, docs/launcher.md), and on Linux with real GPUs (the distributions above ran
   in containers, on Mesa's software driver).
3. Decide about `dxcompiler.dll`. It is **not bundled**: without it, DX12 uses FXC (slower pipeline builds,
   no multiview cube; Vulkan is preferred anyway). To ship it, put `dxcompiler.dll`, `dxil.dll` (from
   Microsoft's DirectXShaderCompiler release, `bin/x64/`) and its licence as `LICENSE-DXC.txt` in a folder
   **outside the repository** and run with `DXC_DIR=<that folder>`; the script copies the three files next to
   `overbright.exe` / `game.exe` and adds them to the allow-list. Check that DXC's licence terms allow
   redistribution with a GPL program before doing so.
4. Publish the archives and `SHA256SUMS` from the out dir.

## Measured (2026-11-12, the packaging unit's runs; work-copy commits, version = short commit id)

- Archives: Linux `.tar.gz` 13.4 MB (game 28.1 MB, overbright 7.4 MB, THIRD-PARTY-LICENSES.txt 1.8 MB: 199
  crates, 181 linked, 128 distinct texts, + the Rust std notices), Windows `.zip` 10.9 MB (game.exe 20.9 MB,
  overbright.exe 4.2 MB after the strip; 160 crates, 145 linked).
- Two builds of one commit from different crate/target caches: all four programs and both archives
  byte-identical (after the three fixes above).
- Checks: every check `ok` except the two privacy/game-data checks, which **fail on the current source**
  (not on the packaging). In the shipped `game`/`game.exe` (the launcher is clean):
  - text: dated maintainer remarks in WGSL shader comments compiled in through `include_str!`
    (`src/render/car_paint.wgsl`, `volumetric.wgsl`, `shaders/enh/lighting/{world_lit,mlrt,modern}.wgsl`,
    `shaders/enh/weather/rain.wgsl`, ...) and the GPU model of the build machine in a settings text
    (`src/frontend/enh/settings_hub/text.rs`, `CostMeasuredOn`);
  - embedded game data: `src/formats/native_save_fresh.bin` (54,966 bytes, a save made by the original, all
    of it), 53 numeric tables copied from SPEED2.EXE (18.4 KB; e.g. the scripted engine-audio patterns of
    `src/audio/car/scripted.rs`, the car light rig of `src/render/native/car/lighting.rs`; the "tables"
    mode of `scan.py` lists 45 such tables in the source), and 7 English game texts from LANGUAGES/*.bin
    (`src/bin/game/flow/menu_tick.rs`, `src/bin/game/menus/keys/{parts,mc}.rs`).
  These must be fixed in the source (scrub the comments, read the tables, texts and save template from the
  player's own files at run time) before a release; `allow-archive.txt` holds only false positives.
  Since the tables are read from the player's executable (docs/exe-tables.md), the embedded game data is gone: the shipped `game` holds no run of ≥ 64
  bytes equal to SPEED2.EXE, English.bin or the old save template (`scan.py archive`: `ok embedded game
  data`); the text/strings check is unchanged by it.
- Planted inputs: a home path in README.txt, a home path appended to game.exe (also caught by
  `check_pe.py`), and a tool binary in the Linux folder (caught by the listing) each add their own FAIL line.
- Wine 11 (private prefix, Xvfb): `game.exe --version` prints the stamp; `overbright.exe --render about`
  draws the same PNG bytes as the Linux launcher.
