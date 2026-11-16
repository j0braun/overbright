import sys,collections,math
def rows(p,native):
    for line in open(p):
        c=line.rstrip('\n').split('\t')
        if c[0]=='wall' or c[0].startswith('#') or len(c)<15: continue
        x,y,z=map(float,c[6].split(','))
        yield c[4],int(c[5]),x,y,(c[14] if native else c[16])=='1'
native=sys.argv[2]=='native'
st={}; C=collections.Counter(); T=collections.Counter()
for car,m,x,y,act in rows(sys.argv[1],native):
    s=st.get(car)
    if s is None: st[car]=[m,x,y,act,'start' if not act else None]; 
    else:
        jump=math.hypot(x-s[1],y-s[2])>40
        if not act and (s[3] or jump):
            s[4]=('placed from %d'%s[0]) if jump else ('dropped in mode %d->%d'%(s[0],m))
        if act: s[4]=None
        s[0],s[1],s[2],s[3]=m,x,y,act
    s=st[car]
    if not act: T[(s[4], m)]+=1
tot=sum(T.values())
for k,v in T.most_common(10): print(k, v, f'{100*v/tot:.0f}%')
