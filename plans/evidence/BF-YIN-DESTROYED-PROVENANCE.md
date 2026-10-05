# BF-P01 — Yin destroyed-ship provenance: every `SHIP_DESTROYED` says what removed the ship

Package: P01 from `plans/HANDOVER_2026-10-04_BASE_FACTIONS_PI.md` ("Yin copied Milor: reconcile
combat provenance").
Branch: `wp/base-factions`. HEAD at start and at the end of this package: **ac296cbc** (no commit
was made; see "Commit state" below).

## Normative sources

- `crates/ti4-content/content/leaders.json`, `yinagent` (Brother Milor):
  `abilityWindow: "After a player's unit is destroyed during combat:"`, with the corpus note
  `"\"During combat\" timing is wording from the latest (TE) version of this agent."` The printed
  trigger is the requirement; the engine had been treating every `SHIP_DESTROYED` as satisfying it.
- `crates/ti4-content/content/action_cards.json`, `courageous` (Courageous to the End):
  window `"After 1 of your ships is destroyed during a space combat"`.
- Ground precedent already in the tree: `crates/ti4-engine/src/invasion.rs` emits ground
  destructions with an explicit `cause` (`"ground_combat"`, `"harrow"`, `"bombardment"`,
  `"space_cannon_defense"`), and Yin's ground Milor already reads it
  (`matches!(event.text("cause"), Some("ground_combat" | "harrow"))`). This package applies the
  same idea to ships.
- No Python reference was consulted; Python parity is not an acceptance criterion.

## The problem, measured before the change

`SHIP_DESTROYED` is emitted by more than one producer. A producer census (read-only, before any
edit) found:

| Producer | Path | Was it a space combat? |
|---|---|---|
| Assigned combat casualties | `combat.rs` `announce_ship_destroyed` (3 call sites in the combat driver) | yes |
| Fleet reroll casualties | `combat.rs::remove_casualty_units` → `announce_ship_destroyed` | yes |
| Yin Impulse Core / Devotion sacrifice | `factions/yin.rs` → `combat::destroy_units` → staged drain | yes (a `CombatMoment` of that combat) |
| Yin Van Hauge flagship chain | `factions/yin.rs` → `combat::destroy_units` | depends on what killed the flagship |
| Sardakk Exotrireme II | `factions/sardakk.rs`, registered on `SPACE_COMBAT_ROUND_ENDED` | yes |
| Direct Hit | `action_cards.rs::direct_hit` → `pending_destructions` → `reactions.rs` drain | yes (its only window is `SUSTAIN_DAMAGE_USED`) |
| Courageous | `action_cards.rs::courageous` → staged drain | not verifiable from the effect |
| Muaat Nova Seed | `factions/muaat.rs` → `combat::destroy_units` | **no** |
| Generic component/leader effects using the staged route | various | no |

Yin's ship Milor and its Ssruu copy both fired for all of them. A Nova Seed map edit is not a
combat loss, and a listener cannot recover the difference from the board: once the ship is off it,
a combat casualty and a breakthrough removal are the same fact.

`GameState::pending_destructions` also carried only `(system, player, unit_type)`, so an effect that
stages a destruction and the game that later announces it had no way to carry the truth between the
two moments.

## Design

One vocabulary, stated by the producer, read by the consumer:

- `SHIP_CAUSE_COMBAT = "space_combat"` — the ship was removed while a space combat was resolving:
  an assigned casualty, a reroll casualty, a start-of-combat ability (Assault Cannon), a hit
  producer inside that combat (Impulse Core, Devotion), a ship a card played into that combat
  removed (Direct Hit), or a unit removed after a round of that combat (Exotrireme II).
- `breakthrough:<id>`, `unit_ability:<id>`, `component:<id>`, `action_card:<id>` — removed by that
  effect, with no combat the engine can see.
- A chain effect inherits the cause of the destruction that opened its window (Van Hauge), so a
  non-combat loss cannot become a combat one by propagation.
- `destroyed_during_combat(event)` is the single consumer predicate. An event with no `cause` —
  one recorded before this key existed — is **not** a combat loss: an absent statement is not
  evidence of a combat.

The cause is stored with the staged destruction (`pending_destructions` is now a 4-tuple) so the
staging and the announcement cannot drift apart on a key a listener reads.

## Changed paths

| Path | Change |
|---|---|
| `crates/ti4-engine/src/combat.rs` | `SHIP_CAUSE_COMBAT`, `destroyed_during_combat`, shared `pub(crate) ship_destroyed_payload`; `announce_ship_destroyed_type` takes a cause; `destroy_units` and `destroy_ships_announced` take a cause and store it; `announce_staged_destructions` drains the 4-tuple |
| `crates/ti4-model/src/state.rs` | `pending_destructions: Vec<(SystemId, PlayerId, UnitTypeId, String)>`, documented as the cause the eventual announcement carries |
| `crates/ti4-engine/src/reactions.rs` | the action-card drain consumes the 4-tuple and builds the payload through `combat::ship_destroyed_payload` instead of duplicating it |
| `crates/ti4-engine/src/action_cards.rs` | Direct Hit stages `SHIP_CAUSE_COMBAT` (its window is opened only by the combat driver); Courageous stages `"action_card:courageous"` |
| `crates/ti4-engine/src/factions/muaat.rs` | Nova Seed stages `"breakthrough:nova_seed"` |
| `crates/ti4-engine/src/factions/sardakk.rs` | Exotrireme II destructions stage `SHIP_CAUSE_COMBAT` |
| `crates/ti4-engine/src/factions/yin.rs` | the sacrifice through `produce_hits` stages `SHIP_CAUSE_COMBAT`; Van Hauge inherits the triggering event's cause (falling back to `"unit_ability:yin_flagship"`); native and copied Milor ship listeners require `destroyed_during_combat`; fixtures split into `destroyed` (combat) and `destroyed_outside_combat` |
| `crates/ti4-engine/src/game.rs` | one added assertion in the existing Direct Hit game test |

Scope note: the handover suggested starting with `yin.rs` only and specifying the shared producer
work separately. The shared seam is not separable in practice — a Yin consumer test is meaningless
until the producers state their provenance — so this package is effectively P01a (shared producer
vocabulary) plus P01b (Yin consumers), recorded here together.

## Tests and results

Red first. Both new Yin tests were written before the consumer condition existed, and both failed
for the intended reason with the gate stubbed to `true`:

```text
cargo test -p ti4-engine --lib -- brother_milor_ignores_a_ship_destroyed_outside_combat \
                            ssruu_copies_milor_only_for_a_destruction_during_combat
→ FAILED. 0 passed; 2 failed  ("the copied trigger is the printed one": left 2, right 0)
```

After the condition was added:

```text
cargo test -p ti4-engine --lib factions::yin   → 35 passed, 0 failed, 1 ignored
cargo test -p ti4-engine --lib combat::        → 93 passed, 0 failed
cargo test -p ti4-engine --lib game::tests::direct_hit → 2 passed
cargo test -p ti4-engine --lib                 → 2046 passed, 0 failed, 1 ignored
cargo test -p ti4-engine                       → lib 2046/0, content_ids_resolve 1/1,
                                                decision_delivery_inventory 4/4, other integration 5/5
cargo test -p ti4-model                        → 82 passed
cargo test -p ti4-sim                          → 51 passed, 1 ignored
cargo test -p ti4-policy                       → 273 passed
cargo test -p ti4-bridge                       → 62 + 6 + 12 + 3 passed
cargo test -p ti4-content                      → 134 + 1 passed
cargo test -p ti4-legacy                       → 25 passed
cargo test -p ti4-cli, cargo test -p xtask     → 0 tests, clean
cargo check --workspace --all-targets          → clean
cargo fmt -p ti4-model -p ti4-engine           → no changes required after formatting
cargo clippy -p ti4-model -p ti4-engine --all-targets → no warning points at the new code;
                                                        the pre-existing pedantic warnings remain
```

New assertions cover, in order:

1. a combat-assigned casualty carries `cause = space_combat` (`combat.rs::a_forced_hit_takes_a_non_fighter_while_one_remains`);
2. a staged destruction announces exactly the cause it was staged with, and `destroyed_during_combat`
   is false both for a non-combat cause and for a payload with no `cause` key at all
   (`combat.rs::a_staged_destruction_announces_the_cause_it_was_staged_with`);
3. a real game's Direct Hit loss carries `space_combat` (`game.rs::direct_hit_destroys_the_ship_that_sustained_the_holders_hit`);
4. native Milor refuses a Nova Seed loss and a self-named component loss, and does not exhaust the
   agent for an unmet trigger (`yin.rs::brother_milor_ignores_a_ship_destroyed_outside_combat`);
5. the Ssruu copy refuses the same non-combat loss and does not spend Ssruu
   (`yin.rs::ssruu_copies_milor_only_for_a_destruction_during_combat`).

Existing Yin/Ssruu fixtures that stand for combat losses now declare `cause: space_combat`
explicitly rather than relying on the old "every `SHIP_DESTROYED` is a combat" assumption.

## Compatibility and schema effects

- No fixture file contains `SHIP_DESTROYED`, so no golden event fixture was regenerated.
- `event_hash` (`crates/ti4-engine/src/fingerprint.rs`) has no consumers outside that file; the new
  payload key changes event hashes but nothing currently compares them.
- `pending_destructions` is in-flight bookkeeping that is emptied before any checkpoint comparison;
  its serialized tuple arity did change (3 → 4). No stored checkpoint in the repository contains it.
- `ti4-review`'s frozen golden (`tests/golden/pre-r02-mlp-seed7777-rotation2.json`) fails at frame 7.
  This was checked for causality: with the `cause` key temporarily removed from
  `ship_destroyed_payload`, the same test fails at the same frame with the same hashes (the current
  trace emits event id 2 where the golden has id 1). The mismatch predates this package and comes
  from other uncommitted work in the shared checkout. It is left open, not regenerated.

## Deliberate limitations recorded

1. **Courageous.** Its printed window says "during a space combat", but the engine's guard for that
   window cannot read the qualifier, and the effect cannot see the event that opened it (there is no
   live-combat flag in `GameState`). Its destructions therefore name themselves
   (`action_card:courageous`) rather than claiming a combat they cannot see. Consequence: if Yin
   plays Courageous inside a real combat, the ships it destroys do not trigger Milor. That is a
   rules-accuracy gap in the *guard*, not in the provenance mechanism; fixing it needs either a
   live-combat indicator or a provenance-carrying `last_ship_destroyed` handoff. Not done here.
2. **Removals that never announce** (barrage fighters, units stranded by a retreat, Saar Armageddon
   Relay's direct removals) produce no `SHIP_DESTROYED` at all, so they have no provenance to label.
   That is unchanged by this package.
3. **Mentak hero** reads `SHIP_DESTROYED` too; its printed trigger has no "during combat"
   qualifier, so it is intentionally unaffected.

## Review status

Tier C (event payload vocabulary + timing/legality of a printed trigger) is **required and not yet
performed**. No independent reviewer has run on this package; acceptance is pending. Self-review is
not acceptance.

## Commit state

No commit was made. `combat.rs`, `action_cards.rs`, `game.rs`, `muaat.rs`, `sardakk.rs`, `yin.rs`
and `state.rs` all carry hunks from other sessions in the shared dirty checkout (e.g. `combat.rs`
differs from HEAD by 284 lines, of which this package is a small part). File-level staging would
commit unrelated work, and a subset commit would not compile on its own. The package's behaviour is
implemented and green in the shared tree; a scoped commit needs hunk-ownership reconciliation first.

## Next safe action

Tier-C review of the provenance vocabulary and the Milor consumer condition, then either the
Courageous guard correction (needs a live-combat seam) or move to P02 (Jol-Nar copied research
agent), which touches disjoint files.

## 2026-10-05 correction: effect source and combat window are independent

An independent review rejected the single-vocabulary design above. `cause` answers **what removed
the ship**; it cannot also answer **whether that removal happened during a space combat**. The
counterexample is Direct Hit: the card can destroy a ship that sustained against pre-combat SPACE
CANNON, while the same card can also destroy one during a combat round. Courageous, Devotion, and
Impulse Core likewise need to retain their real source when they act inside combat.

The implementation now carries both facts:

- every `SHIP_DESTROYED` payload has `cause` and `during_space_combat`;
- `destroyed_during_combat` reads only the boolean;
- staged destructions use `(system, owner, unit, cause, during_space_combat)`;
- Direct Hit stages `cause = action_card:direct_hit` and copies the boolean from the sustain
  handoff; the ordinary absorption path defaults to non-combat, while the combat window and Waylay
  barrage path state `true`;
- Courageous is offered only when the incoming destruction says `during_space_combat = true`, and
  its successful revenge losses retain `cause = action_card:courageous` with the boolean true;
- Devotion and Impulse Core retain `faction_ability:devotion` and `technology:impulse_core`, both
  with the boolean true; Van Hauge propagates both facts independently;
- native and copied Brother Milor read only the boolean, so an action-card-caused loss during
  combat counts while the same cause outside combat does not.

### Save compatibility

`pending_destructions` has a narrow custom decoder for all observed wire shapes. Current 5-tuples
load directly. A 4-tuple preserves its cause and maps the historical `space_combat` sentinel to
`true`; any other cause maps to false. A legacy non-empty 3-tuple has neither fact and loads as
`cause = unknown`, `during_space_combat = false`. The sustain handoff similarly loads its legacy
4-tuple with a conservative false boolean. The full `GameState` round-trip test now exercises
non-empty current tuples, and a compatibility test covers the legacy 3- and 4-tuple forms.

### Regression coverage added

- Direct Hit preserves `action_card:direct_hit` and a false pre-combat sustain flag.
- Courageous is offered for the same effect source only with a true incoming flag, and refused
  with false.
- Milor accepts `action_card:courageous` with a true flag and refuses non-combat Nova Seed and
  component losses.
- staged announcements assert the boolean alongside the cause.

Per the bounded task instruction, no Cargo command was run for this correction. `git diff --check`
passed for the touched files. `rustfmt --check` found only formatting in a concurrently edited,
out-of-scope Naaz test hunk in `action_cards.rs`; the correction's own reported formatting was
applied manually without rewriting shared files.

## October 5 independent review continuation

Root reviewed the previous Terra provenance correction; cause and during_space_combat remain independent, with conservative legacy migration. AFB/Waylay, Reflective Shielding, retreat capacity, Van Hauge, Assault Cannon, and Exotrireme source gaps are corrected in the working tree. New ordinary AFB and absorption routes are BF-gated and now propagate their destruction-window errors. The model crate passes all83 tests, including nonempty current and old tuple shapes.

A separate Luna source review found phantom planetary Courageous removal, ambiguous Direct Hit matching after sustain, and swallowed staged-drain errors. Root corrected Courageous via the shared exact remover; both actual sustain sites now preserve exact Unit/location in BF-only private handoff bookkeeping, and Direct Hit uses it. Game BF staged drains now use a fallible transactional helper restoring the pending batch, state, RNG/dice, timing/sequence and external log onerror. New regression source is present; final full-engine run and reviewer follow-up remain pending. No commit/whole-plan closure is claimed.
