# Backward planning, Agent B's groups

Companion to `docs/BACKWARD_PLANNING.md` (Agent A: framework, Mecatol/Imperial, territory, fleet). Code in `crates/ti4-rule-bot/src/backward/`; knobs are A's (`bw_enabled`, `bw_payment`, `bw_technology`, `bw_combat`), all off by default (`bw_enabled=false`). No existing default or `for_version` table changed.

## Groups

| file | goals | what it plans | steer |
|---|---|---|---|
| `payment.rs` | spend N resources / influence / trade goods / tokens (monument, golden_age, sway_council, manipulate_law, trade_routes, centralize_trade, lead, galvanize, amass_wealth, vast_reserves) | modelled as cheap: payable now has p 0.95 (the existing `Reserve` keeps the price); a gap of at most 3 trade goods or tokens is helped by Trade or Leadership (p 0.45); planet-spend gaps are left to the default logic. `Plan.payment = true` so the framework's discount applies | `cards` |
| `technology.rs` | unit upgrades, colour pairs, one-colour runs, faction technologies (families `unit_upgrades`, `colours_2`, `same_colour_technologies`, `faction_technologies`) | smallest set (at most 3) of technologies that makes the requirement true, ordered by printed prerequisites; researches before the status phase: own Technology primary 0.95, its paid second 0.9 (6 resources), a rival's secondary 0.7 (token + 4 resources) | `cards: Technology`; `research_pick` steers the research choice itself (hooked in `objectives::pick_research`); `reach_by_research(seen, me, ids)` for prep steps |
| `combat.rs` | event secrets dts, sar, dtgs, btv, dyp (no engine progress, so `event_goals` adds them to `PlanCtx.goals`) | one-step plan over `PlanCtx.steps`: the fight that stages the event, p = fight success x event factor | `systems` |
| `structures.rs` | families `structures`, `structures_away`, `units_pds`, `units_spacedock` | structures missing vs room on controlled planets vs placements from Construction (own primary 2 at 0.95, rival secondary 1 at 0.7 per strategy token) | `cards: Construction` |

Not covered, left to the default logic: the bought-objective gaps that are planet spends, secrets whose event needs a promissory note, space cannon or bombardment, faction-specific objectives.

## Measured (fixed table sol/letnev/jolnar/mentak, 4pBgaBalancedD, cyclic rotation, 160 tables = 320 games, mixed mode, candidate only the group's knobs plus `bw_enabled`)

| group | ctrl VP/round | ctrl r3 | ctrl r4 | ctrl r5 | win share |
|---|---|---|---|---|---|
| technology | +0.019 [-0.004, +0.043] | +0.016 | +0.059 [0.014, 0.104] | +0.105 [0.023, 0.187] | +2.8 pp [-0.9, +6.5] |
| combat + structures | +0.003 [-0.011, +0.018] | 0.000 | -0.006 | -0.006 | +0.9 pp [-1.3, +3.1] |
| payment | not run: with no systems in its steer the planner changes nothing until the draft reads `steer.cards` | | | | |

Games in `/root/rulebot-results/backward-b/games/`.
