# Lane C8 (determinisation of the hidden cards at the search root): resume

Branch `core-determinise-1`, cut from `core-search-api-1` (`8a32981b2`, C6/C7, `docs/C6_RESUME.md`). Not pushed, not deployed, live checkout
untouched. Design: `git show core-theory-fork-rng:docs/FORK_RNG_THEORY.md` 4.8 and lane C8; user decision (memory
`project_mcts_bot_decisions`): the search never peeks; at each root the unseen decks are reshuffled consistently with what the searching
seat has seen, from the search's own stream. No heavy gate (by the brief): targeted tests only.

**No core change, no thin-crate change, no Cargo change**: everything is in `crates/ti4-engine-search` (a dev tool, not in the wasm or the
production graph). The `Deck::set` the core already has is the whole "reshuffle API" needed. So the wasm cannot grow and the pins cannot move:
quick game `fd54dc6c956758bc` re-checked by `the_quick_game_through_the_api_is_the_product_quick_game`; the perf_kernel suite
(`31fe4e2bdd62069f`) builds nothing this branch touches.

## 1. Inventory: the hidden information of a `CoreState`

| what | where | who sees it | C8 |
|---|---|---|---|
| secret objectives in hand | `players[i].cards.secrets` | the holder; others the count (`SeatView::secret_objectives_held`); a viewer a reveal shows them to (`MarkKey::CardsReveal` rows) | **redealt** (other seats') |
| secret objective deck | `decks.secret` (order and content) | nobody | **redealt** with the other hands, one pool |
| public objective deck | `decks.objective`: 5 stage I then 5 stage II as dealt at setup (`deck::build_starting_decks_excluding`), minus the revealed ones | nobody: neither the order nor WHICH five of a stage were dealt | **redrawn per stage from the game's pool** (`Table::objective_pools`: the scope's catalogue minus `map_feasibility`'s ruled-out objectives, a public rule) minus the revealed/scored ones |
| agenda deck | `decks.agenda` (content = catalogue minus laws / discards / in play: public) | nobody; the Politics holder knows where it put the two agendas it looked at | **reshuffled**; the agendas the searching seat itself placed with Politics stay in place |
| relic deck, the 4 exploration decks | `decks.relic`, `decks.explore[k]` | nobody | **reshuffled** (the search does not branch on them, but without this it played on the real next card) |
| action card deck, other hands | `decks.action`, `players[i].cards.hand` | holder; others the count | **not resampled** (action cards are not simulated in v1, brief) |
| promissory notes in hand | `cards.promissory` (minus `promissory_faceup`) | holder (+ M'aban's standing permission) | not resampled (the deal is fixed per faction; which notes changed hands is public in practice) |
| covert agenda | `agenda.covert` (Covert Legislation, an action card) | the speaker | not resampled (action card) |
| cards a window holds while open | `pending` (typed windows), the driver's call stack (Politics' looked-at pair), the stop's `Asking` | the window's actor | kept in place (see 2) |
| the RNG streams | `rng` | nobody | not a card: in `Mode::Search` the search answers every chance request itself (C4/C6); a determinised copy is always `Mode::Search` |

`Seen` (the rule bot's typed view) reads no deck at all (`seen::READS` excludes `Slice::Decks`), so it shows the deck sizes nowhere; the
determinisation keeps every size anyway.

## 2. What there is

`ti4_engine_search::determinize` (`src/determinize.rs`):

| item | what |
|---|---|
| `determinize(&Position, seat, &mut SearchRng) -> Position` | a `Mode::Search` copy (`fork_search`) with every card `seat` has not seen drawn anew; `pos` untouched; no stream read or drawn |
| `determinize_report(..) -> (Position, Report)` | the same, with the pool sizes per class and `pinned` (cards kept because the state names them) |
| `seen_digest(&Seen) -> String` | every accessor of the view that takes no imagined position, as text (facts, each seat's public standing, scored/revealed objectives and their progress, agenda facts, the board, the bound seat's cards, secrets, notes, what others showed it, its secret progress): the consistency check |
| `Table::objective_pools` | per game: the stage I / II objectives the setup could have dealt (content records of the scope by points, minus `ti4_engine::map_feasibility::unsatisfiable_public` on this board) |
| `Position::placed` (internal) | the agendas each seat put back with Politics in this line of play (recorded in `decided()` from the question `Strategy(PoliticsAgendas/PoliticsAgenda)`; shared by forks, `Rc`; not compared by `differences`) |

How: per class (secrets; objectives stage I; stage II; agendas; relics; each exploration deck) the **unseen pool** is the sorted set of cards
that are candidates in EVERY state that holds cards (the live core and the stop's `Asking::at` / nested position: the core is rolled back
before a step's question, `at` is the position it was put in), minus cards **named** in the `Debug` text of those states' pending machine,
scratch, trade, agenda slice (without its logs), deck reserve, marks (a `CardsReveal` row only when `seat` is its viewer), players (without
the hidden hands) and the stop's question, minus the seat's own Politics placements. A uniform permutation `sigma` of each pool
(Fisher-Yates over `SearchRng::below`) is applied as a relabelling to every one of those states (`Position::rewrite_card_states`: the core
and a copy of the stop's `Asking`), so the states stay consistent with each other and the identity holds outside the pools. A uniform
relabelling of the pool applied to the true arrangement is a uniform draw among the arrangements consistent with the seat's view (for the
objectives the pool includes the cards out of the game, so the dealt set is redrawn as well).

What was pinned in practice (seed 3, every question): agendas named in the agenda phase's windows (the revealed agenda, still in the rolled-
back core's deck), the Politics pair named by its own question, the secret named by the seat's own "return a secret" question: ~0.9 cards
per tree decision, all public or the seat's own. No other seat's hidden secret was ever pinned.

Cost (`search_bench --only determinize --games 3`, gate, shared box, load ~5): 1,404 tree decisions, **median 136 us**, 59 KB allocated, 188
cards redrawn per call (mostly the explore and agenda decks). Fine once per root; as a per-simulation determinisation (ISMCTS) it is ~25 % of
a 0.59 ms simulation. The bulk is the `Debug` text of the "named" scan; a typed scan of the windows would cut it.

## 3. Tests (`cargo test --profile gate -p ti4-engine-search c8_tests`, ~12 s; all 16 tests of the crate pass with `--include-ignored`)

| test | checks |
|---|---|
| `a_determinisation_keeps_what_the_seat_sees_and_redraws_the_rest` | MCTS seed 3, rule bot with its view, every 4th question (396 checks, 157 with an asked-in position): for the actor, two seeds and a repeat: `seen_digest` of the question's position and of the live core equal the real ones; the MCTS features (`ti4_mcts_features::encode`) and the rule bot's rendered question equal; one seed = one copy (`same_as`, `state_hash`); the real position's hash and the streams untouched; the copy `Mode::Search`, its streams not drawn; hidden classes are permutations of the same cards (hand/deck sizes, own secrets, action cards and decks, objectives/agenda/cards/board/marks slices equal; each objective slot keeps its stage, no duplicate, nothing revealed back in the deck, all from the pool); the asked-in positions relabelled alike. Tallies: two seeds differ in the secrets / agendas / objective deck **396 / 396 / 396 of 396**; the copy's secret top is the real one 10 of 396 (~1/30 = chance); the real stage II set kept **0** of 396. The game itself equals the game never determinised (answers, `same_as`) |
| `determinised_copies_fork_and_resume_identically` | MCTS seed 5, every 5th tree decision: a fork equals the copy; two forks played alike (same tree answer, rule bot delegate, same search seed, `advance_to_tree_decision`) are `same_as` with equal hashes and halts; a fork answering another option leaves the copy alone; no copy moves the streams; every 40th: the determinised copy rolled out to the end without a failure |
| `politics_placements_stay_where_their_placer_put_them` | seeds 1-4, every question and seat that placed agendas: the placer's agendas keep their deck index in its own determinisations (8,529 checks); for another seat they move (8,466 of 8,529) |
| `the_objective_pools_hold_the_dealt_objectives_and_the_draw_leaves_them` | every dealt objective is in its stage's pool; 64 determinisations of the opening deal objectives outside the real deal (>10), never a revealed one |

Negative controls (mutations, reverted): the seat's own secrets put in the pool (and not named) -> the view test fails (`seen_digest`);
the asked-in positions not relabelled -> "an asked-in position not relabelled"; the Politics pin dropped -> the Politics test fails; the
objective pool reduced to the deck's cards -> the pool test and `stage2_kept` fail. (The first mutation alone, with the seat's secrets still
named in the scan, is caught by nothing: the scan pins them, defence in depth.)

## 4. Checks

| check | result |
|---|---|
| `check_layers.py` core / thin / setup / chance / resolver / search | 576 / 193 / 14 / 5 / 1 / 12 files, 0 violations |
| `check_production_graph.sh` (strict) | 0 violations, 0 failed controls |
| `clippy_core.sh` | ok, engine warnings 41 = baseline |
| `lint_core.sh` | 578 files, 0 hits |
| wasm | no wasm crate or dependency touched (search crate only): cannot grow; not rebuilt |
| fingerprints | quick game `fd54dc6c956758bc` (search crate test, green); perf_kernel suite: nothing it builds changed |

## 5. NOT hidden-information safe yet

* **Action cards**: the action deck order and the other seats' hands are the real ones (v1 does not simulate action cards; the rule bot
  answers their asks). Doing it needs care: the step's `Env` (ability registry, `env_with_plays`) is built from the hands at the step start,
  so a relabelling of hands should happen at a step boundary or rebuild the `Env`. Also `agenda.covert` (Covert Legislation) and
  `agenda.veto_replacement`.
* **Cards a window holds while open** stay in place (the named scan): the Politics pair while its question is open is out of the deck and
  named by the question, so a copy made at another seat's root during that window still has the true pair (others should not know it). Same
  for any window that names a deck card. Rare at tree decisions (0.9 cards per call, all public or the seat's own in the seed-3 game).
* **Knowledge from history** the position does not keep: an objective purged by Neuraloop (revealed, then removed) comes back into the
  stage pool; a Politics agenda that left the deck and came back by a reshuffle stays pinned (stale knowledge). Peeks by factions or cards
  outside the v1 four (an ability that looks at a deck top) are not tracked: only Politics is.
* **Promissory notes in hands** are not resampled (in practice public, but a strict view would redeal the other seats' unplayed notes they
  received in trades nobody saw).
* **The other seats' "knowledge"** is not modelled: an opponent who saw a card (a reveal to it, its Politics placement) is simulated with
  the searching seat's draw, not with what it knows (standard single-observer determinisation).
* The **ledger** (events) of a copy still holds the true history, which may name drawn cards; no rule reads card identities from it (C6:
  only the last event and the dice rows), and the search API exposes it only as `materialized_json` (tests). Do not feed it to a net.
* The positions are determinised by the API, but nothing forces a caller to use it: an MCTS that forks the real position without
  `determinize` still peeks. The MCTS core (`core-mcts-core-1`) should call `determinize(&root, actor, &mut search_rng)` once per root (or per
  simulation for ISMCTS) and search only from the result.

## 6. Join notes

* `core-mcts-core-1`: at the root, `let root = ti4_engine_search::determinize(&pos, actor, &mut SearchRng::at(seed, &[decision]))`; the
  result is already `Mode::Search`. Re-determinise per simulation if wanted (136 us).
* `position.rs` gained `placed` (forks share it), `politics_placement()` in `decided()`, `card_states()`, `stop_choice()`,
  `rewrite_card_states()`, `placed_by()`. A merge that rewrites `decided()` must keep the recording.
* `table.rs`: `Table::objective_pools`, computed in `start()`; any other `Table` constructor must fill it (an empty pool degrades to
  reshuffling the deck's own cards, which leaks which objectives were dealt).
* `search_bench --only determinize`.

Disk: target dir `/root/deploytmp/c8-target` deleted at the end.
