# Card usage audit

How each card of the corpus is used: which JSON fields the engine, host, bots, wasm API and web
actually read, how each card's effect is implemented, where gameplay depends on parsing text, and
what a typed, id-keyed card model with a separate display layer would look like.

Branch `card-usage-audit` (from `engine-optimization-study` at `799194e7`), 2026-10-26. Docs and
tools only: no engine behaviour changed.

Machine-readable output: `docs/card_audit/` (`cards.csv` one row per card, `fields.csv` one row per
category x field, `id_refs.json` ids in both directions, `registry_dump.json` the engine's own
registries). Tool: `tools/card_audit/`. Browsable table: private artifact https://claude.ai/artifact/3ipo9QWjthgoqdrpsSRpBU (built by `tools/card_audit/build_page.py`).

## Summary

- **802 card records in 15 categories, 153 distinct (category, field) pairs, 311.9 KB of minified
  JSON** (46% of the 684.6 KB embedded corpus; planets, systems, units and map templates are the
  rest and are out of scope here).
- **Coverage by the engine's own registries is complete.** Every action card has an effect and a
  bound reaction window, every agenda, relic, secret, public objective, ability, breakthrough and
  exploration card is registered (measured: `registry_dump.json`, built by
  `crates/ti4-engine/examples/card_registry_dump.rs`). The gaps are elsewhere: four registered
  cards that do nothing or almost nothing in play (Galactic Crisis Pact, Classified Document Leaks,
  Covert Legislation, the Demilitarized Zone exploration), 20 galactic events with no code at all,
  and two Vetos too many in the dealt action deck.
- **Where the rules read card data** (beyond the id and the source tag), it is a small set of
  fields: action-card `window`; agenda `type`, `target`; note `playArea`, `faction`; relic
  `isFakeRelic`; leader `faction`, `type`, `homebrewReplacesID`, `abilityWindow`; breakthrough
  `faction`, `synergy`; objective `points` (+ secret `phase`); technology `types`,
  `requirements`, `faction`, `baseUpgrade` (+ one substring of `text`); exploration `type`,
  `resolution`, `attachmentId`; six attachment modifiers; strategy card `initiative` and `name`.
  As typed data that is about **50 KB of JSON (6.8 KB gzip)** for all 802 cards.
- **Five of those are free text that is parsed**: action-card `window` (exact sentence -> event),
  agenda `target` (substring search for "Player", "Planet", ...), leader `abilityWindow`
  (`starts_with("ACTION")`), technology `text` (`contains("cannot be researched")`) and strategy
  card `name` (`match name { "Leadership" => ... }`). Each is a drift risk and each maps onto a small
  enum.
- **Display text is ~250 KB of the 312 KB**: 66 KB read only by the host catalog / web, 71 KB that
  the *engine* reads to label choices and offer cards (62 "Display only" sites), 95 KB that nothing
  reads (flavour text, upstream notes, image URLs, Twilight's Fall and Wild Wild Galaxy printings),
  plus 18 KB of technology text read for one substring. Splitting it off saves its bytes in every
  engine-only binary; in the wasm build it saves bytes only if the page loads the display table as a
  separate asset (see [Size](#size-what-a-split-would-save)).
- **Behaviour keyed on names**: strategy cards (all 12 dispatch on the printed name; base2 and
  pok2diplomacy share "Diplomacy"), action-card previews (`reaction_preview::PREVIEWED`, ~38 names),
  copies of an action card merged by name, and hand-copied names in the web
  (`actionCardTurns.ts`, `pausablePrompts.ts`).
- **36 ids are spelled the same in two categories** (`mr` is a technology and Mecatol Rex, `ps` a
  note and a technology, `crisis` an agenda and an action card): any id enum or interned id must
  stay per category.

## Method

What was measured (by the script, reproducible) and what was read by hand:

| step | how | measured or read |
|---|---|---|
| fields per record | every key of every record of the 15 files; bytes = `"key":value,` in minified JSON | measured |
| reader sites | regex over Rust production code for record accessors (`.text("f")`, `.int`, `.flag`, `.strings`, `.raw`, `.get`), the host helpers `text(record, "f")` / `field(record, "f")` and the closures `field("f")` / `printed("f")` / `text_of(record, "f")`; a site is given to a category by the nearest `ContentType::X` in its function (or the next 3 lines), else by the field being unique to one category; sites that stay ambiguous are listed in `id_refs.json` (`unattributed_field_sites`), never guessed | measured; 9 mis-attributed sites removed by hand (`exclude_sites` in `curation.json`, each with a reason), 6 added (`extra_sites`) |
| production vs test code | `tests/`, `benches/`, `*_tests/` dirs, `fixtures.rs`, files with `#![cfg(test)]`, and everything after the first `#[cfg(test)]` that introduces a `mod` | measured |
| web readers | the web reads the host catalog (`ti4-host/src/view/records.rs` -> `GET /api/catalog` or the wasm engine -> `catalogStore.ts`), not the corpus; the script maps each raw field to its catalog field by parsing `content_records` and counts `.field` / `.camelField` reads in non-test web code | measured, but counts for generic names (`name`, `text`, `kind`, `phase`) are upper bounds |
| implementation status | the engine's registries (`effect_for`, `registered_aliases`, `requirement_for`/`cost_of`, `unimplemented`, `factions::assets` + `implemented`, `reactions::window_for`) dumped by `card_registry_dump`; categories with no registry (technologies, attachments, strategy cards, galactic events, generic cards) from literal id references, unit-upgrade wiring in `units.json`, the active technology deck and the faction sheets | measured |
| id references | every string literal in production code (Rust and TS) that equals a card id (also an action card's `automationID` and a generic note's bare alias) | measured; ambiguous for the 36 colliding ids |
| ids in code not in content | literals passed to typed card-id constructors (`LeaderId::new("...")`) and literals in match arms / lists of the *dispatch functions* of each category's owner module (a function where some arm names a real card id); 51 of 52 hits are decision kinds, reviewed and listed in `curation.json` | measured + reviewed |
| name-keyed behaviour | literals equal to a card's printed name (not its id) in engine, bot, host and web production code | measured |
| partial implementations | `Deferred`/`Unresolved` returns and "not modelled / not implemented / TODO / simplified" comments inside the handler a card is routed to | measured, then read: they are evidence to read, not verdicts |
| objective thresholds | numbers printed in an objective's text vs numbers in its code arm (`objectives.rs`, `secrets.rs`) | measured (heuristic; all agree today, one false candidate `dyp`) |
| classification of fields, mechanism per category, issues | reading the reader sites | read; every curated claim names `file:line ~ pattern` and the script fails if the pattern is no longer within 3 lines |

Run it:

```text
# optional, refreshes the engine registries (needs a cargo build of ti4-engine, ~6 min cold):
CARGO_TARGET_DIR=/tmp/t cargo run -q -p ti4-engine --example card_registry_dump > docs/card_audit/registry_dump.json
python3 -I tools/card_audit/card_audit.py                # writes docs/card_audit/{cards,fields}.csv, id_refs.json
python3 -I tools/card_audit/card_audit.py --check        # summary only, exit 1 on stale evidence
python3 -I tools/card_audit/card_audit.py --json x.json  # everything in one file (input of the artifact page)
python3 -I tools/card_audit/build_page.py x.json page.html
```

Limits: a reader that builds the field name at runtime (none was found) or reads a whole record
(`provenance::digest_of`, the host's `every()` helper) is not attributed per field; literal counts
do not see ids composed with `format!` (Support for the Throne is `support:<faction>`, a note id
is `alias:faction`); the partial-implementation markers only cover the
owner modules, not the 48 faction modules.

### Field classes

| class | meaning |
|---|---|
| `id` | the identity key |
| `scope` | the `source` tag: parsed once at load, filters every deck and lookup |
| `gameplay` | typed data the rules read (affects the outcome) |
| `gameplay-text` | the rules outcome is decided by parsing a free-form string |
| `engine-label` | the engine reads it only to label a choice, an event payload or an offer card |
| `bot` | only bot heuristics read it (none exclusively: every field the bots read the engine reads too) |
| `ui` | only the host catalog / views and the web read it |
| `unused` | no runtime reader |

## Per category

| category | cards | status (engine registry / scan) | pure id | needs data | gameplay fields | text-parsed fields | KB: gameplay+id+scope / text-parsed / engine-label / ui / unused |
|---|---|---|---|---|---|---|---|
| action_cards | 142 | registered 142 | 0 | 142 | - | `window` | 5.2 / 6.7 / 3.3 / 18.0 / 35.5 |
| agendas | 63 | registered 63 | 0 | 63 | `type` | `target` | 3.4 / 2.1 / 17.5 / 0.0 / 1.1 |
| promissory_notes | 40 | registered 34, referenced 5, not-dealt 1 | 0 | 40 | `faction`, `playArea` | - | 2.8 / 0.0 / 0.0 / 10.7 / 0.9 |
| relics | 24 | registered 24 | 23 | 1 | `isFakeRelic` | - | 1.0 / 0.0 / 0.0 / 4.9 / 4.3 |
| leaders | 103 | registered 99, not-dealt 3, none 1 | 0 | 103 | `faction`, `homebrewReplacesID`, `type` | `abilityWindow` | 7.5 / 5.1 / 18.0 / 9.8 / 31.1 |
| breakthroughs | 31 | registered 31 | 0 | 31 | `faction`, `synergy` | - | 3.0 / 0.0 / 7.9 / 0.0 / 2.2 |
| public_objectives | 40 | registered 40 | 0 | 40 | `points` | - | 2.0 / 0.0 / 0.0 / 4.2 / 0.4 |
| secret_objectives | 40 | registered 40 | 0 | 40 | `phase`, `points` | - | 2.3 / 0.0 / 1.2 / 2.8 / 1.5 |
| technologies | 102 | registered 63, referenced 28, data-driven 7, not-dealt 4 | 0 | 102 | `baseUpgrade`, `faction`, `requirements`, `types` | `text` | 8.9 / 17.8 / 2.8 / 0.0 / 3.3 |
| abilities | 73 | registered 73 | 73 | 0 | - | - | 2.6 / 0.0 / 9.7 / 5.9 / 0.8 |
| explores | 80 | registered 80 | 0 | 80 | `attachmentId`, `resolution`, `type` | - | 5.9 / 0.0 / 10.0 / 6.4 / 0.0 |
| attachments | 22 | data-driven 14, referenced 5, unreferenced 3 | 3 | 19 | `influenceModifier`, `planetTypes`, `resourcesModifier`, `spaceCannonDieCount`, `spaceCannonHitsOn`, `techSpeciality` | - | 1.5 / 0.0 / 0.6 / 0.1 / 0.1 |
| strategy_cards | 12 | name-keyed 10, referenced 2 | 0 | 12 | `initiative` | `name` | 0.6 / 0.2 / 0.0 / 3.6 / 0.7 |
| galactic_events | 20 | unreferenced 20 | 20 | 0 | - | - | 1.0 / 0.0 / 0.0 / 0.0 / 11.4 |
| genericcards | 10 | referenced 5, unreferenced 5 | 10 | 0 | - | - | 0.5 / 0.0 / 0.0 / 0.0 / 1.4 |

"Pure id" means no field of the record other than its id and source is read by the rules; "needs
data" counts records that carry at least one gameplay field (what a typed record would still have
to hold). Status values: `registered` (an engine registry claims it), `referenced` (no registry for
the category; the id appears as a literal in engine production code), `data-driven` (behaviour
comes from the record's fields or from another file naming it: unit upgrades via
`units.json requiredTechId`), `name-keyed` (dispatched on the printed name), `not-dealt` (no deck,
faction sheet or active technology list includes it), `none` (a registry lists it as
unimplemented), `unreferenced` (nothing at all).

Per category in words, with the rows for every field after it:

- **Action cards**: one arm per physical copy in `action_cards::effect_for` (copies listed by hand;
  `automationID` exists but the engine does not use it). A reaction card plays only if its printed
  `window` sentence is a key of `reactions::window_table` (all 142 are bound today). `name` merges
  copies into one option. Typed form: `kind` (the printed card, shared by copies) + `timing`
  (enum from the window table) + nothing else. The 36 Wild Wild Galaxy printings are unused.
- **Agendas**: `type` (Law/Directive) and the elect kind from `target`; effects id-matched in
  `agenda_effects::resolve_with` and standing rules in `laws`. The engine copies `name`, `text1`,
  `text2` into agenda payloads (17.5 KB of engine-label text). Typed form:
  `kind: Law|Directive`, `elects: ForAgainst|Player|Planet{non_home}|Law|ScoredSecret|StrategyCard|Hidden`.
- **Promissory notes**: `playArea` and `faction`; effects in `promissory.rs` and faction modules.
  The five generic notes are `<color>_x` in the corpus and the bare alias in the engine
  (`promissory::GENERIC`), Support for the Throne is `support:<faction>`. `playImmediately` and
  `attachment` look like gameplay data but the code encodes those facts per note.
- **Relics**: pure id, plus `isFakeRelic` on one record for the deck.
- **Leaders**: dealt by `faction` + the faction sheet + `homebrewReplacesID`; locked/readied by
  `type`; action-ness parsed from `abilityWindow`; effects id-matched (`leaders::dispatch_leader`,
  `modifiers`, faction modules). 31 KB unused (image URLs, Twilight's Fall printings). Four records
  are never dealt (three `redcreuss*` variants and `orlandohero`).
- **Breakthroughs**: `faction` and `synergy` (two colours); effects id-matched.
- **Public objectives**: `points` decides the stage when the deck is built and the VP; predicates
  and prices are id-matched with the thresholds as Rust literals (all agree with the printed text,
  measured).
- **Secret objectives**: `phase` decides when it can be scored, `points` the VP; predicates
  id-matched (`secrets.rs`).
- **Technologies**: the research rules are data (`types`, `requirements` as a letter string,
  `faction`, `baseUpgrade`), the effects are id checks (`seat.technologies.contains(&TechnologyId::new("gd"))`,
  `has_technology(state, player, "x")`; 212 literal references to 91 technology ids in engine
  production code), unit upgrades are wired through `units.json requiredTechId` (7 generic
  upgrades have no literal at all and work through that). The researchable set comes from
  `decks.json techs_pok_c4` plus Thunder's Edge; four superseded printings are never offered.
- **Abilities**: pure id (the faction's list comes from `factions.json abilities`); the engine
  reads `name`, `window`, `windowEffect` only for offer cards.
- **Exploration cards**: deck by `type`, path by `resolution`, the attachment by `attachmentId`;
  Instant cards id-matched in `exploration::resolve_instant`; Attach cards need no handler (which is
  how `dmz` came to do nothing).
- **Attachments**: fully data-driven planet modifiers; `dmz` carries none and the Custodia Vigilia
  and Helios records are UI markers (`isFakeAttachment`).
- **Strategy cards**: dispatched on the printed `name`; `te4construction` and `te6warfare` by id;
  `initiative` from data; the dealt set from `strategy_card_sets.json`.
- **Galactic events**: no reader anywhere (not offered, not implemented).
- **Generic cards**: the five Firmament plot cards are named in `ti4_model::plots::CARDS` and
  `factions/firmament_plots.rs`; the `mutated_*` twins are unused.

### Fields, per category

#### action_cards (142 records)

Mechanism: id-matched effect (action_cards::effect_for, one arm per copy alias) + reaction window bound by the printed window sentence.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `flavorText` | 142/142 | 23647 | unused | - | no reader except for explores (see explores.flavorText) |
| `text` | 142/142 | 14316 | ui | host 3, web 161 (`text`) | host catalog text |
| `window` | 142/142 | 6717 | gameplay-text | engine 7, host 4, web 21 (`window`) | the printed timing sentence is the key of reactions::window_table (exact string, trimmed) that binds a card to an engine event; 'Action' marks a component action (is_component_action, keleres::has_component_action); content::action_cards::timing_of groups ~50 texts by hand |
| `notes` | 33/142 | 4262 | unused | - | upstream bot notes; no reader |
| `wildWildText` | 36/142 | 3408 | unused | - | Wild Wild Galaxy galactic-event printing; galactic events are not implemented |
| `name` | 142/142 | 3309 | engine-label | engine 2, host 5, web 352 (`name`) | printed name; the engine also uses it to merge copies into one option (first_of_each keys the hand by name) and to label discard choices, so it is not purely display; reaction_preview::PREVIEWED and web actionCardTurns BY_NAME key on it |
| `alias` | 142/142 | 2769 | id | engine 2 | identity; effect_for / REGISTERED_ALIASES match on it, one alias per physical copy (mb1..mb4) |
| `wildWildWindow` | 36/142 | 2572 | unused | - | as wildWildText |
| `source` | 142/142 | 2472 | scope | - | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `phase` | 142/142 | 2399 | ui | host 3, web 72 (`phase`) | host catalog phase; content::action_cards::timing_of groups by it; web offersOwnTurnForCard reads it |
| `automationID` | 45/142 | 1278 | ui | host 1, web 2 (`automation_id`) | host catalog automation_id; web indexes cards by it too. The engine does not read it: it lists every copy alias by hand |
| `affectedByWildWildGalaxy` | 36/142 | 1152 | unused | - | as wildWildText |
| `playTiming` | 16/142 | 441 | unused | - | upstream enum-like timing (AGENDA_WHEN, ...) on 16 cards; would be a better key than the window sentence but nothing reads it |

#### agendas (63 records)

Mechanism: id-matched effect (agenda_effects::resolve_with) or standing rule (laws::enforced_aliases); outcome set parsed from target; law/directive from type.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `text1` | 63/63 | 9597 | engine-label | engine 2, host 2 | For/elected text: engine copies it into AGENDA payloads (strategy_cards::agenda_details) and law prompts; host catalog/score views |
| `mapText` | 27/63 | 3051 | engine-label | engine 1 | text of an elected law, preferred over text1 in the law-discard prompt |
| `text2` | 63/63 | 2956 | engine-label | engine 1, host 2 | Against text, same paths as text1 |
| `target` | 63/63 | 2135 | gameplay-text | engine 4, host 2, bot 1 | the free-form 'Elect ...' sentence is parsed (split at '(' then substring 'Player' / 'Planet' / 'Non-Home' / 'Other Than Mecatol' / 'Law') to decide the outcome set; 'Elect Scored Secret Objective' and 'Elect Strategy Card' yield no candidates; game.rs uses starts_with("Elect Player") for Committee Formation |
| `name` | 63/63 | 1867 | engine-label | engine 3, host 1 | names in law-discard prompts and agenda details payloads |
| `alias` | 63/63 | 1469 | id | engine 2 | identity; agenda_effects::resolve_with and laws match on it |
| `source` | 63/63 | 995 | scope | - | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `type` | 63/63 | 957 | gameplay | engine 3, host 1, bot 1 | 'Law' vs 'Directive' decides whether a passed outcome stays in play (8.20) |
| `againstEmoji` | 17/63 | 438 | unused | - |  |
| `forEmoji` | 16/63 | 340 | unused | - |  |
| `notes` | 3/63 | 284 | unused | - | upstream bot notes; no reader |

#### promissory_notes (40 records)

Mechanism: id-matched (promissory.rs consts and faction modules); play area from data.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `text` | 40/40 | 9746 | ui | host 1, web 161 (`text`) | host catalog |
| `name` | 40/40 | 942 | ui | host 1, web 352 (`name`) | host catalog |
| `alias` | 40/40 | 726 | id | engine 1 | identity; generic notes are '<color>_cf' etc. and the engine names them by the bare alias (promissory::GENERIC = cf, ps, ta, an) plus the prefix; Support for the Throne is keyed 'support:<faction>' |
| `source` | 40/40 | 693 | scope | - | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `faction` | 35/40 | 671 | gameplay | engine 2, host 1, web 119 (`faction`) | owner faction of a faction note (play-area check, faction asset ledger) |
| `playArea` | 40/40 | 669 | gameplay | engine 2 | whether a received note goes face up in the play area (and so its effect is active) |
| `playImmediately` | 11/40 | 262 | unused | - | would be gameplay (note resolves on receipt); the engine encodes that per note in code |
| `notes` | 2/40 | 188 | unused | - | upstream bot notes; no reader |
| `attachment` | 7/40 | 138 | unused | - | token/attachment the note places (Terraform...); engine uses its own ids (attachments 'titanspn') |
| `shortName` | 4/40 | 132 | unused | - | no runtime reader (two training/report examples only) |
| `color` | 5/40 | 90 | unused | - |  |
| `shrinkName` | 5/40 | 90 | unused | - | upstream image layout hint |

#### relics (24 records)

Mechanism: id-matched (relics::use_relic, registered_aliases).

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `text` | 24/24 | 4282 | ui | host 2, web 161 (`text`) | host catalog |
| `flavourText` | 22/24 | 3796 | unused | host 1 | host reads flavourText only for explores |
| `name` | 24/24 | 604 | ui | host 1, web 352 (`name`) | host catalog |
| `alias` | 24/24 | 541 | id | engine 1 | identity; relics::use_relic / registered_aliases match on it |
| `source` | 24/24 | 448 | scope | host 1, web 95 (`source`) | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `notes` | 3/24 | 362 | unused | - | upstream bot notes; no reader |
| `shortName` | 5/24 | 155 | unused | - | no runtime reader (two training/report examples only) |
| `isFakeRelic` | 1/24 | 19 | gameplay | engine 1 | keeps a pseudo-relic out of the relic deck |
| `shrinkName` | 1/24 | 18 | unused | - | upstream image layout hint |

#### leaders (103 records)

Mechanism: id-matched (leaders::dispatch_leader, faction modules, leaders::modifiers); deal and lock state from data; action-ness parsed from abilityWindow.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `imageBackURL` | 103/103 | 16861 | unused | - | upstream image URL, 16.9 KB |
| `abilityText` | 103/103 | 15629 | engine-label | engine 1, host 3, web 6 (`ability_text`) | copied into offer-card payloads (strategy_cards::leader_card); host catalog |
| `tfImageURL` | 49/103 | 8117 | unused | - | Twilight's Fall image URL |
| `abilityWindow` | 103/103 | 5090 | gameplay-text | engine 3, host 3, web 4 (`ability_window`) | printed window parsed: starts_with('ACTION') or 'During the action phase' decides whether a leader is offered as an action-phase ability and whether using it ends the turn; also copied into offer cards |
| `unlockCondition` | 103/103 | 5035 | ui | host 2, web 3 (`unlock_condition`) | host catalog only; the unlock rule of every commander is code (leaders::commander_unlocks) |
| `title` | 103/103 | 2883 | ui | host 2, web 52 (`title`) | host catalog |
| `name` | 103/103 | 2380 | engine-label | engine 3, host 2, web 352 (`name`) | choice labels and offer cards; host catalog |
| `id` | 103/103 | 2096 | id | engine 1 | identity; leaders::dispatch_leader / registered_abilities / faction modules match on it |
| `faction` | 103/103 | 1947 | gameplay | engine 2, host 1, web 119 (`faction`) | which faction is dealt the leader (for_faction, with keleres tag rules) |
| `abilityName` | 36/103 | 1870 | ui | host 1, web 4 (`ability_name`) | host catalog |
| `notes` | 12/103 | 1747 | unused | - | upstream bot notes; no reader |
| `source` | 103/103 | 1745 | scope | host 1, web 95 (`source`) | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `type` | 103/103 | 1637 | gameplay | engine 5, host 3, web 354 (`kind`) | agent / commander / hero: readied vs locked at start, unlock rules (51.2a, 51.7), Alliance note, Yin commander |
| `tfName` | 50/103 | 1356 | unused | - | Twilight's Fall printing |
| `tfTitle` | 51/103 | 965 | unused | - |  |
| `shortName` | 23/103 | 703 | unused | - | no runtime reader (two training/report examples only) |
| `tfAbilityText` | 4/103 | 684 | unused | - |  |
| `tfShortName` | 9/103 | 296 | unused | - |  |
| `tfAbilityWindow` | 4/103 | 272 | unused | - |  |
| `tfShrinkName` | 4/103 | 80 | unused | - |  |
| `shrinkName` | 3/103 | 54 | unused | - | upstream image layout hint |
| `homebrewReplacesID` | 1/103 | 33 | gameplay | engine 2 | a reprint (xxchahero-te) removes the record it replaces from the deal |

#### breakthroughs (31 records)

Mechanism: id-matched (breakthroughs.rs, faction modules); synergy colours from data.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `text` | 31/31 | 7161 | engine-label | engine 1, host 1, web 161 (`text`) | first sentence copied into the invasion offer card (breakthrough_card) |
| `notes` | 10/31 | 1271 | unused | - | upstream bot notes; no reader |
| `synergy` | 31/31 | 1043 | gameplay | engine 2, host 1, web 7 (`synergy`) | the two technology colours that count as one another (synergy.rs, fracture.rs) |
| `shortName` | 28/31 | 891 | unused | - | no runtime reader (two training/report examples only) |
| `name` | 31/31 | 783 | engine-label | engine 1, host 1, web 352 (`name`) | offer-card title; host catalog |
| `source` | 31/31 | 775 | scope | - | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `alias` | 31/31 | 587 | id | engine 5 | identity; breakthroughs::registered_aliases and faction modules |
| `faction` | 31/31 | 587 | gameplay | engine 4, host 1, bot 1, web 119 (`faction`) | which faction owns the breakthrough |
| `shrinkName` | 2/31 | 36 | unused | - | upstream image layout hint |

#### public_objectives (40 records)

Mechanism: id-matched predicate or price with thresholds as Rust literals; stage and VP from points.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `text` | 40/40 | 2359 | ui | host 2, web 161 (`text`) | host catalog; the requirement it states is duplicated as Rust literals (see threshold_check) |
| `name` | 40/40 | 1146 | ui | host 3, web 352 (`name`) | host catalog / score view |
| `alias` | 40/40 | 920 | id | - | identity; objectives::requirement_for / cost_of match on it (thresholds are Rust literals) |
| `phase` | 40/40 | 680 | ui | host 1, web 72 (`phase`) | host catalog only; public objectives score in the status phase by rule, not by this field |
| `source` | 40/40 | 620 | scope | - | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `points` | 40/40 | 440 | gameplay | engine 3, host 1, web 32 (`points`) | 1 = stage I, 2 = stage II when the deck is built; victory points awarded (points_for) |
| `notes` | 5/40 | 367 | unused | - | upstream bot notes; no reader |

#### secret_objectives (40 records)

Mechanism: id-matched predicate (secrets.rs); scoring phase and VP from data.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `text` | 40/40 | 2781 | ui | host 3, web 161 (`text`) | host catalog |
| `notes` | 12/40 | 1468 | unused | - | upstream bot notes; no reader |
| `name` | 40/40 | 1166 | engine-label | engine 1, host 3, web 352 (`name`) | a Thunder's Edge choice label; host catalog |
| `phase` | 40/40 | 680 | gameplay | engine 1, host 2, web 72 (`phase`) | Action / Status / Agenda: when the secret may be scored |
| `source` | 40/40 | 620 | scope | - | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `alias` | 40/40 | 557 | id | engine 1 | identity; secrets.rs match arms (counting_progress, nine_ground_forces, Feat bytes) |
| `points` | 40/40 | 440 | gameplay | engine 2, bot 1, web 32 (`points`) | victory points awarded (always 1 in the corpus); bot value |

#### technologies (102 records)

Mechanism: effects id-matched (ownership by exact id); research rules data-driven (types, requirements, faction, baseUpgrade); unit upgrades data-driven through units.json requiredTechId.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `text` | 102/102 | 17776 | gameplay-text | engine 2, host 1, web 161 (`text`) | researchability: a card whose text contains 'cannot be researched' is skipped (2 Obsidian cards); host catalog |
| `name` | 102/102 | 2768 | engine-label | engine 8, host 1, web 352 (`name`) | choice labels (research, action-card targets); host catalog |
| `types` | 102/102 | 2221 | gameplay | engine 10, host 1, bot 5, web 9 (`types`) | colour(s) or UNITUPGRADE: prerequisites satisfied, unit-upgrade rules, objectives counting colours, leader effects; bots value colours |
| `requirements` | 92/102 | 1835 | gameplay | engine 1, host 1, bot 2, web 2 (`requirements`) | prerequisites as a letter string ('GG', 'BR'; 'null'/'None' spelled out); parsed letter by letter |
| `source` | 102/102 | 1731 | scope | engine 1, host 1, web 95 (`source`) | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `alias` | 102/102 | 1530 | id | engine 9, bot 1 | identity; ownership is checked by exact id (TechnologyId::new("gd")) in ~300 places |
| `shortName` | 45/102 | 1473 | unused | - | no runtime reader (two training/report examples only) |
| `faction` | 65/102 | 1230 | gameplay | engine 11, host 1, bot 1, web 119 (`faction`) | faction technology belongs to that faction only (90.11); faction asset ledger |
| `notes` | 6/102 | 941 | unused | - | upstream bot notes; no reader |
| `initials` | 28/102 | 461 | unused | - |  |
| `shrinkName` | 21/102 | 383 | unused | - | upstream image layout hint |
| `baseUpgrade` | 16/102 | 323 | gameplay | engine 2, host 1, web 2 (`base_upgrade`) | the generic unit upgrade a faction upgrade replaces (makes the generic unresearchable for that faction) |

#### abilities (73 records)

Mechanism: id-matched (faction_abilities::registered, faction modules); the faction's list comes from factions.json.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `windowEffect` | 48/73 | 5508 | engine-label | engine 1, host 3, web 4 (`window_effect`) | ability offer card; host catalog |
| `permanentEffect` | 34/73 | 4550 | ui | host 3, web 4 (`permanent_effect`) | host catalog and reaction view |
| `window` | 48/73 | 2665 | engine-label | engine 1, host 3, web 21 (`window`) | ability offer card; host catalog |
| `name` | 73/73 | 1524 | engine-label | engine 1, host 2, web 352 (`name`) | ability offer card (faction_abilities::ability_card) |
| `faction` | 73/73 | 1385 | ui | host 1, web 119 (`faction`) | host catalog; the engine takes a faction's abilities from factions.json 'abilities', not from this field |
| `id` | 73/73 | 1347 | id | engine 1 | identity; faction_abilities::registered and faction modules |
| `source` | 73/73 | 1291 | scope | - | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `shortName` | 15/73 | 473 | unused | - | no runtime reader (two training/report examples only) |
| `notes` | 4/73 | 322 | unused | - | upstream bot notes; no reader |
| `shrinkName` | 1/73 | 18 | unused | - | upstream image layout hint |

#### explores (80 records)

Mechanism: deck and resolution path from data; Instant cards id-matched (exploration::resolve_instant); Attach cards data-driven via attachmentId + attachments.json.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `text` | 80/80 | 7759 | engine-label | engine 1, host 3, web 161 (`text`) | printed text in exploration offer; host catalog |
| `flavorText` | 44/80 | 6400 | ui | host 1 | host catalog flavor_text |
| `name` | 80/80 | 2218 | engine-label | engine 1, host 2, web 352 (`name`) | offer label in exploration choices |
| `resolution` | 80/80 | 1845 | gameplay | engine 2, host 3, web 12 (`resolution`) | Instant / Fragment / Attach / Token decides the resolution path; Fragment and Attach need no handler |
| `type` | 80/80 | 1500 | gameplay | engine 4, host 2, web 354 (`kind`) | which exploration deck (cultural/hazardous/industrial/frontier), compared case-insensitively |
| `source` | 80/80 | 1218 | scope | - | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `id` | 80/80 | 943 | id | engine 1 | identity; exploration::resolve_instant matches on it |
| `attachmentId` | 15/80 | 402 | gameplay | engine 1, host 2 | the attachment an Attach card puts on a planet; '{id}stat' naming convention picks the research-facility variant |

#### attachments (22 records)

Mechanism: data-driven: planet value, specialty, traits and space cannon read from the record.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `name` | 20/22 | 564 | engine-label | engine 1, host 2, web 352 (`name`) | production breakdown label; host catalog |
| `id` | 22/22 | 406 | id | - | identity; planets carry attachment ids |
| `source` | 22/22 | 356 | scope | - | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `resourcesModifier` | 14/22 | 308 | gameplay | engine 1, host 2, web 6 (`resources_modifier`) | planet resource bonus |
| `influenceModifier` | 11/22 | 242 | gameplay | engine 1, host 2, web 6 (`influence_modifier`) | planet influence bonus |
| `techSpeciality` | 4/22 | 121 | gameplay | engine 1, host 2, web 17 (`tech_specialties`) | technology specialty given to the planet |
| `isFakeAttachment` | 4/22 | 96 | ui | host 1 | host exploration view only |
| `planetTypes` | 1/22 | 52 | gameplay | engine 1 | traits added to the planet (Dyson Sphere?) |
| `token` | 2/22 | 33 | unused | - |  |
| `spaceCannonDieCount` | 1/22 | 24 | gameplay | engine 1 | as spaceCannonHitsOn |
| `spaceCannonHitsOn` | 1/22 | 22 | gameplay | engine 1 | planet SPACE CANNON granted by the attachment |
| `isLegendary` | 1/22 | 19 | unused | - | would be gameplay (legendary attachment); nothing reads it |

#### strategy_cards (12 records)

Mechanism: dispatch on printed name ('Leadership' => ...), id only for the two Thunder's Edge variants; initiative from data; dealt set from strategy_card_sets.json.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `primaryTexts` | 12/12 | 2145 | ui | host 1, web 6 (`primary_text`) | host catalog |
| `secondaryTexts` | 12/12 | 1450 | ui | host 1, web 8 (`secondary_text`) | host catalog |
| `imageFileName` | 12/12 | 334 | unused | - |  |
| `colourHexCode` | 10/12 | 260 | unused | - |  |
| `id` | 12/12 | 232 | id | engine 2 | identity; but behaviour dispatches on the printed name (see name), except te4construction / te6warfare which are matched by id |
| `name` | 12/12 | 229 | gameplay-text | engine 2, host 3, bot 1, web 352 (`name`) | strategy_cards::card_name feeds `match name.as_str()` ("Leadership" => ...) for primaries and secondaries; base2 and pok2diplomacy, base4/pok4construction/te4construction share a name |
| `source` | 12/12 | 211 | scope | - | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `initiative` | 12/12 | 180 | gameplay | engine 2, content 1, host 2, bot 1, web 14 (`initiative`) | turn order and the dealt set; sort key |
| `botSCAutomationID` | 3/12 | 110 | unused | - |  |

#### galactic_events (20 records)

Mechanism: not implemented: no reader anywhere.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `text` | 20/20 | 9506 | unused | - |  |
| `flavorText` | 3/20 | 960 | unused | - | no reader except for explores (see explores.flavorText) |
| `name` | 20/20 | 546 | unused | - |  |
| `alias` | 20/20 | 500 | id | - | no reader: galactic events are not implemented |
| `source` | 20/20 | 500 | scope | - | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `complexity` | 20/20 | 300 | unused | - | the two content sites the pattern finds are faction complexity |
| `notes` | 1/20 | 50 | unused | - | upstream bot notes; no reader |

#### genericcards (10 records)

Mechanism: plot cards id-matched in factions/firmament_plots.rs; mutated_* twins unused.

| field | records | bytes | class | readers (engine/host/bot, web catalog sites) | use |
|---|---|---|---|---|---|
| `text` | 10/10 | 995 | unused | - | not in the host catalog either: the web has no printed text for plot cards from the corpus |
| `source` | 10/10 | 250 | scope | - | parsed at load into Record::source; ContentStore::from_sources filters every deck, catalogue and lookup by it |
| `alias` | 10/10 | 216 | id | - | the five plot cards are named in code (ti4_model::plots::CARDS, firmament_plots consts) |
| `name` | 10/10 | 206 | unused | - |  |
| `cardType` | 10/10 | 180 | unused | - | 'plot'; read only by a test |

## Size: what a split would save

Bytes of the 15 card files, minified as they are embedded (`ti4-content/build.rs`), by class
(measured; each subset below includes the id field, so subsets overlap by it):

| subset | raw | gzip -9 | xz |
|---|---|---|---|
| all card records | 313.8 KB | 67.5 KB | 56.7 KB |
| gameplay as today (id, scope, gameplay, gameplay-text) | 82.0 KB | 13.0 KB | 11.9 KB |
| gameplay without the prose fields (text-parsed fields replaced by enums) | 50.0 KB | 6.8 KB | 6.2 KB |
| display read today (ui + engine-label) | 153.9 KB | 34.6 KB | 30.5 KB |
| unused | 111.2 KB | 29.1 KB | 26.5 KB |

By class over all fields: unused 94.5 KB, engine-label 70.9 KB, ui 66.3 KB, gameplay-text
31.9 KB (17.8 KB of it is technology `text`, read for one substring), gameplay 19.4 KB, id 14.8 KB,
scope 13.9 KB.

What that means per build (inferred from these numbers and `docs/BUILD_SIZES_AND_TIMES.md`):

- **Engine-only binaries** (self-play, training, the rule-bot harness): the whole corpus sits in
  `.rodata` and is parsed into `serde_json::Map`s at start. Dropping unused + display card fields
  removes ~250 KB of the 685 KB of embedded text and the matching parsed maps; typed records (parsed
  once, `Copy` enums) would also remove the string-keyed field lookups that
  `docs/ENGINE_OPTIMIZATION.md` puts at 8-9% of a bot game (row 13, "est. -5..-9%").
- **wasm**: the page needs the display text anyway (the web reads it from the catalog, which in the
  offline build comes out of the same wasm). Moving it out of the engine shrinks the wasm by ~250 KB
  raw only if the display table becomes a separately fetched, cacheable asset
  (`catalog-display.json`, ~35 KB gzip for what is read today, ~6 KB less if flavour text is dropped);
  the first download is then about the same, later loads skip it when cached, and an engine update
  no longer re-downloads card text. Deleting the unused fields alone (no split) removes ~111 KB raw /
  ~29 KB gzip from every build with no behaviour change at all.
- **Server**: no difference that matters (one copy in memory).

## Issues

Prioritised; triage separately (nothing here was fixed). P1 = a rule is wrong in the default
scope, P2 = drift risk or text-parsing that decides an outcome, P3 = cleanup/architecture. "Basis"
says whether it was measured by the tool or read in the code.

| id | prio | kind | issue | cards | evidence | basis |
|---|---|---|---|---|---|---|
| I01 | P1 | deck | Two extra Veto copies are dealt: the action deck is 142 cards, not 140 | action_cards:veto3, action_cards:veto4 | `crates/ti4-engine/src/deck.rs:122`<br>`crates/ti4-content/content/decks.json:1` | measured |
| I02 | P1 | silently-ignored | Galactic Crisis Pact (agenda 'crisis') is always discarded and has no effect | agendas:crisis | `crates/ti4-engine/src/vote.rs:117`<br>`crates/ti4-engine/src/game.rs:6014`<br>`crates/ti4-engine/src/agenda_effects.rs:987` | read |
| I03 | P1 | silently-ignored | Classified Document Leaks is always discarded, even when secrets are scored | agendas:classified | `crates/ti4-engine/src/vote.rs:116`<br>`crates/ti4-engine/src/game.rs:6429` | read |
| I04 | P1 | partial | Covert Legislation is voted as a plain For/Against agenda with no effect | agendas:covert | `crates/ti4-engine/src/agenda_effects.rs:983`<br>`crates/ti4-engine/src/vote.rs:83` | measured |
| I05 | P1 | partial | Demilitarized Zone exploration attachment does nothing beyond attaching | explores:dmz, attachments:dmz | `crates/ti4-engine/src/exploration.rs:719`<br>`crates/ti4-engine/src/laws.rs:169` | measured |
| I06 | P2 | text-parsed | Reaction windows are bound by the exact printed sentence | action_cards:* | `crates/ti4-engine/src/reactions.rs:633`<br>`crates/ti4-content/src/action_cards.rs:70` | measured |
| I07 | P2 | text-parsed | Agenda elections are parsed from the `target` prose | agendas:* | `crates/ti4-engine/src/vote.rs:82`<br>`crates/ti4-engine/src/game.rs:5819` | read |
| I08 | P2 | text-parsed | Whether a leader is an action and ends the turn is parsed from `abilityWindow` | leaders:* | `crates/ti4-engine/src/leaders.rs:467`<br>`crates/ti4-engine/src/leaders.rs:482` | read |
| I09 | P2 | name-keyed | Strategy cards are dispatched on the printed name | strategy_cards:base2, strategy_cards:base4, strategy_cards:pok2diplomacy, strategy_cards:pok4construction (+2) | `crates/ti4-engine/src/strategy_cards.rs:2653`<br>`crates/ti4-engine/src/strategy_cards.rs:2667`<br>`crates/ti4-engine/src/strategy_cards.rs:2526` | read; texts compared from strategy_cards.json |
| I10 | P2 | text-parsed | Technology researchability depends on the substring 'cannot be researched' | technologies:planesplitter-obs, technologies:parasite-obs | `crates/ti4-engine/src/technology.rs:1133`<br>`crates/ti4-engine/src/factions/deepwrought.rs:391` | measured |
| I11 | P2 | deck | Deck composition uses four different mechanisms | agendas:*, technologies:*, action_cards:*, relics:* | `crates/ti4-engine/src/deck.rs:156`<br>`crates/ti4-engine/src/technology.rs:39`<br>`crates/ti4-engine/src/deck.rs:113` | read |
| I12 | P2 | duplication | Objective thresholds and prices are Rust literals beside the printed text | public_objectives:*, secret_objectives:* | `crates/ti4-engine/src/objectives.rs:1129`<br>`crates/ti4-engine/src/objectives.rs:1512` | measured (threshold_check) |
| I13 | P2 | guard | The hard-coded-id test skips 12,005 production lines and merges every category's ids | - | `crates/ti4-engine/tests/content_ids_resolve.rs:179`<br>`crates/ti4-engine/tests/content_ids_resolve.rs:145` | measured |
| I14 | P2 | ids | 36 ids are spelled the same in two categories | technologies:mr, promissory_notes:<color>_ps, technologies:ps, agendas:crisis (+7) | `docs/card_audit/id_refs.json:1` | measured (id_collisions_across_categories) |
| I15 | P3 | typo | faction_abilities::blocked names 'your_ships_have_no_shields', which is no ability id | - | `crates/ti4-engine/src/faction_abilities.rs:50` | measured (dangling literal) |
| I16 | P3 | consistency | The same field is compared in different ways | - | `crates/ti4-engine/src/action_cards.rs:462`<br>`crates/ti4-engine/src/factions/keleres.rs:662`<br>`crates/ti4-engine/src/laws.rs:808`<br>`crates/ti4-engine/src/vote.rs:231` | read |
| I17 | P3 | architecture | The engine reads display text to build labels and offer cards | - | `crates/ti4-engine/src/choice.rs:194`<br>`crates/ti4-engine/src/strategy_cards.rs:1820`<br>`crates/ti4-engine/src/fingerprint.rs:209` | measured |
| I18 | P3 | name-keyed | Card names are copied by hand into code | - | `crates/ti4-engine/src/reaction_preview.rs:29`<br>`web/src/presentation/actionCardTurns.ts:14`<br>`crates/ti4-engine/src/laws.rs:215` | measured (name_keyed_sites) |
| I19 | P3 | duplication | Id lists that duplicate the corpus | - | `crates/ti4-model/src/plots.rs:27`<br>`crates/ti4-engine/src/promissory.rs:26`<br>`crates/ti4-engine/src/deck.rs:156` | read |
| I20 | P3 | not-implemented | Galactic events and Wild Wild Galaxy printings are not implemented | galactic_events:* | `crates/ti4-engine/src/action_cards.rs:3387` | measured |
| I21 | P3 | dead-data | Content records that are never dealt | leaders:redcreussagent, leaders:redcreusscommander, leaders:redcreusshero, leaders:orlandohero (+10) | `crates/ti4-engine/src/leaders.rs:198` | measured (status not-dealt) |
| I22 | P3 | dead-data | Gameplay-shaped fields nobody reads | - | `docs/card_audit/fields.csv:1` | measured |
| I23 | P3 | ledger | Coverage ledgers report 100% where some cards do nothing | agendas:crisis, agendas:covert, agendas:classified, explores:dmz | `crates/ti4-engine/src/registry.rs:135` | measured |

Details:

- **I01 Two extra Veto copies are dealt: the action deck is 142 cards, not 140.** deck::build_starting_decks takes every action-card record in scope. veto3 and veto4 are source 'base' records that no official deck in decks.json lists (action_cards_te has 140 ids, action_cards_basegame 80); the dealt deck has three Vetos. Basis: measured: registry_dump decks_seed1.action_cards has 142 ids incl. veto3/veto4; set difference with decks.json action_cards_te.
- **I02 Galactic Crisis Pact (agenda 'crisis') is always discarded and has no effect.** Its target is 'Elect Strategy Card'; vote::outcomes returns no candidates for it ('Neither is modelled'), so Game::reveal_agenda discards it and reveals the next card. agenda_effects lists it as an 'availability-only' standing rule, but the corpus types it a Directive and nothing else in the engine names it. The registry counts it implemented. Basis: read: code path; content type 'Directive' measured.
- **I03 Classified Document Leaks is always discarded, even when secrets are scored.** Target 'Elect Scored Secret Objective (...)' yields no candidates in vote::outcomes in every state, so the card is discarded on reveal; the effect code (laws::classified_leak via game.rs) is unreachable. The test an_agenda_with_nothing_to_elect_is_replaced_by_the_next_card only covers the no-secret case. Basis: read.
- **I04 Covert Legislation is voted as a plain For/Against agenda with no effect.** Its target is a rules sentence ('When this agenda is revealed, the speaker draws ...'), which vote::outcomes reads as For/Against; the effect arm is empty and the comment says the driver applies it, but no other code names 'covert'. Basis: measured: no production reference outside agenda_effects.rs.
- **I05 Demilitarized Zone exploration attachment does nothing beyond attaching.** explores 'dmz' (Attach, attachmentId 'dmz') attaches a record with no modifiers. Its text (return structures and ground forces; units cannot be committed to, produced on or placed on the planet) has no implementation: no production code names 'dmz'. laws::planet_is_demilitarized covers only the base agenda 'demilitarized_zone'. The exploration ledger counts every Attach card as implemented. Basis: measured: grep of engine production code for "dmz" finds nothing.
- **I06 Reaction windows are bound by the exact printed sentence.** reactions::window_for looks the trimmed `window` text up in window_table; content::action_cards::timing_of repeats ~50 of the same sentences. An errata or a whitespace change in the JSON unbinds a card (the engine then treats it as not a reaction). The upstream `playTiming` enum (16 cards) is unused. Basis: measured: every printed window is bound today (registry_dump).
- **I07 Agenda elections are parsed from the `target` prose.** vote::outcomes splits target at '(' and searches for 'Player', 'Planet', 'Non-Home', 'Other Than Mecatol', 'Law'; game.rs checks starts_with("Elect Player") separately. Unknown shapes fall back to For/Against (Covert) or to no candidates (Classified, Crisis). Basis: read.
- **I08 Whether a leader is an action and ends the turn is parsed from `abilityWindow`.** starts_with('ACTION') / 'During the action phase'; the doc comment records an earlier bug where no PoK agent printed for the action phase was ever offered. Basis: read.
- **I09 Strategy cards are dispatched on the printed name.** strategy_cards::card_name feeds `match name.as_str()`; base2 and pok2diplomacy are both 'Diplomacy' (base2 in the base_game set prints 'ready each exhausted planet you control in that system', the engine readies up to 2), base4 / pok4construction / te4construction are all 'Construction' (te4 and te6 are special-cased by id). Basis: read; texts compared from strategy_cards.json.
- **I10 Technology researchability depends on the substring 'cannot be researched'.** Checked in two places with their own copy of the test; it affects the two Obsidian cards today. Basis: measured: the two records whose text contains it.
- **I11 Deck composition uses four different mechanisms.** Action cards and explores: every record in scope. Agendas: in scope minus the hard-coded SUPERSEDED_BY_POK list. Relics: in scope minus isFakeRelic. Technologies: decks.json techs_pok_c4 (plus TE) regardless of scope, so a base-only game is offered PoK technologies. The other 23 decks of decks.json are unused. Basis: read.
- **I12 Objective thresholds and prices are Rust literals beside the printed text.** objectives::counting_progress and cost_of hold the numbers (6 planets, 8 resources ...); the printed text is display only. threshold_check in cards.csv compares them mechanically: all public objectives agree today. Basis: measured (threshold_check).
- **I13 The hard-coded-id test skips 12,005 production lines and merges every category's ids.** tests/content_ids_resolve.rs cuts each file at the first `#[cfg(test)]`, which is often a test-only function in the middle of the file (game.rs: 5,448 lines skipped, combat.rs 2,812, timing.rs 838, hooks_cards.rs 775 ...). It also accepts a literal that is an id of any category, so a wrong-category id (36 ids are spelled the same in two categories) passes. Running its rule over the skipped lines finds no dangling id today. Basis: measured.
- **I14 36 ids are spelled the same in two categories.** e.g. technology 'mr' and planet Mecatol Rex 'mr', promissory 'ps' (Political Secret, bare alias) and technology 'ps' (Plasma Scoring), agenda and action card 'crisis', relic and attachment 'nanoforge', ability and public objective 'galvanize', technology and secret 'sar'. Literal-reference counts for these ids are ambiguous (secrets.rs b"sar" is the secret, not the technology). A shared string-id namespace or enum would have to stay category-scoped. Basis: measured (id_collisions_across_categories).
- **I15 faction_abilities::blocked names 'your_ships_have_no_shields', which is no ability id.** The other four entries are abilities; this one matches no record of any category, so the filter that drops blocked entries once a module implements them can never apply to it. Basis: measured (dangling literal).
- **I16 The same field is compared in different ways.** window 'Action': trim() == "Action" in action_cards.rs, eq_ignore_ascii_case("action") in keleres.rs; agenda type: eq_ignore_ascii_case("law") in vote.rs, == Some("Law") in laws.rs. Basis: read.
- **I17 The engine reads display text to build labels and offer cards.** 62 'Display only' sites (choice::offer_card, agenda_details, leader_card, ability_card, breakthrough_card, exploration offers) copy names, windows and texts into payloads, so the engine cannot drop display text today. Prompts that embed a name are part of the V2 decision fingerprint; payload facts are not. Basis: measured: grep count of 'Display only' in engine production code.
- **I18 Card names are copied by hand into code.** reaction_preview::PREVIEWED and preview_for match on ~38 printed action-card names; strategy_cards::registered_cards lists names; web actionCardTurns BY_NAME keys 8 names and classifies windows by regex; web pausablePrompts repeats ~60 names as labels; laws::planet_value_bonus_sources repeats three agenda names with their numbers. Basis: measured (name_keyed_sites).
- **I19 Id lists that duplicate the corpus.** ti4_model::plots::CARDS (the five plot ids), promissory::GENERIC (cf, ps, ta, an), deck::SUPERSEDED_BY_POK (13 agendas), content::agendas/public_objectives/secret_objectives group_of tables (tests catch a missing id, not a wrong group). Basis: read.
- **I20 Galactic events and Wild Wild Galaxy printings are not implemented.** No code reads galactic_events.json (20 Thunder's Edge cards) and nothing reads the wildWild* fields of 36 action cards; War Machine documents that its WILD WILD printing is not modelled. Nothing offers them either, so this is scope, not a silent bug. Basis: measured: no reader.
- **I21 Content records that are never dealt.** leaders redcreussagent/commander/hero (unofficial variants under source 'pok'), orlandohero (a Bastion hero tagged source 'pok' although Bastion is a Thunder's Edge faction); promissory 'rider' (superseded by ridera/riderm/riderx); technologies x89_base, x89, md_base, md_c1 (superseded printings); genericcards mutated_* (5). Basis: measured (status not-dealt).
- **I22 Gameplay-shaped fields nobody reads.** promissory_notes.playImmediately (11) and .attachment (7), attachments.isLegendary and .token, action_cards.playTiming (16), strategy_cards.botSCAutomationID; the engine encodes those facts per card in code instead. Basis: measured.
- **I23 Coverage ledgers report 100% where some cards do nothing.** registry::ledger / coverage_report count a registered alias as implemented; availability-only agenda arms (crisis, covert) and every Attach exploration card count. Combined with I02-I05 the ledgers overstate coverage by at least four cards. Basis: measured: registry_dump shows no unimplemented card in any registry.

## Recommendation

### Target shape

1. **A typed id per category, generated from the corpus**, e.g. `ActionCard`, `Agenda`,
   `Technology`, `Leader`, ... (15 types, 802 ids). Generated by `ti4-content/build.rs` (or a
   checked-in file regenerated by an `xtask`, with a test that fails when it is stale; better for
   reviews and IDEs) as `#[repr(u16)] enum` with:
   - variants **declared in lexical order of the id** and `Ord` checked against `as_str()` order,
     because `BTreeMap`/`BTreeSet` iteration and `deck::ids` (sort, then a seeded shuffle) depend on
     string order today; any other order changes every seeded game and every golden digest;
   - `as_str()`, `FromStr`, `Display`, and serde as the string (wire format, saved games, replays,
     decision fingerprints unchanged);
   - generated names from the id (`Mb1`, `Sabo3`, `Xxchahero`, `XxchaheroTe`); the printed name
     is display data and does not become the variant name (copies share a name, and names change
     with errata);
   - for action cards a second enum `ActionCardKind` (the printed card: `MoraleBoost`) with
     `ActionCard::kind()`; `effect_for` matches kinds, which also retires the hand-listed copy arms
     and the "merge copies by name" logic.
   Keep the categories apart: 36 ids collide across categories (`mr`, `ps`, `crisis`, `nanoforge`,
   `galvanize`, `sar`, ...).
2. **A small gameplay struct per category, parsed once at load**, holding only what the rules read
   (the "needs data" column above), with the free-text fields replaced by enums:

   | category | typed gameplay data |
   |---|---|
   | action cards | `kind: ActionCardKind`, `timing: Timing` (the `window_table` key becomes the enum; the sentence moves to display) |
   | agendas | `kind: Law \| Directive`, `elects: ForAgainst \| Player \| Planet { non_home_non_mecatol } \| Law \| ScoredSecret \| StrategyCard \| Hidden` |
   | promissory notes | `faction: Option<FactionId>`, `play_area: bool` (+ `play_immediately` if the code is to use it) |
   | relics | `in_deck: bool` |
   | leaders | `faction`, `kind: Agent \| Commander \| Hero`, `replaces: Option<Leader>`, `timing: Action \| ActionPhase \| Other` |
   | breakthroughs | `faction`, `synergy: [Colour; 2]` |
   | public objectives | `stage: 1 \| 2`, `points` (thresholds stay in code, or move here as data with a test) |
   | secret objectives | `phase: Action \| Status \| Agenda`, `points` |
   | technologies | `colour: Option<Colour>`, `unit_upgrade: bool`, `prereqs: [u8; 4]`, `faction`, `replaces: Option<Technology>`, `researchable: bool` |
   | abilities | nothing (membership comes from the faction sheet) |
   | explores | `deck: Trait`, `resolution: Instant \| Fragment \| Attach \| Token`, `attachment: Option<Attachment>` |
   | attachments | `resources`, `influence`, `specialty`, `traits`, `space_cannon: Option<(hits_on, dice)>` |
   | strategy cards | `kind: Leadership \| Diplomacy \| ...` plus a variant tag (`Base`, `Codex`, `ThundersEdge`), `initiative` |
   | galactic events, generic cards | nothing until implemented |

   These can be `static` tables generated from the JSON (no parse at start) or parsed from the
   JSON into `Vec<T>` indexed by the enum; either way a lookup is an array index instead of a
   string-keyed `BTreeMap` walk.
3. **A display layer outside the engine**: names, texts, windows as printed, flavour, titles,
   unlock conditions; one table per category keyed by the same ids, owned by the host (which
   already builds exactly this as `view::records::ContentRecords`) and shipped to the web as today
   (`GET /api/catalog`, or a static `catalog-display.json` for the offline page). The engine's
   offer cards and payloads carry ids and the host joins the text in (the catalog is already on the
   page, so `offer_card(title, tag, window, text)` can become `offer_card(card_ref)`).
4. **Deck composition from one source**: `decks.json` (or a typed deck list) per scope, used for
   every deck, replacing the four mechanisms in I11.

### What is lost

- **Adding a card without code** keeps working: the enum is generated from the JSON, so a new
  record still appears in decks and catalogs and stays "unavailable" until code handles it, as
  today. A new card *with* an effect needs JSON + code today and still does.
- **Loading a different corpus at runtime** (`ContentStore::from_dir`, used by provenance tests and
  one `ti4-policy` test that edits Sol's commodities) can no longer introduce *new* ids, only
  change data of known ones. Unknown ids in a saved game or replay need a policy: reject on load, or
  an `Unknown(Box<str>)` escape variant (keeps old saves readable but reintroduces strings in the
  hot path). Recommend: reject, after checking all stored replays and checkpoints parse.
- **Upstream re-extraction** gets a codegen step and a reviewable diff of the enum, which is the
  point (today a renamed id silently unbinds code; I06/I13).

### Risks

- **Wire, saves, replays, fingerprints** must keep string ids: serde as string, and keep the V2
  decision fingerprint inputs byte-identical. Prompts that embed card names
  (`format!("... {name}")`) are hashed (fingerprint.rs:209 binds the prompt), so moving names out
  of the engine must keep the prompt text or version the fingerprint.
- **Ordering** (see 1. above): an enum whose `Ord` differs from the string order changes seeded
  games; generate in lexical order and test it.
- **The behavioural baseline** of `ti4-sim` moves only through its versioned process; fixing I01
  (deck), I02-I05 (agendas, DMZ) and I09 (Diplomacy variant) changes games and has to go through
  it. The pure refactors (ids, typed records, display split) must not move it: use the golden
  games (`tools/engine_profile/verify_benches.sh`) as the gate.
- **State still holds strings** in places (`agenda_deck: Vec<String>`, `laws: BTreeMap<String,
  String>`, `exploration_decks: BTreeMap<String, Vec<String>>`, note ids `alias:faction`,
  `support:<faction>`), so the id migration is per field of `GameState`, not only per content type.

### Staged plan

| stage | what | effort | gate |
|---|---|---|---|
| 0 | Guards first: fix the id test (cut at the test *module*, check per category), add a test that every action-card window, agenda target, leader window and strategy-card name maps to a known enum value, delete the unused fields (111 KB) | 0.5-1 day | all tests, digest of the corpus changes (expected), golden games unchanged |
| 1 | Generated per-category id enums with string serde; adopt them first where code matches ids (`effect_for`, `resolve_with`, `use_relic`, `requirement_for`, `dispatch_leader`, `resolve_instant`); typos become compile errors | 3-5 days | golden games and fingerprints byte-identical |
| 2 | Typed gameplay structs per category (table above), replacing `Record` reads in the engine; the five text-parsed fields become enums (window, target, abilityWindow, tech `researchable`, strategy-card kind) | 3-5 days | as 1; plus the new mapping tests |
| 3 | Engine stops reading display text: offer cards and payloads carry ids, host joins names/texts; keep prompt strings stable or version the fingerprint | 3-5 days | web screenshots of offer cards (UI work: artifact per CLAUDE.md), golden games |
| 4 | Display layer: `ContentRecords` built from a display-only file; engine builds without it; wasm loads it as an asset | 2-3 days | wasm size budget, offline smoke |
| 5 | The rule fixes of the issue list (I01-I05, I09, I11), each as its own behaviour change through the baseline process | 0.5-2 days each | `ti4-sim` baseline bump with cause |

Stages 0, 1 and 2 are worth doing on their own (type safety, the 8-9% content-lookup cost); 3 and
4 are what removes text from the engine build.

## Files

- `tools/card_audit/card_audit.py`: the audit (static scan + curation + registry dump join).
- `tools/card_audit/curation.json`: field classes, mechanisms, issues, reviewed literals; each
  claim with `file:line ~ pattern` evidence that the script re-checks.
- `tools/card_audit/build_page.py` + `page_template.html`: build the browsable HTML page from `--json` output.
- `crates/ti4-engine/examples/card_registry_dump.rs`: read-only dump of the engine's registries.
- `docs/card_audit/cards.csv`, `fields.csv`, `id_refs.json`, `registry_dump.json`: outputs.
