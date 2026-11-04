# D2-9 lane 9a: production graph checks, output probes, wiring (2026-11-04)

Branch `core-d29a-graph`, cut from `core-integration-20` (b8e43ef8d). Plan: `docs/todo/D2-9-strip.md` on `core-todo-d29` (sections 2.1-2.4, 4.3).
No Rust source and no Cargo.toml was edited. Everything here is `cargo tree` / `cargo metadata` / `grep`: no build.

## 1. What is new

| file | what |
|---|---|
| `tools/engine_core/check_production_graph.sh` | `[--report \| --self-test \| --root DIR]`. Strict (default) exits 1 on any violation; `--report` prints them and exits 0; `--self-test` builds a synthetic 7-crate workspace in a temp dir and proves the checker catches a feature smuggle (normal and dev edge), a direct legacy dependency, a dev tool listed as production, a dev tool in an alias set, `--workspace` in deploy, and passes a clean crate. |
| `tools/engine_core/check_output_probes.sh` | `[--allow-legacy-probe] BINARY... \| --self-test`. Greps the built server / wasm for the legacy env-var literals (`TI4_BAIL_PROBE TI4_DUAL_FAIL_FAST TI4_AREAS TI4_CHURN TI4_LEND_PROOF`), `ti4_engine_legacy`, `reverse_lend`, `lend_table`, `lend-string` and the panic path `crates/ti4-engine/src/` (with the slash: the thin crate is `crates/ti4-engine-next/`). Missing/empty file always fails, even with the override. |
| `tools/engine_core/check_layers.py thin` | new mode: `crates/ti4-engine-next/src` may name `ti4_engine_legacy` only in module `lend_string` (declared `#[cfg(feature = "lend-string")]`); `tests*.rs` files skipped. Exit 1 on a violation. NOT wired yet (red until lane 9b), see section 3. |

What `check_production_graph.sh` checks, per production crate (`ti4-host ti4-server ti4-wasm ti4-rule-bot ti4-model ti4-content ti4-calc ti4-engine`, override with `PROD_CRATES`; `ti4-wasm` again with `--target wasm32-unknown-unknown`):
1. `cargo tree -e normal,build,features` and the same with `dev` added: no `ti4-engine-legacy`, no `lend-string` (a dev-dependency cannot smuggle it in). The first inverted path (`-i ti4-engine-legacy`) is printed for each reach.
2. the thin crate `ti4-engine` as resolved for the crate carries none of `typed-core native-subject observed-count string-* lend-string`.
3. `ti4-engine-core-bridge` / `ti4-engine-core-tests` are not in the tree.
4. static: the `*-web` aliases of `.cargo/config.toml` list no dev-tool crate (`DEV_TOOLS` list inside the script) and no `ti4-engine-legacy` (also the `t-web-slow` profile key); `lend-string` occurs only in the Cargo.toml allow-list (`ALLOW_LEND`); the thin crate's `default` features do not reach legacy/lend-string; `deploy/`, `web-shared/scripts`, `scripts/` contain no `--workspace` / `lend-string`.
5. negative controls, always run (also in `--report`; a failed control exits 1 in every mode): the bridge MUST reach legacy; `ti4-engine --features lend-string` MUST (only once that feature exists); `cargo tree -i <missing>` MUST still say `did not match any packages` (cargo 1.94.1 here).

Cargo-tree facts learned: `-i` on a package outside the graph is an error text (exit 101), not an empty success; `-e dev` alone prints "nothing to print" for an inverted query, so the script always uses `normal,build,dev`.

## 2. Current-state inventory (`check_production_graph.sh --report`, core-integration-20)

Summary: 26 violations, 0 failed controls. Reading:
* `ti4-model`, `ti4-content`: clean (no edge to `ti4-engine*`).
* `ti4-host`, `ti4-server`, `ti4-wasm` (also for wasm32), `ti4-rule-bot`, `ti4-engine`: reach `ti4-engine-legacy` through the single edge `ti4-engine (crates/ti4-engine-next) -> ti4-engine-legacy` (non-optional); the thin crate carries `typed-core` (its only default feature).
* `ti4-calc`: normal tree clean; its **dev** tree reaches legacy (dev-dep `ti4-engine`).
* No `lend-string` anywhere yet (the feature does not exist); the bridge and tests crates are in no production tree.
* Aliases `c-web l-web t-web t-web-slow wire-web b-web` list `ti4-engine-legacy`; no dev-tool crate is in any alias set (good).
* `crates/ti4-engine-next/Cargo.toml`: default `['typed-core']` reaches legacy.
* deploy/build scripts: no `--workspace` / `lend-string`.
* `check_layers.py thin`: 4 files with legacy references outside a gated module (`lib.rs` 32, `product.rs` 1, `product_init.rs` 1, `core_sync/unsupported.rs` 1).
* Output probes on the last alpha build (`/root/.cache/ti4-public-target`, release server and `ti4_wasm.wasm`): only `crates/ti4-engine/src/` panic paths hit (server x66, wasm x71); the env-var literals are not in those builds (so TI4_* are read from a different build/profile or already absent there: the probe still guards them).

Full report output:

```text
VIOLATION ti4-host [normal,build]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
    via ti4-host v0.1.0 (crates/ti4-host)
VIOLATION ti4-host [normal,build,dev]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
    via ti4-host v0.1.0 (crates/ti4-host)
VIOLATION ti4-host [normal,build]: thin crate ti4-engine carries feature "typed-core" 
VIOLATION ti4-server [normal,build]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
    via ti4-host v0.1.0 (crates/ti4-host)
    via ti4-server v0.1.0 (crates/ti4-server)
    via ti4-rule-bot v0.1.0 (crates/ti4-rule-bot)
    via ti4-server v0.1.0 (crates/ti4-server)
VIOLATION ti4-server [normal,build,dev]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
    via ti4-host v0.1.0 (crates/ti4-host)
    via ti4-server v0.1.0 (crates/ti4-server)
    via ti4-wasm v0.1.0 (crates/ti4-wasm)
    via ti4-server v0.1.0 (crates/ti4-server)
VIOLATION ti4-server [normal,build]: thin crate ti4-engine carries feature "typed-core" 
VIOLATION ti4-wasm [normal,build]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
    via ti4-host v0.1.0 (crates/ti4-host)
    via ti4-wasm v0.1.0 (crates/ti4-wasm)
    via ti4-rule-bot v0.1.0 (crates/ti4-rule-bot)
    via ti4-wasm v0.1.0 (crates/ti4-wasm)
VIOLATION ti4-wasm [normal,build,dev]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
    via ti4-host v0.1.0 (crates/ti4-host)
    via ti4-server v0.1.0 (crates/ti4-server)
    via ti4-wasm v0.1.0 (crates/ti4-wasm)
    via ti4-wasm v0.1.0 (crates/ti4-wasm)
VIOLATION ti4-wasm [normal,build]: thin crate ti4-engine carries feature "typed-core" 
VIOLATION ti4-rule-bot [normal,build]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
    via ti4-rule-bot v0.1.0 (crates/ti4-rule-bot)
VIOLATION ti4-rule-bot [normal,build,dev]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
    via ti4-rule-bot v0.1.0 (crates/ti4-rule-bot)
VIOLATION ti4-rule-bot [normal,build]: thin crate ti4-engine carries feature "typed-core" 
VIOLATION ti4-calc [normal,build,dev]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
    via ti4-calc v0.1.0 (crates/ti4-calc)
VIOLATION ti4-engine [normal,build]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
VIOLATION ti4-engine [normal,build,dev]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
    via ti4-host v0.1.0 (crates/ti4-host)
    via ti4-engine v0.1.0 (crates/ti4-engine-next) (*)
VIOLATION ti4-engine [normal,build]: thin crate ti4-engine carries feature "typed-core" 
VIOLATION ti4-wasm (--target wasm32-unknown-unknown) [normal,build]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
    via ti4-host v0.1.0 (crates/ti4-host)
    via ti4-wasm v0.1.0 (crates/ti4-wasm)
    via ti4-rule-bot v0.1.0 (crates/ti4-rule-bot)
    via ti4-wasm v0.1.0 (crates/ti4-wasm)
VIOLATION ti4-wasm (--target wasm32-unknown-unknown) [normal,build,dev]: reaches legacy / lend-string
    via ti4-engine-legacy v0.1.0 (crates/ti4-engine)
    via ti4-engine v0.1.0 (crates/ti4-engine-next)
    via ti4-host v0.1.0 (crates/ti4-host)
    via ti4-server v0.1.0 (crates/ti4-server)
    via ti4-wasm v0.1.0 (crates/ti4-wasm)
    via ti4-wasm v0.1.0 (crates/ti4-wasm)
VIOLATION ti4-wasm (--target wasm32-unknown-unknown) [normal,build]: thin crate ti4-engine carries feature "typed-core" 
VIOLATION alias c-web: lists ti4-engine-legacy (lane 9d drops it)
VIOLATION alias l-web: lists ti4-engine-legacy (lane 9d drops it)
VIOLATION alias t-web: lists ti4-engine-legacy (lane 9d drops it)
VIOLATION alias t-web-slow: profile key ti4-engine-legacy (lane 9d renames it)
VIOLATION alias wire-web: lists ti4-engine-legacy (lane 9d drops it)
VIOLATION alias b-web: lists ti4-engine-legacy (lane 9d drops it)
VIOLATION crates/ti4-engine-next/Cargo.toml: default features ['typed-core'] reach legacy / lend-string
VIOLATION deploy/build scripts use --workspace or lend-string:
    deploy/public/deploy.sh:182:  say "Production graph check (no legacy engine, no lend-string)"
production graph: 27 violation(s), 0 failed control(s) [mode report]
```

## 3. Wiring (already applied where purely additive and guarded by the script existing)

All wired steps default to **report mode** because the strict mode is red until lane 9b (the thin crate drops its legacy edge). Lane 9b's merge flips them (section 4).

| file | edit (applied) |
|---|---|
| `tools/engine_core/verify_package.sh` | after `lint-core`: `check prod-graph tools/engine_core/check_production_graph.sh ${PROD_GRAPH_MODE---report}` and a summary note; guarded by `[[ -x ... ]]`. |
| `tools/engine_core/verify_stage.sh` | after `cd "$repo"`: the same one-line `check prod-graph`, same guard. |
| `tools/engine_core/end_of_d1.sh` | new step `prod-graph` (before `lint`): both `--self-test`s, then the graph check in `${PROD_GRAPH_MODE---report}`; guarded. G7 row 9. |
| `deploy/public/deploy.sh` | new flag `--allow-legacy-probe` (the override the user accepted). Before the server build: graph check from the deployed worktree `$WT`. After the build: `check_output_probes.sh` on `$CARGO_TARGET_DIR/release/server` and every `web/dist/*.wasm`. `D29A_MODE` (default report): report prints a summary and never blocks (probes then run with the override); `D29A_MODE=strict` makes the graph check abort before the build and the probes abort before install unless `--allow-legacy-probe` was passed. Guarded by the scripts existing in `$WT`. |

Not applied (needs the edits of other lanes, do them in 9b/9d):
* **`check_layers.py`**: the new mode `thin` exists. Wire it in `verify_package.sh` next to `layers`: `check layers-thin python3 -I tools/engine_core/check_layers.py thin` once `crates/ti4-engine-next/src` is clean (lane 9b), and add it to the `layers` step of `end_of_d1.sh` (`... check_layers.py core && ... thin`).
* **G7 row 9** (hygiene) text: `check_layers.py core` and `thin`, `lint_core.sh`, clippy, `check_production_graph.sh` (strict, with its negative controls via `--self-test`), `check_output_probes.sh --self-test` and the probes on the freshly built server binary and `ti4_wasm.wasm`.
* **G7 row 2** rewording: the immutable-surface gate `git diff --exit-code <int-7> -- web/ docs/RULES_VERSION.md crates/ti4-model/src/state.rs 'crates/ti4-wasm/**/fixtures'` conflicts with D2-9 editing `docs/RULES_VERSION.md` (the "versions below MIN are refused" note). Reword to: "`docs/RULES_VERSION.md` is excepted from the diff, and its only allowed change is the sentence that old versions are refused (checked by `git diff <int-7> -- docs/RULES_VERSION.md` showing only that hunk); `web/` excepted for the P1 diff as before". The same exclusion goes into the `lint` step of `end_of_d1.sh` (it runs `git diff --exit-code $base -- web/ docs/RULES_VERSION.md ...`): add `':!docs/RULES_VERSION.md'` there when the doc edit lands.
* **aliases** (`.cargo/config.toml`), thin-crate Cargo.toml: lane 9b/9d, not this lane.

## 4. Flip to strict (lane 9b merge checklist)

1. `tools/engine_core/check_production_graph.sh` must print `0 violation(s)`; `check_layers.py thin` must be green.
2. Change the defaults: in `verify_package.sh`, `verify_stage.sh`, `end_of_d1.sh` replace `${PROD_GRAPH_MODE---report}` by `${PROD_GRAPH_MODE-}` (empty = strict); in `deploy.sh` replace `${D29A_MODE:---report}` by `${D29A_MODE:-strict}` (twice).
3. Add `lend-string` to the `ALLOW_LEND` list if a new dev crate needs it; the self-test canary: add `features = ["lend-string"]` to `ti4-server`'s `ti4-engine` edge in a scratch commit, the script must go red (the self-test proves the same on a synthetic workspace).
4. Run the output probes on a fresh server build and wasm: any hit means the build still links legacy; a documented false positive may be deployed with `deploy.sh --allow-legacy-probe`.
