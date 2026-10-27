# Wave C: the faction shards (`ti4-engine-core-factions`)

Preparation for `docs/ENGINE_CORE_STATE.md` wave C (4.1 layer L6, 4.4 row C1-C6, 4.7, 6.2, 7). Written on
branch `core-c-prep` (from `core-state-main` 201f6714 plus `core-a5-kernel` 39b20cb8 for the `HookTable`). All
numbers were measured on that tree; the hook and caller counts come from a regex over the sources and are
within a few percent (a slot filled through a non-literal `..` is missed). **`docs/ENGINE_DISENTANGLE.md`
(branch `core-disentangle-design`) did not exist yet when this was written**; when it appears, re-read its
plan against section 1 (module ownership), section 5 (briefs) and the "ported set" idea in 6, and fix this file.

## 1. Inventory of `crates/ti4-engine/src/factions/` (88 383 lines in 49 files; about 45% is inline `#[cfg(test)]`)

Three kinds of file:

* **Faction modules**: `<f>.rs` declares `pub const MODULE: FactionModule` (id lists + `Hooks` table), the
  rest are helper files of the same faction (`_units`, `_cards`, `_agents`, `_leaders`, `_research`, `_flip`,
  `_plots`). `MODULES` in `mod.rs` has 27 entries (Keleres three times) and its order fixes the dice stream.
* **Hook plumbing, not faction code** (`mod.rs` 1 217, `hooks_cards/combat/economy/ground/movement/strategy.rs`
  4 407): the `Hooks` struct, the six group structs and the dispatch functions the shared engine calls. In the
  core these are `kernel/hooks/*.rs` (done by A5: 99 string slots + about 91 `direct` slots). Not ported by C;
  deleted by D2. A shard edits only the body of the dispatch function of a slot it ports (section 6).
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

| Faction | Files | Lines | top | group slots filled | direct | Needs | Shard |
|---|---|---:|---:|---|---:|---|---|
| arborec | arborec | 3 006 | 6 | cannot_produce | 0 | A5 B3 B7 | C2 |
| argent | argent | 2 735 | 5 | votes_first, afb_excess, production_destinations, free_cargo | 1 | A2 A5 B1 B3 B7 B9 | C2 |
| bastion | bastion, bastion_units | 3 326 | 2 | - | 13 | A1 A5 B1 B2 B3 B7 | C5 |
| cabal | cabal | 4 606 | 6 | gravity_rifts, move_bonus, rift_crucible, rift_roll_exempt | 1 | A2 A5 B5 B7 | C1 |
| crimson | crimson, crimson_cards | 3 828 | 6 | may_sustain x2, linked_systems, move_bonus, severed_systems, usable_wormhole_kinds | 9 | A1 A2 A5 B1 B2 B3 B7 D1 | C5 |
| deepwrought | deepwrought, _cards, _research | 3 578 | 6 | own_unit_passage | 9 | A2 A5 B2 B3 B6 B7 B8 | C5 |
| empyrean | empyrean, empyrean_units | 3 256 | 6 | blocked_borders, ignores_nebulae, move_bonus, passable_owners | 1 | A1 A2 A5 B7 | C1 |
| firmament | firmament, _flip, _plots | 4 146 | 6 | move_bonus, unladen_pass | 11 | A1 A2 A4 A5 B1 B2 B7 | C6 |
| ghost | ghost | 2 779 | 6 | production_cost_reduction, extra_wormholes, linked_systems, move_bonus | 0 | A2 A5 B3 B7 | C1 |
| keleres (3 modules) | keleres, keleres_units | 4 007 | 5 | - | 11 | A1 A5 B2 B3 B6 B7 B8 D1 | C6 |
| mahact | mahact, mahact_units | 2 726 | 6 | secondary_waived, secondary_waivers | 8 | A1 A5 B1 B2 B5 B7 B8 D1 | C4 |
| mentak | mentak | 2 531 | 4 | may_sustain x2, trade_good_worth, unit_form_override | 0 | A5 B1 B2 B3 B4 B7 | C3 |
| muaat | muaat | 3 222 | 6 | may_enter_supernova (+ may_pass_through_supernova, extra_production) | 0 | A2 A5 B7 | C1 |
| naalu | naalu | 2 222 | 1 | may_view_hand, fighter_fleet_weight_halves, temporary_ground_force, secondary_waived/ers, strategy_phase_ended | 1 | A1 A5 B1 B2 B4 B8 | C4 |
| naaz | naaz | 3 650 | 7 | cannot_produce, explored, ground_combat_round_started | 0 | A5 B1 B2 B3 B5 B7 | C3 |
| nekro | nekro, nekro_units | 3 771 | 6 | - | 6 | A5 B1 B2 B7 D1 | C5 |
| nomad | nomad, nomad_agents | 3 499 | 4 | transaction_reach, borrowed_stats, grants_sustain | 3 | A5 B1 B2 B4 B7 | C3 |
| obsidian | obsidian | 3 137 | 5 | secondary_waived/ers | 1 | A5 B7 B8 D1 | C4 |
| ralnel | ralnel, ralnel_cards | 2 805 | 5 | free_cargo | 5 | A2 A5 B1 B2 B3 B6 B7 | C3 |
| saar | saar | 2 470 | 6 | production_destinations, move_bonus, scores_without_home | 0 | A2 A4 A5 B3 B7 | C2 |
| sardakk | sardakk | 2 831 | 3 | commit_candidates, ground_rolls_extra_hits | 0 | A5 B2 B7 | C3 |
| titans | titans, titans_leaders | 3 356 | 6 | action_card_draw_bonus, pds_placement_alternative(+_performed) | 0 | A5 B3 B6 B7 | C2 |
| winnu | winnu | 2 888 | 8 | custodians_free, linked_systems, move_bonus, secondary_waived/ers | 2 | A2 A5 B1 B2 B3 B7 B8 | C4 |
| yin | yin | 3 539 | 5 | - | 0 | A5 B7 | C4 |
| yssaril | yssaril | 2 226 | 6 | transaction_reach, action_card_draw_bonus/requested, action_cards_drawn, action_card_limit, action_cards_forbidden | 0 | A5 B4 B6 B7 | C2 |
| (borrowed) | borrowed_commanders, _b, borrowed_round_agents | 2 622 | 0 | - | 6 | A4 A5 B1 B2 | C6 |

Already in the core (B1, branch `core-b1-combat`): `rules/combat.rs::register_naaz_nomad` (Naaz
`ability_hit_immune`, Nomad `grants_sustain`), called from `ti4-engine/src/core_combat.rs`. C3 moves that
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
  needs the shard that holds those factions: B1 <- C3, C5 (bastion/crimson/nekro dice and sustain), B2 <- C3, C5,
  B3 <- C2, C5, B4 <- C2/C3 (transaction_reach, trade_good_worth), B6 <- C2, C5, B7 <- C1-C6 (all leaders),
  B8 <- C4, C6, B9 <- C2 (votes_first), D1 <- all six (the `game.rs` direct slots: keleres, crimson, nekro, mahact).
  Until a shard lands, its packages gate with the faction's string module still answering (the "ported set",
  section 6).

## 2. Proposed split into 6 shards (disjoint ownership, about 13.8 k lines each, total 82 762)

Order = by the latest package each shard needs; within a shard, port value hooks first, ask hooks when the
area's Ask/Opt exists, so a shard may start before its last blocker merges.

| Shard | Branch | Factions (lines) | Starts when | Directory |
|---|---|---|---|---|
| C1 movement/board | `core-c1-factions-movement` | cabal 4 606, empyrean 3 256, ghost 2 779, muaat 3 222 = 13 863 | A2, A5, B7 interface (B3 for ghost's cost reduction, B5 for cabal's replenish note) | `ti4-engine-core-factions/src/c1_movement/` |
| C2 economy/draws | `core-c2-factions-economy` | arborec 3 006, argent 2 735, saar 2 470, titans 3 356, yssaril 2 226 = 13 793 | B3, B6 (draw asks), B4 (transaction_reach), A4 (saar), B9 interface (argent votes_first) | `.../c2_economy/` |
| C3 combat/ground | `core-c3-factions-combat` | mentak 2 531, nomad 3 499, naaz 3 650, ralnel 2 805, sardakk 2 831 = 15 316 | B1, B2, B3, B4 | `.../c3_combat/` |
| C4 strategy/agenda | `core-c4-factions-strategy` | mahact 2 726, naalu 2 222, obsidian 3 137, winnu 2 888, yin 3 539 = 14 512 | B8, B1/B2 for naalu/winnu/mahact | `.../c4_strategy/` |
| C5 cross-area | `core-c5-factions-cross` | bastion 3 326, crimson 3 828, deepwrought 3 578, nekro 3 771 = 14 503 | B1, B2, B3, B6, B8 (13 + 9 + 9 + 6 direct slots) | `.../c5_cross/` |
| C6 late/leftovers | `core-c6-factions-late` | firmament 4 146, keleres 4 007, borrowed_* 2 622 = 10 775 | A4, B5, B6, B8, D1 pieces (Keleres touches 8 areas; firmament plots score objectives) | `.../c6_late/` |

C6 is the smallest on purpose (riskiest: three Keleres modules, plots, helpers shared by other factions) and
merges last; it also removes `#[ignore]` from `every_module_is_provided`. File ownership: a shard owns its
directory, `crates/ti4-engine/src/render/factions/<its files>` and `core_factions/<shard>.rs` (section 6), the
string files of its factions only for the dual wrapper lines (no behaviour edits), and the `state/marks`
records of its factions through F5's typed keys. No shard edits `lib.rs`, another shard's directory, a `kernel/`
file, any area file (`rules/`, `choice/`, `query/`): a missing hook point or Ask variant is requested from the
A5 steward / the area owner, not added.

## 3. The scaffold (this branch)

```text
crates/ti4-engine-core-factions/
  Cargo.toml                 depends on ti4-engine-core only
  src/lib.rs                 Id (28 modules in MODULES order), Modules, SHARDS, register_all, missing()
  src/c1_movement/{mod,muaat}.rs     C1; Muaat is the compile-checked example (2 movement slots)
  src/c2_economy/mod.rs ... c6_late/mod.rs      empty `MODULES`
```

* **Registration pattern** (same as A5's doc and B1's `register_naaz_nomad`): a faction file defines slot functions with the
  exact signature of the core slot, each returning the neutral value for any seat that is not this faction's,
  and `pub fn register(t: &mut HookTable)` pushing them (`t.movement.may_enter_supernova.push(f)`). The shard's
  `mod.rs` lists `(Id::Muaat, muaat::register)` in `MODULES`.
* **Order**: `register_all` iterates `Id::ALL` (the string `MODULES` order) and, for each id, the shard that
  provides it. Shard order is irrelevant, so six branches merge in any order and the dice stream is the string
  engine's. `Id::Borrowed` (last) is the one non-faction entry (direct slots of the borrowed helpers).
* **Hygiene**: `check_layers.py` already treats the crate as L6 (imports only `ti4_engine_core`; checked, 0
  violations); `lint_core.sh` already scans it (0 hits). `verify_package.sh` now also tests and clippies
  `-p ti4-engine-core-factions`. The test `no_module_has_two_providers` guards the shard split; the ignored
  `every_module_is_provided` is the wave's done-test.
* `ti4-engine` has the optional dependency (feature `typed-core` brings it) and the feature `string-factions`.

## 4. Dual-harness addition (`Area::Factions`)

`Area::Factions` ("factions", AreaMask bit 14) **already exists** in `dual.rs` (`Area::ALL`, `name`, `parse`,
`TI4_AREAS=factions=dual`); nothing to add to the machinery. Added now: the off-switch feature
`string-factions` (precedent `string-movement`) and a note in `build_default`. Not added now (needs the first
port): the line `if cfg!(all(feature = "typed-core", not(feature = "string-factions"))) { mask = mask.with(Area::Factions, Backend::Core) }`, which the **first shard to pass its gate** adds. Until then the area
is `String` by default, `Dual`/`Core` only through `TI4_AREAS`, tests and bins.

How a call site is checked (what the shards implement, one pattern for all):

1. **Adapter** `ti4-engine/src/core_factions/mod.rs` (first shard to start creates it, later shards add
   only `core_factions/<shard>.rs`): `FactionView` = a `Kernel` with `register_all` + the lent `CoreView`, built like
   `CombatView::of` in `core_combat.rs` (B1) and cached per game position.
2. **Call sites** are the dispatch functions in `factions/hooks_*.rs` and the direct-call sites. A ported
   slot's dispatcher becomes `dual_query!(Area::Factions, "factions::<slot>", string_dispatch(..), view.<slot>(..))`
   (read-only slots), `dual_typed!` where the typed side can decline (an unmapped position), `dual!` for slots that
   mutate (`control_gained`, `perform_component`, `use_leader`; they take `&mut GameState`, as `dual!` does).
3. **The ported set.** Dual and Core must not compare a core answer that only knows the ported modules with a
   string answer from all 27. `core_factions::PORTED: &[Id]` (maintained by each shard in its own file) is the
   list; the string side in `Dual` is `dispatch_over(MODULES.filter(ported))`, and `Core` backend is allowed only
   for a slot all of whose providers are ported (otherwise the slot stays on String: folds interleave modules in
   `MODULES` order and cannot be split). Slot-level readiness table: the inventory above (slots per faction).
4. **Report**: area name `factions`, labels `factions::<slot>`; the nightly runs `factions=dual` for the shards
   merged since the last clean night (6.2 of ENGINE_CORE_STATE). Deletion commit per shard = remove the string
   `factions/<f>*.rs` files of its factions and their `MODULES` entries (after one clean night), D2 removes the rest.

## 5. Agent briefs (ready to paste; replace `<k>`)

Common rules (all six):

> You work in the repository `/root/TI4-Bot-Dev` in your own worktree `.claude/worktrees/core-c<k>` on branch
> `core-c<k>-factions-<name>`, started from `core-state-main` (check that A5 and the packages named under "Starts when"
> in `docs/ENGINE_CORE_FACTIONS_PLAN.md` are merged; otherwise start with the value hooks only). Read first:
> `CLAUDE.md`, `docs/ENGINE_CORE_STATE.md` (4.1, 4.3, 5, 6.5, 7 "C1-C6"), `docs/ENGINE_CORE_FACTIONS_PLAN.md`, the module docs of
> `crates/ti4-engine-core/src/kernel/hooks/mod.rs` and `direct.rs`, `ti4-engine-core-factions/src/c1_movement/muaat.rs`
> (the pattern) and, for hooks that ask, the area's `choice/<area>.rs` and `render/<area>.rs`.
>
> **Pure refactor.** Behaviour, `RULES_VERSION`, serde output, fixtures, fingerprints and the dice stream
> stay identical. Port the string function as it is, including its quirks; a bug you find is reported (file,
> line, case) and fixed on the string side first through the normal baseline process, never "fixed" in the port.
> Never edit a test of the string engine (add tests only). No new hook points, no `Ask`/`Opt` variants, no
> state fields outside the marks of your factions: ask the A5 steward / the area owner. Core code: no
> `String`, `&str`, `serde`, `BTree*`, `format!`, `unsafe` in non-test code; imports only from `ti4_engine_core`.
> Do not edit `lib.rs` or another shard's directory. One commit per faction (message `C<k>: <faction> ...`).
>
> **Per faction, in this order:** (1) a table of its `Hooks` entries and `direct` slots from the string
> module (slot, string function, core slot, signature); (2) value hooks (bool/number/list) in
> `<dir>/<faction>.rs`, `register`, entry in `MODULES`; (3) the dual wrapper at each string dispatcher
> or call site of the ported slots (section 4 of the plan) and its entry in `core_factions::PORTED`;
> (4) ask hooks and window openers when the area's Ask/Opt exist, with the renderer in
> `render/factions/<faction>.rs`; (5) the faction's marks record on F5's typed keys; (6) tests: one per hook
> (core side) mirroring the string module's tests, plus the string-vs-core equivalence over a seat of that
> faction.
>
> **Gate** (run once at the end of the shard and after rebasing onto `core-state-main`; in between only
> `cargo check`/targeted tests): `tools/engine_core/verify_package.sh factions OUT BASE_SELFPLAY` with
> `CARGO_TARGET_DIR` private, `TI4_AREAS=factions=dual` over the engine suite, the 20 golden games and the
> template openings (a seat of each of your factions; Thunder's Edge content via unit tests), 200/200 benches
> with `factions=core`, golden hashes 20/20, CPU A/B not worse than +1.5%, wasm size reported, `check_layers.py core`
> and `lint_core.sh` clean, clippy clean in the new crate. **Limits:** CARGO_BUILD_JOBS=3, one cargo run at a time,
> private target dir deleted when done, no push, no merge into the live branch; merge into `core-state-main` only after
> the gate, rebased. Report: per faction the slots ported, the dual counts, the gate summary, open questions.

* **C1 `core-c1-factions-movement`** (cabal, empyrean, ghost, muaat; `src/c1_movement/`). Scope: Cabal gravity rifts
  and rift hooks (`gravity_rifts`, `rift_roll_exempt`, `rift_crucible`, `move_bonus`, the replenish note), Empyrean
  (`blocked_borders`, `ignores_nebulae`, `passable_owners`, `move_bonus`), Ghosts (wormholes, `linked_systems`,
  `production_cost_reduction`), Muaat (extend the example: `extra_production`, supernova hooks, Nucleus/Magmus
  component actions). Files: the four faction files + `_units`, `render/factions/{cabal,empyrean,ghost,muaat}.rs`,
  `core_factions/c1.rs`. Dual harness: `dual_query!` on the A2 movement dispatchers (`hooks_movement.rs`) with the ported set; movement
  area already on Core, so the movement suite must stay at 0 differences with `movement=dual,factions=dual`.
  Gate steps: common gate + the movement template check (`core_equivalence templates --routes`). Limits: movement
  hooks are A2's signatures; `PassSite`/`MoveSite` changes go to the A5 steward.
* **C2 `core-c2-factions-economy`** (arborec, argent, saar, titans, yssaril; `src/c2_economy/`). Scope: production
  hooks (`cannot_produce`, `production_destinations`, `pds_placement_alternative[_performed]`), action-card
  draw hooks (`action_card_draw_bonus/requested`, `action_cards_drawn`, `action_card_limit`, `action_cards_forbidden`), `transaction_reach`,
  `votes_first`, `afb_excess`, `scores_without_home`, Arborec/Titans/Saar leaders and component actions. Files: the
  faction files + `titans_leaders.rs`, `render/factions/...`, `core_factions/c2.rs`. Dual: `dual_query!`
  for value slots, `dual!` for the performed/ask slots. Gate: common gate with `TI4_AREAS=factions=dual,production=dual`. Limits:
  the draw asks are B6's families; production placement is B3's.
* **C3 `core-c3-factions-combat`** (mentak, nomad, naaz, ralnel, sardakk; `src/c3_combat/`). Scope: `may_sustain`
  (combat and ground), `grants_sustain`, `borrowed_stats`, `trade_good_worth`, `unit_form_override`, `explored`,
  `ground_combat_round_started`, `commit_candidates`, `ground_rolls_extra_hits`, `unit_dice`, `skip_to_commit`,
  the direct slots of nomad/ralnel; **moves** `register_naaz_nomad` out of `rules/combat.rs` and replaces the call
  in `core_combat.rs` by `register_all` (same commit, after B1 is merged). Dual: combat and invasion call sites
  (`dual_query!` per unit/round; the dice stream must stay bit-identical: compare rng positions). Gate: common
  gate with `factions=dual,combat=dual,invasion=dual`. Limits: dice order follows `MODULES` order.
* **C4 `core-c4-factions-strategy`** (mahact, naalu, obsidian, winnu, yin; `src/c4_strategy/`). Scope: secondary
  waivers (`secondary_waived/ers`), `strategy_phase_ended`, `may_view_hand`, `fighter_fleet_weight_halves`,
  `temporary_ground_force`, `custodians_free`, `use_leader_timed`, `use_leader_strategy_primary`, Mahact tokens (8 direct slots),
  Winnu/Yin/Obsidian leaders and timing abilities. Dual: strategy dispatchers; leader slots with `dual!`. Gate: common gate
  with `factions=dual,strategy=dual`. Limits: Mahact's `state/marks` (foreign tokens) and `fleet_pool_owners` are read by A1 queries; keep
  the mark layout F5 defined.
* **C5 `core-c5-factions-cross`** (bastion, crimson, deepwrought, nekro; `src/c5_cross/`). Scope: the 37 direct slots
  (`bastion_*`, `crimson_*`, `deepwrought_*`, `nekro_*`), Crimson breaches/deploy, Deepwrought oceans and research,
  Nekro Alastor/propagation/Valefar copies (`flagship_has_text` helper used by other factions: move it
  with Nekro and expose it as a core query only if a B package already did; else report). Dual: `dual_query!` at
  the combat/invasion/production call sites of each direct slot. Gate: common gate with `factions=dual` plus
  every area that has a call site, on Dual once. Limits: highest cross-area risk; commit and gate faction by faction.
* **C6 `core-c6-factions-late`** (firmament, keleres x3, borrowed_*; `src/c6_late/`). Scope: Firmament plots and flip
  (scores objectives through A4/B5 interfaces), Keleres three modules (agent offer/window, executive order, Argent/Mentak/Xxcha
  variants, `keleres_units`), the borrowed helpers (`Id::Borrowed`). Dual: as C5; plots with `dual!`. Gate: common gate, then
  the wave-C close-out: remove `#[ignore]` from `every_module_is_provided`, `missing()` empty, `PORTED` = all 28,
  `factions=dual` on all areas on the golden games, and the build default switched to `Core` for `Area::Factions` if
  every earlier shard's gate is on record. Limits: do not start the deletion commit; that is D2 after a clean night.

## 6. Open points for the user

1. The ported-set rule (4.3) means a slot reaches `Core` only when all its providers are ported, so the default
   flips late (C6) even though Dual runs from C1. Alternative: flip per slot (more bookkeeping). Recommendation: as written.
2. `register_naaz_nomad` (B1) is moved by C3; if B1 is merged earlier the move is a trivial rename, no decision needed.
3. The Disentangle plan may change which module owns `faction_abilities.rs`/`leaders.rs` dispatch (B7): re-check
   when `docs/ENGINE_DISENTANGLE.md` lands.
