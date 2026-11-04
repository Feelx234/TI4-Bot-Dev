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

## Results (gate profile for binaries, private targets)
| check | result |
|---|---|
| `check_production_graph.sh` strict / `--self-test` / canary (lend-string on the server edge) | 0 violations, 0 failed controls / ok / red (6) |
| `check_layers.py core` / `thin`, `lint_core.sh`, lint step vs int-21 | 0 / 0 / 0 hits / exit 0 (RULES_VERSION.md rule checked both ways) |
| `cargo check --workspace --all-targets` (no torch), wasm32 check of ti4-wasm and ti4-host, `c-web`, `c-dev` | ok |
| thin `ti4-engine --tests` (12 binaries) | all green (lib 365, strip_parity 6 incl. relic/law waivers) |
| unit suites: host 310, rule-bot 190, wasm 9, bridge 141; `t-legacy` (legacy 5,302 + 4 bins) | green |
| server `it`: crash_recovery, replay_export, secondary_preview, bluff_reaction, session_deterministic_replay (gate) | 32 passed |
| `native_play --quick` | 1,165 steps, fingerprint 74d5675ac754ac81, zero gaps (also with `--shadow`) |
| plain 96-set | 96/96 zero gaps, exit 0 |
| Z-gate `--factions all --players P --seeds 8 --shadow` | 3p: 192 games, 20 known string bugs, exit 0; 4p: 192, 39, exit 0; 5p/6p see the final report |
| `cargo wire-web` | generated TS in `web-shared` unchanged. It rewrites `fixtures/protocol/*_snapshot.json` (drops objective_progress/home_system_lost): the known int-21 Unmapped fixture issue (INTEGRATION21 section 5), reverted, not caused by 9b |
| `host_create_golden` | 45 of 1405 rows differ (production_batch, production_payment_batch, research_tech_skips, tactical_action presets): IDENTICAL on core-integration-21 (checked in a worktree), so pre-existing; the golden file is untouched |
| output probes | server release binary and wasm: no legacy literal (the `generated` module path does not trip them). int-21 wasm: 99 hits |

## wasm size (cargo output, `wasm-release`, no wasm-opt available here)
| | raw | gzip -9 | brotli -11 |
|---|---|---|---|
| int-21 (legacy linked) | 13,079,485 | 3,483,021 | 2,342,145 |
| 9b (no legacy) | 7,817,249 | 2,157,541 | 1,508,662 |
Budget 7.0 MB / 2.4 MB / 1.8 MB: gzip and brotli ok, RAW 7.82 MB is OVER. `size-budget.json` NOT ratcheted (rule: only when within budget); the raw budget needs a decision (the budget was set against the wasm-opt'ed file; wasm-opt is not installed here). Build: 1:25 wall, 1.9 GB RSS.

## Remaining
* Torch crates (review, mlp, replayer, advisor, tensor) were rewired by reading and are unbuilt: they need a machine with torch.
* Raw wasm budget (above).
* Heavy gates (G7) not run.

## Join notes (integration 22)
Expected conflicts: `Cargo.lock`; thin `lib.rs` (glob removal, shells); `.cargo/config.toml`; Cargo.toml of host, server, rule-bot, wasm, policy, sim, training, thin. Resolve by keeping the 9b side for dependency edges and features and re-running `check_production_graph.sh`.

## Not built here
Torch crates (review, mlp, replayer, advisor, tensor): unbuilt, need a machine with torch.
