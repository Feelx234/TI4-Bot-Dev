# A typed engine core under the string engine: design, prototype and the first five stages (2026-10-26)

The idea (the user's): a new crate, `ti4-engine-core`, that knows **no string ids at all**. Inside it,
factions, systems, planets, units, technologies, cards, objectives and seats are compact typed ids
(u8/u16 newtypes, enums), and decisions, options and prompts are enums. The current `ti4-engine`
becomes a wrapper around it that translates string ids <-> typed ids at the boundary, so everything
outside (wire format, option ids like `<id>|r<k>`, saves, replays, decision hashes, content JSON,
benches, TypeScript types, bots, UI, wasm API) sees exactly today's behaviour.

Branch `engine-raw-core-design`, based on `engine-optimization-study` (`8f2d03e3`, which has the
`Arc<str>` ids of stage 0 and the profiling tools). Inputs: `docs/ENGINE_OPTIMIZATION.md` (profile,
id inventory, ids plan stages 0-4) and `docs/CARD_USAGE_AUDIT.md` (branch `card-usage-audit`: how
every card field is read, 36 ids shared by two categories, the typed-card proposal).

Overview page (private artifact): https://claude.ai/artifact/UMvrB8SYndBvGEc6wSjUJZ

**Measured** = a number from a run described in section 4 (reproducible with the tools listed there).
**Estimated** = derived from profile shares or from the prototype's ratios; always labelled.

## 0. The port so far (stages 1-5, landed 2026-10-26 evening)

The morning's study (sections 1-7 below, kept as written) recommended a staged strangler port. Five
stages of it landed the same day on this branch, one commit each, each passing the whole gate
(`tools/engine_core/verify_stage.sh`): the dual run (`core_equivalence`, extended per stage), the
model/content/engine/host/rule-bot/selfplay/core/bridge/xtask tests and wasm `--features ts`,
`cargo xtask gen-cards --check`, the wasm build, all six benches 200/200 with the core on and `bench`
200/200 with it off, and the 20-golden-game CPU A/B against `8f2d03e3` (3 interleaved runs).

| Stage | Commit | What moved to the core | Dual run added (0 differences each) | 20 golden games vs `8f2d03e3` |
|---|---|---|---|---|
| 1 | `8665470d` | `typed-core` on by default (off: `--features ti4-engine/string-movement`); every movement route search (`movable_into`, rift exits, ship routes, the real move's route, `Observed::can_reach`) | 7.3 M route queries, 93 k route sets, 1.5 M adjacency sets over 3 099 positions | -8.7% |
| 2 | `a0db38bc` | per-seat typed movement rules retargeted per destination (`core_adapter::SeatReach`) and a per-seat fleet (`tactical::SeatFleet`) in `Observed`: the bot stops building `MovementRules` per destination | 1.09 M field-by-field rule comparisons, 14.9 M `Observed` answers | **-25.3%** |
| 3 | `b8eb5e39` | merge of `card-generator`: the core runs on the generated `System`/`Unit`/`Planet` enums and rows; ids from the generated adapter tables; prototype `build.rs` and `names` feature removed | (same counts, on the generated tables) | -26.4% |
| 4 | `389d1be6` | LRR 60 neighbours (`transactions::present_together`) on the core's adjacency | 492 k neighbour answers (each seat pair, six map-switch variants) | -25.6% (inside the noise of stage 3) |
| 5 | `f8434afd` | printed planet values, technology specialties, space stations from the generated rows (per-scope resolution tables; run-time content stays on the record path) | every planet and system id x BASE/POK/FULL | -29.4% |

**Cumulative, measured on the final commit** (`f8434afd`, CPU time):

| Workload | `8f2d03e3` | final | Change |
|---|---:|---:|---:|
| 20 golden games (`profile-20.json`), median of 5 interleaved runs, 4 workers | 50.0 s | 36.5 s | **-27.0%**, 20/20 hashes |
| 10 golden 4p games, 1 worker, user time; peak RSS | 17.6 s; 59.0 MB | 12.5 s; 56.9 MB | -29%; -3.5% |
| six benches, sum of CPU (not interleaved, 6 workers, shared machine) | 2 751 s (morning run) | 2 233 s | about -19% |
| 4 golden 4p games with views (`engine_profile game`, 2 runs each, core off vs on in the same build) | bot 4.96 s, views 6.74 s, replay 3.67 s | bot 3.04 s, views 6.48 s, replay 3.63 s | bot -38%, views -4%, replay ~-1% (noise); view JSON identical size (299.9 KiB per boundary) |
| solo bench native (random seats, no bot), 6 runs each, core off vs on | 1.60 s median | 1.62 s median | 0% (noise) |
| wasm (`wasm-release` + `wasm-opt` as `build-wasm.mjs`) raw / gzip / brotli | 5 857 817 / 1 829 366 / 1 313 584 B | 5 933 830 / 1 859 210 / 1 332 503 B | +76 KB (+1.3%) / +29.8 KB (+1.6%) / +18.9 KB (+1.4%) |
| compile, release `ti4-engine` lib after touching `lib.rs` | 86.4 s | 93.8 s | +8.6% (one run each, shared machine); `ti4-engine-core` with 550 KB of generated source: 1.2 s |

The wasm size gate (`scripts/wasm-size-gate.mjs`) **fails before and after**: its gzip and brotli
budgets (1.80 / 1.30 MB) were already exceeded at `8f2d03e3`; the core adds 30 KB gzip. The budget was
not raised here (a decision for the user).

**Profile after stage 2** (perf, frame pointers, 10 golden 4p games): the bot's movement/reach bucket
fell from 27.5% to **3.7%** of a game. The top items now: bot other decision logic 24.8%, objective
evaluation 20.3% (`Observed::revealed_objective_progress_imagining` 15.8% inclusive), automatic
engine steps 17.9%, checkpoints 17.7% (`Resolver::checkpoint` 9.6%, `BTreeMap` clones of logs),
content lookups (`ContentStore::get` + `resolve_id`, 11% inclusive, spread over planets, units and
technologies), memcmp 18% of leaf samples.

**What is left, in the order the profile suggests** (each its own stage with its own dual run):

1. Objective evaluation onto typed board/control data (est. -8..-12%). Objectives are card records;
   wait until `card-p1-fixes` and `card-text-fields-to-enums` are merged.
2. Checkpoints without whole-log copies (ENGINE_OPTIMIZATION #8; not a core change; est. -8..-12%).
3. Unit and technology content lookups on the generated rows (est. -3..-5%; technology waits for the
   card branches, `technology.rs` is in their diff).
4. The string search in `MovementRules` can be deleted after a nightly with the core on; the
   `string-movement` switch then goes too.
5. Stage "core owns the state" (the morning's stage 5) was not started, as instructed.

**Merge notes**: stage 1 changes `Game::default_route` in `game.rs` (one call) and stage 2
`choice.rs`/`tactical.rs`; `card-p1-fixes` also edits `game.rs`, so a small textual conflict is
possible there. Stage 5 edits `production.rs`, `planets.rs`, `space_stations.rs`; none of the
card-branch files (`reactions.rs`, `vote.rs`, `leaders.rs`, `technology.rs`, `strategy_cards.rs`,
`deck.rs`, `agenda_effects.rs`, laws, `state.rs`) is touched. No `RULES_VERSION` change, no wire,
TS type, view or fixture change (`cargo test -p ti4-wasm --features ts` checks the generated wire
types and fixtures).

## Summary (the study, morning of 2026-10-26)

- **Feasible, and the prototype says the typed representation pays where it is used**: a typed
  movement slice (`crates/ti4-engine-core`: typed ids, generated content tables, bitset adjacency, the
  route search) answers **exactly** what the string engine answers (dual run: 3 099 positions, 7.3 M
  route queries, 1.5 M adjacency sets, 93 k route sets, 0 differences) and its search is **~16x
  faster** than the string search on the same rules (measured, timed phase only), **~5x** faster
  including building the rules through the string engine and translating them (the "adapter" stage).
- **End to end, measured**: routing only the bot's three reach questions (`movable_into`,
  `Observed::can_reach`, `route_rift_exits`) through the typed core behind a feature flag saves
  **9.6% CPU of a golden bot game** (20 golden games, 3 interleaved runs, every hash identical;
  all six benches 200/200 identical with the flag on, see 4.4). The rest of the movement bucket stays
  string-bound: building `MovementRules` through faction hooks (`with_laws`, 6.8% of a game) and the
  board facts. Porting those hooks too: est. another -6..-10%.
- **The whole-engine version is a rewrite, not a refactor.** The ids are not concentrated in a
  boundary layer: 95% of the engine's 6 073 production id mentions and 718 of its 736 option-building
  sites sit inside rules modules (`factions` alone 41.9k lines), interleaved with the rules; 55 of
  75 modules form one dependency cycle; `GameState` (106 fields + 70 per player) is defined in
  `ti4-model` and read field by field by the host (177 mentions), server, review, replayer, wasm and
  selfplay. And the engine moves fast: 243 commits and +43.8k lines in `ti4-engine` in the last 14 days.
- **Recommendation: do it in stages, area by area (strangler), not as a big-bang crate swap**, and
  only where the profile says it pays: movement/adjacency first (this prototype, ready to finish),
  then the board/occupancy queries and the bot's objective evaluation, then typed content records.
  Keep `GameState` as the save/wire schema for now; let the typed core own derived, rebuildable data
  (adjacency, occupancy, content tables) before it owns state. Decide at the stage-3 gate whether
  owning the state (the real "typed core") is worth its cost. Details and decisions in section 5.

| | today (`engine-optimization-study`) | measured with the slice | estimated, full typed core |
|---|---|---|---|
| Golden bot games, CPU | 49.4 s (20 games) | **44.7 s (-9.6%)**, 20/20 hashes | -35..-55% (bots ported to the typed view), -20..-35% (bots on the string view) |
| Reach search alone (1.25 M questions x 3) | 7 998 ms | **492 ms (16.2x)**, same answers | (same) |
| Reach incl. building the rules | 9 009 ms | **1 701 ms (5.3x)** through the adapter, same answers | ~10-15x when hooks are typed (no string rules built) |
| Memory | `Unit` 40 B, `MovementRules` 632 B | `CoreRules` 400 B, adjacency 7 392 B per board | typed state est. 4-8x smaller (unit 40 -> ~6 B, sets as bitsets) |
| wasm size | 7 147 998 B (stage 0) | +49 313 B (+0.7%, `wasm-release` before `wasm-opt`) with the slice compiled in | -5..+10% at the end (two engines in the binary during the migration: up to +30%) |
| Compile, release `ti4-engine` | 153 s, one codegen unit, one 55-module cycle | core crate 1.2 s (build script + crate, release) | an acyclic typed core in several crates could cut the critical path est. 30-50%; speculative |

## 1. Architecture

### 1.1 Crates and dependency direction

```text
            consumers (unchanged API): ti4-host, ti4-server, ti4-wasm, ti4-rule-bot, ti4-selfplay,
            ti4-policy, ti4-training, ti4-sim, ti4-review, ti4-replayer, ...
                         |  string ids, Choice/ChoiceOption, GameState, DecisionRecord
                         v
   ti4-engine  (the adapter: today's public API, wire types, id <-> string tables, display text,
                option-id grammar, labels, prompts, payloads, details, fingerprints, saves/replays)
          |                      |                          |
          v                      v                          v
   ti4-model (GameState,    ti4-content (JSON corpus,    ti4-engine-core (standalone: id enums,
   ids, wire schema)        loader, display data)        typed data tables, state, rules, effects
                                 |                        as exhaustive matches, decisions as enums;
                                 |                        no JSON, no id strings, no card text)
                                 v                                  ^
            cargo xtask gen-cards (branch card-generator) ---------+---> adapter name/text tables
            one run writes both outputs, committed; --check fails when stale
```

- `ti4-engine-core` is **standalone**: no runtime dependencies, no JSON, no id strings, no card
  text. It holds per-category id enums, typed gameplay-data tables, the state and the rules, with
  card effects as hand-written exhaustive `match` arms over the enums.
- **The generator** (`cargo xtask gen-cards`, being built on branch `card-generator`; not duplicated
  here) reads `crates/ti4-content/content/*.json` and writes, in one run, the core's generated source
  (enums + typed data, no strings; assumed under `crates/ti4-engine-core/src/generated/`) and the
  adapter's name and text tables. Both outputs are committed; a `--check` test fails when they are
  stale. Variant names come from the record name (`MecatolRex`, `GalacticCrisisPact`) through a
  committed id -> variant mapping file that is never renamed, plus overrides; variants are emitted in
  id text order, so integer order equals today's string order and derived `Debug` is readable.
- **The prototype predates the generator**: its `build.rs` reads the corpus through `ti4-content` (a
  build dependency) and emits index tables (`SystemIx(u16)`, ...) and, behind a `names` feature, the
  name tables. That is a stand-in with the same ordering contract; when the generator lands, the
  prototype's tables become `generated/` enums (`System::MecatolRex as u16` is the index the
  prototype uses), its `build.rs` and `names` feature are deleted, and the adapter takes the names
  from the generator's adapter tables.
- `ti4-engine` depends on `ti4-engine-core` (in the prototype behind the `typed-core` feature) and
  keeps every public item consumers use. Nothing outside `ti4-engine` sees a typed id unless it opts in.
- Bots may later depend on `ti4-engine-core` directly for a typed position view (section 2.3).

### 1.2 What lives where

| Concern | typed core (`ti4-engine-core`) | adapter (`ti4-engine`) |
|---|---|---|
| Ids | one generated `#[repr(u16)]` enum per category (`System`, `Planet`, `UnitType`, `Technology`, `ActionCard`, `Agenda`, ...; the prototype's `SystemIx(u16)` etc. stand in), `Seat(u8)`, `WormholeKinds(u8)` | id <-> string tables from the generator (prototype: binary search in sorted name tables, `core_adapter::system_ix`), seat table per game |
| Content | generated typed gameplay data per category (section 1.7), scope masks; no text | the JSON corpus via `ti4-content`, display text and prompt strings, the host catalog |
| State | typed state: arrays/bitsets indexed by id (board as `[SystemState; N]`, units as `(UnitIx, Seat, flags)`), typed `faction_marks`/laws/decks | `GameState` (string) as the save/wire schema; conversions both ways |
| Rules | legality, option generation as typed `Decision`/`Answer` values, applying answers, automatic steps, windows | none (only translation); RULES_* version passed into the core as a number |
| RNG, decks | the same seeded `ChaCha` streams, drawing in the same order over decks sorted in text order (= index order) | seeds from the wire |
| Options, prompts | `enum Decision { Activate{..}, Move{..}, ... }` with typed answers | option ids (`move_gd|38|0|r2`), kinds, labels, payloads, `details` (`details.routes`), prompts, `free_form` |
| Records, hashes | nothing | `DecisionRecord` (strings), V1/V2 decision hashes, event hashes |
| Faction hooks | typed hook tables (as `hooks_movement` today, over typed state) | none at the end; during the migration they stay here and their results are translated (the prototype does exactly that) |

### 1.3 Content tables

- Target: generated by `cargo xtask gen-cards` into `crates/ti4-engine-core/src/generated/`, committed,
  checked by `--check` (1.1). Prototype: `crates/ti4-engine-core/build.rs`, 231 systems, 159 planets,
  136 unit types, 5 wormhole kinds, 7 source tags (about 30 KB of generated Rust), read with
  `ti4-content`'s parser so a flag or a planet list means what it means to the string engine.
- **Ids are numbered in the text order of the id** (`str` byte order). Then index order equals the
  order of every `BTreeMap<String, _>`/`BTreeSet<String>` in the engine, so a bitset iterated in index
  order visits ids exactly as the string set does. That is what makes the prototype's BFS return the
  same route (not just a route of the same length): neighbour order is part of the answer. The test
  `ids_are_numbered_in_text_order` checks `i.cmp(&j) == name_i.cmp(name_j)` for all pairs.
- **Per category, never shared**: 36 ids are spelled the same in two categories (`mr` the technology
  and Mecatol Rex, `ps`, `crisis`, `galvanize`, ...; card audit I14), so each category has its own
  index space and type.
- **Scope** (`SourceSet`) is a bit mask over the record's source tag (`Scope` in the prototype);
  "in scope" is one AND, as `Record::in_sources` is one set test.
- **Newer printings / Thunder's Edge variants** (`ContentStore::resolve_id`: `xxchahero` ->
  `xxchahero_te` when TE is in scope): the variant is its own index; resolution is a per-scope table
  `resolved[scope][ix] -> ix` computed at load (scopes are a handful), so the core never resolves text.
- **Ordering risks** (ENGINE_OPTIMIZATION 4.2, CARD_USAGE_AUDIT "Risks"): every place where today's
  order comes from text must keep it: `BTreeMap` iteration (board, players' sets, scored objectives),
  `deck::ids` (sort then seeded shuffle), option order (part of the decision hash: "the
  offered-option sequence is deliberately not sorted"), `all_routes_from_ship` sorted by
  `(len, ids)`. Text order = index order covers all of them for corpus ids. It does **not** cover a
  mixed set of corpus ids and dynamic ids (below): those need text compare or must not mix.

### 1.4 Generated and dynamic ids

| Id | Today | In the typed core |
|---|---|---|
| Seats | `PlayerId` text: `a`..`h` in selfplay, lobby seat ids on the server | `Seat(u8)` per game: the game's owner table in text order (players plus every owner on the board), built once at load; text at the boundary. The prototype's `Seats::of` does this; at most 16 owners |
| Systems on the map | `Galaxy::placed` refuses ids the corpus does not carry; hyperlane turns (`87a180`) are corpus records | corpus `SystemIx`; nothing dynamic was found (all 3 099 checked positions mapped, 0 unmapped) |
| Created/placed planets | the 12 planets with no `tileId` are corpus records placed during play | corpus `PlanetIx` plus a placement table in state |
| Thunder's Edge / faction variants | separate records, resolved per scope | separate indices + per-scope resolution table (1.3) |
| `faction_marks` (367 production uses), `laws`, `exploration_decks`, `Game::events` | `BTreeMap<String, String>` with `format!`-built keys (`private:#moved:{seq}`) | typed enums per mark family with a text codec in the adapter; the codec must reproduce today's key text and order exactly for saves and for `BTreeMap` iteration |
| Content from `ContentStore::from_dir` (provenance tests, one `ti4-policy` test editing Sol's commodities), homebrew | runtime store | the core's data is compiled in, so a store that changes or adds records is not the core's content. Options: refuse (recommended for new ids: an effect needs a `match` arm anyway), or let the adapter answer such games with the string engine while it exists (the prototype's `Unmapped` fallback). The `ti4-policy` data-edit test would need a core table override hook or to stay on the string path |

### 1.5 Decisions, options and prompts as enums

Today a `Choice` is `{player, prompt, options: [ChoiceOption{id, kind, label, payload}], context,
details, free_form}`, built inside the rules (736 builder sites, 1 224 `format!` calls in engine
production code) and answered by option id. In the typed core:

```rust
enum Decision {                           // what is being asked (the prompt is rendered from it)
    Movement { active: SystemIx, moves: Vec<Move> },
    Activate { candidates: SysSet },
    ...                                   // one variant per decision family (~100)
}
struct Move { boost: Boost, origin: SystemIx, index: u8, routes: Vec<Route> }
enum Answer { Move { boost: Boost, origin: SystemIx, index: u8, route: Option<u8> }, Finish, Decline, ... }
```

The adapter **renders** a `Decision` into today's `Choice` byte for byte: id grammar per variant
(`{verb}|{origin}|{index}`, route answers `{option_id}|r{k}`, `strategic|{card}`, ...), `kind`, label
(with names from the display tables), payload, `details` (e.g. `details.routes`), prompt text, and
**parses** an answer id back into an `Answer` (unknown or `free_form` ids keep today's handling).
Consequences:

- **Decision fingerprints and replays are unchanged by construction** as long as the rendered
  `DecisionRecord` (chosen id, offered ids, context) is byte-identical: the hash is over the rendered
  strings, never over typed values. Prompts that embed card names are hashed in V2 (card audit I17), so
  the renderer must produce the same prompt text; the dual run compares rendered choices, not enums.
- **Option order** is part of the hash: the typed decision must list options in today's order. When
  today's order is "sorted by id text", index order gives it; when it is insertion order, the typed
  code must insert in the same order.
- The rendering code is mostly glue the engine already has (labels, payload helpers in `choice.rs`);
  splitting it from the rules is the bulk of the per-module porting work (section 2.4).

### 1.6 State, serialization, saves

Two options were considered:

| | (a) adapter converts `CoreState` <-> `GameState` | (b) core state has its own serde through the name tables |
|---|---|---|
| Wire, saves, replays | unchanged by construction (`GameState` serde as today) | must re-implement 182 fields' serde to emit the same JSON byte for byte (field order, `skip_serializing_if`, map key order), and `ts-rs` types |
| Consumers reading `GameState` (host 177 mentions, server, review, replayer, wasm, selfplay) | unchanged: they read a materialised `GameState` | all of them ported to core types or a view |
| Cost | a conversion each time somebody reads string state (est. 20-60 µs for a 37 KiB state, vs ~1 ms of engine+bot per decision); lazily, only when asked (views, saves, string bots) | none at run time |
| Risk | two representations to keep consistent (conversion round-trip test over every golden position) | high: the save format is the compatibility contract of the alpha |

**Recommendation: (a)**, with the string state materialised lazily and the typed state as the source
of truth only after the stage that moves the state (stage 5). Until then the typed core owns only
derived data that can be rebuilt from `GameState` (adjacency, occupancy, content tables), which is
what the prototype does, so there is nothing to keep consistent. Resume/replay paths load
`GameState` and convert once.

### 1.7 Cards in the core

Built on the card design decided with the user (core standalone; enums + typed data generated by
`cargo xtask gen-cards`; effects as hand-written exhaustive matches; the adapter owns names, text and
prompts) and on `docs/CARD_USAGE_AUDIT.md` (802 card records in 15 categories; what the rules read).

**What the core needs per category** (the audit's "gameplay" and "gameplay-text" fields; everything
else is display and stays in the adapter's text tables):

| Category (records) | Core enum | Typed data the rules read | Effects |
|---|---|---|---|
| action cards (142) | `ActionCard` (one variant per physical copy, `MoraleBoost1..4`) + `ActionCardKind` (the printed card) | `kind`, `timing: Timing` (from the printed `window` sentence) | `match kind` (retires the hand-listed copy arms and "merge copies by name") |
| agendas (63) | `Agenda` | `kind: Law \| Directive`, `elects: ForAgainst \| Player \| Planet{..} \| Law \| ScoredSecret \| StrategyCard \| Hidden` (from `target`) | `match` in the core's `agenda_effects` / laws |
| promissory notes (40) | `PromissoryNote` | `faction: Option<Faction>`, `play_area: bool` | `match` |
| relics (24) | `Relic` | `in_deck: bool` | `match` |
| leaders (103) | `Leader` | `faction`, `kind: Agent \| Commander \| Hero`, `replaces: Option<Leader>`, `timing: Action \| ActionPhase \| Other` (from `abilityWindow`) | `match` |
| breakthroughs (31) | `Breakthrough` | `faction`, `synergy: [Colour; 2]` | `match` |
| public / secret objectives (40 / 40) | `PublicObjective`, `SecretObjective` | `stage`, `points`; secrets `phase` | predicates as `match` arms (thresholds in code, as today, or as data with the audit's threshold test) |
| technologies (102) | `Technology` | `colour`, `unit_upgrade`, `prereqs: [u8; 4]`, `faction`, `replaces`, `researchable: bool` (from the `text` substring) | `match` / has-technology bitset per seat |
| abilities (73) | `Ability` | none (membership from the faction table) | `match` |
| explores (80), attachments (22) | `Exploration`, `Attachment` | deck trait, `resolution`, `attachment`; attachment modifiers | `match` (Instant), data (Attach) |
| strategy cards (12) | `StrategyCard` | `kind: Leadership \| Diplomacy \| ...` + variant tag, `initiative` (replaces dispatch on the printed name) | `match kind` |
| galactic events (20), generic cards (10) | enums only | nothing until implemented | none |

Board content (systems 231, planets 159, units 136, factions 34) follows the same pattern; the
prototype's movement slice is the first user of the system and unit tables.

**The text-parsed fields become enums in the generator, not in the core**: the five fields whose
prose decides a rule today (action-card `window`, agenda `target`, leader `abilityWindow`,
technology `text` "cannot be researched", strategy-card `name`; audit I06-I10) are mapped to enum
values by the generator with an explicit table, and an unmapped sentence fails `gen-cards` (and
`--check`) instead of silently unbinding a card at run time. The core sees only the enum. This is
the audit's stage 0/2 cleanup moved to generation time.

**The adapter's role for cards**: id <-> enum through the generator's tables (wire, saves, option
ids keep the strings); names and texts for labels, offer cards and the host catalog; the prompt
strings, which are part of the V2 decision fingerprint, rendered byte for byte as today (audit I17).
The engine's 62 "display only" sites that copy card text into payloads move to the renderer.

**How it fits the stages** (section 3.2): the card enums and typed data are stage 3. They can land
before any rule moves into the core: first the generated enums and tables (core) plus the adapter
tables, then the string engine switches its `Record` field reads to typed data looked up via the
enum (same answers, dual-run against the record reads, audit stage 2), then effects move into core
`match` arms area by area in stage 6 together with the state they mutate. The 36 ids shared by two
categories stay separate enums. The behaviour fixes the audit found (I01-I05, I09, I11) stay separate
behaviour changes through the baseline process, never part of a refactor stage.

## 2. Boundary inventory

Measured with `tools/engine_core/boundary_inventory.py` (text-based, like
`tools/engine_profile/module_graph.py`; JSON with `--json`). Production code unless noted.

### 2.1 The public surface of `ti4-engine`

- **925 `pub fn`, 421 `pub` methods, 133 structs, 82 enums, 320 consts, 12 type aliases, 2 traits**
  in 75 top-level modules (all `pub mod`).
- **Consumers name 305 distinct engine paths from 56 modules** in production code. Boundary types:
  `Game` 34 pub methods, `Observed` 65 + `SeatObservation` 12 (the bot's view), `Table` 17,
  `Choice`/`ChoiceOption` 9 each, `MovementRules` 16.
- `GameState` is **not** in the engine: it is `ti4_model::state::GameState` (106 fields, `Player` 70,
  `SystemState` 6), shared by every crate. That is the real boundary: whoever holds a `GameState` reads
  it directly.

### 2.2 Per consumer

| Consumer | engine paths (prod) | engine modules | `GameState` mentions | `Observed` mentions / calls | reads state fields | what changes for a typed core |
|---|---:|---:|---:|---:|---|---|
| ti4-host | 154 | 48 | 177 | 8 / 4 | directly (projection, views, records) | nothing with option (a); views could later read typed board data for speed (views are 2.2 ms per viewer, ENGINE_OPTIMIZATION 1) |
| ti4-server | 24 | 11 | 25 | 12 / 4 | directly (sessions, saves) | nothing |
| ti4-wasm | 13 | 4 | 20 | 7 / 4 | directly (host session) | nothing; size budget |
| ti4-rule-bot | 47 | 15 | **2** | **378 / 502** | almost never: through `Observed` | nothing at first; a typed view later (2.3) |
| ti4-policy | 83 | 20 | 7 | 139 / 306 | through `Observed` | as the rule bot |
| ti4-selfplay | 14 | 6 | 13 | 3 / 8 | harness (timeline, census) | nothing |
| ti4-training, ti4-sim, ti4-mlp, ti4-review, ti4-replayer, ti4-advisor, ti4-calc, ti4-bridge | 1-35 each | 1-12 | 0-37 | 0-16 | mixed | nothing; libtorch crates not compiled here |

(Field-read counts by name are in the script output; they are upper bounds because names such as `id`
or `round` also exist on other types, so the table uses type mentions.)

Test code names 1-87 engine paths per crate (`fixtures`, `Game`, `choice`), all through string ids:
fixtures stay in the adapter and keep working.

### 2.3 Where the rule bot reads state, and what a typed view would gain

The bot never holds a `GameState`: 502 calls of `Observed`/`SeatObservation` methods, 2 mentions of
`GameState`. Its reach questions go through `Observed::movable_into` (cached per position),
`Observed::can_reach`, `Observed::rift_exits_into`, `ship_routes_into`. Profile of 10 golden 4p
games (perf, frame pointers, share of all samples; B is the first build, with only `movable_into`
converted, -7.9%; the final build also converts `can_reach` and the rift count, -9.6%):

| inclusive | string engine (A) | typed reach behind the adapter (B) |
|---|---:|---:|
| `tactical::movable_into` | 16.8% | 9.1% |
| `MovementRules::search_path` (all callers) | 15.5% | 5.9% (left: `Observed::can_reach` and `route_rift_exits`, converted only in the final build) |
| typed core + adapter (`ti4_engine_core::*`, `core_adapter::*`) | - | 2.2% |
| `MovementRules::with_laws` | 6.3% | 6.8% |
| `Galaxy::adjacent` | 6.6% | 3.8% |
| bucket "bot: movement/reach queries" (exclusive) | 27.5% | 21.0% |

- **Keep the string API at first** (stage 1, done in the prototype): the bot is unchanged and gains
  9.6% CPU of a game (measured), because the searches are typed. What remains is string work done per
  question: `with_laws` (faction hooks over `GameState`), `Board::for_player`, move values.
- **Port the bot's reach queries to a typed view** (stage 2): one typed board + adjacency per position
  (`Observed` already caches per position), typed rules per (seat, destination) with typed hooks.
  Estimated from the micro-benchmark: the reach bucket goes from 27.5% to ~2-4% of a game, i.e.
  **est. -20..-25% bot CPU in total** (the 9.6% already measured included). This is the same size as
  ENGINE_OPTIMIZATION proposal #9 (one search per ship for all destinations); the two compound
  (#9 on typed data is cheaper still).
- The bot's objective evaluation (15.1%) and other decision logic (17.9%) read planets, control and
  unit counts through `Observed`; typed board/control data would serve them too (est. -5..-10% more).

### 2.4 Logic versus glue in the engine

- **Id mentions** (`PlayerId`, `SystemId`, ... in engine production code): **6 073**, of which **273
  (4.5%) in boundary modules** (`choice`, `fingerprint`, `decision_context`, `registry`, `fixtures`,
  `observation`, `preview`, `reaction_preview`, `secondary_preview`, `event`, `wiring`, `valuation`,
  `reach_query`) and **5 800 (95.5%) in rules modules**. Tests: 7 049 more (they stay string, through
  the adapter). The 19k figure of ENGINE_OPTIMIZATION 4.1 is the whole workspace.
- **Option building** (`ChoiceOption::new/labelled`, `.with("...")`, `offer_*`): 736 sites, **718 in
  rules modules**. Labels, payloads and offer cards are built where the rule is decided. `format!`:
  1 224 calls. Literal id constructions: 160. `faction_marks`: 367 uses. Short string literals (ids,
  kinds, keys): 6 033.
- Lines: 6.4k production lines in boundary modules, 115.8k in rules modules (factions 42.0k,
  action_cards 7.8k, game 7.4k, combat 6.6k, invasion 5.0k, production 3.8k, diplomacy 3.7k).
- **So: almost everything moves.** A typed core is a port of ~116k lines of rules to typed state with
  the option rendering split out of them; the adapter that stays is ~6k lines plus a renderer per
  decision family (est. 10-15k new lines). This is the opposite of "the engine is a thin layer over a
  core": the core does not exist yet.

### 2.5 The dependency cycle

55 of 75 modules are one strongly connected component (620 edges; `game` fans out to 46, `choice` has
fan-in 43). For the split this means:

- **No module can move alone** if the rule is "the core must not depend on the adapter": a ported
  `combat` would still call `game`, `choice`, `factions`. The strangler therefore goes by **query or
  area behind a function boundary**, not by module: the string function stays where it is and calls
  the typed implementation inside (as `movable_into` does in the prototype), with the translation at
  that call. Only leaf areas (adjacency, movement search, occupancy, content tables, dice/decks) can
  move first; the cycle is broken from the leaves inwards.
- It is also the opportunity: the typed core can be acyclic by construction (board -> movement ->
  combat -> invasion -> phases), which is the only way the engine becomes compilable and testable in
  parts (ENGINE_OPTIMIZATION proposal #18).

## 3. Migration strategy

### 3.1 Strangler, by area, with a dual run

Each stage moves one area behind the string API and keeps the string implementation as the oracle
until the area's gate passes:

1. typed implementation in `ti4-engine-core`; translation in `ti4-engine` (`core_adapter`);
2. a **dual-run mode** (as `core_equivalence` here): the same position asked of both, every answer
   compared (routes, not just booleans; rendered choices, not enums), over the template openings, the
   golden games' positions and the solo bench; plus the engine's own tests with the feature on;
3. switch the call site to the typed path behind a cargo feature (`typed-core`), A/B and verify all six
   benches with the feature **on** (must be 200/200) and **off**;
4. make the feature the default; delete the string implementation of that area after one nightly.

**Benches and `RULES_*` gating** (31 `RULES_*` constants, 151 `rules_version` reads in engine
production code): the version is a number in the typed state and every gated rule is ported with its
gate (`if version >= RULES_HYPERLANE_LANES`). The six benches pin versions 1-5 and 23: they are the
gate that a ported area kept every old version's behaviour. The prototype's adjacency honours the
rules-22 lanes switch as `Galaxy` does.

### 3.2 Stages

| Stage | What | Effort (est., agent-assisted) | Risk | Gain (basis) |
|---|---|---|---|---|
| 0 | `Arc<str>` ids (done on the base branch) | done | low | measured -7.6% bot |
| 1 | **This prototype finished**: typed adjacency + route search behind `typed-core` for the bot's reach queries and the engine's movement options (both through `movable_into`: done) plus the routes (`all_routes_from_ship`, proven equivalent, not yet wired) and `apply_tactical`'s path; nightly dual run | 2-4 days | low (dual run in place, benches 200/200 with it on) | measured -9.6% bot; the rest of stage 1 adds little CPU (routes are 0.0% of samples), its value is one implementation |
| 2 | Typed movement hooks (`hooks_movement` over typed state) and a typed board/occupancy view per position for the bot (`Observed` caches it) | 1-2 weeks | medium: 40 faction modules' movement hooks; destination-dependent hooks (`PassSite`) | est. -10..-15% more bot (reach bucket 21% -> 2-4%) |
| 3 | Cards and content (1.7): generator output (core enums + typed data, adapter tables; `card-generator` branch), string engine reads typed data instead of `Record` fields, text-parsed fields as enums | 1-2 weeks after the generator | low-medium (36 cross-category ids, newer printings) | est. -5..-9% (content lookups are 9.7% of a game) |
| 4 | Typed control/planet/objective queries for objective evaluation and views | 1-2 weeks | medium | est. -5..-10% bot, views -30..-50% |
| **gate** | decide whether the core should own the state | | | |
| 5 | Typed state as the source of truth (`CoreState`), `GameState` materialised by the adapter; typed `faction_marks`, laws, decks; checkpoints copy typed state | 4-8 weeks | high: 182 fields, round-trip conversions, save compatibility, every module touched | est. -15..-25% (checkpoints 12.7%, clones, memcmp of the rest) |
| 6 | Rules ported area by area onto typed state with decisions as enums and the renderer split out (combat, invasion, production, phases, cards, 48 faction modules) | 3-6 months | high: 116k lines of rules, 736 option sites, prompt/label text in fingerprints, the churn below | est. the remainder of the string cost; total with 1-6 -35..-55% bot CPU |

Totals are not additive: each stage removes part of a shrinking total.

### 3.3 Open branches and churn

- The engine changed by 243 commits and +43.8k/-2.7k lines in the 14 days before this study (814
  commits in the repo); 112 local branches were updated since 2026-10-19, 17 of them unmerged with
  changes in `ti4-engine`/`ti4-model`.
- Stages 1-4 add code beside the string code and change a handful of call sites each (stage 1: 3 call
  sites), so they merge like any fix. Stages 5-6 rewrite the code that every fix touches: a fix to
  `combat` made on a branch in the string engine during stage 6 has to be ported again. Manage it by
  porting one area at a time **on the consolidated line**, merging the area quickly (days, not
  weeks), freezing other changes to that area meanwhile, and keeping the dual run in the nightly so a
  late fix to the string side shows up as a difference.

### 3.4 Flexibility lost and kept

- **Adding a card/unit/faction**: kept. The tables are generated from the JSON, so a new record gets an
  index and appears in decks and catalogs; code that handles it names a generated constant instead of a
  string (a typo becomes a compile error, card audit I13). A new card with an effect needs JSON + code,
  as today.
- **Content updates** (re-extraction): a reviewable diff of the generated tables; renamed ids fail to
  compile instead of silently unbinding (today's failure mode, card audit I06).
- **Fixtures and tests**: kept, they go through the adapter with string ids (7 049 id mentions in engine
  tests; no test needs to change).
- **Runtime content (`from_dir`, homebrew)**: data edits of known ids kept (runtime tables); new ids
  not supported by named constants (1.4). This is the one real loss.
- **Debuggability**: typed state prints as numbers; the adapter must provide `Debug` through the name
  tables (the prototype's `route_text`).

### 3.5 Compared with the cheaper alternatives

| Alternative | Gain | Effort | Note |
|---|---|---|---|
| Stage 0 `Arc<str>` ids | measured -7.6% bot, -22% views, -17% resume | done | no behaviour change by construction |
| Ids stages 1-4 (ENGINE_OPTIMIZATION 4.4: temporaries, rank tables, seat indices, typed records) | est. -23..-35% cumulative | 4-7 weeks | keeps strings as the representation; ranks give integer compares for corpus ids |
| Checkpoints without log copies (#8) | est. -8..-15% bot, -30..-40% resume | medium | orthogonal: needed under a typed core too |
| Bot reachability per position (#9) | est. -20..-25% bot | medium | orthogonal; overlaps with stage 2 here, compounds with it |
| Typed core stages 1-4 (this design, no state ownership) | measured -9.6% (stage 1 part); est. -25..-35% cumulative | 4-7 weeks | typed data where it is hot; same order of effort and gain as ids stages 1-4, but with bitsets/arrays instead of rank-compared strings, and code that stays usable for stages 5-6 |
| Typed core stages 5-6 (state ownership, rules on typed state) | est. a further -15..-25% | 4-7 months | a rewrite; only after the gate |

## 4. Prototype and measurements

### 4.1 What was built

- `crates/ti4-engine-core` (no dependencies): `ids.rs` (typed ids), `bits.rs` (bitsets iterating in
  index order), `tables.rs` + `build.rs` (generated tables: system flags, wormhole masks, lanes, planets,
  unit types, source bits; names behind feature `names`), `hex.rs` (axial hexes, dense grid),
  `galaxy.rs` (`CoreGalaxy`: placement, wormholes with the five law/card switches, the rules-22
  hyperlane lane walk ported from `ti4_content::hyperlane`, ability links; `adjacency()` computes every
  system's neighbours once), `board.rs` (owner masks per system), `movement.rs` (`CoreRules`: every
  field of `MovementRules`, the same BFS with the same dominance rule, `path_from_ship`,
  `all_routes_from_ship`, the plain builder).
- `crates/ti4-engine/src/core_adapter.rs` (feature `typed-core`, off by default): text <-> index,
  `galaxy_from_legacy`, `galaxy_native`, `Seats`, `board_from_state`, `rules_from_legacy` (reads a built
  `MovementRules` through the new read-only `MovementRules::parts()`), `adjacency_for` (per-thread
  cache keyed by the galaxy value), `path_from_ship` with fallback, and an `Unmapped` error for
  anything the core cannot express (the strangler's escape hatch; it never fired on the checked positions).
- Engine call sites behind `#[cfg(feature = "typed-core")]`: `tactical::movable_into`,
  `tactical::route_rift_exits`, `Observed::can_reach`. With the feature off the code is the same as
  before (the closure and the `let` binding are semantics-neutral), and all six benches are identical.
- `crates/ti4-engine-core-bridge`: `check.rs` (dual run), `workload.rs` (template openings, golden
  positions played exactly as `selfplay run`), binaries `core_equivalence` and `core_movement_bench`,
  `tests/equivalence.rs`.
- `tools/engine_core/ab_movement.sh` (interleaved A/B of the micro-benchmark),
  `tools/engine_core/boundary_inventory.py` (section 2).

### 4.2 Equivalence (measured)

| Run | Positions | Adjacency sets compared | Rules built (plain / with hook state / board facts changed by a hook / map copies by a hook) | Route queries | Route sets (`all_routes`) | Differences |
|---|---:|---:|---|---:|---:|---:|
| `core_equivalence templates --routes`: openings of 50 map templates x {POK, FULL}, all 24 lobby factions rotated over the seats, plus 37 map-switch variants each (5 switches, token wormholes, links, retained kinds, severed system) | 100 | 831 600 | 18 950 (13 054 plain) | 185 824 | 92 912 | **0** |
| `core_equivalence golden profile-20.json 20 10`: every 10th position of the 20 golden games (10 4p + 10 6p, the bench's games: 20/20 hashes equal `tools/rulebot/baseline.json`) | 2 999 | 692 769 | 547 096 (345 298 plain, 222 map copies) | 7 093 514 | - | **0** |
| `cargo test -p ti4-engine-core-bridge` (tables vs corpus, text order, natively built galaxy = converted, templates) | | | | | | 6/6 pass |
| `cargo test -p ti4-engine --features typed-core` (the engine's own suite through the typed reach) | | | | | | 3 918 pass |

Each route query compares the full route `path_from_ship` returns (origin first, active system
last), at the ship's move value and one more, through the adapter and, where the seat carries no
hook state, through the plain typed builder too. 30 templates were not built by the selfplay spec
(9/12 players, the `*StaticEq` templates name a tile `0g` the corpus does not have, `2025scptFinals`
places a tile twice), refused by the string engine as well.

### 4.3 Micro-benchmark (measured)

`core_movement_bench MODE profile-4p.json 4 10 3` via `tools/engine_core/ab_movement.sh` (positions of
4 golden 4p games, every 10th; every seat x every destination x every ship of the seat; CPU time of
the timed phase on the thread; interleaved runs; same answer digest in every mode):

| Mode A vs mode B (5 interleaved runs each, 498 positions, 1 250 484 questions x 3) | A, median | B, median | B/A |
|---|---:|---:|---:|
| `legacy-search` vs `core-search` (rules built untimed; only the searches) | 7 998 ms | 492 ms | **0.062 (16.2x)** |
| `legacy` vs `adapter` (string rules + translation + typed search) | 9 009 ms | 1 701 ms | **0.189 (5.3x)** |
| `legacy` vs `core` (typed board and rules; seats with hook state answered by the string engine: 31% of the questions) | 8 991 ms | 3 466 ms | 0.385 (2.6x) |

Same answer digest in all 30 runs. Per question: 7.2 us string, 1.4 us adapter, 0.4 us typed search
alone. In `core` mode the 31% fallback questions cost about 2.8 s of the 3.5 s; the plain-typed part
is est. ~0.75 us per question (~9.5x), which is what typed hooks (stage 2) would extend to every seat.

- The **search** on typed data is ~16x faster: bitset adjacency read per step instead of a
  `BTreeSet<&str>` built per call (`Galaxy::adjacent`), integer dominance keys instead of
  `(String, bool, BTreeSet<String>)`, a route arena instead of a `Vec<String>` clone per queue entry.
- Building the rules (`Board::for_player` + `with_laws` + `apply_movement_effects`) is ~11% of the
  string path (9 009 - 7 998 ms) and becomes the larger part of the adapter path; the plain typed builder removes it for
  seats without hook state, which is why stage 2 (typed hooks) matters.

### 4.4 End to end (measured)

| | A: `typed-core` off | B: `typed-core` on | Change |
|---|---:|---:|---:|
| 20 golden games (`profile-20.json`), CPU, median of 3 interleaved, 4 workers | 49.4 s | 44.7 s | **-9.6%**, 20/20 hashes |
| only `movable_into` converted (first build) | 48.8 s | 45.0 s | -7.9%, 20/20 hashes |
| `bench`, `bench-v1..v5` (`verify_benches.sh`) | 200/200 each (CPU 2 751 s over the six) | 200/200 each (CPU 2 465 s, -10.4%, sequential runs, not interleaved) | identical |

### 4.5 What the prototype does not show

- Typed **state**, typed **decisions** and the **renderer** were not prototyped: the slice is a
  read-only query over derived data. The hard parts of stages 5-6 (state round-trips, rendering
  byte-identical choices and prompts, porting rules with their option building) are estimated, not
  measured.
- The faction hooks were not ported: the adapter translates their results (and a map copy one of
  them made, 222 times in the golden positions). Stage 2's estimate assumes typed hooks cost about what
  the plain builder costs.
- wasm CPU was not measured for the slice (the offline bot uses the same `movable_into`).

## 5. Recommendation

**Do it in stages; do not start with the crate swap.** Concretely:

1. **Now (stage 1, 2-4 days)**: finish the movement slice: route `all_routes_from_ship` (already
   proven equivalent) and `apply_tactical`'s path through it, add the dual run to the nightly,
   make `typed-core` the default after a nightly with it on. Measured gain so far -9.6% of bot CPU with
   every bench identical.
2. **Next (stages 2-4, 4-7 weeks)**: typed movement hooks and a typed board view for the bot, typed
   content records (the card audit's plan), typed control/objective queries. Each is an area behind an
   existing function, with its own dual run and A/B; together est. -25..-35% bot CPU, overlapping with
   ids stages 1-4 (do these instead of rank tables, not in addition).
3. **Gate before stages 5-6**: own the state and port the rules only if, after 1-4 and the
   checkpoint fix (#8), the profile still shows string work as the top item and the alpha's rule
   fixes have slowed. Otherwise the 4-7 months are better spent elsewhere.

**Decisions needed from the user** (the evening's port applied the recommended answer to each as the default; see section 0):

1. Stage 1 as the next step, with `typed-core` becoming the default after one nightly with it on?
2. (Decided: generator with committed output, `--check`.) When `card-generator` lands, should the
   movement slice switch to its system/unit enums in the same change (recommended), deleting the
   prototype's `build.rs`?
3. Runtime content (`from_dir`, homebrew, the `ti4-policy` data-edit test): refuse in the core
   (recommended) and keep such games on the string path while it exists, or add a table override hook?
4. Typed core stages 2-4 in place of ids stages 1-4 (recommended), or both?
5. Whether stage 5 (typed state as the source of truth) is a goal at all, or the core stays a typed
   accelerator over `GameState` for derived data.

## 6. Open risks

- **Ordering**: text order = index order holds for corpus ids (tested); any mix with dynamic ids, or
  a sort today done on labels or display names, must be found per area. The dual run catches it only
  where positions exercise it.
- **Hook coverage**: the golden games and template openings seat the 24 lobby factions; Thunder's
  Edge factions are not seatable here, so their movement hooks (Sundered, Void Tether, Luminous,
  Myru Vos) were exercised only through the engine's unit tests with `typed-core` on (3 918 pass), not
  by the dual run on game positions.
- **Fingerprints**: prompts embed card names and are hashed (V2); a renderer that changes one space
  changes every replay digest. Stage 6's dual run must compare rendered `DecisionRecord`s.
- **The adjacency cache** compares whole `Galaxy` values per call (`adjacency_for` incl. the
  compare: 0.8% of samples) and is per
  thread; it is a prototype convenience. Stage 2 should carry the typed adjacency in `Observed`/`Game`
  with the galaxy instead.
- **Two implementations** of each moved area exist until its string version is deleted; a fix to the
  string side during that window must be ported (the nightly dual run is the guard).
- **libtorch crates** (`ti4-mlp`, `ti4-tensor`, `ti4-advisor`, `ti4-replayer`, `ti4-review`) were not
  compiled here; they do not name the changed items, but need a compile before a merge.
- **Effort estimates for stages 5-6 are the least certain numbers here**: they are scaled from line
  counts and from the prototype (about 3 100 lines of Rust: core, adapter, checks, benchmark, written
  and verified in one working session), not from porting a rules module, where the option rendering
  and the hooks are the work.

## 7. Reproduce

```text
export CARGO_TARGET_DIR=/some/private/target
cargo build --release -p ti4-engine-core-bridge                     # core_equivalence, core_movement_bench
target/release/core_equivalence templates --routes
target/release/core_equivalence golden tools/engine_profile/profile-20.json 20 10
tools/engine_core/ab_movement.sh target/release/core_movement_bench legacy core
tools/engine_core/ab_movement.sh target/release/core_movement_bench legacy-search core-search
cargo build --release -p ti4-selfplay && cp target/release/selfplay /tmp/selfplay-on
cargo build --release -p ti4-selfplay --features ti4-engine/string-movement && cp target/release/selfplay /tmp/selfplay-off
tools/engine_profile/ab_time.sh /tmp/selfplay-off /tmp/selfplay-on tools/engine_profile/profile-20.json 3 4
tools/engine_profile/verify_benches.sh /tmp/selfplay-on               # all six: 200 identical
cargo test -p ti4-engine-core -p ti4-engine-core-bridge
cargo test -p ti4-engine --features typed-core
python3 -I tools/engine_core/boundary_inventory.py
```

`typed-core` is a default feature of `ti4-engine`; to compare with the string engine build with
`--features ti4-engine/string-movement` (the off switch). The whole gate of a stage:
`CARGO_TARGET_DIR=... WASM_OPT=<binaryen wasm-opt> tools/engine_core/verify_stage.sh OUT BASE_SELFPLAY`.

## Files

- `crates/ti4-engine-core/` (core: `src/{ids,bits,tables,hex,galaxy,board,movement}.rs`, `src/generated/`; the prototype `build.rs` was replaced by the generator in stage 3)
- `crates/ti4-engine/src/core_adapter.rs` (adapter, feature `typed-core`); call sites in
  `tactical.rs` (`movable_into`, `route_rift_exits`) and `choice.rs` (`Observed::can_reach`);
  `movement.rs` (`MovementRules::parts`, `MovementParts`)
- `crates/ti4-engine-core-bridge/` (`src/check.rs`, `src/workload.rs`, `src/bin/core_equivalence.rs`,
  `src/bin/core_movement_bench.rs`, `tests/equivalence.rs`)
- `tools/engine_core/ab_movement.sh`, `tools/engine_core/boundary_inventory.py`,
  `tools/engine_core/verify_stage.sh` (the gate of a stage)
- Stages 2-5: `crates/ti4-engine/src/core_adapter.rs` (`SeatReach`, `SeatCache`, `present_together`,
  `planet_in_scope`, `system_in_scope`, `planet_value`, `printed_specialties`, `stations_in`),
  `tactical.rs` (`SeatFleet`, `search_path`, `search_all_routes`), `choice.rs` (`Observed`'s typed
  queries), `transactions.rs`, `production.rs`, `planets.rs`, `space_stations.rs`; generated code from
  `card-generator` in `crates/ti4-engine-core/src/generated/`, `crates/ti4-engine-core/names/`,
  `crates/ti4-engine/src/generated/`
