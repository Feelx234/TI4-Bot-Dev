# Wave B (BF-00c-space, BF-00c-ground, BF-00b-economy) — integration record

Implementers: three Sonnet subagents in parallel (disjoint files). Coordinator: Claude Code (Opus).
Reviews: Opus tier C/D review (4 blockers, 7 should-fix) → fixes → Opus verification (no blockers;
2 new should-fix + nit, fixed by the coordinator). Per-package detail in the three evidence files.

## Checks on the committed tree

| Command | Result |
|---|---|
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1457 passed, 1 ignored; all integration binaries ok (`decision_delivery_inventory` 4/4) |
| `LIBTORCH=out/libtorch-2.9.1-cpu cargo test -p ti4-sim -q` | 51 passed, 1 ignored — authored-bot behaviour unchanged |
| `cargo check --workspace --all-targets` | ok |
| ti4-policy / ti4-review / ti4-bridge / ti4-legacy / ti4-cli tests | all pass except `ti4-review --test semantic_golden` (below) |
| `cargo clippy -p ti4-engine --all-targets` | no new warnings (pre-existing ones unchanged) |

## Pre-existing failure, not caused by this work

`ti4-review --test semantic_golden` (`the_reviewers_default_path_matches_the_frozen_pre_r02_golden`)
differs at frame 8: Xxcha's first action menu now offers "open a transaction with hacan". The golden
was regenerated 2026-09-21 (`d878701b`/`1c4a86a5`); `52066efa` (2026-09-23, "Guild Ships works both
ways when a contact opens") made `transactions::available_actions` use `may_transact`, which offers
exactly this option. So `main` has failed this test since 2026-09-23. Not regenerated: that needs
operator approval and the golden's documented process.

## Known gaps carried forward

Typed status-phase events (needs `game.rs`, another session's uncommitted edits); status/agenda AC
draws bypass `action_cards::draw`; `TRADE_GOODS_GAINED` call sites (31 sites); invasion as a leader
effect; `GROUND_FORCE_DESTROYED` covers invasion paths only; `htp` state field; `mc` hard-code moves
in the Mentak package.
