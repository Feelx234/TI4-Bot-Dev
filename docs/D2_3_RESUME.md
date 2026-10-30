# D2 lane B resume note (branch `core-d2-3`, cut from `core-integration-5` @ 670963aa)

**Gate deferred to end of D1** (coordinator directive: no heavy work until D1 is done). No heavy run was completed on this branch.
A gate that was queued and had started (`verify_package none`, step 1: layers, lint, clippy, immutable surfaces, builds, part of the debug-profile tests) was killed on that directive.
Everything below was checked with cheap checks only (lockstep tests, mutation runs, one 8-game churn proof).

## What is in the branch

| Package | Done | Where |
|---|---|---|
| D2-3 text tables | `containers::Interner` (state-owned, base `Arc` + local suffix, sorted index, no linear scan), `CoreState.texts` (`codes`, `blobs`), `CoreState.owners` (`OwnerTable`), codecs/keys read and write through the core, `Ctx.texts` is a staging copy (`Ctx::attach`/`adopt`/`minting`), `Ctx::refresh` is an `Unmapped` on a changed owner set, `TextSeeds` owns a clone and debug-asserts a `Dyn` domain has a text, `core_sync_gate` prints the code/blob high-water marks | `ti4-engine-core/src/{containers/interner.rs,state/texts.rs}`, `ti4-engine/src/core_sync/{ctx,texts,lend}.rs`, `codec/*`, `keys/*` |
| D2-4a resolver sync | `resolver_sync.rs` (`ResolverImage`, `export`, `rebuild`, `delta`/`apply`, `churn`), additive `pub(crate)` accessors in `timing.rs`, `EventSequence::{peek, from_next}`, `churn.rs` (`TI4_CHURN=resolver`), the hook at the top of `Game::step`, tests I1-I5 + churn equality | `ti4-engine/src/{resolver_sync,churn,timing,event,game}.rs` |
| D2-4b resolver in state (partial) | `state::ResolverState` in `Pending` (`used` behind a `Cow`), `UsedScope` moved to `state`, `Resolver::{from_state, into_state}`, `Driver::{from_state, store}`, `resolver_sync::{to_state, with_state}` | `ti4-engine-core/src/{state/resolver.rs,kernel/resolver.rs,driver/mod.rs}` |

## Not done (exact next items)

1. **Gate** (when D1 is done): `flock`/`/root/heavy2.sh` + `SHARDS=3`, `SKIP_AB=1 SKIP_WASM=1`, the script used was `/root/deploytmp/core-d2-3-out/gate.sh WORKTREE OUT` (it runs `verify_package.sh none`, the golden dual with `all=dual` and with the wide mask, `board_equivalence` templates + golden, `core_sync_gate --strict`, and the benches with `TI4_CHURN=resolver`). Pass criteria: 0 differences, board_equivalence 0 mismatches, benches 200/200 x6 (also with the churn), clippy clean (the 12 hits of the first run are fixed in 510bde36 but were not re-run by the gate), `core_sync_gate` output equal to the int-5 one except the added high-water line (compare the counts, not the lend nanoseconds).
2. D2-4b: the `Pending` codec does not load/store `pending.resolver` (needs a `resolver` field in `Sources` and its 40 struct-literal sites, files of other lanes). No `Driver::new` site (190) was switched; the dual windows build small resolvers of their own (initiative = the seats involved), so the per-window assertion `state.resolver == export(game.timing)` cannot hold yet and was not written.
3. `render::Render` holds a third text table with a linear lookup (`ENGINE_KNOWN_ISSUES.md` D2-3-open-02).
4. `alpha_roundtrip` (D2-1, lane A) does not exist on this base; the high-water output exists only in `core_sync_gate`.

## Facts the next owner needs

* The string resolver's clock (phase, seating, active, speaker, round, turn) lags the state until the first `sync_timing_context`, and its `initiative_order` differs from the state's later in a game: the image carries all of them (`Context::Image`).
* Random-play games spend no frequency keys (the standing slots are `Unlimited`); `spent_resolver()` in the `resolver_sync` tests builds a resolver that did.
* Results: lockstep `ti4-engine --lib` 4610 passed (int-5: 4589), `ti4-engine-core` 327 + 26 + 3, factions 109, bridge bins/tests compile, `check_layers.py core` 0 violations, `lint_core.sh` 0 hits, `size_of::<CoreState>()` 2600 B (limit 4096), 8-game bench slice (`golden.json` seeds 1..5) 8 identical with `TI4_CHURN=resolver`.
