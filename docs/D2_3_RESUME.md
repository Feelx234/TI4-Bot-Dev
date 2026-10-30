# D2 lane B resume note (branch `core-d2-3`, cut from `core-integration-5` @ 670963aa)

**Gate deferred to end of D1** (coordinator directive: no heavy work until D1 is done). No heavy run was completed on this branch.
A gate that was queued and had started (`verify_package none`, step 1: layers, lint, clippy, immutable surfaces, builds, part of the debug-profile tests) was killed on that directive.
Everything below was checked with cheap checks only (lockstep tests, mutation runs, one 8-game churn proof).

## What is in the branch

| Package | Done | Where |
|---|---|---|
| D2-3 text tables | `containers::Interner` (state-owned, base `Arc` + local suffix, sorted index, no linear scan), `CoreState.texts` (`codes`, `blobs`), `CoreState.owners` (`OwnerTable`), codecs/keys read and write through the core, `Ctx.texts` is a staging copy (`Ctx::attach`/`adopt`/`minting`), `Ctx::refresh` is an `Unmapped` on a changed owner set, `TextSeeds` owns a clone and debug-asserts a `Dyn` domain has a text, `core_sync_gate` prints the code/blob high-water marks | `ti4-engine-core/src/{containers/interner.rs,state/texts.rs}`, `ti4-engine/src/core_sync/{ctx,texts,lend}.rs`, `codec/*`, `keys/*` |
| D2-4a resolver sync | `resolver_sync.rs` (`ResolverImage`, `export`, `rebuild`, `delta`/`apply`, `churn`), additive `pub(crate)` accessors in `timing.rs`, `EventSequence::{peek, from_next}`, `churn.rs` (`TI4_CHURN=resolver`), the hook at the top of `Game::step`, tests I1-I5 + churn equality | `ti4-engine/src/{resolver_sync,churn,timing,event,game}.rs` |
| D2-4b resolver in state | `state::ResolverState` in `Pending` (`used` behind a `Cow`), `UsedScope` moved to `state`, `Resolver::{from_state, into_state}`, `Driver::{from_state, store}`, `Ctx::resolver` as the in/out channel of the `Pending` codec, `resolver_sync::{to_state, with_state, cfg_state, rebuild_for_lend}`, `reactions_core::resolver_of` built from the state, `dual_window::compare_resolver` (per-window `resolver.state` comparison: `Recording::{after, finished, failed_emit}`, `note_resolver_after`); `Render.codes` on the shared `Interner` (decision: window-local until D2-8, `with_string` builds its `Render` from the state's table) | `ti4-engine-core/src/{state/resolver.rs,kernel/resolver.rs,driver/mod.rs}`, `ti4-engine/src/{resolver_sync,reactions_core,dual_window,timing,render/mod}.rs`, `core_sync/{ctx,codec/pending}.rs` |
| D2-5 reverse lend (as far as cheap checks go) | `core_sync/materialize.rs` (`materialize`, `rng_positions`, `written_outside`, `slice_of_field`), `reverse_lend.rs` (`with_string`, `StringCtx`, `StringTape`, `Lent`, footprint audit incl. streams and resolver), `lend_table.rs` + `TI4_BAIL_PROBE=FILE` in `run_dual_window` + bin `bail_table`, `TI4_CHURN=state` (`Game::churn_state`) | `ti4-engine/src/{reverse_lend,lend_table,churn,game,dual}.rs`, `ti4-engine-core-bridge/src/bin/bail_table.rs` |

## Not done (exact next items)

1. **Gate** (when D1 is done): `/root/heavy2.sh` + `SHARDS=3`, `SKIP_AB=1 SKIP_WASM=1`, the script used was `/root/deploytmp/core-d2-3-out/gate.sh WORKTREE OUT` (it runs `verify_package.sh none`, the golden dual with `all=dual` and with the wide mask, `board_equivalence` templates + golden, `core_sync_gate --strict`, and the benches with `TI4_CHURN=resolver`; add the benches with `TI4_CHURN=state` and `resolver,state`). Pass criteria: 0 differences (the new `resolver.state` label included), board_equivalence 0 mismatches, benches 200/200 x6 (also under both churn modes), clippy clean, `core_sync_gate` output equal to the int-5 one except the added high-water line (compare the counts, not the lend nanoseconds).
2. **Bail table**: run the sharded golden dual with `TI4_BAIL_PROBE=FILE`, `cargo run -p ti4-engine-core-bridge --bin bail_table FILE..`, paste the rows into `lend_table::BAILS`, add `ids`/`test`, widen writes by hand; then wire `missing_entries`/`stale_entries` to the real `DualReport.windows` as the two tests of D2-5 item 5 and write one `with_string` wrapper per label (windows with plain-data state first; labels whose state is a nested `Game` field are D1b's).
3. D2-8 owns: `Game::with_string` / `Game::materialize()` and its `OnceLock` cache around `reverse_lend::with_string`, the remaining ~190 `Driver::new` sites (their resolvers have an empty registry today, so the initiative order is unobservable there), the `Render` borrowing the game's `CoreState::texts.codes`.
4. `alpha_roundtrip` (D2-1, lane A) does not exist on this base; the high-water output exists only in `core_sync_gate`.

## Facts the next owner needs

* The string resolver's clock (phase, seating, active, speaker, round, turn) lags the state until the first `sync_timing_context`, and its `initiative_order` differs from the state's later in a game: the image carries all of them (`Context::Image`).
* Random-play games spend no frequency keys (the standing slots are `Unlimited`); `spent_resolver()` in the `resolver_sync` tests builds a resolver that did.
* Results: lockstep `ti4-engine --lib` 4610 passed (int-5: 4589), `ti4-engine-core` 327 + 26 + 3, factions 109, bridge bins/tests compile, `check_layers.py core` 0 violations, `lint_core.sh` 0 hits, `size_of::<CoreState>()` 2600 B (limit 4096), 8-game bench slice (`golden.json` seeds 1..5) 8 identical with `TI4_CHURN=resolver`.
