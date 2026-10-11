# web2: rules for agents

Read this before you change the UI in `web2/`. These are decisions of the project owner (2026-10-07).

## Consistency

- The UI must feel consistent. A new UX feature must work the same way on every screen where a player can expect it.
- Before you add an interaction (a key, a gesture, a navigation, a kind of control), check each other screen: does it apply there, and does it mean the same thing there?
- If it applies to one screen or a few screens only, do not add it. Use an interaction that the UI already has, or find one that applies everywhere.
- An exception needs a reason that a player can see on the screen, and a line in this file.

## Patterns

These rules hold on every screen. The feature sections below give the specifics and the exceptions.

1. **Controls are where the choice is made.** A control is next to the thing that it changes, not in another place that only has the room. Cases: the payment controls are on the map; the editor is in the row of the viewer in the seat list; the selected technology cell is the control.
2. **Everything that a choice needs is on one screen, with no click.** Prefer one overview to a view for each item. A view that needs a click for each item is the last resort. Case: the seat list shows what every player did.
3. **A staged change shows "now → after" in game values.** Cases: pools "3 → 5", "Trade goods 2 → 5", "Strategy pool 2 → 1".
4. **Colour is never the only carrier of a state.** A state also has a sign, a word or a shape. Cases: ▶ ✓ – ✎ · in the seat list; "+" and the empty outline on tokens; hollow prerequisite marks; "⚠ 1 wasted".
5. **An amount against a cost has three states, always the same.** Short is red and blocks the main button. Exact is green. More than the cost is orange with "⚠", and it is allowed.
6. **One signal for one state.** A state is a pill or a sign. Do not add a frame, a tint and a badge for the same state, and do not repeat it in several places of one screen.
7. **An element that never changes and repeats other content is removed.** Case: the pills for the stages of a strategy card.
8. **An amount changes in steps of one.** A button for the usual target can stand next to the counter ("Pay rest", "Auto-pay"). There is no all-or-nothing button for an amount.
9. **When a screen does not fit: move, restructure, make dense. Do not scroll.** In this order: put a summary into a row that exists on every screen (the footer, the heading, the header of a block); remove what repeats; make secondary rows dense. Only then ask the owner for an exception. Cases: the payment line in the footer; the chosen technologies in the header of the tree; the 24px rows of the seat list.
10. **Live, Draft, History and the event log use the same screen.** Read-only is the same screen with nothing open, not another view.
11. **Hidden information.** A choice of another player is visible when it is resolved, never before. A row of another player is not a control.
12. **Symbols follow the physical game** where the game has a convention. Case: command tokens are triangles, and the fleet pool points up.
13. **Names and long texts.** A name is in a column of fixed width and is cut with "…"; the full name is on hover. The demo data keeps two long player names, 8 players, and the rows that only some factions have (faction technologies). Check every screen against them.

Checklist for a new screen or a changed screen:

- Where does the player make the choice? Are the controls there?
- What must the player see to decide? Is all of it on this screen?
- Does every state have a sign or a word, and only one signal?
- Does it fit at 1920×1080 with 8 players and the long names?
- On a phone: is nothing cut at the side, and is the main button in view?
- Does History show the same screen read-only?

## Screen size

There are two layouts, and no others.

- **Desktop** is the design target: a viewport of 1920×1080 or larger. Every rule in this file is written for it.
- **Phone** is a portrait phone, checked on a Pixel 9 (412×923, the installed app with no browser bars). The owner added it on 2026-10-08. It shows the same views with the same blocks; only the shell differs.
- Tablets, landscape phones and narrow desktop windows are a **non-goal**. They get whichever of the two layouts their width selects.
- There is one breakpoint: `phone:` in `src/styles/theme.css`, and `usePhone()` in `src/ui/usePhone.ts` with the same width. Do not add another breakpoint or a container query.
- `touch:` (a coarse pointer) is only for the size of a control under a finger. It does not change the layout.
- Desktop screenshots and layout tests use 1920×1080 (`e2e/shots.spec.ts`). Phone tests use the `phone` project (`e2e/phone.spec.ts`).

### Phone

- The shell shows one pane at a time: **Map · Players · Action**, with a tab bar at the bottom. The toolbar stays on top.
- The footer of the action (note, payment line, main button) is under every pane, so the main button is always in reach. This is how pattern 2 holds on a phone: the choice and the button that sends it are on one screen, the rest is one tap away.
- Exception (owner, 2026-10-10): while the fleet of a system is open on the map, the footer is not shown. The sheet has its room. The "✓" of the sheet closes it, and the footer is back. The landings of an invasion are such a sheet too.
- The pane follows the choice (pattern 1): a choice on the board opens Map, a choice of a player opens Players, everything else opens Action. The player can switch at any time. An undo opens the pane of the choice that is open again, also when it is of the same kind as the one before.
- A pane may scroll down. Pattern 9 and "the open step must not need vertical scroll" are desktop rules. Nothing may be cut at the side: `e2e/phone.spec.ts` checks every example for this.
- The player table keeps the names in view and scrolls the values sideways. A tap on a row opens the sheet of the player; there is no hover sheet.
- A phone has no hover and no keys. A shortcut key is not shown. A name that desktop cuts with "…" takes a second line where the full name is only on hover (the technology tree, the action picker).
- The map zooms with two fingers; the "+" and "−" buttons are not shown under a finger. "Fit task" and "Fit board" stay, in the row of the map views.
- Exception to "no short variants of labels" (owner, 2026-10-10): the map views and the two fit buttons are icons on a phone, so that they are one row. The name is the tooltip and the accessible name. Desktop keeps the words.
- The reference sheets, the draft menu and (in a local game) Dev are one menu in the toolbar. A sheet covers the pane.
- The app can be installed (`public/manifest.webmanifest`): fullscreen, locked to portrait. There is no service worker: the game is live, and the CDN already caused stale bundles once. The icons come from `scripts/make-icons.mjs`.
- A new screen is checked in both layouts. Short variants of labels are still not allowed; restructure the row instead.

## Action panel

- The action panel has one fixed width. It does not change with the step.
- There is no control to change the panel width.
- The board for 8 players (61 systems) must stay usable in the space that remains. Improve the board; do not make the panel narrower.

## Text

- The users are experts of the game.
- Text that states a rule or explains the UI is not visible by default. Put it behind hover/click (`Hint` in `src/ui/overlays.tsx`).
- Exception: a list where the player chooses one card (the list of action cards) shows the printed text of each card. The list has the room, and the text is what the player chooses by.
- The action picker itself keeps its rule text behind hover/click. Inline text on every row was tried (2026-10-07) and was too much.
- Visible text states the game state: costs, counts, errors, and who the game waits for.
- The open step must not need vertical scroll at 1920×1080. `e2e/shots.spec.ts` checks this.
- Do not abbreviate labels when there is room. The player table uses full column names.
- **Rules that a player may need to look up are in the UI, behind hover or click** (owner, 2026-10-11). These are: every text that is printed only on a component and is not in the rule book (a unit, a card, a technology, a leader, a faction ability), and special abilities that are not used often. General rules are not in the UI: the flow of the game, and the rules of every game such as combat dice or the unit keywords (SUSTAIN DAMAGE, BOMBARDMENT). A keyword is shown with its values and no explanation. These are examples, not a list: decide by "would an expert look this up?".
- Rule text is never written in web2. It comes from `ti4-content` through the engine (a fact of the update, or the catalogue).

## Action picker and shortcuts

- There is no separate turn bar. The action picker is the content of the action panel when the viewer has the turn and no action is open.
- The picker has fixed groups in a fixed order: Tactical, Strategic, Component, Trade, Pass. A row that cannot be used stays in its place and shows the reason.
- The picker is one list. The whole row is the control: key, group, name, and the state at the right edge. Rows have no buttons of their own.
- A choice in the picker sends nothing. A tactical action opens as the private draft. A strategic or component action is staged. The main button of the footer sends it.
- After the viewer's action, the panel shows what uses no action, and the footer has "End turn".
- A shortcut key is data: `ActionButtonView.key`. The button shows the key. Enter is the main button of the footer. Esc goes one level back.
- The picker is static: it has the same rows in every turn of a game. Only the number of strategy cards changes it, with the player count. Action cards have one row ("Play an action card"), other components have one row. A second list asks which one, with the keys 1, 2, 3.
- A strategy card has its number as key.
- Browser back is not used for game choices. Esc goes one level back; a draft has undo and redo. Browser back was tried for the picker (2026-10-07) and removed: it could not apply to the other screens.
- Shortcuts do not act in a text field or while a dialog, a menu or a rule text is open. A control that the player reached with Tab keeps Enter.

## Decisions

A decision is any choice that a card, an ability or a rule asks for. Every decision uses the same parts.

- A decision appears where its cause is: as blocks in the open action. The panel does not switch to another screen, and there is no dialog.
- The source is on top: the card, ability or rule that asks, as printed (the `source` block). The heading says what the player must do.
- There are three shapes of choice. The thing that is chosen decides the shape:
  - on the board, when it is on the board (a system, a planet, a place). The panel shows the choice and the limits, not a list of candidates;
  - in the player table, when it is a player. The table already shows the values of each player;
  - in a list, for everything else (answers, rewards, cards). Rows are the controls, with the keys 1, 2, 3 and the effect in game values at the right edge.
- An amount is a counter row. A limit is a gauge above the rows.
- A choice is staged, then sent. A staged choice shows "Selected" and "Not sent". The main button of the footer sends it. Esc clears the choice first, then leaves the action.
- A thing that cannot be chosen stays in view and shows the reason.
- Draft and History use the same blocks.

### Reaction windows

- A reaction window is a block on top of the open action (`interrupt` in the panel frame). The open action and the board show what happened; the block states the trigger in game values.
- The block lists the cards that the viewer can play now, with the printed text and the keys 1, 2, 3. It is the same block inside and outside a battle.
- The main button of the footer is Pass. A card row stages that card; the main button then plays it, and Esc clears it. Enter always does what the main button says.
- "Never offer" is a setting of a card. It is in the Cards reference sheet; the reaction window has a button that opens the sheet. A window that does not open because of the setting leaves a toast.

### Action cards

The owner asked for this on 2026-10-11: the Cards sheet has the hand of the viewer and the whole deck.

- "Your action cards" has the count against the hand limit ("Hand 5 / 7"; over the limit it is red with "⚠") and one card for each printed name, in the frame of the cards of a player sheet: the name, "×2" for copies, when it is played ("ACTION:", a timing) and the text.
- A card that a window offers has its setting at the right of the name: "Offered" or "Never offered". A card with "ACTION:" has none: no window offers it.
- "Action card deck" says how many cards are left and how many are discarded, then the cards in groups by when a card is played ("Action", "Your activation", "Combat", "Agenda phase"), each with the number of its names. A group is closed until the player opens it (owner, 2026-10-11), with "▸" and "▾" as the rounds of the log; there are too many cards for one list. The groups and their order come from the catalogue (`action_card_groups`, written by hand in `ti4-content`); the hand has the same order, with no headings. A group has one row for all cards of a name, as the exploration decks: "3 of 4 unseen" (not discarded and not in the hand of the viewer, so in the deck or in the hand of another player), "1 in your hand · 1 discarded", and the printed text. A card with none unseen is dimmed; "0 of 1 unseen" is its sign, with no "–" before it (owner, 2026-10-11).
- Exception to "rule text is behind hover": the sheet shows the printed text of each card, as the deck sheet of the exploration does.
- "Fake" is on the row of a card that the viewer does not hold and that can still be in a hand. A card with "ACTION:" has none (owner, 2026-10-11): no window offers it, so there is nothing to fake. A faked card is shown after the hand, with a dashed frame and the word "Faked", and "Stop faking". It is a setting of the page and is sent nowhere yet: later it gives the viewer the windows of that card.
- The facts come from the engine: the hand (`held_action_cards`), the deck count, the discard pile and the hand limit (`action_cards` of the update). The printed cards and the deck that a game begins with are the catalogue (`components.action_cards`). Copies of a card have ids of their own; the sheet counts by printed name.
- In local play the page answers a window that offers only Never cards with its decline (`filterOffers` in `src/session/select/cards.ts`, the rule of `ti4_engine::reaction_modes`). It is an answer like any other, so the saved game has it. An undo takes it back together with the answer of the player before it. A window with another card is asked without the Never cards. The Never cards are kept with the saved game (`cardsKey`).

### Secret objectives

The owner asked for this on 2026-10-11: the Secrets sheet has the secret objectives of the viewer and the whole deck. It is a reference sheet of its own, after Cards.

- "Your secret objectives" has "1 scored · 3 / 3" (scored and unscored together against the limit) and one card for each objective, in the frame of the cards of a player sheet: the name, the phase in which it is scored and the text. The state is at the right of the name: "✓ Scored" or "Not scored". The hand is first, then the scored objectives.
- "Secret objective deck" says how many objectives are unseen and how many cards the deck holds ("31 of 40 unseen · 22 in the deck"), then the objectives in groups by what an objective asks for ("Planets you control", "Ships on the board", "Win a combat"). A group is closed until the player opens it, as a group of the action card deck; its heading says how many of it are unseen ("4 of 6 unseen"). The groups and their order come from the catalogue (`secret_objective_groups`, written by hand in `ti4-content`).
- Every objective is in the deck once (owner, 2026-10-11), so a row has a word and no count: "Unseen" (in the deck or in the hand of another player), "In your hand", "✓ Scored by you", or "✓ Scored by" with the seat. A hand that the game reveals (Search Warrant) is "In the hand of" with the seat. A row that is not unseen is dimmed. The row has the phase and the printed text.
- Exception to "rule text is behind hover": the sheet shows the printed text of each objective, as the Cards sheet does.
- The facts come from the engine: the hand (`held_secret_objectives`), and the deck count, what each seat has scored and the limit (`secret_objectives` of the update). The printed objectives and the deck that a game begins with are the catalogue (`components.secret_objectives`). The "Secrets" column of the player table counts the scored objectives from the same fact.
- Not there yet: how far a seat is with an objective, and the sheet for another seat sorted by that.

### Payment

- Every control of an open payment is on the board, in one bar on top of the map: how the payment stands ("Paying 4 / 6 resources", then "2 more" red, "✓ Paid" green or "⚠ 1 wasted" orange), the trade goods, "Auto-pay" and "Clear payment".
- Trade goods are not on the map. The bar has a counter for them, in steps of one, and "Pay rest": it sets the trade goods to what the planets leave, so the payment is exact. It is disabled when the trade goods cannot do that.
- Auto-pay stages the payment with the least waste, with planets before trade goods. The player can change it; nothing is sent.
- The panel has no payment block while a payment is open. The footer has one line for it, left of the note: what pays and the total ("Payment Jord 4 + Vefut 2 · 6 / 6"). It uses no vertical space.
- A recorded or planned payment is a read-only line in the content. The board has no bar for it.

## Strategy cards

All eight cards use one frame: one list of seats. There are no tabs, no seat column and no trail.

- The list has the primary ("P") and then the secondaries in seat order. Each seat is one full-width line: order, symbol, name, state, and what the seat did. All decisions made so far are on one screen, with no click.
- A state has a sign: "▶ Deciding", "· Waits for Blair", "✎ Private draft", "✎ Draft ready", "✓ Resolved", "✓ Followed", "– Passed".
- Only the row of the viewer opens: into the editor, in place, while the viewer has a choice or an open draft there. After it resolves the row closes and shows its result like any other row. A ready draft is closed and says what it will do.
- Other rows are not controls. A choice of another player is visible when it is resolved, never before.
- The same list, with no open row, is the read-only view of a strategy card in the event log.
- The list is dense (24px rows) so that the largest editor, the technology tree with a payment, fits with all eight seats.

- The whole primary is one screen. Every part is staged; "Resolve primary" sends all of it. It is split only where the game reveals hidden information (Politics: the agenda cards are a second screen, sent again).
- A part of a card is a block that uses one of the decision shapes. The board and the player table serve the first part that is not complete.
- A chosen thing is a row with "Change". "Change" clears that part, so it is the open part again. Esc clears the last choice, then leaves the action.
- A part that needs no choice is a value: "Trade goods 2 → 5", "Victory points 6 → 7".
- A secondary is the same editor as its part of the primary, in the row of the viewer: the pill of the heading says "Private draft" or "Your decision". It can be drafted before the seat is reached. It states what it costs: "Strategy pool 2 → 1", or who gave it for free.
- A secondary with a choice: nothing chosen is a pass, and the main button says so. A secondary without a choice (Politics, Trade, Imperial) is a list with "Follow" and "Pass", keys 1 and 2.
- A command pool is a row of triangles, one for each token, with the count ("3 → 5") and a counter. The fleet pool points up, the tactic and strategy pools point down, as on the command sheet. A token that stays is filled, an added token has "+", a removed token is an empty outline with "−".
- Exception to "the source is on top": the printed text of a strategy card is behind hover. There are eight cards and every player knows them.
- Several players are chosen in the player table too (Trade). A row shows what the choice gives that player ("+6 commodities").
- The technology tree is a block in the panel, not a dialog: four colour columns, a row for the unit upgrades, and a row for the faction technologies when the faction has any that are not unit upgrades. A faction technology shows its colour as a square mark before the name. A cell is the control. A filled symbol is a prerequisite that the player has, a hollow one is missing. The printed text of a technology is behind "?".
- Exception to "a chosen thing is a row with Change": in the technology tree the selected cell is the chosen thing and the control. The header of the tree names what is chosen and its price.
- Exception to "keys 1, 2, 3": the cells of the technology tree have no keys. There are too many. Enter and Esc act as everywhere.

## Board

- Payment and planet choices are made on the map only, and the controls of a payment are there too. The footer of the panel names what pays.
- Planets that the open task can use must stand out. The other board content is dimmed during the task.
- The activation has three levels (owner, 2026-10-10). A system stands out when the action does something there: ships of the player can move to it, the player has a production value there, or the player's SPACE CANNON fires at the ships of another player there. A system that cannot be activated (the player's command token) is dimmed. Every other system is neither: a click still chooses it. The facts come from the engine (`BoardTaskView.highlight`).
- System names are flavor. The map does not show them. The inspector shows the name when the player clicks a system.
- A system can have up to three planets. The tile layout must support this.
- A planet that can pay shows resources and influence (`res/inf`). The value that pays is large. The player must see what the payment gives up.
- The inner ring of a system shows who has ships there, in the colour of that seat. A red dashed ring shows that two or more players have ships there.
- Seat symbols are drawn shapes of one size (`SeatShape` in `src/game/context.tsx`). Do not use font glyphs for them: their sizes are not equal.

- The board has no log line and no log button. The log is a reference sheet in the toolbar.
- The map view "Exploration" shows the type of each planet as a letter in the colour of the type (C, H, I), "?" while the planet is still explored when a player takes it, and "+" when something is attached. It is not a control. A planet of the open choice keeps its `res/inf` in every map view.
- Every map view shows the board after the staged movement: the ships and ground forces that stay, and the fleet that arrives. The player must see what a move leaves behind.

## Exploration

The owner chose on 2026-10-10 where it shows: in the landing sheet and the system inspector, in a map view of its own, and the decks as a reference sheet.

- Every screen that lists a planet has the same line for it (`PlanetTraits`): the type as icon and word, "Unexplored" while a player who takes the planet explores it, and each attachment with its name and what it changes ("Dyson Sphere +2 / +1"). The printed text of an attachment is behind "?". The landing sheet and the system inspector have it.
- The type is the control of its deck: it opens the reference sheet "Exploration" at that deck. The sheet is in the toolbar too, as the other reference sheets.
- The sheet has the four decks. A deck says how many cards are left and who has its relic fragments. A card is one row for all cards of that name: the printed text, "left / total", and who drew one, when and where. A card with none left is dimmed; "0 / 1" is its sign, with no "–" before it (owner, 2026-10-11).
- Exception to "rule text is behind hover": the deck sheet shows the printed text of each card, as a list of cards does. The reason: the player opens the sheet to read what the deck can still give.
- `res/inf` is the printed value of a planet. The attachment says what it adds, next to it.
- A decision that an exploration card asks has the card on top, as printed (the `source` block), with the deck and the planet.
- The facts come from the engine: which planets are explored, what the decks hold, what was drawn (`exploration` of the update). The names and texts are the catalogue of the engine (`Transport.catalog`); without it a card is shown by its id.
- The relic fragments and the relics of a player are in the sheet of that player.

## Player sheet

The owner asked for this on 2026-10-11: the sheet of a player shows what that player can use and what is used.

- The sheet has three groups of cards, as printed: "Faction abilities", "Leaders" (agent, commander, hero) and "Action components". A card has its name, its kind, when it is used ("ACTION:", a timing) and its text.
- "Action components" are the cards that a player uses: a technology with an ACTION or one that exhausts, the relics, an exploration card that stays in the play area, and the ability card of a legendary planet. A technology that always holds is not a card. Every technology stays in the list "Technologies" of the sheet.
- The state of a card is one word at its right edge: "Ready", "Exhausted", "Locked", "Purged". A card that cannot be used now is dimmed. A locked leader says how it is unlocked ("Unlock: Have 3 scored objectives."). A card with no state (a faction ability, a relic that always holds) has no word.
- Exception to "rule text is behind hover": the sheet shows the printed text of each card, as the deck sheet does. The reason: the player opens the sheet to read what a seat can do.
- The hover sheet of the player table has the names and the states only, with no text.
- The sheet has public information only. The action cards of the viewer are in the Cards sheet, the secret objectives in the Secrets sheet.
- The facts come from the engine (`leaders`, `exhausted_technologies`, `exhausted_relics`, `exhausted_legendary` of the player). The names, the texts and which cards are used are the catalogue (`Transport.catalog`, `components`); without it a card is shown by its id.
- A faction ability has no state yet: the engine does not say when one was used.

## Movement

The owner chose this design on 2026-10-10, from three that were compared.

- The movement is staged on the map. A system that ships can leave opens its fleet there, in a sheet: one token for each ship. A tap moves a ship or takes it back. The panel says what leaves each system; it has no controls.
- A ship that moves is one row: its token, then its own hold, one slot for each unit. The game rolls for each ship at a gravity rift, so the player must see what each ship carries. Ships of a kind that stay are tokens in one row.
- What the ships of a system can load is one row of the sheet, a chip for each kind of unit with the count that is left. The chips are grouped by place, and the place is text before its chips ("Space area", "Planet Jord", "#23 Planet Vefut"). One chip is chosen (▸). A tap on an empty slot loads that kind; a tap on a used slot unloads. "Fill" loads the chosen kind on every ship that leaves the system, until the holds are full or none is left. A chip is the icon of the unit and the count; the name is its tooltip. The heading and this row stay in view while the ships scroll.
- When the ships of a system can load at more than one place, the row of a ship says in text where its cargo is from ("Planet Jord 2 · Space area 1").
- A pickup on the way has one sign, the bent arrow: on its chip, after the number of the system, and on the slot that holds the unit. A slot of a ship that does not pass the system cannot take it.
- Gravity Drive is a toggle on the row of a ship, where it changes what the ship does: it spares the ship a gravity rift, or the ship cannot move without it. One ship of the action has it; the toggle of another ship names that ship. The game chooses the path, and the row shows what the path means ("⚄ Rift roll").
- A system that the player is done with has a check mark: the movement takes something from it, or the player marked it. The "✓" of the sheet closes the sheet and marks the system; the sheet has no "×". A tap on the system closes the sheet with no mark. The mark of the player is kept in the page only, until the movement is sent: a reload loses it. The panel says "✓ Nothing leaves" for a marked system with nothing staged. The count of the player's ships shows "now → after" in the standard view.
- The paths of the ships are drawn for the open system only, with the marks of that path (rift roll, Gravity Drive). The game chooses the path. The map shows no cargo labels.
- Exception to "a choice is staged, then sent": a draft records every change of the movement at once, and the later steps are checked again. The same holds for the landings and the production of a draft. The step has no "Reset selection". Its main button sends nothing: it records a step with nothing staged ("Move nothing", "Land nothing", "Produce nothing"), or goes on to the step that the game comes to ("Go to invasion", "Go to production"). When no step follows, there is no button. The reason: a draft is private, every change is one step of undo, and the step tabs go on. Live keeps "Move fleet", because that sends to the game.
- The active system opens the list of what is committed to it: the ships that were there, then the ships that move, by the system that they leave, each with its hold. It is read-only; a system in the list opens its sheet.
- The demo data follows the game: before a movement, ships of two players are never in one system. Ground forces of two players can be on different planets of one system.
- The sheet has a small "Reset": it takes back what leaves that system, and its check mark, also one that the player set.
- The read-only view of a movement (a recorded step in Live, History) is the same tokens in the panel, one band for each system.

## Invasion

The owner chose this design on 2026-10-10, from three that were compared.

- The landings are staged on the map, in one sheet at the active system. It opens when the step opens. The panel says what lands on each planet and what stays in space; it has no controls. A row of it opens the sheet.
- The ground forces that can land are one row of the sheet, the same chips as the load row of the movement: a chip for each kind with the count that is left, grouped by place, one chip chosen (▸).
- Each planet of the system is one row: its name, `res/inf`, its type and what the exploration left there (the Exploration section), who controls it, the units that are there by seat, then one token for each unit that lands and one free slot ("+"). A tap on the slot lands the chosen kind; a tap on a token takes it back. A planet where nothing can land stays in its place and says so.
- A tap on a planet of the active system on the map lands one more unit of the chosen kind there. The planets that can be landed on stand out, as in a payment, and a planet with a staged landing has the check mark.
- Every map view shows the ground forces of a planet after the staged landings.
- "Reset" of the sheet takes every staged landing back. The "✓" closes the sheet and sets no mark: there is one system. A tap on the active system opens it again. On a phone the open sheet has the room of the footer, as the fleet of a system has.
- The footer has "Commit ground forces", or "Land nothing" when nothing is staged, and "Reset selection".
- What follows the landings (space cannon defense, ground combat, control, exploration) is not on this screen yet: these are decisions in the list. A decision of an exploration card names the card.

## Production

The owner chose this design on 2026-10-11: the build first, then the payment, both on the map.

- The step has two stages. Nothing is sent before the last button.
- **Build.** The units are staged on the map, in one sheet at the system that produces. It opens when the step opens. The panel says what is produced in each place and has the limits; it has no controls. A row of it opens the sheet.
- Every kind of unit of a production is one row (owner, 2026-10-11; a row of chips with one chosen kind and a row with all printed values were tried, and were hard to read): the icon, the name, the cost ("2", "1 for 2"), how many are left in the supply, the units that are staged, and the tokens. The columns have fixed widths, so cost and left line up over the rows; each group heading says "Cost" and "Left" once.
- The printed values are behind the name (`Peek`): hover shows them, a click or a tap keeps them open, and the card opens above the name when there is no room below. The name has a dotted underline on both layouts. The card is laid out as the printed unit card: the icon and the name, the four values in one row (cost, combat, move, capacity; the number over its word), the keywords with their values, one line each in capitals, and the printed special rule of the unit (`rules`). A section with nothing is not shown. The card has no control, and a keyword is not explained (the Text section).
- A dice value is read as the game says it: "5+", and "3+ (x3)" with more than one die; the same for BOMBARDMENT, ANTI-FIGHTER BARRAGE and SPACE CANNON. PRODUCTION has the printed value ("PRODUCTION +2").
- The rows are in two groups. "Ships · Space area" says the place once. "Ground forces" says its place in the heading when there is one place only ("Planet Jord"); with several places each unit has one line for each ("Planet Lisis"), under a row with the total.
- What is staged is the count of units (zero is dimmed, more than zero in the accent colour), then one token for each purchase, with the icon of every unit that it makes: a pair of fighters is a token with two icons. A tap on a token takes that purchase back. The dashed slot adds one purchase and shows the units that it makes. It is not shown when nothing more can be added. One step is one purchase. Fighters and infantry come two at a time, as the game sells them; the last pair makes one unit when the production limit has room for one only.
- A unit that cannot be produced in this use keeps its row. It is dimmed and has a word in place of the tokens ("No technology", "None left", "Blockaded", "No place"), with the reason in full as tooltip.
- The limits are gauges of the sheet and of the panel: "Production capacity", "Resources", "Fleet supply after", "Fighters and cargo in space". The heading of the sheet says what is staged and what it costs after a discount ("5 units · 3 resources · discount 1").
- The limits are soft, except the supply of a unit (the plastic): the player plans against them. A build over a soft limit is staged. The gauge is red, the footer says what is over ("2 resources short · 1 unit over the production limit"), and the main button and the "✓" of the sheet wait (pattern 5). The soft limits are the resources (the cost of the staged build against what the ready planets and the trade goods of the seat pay), the production limit, the fleet supply and the capacity. A purchase after the production limit makes all its units. The supply of a unit is hard: "Left" counts down, and when none is left the slot is not shown.
- "Fighters and cargo in space" has both sides after the build: a staged fighter uses capacity, and a staged ship adds its own.
- No planet pays while the build is chosen, and there is no payment bar. The main button says what is next: "Pay 3 resources", "Produce units" when the build costs nothing, "Produce nothing" when nothing is staged. The "✓" of the sheet does what "Pay …" does. "Reset" takes the build back.
- **Pay.** The sheet closes and the map is the payment (the Payment section): the bar, the planets that pay, Auto-pay. The footer has the payment line and "Produce units", which is disabled while the payment is short. "Back to build" (Esc) or a tap on the system that produces opens the sheet again; the staged payment stays and is measured against the new cost.
- Exception to "a tap on a planet of the active system acts on the open sheet" (the Invasion section): in a production a planet always pays. The reason: every ready planet pays, also the planets of the system that produces.
- On a phone the open sheet has the room of the footer, as the fleet of a system has. The payment has the whole map pane, with the footer under it.
- Every map view shows the ships and the ground forces of the system after the staged build.
- What the game asks before the production (AI Development Algorithm) is a decision in the list.

## Battle table

- The headers of the attacker and the defender have the same lines, so the unit rows start at the same height.
- The simulated odds are in these headers (win % and survivors). They have no card of their own.
- A player can have more than one strategy card (a game of four or fewer players). The player table shows all of them.

## Tests

- `npm test` runs two kinds of test (the projects in `vite.config.ts`). A `*.test.ts` has no DOM and runs in node: the view models, the drafts, the real engine. A `*.test.tsx` renders components in jsdom with `@testing-library/react`.
- A component with logic of its own (open and close, keys, a place on the screen) gets a component test next to it (`src/ui/overlays.test.tsx`). jsdom has no layout: a test of a position gives the rectangle of the trigger.
- A screen is tested over an update that the engine recorded (`src/session/fixtures/`), not over data written by hand, so the test sees what the engine sends. Such a test imports the session and the components, so it is in `src/dev/` (`src/dev/productionSheet.test.tsx`; the layers are in `src/boundaries.test.ts`). A fixture is recorded again with its command when the facts change; it is not edited.
- The page takes its facts from `src/session/ti4.wasm`, which is not in the repository. After a change of a fact in `ti4-view`, build it again (`scripts/build-wasm.sh`); `wasmTransport.test.ts` checks the shape that the screens read.
- The tests of a local game (`e2e/local.spec.ts`) collect the errors of the page and expect none. A new part of a screen that opens (a card, a sheet, a menu) is opened once there.

## Rust builds

- Work in `web2/` uses six crates: `ti4-model`, `ti4-content`, `ti4-engine`, `ti4-view`, `ti4-server`, `ti4-wasm`. Check them with `cargo c-web` and test them with `cargo t-web` (the aliases are in `.cargo/config.toml`).
- Do not build or test another crate for this work, and do not use `--workspace`. The other crates (`ti4-policy`, `ti4-sim`, `ti4-training`, the libtorch crates, and the rest) are slow to test and fill the target directory. They are fixed later.
- A fact that the UI needs is added in `ti4-view`, not in `ti4-engine`, when that takes little code: a change of the engine can break replays. Change the engine when the fact obviously belongs there, or when `ti4-view` would need much code or would repeat a rule. Such a change must not alter what the game does (owner, 2026-10-10).
- Before you commit Rust changes, all three must be clean: `cargo fmt --all -- --check`, `cargo l-web` (Clippy, no warnings), and `cargo t-web`.
- The server integration tests are one binary. Run one file with `cargo test -p ti4-server --test it <file name>::`.
- Temporary (2026-10-09): update this section when the other crates are fixed.

## Local play

- `?local=<seed>&players=8&humans=<mask>` plays a real game of the engine in the page (`src/dev/LocalApp.tsx`). Build the engine first: `scripts/build-wasm.sh`.
- The bare address `/` is that game with `DEFAULT_GAME` (seed 42, 8 seats, seat A played here), so the installed app opens it. `?example=…` is the demo and `?gallery` the gallery; they get no local game.
- If the engine does not load (missing file, or no JSPI), the demo opens with the reason in a line above the shell.
- The header has a link to the other app: "Demo" in a local game, "Local game" in the demo. On desktop it is a button after the reference sheets; on a phone it is the last item of "Reference and more". A page load, not a game choice.
- A local game has no bar at the bottom. "Dev" (a header button on desktop; the item after the other-app item in "Reference and more" on a phone) opens the Dev sheet, which is a reference sheet: the seed, the seats, the saved answers, the other seats, "Play as", "Start from", New game, Export and Import. It was "Settings" until 2026-10-10: everything in it is for testing. Without the shell (loading, a saved game of another engine, a notice) the page has a row with only "Dev". The demo has its own bar and no Dev sheet.
- "Start from" is one list: "Round 1" and the scenarios of the engine (`crates/ti4-view/src/scenario.rs`). A row is the control and loads the page at the address of that game (`?scenario=<id>&humans=<mask>`); what a scenario holds is behind "?". A scenario is a game of its own: the engine sets up its board on a fixed seed, and its answers are saved under its own key, so a reload, an undo and New game work as in any game. A new case for testing is added to a scenario in Rust, with a test that the facts show it. A new feature gets its cases in the late-game scenarios when it can (owner, 2026-10-11), so that it is seen in a full game and not in round 1 only: every state that its screen shows, at every seat. Example: for the Secrets sheet every seat has three secret objectives, some scored, some that are scored in the action phase and some of the other phases.
- `src/session/` turns an update of the game into the view models. It imports `model` only, and it decides no rule: a value that needs a rule comes from the engine.
- A decision without a dedicated screen is one list in the action panel.
- The activation and the movement of a tactical action use the screens of these steps. In the movement the systems that ships can leave are the choice on the board, as in the demo, so a phone stays on the map. The facts come from the engine with the update (`tactical`), and "Move fleet" sends the staged movement as one plan (`Transport.submitPlan`). A plan that the game refuses is taken back whole; one undo takes a movement back whole, and it is staged again as it was sent, so the player can change a part of it. The check marks that the player set by hand are not kept.
- The landings of an invasion use the screen of that step too (the Invasion section). The facts are the third kind of `tactical`, and "Commit ground forces" sends the staged landings as one plan, with the rules of a movement plan: refused whole, stopped by another decision with the rest staged again, and taken back whole by one undo. The staged landings are kept for a reload as the staged movement is. After the last unit the game does not ask for the end of the landings, so the plan needs no answer for it.
- The production uses the screen of that step too (the Production section). The facts are the fourth kind of `tactical` (`ProductionFacts`): every unit of a production with its price, its places, its supply and its printed values, and why the seat cannot produce it when it cannot, the limits, the discount, and the planets and trade goods that pay. "Produce units" sends the build and its payment as one plan, with the rules of a movement plan. The game asks for a payment after each purchase; the plan answers with the staged payments in their order, whenever it is asked, and leaves out what the game did not need. A build whose payment does not cover it is refused whole. The staged production is kept for a reload.
- The limits of a staged production are sums over the facts: the fleet pool and the capacity do not count what a rule excuses (fighters at a space dock, Fighter II). The game has the last word when the plan is sent.
- The Draft tab opens the private draft of a tactical action: its activation, its movement, its landings and its production, on the screens of these steps (pattern 10). A draft is a script (`src/session/draft.ts`): the tactical action, its system, then the steps of its movement, of its landings and of its production. The engine plays the script as the next turn of the seat on a copy of the game, each time the draft or the game changes (`Transport.runDraft`), and the screens are made from where that run ended. Nothing of it is in the game or in its saved answers.
- The draft opens whenever the page waits in the strategy phase or the action phase and the seat has not passed; the tab says why when it does not. In the strategy phase it is the draft of the turn that comes (owner, 2026-10-10). The action menu stays a list: a tactical action chosen there is played live.
- In the draft, "Activate system" records the system, and every change of the movement is recorded at once (the Movement section). Undo and Redo of the toolbar go through the states of the draft; "Start over" and "Discard draft" are in its menu. The draft and its history are kept in `localStorage` (`planKey`), so a reload keeps them.
- "Apply to Live" is enabled at the turn menu of the seat, when the game played the whole draft. Its dialog says what is sent. It sends the action, the system and the movement as one request (`Transport.applyDraft`): refused whole, or stopped by another decision, as a movement plan is. The draft is then gone.
- A draft that the game no longer plays (the system got a command token, a ship is gone) keeps its script: the screen says which part does not fit, and Apply is disabled.
- The draft goes on as far as the game is certain: from the movement to the landings and to the production. A step that is not drafted is decided in Live. A step in view stays in view while it is changed; its main button goes on. The tabs open every step that the game came to with the draft. A change of an earlier step keeps the later steps, and the game checks them again.
- The draft ends where something random lies ahead: a roll (a gravity rift, space cannon, a combat), a card that is drawn (a landing on a planet that nobody explored), ships of another seat in the system. The footer says so. This ends the check of the draft, not the draft: Apply stays possible, and the game goes on in Live. Apply sends the steps that the game played; what the draft holds after that point is staged again in Live when its step opens, and nothing of it is sent.
- A production of a draft is in the draft when it is paid. Until then the build is kept, the footer says so, and the game is not asked for it.
- What an ability of the seat asks before a step (AI Development Algorithm) is a list in the draft too. Its main button is "Record": the answer is a part of the draft.
- The game asks "negotiate, or continue" between the steps of a tactical action when the seat has a neighbour. A plan that still has steps continues; so does a draft.
- The Dev sheet has "Other seats": Run (the default) or Step. With Step the game waits before each decision of a seat that is not played here; the panel names the seat, and the main button "Step" lets it decide. The seats decide the same either way, so the saved game does not change. The setting is of the page (`STEPPING_KEY`), for testing: a draft can be opened while another seat has the turn. A reload plays the saved answers again and then waits at the first step after them.
- The Dev sheet has "Play as": one row for each seat, with "✓ Playing" or "Random". A row of a random seat is the control: that seat is played here from now on, and the seat that the page was shown as decides at random, from its open decision on (`Transport.switchSeat`). With a mask of several seats only the shown seat is given up. The staged movement and the private draft of the seat that is left are dropped. Undo does not go back before the change: to play the old seat again, choose it (owner, 2026-10-10). The changes are part of the saved game (`switches`), so a reload, Export and Import keep them.
- In local play the game decides when a ship uses Gravity Drive: only when the ship cannot arrive without it. The toggle is then on and locked. The free toggle of the Movement section is not offered there.
- The game is saved in `localStorage` as its seed and the ids of the answers (`src/session/savedGame.ts`). A reload and an undo play it again from the seed.
- The staged movement of the open movement step is saved too (`draftKey`), with the number of its choice. After a reload it is staged again, so its systems keep their check marks.
- The details, the measurements and the limits are in `crates/ti4-wasm/README.md`.
