#!/usr/bin/env python3
"""Hygiene lint for the core crates (docs/ENGINE_CORE_STATE.md 4.1): no String / &str / serde /
BTreeMap / BTreeSet / format! / unsafe in non-test code. Simple version by package F1; F3's
lint_core.sh supersedes it. Usage: f1_lint_core.py [crate_src_dir ...]  (default: ti4-engine-core/src)."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BANNED = [
    (r"\bString\b", "String"),
    (r"&\s*(?:'\w+\s+)?str\b", "&str"),
    (r"\bserde\b", "serde"),
    (r"\bBTreeMap\b", "BTreeMap"),
    (r"\bBTreeSet\b", "BTreeSet"),
    (r"\bformat!", "format!"),
    (r"\bunsafe\b", "unsafe"),
]


def strip(src: str) -> str:
    """Remove comments, string/char literal contents, and #[cfg(test)] items."""
    out, i, n = [], 0, len(src)
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            while i < n and src[i] != "\n":
                i += 1
        elif src.startswith("/*", i):
            depth = 1
            i += 2
            while i < n and depth:
                if src.startswith("/*", i):
                    depth += 1
                    i += 2
                elif src.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
        elif c == '"':
            i += 1
            while i < n and src[i] != '"':
                i += 2 if src[i] == "\\" else 1
            i += 1
            out.append('""')
        else:
            out.append(c)
            i += 1
    text = "".join(out)
    # Drop `#[cfg(test)]` items (a module or fn with a braced body).
    while True:
        m = re.search(r"#\[cfg\(test\)\]", text)
        if not m:
            return text
        j = text.find("{", m.end())
        semi = text.find(";", m.end())
        if semi != -1 and (j == -1 or semi < j):
            text = text[: m.start()] + text[semi + 1 :]
            continue
        depth, k = 0, j
        while k < len(text):
            depth += (text[k] == "{") - (text[k] == "}")
            k += 1
            if depth == 0:
                break
        text = text[: m.start()] + text[k:]


def main() -> int:
    dirs = [Path(a) for a in sys.argv[1:]] or [ROOT / "crates/ti4-engine-core/src"]
    bad = 0
    for d in dirs:
        for path in sorted(d.rglob("*.rs")):
            text = strip(path.read_text())
            for lineno, line in enumerate(text.splitlines(), 1):
                for pat, name in BANNED:
                    if re.search(pat, line):
                        print(f"{path}:{lineno}: {name}: {line.strip()}")
                        bad += 1
    print(f"lint_core: {bad} violation(s)")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
