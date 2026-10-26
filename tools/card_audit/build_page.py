#!/usr/bin/env python3
"""Build the browsable card-audit page from `card_audit.py --json` output.

    python3 -I tools/card_audit/card_audit.py --json /tmp/audit.json
    python3 -I tools/card_audit/build_page.py /tmp/audit.json /tmp/card-audit.html

One self-contained HTML file (no external scripts): a per-card table filterable by category,
status, field class (cards with a gameplay / text-parsed field) and issue, a field table filterable
by category and class, and the issue list.
"""

import json
import sys
from pathlib import Path

CARD_KEYS = ["category", "id", "name", "source", "status", "status_detail", "gameplay_fields",
             "text_parsed_fields", "refs_engine", "refs_host", "refs_bot", "refs_web",
             "first_engine_ref", "partial_markers", "name_keyed_sites", "threshold_check",
             "id_collides_with", "issues", "fields"]
FIELD_KEYS = ["category", "field", "records", "of", "bytes", "classification", "engine", "host",
              "bot", "web_sites", "host_catalog_field", "use", "evidence"]


def main():
    src, out = sys.argv[1], sys.argv[2]
    a = json.loads(Path(src).read_text())
    data = {
        "cards": [[c.get(k, "") for k in CARD_KEYS] for c in a["cards"]],
        "cardKeys": CARD_KEYS,
        "fields": [[f.get(k, "") for k in FIELD_KEYS] for f in a["fields"]],
        "fieldKeys": FIELD_KEYS,
        "issues": a["issues"],
        "perCategory": a["per_category"],
        "implementation": a.get("implementation", {}),
        "bytesByClass": a["bytes_by_class"],
        "bytesTotal": a["bytes_total"],
    }
    template = (Path(__file__).parent / "page_template.html").read_text()
    blob = json.dumps(data, separators=(",", ":"), ensure_ascii=False).replace("</", "<\\/")
    Path(out).write_text(template.replace("/*DATA*/null", blob))
    print(f"wrote {out} ({Path(out).stat().st_size} bytes)")


if __name__ == "__main__":
    main()
