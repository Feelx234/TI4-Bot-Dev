# Pausable ability prompts: audit

Request: every optional reaction the engine asks a player about repeatedly (an opponent's activation, movement, a combat round, a roll, a production, ...) should have the per-browser 'pause until I switch it back on' switch that I48S and Aetherpassage have.

The engine is unchanged: it asks every question every time, rules, checksum and decision hashes are as before. A paused ability is declined by the client (an ordinary recorded `submit_choice` of the decline answer).

## How the audit was done

- Reaction windows: every `Ability::stateful(..).with_optional(true)` in `crates/ti4-engine/src` (126 sites, 115 with a literal slot id plus the Argent unit-ability slot; test-only and `game.rs` fixtures excluded). The window question is `reaction_<relation>_<EVENT>` with one `kind: ability` option per eligible ability and a decline; the option id is the slot id (`leader:<faction>:<leader>:<EVENT>:<relation>`, `technology:...`, `ability:...`, `unit:...`, `breakthrough:...`, `promissory:...`).
- Questions asked by an ability itself (outside a window): every `DecisionContext` subtype in `faction_abilities.rs`, `faction_techs.rs`, `technology.rs`, `combat.rs`, `vote.rs`, `relics.rs`, `production.rs`, `leaders.rs`, `factions/mentak.rs`, `factions/yin.rs`, whose answers include a decline.
- Action card windows (`reaction:<faction>:<EVENT>:<relation>`): already handled by the seat's `reaction_card_modes`; listed as covered, not duplicated.
- Rule of thumb for the verdict: TOGGLE when the question is asked on a frequent event (every activation, movement, combat round, roll, production, unit loss, command token, trade-good gain, turn start or end, action card, vote) and the card is a standing ability (faction ability, technology, agent, commander, mech, flagship, breakthrough, relic). NO for one-shot cards (promissory notes return after use, heroes are purged), once-per-round abilities, own-choice abilities that only fire in a rare position (own conquest, own win), passive effects, and anything that is not a question. A free 'may gain' (Magmus, Tungstantus, Farran) is toggleable too: pausing it gives up the gain, the player decides.

Counts (table rows; a not-needed row can name several cards): 65 registered (toggle), 1 covered by an existing mechanism (action card modes), 53 not needed, 2 not skippable (no decline, a rule).

Not found as a recurring optional question with a decline in this build (listed as not needed with the reason): Hacan agent and commander, Jol-Nar agent, Letnev agent, Xxcha agent and commander, Mahact agent, L1Z1X commander (passive).

## Registered abilities (toggle)

Storage key `player_ask_<id>` in localStorage, default ask. The registry is `web/src/presentation/pausablePrompts.ts`; adding an ability is one `prompt(...)` line. Ssruu (Yssaril) copies of an agent match the agent's entry (the offer shows the checkbox; the sheet lists the entry for its own faction).

| id | Faction | Ability | Kind | Trigger | Asked / how often |
|---|---|---|---|---|---|
| `i48s` | L1Z1X | I48S (and Ssruu copy) | agent | SYSTEM_ACTIVATED after | owner, any activation; every activation while readied |
| `aetherpassage` | Empyrean | Aetherpassage | faction ability | SYSTEM_ACTIVATED after | owner, any activation; every activation |
| `instinct_training` | Xxcha | Instinct Training | technology | ACTION_CARD_PLAYED when | owner, others' cards; every action card played by another player |
| `nullification_field` | Xxcha | Nullification Field | technology | after another player activates a system with your ships (inline question) | owner; every such activation |
| `arborec_commander` | Arborec | Dirzuga Rophal | commander | SYSTEM_ACTIVATED after | owner, others' activations; every activation next to your producing units |
| `argent_agent` | Argent | Trilossa Aun Mirik (and Ssruu copy) | agent | GROUND_FORCES_BEING_PRODUCED when | owner, anyone's production; every ground-force production while readied |
| `argent_commander` | Argent | Trrakan Aun Zulok | commander | UNIT_ABILITY_ROLLED when | owner, own rolls; every Space Cannon / Bombardment / AFB roll |
| `bastion_agent` | Bastion | Dame Briar | agent | unit destroyed when | owner, anyone's losses; every destroyed unit while readied |
| `phoenix_standard` | Bastion | Phoenix Standard | faction ability | end of combat | owner; every combat you fight |
| `proxima_targeting` | Bastion | Proxima Targeting VI | technology | GROUND_COMBAT_ROUND_BEGAN after | owner; every ground combat round with a usable bombardment |
| `cabal_agent` | Vuil'raith | The Stillness of Stars | agent | COMMODITIES_REPLENISHED after | owner, others' replenishing; every replenish by another player (about once a round per player) |
| `crimson_exile` | Crimson Rebellion | Exile I/II | unit | end of any combat in or next to the unit's system | owner; every combat nearby |
| `subatomic_splicer` | Crimson Rebellion | Subatomic Splicer | technology | SHIP_DESTROYED after (own ship) | owner; every own ship lost |
| `null_reference` | Nekro | Null Reference | technology | SHIP_DESTROYED after (own ship) | owner; every own ship lost |
| `technological_singularity` | Nekro | Technological Singularity | faction ability | opponent's unit destroyed after | owner; every opponent loss until used (once per combat) |
| `aetherstream` | Empyrean | Aetherstream | technology | SYSTEM_ACTIVATED after (you or a neighbor) | owner; every qualifying activation |
| `empyrean_agent` | Empyrean | Acamar | agent | MOVEMENT_FINISHED after | owner, anyone's moves; every movement into an empty system while readied |
| `empyrean_commander` | Empyrean | Xuange | commander | SHIP_MOVED after | owner, others' moves; every move into a system with your token |
| `dynamo` | Empyrean | Dynamo | flagship | SUSTAIN_DAMAGE after | owner, anyone's sustains; every sustain next to the flagship |
| `watcher` | Empyrean | Watcher | mech | ACTION_CARD_PLAYED when | owner, others' cards; every action card played in reach of a mech |
| `void_tether` | Empyrean | Void Tether (allow) | breakthrough | SYSTEM_ACTIVATED after | owner, others' activations; every other player's activation |
| `firmament_agent` | Firmament | Myru Vos | agent | SYSTEM_ACTIVATED after | owner, any activation; every activation while readied |
| `ghost_agent` | Creuss | Emissary Taivra (and Ssruu copy) | agent | SYSTEM_ACTIVATED after | owner, any wormhole activation; every wormhole activation while readied |
| `icarus_drive` | Creuss | Icarus Drive | mech | SYSTEM_ACTIVATED after | owner, any activation; every activation while a mech is on the board |
| `minister_of_peace` | (law owner) | Minister of Peace | law | offer_discard_law, content source `minister_peace` | owner of the law, after an activation of a system with another player's units; listed on the sheet while the law is held. Shares the subtype with the other ministers, told apart by the question's content source |
| `laws_order` | Keleres | Laws Order | faction ability | TURN_BEGAN after | owner, every player's turn; every turn start while a law is in play |
| `pillage` | Mentak | Pillage | faction ability | TRADE_GOODS_GAINED / TRANSACTION_RESOLVED after | owner, neighbors' gains; every neighbor gain or trade with 3+ trade goods |
| `ambush` | Mentak | Ambush | faction ability | start of space combat (inline question) | owner; every space combat with cruisers/destroyers |
| `muaat_commander` | Muaat | Magmus | commander | STRATEGY_TOKEN_SPENT after | owner, own spending; every strategy token spent |
| `naalu_agent` | Naalu | Z'eu (and Ssruu copy) | agent | COMMAND_TOKEN_PLACED after | owner, anyone's tokens; every command token placed while readied |
| `foresight` | Naalu | Foresight | faction ability | MOVEMENT_FINISHED after | owner, others' moves; every movement by another player |
| `naaz_agent` | Naaz-Rokha | Garv and Gunn (and Ssruu copy) | agent | TURN_PASSED after | owner, every turn end; every turn end while readied |
| `thunders_paradox` | Nomad | Thunder's Paradox | breakthrough | TURN_BEGAN after | owner, every turn; every turn start |
| `temporal_command_suite` | Nomad | Temporal Command Suite | technology | AGENT_EXHAUSTED after | owner, any agent use; every agent exhaustion |
| `nomad_artuno` | Nomad | Artuno the Betrayer | agent | TRADE_GOODS_GAINED after | owner, own gains; every trade-good gain while readied |
| `nomad_mercer` | Nomad | Field Marshal Mercer | agent | TACTICAL_ACTION_ENDED after | owner, anyone's turn; every tactical action end while readied |
| `nomad_thundarian` | Nomad | The Thundarian | agent | after the Roll Dice step | owner, any combat; every combat round while readied |
| `obsidian_agent` | Obsidian | Vos Hollow | agent | SHIP_DESTROYED when | owner, anyone's losses; every ship destroyed while readied |
| `alarum` | Ral Nel | Alarum | mech | GROUND_COMBAT_ROUND_ENDED after | owner; every ground combat round on its planet |
| `saar_agent` | Saar | Captain Mendosa (and Ssruu copy) | agent | SYSTEM_ACTIVATED after | owner, any activation; every activation while readied |
| `exotrireme` | N'orr | Exotrireme II | technology | SPACE_COMBAT_ROUND_ENDED after | owner; every combat round it fights in |
| `sardakk_agent` | N'orr | T'ro (and Ssruu copy) | agent | ACTION_COMPLETED / TACTICAL_ACTION_ENDED after | owner, anyone's turn; every tactical action end while readied |
| `titans_agent` | Titans | Tellurian | agent | HITS_TO_ASSIGN when | owner, any hit; every hit assignment while readied |
| `titans_commander` | Titans | Tungstantus | commander | PRODUCTION_USED when | owner, own production; every production |
| `hegemonic_trade_policy` | Winnu | Hegemonic Trade Policy | technology | PRODUCTION_USED after | owner, own production; every production while ready |
| `winnu_agent` | Winnu | Berekar Berekon (and Ssruu copy) | agent | PRODUCTION_USED after | owner, others' production (own production: the "Exhaust Berekar Berekon" option of the production dialog from rules version 26); every production while readied |
| `yin_spinner` | Yin | Yin Spinner Omega | technology | UNITS_PRODUCED after | owner, own production; every production |
| `yin_agent` | Yin | Brother Milor (and Ssruu copy) | agent | SHIP_DESTROYED / GROUND_FORCE_DESTROYED after | owner, anyone's combat losses; every combat loss while readied |
| `indoctrination` | Yin | Indoctrination | faction ability | start of ground combat (inline question) | owner; every ground combat against infantry |
| `yssaril_commander` | Yssaril | So Ata | commander | SYSTEM_ACTIVATED after | owner, others' activations; every activation of a system holding your units |
| `letnev_agent` | Letnev | Viscount Unlenn | agent | COMBAT_ROUND_STARTED after | owner (and Ssruu copy); every space combat round while readied |
| `sol_agent` | Sol | Evelyn DeLouis | agent | GROUND_COMBAT_ROUND_STARTED after | owner; every ground combat round while readied |
| `sol_commander` | Sol | Claire Gibson | commander | GROUND_COMBAT_STARTED after | owner; every ground combat on a planet you control |
| `munitions_reserves` | Letnev | Munitions Reserves | faction ability | combat round (inline question) | owner; every space combat round with 2+ trade goods |
| `letnev_commander` | Letnev | Rear Admiral Farran | commander | own unit sustains damage (inline question) | owner; every sustain |
| `wrath_of_kenara` | Hacan | Wrath of Kenara | flagship | after your combat roll (inline question) | owner; every combat roll with trade goods |
| `self_assembly_routines` | any | Self-Assembly Routines | technology | after your PRODUCTION (inline question) | owner; every production while ready |
| `ai_development_algorithm` | any | AI Development Algorithm | technology | your PRODUCTION (inline question) | owner; every production with upgrades |
| `agency_supply_network` | Keleres | Agency Supply Network | technology | your PRODUCTION (inline question) | owner; every production |
| `scanlink_drone_network` | any | Scanlink Drone Network | technology | your activation (inline question) | owner; every own activation |
| `spatial_conduit_cylinders` | Jol-Nar | Spatial Conduit Cylinders | technology | your activation (inline question) | owner; every own activation while ready |
| `bio_stims` | any | Bio-Stims | technology | end of your turn (inline question) | owner; every own turn end while ready |
| `predictive_intelligence` | any | Predictive Intelligence | technology | end of your turn; your vote (inline questions) | owner; every own turn end while ready; every vote |
| `jolnar_commander` | Jol-Nar | Agnlan Oln | commander | after your unit-ability rolls (inline, one question per die) | owner; every Space Cannon / Bombardment / AFB roll |
| `crown_of_thalnos` | relic | The Crown of Thalnos | relic | combat round dice (inline, one question per die) | owner; every combat round |
| `heart_of_ixth` | relic | Heart of Ixth | relic | after your combat roll (inline question) | owner; every combat round while ready |

## Covered by an existing mechanism

| Faction | Ability | Trigger | Mechanism |
|---|---|---|---|
| any | Action cards with a window: Sabotage, Shields Holding, Direct Hit, Maneuvering Jets, Skilled Retreat, Courageous to the End, Bunker, ... | every printed window (`reaction:<faction>:<EVENT>:<relation>` slots) | Existing seat setting `reaction_card_modes` (Always / Never / own turns only, 'Never offer X again this game', server state per game). Not duplicated; the pause toggles are per browser and cover abilities only. |

## Not needed or not skippable

| Faction | Ability | Kind | Trigger | How often | Verdict and reason |
|---|---|---|---|---|---|
| Arborec | Bioplasmosis | technology | STATUS_PHASE_ENDED after | once per round | no: once per round |
| Arborec | Duha Menaimon | flagship | own activation of the flagship's system | only when the flagship is in the system you activate | no: own deliberate choice, rare |
| Arborec | Stymie | promissory note | SHIP_MOVED after | once (card returns) | no: one-shot note |
| Argent | Wing Transfer | breakthrough (Thunder's Edge) | own activation of a lone system | few per game | no: own deliberate choice, conditional |
| Bastion | Phoenix Standard hero / Lyra Keen | hero | unit destroyed when | once (purged) | no: hero: one-shot |
| Crimson Rebellion | Incursion (flip) | faction ability | own activation of a breach system | few per game | no: own deliberate choice, conditional |
| Crimson Rebellion | Incursion remove | faction ability | STATUS_PHASE_ENDED after | once per round | no: once per round |
| Crimson Rebellion | Homesick Phantom | hero | SPACE_COMBAT_STARTED after | once (purged) | no: hero: one-shot |
| Deepwrought | Radical Advancement | technology | STATUS_PHASE_BEGAN after | once per round | no: once per round |
| Deepwrought | Eanautic | mech | another player activates this system | only for the system the mech stands in | no: rare |
| Empyrean | Void Tether (place) | breakthrough | own activation of a system next to your units | own turns, conditional | no: own deliberate choice |
| Firmament | Plot: Extract | faction ability | TECHNOLOGY_GAINED after (puppeted player) | rare | no: rare, tied to puppets |
| Creuss | Creuss Iff | promissory note | TURN_BEGAN after | once (card returns) | no: one-shot note |
| Keleres | Suffi An (commander) | commander | ACTION_COMPLETED after | a few component actions per game | no: rare |
| Keleres | Rider, Odlynn Myrr, Kuuasi Aun Jalatai | note / heroes | agenda reveal / combat start | once | no: one-shot cards, heroes |
| Mentak | Suffi An (agent) | agent | PILLAGE_USED after | after each Pillage only | no: rare |
| Mentak | S'ula Mentarion | commander | SPACE_COMBAT_WON after | own wins only (a few per game) | no: own win, rare |
| Mentak | Salvage Operations | technology | SPACE_COMBAT_ENDED produce | own wins only | no: own win, rare |
| Mentak | Ipswitch (hero) | hero | SPACE_COMBAT_STARTED after | once | no: hero: one-shot |
| Muaat | Adjudicator Ba'al (hero), Stellar Genesis | hero / breakthrough | MOVEMENT_FINISHED / SHIP_MOVED | once / rare | no: hero one-shot; conditional breakthrough |
| Naalu | Gift of Prescience | promissory note | STRATEGY_PHASE_ENDED after | once | no: one-shot note |
| Naalu | Mech (Thunder's Edge DEPLOY) | mech | RELIC_GAINED after | rare | no: rare |
| Naalu | The Oracle | hero | STATUS_PHASE_ENDED after | once | no: hero: one-shot |
| Naaz-Rokha | Dart and Tai | commander | PLANET_CONTROL_GAINED after | own conquests of other players' planets (a few per game) | no: own conquest, rare |
| Naaz-Rokha | Absolute Synergy | breakthrough | own turn with 4 mechs in a system | rare | no: conditional, rare |
| Nekro | Galactic Threat (prediction) | faction ability | AGENDA_REVEALED after | once per agenda (2-3 per game) | no: rare |
| Nekro | Antivirus | promissory note | start of combat | once | no: one-shot note |
| Nekro | Nekro Acidos | commander | TECHNOLOGY_GAINED after | a few per game | no: own research, rare |
| Nomad | Mech / Cavalry | promissory note | SPACE_COMBAT_STARTED after | once | no: one-shot note |
| Obsidian | Planesplitter, Malevolency | technology / note | own strategic action / own tactical end | once per round / one-shot | no: once per round or one-shot |
| Ral Nel | Survival Instinct | faction ability | SYSTEM_ACTIVATED after | own activation, conditional | no: own deliberate choice, conditional |
| Ral Nel | Miniaturization | faction ability | TACTICAL_ACTION_ENDED after | own turn end, conditional | no: own deliberate choice, conditional |
| Ral Nel | Flagship retreat, Nano-Link Permit, Director Nel | flagship / note / hero | retreat / own activation / pass | rare / one-shot | no: rare or one-shot |
| Saar | Scavenger Zeta | mech | PLANET_CONTROL_GAINED after | own conquests (a few per game) | no: own conquest, costs trade goods |
| Saar | Raghs Call | promissory note | UNITS_COMMITTED after | once | no: one-shot note |
| N'orr | N'orr Supremacy | breakthrough | after you win a combat | own wins | no: own win, rare |
| N'orr | Tekklar Legion | promissory note | GROUND_COMBAT_STARTED after | once | no: one-shot note |
| Titans | Terragenesis, Awaken, Ouranos | faction ability / flagship | own explore / own activation | own turns, conditional | no: own deliberate choice, conditional |
| Titans | Sleeper placement | breakthrough | TURN_BEGAN after | once per round | no: once per round |
| Winnu | Reclamation, Reclaimer | faction ability / mech | ACTION_COMPLETED after | rare | no: rare |
| Winnu | Imperator | breakthrough | own activation of a legendary planet system | rare | no: conditional, rare |
| Yin | Greyfire Mutagen | promissory note | GROUND_COMBAT_STARTED after | once | no: one-shot note |
| Yssaril | Spy Net | promissory note | TURN_BEGAN after | once | no: one-shot note |
| Sol | Orbital Drop, Spec Ops | faction ability / unit | own action / own destruction | action menu / automatic | no: not a reaction question |
| Hacan | Carth, Gila the Silvertongue | agent / commander | action menu / your vote (Gila) | action / once per vote | no: agent is a component action; Gila only asks while voting (about twice a game) |
| Jol-Nar | Doctor Sucaban | agent | when a player researches (research step) | every research while readied | no: asked inside the research step (sucaban_sources), not as a reaction window with its own decline in this build; the only prompt is a source/target pick, so there is nothing to pause without changing the step |
| Letnev | Viscount Unlenn | agent | start of a space combat round | every round while readied | no: offered through the leader menu / Ssruu copy only in this build, not as a recurring question; Ssruu copy is a window option (not registered: Yssaril sees it as an own-agent copy) |
| Xxcha | Ggrocuto Rinn, Elder Qanoj | agent / commander | component action / passive | action / no question | no: component action or passive effect |
| Mahact | Jae Mir Kan, edicts, Il Na Viroset | agent / ability / commander | secondary spending / action | rare / passive | no: asked only when you would spend a token; not recurring |
| L1Z1X | 2RAM, Dimensional Tear | commander / unit ability | bombardment shield / passive | automatic | no: no question (passive); nothing to decline |
| any | Magen Defense Grid | technology | SYSTEM_ACTIVATED when / GROUND_COMBAT_STARTED after | every opposed activation | not skippable: mandatory trigger (a rule): the engine asks only which ground force takes the hit and offers no decline |
| any | Infantry II revival, Devour, Scavenge, Mitosis | technology / faction abilities | unit destroyed etc. | automatic | not skippable: not optional: no question is asked (a die roll or a fixed gain) |
| any | Quantum Datahub Node, Dominus Orb, Neuraloop, Codex, Crown of Emphidia, War Funding, Dimensional Splicer, Psychoarchaeology, Chaos Mapping | technologies / relics / notes | once per round or once per game | once | no: once per round or one-shot |
| any | Antimass Deflectors, Graviton Laser System, Neural Motivator, Sarween Tools | technologies | passive | automatic | no: passive effects: the engine asks nothing |
| any | Hacan Arbitrage / Quantum Dimension questions (trade) | faction abilities | transactions | as the player wishes | no: initiated by the player, not offered as a reaction |

## Safety rules of the client decline

- Only the offer's actor (the viewer's own seat), only when the question has a decline answer.
- Reaction window: only when every non-decline option is a registered, paused ability. Any other ability or an action card slot in the same window keeps the question open.
- Own question (decision subtype): declined when its ability is paused.
- Only after a forward move of the game (history cursor grew, nothing to redo), never after an undo, redo, restore or page load; the decision is taken when the offer appears (switching off while it is open leaves that one to the player).
- A toast `<ability> skipped (switched off)` is shown; the setting is per browser (localStorage, wrapped in try/catch, failure means ask).

## UI

- Offer dialog: a checkbox per registered ability the question is about (reaction dialog; for an ability that asks by itself a small strip under the question).
- Player sheet: one button per switchable ability while the seat has three or fewer (I48S and Aetherpassage keep their buttons and test ids), one `Prompts` button above that, opening the list with `Ask` / `Skipped` switches. An ability of another faction is listed while it is skipped.
- A chip `N prompts skipped` under the top bar while anything is paused; click opens the list.

Screens: `web/e2e/screenshots/FV-ability-pause-toggles/` (`npm run screenshots -- FV`).
