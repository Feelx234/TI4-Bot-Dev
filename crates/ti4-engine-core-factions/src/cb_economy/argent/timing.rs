//! argent: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: Trilossa Aun Mirik, the
//! Argent agent ("when you would produce ground forces: exhaust this card to place them on a planet you control in
//! an adjacent system"), the offer of the production window. Not ported: the Yssaril copy (Ssruu), the Strike Wing
//! Ambuscade extra dice and the wing transfer.
//!
//! The grant is a mark (`MarkKey::ArgentProduction(recipient, system)`: the planets the units may be placed on, as
//! `system@planet` entries) that the production window reads and clears when it closes. The systems adjacent to the
//! producing system are the adapter's fact (`Facts::reach`: no galaxy is lent to the window).

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, Leader, Planet, PlanetTrait, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec, Unknown};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::{board, laws, reach};
use ti4_engine_core::state::{CoreState, EventKind, Key, LeaderStatus};

/// Trilossa Aun Mirik (`leader:argent:argentagent:GROUND_FORCES_BEING_PRODUCED:when`).
pub const AGENT: Slot = Slot(4001);

const AGENT_LEADER: Leader = Leader::TrilossaAunMirik;

fn ready_card(s: &CoreState, who: Seat, card: Leader) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&card) == Some(&LeaderStatus::Readied))
}

/// `agent_planets`: the planets `recipient` controls in a system adjacent to `producing`, not demilitarized and
/// not a space station, sorted by system and planet. The neighbours are the adapter's when it recorded them, else the position's own movement map
/// (a live game holds the galaxy); `None`: neither knows.
fn destinations(s: &CoreState, k: &ti4_engine_core::kernel::Kernel, facts: &ti4_engine_core::kernel::ability::AdapterFacts, recipient: Seat, producing: System) -> Option<Vec<(System, Planet)>> {
    let allowed: Vec<System> = match facts.reach(recipient, producing) {
        Some(recorded) => recorded.to_vec(),
        None => reach::neighbours_of(s, k, recipient, producing)?,
    };
    let blanked = reach::blanked(s);
    Some(
        board::controlled_planets(s, recipient)
            .as_slice()
            .iter()
            .filter(|(system, planet)| {
                allowed.contains(system) && !laws::planet_is_demilitarized(s, blanked, *planet) && !planet.data().traits.contains(&PlanetTrait::Spacestation)
            })
            .copied()
            .collect(),
    )
}

fn agent_when(c: &CondCx<'_>) -> bool {
    when_by(c, AGENT_LEADER, true)
}

/// The Ssruu copy (`lent`: the lender's agent may be copied, tested by the caller): Ssruu is the card that is ready.
pub(crate) fn copy_when(c: &CondCx<'_>) -> bool {
    when_by(c, Leader::Ssruu, crate::cd_registry::yssaril::ssruu::borrowable(c.state, c.def.owner, c.def.pair_source(), AGENT_LEADER))
}

fn when_by(c: &CondCx<'_>, card: Leader, lent: bool) -> bool {
    let p = &c.event.payload;
    let (Some(recipient), Some(system), Some(count)) = (p.seat(Key::Player), p.system(Key::System), p.int(Key::Count)) else { return false };
    if count <= 0 || !lent || !ready_card(c.state, c.def.owner, card) {
        return false;
    }
    let Some(found) = destinations(c.state, c.kernel, c.facts, recipient, system) else { return c.unknown(Unknown::MissingFact) };
    if found.is_empty() {
        return false;
    }
    true
}

fn agent(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    agent_by(eff, AGENT_LEADER)
}

/// The Ssruu copy of the grant.
pub(crate) fn copy_agent(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let (me, lender) = (eff.owner(), eff.def.pair_source());
    if !crate::cd_registry::yssaril::ssruu::borrowable(eff.state(), me, lender, AGENT_LEADER) {
        return Ok(());
    }
    agent_by(eff, Leader::Ssruu)
}

fn agent_by(eff: &mut Eff<'_, '_>, card: Leader) -> Result<(), TimingError> {
    let owner = eff.owner();
    let (Some(recipient), Some(system)) = (eff.event.payload.seat(Key::Player), eff.event.payload.system(Key::System)) else { return Ok(()) };
    let facts = eff.adapter_facts().clone();
    let found = { let (state, kernel) = eff.read(); destinations(state, kernel, &facts, recipient, system).unwrap_or_default() };
    if found.is_empty() || !ready_card(eff.state(), owner, card) {
        return Ok(());
    }
    let (state, kernel) = eff.state_and_kernel();
    ti4_engine_core::kernel::fx_strategy::exhaust_agent(state, kernel, owner, card);
    let entries = found.into_iter().map(|(target, planet)| MarkVal::Rec(vec![MarkVal::Atom(Atom::System(target)), MarkVal::Atom(Atom::Planet(planet))])).collect();
    eff.state().marks_mut().tail.edit().insert(MarkKey::ArgentProduction(recipient, system), MarkVal::List(entries));
    Ok(())
}

/// Register the abilities, in the string order of the Argent's `timing_abilities` (the agent).
pub fn register_abilities(t: &mut AbilityTable) {
    t.add(
        Spec::new(AGENT, AbilityKind::Leader, EventKind::GroundForcesBeingProduced, Relation::When, agent)
            .applies(Applies::Faction(Faction::TheArgentFlight))
            .optional()
            .when(agent_when),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agent_is_registered_for_the_argent_only() {
        let mut t = AbilityTable::default();
        register_abilities(&mut t);
        let spec = t.of_slot(AGENT).expect("registered");
        assert!(spec.optional && spec.condition.is_some());
        assert_eq!(spec.event, EventKind::GroundForcesBeingProduced);
    }

    /// Lane L: the planets the agent names are the recipient's controlled planets in systems next to the producing one. The adapter's recorded neighbours win; a
    /// position with a map answers the pair nobody recorded (it was the counted `MissingFact` that stopped the Argent games); with neither the answer is unknown.
    #[test]
    fn the_destinations_follow_the_recorded_neighbours_else_the_position_own_map() {
        use crate::common::testkit::table;
        use ti4_engine_core::galaxy::CoreGalaxy;
        use ti4_engine_core::kernel::ability::AdapterFacts;
        use ti4_engine_core::hex::Hex;
        static NO_LANES: [Option<&[(u8, u8)]>; System::COUNT] = [None; System::COUNT];
        let (mut s, k) = table(&[Faction::TheArgentFlight, Faction::TheArborec]);
        let plain: Vec<System> = System::ALL.iter().copied().filter(|x| !x.data().planets.is_empty() && !x.data().hyperlane && !x.data().home_system).take(3).collect();
        let (producing, near, far) = (plain[0], plain[1], plain[2]);
        let f = AdapterFacts::default();
        assert_eq!(destinations(&s, &k, &f, Seat(1), producing), None, "no recorded neighbours and no map");
        let mut galaxy = CoreGalaxy::new(&NO_LANES);
        galaxy.place(producing, Hex::new(0, 0)).unwrap();
        galaxy.place(near, Hex::new(1, 0)).unwrap();
        galaxy.place(far, Hex::new(5, 0)).unwrap();
        galaxy.refresh_lane_links();
        s.map_mut().set_galaxy(galaxy);
        for system in [near, far] {
            s.board_mut().planet_mut(system.data().planets[0]).control = Some(Seat(1));
        }
        let next_door: Vec<(System, Planet)> = vec![(near, near.data().planets[0])];
        assert_eq!(destinations(&s, &k, &f, Seat(1), producing), Some(next_door), "only the planet in the neighbouring system");
        let mut recorded = AdapterFacts::default();
        recorded.set_reach(Seat(1), producing, vec![far]);
        assert_eq!(destinations(&s, &k, &recorded, Seat(1), producing), Some(vec![(far, far.data().planets[0])]), "the adapter's neighbours win");
    }
}
