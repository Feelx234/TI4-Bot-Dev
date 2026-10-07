# BF-01 + factions wave C/D (Winnu, Mentak, Argent, Yssaril, Creuss, Naalu, Arborec, Saar, Muaat, Naaz)

Coordinator: Claude Code (Opus). Implementers: Sonnet subagents, one file each. Reviews: Opus, four
batches (BF-01 + Winnu/Mentak/Argent; Yssaril/Creuss/Naalu; Arborec/Saar/Naaz + hooks; Muaat). Every
blocker fixed or the item unclaimed; per-faction detail in `BF-<alias>.md` ("Review fixes").

## BF-01 coordinator changes

faction_marks privacy (`ti4_model::view::{mark_visible_to, redact_marks}` used by model/policy
`view_for` and `choice.rs` redaction; `leaks()` checks marks); ACTION_CARD_TAKEN without card id;
waiver path drops an unpayable "yes"; `relocate_ships` refuses unenterable destinations; Rearmament
uses the seat home; game.rs: per-ship path/move value in `begin_one_move` + module bonus in
Gravleash, staged card events announced after component/leader actions and after every
`emit_typed`, `clear_reveals(Action)` at ACTION_COMPLETED, module extra wormholes in
`laws::apply_to_galaxy`, `replay_map_edits` each step (`GameError::MapEdit`), SHIP_MOVED payload
`system`/`origin`/`unit`, typed STRATEGY_PHASE_ENDED / STATUS_PHASE_BEGAN / STATUS_PHASE_ENDED (sync
before status), activation through `activation_options_with(content, sources)`; hooks
`MovementHooks::cannot_activate`, `StrategyHooks::scores_without_home`; Mirror Computing moved onto
the trade_good_worth hook (payment now honours it); decision sites registered from the registry scan.

## Checks on the committed tree

| Command | Result |
|---|---|
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1782 passed, 1 ignored; all integration binaries ok |
| `cargo test -p ti4-model -p ti4-content -q` | ok |
| `cargo check --workspace --all-targets` | ok |
| `LIBTORCH=out/libtorch-2.9.1-cpu cargo test -p ti4-sim -q` | 51 passed — behaviour unchanged |
| clippy (ti4-engine) | no new warnings |

## Ledger (claimed / assets)

arborec 9/12, argent 6/13, ghost 10/12, mentak 8/12, muaat 7/13, naalu 6/13, naaz 6/13, saar 3/13,
sardakk 10/12, winnu 10/11, yin 8/11, yssaril 9/12 — **92 / 145**.

## Needs operator approval (changes in-scope play → ti4-sim re-baseline)

1. Typed SHIP_MOVED for ships moving without cargo (game.rs no-cargo branch emits only a label).
   Unblocks Naalu Foresight, Muaat hero, Arborec Stymie/others; also fixes two existing reaction
   cards that miss those moves. Also needs a "movement finished" window for once-per-move effects.
2. Typed SYSTEM_ACTIVATED for the strategy-card free tactical action.

## Shared follow-ups (coordinator packages)

GROUND_FORCE_DESTROYED / PLANET_CONTROL_GAINED / TRADE_GOODS_GAINED from all destroy/control/gain
sites (Letani Warrior II, Scavenge, Pillage); status-phase + agenda action-card draws through
`action_cards::draw` (Scheming); leader-effect destruction announcements; UNITS_PRODUCED from
ability production in leaders; explore hooks (Naaz); flip_form call sites (Naaz mech); supernova
"through" vs "into" (Gashlai); extra-production / cost hooks (Creuss/Muaat/Argent breakthroughs);
mobile-dock movement and blockade (Saar); hook sources/galaxy for component/leader offers;
TURN_PASSED for the final pass; leaders::use_leader_text (Ssruu); transactions limit call site.

## f77a0347 review resolution (2026-10-03)

| Finding | Resolution |
|---|---|
| `yinhero` overclaimed (no GROUND_COMBAT_STARTED/destroyed events, one rival only) | Unclaimed; Yin missing = `yincommander`, `yinhero`, `yinbt` |
| `yincommander` waiver chooses infantry for the player | Unclaimed until the player chooses |
| `yssarilagent` copies only shared-code ACTION agents | Unclaimed |
| TG gains not announced: Psychoarchaeology, Suffi An, Mentak after-combat, Muaat strategy-token | Now `gain_trade_goods_staged` |
| `announce_gains` wrote `#seen` marks before emitting | Marks written only after every emit succeeds |
| Unconventional Measures drops `draw_announced` error | Rejected: `resolve_with` returns `Effect`, the only error is an illegal decider answer, same convention as Archived Secret; propagation needs an `Effect` signature change outside this batch |
| Pillage double trigger / late flush; Politics Rider `rider_payoff` bypasses `draw` (Scheming) | Open; `pillage`, `mentakagent` and Yssaril Scheming stay unclaimed. Assigned to the Mentak follow-up |

Checks: `cargo test -p ti4-engine` — lib 1901 passed, 1 ignored; integration binaries all ok. Clippy: no new warnings in touched code. Ledger: yin 8/11, yssaril 9/12, naalu 10/13, mentak 9/12, muaat 11/13.
