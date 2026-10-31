# D2 lane F (`core-d2-c3-menu`): the action menu and its dispatch, resume note (2026-10-31)

Branch `core-d2-c3-menu`, cut from `core-d2-c1-agenda` (`c5cd9ac2`), merged with `core-d2-f2-hooks` (`88aba4fa`, clean; `rules/production/probe.rs` stays: `home.rs` is in the
M2 lineage, which is not in this base) and with `core-m1-trade` (`122bc3af`; two mechanical conflicts: the step.rs registry test and the test-module list of game.rs; taken
because the transactions source is the trade island's `open_partners_live` and the chosen partner opens its window through `driver::windows::trade::begin`). Never pushed;
`nightly-fixes-2026-10-26` untouched. Packages KM0-KM4 of `docs/todo/D2-islands-composition.md` (section 7). User decision applied (2026-10-31): only the LATEST rules version is
supported, no version branches and no old-version gaps anywhere in this lane.

## What is native

| piece | where | evidence |
|---|---|---|
| KM0 option model: `Opt::{Purge, UseRelic, Device, Play{index,card}, Salvaged, Open{with,faction}, Faction(ComponentAction)}` next to the F2 `Leader/Borrowed/Tech`; `Opt::key()/from_key()` (injective u32); byte-exact render and parse (`render/turn.rs`, `render/component.rs` = the id/kind/label table of every faction component action) | `choice/turn.rs` (`ComponentAction` moved here from `kernel::hooks::sites`, re-exported: layering), `kernel/hooks/components.rs` (the codes) | core unit test `every_menu_option_round_trips_through_its_key`, render tests, bridge lockstep below |
| KM1 sources listed by the core: relics (purge, use; Enigmatic Device and Stellar Converter resolvability), the Enigmatic Device cards of the play area, the action cards of the hand (`Gates`, the six printed bills, Seize Artifact, Plagiarize), the transaction partners (trade island), the faction component actions (Orbital Drop, Production Biomes, Trade Convoys, Blood/Dark Pact, Promise of Protection, Wormhole Generator, Lazax Gate Folding, Muaat x4, Naaz x3, Yssaril Stall Tactics and Mageon Implants), plus F2's strategic, leaders, technologies | `driver/menu_cards.rs`, `driver/menu_faction.rs`, `driver/menu_sources.rs`, `ti4-engine/src/menu_sources_view.rs` (position -> lists), `menu_dual.rs` builds the dual's menu from them | bridge `tests/menu_sources.rs`: three tests, 300+ position variants x 6 seats x 3 tables x 5 deals; string and core lists equal option by option (id, kind, label, order), every shape reached (asserted: the faction ids reached are named in the test) |
| The action menu as the live question: all 13 sources in `ORDER`, the tactical entry (`transit::activatable`), the failed-action filter (`Pending::failed_keys`, cleared with `failed_component_actions`), contacts/payments/Thunder's Edge empty (refused content) | `driver/turn/action_menu.rs`, `driver/turn/ask.rs` (`AskArm::ActionMenu`) | engine lockstep `action_menu_live_tests.rs`: question (prompt, ids, kinds, labels, `details`, context), answer, events, position equal to the string's step |
| The closing menu (KM3): `driver::menu::closing_menu` over the entries with `free`/`pass` computed by the core (`is_free`: transaction, leader printed "During the action phase"), the second action; `closing::has_free_leader` and the `TurnGap::MenuSources` gate are gone; `COMMANDER_HOOK_MISSING` deleted | `driver/turn/closing.rs`, `end.rs::run_finish` (roster from the play facts) | lockstep: Fleet Logistics' second action, a free leader; core tests `turn_jobs2` |
| KM4 `with_turn_menu` display facts: `kind`, `closing`, `actions_taken`, `tokens`, `strategy_cards` (with the option that plays each), `partners` (available, in_contact, reason, goods, commodities, notes), `action_card_effects` (Economic Initiative's cultural planets) | `ti4-engine/src/live/menu.rs` | the lockstep compares `details` JSON (the harness `shape` has always included it) |
| KM2 dispatch: `TurnJob::Action(key)`; **pass** end to end (mark, legendary pass window, Prove Endurance occurrence + event scoring, `PLAYER_PASSED` plain and typed, additional action declined, the turn ends); **a transaction partner** opens the trade window (`trade::begin`); the closing-menu second-action bookkeeping | `driver/turn/action.rs`, `ask.rs::run_chosen` | lockstep pass; core test `choosing_a_transaction_partner_from_the_menu_opens_the_trade_window_and_spends_the_pair` |

## Counted gaps (never silent): `Unknown::TurnGap(TurnGap::Menu(MenuGap))`

* `MenuGap::Faction(f)`: the menu cannot be listed for a seat that holds Nekro's `???_ERROR_ERROR_???` (PDS placement spots), a Terraform note (home-planet targets) or Cabal's Vortex (targets over the map). The faction hooks Vaults of the Heir, Psychospore, Deorbit Barrage and Deepgloom Executable are Thunder's Edge breakthroughs: no position of the game offers them.
* `MenuGap::ActionCard`: Signal Jamming in a hand (its systems), and an action card whose legal-target arm is a string-only answer (`has_legal_target` `None`).
* `MenuGap::Action(code)`: the chosen option has no native body yet. Codes (`driver/turn/action.rs::gap_code`): 1 tactical, 2 strategic (entries of lanes M6 and E: they call `open_finish` when the action ends), 5 leader (`use_leader` router), 6 technology, 7 relic, 8 Enigmatic Device card, 9 action card, 11 faction. The step leaves the position as it was.
* `TurnGap::NoRoster`: the play facts carry no unit roster (the menu sources that read the units need it).

## NOT done (honest list)

The task asked for every menu source AND the dispatch of every chosen option natively, and for the 19 faction/Mapped modules. What is left:

* The bodies behind `MenuGap::Action(5..11)`: the `use_leader` router (28 lent sites; the three string-only heroes), `technology::perform_component`, relic/exploration performers, `action_cards::perform` routing, and the faction `perform_component` router over the module windows (`rules::faction_component`, `muaat_component`, `hero_windows`) with `COMPONENT_ACTION_RESOLVED/FAILED` and `Pending::note_failed_key`. Every one needs the facts the string adapters compute (`*Facts`, destinations, placement spots) from the core; none is a one-line wire. The generic wrapper (staged destructions and cards announced, Extreme Duress, `finish_action` through `open_finish`) is also not written.
* The three faction listings that are gaps (Nekro PDS, Terraform, Vortex); the Mapped source is only Cabal's Vortex (+ Saar's Deorbit, out of scope).
* Nucleus and Inferno are listed by the core but no bridge variant reaches them (both sides empty); Signal Jamming's systems.
* Tactical and strategic entries of the menu (lanes M6, E); the dispatch of those two is `Dispatch::{Tactical, Strategic}` -> gap codes 1 and 2.
* The oracle's `NativeSubject` line for the menu (`open_arm`): the menu question is compared by the engine lockstep, not yet by the shadow oracle; bail-as-failure scenarios over real turns with every lobby faction were not run.
* Mutation checks of the new files (`menu_cards.rs`, `menu_faction.rs`, `action_menu.rs`, `action.rs`): not run (disk and time).

## Findings

* `KernelEnv` of the base answered Hacan's Guild Ships `false` (hook `ignores_neighbours` is registered by no module): the transactions source needs the trade island's `kernel::trade_env` (merged). The first lockstep run found it (Hacan seen as no partner).
* Action-phase `neighbors` of the action cards (Plagiarize, Seize Artifact) are the seating neighbours, not the map's: ported as such.
* Pre-existing at the `core-d2-c1-agenda` tip (checked there): core test `hero_windows::a_taker_over_the_hand_limit_discards_through_the_hand_limit_window_unless_the_faction_lifts_it` and its sibling `promissory_windows::mageon_implants_takes_a_single_card_unasked_and_a_hand_over_the_limit_asks_which_card_goes` (a hand over the limit is not asked): lane D's hooked draws.
* The dual census (`d1::step [bail: ask.action_menu]`) counts every string step of that arm; it falls only when the wrapper calls `live_step`.

## Checks (cheap), private target `/root/deploytmp/core-f` (`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0`)



* `cargo test -p ti4-engine-core --lib`: 474 passed. `-p ti4-engine-core-tests`: every binary ok except two failures that are PRE-EXISTING at the `core-d2-c1-agenda` tip (`hero_windows::a_taker_over_the_hand_limit...`, checked there in a scratch worktree, and its sibling `promissory_windows::mageon_implants_takes_a_single_card...`); the binaries this lane changed (`turn_jobs2` 26, `turn_jobs`, `turn_end_job`, `status_phase`, `driver_flow`, `agenda_flow`, `trade_tcs`) pass.
* `cargo test -p ti4-engine --lib` (default areas, whole lib at the end): 5013 passed, 2 failed, 7 ignored; both failures were merge artifacts of `core-m1-trade` (a reaction-table test text reverted by the textual merge; Lie in Wait reads the trade slice, which the play footprint did not load) and are fixed (`reactions_core`, `action_cards_view::PLAY`); the two re-ran green. `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` over `menu_dual turn_live action_menu d1_ faction_effects_tests::d5 trade_live`: 152 passed. The whole lib under all=dual was not re-run.
* `cargo test -p ti4-engine-core-bridge --lib`: 74 of 76 passed before the last fix; the two shadow-oracle tests `the_turn_flow_arms_agree_with_the_legacy_game_for_every_lobby_faction` and `the_status_phase_arms_agree_with_the_legacy_game_for_every_lobby_faction` FAIL: the first on Arborec's Mitosis question (the legacy asks it, the native arm does not: lane B/C territory, reached now because the native `Arm::Ask` carries the game further than before), the second on a staged `TRADE_GOODS_GAINED`/`nomad:artuno` mark in the token gain of Mahact, Naaz, Nomad and Titans tables (status lane). Not verified whether the second fails at the `core-d2-c1-agenda` tip. The shadow oracle counts the legacy's body of a menu option the core does not run yet as lent (`native_subject::menu_body_is_lent`). `--test kernel_env` 3, `--test menu_sources` 3: ok (`play_facts` was built into the same run; result line not seen).
* `check_layers.py core`: 480 files, 0 violations. `lint_core.sh`: 482 files, 0 hits.
* `playthrough_quick.sh --maxsteps 1500` (gate profile, sol, 4 players, seed 1): island F **8 BEFORE and 8 AFTER** (`d1::step [bail: ask.action_menu]` 7, `action_cards::announce` 1; the other islands identical, 2772 in all): the bin plays the string `Game` under all=dual and the dual counts the string's steps, so the table falls only when the wrapper (D2-8) calls `live_step`. The native menu is proven by the lockstep above.
* Mutation checks of the new core halves: NOT run.
* Not run (standing rule): golden/wide dual sweeps, benches, soak, mutation ratchet, census, workspace clippy, the armed dual, bail-as-failure scenarios with every lobby faction over real turns (only the existing shadow tests, above).
