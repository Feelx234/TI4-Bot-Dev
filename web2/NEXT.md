Also look for UX ideas on how to integrate this: in web/, and in the demo of web2 (`?example=…`, the code in `web2/src/mock/`), which shows screens that are not wired to the engine yet.

- **important: the draft must go on to the invasion and the production wherever nothing random lies between** (no dice, no decision of another seat). Today the draft ends with the movement: the run does not answer "done moving" (`ti4-view/src/planning/draft.rs`). The server already plans these steps, so most of the code is there: its planning runner (`ti4-server/src/planning/runner.rs`) and the audit of the `commit_ground_forces` and production offers (`ti4-view/src/planning/audit.rs`). The landings have their screen in Live (2026-10-10); the draft should use the same screen.
- the invasion screen covers the landings only: space cannon defense, ground combat, control and exploration after them are still lists, and so are bombardment before them and the space combat. The Invasion tab shows the steps before it as done, also when the game skipped them.
- the production step is still a list
- a step of the landings is matched to an option in TypeScript only (`planAnswer`); the server's batch has no landing step yet
- on a phone a row of the panel summary (movement, landings) opens the sheet but does not open the Map pane
- a question of the seat before its turn menu (a start-of-turn ability) stops the draft at its first step; the server's runner shows such a question
- the draft has no way to change the system but Undo and Start over: the activation step of a draft should open for an edit, as in the demo
- the action picker of the demo, so that a tactical action opens as the draft
- state should be inspectable and wired up next (exhausted planets with dotted ring instead of solid, objectives, technology, cards, ...)
- payment is also painful and should be done via the map (like the demo of web2, `?example=draft-production`)
- map space combat view should be similar to web/
- everywhere a system name is used (e.g. move summary, later event log, ...): on desktop hover should highlight, click should be like click on the system on the map. on mobile map view is opened with system selected. The same with planets.
- we should also show the command tokens of other players that activate systems (we need to test the UI how to do this for a system where 8 players have activated it)
- the scenario "Late game" (Dev sheet) found these:
  - Antimass Deflectors does nothing in the engine. The flag that opens asteroid fields (`MovementRules::asteroid_fields_open`) is only ever set in a test. The asteroid field is closed in both scenario variants, so "Late game with deflectors" really only shows Light/Wave Deflector. A test in `ti4-view/src/scenario.rs` pins this and will fail when the engine implements it. Hacan starts with this tech, so fixing it changes existing seeded games: owner decision.
  - The 8-seat map has no asteroid field, so the scenario replaces tile 31 with tile 44 next to Sol's home.
  - Upgraded units showed as "Cruiser": web2 did not know ids like `sol_carrier2` or `fighter2`. Fixed in `names.ts`; they now show as their base unit, with no "II" marker.
