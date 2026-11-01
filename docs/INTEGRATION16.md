# core-integration-16: merge record (2026-11-01)

Branch `core-integration-16`, cut from `core-integration-15` @ `3ab5e6e64`. Two merges, one at a time, in the ordered list: lane N (`core-n-waiver`, Jae Mir Kan's waiver with a Nomad, the close order of
the follower window, War Machine in a research's pool) and lane P (`core-p-shadow2`, the shadow harness, the nested emissions of the aftermath chain, the ruling on S1). Lane Q (`core-q-research`) is NOT merged
here (integration 17). Not pushed. The live branch `nightly-fixes-2026-10-26` was not touched. Pure refactor: `RULES_VERSION`, serde, fixtures and fingerprints of the string engine are unchanged (no file of those was
edited; the new core-only payload is `Out::journal`, lane P's, never stored).

Read first: `docs/INTEGRATION15.md`, `docs/INTEGRATION14.md`, `docs/N_RESUME.md`, `docs/P_RESUME.md`, `docs/J_RESUME.md`, `docs/STRING_ENGINE_BUGS.md`.

## 1. Merges

| # | Branch | Tip merged | Merge commit | What it brings | Files in conflict |
|---|---|---|---|---|---|
| 1 | `core-n-waiver` (N; contains `core-k-actioncards`, already in int. 15, so only N's own commits come in) | `767e8d946` | `c0fe3dcf8` | N1 no Nomad gate in `facts.rs::waivers_of` (Jae Mir Kan's waiver is paid by the core; the Nomad's suite hears the staged `AGENT_EXHAUSTED`), N2 close order (see section 2), N3 a test (a ported waiver beside an unported one), N4 War Machine's bonus is no way to pay a research (`technology_secondary_affordable`, `Research::after_offers`) | 1 (`driver/windows/strategic/mod.rs`, the known one) |
| 2a | `core-p-shadow2` (P) | `d5e0e9c64` | `f44a237fb` | the shadow harness (phase / forced pick / idle-step / cut-group / reconcile glue), `Out::journal` (aftermath chain mirrors every emission of a sub-window), `CombatTexts::guns`, `component::announce_staged`, the chain's `Pending::nested` check, the S1 ruling (active player first, then initiative order: `Resolver::player_order`), the dual's rotated list | 0 |
| 2b | `core-p-shadow2` (P, final tip, asked for by the coordinator) | `f800ceb2f` | `10845659f` | clippy fix of a test, P_RESUME rows, `steps_idle` and `s1_in_error` tests, a `mutants:skip` reason, the SC test reaches the reroll question of the game as it plays now (bound 185 -> 195) | 0 (`git diff d5e0e9c64 core-p-shadow2 --stat`: `native_play.rs` tests/comment, `docs/P_RESUME.md`) |

After EVERY merge: `cargo check --workspace --all-targets` (without ti4-review, ti4-replayer, ti4-mlp, ti4-tensor, ti4-advisor) green, and `awk '/^fn settle_run/,/^}/' crates/ti4-engine/src/dual_window.rs | grep -ci neutral` = **0**
(after merge 1, 2a, 2b). The compile found no stale `Out { .. }` literal (P adds `journal`, N adds none) and no dropped registration. The registry-equality tests, the audit test and
`no_two_rules_of_the_core_table_share_a_slot` are green in the engine lib (section 6): no silent duplicate. P's merge was textually clean: no file was touched by both N and P (N: strategic/facts, strategic/mod,
kernel_env, strategy_cards, strategy_secondary + tests; P: harness, chain, resolver, fx, tactical, combat_live, reactions_core).

## 2. The one conflict: lane J's held follower window x lane N's close order

Both fixed the same string behaviour: when the followers left in the list after an answer are all skipped, the string ends the action in a step of its own, and that step starts with the reconcile, so what the answer
staged (`STRATEGY_TOKEN_SPENT`, the exhaustion Jae Mir Kan stages for a Nomad's suite, which may ASK the Nomad) is announced BEFORE `STRATEGIC_ACTION_COMPLETE`.

* Lane J (already in the base): `Held::holds` / `Out::held`. `SecondaryWin::step` ends the CALL of an answer that was not the last place of the list (`cut`) with the window still open when nobody who may follow is left; the
  next call skips them, closes the window and runs the `Finish` job. `live_step_inner` treats a held call as a step that is not finished, and the caller runs the reconcile (`live_reconcile`) before every call.
* Lane N (N2): in the same call, `SecondaryWin::step` ran `prelude::run_reconcile` before completing when `skip_ineligible` had skipped followers right after an answer.

Determination: J's mechanism subsumes N's. The case N2 fixes (followers skipped right after an answer) is exactly J's `cut && pending().is_none()` case, and J's answer-call returns before any skipping, so N's block is
only reached in the CLOSING call, after the caller's own reconcile: it ran a second reconcile that finds nothing staged (and could not ask anything the first did not). Where the reconcile happens is the caller's, as in the string
(the string's step start). The merged `step` therefore keeps J's early return and drops N's block (`before`, `skipped_after_answer`, the `run_reconcile` call): ONE behaviour, the string's. Proof by tests:

* N's two engine lockstep tests (`the_token_a_follower_spent_is_announced_before_the_action_completes_when_the_rest_are_skipped_like_the_string`, `jae_mir_kan_as_the_last_follower_asks_the_suite_before_the_action_completes_like_the_string`)
  compare the real `live_step` + `live_reconcile` against the string: green with J's hold alone (`strategic_live_tests` 30 passed, `strategy_` filter 195 passed), unchanged.
* J's two core tests (`an_answer_that_is_not_the_last_place_...`, `the_events_a_follow_staged_come_before_the_close_...`) green, unchanged.
* **One assertion changed**: N's core test `strategic_arms::what_the_last_answer_staged_is_announced_before_the_action_completes_when_the_rest_are_skipped` called `advance_secondary` ONCE and expected
  `["exhausted", "complete"]`. With J's hold the answer's call ends with the window open (`out.held`, nothing completed), so the first merged run failed on the order check (no `complete` yet). The test now drives the three calls a
  step is made of, as the caller does: the answer (asserting `out.held`), `advance_reconcile`, the closing `advance_secondary`; the expectation `["exhausted", "complete"]` is unchanged. This is a change of WHO runs the
  reconcile (the caller, as in the string), not of the order the test pins. The test still guards the order: with J's `cut` branch disabled (`&& false`, mutation checked) it fails together with J's two core tests (3 of 40 in `strategic_arms`).
* N's pre-fix check (N2 reverted) is replaced on the merged tree by the J-hold mutation above (core half only: the engine lockstep half of the mutation was not re-run; N's own note shows those tests fail when the close comes before the staged events).

N1 (`facts.rs::waivers_of` without the Nomad gate) and N4 (`technology_secondary_affordable` / `Research::after_offers` subtract `war_machine_bonus`) are independent of J and P and merged as they are; their tests (`strategic_arms`
38+, `research_e2` 5, engine `strategic_live_tests`) are green.

Commit of the resolution: `8c18a384b`.

## 3. Bugs found, and how each is guarded

| # | Where | What | Fix | Guard |
|---|---|---|---|---|
| B1 | **J x N** (section 2) | the same close order fixed twice: N's in-step reconcile became a redundant second reconcile in the closing call | N's block removed, J's hold kept | N's core test drives the three calls; N's engine lockstep tests and J's core tests unchanged; mutation of J's `cut` branch fails 3 tests |
| B2 | trip-wire `native_play::a_native_game_plays_a_whole_game_with_zero_gaps` | the quick game is a different game after N4 (a Technology secondary question the string never asks is gone) and the pins moved | re-measured, section 5 | the test itself |
| B3 | trip-wire `native_play::the_shadow_run_agrees_with_the_legacy_game_up_to_the_known_follower_window_of_the_technology_card` | P's pin (293 legacy steps, then step 448: native asks seat `c`, the string `d`) fired as written: that question IS N4's (seat `c` held War Machine's bonus as its only pool). The quick game now shadows 589 steps with no difference, then a new first difference at step 862 | the next first difference investigated and pinned (section 5) | the test itself |

No new rule bug of the merge was found: every one of the sweep's first differences (section 7) is a class lane P listed as open or a class that the merged games reach for the first time; none is caused by the join (checked: P's tip binary, the
same 96 games, differs from the merged tree in exactly three games, section 7.2).

## 4. S1

Lane P applied the user's ruling (ACTIVE PLAYER FIRST, then the other seats in INITIATIVE order) in the core's `Resolver::player_order`; the string stays unchanged and wrong (`docs/STRING_ENGINE_BUGS.md` S1 updated by P). The shadow
recognises S1 and reports it apart (`KNOWN STRING BUG ... (known string bug S1)`, excluded from the exit code): 9 of 96 games, 24 of 288. Nothing was added in this integration.

## 5. Edits of assertions, fixtures and harnesses (reasons)

| Where | Edit | Reason |
|---|---|---|
| `strategic_arms::what_the_last_answer_staged_is_announced_before_the_action_completes_when_the_rest_are_skipped` | drives answer + `advance_reconcile` + closing call (asserts `out.held` after the answer); same expectation | section 2 |
| `native_play` `a_native_game_plays_a_whole_game_with_zero_gaps` | steps 1,269 -> **1,165** (rounds 9, 26 phases), fingerprint `24b0d67462f4772b` -> **`74d5675ac754ac81`** (lane N's own fingerprint; `--twice` the same) | N4 removes the Technology-secondary question the string never asks of a seat with War Machine's bonus as its only pool: a different game (P's own ruling on S1 had moved it from 1,198 / `2e93df9fa00fa982` to 1,269 / `24b0d67462f4772b`) |
| `native_play` `..._up_to_the_known_follower_window_of_the_technology_card` -> `..._up_to_the_known_split_of_the_commitment_step` | 293 compared / step 448 -> **589 compared / step 862**, `[aftermath] questions [0]`: native `commit ground forces in 38`, legacy `<absent>` | the old pin's difference is N4's (see B3) and is gone. The new one was traced (`TI4_NATIVE_PLAY_TRACE`): after `SPACE_COMBAT_ENDED` the legacy step produces exactly the head of the native events (`SPACE_COMBAT_WON`, `INVASION_BEGAN`, `SPACE_COMBAT_RESOLVED`), uses none of the answers handed to it and asks nothing; the native step goes on to the commitment question and answers it. The string asks the commitment in its NEXT step: a split of the step the harness does not catch up (its catch-up loop steps the string again only over a step that SAYS nothing, `steps_idle`). Pinned the way integrations 11/14/15 pinned theirs; it fails the day the harness learns the split. It is the same shape as the `SP` class of section 7 (retreat announcements, hits, production, the research secondary) |
| `native_play` `the_round_the_asks_of_a_job_and_the_shadow_count_are_the_positions` | no edit (stays green: 61 compared) | -- |

No assertion was loosened.

## 6. Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/int16` (profile `lockstep` with debug assertions + overflow checks for the three core crates, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0` after the coordinator's request; deleted at the end)

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without the torch crates) after each merge | ok (N, P 2a, P 2b); `settle_run` neutral-side grep **0** each time |
| `cargo test -p ti4-engine --lib` plain | **5291 passed**, 0 failed, 7 ignored (int. 15: 5286; +N's lockstep tests, +P's) |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (WHOLE lib) | 5291 passed, 0 failed, 7 ignored |
| `ti4-engine-core --lib` | 477 passed |
| `ti4-engine-core-factions --lib`, `ti4-rule-bot-typed --lib` | 151, 1 passed |
| `ti4-engine-core-bridge --lib` (final tip) and `--test kernel_env --test play_facts --test menu_sources` | 122 (the first run, before the pins were re-measured: 2 pins failed, section 5), 3, 3, 2 passed |
| the D4 crate (`ti4-engine-core-tests`, `lockstep` profile, the flags of `tools/engine_core/d4/run_core_tests.sh`, run in this worktree) | 154 result lines, **1958 passed, 0 failed** (int. 15: 1949; run before merge 2b, which touches only bridge tests and docs) |
| `check_layers.py core`, `lint_core.sh` | 574 files, 0 violations; 576 files, 0 hits |
| `tools/engine_core/clippy_core.sh` | ok (engine 41 = baseline 41; core crates 0 warnings) |
| `native_play.sh` | section 7 |
| usage | five-hour window 5 % at the start, 10 % at the end (weekly figure not a criterion) |

Not run (user rule): `end_of_d1.sh` and its steps (golden / wide dual, `board_equivalence`, `core_sync_gate`, `alpha_roundtrip`, benches, soak, census, ratchet, workspace clippy beyond `clippy_core.sh`), mutation batches of
lanes N and P (their resume notes carry their own numbers), `TI4_CORE_STRICT=1`, Playwright.

## 7. The native playthrough and the shadow (`tools/engine_core/native_play.sh`, gate profile, `te` card set)

### 7.1 The whole game, plain

| run | games | finished with zero gaps | native steps | notes |
|---|---|---|---|---|
| `--quick` (sol, 4 players, seed 1) | 1 | **1** | 1,165, 9 rounds, fingerprint `74d5675ac754ac81` | `--twice`: the same fingerprint |
| `--factions all --players 3,4,5,6 --seeds 1` | 96 | **96** (int. 15: 94) | 103,150 | the two Doctor Sucaban research gaps of int. 15 (sol 6p, ghost 5p) are gone: they were N4's cause (War Machine's bonus in the pool, `PayStep::took == None`) |
| `--factions all --players 3,4,5,6 --first-seed 2 --seeds 3` | 288 | **287** (int. 15: 285) | 297,968 | the one gap: `UnportedAftermath(CombatUnported)` (yin 6p seed 3, step 568; lane L's 2d family, unchanged); the two `StrategyAbility` stops of int. 15 (l1z1x 4p seed 3, naalu 3p seed 4) are gone |

### 7.2 The shadow (`--shadow`)

`--quick --shadow`: **589 legacy steps compared with no difference**, then step 862 (class SP, section 5).

`--factions all --players 3,4,5,6 --seeds 1 --shadow` (96 games): **no game stops at a gap, 59 play the whole game with no difference**, 9 stop at the known string bug S1 (excluded), **28 show a first difference**
(P's tip, same binary recipe, same games: 58 clean + 2 gaps + 9 S1 + 27 differences). Games that differ between P's tip and the merged tree: sol 4p (P: step 448, N4's question; merged: 862, class SP), ghost 5p (P: gap, now clean),
sol 6p (P: gap, now a difference of class AW at step 1026). Nothing that was clean in P's tip differs in the merge.

`--first-seed 2 --seeds 3 --shadow` (288 games): **0 gaps** (P: 1), **196 clean** (P: 194), 24 S1 (P: 24), **68 first differences** (P: 69).

Classes of the first differences (my own grouping of the printed signatures; counts 96 / 288):

| code | 96 / 288 | first difference | what is known |
|---|---|---|---|
| SP | 6 / 8 | `questions [0]: native <question> | legacy <absent>`: `commit ground forces in N`, `announce a retreat from N`, `assign a hit`, `produce in N`, `spend a strategy token and 4 resources to research` (cabal 4p: the Technology secondary of seat `c`, the string's step ends before asking it) (and in the 288 set `state combat_presentation.barrage_dice` once) | the string's step ends before the question and asks it in its NEXT step (traced for the quick game, section 5): a split of the step in the harness' grouping, not shown to be a difference of the game. The cabal 4p case is P's "Cabal secondary" leftover (the state: seat `c` Xxcha with one unexhausted planet and one trade good cannot pay 4: P's list, not N4's cause), NOT traced further |
| OT | 6 / 6 | `native "decline" | legacy "leader:sardakk:sardakkagent_mark:SYSTEM_ACTIVATED:after"` / `ability:winnu:reclamation_tactical:...` | P's "mark abilities the string offers as choices"; plus OT82 (sol 5p: option `82a`) 1 / 1 |
| RW | 3 / 12 | `script wanted "done_moving" | "decline" | "munitions" but ... offered a reaction / [stay, retreat]` | a reaction or retreat question the string asks in a step the native splits otherwise (P's "wanted done_moving but offered reaction:...:SYSTEM_ACTIVATED:after" family, about 12 games in P's note); not traced |
| RD | 4 / 6 | `script wanted "" but ... offered ["remove|0", ...] / ["0|2|4", ...]` | P's "warfare redistribution after a removal question"; not traced |
| TG | 2 / 10 | `native TRADE_GOODS_GAINED | legacy SYSTEM_ACTIVATED / MOVEMENT_FINISHED` (and the reverse order) | P's "staged TRADE_GOODS_GAINED at the activation"; not traced |
| EV | 2 / 9 | `COMMAND_TOKEN_PLACED | SPACE_COMBAT_RESOLVED` (retreat token, 6 of the 288), `TURN_BEGAN | STRATEGY_TOKEN_SPENT`, `SHIP_DESTROYED | HITS_TO_ASSIGN`, `STRATEGY_TOKEN_SPENT | PRODUCTION_RESOLVED` | P's "retreat command token in the aftermath" and others; not traced |
| AW | 3 / 4 | `reaction:...:ACTION_CARD_PLAYED:when` offered in another seat order (the card player's window) | P's "reaction windows of `ACTION_CARD_PLAYED:when`: may be a second S1-like rule"; not traced. Includes sol 6p (formerly a gap) |
| OTHER | 1 / 12 | 96 set: ghost 4p, `state combat_presentation.barrage_dice`. 288 set: `reaction:...:STRATEGY_PHASE_BEGAN:after` in another seat order (3 games: letnev 6p, l1z1x 6p, yin 6p; the strategy phase keeps speaker + seating by P's ruling, so this may be a second S1-like order, NOT traced), reaction-before-move windows (sardakk, winnu 4p), Yssaril agent / Pillage, cabal 6p retreat | not traced |
| S1 | 9 / 24 | see section 4 | known string bug, excluded |

Per game (96 set; C = clean to the end, `n CODE` = first difference at native step n, S = S1 stop):

| faction | 3 players | 4 players | 5 players | 6 players |
|---|---|---|---|---|
| sol | C | 862 SP | 867 OT82 | 1026 AW |
| hacan | C | C | C | C |
| letnev | 684 RD | 289 RD | C | S |
| xxcha | 700 RW | C | C | C |
| jolnar | C | C | 369 SP | C |
| l1z1x | C | S | C | C |
| arborec | C | 652 EV | C | C |
| mentak | C | C | 1012 TG | C |
| muaat | 514 TG | C | C | C |
| naalu | C | C | C | S |
| nekro | C | S | C | 1100 SP |
| saar | C | 303 SP | C | S |
| sardakk | 230 OT | 433 RW | 333 OT | 280 OT |
| winnu | 500 OT | 769 OT | C | 603 OT |
| yin | C | C | C | C |
| yssaril | C | C | S | C |
| ghost | C | 548 OTHER | C | C |
| argent | C | 464 RD | C | S |
| cabal | 766 EV | 881 SP | C | C |
| empyrean | C | S | 557 RD | C |
| mahact | C | 1002 AW | C | 1172 AW |
| naaz | C | C | C | C |
| nomad | C | 887 RW | S | C |
| titans | C | C | 910 SP | C |

(`OTHER` for ghost 4p, step 548: `state combat_presentation.barrage_dice[2].player` native `a`, legacy `b`, at the closing menu; not traced.)

## 8. Remaining gates, consolidated

Dropped by decision (not open): as in `docs/INTEGRATION11.md` section 8. Lane Q is integration 17.

1. **The shadow classes of section 7.2 are the work list of the next shadow lane**; the biggest is SP (the string's step splits where the harness groups), then OT (mark abilities), RW, RD, TG, EV, AW (and the `STRATEGY_PHASE_BEGAN` order). The shadow can only compare what plays: once these open, it will meet the differences behind them.
   The cabal 4p Technology-secondary case (`spend a strategy token and 4 resources to research`, native asks seat `c`, the string does not) is worth one trace: it is the only first difference that may be an eligibility difference of the core rather than a split.
2. `CombatUnported` (yin 6p seed 3): the combat texts do not name a roller form (L 2d family).
3. Open from integration 12-15 and not touched here: the other staged-event announcers with a string twin (`driver/agenda/ground.rs`, `turn/component.rs::flush_staged_events`; the card announcement is in three places on purpose), Scheming's two machines, Mathis / Dannel corners of
   Thunder's Edge (`TurnGap::HeroFight`), the display facts of card picks, no lockstep of Signal Jamming's effect through the menu with a map. Lane N's note: the Ssruu / borrowed-agent variant of the waiver (`exhaust_ready_agent` for the Mahact agent held by its own seat only) was not exercised.
4. Heavy gates still to run (a zero-bail playthrough of the 96 games is now met: 96 of 96 plain, 287 of 288 in the second set): golden and wide dual sweeps, `board_equivalence`, `core_sync_gate --strict`, `alpha_roundtrip --strict`, the benches, `census`, `soak`, the mutation ratchet and the mutation batches of every
   resume note since integration 9 (plus lane N's and lane P's), workspace clippy, `TI4_CORE_STRICT=1`.

## 9. Tip

`git log -1 core-integration-16` (the last commit is this document's). The private target dir and the logs under `/root/deploytmp/int16*` were deleted after the last check.
