# D2 lane E (`core-d2-c1-strategic`): the strategic action, resume note (2026-10-31)

Branch `core-d2-c1-strategic`, cut from `core-d2-c1-status` (`bdb6aae9`) and merged with `core-w-l14` (see "Merge" below). Read `docs/KS_RESUME.md`, `docs/KP_RESUME.md`,
`docs/D2_LF_RESUME.md` and `docs/D2_LF2_RESUME.md` first (the recipe, `NativeSubject`, the jobs of the turn flow). Never pushed; `nightly-fixes-2026-10-26` untouched.
Packages KX1-KX3 of `docs/todo/D2-islands-composition.md` (section 6.2).

## Merge: why `core-w-l14` is in this branch

`core-d2-c1-status` does not hold the Thunder's Edge Warfare / Construction primaries as children of the card windows, the home production as a child
(`Spawn::HomeAt`), Scheming and Crafty in `DrawCards`, nor `Report::{Free, Produced}`: `core-w-l14` (with lanes 9, 10 and 13) does. Merging
`core-integration-10` instead would have meant resolving lane C's `driver/windows/scoring.rs` against M5's (two different `EventScoring` windows): that is the
integration agent's. The l14 merge had 8 conflicts, all of the "both sides add" kind; resolved by keeping both, except:

* `choice::cards::Reason`: lane C's `SchemingDiscard` and l14's `Scheming` named one question; lane C's variant is gone (`Reason::Scheming` everywhere).
* l14's `Driver::new` became `window_driver::driver_for` (the KF6 constructor); `settle_run` keeps `tables_of_sides` and no neutral bail (integration-9 B1).
* Not merged: `core-w-l12` (Titans' Hecatoncheires in Construction, legacy rules versions), `core-m2-production`, `core-m6*`.

## What is native

Three arms and the strategic dispatch, all typed, no string state (`driver/windows/strategic/`):

| piece | arm / job | string | where |
|---|---|---|---|
| KX1 follower window | `Arm::Secondary` (`SecondaryWin`, HELD, `WindowKind::StrategySecondary`) | `step_secondary`, `secondary_step`, `follow` | `strategic/mod.rs` |
| KX1 preview | `state::Secondary.preview` (new field), `SECONDARY_PREVIEW_COMPLETE`, no `Finish` | `StrategySecondaryWindow::preview` | same |
| KX2 hero followers | `Arm::LeaderFollowers` (`LeaderFollowersWin`, HELD); `select_leader_strategy_followers`, `complete_leader_strategy` | `step_leader_followers` | same; new question `Ask::Leaders(WinnuFollower)`, `Opt::HeroFollower`, `Subtype::HeroFollower` |
| game over during an action | `Arm::Finished` (`close_finished`) | the `Finished` arm of `step_flow` | same |
| `StrategyAnnounce` | already native (lane B, `TurnJob::Announce`) | | |
| KX3 dispatch | `TurnJob::Strategic(card)` (`begin::begin_strategic(core, env, card)` is what the menu calls) | the `Strategic` option of `apply_choice` | `strategic/begin.rs` |
| end of the free tactical action | `TurnJob::AfterTactical` (`after::open_after(core)`) | `close_tactical`'s hero / `secondary_after_tactical` branches | `strategic/after.rs` |
| the abilities | `perform::run` (child windows by `fx_perform::begin`, questions to the tape), `perform::resolved` | `strategy_cards::{primary, secondary, follow}`, `resolve_faction_strategy` | `strategic/perform.rs` |
| the facts | `facts::{follower_facts, perform_facts}` from the CORE (hooks, `KernelEnv`, the production rules) | `core_facts`, `hero_dual::perform_facts` | `strategic/facts.rs` |

All eight standard cards and the Thunder's Edge Construction run natively as primary and secondary; the Winnu hero's selection and the follower window of a card nobody
holds; a follower whose faction substitutes the primary (Jol-Nar's Brilliant); Hacan's Masters of Trade (a follow without a token: `KernelEnv` now reads the printed
abilities as the string does); Acquiescence and Jae Mir Kan as waivers (`Opt::WaiveBy(module, code)` from the kernel's `secondary_waivers`; the string module's own are still
`Opt::Waive(shape)` for the dual); `STRATEGIC_ACTION_BEGAN` with Coup d'Etat's cancellation; Extreme Duress lifting; the Imperial primary's score (heroes unlock, the winner);
the Trade primary's goods, the Diplomacy primary's tokens, the Warfare secondary's `UNITS_PRODUCED` (all staged for the next step's reconcile, as the string stages them).

One call is one string step: the call that applies the last follower's answer also ends the window, says `STRATEGIC_ACTION_COMPLETE` and runs lane B's `Finish` job
(the string's `finish_action`), like lane C's close. A follower's ability asks through the tape, so a caller that returns between two questions, a clone of the position
mid-ability and a fresh driver continue identically (`Pending::nested`).

Engine half: `live/strategic.rs` (display facts of the follower question: `tokens_left`, Leadership's `purchase` and pools, Diplomacy's `ready_hints`, Trade's `commodities`,
Politics' seats and agenda, Trade's seats, the technology names), `live/turn.rs` (`TurnCodes` grew the texts of the strategic action), `render/{strategy,leaders}.rs`
(the waiver ids carry the position of the module among the string modules with the hook), `live/mod.rs::render_asking` renders every question with the position (the
production questions of the Warfare secondary need it; `live::kernel::static_of`), `live/details.rs` (kernel in the signature, `pool_details` for `buy` and the Warfare
redistributions), `state::List::Produced` + `event_of` (identical to M2's addition).

## Gaps (counted, `Unknown::TurnGap(..)`), never silent

| gap | when | who closes it |
|---|---|---|
| `FreeTactical` | Thunder's Edge Warfare's primary (a system to activate without a token) | lane M6: in `begin.rs` call `tactical::open_free(core, active, system)` (M6c has it) instead of the gap, and where the tactical tail says `Then::LeaderStrategy` / `Then::Secondary` call `strategic::after::open_after(core)` |
| `PeaceAccords` | Xxcha follows or plays Diplomacy and a planet could be annexed | a window for Peace Accords (annex, Integrated Economy, legendary control gain, explore): not ported |
| `StrategyAbility` | an ability that cannot begin (no unit roster in the table's play facts, a research below `RULES_TECH_PAYMENT_CHOICE`, Minister of Industry's dock, a Mahact waiver with a Nomad at the table, Mindsieve) | n/a (content out of scope or the Nomad's suite) |
| `NoLink` | the table has no link to the window of an ability | defect of the env |
| `StrategicCancelled`, `ForeignWindow`, `ProductionAnnounce` | declared, unused | |

Also reached from this lane but owned elsewhere: `MenuSources` (the closing menu after the action, lane F), `Propagation` (Nekro, lane B's reconcile), `CommanderUnlock`.

## Departures and findings

* Departures by decision: no Firmament Enervate (`primary_instead` is false), no Keleres agent term, Mindsieve (a Naalu Thunder's Edge breakthrough) is refused.
* FINDING: the Xxcha's Peace Accords are string-only (`faction_abilities::strategy_resolved`); the core had no window for them.
* FINDING (found by the lockstep): `KernelEnv::{secondary_is_free, substitutes_primary}` did not read the printed abilities Masters of Trade and Brilliant (the string's `StratView` did). Fixed with `rules::abilities`.
* FINDING (found by the shadow behind a real game, a REAL DEFECT of lane B's closing menu): `rules::trade::KernelEnv::{wide, trades_action_cards}` read the hook table only, not Hacan's printed Guild Ships and Arbiters:
  after a strategic action the string offers `component|trade|hacan` (TURN_CLOSING) and the core ended the turn (TURN_PASSED). Fixed (printed abilities read like `rules::abilities` does), regression test
  `hacans_printed_abilities_are_read_by_the_strategy_and_trade_envs`. Two core-tests fixtures of lane B (`turn_jobs2`) assumed the wrong behaviour and use one seat now.
* FINDING (shadow): the string asks "Doctor Sucaban: exhaust to let a trade infantry for research" whenever the Jol-Nar agent is readied at the table and the researcher has infantry on the board; the core `Research` window
  does not ask it. `facts::research_asks_more` makes the ability `Unsupported` (counted gap `StrategyAbility`) instead of researching without the offer.
* FINDING (merge): after the l14 merge `set_table_codes` had lost `table.codes.indoctrination` (the Yin hero lockstep failed); restored.
* Latest rules version only (coordinator decision): no version gate in `strategic/`; the string's gates (`RULES_TECH_PAYMENT_CHOICE`, `RULES_FACTION_BUGFIXES`, `RULES_WINNU_AGENT_PRODUCTION`, `RULES_LETNEV_FLEET_POOL`) are taken at their current value.
* The observer `on_strategic_action_chosen` (`game.rs`) is the wrapper's: call it with the result of `begin_strategic`.
* The wrapper's `Render` must carry the faction texts (`Render::with_factions`): Politics' and Trade's options name seats by faction.

## Checks (cheap), `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, private target dir `/root/deploytmp/core-e` (deleted at the end)

* `cargo test -p ti4-engine --lib --profile lockstep`: 5002 passed, 4 ignored; also `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`: 5002 passed (whole lib, once at the end, after the last code change).
* Lockstep against the string (`ti4-engine/src/strategic_live_tests.rs`, 25 scenarios): the follower window of all eight standard cards and the Thunder's Edge Construction (the question as rendered with its display facts, the events, the position, the dice streams,
  the events left staged); the primary of the same cards through `TurnJob::Strategic`; nobody eligible; the Winnu hero's selection and window; a preview window; the end of the free tactical action; Thunder's Edge Warfare's primary stops at the counted gap;
  bail-as-failure over a native strategic action of EVERY lobby faction (24) with every standard card and the Thunder's Edge Construction (the Nekro `Propagation`, the closing menu's `MenuSources` and Doctor Sucaban are the only tolerated stops).
* `ti4-engine-core-tests`: `strategic_arms` 34 tests (clone mid-window, mid-ability with `Pending::nested`, illegal answers, the payload, waivers, the hero, the dispatch, the end of the free action, the findings above); the whole crate: all binaries ok
  after two fixtures followed the Hacan/Yssaril hooks (`turn_jobs2`, `promissory_windows`) and `combat_fight` got the `show_neutral` field of the l11 merge. `-p ti4-engine-core --lib`: 467 passed. `-p ti4-engine-core-factions`: 126 passed.
* `-p ti4-engine-core-bridge` (lib + integration): all ok, incl. `native_subject` (14): the strategic arms in shadow behind a legacy game through round one, and with each of the 24 lobby factions in six tables with a counted bail a failure: zero differences except the
  counted gaps `MenuSources`, `CommanderUnlock`, `PeaceAccords`, `StrategyAbility` (5 occurrences), and the follower window ran at every table.
* `check_layers.py core`: 470 files, 0 violations. `lint_core.sh`: 472 files, 0 hits.
* Mutation (`mutants.sh` on a committed snapshot in a scratch worktree, `strategic/` as one target, two test sets, union): `strategic/{mod,facts,perform,begin,after}.rs`: 173 mutants (the skips with reasons excluded), 163 killed (94 %).
  Core tests alone (`strategic_arms`): after 23/23, begin 15/15, mod 64/65, facts 44/48, perform 18/23; the engine lockstep (`strategic_live`) kills the rest. First run: core alone 108/188. The 10 left: `is_complete` returning `true` (the
  exhaustion comes one answer early: only visible in the card's flags), the Imperial primary's `finish_if_won` / unlock branch (needs a score that reaches the target: the string lockstep of `imperial_primary_scores...` is the dual's), the `0 -> 1` of a capacity
  test in `te_uses`, two board/hyperlane filters of the free-action systems, Diplomacy-and-Xxcha `&&`. The refactored functions of `rules/strategy_secondary.rs` (`follow_choice`, `take_answer`, `pay_waiver`, `note_answer`): 48/61 with the core tests; the rest is
  covered by the dual's `strategy_secondary_dual_tests` (not run here). The debug build of the scratch tree found a real panic (`core.galaxy()` reads the map slice; fixed with a guard and a test).
* `playthrough_quick.sh --maxsteps 1500` (gate profile, sol, 4 players, seed 1, te card set): island E 121 counted bails BEFORE and AFTER (the bin still plays the string `Game` under `all=dual`; `d1::pending.close/open [bail: StrategySecondary]` 49 + 49, `strategy_cards::technology_primary` 7, ...).
  They fall only when the wrapper (D2-8) calls `live_reconcile`/`live_step`. The native arms are proven by the lockstep and the shadow oracle above, not by this table. Whole run: 56 causes, 0 unmapped, 0 lent.

## Hand-overs

* **Lane F (menu):** on the answer `Opt::Strategic{card}` call `driver::windows::strategic::begin::begin_strategic(core, env, card)` in the same call (the string does the primary in the step of the answer); the observer `on_strategic_action_chosen` is the wrapper's, fed from its result.
  `closing::options` (`MenuSources`) is what the end of every action still stops at when a transaction partner, a free leader or a second action exists.
* **Lane M6 (tactical):** in `begin.rs` replace the `TurnGap::FreeTactical` by `tactical::open_free(core, active, system)`; in the tail, where `Then::LeaderStrategy` / `Then::Secondary` is decided, call `strategic::after::open_after(core)` (the next `Arm::Ask` call runs it).
* **Integration agent:** `Opt::Waive(shape)` (the dual's) and `Opt::WaiveBy(module, code)` (native) are two forms of one option; `Reason::Scheming` replaced lane C's `SchemingDiscard`; `state::List::Produced` and the `UNITS_PRODUCED` staging of `perform::announce_made` are the same shape as M2's
  (replace `TurnCodes::{production, units}` by `codes.production.{production, units}` when M2 is in); `live::kernel::static_of` and the position-carrying `render_asking` are copied from integration-10.
* **Not ported:** Peace Accords (gap), TE Warfare's free action (M6), Doctor Sucaban's offer in a research (gap), Mindsieve (out), `Opt::PrimaryInstead` (Firmament, dropped).

Not run (standing rule): golden/wide dual sweeps, benches, soak, mutation ratchet, census, workspace clippy, the armed dual, the Z-gate.
