# MCTS prep: M0 (engine speed) and M1 (ask census)

Lanes M0 and M1 of `docs/MCTS_BOT_PLAN.md` (branch `core-theory-mcts`), branch `core-mcts-prep-1` from `core-integration-28`.
Raw output: `docs/mcts_data/census200.md` (all tables, every ask kind) and `census200.json`.

## Setup and method

* **Games**: 200 rule-bot games, seeds 1..200, 4 players on `4pLearnToPlayPoK`, factions Sol, Letnev, Xxcha, Jol-Nar, seating
  `seed % 24` (all 24 permutations, each about 8 times), speaker drawn from the seed, `starting_tech` on, the product's default
  strategy card set (`te`: Thunder's Edge Construction and Warfare). Round cap 12, step cap 6000. **All 200 finished** (victory),
  no error, no panic. Rounds at the end: 4: 1, 5: 22, 6: 85, 7: 69, 8: 19, 9: 4.
* **Not yet the target setup.** There is no setup switch for "no action cards" or "no exploration randomness": these games have
  both (action-card asks, reaction windows, exploration cards and relics all appear in the census). Open problem P1 below.
* **Driver**: new dev crate `ti4-mcts-lab` (`crates/ti4-mcts-lab`). Its `step` is the product's native step
  (`ti4_engine::native::Game::native_step`) without the wrapper's extras that a search never needs: no string mirror
  (`materialized` per step), no event text, no table/journal, no window observers. What a position is here (`Pos`) is what C6's
  `search::Position` will be: the `CoreState`, the `Render` (code table, env cache), the glue flag. Every question goes to a
  `Decide` with the core `Choice`, the wire `Choice` and the position it is asked in.
* **Rule bot on the typed view**: `Seen::new(at, kernel, actor)` -> `SeenView` -> `ti4_rule_bot::choose_explained_view` (decision
  index and seat place as in selfplay). Not the string `Observed` of the product's table. The rule bot's thread-local movement
  memory (`backward::memory`) is reset per game and saved/restored around every replay.
* **Binary**: `mcts_census --games 200 --workers 4 --json PATH` (gate profile). Timing per ask with `Instant`; "engine" is the loop
  time minus the rule bot (incl. building the `SeenView`) minus the filter. The forks and the replays are timed apart and taken out
  of the loop time.
* **Noise**: the box was shared (load average 8 to 13 on 8 cores, other lanes building). 4 workers. Read wall times as +-20 %; the
  counts are exact. CPU: AMD EPYC Genoa.

Reproduce: `cargo build --profile gate -p ti4-mcts-lab --bin mcts_census && target/gate/mcts_census --games 200 --workers 4 --json x.json`
(125 s wall on the loaded box).

## M0: engine speed

| quantity | value |
|---|---|
| steps / game | 1,752 |
| asks (decisions) / game | 1,955 |
| loop time / game | 1,978 ms |
| of it engine (the core's live loop, env build included) | 388 ms (20 %) |
| of it the `Env` build (`env_with_plays`, cached) | 6 ms |
| of it the rule bot (all asks, incl. `SeenView`) | 1,574 ms (80 %) |
| of it building `Seen` + `SeenView` | 2.6 ms (1.3 us per ask: the view is lazy) |
| of it the Activate filter (all activation asks) | 16 ms (0.2 ms per activation) |
| **engine steps / s** (bot removed) | **4,500** |
| steps / s, whole loop with the rule bot | 890 |
| step wall p50 / p99 | 167 us / 16 ms |
| **`Pos::fork`** (core clone + render rebuilt from the code table) | **3.8 us** |
| `CoreState::clone` alone | 2.4 us |
| fork + replay one step with the rule bot (44 k samples, 1.12 asks each) | 1.8 ms |
| tree nodes / game (all seats; table 2.2 kinds with 2+ options after the filter) | **577** (144 per seat) |
| loop time per tree node | 3.4 ms |
| engine time per tree node | **0.67 ms** |
| rule bot on the delegated asks per tree node (368 ms / game / 577) | **0.64 ms** |
| segment wall between two tree nodes: mean / p50 / p90 / p99 | 4.1 / 1.1 / 8.6 / 44 ms (includes the rule bot answering the *tree* ask that opens it) |

**Rule bot per ask kind** (top by time; all 104 kinds are in `census200.md`):

| ask kind | tree | per game | mean us | p99 us | ms / game |
|---|---|---:|---:|---:|---:|
| Turn.ActionMenu | tree | 169.5 | 3,924 | 44,665 | 665 |
| Tactical.Activate | tree | 75.2 | 4,326 | 42,676 | 325 |
| Production.Choose | | 78.0 | 2,226 | 30,276 | 174 |
| Strategy.Draft | tree | 44.9 | 3,784 | 36,672 | 170 |
| Strategy.ResearchTechnology | | 14.5 | 2,626 | 32,019 | 38 |
| Agenda.Planets | | 85.4 | 402 | 3,262 | 34 |
| Reaction.Window | | 100.4 | 235 | 608 | 24 |
| Tactical.Move | tree | 155.6 | 149 | 594 | 23 |
| Turn.Closing | tree | 143.9 | 120 | 167 | 17 |
| Strategy.GainPool | | 124.9 | 133 | 185 | 17 |
| Agenda.Vote | | 31.9 | 386 | 3,282 | 12 |
| Production.Pay | | 184.1 | 58 | 101 | 11 |
| Tactical.Load | | 127.1 | 70 | 117 | 9 |
| everything else (90 kinds) | | | 20-150 typ. | | < 10 each |

Delegated asks cost 368 ms per game in total, tree asks 1,206 ms (77 % of the bot time). The expensive answers are the planning
ones (`PlanCtx`-style forecasts: menu, activation, draft, production choice, research), 2 to 4 ms each with a long tail to 30-45 ms.

**What this means for the search.** A simulation is one segment: the engine to the next tree node (0.67 ms) plus the rule bot on the
delegated asks inside it (0.64 ms), plus the fork (4 us) and the net (plan: about 0.1 ms). **About 1.4 ms per simulation**, the low end of
the plan's 1-5 ms. With the playout cap of 2.6 (0.25 x 32 + 0.75 x 8 = 14 simulations on average) a searched game is about
577 x 14 x 1.4 ms = **11 s per core** (plan: 20 s). 7 workers x 86,400 s / 11 s = 55 k games per day is the bound for an idle
box; with the nightly windows and the shared box, plan for 10-20 k per day.
Rule-bot data generation (L2) costs 2 s per game per core (the tree-kind answers are the expensive part of it).

## M1: ask census

### Asks per kind (the tree kinds and the biggest delegated ones)

| ask kind | tree | per game | smart = false | opts mean (min-max) | p90 | after filter mean (min-max) | p90 | 1 option after filter |
|---|---|---:|---:|---|---:|---|---:|---:|
| Turn.ActionMenu | tree | 169.5 | 31 % | 5.3 (1-13) | 8 | = | | 2 % |
| Tactical.Move | tree | 155.6 | 1 % | 2.7 (1-13) | 4 | = | | 20 % |
| Turn.Closing | tree | 143.9 | **99 %** | 3.3 (1-11) | 4 | = | | 3 % |
| Tactical.Activate | tree | 75.2 | 0 % | 36.3 (29-38) | 38 | **17.4 (1-38)** | 31 | 1 % |
| Strategy.Draft | tree | 44.9 | 15 % | 5.0 (2-8) | 8 | = | | 0 % |
| Objectives.Score | tree | 17.5 | 0 % | 2.4 (2-7) | 3 | = | | 0 % |
| Strategy.TeWarfareTactical | tree (new) | 6.2 | 11 % | 37.1 (37-38) | 38 | 15.2 (1-37) | 32 | 5 % |
| Strategy.ImperialScore | tree | 3.9 | 0 % | 2.2 (2-5) | 3 | = | | 0 % |
| Production.Pay | | 184.1 | 0 % | 4.8 (2-20) | 8 | | | 0 % |
| Tactical.Load | | 127.1 | 0 % | 3.4 (2-10) | 5 | | | 0 % |
| Strategy.GainPool | | 124.9 | 0 % | 3.0 | 3 | | | |
| Reaction.Window | | 100.4 | 44 % | 2.0 (2-3) | 2 | | | |
| Strategy.Follow | | 87.0 | 43 % | 2.0 | 2 | | | |
| Agenda.Planets | | 85.4 | 0 % | 6.0 (2-20) | 10 | | | |
| Production.Choose | | 78.0 | 0 % | 7.1 (2-9) | 9 | | | |
| Invasion.CommitGround | | 72.4 | 0 % | 3.0 (1-10) | 5 | | | |
| Strategy.LeadershipBuy | | 48.2 | 0 % | 2.0 | 2 | | | |
| Agenda.Vote | | 31.9 | 0 % | 5.3 (2-39) | 5 | | | |
| Trade.Propose | | 31.5 | 0 % | 17.7 (3-49) | 29 | | | |
| Strategy.StatusRedistribute | | 22.0 | 0 % | 26.5 (3-136) | 45 | | | |

By phase (asks per game): action 1,630, agenda 183, status 92, strategy 50. Tree nodes per game: 577 (draft 45, menu 166, closing
140, activation 80, move 124, scoring 21). "smart = false" is the share of answers where the rule bot had no opinion and its random
pick stood (`choose_explained_view`'s flag).

### The Activate hard filter

| | Tactical.Activate | TeWarfareTactical |
|---|---|---|
| options before / after | 36.3 / 17.4 | 37.1 / 15.2 |
| kept by reason (per ask): move / produce / shoot | 16.7 / 1.3 / 2.2 | 14.2 / 1.4 / 1.2 |
| nothing survived (pass-through) | 0.0 % | 0.4 % |
| **the rule bot's pick is dropped by the filter** | **0.93 %** (140 of 15,040) | **9.9 %** (123 of 1,240) |
| the rule bot's picks with reason move / produce / shoot | 92 / 16 / 16 % | 76 / 30 / 15 % |

The filter halves the activation (36 -> 17), not the 5-12 the plan expected: with movement 1-2 ships and the Learn-to-play map's
size, most systems are reachable by some ship. The rule bot sometimes activates a system for none of the three reasons (an
empty system to place a token? a Warfare action to pull back a token): 1 % of activations, 10 % of the Warfare free tactical
actions.

### Segments between tree nodes

| delegated asks between two tree nodes | share |
|---|---:|
| 0 | 57.0 % |
| 1 | 13.1 % |
| 2 | 10.0 % |
| 3-4 | 7.8 % |
| 5-8 | 6.0 % |
| 9-16 | 3.5 % |
| 17-32 | 1.1 % |
| 33+ | 1.5 % |

Mean 2.4 delegated asks per segment. The long segments are the status phase (token gain, redistribution, scoring), combat and the
agenda phase.

### Chance (the committed dice of the ledger)

141 roll requests and 221 dice per game, 1.6 dice per request; the largest request is 10 dice (n + 1 <= 11 outcomes).

| purpose | requests / game | dice / request | dice-count histogram (all games) | faces read beyond a threshold |
|---|---:|---:|---|---:|
| SpaceCombat | 52.8 | 1.47 | 1:6859 2:2761 3:665 4:202 5:60 6:10 7:2 | 0 |
| GroundCombat | 36.4 | 1.58 | 1:4273 2:2111 3:673 4:173 5:34 6:10 7:9 8:2 10:1 | 0 |
| SpaceCannon | 28.1 | 1.41 | 1:4040 2:911 3:610 4:53 | 0 |
| Reroll | 10.1 | 2.05 | 1:640 2:893 3:313 4:117 5:18 6:27 7:7 | 0 (all have the reroll mark) |
| Bombardment | 7.6 | 1.84 | 1:745 2:496 3:78 4:209 | 0 |
| AntiFighterBarrage | 4.7 | 2.03 | 2:920 3:28 | 0 |
| Other | 0.8 | 1.39 | | 0 |
| Ability | 0.3 | 2.37 | 1:2 2:39 3:27 | 57 % (no `hits_on`) |

Every combat-type request has a `hits_on`: hit counts are all the rules read, so the plan's hit-count buckets are exact for
99.8 % of the requests. The "Ability" requests (0.3 per game; 57 % of them without a threshold, the faces themselves are read) are
the open-loop `sample` case of the plan. The 10 reroll requests per game follow `Combat.RerollDie` (4.9 asks per game, a delegated
ask): which dice to reroll is a rule-bot answer, and the reroll is a new request, so it is an ordinary chance node.

### Wire / core option order

Over all 391 k asks: the rendered option count equals the core `Opts` count every time (0 misaligned), and for every activation
(`Tactical.Activate`, `TeWarfareTactical`) and draft ask every rendered option id names the core option at the same index
(0 misnamed). The index carries over (risk R4 holds on this line). L2 should keep the assertion on every record.

### Canonical move order

78.8 movement steps per game (15,769 in total); moves per step: 0 (done at once) 22 %, 1: 63 %, 2: 10 %, 3+: 4 %.

| | share of movement steps |
|---|---:|
| the rule bot's order is already canonical | 93.8 % |
| not canonical, but the same set of ships is reachable in canonical order (replayed from the step's fork) | 6.0 % |
| **the set is reachable only out of canonical order** | **0.14 %** (22) |
| replay inconclusive | 10 |

Canonical key: (capacity providers first, origin, kind, damaged). The replay forks the position at the start of the step of the
first move, replays the answers before it, then moves the same ships (by origin, kind, damaged, boosts) always taking the smallest
offered key, the rule bot answering the loads. R6 is small: keep the canonical order in v1.

### Results of the games (rule bot vs rule bot, 200 games)

Wins: Sol 89 (44.5 %), Letnev 53 (26.5 %), Jol-Nar 36 (18 %), Xxcha 22 (11 %). Mean VP: Sol 8.1, Letnev 7.4, Jol-Nar 6.9, Xxcha 6.2.
A strong faction imbalance for the rule bot: the benchmark must report per faction (as planned), and "25 %" is not the baseline of
any single faction.

## What changes in the plan

1. **The rule bot is the cost, not the engine.** 80 % of a rule-bot game is the bot (77 % of it on the tree-kind asks: menu 3.9 ms,
   activation 4.3 ms, draft 3.8 ms each). Inside the tree those are answered by the net, so a simulation costs about **1.4 ms**
   (engine 0.67 + delegated rule bot 0.64 + fork 0.004 + net). Section 2.7 of the plan: 11 s per searched game instead of 20 s.
2. **Delegated planning asks have a long tail**: `Production.Choose` (2.2 ms, p99 30 ms, 78 per game) and
   `Strategy.ResearchTechnology` (2.6 ms) are the only expensive delegated kinds. R1's mitigation applies to them: inside the
   tree, cache or simplify (first a per-segment plan cache; a cheap default only if that is not enough).
3. **Thunder's Edge Warfare's free tactical action is an activation** (`Strategy.TeWarfareTactical`, 6.2 per game, 37 systems):
   the product's default card set is `te`, so the table of 2.2 needs this row. Done in `ti4-mcts-features` (`TreeKind::Activate`,
   filtered like `Tactical.Activate`).
4. **The filter is weaker than expected (36 -> 17, not 5-12)** and drops the rule bot's pick in 0.9 % of activations and 9.9 % of
   the Warfare actions. L2: record those positions with the pick added back to the kept set (or drop the record), and count it. A
   tighter v1.1 rule (a move counts only when it changes something: enemy units, a planet to take, a threatened own system) can be
   measured with the same tool once the net exists; not before, because it would cut moves the rule bot makes.
5. **`Turn.Closing` is 99 % random** (the rule bot has no opinion; options include component actions, so the bot takes random
   component actions at the end of its turn). It is 25 % of the tree nodes (140 of 577). For imitation it carries no policy target
   (weight 0, as planned). Proposal: keep it a tree node (the search can learn it) but give it the reduced simulation count
   (n / 2, as `Move`); alternatively answer it with a fixed "end turn" in v1. Decision for the user (Q4 below).
6. **`Tactical.Move` is 20 % single-option** (only `DoneMoving` left): those are not nodes. 124 move nodes per game.
7. **Segments are short**: 57 % of tree nodes follow another directly, mean 2.4 delegated asks. The tree is deep in decisions per
   unit of engine time; tree reuse (2.6) matters more than transpositions.
8. **Chance**: hit-count buckets exact for 99.8 % of requests; at most 11 outcomes per node; "Ability" requests (0.3 per game) are
   sampled open-loop. No change, it confirms 2.3.
9. **Game length**: 1,955 asks per game (plan: 1,570), 1,752 steps, 6.6 rounds; 577 tree nodes (plan: 400-600).
10. **Faction imbalance** in the rule-bot baseline (Sol 44.5 % wins): report every result per faction; the 25 % baseline is only for
    the faction-balanced aggregate.
11. **State outside the position**: the rule bot keeps a thread-local movement memory (`ti4_rule_bot::backward` gather/pile)
    between decisions. A search that answers delegated asks in many branches must save it per node (or restore it before each
    rule-bot answer from the node). The lab saves and restores it around every replay.

## Open problems

* **P1: no "no action cards" / "no exploration randomness" switch** in the setup or the core. The census runs the product rules
  (action cards in hand, reaction windows, exploration decks, relics). The target setup needs a table flag (deck sources without
  action cards and exploration, or the core skipping draws) owned by a setup/engine lane; until then L2 data is the product game.
* **P2: rule bot on `SeenView` vs on `Observed`.** The census answers with the typed view; the product answers with the string
  `Observed`. They are proven equal by the shadow harness (`bot_shadow` ViewDiff), not re-proven here. The lab also passes the
  wire choice without the table's `about_invasion` context and the game's starting galaxy (laws that change lanes later are not
  re-applied to the `SeenView` galaxy).
* **Q4 (user)**: `Turn.Closing` as a tree node with half the simulations (default), or a fixed answer in v1?
