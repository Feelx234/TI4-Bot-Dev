#!/usr/bin/env python3
"""Boundary inventory of ti4-engine for the typed-core design (docs/ENGINE_CORE.md, section 2).

    python3 -I tools/engine_core/boundary_inventory.py [--json out.json]

Text-based, like tools/engine_profile/module_graph.py (no name resolution):
- the engine's public surface: `pub` items per top-level module (free fns, types, consts, methods);
- per consumer crate, the engine items it names (`use ti4_engine::...` with nested braces, inline
  `ti4_engine::a::b` paths), split production / test code;
- calls of the public methods of the boundary types (Game, Observed/SeatObservation, Table,
  ChoiceView-like) by name, an upper bound for generic names;
- direct reads of GameState / Player / SystemState fields (`.field` not followed by `(`) per consumer, an
  upper bound (field names such as `round` or `phase` also exist elsewhere);
- id-type mentions per engine module and the share in modules classed as glue (wire, options,
  labels, fingerprints, fixtures) versus rules logic.
"""
import json, os, re, sys
from collections import defaultdict, Counter

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
CRATES = os.path.join(ROOT, 'crates')
ENGINE = os.path.join(CRATES, 'ti4-engine', 'src')
CONSUMERS = ['ti4-host', 'ti4-wasm', 'ti4-server', 'ti4-rule-bot', 'ti4-selfplay', 'ti4-policy',
             'ti4-training', 'ti4-sim', 'ti4-mlp', 'ti4-review', 'ti4-replayer', 'ti4-advisor',
             'ti4-calc', 'ti4-bridge', 'ti4-legacy', 'ti4-cli', 'ti4-bot-agent', 'xtask']
ID_TYPES = ['PlayerId', 'SystemId', 'PlanetId', 'FactionId', 'LeaderId', 'TechnologyId',
            'UnitTypeId', 'ActionCardId', 'ObjectiveId', 'SecretObjectiveId', 'StrategyCardId',
            'BreakthroughId', 'RelicId', 'CommandId', 'TokenId', 'AgendaId', 'LawId', 'EventId',
            'ChoiceId', 'OptionId', 'FragmentId', 'ExplorationCardId', 'ExpeditionTileId',
            'PromissoryNoteId']
# Engine modules whose job is the string boundary (they stay in the adapter), by reading them:
GLUE = {
    'choice': 'Choice/ChoiceOption wire types, option ids, labels, payload helpers, Observed view',
    'fingerprint': 'canonical hashes of records and events',
    'decision_context': 'typed decision context of the wire and the V2 fingerprint',
    'registry': 'coverage ledgers by card id text',
    'fixtures': 'test fixtures built from id text',
    'observation': 'observation/feature views for bots and training',
    'reaction_preview': 'display previews keyed by card names',
    'secondary_preview': 'display previews',
    'preview': 'option consequence previews (analysis, not rules)',
    'event': 'event payloads of the wire',
    'wiring': 'registration tables',
    'valuation': 'bot-side valuation helpers',
    'reach_query': 'observation query for bots',
}

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
            c = text[j]
            depth += (c == '{') - (c == '}')
            j += 1
        i = j
    return ''.join(out)

def is_test_path(p):
    parts = p.split(os.sep)
    return any(x in ('tests', 'benches', 'examples') or x.endswith('_tests') for x in parts) \
        or os.path.basename(p) in ('fixtures.rs', 'tests.rs') or os.path.basename(p).endswith('_tests.rs')

def rs_files(d):
    for base, _, fs in os.walk(d):
        if 'target' in base.split(os.sep):
            continue
        for f in fs:
            if f.endswith('.rs'):
                yield os.path.join(base, f)

def split_prod_test(path):
    text = open(path, encoding='utf-8', errors='replace').read()
    if is_test_path(path) or text.lstrip().startswith('#![cfg(test)]'):
        return '', text
    prod = strip_tests(text)
    return prod, text[len(prod):] if len(prod) < len(text) else ''

def engine_surface():
    items = defaultdict(Counter)
    methods = defaultdict(set)   # type -> pub method names
    for p in rs_files(ENGINE):
        prod, _ = split_prod_test(p)
        if not prod:
            continue
        rel = os.path.relpath(p, ENGINE).split(os.sep)[0]
        mod = rel[:-3] if rel.endswith('.rs') else rel
        # top-level-ish pub items (column 0)
        for m in re.finditer(r'^pub (fn|struct|enum|trait|type|const|static|mod) (\w+)', prod, re.M):
            items[mod][m.group(1)] += 1
        # pub methods inside impl blocks (indented)
        cur = None
        for line in prod.splitlines():
            mi = re.match(r'^impl(?:<[^>]*>)?\s+(?:[\w:<>, ]+\s+for\s+)?(\w+)', line)
            if mi:
                cur = mi.group(1)
                continue
            mm = re.match(r'^    pub (?:const )?fn (\w+)', line)
            if mm and cur:
                items[mod]['method'] += 1
                methods[cur].add(mm.group(1))
    return items, methods

def use_paths(text):
    """Every engine path named: `ti4_engine::a::{b, c::{d}}` expanded, plus inline paths."""
    found = []
    for m in re.finditer(r'\buse\s+ti4_engine::', text):
        j, depth, start = m.end(), 0, m.end()
        while j < len(text):
            c = text[j]
            if c == '{': depth += 1
            elif c == '}': depth -= 1
            elif c == ';' and depth == 0: break
            j += 1
        found.extend(expand(text[start:j]))
    for m in re.finditer(r'(?<!use )\bti4_engine::((?:\w+::)*\w+)', text):
        found.append(m.group(1))
    return found

def expand(tree):
    # `Name as Alias` first: with the blanks gone `CanonicalHash as X` would read as `CanonicalHashasX`, and the alias cut below would
    # also take the `ash` off a name that ends in `Hash`.
    tree = re.sub(r'\s+as\s+\w+', '', tree)
    tree = re.sub(r'\s+', '', tree)
    def rec(prefix, s):
        if '{' not in s:
            return [prefix + s] if s else []
        i = s.index('{')
        head, body = s[:i], s[i + 1:-1]
        parts, depth, cur = [], 0, ''
        for c in body:
            if c == ',' and depth == 0:
                parts.append(cur); cur = ''
                continue
            depth += (c == '{') - (c == '}')
            cur += c
        parts.append(cur)
        out = []
        for part in parts:
            out.extend(rec(prefix + head, part))
        return out
    res = []
    for path in rec('', tree):
        path = path.replace('::self', '')
        res.append(path.rstrip(':'))
    return res

def state_fields():
    text = open(os.path.join(CRATES, 'ti4-model', 'src', 'state.rs'), encoding='utf-8').read()
    fields = {}
    for name in ('GameState', 'Player', 'SystemState'):
        m = re.search(r'pub struct ' + name + r'\s*\{', text)
        if not m:
            continue
        depth, j = 1, m.end()
        while depth:
            depth += (text[j] == '{') - (text[j] == '}')
            j += 1
        fields[name] = re.findall(r'^\s*pub (\w+):', text[m.end():j], re.M)
    return fields

def main():
    out = {}
    items, methods = engine_surface()
    total = Counter()
    for mod, c in items.items():
        total.update(c)
    out['engine_public_items'] = dict(total)
    out['engine_public_items_by_module'] = {m: dict(c) for m, c in sorted(items.items())}
    boundary_types = ['Game', 'Observed', 'SeatObservation', 'Table', 'Choice', 'ChoiceOption',
                      'DecisionRecord', 'MovementRules', 'Galaxy']
    out['boundary_type_methods'] = {t: len(methods.get(t, ())) for t in boundary_types}
    fields = state_fields()
    out['state_fields'] = {k: len(v) for k, v in fields.items()}
    all_fields = set().union(*fields.values())
    obs_methods = methods.get('Observed', set()) | methods.get('SeatObservation', set())
    game_methods = methods.get('Game', set())
    consumers = {}
    for crate in CONSUMERS:
        d = os.path.join(CRATES, crate)
        if not os.path.isdir(d):
            continue
        prod_paths, test_paths = Counter(), Counter()
        prod_text_all = []
        for p in rs_files(d):
            prod, test = split_prod_test(p)
            for path in use_paths(prod):
                prod_paths[path] += 1
            for path in use_paths(test):
                test_paths[path] += 1
            prod_text_all.append(prod)
        prod_text = '\n'.join(prod_text_all)
        field_reads = Counter(m.group(1) for m in re.finditer(r'\.(\w+)\b(?!\s*\()', prod_text)
                              if m.group(1) in all_fields)
        obs_calls = Counter(m.group(1) for m in re.finditer(r'\.(\w+)\s*\(', prod_text)
                            if m.group(1) in obs_methods)
        game_calls = Counter(m.group(1) for m in re.finditer(r'\.(\w+)\s*\(', prod_text)
                             if m.group(1) in game_methods)
        modules = Counter(p.split('::')[0] for p in prod_paths)
        consumers[crate] = {
            'engine_paths_prod_distinct': len(prod_paths),
            'engine_paths_prod_uses': sum(prod_paths.values()),
            'engine_paths_test_distinct': len(test_paths),
            'engine_modules_prod': len(modules),
            'top_modules_prod': modules.most_common(8),
            'state_field_reads_prod': sum(field_reads.values()),
            'state_fields_distinct_prod': len(field_reads),
            'top_state_fields': field_reads.most_common(8),
            'observed_method_calls_prod': sum(obs_calls.values()),
            'observed_methods_distinct_prod': len(obs_calls),
            'game_method_calls_prod': sum(game_calls.values()),
            'game_methods_distinct_prod': len(game_calls),
            'mentions_GameState_prod': len(re.findall(r'\bGameState\b', prod_text)),
            'mentions_Observed_prod': len(re.findall(r'\b(Observed|SeatObservation)\b', prod_text)),
        }
    out['consumers'] = consumers
    # union of engine paths used by all consumers in production code
    union = set()
    for crate in consumers:
        d = os.path.join(CRATES, crate)
        for p in rs_files(d):
            prod, _ = split_prod_test(p)
            union.update(use_paths(prod))
    out['engine_paths_used_by_any_consumer_prod'] = len(union)
    out['engine_modules_used_by_any_consumer_prod'] = len({p.split('::')[0] for p in union})
    # id mentions per engine module
    idre = re.compile(r'\b(' + '|'.join(ID_TYPES) + r')\b')
    lit_new = re.compile(r'\b(?:' + '|'.join(ID_TYPES) + r')::new\(\s*"')
    per_mod = defaultdict(Counter)
    for p in rs_files(ENGINE):
        prod, test = split_prod_test(p)
        rel = os.path.relpath(p, ENGINE).split(os.sep)[0]
        mod = rel[:-3] if rel.endswith('.rs') else rel
        per_mod[mod]['prod_lines'] += prod.count('\n')
        per_mod[mod]['test_lines'] += test.count('\n')
        per_mod[mod]['id_mentions_prod'] += len(idre.findall(prod))
        per_mod[mod]['id_mentions_test'] += len(idre.findall(test))
        per_mod[mod]['id_literals_prod'] += len(lit_new.findall(prod))
        per_mod[mod]['string_literals_prod'] += len(re.findall(r'"[a-z0-9_]{2,24}"', prod))
        per_mod[mod]['format_prod'] += len(re.findall(r'\bformat!\(', prod))
        per_mod[mod]['option_builders_prod'] += len(re.findall(
            r'ChoiceOption::(?:new|labelled)|\.with\(\s*"|offer_(?:card|fact|caption)', prod))
        per_mod[mod]['payload_reads_prod'] += len(re.findall(r'payload\.get\(|\.payload\[', prod))
        per_mod[mod]['faction_marks_prod'] += len(re.findall(r'faction_marks', prod))
    glue = {m: c for m, c in per_mod.items() if m in GLUE}
    logic = {m: c for m, c in per_mod.items() if m not in GLUE}
    def tot(group, key):
        return sum(c[key] for c in group.values())
    out['engine_modules'] = len(per_mod)
    out['id_mentions'] = {
        'prod_total': tot(per_mod, 'id_mentions_prod'),
        'test_total': tot(per_mod, 'id_mentions_test'),
        'prod_in_glue_modules': tot(glue, 'id_mentions_prod'),
        'prod_in_logic_modules': tot(logic, 'id_mentions_prod'),
        'literal_id_constructions_prod': tot(per_mod, 'id_literals_prod'),
        'short_string_literals_prod': tot(per_mod, 'string_literals_prod'),
        'format_calls_prod': tot(per_mod, 'format_prod'),
        'option_builder_sites_prod': tot(per_mod, 'option_builders_prod'),
        'option_builder_sites_in_logic_modules': tot(logic, 'option_builders_prod'),
        'payload_reads_prod': tot(per_mod, 'payload_reads_prod'),
        'faction_marks_prod': tot(per_mod, 'faction_marks_prod'),
        'prod_lines_glue': tot(glue, 'prod_lines'),
        'prod_lines_logic': tot(logic, 'prod_lines'),
    }
    out['per_module'] = {m: dict(c) for m, c in sorted(per_mod.items(), key=lambda kv: -kv[1]['prod_lines'])}
    out['glue_modules'] = GLUE
    if '--json' in sys.argv:
        json.dump(out, open(sys.argv[sys.argv.index('--json') + 1], 'w'), indent=1)
    # summary
    print('engine public items (non-test):', dict(total))
    print('boundary type pub methods:', out['boundary_type_methods'])
    print('state fields:', out['state_fields'])
    print('engine paths used by any consumer (prod): %d in %d modules' % (
        out['engine_paths_used_by_any_consumer_prod'], out['engine_modules_used_by_any_consumer_prod']))
    print('%-14s %6s %6s %5s %7s %7s %7s %7s %7s' % ('consumer', 'paths', 'uses', 'mods', 'fields', 'obs()', 'game()', 'GState', 'Obs'))
    for c, v in consumers.items():
        print('%-14s %6d %6d %5d %7d %7d %7d %7d %7d' % (c, v['engine_paths_prod_distinct'], v['engine_paths_prod_uses'],
              v['engine_modules_prod'], v['state_field_reads_prod'], v['observed_method_calls_prod'],
              v['game_method_calls_prod'], v['mentions_GameState_prod'], v['mentions_Observed_prod']))
    print('id mentions:', out['id_mentions'])
    print('top modules by prod lines:')
    for m, c in list(out['per_module'].items())[:20]:
        print('  %-18s prod %6d ids %5d lit %4d opts %4d fmt %4d marks %4d %s' % (
            m, c['prod_lines'], c['id_mentions_prod'], c['id_literals_prod'], c['option_builders_prod'],
            c['format_prod'], c['faction_marks_prod'], '(glue)' if m in GLUE else ''))

if __name__ == '__main__':
    main()
