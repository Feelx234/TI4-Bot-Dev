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
/// not a space station, sorted by system and planet. `None`: the adapter did not compute the neighbours.
fn destinations(s: &CoreState, facts: &ti4_engine_core::kernel::ability::AdapterFacts, recipient: Seat, producing: System) -> Option<Vec<(System, Planet)>> {
    let allowed = facts.reach(recipient, producing)?;
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
    let Some(found) = destinations(c.state, c.facts, recipient, system) else { return c.unknown(Unknown::MissingFact) };
    if found.is_empty() {
        return false;
    }
    // `leaders::exhaust` announces the exhaustion to a watching Nomad; the core does not.
    if c.kernel.hooks.direct.nomad_watches_agent_exhaustion(c.state) {
        return c.unknown(Unknown::NomadAgentExhaustion);
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
    let found = destinations(eff.state(), &facts, recipient, system).unwrap_or_default();
    if found.is_empty() || !ready_card(eff.state(), owner, card) {
        return Ok(());
    }
    if let Some(p) = eff.state().players_mut().by_seat_mut(owner) {
        p.leaders.status.insert(card, LeaderStatus::Exhausted);
    }
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
}
