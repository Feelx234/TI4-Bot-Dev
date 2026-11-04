# Engine layout (after D2-9 lane 9d)

## Crates

| Directory | Package | Role |
|---|---|---|
| `crates/ti4-engine-next` | `ti4-engine` | The thin, product-facing engine (host, server, wasm, rule-bot depend on it). |
| `crates/ti4-engine` | `ti4-engine-legacy` | The old string engine. Directory name kept on purpose; dev/oracle crate, kept until D4, never in the production set. Do not confuse with the stub `crates/ti4-legacy`. |
| `crates/ti4-engine-core` | `ti4-engine-core` | Typed core: no JSON, no id strings; generated ids (`cargo xtask gen-cards`), state, kernel, rules. |
| `crates/ti4-engine-core-factions` | `ti4-engine-core-factions` | Layer L6: faction modules filling the kernel's hook table. Depends on the core only. |
| `crates/ti4-engine-core-bridge` | `ti4-engine-core-bridge` | Adapter string ids <-> typed core, oracle/dual/golden checks, `bot_shadow`. Links legacy. `crates/ti4-bridge` is a separate small crate on the thin engine. |
| `crates/ti4-engine-core-tests` | `ti4-engine-core-tests` | D4 kit: oracle-free typed tests (Scenario, Rig, delta, covers!). Test-only. |

Layers: model/content/calc -> core (L0..L5) -> factions (L6) -> thin `ti4-engine` -> host -> server/wasm/rule-bot. Legacy and the bridge sit beside this stack as oracle and tooling.

## Where to run what

| Command | What |
|---|---|
| `cargo l-web` (`c-web`) | clippy/check of the production set (no legacy, no libtorch) |
| `cargo t-web [filter]` | fast tests of the production set; `cargo t-web-slow` the `slow_` tier (opt-level 1 on core, factions, thin engine and the game crates) |
| `cargo b-web` | `target/debug/server` for e2e |
| `cargo wire-web` | regenerate wire TS types and fixtures |
| `cargo t-legacy` | legacy's own tests (oracle, until D4) |
| `cargo c-dev` / `cargo t-dev` | dev set: legacy, core, factions, core-bridge, core-tests (one fixed package set, never mixed with the web aliases) |

## Nightly restore (to do when the cron is restored)

User decision 2026-11-04: the nightly cron is restored at 22:30 Berlin. CLAUDE.md says 20:30 and must be updated at restore time (lines 31 and 33). Every place that says 20:30 or 23:30:

- `CLAUDE.md` (20:30, lines 31, 33)
- `tools/nightly-smoke/config.sh:15` (`START_HHMM` default 20:30), `nightly.sh` (header line 2, line 918), `prewarm.sh:5`
- `tools/nightly-smoke/README.md` lines 13, 25, 28, 240, 345, 432
- `tools/nightly-smoke/test_schedule.sh`, `test_slots.sh`, `test_offline.sh` (test times built on 20:30 / 23:30)
- crontab backup `/root/crontab.backup-2026-10-27` (`NIGHTLY_START=23:30`, currently `#SKIPPED-2026-10-27`)
- docs: `docs/ENGINE_KNOWN_ISSUES.md` (A3 row, E2), `docs/ENGINE_DISENTANGLE.md:467`, `docs/PP_MAIN_INTEGRATION_PLAN.md:187`
