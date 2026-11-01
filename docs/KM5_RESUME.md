# D2 lane F2e (`core-d2-c3-dispatch4`): the rest of the menu options routed, resume note (2026-11-01)

Branch `core-d2-c3-dispatch4`, cut from `core-d2-c3-dispatch3` (`8b212698a`). Never pushed; `nightly-fixes-2026-10-26` untouched. Only the LATEST rules version is supported.
Read `docs/KM4_RESUME.md` (F2d), `docs/KM3_RESUME.md` (F2c), `docs/KM2_RESUME.md` (the dispatch) first. Pure refactor: RULES_VERSION, serde, fixtures and fingerprints are unchanged.
Based on `core-d2-c3-dispatch3` only: M6c's `resume_strategy` was NOT needed (see Mathis Mathinus below), so `core-m6c-tactical` is not merged.

## 1. Routed now (`Opt` -> core body), additions to KM4 section 1

| kind | option | file | evidence |
|---|---|---|---|
| leader (agent, ACTION) | Umbat (Muaat), also through Ssruu | `turn/leader.rs`, `turn/produce.rs` (new) | core `umbat_*`; lockstep `umbat_chosen_..`, `umbat_for_another_player_..` |
| leader (hero, ACTION) | Letani Miasmiala (Arborec) | same | core `letani_miasmiala_..`, Auto-Factories and Prophecy tests; lockstep |
| leader (hero) | Rin (Jol-Nar): corpus order from `codes.agenda.tech_corpus`, alias order from `plays.tech_order` | `leader.rs` | core `rin_*`; lockstep `rin_chosen_..` |
| leader (hero) | Riftwalker Meian (Ghost): `SwapFacts` from the core galaxy (wormholes, the Creuss/Nexus/Fracture exclusions) | `leader.rs` | core only (the lockstep harness loads no map); shadow |
| leader (hero) | UNIT.DSGN.FLAYESH, Kyver, It Feeds on Carrion, Hesh and Prit, Mirik Aun Sissiri, Conservator Procyon | `turn/hero.rs` (new) | core tests each; lockstep for Flayesh, Kyver, Hesh and Prit (Carrion, Mirik, Procyon need a map: core + shadow) |
| leader (hero) | Mathis Mathinus (Winnu): the primary performed, then `LeaderStrategy` + `select_leader_strategy_followers` (strategic lane E) | `hero.rs` | core `mathis_..`; lockstep with the Trade primary |
| relic | Enigmatic Device, Maw of Worlds, Stellar Converter, Titan Prototype, The Silver Flame (a die on the dice stream, purpose `silver_flame`) | `turn/relic.rs` | core tests each; lockstep for all but the Converter (map) |
| exploration card | Enigmatic Device cards (`Opt::Device`): own question variant `RAsk::DeviceTechnology { card }` ("research which technology with the Enigmatic Device") | `relic.rs::use_device`, `choice/relic.rs`, `render/relic.rs` | core + lockstep |
| faction | Wormhole Generator (Ghost) | `turn/faction.rs` | core (map) |
| faction (menu gaps closed) | Nekro ???_ERROR_ERROR_??? (place PDS, repair, draw), Titans Terraform (one option per controlled non-home planet), Cabal Vortex: now LISTED by `menu_faction::faction` and routed. `MenuGap::Faction` is no longer raised | `menu_faction.rs`, `faction.rs` | core + lockstep (C4R x3, Terraform); Vortex core only (map) |

`production by an ability` (Umbat, Miasmiala): facts from `rules::production::probe::ability_facts` (limit 2 and cost ceiling 4 for Umbat, 1000 for Miasmiala); the string adapter's tail after the child (`leaders_core::announce_uses`) is `turn/produce.rs::finish_uses`: planet-value swap lapses, Hacan's Auto-Factories pays a fleet token for three non-fighter ships, the Prophecy of Ixth is discarded unless two fighters were made, `UNITS_PRODUCED` (source `ability`) announced at once.

Other changes: `TurnCodes` gets `silver_flame` and `nekrohero` (interned in `ti4-engine/src/live/turn.rs`); `driver/agenda/ground.rs` announces the ground forces of UNIT.DSGN.FLAYESH (cause `nekrohero`, before the goods: found by the lockstep); `ROUTED_FACTION` in `native_subject.rs` and the `play_area:` lend are updated; small visibility changes (`end_value_swap`, `announce_made`, `place_tokens`, `Umbat::candidates`, `PlaceUnits::has_spot`, `Vortex::offered`).

## 2. NOT routed (counted gap `Unrouted(5, key)`), with owners

* Airo Shir Aur (Mahact hero) and Dannel of the Tenth (Yin hero): both run a combat / invasion as a child; their windows take `Prepared`/`YinFacts` (the machine environment and fields of a fresh fight). Owner: M5/M6 (combat and invasion preparation), then this table.
* Mathis Mathinus with Thunder's Edge Warfare: the free tactical action (`TurnGap::FreeTactical`, M6c `open_free`/`resume_strategy`).
* Z'eu: lane H4 (dropped content here).

## 3. `native_play --shadow` after the lane

The table of KM4 section 2 is unchanged game for game (same steps, same stops; every stop is another lane's): the games of this lane's routes were not in it. The wider sweep (`--factions all --players 3,4,5,6 --seeds 3 --first-seed 20 --shadow`, 288 games, 0 clean finishes): first gaps `UnportedTactical(ActivationHook)` 207, `TurnGap(StrategyAbility)` 64, `PeaceAccords` 2, and **`Unrouted` 0** (KM4: 5 Umbat + Terraform). First differences (16 games): strategic-action close order (`STRATEGIC_ACTION_COMPLETE` before `STRATEGY_TOKEN_SPENT`, owner E) 8, space cannon head / Saar and Nomad agent reactions (G/H1) 4, Cabal/other reaction questions, the rest single games. Owners: ActivationHook = M6; StrategyAbility = M6c/M6d (Thunder's Edge Warfare, Doctor Sucaban); PeaceAccords = the Xxcha annexation window (lane E/H).

## 4. Mutation debt of F2d and this lane's own (single files, `tools/engine_core/mutants.sh`, union of the core test `action_dispatch` and the engine lockstep `action_dispatch_live_tests`, `D4_COVERS_FILE=/dev/null`)

See section 6 for the final numbers; the F2d re-run at the tip `8b212698a` found: `leader.rs` 29/29 killed by the core tests; `relic.rs` and `technology.rs` all killed in the union; `faction.rs` one survivor (the `bmf`/`fabrication` source choice), killed by `black_market_forgery_and_fabrication_ask_under_their_own_source`.

## 5. Checks run

`check_layers.py core` and `lint_core.sh` clean. Core tests: 20 named binaries pass (`turn_jobs turn_jobs2 turn_end_job aftermath_chain leader_windows leader_windows_agents leader_heroes leaders leadership strategic_arms action_dispatch faction_components hero_windows leader_production promissory_windows relic_windows invasion_composer invasion_yin explore_windows exploring`); `action_dispatch` has 66 tests (KM4: 30); engine lockstep `action_dispatch_live_tests` has 38 (KM4: 21). Bridge lib 105. Not run (standing rule): golden/wide dual, benches, soak, census, workspace clippy, the ratchet.
