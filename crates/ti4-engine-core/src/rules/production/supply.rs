//! Supply mutations (package B3): the writes of `supply.rs` that move goods and plastic between the
//! reserve, the board and a captor's sheet: spending trade goods (with the commodities a Keleres agent
//! window lets a payer spend), capturing a unit from the board or the reinforcements, returning a
//! captured model and releasing every captured model.
//!
//! Each function is a total copy of its string twin, including the refusals: a refusal changes nothing.

use crate::generated::{Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::Kernel;
use crate::query::fleet;
use crate::state::{CoreState, Piece};
use crate::tables::Scope;

/// `supply::spendable_goods`: the trade goods plus the commodities an open agent window lets `who` spend as
/// trade goods.
#[must_use]
pub fn spendable_goods(s: &CoreState, k: &Kernel, who: Seat) -> i64 {
    let goods = s.players().by_seat(who).map_or(0, |p| i64::from(p.econ.trade_goods.max(0)));
    goods + k.hooks.direct.keleres_spendable_commodities(s, who)
}

/// `supply::spend_goods`: spend `amount` trade goods, commodities an open agent window allows first. All of it or
/// none: `false`, state untouched, when [`spendable_goods`] falls short.
pub fn spend_goods(s: &mut CoreState, k: &Kernel, who: Seat, amount: i32) -> bool {
    if amount <= 0 {
        return true;
    }
    if spendable_goods(s, k, who) < i64::from(amount) {
        return false;
    }
    let commodities = i32::try_from(k.hooks.direct.keleres_spendable_commodities(s, who)).unwrap_or(0);
    let Some(p) = s.players_mut().by_seat_mut(who) else { return false };
    let from_commodities = amount.min(commodities);
    p.econ.commodities -= i16::try_from(from_commodities).unwrap_or(i16::MAX);
    p.econ.trade_goods -= i16::try_from(amount - from_commodities).unwrap_or(i16::MAX);
    true
}

/// `supply::capture_from_board`: take `piece` off `system` (its space area, or `planet`) and onto `captor`'s
/// sheet. `false`, nothing changed, when the unit is not there, when a seat is missing or when they are the same
/// player.
pub fn capture_from_board(s: &mut CoreState, captor: Seat, system: System, planet: Option<Planet>, piece: Piece) -> bool {
    let owner = piece.owner();
    if captor == owner || s.players().by_seat(captor).is_none() || s.players().by_seat(owner).is_none() {
        return false;
    }
    let list = match planet {
        Some(p) => &mut s.board_mut().planet_mut(p).units,
        None => &mut s.board_mut().sys_mut(system).units,
    };
    let Some(at) = list.as_slice().iter().position(|found| *found == piece) else { return false };
    list.remove(at);
    if let Some(p) = s.players_mut().by_seat_mut(captor) {
        p.misc.captured_units.push((owner, piece.kind()));
    }
    true
}

/// `supply::capture_from_reinforcements`: capture a model out of `owner`'s reinforcements. Needs a model left in
/// the box (31.4); `false`, nothing changed, otherwise.
pub fn capture_from_reinforcements(s: &mut CoreState, scope: Scope, captor: Seat, owner: Seat, unit: Unit) -> bool {
    if captor == owner
        || s.players().by_seat(captor).is_none()
        || s.players().by_seat(owner).is_none()
        || fleet::remaining(s, scope, owner, unit, 0) == 0
    {
        return false;
    }
    if let Some(p) = s.players_mut().by_seat_mut(captor) {
        p.misc.captured_units.push((owner, unit));
    }
    true
}

/// `supply::return_captured`: return the first captured model of `owner`'s that has this base type from
/// `captor`'s sheet. The unit comes back in the form the table's `captured_unit_return_form` fold says;
/// `None` (nothing changes) when the captor holds none of that base type for that owner.
pub fn return_captured(s: &mut CoreState, k: &Kernel, captor: Seat, owner: Seat, base: UnitBaseType) -> Option<Unit> {
    let p = s.players_mut().by_seat_mut(captor)?;
    let at = p.misc.captured_units.as_slice().iter().position(|(who, unit)| *who == owner && unit.data().base_type == base)?;
    let (_, unit) = p.misc.captured_units.remove(at);
    Some(k.hooks.economy.captured_unit_return_form(s, k, owner, unit))
}

/// `supply::release_all_captured`: every model `captor` holds goes back to its owner; how many.
pub fn release_all_captured(s: &mut CoreState, captor: Seat) -> usize {
    s.players_mut().by_seat_mut(captor).map_or(0, |p| {
        let n = p.misc.captured_units.len();
        p.misc.captured_units.clear();
        n
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::Faction;
    use crate::ids::system_at;
    use crate::state::{Player, Players};

    fn table() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![
            Player::new(Seat(0), Faction::TheFederationOfSol),
            Player::new(Seat(1), Faction::TheXxchaKingdom),
            Player::new(Seat(2), Faction::TheVuilraithCabal),
        ]);
        s
    }

    fn held(s: &CoreState, who: Seat) -> Vec<(Seat, Unit)> {
        s.players().by_seat(who).map(|p| p.misc.captured_units.as_slice().to_vec()).unwrap_or_default()
    }

    #[test]
    fn spending_goods_is_all_or_nothing_and_a_non_positive_spend_is_free() {
        let k = Kernel::plain();
        let mut s = table();
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 3;
        assert!(spend_goods(&mut s, &k, Seat(0), 0));
        assert!(spend_goods(&mut s, &k, Seat(0), -2));
        assert!(!spend_goods(&mut s, &k, Seat(0), 4));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 3);
        assert!(spend_goods(&mut s, &k, Seat(0), 3));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 0);
        assert!(!spend_goods(&mut s, &k, Seat(7), 1), "a missing seat has nothing");
        assert_eq!(spendable_goods(&s, &k, Seat(0)), 0);
    }

    #[test]
    fn a_capture_needs_the_exact_unit_two_seats_and_a_different_captor() {
        let mut s = table();
        let sys = system_at(30);
        let plain = Piece::new(Unit::CarrierI, Seat(1));
        s.board_mut().sys_mut(sys).units.push(plain.with_sustained(true));
        assert!(!capture_from_board(&mut s, Seat(0), sys, None, plain), "damage is part of the unit");
        assert!(!capture_from_board(&mut s, Seat(1), sys, None, plain.with_sustained(true)), "never from oneself");
        assert!(!capture_from_board(&mut s, Seat(9), sys, None, plain.with_sustained(true)), "the captor must be seated");
        assert!(held(&s, Seat(0)).is_empty());
        assert!(capture_from_board(&mut s, Seat(0), sys, None, plain.with_sustained(true)));
        assert_eq!(held(&s, Seat(0)), vec![(Seat(1), Unit::CarrierI)]);
        assert!(s.board().sys(sys).units.is_empty());
        assert!(!capture_from_board(&mut s, Seat(0), sys, None, plain.with_sustained(true)), "it is gone");
    }

    #[test]
    fn a_return_takes_the_first_model_of_the_base_type_and_release_empties_the_sheet() {
        let k = Kernel::plain();
        let mut s = table();
        let held_by = &mut s.players_mut().by_seat_mut(Seat(0)).unwrap().misc.captured_units;
        held_by.push((Seat(1), Unit::CruiserI));
        held_by.push((Seat(2), Unit::CarrierI));
        held_by.push((Seat(1), Unit::CarrierII));
        held_by.push((Seat(1), Unit::CarrierI));
        assert_eq!(return_captured(&mut s, &k, Seat(0), Seat(1), UnitBaseType::Carrier), Some(Unit::CarrierII));
        assert_eq!(return_captured(&mut s, &k, Seat(0), Seat(1), UnitBaseType::Mech), None);
        assert_eq!(return_captured(&mut s, &k, Seat(2), Seat(1), UnitBaseType::Carrier), None, "the other sheet is empty");
        assert_eq!(release_all_captured(&mut s, Seat(0)), 3);
        assert!(held(&s, Seat(0)).is_empty());
        assert_eq!(release_all_captured(&mut s, Seat(0)), 0);
        assert_eq!(release_all_captured(&mut s, Seat(9)), 0);
    }

    #[test]
    fn capturing_from_the_box_needs_a_model_left_in_it() {
        let mut s = table();
        let scope = Scope(u32::from(Kernel::plain().sources.0));
        // Four carriers on the board leave none in the box.
        let sys = system_at(30);
        for _ in 0..4 {
            s.board_mut().sys_mut(sys).units.push(Piece::new(Unit::CarrierI, Seat(1)));
        }
        assert!(!capture_from_reinforcements(&mut s, scope, Seat(0), Seat(1), Unit::CarrierI));
        assert!(!capture_from_reinforcements(&mut s, scope, Seat(1), Seat(1), Unit::CruiserI), "never from oneself");
        assert!(capture_from_reinforcements(&mut s, scope, Seat(0), Seat(1), Unit::CruiserI));
        assert_eq!(held(&s, Seat(0)), vec![(Seat(1), Unit::CruiserI)]);
        assert!(capture_from_reinforcements(&mut s, scope, Seat(0), Seat(1), Unit::FighterI), "fighters are uncapped");
    }
}
