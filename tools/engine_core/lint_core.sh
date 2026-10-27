#!/usr/bin/env bash
# Core hygiene (docs/ENGINE_CORE_STATE.md 4.1, gate 9): no String/&str/serde/BTreeMap/BTreeSet/
# format!/unsafe in the NON-TEST, non-comment code of the core crates.
#
#   tools/engine_core/lint_core.sh [ROOT...]     default: the core crates that exist
#
# Test code (#[cfg(test)] mod blocks, tests/, benches/, examples/) and comments are ignored.
# `// lint-allow(core): reason` on the line (or the one above) lets a line through; the count of
# allowed lines is printed. Exit 1 on any hit.
set -uo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
roots=("$@")
if [[ ${#roots[@]} == 0 ]]; then
  for c in ti4-engine-core ti4-engine-core-factions; do
    [[ -d "$repo/crates/$c/src" ]] && roots+=("$repo/crates/$c/src")
  done
fi
python3 -I - "${roots[@]}" <<'PY'
import os, re, sys
PAT = re.compile(r'\bString\b|&\s*(?:\'\w+\s+)?str\b|\bserde(?:_json)?\b|\bBTree(?:Map|Set)\b|\bformat!|\bunsafe\b|\.to_string\(\)')
def strip_tests(text):
    out, i = [], 0
    pat = re.compile(r'#\[cfg\(test\)\]\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*\{')
    while True:
        m = pat.search(text, i)
        if not m:
            out.append(text[i:]); break
        out.append(text[i:m.start()] + '\n' * text.count('\n', m.start(), m.end()))
        depth, j = 1, m.end()
        while j < len(text) and depth:
            depth += {'{': 1, '}': -1}.get(text[j], 0); j += 1
        out.append('\n' * text.count('\n', m.end(), j))
        i = j
    return ''.join(out)
hits = allowed = files = 0
for root in sys.argv[1:]:
    for d, _, fs in os.walk(root):
        for f in sorted(fs):
            if not f.endswith('.rs'):
                continue
            p = os.path.join(d, f)
            raw = open(p, encoding='utf-8').read()
            lines = strip_tests(raw).split('\n')
            orig = raw.split('\n')
            files += 1
            for n, line in enumerate(lines):
                code = re.sub(r'"(?:[^"\\]|\\.)*"', '""', line.split('//')[0])
                if not PAT.search(code):
                    continue
                near = orig[n] + (orig[n - 1] if n else '')
                if 'lint-allow(core)' in near:
                    allowed += 1; continue
                hits += 1
                print(f'{p}:{n + 1}: {line.strip()}')
print(f'lint_core: {files} files, {hits} hits, {allowed} allowed')
sys.exit(1 if hits else 0)
PY
