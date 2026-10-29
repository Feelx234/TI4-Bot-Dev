# Disabling XF-1, XF-2 and XF-3 (diplomacy, deck reserve, Last Bastion / Crimson Rebellion)

Decision (owner, 2026-10-29): the three whole-window bail families of `docs/ENGINE_AREA_STATUS.md`
(branch `core-area-status`, table 1.2) are **switched off in the product** and the core will panic
if one is ever used. Porting them costs 25-35 agent-hours and is dropped. The string engine keeps
running games until D2, so its code stays; this branch (`disable-xf123`, from
`nightly-fixes-2026-10-26`) only closes the **product entry points**. The core-side panics are a
separate later task on the core branches (section 5).

| Family | What it is | Faction / option ids |
|---|---|---|
| XF-1 | structured diplomacy (promises, deals, signals) | `GameState.diplomacy.enabled` (`ti4-model/src/diplomacy.rs`), module `ti4-engine/src/diplomacy/*`. Not the Diplomacy strategy card. |
| XF-2 | turn-redo deck reserve | `GameState.deck_reserve` (core: `cards.reserve`); set by `RngForce::attach` (`ti4-host/src/rng_force.rs:218`) |
| XF-3 | Last Bastion, the Crimson Rebellion | faction ids `bastion`, `crimson` (content `factions.json`: "Last Bastion", "The Crimson Rebellion") |

## 1. Step 1: investigation

### 1.1 Where each feature enters the product (before this branch)

**XF-1, structured diplomacy.** There is no product entry at all.
* `TableSetup` (`ti4-host/src/setup.rs`), `CreateGameRequest` (`ti4-server/src/http/games.rs`), the
  wasm `Setup` (`ti4-wasm/src/host_setup.rs`) and the web lobby have no diplomacy field or toggle.
  `DiplomacyState::default()` is `enabled: false`; `GameState::new` builds `for_players(.., false)`.
* The only non-test callers that set `enabled = true` are developer tools: `ti4-training`
  (`rollout.rs:696`, `:956`, `SimulationCapabilities { diplomacy }`), `ti4-mlp` examples,
  `ti4-review` (`gui.rs`, `diplomacy.rs:629`, `--diplomacy`), `ti4-replayer` GUI, `ti4-bridge`
  (LLM adapter contracts). None of them is reachable from server, wasm or web.
* A save can only carry it through `initial_state.diplomacy.enabled` (init record); every alpha
  `init.json` has it false (section 1.3).

**XF-2, deck reserve.** One live product entry: **turn redo on the server**.
* `POST /api/games/{id}/turn-redo` (`http/games.rs::turn_redo`, route in `http/mod.rs:139`) with
  `request` / `autoplay` / `restore` / `keep`, `GET` for the status. Registry:
  `session/registry/turn_redo_ops.rs`. Host: `ti4-host/src/history/turn_redo.rs`.
  A redo writes `rng_marks` (with `decks`) into `history.json`; the worker
  (`session/worker.rs:1089`), recovery (`storage.rs::recover_from_init` ->
  `replay_session_with_copy`), `batch.rs:1479` and `replay.rs:176` build an `RngForce` from the marks
  and `attach` creates `state.deck_reserve = Some(..)` whenever the marks are non-empty
  (`RngForce::new` is `None` for empty marks). So the reserve exists **exactly** in games whose
  history has non-empty `rng_marks`.
* Web: `TurnRedoBar` / `useTurnRedo` / `protocol/turnRedo.ts`, shown when
  `capabilities.turnRedo` (`protocol/capabilities.ts` `SERVER_CAPABILITIES`, `App.tsx:465`).
* Smoke harness: `TI4_SMOKE_REDO=1` (`web/e2e/smokeExercises.ts`), 5 % of nightly runs
  (`tools/nightly-smoke/config.sh` `REDO_PROBABILITY`).
* **Offline single player does not use the reserve**: `ti4-wasm/src/host_session.rs::undo_turn`
  restarts the game from the kept answers (replay); the wasm crate has no `RngForce`, `RngMarks` or
  `deck_reserve`. Its `turnRedo: true` capability (`LocalTransport.ts`, `offline/capabilities.ts`)
  stays.
* Selfplay, bench, bots: none of them attach a force (`ti4-selfplay` has no `rng_marks` / reserve).

**XF-3, Last Bastion / Crimson Rebellion.** Already closed everywhere a person can seat a faction.
* `seating::LOBBY_FACTIONS` (24 base + PoK factions) is the only pool of the lobby; its doc says
  the Thunder's Edge factions are left out. It gates: server human seat choice
  (`registry.rs::check_faction_free`), bots (`bot_lobby_factions.rs` already asserts `"bastion"` is
  refused with "not available"), the lobby record validator (`storage.rs:325`), wasm
  `human_faction` (`host_setup.rs:611`) and seats, `lobby_faction_draw` (random seats), and the
  selfplay spec (`ti4-selfplay/src/spec.rs:335`, `:352`).
* Presets: `preset::ROSTER` (the `+fac` / `world` walk) excludes them; `+fac:bastion` fails
  `parse` -> unknown preset. Scenarios do not seat them.
* The `thunders_edge` game option (`TableSetup.thunders_edge`, `CreateGameRequest.thunders_edge`)
  switches TE *content* on (FULL sources, expedition); it does not add TE factions to any pool.
* Where the two ids still exist: the engine (`factions/bastion*.rs`, `crimson*.rs`, rule-bot
  `faction_plans.rs`), `seating::wide_roster` / `FactionRoster::Wide|New` with FULL/DEFAULT sources
  (used by `ti4-training`, `ti4-review`, `ti4-policy`, `ti4-calc` arena, `ti4-sim` soak and
  `wide_roster_smoke`), ML faction vocabularies (`ti4-mlp/src/lib.rs`, checkpoint manifests) and
  the web display tables (`factionInfo`, `inPlay`, `mapOverlays`, ... : display only).
* Web: no faction list of its own; the lobby gets the pool from the server.

### 1.2 Bench and golden baselines

**No impact.** Checked, not assumed:
* `tools/rulebot/golden*.json` (`bench`, `bench-v1`..`bench-v5` use `golden.json`, `golden-v2/v4/v5`)
  have no `factions`, `faction_pool`, `thunders_edge` or `roster` key. `SetupSpec::expand`
  (`ti4-selfplay/src/spec.rs`) then draws with `lobby_faction_draw`, i.e. from `LOBBY_FACTIONS`,
  which never contained `bastion` or `crimson`. The same holds for `stress.json`, `completion.json`
  and the core golden dual (`golden_positions(config, ..)` plays these configs).
* The explicit pools of `SetupSpec` are validated against `LOBBY_FACTIONS` too.
* No golden/bench game turns diplomacy on (no key; selfplay never sets it) and none attaches an
  `RngForce` (no reserve). `docs/ENGINE_AREA_STATUS.md` R3 says the same for the golden dual.
* Nothing in this branch touches seating, the selfplay pools, `wide_roster`, the engine or the
  rule bot, so no seed changes and the 200/200 hashes cannot move. The only changed shared function
  is `ti4_host::setup::create`, which selfplay does not call (it uses
  `map::create_game_with_template_seated_in` directly).
* `ti4-review/tests/golden/pre-r02-mlp-seed7777-rotation2.json` and
  `examples/reviewer/checkpoint-473312/manifest.json` mention `bastion` / `crimson` only inside the
  checkpoint's faction vocabulary; the golden table is the six in-scope factions.
* Things that do contain the factions but are **not baselines**: training seed files
  (`*.seeds`, used with `--roster wide` they can seat them), `/root/rulebot-results/custodians-factions/*`
  (deliberate Bastion/Crimson phantom-seat experiments, stored results; left alone). Both are
  untouched; neither is a gate of the core port.
* Caveat for the core branches (their gates, not this branch): the core-side panics must not fire in
  the golden dual / `board_equivalence`; per the above they cannot, since those games never seat
  the factions, enable diplomacy or attach a reserve. Dev tests that build such tables with the
  fixtures (`seated_game(.., "bastion")`, `diplomacy::..for_players(.., true)`, `deck_reserve =
  Some(..)`) will need to be removed or moved to the string side when the panics land (list in 5).

### 1.3 Stored games and fixtures

Read-only look at `/var/lib/ti4-public` (24 game dirs, 14 with `init.json`, 8.3 MB):
* **Bastion / Crimson: 0 games** (all 14 inits and all snapshots; factions seated: sol 7, sardakk 5,
  hacan 5, letnev 4, ghost 4, nomad 4, nekro 4, naalu 4, mentak 4, arborec 4, cabal/l1z1x/jolnar/mahact/titans/yssaril/xxcha 3,
  saar 2, empyrean/winnu/muaat/naaz 1; no `thunders_edge` game).
* **Diplomacy enabled: 0 games.**
* **Turn redo (non-empty `rng_marks`, thus a reserve): 3 of 24 games**, all from 12-13 Oct:
  `game_935f5618...` (80 decisions, one deck segment, reserved cards), `game_a36f40db...` (32
  decisions, dice + deck segment), `game_d26c07f1...` (9 decisions, two open segments, no reserved
  cards). These are the saves the load refusal in 2 will not open (the files stay).

Repo fixtures:
* `crates/ti4-host/tests/fixtures/replay_format1_sample.json` (used by
  `ti4-host/tests/export_format1.rs`) has `rng_marks` with deck segments. It is a **format fixture of
  the host export parser**; this branch does not touch the parser, so it keeps passing (the refusal
  lives in the server recovery, not in `ti4_host::export`).
* No fixture JSON seats Bastion/Crimson or enables diplomacy. Rust tests that do (all engine/core
  unit tests: `seated_game(.., "bastion")`, `DiplomacyState::for_players(.., true)`,
  `deck_reserve = Some`) construct states directly and are unaffected.
* Server tests that exercise the redo through the registry (`turn_redo_registry.rs`,
  `turn_redo.rs`, `batch_after_redo.rs`, `random_bots.rs`) call the registry/host API and keep
  passing, except four that go through the HTTP route or restart-recover a redone history; they are
  `#[ignore]`d with a reason (step 2): `turn_redo_registry::{the_redo_and_its_saved_original_survive_a_restart_with_their_forced_dice,
  http_permissions_and_shapes, a_redo_in_its_new_turn_recovers_from_disk_with_its_open_card_reservations}`
  and `batch_after_redo::slow_a_batch_confirmed_after_a_turn_redo_matches_the_full_replay_in_game_marks_and_recovery`.
  Web unit tests of the redo bar use a mocked transport (capability passed explicitly) and keep
  passing.

### 1.4 Verdict of step 1

No baseline moves and no stored fixture breaks if only the product entry points are gated. Step 2
therefore went ahead (section 2).

## 2. Step 2: what is disabled, where

Single source: `ti4_host::disabled` (`crates/ti4-host/src/disabled.rs`).

| Edge | Refusal |
|---|---|
| `POST /api/games/{id}/turn-redo` (all four commands) | `403` "Turn redo is switched off" (`http/games.rs`); the GET status keeps answering "no redo" |
| web, server transport | `SERVER_CAPABILITIES.turnRedo = false` (`VITE_TI4_SERVER_TURN_REDO=1` turns it on, set only by the screenshot runner so folders Q, AW, CF stay reproducible): the redo bar is not shown (offline stays on: it replays answers, no reserve). Screenshots: `web/e2e/screenshots/HJ-turn-redo-off` |
| server recovery (`storage.rs::recover_from_init`) | `StorageError::Disabled`: a game whose history has non-empty `rng_marks`, whose initial state has diplomacy on or seats `bastion` / `crimson` is reported in `RecoveryReport.failed`, its files are kept |
| `ti4_host::setup::create` (server lobby start, wasm new game / load of a setup) | `Err("... is switched off ...")` for a table that seats `bastion` / `crimson` or has `diplomacy.enabled` |
| lobby human seat / bots / `seat-faction` | unchanged: `LOBBY_FACTIONS` already refuses both ids (tests added for `crimson` and the seat endpoint) |
| nightly smoke | `NIGHTLY_REDO_PROBABILITY` default 5 -> 0 |

Not done on purpose (would be selfplay/test/training paths, which must keep working for the
baselines and the string engine until D2): `seating::wide_roster`, `FactionRoster`, the training and
review tools, the rule-bot faction plans, the engine modules, `DiplomacyState`, `deck_reserve`
itself, the host redo code (`history/turn_redo.rs`, `rng_force.rs`) and its registry ops.

## 3. Tests added

* `ti4-host/src/disabled.rs` (unit): ordinary tables pass; `bastion` and `crimson` refused by id;
  diplomacy on refused; non-empty `rng_marks` refused.
* `ti4-host/src/setup.rs`: `create` refuses a draw seating either faction (with and without Thunder's
  Edge), and `draw` refuses them as a chosen faction.
* `ti4-server/tests/it/disabled_features.rs`: bot seat and host seat-faction refuse both ids (with
  and without Thunder's Edge); random draws never seat them; `POST /turn-redo` answers 403 for all
  four commands and writes no history; recovery reports a save with diplomacy on, with either
  faction, or with `rng_marks` as `StorageError::Disabled` and keeps every file.
* `web`: `useGameSession.transport.test.tsx` expects `turnRedo: false` for the server transport.

## 5. Core-side panic sites (for the later task on the core branches)

Greps on `core-area-status` (5c02b4db); re-run them on the branch that gets the panics.

**XF-3 (Bastion / Crimson).** A panic where a typed game is built from a state that seats `bastion`
or `crimson` (the codec/`load`) is the single clean place; the following are the sites that currently
decline or bail on them and become dead code or panics:
* `ti4-engine/src/production_view.rs:39` `unported_economy` (callers `:64`, `:84`, `:106`, `:293`),
  `supply_core.rs:28`, `production_window_dual.rs:259`.
* `ti4-engine/src/dual_window.rs:1611` (Last Bastion retreat note bail).
* `ti4-engine-core/src/kernel/hooks/direct.rs`: `bastion_*` hooks (lines 68-93: `bastion_extra_die`,
  `_note_ship_lost`, `_take_ship_lost`, `_note_retreat`, `_clear_retreated`, `_adjust_ship_spots`,
  `_ship_placed`, `bastion_units_resource_bonus`, `_cancel_bombardment_hits`, `_cancel_in_groups`,
  `_watches_ground_round`, `_non_home_systems_with_planets`, `bastion_commander_blocks_sabotage`)
  and `crimson_*` (111-127: `crimson_active_breaches`, `_abilities_lost`, `_enforce_sundered`,
  `_can_deploy`, `_deploy_planets`, `_note_deployed`, `_place_sorrow_breach`,
  `crimson_cards_offer_card_spot`, `crimson_cards_put_on_card`).
* `ti4-engine-core-factions/src/ca_combat/{bastion,crimson}/*`, `ca_combat/mod.rs:13-21`,
  `cb_economy/winnu/query.rs:111` (`bastion_units_resource_bonus`), `cards/mod.rs` (Skilled Retreat
  with the Last Bastion).
* Bail IDs: B3-bail-03, B12-bail-03, A4b-1-bail-1, B2-K2, B8-bail-07/08, B8c2-bail-04,
  R-bail-10/12, B1g-1 (Last Bastion retreat).

**XF-2 (deck reserve).** Panic where `reserve.is_some()` is read into the core (`core_sync/codec/cards.rs:44-47`, sets
`c.reserve = None` otherwise) and remove the guards:
* `ti4-engine/src/explore_write.rs:49,152,301`, `dual_window.rs:919,990`,
  `objectives_write.rs:90,98,106`, `strategy_cards_core.rs:461,486,550`.
* `ti4-engine-core/src/rules/action_cards/effects.rs:208` (`Fx::Veto && cards().reserve.is_some()`),
  `:779` (`cards_mut().reserve = Some(..)`).
* Bail IDs: B5-bail-01/08, A4-bail-2, B8-bail-06/09/11, B9-bail-07, B9c1-bail-01, B9c2-bail-01,
  B6-bail-06.

**XF-1 (diplomacy).** Panic where `diplomacy.enabled` is read into the core (`core.trade().diplomacy.enabled`
is the typed field) and remove the guards:
* `ti4-engine/src/explore_write.rs:304`, `dual_window.rs:788,1547,2325`, `reactions_core.rs:261`,
  `leaders_core.rs:141`, `transactions.rs:1193,2134`, `game.rs:2381,5855` (string side, stays).
* `ti4-engine-core/src/query/trade.rs:209`, `rules/diplomacy.rs:94,148` (the whole rule file:
  `evaluate_event` / promises hook), `rules::diplomacy_book`, the `DiplomacyWindow`
  (`core-b4-trade`).
* Bail IDs: B1g-1, B5-bail-02, B7-bail-5/6, B8-bail-01/04/05, A4b-2-bail-2, B9c2-bail-01, R-bail-08,
  B4d-bail-07.

The dev paths that enable these (`ti4-training`, `ti4-mlp` examples, `ti4-review`, `wide_roster`
with FULL sources) are string-only today; when the core panics land, keep them off the core (they
run on the string engine) or drop the `--diplomacy` / `--roster wide` options that reach them.
