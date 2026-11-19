#!/usr/bin/env python3
"""musmon3.py COPY OUT.tsv SECS: music object state (+0xC0), song (+0x14), stream id 0x82AFD4, level +0xBC>>8,
audio mode +0x90, pause flag +0x13 / mode +0x50, game state 0x8654A4, flow [0x8900D8], race status [0x890118],
PC_Loading-ish 0x4A9330 not callable -> omitted; NIS objects 0x827E98."""
import sys, struct, time
sys.path.insert(0, sys.argv[1]); import peek
s = peek.Session(sys.argv[1]); out = open(sys.argv[2], 'a'); secs = float(sys.argv[3])
t0 = time.time(); last = None
out.write('t\ttick\tstate\tsong\tstream\tlvl\tfe90\tp13\tp50\tgs\tflow\trstat\tnis\n')
while time.time() - t0 < secs:
    try:
        a = s.u32(0x82B884); m = s.u32(a + 0xA0); mb = s.read(m, 0xD4); ab = s.read(a, 0x94)
        rs = s.u32(0x890118); fl = s.u32(0x8900D8)
        row = (struct.unpack_from('<i', mb, 0xC0)[0], struct.unpack_from('<i', mb, 0x14)[0],
               struct.unpack('<i', s.read(0x82AFD4, 4))[0], struct.unpack_from('<i', mb, 0xBC)[0] >> 8,
               struct.unpack_from('<i', ab, 0x90)[0], ab[0x13], struct.unpack_from('<i', ab, 0x50)[0],
               s.u32(0x8654A4), s.u32(fl) if fl else -1, s.u32(rs) if rs else -1, int(s.u32(0x827E98) != 0))
        if row != last:
            out.write('%.2f\t%d\t' % (time.time() - t0, s.u32(0x865188)) + '\t'.join(map(str, row)) + '\n'); out.flush(); last = row
    except Exception as e:
        out.write('%.2f\terr %s\n' % (time.time() - t0, e)); out.flush(); time.sleep(0.5)
    time.sleep(0.05)
