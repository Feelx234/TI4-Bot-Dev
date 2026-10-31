//! I48S, the L1Z1X agent (`reactions::l1z1x_agent`): "After a player activates a system: you may exhaust this card to
//! allow that player to replace 1 of their infantry in the active system with 1 mech from their reinforcements."
//! Registered for every seat (an agent with a timing window needs a standing slot, like a reaction card).
//!
//! The mech is the active player's faction mech (the one unit of that faction with the mech base type); a faction
//! with more than one form of it in the tables, a Naaz target (the placement is announced) and the diplomacy
//! promises of a leader's use are the string's.

use crate::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Leader, Planet, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec, Unknown};
use crate::kernel::resolver::TimingError;
use crate::query::{board, fleet, tech};
use crate::state::{CoreState, EventKind, LeaderStatus};
use crate::tables::Scope;

/// I48S (`leader:*:l1z1xagent:SYSTEM_ACTIVATED:after`).
pub const I48S: Slot = Slot(6);

const AGENT: Leader = Leader::I48S;

fn ready(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&AGENT) == Some(&LeaderStatus::Readied))
}

/// The faction mech of `target`, `Ok(None)` for a faction without one, `Err` when the tables hold several.
fn faction_mech(s: &CoreState, target: Seat) -> Result<Option<Unit>, Unknown> {
    let Some(p) = s.players().by_seat(target) else { return Ok(None) };
    let found: Vec<Unit> = Unit::ALL
        .into_iter()
        .filter(|u| {
            let d = u.data();
            d.base_type == UnitBaseType::Mech && d.faction.is_some_and(|tag| tech::tag_is(tag, p.faction))
        })
        .collect();
    match found.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(*one)),
        _ => Err(Unknown::MissingFact),
    }
}

/// The planet of the active system with an infantry of `target` (the first in planet order), if the swap can happen.
fn swap_site(s: &CoreState, target: Seat) -> Result<Option<(Planet, usize, Unit)>, Unknown> {
    let Some(system) = s.meta().active_system else { return Ok(None) };
    let Some(mech) = faction_mech(s, target)? else { return Ok(None) };
    if fleet::remaining_all(s, Scope(u32::MAX), target, mech) <= 0 {
        return Ok(None);
    }
    let mut planets: Vec<Planet> = board::planets_of(s, system).collect();
    planets.sort_unstable();
    for planet in planets {
        let at = board::planet_units(s, planet).iter().position(|u| u.owner() == target && u.kind().data().base_type == UnitBaseType::Infantry);
        if let Some(at) = at {
            return Ok(Some((planet, at, mech)));
        }
    }
    Ok(None)
}

fn condition(c: &CondCx<'_>) -> bool {
    let (Some(target), true) = (c.state.meta().active, ready(c.state, c.def.owner)) else { return false };
    match swap_site(c.state, target) {
        Ok(Some(_)) => {}
        Ok(None) => return false,
        Err(why) => return c.unknown(why),
    }
    // The placement of a Naaz mech is announced: the string's.
    if c.state.players().by_seat(target).is_some_and(|p| p.faction == crate::generated::Faction::TheNaazRokhaAlliance) {
        return c.unknown(Unknown::MissingFact);
    }
    true
}

fn swap(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let target = eff.state().meta().active;
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::AgentSwap { target }),
        ctx: Ctx {
            origin: Origin::Content(Content::Leader(AGENT)),
            subtype: Subtype::Reaction(RSub::AgentSwap),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: [Opt::Reaction(ROpt::Use), Opt::Decline].into_iter().collect::<Opts>(),
    };
    let Pick::Opt(Opt::Reaction(ROpt::Use)) = eff.ask(&choice) else { return Ok(()) };
    let Some(target) = target else { return Ok(()) };
    let Ok(Some((planet, at, mech))) = swap_site(eff.state(), target) else { return Ok(()) };
    if !ready(eff.state(), me) {
        return Ok(());
    }
    eff.state().board_mut().planet_mut(planet).units.as_mut_slice()[at] = crate::state::Piece::new(mech, target);
    let (state, kernel) = eff.state_and_kernel();
    crate::kernel::fx_strategy::exhaust_agent(state, kernel, me, AGENT);
    Ok(())
}

pub fn register(t: &mut AbilityTable) {
    t.add(Spec::new(I48S, AbilityKind::Leader, EventKind::SystemActivated, Relation::After, swap).optional().when(condition));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::reaction::{AbilityId, Opt as ROpt};
    use crate::driver::Status;
    use crate::generated::{Faction, System};
    use crate::kernel::{fx, Kernel};
    use crate::rules::reactions::testkit::driver;
    use crate::state::{Code, Key, Payload, Player, Players, Val};

    const SYSTEM: System = System::MecatolRex18;
    const PLANET: Planet = Planet::MecatolRex;

    /// Seat 0 holds the agent; seat 1 (Hacan) is the active player with an infantry on Mecatol Rex.
    fn game() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheL1Z1XMindnet), Player::new(Seat(1), Faction::TheEmiratesOfHacan)]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(AGENT, LeaderStatus::Readied);
        s.meta_mut().active = Some(Seat(1));
        s.meta_mut().active_system = Some(SYSTEM);
        fx::place_unit(&mut s, Seat(1), SYSTEM, Some(PLANET), Unit::InfantryI);
        s
    }

    fn activated() -> crate::state::EventRec {
        fx::event(EventKind::SystemActivated, Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::System, Val::System(SYSTEM)))
    }

    fn table() -> AbilityTable {
        let mut t = AbilityTable::default();
        register(&mut t);
        t
    }

    fn pick_ability() -> Pick {
        Pick::Opt(Opt::Reaction(ROpt::Ability(AbilityId { kind: AbilityKind::Other, key: Code(I48S.0) })))
    }

    #[test]
    fn using_the_agent_replaces_an_infantry_by_the_faction_mech_and_exhausts_the_card() {
        let (mut s, k) = (game(), Kernel::plain());
        let mut d = driver(table(), &s, &[Seat(0), Seat(1)], activated());
        d.run(&mut s, &k).unwrap();
        let Status::Asking(q) = d.resume(&mut s, &k, pick_ability()).unwrap() else { panic!("the ability asks") };
        assert_eq!(q.ask, Ask::Reaction(RAsk::AgentSwap { target: Some(Seat(1)) }));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Reaction(ROpt::Use))).unwrap(), Status::Done);
        let units = board::planet_units(&s, PLANET);
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].kind().data().base_type, UnitBaseType::Mech);
        assert_eq!(units[0].owner(), Seat(1));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&AGENT), Some(&LeaderStatus::Exhausted));
    }

    #[test]
    fn declining_or_having_no_infantry_changes_nothing() {
        let k = Kernel::plain();
        let mut s = game();
        let mut d = driver(table(), &s, &[Seat(0), Seat(1)], activated());
        d.run(&mut s, &k).unwrap();
        d.resume(&mut s, &k, pick_ability()).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap(), Status::Done);
        assert_eq!(board::planet_units(&s, PLANET)[0].kind(), Unit::InfantryI);
        let mut empty = game();
        empty.board_mut().planet_mut(PLANET).units.clear();
        let mut d = driver(table(), &empty, &[Seat(0), Seat(1)], activated());
        assert_eq!(d.run(&mut empty, &k).unwrap(), Status::Done, "no infantry: not offered");
    }

    #[test]
    fn a_naaz_target_is_unknown_and_an_exhausted_agent_is_not_offered() {
        let k = Kernel::plain();
        let mut s = game();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheL1Z1XMindnet), Player::new(Seat(1), Faction::TheNaazRokhaAlliance)]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(AGENT, LeaderStatus::Readied);
        let mut d = driver(table(), &s, &[Seat(0), Seat(1)], activated());
        let _ = d.run(&mut s, &k).unwrap();
        assert_eq!(d.unknown(), Some(Unknown::MissingFact));
        let mut spent = game();
        spent.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(AGENT, LeaderStatus::Exhausted);
        let mut d = driver(table(), &spent, &[Seat(0), Seat(1)], activated());
        assert_eq!(d.run(&mut spent, &k).unwrap(), Status::Done);
    }
}
