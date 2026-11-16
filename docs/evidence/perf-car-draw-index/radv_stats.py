# Usage: RADV_DEBUG=shaderstats,nir MESA_SHADER_CACHE_DISABLE=true game --bench race 8 2> run.err; python3 radv_stats.py run.err
# Pair RADV_DEBUG=shaderstats,nir blocks: per pipeline FS NIR, VS NIR, then Pixel / Vertex stats.
import sys, re, collections
lines = open(sys.argv[1], errors='replace').read().split('\n')
cur = None; last = {'FRAGMENT': None, 'VERTEX': None}
recs = []; i = 0
while i < len(lines):
    l = lines[i]
    m = re.match(r'^shader: MESA_SHADER_(\w+)', l)
    if m:
        cur = {'stage': m.group(1), 'entry': '?', 'cube': 0, 'ssbo': 0, 'smem_ssbo': 0, 'push': 0, 'scratch': 0, 'flat': 0}
        last[m.group(1)] = cur
    elif cur is not None:
        if '(entrypoint)' in l and 'decl_function ' in l:
            cur['entry'] = l.split('decl_function ')[1].split(' ')[0]
        if 'textureCube' in l: cur['cube'] = 1
        if l.startswith('num_ssbos:'): cur['ssbo'] = int(l.split(':')[1])
        if l.startswith('decl_var push_const') or 'push_const' in l.split(' ')[1:2]: cur['push'] = 1
        if '@load_scratch' in l or '@store_scratch' in l: cur['scratch'] += 1
    hm = re.match(r'^(Pixel Shader|Vertex Shader as \w+|Compute Shader):$', l)
    if hm:
        st = {}
        j = i + 2
        while j < len(lines) and not lines[j].startswith('****'):
            if ':' in lines[j]:
                k, v = lines[j].split(':', 1); st[k.strip()] = v.strip()
            j += 1
        stage = 'FRAGMENT' if hm.group(1) == 'Pixel Shader' else ('VERTEX' if 'Vertex' in hm.group(1) else 'COMPUTE')
        nir = last.get(stage)
        if stage in last: last[stage] = None
        recs.append((stage, nir, st)); i = j
    i += 1
c = collections.Counter()
for stage, nir, st in recs:
    if stage == 'COMPUTE': continue
    n = nir or {'entry': '(nir reused)', 'cube': '-', 'ssbo': '-', 'smem_ssbo': '-', 'push': '-', 'scratch': '-'}
    key = f"{stage[:4]} {n['entry']:<26} cube={n['cube']} ssbo={n['ssbo']:>3}/smem {n['smem_ssbo']:>3} push={n['push']:>2} nirscratch={n['scratch']:>3} VGPR={st.get('VGPRs')} SGPR={st.get('SGPRs')} scratch={st.get('Scratch size')} waves={st.get('Subgroups per SIMD')} instr={st.get('Instructions')} VMEM={st.get('VMEM')} SMEM={st.get('SMEM')} lat={st.get('Latency')}"
    c[key] += 1
for k, v in sorted(c.items(), key=lambda kv: kv[0]):
    print(f"{v:4d} {k}")
