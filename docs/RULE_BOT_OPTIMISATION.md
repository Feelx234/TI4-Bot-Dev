# Rule bot optimisation: headless self-play

`selfplay` plays complete games with the rule bot on every seat: no browser, no server, no ports,
no libtorch. It is the entry point for improving the bot (`crates/ti4-rule-bot`): play many games
on fixed seeds, look at what the bot does and where it loses, change something, play the same
seeds again, compare.

```text
tools/rulebot/selfplay.sh smoke        # 10 games, ~10 s on 8 cores
tools/rulebot/selfplay.sh bench        # the golden set, 200 games, ~80 s on 8 cores, checked against baseline.json
tools/rulebot/selfplay.sh ab eager_worth=2,win_margin=0.8 100   # candidate vs shipped bot, 100 tables of 4p and 6p
tools/rulebot/selfplay.sh complete     # time to completion, 3 to 8 players, 19 maps, no cutoff
tools/rulebot/selfplay.sh replay --seed 17 --players 4 --text   # one game, every decision
```

The script builds `cargo build -p ti4-selfplay --release` (about one minute cold) into
`$SELFPLAY_TARGET` (default `<repo>/target`) and writes results to `<target>/selfplay-out/`.
Use `nice` / a private `CARGO_TARGET_DIR` on a shared machine (the script already uses `nice`).
The binary is `selfplay` (`cargo run -p ti4-selfplay --release -- run ...`).

## What a game is

One `GameSpec` is everything a game is made from: seed, number of players, map (template alias or
`random`), the faction of every seat, speaker (`seat`, `random` or a seat number), card set. It is
printed for every problem and stored for every game in the summary (`games[].spec`), and
`selfplay replay` takes it back as flags. Same spec, same build: identical game (checked with a
hash of the decision log, the `decisions_hash` that saves of the browser host carry).

The game is driven exactly as the browser host drives a rule seat (`ti4-wasm`: `Seat::decide` and
`draw`): one decider answers every seat. A question with the position in hand
(`Decider::choose_seeing`) goes to `ti4_rule_bot::choose(choice, seen, seed, seat, n)` with `n`
the number of the decision over all seats; a question without goes to one shared seeded random
stream. Rounds, decisions, victory points and the end of the game are the engine's (`GameState::finished`, ten points; the winner is
`ti4_engine::objectives::leader`, which breaks ties by initiative).

Not the same game as a table on the server: the server's `RandomBotDecider` counts decisions per
seat, the offline host and this tool count them over the table. A game of a human against bots
cannot be replayed here, and a game here cannot be replayed with `replay_save` (that takes the
answers of one human seat); use `selfplay replay` and the hash.

## Setups: maps, factions, seats, seeds

One setup per `--players` value (default `4,6`), or a matrix file (`--config`, JSON) with several.
All flags below are fields of a setup in the file (`tools/rulebot/matrix-example.json`).

| flag | meaning |
|---|---|
| `--players 3,4,5,6,7,8` | table sizes; one setup each (`4p`, `6p`, ...) |
| `--games N --seed-start S` / `--seeds 1..50` / `--seeds 3,5,9` | seeds (`a..b` excludes b, `a..=b` includes it) |
| `--map 4pHyperlanes,4pOuterRim,random` | template aliases (`selfplay maps --players 4`) and/or `random`; default: the recommended template. A map that does not build for the size is skipped and listed under `skipped` |
| `--map-policy cycle` / `random` / `cross` | maps share the seeds: seed `i` plays map `i mod n` (default), each seed draws its map from the seed, or every map for every seed |
| `--factions sol,_,hacan` | fixed seats in seat order (`_` free); the rest are drawn without duplicates |
| `--faction-pool a,b,c,...` | the free seats are drawn from this pool |
| `--rotate cyclic` (alias `latin`) / `all` / `none` | each table is also played with its factions shifted over the seats: `n` games per seed (every faction in every seat once), or all `n!` permutations (up to 6 seats). Separates seat from faction effects |
| `--speaker seat` / `random` / `2` | opening speaker (default `random`: drawn from the seed, as in the product) |
| `--card-set te|pok|base_game_codex1` | strategy card set (default: the host's default) |
| `--max-rounds 50 --max-steps 400000 --max-seconds 300 --loop-steps 5000` | limits (below) |

Faction draws without a pool are the lobby's own (`ti4_engine::seating::lobby_faction_draw`), so an unconstrained
setup is the game the product makes. Maps are those of `ti4-host` (`maps::catalog`); 3 to 8 players work.

A matrix:

```json
{ "limits": {"max_rounds": 50},
  "bot": {"win_margin": "0.7"},
  "setups": [
    {"name": "latin4", "players": 4, "maps": ["4pHyperlanes"], "factions": ["sol","hacan","xxcha","l1z1x"],
     "rotate": "cyclic", "seeds": "1..11", "speaker": "seat"},
    {"name": "maps5", "players": 5, "maps": ["5pHyperlanes","5pOuterRim","random"], "map_policy": "cross",
     "faction_pool": ["sol","hacan","xxcha","l1z1x","jolnar","naalu","yin"], "seeds": "1..4"} ] }
```

Each setup is aggregated on its own and all together (`setups` and `combined` in the summary).

## Termination

`victory` (ten points), `cutoff_rounds` (the round after `--max-rounds` was reached, default 50; a
cutoff, not an error), `cutoff_steps`, `stuck` (the same question answered the same way
`--loop-steps` times in a row, or `--max-seconds` of wall time), `engine_error` (the engine refused
a step), `panic` (caught; message and place in `detail`), `setup_error`. `problems` in the summary lists
everything but victory and `cutoff_rounds`, each with the command that plays it again.
For a time-to-completion measurement the cutoffs are set so high (`tools/rulebot/completion.json`)
that only a real hang ends a game.

## The summary

`selfplay run --out summary.json` writes JSON and prints the table. Per setup and combined:

- `terminations`, `cutoff_rate`, `rounds` / `decisions` / `wall_s` (min, mean, median, p90, max);
- `win_by_seat` and `win_by_faction`: games, wins, win rate with 95% Wilson interval, mean points
  (finished games only); `winner_vp`, `final_vp` histograms, `zero_vp_seat_rate`, `tied_at_top`;
- `completion` / `by_players` / `by_map`: time to completion (finished games), `us_per_decision`;
- `phase_ms_mean` (engine step wall time by the phase the step began in, bot time included) and
  `bot_share` (share of wall time spent inside the bot's decisions; the rest is the engine);
- `by_subtype`: the **profiling hook and decision-type histogram**. Per kind of question
  (`DecisionContext::subtype`, else `~<option kind>`): `n`, `smart` (the bot's own rule chose,
  not the random pick it falls back on), `ns` in the decider. The table shows the top by time;
- `usage_mean`: how often each notable option is taken per game: `action_menu=tactical|strategic|pass|...`,
  `draft_strategy_card=...`, `research_technology=...`, `score_objective=...`, `reaction=<card or ability>`;
- `objective_rate`: share of seat-games that scored each objective; `vp_source_mean`: points per
  game by source (the engine's `vp_ledger`: objective, secret_objective, imperial_primary, support_for_the_throne, custodians, ...);
- `problems`, `skipped`, `meta` (git rev, cores, workers, release/debug, wall time, games per hour);
- `games`: a compact line per game (spec, termination, winner, points, rounds, decisions, wall ms, hash).
  `--games-out FILE.jsonl` writes the full per-game results (the metrics above per game).

## Reproduce and inspect one game

```text
selfplay replay --seed 17 --players 4 --map 4pHyperlanes --factions sol,jolnar,hacan,titans --speaker random \
    --text --from 0 --to 80 --seat a --trace /tmp/g.jsonl --expect-hash bd3aca6e68e88918
selfplay replay --summary summary.json --seed 17 [--setup 4p --nth 0]     # take the spec from a summary
selfplay replay --spec '{"setup":"x","seed":17,...}'
```

`--text` prints one line per decision: number, round, phase, seat, question (`subtype`), `rule` or
`random` (did the bot's own rule choose, or the random pick), the chosen option id, how many
were offered, and the prompt. `--trace` writes the same as JSON lines after a header with the spec and hash.
`--bot key=val` replays with a changed bot. The rule bot gives no written reason beyond this:
to see why a rule fired, look up its question in the map below. `selfplay run --trace-dir DIR` writes traces of games
that did not end in victory (`--trace-which all` of every game).
Exit code 2 when `--expect-hash` differs.

## A/B: baseline against candidate

```text
selfplay compare --players 4,6 --games 200 --seed-start 20000 --candidate eager_worth=2,win_margin=0.8
selfplay compare ... --candidate-config cand.json --baseline win_margin=0.5   # either side may carry overrides
selfplay compare ... --rotate cyclic --mode mixed|separate
```

`mixed` (default): each table (the same spec for both) is played twice. In the first game the even
seats play the candidate and the odd seats the baseline, in the second they swap, so every seat
and faction plays both bots. For each pair: the share of the two games won by a candidate seat,
minus 0.5 (`win_share_advantage`), and the points of the candidate's seats minus the baseline's.
Output: mean and 95% interval (normal approximation over pairs, so use at least 100 tables;
pairs where a game did not end in victory are dropped and counted) and a verdict. Equal bots give exactly 0
(there is a test). Sanity check: `objectives_enabled=false` loses about 17 points of win share on 60 4p tables.
`separate`: the table is played once with the baseline on every seat and once with the candidate on every seat;
use it for length, speed and finish rates, not for strength.

### The knobs

`selfplay knobs` lists them with defaults. They are `ti4_rule_bot::tuning`: the numbers that were
constants (`win_margin`, `retreat_margin`, `activation_threshold`, `eager_worth`, `accept_net`,
`mecatol_bonus`, `expansion_rounds`, `objective_round`, `complete_value`, `progress_value`,
`deny_value`, `short_bonus`, ... ) plus two switches (`objectives_enabled`, `faction_plans_enabled`).
Set with `--bot key=val,key=val` / `--bot-config file.json` (`{"win_margin": 0.7}`), `bot` in a
matrix, or `--candidate`. Overrides are per thread (`tuning::set`, `tuning::with`), so a baseline
and a candidate play in one process, even in one game. The server and the browser never set
them: the defaults are the shipped bot, byte for byte (60 saved nightly games replay to the
same hashes on the build before and after the hook; the golden baseline below checks it again).
To add a knob: add the constant's default and a line in the `tuning!` list in
`crates/ti4-rule-bot/src/tuning.rs`, and read `tuning::name()` where the constant was used.
A sweep is a loop over `selfplay compare`; the JSON (`--out`) has the estimate and interval.

## Golden benchmark and baseline

- `tools/rulebot/golden.json`: 200 games, 4 players seeds 1..100 on `4pHyperlanes` / `4pOuterRim`, 6 players seeds 1..100 on `6pStandard` / `6pOuterRim`.
- `tools/rulebot/baseline.json`: its summary for the shipped bot, recorded on the build named in `meta.git_rev`
  (AMD EPYC-Genoa, 8 cores, release build, 7 workers, 82 s, other jobs running on the machine).
- `tools/rulebot/selfplay.sh bench` runs the set and compares every game's hash, termination and points with the baseline
  (`--verify`): **identical** is the gate for "I did not change the bot or the engine"; **different** is expected
  after a deliberate bot change (judge it with `compare`) and is a regression if the change was meant to be
  a refactor, or if it came with a rebase of the engine. After an accepted change, rerun `selfplay.sh baseline`
  and commit the new file on its own, saying why it changed.
- `tools/rulebot/completion.json` / `completion-baseline.json`: 6 table sizes, 19 maps, no cutoff (below).
- `tools/rulebot/stress.json`: 340 games over every template of every size plus `random`, for hunting stuck games and panics.

## Time to completion

Measured with `tools/rulebot/completion.json` (3 to 8 players, 19 maps, `--max-rounds 1000`, no step cap, 900 s per game
safety net), release build, AMD EPYC-Genoa 8 cores, **one worker**, while other jobs kept the machine at load average 5 to 11
(so single-game times are somewhat pessimistic; the 8-core run is slower per game than 1 worker by about 10%).
`wall` is seconds per game, `us/dec` the mean wall time per decision, `rounds` / `decis.` per game. The table is also in
`tools/rulebot/completion-baseline.json`.

```text
by player count        games  done wall mean   median      p90    max s   rounds   decis.   us/dec    (rate)
3p                        18    18     0.90     0.89     1.28     1.43     6.61     1215      739      100%
4p                        35    35     1.91     1.85     2.63     3.27     6.94     1930      990      100%
5p                        15    15     2.16     1.98     3.15     3.81     7.13     2011     1072      100%
6p                        20    20     3.35     3.40     4.44     5.07     7.25     2626     1277      100%
7p                         8     8     5.20     4.79     6.45     6.58     6.88     3133     1659      100%
8p                         8     8     8.23     8.27     9.95     11.1     7.75     3658     2249      100%

by map                 games  done wall mean   median      p90    max s   rounds   decis.   us/dec    (rate)
3pBgaBalancedD             6     6     0.85     0.91     1.01     1.08     6.50     1176      721      100%
3pHyperlanes               6     6     0.92     0.80     1.42     1.43     6.67     1224      753      100%
3pLearnToPlayPoK           6     6     0.92     0.89     1.14     1.22     6.67     1244      742      100%
4pBgaBalancedD             7     7     2.21     2.48     2.94     3.27     7.00     2041     1085      100%
4pExpanse                  7     7     1.58     1.53     1.86     1.89     7.14     1869      847      100%
4pHyperlanes               7     7     1.94     1.88     2.37     2.69     6.86     2009      965      100%
4pLearnToPlayPoK           7     7     1.97     2.09     2.54     2.82     7.00     1940     1016      100%
4pOuterRim                 7     7     1.85     2.02     2.36     2.37     6.71     1789     1032      100%
5pBgaBalancedD             5     5     2.20     1.88     3.15     3.35     7.20     2000     1099      100%
5pHyperlanes               5     5     2.31     2.08     3.19     3.81     7.20     2138     1082      100%
5pOuterRim                 5     5     1.96     1.56     2.70     2.76     7.00     1894     1034      100%
6pBgaBalancedD             5     5     3.08     3.42     3.80     4.04     7.40     2493     1236      100%
6pGravity                  5     5     3.63     3.63     4.69     5.07     7.40     2711     1339      100%
6pOuterRim                 5     5     3.29     2.92     4.35     4.38     7.40     2668     1233      100%
6pStandard                 5     5     3.42     3.38     4.58     4.93     6.80     2633     1298      100%
7pAnaphase                 4     4     5.52     5.42     6.39     6.58     7.00     3184     1734      100%
7pHyperlanes               4     4     4.87     4.45     5.81     6.39     6.75     3081     1581      100%
8pBeMyNeighborHyperlanes     4     4     8.74     9.26     10.6     11.1     7.25     4000     2185      100%
8pSpoonMap                 4     4     7.72     7.50     8.56     9.00     8.25     3317     2326      100%
```

**All 104 games finished with a victory; none hit any cap, hung, panicked or hit an engine error.** Games end after
6.6 to 8 rounds (about 1200 decisions at 3 players, 1900 at 4, 2600 at 6, 3700 at 8).
Time per game grows faster than linearly with the players (0.9 s at 3, 1.9 s at 4, 3.4 s at 6, 8.2 s at 8): decisions per game grow
with the players, and so does the cost of each decision (the engine builds the legal options of a larger board).

Throughput (games per hour, same machine, other jobs running):

| | 1 worker | 8 workers | speed-up |
|---|---|---|---|
| 4 players (40 games, 4pHyperlanes, no cutoff) | 2084 | 14366 | 6.9x |
| 6 players (24 games, 6pStandard, no cutoff) | 981 | 6219 | 6.3x |
| the completion mix above (104 games) | 1295 | 8708 | 6.7x |
| golden set (100 x 4p + 100 x 6p, 7 workers) | | 8800 | |

Games are independent and the results do not depend on the number of workers (checked: identical hashes at 1 and 8 workers).

## Where the bot lives (`crates/ti4-rule-bot/src`)

- `lib.rs` - `choose` / `choose_explained` (entry; random pick first, then the rule), `pick` (dispatch on
  `choice.context.subtype`), and the rules: `pick_action` (action menu), `activation_value` / `pick_activation`,
  `pick_move`, `pick_load`, `pick_commit`, `pick_retreat`, `pick_produce`, `pick_pay`, `pick_strategy_card`,
  `pick_token_pool`, `pick_research`, `pick_redistribution`, `pick_answer` / `pick_proposal` (trades), `expanding` (rounds 1-2).
  The module comment at the top lists every opinion. Questions `pick` returns `None` for stay random
  (the table shows them as `smart` < 100%).
- `objectives.rs` - from round 3: which objectives to score and aim at, `gain` of an action, denying a rival near ten points.
- `faction_plans.rs` - per faction technology order, unit weights, preferred strategy card.
- `agents.rs` - when to use faction agents and other component actions.
- `random.rs` - the seeded random pick under the rule bot (`decision_rng(seed, seat, n)`).
- `tuning.rs` - the knobs.

How the seats are driven elsewhere: `ti4-server/src/session/random_bot.rs` (online tables), `ti4-wasm/src/lib.rs`
`Seat::decide` / `draw` (the offline game, the nightly offline mode) - this tool copies the latter.
Other old tooling, not used here: `ti4-sim` (an older simulation and benchmark crate), the MLP / review crates (need libtorch).

## Rules for safe optimisation

1. **Determinism.** `choose` must stay a pure function of (position, seed, seat, decision number). No clock, no thread-local state
   other than `tuning`, no hash-map iteration order, no global counters. The random pick is always drawn first so the stream is the same whether or not a rule
   overrides it; keep that.
2. **No hidden information.** A rule reads only the `SeatObservation` it is given (public board plus the bound seat's own hand and
   secret progress). Do not reach into `GameState` through another path, and do not read what another seat holds
   (hands, secret objectives, the decks' order).
3. **Every answer is one of the offered options** (the index returned is always `< options.len()`); a rule that
   cannot decide returns `None`.
4. **Judge changes on paired seeds**, with `compare --mode mixed`, at least 100 tables each of 4 and 6 players (more for
   effects under 5 points); look at the interval, not the mean. Also run with `--rotate cyclic` once, to see that a change
   does not just help one faction or seat. Check `terminations` (no new cutoffs, stuck or panics) and games per hour.
5. **Gates before a commit:** `cargo test -p ti4-rule-bot -p ti4-selfplay`; `selfplay.sh bench` identical to the baseline
   when you did not mean to change behaviour; `selfplay run --config tools/rulebot/stress.json` with `problems: none`
   when you touched movement, production or anything that loops; for any change in `ti4-rule-bot` that must not change the
   default bot, `replay_save` on saved games gives the same hashes before and after.
6. Keep `crates/ti4-rule-bot` free of dependencies that do not build for `wasm32-unknown-unknown` (the browser links it).
7. A new rule goes behind a knob first (`tuning.rs`, default = old behaviour), is compared, and only then becomes the default
   together with a new baseline.

## Adding a metric

Per game: a field in `GameResult` (`play.rs`), filled in `play()` from the final `GameState` (`game.state`), the decision
log (`game.table.log.records`) or in the decider (`SelfPlay::decide`, which sees every question and answer; the
`Shared` struct holds the counters). Per run: a field in `Agg` (`stats.rs`), filled in `aggregate`, printed in `agg_text`.
Counting a chosen option by name: add its question to `usage_key` (`play.rs`). Add a line to the schema test in
`crates/ti4-selfplay/tests/selfplay.rs` if the field must stay.

## First data and where to start

Data from the golden set, a 340-game stress run (every template of every size, random maps, 3 to 8 players), 300 more games
with the `pok` and `base_game_codex1` card sets, and a 240-game Latin square (6 players, `--factions jolnar,saar,sol,hacan,yssaril,muaat --rotate cyclic`,
`6pStandard`, 40 seeds).

**Health.** About 1000 games in all: no stuck game, no engine error, no panic, no cutoff. Nothing to fix in the engine was found.
The only oddities: a game can end with a winner on 7 to 9 points (7 of 200 golden games: 1 on 7, 6 on 9; the public objective deck ran
out, or the last objective was scored), and the top two seats were tied on points in 11 of 200 golden games (the tie goes to initiative).

**Where the time goes.** The bot is only about 26% of the wall time; the rest is the engine (building the legal options of each
question). Two questions are 82% of the bot's own time: `action_menu` (49%, about 1.9 ms each) and `activate_system` (33%, 2.9 ms each; it values every
system on the board). Everything else is under 2.5% each. Speeding the bot up means `activation_value` / `pick_action`; speeding the
games up beyond that means the engine's `legal_options`.

**Strength and balance (what a first round should look at).**

- Seats are fair: win rates by seat are flat (the Latin square: 15 to 20% for 6 seats, expected 16.7%).
- Factions are not. Latin square, same map and the same table with the factions shifted over the seats: sol 29%, saar 19%, jolnar 17%,
  muaat 12.5%, hacan 11%, yssaril 11% (95% intervals about +-5 points). Over the golden set (24 factions, 29 to 58 games each,
  intervals +-12): jolnar 41%, saar 38%, mentak 33%, titans 31%, sol 28% against yssaril 8.5%, l1z1x 9%, nekro 9%, hacan 10%, arborec 10%.
  A faction that wins rarely is a faction whose plan (`faction_plans.rs`) or abilities the bot plays badly: start with yssaril,
  hacan, muaat, nekro, l1z1x, naaz, mahact, empyrean.
- The objective layer matters: `objectives_enabled=false` loses 17 points of win share (CI -24 to -9). Scoring comes from public
  objectives (19 points per game over all seats), secret objectives (7), Imperial (1.8), Support for the Throne (3.3). The rarely
  scored public objectives (`ose`, `dfat`, `command_armada`, `become_legend`, `master_science`, `fwp`: under 0.3% of seat-games)
  are ones the bot never aims at.
- Questions where the bot has no opinion and answers at random (share `smart` under 5%, number of decisions in 340 games):
  `end_turn` 55k, `vote_exhaust_planet` 30k and `cast_vote` 11k (the agenda phase: the bot votes and spends influence at random), `ready_planet` 7k,
  `fight_ground_combat_round` 6.7k, `trade_choose_replenish` 4.7k, `reaction_after_AGENDA_REVEALED` 4.3k, `sustain_damage` 4k,
  `assign_casualty` 3.1k, `politics_place_agenda` 3.1k. Voting, casualty assignment and sustain damage are the cheapest places to add
  a rule; in the stress run `agenda` points are only 0.4 per game, so the agenda phase is nearly irrelevant to the winner now.
- Action menu per game over all seats (340 games): tactical 82, strategic 35, pass 34, action cards 6. The strategy card draft takes
  technology, construction, imperial, leadership, warfare most; politics and diplomacy least.
- Games end by round 7 on 10 points almost always; the bot is fast and aggressive early. Whether that is good play against humans is
  not measurable here (no cutoff ever happens, so cutoff rate is not a useful signal yet).

**Suggested first optimisation round.** (1) Sweep the knobs with `compare` (`win_margin`, `activation_threshold`, `eager_worth`,
`complete_value`, `progress_value`, `short_bonus`, `mecatol_bonus`, `expansion_rounds`) in one-at-a-time steps, 200 tables each.
(2) Add rules for the random questions above, starting with agenda voting and casualty/sustain choices.
(3) Rework faction plans for the weak factions, judged with `--rotate cyclic` setups for the faction in question.
(4) Cut `activate_system` / `action_menu` time if the sweeps need more games per hour.
