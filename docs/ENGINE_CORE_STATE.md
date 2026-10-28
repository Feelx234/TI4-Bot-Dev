# The core owns the state and the rules: state design, migration shape, work packages (2026-10-27)

Design only. Branch `engine-core-state-design`, based on `engine-raw-core-design` (`15a5f101`). Nothing in
the code was changed; this file is the only change.

Decision by the user (2026-10-27): `ti4-engine-core` will **own the game state and the game logic**, with no
strings and no JSON inside it. `ti4-engine` becomes an **adapter**: it translates string ids, option ids,
JSON and text to and from the core and keeps the wire format, views, saves, decision fingerprints and
benches byte-identical. Goal of the shape: cheap clones and checkpoints (dense typed arrays, bitsets,
`Arc`/copy-on-write for the large shared parts), no `String` and no `BTreeMap` in core state,
deterministic and order-preserving (string order == enum integer order wherever order is observable).

Inputs read: `docs/ENGINE_CORE.md` (stages 1-5 landed), `docs/ENGINE_CORE_AUDIT.md` (branch
`engine-core-audit`), `docs/BOT_STRING_AUDIT.md` (branch `bot-string-audit`), `docs/CARD_USAGE_AUDIT.md`,
`docs/CONTENT_GENERATOR.md`, and the code: `ti4-model/src/state.rs` (`GameState`, `Player`, `SystemState`),
`ti4-engine/src/{game,choice,decision_context,fingerprint,rng,dice,timing}.rs`, `tools/engine_profile/module_graph.py`.

**Labels.** *Measured* = I ran it for this document (module graph, line counts, layer analysis in section 4.2).
*Copied* = a number from the earlier documents, not re-measured. *Estimated* = my judgement, with the basis
given. Sizes of the proposed types are computed from the sketches (`size_of` is not measured: no such type
exists yet; the foundation package must measure them, section 4.3).

Contents: 0 summary and decisions taken here, 1 state types, 2 decision/option model, 3 state sync during the
migration, 4 layering, work breakdown and merge order, 5 gating, 6 risks / rollback / size / time, 7 agent
briefs, 8 open decisions.

---

## 0. Summary

| Question | Answer |
|---|---|
| State shape | One `CoreState` of ~16 **slices** (each its own struct and file): small fixed-size structs, bitsets for sets over generated enums, small inline lists for ordered lists, a `Chunked`/`Cow` board and `Arc<Player>`s so that a clone is ~60 refcount bumps plus ~2 KB of memcpy (estimated), append-only `Log`s with shared prefixes, `Deck`s as `Arc<[T]> + head`. No `String`, no `BTreeMap`, no `serde`. RNG streams, decks and hands are ordinary slices; hiding is a property of the projection, not of storage. |
| Decisions | The core stores a typed `Pending` (what is asked), **not** the option list. `Choice { actor, ask: Ask, opts }` and `Opt` are enums, one sub-enum per rule area; options are a pure function of `(state, pending)`, listed in today's order. The adapter renders `(Ask, Opt)` into today's `ChoiceOption {id, kind, label, payload}`, prompt and `DecisionContext`, byte for byte, and parses ids back. Fingerprints are hashed over the rendered strings, so they stay identical by construction; the dual run checks it. |
| Migration shape | **Lend with declared footprints, flipping to ownership at the driver.** While an area is being ported the old string `GameState` stays the source of truth between calls; a ported call *lends* the slices it declares (convert in, run in the core, write back), consecutive ported calls share one lend region, read-only query areas use one cached typed view per position. When the driver (`Game::step`) is ported, the authority flips: `CoreState` is the state and `GameState` is materialized on demand; unported leftovers are then lent the other way. Not a per-position full conversion (too slow for mutators, no way to find what the string code changed), not field-level ownership of `GameState` (18.5 k lines of `game.rs` and 177 host mentions poke fields directly). |
| Packages | 4 foundation + 4 independent preparatory (start at once) + 5 query/kernel (wave A) + 9 rule areas (wave B) + 6 faction shards (wave C) + 2 driver/flip (wave D). Sections 4.4 to 4.7. |
| Estimate | About 160 agent-days of package work plus ~20% integration and diff-chasing = **150-230 agent-days** (range reflects the uncertainty of rule areas, not rounding); critical path ~65 agent-days; with 6-8 agents in parallel about **8-12 calendar weeks**. The checkpoint gain needs the flip (last), except for the string-side fix X4. |
| Gating | Pure refactor. `RULES_VERSION` (29 at `15a5f101`) and all `RULES_*` constants unchanged; `bench`, `bench-v1..v5` 200/200 identical with each area on the core **and** off; wire/TS types, fixtures, saves untouched (`git diff --exit-code` on them); dual run clean; 20-golden CPU A/B not worse; wasm size reported. Section 5. |

Decisions taken here (the user may overrule; none is hard to change before wave A):

1. The core keeps **one dependency exception**: `rand_chacha` (+ `rand_core`, default features off) for the
   ChaCha8 streams, which must stay bit-identical. The criterion is "no strings / JSON / maps", not "no crates".
2. Option lists are **derived, not stored** (section 2.2).
3. Hook points (what factions plug into) are **defined in the core's kernel layer** as a typed table; the
   faction modules are a separate top layer (`ti4-engine-core-factions`), so factions stop being part of the cycle.
4. Where today's code sorts or orders by **display name or label**, the generator emits a rank table
   (`NAME_RANK`), so the core still never sees text.
5. Runtime content (`ContentStore::from_dir`, homebrew) is **refused by the core** (as decided in
   `ENGINE_CORE.md` 1.4); such stores stay on the string path while it exists.

---

## 1. State types

### 1.1 Principles (the rules every type follows)

1. **Index = text order.** Every id enum is numbered in text order (generated; `ids_are_numbered_in_text_order`
   is a test). A bitset iterated in index order therefore visits ids exactly as `BTreeSet<String>` did. `Seat`
   follows the same rule over the game's owner table (section 1.3).
2. **Sets are bitsets, ordered lists stay lists.** Where the string state has a `BTreeSet<Id>` or the keys of
   a `BTreeMap<Id, _>` the order is the id order: use `Set<T, W>` (a `[u64; W]`). Where it has a `Vec<Id>` the
   order is the insertion order and is observable (hand order feeds option order and JSON): use `SmallList<T, N>`
   (inline up to `N`, spills to the heap). Never "improve" a list into a set: the output would change.
3. **Maps over a closed key set are arrays** (`[T; Faction::COUNT]`, `[u8; 4]` for the four relic-fragment
   traits). Sparse maps over a large key set are `SortedMap<K, V, N>`: a sorted small vec, iteration in key
   order, lookups by binary search.
4. **Numeric text order.** A key family whose text embeds a decimal number (`private:#moved:{seq}`) iterates in
   *text* order today (`"10" < "9"`). The typed key wraps the number in `DecimalTextOrd(u32)` whose `Ord` compares
   the decimal digits as text. This is the one place where integer order and string order differ and it is tested.
5. **Copy-on-write for the big and the long.** Anything larger than ~512 B or append-only sits behind an `Arc`
   (`Cow<T>` = `Arc<T>` + `make_mut`) or is chunked. A checkpoint is `CoreState::clone()`.
6. **Nothing derived is stored.** Option lists, adjacency, occupancy summaries, resource totals, control
   counts are functions or caches outside `CoreState` (the existing `Rc<Adjacency>`, `SeatReach` caches move to
   a `Derived` struct kept next to the state, rebuilt on demand, never cloned into checkpoints).
7. **Every `String` field of `GameState` has a typed home** (table in 1.8). The adapter owns the codec text.
8. **Version is state.** `Meta.rules_version: u32` is read by every gated rule exactly where the string engine
   reads it (151 reads in production code, *copied*); the constants live in the core and are re-exported by
   `ti4-model` (the values do not change).

### 1.2 Containers (foundation F1; ~1.2 k lines; no unsafe)

```rust
pub struct Set<T: Idx, const W: usize>([u64; W]);          // TechSet = Set<Technology,2> (102 techs), SysSet (existing, 4 words)
pub struct SeatSet(u16);                                    // up to 16 owners
pub struct SmallList<T: Copy, const N: usize> { .. }        // enum { Inline{len,[T;N]}, Heap(Vec<T>) }
pub struct SortedMap<K: Ord + Copy, V: Clone, const N: usize> { .. }   // small sorted vec, key order
pub struct Cow<T: Clone>(Arc<T>);                           // Deref; fn edit(&mut self) -> &mut T = Arc::make_mut
pub struct Chunked<T: Clone, const LEN: usize, const CHUNK: usize> { chunks: [Cow<[T; CHUNK]>; LEN/CHUNK] }
pub struct Log<T: Clone> { head: Arc<LogNode<T>>, len: u32 }     // append-only; clone shares the prefix; tail chunk is COW
pub struct Deck<T: Copy> { order: Cow<Vec<T>>, head: u32 }       // draw = head += 1 (no copy); insert/bottom edits copy the Vec
pub struct Opt32(NonZeroU32);                               // Option<u32> for round/activation stamps without a tag word
```

`Idx` is implemented by the generated enums and `Seat`. No container allocates on `clone` except `Log`'s
shared tail chunk on the first append after a clone (one small allocation).

### 1.3 Seats and owners

`Seat(u8)` is an index into the game's **owner table**, built once at game creation (and at `load`) from the
`PlayerId` texts of the players **plus every other owner that can appear on the board** (neutral units, the
`styx_holder`, the Fracture's owners, anything `neutral_units.rs` uses), sorted in **text order**, at most 16.
Text order of owners is what `BTreeMap<PlayerId, _>` iteration depends on today, so `Seat` order equals it.
The core knows `n_players` and the ordered list `seating_order: [Seat; 8]`; it never sees the texts. The
adapter's `Seats` (today `core_adapter::Seats::of`) becomes the persistent table inside `Game`. Dual-run check:
`seat_text(order) == sorted texts` at every lend.

### 1.4 The state (`ti4-engine-core/src/state/*.rs`, one file per slice)

```rust
#[derive(Clone)]
pub struct CoreState {
    pub meta:       Meta,            // 1  round, phase, speaker, active, rules_version, seq counters, flags      (~160 B inline)
    pub players:    Players,         // 2  [Cow<Player>; 8] + n                                                  (~72 B + Arcs)
    pub board:      Board,           // 3  systems + planets cells, chunked COW                                  (~30 Arcs)
    pub map:        Cow<CoreGalaxy>, // 4  placement, wormholes, lanes, adjacency (the existing core type)       (1 Arc)
    pub decks:      Decks,           // 5  action, secret, agenda, relic, 3+1 exploration, public objectives     (~9 x (Arc + u32))
    pub cards:      CardZones,       // 6  discard piles, salvage yard, reserve (DeckReserve), promissory table   (Cow lists)
    pub objectives: Objectives,      // 7  revealed, scored per seat, ledger of feats, plots                      (~200 B)
    pub agenda:     AgendaState,     // 8  laws, in-play agenda, votes, choices, override slots, logs             (~250 B + Log)
    pub strategy:   StrategyState,   // 9  unclaimed, goods per card, initiative, picks per player                (~120 B)
    pub explore:    ExploreState,    // 10 attachments, frontier/ingress/breach tokens, exploration log           (Cow)
    pub trade:      TradeState,      // 11 transactions this round, negotiations, diplomacy state                  (Cow)
    pub marks:      Marks,           // 12 faction memory (the 367-use `faction_marks`) as typed per-faction records
    pub transient:  Transient,       // 13 `last_*` slots, combat round scratch, production counters, TransientFlags
    pub pending:    Pending,         // 14 the open windows and the question (section 2)
    pub rng:        Rng,             // 15 seed + ChaCha8 streams by typed domain
    pub ledger:     Ledger,          // 16 events, dice history, vp ledger, decision log mirror   (all `Log<T>`)
}
```

**Players** (`Players { n: u8, seats: [Cow<Player>; 8] }`): `Player` is split into sub-structs, one file each, so
packages own disjoint parts (`state/player/{econ,tech,cards,leaders,timers,combat_timers,..}.rs`):

```rust
pub struct Player {
    pub faction: Faction, pub home_system: Option<System>, pub home_planets: SmallList<Planet, 4>,
    pub econ:    PlayerEcon,     // tactic/fleet/strategic tokens, trade_goods, commodities, vp: i16 each; passed; flags   ~24 B
    pub strat:   SmallList<StrategyCard, 3>, pub strat_exhausted: Set<StrategyCard, 1>,
    pub tech:    PlayerTech,     // owned: TechSet (16 B), exhausted: TechSet, assimilated: SortedMap<Technology, Technology, 2>
    pub cards:   PlayerCards,    // hand: SmallList<ActionCard, 10>, reaction modes: SortedMap<ActionCard, ReactionMode, 4>,
                                 // secrets: SmallList<SecretObjective, 4>, relics: SmallList<Relic, 4>, exhausted relics: Set,
                                 // fragments: [u8; 4], explore cards: SmallList<Exploration, 4>, plots, plot_objectives
    pub leaders: [LeaderSlot; 4],// agent, commander, hero, mech/alliance slot; status enum {Locked,Unlocked,Exhausted,Purged,..}
    pub timers:  Timers,         // the ~35 `Option<u32>` round/activation stamps -> Opt32 (4 B each, ~140 B) + the 9 `Vec<u32>` -> SmallList<u32,2>
    pub misc:    PlayerMisc,     // captured units, returning counters, duress_by, stability, feats, breakthrough, ...
}
```

Estimated `size_of::<Player>()` 450-550 B inline (today: ~1.3 KB inline plus ~60-100 heap allocations per
player: each `BTreeSet`/`BTreeMap` node, each `Arc<str>` bump, each `Vec`; *estimated* from the field list,
`state.rs:340-602`). `Cow<Player>` makes a clone one refcount bump per seat; a write to one seat copies one
Player (~0.5 KB).

**Board** (largest mutable structure):

```rust
pub struct Board {
    sys:    Chunked<SysCell,    231, 16>,   // indexed by `System as usize`; 15 chunks of 16 cells
    planet: Chunked<PlanetCell, 159, 16>,   // indexed by `Planet as usize`; 10 chunks
}
pub struct SysCell {                         // ~40 B
    pub units:   UnitList,                   // SmallList<Unit, 12>: space area, order = today's `Vec<Unit>` order
    pub cmd:     SeatSet,                    // command tokens
    pub flags:   SysFlags,                   // touched, frontier, ingress, breach, purged, thunders_edge_system, ...
}
pub struct PlanetCell {                      // ~48 B
    pub control:   Option<Seat>, pub units: UnitList, pub attach: SmallList<Attachment, 3>,
    pub coexist:   SeatSet, pub exhausted: bool, pub placed_in: Option<System>,   // `placed_planets`
}
#[derive(Copy, Clone)] pub struct Unit(u16);   // 8 bits type, 4 bits owner, sustained, galvanized  (today `Unit` is 40 B with two Arc<str>)
```

"`touched`" reproduces the string state's rule that an *absent* `board` entry is not an empty system
(`state.rs:1366-1384`): the flag is part of the cell, so the codec round-trips exactly and `Galaxy`-vs-ledger
mistakes stay impossible. `Unit` order inside a list is preserved (casualty option order and the JSON depend
on it; it is **not** canonicalised). Estimated board size: 231x40 + 159x48 = ~17 KB, of which a clone copies
nothing (25 Arc bumps) and a unit move writes one 640-768 B chunk.

**Decks and hidden information.**

| Field today | Core holder | Visible to | Notes |
|---|---|---|---|
| `action_card_deck`, `secret_deck`, `relic_deck`, `agenda_deck`, `objective_deck` (stage I then II), `exploration_decks` (4 traits) | `Decks`: one `Deck<T>` each (`Cow<Vec<T>>` + `head`) | nobody (order hidden), top card for some effects | a draw advances `head`; `put_bottom` / reshuffle edits copy the vec (~0.1-0.6 KB). Reshuffles use `Rng` and sort-then-shuffle in id order = enum order (as `deck::ids` does) |
| `discarded_action_cards`, `salvage_yard`, `DeckReserve` | `CardZones` | public (discard) | lists keep order |
| hand `action_cards`, `secret_objectives`, `promissory`, `exploration_cards`, `relic_fragments` | `PlayerCards` | owner; counts public | the adapter's projection (`views`) hides, as it does now |
| RNG stream positions | `Rng` | nobody | determinization = reseed + reshuffle decks + redeal hands: all hidden info sits in `decks`, `players[*].cards`, `rng`, so a sampler for the bot needs exactly three slices |

**RNG.** Today `GameRng { seed, streams: BTreeMap<String, ChaCha8Rng>, observation, sync }` and a stream for
domain `d` is seeded `SHA-256(seed_le || d)` (`rng.rs`, `derive_seed`). In the core:

```rust
pub struct Rng { seed: u64, streams: SortedMap<RngDomain, ChaCha8Rng, 12> }     // ChaCha8Rng 136 B; clone = ~1.6 KB at 12 streams
pub enum RngDomain { Combat, Retry(u16), Seating(SeatingKind), Fracture, Neuraloop, YssarilHero, Direct, /* .. from the 20 literal domains */, Dyn(Family, u32) }
pub trait SeedSource { fn seed_for(&self, seed: u64, d: RngDomain) -> [u8; 32]; }   // implemented by the adapter (needs the domain text for SHA-256)
```

Draw order, `shuffled`, `die`, word positions: identical code over the same crate (`rand_chacha`). The domain text
is needed once per stream (on first use) and lives in the adapter's `SeedSource`; the core never holds it. The
`observation`/`sync` side channels (replay "restore positions", marks) stay in the adapter: they wrap
`Rng::positions()` / `set_positions()` (both exist as plain `[(RngDomain, u128)]` in the core). `Rng` is a
`Cow` inside `CoreState` only if profiling shows the 1.6 KB hurts a checkpoint; a first cut stores it inline.

**Log, rolls, ledger** (all `Log<T>`, append-only, shared prefix, typed records):

| Today | Core |
|---|---|
| `Game::events: Vec<String>` and `Resolver::log: Vec<String>` | `Log<EventRec>`; `EventRec` is the typed `Event` (`event.rs` is 139 lines) + minimal args; the adapter renders the text |
| `Dice { history: Vec<Roll> }` | `Log<RollRec { unit: UnitType, needed: u8, face: u8, purpose: RollPurpose, scene: System }>`; `RollPurpose` replaces the strings (`"ground combat"`, `"bombardment"`, ... ~25 literals), adapter table gives the text |
| `vp_ledger: Vec<(PlayerId, i32, String)>` | `Log<(Seat, i16, LedgerReason)>`; `LedgerReason` is an enum with payload variants (`Objective(Public..)`, `Relic(..)`, ..); the codec reproduces the text |
| `combat_round_dice`, `exploration_log`, `agenda_log`, `agenda_casts`, `relic_log` (viewer history) | `Log<..>` each |
| `DecisionLog` (`records: Vec<DecisionRecord>`) | **stays in the adapter**: it is the rendered, hashed form. The core does not know prompts |

### 1.5 Pending decisions (summary here, model in section 2)

`Pending` replaces the private window fields of `Game<'a>` (`secondary`, `scoring`, `voting`, `tactical`,
`aftermath`, `trade`, `diplomacy`, `agenda_talks`, `tokens`, `executive_order`, `leader_*`, ..., about 25
`Option<Window>` fields, `game.rs:919-975`): a stack of typed windows, with the question on top.

```rust
pub struct Pending { pub stack: SmallList<Window, 4>, pub top: Option<Ask> }   // `Window` is an enum, one variant per window kind, each a small struct
```

### 1.6 Sizes and clone costs (computed from the sketches; **estimated**, to be measured by F1/F3)

| Part | Inline bytes | Heap / Arcs | Clone today (string state, *estimated*) | Clone in the core |
|---|---:|---|---|---|
| `Meta` + `Pending` + scratch + marks | ~700 | `marks`: `SortedMap` of small enums | ~150 allocations | memcpy |
| 6 players | 6 x 8 | 6 Arcs (0.5 KB each behind them) | ~6 x 80 allocations | 6 refcount bumps |
| board | 0 | 25 Arcs (17 KB behind them) | ~300 allocations (one `Vec<Unit>` + one `BTreeMap` node per touched system/planet; every `Unit` has 2 `Arc<str>`) | 25 bumps |
| decks, cards, objectives, strategy, explore, trade | ~600 | ~14 Arcs | ~200 allocations | ~14 bumps |
| logs (events, dice, ledger, histories) | 6 x 16 | 6 Arcs | whole-log copy (the checkpoint cost, 17.7% of a game, *copied*) | 6 bumps |
| `Rng` | ~1.7 KB (12 streams) | - | 12 `String` keys + 12 nodes | memcpy |
| **Total** | **~3 KB** | **~65 Arcs** | **~800+ allocations, ~30-60 µs** (the estimate `ENGINE_CORE.md` 1.6 gives for a 37 KiB state) | **~65 bumps + 3 KB memcpy, est. 150-300 ns** |

A checkpoint (`Resolver::checkpoint`, 10% inclusive, `DecisionRecord::clone` 5.9% in the profile, *copied*)
becomes a clone plus the `Log` tail. Targets measured by F3 on the golden positions: `CoreState::clone` <= 300 ns
and zero heap allocations apart from the log tail; `size_of::<CoreState>()` <= 4 KB; `size_of::<Player>()` <= 600 B.

### 1.7 What is not in `CoreState`

The content store and all text; `Table` (seat names / lobby); the decision log and prompts; `GameRng`'s
`observation`/`sync` hooks; `GameError` text (typed `Error` in the core, texts in the adapter);
`aftermath_observer`, `round_income_*` experiment knobs and similar harness fields (they stay in `Game`, passed
into the core as a `Knobs` argument where a rule reads them); caches (`SeatReach`, adjacency, memo maps).

### 1.8 Every `String`-typed field of `GameState` and its typed home

| Field (`state.rs`) | Typed home | Codec rule |
|---|---|---|
| `pending: Option<String>` | `Pending` (stack of `Window`) | the strings are the window names; table in adapter |
| `exploration_decks: BTreeMap<String, Vec<String>>` | `Decks.explore: [Deck<Exploration>; 4]` keyed by `Trait` | key text = trait name; card text = `Exploration` id |
| `planet_attachments: BTreeMap<PlanetId, Vec<String>>` | `PlanetCell.attach` | |
| `agenda_deck: Vec<String>` (also `agenda_in_play`, `agenda_choices`, `agenda_veto_replacement`, `agenda_votes: BTreeMap<PlayerId,String>`) | `Deck<Agenda>`, `Option<Agenda>`, `SmallList<Outcome,..>`, `[Option<Vote>; 8]` | `Outcome` enum (For/Against/Player(Seat)/Planet/..) with text codec |
| `laws: BTreeMap<String,String>` | `Laws`: `SortedMap<Agenda, Outcome, 16>` | alias text <-> `Agenda`, outcome text <-> `Outcome` |
| `faction_marks: BTreeMap<String,String>` (367 uses, `format!` keys) | `Marks`: per-faction records + a generic `SortedMap<MarkKey, MarkVal, 32>` for the long tail; `MarkKey` enum per family, numbers as `DecimalTextOrd` | codec must reproduce key text, value text and **iteration order**; corpus = every key seen in the 20 golden games and the fixtures (package X1) |
| `expedition_slices: BTreeMap<String, PlayerId>`, `wormhole_tokens: BTreeMap<String, SystemId>`, `ion_storm: Option<(SystemId, String)>`, `galactic_event: Option<String>` | `[Option<Seat>; 4]`, `SortedMap<WormholeKind, System, 5>`, `Option<(System, WormholeKind)>`, `Option<GalacticEvent>` | |
| `vp_ledger` reason, `pending_destructions` reason, `event_feats` | `LedgerReason`, `DestroyReason`, `Feat` | |
| `relic_fragments: BTreeMap<String,i32>`, `exploration_cards: Vec<String>`, `plots: Vec<String>`, `assimilated_technologies: BTreeMap<String,TechnologyId>` | `[u8;4]` by trait, `SmallList<Exploration,4>`, `SmallList<Plot,..>`, `SortedMap<Technology,Technology,2>` | |
| `exploration_log`, `agenda_log`, `agenda_casts`, `relic_log` records | typed records in `Log<..>` | |

Package X1 turns this table into code (text <-> typed pure functions with a corpus test) before any state
exists, so F2 can use them.

---

## 2. The decision / option model

### 2.1 Types (core, `choice/*.rs`)

```rust
pub struct Choice { pub actor: Seat, pub ask: Ask, pub ctx: Ctx, pub opts: Opts }     // what a decision point is
pub struct Opts(SmallList<Opt, 16>);                                                   // order is observable: today's order
pub enum Ask { Strategy(strategy::Ask), Tactical(tactical::Ask), Combat(combat::Ask), Invasion(invasion::Ask),
               Production(production::Ask), Reaction(reaction::Ask), Agenda(agenda::Ask), Vote(vote::Ask), ... /* ~28 areas */ }
pub enum Opt { Decline, Done, Strategy(strategy::Opt), Tactical(tactical::Opt), Combat(combat::Opt), ... }
pub struct Ctx { source: Source, subtype: Subtype, optional: bool, target: Option<Target>, outstanding: SmallList<Owed, 4>,
                 space_battle: bool, invasion_seq: Option<u64>, /* display-only parts: trigger, hit, card previews */ }
pub enum Pick { Opt(Opt), Number(i32), Amount(..) }                                    // an answer; `free_form` becomes `Number`/`Amount`
```

The name is `Opt`, not `Option`. Each area owns its sub-enums in its own file (`choice/<area>.rs`); the
top-level `Ask`/`Opt` have one variant per area, fixed in F4, so adding an option kind to an area edits only
that area's file. Variants carry the data the option needs to be applied (`Move { boost, origin, index, route:
Option<u8> }`, `Exhaust { planet }`, `Strategic { card }`), not text.

### 2.2 Options are derived

`Pending` stores *what is asked* (`Ask` with its parameters). The options are `fn opts(&CoreState, &Ask) -> Opts`
(pure, deterministic, in today's order). Consequences: states stay small and cheap to clone; a restored state
asks the same question with the same options; the adapter and the preview code can call `opts` on a cloned
state (a clone is ~65 bumps) to compute `preview` instead of the string engine's dedicated preview paths;
`Opt` values are used only for the current decision, never stored in the state.

Engine API:

```rust
pub fn ask(state: &CoreState) -> Option<Choice>;                              // None when the game is finished
pub fn answer(state: &mut CoreState, pick: &Pick, k: &Kernel) -> Result<Step, Error>;   // `Kernel` = hooks + content tables + rules knobs
pub enum Step { Asked, Finished, Emitted(SmallList<EventRec, 4>) }              // events are also appended to `ledger.events`
```

### 2.3 Mapping to today's strings (adapter, `ti4-engine/src/render/<area>.rs`)

One module per area implements the same trait; the registry is a `match` over the top-level `Ask`/`Opt`:

```rust
pub trait Family {
    type Ask; type Opt;
    fn prompt(r: &Render, a: &Self::Ask) -> String;                         // byte-identical prompt text (hashed in V2)
    fn context(r: &Render, a: &Self::Ask, c: &Ctx) -> DecisionContext;      // subtype text from `Subtype::as_str`, never from the prompt
    fn option(r: &Render, a: &Self::Ask, o: &Self::Opt) -> ChoiceOption;    // id, kind, label, payload (BTreeMap<String,Value>), details
    fn parse(r: &Render, a: &Self::Ask, id: &str) -> Option<Self::Opt>;     // inverse of `option(..).id`; includes the `|r<k>` route suffix
}
```

| Today | Core | Adapter (render) | Parse |
|---|---|---|---|
| id `decline`, kind `decline`, label `Decline` | `Opt::Decline` | the constants of `choice.rs` | exact match |
| `{verb}|{origin}|{index}` and route answers `{option_id}|r{k}` | `tactical::Opt::Move { boost, origin, index, route: Option<u8> }` | `format!` of the same pieces; `|r{k}` only when `route` is `Some`; `details.routes` rebuilt from `ask` | `rsplit_once("|r")`, then split; ids not parsed -> same `GameError` text as today |
| `strategic|{card}` | `strategy::Opt::Strategic { card: StrategyCard }` | card text from the generator table | |
| `exhaust|<planet>` and payload (`planet`, `resources`, `influence`) | `Opt::Exhaust { planet }` | payload built with the same `Value::from(i32/f64/&str)` types (JSON number type matters for byte equality) | |
| label text (card/planet/system names, counts) | none | display tables from the content store | never parsed back |
| prompt text, incl. card names | `Ask` variant + parameters | `prompt()` | |
| `DecisionContext` (`subtype: String`, source, phase, round, optional, target, outstanding, `space_battle`, `invasion_seq`, display-only trigger/hit/previews) | `Ctx` + `Subtype` enum | `Subtype::as_str` is a fixed table; display-only parts built from `ask` | |
| `free_form` choice | `Pick::Number` / `Pick::Amount` | `free_form` flag per `Ask` | numeric parse as today |
| `ChoiceOption.preview` (skip-serialised) | derived by cloning the state and applying | adapter | |

**Fingerprint V2 stays identical** because `DecisionRecord { player, prompt, chosen, offered, context }` is
produced by the adapter from rendered strings and `fingerprint::decision_hash` is untouched. The only ways to
break it: a different option *order*, a different prompt or id character, a different context field. The model
makes each of them a tested property:

* **Order**: the typed `opts()` of an area is written in the same sequence as the string builder; when the
  string builder sorts by id text, the typed code iterates in enum order (same by the numbering rule); when it
  uses insertion order, the typed code inserts in the same order. Test: per family, `render(opts(state)) ==`
  string engine's `Choice.options` (ids, kinds, labels, payload JSON) at every decision of the corpus.
* **Corpus** (package X2/F4): every decision of the 20 golden games, the template openings, the engine's
  fixtures and the six bench configurations is recorded once as `(state hash, rendered Choice JSON, V1 hash,
  V2 hash)` under `tools/engine_core/decision_corpus/`. Round trip test: `parse(option(o).id) == o` for every
  option of every recorded decision; `render == recorded` for every decision once the family is on the core.
* **Unknown ids**: `Opt` has no catch-all; the adapter's `parse` returns `None` and the caller produces today's
  error text, so replays of damaged logs fail the same way.

### 2.4 What the bots see

Unchanged. `Observed`, `ChoiceOption`, `Choice.prompt` stay string-typed on the adapter side for the whole
program; moving the rule bot to typed options (BOT_STRING_AUDIT items 7-8, est. 2-4% CPU, removes ~120 parse
sites) is a **later, separate project** (D3), because it changes bot code, not the engine. The only bot-visible
change is speed.

---

## 3. State sync during the migration

### 3.1 The three shapes, compared

| | (P) per-position conversion | (O) incremental ownership of `GameState` fields | (L) **lend with footprints** (chosen), flip at the driver |
|---|---|---|---|
| Idea | Convert the whole `GameState` to `CoreState` for every ported call (or once per position) and back | Move fields out of `GameState` into the core one by one; `GameState` keeps accessors | `GameState` is authoritative between calls; a ported call declares a footprint (slices read, slices written), the adapter loads exactly those slices, runs the core, stores the written slices back. Consecutive ported calls share one *region*. At the flip the authority reverses |
| Cost per call | 20-60 µs (est., full 37 KiB state) | none for owned fields | proportional to the footprint: a unit list and a few planets for a combat round (est. 1-5 µs) |
| Finds string-side mutations? | not needed (always reconverts) | impossible to miss (single owner) but `state.players[i].trade_goods += 1` is written directly in ~thousands of places in 18.5 k lines of `game.rs` etc. and in 177 host mentions: every access would first have to be encapsulated | not needed: string code runs only *between* regions; a region reloads what it needs |
| Hot loops | fine inside a ported area, hopeless as a per-query cost (the bot asks ~10^6 queries per bench) | best | read-only areas use **one typed view per position** (`CoreView`, cached in `Observed`, as `Seats::of` + `board_from_state` already are: 20-60 µs once per position) |
| Independence of packages | yes | no: a field has one owner, all its users must be ported or encapsulated first | **yes**: each package declares its own footprint and owns its slices' codecs |
| Oracle | yes | hard: there is no second copy to compare against | **yes**: run string and core on two copies of the state and compare |
| Risk | slow, not wrong | big-bang encapsulation refactor on a branch with 243 commits / 14 days of churn | two codecs per slice must stay total and round-trip (tested over every golden position) |
| Checkpoint gain | none until the flip | partial | none until the flip (the string-side fix X4 gives part of it now) |

**Chosen: (L).** Reasons: it is the only shape in which every area can be ported, dual-run and merged
independently while the rest of the 134 k-line engine keeps working, and it converges on ownership: the
region grows as neighbours are ported, so conversion cost falls monotonically; the flip then costs nothing
because both codecs already exist and are proven by the round-trip tests. The price is paid on the early,
small packages (their regions are tiny and their lend cost can exceed their gain), so early packages are
chosen to be **read-only queries (use the cached per-position view)** or **large self-contained mutators
(combat round, ground round, production) where the work inside dwarfs the lend**.

### 3.2 The mechanism (foundation F2; adapter file `ti4-engine/src/core_sync/`)

```rust
pub enum Slice { Meta, Seats, Players, Board, Map, Decks, Cards, Objectives, Agenda, Strategy, Explore, Trade, Marks, Transient, Pending, Rng, Ledger }
pub struct Footprint { pub reads: SliceSet, pub writes: SliceSet }          // declared per ported entry point, `const`
pub struct Lend<'g> { core: CoreState, loaded: SliceSet, game: &'g mut GameState, .. }

impl Codec {                                                                  // one file per slice: core_sync/codec/<slice>.rs, owned by that slice's owner
    fn load(state: &GameState, seats: &Seats, galaxy: &Galaxy, slices: SliceSet) -> CoreState;   // other slices stay default, flagged unloaded
    fn store(core: &CoreState, state: &mut GameState, seats: &Seats, slices: SliceSet);
}
// use at a call site (string module, one line):  game.with_core(FOOTPRINT, |core, k| ti4_engine_core::rules::combat::roll_round(core, k, args))
```

* `store` writes a slice through the **existing `GameState` structs and serde attributes**, so field presence,
  `skip_serializing_if`, map-key order and numeric types are the string engine's own: JSON bytes cannot differ
  because the JSON is still produced by the old serialiser from an old `GameState`.
* Unloaded slices are guarded: every slice accessor does `debug_assert!(loaded.has(slice))`, so a package that
  forgot a slice in its footprint fails in debug and in the dual run, not silently in production.
* A **region** (`game.region(FOOTPRINT, |g| { .. })`) loads once and runs several ported calls; calls inside a
  region must be ported areas only. Regions merge as the boundary moves outward. Windows that live in private
  `Game` fields (`tactical`, `aftermath`, ...) are passed as typed **arguments** by the string caller
  (`lend_args`, owned by the area) until the driver is ported.
* **Read-only areas** (wave A queries) get `CoreView` = `load(.., reads)` once per position inside `Observed`
  (like `Seats::of` now), never per query.
* **After the flip** the same mechanism runs backwards: `Game` owns `CoreState`; leftover string modules call
  `game.with_string(FOOTPRINT, |gs| ..)` which materializes the footprint into a `GameState`, runs the old
  function, and loads the written slices back. The host/server/views call `game.materialize()` (est. 20-60 µs,
  vs ~2.2 ms per viewer for the views, *copied*).

### 3.3 The dual run (foundation F3; adapter `ti4-engine/src/dual.rs`, harness `crates/ti4-engine-core-bridge`)

Three runtime backends per area, set by `AreaMask` (bit per area; default from the cargo feature, overridable in
tests and bins): `String`, `Core`, `Dual`. In `Dual` the call site runs the old implementation on one copy of
the `GameState` and the lent core implementation on another, then compares, then **returns the string result**
(so a bug in the port cannot change the game under test):

1. **State**, field by field: `serde_json::to_value(&string_state)` vs the same for the written-back state. A
   differing leaf is reported with its path (`players[2].technologies[3]`, `board.38.units[1].owner`) and both
   values. Serialised bytes (`serde_json::to_vec`) are compared last as the byte-exact check; this includes the
   fields the engine's `PartialEq` marks "not compared" (they still reach saves and views).
2. **Returned value / error**: `Result` equality; error text equality.
3. **Decisions**: if the call opens a decision, the string `Choice` and `render(core ask)` are compared
   (ids, kinds, labels, payload JSON, prompt, context incl. `canonical()`), and the V1 and V2 `decision_hash`
   of a `DecisionRecord` built from each are compared. If the call answers one, the same for the answer.
4. **Events**: the ordered `Event` list (`event_hash` V1/V2) and the rendered event text.
5. **Randomness**: `GameRng::positions()` of both sides (every stream, word position) -- equal position means
   the same number of draws in the same order; plus the dice history.
6. **Cost**: lend size (slices, bytes) and times, to find regions that should merge.

Where it runs:

| Run | What | Gate use |
|---|---|---|
| inline, in every test | the engine's own suite (3 918 tests *copied*) with the area on `Dual` and `fail_fast` | every PR of a package |
| inline, in a game | 20 golden games (`profile-20.json`) and the template openings played with `Dual` for the area under port | every PR; nightly |
| bench | `bench`, `bench-v1..v5` with `Dual` (slow, ~2-3x): the old rule versions (1-5, 23) exercise the gated branches | before a package's deletion commit |
| offline positions | extension of `core_equivalence`: every 10th position of the golden games asked of both, for query areas | wave A |
| corpus | decision corpus replay (2.3) | every package with decisions |

A mismatch in the nightly (a late fix on the string side) shows up as a `Dual` report, which is the guard
against the drift problem of `ENGINE_CORE.md` 3.3.

### 3.4 Why not "per-position conversion" for everything

It would make the dual run trivially correct but it prices every ported mutator call at a full conversion,
and gives no path to ownership. It remains exactly right for the read-only query areas (one `CoreView` per
position), which is how stages 1-5 already work.

---

## 4. Layering, work breakdown, order

### 4.1 Layers of the core (the cycle is broken by construction)

```text
L6  ti4-engine-core-factions   faction modules: fill the hook table (one file per faction); depends on L0-L5
L5  driver                     step loop, phases, windows dispatch, setup; depends on L0-L4
L4  rule areas                 combat, invasion, production, tactical, cards, leaders, ... (each: opts + apply)
L3  kernel                     events/triggers/reactions registry, hook table DEFINITIONS (typed fn signatures), Fx primitives
L2  queries                    pure reads over &CoreState: resources, supply, fleet, movement, tech, laws, objectives progress
L1  state + decision types     CoreState, containers, Rng, Choice/Ask/Opt/Ctx (data only), Event
L0  data                       generated enums + tables, bitsets, hex, galaxy, adjacency, route search (exists)
```

Rules (enforced by `tools/engine_core/check_layers.py`, a CI check): a module may import only from lower
layers; **L4 areas may not import each other**: they talk through `Next` values (see below); L3 defines hooks
but never imports L6; `unsafe` is forbidden; no `String`/`&str`/`serde`/`BTreeMap`/`format!` in non-test code
(`lint_core.sh`).

**How the L4 peers (65 module pairs today) stop calling each other.** The string engine is already
window-oriented (it keeps `tactical`, `aftermath`, `voting`, ... windows in `Game`). In the core, an area never
calls another area's resolver; it returns `Next`:

```rust
pub enum Next { Done, Open(Window), Emit(Event), Continue(Ask), Chain(SmallList<Next, 3>) }
```

e.g. an action card that "produces 1 unit" returns `Next::Open(Window::Production{..})`; the L5 driver
dispatches the window to `production::apply`. Pure helpers shared by several areas (placing a unit, gaining
a trade good, exhausting a planet, scoring) are **L3 primitives** (`kernel::fx`) which also fire triggers.

### 4.2 The cycle analysis (measured)

`tools/engine_profile/module_graph.py` on `ti4-engine/src` (this commit): **77 modules, 631 edges, one SCC of
56 modules**; 133 929 non-test lines in `ti4-engine/src` (the "283 k" figure counts the tests: 28.9 k in test
directories and ~120 k in inline `mod tests`); of the 133.9 k, 10.8 k are generated tables and 42.0 k are
`factions`, leaving ~81 k hand-written outside factions. Fan-in hubs: `choice` 43, `factions` 38,
`decision_context` 36, `supply` 28, `technology` 26. Fan-out: `game` 46, `factions` 40.

I assigned every module to a layer (tiers below) and counted the references that point *up* or sideways
inside L4 (my script, reproducible by `check_layers.py` from this table):

| Tier | Modules |
|---|---|
| L1 | rng dice deck event trigger fleet_strength neutral_units coexistence decision_context observation generated, **and the data types of `choice`** |
| L2 | planets payment synergy valuation tokens supply fleet seating laws movement transit map_feasibility starting_tech preview reach_query space_stations agenda breakthroughs entropic_scars |
| L3 | timing reactions reaction_preview registry infantry_ii magen_defense faction_abilities faction_techs |
| L4a | technology exploration relics promissory objectives secrets transactions diplomacy legendary thunders_edge fracture |
| L4b | production combat invasion tactical action_cards leaders strategy_cards strategy secondary_preview agenda_effects vote status draft setup opening |
| L5 | game phase wiring |
| L6 | factions |
| adapter | the builder/rendering half of `choice` (41 modules depend on it today), core_adapter, fingerprint, fixtures |

Result: **3 205 module references** between non-adapter modules, of which **180 module pairs (971 references,
30%) break the layering**: **35 pairs / 435 refs point up into `factions`** (L4b->factions 11 pairs / 289 refs,
L2->factions 11 / 52, L4a->factions 8 / 44, L3->factions 3 / 23, L5->factions 2 / 27); **65 pairs / 330 refs are
L4 peers** (L4a 30 / 145, L4b 35 / 185) and 18 pairs / 71 refs go from L4a up to L4b; **57 pairs / 129 refs
point from L2/L3 up into L3/L4** (e.g. `starting_tech->technology` 10, `reactions->action_cards` 13,
`laws->production` 5, `supply->timing` 8); 4 pairs / 5 refs L4b->L5 and 1 L1->L2. The 41 modules' dependency
on `choice` disappears because `Choice/Opt` become L1 data. The cuts:

1. `factions` becomes L6 and is reached only through the **hook table** defined in L3 (`hooks.rs`: a struct of
   typed function pointers per hook point, indexed by `Faction`; the 40 modules' calls into `factions::` become
   `k.hooks.movement_pass(faction, ..)`). This removes 435 of the 971 references.
2. L2/L3 -> L3/L4 edges (57 pairs) are mostly *data lookups placed in the wrong module* (e.g. `technology` owning
   prerequisite queries): move the query half down to L2 (`query/tech.rs`), keep the mutating half in L4.
3. L4 peers use `Next` as above (65 + 18 pairs); the ~20 genuinely shared helpers become L3 primitives.

### 4.3 Ordering contract and layout of the new code (so packages do not collide)

```text
crates/ti4-engine-core/src/
  (existing L0: ids, bits, hex, galaxy, movement, board(old name: occupancy), tables, generated/)
  containers/   set, small_list, sorted_map, cow, chunked, log, deck, opt32          [F1]
  state/        mod, meta, seats, players/{econ,tech,cards,leaders,timers,misc}, board, map, decks, cards,
                objectives, agenda, strategy, explore, trade, marks, transient, pending, rng, ledger   [F1 creates all; each file then owned by its slice owner, table 4.6]
  choice/       mod (Ask/Opt/Ctx/Pick skeleton) [F4], <area>.rs [area owner]
  kernel/       events, triggers, reactions, hooks, fx  [A5]
  query/<area>.rs     [area owner]
  rules/<area>.rs     [area owner]
  driver/       [D1]
crates/ti4-engine-core-factions/src/<faction>.rs                                          [C shards]
crates/ti4-engine/src/core_sync/{mod,footprint,lend}.rs  [F2]   core_sync/codec/<slice>.rs [slice owner]
crates/ti4-engine/src/render/<area>.rs                                                   [area owner]
crates/ti4-engine/src/dual.rs                                                           [F3]
tools/engine_core/{verify_package.sh, check_layers.py, lint_core.sh, state_size.rs, decision_corpus/}   [F3, X2]
```

**State change protocol** (the contention point): a package adds fields only inside the slice files it owns,
plus the codec file of the same slice, plus the round-trip test; it never edits another owner's slice file. A
field needed by two packages lives in the slice of the package that *mutates* it; the reader goes through that
slice's accessor. New slices and new top-level `Ask`/`Opt` variants go through the foundation steward (one
named agent, answers within the day). Because `Player` and `Pending` are split into per-owner sub-structs
(1.4), most edits are file-disjoint.

**Branches and merging.** Packages branch from the integration branch `core-state-main` (created from this
branch after the user approves the design) and merge back there; `core-state-main` is rebased onto the live
line every day (memory: "reconsolidate before push"). Merge fast (days, not weeks): while a package is open,
its string modules are frozen against unrelated edits (policy in 6.5); a late string-side fix is ported by the
package owner and caught by the dual run.

### 4.4 Packages (summary table)

Size = lines of string code the package replaces (non-test, from the module graph); the core gets 0.6-0.8x
of that plus the adapter renderers. Estimates in agent-days (one agent, one worktree, including its dual-run
chasing); ranges are +-30-50%. Payoff = estimated share of a golden bot game (copied bases: `BOT_STRING_AUDIT`
buckets), **not additive**.

| Pkg | Name | Area (string modules) | Lines | Depends on | Risk | Payoff (est.) | Days |
|---|---|---|---:|---|---|---|---:|
| **F1** | state-types | containers, all state slices, `Seat`, `Rng`, `Ledger`, `Pending`, codecs' type targets, layer skeleton | new ~7 k | - | M | enables; measures size/clone | 4-5 |
| **F2** | core-sync | `Codec` per slice, `Footprint`/`Lend`/`region`, round-trip tests over golden positions | new ~5 k | F1 (slice by slice) | M-H | - | 5-6 |
| **F3** | dual-harness | `dual!`, diff reporter, `AreaMask`, `verify_package.sh`, `check_layers.py`, `lint_core.sh`, `state_size`, bench wiring | new ~3 k | none for the diff reporter; F1 for the rest | L | - | 3-4 |
| **F4** | decision-model | `Choice/Ask/Opt/Ctx/Pick`, `Family` trait + registry, corpus replay harness, generic families (decline, done, numbers) | new ~3 k | F1 (ids), X2 | M | - | 3-4 |
| X1 | key-codecs | typed <-> text for `faction_marks`, laws, ledger reasons, deck keys, `DecimalTextOrd` | new ~2 k | generated enums only | L | - | 3 |
| X2 | decision-corpus | inventory of the 736 option sites into families + recorded decision corpus + grammar table | tools ~1.5 k | none | L | - | 3 |
| X3 | gen-text-enums | generator: `NAME_RANK`, `Timing` (action-card window), agenda `elects`, leader `timing`, strategy-card kinds | gen ~1 k | none (card-generator merged) | L | content lookups -3..-5% | 3 |
| X4 | checkpoint-cow-string | checkpoints/records without whole-log copies **on the string state** (ENGINE_OPTIMIZATION #8) | ~1 k | none | M | **-8..-12%** | 4 |
| A1 | board-queries | planets 251, supply (query half) ~600, tokens 581, payment 192, fleet 758, fleet_strength, seating 967, laws (query half) ~500, coexistence, neutral_units, space_stations | ~5 k | F1-F3 | L | board/control reads, enables A4 | 6 |
| A2 | movement-tactical | movement 1210, transit 973, tactical 1309, reach_query, opening, `core_adapter::SeatReach`, movement hook table; deletes `string-movement` | ~4.7 k | F1-F3, A5 interfaces | M | reach bucket 3.6% -> ~1.5% | 6 |
| A3 | technology | technology 1794, starting_tech 263, faction_techs 546, synergy 150, breakthroughs 270, entropic_scars 260 | ~3.3 k | F1-F3, A1 | M | -3..-5% | 5 |
| A4 | objectives | objectives 2494, secrets 1295, valuation 271 (read-only progress/scoring predicates) | ~4.1 k | F1-F3, A1, X3 | M | **-8..-12%** (objective eval is 20% of a game) | 7 |
| A5 | kernel | timing 1429, reactions 1293, reaction_preview 576, registry 184, infantry_ii, magen_defense, trigger, event, hook table | ~4.5 k | F1, F4 | **H** | enables all B | 8 |
| B1 | combat | combat 6643, dice (+ rolls) | 6.6 k | A1, A3, A5 | H | part of -15..-20% for B | 9 |
| B2 | invasion | invasion 4957, magen_defense follow-ups | 5 k | B1 interface | H | (B) | 7 |
| B3 | production | production 3863, payment mutations, supply mutations | 4.5 k | A1, A3, A5 | M-H | option generation (production::build_options 237 lines) | 6 |
| B4 | trade-diplomacy | transactions 2013, diplomacy 3716 | 5.7 k | A1, A5 | M | | 7 |
| B5 | exploration-relics | exploration 1652, relics 1278, legendary 1095, fracture 509, thunders_edge 625, promissory 823 | 6 k | A1, A3, A5 | M | | 9 |
| B6 | action-cards | action_cards 7780 (142 cards) | 7.8 k | B1, B3, B5 interfaces | H | | 10 |
| B7 | leaders-abilities | leaders 2168, faction_abilities 1142 | 3.3 k | A5, B3 | M | | 5 |
| B8 | strategy | strategy_cards 2795, strategy 973, secondary_preview 414, draft 117, status 294 | 4.6 k | A1, A4, B3 | M | | 8 |
| B9 | agenda-laws | agenda_effects 1370, vote 1217, agenda 116, laws (mutating half) | 3.2 k | A4, A5, B8 | M | | 5 |
| C1-C6 | faction shards | `factions/*` 41 951 lines in 6 shards (alphabetical by file size to balance, ~7 k each) | 42 k | A5 hooks + the B packages whose hooks a faction uses | M | | 2-3 each (14) |
| D1 | driver | game 7408, phase 252, wiring 744, setup 205, fixtures; `Pending` windows | 8.6 k | all B, hooks for C | **H** | automatic steps 18% -> ~6% | 10 |
| D2 | flip-and-freeze | authority flip, `materialize`, reverse lend for leftovers, MOVE (not delete) the string modules per area into `ti4-engine-legacy`, remove legacy and `core-dual` wiring from production builds, checkpoints = `CoreState::clone` | -50 k from production (moved to `ti4-engine-legacy`) | D1, C | H | checkpoints 18.7% -> ~2% | 6 |
| D3 | typed-bots (optional, later) | bots on `CoreView` / typed options | - | D2 | - | -2..-4% | - |

### 4.5 Interfaces each package must expose first ("interface PR", merged within 1-2 days of start)

An interface PR is signatures + doc comments + `todo!()` bodies + the codec/render stubs, so dependants can
compile and be reviewed against it before the implementation lands.

| Pkg | Interface (exact exports) |
|---|---|
| F1 | all of `state::*`, `containers::*`, `Seat`, `Idx`; **freeze order**: Meta/Seats/Players/Board in the first 2 days, the rest by day 4 |
| F2 | `Slice`, `SliceSet`, `Footprint`, `GameState`-side `with_core`/`region`/`CoreView::of(&GameState, ..)`, `trait SliceCodec { load, store }` |
| F3 | `dual!(area, string_expr, core_expr)`, `AreaMask`, `Area` enum (one variant per package area), `DualReport`, `verify_package.sh AREA` |
| F4 | `Ask`, `Opt`, `Ctx`, `Pick`, `Choice`, `trait Family`, per-area empty sub-enums, `render::register` |
| A1 | `query::{control, resources, influence, supply_cap, fleet_count, neighbours, seat_order}` over `&CoreState`/`CoreView` |
| A2 | `query::reach::{movable_into, path, routes, can_reach}`, `hooks::movement::*` signatures |
| A3 | `query::tech::{owned, researchable, prereqs_met, unit_stats, active_aliases}` (returns `TechSet`/rows) |
| A4 | `query::objectives::{progress, qualifies, values_imagining}`, `rules::objectives::{score, reveal}` |
| A5 | `kernel::{emit, open, Trigger, Window, Next, fx::*}`, `hooks::HookTable` with every hook point named (the 40 modules' call sites enumerated) |
| B1 | `rules::combat::{open, roll_round, assign_hits, retreat, end}`, `combat::Ask/Opt`, `ground_round(..)` for B2 |
| B3 | `rules::production::{open, cost, place}` for B6/B7/B8 |
| B5 | `rules::{explore, relic, promissory}::*` entry points used by cards/agenda |
| D1 | `driver::{step, answer, ask}`; windows dispatch table |

### 4.6 Slice ownership (file-disjointness)

| Slice / file | Owner | Others edit it? |
|---|---|---|
| `containers/*`, `state/{meta,seats,rng,ledger,pending,mod}` | F1 (steward) | via steward only |
| `state/board`, `state/player/{econ,misc}`, coexist/neutral | A1 | no |
| `state/map` | A2 | no |
| `state/player/tech`, tech marks | A3 | no |
| `state/objectives` | A4 | no |
| `state/{decks,cards}`, `player/cards`, `explore` | B5 (decks shared: B6 reads) | via B5 |
| `state/player/timers` -> split by area: `combat_timers` B1, `production_timers` B3, `card_timers` B6 | respective | no |
| `state/{strategy,agenda}` | B8 / B9 | no |
| `state/trade` | B4 | no |
| `state/marks` | per-faction record in C shard, key families in X1 | no |
| `state/transient` | split like timers (B1: `last_*` combat slots; B2: invasion; B3: production counters) | no |

### 4.7 Merge order and what can start in parallel

```text
day 0   start together:  F1, F3 (diff reporter and scripts need no core types), X1, X2, X3, X4
day 2-4 F1 freezes slices in order Meta/Seats/Players/Board -> F2 and F4 start on the frozen slices
day 4-5 F1 done; F2 and F4 continue; A5 starts its interface PR (needs F1 + F4 skeleton)
day ~10 F2, F3, F4 merged  =>  FOUNDATION DONE (gate: round trip over all golden positions; dual harness green on the movement area that
        already exists, migrated to the new Footprint API as the proof)
wave A  (days 10-20, parallel, disjoint files):  A1, A3, A4, A2, A5
wave B  (days 18-45, parallel where the interface rule allows):
        B1 -> B2 (B2 merges after B1);   B3;  B4;  B5;  B6 (starts on interfaces of B1/B3/B5, merges last of the cards);  B7;  B8 -> B9
wave C  (days 35-55): shards C1-C6, each when the hooks its factions use are merged (the hooks used by movement land with A2)
wave D  (days 50-65): D1 (needs every window kind), then D2 (flip, delete, measure)
```

Merge order (strict dependency order, otherwise first-ready): F3, F1, X1/X2/X3/X4 (any), F2, F4, A5, A1, A3,
A2, A4, B1, B3, B4, B5, B2, B7, B8, B6, B9, C shards, D1, D2. Rule: **a package merges only after its
dependencies' gates passed on the integration branch**, and after rebasing onto it.

What starts in parallel with the foundation F1: F3 (harness), X1 (key codecs: pure text<->enum functions with
a corpus), X2 (decision inventory and corpus recording from the existing string engine), X3 (generator enums
and rank tables), X4 (checkpoint COW on the string state, a payoff that does not wait for the core). Nothing
that touches `CoreState` starts before F1's first freeze.

### 4.8 Why this order

* **Queries first (A1-A4)**: read-only, merge with one cached view, easy oracle, and A4 is the single largest
  bot cost (objective evaluation 20% of a game, *copied*); A3 removes content lookups (9.3%).
* **Kernel next (A5)**: every mutating area opens windows and fires triggers through it; high risk, so it
  starts early and runs in parallel with the queries.
* **Rule areas (B)** bottom-up by the layering: combat before invasion and cards; production before cards,
  leaders, strategy; the cards package is last because it calls the others (through `Next`).
* **Factions (C)** after their hook points exist; mostly mechanical per faction, so 6 agents at once.
* **Driver and flip last** because the checkpoint gain is only real when `CoreState` is the state.

---

## 5. Gating policy (pure refactor)

Invariants: `RULES_VERSION` and every `RULES_*` constant unchanged (values and definition in
`ti4-model/src/state.rs`; a package that needs them in the core re-exports, it does not redefine);
`GameState` serde output unchanged; `ts-rs` types and wire fixtures unchanged; `DecisionRecord`/fingerprint
V1 and V2 unchanged; `docs/RULES_VERSION.md` unchanged; no behaviour fix inside a package (a bug found by the
dual run is reported and fixed separately on the string side first, through the normal baseline process).

**Per-package acceptance gate** (`tools/engine_core/verify_package.sh AREA OUT BASE_SELFPLAY`, extending
`verify_stage.sh`; every line must pass before merge into `core-state-main`):

| # | Check | Pass criterion |
|---|---|---|
| 1 | Unit and integration tests | `cargo test -p ti4-model -p ti4-content -p ti4-engine -p ti4-host -p ti4-rule-bot -p ti4-selfplay -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine-core-bridge -p xtask`, `cargo test -p ti4-wasm --features ti4-wasm/ts`, `cargo xtask gen-cards --check`: all green; no test edited except to add (a changed assertion needs the user) |
| 2 | Immutable surfaces | `git diff --exit-code <base> -- web/ crates/ti4-wasm/**/fixtures docs/RULES_VERSION.md crates/ti4-model/src/state.rs` (except additive comments) and generated TS: empty |
| 3 | Dual run | area on `Dual`: engine suite + 20 golden games + template openings + corpus replay: **0 differences**, with counts printed (calls compared, decisions, hashes) |
| 4 | Round trip | `load(store(x)) == x` and `store(load(g)) == g` (serialised bytes) for the package's slices over every golden position (>= 3 000 positions) and the fixtures |
| 5 | Benches | `bench`, `bench-v1..v5`: **200/200 identical** with the area on `Core`; `bench` 200/200 with it `String`; for packages that touch a gated rule, also `Dual` once (before the deletion commit) |
| 6 | Golden hashes | 20/20 `profile-20.json` hashes equal `tools/rulebot/baseline.json` |
| 7 | CPU A/B | 20 golden games, 3 interleaved runs, 4 workers (`tools/engine_profile/ab_time.sh`): median CPU **not worse than +1.5%** vs the previous merge commit with the area on `Core`, and the expected gain stated in 4.4 reported; the lend-region report (3.3 item 6) attached |
| 8 | wasm size | `wasm-release` + `wasm-opt` as `build-wasm.mjs`: raw, gzip, brotli before/after reported; budget in 6.3 |
| 9 | Core hygiene | `lint_core.sh` (no `String`/`&str`/`serde`/`BTree*`/`format!` in non-test core code, no `unsafe`), `check_layers.py` (no upward or L4-peer import), clippy workspace lints clean |
| 10 | Allocation check (packages that touch hot state) | `state_size` bench: `CoreState::clone` and the package's hot function report ns and allocations; no regression vs the previous merge |
| 11 | Compile time | release `ti4-engine` and `ti4-engine-core` build times reported (one run each, shared machine); regression > 10% needs a note |

**Deletion commit** (separate, after one clean nightly with the area on `Core`): removes the area's string
implementation and its `AreaMask` bit; gate: 1-3, 5, 6, 8 again. Revertable as one commit.

**Foundation gate** (F1-F4): 1, 2, 4 (all slices), 9, 10 (`CoreState` size and clone targets of 1.6), and the
existing movement area re-hosted on the new `Footprint`/`Dual` APIs with 0 differences.

**Final gate** (D2): all of the above on the whole program, plus the live alpha replay preflight (server
replays every stored game from `init.json` and compares the state checksum) on a copy of the alpha logs.

---

## 6. Risks, rollback, size, time

### 6.1 Risk register

| # | Risk | Where | Mitigation |
|---|---|---|---|
| R1 | **Option order or text drifts** (fingerprint changes, replays fail) | every area with decisions | corpus replay per family; order tests; fingerprint hashes are in the dual run (3.3 item 3) |
| R2 | **Text-order vs index-order** differences for dynamic or numeric keys | `faction_marks`, laws, owner table | `DecimalTextOrd`, X1 corpus, owner-table check at every lend; ordering tests per codec |
| R3 | **JSON number types** (`i32` vs `f64`) in payloads | renderers | renderers use the same `Value::from` types; the dual run compares serialised bytes |
| R4 | **Hidden coupling through `GameState` fields** the footprint misses | every package | debug guard on unloaded slices, byte-exact `GameState` compare after each call, `Dual` over the whole engine suite |
| R5 | **Churn**: 243 commits/14 days in `ti4-engine` (*copied*); fixes land in code being ported | all B, C | area freeze <= 3 days per package, fast merges, nightly `Dual` as drift detector, daily rebase of `core-state-main` |
| R6 | **Lend cost exceeds gain** in early small regions | A2, B1 | choose read-only/large mutators first; region report; merge regions as neighbours port |
| R7 | **Hook explosion**: ~100 faction modules x many hook points | A5, C | enumerate the hook points in A5's interface PR from the 40 modules' call sites (3.2 of `ENGINE_CORE.md`'s count); typed table with defaults; C shards add one entry each |
| R8 | **Window-state port** (25 `Option<Window>` fields in `Game`) | D1 | `Pending` designed in F1; areas already take their window as a typed argument via `lend_args`, so D1 only moves ownership |
| R9 | **Two engines in the binary** (wasm and compile time) | whole program | 6.3, 6.4; deletion commits remove the old code area by area |
| R10 | **RNG draw order**: any extra or missing draw desynchronises every later roll | combat, exploration, decks | `Rng` positions compared after every dual call; no reordering of draws is allowed in a port |
| R11 | **Thunder's Edge factions are not seated by the golden games** | C shards | engine unit tests with `Dual` (like `ENGINE_CORE.md` 6); X2 adds targeted scenarios for them to the corpus |
| R12 | **Core-hygiene lint false positives/negatives** | gate 9 | lint is a regex check plus `cargo tree`; reviewed by the steward |
| R13 | **Estimates for rule areas** are scaled from line counts and a 3.1 k-line prototype, not from a ported rule module | all | F3 reports actual days per package into this file's table; re-plan after wave A |

### 6.2 Rollback / off switch (per area, like `string-movement`)

* Runtime `AreaMask` (default `Core` for merged areas; tests and bins set `String`/`Dual`); native servers can
  override with an environment variable read once at start (name chosen by F3); wasm uses the compile-time
  default.
* Cargo feature `string-<area>` per area (precedent: `ti4-engine/string-movement`): forces the string path.
* Until the deletion commit, the string implementation stays. Rollback of a bad area = flip its bit (no
  rebuild on native) or revert its merge commit; rollback after deletion = revert the deletion commit.
* The nightly runs `Dual` for the areas merged since the last clean night; a difference blocks that area's
  deletion, not the other areas.

### 6.3 wasm size

Before: 5 857 817 raw / 1 829 366 gzip / 1 313 584 brotli (*copied*, `8f2d03e3`), 5 933 830 / 1 859 210 /
1 332 503 after stage 5; the gate (1.80 / 1.30 MB) already fails. While both implementations exist the binary
grows: *estimate* +0.8-1.6% gzip per 10 k lines of string code mirrored in the core (the stage 1-5 prototype
cost +30 KB gzip for ~3 k lines incl. 550 KB of generated tables) -> up to **+20-30% gzip at the peak** (about
2.2-2.4 MB) before the deletions; the end state is estimated at **-5..+10%** against the start, because
`CoreState` code is non-generic and string-free, while the adapter renderers (10-15 k new lines) are added.
`core-dual` and the harness are not compiled into wasm. Each package reports the delta; the budget is an open
decision (section 8, item 3).

### 6.4 Compile time

Core code is integer/enum code without `BTreeMap<String,_>`/`serde` generics; ~1.2 s per 550 KB of generated
source (*copied*). Estimate: the core adds 40-70 s to a cold release build of the workspace while the string
engine (94 s release, one codegen unit, *copied*) is still there, and is ~parallel with it because the core
does not depend on `ti4-engine`. Splitting L6 into `ti4-engine-core-factions` (a crate per layer is the
precondition for the 30-50% critical-path cut estimated in `ENGINE_CORE.md`) lets a faction agent rebuild only
that crate; a three-crate split (L0 data, L1-L5, L6) is recommended from day 1. Dual-run builds (feature
`core-dual`) compile both paths and are slower; packages use the check profile for iteration and the full gate
once.

### 6.5 Process rules

* One owner per slice file and per area (4.6). The steward arbitrates cross-slice needs the same day.
* **Freeze window**: when a package opens, its string modules are frozen for unrelated edits; nightly fixers
  rebase on the package or, if urgent, fix the string side and tell the owner (the dual run is the safety net).
  Maximum open time of a wave B package: 3 working days between "first dual run clean" and merge.
* Each package ends with a short note appended to the package row in this file's table (actual days, lines,
  A/B numbers) so that estimates improve.
* No package changes behaviour. A found bug -> a separate ticket and a string-side fix through the baseline
  process; the port reproduces the bug until then (the dual run requires it).

### 6.6 Time estimate (agent-days; honest ranges)

| Group | Packages | Agent-days |
|---|---|---:|
| Foundation | F1 4-5, F2 5-6, F3 3-4, F4 3-4 | 15-19 |
| Preparatory | X1 3, X2 3, X3 3, X4 4 | 13 |
| Wave A | A1 6, A2 6, A3 5, A4 7, A5 8 | 32 |
| Wave B | B1 9, B2 7, B3 6, B4 7, B5 9, B6 10, B7 5, B8 8, B9 5 | 66 |
| Wave C | 6 shards x 2-3 | 14 |
| Wave D | D1 10, D2 6 | 16 |
| **Sum** | | **156-160** |
| Integration, rebases, diff-chasing, gate reruns (+20%) | | +30 |
| **Total** | range 150-230 (rule areas dominate the uncertainty) | **~190** |

Critical path: F1 (5) -> F2 (6) -> A5 (8) -> B1 (9) -> B2 (7) -> B6 (10) -> C (3) -> D1 (10) -> D2 (6) = **~64
agent-days**. With 6-8 agents in parallel, 8-12 calendar weeks; with 3-4 agents, ~16-20 weeks. The first
visible gains: X4 (days 1-4, est. -8..-12%), A4 (est. -8..-12%), A3 (-3..-5%), then B (est. -15..-20% of the
28.6% of a game spent in automatic steps, option generation and applying answers, copied bases), and the flip
(checkpoints 18.7% -> ~2%): the total of `ENGINE_CORE.md` stage 6 (-35..-55%) is the target, not a promise.

---

## 7. Agent briefs (ready to paste)

**Common preamble (prepend to every brief).**

> You work in your own git worktree on the branch named below, based on `core-state-main`
> (`/root/TI4-Bot-Dev`, repository rules in `CLAUDE.md` apply). Read `docs/ENGINE_CORE_STATE.md` sections 1-5
> first; it fixes the types, the lend/dual-run mechanism, the layers and the gate. This is a **pure refactor**:
> `RULES_VERSION` and all `RULES_*` constants, `GameState` serde output, wire/TS types, fixtures, decision
> fingerprints and bench results must stay byte-identical; do not fix behaviour bugs, report them. The core
> (`ti4-engine-core`, `ti4-engine-core-factions`) has no `String`/`&str`/`serde`/`BTreeMap`/`format!`/`unsafe`
> in non-test code and imports only from lower layers (`check_layers.py`). Edit only the files listed under
> "owned"; changes to others' slice files go through the steward. Run the area under `Dual` early and often;
> a dual-run difference is the definition of "not done". Use a private `CARGO_TARGET_DIR` and the scratchpad,
> never the live checkout; do not push or deploy. Finish with `tools/engine_core/verify_package.sh <AREA>`
> green, a commit (message ends with the attribution lines from the session reminder), and a report: what
> moved, lines, actual days, gate numbers (dual counts, benches, CPU A/B, wasm delta), open issues.

Each brief: **Branch / Scope / Owned / Forbidden / Interface (exposes first) / Gate (beyond the common gate) / Notes.**

### F1 `core-f1-state-types`
* Scope: sections 1.2-1.8: containers, every state slice (all fields of `GameState` incl. `Player`, `SystemState`, per-owner sub-structs as in 1.4), `Seat`/owner-table type, `Rng`, `Ledger`, `Pending` skeleton, `Meta`. Types, `Clone`, `Default`, constructors, accessors with `debug_assert!(loaded)`. No rules.
* Owned: `crates/ti4-engine-core/src/{containers,state}/**`, `lib.rs` exports, `Cargo.toml` (rand_chacha/rand_core, no default features).
* Forbidden: `ti4-engine/**`, `ti4-model/**`, generated files.
* Interface: freeze order Meta/Seats/Players/Board (day 2), the rest (day 4); announce each freeze in the branch log.
* Gate: `state_size` numbers vs targets in 1.6; unit tests for containers (order, text-order for `DecimalTextOrd`, COW sharing, `Log` prefix sharing); layer + hygiene lints.
* Notes: every `GameState` field must have a home or an explicit "not in core" line (1.7); write the field-by-field mapping table into `docs/ENGINE_CORE_STATE_FIELDS.md`.

### F2 `core-f2-sync`
* Scope: 3.2: `Slice`, `Footprint`, `Lend`, `region`, `CoreView`, `SliceCodec` for every slice (load/store through the existing `GameState` structs), `Seats` owner table, round-trip tests.
* Owned: `ti4-engine/src/core_sync/**`, `ti4-engine/src/lib.rs` (one `mod` line), tests.
* Forbidden: `ti4-model/**` (no `GameState` change), core state files (ask the steward).
* Interface: `with_core(FOOTPRINT, |core, k| ..)`, `region`, `CoreView::of`.
* Gate: gate 4 over >= 3 000 golden positions plus fixtures, **all** slices; lend cost table per slice (bytes, ns).
* Notes: start per slice as F1 freezes it; the existing `core_adapter` stays working untouched.

### F3 `core-f3-dual-harness`
* Scope: 3.3: `dual!`, `DualReport`, JSON-path diff reporter, `AreaMask`/`Area` enum, hook into `ChoiceOption`/`DecisionRecord`/event/rng comparison, `verify_package.sh`, `check_layers.py` (tier table of 4.2), `lint_core.sh`, `state_size` bench, bench wiring for `Dual`.
* Owned: `ti4-engine/src/dual.rs`, `crates/ti4-engine-core-bridge/**`, `tools/engine_core/**` (except `decision_corpus/`).
* Forbidden: core state files, rule modules.
* Interface: first day: `dual!`, `AreaMask`, diff reporter on plain `GameState` (needs no core types).
* Gate: re-host the existing movement area (stages 1-5) on `AreaMask` + `dual!` with 0 differences; the harness detects a deliberately injected difference in each comparison category (state, decision hash, event, rng).

### F4 `core-f4-decision-model`
* Scope: section 2: `Choice/Ask/Opt/Ctx/Pick`, per-area empty sub-enums, `Family` trait and registry in the adapter, `Subtype`, generic families (decline, done, number), corpus replay harness (round trip + render equality).
* Owned: `ti4-engine-core/src/choice/{mod,generic}.rs`, `ti4-engine/src/render/{mod,generic}.rs`.
* Forbidden: area files `choice/<area>.rs`, `render/<area>.rs` (created empty with owner comments).
* Interface: as 4.5 F4.
* Gate: corpus (from X2) loads; `parse(option(o).id) == o` for the generic families over the corpus.

### X1 `core-x1-key-codecs`
* Scope: 1.8 table: `MarkKey`/`MarkVal`, `Outcome`, `Law` keys, `LedgerReason`, `DestroyReason`, deck-key enums, `DecimalTextOrd`; pure `parse`/`render`; corpus test from every key/value seen in the 20 golden games + fixtures + engine tests (collect them with a debug dump).
* Owned: `ti4-engine-core/src/keys/**` (enums only), `ti4-engine/src/core_sync/keys/**`.
* Forbidden: state files, rule modules.
* Gate: `render(parse(t)) == t` for every observed text; iteration-order test against `BTreeMap<String,_>`.

### X2 `core-x2-decision-corpus`
* Scope: inventory the 736 option-building sites into families (grammar: id format, kind, payload keys and number types, label template, prompt template, order rule); record the corpus under `tools/engine_core/decision_corpus/` (state hash, rendered `Choice` JSON, V1/V2 hashes) from the 20 golden games, template openings, fixtures, bench configurations; write `docs/DECISION_FAMILIES.md`.
* Owned: `tools/engine_core/decision_corpus/**`, `docs/DECISION_FAMILIES.md`, one recording bin in `ti4-engine-core-bridge`.
* Forbidden: engine code.
* Gate: corpus replays against the current string engine with identical hashes.

### X3 `core-x3-gen-text-enums`
* Scope: `cargo xtask gen-cards`: `NAME_RANK` tables, `Timing`, agenda `elects`, leader `timing`, strategy-card kind, tech `researchable`, per `ENGINE_CORE.md` 1.7 and `CONTENT_GENERATOR.md`; unmapped sentence fails `--check`; adapter-side strings stay in the adapter tables.
* Owned: `xtask/**`, `ti4-engine-core/src/generated/**`, `ti4-engine/src/generated/**`.
* Forbidden: hand-written rule code.
* Gate: `gen-cards --check`; existing tests; string engine unchanged (it may read the new tables only in A-packages).

### X4 `core-x4-checkpoint-cow`
* Scope: ENGINE_OPTIMIZATION #8 on the string state: checkpoints and `DecisionRecord`/`DecisionContext`/`Ability` clones without whole-log copies (persistent or `Arc`-shared logs), no core types.
* Owned: `timing.rs` (checkpoint parts), `choice.rs` (`DecisionLog`), `ti4-model` log field wrappers if needed (serde unchanged).
* Forbidden: everything else.
* Gate: common gate; CPU A/B must show the gain (est. -8..-12%); serialised state bytes identical.

### A1 `core-a1-board-queries`
* Scope: rows A1 of 4.4: pure queries over `CoreView`; string callers use one view per position.
* Owned: `ti4-engine-core/src/query/{board,economy,fleet,seats,laws}.rs`, `state/board.rs`, `state/player/{econ,misc}.rs`, codecs of those slices, `core_adapter` additions for those queries, string call-site delegations in `planets.rs supply.rs tokens.rs payment.rs fleet.rs fleet_strength.rs seating.rs laws.rs coexistence.rs neutral_units.rs space_stations.rs`.
* Forbidden: other string modules; mutators (stay string).
* Interface: 4.5 A1. Gate: offline dual (`core_equivalence` extension) over golden positions, 0 differences.

### A2 `core-a2-movement-tactical`
* Scope: typed movement hooks and `SeatReach` on `CoreView`; `movement`, `transit`, `tactical`, `reach_query`, `opening`; delete the string `MovementRules` search and `string-movement` after a clean nightly.
* Owned: `query/reach.rs`, `kernel/hooks/movement.rs` (signatures from A5), `state/map.rs`, `core_adapter.rs`, string `movement.rs transit.rs tactical.rs reach_query.rs opening.rs`.
* Forbidden: `choice.rs` except the `Observed` movement methods already adapted.
* Gate: dual 1.09 M rule comparisons as before + hooks; reach bucket measured.

### A3 `core-a3-technology`
* Scope: tech queries (`owned`, `researchable`, prerequisites, unit upgrades, aliases), typed tech sets, faction tech hooks (query side).
* Owned: `query/tech.rs`, `state/player/tech.rs`, codec, string `technology.rs starting_tech.rs faction_techs.rs synergy.rs breakthroughs.rs entropic_scars.rs`.
* Gate: dual over every `(position, seat, technology)` of golden positions; content-lookup bucket measured.

### A4 `core-a4-objectives`
* Scope: objective progress/qualification (`values`, imagining), scoring, reveals; secret objectives; depends on typed board (A1) and X3 enums.
* Owned: `query/objectives.rs`, `rules/objectives.rs`, `state/objectives.rs`, codec, string `objectives.rs secrets.rs valuation.rs`, `Observed::revealed_objective_progress_imagining`.
* Gate: dual over every `(position, seat, objective)` of golden positions incl. imagining variants; expected -8..-12% CPU; objective thresholds either in data or in code exactly as today.

### A5 `core-a5-kernel`
* Scope: events, triggers, reaction windows, registry, `Next`, `kernel::fx` primitives, the `HookTable` with every hook point; first an interface PR listing all hook points found at the 40 modules' call sites.
* Owned: `kernel/**`, `state/{pending,ledger}` additions (via steward), string `timing.rs reactions.rs reaction_preview.rs registry.rs infantry_ii.rs magen_defense.rs trigger.rs event.rs`, `render/reaction.rs`.
* Gate: dual of the reaction-window machinery over the 20 golden games (window order, ability order, `used` frequency scopes); corpus of reaction decisions.

### B1 `core-b1-combat`
* Scope: space combat, ground combat rounds, bombardment, anti-fighter barrage, space cannon, retreats, hits assignment, dice; decisions `combat::Ask/Opt`.
* Owned: `rules/combat.rs`, `choice/combat.rs`, `render/combat.rs`, `state/transient/combat.rs`, `state/player/combat_timers.rs`, string `combat.rs dice.rs`.
* Interface: `combat::{open, roll_round, assign_hits, retreat, end}`, `ground_round`. Gate: dual per combat round incl. rng positions and dice history; combat decisions in the corpus.

### B2 `core-b2-invasion`
* Scope: invasion, commit ground forces, bombardment hand-off, landing; merges after B1.
* Owned: `rules/invasion.rs`, `choice/invasion.rs`, `render/invasion.rs`, `state/transient/invasion.rs`, string `invasion.rs`.

### B3 `core-b3-production`
* Scope: production options and placement, cost, payments, discounts, supply mutations.
* Owned: `rules/production.rs`, `choice/production.rs`, `render/production.rs`, `state/transient/production.rs`, `state/player/production_timers.rs`, string `production.rs payment.rs supply.rs`.
* Notes: `production::build_options` is 237 lines and 1 224 `format!`s are spread across the engine; renderers only here.

### B4 `core-b4-trade-diplomacy`
* Scope: transactions (neighbours already on the core), trade window, diplomacy (3.7 k lines, opt-in feature, its own `DiplomacyState`).
* Owned: `rules/{trade,diplomacy}.rs`, `choice/{trade,diplomacy}.rs`, `render/{trade,diplomacy}.rs`, `state/trade.rs`, string `transactions.rs diplomacy/**`.

### B5 `core-b5-exploration-relics`
* Scope: exploration, relics, legendary planets, fracture, Thunder's Edge, promissory notes; typed decks (`state/decks`, `cards`, `explore`).
* Owned: `rules/{explore,relic,legendary,fracture,thunders_edge,promissory}.rs`, matching `choice/` and `render/` files, `state/{decks,cards,explore}.rs`, string `exploration.rs relics.rs legendary.rs fracture.rs thunders_edge.rs promissory.rs`.
* Gate: dual including deck order after shuffles and draws (rng positions).

### B6 `core-b6-action-cards`
* Scope: 142 action cards as `match kind` over `ActionCardKind` (X3 timing), play windows, reactions.
* Owned: `rules/action_cards.rs` (may split per card group), `choice/cards.rs`, `render/cards.rs`, `state/player/card_timers.rs`, string `action_cards.rs`.
* Notes: starts on the interfaces of B1/B3/B5; merges after them; effects open windows via `Next`, never call other areas.

### B7 `core-b7-leaders-abilities`
* Scope: leaders dispatch (`dispatch_leader` 817 lines), faction ability plumbing.
* Owned: `rules/leaders.rs`, `kernel/hooks/abilities.rs` (with A5's steward OK), `choice/leaders.rs`, `render/leaders.rs`, `state/player/leaders.rs`, string `leaders.rs faction_abilities.rs`.

### B8 `core-b8-strategy`
* Scope: strategy cards (primary, secondary, follow), strategy phase, draft, status phase, secondary previews.
* Owned: `rules/{strategy,status,draft}.rs`, `choice/strategy.rs`, `render/strategy.rs`, `state/strategy.rs`, string `strategy_cards.rs strategy.rs secondary_preview.rs draft.rs status.rs`.

### B9 `core-b9-agenda-laws`
* Scope: agenda phase, voting, `agenda_effects::resolve_with` (906 lines), laws mutation.
* Owned: `rules/{agenda,vote,laws}.rs`, `choice/agenda.rs`, `render/agenda.rs`, `state/agenda.rs`, string `agenda_effects.rs vote.rs agenda.rs laws.rs`.

### C1-C6 `core-c<k>-factions-<shard>`
* Scope: port the faction modules of your shard (list by file, balanced to ~7 k lines): hook entries + faction state in `Marks`; the faction's decisions become families in the areas that own the hook.
* Owned: `ti4-engine-core-factions/src/<faction>.rs` of the shard, `state/marks` record of those factions, string `factions/<files>`, `render/factions/<files>`.
* Forbidden: other shards' files; area files (ask the area owner for a hook change).
* Gate: dual with a seat of each faction (all 24 lobby factions are in the template openings; Thunder's Edge ones via unit tests).

### D1 `core-d1-driver`
* Scope: `driver::{step, answer, ask}`, window dispatch, phases, setup, `Pending`; the private `Game` window fields move into `Pending`.
* Owned: `ti4-engine-core/src/driver/**`, `ti4-engine/src/game.rs phase.rs wiring.rs setup.rs fixtures.rs`.
* Gate: whole-game dual (every step of the 20 golden games), benches with `Dual`.

### D2 `core-d2-flip`
* Scope: authority flip, `materialize`, reverse lend for the leftovers, MOVE (not delete) the string modules per area into `ti4-engine-legacy` (commit per area), checkpoints = `CoreState::clone`, remove legacy and `core-dual` wiring from production builds (server, wasm). See "Final structure after D2" below.
* Owned: `Game`, `core_sync`, the move commits across packages' string modules.
* Gate: final gate (section 5); measured checkpoint bucket; the dual harness still green with its allow-list.

---

## 8. Open decisions for the user (max 5, each with my recommendation)

1. **Crate split.** One `ti4-engine-core` with a layer lint, or three crates (L0 data, L1-L5, L6 factions)?
   *Recommend:* two now (`ti4-engine-core` L0-L5, `ti4-engine-core-factions` L6), lint enforcing layers
   inside the first; split further when the layers have settled (cheap then, painful before).
2. **Do X4 (string-side checkpoint COW) now?** It is independent of the core and worth an estimated -8..-12%
   at a cost of ~4 agent-days, but it edits `timing.rs`/`choice.rs`, which the kernel package will later port.
   *Recommend:* yes, first week; the kernel package ports the improved code.
3. **wasm size budget during the migration.** The gate already fails (1.86 vs 1.80 MB gzip); both
   implementations coexisting may reach ~2.2-2.4 MB gzip at the peak.
   *Recommend:* raise the gate to 2.4 MB gzip temporarily with a ratchet that is lowered at each deletion
   commit; no area is deleted before one clean nightly.
4. **Freeze policy for string modules under port** (<= 3 working days per wave B package, nightly fixers
   rebase onto the package, dual run as safety net) versus "fixers always win, the port is redone".
   *Recommend:* the freeze, with an escape: an urgent alpha fix goes to the string side and the package owner
   re-applies it; the dual run makes this visible within a night.
5. **Scope of this program.** Stop at D2 (engine on the core, bots and host unchanged) or include D3
   (typed options and a typed view for the rule bot, est. -2..-4% and removal of ~120 parse sites)?
   *DECIDED (user): D3 is in scope, after D2.* D3 still starts only after the flip is a nightly old and has its
   own baseline/A-B process. D2 itself is "flip and freeze" (legacy moved, not deleted); see the next section.

---

## 9. Final structure after D2 (decided by the user)

1. **The old string engine is kept, frozen, as `ti4-engine-legacy`.** It is built only for tests and tools
   (dev profile or a `legacy` feature), never for the server or wasm builds. Deleting it is an optional later
   step. Relation to the existing workspace crate `crates/ti4-legacy` (package `ti4-legacy`): that crate is
   unrelated, it is the Python artifact / replay converter stub (`checkpoint`, `converter`, `corpus`,
   `replay`, `state_import`; it depends on `ti4-engine`). The names are different (`ti4-legacy` vs
   `ti4-engine-legacy`), so there is no clash; it is neither renamed nor deleted, but the two are easy to
   confuse and docs should always write the full name.
2. **The new `ti4-engine` becomes a thin string wrapper around the typed core.** It holds `CoreState`,
   parses and renders the string `GameState` and the option strings (materialize, cached),
   `render/<area>.rs` maps Ask/Opt/Subtype <-> byte-exact text, and it owns the serde / wire / save formats.
   Host, server, wasm and the bots keep the string interface until D3. D3 gives the bots typed options and a
   typed view; afterwards the wrapper serves only the wire and save formats.
3. **The dual harness stays as a permanent regression oracle** (`ti4-engine-core-bridge`: legacy vs core on
   the golden games, benches and alpha replays). Rule fixes go into the core only (e.g. the Heart of Ixth
   string bug, `ENGINE_KNOWN_ISSUES.md` B1), so legacy and core diverge on purpose. The oracle is valid per
   `RULES_VERSION` and needs an **allow-list of intended differences** (keyed by rules version, area and
   game/step). Costs and risks: a duplicated build in the dev profile, frozen legacy tests, allow-list upkeep
   (a stale entry hides a real regression; every entry needs a reason and a version bound).
4. **D2 scope**: authority flip, materialize, reverse lend for leftovers, MOVE the string modules into
   `ti4-engine-legacy` per area, checkpoints = `CoreState::clone`, remove legacy and core-dual wiring from
   production builds. Wasm size: only the new engine ships.
5. **Layer diagram** (`ti4-engine-legacy` and `ti4-engine-core-bridge` stand beside the production line):

```text
bots / host / server / wasm            (string interface until D3)
        |
        v
ti4-engine                 thin wrapper: CoreState + string parse/render, render/<area>.rs, serde/wire/save
        |
        v
ti4-engine-core            L0 data | L1 state + choice | L2 query
                           L3 kernel (HookTable, AbilityTable, seams, Stage/Driver, fx)
                           L4 rules/<area> | L5 driver
        ^
        |  (fills the hook table)
ti4-engine-core-factions   L6

beside them (tests / tools only, not in server or wasm):
ti4-engine-legacy          frozen string engine (moved modules)
ti4-engine-core-bridge     dual harness: legacy vs core, with the allow-list
```
