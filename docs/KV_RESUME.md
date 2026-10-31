# D2 lane D (`core-d2-c1-agenda`): the agenda phase, resume note (2026-10-31)

Branch `core-d2-c1-agenda`, cut from `core-d2-c2-turn` (`fe3e3b85`) and merged with `core-d2-c1-status` (lane C: `EventScoring`, `open_event`, the close
jobs). Read `docs/KP_RESUME.md` (the jobs of the turn flow, `inline::run`, the tape), `docs/KS_RESUME.md` (the close pattern), `docs/D2_LF_RESUME.md` and
`docs/D2_LF2_RESUME.md` first. Never pushed; `nightly-fixes-2026-10-26` untouched. Packages KV1-KV3 of `docs/todo/D2-islands-composition.md` (section 6.1).

## What is native

| piece | where | notes |
|---|---|---|
| `Arm::Agenda` (KV1) | `driver/agenda/flow.rs::run_open`, job `TurnJob::AgendaOpen` | the `AGENDA_RESOLVED` flag, the two top agendas, then `open_next_vote`: the reveal chain (`rules::agenda::reveal::Reveal`: Quash, Political Favor, Veto, Political Secret, the replacement chain), Committee Formation (`OfferDiscard` + `Committee`), the vote put on the stack; at the end `ready_after_agenda_phase` (Checks and Balances' limit) and `AGENDA_PHASE_RESOLVED` |
| `Arm::Voting` (KV2) | `driver/agenda/mod.rs::VotingWin` (kind `Voting`, `Held`) | payload = the existing `state::Voting`; `Vote::restore`/`to_payload`/`settle`/`casts` (new, `rules/agenda/window.rs`); every answer is followed by the vote's own settle, the `agenda_casts` mirror and `VOTES_CAST` (once per voter from `RULES_BRIBERY_PER_PLAYER`) through the reaction windows; the voting order is `rules::agenda::voting_order` over a real `KernelEnv` (Nekro's bar, Elder Qanoj, Argent's first vote, Hack Election's last one; `Facts::of_core`: Blood Pact partners from the faceup notes) |
| the close (KV3) | `driver/agenda/close.rs` | the call that applies the last answer also closes: Covert Legislation's hidden agenda, `Close` (AGENDA_RESOLVED, the cards played into it, log, redirect, correct riders, law), the effect (`Resolve`), the feat `ElectedByAnAgenda`, the agenda-timed scoring window (lane C's `open_event`) with the agendas to come held behind it (`AgendaQueueAfterEventScoring`); a vote that resolved nothing goes straight to the next agenda |
| `TurnJob::AgendaNext` | `flow::run_next`; lane C's `advance_event_scoring` closes into it when the queue is on the stack | the next reveal and vote after the scoring window (replaces lane C's `TurnGap::AgendaQueue`) |
| effects now in the core | `rules/agenda/effects/placements.rs`, `driver/agenda/grant.rs` | Swords to Plowshares (Against), Rearmament Agreement (For), Colonial Redistribution (new `Ask::Settler`), Research Grant Reallocation (the relics' technology question composed at the driver level: an area window may not import another) |
| announcement of an agenda's destroyed ground forces | `driver/agenda/ground.rs`, called by the reconcile (`driver/turn/prelude.rs::announce_staged`) | the staged `PrivateStagedGround` rows with cause `DestroyReason::Agenda` become `GROUND_FORCE_DESTROYED` in the next step's reconcile, before the staged events (the string's order); other causes are still the staging lane's |

State/protocol decisions: the vote and the close stay in ONE held window (`VotingWin`) so the next agenda's vote can replace it inside the step (the driver closes the window kind
on `Done`, which would pop a window the step had just opened); the vote of the next agenda is asked as the arm's stage question. A question from inside the step (a reaction window,
a rider's pick) goes to the tape (`driver::turn::inline`), so a caller that returns between questions continues from the position alone (`Pending::nested`).

Registry: `driver/live.rs` `live_arm`/`ask_arm_fn` gained `Agenda` (`agenda::advance_agenda`, `turn::ask_job`) and `Voting` (`agenda::advance_voting`/`ask_voting`). Engine half:
`live/agenda.rs` (the texts `AgendaCodes`, the whole tally of a long election, the Committee prompt, research labels), `reactions_core::set_table_codes`, `native_subject::open_arm`
(`Arm::Voting` from the observed window, `Arm::Agenda` opens itself), the lockstep `agenda_live_tests.rs`.

## Found and fixed on the way (each with a test that fails on the old code)

* `driver/windows/scoring.rs` (lane C): an agenda's event window recorded the occurrence a seat used (`scored_feat_occurrences`); only the one-per-player window does (`ScoringWindow::score`). Test: `a_for_against_law_...` / the sweep (position differs).
* `Tally::MAX` (8): a planet election among more than 8 planets made the vote question `None`; the question keeps the first 8 and `live::agenda::full_tally` rebuilds `details.tally` from the vote on the stack.
* `KernelEnv::is_nekro` answered `false` ("the adapter applies it"): Galactic Threat now bars the Nekro seat in the core's voting order. Test: `nekro_cannot_vote_on_agendas_...`.

## More native pieces added after the first draft

* The placement riders (Construction, Diplomacy, Warfare) in `rules/agenda/close.rs`: the place is forced when there is one and asked when there are several (new `Ask::RiderTarget`, `Opt::Spot`, rendered byte-exact);
  the space dock goes where the string puts it (the dock I forms only count as a dock on the planet: the string compares the unit id with `spacedock`, so a planet with a dock II is offered again: parity kept),
  Diplomacy takes a fleet token from every other seat that has one and stages `COMMAND_TOKEN_PLACED`, Warfare places the dreadnought without a supply check (as the string).
* Draws with hooks: `driver/windows/hooked_draw.rs` (`HookedDraw`, registered in `driver/children.rs` for `Spawn::DrawCards`): without a hook it is `DrawCards`; with one (Yssaril's Scheming and Crafty, a draw bonus, a changed limit,
  blank laws) the bonus card joins, `SchemingDiscard` (lane C) comes before the limit and the limit is the faction's. The Politics Rider and Unconventional Measures of a Yssaril seat run natively.
* The card moves a faction effect staged in a window (a Scheming discard) are announced right after `AGENDA_REVEALED` and `AGENDA_RESOLVED` (`kernel/fx_cards.rs`, as `Live::emit_typed` does).
* The Technology Rider's research options are labelled with the printed names (`live::agenda::complete`).

## Gaps (counted, never silent): `Unknown::AgendaGap(AgendaGap)` -> `live::<arm> [AgendaGap(..)]`

`kernel/agenda_codes.rs`. Open at the end of this lane:

| gap | when | who closes it |
|---|---|---|
| `NoEffect(15)` | Galactic Crisis Pact elected: each seat may perform the secondary of the elected strategy card without a command token (`strategy::crisis_pact_choice`, `strategy_cards::secondary`) | lane E (KX): the per-card secondary windows and `secondary_can_do_something`; one `Kind::Pact` in `rules/agenda/effects` or a driver-level window like `driver/agenda/grant.rs` |
| `CloseUnsupported` | a correct Technology Rider at a seat the research window does not carry (no unit roster in the table's play facts, `research: None`); any correct rider below `RULES_COMMAND_TOKEN_POOL` (12) or a placement rider below `RULES_TECH_RIDER` (9); `Classified Document Leaks` electing a non-secret | old rules versions: the string path; the roster: the live `Env` must carry `plays` (`env_with_plays`) |
| `EffectUnsupported(n)` | Plowshares / Rearmament / Redistribution / Grant without the unit roster in the table's play facts | same |
| `PhaseNotReady`, `RevealFailed`, `CloseFailed`, `ReplayDiverged`, `OpenedWindow`, `EmissionRefused`, `Refused` | defects of the position or the environment; `Refused`: a deck reserve (turn redo, dropped) | n/a |

Not native by decision (the dropped content): the Executive Order, the Keleres agent's goods window, `Arm::AgendaTalks` (structured diplomacy), the Deepwrought oceans, Last Bastion's dock bonus.
NOT done: the vote of a Valefar Assimilator carrying Genetic Recombination (the holder query of the core reads the owned technology only; the string's `mahact::recombination_holder` also reads the Valefar token): the core would
ask the wrong seat. Not covered by a scenario either; list it with the Nekro lane.

## Checks (cheap)

See the end of this file.
