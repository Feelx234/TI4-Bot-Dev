#!/usr/bin/env python3
"""Census of the `faction_marks` key templates of the string engine against the typed families (package D2-1).

    python3 -I tools/engine_core/marks_census.py [--strict] [--all] [ROOT]

`GameState.faction_marks` is the one open set of the codecs (docs/todo/D2-prereqs.md item 5): a key that no family of
`crates/ti4-engine/src/core_sync/keys/marks.rs` (`PATTERNS`) types is refused by the Marks codec ("mark not typed").
This script collects every key written by production code (`faction_marks.insert / entry / remove`, test files and
`#[cfg(test)]` modules skipped), turns it into a template (`format!("a:{x}")` -> `a:{}`), resolves one level of local
`let key = ...` and `fn ..._key(..) -> String` helpers, and checks each template against the patterns. Text based, like
tools/engine_profile/module_graph.py: a template it cannot resolve is listed as UNRESOLVED for hand review.

Prints the unknown templates (and, with --all, every template and the families nobody writes). Exit 1 with --strict
when a template is unknown.
"""
import os
import re
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
SRC = os.path.join(REPO, 'crates', 'ti4-engine', 'src')
MARKS = os.path.join(SRC, 'core_sync', 'keys', 'marks.rs')
FIELD = '[^|:\\s]+'
WORD = '[a-z_:]+'


def strip_comments(text):
    return re.sub(r'//[^\n]*', '', text)


# ---- the typed patterns ------------------------------------------------------------------------------------------

def parse_patterns(path):
    text = strip_comments(open(path).read())
    start = text.index('pub(crate) static PATTERNS')
    body = text[start:]
    # (Family::Name, &[ ...segments... ]),
    out = []
    pos = 0
    for m in re.finditer(r'\(\s*Family::(\w+)\s*,\s*&\[', body):
        i = m.end()
        depth = 1
        j = i
        while depth:
            c = body[j]
            if c == '[':
                depth += 1
            elif c == ']':
                depth -= 1
            j += 1
        out.append((m.group(1), body[i:j - 1]))
    return [(name, seg_regex(segs)) for name, segs in out]


def seg_regex(segs):
    """Regex of a segment list: L("lit") literal, any other name / F(..) a field, O(&[..]) optional group."""
    res = ''
    i = 0
    while i < len(segs):
        if segs[i].isspace() or segs[i] == ',':
            i += 1
            continue
        m = re.compile(r'L\("([^"]*)"\)').match(segs, i)
        if m:
            res += re.escape(m.group(1))
            i = m.end()
            continue
        if segs.startswith('O(&[', i):
            j = i + 4
            depth = 1
            while depth:
                depth += {'[': 1, ']': -1}.get(segs[j], 0)
                j += 1
            res += '(?:' + seg_regex(segs[i + 4:j - 1]) + ')?'
            i = j + 1  # the closing ')'
            continue
        if segs.startswith('F(', i):
            j = i + 2
            depth = 1
            while depth:
                depth += {'(': 1, ')': -1}.get(segs[j], 0)
                j += 1
            res += WORD if segs.startswith('F(K::Word', i) else FIELD
            i = j
            continue
        m = re.compile(r'[A-Z][A-Z0-9_]*').match(segs, i)
        if m:  # SEAT, SYS, PLANET, ...
            res += FIELD
            i = m.end()
            continue
        raise ValueError('cannot parse segment at: ' + segs[i:i + 40])
    return res


# ---- the keys the engine writes -------------------------------------------------------------------------------------

def production_files():
    for root, dirs, files in os.walk(SRC):
        dirs[:] = [d for d in dirs if d not in ('generated',)]
        for f in files:
            if not f.endswith('.rs'):
                continue
            if f.endswith(('_tests.rs', '_check.rs')) or f.startswith('tests') or f == 'fixtures.rs' or '_lockstep' in f:
                continue
            yield os.path.join(root, f)


def production_text(path):
    """The file without comments and without `#[cfg(test)] mod ... { }` blocks."""
    text = strip_comments(open(path).read())
    while True:
        m = re.search(r'#\[cfg\(test\)\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*(\{|;)', text)
        if not m:
            return text
        if m.group(1) == ';':
            text = text[:m.start()] + text[m.end():]
            continue
        j = m.end()
        depth = 1
        while depth and j < len(text):
            depth += {'{': 1, '}': -1}.get(text[j], 0)
            j += 1
        text = text[:m.start()] + ' ' * 0 + text[j:]


def first_arg(text, i):
    """The first argument of the call whose '(' is at text[i-1]."""
    depth = 0
    j = i
    in_str = False
    while j < len(text):
        c = text[j]
        if in_str:
            if c == '\\':
                j += 1
            elif c == '"':
                in_str = False
        elif c == '"':
            in_str = True
        elif c in '([{':
            depth += 1
        elif c in ')]}':
            if depth == 0:
                break
            depth -= 1
        elif c == ',' and depth == 0:
            break
        j += 1
    return text[i:j].strip()


# Templates the text analysis cannot resolve (a key built by a helper that calls another helper), checked by hand:
# (file suffix, template) -> the family it is.
REVIEWED = {
    ('factions/nekro_units.rs', '{}{}|{}'): 'NekroAlastor (alastor_key = alastor_prefix("nekro:alastor|seq|system|player|") + planet|unit)',
}

CONSTS = {}
LOCAL = {}


def load_consts():
    """`const NAME: &str = "lit";` of the whole engine (a name defined twice with different values is dropped)."""
    dup = set()
    for root, _dirs, files in os.walk(SRC):
        for f in files:
            if not f.endswith('.rs'):
                continue
            for m in re.finditer(r'\bconst\s+([A-Z][A-Z0-9_]*)\s*:\s*&(?:\'static\s+)?str\s*=\s*"([^"]*)"',
                                 strip_comments(open(os.path.join(root, f)).read())):
                if CONSTS.get(m.group(1), m.group(2)) != m.group(2):
                    dup.add(m.group(1))
                CONSTS[m.group(1)] = m.group(2)
    for d in dup:
        CONSTS.pop(d, None)


def local_consts(text):
    return {m.group(1): m.group(2) for m in re.finditer(
        r'\bconst\s+([A-Z][A-Z0-9_]*)\s*:\s*&(?:\'static\s+)?str\s*=\s*"([^"]*)"', text)}


def split_args(text):
    """Top-level comma split of a call's argument text."""
    out, depth, cur, in_str, i = [], 0, '', False, 0
    while i < len(text):
        c = text[i]
        if in_str:
            cur += c
            if c == '\\':
                i += 1
                cur += text[i]
            elif c == '"':
                in_str = False
        elif c == '"':
            in_str = True
            cur += c
        elif c in '([{':
            depth += 1
            cur += c
        elif c in ')]}':
            depth -= 1
            cur += c
        elif c == ',' and depth == 0:
            out.append(cur.strip())
            cur = ''
        else:
            cur += c
        i += 1
    if cur.strip():
        out.append(cur.strip())
    return out


def expand_format(expr):
    """`format!("..{}..", a, b)` -> template with consts substituted and runtime parts as '{}'."""
    m = re.match(r'format!\(', expr)
    inner = expr[m.end():expr.rindex(')')]
    args = split_args(inner)
    fmt = re.match(r'"((?:[^"\\]|\\.)*)"$', args[0], re.S)
    if not fmt:
        return None
    rest = args[1:]
    named = {}
    positional = []
    for a in rest:
        nm = re.match(r'([a-z_]\w*)\s*=\s*(.*)$', a, re.S)
        if nm:
            named[nm.group(1)] = nm.group(2)
        else:
            positional.append(a)
    index = 0

    def const_of(arg):
        last = re.split(r'::', arg.strip().lstrip('&'))[-1].strip()
        return LOCAL.get(last, CONSTS.get(last))

    def sub(mm):
        nonlocal index
        name = mm.group(1)
        if name == '':
            arg = positional[index] if index < len(positional) else ''
            index += 1
            v = const_of(arg)
        elif name.isdigit():
            v = const_of(positional[int(name)]) if int(name) < len(positional) else None
        elif name in named:
            v = const_of(named[name])
        else:
            v = LOCAL.get(name, CONSTS.get(name))
        return v if v is not None else '{}'
    return re.sub(r'\{([^}:]*)(?::[^}]*)?\}', sub, fmt.group(1))


def to_template(expr, text, seen=()):
    """The template(s) of a key expression: list of strings with '{}' for a runtime part, or None."""
    e = expr.strip()
    e = re.sub(r'^&+', '', e).strip()
    e = re.sub(r'\.(clone|to_owned|into|to_string)\(\)$', '', e).strip()
    m = re.fullmatch(r'String::from\((.*)\)', e, re.S)
    if m:
        return to_template(m.group(1), text, seen)
    m = re.fullmatch(r'"((?:[^"\\]|\\.)*)"', e, re.S)
    if m:
        return [m.group(1)]
    if e.startswith('format!('):
        t = expand_format(e)
        return [t] if t is not None else None
    m = re.fullmatch(r'([A-Za-z_]\w*)', e)
    if m and e not in seen:
        name = e
        found = []
        for d in re.finditer(r'\blet\s+(?:mut\s+)?' + re.escape(name) + r'\s*(?::\s*[\w<>&\' ]+)?=\s*([^;]+);', text):
            t = to_template(d.group(1), text, seen + (name,))
            if t:
                found += t
        if name in LOCAL:
            found.append(LOCAL[name])
        for d in re.finditer(r'\b(?:const|static)\s+' + re.escape(name) + r'\s*:\s*&(?:\'static\s+)?str\s*=\s*"([^"]*)"', text):
            found.append(d.group(1))
        return found or None
    m = re.fullmatch(r'([A-Za-z_][\w:]*)\((.*)\)', e, re.S)
    if m:
        fn = m.group(1).split('::')[-1]
        found = []
        for d in re.finditer(r'\bfn\s+' + re.escape(fn) + r'\b[^{;]*->\s*String\s*\{', text):
            j = d.end()
            depth = 1
            while depth and j < len(text):
                depth += {'{': 1, '}': -1}.get(text[j], 0)
                j += 1
            body = text[d.end():j - 1]
            for f in re.finditer(r'format!\(', body):
                j2 = f.end()
                depth2 = 1
                while depth2 and j2 < len(body):
                    depth2 += {'(': 1, ')': -1}.get(body[j2], 0)
                    j2 += 1
                t = expand_format(body[f.start():j2])
                if t is not None:
                    found.append(t)
        return found or None
    return None


def collect():
    rows = []  # (template or None, file, line, expr)
    for path in production_files():
        text = production_text(path)
        LOCAL.clear()
        LOCAL.update(local_consts(text))
        for m in re.finditer(r'faction_marks\s*\.\s*(insert|entry|remove)\s*\(', text):
            arg = first_arg(text, m.end())
            line = text.count('\n', 0, m.start()) + 1
            rel = os.path.relpath(path, REPO)
            templates = to_template(arg, text)
            if templates is None:
                rows.append((None, rel, line, arg))
            else:
                for t in templates:
                    rows.append((t, rel, line, arg))
    return rows


def main():
    strict = '--strict' in sys.argv
    show_all = '--all' in sys.argv
    load_consts()
    patterns = [(n, re.compile(r)) for n, r in parse_patterns(MARKS)]
    rows = collect()
    unknown, unresolved = [], []
    families = {}
    templates = {}
    for t, rel, line, arg in rows:
        if t is None:
            unresolved.append((rel, line, arg))
            continue
        # a runtime part is one field, or a `note:faction` style pair (the note key of the cards marks)
        samples = [t.replace('{}', 'x'), t.replace('{}', 'x:x')]
        fam = next((n for n, rx in patterns if any(rx.fullmatch(sm) for sm in samples)), None)
        templates.setdefault(t, []).append((rel, line, fam))
        reviewed = REVIEWED.get((rel.split('ti4-engine/src/')[-1], t))
        if reviewed:
            fam = reviewed.split()[0]
        if fam:
            families.setdefault(fam, set()).add(t)
        else:
            unknown.append((t, rel, line))
    print(f'marks census: {len(rows)} write sites, {len(templates)} distinct templates, {len(patterns)} typed families')
    for t, rel, line in sorted(set(unknown)):
        print(f'UNKNOWN    {t!r:60} {rel}:{line}')
    for rel, line, arg in unresolved:
        print(f'UNRESOLVED {arg[:60]!r:62} {rel}:{line}  (hand review)')
    if show_all:
        for t, where in sorted(templates.items()):
            print(f'  {t!r:60} -> {where[0][2] or "-"}  ({len(where)} sites)')
        silent = [n for n, _ in patterns if n not in families]
        print(f'families no template writes ({len(silent)}): {", ".join(silent)}')
    print(f'unknown templates: {len(set(unknown))}, unresolved sites: {len(unresolved)}')
    sys.exit(1 if strict and unknown else 0)


if __name__ == '__main__':
    main()
