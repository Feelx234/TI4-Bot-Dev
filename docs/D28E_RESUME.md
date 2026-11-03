# D2-8 lane E resume: product edges (package P1)

Branch `core-d28e-product`, cut from `core-integration-17` (`d61ef6b08`), then `core-todo-d28` (the D2-8 theory document,
`docs/todo/D2-8-wrapper.md`), the live branch `nightly-fixes-2026-10-26` (local merge only, nothing pushed, the live branch
itself untouched) and `disable-xf123` (`f472fd4e1`). Product code only: no engine rule body, no wrapper, no
`RULES_VERSION` change. Owner decisions of 2026-11-03 are the brief; each is a row below.

## What is done

| decision | what | where |
|---|---|---|
| 1 live merge | `nightly-fixes-2026-10-26` merged; `RULES_VERSION` stays 35 (the core line's). Conflicts: the live held-decisions batch fix (`273138a55`) replaces the core line's earlier held-back one (`ecf996dc0`) in `ti4-server` `session/{mod,registry,worker}.rs` (the core line's batch test `held_back_batch.rs` is kept and adapted to the new `batch_basis`); `ti4-host` `view/tactical.rs` keeps one engine-map helper (the core line's `as_the_engine_sees_it`, the live copy dropped, the live hyperlane test kept); `view/scenario` test: the live one | merge commit `64649e211` |
| 2 Thunder's Edge | `thunders_edge` (the table switch), the `expedition` preset (any suffix) and a saved game that carries the content are refused at creation (`TableSetup::check`, `setup::create`, HTTP `POST /api/games` 400 with the reason, lobby settings, wasm start) and at load (recovery, replay, resume). The strategy card set `te` is allowed. Lobby row, offline setup switch, `TableRequest.thundersEdge` and the wire fields (`TableSetup.thunders_edge`, wasm `Setup.thunders_edge`, `ts(skip)`; Rust keeps them to read and refuse old saves) are gone; the wire TS lost exactly those two fields. Nightly: `NIGHTLY_TE_PROBABILITY=0`, `NIGHTLY_OFFLINE_TE_PERCENT=0` (`tools/nightly-smoke/config.sh`, README) | `ti4-host/src/{disabled,setup}.rs`, `ti4-server/src/{http/games,session/{registry,table}}.rs`, `ti4-wasm`, `web/`, `web-shared/` |
| 3 old saves | `MIN_SUPPORTED_RULES_VERSION = RULES_VERSION = 35` (`ti4-model`). `ti4_host::disabled::check_playable` refuses a lower version at creation, recovery/load, replay (every `ti4_host::replay` entry; the endpoint answers 422) and the wasm resume (`host_api::prepare`, before anything is built). No list, no viewer, no deletion screen; files are kept. The recovery report marks refusals (`RecoveryFailure::is_refusal`, `StorageError::Disabled`), the server logs `refused 'ID'` and counts them apart (`; F failed; R refused`). The alpha preflight (`recovery-check.sh`) counts refused saves apart, needs no `--allow-failed` for them and requires every refusal to name its reason; self-test `deploy/public/test-recovery-check.sh`. `alpha_roundtrip` lists refused games as skipped with the reason | see `docs/RULES_VERSION.md` |
| 4 replay endpoint | unchanged (leaks stay) but it refuses what load refuses (422 for an older version) | `http/games.rs::get_replay` |
| 5 disable-xf123 | applied (merge) and extended: nine ids (`bastion, crimson, keleresa/m/x, deepwrought, firmament, obsidian, ralnel`) refused at creation and load, plus structured diplomacy and the deck reserve / turn-redo marks. There is **no second list**: `ti4_host::disabled` asks the engine's own (`core_sync::unsupported::find`); `DISABLED_FACTIONS` is the ids of the tests, a test keeps it equal to the engine's over every faction of the content | `ti4-host/src/disabled.rs` |
| 6 `product_init()` | `ti4_engine::product_init()` (thin crate, `src/product_init.rs`, own section of `lib.rs`): `product::set_strict(true)`, `core_sync::unmapped::set_strict(true)`, `core_sync::unsupported::set_disabled_policy(Policy::Panic)`. Called from `ti4-server` `bin/server.rs` and, once per module under `cfg(target_arch = "wasm32")`, from `host_api::prepare` (the native wasm-crate tests share one process and keep the library defaults). There is no host CLI binary (`ti4-cli` is a stub that plays nothing): nothing to call. A known refusal is an `Err` first, so the panic stays for positions that cannot arise | |

## Tests (each fails without the change)

* `ti4-host` `disabled::tests` (version, TE, each of the nine factions, diplomacy, deck reserve, marks, version first), `setup::tests` (creation refuses every dropped faction and every TE form including the preset; the disabled list equals the engine's), `tests/refusals.rs` (every `replay_session*` entry refuses an older version, TE, a dropped faction, diplomacy).
* `ti4-server` `tests/it/refusals.rs`: creation answers 400 with the reason (switch, fill-seats form, preset and `expedition+rot`), `te` card set and `thunders_edge:false` still create, the lobby settings refuse the switch, recovery refuses each older version (0, 1, 27, 34), TE and each of the nine factions as `StorageError::Disabled` with every file byte-identical and the save loading again once restored, the version wins over the other reasons, the replay endpoint answers 422 for an older game, a saved lobby with the old switch cannot be started. `disabled_features.rs` extended to the nine ids.
* `ti4-wasm` `host_thunders_edge.rs` (start with the switch or the preset refused with the reason, `te` cards play), `replay_export.rs` (a save carrying the switch is refused and resumes without it; older versions 0/1/34/none refused with both numbers), `host_history.rs` (a current save resumes, every older version is refused).
* `ti4-engine` `product_init` (a child process: defaults off, after the call strict and `Panic`).
* `bridge` `alpha_roundtrip.rs`: older version and TE games are listed as refused, not checked.
* `deploy/public/test-recovery-check.sh`: parse, compare and the preflight against a fake server (refused ones pass and are counted, damaged ones fail, a refusal without a reason fails, counts must add up).
* web: `offline.test.tsx`, `launcher.test.ts`, `web-shared` `api.test.ts` (no Thunder's Edge field any more), screenshots below.

Tests that played old-version or TE saves could not stay as they were: `host_local` resumes with the current version; three wasm fixture tests are `#[ignore]`d with the reason (`host_neutral_opponent` is a Thunder's Edge game at version 27, the two `host_reaction_followup` saves are version 18): **re-record those positions at version 35 without the content** (corpus note below).

## Screenshots

`web/e2e/screenshots/HK-thunders-edge-off` (manifest, `capture-1-forms.ts`, committed `before/` PNGs taken on the commit before the change): the online table form, the offline setup and the lobby (desktop and 390x844) without the switch, and the message a refused table shows. `cd web && npm run screenshots -- HK`. `FP-thunders-edge-expedition` cannot be taken any more and is marked `"retired"` (the runner skips such folders).

## Corpus note (what replaces the alpha saves)

All 14 stored alpha games (`/var/lib/ti4-public`) are rules version 0: after this lane they are **refused**, so `deploy.sh --preflight-only` proves only that they are refused cleanly. Recovery and replay are proved on version-35 data instead: the server recovery tests (`tests/it` session recovery, `refusals.rs`), `host_session`/`HostSave` resume tests, the golden sets re-recorded at 35 (D2-8 section 4.3, lane F), and the nightly offline resume check. The fixtures above are the first thing lane F re-records.

## Join points

* **Integration 18** (merges C, A, B2, R in parallel): `git merge-tree` of this branch against `core-d28c-setup`, `core-integration-18`, `core-d28a-types`, `core-d28b-surface`, `core-d28b2-surface` conflicts **only** in `crates/ti4-engine-next/src/lib.rs` (this lane's section `// ---- lane E ----` with `mod product_init; pub use product_init::product_init;` against theirs: keep both). Lane C's host call-site edits (`preset.rs`, `replay.rs`, `view/scenario`, `view/tactical.rs`, `map.rs`, `session/worker.rs`) auto-merge with ours.
* **Lane D (wrapper)**: `Game::try_*` should return `GameError::Disabled` for what `check_playable` refuses so the host calls stay one rule; `ti4_host::replay::replay` is the funnel that now carries the check (`ReplayError::Refused`). `product_init()` needs `core_sync::{unmapped, unsupported}` and `product` under the same paths in the thin crate (lane A moves them; keep the names).
* **Lane A**: `crates/ti4-engine-next/Cargo.toml` is untouched by this lane.
* **Lane F**: `native_play`/`host_create_golden` use `thunders_edge: false` already (the golden's TE rows, if any, must go: `setup::create` refuses them); `alpha_roundtrip` needs a version-35 data directory.

## What remains (outside this lane's brief)

* Re-record the three ignored fixtures at version 35 (above).
* The fixer/summary prompts (`tools/nightly-smoke/prompts/{fixer,summary}.md`) still name `crates/ti4-engine` as the Rust engine: change them at the flip, not before.
* `FP-thunders-edge-expedition` and the TE branches of the smoke harness (`TI4_SMOKE_TE`, `TI4_OFFLINE_TE`, the `expedition` preset builder, TE presentation code in `web/src/presentation`) are kept as dead-but-guarded code (the harness throws a clear message); delete them together with the engine's TE code when the content is dropped for good.
* A list of refused games in "My games" and a message why a recovered game is missing (D2-8 4.2) were dropped by decision 3.
* `ti4-cli` plays nothing; if a host CLI appears it calls `ti4_engine::product_init()` first.
* `replay_export::a_game_in_progress_replays_to_its_open_question` (server) changed its table from Thunder's Edge on to off; it now stops at 62 answers because of a **latent, position-specific mismatch on `core-integration-17` itself** (checked on `d61ef6b08` and on the live merge): at some positions (40, 60, 61, 80 answers on this table, seed 18432220952314264515) the live session's `current_state()` lacks a `private:#staged:event:000000` mark that the replay to the next question has (a staged trade-primary event flushed in the live state). Not caused by this lane; worth a look by whoever owns the held/staged events.
