# TODO D2-islands-machines: the five machine islands, driven by the core (2026-10-31)

Theory document (read-only analysis: no cargo, no builds, no tests were run). Branch `core-todo-d2c`, cut from `core-todo-d2b` (`756c0742`,
`docs/todo/D2-remaining.md`). Tree read: `core-d2-l1` @ `714b0074` (= `core-integration-8` + `docs/D2_8_0_AUDIT.md`, `lent_sites.tsv`,
`window_driver.rs`, `oracle.rs`), through `git show` and the worktree `.claude/worktrees/core-d2-l1`; line numbers are from that tree: re-grep before
editing. Only this file changed. Sibling theory documents cover the three *composition* islands (C1 status/agenda phase machines, C2 turn hooks and
phase events, C3 action-menu dispatch): this file does not repeat them; section 4 lists the contract points where the two meet.

**Labels.** *Measured* = I counted it in the tree. *Cited* = from a document of the tree. *Estimated* = my judgement (agent-hours; calibration of
`D2-prereqs.md` 2.2: window/stage work 0.2-0.35 of document estimates, leaf work 0.5-0.7). **VERIFY** = a 10-minute check the agent does first.
**END-OF-PHASE ONLY** = a heavy run (`/root/heavy2.sh`, `SHARDS=3`, private `CARGO_TARGET_DIR`).

**The rule this plan serves (user, 2026-10-31).** One can play an ENTIRE game in the new engine without any bail, on the agreed content: everything
except Thunder's Edge and the dropped features (Last Bastion, Crimson Rebellion, Keleres, Deepwrought, Firmament, Obsidian, Ral Nel, deck reserve/turn
redo, structured diplomacy). Acceptance: full games over every lobby-seatable faction with a zero bail count. The heavy phase does not START before a
whole game is playable that way (`feedback_gate_only_when_area_complete`, update 2026-10-31). So this plan is about **Architecture A**: the core owns the loop and
opens, drives and closes these five windows; **B (embedded legacy `Game` as lender) is at most a temporary scaffold**, and the *opaque held string
struct* variant of A in `D2_8_0_AUDIT.md` 5 (`Option<InvasionWindow>` next to the `CoreState`) is also not allowed to remain: a string window held by the
wrapper is a lent body that never ends.

---

## STATUS AND AGREEMENT (2026-10-31, lane A2, branch `core-d2-f-live2`): read this before the text below

This document (the machine islands) and `D2-islands-composition.md` (the composition islands) are ONE design; M0 here and KF1/KF2 there are the same contract, built by
lane A and recorded in `docs/D2_LF_RESUME.md` (sections 1-2, the `INTERFACE:` commit `3c31be4d`) and `docs/D2_LF2_RESUME.md`. Differences between the text below and what was built:

| Text below | Built |
|---|---|
| 2.1 persisted form (payload on `Pending` + `Pending.nested` journal), decision D-L0 | decided and built as recommended: payload + journal; proven by `driver::live::tests::a_nested_question_survives_a_dropped_driver_and_continues_to_the_same_position` |
| 2.2 `Held { type Args; type Closed; fn open; fn closed }`, `Live<C>`, `ask -> Option<Choice>` | built smaller: `Held { KIND; from_pending; to_pending; closed(&self,&mut CoreState,&Kernel) {} }`, `Live::{Asks(Box<Asking>), Done}`, `ask -> Result<Option<Asking>, LiveError>`, openers are free functions of the lane (`open(core, &window)`) |
| 2.2 `Env` "built once per game, never per step" | still true for the registry and the table; the position-dependent PLAY FACTS of a card played inside the window are computed from the core at every driver build by `AdapterFacts::plays_provider` (`live::plays`, `live::env::env_with_plays`): lanes whose windows can see a card play build their `Env` with `env_with_plays` |
| 2.3 the arm registry | `live_arm(Arm)` / `ask_arm_fn(Arm)` in `driver/live.rs`: one line each per arm |
| 2.6 the arm-level `dual_window!` lockstep (`live::<arm>`) | NOT built; the oracle's `NativeSubject` (`ti4-engine-core-bridge/src/native_subject.rs`, `oracle --native ARMS`) replaces it: a legacy game runs the arm in shadow per step and compares questions, plain events and positions; a lane adds ONE line to `native_subject::open_arm` (how the observed window opens its core window) |
| 2.7 item 5 bail-as-failure | built (`TI4_BAIL_IS_FAILURE`, `dual::set_bail_is_failure`) |
| 2.7 item 6 coverage counters | built (`live::coverage`, printed by `playthrough`) |
| 5.4 `playthrough` / M7 | built by lane A: `--quick` is the cheap check (a); the per-island table is the work list of each lane; the Z-gate (b) is not started |

How an island lane runs the cheap check: `docs/D2_LF2_RESUME.md` section "Running the playthrough".

---

## 0. Summary and findings that change the plan

1. **The five machines are not "islands with no machine": the core already has the *leaf* windows and no *composer*.** *Measured:* `rules/invasion/` has
   17 leaf windows (`Bombardment`, `BombardTarget`, `Custodians`, `CommitGround`, `LandOne`, `SpaceCannonDefense`, `BeginGroundRound`, `GroundRoll`,
   `GroundRollTail`, `GroundRoundHits`, `Harrow`, `GroundHits`, `AbsorbGround`, `Advance`, `NextCombat`, `FightRound`, `ControlGained`),
   `rules/production/window.rs` has `ProductionWindow`, `rules/combat/machine.rs` has the combat `Machine`, `rules/tactical/mod.rs` has the tactical `Machine`
   with `payload()`/`from_payload()`, `rules/trade_window.rs` has `TradeWindow::{open, at, single}`, `driver/windows/aftermath.rs` has `Head` and `FightOver`.
   What is missing is (a) the **composers** that chain the leaves in the order of the string `InvasionWindow::{settle, resolve, drive}` /
   `ProductionWindow` + `enter_production` + `finish_use` / `AftermathWindow::{new, settle, next_step}`, (b) the **typed state** of the composers
   (`Window::Invasion`, `Window::Production` are `Untyped { data: [u32; 3] }`), (c) a **live arm** that runs them from `Session::step`, and (d) the **facts**
   the leaves still take from string code (`Facts`/`Env`/`PoolIn`/`DealFacts`/`ProduceFacts` built by the adapters from the string position).
2. **There is no live loop at all in the core today.** `driver::step::step` is a free function (there is no `Session` struct); its window arms return
   `Step::Lend(Lent::Arm(arm))`; the core half of every `dual_window!` *replays the string side's recording* (`Driver::run`/`resume` driven by
   `dual_window::drive_positioned`). So "the core drives this window" needs a **new shared mechanism** (section 2: `driver/live.rs`, lane M0). It is not
   covered by the three composition documents; this document owns it, and they must reference it.
3. **The audit counts the combat window as "Portable" and the aftermath's `Fighting` stage as a sub-state. For zero bails it is a sixth machine**:
   `Aftermath::Fighting(CombatWindow)` is driven by `CombatWindow::{settle_open, resolve_answer, pending_choice}` (string) with the core `Machine` only as a
   replay half and with documented bails (`Bail::{RollEndsFight, Unported}`, the W2-bail-02 list). Lane M4 (section 3.4) covers the live drive and the residual
   openings; the coordinator must say whether a leaf lane already owns the W2-bail-02 list (question 3).
4. **Production is bigger than the aftermath's step.** The *end-of-use* effects (`ProductionWindow::finish_use`: `end_value_swap`, Hacan's breakthrough fleet token,
   Prophecy of Ixth discard, `UNITS_PRODUCED` announcement, Self-Assembly Routines) and the *start-of-use* effects (`enter_production`: capacity shortcut,
   `production_seq`, Hacan hero offer, `technology::production_used`, `PRODUCTION_USED`) are **not in the core at all** (`rules/production/window.rs` header and
   `rules/leader_production.rs` header: "declined by the adapter"). They run for every use of PRODUCTION (the aftermath step, Warfare's secondary through
   `production::resolve_timed`, Construction, the Muaat/Arborec leader windows), so the production step is one island with several callers.
5. **Keleres and Deepwrought are dropped, which deletes work**: `agency_supply_network`, `keleres::offer_agent`/`close_agent_window`, the Keleres toll, the
   Keleres rows of `production_view::unported_*` and `coexist_instead` (Deepwrought) are not ported (they stay legacy-only; the core panics if reached).
   Crimson (`Aftermath` Crimson deploy, `can_deploy`) and Last Bastion rows likewise. *Free-form trade deals are PORTED* (user, 2026-10-29: the web trade desk
   sends them): they are a bail in `B4c-bail-01` today and must become typed (section 3.1).
6. **Contact pause (`Aftermath::Negotiating`) is dead in the agreed product**: `available_contacts` is empty with diplomacy off and structured diplomacy is
   dropped (`driver::aftermath::Settled::Fighting` doc: "never with diplomacy off"). The live arm does not implement the pause; the enum variant stays for the
   dual until D4. **VERIFY** (10 minutes): `diplomacy::candidates::available_contacts` returns `[]` when `state.diplomacy.enabled` is false.
7. **Estimate.** This work is **~105-170 agent-hours (estimated)**, not the 25-40 h of `D2_8_0_AUDIT.md` 6 for "8e with opaque holders": that figure kept the string windows
   alive. Section 8.

---

## 1. Facts the plan stands on (measured on `core-d2-l1` @ `714b0074`)

| Fact | Evidence |
|---|---|
| `state::Window` (`state/pending.rs`) has typed variants `Tactical(Tactical)`, `Aftermath(Box<Aftermath>)`, `Combat(Box<CombatFields>)`, `Trade(Trade)` and `Untyped { kind, actor, data: [u32;3] }` for `Invasion`/`Production`; `WindowKind` has 18 kinds; `Pending { staged, stack, top, tag, driver, turn_closing, actions_this_turn, prepared_turn_seq, resolver }`; at most one window per kind; `Pending::{open, close, window, window_mut}` | `state/pending.rs` |
| `Aftermath` payload = `{ player, system, stage: AStage, before_combat: Before, feats_noted, event_scoring: Option<(u64, PauseLimit)>, notes_at_start }`; `AStage::{Fighting, Negotiating{invade}, Invading, Producing, Done}`; the live sub-window is a *separate* stack entry above it (`Window::Combat`) | `state/aftermath.rs` |
| `Tactical` payload = `{ player, stage: TStage::{Activating, Moving, Loading{origin, ship, path, loaded, closed}}, notes_at_start }`; `rules::tactical::Machine::{activating, moving, loading, closing, free, payload, from_payload, single, with_route, done_next}`; `Bail::{NoGalaxy, Route}` | `state/tactical.rs`, `rules/tactical/mod.rs:113-262` |
| `Trade` payload = `{ proposer, partner, rounds_left, deal: Option<Shape> }`; `TradeWindow::{open(proposer, partner, DealFacts), at(.., deal, rounds_left, facts), single()}` implements `Window` and `accepts_unlisted`; `DealFacts { note_order, card_order, unlocked, pair, may, arbiters, planet_values, free: [Option<FreeDeal>; 2] }` | `rules/trade_window.rs`, `query/trade_deals.rs:28-48` |
| `rules::trade` has `record_transaction`, `note_negotiation`, `record_resolved`, `clear_turn_transactions`, `clear_action_negotiations`, `reset_round`, `exchange`; `query::trade` has `are_neighbours`, `neighbours`, `neighbours_who_transacted`, `partners`, `may_transact`, `may_open_again`, `open_partners`, `why_illegal(_with)` (all `fn(&Cx, Seat..)`): **the 16 read/bookkeeping fallbacks of the trade sites (9 `dual_typed!`, 4 `dual!`, 3 `dual::write`) have a typed implementation** | `rules/trade.rs`, `query/trade.rs` |
| `Driver<W>`: `new`, `from_state(window, &CoreState, registry, table)`, `store(&mut CoreState)` (writes `pending.resolver` + trace counter), `with_seeds`, `run`, `resume`, `emitted`, `opened`, `nested_facts`, `unknown`, `is_done`. A nested question (asked from inside an emission) rolls the run back to its start and returns `Awaiting::Nested(op)` with the **`Tape`** (`answers: Vec<Pick>`, private) | `driver/mod.rs:101-340`, `kernel/ability.rs:593` |
| `driver::step::step(core, &Facts, Needs) -> Step` runs 3 arms; every window arm is `Step::Lend(Lent::Arm(arm))` (test `a_finished_game_stops_the_loop_and_an_open_window_is_lent_by_its_arm`); `step_arm`/`ask_arm` are pure functions of `Facts`; `ask_arm` has **no Trade arm** (test `..._without_trade_...`) | `driver/step.rs:56-95`, `driver/session.rs` |
| Composed core windows that already RUN (in the dual): `Head` (enforcement + cannon offense + `Before` + opening writes), `FightOver`, `EnforceAll`, `SpaceCannonOffense`, `Enforce`, `Minister`; `driver::aftermath::{next, announced, note_feats, opens_invasion}`; `driver::tactical_tail::{then, det_fires, crown_offers, tro_window}` | `driver/windows/*`, `driver/aftermath.rs`, `driver/tactical_tail.rs` |
| Core `Window` trait: `stage`, `step(&mut Sx) -> Next`, `ask_of(&CoreState, &Kernel) -> Option<Choice>`, `answer(&mut Sx, Pick)`, `accepts_unlisted`; `Child<W>` with `idle/start/running/step/ask_of/answer/window`; `Next::{Done, Open(Window), Emit, Continue(Ask), Chain}` (**`Next::Open` is how a window asks the driver to open another**) | `kernel/stage.rs:38-62,242`, `kernel/next.rs` |
| String side: `Game.{tactical, aftermath, trade}` private `Option` fields; `Aftermath::{Fighting(Box<CombatWindow>), Negotiating, Invading(Box<InvasionWindow>), Producing(Box<ProductionWindow>), Done}`; `InvasionWindow` has 16 fields and 9 `Stage`s (`invasion.rs:2751`, `:2645`), `ProductionWindow` 19 fields and 4 stages (`production.rs:2483`, `:2458`), `TradeWindow` 4 fields (`transactions.rs:2125`) | code |
| String openers/closers: `apply_choice` opens `Trade` (`game.rs:3385`, after `settle_extreme_duress` + `note_negotiation`, then emits typed `TRANSACTION_OPENED`) and `Tactical` (`:3436`, plain `TACTICAL_ACTION_BEGAN`, `notes_at_start = combat::note_holdings`); `step_tactical :3669`, `apply_tactical_windowed :3763`, `begin_free_tactical :4110`, `finish_tactical :5124`, `finish_head(_windowed) :5149/:5216`, `finish_tactical_inner :5257` (opens `AftermathWindow::new`/`at_commit_step`), `step_aftermath(_inner) :5359/:5409`, `close_tactical :5578`, `step_trade :5890` | `game.rs` |
| `AftermathWindow::{new :218, head :252, at_commit_step :535, fight_over :569, next_step :747, enter_production :813, settle :898}`; `Window for AftermathWindow :1036` | `game.rs` |
| Typed state that already lives in `CoreState` and that the string windows write: `transient.invasion.active: Option<Cow<ActiveInvasion>>` (`ActiveInvasion { system, invader, seq, phase, planet, defender, ground_round, last_step }`, the public boundary), `transient.combat.active_space_combat`, `meta.production_seq`, `meta.combat_round_seq`, `player.production_timers.free_production_use`, `pending.staged` | `state/transient/invasion.rs`, `state/meta.rs:71`, `state/player/production_timers.rs` |
| String-struct rebuilders / comparison keys (the "back" functions): `d1_action.rs` `tactical_of :89`, `tactical_back :113` (returns a `TKey`), `aftermath_of :169`, `aftermath_back :201` (returns an `AKey`), `sub_of`, `combat_key`; `d1_windows.rs` `trade_of`, `trade_key :297`, `trade_key_of :302`; `dual_window.rs` `fields_of`, `fields_back :4009` (returns `FieldsIn`, the machine's inputs) | `ti4-engine/src` |
| `lent_sites.tsv` (484 rows): `invasion.rs` 34, `production.rs` 27 plus its start/end-of-use neighbours outside the file (`leaders.rs:984 production::hero_offer`, `technology.rs:636 production_used`, `:736 self_assembly`, `strategy_cards.rs:2146 construction_*`, `faction_abilities.rs:573 production_biomes`; the audit's 31 is a rounded group count), `transactions.rs` 17, `tactical.rs` 3 + `game.rs` tactical 3, `game.rs` aftermath 3 + `combat.rs` `space_cannon_offense` + `fleet.rs` 2 | `tools/engine_core/lent_sites.tsv` |
| Oracle: `Subject { new, options, answer, step, state_json, events_since, decisions_since, rng_positions, finished, round, rules_version, disabled, decisions_hash }`, `MaskSubject<LegacyMask|CoreMask>`, `Pilot::{Bot, Script}`, categories `Decision, DecisionHash, Value, Event, Rng, State`, label `oracle::<subtype>` | `core-bridge/src/oracle.rs` |
| `window_driver::{driver_for, armed, with_armed, set_armed}`, env `TI4_DRIVER_ARMED=1`: the registry-armed driver (D2-8a); 100 production `Driver::new` sites go through `driver_for` | `ti4-engine/src/window_driver.rs` |
| Process switches: `TI4_CORE_STRICT` (`product::strict`), `TI4_UNMAPPED`, `TI4_DISABLED`, `TI4_BAIL_PROBE`, `TI4_DUAL_FAIL_FAST`, `TI4_CHURN`, `TI4_AREAS`. **There is no "a bail is a failure" switch** | `grep env::var` |

---

## 2. What "the core drives this window" means (shared contract, lane M0)

For each of the five windows, "the core drives it" means exactly these six things, all in the core crate plus the wrapper's glue; nothing of the string window
struct is held, rebuilt or lent afterwards.

### 2.1 Persisted form: payload on the `Pending` stack, plus the nested journal

* The **only** holder of an open machine is `CoreState.pending.stack` (a `Window` variant with a typed, text-free, `Clone + PartialEq + Eq` payload). The wrapper
  holds no `tactical`/`aftermath`/`trade` field. Checkpoint = `CoreState::clone` (the stated end state, `ENGINE_CORE_STATE.md` 9.4).
* The machine **object** (with its `Child<..>` fields) is rebuilt at every `answer()` from `(payload, position)`: the pattern of `tactical::Machine::from_payload`
  and `combat::Machine::awaiting(env, fields)`. A machine is therefore a function of `(CoreState, payload)`; a payload field exists only for what the
  position cannot re-derive.
* A question asked from **inside an emission** (a reaction window, an ability) is the driver's `Awaiting::Nested(op)` + `Tape`. Its persistence is new:
  add `Pending.nested: Option<Nested { op: NestedOp, answers: SmallList<Pick, 8> }>` (`NestedOp::{Step, Answer(Pick)}`; `Pick` is `Copy`). The machine payload at that
  moment is the one the driver rolled back to (the op's start). Re-creating the driver = `Driver::from_state(window, core, registry, table)` +
  `Driver::resume_nested(op, answers)` (new; the tape's `answers` field is private: add `Tape::from_answers`/`Tape::answers()`). The replay is deterministic
  because the state's rng streams roll back with the state (`Driver::settle` snapshot) and the dice come from `Sx::die`.
* Children that are mid-question are *stages of the composer*: the composer's `ask_of` delegates to the running child, and the payload carries a child tag plus
  that child's plain fields where the child holds data the position cannot give (the bombardment cursor, the commit pool's origins, a production `Paying{..}`).
  A child that only asks through reactions (the ground-round dice, rerolls, hit windows) needs no payload: its questions are nested questions, i.e. the journal.
* **Design decision D-L0 (for you, question 1):** this (typed payload + nested journal) versus holding the live `Driver<W>` objects in a wrapper `LiveSlot`
  (cloned in checkpoints, rebuilt by decision-log replay on recovery). I recommend the payload: it is what D1a/D1b built (`d1::pending` holds and compares it every
  step), it keeps `Game::state()`/oracle comparisons on `CoreState` alone and keeps the checkpoint the plain clone. Fallback if the journal replay proves
  unsound for one child: that child alone is held as a `Clone` object in a `Pending.live: Vec<Box<dyn ErasedWindow + Clone>>` (documented, one owner, one test).

### 2.2 The live functions (new module `driver/live.rs`, L5; names are proposals, the semantics are the contract)

```rust
/// What a held machine adds to `Window`: its stack kind, how it is read back from the position and written to it.
pub trait Held: Window {
    const KIND: WindowKind;
    type Args;                                                      // what the opener needs
    type Closed;                                                    // what the close hands to the next arm
    fn open(core: &mut CoreState, k: &Kernel, args: Self::Args) -> Result<Self, LiveError>;   // the opener (writes the opening events/state)
    fn from_pending(core: &CoreState, k: &Kernel) -> Option<Self>;  // payload + position -> machine (None: no such window open)
    fn to_pending(&self, core: &mut CoreState);                     // machine -> payload on the stack
    fn closed(&self) -> Option<Self::Closed>;
}
pub enum Live<C> { Asks(Choice), Done(C) }                          // a question for the table, or the window closed
pub struct Env<'a> { pub kernel: &'a Kernel, pub registry: Cow<'a, Registry>, pub table: AbilityTable, pub seeds: Rc<dyn SeedSource> }
/// Run the open machine of kind `M` to its next question or its end; with `answer` first apply it. Writes payload, `pending.resolver`
/// (`Driver::store`) and `pending.nested`; returns the events the machine emitted (`Driver::emitted`) and the windows it asked to open (`Driver::opened`).
pub fn advance<M: Held>(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live<M::Closed>, Out), LiveError>;
pub fn ask<M: Held>(core: &CoreState, env: &Env<'_>) -> Option<Choice>;   // `legal_options` of this window: `M::from_pending(..)?.ask_of(..)`
```

`Out { emitted: Vec<EventRec>, opened: Vec<state::Window>, plain: Vec<PlainRec> }`. `LiveError` = `Illegal` (the pick is no option; the state is untouched), `Contract`,
`Timing`, `Unmapped(label)` (a position a leaf still refuses: **in the product this is the bail**, section 5). `Env` is built once per game by the wrapper
(D2-8a's `RegistryCache` + `reactions_core::table`), never per step.

### 2.3 The `Session::step` arm

`driver::step::step`'s window arms stop returning `Step::Lend(Lent::Arm(arm))`: `Arm::Trade`, `Arm::Tactical`, `Arm::Aftermath` call `live::advance::<TradeWindow..>` etc.
and return `Step::Ran { arm, lent: None }` (or `Step::Asks(AskArm::..)` while a question is pending). Concretely: (1) add `AskArm::Trade` (today a trade question is asked
"through its own step", `ask_arm` has no arm for it) and fix the test that pins `ask_arm(with(&[K::Trade])) == ActionMenu`; (2) `Needs` gains the `Env` handle;
(3) `Lent::Arm(Arm::Trade|Tactical|Aftermath)` is removed from the enum last, as the compiler's proof that no caller still lends them. **Wrapper
semantics** (D2-8d, not this document): `Game::step()` stays pull-style, as the string's: on `Step::Asks` it asks the table, then calls `advance(.., Some(pick))`.

### 2.4 The facts builders (the part that is really string today)

Each leaf takes a `Facts`/`Env` the adapter builds from the *string* position (`cannon_dual::Args`, `tactical::Facts { pool: Code }`, `combat::Env`, `begin::Facts`,
`PoolIn`, `DealFacts`, `ProduceFacts`, `window_env`, `core_invasion::InvasionView`). In the flip these are functions of `(CoreState, Kernel)` only:
`fn facts_of(core: &CoreState, k: &Kernel, ..) -> Facts`, one per leaf, each replacing the adapter's string reads. Texts the core has no text for (`Code` interning of
`tactic`, `pre_roll`, event keys) come from `core.texts` (D2-3, in `CoreState`), minted by a `Codes` helper at game start. The `None` returns of the typed
views (`InvasionView::of -> Option`, `unported` predicates) become `Err(LiveError::Unmapped(why))`, which the zero-bail playthrough counts (section 5).

### 2.5 What is never built after the flip (and what stays only in legacy)

`tactical_back`/`aftermath_back`/`trade_key_of`/`fields_back` and the `*_of` observers exist so the dual can *compare* a typed copy with a string field. After the flip
there is no string field: they are compare-only code that moves to `ti4-engine-legacy` with the dual wiring (D2-7/D2-9) and **must not be turned into
`TacticalWindow`/`AftermathWindow`/`TradeWindow` builders**. Rebuilding a string struct from the typed payload is exactly the lend this plan removes (T-M of
`D2-remaining.md`: `Untyped` cannot carry a string window). The one place a string struct is still built from core data in the *scaffold* phase is the lend of a
leaf gate (`reverse_lend::with_string` with `Args::Typed`); each island below ends with its list of such lends and their exit.

### 2.6 How it is tested before the wrapper exists: the arm-level lockstep

The wrapper (`ti4_engine::Game` over `CoreState`, D2-8b..d) is not there yet, and the island lanes must not wait for it. Use the dual's own mechanism one level up:
wrap the **whole arm body** in the string engine (`step_aftermath_inner`, `step_trade`, `step_tactical`, `finish_tactical_inner`) in
`dual_window!(Area::Driver, "live::<arm>", state, string closure, core closure)`; the core closure runs `live::advance` on a copy of the state with the string
side's `Recording` (the table's answers, in order, nested ones included) and the dual compares the question sequence, the state, the events, the rng positions
(`dual_window::drive_positioned`, unchanged). This is *the same* comparison the oracle makes at step granularity, available today, per arm, with the string
still running. When the wrapper lands, the oracle (`MaskSubject<CoreMask>` replaced by the wrapper `Game`) makes the same comparison on whole games and the
`live::*` wrappers are deleted with the dual. Label convention: `live::trade`, `live::tactical`, `live::aftermath` (+ `live::invasion`, `live::production`,
`live::combat` for the sub-windows inside the aftermath arm).

### 2.7 M0 deliverables (lane M0, first, small)

1. `state/pending.rs`: replace `Untyped` use for `Invasion`/`Production` by `Window::Invasion(Box<state::Invasion>)` and `Window::Production(Box<state::ProductionUse>)` with
   **stub payload types** in new files `state/invasion.rs`, `state/production.rs` (the lanes M3/M2 fill them); add `Pending.nested`. Files that name the
   `Window` variants and must follow (*measured*, non-test): `state/pending.rs` (`Window::kind`), `d1_pending.rs` (14 mentions: `observe`, `roundtrip`), `d1_action.rs` (`sub_of`), `d1_windows.rs`, `dual_window.rs`, `lend_table.rs`, `combat.rs`, `driver/step.rs` tests.
2. `driver/live.rs` (the trait, `advance`, `ask`, `Env`, `Out`, `LiveError`), `Driver::resume_nested`, `Tape::{from_answers, answers}`; unit tests on a toy window with
   a reaction that asks (journal round trip: ask, drop the driver, rebuild from `Pending`, answer, same state as the uninterrupted run).
3. `driver/step.rs`/`session.rs`: `AskArm::Trade`, the three window arms call a registry `fn live_arm(arm) -> Option<LiveFn>` that is `None` until the lane fills it
   (so `Lend` is still returned and nothing changes; each lane flips its own arm and deletes its `Lend` variant).
4. `ti4-engine/src/d1_live.rs`: the `dual_window!` arm wrapper (2.6) behind `Area::Driver`; generic over the arm; counts compared/bailed per label like every window.
5. **Bail-as-failure** (the acceptance switch, section 5): `TI4_BAIL_IS_FAILURE=1` in `dual.rs` turns `note_bail`, `note_gate`, `Lent::*` and every `Unmapped` of a live arm into a
   `Difference` of category `Bail`, label `bail::<label>`, so `dual_run` exits 1 on the first one and the shard report lists them. Default off. `dual-all` itself must stay
   un-strict (T8 of `D2-remaining.md`: strict turns a counted bail into a panic and empties the probe); the new switch is a *report*, not a panic.
6. Coverage counters (section 5, T-Z): per arm and per leaf, "times the core live path ran". Without them a zero bail count is vacuous.

Estimate M0: **8-12 h** (estimated); the interface commit (items 1 and 3 and the trait of 2) is ~1-2 h and unblocks the other lanes.

---

## 3. The islands, in the order they are built

Build order is bottom-up (a composer needs the thing it composes): **M0 shared -> I5 trade (independent) -> I2 production step -> I1 invasion -> combat live (M4) ->
I3 aftermath chain -> I4 tactical.** M1, M2, M3, M6 can start in parallel after M0's interface commit; M4 and M5 consume M2/M3. Each section: definition of done, the typed state
(which string struct it replaces), the ordered tasks (files and signatures), the residual gates (the zero-bail work), tests, cheap vs END-OF-PHASE, traps, estimate.

### 3.1 I5 Trade window (17 sites) - lane M1

**Done when:** an open transaction lives as `Window::Trade` on the stack, is opened by a core opener, asked/answered/closed by `live::advance::<TradeWindow>`, free-form and
Pride-of-Kenara deals included; `Game.trade`, `TradeWindow`, `TradeParts` and `trade_window_dual::step` are not on the live path; the 17 sites are core calls or `Unmapped`.

**State moved into `CoreState`.** `state::Trade { proposer, partner, rounds_left, deal: Option<Shape> }` is complete for the standard deals (the terms are a function of the shape and
the position: `query::trade_deals::offer_of`). Missing: the **free-form deal's terms** (`Shape::Free(i)` indexes `DealFacts.free`, a table the adapter fills from the id text,
`trade_window_dual::register_free`) and the **net the partner's accept option reports** (the string's own number; `Render::register_free`). Add `free: Option<FreeDeal>` to the payload
(move `FreeDeal { given: Terms, received: Terms }` from `query::trade_deals` to `state` or re-export through it; layering: `state` is L1, `query` L2) and a `net: i32`. The id text of a
free-form deal is `FREE_FORM_PREFIX` + terms (`x;gt=2;rc=1;..`, `W15-design-01`): the live pick for the proposer's unlisted answer is `Pick::Opt(Opt::Deal(Shape::Free(0)))` with the terms parsed by
a new pure `rules::trade_window::parse_free(&CoreState, &str, proposer, partner) -> Option<(FreeDeal, net)>` (a port of `transactions::offer_from` + `trade_core::terms` +
`Offer::{worth_to_receiver, cost_to_giver}`; the string functions are the oracle for a property test over generated ids). **VERIFY** how a typed `Pick` reaches the core for an unlisted pick:
`Window::accepts_unlisted` exists; the wire-to-`Pick` parse (`render::trade::parse_pick`) is the wrapper's, but the payload must carry the parsed deal so a rebuilt machine can answer it.

**String struct replaced / string code that stops being called.** `TradeWindow` (`transactions.rs:2125`), `TradeParts`, `trade_of`/`trade_key`/`trade_key_of` (`d1_windows.rs`),
`Game::step_trade` (`game.rs:5890`), the `apply_choice` trade branch (`:3376-3408`), `transactions::{answer_with, resolve, open_with_content, record_unless_exempt}`.

**Ordered tasks.**
1. `DealFacts` from the core alone: `fn deal_facts(core: &CoreState, k: &Kernel, cx: &Cx<'_>, proposer, partner) -> DealFacts` in `rules/trade_window.rs` (or `driver/live/trade.rs`):
   `unlocked` = bit per seat whose commander is unlocked (`players().leaders.status`, the `promissory::commander_unlocked` rule; **VERIFY** the Alliance/Mahact "held another way" cases),
   `may` = `query::trade::may_transact(cx, a, b)` both ways (needs the `Cx` with adjacency/galaxy: `core.map` holds the galaxy, `rules::tactical::Bail::NoGalaxy` shows the failure mode),
   `arbiters` = `transactions::trades_action_cards`: the faction's content record lists the ability `arbiters` (Hacan), i.e. a faction-table lookup in the core (**VERIFY** a generated flag exists, else add one),
   `planet_values` = a `static` from the generated planet table, `note_order`/`card_order` = the text-order comparators (already `fn` pointers in the adapter: move them next to `DealFacts`).
2. Opener `fn open_trade(core, env, proposer: Seat, partner: Seat) -> Result<(), LiveError>`: exactly the string order (`game.rs:3376-3407`): `note_negotiation` (core `rules::trade::note_negotiation`),
   record the pair unless exempt (`hooks_cards::transaction_exempt_from_limit` -> a ground/cards hook in the factions crate: **VERIFY**, port if missing) via `rules::trade::record_transaction`, push
   `Window::Trade { rounds_left: 2, deal: None }`, emit the typed `TRANSACTION_OPENED` (payload `player`, `partner`; **VERIFY** the `EventKind` exists, `TransactionResolved` does) so the card played "when you negotiate" has a window.
   `settle_extreme_duress` (the string call before the opener) is the dispatch lane's (C3): the opener documents it as a precondition. Diplomacy's `consume_initiation` is not ported (dropped).
3. `impl Held for TradeWindow` (+`Args = (Seat, Seat)`): `from_pending` = `TradeWindow::at(proposer, partner, deal, rounds_left, deal_facts(..)).single()`; `to_pending` writes `rounds_left`, `deal`, the free deal.
   `single()` is kept (one answer per `advance`; the stage question is the window's `ask_of`).
4. The close, from `step_trade :5890-5945`: on `Outcome::Resolved` emit the typed `TRANSACTION_RESOLVED` (`resolved_payload(proposer, partner)`) **after** the staged `TRADE_GOODS_GAINED`
   events of both sides and Dark Pact's gains are announced (`fx::announce_staged`, the staged queue is `pending.staged`; these two are the "stays with the string" items in `trade_window.rs`'s header: port them
   into the window's `answer` or the close: **VERIFY** which events the string stages: `trade_core::exchange` + `supply::stage_*`); push the plain event of the outcome (`TRANSACTION`, `TRANSACTION_REFUSED`,
   `TRANSACTION_OFFERED`, `COUNTEROFFER`, `TRANSACTION_REJECTED`, `TRANSACTION_ABANDONED` -> `PlainKind`s); when complete, close the stack entry and clear `transient.misc.flags` `BLACK_MARKET` (the string's `transient_flags`).
5. The 16 fallbacks (`are_neighbours`, `present_together`, `neighbours_who_transacted`, `neighbours`, `partners`, `may_transact`, `why_illegal`, `open_partners`, `may_open_again`, `settle`, `record_resolved`,
   `record_transaction`, `note_negotiation`, `clear_turn_transactions`, `clear_action_negotiations`, `reset_round`): in the flip each is a direct call to the `query::trade`/`rules::trade` function above (they are the typed
   arms of the `dual_typed!`/`dual!`/`dual::write` macros already). Do nothing in this lane except confirm, per site, that no caller still depends on the string arm's `None` branch; the call-site rewiring is D2-8e's
   mechanical part. The menu source `open_partners` is C3's input: provide `query::trade::open_partners` through the facts builder.
6. `Arm::Trade` live: `live_arm(Trade)`; `AskArm::Trade`.
7. Product assertions (`product::unreachable_here`, L19): "a transaction window inside a battle or an invasion", "a transaction with no map", "a faction played by two seats" are asserted in `open_trade`
   (not bails): they are positions the lobby cannot create.

**Residual gates (zero-bail).** B4c-bail-01 free-form (task above), B4c-bail-02 Pride of Kenara planet card (Hacan mech from rules version 17: `Shape` for `kp`/`kq`; add the shapes to `choice::trade::Shape` and
`render::trade`, price in `query::trade_deals`), B4c-bail-05 "a deal on the table the core has no `Shape` for", B4-bail-02/03 (note writes for an unknown id, `exchange` for a planet-card leg and a note
without typed id: the planet-card leg is the same Kenara/Free work), B4c-bail-03/04 are product assertions / dropped (two seats of one faction; diplomacy on).

**Tests.** (1) `live::trade` arm lockstep over a transaction of every `Shape` (existing `trade_window_tests.rs` positions) and free-form deals of every term kind (reuse
`free_form_deals_with_every_kind_of_term_agree_with_the_string`); (2) payload identity: `from_pending(to_pending(m)) == m` after each answer; (3) an interrupted journal: a deal whose acceptance opens a reaction
window (Lie in Wait / a card in `TRANSACTION_RESOLVED`'s After) asks, the driver is dropped, rebuilt from `Pending`, answered, same state; (4) opener parity: event order `TRANSACTION_OPENED` before the first question;
(5) mutation check of `deal_facts`, `parse_free`, the close; (6) property test `parse_free` vs `transactions::offer_from` on generated ids (string stays callable as an oracle until D2-7); (7) regression test per gate failure.
**Cheap:** all of the above, plus `dual_run profile-20.json 20 driver=dual --game 0`. **END-OF-PHASE ONLY:** the 20-game armed dual including trade; counts of free-form deals in the corpora are 0 (the bot never
proposes them): the property test and the unit lockstep are the cover, a scripted free-form game in the playthrough (section 5) is the end-to-end proof.

**Traps.** T-I5-1 the string `legal_options()` has no trade branch (`:2261-2330`; `step_trade` asks the table itself), so the legacy subject of the oracle shows the *action menu* before a trade step, the new engine the proposal:
compare the settled decision (`DecisionHash`) for steps with a trade open and skip the pre-step `Decision` compare there, or give legacy's `legal_options` a trade branch (a view-only edit; your call, question 6).
T-I5-2 `DealFacts.pair` is the window's chairs "when it was built"; a rebuilt machine must rebuild `may`/`arbiters` for the *current* position (the string recomputes at each step) - do not cache across steps.
T-I5-3 the partner's accept option reports "the string's own number" for a free-form deal: keep the number in the payload, never recompute it from the terms.
T-I5-4 `record_unless_exempt` writes before the window exists; an `Illegal` pick later must not leave the pair unrecorded (the string spends 94.1 on *opening*): put the write in the opener, not in the first `answer`.
**Estimate (estimated): 8-12 h** (facts builder 2, opener/close 3, free-form + Kenara 3-5, tests 2).

### 3.2 I2 Production step (31 sites) - lane M2

**Done when:** every use of PRODUCTION (the aftermath step, Warfare's secondary/primary `production::resolve_timed`, Construction, Muaat/Arborec leader windows, Chaos Mapping, Sling Relay,
Integrated Economy) opens through one core composer `ProductionUse` (start effects, the `ProductionWindow`, end-of-use effects), held as `Window::Production` while it asks, with its facts computed from `CoreState`;
`enter_production`, `ProductionWindow::{new, refresh, to_core, finish_use}` and `production_window_dual::step` are not on the live path.

**State moved into `CoreState`.** `state::ProductionUse` (`state/production.rs`): the fields of the core `ProductionWindow` (`who, system, stage: Choosing|Paying{unit, owed, made, cost, paid}|Placing{unit, made}|Done, limit, remaining,
free_capacity, small_exemptions, credit, discount_remaining, fleet_limit, armada, free_this_use, flagship_free, only_unit, max_unit_cost, buildable: [Option<Unit>; 9], produced: SmallList<(Unit, Spot), 12>, purchased,
placement_timing_done, agent_rules, agent_spent, emit_timing, ability`) plus the composer's own: `phase: Starting|Using|Finishing|Done`, `settled: bool` (end-of-use fired), `value_swap_active`. All of it is plain data
(the core window already derives `Clone`): the payload is a mirror struct (`state` may not import `rules`), with `From` both ways, tested for identity. Already in `CoreState`: `meta.production_seq`,
`player.production_timers.free_production_use`, `production_discount_remaining` (**VERIFY** the core name), `pending.staged`.

**String struct replaced.** `ProductionWindow` (19 fields, `production.rs:2483`), `Stage` (`:2458`), `WindowSnap`, `ProductionReport`, `Aftermath::Producing(Box<ProductionWindow>)`, `enter_production` (`game.rs:813`),
`resolve_timed` (`production.rs:4236`), `to_core` (the string->core converter, `:~3990`).

**Ordered tasks.**
1. `fn facts_of(core, k, who, system) -> ProduceFacts` in `rules/production/` (replaces the string computations `capacity()`, `free_capacity_granted`, small-unit exemptions (Cabal commander), `fleet::is_unlimited`/`UNLIMITED_FLEET_BILL`,
   the system's discount, `buildable_for_window` (the nine buildable units; "more than nine buildable units" is a bail today: lift the array to the unit-type count or a `SmallList`), `enforcement_limit`).
   Today only the *ability* variants get facts from the string (`leader_production.rs` header); the unit-production variant needs `ProductionWindow::of_unit(facts, agent_rules, emit_timing)` (new constructor: today `to_core` builds
   it from a string window). Parity oracle for the property test: `ProductionWindow::new` + `refresh` (string) on the same position until D2-7.
2. Start-of-use effects as core: `fn begin_use(core, env, who, system, mode: UseMode::{Unit, Ability{fixed_limit}}) -> Result<ProductionUse, LiveError>` in the exact `enter_production` order (`game.rs:813-896`):
   (a) `capacity == 0` => window created with stage `Done`, no effects; (b) `meta.production_seq += 1`; (c) Hacan hero offer (`leaders::offer_production_hero`: the core `hero_offer` window exists in `rules::production::triggered`/`production_window_dual::hero_offer`; **VERIFY**);
   (d) `technology::production_used` (Sarween Tools discount, AI Development Algorithm ask: core `technology_windows`/`technology_core`, `technology_dual::production_used` exists); (e) emit `PRODUCTION_USED` (typed; War Machine etc. react through the registry); (f) `refresh`: re-derive
   the budget from `production_discount_remaining` and re-open a window that would have been `Done`. **Dropped by decision, do not port:** `agency_supply_network` (Keleres) and `keleres::offer_agent`/`close_agent_window`.
3. End-of-use effects (`finish_use`, `production.rs:3861`) in the composer's `Finishing` phase, once (`settled`): `end_value_swap` (Hegemonic Trade Policy), `breakthroughs::on_production_finished` (Hacan breakthrough: `hacanbt`, >= 3 non-fighter ships,
   one fleet token), `laws::prophecy_after_production` (discard unless >= 2 fighters), `announce_produced` (`UNITS_PRODUCED` with `source: "production"|"ability"`, `units[]`, `count`; typed emission so reactions open; staged when the resolver is off), then
   Self-Assembly Routines when `!ability && !made.is_empty()` (core technology window, `technology_dual::self_assembly`). These also fix the children: `Spawn::Produce { facts }` (`driver/children.rs`) must run the same `Finishing` (it declines them today:
   `leader_production.rs` header), with `ability: true` so Self-Assembly is skipped.
4. `impl Window for ProductionUse` (composer, `Child<ProductionWindow>` for the middle phase; stage = phase + child stage) and `impl Held` (`KIND = WindowKind::Production`, `Closed = Made`). `Made` is already the child report type.
5. Callers: aftermath `Producing` (task owned by M5, calls `begin_use`), Warfare secondary/primary (`strategy_cards::resolve_timed` callers: **VERIFY** the list with `git grep "resolve_timed("`), `Construction`/`TitanBuild`/`Leadership` windows in
   `rules/production/strategy_cards.rs`, `Spawn::Produce`/`ProduceOne` children. Each caller's `lent_sites.tsv` row (`strategy_cards::construction_primary|secondary`, `te4_construction` [Thunder's Edge: exempt]) is closed by routing through `begin_use`.
6. Payload identity and nested journal as 2.1; `live_arm` for production is not an arm of its own (it runs inside the aftermath arm and inside the strategy arms); `live::production` is the lockstep label.
7. The pay step: `production::pay_step` (the free payment loop `pay_with_observation_credit`), `sling_relay`, `integrated_economy`, `produce_one` are `dual_window!` sites with core halves (`production_window_dual::{pay_step, sling_relay,
   integrated_economy, produce_one}`); in A they are direct core windows (`PayBill` is a `Spawn::Bill` child already). Confirm each has no `unported_triggered` precondition left once Keleres is dropped (`production_view::unported_window/unported_triggered`
   become constant `false`; delete after D2-7).

8. The neighbours of the step outside `production.rs` that the sites list counts with it: the faction production hooks (`argent::extra_production_planet`/`production_destinations`, `ghost::extra_production`/`production_cost_reduction`, `muaat::extra_production`, `saar::production_destinations`: core hook slots exist,
   the `cb_dual`/`cc_dual` adapters fall back to string on `None`: make each a direct call), `faction_abilities::production_biomes`, and the `supply::*` writes the step uses (`gain_trade_goods`, `spend_goods`, `capture_from_board`, `capture_from_reinforcements`, `return_captured`, `release_all_captured`: `dual::write`s with a core arm; direct calls in the flip).

**Residual gates (zero-bail).** From `B3-bail-01..04`, `W0-bail-01`, `W6-bail-01`, `production_window_dual` reasons (*cited*; refresh from X0's `lent-census.md`): Naaz mech staged event (`NAAZ_MECH_PLACED` via `supply::stage_naaz_mech_placed`: carried per `production_view` doc, **VERIFY**
with the tier-W test names), Mentak Corsair form, Nomad exhaustion watch, Yssaril borrowed agents (carried), "a unit over the fleet supply or the capacity of its system (the string asks which units go)" in `produce_one`, "more than nine buildable units",
Argent agent redirect ("a producer that is not an Argent seat"), the Winnu agent's exhaustion outside the window, Letnev commander's payment held another way ("held another way than as an unlocked leader"), the Hacan flagship / Munitions Reserves with goods only a Keleres agent
could spend (Keleres: gone), `ProductionWindow::to_core` gates `ability`/`fixed_limit` (W5 done: confirm), a `settled` window, "a count beyond 32 bits". Everything keyed to Keleres/Deepwrought/Last Bastion/Crimson is dropped, not ported.

**Tests.** (1) `live::production` lockstep for: a plain use (one unit), a use with a Sarween discount, War Machine (+reaction asked), Hacan hero free use, Cabal commander exemptions, Sol capacity-ship free units, Winnu agent in-window, Argent agent, a use that makes nothing, a 3-ship use (Hacan breakthrough token), a
2-fighter use under Prophecy of Ixth (no discard) and 1-fighter (discard), Self-Assembly Routines pay and decline; (2) payload identity per stage (`Paying`, `Placing`); (3) the `UNITS_PRODUCED` event bytes with `source` for unit vs ability uses; (4) Warfare secondary through `begin_use` (replaces `the_warfare_secondary_is_not_carried_and_makes_the_window_unsupported`);
(5) mutation check per end-of-use effect (the effect of each must kill a mutant in the lockstep); (6) staged-event path (no resolver): `UNITS_PRODUCED` staged and flushed by `supply::flush_staged_events` equivalent. **Cheap:** all of it + a 1-game `driver=dual` smoke. **END-OF-PHASE ONLY:** full dual and the playthrough.

**Traps.** T-I2-1 the end-of-use block runs "where the use reaches `Done`", possibly from `settle` inside `answer` without another question (`finish_use`'s comment): the composer's `Finishing` phase must run on the same `advance` call that ended the child, not on the next.
T-I2-2 `production_seq` is incremented *before* the Hacan hero offer because the free-use marker names this production (`enter_production` comment): keep the order or the hero's marker names the previous use.
T-I2-3 `capacity == 0` returns the window *without* the events but the later `refresh` still runs in other callers (`resolve_timed` differs from `enter_production`: it offers the hero only `if hero_ready && capacity > 0`, and resets `production_discount_remaining` first from `RULES_WINNU_AGENT_PRODUCTION`): two openers or a mode, tested separately.
T-I2-4 `ability` uses skip `PRODUCTION_USED`, hero offer and Self-Assembly (they "use no unit's PRODUCTION"); a shared composer that forgets this double-pays Sarween.
T-I2-5 rules-version gates (`RULES_WINNU_AGENT_PRODUCTION` and others in `production.rs`): a game below the version needs the old branch (question 5).
**Estimate (estimated): 16-24 h** (facts 4-6, start effects 3-4, end effects 3-4, composer + payload 3-4, callers 2-3, tests 3).

### 3.3 I1 Invasion (34 sites) - lane M3 (the longest pole)

**Done when:** the invasion of an aftermath opens (`INVASION_BEGAN`, `transient.invasion.active`), runs through bombardment, custodians, commitment (with the pool from the hooks), space cannon defense, ground rounds, coexistence, control gain and exploration, and closes - all as
one core composer `Invasion` held as `Window::Invasion`, with no `InvasionWindow`, `InvasionView`-with-`None`, or `[gate: finish_committing|finish_control_gain|control_gain_exploration]` left on the path. The same composer is reachable as a **child** for the hero windows that fight a committed planet
(Yin hero `fight_committed_planet`, Mahact hero, Sardakk hero's `at_commit_step`): they are the leaders lane's callers, not this lane's work, but the constructors below are this lane's contract.

**State moved into `CoreState`** (`state/invasion.rs`, replaces the 16 fields of `InvasionWindow`, `invasion.rs:2751`; names are proposals):

| String field | Typed |
|---|---|
| `invader`, `system` | `Seat`, `System` |
| `stage: Stage` (9 variants: `Bombarding`, `ChoosingBombardment(BombardCursor)`, `Custodians`, `Committing`, `Fighting{planets, index, defender}`, `Advancing{planets, index}`, `ChoosingNextCombat{planets, index, planet, remaining}`, `FinalizingControl{planets, index, planet, previous}`, `Done`) | `IStage` with `SmallList<Planet, 8>`, `u8`, `Seat`, `SmallList<Seat, 6>`, `Option<Seat>` |
| `report: InvasionReport { explored, committed, captured, bombardment_kills, custodians_removed, coexisted }` | `IReport` (`explored: SmallList<(Planet, ExploreResult), 4>` - **VERIFY** what `exploration::Explored` carries; `captured: SmallList<(Planet, Option<Seat>), 8>`) |
| `pending_scoring_occurrences: VecDeque<(FeatOccurrence, bool)>`, `current_ground_occurrence` | `SmallList<(u64, bool), 4>`, `Option<u64>` |
| `notes_at_tactical_start` | `NoteHold` (exists, `state/aftermath.rs`) |
| `bombard_plan: Vec<BombardPlan>`, `bombard_index`, `bombard_occurrence`, `bombard_announced` | the core `bombard::{Entry, Cursor}` (plain data already), `u8`, `u64`, `bool` |
| `extra_commits: Vec<CommitOrigin>` | `SmallList<CommitOrigin, 8>` (the type of `hooks::ground::commit_candidates`) |
| `temporary_space_commits: BTreeMap<PlanetId, Vec<Unit>>` | `SmallList<(Planet, SmallList<Unit, 8>), 4>` |
| `combat_only` | `bool` |
| `strict_timing_error`, `galaxy` | not state (the error is a `LiveError`; the galaxy is `core.map`) |

The public boundary `ActiveInvasion` (`phase` text per stage, `planet`, `defender`, `ground_round`, `last_step`) is already `transient.invasion.active`; `update_public_boundary` (`invasion.rs:2785`) becomes `fn publish(&self, core)`: same phase mapping
(`bombardment|custodians|landing|ground_battle|planet_result|next_defender|control|complete`), same reset rules. The UI reads it (wire unchanged).

**String code replaced.** `InvasionWindow::{new, new_with_notes, at_commit_step, settle, pending_choice, resolve, drive, finish_committing, announce_and_defend, offer_research_team(_defense), add_forced_combats, begin_ground_round, roll_ground_pair, roll_ground_tail,
assign_ground_round_hits, finish_ground_round, advance_fighting, advance_control, finish_control_gain, control_gain_chain, control_gain_fracture_relic, control_gain_exploration, answer_*}` (`invasion.rs:2843-6000`), `AftermathWindow::next_step`'s invasion branch (`game.rs:747-812`),
`Aftermath::Invading`, and the 12 `dual_typed!` fallback reads (custodians_cost/gate, bombardable, ground_combat_value, bombard_plan, landable, landable_planets, commit_options, ground_pools, ground_hit_plan, roll_bombard dice check, take_bombard_hits_viewed -> `InvasionView`).

**Ordered tasks.**
1. Payload + `From` conversions + identity test; `IStage` <-> the leaf windows' stages mapping table written first, as a comment on the composer (each string `Stage` arm -> the leaf and the `Next` it hands on).
2. `facts_of` per leaf from `(CoreState, Kernel)`: `begin::Facts`, `roll::Facts`, `tail::Facts`, `round::Facts`, `defense::Facts`, `advance::Facts`, `control::Facts` (`infiltrate_fix` from `rules_version`, `announce`), `harrow::Facts`; replacing `core_invasion::InvasionView` (which returns `Option`) by
   infallible `Rd`-based calls (`rules::invasion::{bombardable, bombarders, roll_bombard_plan, landable, landable_planets, ground_pools, ground_hit_plan, custodians_cost, custodians_gate}` are already free functions of `Rd`).
3. **The commit pool computed in the core**: `fn commit_pool(rd, invader, system, extra: &[CommitOrigin]) -> Vec<Troop>` = space troops + `k.hooks.ground.temporary_space_commit_candidates` + `commit_candidates(.., already)` (the hook slots exist: `kernel/hooks/ground.rs`; Sardakk G'hom Sek'kus is registered in the factions crate;
   Naalu Matriarch fighters: **VERIFY** a registration). Replaces the string-computed `PoolIn` (`dual_window.rs:2980`) and removes the `commit_candidates` call into `factions::hooks_ground`.
4. The opener `fn open_invasion(core, env, aftermath: &Aftermath) -> Result<(), LiveError>` = the invasion branch of `next_step` (`game.rs:747-790`): write `transient.invasion.active` (phase `bombardment`, `seq = meta.activation_seq`), emit `INVASION_BEGAN` (typed; "two cards read at the start of an invasion"),
   build the payload with the rolled bombardment (`new_with_notes` rolls the plan: `roll_bombard_plan`, `roll_bombard` dice from `Sx::die`), push `Window::Invasion`. And `open_at_commit(core, env, aftermath)` for the Sardakk hero (`at_commit_step`: stage `Custodians`/`Committing` without bombardment) and `Invasion::fight_committed(core, planet, defender)` (`combat_only`).
5. The composer `driver/windows/invasion.rs`: `impl Window for InvasionMachine` with `Child<..>` per leaf, a `loop` over `IStage` exactly as `settle` (`invasion.rs:3367-3447`) and `resolve` (`:5999-6130`): stages with questions delegate `ask_of` to the leaf
   (`BombardTarget`, `Custodians`, `CommitGround`, `FightRound`, `NextCombat`); stages without (`Bombarding`, `Advancing`, `FinalizingControl`) run children to completion. **Scoring pauses**: `take_scoring_occurrence` returns `(occurrence, combat)` which the aftermath turns into an `EventScoring`
   window (`PauseLimit::OnePerPlayer` if combat else `AnyPerPlayer`): the composer sets `Closed::Pause(occurrence, limit)` / an `Open(Window::EventScoring(..))` in `Out.opened` and resumes when that window closes (the stack entry `Invasion` stays below it).
6. The two string-only tails, ported as leaves (these are the "gate rows" `finish_committing` and `finish_control_gain`, the biggest lend by frequency): **5b** `finish_committing` remainder - Research Team offers (`offer_research_team`, `offer_research_team_defense`), `add_forced_combats` (Titans of Ul Coalescence via `hooks.ground.forced_combat_planets`),
   the announcement/defense already core (`SpaceCannonDefense`); **5a** `control_gain_chain` remainder after `ControlGained` - Fracture relic (`control_gain_fracture_relic`), the exploration of a first claim (`control_gain_exploration`: deck choice, draw, reward windows; core `rules::explore_windows::Reward`/`ExplorationRelics` area is `Core` already), Integrated Economy, `destroy_rival_structures`/`advance_control` (planet index, previous controller).
   `coexist_instead` (Deepwrought) and Last Bastion rows: dropped.
7. `impl Held for InvasionMachine`; `Closed = IReport` (what the aftermath needs: nothing but "done"; the hero callers read `captured`).
8. Arm: runs inside `Arm::Aftermath` (M5). `live::invasion` lockstep label, wrapped around `settle`+`resolve` of the invasion stage.
9. Retire the 3 `run_dual_ask` question sites (`bombard_target_ask`, `next_combat_ask`, `fight_round_ask`: their core halves are `BombardTarget`, `NextCombat`, `FightRound` windows; the string asks stop being needed).

**Residual gates (zero-bail), *cited* from `ENGINE_KNOWN_ISSUES.md` (`D1b2-bail-02`, `W0-bail-01`, section 5 of `ENGINE_CORE_DRIVER.md` "bails of the new windows") - refresh from X0's census:** begin: Naaz Supercharge (35 in 20 golden games), Proxima Targeting VI, a Letnev side with infantry on the planet
(Dunlain Reaper: "a question and a payment inside the opening of a ground round"), the copied-round Sol agent offer (nested); roll/tail: X-89 Bacterial Weapon (doubled hits), a readied The Thundarian (`GROUND_COMBAT_ROLL_STEP_ENDED` and the replay loop), a roll-replay mark; round hits: a **neutral side**, hits a faction adds to a roll,
the **nested** family (reaction windows after `GROUND_FORCE_DESTROYED` 29, `TRADE_GOODS_GAINED` 14, `GROUND_HITS_TO_ASSIGN` 8, ability effects 24 of the 82 in the golden games: all closed by the armed registry + the reaction/ability port, i.e. D2-8a's `armed()` plus the lent-bodies lanes);
bombardment: a neutral victim, a ship-form unit flipping on landing (Naaz Eidolon), a landing the core commit window does not carry; defense: entropic scar, L4 Disruptors, attachment cannon (Geoform), neutral invader; "an Assimilate seat whose unit forms the core cannot name" (roster). **Not to port (dropped or Thunder's Edge):** Keleres toll,
Crimson deploy, Alarum (Ral Nel), Deepwrought coexistence, Last Bastion, the TE legendary planet, Heart of Ixth.

**Tests.** (1) `live::invasion` lockstep per stage family on the existing positions (`invasion.rs` tests that are named `*_in_lockstep`: `harrow_runs_as_a_core_window_after_the_removals_in_lockstep`, `infantry_ii_rolls_in_the_core_window_in_lockstep`, `the_cabal_captures_in_the_core_window_in_lockstep`,
`a_legionnaire_destroyed_by_a_bombardment_stages_its_trade_good_in_lockstep`, `self_assembly_routines_pays_in_the_core_window_in_lockstep`, ...: re-run each with the arm wrapper); (2) a payload identity test after *every* question of a full multi-planet invasion with coexistence (three seats), a custodians offer, a bombardment on a shielded planet, forced combats;
(3) journal round trip inside a ground round (a reroll or sustain reaction asks, drop the driver, rebuild, answer); (4) the scoring-pause interleave (invasion paused by an `EventScoring` window and resumed); (5) the hero constructors (`open_at_commit`, `fight_committed`) vs the string `at_commit_step`/`fight_committed_planet`; (6) mutation checks per task; (7) regression test per failure.
**Cheap:** (1)-(6) + `dual_run profile-20.json 20 driver=dual,invasion=dual --game 0` (the first game that reaches an invasion; the 20 golden games have 2007 `commit_one` calls). **END-OF-PHASE ONLY:** the full armed dual and the playthrough.

**Traps.** T-I1-1 `InvasionWindow::settle` is re-entrant: `Committing` with nothing to land runs `add_forced_combats` and `advance_fighting` *inside settle* and returns; the composer must reproduce "settle after every answer, not after every step" or a commit that lands the last unit skips the combats (`resolve`'s tail:
"Committing settles when nothing is left to land" calls `finish_committing`). T-I1-2 the `Custodians` stage falls through to `Committing` in `resolve` when `!custodians_removable` ("the ask that was actually answered was the commit one"): the question the table sees is the commit one; keep it in `ask_of` or the Decision compare differs. T-I1-3 `SettleError` (`strict_timing_error`):
the string stores the first `TimingError` and returns it at `take_settle_error`; the composer returns `Err` from `advance` with the state rolled back (the driver's snapshot does this) - make sure the wrapper restores the *whole* game state on `Err` (`step_aftermath` today clones state, dice, rng, event sequence, timing, table log, events, aftermath, tactical, turn_closing, failed actions: the new checkpoint is `CoreState::clone`). T-I1-4 dice: all invasion rolls must come from `Sx::die` / the state's streams; test faces (`Dice::preloaded`) are irreproducible - product assertion XF-7, not a bail in the product. T-I1-5 seat order: `next_fight`'s tie-break by player-id text and "seats not in id order" bails: the core uses seat order = id order by construction (`Advance` doc: unreachable); keep a debug assertion. T-I1-6 `ActiveInvasion.last_step` records are written by leaf windows (`bombard.rs`, `defense.rs`, `round.rs`) and read by the UI: the composer must not overwrite them in `publish`. T-I1-7 the invasion is also driven in tests through `InvasionWindow::new` (`game.rs:14368`, `:14496`, `:19186`): those stay legacy.
**Estimate (estimated): 36-56 h** (payload + conversions 4, facts 6-8, commit pool 3, opener/hero constructors 4, composer 10-14, 5b tail 5-8, 5a tail 6-10, tests 6-10). Largest single piece of the plan.

### 3.4 Combat as the aftermath's `Fighting` stage - lane M4

**Done when:** the space combat runs as `Window::Combat(CombatFields)` on the stack above `Aftermath`, opened by the aftermath composer after `Head`, driven by `live::advance::<CombatMachine>` (the existing `rules::combat::machine::Machine` + `Env`/`Fields`), and ends with the feats
and `FightOver`; `CombatWindow::{settle_open, resolve_answer, pending_choice}`, `fields_back`/`FieldsIn` and the 40 `combat.rs` sites are not on the live path.

**State:** `CombatFields` is typed (`state/aftermath.rs`: `CombatStep` incl. `Queue{assigning, queue}`, `Retreating`, `HitPhase`, `pending_retreats`, `offered`, `damaged`, `fled`, `at_start`, `scoring`, `outcome`, `end_announced`). Missing: `Env` from the core (`attacker, defender, system, announce, staging, feat occurrence, interned texts,
`may_retreat`, retreat neighbours, rule gates`): `fn env_of(core, k, system, attacker, defender) -> Env` replacing `core_combat::window_env` (string-derived).

**Tasks.** (1) `Env` from `CoreState`; (2) `impl Held for CombatMachine` (`from_pending` = `Machine::awaiting(env, fields)` for a pending decision, `Machine::new(env, fields)` for a settle, as the adapters do - **VERIFY** how `dual_window` chooses between them, `settle_machine_dual`); (3) the `Stop` mapping: `Pending` -> `Live::Asks`, `Scoring` -> `Out.opened = [EventScoring(..)]` then resume at `AfterBarrage`, `Over` -> `Closed::Over(outcome)`, `Bailed(_)` -> `LiveError::Unmapped` (the zero-bail target is to delete both `Bail` variants); (4) burn down `W2-bail-02` and `W0-bail-01` (below); (5) `rules::combat::feats` + `FightOver` already core.

**Residual gates (zero-bail), *cited* from `INTEGRATION7.md` 10.3 and `W0-bail-01` (rare in the golden census; each has a test today):** Letnev round opening (`round_opening_why`), Mentak Ambush / Assault Cannon / Dimensional Splicer / Impulse Core (`opening_why`), an Argent side's excess hits of the barrage (`barrage_why`), the Jol-Nar commander's reroll question in the barrage,
Waylay in the barrage, The Thundarian in the space roll step (`roll_why`), Hacan flagship and Munitions Reserves with spendable goods (below rules version 8: question 5), hits cancelled by a card in the window, staged destructions waiting for an announcement (Courageous, Reflective Shielding child flows), a roll that ends the fight (`RollEndsFight`), Duranium Armor, Devotion (W2: fixed), a retreat that moves a flipping unit or a Floating Factory,
neutral side (Assault Cannon vs neutral), X-89 (`roll_tail_unported`), Mahact hero's fleet fight (`combat::resolve_resolving`: owned by the leaders lane, uses this machine as a child). **Exempt:** Heart of Ixth (Thunder's Edge), Last Bastion retreat, diplomacy. Ask whether the lent-bodies lane W2 already owns this list (question 3); if not, M4 owns it.

**Tests / cheap / END-OF-PHASE:** the existing `the_composed_combat_*` tests run once through the arm wrapper (`live::combat`) and once with the machine rebuilt from `CombatFields` after every question (identity); one test per removed bail (kill/justify mutants). **Estimate (estimated): 14-24 h** (excluding the residual list: +10-18 h if not owned elsewhere).

### 3.5 I3 Aftermath chain (5 sites) - lane M5

**Done when:** `finish_tactical_inner`'s second half (the opening of the aftermath) and `step_aftermath` run in the core: the chain `Head -> Fighting -> feats -> FightOver -> Invading -> Producing -> Done` is opened, driven (including every scoring pause), and closed by one composer; `AftermathWindow`, `Aftermath` (string),
`d1_pending::transition`, the 5 sites (`aftermath::head`, `combat::space_cannon_offense`, `aftermath::fight_over`, `fleet::enforce_everywhere`, `fleet::standing`) and `Game.aftermath` are not on the live path.

**State.** `state::Aftermath` is complete for the chain bookkeeping (`stage`, `before_combat`, `feats_noted`, `event_scoring`, `notes_at_start`); the sub-windows are separate stack entries (`Combat`, `Invasion`, `Production`). The `log: Vec<String>` the string drains into `events` (`SPACE_COMBAT_RESOLVED`, `INVASION_RESOLVED`, `PRODUCTION_RESOLVED`) is `PlainKind`s through
`driver::aftermath::announced` (exists). Add `Aftermath.head: Option<HeadPhase>` if the head can ask mid-way (enforcement `Enforce`, cannon `SpaceCannonOffense` ask): the head today is a *window run in one go* in the string; in the core it is the `Head` composer with stages `Enforcing|Firing|Opening|Done`, so the payload must hold `stage = Head(HeadStage)` or the head runs as its own
stack entry `Window::Head` (**decision for the lane; recommended: a `AStage::Heading(HeadStage)` variant**, the payload of `Head` being re-derivable from the position + the `cannon: Facts` rebuilt by `facts_of`).

**Ordered tasks.**
1. Opener `fn open_aftermath(core, env, closed: &tactical::Closed, notes) -> Result<(), LiveError>`: `finish_tactical_inner :5257-5356` second half: build `Head::new(Facts)` (`facts_of`: `limits` from fleet, `CannonFacts`, `pre_roll` code, `skip_to_commit` for the Sardakk hero), push `Window::Aftermath` with stage `Heading`; or `at_commit_step` (`:535`) -> `open_invasion(.. at commit)` directly (`feats_noted = true`).
2. Composer `driver/windows/chain.rs` (`AftermathChain`): `Heading -> (pause for the cannon's event scoring) -> open Combat (settle_open) -> Fighting -> feats (`note_feats`, `combat::feats`) -> maybe pause (`OnePerPlayer`) -> FightOver -> next(Settled::Fighting{holds, contacts: false}) -> open_invasion | begin_use -> ... -> Done`,
   using `driver::aftermath::{next, announced, note_feats, opens_invasion}` for every transition and `Settled`. `contacts` is constant `false` (finding 6; keep the `Negotiating` arm only behind `debug_assert!(false)`).
3. The event-scoring pause: `pending_event_scoring` -> `open_occurrence_event_scoring(Timing::Action, occurrence, limit)` (`game.rs:6230`) opens an `EventScoring` window (`Window::EventScoring(Scoring { timing: Action, occurrence, one_per_player })`, the core `rules::scoring_book`). The opener of that window is **C1's** (status/agenda/scoring); until C1 lands, M5 adds `driver::scoring::open_occurrence` as a 10-line function in a file of its own and C1 keeps whichever lands first.
   While the `EventScoring` window is open, `step_arm` serves it (`Arm::EventScoring` precedes `Arm::Aftermath` in the chain); when it closes the aftermath resumes by its stage. This interleave is the main correctness risk of the lane.
4. `fn close_aftermath` hands `Done` to the tactical tail (`close_tactical`, task of M6) via `Out.opened`/`Closed`.
5. `Held for AftermathChain` (`KIND = Aftermath`), `Closed = ()`. `live_arm(Aftermath)`: the arm advances whichever of `Combat | Invasion | Production | Head` is the top sub-window, then re-evaluates the chain.
6. `ask_arm`: `AskArm::Aftermath` returns the top sub-window's question (string `AftermathWindow::pending_choice` delegates to the current stage).
7. `fleet::enforce_everywhere` and `fleet::standing` (`fleet.rs:157,513`): `EnforceAll` exists in `driver/windows/enforce_all.rs`; `standing` is a `dual_typed!` read (`query::fleet`): direct call.

**Residual gates.** D1b2-bail-01/03: `FightOver` bails for a dropped-faction seat (gone), Titans of Ul (Coalescence forced combats: port `forced_combat_planets` use, already a hook), structured diplomacy (gone), **Shard of the Throne in play (its theft in `fight_over_text`: `laws::steal_throne_card`: port as a `fx` write)**, a neutral side; head/cannon: irreproducible dice (product assertion), a rules version below the faction effects (question 5), **a Jol-Nar commander or Heart of Ixth in play** (Jol-Nar commander: port; Heart of Ixth: Thunder's Edge, exempt), a Letnev commander that pays on a sustain without being unlocked, neutral units in the system, seats not in id order (unreachable),
nested activity the table does not follow (Scramble Frequency, Maneuvering Jets: the reaction/ability port).

**Tests.** `live::aftermath` lockstep over: a no-fight aftermath (no enemy) going straight to invasion; a fight with a retreat that leaves the active seat holding the space; a fight with a barrage scoring pause; a draw (Skilled Retreat) -> production only; the cannon offense killing the last non-fighters (`SpaceCannonTookTheLastNonFighters` pause); a Sardakk hero skip to commit; a conquest with custodians; a production with a mid-chain reaction (War Machine);
payload identity at every question; the pause interleave (aftermath <-> EventScoring) with the window closed by the scoring book. Existing: `d1_pending.rs` tests `aftermath_game(...)` (`:1258`, `:1273`), `cannon_prelude_tests.rs`, `tactical_dual_tests.rs`. **Cheap / END-OF-PHASE:** as before. **Estimate (estimated): 16-26 h.**

### 3.6 I4 Tactical window (6 sites) - lane M6

**Done when:** `Dispatch::Tactical` opens `Window::Tactical`, `live::advance::<tactical::Machine>` runs activation, movement, loading, the move with its rift rolls, the close of the movement step, then opens the aftermath and, when the aftermath closes, runs the **tactical tail** (`close_tactical`) in the core; the 3 window sites
(`tactical::activate|move|load` via `tactical_dual::label`, `tactical::free`, `tactical::finish`) and 3 fallbacks (`search_path`, `search_all_routes`, `movable_into`) are core calls; `TacticalWindow`/`TacticalStage`/`Game.tactical` are not on the live path.

**State.** `state::Tactical` is complete (payload round-trips through `Machine::payload`/`from_payload`, `d1::pending.roundtrip` today). Add: the route chosen (`RoutePick`) is a pick, not state, once the machine can list routes itself (`tactical::ship_routes`: the display/selection logic needs `loadable` for its pickups; **A2b-bail-07**).

**String code replaced.** `apply_choice`'s tactical branch (`game.rs:3436`), `step_tactical :3669`, `apply_tactical_windowed :3763`, `begin_free_tactical :4110`, `finish_tactical :5124`, `finish_head(_windowed) :5149/:5216`, the first half of `finish_tactical_inner`, `close_tactical :5578`, `check_close_gates :5545` (core: `driver::tactical_tail`), `tactical_choice`, `tactical_dual::{step, free, finish, enforce, label}`.

**Ordered tasks.**
1. `Facts { pool: Code }` from `core.texts`; opener `open_tactical(core, env, seat)`: `Window::Tactical { stage: Activating, notes_at_start: note_holdings(core) }` (compute the `(holder, issuer)` pairs from the `cards` slice; today `combat::note_holdings(&GameState)`), plain event `TACTICAL_ACTION_BEGAN`. (`settle_extreme_duress` precedes it: C3.)
2. `impl Held for tactical::Machine` over `Machine::from_payload` / `payload`; the machine's `Step` results become `Out.plain`; `with_route` becomes a pick.
3. **The hook chain of an activation (`A2b-bail-02`, `activation_unported`)**: today a held Spatial Conduit Cylinders, Scanlink Drone Network, E-Res Siphons, Nullification Field, Mahact at the table (Starlancer, the commander's re-activation, a granted `mahactcommander`), Minister of Peace in play, the Dominus Orb held by the activator, a Ceasefire that denies the move keep the activation on the string.
   Port each as a `Next::Open` window of the activation (they are `SYSTEM_ACTIVATED`/`before activation` abilities of the table; most are already registry abilities: **VERIFY** which exist in `ti4-engine-core-factions`), with the golden-census order (62 `sardakkagent_mark:SYSTEM_ACTIVATED`, 59 `aetherpassage:SYSTEM_ACTIVATED`, 7 `technology:as`, 4 Scanlink held per 2 games, `A2b-bail-06`).
4. **The close of the movement step (`A2b-bail-08`)**: Nova Seed's map edits (the derived `Galaxy` is string-owned today: `movement::replay_map_edits`; owned by the *turn hooks* sibling for the per-step replay, but the edit itself is written by the faction effect: provide `fx::map_edit` writing to `map` slice - **VERIFY** whether `MapSlice` holds edits), the Sardakk hero unlocked (asks to skip to the commit step -> `skip_to_commit`), a Floating Factory in the active system (blockade destroys it).
5. `relocate_ships` and `relocate_ships_with_cargo` (`A2b-bail-09`, 24 call sites in `factions/hooks_movement.rs`, `naalu.rs`, `argent.rs`): faction effects, not the action; a lane of the leaf gates (list them here so they are not forgotten).
6. **The tactical tail** as a composer step after the aftermath closes: `driver::tactical_tail::{then, det_fires, crown_offers, tro_window}` decide; run `CrownExplore` (core window, exists), the frontier `explore` of Dark Energy Tap (core exploration windows), emit `TACTICAL_ACTION_ENDED` when `tro_window` (typed event; saved/restored on error in the string: the driver's rollback does it),
   write `turn_write::close_tactical_writes` (core `driver::tactical_tail` has the closing writes), emit `TACTICAL_ACTION_COMPLETE`, then `Then::{LeaderStrategy | Secondary | FinishAction}`. `LeaderStrategy` and `Secondary` after tactical are Thunder's Edge Warfare (exempt: `Then::LeaderStrategy` stays `Unmapped("thunders_edge")`); `Then::FinishAction` calls `turn_flow::finish_action` (exists; C2 owns what it calls).
7. The free tactical action (`begin_free_tactical`: `Machine::free(seat, facts, system)`), used by strategy cards: opened by their composers as a `Window::Tactical`.
8. Route lists: port `tactical::ship_routes`/`loadable` pickups (the `|rN` answers) or have the machine own the route list; delete `RoutePick` and `Bail::Route`.
9. `live_arm(Tactical)`; `AskArm::Tactical` = the machine's `ask_of`.

**Residual gates (zero-bail), *cited* A2b rows:** 02 (task 3), 06 nested (task 3 + the armed registry), 07 (task 8), 08 (task 4), 09 (task 5), 05 irreproducible dice (product assertion), 04 two seats of one faction (product assertion), 01/03 diplomacy and dropped factions (dropped), 10 "a hold that offers nothing while not complete" (never seen: make it a `debug_assert`).

**Tests.** `live::tactical` lockstep for each of: activation with each hook technology held; a move with a gravity rift loss; loading (cargo) with a non-default route; the close with Nova Seed / Sardakk hero / Floating Factory; the free tactical action; a whole tactical action through to the aftermath and the tail (DET and Crown explorations, T'ro listener);
payload identity at every question; the journal across an activation whose `SYSTEM_ACTIVATED` reaction asks. Existing: `tactical_dual_tests.rs`, `nexus_reach_tests.rs`, the `d1_pending.rs` tests at `:1201`. **Estimate (estimated): 22-34 h** (opener/machine/arm 5, hook chain 6-10, close/Nova Seed/Floating Factory/Sardakk 4-6, routes 3-5, tail 4-6, tests 3-4).

---

## 4. Order, dependencies, and the seams with the composition islands

```text
M0 (contract, stubs, bail-as-failure, arm-level lockstep)            <- everything below
 +-- M1 trade        (independent)                         needs C3 only for the menu option that opens it
 +-- M2 production   (independent; callers: M5, strategy cards, leaders)
 +-- M3 invasion     (independent of M2 except custodians/toll payment = PayBill child, exists)
 +-- M6 tactical     (independent; its tail needs C2's turn_flow::finish_action, exists)
 M4 combat live      (needs M0; independent of M2/M3)
 M5 aftermath chain  (needs M2, M3, M4 closes; builds top-down with stubbed sub-windows meanwhile; needs C1's EventScoring opener)
 M7 zero-bail playthrough harness (needs M0; grows as lanes land)
```

Seams (each is a function the other document must provide or call; whichever lands first keeps its version, the other deletes theirs):

| Seam | This document needs | The composition document provides / consumes |
|---|---|---|
| C1 scoring | `driver::scoring::open_occurrence(core, Timing::Action, occurrence: u64, limit: PauseLimit)`: open an `EventScoring` window, and "closed" observable through `Pending` (the aftermath resumes by its stage) | opens/closes `Scoring`, `EventScoring`, `Tokens`, `Voting`; owns `Arm::EventScoring` precedence |
| C2 hooks | `TACTICAL_ACTION_ENDED`, `TURN_BEGAN` ordering; `turn_flow::finish_action` after the tail; `Lent::TurnHooks` stays out of these windows; `technology::end_turn` etc. are not called inside any of the five | owns per-step reconcile hooks (`space_stations::reconcile_all`, `legendary::settle_control_points`, `laws::apply_to_galaxy`, `movement::replay_map_edits`: **called between arms**, never inside an island) |
| C3 dispatch | `open_trade(core, env, proposer, partner)`, `open_tactical(core, env, seat)`, `begin_use(.., UseMode)` callable from the dispatcher; the options of the transaction source come from `query::trade::open_partners` | owns `apply_choice` (Lent options -> opener calls), `settle_extreme_duress` before the openers, the failed-component-action bookkeeping |
| Leaders (leaf gates, not in either document) | `Invasion::{open_at_commit, fight_committed}`, `begin_use` as `Spawn::Produce`, the combat machine as a child (Mahact hero) | Yin/Mahact/Sardakk/Empyrean/Hacan hero windows |
| Reaction/ability port (lent bodies) | the armed registry (`window_driver::armed`) as the **default** for live drivers: a live arm runs with the registry, so every `nested (unported ability ...)` bail becomes a question or a missing ability | the W-lanes |

Interference to watch: C3's `Dispatch` and M6's opener are two halves of the same edit in `driver/dispatch.rs`/`step.rs` (shared hot files, lane M0 pre-wires them); M5 and C1 share `EventScoring`; M3 and the leaders lane share the invasion constructors.

---

## 5. Tests and the acceptance chain

**5.1 Per-island lockstep (cheap, every package).** The arm-level `dual_window!` wrapper of 2.6 plus the existing leaf lockstep tests; labels `live::trade|tactical|aftermath|invasion|production|combat`. Run `TI4_AREAS=driver=dual TI4_DUAL_FAIL_FAST=1` on the touched tests only
(`cargo test --profile lockstep -p ti4-engine --lib <module>::`); the full lib with all areas `Dual` (4 860 tests) once per package end, as `D2-remaining.md` section 5 says.

**5.2 Payload tests (cheap).** For every `Held` type: (a) identity `to_pending(from_pending(c)) == payload` after each question of each lockstep test; (b) journal: ask, drop the driver, rebuild from `(CoreState, Pending)`, answer, equal to the uninterrupted run (state, events, rng positions); (c) `Clone` of `CoreState` mid-window continues identically (the `fork_equivalence` shape).

**5.3 Oracle (cheap per game, END-OF-PHASE ONLY on the corpora).** `ti4-engine-core-bridge::oracle`: subject A legacy (`MaskSubject<LegacyMask>`), subject B the new engine. Until the wrapper exists B is `MaskSubject<CoreMask>` with the live arms switched on (`set_core_mask` + the arm wrappers on `Core`); after D2-8d B is the wrapper `Game` and **the harness does not change**.
Extend: (i) `--zero-bail` (below); (ii) per-island coverage counters printed with the report; (iii) a `Skipped` reason for scenarios that need Thunder's Edge. The oracle's per-step `Decision` compare has the trade quirk (T-I5-1).

**5.4 Bail-as-failure whole-game playthrough (the acceptance test; lane M7).** A bridge bin `playthrough` (or `oracle --zero-bail`) that plays N seeded complete games on the **new engine only** with the shipped rule bot (`Pilot::Bot`), and fails (exit 1) on any of: a `Difference` of category `Bail` (`TI4_BAIL_IS_FAILURE=1`), a `LiveError::Unmapped`, a non-zero `Lent` counter (once the wrapper has them), a game that does not reach `finished` within the round limit, `core_sync::unsupported::find` being non-empty, a panic.
It prints, per game: seats and factions, rounds, steps, and the **coverage matrix** - per arm/leaf the number of times the core live path ran (`live::trade`, `live::tactical`, `live::aftermath`, `live::invasion`, `live::production`, `live::combat`, and each leaf window) - and refuses to pass a corpus whose matrix has a zero for an island (a zero bail count with zero invasions proves nothing; T-Z1).
Corpora: (a) cheap, any time: `--games 1` on one 4-player and one 6-player seed, to tell the island lanes they are not blind; (b) **the Z-gate** (END-OF-PHASE ONLY, the first heavy run, only when (a) is green on 8 consecutive seeds): golden 20 (`profile-20.json`), `tools/rulebot/golden-lent.json` (19 games, six factions per table), the wide roster, and a faction sweep that seats **every lobby-seatable faction** at least twice,
plus scripted scenarios that the bot never produces: a free-form trade deal, a Pride of Kenara deal, a Mecatol invasion with custodians, a coexistence ground combat (three seats), a Sardakk hero skip to commit, a Winnu/Argent agent production, a Hacan three-ship production, Prophecy of Ixth in force.

**5.5 What the old gates still do.** `golden check --mask all=core|all=string` (legacy-independent recorded answers), `board_equivalence`, the benches and `alpha_roundtrip` are unchanged and remain G-flip/G7 material (`D2-remaining.md` 5); the islands add nothing to them but must not break them.

---

## 6. Cheap vs END-OF-PHASE ONLY (consolidated)

| Cheap (every package, any time) | END-OF-PHASE ONLY |
|---|---|
| `cargo check`/`clippy` of the touched crates; targeted `--profile lockstep` tests of the touched module; `check_layers.py core` (all new files are L1 `state/`, L4 `rules/`, L5 `driver/`), `lint_core.sh` | the Z-gate (5.4 b): the full zero-bail playthrough over golden 20 + lent 19 + wide + the faction sweep |
| arm-level lockstep per island (5.1), payload tests (5.2), mutation checks of new core files (single-file `mutants.sh`) | the armed `dual-all` (`TI4_DRIVER_ARMED=1`, 20 games) with the live arms on, `dual-wide`, soak, benches x churn modes |
| `dual_run profile-20.json 20 driver=dual --game 0` (and `--game 1`) as a smoke after each lane | the lend-proof run (R5 of `D2-remaining.md`) for the *remaining* leaf lends |
| `playthrough --games 1` (5.4 a) once the lane's island is wired | `golden check` full, `board_equivalence`, `alpha_roundtrip` on a copy, G-flip, G7 |
| the whole-lib `TI4_AREAS=all=dual` run once at a package end | any run that needs `/root/heavy2.sh` |

Rule (user, 2026-10-31): no heavy run starts before the cheap playthrough (a) is green. Every failure found by any comparison gets a regression test that fails on the pre-fix code (standing rule).

---

## 7. Traps (summary; island-local ones are in 3.x)

| # | Trap | Where | What to do |
|---|---|---|---|
| T-Z1 | A zero bail count is vacuous when an arm never runs | 5.4 | coverage matrix per leaf; a corpus with a zero cell fails |
| T-Z2 | `Backend::Core` of `dual_window!` runs the *string* closure for windows (`dual.rs:1479`, finding 3 of `D2-remaining.md`): a green `TI4_AREAS=all=core` run proves nothing about live drive | 2.6 | the arm wrapper compares a real `live::advance`; never count `Core` runs as live |
| T-Z3 | The probe (`TI4_BAIL_PROBE`) sees `dual_window!` bails only; the 164 `dual_typed!` `None`s, `dual!` (87), `write` (65), gates (8) are invisible | `D2-remaining.md` finding 2 | in a live arm each `None` is `Unmapped(label)`; bail-as-failure covers them |
| T-N1 | `Driver::settle` snapshots `s.clone()` per op (a full `CoreState` clone), and the journal re-runs an op per nested answer | 2.1 | measure with `state_size`/the census on an invasion-heavy game before M3 ends; the clone is the whole cost model of the flip (`ENGINE_CORE_STATE.md` 6.6) |
| T-N2 | Rolling a nested question back restores `state.rng`; a body that draws from anything else (a thread-local, the string `GameRng`) breaks the replay | 2.1 | all draws through `Sx::die`/`SeedSource`; test: journal round trip around a dice-rolling op |
| T-E1 | Event order: the string `mirror_timing_log`, `emit`, `emit_typed`, plain events and the `private:#staged:*` marks make one ordered log; the core has `Driver.emitted` + `Pending.staged` + plain records | all | the wrapper (D2-8d/P1) merges them; every island lane's lockstep compares the *merged* order, not the typed list alone |
| T-V1 | Rules-version gates inside the machines (`RULES_WINNU_AGENT_PRODUCTION`, `RULES_INFILTRATE_FIX`, Munitions Reserves below 8, "a game below the faction effects/bug fixes") | 3.2, 3.3, 3.4 | question 5: port the old branches or refuse old saves; do not leave them as bails |
| T-L1 | Shared hot files: `state/pending.rs`, `driver/step.rs`, `driver/session.rs`, `ti4-engine/src/dual_window.rs` (4 554 lines), `d1_pending.rs` (1 366), `game.rs` (19 289) | lanes | M0 pre-wires the variants and arm registry; no lane edits `dual_window.rs` or `game.rs` except the single `dual_window!` arm wrapper line it owns; new adapters go in new files (`invasion_live.rs` etc.) |
| T-L2 | Nightly rule: whatever lands before 20:30 Berlin is in that night's sweep; these lanes branch from the integration line, not the live branch | all merges | nothing here merges into the live checkout; the integration line is merged by the coordinator |
| T-L3 | `decision_delivery_inventory.rs` rows name functions by file: a renamed/extracted `*_string` body needs a rename-only registry edit (done in W-lanes) | tasks that extract bodies | one commit per file group, listed in the report (question 5 of `D2-remaining.md`) |
| T-D1 | `Window::Untyped` carries `data: [u32; 3]` only and `done_next()` of the tactical machine opens the aftermath that way | 3.5, 3.6 | open `Window::Aftermath` typed; delete the `Untyped` aftermath/invasion/production plumbing and `WindowKind::Invasion/Production` placeholders last |
| T-D2 | Contact pause, `Diplomacy`/`AgendaTalks` arms | finding 6 | not implemented; keep the enum variants for the dual until D4 |

---

## 8. Estimates (agent-hours, estimated; calibration of `D2-prereqs.md` 2.2)

| Lane | Island | Hours | Notes |
|---|---|---:|---|
| M0 | shared contract, stubs, arm-level lockstep, bail-as-failure, coverage counters | 8-12 | interface commit in 1-2 h |
| M1 | I5 trade (17 sites) | 8-12 | free-form parser is the big item |
| M2 | I2 production step (31 sites) | 16-24 | start + end-of-use effects, facts |
| M3 | I1 invasion (34 sites) | 36-56 | composer + 5a/5b tails + facts + hero constructors |
| M4 | combat live + residual openings | 14-24 (+10-18) | + only if no leaf lane owns W2-bail-02 |
| M5 | I3 aftermath chain (5 sites) | 16-26 | scoring-pause interleave |
| M6 | I4 tactical (6 sites) | 22-34 | hook chain, routes, Nova Seed, tail |
| M7 | zero-bail playthrough bin, coverage matrix, scripted scenarios, burn-down loop | 10-16 | + the burn-down itself is the sum of the residual lists |
| | **Total** | **130-204** | **105-170 without M4's optional residual and M7's burn-down loop** |

Critical path: M0 (2 h interface) -> M3 (36-56 h) -> M5 (16-26 h) -> M7's first cheap playthrough: about **60-90 h** of elapsed implementer time with four lanes in parallel. The residual gate lists (3.x "Residual gates") are the part with the widest error bar: refresh them from X0's `lent-census.md` before the lanes start, because
the pre-W numbers in `LENT_CENSUS.md` are out of date (`D2_8_0_AUDIT.md` 4).

---

## 9. Lanes (branches, worktrees, file ownership)

Branches are cut from the newest `core-integration-N` plus `core-d2-l1` (the oracle, `window_driver`, `lent_sites.tsv`); one worktree each `.claude/worktrees/<branch>`; max 4 implementation agents at once (`reference_impl_agent_brief`); Haiku runners for every compile/test/mutation run (`feedback_theory_impl_haiku_workflow`).

| Lane | Branch | Starts | Owns (writes) | Reads / must not touch |
|---|---|---|---|---|
| **M0** | `core-m0-live` | first | `ti4-engine-core/src/driver/{live.rs, step.rs, session.rs, mod.rs (Driver::resume_nested)}`, `kernel/ability.rs` (Tape accessors), `state/{pending.rs, invasion.rs (stub), production.rs (stub), mod.rs}`, `ti4-engine/src/{d1_live.rs, dual.rs (bail-as-failure), lend_table.rs (variant match only)}` | everything else |
| **M1** | `core-m1-trade` | after M0 interface | `rules/trade_window.rs`, `rules/trade.rs`, `query/trade_deals.rs`, `query/trade.rs`, `driver/live/trade.rs`, `state/pending_windows.rs` (Trade payload), `choice/trade.rs`, `ti4-engine/src/{trade_window_dual.rs, trade_core.rs, trade_window_tests.rs, render/trade.rs}` | `transactions.rs`, `game.rs` (only the one arm-wrapper line) |
| **M2** | `core-m2-production` | after M0 interface | `rules/production/**`, `rules/leader_production.rs`, `driver/live/production.rs`, `state/production.rs`, `ti4-engine/src/{production_window_dual.rs, production_view.rs, technology_dual.rs, breakthroughs.rs (extract only)}` | `production.rs`, `game.rs` |
| **M3** | `core-m3-invasion` | after M0 interface | `rules/invasion/**`, `driver/windows/invasion.rs`, `driver/live/invasion.rs`, `state/invasion.rs`, `kernel/hooks/ground.rs`, `ti4-engine/src/{core_invasion.rs, invasion_live.rs (new)}` | `invasion.rs` (string; only `*_string` extractions as pure moves), `dual_window.rs` (use a new file) |
| **M6** | `core-m6-tactical` | after M0 interface (wave 2 if four slots are taken) | `rules/tactical/**`, `driver/tactical_tail.rs`, `driver/live/tactical.rs`, `state/tactical.rs`, `ti4-engine/src/{tactical_dual.rs, tactical_dual_tests.rs}` | `tactical.rs` (string), `game.rs` |
| **M4** | `core-m4-combat-live` | wave 2 | `rules/combat/**`, `driver/live/combat.rs`, `ti4-engine/src/{core_combat.rs, cannon_dual.rs}` | `combat.rs` (string; pure moves only) |
| **M5** | `core-m5-aftermath` | wave 2 (stubs earlier) | `driver/windows/{aftermath.rs, chain.rs, enforce_all.rs}`, `driver/aftermath.rs`, `driver/live/aftermath.rs`, `state/aftermath.rs`, `ti4-engine/src/{d1_action.rs, d1_pending.rs (transition hooks)}` | `game.rs` |
| **M7** | `core-m7-playthrough` | wave 2 | `ti4-engine-core-bridge/src/{oracle.rs, workload.rs}`, `bin/{oracle.rs, playthrough.rs}`, scenario fixtures | the engine crates |

Waves: **wave 1** M0 (then immediately) M3, M2, M1 (four slots with M0 for its first 2 h, then M6 takes the M0 slot); **wave 2** M4, M5, M7 and whatever of M6 is left. M3 is the long pole: give it the strongest agent and start it the moment the interface commit of M0 lands. The merge order is M0, M1, M2, M3, M4, M6, M5, M7 (the coordinator merges; conflicts in shared files are resolved keeping both sides).

Every lane brief starts with the STANDING RULES of `reference_impl_agent_brief` (items 1-12) and adds: (a) read this file's sections 2 and its own 3.x; (b) the START RULE: if M0's interface commit is not on the integration line, stop; (c) no heavy run (6); (d) the hand-back lists the residual gates still open in its island with their `ENGINE_KNOWN_ISSUES` ids, the coverage counts of its arm in the 1-game smoke, and the regression tests added.

---

## 10. Open questions for you

1. **Persistence design D-L0 (2.1):** typed payload on the `Pending` stack + `Pending.nested` journal (recommended), or the live `Driver` objects held in a wrapper `LiveSlot`? The first makes the checkpoint the plain `CoreState` clone and matches D1; the second is ~20-30 % less code in M3/M2 but keeps a second kind of window state. *Recommend:* payload + journal, `LiveSlot` as a per-child fallback only.
2. **Who owns M0 and the shared contract?** The three composition documents also need `live::advance` and the arm wrapper. *Recommend:* M0 as written here is the single owner; the other documents' lanes depend on its interface commit.
3. **Combat residual list (W2-bail-02, W0-bail-01: Letnev/Mentak/Argent/Jol-Nar/Thundarian/Waylay openings, staged destructions, neutral side, X-89):** does a lent-bodies leaf lane already own it? If yes M4 shrinks to the live drive (14-24 h); if not it adds 10-18 h.
4. **Contact pause:** confirm that structured diplomacy being dropped means `Aftermath::Negotiating` is unreachable in the product (finding 6), so the live arm may leave it out.
5. **Old rules versions:** must games saved at an older `RULES_VERSION` (the alpha saves, 11 of 14 replayable) still run on the new engine mid-game? That needs the version branches inside the machines (production `RULES_WINNU_AGENT_PRODUCTION`, invasion `RULES_INFILTRATE_FIX`, combat below version 8, "a game below the faction effects") ported instead of bailed. *Recommend:* ports for the branches the alpha saves actually pass through (found by the alpha replay on a COPY), and a refuse-at-load for the rest; say if you prefer to refuse all pre-latest saves.
6. **Oracle quirk (T-I5-1):** may legacy's `legal_options()` get a trade branch (view-only, no rule-body edit) so that both subjects show the same question before a trade step, or do we skip the pre-step `Decision` compare for trade steps?
7. **Free-form deals in the bot corpora are 0:** the property test + a scripted scenario is the cover (5.4); is a bot that proposes free-form deals (training-side) wanted for the Z-gate, or is the scripted scenario enough?
