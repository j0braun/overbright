#!/usr/bin/env python3
"""Rate gate v2 audio (docs/checks.md "Rate gate"): the offline mix of one flow (NFSU2_AUDIO_WAV, pumped per world
update) at several display schedules, compared byte for byte against the reference schedule's.
usage: ratewav.py REF HZ=FILE.wav ...   ("ctl" = a second run at the reference: run-to-run determinism)
Prints one line: per schedule "identical (N s)", the first differing sample's time, or a length difference.
Exit 1 when any differs or a file is missing (report-only today: the mix is not tick-exact yet)."""
import sys

HEADER = 44
RATE = 48000 * 4  # 16-bit stereo bytes per second (dev/audio_capture.rs)


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    ref = argv[0]
    data = {}
    for a in argv[1:]:
        hz, _, path = a.partition('=')
        try:
            data[hz] = open(path, 'rb').read()
        except OSError:
            data[hz] = None
    r = data.get(ref)
    if not r:
        print(f'no WAV at the reference {ref}')
        return 1
    parts, bad = [], 0
    for hz, d in data.items():
        if hz == ref:
            continue
        if d is None:
            parts.append(f'{hz}: MISSING')
            bad += 1
            continue
        a, b = r[HEADER:], d[HEADER:]
        n = min(len(a), len(b))
        i = next((k for k in range(0, n, 4) if a[k:k + 4] != b[k:k + 4]), None) if a[:n] != b[:n] else None
        if i is not None:
            parts.append(f'{hz}: differs from {i / RATE:.3f} s')
            bad += 1
        elif len(a) != len(b):
            parts.append(f'{hz}: same for {n / RATE:.3f} s, then one ends ({len(b) / RATE:.3f} vs {len(a) / RATE:.3f} s)')
            bad += 1
        else:
            parts.append(f'{hz}: identical ({len(a) / RATE:.1f} s)')
    print(f'vs {ref}: ' + '; '.join(parts))
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
