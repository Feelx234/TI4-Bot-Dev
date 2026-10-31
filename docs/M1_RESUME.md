# D2 lane M1 (`core-m1-trade`): the trade island, resume note (2026-10-31)

Branch `core-m1-trade`, cut from `core-d2-c1-status` (`bdb6aae9`). Never pushed; `nightly-fixes-2026-10-26` untouched. Read `docs/D2_LF_RESUME.md` and `docs/D2_LF2_RESUME.md`
(the recipe, `NativeSubject`, `playthrough --quick`) first; the design is section 3.1 of `docs/todo/D2-islands-machines.md` (branch `core-todo-d2c`).

## What is native

`Arm::Trade` (the string's `step_trade` and the opening in `apply_choice`) runs in the core: typed state, composer, opener, close, events, display facts.

| piece | where |
|---|---|
| The payload on the stack: `state::Trade { proposer, partner, rounds_left, deal, free: [Option<FreeSpec>; 2], announced }`. `FreeSpec`/`FreeTerms`/`FreeNote` are the terms of a free-form deal WITHOUT the facts of the position (slot 0: the deal on the table, slot 1: the deal the proposer has just named) | `state/pending_windows.rs` |
| The machine: `rules::trade_window::TradeWindow` is also `Held` (`Window::Trade`): `from_pending` rebuilds it from the payload and the position every call (`live_facts`: reach by the map, Arbiters, unlocked commanders, free-form deals priced and checked against the position NOW), `to_pending` writes it back, `closed` clears the Black Market mark. A live window announces `TRANSACTION_OPENED` in its first call, stages the two `TRADE_GOODS_GAINED` of an accepted deal (and Dark Pact's) only while `facts.staging` (`supply::staging_enabled`: some seated faction has a module), emits the typed `TRANSACTION_RESOLVED` (the reaction windows open there: Lie in Wait, Sabotage ...) and the plain event of the outcome (`TRANSACTION`, `TRANSACTION_REFUSED`, `TRANSACTION_OFFERED`, `COUNTEROFFER`, `TRANSACTION_REJECTED`, `TRANSACTION_ABANDONED`). A window of the dual (`At`, `single`) does none of it | `rules/trade_window.rs` |
| The opener `driver::windows::trade::begin(core, env, proposer, partner)`: note the negotiation, spend 94.1 on OPENING unless a module exempts the pair (`hooks.cards.transaction_limit_exempt`), put the window on the stack, announce `TRANSACTION_OPENED` and run to the first question (a window with nothing to propose is dropped there, the string drops it one step later with the same position). The caller (the action menu's dispatch, lane F) does `settle_extreme_duress` first | `driver/windows/trade.rs` |
| The arm: `advance_trade` / `ask_trade` registered in `driver/live.rs` (one line each) | `driver/windows/trade.rs` |
| The trade queries on a live position: `KernelEnv` (Guild Ships, Trade Convoys, `hooks.cards.transaction_reach`, `transaction_limit_exempt`, Arbiters, Black Market), `with_cx`, `open_partners_live` (the menu source), `lie_in_wait_traders` | `kernel/trade_env.rs` (L3; `rules::trade` re-exports it) |
| Facts that were the adapter's: `note_order` (a rank table pinned to the text by an engine test), `card_order`, `planet_values`, `commander_unlocked_mask`, `free_deal`, `free_nets` | `query/trade_deals.rs` |
| Dark Pact and the stage helper | `rules/trade.rs::dark_pact_gains`, `kernel/fx.rs::stage_gain` |
| Lie in Wait reads the neighbours who traded from the position as it stands when the effect runs (the deal that opened the window was struck in the very call that plays the card); the adapter's `Facts::traders` still wins when it gave one | `rules/action_cards/effects.rs` |
| Engine half: free-form text. `live::trade::pick` reads the proposer's `x;...` id with the string's own reader (`transactions::offer_from_with`), resolves it to the typed tables (`spec_of`), writes slot 1 of the payload and names it with `Pick(Deal(Free(1)))`; a malformed id is a decline (the string: "offered nothing"); a term the tables do not name is the counted gap `TurnGap::TradeTerms`. `live::trade::sync` hands the renderer's table the text of slot 0 and the net the accept option reports | `ti4-engine/src/live/trade.rs`, one hook in `live/mod.rs` |
| `TRANSACTION_OPENED` IS a core `EventKind` (`TransactionOpened`; the reaction table maps `WhenNegotiatingTransaction` to it) | |
| Oracle: `open_arm(Arm::Trade)` (one line), the shadow compare of the events left staged (by name), `Subject::in_transaction` (question 6: the pre-step question of a trade step is not compared; legacy `legal_options()` shows the action menu there) | `bridge/src/{native_subject,oracle}.rs` |

Step boundary (the decision of lane C applies): a native `live_step` is one decision plus everything that follows until the next decision. The string opens the
window in the menu step and asks the first question in the next one; `begin` does both, so the menu pick, `TRANSACTION_OPENED` and the first question are one call.

## Parity departures and findings

* FINDING (string, kept): the string stages the gains of a deal only while some seated faction has a module (`supply::staging_enabled`); Sol, Hacan, Jol-Nar, Letnev, L1Z1X and Xxcha have none. Found by the first lockstep (the core staged unconditionally); the core reads `AdapterFacts::staging`.
* FINDING (string, kept): a window with nothing to propose is dropped WITHOUT clearing the Black Market mark (`step_trade` clears it only after an answer). `closed` does the same.
* FINDING (live resolver): the base lacks the initiative order in `sync_clock` (lane F2 / M6b fix `69eab360`): in the action phase only the active seat saw a reaction window. The one line is taken over verbatim.
* A faction played by two seats, a transaction with no map: unreachable in the product; the window is dropped (no map) or the Support leg is refused (two seats of one faction).
* Diplomacy is not ported (dropped): `consume_initiation` and `FairTransaction` are no-ops with the feature off.
* Free-form deals with an unknown card, note, planet or fragment kind: the string answers with a rejection (it reads the text); the core stops with `TurnGap::TradeTerms` (a counted gap). The web desk cannot send them.

## Not done (with reasons)

* The playthrough's `M1 trade` row counts the DUAL's census of the string engine's windows (`d1::step [bail: trade]`, `d1::pending.open/close [bail: Trade]`); `playthrough` still plays the string `Game`, so it falls only when the wrapper (D2-8) calls `begin`/`live_step`. The native arm is proven by the lockstep, the shadow oracle and the core tests (below).

## Temporal Command Suite (done, as a child of the card's effect)

`ce_movement/nomad/timing.rs`: slot `TCS` (`technology:nomad:tcs:AGENT_EXHAUSTED:after`, named in `reactions_core::NAMED`), optional, on `EventKind::AgentExhausted` After: condition = a Nomad with a ready
TCS and the event's agent still exhausted. Effect, in the string's order: exhaust the card, ready the agent; for another seat's agent and a pair that may open again, the typed question
`Ask::Technology(TcsTransaction { partner })` (`Opt::Transact`, subtype `tcs_transaction`; new variants, rendered byte-exact in `render/technology.rs`), then the mark `NomadTcsNegotiating` (the reach the card grants),
the pair's transaction spent unless a module exempts it, and `TradeWindow::inline` run to its end with `trade_window::run_inline` over the effect's tape (questions are nested questions: a clone mid-negotiation continues identically).
The inline window stages the gains and emits the typed `TRANSACTION_RESOLVED` but announces no opening and logs no plain event (the string's TCS loop); it does not mirror (the outer emission mirrors the journal once).
Who announces `AGENT_EXHAUSTED`: the sites that exhaust an agent. Only the production window's Berekar Berekon stages it today; every other site still raises `Unknown::NomadAgentExhaustion` when a Nomad holds TCS (those sites are other lanes'; each stages `fx::stage(AgentExhausted)` instead of the bail when it adopts this ability).

## Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/core-m1`

* `cargo test -p ti4-engine-core --lib`: 466 passed. `-p ti4-engine-core-tests --test trade_live` (28), `--test trade_tcs` (5), `trade_window`, `trade_ops`, `query_trade*`, `driver_session`: ok.
* `cargo test -p ti4-engine --lib` (default areas): 4987 passed after fixing two tests my change touched (`reactions_core::bail_every_family...` no longer names TCS as unported; `lie_in_wait ... in_the_core_window` needs the Trade slice before the position-read traders: it falls back to the adapter's facts). `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` on `trade nomad dual_window d1_pending reactions window_driver live:: transactions tcs temporal`: 426 passed.
* Lockstep (`ti4-engine/src/trade_live_tests.rs`, 22 scenarios): every deal shape of the grid, notes (sell, gift, buy, swap, Support, Alliance with the commander locked/unlocked, Mahact), haggling (refuse, decline, counter, bounded), Arbiters, Black Market (also played at the opening: a nested question inside `begin`), free-form of every term kind, malformed and unpayable free-form, Guild Ships over strangers, Pride of Kenara (gift, sale, purchase, free-form, mech gone), Dark Pact, Lie in Wait, a window with nothing to propose, the menu partners (`open_partners_live` against `open_partners_text` over strangers, the pair limit, the negotiation limits), Temporal Command Suite (4 scripts). Questions as rendered, events, staged events, position, dice streams.
* Property test (`live::trade::tests`, 4000 generated ids): the typed reading of a free-form id is priced to the last bit like the string's and writes back to a text that reads as the same deal; the generator reaches every term kind; the text-free tables of the core (note order rank, card order, planet worths, negotiation limits) equal the string's text.
* Shadow oracle (`native_subject::tests`): `the_trade_arm_agrees_with_the_legacy_game_over_real_transactions_for_every_lobby_faction`: six tables of four, all 24 lobby factions, three rounds, TI4_BAIL_IS_FAILURE on: 139 trade steps compared (questions, plain events, staged events by name, the position at every step, the close), zero differences; `a_perturbed_native_trade_position_is_reported_as_a_state_difference`.
* `check_layers.py core`: 0 violations; `lint_core.sh`: 0 hits.
* Regression tests (fail on the pre-fix code, checked): staging only with a module seated (`a_commodity_swap...` and `the_gains_of_a_deal_are_staged...`), the initiative order of the live resolver (`the_resolution_opens_its_reaction_window_to_a_seat_that_is_not_the_active_one`: failed with the line commented out), Lie in Wait's neighbours read at the effect (lockstep), the double mirror of the card's transaction (TCS lockstep).
* `playthrough_quick.sh` (gate profile, sol, 4 players, seed 1, 1721 steps, game finished, hash `93978441ccd98b4e` BEFORE and AFTER: the refactor changes no string behaviour): island `M1 trade` 142 counted bails before and after (`d1::step [bail: trade]` 62, `d1::pending.open [bail: Trade]` 40, `d1::pending.close [bail: Trade]` 40). These count the dual's census of windows the STRING engine opens and closes; the bin does not call the native arm yet (D2-8).
* Mutation (`mutants.sh`, Haiku runners, committed snapshots in a scratch worktree, `D4_COVERS_FILE=/dev/null`, two test sets: core tests and engine lockstep): see "Mutation results".

Not run (standing rule): golden/wide dual sweeps, benches, soak, mutation ratchet, census, workspace clippy, the armed dual, the Z-gate.

## Hand-overs

* The dispatch of the action menu (lane F): `settle_extreme_duress`, then `driver::windows::trade::begin(core, env, active, partner)`; the option list from `rules::trade::open_partners_live`.
* Whoever finishes a step calls `live_reconcile` first (the staged gains of a deal are announced there).
* The sites that exhaust an agent while a Nomad holds TCS: stage `AGENT_EXHAUSTED` (see above).
* `Window::Trade` payload gained `free` and `announced` (`Trade::opened(..)`); `d1_windows::trade_of` fills them.
