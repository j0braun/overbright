#!/usr/bin/env python3
"""Checks a Windows release executable (PE32+): every imported DLL (normal and delay-load) is a system
DLL from the allow-list below (no mingw runtime DLLs such as libgcc_s_seh-1.dll, libwinpthread-1.dll or
libstdc++-6.dll, which a player would not have), the subsystem (--gui: GUI, the shipped game), an icon,
a version resource and the application manifest, and no build machine home paths in its strings.

  check_pe.py [--gui] [--list] game.exe     exit 0 when every check passes

Python 3 standard library only.
"""
import argparse
import re
import struct
import sys

# System DLLs present on every supported Windows (10/11). Matching ignores case; api-ms-win-* and
# ext-ms-win-* are the API sets.
ALLOWED = {
    "kernel32.dll", "kernelbase.dll", "ntdll.dll", "user32.dll", "gdi32.dll", "advapi32.dll", "shell32.dll",
    "ole32.dll", "oleaut32.dll", "combase.dll", "ws2_32.dll", "bcrypt.dll", "bcryptprimitives.dll", "msvcrt.dll", "userenv.dll",
    "imm32.dll", "dwmapi.dll", "uxtheme.dll", "winmm.dll", "comctl32.dll", "shlwapi.dll", "setupapi.dll",
    "hid.dll", "cfgmgr32.dll", "propsys.dll", "dinput8.dll", "xinput1_4.dll", "dxgi.dll", "d3d12.dll",
    "d3dcompiler_47.dll", "opengl32.dll", "version.dll", "powrprof.dll", "secur32.dll", "crypt32.dll",
    "ksuser.dll", "avrt.dll", "mmdevapi.dll", "synchronization.dll", "dbghelp.dll", "winhttp.dll",
}
ALLOWED_PREFIXES = ("api-ms-win-", "ext-ms-win-")
SUBSYSTEMS = {2: "GUI", 3: "console"}
HOME_PATTERNS = [rb"/home/", rb"C:\\Users\\", rb"c:\\users\\"]


class Pe:
    def __init__(self, data):
        self.data = data
        if data[:2] != b"MZ":
            raise ValueError("not an MZ executable")
        pe = struct.unpack_from("<I", data, 0x3C)[0]
        if data[pe:pe + 4] != b"PE\0\0":
            raise ValueError("no PE header")
        nsec, = struct.unpack_from("<H", data, pe + 6)
        opt_size, = struct.unpack_from("<H", data, pe + 20)
        opt = pe + 24
        magic, = struct.unpack_from("<H", data, opt)
        if magic != 0x20B:
            raise ValueError("not PE32+ (64-bit)")
        self.subsystem, = struct.unpack_from("<H", data, opt + 68)
        ndirs, = struct.unpack_from("<I", data, opt + 108)
        self.dirs = [struct.unpack_from("<II", data, opt + 112 + 8 * i) for i in range(ndirs)]
        self.sections = []
        for i in range(nsec):
            s = opt + opt_size + 40 * i
            vsize, va, rsize, raw = struct.unpack_from("<IIII", data, s + 8)
            self.sections.append((va, max(vsize, rsize), raw))

    def off(self, rva):
        for va, size, raw in self.sections:
            if va <= rva < va + size:
                return rva - va + raw
        raise ValueError(f"rva {rva:#x} outside the sections")

    def cstr(self, rva):
        o = self.off(rva)
        return self.data[o:self.data.index(b"\0", o)].decode("ascii", "replace")

    def dir(self, i):
        return self.dirs[i] if i < len(self.dirs) else (0, 0)

    def imports(self):
        names = []
        rva, size = self.dir(1)
        if rva:
            o = self.off(rva)
            while True:
                ilt, _, _, name, iat = struct.unpack_from("<IIIII", self.data, o)
                if not (ilt or name or iat):
                    break
                names.append(self.cstr(name))
                o += 20
        return names

    def delay_imports(self):
        names = []
        rva, size = self.dir(13)
        if rva:
            o = self.off(rva)
            while True:
                attrs, name = struct.unpack_from("<II", self.data, o)
                if not name:
                    break
                names.append(self.cstr(name))
                o += 32
        return names

    def resource_types(self):
        rva, size = self.dir(2)
        if not rva:
            return []
        o = self.off(rva)
        named, ids = struct.unpack_from("<HH", self.data, o + 12)
        types = []
        for i in range(named + ids):
            ident, _ = struct.unpack_from("<II", self.data, o + 16 + 8 * i)
            if not ident & 0x80000000:
                types.append(ident)
        return types


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("exe")
    ap.add_argument("--gui", action="store_true", help="require the GUI subsystem (the shipped game)")
    ap.add_argument("--list", action="store_true", help="print every imported DLL")
    a = ap.parse_args()
    data = open(a.exe, "rb").read()
    pe = Pe(data)
    fail = False

    def report(good, msg):
        nonlocal fail
        print(("ok    " if good else "FAIL  ") + msg)
        fail |= not good

    # One entry per DLL (the linker emits one import descriptor per import library).
    dlls, seen = [], set()
    for n, kind in [(n, "") for n in pe.imports()] + [(n, " (delay-load)") for n in pe.delay_imports()]:
        if (n.lower(), kind) not in seen:
            seen.add((n.lower(), kind))
            dlls.append((n.lower(), kind))
    dlls.sort()
    if a.list:
        for n, kind in dlls:
            print(f"      import {n}{kind}")
    bad = [n + k for n, k in dlls if n.lower() not in ALLOWED and not n.lower().startswith(ALLOWED_PREFIXES)]
    report(not bad, f"{len(dlls)} imported DLLs, all on the allow-list" if not bad
           else "imports outside the allow-list: " + ", ".join(bad))
    sub = SUBSYSTEMS.get(pe.subsystem, str(pe.subsystem))
    if a.gui:
        report(pe.subsystem == 2, f"subsystem {sub}")
    else:
        print(f"      subsystem {sub}")
    types = pe.resource_types()
    report(14 in types, "icon resource (RT_GROUP_ICON)")
    report(16 in types, "version resource (RT_VERSION)")
    # The application manifest (DPI awareness, long paths, UTF-8 code page).
    report(24 in types and b"PerMonitorV2" in data and b"longPathAware" in data,
           "manifest resource (RT_MANIFEST: PerMonitorV2, longPathAware)")
    hits = sum(len(re.findall(p, data)) for p in HOME_PATTERNS)
    report(hits == 0, f"strings: {hits} home paths (/home/, C:\\Users\\)")
    return 1 if fail else 0


if __name__ == "__main__":
    sys.exit(main())
