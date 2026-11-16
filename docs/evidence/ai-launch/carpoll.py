#!/usr/bin/env python3
"""carpoll.py COPY OUT LOG: from 'held at the countdown' in LOG until 'recorded' (or 40 s), as fast as the
supervisor answers: sim clock, then per world car (0x890080 list): owner kind +0x24, gear +0x434, rpm +0x440,
speed +0x430, gas (+0x30 > +0x20C), clutch (+0x34 > +0x4C > +0x14), wheel spins (+0x34 > +0x38.. > +0x28),
the exported slip pairs car +0x3B4 + 8i."""
import os, sys, struct, time
sys.path.insert(0, os.environ['WINE_TOOLS'])   # the Wine tools' folder (peek.py)
import peek
copy, out, log = sys.argv[1:4]
while 'held at the countdown' not in open(log).read(): time.sleep(0.2)
s = peek.Session(copy)
w = s.u32(0x890080); n = s.u32(w + 0x10)
cars = [s.u32(w + 0x1C + 4 * i) for i in range(n)]
info = []
for c in cars:
    kind = s.u32(c + 0x24)
    if kind not in (1, 3): continue
    mover = s.u32(c + 0x34)
    wheels = struct.unpack('<4I', s.read(mover + 0x38, 16))
    clutch = s.u32(mover + 0x4C); ctl = s.u32(c + 0x30)
    info.append((c, kind, wheels, clutch, ctl))
f = open(out, 'w', buffering=1)
f.write('clock\tcar\tkind\tgear\trpm\tspeed\tgas\tclutch\tspin_fl\tspin_fr\tspin_rr\tspin_rl\tx_fl\tl_fl\tx_fr\tl_fr\tx_rr\tl_rr\tx_rl\tl_rl\n')
t0 = time.time()
while time.time() - t0 < 40 and ' recorded' not in open(log).read():
    lines = ['r 0x865188 4']
    for c, kind, wh, cl, ctl in info:
        lines += [f'r {c + 0x430:#x} 20', f'r {ctl + 0x20C:#x} 4', f'r {cl + 0x14:#x} 4'] + [f'r {x + 0x28:#x} 4' for x in wh] + [f'r {c + 0x3B4:#x} 32']
    try: r = s.req(*lines)
    except Exception as e: f.write('#err %s\n' % e); continue
    b = [bytes.fromhex(x.split()[1]) for x in r]
    clock = struct.unpack('<I', b[0])[0]; k = 1
    for i, (c, kind, wh, cl, ctl) in enumerate(info):
        speed, gear, _, _, rpm = struct.unpack('<fiiif', b[k]); gas = struct.unpack('<f', b[k + 1])[0]; clu = struct.unpack('<f', b[k + 2])[0]
        spins = [struct.unpack('<f', b[k + 3 + j])[0] for j in range(4)]; xs = struct.unpack('<8f', b[k + 7]); k += 8
        f.write('%d\t%d\t%d\t%d\t%.0f\t%.3f\t%.3f\t%.3f\t%s\t%s\n' % (clock, i, kind, gear, rpm, speed, gas, clu,
                '\t'.join('%.3f' % v for v in spins), '\t'.join('%.3f' % v for v in xs)))
