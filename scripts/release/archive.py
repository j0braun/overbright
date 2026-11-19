#!/usr/bin/env python3
"""Reproducible release archives (scripts/release/package.sh, docs/developing/building-releases.md).

  archive.py tar   STAGE TOP OUT.tar.gz EPOCH    STAGE/TOP/** into a gzip'd tar
  archive.py zip   STAGE TOP OUT.zip    EPOCH    STAGE/TOP/** into a zip
  archive.py list  ARCHIVE                         one entry per line (files; directories end in /)
  archive.py unpack ARCHIVE DIR                    unpack (keeps the executable bit from a zip too)
  archive.py png   RGBA N OUT.png                   an N x N RGBA file (raw, rows top down) as PNG

Entries are sorted by name, every time stamp is EPOCH (the commit's time, SOURCE_DATE_EPOCH), owner and
group are 0/0 with no names, modes are 0755 (directories, executables) or 0644, and the gzip header has no
name and no time: the same files give the same bytes. Python 3 standard library only.
"""
import gzip
import io
import os
import stat
import struct
import sys
import tarfile
import time
import zipfile
import zlib
from pathlib import Path


def entries(stage, top):
    root = Path(stage)
    out = [top + "/"]
    for d, dirs, files in os.walk(root / top):
        dirs.sort()
        rel = Path(d).relative_to(root).as_posix()
        out += [f"{rel}/{x}/" for x in dirs] + [f"{rel}/{x}" for x in sorted(files)]
    return sorted(set(out))


def mode_of(path):
    return 0o755 if path.is_dir() or os.access(path, os.X_OK) else 0o644


def make_tar(stage, top, out, epoch):
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.PAX_FORMAT) as t:
        for name in entries(stage, top):
            p = Path(stage) / name.rstrip("/")
            info = tarfile.TarInfo(name.rstrip("/"))
            info.mtime, info.uid, info.gid, info.uname, info.gname = epoch, 0, 0, "", ""
            info.mode = mode_of(p)
            if p.is_dir():
                info.type = tarfile.DIRTYPE
                t.addfile(info)
            else:
                info.size = p.stat().st_size
                with open(p, "rb") as f:
                    t.addfile(info, f)
    with open(out, "wb") as f, gzip.GzipFile(filename="", mode="wb", fileobj=f, compresslevel=9, mtime=0) as g:
        g.write(raw.getvalue())


def make_zip(stage, top, out, epoch):
    dt = time.gmtime(max(epoch, 315532800))[:6]  # zip times start in 1980
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for name in entries(stage, top):
            p = Path(stage) / name.rstrip("/")
            info = zipfile.ZipInfo(name, date_time=dt)
            info.create_system = 3  # Unix: the mode below is honoured by unzip
            if p.is_dir():
                info.external_attr = (stat.S_IFDIR | 0o755) << 16 | 0x10
                z.writestr(info, b"")
            else:
                info.external_attr = (stat.S_IFREG | mode_of(p)) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                z.writestr(info, p.read_bytes(), compresslevel=9)


def list_archive(path):
    if str(path).endswith(".zip"):
        with zipfile.ZipFile(path) as z:
            return [i.filename for i in z.infolist()]
    with tarfile.open(path) as t:
        return [m.name + ("/" if m.isdir() else "") for m in t.getmembers()]


def unpack(path, dest):
    dest = Path(dest)
    dest.mkdir(parents=True, exist_ok=True)
    if str(path).endswith(".zip"):
        with zipfile.ZipFile(path) as z:
            for i in z.infolist():
                if i.filename.startswith("/") or ".." in Path(i.filename).parts:
                    raise SystemExit(f"unsafe entry {i.filename}")
                z.extract(i, dest)
                mode = (i.external_attr >> 16) & 0o777
                if mode and not i.is_dir():
                    os.chmod(dest / i.filename, mode)
    else:
        with tarfile.open(path) as t:
            t.extractall(dest, filter="data")


def png(rgba_path, n, out):
    data = Path(rgba_path).read_bytes()
    if len(data) != n * n * 4:
        raise SystemExit(f"{rgba_path}: {len(data)} bytes, want {n * n * 4}")
    rows = b"".join(b"\0" + data[y * n * 4:(y + 1) * n * 4] for y in range(n))

    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", n, n, 8, 6, 0, 0, 0)
    Path(out).write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(rows, 9))
                          + chunk(b"IEND", b""))


def main(argv):
    cmd = argv[1] if len(argv) > 1 else ""
    if cmd == "tar" and len(argv) == 6:
        make_tar(argv[2], argv[3], argv[4], int(argv[5]))
    elif cmd == "zip" and len(argv) == 6:
        make_zip(argv[2], argv[3], argv[4], int(argv[5]))
    elif cmd == "list" and len(argv) == 3:
        print("\n".join(list_archive(argv[2])))
    elif cmd == "unpack" and len(argv) == 4:
        unpack(argv[2], argv[3])
    elif cmd == "png" and len(argv) == 5:
        png(argv[2], int(argv[3]), argv[4])
    else:
        print(__doc__, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
