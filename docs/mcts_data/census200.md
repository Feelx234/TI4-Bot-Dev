games 200 (finished 200), errors 0, workers 4, wall 125.2 s
rounds at the end: {4: 1, 5: 22, 6: 85, 7: 69, 8: 19, 9: 4}
winners: {"jolnar": 36, "letnev": 53, "sol": 89, "xxcha": 22}; mean VP: [("jolnar", "6.92"), ("letnev", "7.35"), ("sol", "8.12"), ("xxcha", "6.15")]

## M0

| quantity | value |
|---|---|
| steps / game | 1752 |
| decisions (asks) / game | 1955 |
| loop time / game (ms) | 1978 |
| of it engine (ms) | 388 (20 %) |
| of it env build (ms) | 6.0 |
| of it rule bot incl. view (ms) | 1574 (80 %) |
| of it SeenView build (ms) | 2.6 |
| of it Activate filter (ms) | 16.0 |
| steps / s (whole loop) | 886 |
| engine steps / s (bot time removed) | 4514 |
| asks / s (whole loop) | 988 |
| step wall p50 / p99 (us) | 167 / 16083 |
| `Pos::fork` (core clone + render rebuild) (us) | 3.84 |
| `CoreState::clone` alone (us) | 2.41 |
| fork + replay one step with the rule bot (us) | 1806 (43916 samples, 1.12 asks each) |
| tree nodes / game (all seats) | 577 |
| tree nodes / seat / game | 144 |
| loop time per tree node (ms): the cost of one segment | 3.427 |
| of it engine per tree node (ms) | 0.672 |
| segment wall between tree nodes: mean / p50 / p90 / p99 (us) | 4141 / 1071 / 8614 / 43678 |
| of it delegated rule-bot answers: mean / p50 / p90 (us) | 645 / 0 / 337 |

## Asks per kind

| ask kind | tree | per game | share smart=false | opts mean (min-max) | p90 opts | after filter mean (min-max) | p90 filt | 1-opt share | bot+view us mean | p50 | p99 | misaligned | misnamed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Production.Pay |  | 184.1 | 0 % | 4.8 (2-20) | 8 | 4.8 (2-20) | 8 | 0 % | 58 | 33 | 101 | 0 | 0 |
| Turn.ActionMenu | tree | 169.5 | 31 % | 5.3 (1-13) | 8 | 5.3 (1-13) | 8 | 2 % | 3924 | 479 | 44665 | 0 | 0 |
| Tactical.Move | tree | 155.6 | 1 % | 2.7 (1-13) | 4 | 2.7 (1-13) | 4 | 20 % | 149 | 73 | 594 | 0 | 0 |
| Turn.Closing | tree | 143.9 | 99 % | 3.3 (1-11) | 4 | 3.3 (1-11) | 4 | 3 % | 120 | 34 | 167 | 0 | 0 |
| Tactical.Load |  | 127.1 | 0 % | 3.4 (2-10) | 5 | 3.4 (2-10) | 5 | 0 % | 70 | 46 | 117 | 0 | 0 |
| Strategy.GainPool |  | 124.9 | 0 % | 3.0 (3-3) | 3 | 3.0 (3-3) | 3 | 0 % | 133 | 77 | 185 | 0 | 0 |
| Reaction.Window |  | 100.4 | 44 % | 2.0 (2-3) | 2 | 2.0 (2-3) | 2 | 0 % | 235 | 53 | 608 | 0 | 0 |
| Strategy.Follow |  | 87.0 | 43 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 83 | 49 | 173 | 0 | 0 |
| Agenda.Planets |  | 85.4 | 0 % | 6.0 (2-20) | 10 | 6.0 (2-20) | 10 | 0 % | 402 | 242 | 3262 | 0 | 0 |
| Production.Choose |  | 78.0 | 0 % | 7.1 (2-9) | 9 | 7.1 (2-9) | 9 | 0 % | 2226 | 1147 | 30276 | 0 | 0 |
| Tactical.Activate | tree | 75.2 | 0 % | 36.3 (29-38) | 38 | 17.4 (1-38) | 31 | 1 % | 4326 | 2488 | 42676 | 0 | 0 |
| Invasion.CommitGround |  | 72.4 | 0 % | 3.0 (1-10) | 5 | 3.0 (1-10) | 5 | 1 % | 68 | 39 | 110 | 0 | 0 |
| Strategy.LeadershipBuy |  | 48.2 | 0 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 61 | 35 | 106 | 0 | 0 |
| Strategy.Draft | tree | 44.9 | 15 % | 5.0 (2-8) | 8 | 5.0 (2-8) | 8 | 0 % | 3784 | 2439 | 36672 | 0 | 0 |
| Agenda.Vote |  | 31.9 | 0 % | 5.3 (2-39) | 5 | 5.3 (2-39) | 5 | 0 % | 386 | 234 | 3282 | 0 | 0 |
| Trade.Propose |  | 31.5 | 0 % | 17.7 (3-49) | 29 | 17.7 (3-49) | 29 | 0 % | 80 | 37 | 109 | 0 | 0 |
| Strategy.StatusRedistribute |  | 22.0 | 0 % | 26.5 (3-136) | 45 | 26.5 (3-136) | 45 | 0 % | 103 | 66 | 145 | 0 | 0 |
| Combat.AnnounceRetreat |  | 21.8 | 0 % | 2.0 (1-2) | 2 | 2.0 (1-2) | 2 | 0 % | 64 | 52 | 127 | 0 | 0 |
| Strategy.ReadyPlanet |  | 21.7 | 0 % | 5.6 (1-22) | 10 | 5.6 (1-22) | 10 | 7 % | 58 | 41 | 105 | 0 | 0 |
| Trade.Answer |  | 20.4 | 0 % | 3.0 (3-3) | 3 | 3.0 (3-3) | 3 | 0 % | 74 | 39 | 128 | 0 | 0 |
| Legendary.Offer |  | 19.6 | 100 % | 2.0 (2-4) | 2 | 2.0 (2-4) | 2 | 0 % | 66 | 29 | 100 | 0 | 0 |
| Objectives.Score | tree | 17.5 | 0 % | 2.4 (2-7) | 3 | 2.4 (2-7) | 3 | 0 % | 29 | 21 | 71 | 0 | 0 |
| Strategy.PlaceStructure |  | 16.8 | 75 % | 14.5 (2-40) | 23 | 14.5 (2-40) | 23 | 0 % | 190 | 89 | 489 | 0 | 0 |
| Invasion.FightRound |  | 15.2 | 100 % | 1.0 (1-1) | 1 | 1.0 (1-1) | 1 | 100 % | 43 | 35 | 99 | 0 | 0 |
| Strategy.ResearchTechnology |  | 14.5 | 0 % | 18.1 (8-30) | 25 | 18.1 (8-30) | 25 | 0 % | 2626 | 1774 | 32019 | 0 | 0 |
| Agenda.UseOnAgenda |  | 13.0 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 39 | 33 | 101 | 0 | 0 |
| Tactical.Remove |  | 13.0 | 83 % | 1.4 (1-5) | 2 | 1.4 (1-5) | 2 | 73 % | 42 | 34 | 103 | 0 | 0 |
| Combat.CancelHit |  | 12.5 | 0 % | 2.1 (2-4) | 2 | 2.1 (2-4) | 2 | 0 % | 55 | 33 | 89 | 0 | 0 |
| Strategy.PoliticsAgenda |  | 12.3 | 0 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 186 | 133 | 266 | 0 | 0 |
| Strategy.SucabanOffer |  | 11.2 | 0 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 43 | 29 | 105 | 0 | 0 |
| Cards.Pick |  | 10.0 | 94 % | 7.3 (2-35) | 15 | 7.3 (2-35) | 15 | 0 % | 42 | 29 | 90 | 0 | 0 |
| Objectives.ReturnSecret |  | 8.7 | 0 % | 3.0 (2-4) | 4 | 3.0 (2-4) | 4 | 0 % | 55 | 27 | 90 | 0 | 0 |
| Leaders.RoundAgentUnit |  | 8.0 | 0 % | 4.0 (2-16) | 7 | 4.0 (2-16) | 7 | 0 % | 62 | 35 | 101 | 0 | 0 |
| Cards.HandLimit |  | 7.1 | 100 % | 7.2 (2-10) | 9 | 7.2 (2-10) | 9 | 0 % | 33 | 27 | 90 | 0 | 0 |
| Strategy.DiplomacySystem |  | 6.2 | 100 % | 5.3 (1-15) | 8 | 5.3 (1-15) | 8 | 0 % | 40 | 29 | 101 | 0 | 0 |
| Strategy.TeWarfareRedistribute |  | 6.2 | 0 % | 23.1 (1-120) | 45 | 23.1 (1-120) | 45 | 0 % | 151 | 91 | 263 | 0 | 0 |
| Strategy.TeWarfareRedistributeAfter |  | 6.2 | 0 % | 23.1 (1-120) | 45 | 23.1 (1-120) | 45 | 0 % | 121 | 95 | 230 | 0 | 0 |
| Strategy.TeWarfareTactical | tree | 6.2 | 11 % | 37.1 (37-38) | 38 | 15.2 (1-37) | 32 | 5 % | 766 | 430 | 6523 | 0 | 0 |
| Strategy.ConstructionAbility |  | 6.2 | 0 % | 2.3 (1-28) | 3 | 2.3 (1-28) | 3 | 1 % | 148 | 47 | 197 | 0 | 0 |
| Strategy.PoliticsSpeaker |  | 6.2 | 0 % | 3.0 (3-3) | 3 | 3.0 (3-3) | 3 | 0 % | 36 | 26 | 85 | 0 | 0 |
| Strategy.TradeReplenish |  | 6.2 | 0 % | 3.8 (2-4) | 4 | 3.8 (2-4) | 4 | 0 % | 59 | 23 | 86 | 0 | 0 |
| Explore.Reward |  | 6.0 | 72 % | 1.7 (1-3) | 2 | 1.7 (1-3) | 2 | 37 % | 27 | 19 | 69 | 0 | 0 |
| Legendary.ArmsVault |  | 5.4 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 45 | 26 | 97 | 0 | 0 |
| Legendary.PlaceOnPlanet |  | 5.1 | 100 % | 9.9 (3-20) | 14 | 9.9 (3-20) | 14 | 0 % | 42 | 28 | 100 | 0 | 0 |
| Combat.AssignHit |  | 5.0 | 0 % | 1.9 (1-4) | 3 | 1.9 (1-4) | 3 | 26 % | 95 | 30 | 82 | 0 | 0 |
| Combat.RerollDie |  | 4.9 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 89 | 38 | 98 | 0 | 0 |
| Leaders.JolnarSwap |  | 4.8 | 100 % | 6.2 (4-8) | 8 | 6.2 (4-8) | 8 | 0 % | 33 | 26 | 76 | 0 | 0 |
| Production.Place |  | 3.9 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 37 | 31 | 88 | 0 | 0 |
| Strategy.ImperialScore | tree | 3.9 | 0 % | 2.2 (2-5) | 3 | 2.2 (2-5) | 3 | 0 % | 75 | 26 | 86 | 0 | 0 |
| Technology.Restack |  | 3.8 | 0 % | 5.4 (3-7) | 7 | 5.4 (3-7) | 7 | 0 % | 46 | 30 | 80 | 0 | 0 |
| Leaders.XxchaPlanet |  | 3.3 | 9 % | 20.9 (2-37) | 31 | 20.9 (2-37) | 31 | 0 % | 131 | 26 | 119 | 0 | 0 |
| Combat.RetreatTo |  | 3.2 | 0 % | 2.5 (2-6) | 4 | 2.5 (2-6) | 4 | 0 % | 48 | 34 | 85 | 0 | 0 |
| Strategy.PeaceAccords |  | 3.0 | 100 % | 4.1 (2-11) | 7 | 4.1 (2-11) | 7 | 0 % | 89 | 22 | 85 | 0 | 0 |
| Explore.PayInfantry |  | 2.9 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 29 | 24 | 74 | 0 | 0 |
| Technology.TransitDiodes |  | 2.8 | 100 % | 43.0 (2-170) | 89 | 43.0 (2-170) | 89 | 0 % | 98 | 38 | 97 | 0 | 0 |
| Combat.MunitionsRoll |  | 2.7 | 0 % | 2.4 (2-3) | 3 | 2.4 (2-3) | 3 | 0 % | 135 | 29 | 106 | 0 | 0 |
| Combat.WarFunding |  | 2.4 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 102 | 40 | 111 | 0 | 0 |
| Combat.DeployMech |  | 2.1 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 47 | 34 | 97 | 0 | 0 |
| Leaders.XxchaInfantry |  | 1.7 | 0 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 23 | 16 | 84 | 0 | 0 |
| Agenda.Tiebreak |  | 1.6 | 0 % | 5.6 (2-38) | 23 | 5.6 (2-38) | 23 | 0 % | 253 | 133 | 249 | 0 | 0 |
| Leaders.SustainCommander |  | 1.4 | 0 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 45 | 29 | 101 | 0 | 0 |
| Technology.SelfAssembly |  | 1.4 | 0 % | 2.9 (2-4) | 3 | 2.9 (2-4) | 3 | 0 % | 136 | 29 | 85 | 0 | 0 |
| Promissory.PlaceUnits |  | 1.4 | 100 % | 9.4 (2-17) | 13 | 9.4 (2-17) | 13 | 0 % | 40 | 34 | 97 | 0 | 0 |
| Agenda.DiscardLaw |  | 1.3 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 60 | 38 | 88 | 0 | 0 |
| Combat.Withdraw |  | 1.3 | 100 % | 4.5 (2-9) | 6 | 4.5 (2-9) | 6 | 0 % | 66 | 34 | 120 | 0 | 0 |
| Cards.Predict |  | 1.2 | 0 % | 3.1 (2-31) | 4 | 3.1 (2-31) | 4 | 0 % | 321 | 266 | 1442 | 0 | 0 |
| Invasion.RemoveCustodians |  | 1.0 | 0 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 168 | 22 | 65 | 0 | 0 |
| Promissory.OrbitalPlanet |  | 0.9 | 100 % | 10.1 (4-19) | 15 | 10.1 (4-19) | 15 | 0 % | 33 | 28 | 76 | 0 | 0 |
| Production.One |  | 0.9 | 0 % | 5.8 (2-7) | 7 | 5.8 (2-7) | 7 | 0 % | 1368 | 123 | 10004 | 0 | 0 |
| Legendary.Exterrix |  | 0.8 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 35 | 26 | 91 | 0 | 0 |
| Production.Economy |  | 0.7 | 0 % | 4.6 (3-7) | 7 | 4.6 (3-7) | 7 | 0 % | 3590 | 2778 | 12598 | 0 | 0 |
| Cards.PlayCard |  | 0.6 | 99 % | 3.0 (3-4) | 3 | 3.0 (3-4) | 3 | 0 % | 40 | 35 | 85 | 0 | 0 |
| Agenda.Predictive |  | 0.5 | 0 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 88 | 28 | 83 | 0 | 0 |
| Tactical.Hook |  | 0.5 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 50 | 47 | 116 | 0 | 0 |
| Technology.BioStims |  | 0.5 | 100 % | 2.8 (2-5) | 4 | 2.8 (2-5) | 4 | 0 % | 46 | 43 | 117 | 0 | 0 |
| Technology.Psychoarchaeology |  | 0.4 | 100 % | 2.4 (2-4) | 3 | 2.4 (2-4) | 3 | 0 % | 45 | 39 | 94 | 0 | 0 |
| Agenda.RiderTarget |  | 0.3 | 100 % | 5.2 (2-13) | 8 | 5.2 (2-13) | 8 | 0 % | 39 | 35 | 83 | 0 | 0 |
| Strategy.CrisisPact |  | 0.3 | 0 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 43 | 39 | 81 | 0 | 0 |
| Relic.GainTechnologyFrom |  | 0.2 | 0 % | 35.3 (28-38) | 38 | 35.3 (28-38) | 38 | 0 % | 51 | 44 | 137 | 0 | 0 |
| Turn.MinisterToken |  | 0.2 | 100 % | 1.2 (1-3) | 2 | 1.2 (1-3) | 2 | 77 % | 34 | 30 | 66 | 0 | 0 |
| Reaction.InstinctTraining |  | 0.2 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 46 | 45 | 92 | 0 | 0 |
| Production.SlingShip |  | 0.2 | 0 % | 4.5 (2-6) | 6 | 4.5 (2-6) | 6 | 0 % | 2005 | 1542 | 7015 | 0 | 0 |
| Promissory.OrbitalDeploy |  | 0.2 | 0 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 4076 | 1407 | 78402 | 0 | 0 |
| Strategy.ArbiterGive |  | 0.2 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 35 | 32 | 91 | 0 | 0 |
| Strategy.ArbiterTake |  | 0.2 | 100 % | 6.0 (6-6) | 6 | 6.0 (6-6) | 6 | 0 % | 31 | 29 | 80 | 0 | 0 |
| Tactical.Scanlink |  | 0.2 | 100 % | 2.2 (2-4) | 3 | 2.2 (2-4) | 3 | 0 % | 46 | 42 | 103 | 0 | 0 |
| Relic.CrownOfEmphidia |  | 0.1 | 100 % | 10.1 (8-14) | 12 | 10.1 (8-14) | 12 | 0 % | 51 | 49 | 83 | 0 | 0 |
| Agenda.Crater |  | 0.1 | 100 % | 3.4 (2-7) | 7 | 3.4 (2-7) | 7 | 0 % | 32 | 28 | 51 | 0 | 0 |
| Production.SlingSystem |  | 0.1 | 100 % | 2.0 (2-3) | 2 | 2.0 (2-3) | 2 | 0 % | 37 | 36 | 64 | 0 | 0 |
| Relic.Codex |  | 0.1 | 100 % | 18.5 (3-30) | 29 | 18.5 (3-30) | 29 | 0 % | 39 | 44 | 70 | 0 | 0 |
| Agenda.Committee |  | 0.1 | 100 % | 4.0 (4-4) | 4 | 4.0 (4-4) | 4 | 0 % | 28 | 23 | 53 | 0 | 0 |
| Relic.DominusOrb |  | 0.1 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 46 | 46 | 70 | 0 | 0 |
| Agenda.NameTied |  | 0.1 | 100 % | 2.1 (2-3) | 3 | 2.1 (2-3) | 3 | 0 % | 32 | 30 | 55 | 0 | 0 |
| Agenda.Structure |  | 0.1 | 100 % | 3.0 (2-6) | 4 | 3.0 (2-6) | 4 | 0 % | 45 | 40 | 71 | 0 | 0 |
| Technology.Aida |  | 0.1 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 2377 | 46 | 28063 | 0 | 0 |
| Agenda.Settler |  | 0.1 | 100 % | 2.4 (2-4) | 3 | 2.4 (2-4) | 3 | 0 % | 42 | 39 | 101 | 0 | 0 |
| Relic.GainTechnology |  | 0.0 | 0 % | 33.4 (29-36) | 36 | 33.4 (29-36) | 36 | 0 % | 48 | 51 | 69 | 0 | 0 |
| Relic.NanoForge |  | 0.0 | 100 % | 9.1 (4-18) | 18 | 9.1 (4-18) | 18 | 0 % | 29 | 28 | 43 | 0 | 0 |
| Relic.TitanPrototype |  | 0.0 | 100 % | 4.0 (4-4) | 4 | 4.0 (4-4) | 4 | 0 % | 40 | 41 | 70 | 0 | 0 |
| Relic.StellarConverter |  | 0.0 | 100 % | 6.2 (3-11) | 11 | 6.2 (3-11) | 11 | 0 % | 26 | 22 | 38 | 0 | 0 |
| Legendary.PlaceFighters |  | 0.0 | 100 % | 4.3 (4-5) | 5 | 4.3 (4-5) | 5 | 0 % | 30 | 28 | 39 | 0 | 0 |
| Reaction.G |  | 0.0 | 0 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 40 | 55 | 55 | 0 | 0 |
| Relic.DeviceTechnology |  | 0.0 | 0 % | 18.5 (12-25) | 25 | 18.5 (12-25) | 25 | 0 % | 55 | 84 | 84 | 0 | 0 |
| Relic.Neuraloop |  | 0.0 | 100 % | 2.0 (2-2) | 2 | 2.0 (2-2) | 2 | 0 % | 29 | 32 | 32 | 0 | 0 |

filter Strategy.TeWarfareTactical: passthrough 0.4 %, rule-bot pick dropped by the filter 9.93 % (123), kept options per ask by reason move 14.2 / produce 1.4 / shoot 1.2; rule-bot picks with reason move 75.5 % / produce 29.6 % / shoot 14.6 %

filter Tactical.Activate: passthrough 0.0 %, rule-bot pick dropped by the filter 0.93 % (140), kept options per ask by reason move 16.7 / produce 1.3 / shoot 2.2; rule-bot picks with reason move 92.0 % / produce 16.0 % / shoot 15.8 %

asks by phase (per game): [("Action", "1630"), ("Agenda", "183"), ("Status", "92"), ("Strategy", "50")]

## Delegated asks between two tree nodes

| delegated asks | share |
|---|---|
| 0 | 57.0 % |
| 1 | 13.1 % |
| 2 | 10.0 % |
| 3-4 | 7.8 % |
| 5-8 | 6.0 % |
| 9-16 | 3.5 % |
| 17-32 | 1.1 % |
| 33+ | 1.5 % |

mean delegated asks per segment 2.39

## Dice (committed roll records of the ledger)

rolls (requests) per game 141, dice per game 221

| purpose | records / game | dice / record | no threshold (faces read) | rerolled | dice-count histogram |
|---|---|---|---|---|---|
| Ability | 0.3 | 2.37 | 57.4 % | 0.0 % | 1:2 2:39 3:27 |
| AntiFighterBarrage | 4.7 | 2.03 | 0.0 % | 0.0 % | 2:920 3:28 |
| Bombardment | 7.6 | 1.84 | 0.0 % | 0.0 % | 1:745 2:496 3:78 4:209 |
| GroundCombat | 36.4 | 1.58 | 0.0 % | 0.0 % | 1:4273 2:2111 3:673 4:173 5:34 6:10 7:9 8:2 10:1 |
| Other | 0.8 | 1.39 | 0.0 % | 0.0 % | 1:125 2:25 3:4 4:4 5:3 7:1 |
| Reroll | 10.1 | 2.05 | 0.0 % | 100.0 % | 1:640 2:893 3:313 4:117 5:18 6:27 7:7 |
| SpaceCannon | 28.1 | 1.41 | 0.0 % | 0.0 % | 1:4040 2:911 3:610 4:53 |
| SpaceCombat | 52.8 | 1.47 | 0.0 % | 0.0 % | 1:6859 2:2761 3:665 4:202 5:60 6:10 7:2 |

## Canonical move order

movement steps 15769 (78.8 / game); length histogram {0: 3546, 1: 9961, 2: 1626, 3: 478, 4: 109, 5: 36, 6: 9, 7: 4}
already canonical 14794 (93.8 %), reordered and reachable 943 (6.0 %), reachable only out of canonical order 22 (0.14 %), replay inconclusive 10

