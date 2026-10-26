#!/usr/bin/env python3
"""Module dependency graph of one crate: which top-level modules name which (`crate::m`),
the strongly connected components (cycles), fan-in / fan-out, and the biggest functions.

    python3 -I tools/engine_profile/module_graph.py crates/ti4-engine/src [--json out.json]

Test-only code (`#[cfg(test)] mod ...` blocks, `*_tests*` files) is not counted as a dependency.
A rough, text-based analysis: `use crate::x` and `crate::x::` paths, not name resolution.
"""
import json, os, re, sys
from collections import defaultdict

def strip_tests(text):
    # drop `#[cfg(test)]\nmod name { ... }` blocks by brace matching
    out, i = [], 0
    pat = re.compile(r'#\[cfg\(test\)\]\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*\{')
    while True:
        m = pat.search(text, i)
        if not m:
            out.append(text[i:]); break
        out.append(text[i:m.start()])
        depth, j = 1, m.end()
        while j < len(text) and depth:
            c = text[j]
            if c == '{': depth += 1
            elif c == '}': depth -= 1
            j += 1
        i = j
    return ''.join(out)

def module_of(root, path):
    rel = os.path.relpath(path, root)
    head = rel.split(os.sep)[0]
    return head[:-3] if head.endswith('.rs') else head

def functions(text):
    """(name, lines) of every `fn` by brace matching from its body start."""
    res = []
    for m in re.finditer(r'\bfn\s+(\w+)[^;{]*\{', text):
        depth, j = 1, m.end()
        while j < len(text) and depth:
            c = text[j]
            if c == '{': depth += 1
            elif c == '}': depth -= 1
            j += 1
        res.append((m.group(1), text.count('\n', m.start(), j) + 1))
    return res

def main():
    root = sys.argv[1]
    files = []
    for d, _, fs in os.walk(root):
        for f in fs:
            if f.endswith('.rs'):
                files.append(os.path.join(d, f))
    lines = defaultdict(int); test_lines = defaultdict(int)
    edges = defaultdict(lambda: defaultdict(int))
    fns = []
    mods = set()
    for p in files:
        m = module_of(root, p)
        if m in ('lib', 'main'):
            continue
        mods.add(m)
    for p in files:
        m = module_of(root, p)
        text = open(p, encoding='utf-8').read()
        is_test = 'test' in os.path.basename(p) or '_tests' in p
        prod = '' if is_test else strip_tests(text)
        lines[m] += prod.count('\n'); test_lines[m] += text.count('\n') - prod.count('\n')
        for t in re.findall(r'crate::(\w+)', prod):
            if t in mods and t != m:
                edges[m][t] += 1
        # `use crate::{a, b::c}` groups
        for grp in re.findall(r'use crate::\{([^}]*)\}', prod):
            for t in re.findall(r'(?:^|,)\s*(\w+)', grp):
                if t in mods and t != m:
                    edges[m][t] += 1
        for name, n in functions(prod):
            fns.append((n, f'{os.path.relpath(p, root)}::{name}'))
    # Tarjan SCC
    index, low, on, stack, sccs, counter = {}, {}, set(), [], [], [0]
    sys.setrecursionlimit(10000)
    def strong(v):
        index[v] = low[v] = counter[0]; counter[0] += 1; stack.append(v); on.add(v)
        for w in edges[v]:
            if w not in index:
                strong(w); low[v] = min(low[v], low[w])
            elif w in on:
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            comp = []
            while True:
                w = stack.pop(); on.discard(w); comp.append(w)
                if w == v: break
            sccs.append(comp)
    for v in sorted(mods):
        if v not in index:
            strong(v)
    big = sorted((c for c in sccs if len(c) > 1), key=len, reverse=True)
    fan_in = defaultdict(int)
    for a in edges:
        for b in edges[a]:
            fan_in[b] += 1
    report = {
        'modules': len(mods),
        'edges': sum(len(v) for v in edges.values()),
        'sccs': [sorted(c) for c in big],
        'largest_modules': sorted(((lines[m], test_lines[m], m) for m in mods), reverse=True)[:25],
        'fan_out': sorted(((len(edges[m]), m) for m in mods), reverse=True)[:15],
        'fan_in': sorted(((fan_in[m], m) for m in mods), reverse=True)[:15],
        'largest_functions': sorted(fns, reverse=True)[:30],
    }
    print(f"modules {report['modules']}, module->module edges {report['edges']}")
    for c in big:
        print(f"cycle (SCC) of {len(c)} modules: {', '.join(sorted(c)[:60])}{' ...' if len(c) > 60 else ''}")
    print("largest modules (non-test lines, test lines):")
    for n, t, m in report['largest_modules']:
        print(f"  {m:28} {n:7} {t:7}")
    print("fan-out:", ', '.join(f'{m} {n}' for n, m in report['fan_out']))
    print("fan-in:", ', '.join(f'{m} {n}' for n, m in report['fan_in']))
    print("largest functions (lines):")
    for n, f in report['largest_functions']:
        print(f"  {n:6} {f}")
    if '--json' in sys.argv:
        json.dump(report, open(sys.argv[sys.argv.index('--json') + 1], 'w'), indent=1)

main()
