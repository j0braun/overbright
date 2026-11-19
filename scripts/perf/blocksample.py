#!/usr/bin/env python3
"""What each thread of a run waits for (Linux; docs/notes/performance/budget.md "Load times").

  blocksample.py run OUT.tsv -- CMD ARGS...   run CMD, stamp every output line (ms since the start) and sample each
                                              thread about every millisecond: running, or the system call it is
                                              blocked in (/proc/PID/task/TID/syscall; an ioctl with its request)
  blocksample.py stages OUT.tsv...            per stage (the NFSU2_TIMING spans' windows and the bench's first frame),
                                              medians over the runs: wall, the main thread's time running / waiting
                                              for its workers / waiting for the GPU / in the submit call, and the
                                              other threads' time running / in GPU memory and submit calls / reading
  blocksample.py seq OUT.tsv FROM_MS TO_MS [MIN_MS]   the main thread's runs of one state between two times

Start the game as every headless run is started (its own cache folder and data folders, no display variables),
with NFSU2_TIMING=1 for the stages. The sampler reads only its own child's /proc entries. A wall time says how
long a stage took; this says whether the thread was working, waiting for another thread or waiting for the GPU,
which a busy machine or a GPU shared with another program changes a lot. The ioctl names are the amdgpu and DRM
sync-object requests; another driver's requests print as their number.
"""
import collections
import os
import re
import statistics
import subprocess
import sys
import threading
import time

STAGES = ['Game::new', 'World::load', 'Car::load_tuned', 'WorldRenderer::new: groups and draws',
          'WorldRenderer::new: texture arrays: upload', 'WorldRenderer::new: buffers, uniforms',
          'WorldRenderer::new: pipelines', 'WorldRenderer::new', 'drive_in: car renderer', 'drive_in: uploads',
          'drive_in: warm', 'drive_in', 'rt: world geometry']
# x86-64 system call numbers: futex, sleeps and yields are waits for another thread.
WORKER_WAITS = {'202', '230', '35', '24'}
FENCE_WAITS = {'dca', 'dc3', 'd49', 'd52'}       # sync-object waits, amdgpu WAIT_CS / WAIT_FENCES
GPU_CALLS = {'d40', 'd41', 'd44', 'd48', 'd09', 'd50'}   # amdgpu GEM_CREATE, GEM_MMAP, CS, GEM_VA, GEM_CLOSE, GEM_OP


def run(out, cmd):
    t0 = time.monotonic()
    now = lambda: (time.monotonic() - t0) * 1000.0
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, bufsize=0)
    rows = []

    def reader():
        buf = b''
        while True:
            b = os.read(p.stdout.fileno(), 65536)
            if not b:
                break
            t = now()
            buf += b
            while b'\n' in buf:
                line, buf = buf.split(b'\n', 1)
                rows.append(('L', t, line.decode('utf-8', 'replace')))

    th = threading.Thread(target=reader)
    th.start()
    base = f'/proc/{p.pid}/task'
    while p.poll() is None:
        t = now()
        try:
            tids = os.listdir(base)
        except OSError:
            break
        for tid in tids:
            try:
                with open(f'{base}/{tid}/syscall') as f:
                    s = f.read().split()
            except OSError:
                continue
            if not s:
                continue
            if s[0] in ('running', '-1'):
                state = 'run'
            else:
                state = 'sys:' + s[0]
                if s[0] == '16' and len(s) > 2:      # ioctl: the request's type and number
                    req = int(s[2], 16)
                    kind = (req >> 8) & 0xFF
                    state += ':%s%02x' % (chr(kind) if 32 <= kind < 127 else '?', req & 0xFF)
            rows.append(('S', t, tid, state))
        time.sleep(0.0005)
    th.join()
    with open(out, 'w') as f:
        f.write(f'# pid {p.pid} rc {p.returncode}\n')
        for r in rows:
            f.write('\t'.join(f'{x:.2f}' if isinstance(x, float) else str(x) for x in r) + '\n')
    return p.returncode


def load(path):
    with open(path) as f:
        pid = f.readline().split()[2]
        rows = [l.rstrip('\n').split('\t') for l in f]
    lines = [(float(r[1]), r[2]) for r in rows if r[0] == 'L']
    samples = [(float(r[1]), r[2], r[3]) for r in rows if r[0] == 'S']
    return pid, lines, samples


def main_class(state):
    p = state.split(':')
    if p[0] == 'run':
        return 'run'
    if p[1] in WORKER_WAITS:
        return 'workers'
    if p[1] == '16' and len(p) > 2:
        return 'gpu' if p[2] in FENCE_WAITS else 'submit' if p[2] == 'd44' else 'other'
    return 'other'


def other_class(state):
    p = state.split(':')
    if p[0] == 'run':
        return 'run'
    if (p[1] == '16' and len(p) > 2 and p[2] in GPU_CALLS) or p[1] == '9':     # 9: mmap (staging memory)
        return 'gpucalls'
    return 'io' if p[1] == '17' else None      # 17: pread


def stages(paths):
    res = collections.defaultdict(lambda: collections.defaultdict(list))
    for path in paths:
        pid, lines, samples = load(path)
        windows, seen = [], set()
        for t, l in lines:
            m = re.match(r'\[time\] (.+?): ([\d.]+) ms', l)
            if m and m[1] in STAGES and m[1] not in seen:
                seen.add(m[1])
                windows.append((m[1], t - float(m[2]), t))
            m = re.match(r'first frame: update ([\d.]+) ms, draw ([\d.]+) ms, GPU wait ([\d.]+) ms', l)
            if m:
                _, draw, wait = map(float, m.groups())
                windows.append(('first frame: draw', t - wait - draw, t - wait))
                windows.append(("first frame: the bench's wait for the GPU", t - wait, t))
        for name, a, b in windows:
            acc, prev = collections.Counter(), {}
            for t, tid, state in samples:
                if t < a - 30 or t > b + 30:
                    continue
                if tid in prev:
                    pt, ps = prev[tid]
                    lo, hi = max(pt, a), min(t, b)
                    if hi > lo:
                        c = 'm_' + main_class(ps) if tid == pid else other_class(ps) and 'o_' + other_class(ps)
                        if c:
                            acc[c] += hi - lo
                prev[tid] = (t, state)
            res[name]['wall'].append(b - a)
            for k in ('m_run', 'm_workers', 'm_gpu', 'm_submit', 'o_run', 'o_gpucalls', 'o_io'):
                res[name][k].append(acc.get(k, 0.0))
    med = lambda v: f'{statistics.median(v):.0f}'
    print(f'{len(paths)} runs; ms; wall: median [min..max], the rest medians')
    print('stage | wall | main: running | main: waits for workers | main: waits for the GPU | main: in the submit '
          'call | other threads: running | other threads: GPU memory and submit calls | other threads: file reads')
    for name in [s for s in STAGES if s in res] + [k for k in res if k.startswith('first frame')]:
        r = res[name]
        w = sorted(r['wall'])
        print(f'{name} | {med(w)} [{w[0]:.0f}..{w[-1]:.0f}] | ' + ' | '.join(
            med(r[k]) for k in ('m_run', 'm_workers', 'm_gpu', 'm_submit', 'o_run', 'o_gpucalls', 'o_io')))


def seq(path, a, b, least):
    pid, lines, samples = load(path)
    events = [(t, None, l) for t, l in lines if a <= t <= b] + [(t, s, None) for t, tid, s in samples if tid == pid and a <= t <= b]
    cur = None
    for t, state, line in sorted(events, key=lambda e: e[0]):
        if line is not None:
            print(f'{t:9.1f}            | {line[:150]}')
        elif cur is None or cur[1] != state:
            if cur is not None and t - cur[0] >= least:
                print(f'{cur[0]:9.1f} {t - cur[0]:7.1f} ms  {cur[1]}')
            cur = (t, state)


if __name__ == '__main__':
    a = sys.argv[1:]
    if len(a) >= 4 and a[0] == 'run' and a[2] == '--':
        sys.exit(run(a[1], a[3:]))
    if len(a) >= 2 and a[0] == 'stages':
        stages(a[1:])
    elif len(a) >= 4 and a[0] == 'seq':
        seq(a[1], float(a[2]), float(a[3]), float(a[4]) if len(a) > 4 else 2.0)
    else:
        sys.exit(__doc__)
