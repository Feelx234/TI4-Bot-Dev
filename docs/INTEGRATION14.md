# core-integration-14: merge record (2026-11-01)

Branch `core-integration-14`, cut from `core-integration-13` @ `52f3ae0ef`. Four merges in the ordered list (F2e's dispatch4, H5, G5, E2), cheap checks only (the user rule: heavy work waits for a zero-bail
playthrough; one is not possible yet, see section 6). Not pushed. The live branch `nightly-fixes-2026-10-26` was not touched.
Pure refactor: `RULES_VERSION`, serde, fixtures and fingerprints are unchanged (no file of those was edited; the one new window field, `Research::waiver_required`, is core-only payload of a window, never stored).

## 1. Merges

| # | Branch | Tip merged | Merge commit | What it brings | Files in conflict |
|---|---|---|---|---|---|
| 1 | `core-d2-c3-dispatch4` (F2e) | `6c40af55a` | `d2da0042b` | the rest of the menu options routed: Umbat, Letani Miasmiala, Rin, Riftwalker Meian, the Nekro / Kyver / Carrion / Hesh and Prit / Mirik / Procyon / Mathis heroes, the relic uses (Enigmatic Device and its exploration cards, Maw of Worlds, Stellar Converter, Titan Prototype, Silver Flame), Wormhole Generator, Nekro's C4R, Titans' Terraform, Cabal's Vortex (the three menu gaps), `turn/{hero,produce}.rs` | 1 |
| 2 | `core-h5-leftovers` (H5) | `0d295c0e4` (contains H4, already in int. 13) | `7fcf62915` | Imperial Arbiter and the Quantum Datahub Node swap (`driver/turn/swaps.rs`), Chaos Mapping, the Yin hero's scoring window (`Spawn::EventScoring`), Duranium Armor lockstep, the invasion in every non-card question's context, clippy repairs | 12 |
| 3 | `core-g5-abilities` (G5) | `97e92d02a` (on G4, already in int. 13) | `7b54a6150` | the Research window asks the waiver and its payment (Cabal Riftmeld, Yin Brother Omar), `kernel/fx_cards.rs` (the consumer of the staged card moves, called after every top-level typed event), `Seen::revealed_cards` (M'aban) | 6 |
| 4 | `core-e2-strategic` (E2) | `351315c01` | `635c54126` | Peace Accords, Galactic Crisis Pact, Strengthen Bonds / Produce en Masse answered by the core, the Thunder's Edge Construction uses on a late board, the paid research whose bill takes the prerequisite away, the status close counted as native | 6 |

Later commits are the fixes the checks found (section 3). The neutral-side bail of `dual_window::settle_run` was checked after EVERY merge (`awk '/^fn settle_run/,/^}/' crates/ti4-engine/src/dual_window.rs | grep -ci neutral`):
**0 each time**. `cargo check --workspace --all-targets` (without ti4-review, ti4-replayer, ti4-mlp, ti4-tensor, ti4-advisor) after every merge: merge 1 green, merge 2 two compile errors (B1), merge 3 green, merge 4 one compile error (a stale line of
E2's `live/details.rs`, B1).
E2's import commit of H4's Sucaban hunks (`e6330f664`) came in with E2's history; its hunks are byte-identical to the ones already in the tree (H4 is in int. 13), so the merge took them as no change (no duplicate: the check would have
failed with a second `sucaban_sources`).

## 2. Conflicts and how they were resolved (keep both sides)

| File | Conflict | Resolution |
|---|---|---|
| `driver/agenda/ground.rs` (1) | F2e announces the ground forces of UNIT.DSGN.FLAYESH with a special `match` arm (cause `codes.turn.nekrohero`); H4 already generalised the announcement with `GROUND_OTHER` / `codes.agenda.ground_other` (Plague, Unstable Planet, Nova Seed **and the Nekro hero**, `LeaderBare(UNITDSGNFLAYESH)`) | H4's (the same cause text, `render_destroy`); F2e's arm and its `GROUND_CAUSES` loop are not taken. The F2e lockstep of the hero (found "before the goods" ordering) is in the engine lib and the D4 crate: green |
| `native_subject.rs`, `g2_abilities.rs` (2) | the test that H5 moved is already in HEAD (a duplicate hunk); the Ssruu assertion of integration 12 against H5's `&&` form | HEAD's (one test; the integration 12 assertion says the same) |
| `turn_jobs2.rs` (2) | HEAD's partner test ended where H5's Chaos Mapping tests begin (the hunk split a function) | both: HEAD's test, a closing brace, H5's tests |
| `agenda/{close,flow}.rs` (2) | two `allow(large_enum_variant, reason)` texts | HEAD's |
| `driver/turn/mod.rs` (2) | `pub mod technology;` against `pub mod swaps;` | both |
| `kernel/turn_codes.rs`, `live/turn.rs` (2) | HEAD's F2d/F2e codes against H5's `arbiter_prompt`, `arbiter`, `quantum_datahub` (+ the `production` / `units` codes H5's base still had, which integration 11 deleted) | all of HEAD's, plus H5's three; `production` / `units` stay deleted |
| `ability_audit.rs`, `live/trade.rs`, `strategic_live_tests.rs`, `trade_live_tests.rs` (2) | the `Lane` variant H5 deleted against the one integration 12 kept under `allow(dead_code)`; two clippy repairs of H5 against HEAD's | HEAD's variant (and its arm in the status match, B1); H5's clippy repairs (`PlayerId::new(String::from_utf8_lossy(..))`, `if reconcile { loop }`, the `Tweak` alias) |
| **`kernel/fx_cards.rs`** (3, add/add) | lane D (`047061059`) and lane G5 each wrote `announce_staged_cards`: D's is called by name from the agenda reveal and close (`rules/agenda/{reveal,close}.rs`) and emits with `emit_mirrored`; G5's drains the rows, handles the Data Skimmer bail, emits with `emit` and mirrors once, and is called from `Sx::emit_mirrored` after every top-level typed event | **G5's file** (a superset: the same rows, the same events, `Result<usize, _>`). D's two callers only test `.is_err()`, and they find nothing left to announce when `emit_mirrored` already did it (the second call is a no-op). The agenda lockstep sweeps pass. A THIRD copy stays in `driver/windows/status_finish.rs` (the job-flow form with `Flow`): not merged, same events, listed in section 8 |
| `choice/strategy.rs` (3) | `ArbiterGive/ArbiterTake` (H5) against `ResearchWaiver*` (G5), both appended at the end of `Ask` and `Subtype` | all variants |
| **`rules/production/strategy_cards.rs`, the Research window** (3, 6 hunks; 4, 10 hunks) | see section 4 (join point a) | one window with H4's Sucaban stages + G5's waiver stages + E2's rollback, ONE snapshot |
| `render/strategy.rs` (3, 4) | the three lanes' arms for `Ask`, `Subtype`, `Opt`, `parse`, `details` | all arms (Sucaban, Arbiter, ResearchWaiver*, PeaceAccords/Annex, CrisisPact) |
| `choice/strategy.rs` (4) | `waiver_ctx` (G5) against `ability_ctx` (E2) | both functions; H5's comment about the Arbiter's card kept |
| `live/details.rs` (4) | E2's side re-added a `harness_preview` arm for `SystemActivated` that H4 moved into `live/reaction_preview.rs` | HEAD's Carth arm and E2's Integrated Economy arm; the stale arm deleted (B1) |
| `native_subject.rs`, `native_play.rs` (4) | the quick game's counts and doc comments | `native_subject`: HEAD's text; `native_play`: E2's text as a placeholder, every number then re-measured (section 5) |
| `show_neutral` twice (`ti4-engine-core-tests/tests/combat_fight.rs`, join point c) | F2e's branch carries `show_neutral: false` at two lines (a clippy error "specified twice", from the `core-d2-c1-strategic` merge) | **not in the merged tree**: the textual merge keeps the single line of integration 13. `clippy_core.sh` is green |
| `Render::sync_codes` (join point d) | none | H3's checked `extends` body (the `bool` one) is in the tree and no merged branch touches it (read after merge 1 and after merge 4) |

Gap rows: none to delete (`silent::gaps::GAPS` is empty since integration 13 and none of the four lanes adds a row).

## 3. Bugs and breaks found, and how each is guarded

"Fails before": R = seen to fail on the code before the fix, B = by construction.

| # | Where | What | Fix | Guard |
|---|---|---|---|---|
| B1 | compile (merges 2, 4) | `ability_audit::tests::Status::Lane` not matched (H5 deleted the arm, HEAD kept the variant); `turn_live_tests` read `asker.seen` of a `LoneCard` wrapper (H5 wraps the asker); `live/details.rs` used names H4 moved | arm restored; the diagnostic of the panic dropped (H5's form); the stale line deleted | the compile |
| B2 | clippy (`clippy_core.sh`) | unused `Faction` import (`kernel/hooks/sites.rs`: `Waiver` moved there from G5), `native_stop_with` unused (the Arbiter test that called it became native in H5, F2c's wrapper went), two very complex types in G5's `research_waiver.rs` | import and helper deleted, `type Offer` | `clippy_core.sh`: ok, engine 41 = baseline |
| B3 | **hyperlane maps: the core's map followed no lane** (found by `native_play --quick --shadow` once E2's pin of the glued follower question was out of the way: the first difference moved from step 85 to step 352, Peace Accords' candidates) | the string applies the laws and the printed lanes (`hyperlane_lanes`, rules version 22) to its galaxy at every reconcile (`laws::apply_to_galaxy`, game.rs:2525); `NativeGame::from_game` loaded the galaxy of a game that was not stepped yet, so the core's map had `hyperlane_lanes == false`: a lane tile was a neighbour of its systems and nothing was joined across it. The Xxcha seat's question `Peace Accords: annex a planet` listed the planets of one system short (`wellon thibah arinam ...` against `wellon thibah corneeq resculon arinam ...`); it is also the root of the "movement options on a hyperlane map" class of integration 13 (3 games) | `NativeGame::from_game` applies the laws to a clone of the galaxy before the load | `native_play::tests::the_core_map_of_a_hyperlane_game_follows_the_printed_lanes_from_the_first_step` (R: `rules version 22: adjacency follows the printed lanes`); the shadow's first difference moves from step 352 to 397 |
| B4 | **join E2 x G5 in the Research window** (join point a) | E2's `pick` and G5's `before` are the same snapshot: ONE (`before`). E2's rollback is in `land`: `Failed` with a snapshot restores it and ends with nothing gained; without a snapshot it is `failed`. `waiver_pending` is G5's `Settled::Waiver`. **The combination was wrong in a case neither lane tests**: the string reads `waiver_required` BEFORE the bill (`faction_waiver_required`), and a declined waiver is `*state = before; if !waiver_required { return Ok(false) } declined.insert(t)`. G5's decline always asked the technology again. The case: the bill spends the specialty planet that covered the prerequisite and a waiver opens the technology only now: the string asks the waiver, a decline ENDS the research | `Research::waiver_required` read at the pick (`needs_waiver`: prerequisites unmet, no Inheritance Systems, a waiver offered); `decline_waiver` ends the research (`end_paid(false)`, which also restores a Ssruu checkpoint) when it was false | `research_e2::a_waiver_that_opens_the_technology_only_after_the_bill_took_the_prerequisite_ends_the_research_when_declined` (R: with the flag forced true the research asks the technology again); `research_e2::a_technology_only_a_faction_waiver_opens_asks_the_waiver_after_the_bill_and_a_decline_gives_the_bill_back` (replaces E2's pin of `failed`, which G5 overtakes: its own comment said so); G5's six `research_waiver` tests and H4's ten `research_sucaban` tests stay green. The two snapshots that remain are independent: H4's `checkpoint` (before a Ssruu offer, restored by `end_paid` when nothing lands) and `before` (after the Sucaban trade, before the bill) |
| B5 | trip-wire `native_play` (quick game, shadow) | see section 4 | re-measured | the tests themselves |
| B6 | trip-wire `ti4-engine-core-bridge/tests/menu_sources.rs::the_faction_component_actions_listed_equal_the_strings` | it demanded `report.gaps > 0` ("the gap cases were reached"); F2e listed Nekro's C4R, the Titans' Terraform and Cabal's Vortex, so the core lists every faction action | the assertion is `gaps == 0` | the test (fired as written; strengthened, not loosened) |

Not bugs of the merge (checked, nothing to fix): the lib counts grew as expected (the per-file `#[test]` count of the merge equals int. 13 + each branch's own additions, except `strategy_research_tests.rs`: E2 renamed
`inheritance_systems_after_the_bill_keeps_the_string_...` and G5 replaced `bail_the_questions_of_the_paid_research_stay_string` by its lockstep file).

## 4. Join points, as asked

(a) **The Research window.** One struct, the fields of all three lanes: H4 `sucaban`, `offer_at`, `checkpoint`; G5 `declined`, `waiving`, `before`; (E2's `pick` and `waiver_pending` are NOT kept: `before`, `Settled::Waiver`); mine `waiver_required`.
Stages `Offer`, `Trade` (H4), `Waive`, `WaivePay` (G5). `land`: `try_settle` -> `Landed` (drops the snapshot), `Failed` (E2: restore `before` and end, or `failed` when no snapshot), `Waiver` (G5: ask); then `end_paid(landed)` (H4: restore a Ssruu
checkpoint when nothing landed). `waive_and_pay` ends with `end_paid(true)` after a landing (G5's own join note). B4 is the one behaviour the combination needed that no lane had.
(b) **Slots**: G5 registers no rule (no slot). In the tree: `40_`/`41_` the gap rows, `42_` G3, `43_` G2, `44_` G4, `46_` Strike Wing, `45_` free for the next G lane. `reactions_core::tests::no_two_rules_of_the_core_table_share_a_slot` green.
(c) `show_neutral`: see section 2 (not present after the merge). (d) `Render::sync_codes`: H3's body, untouched. (e) the sweep: section 6.

## 5. Edits of assertions, fixtures and harnesses (reasons)

| Where | Edit | Reason |
|---|---|---|
| `native_play` `a_native_game_plays_the_first_turns_...` | gap arm `ask.action_menu` -> `secondary`; 9 rounds / 888 steps -> **8 / 967**; phases 25 -> 22; tokens/status steps `>= 8` -> `>= 7` | B3 changed the game: the core's map follows the lanes now, so the table plays another game (E2's counts were measured on the unfixed map). The cause is the same `TurnGap(Menu(ActionCard))`, asked of a follower in a secondary. `--quick --twice`: the same fingerprint `ed94b770fabcf9f9` both times |
| `native_play` `the_round_the_asks_of_a_job_and_the_shadow_count_are_the_positions` | shadow of 100 native steps: compared 65 (was 57 / 54); a budget of one round ends at `StepLimit` with no difference (was `Gap` / `Finished`) | the 100-step budget now ends before any gap or difference; the old assertions pinned the Peace Accords gap (int. 13) and E2's difference |
| `native_play`: E2's `..._up_to_the_glued_follower_question_of_the_warfare_card` | replaced by `..._up_to_the_known_production_order_difference` (275 legacy steps compared, then step 397 `events [1]`: native `PRODUCTION_USED`, legacy `INVASION_BEGAN`) | the glued follower question is gone (integration 13 B7 fixed the harness); the new first difference is the "production order" class of int. 13 section 6 (M5 / M2, not investigated). A pin of a KNOWN difference, like integration 11's exploration pin: it fails the day the difference is fixed |
| `research_e2` third test | `failed` -> the waiver is asked, a decline gives the bill back | G5 landed; the test said "until it lands the run is the string's" |
| `menu_sources` | `gaps > 0` -> `gaps == 0` | B6 |
| `turn_live_tests` | the panic message of `native_marked_arms` no longer lists the questions seen | H5's form (the wrapper owns the scripted asker); diagnostics only |
| `ability_audit.rs` status match | `Lane` arm kept | B1 |

No assertion was loosened to make a test pass.

## 6. The native playthrough (`tools/engine_core/native_play.sh`, gate profile, `te` card set)

`--quick` (sol, 4 players, seed 1): **8 rounds, 967 native steps**, then `secondary [gap: TurnGap(Menu(ActionCard))]` (a follower plays an action card from the menu). Fingerprint `ed94b770fabcf9f9` (the same on `--twice`).
Integration 13: 90 steps, `PeaceAccords`; E2 alone (on the unlaned map): 888 steps, 9 rounds.

`--quick --shadow`: **275 legacy steps compared with NO difference**, then step 397 (tactical + aftermath) `events [1]`: native `PRODUCTION_USED`, legacy `INVASION_BEGAN` (the known production-order class, M5 / M2). Integration 13: 57 / 90.

The gap matrix over `--factions all --players 3,4,5,6 --seeds 1` (96 games, 79,377 native steps; 80 s): **45 games finish with zero gaps** (integration 13: 0; E2 alone: 16; the 15 games of E2's 16 that are still the same
game finish; the sixteenth, mentak with 4 players, is a 4-player hyperlane game and a different game since B3: it now stops at the action card, step 1003). First gap per game, steps then cause
(FIN finished clean, AC `TurnGap(Menu(ActionCard))`, MF `MissingFact`, ZU `Menu(Unrouted(5, Z'eu))`, CU `UnportedAftermath(CombatUnported)`, IE `Unported(IntegratedEconomy)`):

| faction | 3 players | 4 players | 5 players | 6 players |
|---|---|---|---|---|
| sol | 817 FIN | 967 AC | 777 AC | 819 AC |
| hacan | 1039 FIN | 1358 FIN | 1356 FIN | 1164 AC |
| letnev | 765 FIN | 989 AC | 927 AC | 857 AC |
| xxcha | 841 FIN | 890 AC | 871 FIN | 782 AC |
| jolnar | 727 FIN | 885 AC | 527 CU | 1076 FIN |
| l1z1x | 702 FIN | 986 FIN | 928 AC | 908 AC |
| arborec | 807 FIN | 1050 FIN | 944 AC | 856 AC |
| mentak | 710 FIN | 1003 AC | 988 FIN | 752 AC |
| muaat | 812 FIN | 702 IE | 881 AC | 985 AC |
| naalu | 154 ZU | 219 ZU | 192 MF | 161 ZU |
| nekro | 740 FIN | 1153 FIN | 921 AC | 822 AC |
| saar | 643 FIN | 1151 FIN | 1074 FIN | 793 AC |
| sardakk | 624 FIN | 1098 FIN | 522 CU | 762 AC |
| winnu | 642 FIN | 1176 FIN | 829 FIN | 920 AC |
| yin | 762 FIN | 758 AC | 782 AC | 998 AC |
| yssaril | 746 FIN | 848 AC | 908 AC | 1216 FIN |
| ghost | 718 FIN | 1197 FIN | 792 AC | 1027 FIN |
| argent | 651 FIN | 194 MF | 658 AC | 970 AC |
| cabal | 758 FIN | 1096 FIN | 896 AC | 842 AC |
| empyrean | 118 MF | 1147 FIN | 174 MF | 783 AC |
| mahact | 653 FIN | 1178 FIN | 1070 FIN | 897 AC |
| naaz | 741 FIN | 1179 FIN | 150 MF | 195 MF |
| nomad | 645 FIN | 1240 FIN | 791 IE | 829 AC |
| titans | 722 FIN | 924 AC | 992 FIN | 1088 AC |

| code | first gap | games | owner |
|---|---|---|---|
| FIN | the whole game, 7 to 9 rounds (527 to 1,358 steps) | 45 | -- |
| AC | `Menu(ActionCard)`: an action card played from the action menu (29, `ask.action_menu`) or in a secondary (9) | 38 | lane F (the menu's action-card dispatch; `docs/INTEGRATION12.md` section 6) |
| MF | `MissingFact` (tactical 3, aftermath 1, menu 2) | 6 | the M lanes (E2 knew the same 6) |
| ZU | `Menu(Unrouted(5, 67108917))`: the Naalu agent Z'eu (ACTION: a free tactical action for the chosen player) | 3 | lane F: Z'eu's route needs `begin_free_tactical` for a chosen seat (the rule exists, G3; F2e left it with H4, "dropped content"). Every Naalu game stops there at step 154 to 219 |
| CU | `UnportedAftermath(CombatUnported)` | 2 | M4 / M5 |
| IE | `Unported(IntegratedEconomy)`: a planet taken by invasion while the invader owns Integrated Economy | 2 | the invasion lane (`driver/windows/invasion/fight.rs`; E2 section 3 says how: `perform::integrated_economy`) |

(Integration 13: PA 64, TOK 30, L5 2.) No game stops at Peace Accords, the status close, a leader's component action other than Z'eu, a technology or faction component any more.

The second sweep `--factions all --players 3,4,5,6 --seeds 3 --first-seed 2` (288 games, 75,224 steps, 46 s): **59 finish with zero gaps** (E2 alone: 0; seed 2: 0, seed 3: 1, seed 4: 58). Causes of the rest: AC 172, MF 32, ZU 24, `PeaceAccords` 1
(the Freelancers corner of E2 section 5).

Native steps by arm (96 games): turn prelude 13,650; action menu 14,099; tactical 15,736; secondary 9,191; aftermath 5,557; tokens 6,061; draft 4,804; closing menu 3,783; phase 2,151; trade 1,841; scoring 923; event scoring 758; status 715; strategy announce 96; starting tech 12.
By island: M1 trade 1,841, M5 aftermath 5,557 (3 gaps), M6 tactical 15,736 (3), B turn flow 24,496 (32), C status 8,457 (0), E strategic action 9,191 (9), F menu and leaders 14,099 (2), M4 combat 2 gaps; M2, M3, D agenda: not reached.

## 7. Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/int14` (profile `lockstep`, `CARGO_PROFILE_DEV_DEBUG=0`, no incremental; deleted at the end)

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without ti4-review, replayer, mlp, tensor, advisor) after every merge and at the end | ok |
| `cargo test -p ti4-engine --lib` plain (final tip) | **5279 passed**, 0 failed, 7 ignored (integration 13: 5235) |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (WHOLE lib) | 5279 passed, 0 failed, 7 ignored |
| `ti4-engine-core --lib`, `-factions --lib`, `-bridge --lib`, `ti4-rule-bot-typed --lib` | 475, 149, 108, 1 passed (the first bridge run had 2 failures: B5) |
| bridge `--test kernel_env --test play_facts --test menu_sources` | 3 + 3 + 2 passed (the first run: `menu_sources` failed, B6) |
| `check_layers.py core`, `lint_core.sh` | 574 files, 0 violations; 576 files, 0 hits |
| `tools/engine_core/clippy_core.sh` | ok (core crates 0 warnings; engine 41, baseline 41); the first run found B2 |
| the D4 crate (`ti4-engine-core-tests`) with the flags of `tools/engine_core/d4/run_core_tests.sh` | 153 test binaries: **1925 passed, 0 failed** (built once, at `292eee6b7`, the tip before this document) |
| `native_play.sh --quick` / `--quick --twice` / `--quick --shadow` / the two sweeps | section 6 |
| the usage: five-hour window 12 % at the start, 1 % at the end (the window had reset meanwhile) | -- |

Where the runs were done: all in this worktree with the private target (the D4 crate in the script's scratch worktree `/root/deploytmp/int14-d4wt`), one cargo at a time, as background scripts of this session (no helper agents).
The target dir, the scratch worktree and the logs under `/root/deploytmp/int14*` are deleted at the end.

Not run (user rule): `end_of_d1.sh` and its steps (golden / wide dual, `board_equivalence`, `core_sync_gate`, `alpha_roundtrip`, benches, soak, mutation ratchet, census, workspace clippy beyond `clippy_core.sh`), `decision_replay`, mutation
batches (the new lines of this integration -- `waiver_required` / `needs_waiver`, the galaxy clone in `from_game` -- were not mutation-checked), `TI4_CORE_STRICT=1`, Playwright.

## 8. Remaining gates, consolidated

Dropped by decision (not open): as in `docs/INTEGRATION11.md` section 8. Items 1 to 11 of `docs/INTEGRATION9.md` section 6 stand unless named below.

1. **The next first gaps, by games stopped** (section 6): the action card played from the action menu or in a secondary (38 + 172 games), `MissingFact` (6 + 32), Z'eu (3 + 24, every Naalu game), `CombatUnported` 2, Integrated Economy in the invasion 2, Freelancers + Peace Accords 1.
2. **Shadow differences**: the production order against the invasion's begin (`--quick --shadow` step 397; int. 13: 3 games), the close order of a strategic action (int. 13: 27 games, not re-run here), Mitosis in the phase glue, the Winnu tactical mark, the Yssaril secondary.
   The shadow sweep (`--factions all ... --shadow`) was not re-run (not required).
3. **Three copies of `announce_staged_cards`** (G5's `kernel/fx_cards.rs`, the one in use; `driver/windows/status_finish.rs` with `Flow`; lane D's callers of the first): the same rows and events. Merge `status_finish`'s into `fx_cards` when the job flow can take a `Result`.
4. Open from integration 12/13: Scheming's two machines, the dispatch of the menu beyond F2e's list (Airo Shir Aur, Dannel of the Tenth, Mathis with the Warfare card), the tactical remainder (`CannonHits`, ship routes beyond five), turn flow (`ChaosMapping` remains only without the production link), action cards played inside another window.
5. Heavy gates still to run once a zero-bail playthrough is possible: golden and wide dual sweeps, `board_equivalence`, `core_sync_gate --strict`, `alpha_roundtrip --strict`, the benches, `census`, `soak`, the mutation ratchet and the mutation batches of every resume note since integration 9
   (this integration adds `waiver_required`, the galaxy clone), workspace clippy, `TI4_CORE_STRICT=1`.

## 9. Tip

`git log -1 core-integration-14` (the last commit is this document's). The private target dir was deleted.
