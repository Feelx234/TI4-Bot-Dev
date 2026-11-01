# D2 lane E2 (`core-e2-strategic`): Peace Accords, the Technology primary, the status close, the secrets, Galactic Crisis Pact -- resume note (2026-11-01)

Branch `core-e2-strategic`, cut from `core-integration-12` (`fea5991b1`). Never pushed; the live branch `nightly-fixes-2026-10-26` is untouched. Read `docs/INTEGRATION12.md`
(section 6 is the gap matrix this lane works off), `docs/KX_RESUME.md` (the strategic action), `docs/KV_RESUME.md` (the agenda), `docs/KS_RESUME.md` (the status phase),
`docs/G5_RESUME.md` and `docs/H4_RESUME.md` (the two lanes that share the Research window).
Pure refactor: `RULES_VERSION`, serde, fixtures and fingerprints are unchanged (the new `Ask`/`Opt`/`Subtype` variants are never stored; no state field was added).

## 1. Result in one table: the first stop of the 96 games (`--factions all --players 3,4,5,6 --seeds 1`, gate profile, `te` card set)

| | integration 12 | this branch |
|---|---|---|
| games that finish with zero gaps | 0 | **16** (9 rounds, 540-1,070 steps each) |
| native steps in all | 3,729 | **33,014** |
| games that stop at Peace Accords | 45 (+1 secondary) | **0** |
| games that stop at the Technology primary (`StrategyAbility`) | 16 | **0** |
| games that stop at the status `tokens` close | 9 | **0** |
| games that stop at a secret (`ScoringSecret`, new once the close was past) | -- | **0** (it was the first stop of 23 games the moment the close ran) |
| the quick game (sol, 4 players, seed 1) | 54 steps, round 1, `StrategyAbility` | **888 steps, 9 rounds**, then `Menu(ActionCard)` |
| `--quick --shadow` | 36 legacy steps compared, then the gap | **54 compared with no difference**, then the glued follower question (section 5) |

What stops the 80 other games is not in the strategic, status or agenda islands (counts of the FIRST gap): a leader's component action `Menu(Unrouted(5, ..))` 39
(F2e), technology `Menu(Unrouted(6, ..))` 20 (the Sling Relay / technology dispatch of the menu: F2d, not in this tree), `Menu(ActionCard)` 6 (an action card played from
the menu), `MissingFact` 6 (tactical 3, menu 2, aftermath 1: M-lanes), a faction component `Menu(Unrouted(11, ..))` 4 (F2e), Foresight `AbilityGap(41000)` 2 (G4),
`Menu(Faction(Cabal))` 2 (F2e), `CombatUnported` 1. Seeds 2-4 (`--seeds 3 --first-seed 2`, 288 games): no game finishes (the first stop is a leader's component action 114,
an action card from the menu 79, a faction component 22, `MissingFact` 20, ...); the one stop of this lane in them is Peace Accords on a Freelancers exploration (section 5).
The **agenda island is still not reached by any game** (nobody takes Mecatol Rex before the games end or stop), so the agenda phase is proven by its lockstep tests only.

First gap per game, steps then cause (L5 leader component action, T6 technology menu key, F11 faction component, AC action card from the menu, MF `MissingFact`, AG Foresight,
VC Cabal faction menu, CU `CombatUnported`, FIN finished):

| faction | 3 players | 4 players | 5 players | 6 players |
|---|---|---|---|---|
| sol | 632 L5 | 888 AC | 691 T6 | 100 L5 |
| hacan | 96 F11 | 118 F11 | 286 T6 | 17 L5 |
| letnev | 765 FIN | 387 T6 | 55 L5 | 19 L5 |
| xxcha | 841 FIN | 860 AC | 56 L5 | 49 L5 |
| jolnar | 727 FIN | 85 T6 | 27 L5 | 19 L5 |
| l1z1x | 702 FIN | 964 AC | 47 L5 | 19 L5 |
| arborec | 59 L5 | 11 L5 | 73 L5 | 44 L5 |
| mentak | 710 FIN | 1022 FIN | 988 FIN | 180 L5 |
| muaat | 28 F11 | 11 L5 | 163 L5 | 44 L5 |
| naalu | 122 AG | 219 L5 | 41 AG | 161 L5 |
| nekro | 26 L5 | 11 L5 | 13 L5 | 14 L5 |
| saar | 206 T6 | 832 T6 | 125 T6 | 176 L5 |
| sardakk | 624 FIN | 745 T6 | 522 CU | 451 L5 |
| winnu | 642 FIN | 833 L5 | 829 FIN | 69 L5 |
| yin | 762 FIN | 798 AC | 782 AC | 196 L5 |
| yssaril | 64 L5 | 72 L5 | 152 F11 | 46 L5 |
| ghost | 303 T6 | 803 T6 | 231 T6 | 200 L5 |
| argent | 651 FIN | 564 MF | 97 T6 | 182 L5 |
| cabal | 510 T6 | 33 VC | 449 VC | 46 L5 |
| empyrean | 118 MF | 719 MF | 174 MF | 45 L5 |
| mahact | 209 T6 | 962 AC | 1070 FIN | 92 L5 |
| naaz | 741 FIN | 273 T6 | 150 MF | 195 MF |
| nomad | 247 T6 | 11 T6 | 290 T6 | 44 T6 |
| titans | 722 FIN | 395 T6 | 992 FIN | 180 L5 |

Native steps by arm (96 games): turn prelude 5,833; action menu 5,859; tactical 7,031; aftermath 2,515; secondary 4,014; tokens 2,267; draft 2,432; closing menu 731; phase 967;
scoring 375; event scoring 310; status 301; trade 271; strategy announce 96; starting tech 12. By island: C status 3,253 steps and no gap, E strategic action 4,014 and no gap
of the lane.

## 2. What is native now

| piece | where | string |
|---|---|---|
| **Peace Accords** (Xxcha, after a Diplomacy primary or secondary): the candidates, the typed question, the annexation | `rules/peace_accords.rs` (`PeaceAccords`, `annexable`, `has_peace_accords`), `choice::strategy::{Ask::PeaceAccords { controlled }, Opt::Annex(System, Planet), Subtype::PeaceAccordsAnnex, ability_ctx}` | `faction_abilities::{annexable, strategy_resolved}` |
| ... then Integrated Economy's production on the planet, the fleet-supply enforcement of its system, the exploration of the planet by its first trait (announced to nobody) | `driver/windows/strategic/perform.rs` (`resolved`, `peace_accords`, `integrated_economy`, `run_spawn`) | `technology::control_gained`, `production::integrated_economy`, `exploration::explore_with` |
| renderer (byte-exact: prompt `Peace Accords: annex a planet`, option id = the planet, kind `annex`, label `gain control of {planet}`, details `system`/`planet`, the preview `PlanetsControlled` n -> n + 1) | `ti4-engine/src/render/strategy.rs`; Integrated Economy's question drops its context (`live/details.rs`: the string asks it with a plain `Choice::new`) | |
| **the Technology primary / secondary**: Doctor Sucaban (and Ssruu's copy) in the Research window | the two H4 commits (`ef115be9f`, `021b21b03`), imported, see section 3 | `strategy_cards::doctor_sucaban` |
| ... a paid research whose bill takes the prerequisite away (a specialty planet that covered it paid it) puts the position back and ends with nothing gained | `Research::{pick, waiver_pending, land}` | `paid_research`: `*state = before; if !waiver_required { return Ok(false) }` |
| the Thunder's Edge Construction primary on a late board | `strategic/facts.rs::te_uses` (the systems the string holds an entry for), `kernel/children.rs::TE_USES` 12 -> 64 | `te4_construction` (`state.board.keys()`) |
| **the status phase's close** | nothing new in the core: KS5 had it (`TurnJob::StatusFinish`, run by `advance_tokens` in the call that places the last token). The native_play harness still counted it as lent (`lent_close(Arm::Tokens)`, 9 games); that entry is gone | |
| **Strengthen Bonds, Produce en Masse** (the two secrets `secret_progress` answered `Unsupported`) | `query/objectives.rs::{rival_note_issuers, wants_capacity}` and the rows of `secret_row`; `seen/objectives.rs::terms` fills `Terms::capacity` for a seat that holds or sees the card; the engine's typed view (`objectives_core`): `terms_of` fills it from `capacity_text`, the view loads the Cards and Trade slices | `secrets::{rival_note_issuers_count, production_in_one_system_count}` |
| **Galactic Crisis Pact** (each player, from the speaker, may perform the secondary of the elected card without a command token) | `rules/crisis_pact.rs` (`CrisisPact`), `driver/agenda/crisis.rs` (the seats, the eligibility, the card's own child window), `Ask::CrisisPact { card }`, renderer + `live/strategic.rs` (the follower's display facts without `tokens_left` and `played_by`, with the agenda card) | `agenda_effects::effect_crisis`, `strategy::crisis_pact_choice` |

## 3. Join points with the other lanes

* **H4 (Doctor Sucaban, `core-h4-leftovers`)**: the Technology primary stopped only at `research_asks_more` (a readied Jol-Nar agent at the table and infantry to trade). H4 ported it in
  the core Research window. To MEASURE this tree I cherry-picked H4's two Sucaban commits as one commit (`8b7bd42f8`): the hunks are H4's, byte for byte, so the merge of
  `core-h4-leftovers` is clean there; one test conflict (`strategic_arms.rs`) was resolved by keeping the Titans test of this base and letting the two `research_asks_more`
  tests H4 deleted stay deleted. The commit can be dropped at integration if H4 is merged first.
* **G5 (the research waiver questions, `core-g5-abilities`)**: not imported (it conflicts with H4 in `strategy_cards.rs`, `choice/strategy.rs`, `render/strategy.rs`, as its note says). The two
  lanes meet this one in the Research window: `Research` gets two fields here (`pick: Option<Box<CoreState>>`, `waiver_pending: bool`) next to H4's `sucaban`, `offer_at`, `checkpoint` and G5's
  `declined`, `waiving`, `before`. **G5's `before` is the same snapshot as `pick`** (the position when a paid technology is picked, before the bill): on the merge, keep ONE of them. The
  branch to unify is `land`: G5's `try_settle -> Settled::{Landed, Failed, Waiver}` has `Failed => self.failed = true`, which is exactly the case this lane carries: **`Failed` without a
  waiver must restore `before` and end the research with nothing gained, not set `failed`** (the string's `*state = before; if !waiver_required { return Ok(false) }`; two games of the sweep
  stop on it, `SpaceDockII` of a Xxcha seat and `DacxiveAnimators` of an Argent seat). `waiver_pending` is the bit G5's `Settled::Waiver` already is. The core test
  `research_e2::a_bill_paid_with_the_specialty_planet...` and the lockstep `peace_accords_live_tests::research::...` pin it.
* **F2e (leader and faction components)**: nothing touched. `tools/engine_core/native_play.sh` shows their stops as `Menu(Unrouted(5|11, ..))`, `Menu(Faction(..))`.
* **The invasion lane (`driver/windows/invasion/fight.rs`, `Why::IntegratedEconomy`)**: a planet taken by invasion still bails when the invader owns Integrated Economy. `perform::integrated_economy`
  (budget from `rules::production::planet_value`, the facts from `driver::children::one_link().facts`, `EconomyWindow`, then the `Spawn::Seeing` enforcement) is the whole composition; the invasion
  can call it from `finish_control_gain` (make it `pub(crate)` and move it, it reads nothing of the strategic action).
* **Hot shared files, additive edits only**: `choice/strategy.rs` (three variants inserted after `ImperialScore` / `Objective` / `ImperialScoreObjective`, `CrisisPact` after
  `PeaceAccords`; G5 appends at the ends, H4 inserts after `ResearchTechnology`), `render/strategy.rs` (arms next to those), `driver/windows/strategic/perform.rs`, `live/strategic.rs`,
  `live/details.rs` (one arm), `kernel/children.rs` (one constant), `game.rs` (one test module line), `rules/mod.rs` (two lines), `driver/agenda/{mod,close}.rs` (one line, one early return).

## 4. Findings and breaks, each with its guard ("fails before": R = seen to fail on the code before the fix, B = by construction)

| # | Where | What | Fix | Guard |
|---|---|---|---|---|
| 1 | `native_play::lent_close` (harness) | the status close was counted as lent for 9 games although KS5's `StatusFinish` runs in the same call | the entry is gone | `native_play::tests::the_last_token_of_the_status_phase_runs_the_close_natively_in_the_same_step` (R: the old test pinned the gap), the quick game's nine rounds |
| 2 | `objectives_core::ObjView::build` (the engine's typed view of the objectives) | **it did not load the Cards and Trade slices**: the Dual comparison of Strengthen Bonds found `typed 0 / string 1` the moment the secret was answered by the core | the view loads both | `peace_accords_live_tests::secrets::strengthen_bonds_is_answered_by_the_typed_view_like_the_string` (R: difference 1) |
| 3 | a paid research whose bill takes the prerequisite away | the core ended `failed` (a counted bail in 2 games) where the string restores the position and ends | `Research::land` | `research_e2` (R: `failed` on the old code), lockstep `research::a_bill_paid_with_the_specialty_planet...` (counts `(1, 0)`; the Dual backend counts a bail as `(0, 1)`, B) |
| 4 | `strategy_research_tests::inheritance_systems_after_the_bill_keeps_the_string_when_the_two_resources_are_gone` | pinned "the core does not carry that rollback" (`(0, 1)`): the same fix carries it | renamed, now `(1, 0)` with the position checked | a trip-wire that fired as written |
| 5 | `strategic/facts.rs::te_uses` | War Machine puts PRODUCTION in every system: the core listed all 231 cells of its board, `TeUses::new` (12) refused and the Construction primary stopped (`StrategyAbility`, an L1Z1X table at round 6) | only the cells the string holds an entry for (`TOUCHED`); `TE_USES` 64 for a late board | `te_uses_e2` (3 tests; R: `te_uses` is `None` on the old code) |
| 6 | `status_phase`, `strategic_arms` | four tests pinned the gaps (`ScoringSecret` for Strengthen Bonds twice, `PeaceAccords` for an Xxcha follower, the `NoEffect(15)` of the pact in `agenda_live_tests`) | now pin the new behaviour | trip-wires |
| 7 | `native_play` / `native_subject` tests | the quick game's counts (54 steps, round 1, `StrategyAbility`), the shadow's "no difference", `KNOWN_SILENT = [StrategyAbility]` | updated to 888 steps / 9 rounds / `Menu(ActionCard)`, 54 compared then the documented glue, `KNOWN_SILENT` empty | trip-wires; nothing loosened |
| 8 | the new lockstep of Integrated Economy | the string asks the production question with no context, the core question has one | `live/details.rs` drops it | `peace_accords_live_tests::the_xxcha_with_integrated_economy...` (R: `context` differs) |
| 9 | Peace Accords on the Thunder's Edge planet | the string's `legendary::control_gained` breakthrough looked like a gap | the planet is placed during play and printed on no system, so it is never a candidate | `peace_accords::the_planet_the_thunders_edge_legend_places...` |

| 10 | **my own first lockstep of Peace Accords explored nothing** (found by the mutation run: `Exploring` negations survived) | `fixtures::plain_hub` is made of the first systems of the corpus, which are HOME systems: every candidate planet was a faction homeworld with no trait to explore into | `explorable_hub()` of the test file: seven systems whose planets all have a trait | the explored test now asserts the exploration decks differ from the declined run's; with it the native exploration agrees with the string, question by question |

Not a bug: the lockstep needed `promissory::deal` after seating (trap T1 of `docs/todo/D2-prereqs.md`: the cards codec refuses the placeholder faction).

## 5. Not done / what remains

* **Peace Accords + a Freelancers card** (1 stop in 384 games: `empyrean`, 4 players, seed 2, step 33): the exploration of the annexed planet drew Freelancers and the unit it produces stands over the fleet supply or the
  capacity of the system; the string asks which units go, `Exploring` cannot (`EWhy::Freelancers`, "a unit that stands over the supply", documented in `explore_windows.rs`). The gap is counted
  (`TurnGap::PeaceAccords`). It needs the `Spawn::Seeing` child inside the exploration (explore lane).
* **The shadow's glued follower question** (INTEGRATION12 section 8 item 4, unchanged): `--quick --shadow` now compares 54 legacy steps and meets it at native step 85 (a Thunder's Edge Warfare
  primary: one native `secondary` step asks the redistribution after the free action AND the first follower; the legacy game asks the follower in its next step). `native_play::tests::the_shadow_run_agrees_with_the_legacy_game_up_to_the_glued_follower_question_of_the_warfare_card`
  pins exactly that difference and nothing else.
* **Technology: waiver-only research** (`Settled::Waiver`, a Cabal / Yin seat): G5's. Specialist Compounds (a Thunder's Edge breakthrough) and the Deepwrought commander (dropped) stay in the string.
* **Integrated Economy in the invasion** (`Why::IntegratedEconomy`): see section 3.
* **The agenda island is not reached by a game**: Galactic Crisis Pact is proven by `agenda_live_tests` (both answers) and the sweep over every agenda (`every_agenda_is_voted_and_resolved_like_the_string_*`, `GAPS` is empty now).
* The harness `--shadow` is unchanged otherwise.

## 6. Checks (cheap), private target `/root/deploytmp/core-e2` (`CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, no incremental; deleted at the end)

* Whole `ti4-engine --lib` (lockstep profile): **5213 passed**, 7 ignored (int-12: 5199) plain, and the same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`: 5213 passed (run before the last test-only edits; the E2 modules and `strategy_research_tests` were rerun after them, plain: 33 + 27 passed).
* `ti4-engine-core --lib` 480, `-factions --lib` 145, `ti4-rule-bot-typed --lib` 1, `ti4-engine-core-bridge --lib` 107 (all pass after the pins of section 4), bridge `kernel_env` 3, `play_facts` 3, `menu_sources` 2.
* Core tests, named binaries (39, all pass): `peace_accords` 8, `crisis_pact` 4, `secret_progress_e2` 5, `research_e2` 3, `te_uses_e2` 3 (new), `query_objectives{,2}`, `status_phase`, `strategic_arms`, `research{,2}`, `research_sucaban`, `strategy_*`, `agenda_*`, `objective_windows`, `scoring_book`, `turn_jobs{,2}`, `explore_windows`, `exploring{,_freelancers}`, `leadership`, `construction`, `source_hygiene`, `driver_{windows,flow}`, `aftermath_chain`, `combat_{machine,fight}`.
* Lockstep against the string (new, in `ti4-engine/src/peace_accords_live_tests.rs` and `agenda_live_tests.rs`): Peace Accords as the primary's player (annex with exploration, decline, another faction), as a follower, with Integrated Economy (and at the edges of its budget); the pact (all decline, all accept); Strengthen Bonds and Produce en Masse through the Dual backend of the objectives area; the paid research whose bill takes the prerequisite away.
* `check_layers.py core`: 559 files, 0 violations. `lint_core.sh`: 561 files, 0 hits. `clippy_core.sh`: ok (core crates 0 warnings, engine 41 = baseline); it found `iter().any` -> `contains`, a lockstep file's complex types and an unused helper, fixed.
* `native_play.sh --quick`, `--quick --shadow`, `--factions all --players 3,4,5,6 --seeds 1` (and `--seeds 3 --first-seed 2`): section 1.
* Mutation (`tools/engine_core/mutants.sh`, committed snapshots in a scratch worktree, run by a Haiku runner and by me, `D4_COVERS_FILE=/dev/null`, single files; core tests (A) and engine lockstep (B) as two sets, union):

| target (functions) | A core tests | B lockstep | union |
|---|---|---|---|
| `rules/peace_accords.rs` | 20/20 | 17/20 | 20/20 (the three `&&` of `annexable` are the core tests') |
| `rules/crisis_pact.rs` | 12/12 | 11/12 | 12/12 |
| `driver/agenda/crisis.rs` | -- | 10/10 | 10/10 |
| `query/objectives.rs` (`rival_note_issuers`, `wants_capacity`, the rows of `secret_row`, the arms of `secret_progress`) | 43/49 | 3/49 | **all of E2's killed**; the 6 survivors are older rows and arms of the same functions (`DestroyHereticalWorks` 2, `FosterCohesion` 0, `DefyspaceAndTime` 1, `SameColourTechnologies`/`ActionCards`/`CombinedValue` `unwrap_or`) |
| `seen/objectives.rs::terms` | 3/8 | -- | E2's lines killed (the capacity scan; the two equivalent mutants carry `mutants:skip`); the 5 survivors are the older hook terms |
| `strategy_cards.rs` (`land`, `settle`) | 10/15 | 11/15 | `land` 100%; the survivors are older lines of `settle` (the Inheritance path) |
| `strategic/facts.rs::te_uses` | 4/4 | 3/4 | 4/4 |
| `strategic/perform.rs` (`resolved`, `peace_accords`, `integrated_economy`, `run_spawn`) | 2/22 (the driver has no core test but `strategic_arms`) | 20/22 | 20/22; the two left are equivalent and carry `mutants:skip` with the reason (`announce` of the exploration, `budget <= 0`) |

First runs (before the tests added for them): `peace_accords.rs` 20/20, `crisis_pact.rs` 12/12 (A), `objectives.rs` 44/51, `te_uses` 2/4 (A), `land`/`waiver_only` 3/8 (a function this note no longer has: `waiver_pending` replaced it),
`perform.rs` B 16/22. The run found that the lockstep of the exploration was vacuous (finding 10).

Not run (user rule): `end_of_d1.sh` and its steps (golden / wide dual, `board_equivalence`, `core_sync_gate`, `alpha_roundtrip`, benches, soak, ratchet, census, workspace clippy beyond `clippy_core.sh`), `decision_replay`,
`TI4_CORE_STRICT=1`, Playwright. The private target and the scratch worktree/mutation directories are deleted at the end.

