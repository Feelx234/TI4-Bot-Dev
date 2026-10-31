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

* Temporal Command Suite as a trade-window child: see the end of this file.
* The playthrough's `M1 trade` row counts the DUAL's census of the string engine's windows (`d1::step [bail: trade]`, `d1::pending.open/close [bail: Trade]`); `playthrough` still plays the string `Game`, so it falls only when the wrapper (D2-8) calls `begin`/`live_step`. The native arm is proven by the lockstep, the shadow oracle and the core tests (below).
