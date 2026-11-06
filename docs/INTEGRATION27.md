# core-integration-27 (2026-11-06)

Branch `core-integration-27`, cut from `core-heavyfix-1` @ `b7ac383a9` (= `core-integration-26` `d7adf5fe8` + 4 commits). Not pushed. The live
branch `nightly-fixes-2026-10-26` was not touched; the nightly cron was not changed. `RULES_VERSION` stays 36 (`MIN_SUPPORTED_RULES_VERSION` =
`RULES_VERSION` = 36 in `state.rs`, unchanged against int-26), so this line refuses no save that int-25 / int-26 accepted.
Purpose: the int-26 line plus the fixes for the six steps that failed in round 2 of the heavy run (`docs/HEAVYFIX1_RESUME.md`), deployed to the alpha.

## 1. Content (no merges)

The branch is `core-heavyfix-1` itself; nothing else was merged (`core-theory-fork-rng` is doc-only and stays out; the Jol-Nar mech
investigation runs in its own lane, `core-jolnar-mech-1`).

| commit | what |
|---|---|
| `4a69524b1` | `leaders` test: Rin's re-form is native since core-lent-gaps; the stale bail assertion now asserts the core plays it |
| `6bd911642` | bench baselines: the four Jol-Nar/Rin games re-recorded (`baseline-v2/-v4/-v5.json`); core test of the native re-form |
| `6e061ef4c` | core `Tape::take_unknown`: an unknown noted in the speculative rest of a dry run is dropped with its rollback (zero-bail's unclassified `close_vote` cause) |
| `b7ac383a9` | `docs/HEAVYFIX1_RESUME.md` |

Own commits on the line: `end_of_d1.sh` lint base `heavy-base-int27` first (then int26, int25, int24, ...); `docs/HEAVY_RUN_PLAN.md` header;
this document and `docs/I27_RESUME.md`. `web/`, `crates/ti4-model/src/state.rs` and `docs/RULES_VERSION.md` equal `heavy-base-int26` (empty diff).
Local tag `heavy-base-int27` = the tip.

## 2. Bugs found

None. Every check below is green and equal to int-26 where it is a hash.

## 3. Matrix (private target `/root/deploytmp/i27-target`, `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`; script and logs `/root/deploytmp/i27-logs/`)

| check | result |
|---|---|
| `cargo check --workspace` (without review, replayer, mlp, tensor, advisor) `--all-targets`; `-p ti4-engine-core-tests --features wrapper --all-targets`; `-p ti4-host --features ts -p ti4-wasm` | green |
| `clippy_core.sh` | ok, engine warnings 41 = baseline |
| `check_layers.py core` / `thin` / `setup` | 576 / 192 / 14 files, 0 violations each |
| `check_production_graph.sh` (strict) | 0 violations, 0 failed controls |
| `native_play --quick --shadow --twice` | 1,165 steps, fingerprint `538dc68f23dddcc9`, shadow compared 808, ZERO GAP, deterministic |
| `perf_kernel suite 1` | `suite_hash 775f4136e60a99d5` (14 games, 14,738 steps) |
| tests (gate profile), crates the 4 commits touch: core crates (`ti4-engine-core`, `-factions`, `-tests`) | 2,710 passed, 0 failed, 1 ignored (int-26: 2,708; +2 = the two new regression tests) |
| `ti4-engine-legacy --lib` (`leaders.rs`, `agenda_close_tests.rs`) | 5,344 passed, 0 failed, 7 ignored; with `TI4_AREAS=all=dual` `leaders::tests` + `agenda_close_tests`: 126 passed |
| `ti4-wasm --features ts` / without features | 10 + 48 (3 ignored) / 9 + 47 (3 ignored), 0 failed |
| `ti4-server` (`--test-threads=2`) | 58 + 357 (8 ignored), 0 failed |
| wasm (`web-shared/scripts/build-wasm.mjs`, wasm-release + binaryen from `web/node_modules`) + `scripts/wasm-size-gate.mjs` | 6,597,047 B (6.60 MB) raw / 2.10 gzip / 1.51 brotli against 6.63 / 2.11 / 1.52: ok (same size as int-26, still tight) |
| `check_output_probes.sh` on the wasm | ok: no legacy literal |
| web `tsc --noEmit`, `vitest run` | clean; 278 files passed, 2 skipped; 2,750 tests passed, 4 skipped |

Not run (by the brief): heavy gate steps, mutation ratchet, golden dual, bench verifies, soak, census, zero-bail.

## 4. Deploy to the alpha

Approved by the user. Run from this worktree's `deploy/public/deploy.sh` (the newest script: Caddy CSP warning), private
`CARGO_TARGET_DIR=/root/deploytmp/int27-target`, `TI4_PUBLIC_BUILD_DIR=/root/deploytmp/int27-build`; logs `/root/deploytmp/i27-logs/`
(`preflight.log`, `deploy.log`, `smoke-after.log`, the smoke script `alpha-smoke.mjs`).

* Deployed commit: `be97102eb` (the line before this docs commit; code identical to `b7ac383a9` and to the tip, only docs and
  `end_of_d1.sh` differ).
* **Release `20261106T055127Z-be97102e`**, live 2026-11-06 06:59 Berlin (deploy started 06:51:27, finished 06:59:48 CET).
  `previous` -> `20261105T061956Z-3e7a099a` (int-25).
* **Rollback:** `deploy/public/deploy.sh rollback` (points `current` back at `20261105T061956Z-3e7a099a` and restarts).
* Preflight first (`--preflight-only` on `b7ac383a9`, finished 06:13 Berlin): 27 saves, recovered 10, failed 0, refused 17 (the rules-version-0 saves,
  by design; the same 17 ids as int-25). Inside the real deploy: 28 saves (the script-proving smoke games against int-25 in between; terminated games are stored), recovered 11, failed 0,
  refused 17; production graph strict 0 violations; output probes ok (server, wasm); no Caddy CSP warning. Post-restart: the live service
  recovered 11 / failed 0 / refused 17, as predicted.
* Smoke (through Caddy, `https://ti4.diefamiliestamm.de`): `/health` `{"status":"ok","protocol_version":4,"commit":"be97102e","public":true}`;
  password access; a 3-seat game (2 random bots, `bot_delay_ms` 0) created, the host answered 27 decisions over the websocket
  (26 accepted, 0 rejected), game version 3 -> 147; the game was terminated afterwards. The wasm
  `/assets/ti4_wasm-CNjnYVvL.wasm` served 200 `application/wasm`, brotli, 6,597,047 B decoded, wasm magic. `index` 200; no panic/error
  in the journal since the restart. The same smoke was run against int-25 before the deploy (to prove the script): OK.
