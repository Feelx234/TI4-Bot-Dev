# Bench lane rv36 (docs/BENCH_RV36_RESUME.md). Compares the string recording at 35 (base35/) with the native verify output (ver36-out/) by (setup, seed), ignoring the rules version in the spec.
# Paths are relative to a work dir that holds base35/ and ver36-out/ (the lane used /root/deploytmp/rv36).
import json,sys
names={'bench':'baseline.json','bench-v5':'baseline-v5.json','bench-v4':'baseline-v4.json','bench-v1':'baseline-v1.json','bench-v2':'baseline-v2.json','bench-v3':'baseline-v3.json'}
out={}
for n,f in names.items():
    a={(g['spec']['setup'],g['spec']['seed']):g for g in json.load(open('base35/'+f))['games']}
    b={(g['spec']['setup'],g['spec']['seed']):g for g in json.load(open('ver36-out/%s.json'%n))['games']}
    same=[k for k in a if k in b and a[k]['hash']==b[k]['hash']]
    diff=[k for k in a if k in b and a[k]['hash']!=b[k]['hash']]
    print(n,len(a),len(b),'same',len(same),'diff',len(diff), 'term35',{t:sum(1 for g in a.values() if g['termination']==t) for t in set(g['termination'] for g in a.values())}, 'term36',{t:sum(1 for g in b.values() if g['termination']==t) for t in set(g['termination'] for g in b.values())})
    out[n]=[(k[0],k[1]) for k in sorted(diff)]
json.dump(out,open('diff.json','w'))
