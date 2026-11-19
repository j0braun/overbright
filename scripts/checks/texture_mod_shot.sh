#!/bin/bash
# A texture package the launcher keeps is picked up by the game's renderer (docs/texture-packs.md, "TexMod
# packages and the HUD mod"): the free-roam shot with a package that is on, off, and with no list at all.
#
#   texture_mod_shot.sh GAME OUTDIR      (GAME: a dev-harness build; NFSU2_ROOT: the player's game files)
#
# The package is made here, in a prepared data folder the shot uses (NFSU2_SHOT_DATA; nothing of the
# player's): `texture-pack/imported/shot-test/world/` holds one 4 x 4 magenta PNG under the name of every
# texture of the city's bundles (the loader's name fallback, `<NAME>.png`), with its `package.txt`, and
# `texture-mods.txt` lists it. No game code is changed for this: the shot reads the folder as a player's
# would. Expected: "on" differs from the plain shot over most of the picture and is mostly magenta; "off"
# and "none" are the plain shot exactly. Last line: "TEXTURE_MOD_SHOT: PASS|FAIL (…)".
set -u -o pipefail
GAME=${1:?usage: texture_mod_shot.sh GAME OUTDIR}
O=${2:?usage: texture_mod_shot.sh GAME OUTDIR}
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
TGT=${CARGO_TARGET_DIR:-$REPO/target}
P=${CHECKS_PROFILE:-gate}; [ "$P" = dev ] && P=debug
TEX_FIND=${TEX_FIND:-$TGT/$P/examples/tex_find}
: "${NFSU2_ROOT:?set NFSU2_ROOT to the game folder}"
: "${NFSU2_CACHE_DIR:?NFSU2_CACHE_DIR: a cache folder of your own}"
[ -x "$TEX_FIND" ] || { echo "no tex_find at $TEX_FIND (cargo build --profile ${CHECKS_PROFILE:-gate} --examples)"; exit 2; }
mkdir -p "$O"; O=$(cd "$O" && pwd)

# The prepared data folders (one per run: a shot may write into its data folder).
prep() { # DIR STATE(on|off|none)
  local d=$1/overbright
  rm -rf "$1"; mkdir -p "$d/texture-pack/imported/shot-test/world"
  [ "$2" = none ] && return
  printf '# Overbright texture mods v1\n%s shot-test\n' "$2" > "$d/texture-mods.txt"
  printf 'name=Shot test (every city texture magenta)\nsource=texture_mod_shot.sh\nfiles=%s\n' "$N" \
    > "$d/texture-pack/imported/shot-test/package.txt"
  local w=$d/texture-pack/imported/shot-test/world
  while read -r name; do cp "$O/magenta.png" "$w/$name.png"; done < "$O/names.txt"
}

python3 -I - "$O/magenta.png" << 'EOF'
import struct, sys, zlib
def chunk(t, b): return struct.pack('>I', len(b)) + t + b + struct.pack('>I', zlib.crc32(t + b) & 0xffffffff)
raw = b''.join(b'\0' + b'\xff\x00\xff\xff' * 4 for _ in range(4))
png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 4, 4, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b'')
open(sys.argv[1], 'wb').write(png)
EOF
mapfile -t bundles < <(find "$NFSU2_ROOT/TRACKS" -maxdepth 1 \( -iname 'L4RA.BUN' -o -iname 'STREAML4RA.BUN' \))
"$TEX_FIND" "" "${bundles[@]}" 2> /dev/null | sed -E 's/^.*\.BUN: ([^ ]+) .*/\1/I' | grep -vE '[/\\]' | sort -u > "$O/names.txt"
N=$(wc -l < "$O/names.txt")
echo "city textures named: $N"

for state in none on off; do
  prep "$O/data-$state" "$state"
  env -u DISPLAY -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE NFSU2_SHOT_DATA="$O/data-$state" NFSU2_HEADLESS_ONLY=1 \
    NFSU2_SHOT_CLOCK=0 timeout 900 "$GAME" --shot explore "$O/explore-$state.png" > "$O/explore-$state.log" 2>&1 &
  wait $!
  echo "shot $state rc=$?"
done

python3 -I - "$O" << 'EOF'
import struct, sys, zlib
from pathlib import Path
def rgba(path):
    b = Path(path).read_bytes(); p = 8; idat = b''; w = h = 0
    while p < len(b):
        n = struct.unpack('>I', b[p:p + 4])[0]; t = b[p + 4:p + 8]; d = b[p + 8:p + 8 + n]; p += 12 + n
        if t == b'IHDR': w, h, depth, ctype = struct.unpack('>IIBB', d[:10]); assert depth == 8 and ctype in (2, 6), (depth, ctype)
        elif t == b'IDAT': idat += d
    bpp = 4 if ctype == 6 else 3; raw = zlib.decompress(idat); out = bytearray(); prev = bytearray(w * bpp); s = w * bpp
    for y in range(h):
        f = raw[y * (s + 1)]; line = bytearray(raw[y * (s + 1) + 1:(y + 1) * (s + 1)])
        for i in range(s):
            a = line[i - bpp] if i >= bpp else 0; up = prev[i]; c = prev[i - bpp] if i >= bpp else 0
            if f == 1: line[i] = (line[i] + a) & 255
            elif f == 2: line[i] = (line[i] + up) & 255
            elif f == 3: line[i] = (line[i] + (a + up) // 2) & 255
            elif f == 4:
                pa, pb, pc = abs(up - c), abs(a - c), abs(a + up - 2 * c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else up if pb <= pc else c)) & 255
        out += line; prev = line
    return w, h, bpp, out
o = sys.argv[1]
w, h, bpp, none = rgba(f'{o}/explore-none.png')
fail = []
for state in ['on', 'off']:
    w2, h2, _, img = rgba(f'{o}/explore-{state}.png')
    assert (w2, h2) == (w, h)
    px = range(0, w * h * bpp, bpp)
    differ = sum(1 for i in px if img[i:i + 3] != none[i:i + 3])
    magenta = sum(1 for i in px if img[i] > 2 * img[i + 1] + 40 and img[i + 2] > 2 * img[i + 1] + 40)
    share, mshare = differ / (w * h), magenta / (w * h)
    print(f'{state}: {share:.1%} of the pixels differ from the plain shot, {mshare:.1%} magenta')
    if state == 'on' and (share < 0.5 or mshare < 0.25): fail.append('on: the package does not show')
    if state == 'off' and differ: fail.append('off: not the plain shot')
print('TEXTURE_MOD_SHOT: ' + ('PASS' if not fail else 'FAIL (' + '; '.join(fail) + ')'))
sys.exit(1 if fail else 0)
EOF
