# D2 lane F2c (`core-d2-c3-dispatch2`): the first real native-vs-legacy differences, resume note (2026-10-31)

Branch `core-d2-c3-dispatch2`, cut from `core-d2-c3-dispatch` (`ccc59b87`). Never pushed; `nightly-fixes-2026-10-26` untouched. Only the LATEST rules version is supported.
Read `docs/KM2_RESUME.md` (the dispatch), `docs/KM_RESUME.md` (the menu) first. Integration 11 (`core-integration-11`) was NOT finished (no `docs/INTEGRATION11.md`, it does
not contain `core-d2-c3-dispatch`), so this branch is based on `core-d2-c3-dispatch`; `core-x-explore` (the exploration window, Local Fabricators) is already in it through
lane H (`8df15b0a` is an ancestor).

## 1. The first difference of `native_play --shadow` (sol, 4 players, seed 1): who is right

Step 24 `[aftermath]`: "the native aftermath asks Local Fabricators' reward (`gain`), the legacy replay is already at the next seat's menu".
**Both games ask it; the harness was wrong.** The string asks the reward INSIDE the legacy step that answers the second commit (an exploration after a first claim puts its
question synchronously, `ctx.ask_seeing`), the native loop asks it as a question of its own in the NEXT step. The shadow gave the legacy game the answers of one native step per
legacy step, so the string's inline question met an empty script: `Follower` errors, `exploration::ask` swallows the error (`.ok()`) and takes the default `gain` unrecorded, and
the legacy game ran on to the next menu (`legacy exploration_log` showed `lf2 ... gained 1 commodity`). Not a difference in the game, a difference in the oracle.

Fixes (all in `ti4-engine-core-bridge/src/native_play.rs` unless said):

| # | what | fix | guard |
|---|---|---|---|
| F1 | a question the string asks inside a step is answered in that legacy step | `shadow_play` plays the next `AHEAD` (4) native steps on a `NativeGame::fork()` and queues their answers behind the group's; the legacy step reports how many it used (`records.len()`), the group grows by exactly those native steps; the unused answers are dropped (`MaskSubject::clear_script`) | `a_question_the_string_asks_inside_a_step_is_answered_in_that_legacy_step` (fails on the old shadow: `script wanted "gain"`) |
| F2 | the aftermath did not mirror what its sub-windows emitted into the event queue (the string mirrors its timing log into `Game::events` when the step returns, before its own `INVASION_RESOLVED`...): `UNITS_COMMITTED`, `GROUND_COMMITMENT_FINISHED`, `PLANET_CONTROL_GAINED`, `PLANET_EXPLORED`, `INVASION_BEGAN` were missing from the native event list. Hidden before because the shadow compared a prefix of length 0 | `driver/windows/chain.rs`: `Out::emitted` of every sub-window of the step is written (`fx::mirror_recs`, new, additive in `kernel/fx.rs`) before the aftermath's plain log, at the three places the step ends (`mirror_then_flush`); a step that stops at a nested question mirrors nothing (the replay re-emits). Known limit: only top-level emissions (an event a reaction emitted inside another is not mirrored) | core test `aftermath_chain::the_events_an_invasion_emitted_are_mirrored_into_the_queue_before_its_resolved_line` (fails on the old chain: no typed event in the queue) |
| F3 | `native_play` rendered without the faction texts: Politics' speaker question and Trade's replenish question had option ids `""` | `NativeGame::from_game` builds the `Render` `with_factions` (kept across a rolled-back step) | seen in the trace (`["","",""]`); every Politics/Trade shadow past it |
| F4 | the words of an exploration row are minted straight into `core.texts.codes` (`exploring_dual::words`), then the step's end overwrote that table with the render's older copy: `native Explore: unknown text code "580"` at the next strict compare | `Render::sync_codes(&mut Interner)` (new, `render/mod.rs`): one table for both, only when one is a prefix of the other; called in `live_step_inner` before the step and after every `advance` | bridge test `the_words_of_an_exploration_row_survive_the_step_and_the_position_materializes` (fails before: unknown text code) |
| F5 | the two games end a step in different places inside one arm's close (the native secondary arm closes the whole strategic action in the step the string takes two for; the string's tactical step runs the aftermath up to its first question where the native steps split) | the events are two streams: what one side pushed beyond the other's end of step is `Owed` to the next group (`Owed::settle`); a strict step (Ask, Phase) must end level; a native step that asks nothing while the legacy game stands at a question is matched against what the legacy pushed ahead; a legacy step that used no answer (the close of an action) is repeated with the same answers | `events_one_side_is_ahead_by_are_owed_to_the_next_group_and_a_strict_step_must_end_level` |
| F6 | the legacy position keeps the events effects staged as `private:#staged:event:*` marks, the core in `Pending::staged` | compared by name and kept out of the position (as `NativeSubject` does) | the hacan/winnu/xxcha shadows past it |

`TI4_NATIVE_PLAY_TRACE=1` now also prints, per compared group, the answers given to the legacy game, the decisions it recorded and both event lists.

## 2. How far `native_play --shadow` goes now (cheap build, `gate` profile; steps = native steps, compared = legacy steps compared)

| game | steps (compared) | first stop |
|---|---|---|
| sol, 4p hyperlanes, seed 1 (the quick game) | 54 (36) | `GAP ask.action_menu`: `TurnGap::StrategyAbility` (Thunder's Edge Warfare's free tactical action / Doctor Sucaban's research offer): lane M6c/M6d (`open_free`, `resume_strategy`) |
| hacan, 5p, seed 3 | **98 (73)**: the whole action phase of round 1 | difference at `[phase]` (status phase): the legacy offers `infantry\|05\|nestphar` / `mech\|05\|nestphar` (Arborec's Mitosis) where the native asks `tactic_tokens`: the status phase does not ask Mitosis. Owner: C (status) / the Arborec faction module |
| naalu, 5p, seed 12 | 64 (42) | difference at the menu step: native `STRATEGIC_ACTION_COMPLETE` where the legacy has `STRATEGY_TOKEN_SPENT` first (order of the closing of a strategic action vs a spent token). Owner: E |
| sol, 3p, seed 2 | 31 (20) | difference at the aftermath: the space cannon of the activated system (see 3) |
| arborec, 6p standard, seed 4 | 20 (16) | the same space cannon difference |
| letnev, 4p hyperlanes, seed 5 | 14 (12) | `[tactical]`: the native movement question offers `done_moving` only where the legacy offers `move\|10\|2` (see 3) |
| xxcha, 3p, seed 6 | 23 (15) | `GAP ask.action_menu`: `Unrouted(11, ..)`: a faction component (Star Forge or the like) |
| sol, 3p, seed 9 | 11 (8) | `GAP ask.action_menu`: `Unrouted(5, ..)`: Nekro Malleon (the core window `NekroMalleon` exists; it needs the interned `nekroagent` source code in a `Codes` table and the action-card name groups in the play facts) |
| jolnar, 5p, seed 7 / mentak, 4p, seed 10 | 10 (8) / 14 (12) | `[tactical]`: the legacy offers `leader:saar:saaragent:SYSTEM_ACTIVATED:after` / decline, the native tactical step does not (see 3) |
| winnu, 4p outer rim, seed 8 | 24 (19) | `GAP tactical`: `UnportedTactical(ActivationHook)` (`activation_hook_held`): lane M6 |
| yssaril, 6p outer rim, seed 11 | 21 (17) | `GAP tactical`: `UnportedTactical(SsruuLends)`: lane M6 / G |

No game finishes natively yet; the hacan game plays a whole round of actions natively and in lockstep with the string (73 legacy steps compared, no difference until the status phase).

## 3. Differences / gaps found and NOT fixed, with owners

* **Space cannon at the head of the aftermath** (arborec seed 4, sol 3p seed 2): the legacy pushes `UNIT_ABILITY_ROLLED`, `SPACE_CANNON_HITS` after `MOVEMENT_FINISHED`; the native aftermath (`Head::Firing`,
  `SpaceCannonOffense`) pushes neither (no roll at all, or none announced). Facts: `tactical/facts.rs::head_facts` (`reaching: REACHING`, `mapped`, `unit_texts: Vec::new()`). Owner: M6 / M5 (combat head). Reproduce:
  `TI4_NATIVE_PLAY_TRACE=1 .../native_play --factions arborec --players 6 --first-seed 4 --shadow`.
* **Movement options on a hyperlane map** (letnev 4p seed 5): `done_moving` only vs `move|10|2` in the legacy. The string fixed its facts in `7c28b67f` ("find routes on the map as the engine has it (laws, lanes, map edits)"); the core's movement facts read the setup map. Owner: A2 / M6 (movement facts).
* **Saar agent reaction after a system is activated** (jolnar seed 7): the native tactical step does not open the `SYSTEM_ACTIVATED:after` reaction of the Saar agent. Owner: G (abilities) / H1.
* **Diplomacy primary stages no `COMMAND_TOKEN_PLACED`** at a table with a module (xxcha seed 6 before F6 showed it; the strategic lockstep covers trade, politics, leadership, imperial only). To re-check after F6: it may be fine now. Owner: E.
* **Activation hooks** (`UnportedTactical(ActivationHook)`), **Thunder's Edge Warfare / Doctor Sucaban** (`StrategyAbility`): M6c/M6d, as in KM2 section 3 (not merged here: 20 and 12 conflicts).
* **Not routed from the menu** (this lane, `Unrouted(kind, key)`): every leader except Ggrocuto Rinn and Carth (kind 5; Nekro Malleon next), Production as an ability (6), relics in use (7), Enigmatic Device (8), faction components (11: Production Biomes, Trade Convoys, Blood/Dark Pact, Wormhole Generator, Promise of Protection, Muaat, Naaz, Yssaril, Nekro C4R, Terraform, Cabal's Vortex).

## 4. What is native now (additions to KM2 section 2)

* `driver/turn/leader.rs`: Carth of Golden Sands (a free use: `LEADER_ABILITY_RESOLVED`, the turn stays open; core test `carth_chosen_from_the_menu_is_a_free_use_that_gains_commodities_and_leaves_the_turn_open`) and Ggrocuto Rinn (Xxcha agent), chosen from the menu: the core window `XxchaAgent`, the agent exhausted through `fx_strategy::exhaust_ready_agent` (a listening Nomad is told), `conclude` (staged
  destructions/cards announced, Extreme Duress, `COMPONENT_ACTION_RESOLVED`/`_FAILED`, `finish_action`). `NativeSubject::menu_body_is_lent` no longer lends `component|leader|xxchaagent`. Evidence: core test
  `ggrocuto_rinn_chosen_from_the_menu_readies_the_planet_exhausts_the_agent_and_ends_the_action` (a map, an exhausted planet); the shadow of sol seed 1 plays through the real use (step 43 on). The engine lockstep file
  (`action_dispatch_live_tests`) loads no map, so the lockstep against the string for this leader is the `native_play` shadow (a lockstep with a galaxy is the next test to add).
* The other leaders (agents, commanders, heroes, Ssruu copies) need the same: a window from `rules::leader_windows` / `hero_windows` / `leader_production`, its facts computed from the core, `fx_strategy::exhaust_ready_agent` or `rules::leaders::purge`,
  and for the ones that are not an ACTION (`uses_the_action == false`: Carth) `LEADER_ABILITY_RESOLVED` without `finish_action`. The route table is `leader::run`; add one arm per leader.

## 5. Tests added (KM2 section 8)

| item | test | note |
|---|---|---|
| the Orbital Drop 3-resource boundary | core `action_dispatch::orbital_drop_chosen_from_the_menu_offers_the_mech_at_three_resources_and_not_at_two` | kills `>= 3` -> `>= 2` / `> 3` |
| the second action out of the closing menu | core `action_dispatch::an_action_taken_from_the_closing_menu_is_the_second_action_and_ends_the_closing_menu` | the free-option half stays lane F's lockstep |
| failed Lazax Gate Folding / failed Orbital Drop | none: the menu offers Lazax only with a ready card, Mecatol and an infantry in the box, and Orbital Drop only with a token, so the `Failed` verdict is unreachable through the menu; the literals stay (defensive) | to mark `// mutants:skip` with this reason at the next mutation pass |
| destruction announced after a component action, cancelled card play | not written | need a Sabotage (a reaction) or a destroying ACTION card in the engine lockstep harness; unchanged from KM2 |

## 6. Checks run (cheap)

* `cargo test -p ti4-engine-core-tests --test action_dispatch`: 9 passed (5 of KM2, the Xxcha and Carth routes, the Orbital Drop boundary, the closing-menu second action); `--test aftermath_chain`: 18 passed (incl. the mirror test).
* `cargo test -p ti4-engine --lib` (whole, plain areas, once at the end): **5137 passed, 0 failed, 7 ignored**. `cargo test -p ti4-engine-core-bridge --lib`: 102 passed + the 3 stale `native_play` tests brought to the game they now play (54 steps, gap
  `StrategyAbility`; the pinned 11 steps / `AskSource` were the state before the menu dispatch) and 3 new ones: 26 `native_play` tests pass after the update (the other 79 passed in the earlier whole run).
* `check_layers.py core`: 513 files, 0 violations. `lint_core.sh`: 515 files, 0 hits.
* `native_play --shadow`: the table of section 2 (`CARGO_TARGET_DIR=/root/deploytmp/core-f2c`, deleted afterwards).
* Not run (standing rule): golden/wide dual sweeps, `TI4_AREAS=all=dual`, benches, soak, the mutation ratchet, census, workspace clippy, all 141 core test binaries. **The mutation check of the new core halves was NOT run** (`driver/turn/leader.rs`, the mirror in
  `driver/windows/chain.rs`, `kernel/fx.rs::mirror_recs`, `Render::sync_codes`): the first tests exist, the survivors are unknown. The failure arms of `leader.rs` (a leader that resolves nothing) are unreachable through the menu (the offer needs an exhausted planet / a
  possible target): `// mutants:skip` them with that reason at the pass.

## 7. Next (in order)

1. The remaining leaders from the menu (`leader::run`): Nekro Malleon (needs the `nekroagent` source code and the card name groups), Z'eu is dropped content, Jace X, Darktalon Treilla, Rin, The Helmsman, Xxekir Grom TE, commanders and Ssruu copies; heroes that open a strategy primary need M6c's `resume_strategy`.
2. Technology (Production as an ability: `UseOpen::Ability` is a held window the step loop cannot select), relics in use and the Enigmatic Device (facts: research payment, targets over the map), the faction components of KM2 section 3.
3. The differences of section 3 (space cannon head facts, movement on hyperlane maps, the Saar agent reaction, Mitosis in the status phase, the strategic action closing order), each with its owner.
4. Merge `core-m6c/m6d-tactical`, `core-g2/g3-abilities` at the join points named in KM2 section 4; rebase on `core-integration-11` when it is finished.
