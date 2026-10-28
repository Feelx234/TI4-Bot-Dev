# The reaction / ability port (package `core-reactions`)

Branch `core-reactions`, cut from `core-integration` 04578c39, with `core-b1-combat` (a01f1eb9), `core-b2-invasion`
(d3c9316e) and `core-b3-production` (aac6a4b3, already inside B2) merged in. Nothing pushed.

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

The reaction-card slot (`reaction:`) needs `playable_now` (hand, window table, `reaction_can_matter`, may-play bars)
and the whole action-card effect set (B6, not merged). It is registered in no core table; a window where it is
eligible bails with its id. The same holds for every ability not yet ported.

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
2. ran the effect of an `eligibility_only` ability (a reaction card was played);
3. did something nested that no ability explains (`nested outside an ability`: a question or an event inside an
   emission outside every effect and every pick);
4. has no resolver configuration (`nested activity without a resolver`);
5. reached a condition the core cannot answer (`Unknown`: Yssaril Transparasteel Plating, Bribery's goods, a card
   cost the core does not judge, Harness Energy / Infiltrate);
6. rolled dice in an ability in a window that is lent no stream (the lend fails and the window bails silently).

Each has a unit test (`reactions_core::tests`). An ability whose condition failed never bails a window.

## 4. Ported abilities

| slot | string id (`*` = the owner's faction) | notes |
|---|---|---|
| 1 | `technology:*:inf2:GROUND_FORCE_DESTROYED:after` | rolls a die from the dice stream, `Inf2Card` mark; the return at TURN_BEGAN is string |
| 100+i | `reaction:*:<EVENT>:<rel>` (one per printed window) | eligibility and offer only |
| 3001 | `unit:*:cabal_mech:GROUND_FORCE_DESTROYED:after` | Reanimator |
| 3002 / 3003 | `ability:*:devour:SHIP_DESTROYED:after` / `...GROUND_FORCE_DESTROYED:after` | Devour |
| 3004 / 3005 | `unit:*:cabal_flagship:SHIP_DESTROYED:after` / `...GROUND_FORCE_DESTROYED:after` | The Terror Between (a Cabal seat; the Nekro-carried text stays string) |
| 3006-3009 | `ability:*:devour_mark` / `devour_clear` on SPACE/GROUND_COMBAT_STARTED/ENDED | participation marks |

Not ported (designed, not done in this package): Instinct Training and I48S (the two other S8 slots: `Spec`s with an
`Ask::Reaction` variant each, `Unknown` for a Nekro-carried card), the tier-W faction offers (Keleres/Winnu agent, Hacan
hero, payment-window agent, Argent agent redirect: they are `Stage::Offering` points of B3's production window, not
reaction windows, and need the production dual to follow nested activity as `drive_with` now does for the others),
and every other registered ability (about 270; the census names them by frequency).

## 5. Census: the nested bails before and after

`TI4_NESTED_LOG=1 dual_run tools/engine_profile/profile-20.json 20 driver=dual,invasion=dual,production=dual,leaders=dual,exploration=dual,agenda=dual`
(the 20 golden games, 0 differences, hashes equal both times). The run prints `nested activity: N signatures, M
runs`: 203 signatures, **736 runs** in which a string window had nested activity (a reaction window, an ability).
Before this package every one of them was a bail; only the invasion windows said so (199 reported; the combat
windows bailed silently).

| window | nested bails before | after | compared before -> after |
|---|--:|--:|---|
| invasion::ground_round_hits | 82 | 61 | 280 -> 301 |
| invasion::begin_ground_round | 69 | 69 | 248 -> 248 (every one is the Sol agent) |
| invasion::commit_one | 23 | 16 | 2007 -> 2014 |
| invasion::roll_ground_tail | 13 | 7 | 307 -> 313 |
| invasion::bombard_step | 6 | 2 | 1371 -> 1375 |
| invasion::space_cannon_defense | 6 | 6 | 1074 -> 1074 |
| combat::settle | silent | 64 | 319 -> 360 |
| combat::begin_round | silent | 61 | 151 -> 182 |
| combat::answer | silent | 50 | 793 -> 814 |
| combat::flow_step | silent | 41 | 1822 -> 1838 |
| combat::barrage_step / offer_sustain / announce_end / destroy_fighters | silent | 9 / 3 / 4 / 1 | 183 -> 185 / 98 -> 101 / 195 -> 195 / 11 -> 11 |

152 windows that bailed on nested activity before are compared now (21% of the 736 runs); every bail that remains
is reported with the ability that causes it. Reasons, by runs, after:

| runs | reason |
|--:|---|
| 64 | `leader:*:solagent:GROUND_COMBAT_ROUND_STARTED:after` (Sol agent, 69 with the Ssruu copies; the next port) |
| 42 + 30 + 14 + 13 + 3 | `reaction:*:...` a card was played (COMBAT_ROUND_STARTED, HITS_TO_ASSIGN, SUSTAIN_DAMAGE_USED, UNITS_COMMITTED, ...): the play is B6's effect set |
| 38 | Yssaril Transparasteel Plating (the faction hook is not in the core kernel the windows use) |
| 17 | `tech:*:sar:GROUND_FORCE_DESTROYED:after` (Saar) |
| 16 + 15 + 9 + 7 | Yin agent (and the Ssruu copy), ground forces and ships |
| 15 + 8 | Titans agent (HITS_TO_ASSIGN, GROUND_HITS_TO_ASSIGN) |
| 15 | `ability:*:technological_singularity_open:SPACE_COMBAT_STARTED:after` (Nekro) |
| 14 | `unit:*:empyrean_flagship:SUSTAIN_DAMAGE_USED:after` |
| 9 | Mentak hero |
| rest | Mahact infantry II/Crimson Legionnaire, Sardakk, Arborec infantry II, `so` (Sol's Spec Ops), promissory notes (Cavalry, Antivirus) |

Greedy order of the next ports by the runs they free (the covering of `reaction:` as a decline is already counted):
Sol agent (+60), Argent agent in the production window (+50), Cabal done, Letnev agent (+30), Saar, Yin agent, Titans
agent, the Nekro Technological Singularity pair.

## 6. Gate (on this branch)

* `tools/engine_core/verify_package.sh driver` (CARGO_BUILD_JOBS=3, `--profile gate`, `RUN_AB`/`RUN_WASM` off):
  layers 0 violations (324 files), lint_core 0 hits, clippy 0 in core/bridge/dual/core_sync/render, immutable
  surfaces unchanged (web/, RULES_VERSION.md, model state.rs, wasm fixtures, vs `core-integration`), 41 test suites
  ok, `gen-cards --check` ok, xtask and wasm tests ok, benches `bench` + `bench-v1..v5` with `driver=core` all 200
  identical / 0 different, `bench` with the driver on String 200/0.
* The first run of the gate failed two steps, both fixed on the branch: the dual suite (a window test counts into
  the process-wide report, and a global `TI4_AREAS=driver=dual` makes every test a dual run: `FOCUS` now routes the
  other threads into a throwaway report while a `serial()` test holds it) and the golden dual (52 differences in the
  `context.previews` of reaction windows: display facts the string builds from its position; they are the recorded
  question's now, like the option previews of B1g-10). After the fixes: dual suite (all targets) ok; golden dual
  `driver=dual` 0 differences, hashes equal; the wide mask above 0 differences, hashes equal.
* The benches were measured on the commit before those two fixes; the fixes touch the Dual path and test code only.
* Mutation checks (each fails the lockstep test): Infantry II threshold 6 -> 5 (dice and `inf2:card` mark differ);
  Devour taking the captor's own models (state differs).
* Tests per documented bail: `reactions_core::tests` (`bail_*`), registry equivalence for the ported abilities
  (`the_native_registry_equals_the_string_one_for_the_ported_abilities`), lockstep tests in `invasion.rs`
  (`infantry_ii_rolls_in_the_core_window_in_lockstep`, `a_reaction_slot_that_is_declined_is_followed_by_the_core_window`,
  `the_cabal_captures_in_the_core_window_in_lockstep`) and the driver tests for the nested facts.

## 7. What the next package needs to know

* Adding an ability: a `Spec` in the owning crate (slot range, `Applies`, condition, effect), one `Named` line in
  `reactions_core.rs`, a lockstep test with the string scenario; the registry-equivalence test then checks its
  registration. A question the effect asks needs `Ask`/`Opt`/`Subtype` variants and a byte-exact renderer (the
  trigger and the `active` detail of a window are completed by `complete_nested`).
* The production window (`ProductionWindow::step`, `pay_step`) has its own dual (`production_window_dual.rs`) that still
  builds an empty resolver: the Argent agent (54 runs) and the tier-W offers need `drive_with`'s nested handling there.
* `Unknown` is the way a condition says "the core cannot answer": add a variant and its wording in `drive_with`.
* Event trace ids: the core numbers its events from the first id the string run used (`Recording.first_event_id`);
  an ability whose nested events skip ids in the string would show up as a trigger `event_id` difference.
