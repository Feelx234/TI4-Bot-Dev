# Lane C6 (search API) and C7 (ledger for search copies): resume

Branch `core-search-api-1`, cut from `core-integration-29` (`437ca1bd9`). Not pushed, not deployed, live checkout untouched. Design:
`git show core-theory-fork-rng:docs/FORK_RNG_THEORY.md` (lanes C6, C7), `git show core-theory-mcts:docs/MCTS_BOT_PLAN.md` (2.1, 2.3, 2.6,
4.4), decisions in memory `project_mcts_bot_decisions`. No heavy gate run (by the brief): targeted tests and cheap fingerprints only.

## 1. What there is

New crate `crates/ti4-engine-search` (dev tool: not in the production graph, not in the wasm). Dependencies: the core, the resolver
`ti4-engine-chance`, `ti4-chance` (feature `search`), and the thin crate `ti4-engine` with its `search` feature for what the core cannot do
alone: the `Env` (ability registry with its text ids, the `Render`'s env cache), the combat texts the core wants as codes, the code table,
and the wire form of a question (the rule bot's input). No legacy, host, server or browser crate (`check_layers.py search`). The game specs
come from `ti4-selfplay` (`GameSpec::start`, the native setup), as in the MCTS lab.

### API

| item | what |
|---|---|
| `start(&GameSpec) -> Position` | the native setup, run to the first stop, `Mode::Play`. `quick_spec()` (the `native_play --quick` game), `lobby_spec(f, p, seed)` (the bridge's lobby / perf-suite games), `mcts_spec(seed)` (the MCTS setup: Sol/Letnev/Xxcha/Jol-Nar, Learn-to-play map) |
| `Position::node() -> Node` | `Decide(&Choice)` (core question of `choice.actor`), `Chance(Request)` (Search mode only), `End`, `Failed(&str)` |
| `legal()` / `answers(limit)` | one `Pick` per option in option order (`Choice::pick`: option `i` of the wire form = answer `i`) / a multi-pick's whole sets |
| `answer(Pick)`, `answer_index(i)`, `answer_id(&str)`, `answer_chance(Faces)`, `apply(Decision)` | resume the kernel at the ask and run on to the next stop. `answer_id` reads a wire id as the product's live loop does (`trade::pick`, `pick_of`). An answer that is no option: `SearchError::Illegal`, position unchanged |
| `fork()` / `fork_search()` | an independent copy (same mode / `Mode::Search`) |
| `rendered()` / `seen()` / `seen_for(seat)` | the wire `Choice` (rule bot), the actor's `Seen` in the position the question is asked in (a nested question's own) |
| `outcome()` | `{finished, round, vp, winner}`; winner = most points, ties by initiative order (the product's `objectives::leader`) |
| `steps()`, `decisions()`, `round()`, `vp()`, `is_terminal()`, `rolled()` (the dice a Play-mode answer drew) | |
| `stream_positions()` | how far each stream was drawn (positions, never keys: no accessor of the streams exists) |
| `state_hash()`, `core_hash()`, `same_as()` / `differences()`, `materialized_json()` | checks (O(game)) |
| `trim_ledger()`, `ledger_lens()`, `ledger_bytes()` | lane C7 (section 3) |
| `bot_memory()` / `set_bot_memory()` | the rule bot's movement memory of this line of play |
| `advance::advance_to_tree_decision(pos, delegate, rng, &Rules) -> (Halt, Walked)` | plays on to `Halt::Tree` / `Halt::Chance(req)` / `Halt::End`: non-tree questions to the `delegate` (a `SeatAgent`, e.g. the rule bot), a tree question with one answer taken, a chance request that is no node answered from the search's own `SearchRng`. `Rules { tree, chance_node, skip_single, max_answers }`, defaults `default_tree` (the `ti4_mcts_features::kind` tree kinds, not an action card's question, 2+ answers) and `chance::is_hit_node`. Also `advance::rollout` |
| `chance::SearchRng` (`new(seed)`, `at(seed, &path)`, `sample(req)`, `below(n)`), `hit_outcomes(req)` (hit classes with weights and a representative answer each), `exact_outcomes(req, limit)`, `hits(req, faces)` | |
| `agent::SeatAgent { decide(&Position, &Choice) -> Result<Decision>, observe(&Position, &Applied) }`; `Decision { answer: Answer::{Pick, Index, Id}, memory }` | |
| `agent::RuleBotAgent { view, seed, index }` (`seeing()` = typed view, `blind()` = the random pick of the quick game), `PolicyAgent::new(scorer)` (greedy policy-net hook on the tree asks, rule bot on the rest), `RandomAgent(SearchRng)` | |
| `driver::play(pos, agents, Limits, observe, record) -> Ended`, `driver::replay(pos, &[Record])` | the core driver for selfplay; a replay of the records is the same game |
| bins `search_selfplay` (`--setup mcts\|quick\|lobby:F:P --games N --seed S --agents rule,blind,random,... --games-out X.jsonl --out S.json`) and `search_bench` | |

### How the step is cut

`Position` holds the `CoreState`, the step's place (`Phase::Boundary | Reconcile { round, left_open } | Arm(arm)`), the step's `Env` and the
run's `Env` (with the run's combat texts; both `Rc`, shared by forks, never changed), the run's `fresh` flag (the run's first segment: its
question belongs to the run whatever its nesting; after an answer only a question from inside the answer does), the arm's question count
(the glue rule), the stop (the core `Asking`, `Rc`), the counters, the bot memory (`Rc`) and a warm copy of the `Render`. It is
`native_step` (the product) step for step: the step's `Env` at its start, up to 8 reconcile runs unless the glue flag is set, the arm,
`live_step_inner`'s run (combat texts, code sync, `ask`, else `advance(None)`, a nested question stays in the run), `run_live`'s staged-event
drop, the lone draft card taken without the decider (and rendered, counted as asked), the hand-over of a finished tactical action, the code
table and the glue flag. The wrapper's presentation is not made (no string mirror, events, rolls, journal, observers, auto-paid notes).

Modes: `Play` calls every live function through `ti4_engine_chance::advance_observed` (the product's resolver, the faces kept in `rolled()`);
`Search` calls them bare, so every chance request stops the run as `Node::Chance` and `answer_chance` resumes it with the caller's faces. A
Play position that a search forks becomes Search with `fork_search()`; there is no way back.

Warm fork: `Render::fork_warm` (new, thin crate, behind `search`) copies the code table, the free-form deals, the table codes and the kept
`Env` template (so the next step's `env_with_plays` is a cache hit; a cold `Render::new` costs ~300 us at the next step). Not the ability-id
memo (17 KB per copy, only read by a cold `Env` build; interning a text again gives its old code).

### Chance nodes: the thresholds on the requests (core change, draw-neutral)

On `core-integration-29` no chance request carried a threshold: every die went through `Sx::dice(domain, n, sides)` with `hits_on: None`
(probe on an MCTS game: 153 of 153 requests without). So no tree could group the answers by hit count. Changed in the core (own commit):
`Tape::dice` takes `hits_on: Option<u8>`, new `Sx::dice_on` / `Sx::die_on`, used where the threshold is at hand: space combat (each value
group and its Munitions Reserves redraw), the combat reroll of a staged die, anti-fighter barrage (roll and reroll), space cannon offense
and defense (units and attachments), ground combat, bombardment (the thresholds of the plan in plan order), Harrow, the Ambush/produce roll,
Spec Ops II, the Jol-Nar commander reroll; `Eff::roll` passes the threshold it already had (Infantry II, Strike Wing, Arborec). The answer
never reads `hits_on`, so the draw is the same: quick-game fingerprint and the searched/played suite unchanged (section 4). After it: 146 of
153 requests of the probe game carry a threshold; the 7 left are face-reading or threshold-less draws (rift, crater, Silver Flame, hero
`<= 3`, action-card effects), answered from the search stream.

Caveat (plan 2.3, not checked here): a rule that reads a face beyond the threshold (the Jol-Nar flagship's 9-10) sees the class
representative (`faces_for_hits`: the threshold for a hit, 1 for a miss). `exact_outcomes` or sampling where that matters.

## 2. Tests (`cargo test --profile gate -p ti4-engine-search [-- --include-ignored]`)

| test | result |
|---|---|
| `the_quick_game_through_the_api_is_the_product_quick_game` | the quick game through `Position`, the blind rule bot at every seat: **fingerprint `fd54dc6c956758bc`, 1,110 steps** (= `native_play --quick`, which `wrapper_native` proves equal to the thin `Game` with the blocking step). The driver gives the same records and position; a replay of the records is the same position |
| `the_rule_bot_with_its_view_plays_the_labs_game` (`tests/lab.rs`) | the MCTS setup, seeds 1 and 2, `RuleBotAgent::seeing()` at every seat vs `ti4_mcts_lab::play` (the product's native step, synchronous asker): the same answer ids, steps (1,147 / 1,448), decisions, VP and final core (`Debug` hash) |
| `a_copy_at_every_question_of_the_quick_game_resumes_identically` | at every question: a fork equals the original; a fork answered alike stays equal after the answer; a fork answering another option leaves the original unchanged; every 97th question that copy plays 4 more stops and equals an independent replay from the start with the same answers; the forked game ends with the fingerprint pin |
| `a_copy_at_every_question_of_the_suite_resumes_identically` (ignored, 6 s) | the same over the 14 games of `perf_kernel suite`; each ends equal to its unforked game (`same_as`, chain) |
| `a_search_answers_every_chance_request_itself_and_never_moves_the_streams` + `the_suite_searched_is_the_suite_played` (ignored) | the C4 shape: the game in Search mode, every request answered from the test's own copy of the starting streams, a copy at every request answered with other faces: the position's streams never move (also in the copies); same questions and answers; with the test's streams put in, the searched core `==` the played one. Suite: 252 requests (= chance-2's tip count) |
| `a_search_at_every_tree_decision_leaves_the_game_alone` | MCTS setup seed 3, rule bot with its view at every seat; at every third tree decision two simulations (`fork_search`, a random tree answer, `advance_to_tree_decision` with the rule bot delegate, its own seed): streams untouched, halts are tree kinds / hit nodes / the end; the real game equals the game without search (answers, position, bot memory); the simulations are deterministic |
| `the_combat_rolls_are_chance_nodes_with_hit_classes` | 146 chance nodes, 358 hit classes: every class is a legal answer and a different continuation; a fork shares the ledger (`shares_prefix_with`) |
| `the_trimmed_ledger_plays_the_same_game` | C7: the quick game with `trim_ledger()` before every answer: the same answers, the same core but for the ledgers, the same dice rows |
| `a_policy_seat_and_a_random_seat_play_deterministic_games` | `PolicyAgent` + `RandomAgent` + both rule bots through the driver, twice the same |
| `chance::tests` (2) | hit classes sum to 1 and have their hit count; the search stream is seeded per node path |

Negative controls (mutations, reverted): the run's `fresh` rule dropped -> the fingerprint test and the fork test fail. Not caught: the
staged-event drop and the `arm_asked == 0` clause of the glue rule (neither moves the quick game or the 14 suite games), the lone card's
render (no new code).

## 3. C7: the ledger of a search copy (measured)

The ledger is already copy-on-write (`containers::Log`: shared sealed chunks and tail, two refcount bumps per log): a fork copies nothing
of `ledger.events` (asserted: `shares_prefix_with`). What the copy-on-write costs is the first append after a fork, which copies the tail
chunk (up to 32 records). The rules read only three things of it (`FORK_RNG_THEORY.md` 1.5): `events.last()` (`last_plain_is`), and the
dice rows after a mark (`dice.len()`, barrage and Space Cannon offense, the mark an absolute count stored in the window). So
`Position::trim_ledger()` (search copies only) keeps the last event and the dice rows and drops the older events and the VP/relic rows
(output only). Tested: the same game (section 2). Not a core change.

Measured (`search_bench`, quick game, 1,024 questions; gate profile, shared box, load ~11):

| | value |
|---|---|
| ledger at the end of the quick game | events 2,121 records (492 KB inline), dice 20 rows (800 B), VP 7, relics 0 |
| bytes allocated by `fork()` | **2,077 B** mean (core clone 912 B: the `Vec`s of the pending stack; warm renderer 1,165 B) |
| time of `fork()` | **1.5 us** median, **~650 k forks/s** (core clone 0.9 us, warm renderer 0.8 us; with the ability-id memo in the warm copy it was 19.5 KB and 1.9 us) |
| first answer of a fork, bytes allocated | the original alone 74,111 B; a fork 76,931 B; a fork after `trim_ledger` 71,962 B (trim 0.4 us) |
| live heap kept per simulated fork (one segment played, MCTS setup) | 26.5 KB mean (25 KB median) |

So the tail copy is ~3-5 KB per first answer, a few percent of what an answer allocates anyway (~74 KB: the core's per-call snapshots);
memory per fork is ~2 KB plus what the segment appends. The trim's value is elsewhere: `CoreState ==` and the `Debug` hash of a trimmed copy
read 1 event instead of thousands (O(game) -> O(1) for the events), and a long rollout lineage stops growing. Not done (theory 1.5 step 1-2):
moving the three reads off the ledger and a `Tail(k)` sink with an absolute base in the core; with the trim above they are not needed for
search, and they would change production code and the wasm.

## 4. Speed (`search_bench --games 3 --sims 8`, gate, shared box)

| | value |
|---|---|
| quick game through the API, blind rule bot | 1,110 steps, 1,024 answers in 82 ms: **13,500 steps/s, 12,400 answers/s** |
| simulations (MCTS setup, rule bot with view as delegate, 8 per tree decision, one segment each) | 3 games (seeds 1-3), 1,404 tree decisions x 8 = 11,232 simulations in 6.6 s: **1,700 sims/s, 0.59 ms per simulation**, 5,050 answers/s inside; halts: tree 10,649, chance node 576, end 7. The games themselves (rule bot with view at every seat): 2.5 s for 3 |

(M0 estimated 1.4 ms per simulation on the lab; the lab timed the engine and the delegated rule bot apart.)

## 5. Checks

| check | result |
|---|---|
| `check_layers.py` core / thin / setup / chance / resolver / **search** (new mode) | 576 / 193 / 14 / 5 / 1 / 10 files, 0 violations; the search mode's negative control (a `.rng` read and an `answer_mut`) fails as it must |
| `check_production_graph.sh` (strict; `ti4-engine-search` added to the dev tools and to the "never in a production tree" list) | 0 violations, 0 failed controls |
| `clippy_core.sh` (now also `-p ti4-engine-search`) | ok, engine warnings 41 = baseline |
| `lint_core.sh` | 578 files, 0 hits |
| tests of the core change (gate / lockstep) | `ti4-engine-core --lib`, factions, chance, resolver, `ti4-engine-core-tests`: green; bridge `perf_kernel` (incl. `the_suite_hash_is_the_pinned_one`: `31fe4e2bdd62069f` holds), `fork_core` (C0 + C4), `fork_mid` (quick), `wrapper_native`: green |
| `ti4-engine-search` (gate, `--include-ignored`) | 11 + 1 passed, ~14 s |
| wasm (real `build-wasm.mjs` build, wasm-opt; `/root/deploytmp/c6-logs/ti4_wasm.c6.wasm`) | **6,662,335 raw: 1,129 B OVER the raw budget** (int-29: 6,661,206 = the budget); gzip and brotli under. All of it is the core threshold commit (`82b356bb0`; the thin changes are behind `search`, not in the browser build). **Decision needed**: approve +1,129 B raw, or drop that commit (then every dice request has no threshold and `is_hit_node` never holds: the search samples every roll open-loop; everything else of C6 stands). Not tried: trimming it (the bombardment threshold `Vec`, the `die_on` wrappers) |

## 6. Remaining

* MCTS itself (lane M2+): the tree, the Gumbel selection, the net (`PolicyAgent` is the greedy hook; the forward pass is `core-mcts-prep-2`'s).
* C8 (determinisation of the unseen decks at the search root): not started. A search copy today sees the real deck order (the
  exploration and action-card draws are the deck tops; nothing to branch on).
* `ti4-selfplay` itself has no core driver mode (it cannot depend on this crate: this crate depends on it for `GameSpec`); the driver is
  `search_selfplay` here. Moving `GameSpec` down (or the driver into a new bin crate) would let `selfplay run --agent ...` use it.
* The canonical move order and the activation filter are not applied by `default_tree` (the filter needs `Seen`; the lab and the features
  crate have them): a search passes its own `Rules::tree`.
* The rule bot's answers by option id (`answer_id`) render every question; a search that answers tree asks by `Pick` without rendering plays
  the same game with other code numbers (the code table is a memo): `same_as` then differs although the games are equal.
* A position is `!Send` (`Rc`, `RefCell` in the renderer): one game per thread, as the plan has it.

## 7. Join notes

* `core-multiask-1-rebase` re-merge: no overlap expected with this crate; the core change touches `kernel/ability.rs` (`Tape::dice`,
  `Eff::roll`), `kernel/stage.rs` (`Sx::dice_on`, `die_on`), the combat/invasion roll sites, `driver/turn/prelude.rs` (Spec Ops),
  `driver/windows/invasion/ops.rs` (bombardment), `kernel/commander_walk.rs`. A new `Tape::dice` caller passes `None` or its threshold.
* The thin crate: `Render::fork_warm` (render/mod.rs) and `impl Clone for live::env::Template` (env.rs), both behind `search`.
* Any change to `native_step` / `live_step_inner` / `run_live` / `after_arm` / the glue rule must be made in `position.rs` too
  (the module doc maps them); the quick-game fingerprint test and `tests/lab.rs` fail when they drift.
* `core-mcts-prep-2` / the MCTS lab: replace `ti4_mcts_lab::Pos` / `step` by `Position` / `advance_to_tree_decision`; the `Decide` trait maps
  onto `SeatAgent` (`RuleBotAgent::seeing()` is the lab's `RuleBot`, proven equal by `tests/lab.rs`).

Disk: target dirs deleted at the end (`/root/deploytmp/c6-target`, `c6-wasm`); kept: `/root/deploytmp/c6-logs`.
