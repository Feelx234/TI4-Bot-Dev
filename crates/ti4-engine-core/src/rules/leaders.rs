//! Leaders: status, kinds, unlock checks and the action-phase offer (package B7; LRR 51).
//! The reads of `leaders.rs`: `status`, `of_kind`, `usable`, `uses_the_action`, `commander_unlocked`,
//! the candidate lists of `check_unlocks` and `component_actions`, `combines_planet_values`, and
//! `promissory::has_commander_ability`.
//!
//! Everything the core cannot see yet is asked of an [`Env`]: the faction modules' answers
//! (`HookTable::commander_unlocked`, `leader_action`, `bastion_units_resource_bonus`,
//! `mahact_imperia_grants`: the slots exist on the table, but its functions cannot read the string
//! state the dual run lends from, so the string engine implements this trait with the same calls)
//! and the few rules that need the galaxy or the content store. The mutations (`ready`, `exhaust`,
//! `purge`, `use_leader`) stay with the string engine until the driver lands; the unlock check is split
//! into [`unlockable`] (here, a pure read) and the status writes (adapter).

use crate::containers::SmallList;
use crate::generated::{Leader, LeaderType, LeaderWindow, Planet, System, Unit, UnitBaseType};
use crate::state::Piece;
use crate::ids::Seat;
use crate::query::board;
use crate::query::economy::{self, Spend};
use crate::state::{CoreState, Feat, LeaderStatus};
use crate::tables::Scope;

pub use crate::query::leaders::{has_commander_ability_in, has_commander_ability_with, imperia_from_pool, status};
use crate::SysSet;

/// `leaders::HERO_OBJECTIVES`: 51.7, a hero unlocks once its owner has scored three objectives.
pub const HERO_OBJECTIVES: usize = 3;
/// `ti4_model::state::RULES_FACTION_EFFECTS`: Watchful Ojz is checked from this rules version.
pub const RULES_FACTION_EFFECTS: u32 = 16;

/// What the leader rules ask of the factions and of rules not yet in the core.
pub trait Env {
    /// `factions::commander_unlocked` (`HookTable::commander_unlocked`): `None` if no module owns the leader.
    fn faction_commander_unlocked(&self, who: Seat, leader: Leader) -> Option<bool>;
    /// `bastion_units::resource_bonus` (`HookTable::bastion_units_resource_bonus`): the Helios dock term.
    fn resource_bonus(&self, planet: Planet) -> i64;
    /// Mecatol Rex and the systems next to it on this galaxy; `None` without a map.
    fn mecatol_area(&self) -> Option<SysSet>;
    /// `factions::leader_action` (`HookTable::leader_action`): a faction's own action leader, and whether it resolves.
    fn leader_action(&self, who: Seat, leader: Leader) -> Option<bool>;
    /// `mahact::imperia_grants` (`HookTable::mahact_imperia_grants`).
    fn imperia_grants(&self, who: Seat, commander: Leader) -> bool;
    /// The arms of `can_resolve_action` that read the content store, the map or the supply (Z'eu,
    /// Doctor Sucaban's swap, the Thunder's Edge Xxcha hero): whether `leader` can resolve now.
    fn other_action_resolves(&self, who: Seat, leader: Leader) -> bool;
}

/// What the rules read besides the state: the source scope, the blanked-laws flag and the [`Env`].
#[derive(Clone, Copy)]
pub struct Cx<'a> {
    pub core: &'a CoreState,
    pub scope: Scope,
    pub blanked: bool,
    pub env: &'a dyn Env,
}

/// A player's leaders (at most the faction's three, plus what cards give).
pub type Leaders = SmallList<Leader, 6>;

/// `leaders::kind_of`.
#[must_use]
pub fn kind(leader: Leader) -> LeaderType {
    leader.data().kind
}

/// `leaders::uses_the_action`: a component action that takes the turn (`ACTION:`).
#[must_use]
pub fn uses_the_action(leader: Leader) -> bool {
    leader.data().window == LeaderWindow::Action
}

/// `leaders::is_action_window`: printed for the action phase (`Action` or `ActionPhase`).
#[must_use]
pub fn is_action_window(leader: Leader) -> bool {
    matches!(leader.data().window, LeaderWindow::Action | LeaderWindow::ActionPhase)
}

/// `leaders::of_kind`: the player's leaders of one kind, in leader order.
#[must_use]
pub fn of_kind(core: &CoreState, who: Seat, wanted: LeaderType) -> Leaders {
    let mut out = Leaders::new();
    if let Some(p) = core.players().by_seat(who) {
        for leader in p.leaders.status.keys() {
            if kind(leader) == wanted {
                out.push(leader);
            }
        }
    }
    out
}

/// A readied agent or an unlocked hero: what can be used at all.
fn usable_now(leader: Leader, status: LeaderStatus) -> bool {
    matches!((status, kind(leader)), (LeaderStatus::Readied, LeaderType::Agent) | (LeaderStatus::Unlocked, LeaderType::Hero))
}

/// `leaders::usable`: readied agents and unlocked heroes (commanders are never "usable").
#[must_use]
pub fn usable(core: &CoreState, who: Seat) -> Leaders {
    let mut out = Leaders::new();
    if let Some(p) = core.players().by_seat(who) {
        for (leader, status) in p.leaders.status.iter() {
            if usable_now(*leader, *status) {
                out.push(*leader);
            }
        }
    }
    out
}

/// `leaders::combines_planet_values` (Xxekir Grom): the unlocked hero is a passive.
#[must_use]
pub fn combines_planet_values(core: &CoreState, who: Seat) -> bool {
    status(core, who, Leader::XxekirGrom) == Some(LeaderStatus::Unlocked)
}

/// `promissory::has_commander_ability`: the player's own unlocked commander or a durable grant, Mahact's
/// Imperia, or a faceup Alliance whose owner has this commander unlocked.
#[must_use]
pub fn has_commander_ability(cx: &Cx<'_>, who: Seat, commander: Leader) -> bool {
    has_commander_ability_with(cx.core, who, commander, || cx.env.imperia_grants(who, commander))
}

/// `leaders::controlled_total`: resources or influence of everything the seat controls.
fn controlled_total(cx: &Cx<'_>, who: Seat, spend: Spend) -> i64 {
    board::controlled_planets(cx.core, who)
        .as_slice()
        .iter()
        .map(|(_, p)| {
            let extra = if spend == Spend::Resources { cx.env.resource_bonus(*p) } else { 0 };
            economy::value_now(cx.core, cx.scope, cx.blanked, *p, spend, extra)
        })
        .sum()
}

fn units_in_space<'a>(core: &'a CoreState, who: Seat) -> impl Iterator<Item = (crate::generated::System, crate::state::Piece)> + 'a {
    core.board().systems().flat_map(move |(sys, cell)| cell.units.as_slice().iter().filter(move |u| u.owner() == who).map(move |u| (sys, *u)))
}

/// `UnitType::is_ground_force`: by base type, and the Titans' PDS (a structure that is a ground force).
fn is_ground_force(unit: Unit) -> bool {
    matches!(unit.data().base_type, UnitBaseType::Infantry | UnitBaseType::Mech) || matches!(unit, Unit::HelTitanI | Unit::HelTitanII)
}

/// `leaders::commander_unlocked`: whether this commander's unlock condition is met; `None` for one
/// with no registered check (it stays locked).
#[must_use]
pub fn commander_unlocked(cx: &Cx<'_>, who: Seat, leader: Leader) -> Option<bool> {
    let core = cx.core;
    let player = core.players().by_seat(who);
    let met = match leader {
        Leader::ClaireGibson => controlled_total(cx, who, Spend::Resources) >= 12,
        Leader::ElderQanoj => controlled_total(cx, who, Spend::Influence) >= 12,
        Leader::GilaTheSilvertongue => player.is_some_and(|p| p.econ.trade_goods >= 10),
        Leader::AgnlanOln => player.is_some_and(|p| p.tech.owned.len() >= 8),
        Leader::L1z1xcommander => {
            units_in_space(core, who).filter(|(_, u)| u.kind().data().base_type == UnitBaseType::Dreadnought).count() >= 4
        }
        // Five non-fighter ships in *one* system.
        Leader::RearAdmiralFarran => core.board().systems().any(|(_, cell)| {
            cell.units
                .as_slice()
                .iter()
                .filter(|u| u.owner() == who)
                .filter(|u| {
                    let d = u.kind().data();
                    d.ship && d.base_type != UnitBaseType::Fighter
                })
                .count()
                >= 5
        }),
        Leader::Maban => {
            let Some(area) = cx.env.mecatol_area() else {
                return Some(false);
            };
            area.iter().any(|i| {
                let Some(sys) = crate::generated::System::from_index(i) else { return false };
                board::space_units(core, sys).iter().any(|u| u.owner() == who && is_ground_force(u.kind()))
                    || board::planets_of(core, sys)
                        .any(|pl| board::planet_units(core, pl).iter().any(|u| u.owner() == who && is_ground_force(u.kind())))
            })
        }
        Leader::WatchfulOjz if core.meta().rules_version >= RULES_FACTION_EFFECTS => {
            player.is_some_and(|p| p.misc.event_feats.as_slice().iter().any(|(f, _)| *f == Feat::LastToPass))
        }
        other => return cx.env.faction_commander_unlocked(who, other),
    };
    Some(met)
}

/// How many objectives the seat has scored (`GameState::scored_by(..).len()`).
#[must_use]
pub fn scored_count(core: &CoreState, who: Seat) -> usize {
    let set = core.objectives().scored[usize::from(who.0)];
    set.public.len() + set.secret.len()
}

/// `leaders::check_unlocks` without the writes: the leaders that unlock now, commanders (those whose
/// condition holds) first, then the locked heroes when three objectives are scored.
#[must_use]
pub fn unlockable(cx: &Cx<'_>, who: Seat) -> Leaders {
    let mut out = Leaders::new();
    let Some(p) = cx.core.players().by_seat(who) else { return out };
    for (leader, status) in p.leaders.status.iter() {
        if *status == LeaderStatus::Locked && commander_unlocked(cx, who, *leader).unwrap_or(false) {
            out.push(*leader);
        }
    }
    let commanders = out.len();
    if scored_count(cx.core, who) >= HERO_OBJECTIVES {
        for (leader, status) in p.leaders.status.iter() {
            if *status == LeaderStatus::Locked && kind(*leader) == LeaderType::Hero && !out.as_slice()[..commanders].contains(leader) {
                out.push(*leader);
            }
        }
    }
    out
}

// -- status writes (the mutation half) -----------------------------------------------------------

/// Set a leader the player holds to `to` when `from` accepts its status; `false` otherwise.
fn move_status(core: &mut CoreState, who: Seat, leader: Leader, from: impl Fn(LeaderStatus) -> bool, to: LeaderStatus) -> bool {
    let Some(p) = core.players_mut().by_seat_mut(who) else { return false };
    match p.leaders.status.get_mut(&leader) {
        Some(status) if from(*status) => {
            *status = to;
            true
        }
        _ => false,
    }
}

/// `leaders::ready_all`: every exhausted leader of the player becomes readied; returns them in leader order.
pub fn ready_all(core: &mut CoreState, who: Seat) -> Leaders {
    let mut out = Leaders::new();
    let Some(p) = core.players_mut().by_seat_mut(who) else { return out };
    for leader in p.leaders.status.keys().collect::<Leaders>().as_slice() {
        if p.leaders.status.get(leader) == Some(&LeaderStatus::Exhausted) {
            p.leaders.status.insert(*leader, LeaderStatus::Readied);
            out.push(*leader);
        }
    }
    out
}

/// `leaders::ready`: ready one exhausted leader; `false` when it was not exhausted.
pub fn ready(core: &mut CoreState, who: Seat, leader: Leader) -> bool {
    move_status(core, who, leader, |s| s == LeaderStatus::Exhausted, LeaderStatus::Readied)
}

/// `leaders::exhaust`: exhaust a readied leader; `false` when it was not readied.
pub fn exhaust(core: &mut CoreState, who: Seat, leader: Leader) -> bool {
    move_status(core, who, leader, |s| s == LeaderStatus::Readied, LeaderStatus::Exhausted)
}

/// `leaders::purge`: purge a leader the player holds (permanent); `false` when it holds none such.
pub fn purge(core: &mut CoreState, who: Seat, leader: Leader) -> bool {
    move_status(core, who, leader, |_| true, LeaderStatus::Purged)
}

/// The write of `leaders::check_unlocks`: the given leaders become unlocked (the list is [`unlockable`]'s).
pub fn unlock(core: &mut CoreState, who: Seat, leaders: &[Leader]) {
    let Some(p) = core.players_mut().by_seat_mut(who) else { return };
    for leader in leaders {
        p.leaders.status.insert(*leader, LeaderStatus::Unlocked);
    }
}

// -- the effects of leaders that ask nothing (`dispatch_leader` arms without a decision) -----------

/// Evelyn DeLouis / Viscount Unlenn: one unit rolls an extra die this combat round (`unit` is the base type
/// the caller resolved for the seat). Held as the round number, so it expires with the round.
pub fn extra_die_agent(core: &mut CoreState, who: Seat, unit: Unit) -> bool {
    let round = core.meta().combat_round_seq;
    let Some(p) = core.players_mut().by_seat_mut(who) else { return false };
    p.combat_timers.extra_die_round = crate::Opt32::some(round);
    p.combat_timers.extra_die_unit = Some(unit);
    true
}

/// Darktalon Treilla: fleet supply is unlimited until the end of this round; `false` when already active.
pub fn activate_letnev_hero(core: &mut CoreState, who: Seat) -> bool {
    let round = core.meta().round;
    let Some(p) = core.players_mut().by_seat_mut(who) else { return false };
    if p.production_timers.fleet_supply_unlimited_until.is(round) {
        return false;
    }
    p.production_timers.fleet_supply_unlimited_until = crate::Opt32::some(round);
    true
}

/// Hacan's hero: production of this sequence is free.
pub fn activate_hacan_hero(core: &mut CoreState, who: Seat) -> bool {
    let seq = core.meta().production_seq;
    let Some(p) = core.players_mut().by_seat_mut(who) else { return false };
    p.production_timers.free_production_use = crate::Opt32::some(seq);
    true
}

/// Jace X. 4th Air Legion (`solhero`): every command token of the seat is returned to its reinforcements.
pub fn sol_hero(core: &mut CoreState, who: Seat) -> bool {
    for system in System::ALL {
        if core.board().sys(system).cmd.contains(who) {
            core.board_mut().sys_mut(system).cmd.remove(who);
        }
    }
    true
}

/// Ahk-Syl Siven (`nomadhero`): the effect runs through this game round, held as a mark naming the round. `false` (nothing
/// written) when it is already running or the Nomad has no flagship on the board.
pub fn nomad_hero(core: &mut CoreState, who: Seat) -> bool {
    use crate::keys::{Atom, MarkKey, MarkVal};
    let round = core.meta().round;
    let running = matches!(core.marks().tail.get(&MarkKey::NomadHero(who)), Some(MarkVal::Atom(Atom::Num(r))) if *r == round);
    let flagship = core.board().systems().any(|(_, cell)| cell.units.as_slice().iter().any(|u| u.owner() == who && u.kind().data().base_type == crate::generated::UnitBaseType::Flagship));
    if running || !flagship {
        return false;
    }
    core.marks_mut().tail.edit().insert(MarkKey::NomadHero(who), MarkVal::Atom(Atom::Num(round)));
    true
}

/// Ul the Progenitor (`titanshero`, Geoform): Elysium is readied and wears the card. `false` (nothing written) when Elysium is
/// not in play (controlled by somebody) or already wears Geoform.
pub fn titans_hero(core: &mut CoreState) -> bool {
    use crate::generated::{Attachment, Planet};
    let available = core.board().planet(Planet::Elysium).control.is_some() && !board::attachments(core, Planet::Elysium).as_slice().contains(&Attachment::Geoform);
    if !available {
        return false;
    }
    crate::kernel::fx::ready_planet(core, Planet::Elysium);
    let cell = core.board_mut().planet_mut(Planet::Elysium);
    cell.flags.set(crate::state::PlanetFlags::ATTACH_ENTRY, true);
    cell.attach.push(Attachment::Geoform);
    true
}

/// I48S (`l1z1xagent`): one infantry of `target` on a planet of `system` becomes `mech`. The first planet (in
/// planet order, the string map's key order) holding such an infantry is the one; the new unit is fresh (no
/// damage). Whether a swap happened. As in the string arm the system's entry exists afterwards either way.
pub fn l1z1x_agent(core: &mut CoreState, scope: Scope, target: Seat, system: System, mech: Unit) -> bool {
    core.board_mut().sys_mut(system).flags.set(crate::state::SysFlags::TOUCHED, true);
    let mut planets: SmallList<Planet, 8> = board::planets_of(core, system)
        .filter(|p| core.board().planet(*p).flags.has(crate::state::PlanetFlags::UNITS_ENTRY))
        .collect();
    planets.as_mut_slice().sort_unstable();
    let infantry = |u: &Piece| u.owner() == target && scope.unit(u.kind()).is_some_and(|d| d.base_type == UnitBaseType::Infantry);
    for planet in planets.as_slice() {
        let list = &mut core.board_mut().planet_mut(*planet).units;
        if let Some(at) = list.as_slice().iter().position(infantry) {
            list.as_mut_slice()[at] = Piece::new(mech, target);
            crate::kernel::fx::stage_naaz_mech_placed(core, target, system, mech);
            return true;
        }
    }
    false
}

/// Claire Gibson (`solcommander`): `count` of `unit` (what the seat's cards make of "infantry", resolved by the
/// caller together with the supply) onto the planet.
pub fn sol_commander(core: &mut CoreState, who: Seat, system: System, planet: Planet, unit: Unit, count: usize) -> bool {
    for _ in 0..count {
        crate::kernel::fx::place_unit(core, who, system, Some(planet), unit);
    }
    true
}

/// `leaders::end_of_round`: Darktalon Treilla's effect ends with the round she was used in; she is purged.
/// Returns the seats whose hero was purged.
pub fn end_of_round(core: &mut CoreState) -> SmallList<Seat, 8> {
    let round = core.meta().round;
    let mut purged = SmallList::new();
    let seats: SmallList<Seat, 8> = core.players().iter().map(|p| p.seat).collect();
    for seat in seats.as_slice() {
        let Some(p) = core.players_mut().by_seat_mut(*seat) else { continue };
        if p.production_timers.fleet_supply_unlimited_until.is(round) {
            p.production_timers.fleet_supply_unlimited_until = crate::Opt32::NONE;
            if p.leaders.status.get(&Leader::DarktalonTreilla) == Some(&LeaderStatus::Unlocked) {
                p.leaders.status.insert(Leader::DarktalonTreilla, LeaderStatus::Purged);
                purged.push(*seat);
            }
        }
    }
    purged
}

/// `leaders::can_resolve_action`: the arms the core can decide itself; the rest is asked of the [`Env`].
#[must_use]
pub fn can_resolve_action(cx: &Cx<'_>, who: Seat, leader: Leader) -> bool {
    let core = cx.core;
    match leader {
        // "Ready any planet" needs a planet to be readying.
        Leader::GgrocutoRinn => core.board().planets().any(|(_, c)| c.flags.has(crate::state::PlanetFlags::EXHAUSTED)),
        // Offering a use that burns the agent for nothing is a dead button.
        Leader::CarthOfGoldenSands => core.players().iter().any(|p| i16::from(p.faction.data().commodities) > p.econ.commodities),
        // Already active this round: re-using it would change nothing.
        Leader::DarktalonTreilla => core
            .players()
            .by_seat(who)
            .is_some_and(|p| p.production_timers.fleet_supply_unlimited_until.get() != Some(core.meta().round)),
        Leader::Zeu | Leader::RinTheMastersLegacy | Leader::XxekirGromTe => cx.env.other_action_resolves(who, leader),
        other => cx.env.leader_action(who, other).unwrap_or(true),
    }
}

/// The own leaders of `who` that `component_actions` offers, in leader order: readied agents and
/// unlocked heroes printed for the action phase, delivered (`delivered`: the string list of leaders the
/// engine resolves itself) or owned by a faction module, and able to resolve.
#[must_use]
pub fn action_candidates(cx: &Cx<'_>, who: Seat, delivered: impl Fn(Leader) -> bool) -> Leaders {
    let mut out = Leaders::new();
    let Some(p) = cx.core.players().by_seat(who) else { return out };
    for (leader, status) in p.leaders.status.iter() {
        let leader = *leader;
        if usable_now(leader, *status)
            && is_action_window(leader)
            && (delivered(leader) || cx.env.leader_action(who, leader).is_some())
            && can_resolve_action(cx, who, leader)
        {
            out.push(leader);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::Faction;
    use crate::state::{Player, Players};

    struct NoFactions;
    impl Env for NoFactions {
        fn faction_commander_unlocked(&self, _: Seat, _: Leader) -> Option<bool> {
            None
        }
        fn resource_bonus(&self, _: Planet) -> i64 {
            0
        }
        fn mecatol_area(&self) -> Option<SysSet> {
            None
        }
        fn leader_action(&self, _: Seat, _: Leader) -> Option<bool> {
            None
        }
        fn imperia_grants(&self, _: Seat, _: Leader) -> bool {
            false
        }
        fn other_action_resolves(&self, _: Seat, _: Leader) -> bool {
            false
        }
    }

    fn core_with(leaders: &[(Leader, LeaderStatus)]) -> CoreState {
        let mut core = CoreState::all_loaded();
        let mut p = Player::new(Seat(0), Faction::TheArborec);
        for (l, s) in leaders {
            p.leaders.status.insert(*l, *s);
        }
        core.players = Players::from_vec(vec![p]);
        core
    }

    #[test]
    fn usable_is_readied_agents_and_unlocked_heroes() {
        let core = core_with(&[
            (Leader::LetaniOspha, LeaderStatus::Readied),
            (Leader::DirzugaRophal, LeaderStatus::Unlocked),
            (Leader::LetaniMiasmiala, LeaderStatus::Locked),
        ]);
        assert_eq!(usable(&core, Seat(0)).as_slice(), &[Leader::LetaniOspha]);
        assert_eq!(of_kind(&core, Seat(0), LeaderType::Commander).as_slice(), &[Leader::DirzugaRophal]);
    }

    #[test]
    fn a_hero_unlocks_with_three_scored_objectives_and_not_before() {
        let mut core = core_with(&[(Leader::LetaniMiasmiala, LeaderStatus::Locked)]);
        let env = NoFactions;
        let cx = Cx { core: &core, scope: Scope(u32::MAX), blanked: false, env: &env };
        assert!(unlockable(&cx, Seat(0)).is_empty());
        let objectives = core.objectives_mut();
        for o in crate::generated::PublicObjective::ALL.iter().take(3) {
            objectives.scored[0].public.insert(*o);
        }
        let cx = Cx { core: &core, scope: Scope(u32::MAX), blanked: false, env: &env };
        assert_eq!(unlockable(&cx, Seat(0)).as_slice(), &[Leader::LetaniMiasmiala]);
    }

    #[test]
    fn a_commander_without_a_registered_check_stays_locked() {
        let core = core_with(&[(Leader::DirzugaRophal, LeaderStatus::Locked)]);
        let env = NoFactions;
        let cx = Cx { core: &core, scope: Scope(u32::MAX), blanked: false, env: &env };
        assert_eq!(commander_unlocked(&cx, Seat(0), Leader::DirzugaRophal), None);
        assert!(unlockable(&cx, Seat(0)).is_empty());
    }

    #[test]
    fn status_writes_follow_the_string_rules() {
        let mut core = core_with(&[
            (Leader::LetaniOspha, LeaderStatus::Readied),
            (Leader::DirzugaRophal, LeaderStatus::Locked),
            (Leader::LetaniMiasmiala, LeaderStatus::Exhausted),
        ]);
        let me = Seat(0);
        assert!(!ready(&mut core, me, Leader::LetaniOspha), "only an exhausted leader readies");
        assert!(exhaust(&mut core, me, Leader::LetaniOspha));
        assert!(!exhaust(&mut core, me, Leader::LetaniOspha), "only a readied leader exhausts");
        assert_eq!(ready_all(&mut core, me).as_slice().len(), 2);
        assert_eq!(status(&core, me, Leader::LetaniMiasmiala), Some(LeaderStatus::Readied));
        assert!(!purge(&mut core, Seat(1), Leader::LetaniOspha));
        assert!(purge(&mut core, me, Leader::DirzugaRophal));
        assert_eq!(status(&core, me, Leader::DirzugaRophal), Some(LeaderStatus::Purged));
        unlock(&mut core, me, &[Leader::LetaniMiasmiala]);
        assert_eq!(status(&core, me, Leader::LetaniMiasmiala), Some(LeaderStatus::Unlocked));
    }
    #[test]
    fn jace_returns_every_command_token_of_the_seat_and_only_theirs() {
        let mut core = core_with(&[]);
        let (a, b) = (System::ALL[3], System::ALL[40]);
        core.board_mut().sys_mut(a).cmd.insert(Seat(0));
        core.board_mut().sys_mut(a).cmd.insert(Seat(1));
        core.board_mut().sys_mut(b).cmd.insert(Seat(0));
        assert!(sol_hero(&mut core, Seat(0)));
        assert!(!core.board().sys(a).cmd.contains(Seat(0)) && !core.board().sys(b).cmd.contains(Seat(0)));
        assert!(core.board().sys(a).cmd.contains(Seat(1)), "another seat's token stays");
    }

    #[test]
    fn i48s_swaps_the_first_infantry_in_planet_order_for_a_fresh_mech_and_touches_the_system() {
        let mut core = core_with(&[]);
        let scope = Scope(u32::MAX);
        let system = System::ALL.iter().copied().find(|s| s.data().planets.len() >= 2).expect("a two-planet system");
        let (first, second) = (system.data().planets[0], system.data().planets[1]);
        let (inf, mech) = (Unit::InfantryI, Unit::FactionlessMech);
        crate::kernel::fx::place_unit(&mut core, Seat(0), system, Some(second), inf);
        assert!(l1z1x_agent(&mut core, scope, Seat(0), system, mech));
        assert_eq!(core.board().planet(second).units.as_slice()[0].kind(), mech);
        assert!(!l1z1x_agent(&mut core, scope, Seat(0), system, mech), "no infantry left to swap");
        assert!(core.board().planet(first).units.is_empty());
        let empty = System::ALL.iter().copied().find(|s| *s != system).unwrap();
        assert!(!l1z1x_agent(&mut core, scope, Seat(0), empty, mech));
        assert!(core.board().sys(empty).flags.has(crate::state::SysFlags::TOUCHED));
    }

    #[test]
    fn claire_places_the_counted_units_on_the_planet_and_none_for_zero() {
        let mut core = core_with(&[]);
        let system = System::ALL.iter().copied().find(|s| !s.data().planets.is_empty()).unwrap();
        let planet = system.data().planets[0];
        assert!(sol_commander(&mut core, Seat(0), system, planet, Unit::InfantryI, 0));
        assert!(core.board().planet(planet).units.is_empty());
        assert!(sol_commander(&mut core, Seat(0), system, planet, Unit::InfantryI, 2));
        assert_eq!(core.board().planet(planet).units.len(), 2);
    }

}
