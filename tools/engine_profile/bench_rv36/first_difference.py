# Bench lane rv36 (docs/BENCH_RV36_RESUME.md). Replays every game with the string recorder (35) and the native selfplay (36) in text mode and reports the first differing decision,
# after skipping runs of reaction decisions that are the same set in a different order (S1). Constants S, N and the work dir (base35/, texts2/) are those of the lane (/root/deploytmp/rv36); edit them.
import json,subprocess,sys,re,os,concurrent.futures as cf
S='/root/deploytmp/core-rec-target/gate/selfplay'; N='/root/deploytmp/rv36-target/gate/selfplay'
bots={'bench':[],'bench-v5':['bot_version=5'],'bench-v4':['bot_version=4'],'bench-v1':['bot_version=1'],'bench-v2':['bot_version=2'],'bench-v3':['bot_version=3']}
bf={'bench':'baseline.json','bench-v5':'baseline-v5.json','bench-v4':'baseline-v4.json','bench-v1':'baseline-v1.json','bench-v2':'baseline-v2.json','bench-v3':'baseline-v3.json'}
os.makedirs('texts2',exist_ok=True)
def run(binp,spec,bench,rv,tag):
    fn='texts2/%s-%s-%s-%d.txt'%(bench,tag,spec['setup'],spec['seed'])
    if os.path.exists(fn): return fn
    a=[binp,'replay','--seed',str(spec['seed']),'--players',str(spec['players']),'--map',spec['map'],'--factions',','.join(spec['factions']),'--speaker',spec['speaker'],'--starting-tech','--rules-version',str(rv),'--text']
    for b in bots[bench]: a+=['--bot',b]
    r=subprocess.run(a,capture_output=True,text=True)
    open(fn,'w').write(r.stdout+'\n#STDERR '+r.stderr[-400:]+'\n#RC %d\n'%r.returncode)
    return fn
def tok(l):
    m=re.match(r'\s*\d+ (r\d+)\s+(\w+)\s+(\S)\s+(.*)$',l)
    if not m: return (l.strip(),)
    seat=m.group(3); rest=m.group(4)
    m2=re.match(r'(?:(\w+)\s+)?(rule|random)\s+(.*)$',rest)
    if not m2: return (seat,rest)
    q=m2.group(1) or ''
    return (seat,q,m2.group(2),m2.group(3))
def isreact(t): return len(t)>1 and t[1].startswith('reaction')
def first_diff(A,B):
    A=[tok(l) for l in A]; B=[tok(l) for l in B]; i=0; reord=0
    while i<len(A) and i<len(B):
        if A[i]==B[i] or (len(A[i])==4 and len(B[i])==4 and A[i][1]=='' and (A[i][0],A[i][2],A[i][3])==(B[i][0],B[i][2],B[i][3])): i+=1; continue
        if isreact(A[i]) and isreact(B[i]):
            j=i
            while j<min(len(A),len(B)) and isreact(A[j]) and isreact(B[j]): j+=1
            # extend to the end of reaction runs independently
            ja=i
            while ja<len(A) and isreact(A[ja]): ja+=1
            jb=i
            while jb<len(B) and isreact(B[jb]): jb+=1
            if sorted(A[i:ja])==sorted(B[i:jb]):
                reord+=1; i=ja if ja==jb else i; 
                if ja!=jb: return i,A[i],B[i],reord
                continue
        return i,A[i],B[i],reord
    if len(A)!=len(B): return i,(A[i] if i<len(A) else 'END'),(B[i] if i<len(B) else 'END'),reord
    return None,None,None,reord
def one(job):
    bench,spec=job
    s=run(S,spec,bench,35,'s'); n=run(N,spec,bench,36,'n')
    def rd(f):
        L=open(f).read().split('\n'); body=[l for l in L if re.match(r'\s*\d+ r\d+ ',l)]; tail=[l for l in L if l.startswith('#')]
        return body,tail
    A,ta=rd(s); B,tb=rd(n)
    i,a,b,reord=first_diff(A,B)
    return dict(bench=bench,setup=spec['setup'],seed=spec['seed'],idx=i,a=a,b=b,reord=reord,nA=len(A),nB=len(B),tailn=tb)
if __name__=='__main__':
    jobs=[]
    for b in bf:
        for g in json.load(open('base35/'+bf[b]))['games']:
            jobs.append((b,g['spec']))
    lim=int(sys.argv[1]) if len(sys.argv)>1 else len(jobs)
    # interleave so a limit samples all benches
    jobs=jobs[:lim] if lim<len(jobs) else jobs
    res=[]
    with cf.ThreadPoolExecutor(int(os.environ.get('PAR','7'))) as ex:
        for r in ex.map(one,jobs): res.append(r)
    json.dump(res,open('classify.json' if lim>=1200 else 'classify-part.json','w'))
    print(len(res))
