//! The Embers of Muaat, ported from `crates/ti4-engine/src/factions/muaat.rs` (string module).
//!
//! The example of the registration pattern (docs/ENGINE_CORE_FACTIONS_PLAN.md): one `register` that pushes
//! the faction's hook functions, each with the neutral answer for every other seat. Only the two
//! supernova movement slots are ported here; the rest of Muaat (`extra_production`, component actions,
//! leaders, timing abilities) is the C1 shard's work.

use ti4_engine_core::generated::{Faction, Technology};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::kernel::hooks::HookTable;
use ti4_engine_core::state::CoreState;

fn is_muaat(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheEmbersOfMuaat)
}

/// Owns Magmus Reactor, or a Nekro copy of it stands for it (`holds_reactor`).
fn holds_reactor(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| {
        p.tech.owned.contains(Technology::MagmusReactor)
            || p.tech.assimilated.iter().any(|(_, stands_for)| *stands_for == Technology::MagmusReactor)
    })
}

/// Magmus Reactor: a move may end in a supernova (`muaat::may_enter_supernova`).
fn may_enter_supernova(s: &CoreState, _: &Kernel, who: Seat) -> bool {
    is_muaat(s, who) && holds_reactor(s, who)
}

/// Gashlai Physiology: ships move through supernovas (`muaat::may_pass_through_supernova`).
fn may_pass_through_supernova(s: &CoreState, _: &Kernel, who: Seat) -> bool {
    is_muaat(s, who)
}

/// Fill the slots of this module.
pub fn register(t: &mut HookTable) {
    t.movement.may_enter_supernova.push(may_enter_supernova);
    t.movement.may_pass_through_supernova.push(may_pass_through_supernova);
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
        s.players_mut().edit(0).faction = muaat;
        s.players_mut().edit(1).faction = other;
        assert!(t.movement.may_pass_through_supernova(&s, &k, Seat(0)));
        assert!(!t.movement.may_pass_through_supernova(&s, &k, Seat(1)));
        assert!(!t.movement.may_enter_supernova(&s, &k, Seat(0)), "no reactor");
        s.players_mut().edit(0).tech.owned.insert(Technology::MagmusReactor);
        assert!(t.movement.may_enter_supernova(&s, &k, Seat(0)));
        assert!(!t.movement.may_enter_supernova(&s, &k, Seat(1)));
    }
}
