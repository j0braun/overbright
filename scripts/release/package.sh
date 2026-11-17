#!/bin/bash
# The player archives of one commit (docs/building-releases.md): builds Linux and Windows in the release container
# (container/build.sh, `git archive <commit>`, no developer harness), assembles
#   overbright-<version>-linux-x86_64.tar.gz    overbright-<version>-windows-x86_64.zip
# and checks them. Nothing is uploaded or published: the archives stay in <out dir>.
#
#   package.sh [--no-build] [--no-wine] [--plant home-text|home-binary|file] <commit> <out dir>
#     --no-build   reuse <out dir>/build from an earlier run (the same commit; the version check proves it)
#     --no-wine    skip the checks under Wine (reported as SKIP; a release needs them)
#     --plant K    test the checks: plant a bad input after assembling (home-text: a home path in README.txt,
#                  home-binary: one appended to the game, file: an extra file); the run must end in FAIL
#
# Environment:
#   SOURCE_URL   where the source code is published (README.txt, the build's OB_SOURCE_URL). Unset: the
#                placeholder "SOURCE_URL" (reported; a release must set it)
#   DXC_DIR      a folder OUTSIDE the repository with dxcompiler.dll, dxil.dll and the DXC licence
#                (LICENSE-DXC.txt): they go into the Windows archive. Unset: no DXC (the default)
#   REPO         the repository (default: the tree holding this script)   JOBS  cargo jobs (default 4)
#   NFSU2_ROOT   the player's game (scripts/local.env): `game --check-game` and the embedded-data scan
#   PKG_DISPLAY  an X display number of your own for Wine (default: Xvfb picks a free one)
# Out dir: build/ (container/build.sh's; kept for --no-build), stage/, check/, wine/ (the Wine prefix),
# the two archives, SHA256SUMS, checks.txt (every check line). Exit 0 only when every check is ok; 3 when every
# check is ok but the maintainer's privacy scan or its private search terms were not there (a public checkout:
# "privacy scan: maintainer tool, not in this checkout").
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(cd "$HERE/../.." && pwd)}
JOBS=${JOBS:-4}
PODMAN=${PODMAN:-podman}
BUILD=1 WINE=1 PLANT=""
while [ $# -gt 0 ]; do
    case $1 in
        --no-build) BUILD=0 ;;
        --no-wine) WINE=0 ;;
        --plant) PLANT=${2:?--plant needs a kind}; shift ;;
        -h|--help) sed -n '2,25p' "$0"; exit 0 ;;
        -*) echo "package.sh: unknown option $1" >&2; exit 2 ;;
        *) break ;;
    esac
    shift
done
[ $# -eq 2 ] || { sed -n '2,25p' "$0" >&2; exit 2; }
case $PLANT in ""|home-text|home-binary|file) ;; *) echo "package.sh: --plant home-text|home-binary|file" >&2; exit 2 ;; esac
[ -f "$REPO/scripts/local.env" ] && . "$REPO/scripts/local.env"
OUT=$(mkdir -p "$2" && cd "$2" && pwd)
LOG=$OUT/checks.txt; : > "$LOG"
FAILS=0
say() { echo "$*" | tee -a "$LOG"; }
ok() { say "ok    $*"; }
bad() { say "FAIL  $*"; FAILS=$((FAILS + 1)); }
check() { # check <description> <command...>: ok when the command exits 0
    local d=$1; shift
    if "$@" >> "$OUT/check-detail.log" 2>&1; then ok "$d"; else bad "$d (check-detail.log)"; fi
}
die() { say "PACKAGE: FAIL ($*)"; exit 1; }
: > "$OUT/check-detail.log"

COMMIT=$(git -C "$REPO" rev-parse --verify "$1^{commit}") || die "no commit $1"
VERSION=$(git -C "$REPO" describe --tags --always "$COMMIT")
[[ $VERSION =~ ^v[0-9] ]] && VERSION=${VERSION#v}   # tag v0.1.0 -> version 0.1.0 (container/build.sh does the same)
EPOCH=$(git -C "$REPO" log -1 --format=%ct "$COMMIT")
[[ $VERSION =~ ^[A-Za-z0-9._+-]+$ ]] || die "version '$VERSION' is not a file-name-safe string"
SOURCE_URL=${SOURCE_URL:-SOURCE_URL}
LIN=overbright-$VERSION-linux-x86_64
WIN=overbright-$VERSION-windows-x86_64
say "package: commit $COMMIT  version $VERSION  epoch $EPOCH  source $SOURCE_URL"

# ---- build --------------------------------------------------------------------------------------------------
B=$OUT/build
if [ $BUILD = 1 ]; then
    t=$(date +%s)
    OB_SOURCE_URL=$SOURCE_URL REPO=$REPO JOBS=$JOBS "$HERE/container/build.sh" "$COMMIT" "$B" > "$OUT/build.log" 2>&1
    rc=$?
    say "build: rc=$rc $(( $(date +%s) - t )) s (build.log)"
    [ $rc = 0 ] || die "build (build.log)"
fi
for f in linux/game linux/overbright windows/game.exe windows/overbright.exe; do
    [ -f "$B/dist/$f" ] || die "no $B/dist/$f"
done
IMAGE=${IMAGE:-overbright-build:$(sha256sum "$HERE/container/Containerfile" | cut -c1-12)}

# ---- the licence list (cargo metadata in the image, offline, from the build's crate cache) ---------------------
W=$OUT/work; rm -rf "$W"; mkdir -p "$W"
# The toolchain's own notices: the Rust standard library (both targets) and, for Windows, the statically linked
# mingw-w64 runtime and libgcc (GCC Runtime Library Exception).
mkdir -p "$W/rustdoc"
"$PODMAN" run --rm --network=none "$IMAGE" sh -c 'tar -C /usr/local/share/doc/rust -cf - . ; true' | tar -x -C "$W/rustdoc" \
    && "$PODMAN" run --rm --network=none "$IMAGE" cat /usr/share/doc/mingw-w64-common/copyright > "$W/mingw-w64-copyright.txt" \
    && RUSTC_V=$("$PODMAN" run --rm --network=none "$IMAGE" rustc -V) || die "toolchain notices from $IMAGE"
STD=(--section "The Rust standard library ($RUSTC_V), linked into every program: its copyright notices" \
    "$W/rustdoc/COPYRIGHT-library.html")
for f in "$W"/rustdoc/licenses/*.txt; do
    id=$(basename "$f" .txt)
    grep -qF "$id" "$W/rustdoc/COPYRIGHT-library.html" && STD+=(--section "Licence text $id (the Rust standard library)" "$f")
done
WINRT=(--section "mingw-w64 runtime (C runtime parts statically linked into the Windows programs; Debian's copyright file of mingw-w64)" \
    "$W/mingw-w64-copyright.txt" \
    --section "libgcc (statically linked into the Windows programs): GPL-3.0-or-later with the GCC Runtime Library Exception 3.1" \
    "$W/rustdoc/licenses/GCC-exception-3.1.txt")
for t in x86_64-unknown-linux-gnu x86_64-pc-windows-gnu; do
    "$PODMAN" run --rm --network=none -v "$B/src:/src:ro" -v "$B/cargo-home:/cargo-home:ro" -w /src "$IMAGE" \
        cargo metadata --format-version 1 --locked --offline --filter-platform "$t" > "$W/meta-$t.json" 2> "$W/meta-$t.err" \
        || die "cargo metadata $t ($W/meta-$t.err)"
    extra=("${STD[@]}"); [ $t = x86_64-pc-windows-gnu ] && extra+=("${WINRT[@]}")
    if python3 -I "$HERE/crate_licenses.py" --metadata "$W/meta-$t.json" --map "/cargo-home=$B/cargo-home" \
        --target "$t" --out "$W/LICENSES-$t.txt" "${extra[@]}" > "$W/licences-$t.log" 2>&1; then
        ok "third-party licences $t: $(sed -n 1p "$W/licences-$t.log" | sed 's/^crate_licenses: [^:]*: //; s/ -> .*//') + std$([ $t = x86_64-pc-windows-gnu ] && echo ', mingw-w64, libgcc')"
    else
        bad "third-party licences $t: $(tr '\n' ' ' < "$W/licences-$t.log" | cut -c1-300)"
    fi
done

# ---- the icon (build_support/icon_art.rs of the commit) -------------------------------------------------------
rustc -O --edition 2021 -o "$W/icon_rgba" "$B/src/scripts/release/icon_rgba.rs" > "$W/icon.log" 2>&1 || die "icon_rgba ($W/icon.log)"
"$W/icon_rgba" 256 > "$W/icon.rgba" && python3 -I "$HERE/archive.py" png "$W/icon.rgba" 256 "$W/overbright.png" || die "icon png"

# ---- stage ----------------------------------------------------------------------------------------------------
S=$OUT/stage; rm -rf "$S"; mkdir -p "$S/$LIN" "$S/$WIN"
readme() { # readme <linux|windows> <licence file name> <crates file name> <dest>
    python3 -I - "$B/src/scripts/release/player/README.txt" "$1" "$2" "$3" "$4" "$VERSION" "$COMMIT" "$SOURCE_URL" <<'EOF'
import sys
src, os_, lic, crates, dest, ver, commit, url = sys.argv[1:]
out, keep = [], True
for line in open(src, encoding="utf-8").read().split("\n"):
    if line.startswith("@@"):
        keep = line[2:] in (os_, "end")
        continue
    if keep:
        out.append(line)
text = "\n".join(out)
for k, v in {"@VERSION@": ver, "@COMMIT@": commit, "@SOURCE_URL@": url, "@LICENSE_FILE@": lic,
             "@CRATES_FILE@": crates}.items():
    text = text.replace(k, v)
open(dest, "w", encoding="utf-8", newline="\n").write(text)
EOF
}
crlf() { python3 -I -c 'import sys; d=open(sys.argv[1],"rb").read().replace(b"\r\n",b"\n"); open(sys.argv[2],"wb").write(d.replace(b"\n",b"\r\n"))' "$1" "$2"; }
install -m 755 "$B/dist/linux/overbright" "$B/dist/linux/game" "$S/$LIN/"
install -m 644 "$B/src/LICENSE" "$B/src/THIRD-PARTY-NOTICES.md" "$W/overbright.png" \
    "$B/src/scripts/release/player/overbright.desktop" "$S/$LIN/"
install -m 644 "$W/LICENSES-x86_64-unknown-linux-gnu.txt" "$S/$LIN/THIRD-PARTY-LICENSES.txt"
readme linux LICENSE THIRD-PARTY-LICENSES.txt "$S/$LIN/README.txt"
install -m 755 "$B/dist/windows/overbright.exe" "$B/dist/windows/game.exe" "$S/$WIN/"
readme windows LICENSE.txt THIRD-PARTY-LICENSES.txt "$W/README-win.txt"
crlf "$W/README-win.txt" "$S/$WIN/README.txt"
crlf "$B/src/LICENSE" "$S/$WIN/LICENSE.txt"
crlf "$B/src/THIRD-PARTY-NOTICES.md" "$S/$WIN/THIRD-PARTY-NOTICES.md"
crlf "$W/LICENSES-x86_64-pc-windows-gnu.txt" "$S/$WIN/THIRD-PARTY-LICENSES.txt"
LIN_FILES="LICENSE README.txt THIRD-PARTY-LICENSES.txt THIRD-PARTY-NOTICES.md game overbright overbright.desktop overbright.png"
WIN_FILES="LICENSE.txt README.txt THIRD-PARTY-LICENSES.txt THIRD-PARTY-NOTICES.md game.exe overbright.exe"
if [ -n "${DXC_DIR:-}" ]; then
    case $(cd "$DXC_DIR" && pwd)/ in "$REPO"/*) die "DXC_DIR must be outside the repository" ;; esac
    for f in dxcompiler.dll dxil.dll LICENSE-DXC.txt; do
        [ -f "$DXC_DIR/$f" ] || die "DXC_DIR has no $f"
        install -m 644 "$DXC_DIR/$f" "$S/$WIN/"
    done
    WIN_FILES="$WIN_FILES LICENSE-DXC.txt dxcompiler.dll dxil.dll"
    say "dxc: from $DXC_DIR"
fi
case $PLANT in
    home-text) echo "planted: /home/x/secret" >> "$S/$LIN/README.txt"; say "PLANTED a home path in $LIN/README.txt" ;;
    home-binary) printf '\0/home/x/planted\0' >> "$S/$WIN/game.exe"; say "PLANTED a home path at the end of $WIN/game.exe" ;;
    file) cp "$B/dist/linux/game" "$S/$LIN/car_check"; say "PLANTED an extra file $LIN/car_check" ;;
esac

# ---- archives -------------------------------------------------------------------------------------------------
TAR=$OUT/$LIN.tar.gz ZIP=$OUT/$WIN.zip
rm -f "$TAR" "$ZIP" "$OUT/SHA256SUMS"
python3 -I "$HERE/archive.py" tar "$S" "$LIN" "$TAR" "$EPOCH" || die "tar"
python3 -I "$HERE/archive.py" zip "$S" "$WIN" "$ZIP" "$EPOCH" || die "zip"
(cd "$OUT" && sha256sum "$LIN.tar.gz" "$WIN.zip" > SHA256SUMS)
say "archives: $(stat -c '%n %s' "$TAR" "$ZIP" | sed "s|$OUT/||g" | tr '\n' ' ')"

# ---- checks on the unpacked archives --------------------------------------------------------------------------
C=$OUT/check; rm -rf "$C"; mkdir -p "$C/lin" "$C/win" "$C/home" "$C/tmp"
python3 -I "$HERE/archive.py" unpack "$TAR" "$C/lin" || die "unpack $TAR"
python3 -I "$HERE/archive.py" unpack "$ZIP" "$C/win" || die "unpack $ZIP"
L=$C/lin/$LIN WD=$C/win/$WIN

# 1. the listing is exactly the allow-list
listing() { # listing <archive> <top> <files...>: the expected entries vs the archive's
    local a=$1 top=$2; shift 2
    { echo "$top/"; for f in "$@"; do echo "$top/$f"; done; } | sort > "$W/want.txt"
    python3 -I "$HERE/archive.py" list "$a" | sort > "$W/have.txt"
    if diff "$W/want.txt" "$W/have.txt" > "$W/listing.diff"; then ok "listing $(basename "$a"): exactly the $# allowed files"
    else bad "listing $(basename "$a"): $(grep '^[<>]' "$W/listing.diff" | sed 's/^</missing/; s/^>/extra/' | tr '\n' ' ')"; fi
}
# shellcheck disable=SC2086
listing "$TAR" "$LIN" $LIN_FILES
# shellcheck disable=SC2086
listing "$ZIP" "$WIN" $WIN_FILES
lf=$(python3 -I -c 'import sys, pathlib; print(" ".join(p.name for p in sorted(pathlib.Path(sys.argv[1]).iterdir())
    if p.suffix in (".txt", ".md") and b"\n" in p.read_bytes().replace(b"\r\n", b"")))' "$WD")
[ -z "$lf" ] && ok "zip text files: CRLF line ends" || bad "zip text files with LF-only lines: $lf"

# 2. the version stamp: in all four binaries, equal to the archive name's
for b in "$L/game" "$L/overbright" "$WD/game.exe" "$WD/overbright.exe"; do
    n=$(basename "$(dirname "$b")")/$(basename "$b")
    if grep -aqF "$VERSION" "$b" && grep -aqF "$COMMIT" "$b"; then ok "version stamp $n: $VERSION ($COMMIT)"
    else bad "version stamp $n: '$VERSION' / '$COMMIT' not in the binary"; fi
done
case "$(basename "$TAR")/$(basename "$ZIP")" in
    "overbright-$VERSION-linux-x86_64.tar.gz/overbright-$VERSION-windows-x86_64.zip") ok "archive names carry the version $VERSION" ;;
    *) bad "archive names" ;;
esac

# 3. portability and Windows resources
for b in game overbright; do
    if "$HERE/check_linux.sh" "$L/$b" > "$W/cl-$b.txt" 2>&1; then ok "check_linux.sh $b: $(grep -c '^ok' "$W/cl-$b.txt") ok"
    else bad "check_linux.sh $b: $(grep FAIL "$W/cl-$b.txt" | tr '\n' ' ')"; fi
    if python3 -I "$HERE/check_pe.py" --gui "$WD/$b.exe" > "$W/pe-$b.txt" 2>&1; then ok "check_pe.py --gui $b.exe: $(grep -c '^ok' "$W/pe-$b.txt") ok"
    else bad "check_pe.py $b.exe: $(grep FAIL "$W/pe-$b.txt" | tr '\n' ' ')"; fi
done

# 4. no developer harness: its flags end in the usage error, its strings are absent (scripts/checks/release_check.sh)
for flag in --selftest --dev-harness; do
    out=$(env -i HOME="$C/home" PATH=/usr/bin:/bin NFSU2_HEADLESS_ONLY=1 "$L/game" "$flag" 2>&1); rc=$?
    if [ $rc = 2 ] && echo "$out" | grep -q "unknown option $flag"; then ok "game $flag: usage error (rc 2)"
    else bad "game $flag: rc $rc: $(echo "$out" | head -2 | tr '\n' ' ')"; fi
done
# The private save names come from the maintainer's privacy scan (never published): without it they are not
# searched, so the archives cannot be shown clean of them. As in release_check.sh the run then never ends in a
# plain ok: every other check passing ends "PACKAGE: ok, ..." with exit status 3 (what a public checkout can
# reach; a release needs exit 0).
PRIV_MISSING=0 SCAN=$HERE/scan.py HAVE_SCAN=1
if [ ! -f "$SCAN" ]; then
    PRIV_TERMS=""; PRIV_MISSING=1 HAVE_SCAN=0
    say "SKIP  privacy scan: maintainer tool, not in this checkout"
elif PRIV_TERMS=$(python3 -I "$SCAN" leak-terms 2> "$W/leak-terms.txt"); then
    ok "private terms: loaded $(echo -n "$PRIV_TERMS" | grep -c .)"
else
    PRIV_TERMS=""; PRIV_MISSING=1
    say "SKIP  private terms: NOT LOADED (needs the maintainer's private set (OB_PRIVATE)): the private save names were not searched"
fi
for b in "$L/game" "$WD/game.exe"; do
    found=""
    for s in $PRIV_TERMS --selftest --playthrough --flow --photo NFSU2_STATE_HASH NFSU2_SHOT_DATA \
        NFSU2_SHOT_MS NFSU2_SHOT_REAL_SAVE NFSU2_DEV_SHOP NFSU2_AUDIO_WAV; do
        grep -aqF -- "$s" "$b" && found="$found $s"
    done
    if [ -z "$found" ]; then ok "no harness strings in $(basename "$b")"; else bad "harness strings in $(basename "$b"):$found"; fi
done

# 5. privacy and game data in everything that ships (the privacy scan: text, binary strings, embedded bytes)
REFS=(--game-ref "${NFSU2_ROOT:-/nonexistent}")
for r in "$B/src/tests/fixtures/native_saves" "$B/src/src/formats/native_save_fresh.bin" "${OB_PRIVATE:-/nonexistent}/saves/native_save_fresh.bin" "$HOME/.local/share/NFS Underground 2"; do
    [ -e "$r" ] && REFS+=(--game-ref "$r")
done
[ $HAVE_SCAN = 1 ] && { [ -d "${NFSU2_ROOT:-/nonexistent}" ] || bad "embedded game data: NFSU2_ROOT is not set (scripts/local.env)"; }
[ $HAVE_SCAN = 1 ] && for d in "$L" "$WD"; do
    n=$(basename "$d")
    # Without the private patterns (PRIV_MISSING, counted above) the public ones are still searched (--no-private):
    # the scan would otherwise stop at its NOT LOADED line with no verdict.
    NOPRIV=(); [ $PRIV_MISSING = 1 ] && NOPRIV=(--no-private)
    python3 -I "$HERE/scan.py" archive "${NOPRIV[@]}" "$d" "${REFS[@]}" > "$W/scan-$n.txt" 2>&1
    # The verdict lines (text/strings, embedded game data), each followed by up to 12 of its findings.
    awk -v n="$n" '/^  / { if (k++ < 12) det = det "      " substr($0, 3) "\n"; next }
        /^(ok|FAIL)/ { v = substr($0, 1, 6); printf "%s%s: %s\n%s", v, n, substr($0, 7), det; det = ""; k = 0 }' \
        "$W/scan-$n.txt" | tee -a "$LOG"
    grep -q '^ok .*archive text' "$W/scan-$n.txt" || { grep -q '^FAIL .*archive text' "$W/scan-$n.txt" \
        || say "FAIL  $n: archive text: no verdict from the privacy scan ($W/scan-$n.txt)"; FAILS=$((FAILS + 1)); }
    grep -q '^ok .*embedded game data' "$W/scan-$n.txt" || { grep -q '^FAIL .*embedded game data' "$W/scan-$n.txt" \
        || say "FAIL  $n: embedded game data: no verdict from the privacy scan ($W/scan-$n.txt)"; FAILS=$((FAILS + 1)); }
done

# 6. the Linux programs from the unpacked archive: empty HOME, no GPU, no display
run_lin() { env -i HOME="$C/home" PATH=/usr/bin:/bin TMPDIR="$C/tmp" VK_ICD_FILENAMES=/nonexistent LANG=C.UTF-8 "$@"; }
out=$(run_lin "$L/game" --version 2>&1); rc=$?
want="Overbright $VERSION (commit $COMMIT, linux-x86_64)"
[ $rc = 0 ] && [ "$out" = "$want" ] && ok "game --version: $out" || bad "game --version: rc $rc '$out' (want '$want')"
if [ -d "${NFSU2_ROOT:-/nonexistent}" ]; then
    run_lin "$L/game" --check-game "$NFSU2_ROOT" > "$W/check-game.txt" 2>&1; rc=$?
    [ $rc = 0 ] && ok "game --check-game <NFSU2_ROOT>: rc 0, $(wc -l < "$W/check-game.txt") lines" \
        || bad "game --check-game: rc $rc ($(tail -2 "$W/check-game.txt" | tr '\n' ' '))"
else
    say "SKIP  game --check-game: NFSU2_ROOT is not set (scripts/local.env)"
fi
rm -f "$C/about-linux.png"
run_lin "$L/overbright" --render about --size 1280x720 "$C/about-linux.png" > "$W/render.txt" 2>&1; rc=$?
if [ $rc = 0 ] && [ "$(head -c 8 "$C/about-linux.png" 2> /dev/null | od -An -tx1 | tr -d ' \n')" = 89504e470d0a1a0a ]; then
    ok "overbright --render about (empty HOME, VK_ICD_FILENAMES=/nonexistent, no display): $(stat -c %s "$C/about-linux.png") bytes"
else bad "overbright --render about: rc $rc ($(tail -2 "$W/render.txt" | tr '\n' ' '))"; fi
run_lin "$L/overbright" --version 2>&1 | grep -qE '^source: https?://' && ok "about screen: source link $SOURCE_URL" || bad "about screen: no source link (unofficial build; set SOURCE_URL to the public repository)"
[ -z "$(ls -A "$C/home")" ] && ok "nothing written to HOME by those runs" || bad "files written to HOME: $(cd "$C/home" && find . | head -5 | tr '\n' ' ')"

# 7. the Windows programs under Wine (private prefix, private Xvfb; nothing reaches the desktop)
if [ $WINE = 0 ]; then
    say "SKIP  Wine checks (--no-wine)"
elif ! command -v wine > /dev/null || ! command -v Xvfb > /dev/null; then
    bad "Wine checks: wine or Xvfb missing"
else
    export WINEPREFIX=$OUT/wine/prefix WINEDEBUG=-all WINEDLLOVERRIDES="mscoree=;mshtml="
    mkdir -p "$OUT/wine"
    if [ -n "${PKG_DISPLAY:-}" ]; then
        DNUM=${PKG_DISPLAY#:}
        Xvfb ":$DNUM" -screen 0 1280x960x24 -nolisten tcp > "$OUT/wine/xvfb.log" 2>&1 &
        XPID=$!; sleep 1
    else
        exec 9> "$OUT/wine/xvfb.display"
        Xvfb -displayfd 9 -screen 0 1280x960x24 -nolisten tcp > "$OUT/wine/xvfb.log" 2>&1 &
        XPID=$!; exec 9>&-
        for _ in $(seq 100); do [ -s "$OUT/wine/xvfb.display" ] && break; sleep 0.1; done
        DNUM=$(tr -dc '0-9' < "$OUT/wine/xvfb.display")
    fi
    wine_stop() { wineserver -k > /dev/null 2>&1; kill "$XPID" 2> /dev/null && wait "$XPID" 2> /dev/null; }
    trap wine_stop EXIT
    if ! kill -0 "$XPID" 2> /dev/null; then
        bad "Wine checks: Xvfb :$DNUM did not start ($OUT/wine/xvfb.log)"
    else
        # Wine on this X display only: no Wayland socket (a private XDG_RUNTIME_DIR), the prefix pinned to X11.
        mkdir -p "$OUT/wine/xdg" && chmod 700 "$OUT/wine/xdg"
        Wn() { env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE XDG_RUNTIME_DIR="$OUT/wine/xdg" DISPLAY=":$DNUM" NFSU2_DATA_DIR="$C/wine-data" "$@"; }
        Wn wineboot -i > "$OUT/wine/wineboot.log" 2>&1
        Wn wine reg add 'HKCU\Software\Wine\Drivers' /v Graphics /d x11 /f >> "$OUT/wine/wineboot.log" 2>&1
        # No sound driver in this prefix: nothing a package run starts may be heard.
        Wn wine reg add 'HKCU\Software\Wine\Drivers' /v Audio /d '' /f >> "$OUT/wine/wineboot.log" 2>&1
        out=$(Wn timeout 120 wine "$WD/game.exe" --version 2> "$OUT/wine/version.err" | tr -d '\r'); rc=$?
        want="Overbright $VERSION (commit $COMMIT, windows-x86_64)"
        [ $rc = 0 ] && [ "$out" = "$want" ] && ok "wine game.exe --version: $out" \
            || bad "wine game.exe --version: rc $rc '$out' (want '$want')"
        rm -f "$C/about-windows.png"
        Wn timeout 180 wine "$WD/overbright.exe" --render about --size 1280x720 "$(winepath -w "$C/about-windows.png" 2> /dev/null)" \
            > "$OUT/wine/render.log" 2>&1; rc=$?
        if [ $rc = 0 ] && [ "$(head -c 8 "$C/about-windows.png" 2> /dev/null | od -An -tx1 | tr -d ' \n')" = 89504e470d0a1a0a ]; then
            ok "wine overbright.exe --render about: $(stat -c %s "$C/about-windows.png") bytes"
            # The render uses fixed fake data, so both systems draw the same image.
            if cmp -s "$C/about-windows.png" "$C/about-linux.png"; then ok "about screen: Windows image = Linux image (byte-identical)"
            else say "NOTE  about screen: the Windows and Linux images differ (fonts/rasteriser; look at $C/about-*.png)"; fi
        else bad "wine overbright.exe --render about: rc $rc ($(tail -2 "$OUT/wine/render.log" | tr '\n' ' '))"; fi
    fi
    wine_stop; trap - EXIT
fi

# 8. what a human must still do
[ "$SOURCE_URL" = SOURCE_URL ] && say "NOTE  SOURCE_URL is the placeholder: set SOURCE_URL=<public repository> for a release"
[ -z "${DXC_DIR:-}" ] && say "NOTE  no dxcompiler.dll in the Windows archive (DXC_DIR unset; docs/building-releases.md)"
say "sha256: $(tr '\n' ' ' < "$OUT/SHA256SUMS")"
if [ $FAILS = 0 ] && [ $HAVE_SCAN = 0 ]; then say "PACKAGE: ok, privacy scan not run ($LIN.tar.gz, $WIN.zip)"; exit 3; fi
if [ $FAILS = 0 ] && [ $PRIV_MISSING = 1 ]; then say "PACKAGE: ok, private terms NOT LOADED ($LIN.tar.gz, $WIN.zip)"; exit 3; fi
if [ $FAILS = 0 ]; then say "PACKAGE: ok ($LIN.tar.gz, $WIN.zip)"; exit 0; fi
say "PACKAGE: FAIL ($FAILS checks)"; exit 1
