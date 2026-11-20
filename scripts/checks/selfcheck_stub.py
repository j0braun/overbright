#!/usr/bin/env python3
"""A stand-in `game` binary for selfcheck.sh: answers --shot / --bench / --flow / --playthrough /
--selftest the way the gates read them (PNGs, dump sets, bench JSON, WAVs, NFSU2_STATE_HASH tables,
the adapter log line), in milliseconds and without a GPU or the game's data.

Faults: one per line in the file `faults` next to this script (absent: none), each `KIND` or `KIND:NAME`:
  nodump:NAME|all   no frames in the dump set NAME (basename of NFSU2_BENCH_DUMP)
  imgdiff:NAME      a different pixel in the dump set NAME
  extradump         also writes a dump set `<prefix>-extra` next to the given one
  rc:NAME           exit 1 after writing everything (NAME: dump set, shot, flow or bench scene)
  noshot:NAME       --shot NAME writes no PNG
  unknown:NAME      --shot NAME is no target of this binary: "unknown screen", exit 1, no PNG
  shotdiff:NAME     --shot NAME draws a different pixel
  nondet:NAME       --shot NAME draws a random pixel (differs run to run)
  panic:NAME        a "panicked at" line in the log of shot / flow / bench NAME
  wavdiff           NFSU2_AUDIO_WAV gets different bytes
  hashdiff:NAME     flow NAME's state hash differs from step 50
  nohash:NAME       flow NAME writes no state hash table
  ratedep           the state hash depends on the display rate (*_HZ / NFSU2_BENCH_HZ / NFSU2_RATE) from step 40
  rateimg           the rate run's α = 0 dumps depend on the display rate (NFSU2_RATE ≠ 60: a different pixel)
  jittershort       a jittered rate bench ends two ticks short of its NFSU2_RATE_TICKS (the gate must count it)
  noselftest        NFSU2_RATE_SELFTEST's injected rate dependence does not reach the hashes (the gate must say MISSED)
  benchfail:SCENE   --bench SCENE exits 1 without its JSON
  privskip:NAME     flow NAME needs the private set: a "SKIP: ... needs the maintainer's private set (OB_PRIVATE)" line, exit 3
  benchfailenh:TEXT --bench exits 1 without its JSON when NFSU2_BENCH_ENH contains TEXT (one preset's configs)
  adapter:NAME      the adapter's name (default "Generic GPU (stub)"); `noadapter`: no line
  matrixpanic       a matrix bench prints a panic
  noharness         a shipping build: --dev-harness ends in the usage error (rc 2)
  prepfail          --prepare ends with error= / ok=0 and exit code 1
  nostamp           --prepare says ok=1 but writes no stamp
  prepgrow          every --prepare run adds bytes to a cache file (not idempotent)
  loadgrow          a --bench after a prepare run adds bytes to the pipeline cache
  levelstale        --probe says prepared=1 whatever the Graphics level in options.txt (no program key)
Called as `overbright-editor` (a copy of this file under that name) it answers the map editor's `--shot OUT
--size WxH [--scale F] [--mod M --mode props|races ...]` with a PNG of that size (rows of grey stripes; on a world
screen a chequered view in the middle), and refuses to run with a display variable set (a window could open).
Its screen: `start` without --mod, else `props_3d` / `races_map` by --mode. Faults (SCREEN: one screen or `all`):
  edrc:SCREEN       exit 1 after writing the PNG
  edpanic:SCREEN    a "panicked at" line in the log
  ednoshot:SCREEN   no PNG written
  edsize:SCREEN     the PNG one pixel narrower than asked
  edflat:SCREEN     the whole image one colour
  edworld:SCREEN    the middle of the window (the view) one colour
  edclip:SCREEN     the log says "editor layout: toolbar clipped: …" (a world screen says "… toolbar fits …" otherwise)
  ednondet:SCREEN   one pixel differs on every run (the picture is not repeatable)
"""
import json
import os
import random
import struct
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(sys.argv[0]))


def faults():
    try:
        return {l.strip() for l in open(os.path.join(HERE, 'faults')) if l.strip()}
    except OSError:
        return set()


F = faults()


def has(kind, name=None):
    return kind in F or (name is not None and f'{kind}:{name}' in F)


def program_key():
    """The stand-in for the settings' program key: the graphics lines of options.txt
    (`enh.` and `custom.`) in the data folder the run is given."""
    path = os.path.join(os.environ.get('XDG_DATA_HOME', ''), 'overbright', 'options.txt')
    try:
        lines = sorted(l.strip() for l in open(path) if l.startswith(('enh.', 'custom.')))
    except OSError:
        lines = []
    return '%08x' % zlib.crc32('\n'.join(lines).encode())


def png(path, rgba):
    w = h = 4
    raw = b''.join(b'\0' + bytes(rgba) * w for _ in range(h))
    def chunk(t, d):
        return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    data = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b'')
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    open(path, 'wb').write(data)


def adapter():
    if 'noadapter' in F:
        return None
    name = next((f.split(':', 1)[1] for f in F if f.startswith('adapter:')), 'Generic GPU (stub)')
    return name


def hashes(name, steps, hz=None):
    path = os.environ.get('NFSU2_STATE_HASH')
    if not path or has('nohash', name):
        return
    with open(path, 'w') as f:
        f.write('step\tdt\tall\tglobal\tplayer\trace\trivals\ttraffic\n')
        for i in range(steps):
            v = i
            if has('hashdiff', name) and i >= 50:
                v += 1000
            if 'ratedep' in F and hz is not None and hz != 60 and i >= 40:
                v += int(hz)
            st = os.environ.get('NFSU2_RATE_SELFTEST')
            if st and 'noselftest' not in F and hz is not None and hz != 60 and i >= int(st):
                v += 7
            h = [f'{(v * 2654435761 + k) & 0xffffffffffffffff:016x}' for k in range(6)]
            f.write(f'{i}\t3c888889\t' + '\t'.join(h) + '\n')


def rate_hz():
    """NFSU2_RATE's schedule as a rate (jitter: 45 frames a second, as perf.sh sizes its runs)."""
    r = os.environ.get('NFSU2_RATE')
    if not r:
        return None
    return 45.0 if r.startswith('jitter:') else float(r)


def rate_outputs():
    """A rate run's α = 0 dumps and WAV (the same at every rate)."""
    d = os.environ.get('NFSU2_RATE_DUMP')
    if d:
        os.makedirs(d, exist_ok=True)
        other = has('rateimg') and rate_hz() not in (None, 60.0)
        for k in range(3):
            png(os.path.join(d, f'tick-{k * 6:06}.png'), [k * 40, 70, 81 if other else 80, 255])
    wav = os.environ.get('NFSU2_AUDIO_WAV')
    if wav and os.environ.get('NFSU2_RATE'):
        open(wav, 'wb').write(b'RIFF-stub-' + b'A' * 64)


def png_rows(path, w, h, row):
    """A w x h RGBA PNG whose row y is the bytes row(y) (4 * w of them)."""
    raw = b''.join(b'\0' + row(y) for y in range(h))
    def chunk(t, d):
        return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    data = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw, 1)) + chunk(b'IEND', b'')
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    open(path, 'wb').write(data)


def editor(a):
    """The map editor's --shot (see the module's notes)."""
    for var in ('DISPLAY', 'WAYLAND_DISPLAY'):
        if os.environ.get(var):
            print(f'Error: {var}={os.environ[var]} is set: the stand-in refuses (a window could open)', file=sys.stderr)
            return 1
    if a[:1] == ['--version']:
        print('Overbright stub (commit stub, linux-x86_64) map editor (preview)')
        return 0
    opts = {}
    i = 0
    while i < len(a):
        if a[i].startswith('--') and i + 1 < len(a):
            opts[a[i]] = a[i + 1]
            i += 2
        else:
            print(f'Error: unknown argument {a[i]}', file=sys.stderr)
            return 2
    out = opts.get('--shot')
    if not out:
        print('Error: the stand-in only draws shots', file=sys.stderr)
        return 2
    w, h = (int(v) for v in opts.get('--size', '1600x900').split('x'))
    screen = 'start' if '--mod' not in opts else ('races_map' if opts.get('--mode') == 'races' else 'props_3d')
    def ed(kind):
        return has(kind, screen) or f'{kind}:all' in F
    if ed('edsize'):
        w -= 1
    flat = ed('edflat')
    noise = (random.randrange(w), random.randrange(h)) if ed('ednondet') else None
    x0, x1 = int(w * 0.3), int(w * 0.7)
    y0, y1 = int(h * 0.3), int(h * 0.7)
    def row(y):
        if flat:
            return bytes([40, 40, 40, 255]) * w
        g = (y * 37) % 200 + 20
        line = bytes([g, g, g, 255]) * w
        if screen != 'start' and y0 <= y < y1:
            if ed('edworld'):
                mid = bytes([90, 90, 90, 255]) * (x1 - x0)
            else:
                mid = b''.join(bytes([200, 60, 60, 255]) * 8 if ((x // 8) + (y // 8)) % 2 else bytes([20, 20, 60, 255]) * 8
                               for x in range(x0, x1, 8))[:4 * (x1 - x0)]
            line = line[:4 * x0] + mid + line[4 * x1:]
        if noise and y == noise[1]:
            x = noise[0]
            line = line[:4 * x] + bytes([random.randrange(256), 7, 7, 255]) + line[4 * x + 4:]
        return line
    print(f'[stub INFO  ob_studio_gui] screen {screen}')
    if screen != 'start':
        if ed('edclip'):
            print('editor layout: toolbar clipped: Test race cut off (1270..1290 of 0..1280)')
        else:
            print(f'editor layout: toolbar fits ({w} points wide, tier 0): Select · Move · Test race')
    if not ed('ednoshot'):
        png_rows(out, w, h, row)
        print(f'wrote {out} ({w}x{h})')
    if ed('edpanic'):
        print(f"thread 'main' panicked at src/stub.rs:1:1: stub panic in the editor's {screen}")
    return 1 if ed('edrc') else 0


def main(a):
    if os.path.basename(sys.argv[0]) == 'overbright-editor':
        return editor(a)
    rc = 0
    name = ''
    ad = adapter()
    if a[:1] == ['--dev-harness']:
        if 'noharness' in F:
            print('game: unknown option --dev-harness', file=sys.stderr)
            return 2
        print('dev-harness: on')
        return 0
    if a[:1] == ['--selftest']:
        print('selftest: stub ok')
        return 0
    cache = os.environ.get('NFSU2_CACHE_DIR')
    if a[:1] == ['--prepare']:
        # docs/developing/prepare.md: the lines the launcher reads, the pipeline cache, the stamp last.
        print('prepare=1\nstep=1/5 graphics device\nadapter=Generic GPU (stub)\npipeline_cache=1', flush=True)
        if 'prepfail' in F:
            print('error=stub failure\nok=0')
            return 1
        os.makedirs(cache, exist_ok=True)
        print('step=2/5 shader programs', flush=True)
        pc = os.path.join(cache, 'wgpu_pipeline_cache_stub')
        if 'prepgrow' in F or not os.path.exists(pc):
            open(pc, 'ab').write(b'P' * 32)
        print('step=3/5 the city\nstep=4/5 the first race\'s track\nstep=5/5 saving\npipelines=7')
        if 'nostamp' not in F:
            open(os.path.join(cache, 'prepared.txt'), 'w').write(f'stub build\nprograms={program_key()}\n')
        print('ok=1')
        return 0
    if a[:1] == ['--probe']:
        stamp = os.path.join(cache, 'prepared.txt') if cache else ''
        ok = bool(cache) and os.path.exists(stamp)
        if ok and 'levelstale' not in F:
            ok = f'programs={program_key()}' in open(stamp).read().splitlines()
        print(f"prepared={int(ok)}")
        return 0
    if a[:1] == ['--verify-cache']:
        print('verify-cache: stub ok')
        return 0
    if a[:1] == ['--shot']:
        name, out = a[1], a[2]
        if has('unknown', name):
            print(f'Error: unknown screen {name}', file=sys.stderr)
            return 1
        if ad:
            print(f'[stub INFO  nfsu2::render] GPU adapter: {ad} (DiscreteGpu, Vulkan)')
        if not has('noshot', name):
            px = [10, 20, 30, 255]
            if has('shotdiff', name):
                px[0] += 1
            if has('nondet', name):
                px[1] = random.randrange(256)
            png(out, px)
        wav = os.environ.get('NFSU2_AUDIO_WAV')
        if wav:
            open(wav, 'wb').write(b'RIFF-stub-' + (b'B' if 'wavdiff' in F else b'A') * 64)
        hashes(name, 10)
    elif a[:1] == ['--bench']:
        name = a[1]
        frames = int(a[2]) if len(a) > 2 else 600
        if ad:
            print(f'adapter: {ad} (Vulkan, stub); BC true')
        if os.environ.get('NFSU2_BENCH_MATRIX'):
            print("thread 'render' panicked at stub" if 'matrixpanic' in F else 'stub')
            print('matrix: 3 of 3 settings drawn')
        if has('benchfail', name) or any(f.startswith('benchfailenh:') and f[13:] in os.environ.get('NFSU2_BENCH_ENH', '') for f in F):
            print('bench: stub failure')
            return 1
        dump = os.environ.get('NFSU2_BENCH_DUMP')
        if dump:
            d = os.path.basename(dump)
            rc |= has('rc', d)
            os.makedirs(dump, exist_ok=True)
            if not (has('nodump', d) or 'nodump:all' in F):
                for k in range(3):
                    px = [k * 40, 50, 60, 255]
                    if has('imgdiff', d):
                        px[2] += 1
                    png(os.path.join(dump, f'frame{k:04}.png'), px)
            if 'extradump' in F:
                png(os.path.join(dump + '-extra', 'frame0000.png'), [1, 2, 3, 255])
        hz = os.environ.get('NFSU2_BENCH_HZ')
        hz = float(hz) if hz else rate_hz()
        steps = int(frames * 60 // hz) if hz else frames
        # A rate run's benches end on their tick count (NFSU2_RATE_TICKS), whatever the schedule.
        if os.environ.get('NFSU2_RATE') and os.environ.get('NFSU2_RATE_TICKS'):
            steps = int(os.environ['NFSU2_RATE_TICKS'])
            if 'jittershort' in F and os.environ['NFSU2_RATE'].startswith('jitter:'):
                steps -= 2
        hashes(name, steps, hz)
        rate_outputs()
        js = os.environ.get('NFSU2_BENCH_JSON')
        if js:
            json.dump({'frames': frames, 'gpu_busy_ms': [1.0, 1.01, 0.99, 1.0], 'draw_cpu_ms': [0.5, 0.5, 0.51, 0.49],
                       'threads': [{'name': 'main (game)', 'ms_per_frame': 0.8}, {'name': 'render 0', 'ms_per_frame': 0.3}],
                       'process_cpu_ms_per_frame': 1.5, 'draw_cpu_ms_per_frame': 0.5, 'load_ms': 300}, open(js, 'w'))
        if 'loadgrow' in F and cache and os.path.exists(os.path.join(cache, 'wgpu_pipeline_cache_stub')):
            open(os.path.join(cache, 'wgpu_pipeline_cache_stub'), 'ab').write(b'L' * 8)
        print(f'bench {name}: stub, {frames} frames; load 300 ms, first frame 1.0 ms')
    elif a[:1] in (['--flow'], ['--playthrough']):
        name = a[1] if a[0] == '--flow' else 'playthrough'
        if has('privskip', name):
            print(f"SKIP: --flow {name} (stub.sav): needs the maintainer's private set (OB_PRIVATE)")
            return 3
        hz = None
        for var in ('NFSU2_FIRSTRACE_HZ', 'NFSU2_FIRSTDRAG_HZ', 'NFSU2_POSTRACE_HZ'):
            if os.environ.get(var):
                hz = float(os.environ[var].split(',')[0])
        if rate_hz() is not None:
            hz = rate_hz()
        hashes(name, 100, hz)
        rate_outputs()
        print(f'flow {name}: stub done')
    else:
        print(f'stub: unknown arguments {a}')
        return 2
    if has('panic', name):
        print(f"thread 'main' panicked at src/stub.rs:1:1: stub panic in {name}")
    if has('rc', name):
        rc = 1
    return 1 if rc else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
