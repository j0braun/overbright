import csv,sys,math
def load(p):
    return list(csv.DictReader(open(p),delimiter='\t'))
def f(r,k):
    try: return float(r[k])
    except: return float('nan')
for side in ('native','clone'):
    rows=load(sys.argv[1]+f'/{side}.tsv')
    print('==',side,len(rows), [k for k in rows[0] if 'shake' in k or 'accel' in k])
    for i,r in enumerate(rows):
        on=f(r,'shake_on'); 
        a=[f(r,'c0.accel.'+c) for c in 'xyz']
        w=math.sqrt((0.7*a[0])**2+(0.5*a[1])**2+(2*a[2])**2) if not any(map(math.isnan,a)) else float('nan')
        if (on and not math.isnan(on)) or w>200:
            print(i, 'on' if on else '--', round(f(r,'shake_t'),4), [round(f(r,'shake_vec.'+c),4) for c in 'xyz'], 'w',round(w,1), 'eye',[round(f(r,'cam_eye.'+c),3) for c in 'xyz'],'fwd',[round(f(r,'cam_fwd.'+c),4) for c in 'xyz'])
