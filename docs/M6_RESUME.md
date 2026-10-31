# D2 lane M6 (`core-m6-tactical`): the tactical island, resume note (2026-10-31)

Branch `core-m6-tactical`, cut from `core-d2-f-live` (da047471), merged with `core-w-l12` (729c7184; `core-w-l10` was already in it) and
`core-d2-f-live2` (a2c11763). Never pushed; the live branch is untouched. Read `docs/D2_LF_RESUME.md` (the live-loop interface and the recipe) first.

## What is native

`ti4-engine-core/src/driver/windows/tactical/` = `TacticalAction`, the composer of the tactical action. The payload is `state::Tactical`
(`state/tactical.rs`, stages added: `Free`, `Hooks`, `Closing`, `Heading(THead)`, `Aftermath`, `Tail`; they exist only on the live path). `Held` impl,
`Arm::Tactical` registered in `driver/live.rs` (`advance_tactical` / `ask_tactical`; a variant of the generic `advance` because a window that handed
the action to the aftermath stays on the stack under it, as M3's invasion does).

| stage | what runs |
|---|---|
| `Activating` / `Moving` / `Loading` | the leaf `rules::tactical::Machine` (activation, movement step, hold, sail with the rift rolls, arrival event), now with a `live` mode: typed events go to the emission queue (`emit_mirrored`), plain events as they happen (`Machine::{emit, note}`); the dual's machine is unchanged |
| `Free(system)` | the free tactical action (`Machine::free`): `open_free(core, seat, system)` for the strategy-card composers |
| `Hooks` | `hooks.rs`: the Sardakk agent / Winnu activation marks (string stateful abilities the core faction crates have not ported), E-Res Siphons, Ceasefire (skips the movement step); any other hook that asks is the counted gap `ActivationHook` |
| `Closing` | `MOVEMENT_FINISHED`, the `skip_to_commit` hook (Sardakk hero, nested question), the arrival count, the Floating Factory blockade (`rules::tactical::destroy_blockaded_mobile_docks`); Nova Seed is the gap `NovaSeed` |
| `Heading` | `driver::windows::aftermath::Head` (fleet enforcement, then the space cannon offense) with facts computed from the position (`facts.rs`: fleet limit through the hooks, half-weight fighters, the note holdings, the reaching guns); the enforcement question is persisted (`THeadStep::Enforcing`, re-derived from the position); a cannon hit that needs a question is the gap `CannonHits` |
| `Aftermath` | the head ends by writing `Window::Aftermath(state::Aftermath)` above this entry (`stage Fighting`, or `Invading` + `feats_noted` for the Sardakk skip, `event_scoring` from the cannon's feat, `before_combat`, the notes) and returns `Live::Done` with this entry kept on the stack. The aftermath arm (lane M5) runs the chain and closes ITS entry |
| `Tail` | when no `Window::Aftermath` is open: Crown/DET (gap), `TACTICAL_ACTION_ENDED` when `tro_window` (the event no core window emitted), `close_writes`, `TACTICAL_ACTION_COMPLETE`, then `tactical_tail::then` |

Openers: `open_tactical(core, seat)` (`TACTICAL_ACTION_BEGAN`, the notes; `settle_extreme_duress` stays the dispatcher's), `open_free`.

Engine half: `ti4-engine/src/tactical_live.rs` (`complete`: the display facts of the three tactical questions computed from the core: option previews, `movable`,
`fleet_supply`, `rift_exits`, the removal question without context; `gap_text`; the lockstep tests). `live::Cx::complete` now also gets the kernel
(signature change in `live/mod.rs` and `live/details.rs`, one arm added). `reactions_core::set_table_codes` interns `TacticalCodes`
(`kernel/ability.rs`: new `Codes.tactical`, `Unknown::UnportedTactical(TacticalGap)`).

## Contract with the neighbours

* **Aftermath lane (M5, nobody is running it)**: after the head this window parks. The chain (`Fighting` -> feats -> `FightOver` -> `Invading` -> `Producing`)
  and its closing of `Window::Aftermath` are M5's. A fresh `AStage::Invading` with `feats_noted` and no invasion open is the Sardakk skip (`open_at_commit`).
  The pause for `Aftermath.event_scoring` is M5's (`open_occurrence_event_scoring`).
* **Turn flow (lane A2/B)**: the tail ends at `TACTICAL_ACTION_COMPLETE` and the window leaves the stack. `finish_action` (`ACTION_COMPLETED`, the closing
  menu or the next turn) is NOT run here. `Temporal Command Suite`'s staged-event flush is theirs too.
* **Menu dispatch (C3)**: `Dispatch::Tactical` calls `open_tactical` after `settle_extreme_duress`.
* **M3 invasion**: independent (a shared `kernel/ability.rs` hunk: `Codes.tactical` and `Unknown::UnportedTactical` next to M3's `invasion` / `Unported`; keep both).

## Open (in the order I would do it)

1. **Activation hooks that ask** (A2b-bail-02), each a stage of `Hooks` with its question: Spatial Conduit Cylinders (`faction_techs::offer_spatial_conduit`, writes
   `spatial_conduit` and the map links), Scanlink Drone Network (explores: needs the native exploration window), Nullification Field and Starlancer (end the turn),
   Minister of Peace (`laws::offer_discard`), the Dominus Orb (`rules::explore_windows::DominusOrb` exists as a leaf), Mahact (the commander's re-activation lets
   the seat activate a tokened system: the `Activating` question differs, so Mahact at the table is a gap before the question). Ending the turn needs the turn flow's `advance_turn`.
2. **String-registered `SYSTEM_ACTIVATED` abilities the core registry lacks**: the live `Env` registry is the core table; an ability the string registers and the core
   has not ported is SILENTLY missing (the dual bailed on it through `nested_check`). The two I found by the lockstep (Sardakk agent mark, Winnu mark) are written by
   `hooks::marks`. Audit the rest (`ghost.rs` has ~15, Aether Passage, `technology:as`) with a registry comparison per faction; this is the check the faction sweep test cannot make.
3. **Routes** (A2b-bail-07): `Opt::Move.route` and the `routes` detail (`tactical::ship_routes`, rules version 29+) are not computed; a `|rN` answer is refused and the
   question lacks `details.routes` when a ship has more than one route (the lockstep scenarios have one route per ship).
4. **The space cannon offense mid-absorption**: a hit that needs a question (sustain damage, a casualty to choose) is the gap `CannonHits`; persisting it needs `to_state/from_state`
   on `SpaceCannonOffense`, `Absorb`, `SustainOffer`, `AssignHits` (rules/combat, M4's files).
5. **Tail**: the Crown of Emphidia and Dark Energy Tap explorations (no native planet exploration window), Ssruu-borrowed agents, Thunder's Edge Warfare's
   `Then::Secondary` (`Window::StrategySecondaryAfterTactical` -> `StrategySecondary` after `redistribute_after_te_warfare`) and `Then::LeaderStrategy`.
6. Nova Seed (the per-step map replay of `MapEdit` marks is the turn hooks').
7. Rift-loss lockstep (needs the `rng` positions of the string game; the core test `rules::tactical` covers the rolls from the core's own seeds).

## Counted gaps (`TacticalGap`)

`NoProgress`, `ActivationHook`, `NovaSeed`, `CannonHits`, `ExplorationAtTheEnd`, `WarfareSecondary`, `LeaderStrategy`, `SsruuLends`, `NoGalaxy`.
