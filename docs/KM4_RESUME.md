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

| game | F2c steps (compared) -> now | first stop now |
|---|---|---|
| sol, 4p hyperlanes, seed 1 | 54 (36) -> 54 (36) | `GAP ask.action_menu`: `TurnGap::StrategyAbility` (Thunder's Edge Warfare / Doctor Sucaban: lane M6c/M6d) |
| hacan, 5p, seed 3 | 98 (73) -> 98 (73) | difference at `[phase]`: the status phase does not ask Arborec's Mitosis (owner C / Arborec module) |
| naalu, 5p, seed 12 | 64 (42) -> 64 (42) | difference at the menu step: `STRATEGIC_ACTION_COMPLETE` before `STRATEGY_TOKEN_SPENT` (owner E) |
| sol, 3p, seed 2 | 31 (20) -> 31 (20) | aftermath: the space cannon of the activated system (head of the aftermath: M6/M5) |
| arborec, 6p standard, seed 4 | 20 (16) -> 20 (16) | the same space cannon difference |
| letnev, 4p hyperlanes, seed 5 | 14 (12) -> 14 (12) | `[tactical]` movement options on a hyperlane map (A2/M6 movement facts) |
| xxcha, 3p, seed 6 | 23 (15), `Unrouted(11)` (Star Forge) -> **66 (40)** | aftermath: the same space cannon difference |
| sol, 3p, seed 9 | 11 (8), `Unrouted(5)` (Nekro Malleon) -> **14 (11)** | `[tactical]`: the Saar agent's `SYSTEM_ACTIVATED:after` reaction (G / H1) |
| jolnar, 5p, seed 7 | 10 (8) -> 10 (8) | the same Saar agent reaction |
| mentak, 4p outer rim, seed 10 | 14 (12) -> 14 (12) | the same Saar agent reaction |
| winnu, 4p outer rim, seed 8 | 24 (19) -> 24 (19) | `GAP tactical`: `UnportedTactical(ActivationHook)` (M6) |
| yssaril, 6p outer rim, seed 11 | 21 (17) -> 21 (17) | `GAP tactical`: `UnportedTactical(SsruuLends)` (M6 / G) |

Both games that stopped at a route of this lane now play on: each ends at another lane's stop. No game finishes natively yet. The reproduction is `native_play --factions F --players N --first-seed S --shadow` (the flags of the table; the F2c numbers reproduce exactly with the F2c binary).

A wider sweep (`--factions all --players 3,4,5,6 --seeds 3 --first-seed 20 --shadow`, 288 games): the first gaps were `UnportedTactical(ActivationHook)` 204, `StrategyAbility` 62, `Unrouted(5, Umbat)` 4 (before this lane: `Unrouted(6, Sling Relay)` in 44 games, which is gone), `PeaceAccords` 2, Titans' Terraform 1. First differences: strategic-action close order 5, the space cannon 5, Military Support's token event 1, the Cabal agent's reaction after Carth (section 3) and Mahact's Predictive Intelligence question 1 each.

## 3. Not routed, with what each needs (the counted gap `Unrouted(kind, key)`)

* leaders: Umbat (Muaat agent) and Letani Miasmiala (Arborec hero): a use of PRODUCTION by an ability needs the facts of `ProduceFacts` (limit, cost ceiling, `home_facts` is the base) and the end-of-use effects (`announce_uses`: breakthroughs, the Prophecy of Ixth, `UNITS_PRODUCED`) that the string adapter runs after the child; Rin (needs the corpus order of the technologies, a table the generator does not emit); Riftwalker Meian (the wormhole/`allowed` reads of the string map); Nekro Flayesh, Kyver, Cabal's It Feeds on Carrion, Naaz Hesh and Prit, Winnu Mathis (a strategy primary: needs M6c's `resume_strategy`), Argent Mirik, Empyrean Conservator, Mahact Airo, Yin Dannel: each takes facts the string adapters compute (`hero_dual.rs`). Z'eu is dropped content.
* relics: the Enigmatic Device and the Maw of Worlds (research/gain with the list of technologies), the Stellar Converter (targets over the map), the Titan Prototype (a build child), The Silver Flame (a die). Enigmatic Device cards (`Opt::Device`).
* faction components: Wormhole Generator (destinations over the map), Cabal's Vortex, Nekro C4R and Titans' Terraform (the menu itself gaps: `MenuGap::Faction`).
* the Cabal agent's reaction to `COMMODITIES_REPLENISHED` after a Carth replenish (the string offers `leader:cabal:cabalagent:COMMODITIES_REPLENISHED:after`; the native step announces the event but opens no reaction): owner G / H1, like the Saar agent's.
* other lanes' stops (not taken): space cannon at the head of the aftermath, Arborec Mitosis in the status phase, movement options on hyperlane maps, the Saar agent's `SYSTEM_ACTIVATED:after` reaction, `UnportedTactical(ActivationHook)`, `SsruuLends`, the order of a strategic action's close against a spent token (`STRATEGIC_ACTION_COMPLETE` before `STRATEGY_TOKEN_SPENT`), Thunder's Edge Warfare's free tactical action (`StrategyAbility`), Military Support's token event announced one step early (sol 4p seed 21).

## 4. Tests added

Core: `tests/action_dispatch.rs` 30 tests (was 9): Nekro Malleon, Jace X, Darktalon (incl. `end_of_round`), the Helmsman, Gurno Aggero, Xxekir Grom, Letani Ospha, Ahk-Syl + Ul, Ssruu copies (Ggrocuto, Carth, Ospha), the four action notes, Production Biomes, Star Forge, The Nucleus, The Inferno, Fires, Fabrication (token, relic), Black Market Forgery, the three relics, Sling Relay, a staged ship destruction announced after a component action (KM2 section 8, via a test window that calls `announce_staged`), the gap test now on Kyver. Engine lockstep (`action_dispatch_live_tests`, 21): Jace X, Darktalon, Nekro (discard path with staged cards), Dynamis Core, Book of Latvinia, Codex, the notes, Production Biomes, Stall Tactics, Mageon, **the cancelled card play (Sabotage; KM2 section 8)**, Carth at a Cabal table (native order only). Bridge lib 105, core lib 477, 11 core test binaries (`turn_jobs turn_jobs2 turn_end_job aftermath_chain leader_windows leader_windows_agents leader_heroes leaders leadership strategic_arms action_dispatch`), `check_layers.py core` 514 files 0 violations, `lint_core.sh` 516 files 0 hits. Not run (standing rule): golden/wide duals, benches, soak, census, workspace clippy, the ratchet, all 141 binaries, the full engine lib.

Mutation check (single files, `tools/engine_core/mutants.sh`, union of the core tests `action_dispatch` and the lockstep `action_dispatch_live_tests`, `D4_COVERS_FILE=/dev/null`): the F2c halves `chain.rs` (`mirror_then_flush`, `flush_open`): 2/2 killed; `fx::mirror_recs`: 1/1; `Render::sync_codes`: the first pass left 2 of 4 (the first text of the table was not compared, an equal-length table was not distinguished); a unit test now kills all 4 (`sync_codes_tests`). The new files: `leader.rs` 29/31 killed by the core tests, `faction.rs` 18/32 core + 12/32 lockstep (the union leaves the surviving points below, all marked), `relic.rs` and `technology.rs` all killed in the union. Survivors were closed by tests (Helmsman, Gurno, Xxekir, Ospha, Ahk-Syl, Ul, Ssruu copies, Muaat, Fabrication, relics, `conclude` flags folded into `conclude_announced`/`conclude_quiet`, the gap key passed instead of recomputed) or marked `// mutants:skip` with the reason: the `Failed` verdict of a body (`component::verdict`: the menu offers an option only when its body can resolve), the free-use failure arm, a halted step (the driver rolls it back), the key defaults, `XxchaAgent::new(.., true)` (every game has a map), the interned `trade_agreement` index (pinned by the shadow), Mageon's `limited` literal (the string bails when a faction changes the hand limit), `supply::allowed(.., 1)` of Xxekir.

Other changes: the core menu now offers Xxekir Grom TE (`KernelEnv::other_action_resolves` returned `false` for every leader but Z'eu and Rin, so the Thunder's Edge card set's Xxcha hero was never listed natively; the approximation is a controlled planet and a PDS in the box, the string also counts the mech). `muaat_component::StarForge::war_sun_systems` is `pub`.

## 5. Next (in order)

1. The leaders above, in the order of their volume in the sweep (`tools/engine_core/native_play.sh --factions all --players 3,4,5,6 --seeds 3 --first-seed 20 --shadow`): Umbat, then the production heroes.
2. The other lanes' stops (section 3), each with its owner in KM3 section 3.
3. The research relics (Enigmatic Device, Maw of Worlds): the list of technologies and the grant.
