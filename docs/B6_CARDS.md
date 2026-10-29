# B6 closing package: the fixed card list (2026-10-29)

Derived from `ENGINE_KNOWN_ISSUES.md` B6-bail-01..14, `ENGINE_REACTIONS.md` sections 7 and 9 (reaction-slot plays done in stage 3:
Sabotage, Intercept, Parley, Direct Hit, Skilled Retreat as `CardPlay`s) and the code (`Fx::of`, `Simple::of`, the registered
`CardPlay`s). Of the 142 action cards, 97 have a core effect (44 `Simple` stamps, 44 `Fx` windows incl. Intercept, Rescue, Fighter
Conscription, Manipulate Investments, 9 registered plays incl. Direct Hit x4, Skilled Retreat x4, Parley, Sabotage x4 counted in Fx).
The 45 cards below (by printed name, 55 copies counted as names) had no core effect at the start of this package.

Status column: PORT (core effect, lockstep test, mutation checked), BAIL (documented, with a test). Updated as the package
proceeds; the final column is the state at the closing commit.

| card | window | what it needs | plan |
|---|---|---|---|
| Repeal Law | action | pick a law in play, B9 `laws` leave | Fx pick |
| Spy | action | pick a seat, a random card (dice stream) | Fx + dice |
| Plague | action | pick a rival planet, one die per infantry, staged ground destruction | Fx + dice + staged mark |
| Unstable Planet | action | hazardous planet, 3 infantry destroyed (staged mark) | Fx + staged mark |
| Seize Artifact | action | neighbours with relic fragments (map fact) | Fx + neighbour fact |
| Lie in Wait | after neighbours transact | neighbours who traded (map + transactions fact) | Fx + fact |
| Exchange Program | action | another seat answers yes/no, fleet tokens | Fx (second asked seat) |
| Reparations | after other gains your planet | two questions, control-gained record | Fx |
| Public Disgrace | when other chooses a strategy card | the strategy card pick again | Fx |
| Summit | start of strategy phase | gain 2 command tokens (pool pick) | Fx |
| Ghost Ship | action | wormhole systems free of ships (map fact) | Fx + fact |
| Lucky Shot / Scuttle | action | per-unit option details | see plan |
| Archaeological Expedition / Exploration Probe | action | exploration decks (rng, frontier) | see plan |
| Divert Funding / Brilliance | action | technology / planet | see plan |
| Bribery / Focused Research / Impersonation / Plagiarize / Reveal Prototype / Mercenary Contract / Pirate Fleet | various | payment windows, research | registered play + child window |
| Strategize x4 / Overrule | action | perform another card's ability | registered play + child window |
| Pirate Contract x4 / Refit Troops / Upgrade / Decoy Operation / Ghost Squad / Experimental Battlestation | various | faction hooks, neutral units, unit-form flip | see plan |
| Fire Team / Scramble Frequency / Courageous To The End / Crash Landing | in a combat window | rerolls, dice, casualty questions | see plan |
| Harness Energy / Infiltrate / Signal Jamming | various | tile catalogue, swap rules, blockade map | see plan |

The final state per card is in the table "Closing state" below (written when the package closes).
