# BF-ALLIANCE-ROUTES-A: Arborec, Argent, and Ghost

Status: route integration implemented; parent-coordinated Cargo validation pending.

## Eligibility predicate changes

The only commander-rights predicate in each of these modules is now `promissory::has_commander_ability(state, recipient, commander_id)`:

| Module | Listener / consumer | Exact replacement |
|---|---|---|
| Arborec | `commander_ready` for `SYSTEM_ACTIVATED:after` | `has_commander_ability(context.state, owner, "arboreccommander")` gates another player's activation and a producing system. |
| Argent | `holds(..., Source::Commander)` for the unit-ability extra die | `has_commander_ability(state, seat, "argentcommander")` replaces both Argent faction identity and local unlocked-status checks. |
| Ghost | Sai Seravus `SHIP_MOVED` eligibility, fighter availability, and `MOVEMENT_FINISHED` condition | Each checks `has_commander_ability(state, owner, "ghostcommander")`; movement/capacity/fighter legality remains separate. |

No commander unlock-condition callback was changed. Native module-only routes remain faction-scoped: Arborec's non-commander listeners are returned only for Arborec seats; Argent's agent and Argent breakthrough token-transfer listeners remain Argent-only. Argent's technology, Ambuscade, and commander listeners remain armed for every seat because their conditions already limit them to the actual holder. Ghost's listeners were already assembled for every seat; its IFF and adjacency behavior needs to continue working for promissory-note holders, while commander eligibility is now ability-based.

## Route coverage added

Resolver-driven tests cover direct public grants to non-native recipients for Arborec's production trigger, Argent's bonus die, and Ghost's movement fighter trigger. The corresponding commander card remains absent from those recipient leader maps. Existing native unlocked-route tests are preserved. Ghost additionally tests ordinary Alliance staying inert while its seated owner has not unlocked the exact commander. Argent's existing locked-own case remains and a new ordinary Alliance locked-owner test verifies no extra die is added.

Parent review also found Argent agent destinations used geometric `Galaxy::adjacent`, omitting recipient-specific wormhole adjacency. `agent_planets` now uses `movement::PlayerAdjacency::new(state, content, sources, galaxy, recipient)`. A regression fixture verifies a non-geometric alpha/beta linked destination is legal to a Ghost recipient while destinations outside the resulting adjacency remain excluded.

Formatting and diff checks are recorded after targeted rustfmt. Cargo was not run by this agent per the shared-build instruction.

## Root validation corrections

Stymie must remain armed for every seat because its foreign holder uses the note; the native-only listener split accidentally restricted it. Root restored it to the always-armed list. Direct-grant fixtures retain the starting Locked commander record and assert that it remains Locked; borrowed rights do not fabricate an unlock. Arborec explicitly answers the optional ability offer before buying its unit. Argent map fixtures skip planet-less wormhole tiles and choose a planet-bearing remote destination. The latest engine library run includes these routes and passes (1,985 passed, 1 ignored); full integration rerun follows the test-helper boundary correction.
