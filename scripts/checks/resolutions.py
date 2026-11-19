#!/usr/bin/env python3
"""The resolution matrix (docs/resolutions.md). Test tooling only.

  resolutions.py plan [--set all|family] [--sizes WxH,...] [--screens a,b] [--mouse]
      one job per line: cell<TAB>size<TAB>option<TAB>screen<TAB>target<TAB>env (space-separated K=V)
  resolutions.py selftest OUTDIR
      breaks passing cells of a run one thing at a time and asserts each check fails (the self-test)
  resolutions.py check OUTDIR
      reads OUTDIR/cells/<cell>/{shot.png,shot.log,geom.json,rc}; writes OUTDIR/results.txt (one line per
      (size, option, screen): PASS|FAIL and the reasons), OUTDIR/results.tsv, OUTDIR/table.md (aspect x screen
      pass/fail counts), OUTDIR/causes.txt (failures grouped by check), contact sheets OUTDIR/sheet_<aspect>_<option>.jpg.

Needs numpy and ImageMagick (convert, montage). The geometry comes from the game's dev-harness dump
(NFSU2_UI_GEOM, src/render/native/ui/geom_dump.rs).
"""
import json
import math
import os
import re
import subprocess
import sys
from collections import defaultdict

import numpy as np

# ImageMagick 7 (`magick`) or 6 (`convert`, `identify`, `montage`).
import shutil
IM = ["magick"] if shutil.which("magick") else []

# The matrix (W, H, family). Families group sizes of one aspect for the downscale compare.
MATRIX = [
    (640, 480, "4:3"), (800, 600, "4:3"), (1024, 768, "4:3"), (1280, 960, "4:3"), (1600, 1200, "4:3"),
    (1280, 1024, "5:4"),
    (1280, 800, "16:10"), (1440, 900, "16:10"), (1680, 1050, "16:10"), (1920, 1200, "16:10"), (2560, 1600, "16:10"),
    (1280, 720, "16:9"), (1366, 768, "16:9"), (1600, 900, "16:9"), (1920, 1080, "16:9"), (2560, 1440, "16:9"),
    (3840, 2160, "16:9"),
    (2160, 1440, "3:2"), (2256, 1504, "3:2"),
    (2560, 1080, "21:9"), (3440, 1440, "21:9"), (3840, 1600, "21:9"),
    (3840, 1080, "32:9"), (5120, 1440, "32:9"),
    (1080, 1920, "9:16"),
]
# 640x480 is the smallest window (display::MIN_WINDOW, the original's smallest mode); 320x240 left the
# matrix (a smaller requested size is raised to it).
MIN_SIZE = (640, 480)
# One size per family plus the odd ones (the minimum 640x480, the portrait): the FULL checks set's pass.
FAMILY_SET = ["640x480", "1280x1024", "1680x1050", "1366x768", "2256x1504", "3440x1440", "5120x1440", "1080x1920"]
# Known failures that are the option's own rule, reported as XFAIL (not counted as failures) when that check is
# their only failure: Stretched is the original's behaviour (Vert-, 0x5C7ED0), which at 9:16 grows the vertical
# angle to ~161 deg; the road under the chase camera then lies nearer than the 0.5 m near plane (a band of the clear
# colour at the bottom). SPEED2.EXE has no mode narrower than 5:4; Wide is the layout for portrait.
KNOWN = {
    ("1080x1920", "stretched", "freeroam_hud"): "flat",
    ("1080x1920", "stretched", "race_hud"): "flat",
    ("1080x1920", "stretched", "pause"): "flat",
    ("1080x1920", "stretched", "hdr_calibration_pause"): "flat",
    # The calibration page's help line quotes the HDR pass's cost at the shot's own size ("+0.56 ms at
    # 1600x1200"), so its text differs from the 640x480 reference's (no layout change).
    ("1600x1200", "stretched", "hdr_calibration_pause"): "resind",
    ("1600x1200", "wide", "hdr_calibration_pause"): "resind",
}

# screen -> (shot target, kind). kind: "fe" front end over the garage, "fe2d" front end without a 3D
# scene, "game" 3D view with HUD / menu, "pillar" a 4:3 picture pillarboxed (movie, loading), "nis" a
# scene with cinematic bars.
SCREENS = {
    "main": ("main", "fe"),
    "options_sliders": ("options_audio", "fe"),
    "settings_hub": ("options_enhancements", "fe"),
    "settings_main_enhanced": ("settings_main_enhanced", "fe"),
    "settings_advanced": ("settings_advanced", "fe"),
    "car_select": ("qrcars", "fe"),
    "parts_browser": ("parts", "fe"),
    "world_map": ("worldmap", "game"),
    "race_hud": ("lapko", "game"),
    "freeroam_hud": ("explore", "game"),
    "pause": ("pause", "game"),
    "loading": ("loading", "pillar"),
    "dialog": ("dialog", "fe"),
    "movie": ("movie", "pillar"),
    "photo": ("photo", "game"),
    "post_race": ("results", "fe"),
    "race_start": ("qrstart", "nis"),
    "hdr_calibration": ("settings_display", "fe"),
    "hdr_calibration_pause": ("pause_settings_display", "game"),
}
# Screens rendered with HDR Output on a faked HDR10 surface (`NFSU2_HDR_FAKE`): the calibration page, whose test
# pattern (drawn by the display transform, `<shot>.hdr-preview.png`) must sit in the page's free rows: inside the
# menu, over no row, text or other widget (the `hdr` check).
SCREEN_ENV = {
    "hdr_calibration": {"NFSU2_HDR_FAKE": "pq", "NFSU2_SHOT_ENH": "enh.hdr_output 1"},
    "hdr_calibration_pause": {"NFSU2_HDR_FAKE": "pq", "NFSU2_SHOT_ENH": "enh.hdr_output 1"},
}
HDR_SCREENS = set(SCREEN_ENV)
HDR_RE = re.compile(r"^hdr pattern: (none|px ([-\d.]+) ([-\d.]+) ([-\d.]+) ([-\d.]+))$", re.M)
HUD_SCREENS = {"race_hud", "freeroam_hud", "photo"}
# Options the settings offer for the display: Aspect Ratio (Wide = the clone's default, Stretched = the
# original's behaviour) and, for the HUD, HUD Layout Full Width (Enhancements).
OPTIONS = {
    "stretched": {"NFSU2_ASPECT": "stretched"},
    "wide": {"NFSU2_ASPECT": "wide"},
    "wide_hudfull": {"NFSU2_ASPECT": "wide", "NFSU2_SHOT_ENH": "enh.hud_full_width 1",
                     "NFSU2_PHOTO_ENH": "enh.hud_full_width 1"},
}
# The existing mouse shots (`--shot mouse_*`, src/bin/game/dev/shot): each asserts that a pointer put on
# a widget acts on it; run at every size / aspect of the set.
MOUSE_SHOTS = ["mouse_main", "mouse_options", "mouse_settings", "mouse_parts", "mouse_map", "mouse_carlot",
               "mouse_lists", "mouse_controls"]

FONT_RE = re.compile(r"(^FONT_|_TT\d+)")


def sz(s):
    w, h = s.split("x")
    return int(w), int(h)


def family_of(w, h):
    for mw, mh, f in MATRIX:
        if (mw, mh) == (w, h):
            return f
    g = math.gcd(w, h)
    return f"{w // g}:{h // g}"


def plan(args):
    which = "all"
    sizes = None
    screens = list(SCREENS)
    mouse = False
    it = iter(args)
    for a in it:
        if a == "--set":
            which = next(it)
        elif a == "--sizes":
            sizes = [x for x in next(it).split(",") if x]
        elif a == "--screens":
            screens = [x for x in next(it).split(",") if x]
        elif a == "--mouse":
            mouse = True
    if sizes is None:
        sizes = FAMILY_SET if which == "family" else [f"{w}x{h}" for w, h, _ in MATRIX]
    for s in sizes:
        for sc in screens:
            target = SCREENS[sc][0]
            for opt in OPTIONS:
                if opt == "wide_hudfull" and sc not in HUD_SCREENS:
                    continue
                env = dict(OPTIONS[opt])
                env.update(SCREEN_ENV.get(sc, {}))
                env["NFSU2_SHOT_SIZE"] = s
                env["NFSU2_PHOTO_SIZE"] = s
                cell = f"{s}__{opt}__{sc}"
                print("\t".join([cell, s, opt, sc, target, " ".join(f"{k}={v.replace(' ', '%20')}" for k, v in env.items())]))
        if mouse:
            for opt in ("wide", "stretched"):
                for m in MOUSE_SHOTS:
                    env = dict(OPTIONS[opt])
                    env["NFSU2_SHOT_SIZE"] = s
                    print("\t".join([f"{s}__{opt}__{m}", s, opt, m, m,
                                     " ".join(f"{k}={v.replace(' ', '%20')}" for k, v in env.items())]))


# ---------------------------------------------------------------- images

def load_rgb(path, size=None):
    """The PNG as an HxWx3 uint8 array (ImageMagick), resized to `size` (w, h) when given."""
    cmd = (IM or ["convert"]) + [path, "-alpha", "off"]
    if size:
        cmd += ["-filter", "Box", "-resize", f"{size[0]}x{size[1]}!"]
    cmd += ["-depth", "8", "rgb:-"]
    raw = subprocess.run(cmd, capture_output=True, check=True).stdout
    if size is None:
        dims = subprocess.run(IM + ["identify", "-format", "%w %h", path], capture_output=True, text=True,
                              check=True).stdout.split()
        size = (int(dims[0]), int(dims[1]))
    return np.frombuffer(raw, dtype=np.uint8).reshape(size[1], size[0], 3)


def diff_stats(a, b):
    d = np.abs(a.astype(np.int16) - b.astype(np.int16)).max(axis=2)
    return float(d.mean()), float((d > 48).mean())


# ---------------------------------------------------------------- expected rules

def expected_frame(w, h, mode, hud_full):
    """The UI frame the option's rule gives (docs/resolutions.md "Rules"), computed here independently."""
    if mode == "stretched":
        return dict(canvas=(0, 0, w, h), scale=(w / 640, h / 480), reach=(0, 0), hud_reach=0)
    if w * 3 > h * 4:
        cw = min(round(h * 4 / 3), w)
        canvas = (math.floor((w - cw) / 2), 0, cw, h)
    elif w * 3 < h * 4:
        ch = min(round(w * 0.75), h)
        canvas = (0, math.floor((h - ch) / 2), w, ch)
    else:
        canvas = (0, 0, w, h)
    cx, cy, cw, ch = canvas

    def reach(spread):
        region = h * spread if spread and w > h * spread * 1.01 else w
        return max(0, min(math.floor((region - cw) / 2), cx))

    safe = reach(16 / 9)
    return dict(canvas=canvas, scale=(cw / 640, ch / 480), reach=(safe, cy), hud_reach=reach(None) if hud_full else safe)


def expected_view(aspect, mode):
    """(h/w, m00 factor) of the option's FOV rule: Wide = the vertical angle of 4:3 at every aspect (Hor+ wider
    than 4:3, the 4:3 view's sides cropped narrower); Stretched = the original's Vert- (horizontal angle
    fixed)."""
    if mode == "wide":
        return 0.75, 4 / 3 / aspect
    return 1 / aspect, 1.0


# ---------------------------------------------------------------- per-cell checks

def inter(a, b):
    x0, y0, x1, y1 = max(a[0], b[0]), max(a[1], b[1]), min(a[2], b[2]), min(a[3], b[3])
    return (x0, y0, x1, y1) if x1 > x0 and y1 > y0 else None


def area(r):
    return 0.0 if r is None else (r[2] - r[0]) * (r[3] - r[1])


def check_cell(cell_dir, w, h, opt, screen, kind):
    fails, notes = [], []
    rc_file = os.path.join(cell_dir, "rc")
    rc = open(rc_file).read().strip() if os.path.exists(rc_file) else "missing"
    log = os.path.join(cell_dir, "shot.log")
    png = os.path.join(cell_dir, "shot.png")
    text = open(log, errors="replace").read() if os.path.exists(log) else ""
    if rc != "0":
        tail = text.strip().splitlines()[-1][:140] if text.strip() else ""
        fails.append(("run", f"rc {rc}: {tail}"))
    if re.search(r"panicked|validation error", text, re.I):
        fails.append(("run", "panic/validation in log"))
    if not os.path.exists(png):
        fails.append(("run", "no PNG"))
        return fails, notes, None
    if screen.startswith("mouse_"):
        return fails, notes, None
    gpath = os.path.join(cell_dir, "geom.json")
    if not os.path.exists(gpath):
        fails.append(("dump", "no geometry dump (UI pass not reached)"))
        return fails, notes, None
    g = json.load(open(gpath))
    if screen in HDR_SCREENS:
        m = HDR_RE.findall(text)
        g["hdr_pattern"] = None if not m or m[-1][0] == "none" else [float(v) for v in m[-1][1:]]
        prev = os.path.join(cell_dir, "shot.hdr-preview.png")
        if not os.path.exists(prev):
            fails.append(("hdr", "no HDR preview written (HDR Output not on)"))
        elif g["hdr_pattern"] is not None:
            try:
                fails += hdr_pixels(load_rgb(prev), g["hdr_pattern"])
            except Exception as ex:  # noqa: BLE001
                fails.append(("hdr", f"unreadable HDR preview ({ex})"))
    judge(g, w, h, opt, screen, kind, fails, notes)
    return fails, notes, g


def hdr_pattern_check(g, fails, notes):
    """The calibration pattern's box (the game's log line): present, inside the menu's canvas, and over no drawn
    quad except ones that hold it whole (the panel, full-screen fills): no row, text, arrow or scroll bar under it."""
    r = g.get("hdr_pattern")
    if r is None:
        fails.append(("hdr", "no calibration pattern on the calibration page"))
        return
    cx, cy, cw, ch = g["canvas"]
    if r[0] < cx - 1 or r[1] < cy - 1 or r[2] > cx + cw + 1 or r[3] > cy + ch + 1:
        fails.append(("hdr", f"pattern {[round(v) for v in r]} outside the menu's canvas {[round(v) for v in g['canvas']]}"))
    if r[2] - r[0] < 8 or r[3] - r[1] < 8:
        fails.append(("hdr", f"pattern {[round(v) for v in r]} too small"))
    over = []
    for q in g["quads"]:
        px = q["px"]
        sc = q["scissor"]
        vis = inter(inter(px, (sc[0], sc[1], sc[0] + sc[2], sc[1] + sc[3])) or (0, 0, 0, 0), (0, 0, *g["size"]))
        if vis is None or area(vis) <= 4:
            continue
        hit = inter(vis, r)
        # Shared edges and anti-aliased fringes are not an overlap.
        if hit is None or (hit[2] - hit[0]) <= 1.5 or (hit[3] - hit[1]) <= 1.5:
            continue
        if vis[0] <= r[0] + 1 and vis[1] <= r[1] + 1 and vis[2] >= r[2] - 1 and vis[3] >= r[3] - 1:
            continue  # holds the pattern whole: a backdrop
        over.append(q["tex"] or "flat")
    if over:
        fails.append(("hdr", "pattern over " + ", ".join(sorted(set(over))[:6])))
    notes.append(f"hdr pattern {[round(v) for v in r]}")


def hdr_pixels(img, r):
    """The preview shows the pattern at the logged box: its frame is black, its peak box bright."""
    h, w = img.shape[:2]
    x0, y0, x1, y1 = (int(math.ceil(r[0])) + 1, int(math.ceil(r[1])) + 1, int(r[2]) - 2, int(r[3]) - 2)
    if x1 - x0 < 4 or y1 - y0 < 4 or x0 < 0 or y0 < 0 or x1 >= w or y1 >= h:
        return [("hdr", f"pattern box {[round(v) for v in r]} off the image")]
    lum = img.astype(np.float32).mean(axis=2)
    ring = np.concatenate([lum[y0, x0:x1], lum[y1, x0:x1], lum[y0:y1, x0], lum[y0:y1, x1]])
    fails = []
    if (ring <= 3).mean() < 0.9:
        fails.append(("hdr", f"no pattern frame at the logged box (black {(ring <= 3).mean() * 100:.0f}%)"))
    pk = lum[(y0 + y1) // 2, x0 + int((x1 - x0) * 0.125)]
    if pk < 200:
        fails.append(("hdr", f"no peak box at the logged box (luma {pk:.0f})"))
    return fails


def judge(g, w, h, opt, screen, kind, fails, notes):
    """The geometry checks of one cell (canvas, reach, cut, outside, scale, text, mouse, fov) on its dump."""
    if screen in HDR_SCREENS:
        hdr_pattern_check(g, fails, notes)
    mode = g["aspect_mode"]
    want_mode = "stretched" if opt == "stretched" else "wide"
    if mode != want_mode:
        fails.append(("option", f"aspect mode {mode}, option {want_mode}"))
    if tuple(g["size"]) != (w, h):
        fails.append(("size", f"UI target {g['size'][0]}x{g['size'][1]} not {w}x{h}"))
    e = expected_frame(w, h, want_mode, opt == "wide_hudfull")
    # 1. the canvas rectangle and the reaches.
    if [round(v, 2) for v in g["canvas"]] != [round(v, 2) for v in e["canvas"]] or \
            abs(g["origin"][0] - e["canvas"][0]) > 0.01 or abs(g["origin"][1] - e["canvas"][1]) > 0.01:
        fails.append(("canvas", f"canvas {g['canvas']} expected {list(e['canvas'])}"))
    if abs(g["scale"][0] - e["scale"][0]) > 1e-3 or abs(g["scale"][1] - e["scale"][1]) > 1e-3:
        fails.append(("canvas", f"scale {g['scale']} expected {[round(v, 4) for v in e['scale']]}"))
    if abs(g["reach"][0] - e["reach"][0]) > 0.01 or abs(g["reach"][1] - e["reach"][1]) > 0.01 or \
            abs(g["hud_reach"] - e["hud_reach"]) > 0.01:
        fails.append(("reach", f"reach {g['reach']}/{g['hud_reach']} expected {list(e['reach'])}/{e['hud_reach']}"))
    sx_f, sy_f = g["scale"]
    ox, oy = g["origin"]
    cw, ch = 640 * sx_f, 480 * sy_f
    target = (0, 0, w, h)
    cut, outside, nonuni, textbad = [], [], [], []
    for q in g["quads"]:
        if q["angle"] != 0:
            continue
        r = q["rect"]
        lo = (min(r[0], r[2]), min(r[1], r[3]))
        hi = (max(r[0], r[2]), max(r[1], r[3]))
        cwq, chq = hi[0] - lo[0], hi[1] - lo[1]
        px = q["px"]
        sc = q["scissor"]
        sbox = (sc[0], sc[1], sc[0] + sc[2], sc[1] + sc[3])
        vis = inter(inter(px, sbox) or (0, 0, 0, 0), target)
        name = q["tex"] or "flat"
        if q["flat"] or q["tex"] is None or cwq < 0.5 or chq < 0.5:
            continue  # fills reach the edges by design (fades, dims, bars, backings)
        # 2a. a picture the original shows whole (canvas rect inside 0..640 x 0..480) cut off here.
        inside = lo[0] >= -0.5 and lo[1] >= -0.5 and hi[0] <= 640.5 and hi[1] <= 480.5
        # (more than 1.5 px lost on a side: the canvas edge's half-pixel shift is not a cut)
        if inside and area(px) > 4:
            # what lies past the canvas edge (up to 0.5 units) is off screen natively too
            sxq, syq = (px[2] - px[0]) / cwq, (px[3] - px[1]) / chq
            over = (max(0, -lo[0]) * sxq, max(0, -lo[1]) * syq, max(0, hi[0] - 640) * sxq, max(0, hi[1] - 480) * syq)
            if vis is None or max(vis[0] - px[0] - over[0], vis[1] - px[1] - over[1], px[2] - vis[2] - over[2],
                                  px[3] - vis[3] - over[3]) > 1.5:
                cut.append(name)
        # 2b. drawn outside the canvas as its place moves it (natively off screen, now visible).
        pl = q["place"]
        rx = g["hud_reach"] if pl["hud"] else g["reach"][0]
        box = (ox + pl["x"] * rx - 1, oy + pl["y"] * g["reach"][1] - 1, ox + pl["x"] * rx + cw + 1,
               oy + pl["y"] * g["reach"][1] + ch + 1)
        if pl["stretch"]:
            box = (box[0] - rx, box[1], box[2] + rx, box[3])
        if vis is not None and area(vis) > 4 and area(inter(vis, box)) < area(vis) - 2 * (vis[3] - vis[1]) - 2:
            outside.append(name)
        # 3. uniform scale (and the same scale as the frame).
        if cwq >= 2 and chq >= 2 and not pl["stretch"]:
            sx, sy = (px[2] - px[0]) / cwq, (px[3] - px[1]) / chq
            if want_mode == "wide":
                bad = abs(sx / sy - 1) > 0.01 or abs(sx / sx_f - 1) > 0.01
            else:
                bad = abs(sx / sx_f - 1) > 0.01 or abs(sy / sy_f - 1) > 0.01
            if bad:
                (textbad if FONT_RE.search(name) else nonuni).append(f"{name}({sx:.2f}x{sy:.2f})")
    def uniq(v):
        out = []
        for x in v:
            if x not in out:
                out.append(x)
        return out
    if cut:
        fails.append(("cut", "cut off: " + ", ".join(uniq(cut)[:6])))
    if outside:
        fails.append(("outside", "drawn outside its canvas: " + ", ".join(uniq(outside)[:6])))
    if nonuni:
        fails.append(("scale", "scale off the frame's: " + ", ".join(uniq(nonuni)[:5])))
    if textbad:
        fails.append(("text", "glyph scale off the frame's: " + ", ".join(uniq(textbad)[:4])))
    # 4. mouse: the game's own hit test at the centre of each recorded widget as drawn.
    mbad = [m for m in g.get("mouse", []) if m["hit"] is False]
    moff = []
    for m in g.get("mouse", []):
        if m["drawn_px"] and m["mouse_centre_px"]:
            d, c = m["drawn_px"], m["mouse_centre_px"]
            if not (d[0] - 1 <= c[0] <= d[2] + 1 and d[1] - 1 <= c[1] <= d[3] + 1):
                moff.append(m)
    if mbad:
        fails.append(("mouse", f"{len(mbad)} widgets miss the hit test at their drawn centre "
                               f"({', '.join(sorted({m['pkg'] for m in mbad}))})"))
    if moff:
        # A group whose children go to different sides (the help bar's) is tested with its first quad's
        # place: its rectangle's centre is not on what was drawn. Reported, not failed (groups are not
        # what the screens hit-test).
        notes.append(f"{len(moff)} group rects off their drawn box")
    nm = sum(1 for m in g.get("mouse", []) if m["hit"] is not None)
    if nm:
        notes.append(f"mouse {nm} widgets")
    # 5. the 3D view's FOV rule.
    asp = w / h
    projs = g.get("projections", [])
    main = [p for p in projs if abs(p["aspect"] / asp - 1) < 1e-3]
    if kind in ("game", "nis") and not projs and screen not in ("pause", "world_map", "hdr_calibration_pause"):
        fails.append(("fov", "no 3D projection recorded"))
    if projs and not main:
        fails.append(("fov", f"no projection at the target's aspect {asp:.4f} (got {sorted({round(p['aspect'], 4) for p in projs})})"))
    gx, gy = g.get("fov_consts", [1.1, 1.265])
    for p in main:
        if p["fov"] < 0:
            # A free camera (photo mode): a fixed vertical angle, i.e. Hor+ under every option.
            m11 = 1 / math.tan(-p["fov"] * 0.5)
            if abs(p["m11"] / m11 - 1) > 1e-3 or abs(p["m00"] * asp / m11 - 1) > 1e-3:
                fails.append(("fov", f"free camera m00/m11 {p['m00']:.4f}/{p['m11']:.4f} not the vertical-angle rule"))
            elif want_mode == "stretched" or asp < 4 / 3:
                notes.append("free camera Hor+ (photo mode keeps its vertical angle under every option)")
            continue
        hw, across = expected_view(asp, want_mode)
        m00 = across / math.tan(p["fov"] * gx * 0.5)
        m11 = 1 / math.tan(p["fov"] * gy * hw * 0.5)
        if abs(p["m00"] / m00 - 1) > 1e-3 or abs(p["m11"] / m11 - 1) > 1e-3:
            fails.append(("fov", f"fov {p['fov']:.3f}: m00/m11 {p['m00']:.4f}/{p['m11']:.4f}, rule gives {m00:.4f}/{m11:.4f}"))
            break


def bars_check(img, g, w, h, opt, kind):
    """Black bars exactly where the option puts them."""
    fails = []
    lum = img.astype(np.float32).mean(axis=2)
    cx, cy, cw, ch = g["canvas"]
    cx, cy, cw, ch = int(cx), int(cy), int(cw), int(ch)
    wide = opt != "stretched"
    if kind == "pillar":
        if wide and cx >= 2:
            side = np.concatenate([lum[:, :cx - 1], lum[:, cx + cw + 1:]], axis=1)
            if side.size and side.max() > 6:
                fails.append(("bars", f"pillarbox not black (max {side.max():.0f})"))
        if wide and cy >= 2:
            side = np.concatenate([lum[:cy - 1], lum[cy + ch + 1:]], axis=0)
            if side.size and side.max() > 6:
                fails.append(("bars", f"letterbox not black (max {side.max():.0f})"))
        inner = lum[cy:cy + ch, cx:cx + cw]
        if inner.size and inner.max() < 8:
            fails.append(("bars", "picture area black"))
    else:
        bars = g.get("scene_bars")
        edge = max(1, w // 100)
        if bars:
            x = int(bars[0] - bars[1])
            if x > 1 and lum[:, :x - 1].max() > 6:
                fails.append(("bars", "scene-cap bars not black"))
        elif kind in ("fe", "game", "nis") and g.get("projections"):
            for name, strip in (("left", lum[:, :edge]), ("right", lum[:, w - edge:])):
                if strip.max() < 4:
                    fails.append(("bars", f"unexpected black bar at the {name} edge"))
        # A flat, non-black band along an edge of a 3D screen: nothing of the scene drawn there (the clear
        # colour), e.g. an extreme vertical angle. Normal frames stay below ~0.6 uniform (a garage floor in portrait 0.96).
        if g.get("projections"):
            sh, sw = max(2, h // 50), max(2, w // 50)
            for name, strip in (("top", img[:sh]), ("bottom", img[h - sh:]), ("left", img[:, :sw]),
                                ("right", img[:, w - sw:])):
                px_ = strip.reshape(-1, 3).astype(np.int16)
                med = np.median(px_, axis=0)
                if med.max() > 20 and (np.abs(px_ - med).max(axis=1) <= 3).mean() >= 0.98:
                    fails.append(("flat", f"flat {name} band of colour {med.astype(int).tolist()} (no scene drawn)"))
        if kind == "nis":
            band = lum[: max(1, h // 20)]
            if band.mean() > 4:
                fails.append(("bars", f"cinematic bar does not span the top (mean {band.mean():.1f})"))
    return fails


def check(outdir):
    cells_dir = os.path.join(outdir, "cells")
    jobs = [l.rstrip("\n").split("\t") for l in open(os.path.join(outdir, "jobs.tsv")) if l.strip()]
    results = {}
    geoms = {}
    for cell, s, opt, screen, target, _env in jobs:
        w, h = sz(s)
        kind = SCREENS.get(screen, (screen, "mouse"))[1]
        d = os.path.join(cells_dir, cell)
        fails, notes, g = check_cell(d, w, h, opt, screen, kind)
        png = os.path.join(d, "shot.png")
        if g is not None and os.path.exists(png):
            try:
                fails += bars_check(load_rgb(png), g, w, h, opt, kind)
            except Exception as ex:  # noqa: BLE001
                fails.append(("image", f"unreadable PNG ({ex})"))
            geoms[cell] = g
        results[cell] = dict(size=s, w=w, h=h, opt=opt, screen=screen, target=target, fails=fails, notes=notes,
                             fam=family_of(w, h))
    # 6. resolution independence: within an aspect family, each size downscaled to the family's smallest
    # matrix size (320x240 is checked against 640x480 the other way: 640x480 down to 320x240).
    # Only sizes of the same aspect (within 0.3 %: 1366x768 counts as 16:9) are compared; the 21:9 sizes are
    # three different aspects (2.370, 2.389, 2.400) and are compared with none.
    def same_aspect(a, b):
        return abs((a["w"] / a["h"]) / (b["w"] / b["h"]) - 1) < 0.003

    groups = defaultdict(list)
    for r in results.values():
        if not r["screen"].startswith("mouse_"):
            groups[(r["fam"], r["opt"], r["screen"])].append(r)
    for cell, r in results.items():
        if r["screen"].startswith("mouse_"):
            continue
        peers = [p for p in groups[(r["fam"], r["opt"], r["screen"])] if same_aspect(p, r) and p["w"] != 320]
        if not peers:
            continue
        ref = min(peers, key=lambda p: p["w"])
        if ref is r:
            continue
        a_cell = cell
        b_cell = f"{ref['size']}__{r['opt']}__{r['screen']}"
        a_png = os.path.join(cells_dir, a_cell, "shot.png")
        b_png = os.path.join(cells_dir, b_cell, "shot.png")
        if not (os.path.exists(a_png) and os.path.exists(b_png)):
            continue
        try:
            if r["w"] < ref["w"]:
                small, big = load_rgb(a_png), load_rgb(b_png, (r["w"], r["h"]))
            else:
                small, big = load_rgb(b_png), load_rgb(a_png, (ref["w"], ref["h"]))
            mean, frac = diff_stats(small, big)
        except Exception as ex:  # noqa: BLE001
            r["fails"].append(("image", f"compare failed ({ex})"))
            continue
        r["notes"].append(f"vs {ref['size']}: mean {mean:.2f}, {frac * 100:.2f}% px")
        # Calibrated on the 4:3 / 16:9 / 16:10 families (2026-11-11): finer 3D detail and texture filtering at a
        # higher size give a mean up to ~6 with 1-2 % of pixels off at edges (no layout change); a layout or
        # scale change moves whole elements (several % of the pixels).
        if mean > 10.0 or frac > 0.03:
            r["fails"].append(("resind", f"differs from {ref['size']} beyond resolution (mean {mean:.1f}, "
                                         f"{frac * 100:.1f}% of pixels > 48)"))
    # Output.
    lines, tsv = [], []
    table = defaultdict(lambda: [0, 0])
    causes = defaultdict(list)
    nxf = 0
    for cell, _s, _opt, _screen, _t, _e in jobs:
        r = results[cell]
        known = KNOWN.get((r["size"], r["opt"], r["screen"]))
        xfail = bool(r["fails"]) and known is not None and all(k == known for k, _m in r["fails"])
        r["xfail"] = xfail
        nxf += xfail
        ok = not r["fails"] or xfail
        verdict = "XFAIL" if xfail else "PASS" if ok else "FAIL"
        reason = "; ".join(f"[{k}] {m}" for k, m in r["fails"]) if r["fails"] else ", ".join(r["notes"])
        if xfail:
            reason = "known, the option's own rule (KNOWN in resolutions.py): " + reason
        lines.append(f"{verdict} {r['size']} {r['opt']} {r['screen']} ({r['target']}): {reason}")
        tsv.append("\t".join([r["size"], r["fam"], r["opt"], r["screen"], r["target"], verdict, reason]))
        t = table[(r["fam"], r["screen"])]
        t[0 if ok else 1] += 1
        if not xfail:
            for k, m in r["fails"]:
                causes[k].append(f"{r['size']} {r['opt']} {r['screen']}: {m}")
    open(os.path.join(outdir, "results.txt"), "w").write("\n".join(lines) + "\n")
    open(os.path.join(outdir, "results.tsv"), "w").write(
        "size\tfamily\toption\tscreen\ttarget\tverdict\treason\n" + "\n".join(tsv) + "\n")
    fams = []
    for _w, _h, f in MATRIX:
        if f not in fams:
            fams.append(f)
    fams = [f for f in fams if any(k[0] == f for k in table)]
    screens = [s for s in list(SCREENS) + MOUSE_SHOTS if any(k[1] == s for k in table)]
    md = ["| screen | " + " | ".join(fams) + " |", "|---|" + "---|" * len(fams)]
    for s in screens:
        row = []
        for f in fams:
            p, fl = table.get((f, s), (0, 0))
            row.append("" if p + fl == 0 else (f"{p}/{p + fl}" if fl == 0 else f"**{p}/{p + fl}**"))
        md.append(f"| {s} | " + " | ".join(row) + " |")
    open(os.path.join(outdir, "table.md"), "w").write("pass/total per aspect family and screen (bold: some fail)\n\n"
                                                         + "\n".join(md) + "\n")
    with open(os.path.join(outdir, "causes.txt"), "w") as f:
        for k, v in sorted(causes.items(), key=lambda kv: -len(kv[1])):
            f.write(f"== {k}: {len(v)} cells\n")
            for x in v:
                f.write(f"  {x}\n")
    # Contact sheets per family and option.
    if os.environ.get("RES_SHEETS", "1") != "0":
        for f in fams:
            for opt in OPTIONS:
                pngs = []
                for cell, s, o, screen, _t, _e in jobs:
                    r = results[cell]
                    if r["fam"] == f and o == opt and not screen.startswith("mouse_"):
                        p = os.path.join(cells_dir, cell, "shot.png")
                        if os.path.exists(p):
                            pngs.append((p, f"{s} {screen}{'' if not r['fails'] else ' XFAIL' if r['xfail'] else ' FAIL'}"))
                if not pngs:
                    continue
                out = os.path.join(outdir, f"sheet_{f.replace(':', 'x')}_{opt}.jpg")
                cmd = IM + ["montage", "-pointsize", "11", "-background", "#222", "-fill", "white"]
                for p, label in pngs:
                    cmd += ["-label", label, p]
                cmd += ["-tile", "5x", "-geometry", "320x240+3+3", "-quality", "82", out]
                subprocess.run(cmd, capture_output=True)
    n = len(results)
    nf = sum(1 for r in results.values() if r["fails"] and not r["xfail"])
    print(f"RESOLUTIONS: {n} cells, {n - nf} pass ({nxf} known), {nf} fail; by check: "
          + (", ".join(f"{k} {len(v)}" for k, v in sorted(causes.items(), key=lambda kv: -len(kv[1]))) or "none"))
    return 1 if nf else 0


# ---------------------------------------------------------------- self-test

def selftest(outdir):
    """Each check must fail on broken input (the self-test): takes cells of a run that pass, breaks one thing at a time in
    memory (the dump or the image) and asserts that exactly the check meant for it fails. Prints one line per
    check; returns 1 when a check did not catch its breakage or no passing cell of a kind exists."""
    import copy
    cells_dir = os.path.join(outdir, "cells")
    jobs = [l.rstrip("\n").split("\t") for l in open(os.path.join(outdir, "jobs.tsv")) if l.strip()]

    def passing(pred):
        for cell, s_, opt, screen, _t, _e in jobs:
            w, h = sz(s_)
            kind = SCREENS.get(screen, (screen, "mouse"))[1]
            if kind == "mouse" or not pred(w, h, opt, screen, kind):
                continue
            d = os.path.join(cells_dir, cell)
            fails, _n, g = check_cell(d, w, h, opt, screen, kind)
            if g is None or fails:
                continue
            img = load_rgb(os.path.join(d, "shot.png"))
            if bars_check(img, g, w, h, opt, kind):
                continue
            return dict(d=d, w=w, h=h, opt=opt, screen=screen, kind=kind, g=g, img=img)
        return None

    def run(c, g=None, img=None):
        g = c["g"] if g is None else g
        img = c["img"] if img is None else img
        fails, notes = [], []
        judge(g, c["w"], c["h"], c["opt"], c["screen"], c["kind"], fails, notes)
        fails += bars_check(img, g, c["w"], c["h"], c["opt"], c["kind"])
        return {k for k, _m in fails}

    def textured(g, font):
        for q in g["quads"]:
            r = q["rect"]
            if q["angle"] or q["flat"] or q["tex"] is None or q["place"]["stretch"]:
                continue
            if bool(FONT_RE.search(q["tex"])) != font:
                continue
            lo, hi = (min(r[0], r[2]), min(r[1], r[3])), (max(r[0], r[2]), max(r[1], r[3]))
            if hi[0] - lo[0] >= 8 and hi[1] - lo[1] >= 8 and lo[0] >= 0 and lo[1] >= 0 and hi[0] <= 640 and hi[1] <= 480:
                return q
        return None

    results = []

    def expect(name, c, broken, want):
        if c is None:
            results.append((name, False, "no passing cell to break"))
            return
        got = broken(c)
        ok = got is not None and want in got
        results.append((name, ok, f"{os.path.basename(c['d'])}: fails {sorted(got) if got is not None else 'n/a'}"))

    def mut_g(c, f):
        g = copy.deepcopy(c["g"])
        return run(c, g=g) if f(g) is not False else None

    def mut_img(c, f):
        img = c["img"].copy()
        f(img)
        return run(c, img=img)

    wide_fe = passing(lambda w, h, o, s_, k: o == "wide" and w * 3 > h * 4 and k == "fe")
    wide_game = passing(lambda w, h, o, s_, k: o == "wide" and k == "game" and s_ == "freeroam_hud")
    tall_game = passing(lambda w, h, o, s_, k: o == "wide" and h > w and k == "game" and s_ == "freeroam_hud")
    pillar = passing(lambda w, h, o, s_, k: o == "wide" and w * 3 > h * 4 and k == "pillar")
    hdr_cell = passing(lambda w, h, o, s_, k: s_ in HDR_SCREENS)
    mouse_cell = wide_fe

    def shift(q, dx, dy=0):
        q["px"] = [q["px"][0] + dx, q["px"][1] + dy, q["px"][2] + dx, q["px"][3] + dy]

    expect("canvas", wide_fe, lambda c: mut_g(c, lambda g: g.__setitem__("canvas", [g["canvas"][0] + 10] + g["canvas"][1:])), "canvas")
    expect("reach", wide_fe, lambda c: mut_g(c, lambda g: g.__setitem__("reach", [g["reach"][0] + 7, g["reach"][1]])), "reach")

    def cut(g):
        q = textured(g, False)
        if q is None:
            return False
        q["scissor"] = [q["scissor"][0], q["scissor"][1], 0, 0]
    expect("cut", wide_fe, lambda c: mut_g(c, cut), "cut")

    def outside(g):
        q = textured(g, False)
        if q is None:
            return False
        # into the pillar left of the canvas (on screen, outside the canvas as its place moves it)
        cx = g["canvas"][0] + (g["hud_reach"] if q["place"]["hud"] else g["reach"][0]) * q["place"]["x"]
        if cx < 8:
            return False
        q["px"] = [0, q["px"][1], cx - 2, q["px"][3]]
        q["scissor"] = [0, 0, 1 << 20, 1 << 20]
    expect("outside", wide_fe, lambda c: mut_g(c, outside), "outside")

    def stretch(font):
        def f(g):
            q = textured(g, font)
            if q is None:
                return False
            q["px"] = [q["px"][0], q["px"][1], q["px"][0] + (q["px"][2] - q["px"][0]) * 1.5, q["px"][3]]
        return f
    expect("scale", wide_fe, lambda c: mut_g(c, stretch(False)), "scale")
    expect("text", wide_fe, lambda c: mut_g(c, stretch(True)), "text")

    def miss(g):
        ms = [m for m in g.get("mouse", []) if m["hit"]]
        if not ms:
            return False
        ms[0]["hit"] = False
    expect("mouse", mouse_cell, lambda c: mut_g(c, miss), "mouse")

    def fov(g):
        ps = [p for p in g.get("projections", []) if p["fov"] > 0]
        if not ps:
            return False
        for p in ps:
            p["m11"] *= 1.1
    expect("fov (wide screen)", wide_game, lambda c: mut_g(c, fov), "fov")
    # The old rule in portrait (before the resolution fixes): Vert- (vertical angle grown by (4/3) / aspect).
    def vert_minus(g):
        asp = g["size"][0] / g["size"][1]
        ps = [p for p in g.get("projections", []) if p["fov"] > 0 and abs(p["aspect"] / asp - 1) < 1e-3]
        if not ps:
            return False
        gx, gy = g.get("fov_consts", [1.1, 1.265])
        for p in ps:
            p["m00"] = 1 / math.tan(p["fov"] * gx * 0.5)
            p["m11"] = 1 / math.tan(p["fov"] * gy * (1 / asp) * 0.5)
    expect("fov (portrait, old Vert- rule)", tall_game, lambda c: mut_g(c, vert_minus), "fov")

    def band(img):
        img[int(img.shape[0] * 0.85):] = (70, 40, 110)
    expect("flat", tall_game, lambda c: mut_img(c, band), "flat")

    def grey_bar(img):
        img[:, :4] = (100, 100, 100)
    expect("bars", pillar, lambda c: mut_img(c, grey_bar), "bars")

    def black_side(img):
        img[:, :max(2, img.shape[1] // 50)] = 0
    expect("bars (3D side bar)", wide_game, lambda c: mut_img(c, black_side), "bars")

    # hdr: the pattern moved onto a row's text, or gone.
    def hdr_onto_text(g):
        q = textured(g, True)
        if q is None or g.get("hdr_pattern") is None:
            return False
        p = q["px"]
        g["hdr_pattern"] = [p[0] - 4, p[1] - 4, p[0] + 60, p[3] + 4]
    expect("hdr (pattern over a row)", hdr_cell, lambda c: mut_g(c, hdr_onto_text), "hdr")
    expect("hdr (no pattern)", hdr_cell, lambda c: mut_g(c, lambda g: g.__setitem__("hdr_pattern", None)), "hdr")
    if hdr_cell is not None and hdr_cell["g"].get("hdr_pattern"):
        prev = load_rgb(os.path.join(hdr_cell["d"], "shot.hdr-preview.png"))
        r = hdr_cell["g"]["hdr_pattern"]
        moved = [r[0], r[1] - (r[3] - r[1]) * 1.5, r[2], r[1] - (r[3] - r[1]) * 0.5]
        got = {k for k, _m in hdr_pixels(prev, moved)}
        results.append(("hdr (logged box off the drawn pattern)", "hdr" in got and not hdr_pixels(prev, r),
                        f"{os.path.basename(hdr_cell['d'])}: moved {sorted(got)}"))
    # resind: the compare's thresholds on a frame against itself with a fifth of it moved.
    if wide_game is not None:
        a = wide_game["img"]
        b = a.copy()
        hh = a.shape[0] // 5
        b[:hh] = np.roll(a[:hh], a.shape[1] // 3, axis=1)
        b[:hh] = 255 - b[:hh]
        mean, frac = diff_stats(a, b)
        same = diff_stats(a, a.copy())
        ok = (mean > 10.0 or frac > 0.03) and same == (0.0, 0.0)
        results.append(("resind", ok, f"moved fifth: mean {mean:.1f}, {frac * 100:.1f}% px; same frame {same}"))
    else:
        results.append(("resind", False, "no passing cell"))

    # run: a non-zero rc and a panic line fail the cell.
    if wide_fe is not None:
        import tempfile
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as t:
            for f in ("shot.png", "geom.json"):
                os.symlink(os.path.join(wide_fe["d"], f), os.path.join(t, f))
            open(os.path.join(t, "rc"), "w").write("101\n")
            open(os.path.join(t, "shot.log"), "w").write("thread 'main' panicked at x\n")
            fails, _n, _g = check_cell(t, wide_fe["w"], wide_fe["h"], wide_fe["opt"], wide_fe["screen"], wide_fe["kind"])
            results.append(("run", any(k == "run" for k, _m in fails), f"rc 101 + panic: {sorted({k for k, _m in fails})}"))
            os.remove(os.path.join(t, "geom.json"))
            open(os.path.join(t, "rc"), "w").write("0\n")
            open(os.path.join(t, "shot.log"), "w").write("")
            fails, _n, _g = check_cell(t, wide_fe["w"], wide_fe["h"], wide_fe["opt"], wide_fe["screen"], wide_fe["kind"])
            results.append(("dump", any(k == "dump" for k, _m in fails), f"no geom.json: {sorted({k for k, _m in fails})}"))
    bad = [r for r in results if not r[1]]
    for name, ok, msg in results:
        print(f"selftest {'ok  ' if ok else 'MISS'} {name}: {msg}")
    print(f"RESOLUTIONS SELFTEST: {len(results) - len(bad)}/{len(results)} checks fail on broken input"
          + (f"; missed: {', '.join(r[0] for r in bad)}" if bad else ""))
    return 1 if bad else 0


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(2)
    if sys.argv[1] == "plan":
        plan(sys.argv[2:])
    elif sys.argv[1] == "check":
        sys.exit(check(sys.argv[2]))
    elif sys.argv[1] == "selftest":
        sys.exit(selftest(sys.argv[2]))
    else:
        print(__doc__)
        sys.exit(2)
