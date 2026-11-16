"""The launcher's folder browser by the mouse in a private, headless Wayland session (tests/launcher_wayland.sh).

Runs inside that session's own D-Bus: scales mutter's virtual monitor (org.gnome.Mutter.DisplayConfig), then
moves the pointer, clicks and scrolls through org.gnome.Mutter.RemoteDesktop (relative motion; the window's
origin is found from the launcher's own cursor lines), and reads the screens from the launcher's log.

  launcher_wayland.py scale <factor>
  launcher_wayland.py drive <launcher log> <scale>
Prints one line per step and "RESULT PASS" or "RESULT FAIL <why>".
"""

import re
import sys
import time

from gi.repository import Gio, GLib


def bus():
    return Gio.bus_get_sync(Gio.BusType.SESSION)


def set_scale(want):
    dc = Gio.DBusProxy.new_sync(bus(), 0, None, "org.gnome.Mutter.DisplayConfig", "/org/gnome/Mutter/DisplayConfig",
                                "org.gnome.Mutter.DisplayConfig")
    serial, monitors, _logical, _props = dc.call_sync("GetCurrentState", None, 0, -1).unpack()
    (conn, _vendor, _product, _ser), modes, _mprops = monitors[0]
    mode = next(m for m in modes if m[6].get("is-current", False))
    best = min(mode[5], key=lambda s: abs(s - want))
    cfg = GLib.Variant("(uua(iiduba(ssa{sv}))a{sv})", (serial, 1, [(0, 0, best, 0, True, [(conn, mode[0], {})])], {}))
    dc.call_sync("ApplyMonitorsConfig", cfg, 0, -1)
    print("monitor %s %s at scale %s (asked %s)" % (conn, mode[0], best, want))


def drive(log, scale):
    rd = Gio.DBusProxy.new_sync(bus(), 0, None, "org.gnome.Mutter.RemoteDesktop", "/org/gnome/Mutter/RemoteDesktop",
                                "org.gnome.Mutter.RemoteDesktop")
    path = rd.call_sync("CreateSession", None, 0, -1).unpack()[0]
    s = Gio.DBusProxy.new_sync(bus(), 0, None, "org.gnome.Mutter.RemoteDesktop", path,
                               "org.gnome.Mutter.RemoteDesktop.Session")
    s.call_sync("Start", None, 0, -1)

    def call(name, sig, args, wait=0.0):
        s.call_sync(name, GLib.Variant(sig, args), 0, -1)
        time.sleep(wait)

    def text():
        return open(log, errors="replace").read()

    def cursor():
        m = re.findall(r"cursor ([\d.]+),([\d.]+) scale ([\d.]+)", text())
        return tuple(map(float, m[-1])) if m else None

    def box(kind, id_):
        m = re.findall(r"\[%s %s @(-?\d+),(-?\d+) (\d+)x(\d+)\]" % (kind, id_), text())
        return tuple(map(float, m[-1])) if m else None

    def screen():
        m = re.findall(r"launcher::screen\] screen (\S+)", text())
        return m[-1] if m else None

    def rows():
        m = re.findall(r"rows first=(\d+) shown=(\d+) sel=(\d+) len=(\d+) row_h=(\d+) pad=(\d+)", text())
        return tuple(map(int, m[-1])) if m else None

    def here():
        m = re.findall(r"\[link crumb\d+ [^\]]*\] [^⟨\n]* ⟨([^⟩]*)⟩", text())
        return m[-1] if m else None

    def until(f, secs=5.0):
        end = time.time() + secs
        while time.time() < end:
            v = f()
            if v:
                return v
            time.sleep(0.1)
        return f()

    def fail(why):
        print("RESULT FAIL " + why)
        sys.exit(1)

    pos = [0.0, 0.0]
    call("NotifyPointerMotionRelative", "(dd)", (-5000.0, -5000.0), 0.15)
    call("NotifyPointerMotionRelative", "(dd)", (960.0, 540.0), 0.4)
    pos[:] = [960.0, 540.0]
    c = until(cursor)
    if not c:
        fail("the launcher saw no pointer (the window is not under the screen's middle)")
    got = c[2]
    if abs(got - scale) > 0.01:
        fail("winit reports scale %s, the monitor has %s" % (got, scale))
    ox, oy = 960 - c[0] / got, 540 - c[1] / got
    print("scale %s: the window's origin at %.1f,%.1f (logical); cursor in physical pixels %s" % (got, ox, oy, c[:2]))

    def goto(px, py):
        tx, ty = ox + px / got, oy + py / got
        call("NotifyPointerMotionRelative", "(dd)", (tx - pos[0], ty - pos[1]), 0.2)
        pos[:] = [tx, ty]

    def click(n=1):
        for _ in range(n):
            for st in (True, False):
                call("NotifyPointerButton", "(ib)", (272, st), 0.04)
        time.sleep(0.5)

    b = until(lambda: box("button", "browse"), 20)
    if not b:
        fail("no Browse… button")
    goto(b[0] + b[2] / 2, b[1] + b[3] / 2)
    click()
    if until(lambda: screen() == "browse") is not True:
        fail("a click on Browse… did not open the browser (screen %s)" % screen())
    print("a click on Browse… opened the browser")
    lb = box("list", "dirs")
    goto(lb[0] + lb[2] / 2, lb[1] + lb[3] / 2)
    r0 = rows()
    call("NotifyPointerAxisDiscrete", "(ui)", (0, 1), 0.5)
    r1 = rows()
    if r1[0] - r0[0] != 3:
        fail("a wheel notch moved the list from %d to %d (3 expected)" % (r0[0], r1[0]))
    print("a wheel notch: first row %d -> %d" % (r0[0], r1[0]))
    # A touchpad: 10 small steps of 10 logical px, then the scroll's end.
    for _ in range(10):
        call("NotifyPointerAxis", "(ddu)", (0.0, 10.0, 0), 0.02)
    call("NotifyPointerAxis", "(ddu)", (0.0, 0.0, 1), 0.5)
    r2 = rows()
    want = int(100 * got // r2[4])
    if r2[0] - r1[0] != want:
        fail("100 logical px of touchpad scrolling moved %d rows (%d expected at %d px a row)"
             % (r2[0] - r1[0], want, r2[4]))
    print("100 logical px of touchpad scrolling: first row %d -> %d (%d px a row)" % (r1[0], r2[0], r2[4]))
    before = here()
    k = 3
    goto(lb[0] + lb[2] / 3, lb[1] + r2[5] + (k + 0.5) * r2[4])
    click(2)
    after = until(lambda: here() if here() != before else None)
    if not after:
        fail("a double click on a row did not open it")
    print("a double click on the shown row %d opened %s" % (k, after.rsplit("/", 1)[-1]))
    print("RESULT PASS")


if __name__ == "__main__":
    if sys.argv[1] == "scale":
        set_scale(float(sys.argv[2]))
    else:
        drive(sys.argv[2], float(sys.argv[3]))
