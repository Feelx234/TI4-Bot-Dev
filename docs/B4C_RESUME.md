# B4c resume note (TradeWindow as a Stage machine, offer_options pricing)

Branch `core-b4-trade` (merged with `core-integration-2` dc2f4255; it was already contained). Gate deferred until the
trade/diplomacy area is complete (user policy of 2026-10-29): NO heavy run has been made on this branch.

## Done
* Core: `choice::trade` (`Shape`, `Ask`, `Opt`), `query::trade_deals` (the option list `deals`, prices `nets` /
  `option_price` / `note_worth` / `note_cost`, `offer_of`), `query::trade::why_illegal_with`, `rules::trade_window::TradeWindow`
  (Window impl: Proposing / Answering / Done, two proposals, counter swaps the chairs, accept = `exchange` + `record_resolved`).
* Adapter: `trade_window_dual.rs` (`step`, `facts_of`, `At`), `TradeWindow::answer_with` (a `dual_window!` per string step,
  used by `Game::step_trade`), `render/trade.rs` (`deal_option`, `shape_of`), `transactions::offer_from_with`,
  `Offer::describe_as`. Removed: interned option shapes of `Render`, `trade_core::typed_choice` / `rendered`.
* Tests: `crates/ti4-engine/src/trade_window_tests.rs` (24 tests, census of random tables), core unit tests in
  `query/trade_deals.rs`. Mutation check: 73 mutants, 68 killed, 5 justified survivors (ENGINE_KNOWN_ISSUES B4c-design-07).
* ENGINE_KNOWN_ISSUES.md (core-integration, commit cdc2ef77): B4c-design-01..07, B4c-bail-01..05, B4c-test-01, B4c-census.

## Checks that ran (private target dir, CARGO_BUILD_JOBS=2)
`cargo check` of the touched crates; `cargo test -p ti4-engine --lib` (4220 ok); `-p ti4-engine --tests` and `--doc`;
`-p ti4-engine-core` (141 ok); `decision_delivery_inventory`; `check_layers core` 0 violations; `lint_core` 0 hits.

## Not done / not run
* Not in this block (the prereq doc puts them out of scope): `DiplomacyWindow`, the deal book (create_deal, add_counter,
  archive_deal, apply_acceptance), signals, the promise ledger, planet-card deals (Pride of Kenara `kp`/`kq` shapes). The window
  bails when any of them is live (diplomacy on, Kenara).
* The area gate: `verify_package.sh trade`, golden dual (driver=dual), `board_equivalence`, benches, `trade_dual`, workspace
  clippy. `crates/ti4-engine-core-bridge/src/bin/trade_dual.rs` still drives `TradeWindow::resolve` directly; when the area is
  complete, change it to `answer_with` so the gate's fuzz reaches the window dual.

## To resume (when the whole trade/diplomacy area is done)
1. Merge `core-dual-shards` (bdbe148d) into this branch.
2. `flock /root/.ti4-heavy.lock env CARGO_TARGET_DIR=<private> SKIP_AB=1 SKIP_WASM=1 SHARDS=4 tools/engine_core/verify_package.sh trade <out> <base selfplay>`
   (profile gate; the golden dual runs the shards inside the lock).
3. Rerun only the failed steps.

## Open decisions
* A free-form deal stays on the string (B4c-bail-01); a typed free-form `Opt` would make `Opt` larger than 16 bytes.
