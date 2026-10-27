#!/usr/bin/env python3
"""Layer check of docs/ENGINE_CORE_STATE.md 4.1/4.2.

    python3 -I tools/engine_core/check_layers.py core [ROOT]     the gate (exit 1 on a violation)
    python3 -I tools/engine_core/check_layers.py engine [ROOT]   the report on ti4-engine/src (4.2 table)

core:   every module of `crates/ti4-engine-core/src` (and `ti4-engine-core-factions`, when it exists)
        belongs to a layer; a module may import only from LOWER layers (or its own subtree), and the
        files of `rules/` (L4 areas) may not import each other: they talk through `Next`.
        L3 (`kernel`) never imports L6. Test code (`#[cfg(test)] mod` blocks, `tests/`) is ignored.
engine: the tier table of 4.2 over the string engine; counts the module references that point up or
        sideways inside L4 (the number the port brings to zero). Reporting only, exit 0.
"""
import os, re, sys
from collections import defaultdict

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))

# --- core layers (4.1/4.3) ---
CORE_LAYER = {
    # L0 data
    'ids': 0, 'bits': 0, 'hex': 0, 'galaxy': 0, 'movement': 0, 'board': 0, 'occupancy': 0,
    'tables': 0, 'generated': 0,
    # L1 state + decision types
    'containers': 1, 'state': 1, 'choice': 1, 'event': 1,
    # L2 queries
    'query': 2,
    # L3 kernel
    'kernel': 3, 'hooks': 3,
    # L4 rule areas
    'rules': 4,
    # L5 driver
    'driver': 5,
}
FACTIONS_LAYER = 6

# --- the string engine's tiers (4.2) ---
TIERS = {
    'L1': 'rng dice deck event trigger fleet_strength neutral_units coexistence decision_context observation generated',
    'L2': 'planets payment synergy valuation tokens supply fleet seating laws movement transit map_feasibility starting_tech preview reach_query space_stations agenda breakthroughs entropic_scars',
    'L3': 'timing reactions reaction_preview registry infantry_ii magen_defense faction_abilities faction_techs',
    'L4a': 'technology exploration relics promissory objectives secrets transactions diplomacy legendary thunders_edge fracture',
    'L4b': 'production combat invasion tactical action_cards leaders strategy_cards strategy secondary_preview agenda_effects vote status draft setup opening',
    'L5': 'game phase wiring',
    'L6': 'factions',
    'adapter': 'choice core_adapter fingerprint fixtures dual',
}
TIER_RANK = {'L1': 1, 'L2': 2, 'L3': 3, 'L4a': 4, 'L4b': 4, 'L5': 5, 'L6': 6}


def strip_tests(text):
    out, i = [], 0
    pat = re.compile(r'#\[cfg\(test\)\]\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*\{')
    while True:
        m = pat.search(text, i)
        if not m:
            out.append(text[i:]); break
        out.append(text[i:m.start()])
        depth, j = 1, m.end()
        while j < len(text) and depth:
            depth += {'{': 1, '}': -1}.get(text[j], 0); j += 1
        i = j
    text = ''.join(out)
    return re.sub(r'//[^\n]*', '', text)


def targets(text, prefix):
    """Top-level module names (and the second path segment) imported through `prefix::`."""
    found = []
    for m in re.finditer(re.escape(prefix) + r'::(\w+)(?:::(\w+))?', text):
        found.append((m.group(1), m.group(2)))
    for grp in re.finditer(re.escape(prefix) + r'::\{([^}]*)\}', text):
        for part in re.findall(r'(?:^|,)\s*(\w+)(?:::(\w+))?', grp.group(1)):
            found.append(part)
    return found


def rs_files(root):
    for d, _, fs in os.walk(root):
        for f in sorted(fs):
            if f.endswith('.rs'):
                yield os.path.join(d, f)


def top_module(root, path):
    rel = os.path.relpath(path, root).split(os.sep)
    head = rel[0][:-3] if rel[0].endswith('.rs') else rel[0]
    sub = None
    if len(rel) > 1:
        sub = rel[1][:-3] if rel[1].endswith('.rs') else rel[1]
    return head, sub


def check_core(core_root, factions_root):
    bad, units = [], 0
    for root, own_crate, floor in ((core_root, 'crate', None), (factions_root, 'ti4_engine_core', FACTIONS_LAYER)):
        if not root or not os.path.isdir(root):
            continue
        for path in rs_files(root):
            head, sub = top_module(root, path)
            if head in ('lib', 'main', 'mod'):
                continue
            if floor is None:
                layer = CORE_LAYER.get(head)
                if layer is None:
                    bad.append(f'{os.path.relpath(path, REPO)}: module `{head}` has no layer (add it to CORE_LAYER)')
                    continue
            else:
                layer = floor
            text = strip_tests(open(path, encoding='utf-8').read())
            units += 1
            pre = 'crate' if floor is None else 'ti4_engine_core'
            for t, t2 in targets(text, pre):
                if floor is None and t in ('self', 'super'):
                    continue
                if t not in CORE_LAYER:
                    continue
                other = CORE_LAYER[t]
                where = f'{os.path.relpath(path, REPO)}: {pre}::{t}' + (f'::{t2}' if t2 else '')
                if other > layer:
                    bad.append(f'{where}: L{layer} imports up into L{other}')
                elif other == layer and layer == 4 and t == 'rules' and head == 'rules':
                    if t2 and sub and t2 != sub:
                        bad.append(f'{where}: L4 area `{sub}` imports L4 area `{t2}` (use Next)')
            if floor is None and head in ('kernel', 'hooks'):
                if re.search(r'ti4_engine_core_factions', text):
                    bad.append(f'{os.path.relpath(path, REPO)}: L3 imports L6')
    return bad, units


def check_engine(root):
    tier = {}
    for name, mods in TIERS.items():
        for m in mods.split():
            tier[m] = name
    edges = defaultdict(int)
    mods = set()
    for path in rs_files(root):
        head, _ = top_module(root, path)
        if head not in ('lib', 'main'):
            mods.add(head)
    for path in rs_files(root):
        head, _ = top_module(root, path)
        if head in ('lib', 'main') or 'test' in os.path.basename(path) or head.endswith('_tests'):
            continue
        text = strip_tests(open(path, encoding='utf-8').read())
        for t, _ in targets(text, 'crate'):
            if t in mods and t != head:
                edges[(head, t)] += 1
    untiered = sorted(m for m in mods if m not in tier)
    pairs = refs = 0
    kinds = defaultdict(lambda: [0, 0])
    for (a, b), n in edges.items():
        ta, tb = tier.get(a), tier.get(b)
        if not ta or not tb or 'adapter' in (ta, tb):
            continue
        ra, rb = TIER_RANK[ta], TIER_RANK[tb]
        if rb > ra or (ra == rb == 4 and (ta == tb or (ta, tb) == ('L4a', 'L4b'))):
            kind = f'{ta}->{tb}'
            kinds[kind][0] += 1; kinds[kind][1] += n
            pairs += 1; refs += n
    total = sum(n for (a, b), n in edges.items() if tier.get(a) not in (None, 'adapter') and tier.get(b) not in (None, 'adapter'))
    print(f'engine: {len(mods)} modules, {total} module references between non-adapter modules')
    print(f'layering violations (up or sideways in L4): {pairs} module pairs / {refs} references')
    for k, (p, r) in sorted(kinds.items(), key=lambda kv: -kv[1][1]):
        print(f'  {k:10} {p:3} pairs {r:4} refs')
    if untiered:
        print('modules without a tier:', ', '.join(untiered))


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else 'core'
    if mode == 'core':
        root = sys.argv[2] if len(sys.argv) > 2 else os.path.join(REPO, 'crates/ti4-engine-core/src')
        fac = os.path.join(REPO, 'crates/ti4-engine-core-factions/src')
        bad, units = check_core(root, fac)
        for b in bad:
            print('LAYER', b)
        print(f'check_layers core: {units} files checked, {len(bad)} violations')
        sys.exit(1 if bad else 0)
    elif mode == 'engine':
        check_engine(sys.argv[2] if len(sys.argv) > 2 else os.path.join(REPO, 'crates/ti4-engine/src'))
    else:
        print(__doc__); sys.exit(2)


main()
