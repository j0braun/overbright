#!/bin/bash
# The launcher with the mouse alone, in a real window on a private X display (never the desktop's): the
# folder browser by clicks, double clicks, the wheel and the scroll bar, then a game folder chosen and a
# package added on the Mods page. No key is pressed.
#   a. the launcher opens on the first-run screen (a home folder holding 30 folders, no game found);
#   b. a click on Browse… opens the browser in the home folder;
#   c. the wheel over the list scrolls the list (3 rows a notch) until "Folder 27" shows;
#   d. a double click on "Folder 27" opens it;
#   e. one click on "Deep" chooses it without opening it; the Open button opens it;
#   f. the mouse's back button goes up one folder;
#   g. the Home place goes home; the scroll bar's thumb dragged to the bottom shows the last rows; a click
#      on the track above the thumb pages up;
#   h. a click on a part of the path (the folder above home) goes there;
#   i. Folder 27 > Deep > Target by double clicks, "Use this folder": the check screen for it (refused);
#   w. (Windows / Wine) "This PC" in the path shows the drives; a double click on C:\ opens it; "Up one
#      folder" at the drive's root goes back to the drives;
#   j. (NFSU2_ROOT set) Back, Browse…, the game (a link "NFSU2" in the home folder) by a double click, Use
#      this folder, Use this folder: the main screen; Mods, Add packages…, the package folder by a double
#      click, the package by a double click: the Mods page with the package's result line;
#   k. (Linux, NFSU2_ROOT set) the Saves page: the original's profile in the home folder's ~/.wine found,
#      imported byte for byte (Import), then backed up beside the profiles (Back up).
# The screens are read from the launcher's own output (RUST_LOG launcher::screen=debug: each control's
# box, each list's rows and scroll bar); the window's origin from its cursor lines (launcher::input) after
# one pointer move, so a window manager's or Wine's frame does not matter. Screenshots in <work dir>.
#
# Usage: tests/launcher_mouse.sh <overbright | overbright.exe> <X display | auto> <work dir> [scale]
#   scale: X11's WINIT_X11_SCALE_FACTOR (1, 2, …). OB_WINE=1: `wine <overbright.exe>` in the caller's
#   private WINEPREFIX (pinned to x11, the caller's XDG_RUNTIME_DIR); the home folder is the prefix's profile
#   folder and the scale is the prefix's (HKCU\Control Panel\Desktop LogPixels, set by the caller).
# Last line: "MOUSE: PASS" or "MOUSE: FAIL (<step>)". Exit 0 only on PASS.
set -u -o pipefail
OB=$(cd "$(dirname "${1:?overbright}")" && pwd)/$(basename "$1") DISP=${2:?display} W=${3:?work dir} SCALE=${4:-1}
WINE=${OB_WINE:-0}
for t in xdotool python3; do command -v $t > /dev/null || { echo "MOUSE: FAIL (no $t)"; exit 1; }; done
[ -f "$OB" ] || { echo "MOUSE: FAIL (no $OB)"; exit 1; }
mkdir -p "$W"; W=$(cd "$W" && pwd)
rm -rf "$W"/launcher.out "$W"/screen.txt "$W"/*.png "$W/data" "$W/cache"
XPID="" LPID="" STEP=setup
stop() {
  for p in $LPID $XPID; do kill "$p" 2> /dev/null && wait "$p" 2> /dev/null; done
  [ "$WINE" = 1 ] && wineserver -w 2> /dev/null
}
fail() { echo "FAIL: $*"; stop; echo "MOUSE: FAIL ($STEP)"; exit 1; }
ok() { echo "ok    $STEP: $*"; }

if [ "$DISP" = auto ]; then
  command -v Xvfb > /dev/null || { echo "MOUSE: FAIL (no Xvfb)"; exit 1; }
  exec 9> "$W/xvfb.display"
  Xvfb -displayfd 9 -screen 0 1920x1080x24 -nolisten tcp > "$W/xvfb.log" 2>&1 &
  XPID=$!; exec 9>&-
  for _ in $(seq 100); do [ -s "$W/xvfb.display" ] && break; sleep 0.1; done
  DISP=:$(tr -dc '0-9' < "$W/xvfb.display")
fi
X() { env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE DISPLAY="$DISP" "$@"; }
[ -z "$(X xdotool search --onlyvisible --name . 2> /dev/null)" ] || { echo "MOUSE: FAIL (display $DISP already has windows)"; stop; exit 1; }

# The home folder: a fresh one (Linux) or the prefix's profile folder (Wine: where the launcher's home is).
if [ "$WINE" = 1 ]; then
  [ -n "${WINEPREFIX:-}" ] || { echo "MOUSE: FAIL (OB_WINE=1 without WINEPREFIX)"; exit 1; }
  H=$WINEPREFIX/drive_c/users/$(id -un)
  HW='C:\users\'$(id -un)
else
  H=$W/home; rm -rf "$H"; HW=$H
fi
mkdir -p "$H"
for i in $(seq -w 1 30); do mkdir -p "$H/Folder $i/Other"; done
mkdir -p "$H/Folder 27/Deep/Target" "$H/Folder 27/Deep/Other" "$H/Mods pack"
rm -f "$H/NFSU2"; [ -n "${NFSU2_ROOT:-}" ] && [ -d "$NFSU2_ROOT" ] && ln -s "$NFSU2_ROOT" "$H/NFSU2"
# A profile of the original in the home folder's Wine prefix (the Saves page finds and imports it): the
# game's container (0x6FD3F0) around an empty payload named WINEKID.
python3 -I - "$H/.wine/drive_c/users/me/AppData/Local/NFS Underground 2/WINEKID/WINEKID" << 'PY'
import os, struct, sys
T = []
for i in range(256):
  c = i << 24
  for _ in range(8): c = ((c << 1) ^ 0x04C11DB7) & 0xffffffff if c & 0x80000000 else (c << 1) & 0xffffffff
  T.append(c)
def ck(d):
  r = ~struct.unpack('>I', d[:4])[0] & 0xffffffff
  for b in d[4:]: r = (((r << 8) & 0xffffffff) | b) ^ T[r >> 24]
  return ~r & 0xffffffff
p = bytearray(0xD692); n = 0x993E + 0x2AE5 + 0xDDE; p[n:n + 7] = b'WINEKID'; p = bytes(p)
s1 = struct.pack('<II', 0x10D, len(p))
h = struct.pack('<6I', 0x4D433032, 0x24 + len(p), 8, len(p), ck(s1), ck(p)); h += struct.pack('<I', ck(h))
os.makedirs(os.path.dirname(sys.argv[1]), exist_ok=True); open(sys.argv[1], 'wb').write(h + s1 + p)
PY
# A package made for another game (the Mods page reports it in its result line).
python3 -I - "$H/Mods pack/other-game.tpf" << 'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1], 'w') as z:
    z.writestr('GTA-SA.EXE_0x12345678.dds', b'DDS ' + bytes(124) + bytes(8))
PY
# A folder under home as the launcher names it.
win() { if [ "$WINE" = 1 ]; then printf '%s\\%s' "$HW" "$(printf '%s' "$1" | tr / '\\')"; else printf '%s/%s' "$H" "$1"; fi; }

L=$W/launcher.out
: > "$L"
LOGS="info,launcher::screen=debug,launcher::input=debug"
if [ "$WINE" = 1 ]; then
  mkdir -p "$W/data"
  (exec env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE DISPLAY="$DISP" NFSU2_NO_PADS=1 OVERBRIGHT_NO_PREPARE=1 OVERBRIGHT_OWN_PICKER=1 RUST_LOG="$LOGS" \
    NFSU2_DATA_DIR="$(winepath -w "$W/data" 2> /dev/null)" NFSU2_ROOT= wine "$OB" > "$L" 2>&1) &
else
  (cd "$H" && exec env -i HOME="$H" PATH=/usr/bin:/bin DISPLAY="$DISP" LANG=C.UTF-8 NFSU2_NO_PADS=1 OVERBRIGHT_NO_PREPARE=1 OVERBRIGHT_OWN_PICKER=1 \
    WINIT_X11_SCALE_FACTOR="$SCALE" NFSU2_CACHE_DIR="$W/cache" XDG_DATA_HOME="$W/data" RUST_LOG="$LOGS" "$OB" > "$L" 2>&1) &
fi
LPID=$!

# The last screen traced, into $W/screen.txt (lines without the log's indent); q <what> asks it.
last() { python3 -I -c '
import sys, itertools
t = open(sys.argv[1], errors="replace").read().replace("\r", "")
i = t.rfind("launcher::screen] screen ")
b = t[i:].split("] ", 1)[1].split("\n") if i >= 0 else []
out = b[:1] + list(itertools.takewhile(lambda l: not l.startswith("["), b[1:]))
print("\n".join(l.strip() for l in out))' "$L" > "$W/screen.txt"; }
q() { last; python3 -I - "$W/screen.txt" "$@" << 'EOF'
import re, sys
s = open(sys.argv[1]).read().split("\n")
what = sys.argv[2]
if what == "screen":
    print(s[0].replace("screen ", "").strip())
elif what == "box":  # box <kind> <id>: its centre
    for l in s:
        m = re.match(r"\[%s %s @(-?\d+),(-?\d+) (\d+)x(\d+)\]" % (sys.argv[3], re.escape(sys.argv[4])), l)
        if m:
            x, y, w, h = map(int, m.groups()); print(x + w // 2, y + h // 2); break
elif what == "link":  # link <text>: the id of the link showing it
    for l in s:
        m = re.match(r"\[link (\S+) @[^\]]*\] (.*)$", l)
        if m and (m.group(2) == sys.argv[3] or m.group(2).startswith(sys.argv[3] + " ⟨")):
            print(m.group(1)); break
elif what in ("row", "list", "bar"):
    for i, l in enumerate(s):
        m = re.match(r"\[list %s @(-?\d+),(-?\d+) (\d+)x(\d+)\]" % sys.argv[3], l)
        if not m:
            continue
        x, y, w, h = map(int, m.groups())
        r = re.match(r"rows first=(\d+) shown=(\d+) sel=(\d+) len=(\d+) row_h=(\d+) pad=(\d+)(?: bar=(\d+),(\d+),(\d+),(\d+) thumb=(\d+),(\d+))?: (.*)", s[i + 1])
        first, shown, sel, n, rh, pad = map(int, r.groups()[:6])
        names = r.group(13).split(" | ")
        if what == "list":
            print(first, shown, sel, n)
        elif what == "bar":  # the thumb's centre, the track's bottom and top
            if r.group(7):
                bx, by, bw, bh, ty, th = map(int, r.groups()[6:12])
                print(bx + bw // 2, ty + th // 2, by + bh - 2, by + 2)
        elif sys.argv[4] in names:
            k = names.index(sys.argv[4])
            print(x + w // 3, y + pad + int((k + 0.5) * rh))
        break
elif what == "has":
    print("yes" if any(sys.argv[3] in l for l in s) else "no")
EOF
}
# wait_until <seconds> <command…>: until the command prints something other than nothing / "no".
wait_until() { local n=$(($1 * 10)); shift; for _ in $(seq $n); do v=$("$@"); [ -n "$v" ] && [ "$v" != no ] && return 0; sleep 0.1; done; return 1; }
screen_is() { [ "$(q screen)" = "$1" ] && echo yes; }
row_shown() { [ -n "$(q row dirs "$1")" ] && echo yes; }
in_dir() { q has "⟨$1⟩"; }
WX=0 WY=0
at() { X xdotool mousemove $((WX + $1)) $((WY + $2)); sleep 0.15; }
click() { at "$1" "$2"; X xdotool click 1; sleep 0.35; }
dclick() { at "$1" "$2"; X xdotool click --repeat 2 --delay 80 1; sleep 0.5; }
click_id() { local p; p=$(q box "$1" "$2"); [ -n "$p" ] || fail "no $1 '$2' on $(q screen)"; click $p; }
dclick_row() { local p; p=$(q row dirs "$1"); [ -n "$p" ] || fail "row $1 not shown: $(q list dirs)"; dclick $p; }
wheel_to() { # the pointer over the list, the wheel down until row $1 shows
  local p; p=$(q box list dirs); at $p
  for _ in $(seq 20); do [ -n "$(q row dirs "$1")" ] && return 0; X xdotool click 5; sleep 0.3; done
  [ -n "$(q row dirs "$1")" ]
}

# ---- a ----------------------------------------------------------------------------------------
STEP=a
wait_until 60 screen_is first-run || fail "no first-run screen: $(tail -5 "$L")"
WIN=$(X xdotool search --onlyvisible --name '^Overbright$' | head -1)
[ -n "$WIN" ] || fail "no window on $DISP"
read -r GX GY < <(X xdotool getwindowgeometry "$WIN" | sed -nE 's/.*Position: ([0-9]+),([0-9]+).*/\1 \2/p')
# The client area's origin: a pointer move to a known spot, where the launcher saw it (a new cursor line).
cursors() { grep -c 'launcher::input\] cursor ' "$L"; }
for d in 60 90 120; do
  n=$(cursors)
  X xdotool mousemove $((GX + d)) $((GY + d))
  for _ in $(seq 30); do [ "$(cursors)" -gt "$n" ] && break; sleep 0.1; done
  [ "$(cursors)" -gt "$n" ] && break
done
read -r CX CY S < <(grep -oE 'cursor [0-9.]+,[0-9.]+ scale [0-9.]+' "$L" | tail -1 | sed -E 's/cursor ([0-9.]+),([0-9.]+) scale ([0-9.]+)/\1 \2 \3/')
[ "$(cursors)" -gt "$n" ] && [ -n "${CX:-}" ] || fail "the launcher saw no pointer move"
WX=$((GX + d - ${CX%.*})) WY=$((GY + d - ${CY%.*}))
ok "window $WIN on $DISP, client area at $WX,$WY, scale factor $S"

# ---- b ----------------------------------------------------------------------------------------
STEP=b
click_id button browse
wait_until 5 screen_is browse || fail "Browse… did not open the browser"
wait_until 3 in_dir "$HW" || fail "the browser is not in the home folder $HW: $(grep -m3 crumb "$W/screen.txt")"
read -r F0 SHOWN _ N < <(q list dirs)
[ "$N" -ge 31 ] || fail "at least 31 rows expected (.. and 30 folders), got $N"
X import -window root "$W/browse-$SCALE.png" 2> /dev/null
ok "the browser in $HW: $N rows, $SHOWN shown"

# ---- c ----------------------------------------------------------------------------------------
STEP=c
p=$(q box list dirs); at $p
X xdotool click 5; sleep 0.4
read -r F1 _ < <(q list dirs)
# 3 rows a notch; winit 0.30 on X11 reports XTEST's wheel button both on its press and on its release
# (a real wheel comes as XInput2 scroll motion, once), so 3 or 6 here.
case $((F1 - F0)) in 3 | 6) ;; *) fail "a wheel notch moved the list from $F0 to $F1 (3 rows a notch expected)" ;; esac
wheel_to "Folder 27" || fail "the wheel did not bring Folder 27 into view: $(q list dirs)"
ok "first row $F0 -> $F1 after one notch; Folder 27 in view ($(q list dirs))"

# ---- d ----------------------------------------------------------------------------------------
STEP=d
dclick_row "Folder 27"
wait_until 5 in_dir "$(win "Folder 27")" || fail "a double click did not open Folder 27"
ok "a double click opened Folder 27"

# ---- e ----------------------------------------------------------------------------------------
STEP=e
p=$(q row dirs Deep); click $p
[ "$(in_dir "$(win "Folder 27/Deep")")" = no ] || fail "one click opened Deep"
read -r _ _ SEL _ < <(q list dirs)
click_id button open
wait_until 5 in_dir "$(win "Folder 27/Deep")" || fail "Open did not open the chosen row ($SEL)"
ok "one click chose row $SEL, Open opened Deep"

# ---- f ----------------------------------------------------------------------------------------
STEP=f
X xdotool click 8; sleep 0.5
wait_until 5 in_dir "$(win "Folder 27")" || fail "the back button did not go up to Folder 27"
ok "the mouse's back button went up"

# ---- g ----------------------------------------------------------------------------------------
STEP=g
click_id link "$(q link Home)"
wait_until 5 in_dir "$HW" || fail "the Home place did not go home"
read -r tx ty bottom top < <(q bar dirs)
[ -n "${tx:-}" ] || fail "no scroll bar on a list of $N rows"
at "$tx" "$ty"; X xdotool mousedown 1; sleep 0.15
at "$tx" $((bottom + 40)); sleep 0.2; X xdotool mouseup 1; sleep 0.4
read -r FEND SHEND _ NEND < <(q list dirs)
[ "$FEND" = $((NEND - SHEND)) ] || fail "the thumb dragged to the bottom shows rows from $FEND (expected $((NEND - SHEND)))"
read -r tx ty bottom top < <(q bar dirs)
click "$tx" $((top + 2))
read -r FUP _ < <(q list dirs)
[ "$FUP" -lt "$FEND" ] || fail "a click on the track above the thumb did not page up ($FEND -> $FUP)"
ok "thumb dragged to the end (first row $FEND of $NEND), a click on the track paged up to $FUP"

# ---- h ----------------------------------------------------------------------------------------
STEP=h
UP=$(basename "$(dirname "$H")") HOMEN=$(basename "$H")
id=$(q link "$UP"); [ -n "$id" ] || fail "no path part '$UP': $(grep crumb "$W/screen.txt")"
click_id link "$id"
wait_until 5 row_shown "$HOMEN" || fail "the path part '$UP' did not go there"
ok "the path part '$UP' went to the folder above home"

# ---- i ----------------------------------------------------------------------------------------
STEP=i
dclick_row "$HOMEN"
wait_until 5 in_dir "$HW" || fail "the home row did not open home"
wheel_to "Folder 27" || fail "Folder 27 not in view: $(q list dirs)"
for name in "Folder 27" Deep Target; do
  wait_until 5 row_shown "$name" || fail "$name not in view: $(q list dirs)"
  dclick_row "$name"
done
wait_until 5 in_dir "$(win "Folder 27/Deep/Target")" || fail "Target not opened"
click_id button use
wait_until 5 screen_is check || fail "Use this folder did not check the folder"
q has "Target" | grep -q yes || fail "the check screen is not about Target"
ok "Target chosen by double clicks, Use this folder: the check screen"

# ---- w (Windows) ------------------------------------------------------------------------------
if [ "$WINE" = 1 ]; then
  STEP=w
  click_id button another
  wait_until 5 screen_is browse || fail "Choose another folder… did not open the browser"
  click_id link "$(q link "This PC")"
  wait_until 5 row_shown 'C:\' || fail "This PC did not list the drives: $(q list dirs)"
  q has 'Places:' | grep -q yes || fail "no places"
  dclick_row 'C:\'
  wait_until 5 in_dir 'C:\' || fail "C:\\ did not open"
  click_id button up
  wait_until 5 row_shown 'C:\' || fail "Up at C:\\ did not go to the drives"
  X import -window root "$W/drives.png" 2> /dev/null
  ok "This PC lists the drives, C:\\ opened by a double click, Up at its root goes back to the drives"
  click_id button cancel
fi

# ---- j ----------------------------------------------------------------------------------------
if [ -e "$H/NFSU2" ]; then
  STEP=j
  [ "$(q screen)" = check ] && { click_id button legend:back; wait_until 5 screen_is first-run || fail "Back"; }
  wait_until 5 screen_is first-run || fail "not on the first-run screen: $(q screen)"
  click_id button browse
  wait_until 5 screen_is browse || fail "Browse…"
  click_id link "$(q link Home)"
  wait_until 5 in_dir "$HW" || fail "Home"
  wheel_to NFSU2 || fail "NFSU2 not in view"
  dclick_row NFSU2
  wait_until 5 in_dir "$(win NFSU2)" || fail "the game folder did not open"
  click_id button use
  wait_until 30 eval '[ "$(q screen)" = check ] && [ -n "$(q box button use)" ] && echo yes' || fail "the game folder is not usable: $(cat "$W/screen.txt")"
  click_id button use
  wait_until 10 screen_is main || fail "Use this folder did not reach the main screen"
  ok "the game folder chosen by the mouse: the main screen"
  click_id row row_mods
  wait_until 5 screen_is mods || fail "Mods"
  click_id button add
  wait_until 5 screen_is browse || fail "Add packages…"
  wheel_to "Mods pack" || fail "Mods pack not in view"
  dclick_row "Mods pack"
  wait_until 5 row_shown other-game.tpf || fail "the package is not listed: $(q list dirs)"
  dclick_row other-game.tpf
  done_line() { q screen > /dev/null; grep -m1 -E '^\[(ok|warn|error)\] .*other-game\.tpf' "$W/screen.txt"; }
  wait_until 60 done_line || fail "no result line for the package: $(cat "$W/screen.txt")"
  X import -window root "$W/mods-$SCALE.png" 2> /dev/null
  ok "Mods > Add packages > the package by double clicks: $(done_line | cut -c1-110)"
  if [ "$WINE" != 1 ]; then
    STEP=k
    click_id page page:saves
    wait_until 5 screen_is saves || fail "the Saves page did not open"
    wait_until 30 eval '[ -n "$(q box button src0)" ] && ! grep -q "(off)" <<< "$(grep "button src0" "$W/screen.txt")" && echo yes' \
      || fail "the save in the home folder's Wine prefix was not found: $(cat "$W/screen.txt")"
    click_id button src0
    wait_until 5 eval '[ -n "$(q box button import_go)" ] && echo yes' || fail "no import plan: $(cat "$W/screen.txt")"
    click_id button import_go
    wait_until 30 q has "Imported 1 profile." || fail "not imported: $(cat "$W/screen.txt")"
    cmp -s "$H/.wine/drive_c/users/me/AppData/Local/NFS Underground 2/WINEKID/WINEKID" "$W/data/NFS Underground 2/WINEKID/WINEKID" \
      || fail "the imported profile differs from the original's"
    # A narrow window keeps the profile's actions behind Actions….
    [ -n "$(q box button backup)" ] || { click_id button actions; wait_until 5 eval '[ -n "$(q box button backup)" ] && echo yes'; }
    click_id button backup
    wait_until 30 q has "WINEKID: backed up." || fail "not backed up: $(cat "$W/screen.txt")"
    ls -d "$W/data/NFS Underground 2 Backups/WINEKID"/launcher-* > /dev/null 2>&1 || fail "no backup folder"
    X import -window root "$W/saves-$SCALE.png" 2> /dev/null
    ok "Saves: the original's save found in ~/.wine, imported byte for byte, backed up beside the profiles"
  fi
fi
stop
echo "MOUSE: PASS"
