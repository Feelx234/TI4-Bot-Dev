# core-integration-21: the wrapper joins the line (2026-11-04)

Branch `core-integration-21`, cut from `core-integration-20` @ `b8e43ef8d`. Three merges (gaps 19, the wrapper with the movement facts, the ack-hang fix), then the wrapper-side follow-ups of `CORE_GAPS_19.md` and the module-by-module run of `ti4-server` `tests/it`.
Not pushed; the live branch `nightly-fixes-2026-10-26` was not touched. `RULES_VERSION` 35, serde, fixtures (one exception, section 5), generated wire TS, `host_create.golden` (byte-identical) unchanged. Quick game: 1,165 steps, fingerprint `74d5675ac754ac81` (seen after the merges and after part 2).

Read first: `docs/INTEGRATION20.md`, `docs/D28D_RESUME.md`, `D28F_RESUME.md`, `D28G_RESUME.md`, `CORE_GAPS_19.md`.

## 1. Merges

| # | Branch | Tip | Conflicts and how they were joined |
|---|---|---|---|
| 1 | `core-gaps-19` | `07777d825` | `invasion_composer.rs` (x89 tests of int. 20 and the Dacxive test: both kept, one closing brace). `cargo check --workspace --all-targets` green; `invasion_composer` 19, `strategy_primaries` 18, `fire_team` pass |
| 2 | `core-d28f-movement` (brings `core-d28d-wrapper`) | `bf66b6526` | `oracle.rs` (Follower: d28f's legacy-path form + int. 20's `TI4_NATIVE_PLAY_TRACE` line), `kernel/ability.rs` (`skipped` and `auto_paid` are adjacent fields, both kept), `replay_export.rs` (int. 20's two assertions), `surface_paths.baseline.json` (theirs, then `--update`: **0 legacy**, 195 paths, check ok). No conflict in `ti4-engine-next/src/lib.rs`, `render/leaders.rs`, `combat_live.rs`, `ties/*`, the server call sites |
| 3 | `core-d28g-ackhang` | `ba22ecbaf` | none (see 4.1 for the join it needed) |

Join points checked: `Game::try_new` / `check_playable` / the first step refuse what `ti4_host::disabled::check_playable` refuses with `GameError::Disabled`; the equality check on `RULES_VERSION` became `MIN_SUPPORTED_RULES_VERSION ..= RULES_VERSION` (`GameError::Unmapped`, as the existing test pins) and `thunders_edge_system.is_some()` counts as Thunder's Edge content (test line added to `the_wrapper_refuses_what_check_playable_refuses_with_disabled`). `product_init` is still called by `server.rs` and `host_api.rs`. `host_create_golden`: int. 19's form, golden file untouched.

## 2. Part 2 (wrapper-side follow-ups)

| # | What | Guard (fails on the pre-fix code) |
|---|---|---|
| 1 | single-way payment note: `native_step` installs `table.auto_paid` beside `skipped`, drains it after the committed step only (one note per question and position), renders `choice` at `at` (`render_asking`) and calls `table.auto_resolve(.., "it was the only way left to pay")`. The asker's `rendered.options.len() == 1 && pay_resources` check is removed | `splice::a_single_option_question_that_appears_is_reported` and `..._vanishes_...` (both failed: no note), new `splice::a_single_way_payment_is_told_once_with_the_reason`. Mutation: dropping the per-(question, position) dedupe survives (no case found where the driver logs a payment twice in a committed step; it stays as a defence) |
| 2 | `ties::invasion::fill_codes` (thin copy of `invasion_live::fill_codes`, all texts the render's) called from `set_table_codes`: ground-dice labels are named in product sessions | thin `live::plays::the_product_env_carries_the_invasion_texts_of_the_ground_dice` (fails with the call removed). Fingerprint `74d5675ac754ac81` and 1,165 steps unchanged |
| 3 | turn redo | see 3 |

## 3. Turn redo tests: rewritten to the refusal

`ti4_host::disabled::TURN_REDO_ENABLED` is false: the HTTP route answers 403, a state with a deck reserve is `GameError::Disabled`. The registry's `turn_redo_request` now refuses too (`HistoryError::Forbidden(TURN_REDO_OFF)`, before any credential check; autoplay / restore have nothing in flight). Tests:

| File | Was | Now |
|---|---|---|
| `turn_redo_registry.rs` | 9 scenarios (permissions, chains, restart, bot seats) | 3: request refused for host, guest and a stranger with the game untouched; the other commands have nothing to act on; every HTTP command answers 403 with `TURN_REDO_OFF` |
| `turn_redo.rs` (1,628 lines, host library scenarios) | 16 | 2: attaching a reserve is `GameError::Disabled` at the first step; `check_playable` names `Disabled::TurnRedo`. The scenarios are in git history |
| `batch_after_redo.rs` | 1 (ignored) | 1: the redo is refused and the history carries no marks |
| `batch_fork_differential::..forced_rng..`, `fork_equivalence::..forced_rng..` | forked / batch-checked histories with marks | `support::assert_forced_replay_is_refused` (first step `Disabled`, nothing played) |
| `random_bots::the_host_can_redo_a_bot_seats_turn..` | redo of a bot turn | refused, game goes on |
| `fork_equivalence::slow_..newer_factions..` | Bastion, Deepwrought, Crimson, Ral Nel, Firmament tables | those lists are `Disabled::Faction`; the playable newer faction (Nekro) tables are forked as before |

No other assertion was loosened.

## 4. What the join exposed

### 4.1 Live and recovered games showed different positions at a pending question (d28g x the session)

`batch_commit_resume` failed after merge 3 (live `tactic_tokens` 3, recovered 2; events equal). d28g shows the table `Asking::at` (the position inside the rolled-back step) for questions at the close of a window; the session published that state only when the question carried a new decision (`worker.rs` `on_observed_offer`: `records.len() <= published` returned before `latest_state` was set), so the live game showed `at` and a game recovered from its log showed the step-start state. Fix: the offered state is published also when no decision is new. Restricting `position_of` to card questions was tried first and rejected (it hung `random_bots` undo/redo). Guard: `batch_commit_resume` (fails on the merged tree without the fix). `declared_card_reply`, `random_bots`, `history`, `session_recovery_replay`, `crash_recovery` pass.

### 4.2 Other joins, all test-side except where said

* `golden::tests::the_smoke_subset_reproduces`, `a_planted_answer_difference_is_found_at_its_step` (bridge lib): the frozen sets replay by the **string** engine (rules versions 1..35); `replay_game` built the wrapper's game, which plays 35 only. It builds `legacy_game(spec)` again (as int. 19 did). Golden files untouched.
* `native_play::..the_printed_lanes_of_the_hyperlane_map`: its premise (`spec.build()` comes with the laws unapplied) no longer holds; the wrapper applies them when it takes the map. The test asserts the wrapper's galaxy and the core's hold the lanes.
* `session_e2e_game`: `start_game_seeded` seats the invented `generic` faction, which the core refuses; the test seats Sol, Hacan, Letnev.
* `splice`: decisions 40 / 54 re-chosen (`ss`, `pass`), the dice test searches seeds 2..9 for a roll; `a_single_option_question_that_appears_is_reported` no longer says the history survives to its end (paying the mech spends resources the same seat's Orbital Drop at decision 79 needs, so the history really differs there): it asserts the conflict is that decision and that decisions 64..79 all fit (the unlogged payment shifts no cursor).
* `bluff_reaction` (`pass_ends_the_hold_early..`, `the_default_hold_lasts_two_to_four_seconds`) failed in the **gate** profile and passed in dev: **profile timing, not logic, but a real behaviour of the wrapper**. The native step tells `window_skipped` of every window it passed by after the step, in a row, so a combat that opens several declared windows holds them back to back with no engine work between; the poll of the test sees a gap in a slow (dev) build and none in an optimised one, and took two holds for one. The tests set `seat_round_holds: 1` (one hold per round) and say so. Both pass in the gate profile (4.8 s for the module). Not decided: whether chained holds are wanted (total time is the sum of the windows, capped by the seat budget of 60 s and 8 holds).
* `ti4-selfplay` `thunders_edge_tables_offer..` is `thunders_edge_tables_are_refused_and_default_tables_never_offer_the_expedition` (the table is an engine error "Thunder's Edge content is switched off", 0 decisions). Note: `selfplay-results/` (git-ignored) caches results by spec; a contaminated run of mine had filled it, delete it before judging selfplay tests.
* `lint_core.sh` had 4 hits left by int. 20 (`empty_slots`, the slot census helper): `lint-allow(core)` with the reason. `clippy_core.sh` was red (unused imports left by gaps 19 and d28f, a `very complex type` for `skipped` -> `SkippedLog`, a `a || (b && a)` in `silent_census.rs` -> `at < end`, unused imports in the thin crate): clean, engine 41 = baseline 41.

## 5. Red tests remaining (and why)

| Test | Cause | Class |
|---|---|---|
| `ti4-server` `fixtures_verification::ensure_and_verify_fixtures` | the sample game of `ti4-server::fixtures` is hand-built with invented ids (`leadership`, `direct_hit`, `destroy_their_greatest_ship`, ...); the native objective progress cannot load it (`Unmapped`), so `objective_progress` / `home_system_lost` drop out of the table of the snapshot fixtures. The fixtures were not regenerated. Fixing it means valid ids in `create_sample_game`, which `projection_redaction` asserts by name and the checked-in `fixtures/protocol/*.json` (wire fixtures read by `web`) carry | **question for the user (Q3)**: regenerate the fixtures with valid ids (and re-point `projection_redaction`), or accept snapshots without the progress fields. Default: leave red |
| `ti4-server` `lobby_all_factions::slow_tricky_factions_play_a_longer_smart_game` | `ghost`: `aftermath: UnportedAftermath(CombatUnported)`: a fight with more than `MAX_TYPES` (10) named unit kinds (ships with their upgrade twins) names none. Known remaining risk of `CORE_GAPS_19.md` section 5 | core gap, not the join |
| `ti4-engine-core-tests --features wrapper --test wrapper_props` (the 155th binary; needs the product crates, not part of the 154) | 4 of 6 red: `p7_the_codec_round_trips_fixture_tables_with_real_factions` ("load(store(x)) differs from x" for the 3, 4 and 6 seat fixtures), `p7_..every_template_opening`, `p7_..positions_of_golden_games`, `leak_scan::p9_..`. **Not triaged and not compared with int. 20** (a second target for the old tree did not fit the budget); the thin `fixtures` / the swap are the first suspects | open, to be looked at first in the next lane |
| `ti4-selfplay` `same_seeds_give_identical_games_on_any_number_of_workers..` | the 4p table sardakk, cabal, nomad, nekro, seed 1 stops at round 2 with `aftermath: Unported(Thundarian)` (`driver/windows/invasion/fight.rs::round_unported`: a readied The Thundarian in a ground combat with a Nomad seated is not carried) | core gap, long known (M3_RESUME); the user's "port everything" applies |

## 6. Matrices (this tree, `gate` profile for the binaries)

| Check | Result |
|---|---|
| `native_play --quick --shadow` | 1 game, 1,165 steps, fingerprint `74d5675ac754ac81`, zero gaps |
| plain 96-set / 288-set | 96 / 96, 102,992 steps; 288 / 288, 298,270 steps (= int. 19 / 20) |
| Z-gate `--factions all --players 3,4,5,6 --seeds 8 --shadow` (four processes, two at a time) | all four exit 0; 768 finished, zero gaps, zero differences of the core; known string bugs 20 / 39 / 34 / 59 (3p / 4p / 5p / 6p) = 152, identical to int. 20 |
| `ti4-server` `tests/it`, 75 modules, gate profile, `--test-threads=2` | 73 modules green (bluff_reaction 11, splice 16, undo 3 in 584 s, smart_bots 4, random_seats 2, ...); 2 red (section 5) |
| unit suites (dev profile) | host 310 (+3, +2 in the other binaries), rule-bot 190, policy 287 (includes `scored_games_stay_legal_and_deterministic_across_nested_windows`, 477 s), wasm `--lib` 9, thin `ti4-engine` 365, bridge `--lib` 141 (golden 6), core 479, factions 154, legacy 5,302; selfplay: 4 of 5 (section 5) |
| D4 crate, 154 binaries | 154 green, 1,990 tests |
| `cargo check --workspace --all-targets` (no torch crates), `cargo check -p ti4-wasm --target wasm32-unknown-unknown` | ok |
| `check_layers.py core` / `lint_core.sh` / `clippy_core.sh` / `surface_paths.py --check` | 574 files 0 violations / 0 hits, 4 allowed / ok (engine 41 = baseline 41) / ok (195 paths, 0 legacy) |

## 7. String engine bugs

None new. The S5 rows (43 games) are the same as int. 20. Questions open: Q1 (Wormhole Nexus as Dannel's target), Q2 (`secrets::award`), Nano-Forge (not implemented in either engine), **Q3** (section 5, fixtures).

## 8. Not done

Heavy gates (golden / wide dual, benches, soak, census, ratchet, workspace clippy), mutation batches beyond the one in section 2, the Thundarian port, the `MAX_TYPES` naming, torch crates. D4 crate: the full run of the 154 binaries of `ti4-engine-core-tests` (default features, one `cargo test --test` per binary, binaries deleted after each) passed: 154 of 154, 1,990 tests.
