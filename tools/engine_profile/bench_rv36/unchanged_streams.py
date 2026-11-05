# Bench stops lane (docs/BENCH_STOPS_RESUME.md): the decision streams of the games that already worked must not change.
# Compares two native verify output dirs (before the fixes, after) per (setup, seed): a game that ended in a victory before must have the same hash after.
# Usage: unchanged_streams.py BEFORE_DIR AFTER_DIR   (each holds bench.json, bench-v5.json, ... as verify_benches.sh writes them)
import json, sys
before, after = sys.argv[1], sys.argv[2]
bad = 0
for n in ['bench', 'bench-v5', 'bench-v4', 'bench-v1', 'bench-v2', 'bench-v3']:
    a = {(g['spec']['setup'], g['spec']['seed']): g for g in json.load(open('%s/%s.json' % (before, n)))['games']}
    b = {(g['spec']['setup'], g['spec']['seed']): g for g in json.load(open('%s/%s.json' % (after, n)))['games']}
    worked = [k for k in a if a[k]['termination'] != 'engine_error']
    changed = [k for k in worked if k not in b or a[k]['hash'] != b[k]['hash']]
    fixed = [k for k in a if a[k]['termination'] == 'engine_error' and k in b and b[k]['termination'] != 'engine_error']
    still = [k for k in b if b[k]['termination'] == 'engine_error']
    print(n, 'worked before', len(worked), 'changed', len(changed), '| engine_error before', len(a) - len(worked), 'fixed', len(fixed), '| engine_error after', len(still))
    bad += len(changed)
    for k in changed[:5]:
        print('  CHANGED', k)
sys.exit(1 if bad else 0)
