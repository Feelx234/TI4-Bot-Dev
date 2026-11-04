# core-integration-23 (2026-11-04)

Branch `core-integration-23`, cut from `core-integration-22` @ `946e77549`. Not pushed. Order changed by the coordinator: deploy to the public alpha right after the fast checks, the rest of the gate afterwards on the deployed commit (section 3, filled in at the end).

## 1. Merges

| # | Branch | Tip | Conflicts and joins |
|---|---|---|---|
| a | `core-d29b-strip` | `5b9ad0b05` | one textual conflict: `ti4-engine-next/src/lib.rs` `mod objectives` re-export. Kept int-22's list (`family`, `cost_progress_of` included), dropped 9b's removed legacy glob (`pub use ti4_engine_legacy::objectives::*`). Cargo.lock/Cargo.toml/config merged by git. `cargo check --workspace --all-targets` (no torch crates) green with no further join: the d3-view code (seen_view.rs, bot_shadow, bridge shadow.rs) needed no change. `check_production_graph.sh` strict 0 violations, 0 failed controls; `check_layers.py core` 574 files 0, `thin` 191 files 0. |
| b | `core-golden45` | `e5fc88daf` | none (test only); `host_create_golden` passes |
| c | `core-yin-nexus` | `d7ff8d403` | none; `action_dispatch dannel` 7 pass |

Join fix: `tools/engine_core/surface_paths.baseline.json` gained `strategy::STRATEGIC_ACTION_ID` (native, 0 legacy) after int-22's d3-view code met 9b's surface.

## 2. Fast checks before the deploy
See section 3 / the final report for the numbers. Quick game: 1,165 steps, fingerprint `74d5675ac754ac81`, shadow compared 808. Plain 96-set 96/96 (zero gaps), 288-set 288/288. wasm32 check, `lint_core`, `clippy_core` (engine 41 = baseline) green. `surface_paths --check` was red once (the baseline line above) and is green after it.

Known open (from 9b): raw wasm size 7.82 MB is over the 7.0 MB budget (gzip and brotli within it); `size-budget.json` not ratcheted.
