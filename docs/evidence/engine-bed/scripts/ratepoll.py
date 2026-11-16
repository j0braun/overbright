# ratepoll.py COPY OUT LOG: audio update rate (ctx +0x48 changes) and frame rate (world counter 0x7FB724) vs the sim clock
import os, sys, struct, time
sys.path.insert(0, os.environ.get('WINE_TOOLS', '.'))
import peek
LOG = sys.argv[3]
while ' inputs ' not in open(LOG).read(): time.sleep(0.2)
s = peek.Session(sys.argv[1]); VT = struct.pack('<I', 0x78AF30)
def find_eng():
    for a in range(0x3000000, 0x7000000, 0x100000):
        try: b = s.read(a, 0x100000)
        except Exception: continue
        i = b.find(VT)
        while i >= 0:
            if i % 4 == 0:
                hm = s.u32(a + i + 0xC0)
                if 0x10000 < hm < 0x7fffffff: return a + i
            i = b.find(VT, i + 1)
eng = find_eng(); ctx = s.u32(s.u32(eng + 0xC0) + 0x44)
out = open(sys.argv[2], 'w'); last = None; n = 0; t0 = time.time(); c0 = s.u32(0x865188); w0 = s.u32(0x7FB724); f0=s.u32(0x8651A8)
while time.time() - t0 < 6:
    v = s.read(ctx + 0x48, 4)
    if v != last: n += 1; last = v
c1 = s.u32(0x865188); w1 = s.u32(0x7FB724); f1=s.u32(0x8651A8)
out.write('wall %.2f s, sim %.2f s, real %.2f s, ctx rpm changes %d (%.1f /sim s), world counter %d (%.1f /sim s)\n' % (time.time() - t0, (c1 - c0) / 4000, (f1-f0)/4000, n, n / ((c1 - c0) / 4000), w1 - w0, (w1 - w0) / ((c1 - c0) / 4000)))
