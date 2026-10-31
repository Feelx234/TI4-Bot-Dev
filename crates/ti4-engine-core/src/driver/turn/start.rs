//! The start of a turn: the `turn_prelude` block of `Game::step_flow`, in the string's order.
//!
//! 1. the start-of-turn technologies (`technology::start_turn`: Psychoarchaeology, Transit Diodes, Chaos Mapping),
//! 2. the commander unlocks (`Game::refresh_commander_unlocks`: `LEADER_UNLOCKED:<leader>`),
//! 3. Military Support (`promissory::turn_started`; `MILITARY_SUPPORT_USED`),
//! 4. the Spec Ops units waiting on the card (`SPEC_OPS_RETURNED:<seat>:<n>`),
//! 5. the turn is prepared (`prepared_turn_seq`), then the typed `TURN_BEGAN` (the resolver learns the position first).

use super::inline::{emit_typed, gap, halted, player_payload, run, sync_clock, Flow};
use crate::driver::kernel_env::KernelEnv;
use crate::driver::turn_flow;
use crate::generated::{FactionTag, LeaderType, PromissoryNote, Technology, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::fx_strategy;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::TurnGap;
use crate::query::{promissory, reach};
use crate::rules::leaders;
use crate::rules::promissory_windows::{PlaceUnits, Then};
use crate::rules::technology_windows::TurnStart;
use crate::state::{EventKind, LeaderStatus, Phase, PlainArg, PlainKind, PlainRec};

/// The unit a Sol seat's Spec Ops return places (`faction_techs::return_spec_ops` names `sol_infantry2`).
const SPEC_OPS_UNIT: Unit = Unit::SpecOpsII;

/// The factions whose commander unlock condition the string states (`factions::<faction>::commander_unlocked`) and no hook of the
/// core does yet (`commander_unlocked`: lane F2's list, `docs/D2_LF2_RESUME.md`). A locked commander of one of them cannot be judged.
const COMMANDER_HOOK_MISSING: [FactionTag; 12] = [
    FactionTag::Arborec,
    FactionTag::Argent,
    FactionTag::Cabal,
    FactionTag::Empyrean,
    FactionTag::Ghost,
    FactionTag::Mahact,
    FactionTag::Mentak,
    FactionTag::Muaat,
    FactionTag::Nekro,
    FactionTag::Nomad,
    FactionTag::Saar,
    FactionTag::Winnu,
];

/// `technology::start_turn`.
fn technologies(sx: &mut Sx<'_>, active: Seat) -> Flow {
    if reach::technology_text(sx.state, active, Technology::ChaosMapping) {
        // The single-unit production needs the production facts (`buildable`, the capacity of the seat's systems): the production lane's.
        gap(sx, TurnGap::ChaosMapping);
        return Flow::Halt;
    }
    run(sx, &mut TurnStart::new(active, sx.table.codes.turn.psychoarchaeology))
}

/// The leaders `leaders::check_unlocks` would unlock for `seat` now, or `None` after noting the gap when a locked commander of a faction whose
/// condition no hook states yet could unlock (`COMMANDER_HOOK_MISSING`). `mapless`: the check of `objectives::award_tail`, which has no map
/// (Naalu's commander asks about the Mecatol area; the status phase checks again with one).
pub(crate) fn unlockable(sx: &Sx<'_>, seat: Seat, mapless: bool) -> Option<leaders::Leaders> {
    let env = KernelEnv::new(sx.state, sx.kernel);
    let lcx = leaders::Cx { core: sx.state, env: &env, scope: reach::scope_of(sx.kernel), blanked: reach::blanked(sx.state) };
    let locked_without_a_hook = sx.state.players().by_seat(seat).is_some_and(|p| {
        p.leaders.status.iter().any(|(leader, status)| {
            *status == LeaderStatus::Locked
                && leader.data().kind == LeaderType::Commander
                && COMMANDER_HOOK_MISSING.contains(&leader.data().faction)
                && leaders::commander_unlocked(&lcx, seat, *leader).is_none()
        })
    });
    if locked_without_a_hook {
        gap(sx, TurnGap::CommanderUnlock);
        return None;
    }
    let mut list = leaders::unlockable(&lcx, seat);
    if mapless {
        // Naalu's commander (Maban): no map, no Mecatol area, so it stays locked.
        list.retain(|l| *l != crate::generated::Leader::Maban);
    }
    Some(list)
}

/// `Game::refresh_commander_unlocks`.
fn commanders(sx: &mut Sx<'_>, active: Seat) {
    let Some(unlocking) = unlockable(sx, active, false) else { return };
    leaders::unlock(sx.state, active, unlocking.as_slice());
    for leader in unlocking.as_slice() {
        sx.plain(PlainRec::one(PlainKind::LeaderUnlocked, PlainArg::Leader(*leader)));
    }
}

/// `promissory::turn_started` (Military Support): "At the start of the Sol player's turn: ... the holder may place 2 infantry ... Then,
/// return this card to the Sol player."
fn military_support(sx: &mut Sx<'_>, active: Seat) -> Flow {
    let Some(holder) = promissory::holder_of(sx.state, PromissoryNote::MilitarySupport, active) else { return Flow::Done };
    let Some(roster) = sx.table.facts.plays.as_ref().and_then(|p| p.roster) else {
        gap(sx, TurnGap::NoRoster);
        return Flow::Halt;
    };
    let scope = reach::scope_of(sx.kernel);
    let form = fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, holder, UnitBaseType::Infantry)
        .filter(|unit| !sx.kernel.hooks.economy.effect_placement_forbidden(sx.state, sx.kernel, holder, *unit));
    let codes = sx.table.codes.turn;
    let mut window = PlaceUnits::new(holder, form, 2, None, true, scope, codes.military_support, Then::MilitarySupport { owner: active, reason: codes.military_support_reason });
    if run(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    sx.plain(PlainRec::bare(PlainKind::MilitarySupportUsed));
    Flow::Done
}

/// The prelude job.
pub fn run_prelude(sx: &mut Sx<'_>) -> Flow {
    let Some(active) = sx.state.meta().active else { return Flow::Done };
    if sx.state.meta().phase != Phase::Action {
        return Flow::Done;
    }
    if technologies(sx, active) == Flow::Halt {
        return Flow::Halt;
    }
    commanders(sx, active);
    if halted(sx) || military_support(sx, active) == Flow::Halt {
        return Flow::Halt;
    }
    let returned = turn_flow::return_spec_ops(sx.state, active, SPEC_OPS_UNIT);
    if returned > 0 {
        sx.plain(PlainRec::two(PlainKind::SpecOpsReturned, PlainArg::Seat(active), PlainArg::Count(u16::try_from(returned).unwrap_or(u16::MAX))));
    }
    turn_flow::mark_prepared(sx.state);
    // "At the start of another player's turn": typed so a window can hang off it, the marker lands before the seat has chosen anything.
    sync_clock(sx);
    emit_typed(sx, EventKind::TurnBegan, player_payload(active))
}
