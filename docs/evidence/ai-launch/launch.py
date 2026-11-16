#!/usr/bin/env python3
"""launch.py RUN [native|clone] [STEP] [SECS]: per AI car the launch quantities every STEP frames."""
import csv, sys, os
run = sys.argv[1]; side = sys.argv[2] if len(sys.argv) > 2 else 'native'
step = int(sys.argv[3]) if len(sys.argv) > 3 else 6; secs = float(sys.argv[4]) if len(sys.argv) > 4 else 3
rows = list(csv.DictReader(open(os.path.join(run, side + '.tsv')), delimiter='\t'))
cars = sorted({k.split('.')[0] for k in rows[0] if k.startswith('c') and k.endswith('.gear')})
def g(r, c, k):
    v = r.get(f'{c}.{k}', '')
    try: return float(v)
    except: return float('nan')
for c in cars:
    print('==', c, side, run)
    print('fr   gear  rpm    v     gas  clutch  spin fl/fr/rr/rl           xslip fl/fr/rr/rl            slide fl/fr/rr/rl  W90  W98')
    for i, r in enumerate(rows):
        if i % step or i > secs * 60: continue
        x = [g(r, c, 'xslip_' + w) for w in ('fl', 'fr', 'rr', 'rl')]
        on = int(g(r, c, 'wheels')) if g(r, c, 'wheels') == g(r, c, 'wheels') else 0
        print('%3d %4.0f %6.0f %5.2f %5.2f %5.2f  %s  %s  %s %5.2f %5.2f' % (
            i, g(r, c, 'gear'), g(r, c, 'rpm'), g(r, c, 'speed'), g(r, c, 'gas'), g(r, c, 'clutch'),
            ' '.join('%6.2f' % g(r, c, 'spin_' + w) for w in ('fl', 'fr', 'rr', 'rl')),
            ' '.join('%6.2f' % v for v in x),
            ''.join('%d' % g(r, c, 'slide_' + w) if g(r, c, 'slide_' + w) == g(r, c, 'slide_' + w) else '-' for w in ('fl', 'fr', 'rr', 'rl')),
            x[1] + x[2], x[0] + x[3]))
