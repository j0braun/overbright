import sys, numpy as np
rows = [l.split() for l in open(sys.argv[1])]
a, b = int(open(sys.argv[2]).read()), int(open(sys.argv[3]).read())
r = rows[a:b]
t = np.array([int(x[0]) for x in r], dtype=np.float64) * 1e-9
up = np.array([int(x[1]) for x in r]); al = np.array([float(x[2]) for x in r], dtype=np.float32)
p = np.array([[float(v) for v in x[3:6]] for x in r], dtype=np.float32)
c = np.array([[float(v) for v in x[6:9]] for x in r], dtype=np.float32)
dt = np.diff(t)
print(f"frames {len(r)}  frame time ms: mean {dt.mean()*1e3:.2f} p50 {np.median(dt)*1e3:.2f} p95 {np.percentile(dt,95)*1e3:.2f} max {dt.max()*1e3:.2f}; ticks/frame {np.bincount(up).tolist()}")
v = np.diff(p.astype(np.float64), axis=0) / dt[:, None]
sp = np.linalg.norm(v, axis=1)
acc = np.linalg.norm(np.diff(v, axis=0), axis=1) / dt[1:]
print(f"drawn speed m/s: mean {sp.mean():.2f} sd {sp.std():.2f} (cv {sp.std()/max(sp.mean(),1e-9)*100:.1f} %) min {sp.min():.2f} max {sp.max():.2f}")
print(f"second difference |d2p/dt2| m/s2: p50 {np.median(acc):.1f} p95 {np.percentile(acc,95):.1f} max {acc.max():.1f}")
# the tick positions alone (S_n, steps of 1/60 s) for reference
tick = np.flatnonzero(up > 0)
# base's blend reconstructed: prev = the tick position before the frame's newest (one-tick frames only)
diff = []; n = 0
prev = None
for k in range(len(r)):
    if up[k] == 1 and k > 0:
        prev = c[k - 1]
    elif up[k] > 1:
        prev = None
    if prev is None or al[k] >= 1.0 or al[k] <= 0.0 and up[k] == 0:
        continue
    s = np.float32(al[k])
    base = prev * (np.float32(1) - s) + c[k] * s
    diff.append(np.abs(base - p[k]).max()); n += 1
print(f"base's lerp on the same frames: {n} frames compared, max |base - new| {max(diff) if diff else 0:.6f} m (trace has 6 decimals)")
