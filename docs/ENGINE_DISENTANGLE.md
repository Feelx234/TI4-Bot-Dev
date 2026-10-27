# Untangling the windows from the factions and cards, so the core ports can run in parallel (2026-10-27)

Design only. Written on branch `core-disentangle-design` from `core-state-main` (`201f6714`); read-only on code, no
builds, no tests. Inputs: `ENGINE_CORE_STATE.md` (sections 3, 4, 6, 7), `ENGINE_CORE_DECISIONS.md`, and the
unmerged package branches as they stand today: `core-a5-kernel` (tip `39b20cb8`: `kernel/hooks/**`, `Resolver`,
reaction dual), `core-b1-combat` (tip `3678aede`: `rules/combat.rs`, `core_combat.rs`), `core-b3-production`
(tip `d24c750e`: `rules/production.rs`, `production_view.rs`), `core-a2-movement-tactical` (tip `98a7039e`).

**How the numbers were made.** Everything below "measured" is a grep/AST-free text count over
`crates/ti4-engine/src/**` with the `#[cfg(test)] mod tests` tail cut off (the files are 50% tests: `combat.rs` is
12 383 lines, 6 603 of them non-test; `factions/` is 82.8 k lines, 38.0 k non-test). A "reference" is one
`module::` path occurrence on a non-comment line; it is not the same as a call site (a `use` alias is invisible, a
line with two calls counts twice), so read the counts as +-10%. The scripts are in the Appendix so the numbers can
be reproduced; they are not committed tools. "Estimated" is labelled.

## 0. Summary

1. The coupling is **narrower than it feels, and more regular**. From `combat`, `invasion` and `production` there
   are only **13 faction modules** with direct references (107 references in total, table 1.1); the other
   14 factions reach those windows only through the registered `Hooks`. Content-id string literals in the windows
   are few (combat 40, production 33, invasion 23; the 142 action cards sit in `action_cards.rs` with 349).
2. The hard part is not the number of call-outs, it is that they are **of eight different kinds, each spelled a
   different way**: 138 of the 190 hook slots that A5 defined are pure queries over `&CoreState`; only 52 mutate
   or ask. The A5 `HookTable` is a faithful *transcription* of today's call sites (99 `factions::Hooks` slots +
   91 "direct" slots named `bastion_extra_die`, `nekro_units_mech_roll_bonus`, ...). So after A5 the window still
   knows *which faction* it is talking to; only the `use` changed. That is why B1 wrote a 13-method `Faction`
   trait and B3 a 10-method `Env` trait next to the table: each window re-invents its own seam.
3. Proposal: **one seam per pattern, not per faction and not per window** (section 2): modifier stack,
   predicate, contribution list, rule-step replacement, observer, offer point. Eight of the 91 direct slots
   collapse into one `hit_shift` stack and one `dice` stack for combat alone; `argent::clear_agent_production_destinations`
   (7 references in `production.rs`) becomes one `ProductionWindowClosed` observer.
4. **Non-faction providers** are the other half and are not on the table at all: in `combat.rs` the references to
   `laws`, `relics`, `technology`, `promissory`, `leaders`, `neutral_units`, `supply`, `diplomacy`,
   `faction_abilities` (about 75) outnumber the faction ones (46). They must feed the same seams, as
   `query::` functions of the packages that own them (A1 laws, A3 technology, B5 relics/promissory), registered as
   sources of the pattern, not called from the window.
5. **Most of it can be done first in the string engine as pure refactors** (section 3): extract the composition of
   each seam out of the window into a `<window>_effects.rs` of plain functions, called from one place, dual-checked
   at the seam. This is the move that unblocks parallel work, because it turns "port combat" into "port seam X
   (3-6 functions)" + "port the machine".
6. **Wave C is re-cut by seam and blocking order** (section 4), not alphabetically by file size: five packages of
   7.0-8.1 k non-test lines each, every package split into tiers Q (pure hooks; can start the day the seam
   signatures are frozen, validated by a hook-level dual with no window needed), O (observers; need the event
   payload types), W (asks/offers; ported with the window that owns the point). 73% of the hook slots are tier Q.
7. **Windows as explicit state machines in core: yes** (section 6). The string windows already are: `combat::Stage`
   has 9 variants and `production::Stage` exists; what is *not* a machine is the **205 inline `table.ask*` calls**
   (75 of them in faction modules, 11 in `production.rs`, 7 in `combat.rs`, 10 in `invasion.rs`, 17 in
   `action_cards.rs`) that rely on a synchronous `Decider`. Each becomes a named stage. That list, not the
   line count, is the size of a window port.
8. Biggest risks: hook application order (the dice stream is a function of `MODULES` order, and Keleres is three
   modules), option order inside offers and contributions (it is part of the decision hash), and inline asks that
   change when a question is posed relative to an event. Section 5: a hook-level dual, an order-mutation audit for
   the commutative kinds, and the decision corpus.

---

## 1. Measuring the coupling

### 1.1 Direct references from the windows into faction modules

Rows: faction module files (`factions/<name>.rs`); columns: the window group. `movement` = `movement + transit +
tactical`; `reactions` = `reactions + timing + registry`; `agenda` = `agenda_effects + vote + agenda`; `leaders` =
`leaders + faction_abilities`; `game` = `game + phase + wiring`. Cell = number of `<name>::` references, `.` = 0.
Hook files (`hooks_*`) and `factions::<fn>` dispatchers are in 1.2.

| faction module | combat | invasion | production | movement | reactions | action_cards | agenda | leaders | game | total |
|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| keleres | . | . | 15 | . | . | 1 | . | 1 | 5 | 22 |
| crimson | 4 | 9 | 1 | . | . | . | . | . | 2 | 16 |
| bastion | 8 | 3 | 2 | . | . | . | . | . | . | 13 |
| mahact | . | . | . | . | . | . | 10 | . | . | 10 |
| deepwrought | . | 5 | 1 | . | . | 1 | 2 | . | . | 9 |
| ralnel | 4 | 1 | 1 | 1 | . | . | . | . | 1 | 8 |
| bastion_units | . | 6 | 2 | . | . | . | . | . | . | 8 |
| nekro_units | 6 | 1 | . | . | . | . | . | . | . | 7 |
| borrowed_round_agents | 3 | 3 | . | . | 1 | . | . | . | . | 7 |
| argent | . | . | 7 | . | . | . | . | . | . | 7 |
| keleres_units | . | 4 | . | 1 | . | 1 | . | . | . | 6 |
| firmament_plots | 3 | 3 | . | . | . | . | . | . | . | 6 |
| firmament | 1 | 4 | . | 1 | . | . | . | . | . | 6 |
| deepwrought_research | . | . | . | . | . | 6 | . | . | . | 6 |
| nomad_agents | 1 | 1 | . | . | . | . | . | 2 | . | 4 |
| mahact_units | 3 | . | . | . | . | . | . | . | 1 | 4 |
| crimson_cards | . | . | 3 | . | . | . | . | . | . | 3 |
| winnu | . | . | 2 | . | . | . | . | . | . | 2 |
| nomad / nekro / obsidian / ralnel_cards / empyrean_units | . | . | . | . | . | 1 | 2 | 2 | 2 | 7 |
| **total** | **33** | **40** | **34** | **3** | **1** | **10** | **14** | **5** | **11** | **151** |

Reading:

* **Movement and the reactions kernel are already clean.** `movement.rs` has 0 direct references; faction
  behaviour reaches it through `MovementHooks` (22 slots, 14 registering factions) which `MovementRules::with_laws`
  folds into a plain struct of flags and sets (`nebulae_open`, `supernovae_open`, `also_adjacent`, ...) once per
  search. That is the model to copy (section 2.0), and it is why A2 got -25% CPU with a small port.
  `reactions.rs` points the other way: it *registers* cards and factions (`arm`, 1018-1075: the table-driven
  `slot()`, `instinct_training`, `l1z1x_agent`, `infantry_ii`, `magen_defense`, `borrowed_round_agents::abilities`,
  and `factions::timing_abilities`, which every one of 24 factions implements).
* **Combat, invasion, production are 107 of 151 references**, in 13 modules. These are the modules that block
  B1/B2/B3: `keleres`(+units), `crimson`(+cards), `bastion`(+units), `deepwrought`, `ralnel`, `nekro_units`,
  `borrowed_round_agents`, `argent`, `firmament`(+plots), `mahact_units`, `nomad_agents`, `winnu`.
* `mahact::` (10 in agenda) and `deepwrought_research::` (6 in action cards) belong to B9/B6, not to the windows.
* The "old six" (Sol, Hacan, Letnev, Xxcha, Jol-Nar, L1Z1X) have **no module**: their behaviour is literal text in
  the shared files. Faction-name literals: `leaders.rs` 84, `combat.rs` 25 (`naaz` 8, `letnev` 8, `jolnar` 4,
  `hacan` 2), `invasion.rs` 9, `reactions.rs` 5, `faction_abilities.rs` 8. `factions::flagship_has_text(state, owner,
  unit_type, "<flagship id>")` is the one construct that spreads this (combat 5 + 1, invasion 1; the Nekro Z-token
  lends the text of another faction's flagship). B1's `FlagshipText { Jolnar, Letnev, L1z1x, Hacan }` enum is a
  workaround for it.

### 1.2 Hook registrations: which factions plug into which group

`Hooks` slots a faction module sets (`hooks: Hooks { .. }` blocks, counted per group). Totals: general 132,
movement 31, economy 23, strategy 16, combat 9, ground 9, cards 4 = **224 registrations** by 24 factions.

| group (A5 file) | slots in table | registrations | registering factions (count of slots each) |
|---|--:|--:|---|
| combat (`hooks/combat.rs`) | 9 | 9 | ghost 1, mentak 2, yin 1, crimson 1, argent 2, naalu 1, naaz 1 |
| ground (`hooks/ground.rs`) | 8 | 9 | crimson 1, mentak 1, naalu 2, naaz 1, sardakk 2, titans 1, winnu 1 |
| economy (`hooks/economy.rs`) | 20 | 23 | naaz 5, yssaril 5, arborec 1, argent 2, cabal 2, ghost 2, mentak 1, muaat 1, titans 2, winnu 1 |
| movement (`hooks/movement.rs`) | 22 | 31 | cabal 4, crimson 4, empyrean 4, ghost 3, argent 2, saar 2, muaat 2, winnu 2, nomad 2, firmament 2, deepwrought 1, mentak 1, yssaril 1, ralnel 1 |
| cards / strategy | 7 / 8 | 4 / 16 | yssaril, nomad, argent / mahact, naalu, obsidian, winnu, cabal, yin, mentak, saar |
| general (`hooks/mod.rs`) | 26 | 132 | `timing_abilities` 24, `commander_unlocked` 21, `leader_action` 19, `use_leader` 18, `perform_component` 16, `component_actions` 13, `unit_roll_modifier` 5, the rest <= 3 |
| direct (`hooks/direct.rs`) | 91 | n/a | one provider each (kind `call`), named `<faction>_<fn>` |

The `FactionModule` order is `MODULES` (27 entries; Keleres is three: `MODULE_M`, `MODULE_X`, `MODULE_A`). A5
documents that this order fixes the dice stream. Faction code outside its own hook slots: 75 inline
`table.ask*` calls (arborec 8, winnu 7, yssaril 6, naaz 4, mahact_units 4, argent 4, sardakk 4, ...).

### 1.3 Hook kinds per slot, and how many slots can start without a window

A5's combining kinds, counted over its 190 slots (`hook_group!` keywords): `call` 57, `any` 37, `each` 31,
`concat`/`concat_sorted` 29, `sum` 12, `first` 8, `fold` 7, `try_*` 6, `all` 2, `any_some_true` 1.
Pure (first argument `s: &CoreState`, no `cx: &mut Cx`) versus mutating/asking: direct 61/30, combat 7/2, ground 7/1,
economy 15/5, movement 21/0, cards 5/2, strategy 5/3, general 17/9 = **138 pure, 52 mutating** (73% pure).

### 1.4 What each window calls into, by hook kind

Hand-classified from the grep of each file (`combat.rs` lines ~190-5060, `production.rs` ~279-3720). +-3 per cell.
The kinds are the six patterns of section 2 plus "event" (emits) and "state" (reads a `GameState` field that a
card/faction writes: not a call-out but the same coupling).

| pattern | combat (about 62) | production (about 60) | invasion (about 55) | movement | reactions kernel |
|---|---|---|---|---|---|
| P1 modifier on a number | `combat_modifier`, `unit_roll_modifier`, `mahact flagship_roll_bonus`, `nekro mech_roll_bonus`, `assail_space/against`, `laws::fighter_combat_bonus`, `unit_dice`, `bastion::extra_die` x3, `borrowed_round_agents::extra_die_for`, `relics::extra_barrage`, Jol-Nar flagship text (about 14) | `planet_spend_value` x3, `extra_production`(+planet), `production_cost_reduction` x2, `trade_good_worth`, `keleres::spendable_commodities` x3, `bastion_units::resource_bonus` x2, `laws::planet_value_bonus` x2, `armada_bonus` (about 15) | `unit_roll_modifier`, `unit_dice`, `bastion extra_die`, `assail_ground`, `ground_rolls_extra_hits` (about 8) | `move_bonus` | `Frequency`/`repeatable_in_window` counters |
| P2 predicate / veto | `may_sustain`, `grants_sustain`, `*_hit_immune`, `space_cannon_barred`, `crimson abilities_lost` x4, `ralnel silenced_in_space` x2, `firmament space_cannon_silenced`, `mahact ship_movement_barred`, `laws::sustain_suppressed` x2, `neutral_units::is_neutral`, `technology::{amd,nes,det}`, `leaders::pays_on_sustain` (about 22) | `cannot_produce` x3, `crimson abilities_lost`, `ralnel silenced_in_space`, `entropic_scars::abilities_usable` x2, `laws::{production_forbidden_on,planet_is_demilitarized,structure_cap_lifted,single_unit_production}` x5, `winnu agent_ready` (about 13) | `crimson_can_deploy`, `keleres_units commit_blocked`, `firmament viper_open`, `ralnel alarum_on`, `deepwrought research_team_open` x3 (about 12) | 12 `any` slots (`blocks_passage`, `may_move_through_ships`, `may_enter_supernova`, ...) | ability `condition`, `cannot` prohibitions |
| P3 contribution list | `nekro alastor_participants`, `ralnel linkship_guns` (about 4) | `production_destinations`, `bastion adjust_ship_spots`, `crimson_cards offer_card_spot`, `deepwrought spendable_oceans`, `mobile_docks` (about 5) | `crimson_deploy_planets`, commit candidates (`sardakk`, `naalu`) (about 5) | 7 `concat`/`concat_sorted` (`linked_systems`, `blocked_borders`, `severed_systems`, `extra_wormholes`, ...) | `timing_abilities` (24 factions), `slot()` per seat |
| P4 replacement of a rule step | `borrowed_stats` x2, `galaxy_for_mover`, L1Z1X flagship rule (about 4) | `unit_form_override`, `production_unit_exchange`(+performed), `breakthroughs::free_capacity` x2, PDS alternative (about 5) | `skip_to_commit`, `temporary_ground_force`, `bastion cancel_bombardment_hits` (about 5) | `own_unit_passage`, `unladen_pass` | `Ability.effect` bodies |
| P5 observer / bookkeeping | `bastion note_ship_lost`, `take_ship_lost` x2, `note_retreat`, `clear_retreated`, `nekro alastor_decrement/clear` x3, `space_combat_round_started`, `produced_hits` (about 9) | `argent clear_agent_production_destinations` x7, `bastion ship_placed`, `crimson_cards put_on_card`, `breakthroughs::on_production_finished`, `laws::prophecy_after_production`, `emit UNITS_PRODUCED` (about 13) | `ground_combat_round_started/ended`, `deepwrought begin_coexisting`, `control_gained` (about 8) | none | the emit itself (128 event kinds) |
| P6 offer / nested ask | `munitions_after_roll`, `afb_excess`, `produced_hits`, `nomad watches_roll_step`, `promissory war_funding`, `diplomacy::evaluate_event` (about 7) | `keleres offer_agent` x3 + `close_agent_window` x4, `winnu exhaust_agent_for_production`, `leaders::offer_production_hero` (about 9) | `keleres_units pay_commit_toll` x3, `firmament viper`, `deepwrought research_team` (about 8) | none | the window itself |

Takeaways: (a) P2+P1 are 60% of combat and production; they are pure queries, so they never need a window;
(b) P5 shows how much of the coupling is *cleanup and bookkeeping at window boundaries* (argent's 7 calls, bastion's
5) which belongs to events, not to the window code; (c) P6 is small (about 8 per window) but is where ordering
and decision hashes live.

### 1.5 The other axis: how a window asks

* `impl Window for` appears 6 times in non-test code (`choice.rs`, `game.rs`, `combat::CombatWindow`,
  `production::ProductionWindow`, `invasion::InvasionWindow`, `diplomacy/window.rs`); `Game` additionally keeps 15 `Option<Window>` fields
  (the core's `WindowKind`, `state/pending.rs`).
* **205 inline asks** (`Table::ask_seeing` / `Table::ask` from non-test code): `game.rs` 18, `action_cards.rs` 17,
  `production.rs` 11, `legendary.rs` 11, `invasion.rs` 10, `leaders.rs` 9, `relics.rs` 7, `technology.rs` 7,
  `combat.rs` 7, `faction_abilities.rs` 6, `agenda_effects.rs` 4, factions 75. They return the answer to the caller
  synchronously. The engine copes with a human seat because the host replays the game with a scripted decider;
  the typed core's `Pending` stores *what is asked*, so every inline ask has to become a stage (6.3).
* Rerolls: `state.reroll_staging` is read/written in `combat.rs` 25, `invasion.rs` 14, `action_cards.rs` 5,
  `game.rs` 3, `faction_abilities.rs` 1. It is a shared sub-window (stage the rolls, offer a reroll per die, commit),
  not a combat detail.
* Events: 128 event kinds (A5's closed `EventKind`); emit sites: `game.rs` 137, `timing.rs` 58, `invasion.rs` 19,
  `combat.rs` 16, `reactions.rs` 8, factions about 60.
* Content ids: string literals that are card/unit/relic ids: `combat.rs` 40 (units 20 over 10 distinct, leaders 6,
  relics 5, promissory 5), `production.rs` 33 (units 25), `invasion.rs` 23, `reactions.rs` 11, `action_cards.rs` 349
  (304 are action-card aliases, 119 distinct), `agenda_effects.rs` 149 (132 agenda aliases, 62 distinct). The
  windows therefore are not "card-id switches"; `action_cards.rs` and `agenda_effects.rs` are, and they are
  separate packages (B6, B9).

### 1.6 Why B1 and B3 stalled, in these terms

* B1 (`core-b1-combat`) has `rules::combat::Faction` with 13 methods because 6 of the 13 calls have no table slot
  that matches (`flagship_has_text`, `law_fighter_bonus`, `combat_modifier` answer neutral in `KernelFaction`) and
  the rest are `direct` slots that name one faction. The dual passes the *string engine's* answer for each method,
  which is correct but means the combat port is verified only against the one string implementation of every
  seam, never against the table.
* B3 (`core-b3-production`) has `trait Env` with 10 methods (`planet_spend_value`, `extra_production`,
  `cannot_produce`, `abilities_lost`, `silenced_in_space`, `resource_bonus`, ...), again one per call-out.
  Production's P6 (Keleres/Winnu agents, hero) cannot be expressed as an `Env` method because it asks.
* Both ported the **query half** (reads, 20-30% of the area) first and stopped at the machine, which is where P5/P6
  and inline asks sit.

---

## 2. Patterns and the uniform seams

### 2.0 Principle: windows read data, factions fill data

`MovementRules` is the existing proof: the faction hooks are consulted **once**, folded into a plain struct, and the
search never sees a faction. Generalise: for each window there is a small `*Rules`/`*Sheet` value that the *seam*
builds from all providers (factions, technologies, relics, laws, promissory notes, leaders) at the points where
the string engine consults them, and the window machine reads only that value. Providers see a read-only `Q`.

Three layers, top to bottom (this refines the L-table of `ENGINE_CORE_STATE.md` 4.1; nothing new is added to
the layer order):

```text
window machine (L4)   rules/combat/*.rs      reads Seam values + calls fx; knows no faction, no card, no relic
seam           (L3)   kernel/seam/<area>.rs   composes providers in the fixed order; one function per call-out group
providers      (L2/L6) query/{laws,tech,relics,...}.rs   and   ti4-engine-core-factions/<f>/*.rs
```

The seam layer is the place where today's *order of evaluation* is written down once (see 5.2). It replaces
B1's `Faction` trait and B3's `Env` trait: they become `Seam` implementations whose body is the table, with no
per-window trait.

```rust
// kernel/seam/mod.rs  (new; owner: seam steward, see 4.3)
/// What every pure provider gets. `Copy`, two pointers.
#[derive(Copy, Clone)]
pub struct Q<'a> { pub s: &'a CoreState, pub k: &'a Kernel }

/// Where a contribution comes from (used for details/UI facts and the order-mutation audit).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Source {
    Faction(Faction), Technology(Technology), Relic(Relic), Law(Agenda), Promissory(Promissory),
    Leader(Leader), Breakthrough(Breakthrough), ActionCard(ActionCard), Printed, Rule(RuleRef),
}
```

### 2.1 P1 modifier stack on a number

*Call-outs*: the `sum`/`fold` slots and the direct slots that return a bonus (`combat_modifier`, `unit_roll_modifier`,
`unit_dice`, `bastion_extra_die`, `borrowed_round_agents_extra_die_for`, `nekro_units_mech_roll_bonus`,
`mahact_units_flagship_roll_bonus`, `firmament_plots_assail_*`, `planet_spend_value`, `extra_production`,
`production_cost_reduction`, `trade_good_worth`, `keleres_spendable_commodities`, `bastion_units_resource_bonus`,
`move_bonus`, `fleet_supply`, `status_tokens`, `vote_bonus`).

Representation: a typed `Site` per stack, a `Delta` per provider, a fixed `Layer` order that equals today's
order of terms. `Add` contributions commute; only `Set` and clamps are order-sensitive, and those carry a
`why` so that A5's "an absolute set must say why" holds.

```rust
pub enum Delta { None, Add(i32), Set { to: i32, why: SetWhy } }
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer { Printed, Technology, Relic, Law, Faction, Leader, Card, Clamp }

pub struct Provider<S> { pub source: Source, pub layer: Layer, pub f: fn(Q, &S) -> Delta }
pub struct Stack<S> { items: Vec<Provider<S>> }          // sorted by (layer, registration index) at HookTable::seal()
pub struct Evaluated { pub value: i32, pub parts: SmallList<(Source, i32), 8> }   // `parts` feeds the UI facts
impl<S> Stack<S> { pub fn eval(&self, q: Q, site: &S, base: i32, clamp: Clamp) -> Evaluated; }

// combat
pub struct RollSite { pub unit: CombatUnit, pub kind: RollKind /* Combat|SpaceCannon|Barrage|Bombardment|Ground */,
                      pub round: u32, pub system: System, pub planet: Option<Planet>, pub foes: SeatSet }
// HookTable additions (hook_group! gets the keywords `stack`, `observe`, `offer` below)
stack  hit_shift(site: &RollSite)  -> i32;   // = combat_modifier + unit_roll_modifier + mahact/nekro bonuses + assail + law fighter bonus
stack  dice(site: &RollSite)       -> i32;   // = unit_dice fold + bastion extra die + round-agent extra die
stack  planet_spend(site: &SpendSite) -> i32;  // production.rs `planet_spend_value`, `resource_bonus`, `planet_value_bonus`
stack  extra_production(site: &ProdSite) -> i32;
stack  production_discount(site: &ProdSite) -> i32;
```

What it replaces: `combat.rs` lines 340-435 (six calls in `roll_for_unit`/`hit_value` territory), 1276-1306, 1734-1744,
2380-2389, and the same three in `invasion.rs`: roughly 17 call-outs across two windows become two `eval`s.
**Existing slots are kept as provider adapters**: a faction that registers `unit_roll_modifier` today is wrapped
as `Provider { source: Faction(f), layer: Faction, f: ... }` by the C shard with one line; the string order is
reproduced by `(layer, registration index)`. Where the string code's order is not layer order (see 5.2) the
provider declares its layer explicitly, and the order-mutation audit (5.3) proves the `Add` ones do not matter.

### 2.2 P2 predicate / veto

*Call-outs*: `may_sustain` (`all`), `*_hit_immune` (`any`), `space_cannon_barred`, `crimson_abilities_lost`,
`ralnel_silenced_in_space`, `firmament_space_cannon_silenced`, `cannot_produce`, `blocks_passage`,
`may_move_through_ships`, `mahact_units_ship_movement_barred`, `crimson_can_deploy`, `keleres_units_commit_blocked`,
`laws::sustain_suppressed`, `laws::production_forbidden_on`, `entropic_scars::abilities_usable`, the `condition`
of every timing ability.

```rust
pub enum Verdict { Abstain, Allow, Forbid }
pub struct Rule<S> { pub source: Source, pub f: fn(Q, &S) -> Verdict }
pub enum Combine { AnyForbidsWins /* all(): every module must allow */, AnyAllowsWins /* any() */ }
pub struct Pred<S> { rules: Vec<Rule<S>>, combine: Combine }
impl<S> Pred<S> { pub fn decide(&self, q: Q, site: &S, default: bool) -> bool;
                  pub fn why(&self, q: Q, site: &S) -> SmallList<Source, 4>; }  // for "details" (who barred it)
```

Slots keep their A5 names but are re-typed `pred` with a typed site, e.g. `pred may_sustain(unit: &CombatUnit)`,
`pred silenced(piece: Piece, ctx: CombatCtx)` (this unifies `context_silenced`, `ralnel_silenced_in_space`,
`crimson_abilities_lost`, `firmament_space_cannon_silenced` and the Naalu/Entropic Scars variants: **5 direct
slots to 1**). Non-faction rules (laws, technology, neutral player) register as `Source::Law/Technology` rules in
the same slot, which is what removes the `crate::laws::sustain_suppressed(...) && crate::factions::hooks_combat::may_sustain(...)`
pairs at `combat.rs` 192-197 and 2673-2696.

### 2.3 P3 contribution list (extra option / participant source)

*Call-outs*: `component_actions`, `mapped_component_actions`, `timing_abilities`, `production_destinations`,
`bastion_adjust_ship_spots`, `crimson_cards_offer_card_spot`, `deepwrought_spendable_oceans`,
`nekro_units_alastor_participants`, `ralnel_linkship_guns`, `crimson_deploy_planets`, `firmament_plot_options`,
`linked_systems`, `blocked_borders`, `extra_wormholes`.

```rust
pub struct Contrib<S, T, const N: usize> { pub source: Source, pub f: fn(Q, &S, &mut SmallList<T, N>) }
pub enum Order { Registration /* `concat` */, Sorted /* `concat_sorted`: sort + dedup */ }
pub struct Gather<S, T, const N: usize> { items: Vec<Contrib<S, T, N>>, order: Order }
impl<S, T: Ord + Copy, const N: usize> Gather<S, T, N> { pub fn collect(&self, q: Q, site: &S) -> SmallList<T, N>; }
```

Item types are the *window's* typed options (`Spot`, `Planet`, `Piece`, `AbilityDef`, `ComponentAction`), so the
contribution already is the decision's option source: `Ask::Place { spots }` is rendered by the existing
`render/production.rs` unchanged. "In-place edit" hooks (`bastion_adjust_ship_spots(..., spots: &mut Systems)`,
`bastion_units_cancel_in_groups(..., groups: &mut ..)`) are P3 with a `Remove`/`Replace` item and an explicit
`Phase::{Add, Edit}` in the `Order`; both exist today (the edit runs after the adds), so the seam keeps two
passes.

### 2.4 P4 replacement of a rule step

*Call-outs*: `borrowed_stats` (The Cavalry), `unit_form_override`, `production_unit_exchange` +
`production_unit_exchange_performed`, `pds_placement_alternative` + `_performed`, `skip_to_commit`,
`substitutes_primary`, `captured_unit_return_form`, `breakthroughs::free_capacity_granted`, `galaxy_for_mover`.

The string engine already splits a replacement into an *offer* and a *performed* hook (4 pairs). Represent it as
one value with two fields and a closed `Step` enum, so a window states which of its steps are replaceable:

```rust
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Step { UnitForm, ProductionExchange, PdsPlacement, SecondaryAsPrimary, SkipToCommit,
                BorrowedStats, CapturedReturn, FreeCapacity }
pub struct Replacement<S, O> {
    pub source: Source, pub step: Step,
    pub offer:   fn(Q, &S) -> Option<O>,                              // first Some wins, in registration order
    pub perform: fn(&mut Cx<'_>, &S, O) -> Result<Done, Ask>,         // may ask: returns the Ask to suspend on
}
```

`first`-wins is what the string dispatch does (`Hooks` kind `first`); the `Ask` return replaces the inline
`table.ask` in the performed hook (6.3).

### 2.5 P5 observer on an event

*Call-outs* (all `each` slots and most mutating direct slots): `bastion_note_ship_lost`, `bastion_take_ship_lost`,
`bastion_note_retreat`, `bastion_clear_retreated`, `nekro_units_alastor_decrement/clear`, `argent_clear_agent_production_destinations`,
`bastion_ship_placed`, `crimson_cards_put_on_card`, `cabal_note_replenished`, `control_gained`, `strategy_resolved`,
`space_combat_round_started`, `ground_combat_round_started/ended`, `deepwrought_note_coexistence_began`,
`nomad_agents_agent_readied`, `keleres_close_agent_window`.

These are not reactions (no window, no decision, no frequency): they are synchronous bookkeeping at a program point,
in `MODULES` order, *before* the shared code where the doc says so (`control_gained`, `strategy_resolved`,
`space_combat_round_started` "run BEFORE shared code"). Do **not** put them through the `Resolver`: that would
change window and decision sequences. Give them a typed channel of their own:

```rust
pub enum Obs {            // payload types are A5's `Payload` structs; one variant per program point
    ShipLost { system: System, piece: Piece },       ShipRetreated { system: System, to: System, owner: Seat, ships: ShipList },
    RetreatsCleared { system: System },              CombatRoundStarted { system: System, round: u32, who: SeatSet },
    GroundRoundStarted { .. },                       GroundRoundEnded { .. },
    ShipPlaced { who: Seat, producing: System, target: System, unit: Unit },
    ProductionWindowClosed { who: Seat, system: System },     // replaces argent x7 and keleres close_agent_window x4
    ControlGained { who: Seat, system: System, planet: Planet },     Replenished { who: Seat }, ...
}
pub type Observer = fn(&mut Cx<'_>, &Obs);
// HookTable: observers: [Vec<Observer>; Obs::COUNT], dispatched by `kernel.observe(cx, obs)` in registration order
```

Two rules keep this a pure refactor: (1) the observer fires at *exactly* the program point of the string call, with
the same ordering relative to the shared code (`Phase::Before | After` on the variant); (2) it never emits an
event (if a string hook does emit, e.g. `hooks_economy::emit(ctx, state, "UNITS_PRODUCED", ..)` at
`production.rs:3420`, that emit stays in the window and the observer is told the result).

### 2.6 P6 offer point (nested ask)

*Call-outs*: `keleres_offer_agent`, `winnu_exhaust_agent_for_production`, `leaders::offer_production_hero`,
`produced_hits` (asks), `afb_excess` (asks), `munitions_after_roll`, `keleres_units_pay_commit_toll`,
`mahact_units_offer_starlancer`, `deepwrought_research_open/settle`, `firmament_viper_*`, `promissory war_funding`.

```rust
pub enum Point { PaymentWindow, ProductionStart, ProductionEnd, AfterRollDice, AfterBarrage, CombatStart,
                 RoundEnd, BeforeCommit, CommitToll, AfterActivation, ... }          // closed; each window lists its points
pub struct Offer<S> {
    pub source: Source,
    pub ask:   fn(Q, &S) -> Option<Ask>,                        // None = nothing to offer to this seat now
    pub apply: fn(&mut Cx<'_>, &S, Opt) -> Next,                // Next::{Done, Open(Window), Continue(Ask), Chain}
}
// window side: Stage::Offering { point: Point, idx: u8 } -- iterate offers in registration order, one Ask at a time
```

This is also the shape of an A5 reaction window; the difference is only that a *point* is a named position inside a
window rather than an event, so no `Resolver` involvement and no frequency scopes. The `Ask` variants live in the
area that owns the point (`choice/production.rs`), which is how a faction's decision becomes "a family in the area
that owns the hook" (the sentence in the C1-C6 brief).

### 2.7 Pattern table (what changes where)

| pattern | A5 kinds | hooks | share of 190 slots (est.) | window code after |
|---|---|--:|--:|---|
| P1 modifier | `sum`, `fold`, `call -> i64` | about 36 | 19% | `stack.eval(q, &site, base, clamp)` |
| P2 predicate | `any`, `all`, `call -> bool` | about 70 | 37% | `pred.decide(q, &site, default)` |
| P3 contribution | `concat`, `concat_sorted` | about 29 | 15% | `gather.collect(q, &site)` |
| P4 replacement | `first`, `call -> Option`, `*_performed` | about 14 | 7% | `replace.first(q, &site)` + `perform` |
| P5 observer | `each`, mutating `call` | about 29 | 15% | `kernel.observe(cx, Obs::X)` |
| P6 offer | `try_*`, asking `call` | about 12 | 6% | `Stage::Offering` |

(Counts are by A5 combining kind, then by the call-out text; a slot can be re-typed into a different pattern when
the seam is written. They show where the volume is, not an exact partition.)

### 2.8 Non-faction providers

The same registration applies to cards, laws, relics, technologies, promissory notes, leaders and breakthroughs.
Two rules:

* **State-derived effects are queries, not hooks.** `laws::sustain_suppressed(state, base)` is already
  `query::laws` in A1; `technology::owns_technology(state, victim, "amd")` is A3. A provider entry is a three-line
  adapter: `Provider { source: Technology(Amd), layer: Technology, f: |q, site| Delta::..(q.s.tech_owned(site.victim, Amd)) }`.
  Where a thing is a plain "has X, then Y" it should be **data** in the generated table (X3: `UnitText` flags,
  `ModifierRow { source, applies_to: UnitMask, kind, amount }`), registered by the table loader, with code only for
  the rest. First candidates (all in `combat.rs`, from the grep of `"amd"`, `"nes"`, `"thalnos"`, `"crown_of_thalnos"`,
  `"jolnarcommander"`, `FIGHTER_PROTOTYPE`, `L1Z1X_FLAGSHIP`, `JOLNAR_FLAGSHIP`, `hacan_flagship`): about 12 rows.
* **Text-bearing flagships** (`flagship_has_text`, 7 references): replace by generated `Unit -> FlagshipTextSet`
  (bits `Jolnar, Letnev, L1z1x, Hacan, ...`) and a `P2`-style `lends_text(unit, text)` slot for the one lender
  (Nekro Valefar Assimilator Z). This deletes the B1 `FlagshipText` enum and its three neutral stubs.

### 2.9 Where the existing A5 table is kept

Nothing in A5 is thrown away. The `hook_group!` macro gains three keywords (`stack`, `observe`, `offer`) and the
`pred`/`gather` keywords are aliases for `any/all` and `concat/concat_sorted` with a typed site. Existing slot names
stay as *registration* names so the C shards' `register` functions do not move; the 91 `direct` slots are
re-expressed as registrations of the pattern slots (a mechanical table, 4.4), then deleted from `direct.rs` as the
window that used them is ported. `tools/engine_core/hook_inventory.py` (A5) is extended with the pattern column
so the table of 1.4 is regenerated, not hand-kept.

---

## 3. What can be done in the string engine first

"Extract-then-port": a **pure refactor** on the string side that moves each seam's composition out of the window
into one function. Gate: the existing suite, `bench`/`bench-v1..v5` 200/200 identical, the 20 golden games'
decision hashes identical (no `RULES_VERSION` change), and a `dual_query!` at the seam once the core side exists.
No behaviour change is possible by construction, because the new function is the old expression moved.

### 3.1 String-side steps (S-steps), each a separate agent, file-disjoint

| step | what | files touched | removes from the window | size |
|---|---|---|---|---|
| S1 combat-effects | new `combat_effects.rs`: `hit_shift(site)`, `dice(site)`, `may_sustain`, `sustain_granted`, `hit_immune`, `silenced`, `cannon_barred`, `afb_dice`; the six-term sums of `combat.rs` ~330-440, ~1240-1310, ~1640-1750, ~2120-2260, ~2380-2400, ~2570-2770, ~3611 | `combat.rs`, `invasion.rs` (same terms), new file | about 45 call-outs | M (1.5 d) |
| S2 observers | `factions::observe(Obs)` in `factions/mod.rs`, dispatching to bastion/nekro/argent/keleres in the **current call order**; windows call only `observe`. 18 call sites in combat + production + invasion | `factions/mod.rs`, `combat.rs`, `production.rs`, `invasion.rs`, 5 faction files (one fn each) | about 30 | M (1 d) |
| S3 production-effects | `production_effects.rs`: `spend_value`, `payment_sources` (what `spendable_planets`, `trade_good_worth`, `keleres::spendable_commodities`, `deepwrought::spendable_oceans` feed), `capacity`, `placement_spots` (hooks `bastion::adjust_ship_spots`, `crimson_cards::offer_card_spot`, `production_destinations`), `cost_after_discounts` | `production.rs`, `payment.rs`, `supply.rs` (reads), new file | about 35 | M (1.5 d) |
| S4 offer points | name the points: `OfferPoint::{PaymentWindow, ProductionStart, ProductionEnd}` and route `keleres::offer_agent`, `winnu::exhaust_agent_for_production`, `leaders::offer_production_hero` through `offers_at(point)`, in the order of the current code | `production.rs`, `factions/mod.rs` | about 9 | M (1 d) |
| S5 flagship text | generated `Unit -> FlagshipTextSet` (X3 generator) + `lends_text`; delete `flagship_has_text(.., "<id>")` calls | `factions/mod.rs`, `combat.rs`, `invasion.rs`, generator | 7 | S (0.5 d) |
| S6 roll sub-window | extract `roll_step` (`combat.rs::roll_round` ~230 lines, `invasion.rs` roll code, `dice.rs` staging, `reroll_staging` users) into `roll_window.rs`: stage -> offer rerolls -> commit | `combat.rs`, `invasion.rs`, `action_cards.rs`, `faction_abilities.rs`, `dice.rs` | shared by 4 users | L (2.5 d) |
| S7 inline asks to stages (combat, production) | each of the 7 + 11 inline asks becomes a named `Stage` in the window it sits in (table in 6.3), keeping the question text and order | `combat.rs`, `production.rs` | 18 asks | L (3 d each window) |
| S8 reactions sources | `arm()` already is a list; move the sources into one `sources()` returning `Vec<AbilityDef>` where `AbilityDef` is data (id, owner, event kind, relation, frequency, optional, `EffectId`) and the closures become `match EffectId` in `timing_effects.rs` | `reactions.rs`, `timing.rs`, `registry.rs`, factions' `timing_abilities` | the closure types (`AbilityEffect = Arc<dyn Fn>`) | L (4 d), A5 has done the core half |

S1-S5 are independent of each other (disjoint files except `combat.rs`/`invasion.rs` between S1 and S2: S1 first,
or two agents on separate functions). S6 and S7 are sequenced after S1/S2.

### 3.2 What is only possible in the core

* The typed `Site`/`Delta`/`Verdict`/`Obs` types and the `HookTable` kinds (they exist only on typed ids).
* Making `Q` the *only* argument of a provider (the string providers take `(&GameState, &ContentStore, SourceSet, &PlayerId, ..)`;
  `Q` is two pointers).
* Collapsing the 91 `direct` slots by pattern (their bodies read `faction_marks`, which the core types in F5).
* `Stage` as data stored in `Pending` and the pure `step` (section 6); the string `CombatWindow` can be tidied but
  stays a struct of `PlayerId`/`Vec<String>`.
* Removing `Arc<dyn Fn>` closures from abilities (A5 did it on the core side: `AbilityDef`).

### 3.3 Pure-refactor gate for S-steps

A string-side step merges to the live line (not only to `core-state-main`) after: engine tests, `bench` and
`bench-v1..v5` 200/200 identical, 20 golden games identical hashes, `RULES_VERSION` unchanged. Because it is a
pure refactor it can land before the core packages that depend on it, and it shrinks their diffs. The nightly
sweep covers it automatically (CLAUDE.md: the sweep tests whatever landed before 20:30 Berlin).

---

## 4. Ordering and parallel packages for wave C

### 4.1 Blocking order (from table 1.1 and 1.2)

Score = references from combat+invasion+production + registrations in combat/ground/economy groups (grep-derived):

| rank | faction group (files) | non-test lines | blocks | score | pure / mutating-or-asking |
|---|---|--:|---|--:|---|
| 1 | bastion (`bastion`, `bastion_units`) | 1 605 | B1, B2, B3 | 21 | extra_die, resource_bonus (P1); note_ship_lost, ship_placed (P5); cancel_bombardment (P4) |
| 2 | keleres (`keleres`, `keleres_units`) | 1 561 | B3, B2 | 19 | spendable_commodities (P1); offer_agent, pay_commit_toll (P6) |
| 3 | crimson (`crimson`, `crimson_cards`) | 1 867 | B1, B2, B3 | 19 | abilities_lost, can_deploy (P2); put_on_card (P5), offer_card_spot (P3) |
| 4 | firmament (3 files) | 2 399 | B1, B2 | 11 | assail_* (P1), space_cannon_silenced (P2), viper (P2/P6) |
| 5 | argent | 1 474 | B3, B1 | 11 | production_destinations (P3), afb_excess (P6), clear_destinations (P5) |
| 6 | nekro (`nekro`, `nekro_units`) | 1 953 | B1 | 7 | mech_roll_bonus (P1), alastor_* (P3/P5) |
| 7 | naaz | 1 453 | B1, B3, B2 | 7 | economy 5, combat 1, ground 1 |
| 8 | ralnel (2 files) | 1 538 | B1, B3 | 6 | silenced_in_space (P2), linkship_guns (P3) |
| 9 | borrowed (`borrowed_round_agents`, `borrowed_commanders*`) | 1 248 | B1, B2, A5 | 6 | extra_die_for (P1), abilities (P3) |
| 10 | deepwrought (3 files) | 1 966 | B2, B3, B6 | 6 | spendable_oceans (P3), research_team (P2/P6), begin_coexisting (P5) |
| 11 | mentak, winnu, naalu, ghost, titans, sardakk, yin | 7 983 | B1, B2, B3 (hook registrations only) | 1-4 each | all registered through `Hooks`, no direct reference |
| 12 | yssaril, cabal, arborec, muaat, saar, empyrean, nomad, obsidian, mahact | 11 036 | none of B1-B3 (movement/leaders/cards) | 0-2 | A2 movement, B7, B9 |

### 4.2 The key move: port faction tier Q *before* the window

A provider is a function of `(Q, &site)`. It does not need the window it will be called from, only the state slices
it reads (board, marks, players) and the site type. So the factions' pure hooks (138 of 190 slots) can be ported
and verified as soon as (a) the seam signatures are frozen and (b) the F5 mark types exist, by a **hook-level
dual**: for every `(position, seat, site)` of the golden games, call the string provider and the core provider and
compare (the way A2 compared `SeatReach`). No window machine is involved. When B1 later calls `stack.eval` the
table is already full and already equivalent.

Per faction the work then has three tiers, ported in this order:

* **Q** (pure): P1, P2, P3, P4-offer. Needs: `Q`, site types, F5 marks. Verified by the hook dual (5.1).
* **O** (observers): P5. Needs: `Obs` payload types and `Cx`/`fx` (A5). Verified with the window it is observed in.
* **W** (asks/offers/performed): P6, P4-perform. Needs the window's `Ask` family and stage. Ported *with* the
  window by the window's owner, with the faction contributing its function in the faction's file.

By the table in 1.3 about 73% of the hook slots are tier Q, and for the 13 blocking modules (21+19+19+11+11+7+7+6+6+6
= 113 references) the split by the call-out lists in 1.4 is about 60% Q, 25% O, 15% W (estimated; recount when S-steps
land and the lists are the seam functions).

### 4.3 Packages (replace C1-C6 in `ENGINE_CORE_STATE.md` 4.4; sizes are non-test lines, they sum to the 38 007 of `factions/*` minus `hooks_*`)

| pkg | name | factions (string files) | lines | tiers now | blocks | days (est.) |
|---|---|---|--:|---|---|--:|
| **C0** | seam-steward | `kernel/seam/**`, `kernel/hooks/**` pattern keywords, `ti4-engine-core-factions` crate skeleton and `register` convention, `dual_hook!` harness, extend `hook_inventory.py`; S5 generator part | new about 2.5 k | Q-signature freeze first (2 days), then maintenance | all | 4 |
| **CA** | combat-line | bastion(+units), crimson(+cards), nekro(+units), ralnel(+cards) | 6 963 | Q now, O after C0 `Obs`, W with B1/B2 | B1, B2, B3 | 4-5 |
| **CB** | economy-line | keleres(+units), argent, winnu, deepwrought(+cards, research), cabal | 8 071 | Q now, O/W with B3 | B3, B2, B6 | 5 |
| **CC** | ground-and-ship-line | firmament(+plots, flip), mahact(+units), borrowed(+commanders, round agents), ghost, mentak | 7 354 | Q now | B1, B2, B9 | 4 |
| **CD** | registry-line | naaz, titans, sardakk, naalu, yin, yssaril | 7 642 | Q now (small), rest after B-areas | B1-B3 minor, B6, B7 | 4 |
| **CE** | movement-and-leaders-line | empyrean(+units), saar, muaat, arborec, nomad(+agents), obsidian | 7 977 | Q now (A2 hooks), W with B7 | A2, B7 | 4 |

Ownership boundaries (file-disjoint, as in 4.6 of the base doc):

* Each package owns the *string* faction files it lists (read-only to others, since the S-steps touch one function
  each in a few of them: S2 edits `bastion`, `nekro_units`, `argent`, `keleres`, `crimson_cards`, one fn each and
  lands **before** CA/CB start), the core files `ti4-engine-core-factions/src/<faction>/{query,observe,offer}.rs`,
  and the `state/marks` record of the same factions (F5).
* A faction's three tier files are different files, so a tier-W change from the window owner does not collide with
  a tier-Q change from the shard: the window owner writes only `<faction>/offer.rs` for the offers of its points,
  after a request to the shard owner (same rule as "ask the area owner for a hook change", reversed).
* Nobody else edits `kernel/seam/**` or `kernel/hooks/**`; adding a slot is a request to C0 (same-day answer,
  the steward rule of `ENGINE_CORE_STATE.md` 4.3).
* CA..CE order: CA and CB start immediately after C0's signature freeze (they block B1/B3), CC next, CD and CE in
  the second slot. Within CA, bastion first (rank 1), then crimson, nekro, ralnel.
* Tests: the 44.8 k test lines in `factions/` (58% of the directory) are **not** ported one for one. They run
  against the string engine until D2; equivalence is carried by the hook dual (Q), by the window dual (O/W) and
  by "a seat of each faction in the 20 golden games" (the existing gate for C). A shard adds core unit tests only
  for edge cases the golden games do not reach (each `Delta::Set`, each `Replacement` perform).

### 4.4 Mapping the 91 direct slots (to be generated, not hand-kept)

Examples of the mechanical mapping C0 produces in its interface PR (full table generated by the extended
`hook_inventory.py`):

| direct slot (A5) | pattern slot | note |
|---|---|---|
| `bastion_extra_die`, `borrowed_round_agents_extra_die_for` | `dice` (P1) | `Layer::Faction` |
| `nekro_units_mech_roll_bonus`, `mahact_units_flagship_roll_bonus`, `firmament_plots_assail_space/against/ground` | `hit_shift` (P1) | commute |
| `crimson_abilities_lost`, `ralnel_silenced_in_space`, `firmament_space_cannon_silenced`, `context_silenced` | `silenced` (P2) | 4 slots to 1 |
| `bastion_note_ship_lost`, `bastion_take_ship_lost`, `bastion_note_retreat`, `bastion_clear_retreated` | `Obs::{ShipLost, ShipRetreated, RetreatsCleared}` (P5) | `take_ship_lost` returns a bool the window reads: make it `Obs::ShipLost` + a `query` on the bastion marks |
| `nekro_units_alastor_*` (4) | `alastor_participants` (P3) + `Obs::{ShipLost, CombatEnded}` (P5) | |
| `argent_clear_agent_production_destinations`, `keleres_close_agent_window` | `Obs::ProductionWindowClosed` (P5) | 11 call sites to 1 |
| `keleres_offer_agent`, `winnu_exhaust_agent_for_production` | `Offer` at `Point::PaymentWindow` / `ProductionStart` (P6) | |
| `bastion_adjust_ship_spots`, `crimson_cards_offer_card_spot`, `production_destinations` | `placement_spots` (P3, two passes) | |
| `keleres_with_agent_granted` | `Replacement` at `Step::FreeCapacity`-like probe, or a pure `Q`-transform | the only hook that wants a hypothetical state (a cheap `CoreState` clone) |

Counting: about 40 of the 91 direct slots disappear into 12 pattern slots; the remaining 51 are faction-specific
queries that feed a pattern anyway (they remain as the *provider functions* in the faction files, no longer slots).

---

## 5. Risks and how the dual harness catches them

### 5.1 Granularity of the dual: add a hook level

`dual.rs` today has one `Area` per package (`Combat`, `Production`, `Factions`, ...). That is too coarse for this
plan: a difference in `hit_shift` appears only as a wrong dice count three layers up. Add (C0, small):

* `dual_hook!(slot, site, string_expr, core_expr)` next to `dual_query!`: compares the *return value of one seam
  call* (value, `parts` for stacks, ordered list for gathers) and records the slot name in `DualReport`. Call
  sites are the S-step seam functions, so the string side needs no new code.
* A mask per slot group (`Area::Factions` split into `Hooks(Combat|Ground|Economy|Movement|Cards|Strategy|General)`)
  so a shard turns on only its slots.
* The offline cross-product runner (`core_equivalence` style): for every 5th position of the golden games, every
  seat, every unit/planet/system that is a legal `site` of the slot, call both. This is exactly how A2 verified 14.9 M
  `Observed` answers, so the infrastructure and the position cache exist.

### 5.2 Risk register

| # | risk | where | how it shows | catch |
|---|---|---|---|---|
| D1 | **Hook application order.** The dice stream is a function of `MODULES` order (A5 doc); `produced_hits` rolls dice per module; `fold` slots (`unit_dice`) are order-sensitive; Keleres is three modules (`MODULE_M`, `MODULE_X`, `MODULE_A`) | `HookTable::register`, `seal()` | rng positions differ after a combat | registration order is the `MODULES` array order, asserted by a test that lists the string `MODULES` aliases and compares with the core registration trace; `GameRng::positions()` is already compared by `dual!`; golden games contain Naaz/Mentak/Yin/Ghost seats |
| D2 | **Layer order differs from the string expression order** for non-commutative terms (a `.max(0)` clamp, a `Set`) | stacks (P1) | off-by-one dice or cost in a rare position | `Clamp` is applied last and listed in the site; the hook dual compares `value` and `parts`; every `Set` provider is listed in the review checklist with the string line it replaces |
| D3 | **Option order and fingerprints.** Contribution lists and offers decide the order of options, which is part of V1/V2 decision hashes ("the offered-option sequence is deliberately not sorted") | P3, P6 | `decision_replay` hash mismatch | `Order::{Registration, Sorted}` is explicit per gather and copies the string choice; the decision corpus (X2, 114 654 decisions) with lifters for production/combat; `dual!` compares V1/V2 hashes of each decision |
| D4 | **Inline ask becomes a stage and the question moves** relative to an emitted event or another question | S7, section 6 | event list or decision log differs in order | the `dual!` event and decision sequence comparison (3.3 items 3-4 of the base doc); each stage replaces exactly one ask site and the table in 6.3 lists the site |
| D5 | **Observer fires at a different program point** (before/after the shared code; before/after a state change it reads) | P5 | marks differ, usually one window later | `Phase::{Before, After}` on every `Obs`; state compare field by field after each window step; `faction_marks` are part of the compared JSON |
| D6 | **A hook is called for every seat**, not only for its faction (promissory notes, copied abilities, Nekro Z) and a port that checks `is_faction` first loses the borrowed case | all providers | a borrowed-ability seat differs | the hook dual iterates *all* seats at each position, and the golden games include the borrowed-commander and Nekro cases; the lint "provider must not read `faction` of the seat" in `check_layers.py` (reviewer aid, not a hard gate) |
| D7 | **Replacement offer/perform split.** The string `*_performed` hooks assume the offer was made in the same step | P4 | a performed hook runs on a different state | one `Replacement` value with both functions; the dual compares state after perform |
| D8 | **Reroll staging as a sub-window** changes who may reroll when | S6 | decision-log differences in combat and in action cards | extract with the string engine's existing tests (`dice.rs` has 33 reroll references) and the dual in S6 *before* any core port |
| D9 | **Data-driven rows** (the generated modifier tables) drift from text (card errata) | 2.8 | a card behaves differently only in one source set | generator `--check`; per-card golden positions; rows only for effects that are a literal `if has X then +n` today |
| D10 | **Dual run cost** grows with the hook dual (every slot x every position) | CI time | nightly overruns | sample 1 in 5 positions for the cross-product; full cross-product only in the shard's own gate |
| D11 | **Seam steward is a bottleneck** (one agent edits `kernel/seam` and `hooks`) | C0 | other packages idle waiting for a slot | a slot request is a one-line PR against a generated list; same-day answer rule; the first freeze (2 days) covers every slot found in 1.4 |
| D12 | **Two ports of the same ground**: B1 and B3 already contain `Faction`/`Env` seams | `rules/combat.rs`, `rules/production.rs` | merge conflict in 4.4 files | B1/B3 keep their branches; their trait methods become the `Seam` implementation in a mechanical step (the dual-checked bodies are reused); no rewrite of the query half |

### 5.3 Order-mutation audit (a cheap proof of the commutativity claims)

For the slots declared commutative (`sum`/`any`/`all`/`concat_sorted`, and `Add` deltas), add a test mode in which
the core registers providers in **reverse** order and (separately) in a seeded shuffle, then reruns the hook dual
over the golden positions. Any difference means the slot is not commutative and must be reclassified
(`fold`/`first`/ordered `concat`) before a shard builds on it. This turns the base doc's assumption "text order =
index order covers all of them" into a measured fact per slot, and it is independent of any window.

---

## 6. Windows as explicit state machines in core

### 6.1 Recommendation

**Yes**, for the windows that already are machines in the string engine (`CombatWindow`, `ProductionWindow`,
`InvasionWindow`, the diplomacy window, and the `Game` window fields), as:

* a `Machine` struct (copyable, no heap except `SmallList`s) stored in `state.pending` (so it is part of the cheap
  `CoreState` clone and of checkpoints);
* a `Stage` enum, **one variant per suspension point**, including the ones that are inline `table.ask` in the string
  code;
* `fn step(m: &mut Machine, cx: &mut Cx<'_>) -> Step` that runs until it must ask or is done (the string `settle`);
* `fn ask_of(m: &Machine, q: Q) -> Option<Choice>` (the string `pending_choice`, pure);
* `fn answer(m: &mut Machine, cx: &mut Cx<'_>, opt: Opt) -> Result<(), Illegal>` (the string `resolve`).

"Pure" means: the only inputs are `(Machine, CoreState, HookTable)` and the answer; no `Table`, no closures, no
global; mutation goes through `fx` primitives that also fire triggers and observers (so it is **not** an effect-list
design: returning `Vec<Effect>` would force every `fx` primitive to be re-expressed and breaks the "fire triggers
inside" rule of A5). The dual unit is one `answer` (+ the following `step`): state before/after, `Machine`
before/after rendered through a `Stage -> string` mapping the adapter owns, the pending `Choice`, events, rng.

Why: (1) it is the shape the string code already has (`Stage` 9 variants, `settle_stages` "one long match ... each
arm falls through to the next stage"), so the port is a typing exercise, not a redesign; (2) it removes the
`Table`/`Decider` dependency from the rule code, which is what lets the same machine run in the bot, the server and
wasm; (3) a stage list is reviewable against the LRR step list; (4) it gives the dual a natural granularity;
(5) **inline asks are the real cost** and a stage per ask is the only representation `Pending` can hold.

Why not "port the function bodies and keep inline asks via a callback `Decider`": the core stores `Pending` (what is
asked), not a suspended call stack; a callback decider would need replay-based suspension (the string engine's trick)
inside the core, defeating the cheap-clone checkpoints that motivate D2.

### 6.2 CombatWindow sketch

String side today (`combat.rs`): `Stage` {`Opening`, `Announcing`, `Rolling`, `RollingAfterBarrage`, `Sustaining`,
`Assigning`, `Retreating`, `Done`} with `HitPhase` {`Round`, `CombatStart`, `RoundEnd`} and 14 flags/collections on
the struct (`pending_retreats`, `hit_reaction_offered`, `damaged_before_round`, `roll_replays`, `fled`,
`fleet_at_start`, `end_announced`, `resuming_round`, `galaxy` ...). Functions: `open_round` (4974), `roll_round`
(5233), `settle_stages` (5504-5757), `pending_choice` (5759), `resolve` (6041), plus `BeforeCombat` (6300, space
cannon offense and the pre-combat window).

Core:

```rust
// ti4-engine-core/src/rules/combat/{mod,machine,roll,hits,retreat,barrage}.rs   (B1)
#[derive(Clone, Copy)] pub struct SeatSet(u8);
pub struct Combat {                       // lives in state.pending as WindowKind::Combat's data
    pub system: System, pub attacker: Seat, pub defender: Seat,
    pub stage: Stage, pub hit_phase: HitPhase,
    pub flags: CombatFlags,               // resuming_round | end_announced
    pub retreating: SeatSet,              // pending_retreats
    pub reacted: SmallList<(u8 /*round*/, Seat), 4>,   // hit_reaction_offered
    pub fled: SmallList<Fled, 2>,         // who, ships arrived
    pub replays: u8,                      // roll_replays
}
pub enum Stage {
    Opening   { round: u8 },
    Announcing{ round: u8, asking: Seat, announced: SeatSet },
    Rolling   { round: u8 },
    Reroll    { round: u8, side: Seat, die: u8 },                    // NEW: choose_reroll_dice (inline ask, combat.rs:597)
    RollingAfterBarrage { round: u8 },
    Offering  { point: Point, idx: u8, then: Resume },               // NEW: munitions, Heart of Ixth (1027), Wrath of Kenara (1563)
    Sustaining{ round: u8, queue: HitQueue },
    SustainCommander { round: u8, queue: HitQueue },                 // NEW: ask_sustain_commander (2627)
    Assigning { round: u8, queue: HitQueue },
    Retreating{ round: u8, leaving: SeatSet },
    Destructions { staged: u8 },                                     // NEW: announce_staged_destructions (3924)
    Done(Outcome),
}
pub struct HitQueue(SmallList<PendingHits, 4>);
pub struct PendingHits { pub player: Seat, pub hits: u8, pub producer: Seat, pub flags: HitFlags /* non_fighters_only | producer_assigns */ }

pub enum Step { Continue, Ask, Done }
pub fn step  (m: &mut Combat, cx: &mut Cx<'_>) -> Step;                  // = settle_stages, loops while Continue
pub fn ask_of(m: &Combat, q: Q) -> Option<Choice>;                       // = pending_choice (pure)
pub fn answer(m: &mut Combat, cx: &mut Cx<'_>, a: Opt) -> Result<(), Illegal>;   // = resolve
```

`step` composition, so the machine knows no faction:

```rust
Stage::Opening { round } => {
    cx.observe(Obs::CombatRoundStarted { system: m.system, round, who: m.sides() });           // P5: replaces
                                                                                               // space_combat_round_started + bastion/nekro/nomad bookkeeping
    cx.emit(EventKind::CombatRoundStarted, ..)?;                                               // reaction window (A5 Resolver)
    m.stage = if let Some(p) = offers_pending(cx, Point::CombatStart, m) { Stage::Offering{point: Point::CombatStart, idx: 0, then: Resume::Barrage{round}} } else { Stage::Rolling { round } };
}
Stage::Rolling { round } => {
    for side in m.sides() {
        let groups = fleet_groups(cx.q(), m.system, side);                                    // existing B1 query
        for g in groups { let n = cx.hooks.dice.eval(cx.q(), &g.site(), g.printed_dice, Clamp::Min0).value;
                          let t = cx.hooks.hit_shift.eval(cx.q(), &g.site(), g.printed_target, Clamp::Roll).value; roll(cx, g, n, t) }
    }
    m.stage = Stage::Reroll { round, side: first_roller, die: 0 };                             // or straight to Sustaining when no reroll source
}
```

Mapping of string items to core (all line numbers are `combat.rs` at `201f6714`):

| string | core |
|---|---|
| inline asks 597, 1027, 1563, 2627, 2973, 3924, 5340 | stages `Reroll`, `Offering` (x2), `SustainCommander`, `Sustaining` (the existing one), `Destructions`, `Rolling`'s reroll offer |
| `produced_hits` (4737), `afb_excess` (1890) | `Offering` at `Point::{CombatStart, RoundEnd, AfterBarrage}` with `Offer::apply` returning `Next` |
| bastion/nekro/nomad bookkeeping (252, 276, 3369, 4098, 4151, 4163, 4956-4957, 5370) | `Obs::{ShipLost, ShipRetreated, RetreatsCleared, CombatEnded}` |
| 6-term roll-shift expression (340-435, 1276-1306, 1734-1744, 2380-2389) | `hit_shift.eval`, `dice.eval` |
| 22 predicates | `may_sustain`, `hit_immune`, `silenced`, `cannon_barred`, `ship_movement_barred` as `Pred`s |
| `neutral_units` branches (597, 2961, 3866, 4445, 4622, 5650) | a `Seat::NEUTRAL` provider set: neutral answers are a P4 `Replacement` at `Step::Assign`/`Step::Retreat` (neutral never asks) |
| `Stage::Done` + `announce_end` | `Obs::CombatEnded` + `EventKind::SpaceCombatEnded` |

Sub-machines that must be separate files and separate owners' concerns: `roll.rs` (shared with B2 and B6, S6),
`hits.rs` (queue and assignment), `retreat.rs`, `barrage.rs` (anti-fighter barrage, space cannon offence as in
`BeforeCombat`). B2 (ground combat) calls `roll::stage`, not B1 internals.

### 6.3 Inline asks per window (the real work list)

| window | inline asks (non-test) | stage they become |
|---|---:|---|
| combat | 7 (`choose_reroll_dice` 524, `heart_ixth` 941, `wrath_of_kenara_buy` 1512, `ask_sustain_commander` 2596, `offer_sustain` 2800, `announce_staged_destructions` 3774, `roll_round` 5233) | `Reroll`, `Offering`, `Offering`, `SustainCommander`, `Sustaining`, `Destructions`, (in `Rolling`) |
| production | 11 (`pay_with_observation_credit` 905, `sling_relay` 1057 x2, `integrated_economy` 1159, `produce_one_from` 1338 x2, `resolve_timed` 3588, `produce_by_ability_capped` 3698, `produce_unit_by_ability` 3754, `agency_supply_network` 3794) | `Stage::PayCredit`, `Sling{..}`, `Integrated`, `Produce{..}`, `Offering{Point::PaymentWindow}`, `ByAbility{..}` |
| invasion | 10 | to be listed by B2's interface PR (same method) |
| factions | 75 | in the shard's `offer.rs`, as `Offer.ask`/`apply` pairs (tier W) |
| action cards | 17 | B6 |

### 6.4 What does not become a machine

* Leaf helpers that ask once and return (`Offer.apply` bodies): they are the `Next::Continue(Ask)` of a
  `Replacement.perform` or `Offer.apply`, not machines.
* The reaction kernel: A5's `Resolver` already is the machine (window stack, player order, frequency scopes);
  windows only `emit`.
* `Game::step_*` functions: they belong to D1 (driver) and become `Pending` machines there.

---

## 7. Order of work, in short

```text
day 0     S1 combat-effects, S2 observers, S3 production-effects, S5 flagship text  (string, pure refactors, 4 agents, disjoint)
day 0-2   C0: seam signatures (Q, Stack, Pred, Gather, Replacement, Obs, Offer), dual_hook!, hook_inventory.py pattern column, faction crate skeleton
day 2     C0 freeze  ->  CA, CB start tier Q (hook dual only; no window needed); B1, B3 swap Faction/Env for the Seam
day 2-6   S4 offer points, S6 roll sub-window (string)
day 4-12  CA, CB tier Q done; CC, CD, CE tier Q; B1/B3/B2 consume stack/pred/gather (they no longer wait for factions)
day 8-20  S7 inline asks -> stages (string, combat then production then invasion); B1/B3/B2 window machines; tiers O then W per shard
```

Critical path for B1: S1 (1.5 d) -> C0 freeze (2 d) -> CA tier Q (4 d, parallel with the B1 machine) -> B1 machine
(Stage port + S7 for combat, about 6 d) -> CA tiers O/W (2 d). That is about 11-13 days to a combat that runs on
the table, against "B1 waits for the factions it calls", and B3 and B2 run in parallel on the same footing.
(Estimated; the base doc's B1 estimate is 9 days for the whole area.)

## 8. Decisions for the user

1. Accept re-cutting wave C into CA-CE with tiers Q/O/W (replaces C1-C6)? Recommended: yes.
2. Land the S-steps on the live line as pure refactors (they ride the nightly) rather than only on `core-state-main`?
   Recommended: yes, S1/S2/S3/S5 first; they are small and shrink every later diff.
3. Port data-driven modifier rows for the 12 "has X then +n" effects in combat (2.8), or keep all of them as code
   providers for now? Recommended: code first, rows after the Q-tier duals are green (a row is a refactor of a
   provider, with the hook dual as its test).

## Appendix: how the counts were produced

* Non-test cut: everything from `#[cfg(test)]` + `mod tests` to the end of file.
* Table 1.1: for each file in a column, for each `factions/<name>.rs`, count `(?<![A-Za-z_0-9])<name>::` on lines
  that do not start with `//`; `hooks_*` files and `factions::<fn>(` dispatchers are counted separately; faction
  hook registrations: parse each `hooks: Hooks { .. }` block, track the sub-struct (`combat: CombatHooks {`) and
  count `<slot>: Some(`.
* Content-id literals: all `"[a-z0-9_]{4,}"` literals in the cut file that equal an `id`/`alias` of
  `crates/ti4-content/content/{abilities,action_cards,agendas,breakthroughs,leaders,promissory_notes,relics,technologies,units,explores,secret_objectives,public_objectives,factions}.json`.
* Hook kinds and pure/mutating split: the `hook_group!` rows of `crates/ti4-engine-core/src/kernel/hooks/*.rs` on
  branch `core-a5-kernel` (`git show core-a5-kernel:...`), `cx: &mut Cx` in the arguments = mutating.
* Inline asks: `.ask_seeing(`, `table.ask(`, `.ask(&` in non-test code of `ti4-engine/src/**`.
* Pattern classification (1.4, 2.7): by reading the grep output of `combat.rs` and `production.rs` and the doc line of
  each A5 slot; marked "about".
