#!/bin/bash
# Runs inside a release image (Containerfile: Windows, Containerfile.linux: Linux), started by build.sh. Mounts:
#   /src         the source tree (read-only)       /cargo-home  the crate cache (CARGO_HOME)
#   /target-linux, /target-windows                 cargo's target directories (kept between runs)
#   /dist        the binaries this run delivers
# Environment (from build.sh): MODE=fetch|build, TARGETS="linux windows", DEV_HARNESS=0|1, TESTS=0|1,
# FAST=0|1, CARGO_BUILD_JOBS, OB_VERSION, OB_COMMIT, OB_SOURCE_URL, SOURCE_DATE_EPOCH.
set -euo pipefail
cd /src
rustc -V
if [ "${MODE:-build}" = fetch ]; then
    # Every platform's crates (cargo fetch is not limited to the host), so the build can run offline.
    cargo fetch --locked
    exit 0
fi
ldd --version | sed -n 1p
gcc --version | sed -n 1p
# mingw-w64 is only in the Windows image (Containerfile).
if command -v x86_64-w64-mingw32-gcc > /dev/null; then x86_64-w64-mingw32-gcc --version | sed -n 1p; fi
# Paths in panic messages and debug info: the tree and the crate cache under neutral names (RUSTFLAGS,
# set per target in build() below).
# Ubuntu 22.04's mingw-w64 (8.0) headers default to Windows Server 2003 (_WIN32_WINNT 0x0502) and
# lack ERROR_COMMITMENT_MINIMUM (winerror.h: 635): mimalloc 3 needs both (SRW locks; Windows 10 is the
# oldest supported system anyway).
export CFLAGS_x86_64_pc_windows_gnu="-D_WIN32_WINNT=0x0A00 -DERROR_COMMITMENT_MINIMUM=635"
# Reproducible C parts (docs/developing/building-releases.md "Reproducibility"): mimalloc puts __DATE__/__TIME__ into the
# binary. GCC takes them from SOURCE_DATE_EPOCH; a fixed value (not the commit's time) keeps a build
# independent of which commit first filled the crate cache (cargo does not rebuild C code when only
# SOURCE_DATE_EPOCH changes; the -D below is in CFLAGS, which it does watch, so a cache built before this
# rule is rebuilt once). C source paths are mapped like the Rust ones.
export SOURCE_DATE_EPOCH=315532800
REPRO_CFLAGS="-DOB_REPRODUCIBLE_DATE=315532800 -ffile-prefix-map=/src=overbright -ffile-prefix-map=/cargo-home=cargo"
export CFLAGS_x86_64_pc_windows_gnu="$CFLAGS_x86_64_pc_windows_gnu $REPRO_CFLAGS"
export CFLAGS_x86_64_unknown_linux_gnu="$REPRO_CFLAGS"
if [ "${FAST:-0}" = 1 ]; then
    # Check builds only (not for a release): no LTO, parallel code generation.
    export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
fi
# The shipping game and its tools (no developer harness), then the harness build where asked; each set
# is copied out before the next build replaces the files in target/.
build() { # build <name> <triple> <target dir> <exe suffix>
    local name=$1 triple=$2 tdir=$3 ext=$4 out=/dist/$1
    # Windows: no link time stamp in the PE header (and no COFF symbol table: see the strip below).
    local rf="--remap-path-prefix=/src=overbright --remap-path-prefix=/cargo-home=cargo"
    # ThinLTO renames the local symbols it shares between modules to `<name>.llvm.<module hash>`, the
    # hash being a digest of the module's bitcode. The optimizer does not always write a phi node's
    # incoming values in the same order (same code, different bytes), so the game crate's hash, the
    # names of its promoted symbols and the build-id changed from build to build. This option names
    # them after the module's source name instead (`<crate>.<crate hash>-cgu.<n>`: unique, fixed);
    # the generated code is unchanged (docs/developing/building-releases.md "Reproducibility").
    rf="$rf -C llvm-args=-use-source-filename-for-promoted-locals"
    [ "$name" = windows ] && rf="$rf -C link-arg=-Wl,--no-insert-timestamp"
    export RUSTFLAGS="$rf"
    mkdir -p "$out" "$out-dev"
    echo "== $name: shipping (--bins)"
    cargo build --release --locked --offline --target "$triple" --target-dir "$tdir" --bins
    for b in "$tdir/$triple"/release/*"$ext"; do
        [ -f "$b" ] && [ -x "$b" -o -n "$ext" ] && cp -p "$b" "$out/"
    done
    # Windows: the COFF symbol table names rustc's raw-dylib import libraries after random temporary folders,
    # so it differs from build to build; it is stripped (deterministic, keeps the zero time stamp; `-C
    # link-arg=-s` did not remove it). The cost: a Windows panic backtrace shows addresses, not function
    # names; the panic message and its source location stay. The Linux programs keep their symbols.
    if [ "$name" = windows ]; then
        for b in "$out"/*.exe; do x86_64-w64-mingw32-strip --strip-all "$b"; done
    fi
    if [ "${DEV_HARNESS:-0}" = 1 ]; then
        echo "== $name: dev-harness (--bin game)"
        cargo build --release --locked --offline --target "$triple" --target-dir "$tdir" --features dev-harness --bin game
        cp -p "$tdir/$triple/release/game$ext" "$out-dev/"
    fi
    if [ "${TESTS:-0}" = 1 ]; then
        echo "== $name: cargo test --no-run (dev-harness)"
        cargo test --release --locked --offline --target "$triple" --target-dir "$tdir" --features dev-harness --no-run
    fi
}
for t in ${TARGETS:-linux windows}; do
    case $t in
        linux) build linux x86_64-unknown-linux-gnu /target-linux "" ;;
        windows) build windows x86_64-pc-windows-gnu /target-windows .exe ;;
        *) echo "unknown target $t" >&2; exit 2 ;;
    esac
done
