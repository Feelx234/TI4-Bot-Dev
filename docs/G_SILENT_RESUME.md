# D2 lane G (`core-g-silent`): the silent gaps, resume note (2026-10-31)

Branch `core-g-silent`, cut from `core-m7-native` (`40081691`). Never pushed; the live branch is untouched. Read `docs/BAIL_MAP.md` section 5 and `docs/M7_RESUME.md` (S1-S4) first.
Slots `40_000..` (rules) and `41_000..` (loud gaps) are this lane's. Cheap checks only (see the end).

## The rule of this lane

A string-registered ability the core has no twin for is MISSING from a native game without any bail. Each one is now either **native** (a rule of the core table, lockstep against the
string) or a **loud gap** (`silent::gaps::GAPS`: a native game stops with a counted `Unknown::AbilityGap(slot)` where a seat holds the card), and a registry audit fails for any
ability that is neither (`ti4-engine/src/ability_audit.rs`).

## Native (core half + question types + byte-exact renderer + lockstep)

| item | where | string | lockstep |
|---|---|---|---|
| S1 Gift of Prescience (play at `STRATEGY_PHASE_ENDED`, return at `STATUS_PHASE_ENDED`) | `ti4-engine-core-factions/src/silent/gift.rs` | `factions::naalu::{gift, gift_return}` | `silent_live_tests::gift_*` (phase arm; both answers) ; the return has no native emitter in this base (lane C emits `STATUS_PHASE_ENDED`): core test only |
| S2 Mitosis (asks where the infantry goes / which infantry becomes a mech) | `silent/mitosis.rs`, `Ask::G(Mitosis)` | `factions::arborec::mitosis` | `silent_live_tests::mitosis_*` |
| S4 Guild Ships / Trade Convoys / Arbiters in the trade env | `rules/trade.rs::KernelEnv` (the same four answers lane M1's `kernel::trade_env` gives: M1 supersedes this, take its side on conflict) | `faction_abilities::{ignores_neighbours, trades_action_cards}` | core: `silent_abilities` does not cover it; closing-menu shadow below |
| Infantry II / Letani Warrior II / Crimson Legionnaire II return | `silent/returns.rs`, `Ask::G(Landing)` | `infantry_ii::returns`, `arborec::warrior_returns`, `mahact_units::legionnaire_returns` | `silent_live_tests::infantry_ii_*`, `letani_warriors_and_crimson_legionnaires_*` (prelude arm; 1, 2 units, 1 or 2 planets, tech gone) |
| Saar Scavenge | `silent/scavenge.rs` | `factions::saar::scavenge` | core test only (no native emitter of `PLANET_CONTROL_GAINED` outside the invasion window) |
| Mentak Pillage (both events, targets by the neighbours of the trade env, Promise of Protection, staged gain) | `silent/pillage.rs`, `Ask::G(Pillage)` | `factions::mentak::pillage` | core test; its Suffi An half is the loud gap `gaps::SUFFI_AN` |

Registration order matters inside one bucket (the registry test compares it): Pillage is registered before the movement package (`silent::register_before_movement`), the rest after the
packages (`silent::register_abilities`); both called from `reactions_core::table()`.

## Loud gaps (`silent/gaps.rs`, 61 rows) and the audit

`GAPS` rows are registered only in the LIVE environment (`live/env.rs::env_for`, after `set_table_codes`): the dual keeps its own bail (`Recording::unported`), the string runs next to the core there.
Each row's condition is coarse on purpose (the seat plays the faction / holds the note or technology) and answers `Unknown::AbilityGap(slot)` when the event is announced. The names are
`ability_gaps::GAP_IDS` (index-aligned with `GAPS`). `ability_audit::tests` pins: the set of string abilities without a rule equals `ACCOUNTED` (each `Dropped` / `Carried` / `Lane` / `Gap(row)`),
every `Gap` has its row (same event, relation, kind), every row is registered under the string's id.
`Lane("core-m6c-tactical")` / `Lane("core-m1-trade")` entries are rules those branches carry: the test FAILS when they merge (delete the lines; for the M6c gaps of its own audit that this lane
turned into rows - Titans Awaken and Ouranos, Arborec commander and flagship, So Ata, the Ssruu copy of I48S, Mercer - delete the row when M6c ports the rule).

## Hand-overs

* Leader unlock moments (task item 4): the turn start is native; the pass (Watchful Ojz) is in lane F (`driver/turn/action.rs::pass`), the scored objective and the status bookkeeping in lane C (`driver/windows/{scoring,status_bookkeeping}.rs`). Nothing is left in this base.
* Peace Accords (Xxcha) is lane E's counted gap `TurnGap::PeaceAccords`; not duplicated.
* Not ported here, loud gaps: Naaz agent / commander (needs the exploration as a child of an effect), Foresight (relocation), Sol commander, Winnu Reclamation family and mech/agent/HTP, Muaat commander/hero, Ghost commander, Empyrean agent/commander/Voidwatch, Cabal agent, Argent commander/Strike Wing, Sardakk Exotrireme/Tekklar/agent, Nekro commander/flagship/C4Y, Yin Spinner, Naalu Z'eu/Oracle/mech, Bioplasmosis, Magen Defense hit, the Ssruu copies, Saar Scavenger Zeta, Ragh's Call, Scepter, Spy Net, Stymie, IFF, Ambuscade.
* Hook slots (BAIL_MAP part b) and listeners without an emitter (part c): see "Not done".

## Not done

* Hook-slot audit (part b: Cabal research waivers, `secondary_waived`, Naalu `may_view_hand`) and the part c audit.
* The shadow (`native_play.sh --shadow`) was not re-run after the batch (disk emergency, builds were slow).
* Mutation check of the new core halves.
