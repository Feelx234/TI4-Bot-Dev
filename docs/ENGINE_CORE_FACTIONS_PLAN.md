# Wave C: the faction packages (`ti4-engine-core-factions`)

Preparation for `docs/ENGINE_CORE_STATE.md` wave C (4.1 layer L6, 4.4, 4.7, 6.2, 7), **re-cut by
`docs/ENGINE_DISENTANGLE.md` (accepted)**: five packages CA to CE (instead of the six shards C1 to C6) by seam
and blocking order, each split into tiers Q, O and W, plus the steward C0. Branch `core-c0-seams` (from
`core-state-main` 201f6714 plus A5 39b20cb8, A1 e1de8bef, C-prep 6664bba1, A2 98a7039e, the disentangle design
e8d4042c). The inventory (section 1) and the dual-harness text (section 4) come from `core-c-prep` and are still
valid; the package table (2), the scaffold (3) and the briefs (5) replace C1 to C6. Counts are a regex over the
sources, within a few percent.

**Read first:** `docs/ENGINE_DISENTANGLE.md` (the six seams, the tiers, the risks) and
`crates/ti4-engine-core/src/kernel/seam/mod.rs` (the real types).

## 1. Inventory of `crates/ti4-engine/src/factions/` (88 383 lines in 49 files; about 45% is inline `#[cfg(test)]`)

Three kinds of file:

* **Faction modules**: `<f>.rs` declares `pub const MODULE: FactionModule` (id lists + `Hooks` table), the
  rest are helper files of the same faction (`_units`, `_cards`, `_agents`, `_leaders`, `_research`, `_flip`,
  `_plots`). `MODULES` in `mod.rs` has 27 entries (Keleres three times) and its order fixes the dice stream.
* **Hook plumbing, not faction code** (`mod.rs` 1 217, `hooks_cards/combat/economy/ground/movement/strategy.rs`
  4 407): the `Hooks` struct, the six group structs and the dispatch functions the shared engine calls. In the
  core these are `kernel/hooks/*.rs` (done by A5: 99 string slots + about 91 `direct` slots). Not ported by C;
  deleted by D2. A shard edits only the body of the dispatch function of a slot it ports (section 4).
* **Cross-faction helpers**: `borrowed_commanders.rs`, `borrowed_commanders_b.rs`, `borrowed_round_agents.rs`
  (2 622 lines; Dune-style "borrowed" leaders used by several factions; only `direct` slots).

Faction-specific code **outside** `factions/` that stays with its area owner and is not part of C:
`faction_abilities.rs` (2 184 lines, B7), `faction_techs.rs` (918, A3), `leaders.rs` (4 276, B7),
`breakthroughs.rs` (494, A3), `starting_tech.rs`, `fixtures.rs` (D1), `core_sync/keys/marks.rs` (typed marks,
F5/X1), `faction_effects_tests/`, `technology_effects_tests/` (tests of the string engine; never edited).
Shared modules that call into a faction (`production.rs` 49 sites, `combat.rs` 38, `invasion.rs` 56,
`game.rs` 24, `objectives.rs` 13, ...) do it through the `direct` slots, which A5 already defined.

Columns: lines = all files of the faction; top = slots of `Hooks` itself (general: timing_abilities,
component_actions, perform_component, leaders, commander_unlocked, unit_dice, ...); group = slots of the six
group structs (movement, combat, ground, economy, cards, strategy); direct = `direct` slots named
`<faction>_*`; "needs" = packages whose queries, asks or windows the ported code compiles against (from the
slot's `String:` callers and the general-slot map: leaders/component/commander -> B7, timing_abilities -> A5,
unit_dice -> B1, skip_to_commit -> B2, vote_bonus -> B9).

| Faction | Files | Lines | top | group slots filled | direct | Needs | Package |
|---|---|---:|---:|---|---:|---|---|
| arborec | arborec | 3 006 | 6 | cannot_produce | 0 | A5 B3 B7 | CE |
| argent | argent | 2 735 | 5 | votes_first, afb_excess, production_destinations, free_cargo | 1 | A2 A5 B1 B3 B7 B9 | CB |
| bastion | bastion, bastion_units | 3 326 | 2 | - | 13 | A1 A5 B1 B2 B3 B7 | CA |
| cabal | cabal | 4 606 | 6 | gravity_rifts, move_bonus, rift_crucible, rift_roll_exempt | 1 | A2 A5 B5 B7 | CB |
| crimson | crimson, crimson_cards | 3 828 | 6 | may_sustain x2, linked_systems, move_bonus, severed_systems, usable_wormhole_kinds | 9 | A1 A2 A5 B1 B2 B3 B7 D1 | CA |
| deepwrought | deepwrought, _cards, _research | 3 578 | 6 | own_unit_passage | 9 | A2 A5 B2 B3 B6 B7 B8 | CB |
| empyrean | empyrean, empyrean_units | 3 256 | 6 | blocked_borders, ignores_nebulae, move_bonus, passable_owners | 1 | A1 A2 A5 B7 | CE |
| firmament | firmament, _flip, _plots | 4 146 | 6 | move_bonus, unladen_pass | 11 | A1 A2 A4 A5 B1 B2 B7 | CC |
| ghost | ghost | 2 779 | 6 | production_cost_reduction, extra_wormholes, linked_systems, move_bonus | 0 | A2 A5 B3 B7 | CC |
| keleres (3 modules) | keleres, keleres_units | 4 007 | 5 | - | 11 | A1 A5 B2 B3 B6 B7 B8 D1 | CB |
| mahact | mahact, mahact_units | 2 726 | 6 | secondary_waived, secondary_waivers | 8 | A1 A5 B1 B2 B5 B7 B8 D1 | CC |
| mentak | mentak | 2 531 | 4 | may_sustain x2, trade_good_worth, unit_form_override | 0 | A5 B1 B2 B3 B4 B7 | CC |
| muaat | muaat | 3 222 | 6 | may_enter_supernova (+ may_pass_through_supernova, extra_production) | 0 | A2 A5 B7 | CE |
| naalu | naalu | 2 222 | 1 | may_view_hand, fighter_fleet_weight_halves, temporary_ground_force, secondary_waived/ers, strategy_phase_ended | 1 | A1 A5 B1 B2 B4 B8 | CD |
| naaz | naaz | 3 650 | 7 | cannot_produce, explored, ground_combat_round_started | 0 | A5 B1 B2 B3 B5 B7 | CD |
| nekro | nekro, nekro_units | 3 771 | 6 | - | 6 | A5 B1 B2 B7 D1 | CA |
| nomad | nomad, nomad_agents | 3 499 | 4 | transaction_reach, borrowed_stats, grants_sustain | 3 | A5 B1 B2 B4 B7 | CE |
| obsidian | obsidian | 3 137 | 5 | secondary_waived/ers | 1 | A5 B7 B8 D1 | CE |
| ralnel | ralnel, ralnel_cards | 2 805 | 5 | free_cargo | 5 | A2 A5 B1 B2 B3 B6 B7 | CA |
| saar | saar | 2 470 | 6 | production_destinations, move_bonus, scores_without_home | 0 | A2 A4 A5 B3 B7 | CE |
| sardakk | sardakk | 2 831 | 3 | commit_candidates, ground_rolls_extra_hits | 0 | A5 B2 B7 | CD |
| titans | titans, titans_leaders | 3 356 | 6 | action_card_draw_bonus, pds_placement_alternative(+_performed) | 0 | A5 B3 B6 B7 | CD |
| winnu | winnu | 2 888 | 8 | custodians_free, linked_systems, move_bonus, secondary_waived/ers | 2 | A2 A5 B1 B2 B3 B7 B8 | CB |
| yin | yin | 3 539 | 5 | - | 0 | A5 B7 | CD |
| yssaril | yssaril | 2 226 | 6 | transaction_reach, action_card_draw_bonus/requested, action_cards_drawn, action_card_limit, action_cards_forbidden | 0 | A5 B4 B6 B7 | CD |
| (borrowed) | borrowed_commanders, _b, borrowed_round_agents | 2 622 | 0 | - | 6 | A4 A5 B1 B2 | CC |

Already in the core (B1, branch `core-b1-combat`, not yet merged): `rules/combat.rs::register_naaz_nomad` (Naaz
`ability_hit_immune`, Nomad `grants_sustain`), called from `ti4-engine/src/core_combat.rs`. CD and CE move that
code into the factions crate and replaces the call by `register_all` in one commit (never both: a slot would
be registered twice).

### What "blocks" what

* Every `direct` slot and every group slot **already exists** in the `HookTable` (A5), so the *types* are not
  the blocker. What blocks a shard is (a) the **state** it reads (A1 board/resources, A2 reach, A3 tech, A4
  objectives, `state/marks` of its factions: F5) and (b) for hooks that ask or open windows (`timing_abilities`,
  `perform_component`, `use_leader*`, `produced_hits`, `afb_excess`, `action_cards_drawn`, `pds_placement_*`) the
  **Ask/Opt family and `Next` windows of the owning area** (B1 combat, B2 invasion, B3 production, B6 action
  cards, B7 leaders, B8 strategy). Pure value hooks (bool/number/list) need only A1-A3 and port at once.
* Conversely a package's faction gate (a seat of each faction in the dual run, gate 3 of ENGINE_CORE_STATE 5)
  needs the package that holds those factions: B1 <- CA, CC, CD (bastion/crimson/nekro dice and sustain), B2 <- CA, CD,
  B3 <- CB, CA, B4 <- CB/CD (transaction_reach, trade_good_worth), B6 <- CB, CA, B7 <- CA-CE (all leaders),
  B8 <- CC, CD, B9 <- CB (votes_first), D1 <- all five (the `game.rs` direct slots: keleres, crimson, nekro, mahact).
  Until a shard lands, its packages gate with the faction's string module still answering (the "ported set",
  section 4).


Faction behaviour that is **not** a hook in the faction file (a law, a relic, a technology, a promissory note, a leader)
registers on the same seams from the area that owns it (`kernel/seam/providers.rs`, `data.rs`); it is not part of
CA to CE.

## 2. The five packages (disjoint file ownership; sizes are non-test lines of the string files)

| pkg | name | factions (string files) | lines | tiers now | blocks | start |
|---|---|---|--:|---|---|--:|
| **C0** | seam steward | `kernel/seam/**`, `kernel/hooks/**` pattern keywords, `ti4-engine-core-factions/src/{lib,common}.rs`, `dual_hook!`, the order audit | done in this branch | maintenance | all | frozen |
| **CA** | combat line | bastion(+units), crimson(+cards), nekro(+units), ralnel(+cards) | 6 963 | Q now, O after `Obs`, W with B1/B2 | B1, B2, B3 | 1st |
| **CB** | economy line | keleres x3 (+units), argent, winnu, deepwrought(+cards, research), cabal | 8 071 | Q now, O/W with B3 | B3, B2, B6 | 1st (parallel to CA) |
| **CC** | ground-and-ship line | firmament(+plots, flip), mahact(+units), borrowed (commanders, round agents), ghost, mentak | 7 354 | Q now | B1, B2, B9 | 2nd |
| **CD** | registry line | naaz, titans, sardakk, naalu, yin, yssaril | 7 642 | Q now (small), rest after the B areas | B1-B3 minor, B6, B7 | 3rd |
| **CE** | movement-and-leaders line | empyrean(+units), saar, muaat, arborec, nomad(+agents), obsidian | 7 977 | Q now (A2 hooks are in), W with B7 | A2, B7 | 3rd |

Order: CA and CB start first (they block B1 and B3), CC next, CD and CE in the second slot. Within CA: bastion, crimson,
nekro, ralnel. Within CB: keleres, argent, winnu, deepwrought, cabal. The A2 movement slots of 14 factions are already
ported (`<faction>/movement.rs`), so every package starts with a non-empty crate.

**Tiers.** Q = pure hooks (`query.rs`: stacks, predicates, gathers, replacement offers; verified by the hook dual, no
window needed); O = observers (`observe.rs`: `Obs` bookkeeping; verified with the window that observes); W = offers,
asks and `*_performed` (`offer.rs`; ported with the window by the window owner, who writes only that file after a request
to the package owner). About 73% of the hook slots are Q, and for the 13 blocking modules about 60% Q, 25% O, 15% W.

**Ownership.** A package edits only its directory (`ca_combat/` ... `ce_movement/`), the render files
`render/factions/<its factions>.rs`, `core_factions/<package>.rs`, and the `state/marks` records of its factions (F5);
the string files of its factions only for the dual wrapper lines (no behaviour edits; the string-side refactors S1 to S8
of the disentangle doc are another agent's branch and land first, they touch one function each in a few of these files).
Nobody else edits `kernel/seam/**`, `kernel/hooks/**`, `lib.rs`, `common.rs` or another package's directory: a missing
slot, site field or helper is a request to C0 (same-day answer).

## 3. The scaffold (this branch)

```text
crates/ti4-engine-core-factions/
  Cargo.toml                    depends on ti4-engine-core only
  src/lib.rs                    Id (28 modules, the string MODULES order), Modules, PACKAGES, register_all, movement_table, missing()
  src/common.rs                 typed readers every faction shares (is_faction, flagship_has_text, marks, scope)
  src/ca_combat/mod.rs          MODULES + MOVEMENT tables; one `pub mod` per faction
  src/ca_combat/<faction>/      mod.rs (register = movement, query, observe, offer, in that order)
                                movement.rs (A2, ported)   query.rs (Q)   observe.rs (O)   offer.rs (W)
  src/cb_economy/ cc_ground/ cd_registry/ ce_movement/   the same
```

* **Registration pattern**: a slot function has the exact signature of the core slot and returns the neutral value for any
  seat that is not this faction's; `register(t: &mut HookTable)` pushes it (`t.movement.may_enter_supernova.push(f)`) or, for a
  seam, `t.seam.hit_shift.push_add(Source::Faction(..), Layer::Faction, f)`. Keleres has `register_m`, `register_x`, `register_a`.
* **Order**: `register_all` iterates `Id::ALL` (the string `MODULES` order) and, for each id, the package that provides it. The
  package order is irrelevant, so five branches merge in any order and the dice stream is the string engine's. `Id::Borrowed`
  (last) is the one non-faction entry.
* **Tests**: `no_module_has_two_providers`; `every_module_is_provided` (now active: all 28 have a registrar, empty tiers
  included); `the_movement_table_is_the_movement_part_of_the_full_table` (fails the day a package fills a non-movement slot:
  update the assertion to count only the movement slots then). Hygiene: `check_layers.py` treats the crate as L6, `lint_core.sh`
  scans it, `verify_package.sh` tests and clippies it.
* `ti4-engine` has the optional dependency (feature `typed-core`) and the feature `string-factions`. `core_adapter` builds the
  movement hooks with `movement_table()`; nothing else uses the crate yet.

### The six seams in one table (`kernel/seam`, package C0)

| pattern | type, slot of `HookTable.seam` | window call | factions and other providers register with |
|---|---|---|---|
| P1 modifier | `Stack<S>`: `hit_shift`, `dice`, `planet_spend`, `extra_production`, `production_discount` | `.eval(q, &site, base, Clamp)` -> `Evaluated { value, parts }` | `push_add(Source, Layer, f)`, `push_set(.., f)` (a `Set` says why), `push_row(&ModifierRow)` |
| P2 predicate | `Pred<S>`: `may_sustain` (forbid wins), `hit_immune`, `silenced`, `cannon_barred`, `cannot_produce`, `commit_blocked` (allow wins) | `.decide(q, &site, default)`, `.why(..)` | `push(Source, f)` returning `Verdict::{Abstain, Allow, Forbid}` |
| P3 contribution | `Gather<S, T, N>`: `alastor_participants`, `production_destinations`, `deploy_planets` | `.collect(q, &site)` | `push(Source, Pass::{Add, Edit}, f)` (adds first, then edits; `Order::Sorted` sorts and dedups) |
| P4 replacement | `Replacements<S, O>`: `borrowed_stats`, `unit_exchange` | `.first(q, &site)`, then `perform` | `push(Replacement { source, step, offer, perform })` (`perform` returns `Next`, may ask) |
| P5 observer | `Observers` by `ObsKind` and `Phase::{Before, After}` | `cx.observe(&Obs::X { .. }, Phase)` | `push(ObsKind, Phase, f)` |
| P6 offer | `Offers` by `Point` | stage `Offering { point, idx }`: `next_ask`, `apply` | `push(Point, Offer { source, ask, apply })` |

Order: a stack is ordered by `(Layer, registration index)` with `Layer = Printed, Technology, Relic, Law, Faction, Leader, Card`;
a clamp is the last argument of `eval`; first-wins and gathers keep registration order. Rule of thumb for a provider: if the
string expression it replaces is a sum, it is `push_add` (and commutes); if it is a fold, it is `push_set` with the legacy
reason until it is rewritten as an add.

**Existing seams on the new ones** (`seam/adapters.rs`, tables in its doc): every legacy slot has a *bridge* provider
(`bridge_legacy`), so a window can switch to the seam before any faction is ported, and a ported faction registers natively
while the bridge keeps answering for the rest. B1's `Faction` trait (13 methods) and B3's `Env` trait (10 methods) map to
seam calls and are deleted when the window switches (section 7). The 12 combat "has X then +n" effects are data rows
(`seam/data.rs::COMBAT_ROWS`: Unrelenting, Fragile, Fighter Prototype, Prophecy of Ixth, Morale Boost, the nebula defender,
Antimass Deflectors, Non-Euclidean Shielding, Plasma Scoring, Metali Void Shielding, Metali Void Armaments, Lightrail
Ordnance); not rows because they need more than "this seat has X": Mahact's Arvicon Rex (opponent token not in the fleet
pool), Nekro's Mordred (X/Y token), Naaz flagship+mech and Supercharge (marks), Bastion's Galvanize (per-unit flag), the
Thalnos/Hacan/Jol-Nar text effects: those stay provider functions of the faction files.

## 4. Dual-harness addition (`Area::Factions`)

`Area::Factions` ("factions", AreaMask bit 14) already exists in `dual.rs`; the off switch is the feature `string-factions`.
The first package to pass its gate adds the line
`if cfg!(all(feature = "typed-core", not(feature = "string-factions"))) { mask = mask.with(Area::Factions, Backend::Core) }`.

**Hook level (new, C0).** `dual_hook!(HookGroup::Seam, "hit_shift", string_expr, core_expr)` compares one seam call (an
`Evaluated` compares `value` and `parts`, a gather its ordered list), counts the slot in `DualReport::hooks`, and labels a
difference `hook::<slot>` in `Area::Factions`. `HookGroup` is the file of `kernel/hooks/` (or `Seam`); `TI4_HOOKS=combat,economy`
(or `dual::set_hook_groups`) turns on only a package's groups, the rest stay on the string answer. Call sites are the S-step
seam functions on the string side (`combat_effects.rs`, `production_effects.rs`, ...), so the string side needs no new code.
The offline cross-product (every 5th golden position, every seat, every legal site) is the runner a package writes in its
own `core_factions/<package>.rs`, as A2 did for `SeatReach`.

**Order audit (new, C0).** `HookTable::permuted(Permute)` registers the commutative slots (a pure `sum`/`any`/`all`/
`concat_sorted`, and the seams that declare it) in reverse and in two seeded shuffles; `HookTable::audit_commutative(probe)`
returns the permutations under which `probe` changed; `dual::audit_hook_order(group, slot, &table, probe)` reports it as
`order::<slot>`. A package runs it on its slots over the same golden positions: any difference means the slot is not
commutative and must be reclassified (`fold`, `first`, ordered `concat`) before anyone builds on it. Observers, `fold`,
`first`, `concat`, `each`, `try_*` and anything taking `cx` are never permuted.

**The ported set.** Dual and Core must not compare a core answer that only knows the ported modules with a string answer
from all 28. `core_factions::PORTED: &[Id]` (each package maintains its entries in its own file) lists the ported modules;
the string side in `Dual` is `dispatch_over(MODULES.filter(ported))`, and `Core` is allowed only for a slot all of whose
providers are ported (folds interleave modules in `MODULES` order and cannot be split). A slot that both a bridge and a
native provider fill is counted once: native registration removes the faction from the bridge's answer by the ported set.

**Report**: labels `factions::<slot>` / `hook::<slot>` / `order::<slot>`; the nightly runs `factions=dual` for the packages
merged since the last clean night. Deletion commit per package = remove the string `factions/<f>*.rs` files and their
`MODULES` entries (after one clean night); D2 removes the rest.

## 5. Agent briefs (ready to paste; replace `<k>` by a-e, `<name>` by the package name)

Common rules (all five):

> You work in the repository `/root/TI4-Bot-Dev` in your own worktree `.claude/worktrees/core-c<k>` on branch
> `core-c<k>-<name>`, started from `core-c0-seams` (merge it, or `core-state-main` once it contains it) plus the
> packages named under "Needs". Read first: `CLAUDE.md`, `docs/ENGINE_DISENTANGLE.md`, `docs/ENGINE_CORE_STATE.md` (4.1,
> 4.3, 5, 6.5, 7), `docs/ENGINE_CORE_FACTIONS_PLAN.md` (all), the module docs of
> `crates/ti4-engine-core/src/kernel/seam/mod.rs` and `kernel/hooks/mod.rs`, and one finished example:
> `ti4-engine-core-factions/src/ce_movement/muaat/movement.rs`.
>
> **Pure refactor.** Behaviour, `RULES_VERSION`, serde output, fixtures, fingerprints and the dice stream stay
> identical. Port the string function as it is, including its quirks; a bug you find is reported (file, line, case) and fixed
> on the string side first through the normal baseline process, never "fixed" in the port. Never edit a test of the
> string engine (add tests only). Core code: no `String`, `&str`, `serde`, `BTree*`, `format!`, `unsafe`, `.to_string()` in
> non-test code; imports only from `ti4_engine_core`. You edit only your package directory
> (`src/c<k>_*/`), `render/factions/<your factions>.rs`, `core_factions/c<k>.rs` and the marks of your factions; not
> `lib.rs`, `common.rs`, `kernel/seam/**`, `kernel/hooks/**`, another package, or any area file: a missing slot, site field,
> `Obs` variant or `Point` is a request to C0, an `Ask`/`Opt` variant a request to the area owner. One commit per faction
> (message `C<K>: <faction> ...`).
>
> **Per faction, in this order:** (1) a table of its `Hooks` entries and `direct` slots from the string module (slot,
> string function, tier Q/O/W, target: legacy slot or seam) in the faction's `mod.rs` doc; (2) **tier Q** in `query.rs`: each
> pure hook as a provider on its seam (`hit_shift`/`dice` stacks, `silenced`/`may_sustain` predicates, gathers, replacement
> offers) or on the legacy slot where no seam fits; the provider returns the neutral answer for every seat that is not the
> faction's; the string function's body is moved, not rewritten; (3) the **hook dual** `dual_hook!` at the S-step seam
> function of each ported slot and the entry in `core_factions::PORTED`; run the order audit on the slots you declared
> commutative; (4) **tier O** in `observe.rs`: the faction's `Obs` observers, fired at exactly the string program point; (5)
> **tier W** only for the points whose window owner has merged the `Ask` family (otherwise leave `offer.rs` empty and
> list the points); (6) the faction's marks on F5's typed keys; (7) tests: one per provider (core side) mirroring the
> string module's tests, plus string-vs-core over a seat of that faction.
>
> **Gate** (run once at the end and after rebasing onto `core-state-main`; in between only `cargo check` and targeted
> tests): `tools/engine_core/verify_package.sh factions OUT BASE_SELFPLAY` with a private `CARGO_TARGET_DIR`,
> `TI4_AREAS=factions=dual` (and `TI4_HOOKS=` your groups) over the engine suite, the 20 golden games and the template
> openings (a seat of each of your factions), dual golden 0 differences, 200/200 benches with `factions=core`, golden
> hashes 20/20, `check_layers.py core`, `lint_core.sh` and clippy clean. **Limits:** CARGO_BUILD_JOBS=3, one cargo run at a
> time, private target dir deleted when done, no push, no merge into the live branch; merge into `core-state-main` only
> after the gate, rebased. Report: per faction the slots ported by tier, the dual and audit counts, the gate summary, open
> questions.

* **CA `core-ca-combat`** (bastion, crimson, nekro, ralnel; `src/ca_combat/`; needs A1, A2, A5, C0; blocks B1, B2, B3).
  *Q*: Bastion `extra_die` (-> `dice`), `resource_bonus` (-> `planet_spend`), `unit_roll_modifier` (-> `hit_shift`),
  `cancel_in_groups`, `adjust_ship_spots` (-> a `Pass::Edit` contribution); Crimson `abilities_lost` and `can_deploy` (->
  `silenced`, a predicate), `deploy_planets` (gather), `may_sustain` x2, the `severed_systems`/`linked_systems` movement hooks
  already in `movement.rs`; Nekro `mech_roll_bonus` (-> `hit_shift`), `alastor_participants`/`alastor_covers` (gather and
  predicate), the Valefar copies (`flagship_has_text` stays in `common.rs`, C0's); Ralnel `silenced_in_space` (-> `silenced`),
  `linkship_guns` (gather). *O*: Bastion `note_ship_lost`/`take_ship_lost`/`note_retreat`/`clear_retreated`/`ship_placed` (->
  `Obs::ShipLost`, `ShipRetreated`, `RetreatsCleared`, `ShipPlaced`; `take_ship_lost` returns a bool the window reads: make it
  `Obs::ShipLost` plus a query on the Bastion marks), Nekro `alastor_decrement`/`alastor_clear`, Crimson `put_on_card`. *W*:
  Crimson breach and deploy offers, `cancel_bombardment_hits` (P4), Nekro propagation. Order inside: bastion, crimson,
  nekro, ralnel. Highest cross-area risk: commit and gate faction by faction.
* **CB `core-cb-economy`** (keleres x3, argent, winnu, deepwrought, cabal; `src/cb_economy/`; needs A5, A2, B7 interface, C0;
  blocks B3, B2, B6). *Q*: Keleres `spendable_commodities` (-> `planet_spend`/payment), `with_agent_granted` (needs a cheap
  `CoreState` clone), Argent `production_destinations` (gather), Winnu `agent_ready`, `custodians_free`, `secondary_waived/ers`,
  Deepwrought `spendable_oceans` (gather), `research_team_open` (predicate), Cabal `gravity_rifts`, `rift_roll_exempt`,
  `rift_crucible`, `move_bonus` (already in `movement.rs`). *O*: `ProductionWindowClosed` (replaces Argent's 7 and Keleres'
  4 call sites), Deepwrought `begin_coexisting`, Cabal `note_replenished` (`Obs::Replenished`). *W*: Keleres
  `offer_agent`/`pay_commit_toll` (P6 at `Point::PaymentWindow`/`CommitToll`), Winnu `exhaust_agent_for_production`,
  Argent `afb_excess`, Deepwrought research settle. Keleres is three registrars (`register_m/x/a`): keep the string
  order of the three modules.
* **CC `core-cc-ground`** (firmament, mahact, borrowed, ghost, mentak; `src/cc_ground/`; needs A4, A5, B5 interface, C0; blocks
  B1, B2, B9). *Q*: Firmament `assail_space/against/ground` (-> `hit_shift`), `space_cannon_silenced` (-> `silenced`),
  `viper_open`/`plot_options`; Mahact `flagship_roll_bonus` (-> `hit_shift`), `ship_movement_barred` (predicate), `secondary_waived`;
  Borrowed `extra_die_for` (-> `dice`), `unit_roll_modifier`, the abilities (gather); Ghost `production_cost_reduction`
  (-> `production_discount`), `extra_wormholes`; Mentak `may_sustain` x2 (-> `may_sustain`), `trade_good_worth`,
  `unit_form_override` (P4). *O*: Mahact `control_gained`, Firmament plot scoring bookkeeping. *W*: Mahact `offer_starlancer`,
  Firmament viper offers. Borrowed helpers register as `Id::Borrowed`.
* **CD `core-cd-registry`** (naaz, titans, sardakk, naalu, yin, yssaril; `src/cd_registry/`; needs A5, B4, B6, B7 interfaces, C0;
  blocks B1-B3 minor, B6, B7). *Q* (small): Naaz `cannot_produce`, `unit_roll_modifier`, `unit_dice` (+1 die with flagship and
  mech: stays a provider, not a row), `explored`; Sardakk `commit_candidates`, `ground_rolls_extra_hits`; Naalu
  `fighter_fleet_weight_halves`, `temporary_ground_force` (P4), `may_view_hand`; Titans `pds_placement_alternative` (P4);
  Yssaril `transaction_reach`, the action-card draw hooks. *O*: Naaz `ground_combat_round_started` (-> `Obs::GroundRoundStarted`),
  Naalu `strategy_phase_ended`. *W*: Titans `_performed`, Yssaril `action_cards_drawn`, the leaders and timing abilities of
  all six (the draw asks are B6's families). **Moves `register_naaz_nomad` out of `rules/combat.rs`** together with CE's Nomad
  part: one commit, after B1 is merged, replacing the call in `core_combat.rs` by `register_all`.
* **CE `core-ce-movement`** (empyrean, saar, muaat, arborec, nomad, obsidian; `src/ce_movement/`; needs A2, A5, B7 interface, C0;
  blocks A2, B7). *Q*: the A2 movement hooks are in; add Muaat `extra_production` (-> `extra_production`), the supernova
  production hooks, Saar `production_destinations` (gather) and `scores_without_home`, Arborec `cannot_produce` (predicate),
  Nomad `grants_sustain` (-> `may_sustain` Allow side is not a Forbid: keep the legacy slot until the predicate has an
  `Allow` for `may_sustain`; ask C0), `borrowed_stats` (P4), `transaction_reach`, Obsidian `secondary_waived/ers`. *O*: Nomad
  `agent_readied`. *W*: the leaders and component actions (B7), Arborec's 8 inline asks (stages in `offer.rs`), Nomad
  `watches_roll_step`. Gate adds the movement template check (`core_equivalence templates --routes`).

## 6. Open points for the user

1. The ported-set rule (4) means a slot reaches `Core` only when all its providers are ported, so the default flips at the last
   package even though Dual runs from the first. Alternative: flip per slot (more bookkeeping). Recommendation: as written.
2. `kernel_sync` is not gated by `typed-core` (`cargo check -p ti4-engine --no-default-features` fails): `timing.rs`,
   `trigger.rs` and `render/{reaction,generic}.rs` all call into it. A proper fix gates those call sites or splits the
   core-free half of `kernel_sync`; it is more than a one-line gate, so it was left (A5's file).
3. `tools/engine_core/hook_inventory.py` (named in the disentangle doc) does not exist in this tree; the pattern column and the
   generated 91-direct-slot table (4.4 of the disentangle doc) are therefore not produced yet.

## 7. What B1 and B3 change when they resume (ready to paste)

**B1 `core-b1-combat` (a3983a7d), resume message:**

> You resume package B1 on branch `core-b1-combat`. First merge `core-c0-seams` (the seam steward's branch; it contains A5,
> A1, A2 and the factions crate). Read `docs/ENGINE_CORE_FACTIONS_PLAN.md` sections 3, 5 and 7 and the docs of
> `kernel/seam/mod.rs` and `seam/adapters.rs`. Then: (1) delete the `Faction` trait, `KernelFaction`, `FlagshipText` and the
> three neutral stubs in `rules/combat.rs`, and `Strs` in `core_combat.rs`; the rules take `Q` (`Q::new(s, k)`) and read the
> seams of `k.hooks.seam` instead: the six terms of `effective_from` (`combat_modifier`, `unit_roll_modifier`,
> `mahact_flagship_bonus`, `nekro_mech_bonus`, `assail_space`, `law_fighter_bonus`) become one `hit_shift.eval(q, &RollSite, 0, Clamp::None)`: its `value` is the total shift to the result (the string subtracts it from the printed threshold: `threshold = printed - value`, then the existing clamps); `parts` feeds the UI facts; `unit_dice` + `bastion_extra_die` + `round_agent_extra_die` become `dice.eval(q, &site, printed,
> Clamp::None)`; `may_sustain_modules` is `may_sustain.decide(q, &unit, true)`; `borrowed_stats` is
> `borrowed_stats.first(q, &SeatSystemSite)` (then the piece lookup you already have); `alastor_participants` is
> `alastor_participants.collect`. (2) Kernel setup registers `seam::providers::register_core` (rows for Unrelenting,
> Fragile, Fighter Prototype, Ixth, Morale Boost, the nebula defender, Antimass Deflectors, Plasma Scoring and the relic rows)
> and, until CA/CC/CD land, `seam::adapters::bridge_legacy(&mut kernel.hooks.seam)`; the row amounts replace your hand-written
> `law_fighter_bonus`/Fighter Prototype/Morale/nebula code, so delete those terms from the query half and keep their tests
> pointing at the rows. `non_euclidean_shielding` and relic sustain: `seam::data::cancel_on_sustain` and `grants_sustain` over
> `k.hooks.seam.rows`. (3) Replace `flagship_has_text(.., FlagshipText)` by the generated `Unit -> FlagshipTextSet` once the
> string step S5 has landed; until then keep a local function but do not export it. (4) `register_naaz_nomad` stays where it
> is until CD/CE take it; do not move it. (5) The dual: wrap each seam call with `dual_hook!(HookGroup::Seam, "<slot>",
> string_expr, core_expr)` (string side = the S1 `combat_effects` function once it exists, else the existing expression
> moved into a local fn), keep the existing `dual_typed!` calls for the machine. Run `audit_hook_order` for `hit_shift`,
> `dice` and `may_sustain` over the golden positions. (6) Stages: each of the 7 inline asks becomes a `Stage` as in 6.2 of
> `docs/ENGINE_DISENTANGLE.md` (`Reroll`, `Offering`, `SustainCommander`, `Destructions`); observers
> (`Obs::ShipLost`, `ShipRetreated`, `RetreatsCleared`, `CombatRoundStarted`, `CombatEnded`) are fired with `cx.observe`
> at exactly the string program points. Pure refactor, dual golden 0 differences, no CPU A/B, no wasm size.

**B3 `core-b3-production` (d24c750e), resume message:**

> You resume package B3 on branch `core-b3-production`. First merge `core-c0-seams`. Read `docs/ENGINE_CORE_FACTIONS_PLAN.md`
> sections 3, 5 and 7, and the docs of `kernel/seam/{mod,stack,pred,gather,adapters}.rs`. Then: (1) delete `trait Env`
> and `ProdView`'s `impl Env`; `rules::production::Cx` carries `Q` (state + kernel) instead of `env: &dyn Env`, and every
> `cx.env.<m>(..)` becomes a seam call: `planet_spend_value` + `resource_bonus` -> `planet_spend.eval(q, &SpendSite,
> printed_value, Clamp::None)` (laws are in it via `providers::register_core`: delete your `laws::planet_value_bonus` term
> from `value_now` so it is not counted twice); `extra_production`/`extra_production_planet` -> `extra_production.eval(q,
> &ProdSite, ..)`; `production_cost_reduction` -> `production_discount.eval`; `cannot_produce`/`has_cannot_produce` ->
> `cannot_produce.decide(q, &ProduceSite, false)` and `.is_empty()`; `silenced_in_space` + `abilities_lost` +
> `abilities_usable` -> one `silenced.decide(q, &SilenceSite { ctx: SilenceCtx::Production .. }, false)` (Entropic Scars is
> a technology-side `Allow` rule to be registered by A3's owner or by you in `providers.rs`: ask C0); `is_space_station` is
> a generated planet property, not a seam. (2) Kernel setup registers `register_core` and `bridge_legacy` as for B1.
> (3) Placement: `production_destinations.collect(q, &DestSite)`; `crimson_cards_offer_card_spot` and
> `bastion_adjust_ship_spots` are `Pass::Add`/`Pass::Edit` contributions of CA (request a `spots` gather from C0 when you
> reach it; today's `production_destinations` carries only `(System, Option<Planet>)`). (4) The asking half you had not ported
> (Keleres/Winnu agents, `leaders::offer_production_hero`, the 11 inline asks) becomes `Stage::Offering { point:
> Point::PaymentWindow | ProductionStart | ProductionEnd }` over `offers.next_ask`/`apply` plus the named stages of 6.3 of the
> disentangle doc; the string side is S4 (another agent). (5) Observers: `ProductionWindowClosed` (replaces the 7 Argent and 4
> Keleres call sites), `ShipPlaced`, `Replenished`. (6) `dual_hook!` at each seam call and `audit_hook_order` for the five
> stacks and `cannot_produce`; the existing `dual_typed!` wrappers stay for the queries you ported. Pure refactor, dual golden
> 0 differences, no CPU A/B, no wasm size.
