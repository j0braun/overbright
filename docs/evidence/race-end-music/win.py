"""Instrumented human win (as lose.py, the human's block only): laps and the finish-line tick."""
import sys, struct, json
C = sys.argv[1]; sys.path.insert(0, C); import peek
s = peek.Session(C)
rs = s.u32(0x890118); assert s.u32(rs) == 4, 'race not running'
pc = s.u32(s.u32(0x8900ac) + 4)
laps = s.read(rs + 0xbe8, 1)[0]; fl = s.read(rs + 0xbf7, 1)[0]
n = s.u32(rs + 0x24)
blks = [s.u32(rs + 0xbd0 + 4 * k) for k in range(n)]
human = next(b for b in blks if s.u32(b) == pc)
now = s.u32(0x865188)
w = [(human + 0x30 + 4 * fl, struct.pack('<I', now)), (human + 8, bytes([laps, laps]))]
s.req(*(f'w {a:#x} {v.hex()}' for a, v in w))
print(json.dumps(dict(laps=laps, fl=fl, human=hex(human), tick=now)))
