import sys, re, subprocess, collections
data=sys.argv[1]; TOP=int(sys.argv[2]) if len(sys.argv)>2 else 30
out=subprocess.run(['perf','script','-i',data,'-F','event,trace,ip,sym','--no-inline'],capture_output=True,text=True).stdout
# inline frames help naming; use inline-aware output
out=subprocess.run(['perf','script','-i',data,'-F','event,trace,ip,sym'],capture_output=True,text=True).stdout
recs=[]; cur=None
for line in out.split('\n'):
    if line.startswith('probe_'):
        m=re.search(r'size=(0x[0-9a-f]+)',line)
        cur=[int(m.group(1),16) if m else 0,[]]; recs.append(cur)
    elif line.strip() and cur is not None:
        p=line.strip().split(None,1)
        cur[1].append(p[1] if len(p)>1 else '?')
def norm(s):
    s=s.replace(' (inlined)','')
    prev=None
    while prev!=s:
        prev=s; s=re.sub(r'(?<=[A-Za-z0-9_#}])<[^<>]*>','',s)
    return s[:140]
LOW=re.compile(r'ti4_engine_core::(state|containers|ids|bits|board|keys|choice)::|ti4_engine_core::state::|ti4_engine_core::containers::')
tot_b=sum(r[0] for r in recs); tot_n=len(recs)
print(f'{tot_n} allocation events, {tot_b/1e6:.1f} MB')
by_owner=collections.Counter(); n_owner=collections.Counter()
by_demand=collections.Counter(); n_demand=collections.Counter()
by_kind=collections.Counter(); by_size=collections.Counter(); by_phase=collections.Counter(); nphase=collections.Counter()
PH=[('env_with_plays/env_for (adapter)','live::env::env_with'),('env_with_plays/env_for (adapter)','live::env::env_for'),('Env::clone per live call','Env as core::clone::Clone'),('core driver::live::advance','driver::live::advance'),('core driver::live::ask','driver::live::ask'),('texts/sync_codes','combat_texts'),('texts/sync_codes','sync_codes')]
recs=[r for r in recs if any('Game::play' in x for x in r[1])]
tot_b=sum(r[0] for r in recs); tot_n=len(recs)
print('(only allocations inside Game::play)')
sub=collections.defaultdict(collections.Counter)
for size,st in recs:
    ns=[norm(x) for x in st]
    owner=next((x for x in ns if x.startswith('ti4_') or x.startswith('<ti4_')),'-')
    demand=next((x for x in ns if (x.startswith('ti4_') or x.startswith('<ti4_')) and not LOW.search(x)),owner)
    by_owner[owner]+=size; n_owner[owner]+=1
    chain=[x for x in ns if not (x.startswith('mi_') or 'ti4_perf_alloc' in x or '__rust' in x or x.startswith('alloc::alloc'))][:3]
    sub[owner][' < '.join(chain)]+=size
    by_demand[demand]+=size; n_demand[demand]+=1
    kind=next((x for x in ns[1:] if not x.startswith('mi_') and 'alloc::alloc' not in x and 'ti4_perf_alloc' not in x and '__rust' not in x and not x.startswith('alloc::raw_vec') or False),'-')
    by_kind[kind]+=size
    bucket=next((i for i in (1<<k for k in range(0,30)) if size<=i),1<<30)
    by_size[bucket]+=size
    ph='other'
    full=' '.join(ns)
    for name,key in PH:
        if key in full: ph=name; break
    by_phase[ph]+=size; nphase[ph]+=1
def show(c,n,title,N=None):
    print(f'\n== {title}')
    for k,v in c.most_common(TOP):
        print(f'{v/1e6:8.2f} MB {100*v/tot_b:5.1f}%  {n[k] if n else "":>8}  {k}')
show(by_phase,nphase,'by phase (MB, share, count)')
show(by_owner,n_owner,'by nearest project frame (direct allocator of the bytes)')
show(by_demand,n_demand,'by nearest rule-level project frame (skips state/containers helpers)')
print('\n== by size class (<= bytes): MB'); 
for k in sorted(by_size): print(f'{k:>10} {by_size[k]/1e6:8.2f}')

print('\n== inside the top owners: the three frames right above the allocator')
for o,_ in by_owner.most_common(8):
    print(o)
    for k,v in sub[o].most_common(5): print(f'     {v/1e6:7.2f} MB  {k}')
