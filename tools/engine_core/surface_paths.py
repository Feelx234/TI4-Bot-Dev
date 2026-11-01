#!/usr/bin/env python3
"""Appendix B of docs/todo/D2-8-wrapper.md as a script: the paths of `ti4_engine` that the production crates use, and which of
them the thin crate (crates/ti4-engine-next) answers natively / still through the legacy facade.

    python3 -I tools/engine_core/surface_paths.py            # the table by module, then the legacy paths
    python3 -I tools/engine_core/surface_paths.py --json     # every path with its status and the crates that use it
    python3 -I tools/engine_core/surface_paths.py --check    # exit 1 on drift from tools/engine_core/surface_paths.baseline.json
    python3 -I tools/engine_core/surface_paths.py --update   # rewrite the baseline (after a path turned native, or a new one was added)

Method (text based, like boundary_inventory.py, whose `use_paths` / `split_prod_test` it reuses): production code (tests cut off) of host,
server, wasm, rule-bot, rule-bot-typed and selfplay; every `ti4_engine::a::b` path of one or more segments below the module. A path is
**native** when the thin crate's `lib.rs` answers it from its own code:
  * the first segment is a module declared as a file module (`pub mod render;`) other than the helper modules `ties`, `queries`, `kinds`; or
  * the first segment is a shell module (`pub mod objectives { ... }`) that names the second segment in an explicit `pub use crate::..::{x, y}` (no
    glob) or defines it inline (`pub mod`, `pub fn`, `pub const`, ...), looking through nested shell modules (`diplomacy::candidates`).
A path that is not native but whose item (second segment) is defined in `crates/ti4-engine-next/src/queries/<module>.rs` (or, for exploration,
objectives and secrets, `queries/content.rs`) is **pending**: the native item exists and is parity-tested, the shell of the module does not export
it yet because its type is also spoken by the legacy `Observed`/`Game` (lane D's swap adds the `pub use`; docs/D28B_RESUME.md).
Everything else is **legacy** (reached through the facade glob `pub use ti4_engine_legacy::*;`).

`--check` also verifies that every name a shell module re-exports from `crate::queries::<file>` / `crate::kinds::<mod>` / `crate::ties::<mod>`
is defined there (a rename in the helper modules cannot silently drop a native item), and fails when
  * a path that is native in the baseline is legacy now,
  * a path is used that the baseline does not list (the table drifted; run --update after reading the list),
  * the legacy count grew.
The baseline records only the status and the module of each path; shrinking it is always allowed (--update).
"""
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import boundary_inventory as bi  # noqa: E402

ROOT = bi.ROOT
THIN = os.path.join(ROOT, 'crates', 'ti4-engine-next', 'src')
GROUPS = ['ti4-host', 'ti4-server', 'ti4-wasm', 'ti4-rule-bot', 'ti4-rule-bot-typed', 'ti4-selfplay']
HELPERS = {'ties', 'queries', 'kinds'}
BASELINE = os.path.join(HERE, 'surface_paths.baseline.json')


def used_paths():
    """{path: sorted crates} of the engine paths below a module, production code only."""
    found = {}
    for g in GROUPS:
        for p in bi.rs_files(os.path.join(bi.CRATES, g)):
            prod, _ = bi.split_prod_test(p)
            if not prod:
                continue
            for u in bi.use_paths(prod):
                if '::' in u:
                    found.setdefault(u, set()).add(g)
    return {k: sorted(v) for k, v in sorted(found.items())}


def split_items(body):
    """Top-level comma separated parts of a `{...}` use body."""
    parts, depth, cur = [], 0, ''
    for c in body:
        if c == ',' and depth == 0:
            parts.append(cur)
            cur = ''
            continue
        depth += (c == '{') - (c == '}')
        cur += c
    parts.append(cur)
    return [p.strip() for p in parts if p.strip()]


def exported_names(block):
    """(names, nested, reexports) of a shell module body: the explicit `pub use` names, the inline definitions, the nested shell modules,
    and `(source file kind, name)` pairs for the helper-module check."""
    names, nested, sources = set(), {}, []
    text = re.sub(r'//[^\n]*', '', block)
    # nested shell modules first (cut out of the text so their names do not leak upward)
    while True:
        m = re.search(r'pub mod (\w+)\s*\{', text)
        if not m:
            break
        depth, j = 1, m.end()
        while j < len(text) and depth:
            depth += (text[j] == '{') - (text[j] == '}')
            j += 1
        nested[m.group(1)] = text[m.end():j - 1]
        names.add(m.group(1))
        text = text[:m.start()] + text[j:]
    for m in re.finditer(r'pub use ([^;]+);', text):
        spec = re.sub(r'\s+', ' ', m.group(1)).strip()
        if spec.endswith('::*'):
            g = re.match(r'crate::(queries|ties)::(\w+)::\*$', spec)
            if g:  # a glob of a helper file: everything it defines publicly
                path = os.path.join(THIN, g.group(1), g.group(2) + '.rs')
                if os.path.exists(path):
                    body = open(path, encoding='utf-8').read()
                    names.update(re.findall(r'^pub (?:const |unsafe )?(?:fn|struct|enum|const|static|type|trait|mod) (\w+)', body, re.M))
            continue

        def leaf(item):
            alias = re.search(r' as (\w+)$', item)
            return alias.group(1) if alias else item.split('::')[-1].strip()

        if '{' in spec:
            head, body = spec.split('{', 1)
            head = head.replace(' ', '')
            for part in split_items(body.rstrip('}').rstrip()):
                names.add(leaf(part))
                sources.append((head + re.sub(r' as \w+$', '', part)).replace(' ', ''))
        else:
            names.add(leaf(spec))
            sources.append(re.sub(r' as \w+$', '', spec).replace(' ', ''))
    for m in re.finditer(r'pub (?:const |unsafe )?(?:fn|struct|enum|const|static|type|trait) (\w+)', text):
        names.add(m.group(1))
    return names, nested, sources


def thin_surface():
    """(file modules, shell module name -> (names, nested, sources))."""
    text = open(os.path.join(THIN, 'lib.rs'), encoding='utf-8').read()
    text = re.sub(r'^\s*//[^\n]*$', '', text, flags=re.M)
    files = set(re.findall(r'^pub mod (\w+);', text, re.M)) - HELPERS
    shells = {}
    for m in re.finditer(r'^pub mod (\w+)\s*\{', text, re.M):
        depth, j = 1, m.end()
        while j < len(text) and depth:
            depth += (text[j] == '{') - (text[j] == '}')
            j += 1
        shells[m.group(1)] = exported_names(text[m.end():j - 1])
    return files, shells


def is_native(path, files, shells):
    segs = path.split('::')
    head = segs[0]
    if head in files:
        return True
    if head not in shells or len(segs) < 2:
        return False
    names, nested, _ = shells[head]
    cur_names, cur_nested = names, nested
    i = 1
    while True:
        if segs[i] not in cur_names:
            return False
        if segs[i] in cur_nested and i + 1 < len(segs):
            cur_names, cur_nested, _ = exported_names(cur_nested[segs[i]])
            i += 1
            continue
        return True


def defined_in_helpers(sources):
    """Problems: a `crate::queries::f::name` / `crate::kinds::m::name` / `crate::ties::m::name` that is not defined in its file."""
    bad = []
    for src in sources:
        m = re.match(r'crate::(queries|kinds|ties)::(\w+)::(\w+)$', src)
        if not m:
            continue
        kind, mod, name = m.groups()
        if kind == 'kinds':
            body = open(os.path.join(THIN, 'kinds.rs'), encoding='utf-8').read()
            mm = re.search(r'pub mod %s\s*\{' % mod, body)
            if not mm:
                bad.append(src)
                continue
            body = body[mm.end():]
        else:
            path = os.path.join(THIN, kind, mod + '.rs')
            if not os.path.exists(path):
                path = os.path.join(THIN, kind, mod, 'mod.rs')
            if not os.path.exists(path):
                bad.append(src)
                continue
            body = open(path, encoding='utf-8').read()
        if not re.search(r'pub (?:const |unsafe )?(?:fn|struct|enum|const|static|type|trait|mod) %s\b|pub use [^;]*\b%s\b' % (name, name), body):
            bad.append(src)
    return bad


QUERY_ALIASES = {'exploration': ['content'], 'objectives': ['content'], 'secrets': ['content']}


def query_definitions():
    """{queries file stem: text}."""
    out = {}
    qdir = os.path.join(THIN, 'queries')
    for f in sorted(os.listdir(qdir)):
        if f.endswith('.rs'):
            out[f[:-3]] = open(os.path.join(qdir, f), encoding='utf-8').read()
    return out


def is_pending(path, defs):
    segs = path.split('::')
    if len(segs) < 2:
        return False
    for stem in [segs[0]] + QUERY_ALIASES.get(segs[0], []):
        body = defs.get(stem)
        if body and re.search(r'pub (?:const |unsafe )?(?:fn|struct|enum|const|static|type|trait|mod) %s\b|pub use [^;]*\b%s\b' % (segs[1], segs[1]), body):
            return True
    return False


def status_of(path, files, shells, defs):
    if is_native(path, files, shells):
        return 'native'
    return 'pending' if is_pending(path, defs) else 'legacy'


RANK = {'legacy': 0, 'pending': 1, 'native': 2}


def classify():
    files, shells = thin_surface()
    defs = query_definitions()
    uses = used_paths()
    rows = {p: {'status': status_of(p, files, shells, defs), 'module': p.split('::')[0], 'crates': c} for p, c in uses.items()}
    problems = []
    for name, (_, nested, sources) in shells.items():
        problems += defined_in_helpers(sources)
        for body in nested.values():
            problems += defined_in_helpers(exported_names(body)[2])
    return rows, problems


def table(rows):
    mods = {}
    for p, r in rows.items():
        m = mods.setdefault(r['module'], {'native': 0, 'pending': 0, 'legacy': 0})
        m[r['status']] += 1
    out = ['module                     paths  native pending  legacy']
    for m, c in sorted(mods.items(), key=lambda kv: (-kv[1]['legacy'], -kv[1]['pending'], kv[0])):
        out.append('%-26s %5d %7d %7d %7d' % (m, sum(c.values()), c['native'], c['pending'], c['legacy']))
    tot = {s: sum(1 for r in rows.values() if r['status'] == s) for s in RANK}
    out.append('%-26s %5d %7d %7d %7d' % ('TOTAL', len(rows), tot['native'], tot['pending'], tot['legacy']))
    return '\n'.join(out)


def main(argv):
    rows, problems = classify()
    if '--json' in argv:
        json.dump(rows, sys.stdout, indent=1, sort_keys=True)
        print()
        return 0
    if '--update' in argv:
        with open(BASELINE, 'w', encoding='utf-8') as f:
            json.dump({p: {'status': r['status']} for p, r in rows.items()}, f, indent=1, sort_keys=True)
            f.write('\n')
        print('baseline rewritten: %d paths, %d legacy, %d pending' % (len(rows), sum(1 for r in rows.values() if r['status'] == 'legacy'), sum(1 for r in rows.values() if r['status'] == 'pending')))
        return 0
    print(table(rows))
    if '--check' not in argv:
        for status in ('pending', 'legacy'):
            listed = [p for p, r in rows.items() if r['status'] == status]
            print('\n%s (%d):' % (status, len(listed)))
            for p in listed:
                print('  %-60s %s' % (p, ','.join(c.replace('ti4-', '') for c in rows[p]['crates'])))
        return 0
    fail = 0
    if problems:
        print('\nshell re-exports that name something the helper module does not define:')
        for p in problems:
            print('  ' + p)
        fail = 1
    base = json.load(open(BASELINE, encoding='utf-8')) if os.path.exists(BASELINE) else {}
    regressed = [p for p, b in base.items() if p in rows and RANK[rows[p]['status']] < RANK[b['status']]]
    unknown = [p for p in rows if p not in base]
    better = [p for p, r in rows.items() if p in base and RANK[r['status']] > RANK[base[p]['status']]]
    legacy_now = sum(1 for r in rows.values() if r['status'] == 'legacy')
    legacy_base = sum(1 for b in base.values() if b['status'] == 'legacy')
    if regressed:
        print('\nless native than in the baseline:')
        for p in regressed:
            print('  %s: %s -> %s' % (p, base[p]['status'], rows[p]['status']))
        fail = 1
    if unknown:
        print('\nused but not in the baseline (run --update after reading):')
        for p in unknown:
            print('  %s (%s)' % (p, rows[p]['status']))
        fail = 1
    if base and legacy_now > legacy_base:
        print('\nthe legacy count grew: %d > %d' % (legacy_now, legacy_base))
        fail = 1
    if better:
        print('\nmore native than the baseline (run --update to shrink it): %d paths' % len(better))
    print('\nsurface_paths --check: %s (%d paths, %d legacy, baseline %d legacy)' % ('FAIL' if fail else 'ok', len(rows), legacy_now, legacy_base))
    return fail


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
