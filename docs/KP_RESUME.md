# D2 lane B (`core-d2-c2-turn`): turn hooks, phase events and the per-step prelude, resume note (2026-10-31)

Branch `core-d2-c2-turn`, cut from `core-d2-f-live2` (`bfbab082`). Read `docs/D2_LF_RESUME.md` and `docs/D2_LF2_RESUME.md` first (the recipe, `NativeSubject`,
`playthrough --quick`). Never pushed; `nightly-fixes-2026-10-26` untouched. Packages KP0-KP4 of `docs/todo/D2-islands-composition.md` (section 4).

## What is native

One held window carries the whole lane: `WindowKind::TurnFlow` / `Window::TurnFlow(TurnJob)` (`state/turn_job.rs`), machine `driver::turn::TurnFlowWin`
(`Held`). The payload is only WHICH job is open. A job is ONE `Window::step` that composes the windows the string composes, in the string's order, and
asks every question through the tape (`driver/turn/inline.rs::run`): the driver rolls the step back at a question and `driver::live` stores the answers given so
far (`Pending::nested`), so a caller that returns between questions, a clone of the position mid-job and a fresh driver all continue identically. Nothing of a
half-run job is ever kept; a job that fails (a counted gap, an illegal answer) leaves the position as it was, the job it opened included.

| job (`TurnJob`) | string body | where it is entered | what runs |
|---|---|---|---|
| `Reconcile` (KP0) | the reconcile hooks of `Game::step_flow` | `live::live_reconcile` (before every `live_step`) | `starting_tech::settle` (the picks that need no decision), the staged events announced, `announce_gains` (`TECHNOLOGY_GAINED` etc., the seen-marks), the Wormhole Nexus latch, Styx's point, Spec Ops II survival rolls (dice from the state's stream, `RollRec`, `SPEC_OPS_SURVIVED/LOST`) |
| `Prelude` (KP1) | the `turn_prelude` block | `Arm::TurnPrelude` (self-opening) | Psychoarchaeology + Transit Diodes (`TurnStart`), the commander unlocks (`LEADER_UNLOCKED`), Military Support (`PlaceUnits`), Spec Ops return, `prepared_turn_seq`, typed `TURN_BEGAN` |
| `Finish` (KP2) | `Game::finish_action` | `driver::turn::open_finish(core)` by the arm that finished an action | typed `ACTION_COMPLETED` (+ `component`), the action reveals ended, the action counted, the closing menu or the end of the turn |
| `End` (KP2) | `advance_turn_inner` + `pass_on_tail` | `Finish`, the closing menu's "end your turn" (`Arm::Ask`) | retention, Minister of War (`MinisterOfWar`), the per-turn scratch, Predictive Intelligence / Bio-Stims (`TurnEnd`), the legendary planets (`Legendary`), Jupiter Brain's retention, the fleet sweep (`EnforceAll`, limits from the position: `driver/turn/facts.rs`), the turn passes on with `TURN_PASSED` (plain then typed), Crisis' skip, Puppets |
| `Phase` (KP3) | `Game::step_phase` | `Arm::Phase` (self-opening) | typed `STRATEGY_PHASE_ENDED`, `phase::advance_phase` (stock the mat, Political Stability, the faction hooks, the flip), the plain and typed phase events, the round's bookkeeping (`begin_next_round`, the mat from the initiative table, the flags, `ROUND_BEGAN`, the Anti-Intellectual Revolution's levy) |
| `Announce` (KP3) | the `strategy_announce` block | `Arm::StrategyAnnounce` (self-opening) | the flag, typed `STRATEGY_PHASE_BEGAN` |
| `Draft(card)` (KP4) | `take_timed_strategy_card` + the end of the draft | `Arm::Ask` with `AskArm::Draft` | typed `STRATEGY_CARD_CHOSEN` with the card taken between its WHEN and AFTER windows (the goods paid, the `TRADE_GOODS_GAINED` staged), the plain event |

`Arm::Ask` (`driver/turn/ask.rs`) is native for the question sources this lane owns: the draft (`rules::strategy::draft`; new typed question `Ask::Strategy(Draft)`,
`Opt::Card`), the starting technology (`driver/turn/starting.rs`; new `Ask::Technology(StartingTech)`, `Opt::Tech`, `StartRule`; options from `query::tech::researchable_by_prerequisites`
through `KernelHooks`, `settle`/`apply` with `fx_strategy::apply_unit_upgrades`) and the closing menu where the core assembles it (`closing.rs`), plus an open turn-flow
job. Any other source (`ActionMenu`, `Secondary`, ...) stops with the counted gap `TurnGap::AskArm(n)` until its lane registers it in `ask_ask`/`advance_ask` (one `match` arm each).

Registry: `driver/live.rs` `live_arm`/`ask_arm_fn` gained `TurnPrelude`, `StrategyAnnounce`, `Phase` (`driver::turn::{advance_*, ask_job}`) and `Ask` (`ask::{advance_ask, ask_ask}`).
Engine half: `live/turn.rs` (the interned texts `TurnCodes`, the display facts: Predictive Intelligence's pools, Military Support's offer card, the Minister of War's offer,
the starting-technology details, the contexts the string leaves out), `live::live_reconcile`, and two changes to the shared loop in `live/mod.rs`: a question asked
from inside a step (`Pending::nested` set) belongs to the step even without an emission's facts, and `Asking.at` carries the position a question was put in so the display facts read
that one (`driver/live.rs`, one field).

## Gaps (counted, never silent): `Unknown::TurnGap(TurnGap)` -> `live::<arm> [TurnGap(..)]`

| gap | when | who closes it |
|---|---|---|
| `ChaosMapping` | the active seat owns Chaos Mapping at the start of its turn | production lane (`ProduceOne` needs `buildable`, the capacity of the seat's systems) |
| `CommanderUnlock` | a locked commander of arborec/argent/cabal/empyrean/ghost/mahact/mentak/muaat/nekro/nomad/saar/winnu (the `commander_unlocked` hook slot is empty) at the prelude | the hook slots (see "Hook slots" below); delete `COMMANDER_HOOK_MISSING` in `start.rs` afterwards |
| `MenuSources` | the closing menu could hold a free option (a transaction partner, a leader printed "During the action phase", Fleet Logistics' second action) | lane F + trade lane: replace the body of `closing::options` by the menu assembly |
| `AskArm(n)` | `Arm::Ask` for a source of questions nobody registered | the owning lane |
| `ImperialArbiter`, `QuantumDatahub` | the last card of the draft is taken while the law/technology could swap cards | their windows (two questions each) |
| `MapEdits` | a `MapEdit` mark exists (Nova Seed, Riftwalker Meian): the core's map has no edit to replay | the hero lane |
| `Propagation` | Nekro owes Propagation tokens (the reaction is the string's) | Nekro lane |
| `NoRoster`, `OpenedWindow`, `ReplayDiverged`, `EmissionRefused`, `EventCancelled`, `PhaseNotReady`, `PromiseBook`, `LegendaryUnsupported` | defects of the env or position (play facts missing, a child window that opens another, ...) | n/a |

Parity departures by decision: the experiment knobs `STATUS_INCOME_TRADE_GOODS` and `Game::round_income_trade_goods` (trade goods every seat gains as a phase begins; zero in every
played game) are the harness's, not ported; `RELIC_GAINED` and `BREAKTHROUGH_GAINED` carry the player only (no payload value exists for a relic or a breakthrough and no ability of the
core listens); Thunder's Edge space-station control is not reconciled (refused). The lone-card auto-resolve of the draft (`table.auto_resolve`, never journaled) is the table's.

## Hand-overs for the neighbours

* **Whoever finishes an action** (the tactical tail of M6, the secondary of KX, the menu actions of F) calls `driver::turn::open_finish(core)`; the next `live_step(Arm::Ask)` runs the job. M6 says
  "`finish_action` is not run here": this is it.
* **Whoever starts a step** calls `live::live_reconcile(core, &cx, asker)` before `live_step` (the legacy step runs the reconcile hooks first; `NativeSubject` does it already).
* **Lane F**: `closing::options` is the seam of the closing menu; `ask_ask`/`advance_ask` take one arm per source.
* `Window::TurnFlow` is in `Facts::open` (bit 18): `step_arm` does not look at it, `Arm::Ask` does (a suspended job is served by the arm the step was in).

## Hook slots (`commander_unlocked`, `leader_action`): NOT done

Not started: the 12 in-scope commanders each read a different corner of the position (Arborec: 12 ground forces on planets; Argent: 6 units with barrage, cannon or
bombardment; Cabal: units in 3 gravity rifts; Empyrean: neighbour of every other seat, needs the map and the trade `Cx`; Ghost: 3 systems with an alpha/beta wormhole and units;
Mahact: tokens of two other seats in the fleet pool; Mentak: 4 cruisers; Muaat and Winnu: the status the abilities set; Nekro: 3 technologies without the Valefar cards;
Nomad: one scored objective; Saar: 3 space docks) and each needs a variant on BOTH sides that satisfies and one that misses the condition (the `check_leaders` variants do
not reach them). The turn flow depends on it only through `TurnGap::CommanderUnlock` (prelude): a locked commander of those factions stops the prelude with a counted gap
(`start.rs` `COMMANDER_HOOK_MISSING`; the bridge test `the_turn_flow_arms_agree_with_the_legacy_game_for_every_lobby_faction` allows exactly that gap and asserts it is
still counted). Fill the slots, delete the constant and the allowance.

## Checks (cheap), `CARGO_BUILD_JOBS=2`, private target dir `/root/deploytmp/core-d2-b`

See the end of this file.
