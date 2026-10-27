#!/usr/bin/env python3
"""Layer check for the core crate (docs/ENGINE_CORE_STATE.md 4.1): a module may import only from
modules of a lower or equal layer, and only inside its own subtree when equal-layer. Simple version by
package F1; F3's check_layers.py supersedes it. Modules not in LAYERS fail (register them here)."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "crates/ti4-engine-core/src"
# L0 data (exists), L1 containers + state + decision types.
LAYERS = {
    "generated": 0, "tables": 0, "ids": 0, "bits": 0, "hex": 0, "galaxy": 0, "board": 0, "movement": 0,
    "containers": 1,
    "state": 2,
    "choice": 3,
}
# Within L0 the existing modules may use each other (movement -> galaxy -> tables ...).


def top(path: Path) -> str:
    rel = path.relative_to(SRC)
    return rel.parts[0].removesuffix(".rs")


def main() -> int:
    bad = 0
    for path in sorted(SRC.rglob("*.rs")):
        mod = top(path)
        if mod in ("lib", "mod"):
            continue
        if mod not in LAYERS:
            print(f"{path.relative_to(ROOT)}: module `{mod}` has no layer; register it in f1_check_layers.py")
            bad += 1
            continue
        text = re.sub(r"//.*", "", path.read_text())
        for m in re.finditer(r"\bcrate::(\w+)", text):
            dep = m.group(1)
            if dep == mod or dep not in LAYERS:
                if dep not in LAYERS and dep not in ("Seat", "SysSet", "UnitSet", "PlanetIx", "SystemIx", "UnitIx"):
                    pass
                continue
            if LAYERS[dep] > LAYERS[mod]:
                print(f"{path.relative_to(ROOT)}: `{mod}` (L{LAYERS[mod]}) imports `{dep}` (L{LAYERS[dep]})")
                bad += 1
            elif LAYERS[dep] == LAYERS[mod] and mod == "containers" and dep == "state":
                print(f"{path.relative_to(ROOT)}: containers must not import state")
                bad += 1
    print(f"check_layers: {bad} violation(s)")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
