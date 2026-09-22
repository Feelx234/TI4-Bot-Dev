# Trade arena: measurement and draft value sheet (2026-09-22)

Status: **values decided by the operator (2026-09-22), section 4.** Nothing here is implemented yet.

Idea (operator, 2026-09-22): diplomacy slows full games and the policy trades badly. Pretrain the
diplomacy head in an arena where seats conduct one negotiation and are scored by heuristic values,
as the battle arena pretrains fleet evaluation. Full-game PPO then refines it.

## 1. What diplomacy costs now

Self-play, near-greedy (T=0.001), holdout maps, 4 rounds, 72 games per row (12 seeds x 6
rotations), `vp_sources --self-play [--diplomacy]`, seed base 910005000. Wall time is with 24
games running in parallel, so compare rows, not absolute seconds.

| Checkpoint | Diplomacy | Decisions/game | Diplomacy decisions | Share of decisions | Share of decide time | Wall s/game | Table VP |
|---|---|---|---|---|---|---|---|
| 15508 | off | 1,555 | - | - | - | 2.4 | 2.39 |
| 15508 | on | 2,273 | 939 | 41% | 50% | 4.4 | 2.48 |
| 42992 | off | 1,545 | - | - | - | 2.5 | 2.35 |
| 42992 | on | 3,197 | 1,792 | 56% | 58% | 5.9 | 3.53 |

- Diplomacy makes a game 1.8x (15508) to 2.4x (42992) slower, and the share grows as training goes on.
- The table VP gain with diplomacy on at 42992 (+1.2) is Support for the Throne passing to Hacan
  (see the 42992 eval: 5.93 Supports held per game, 4.05 of them by Hacan). Without diplomacy the
  two checkpoints are equal (2.39 vs 2.35).
- With diplomacy off the old `trade` head carries ~200-240 decisions per game; with it on, those
  move into the diplomacy head.

## 2. The arena episode

1. **Start state:** a snapshot from a real self-play game at the moment a contact can open (active
   player, a legal partner, the negotiation limits not yet spent). Snapshots are drawn across rounds
   1-4 and all six factions so every seat meets every kind of position.
2. **Play:** the active player builds a deal item by item (offers, then asks); the partner accepts,
   declines or counters; at most 2 counters, exactly as in the full game. Both seats are the current
   policy.
3. **Score:** apply the settled deal to a copy of the state and score both seats with section 3.
   A declined, voided or empty deal scores 0 for both.
4. **Train:** every diplomacy decision each seat made in the episode gets that seat's score as its
   return. Nothing else in the game is played.

**Promises are in the arena, priced to start biased against them** (operator): receiving one is
worth 0, giving one costs 0.1. That covers future payment, do-not-activate, do-not-attack, votes,
attack, replenish and leader use. Secret objectives are not offered in v1.

## 3. Scoring

Values are in trade goods (TG). Each item has a value **to the seat that ends up holding it** (V)
and a **cost to the seat that gave it** (C), both judged from the snapshot's position.

For seat *i* trading with partner *j*:

```text
own_i   = sum V(items i receives) - sum C(items i gives)
score_i = own_i - alpha * own_j
```

**alpha = 0.5** (operator). What I gain and give count in full; what my partner gains and gives
counts at half: `score_i = own_i - 0.5 * own_j`.

alpha = 1 was considered first and rejected: it makes every trade exactly zero-sum
(`score_j = -score_i` for any items), so no deal can be good for both and the arena would learn never
to trade. At 0.5 a deal is good for both when each gains more than half of what the other gains --
the six-player reality, where two partners both gain on the other four.

A trade is good for both only when the seats value the items differently. That is what the
position-dependent values below are for: commodities are worth little to their owner and a full TG
to the receiver, a fragment completes one seat's set and not the other's, and so on.

**Conversion:** 1 VP = **5 TG**, the price of the public objective that spends 5 trade goods (operator).

### Components

| Item | Value to the receiver (V) | Cost to the giver (C) | Notes |
|---|---|---|---|
| Trade good | 1 | 1 | Fungible. |
| Commodity | 1 (it becomes a TG on receipt) | 0.25 | The owner cannot spend commodities; the cost is only the chance to trade them elsewhere. |
| Relic fragment, same type as 2 already held (completes a set) | 2.5 | - | A relic is valued at 5 TG (1 VP-equivalent), paid out over a set of 3. |
| Relic fragment, 1 of that type held | 1.5 | - | |
| Relic fragment, none of that type held | 1 | - | Unknown fragments count toward any type. |
| Relic fragment (giver's cost) | - | the value it has to the giver by the same three rows | |
| Action card (Hacan in the deal only) | 1 | 1 | Flat base (operator). The card's actual worth is not modelled in v1. |
| **Support for the Throne** | **4.5** | **0** own, but the receiver's +4.5 enters the giver's score through alpha | Below a full VP (5) because it can be lost back and has a drawback (operator). Giving it away is not free: under alpha = 0.5 it scores -2.25 for the giver before anything paid back. |
| Trade Agreement | owner's commodity value x P(owner replenishes before round 4 ends) | the same commodities at 0.25 each | One-shot: the note returns after it fires. |
| Ceasefire | 1 if the receiver has units adjacent to the owner's, else 0.25 | 0 | Defensive, and position-bound. |
| Political Secret | 1 x P(an agenda phase before the game ends) | 0 | In 4-round games this is usually ~0. |
| Alliance | 2 | 0 | Tradable only once the owner's commander is unlocked. |
| War Funding (Letnev) | 1.5 if the receiver is likely to fight in space, else 0.5 | 0 | |
| Trade Convoys (Hacan) | 1 | 0 | Lets the receiver trade with anyone. |
| Military Support (Sol) | 2 | 0.5 | Two infantry on the receiver's turn, paid for with the owner's strategy token. |
| Cybernetic Enhancements (L1Z1X) | 1.5 | 0.5 | One command token; the owner loses one. |
| Research Agreement (Jol-Nar) | 4 x P(Jol-Nar researches a tech the receiver lacks before the end) | 0 | A free technology. |
| Political Favor (Xxcha) | 1 x P(an agenda phase) | 0 | |
| Another player's note you hold, passed on | its V to the new holder | its V to you | The note's owner is unaffected. |
| Any promise (future payment, do-not-activate, do-not-attack, vote, attack, replenish, leader use) | **0** | **0.1** | Starts the policy biased against promises (operator). |

The P(...) terms come from the snapshot: rounds left, and simple position facts (adjacency, the
owner's commodity cap, whether agendas are live). No learned model is used for scoring.

## 4. Operator decisions (2026-09-22)

1. **alpha = 0.5**: my gains and costs in full, my partner's at half. (alpha = 1 was chosen first, then dropped because it makes every trade zero-sum.)
2. **1 VP = 5 TG.** Support for the Throne 4.5 (losable, has a drawback). Action cards 1 TG base.
3. **Promises are in scope**, valued 0 to receive and 0.1 to give, to start biased against them.
4. **Full games keep diplomacy on.**
5. **The full-game reward does not change.** PPO is allowed to undo what the arena teaches.
