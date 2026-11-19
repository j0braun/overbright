#!/usr/bin/env python3
"""Frame budget tables (docs/notes/performance/budget.md) from a budget.sh run.

  budget.py --presets          shell lines exporting PRESET_<name> (the leave-one-out presets; @MAX@ = perf.sh's MAX)
  budget.py --configs WxH      the leave-one-out configs at that size
  budget.py OUT [--label L]    OUT/budget.md: preset totals and per-effect costs
"""
import os, sys, statistics as st
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import perfstat as ps

# name, row (Graphics page), lines appended to MAX, heavy scene, UI hint at 4K (graphics_table.rs *_COST_4K), target budget
EFFECTS = [
    ('mloff', 'Modern Lighting (Modern + RT -> Original)', 'enh.modern_lighting 0', 'racing', 5.58, '<= +2 ms 1080p (Modern)'),
    ('mlnort', 'Modern Lighting RT part (Modern + RT -> Modern)', 'enh.modern_lighting 2', 'racing', 3.3, ''),
    ('smokeoff', 'Particles Cinematic -> Original', 'enh.particles 0', 'explore', 0.21, '<= +1.5 ms 4K heavy'),
    ('smokeultra', 'Particles Cinematic -> Ultra (Cinematic minus Ultra)', 'enh.particles 2', 'explore', None, ''),
    ('wateroff', 'Animated Water Realistic -> Off', 'enh.water 0', 'water', 1.65, '<= +1.5 ms 4K fountain'),
    ('wateron', 'Animated Water Realistic -> On', 'enh.water 1', 'water', None, ''),
    ('wetintoff', 'Wet Road Interaction High -> Off', 'enh.wet_interaction 0', 'rain', 0.82, '<= +1 ms 1080p'),
    ('wetroadsoff', 'Wet Roads -> Off', 'enh.wet_roads 0', 'rain', None, ''),
    ('raindetoff', 'Rain Detail High -> Off', 'enh.rain_detail 0', 'rain', 0.27, ''),
    ('sprayoff', 'Spray & Splashes -> Off', 'enh.spray 0', 'rain', 0.25, ''),
    ('lensoff', 'Lens Drops -> Off', 'enh.lens_drops 0', 'rain', 0.01, ''),
    ('wetcaroff', 'Wet Car -> Off', 'enh.wet_car 0', 'rain', 0.02, ''),
    ('ssroff', 'SSR Ultra (full) -> Off', 'enh.ssr 0', 'rain', None, ''),
    ('ssrrt', 'SSR full -> SSR + RT (step up)', 'enh.ssr 3', 'rain', None, ''),
    ('vfogoff', 'Volumetric Fog Low -> Off', 'enh.volumetric_fog 0', 'rain', 0.27, ''),
    ('vfoghigh', 'Volumetric Fog Low -> High (step up)', 'enh.volumetric_fog 2', 'rain', 0.46, ''),
    ('beamsoff', 'Headlight Beams -> Off', 'enh.headlight_beams 0', 'rain', 0.21, ''),
    ('surfoff', 'Surface Detail Mat + Normals -> Off', 'enh.surface_detail 0', 'rain', 0.09, ''),
    ('ssaooff', 'SSAO 25 % -> Off', 'enh.ssao 0', 'racing', None, ''),
    ('ssaomax', 'SSAO 25 % -> 100 % (step up)', 'enh.ssao 100', 'racing', None, ''),
    ('lampshoff', 'Lamp Shadows High -> Off', 'enh.lamp_shadows 0', 'racing', None, ''),
    ('lampsoff', 'Dynamic Lamps -> Off', 'enh.dynamic_lamps 0', 'racing', None, ''),
    ('roadrefoff', 'Road Reflection Ultra -> Off', 'enh.road_reflection 0', 'racing', None, ''),
    ('enhrefoff', 'Enhanced Reflections -> Off', 'enh.enhanced_reflections 0', 'racing', None, ''),
    ('hdrpostoff', 'High Precision Post (full glow) -> Off', 'enh.hdr_post 0', 'racing', None, ''),
    ('mboff', 'Motion Blur Per-Object -> Original', 'enh.motion_blur 0', 'racing', 1.16, ''),
    ('dofoff', 'Depth of Field Cinematic -> Original', 'enh.depth_of_field 0', 'racing', 1.3, ''),
    ('probesoff', 'Light Probes -> Off', 'enh.light_probes 0', 'racing', 0.0, ''),
    ('aaoff', 'Anti-Aliasing SSAA 2x -> Native (8x MSAA)', 'enh.post_aa 0;enh.aa_msaa 0', 'racing', None, ''),
    ('glassoff', 'Enhanced Glass -> Off', 'enh.enhanced_glass 0', 'race', 0.33, ''),
    ('carrefoff', 'Car Reflection 1024 -> native 128', 'enh.car_reflection 128', 'race', None, ''),
    ('shadowsoff', 'Car Shadow Ultra -> native', 'enh.shadows 0', 'race', None, ''),
    ('contactoff', 'Contact Shadows -> Off', 'enh.contact_shadows 0', 'race', None, ''),
    ('paintoff', 'Enhanced Car Paint -> Off', 'enh.enhanced_paint 0', 'race', None, ''),
    ('smoothoff', 'Smooth Car Shapes -> Off', 'enh.smooth_cars 0', 'race', 0.0, ''),
    ('fardrawoff', 'World Detail Extreme -> Off', 'enh.far_draw 0', 'explore', 0.1, ''),
    ('lodfadeoff', 'LOD Fade -> Off', 'enh.lod_fade 0', 'explore', None, ''),
    ('detailoff', 'World / Car Detail 200 % + Keep All Loaded -> native', 'enh.detail_distance 100;enh.car_detail 100;enh.keep_loaded 0', 'explore', None, ''),
]
# The row's own GPU passes (timestamps) in the max run: substrings of the pass names ('=name' exact).
OWN = {'mlnort': '', 'smokeoff': 'smoke cinematic', 'smokeultra': 'smoke cinematic', 'wateroff': 'water', 'wateron': 'water', 'wetintoff': 'wet road map', 'raindetoff': 'weather fx', 'sprayoff': 'weather fx', 'lensoff': 'lens drops', 'ssroff': 'ssr ', 'vfogoff': 'volumetric fog', 'ssaooff': 'ssao', 'lampshoff': 'lamp shadows', 'roadrefoff': 'road reflection', 'hdrpostoff': 'overbright (high precision)', 'mboff': 'motion blur', 'dofoff': 'depth of field', 'probesoff': 'light probes', 'aaoff': 'render scale', 'glassoff': 'enhanced glass', 'carrefoff': 'environment cube', 'shadowsoff': '=shadows'}
PRESETS = [('orig', 'Original'), ('modern', 'Modern (preset)'), ('ultra26', 'Ultra (Preset::Ultra of the first budget run)'), ('extreme', 'Extreme (preset)'),
           ('user', 'the play setup (PRESET_user)'), ('max', 'everything max (play setup + newest rows)')]
SCENES = ['explore', 'racing', 'race', 'rain', 'water']
SIZES = ['1920x1080', '3840x2160']


def med(runs, m):
    v = ps.values(runs, m)
    return st.median(v) if v else None


def f(v, d=2):
    return '–' if v is None else f'{v:.{d}f}'


def main(a):
    if a[0] == '--presets':
        for name, _, lines, *_ in EFFECTS:
            print(f"export PRESET_{name}='@MAX@;{lines}'")
        return 0
    if a[0] == '--configs':
        for name, _, _, scene, *_ in EFFECTS:
            print(f'{scene}-{name}-{a[1]}')
        return 0
    out = a[0]
    label = a[a.index('--label') + 1] if '--label' in a else 'main'
    data = ps.load_dir(out)[label]
    L = []
    nmax = max((len(r) for r in data.values()), default=0)
    L.append(f'## Preset totals ({label}; median over {nmax} runs of the run medians)\n')
    L.append('GPU = GPU busy median ms/frame (timestamps), p95 in brackets; CPU = main thread / render workers ms/frame (thread CPU).\n')
    for size in SIZES:
        hdr = '| preset | ' + ' | '.join(SCENES) + ' |'
        rows = []
        for p, pname in PRESETS:
            cells = []
            for s in SCENES:
                r = data.get(f'{s}-{p}-{size}')
                if not r:
                    cells.append('–'); continue
                cells.append(f"{f(med(r, 'gpu_med'))} [{f(med(r, 'gpu_p95'))}] · {f(med(r, 'cpu_main'))} / {f(med(r, 'cpu_render'))}")
            if any(c != '–' for c in cells):
                rows.append(f'| {pname} | ' + ' | '.join(cells) + ' |')
        if rows:
            L += [f'### {size}\n', hdr, '|---|' + '---|' * len(SCENES)] + rows + ['']
    if any(k.split('-')[1] == name for k in data for name, *_ in EFFECTS):
        L.append('## Per effect: everything max minus the row (leave-one-out), its heavy scene\n')
        L.append('Cost = max - (max with the row down): GPU busy median ms (p of the run-level Mann-Whitney U, noise %), main thread / render workers CPU ms. '
                 'A "step up" row is the other way: (max with the row up) - max. UI = the Graphics page\'s 4K cost hint (graphics_table.rs).\n')
        L.append('| row | scene | 1080p GPU (p, noise) | 1080p CPU main / workers | 4K GPU (p, noise) | 4K CPU main / workers | own passes 1080p / 4K (max) | UI hint 4K | budget |')
        L.append('|---|---|---|---|---|---|---|---|---|')
        for name, row, lines, scene, hint, budget in EFFECTS:
            cells = []
            for size in SIZES:
                b, x = data.get(f'{scene}-max-{size}'), data.get(f'{scene}-{name}-{size}')
                if not b or not x:
                    cells += ['–', '–']; continue
                up = 'step up' in row
                hi, lo = (x, b) if up else (b, x)
                vb, vx = ps.values(lo, 'gpu_med'), ps.values(hi, 'gpu_med')
                if not vb or not vx:  # runs without GPU timestamps: no cost, not a crash
                    cells += ['no GPU values', '–']; continue
                p, _ = ps.mannwhitney(vb, vx)
                nz = ps.noise([vb, vx])
                d = st.median(vx) - st.median(vb)
                dm = (med(hi, 'cpu_main') or 0) - (med(lo, 'cpu_main') or 0)
                dr = (med(hi, 'cpu_render') or 0) - (med(lo, 'cpu_render') or 0)
                cells += [f'{d:+.2f} ({p:.2f}, {100 * nz:.0f} %)', f'{dm:+.2f} / {dr:+.2f}']
            ownc = []
            for size in SIZES:
                b = data.get(f'{scene}-max-{size}')
                key = OWN.get(name)
                if not key or not b:
                    ownc.append('–'); continue
                hit = (lambda n: n == key[1:]) if key.startswith('=') else (lambda n: key in n)
                ownc.append(f(st.median([sum(v for n, v in r['gpu_parts'].items() if hit(n)) for r in b])))
            L.append(f'| {row} | {scene} | ' + ' | '.join(cells) + f" | {' / '.join(ownc)} | {f(hint)} | {budget} |")
        L.append('')
    text = '\n'.join(L)
    open(os.path.join(out, 'budget.md'), 'w').write(text + '\n')
    print(text)
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
