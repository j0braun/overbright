"""One line per distribution from run.sh's results (<out>/<name>-{bare,full}/results.txt).

    python3 -I summary.py <out dir> <name>...

works: both programs start, --check-game passes, the probe finds the (software) Vulkan adapter, every
shot drew a picture without a panic or validation error, and the launcher stayed open and was
photographed. Otherwise the first thing that failed, in the player's terms (a glibc version, a library,
Vulkan). "bare" lists what the image as pulled lacks (a minimal install, not a desktop).
"""

import re
import sys
from pathlib import Path


def results(path: Path) -> list[str]:
    if not path.exists():
        return []
    return [l[len("RESULT "):] for l in path.read_text(errors="replace").splitlines() if l.startswith("RESULT ")]


def get(lines: list[str], prefix: str) -> str | None:
    return next((l for l in lines if l.startswith(prefix)), None)


def why_not_starting(line: str) -> str:
    want = sorted(set(re.findall(r"GLIBC_([0-9.]+)' not found", line)), key=lambda v: [int(x) for x in v.split(".")])
    if want:
        return f"needs glibc {want[-1]}"
    lib = re.search(r"([A-Za-z0-9_.+-]+\.so[.0-9]*): cannot open shared object file", line)
    if lib:
        return f"missing {lib.group(1)}"
    return line[:120]


def verdict(full: list[str]) -> tuple[str, str]:
    if not full:
        return "FAIL", "no result (the test image could not be built?)"
    for b in ("game", "overbright"):
        v = get(full, f"{b} --version")
        if not v or "rc=0:" not in v:
            return "FAIL", f"{b} does not start: {why_not_starting(v or '')}"
    c = get(full, "game --check-game")
    if not c or "rc=0:" not in c:
        return "FAIL", f"--check-game: {c}"
    p = get(full, "game --probe")
    if not p or "ok=1" not in p:
        return "FAIL", f"no Vulkan adapter: {p}"
    for s in (l for l in full if l.startswith("shot ")):
        m = re.match(r"shot (\S+) rc=(\d+) (\d+) bytes (\d+) s panics/validation=(\d+)", s)
        if not m or m.group(2) != "0" or m.group(3) == "0" or m.group(5) != "0":
            return "FAIL", s
    if not get(full, "launcher running") or not get(full, "launcher screenshot taken"):
        return "FAIL", f"launcher: {get(full, 'launcher')}"
    return "works", ""


def main() -> int:
    out = Path(sys.argv[1])
    rows = []
    for n in sys.argv[2:]:
        bare, full = results(out / f"{n}-bare" / "results.txt"), results(out / f"{n}-full" / "results.txt")
        osname = (get(full or bare, "os ") or "os ?")[3:]
        glibc = (get(full or bare, "glibc ") or "glibc ?").split()[-1]
        missing = set()
        for b in ("game", "overbright"):
            l = get(bare, f"ldd {b}:") or ""
            missing.update(re.findall(r"([A-Za-z0-9_.+-]+\.so[.0-9]*) => not found", l))
        v, why = verdict(full)
        adapter = re.search(r"adapter\.driver=([^|]*)", get(full, "game --probe") or "")
        shots = " ".join(
            f"{m.group(1)} {m.group(2)} s"
            for m in (re.match(r"shot (\S+) rc=0 \d+ bytes (\d+) s", s) for s in full if s.startswith("shot "))
            if m
        )
        rows.append(
            f"{n:15} {v:5} glibc {glibc:5} {osname[:28]:28} | bare lacks: {' '.join(sorted(missing)) or '-'}"
            + (f" | {adapter.group(1)[:44]}" if adapter else "")
            + (f" | shots {shots}" if shots else "")
            + (f" | {why}" if why else "")
        )
    print("\n".join(rows))
    return 0


if __name__ == "__main__":
    sys.exit(main())
