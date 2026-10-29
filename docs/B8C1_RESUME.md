# B8c-1 (strategy secondary windows): state and what is left

Branch `core-b8c` (from `core-integration-3`). The definition of done of B8c-1 is reached: one follower step of the
strategic-action window (`StrategySecondaryWindow::next_choice` + the table's answer + `take_choice`, `Game::secondary_step`)
runs beside the core `rules::strategy_secondary::SecondaryStep` in lockstep (`dual_window!(Area::Strategy,
"strategy::secondary", ..)`), with the new `choice::strategy::{Ask::Follow, Opt::{Skip, Follow, Waive, PrimaryInstead}, Subtype::FollowSecondary}`
and a renderer that is byte-exact for all twelve cards at both costs.

**Gate deferred until the strategy area is complete** (user policy): no `verify_package.sh`, golden dual, wide dual, benches,
`board_equivalence` or workspace clippy has been run for this package. B8c-2 (Technology non-plain paths) and B8c-3
(`te4construction`, `te6warfare`) are open; the gate runs once all of strategy is done (use the sharded golden dual from
`core-dual-shards` when it is merged).

Ran: `cargo check` (workspace without the torch crates, all targets), `cargo test -p ti4-engine-core -p
ti4-engine-core-factions -p ti4-engine --no-fail-fast`, `check_layers core`, `lint_core`, mutation checks of the core half.

Left (not part of B8c-1): B8c-2, B8c-3; a core implementation of the three waivers' effects (Acquiescence, Jae Mir Kan,
Mindsieve) so a waiver TAKEN no longer bails (`B8c1-bail-01`); the plain events.

Resume: `git -C .claude/worktrees/core-b8c log`; tests: `CARGO_TARGET_DIR=/root/deploytmp/core-b8c CARGO_BUILD_JOBS=2 cargo test -p ti4-engine --lib -- strategy_secondary the_follower_step the_core_follow`.
