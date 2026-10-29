//! bastion: tier W: the Icon's part of a use of PRODUCTION (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CA; B12).
//!
//! | point | string function | here |
//! |---|---|---|
//! | the spots of a produced ship | `bastion::adjust_ship_spots` | `bastion_adjust_ship_spots` |
//! | a ship was placed away from the producing system | `bastion::ship_placed` | `bastion_ship_placed` |
//!
//! "When you produce ships, you may exhaust this card to place those ships in a system that contains 1 of your command tokens,
//! at least 1 of your ground forces, and no other player's ships." Until a ship is placed the Icon's systems are offered as
//! remote spots while the card is ready; once a ship of the use went to one the only spot is that one; once a ship was placed in
//! the producing system the Icon is not offered. Placing a ship in an Icon system exhausts the card.

use ti4_engine_core::choice::production::{Icon, Spot};
use ti4_engine_core::generated::{Breakthrough, System, Unit};
use ti4_engine_core::hooks::direct::SpotList;
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::{board, fleet};
use ti4_engine_core::state::CoreState;

use crate::common::holds_breakthrough;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.direct.bastion_adjust_ship_spots.push(adjust_ship_spots);
    t.direct.bastion_ship_placed.push(ship_placed);
}

fn exhausted(s: &CoreState, who: Seat) -> bool {
    s.marks().tail.contains_key(&MarkKey::BastionIconExhausted(who))
}

/// Systems other than `producing` where `who`'s Icon could place ships: it holds a command token of the player, at least 1 of the
/// player's ground forces and no other player's ships (`bastion::icon_targets`), in system order.
fn icon_targets(s: &CoreState, who: Seat, producing: System) -> Vec<System> {
    System::ALL
        .iter()
        .copied()
        .filter(|system| {
            let cell = s.board().sys(*system);
            *system != producing
                && cell.cmd.contains(who)
                && !cell.units.as_slice().iter().any(|p| p.owner() != who && p.kind().data().ship)
                && cell.units.as_slice().iter().chain(board::planets_of(s, *system).flat_map(|pl| s.board().planet(pl).units.as_slice().iter())).any(|p| p.owner() == who && fleet::is_ground_force(p.kind()))
        })
        .collect()
}

fn adjust_ship_spots(s: &CoreState, _: &Kernel, who: Seat, producing: System, icon: Icon, spots: &mut SpotList) {
    if !holds_breakthrough(s, who, Breakthrough::TheIcon) {
        return;
    }
    match icon {
        Icon::Remote(spot) => {
            spots.clear();
            spots.push(spot);
            return;
        }
        Icon::Local => return,
        Icon::Fresh => {}
    }
    if exhausted(s, who) {
        return;
    }
    for system in icon_targets(s, who, producing) {
        let spot = Spot::Remote(system, None);
        if !spots.as_slice().contains(&spot) {
            spots.push(spot);
        }
    }
}

fn ship_placed(s: &mut CoreState, _: &Kernel, who: Seat, producing: System, target: System, unit: Unit) {
    if target == producing || !holds_breakthrough(s, who, Breakthrough::TheIcon) || exhausted(s, who) || !unit.data().ship || !icon_targets(s, who, producing).contains(&target) {
        return;
    }
    s.marks_mut().tail.edit().insert(MarkKey::BastionIconExhausted(who), MarkVal::Atom(Atom::Num(1)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::generated::Faction;
    use ti4_engine_core::state::Piece;

    /// A Last Bastion (seat 0, the Icon held) producing in `home`; `there` holds its command token and an infantry.
    fn game() -> (CoreState, Kernel, System, System) {
        let (mut s, k) = table(&[Faction::LastBastion, Faction::TheEmiratesOfHacan]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.breakthrough = Some(Breakthrough::TheIcon);
        let (home, there) = (System::ALL[0], System::ALL[1]);
        s.board_mut().sys_mut(there).cmd.insert(Seat(0));
        s.board_mut().sys_mut(there).units.push(Piece::new(Unit::InfantryI, Seat(0)));
        (s, k, home, there)
    }

    fn spots(s: &CoreState, k: &Kernel, home: System, icon: Icon) -> Vec<Spot> {
        let mut out = SpotList::new();
        out.push(Spot::Space);
        adjust_ship_spots(s, k, Seat(0), home, icon, &mut out);
        out.as_slice().to_vec()
    }

    #[test]
    fn the_icon_offers_the_systems_that_hold_a_token_a_ground_force_and_no_foreign_ship() {
        let (mut s, k, home, there) = game();
        assert_eq!(spots(&s, &k, home, Icon::Fresh), vec![Spot::Space, Spot::Remote(there, None)]);
        assert_eq!(spots(&s, &k, there, Icon::Fresh), vec![Spot::Space], "never the producing system");
        s.board_mut().sys_mut(there).units.push(Piece::new(Unit::CruiserI, Seat(1)));
        assert_eq!(spots(&s, &k, home, Icon::Fresh), vec![Spot::Space], "another player's ship");
        s.board_mut().sys_mut(there).units.as_mut_slice().iter_mut().for_each(|p| *p = Piece::new(Unit::InfantryI, Seat(0)));
        s.board_mut().sys_mut(there).cmd.remove(Seat(0));
        assert_eq!(spots(&s, &k, home, Icon::Fresh), vec![Spot::Space], "no command token");
        s.board_mut().sys_mut(there).cmd.insert(Seat(0));
        s.board_mut().sys_mut(there).units.clear();
        assert_eq!(spots(&s, &k, home, Icon::Fresh), vec![Spot::Space], "no ground force");
        s.board_mut().sys_mut(there).units.push(Piece::new(Unit::CruiserI, Seat(0)));
        assert_eq!(spots(&s, &k, home, Icon::Fresh), vec![Spot::Space], "a ship is not a ground force");
        s.board_mut().sys_mut(there).units.clear();
        s.board_mut().sys_mut(there).units.push(Piece::new(Unit::InfantryI, Seat(1)));
        assert_eq!(spots(&s, &k, home, Icon::Fresh), vec![Spot::Space], "another player's ground force");
    }

    #[test]
    fn the_icon_follows_the_use_it_is_exhausted_by_a_remote_ship_and_it_is_the_card_holders() {
        let (mut s, k, home, there) = game();
        assert_eq!(spots(&s, &k, home, Icon::Remote(Spot::Remote(there, None))), vec![Spot::Remote(there, None)], "the first remote ship fixes the spot");
        assert_eq!(spots(&s, &k, home, Icon::Local), vec![Spot::Space], "a ship placed in the producing system closes it");
        ship_placed(&mut s, &k, Seat(0), home, there, Unit::CruiserI);
        assert!(exhausted(&s, Seat(0)), "placing a ship in an Icon system exhausts the card");
        assert_eq!(spots(&s, &k, home, Icon::Fresh), vec![Spot::Space], "exhausted");
        let (mut s, k, home, there) = game();
        ship_placed(&mut s, &k, Seat(0), home, home, Unit::CruiserI);
        ship_placed(&mut s, &k, Seat(0), home, there, Unit::InfantryI);
        assert!(!exhausted(&s, Seat(0)), "not in the producing system, not a ground force");
        let elsewhere = System::ALL[2];
        ship_placed(&mut s, &k, Seat(0), home, elsewhere, Unit::CruiserI);
        assert!(!exhausted(&s, Seat(0)), "a system the Icon does not name");
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.breakthrough = None;
        ship_placed(&mut s, &k, Seat(0), home, there, Unit::CruiserI);
        assert!(!exhausted(&s, Seat(0)) && spots(&s, &k, home, Icon::Fresh) == vec![Spot::Space], "without the Icon");
    }
}
