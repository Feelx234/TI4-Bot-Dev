# Lane N (`core-n-waiver`): Jae Mir Kan's waiver with a Nomad at the table, the close order of the follower window, War Machine in a research's pool -- resume note (2026-11-01)

Branch `core-n-waiver`, cut from `core-integration-14` (`4766eafcc`), with `core-k-actioncards` merged (`319602fdd`: without it every sweep stops at `Menu(ActionCard)` first and the strategic island is never reached).
Never pushed; the live branch `nightly-fixes-2026-10-26` is untouched. Pure refactor: `RULES_VERSION`, serde, fixtures and fingerprints unchanged (the quick game's fingerprint is the same as lane K's, `2e93df9fa00fa982`).

## 1. The stop

`TurnGap::StrategyAbility` in the follower window (`secondary`): a follower chose the Mahact agent's waiver `follow|waived|<i>|agent|<system>` (Jae Mir Kan) while a Nomad sat at the table. `driver/windows/strategic/facts.rs::waivers_of` marked EVERY Mahact waiver
`WaiverEffect::Unported` when any Nomad was seated ("heard by the Nomad's suite"), and `SecondaryWin::answer` turned a taken `Unported` waiver into the counted gap. Lane K measured it as the first gap of 176 of 2,304 games once Z'eu was skipped.

| Cause | What was wrong | Fix | Regression tests (all fail on the pre-fix code) |
|---|---|---|---|
| N1 the waiver | the gate was "a Nomad is seated" -- but the suite listens only when the Nomad OWNS Temporal Command Suite (`direct.nomad_watches_agent_exhaustion`), and the payment is already the core's: `pay_waiver` -> `fx_strategy::exhaust_ready_agent` -> `exhaust_agent` stages `AGENT_EXHAUSTED` when the hook says so. The gate only stopped the game | `waivers_of` maps every offered waiver through `waiver_effect` (the `heard` gate is gone); doc line of `strategy_secondary.rs` | core `strategic_arms::{a_waiver_a_nomad_hears_is_paid_and_stages_the_exhaustion_for_the_suite, a_waiver_without_a_nomad_holding_the_suite_stages_no_exhaustion}` (replace lane E's pin `a_waiver_a_nomad_would_hear_is_offered_and_stops_the_step_when_it_is_taken`); engine lockstep (string against core) `strategic_live_tests::{jae_mir_kan_while_the_nomad_holds_the_suite_runs_like_the_string_and_tells_the_suite, jae_mir_kan_with_a_nomad_that_lacks_the_suite_stages_nothing_like_the_string}` |
| N2 the Nomad's question comes before `STRATEGIC_ACTION_COMPLETE` | found by the shadow once N1 let the game go on: **the "close order" difference lanes E/int. 13/14 left as known (27 games).** When the followers left after the last answer are all skipped, the string ends the action in a step of its own, which starts with the reconcile (staged `STRATEGY_TOKEN_SPENT`, the exhaustion heard by the suite -- which may ASK the Nomad -- gains). The core followed, skipped and completed in the answer's step, so `STRATEGIC_ACTION_COMPLETE` (and the turn's end) came BEFORE the staged events and the suite's question. 10 of 12 Mahact + Nomad games diverged there | `SecondaryWin::step`: when `skip_ineligible` skipped followers right after an answer (not a window that opens with nobody askable), `prelude::run_reconcile` runs before the action completes | core `strategic_arms::what_the_last_answer_staged_is_announced_before_the_action_completes_when_the_rest_are_skipped`; lockstep `strategic_live_tests::{the_token_a_follower_spent_is_announced_before_the_action_completes_when_the_rest_are_skipped_like_the_string, jae_mir_kan_as_the_last_follower_asks_the_suite_before_the_action_completes_like_the_string}` |
| N3 mutation survivor | `answer` stops on a taken `Unported` waiver: `w.opt == opt` -> `!=` survived (a ported waiver taken beside an unported one) | test only | core `strategic_arms::a_ported_waiver_is_taken_although_another_offered_one_is_not_carried` (Winnu Acquiescence beside an Obsidian Enervate) |
| N4 War Machine in the research's pool | found by the first sweep past the waiver: `--factions sol --players 4 --first-seed 18` (sol, empyrean, mahact, naaz), first gap `secondary StrategyAbility` at step 887. The Technology secondary was OFFERED to a seat whose only pool was War Machine's bonus (5, for the activation of the card): `production::available` counts it (right for the units a production buys), the string's gate for research is `payment::affordable` (planets and trade goods, no bonus). The core asked "spend a strategy token and 4 resources to research", the bill then had no option (the bonus is no face), `PayStep` ended with nothing taken, and `Research::after_step` marked the window `failed` = the counted bail. The string never asks that seat. Instrumented runs (Haiku runner, not committed: `RESEARCH-FAILED` sites, `PAY-DEBUG` of the pool and the options) located it: `pay_opts` empty, `available` 5 with the only planet exhausted and no trade goods | `KernelEnv::technology_secondary_affordable` and `Research::after_offers` subtract `production::war_machine_bonus` | core `strategic_arms::war_machines_bonus_does_not_make_the_technology_secondary_affordable`, `research_e2::war_machines_bonus_is_no_way_to_pay_a_research`; engine lockstep `strategic_live_tests::a_technology_secondary_the_pool_of_war_machine_alone_affords_runs_like_the_string` (the string asks the next follower, the core asked this one: `Illegal`) |

The typed `Ask`/`Opt` (`SOpt::WaiveBy(module, code)`) and the byte-exact renderer of the offer already existed (lane KX); nothing new was needed there. The lockstep (string vs core) compares the questions as rendered, the events, the position, the dice streams and the staged events, including the
suite's question `after AGENT_EXHAUSTED` (answer `decline`). The dual-side test `strategy_secondary_dual_tests::jae_mir_kan_while_the_nomad_watches_is_paid_by_the_core_step_and_the_nomad_is_told` was already green (the dual's adapter paid it).

## 2. Pre-fix checks (the new tests guard the fixes)

* revert of N2 only: 1 core + 2 engine tests fail (`["complete"]` instead of `["exhausted","complete"]`; events lack `STRATEGY_TOKEN_SPENT` before `STRATEGIC_ACTION_COMPLETE`; the suite's answer is never taken);
* revert of N1 only (`facts.rs` of `4766eafcc`): 3 core + 3 engine tests fail (`Unknown(TurnGap(StrategyAbility))`), including the Nomad-without-the-suite case (the old gate was wider than the string).

## 3. First-gap matrix

`native_play.sh --factions all --players 3,4,5,6 --first-seed 13 --seeds 3` (288 games, gate profile, `te` set, one run at a time), before and after the merge of lane K:

| tip | finished | first gaps |
|---|---|---|
| `4766eafcc` + N1 (no K) | 69 | 109 aftermath `MissingFact`, 95 `Menu(ActionCard)` (77 menu, 18 secondary), 11 `Menu(Unrouted(5, Z'eu))`, 2 `CombatUnported`, 1 `IntegratedEconomy`, 1 draft `MissingFact` |
| with K (`319602fdd`) | **139** | 126 `MissingFact` (107 aftermath, 16 menu, 1 draft, 1 tokens, 1 tactical), 16 `Menu(Unrouted(5, Z'eu))`, 6 `CombatUnported`, 1 `IntegratedEconomy` |

Island matrix (with K): trade, strategic action (**23,370 native steps, 0 gaps**), production, invasion, agenda green; combat 6, aftermath 108, tactical 1, turn flow 17, status 1, menu 16 -- all lane L's classes. **No game stops at the strategic island any more**; `StrategyAbility` does not occur as a first gap in this sweep (Menu(ActionCard) and the faction MissingFacts hide it behind them -- K's 176 were found with a Z'eu skip hack that is not committed, and the `MissingFact` stops of lane L still hide every later gap of ~44% of the games).
Final-tip sweeps (this note's tip): see section 5.

`--quick`: finished, zero gaps, 9 rounds, 1,174 steps.

### The Mahact + Nomad games

108 of 600 Mahact tables (`--first-seed 1 --seeds 150`, 3-6 players) seat a Nomad; the first 12 (all 3 players: seeds 19 20 21 43 44 45 67 68 69 91 92 93) all FINISH natively with zero gaps after N1+N2 (700-840 steps).
`--shadow` of the 12 (TRACE on): before N2 every one diverged at a strategic action (`native STRATEGIC_ACTION_COMPLETE | legacy STRATEGY_TOKEN_SPENT`); after N2 none does, and the shadow compares 38-387 steps before the first difference, which is always one of the known classes of other lanes:

| class | games | note |
|---|---|---|
| `native STATUS_PHASE_BEGAN \| legacy <end after 0>` (the phase glue, Mitosis) | seeds 19 20 21 68 | known (int. 14 section 8) |
| `native ROUND_BEGAN / TRADE_GOODS_GAINED \| legacy <end after N>` at `tokens+phase` / `ask.draft+phase` | seeds 44 45 69 91 93 | the same phase glue (the legacy game ends its step list first) |
| `native PRODUCTION_RESOLVED \| legacy UNIT_ABILITY_ROLLED` | seeds 43 92 | the known production order |
| `native INVASION_RESOLVED \| legacy ACTION_CARD_PLAYED` (`ACTION_CARD_PLAYED`, `ACTION_CARD_DISCARDED` missing natively after the Naaz reaction `reaction:naaz:INVASION_BEGAN:after`) | seed 67 | **NEW, not in the known list, not the strategic island**: the native Naaz invasion-begin reaction plays/discards an action card without announcing it; belongs to the reactions/Naaz lane (replay: `native_play.sh --factions mahact --players 3 --first-seed 67 --seeds 1 --shadow`, TRACE on) |

## 4. Checks (cheap; private target `/root/deploytmp/core-n`, `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, profile `lockstep`; Haiku runners on committed snapshots in a scratch worktree; the target is deleted at the end)

| Check | Result |
|---|---|
| core `strategic_arms` | 38 tests, all green; `research_e2` 5; `research`, `research2`, `research_sucaban`, `research_waiver` green |
| core `strategy_secondary`, `driver_session`, `action_dispatch` (68), `tactical_composer`, `action_card_effects3` | all green |
| engine `--lib` filters `strategic_live_tests strategy_research strategy_secondary_dual` | 70 passed; `... turn_live_tests action_dispatch_live trade_live_tests` 128 passed before N4 |
| `check_layers.py core`, `lint_core.sh`, `clippy_core.sh` | 574 files 0 violations; 576 files 0 hits; clippy ok (engine warnings 41 = baseline) |
| mutation (single files; union of core `strategic_arms` and engine `strategic_live_tests strategy_secondary_dual`; `D4_COVERS_FILE=/dev/null`) | `strategic/mod.rs` lines 138-175 (`SecondaryWin::step` with the new close, `ask_of`, `answer`): 18 points, 1 not compilable, 17 live: 16 killed by each half, the survivor `w.opt == opt` in `answer` is N3 (killed by the new test, see section 5); `strategic/facts.rs` `waivers_of`, `waiver_effect`: 5 points, 1 not compilable, 4 killed by the union (core half 4/4, engine half 1/4) |

Not run (user rule): golden / wide dual, `AREAS=all=dual` suites (the lockstep tests above compare the string and the core directly; the dual-adapter test of the same waiver was green), benches, soak, census, workspace clippy beyond `clippy_core.sh`, the ratchet, Playwright.

## 5. Sweeps at the final production code (gate profile, one run at a time, no `--shadow`)

| sweep | finished | first gaps | strategic island |
|---|---|---|---|
| `--quick` (sol 4p seed 1) | **zero gaps**, 9 rounds, 1,083 steps, fingerprint `74d5675ac754ac81`, `--twice` deterministic | -- | green |
| `--first-seed 13 --seeds 3` (before N4) | 139 / 288 | 126 `MissingFact`, 16 Z'eu, 6 `CombatUnported`, 1 `IntegratedEconomy` | 23,370 native steps, 0 gaps |
| `--first-seed 16 --seeds 3` (before N4) | 100 / 288 | one game outside lane L's classes: `sol 4p seed 18`, `secondary TurnGap(StrategyAbility)` = N4 | 1 `StrategyAbility` (N4) + 3 `MissingFact` |
| `--first-seed 16 --seeds 3` (after N4) | **101 / 288** | 43+25+... `MissingFact` (aftermath 43, tactical 25, menu 85, phase 7, reconcile 5, tokens 4, draft 1, secondary 3), 13 Z'eu, 1 `IntegratedEconomy` | 21,705 native steps, only 3 `MissingFact` (lane L) |
| `--first-seed 19 --seeds 3` (after N4) | **249 / 288** | 8 menu + 7 aftermath + 6 tactical `MissingFact`, 12 Z'eu, 2 `CombatUnported`, 1 Peace Accords, 3 `Unported(Exploration(Freelancers))` (reconcile 1, secondary 2) | 30,187 native steps, 2 Freelancers |

The quick game moved from 1,174 steps (fingerprint `2e93df9fa00fa982`, lane K's) to 1,083 steps: N4 removes a Technology-secondary question the string never asks, so the bot's game is a different one -- the native game now asks what the string asks. (Nothing the engine stores or fingerprints for decisions changed.)

First-gap matrix of this lane (all seeds above): **no game stops at `StrategyAbility`** and no game stops at a strategic-island cause that is not lane L's (`MissingFact`, Z'eu, `CombatUnported`, `IntegratedEconomy`, Freelancers, Peace Accords). The three `secondary MissingFact` of the seeds 16-18 sweep (letnev 4p seed 18, saar 4p seed 16, argent 5p seed 17) are lane L's faction `MissingFact`s (Argent/Empyrean/Mentak providers), not investigated further.

## 6. What remains

* Lane L's classes stop every game that has them (`MissingFact` 126 of 288, Z'eu 16, `CombatUnported` 6, `IntegratedEconomy` 1): the strategic island cannot be probed past them. Once they are gone, rerun a wide first-gap sweep: the 176/2,304 `StrategyAbility` of lane K were Mahact-waiver ones, but other strategic-island gaps (Doctor Sucaban, Mindsieve -- a Thunder's Edge breakthrough, out of scope) may hide behind them.
* The Naaz invasion-begin reaction's missing `ACTION_CARD_PLAYED` / `ACTION_CARD_DISCARDED` (table in section 3, seed 67).
* The shadow classes of other lanes (phase glue/Mitosis, the production order), unchanged.
* The Ssruu / borrowed-agent variant of the waiver (`exhaust_ready_agent` is used for the Mahact agent held by its own seat only) was not exercised.
