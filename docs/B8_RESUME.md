# B8 resume note (strategy cards)

Branch `core-b8-strategy`, worktree `.claude/worktrees/core-b8`. Private target dir `/root/deploytmp/core-b8m2` (delete when done); gate output `/root/deploytmp/core-b8m2-out`.

Done (lockstep tests in `strategy_cards_core::tests`, mutation-checked, survivors justified in ENGINE_KNOWN_ISSUES B8-design-10 and -13): Warfare primary, Diplomacy, Politics, Leadership (over PayStep), Construction, Technology (plain path), Imperial primary (scoring stays string) and secret draw, Trade secondary, the follower's strategy token spend. Not ported: the secondary sequencing window (documented: question has no DecisionContext; shell of string records), TE variants.

Regression tests for failures found: layering (first gate attempt) -> `tools/engine_core/check_layers.py core` is the test; decision-delivery registry -> `decision_delivery_inventory::every_producer_and_delivery_site_matches_the_reviewed_registry` (failed before the rename of four entries).

Remaining: the final gate, once, on the final commit:
`CARGO_TARGET_DIR=/root/deploytmp/core-b8m2 CARGO_BUILD_JOBS=3 SKIP_AB=1 SKIP_WASM=1 tools/engine_core/verify_package.sh strategy /root/deploytmp/core-b8m2-out /bin/true 3`
If a step fails: fix with a regression test named for it, rerun only that step. Then delete the two private dirs. Never push or merge.
