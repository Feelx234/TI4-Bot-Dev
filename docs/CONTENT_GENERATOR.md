# Content generator (`cargo xtask gen-cards`)

The generator keeps the planned engine core and its adapter in step with the content corpus. The
core is meant to know **no JSON, no id strings and no card text**: only an id enum per category with
readable variant names, typed gameplay tables, and hand-written exhaustive `match` effects. The
adapter (today's `ti4-engine`, later) owns the id <-> string tables, the display text and the prompt
strings. One generator run writes both sides from `crates/ti4-content/content/*.json`, so they cannot
drift apart.

Branch `card-generator` (from `card-usage-audit`), 2026-10-26. Background: `docs/CARD_USAGE_AUDIT.md`.
No engine behaviour changed: nothing in the workspace uses the generated code yet.

> **`crates/ti4-engine-core` on this branch is a placeholder.** The real core crate is designed on
> branch `engine-raw-core-design`. Until it exists, this crate holds only the generator's core
> output (`src/generated/`) and the frozen names (`names/`) so that the output compiles and is
> tested. The core's crate absorbs both directories unchanged. No existing crate depends on it.

## What one run writes

| output | default path | contents |
|---|---|---|
| core | `crates/ti4-engine-core/src/generated/` | `mod.rs`, `values.rs` (value enums and the small shared types `Roll`, `Production`, `Prerequisites`, `Anomalies`, `Requirement<T>`), one `<category>.rs` per category: the id enum, `<Type>Data` and the table `<TYPE>_DATA`. **No string literals** (a test checks it). |
| adapter | `crates/ti4-engine/src/generated/` | `mod.rs` (the `IdStr` trait and the `IdRef` escape hatch), `values.rs` (key strings of the value enums), one `<category>.rs` per category: `<TYPE>_IDS` (id per variant), `impl IdStr`, `<Type>Text` and `<TYPE>_TEXT` (display fields, verbatim), `text(id)`; plus `text_parsed_fields.md` (the report below). Not wired into `ti4-engine` (no `mod generated;` there); compiled and tested by `crates/ti4-engine-core/tests/adapter.rs`. |
| names | `crates/ti4-engine-core/names/<category>.toml` | the frozen id -> variant name mapping, one file per category |

Every path is a parameter:

```text
cargo xtask gen-cards                  # write (only files whose text changed)
cargo xtask gen-cards --check          # regenerate in memory, exit 1 listing every stale file
cargo xtask gen-cards --content DIR --core-out DIR --adapter-out DIR --names DIR \
                      --core-path ti4_engine_core::generated --only units,planets
```

`cargo xtask` is an alias in `.cargo/config.toml` (`run -q -p xtask --`). A run takes ~0.25 s once
the xtask binary is built. Generated files start with a `@generated ... DO NOT EDIT` header naming
the command; the module declarations carry `#[rustfmt::skip]`, so `cargo fmt` leaves them alone.

### Categories

1,390 ids in 20 categories: the 15 card categories of the audit, `tokens` (exploration cards place
them), and `factions`, `planets`, `systems`, `units`.

| category | enum | count | repr | typed gameplay columns (core) | display fields (adapter) |
|---|---|---|---|---|---|
| abilities | `Ability` | 73 | u8 | `source` | name, faction, window, windowEffect, permanentEffect |
| action_cards | `ActionCard` | 142 | u16 | `source`, `component_action` | name, window, phase, text, automationID |
| agendas | `Agenda` | 63 | u8 | `source`, `kind: AgendaType`, `elects: AgendaElects` | name, target, text1, text2, mapText |
| attachments | `Attachment` | 22 | u8 | `source`, `resources`, `influence`, `specialties`, `traits`, `space_cannon` | name, isFakeAttachment |
| breakthroughs | `Breakthrough` | 31 | u8 | `source`, `faction`, `synergy` | name, text |
| explores | `Explore` | 80 | u8 | `source`, `deck`, `resolution`, `attachment`, `token` | name, text, flavorText |
| galactic_events | `GalacticEvent` | 20 | u8 | `source` | name, text |
| genericcards | `GenericCard` | 10 | u8 | `source` | name, text, cardType |
| leaders | `Leader` | 103 | u8 | `source`, `faction`, `kind`, `window: LeaderWindow`, `replaces` | name, title, abilityName, abilityWindow, abilityText, unlockCondition |
| promissory_notes | `PromissoryNote` | 40 | u8 | `source`, `faction`, `play_area` | name, text |
| public_objectives | `PublicObjective` | 40 | u8 | `source`, `points` | name, text, phase |
| relics | `Relic` | 24 | u8 | `source`, `fake` | name, text |
| secret_objectives | `SecretObjective` | 40 | u8 | `source`, `phase`, `points` | name, text |
| strategy_cards | `StrategyCard` | 12 | u8 | `source`, `kind: StrategyCardKind`, `initiative` | name, primaryTexts, secondaryTexts |
| technologies | `Technology` | 102 | u8 | `source`, `types`, `prerequisites`, `faction`, `base_upgrade`, `researchable` | name, text |
| tokens | `Token` | 28 | u8 | `source`, `wormholes`, `space` | aliasList, tokenPlanetName |
| factions | `Faction` | 34 | u8 | `source`, `home_system`, `home_planets`, `commodities`, `starting_tech`, `starting_tech_options`, `starting_tech_amount`, `faction_tech`, `abilities`, `leaders`, `promissory_notes`, `units` | factionName, shortName, shortTag, complexity, startingFleet |
| planets | `Planet` | 159 | u16 | `source`, `resources`, `influence`, `traits`, `specialties`, `legendary`, `system`, `homeworld_of`, `space_cannon` | name, shortName, legendaryAbilityName, legendaryAbilityText, flavourText, aliases |
| systems | `System` | 231 | u16 | `source`, `planets`, `wormholes`, `anomalies`, `hyperlane`, `tile_back`, `home_system` | name, aliases |
| units | `Unit` | 136 | u16 | `source`, `base_type`, `faction`, `cost_halves`, `movement`, `capacity`, `capacity_used`, `combat`, `anti_fighter_barrage`, `bombardment`, `space_cannon`, `production`, `ship`, `ground_force`, `structure`, `space_only`, `sustain_damage`, `planetary_shield`, `deep_space_cannon`, `disables_planetary_shield`, `can_be_direct_hit`, `upgrades_to`, `upgrades_from`, `required_technology`, `replaces` | name, subtitle, ability, asyncId |

The gameplay columns are the `gameplay` (and `scope`) fields of the audit, plus typed forms of the
text-parsed fields where the engine's rule is mechanical. References to other categories are typed
(`Option<Attachment>`, `&'static [Technology]`). The column docs in the generated code name the JSON
field each comes from. Display fields are the `ui` and `engine-label` fields the audit found read,
plus the raw text of the parsed fields; fields nobody reads (flavour of most cards, image URLs,
upstream notes, Twilight's Fall printings) are not generated.

Value enums (`values.rs`): `Source`, `FactionTag`, `TechType`, `AgendaType`, `AgendaElects`,
`LeaderType`, `LeaderWindow`, `ObjectivePhase`, `ExploreType`, `ExploreResolution`,
`StrategyCardKind`, `PlanetTrait`, `Wormhole`, `TileBack`, `UnitBaseType`. Their keys are collected
from the corpus (a new key adds a variant, which makes every exhaustive `match` on it fail to compile
until handled: wanted). `FactionTag` is the faction as card, unit and planet records spell it: a
faction id, or a family tag (`keleres`) with `FactionTag::factions()` listing what it stands for, or
a homebrew leftover with no faction (`ghemina`, two units).

### Each id enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum ActionCard { AncientBurialSites = 0, ArchaeologicalExpedition = 1, /* ... */ }
impl ActionCard {
    pub const COUNT: usize = 142;
    pub const ALL: [Self; Self::COUNT];
    pub const fn index(self) -> usize;
    pub const fn from_index(index: usize) -> Option<Self>;
    pub fn data(self) -> &'static ActionCardData;     // ACTION_CARD_DATA[self as usize]
}
```

- **Order**: variants are declared in byte order of the id (`str`'s `Ord`), so integer order is
  string order: `BTreeMap`/`BTreeSet` iteration and `deck::ids` (sort, then a seeded shuffle) keep
  their order, and seeded games and golden digests do not move when strings become enums. Tested
  for every pair of every category.
- **repr**: `u8` when the enum has room to double (`COUNT * 2 <= 256`), else `u16` (action cards,
  planets, systems, units).
- **One enum per category**: the 36 ids spelled the same in two categories (`crisis`, `mr`, `ps`,
  `nanoforge`, ...) are different variants of different enums.

In the adapter, `IdStr` gives `as_str()` (array index) and `from_id(&str)` (binary search over the
sorted ids, no hashing, no allocation) for every id enum and value enum. `FromStr`/`Display` cannot
be implemented there for the core's types (orphan rule); the core crate can add them if it wants
strings after all, or the adapter keeps the trait.

## Variant names

Readable names derived from the printed `name` (`factionName` for factions), frozen once written
(`crates/xtask/src/content_gen/naming.rs` has the rules as code and tests):

1. **Source**: the printed name; the id when the record has none or the cleaned name does not start
   with an ASCII letter (`2RAM` -> the id).
2. **Cleanup**: `Ω` -> `Omega` (and a few Greek letters), accented Latin letters -> the bare letter,
   any other non-ASCII dropped; apostrophes deleted inside the word (`Z'eu` -> `Zeu`,
   `Coup d'Etat` -> `CoupDEtat`); every other non-alphanumeric character separates words; each word
   gets an upper-case first letter, the rest is kept as printed (`Hel-Titan II` -> `HelTitanII`,
   `X-89 Bacterial Weapon Ω` -> `X89BacterialWeaponOmega`, `I.I.H.Q. Modernization` ->
   `IIHQModernization`).
3. **Records that clean to the same name** (copies, reprints) all get a suffix: (a) the trailing
   digits of the id when every member has some and they differ (`sabo1` -> `Sabotage1`); else (b)
   when one id is a prefix of all the others, the rest of each id (`veto`, `veto3` -> `Veto`,
   `Veto3`; `xxchahero-te` -> `XxekirGromTe`; `bioticstat` -> `BioticResearchFacilityStat`); else
   (c) the whole id (`base2` -> `DiplomacyBase2`).
4. Still not starting with a letter: prefixed `N` (`0.0.0` -> `N000`, tile `06` -> `N06`).
5. **Frozen**: an id with an entry in `names/<category>.toml` keeps it forever. The generator only
   adds entries for new ids (in id order, so the diff is one added line per record) and never renames.
   An id that leaves the corpus keeps its entry, so its name stays reserved; the run prints a note.
6. **Collisions fail**: two ids of one category with one name (including a reserved one) stop the
   generator with the ids and a request for an override.

Value-enum variants are not frozen: they are the key in CamelCase (all-caps keys title-cased first:
`UNITUPGRADE` -> `Unitupgrade`, `For/Against` -> `ForAgainst`), and a collision fails the same way.

### Overrides

An override is an edit of the names file, nothing else:

```toml
"pok2diplomacy" = "DiplomacyCodex"  # override: Codex I printing; derived DiplomacyPok2diplomacy
```

Then run `cargo xtask gen-cards` and fix the compile errors where the old name was used. The
trailing comment is kept by the generator. The committed files carry seven overrides, all in
`strategy_cards.toml` (`DiplomacyBase`, `DiplomacyCodex`, `ConstructionBase`, `Construction`,
`ConstructionThundersEdge`, `Warfare`, `WarfareThundersEdge`), because rule 3(c) gives id-suffixed
names to the strategy-card printings. Names that are correct but not pretty and could be overridden
the same way: `N06` (the L1Z1X home tile, printed `[0.0.0] - L1Z1X`), `SuffiAnMentakagent` /
`SuffiAnKelerescommander`, `ZGravEidolonAbsolNaazMechSpace`, `MecatolRex112` / `MecatolRex18`.

## Adding things

**A card (or any record)**: add the record to the JSON, run `cargo xtask gen-cards`, commit the JSON,
the names file (one new line) and the regenerated files. The new variant exists in the core enum and
in every table; the compiler then points at every exhaustive `match` (effects, windows, predicates)
that needs an arm for it. Writing that arm is the hand-written part: the generator never generates
effects. A record with a new value (a new wormhole kind, a new agenda target shape) adds a variant to
the value enum, and the `match`es on that enum fail to compile the same way.

**A gameplay column**: add a `col(...)` to the category in `crates/xtask/src/content_gen/spec.rs`
(`Conv::Flag`, `Int`, `Value`, `Ref`, or `Custom` with a function), add the check to
`crates/ti4-engine-core/tests/adapter.rs`, run the generator.

**A category**: add a `Cat` to `spec::categories()` (file stem, enum name, id field, name field,
columns, display fields), run the generator, add `ids_agree::<NewType>(...)` and a `*_agree` test.

## `--check` and the tests

`--check` runs the whole generator in memory (including the names files, as if it were going to
append) and compares every output with the file on disk; a missing, different or stray file (a
`*.rs`/`*.md` in an output directory that the run does not produce, e.g. of a removed category) fails
with the list. It never writes.

| test | where | what |
|---|---|---|
| `generated_files_are_current` | `crates/xtask/tests/gen_cards.rs` | `--check` against the committed files |
| `a_rerun_is_a_no_op` | same | two runs give identical files and no new names |
| `adding_a_record_appends_one_name_and_renames_nothing` | same | on a scratch copy: one record added -> exactly one names line added, all old lines unchanged, the variant sits at its byte-order place, writing and re-running is clean |
| `a_name_collision_fails_and_asks_for_an_override` | same | collision error names both ids; an override resolves it |
| `an_overridden_name_is_kept`, `check_reports_a_stale_file`, `the_core_output_has_no_string_literals` | same | as named |
| naming rules, file round trip, frozen/append, collision, retired ids | `crates/xtask/src/content_gen/naming.rs` | unit tests |
| `card_ids_agree_with_the_corpus`, `other_ids_agree_with_the_corpus` | `crates/ti4-engine-core/tests/adapter.rs` | per category: ids of `ContentStore` and of the enum in both directions; for every pair of variants integer order == byte order of the ids; id -> variant -> id for every record and variant -> id -> variant for every variant; the ids of `all_planets`, `all_systems`, `units::catalogue`, `factions::catalogue` |
| `*_agree` (one per category) | same | every gameplay column and every display field of every record against `Record` and the typed views (`galaxy::Planet`, `galaxy::System`, `is_home_system`, `home_systems`, `units::UnitType` including `production()`, `factions::Faction`), and the engine's text rules re-stated (`vote::outcomes`, `leaders::is_action_window`, `uses_the_action`, `cannot be researched`) |
| `value_enums_are_sorted_and_round_trip`, `ids_repeated_across_categories_stay_apart`, `an_id_outside_the_enum_takes_the_dynamic_path`, `faction_tags_name_their_factions`, `the_tables_are_indexed_by_the_variant` | same | as named |

Run: `cargo test -p xtask -p ti4-engine-core` (a few seconds once built). A bare `cargo test` of the
workspace runs them too. They are not in the `t-web` alias: adding packages there changes its
resolved feature set and rebuilds the engine (see the alias comment in `.cargo/config.toml`); add
`-p xtask -p ti4-engine-core` to the push gate instead, or add them to `t-web` once the core is a
dependency of the engine anyway.

Results on this branch: xtask 10 unit + 7 integration tests pass, ti4-engine-core 25 tests pass,
`ti4-engine --test content_ids_resolve` 4 tests pass.

### `content_ids_resolve` (fixed here)

`crates/ti4-engine/tests/content_ids_resolve.rs` cut every file at its first `#[cfg(test)]`, which
is often a test helper in the middle of a file (audit I13). It now blanks each `#[cfg(test)]` item
(to its `;`, or the brace closing its first `{`; strings, raw strings, chars and comments skipped;
line numbers kept), which scans ~12,800 more production lines; the existing rule finds no dangling id
in them. It skips `src/generated/` (the adapter tables are data). New test
`typed_id_literals_name_their_own_category`: a literal in `PlanetId::new("..")`,
`TechnologyId::new("..")`, `LeaderId`, `UnitTypeId`, `SystemId`, `ActionCardId`, `FactionId`,
`SecretObjectiveId`, `ObjectiveId` (public or secret), `StrategyCardId`, `BreakthroughId`, `RelicId`
in production code must be an id of that category; test-only files (`*_tests/`, `fixtures.rs`,
`#![cfg(test)]`) are skipped there. All pass today. The "any category" rule of the original test is
kept for the other contexts (`alias`, `.id()`), where the category is not known from the line.

## Text-parsed rules

The audit found five rules decided by parsing free text, and the extension adds two more. The
generated `crates/ti4-engine/src/generated/text_parsed_fields.md` lists, for each, what was typed
and what is left, with the values and ids to map:

| field | generated | left as TODO (in the report) |
|---|---|---|
| action card `window` | `component_action: bool` (exactly `Action`) | the reaction timing: `reactions::window_table` is engine knowledge; 52 distinct sentences listed for a future `Timing` enum |
| agenda `target` | `elects: AgendaElects` (text before `(`, minus `Elect`; else `ForAgainst`), exactly `vote::outcomes` | the parenthetical discard rule; Covert Legislation types as `ForAgainst` as the engine reads it (I04) |
| leader `abilityWindow` | `window: LeaderWindow` (`Action` / `ActionPhase` / `Other`) | - |
| technology `text` | `researchable: bool` | - |
| strategy card `name` | `kind: StrategyCardKind` | the printing variant (`te4construction`, `te6warfare` special-cased by id; `base2` vs `pok2diplomacy`) |
| faction `startingFleet` | - (display/adapter text) | the fleet grammar of `factions::parse_fleet` |
| unit `ability` | - | `fighter_support` ("up to N ... fighter"); candidates listed |

Also noted there: `UnitType::is_ground_force` decides by base type plus two Titans ids, not by the
printed `isGroundForce` flag that the core column carries.

## What is not static content (the escape hatch)

The enums hold the compiled-in corpus. Things that are ids at run time but not records of it:

- **Composite or derived ids**: `support:<faction>` (Support for the Throne), a generic note's bare
  alias (`cf`, `ps`; the corpus has `<color>_cf`), `alias:faction` note ids, laws keyed by alias in
  `GameState`.
- **Seats and players** (`PlayerId`): never content.
- **Planets and systems made in tests** (`invented_0`, `test-near`, `p`, `9{n}` ...) and a corpus
  loaded with `ContentStore::from_dir` that adds ids the compiled-in one lacks.
- **Newer printings**: `xxchahero-te`, `naalu_mech_omega` are records, so they are variants of their
  own; the lookup that prefers them (`ContentStore::resolve_id` with the `-te`/`_te`/`_omega`
  suffixes and the source scope) is adapter logic over the enums. The reprint relation is data where
  the corpus states it (`LeaderData::replaces`, `UnitData::replaces` from `homebrewReplacesID`).

Escape hatch: the adapter's `IdRef<'a, T>` is `Known(T)` or `Dynamic(&str)`; `IdRef::parse` never
fails. The core never sees `Dynamic`: the adapter keeps such ids as strings at the boundary (seat
names, composite note ids) or maps them to a per-game table. **Where the core needs a dynamic range**:
only for planets and systems if a game can create them (none can in the rules modelled today; only
tests do). The generated `COUNT` is the first free value, which is why planets and systems are
`u16` with room above `COUNT`; a core `PlanetRef(u16)` / `SystemRef(u16)` with values `>= COUNT`
indexing a per-game table would be the shape, decided by the core design. Test fixtures should move
to real corpus ids (or to that table) when the core takes them.

## Decisions

- **Adjacency is not generated.** It depends on the layout (`map_templates.json` positions, the
  game's hyperlanes and their rotations, `Galaxy` edits: swaps, Creuss gates, wormhole tokens,
  suppressed wormholes, extra links). The core output gives per-tile facts (`planets`, `wormholes`,
  `anomalies`, `hyperlane`); neighbours stay a per-game computation.
- **Decks are not generated**: deck composition (audit I11) is a rules decision, not a table.
- **Dangling references fail** unless named in `spec::DANGLING`. One today: unit `ghemina_carrier2`
  requires technology `dsghemcv` (a Discordant Stars leftover), typed
  `Requirement::NotInCorpus`.
- **`cost_halves: Option<u8>`** instead of a float: fighters and infantry cost 0.5, so halves are
  exact and the row stays `Eq`-able; `None` = no cost printed.
- **Value enums from the data, not hand-written**: `NONE` is dropped from technology types and
  synergy (empty list), technology types and specialties are upper-cased into one `TechType`.

## Not generated, deliberately

Effects (the hand-written `match` arms), reaction timings, the fleet grammar, adjacency, deck
composition, prompts and choice labels (they embed names and are part of the V2 decision
fingerprint; their strings stay in the adapter), fields nobody reads.

## Cost

- The placeholder core crate: ~550 KB of generated source (29k lines with the adapter), builds in
  ~0.6 s (dev) / ~0.8 s (release) after a touch. Nothing depends on it: `cargo tree -i
  ti4-engine-core --workspace` lists only itself, `cargo tree -p ti4-wasm` does not contain it, so
  the compile time and wasm size of every existing crate are unchanged (`Cargo.lock` gains only the
  new package; xtask gained no dependency).
- The adapter output is ~430 KB of source, not compiled into anything but its test.

## Remaining work (queue once the core exists)

1. **Core consumes the enums**: the core crate absorbs `src/generated/` and `names/`, and its
   state uses the enums instead of strings per `GameState` field (audit: `agenda_deck`, `laws`,
   `exploration_decks`, note ids are still strings).
2. **Port the engine `match` arms**: `action_cards::effect_for` (one arm per copy today; an
   `ActionCardKind` enum for the printed card, generated from the name groups, would collapse the
   copies), `agenda_effects::resolve_with`, `relics::use_relic`, `objectives::requirement_for` /
   `cost_of`, `secrets.rs`, `leaders::dispatch_leader`, `exploration::resolve_instant`,
   `strategy_cards` (on `StrategyCardKind`), faction modules; replace the text rules with the typed
   columns (`elects`, `window`, `researchable`, `component_action`, `kind`).
3. **Adapter wiring**: `mod generated;` in `ti4-engine` (or wherever the adapter lands), serde of the
   enums as their id strings (wire, saves, replays and fingerprints unchanged), `IdRef` at the
   boundaries that see dynamic ids, `resolve_id` over the enums.
4. **Display split**: offer cards and payloads carry ids, the host joins `<Type>Text`; keep prompt
   text byte-identical or version the fingerprint (audit stage 3, 4); a separate display asset for
   the wasm page.
5. **Planets / systems / units / factions in the engine**: replace `galaxy::Planet`/`System`,
   `units::catalogue`, `factions::Faction` lookups with the tables (the content-lookup cost the
   optimization study measured); decide the dynamic planet/system range (above).
6. **Enums for the text-parsed fields in the content JSON itself**: once the engine reads the typed
   columns, the corpus could carry `timing` (action cards), `elects` (agendas), `timing` (leaders),
   `researchable` (technologies), the strategy-card printing tag and a structured starting fleet,
   with the prose kept as display text; the generator then reads the field instead of re-stating the
   engine's parse (the TODO rows of `text_parsed_fields.md`).
7. **Tests to add with the core**: a test that every `window` sentence maps to a `Timing`, that
   `elects` covers every `AgendaElects` value in `vote::outcomes`, and the golden games
   (`tools/engine_profile/verify_benches.sh`) as the gate for every port.
