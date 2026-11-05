import sys,re,subprocess,collections
data=sys.argv[1]; targets=sys.argv[2:]
out=subprocess.run(['perf','script','-i',data,'-F','ip,sym'],capture_output=True,text=True).stdout
samples=[];cur=[]
for line in out.split('\n'):
    if not line.strip():
        if cur: samples.append(cur);cur=[]
        continue
    p=line.strip().split(None,1); cur.append(p[1] if len(p)>1 else '?')
def norm(s):
    s=s.replace(' (inlined)',''); prev=None
    while prev!=s:
        prev=s; s=re.sub(r'(?<=[A-Za-z0-9_#}])<[^<>]*>','',s)
    return s
loop=[[norm(x) for x in s] for s in samples if any('perf_kernel::Game::play' in x for x in s)]
N=len(loop)
for t in targets:
    c=collections.Counter()
    for ns in loop:
        if not any(t in x for x in ns): continue
        i=next(i for i,x in enumerate(ns) if t in x)
        chain=[]
        for x in ns[i+1:]:
            if re.match(r'^<?ti4_',x) and (not chain or chain[-1]!=x): chain.append(x)
            if len(chain)>=7: break
        c[' <- '.join(y.replace('ti4_engine_core_factions','F').replace('ti4_engine_core','C').replace('ti4_engine','E') for y in chain)]+=1
    tot=sum(c.values())
    print(f'### {t}: {100*tot/N:.2f}% of loop')
    for k,v in c.most_common(4): print(f'   {100*v/N:5.2f}%  {k}')
