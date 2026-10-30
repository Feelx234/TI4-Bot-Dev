# Lent bodies, lane 3 (branch `core-w-l3`, from `core-w` @ 4808a0ab)

Packages: W9 ability-table tail, W10 leaders, W12 technology, W13 exploration/relics, W14 promissory, W15 free-form deals, W11 faction component actions.

## Gate deferred to end of D1

By the user's directive no heavy work ran: no `verify_package`/gate, no golden or wide dual run, no census re-run, no bench, no soak, no
mutation ratchet, no clippy. Gate deferred to end of D1. What ran: `cargo test -p ti4-engine-core -p ti4-engine-core-factions`,
`cargo test -p ti4-engine --lib` (full, at the first commit: 4605 passed, 0 failed), targeted tests, `check_layers.py core` and
`lint_core.sh` (0 hits), single-file mutation checks (`/root/deploytmp/core-w-l3/mut*.sh`, scratch worktree, Haiku runners).

## Done (details in ENGINE_KNOWN_ISSUES.md, lines W10-fix-01, W9-design-01, W13-design-01, W14-design-01 and later ones)

See the report of the lane; this file lists state only.
