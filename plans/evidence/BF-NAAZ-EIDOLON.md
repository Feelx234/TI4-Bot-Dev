# BF-NAAZ-EIDOLON — a hit an Eidolon Maximum may not take is not offered to it

Date: 2026-10-04
Branch: `wp/base-factions`
Inherited HEAD: `920c852c` ("BF-COMBAT-ORIGIN: complete the combat-origin seam and the wave that
left it half-built", 2026-10-04 00:5x)
Package class: engine implementation in a shared site + fixtures
Reviewer tier: **not run** — no review peer was available in this session. See *Limits*.

## Objective

`plans/evidence/BF-naaz.md` Wave G left two Eidolon Maximum clauses as **blocked / none**:

> "It cannot be assigned hits from unit abilities." and "both a ship and ground force"

BF-COMBAT-ORIGIN had completed the `HitOrigin` seam, so a filter existed in
`absorb_hits_with_origin` that drops ability-hit-immune units from the casualty list. Nothing
asserted it, and the SUSTAIN DAMAGE offer — which is part of *assigning* a hit, not a separate
cancellation — still offered the Maximum's sustain for hits it is not allowed to take. This package:
write the fixtures first, then close whatever they found.

## Normative source

`crates/ti4-content/content/units.json`, id `naaz_voltron` (`baseType: mech`, `isShip: true`,
`isGroundForce: true`, `sustainDamage: true`, `canBeDirectHit: true`, `combatDieCount: 4`,
`combatHitsOn: 4`, `moveValue: 3`):

> This unit is both a ship and ground force. It cannot be assigned hits from unit abilities. Repair
> it at the start of every combat round. Game effects cannot place or produce your mechs. When this
> unit is destroyed or removed, flip this card and return it to your play area.

The engine's own rule references are used unchanged: hit assignment cites 15.2a
(`crates/ti4-engine/src/combat.rs`, `if alive.is_empty() { return Ok(()); // 15.2a }`) and the
sustain choice is raised under `DecisionSource::Rule("82")` with `subtype "sustain_damage"`.

## What was actually wrong

`offer_sustain` ran **before** the eligibility filter. For an ability hit against a Naaz fleet whose
only SUSTAIN DAMAGE carrier was the Maximum, the engine asked the player to spend the Maximum's
sustain on a hit that could never have been assigned to it; accepting cancelled the hit and left the
Maximum damaged. With nothing else in the system the hit was consumed rather than discarded under
15.2a.

Failing-first run (`cargo test -p ti4-engine --lib naaz::tests`):

```text
---- factions::naaz::tests::a_maximum_is_never_assigned_a_hit_from_a_unit_ability ----
the Maximum was damaged or destroyed by a hit from a unit ability

---- factions::naaz::tests::an_ability_hit_is_discarded_when_nothing_else_can_take_it ----
15.2a discards a hit that cannot be assigned; ...

test result: FAILED. 27 passed; 2 failed; 0 ignored
```

The third fixture, `a_maximum_takes_ordinary_combat_hits`, passed before the change: it is the
control that shows the immunity did not already swallow ordinary combat rolls.

## Changes

`crates/ti4-engine/src/combat.rs`

1. New `assignable_to_hit(state, content, sources, player, system, unit_type, origin)` next to
   `sustains_in_space`: true unless the hit is `HitOrigin::UnitAbility` **and** the faction hook
   `hooks_combat::ability_hit_immune` refuses it for that unit.
2. `offer_sustain` takes `origin: HitOrigin` and offers only units that pass `assignable_to_hit` —
   both the space-area list and the `planet:<id>` branch that lets a planet-standing Maximum sustain
   alongside a real ship.
3. `absorb_hits_with_origin` uses the same helper for its casualty filter instead of an inline copy
   of the hook call, so the offer and the assignment cannot drift apart.
4. The four existing `offer_sustain` tests pass `HitOrigin::CombatRoll`, which is what a combat
   roll is.

`crates/ti4-engine/src/factions/naaz.rs` — three fixtures plus two test helpers (`assign`,
`maximum_and_a_fighter`, `maximum_whole`). `assign` drives the public
`absorb_hits_seeing_with_origin` with a `FirstOption` decider (takes every sustain it is offered)
or an `AlwaysDecline` decider, so "was the Maximum offered sustain at all" is decided by the engine
and not by the test.

## Fixtures

| Test | Pins |
|---|---|
| `a_maximum_is_never_assigned_a_hit_from_a_unit_ability` | Maximum + fighter, 1 `UnitAbility` hit, decider accepts any sustain: the fighter is destroyed and the Maximum is present and undamaged. Before the change the Maximum took the hit through its sustain. |
| `an_ability_hit_is_discarded_when_nothing_else_can_take_it` | Maximum alone, 1 `UnitAbility` hit: nothing is damaged and the Maximum is still on the board — the hit is discarded (15.2a), not absorbed. |
| `a_maximum_takes_ordinary_combat_hits` | Maximum + fighter, 1 `CombatRoll` hit, sustain declined: the Maximum is destroyed and the fighter is untouched. Immunity is specific to unit abilities; the Maximum is an ordinary ship against a combat roll, and it is still offered sustain for those hits. |

## Commands and results

Debug builds run with `CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0` (this crate's test build is
memory-hungry; see *Environment*).

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -j 4 naaz::tests` (before the fix) | 27 passed, **2 failed** |
| `cargo test -p ti4-engine --lib -j 4 naaz::tests` (after) | **29 passed, 0 failed, 0 ignored** |
| `cargo test -p ti4-engine -q --no-fail-fast -j 4` | **1931 passed, 0 failed, 1 ignored** (was 1928); integration targets 1/1, 4/4, 5/5; doctests clean |
| `cargo fmt -p ti4-engine` then `cargo fmt -p ti4-engine -- --check` | no diff, exit 0 |
| `cargo clippy -p ti4-engine --all-targets -j 4` | `ti4-engine` lib **14** warnings, lib test **18** (14 duplicates), `ti4-model` 1 — identical to the counts recorded in `BF-COMBAT-ORIGIN.md`. No warning points at `naaz.rs`; `offer_sustain`'s `too_many_lines` warning predates this package (155 lines at HEAD, 173 now, threshold 100). |
| twelve-faction seated soak, 12 concurrent slices (`base_faction_soak <faction> 0 200`) | **2,400 games, 0 failures** — every faction `games=200 failures=0`; no engine error, no refusal, no replay divergence. Wall clock **415s** with `rebaseline_behavior` running concurrently (13 processes, 32 logical cores). |
| `rebaseline_behavior` (six original factions) | reproduces the recorded point exactly on all ten metrics: `completion 1.000000`, `faction_differentiation 0.615464`, `score_spread 1.950651`, `share_INVASION_RESOLVED 0.014055`, `share_PRODUCTION_RESOLVED 0.025814`, `share_SHIP_MOVED 0.046439`, `share_SPACE_COMBAT_RESOLVED 0.003217`, `share_SYSTEM_ACTIVATED 0.051628`, `share_TACTICAL_ACTION_BEGAN 0.025229`, `vp_pace 0.472222`; recomputed 95% intervals match. The same five metrics remain outside the retired v45 bounds, as previously recorded. |
| `cargo test -p ti4-sim --lib -q -j 4` | 51 passed, 0 failed, 1 ignored (`the_suite_reproduces_and_stays_within_the_recorded_bounds`, the retired behaviour gate) |
| `cargo test -p ti4-engine --lib print_faction_ledger -- --ignored --nocapture` | unchanged: arborec 12/12, argent 12/13, ghost 12/12, mentak 12/12, muaat 13/13, naalu 11/13, **naaz 11/13**, saar 12/13, sardakk 12/12, winnu 10/11, yin 9/11, yssaril 10/12 — **136/147** |

Release builds for the two `ti4-sim` examples used
`LIBTORCH=D:\Projects\ti4-engine-rs\out\libtorch-2.9.1-cpu\lib` and `cargo build --release -p
ti4-sim --example base_faction_soak --example rebaseline_behavior -j 4` (2m04s). `LIBTORCH` is not
set for debug work: it invalidates the cached `torch-sys` debug artifacts and the pinned install has
no `include` directory.

## Environment: the earlier slow search and the two build OOMs

Nineteen `grep` processes from previous sessions were still running — the oldest four days old —
scanning `out/` and `target-cuda-repack/` (libtorch DLLs, checkpoints, model repacks). While they
ran, `cargo test -p ti4-engine --lib` failed twice with `rustc-LLVM ERROR: out of memory` /
`STATUS_STACK_BUFFER_OVERRUN` at `-j 4` and at `-j 1`. After `Get-Process grep | Stop-Process` the
same command succeeded at `-j 4` with no code change. Content searches in this repository use
`git grep` (tracked files only; ~0.1s) rather than recursive `grep`.

## Ledger decision: `naaz_voltron` and `naazbt` stay unclaimed

The immunity is real and now tested, but claiming the unit would read as coverage of the card, and
three parts of it are not implemented:

1. **Produced hits are still labelled `HitOrigin::CombatRoll`.** The only production site that
   labels hits as ability hits is the space-cannon aftermath (`crates/ti4-engine/src/game.rs:269`).
   `ProducedHits` from unit abilities (Mentak Ambush, Yin/Impulse Core, Devotion) and the
   bombardment/space-cannon-defence paths reach the same assignment machinery unlabelled, so the
   Maximum is still assignable those hits. Deciding which producers count as "unit abilities" is a
   scope decision for a separate package.
2. **Effect placement of mechs is not gated** (BF-naaz request 7): `cannot_produce` covers
   production; "Game effects cannot place … your mechs" has no gate.
3. **A captured Maximum is not re-typed on release** (BF-naaz request 8).

`naazbt` stays unclaimed for the same reason: the Absolute Synergy window exists and is tested, but
the unit it produces is not fully covered. Ledger stays `naaz 11/13`; the two gaps are
`Unit naaz_voltron` and `Breakthrough naazbt`.

## Open findings

1. Which hit producers are "unit abilities" (above). Until that is decided, the immunity covers the
   shared absorption path only.
2. No fixture pins the planet-standing Maximum joining its system's space combat, or the
   `planet:<id>` sustain branch, through a real combat window. Both are implemented
   (`combat.rs:117-133` in `ships_of`, mirrored in `offer_sustain`) and now exercised only by the
   soak and by `offer_sustain`'s own tests.
3. Tier C review is still owed for `920c852c` and for this package.

## Next safe action

Label the produced-hit paths (`ProducedHits`, invasion bombardment and space-cannon defence) with a
decided origin, with fixtures for at least Ambush and one bombardment case; that is the remaining
half of BF-naaz hook request 6 and the precondition for claiming `naaz_voltron`.

## Limits

No independent reviewer or reviewing model ran for this package: no review peer was available in the
session, and `AGENTS.md` requires a reviewer other than the implementer for tier C/D. The claims
above are therefore implementer-verified only. The soak and `rebaseline_behavior` are diagnostics,
not the retired acceptance gate.

## Subsequent independent review, 2026-10-04

See [BF-CODEX-INHERITED-REVIEW-2026-10-04.md](BF-CODEX-INHERITED-REVIEW-2026-10-04.md) for findings and corrections to producer taxonomy, placement/release gap claims, atomic-package scope, and the limits of aggregate compatibility diagnostics. This preserves the original implementation record; later WIP is separately reviewable.
