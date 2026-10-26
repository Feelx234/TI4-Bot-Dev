# Card text fields read as enums

The card audit (`docs/CARD_USAGE_AUDIT.md` on branch `card-usage-audit`, issues I06 to I10) found
five places where a rule was decided by parsing free text from the content JSON. Each is now mapped
once, when a `Record` is built, onto a small enum in `crates/ti4-content/src/card_fields.rs`, and
the engine, host and rule bot read the enum. The behaviour is identical: no RULES_VERSION bump, the
wire format, TS types and fixtures are unchanged, and the golden self-play benches are unchanged
(see "Verification").

## Design

- **The corpus is unchanged.** `crates/ti4-content/content/*.json` stays as extracted; its manifest,
  `CHECKSUMS.sha256` and the canonical digest (`provenance::digest_of`, which hashes the parsed
  fields) are untouched. The value mappings live in Rust beside the corpus, not in it.
- **Mapped once, at load.** `Record::new` calls `CardFields::of` for the five categories and stores
  the result in the record. The accessors are `Record::action_card_window`, `Record::elect_type`,
  `Record::leader_ability_kind`, `Record::research_restriction` and `Record::strategy_card_kind`
  (all `const fn`, `Copy` results, no string work at the call site).
- **No catch-all.** Each field has a value table (`TABLE`) that lists every distinct value the
  corpus prints, with its variant. A value outside the table fails the corpus load with
  `ContentError::UnknownFieldValue { category, index, field, value }`, so a reworded card or a new
  printing is a loud error rather than a card that silently stops being offered. An absent field
  still reads as absent (`None`), exactly as `Record::text` did.
- **Compared after `trim()`**, as the old code compared them. No corpus value carries surrounding
  whitespace, so the trim changes nothing.
- **"An explicit variant for every distinct value"** is read as "every distinct value maps to an
  explicitly listed variant". For the action-card window that is one variant per sentence (the
  engine binds each sentence to its own event). For the other fields several printed values share a
  variant (61 leader windows are `Other`), because one variant per sentence would be a 63-variant
  enum that nothing distinguishes; the tables still list every value.

Tests (`card_fields::tests`, 18): every record of every category maps; each table equals the set of
values the corpus prints (no dead rows); the full value tables are spelled out; each enum agrees,
on every record, with the text test it replaces (which is what makes the call-site switch provably
behaviour-neutral); unknown values are rejected by each parser and by `Record::new`.

## The five fields

### 1. Action-card `window` -> `ActionCardWindow` (I06)

52 distinct printed windows, 52 variants, declared in the byte order of the printed text so `Ord`
(and the iteration order of a map keyed by it) is the order of the strings it replaces
(`the_action_card_window_order_is_the_order_of_the_printed_text`). `Action` (42 cards) is a
component action; the other 51 are reaction windows. `ActionCardWindow::printed()` returns the
sentence; `is_component_action()` replaces the three spellings of the test
(`trim() == "Action"`, `eq_ignore_ascii_case("action")`, `trim() != "Action"`; audit I16).

| printed window | variant |
|---|---|
| Action | `Action` |
| After 1 of your ships is destroyed during a space combat | `AfterOwnShipDestroyedInSpaceCombat` |
| After 2 of your neighbors resolve a transaction | `AfterNeighborsTransact` |
| After a player moves ships into a system that contains your ships | `AfterShipsMoveIntoSystemWithYourShips` |
| After an agenda is revealed | `AfterAgendaRevealed` |
| After another player activates a system that contains 1 of your command tokens | `AfterOtherActivatesSystemWithYourCommandToken` |
| After another player activates a system that contains 1 or more of your structures | `AfterOtherActivatesSystemWithYourStructures` |
| After another player activates a system that contains your units | `AfterOtherActivatesSystemWithYourUnits` |
| After another player commits units to land on a planet you control | `AfterOtherCommitsToYourPlanet` |
| After another player discards an action card that has a component action | `AfterOtherDiscardsComponentActionCard` |
| After another player gains control of a planet you control | `AfterOtherGainsYourPlanet` |
| After another player makes a BOMBARDMENT, SPACE CANNON, or ANTI-FIGHTER BARRAGE roll | `AfterOtherRollsUnitAbility` |
| After another player's ship uses SUSTAIN DAMAGE to cancel a hit produced by your units or abilities | `AfterOtherSustainsAgainstYourHit` |
| After the active player moves ships into the active system during a tactical action | `AfterActivePlayerMovesIntoActiveSystem` |
| After the speaker votes on an agenda | `AfterSpeakerVotes` |
| After you activate a system | `AfterYouActivateSystem` |
| After you activate a system that contains 1 or more of your ships | `AfterYouActivateSystemWithYourShips` |
| After you activate a system that contains another player's ships | `AfterYouActivateSystemWithOtherShips` |
| After you activate an anomaly | `AfterYouActivateAnomaly` |
| After you cast votes on an outcome of an agenda | `AfterYouCastVotes` |
| After you perform an action | `AfterYouPerformAction` |
| After you win a space combat | `AfterYouWinSpaceCombat` |
| After your ground forces make combat rolls during a round of ground combat | `AfterYourGroundForcesRoll` |
| After your opponent declares a retreat during a space combat | `AfterOpponentDeclaresRetreat` |
| At the end of a player's turn, if you have passed | `EndOfTurnIfYouPassed` |
| At the end of any players turn, if there are at least 2 players who have not passed | `EndOfTurnIfTwoNotPassed` |
| At the start of a combat | `StartOfCombat` |
| At the start of a combat round | `StartOfCombatRound` |
| At the start of an invasion | `StartOfInvasion` |
| At the start of an invasion in a system that contains 1 or more of your opponents' PDS units | `StartOfInvasionAgainstPds` |
| At the start of another player's turn, if they have a readied strategy card | `StartOfOtherTurnWithReadiedStrategyCard` |
| At the start of the 'Announce Retreats' step of space combat, if you are the defender | `StartOfAnnounceRetreatsAsDefender` |
| At the start of the agenda phase | `StartOfAgendaPhase` |
| At the start of the first round of a space combat | `StartOfFirstSpaceCombatRound` |
| At the start of the strategy phase | `StartOfStrategyPhase` |
| At the start or end of a combat round | `StartOrEndOfCombatRound` |
| Before you assign hits produced by another player's SPACE CANNON roll | `BeforeAssigningSpaceCannonHits` |
| Before you assign hits to your ships during a space combat | `BeforeAssigningSpaceCombatHits` |
| Before you roll dice for ANTI-FIGHTER BARRAGE | `BeforeAntiFighterBarrageRoll` |
| During the agenda phase when an outcome would be resolved | `WhenOutcomeWouldBeResolved` |
| When 1 or more of your units use PRODUCTION | `WhenYourUnitsUseProduction` |
| When an agenda is revealed | `WhenAgendaRevealed` |
| When another player chooses a strategy card during the strategy phase | `WhenOtherChoosesStrategyCard` |
| When another player is elected as the outcome of an agenda | `WhenOtherIsElected` |
| When another player plays an action card other than 'Sabotage' | `WhenOtherPlaysActionCardNotSabotage` |
| When another player would perform a strategic action | `WhenOtherPerformsStrategicAction` |
| When one of your ships uses SUSTAIN DAMAGE during combat | `WhenYourShipSustainsDamage` |
| When you are elected as the outcome of an agenda | `WhenYouAreElected` |
| When you are negotiating a transaction | `WhenNegotiatingTransaction` |
| When you gain control of a planet | `WhenYouGainPlanet` |
| When you would return your strategy card(s) during the status phase | `WhenYouWouldReturnStrategyCards` |
| When your last ship in the active system is destroyed | `WhenYourLastShipIsDestroyed` |

| site | before (`consolidated-2026-10-25`) | after (`card-text-fields-to-enums`) |
|---|---|---|
| window table keyed by the sentence | `crates/ti4-engine/src/reactions.rs:302` (`BTreeMap<&str, Window>`) | `crates/ti4-engine/src/reactions.rs:303` (`BTreeMap<ActionCardWindow, Window>`) |
| `window_for` | `crates/ti4-engine/src/reactions.rs:629` | `crates/ti4-engine/src/reactions.rs:615` |
| `reachable` / `unmapped_windows` | `crates/ti4-engine/src/reactions.rs:1246`, `:1268` | `crates/ti4-engine/src/reactions.rs:1235`, `:1258` |
| `action_cards::is_component_action` | `crates/ti4-engine/src/action_cards.rs:462` | `crates/ti4-engine/src/action_cards.rs:462` |
| Keleres `has_component_action` | `crates/ti4-engine/src/factions/keleres.rs:662` | `crates/ti4-engine/src/factions/keleres.rs:662` |
| coverage ledger | `crates/ti4-engine/src/registry.rs:120` | `crates/ti4-engine/src/registry.rs:123` |
| host/web timing groups (`timing_of`, display) | `crates/ti4-content/src/action_cards.rs:79` (a second copy of ~50 sentences) | `crates/ti4-content/src/action_cards.rs:82` (matches the variants) |

`unmapped_windows` keeps its `BTreeMap<String, usize>` signature (it reports `printed()`), and
`window_table()` changes only its key type; its one caller outside the engine
(`ti4-server/src/session/bluff.rs`) reads `.values()`.

### 2. Agenda `target` -> `ElectType` (I07)

9 distinct printed targets. The variants are the outcome sets the engine offers.

| printed target | variant | engine outcomes (unchanged) |
|---|---|---|
| For/Against | `ForAgainst` | for, against |
| Elect Player | `Player` | every seated player |
| Elect Planet | `Planet` | every controlled planet |
| Elect Non-Home Planet Other Than Mecatol Rex | `NonHomeNonMecatolPlanet` | controlled planets, not a home world, not Mecatol Rex |
| Elect Law | `Law` | laws in play |
| Elect Law (When this agenda is revealed, if there are no laws in play, discard ...) | `Law` | laws in play (an empty ballot is already discarded) |
| Elect Scored Secret Objective (When this agenda is revealed, if there are no scored secret objectives, ...) | `ScoredSecretObjective` | none: discarded on reveal (audit I03) |
| Elect Strategy Card | `StrategyCard` | none: discarded on reveal (audit I02) |
| When this agenda is revealed, the speaker draws the next card ... (Covert Legislation) | `HiddenNextAgenda` | for, against (audit I04) |

Counts: ForAgainst 27, Player 20, Planet 10, NonHomeNonMecatolPlanet 1, Law 2, ScoredSecretObjective
1, StrategyCard 1, HiddenNextAgenda 1. The three audit issues are kept as they are; the variants
name them so a later fix has a place to go.

| site | before | after |
|---|---|---|
| `vote::outcomes` (split at '(', substring Player/Planet/Non-Home/Law) | `crates/ti4-engine/src/vote.rs:82` | `crates/ti4-engine/src/vote.rs:83` |
| `vote::elects_planet` | `crates/ti4-engine/src/vote.rs:128` | `crates/ti4-engine/src/vote.rs:134` |
| Committee Formation (`starts_with("Elect Player")`) | `crates/ti4-engine/src/game.rs:5819` | `crates/ti4-engine/src/game.rs:5826` |
| bot `agenda_vote::outcomes_of` | `crates/ti4-rule-bot/src/agenda_vote.rs:1292` | `crates/ti4-rule-bot/src/agenda_vote.rs:1284` |
| bot `backward::agenda::agenda_kind` (now returns `(String, ElectType)`) | `crates/ti4-rule-bot/src/backward/agenda.rs:77` | `crates/ti4-rule-bot/src/backward/agenda.rs:75` |
| bot Dictate Policy bias (`target == "For/Against"`) | `crates/ti4-rule-bot/src/answers.rs:330` | `crates/ti4-rule-bot/src/answers.rs:332` |

### 3. Leader `abilityWindow` -> `LeaderAbilityKind` (I08)

63 distinct printed windows, 3 variants: `Action` ("ACTION:", 30 leaders; a component action that
ends the turn), `ActionPhase` ("During the action phase:", 2 leaders; offered on the owner's turn
without ending it) and `Other` (the other 61 windows, 71 leaders). The full 63-row table is
`LeaderAbilityKind::TABLE` (kept in byte order, checked by a test). `is_action_phase_ability()` and
`uses_the_action()` reproduce the two old tests (`trim_end_matches(':')` + `starts_with("ACTION")`
or `eq_ignore_ascii_case("during the action phase")`; and `trim().starts_with("ACTION")`).

| site | before | after |
|---|---|---|
| `leaders::is_action_window` | `crates/ti4-engine/src/leaders.rs:467` | `crates/ti4-engine/src/leaders.rs:466` |
| `leaders::uses_the_action` | `crates/ti4-engine/src/leaders.rs:483` | `crates/ti4-engine/src/leaders.rs:479` |

### 4. Technology `text` -> `ResearchRestriction` (I10)

`Researchable` or `NotResearchable`. The text is prose and is not tabled by value; the rule is:
a text that does not mention "cannot be researched" (ignoring case) is `Researchable`; a text whose
last line is exactly `This technology cannot be researched.` (and mentions it once) is
`NotResearchable`; any other mention is an `UnknownFieldValue` (the old substring test would have
matched it, so it must be looked at rather than guessed). A technology without text is
`Researchable`, as before. The table, by id: `NotResearchable` = {`parasite-obs`,
`planesplitter-obs`}; the other 100 technologies are `Researchable`.

| site | before | after |
|---|---|---|
| `technology::can_research` (the researchable check) | `crates/ti4-engine/src/technology.rs:1136` | `crates/ti4-engine/src/technology.rs:1134` |
| Deepwrought `deck` | `crates/ti4-engine/src/factions/deepwrought.rs:392` | `crates/ti4-engine/src/factions/deepwrought.rs:392` |

### 5. Strategy-card `name` -> `StrategyCardKind`, from the card id (I09)

The kind is mapped from the **id**, not the printed name. Every id of the corpus is tabled:

| id | source | printed name | kind |
|---|---|---|---|
| `base2` | base | Diplomacy | `Diplomacy` |
| `base4` | base | Construction | `Construction` |
| `pok1leadership` | base | Leadership | `Leadership` |
| `pok2diplomacy` | codex1 | Diplomacy | `Diplomacy` |
| `pok3politics` | base | Politics | `Politics` |
| `pok4construction` | pok | Construction | `Construction` |
| `pok5trade` | base | Trade | `Trade` |
| `pok6warfare` | base | Warfare | `Warfare` |
| `pok7technology` | base | Technology | `Technology` |
| `pok8imperial` | base | Imperial | `Imperial` |
| `te4construction` | thunders_edge | Construction | `Construction` |
| `te6warfare` | thunders_edge | Warfare | `Warfare` |

Since every id maps to the kind its printed name named, dispatching on the kind is the same as
dispatching on the name for every game (`the_strategy_card_kind_agrees_with_the_printed_name`). The
Thunder's Edge printings that differ are still told apart by id where they resolve
(`te4construction`, `te6warfare` in `strategy_cards::primary` and `strategy::secondary_question`),
as before. `StrategyCardKind::printed_name()` returns the shared name for the few places that print
it next to a kind (the `spenders` payload of the Leadership purchase details, which serialises the
same string as before).

`ti4_engine::strategy_cards::card_kind(content, id)` is the new lookup; `card_name` stays, documented
as display only (choice labels, prompts, host views). The printed name had been passed on as the
"card" of several internal hooks; those now carry `Option<StrategyCardKind>` (an unknown id, which
used to fall back to the id string and match nothing, is `None` and matches nothing):
`faction_abilities::{substitutes_primary, secondary_is_free, strategy_resolved}`,
`factions::Hooks::{substitutes_primary, secondary_is_free, strategy_resolved}`,
`factions::hooks_strategy::{StrategyHooks::secondary_waivers, secondary_waivers}` (Naalu, Mahact,
Winnu, Obsidian), `firmament_plots::{secondary_waivers, primary_instead_offered,
substitutes_primary}`, and `secondary_preview::follow_up_subtypes` (now takes a kind).

| site | before | after |
|---|---|---|
| `strategy_cards::primary` dispatch | `crates/ti4-engine/src/strategy_cards.rs:2653` | `crates/ti4-engine/src/strategy_cards.rs:2664` |
| `strategy_cards::follow` (Brilliant/Enervate substitution) | `crates/ti4-engine/src/strategy_cards.rs:2736` | `crates/ti4-engine/src/strategy_cards.rs:2757` |
| `strategy_cards::secondary` dispatch | `crates/ti4-engine/src/strategy_cards.rs:2760` | `crates/ti4-engine/src/strategy_cards.rs:2778` |
| `strategy::secondary_question` | `crates/ti4-engine/src/strategy.rs:144`, `:161` | `crates/ti4-engine/src/strategy.rs:146`, `:163` |
| `strategy::...::blocker` | `crates/ti4-engine/src/strategy.rs:406`, `:417` | `crates/ti4-engine/src/strategy.rs:406`, `:417` |
| `strategy::secondary_can_do_something` | `crates/ti4-engine/src/strategy.rs:707` | `crates/ti4-engine/src/strategy.rs:708` |
| other engine comparisons | `strategy.rs` (Leadership/Diplomacy/Trade details, token cost, eligibility), `factions/naaz.rs:859`, `diplomacy/builder.rs:424`, `diplomacy/candidates.rs:459`, `game.rs:3994`, `faction_abilities.rs:219,315,657`, `factions/firmament_plots.rs:114,146,169`, `secondary_preview.rs:124,281` | same functions, `card_kind` / `StrategyCardKind` |
| host draft-ahead rule (`name != "Trade"`) | `crates/ti4-host/src/view/secondary.rs:192` | `crates/ti4-host/src/view/secondary.rs:198` |
| rule bot | `lib.rs:980` (`card_is` by name), `lib.rs:3051` (`r1_card`), `lib.rs:3086` (draft scores), `objectives.rs` (card scores, Imperial), `faction_plans.rs` (`Plan::cards`), `backward/*` (`Steer::cards`, `Steer::play`, `PrepInfo`, `Verdict::cards`/`plays`) | `lib.rs:986`, `lib.rs:3085`, `lib.rs:3134`; the bot's plan data now holds `StrategyCardKind` instead of name strings |

The bot's map of wanted cards (`Verdict::cards`) is now keyed by the kind instead of the name. It is
only ever read with `get`, and the sums into it run in plan order, so the key order does not reach a
decision. Bot reason strings that named a card print `printed_name()`, the same text.

#### The Diplomacy discrepancy (a separate, gated rules fix; not changed here)

The base game set (`strategy_card_sets.json` `base_game`, source `base`) deals `base2`. Its printed
primary ends "Then, ready each exhausted planet you control **in that system**." The Codex printing
`pok2diplomacy` (used by the PoK, Codex I and Thunder's Edge sets) reads "Then, ready up to 2
exhausted planets you control." The engine dispatched on the shared printed name "Diplomacy", so
both resolve the Codex rule: `diplomacy_primary` ends with
`ready_planets(state, content, sources, galaxy, table, player, 2)`
(`crates/ti4-engine/src/strategy_cards.rs`, end of `diplomacy_primary`). The secondaries agree
(ready up to 2 exhausted planets), so only the base primary differs.

This change keeps that: `base2` maps to `StrategyCardKind::Diplomacy`, exactly as its name did. A
fix would give base Diplomacy its own behaviour (for example a `StrategyCardPrinting` tag, or a
`base2` id check in `diplomacy_primary` like the `te4construction`/`te6warfare` ones, that readies
every exhausted planet the player controls in the chosen system). It changes games dealt the
`base_game` set, so it needs a RULES_VERSION gate (old games replay with the Codex rule) and a
behavioural-baseline bump through `crates/ti4-sim/src/behavior.rs`; it is for the user to decide.

`base4` (base Construction) and `pok4construction` differ only in wording of the secondary ("Place 1
token from your strategy pool in any system" vs "Spend 1 token from your strategy pool and place it
in any system"); the rule is the same, so there is no discrepancy there.

## Extras found beyond the audit's five

Searched: every `text("...")` of a content record compared with `==`, `!=`, `contains`,
`starts_with`, `ends_with`, `eq_ignore_ascii_case`, `matches!` or `split` in `ti4-engine`,
`ti4-host` and `ti4-rule-bot` production code, and every strategy-card name literal.

- **Converted: Crash Landing's "not Mecatol Rex" by printed planet name.**
  `crates/ti4-engine/src/action_cards.rs:1975` compared `record.text("name") != Some("Mecatol Rex")`
  for the planets it may land on. Now `crates/ti4-engine/src/action_cards.rs:1976` uses
  `seating::is_mecatol_planet(id)` (ids `mr`, `mrte`). Identical on the corpus: the planets named
  "Mecatol Rex" are exactly `mr` and `mrte`
  (`seating::tests::the_planets_named_mecatol_rex_are_the_mecatol_ids`), and an id the corpus does
  not know is still kept.
- **Converted: the display timing groups.** `ti4_content::action_cards::timing_of` (host and web
  grouping only) carried its own copy of ~50 printed windows; it now matches `ActionCardWindow`
  variants, so the sentences exist once.
- **Not converted (categorical gameplay fields, not prose), listed for the generator:**
  agenda `type` (`Law`/`Directive`; compared both as `== Some("Law")` and with
  `eq_ignore_ascii_case("law")`, audit I16), secret-objective `phase` (`Status`/`Action`/`Agenda`,
  lower-cased with a silent default to status in `secrets::timing`), leader `type`
  (agent/commander/hero), explore `resolution` (`Fragment`/`Attach`/...), tile `tileBack`, action
  card `phase` (display grouping). Each is a small closed set that the generator can emit as an enum
  with the same loud-failure rule.
- **Not converted (display or test only):** `reaction_preview::PREVIEWED` and `preview_for`
  (printed action-card names; audit I18), host `view::reaction_modes::is_action_card_name`, the
  name-based card lookups in tests, `invasion::breakthrough_card` (first sentence of the text, an
  offer-card header).

## What the generator takes over

Branch `card-generator` is building `cargo xtask gen-cards` (per-category id enums and typed data
for a future standalone `ti4-engine-core`). The enums here are shaped so it can emit them:

- **Names.** `ActionCardWindow`, `ElectType`, `LeaderAbilityKind`, `ResearchRestriction`,
  `StrategyCardKind`: one enum per field, named after the field's meaning, not after a category id
  enum, so they do not clash with the generated `ActionCard`, `Agenda`, `Leader`, `Technology`,
  `StrategyCard` id enums.
- **A committed value mapping per field.** What the generator needs as input, beside the JSON, is
  the table that `card_fields.rs` holds today: for each of the five fields, `printed value -> variant
  name` (`ActionCardWindow::TABLE`, `ElectType::TABLE`, `LeaderAbilityKind::TABLE`,
  `StrategyCardKind::TABLE`), plus the one rule-based mapping (`ResearchRestriction::parse`, or a
  per-id table `technology id -> restriction`). Moving them to a data file (for example
  `crates/ti4-content/mappings/card_text_fields.json`) lets the generator emit the enums, the
  `TABLE` constants and the per-record values; the tests in `card_fields::tests` become the
  generator's staleness check (every corpus value mapped, no dead row, unknown value fails).
- **Order.** `ActionCardWindow` is declared in the byte order of the printed text and tested so;
  the generator must keep that (a `BTreeMap` keyed by it iterates as the string-keyed one did).
  `StrategyCardKind` is in initiative order.
- **Per-record storage.** Today `Record` carries the mapped value (`CardFields`). With generated
  typed records the value becomes a field of the generated per-category struct
  (`ActionCardData::window`, `AgendaData::elects`, `LeaderData::ability`,
  `TechnologyData::research`, and `StrategyCardData::kind`, derivable from the id enum), and the
  `Record` accessors become thin forwards or go away.
- **The Diplomacy printing.** If the gated fix above is taken, the generator should also emit the
  printing of a strategy card (base, Codex, Prophecy of Kings, Thunder's Edge) so the rules can tell
  `base2` from `pok2diplomacy` without an id literal.

## Verification

Run on this branch (base `consolidated-2026-10-25`, RULES_VERSION 29, unchanged):

- `cargo t-web` (model, content, calc, engine, host, rule-bot, server, wasm; lib and integration
  tests): all pass (content 164, engine 3919 + 6, host 297 + 2, rule-bot 190, server 58 + 321,
  wasm 9 + 38, model 94, calc 18).
- `cargo wire-web` (rewrites the generated TS wire types and the protocol and catalog fixtures):
  no file changes.
- `cargo l-web`: no new warnings (the same pre-existing ones as on the base).
- `tools/rulebot/selfplay.sh bench`, `bench-v1`, `bench-v2`, `bench-v3`, `bench-v4`, `bench-v5`:
  each 200 identical, 0 different; no stuck game, engine error or panic.
- `cargo test -p ti4-selfplay -p ti4-sim`: all pass except
  `ti4-sim profile::tests::fixture_capture_is_deterministic`, which needs the gitignored
  `out/pools/full_np8_12_holdout.json` (absent on this machine, also in the main checkout).
- `cargo build -p ti4-wasm --target wasm32-unknown-unknown` with and without `--features ts`: builds.
- `cargo check --workspace --all-targets` minus the crates that link libtorch (`ti4-mlp`,
  `ti4-tensor`, `ti4-advisor`, `ti4-bot-agent`, `ti4-review`, `ti4-replayer`): builds. Those six
  use none of the changed APIs (grep for `card_name`, `card_kind`, `window_table`,
  `follow_up_subtypes`, `agenda_kind`, the hook names and `ContentError` variants).
