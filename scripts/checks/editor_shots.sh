#!/bin/bash
# The map editor's shot matrix (docs/developing/checks.md "Editor shots", docs/modding/map-editor.md): `overbright-editor --shot` at
# every size of the resolution matrix (scripts/checks/resolutions.py, docs/notes/graphics/resolutions.md) and a few display
# scales, on three screens, headless: the editor draws its whole window off-screen, and every run starts without
# any display variable (DISPLAY, WAYLAND_DISPLAY unset), so a run that tried to open a window would fail.
#
#   scripts/checks/editor_shots.sh OUTDIR [EDITOR_BIN]
#
# EDITOR_BIN: default $CARGO_TARGET_DIR/<checks profile>/overbright-editor (checks.sh's build makes it).
# Sizes whose window in logical points (the size divided by the scale) is below the editor's smallest window,
# 960x600 (its window cannot be made smaller), are listed as "n/a" and not run.
# Screens (a map mod made here, holds no edits; each cell has its own copy of the maps folder, as the editor
# keeps its recent list there):
#   start      no map mod open: the editor's opening screen
#   props_3d   the mod open on the city (L4RA), a prop's side in front, the 3D view, the prop browser
#   races_map  the same mod, the races' side in front, the top view with the road network (View ▸ Roads), the validation list
# Each cell must: exit 0; have no panic / validation-error line in its log; write a PNG of exactly the asked pixel
# size; not be one flat colour (its standard deviation, alpha off, at least $EDITOR_MIN_STD, default 0.02); and on the
# two world screens show something in the centre of the window, where the view is (the middle 30 % of the image at
# least $EDITOR_MIN_STD too), and say "editor layout: toolbar fits" (nothing in the toolbar cut off or on top of
# another item; the editor checks its own layout and logs "toolbar clipped: …" otherwise).
# Repeatable: the first world cell is drawn a second time and must be the same picture, pixel for pixel (a --shot
# runs on a fixed clock; EDITOR_REPEAT=0 skips this).
# Env: EDITOR_SET=all (default: every size of the matrix) | family (one size per aspect family; checks.sh full);
#      EDITOR_SIZES=WxH[@scale],... (instead of the set), EDITOR_SCREENS=a,b (a subset), JOBS (default 2: each world
#      screen loads the city), EDITOR_SCALES (extra cells at another display scale, default
#      "2560x1440@1.5 3840x2160@2"; empty: none).
# Output: OUTDIR/results.txt (one line per cell: PASS|FAIL|n/a, the cell, why; "repeat" for the second drawing),
# cells/<cell>/{shot.png,shot.log,rc}.
# Last line: "EDITOR_SHOTS: N cells, P pass, F fail; K sizes under the smallest window (S s)". Exit 0 every cell passed, 1 a cell failed, 2 a setup
# error. Test tooling only; every process is waited for by PID.
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/../lib/common.sh"
ck_local_env
ck_need_root
ck_need_tools python3 timeout xargs
if command -v magick > /dev/null; then IM=(magick); else ck_need_tools convert; IM=(); fi
O=${1:?usage: editor_shots.sh OUTDIR [EDITOR_BIN]}
EBIN=${2:-${CARGO_TARGET_DIR:-$CK_REPO/target}/$CK_PDIR/overbright-editor}
[ -x "$EBIN" ] || ck_die "no editor binary $EBIN (cargo build --profile $CK_PROFILE --bins)"
mkdir -p "$O"; O=$(cd "$O" && pwd)
rm -rf "$O/cells" "$O/maps" "$O/tmp"; mkdir -p "$O/cells" "$O/maps/shots" "$O/tmp"
# The game's asset cache: the caller's NFSU2_CACHE_DIR, else a folder of this run (never the player's cache).
export NFSU2_CACHE_DIR="${NFSU2_CACHE_DIR:-$O/cache}"
printf 'overbright-map 1\nname = Editor shots\n' > "$O/maps/shots/map.obmap"
T0=$(date +%s)

# The cells: "<cell>\t<W>x<H>\t<scale>\t<screen>" (sizes from the resolution matrix); n/a.tsv: those under the
# smallest window.
SCREENS=${EDITOR_SCREENS:-start,props_3d,races_map}
python3 -I - "$HERE" "${EDITOR_SET:-all}" "${EDITOR_SIZES:-}" "$SCREENS" "${EDITOR_SCALES-2560x1440@1.5 3840x2160@2}" \
  3> "$O/na.tsv" > "$O/cells.tsv" << 'PY' || ck_die "no cell plan"
import os, sys
na = os.fdopen(3, "w")
here, which, sizes, screens, scales = sys.argv[1:]
sys.path.insert(0, here)
import resolutions as r
if sizes:
    cells = [s for s in sizes.split(",") if s]
else:
    cells = r.FAMILY_SET if which == "family" else [f"{w}x{h}" for w, h, _ in r.MATRIX]
    cells = cells + scales.split()
known = {"start", "props_3d", "races_map"}
for sc in screens.split(","):
    if sc not in known:
        sys.exit(f"unknown screen {sc} ({', '.join(sorted(known))})")
for c in cells:
    size, _, scale = c.partition("@")
    w, h = (int(v) for v in size.split("x"))
    if w / float(scale or 1) < 960 or h / float(scale or 1) < 600:
        print(c, file=na)
        continue
    for sc in screens.split(","):
        print(f"{w}x{h}{'@' + scale if scale else ''}__{sc}\t{w}x{h}\t{scale or '1'}\t{sc}")
PY
N=$(grep -c . "$O/cells.tsv")
[ "$N" -gt 0 ] || ck_die "an empty matrix"

# one_cell LINE: one run, headless (no display variable at all), its own data and cache folders under OUTDIR.
one_cell() {
  local cell size scale screen d args=()
  IFS=$'\t' read -r cell size scale screen <<< "$1"
  d="$O/cells/$cell"; rm -rf "$d"; mkdir -p "$d/data" "$d/xcache"; cp -r "$O/maps" "$d/maps"
  case $screen in
    start) ;;
    props_3d) args=(--mod shots --mode props --view 3d --tab browser) ;;
    races_map) args=(--mod shots --mode races --view top --tab validation) ;;
  esac
  env -u DISPLAY -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE -u WAYLAND_SOCKET XDG_DATA_HOME="$d/data" XDG_CACHE_HOME="$d/xcache" \
    TMPDIR="$O/tmp" RADV_DEBUG=${RADV_DEBUG-nodcc} NFSU2_HEADLESS_ONLY=1 \
    timeout 600 "$EBIN" --shot "$d/shot.png" --size "$size" --scale "$scale" --maps "$d/maps" "${args[@]}" > "$d/shot.log" 2>&1
  echo $? > "$d/rc"
}
export -f one_cell; export O EBIN
cut -f1- "$O/cells.tsv" | tr '\n' '\0' | xargs -0 -P "${JOBS:-2}" -I{} bash -c 'one_cell "$1"' _ {} &
wait $!

# The verdicts.
MIN=${EDITOR_MIN_STD:-0.02}
: > "$O/results.txt"
NF=0
while IFS=$'\t' read -r cell size scale screen; do
  d="$O/cells/$cell" why=""
  rc=$(cat "$d/rc" 2> /dev/null || echo none)
  [ "$rc" = 0 ] || why="$why rc=$rc;"
  bad=$(ck_bad "$d/shot.log")
  [ "$bad" = 0 ] || why="$why $bad panic/validation lines;"
  if [ -s "$d/shot.png" ] && [ "$(head -c 8 "$d/shot.png" | od -An -tx1 | tr -d ' \n')" = 89504e470d0a1a0a ]; then
    read -r w h std < <("${IM[@]}" identify -format '%w %h ' "$d/shot.png" 2> /dev/null;
      "${IM[@]}" convert "$d/shot.png" -alpha off -colorspace gray -format '%[fx:standard_deviation]\n' info: 2> /dev/null)
    [ "${w:-0}x${h:-0}" = "$size" ] || why="$why the image is ${w:-?}x${h:-?}, not $size;"
    awk -v s="${std:-0}" -v m="$MIN" 'BEGIN { exit !(s >= m) }' || why="$why one flat colour (std ${std:-?} < $MIN);"
    if [ "$screen" != start ]; then
      if grep -q 'editor layout: toolbar clipped' "$d/shot.log"; then
        why="$why $(grep -m1 -o 'toolbar clipped: .*' "$d/shot.log" | cut -c1-160);"
      elif ! grep -q 'editor layout: toolbar fits' "$d/shot.log"; then
        why="$why no toolbar layout line;"
      fi
      cstd=$("${IM[@]}" convert "$d/shot.png" -alpha off -colorspace gray -gravity center -crop 30%x30%+0+0 +repage \
        -format '%[fx:standard_deviation]' info: 2> /dev/null)
      awk -v s="${cstd:-0}" -v m="$MIN" 'BEGIN { exit !(s >= m) }' || why="$why nothing in the view (centre std ${cstd:-?} < $MIN);"
    fi
  else
    why="$why no PNG written;"
  fi
  if [ -z "$why" ]; then echo "PASS $cell (std $std${cstd:+, centre $cstd})" >> "$O/results.txt"
  else echo "FAIL $cell:$why" >> "$O/results.txt"; NF=$((NF + 1)); fi
  cstd=""
done < "$O/cells.tsv"
# Repeatable: the first world cell once more, compared pixel for pixel.
if [ "${EDITOR_REPEAT:-1}" != 0 ] && first=$(grep -m1 -v $'\tstart$' "$O/cells.tsv"); then
  cell=${first%%$'\t'*}
  one_cell "$cell.repeat${first#"$cell"}"
  a="$O/cells/$cell/shot.png" b="$O/cells/$cell.repeat/shot.png"
  same=no
  if [ -s "$a" ] && [ -s "$b" ]; then
    ha=$("${IM[@]}" convert "$a" rgba:- 2> /dev/null | md5sum) hb=$("${IM[@]}" convert "$b" rgba:- 2> /dev/null | md5sum)
    [ "$ha" = "$hb" ] && same=yes
    diff=$("${IM[@]}" compare -metric AE "$a" "$b" null: 2>&1 | awk '{print $1}')
  else
    diff="no PNG"
  fi
  N=$((N + 1))
  if [ "$same" = yes ]; then echo "PASS repeat $cell (the same picture twice)" >> "$O/results.txt"
  else echo "FAIL repeat $cell: a second run drew another picture (compare AE ${diff:-?})" >> "$O/results.txt"; NF=$((NF + 1)); fi
fi
rm -rf "$O/tmp"
NA=$(grep -c . "$O/na.tsv")
while read -r c; do echo "n/a  $c: under the editor's smallest window (960x600 points)" >> "$O/results.txt"; done < "$O/na.tsv"
grep '^FAIL' "$O/results.txt" | head -20
echo "EDITOR_SHOTS: $N cells, $((N - NF)) pass, $NF fail; $NA sizes under the smallest window ($(( $(date +%s) - T0 )) s)" | tee -a "$O/results.txt"
[ "$NF" = 0 ]
