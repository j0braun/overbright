#!/usr/bin/env python3
"""rt_free.py SAVE OUT --copy C --display N --mode roll|idle [--kmh V --gear G --ratio R] [--secs S] [--at x,y,z,deg]
The original in real time on the highway straight: boot to free roam from SAVE, the cave holds the game while
the car is placed, the cave is taken out (own pace, audio in real time), then (roll) the throttle key goes down
and the car is set rolling at V km/h in gear G (rigid body P, wheel spin, gear, shaft and engine speed, as
native.py's [start] speed); (idle) the car stands. Records native.wav (+ sync), native-events.tsv and
drive.tsv (clock, gear, rpm car+0x440, speed car+0x430, keys)."""
import argparse, os, sys, time, struct, math, signal
P = os.environ['PARITY_TOOLS']; sys.path.insert(0, P)
import native as N, cave, nav, place
import fields as F
ap = argparse.ArgumentParser(); ap.add_argument('save'); ap.add_argument('out')
ap.add_argument('--copy'); ap.add_argument('--display', type=int)
ap.add_argument('--mode', default='roll'); ap.add_argument('--kmh', type=float, default=120)
ap.add_argument('--gear', type=int, default=4); ap.add_argument('--ratio', type=float, default=5.154)
ap.add_argument('--secs', type=float, default=10); ap.add_argument('--fixed', type=int, default=0); ap.add_argument('--at', default='352.6,-1445.0,36.6,107')
a = ap.parse_args(); os.makedirs(a.out, exist_ok=True)
at = [float(x) for x in a.at.split(',')]
sess = N.Session(a.copy, a.display); watch = None
if os.environ.get('RT_DXVK'):
    # DXVK instead of wined3d (the original at its 120 fps cap: AudioManager runs every 2nd frame, 60 Hz)
    import subprocess, shutil
    def start(save, profile, audio=None, self=sess):
        self.stop()
        prof = os.path.join(self.copy, 'prefix/drive_c/users', os.environ.get('USER', ''), 'AppData/Local/NFS Underground 2')
        assert prof.startswith(os.path.expanduser('~')), prof
        shutil.rmtree(prof, ignore_errors=True); os.makedirs(os.path.join(prof, profile)); shutil.copy(save, os.path.join(prof, profile, profile))
        r = subprocess.run([os.path.join(N.WINE, 'fastgpu.sh'), self.copy, str(self.display), '--audio-file', audio], capture_output=True, text=True)
        assert 'running' in r.stdout, r.stdout + r.stderr
        self.s = N.peek.Session(self.copy); return self.s
    sess.start = start
try:
    s = sess.start(a.save, 'NAME', os.path.join(os.path.abspath(a.out), 'native.wav'))
    n = nav.Nav(s, a.out, N.log); n.boot_to_freeroam(); N.log('free roam')
    time.sleep(3)
    c = cave.Cave(s, a.out); c.install(); c.fixed(0)
    lvl = (0x89E7A8, [], 0xFFFFFFFF, s.u32(0x89E7A8), 0)
    c.hold(lvl); t1 = time.time()
    while not c.held():
        assert time.time() - t1 < 30, 'not held'
        time.sleep(0.05)
    car = s.u32(s.u32(F.WORLD) + 0x1C); hd = math.radians(at[3])
    place.put(s, car, tuple(at[:3]), heading=hd)
    rows = [(math.cos(hd), math.sin(hd), 0.0), (-math.sin(hd), math.cos(hd), 0.0), (0.0, 0.0, 1.0)]
    s.req(*[f'w {car + 0x90 + 16 * k:#x} ' + struct.pack('<3f', *r).hex() for k, r in enumerate(rows)])
    if a.fixed:
        # The cave stays in the frame-time call with every frame 1000/fixed ms (no recording, no lock):
        # the game's cadence is the original's at that frame rate whatever the wall clock.
        c.w32('LOCK', 0); c.w32('ACK', 0x7FFFFFFF); c.w32('ENABLE', 0); c.fixed(a.fixed); N.log('cave fixed', a.fixed)
    else:
        c.uninstall(); N.log('cave out: own pace')
    time.sleep(1.5)                                    # the teleport hold (~20 frames) and the drop onto the road
    watch = N.EventWatch(a.copy, os.path.join(a.out, 'native-events.tsv'))
    N.log('inputs started')
    pid = s.state['game_pid'] if hasattr(s, 'state') else None
    if a.mode == 'roll':
        s.req('down Up')
        v = a.kmh / 3.6; spin = v / 0.29545
        b = place.body(s, car); im = struct.unpack('<f', s.read(b + 0x168, 4))[0]
        fwd = rows[0]; mover = s.u32(car + 0x34); w = spin * a.ratio
        lines = [f'w {base + off + 16 * k:#x} ' + struct.pack('<3f', *r).hex() for base, off in ((b, 0x30), (b, 0xD0), (car, 0x90)) for k, r in enumerate(rows)]
        lines += [f'w {b + 0x80:#x} ' + bytes(12).hex()]
        lines += [f'w {b + 0x70:#x} ' + struct.pack('<3f', *(x * v / im for x in fwd)).hex(),
                  f'w {car + 0x70:#x} ' + struct.pack('<3f', *(x * v for x in fwd)).hex(),
                  f'w {car + 0x434:#x} ' + struct.pack('<i', a.gear).hex(),
                  f'w {s.u32(mover + 0x4C) + 0x3C:#x} ' + struct.pack('<i', a.gear).hex(),
                  f'w {s.u32(car + 0x30) + 0x1E4:#x} ' + struct.pack('<i', a.gear).hex(),
                  f'w {s.u32(mover + 0x4C) + 0x38:#x} ' + struct.pack('<i', a.gear).hex(),
                  f'w {s.u32(mover + 0x4C) + 0x48:#x} ' + struct.pack('<i', 0).hex(),
                  f'w {s.u32(mover + 0x4C) + 0x4C:#x} ' + struct.pack('<f', w).hex(),
                  f'w {s.u32(mover + 0x48) + 0x14:#x} ' + struct.pack('<f', w).hex()]
        lines += [f'w {s.u32(mover + o) + 0x28:#x} ' + struct.pack('<f', spin).hex() for o in (0x38, 0x3C, 0x40, 0x44)]
        gp = s.state['game_pid'] if hasattr(s, 'state') else sess.s.state['game_pid']
        os.kill(gp, signal.SIGSTOP)
        try: s.req(*lines)
        finally: os.kill(gp, signal.SIGCONT)
    if a.mode == 'launch':
        s.req('down Up')
    clock0 = s.u32(0x865188); N.log('rolling' if a.mode == 'roll' else 'standing', 'at clock', clock0)
    log = open(os.path.join(a.out, 'drive.tsv'), 'w'); log.write('clock\tt\tgear\trpm\tspeed\tx\ty\n')
    t0 = time.time()
    while time.time() - t0 < a.secs:
        r = s.req(f'r {car + 0x434:#x} 4', f'r {car + 0x440:#x} 4', f'r {car + 0x430:#x} 4')
        gear = struct.unpack('<i', bytes.fromhex(r[0].split()[1]))[0]; rpm = struct.unpack('<f', bytes.fromhex(r[1].split()[1]))[0]
        spd = struct.unpack('<f', bytes.fromhex(r[2].split()[1]))[0]; clk = s.u32(0x865188)
        log.write('%d\t%.3f\t%d\t%.0f\t%.2f\n' % (clk, (clk - clock0) / 4000, gear, rpm, spd * 3.6))
        time.sleep(0.02)
    s.req('up Up')
    N.log('recorded done')
finally:
    if watch: watch.stop()
    try:
        if a.fixed: c.uninstall()
    except Exception: pass
    sess.stop()
