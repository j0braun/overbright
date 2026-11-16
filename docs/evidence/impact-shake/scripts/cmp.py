import csv,sys,math
CURVE=[(0,1),(0.1,0.7),(0.3,0.5),(0.5,0.4),(1,0)]
def curve(t):
    if t<0 or t>=1: return 0
    i=max(0,sum(1 for k in CURVE if t>=k[0])-1)
    (t0,v0),(t1,v1)=CURVE[i],CURVE[i+1]; return (t-t0)*((v1-v0)/(t1-t0))+v0
def F(r,k):
    try: return float(r[k])
    except: return float('nan')
def V(r,n): return [F(r,f'{n}.{c}') for c in 'xyz']
def cross(a,b): return [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
def norm(a): l=math.sqrt(sum(x*x for x in a)); return [x/l for x in a]
def dot(a,b): return sum(x*y for x,y in zip(a,b))
def tr(x): return int(x)
def main():
  for run in sys.argv[1:]:
   for side in ('native','clone'):
     rows=list(csv.DictReader(open(f'{run}/{side}.tsv'),delimiter='\t'))
     hit=next((i for i,r in enumerate(rows) if math.hypot(0.7*F(r,'c0.accel.x'),0.5*F(r,'c0.accel.y'),2*F(r,'c0.accel.z'))>=250),None)
     on=[i for i,r in enumerate(rows) if F(r,'shake_on')]
     if not on: print(run.split('/')[-1],side,'hit',hit,'no shake'); continue
     s=on[0]; e=on[-1]
     secs=sum(F(rows[i],'ticks')/1800 for i in range(s,e+2))
     vec=V(rows[s],'shake_vec'); amp=math.sqrt(dot(vec,vec))
     w=math.hypot(0.7*F(rows[s],'c0.accel.x'),0.5*F(rows[s],'c0.accel.y'),2*F(rows[s],'c0.accel.z'))
     # the views of each frame: t before that frame's step (0 on the trigger frame)
     peakd=peakr=0; visible=0
     for i in range(s,e+2):
         t=0 if i==s else F(rows[i-1],'shake_t')
         if tr(17*t)&1: continue
         c=curve(t); o=[x*c for x in vec]
         f=norm(V(rows[i+1] if i+1<len(rows) else rows[i],'cam_fwd'))
         r=norm(cross([0,0,-1],f)); d=norm(cross(f,r))
         T=[dot(o,r),dot(o,d),dot(o,f)]
         a=tr(-1200*(T[1]+T[2]))*360/65536; b=tr(2000*T[0])*360/65536
         visible+=1
         peakd=max(peakd,math.sqrt(dot(o,o))); peakr=max(peakr,math.hypot(a,b))
     # measured: eye / forward jump at the first strobe-off view
     print(f"{run.split('/')[-1]:20s} {side:6s} hit row {hit} w {w:6.1f} start row {s} (+{s-hit}) amp {amp:.4f} m dur {secs:.3f} s ({e-s+1} frames, {visible} shaken views) peak disp {peakd:.4f} m peak rot {peakr:.2f} deg")

if __name__=='__main__': main()