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
