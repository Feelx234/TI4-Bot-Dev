# D2-9 lane 9b resume: production strip (thin ti4-engine without ti4-engine-legacy)

Branch `core-d29b-strip`, cut from `core-integration-21` (4a6cc5200) with `core-d29a-graph` and `core-d29d-aliases` merged. Not pushed.

## Done
- Thin crate (`crates/ti4-engine-next`): facade glob and legacy shells removed; legacy optional behind `lend-string` (default `[]`); `typed-core`, `native-subject`, `observed-count`, `string-*` imply `lend-string`; new `production` feature (host, server, wasm, rule-bot edges); `lend_string` module (`legacy`, `from_legacy_wire`, `product_init_legacy`).
- `compile_error!` for lend-string + production only in release builds (debug unification of `cargo check --workspace` must stay green).
- Native: `fleet::{over_supply, over_capacity}`, `supply::staging_enabled`, `factions::flagship_has_text`, `queries::objectives::has_requirement`, `technology::researchable_by_prerequisites` (setup), `invasion::CUSTODIANS_COST`, turn-clear writes in `game/mod.rs`.
- Consumers: host/server/rule-bot name legacy as dev-dependency; policy/sim/training/bridge name it directly (sim, training also `lend-string`); oracle adapter now in the bridge (`legacy_adapter.rs`) and copied into server tests; ti4-calc test uses native `KELERES_VARIANTS`.
- Scripts: `check_production_graph.sh` allows a direct legacy dev-dependency of the checked crate only; strict is the default in verify_package/verify_stage/end_of_d1/deploy; `check_layers.py thin` wired into verify_package and the end_of_d1 layers step; lint step excludes RULES_VERSION.md except refusal hunks.
- Tests: `crates/ti4-engine-next/tests/strip_parity.rs` (5 parity tests against legacy).
- Punch list: 26 at int-20 -> 0 strict, 0 failed controls; `check_layers.py thin` 0.

## Remaining
See the final report of the lane; this file is updated at each commit.

## Join notes (integration 22)
Expected conflicts: `Cargo.lock`; thin `lib.rs` (glob removal, shells); `.cargo/config.toml`; Cargo.toml of host, server, rule-bot, wasm, policy, sim, training, thin. Resolve by keeping the 9b side for dependency edges and features and re-running `check_production_graph.sh`.

## Not built here
Torch crates (review, mlp, replayer, advisor, tensor): unbuilt, need a machine with torch.
