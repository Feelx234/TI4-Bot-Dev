#!/usr/bin/env python3
"""Delete loud-gap rows by their string id and renumber (docs/G_SILENT_RESUME.md, docs/G2_RESUME.md).

    tools/engine_core/delete_gap_rows.py 'leader:*:solcommander:GROUND_COMBAT_STARTED:after' ...

Rewrites, by id (so the order of the rows does not matter and a merge of two lanes' deletions can be redone):
  * `silent/gaps.rs`: the `Gap { .. }` lines and the `SUFFI_AN*` constants (the index of their ids),
  * `ability_gaps.rs`: `GAP_IDS`,
  * `ability_audit.rs`: the `ACCOUNTED` lines of the deleted ids go, every other `Status::Gap(i)` is renumbered.
"""
import os
import re
import sys

root = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'crates') + os.sep
G = root + 'ti4-engine-core-factions/src/silent/gaps.rs'
N = root + 'ti4-engine/src/ability_gaps.rs'
A = root + 'ti4-engine/src/ability_audit.rs'

ids = sys.argv[1:]
if not ids:
    sys.exit(__doc__)
names = open(N).read()
m = re.search(r'GAP_IDS: &\[&str\] = &\[\n(.*?)\];', names, re.S)
old_ids = re.findall(r'"([^"]+)"', m.group(1))
for i in ids:
    if i not in old_ids:
        sys.exit(f'not a row: {i}')
dead = {old_ids.index(i) for i in ids}
keep = [i for k, i in enumerate(old_ids) if k not in dead]
new_index = {i: k for k, i in enumerate(keep)}
names = names[:m.start(1)] + ''.join(f'    "{i}",\n' for i in keep) + names[m.end(1):]
open(N, 'w').write(names)

lines = open(G).read().split('\n')
out, k = [], 0
for ln in lines:
    if ln.startswith('    Gap {'):
        if k not in dead:
            out.append(ln)
        k += 1
    else:
        out.append(ln)
assert k == len(old_ids), (k, len(old_ids))
g = '\n'.join(out)
for const, row in (('SUFFI_AN', 'leader:*:mentakagent:PILLAGE_USED:after'), ('SUFFI_AN_COPY', 'leader:*:a:yssarilagent:mentakagent:PILLAGE_USED:after')):
    if row not in new_index or f'pub const {const}:' not in g:
        continue  # lane G3 ported Suffi An: the constants and the rows are gone
    g = re.sub(rf'pub const {const}: usize = \d+;', f'pub const {const}: usize = {new_index[row]};', g)
open(G, 'w').write(g)

out = []
for ln in open(A).read().split('\n'):
    mm = re.match(r'(\s*)\("([^"]+)", Status::Gap\((\d+)\)\),', ln)
    if mm:
        if mm.group(2) in ids:
            continue
        ln = f'{mm.group(1)}("{mm.group(2)}", Status::Gap({new_index[mm.group(2)]})),'
    out.append(ln)
open(A, 'w').write('\n'.join(out))
print(f'deleted {len(dead)} rows; {len(keep)} left')
