# Backward planning for the rule bot (experimental, bot version 4 candidate)

Code: `crates/ti4-rule-bot/src/backward/` (module docs there). All knobs `bw_*` in `tuning.rs`;
`bw_enabled` defaults to **false**, so v1/v2/v3 are unchanged and hash-identical.

## Idea

From round `bw_round` (3) the seat looks at the objectives it can still score (revealed publics and held
secrets) and plans *backwards* from each: what must be true, what is the cheapest route this turn, how likely
is it to work, what does it cost. A weighing step compares the plans with each other and with Mecatol Rex /
Imperial and picks the best expected VP. Below a threshold nothing steers and the default logic plays unchanged.

## Design

- `ObjectivePlanner` (trait): `objective_ids()` (aliases or family tokens) and `plan(obs, ctx) -> Option<Plan>`.
  The dispatcher (`verdict`) calls it for every unmet goal that matches, with `ctx.goal` set. One source file per
  group: `territory.rs`, `fleet.rs` (Agent A), `mecatol.rs`; Agent B's groups (payment, technology, combat).
- `PlanCtx`: round, tokens, resources, influence, trade goods, Mecatol state, and `steps`: every activation that
  takes a planet or brings a new ship, with its success odds (fight odds from the fleet-strength matchup, 0.9 for a
  landing, 0.6 when the ship would have to leave a system it alone holds).
- `PlanCtx::route`: the fewest steps (<= 3, <= tactic tokens, best 7 candidates) whose imagined result makes the goal
  met, judged by the engine's own requirement functions (`Imagined` planets / systems). Probability is the product
  of the step odds, 0.9 per extra step.
- `weigh`: expected VP = probability x marginal points - `bw_token_cost` per token - `bw_resource_cost` per
  resource. A second public objective only counts a quarter unless Imperial is in play (as `objectives::gain`).
  With two likely public plans (>= `bw_likely`, a met public counts as one) the Imperial card is wanted and each
  likely plan gains `bw_imperial_bonus`. Spend-N plans lose `bw_payment_discount` of their value. The Mecatol plan
  (custodians point if the token stands + 0.6 Imperial point; or hold + play Imperial) competes as one more candidate.
  Best expected VP below `bw_threshold`: fallback, no steer.
- Integration (`objectives.rs`): the chosen plans add `bw_steer x expected VP` to the activation value of the systems
  on their routes (so both the tactical-vs-pass decision and the system choice see it); the Imperial draft gets +20
  when the verdict wants Imperial. Movement, landing and production are the existing logic.

## Knobs

`bw_enabled`, `bw_round`, `bw_threshold`, `bw_steer`, `bw_likely`, `bw_imperial_bonus`, `bw_payment_discount`,
`bw_token_cost`, `bw_resource_cost`, `bw_mecatol`, `bw_mecatol_weight`, and group switches `bw_territory`,
`bw_fleet`, `bw_payment`, `bw_technology`, `bw_combat`.

## Per-objective logic

- Territory (`non_home`, `on_the_rim`, `same_trait`, `tech_specialties`, `planets_of_trait_*`, `attached_planets`,
  `legendary_planets`, `rival_home_planets`, `shared_planet_systems`, `notable_systems`): route of planet-taking steps.
- Fleet (`ships_in_systems`, `planetless_systems`, `ship_systems_beside_*`, `ships_adjacent_to_mecatol`,
  `rival_dock_systems_with_ships`, `mecatol_ships_while_controlling`, `distinct_rival_home_reaches`,
  `weaker_neighbours`): route of ship-arriving steps. Fleet-size goals (`fleet_in_one_system`, capital ships) stay
  with `planner::Bias`.

## A/B

Fixed table (shared with Agent B): 4 players, map 4pBgaBalancedD, factions sol/letnev/jolnar/mentak, cyclic rotation,
mixed mode, seeds 20000.., baseline = knobs off. Games in `/root/rulebot-results/backward-a/`.

(results below)
