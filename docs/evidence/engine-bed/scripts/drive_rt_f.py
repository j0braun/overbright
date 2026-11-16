#!/usr/bin/env python3
"""drive_rt.py SCENARIO.toml OUT --copy C --display N: the scenario's career drag in the original at its
own pace (the cave only brings the start about and is then taken out: the world and the audio run in
real time), driven with real keys: Up from the green, Left Shift at the scenario's shift_up times and
the throttle's ups / downs at theirs (race clock). Records native.wav (+ sync), native-events.tsv
(EventWatch: every instance's parameters with the sim clock) and drive.tsv (sim clock, race clock,
gear, shown rpm, keys)."""
import argparse, os, sys, time, tomllib, struct, threading
P = os.environ['PARITY_TOOLS']; sys.path.insert(0, P)
import native as N, cave, nav, savepatch
ap = argparse.ArgumentParser(); ap.add_argument('scenario'); ap.add_argument('out')
ap.add_argument('--copy'); ap.add_argument('--display', type=int, default=196)
a = ap.parse_args()
scen = tomllib.load(open(a.scenario, 'rb')); os.makedirs(a.out, exist_ok=True)
save = os.path.expanduser(scen['save'])
p = os.path.join(a.out, 'scenario.sav'); open(p, 'wb').write(savepatch.for_scenario(open(save, 'rb').read(), scen['start'])); save = p
sess = N.Session(a.copy, a.display); watch = None
KEY = {'throttle': 'Up', 'shift_up': 'Shift_L'}
try:
    s = sess.start(save, scen.get('profile', 'NAME'), os.path.join(os.path.abspath(a.out), 'native.wav'))
    n = nav.Nav(s, a.out, N.log); n.boot_to_freeroam(); N.log('free roam')
    c = cave.Cave(s, a.out); c.install(); c.fixed(0)
    N.start_career_event(s, scen, n); N.accept_race(s, n, c, 3)
    N.log('held at the countdown')
    watch = N.EventWatch(a.copy, os.path.join(a.out, 'native-events.tsv'))
    N.log('inputs started')
    if os.environ.get('RT_FIXED'):
        # The cave stays with every frame 1000/RT_FIXED ms (no recording, no lock): the original's cadence
        # at its frame cap whatever the wall clock (AudioManager every 2nd frame at 120: 60 Hz).
        c.w32('LOCK', 0); c.w32('ACK', 0x7FFFFFFF); c.w32('ENABLE', 0); c.fixed(int(os.environ['RT_FIXED'])); N.log('cave fixed')
    else:
        c.uninstall(); N.log('cave out: own pace')
    st = s.u32(N.STATUS)
    while s.u32(st) != 4: time.sleep(0.002)
    import fields as F
    w = s.u32(F.WORLD)
    player = [s.u32(w + 0x1C + 4 * i) for i in range(s.u32(w + 0x18)) if s.u32(s.u32(w + 0x1C + 4 * i) + 0x24) == 1][0]
    todo = sorted(scen['run']['inputs'], key=lambda x: x['t']); k = 0
    log = open(os.path.join(a.out, 'drive.tsv'), 'w'); log.write('clock\trace_clock\tgear\trpm\tkey\n')
    green = s.u32(0x865188); N.log('green at clock', green)
    while True:
        clock = s.u32(0x865188); t = (clock - green) / 4000
        key = ''
        while k < len(todo) and todo[k]['t'] <= t:
            i = todo[k]; k += 1
            s.req(('down ' if i.get('down', True) else 'up ') + KEY[i['key']]); key += '%s%s ' % (i['key'], '' if i.get('down', True) else '-up')
        r = s.req(f'r {player + 0x434:#x} 4', f'r {player + 0x440:#x} 4')
        gear = struct.unpack('<i', bytes.fromhex(r[0].split()[1]))[0]; rpm = struct.unpack('<f', bytes.fromhex(r[1].split()[1]))[0]
        log.write('%d\t%.3f\t%d\t%.0f\t%s\n' % (clock, t, gear, rpm, key))
        if t > scen['run']['seconds'] + 0.5: break
        time.sleep(0.004)
    for kk in KEY.values(): s.req('up ' + kk)
    N.log('recorded done', t)
finally:
    if watch: watch.stop()
    sess.stop()
