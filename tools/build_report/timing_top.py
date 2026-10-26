#!/usr/bin/env python3
"""Top-N units of a `cargo build --timings` html (the UNIT_DATA array), and the critical path.

usage: timing_top.py cargo-timing.html [N]
"""
import json, re, sys
html = open(sys.argv[1]).read()
m = re.search(r'const UNIT_DATA = (\[.*?\]);\n', html, re.S)
units = json.loads(m.group(1))
n = int(sys.argv[2]) if len(sys.argv) > 2 else 20
total = sum(u['duration'] for u in units)
end = max(u['start'] + u['duration'] for u in units)
print(f"units {len(units)}  summed unit time {total:.0f}s  wall {end:.0f}s")
print(f"{'sec':>7} {'codegen':>8}  unit")
for u in sorted(units, key=lambda u: -u['duration'])[:n]:
    rm = u.get('rmeta_time')
    name = f"{u['name']} {u['version']}" + (f" [{u['target'].strip()}]" if u.get('target', '').strip() else '') + (' (build script)' if 'custom-build' in u.get('mode', '') or u.get('mode') == 'run-custom-build' else '') + (f" mode={u['mode']}" if u.get('mode') not in ('todo', None, '') else '')
    cg = f"{u['duration'] - rm:.0f}" if rm else '-'
    print(f"{u['duration']:7.1f} {cg:>8}  {name}")
