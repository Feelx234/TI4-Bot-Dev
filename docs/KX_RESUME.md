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

(see the end of this file)
