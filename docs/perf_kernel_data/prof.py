import sys, re, subprocess, collections
data = sys.argv[1]
mode = sys.argv[2] if len(sys.argv) > 2 else 'all'
TOP = int(sys.argv[3]) if len(sys.argv) > 3 else 25
out = subprocess.run(['perf','script','-i',data,'-F','ip,sym,dso'],capture_output=True,text=True).stdout
samples=[]; cur=[]
for line in out.split('\n'):
    if not line.strip():
        if cur: samples.append(cur); cur=[]
        continue
    parts=line.strip().split(None,1)
    sym=parts[1] if len(parts)>1 else '?'
    m=re.match(r'^(.*) \(([^()]*)\)$',sym)
    dso=''
    if m: sym,dso=m.group(1),m.group(2)
    if sym=='[unknown]' and 'libc.so' in dso: sym='libc:memcpy/memmove(stripped)'
    elif sym=='[unknown]': sym='[unknown '+dso.split('/')[-1]+']'
    cur.append(sym)
if cur: samples.append(cur)
def norm(s):
    s=s.replace(' (inlined)','')
    # strip generic arg lists after an identifier
    prev=None
    while prev!=s:
        prev=s
        s=re.sub(r'(?<=[A-Za-z0-9_#}])<[^<>]*>','',s)
    return s[:150]
CORE_LAYER = {'ids':'L0','bits':'L0','hex':'L0','galaxy':'L0','movement':'L0','board':'L0','occupancy':'L0','tables':'L0','generated':'L0',
 'containers':'L1','state':'L1','choice':'L1','event':'L1','keys':'L1','unsupported':'L1','query':'L2','kernel':'L3','hooks':'L3','rules':'L4','driver':'L5','seen':'L5'}
def bucket(sym):
    m=re.search(r'ti4_engine_core_factions::([a-z_0-9]+)',sym)
    if m: return 'L6 factions::'+m.group(1)
    m=re.search(r'ti4_engine_core::([a-z_0-9]+)(?:::([a-z_0-9]+))?',sym)
    if m:
        l=CORE_LAYER.get(m.group(1),'L?')
        sub=m.group(2) or ''
        if m.group(1) in ('kernel','rules','driver','query','state','containers') and sub: return f'{l} {m.group(1)}::{sub}'
        return f'{l} {m.group(1)}'
    m=re.search(r'ti4_engine::([a-z_0-9]+)(?:::([a-z_0-9]+))?',sym)
    if m: return f'ADAPTER ti4_engine::{m.group(1)}'+(f'::{m.group(2)}' if m.group(2) and m.group(1) in ('live','ties','render','core_sync') else '')
    if 'perf_kernel::' in sym: return 'BENCH perf_kernel'
    m=re.search(r'ti4_([a-z_0-9]+)::',sym)
    if m: return 'OTHER ti4_'+m.group(1)
    return None
# keep samples in the game loop
loop=[s for s in samples if any('perf_kernel::Game::play' in x for x in s)]
setup=[s for s in samples if s not in loop] if False else None
N=len(loop)
print(f'{data}: {len(samples)} samples, {N} in the game loop (play)')
selfc=collections.Counter(); incl=collections.Counter(); lay=collections.Counter(); leafkind=collections.Counter()
ALLOC=re.compile(r'^(mi_|_mi_|_?mi\.|__rust_alloc|__rust_dealloc|__rust_realloc|__rdl_|alloc::alloc|malloc|free|realloc|ti4_perf_alloc)')
MEM=re.compile(r'^(__memcpy|__memmove|__memset|memcpy|memmove|memset|_?_?libc_|__GI_|__)')
for s in loop:
    ns=[norm(x) for x in s]
    owner=next((x for x in ns if x.startswith('ti4_') or x.startswith('<ti4_')),'-')
    selfc[ns[0]+'   <-  '+owner]+=1
    for x in set(ns): incl[x]+=1
    leaf=ns[0]
    if ALLOC.match(leaf) or 'mimalloc' in leaf or 'ti4_perf_alloc' in leaf: leafkind['allocator (malloc/free/realloc)']+=1
    elif MEM.match(leaf) or leaf.startswith('libc:'): leafkind['memcpy/memmove/memset']+=1
    else: leafkind['other']+=1
    b=None
    for x in ns:
        b=bucket(x)
        if b: break
    lay[b or 'unattributed (std/libc only)']+=1
def show(c,title,n=TOP):
    print(f'\n== {title}')
    for k,v in c.most_common(n): print(f'{100*v/N:6.2f}%  {k}')
if mode in ('all','self'): show(selfc,'self (leaf, inline-aware)')
if mode in ('all','incl'): show(incl,'inclusive',TOP*2)
if mode in ('all','layer'):
    show(lay,'innermost project frame (time budget by layer/area)',60)
    show(leafkind,'leaf kind')

if mode in ('all','demand'):
    # demand view: the nearest project frame that is NOT a L0/L1/container helper and not an std/alloc frame
    LOW=re.compile(r'ti4_engine_core::(state|containers|ids|bits|board|keys|choice|hex|galaxy|generated|movement)::|ti4_engine_core::(state|containers)::')
    dem=collections.Counter(); dcallee=collections.defaultdict(collections.Counter)
    for s in loop:
        ns=[norm(x) for x in s]
        d=next((x for x in ns if re.match(r'^<?ti4_',x) and not LOW.search(x) and 'perf_alloc' not in x),None)
        dem[d or 'unattributed']+=1
        dcallee[d or 'unattributed'][ns[0]]+=1
    show(dem,'demand view: nearest non-helper project function (self + helpers it calls)',TOP*2)
    print('\n== for the top demanders: their hottest leaf frames')
    for k,_ in dem.most_common(10):
        print(k)
        for l,v in dcallee[k].most_common(3): print(f'      {100*v/N:5.2f}%  {l}')
