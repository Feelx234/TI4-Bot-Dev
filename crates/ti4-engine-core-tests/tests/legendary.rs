//! D4A4: `rules::legendary::Legendary`, the abilities of the legendary planets at the end of the holder's turn and when the holder passes, against
//! the printed ability texts:
//!
//! * Hope's End (Imperial Arms Vault): "ACTION? no: at the end of your turn, exhaust: place 1 mech on a planet you control or draw 1 action card";
//! * Primor (The Atrament): place up to 2 infantry on any planet you control; Mallice (Exterrix Headquarters): gain 2 trade goods or convert
//!   all your commodities to trade goods; Mirage (Mirage Flight Academy): place up to 2 fighters in any system that contains your ships;
//!   Emelpar (The Acropolis): ready any 1 of your planets, technologies, relics, leaders or legendary abilities; Faunus (Maxis Central Control),
//!   Industrex (Aurex Mechanica: place 1 ship you could build with a unit upgrade you own) and Garbozia (Dok 'N Pic's Salvage Yard: take an
//!   action card from the discard pile) when you pass.

use ti4_engine_core::choice::legendary::{Ask as LAsk, Opt as LOpt, When};
use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{ActionCard, Planet, SecretObjective, Technology as T, UnitBaseType};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::query::tech::UnitRoster;
use ti4_engine_core::rules::legendary::Legendary;
use ti4_engine_core::state::{Code, TransientFlags};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn roster() -> &'static UnitRoster {
    Box::leak(Box::new(UnitRoster::default()))
}

fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut t);
    t.facts.card_groups = (0..ActionCard::COUNT).map(|i| u8::try_from(i).unwrap()).collect();
    t
}

fn base() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_ring(7)
}

fn hold(mut s: CoreState, planets: &[Planet]) -> CoreState {
    for p in planets {
        s.board_mut().planet_mut(*p).control = Some(Seat(0));
    }
    s
}

fn window(when: When) -> Legendary {
    Legendary::new(Seat(0), when, Code(5), ALL_SOURCES, roster(), false)
}

fn rig(s: CoreState, when: When) -> Rig<Legendary> {
    Rig::from_parts(s, kernel_all(), window(when)).with_table(table())
}

fn lg(o: LOpt) -> Opt {
    Opt::Legendary(o)
}

fn exhausted(s: &CoreState, p: Planet) -> bool {
    s.players().by_seat(Seat(0)).unwrap().cards.legendary_exhausted.contains_key(&p)
}

fn goods(s: &CoreState) -> i16 {
    s.players().by_seat(Seat(0)).unwrap().econ.trade_goods
}

fn of_base(s: &CoreState, planet: Planet, base: UnitBaseType) -> usize {
    s.board().planet(planet).units.as_slice().iter().filter(|u| u.owner() == Seat(0) && u.kind().data().base_type == base).count()
}

// ---- the offer --------------------------------------------------------------------------------

#[test]
fn the_offer_lists_each_controlled_ready_legendary_planet_in_the_order_of_the_string_and_a_decline() {
    covers!("rules::legendary::Legendary");
    let st = || hold(base().state(), &[Planet::Mallice, Planet::HopesEnd]);
    let mut rig = rig(st(), When::EndOfTurn);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::Offer { when: When::EndOfTurn })));
    assert_eq!(rig.opts(), vec![lg(LOpt::Use(Planet::HopesEnd)), lg(LOpt::Use(Planet::Mallice)), Opt::Decline]);
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(lg(LOpt::Use(Planet::Primor))));
    rig.decline();
    assert!(rig.is_done() && rig.driver.window.used == 0);
    assert_eq!(delta(rig.start_state(), &rig.state), "", "declined: nothing is exhausted");
    let mut pass = self::rig(st(), When::Pass);
    pass.run();
    assert!(pass.is_done(), "neither is a when-you-pass ability: no offer");
    let mut none = self::rig(base().state(), When::EndOfTurn);
    none.run();
    assert!(none.is_done(), "no legendary planet controlled");
    let mut tired = st();
    tired.players_mut().by_seat_mut(Seat(0)).unwrap().cards.legendary_exhausted.insert(Planet::Mallice, ());
    tired.board_mut().planet_mut(Planet::HopesEnd).control = None;
    let mut spent = self::rig(tired, When::EndOfTurn);
    spent.run();
    assert!(spent.is_done(), "an exhausted ability is not offered");
    smoke("legendary/offer", || self::rig(st(), When::EndOfTurn));
}

#[test]
fn exterrix_headquarters_gains_two_trade_goods_or_converts_every_commodity_and_the_card_is_exhausted_first() {
    covers!("rules::legendary::Legendary");
    let st = || {
        let mut s = hold(base().goods(0, 1).commodities(0, 3).state(), &[Planet::Mallice]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.commodities = 3;
        s
    };
    let mut rig = rig(st(), When::EndOfTurn);
    rig.run().pick_opt(lg(LOpt::Use(Planet::Mallice)));
    assert!(exhausted(&rig.state, Planet::Mallice), "exhausted before it resolves");
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::Exterrix { commodities: 3 })));
    assert_eq!(rig.opts(), vec![lg(LOpt::Goods), lg(LOpt::Convert)]);
    rig.assert_options_sound();
    rig.pick_opt(lg(LOpt::Goods));
    assert!(rig.is_done() && rig.driver.window.used == 1);
    assert_eq!(goods(&rig.state), 3, "gain 2 trade goods");
    let mut conv = self::rig(st(), When::EndOfTurn);
    conv.run().pick_opt(lg(LOpt::Use(Planet::Mallice)));
    conv.pick_opt(lg(LOpt::Convert));
    assert_eq!(goods(&conv.state), 4, "3 commodities become 3 trade goods");
    assert_eq!(conv.state.players().by_seat(Seat(0)).unwrap().econ.commodities, 0);
    smoke("legendary/exterrix", || self::rig(st(), When::EndOfTurn));
}

#[test]
fn hopes_end_places_a_mech_on_a_planet_you_control_or_draws_an_action_card() {
    covers!("rules::legendary::Legendary");
    let ps = plain_systems();
    let (p1, p2) = (ps[1].data().planets[0], ps[2].data().planets[0]);
    let st = || {
        let mut s = hold(base().state(), &[Planet::HopesEnd, p1, p2]);
        s.decks_mut().action = Deck::new(vec![ActionCard::Plague, ActionCard::Rout]);
        s
    };
    let mut rig = rig(st(), When::EndOfTurn);
    rig.run().pick_opt(lg(LOpt::Use(Planet::HopesEnd)));
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::ArmsVault)));
    assert_eq!(rig.opts(), vec![lg(LOpt::Mech), lg(LOpt::Card)]);
    rig.assert_options_sound();
    rig.pick_opt(lg(LOpt::Mech));
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::PlaceOnPlanet { source: Planet::HopesEnd, .. })));
    assert!(rig.opts().len() >= 2, "every planet the seat controls");
    rig.assert_options_sound();
    rig.pick_where(|o| matches!(o, Opt::Legendary(LOpt::OnPlanet(p, _)) if *p == p2));
    assert!(rig.is_done());
    assert_eq!(of_base(&rig.state, p2, UnitBaseType::Mech), 1, "1 mech");
    let mut card = self::rig(st(), When::EndOfTurn);
    card.run().pick_opt(lg(LOpt::Use(Planet::HopesEnd)));
    card.pick_opt(lg(LOpt::Card));
    assert!(card.is_done());
    assert_eq!(card.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[ActionCard::Plague], "1 action card from the top");
    // Over the hand limit the draw asks which card goes (the child window).
    let mut full = st();
    let seven = [ActionCard::Rally, ActionCard::NavSuite, ActionCard::CoupDEtat, ActionCard::Spy, ActionCard::Sabotage1, ActionCard::Veto, ActionCard::Uprising];
    full.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand = seven.iter().copied().collect();
    let mut over = self::rig(full, When::EndOfTurn);
    over.run().pick_opt(lg(LOpt::Use(Planet::HopesEnd)));
    over.pick_opt(lg(LOpt::Card));
    assert!(matches!(over.asked().ask, Ask::Cards(_)), "{:?}", over.asked().ask);
    assert!(over.stages.iter().any(|s| s == "Drawing"));
    over.pick(0);
    assert!(over.is_done());
    assert_eq!(over.state.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 7);
}

#[test]
fn primor_places_two_infantry_on_one_planet_and_mirage_two_fighters_in_a_system_with_the_seats_ships() {
    covers!("rules::legendary::Legendary");
    let ps = plain_systems();
    let p1 = ps[1].data().planets[0];
    let st = || hold(base().unit(ps[2], None, Unit::CruiserI, 0, 1).unit(ps[3], None, Unit::DestroyerI, 0, 1).state(), &[Planet::Primor, Planet::Mirage, p1]);
    let mut rig = rig(st(), When::EndOfTurn);
    rig.run().pick_opt(lg(LOpt::Use(Planet::Primor)));
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::PlaceOnPlanet { source: Planet::Primor, .. })));
    rig.assert_options_sound();
    rig.pick_where(|o| matches!(o, Opt::Legendary(LOpt::OnPlanet(p, _)) if *p == p1));
    assert_eq!(of_base(&rig.state, p1, UnitBaseType::Infantry), 2, "2 infantry on the planet chosen");
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::Offer { .. })), "back to the offer: the other ability is still ready");
    assert_eq!(rig.opts(), vec![lg(LOpt::Use(Planet::Mirage)), Opt::Decline], "Primor is exhausted");
    rig.pick_opt(lg(LOpt::Use(Planet::Mirage)));
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::PlaceFighters)));
    assert_eq!(rig.opts(), vec![lg(LOpt::InSystem(ps[2])), lg(LOpt::InSystem(ps[3]))], "only systems that contain the seat's ships");
    rig.assert_options_sound();
    rig.pick(1);
    assert!(rig.is_done());
    assert_eq!(rig.state.board().sys(ps[3]).units.as_slice().iter().filter(|u| u.owner() == Seat(0) && u.kind() == Unit::FighterI).count(), 2, "2 fighters");
    assert_eq!(rig.driver.window.used, 2);
    smoke("legendary/primor_mirage", || self::rig(st(), When::EndOfTurn));
    // Without a ship Mirage asks nothing but is spent.
    let mut dry = self::rig(hold(base().state(), &[Planet::Mirage]), When::EndOfTurn);
    dry.run().pick_opt(lg(LOpt::Use(Planet::Mirage)));
    assert!(dry.is_done() && dry.driver.window.used == 1);
    assert!(exhausted(&dry.state, Planet::Mirage));
}

#[test]
fn the_acropolis_readies_any_exhausted_planet_technology_or_other_legendary_ability_but_not_itself() {
    covers!("rules::legendary::Legendary");
    let ps = plain_systems();
    let p1 = ps[1].data().planets[0];
    let st = || {
        let mut s = hold(base().tech(0, T::SarweenTools).exhaust(p1).state(), &[Planet::Emelpar, Planet::Mallice, p1]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.exhausted.insert(T::SarweenTools);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.legendary_exhausted.insert(Planet::Mallice, ());
        s
    };
    let mut rig = rig(st(), When::EndOfTurn);
    rig.run().pick_opt(lg(LOpt::Use(Planet::Emelpar)));
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::Acropolis)));
    let n = rig.opts().len();
    assert!(n >= 3, "the planet, the technology and the other legendary ability: {n}");
    assert!(!rig.opts().iter().any(|o| matches!(o, Opt::Legendary(LOpt::Ready(r, _)) if format!("{r:?}").contains("Emelpar"))), "not the Acropolis itself");
    rig.assert_options_sound();
    rig.pick_where(|o| matches!(o, Opt::Legendary(LOpt::Ready(r, _)) if format!("{r:?}").contains("Mallice")));
    assert!(!exhausted(&rig.state, Planet::Mallice), "readied");
    assert!(exhausted(&rig.state, Planet::Emelpar));
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::Offer { .. })), "the readied ability can be used again");
    assert!(rig.opts().contains(&lg(LOpt::Use(Planet::Mallice))));
    rig.decline();
    let mut tech = self::rig(st(), When::EndOfTurn);
    tech.run().pick_opt(lg(LOpt::Use(Planet::Emelpar)));
    tech.pick_where(|o| matches!(o, Opt::Legendary(LOpt::Ready(r, _)) if format!("{r:?}").contains("SarweenTools")));
    assert!(!tech.state.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(T::SarweenTools));
    let mut planet = self::rig(st(), When::EndOfTurn);
    planet.run().pick_opt(lg(LOpt::Use(Planet::Emelpar)));
    planet.pick_where(|o| matches!(o, Opt::Legendary(LOpt::Ready(_, at)) if at.0.is_some()));
    assert!(!ti4_engine_core::query::board::exhausted(&planet.state, p1));
    let mut nothing = self::rig(hold(base().state(), &[Planet::Emelpar]), When::EndOfTurn);
    nothing.run().pick_opt(lg(LOpt::Use(Planet::Emelpar)));
    assert!(nothing.is_done(), "nothing exhausted: no question, the ability is still spent");
}

#[test]
fn the_galactic_council_returns_a_secret_and_draws_one_and_thunders_edge_gives_another_action() {
    covers!("rules::legendary::Legendary");
    let st = || {
        let mut s = hold(base().state(), &[Planet::MecatolRexTe, Planet::ThundersEdge]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(SecretObjective::ALL[0]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(SecretObjective::ALL[1]);
        s.decks_mut().secret = Deck::new(vec![SecretObjective::ALL[5], SecretObjective::ALL[6]]);
        s
    };
    let mut rig = rig(st(), When::EndOfTurn);
    rig.run().pick_opt(lg(LOpt::Use(Planet::MecatolRexTe)));
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::Council)));
    assert_eq!(rig.opts().len(), 2, "one option per secret in hand");
    rig.assert_options_sound();
    rig.pick_opt(lg(LOpt::DiscardSecret(SecretObjective::ALL[0])));
    let held = rig.state.players().by_seat(Seat(0)).unwrap().cards.secrets.as_slice().to_vec();
    assert_eq!(held.len(), 2, "one returned, one drawn");
    assert!(!held.contains(&SecretObjective::ALL[0]) && held.contains(&SecretObjective::ALL[1]));
    assert_eq!(rig.opts(), vec![lg(LOpt::Use(Planet::ThundersEdge)), Opt::Decline]);
    rig.pick_opt(lg(LOpt::Use(Planet::ThundersEdge)));
    assert!(rig.is_done());
    assert!(rig.state.transient().misc.flags.has(TransientFlags::ADDITIONAL_ACTION), "perform another action");
    smoke("legendary/council", || self::rig(st(), When::EndOfTurn));
    let mut empty = self::rig(hold(base().state(), &[Planet::MecatolRexTe]), When::EndOfTurn);
    empty.run().pick_opt(lg(LOpt::Use(Planet::MecatolRexTe)));
    assert!(empty.is_done(), "no secret to return");
}

#[test]
fn when_you_pass_faunus_takes_a_planet_garbozia_a_discarded_card_and_industrex_a_ship() {
    covers!("rules::legendary::Legendary");
    let ps = plain_systems();
    let p1 = ps[1].data().planets[0];
    let st = || {
        let mut s = hold(base().tech(0, T::CruiserII).unit(ps[2], None, Unit::CruiserI, 0, 1).state(), &[Planet::Faunus, Planet::Garbozia, Planet::Industrex]);
        s.cards_mut().discarded_action.edit().extend([ActionCard::Rally, ActionCard::Spy]);
        s
    };
    let pass = |s: CoreState| Rig::from_parts(s, kernel_all(), window(When::Pass).with_maxis(vec![(ps[1], p1)])).with_table(table());
    let mut rig = pass(st());
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::Offer { when: When::Pass })));
    assert_eq!(rig.opts(), vec![lg(LOpt::Use(Planet::Faunus)), lg(LOpt::Use(Planet::Industrex)), lg(LOpt::Use(Planet::Garbozia)), Opt::Decline]);
    rig.pick_opt(lg(LOpt::Use(Planet::Garbozia)));
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::Salvage)));
    assert_eq!(rig.opts().len(), 3, "both cards of the discard pile and a decline");
    rig.assert_options_sound();
    rig.pick_opt(lg(LOpt::Salvage(ActionCard::Spy)));
    assert_eq!(rig.state.cards().discarded_action.as_slice(), &[ActionCard::Rally]);
    assert_eq!(rig.state.cards().salvage_yard.as_slice(), &[ActionCard::Spy], "the card moves to the salvage yard");
    rig.pick_opt(lg(LOpt::Use(Planet::Industrex)));
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::Aurex)));
    assert_eq!(rig.opts(), vec![lg(LOpt::Ship(Unit::CruiserII, ps[2])), Opt::Decline], "the ship the unit upgrade unlocks, in a system with the seat's ship");
    rig.assert_options_sound();
    rig.pick(0);
    assert_eq!(rig.state.board().sys(ps[2]).units.as_slice().iter().filter(|u| u.owner() == Seat(0) && u.kind() == Unit::CruiserII).count(), 1);
    rig.pick_opt(lg(LOpt::Use(Planet::Faunus)));
    assert!(matches!(rig.asked().ask, Ask::Legendary(LAsk::Maxis)));
    assert_eq!(rig.opts(), vec![lg(LOpt::Take(ps[1], p1)), Opt::Decline]);
    rig.decline();
    assert!(rig.is_done(), "every ability is spent: no further offer");
    assert_eq!(rig.driver.window.used, 3);
    smoke("legendary/pass", || pass(st()));
    let mut take = pass(st());
    take.run().pick_opt(lg(LOpt::Use(Planet::Faunus)));
    take.pick_opt(lg(LOpt::Take(ps[1], p1)));
    assert!(take.is_done() && take.driver.window.unsupported, "the control gain chain is the string's: the caller declines the run");
    let mut declined = pass(st());
    declined.run().pick_opt(lg(LOpt::Use(Planet::Garbozia)));
    declined.decline();
    assert!(matches!(declined.asked().ask, Ask::Legendary(LAsk::Offer { .. })));
}
