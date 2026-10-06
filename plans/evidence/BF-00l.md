# BF-00l — hook batch (part 1) and agent guide

Branch `wp/base-factions`. Implementer: Claude Code coordinator (Opus 5.5). Python not used.

## What changed

| Path | Change |
|---|---|
| `factions/mod.rs` | New hooks `timing_abilities` (modules register `timing::Ability` per seat), `unit_roll_modifier`, `unit_dice` (with `CombatUnit`); nested per-area hook structs `combat`/`ground`/`economy`; contract docs (construction-time state, unsaved frequency bookkeeping, swallowed cancels, adjust-don't-overwrite) |
| `factions/hooks_{combat,ground,economy}.rs` (new) | Empty per-area hook structs, each owned by one wave-B package so three agents never edit the same hook file |
| `reactions.rs` `arm` | Registers module timing abilities per seat after the shared slots |
| `combat.rs` | `effective_hits_on` subtracts module roll modifier; `fleet_groups` dice through `unit_dice` |
| `invasion.rs` | `ground_combat_value` and ground dice through the same hooks |
| `fixtures.rs` | `seated_game`, `with_context`, `armed_resolver` for faction tests |
| `plans/BF_AGENT_GUIDE.md` (new) | Rules, contract, events, examples and test patterns for faction agents |

## Checks

| Command | Result |
|---|---|
| `cargo test -p ti4-engine -q` | lib 1399 passed, 1 ignored (before the fixture test); integration binaries ok |
| `cargo test -p ti4-engine --lib factions::` | 5 passed, 1 ignored |
| `LIBTORCH=out/libtorch-2.9.1-cpu cargo test -p ti4-sim -q` | 51 passed, 1 ignored — behaviour unchanged |
| `cargo clippy -p ti4-engine --all-targets` | no warnings in touched lines/files |

## Independent review (Opus subagent, tier C)

No code blockers. Neutral with empty modules (empty `register` adds nothing; module slots come
after shared ones). Every space roll goes through `fleet_groups`→`effective_hits_on`, every ground
roll through `roll_ground`→`ground_combat_value`. `arm` runs on every construction path
(`Game::with_table`), so closures are rebuilt on restore.

| Finding | Disposition |
|---|---|
| Swallowed cancels/errors at `let _ = emit` sites | Documented (Hooks + guide) |
| `Resolving::emit` without a timing handle opens no window | Documented in guide |
| Test helpers private in `faction_abilities` | Moved to `fixtures` (`seated_game`, `with_context`, `armed_resolver`) |
| Timing test should arm through `reactions::arm` | Guide prescribes `fixtures::armed_resolver` |
| Unrelenting already applied; re-adding doubles it | Guide lists already-done items |
| Frequency bookkeeping resets on restore | Documented: gate "once" on state |
| `timing_abilities` sees construction-time state | Documented; fixture seats factions before construction |
| `effective_hits_on` takes system from `active_system`; `CombatUnit` has no unit identity | Accepted for now; revisit when a module needs damaged state |
| `unit_dice` fold order | Documented: adjust, don't overwrite |
| Policy/training strength estimates use printed values | Pre-existing; BF-22 |

### Items with no route yet (wave-C factions) — drives wave B

| Item | Missing |
|---|---|
| sardakk `vpw` | add-hit hook after ground rolls (BF-00c) |
| `sardakk_mech` | ground sustain emits no event |
| `exo2`, `sardakk_dreadnought2` | after-space-combat-round event; destroy-chosen-ships API; Direct Hit immunity hook |
| `sardakkcommander`, `sardakkhero` | commit ground forces from adjacent systems; invasion entry point |
| `sardakkagent` | end-of-tactical-action event for any player + placement |
| `tekklar` | combat-scoped modifier readable by `unit_roll_modifier` |
| `sardakkbt` | ground-combat-won event; research-unit-upgrade API |
| `blood_ties` | custodians cost hook |
| `reclamation`, `winnu_mech` | link control gained ↔ tactical action |
| `lgf` | wormhole adjacency (BF-00e) |
| `htp`, `winnuagent` | production cost / planet value hook |
| `winnuhero` | perform-primary API |
| `winnubt` | move-value hook |
| `mitosis`, `bio`, `arborec_mech` | typed status-phase events (`game.rs`, blocked by another session's uncommitted edits) + placement |
| `lw2`, `arborec_infantry2`, `yinagent` | ground-force destroyed event |
| `arborec_flagship`, `arboreccommander`, `arborechero` | ability-driven production entry point |
| `stymie` | place-command-token API |
| `indoctrination`, `greyfire`, `yin_mech` | per-planet start-of-ground-combat event |
| `devotion`, `ic`, `ambush` | pre-combat / after-round hit production (BF-00c) |
| `yso` | after-production event |
| `yinhero` | invasion entry point |
| `yinbt` | alliance grant |
| `yincommander` | "faction ability used" record |
| `pillage`, `mentakagent` | trade-goods-gained event |
| `mc` | trade-good spend value hook |
| `mentak_flagship`, `mentak_mech` | sustain-prohibition hook (space + ground) |
| `mentak_cruiser3`, `mentakbt`, `yssaril_flagship` | move-through-ships hook |
| `so` | space-combat-ended event |
| `mentakhero` | capture (BF-00d) |
| `crafty` | action-card limit hook |
| `scheming` | action-card draw event |
| `stall_tactics`, `yssaril_mech` | BF-00h |
| `tp` | action-card play-legality hook |
| `mi`, `spynet`, `yssarilcommander`, `yssarilhero` | hidden-hand typed view (BF-00h) |
| `yssarilagent` | agent-text copying |
| `yssarilbt` | transaction-limit hook |

Routable now: `winnu_flagship`, `sardakk_flagship`, `winnucommander`, `acq`, `pop`, `arborecagent`,
`arborecbt`, `mentakcommander`, `yin_flagship` (needs destroy API).
