#!/usr/bin/env python3
"""Cold file cache without root (docs/developing/checks.md "Load-speed gate").

    coldcache.py drop PATH...      every regular file under PATH flushed and dropped from the OS's file cache;
                                   prints "dropped N MB, resident after M MB (K of F files still partly resident)"
    coldcache.py resident PATH...  how much of the files is in the OS's file cache now (mincore)
    coldcache.py copy SRC DST      a private copy of SRC (a fresh set of inodes: no other process maps or reads
                                   them, so a drop leaves nothing behind), written and synced, then dropped

`echo 3 > /proc/sys/vm/drop_caches` needs root. This needs none: posix_fadvise(POSIX_FADV_DONTNEED) on a file the
caller can open drops that file's clean, unmapped pages (after fdatasync, every page is clean). Pages another
process has mapped or keeps reading stay; that is why the load gate reads a private copy (copy) and why every
drop reports what is still resident (mincore over a read-only mapping). Reading through O_DIRECT would bypass
the cache instead of emptying it, which the game's own reads (mapped files, buffered reads) cannot do.
Linux only (posix_fadvise and mincore); elsewhere it says so and exits 2.
"""
import ctypes
import ctypes.util
import mmap
import os
import shutil
import sys

PAGE = os.sysconf("SC_PAGE_SIZE") if hasattr(os, "sysconf") else 4096


def files(paths):
    for p in paths:
        if os.path.isfile(p):
            yield p
        elif os.path.isdir(p):
            for d, _, names in os.walk(p):
                for n in names:
                    f = os.path.join(d, n)
                    if os.path.isfile(f) and not os.path.islink(f):
                        yield f


_libc = None


def libc():
    global _libc
    if _libc is None:
        _libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
        _libc.mmap.restype = ctypes.c_void_p
        _libc.mmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_long]
        _libc.munmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        _libc.mincore.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p]
    return _libc


def resident_bytes(path):
    """Bytes of PATH in the OS's file cache (mincore over a read-only shared mapping; mapping touches no page)."""
    size = os.path.getsize(path)
    if size == 0:
        return 0
    fd = os.open(path, os.O_RDONLY)
    try:
        c = libc()
        addr = c.mmap(None, size, mmap.PROT_READ, mmap.MAP_SHARED, fd, 0)
        if addr in (None, ctypes.c_void_p(-1).value):
            return 0
        try:
            n = (size + PAGE - 1) // PAGE
            vec = (ctypes.c_ubyte * n)()
            if c.mincore(ctypes.c_void_p(addr), size, vec) != 0:
                return 0
            return sum(b & 1 for b in vec) * PAGE
        finally:
            c.munmap(ctypes.c_void_p(addr), size)
    finally:
        os.close(fd)


def drop(paths):
    total = res = partly = nfiles = 0
    for f in files(paths):
        nfiles += 1
        try:
            fd = os.open(f, os.O_RDONLY)
        except OSError:
            continue
        try:
            os.fdatasync(fd)
            os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
            total += os.fstat(fd).st_size
        finally:
            os.close(fd)
        r = resident_bytes(f)
        res += r
        partly += r > 0
    print(f"dropped {total / 1e6:.0f} MB, resident after {res / 1e6:.1f} MB ({partly} of {nfiles} files still partly resident)")
    return 0


def resident(paths):
    total = res = nfiles = 0
    for f in files(paths):
        nfiles += 1
        total += os.path.getsize(f)
        res += resident_bytes(f)
    pct = 100.0 * res / total if total else 0.0
    print(f"resident {res / 1e6:.1f} of {total / 1e6:.0f} MB ({pct:.1f} %, {nfiles} files)")
    return 0


def copy(src, dst):
    if os.path.exists(dst):
        print(f"coldcache: {dst} exists", file=sys.stderr)
        return 2
    shutil.copytree(src, dst, symlinks=True)
    return drop([dst])


def main(argv):
    if not sys.platform.startswith("linux") or not hasattr(os, "posix_fadvise"):
        print("coldcache: Linux only (posix_fadvise, mincore)", file=sys.stderr)
        return 2
    if len(argv) >= 2 and argv[0] == "drop":
        return drop(argv[1:])
    if len(argv) >= 2 and argv[0] == "resident":
        return resident(argv[1:])
    if len(argv) == 3 and argv[0] == "copy":
        return copy(argv[1], argv[2])
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
