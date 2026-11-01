# D2 lane F2d (`core-d2-c3-dispatch3`): the remaining dispatch routes, resume note (2026-11-01)

Branch `core-d2-c3-dispatch3`, cut from `core-d2-c3-dispatch2` (`1234b33a`). Never pushed; `nightly-fixes-2026-10-26` untouched. Only the LATEST rules version is supported.
Read `docs/KM3_RESUME.md` (F2c), `docs/KM2_RESUME.md` (the dispatch) first. Pure refactor: RULES_VERSION, serde, fixtures and fingerprints are unchanged.

## 1. What is routed now (`Opt` -> core body), additions to KM3 section 4

| kind | option | file | evidence |
|---|---|---|---|
| leader (free use) | Nekro Malleon (`nekroagent`, printed "during the action phase") | `turn/leader.rs` | core `nekro_malleon_chosen_from_the_menu_is_a_free_use_where_the_payer_spends_a_token_for_two_goods`; lockstep `nekro_malleon_chosen_from_the_menu_takes_a_discard_for_two_goods_like_the_string`; the shadow of sol 3p seed 9 |
| leader (agent, ACTION) | Letani Ospha (Arborec) | `leader.rs` | shadow only (needs a map) |
| leader (hero, ACTION) | Jace X., Ahk-Syl Siven, Ul the Progenitor, Darktalon Treilla (not purged by the use: `leaders::exhaust` moves only a readied card, `end_of_round` purges her), The Helmsman, Gurno Aggero, Xxekir Grom TE | `leader.rs` | core Jace X + Darktalon; lockstep Jace X + Darktalon; the others by the shadow when a game reaches them |
| leader | a copy of Ggrocuto Rinn, Carth, Letani Ospha through Ssruu (`Opt::Borrowed`): same body, Ssruu exhausts | `leader.rs` | core `a_copy_of_an_agent_through_ssruu_exhausts_ssruu_and_not_the_copied_agent` |
| technology | Sling Relay (`SlingRelay` window: dock system, ship, payment; the card exhausts when a ship was produced) | `turn/technology.rs` (new) | core `sling_relay_chosen_from_the_menu_produces_a_ship_...` |
| relic | Dynamis Core, Book of Latvinia, The Codex | `turn/relic.rs` | lockstep `the_dynamis_core_and_the_book_of_latvinia_...`, `the_codex_...` |
| faction | The Nucleus, Star Forge, The Inferno, Fires of the Gashlai (Muaat) | `turn/faction.rs` | shadow of xxcha 3p seed 6 (Star Forge, 23 -> 66 steps) |
| faction | Production Biomes, Trade Convoys, Blood Pact, Dark Pact, Promise of Protection, Naaz Fabrication (relic, token) and Black Market Forgery, Yssaril Stall Tactics and Mageon Implants | `faction.rs` | core pacts/notes + Biomes; lockstep notes, Biomes, Stall Tactics |

The shadow oracle lends exactly the options with no body (`native_subject::menu_body_is_lent`): leaders by `leader::routed(leader, borrowed)`, technologies by `technology::routed`, relics by `relic::routed`, faction components by the list `ROUTED_FACTION` (keep in step with the arms of `faction::run`).

Codes added to `TurnCodes` (all interned in `ti4-engine/src/live/turn.rs`): `nekroagent`, `star_forge`, `muaat_mech`, `inferno`, `production_biomes`, `fabrication`, `bmf`, `stall_tactics`, `yssaril_mech`, `relic`.

`component::announce_events(sx)` (new): the events a window staged are announced at once (the string's `hooks_economy::emit` / `gain_trade_goods_staged` with a resolver) while a seated faction has a module, else dropped (B8). Used by Orbital Drop, Nekro Malleon, Star Forge, The Inferno, Production Biomes. The shadow of sol 3p seed 9 found it (`native LEADER_ABILITY_RESOLVED | legacy TRADE_GOODS_GAINED`) and the xxcha game (`STRATEGY_TOKEN_SPENT` after `COMPONENT_ACTION_RESOLVED`).

Carth now announces its replenish with the interned `trade_agreement` (`not_from_supply[3]`), not `explore.filled`'s code: found by a sweep (cabal 5p seed 22: the string says `COMMODITIES_REPLENISHED`, the native step said nothing).

## 2. How far `native_play --shadow` goes now (`gate` profile; steps = native steps, compared = legacy steps compared)

TABLE

## 3. Not routed, with what each needs (the counted gap `Unrouted(kind, key)`)

* leaders: Umbat (Muaat agent) and Letani Miasmiala (Arborec hero): a use of PRODUCTION by an ability needs the facts of `ProduceFacts` (limit, cost ceiling, `home_facts` is the base) and the end-of-use effects (`announce_uses`: breakthroughs, the Prophecy of Ixth, `UNITS_PRODUCED`) that the string adapter runs after the child; Rin (needs the corpus order of the technologies, a table the generator does not emit); Riftwalker Meian (the wormhole/`allowed` reads of the string map); Nekro Flayesh, Kyver, Cabal's It Feeds on Carrion, Naaz Hesh and Prit, Winnu Mathis (a strategy primary: needs M6c's `resume_strategy`), Argent Mirik, Empyrean Conservator, Mahact Airo, Yin Dannel: each takes facts the string adapters compute (`hero_dual.rs`). Z'eu is dropped content.
* relics: the Enigmatic Device and the Maw of Worlds (research/gain with the list of technologies), the Stellar Converter (targets over the map), the Titan Prototype (a build child), The Silver Flame (a die). Enigmatic Device cards (`Opt::Device`).
* faction components: Wormhole Generator (destinations over the map), Cabal's Vortex, Nekro C4R and Titans' Terraform (the menu itself gaps: `MenuGap::Faction`).
* other lanes' stops (not taken): space cannon at the head of the aftermath, Arborec Mitosis in the status phase, movement options on hyperlane maps, the Saar agent's `SYSTEM_ACTIVATED:after` reaction, `UnportedTactical(ActivationHook)`, `SsruuLends`, the order of a strategic action's close against a spent token (`STRATEGIC_ACTION_COMPLETE` before `STRATEGY_TOKEN_SPENT`), Thunder's Edge Warfare's free tactical action (`StrategyAbility`), Military Support's token event announced one step early (sol 4p seed 21).

## 4. Tests added

CHECKS

## 5. Next (in order)

1. The leaders above, in the order of their volume in the sweep (`tools/engine_core/native_play.sh --factions all --players 3,4,5,6 --seeds 3 --first-seed 20 --shadow`): Umbat, then the production heroes.
2. The other lanes' stops (section 3), each with its owner in KM3 section 3.
3. The research relics (Enigmatic Device, Maw of Worlds): the list of technologies and the grant.
