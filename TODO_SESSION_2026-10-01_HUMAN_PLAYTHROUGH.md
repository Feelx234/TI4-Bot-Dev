# Human Playthrough Findings - Session 2026-10-01

Generated from human playthrough testing. Priority levels: 🔴 Critical, 🟠 High, 🟡 Medium, 🔵 Low

---

## 🔴 CRITICAL - Crashes & Blocking Issues

### C1: Bot Crash with Diplomacy Card
- **Link:** https://ti4.beckermann-zibert.com/games/game_64a6e64f5568b4974af228fec0c10592
- **Description:** Bot crashed when Diplomacy card was played
- **Status:** NEEDS INVESTIGATION
- **Reproducible:** TBD

### C2: Initial Map Generation Broken
- **German:** "Initiale map generation ist kaputt"
- **Description:** Map generation has issues (mentions pre-fabricated maps somewhere)
- **German:** "Planeten initiale Setup ist broken (es gibt irgendwo vorgefertigte Maps)"
- **Status:** NEEDS INVESTIGATION
- **Reproducible:** TBD

### C3: React Warning - Duplicate Keys in Action List
- **Error:** `Encountered two children with the same key, 'round:2:action:action:action_295'`
- **Description:** Keys should be unique so components maintain identity across updates
- **Location:** Likely in action cards/decisions rendering
- **File:** TBD
- **Status:** NEEDS INVESTIGATION

---

## 🟠 HIGH PRIORITY - Major UI/UX Issues

### H1: Hacan Trade System Broken
- **German:** "Clan fähigkeiten anzeigen können" / "Trading for hacan seems broken" / "Hacan Trade funktioniert nicht"
- **Impact:** Core faction ability not working
- **Test Link:** https://ti4.beckermann-zibert.com/games/game_f49965ba37a5a31f310befa69744f1e0
- **Status:** NEEDS INVESTIGATION
- **Reproducible:** TBD

### H2: Invasions Not Displayed Properly
- **Description:** "do not show the boring invasions to other player (i.e. invasions without a fight)"
- **German:** "Invasion resettet in die Standard Ansicht (eigentlich sogar jede)"
- **Issue:** View resets to standard when invasions occur; should only show fights to other players
- **Status:** NEEDS FIX

### H3: Payment & Build Flow Broken
- **German:** "bezahlen der Units ist komisch (mehrere Schritte)" / "Payment und Build klappt nicht richtig"
- **Description:** Multiple payment actions required instead of single flow
- **Also:** "Beim automatischen Bezahlen werden alle Planeten zuerst genommen (auch wenn es trade goods gibt)"
- **Status:** NEEDS FIX

### H4: Simultaneous Action Pipeline Missing
- **German:** "Technology Decision sollte gleichzeitig möglich sein" / "simultaneous action pipeline (warfare, technology, strategy cards)"
- **Description:** Technology, Warfare, and many Strategy Cards should allow simultaneous pre-selection before turn order
- **Status:** NEEDS DESIGN & IMPLEMENTATION

### H5: Action Card Triggers Not Displayed
- **German:** "bei allen Action Cards fehlt der trigger Text" / "Alle trigger fehlen bei allen Action cards"
- **Description:** All action card trigger conditions are missing from UI
- **Example:** "Sabotage sollte anzeigen was man sabotiert" / "when responding to a card, show the card + text"
- **Status:** NEEDS IMPLEMENTATION

### H6: Choice Without Alternatives Should Auto-Select
- **Description:** "if there is only one choice, do not ask for the decision just make it for the player"
- **Examples:**
  - No resources available
  - Last strategy card in 4-player game (no choice)
  - Only one strategic action left
  - Only one fleet able to activate
- **Status:** NEEDS CATALOGING & DECISIONS
- **Note:** Collect all such cases and decide which to auto-resolve

### H7: Undo Actions Without Randomness
- **German:** "Jeder Spieler kann während seines Zuges die nicht permanenten Entscheidungen wieder rückgängig machen" / "undo own actions without randomness"
- **Description:** Players should be able to undo non-permanent decisions during their turn
- **Status:** NEEDS DESIGN & IMPLEMENTATION

### H8: Exchange UI Unclear
- **German:** "das exchange ui ist unübersichtlich" / "beim exchange wird nicht angezeigt was die Karten machen die jemand zum Tausch anbietet"
- **Description:** Trade card UI doesn't show card descriptions/effects when offered in trade
- **Also:** "when using the trade card, allow to make all decisions at the same time"
- **Status:** NEEDS FIX

---

## 🟡 MEDIUM PRIORITY - UI/UX Improvements

### M1: Resource & Influence Icons
- **Description:** "Choose an icon for influence and resources and use them everywhere"
- **Status:** NEEDS DESIGN & IMPLEMENTATION

### M2: Combat Results Display
- **Description:** "Show results of a fight after it is done" / "pop ground combat summary"
- **Also:** "Combat Simulator wieder zum Laufen bekommen"
- **Status:** NEEDS IMPLEMENTATION

### M3: Turn Summary After Completion
- **Description:** "show a quick summary of the turn after it is completed. each player can enable/disable this for itself"
- **Status:** NEEDS DESIGN & IMPLEMENTATION

### M4: Token Hover Information
- **German:** "in the tokens add a hover that shows which of the tokens is strategy fleet, etc" / "Tokens tooltip hover what is what (tactical/fleet/strategy)"
- **Description:** Tooltip on tokens showing tactical/fleet/strategy distinction
- **Status:** NEEDS IMPLEMENTATION

### M5: Command Token Recall via Map
- **Description:** "when playing warfare strategy card, let players choose which command token to recall by selecting a system on the map"
- **Status:** NEEDS IMPLEMENTATION

### M6: Invasion - Show Planet Resources
- **Description:** "During invasion, show the resources of the planets"
- **Status:** NEEDS IMPLEMENTATION

### M7: Planet Selection via Map (Everywhere)
- **German:** "in general whenever you choose a planet, the planet should be chosen through the map"
- **Applies To:** Planet exhaustion, Xxcha agent, agenda phase, etc.
- **Status:** NEEDS IMPLEMENTATION

### M8: Build Units - Tooltip with Abilities
- **Description:** "when building units add a tooltip that summarizes what the units do (e.g. movement, capacity, also add the description of the unit)"
- **Examples:** Flagship, Dreadnough bombardment
- **Status:** NEEDS IMPLEMENTATION

### M9: Token Redistribution Dialog
- **Description:** "when gaining multiple tokens at once, show a diagram with bar chart like diagrams"
- **Details:** Each bar = pips equal to token count; +/- buttons; show remaining tokens to redistribute; prevent invalid assignments
- **German:** Related to token assignment UI
- **Status:** NEEDS DESIGN & IMPLEMENTATION

### M10: Waiting Indicator
- **German:** "oben 'waiting for player' welche decision steckt man gerade"
- **Description:** "when waiting for another player, show what we are currently waiting on and which player we are waiting on"
- **Also:** "show the decision of the current active player as a summary"
- **Status:** NEEDS IMPLEMENTATION

### M11: Used Strategy Cards Grayed Out
- **German:** "benutzte Strategy cards sollten ausgegraut werden" / "Strategy Cards welche wurden benutzt"
- **Description:** Visually distinguish exhausted/used strategy cards
- **Status:** NEEDS IMPLEMENTATION

### M12: Victory Points Clarification
- **German:** "es ist unklar wo die victory points herkommen" / "Noch mal genauer anzeigen, woher jetzt die Punkte kommen"
- **Description:** Show breakdown of VP sources (objectives, Mecatol, Shards for the Throne, etc.)
- **Status:** NEEDS IMPLEMENTATION

### M13: Agenda Phase Display
- **German:** "in der agenda phase wurde die agenda nicht angezeigt" / "when performing politics, show the content of the cards when making choice"
- **Also:** "Politics Agenda Karten Text sehen können" / "ausgang des votings auf die agenda sollte angezeigt werden"
- **Description:** Show agenda cards and voting results in politics phase
- **Status:** NEEDS IMPLEMENTATION

### M14: Action Cards - Show Descriptions
- **German:** "Namen Action cards im Log sollten markiert sein. On hover sollte da stehen was das ist"
- **Description:** Action card names in event log should show tooltip with effect
- **Status:** NEEDS IMPLEMENTATION

### M15: Sound When It's Your Turn
- **Description:** "play a sound when it is your turn (mutable for each one individually)"
- **Status:** NICE TO HAVE

### M16: Player List - Current Player at Top
- **German:** "Der Spieler der man ist sollte oben sein, oder zumindest more condensed player cards"
- **Description:** Current player should be at top; more compact player cards to avoid scrolling
- **Status:** NEEDS IMPLEMENTATION

### M17: Mute Action Cards
- **German:** "Man sollte Action Cards 'muten' können"
- **Description:** Allow disabling/muting specific action cards (suppress offers)
- **Status:** NICE TO HAVE

### M18: Simulate Action Card
- **German:** "Man sollte vorgaukeln können eine Action Card zu haben"
- **Description:** Ability to fake having an action card (testing feature)
- **Status:** TESTING FEATURE

### M19: Fleet Supply Visibility in Production
- **German:** "Man sollte Fleet supply in Production sehen"
- **Description:** Show fleet supply limit during production phase
- **Also:** "Während Production sollte man sehen können in welchen steps man bezahlen kann"
- **Status:** NEEDS IMPLEMENTATION

### M20: Pre-selection for Invasions
- **German:** "default bei invasion schon vorausgewählt"
- **Description:** Pre-select default values in invasion dialogs
- **Status:** NEEDS IMPLEMENTATION

### M21: Payment Stages Warning
- **Description:** Show payment stages during production without wasting resources on planets
- **Status:** NEEDS IMPLEMENTATION

### M22: Newer Strategy Cards
- **German:** "Neuere Strategy Cards verwenden"
- **Description:** Update strategy card set to newer version
- **Status:** BACKLOG

---

## 🔵 LOW PRIORITY / Already in Backlog

### L1: Faction Abilities Display
- **German:** "Clan fähigkeiten anzeigen können"
- **Description:** Show faction-specific abilities clearly
- **Status:** In TODO.md already

### L2: Faction Name Visibility
- **German:** "immer auch name neben faction"
- **Description:** Always show faction name next to faction icon/symbol
- **Status:** NEEDS IMPLEMENTATION

### L3: Make Player Icons Larger
- **German:** "make all player specific icons (e.g. hex, square, circle) larger on the map especially"
- **Description:** Improve visibility of player faction icons on map
- **Status:** NICE TO HAVE

### L4: Mech Abilities Display
- **German:** "Mech abilities auf jeden Fall auch anzeigen"
- **Description:** Show mech abilities clearly
- **Status:** NEEDS IMPLEMENTATION

### L5: Monument Spending Bug
- **Description:** "Erect a Monument Spend 8 resources. (falsch gezählt, vielleicht falls einmal gezählt bleibt immer aktiv)"
- **Issue:** Monument spending count may be wrong or persist incorrectly
- **Status:** NEEDS INVESTIGATION

### L6: Items in Transit UI
- **German:** "Sachen unterwegs mitnehmen funktioniert in der UI nicht"
- **Description:** Items/units moving in transit not displayed correctly
- **Status:** NEEDS INVESTIGATION

### L7: Event Log Scrolling
- **German:** "Event Log scrollt komisch wenn was passiert"
- **Description:** Event log has scrolling issues when new events occur
- **Status:** NEEDS FIX

### L8: Influence Payment Issue
- **German:** "bug: einmal drei influence für einen command + dann ein planet / 2 TG"
- **Description:** Possible bug with influence payment combining with planet/trade goods
- **Status:** NEEDS INVESTIGATION

### L9: Faction Tech Display Bug
- **Description:** "Alle bekommen die Faction Techs von Player 1 angezeigt"
- **Issue:** All players see Player 1's faction technologies
- **Status:** NEEDS INVESTIGATION

### L10: "Blitz" Missing Triggers
- **German:** "'Blitz' in Opponent (außerdem fehlen alle Trigger)"
- **Description:** Blitz card and other action cards missing trigger text
- **Status:** Part of H5

### L11: Action Card Pinning
- **Description:** "play play action card" / "pin action card"
- **Status:** BACKLOG

---

## Investigation Queue

These items require detailed investigation via game replays/logs:

- C1: Diplomacy crash - https://ti4.beckermann-zibert.com/games/game_64a6e64f5568b4974af228fec0c10592
- H1: Hacan trade - https://ti4.beckermann-zibert.com/games/game_f49965ba37a5a31f310befa69744f1e0
- C2: Map generation
- C3: Duplicate key warning
- L5: Monument bug
- L6: Items in transit
- L8: Influence payment bug
- L9: Faction tech bug

---

## Investigation Reports

🎯 **START HERE - COMPLETE MASTER FINDINGS:** [MASTER_FINDINGS_COMPLETE_2026-10-01.md](./MASTER_FINDINGS_COMPLETE_2026-10-01.md)
- All 47 issues investigated and consolidated
- Root causes for every issue
- File locations, line numbers, reproduction steps
- Quick wins list (< 2 hours each)
- Complete 4-phase implementation roadmap
- Dependency map and priority matrix

📋 **Comprehensive findings from 12 parallel investigations:** [INVESTIGATION_FINDINGS_COMPILED.md](./INVESTIGATION_FINDINGS_COMPILED.md)
- Critical/High priority findings in detail
- Root causes with code examples
- Implementation recommendations

📋 **Detailed reproduction steps and findings:** [REPRODUCTION_INVESTIGATION_SESSION_2026-10-01.md](./REPRODUCTION_INVESTIGATION_SESSION_2026-10-01.md)

---

## Implementation Roadmap

### Phase 1: Critical Fixes (Blockers)
- Fix diplomacy card crash
- Fix map generation
- Fix duplicate key warning
- Fix Hacan trade

### Phase 2: High-Impact UI Fixes
- Fix invasions display
- Fix payment/build flow
- Implement action card triggers
- Auto-resolve single-choice decisions
- Implement undo for non-permanent decisions

### Phase 3: UX Improvements
- Implement simultaneous action pipeline
- Add resource/influence icons everywhere
- Improve waiting indicators
- Add missing tooltips and hover info
- Implement map-based selection for planets

### Phase 4: Polish
- Sound notifications
- Turn summaries
- Combat results display
- Better player list layout

