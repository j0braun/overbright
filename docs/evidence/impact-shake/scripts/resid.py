import csv,sys,math
sys.path.insert(0,sys.path[0]); 
from cmp import curve,F,V,tr,dot,norm,cross
run,side=sys.argv[1],sys.argv[2]
rows=list(csv.DictReader(open(f'{run}/{side}.tsv'),delimiter='\t'))
on=[i for i,r in enumerate(rows) if F(r,'shake_on')]; s,e=on[0],on[-1]
vec=V(rows[s],'shake_vec')
# camera recorded in row i+1 is the view of frame i (t before its step)
pred={}
for i in range(s,e+2):
    t=0 if i==s else F(rows[i-1],'shake_t')
    c=0 if tr(17*t)&1 else curve(t)
    pred[i]=[-x*c for x in vec]
off=[j for j in pred if pred[j]==[0,0,0] or all(abs(x)<1e-9 for x in pred[j])]
for j in sorted(pred):
    if j in off: continue
    a=max([k for k in off if k<j],default=None); b=min([k for k in off if k>j],default=None)
    if a is None or b is None: continue
    ea,eb=V(rows[a],'cam_eye'),V(rows[b],'cam_eye')
    base=[x+(y-x)*(j-a)/(b-a) for x,y in zip(ea,eb)]
    res=[x-y for x,y in zip(V(rows[j],'cam_eye'),base)]
    print(j,'pred',[round(x,3) for x in pred[j]],'meas',[round(x,3) for x in res])
