"""The Thunder's Edge lines of the digest (digest.py) and of the offline entry (offline_entry.py).

A run either played with Thunder's Edge content on (meta.json "thunders_edge": true, the table
switch; not the `te` strategy card set) or did not. The report.json of the harness carries
`thundersEdge` and `expedition` {offers, claimed, held, completed, followUps}.
"""


def is_te(meta, report):
    return bool(meta.get("thunders_edge") or (report or {}).get("thundersEdge"))


def label(meta, report):
    """Short label for the header line of a digest or an entry."""
    return "Thunder's Edge ON" if is_te(meta, report) else "Thunder's Edge off"


def line(meta, report, state=None, faction=None):
    """The digest bullet: expedition counts of a Thunder's Edge run, or a note that it was off."""
    if not is_te(meta, report):
        return "- Thunder's Edge: off (default table; no expedition in this run)"
    ex = (report or {}).get("expedition") or {}
    held = ex.get("held") or (state or {}).get("expedition_slices") or {}
    claimed = ex.get("claimed") or {}
    faction = faction or {}
    by_seat = {}
    for slice_id, seat in sorted(held.items()):
        by_seat.setdefault(faction.get(seat, seat), []).append(slice_id)
    follow = ex.get("followUps") or {}
    return (
        "- Thunder's Edge: ON. Expedition: a slice was offered to the click-driven seat(s) in "
        f"{ex.get('offers', 0)} questions; claimed by the click driver {sum(claimed.values())}"
        + (f" ({', '.join(f'{k} x{v}' for k, v in sorted(claimed.items()))})" if claimed else "")
        + f"; slices held at the end {len(held)}/6"
        + (f" ({'; '.join(f'{who}: {', '.join(v)}' for who, v in sorted(by_seat.items()))})" if held else "")
        + f"; completed (planet placed): {'yes' if len(held) >= 6 else 'no'}"
        + (f"; expedition follow-up questions: {', '.join(f'{k} x{v}' for k, v in sorted(follow.items()))}" if follow else "")
    )
