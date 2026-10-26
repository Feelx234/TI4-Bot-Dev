#!/usr/bin/env python3
"""Attribute the samples of a `perf record -g` profile of the engine to categories.

    perf script -i perf.data --show-mmap-events -F tid,ip,sym --no-inline > perf.txt
    python3 -I tools/engine_profile/categorize.py perf.txt [--json out.json] [--top 40]

Needs a build with frame pointers (RUSTFLAGS="-C force-frame-pointers=yes"), so every sample has
its whole stack. Two independent breakdowns of the same samples:

* layer: WHO spent the time, from the stack (root side wins: a bot decision that calls engine
  queries is bot time; a step of the engine that is not inside the bot is engine time, split by
  the engine's entry points);
* kind: WHAT the CPU was doing, from the leaf side (malloc/free, memcmp of strings, memcpy,
  BTreeMap walks, clone/drop, serde_json, formatting, content lookups, sorting, other code).

Plus the inclusive share of a list of named functions (`INCLUSIVE`), counted once per sample.
glibc has no symbols here; its hot addresses are bucketed by offset (memcmp_evex at 0x1a73c0,
memmove at 0x1b0a00, the malloc internals around 0xb0000-0xb6000 of this libc build; see
docs/ENGINE_OPTIMIZATION.md for how they were identified). Another libc needs other ranges.
"""
import json
import re
import sys
from collections import Counter, defaultdict

LIBC_RANGES = [  # (start, end, name) offsets into libc.so.6 of the profiled machine
    (0x1A7300, 0x1A7C00, "memcmp"),
    (0x1B0800, 0x1B1800, "memcpy/memmove"),
    (0x1A0000, 0x1C0000, "other libc string op"),
    (0x09A000, 0x0B8000, "malloc/free"),
]

INCLUSIVE = [
    "ti4_engine::game::Game::step",
    "ti4_engine::game::Game::legal_options",
    "ti4_engine::game::Game::apply_choice",
    "ti4_engine::choice::Table::ask_seeing",
    "ti4_rule_bot::choose_explained",
    "ti4_rule_bot::pick",
    "ti4_rule_bot::backward::verdict",
    "ti4_rule_bot::backward::PlanCtx::new",
    "ti4_rule_bot::planner::Bias::of",
    "ti4_rule_bot::activation_value",
    "ti4_rule_bot::movable_for",
    "ti4_engine::choice::Observed::movable_into",
    "ti4_engine::tactical::movable_into",
    "ti4_engine::movement::MovementRules::search_path",
    "ti4_engine::movement::MovementRules::with_laws",
    "ti4_engine::movement::Board::for_player",
    "ti4_engine::movement::replay_map_edits",
    "ti4_engine::laws::apply_to_galaxy",
    "ti4_engine::space_stations::reconcile_all",
    "ti4_engine::factions::crimson::enforce_sundered",
    "ti4_engine::fleet::enforce_everywhere",
    "ti4_engine::fleet::enforce_seeing",
    "ti4_engine::transactions::may_transact",
    "ti4_engine::timing::Resolver::checkpoint",
    "ti4_engine::timing::Resolver::emit",
    "ti4_engine::game::Game::emit_typed",
    "ti4_engine::game::Game::fork",
    "ti4_engine::game::Game::snapshot",
    "ti4_engine::choice::DecisionLog::record",
    "ti4_engine::technology::start_turn",
    "ti4_engine::leaders::check_unlocks",
    "ti4_content::units::catalogue",
    "ti4_content::galaxy::Galaxy::adjacent",
    "ti4_content::galaxy::planets_in",
    "ti4_content::loader::ContentStore::get",
    "ti4_content::loader::ContentStore::resolve_id",
    "<ti4_model::state::GameState as core::clone::Clone>::clone",
    "<ti4_content::galaxy::Galaxy as core::clone::Clone>::clone",
    "ti4_selfplay::timeline::Recorder::observe",
    "ti4_selfplay::play::unit_census",
    "ti4_wasm::host_session::decisions_hash",
    "serde_json::",
    "ti4_host::",
]


def libc_name(offset):
    for start, end, name in LIBC_RANGES:
        if start <= offset < end:
            return name
    return "other libc"


def parse(path):
    """Yield (tid, [frame names leaf -> root]) per sample."""
    bases = {}  # pid -> (start, pgoff) of libc's text mapping
    mmap = re.compile(r"PERF_RECORD_MMAP2 (\d+)/\d+: \[0x([0-9a-f]+)\(0x[0-9a-f]+\) @ 0x([0-9a-f]+) .*libc\.so")
    tid, frames, pid_of = None, [], {}
    with open(path, errors="replace") as fh:
        for line in fh:
            if "PERF_RECORD_MMAP2" in line:
                m = mmap.search(line)
                if m:
                    bases[int(m.group(1))] = (int(m.group(2), 16), int(m.group(3), 16))
                continue
            if not line.strip():
                if frames:
                    yield tid, frames
                tid, frames = None, []
                continue
            if not line.startswith(("\t", " ")):
                tid = int(line.split()[0])
                continue
            parts = line.strip().split(None, 1)
            ip = int(parts[0], 16)
            name = parts[1] if len(parts) > 1 else "[unknown]"
            if name == "[unknown]":
                # libc: resolve by the offset in the mapping of any process seen
                for start, pgoff in bases.values():
                    if start <= ip < start + 0x200000:
                        name = "libc:" + libc_name(ip - start + pgoff)
                        break
            frames.append(name)
    if frames:
        yield tid, frames


def layer_of(frames):
    """Who: root-side context of the sample."""
    s = " | ".join(frames)
    if "ti4_rule_bot::" in s:
        # bot decision; the part spent inside engine/content queries it made
        leaf_bot = next(i for i, f in enumerate(frames) if f.startswith("ti4_rule_bot::"))
        below = frames[:leaf_bot]
        if any(f.startswith(("ti4_engine::", "ti4_content::")) for f in below):
            return "bot: engine/content queries"
        return "bot: own logic"
    if "ti4_selfplay::timeline::" in s:
        return "harness: timeline recorder"
    if "decisions_hash" in s:
        return "harness: decision hash"
    if "ti4_selfplay::play::unit_census" in s or "record_rift_rolls" in s:
        return "harness: rift census"
    if "ti4_host::" in s and "Game::step" not in s:
        return "host: views/projection"
    if "ti4_engine::" in s:
        for key, label in [
            ("Table::ask_seeing", "engine: ask/record (not bot)"),
            ("Game::legal_options", "engine: option generation"),
            ("Game::apply_choice", "engine: apply answer"),
            ("space_stations::reconcile_all", "engine: per-step reconcile"),
            ("laws::apply_to_galaxy", "engine: per-step reconcile"),
            ("replay_map_edits", "engine: per-step reconcile"),
            ("enforce_sundered", "engine: per-step reconcile"),
            ("settle_control_points", "engine: per-step reconcile"),
            ("keleres::reconcile", "engine: per-step reconcile"),
            ("nexus_is_triggered", "engine: per-step reconcile"),
            ("spec_ops", "engine: per-step reconcile"),
            ("Game::step", "engine: automatic steps (windows, phases, combat)"),
        ]:
            if key in s:
                return label
        return "engine: setup/other"
    if "ti4_selfplay::" in s or "ti4_wasm::" in s:
        return "harness: other"
    return "other (kernel, runtime)"


CLONE_FRAMES = (
    "ti4_engine::timing::Resolver::checkpoint",
    "<ti4_model::state::GameState as core::clone::Clone>::clone",
    "<ti4_model::state::SystemState as core::clone::Clone>::clone",
    "<ti4_content::galaxy::Galaxy as core::clone::Clone>::clone",
    "<ti4_engine::choice::DecisionRecord as core::clone::Clone>::clone",
    "<ti4_engine::choice::DecisionLog as core::clone::Clone>::clone",
    "<ti4_engine::timing::AbilityRegistry as core::clone::Clone>::clone",
    "ti4_engine::game::Game::fork",
    "ti4_engine::game::Game::snapshot_with",
)
MOVEMENT_FRAMES = (
    "ti4_engine::tactical::movable_into",
    "ti4_engine::choice::Observed::movable_into",
    "ti4_engine::choice::Observed::can_reach",
    "ti4_engine::choice::Observed::rift_exits_into",
    "ti4_engine::movement::",
)


def bucket_of(frames):
    """The split asked for bot-only games, exclusive, first match wins (see the doc)."""
    s = " | ".join(frames)
    if "ti4_selfplay::timeline::" in s or "unit_census" in s or "record_rift_rolls" in s:
        return "harness: timeline / statistics"
    if "decisions_hash" in s:
        return "logging: decision hash"
    if any(f in s for f in CLONE_FRAMES):
        return "state cloning / checkpoints"
    if "ti4_content::units::catalogue" in s or "ti4_content::galaxy::all_systems" in s:
        return "content index copies"
    if "ti4_host::" in s:
        return "view building"
    if "serde_json::ser" in s or "Serialize>::serialize" in s:
        return "serialization"
    if "ti4_rule_bot::" in s:
        if any(f in s for f in MOVEMENT_FRAMES):
            return "bot: movement/reach queries (engine code)"
        if "ti4_rule_bot::objectives::goals" in s or "objective_progress" in s or "PlanCtx::meets" in s:
            return "bot: objective/goal evaluation"
        return "bot: other decision logic"
    if "DecisionLog::record" in s or "Table::ask_seeing" in s:
        return "logging: decision record"
    if "Game::legal_options" in s:
        return "engine: option generation"
    if "Game::apply_choice" in s:
        return "engine: applying answers"
    if "ti4_engine::" in s:
        return "engine: automatic steps (windows, combat, phases, reconcile)"
    if "ti4_selfplay::" in s:
        return "harness: other"
    return "other (kernel, runtime, unattributed)"


KIND_RULES = [
    ("libc:malloc/free", "malloc/free/realloc"),
    ("malloc", "malloc/free/realloc"),
    ("cfree", "malloc/free/realloc"),
    ("realloc", "malloc/free/realloc"),
    ("__rust_alloc", "malloc/free/realloc"),
    ("__rust_dealloc", "malloc/free/realloc"),
    ("mi_", "malloc/free/realloc"),  # mimalloc
    ("_mi_", "malloc/free/realloc"),
    ("libc:memcmp", "memcmp (string compare)"),
    ("libc:memcpy", "memcpy/memmove"),
    ("libc:other libc string op", "memcpy/memmove"),
]


def kind_of(frames):
    """What: leaf-side work of the sample."""
    leaf = frames[0]
    for key, label in KIND_RULES:
        if leaf.startswith(key) or leaf == key:
            return label
    if leaf.startswith("libc:"):
        return "other libc"
    near = frames[:3]
    if any("drop_in_place" in f for f in near[:1]):
        return "drop (destructors)"
    if any("as core::clone::Clone>::clone" in f or "to_owned" in f or "to_vec" in f for f in near[:2]):
        return "clone"
    if "serde_json" in leaf or "serde::" in leaf:
        return "serde/json"
    if leaf.startswith(("core::fmt", "alloc::fmt", "<alloc::string::String as core::fmt")):
        return "formatting"
    if "alloc::collections::btree" in leaf:
        return "BTreeMap/BTreeSet"
    if leaf.startswith(("hashbrown", "std::collections::hash", "core::hash")):
        return "HashMap"
    if "sort" in leaf:
        return "sorting"
    if leaf.startswith("ti4_content::"):
        return "content lookups (ti4_content)"
    if leaf.startswith("[k]") or "kernel" in leaf or leaf.startswith(("asm_", "__x64", "entry_")):
        return "kernel"
    if leaf.startswith("ti4_rule_bot::"):
        return "bot code"
    if leaf.startswith(("ti4_engine::", "ti4_model::")):
        return "engine/model code"
    return "other code (iterators, std)"


def main():
    path = sys.argv[1]
    top = int(sys.argv[sys.argv.index("--top") + 1]) if "--top" in sys.argv else 40
    layers, kinds, both = Counter(), Counter(), defaultdict(Counter)
    buckets = Counter()
    inclusive = Counter()
    selfsym = Counter()
    under = sys.argv[sys.argv.index("--under") + 1] if "--under" in sys.argv else None
    under_incl = Counter()
    under_n = 0
    n = 0
    for _, frames in parse(path):
        n += 1
        lay, kind = layer_of(frames), kind_of(frames)
        layers[lay] += 1
        buckets[bucket_of(frames)] += 1
        kinds[kind] += 1
        both[lay][kind] += 1
        selfsym[frames[0]] += 1
        if under and lay.startswith(under):
            under_n += 1
            for f in set(frames):
                if f.startswith(("ti4_", "<ti4_")):
                    under_incl[f] += 1
        seen = set()
        for f in frames:
            for name in INCLUSIVE:
                if name not in seen and (f == name or (name.endswith("::") and f.startswith(name)) or f.startswith(name + "::{{closure}}")):
                    inclusive[name] += 1
                    seen.add(name)
    pct = lambda c: 100.0 * c / n if n else 0.0
    print(f"samples {n}")
    print("\n== layer (who) ==")
    for k, c in layers.most_common():
        print(f"{pct(c):6.1f}%  {k}")
    print("\n== bucket (bot-only split, exclusive) ==")
    for k, c in buckets.most_common():
        print(f"{pct(c):6.1f}%  {k}")
    print("\n== kind (what, leaf) ==")
    for k, c in kinds.most_common():
        print(f"{pct(c):6.1f}%  {k}")
    print("\n== inclusive (named functions, % of all samples) ==")
    for k, c in sorted(inclusive.items(), key=lambda kv: -kv[1]):
        print(f"{pct(c):6.1f}%  {k}")
    print(f"\n== top {top} self symbols ==")
    for k, c in selfsym.most_common(top):
        print(f"{pct(c):6.2f}%  {k[:150]}")
    if under:
        print(f"\n== inclusive frames within layer {under!r} ({pct(under_n):.1f}% of samples), % of all samples ==")
        for k, c in under_incl.most_common(top):
            print(f"{pct(c):6.2f}%  {k[:150]}")
    if "--json" in sys.argv:
        out = sys.argv[sys.argv.index("--json") + 1]
        json.dump(
            {
                "samples": n,
                "layers": {k: pct(c) for k, c in layers.most_common()},
                "buckets": {k: pct(c) for k, c in buckets.most_common()},
                "kinds": {k: pct(c) for k, c in kinds.most_common()},
                "layer_kind": {l: {k: pct(c) for k, c in v.most_common()} for l, v in both.items()},
                "inclusive": {k: pct(c) for k, c in inclusive.most_common()},
                "self": {k: pct(c) for k, c in selfsym.most_common(top)},
            },
            open(out, "w"),
            indent=1,
        )


if __name__ == "__main__":
    main()
