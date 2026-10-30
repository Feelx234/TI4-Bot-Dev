//! Magen Defense Grid (`md`, the Codex 4 printing: `Technology::MagenDefenseGridOmegaOmega`; the superseded printings are not in the deck), the first effect (package W9; string `magen_defense::activation`): "When any player activates a
//! system that contains 1 or more of your structures, place 1 infantry from your reinforcements with each of those structures."
//! Mandatory, one infantry per structure on the planet it stands on (a structure in a space area gets none), placed in the form the
//! owner places, within what the reinforcements hold, before the activator's reactions (`SYSTEM_ACTIVATED`, `when`). From rules
//! version `RULES_TECH_EFFECTS`; registered for every seat (the card can be gained mid-game), the condition names the holder.
//!
//! Not ported: the second effect (a hit on a ground force as a ground combat starts: the owner assigns it), which stays the
//! string's and bails its window as an unported ability.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Planet, System, Technology, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx_strategy;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, EventKind, Key};

/// The activation effect (`technology:*:md:SYSTEM_ACTIVATED:when`).
pub const ACTIVATION: Slot = Slot(16_001);

/// `ti4_model::state::RULES_TECH_EFFECTS`.
const RULES_TECH_EFFECTS: u32 = 15;

/// The planets of `system` on which `who` has structures, one entry per structure, planets in id order.
fn structure_planets(s: &CoreState, who: Seat, system: System) -> Vec<Planet> {
    let mut found = Vec::new();
    for planet in board::planets_of(s, system) {
        for piece in s.board().planet(planet).units.as_slice() {
            if piece.owner() == who && piece.kind().data().structure {
                found.push(planet);
            }
        }
    }
    found
}

fn active(s: &CoreState, who: Seat) -> bool {
    s.meta().rules_at_least(RULES_TECH_EFFECTS) && s.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(Technology::MagenDefenseGridOmegaOmega))
}

fn condition(c: &CondCx<'_>) -> bool {
    active(c.state, c.def.owner) && c.event.payload.system(Key::System).is_some_and(|system| !structure_planets(c.state, c.def.owner, system).is_empty())
}

fn place(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    let planets = structure_planets(eff.read().0, me, system);
    let scope = crate::common::scope(eff.kernel());
    let roster = crate::cd_registry::yin::timing::roster(scope);
    for planet in planets {
        let (state, kernel) = eff.state_and_kernel();
        fx_strategy::place_for(state, kernel, scope, &roster, me, system, Some(planet), UnitBaseType::Infantry);
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(ACTIVATION, AbilityKind::Technology, EventKind::SystemActivated, Relation::When, place).when(condition));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::generated::{Faction, Unit};

    #[test]
    fn a_structure_gets_its_infantry_on_its_planet_one_per_structure_and_only_for_the_holder() {
        let (mut s, _) = table(&[Faction::TheEmiratesOfHacan, Faction::TheArborec]);
        let planet = System::ALL.iter().flat_map(|sys| sys.data().planets.iter().copied()).next().expect("a planet");
        let system = planet.data().system.expect("its system");
        assert!(structure_planets(&s, Seat(0), system).is_empty());
        ti4_engine_core::kernel::fx::place_unit(&mut s, Seat(0), system, Some(planet), Unit::PDSI);
        ti4_engine_core::kernel::fx::place_unit(&mut s, Seat(0), system, Some(planet), Unit::PDSI);
        ti4_engine_core::kernel::fx::place_unit(&mut s, Seat(0), system, Some(planet), Unit::InfantryI);
        ti4_engine_core::kernel::fx::place_unit(&mut s, Seat(1), system, Some(planet), Unit::PDSI);
        assert_eq!(structure_planets(&s, Seat(0), system), vec![planet, planet], "two structures, the infantry is no structure");
        assert_eq!(structure_planets(&s, Seat(1), system), vec![planet]);
        assert!(!active(&s, Seat(0)), "not held");
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::MagenDefenseGridOmegaOmega);
        s.meta_mut().rules_version = 14;
        assert!(!active(&s, Seat(0)), "below the rules version of the technology effects");
        s.meta_mut().rules_version = RULES_TECH_EFFECTS;
        assert!(active(&s, Seat(0)));
        assert!(!active(&s, Seat(1)));
    }
}
