# D4A3 resume (lane A3 of D4: the weak spots left by D4A2)

Branch `core-d4a3` (worktree `.claude/worktrees/core-d4a3`), cut from `core-d4a2` @ `f0bab516`. Only test code and docs changed; **no production code**.

## What was run, what was not

* Run (cheap): `cargo test -p ti4-engine-core-tests --no-fail-fast` in profile `lockstep` with debug assertions and overflow checks ON for the three core
  crates (as `tools/engine_core/d4/run_core_tests.sh`), CARGO_BUILD_JOBS=2, one private target dir: **1105 tests pass, 0 fail** (D4A2: 861; 244 new).
* Run (cheap): single-file mutation checks (`tools/engine_core/mutants.sh`, `TEST_CRATE=ti4-engine-core-tests`, `TEST_ARGS="--test ..."` per file) by Haiku
  runner agents in a scratch worktree at a committed snapshot: round 1 on `c0f5dc27`, round 2 on the commit with the survivor-driven tests (effects.rs: round 2 on the
  last commit). Numbers below.
* **Not run** (user directive, nothing heavy until the end of D1): `verify_package`/gate, golden or wide duals, benches, soak, `mutation_ratchet.sh`, workspace
  clippy. No floor in `tools/engine_core/mutation_floor.json` was written.

## Tests added (`crates/ti4-engine-core-tests/tests/`)

| file | tests | covers |
|---|---:|---|
| `query_objectives2.rs` | 51 | `Pos`/`SeatFacts`/`Terms` fixtures: the counting, remaining and cost tables, every family of public and position-secret objectives, neighbours with I.I.H.Q., bought progress, plans, useful planets, the hand limit |
| `reach2.rs` | 23 | `SeatReach` (enemy ships, tokens, Antimass, Light/Wave, Dominus Orb, anomalies cards, Shared Research + Law's Order, Muaat, Empyrean, Yssaril, Mentak, Aerie Hololattice, Ghosts, Hil Colish, Aetherpassage, Fracture), move values and anchors, Ionian/Gravity boosts |
| `transit2.rs` | 14 | `activatable` (hyperlanes, touched systems, Nexus, Chaos Mapping), `Hold` with a free rider, option dedupe, `wormhole_hops` |
| `query_trade2.rs` | 14 | `Terms`, `can_pay`, `why_illegal` order, action-card door, I.I.H.Q. neighbours, wide partners, `open_partners` (and the diplomacy switch) |
| `agenda_vote2.rs` | 18 | the vote window: Executive Order spender, Gila, Elder Qanoj, Predictive Intelligence, Genetic Recombination + tribute, Blood Pact, extra votes, Zeal, Helios dock, question builders |
| `agenda_mod2.rs` | 5 | `outcomes` of planet/strategy-card/scored-secret elections by rules version, planet offers and faces, shown numbers |
| `production_mod2.rs` | 28 | `placements`, `spots_of`, `build_for` (price, discount, made, supply, `post_build`, Bellum Gloriosum, barred capacity), `payment_faces`, `available`, `pay_opts`, Mitosis, Magmus Reactor, Aerie Hololattice |
| `strategy_cards_prod2.rs` | 17 | Leadership payment loop and credit, Construction (supply, zones, space structures, Thunder's Edge), paid research |
| `action_cards_simple.rs` | 26 | all `Simple` effects (`play_simple`), `has_legal_target`, `reaction_can_matter`, `cost_is_payable`, `is_playable`, `playable_hand` |
| `action_card_effects2.rs` | 25 | 30 cards: Sabotage, Assassinate, Confusing, riders, Puppets, Duress, Veto, Silence, Insider, Pressure, Intercept, Rescue, Invest, Repeal, Spy, Lucky Shot, Exchange, Reparations, Disgrace, Dig, Bribery, Lie in Wait, Seize Artifact |
| `action_card_effects3.rs` | 19 | Pirate Contract/Fleet, Mercenary, Ghost Ship/Squad, Refit, Decoy, Crash Landing, Focused Research, Divert, Plagiarize, Impersonation, Brilliance, Strategize/Overrule, Reveal Prototype, Summit, Exploration Probe |
| `action_card_rerolls.rs` | 4 | Fire Team, Scramble Frequency |

Together with D4A1/D4A2 about 90 of the ported cards now have a rule test (the `Fx` window and the `Simple` effects).

## Mutation numbers (killed / survived; D4A2/D4A1 number in brackets where known)

| file | round 1 | final |
|---|---|---|
| query/objectives.rs | 337 / 39 (90 %) | 349 / 27 (93 %) [A1: 302 survivors] |
| query/reach.rs | 103 / 28 (79 %) | 113 / 18 (86 %) |
| query/transit.rs | 94 / 12 (89 %) | 99 / 7 (93 %) |
| query/trade.rs | 121 / 7 (95 %) | 126 / 2 (98 %) |
| rules/agenda/window.rs | 101 / 33 (75 %) | 111 / 23 (83 %) |
| rules/agenda/mod.rs | 89 / 23 (79 %) | 108 / 4 (96 %) |
| rules/production/mod.rs | 234 / 44 (84 %) | 252 / 26 (91 %) |
| rules/production/strategy_cards.rs | 132 / 65 (67 %) | 149 / 48 (76 %) |
| rules/action_cards/mod.rs | 126 / 26 (83 %) | 141 / 11 (93 %) |
| rules/action_cards/effects.rs | 580 / 315 (65 %) | 654 / 241 (73 %) |

Survivors left are mostly literal `0 -> 1` in initialisers, `>` vs `>=` where the value cannot be 0, unreachable `Env` methods of the vote (`is_nekro`, `votes_first`,
`oceans`), hooks of disabled factions (Keleres commodities, Deepwrought oceans, Ral Nel silence), the Thunder's Edge / waiver paths of `Research`
(`exhaust_specialties`, `settle`), and in effects.rs the options/apply arms of cards whose second step is rarely reached (Fx::Brilliance Fracture roll,
Exploration Probe card kinds, Courageous/Battlestation hit flow, Crash Landing, Decoy). Logs: `/root/deploytmp/core-d4a3/mutout*/` (deleted with the scratch dir).

## Findings (docs/ENGINE_KNOWN_ISSUES.md rows `D4A3-*`)

* D4A3-info-01 the kit's `Scenario::unit` on a planet never sets the system's `TOUCHED` (the string board has an entry); tests that depend on it set it.
* D4A3-found-01 Unstable Planet cannot target a hazardous planet that holds no unit (both engines). Pinned.
* D4A3-info-02 a ship with no capacity cannot carry the Aerie Sentinel (free rider): the `closed` free-rider branch of `Hold::for_ship` is dead. Pinned.
* D4A3-info-03 Archaeological Expedition draws from the first trait deck only. Pinned.
No core defect was fixed (no behaviour change allowed in D4).

## How to resume

1. `git log --oneline core-d4a2..core-d4a3`; `tools/engine_core/d4/run_core_tests.sh $(git rev-parse HEAD)` (or the cargo line above).
2. Still weak: `rules/action_cards/effects.rs` (241 survivors; the hit flows, Brilliance's Fracture roll, Probe's card kinds, Decoy/Crash second steps), the `Research`
   window (`exhaust_specialties`, `settle`, Inheritance Systems), `production/offers.rs`, `combat/roll.rs`, `combat/machine.rs`, `trade_deals.rs`, `invasion/round.rs`.
3. At the end of D1: `MODE=baseline EXTRA_TEST_CRATES=ti4-engine-core-tests tools/engine_core/mutation_ratchet.sh`, then raise the floors (rows for `core/query/objectives`
   etc. when the modules exist in `mutation_modules.tsv`).
4. Scratch: `/root/deploytmp/core-d4a3*` is deleted when this lane is finished.
