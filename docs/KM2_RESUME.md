# D2 lane F2b (`core-d2-c3-dispatch`): the dispatch of the chosen menu options (KM2 rest), resume note (2026-10-31)

Branch `core-d2-c3-dispatch`, cut from `core-d2-c3-menu` (`9f5f62a4`). Never pushed; `nightly-fixes-2026-10-26` untouched. Only the LATEST rules version is supported.
Read `docs/KM_RESUME.md` (the menu itself) and `docs/KX_RESUME.md` (the strategic action) first.

## 0. Merges this branch needed (and why)

The menu branch is a sibling lineage: it had no tactical action, no production, no invasion, no strategic action. The dispatch can only route to windows that
exist in the tree, so three branches were merged (all kept both sides, every conflict resolved by hand, none loosened a test):

| merged | tip | why |
|---|---|---|
| `core-h-leftovers` (twice: `8d639019`, then `1caebccd`) | lane H | it carries `core-m2-production`, `-m3-invasion`, `-m5-aftermath`/`-m6-tactical` (the native tactical, `open_tactical`/`open_free`), M7's `native_play`, W lane 16, the agenda lane, M1 trade |
| `core-d2-c1-strategic` | `c283af1b` | lane E: `begin_strategic`, the follower arms (`Arm::Secondary/LeaderFollowers/Finished`) |
| NOT merged | `core-m6c/m6d-tactical`, `core-g2/g3-abilities` | in flight (20 and 12 conflicts); their hand-over lines are named in section 4 |

Conflict decisions worth knowing: the F lane's `turn/{ask,closing,start}.rs` and the three turn test files win over H's older copies (H still had `MenuSources`); the commander
unlock list is total (`unlockable` returns a list, not an `Option`), as H and the strategic lane have it; `Opt::Waive`/`WaiveBy` of the strategic lane and the
M1 `KernelEnv` (kernel `trade_env`) stay; Titans' Hecatoncheires (`StructureFacts::alt`, from lane W12 in H) is computed by the core in the strategic lane's
construction facts (`strategic/facts.rs::hecatoncheires`), which the strategic lane could not do before H was in.

## 1. Bugs found and fixed on the way (each with a guard)

| # | what | fix | guard |
|---|---|---|---|
| B1 | **A real native-vs-legacy difference** (reported by lane H as "three shadow tests fail since the M1 merge"): Artuno the Betrayer's pile was never paid out when the card readies. The string pays it in `leaders::ready` / `ready_all` (`nomad_agents::agent_readied`: a staged `TRADE_GOODS_GAINED` of source `artuno`). Since lane M1 the Tokens step runs the status phase's finish (which readies every card) natively, so the shadow of the status phase met it: the string had 3 trade goods and the staged event, the core 2 goods plus the pile. Not a shadow artefact. | `rules::leaders::agent_readied(core, who, leader, source)`; called by `status_finish::resolve_after_token_gain` for every leader `ready_all` readied and by Temporal Command Suite's ready | core test `rules::leaders::tests::a_readied_artuno_pays_its_pile_and_no_other_agent_does` (fails on the code before: the function does not exist; the bridge test failed with `players[2].trade_goods` 3 against 2); bridge `the_status_phase_arms_agree_with_the_legacy_game_for_every_lobby_faction` green |
| B2 | the other two tests of that report (`the_turn_flow_arms_agree...`, `a_perturbed_native_position...`) pass on this merge (they were fed by the same difference or by M7's older shadow handling) | none needed | the bridge lib run, section 3 |
| B3 | `rules::strategy_secondary::pay_waiver` of the strategic lane exhausted the Mahact agent with `exhaust_leader` (lane H1: every site that exhausts an agent tells a listening Nomad) | `exhaust_ready_agent` with the kernel | the lane's `jae_mir_kan_tells_a_listening_nomad` (failed after the merge, green now) |
| B4 | the tactical tail did not open `finish_action` after a plain tactical action (lane M6 left it to the dispatcher) | the same `open_finish` lines as `core-m6c-tactical` (`tactical/tail.rs`), so the merge is clean | `tests/tactical.rs` and the engine tactical lockstep still green; the menu lockstep below |
| B5 | `turn_live_tests::native_marked` rendered without the faction texts, so Politics'/Trade's option ids (seats by faction) were empty strings | the harness builds the render `with_factions` | `a_strategic_action_chosen_from_the_menu_begins_like_the_string` |

## 2. What is native now (the dispatch, `driver/turn/{action,component,faction,relic}.rs`)

`TurnJob::Action(key)` (`run_action`) is the one job: the closing-menu bookkeeping (a non-free option taken from the closing menu is the second action), then the body.
The generic wrapper is `component::conclude`: staged destructions / ground events / card moves announced (faction and leader arms only, as the string does), Extreme
Duress settled (`component::settle_extreme_duress`, ported whole: goods to the holder as a staged `TRADE_GOODS_GAINED` of source `extreme_duress`, `EXTREME_DURESS:<seat>`,
every card of the hand discarded and announced one by one through the reaction windows), `COMPONENT_ACTION_RESOLVED` or `_FAILED`, `Pending::note_failed_key` for a
failed action that is withheld (faction, technology, relic, device; NOT an action card, as the string), and `finish_action` (`end::run_finish`) for a resolved one.

| option | route | evidence |
|---|---|---|
| pass | `action::pass` (lane F) | lockstep (lane F) |
| tactical | `action::tactical`: Extreme Duress, `open_tactical` (`TACTICAL_ACTION_BEGAN`); the activation question is the tactical arm's next step; the tail now opens `finish_action` | core `tests/action_dispatch.rs` (window opened, announced, duress first); the existing tactical/aftermath lockstep |
| strategic | `strategic::begin::run_begin` inline (Extreme Duress lifted quietly, Coup d'Etat, the primary, the follower window) | engine lockstep `a_strategic_action_chosen_from_the_menu_begins_like_the_string` (trade, politics, leadership, imperial) |
| a transaction partner | `action::transaction`: Extreme Duress, `trade::open` (note the negotiation, spend the pair), then `ask::chain_trade` runs the trade window to its first question in the same call (also after a job that asked first) | core test of lane F (`choosing_a_transaction_partner...`) and `tests/action_dispatch.rs` |
| action card (hand and salvage yard) | `component::play_card`: the card leaves the hand (salvaged: purged), `play_inline` announces and resolves it inside the step (events mirrored as the string mirrors its timing log), verdict (a cancelled play fails without withholding, 22.4) | engine lockstep `an_action_card_played_as_the_action...`, `extreme_duress_on_the_acting_seat...` |
| relic purge (three fragments for a relic) | `relic::purge` over `rules::explore::purge_for_relic_scored` (the Shard of the Throne scores at once and the relic log says so) | lockstep: purge, the Shard, a failed purge withheld by the next menu |
| Lazax Gate Folding (Winnu) | `faction::lazax_gate_folding` over the core rule, form and Mecatol from the core | lockstep |
| Orbital Drop (Sol) | `faction::orbital_drop`: the core window `OrbitalDrop` run inline (planet, token, infantry, mech and its payment child window), facts from the core (the Sol mech is `ZSThunderboltM2`), the token event announced at once only at a table with a module (as `hooks_economy::emit`) | lockstep: the mech deployed and paid, and a table without a module (`sol, letnev, xxcha, jolnar`: no event) |
| the closing menu | second action, free leader, trade, end turn: all go through `ask::run_chosen` -> the same job | lane F's closing lockstep + the above |

The shadow oracle (`NativeSubject::menu_body_is_lent`) now lends only the options that are still unrouted, so the real-game shadows run the routes above against the legacy game.

## 3. Counted gaps: `Unknown::TurnGap(TurnGap::Menu(MenuGap::Unrouted(kind, key)))`

`kind` is `action::gap_code` (5 leader, 6 technology, 7 relic, 8 device, 11 faction), `key` is `Opt::key()` (injective): the gap names the exact option. The step leaves the
position as it was (core test `an_option_no_window_runs_yet_is_a_gap_that_names_it_and_leaves_the_position`). Why each is still a gap:

| option | why |
|---|---|
| leaders (agent/commander/hero uses, Ssruu copies), kind 5 | the string `use_leader` has 28 lent sites; the core has the windows (`hero_windows`, `leader_windows`, `leader_production`) but each takes facts the string adapters compute (`hero_dual.rs`, `leaders.rs`); there is no core `leader_action` PERFORM hook (the hook slot only lists: `Hooks::leader_action`), so every window needs a facts function from the core. The delivered-eight (Ggrocuto Rinn, Carth (free), Z'eu, Jace X, Darktalon Treilla, Rin, The Helmsman, Xxekir Grom TE) and the three string-only heroes are the first to do. NOT done. |
| technology: Sling Relay, kind 6 | the use of PRODUCTION as an ability (`UseOpen::Ability`) is a held window the step loop cannot select yet (`Arm::Production` is run by name by the aftermath chain; `step_arm` never returns it) |
| relic in use (Neuraloop, Codex, Titan Prototype, Stellar Converter, Enigmatic Device, Fabrication...), kind 7, and the Enigmatic Device cards, kind 8 | the windows exist in `rules/relic_windows.rs`; their facts (research payment, targets over the map) are computed by the string adapters |
| faction: Production Biomes, Trade Convoys, Blood/Dark Pact, Wormhole Generator, Promise of Protection, Muaat (Nucleus, Star Forge, Inferno, Fires), Naaz (two Fabrications, Black Market Forgery), Yssaril (Stall Tactics, Mageon Implants), kind 11 | windows/functions exist (`faction_component`, `muaat_component`, `technology_windows`, `hero_windows`), facts not yet computed from the core; Nekro C4R, Terraform and Cabal's Vortex are not even listed (the menu gap `MenuGap::Faction`), Vaults of the Heir, Psychospore, Deorbit Barrage are Thunder's Edge (out of scope) |
| Thunder's Edge Warfare's free tactical action, and a hero's strategy primary waiting for the end of a free action | `TurnGap::FreeTactical` in `strategic/begin.rs` and `TacticalGap::{WarfareSecondary,LeaderStrategy}` in `tactical/tail.rs`: lane M6c has `open_free` + `resume_strategy` (not merged); the lines are theirs to join (`open_free(core, active, system)` in `begin.rs`, `strategic::after::open_after` in the tail) |
| Executive Order (Keleres), Z'eu, contacts, payments, Thunder's Edge expedition | dropped content (refused by the menu) |

## 4. Hand-overs

* Lane M6c/M6d merge: `tactical/tail.rs` `FinishAction` arm is the same text as theirs (no conflict expected); `strategic/begin.rs::FreeTactical` and the tail's two bails are the join points above.
* Integration: `MenuGap::Action(u8)` is gone (nothing raised it any more); `MenuGap::Unrouted(kind, key)` replaces it. `TurnCodes::extreme_duress` equals `Codes::not_from_supply[2]` (same text); kept as its own field for readability.
* `ask::chain_trade` is the only place the menu arm continues into another arm in the same call; a route that opens another arm's window inside its job should use it (the transaction does).

## 5. Findings of the shadow behind a real game (all fixed, each guarded)

* B6 **Trade primary at a Cabal / Trade-Agreement table** (`rules::strategy_windows::TradePrimary`, `fx_perform` Trade secondary): replenished without telling the table; the string stages `COMMODITIES_REPLENISHED` (Cabal) and the Trade Agreement's gain. Now through `fx::replenish_announced`. Guard: core `action_dispatch::the_trade_primary_stages_cabals_replenish_event_like_the_string` (fails on the old window: `staged == []`), engine lockstep `the_trade_primary_at_a_cabal_table_stages_the_goods_before_the_replenish_like_the_string`.
* B7 **Event order of the strategic Trade primary**: the string stages the three trade goods when it gains them, before the replenishes; the window's report made the parent stage them after. `strategic/perform.rs` stages them first and drops the report. Same guard.
* B8 **Staged events at a table without a module**: the string's `stage_event` is a no-op without a seated module faction; the core's windows stage unconditionally (`STRATEGY_TOKEN_SPENT`, `UNITS_PRODUCED`, trade goods). `live::run_live` drops `Pending::staged` at the end of a step when `facts.staging` is off. Guard: the bridge `the_strategic_action_arms_agree_with_the_legacy_game_for_every_lobby_faction` (sol/hacan/letnev/xxcha), `orbital_drop_at_a_table_without_a_module_announces_nothing_like_the_string`.
* The oracle (`NativeSubject`) now shadows the action-menu ask source and chains it after the turn prelude (one legacy step holds both: the menu is asked and answered in the step that prepares the turn); its render carries the faction texts. `KNOWN_SILENT` got a second entry: `StrategyAbility` (a COUNTED gap, Thunder's Edge Warfare's free tactical action and Doctor Sucaban's research offer); the strict round-one test tolerates exactly that stop.
* `native_play`: the action menu is a native ask source; its test that pinned the old closing-menu gap (`MenuSources`) now asserts the menu is assembled and played.

## 6. The native playthrough (`native_play.sh --quick --shadow`, sol, 4 players, seed 1, te card set)

Before this lane: 11 native steps, then `GAP ask.action_menu` (the menu was not a native ask source). After: the game plays the draft, the turn prelude, the action menu, **Orbital Drop**, and a **tactical action** (activation, two movement steps, four loads, the aftermath's commit steps) natively, 24 steps, no gap. The first
difference is then at step 24 `[aftermath]`: the native aftermath asks Local Fabricators' reward (`gain`) of an exploration; the legacy game, replaying the same answers, was already at the next seat's menu (`legacy script wanted "gain" but d was offered [strategic|pok3politics, strategic|pok5trade, tactical]`). That is the exploration window of lane X / the invasion's first claim (a question the string does not put, or puts elsewhere): not this lane's. Reproduce: `TI4_NATIVE_PLAY_TRACE=1 CARGO_TARGET_DIR=... tools/engine_core/native_play.sh --quick --shadow`.

## 7. Checks run (cheap)

* `cargo test -p ti4-engine-core --lib`: 477 passed. `-p ti4-engine-core-bridge --lib`: 102 passed (incl. the three tests lane H reported, `native_subject` 17, the turn-flow lobby test with the action menu shadowed). `-p ti4-engine --lib` whole: 5135 passed, 0 failed, 7 ignored (before the Orbital Drop route; the Orbital Drop commit re-ran under the section below); `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` over `menu_dual turn_live action_menu action_dispatch strategic_live trade_live d1_ faction_effects_tests::d5`: 187 passed.
* Named core test binaries: `action_dispatch` 5, `strategic_arms` 34, `turn_jobs`, `turn_jobs2`, `turn_end_job`, `status_phase`, `trade_live`, `trade_tcs`, `tactical`, `tactical_composer`, `aftermath_chain`: ok.
* `check_layers.py core`: 512 files, 0 violations. `lint_core.sh`: 514 files, 0 hits.
* Mutation checks of the new core halves (`component.rs`, `faction.rs`, `relic.rs`, `action.rs`): see the end of this file.
* Not run (standing rule): golden/wide dual sweeps, benches, soak, the mutation ratchet, census, workspace clippy, the armed dual, bail-as-failure scenarios beyond the shadow tests above, all 141 core test binaries.

## 8. Mutation check of the new core halves (Haiku runner, `mutants.sh` on the committed tip `0bffecfb`, single files, union of the core tests `action_dispatch turn_jobs2 turn_jobs turn_end_job` and the engine lockstep `action_dispatch_live_tests action_menu_live_tests`, `D4_COVERS_FILE=/dev/null`)

| file | mutants | killed by the union | survivors |
|---|---|---|---|
| `turn/component.rs` | 30 | 22 | `announce_staged` (three: the pending-destruction branch and its halt check: no test stages a ship destruction in a component action), `play_card` (the cancelled-play verdict `Failed{withhold:false}` and the emission-refused halt: needs Sabotage in a lockstep), `flush_staged_events` halt check; the `unwrap_or(0)` default of the goods (marked skip) |
| `turn/faction.rs` | 16 | 9 | Lazax Gate Folding's verdict `true` literals (a failed Lazax is not tested), Orbital Drop's `>= 3` boundary and the `Failed`/`withhold` literals, the key default (marked skip) |
| `turn/relic.rs` | 3 | 2 | the `withhold` literal of a failed purge in the core (the lockstep checks the withholding by the second menu; the survivor is the same verdict under a mutation the first menu does not show) |
| `turn/action.rs` | 14 | 9 | the closing-menu bookkeeping condition `turn_closing.is_some() && !is_free(opt)` (negation and `||`: the second action out of the closing menu is lane F's lockstep, which runs the Finish job, not this step), the `false` literals of `transaction` and `pass` (the unconsumed-answer returns), the key default (marked skip) |

Not closed for lack of time: tests for a destruction announced after a component action, a cancelled card play, a failed Lazax/Orbital Drop, the Orbital Drop 3-resource boundary and the second action taken out of the closing menu. The `unwrap_or` defaults are marked `mutants:skip` with their reasons.
