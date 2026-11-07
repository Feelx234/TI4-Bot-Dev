#!/usr/bin/env python3
"""Layer check of docs/ENGINE_CORE_STATE.md 4.1/4.2.

    python3 -I tools/engine_core/check_layers.py core [ROOT]     the gate (exit 1 on a violation)
    python3 -I tools/engine_core/check_layers.py chance [CRATE]  the bottom crate ti4-chance (exit 1 on a violation)
    python3 -I tools/engine_core/check_layers.py resolver [CRATE]  the chance resolver ti4-engine-chance (exit 1 on a violation)
    python3 -I tools/engine_core/check_layers.py search [CRATE]  the search API ti4-engine-search (exit 1 on a violation)
    python3 -I tools/engine_core/check_layers.py engine [ROOT]   the report on ti4-engine/src (4.2 table)

core:   every module of `crates/ti4-engine-core/src` (and `ti4-engine-core-factions`, when it exists)
        belongs to a layer; a module may import only from LOWER layers (or its own subtree), and the
        files of `rules/` (L4 areas) may not import each other: they talk through `Next`.
        L3 (`kernel`) never imports L6. Test code (`#[cfg(test)] mod` blocks, `tests/`) is ignored.
        No-draw rule (docs/FORK_RNG_THEORY.md 4.2, lane C3): the core and the factions never draw. Every random event is a chance
        question (`Tape::chance`); no `rng_mut()`, no stream draw (`.die(` / `.shuffle(` / `.index(` on the streams), no
        `answer` / `answer_mut` / `try_answer_mut`, no `rand` / `ChaCha` / `random_range` in their non-test code.
setup:  (docs/SETUP_NATIVE_DESIGN.md) `crates/ti4-engine-core-setup/src` is layer L7: it may use every layer of the core and the
        factions crate (L6), and it must name neither the thin crate `ti4_engine` nor the legacy crate nor the host. Exit 1 on a violation.
thin:   (D2-9 lane 9a) `crates/ti4-engine-next/src` may name `ti4_engine_legacy` only inside the module `lend_string`
        (file `lend_string.rs` or directory `lend_string/`), and `lib.rs` must declare it as
        `#[cfg(feature = "lend-string")] pub mod lend_string;`. Exit 1 on a violation (today: red, the facade glue
        is still there; the gate of lane 9b/9d).
chance: (docs/FORK_RNG_THEORY.md 4.2, lane C1) `crates/ti4-chance` is the bottom crate of the randomness: its Cargo.toml
        names no `ti4-` crate outside `[dev-dependencies]`, its sources (test code ignored) name no `ti4_` crate, and they use
        no clock, thread or OS entropy (wasm) and no unsafe. Exit 1 on a violation.
resolver: (4.2, lane C3) `crates/ti4-engine-chance`, the chance resolver over the core: its Cargo.toml names no `ti4-` crate but
        `ti4-engine-core` and `ti4-chance` outside `[dev-dependencies]`, its sources (test code ignored) name no other `ti4_` crate,
        and they use no clock, thread or OS entropy and no unsafe (it is linked into the browser build).
search: (lane C6, docs/C6_RESUME.md) `crates/ti4-engine-search`, the search API: its Cargo.toml names no legacy, host, server, browser or
        bridge crate outside `[dev-dependencies]`, its sources (test code ignored) name none of them, and they never read or draw the game's
        streams: no `.rng` but `.rng.positions()`, no `rng_mut`, no `answer` / `answer_mut` of ti4-chance, no `advance_resolved` / `settle*`;
        the play-mode resolver (`advance_observed`) only in `position.rs`; no OS entropy, no unsafe.
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
    'containers': 1, 'state': 1, 'choice': 1, 'event': 1, 'keys': 1, 'unsupported': 1,
    # L2 queries
    'query': 2,
    # L3 kernel
    'kernel': 3, 'hooks': 3,
    # L4 rule areas
    'rules': 4,
    # L5 driver
    'driver': 5,
    'seen': 5,
}
FACTIONS_LAYER = 6
# Files that sit above their directory's layer: the movement rules builder is a query by shape (a pure
# read of `&CoreState`) but reads the hook table, so it is an L3 module (package A2).
FILE_LAYER = {('query', 'reach'): 3, ('query', 'transit'): 3}

# --- the string engine's tiers (4.2) ---
TIERS = {
    'L1': 'rng dice deck event trigger fleet_strength neutral_units coexistence decision_context observation generated',
    'L2': 'planets payment synergy valuation tokens supply fleet seating laws movement transit map_feasibility starting_tech preview reach_query space_stations agenda breakthroughs entropic_scars',
    'L3': 'timing reactions reaction_preview registry infantry_ii magen_defense faction_abilities faction_techs',
    'L4a': 'technology exploration relics promissory objectives secrets transactions diplomacy legendary thunders_edge fracture',
    'L4b': 'production combat invasion tactical action_cards leaders strategy_cards strategy secondary_preview agenda_effects vote status draft setup opening',
    'L5': 'game phase wiring',
    'L6': 'factions',
    'adapter': 'choice core_adapter core_sync render fingerprint fixtures dual',
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


# The no-draw rule of lane C3: what may not appear in the non-test code of the core and the factions.
NO_DRAW = (
    r'\brng_mut\s*\(',
    r'\brng\s*\(\s*\)\s*\.\s*(die|shuffle|index)\s*\(',
    r'\.rng\s*\.\s*(die|shuffle|index|set_positions)\s*\(',
    r'\b(try_)?answer_mut\b',
    r'\bti4_chance\s*::\s*answer\b',
    r'\bchance\s*::\s*answer\b',
    r'\brandom_range\b',
    r'\bChaCha\w*',
    r'\brand(_core|_chacha)?\s*::',
)


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
                layer = FILE_LAYER.get((head, sub), CORE_LAYER.get(head))
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
            for pat in NO_DRAW:
                for line in text.split('\n'):
                    code = line.split('//')[0]
                    m = re.search(pat, code)
                    if m:
                        bad.append(f'{os.path.relpath(path, REPO)}: `{m.group(0)}`: the core draws nothing (lane C3: a random event is a chance question, Tape::chance)')
    return bad, units


SETUP_FORBIDDEN = (r'\bti4_engine\b', r'ti4_engine_legacy', r'ti4_host', r'ti4_engine_core_bridge', r'ti4_server', r'ti4_wasm', r'ti4_rule_bot', r'ti4_selfplay')


def check_setup(src):
    """The setup crate (L7): imports only downwards. Test code is ignored. Also checks Cargo.toml for the forbidden dependencies."""
    bad, units = [], 0
    for path in rs_files(src):
        units += 1
        text = strip_tests(open(path, encoding='utf-8').read())
        rel = os.path.relpath(path, REPO)
        for pat in SETUP_FORBIDDEN:
            if re.search(pat, text):
                bad.append(f'{rel}: names {pat.strip(chr(92) + "b")}, which the setup crate (L7) must not depend on')
        for t, _ in targets(text, 'ti4_engine_core'):
            if t[:1].islower() and t not in CORE_LAYER and t not in ('self', 'super', 'hooks'):   # CamelCase names are the root re-exports
                bad.append(f'{rel}: ti4_engine_core::{t} is no module of the core')
    cargo = os.path.join(os.path.dirname(src), 'Cargo.toml')
    deps = open(cargo, encoding='utf-8').read().split('[dev-dependencies]')[0]
    for name in ('ti4-engine ', 'ti4-engine=', 'ti4-engine.', 'ti4-engine-legacy', 'ti4-host', 'ti4-engine-core-bridge', 'ti4-server', 'ti4-wasm'):
        if re.search(r'^' + re.escape(name), deps, re.M):
            bad.append(f'{os.path.relpath(cargo, REPO)}: depends on {name.strip(" =.")}')
    return bad, units


CHANCE_FORBIDDEN = (r'\bti4_\w+', r'std::time', r'std::thread', r'\bgetrandom\b', r'thread_rng', r'rand::rng\(', r'from_os_rng', r'from_entropy', r'\bunsafe\b')


def check_chance(crate):
    """The bottom crate `ti4-chance`: no `ti4-` dependency (dev-dependencies excepted), nothing that breaks the wasm build."""
    import tomllib
    bad, units = [], 0
    cargo = os.path.join(crate, 'Cargo.toml')
    t = tomllib.load(open(cargo, 'rb'))
    sections = [('dependencies', t.get('dependencies', {})), ('build-dependencies', t.get('build-dependencies', {}))]
    for target, tt in t.get('target', {}).items():
        for k in ('dependencies', 'build-dependencies'):
            sections.append((f'target.{target}.{k}', tt.get(k, {})))
    for where, deps in sections:
        for name, spec in deps.items():
            real = spec.get('package', name) if isinstance(spec, dict) else name
            if real.startswith('ti4-') or real.startswith('ti4_'):
                bad.append(f'{os.path.relpath(cargo, REPO)}: [{where}] names {real}; the bottom crate depends on no ti4- crate')
    for path in rs_files(os.path.join(crate, 'src')):
        units += 1
        text = strip_tests(open(path, encoding='utf-8').read())
        for pat in CHANCE_FORBIDDEN:
            m = re.search(pat, text)
            if m:
                bad.append(f'{os.path.relpath(path, REPO)}: uses `{m.group(0)}`, which the bottom crate must not (no ti4_ crate; wasm: no clock, threads, entropy; no unsafe)')
    return bad, units


RESOLVER_DEPS = ('ti4-engine-core', 'ti4-chance')
RESOLVER_FORBIDDEN = (r'std::time', r'std::thread', r'\bgetrandom\b', r'thread_rng', r'rand::rng\(', r'from_os_rng', r'from_entropy', r'\bunsafe\b')


def check_resolver(crate):
    """The chance resolver `ti4-engine-chance`: over the core and the bottom crate only, nothing that breaks the wasm build."""
    import tomllib
    bad, units = [], 0
    cargo = os.path.join(crate, 'Cargo.toml')
    t = tomllib.load(open(cargo, 'rb'))
    sections = [('dependencies', t.get('dependencies', {})), ('build-dependencies', t.get('build-dependencies', {}))]
    for target, tt in t.get('target', {}).items():
        for k in ('dependencies', 'build-dependencies'):
            sections.append((f'target.{target}.{k}', tt.get(k, {})))
    for where, deps in sections:
        for name, spec in deps.items():
            real = spec.get('package', name) if isinstance(spec, dict) else name
            if (real.startswith('ti4-') or real.startswith('ti4_')) and real not in RESOLVER_DEPS:
                bad.append(f'{os.path.relpath(cargo, REPO)}: [{where}] names {real}; the resolver depends on ti4-engine-core and ti4-chance only')
    for path in rs_files(os.path.join(crate, 'src')):
        units += 1
        text = strip_tests(open(path, encoding='utf-8').read())
        for m in re.finditer(r'\bti4_\w+', text):
            if m.group(0) not in ('ti4_engine_core', 'ti4_chance'):
                bad.append(f'{os.path.relpath(path, REPO)}: names `{m.group(0)}`; the resolver names ti4_engine_core and ti4_chance only')
        for pat in RESOLVER_FORBIDDEN:
            m = re.search(pat, text)
            if m:
                bad.append(f'{os.path.relpath(path, REPO)}: uses `{m.group(0)}`, which the resolver must not (wasm: no clock, threads, entropy; no unsafe)')
    return bad, units


SEARCH_DEPS_FORBIDDEN = ('ti4-engine-legacy', 'ti4-host', 'ti4-server', 'ti4-wasm', 'ti4-engine-core-bridge', 'ti4-engine-core-tests')
SEARCH_NAMES_FORBIDDEN = ('ti4_engine_legacy', 'ti4_host', 'ti4_server', 'ti4_wasm', 'ti4_engine_core_bridge')
# The game's streams: a search position never reads or draws them (docs/FORK_RNG_THEORY.md 4.4, D2). `.rng.positions()` (how far each stream
# was drawn, no key) is the one read allowed; the play-mode resolver is `ti4_engine_chance::advance_observed` in `position.rs` alone.
SEARCH_FORBIDDEN = (r'rng_mut\(', r'\banswer_mut\b', r'\btry_answer_mut\b', r'ti4_chance::answer\b', r'ti4_chance::\{[^}]*\banswer\b',
                    r'\.rng\b(?!\.positions\(\))', r'thread_rng', r'rand::rng\(', r'from_os_rng', r'from_entropy', r'\bgetrandom\b',
                    r'\bunsafe\b', r'advance_resolved', r'settle_from', r'\bsettle\(')


def check_search(crate):
    """The search API `ti4-engine-search` (lane C6): no product host, server, browser or legacy crate; never the game's streams."""
    import tomllib
    bad, units = [], 0
    cargo = os.path.join(crate, 'Cargo.toml')
    t = tomllib.load(open(cargo, 'rb'))
    for where in ('dependencies', 'build-dependencies'):
        for name, spec in t.get(where, {}).items():
            real = spec.get('package', name) if isinstance(spec, dict) else name
            if real in SEARCH_DEPS_FORBIDDEN:
                bad.append(f'{os.path.relpath(cargo, REPO)}: [{where}] names {real}; the search API depends on the core, the resolver and the thin crate only')
    for path in rs_files(os.path.join(crate, 'src')):
        units += 1
        if os.path.basename(path) == 'tests.rs':
            continue   # the crate's test module (declared under #[cfg(test)]): it reads the streams to check that nothing else does
        rel = os.path.relpath(path, REPO)
        text = strip_tests(open(path, encoding='utf-8').read())
        for name in SEARCH_NAMES_FORBIDDEN:
            if re.search(r'\b' + name + r'\b', text):
                bad.append(f'{rel}: names `{name}`, which the search API must not')
        for pat in SEARCH_FORBIDDEN:
            for m in re.finditer(pat, text):
                bad.append(f'{rel}: uses `{m.group(0)}`: a search position never reads or draws the game\'s streams')
        if os.path.basename(path) != 'position.rs' and re.search(r'advance_observed|advance_in\b', text):
            bad.append(f'{rel}: calls the play-mode resolver outside position.rs')
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


def check_thin(src):
    bad, units = [], 0
    for path in rs_files(src):
        rel = os.path.relpath(path, src)
        units += 1
        if rel == 'lend_string.rs' or rel.startswith('lend_string' + os.sep) or os.path.basename(rel).startswith('tests'):
            continue   # the gated module; test-only files (`tests*.rs`, declared under #[cfg(test)])
        text = strip_tests(open(path, encoding='utf-8').read())
        n = len(re.findall(r'ti4_engine_legacy|ti4-engine-legacy', text))
        if n:
            bad.append(f'{rel}: {n} reference(s) to ti4_engine_legacy outside the cfg(feature = "lend-string") module lend_string')
    lib = os.path.join(src, 'lib.rs')
    if os.path.exists(os.path.join(src, 'lend_string.rs')) or os.path.isdir(os.path.join(src, 'lend_string')):
        t = open(lib, encoding='utf-8').read()
        if not re.search(r'#\[cfg\(feature\s*=\s*"lend-string"\)\]\s*(pub(\([^)]*\))?\s+)?mod\s+lend_string\b', t):
            bad.append('lib.rs: module lend_string is not declared under #[cfg(feature = "lend-string")]')
    return bad, units


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
    elif mode == 'setup':
        root = sys.argv[2] if len(sys.argv) > 2 else os.path.join(REPO, 'crates/ti4-engine-core-setup/src')
        bad, units = check_setup(root)
        for b in bad:
            print('SETUP', b)
        print(f'check_layers setup: {units} files checked, {len(bad)} violations')
        sys.exit(1 if bad else 0)
    elif mode == 'thin':
        root = sys.argv[2] if len(sys.argv) > 2 else os.path.join(REPO, 'crates/ti4-engine-next/src')
        bad, units = check_thin(root)
        for b in bad:
            print('LEGACY', b)
        print(f'check_layers thin: {units} files checked, {len(bad)} violations')
        sys.exit(1 if bad else 0)
    elif mode == 'chance':
        root = sys.argv[2] if len(sys.argv) > 2 else os.path.join(REPO, 'crates/ti4-chance')
        bad, units = check_chance(root)
        for b in bad:
            print('CHANCE', b)
        print(f'check_layers chance: {units} files checked, {len(bad)} violations')
        sys.exit(1 if bad else 0)
    elif mode == 'resolver':
        root = sys.argv[2] if len(sys.argv) > 2 else os.path.join(REPO, 'crates/ti4-engine-chance')
        bad, units = check_resolver(root)
        for b in bad:
            print('RESOLVER', b)
        print(f'check_layers resolver: {units} files checked, {len(bad)} violations')
        sys.exit(1 if bad else 0)
    elif mode == 'search':
        root = sys.argv[2] if len(sys.argv) > 2 else os.path.join(REPO, 'crates/ti4-engine-search')
        bad, units = check_search(root)
        for b in bad:
            print('SEARCH', b)
        print(f'check_layers search: {units} files checked, {len(bad)} violations')
        sys.exit(1 if bad else 0)
    elif mode == 'engine':
        check_engine(sys.argv[2] if len(sys.argv) > 2 else os.path.join(REPO, 'crates/ti4-engine/src'))
    else:
        print(__doc__); sys.exit(2)


main()
