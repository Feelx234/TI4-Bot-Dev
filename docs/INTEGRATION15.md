# core-integration-15: merge record (2026-11-01)

Branch `core-integration-15`, cut from `core-integration-14` @ `4766eafcc`. Three merges, one at a time, in the ordered list: lane K (`core-k-actioncards`, the action card from the menu), lane L (`core-l-leftovers`, the leftovers of
the native playthrough), lane J (`core-j-shadow`, the whole-game shadow differences). Lane N (`core-n-waiver`, Jae Mir Kan's waiver with a Nomad) is NOT merged here (integration 16). Not pushed. The live branch
`nightly-fixes-2026-10-26` was not touched. Pure refactor: `RULES_VERSION`, serde, fixtures and fingerprints of the string engine are unchanged (no file of those was edited; `Out::held`, `Held::holds`, `SecondaryWin::{cut, held}` and
`EThen::Enforced` are core-only payload of windows, never stored).

Read first: `docs/INTEGRATION14.md`, `docs/J_RESUME.md`, `docs/K_RESUME.md`, `docs/L_RESUME.md`, `docs/STRING_ENGINE_BUGS.md` (S1).

## 1. Merges

| # | Branch | Tip merged | Merge commit | What it brings | Files in conflict |
|---|---|---|---|---|---|
| 1 | `core-k-actioncards` (K) | `1e29704b6` | `d38572233` | Signal Jamming on the action menu (the whole gap `Menu(ActionCard)`), Summit with a Mahact fleet pool, Intercept / Courageous To The End at Naaz / Nekro tables (`ships_of` family moved to `kernel::read`), Refit Troops for a Naaz seat | 0 (K is cut from int. 14) |
| 2 | `core-l-leftovers` (L) | `4c1039be9` | `85615cf05` | Z'eu, Airo Shir Aur, Dannel of the Tenth, Mathis with the TE Warfare primary, Freelancers (`EThen::Enforced`), Integrated Economy in the invasion, Argent / Empyrean / L1Z1X-agent facts, combat texts for Reveal Prototype, `status_finish::announce_staged_cards` as a wrapper over `kernel/fx_cards.rs` | 1 |
| 3 | `core-j-shadow` (J, cut from int. 13) | `489c1a43f` | `bd33a7ad9` | held follower windows (`Held::holds`, `Out::held`), the space cannon offense mirrored in the tactical head, marks of a free tactical action, nested-question mirror of the aftermath chain, `inline::emit_typed` announcing staged card moves (`emit_typed_bare`), the hyperlane loader fix, the catch-up of the shadow harness, `docs/STRING_ENGINE_BUGS.md` | 3 |

Later commits are the fixes the checks found (section 3). After EVERY merge: `cargo check --workspace --all-targets` (without ti4-review, ti4-replayer, ti4-mlp, ti4-tensor, ti4-advisor) green (the J merge needed no `held: false`
repair: the compiler found no other lane's `Out { .. }` literal) and `awk '/^fn settle_run/,/^}/' crates/ti4-engine/src/dual_window.rs | grep -ci neutral` = **0** (after K, L, J, and at the end).

## 2. Conflicts and how they were resolved (keep both sides)

| File | Conflict | Resolution |
|---|---|---|
| `ti4-engine-core-tests/tests/action_dispatch.rs` (L) | K's two Signal Jamming menu tests against L's `combat_texts` helper and Airo Shir Aur test, added at the same place | both, K's tests first |
| `driver/windows/status_finish.rs` (J) | L turned `announce_staged_cards` into a wrapper (`Result` -> `Flow`) of `kernel::fx_cards::announce_staged_cards`; J rewrote the old body with `emit_typed_bare` | L's wrapper (the one body); J's `emit_and_announce` = `emit_typed` stays; `emit_typed_bare` import dropped from the file. No double announcement: see the join point (b) below |
| `ti4-engine/src/silent4_live_tests.rs` (J) | the Spy Net lockstep helper `prelude_both`: G5's version asserted "no `cards:staged:*` row left, both sides", J's counted the announced `ACTION_CARD_TAKEN` of both sides | both: zero rows on both sides AND equal announced counts ("each card taken is announced by both sides, once": a double announcement of the two callers would show here) |
| `ti4-engine-core-bridge/src/native_play.rs` (J), 3 hunks | (1) the hyperlane loader fix, written twice (int. 14 B3: `galaxy.map(apply_to_galaxy)` into `Sources`; J cause 6: `ok_or_else` + `apply_to_galaxy`); (2)(3) the quick game's counts (K: 100 steps; J: 93) | (1) ONE copy (int. 14's; the `ok_or_else` is dead, `from_game` returns early on a missing galaxy) with BOTH regression tests (`the_core_map_of_a_hyperlane_game_follows_the_printed_lanes_from_the_first_step`, `a_core_loaded_from_an_unstepped_game_knows_the_printed_lanes_of_the_hyperlane_map`); (2)(3) HEAD's text, then every number re-measured (section 5) |

Auto-merged without conflict but checked by reading (the brief's list): `driver/live.rs`, `kernel/stage.rs`, `driver/turn/inline.rs`, `windows/chain.rs`, `windows/tactical/*`, `windows/strategic/mod.rs`, `ti4-engine live/mod.rs`
(J touched them only around `Held::holds` / `Out::held` / the mirror of the head; none overlaps K's or L's hunks). `native_play.rs`'s pins of K and J: section 5.

Join points asked for:
(a) `Out { .. }` literals (`held: false`), `Held::holds`, `hooks::marks` `pub(super)`, `SecondaryWin::{cut, held}`: the workspace compiles; no other lane constructs them.
(b) **`announce_staged_cards` x `emit_typed`**: three actors announce the staged card moves: G5's hook in `Sx::emit_mirrored` (after every TOP-LEVEL typed event), J's explicit call in `inline::emit_typed` (every typed event of the turn flow, nested ones included) and L's wrapper in `status_finish.rs` (the same body, for component actions). All three drain the same `CardsStaged` marks first, so the second caller finds nothing: no double announcement (checked by the lockstep helper above and `turn_jobs2::a_card_move_staged_earlier_in_the_action_is_announced_right_after_the_typed_event_that_ends_it`, which asserts ONE card on the pile). The join was NOT free: G5's hook fired for EVERY `emit_mirrored`, including the staged events the string announces from its pending list, which brought back the Yssaril difference lane J had fixed: see B1.
(c) the hyperlane loader: one copy, two tests (section 2).
(d) K moved the `ships_of` family to `kernel::read`: L does not use it; compiles.
(e) the `no_two_rules_of_the_core_table_share_a_slot` test, the audit test and the registry-equality tests are green in the engine lib (section 7); no silent duplicate was found (`announce_staged_cards` was the only function written three times, and it is one body).

## 3. Bugs and breaks found, and how each is guarded

"Fails before": R = seen to fail on the code before the fix.

| # | Where | What | Fix | Guard |
|---|---|---|---|---|
| B1 | **G5 hook x J (`Sx::emit_mirrored`, `driver/turn/prelude.rs`)** | The shadow of the four Yssaril tables (3 to 6 players) stopped at the secondary of the first Scheming follow: native `ACTION_CARD_DISCARDED` right after the follower's `STRATEGY_TOKEN_SPENT`, legacy `STRATEGY_SECONDARY_DECLINED`/`FOLLOWED`. The staged events a step announces from its pending list (`prelude::announce_staged`, which the follower windows also run) went through `emit_mirrored`, whose G5 hook announces the staged card moves after a top-level event; in the string those events are not `Live::emit_typed` events, so the Scheming discard waits for the next typed one (`ACTION_COMPLETED`, lane J cause 5). Each lane was right alone; the join was wrong. Found by the merged shadow sweep (first difference class SD, 4 games); with the hook off the four games moved on to the next classes | `Sx::emit_mirrored_bare` (no hook); `emit_mirrored` = bare + hook (all other callers unchanged); `prelude::announce_staged` uses the bare form. The other staged-event announcers (`driver/agenda/ground.rs`, `turn/component.rs::flush_staged_events`) have the same string twin but no evidence of a difference: left as they are (section 8) | `turn_jobs2::the_events_the_reconcile_announces_from_the_pending_list_do_not_announce_the_card_moves_staged_meanwhile` (R: the row is announced at the staged event) |
| B1b | test fixture (`turn_jobs2`) | J's test `a_card_move_staged_earlier_in_the_action_is_announced_right_after_the_typed_event_that_ends_it` wrote the staged row by hand with `Atom::Num(0)` as the "from" of a discard; G5's decoder (`fx_cards::decode`, merged later) accepts `Atom::Absent` there only and DROPS the row (its test "a malformed row is dropped"), so the test failed on the merged tree with no discard at all | both tests stage the move with the real function (`fx::discard_action_card_staged` after giving the seat the card): the fixture cannot drift from the consumer. The assertions are unchanged | the two tests |
| B2 | **Exploration Probe played from the menu or a window did nothing** (`rules/action_cards/announce.rs::env_of`) | The facts a card reads (`Facts::map`) were never set by the core's own env (`..Facts::default()`); only the string-side adapter (`action_cards_view`) set it. Exploration Probe (and Experimental Battlestation) then looked at the systems of the holder's ships alone, found no frontier token next to them, explored nothing and still discarded the card, silently. The shadow's first difference in 12 of the 96 games (every 6-player table: the 6pOuterRim map): `exploration_decks.FRONTIER[0]` native `minent`, legacy `lc2` | `map: rt.read().0.loaded.has(Slice::Map)` (a window lent the map) | `native_play::an_exploration_probe_played_from_the_menu_explores_the_frontier_token_next_to_the_seats_ships` (R: shadow of sol 6p seed 1 stops at step 270 with that difference) |
| B3 | trip-wire `native_play` | the three pins of the quick game and its shadow | re-measured, section 5 | the tests themselves |

Not fixed, found (section 6): the nested events of a reaction card in the production window of the aftermath chain are not mirrored (pinned as a known difference), and the rest of the shadow matrix.

## 4. S1 (string initiative order in action-phase reaction windows)

Treated as a known string bug as instructed: `docs/STRING_ENGINE_BUGS.md` S1 (from lane J) is in the tree, J recorded it there and nowhere in the harness (there is no allow-list in the shadow: a difference is reported as the first difference). I added nothing to the harness. Neither the core nor the string changed for it. In the merged shadow sweep two games show it as the first difference with the signature the note describes (saar 6p step 38: `script wanted leader:l1z1x... but a was offered leader:saar...`; argent 6p step 469: the options of `COMBAT_ROUND_STARTED:after` start with `leader:letnev...` natively and `reaction:arborec...` in the string), and one more with a probable same cause that was not investigated (sardakk 4p step 410: the string offers `reaction:xxcha:SYSTEM_ACTIVATED:after` where the script wanted `done_moving`).

## 5. Edits of assertions, fixtures and harnesses (reasons)

| Where | Edit | Reason |
|---|---|---|
| `native_play` `a_native_game_plays_a_whole_game_with_zero_gaps` (K) | steps 1,174 -> **1,198** (rounds 9, 26 phases, fingerprint `2e93df9fa00fa982` unchanged, `--twice` the same) | lane J holds the follower windows open: three more windows close in a step of their own (J: 90 -> 93 in the old game); +24 steps over the whole game, the same position and fingerprint |
| `native_play` `the_round_the_asks_of_a_job_and_the_shadow_count_are_the_positions` | shadow of 100 native steps: compared 65 -> 66 | the same cause (J's held windows) |
| `native_play` `..._up_to_the_known_production_order_difference` | replaced by `..._up_to_the_known_nested_events_of_a_reaction_card_in_the_production_window` (282 legacy steps compared, then step 408 `events [3]`: native `INVASION_RESOLVED`, legacy `ACTION_CARD_PLAYED`) | lane J fixed the production order (its cause 2, the space cannon offense of the head), so the int. 14 pin fired as written. The next first difference of the quick game is a NEW one, investigated: section 6, NE. Pinned the way integration 11 / 14 pinned theirs: it fails the day the difference is fixed |
| `silent4_live_tests::prelude_both`, `turn_jobs2` (2 tests) | merged helper; staging by the real function | section 2 and B1b |

No assertion was loosened.

## 6. The native playthrough and the shadow (`tools/engine_core/native_play.sh`, gate profile, `te` card set)

### 6.1 The whole game, plain

`--quick` (sol, 4 players, seed 1): **finished, zero gaps, 9 rounds, 1,198 native steps**, 26 phases, fingerprint `2e93df9fa00fa982` (K alone: 1,174 steps, the same fingerprint; the +24 steps are lane J's held follower windows). `--quick --twice`: the same fingerprint both times.

`--factions all --players 3,4,5,6 --seeds 1` (96 games, 96,987 native steps): **94 finish with zero gaps** (int. 14: 45; L alone: 51; K alone: 83). The two that stop are the same family of gap, not an action card: `TurnGap(StrategyAbility)` in the Technology research window with Doctor Sucaban's infantry trade (sol 6p at step 945, ghost 5p at step 794). They became reachable only after fix B2 changed the path of the games (before it the same two games ended clean at 1,070 / 934 steps); the stop is `Research::after_step` finding the pay step without a worth (`PayStep::took == None`: found with a temporary print, not fixed, section 8). First gap per game, native steps (FIN finished clean, GAP stopped):

| faction | 3 players | 4 players | 5 players | 6 players |
|---|---|---|---|---|
| sol | 827 FIN | 1198 FIN | 919 FIN | 945 GAP |
| hacan | 1048 FIN | 1379 FIN | 1370 FIN | 1518 FIN |
| letnev | 772 FIN | 1096 FIN | 987 FIN | 1088 FIN |
| xxcha | 849 FIN | 981 FIN | 883 FIN | 1054 FIN |
| jolnar | 740 FIN | 1047 FIN | 915 FIN | 1092 FIN |
| l1z1x | 713 FIN | 1009 FIN | 1053 FIN | 1148 FIN |
| arborec | 816 FIN | 1068 FIN | 1067 FIN | 1189 FIN |
| mentak | 737 FIN | 1106 FIN | 1003 FIN | 1188 FIN |
| muaat | 814 FIN | 1216 FIN | 1030 FIN | 1180 FIN |
| naalu | 822 FIN | 1035 FIN | 991 FIN | 1134 FIN |
| nekro | 748 FIN | 1172 FIN | 1082 FIN | 1146 FIN |
| saar | 651 FIN | 1168 FIN | 1085 FIN | 1183 FIN |
| sardakk | 637 FIN | 1119 FIN | 893 FIN | 1113 FIN |
| winnu | 652 FIN | 1189 FIN | 838 FIN | 1358 FIN |
| yin | 744 FIN | 985 FIN | 1039 FIN | 1184 FIN |
| yssaril | 754 FIN | 1115 FIN | 1095 FIN | 1054 FIN |
| ghost | 728 FIN | 1218 FIN | 794 GAP | 1173 FIN |
| argent | 662 FIN | 1151 FIN | 918 FIN | 1248 FIN |
| cabal | 774 FIN | 1110 FIN | 1045 FIN | 1113 FIN |
| empyrean | 637 FIN | 1162 FIN | 1009 FIN | 1082 FIN |
| mahact | 661 FIN | 1201 FIN | 1085 FIN | 1179 FIN |
| naaz | 754 FIN | 1195 FIN | 955 FIN | 1181 FIN |
| nomad | 651 FIN | 1261 FIN | 975 FIN | 1072 FIN |
| titans | 733 FIN | 1064 FIN | 1005 FIN | 1160 FIN |

`--first-seed 2 --seeds 3` (288 games, 280,494 native steps): **285 finish with zero gaps** (int. 14: 59; K alone: 208). The three that stop: `StrategyAbility` in a secondary (l1z1x 4p seed 3 at step 715, naalu 3p seed 4 at step 354: both stopped there in the int. 14 + K + L sweeps too, the Technology secondary's research; K's note blamed Jae Mir Kan's waiver with a Nomad for this stop in its own deeper sweep, which is lane N and does not apply to a table without a Nomad: the research window family above is the likelier cause for these two, NOT verified) and `UnportedAftermath(CombatUnported)` (yin 6p seed 3, step 532, L's 2d family: a fight whose roller form the texts do not name).

No game of either sweep stops at `Menu(ActionCard)`, `MissingFact`, `Unrouted` (Z'eu etc.), `IntegratedEconomy`, `Freelancers`, Peace Accords or the status close.

### 6.2 The shadow (`--shadow`)

`--quick --shadow`: **282 legacy steps compared with no difference**, then step 408 (tactical + aftermath) `events [3]`: native `INVASION_RESOLVED`, legacy `ACTION_CARD_PLAYED` (class NE below). Int. 14: 275 steps, then the production order (lane J fixed it).

`--factions all --players 3,4,5,6 --seeds 1 --shadow` (96 games, 21,942 legacy steps compared before the first difference): **no game stops at a gap** (all 96 "finish with zero gaps" up to their first difference) and **every game reaches a first difference** (J alone: 1 of 96, because its games stopped at gaps long before; now they run 76 to 649 steps first). Class per game, with the step of the first difference (codes below the table):

| faction | 3 players | 4 players | 5 players | 6 players |
|---|---|---|---|---|
| sol | 153 PB | 408 NE | 201 PB | 335 PB |
| hacan | 459 NE | 548 AC | 574 NE | 482 PB |
| letnev | 302 SC | 207 PB | 291 PB | 306 AC |
| xxcha | 330 PB | 282 AC | 381 PB | 495 PB |
| jolnar | 341 PB | 199 PB | 342 NE | 568 PB |
| l1z1x | 254 PB | 292 PB | 173 NE | 525 PB |
| arborec | 598 PB | 471 PB | 464 PB | 422 PB |
| mentak | 498 PB | 428 PB | 178 SC | 491 PB |
| muaat | 221 TG | 511 PB | 501 PB | 649 PB |
| naalu | 247 PB | 396 PB | 313 PB | 374 PB |
| nekro | 96 PB | 338 PB | 176 PB | 367 PB |
| saar | 427 PB | 282 OT | 76 PB | 38 S1 |
| sardakk | 166 TG | 410 S1 | 90 PB | 266 OT |
| winnu | 147 PB | 333 PB | 346 AC | 529 PB |
| yin | 318 PB | 188 PB | 311 PB | 404 AC |
| yssaril | 271 PB | 207 PB | 431 NE | 126 PB |
| ghost | 148 PB | 341 PB | 79 PB | 402 PB |
| argent | 246 PB | 324 NE | 315 PB | 469 S1 |
| cabal | 569 TG | 183 PB | 181 PB | 467 PB |
| empyrean | 141 PB | 328 PB | 338 AC | 297 AC |
| mahact | 188 PB | 360 NE | 363 PB | 280 AC |
| naaz | 244 TG | 253 PB | 361 AC | 531 PB |
| nomad | 406 NE | 196 PB | 451 PB | 337 PB |
| titans | 415 PB | 316 AC | 178 SC | 503 PB |

| code | games | first difference | what is known |
|---|---|---|---|
| PB | 65 | the phase boundary: `[phase] events [k]: native STATUS_PHASE_BEGAN / ROUND_BEGAN / STRATEGY_PHASE_ENDED / TRADE_GOODS_GAINED | legacy <end after k>` (`phase`, `tokens+phase`, `ask.draft+phase`, `ask.draft+ask.draft`) | the native `phase` step runs on to the next phase's first events (`ROUND_BEGAN`, `STRATEGY_PHASE_BEGAN`, the draft's trade goods) where the string's step ends before them. Traced for sol 3p and letnev 5p; the quick game crosses 3 phase boundaries without a difference, so it depends on something in the closing step (the redistribution questions of the status phase answered in the same step, the last pass). Probably the shadow harness' grouping (`J` catch-up applies only when the native group asks something) but NOT shown to be; not investigated further |
| AC | 10 | `[tactical+aftermath+tactical+ask.action_menu] events [3]: native <end after 3> | legacy ACTION_COMPLETED` | the string asks the reaction card at `after ACTION_COMPLETED` (Master Plan: `TURN_RETAINED`) inside the step that answers `done_moving`; the native group ends after `TACTICAL_ACTION_COMPLETE` and the finish comes in a later step. Traced for xxcha 4p; not investigated further |
| NE | 9 | `events [3]: native INVASION_RESOLVED / ANTI_FIGHTER_BARRAGE_STARTED | legacy ACTION_CARD_PLAYED` | **root cause found** (the quick game): a reaction card (War Machine at `after PRODUCTION_USED`) is played by the core (hand, discard pile, round mark all right) and announced when the window of the event closes, but the aftermath chain mirrors only the top-level emissions of a sub-window (`Out::emitted`); the events a reaction emits inside another (`ACTION_CARD_PLAYED`, `ACTION_CARD_DISCARDED`) are in the resolver's journal only. Not fixed: `Sx::emit_with` would have to push every `Emit` line since its mark (the string's `mirror_timing_log`), a change of the meaning of `Out::emitted` for every window (dual harnesses read it). The same hole is behind the AC class' card events probably. Pinned (section 5) |
| TG | 4 | `[turn_prelude+ask.action_menu] events [k]: native TRADE_GOODS_GAINED | legacy STRATEGIC_ACTION_BEGAN / TACTICAL_ACTION_BEGAN` | the gains the turn prelude announces come earlier natively; not investigated |
| SC | 3 | `[tactical] questions [1].prompt: native "reroll die 1 of space_cannon" | legacy "reroll die 1 of pds"` | the text of a die of a space cannon offense in a reroll question (Fire Team / Scramble Frequency): the native names the roll, the string the unit; not investigated |
| S1 | 3 | see section 4 | 2 certain, 1 probable |
| OT | 2 | saar 4p step 282 (`aftermath`: the core asks `produce in 35 (5 left)`, the string asks nothing), sardakk 6p step 266 (the string offers `leader:sardakk:sardakkagent_mark:SYSTEM_ACTIVATED:after`, the core only `decline`) | not investigated |

Fixed by this integration (they were the first difference before): SD (4 games, B1: the Yssaril Scheming discard), XP (12 games, B2: Exploration Probe). The first sweep of the merged tree (before both fixes) had PB 52, XP 12, AC 9, NE 8, TG 4, SD 4, SC 3, S1 3, OT 1; the games that escaped XP and SD stop later, mostly at PB.

Native steps by arm (96 plain games): turn prelude 16,469; action menu 17,045; tactical 19,226; secondary 12,426; aftermath 6,821; tokens 7,026; draft 5,388; closing menu 4,791; phase 2,488; trade 2,273; scoring 1,148; event scoring 918; status 860; strategy announce 96; starting tech 12.
By island: M1 trade 2,273, M5 aftermath 6,821, M6 tactical 19,226, B turn flow 29,244, C status 9,952, E strategic action 12,426 (2 gaps), F menu and leaders 17,045; M2, M3, M4, D agenda: "not reached" (those islands count steps of their own windows, which the native loop runs inside the arms above).


## 7. Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/int15` (profile `lockstep` with debug assertions on for the three core crates, `CARGO_PROFILE_DEV_DEBUG=0`; deleted at the end)

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without ti4-review, replayer, mlp, tensor, advisor) after each merge | ok (K, L, J); `settle_run` neutral-side grep 0 each time |
| `cargo test -p ti4-engine --lib` plain (final tip) | **5286 passed**, 0 failed, 7 ignored (int. 14: 5279; +5 L, +2 K) |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (WHOLE lib) | 5286 passed, 0 failed, 7 ignored |
| `ti4-engine-core --lib`, `-factions --lib`, `ti4-rule-bot-typed --lib` | 477, 151, 1 passed |
| `ti4-engine-core-bridge --lib` and `--test kernel_env --test play_facts --test menu_sources` | 111 (the first run: 3 pins failed, section 5), 3, 3, 2 passed |
| the D4 crate (`ti4-engine-core-tests`, `lockstep` profile, debug assertions + overflow checks on for the three core crates, the flags of `tools/engine_core/d4/run_core_tests.sh`) | 154 result lines (153 test binaries + the crate's own): **1949 passed, 0 failed** (int. 14: 1925) |
| `check_layers.py core`, `lint_core.sh` | 574 files, 0 violations; 576 files, 0 hits |
| `tools/engine_core/clippy_core.sh` | ok (engine 41 = baseline 41; core crates 0 warnings) |
| `native_play.sh` | section 6 |
| the usage | five-hour window under 12 % at the start and 0 % at the end (it had reset) |

The D4 crate was run in this worktree with the script's flags, not through the script's scratch worktree. Two test batches were killed and re-run (the first complete batch ran before B2; the last batch above ran on the final code, including the plain engine lib and the dual).
Not run (user rule): `end_of_d1.sh` and its steps (golden / wide dual, `board_equivalence`, `core_sync_gate`, `alpha_roundtrip`, benches, soak, census, ratchet, workspace clippy beyond `clippy_core.sh`), `decision_replay`, mutation batches (the new lines: `emit_mirrored_bare`, `Facts::map` in `env_of`, the merged helper, were not mutation-checked), `TI4_CORE_STRICT=1`, Playwright.


## 8. Remaining gates, consolidated

Dropped by decision (not open): as in `docs/INTEGRATION11.md` section 8. Lane N (Jae Mir Kan's waiver with a Nomad, `WaiverEffect::Unported` in `strategic/facts.rs`) is integration 16.

1. **The shadow matrix of section 6.2 is the work list for the next lane** (every game of the 96 shows a first difference): PB 65 (phase boundary; first decide whether the harness or the core is wrong), AC 10, NE 9 (root cause known: mirror the nested emissions of the sub-windows of the aftermath chain), TG 4, SC 3, S1 3 (the user's ruling on `docs/STRING_ENGINE_BUGS.md` S1 is pending), OT 2. The shadow can only compare what plays: once these open, the sweep will meet the differences behind them.
2. **The Technology research window with Doctor Sucaban** (`Research::after_step`, `PayStep::took == None`; `TurnGap::StrategyAbility`): 2 of 96 and (probably) 2 of 288 games. The join of H4 x E2 x G5 (int. 14 B4) has a path nobody tested: the free or paid research with the infantry trade answered until the bill has no worth. Needs a reproduction of the sol 6p seed 1 position at step 946 (`TI4_NATIVE_PLAY_DUMP`).
3. `CombatUnported` (yin 6p seed 3): the combat texts do not name a roller form (L 2d family).
4. The other staged-event announcers with the same string twin as `prelude::announce_staged` (`driver/agenda/ground.rs`, `turn/component.rs::flush_staged_events`) still go through `emit_mirrored` (with G5's hook): no difference seen, none checked. The card announcement is now in three places on purpose (the hook for top-level events, `inline::emit_typed` for the turn flow, `status_finish::announce_staged_cards` for the component actions); they drain the same marks. If one of them is removed, run `silent4_live_tests` in the dual profile and the Yssaril shadow (`--factions yssaril --players 3,4,5,6 --shadow`).
5. Open from integration 12/13/14 and not touched here: Scheming's two machines, Mathis / Dannel corners of Thunder's Edge (`TurnGap::HeroFight`), the display facts of card picks in the native render (K section 6), no lockstep of Signal Jamming's effect through the menu with a map.
6. Heavy gates still to run (a zero-bail playthrough of the 96 games is now almost there: 94 clean): golden and wide dual sweeps, `board_equivalence`, `core_sync_gate --strict`, `alpha_roundtrip --strict`, the benches, `census`, `soak`, the mutation ratchet and the mutation batches of every resume note since integration 9 (plus this integration's lines), workspace clippy, `TI4_CORE_STRICT=1`.


## 9. Tip

`git log -1 core-integration-15` (the last commit is this document's). The private target dir and the logs under `/root/deploytmp/int15*` were deleted.
