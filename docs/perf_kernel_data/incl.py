import sys,re,subprocess
data=sys.argv[1]; pats=sys.argv[2:]
out=subprocess.run(['perf','script','-i',data,'-F','ip,sym'],capture_output=True,text=True).stdout
samples=[];cur=[]
for line in out.split('\n'):
    if not line.strip():
        if cur: samples.append(cur);cur=[]
        continue
    p=line.strip().split(None,1); cur.append(p[1] if len(p)>1 else '?')
loop=[s for s in samples if any('perf_kernel::Game::play' in x for x in s)]
N=len(loop)
for p in pats:
    n=sum(1 for s in loop if any(p in x for x in s))
    print(f'{100*n/N:6.2f}%  {p}')
