# The reaction / ability port (package `core-reactions`)

Branch `core-reactions`, cut from `core-integration` 04578c39, with `core-b1-combat` (a01f1eb9), `core-b2-invasion`
(d3c9316e) and `core-b3-production` (aac6a4b3, already inside B2) merged in. Nothing pushed.

Stage 2 (this document's sections 3.2, 4, 5, 6 and 8) ported the round agents, the Argent agent in the production
window, Tellurian, Brother Milor, the two S8 slots, Self-Assembly Routines, Letani Warrior II and Salvage Operations,
and lifted two bails of stage 1 (Transparasteel Plating, the silent "no stream" bail). Stage 2b (section 4) added the play of
reaction cards from the standing slot for the stamps of B6. Stage 3 (section 9) registers card effects that read another area
(Sabotage at once, Intercept, Parley, Direct Hit, Skilled Retreat), lifts the guard of the production window for the tier-W
factions and windows the Hacan hero and Keleres agent offers, and ports Antivirus, the Cavalry, the Crimson Legionnaire and
Ipswitch. Stage 1 text follows.

Problem: the core `AbilityTable` is empty, so every core window that emits an event whose WHEN/AFTER window holds a
registered ability (a reaction slot, a technology, a faction ability, a leader) bails (`Recording.nested`, 24
call sites in `dual_window.rs`). This document is the design of the port; section 6 is the census before and after.

## 1. What exists

* `kernel/resolver.rs` (A5): `Resolver` = registry (`AbilityDef`: id, owner, event, relation, frequency, optional,
  repeatable, has_condition, `arg`), player order, eligibility, frequency scopes, depth-first nested emission,
  checkpoints. Data and ordering only; the rules are the `Host`'s (`participates`, `bars_faction_abilities`,
  `condition`, `forbids`, `effect`, `decide`, `window_skipped`, `window_closed`).
* `kernel/stage.rs` + `driver/` (D1): `Window` contract, `Sx::emit` (opens the resolver with `RulesHost`), the
  `Tape` (a nested question found dry: the driver rolls the step back, reports the question, re-runs the step with
  the answer appended), `AbilityTable` (empty), `Sx::die`.
* `kernel_sync/shim.rs` (A5 dual): replays one string emission through the core `Resolver`, answering every rule
  question from the string trace. It proves the *algorithm*; it holds no rule.
* String side: `reactions::arm` registers, per seat, the standing reaction-card slot of each printed window
  (`window_table`), Instinct Training, the L1Z1X agent (I48S), Infantry II, Magen Defense and the per-faction
  `timing_abilities`; `timing::Resolver::eligible_with_context` / `pick_with_context` / `run_window_with_context`.
  S8 (`string-disentangle-s1-s8`, `reactions/timing_effects.rs`) makes the three own slots data (`AbilityDef`,
  `EffectId`) with the closures as the arms of one `match`; it is the shape the core table copies.
* `dual_window.rs` `Recording`: what the string window did at its own level; anything inside an emission sets
  `nested`, and the adapters bail.

## 2. Decisions

### 2.1 A window is `(EventKind, Relation)`; an ability is a `Spec` in a table keyed by `Slot`

The timing window needs no new type: it is the resolver's bucket. What was missing is the table behind the
`AbilityDef`s. The registry text id (`technology:hacan:inf2:GROUND_FORCE_DESTROYED:after`) names a faction and
an event, so it cannot key a static table. Each rule gets a `Slot` (a `#[repr(u16)]` enum in the crate that owns the
rule; one variant per registered ability *rule*, owner-independent) and `AbilityDef.arg` carries it (that is what
`arg` is for). `AbilityTable` holds `Spec`s:

```rust
pub struct Spec {
    pub slot: Slot,
    pub applies: Applies,            // EverySeat | Faction(Faction) | Held(fn(&CoreState, Seat) -> bool)
    pub event: EventKind, pub relation: Relation,
    pub frequency: Frequency, pub optional: bool, pub repeatable: bool,
    pub id: fn(&Render-free key) -> AbilityKey,   // kind + card key + clause, the text the adapter renders
    pub condition: Option<fn(&Eff<'_>) -> bool>,
    pub effect: fn(&mut Eff<'_, '_>) -> Result<(), TimingError>,
}
```

`Spec` is both the registry row (`Spec::def(owner) -> AbilityDef`) and the behaviour, so there is one place per
ability, as in S8. The registry of a game is `AbilityTable::registry(state, order)`: per seat, in the string
order of `reactions::arm` (slots, Instinct Training, I48S, Infantry II, Magen, factions), so the per-owner order of
every bucket equals the string's. Natively built, it is what D2 will use; under the dual it is compared with the
string registry (below).

### 2.2 Ported subset, and when a nested window bails

The string registry has about 270 abilities; porting all of them is the sum of the faction ports. The dual does not
need that to un-bail a window: a core window may *follow* a string run when every ability the string run found
**eligible** (its condition held, whether it was asked, taken as the single mandatory one, or run) is in the
table. An ability whose condition failed cannot change anything and may be missing from the core registry.
`Recording.eligible` / `effects` (new, fed by `timing::eligible*` and the effect call) list them;
`Recording::unported(&table)` names the first one the table lacks, and the adapter bails with *that id* as the reason
(`nested: unported ability technology:*:inf2:...`), so the census is by ability and the next port is obvious.
Window bail reasons that are not abilities (a nested emission outside the resolver, a nested question of a
callee) stay a plain `nested` bail.

The adapter checks the core registry against the string one restricted to ported slots (ids, owners, order,
frequency, optional, repeatable, has_condition): a wrong `Applies` or a wrong registration order is a dual
difference, not a silent divergence.

### 2.3 Effects and conditions run in an `Eff`

`Host::effect` gets `(ability, event, resolver)`; an effect needs the state, the kernel, the question channel, the
dice, a nested `emit` and the trace id counter. `Eff<'h, 'a>` bundles `&mut RulesHost` (state, kernel, table, tape,
seeds, next id), `&mut Resolver`, the `AbilityDef` and the `&mut EventRec`:
`eff.state()`, `eff.kernel()`, `eff.ask(&Choice) -> Pick` (the tape), `eff.die(domain, sides)`,
`eff.emit(rec)` (a nested emission through the same resolver, same host), `eff.cancel()`, `eff.trigger_facts()`.
Conditions are `fn(&Eff) -> bool` over the same (read only). The rollback of the nested-question replay already
covers all of it: state, resolver and trace ids are restored, so an effect that asks twice runs once per committed
run.

### 2.4 How an emission opens a nested reaction stage in the driver

No new stage type: a nested question is the `Tape` mechanism that already exists (a stage is the *window's own*
question; this is a question *inside* `Sx::emit`). What was missing is the facts. A reaction window question is
rendered with a trigger (`DecisionTrigger::from_event`: event id, payload facts, the emission chain), the
`active` detail and the battle/invasion position; none of them is derivable from the typed `Ask::Window`. The
`Tape` therefore stores, next to the dry question, `NestedFacts { event: EventRec, relation, chain }`
(`RulesHost::decide` has both; `Eff::ask` takes them from the effect), and the driver exposes
`Driver::nested_facts()`. The adapter's `complete` callback builds the trigger with `kernel::trigger::Trigger`
(already in the core, already converted by `kernel_sync::convert::trigger_text`), the `active` detail from the
state, and the battle position from the string position at the window's start.

The dual then needs nothing new in `drive_with`: the Nth rendered question, nested or not, is answered with the
Nth recorded answer. `Recording.questions` therefore records nested questions too (flag `nested: true`).

### 2.5 New questions

Each ability question gets `Ask`/`Opt`/`Subtype` variants in the area that owns it and a byte-exact renderer
(`render/<area>.rs`), as every window question since D1. Reaction windows themselves keep `reaction::Ask::Window`.

### 2.6 Order, frequency, optionality

All of it stays in the resolver (unchanged): the registry order is the offer order (part of the decision hash), the
frequency scope is the `used` set of the resolver (compared by the A5 shim), `optional` adds the decline option.
The port only has to give each `Spec` the same three flags and registration position as the string slot.

### 2.7 What stays string

The reaction-card slot (`reaction:`) is registered (eligibility, the offer, a decline); the play needs the whole
action-card effect set and the announcement choreography (section 4). A window where a card was played bails with the
slot's id. The same holds for every ability not yet ported.

## 3. How a nested window is followed (what is built)

Core (`ti4-engine-core`):

* `kernel/ability.rs` (new, L3): `Slot`, `Applies`, `Spec` (registry row and behaviour in one value), `AbilityTable`
  (`registry`, `knows`, `bars`, `codes`), `RulesHost` (the resolver's `Host`: conditions, effects, picks through the
  tape), `Eff` (state, kernel, `ask`, `roll`, `die`, `emit`, `cancel`), `CondCx` (a condition's inputs, `unknown`),
  `NestedFacts`, `Tape` (now carries the facts of the first dry question and the `unknown` flag).
* `driver`: `Driver::nested_facts()` (what the nested question was asked about), `Driver::unknown()` (a condition
  the core could not answer). A dry tape now answers with the *decline* when the question has one: a repeatable,
  unlimited ability (the reaction-card slot) picked again and again would otherwise never end the window.
* `rules/reactions/` (new): the generic abilities registered for every seat: `infantry_ii` (the roll; the
  return at TURN_BEGAN places units and stays string).
* `rules/action_cards/reaction_slot.rs` (new, in B6's area): the standing reaction-card slot, one `Spec` per printed
  window (`SLOT_BASE + index` over `windows()`), `playable` (hand, salvage yard, reaction modes, the printed window
  and its guard, 22.3 `reaction_can_matter`, the bars). `eligibility_only`: the offer and a decline are followed, a
  play is not.
* `ti4-engine-core-factions/src/cb_economy/cabal/timing.rs` (new): Reanimator, Devour (ships, ground forces), The
  Terror Between (ships, ground forces) and the four participation marks, `Applies::Faction(Cabal)`.
* `state::RollPurpose::Ability(slot)`: the ledger row of a die an ability rolled.

String side (`ti4-engine`):

* `reactions_core.rs` (new): the table of the adapter (`table()`), the mapping between registered ids and slots
  (`slot_of`, `id_text`), `Recording::unported`, `follow` (the core resolver configured as the string one: phase,
  order, speaker, counters, used keys) and `Follow::arm` (the registry built natively from the table).
* `dual_window.rs`: `Recording` records the whole nested activity (questions with `nested`, effects, eligible
  abilities, dice rolled by abilities, whether it is *foreign* to every ability, the resolver configuration and the
  first trace id). `drive_with` arms the core resolver, answers nested questions from the recording in order,
  completes their display facts (`complete_nested`: trigger, `active` detail, battle/invasion position, the card
  slot's label and payload) and compares the dice abilities rolled (`nested.ability_dice`). `nested_check` bails
  under the window's own label and with the reason. All 24 `script.nested` bails are gone; the footprint of every
  window grows by what the abilities touch (`nested_fp`).
* `timing.rs`: `EffectGuard`, `PickGuard`, `note_eligible` feed the recording; `core_cfg`.
* `dual.rs`: `DualReport.nested` (the census, printed with `TI4_NESTED_LOG=1`).

### 3.1 When a nested window still bails

A window bails (counted under its own label, reason `nested (...)`) when the string run:

1. found an ability eligible that the table lacks: `unported ability <kind>:*:<card>:<EVENT>:<rel>` (the id names the next
   port);
2. played a reaction card the core does not play: `unported ability reaction:*:<EVENT>:<rel> (card <alias>)` for a card
   outside `reaction_slot::effect_ported` (a card that asks or places, Reflective Shielding), `(a card played at once)` for
   a play in the window of `ACTION_CARD_PLAYED` (Sabotage, Instinct Training answer it at once) or below rules version 3,
   `(Data Skimmer)` with a Ral Nel seat (the discard goes to the breakthrough), and `Unknown` for a card from the salvage
   yard;
3. did something nested that no ability explains (`nested outside an ability`: a question or an event inside an
   emission outside every effect and every pick);
4. has no resolver configuration (`nested activity without a resolver`);
5. reached a condition the core cannot answer (`Unknown`, wording in `drive_with`): Bribery's goods (Keleres agent), a
   printed card cost the core does not judge, Harness Energy / Infiltrate, a Nomad that listens to the exhaustion of an
   agent (Temporal Command Suite: Sol, Letnev, Argent, Tellurian, Brother Milor, I48S), a technology carried by a Nekro
   Valefar Assimilator (Instinct Training), a fact the adapter did not compute (the adjacency of Argent's agent, a
   faction mech the tables hold twice, a Naaz target of I48S), an action card played at once or one the core does not
   play (the safety nets behind bail 2);
6. rolled dice in an ability in a window that was not lent the dice stream (a driver without seeds): the bail
   `nested (dice rolled by an ability in a window lent no stream)`. In stage 1 this was documented as a silent bail; it
   was in fact a `nested.ability_dice` difference (the roll came from a stream at position zero). Now counted and tested;
7. used a leader while diplomacy promises are on (`LeaderUsedFor` is an event of the promises, the core does not
   evaluate them): `unported ability leader:*:solagent:... (diplomacy promises on a leader's use)`, round agents only.

Bails 1-4 have a unit test in `reactions_core::tests` (the card ones: `bail_a_played_card_the_core_does_not_play_is_named`,
`bail_a_card_played_at_once_is_named`, `bail_a_play_with_data_skimmer_or_below_rules_version_3_is_named`, and the lockstep
`reflective_shielding_stays_in_the_string_in_lockstep`), 5 in `rules::action_cards::reaction_slot::tests`,
`rules::reactions::{instinct_training,l1z1x_agent}::tests` and `invasion::tests::bail_the_sol_agent_with_a_listening_nomad_is_counted`,
6 in `invasion::core_ground_round_prechecks::bail_dice_rolled_by_an_ability_in_a_window_lent_no_stream_is_silent`, 7 in
`invasion::tests::bail_the_sol_agent_with_diplomacy_on_is_counted`. An ability whose condition failed never bails a window.

### 3.2 Stage 2: what changed in the machinery

* `Eff::read` (state and kernel together), `Eff::codes`, `Eff::adapter_facts`; `AdapterFacts` (`reach`: the systems
  adjacent to a system as a seat sees them; the core is lent no galaxy, the adapter computes them for the producing
  system of the production window and passes them through `reactions_core::with_reach`).
* `nested.events`: the events the abilities emitted inside the window (the string `nested_kinds` without the questions)
  are compared with the core resolver's journal minus the window's own emissions. A dropped `eff.emit` now shows.
* `drive_positioned`: `drive_with` for a family that reads the position it is asked in (the production questions).
  `production_window_dual::step` uses it, so the production dual follows nested activity; its footprint writes Marks.
* `complete_nested` adds the trigger only to the questions that carry one in the string (`with_trigger_of`: the window's
  own pick, Instinct Training, I48S), the `active` detail only to the window's own pick; the Sol/Letnev agent question
  has neither (`render::leaders::complete_round_agent` builds its `offered` card and captions).
* Typed mark `YssarilSsruuRoundAgent`: the JSON blob of a round agent's bonus (`RoundBonus`) is twelve typed fields in the
  core (`Sh::RoundBonus` in `core_sync/keys/mark_values.rs`, an `Alt` with the blob as fallback; the round trip is
  checked on parse).
* Windows: `BeginGroundRound` gets the sources (the kernel scope: which units are ground forces); `BeginRound` announces
  `COMBAT_ROUND_STARTED` with `round_seq` when a Letnev round agent listens (`round_seq_listens`; `round_opening_unported`
  no longer excludes a Letnev agent, only a Letnev or Naaz side); the combat `Flow` and the barrage spend the cancellations
  an ability granted in the `HITS_TO_ASSIGN` / `ANTI_FIGHTER_BARRAGE_HITS` window (`hits::open_window`) before a sustain
  or a casualty; the production window clears Argent's grant when a placement spends it
  (`ProductionWindowClosed` observers); `announce_end` follows the notes (marks, staged events) the abilities leave when the
  string run had nested activity.
* Card plays (`reaction_slot::play` / `announce_declared`): the slot chooses the card (a question `reaction::Ask::PlayCard`
  when the copies differ: one option per printed card, `AbilityTable::card_name` is the adapter's key), spends it into
  `declared_card_plays`; `AbilityTable::closed` (the kernel's `Host::window_closed`) announces the declared cards at the
  close of the window: `ACTION_CARD_PLAYED` (with the windows of every seat), the stamp (`play_simple`), the discard pile,
  `last_action_discarded`, `ACTION_CARD_DISCARDED`. The string wraps its own announcement in `ClosedGuard` so that what it
  emits there is not foreign. The display facts of a nested question (card names of the window pick) are read from the state
  the question was put in (`Driver::nested_state`): the driver rolls the state back before it reports the question.
* Kernels: the combat kernel carries `economy.action_cards_forbidden` (Transparasteel Plating) and
  `direct.nomad_watches_agent_exhaustion`; the reaction slot's `playable` no longer bails for a Yssaril seat.
* The production dual no longer excludes an Argent seat (`unported_window`): Argent's agent is an ability of the table.

## 4. Ported abilities

| slot | string id (`*` = the owner's faction) | notes |
|---|---|---|
| 1 | `technology:*:inf2:GROUND_FORCE_DESTROYED:after` | rolls a die from the dice stream, `Inf2Card` mark; the return at TURN_BEGAN is string |
| 9001 / 9002 | `leader:sol:solagent:GROUND_COMBAT_ROUND_STARTED:after` / `leader:letnev:letnevagent:COMBAT_ROUND_STARTED:after` | the round agents (own, rules 16 / 17): the unit question, the `YssarilSsruuRoundAgent` mark, the exhaustion; the Ssruu copies stay string |
| 4001 | `leader:argent:argentagent:GROUND_FORCES_BEING_PRODUCED:when` | the production window's grant mark; adjacency is an adapter fact |
| 5 | `technology:*:it:ACTION_CARD_PLAYED:when` | Instinct Training: question, token, exhaustion, staged `STRATEGY_TOKEN_SPENT`, cancel (core tests only: no followed window emits a card play yet) |
| 6 | `leader:*:l1z1xagent:SYSTEM_ACTIVATED:after` | I48S: question, the mech swap, exhaustion (core tests only: no window emits the activation) |
| 100+i | `reaction:*:<EVENT>:<rel>` (one per printed window) | the offer, the choice of card and the declared play of the stamps of B6 (Morale Boost, Shields Holding, Emergency Repairs, Rout, Waylay, Maneuvering Jets, ...); the announcement at the window close |
| 2001 / 2002 | `promissory:*:antivirus:{SPACE,GROUND}_COMBAT_STARTED:after` | Antivirus placed faceup (stage 3) |
| 5001 / 5002 | `unit:*:mahact_infantry{,2}:GROUND_FORCE_DESTROYED:after` | Crimson Legionnaire (stage 3, section 9.5) |
| 8002-8004 | `leader:*:mentakhero:{SPACE_COMBAT_STARTED,SHIP_DESTROYED,SPACE_COMBAT_ENDED}:after` | Ipswitch, Loose Cannon: the purge, a ship placed per destroyed ship of another player, the end of the record (stage 3; a destroyed cruiser or flagship bails: the placed form is the string's) |
| 10001 / 10002 | `promissory:*:cavalry:SPACE_COMBAT_{STARTED,ENDED}:after` | The Cavalry (stage 3) |
| 3001 | `unit:*:cabal_mech:GROUND_FORCE_DESTROYED:after` | Reanimator |
| 3002 / 3003 | `ability:*:devour:SHIP_DESTROYED:after` / `...GROUND_FORCE_DESTROYED:after` | Devour |
| 3004 / 3005 | `unit:*:cabal_flagship:SHIP_DESTROYED:after` / `...GROUND_FORCE_DESTROYED:after` | The Terror Between (a Cabal seat; the Nekro-carried text stays string) |
| 3006-3009 | `ability:*:devour_mark` / `devour_clear` on SPACE/GROUND_COMBAT_STARTED/ENDED | participation marks |
| 6001 | `unit:arborec:arborec_infantry2:GROUND_FORCE_DESTROYED:after` | Letani Warrior II: the roll and the `ArborecLw2` count |
| 7001 / 7002 | `leader:*:yinagent:SHIP_DESTROYED:after` / `...GROUND_FORCE_DESTROYED:after` | Brother Milor: 2 fighters / 2 infantry of the form the owner places (`placed_form`, checked against `placed_unit_id` for every faction) |
| 8001 | `technology:*:so:SPACE_COMBAT_ENDED:after` | Salvage Operations' trade good (staged); the produce half is string |
| 10 | `tech:*:sar:GROUND_FORCE_DESTROYED:after` | Self-Assembly Routines' trade good (announced through the resolver) |
| 11-14 | `leader:*:titansagent:{HITS_TO_ASSIGN,SPACE_CANNON_HITS,GROUND_HITS_TO_ASSIGN,ANTI_FIGHTER_BARRAGE_HITS}:when` | Tellurian: exhaust, one cancellation in the round pool |

Registration order in the table (per owner, the string order of `reactions::arm`): round agents (`cc_ground::borrowed::timing`: a core area may not import the combat and invasion areas, the faction crate may), reaction slots, Instinct
Training, I48S, Infantry II, the faction packages (Argent, Cabal, Arborec, Yin, Mentak), then the technologies the string
chains behind the faction hooks (Tellurian, Self-Assembly Routines).

Not ported, with the reason (each is an `unported ability` bail with a test of the naming): the Ssruu copies of every
agent; the reaction-card play of every card that is neither one of the stamps of `reaction_slot::effect_ported` nor a `CardPlay` of
section 9 (Fire Team, Scramble, Reveal Prototype, Courageous to the End), Reflective Shielding (design in 9.2), a play at once
below rules version 3, Data Skimmer, a card from the salvage yard; the Empyrean flagship (Dynamo: adjacency and an influence payment); Nekro's Technological Singularity;
Salvage Operations' produce half (Mentak's hero and Mahact's Crimson Legionnaire are ported in stage 3); Sardakk's mech
(a hit assigned through the timing, with its own windows); the Empyrean mech's Watcher (it reads the map); (the offers of B3's production window, Hacan's hero and the Keleres
agent, and the promissory notes Cavalry and Antivirus are stage 3).

## 5. Census: the nested bails before and after

`TI4_NESTED_LOG=1 dual_run tools/engine_profile/profile-20.json 20 driver=dual,invasion=dual,production=dual,leaders=dual,exploration=dual,agenda=dual`
(the 20 golden games, gate profile). All runs: 0 differences, hashes equal. The run prints `nested activity: N signatures,
M runs`: 736 runs in which a string window had nested activity (a reaction window, an ability); 203 signatures before,
231 after the card plays (the signatures now name the cards played).

Stage 1 (238b6b55) -> stage 2 (abilities, section 4) -> stage 2b (card plays), nested bails (window calls that bailed
with a `nested (...)` reason): **394 -> 264 -> 213**.

| window | compared before -> after 2 -> after 2b | bailed before -> after 2 -> after 2b |
|---|---|---|
| invasion::begin_ground_round | 248 -> 308 -> 308 | 69 -> 9 -> 9 (Sol agent 64 -> 0; the rest are Ssruu copies and Naaz/Dunlain gates) |
| invasion::ground_round_hits | 301 -> 341 -> 341 | 61 -> 21 -> 21 |
| combat::flow_step | 1838 -> 1847 -> 1865 | 41 -> 20 -> 2 |
| combat::begin_round | 182 -> 197 -> 206 | 187 -> 169 -> 160 |
| combat::settle | 360 -> 381 -> 401 | 91 -> 76 -> 56 |
| combat::barrage_step | 185 -> 187 -> 190 | 9 -> 4 -> 1 |
| combat::answer | 814 -> 823 -> 823 | 80 -> 71 -> 71 |
| combat::announce_end | 195 -> 222 -> 222 | 67 -> 37 -> 37 |
| ProductionWindow::step | 1040 -> 1374 -> 1374 | 2371 -> 2037 -> 2037 (an Argent seat no longer keeps the window string) |
| production::pay_step | 703 -> 999 -> 999 | 1478 -> 1182 -> 1182 |

Reasons by runs, after stage 2b (213 nested bails):

| runs | reason |
|--:|---|
| 61 | a reaction card the core does not play: Parley 9, Direct Hit 13 (a staged destruction), Skilled Retreat 15, Intercept 5, Fire Team 5, Reveal Prototype 3, Sabotage 6 (played at once), Courageous 2, Reflective Shielding 2, Scramble 1 |
| 15 + 11 + 2 | Nekro Technological Singularity (open, close, ground) |
| 14 + 1 | Empyrean flagship (Dynamo) |
| 10 + 9 + 5 + 4 + 7 | Ssruu copies (Yin ships and ground, Solagent, Letnevagent) |
| 9 + 6 + 1 | Mentak hero |
| 8 | Salvage Operations' produce half (an offer; the trade good is ported) |
| 7 | the Empyrean mech's answer to a card play (`unit:*:empyrean_mech:ACTION_CARD_PLAYED:when`, a play at once) |
| 6 + 4 + 3 | promissory notes: Cavalry, Antivirus |
| 5 + 4 | Mahact infantry II and Crimson Legionnaire |
| 4 + 2 | Sardakk hero and mech |
| 6 + 4 + 1 | `Unknown`: a card cost the core does not judge; a technology carried by a Nekro Valefar Assimilator; the "no stream" bail of the bombardment window (Infantry II) |

(The reasons of stage 2 that were new in its table are windows that used to bail before they reached the nested check, on
the faction notes of the end announcement or the Argent seat; they are counted and named now.)

## 6. Gate (on this branch)

Stage 1 (238b6b55): layers, lint_core, clippy, the immutable surfaces, 41 test suites, the dual suite, golden dual
`driver=dual` and the wide mask 0 differences, benches 200/200; its mutation checks and bail tests are in the history of
this file (Infantry II threshold, Devour's captor, `reactions_core::tests` and the lockstep tests of `invasion.rs`).

Stage 2 and 2b (final commit `585d92d1` of the code, `tools/engine_core/verify_package.sh driver OUT none 6`, `CARGO_BUILD_JOBS=3`, `--profile gate`,
`SKIP_AB=1 SKIP_WASM=1`, a private target dir): `PACKAGE OK (driver)`.

* layers 0 violations (333 files), lint_core 0 hits (335 files), clippy 0 in core/bridge/dual/core_sync/render,
  immutable surfaces unchanged (web/, RULES_VERSION.md, model state.rs, wasm fixtures, against the cut point 04578c39),
  41 test suites ok, `gen-cards --check`, xtask and wasm tests ok, dual suite (6 suites, `driver=dual`, fail fast) ok;
* golden dual `driver=dual` (20 games): 4509 calls, 25992 comparisons, **0 differences**, hashes equal; the wide mask
  (`driver,invasion,production,leaders,exploration,agenda=dual`, run on the final binary): 11.65 M comparisons, **0 differences**, hashes equal, the nested census as in section 5;
* benches `bench` + `bench-v1..v5` with `driver=core`: **200 identical / 0 different** each; `bench` with the driver on
  String 200/0;
* `board_equivalence templates` and `golden profile-20 20 10`: 0 mismatches (board, strategy, agenda, action card reads).
* Not run (as agreed): the CPU A/B, the wasm size gate, release builds, `state_size`.

Failures found on the way (each fixed with a test that fails on the code before the fix):
1. `check_layers.py core` (first gate run): the round agents sat in `rules/reactions` and imported the L4 areas combat and
   invasion; moved to the faction crate. Test `rules::reactions::layering::the_reactions_area_imports_no_other_rule_area`.
2. `vote::typed_render_tests::a_whole_vote_runs_in_lockstep_on_the_core_window` ("compared 0", only in the full suite): a
   test that takes the process-wide dual report without `serial()` counts into the throwaway report while a holder runs.
   Seven tests got the lock; test `dual::tests::every_test_that_takes_the_report_holds_the_serial_lock` scans for the rest.
3. `decision_delivery_inventory::every_producer_and_delivery_site_matches_the_reviewed_registry`: the renderer built a
   `Choice` to reuse `Choice::offered`; the details now come from `choice::offer_details` (no new decision site). The
   inventory test is the regression test.
4. The first gate run of the card plays failed clippy (a complex type: `ability::Closed` is its alias; the gate step is the test)
   and the golden dual with 12 differences (Reflective Shielding stages hits the string's sustain window takes up afterwards;
   the card left the ported set, test `reflective_shielding_stays_in_the_string_in_lockstep`, which fails with it back in).
5. Found while developing (not by the gate), each with its lockstep test: the sources of the kernel scope were not lent to
   `BeginGroundRound` (the round agent saw no ground forces: `the_sol_agent_chooses_a_unit_in_the_core_window_in_lockstep`);
   the production dual dropped Argent's grant (`the_argent_agent_redirects_in_the_core_production_window_in_lockstep`);
   the end announcement bailed on any note an ability left (`salvage_operations_pays_in_the_core_window_in_lockstep`); the
   `HITS_TO_ASSIGN` flow did not spend what the window granted (`tellurian_cancels_a_hit_in_the_core_window_in_lockstep`).

Mutation checks (each fails the named test, run one by one): round agents: the exhaustion dropped, the roster index +1 (Sol
and Letnev tests), the `round_seq` payload dropped (Letnev); Argent: the exhaustion dropped, the adjacency inverted, the
window's clearing of the grant dropped; Self-Assembly Routines: the amount 1 -> 2, the nested `TRADE_GOODS_GAINED` emission
dropped (found by the new `nested.events` comparison); Tellurian: the grant dropped, the flow's spend dropped; Instinct
Training: the cancel, the token and the exhaustion dropped; I48S: the exhaustion dropped, the wrong unit form placed;
Card plays: the stamp not played, the pile push, `last_action_discarded`, the declaration, the play log and the option
order of the choice dropped or changed; Letani Warrior II: threshold 6 -> 5, the count not raised; Brother Milor: count 2 -> 1, the exhaustion dropped; Salvage
Operations: the amount 1 -> 2, the gain not staged. (One equivalent mutant is not caught: the unit roster built without the `upgrades_from` filter, as the first match is
the base unit.)

Tests per documented bail: `reactions_core::tests` (`bail_*`), registry equivalence for the ported abilities
(`the_native_registry_equals_the_string_one_for_the_ported_abilities`), the bail tests of section 3.1, and the lockstep
tests (`infantry_ii_...`, `a_reaction_slot_that_is_declined_...`, `a_yssaril_seat_with_a_reaction_card_...`, `the_cabal_captures_...`,
`the_sol_agent_...`, `the_letnev_agent_...`, `the_argent_agent_...`, `tellurian_...` (space combat and barrage),
`self_assembly_routines_...`, `letani_warrior_ii_...`, `brother_milor_...` (ground and ships), `salvage_operations_...`, `declared_reaction_cards_are_played_...`).

## 7. What the next package needs to know

* Adding an ability: a `Spec` in the owning crate (slot range, `Applies`, condition, effect), one `Named` line in
  `reactions_core.rs`, a lockstep test with the string scenario and a mutation check of each effect line; the
  registry-equivalence test then checks its registration. A question the effect asks needs `Ask`/`Opt`/`Subtype`
  variants and a byte-exact renderer (the trigger and the `active` detail of a window are completed by
  `complete_nested`: say in `with_trigger_of` / `is_window_pick` which the string question carries).
* An ability that changes state the codec only knows as text (a JSON blob mark, a staged event) needs a typed shape
  (`Sh::RoundBonus` is the model) or a staged event the window's adapter announces (`apply_staged`).
* A fact the core cannot compute (a galaxy, the content store) is the adapter's: `AdapterFacts` (see `with_reach`) or a
  `Unknown` for the cases it cannot answer. Prefer a core derivation checked against the string answer by a test
  (`the_core_placed_form_equals_the_string_placed_unit_id`).
* The production windows follow nested activity (`production_window_dual::step`); `pay_step` still builds an empty
  resolver, and a table with a Nomad or a Yssaril seat, a Keleres, Deepwrought, Naaz or Mentak producer or a Bastion or
  Crimson seat still keeps the whole window in the string (`unported_window`): those are the tier-W offers.
* The reaction-card play: stamps in `reaction_slot`, effects that read another area as `CardPlay`s in `ti4-engine-core-factions/src/cards/` (section 9). Next: Fire Team, Scramble
  (per-die reroll questions, the dice inside the announcement), Courageous (the producer in the casualty question) and Reflective Shielding (child windows, design 9.2).
* The next ports by census: Technological Singularity (the opponent's technology: needs the technology area), the Ssruu copies (a parametric slot family per lender/borrower pair), the Empyrean flagship and Watcher (adjacency
  from the map: a fact noted like `AdapterFacts::withdraw`), Sardakk's hero and mech, Salvage Operations' produce half.
* Event trace ids: the core numbers its events from the first id the string run used (`Recording.first_event_id`);
  an ability whose nested events skip ids in the string would show up as a trigger `event_id` difference.

## 9. Stage 3: the card effects of the table, the tier-W production, more abilities (branch `core-reactions`, third round)

### 9.1 Design: card effects that read another area (`CardPlay`)

The reaction slot (`rules/action_cards/reaction_slot.rs`) announces a declared card at the close of its window and plays the
stamp of a `Simple` effect itself. A card whose effect reads the combat or invasion area (the retreat, the ships of a system,
the sustained unit) cannot be played from the action-card area: areas do not import each other. The table therefore holds a
second registry, `AbilityTable::card_plays`: a `CardPlay { covers, effect }` per group of copies, registered by the faction
crate (L6, which may import every area): `ti4-engine-core-factions/src/cards/`. `reaction_slot::plays(table, card)` is the one
question "does the core play this card" (the adapter's `Recording::unported` asks it). `AbilityTable::after_play` runs after the
discard of a played card (`ACTION_CARD_DISCARDED` windows) and announces what the effect staged: the ships it destroyed
(`combat.pending_destructions`, announced with `assign::announce_loss`, so the payload (`last`, `galvanized`, cause) is the
combat's). A question the effect asks goes through `RulesHost::ask` (the tape, as an ability's).

Cards in the table now: Sabotage (nothing; see below), Intercept, Parley, Direct Hit, Skilled Retreat.

* **Sabotage, played at once.** The window of `ACTION_CARD_PLAYED:when` is the answer to a play, so the slot plays in it
  immediately (string `reactions::slot`: rules version 3 and later, event `ACTION_CARD_PLAYED`): `Eff::with_host` lends the
  host and the resolver, the card is spent, announced (its own `ACTION_CARD_PLAYED` opens the windows of the others) and
  discarded, and a Sabotage cancels the event that opened the window (`Eff::cancel`). The same path serves any card a seat
  plays in that window (the Empyrean mech's Watcher is an ability, not a card, and stays string).
* **Intercept**: every other combatant of the active system gets `retreat_barred_round` = this round.
* **Parley**: the unit in `last_committed_unit` goes from its planet back to space; a Naaz mech (the other form flips) is a
  documented bail (`UnportedCard`).
* **Direct Hit.** The string reads the `combat:sustain_target` blob mark (the exact sustained unit, written when the unit
  sustains) and removes that unit without the ship-lost observers; it destroys the first sustained ship of the type when there
  is no mark and (below `supply::staging_enabled`) the first ship of the type. The mark is now typed (`Sh::SustainTarget`:
  system, planet, owner, type, damage, galvanize token; JSON round trip checked) and written by the core windows themselves
  (`sustain_offer::remember_target`, from `SustainOffer::take` and the combat machine), so the Direct Hit can read and clear it
  and the adapters no longer write it after the window. The core derives the exact ship as the first sustained ship of the
  type and bails (`MissingFact`) where that could differ from the mark: a planet-standing combatant of the victim (Eidolon
  Maximum, an Alastor ground force), or sustained ships of the type that differ in the galvanize token.
* **Skilled Retreat.** The destinations are the map's; the windows are lent no galaxy. The string card notes them
  (`dual_window::note_withdraw`, a `Recording` field, an *input* of the card like the adjacency of Argent's agent) and
  `follow` installs them as `AdapterFacts::withdraw`. The core filters nothing (the string list already excludes systems with
  other players' ships and the Mahact bar), asks `combat::Ask::Withdraw` when there are several (prompt, `skilled_retreat_choose_system`,
  options "withdraw to X" with the ships-in-system preview, source the played copy), then runs the `Retreat` window (never asks)
  through `with_sx` and sets `combat_draw_round`. Bails (`UnportedCard`, as `dual_window::retreat_unported`): a Last Bastion seat,
  a Naaz dual-form unit, a structure that moves like a ship, an Eidolon Maximum on a planet. Found by its lockstep test: the combat
  machine did not look at the fight after the round opening (a card played at its start can end it), and it began the feat
  occurrence before the opening, so the machine now checks `over` before the barrage and begins the occurrence itself
  (`Env::begin_occurrence`).

### 9.2 Not ported, with the design (cards)

* **Reflective Shielding.** The string stages `pending_reflective_hits = (system, producer, 2)` (the core's `Simple::Reflective`
  writes it too) and the sustain step *drains it right after the emission*: the producer assigns two hits to its own ships
  (`absorb_hits_seeing_with_context` with origin `CombatRoll`, cause `action_card:reflective_shielding`, a full sub-flow: sustain
  offers, casualty questions, `SHIP_DESTROYED` announcements). That is a window inside a window; the core drivers have no child
  windows. Design: the drain becomes a stage of the owner (`SustainOffer::Stage::Reflective`, after `Commander`), which enqueues
  a `Pend { player: producer, hits: 2, producer: sustainer, .. }` at the front of the combat machine's hit queue (a `cause` field
  per `Pend`, today one per queue) and, for the standalone offer of the space cannon, runs an `AssignHits`/`SustainOffer` pair
  as a child through a `Child<W>` helper of `kernel::stage`. Until then the card bails (`unported ability reaction:*:...
  (card reflective)`, test `reflective_shielding_stays_in_the_string_in_lockstep`; the first golden run with it found 12 differences).
* **Courageous to the End** (dice, then the opponent's casualty choice with a `HitDetail` whose producer is the card's holder:
  the `Ctx` carries no producer; **ported in branch `b6d-hits` for the play window only**: `Fx::Courageous` runs the opponent's
  casualty as a child window of the combat area through the kernel links (`combat::effect_hits::Destroy`, `docs/B6_CARDS.md` last
  section), the `HitDetail` is the adapter's display fact, the destructions are staged and announced by `after_play`; played from a
  reaction slot it is still the string's, the census name `(card courageous)` stays), **Experimental Battlestation** (the same,
  `Absorb` = the sustain offer and the assignment as one child window), **Fire Team** and **Scramble Frequency** (per-die reroll questions with a new `RerollSource`,
  the ledger reason of the reroll, and the dice stream inside the announcement: `Recording.dice` only notes dice rolled inside
  an ability's effect, the announcement of a declared card is not one), **Reveal Prototype** (a printed cost the core does not judge
  and a research choice), a card from the salvage yard, Data Skimmer (Ral Nel).

### 9.3 Production: the tier-W factions

`production_view::unported_window` kept the whole production window in the string for a Deepwrought, Keleres, Naaz, Mentak,
Nomad or Yssaril seat, a Bastion and a Crimson seat. `unported_production` (the guard of `ProductionWindow::step` and
`production::pay_step`) now keeps only the Last Bastion, the Crimson (typed reads decline: `unported_economy`) and a Mentak
producer (a bail of the typed window for another reason: its Corsair form). The invasion's custodians and toll payments keep
`unported_window` (their own windows pay inside the core window).

The experiment behind it (a producer of each faction, agents readied, commodities to spend, pseudo-random answers) compared with
no difference except one: the Naaz mech stages `NAAZ_MECH_PLACED` (`supply::stage_naaz_mech_placed`) on every placement, which the
core window did not; it does now (`ProductionWindow::place`) and `production_window_dual::step` applies the staged events.
Test: `production::tests::the_tier_w_factions_run_in_lockstep_on_the_core_production_window` (fails without the staging).
Winnu's Berekar Berekon was already an option of the core question (`POpt::Agent`); the Argent agent is an ability of the table.

### 9.4 The tier-W offers as windows

The offers that open a use of PRODUCTION or a payment are not abilities of the resolver: the string asks them directly of the seat
between the windows of the production. Each is now a window of its own in `rules/production/offers.rs` with a call site
wrapped in `dual_window!(Area::Production, ..)`:

* `production::hero_offer` (`leaders::offer_production_hero`): Harrugh Gefhara, "make this use of PRODUCTION free", `Ask::Production(Hero)`,
  options `Use` / decline. Using it purges the card and sets `free_production_use` to the production sequence.
* `production::agent_offer` (`keleres::offer_agent`): Xander Alexin Victori III, one question per Keleres holder of a readied agent
  (`Ask::Production(AgentOffer { payer })`). Using it exhausts the agent and writes the payer's `KeleresAgentGoods` mark with the holder; if every
  holder declines the mark says `declined`. A Nomad that hears the exhaustion of an agent (`nomad_watches_agent_exhaustion`) is told by the string
  and not by the core: the window bails (`Unknown::NomadAgentExhaustion`, test `bail_the_keleres_agent_with_a_listening_nomad_is_counted`).
* The Winnu agent (`POpt::Agent`) and the Argent agent were in the core window already; the payment window's agent is the Keleres offer above, closed
  by the string after the payment (`keleres::close_agent_window`).

Found by the lockstep test of the offer: the core windows paid with `Extras::default()`, so neither Keleres' commodities (open inside an agent window) nor the
Deepwrought's ocean cards were ever options, which is why the guard kept those seats out. `rules::production::Held` now reads both through the hook table
(`keleres_spendable_commodities`, `deepwrought_spendable_oceans`) for the production window and the payment step; both windows apply the `Commodity` option
(`commodities -= 1`), and the observer of `ProductionWindowClosed` no longer closes the agent window at every placement (the payment window outlives a placement; the
string's caller closes it after the use). Tests: `the_keleres_agent_is_offered_in_the_core_window_in_lockstep`,
`a_payment_with_the_keleres_agent_runs_in_the_core_window_in_lockstep`, `the_hacan_hero_is_offered_in_the_core_window_in_lockstep`.

### 9.5 Abilities

| slot | string id | notes |
|---|---|---|
| 2001 / 2002 | `promissory:*:antivirus:{SPACE,GROUND}_COMBAT_STARTED:after` | Antivirus, the placement: the holder (a note of another faction) may place it faceup (optional) |
| 5001 / 5002 | `unit:*:mahact_infantry{,2}:GROUND_FORCE_DESTROYED:after` | Crimson Legionnaire: gain 1 commodity or convert 1 to a trade good (a question `reaction::Ask::Legionnaire { second }` when both are possible, the commodities in the details); the II waits on the card (`MahactCl2`). Registered for every seat as the string does. A Firmament or Obsidian seat (Siphon watches commodities) is `MissingFact`. |
| 8002-8004 | `leader:*:mentakhero:{SPACE_COMBAT_STARTED,SHIP_DESTROYED,SPACE_COMBAT_ENDED}:after` | Ipswitch, Loose Cannon: the purge (optional; the card is purged, `MentakHero` names the seat for the system), one ship of the base type of every destroyed ship of another player, of the form the owner places (`yin::timing::placed_form`, checked against `placed_unit_id` for every base type but cruiser and flagship: a destroyed cruiser or flagship bails with `MissingFact`), the record cleared at the end |
| 10001 / 10002 | `promissory:*:cavalry:SPACE_COMBAT_{STARTED,ENDED}:after` | The Cavalry: played at the start of a space combat (optional; a question `reaction::Ask::Cavalry` over the non-fighter ship types in id order when there are several; the note faceup, `NomadCavalry` = system and type) and returned to the Nomad at the end of the combat |

The registry of the table now follows the string order of the faction modules for abilities that share a bucket: Mahact and Mentak before Nekro,
Nomad and Yin (`reactions_core::table`; `the_native_registry_equals_the_string_one_for_the_ported_abilities` names the first difference).
`with_trigger_of` no longer gives the Legionnaire's and the Cavalry's question a trigger (the string builds none).

### 9.6 Census and gate (stage 3)

`TI4_NESTED_LOG=1 dual_run tools/engine_profile/profile-20.json 20 driver=dual,invasion=dual,production=dual,leaders=dual,exploration=dual,agenda=dual` (20 golden
games, gate profile): 11 655 304 calls, 11 742 671 comparisons, **0 differences**, hashes equal; `nested activity: 231 signatures, 736 runs`.

Nested bails (window calls that bailed with a `nested (...)` reason): **213 -> 144** (stage 2b -> stage 3). By window, before -> after:
announce_end 29 -> 20, answer 40 -> 22, begin_round 53 -> 32, settle 27 -> 19, commit_one 13 -> 3, ground_round_hits 21 -> 14, offer_sustain 3 -> 2,
bombard_step 2 -> 1, space_cannon_defense 5 -> 4, barrage_step 1, destroy_fighters 1, flow_step 2, begin_ground_round 9, roll_ground_tail 7 (the Fire Team: 6, a Sabotage that
no longer shows), ProductionWindow::step 0 -> 7 (the Ssruu copy of Argent's agent, now reached).

The production windows, compared -> bailed (all reasons): `ProductionWindow::step` 1374 -> 2924 compared, 2037 -> 487 bailed; `production::pay_step` 999 -> 1900 compared, 1182 -> 281 bailed;
new windows `production::agent_offer` 574 compared and `production::hero_offer` 273 compared, none bailed.

Reasons after (runs): Technological Singularity open / close / ground 17 + 12 + 2; Empyrean flagship (Dynamo) 14 + 1 and the mech's Watcher 7; the Ssruu copies (Yin ships and ground 10 + 9,
Letnevagent 9 + 2, Solagent 5 + 4, Argent's agent 6 + 1); Salvage Operations' produce half 8; a Nekro-carried technology 6; a card cost the core does not judge 6 (Bribery / Reveal Prototype);
Fire Team 6, Reveal Prototype 4, Courageous 2, Reflective Shielding 2, Scramble 1; Sardakk hero 4 and mech 2; the Winnu commander's unlock 1, Ambuscade 1, one fact the adapter did not compute, one window lent no dice
stream. Gone since stage 2b: Parley (9), Direct Hit (13), Skilled Retreat (15), Intercept (5), Sabotage (6), Mentak hero (16), Mahact units (9), the Cavalry (10), Antivirus (3).

Gate (`tools/engine_core/verify_package.sh driver OUT none 6`, `CARGO_BUILD_JOBS=3`, `--profile gate`, `SKIP_AB=1 SKIP_WASM=1`, private target dir, code commit `c9188214` plus the rename in the decision
inventory below): layers 0 violations (338 files), lint_core 0 hits (340 files), clippy 0 in core/bridge/dual/core_sync/render, immutable surfaces unchanged (against `04578c39`), `gen-cards --check`, xtask and wasm tests ok,
dual suite (6 suites, `driver=dual`, fail fast) ok, golden dual `driver=dual`: 4566 calls, 26 622 comparisons, 0 differences, hashes equal; the wide mask above (run on the same binary); `bench` + `bench-v1..v5` with `driver=core`
200 identical / 0 different each, `bench` with the driver on String 200/0; `board_equivalence templates` and `golden profile-20 20 10`: 0 mismatches. Not run (as agreed): the CPU A/B, the wasm size gate, release builds, `state_size`.

Failures found, each with a test that fails on the code before the fix:
1. `decision_delivery_inventory` (the gate's first `tests` run): `offer_agent` and `offer_production_hero` became the wrappers of the new windows and their decision sites moved to `offer_agent_text` /
   `offer_production_hero_text`; the reviewed registry names them (`every_producer_and_delivery_site_matches_the_reviewed_registry`, `every_indirect_producer_reaches_its_classified_delivery_api`). The test suite was re-run in full
   (`--no-fail-fast`): 41 suites ok.
2. Found by lockstep tests while developing (each fails without its fix): the combat machine did not look at the fight after the round opening and began the feat occurrence before it
   (`skilled_retreat_withdraws_the_fleet_in_the_core_window_in_lockstep`); the core production window paid with `Extras::default()` (no Keleres commodities, no ocean cards), did not apply the
   `Commodity` option and closed the agent window at every placement (`the_keleres_agent_is_offered_in_the_core_window_in_lockstep`, `a_payment_with_the_keleres_agent_runs_in_the_core_window_in_lockstep`); the core stage of the Naaz
   mech placement (`the_tier_w_factions_run_in_lockstep_on_the_core_production_window`); the core did not write the sustain target (`direct_hit_destroys_the_sustained_ship_in_the_core_window_in_lockstep`,
   `the_sustain_target_mark_is_typed_and_round_trips`); the registry order of the faction modules (`the_native_registry_equals_the_string_one_for_the_ported_abilities`); a trigger on the questions of the Legionnaire
   and the Cavalry (`the_crimson_legionnaire_pays_in_the_core_window_in_lockstep`, `the_antivirus_and_the_cavalry_are_played_in_the_core_window_in_lockstep`); the placed form of a Mentak cruiser
   (`the_core_placed_form_equals_the_string_placed_unit_id` now covers the ship base types).

Mutation checks (each fails the named test, run one by one; two first runs were invalid because a cargo filter held a space and were redone): Sabotage: the cancel dropped; the
after-play announcement dropped (Direct Hit); the sustain-target mark dropped; Direct Hit: the producer check inverted, the removal, the staged destruction, its `during_space_combat`; Intercept: the opponent filter, the round; Parley: the
move; Skilled Retreat: the draw round, the staging, the single-destination rule, the previews, the unported check, the Last Bastion check, the order of the destinations; the machine's check after the opening and the lazy occurrence;
Crimson Legionnaire: both-possible rule, the convert, the gain, the trade good, the card count; the Cavalry: the faceup placement, the return, the mark; Antivirus: the faceup placement; the offers: the free-use stamp, the purge, the
agent's mark, the exhaustion, the declined mark, the Nomad bail; the payment extras and the `Commodity` apply (pay step and window); Ipswitch: the purge, the record, the placement, the end, the other-player filter. Core halves in
`ti4-engine-core-factions/src/cards/mod.rs` (14 unit tests) are mutation-checked in the crate (7 mutations killed).

Tests per documented bail: `reactions_core::tests` (`bail_*`: the table-level bails, a card the core does not play, a card played at once, Data Skimmer, outside an ability), `cards::tests::{bail_direct_hit_on_ships_that_differ_by_their_galvanize_token,
bail_skilled_retreat_with_a_dual_form_unit_or_the_last_bastion, bail_parley_of_a_naaz_mech_whose_form_flips}`, `invasion::tests::bail_the_crimson_legionnaire_with_a_siphoning_firmament_seat_is_counted`,
`production::tests::bail_the_keleres_agent_with_a_listening_nomad_is_counted`, the cruiser bail in `the_mentak_hero_places_ships_in_the_core_window_in_lockstep`.
