# D2 lane M6b (`core-m6b-tactical`): finishing the tactical island, resume note (2026-10-31)

Branch `core-m6b-tactical`, cut from `core-m5-aftermath` (b15c8773), then merged with `core-x-explore` (the native planet exploration window; Van Hauge moved to slot 7005 beside
Indoctrination 7003 and Greyfire 7004) and `core-d2-c2-turn` (lane B, the turn flow: `driver::turn`, `Asking.at`, the `End` / `Finish` jobs). Never pushed; the live branch
`nightly-fixes-2026-10-26` is untouched. Read `docs/M6_RESUME.md` (the tactical composer), `docs/M5_RESUME.md` (the aftermath) and `docs/D2_LF_RESUME.md` (the recipe) first.

## What is native now

| item | where | how |
|---|---|---|
| Activation hooks that ask | `ti4-engine-core/src/driver/windows/tactical/hooks.rs` | ONE step in string order (marks, Mahact commander, Spatial Conduit, Scanlink, E-Res, Nullification Field, Starlancer, Minister of Peace, Ceasefire, Dominus Orb); every question goes to the TAPE (`Sx::ask`, the way lane B's jobs ask): the driver rolls the step back at a question and replays it with the answers (`Pending::nested`). New typed questions `choice::tactical::{Ask::Hook, Ask::Scanlink, Opt::Use, Opt::Explore}` and their renderer arms (`render/tactical.rs`) |
| A hook that ends the turn | same | `end_turn`: active system and pending tag cleared, the plain `TURN_ENDED_BY_*`, then lane B's `TurnJob::End` is opened (`driver::turn::open_job`) and the action leaves the stack. The Mahact commander returns both tokens (`TStage::Hooks(returns)`, `Machine::held_before`); `activate::holds_leave` lets the Activating question list a system holding the seat's own token |
| Spatial Conduit links | `kernel/fx_move.rs` `set_conduit_links` / `clear_conduit_links` | written into the map's `extra_links` for the activation, cleared by the tail |
| Scanlink | `hooks.rs` | planet question, then `Exploring` (lane X) run inline; plain `SCANLINK_EXPLORED` |
| Minister of Peace | `hooks.rs`, `live/turn.rs` | `OfferDiscard` child; the offer facts (law text, the activated system and seat) in `live/turn.rs::complete` |
| Routes | `rules/tactical/routes.rs` (`ship_routes`), `tactical_live.rs` (`details.routes`), `live/mod.rs::pick_of` (`|rN`), `TacticalAction::accepts_unlisted` | the string's selection (default, the safe boosted way, one per profile, fill-up) over `MovementRules::all_routes_from_ship`; the same function lists them and resolves an answer; `MovementRules::is_anomalous` is new |
| Space cannon offense hits that ask | `tactical/mod.rs` (`Heading`) | the questions after the enforcement go to the tape; the head is re-derived from the position at every replay (the enforcement is over by then). `TacticalGap::CannonHits` is gone |
| Tail explorations | `tactical/tail.rs` | Crown of Emphidia (`CrownExplore`, deck = the planet's printed type) and Dark Energy Tap (`Exploring::frontier`, plain `FRONTIER_EXPLORED`) inline; `open_finish` after `TACTICAL_ACTION_COMPLETE` (lane B's job) |
| Initiative order | `driver/windows/live_common.rs::driver_of` (used by tactical and invasion), `driver/live.rs::sync_clock` | `Resolver::set_initiative_order(initiative_order(core))` |
| Silent-gap audit | `reactions_core.rs` test `every_system_activated_ability_the_string_registers_is_ported_or_accounted_for` | diffs the string's `SYSTEM_ACTIVATED` abilities (24 factions) against the core table; each of the 27 is `Carried`, `Dropped` or `Gap` |
| `NativeSubject` | `ti4-engine-core-bridge/src/native_subject.rs::open_arm` | `Arm::Tactical` opens `TacticalAction` from the observed `Window::Tactical` payload (not exercised by a test: see Not done) |

## Not done (precise)

(see the end of the lane for the final list)
