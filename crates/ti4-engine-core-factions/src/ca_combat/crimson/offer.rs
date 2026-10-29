//! crimson: tier W: the hero card in a use of PRODUCTION (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CA; B12).
//!
//! Homesick Phantom: "You may place any of those ships onto this card." While the hero is unlocked every produced ship has the
//! card among its spots (`crimson_cards::offer_card_spot`); a ship placed there is put on the card, the list of units in the order
//! they were placed (`MarkKey::CrimsonCard`), off the board and still out of the box (`fleet::on_crimson_card`).

use ti4_engine_core::choice::production::Spot;
use ti4_engine_core::generated::{Leader, Unit};
use ti4_engine_core::hooks::direct::SpotList;
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::state::{CoreState, LeaderStatus};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.direct.crimson_cards_offer_card_spot.push(offer_card_spot);
    t.direct.crimson_cards_put_on_card.push(put_on_card);
}

fn hero_ready(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&Leader::HomesickPhantom) == Some(&LeaderStatus::Unlocked))
}

fn offer_card_spot(s: &CoreState, who: Seat, spots: &mut SpotList) {
    if hero_ready(s, who) && !spots.as_slice().contains(&Spot::Card) {
        spots.push(Spot::Card);
    }
}

fn put_on_card(s: &mut CoreState, who: Seat, unit: Unit) {
    let tail = s.marks_mut().tail.edit();
    let key = MarkKey::CrimsonCard(who);
    let mut list = match tail.get(&key) {
        Some(MarkVal::List(items)) => items.clone(),
        _ => Vec::new(),
    };
    list.push(MarkVal::Atom(Atom::Unit(unit)));
    tail.insert(key, MarkVal::List(list));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::generated::{Faction, UnitBaseType};
    use ti4_engine_core::query::fleet;
    use ti4_engine_core::tables::Scope;

    #[test]
    fn the_card_is_offered_while_the_hero_is_unlocked_and_holds_the_ships_put_on_it() {
        let (mut s, _) = table(&[Faction::TheCrimsonRebellion, Faction::TheEmiratesOfHacan]);
        let mut spots = SpotList::new();
        offer_card_spot(&s, Seat(0), &mut spots);
        assert!(spots.as_slice().is_empty(), "no hero");
        s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HomesickPhantom, LeaderStatus::Unlocked);
        offer_card_spot(&s, Seat(0), &mut spots);
        offer_card_spot(&s, Seat(0), &mut spots);
        assert_eq!(spots.as_slice(), &[Spot::Card], "once");
        let mut other = SpotList::new();
        offer_card_spot(&s, Seat(1), &mut other);
        assert!(other.as_slice().is_empty(), "another seat");
        let scope = Scope(u32::MAX);
        let before = fleet::remaining_all(&s, scope, Seat(0), Unit::CruiserI);
        put_on_card(&mut s, Seat(0), Unit::CruiserI);
        put_on_card(&mut s, Seat(0), Unit::CruiserI);
        assert_eq!(fleet::on_crimson_card(&s, scope, Seat(0), UnitBaseType::Cruiser), 2);
        assert_eq!(fleet::on_crimson_card(&s, scope, Seat(0), UnitBaseType::Carrier), 0, "another type");
        assert_eq!(fleet::on_crimson_card(&s, scope, Seat(1), UnitBaseType::Cruiser), 0, "another seat");
        assert_eq!(fleet::remaining_all(&s, scope, Seat(0), Unit::CruiserI), before - 2, "off the board, still out of the box");
    }
}
