#!/usr/bin/env python3
"""Group a symbol-size list by crate (first path segment of the demangled name).

Input: CSV from `twiggy top -n 1000000 --format csv <wasm>` (columns Name,ShallowSize,...), or
`nm -S --size-sort -C` text (`--nm`). Prints `bytes  percent  count  group`.
For `<T as Trait>::f` the group is the crate of T; a generic from core/alloc/std keeps that name
(its instantiation is counted under it, not under the crate that asked for it).
"""
import csv, re, sys, collections

def group(name):
    if name.startswith('"function names"') or name.startswith('custom section') or name.startswith('data segment') or name.startswith('"'):
        return '[' + re.sub(r'[0-9]+$', '', name.strip('"'))[:28] + ']'
    m = re.sub(r'^<(&?(mut )?(dyn )?)', '', name) if name.startswith('<') else name
    m = m.lstrip('&').replace('mut ', '', 1) if m.startswith('&') else m
    key = m.split('::')[0].split(' ')[0].split('<')[0]
    return key or '?'

def main():
    path = sys.argv[1]; mode_nm = '--nm' in sys.argv
    agg = collections.Counter(); cnt = collections.Counter()
    if mode_nm:
        for line in open(path, errors='replace'):
            p = line.rstrip('\n').split(' ', 3)
            if len(p) < 4: continue
            try: size = int(p[1], 16)
            except ValueError: continue
            g = group(p[3]); agg[g] += size; cnt[g] += 1
    else:
        for r in csv.DictReader(open(path)):
            g = group(r['Name']); agg[g] += int(r['ShallowSize']); cnt[g] += 1
    total = sum(agg.values()); top = int(sys.argv[2]) if len(sys.argv) > 2 and sys.argv[2].isdigit() else 30
    print(f"{'bytes':>10} {'%':>6} {'items':>7}  group")
    for k, v in agg.most_common(top):
        print(f"{v:>10} {v/total*100:5.1f}% {cnt[k]:>7}  {k}")
    print(f"{total:>10} 100.0% {sum(cnt.values()):>7}  TOTAL")

main()
