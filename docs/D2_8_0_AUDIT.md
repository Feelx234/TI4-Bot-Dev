# D2-8.0: the completeness audit (cheap checkpoint), `core-d2-l1` from `core-integration-8` (2026-10-31)

Read-only analysis of the tree plus the static inventory `tools/engine_core/lent_sites.tsv` (new in this branch). **No census was available** (the end-of-D1 run
X0 has not happened), so "counts" below are *sites in the source* and sizes of the string machines, not calls per game; call counts are cited from
`docs/LENT_CENSUS.md` (pre-W numbers, marked) where they matter. Nothing here changes behaviour. The audit answers the question of `D2-remaining.md` 3.4:
**which string state does the core not yet drive (the islands), and is the flip Architecture A (core owns the loop, islands held next to the `CoreState`) or B
(embedded legacy `Game` as the lender)?**

## 0. Result

| | |
|---|---|
| Window kinds (`state::WindowKind`) | 18 (15 fields of `Game` + `Combat`, `Invasion`, `Production`) |
| Typed in `Pending` (`state::Window` variant with a payload) | 14; `Untyped` placeholders: 4 (`Invasion`, `Production`, and the two diplomacy windows) |
| Windows the core opens, drives and closes in a live step (class **Core**) | **0** (every window arm of `driver::step::step` is `Step::Lend(Lent::Arm(..))`) |
| Windows with a full typed round trip to the string struct (**Portable**, `Args::Typed`) | 10 kinds: 7 with a `*_back` function (follower window x2, scoring x2, leader followers, tokens, voting) and 3 whose payload is the string field itself (leader strategy, executive order, agenda queue) |
| Windows typed for *comparison only*, no way back to the string struct (key, not struct) | 3: `Tactical`, `Aftermath` (stage tag), `Trade`; `Combat` has `fields_back` (the machine's input fields), not the `CombatWindow` |
| **Islands** (string state the wrapper must hold opaquely next to the `CoreState`) | **5 machine islands** + **3 composition islands** = **8** (section 4) |
| Arms of `Session::step` | 19: 2 stopped by the core, 3 run in part (turn prelude, phase flip/end, the question), **14 lent whole** |
| Disabled by decision (not islands, refused at creation by P1) | `Diplomacy`, `AgendaTalks` windows and everything in `core_sync::unsupported` |

**Recommendation: Architecture B as the scaffold (one embedded legacy `Game` per wrapper `Game` as the lender), A afterwards, window family by family.**
The decision rule of the plan is "A if there are at most three islands, each with a clear open and close site". There are eight (five of which have clear
open/close sites), so the rule gives **B**. The reasoning, and what B costs, is in section 5; the corrected estimate is in section 6.

## 1. `Game`'s window fields against `WindowKind` / `Window`

`game.rs:1157-1250`. "Back" = a function that rebuilds the string struct from the typed payload (`d1_windows.rs`, `d1_pending.rs`, `d1_action.rs`).
"Moves in core" = a core transition exists that the dual compares (`d1::pending`): D1a-bail-02/04, D1b-bail-01. In every row the *opening and closing* is a
string body (phase flow is not ported, D1a-bail-05): the held copy is read off the string field at the step that opened it.

| `Game` field | `WindowKind` | `Window` payload | Back to the string struct | Moves in core | Class |
|---|---|---|---|---|---|
| `secondary` | `StrategySecondary` | `Secondary` | `secondary_back` (full) | yes: `strategy_secondary::advance` | Portable |
| `secondary_after_tactical` | `StrategySecondaryAfterTactical` | `Secondary` | `secondary_back` (full) | as above | Portable |
| `leader_strategy` | `LeaderStrategy` | `{player, leader, card}` | the payload is the field (three ids, compared by value in `roundtrip`) | no (observed) | Portable |
| `leader_followers` | `LeaderFollowers` | `LeaderFollowers` | `leader_followers_back` (full) | no (follows the string) | Portable |
| `scoring` | `Scoring` | `Scoring` (the book) | `scoring_back` (full) | yes: the scoring book (`scoring::step`) | Portable |
| `event_scoring` | `EventScoring` | `Scoring` | `scoring_back` (full) | as above | Portable |
| `tokens` | `Tokens` | `Box<Tokens>` (with the typed `StatusReport`) | `tokens_back` (full) | no (follows) | Portable |
| `voting` | `Voting` | `Box<Voting>` | `voting_back` (full) | the vote windows replay in core (`vote::*`) | Portable |
| `executive_order` | `ExecutiveOrder` | `{owner, speaker}` | as above (two ids) | no | Portable |
| `agenda_queue_after_event_scoring` | `AgendaQueueAfterEventScoring` | `SmallList<Agenda>` | as above (a list of ids) | no | Portable |
| `tactical` | `Tactical` | `Tactical` (stage) | **`tactical_back` returns a `TKey`**, not the `TacticalWindow` | observed only (D1b-bail-01) | **Island** (needs a back converter or an opaque struct) |
| `aftermath` | `Aftermath` | `Box<Aftermath>` (stage tag only) | **`aftermath_back` returns an `AKey`** | the head and the fight run as core windows by replay; the chain does not (D1b2-bail-01) | **Island** |
| (sub) `Aftermath::Fighting` | `Combat` | `Box<CombatFields>` | `fields_back` -> `FieldsIn` (inputs of the machine, not the `CombatWindow`) | the combat machine, by replay (`combat::*`) | Portable for a lend of one *method*; Island as a *held* struct inside the aftermath |
| (sub) `Aftermath::Invading` | `Invasion` | `Untyped { kind, actor, data: [u32;3] }` | none | none (D1b2-bail-02) | **Island** |
| (sub) `Aftermath::Producing` | `Production` | `Untyped` | none | none (D1b2-bail-02) | **Island** |
| `trade` | `Trade` | `Trade` (chairs, proposals left, the deal as a `Shape`) | **`trade_key_of`**: key only, no `trade_back` | the trade window replays in core (`transactions::*`) | **Island** |
| `diplomacy` | `Diplomacy` | `Untyped` | none | none | Disabled (P1 refuses diplomacy) |
| `agenda_talks` | `AgendaTalks` | `Untyped` | none | none | Disabled (diplomacy only) |

Fields of `Game` that are not window options (`status_resolved`, `agenda_resolved`, `strategy_phase_announced` = `Pending::flags`; `failed_component_actions`,
`turn_closing`, `actions_this_turn`, `prepared_turn_seq`, `blocked`; the rng and dice) are the business of D2-8d (state and checkpoint), not of this audit.

## 2. `driver::step::step` (`Session::step`): the 19 arms

`driver/step.rs`, `driver/session.rs` (`Arm`). "Lent" is `Step::Lend(Lent::Arm(arm))` (the whole arm is a string body) or the `lent` part of `Step::Ran`.

| Arm | What the core does today | What is lent |
|---|---|---|
| `Finished`, `Blocked` | `Step::Stopped` | nothing |
| `TurnPrelude` | the Spec Ops return, the `prepared_turn_seq` mark | `Lent::TurnHooks`: start-of-turn technologies, commander unlocks, Military Support, the `TURN_BEGAN` window (D1b2-bail-04) |
| `Phase` (action phase over) | `action_phase_ended` | `Lent::PhaseEvents` (`STATUS_PHASE_BEGAN`, `ACTION_PHASE_ENDED`) |
| `Phase` (strategy -> action) | `reset_stability`, `strategy_phase_flip`, `begin_action_turn` | `Lent::StrategyHooks` (Naalu token, Imperial Arbiter, the two events); other phases: whole arm |
| `Ask` | names the question (`AskArm`) | the question itself and its options: the menu *sources* (faction, leader, technology, expedition, relic, exploration, action card, transaction, contact, payment) are string reads (D1b-bail-05); the dispatch of the chosen option (`apply_choice`) is `Dispatch::Lend` at label `turn::component` |
| `LeaderFollowers`, `Secondary`, `EventScoring`, `Scoring`, `Tokens`, `Voting`, `Diplomacy`, `Aftermath`, `AgendaTalks`, `Trade`, `Tactical` | **whole arm lent** (11 window arms) | everything; these windows' own machines run only as dual replay |
| `Status`, `Agenda` | whole arm lent (D1a W3/W4: the status and agenda phase machines, the eleven per-step reconcilers) | everything |
| `StrategyAnnounce` | whole arm lent | the first strategy phase's announcement |

Count: 2 stopped + 3 partly run + **14 lent whole** = 19.

## 3. The `D1*-bail-*` rows against "core runs it / lent"

(`docs/ENGINE_KNOWN_ISSUES.md`; only the rows that say something is still the string's.)

| Row | Core runs | Still lent |
|---|---|---|
| D1a-bail-01..05 | the typed window payloads, the follower window and scoring book transitions, the typed remainder (leader followers, tokens, voting, trade) as *followers* of the string | opening/closing of every window; the status and agenda phase machines; the per-step reconcile hooks (`starting_tech::settle`, `announce_staged_ground_events`, `announce_gains`, `space_stations::reconcile_all`, `nexus_is_triggered`, `legendary::settle_control_points`, `keleres::reconcile`, `crimson::enforce_sundered`, `laws::apply_to_galaxy`, `movement::replay_map_edits`) |
| D1b-bail-01, 03, 04, 05 | tactical/aftermath typed and observed; stage transitions (`driver::aftermath::next`); `step_arm`/`ask_arm` decide the arm; the action menu is assembled (`driver::menu`) | the aftermath composer; the menu sources; `apply_choice` |
| D1b-bail-06, 07 | turn flow: `begin_action_turn`, `advance_turn`, `begin_next_round`, the Action arm of `advance_phase`, retention, the pass writes | `begin_next_round` for a card id the tables do not know (rare) |
| D1b2-2, 3, 4, 6, 7 | the aftermath head (fleet enforcement, cannon offense, `Before`), `close_tactical` gates, `fleet::enforce_everywhere`, Spec Ops return, the feats and `FightOver`, Minister of War, `Session::step` (three arms) | the explorations the Crown/DET start, `TACTICAL_ACTION_ENDED`, `redistribute_after_te_warfare`, `select_leader_strategy_followers` |
| **D1b2-bail-01** | the chain up to the end of the fight | **the opening of the invasion** (`INVASION_BEGAN`, `active_invasion`, `InvasionWindow::new_with_notes`), **the production step** (`enter_production`: capacity shortcut, `production_seq`, Hacan hero offer, `technology::production_used`, `PRODUCTION_USED`, Agency Supply Network, Keleres agent), their stages, the contact pause |
| **D1b2-bail-02** | nothing | `Window::Invasion`, `Window::Production`: `Untyped` placeholders, no machine state in the core |
| D1b2-bail-03 | the head/cannon windows for the common positions | bails: irreproducible dice, dropped factions, Jol-Nar commander / Heart of Ixth, neutral units, seats not in id order, nested activity the table does not follow |
| **D1b2-bail-04** | - | hooks between the turn steps (`technology::end_turn`, `legendary::end_turn`, `minister_of_war` ported), `technology::start_turn`, `refresh_commander_unlocks`, Military Support, `TURN_BEGAN`, the strategy-phase-end faction hook, the status and agenda halves, **every non-native menu source's options and body** |

## 4. The islands, with counts

Static counts from `tools/engine_core/lent_sites.tsv` (sites outside test code: 483 in all) and from the source. "Sites" are the macro sites the flip must turn
into `lent::<label>` calls; the *window bodies that have no macro site at all* (the opening of the invasion, `enter_production`) are in the last column and are
not in the site count. Dynamic call counts (`LENT_CENSUS.md`, golden 20, **pre-W**): `invasion::commit_one` 2 030, `invasion::finish_committing` and
`invasion::finish_control_gain` ~1 100-1 500 gate rows each, `invasion::bombard_step` 1 376, `ProductionWindow::step` 3 336 (53 bailed after W), `combat::settle`
709, `combat::answer` 894. **Re-count with the fresh census of X0 before 8e** (trap T-N).

### Machine islands (state the core has no type for)

| # | Island | String machine | Sites (window / fallback / gate / question) | Open site | Close site | Notes |
|---|---|---|---|---|---|---|
| I1 | **Invasion** | `InvasionWindow` (`invasion.rs`, 13 529 lines; 16 fields, 9 stages: bombard, custodians, committing, fighting, advancing, ...) | 34 (16 / 12 / 3 / 3) | `AftermathWindow` after the fight: `InvasionWindow::new_with_notes`, `game.rs:777` (and `:551`) | `Aftermath::Invading` -> `Producing`/`Done`, `game.rs:957-1010` | `Window::Invasion` is `Untyped`; the single biggest lend by frequency (gate rows `finish_committing`, `finish_control_gain`). 16 `Window` sites, all methods on `self` or free functions of the window |
| I2 | **Production step** | `ProductionWindow` (`production.rs`, 9 468 lines; 19 fields, 4 stages) | 31 (9 / 22 / 0 / 0), incl. `strategy_cards::construction_*` | `enter_production`, `game.rs:813` (and every card/hero/component that produces: `leader_production`, Arborec hero, Muaat agent run it as a core *child*, W-L7) | `Aftermath::Producing` -> `Done`, `game.rs:1014-1026` | `Window::Production` is `Untyped`; the same window is used outside the aftermath (Warfare, Construction, Muaat/Arborec), where it is a core child already |
| I3 | **Aftermath chain** | `AftermathWindow` + `Aftermath` enum (`Fighting`, `Negotiating{invade}`, `Invading`, `Producing`, `Done`) in `game.rs:177-300` | 5 (4 / 1 / 0 / 0): `aftermath::head`, `combat::space_cannon_offense`, `aftermath::fight_over`, `fleet::enforce_everywhere`, `fleet::standing` | `AftermathWindow::new`/`at_commit_step` | `Aftermath::Done` | Holds I1 and I2 and the `CombatWindow` (typed fields, but no struct back); the contact pause (`Negotiating`) and the event-scoring pause live here. Typed only as a *stage tag* |
| I4 | **Tactical window** | `TacticalWindow` (activation, movement, hold being filled) | 3 window + 3 fallback (`tactical::free`, `tactical::finish`, `tactical_dual::label(..)`) | `Game::apply_tactical_windowed` | `close_tactical` (core gates D1b2-3) | typed `Tactical`, but `tactical_back` is a *key*; rebuilding `TacticalWindow` (path, loaded hold) needs a converter. Small |
| I5 | **Trade window** | `TradeWindow` (`transactions.rs`) | 1 window + 16 fallbacks (`transactions::*`) | `Game::apply_choice` (menu dispatch) | `Game` trade close | typed `Trade`, `trade_key_of` only; the core trade window replays |

### Composition islands (typed windows, but the *loop* that opens/closes/chains them is a lent arm)

| # | Island | What is lent | Evidence | Sites |
|---|---|---|---|---|
| C1 | **Status and agenda phase machines** | the two arms `Status`, `Agenda` and the openers/closers of `Scoring`, `Tokens`, `Voting`, `EventScoring` (D1a-bail-05, W3/W4); the eleven per-step reconcilers | 2 whole lent arms, 4 window kinds opened by string | 38 sites in the status/agenda/objectives/laws/secrets group (9 window / 29 fallback) |
| C2 | **Turn hooks and phase events** | `Lent::TurnHooks`, `StrategyHooks`, `PhaseEvents` | 3 `Lent` variants | 6 sites (`technology::start_turn/end_turn`, `legendary::end_turn`, `phase::*`, `turn::minister_of_war`) + the string bodies without macro sites (`refresh_commander_unlocks`, `promissory::turn_started`, the `TURN_BEGAN` emission) |
| C3 | **Action-menu dispatch** | the options and bodies of every non-native menu source (`Dispatch::Lend`, label `turn::component`): `apply_choice` (pass, tactical, strategic, component, action card, trade, contact, payment) | D1b-bail-05, D1b2-bail-04 | the largest group: ~147 sites over leaders, faction abilities, exploration/relics, action cards, promissory, transactions, tactical (27 window / 118 fallback / 2 gate); the `faction_abilities::perform_component` and `leaders::use_leader` gates are in it |

Machine islands: **5** (I1-I5). Composition islands: **3** (C1-C3). Total **8**; even counting only the machine islands whose struct is *absent* from the core
(I1, I2, I3) the plan's "at most three, each with a clear open and close site" is met only if the composition islands C1-C3 are ported first, which is the
D2-8e work the estimate counts (12-25 h).

## 5. Architecture recommendation

**B first.** Reasons, in order of weight:

1. **A needs the core to own the composition before it can hold anything.** 14 of 19 arms are lent whole. Under A, each lent arm must become a core arm or a label lend
   *and* the islands must be held as opaque structs entering and leaving at the labelled sites. With eight islands (five of them machines the core cannot rebuild), A
   means writing converters or opaque holders for I4 and I5 as well, and porting C1 (the status and agenda machines) before the first live step. That is the
   12-25 h of 8e plus an unestimated converter block; the plan's own threshold is three islands.
2. **B moves authority per step with machinery that already exists.** `TI4_CHURN=state` (materialise, run the legacy arm, load the written slices back) is B at
   step granularity and is proven on the bench corpora. B needs no argument builders and no island holders; the string windows stay inside the embedded `Game`.
3. **B does not close the door on A.** The embedded `Game` is replaced family by family: a window family whose arm becomes a core arm stops being run by the lender.
   The lend-proof mode (R5) and `TI4_CHURN=state` benches enforce the same invariant ("the lent code sees exactly what the string engine would have seen") on both.

**What B costs (and why it is still only a scaffold):** the string windows never leave `Game`, so `checkpoint` must clone them (the 18.7 % checkpoint bucket does
not shrink, `ENGINE_CORE_STATE.md` 6.6, until the families move); the string engine stays in the server and wasm binary until C1-C3 and I1-I5 are ported
(that was the phase-a plan anyway); and the CPU A/B of G7 row 7 has the per-step lend cost (`materialize` + load) against it. **Measure the lend cost per step with
X0's census before committing to B for more than one nightly** (T-N).

**Order for B -> A:** C2 (turn hooks, smallest, three `Lent` variants), then C1 (status/agenda), then I4/I5 (typed back converters), then I3 with I1/I2 as opaque
holders (the last three are what stay islands in A: invasion, production step, the chain between them), then C3 (the dispatch) last because it is the
largest and mostly `Args::Plain` lends.

## 6. Corrected estimate for 8a-8g (agent-hours, estimated)

`D2-remaining.md` 3.4 gives 30-45 h (A) and 22-30 h (B). With eight islands:

| Package | A as written | A with this audit | B scaffold |
|---|---:|---:|---:|
| 8a one driver constructor | 8-12 | 8-12 (**done in this branch**, flag off; the armed dual is G-flip) | same |
| 8b wrapper crate content | 2-3 | 2-3 | 2-3 |
| 8c entry and strict mode | 4-6 | 4-6 | 4-6 |
| 8d state, checkpoint, surface | 8-12 | 10-14 (the held island structs are cloned in `GameSnapshot`) | 8-12 (+ the embedded legacy `Game` in the checkpoint) |
| 8e flip the windows | 12-25 | **25-40** (C1 and C3 ported first, converters for I4/I5, opaque holders for I1-I3) | 4-8 |
| 8f/8g reconcile hooks, tests | 4-6 | 4-6 | 4-6 |
| Total | 30-45 | **53-81** | **22-35** |

B reaches "the wrapper `Game` owns the `CoreState` and the surface is the same" in about a third of the time and leaves A as a sequence of per-family follow-ups,
each of which the lend-proof run and the census can accept or refuse on its own.

## 7. What this branch gives the next agent

* `tools/engine_core/lent_sites.py` and `lent_sites.tsv`: the static inventory (R3), `--check` keeps it honest, `--census` joins a dual report.
* `Bail.kind/args/when` and `bail_table --check REPORT PROBE..` (R2, R1 item 5): the check that the census, the probe and the table agree.
* `window_driver::driver_for` / `armed()` (8a): the 99 production `Driver::new` sites of the adapters go through one constructor; flag off changes nothing.
* The oracle harness `oracle` (D2-6b(a)): the comparison the flip is judged by; its second subject becomes the wrapper `Game`.

Not done here: any run of the census, X0, G-flip; the armed dual (END-OF-PHASE ONLY); wrappers (R4) and the lend-proof mode (R5).
