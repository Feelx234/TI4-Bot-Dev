# core-integration-23 (2026-11-04)

Branch `core-integration-23`, cut from `core-integration-22` @ `946e77549`. Not pushed. Order changed by the coordinator: deploy to the public alpha right after the fast checks, the rest of the gate afterwards on the deployed commit (section 3, filled in at the end).

## 1. Merges

| # | Branch | Tip | Conflicts and joins |
|---|---|---|---|
| a | `core-d29b-strip` | `5b9ad0b05` | one textual conflict: `ti4-engine-next/src/lib.rs` `mod objectives` re-export. Kept int-22's list (`family`, `cost_progress_of` included), dropped 9b's removed legacy glob (`pub use ti4_engine_legacy::objectives::*`). Cargo.lock/Cargo.toml/config merged by git. `cargo check --workspace --all-targets` (no torch crates) green with no further join: the d3-view code (seen_view.rs, bot_shadow, bridge shadow.rs) needed no change. `check_production_graph.sh` strict 0 violations, 0 failed controls; `check_layers.py core` 574 files 0, `thin` 191 files 0. |
| b | `core-golden45` | `e5fc88daf` | none (test only); `host_create_golden` passes |
| c | `core-yin-nexus` | `d7ff8d403` | none; `action_dispatch dannel` 7 pass |

Join fix: `tools/engine_core/surface_paths.baseline.json` gained `strategy::STRATEGIC_ACTION_ID` (native, 0 legacy) after int-22's d3-view code met 9b's surface.

## 2. Deploy (before the full gate, by the coordinator's order)
Commit deployed: `54a15dba` (docs-only commits came after: the tip of the branch differs only in docs/ and this file). Tag `pre-flip-live` created on `66f49724b` (the live branch `nightly-fixes-2026-10-26`, not moved). Nothing pushed.

* `deploy.sh --ref core-integration-23 --preflight-only`: production graph strict ok, output probes ok (server binary and wasm: no legacy literal), preflight on a copy of the live data: 24 saved games, recovered 10, failed 0, refused 14 (all "saved with an older version of the rules (version 0); only 35 or newer"), no `--allow-failed`. Release `20261104T145634Z-54a15dba`, `previous` = `20261025T101143Z-48698d59`.
* Real deploy: health `{"status":"ok","protocol_version":4,"commit":"54a15dba","public":true}`, service active, journal "recovered 10 game(s); 0 failed; 14 refused (saved files retained)", lobby page HTTP 200.
* Loopback API checks: `POST /api/games` with `thunders_edge: true` gives HTTP 400 "Thunder's Edge content (the expedition and the breakthroughs) is switched off in this version; the Thunder's Edge strategy cards can still be played"; a game with `strategy_card_set: "te"` is created, a guest joins, it starts; a two-seat websocket script accepted 150 decisions, 0 rejected, no errors, no panics in the journal.
* Rollback: `deploy/public/deploy.sh rollback`.

## 3. Gate on the deployed commit (all green)
| check | result |
|---|---|
| quick game with shadow | 1,165 steps, fingerprint `74d5675ac754ac81`, shadow compared 808 |
| plain 96-set / 288-set | 96/96, 288/288, exit 0 |
| Z-gate 3/4/5/6p (192 games each, shadow) | all exit 0, zero gaps; known string bugs 20 / 38 / 34 / 59 = 151 (int-22: 152; the one S4 game is gone with the yin-nexus ruling) |
| unit suites | host 320, rule-bot 190, policy 287, wasm 9, thin ti4-engine 433 (all targets), bridge lib 142, core 511, factions 154, legacy lib 5,321, selfplay 12, rule-bot-typed 1: all green |
| D4 crate | 156 binaries green one at a time, `wrapper_props` (feature `wrapper`) 6 of 6 |
| ti4-server `tests/it` | all 76 modules green, 357 tests (gate profile, `--test-threads=2`) |
| checks | check_layers core 574 files 0 / thin 191 files 0, lint_core 0, clippy_core engine 41 = baseline, surface_paths --check ok after the baseline line, production graph strict 0/0, wasm32 check ok, output probes ok on release server and wasm (also on the installed files) |
| wire TS / fixtures | no diff in web-shared generated TS or fixtures (ran the wire tests with `TI4_WRITE_WIRE=1`: the `cargo wire-web` alias itself fails here with "Unrecognized option: 'config'", an alias/cargo-version quirk, not a join bug) |
| host_create_golden | passes with the golden45 handling |

## 4. Bugs
No join bug beyond the one-line `lib.rs` re-export conflict and the surface_paths baseline line. No string-engine bug new.

## 5. Open
Raw wasm 7.82 MB over the 7.0 MB budget (gzip/brotli fine; 9b). Torch crates not built. Heavy gate (mutation ratchet, benches, soak, census, workspace clippy) not run. Alias `cargo wire-web` fails with this cargo (see above).
