//! The Embers of Muaat: Gashlai Physiology and Magmus Reactor.

use ti4_engine_core::generated::{Faction, Technology};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::CoreState;

use crate::common::{has_tech_text, is_faction, player};

pub fn register(t: &mut HookTable) {
    t.movement.may_enter_supernova.push(may_enter_supernova);
    t.movement.may_pass_through_supernova.push(may_pass_through_supernova);
}

fn is_muaat(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheEmbersOfMuaat)
}

/// `holds_reactor`: the technology, or a Valefar Assimilator standing for it (any value of the map).
fn holds_reactor(s: &CoreState, who: Seat) -> bool {
    has_tech_text(s, who, Technology::MagmusReactor)
        || player(s, who).is_some_and(|p| p.tech.assimilated.values().any(|t| t == Technology::MagmusReactor))
}

fn may_enter_supernova(s: &CoreState, _: &Kernel, who: Seat) -> bool {
    is_muaat(s, who) && holds_reactor(s, who)
}

fn may_pass_through_supernova(s: &CoreState, _: &Kernel, who: Seat) -> bool {
    is_muaat(s, who)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_muaat_seat_passes_and_it_needs_the_reactor_to_enter() {
        let mut t = HookTable::new();
        register(&mut t);
        assert_eq!(t.registered(), 2);
        let k = Kernel::plain();
        let mut s = CoreState::all_loaded();
        let muaat = Faction::TheEmbersOfMuaat;
        let other = if Faction::ALL[0] == muaat { Faction::ALL[1] } else { Faction::ALL[0] };
        *s.players_mut() = ti4_engine_core::state::Players::from_vec(vec![
            ti4_engine_core::state::Player::new(Seat(0), muaat),
            ti4_engine_core::state::Player::new(Seat(1), other),
        ]);
        assert!(t.movement.may_pass_through_supernova(&s, &k, Seat(0)));
        assert!(!t.movement.may_pass_through_supernova(&s, &k, Seat(1)));
        assert!(!t.movement.may_enter_supernova(&s, &k, Seat(0)), "no reactor");
        s.players_mut().edit(0).tech.owned.insert(Technology::MagmusReactor);
        assert!(t.movement.may_enter_supernova(&s, &k, Seat(0)));
        assert!(!t.movement.may_enter_supernova(&s, &k, Seat(1)));
    }
}
