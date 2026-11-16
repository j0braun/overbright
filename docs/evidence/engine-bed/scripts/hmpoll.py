# hmpoll.py COPY OUT.tsv SECS LOG: the player's HybridMotor (CARSFX_PlayerEngine vtable 0x78AF30 +0xC0) fields
# every ~3 ms with the sim clock: +0x7C/+0x80/+0x84/+0xB8 ints, +0xD0, +0xF8, +0xFC, +0x100, +0x104, +0x108;
# Engine (HM+0x6C) +0x128/+0x12C rpm, +0x130 throttle; ctx (HM+0x44) +0x44 throttle, +0x48 rpm, +0x54 input.
import os, sys, struct, time
sys.path.insert(0, os.environ.get('WINE_TOOLS', '.'))
import peek
LOG = sys.argv[4]
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
eng = None
while eng is None: eng = find_eng()
hm = s.u32(eng + 0xC0); en = s.u32(hm + 0x6C); ctx = s.u32(hm + 0x44)
out = open(sys.argv[2], 'w', buffering=1)
out.write('clock\tframe\thm7c\thm80\thm84\thm88\thmd0\tf8\tfc\t100\t104\t108\te128\te12c\te130\te0\tc44\tc48\tc54\n')
t0 = time.time()
while time.time() - t0 < float(sys.argv[3]) and ' recorded ' not in open(LOG).read():
    try:
        h = s.read(hm + 0x7C, 0x90); e = s.read(en + 0xE0, 0x54); c = s.read(ctx + 0x44, 0x14)
        clk = s.u32(0x865188); fr = s.u32(0x8651A8)
        vals = struct.unpack_from('<4i', h, 0) + (struct.unpack_from('<f', h, 0xD0 - 0x7C)[0],) + struct.unpack_from('<5f', h, 0xF8 - 0x7C)
        ev = struct.unpack_from('<3f', e, 0x128 - 0xE0) + (struct.unpack_from('<i', e, 0)[0],)
        cv = (struct.unpack_from('<f', c, 0)[0], struct.unpack_from('<f', c, 4)[0], struct.unpack_from('<f', c, 0x10)[0])
        out.write('%d\t%d\t' % (clk, fr) + '\t'.join('%.4f' % v if isinstance(v, float) else str(v) for v in vals + ev + cv) + '\n')
    except Exception as ex: out.write('err %s\n' % ex)
    time.sleep(0.003)
