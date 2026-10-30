//! D4-2c: `kernel::fx_explore` (drawing an exploration card, fragments, frontier tokens, wormhole tokens, commodities).

use ti4_engine_core::containers::Deck;
use ti4_engine_core::kernel::fx_explore as fx;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn table() -> CoreState {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).state()
}

#[test]
fn a_draw_comes_from_the_named_deck_only_and_a_deck_nobody_loaded_is_empty() {
    covers!("kernel::fx_explore::draw");
    let mut s = table();
    let cultural: Vec<Explore> = Explore::ALL.into_iter().take(2).collect();
    s.decks_mut().explore[ExploreType::Cultural as usize] = Deck::new(cultural.clone());
    assert_eq!(fx::draw(&mut s, ExploreType::Cultural), None, "the deck is not marked present");
    s.decks_mut().explore_present |= 1 << ExploreType::Cultural as u8;
    assert_eq!(fx::draw(&mut s, ExploreType::Cultural), Some(cultural[0]));
    assert_eq!(fx::draw(&mut s, ExploreType::Cultural), Some(cultural[1]));
    assert_eq!(fx::draw(&mut s, ExploreType::Cultural), None, "empty");
    assert_eq!(fx::draw(&mut s, ExploreType::Hazardous), None);
}

#[test]
fn fragments_commodities_and_cards_go_to_the_seat() {
    covers!("kernel::fx_explore::gain_fragment");
    let mut s = table();
    fx::gain_fragment(&mut s, Seat(0), ExploreType::Industrial);
    fx::gain_fragment(&mut s, Seat(0), ExploreType::Industrial);
    fx::gain_fragment(&mut s, Seat(9), ExploreType::Industrial);
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.fragments.get(ExploreType::Industrial), Some(2));
    fx::play_area_card(&mut s, Seat(1), Explore::ALL[0]);
    assert_eq!(s.players().by_seat(Seat(1)).unwrap().cards.explore.as_slice(), &[Explore::ALL[0]]);
}

#[test]
fn commodities_gained_stop_at_the_factions_cap_and_spent_ones_stop_at_what_is_held() {
    covers!("kernel::fx_explore::gain_commodities");
    let mut s = table();
    let cap = i32::from(s.players().by_seat(Seat(0)).unwrap().faction.data().commodities);
    assert_eq!(fx::gain_commodities(&mut s, Seat(0), 1), 1);
    assert_eq!(fx::gain_commodities(&mut s, Seat(0), 100), cap - 1, "only up to the cap");
    assert_eq!(i32::from(s.players().by_seat(Seat(0)).unwrap().econ.commodities), cap);
    assert_eq!(fx::gain_commodities(&mut s, Seat(0), 1), 0, "full");
    assert_eq!(fx::gain_commodities(&mut s, Seat(9), 1), 0);
    assert_eq!(fx::spend_commodities(&mut s, Seat(0), Some(1)), 1);
    assert_eq!(fx::spend_commodities(&mut s, Seat(0), None), cap - 1, "all that are left");
    assert_eq!(fx::spend_commodities(&mut s, Seat(0), Some(5)), 0);
    assert_eq!(fx::spend_commodities(&mut s, Seat(9), None), 0);
}

#[test]
fn a_frontier_token_is_taken_once_and_a_wormhole_token_moves() {
    covers!("kernel::fx_explore::take_frontier");
    let sys = plain_systems();
    let mut s = table();
    s.explore_mut().frontier.insert(sys[0]);
    assert!(fx::take_frontier(&mut s, sys[0]));
    assert!(!fx::take_frontier(&mut s, sys[0]), "already taken");
    assert!(!fx::take_frontier(&mut s, sys[1]));
    fx::place_wormhole_token(&mut s, Wormhole::Alpha, sys[0]);
    fx::place_wormhole_token(&mut s, Wormhole::Alpha, sys[1]);
    fx::place_wormhole_token(&mut s, Wormhole::Beta, sys[2]);
    assert_eq!(s.explore().wormhole_tokens.get(&Wormhole::Alpha), Some(&sys[1]), "one token per kind: it moved");
    assert_eq!(s.explore().wormhole_tokens.get(&Wormhole::Beta), Some(&sys[2]));
}
