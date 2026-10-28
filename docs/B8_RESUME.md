# B8 resume note (strategy cards, second slice)

Branch `core-b8-strategy`, worktree `.claude/worktrees/core-b8`. Private target dir: `/root/deploytmp/core-b8m2` (delete when done); gate output dir `/root/deploytmp/core-b8m2-out`.

Done (all in lockstep, 16 adapter tests + 1 core unit test green, 47 mutants run, survivors justified in ENGINE_KNOWN_ISSUES B8-design-10):
Warfare primary, Diplomacy (both), Politics (both), Leadership (both, over PayStep), Construction (both), Imperial secret draw. Not done: Technology, Imperial primary scoring, StrategySecondaryWindow sequencing, Trade secondary beyond the replenish write.

Last change (uncommitted at the time of writing, committed with this note): layering fix. Leadership/Construction moved to `rules/production/strategy_cards.rs`; `ctx`, `rule_ctx`, `gain_pool_choice` moved to `choice/strategy.rs`. `check_layers core` = 0 violations, `strategy_cards_core` tests pass.

Gate: first attempt was killed after the layers failure (fixed above); clippy and immutable surfaces had passed. NOT yet run to the end: tests, gen-cards check, dual suite, dual golden (driver=dual), board_equivalence, benches (bench + v1..v5 200/200).
Command (from the worktree):
`CARGO_TARGET_DIR=/root/deploytmp/core-b8m2 CARGO_BUILD_JOBS=3 SKIP_AB=1 SKIP_WASM=1 tools/engine_core/verify_package.sh strategy /root/deploytmp/core-b8m2-out /bin/true 3`
Docs hunk already committed in core-integration worktree (commit "docs: B8 second slice ..."). Not pushed or merged anywhere.
