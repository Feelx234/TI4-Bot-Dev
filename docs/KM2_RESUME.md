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
| faction: Orbital Drop, Production Biomes, Trade Convoys, Blood/Dark Pact, Wormhole Generator, Promise of Protection, Muaat (Nucleus, Star Forge, Inferno, Fires), Naaz (two Fabrications, Black Market Forgery), Yssaril (Stall Tactics, Mageon Implants), kind 11 | windows/functions exist (`faction_component`, `muaat_component`, `technology_windows`, `hero_windows`), facts not yet computed from the core; Nekro C4R, Terraform and Cabal's Vortex are not even listed (the menu gap `MenuGap::Faction`), Vaults of the Heir, Psychospore, Deorbit Barrage are Thunder's Edge (out of scope) |
| Thunder's Edge Warfare's free tactical action, and a hero's strategy primary waiting for the end of a free action | `TurnGap::FreeTactical` in `strategic/begin.rs` and `TacticalGap::{WarfareSecondary,LeaderStrategy}` in `tactical/tail.rs`: lane M6c has `open_free` + `resume_strategy` (not merged); the lines are theirs to join (`open_free(core, active, system)` in `begin.rs`, `strategic::after::open_after` in the tail) |
| Executive Order (Keleres), Z'eu, contacts, payments, Thunder's Edge expedition | dropped content (refused by the menu) |

## 4. Hand-overs

* Lane M6c/M6d merge: `tactical/tail.rs` `FinishAction` arm is the same text as theirs (no conflict expected); `strategic/begin.rs::FreeTactical` and the tail's two bails are the join points above.
* Integration: `MenuGap::Action(u8)` is gone (nothing raised it any more); `MenuGap::Unrouted(kind, key)` replaces it. `TurnCodes::extreme_duress` equals `Codes::not_from_supply[2]` (same text); kept as its own field for readability.
* `ask::chain_trade` is the only place the menu arm continues into another arm in the same call; a route that opens another arm's window inside its job should use it (the transaction does).

## 5. Checks run (cheap) and not run

See the final report of the lane (section "Checks"); the numbers are filled at the end of this file.
