import re,collections,subprocess,sys
data=sys.argv[1]
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
    return s[:110]
recs=[r for r in recs if any('Game::play' in x for x in r[1])]
for want in ['ti4_engine::live::env::env_for','ti4_engine_core::kernel::ability::AbilityTable::registry','ti4_engine_core::containers::log::Log::push','ti4_engine_core::driver::live::table_at','-']:
    c=collections.Counter(); n=collections.Counter()
    for size,st in recs:
        ns=[norm(x) for x in st]
        owner=next((x for x in ns if x.startswith('ti4_') or x.startswith('<ti4_')),'-')
        if owner!=want: continue
        ch=[x for x in ns if not (x.startswith('mi_') or 'ti4_perf_alloc' in x or '__rust' in x or x.startswith('alloc::alloc') or x.startswith('alloc::raw'))]
        key=' < '.join(ch[:9])
        c[key]+=size; n[key]+=1
    print('###',want,flush=True)
    for k,v in c.most_common(4): print(f'  {v/1e6:7.2f} MB {n[k]:>7}  {k[:600]}',flush=True)
