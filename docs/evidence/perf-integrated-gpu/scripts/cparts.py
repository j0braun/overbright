import sys, json, glob, os, statistics as st, collections
d=sys.argv[1]; want=sys.argv[2:]
v=collections.defaultdict(list)
for f in glob.glob(os.path.join(d,'runs','*.json')):
    lab,cfg,rep=os.path.basename(f)[:-5].rsplit('__',2)
    try: j=json.load(open(f))
    except Exception: continue
    for p in j.get('cpu_parts',[]):
        if any(w in p['name'] for w in want): v[(cfg,p['name'].strip(),lab)].append(p['ms_per_frame'])
for k in sorted(v): print(f'{k[0]:22s} {k[1]:32s} {k[2]:6s} {st.median(v[k]):.3f}  n={len(v[k])}')
