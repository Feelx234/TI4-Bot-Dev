//! D4-1 low layer: `query/explore.rs` (decks of a planet, fragments, the Fracture, legendary abilities, what a planet's exploration holds).

use ti4_engine_core::generated::{ActionCard, Explore, ExploreType, Planet, PlanetTrait, Source, System, TileBack, Unit};
use ti4_engine_core::query::explore as q;
use ti4_engine_core::tables::{Scope, source_bit};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{first_planet, plain_systems};

fn table() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_ring(7)
}

#[test]
fn a_trait_names_its_exploration_deck_and_the_others_name_none() {
    assert_eq!(q::deck_of(PlanetTrait::Cultural), Some(ExploreType::Cultural));
    assert_eq!(q::deck_of(PlanetTrait::Hazardous), Some(ExploreType::Hazardous));
    assert_eq!(q::deck_of(PlanetTrait::Industrial), Some(ExploreType::Industrial));
    assert_eq!(q::deck_of(PlanetTrait::Spacestation), None);
}

#[test]
fn a_planets_decks_are_those_of_its_traits_in_scope() {
    let cultural = Planet::ALL.into_iter().find(|p| p.data().traits == [PlanetTrait::Cultural]).expect("a cultural planet");
    assert_eq!(q::traits_of(ALL_SOURCES, cultural).as_slice(), &[ExploreType::Cultural]);
    assert!(q::traits_of(Scope(!source_bit(cultural.data().source)), cultural).is_empty(), "out of scope");
    let station = Planet::ALL.into_iter().find(|p| p.data().traits.contains(&PlanetTrait::Spacestation)).unwrap();
    assert!(q::traits_of(ALL_SOURCES, station).is_empty());
}

#[test]
fn fragments_can_be_purged_three_of_a_kind_with_frontier_fragments_wild() {
    let mut s = table().state();
    assert!(q::purgeable(&s, Seat(0)).is_empty());
    let frags = &mut s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.fragments;
    frags.set(ExploreType::Cultural, 2);
    assert!(q::purgeable(&s, Seat(0)).is_empty(), "two are not three");
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.fragments.set(ExploreType::Frontier, 1);
    assert_eq!(q::purgeable(&s, Seat(0)).as_slice(), &[ExploreType::Cultural], "a frontier fragment counts for any kind");
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.fragments.set(ExploreType::Hazardous, 3);
    assert_eq!(q::purgeable(&s, Seat(0)).as_slice(), &[ExploreType::Cultural, ExploreType::Hazardous]);
    assert!(q::purgeable(&s, Seat(1)).is_empty() && q::purgeable(&s, Seat(9)).is_empty());
    assert_eq!(q::FRAGMENTS_PER_RELIC, 3);
}

#[test]
fn a_card_places_an_attachment_or_a_token_or_nothing() {
    let mut attach = 0;
    let mut token = 0;
    for c in Explore::ALL {
        match q::placed(c) {
            Some(q::Placed::Attachment(a)) => {
                attach += 1;
                assert_eq!(c.data().attachment, Some(a));
            }
            Some(q::Placed::Token(t)) => {
                token += 1;
                assert_eq!(c.data().token, Some(t));
            }
            None => assert!(c.data().attachment.is_none() && c.data().token.is_none()),
        }
        assert_eq!(q::resolution(c), c.data().resolution);
    }
    assert!(attach > 0 && token > 0);
}

#[test]
fn a_planet_has_a_specialty_when_one_is_printed() {
    for p in Planet::ALL {
        assert_eq!(q::has_specialty(p), !p.data().specialties.is_empty());
    }
    assert!(Planet::ALL.iter().any(|p| q::has_specialty(*p)) && Planet::ALL.iter().any(|p| !q::has_specialty(*p)));
}

#[test]
fn the_fracture_is_the_set_of_tiles_with_that_back_and_the_egress_tiles_are_two_of_them() {
    let all = q::fracture_systems(ALL_SOURCES);
    assert!(!all.is_empty());
    assert!(all.as_slice().iter().all(|s| s.data().tile_back == Some(TileBack::Fracture) && q::is_fracture_system(ALL_SOURCES, *s)));
    assert!(!q::is_fracture_system(ALL_SOURCES, plain_systems()[0]));
    let egress = q::egress_systems(ALL_SOURCES);
    assert_eq!(egress.as_slice(), &[System::FractureEgressLeft, System::FractureEgressRight]);
    assert!(egress.as_slice().iter().all(|s| all.as_slice().contains(s)));
    let without = Scope(source_bit(Source::Base));
    assert!(q::fracture_systems(without).is_empty() || q::fracture_systems(without).len() < all.len());
}

#[test]
fn a_legendary_ability_is_ours_to_use_when_we_control_the_planet_and_have_not_exhausted_it() {
    let mut s = table().control(Planet::Tempesta, 0).state();
    assert!(q::legendary_available(&s, Seat(0), Planet::Tempesta));
    assert!(q::ionian_available(&s, Seat(0)));
    assert!(!q::legendary_available(&s, Seat(1), Planet::Tempesta), "not our planet");
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.legendary_exhausted.insert(Planet::Tempesta, ());
    assert!(!q::legendary_available(&s, Seat(0), Planet::Tempesta) && !q::ionian_available(&s, Seat(0)), "exhausted");
    assert!(!q::ionian_available(&table().state(), Seat(0)));
}

#[test]
fn the_salvage_yard_is_read_by_the_holder_of_garbozia_only() {
    let mut s = table().control(Planet::Garbozia, 1).state();
    s.cards_mut().salvage_yard.edit().push(ActionCard::ALL[0]);
    assert_eq!(q::salvaged(&s, Seat(1)), &[ActionCard::ALL[0]]);
    assert!(q::salvaged(&s, Seat(0)).is_empty());
}

#[test]
fn holdings_count_what_an_exploration_card_asks_about() {
    let (system, planet) = first_planet();
    let mut s = table().goods(0, 3).commodities(0, 2).tokens(0, 1, 2, 3).hand(0, &[ActionCard::ALL[0], ActionCard::ALL[1]]).control(planet, 0).unit(system, Some(planet), Unit::InfantryI, 0, 2).unit(system, Some(planet), Unit::FactionlessMech, 0, 1).unit(system, Some(planet), Unit::InfantryI, 1, 4).tech(0, Technology::GravityDrive).state();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(ti4_engine_core::generated::SecretObjective::ALL[0]);
    s.board_mut().planet_mut(planet).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
    let h = q::holdings(&s, Seat(0), Some(planet));
    assert_eq!((h.trade_goods, h.commodities, h.command_tokens, h.action_cards, h.secrets, h.technologies), (3, 2, 6, 2, 1, 1));
    assert_eq!((h.own_infantry, h.own_mechs, h.exhausted), (2, 1, true), "only our own units on the planet");
    let none = q::holdings(&s, Seat(0), None);
    assert_eq!((none.own_infantry, none.own_mechs, none.exhausted), (0, 0, false));
    assert_eq!(q::holdings(&s, Seat(9), Some(planet)).trade_goods, 0);
    assert_eq!(q::holdings(&s, Seat(1), Some(planet)).own_infantry, 4);
}
