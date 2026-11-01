//! I48S, the L1Z1X agent (`reactions::l1z1x_agent`): "After a player activates a system: you may exhaust this card to
//! allow that player to replace 1 of their infantry in the active system with 1 mech from their reinforcements."
//! Registered for every seat (an agent with a timing window needs a standing slot, like a reaction card).
//!
//! The mech is the active player's faction mech (the one unit of that faction with the mech base type); a faction
//! with more than one form of it in the tables and the diplomacy promises of a leader's use are the string's. A Naaz mech's
//! placement is announced (`NAAZ_MECH_PLACED`, staged like the string's `supply::stage_naaz_mech_placed`).

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
use crate::kernel::Kernel;
use crate::query::tech::UnitRoster;
use crate::tables::Scope;

/// I48S (`leader:*:l1z1xagent:SYSTEM_ACTIVATED:after`).
pub const I48S: Slot = Slot(6);

const AGENT: Leader = Leader::I48S;

fn ready(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&AGENT) == Some(&LeaderStatus::Readied))
}

/// The mech `target` places (`supply::placed_unit(.., "mech")`). With the table's unit roster (a live game: `AdapterFacts::roster`) it is the form of the
/// sources in play (Naalu and Naaz have reprints and variants in several sources); without one (the lockstep windows) it is the one faction mech of the
/// scope's tables, `Err` when they hold several. `Ok(None)` for a faction without a mech.
pub fn faction_mech(s: &CoreState, k: &Kernel, roster: Option<&UnitRoster>, target: Seat) -> Result<Option<Unit>, Unknown> {
    let Some(p) = s.players().by_seat(target) else { return Ok(None) };
    let scope = Scope(u32::from(k.sources.0));
    if let Some(roster) = roster {
        return Ok(crate::kernel::fx_strategy::placed_unit(s, k, scope, roster, target, UnitBaseType::Mech).filter(|u| scope.unit(*u).is_some()));
    }
    let found: Vec<Unit> = Unit::ALL
        .into_iter()
        .filter(|u| {
            let Some(d) = scope.unit(*u) else { return false };
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
fn swap_site(s: &CoreState, k: &Kernel, roster: Option<&UnitRoster>, target: Seat) -> Result<Option<(Planet, usize, Unit)>, Unknown> {
    let Some(system) = s.meta().active_system else { return Ok(None) };
    let Some(mech) = faction_mech(s, k, roster, target)? else { return Ok(None) };
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
    match swap_site(c.state, c.kernel, c.facts.roster, target) {
        Ok(Some(_)) => {}
        Ok(None) => return false,
        Err(why) => return c.unknown(why),
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
    let (roster, kernel) = (eff.adapter_facts().roster, eff.read().1);
    let Ok(Some((planet, at, mech))) = swap_site(eff.read().0, kernel, roster, target) else { return Ok(()) };
    if !ready(eff.state(), me) {
        return Ok(());
    }
    eff.state().board_mut().planet_mut(planet).units.as_mut_slice()[at] = crate::state::Piece::new(mech, target);
    // A Naaz mech put on the board announces itself (`supply::stage_naaz_mech_placed`), so Absolute Synergy can resolve before the next action.
    if let Some(system) = eff.state().meta().active_system {
        crate::kernel::fx::stage_naaz_mech_placed(eff.state(), target, system, mech);
    }
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
        assert!(!q.ctx.optional && !q.ctx.space_battle && q.ctx.invasion_seq.is_none() && q.ctx.target.is_none(), "the string asks it with a plain context");
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

    /// Lane L: a Naaz target was `MissingFact` ("the placement of a Naaz mech is announced: the string's"); every game with a Naaz seat and the L1Z1X agent
    /// stopped there. The swap stages `NAAZ_MECH_PLACED` like the string's.
    #[test]
    fn a_naaz_target_gets_its_mech_and_the_placement_is_announced() {
        let k = Kernel::plain();
        let mut s = game();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheL1Z1XMindnet), Player::new(Seat(1), Faction::TheNaazRokhaAlliance)]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(AGENT, LeaderStatus::Readied);
        s.board_mut().planet_mut(PLANET).units.clear();
        fx::place_unit(&mut s, Seat(1), SYSTEM, Some(PLANET), Unit::InfantryI);
        // (A live game's table holds the unit roster, which names the form of the sources in play among the Naaz mech's variants.)
        let mut t = table();
        let mut roster = crate::query::tech::UnitRoster::default();
        roster.set(Faction::TheNaazRokhaAlliance, UnitBaseType::Mech, Some(Unit::Eidolon));
        t.facts.roster = Some(Box::leak(Box::new(roster)));
        let mut d = driver(t, &s, &[Seat(0), Seat(1)], activated());
        d.run(&mut s, &k).unwrap();
        assert_eq!(d.unknown(), None, "no bail");
        d.resume(&mut s, &k, pick_ability()).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Reaction(ROpt::Use))).unwrap(), Status::Done);
        assert_eq!(board::planet_units(&s, PLANET)[0].kind(), Unit::Eidolon, "the Naaz mech replaced the infantry");
        assert_eq!(s.pending().staged.iter().filter(|e| e.kind == EventKind::NaazMechPlaced).count(), 1, "its placement is staged once");
    }

    #[test]
    fn an_exhausted_agent_is_not_offered() {
        let k = Kernel::plain();
        let mut spent = game();
        spent.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(AGENT, LeaderStatus::Exhausted);
        let mut d = driver(table(), &spent, &[Seat(0), Seat(1)], activated());
        assert_eq!(d.run(&mut spent, &k).unwrap(), Status::Done);
    }
    /// Lane L: the mech is the one of the sources in play (a table without a roster), the box must hold one (three on the board still leave the fourth,
    /// four leave none), and the infantry replaced is the TARGET's (a rival's infantry or the target's own PDS standing before it is not).
    #[test]
    fn the_mech_is_the_one_of_the_scope_the_box_must_hold_one_and_the_infantry_is_the_targets_own() {
        use crate::generated::Source;
        let hacan = |s: &CoreState, k: &Kernel| faction_mech(s, k, None, Seat(1));
        let mut s = game();
        let mut pok = Kernel::plain();
        pok.sources = crate::kernel::Sources::NONE.with(Source::Base).with(Source::Pok);
        let mut naalu = game();
        naalu.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheL1Z1XMindnet), Player::new(Seat(1), Faction::TheNaaluCollective)]);
        assert_eq!(faction_mech(&naalu, &pok, None, Seat(1)), Ok(Some(Unit::Iconoclast)), "Prophecy of Kings holds one Naalu mech");
        assert_eq!(faction_mech(&naalu, &Kernel::plain(), None, Seat(1)), Err(Unknown::MissingFact), "every source holds three of them: unknown");
        let mech = hacan(&s, &Kernel::plain()).unwrap().expect("a Hacan mech");
        // The box: three mechs on the board leave one, four leave none.
        let k = Kernel::plain();
        let scope = Scope(u32::MAX);
        let before = fleet::remaining_all(&s, scope, Seat(1), mech);
        for _ in 0..(before - 1) {
            fx::place_unit(&mut s, Seat(1), SYSTEM, None, mech);
        }
        assert_eq!(fleet::remaining_all(&s, scope, Seat(1), mech), 1);
        assert!(swap_site(&s, &k, None, Seat(1)).unwrap().is_some(), "one mech left in the box");
        fx::place_unit(&mut s, Seat(1), SYSTEM, None, mech);
        assert_eq!(swap_site(&s, &k, None, Seat(1)).unwrap(), None, "none left");
        // Whose infantry: a rival's, then the target's own PDS, stand before the target's infantry on the planet.
        let mut t = game();
        t.board_mut().planet_mut(PLANET).units.clear();
        fx::place_unit(&mut t, Seat(0), SYSTEM, Some(PLANET), Unit::InfantryI);
        fx::place_unit(&mut t, Seat(1), SYSTEM, Some(PLANET), Unit::PDSI);
        fx::place_unit(&mut t, Seat(1), SYSTEM, Some(PLANET), Unit::InfantryI);
        let (planet, at, _) = swap_site(&t, &k, None, Seat(1)).unwrap().expect("the target's infantry");
        assert_eq!((planet, at), (PLANET, 2), "the third unit of the planet is the target's infantry");
    }

}
