# TODO D2-islands-composition: the three composition islands and the 14 lent `Session::step` arms (2026-10-31)

Theory document (read-only analysis: no cargo, no builds, no tests were run). Branch `core-todo-d2d`, cut from `core-todo-d2b` (`756c0742`, `docs/todo/D2-remaining.md`).
Tree read: `core-d2-l1` @ `714b0074` (the audit `docs/D2_8_0_AUDIT.md`, `driver_for`/`armed()`, `oracle`, `lent_sites.tsv`), plus `core-integration-8` @ `67994b33` for what
`core-d2-l1` lacks (`tools/engine_core/zero_bail.py`, the `zero-bail` step of `end_of_d1.sh`; `core-d2-l1` is NOT an ancestor of that tip, merge both before starting).
Line numbers are from `core-d2-l1`: re-grep before editing. Only this file changed.

Companion: another theory agent writes the five MACHINE islands (invasion, production step, aftermath chain, tactical, trade). This document does not duplicate them; where a
lent arm belongs to them it is listed in 2.3 with a pointer and nothing more. Shared foundation items (section 3) are written once here; if the other document defines the same
item, take whichever definition lands first and say so in the commit message.

**Labels.** *Measured* = I counted/read it in the tree for this document. *Cited* = from a document. *Estimated* = my judgement in agent-hours (calibration of
`D2-prereqs.md` 2.2 and `lent-bodies.md` 1: leaf 0.3-0.6 h, new question or child 1-1.5 h, new infrastructure 1-2 h, a window family with tests 3-8 h). **VERIFY** = a 10-minute check
the implementation agent does first. **END-OF-PHASE ONLY** = a heavy run (`/root/heavy2.sh`, `SHARDS=3`, private `CARGO_TARGET_DIR`): never inside a package.

---

## STATUS AND AGREEMENT (2026-10-31, lane A2, branch `core-d2-f-live2`): read this before the text below

This document and `D2-islands-machines.md` were written in parallel and are now ONE design; `docs/D2_LF_RESUME.md` (sections 1-2) and
`docs/D2_LF2_RESUME.md` are the record of what was BUILT. Where the text below differs from what was built, the built thing wins:

| Text below | Built (lane A / A2) |
|---|---|
| KF1 `NativeArms`, `Facts::of_core`, `Step::Native` | done as written (`driver/step.rs`, `session.rs`); `Step::Native(Arm)` only for an arm in the set AND with a live function |
| KF2 `drive_live<W>(driver, core, k, r, asker)` with `Stop::Unmapped` | built as `ti4_engine::live::live_step(core, arm, &Cx, &mut dyn Asker) -> Result<StepOut, Stop>` over the core's `driver::live::{advance, ask, open}`; the persisted form is the machines document's (typed payload on `Pending` + `Pending.nested` journal, rebuilt by `Held::from_pending` at every call; no `LiveSlot`, no wrapper-held driver). `Stop::Live(LiveError::Unknown(..))` is the counted bail |
| KF2 "facts from the position" (`*Facts` structs) | one door per family: `live::plays::play_facts` (action-card plays) is computed from the CORE per call through `AdapterFacts::plays_provider`; the others are the lanes' (`facts_of(core, k, ..)` next to the window) |
| KF3 `KernelEnv`s in `rules/{leaders,strategy,abilities}_env.rs` | one `driver/kernel_env.rs::KernelEnv` implementing all three; agrees with the string views except the hook slots `commander_unlocked` (12 in-scope leaders) and `leader_action` (17 in-scope leaders) that no core faction module registers yet (lane F2, list in `docs/D2_LF2_RESUME.md`) |
| KF4 `Pending.failed_component_actions` | done; cleared by `turn_flow::forget_failed_components` |
| KF5 `NativeSubject{core, live, lender}` that EXECUTES `Lend` through the scaffold and counts it | built differently, on purpose: the legacy `Game` steps for real (it is the follower of the oracle) and the native arm is run in SHADOW on a `CoreState` materialized from it before each step, with the answers of the legacy step; the questions, plain events, positions and the end of the window are compared (`oracle::<subtype>`, paths `native.*`), every step not run natively is counted lent (`product::note_lent("oracle::<arm>")`). Nothing diverges, so there is no cascade; a lane wires its arm with ONE line in `native_subject::open_arm`. A step that ends the window is compared on the prefix of questions/events (the string remainder is the close until KS5 ports it). Bridge only, behind the engine feature `native-subject` (no production crate enables it); `oracle CONFIG GAMES --native ARMS` |
| KF5 `playthrough` bin | built (`ti4-engine-core-bridge/src/bin/playthrough.rs`): te card set, `starting_tech`, all areas `Dual`, bail = failure, per-cause and PER-ISLAND tables (`islands.rs`), live coverage, `--quick` for the one-game check (section 5.4 (a) of the machines document), `tools/engine_core/playthrough_quick.sh` (gate profile) |
| KF6 `driver_live` | done (`window_driver::driver_live`); the 99 `driver_for` sites are converted lane by lane |
| G-zero | NOT started: it waits for 8 consecutive green quick seeds (machines document 5.4 (b)) |

The lanes of section 10 start from the `INTERFACE:` commit of `core-d2-f-live` and merge `core-d2-f-live2` (A2) next; `core-w-l11` (cards played inside another
window) is merged into A2 and its `SeatPlayMore`/`SharedPlay` facts are filled from the core in `live/plays.rs`.

---

## 0. Summary and findings that change the plan

The goal (user rule 2026-10-31): an ENTIRE game plays in the new engine without any bail on the agreed content (everything except Thunder's Edge and Last Bastion, Crimson,
Keleres, Deepwrought, Firmament, Obsidian, Ral Nel, deck reserve/turn redo, structured diplomacy). Architecture A (the core owns the loop) is the target; B (embedded legacy `Game` as
lender) may exist only as a temporary scaffold and must not remain. This document turns the composition half of that into ordered work.

1. **`driver::step::step` has no caller.** *Measured:* `grep "step::step\|step::Needs\|Needs {"` over `crates` finds the function only in its own tests (`driver/step.rs`). The engine
   uses only the pure precedence functions `step_arm`/`ask_arm` (`d1_pending::check_arm`, `d1_pending.rs:185`) to compare which arm the string took. So "the core runs 3 of 19 arms" means
   "the core has code for three arms that nothing calls". There is no live loop, no `Step` consumer, no `Lent` executor. Everything in section 3 is new wiring, not a flip of a switch.
2. **A core window is driven today only against a recording of the string run.** `dual_window!` has three backends (`dual.rs:1479`): `String | Core => ($string)($state).value` (the
   core arm runs the STRING), `Dual => run_dual_window`. The core half is `dual_window::drive_windowed` (`dual_window.rs:670`): it feeds each question the id the string run recorded
   (`script.questions[i].answer`, line ~750) and compares ten things against the `Recording` (nested events, ability dice, resolver, plain/typed events, `withdraw`, `first_event_id`...).
   The live driver (section 3, KF2) is the same loop with the table as the answerer. Anything the adapters take from the recording or from the string position (the `*Facts` structs
   `ProduceFacts`, `StructureFacts`, `OneFacts`, `SeeingLimits`; `Recording.withdraw`; `Recording.first_event_id`) needs a core source in live mode.
3. **The composition islands are mostly NOT macro sites.** The audit counts 38 + 6 + ~147 `lent_sites.tsv` rows, but those rows are leaf queries (`dual_typed!` fallbacks: 263 of 483
   rows are `fallback`, 127 `window`, 82 `hook`, 8 `gate`, 3 `question`). The composition itself is ~25 string functions of `game.rs` with no macro at all: `step_status` (6120),
   `begin_status_bookkeeping` (6274), `step_token_gain` (6336), `finish_status_phase` (6375), `step_agenda` (6435), `open_next_vote` (6458), `start_vote` (6517), `step_vote` (6698),
   `close_vote` (7130), `step_phase` (7224), the `turn_prelude` arm of `step_flow` (2383+), `advance_turn_inner` (7428), `pass_on_tail` (7548), `apply_choice` (3083-3670) and
   `turn_options`/`closing_options` (2903/2834). Section 4-6 are written per function, not per site. The site counts remain useful as the work list of the leaf queries (section 7).
4. **The core has no status-phase machine at all.** *Measured:* no `Window` impl and no function for 81.2 (only `rules::objectives` reveal), 81.3 (draw with hooks), 81.4 (recall
   tokens), 81.5 (the token gain window: `choice::tokens` is empty, the Pool question exists as `choice::strategy::gain_pool_choice`, used by `rules/agenda/close.rs` and Leadership),
   81.6-81.8 (only `rules::explore::ready_relics_and_legendary`). `state::Tokens`/`StatusReport` carry the data; nothing moves it. The agenda side is better: `Reveal`, `Committee`,
   `Vote`, `Close`, `OfferDiscard`, `DeadlyPlot`, `CovertDraw`, `Resolve` exist as windows; the loop between them (`open_next_vote`) and the phase entry/exit do not.
5. **Question channel.** The string engine asks through `Table` (`table.ask_seeing`, synchronous, blocking for a human in the server worker, "nested combat windows can ask another
   human before `game.step()` returns", `worker.rs:1262`). The core suspends (`Driver::run` -> `Status::Asking` -> `resume`). The wrapper `Game` keeps `Table` (bots, host, server and
   wasm call `step()` and install deciders; none of that changes before D3). So the live loop is `loop { run/resume }` with an `Asker` that renders the core `Choice`
   (`render::Typed`), calls `table.ask_seeing` and parses the answer back (`render::parse_pick`). A "step" is one table decision, as now.
6. **Corpus gap that hides arms.** `GameSpec.starting_tech` defaults to false (`selfplay/spec.rs:58`); the product turns it on (`ti4-server/src/session/table.rs:38`
   `starting_tech: true`, `ti4-host/src/setup.rs:553`). So the in-game starting-technology question (`AskArm::StartingTech`, Winnu and Argent Flight, both lobby factions) is in no
   golden game. The same for planning forks (`prepare_hypothetical_turn/_secondary`, product path of `ti4-host/planning`, needs a `preview` secondary window the core's `Secondary`
   does not have). Both are "no-bail" paths without corpus coverage: see 5.5 and 6.1.
7. **Env traits implemented over the string engine.** `leaders::Env`, `strategy::Env`, `abilities::Env` (core) are implemented only by `LeaderView`/`StratView`
   (`leaders_view.rs:266,311`, `strategy_view.rs:167`) which read the string position; the core has a `KernelEnv` only for `trade` (`rules/trade.rs:22`) and the vote
   (`rules/agenda/window.rs:61`, with `is_nekro => false`: "Nekro's bar is applied by the adapter"). The leaders and strategic-action sources of the menu cannot be native until
   `KernelEnv`s exist (KF3). `Env` answers such as `other_action_resolves` read "the content store, the map or the supply".
8. **Estimate.** Foundation 25-40 h, C2 15-25 h, C1 (status + agenda + scoring/token/secondary arms) 55-90 h, C3 (menu) 45-75 h, closing/gates 10-15 h: **150-245 agent-hours,
   estimated, wide**, with 6 parallel lanes about 35-55 h wall plus gates. This is above the audit's 25-40 h for all of 8e (`D2_8_0_AUDIT.md` 6) because that figure prices the
   lend/hold of islands and not the porting of the status machine, the agenda loop and the 13 menu sources; it is the number the user's rule implies. Question 1.

---

## 1. Facts the plan stands on (measured on `core-d2-l1` unless marked)

| Fact | Evidence |
|---|---|
| `Session::step` arms: 19 (`session::Arm`); `step()` runs 3 in part (TurnPrelude: `return_spec_ops`, `mark_prepared`; Phase: `action_phase_ended`, strategy->action flip; Ask: names `AskArm`), 2 stopped, **14 lent whole** (`Lent::Arm`) | `driver/step.rs:56-80`; audit 2 |
| `Lent` has 4 variants: `Arm(Arm)`, `TurnHooks`, `StrategyHooks`, `PhaseEvents`; `Step` has `Stopped`, `Ran{arm,lent}`, `Asks(AskArm)`, `Lend(Lent)`; `Needs{asks, spec_ops_unit}` | `step.rs:21-52` |
| `Facts::of(core, blocked, turn_prepared, turn_closing, starting_picks_done)`: three of those four caller args are derivable from `CoreState` (`Pending::prepared_turn_seq`, `Pending::turn_closing`, `Transient::misc.starting_tech_picks`); `blocked` is the wrapper's | `session.rs:43`, `state/pending.rs`, `state/transient/misc.rs:83` |
| `Pending`: `staged`, `stack: Vec<Window>` (at most one per kind), `top`, `tag`, `driver: DriverFlags` (STATUS_RESOLVED, AGENDA_RESOLVED, STRATEGY_PHASE_ANNOUNCED), `turn_closing`, `actions_this_turn`, `prepared_turn_seq`, `resolver`. **No** `failed_component_actions` (string `BTreeSet<String>`), no `blocked`, no round-income knobs | `state/pending.rs` (end), `game.rs:1157-1250` |
| `state::Window`: `StrategySecondary`, `StrategySecondaryAfterTactical`, `LeaderStrategy`, `ExecutiveOrder`, `AgendaQueueAfterEventScoring`, `Scoring`, `EventScoring`, `LeaderFollowers`, `Tokens`, `Voting`, `Trade`, `Tactical`, `Aftermath`, `Combat`, `Untyped`; kinds also `Diplomacy`, `AgendaTalks`, `Invasion`, `Production` | `state/pending.rs` |
| `state` (L1) cannot name `rules` (L4): a typed `Window` payload is plain data and a machine is rebuilt from it (`ScoringBook::restore(..)` `rules/scoring_book.rs:87`; `*_back` converters in the engine). `Vote` has no restore from `state::Voting` (VERIFY: `rules/agenda/window.rs` re-exports `state::VoteStage` as its `Stage` but has no `from_pending`) | grep |
| Core windows that exist for the composition: `ScoreOne`, `PlaceCard`, `HandLimit` (`objective_windows.rs`), `SecondaryStep` (`strategy_secondary.rs`), `TradePrimary`, `Diplomacy`, `Politics`, `WarfarePrimary`, `ImperialPrimary/Secondary`, `TeWarfare` (`strategy_primaries.rs`), `Leadership`, `Construction`, `TitanBuild`, `Research` (`production/strategy_cards.rs`), `Reveal`, `Committee`, `Vote`, `Close`, `Resolve`, `OfferDiscard`, `DeadlyPlot`, `CovertDraw` (`rules/agenda/*`), `TurnStart`, `TurnEnd`, `ProductionUsed`, `ChaosMapping`, `Psychoarchaeology`, `TransitDiodes`, `Vortex`, `WormholeGenerator`, `ProductionBiomes` (`technology_windows.rs`), `Legendary` (`rules/legendary.rs`), `MinisterOfWar`, `EnforceAll`, `Head`, `FightOver` (`driver/windows/*`), the leader/hero/relic/explore/promissory/faction-component windows, `Play` (action card), `Emit` (reaction window) | `grep "impl.*Window for"` (131 impls) |
| **Missing as windows**: status phase (all), token gain, agenda phase loop, turn prelude, phase begin events, strategy-phase draft pick, starting-tech pick, `use_leader` dispatch, action-menu dispatch for 10 of the 13 sources | grep; audit 2-3 |
| Per-step prelude of `step_flow` (`game.rs:2383-2480`), all string: `starting_tech::settle` (only if picks left), dice scene, `announce_staged_ground_events` (3917), `announce_gains` (3960), `space_stations::reconcile_all` (TE), `nexus_is_triggered` latch, `legendary::settle_control_points` (a core rule exists: `rules/legendary.rs`), `keleres::reconcile` (dropped), `crimson::enforce_sundered` (dropped), `laws::apply_to_galaxy` + `movement::replay_map_edits` (galaxy edits, can set `blocked`), `faction_techs::roll_spec_ops` | `game.rs` |
| `Game` window fields are 17 `Option`s plus flags; the host reads them through `open_windows()` (wasm `lib.rs:1810`), `fork`, `GameSnapshot` (11 mentions in engine, 2 host, 1 server test) | `game.rs:1157-1250`, `1800` |
| `Step` consumers outside the engine (production code): `ti4-server/src/session/worker.rs:1494,1587`; `ti4-wasm/src/lib.rs:1091,1242,1780`; `ti4-host/src/{batch.rs:1589, replay.rs:212,230, history/splice.rs:328, history/turn_redo.rs:367, planning/script.rs:443, planning/draft.rs:547}`; `ti4-selfplay/src/play.rs:554`; `ti4-sim/src/profile.rs:537,601`. All call `Game::step()` / `legal_options()` with a `Table`. **None needs a signature change for the composition flip** | grep |
| `driver_for(` sites: 103 hits = 99 in 21 engine files + 3 in `ti4-engine-core-tests/src/rig.rs` + the definition. By file: `dual_window.rs` 34, `leaders_core.rs` 11, `faction_dual.rs` 7, `technology_dual.rs` 6, `hero_dual.rs` 6, `strategy_cards_core.rs` 5, `production_window_dual.rs` 5, `cannon_dual.rs` 5, `tactical_dual.rs` 4, `objectives_write.rs` 3, `explore_write.rs` 2, `agenda_close_dual.rs` 2, `action_cards_view.rs` 2, and 1 each in `trade_window_dual`, `strategy_secondary_dual`, `strategy_core`, `promissory_dual`, `legendary_core`, `diplomacy_window_dual`, `agenda_reveal_dual` | grep |
| `window_driver.rs`: `driver_for(window, resolver, table, next_id)` = `Driver::new`; `arm(driver, r, gs, core)` (no-op with `armed()` off) gives the driver the core ability table, the registry derived from the position, the spent keys and trace counter of `Pending::resolver`; **only `drive_windowed` calls `arm`**; the pay step, `strategy_cards_core` and the production window run their own loop and never arm | `window_driver.rs`, `dual_window.rs:699` |
| Oracle: `Subject` trait (`new/options/answer/step/state_json/events_since/decisions_since/rng_positions/finished/round/rules_version/disabled/decisions_hash`), `MaskSubject<LegacyMask|CoreMask>`, `Pilot::{Bot, Script}`, categories `Decision`, `DecisionHash`, `Value`, `Event`, `Rng`, `State`; label `oracle::<subtype>`; stops a scenario at its first unallowed difference | `bridge/src/oracle.rs` (758 lines) |
| `zero_bail.py` (on int-8 `67994b33`, not on `core-d2-l1`): counts every `[bail: ..]` and `[gate: ..]` of the all=dual reports by cause; `--enforce` fails on any | tool |
| Lobby factions: 24 (`seating::LOBBY_FACTIONS`: 17 base + 7 PoK); `GameSpec.factions` pins every seat's faction; `ti4-sim/examples/base_faction_soak.rs` rotates one target faction per seed over 27 ids (includes dropped ones) | grep |

---

## 2. What "the core drives this" means

### 2.1 Definition of done for an arm (apply to every item below)

An arm A is native when ALL of the following hold, in the production path and with `Step::Lend(A)` unreachable:

1. **Opening is core code.** The function that opens the window (or starts the phase job) is a core function that writes `Pending::open(Window::X(payload))` (or the flag) and the
   same state writes and events, in the same order, as the string body. Opening sites that live in OTHER arms (e.g. `open_occurrence_event_scoring` called from tactical, invasion, agenda
   close, pass) become `Next::Open(Window)` / a `Pending::open` of the owning core window. No `Game::scoring = Some(..)`-style assignment remains for that kind.
2. **The question is core code.** `ask_arm` + a core function `question(core, k, facts) -> Option<Choice>` produces the typed `Choice` from `(Pending window, state)` only.
   It equals what the string `legal_options` rendered (`render::Typed` of it compares equal under the oracle: prompt, option ids/kinds/labels, `DecisionContext`, `details`).
3. **The answer is core code.** `apply(core, k, pick) -> Applied` is `restore machine from the window payload -> Driver::resume -> store payload back -> follow the closing rule`.
   Questions asked INSIDE the answer (a played card, a hand-limit discard) are asked through the live `Asker` within the same step.
4. **Closing and chaining are core code.** The close function pops the stack, performs the closing writes/events and names what comes next (open the next window, set the next flag,
   move the phase), exactly as the string step does when `legal_options()` is `None`.
5. **No string call in the path.** After the arm's last lender is gone, `lent_sites.py --check` rows of its string functions show no production caller reachable from the arm,
   the `d1::step [bail: <arm>]` census row is 0 and `Lent::Arm(A)` is removed from the enum (`unreachable!` in `step()` for A).
6. **State is in `CoreState`.** The `Game` field of the kind is deleted (`open_windows()` reads `Pending::stack`; `GameSnapshot`/`fork` clone `CoreState` only). Until D2-8d moves
   `Game` this is a checklist item for that package; the field must be unused in the native path.
7. **Tests exist (2.4).**

### 2.2 The step loop in the live path (shape to build, KF1/KF2)

```text
Game::step() (wrapper)                              -- StepResult as now, Table as now
  table.begin_step(); dice scene
  prelude(core, k, galaxy)                          -- KP0: the per-step hooks, in the order of step_flow
  facts = Facts::of_core(core, blocked)             -- KF1 (derives prepared/closing/picks from CoreState)
  match driver::step::step(core, &facts, needs, env)
    Stopped(arm)               -> result
    Ran{arm, ..}               -> result (hooks are now core code; `lent` is None for a native arm)
    Asks(ask_arm)              -> q = driver::ask::question(core, k, ask_arm)        -- may be None for window arms: then it is the close path
                                  pick = asker.ask(q)                                 -- Table via render
                                  driver::answer::apply(core, k, pick, &mut asker)    -- nested asks go through the same asker
    Lend(l)                    -> only for arms not yet native (scaffold); product: Err(Unmapped{label}) (KQ)
```

`step()` is extended (not replaced): for the window arms `Step::Asks(AskArm)` is returned while a question exists and the arm's close path runs when `question()` is `None`
(this is exactly `step_scoring`'s `let Some(choice) = self.legal_options() else { return self.begin_status_bookkeeping() }`). `Needs` gains a trait object for what the position does
not hold (the unit names `spec_ops_unit` already is one; content texts, the rules-version table, `strategy_cards` set, round-income knobs).

### 2.3 The 14 lent-whole arms: owner and what it takes

`Arm` -> string function (`game.rs`) -> what native means -> package. "Other" = machine island, written by the companion document; "Dropped" = refused at creation (P1), arm unreachable.

| # | Arm | String | Core status today | What it takes to run natively | Package |
|---|---|---|---|---|---|
| 1 | `LeaderFollowers` | `step_leader_followers` 4379 | `Window::LeaderFollowers`, `leader_followers_back`; Winnu hero window exists (`hero_windows::WinnuHero` via `kernel::fx_perform`) | question = first of `asks` (`factions::leader_strategy_followers` question per seat, a core fn), answer `yes\|<seat>` appends to `followers`; at the last answer open `Window::StrategySecondary` (`StrategySecondaryWindow::foreign`) | KX2 |
| 2 | `Secondary` | `step_secondary` 6046 (+`secondary_step`) | `Window::StrategySecondary`, `SecondaryStep`, `strategy_secondary::advance`, `secondary_back` | follower question and resolution live; `follow` (`strategy_cards::follow` -> the secondary windows), `resolve_faction_strategy`, `complete_leader_strategy`, `STRATEGIC_ACTION_COMPLETE`, `finish_action`; preview windows (planning forks) need `Secondary.preview` | KX1 |
| 3 | `EventScoring` | `step_event_scoring` 6250 | `Window::EventScoring`, `ScoreOne`, `ScoringBook` | question/answer live (`score_step` 6170 = `objectives_write::score_one`); close: `agenda_queue_after_event_scoring.take()` -> `open_next_vote`; game end (`GAME_FINISHED`); openers from other arms via `Next::Open` | KS1 |
| 4 | `Scoring` | `step_scoring` 6149 | `Window::Scoring`, `ScoringBook` | as 3; close = `begin_status_bookkeeping` | KS1, KS3 |
| 5 | `Tokens` | `step_token_gain` 6336 | `Window::Tokens(Box<Tokens>)`, `tokens_back`, `gain_pool_choice` | new `TokenGain` machine (question per token, pool answer, `COMMAND_TOKEN_GAINED:<pool>`), `with_pool_details`; close = `finish_status_phase` | KS4, KS5 |
| 6 | `Voting` | `step_vote` 6698 | `Window::Voting(Box<Voting>)`, `rules::agenda::window::Vote`, `voting_back` | restore `Vote` from `state::Voting`, per-answer `VOTES_CAST` typed event (rules-version branch), `state.agenda_casts` mirror, goods window (Keleres agent: dropped), close = `close_vote` | KV2, KV3 |
| 7 | `Diplomacy` | `step_diplomacy` 5816 | `Untyped` | **Dropped** (structured diplomacy): refusal at creation; arm `unreachable!` | KQ |
| 8 | `Aftermath` | `step_aftermath` 5359 | stage tag only | **Other** (aftermath chain, I3) | other |
| 9 | `AgendaTalks` | `step_agenda_talks` 6600 | `Untyped` | **Dropped** (needs `state.diplomacy.enabled`, refused) | KQ |
| 10 | `Trade` | `step_trade` 5890 | `Window::Trade`, `TradeWindow` | **Other** (trade, I5); its menu opening is KM2 | other |
| 11 | `Tactical` | `step_tactical` 3669 | `Window::Tactical` | **Other** (I4) | other |
| 12 | `Status` | `step_status` 6120 | none | open the 81.1 scoring window (`ScoringBook::new(initiative_order, status = true, ..)`), `STATUS_SCORING_BEGAN`, `STATUS_RESOLVED` flag, `entropic_scars::resolve_status_start` (Thunder's Edge: no-op when TE is refused, VERIFY the early return) | KS2 |
| 13 | `Agenda` | `step_agenda` 6435 | none | the phase composer: `AGENDA_RESOLVED` flag, reveal order, take the top agendas, the loop of `open_next_vote`, `ready_after_agenda_phase`, `AGENDA_PHASE_RESOLVED` | KV1 |
| 14 | `StrategyAnnounce` | inline in `step_flow` (`strategy_phase_announced`) | flag exists | `STRATEGY_PHASE_BEGAN` typed event + flag; needs the live emit with the reaction windows (`Emit`) | KP3 |

Partly run arms with a lent remainder: `TurnPrelude` (KP1), `Phase` (KP3: the Status and Agenda arms of `advance_phase`, `step_phase` 7224), `Ask` (KM, KP4). `Finished` has a string
body that is lent too (`complete_leader_strategy`, `secondary = None`, `diplomacy::settle_game_end`): KX2 and KQ.

### 2.4 Test taxonomy used below

| Name | What | Cost |
|---|---|---|
| **core unit** | a window/composer test in `ti4-engine-core-tests/tests/<arm>.rs` with `rig::Rig` (`Rig::new(scenario, window).run().pick..`), plus the mutation single-file check on the new core file | cheap (runner: `cargo test -p ti4-engine-core-tests <file>`) |
| **arm lockstep** | engine `#[cfg(test)]`, `--profile lockstep`: build a position with `fixtures::game` (or a `golden_positions` entry), `fork()` it, run the string `step()` on one copy and the native arm (KF5 `NativeSubject`, only that arm native) on the other with the SAME answers (`Pilot::Script`), compare the materialised `GameState` JSON, events, decisions, rng positions. The arm-level cut of the oracle | cheap |
| **oracle smoke** | `oracle --game 0 --arms <list>` (KF5): one golden game, subject A legacy, subject B native for the listed arms and lent (scaffold) for the rest; reports `oracle::<subtype>` differences | cheap-ish (minutes), runner |
| **lockstep suite** | `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1 cargo test --profile lockstep -p ti4-engine --lib` (4 860 tests at int-8) | once per package end |
| **playthrough** | whole game, `Lend` = failure (KF5/KQ) | smoke (1 faction, 1 game) cheap; the 24-faction matrix END-OF-PHASE ONLY |
| **G-zero** | the zero-bail acceptance (section 9) | END-OF-PHASE ONLY |

Every failing check gets a regression test before its fix (standing rule, `feedback_regression_test_per_failure`).

---

## 3. KF: the shared foundation (lane A; start now)

Nothing in sections 4-7 can be switched on without KF1, KF2, KF5; the core-side work of all lanes (machines, composers, unit tests) does NOT wait for KF and starts at once.

**KF1. `NativeArms`, `Facts::of_core`, `Step` extension (2-3 h).**
`driver/session.rs`: `Facts::of_core(core, blocked)` derives `turn_prepared = pending.prepared_turn_seq == Some(meta.turn_seq)`, `turn_closing = pending.turn_closing.is_some()`,
`starting_picks_done = transient.misc.starting_tech_picks.is_empty()` (keep `Facts::of` for the dual). `driver/step.rs`: `pub struct NativeArms(u32)` (bit per `Arm`, const sets
`NONE`, `ALL`, `from_env("TI4_NATIVE_ARMS=status,agenda,..")` in the bridge only); `step(core, facts, needs, native)`: an arm not in `native` returns `Step::Lend(Lent::Arm(arm))` exactly as today (so the
existing tests and the dual are unchanged with `NativeArms::NONE`). Add `Step::Asks(AskArm)` handling for the window arms (2.2). Tests: the five existing `step.rs` tests with `NONE`;
a new test per arm class that the arm is native iff in the set (core unit).

**KF2. The live drive loop and the `Asker` (6-10 h; the largest single unknown).**
New engine module `crates/ti4-engine/src/live/mod.rs` (new files only; it moves to the wrapper at D2-8b with `render`, so it must not import `dual*`, `Recording` or `d1_*`):

```rust
pub trait Asker { fn ask(&mut self, core: &CoreState, q: &ti4_engine_core::choice::Choice) -> Result<Pick, Stop>; }   // Stop = IllegalChoice | Blocked | Replay-exhausted
pub fn drive_live<W: Window>(driver: &mut Driver<W>, core: &mut CoreState, k: &Kernel, r: &Render, asker: &mut dyn Asker) -> Result<Done, Stop>;
```

Port from `drive_windowed` (`dual_window.rs:670-790`): the question rendering (`Typed::new`/`new_at` with `Position` for production-family windows, `complete` hooks that add the display
facts `Choice.details`, `complete_nested`), `parse_pick`/the option-id fallback, the `unknown()` bails become `Stop::Unmapped(label)` (a hard error: in live mode an ability the core does not
know is a defect, not a lend), the resolver. Differences from the replay driver, each a decision to make explicit in the code:

* **Resolver**: always armed: `Driver::from_state(window, core, registry, table)` (`driver/mod.rs:149`) with the registry from `AbilityTable::registry(state, order, name)`, `driver.store(core)` when the window ends or asks (`mod.rs:156`). This is `window_driver::arm` made unconditional for the live path (KF6).
* **Dice**: `driver.with_seeds(..)` from the game's streams (the replay driver bails "dice rolled by an ability in a window lent no stream" if not seeded).
* **Rollback**: a step that fails (illegal answer, `IllegalChoice` from the table) restores the `CoreState` clone taken at the step start (the string clones `state` in `begin_status_bookkeeping`, `step_secondary`, `step_vote`; the core clone is the generic form). Cost: a `CoreState` clone per step (`ENGINE_CORE_STATE.md` 3: a checkpoint is `CoreState::clone()` over `Cow`/`Arc` slices; VERIFY the real cost with the `state_size` bin before KF2 is done).
* **Facts from the position**: every `*Facts`/`Recording` input the adapters filled from the string state is computed by a core function in live mode. Inventory first (K0): `grep -n "Facts {" crates/ti4-engine/src/*_dual.rs *_core.rs *_write.rs`; each becomes `fn facts(core, k, seat) -> *Facts` in the window's own module or the factions crate.
* **Events**: the core emits plain events through the P1 queue (`state/plain.rs`, `PlainRec`) and typed events through `Sx::emit` -> `Resolver`; the string's `emit` vs `emit_typed` vs `mirror_timing_log` (`game.rs:3854`) double-logging rule ("only `emit_typed`, `mirror_timing_log` copies what the resolver emitted") must be reproduced: the live path renders `events: Vec<String>` from the queue at the end of the step, in queue order (VERIFY `render/events.rs` already does this for the dual compare: it does, it is what `Recording.plain/mirrored/ordered` compare).

Tests: core unit for the `Asker` contract with a scripted asker; engine arm lockstep on an already-ported window (pick `strategy::secondary`, the smallest) driven by `drive_live` against the string
(`drive_live` and the replay `drive` must produce identical state/questions on the same script: this is the validation of KF2 itself, cheap, and it runs over the existing `dual_window!` lockstep corpus
by turning the `Core` backend of ONE area from "run the string" to "run `drive_live` with a replay asker" under a test-only env).

**KF3. `KernelEnv`s for `leaders::Env`, `strategy::Env`, `abilities::Env` (3-5 h).**
Pattern: `rules/trade.rs:22` (`KernelEnv{core,kernel}` answering through `kernel.hooks.<group>.<slot>`). New files `rules/leaders_env.rs`, `rules/strategy_env.rs`, `rules/abilities_env.rs`
(L4: they use `kernel.hooks`, allowed). Per method decide: a hook slot answers it (most: `faction_commander_unlocked`, `leader_action`, `imperia_grants`,
`secondary_is_free`, `substitutes_primary`, `status_tokens`, `waived_prerequisites`, each through the matching group of `kernel.hooks`, VERIFY the group per slot in `kernel/hooks/mod.rs`), a core query answers it (`mecatol_area` from the galaxy, `can_research` from `query::tech`,
`secret_hand_all_scored` from `query::objectives`), or it needs a new core query (`other_action_resolves`: Z'eu, Doctor Sucaban's swap, the Thunder's Edge Xxcha hero; `leadership_affordable`,
`technology_secondary_affordable` read `payment::affordable`: `query::economy::Spend`). **VERIFY first**: for each method of the three traits, list the string function it calls in `leaders_view.rs`/`strategy_view.rs`
and tick whether a core equivalent exists; the methods without one are new core queries (each 0.5-1 h). Tests: for each method, a lockstep that the `KernelEnv` answer equals the `LeaderView`/`StratView` answer over
`golden_positions` (the existing view dual compares `LeaderView` to the string; add `KernelEnv` as a third party).

**KF4. `CoreState` fields the composition needs (1-2 h).**
Add to `Pending`: `failed_component_actions: SmallList<Code, 8>` (cleared in `advance_turn_inner` and `reset_for_hypothetical`; the menu filter `driver::menu::turn_menu(.., failed)` takes it), and nothing else:
the other `Game` fields are wrapper config (`strategy_cards: Vec<StrategyCardId>` -> `Kernel`/`Needs`, `round_income_trade_goods/_seat` and the `STATUS_INCOME_TRADE_GOODS` experiment static -> wrapper knobs read by the
phase events, `blocked` -> wrapper), or already there. `Facts` flags: `blocked` stays the caller's. Round trip: extend the `Pending` codec in `core_sync` (the `alpha_roundtrip` and `core_sync_gate` fixtures must keep passing). Unit: codec test.

**KF5. `NativeSubject`, arm mask, bail-as-failure and the playthrough bin (6-9 h; shares D2-6b(b), `oracle.rs`).**
* `oracle.rs`: third `Subject` impl `NativeSubject { core, live: LiveGame, lender: Option<LegacyGame> }` in the bridge: it runs `driver::step::step` with `NativeArms` from `TI4_NATIVE_ARMS`; a `Lend` is executed by the scaffold (the existing `TI4_CHURN=state` machinery: `Game::churn_state` / `core_sync::materialize` + `reverse_lend::with_string` at label `step::<arm>`) and counted; the count is the output (`lent arms: status 1, agenda 0, ..`). This scaffold is bridge-only test code and is the "B at most temporary": it must not enter the production crate.
* `Lend` counter exported next to `unmapped` (`product::lent_count(label)`), used by KQ.
* `bin/playthrough.rs`: `playthrough --factions all|<ids> --players 4|6 --seeds N --maxsteps M [--native <arms>]`: for each lobby faction `f` build a `GameSpec` with `f` at seat 0 and the rest drawn (reuse `workload::golden_positions`' config reader and `ti4_selfplay::spec`), `starting_tech: true`, `thunders_edge: false`, all areas native, `product::set_strict(true)`, `unsupported::Policy::Panic`; the rule bot plays (as the oracle's `Pilot::Bot`) to `finished` or `maxsteps`; per game print rounds, steps, decisions hash, lent arms and unmapped count; exit 1 on any `Lend`, `Unmapped`, panic, unfinished game, or step-limit. Output format as `dual_run` so `zero_bail.py` can read it.
* Tests: subject A vs A reports 0 (exists); injected difference reported (exists); new: a position forced into `Lend` shows in the count and fails `playthrough`.

**KF6. `armed` becomes the live semantics; the `driver_for` sites (2-4 h cheap + the armed dual END-OF-PHASE ONLY).**
`window_driver::driver_for` stays the one constructor. Add `driver_live(window, core, kernel_registry, table) -> Driver<W>` = `Driver::from_state` (the live path never goes through `Driver::new`). Per D2-remaining 8a the
first half of G-flip is the 20-game armed dual (`TI4_DRIVER_ARMED=1 dual_run ... all=dual`); the new differences it shows are the nested-ability windows that are about to be live, and each gets a regression test. Assign the 99 sites
(section 8 table) to the lane that owns the window; a site is "converted" when its adapter has a `live::open_<window>` that builds the driver with `driver_live` and the adapter's replay closure is no longer on the production path. The
sites that never `arm` (pay step, `strategy_cards_core`, production window, `leaders_core` own loops) must get `arm` in their live constructor or the live path would run without abilities: checklist item per site.

**Order inside KF:** KF1, KF4 (parallel, 1 day) -> KF2 -> KF5; KF3 and KF6 in parallel with KF2. Gate: KF2's self-validation test (live vs replay on the existing corpus, cheap).

---

## 4. KP: C2, turn hooks, phase events, step prelude, ask-side phases (lane B)

String originals: the `turn_prelude` block of `step_flow` (`game.rs:2518-2575`, arm noted at 2522), `advance_turn_inner` (7428), `pass_on_tail` (7548), `step_phase` (7224), `phase::advance_phase`
(`phase.rs:234`), the Strategy and StartingTech asks of `apply_choice`.

**KP0. The per-step prelude, native (3-5 h).** New `driver/prelude.rs`: `pub fn prelude(core, k, galaxy, env) -> Prelude` running, in the string's order: (1) starting-tech `settle` (if picks left; KP4),
(2) `announce_staged_ground_events` + `announce_gains` (staged events: the P1 queue is already the core's: the function becomes "flush `Pending::staged` through `fx::announce_staged`" + the gains diff against
`marks`/a per-seat snapshot in `Pending`, VERIFY where `announce_gains` keeps "what it held at the last step": it compares each seat with the previous step; needs a field `Pending::gains_seen` or `Marks`), (3) `space_stations::reconcile_all` and
`coexistence::reconcile_all` (Thunder's Edge / the three coexistence grants: with TE refused these are no-ops; VERIFY by running `lent_sites`/grep that `reconcile_all` returns `false` on a non-TE state and drop them if so,
else port occupancy), (4) the `nexus_unlocked` latch (`seating::nexus_is_triggered`: a pure board read, port to `query::board`), (5) `legendary::settle_control_points` (core `rules/legendary.rs` exists), (6) `laws::apply_to_galaxy` and
`movement::replay_map_edits` on the owned galaxy (the core has `CoreGalaxy`/`state::map`; the map edits are in `Slice::Map`; a failing replay sets `blocked`), (7) `faction_techs::roll_spec_ops` (a die per Spec Ops II destroyed: the dice stream and `SPEC_OPS_*` plain events). `keleres::reconcile` and `crimson::enforce_sundered` are dropped with their factions (delete from the native path; the creation refusal covers them).
Each hook: core function + lockstep test on a position where it fires and one where it does not (cheap). The dual already counts these (`d1::reconcile [bail: <hook>]`), so the hooks that change state in the golden corpus are known: use that list (`d1_pending::probed`).

**KP1. TurnPrelude arm complete (4-7 h).** `Window` `TurnPrelude { seat, stage }` in `driver/windows/turn_prelude.rs`, composing in the string's order: `technology::start_turn` -> core `TurnStart` (exists; the `Child<TurnStart>` with `ChaosFacts` from the core) ->
`refresh_commander_unlocks` (`leaders::check_unlocks`: `rules::leaders::unlockable` + status writes; emits `LEADER_UNLOCKED:<id>`) -> Military Support (`promissory::turn_started` -> `rules::promissory_windows::PlaceUnits`, W-L5; `MILITARY_SUPPORT_USED`) -> `turn_flow::return_spec_ops` (exists, `SPEC_OPS_RETURNED:<seat>:<n>`) -> `turn_flow::mark_prepared` (exists) -> `TURN_BEGAN` typed event
(`sync_timing_context` then `emit_typed`: reaction windows open). Not a persisted window: it runs inside one step (the table blocks), `prepared_turn_seq` is the guard (`turn_write::check_prepared`). Replace `Lent::TurnHooks` by the window; `Step::Ran{lent: None}`.
Trap: the string prelude falls through to the question of the same step (`step_flow` continues after the `if` block), so the native step must return `Ran` then continue into `Ask` in the same call (the dual's "first arm only" note, `d1_pending::note_arm`).

**KP2. End-of-turn hooks (3-5 h).** `Hook::{MinisterOfWar, TechnologyEndTurn, LegendaryEndTurn}` (`turn_flow::HOOKS`) become windows run in order by a `TurnEnd` composer: `MinisterOfWar` (exists, `driver/windows/minister.rs`), `TurnEnd` (technology, exists), `Legendary` (exists; "readying last"), then the retention re-check (`take_retention`), `EnforceAll` (exists), `pass_on` (exists) with the announce list rendered as `TURN_PASSED`/`TURN_SKIPPED`/`TURN_PUPPET` plain lines plus the typed `TURN_PASSED` emit (reaction windows; the `TAINTED` watch of `emit_typed_watching` is a dual device and is dropped: in live mode the core re-reads `meta.active` after the emit). `advance_turn_inner` also clears `failed_component_actions` (KF4) and `actions_this_turn`. Test: arm lockstep over the 4 branches (retained turn; Minister retains; Jupiter Brain retains in the end-of-turn window; plain pass-on; Crisis skip; Puppets).

**KP3. Phase events and the remaining `Phase`/`StrategyAnnounce` arms (4-7 h).** One `driver/phase.rs` composer for `step_phase` (7224): the typed `STRATEGY_PHASE_ENDED` before the flip (the factions' initiative hooks, `hooks_strategy::strategy_phase_ended`, run via `kernel.hooks`; `stock_unclaimed_cards`), the Action->Status
(`action_phase_ended` exists; `STATUS_PHASE_BEGAN` plain + typed, the experimental income knob), Status->Agenda/RoundEnded (`custodians_removed && !finished` -> `AGENDA_PHASE_BEGAN` plain + typed; else `begin_next_round(core, cards)` exists, round income knobs, flags reset, `ROUND_BEGAN`,
`laws::revolution_levy` (Anti-Intellectual Revolution; a core rule? VERIFY, else port), typed `STRATEGY_PHASE_BEGAN`), Agenda->RoundEnded. `StrategyAnnounce`: the `STRATEGY_PHASE_BEGAN` typed event + flag at round 1. Replace `Lent::{PhaseEvents, StrategyHooks}`.
Typed events through the live `Sx::emit` (reaction `Emit` window): this is where the nested asks of "start of the strategy phase" cards happen.

**KP4. The Ask arm's strategy-phase and starting-tech halves (5-8 h).** (a) `AskArm::Draft`: question = `strategy_options` (`strategy.rs`, `phase::next_strategy_picker`: the core has `rules::strategy_flow` for the mutation half and the `dual_typed!` queries
`phase::strategy_pick_order/next_strategy_picker`); lone-card auto-resolve (`table.auto_resolve(&choice, "only one strategy card left")`, never journaled); answer = `take_timed_strategy_card` (the string 7614: trade goods on the card to the picker via `strategy_flow`), `STRATEGY_CARD_CHOSEN`, and when no options remain `imperial_arbiter` (6998; Naalu/Arbiter swap) and `faction_techs::offer_quantum_datahub`. (b) `AskArm::StartingTech`: question `starting_tech::choice`, answer `starting_tech::apply`, `STARTING_TECHNOLOGY_CHOSEN`; the picks live in `Transient::misc.starting_tech_picks` (exists). **Not covered by any golden game (finding 6)**: add a corpus config `tools/rulebot/golden-starttech.json` with Winnu, Argent Flight and the other lobby factions at 4/6 players, `starting_tech: true` (the Keleres/TE variants of the picks are dropped/refused). Question 2.

**Order in KP:** KP0 -> KP1 -> KP2 (all three use the live loop) in parallel with KP3 and KP4 (KP3 needs the reaction `Emit` live, i.e. KF2).

---

## 5. KS: C1 status half (lane C)

**KS1. The scoring family live: `Scoring`, `EventScoring` arms (6-10 h).**
Core: `driver/windows/scoring.rs` (L5) with `fn question(core, k) -> Option<Choice>` = `ScoringBook::asked(offers)` + `ScoreOne::new(who, status, offers, occurrence)` question, `fn answer(core, k, pick)`, `fn close(core, k) -> Closed` where `Closed::{Status => KS3, Event => agenda queue / nothing}`. The offers come from `query::objectives` (`Offers` trait of `scoring_book`, currently implemented through the typed view `objectives_core.rs`: VERIFY it needs no string state).
Openers: `Next::Open(Window::EventScoring(..))` from the rule windows (pass: Prove Endurance `driver/dispatch.rs::last_to_pass`; tactical/aftermath: other agent; agenda close: KV3). Open sites in `game.rs`: 3151, 5340, 5447, 5529, 7163 (and tests 13921). `GAME_FINISHED` when `state.finished` after a score (98.7). Plots Within Plots (`PlaceCard`, Firmament: dropped; the `plot_card` field stays `None`).
Tests: core unit (Rig with `ScoreOne`, existing `objective_windows` tests); arm lockstep for both arms on 3 positions (status scoring with 2 scorable objectives; a secret event window; an empty window that closes without a question).

**KS2. `Status` arm (2-3 h).** `driver/windows/status.rs::open_status(core, k)`: `ScoringBook::new(&initiative_order, true, false, false)` -> `Window::Scoring`, set `STATUS_RESOLVED`, plain `STATUS_SCORING_BEGAN`. `entropic_scars::resolve_status_start` (TE; verify it is a no-op on a non-TE state and drop the call under the TE refusal).

**KS3. Bookkeeping 81.2-81.4 as one in-step window `StatusBookkeeping` (8-14 h; the biggest piece of C1-status).**
Stages: `CheckUnlocks` (every seat, `LEADER_UNLOCKED`) -> `Reveal` (81.2: `rules::objectives` reveal exists; an empty deck ends the game: `state.finished = true; report.game_ended`, `GAME_FINISHED`) -> `Draw` per seat in initiative order (81.3): hooks `action_card_draw_requested` (Yssaril Scheming asks), the Neural Motivator +1 (`technology nm`), `action_card_draw_bonus`, the draw (`take_top` with `deck_reserve`: a non-empty reserve is refused at creation, so plain pop), `action_cards_drawn` hook, the rules-version branch (`>= RULES_FACTION_BUGFIXES`: the draw goes through the hand limit: `HandLimit` window exists), `report.action_cards_drawn`; the whole-step rollback on an illegal answer is the generic KF2 rollback -> `Recall` (81.4: every system's `command_tokens` -> `report.returned_command_tokens`) -> `Neuraloop` (`relic_windows::Neuraloop` exists; `OBJECTIVE_REPLACED:<a>:<b>`) -> `STATUS_BOOKKEEPING_RESOLVED` -> build `Tokens` (`TokenGain::for_status`: counts via `abilities::status_tokens` (Env, KF3) over the initiative order, Cybernetic Enhancements' extra token) + `promissory::return_all_foreign("ce")` -> `Pending::open(Window::Tokens(..))`.
Port from `status.rs:72-199` (`resolve_before_token_gain_with`) and `game.rs:6274-6372`. Tests: core unit per stage with fixed decks; arm lockstep on 3 positions (plain; Scheming draw with a discard question; deck exhausted -> game ends); the report equals `StatusPhaseReport` field by field (the existing `tokens_back`/`status` round trip is the comparator).

**KS4. `TokenGain` window and the `Tokens` arm (3-5 h).** New machine `rules/token_gain.rs` (`Window` impl, stage = next seat/index, `Pending` payload `state::Tokens` already): question `gain_pool_choice(who, left)` (exists) + the string's `with_pool_details` display facts (the wrapper's `complete` hook, KF2), answer appends `(Seat, Pool)`, plain `COMMAND_TOKEN_GAINED:<Pool:?>` (note the Debug-formatted pool name: `format!("{pool:?}")`), `is_complete` closes into KS5. Restore from `state::Tokens`. Test: core unit; lockstep including Cybernetic Enhancements and Versatile/Hyper Metabolism counts (they fold into `status_tokens`).

**KS5. `finish_status_phase` 81.5-81.8 (6-10 h).** Per initiative-order seat the redistribution question (`strategy_cards::redistribute_tokens`, subtype `status_redistribute_tokens`; the core has the Warfare redistribute window `WarfareRedistributeTokens` to generalise: a `Redistribute` window parameterised by context), then per seat holding cards the typed `STRATEGY_CARDS_WOULD_RETURN` events (reaction windows: Political Stability etc.), then `resolve_after_token_gain` (`status.rs:199-310`): exhausted technologies cleared, relics/legendaries ready (`rules::explore::ready_relics_and_legendary` exists), leaders ready (`leaders::ready_all`, report), Crown of Emphidia (`relics::crown_of_emphidia_point`), `ready_all_planets`, repair sustained units, Minister of Policy draws (`laws::draws_at_status_end`), 81.8 return strategy cards (`stability` retains; `strategy_write::clear`; `passed = false`), then plain `COMMAND_TOKENS_GAINED`, `faction_techs::genesis` (`GENESIS_INFANTRY:<owner>:<system>`), `STATUS_PHASE_RESOLVED`, typed `STATUS_PHASE_ENDED`. Port as `StatusFinish` window (in-step). Tests: core unit per sub-step with the report as oracle; lockstep on 3 positions (a retained card; Crown; Minister of Policy).

**Order in KS:** KS1 (needs only the core) -> KS2 -> KS3 -> KS4 -> KS5; KS3 and KS5 can be written in parallel by two agents split by file (`status_bookkeeping.rs`, `status_finish.rs`) after the shared `StatusReport` builder is agreed (it exists as `state::StatusReport`).

---

## 6. KV and KX: C1 agenda half and the strategic action (lanes D, E)

### 6.1 KV: the agenda phase

String: `step_agenda` 6435, `open_next_vote` 6458, `reveal_agenda` 6639, `committee_formation` 6842, `start_vote` 6517, `step_vote` 6698, `close_vote` 7130, `run_agenda_effect` 6801, `agenda::resolve_agenda_phase`/`ready_after_agenda_phase` (`agenda.rs:57,105`).

**KV1. `AgendaPhase` composer and the `Agenda` arm (6-10 h).** `driver/windows/agenda_phase.rs` (L5, composes the L4 windows): `AGENDA_RESOLVED` flag; `custodians_removed` guard (else the phase is skipped upstream in `advance_phase`);
`reveal_order` (`agenda_typed::reveal_order`, exists in core as a query); take the top `AGENDAS_PER_PHASE` (2); no agenda -> `ready_after_agenda_phase` + `AGENDA_PHASE_RESOLVED`; else the loop of `open_next_vote(queue)`: `Reveal` (exists: `agenda_reveal_dual`) -> if it elects a player, `Committee` (exists) -> `start_vote`: `Vote::new` and `open` (the reveal-time voting order incl. Nekro's bar, `is_nekro` is `false` in the core `KernelEnv`: the string applies it in the adapter: KF3-style fix: move into `KernelEnv`), `state.agenda_casts` mirror, `Window::Voting(queue)`. The `AgendaTalks` branch is dropped (diplomacy). `ready_after_agenda_phase` incl. the Checks and Balances limit (`laws::agenda_ready_limit`, a core query). Executive Order (Keleres) is dropped: `Window::ExecutiveOrder` is never opened; leave the kind for the codec but refuse the position.
Tests: core unit: composer with 0, 1, 2 agendas, a Veto chain, an agenda that elects nothing (skipped); lockstep: 3 golden positions at the start of the agenda phase.

**KV2. The `Voting` arm (4-7 h).** `Vote::restore(&state::Voting, core, k) -> Vote` and `Vote::to_pending(&self) -> state::Voting` (VERIFY absent, add: the payload already has `stage`, `ballot`, `counts`, `casts`, `obligation`, `demanded`, `predictive`, `recombination_offered`, `spender`, `order`, `choices`), question from `Vote` (exists), answer applies and, when `answered_outcome || rules_version < RULES_BRIBERY_PER_PLAYER`, emits typed `VOTES_CAST` (reaction windows: Bribery). Goods window (Keleres agent) dropped. Version branches must be kept (the old benches pin old versions). Test: core unit already covers `Vote`; add restore/to_pending round trip (property: restore(to_pending(w)) == w for every stage) and lockstep per `VoteStage` (Outcome, Recombine, Tribute, Planets, TradeGoods, Predictive, Tiebreak).

**KV3. Close, effects, chaining (6-10 h).** `Close` (`agenda/close.rs`), `Resolve` (`agenda/effects.rs`: `run_agenda_effect`), `OfferDiscard`, `DeadlyPlot`, `CovertDraw` exist as windows with adapters (`agenda_close_dual.rs`, `agenda_effects.rs:470,512`); compose them as the string `close_vote` does: clear `agenda_choices/in_play`, `close_window` -> `settled`; `resolved` -> `run_agenda_effect`, `begin_feat_occurrence`, `ElectedByAnAgenda` feat, `open EventScoring(Agenda, AnyPerPlayer)`; if an event window opened, `AgendaQueueAfterEventScoring(queue)` else loop (KV1). The two table effects (`agenda_effects_lockstep.rs`) stay in the core windows. Test: lockstep per agenda family (election, For/Against law, Covert Legislation, Veto) on the golden positions that close a vote (the corpus has them: 4-6 agendas per game, cited `lent-bodies.md` 1).

### 6.2 KX: the strategic action (secondary, followers, leader strategy)

**KX1. `Secondary` arm incl. preview windows (6-10 h).** The follower machine exists (`SecondaryStep`, `strategy_secondary::advance`, `Window::StrategySecondary`). New: live `question`/`apply` (answers `Followed`/`Declined`, `STRATEGY_SECONDARY_FOLLOWED/DECLINED`), `follow` = the secondary of the card as a core window (`Spawn::` + `strategy_primaries::*Secondary`, `Research`, `Leadership`...; the string `strategy_cards::follow` + `resolve_faction_strategy`), rollback of the follower's effect on error (generic KF2), the completion path (`complete_leader_strategy`, `STRATEGIC_ACTION_COMPLETE`, `finish_action` -> closing menu or `advance_turn`, KP2), the after-tactical variant (Thunder's Edge Warfare: dropped; the kind stays). **Preview**: add `preview: bool` to `state::Secondary` (codec + `secondary_of/back` + `d1::pending.roundtrip`) and `prepare_hypothetical_turn/secondary` in the wrapper over `CoreState` (`reset_for_hypothetical` 2011: clears the windows and scratch, bumps the four sequence counters). Test: lockstep incl. the planning fork cases of `ti4-server/tests/it/{planning,fork_equivalence,secondary_preview_differential}.rs` (they are the existing differential tests for forks: run them with the native arm).

**KX2. Leader strategy and followers (3-5 h).** `select_leader_strategy_followers` (4363): `factions::leader_strategy_followers` -> `LeaderFollowers` window (Mathis Mathinus) -> `Secondary(foreign)`; `complete_leader_strategy` purges the hero (`leaders::purge`) and emits `COMPONENT_ACTION_RESOLVED`; the `Finished` arm's cleanup (`complete_leader_strategy`, `secondary = None`). Winnu hero is the only user.

**KX3. Strategic dispatch from the menu (4-7 h, overlaps KM2).** The `Strategic` option of `apply_choice`: spend the token (secondary-token rules), run the primary window (core windows exist per card), open `Window::StrategySecondary` with the followers in resolution order (`strategy::secondary_followers`, a query), `on_strategic_action_chosen` observer (host staging hook, `game.rs:2140`: keep as a wrapper callback fed from the dispatch result), the Imperial/Politics/... primaries that hit `Unsupported` today are the lent-bodies W6 list (separate track).

---

## 7. KM: C3, the action-menu dispatch (lanes F1-F3)

Sources in `driver::menu::ORDER` (13): Strategic, Tactical, Transactions, Contacts, Payments, Relics, Exploration, Technology, ThundersEdge, Faction, Mapped, Leaders, ActionCards. Today the core assembles the menu from option ids supplied by the string
(`MenuParts`, `menu_dual.rs`) and only `Pass`, `Tactical`, `Strategic`, `EndTurn`, `Token` are native `Opt`s; every other option is `Opt::Lent(Code)` = an interned string id (`choice/turn.rs:41`), and `Dispatch::Lend` runs the string body at label `turn::component`.

**KM0. The option model (2-3 h; first, one owner, then the sources fan out).** Replace `Opt::Lent(Code)` by typed variants, one per source, over types that exist: `Relic(Relic)`, `Explore(Planet)` (trait exploration of a planet; `component|explore|<planet>`), `Tech(Technology)`, `Faction(ComponentAction)` (`kernel/hooks/sites.rs`: `{module, code, arg}`), `Mapped(ComponentAction)`, `Leader(Leader)`, `ActionCard(u8)` (`play|<index>`), `Transact(Seat)`, `Contact(Seat)` and `Payment(..)` (dropped with diplomacy; keep variants out), `Expedition` (Thunder's Edge: refused). Render both ways in `render/turn.rs` (the exact ids: `faction|<alias>|..`, `component|leader|<id>`, `component|tech|..`, `component|expedition|..`, `action_card|<idx>` vs the `PLAY_PREFIX`, the relic `ACTION_KIND`, `transactions::OPEN_KIND`). `Ord`/`Hash` stay (the codec and the hash). This file is the conflict hotspot of C3: KM0 lands before any source lane starts.

**KM1. Native option lists, one lane per group (hours estimated per source).** Each source = a core function `options(core, k, who) -> SmallList<Opt, N>` and a lockstep that it equals the string list (`menu_dual`'s `MenuParts` is the comparator: compare per source, then the assembled menu).

| Source | String | Core piece that exists | New | h |
|---|---|---|---|---|
| Strategic | `strategy::strategic_action_options` 1016 | `choice::turn::Opt::Strategic` | the bare/`|card` id rule, "pass when no unused card" | 1-2 |
| Tactical | `can_take_tactical` | `Opt::Tactical` | needs tactic token + galaxy (core `CoreGalaxy`) | 0.5 |
| Transactions | `transactions::available_actions` | `query::trade.rs:204` partners | the `may_transact` neighbour check from the core galaxy | 2-3 |
| Contacts, Payments | `diplomacy::candidates::*` | none | dropped (empty when `diplomacy.enabled` is false) | 0 |
| Relics | `relics::available_actions` 1597 | `query/relic.rs:142` | `ACTION_KIND` ids | 2-3 |
| Exploration | `exploration::available_actions` 620 | `query/explore.rs` | the per-planet option rule | 3-4 |
| Technology | `technology::component_actions` 131 | `technology_windows` | which technologies have a component action (VERIFY list: Psychoarchaeology? Bio-Stims? Aetherstream?) | 3-5 |
| Thunder's Edge | `thunders_edge::available_actions` 77 | none | refused content: skip | 0 |
| Faction | `faction_abilities::component_actions` 523 | `hooks.component_actions` (concat) | the text/code registry per module (`ComponentAction.code`) | 4-8 |
| Mapped | `factions::mapped_component_actions` 559 | `hooks.mapped_component_actions` | as above | in Faction |
| Leaders | `leaders::component_actions` 858 | `rules/leaders.rs::action_candidates` (needs `Env`, KF3) | none after KF3 | 2-3 |
| ActionCards | `action_cards::available_actions` | `rules/action_cards/mod.rs:225` | none | 1-2 |

**KM2. Dispatch of the chosen option (`apply_choice` Phase::Action, 3083-3670), one function per source (hours estimated).**

| Option | String arm | Core window | New | h |
|---|---|---|---|---|
| pass | `mark_passed`, `resolve_legendary_pass`, Prove Endurance occurrence, event scoring | `dispatch.rs::{mark_passed,last_to_pass}`, `Legendary` | compose; open EventScoring (KS1) | 2-3 |
| tactical | `begin_tactical` | other agent | the entry call only | 0.5 |
| strategic | see KX3 | primaries exist | KX3 | in KX3 |
| `faction\|..` | `play_faction_action` -> `faction_abilities::perform_component` | `faction_component.rs`, `muaat_component.rs`, `hero_windows`, `leader_production` | a `perform_component` router over `hooks.perform_component` (`any`), `COMPONENT_ACTION_RESOLVED`, failure -> `failed_component_actions` (KF4) | 5-10 |
| `component\|leader\|..` | `leaders::use_leader` (28 lent sites) | `leader_windows` (9 agents), `hero_windows` (6), the others | the router; the 3 string-only heroes (`empyreanhero`, `yinhero`, `mahacthero`) are lent bodies to PORT (lent-bodies W10 follow-up) | 8-16 |
| `component\|tech\|..` | `technology::perform_component` | `technology_windows` | router | 2-4 |
| relic / explore | `relics::perform`, `exploration::explore_*` | `relic_windows`, `explore_windows` | router; Crown/DET explorations lent in `close_tactical` (D1b2) are KM-adjacent | 4-8 |
| `action_card\|i` | `action_cards::play` | `rules::action_cards::effects::Play` | `ActionCardPlayed` announce already a window; the 27 card bodies are the card agent's track | 2-4 |
| transaction open | `transactions::open` | other agent (I5) | the entry | 0.5 |
| `END_TURN` | `advance_turn` | KP2 | - | in KP2 |

**KM3. Closing menu, free options, failed actions (3-5 h).** `closing_options` (2834), `is_free_option` (2815; "a leader printed `During the action phase:`", a contact, a trade, a payment on a deal, an expedition), `driver::menu::closing_menu/second_action` exist; `Entry.free/pass` flags per option now computed by the core. The `turn_closing` path in `apply_choice` (a non-free option consumes the second action: `clear_action_negotiations`). Tests: `menu_dual` already compares; make the native menu the subject.

**KM4. Display details (`with_turn_menu`, 2840-2900) (4-6 h).** `Choice.details` (`action_card_effects`, `kind: turn_menu`, `closing`, `actions_taken`, the per-card `strategy_cards` list with the option id, the per-partner `partners` list: `seat`, `faction`, `available`, `in_contact`, `reason` strings, goods, commodities, `promissory_notes`) is read by the web client and says in comments "never read by the engine". It must be rebuilt byte-exact in the wrapper's `complete` hook from `CoreState` (`state.transacted_with`, `may_transact`, `cultural_planets_preview` for Economic Initiative). Not rules, but a regression in `web/` if wrong. VERIFY that `oracle.rs::compare_choice` compares `details` JSON (if not, extend: a missed display field would pass every gate and break the UI). Question 4.

**Lanes for KM:** F1 = KM0 then KM3, KM4, pass, strategic/tactical entries, KX3; F2 = leaders, factions (component/mapped), technology, legendary/promissory; F3 = relics, exploration, action cards, transactions entry. F2 depends on KF3.

---

## 8. Host/server call sites, the `driver_for` sites and how D2-8b..8g fit

**Production callers** (finding table in 1): all keep `Game::step()`/`legal_options()`/`Table`. What changes for them is behind the wrapper: no edit in `ti4-server`, `ti4-wasm`, `ti4-host` is part of this document, except the product entry `product_init()` of D2-8c and
the planning/fork API (`prepare_hypothetical_*`, `fork`, `open_windows`, `GameSnapshot`) which D2-8d reimplements over `CoreState` (KX1 supplies the preview window).

**`driver_for` sites by lane** (99 engine sites; "convert" = the adapter's live constructor builds with `driver_live` and `arm`s, the replay closure leaves the production path):

| Lane | Files (sites) |
|---|---|
| C1/KS/KV | `objectives_write` 3, `agenda_close_dual` 2, `agenda_reveal_dual` 1 |
| KX | `strategy_secondary_dual` 1, `strategy_cards_core` 5, `strategy_core` 1 |
| KP (C2) | `technology_dual` 6, `promissory_dual` 1 |
| KM (C3) | `leaders_core` 11, `hero_dual` 6, `faction_dual` 7, `explore_write` 2, `action_cards_view` 2, `legendary_core` 1 |
| other (machines) | `dual_window.rs` 34 (mostly combat/invasion/production plus the shared `drive_windowed`: KF2 owns the loop part), `production_window_dual` 5, `cannon_dual` 5, `tactical_dual` 4, `trade_window_dual` 1 |
| dropped | `diplomacy_window_dual` 1 (refused feature: delete with the feature) |

**How D2-8b..8g (D2-remaining 3.4) fit:**

| Package | Composition content |
|---|---|
| 8a (done in `core-d2-l1`) | `driver_for`, `armed()`; KF6 makes the live path armed; the armed 20-game dual stays END-OF-PHASE ONLY and is run once when KF2 and the first lane are in |
| 8b wrapper crate content | `live/` moves with `render`, `core_sync`, `reverse_lend` (copy). The KM4/KF2 `complete` hooks are the render layer's |
| 8c entry and strict | `Lend` in the product = `Err(Unmapped{label})`; `TI4_CORE_STRICT=1` already turns a counted bail into a panic; `unsupported::Policy::Panic` and the P1 creation refusal make arms 7 and 9 unreachable |
| 8d state, checkpoint | each lane deletes its `Game` fields; `GameSnapshot` shrinks; `open_windows()` reads the stack; `failed_component_actions` (KF4) |
| 8e flip the windows | KP/KS/KV/KX/KM are exactly the 8e family packages of this document's islands; order in section 10 |
| 8f reconcile hooks | KP0 |
| 8g tests | sections 2.4, 9 |

Rule for string files: **all new logic goes into `ti4-engine-core` and into new files of the engine crate (`live/*`, `driver` adapters); the string modules get no rule-body edit.** The only edits to string files are (a) pure extraction of a `*_string` body, (b) at 8e the call-site switch. This keeps D2-7's freeze rule ("no rule-body edits") and lets the lanes start before D2-7.

---

## 9. Tests and gates: cheap vs END-OF-PHASE ONLY

**Per item (cheap, runners, never inside the package):** the core unit tests of the new file and `mutants.sh` single-file on it; the arm lockstep(s) of the item; `cargo clippy -p ti4-engine-core -p ti4-engine -p ti4-engine-core-bridge` (baseline `clippy_engine_baseline.txt` may only shrink, `clippy_core.sh`); `check_layers.py core`, `lint_core.sh`; `d4_gate.sh 2 3`; `oracle --game 0 --arms <lane arms>` (one game); the lockstep suite once at package end. Standing rules, runner template and the workflow rule of `D2-prereqs.md` section 6 apply (theory writes TODOs, impl never blocks on checks, Haiku runners run builds and tests).

**Bail-as-failure whole-game playthrough (KF5):** `playthrough` fails on any `Lend`, `Unmapped`, `unsupported` panic, an unfinished game or a step limit. Three sizes:
* smoke (cheap, a runner, minutes): 1 faction x 1 game x 4 players, `--native <arms done so far>` with the scaffold lending the rest: reports the lent count; becomes a real pass only at KQ.
* per-lane (cheap): the lane's arms native, others lent, N=2 games: the lent count of ITS arms must be 0.
* **G-zero (END-OF-PHASE ONLY, once, only when a full zero-bail game is possible):** all arms native, scaffold removed: `playthrough` over every lobby faction (24) at 4 and 6 players, 2 seeds each (96 games) with `starting_tech: true`; the oracle (A legacy vs B native) over golden 20 + `golden-lent.json` + the new starting-tech set; the armed 20-game dual; `golden check --mask all=core|all=string`; `TI4_CORE_STRICT=1`; `zero_bail.py --enforce` over the all=dual reports; `alpha_roundtrip` on a copy; the four bench modes (`verify_benches.sh`). Not before: (a) KF, KP, KS, KV, KX, KM, KQ merged; (b) the five machine islands merged (companion document); (c) the lent-body gates of `INTEGRATION7.md` 10.3 either ported or removed from the product by decision (they are bails inside windows: Empyrean/Yin/Mahact action heroes, Warfare/Imperial inside Naaz/Winnu heroes, Muaat placement gates, Heart of Ixth in the combat opening...); (d) P1 (`disable-xf123`) merged.

Which gates are NOT cheap even for one arm: the armed dual (2.5x calls), the wide mask, the soak, the 20-game oracle, anything that builds `dual_run` in the release/gate profile.

---

## 10. Dependency order, lanes, ownership, estimates

```text
K0 (VERIFY list, read-only, 1-2 h)
 |
 +-- KF1, KF4 --> KF2 --> KF5 --> (arm switches usable by every lane)
 |      KF3 (parallel with KF2)      KF6 (parallel)
 |
 +-- KP0 -> KP1 -> KP2 ; KP3 ; KP4         (core side now; wiring after KF2)
 +-- KS1 -> KS2 -> KS3 -> KS4 -> KS5       (KS3 needs KF3 for status_tokens)
 +-- KV1 -> KV2 -> KV3                     (KV3 needs KS1 for the agenda event window)
 +-- KX1 -> KX2 ; KX3                      (KX3 with KM2)
 +-- KM0 -> KM1/KM2 (F1,F2,F3) ; KM3 ; KM4 (F2 needs KF3; strategic needs KX3; transactions entry needs the trade island)
 |
 +-- KQ: remove Lent/Lend, refusal, unreachable arms, G-zero          (needs everything above + companion doc)
```

| Lane | Branch (cut from the merged `core-integration-8` + `core-d2-l1`) | Owns (files) | Packages | h (est.) |
|---|---|---|---|---:|
| A | `core-d2-f-live` | `ti4-engine/src/{live/, window_driver.rs, dual_window.rs (drive part)}`, `ti4-engine-core/src/driver/{step.rs, session.rs, mod.rs}`, `rules/{leaders_env,strategy_env,abilities_env}.rs`, `core-bridge/src/{oracle.rs, bin/playthrough.rs}`, `state/pending.rs` (KF4) | KF1-KF6, then KQ | 25-40 + 10-15 |
| B | `core-d2-c2-turn` | `driver/{prelude.rs, phase.rs, turn_flow.rs, windows/turn_prelude.rs, windows/turn_end.rs}`, engine `turn_write.rs`, `technology_dual.rs`, `promissory_dual.rs`, `tools/rulebot/golden-starttech.json` | KP0-KP4 | 15-25 |
| C | `core-d2-c1-status` | `driver/windows/{scoring,status,status_bookkeeping,status_finish}.rs`, `rules/token_gain.rs`, engine `objectives_write.rs` | KS1-KS5 | 25-42 |
| D | `core-d2-c1-agenda` | `driver/windows/agenda_phase.rs`, `rules/agenda/{window.rs (restore), phase.rs}`, engine `agenda_{close,reveal}_dual.rs`, `vote_dual.rs` | KV1-KV3 | 16-27 |
| E | `core-d2-c1-strategic` | `rules/strategy_secondary.rs`, `strategy_windows.rs`, `strategy_primaries.rs` (entry points only), `state/pending.rs::Secondary` (preview field, with A's agreement), engine `strategy_secondary_dual.rs`, `strategy_cards_core.rs`, `strategy_core.rs` | KX1-KX3 | 13-22 |
| F1/F2/F3 | `core-d2-c3-menu`, `core-d2-c3-leaders`, `core-d2-c3-misc` | `choice/turn.rs`, `driver/{menu.rs, dispatch.rs}`, engine `render/turn.rs`, `menu_dual.rs`, `turn_write.rs` (F1 only with B's agreement); F2: `leaders_core.rs`, `hero_dual.rs`, `faction_dual.rs`, `rules/{leader_windows,hero_windows,faction_component}.rs`; F3: `explore_write.rs`, `relics.rs`, `action_cards_view.rs`, `rules/{relic_windows,explore_windows}.rs` | KM0-KM4 | 45-75 |

Shared files and who edits: `state/pending.rs` (KF4, KX1 `preview`; A owns, E asks), `choice/turn.rs` and `render/turn.rs` (F1 owns after KM0; no other lane touches), `driver/session.rs` + `step.rs` (A owns; lanes B-F request the arm they add via `NativeArms` bit constants only), `dual_window.rs` (A only).
Merging: one lane per merge, each rebased on the integration line; the integration branch is not the live branch, so the nightly sweep is unaffected; before D2-7 the whole set merges into the line and D2-7 follows (merge the live branch first: D2-remaining 3.3). Do not merge during the nightly window (20:30 Berlin; `TZ=Europe/Berlin date`).

**Totals (estimated):** A 35-55, B 15-25, C 25-42, D 16-27, E 13-22, F 45-75: **149-246 agent-hours**; critical path = A (KF2, KF5) then F1/F2 (about 35-45 h).

---

## 11. Traps

| # | Trap | Where | What to do |
|---|---|---|---|
| T1 | `dual_window!`'s `Core` backend runs the string (`dual.rs:1482`); `Backend::Core` in `build_default()` areas is NOT the core running | KF2, all | the live path is a new entry, not a mask; do not infer "native" from the mask |
| T2 | The replay driver bails on what a live game must handle (nested ability not in the table, dice from an unseeded driver, `withdraw`, `first_event_id`) | KF2 | each bail reason becomes either a core feature or a hard `Unmapped` with the label; never a silent skip |
| T3 | Nested asks inside one step and the decision journal order: the table is asked in the same order as the string (oracle `Decision`/`DecisionHash`); an extra or reordered ask breaks replay/undo in the server (`UndoTrack`) | all | assert the question sequence of a step against the string in the arm lockstep |
| T4 | `table.auto_resolve` of the lone strategy card is never journaled; single-option windows settle "without a decision" in several places (starting tech, `auto_resolve_single`) | KP4, KS | reproduce which answers are automatic, not only which are asked |
| T5 | `rules_version` branches in string bodies (81.3 hand limit `RULES_FACTION_BUGFIXES`, bribery per player `RULES_BRIBERY_PER_PLAYER`, Imperial secret cap 18, Munitions < 8): the old benches pin old versions | KS3, KV2 | port every `if state.rules_version` branch of the body, test at least one old version per branch (bench configs `golden*.json`) |
| T6 | Event double-logging: `emit` (plain) vs `emit_typed` (resolver, mirrored into `events`); comment in `step_phase`: "calling `emit` as well would log the phase twice and fire any card reading it twice" | KP3 | one emission per event through the queue; compare `Event` category in the lockstep |
| T7 | The `TAINTED`/`announced` watch of the end-of-turn tail is a dual device; in live mode a reaction window may change `active`/`turn_seq`/flags and the tail must continue on the NEW position | KP2 | re-read `meta` after each typed emit |
| T8 | `Facts` computed "after the reconcile hooks": the prelude changes the position before the arm is chosen (`blocked` can be set by `replay_map_edits`) | KP0, KF1 | build `Facts` after the prelude, in the same order as `step_flow` |
| T9 | Layering: `state` (L1) cannot hold a rules machine; a window that must survive between steps needs a plain payload plus `restore`/`to_pending`; a window that runs inside one step needs no payload | KS, KV | decide per window: persisted (Scoring, Tokens, Voting, Secondary, LeaderFollowers) vs in-step (Bookkeeping, Finish, TurnPrelude, AgendaPhase composer) |
| T10 | A rollback must also restore the plain events and the decision log of the step (the string truncates `events` and restores `secondary` in `step_secondary`) | KF2, KX1 | the clone covers `CoreState`; the wrapper restores its own event/decision lengths |
| T11 | Corpus blind spots: `starting_tech` off by default, planning forks, 8-seat tables (`EXTRA_SEAT_FACTIONS` sardakk/yin are "not fully ported"), TE strategy card set `te` (14 of 14 alpha games per `lent-bodies.md`) | KP4, KX1, G-zero | new corpus configs; question 3 |
| T12 | The `Env` implementations (KF3) must agree with `LeaderView`/`StratView` BEFORE the string engine is gone, or the oracle compares the core with itself | KF3 | the three-way lockstep |
| T13 | Display facts (`details`) are invisible to the rules and to the dual's hashes | KM4 | extend the oracle's choice compare before porting them |
| T14 | `Window::Untyped` carries `[u32;3]`, never a string window (D2-remaining T-M) | all | typed payload or in-step window only |
| T15 | Parallel lanes editing `state/pending.rs`, `choice/turn.rs`, `render/turn.rs`, `dual_window.rs` | all | ownership table above; one lane per hot file |
| T16 | `core-d2-l1` is behind the int-8 tip (eod1-d, d4a4, zero-bail) | start | first action: merge `core-integration-8` into the lane base |

---

## 12. Open questions for you

1. **Is 150-245 agent-hours for the composition (about 35-55 h wall with 6 lanes) acceptable?** It is 4-6x the audit's figure for all of 8e, because the audit prices lending/holding islands and not porting the status machine, the agenda loop and the 13 menu sources. *Recommend:* yes; the alternative that stays under the audit's number is B (embedded legacy `Game`), which your rule forbids as an end state.
2. **Starting technology (Winnu, Argent Flight; `starting_tech: true` in the server) is an in-game question** asked at the first steps, in no golden game. Native Ask arm (KP4, about 3 h plus a corpus config), or resolve the picks before `load` (D2-remaining 8c says "stays string")? *Recommend:* native; resolving before `load` would need a second question channel before the game exists.
3. **Does "except Thunder's Edge" include the `te` strategy card set** (`strategy_card_set = "te"` in all 14 alpha games per `lent-bodies.md`; the engine has `TeWarfare` and `te4_construction`/`te6_warfare` lent bodies)? If the card set is in the product, those bodies must be ported for zero-bail; if the content flag only (`thunders_edge`), they stay out. *Recommend:* decide before KX3; it moves 6-10 h in or out.
4. **May the oracle's choice compare be widened to the display `details` JSON (KM4) and the wrapper own `with_turn_menu`?** Without it a wrong partner list or card state passes every gate and breaks `web/`. *Recommend:* yes.
5. **Planning forks (`prepare_hypothetical_turn/_secondary`, `ti4-host/planning`) are in the zero-bail scope?** They are a product path (needs `Secondary.preview`, KX1). Memory says the upstream plan-your-turn branch is not ready; the engine API exists and is used by the host. *Recommend:* yes for the API (cheap), no new planning features.
6. **Scaffold B in the bridge only:** may the `NativeSubject` of KF5 lend non-native arms to an embedded legacy `Game` as a TEST harness (never in `ti4-engine`/server/wasm builds), so that each lane can be proven alone before the others land? *Recommend:* yes; it is what makes parallel lanes testable, and KQ deletes the lend path.
7. **Table callback model stays until D3** (the wrapper keeps `Table`/deciders and `step()` blocks in the table; the core is driven by the `Asker`)? *Recommend:* yes; a suspend/resume public API is a D3 topic and would touch server, wasm and the bots.

---

## 13. K0: the 10-minute checks the first agent runs (read-only)

1. Merge check: `git merge-base --is-ancestor core-integration-8 <lane base>`; take `zero_bail.py`.
2. Is `step::step` still without callers? `grep -rn "step::step\|step::Needs" crates --include=*.rs | grep -v "driver/step.rs"`.
3. Inventory of adapter-supplied facts for KF2: `grep -n "Facts {\|Recording\b.*\.\(withdraw\|first_event_id\)" crates/ti4-engine/src/*.rs`.
4. `Env` methods without a core source (KF3): list for `leaders::Env`, `strategy::Env`, `abilities::Env`.
5. `Vote::restore`/`to_pending` absent? `grep -n "state::Voting\|fn restore\|fn to_pending" crates/ti4-engine-core/src/rules/agenda/window.rs`.
6. `space_stations::reconcile_all`, `coexistence::reconcile_all`, `entropic_scars::resolve_status_start`: no-ops on a non-TE state? (read the early returns).
7. `laws::revolution_levy`, `laws::draws_at_status_end`, `laws::agenda_ready_limit`, `strategy::strategic_action_options`: core equivalents (`rg` in `query/laws.rs`, `rules/`).
8. Which technologies offer a component action (`technology::component_actions`, `technology.rs:131`)?
9. Does `oracle.rs` compare `Choice.details`? (`compare_choice` in `dual.rs`).
10. `golden-lent.json` and the golden configs: any with `starting_tech: true` or the `te` card set? (`grep -l starting_tech tools/rulebot/*.json`).
