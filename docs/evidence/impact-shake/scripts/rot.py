import csv,sys,math
from cmp import curve,F,V,tr,dot,norm,cross
def rotx(a,v): c,s=math.cos(a),math.sin(a); return [v[0],c*v[1]-s*v[2],s*v[1]+c*v[2]]
def roty(a,v): c,s=math.cos(a),math.sin(a); return [c*v[0]+s*v[2],v[1],-s*v[0]+c*v[2]]
run,side=sys.argv[1],sys.argv[2]
rows=list(csv.DictReader(open(f'{run}/{side}.tsv'),delimiter='\t'))
on=[i for i,r in enumerate(rows) if F(r,'shake_on')]; s,e=on[0],on[-1]
vec=V(rows[s],'shake_vec')
off=[]; pred={}
for i in range(s,e+2):
    t=0 if i==s else F(rows[i-1],'shake_t'); c=0 if tr(17*t)&1 else curve(t)
    if c==0: off.append(i)
    pred[i]=c
for j in sorted(pred):
    if j in off: continue
    a=max([k for k in off if k<j],default=None); b=min([k for k in off if k>j],default=None)
    if a is None or b is None: continue
    fa,fb=V(rows[a],'cam_fwd'),V(rows[b],'cam_fwd')
    f=norm([x+(y-x)*(j-a)/(b-a) for x,y in zip(fa,fb)])
    r=norm(cross([0,0,-1],f)); d=norm(cross(f,r))
    o=[x*pred[j] for x in vec]; T=[dot(o,r),dot(o,d),dot(o,f)]
    A=tr(-1200*(T[1]+T[2]))*2*math.pi/65536; B=tr(2000*T[0])*2*math.pi/65536
    # new forward = basis * (Rx^T Ry^T e_z)
    v=rotx(-A,roty(-B,[0,0,1]))
    nf=[r[k]*v[0]+d[k]*v[1]+f[k]*v[2] for k in range(3)]
    meas=V(rows[j],'cam_fwd')
    ang=lambda u,w: math.degrees(math.acos(max(-1,min(1,dot(norm(u),norm(w))))))
    print(j,'pred turn %.3f° meas turn %.3f°  pred-vs-meas %.3f°'%(ang(nf,f),ang(meas,f),ang(nf,meas)))
