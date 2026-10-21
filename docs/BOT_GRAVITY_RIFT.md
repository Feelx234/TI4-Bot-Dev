# Rule bot: gravity-rift risk (bot version 6)

Knobs: `rift_risk` (on in `tuning::v6()`, off in v1 to v5), `rift_gain_scale` (1.0), `rift_margin` (3.0),
`rift_flagship_weight` (1.0). Code: `crates/ti4-rule-bot/src/rift.rs`. `BOT_VERSION` is 6 since 2026-10-21 (on in `v6()`).

## The rule (as the engine plays it)

`crates/ti4-engine/src/transit.rs` (41.2): a ship rolls **one die for every gravity rift its route exits**
(the system it starts in, and every system it passes through; the system it ends in is never exited). A roll
of 1 to 3 on the ten-sided die destroys the ship **and everything aboard** (95.1b). One exit costs the ship 30 %,
two exits 51 %, three 66 %. Exits are skipped when the mover ignores anomalies (Nav Suite), the Circlet of the Void,
or a faction exemption (`rift_roll_exempt`). A rift also gives the ship +1 move (41.1), and Gravity Drive adds a step
(its option is a different move with its own route). The engine picks the route; the player chooses only which ships
sail and what they carry. Cargo is picked up from the origin and from every system on the route.

## What the bot did (v5)

It ignored rifts almost entirely. The only guard was `rift_caution` (off in every version): outside a fight, no move that
exits a rift. Self-play, 4p `4pBgaBalancedD`, 200 games, v5 on all seats: 6.5 rift dice per game, 1.84 ships (with their cargo)
destroyed per game, 8.9 resources of units lost per game (1.8 ships, 2.2 ground forces, 0.2 fighters), 21 rift moves offered
per game of which 6.5 were played. Fleets that park in a rift (tiles 41 and 67, and the Cabal home) pay at every departure.

## The model

Every ship that could sail into the active system is *safe* (no exit) or *risky* (`Observed::rift_exits_into`, which runs
the engine's own pathing). For a risky ship:

```text
loss(ship) = P(destroy) * (cost(ship) + cost(cargo)),   P = 1 - 0.7^exits
cargo      = the ground forces (fighters too, in a fight) it could take from its origin, as many as it has room for
cost       = printed cost (infantry and fighters 0.5 each); flagship and war sun times rift_flagship_weight
```

The gain of a risky ship is the worth it adds to the activation: `activation_value` with the chosen set plus this ship
minus without it (negative values count as 0), times `rift_gain_scale`. Admission (`rift::prune`):

1. all safe ships are in the set;
2. risky ships are added one at a time, the best net first, while `rift_gain_scale * added worth - rift_margin * loss > 0`;
3. if no single ship pays, the cheapest prefix of the rest is tried as a group (a fight that needs two rift ships to be won).

A ship that adds nothing (the fight is won by the safe ships, the planet is taken by a safe carrier) is never admitted;
among equals the cheapest expected loss goes first, so fighters and destroyers before carriers with cargo and the war sun last.

The same set is used in three places so they cannot disagree:

- **movement step** (`rift::keep`, before `pick_move`): the move options of risky ships outside the set are removed; the
  existing logic then picks among what is left (another carrier, a safe warship, or "finish movement");
- **fight check in `pick_move`**: the ships that may still come exclude the dropped ones, so the bot does not enter a fight
  that only the dropped ships could win;
- **activation** (`movable_for`, so also the objective and backward-planning layers): dropped ships do not count as reachable,
  and `activation_value` loses the expected loss of the admitted risky ships (`loss / rift_gain_scale`), so an equal target
  behind no rift is taken first.

The engine supplies the facts without changing any answer: `Choice::details["rift_exits"]` (option id to exits) on the movement
step, `rift_exits` on the load step, and `Observed::rift_exits_into`. Details are display-only and absent when no route exits a
rift, so the other bot versions and all hashes are unchanged (benches 200/200).

## Parameters and how they were chosen

`rift_gain_scale * worth` reads a point of activation worth as resources. Margin 3 means the gain must be three times the
expected loss: a loss is not only its price (the tempo, the production slot, the fleet supply, the fight that is then not won).
Four-player A/B (100 tables each, 4pBgaBalancedD, win share / VP per seat; resources lost per seat-game in brackets):

| scale, margin | win share pp | VP per seat | lost |
|---|---|---|---|
| (v5) | | | 2.2 |
| 3, 1 | 0.0 [-5.0, +5.0] | +0.08 | 1.7 |
| 1, 2 | +3.5 [-2.8, +9.8] | -0.05 | 0.9 |
| **1, 3** | 0.0 [-6.1, +6.1] | -0.16 | 0.5 |
| 1, 4 | -4.0 [-10.3, +2.3] | -0.37 [-0.65, -0.08] | 0.3 |

The table is a screening run (100 tables, rule code of that day, the loss column from a separate 200-game run); margin 4 starts to cost points (it refuses crossings that pay); 3 is the strictest setting without a measurable cost.

## Examples (unit tests in `rift.rs`)

- A carrier with two infantry sits in a rift tile and an identical one in a plain tile; both can reach the planet: only the
  plain one is offered.
- A dreadnought in the rift, nothing to win at the destination: the move is removed, "finish movement" is left.
- The same carrier with two infantry for the richest planet of the corpus: allowed. With four infantry aboard (larger loss) and
  a margin between the two losses it stays home: cargo is counted.

## Results (final A/B, v5 against v5 + `rift_risk`, seat-swapped pairs)

4pBgaBalancedD, 400 tables (800 games, random factions): win share -0.1 pp [-3.1, +2.9], VP per seat -0.01 [-0.14, +0.13], VP per round 0.00 [-0.02, +0.02].
6pStandard and 6pOuterRim, 100 tables: +1.5 pp [-2.0, +5.0], +0.08 VP [-0.08, +0.25]. Neutral on strength.

Rift losses per seat-game (4p, 1600 seat-games each side; 6p in brackets):

| | v5 | v5 + rift_risk |
|---|---|---|
| rift dice rolled | 1.50 (0.84) | 0.47 (0.45) |
| ships destroyed | 0.43 (0.27) | 0.13 (0.12) |
| of which dreadnoughts, flagships, war suns | 0.18 (0.10) | 0.02 (0.04) |
| ground forces lost aboard | 0.53 (0.33) | 0.17 (0.18) |
| resources of units lost | 2.16 (1.38) | 0.49 (0.52) |
| moves that exit a rift (played / offered) | 1.49 / 4.83 | 0.47 / 3.82 |

Per 4-player game that is 1.7 ships and 2.1 ground forces destroyed by rifts before, 0.5 and 0.7 after.
