//! Package X: `rules::explore_windows::Exploring`, one exploration of a planet or a frontier token (the string `explore_in` with the choice of
//! the deck): the draw, Distant Suns, the log row and the holdings around the card, every card arm, the faction hooks and `PLANET_EXPLORED`.

use ti4_engine_core::choice::explore::{Opt as EOpt, Step};
use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt, Pool};
use ti4_engine_core::generated::{Attachment, PlanetTrait, UnitBaseType};
use ti4_engine_core::rules::explore_windows::{attachment_for, traits_now, Exploring};
use ti4_engine_core::state::{EDeck, EventKind, Key, Piece, PlanetFlags, Val};
use ti4_engine_core::tables::Scope;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;
use ti4_engine_core_tests::windows::{explore_table, text_of, EXPLORATION_GOODS};

fn ex(o: EOpt) -> Opt {
    Opt::Explore(o)
}

/// A planet of the base game whose printed traits are exactly `traits`, alone in its (plain) system.
fn planet_with(traits: &[PlanetTrait]) -> (System, Planet) {
    plain_systems()
        .into_iter()
        .find_map(|s| {
            let d = s.data();
            let p = *d.planets.first()?;
            (d.planets.len() == 1 && p.data().traits == traits && p.data().source == Source::Base && p.data().specialties.is_empty()).then_some((s, p))
        })
        .unwrap_or_else(|| panic!("a plain base planet with traits {traits:?}"))
}

fn with_specialty() -> (System, Planet) {
    plain_systems()
        .into_iter()
        .find_map(|s| {
            let d = s.data();
            let p = *d.planets.first()?;
            (d.planets.len() == 1 && p.data().source == Source::Base && !p.data().specialties.is_empty() && !p.data().traits.is_empty()).then_some((s, p))
        })
        .expect("a planet with a specialty")
}

struct Setup {
    system: System,
    planet: Planet,
    scenario: Scenario,
}

fn setup(spot: (System, Planet), factions: &[Faction]) -> Setup {
    let (system, planet) = spot;
    let scenario = Scenario::new(factions).map_from(&[system]).control(planet, 0).commodities(0, 0).goods(0, 0).tokens(0, 2, 2, 2);
    Setup { system, planet, scenario }
}

fn stack(state: &mut CoreState, deck: ExploreType, cards: &[Explore]) {
    state.decks_mut().explore[deck as usize].set(cards.to_vec());
    state.decks_mut().explore_present |= 1 << deck as u8;
}

fn build(sc: Setup, deck: ExploreType, cards: &[Explore], announce: bool, mode: EDeck) -> (Rig<Exploring>, System, Planet) {
    let (system, planet) = (sc.system, sc.planet);
    let (mut state, kernel) = sc.scenario.build();
    stack(&mut state, deck, cards);
    let rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), planet, mode, announce)).with_table(explore_table());
    (rig, system, planet)
}

fn quick(spot: (System, Planet), deck: ExploreType, cards: &[Explore]) -> (Rig<Exploring>, System, Planet) {
    build(setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]), deck, cards, false, EDeck::Fixed(deck))
}

fn goods(rig: &Rig<Exploring>) -> i16 {
    rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods
}

fn row(rig: &Rig<Exploring>, at: usize) -> (String, String) {
    let r = rig.state.explore().log.get(at).expect("a row");
    (text_of(&rig.state, r.outcome), r.result.map(|c| text_of(&rig.state, c)).unwrap_or_default())
}

// ---- the deck ------------------------------------------------------------------------------------------------

#[test]
fn a_planet_with_two_traits_asks_the_deck_and_the_draw_follows_the_answer() {
    covers!("rules::explore_windows::Exploring");
    let spot = planet_with(&[PlanetTrait::Cultural]);
    // The planet carries Terraform, which adds the other two traits to the printed one.
    let mut s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    s.scenario = s.scenario.clone();
    let (mut state, kernel) = s.scenario.build();
    state.board_mut().planet_mut(spot.1).attach.push(Attachment::Terraform);
    stack(&mut state, ExploreType::Cultural, &[Explore::CulturalRelicFragment1]);
    stack(&mut state, ExploreType::Hazardous, &[Explore::HazardousRelicFragment1]);
    let traits = traits_now(&state, Scope(u32::MAX), spot.1);
    assert_eq!(traits.as_slice().len(), 3, "{traits:?}");
    let mut rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Choose, false)).with_table(explore_table());
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Explore(ti4_engine_core::choice::explore::Ask::ChooseDeck)), "{}", rig.transcript());
    rig.assert_options_sound();
    let pick = rig.opts().iter().position(|o| *o == ex(EOpt::Deck(ExploreType::Hazardous))).expect("the hazardous deck is offered");
    rig.pick(pick);
    assert!(rig.is_done(), "{}", rig.transcript());
    let log: Vec<_> = rig.state.explore().log.iter().collect();
    assert_eq!((log.len(), log[0].deck, log[0].card, log[0].planet), (1, ExploreType::Hazardous, Explore::HazardousRelicFragment1, Some(spot.1)));
    assert!(rig.driver.window.explored());
}

#[test]
fn a_single_trait_is_taken_unasked_and_a_planet_without_a_trait_explores_nothing() {
    covers!("rules::explore_windows::Exploring");
    let spot = planet_with(&[PlanetTrait::Industrial]);
    let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let (mut state, kernel) = s.scenario.build();
    stack(&mut state, ExploreType::Industrial, &[Explore::IndustrialRelicFragment1]);
    let mut rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Choose, false)).with_table(explore_table());
    rig.run();
    assert!(rig.is_done() && rig.driver.window.explored(), "{}", rig.transcript());
    assert_eq!(rig.state.explore().log.len(), 1);
    // Mecatol Rex has no trait.
    let rex = (System::ALL.iter().copied().find(|s| s.data().planets.contains(&Planet::MecatolRex)).unwrap(), Planet::MecatolRex);
    let s = setup(rex, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let (state, kernel) = s.scenario.build();
    let mut rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), Planet::MecatolRex, EDeck::Choose, false)).with_table(explore_table());
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.explored() && !rig.driver.window.empty());
    assert_eq!(delta(rig.start_state(), &rig.state), "", "nothing to explore: nothing changes");
}

#[test]
fn an_empty_deck_explores_nothing_and_opens_no_row() {
    covers!("rules::explore_windows::Exploring");
    let (mut rig, _, _) = quick(planet_with(&[PlanetTrait::Cultural]), ExploreType::Cultural, &[]);
    rig.run();
    assert!(rig.is_done() && rig.driver.window.empty() && !rig.driver.window.explored());
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

// ---- the arms of the cards ----------------------------------------------------------------------------------

#[test]
fn a_fragment_is_kept_and_its_row_says_so() {
    covers!("rules::explore_windows::Exploring");
    let (mut rig, _, _) = quick(planet_with(&[PlanetTrait::Hazardous]), ExploreType::Hazardous, &[Explore::HazardousRelicFragment2]);
    rig.run();
    assert!(rig.is_done() && rig.driver.window.explored());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.fragments.get(ExploreType::Hazardous), Some(1));
    assert_eq!(row(&rig, 0).0, format!("{:?}", ti4_engine_core::state::EHow::Fragment(ExploreType::Hazardous)));
}

#[test]
fn an_attachment_is_put_on_the_planet_and_a_research_facility_takes_the_form_the_planet_needs() {
    covers!("rules::explore_windows::Exploring");
    let (mut rig, _, planet) = quick(planet_with(&[PlanetTrait::Cultural]), ExploreType::Cultural, &[Explore::BioticResearchFacility]);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.state.board().planet(planet).attach.as_slice(), &[Attachment::BioticResearchFacility]);
    assert!(rig.state.board().planet(planet).flags.has(PlanetFlags::ATTACH_ENTRY));
    assert!(row(&rig, 0).1.contains("attached Some(BioticResearchFacility)"), "{:?}", row(&rig, 0));
    // A planet that prints a specialty takes the +1/+1 form.
    let spot = with_specialty();
    let deck = spot.1.data().traits.iter().find_map(|t| ti4_engine_core::query::explore::deck_of(*t)).unwrap();
    assert_eq!(attachment_for(Explore::BioticResearchFacility, spot.1), Some(Attachment::BioticResearchFacilityStat));
    assert_eq!(attachment_for(Explore::DemilitarizedZone, spot.1), Some(Attachment::DemilitarizedZone), "only the facilities have a second form");
    let (mut rig, _, planet) = quick(spot, deck, &[Explore::BioticResearchFacility]);
    rig.run();
    assert_eq!(rig.state.board().planet(planet).attach.as_slice(), &[Attachment::BioticResearchFacilityStat]);
}

#[test]
fn the_demilitarized_zone_returns_the_structures_and_sends_the_ground_forces_to_space_from_its_version() {
    covers!("rules::explore_windows::Exploring");
    let spot = planet_with(&[PlanetTrait::Cultural]);
    let run = |version: u32| {
        let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
        let sc = s.scenario.rules(version).unit(spot.0, Some(spot.1), Unit::PDSI, 0, 1).unit(spot.0, Some(spot.1), Unit::InfantryI, 0, 2).unit(spot.0, Some(spot.1), Unit::PDSI, 1, 1);
        let (mut state, kernel) = sc.build();
        stack(&mut state, ExploreType::Cultural, &[Explore::DemilitarizedZone]);
        let mut rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Cultural), false)).with_table(explore_table());
        rig.run();
        assert!(rig.is_done() && rig.driver.window.explored());
        rig
    };
    let after = run(34);
    let units: Vec<Unit> = after.state.board().planet(spot.1).units.as_slice().iter().map(|p| p.kind()).collect();
    assert!(units.is_empty() || !units.contains(&Unit::InfantryI), "the ground forces left the planet: {units:?}");
    let space = after.state.board().sys(spot.0).units.as_slice().iter().filter(|p| p.kind() == Unit::InfantryI).count();
    assert_eq!(space, 2, "the infantry is in the space area");
    let before = run(33);
    assert!(before.state.board().planet(spot.1).units.as_slice().iter().any(|p| p.kind() == Unit::InfantryI), "before version 34 the card only attaches");
}

#[test]
fn an_attachment_drawn_for_a_frontier_token_is_discarded() {
    covers!("rules::explore_windows::Exploring");
    let sys = plain_systems()[0];
    let (mut state, kernel) = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[sys]).build();
    stack(&mut state, ExploreType::Frontier, &[Explore::DysonSphere]);
    state.explore_mut().frontier.insert(sys);
    let mut rig = Rig::from_parts(state, kernel, Exploring::frontier(Seat(0), sys, false, true)).with_table(explore_table());
    rig.run();
    assert!(rig.is_done() && rig.driver.window.explored());
    assert_eq!(row(&rig, 0).0, "Discarded");
    assert!(rig.state.explore().frontier.is_empty(), "the token is spent");
    assert_eq!(rig.state.explore().log.get(0).unwrap().system, Some(sys));
}

#[test]
fn an_entropic_field_asks_the_pool_then_pays_its_goods_and_announces_them() {
    covers!("rules::explore_windows::Exploring");
    let s = setup(planet_with(&[PlanetTrait::Hazardous]), &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let (mut rig, _, _) = build(s, ExploreType::Hazardous, &[Explore::EntropicField], true, EDeck::Fixed(ExploreType::Hazardous));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { left: 1 })), "{}", rig.transcript());
    rig.pick_opt(Opt::Strategy(SOpt::Pool(Pool::Fleet)));
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(goods(&rig), 2);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens, 3);
    let kinds: Vec<EventKind> = rig.driver.emitted.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EventKind::TradeGoodsGained, EventKind::PlanetExplored], "the goods, then the planet explored");
    let goods_event = &rig.driver.emitted[0];
    assert_eq!(goods_event.payload.get(Key::Source), Some(Val::Code(EXPLORATION_GOODS)));
    let explored = &rig.driver.emitted[1];
    assert_eq!(explored.payload.get(Key::Deck), Some(Val::Code(ti4_engine_core::state::Code(100 + ExploreType::Hazardous as u16))));
    assert_eq!(explored.payload.get(Key::Card), Some(Val::Code(ti4_engine_core::state::Code(200 + Explore::EntropicField as u16))));
    // Nothing is announced without a timing handle.
    let s = setup(planet_with(&[PlanetTrait::Hazardous]), &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let (mut quiet, _, _) = build(s, ExploreType::Hazardous, &[Explore::EntropicField], false, EDeck::Fixed(ExploreType::Hazardous));
    quiet.run();
    quiet.pick_opt(Opt::Strategy(SOpt::Pool(Pool::Fleet)));
    assert!(quiet.driver.emitted.is_empty());
    assert_eq!(goods(&quiet), 2);
}

#[test]
fn the_command_tokens_of_an_old_game_go_to_the_strategy_pool_unasked() {
    covers!("rules::explore_windows::Exploring");
    let s = setup(planet_with(&[PlanetTrait::Hazardous]), &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let s = Setup { scenario: s.scenario.rules(11), ..s };
    let (mut rig, _, _) = build(s, ExploreType::Hazardous, &[Explore::MinorEntropicField], false, EDeck::Fixed(ExploreType::Hazardous));
    rig.run();
    assert!(rig.is_done());
    assert_eq!((goods(&rig), rig.state.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens), (1, 3));
}

#[test]
fn abandoned_warehouses_gain_commodities_or_convert_them() {
    covers!("rules::explore_windows::Exploring");
    let spot = planet_with(&[PlanetTrait::Industrial]);
    let make = |commodities: i16| {
        let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
        let s = Setup { scenario: s.scenario.commodities(0, commodities), ..s };
        build(s, ExploreType::Industrial, &[Explore::AbandonedWarehouses1], true, EDeck::Fixed(ExploreType::Industrial)).0
    };
    // Nothing to convert (version 19 on): the single option is taken unasked.
    let mut rig = make(0);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities, 2);
    // Commodities in hand: asked; converted up to two they become trade goods (announced).
    let mut rig = make(3);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Explore(ti4_engine_core::choice::explore::Ask::Reward(Step::AbandonedWarehouses))));
    rig.assert_options_sound();
    rig.pick_opt(ex(EOpt::Convert));
    assert!(rig.is_done());
    assert_eq!((goods(&rig), rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities), (2, 1));
    assert_eq!(rig.driver.emitted.iter().filter(|e| e.kind == EventKind::TradeGoodsGained).count(), 1);
    let mut rig = make(3);
    rig.run();
    rig.pick_opt(ex(EOpt::Gain));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities, i16::from(Faction::TheArborec.data().commodities).min(5));
}

#[test]
fn a_functioning_base_trades_a_good_for_an_action_card_through_the_draw_child() {
    covers!("rules::explore_windows::Exploring");
    let spot = planet_with(&[PlanetTrait::Industrial]);
    let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let s = Setup { scenario: s.scenario.goods(0, 2).commodities(0, 1), ..s };
    let (mut state, kernel) = s.scenario.build();
    stack(&mut state, ExploreType::Industrial, &[Explore::FunctioningBase1]);
    state.decks_mut().action.set(vec![ActionCard::Spy, ActionCard::Rout]);
    let mut table = explore_table();
    table.facts.card_groups = (0..ActionCard::COUNT as u8).collect();
    let mut rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Industrial), false)).with_table(table);
    rig.run();
    assert_eq!(rig.opts(), vec![ex(EOpt::Gain), ex(EOpt::SpendTradeGood), ex(EOpt::SpendCommodity)]);
    rig.pick_opt(ex(EOpt::SpendTradeGood));
    assert!(rig.is_done(), "{}", rig.transcript());
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!((p.econ.trade_goods, p.cards.hand.as_slice()), (1, &[ActionCard::Spy][..]));
    assert!(row(&rig, 0).0 == "Resolved");
}

#[test]
fn local_fabricators_place_a_mech_and_charge_only_when_one_is_placed() {
    covers!("rules::explore_windows::Exploring");
    let spot = planet_with(&[PlanetTrait::Industrial]);
    let make = |mechs_left: bool| {
        let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
        let s = Setup { scenario: s.scenario.goods(0, 1), ..s };
        let (mut state, kernel) = s.scenario.build();
        stack(&mut state, ExploreType::Industrial, &[Explore::LocalFabricators1]);
        if !mechs_left {
            // All the mechs of the seat stand on the board already.
            let n = Faction::TheArborec.data().units.iter().filter(|u| u.data().base_type == UnitBaseType::Mech).count().max(1);
            let _ = n;
            for _ in 0..4 {
                state.board_mut().sys_mut(spot.0).units.push(Piece::new(Unit::FactionlessMech, Seat(0)));
            }
        }
        let mut table = explore_table();
        table.facts.roster = Some(Box::leak(Box::new(ti4_engine_core::query::tech::UnitRoster::default())));
        Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Industrial), false)).with_table(table)
    };
    let mut rig = make(true);
    rig.run();
    assert!(rig.opts().contains(&ex(EOpt::SpendTradeGood)), "{}", rig.transcript());
    rig.pick_opt(ex(EOpt::SpendTradeGood));
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(goods(&rig), 0);
    assert!(rig.state.board().planet(spot.1).units.as_slice().iter().any(|p| p.owner() == Seat(0) && p.kind().data().base_type == UnitBaseType::Mech));
    assert!(row(&rig, 0).1.contains("goods 1>0"), "{:?}", row(&rig, 0));
}

// ---- more arms --------------------------------------------------------------------------------------------

#[test]
fn a_merchant_station_a_mercenary_outfit_and_a_mech_or_infantry_that_pays() {
    covers!("rules::explore_windows::Exploring");
    let spot = planet_with(&[PlanetTrait::Industrial]);
    // Merchant Station: replenish.
    let (mut rig, _, _) = quick(spot, ExploreType::Industrial, &[Explore::MerchantStation1]);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities, i16::from(Faction::TheArborec.data().commodities));
    // Mercenary Outfit: place one infantry or nothing.
    let make = || {
        let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
        let (mut state, kernel) = s.scenario.build();
        stack(&mut state, ExploreType::Industrial, &[Explore::MercenaryOutfit1]);
        let mut table = explore_table();
        table.facts.roster = Some(Box::leak(Box::new(ti4_engine_core::query::tech::UnitRoster::default())));
        Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Industrial), false)).with_table(table)
    };
    let mut rig = make();
    rig.run();
    assert_eq!(rig.opts(), vec![ex(EOpt::Place), ex(EOpt::PlaceNothing)]);
    rig.pick_opt(ex(EOpt::Place));
    assert_eq!(rig.state.board().planet(spot.1).units.as_slice().iter().filter(|p| p.owner() == Seat(0) && p.kind().data().base_type == UnitBaseType::Infantry).count(), 1);
    let mut rig = make();
    rig.run();
    rig.pick_opt(ex(EOpt::PlaceNothing));
    assert!(rig.is_done() && rig.state.board().planet(spot.1).units.is_empty());
    // A mech pays without a question; an infantry only when removed; the three rewards.
    for (card, check) in [(Explore::CoreMine1, 0u8), (Explore::Expedition1, 1), (Explore::VolatileFuelSource1, 2)] {
        let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
        let sc = s.scenario.unit(spot.0, Some(spot.1), Unit::InfantryI, 0, 1).exhaust(spot.1);
        let (mut state, kernel) = sc.build();
        stack(&mut state, ExploreType::Industrial, &[card]);
        let mut rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Industrial), false)).with_table(explore_table());
        rig.run();
        assert!(matches!(rig.asked().ask, Ask::Explore(ti4_engine_core::choice::explore::Ask::PayInfantry { .. })), "{}", rig.transcript());
        rig.pick_opt(ex(EOpt::RemoveInfantry));
        if check == 2 {
            // The command token of a Volatile Fuel Source goes into a pool of the seat's choice.
            assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { left: 1 })), "{}", rig.transcript());
            rig.pick_opt(Opt::Strategy(SOpt::Pool(Pool::Tactic)));
        }
        assert!(rig.is_done(), "{}", rig.transcript());
        let p = rig.state.players().by_seat(Seat(0)).unwrap();
        match check {
            0 => assert_eq!(p.econ.trade_goods, 1),
            1 => assert!(!rig.state.board().planet(spot.1).flags.has(PlanetFlags::EXHAUSTED), "the planet is readied"),
            _ => assert_eq!(p.econ.tactic_tokens, 3),
        }
        assert!(rig.state.board().planet(spot.1).units.is_empty(), "the infantry was removed");
        // Declining the removal leaves the card unrewarded.
        let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
        let (mut state, kernel) = s.scenario.unit(spot.0, Some(spot.1), Unit::InfantryI, 0, 1).build();
        stack(&mut state, ExploreType::Industrial, &[card]);
        let mut rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Industrial), false)).with_table(explore_table());
        rig.run();
        rig.pick_opt(ex(EOpt::KeepInfantry));
        assert!(rig.is_done() && goods(&rig) == 0 && rig.state.board().planet(spot.1).units.len() == 1);
    }
}

#[test]
fn the_ion_storm_and_the_mirage_name_the_system_of_the_planet() {
    covers!("rules::explore_windows::Exploring");
    let spot = planet_with(&[PlanetTrait::Hazardous]);
    let (mut rig, _, _) = quick(spot, ExploreType::Hazardous, &[Explore::IonStorm]);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Explore(ti4_engine_core::choice::explore::Ask::IonStorm)));
    rig.pick_opt(ex(EOpt::Face(Wormhole::Beta)));
    assert!(rig.is_done());
    assert_eq!(rig.state.meta().ion_storm, Some((spot.0, Wormhole::Beta)));
    assert!(row(&rig, 0).1.contains(&format!("ion Some(({:?}, Beta))", spot.0)), "{:?}", row(&rig, 0));
    // The same side in the same system again is no new token.
    let (mut again, _, _) = quick(spot, ExploreType::Hazardous, &[Explore::IonStorm]);
    again.state.meta_mut().ion_storm = Some((spot.0, Wormhole::Beta));
    again.run();
    again.pick_opt(ex(EOpt::Face(Wormhole::Beta)));
    assert!(row(&again, 0).1.contains("ion None"), "{:?}", row(&again, 0));
    // The Mirage from a planet goes into the planet's system.
    let (mut rig, _, _) = quick(spot, ExploreType::Hazardous, &[Explore::Mirage]);
    rig.run();
    assert!(rig.is_done());
    let cell = rig.state.board().planet(Planet::Mirage);
    assert_eq!((cell.placed_in, cell.control), (Some(spot.0), Some(Seat(0))));
    // A gamma token needs an active system.
    let (mut rig, _, _) = quick(spot, ExploreType::Hazardous, &[Explore::GammaWormhole]);
    rig.state.meta_mut().active_system = Some(spot.0);
    rig.run();
    assert_eq!(rig.state.explore().wormhole_tokens.get(&Wormhole::Gamma), Some(&spot.0));
}

#[test]
fn distant_suns_draws_a_second_card_before_the_row_is_opened() {
    covers!("rules::explore_windows::Exploring");
    let spot = planet_with(&[PlanetTrait::Cultural]);
    let s = setup(spot, &[Faction::TheNaazRokhaAlliance, Faction::TheArborec]);
    let sc = s.scenario.kernel(kernel_all()).unit(spot.0, Some(spot.1), Unit::Eidolon, 0, 1);
    let (mut state, kernel) = sc.build();
    stack(&mut state, ExploreType::Cultural, &[Explore::CulturalRelicFragment1, Explore::CulturalRelicFragment2, Explore::CulturalRelicFragment3]);
    let mut rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Cultural), false)).with_table(explore_table());
    rig.run();
    // The hook needs a Naaz mech on the planet: if the scenario's mech form does not satisfy it the draw is plain, which the assertion names.
    assert!(matches!(rig.asked().ask, Ask::Explore(ti4_engine_core::choice::explore::Ask::DistantSuns { .. })), "{}", rig.transcript());
    assert!(rig.state.explore().log.is_empty(), "no row before the card to resolve is known");
    rig.pick(1);
    assert!(rig.is_done());
    let log: Vec<_> = rig.state.explore().log.iter().collect();
    assert_eq!((log.len(), log[0].card), (1, Explore::CulturalRelicFragment2));
    assert_eq!(rig.state.decks().explore[ExploreType::Cultural as usize].len(), 1, "both cards left the deck");
}

#[test]
fn what_the_window_does_not_carry_is_named() {
    covers!("rules::explore_windows::Exploring");
    use ti4_engine_core::state::EWhy;
    let spot = planet_with(&[PlanetTrait::Cultural]);
    // Freelancers produce a unit: a table with no faction unit roster (or no link to the production area) cannot name the units it may build.
    let (mut rig, _, _) = quick(spot, ExploreType::Cultural, &[Explore::Freelancers1]);
    rig.run();
    assert!(rig.is_done() && rig.driver.window.unsupported() == Some(EWhy::NoLink), "{}", rig.transcript());
    assert!(!rig.state.meta().influence_pays_for_units.contains(Seat(0)), "the permission to pay with influence is not left behind");
    // A Titans seat that announces explores like any other: Terragenesis is a rule of the table (the registry), not of the window.
    let s = setup(spot, &[Faction::TheTitansOfUl, Faction::TheArborec]);
    let (mut rig, _, _) = build(s, ExploreType::Cultural, &[Explore::CulturalRelicFragment1], true, EDeck::Fixed(ExploreType::Cultural));
    rig.run();
    assert!(rig.is_done() && rig.driver.window.unsupported().is_none() && rig.driver.window.explored());
    // Without the texts of the log row (a table the adapter did not fill).
    let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let (mut state, kernel) = s.scenario.build();
    stack(&mut state, ExploreType::Cultural, &[Explore::CulturalRelicFragment1]);
    let mut rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Cultural), false));
    rig.run();
    assert!(rig.is_done() && rig.driver.window.unsupported() == Some(EWhy::NoTexts));
}

#[test]
fn a_derelict_vessel_draws_a_secret_and_a_lost_crew_two_cards() {
    covers!("rules::explore_windows::Exploring");
    let spot = planet_with(&[PlanetTrait::Hazardous]);
    let (mut rig, _, _) = quick(spot, ExploreType::Hazardous, &[Explore::DerelictVessel1]);
    rig.state.decks_mut().secret.set(vec![SecretObjective::ALL[0]]);
    rig.run();
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.secrets.as_slice(), &[SecretObjective::ALL[0]]);
    let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let (mut state, kernel) = s.scenario.build();
    stack(&mut state, ExploreType::Hazardous, &[Explore::LostCrew1]);
    state.decks_mut().action.set(vec![ActionCard::Spy, ActionCard::Rout]);
    let mut table = explore_table();
    table.facts.card_groups = (0..ActionCard::COUNT as u8).collect();
    let mut rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Hazardous), false)).with_table(table);
    rig.run();
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 2);
}

/// A host that keeps the exploration between two questions stores the run and resumes it with `Exploring::hosted`: the questions and the
/// position are the same as the window that is kept in memory.
#[test]
fn a_run_stored_after_every_question_resumes_to_the_same_end() {
    covers!("rules::explore_windows::Exploring");
    let spot = planet_with(&[PlanetTrait::Hazardous]);
    // The window kept in memory.
    let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let (mut rig, _, _) = build(s, ExploreType::Hazardous, &[Explore::MinorEntropicField], true, EDeck::Fixed(ExploreType::Hazardous));
    rig.run();
    let asked = rig.asked().clone();
    // The stored run, resumed: the question is the same and the answer ends it.
    let run = rig.driver.window.run().clone();
    let resumed = Exploring::hosted(run);
    assert_eq!(resumed.ask_of(&rig.state, &rig.kernel).as_ref().map(|c| (&c.ask, &c.opts)), Some((&asked.ask, &asked.opts)));
    let mut state2 = rig.state.clone();
    let mut driver2 = Driver::new(resumed, ti4_engine_core::kernel::Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), explore_table(), 1);
    assert!(driver2.restore_stage(&state2, &rig.kernel), "the stored run is at its question");
    let status = driver2.resume(&mut state2, &rig.kernel, Pick::Opt(Opt::Strategy(SOpt::Pool(Pool::Strategic)))).expect("the stored run takes the answer");
    rig.pick_opt(Opt::Strategy(SOpt::Pool(Pool::Strategic)));
    assert!(rig.is_done() && matches!(status, Status::Done));
    assert_eq!(delta(&rig.state, &state2), "", "the resumed run ends where the kept one does");
}

/// A host that keeps the run in a payload rebuilds the window at every call: a child of another area started by an answer runs at once (it
/// would be lost otherwise: the card was closed without its draw), and one that asks (the discard of a draw over the hand limit) puts its question
/// to the tape: the step is replayed with the answer, and the exploration ends like any other.
#[test]
fn a_hosted_exploration_runs_a_child_that_does_not_ask_at_once_and_puts_one_that_does_to_the_tape() {
    covers!("rules::explore_windows::Exploring");
    let spot = planet_with(&[PlanetTrait::Industrial]);
    let make = |hand: usize| {
        let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
        let s = Setup { scenario: s.scenario.goods(0, 2), ..s };
        let (mut state, kernel) = s.scenario.build();
        stack(&mut state, ExploreType::Industrial, &[Explore::FunctioningBase1]);
        state.decks_mut().action.set(vec![ActionCard::Spy, ActionCard::Rout]);
        for card in ActionCard::ALL.iter().copied().skip(10).take(hand) {
            state.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(card);
        }
        let mut table = explore_table();
        table.facts.card_groups = (0..ActionCard::COUNT as u8).collect();
        ti4_engine_core::driver::children::register(&mut table);
        (state, kernel, table)
    };
    for (hand, asks) in [(0, false), (7, true)] {
        let (mut state, kernel, table) = make(hand);
        let mut driver = Driver::new(Exploring::hosted(ti4_engine_core::state::ExploreRun::new(Seat(0), ti4_engine_core::state::ETarget::Planet(spot.1), EDeck::Fixed(ExploreType::Industrial), false)), ti4_engine_core::kernel::Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), table.clone(), 1);
        let Status::Asking(_) = driver.run(&mut state, &kernel).unwrap() else { panic!("the base asks") };
        // The host stores the run, drops the window and resumes it at the answer.
        let run = driver.window.run().clone();
        let mut resumed = Driver::new(Exploring::hosted(run), ti4_engine_core::kernel::Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), table, 1);
        assert!(resumed.restore_stage(&state, &kernel));
        let status = resumed.resume(&mut state, &kernel, Pick::Opt(ex(EOpt::SpendTradeGood))).unwrap();
        assert_eq!(resumed.window.unsupported(), None, "hand of {hand}");
        if asks {
            // The discard is a question of the step (nested, in the journal); the draw is rolled back with the step until it is answered.
            let Status::Asking(discard) = status else { panic!("a hand over the limit discards") };
            assert!(!discard.opts.as_slice().is_empty());
            assert_eq!(discard.ctx.invasion_seq, None, "outside an invasion");
            let held = resumed.nested_journal().expect("the suspension");
            let mut again = Driver::new(Exploring::hosted(resumed.window.run().clone()), ti4_engine_core::kernel::Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), explore_table_with_links(), 1);
            again.restore_nested(&held);
            let Status::Asking(same) = again.reask(&mut state, &kernel).unwrap() else { panic!("asks again") };
            assert_eq!(same, discard, "a rebuilt window asks the same question");
            let pick = Pick::Opt(discard.opts.as_slice()[0]);
            assert!(matches!(again.resume(&mut state, &kernel, pick).unwrap(), Status::Done), "the discard ends the exploration");
            assert!(again.window.explored() && again.window.unsupported().is_none());
        } else {
            assert!(matches!(status, Status::Done));
        }
        let held = state.players().by_seat(Seat(0)).unwrap().cards.hand.len();
        assert_eq!(held, if asks { 7 } else { 1 }, "the draw is kept, a hand over the limit gives one back (hand of {hand})");
    }
}

fn explore_table_with_links() -> ti4_engine_core::kernel::stage::AbilityTable {
    let mut table = explore_table();
    table.facts.card_groups = (0..ActionCard::COUNT as u8).collect();
    ti4_engine_core::driver::children::register(&mut table);
    table
}

/// Every question asked while an invasion is open names it, the pool question of a command token included (the string `ask_seeing`).
#[test]
fn a_question_asked_inside_an_invasion_carries_its_sequence_number() {
    covers!("rules::explore_windows::Exploring");
    use ti4_engine_core::containers::Cow;
    use ti4_engine_core::state::{ActiveInvasion, Code};
    let spot = planet_with(&[PlanetTrait::Hazardous]);
    let (mut rig, system, planet) = quick(spot, ExploreType::Hazardous, &[Explore::EntropicField]);
    rig.state.transient_mut().invasion.active = Some(Cow::new(ActiveInvasion { system, invader: Seat(0), seq: 7, phase: Code(1), planet: Some(planet), defender: None, ground_round: 0, last_step: None }));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { .. })));
    assert_eq!(rig.asked().ctx.invasion_seq, Some(7), "{}", rig.transcript());
    // Outside an invasion the question carries none.
    let (mut plain, _, _) = quick(spot, ExploreType::Hazardous, &[Explore::EntropicField]);
    plain.run();
    assert_eq!(plain.asked().ctx.invasion_seq, None);
}

// ---- mutation survivors (package X) -------------------------------------------------------------------------

fn holdings_setup(version: u32, goods: i16, commodities: i16, faction: Faction) -> Setup {
    let s = setup(planet_with(&[PlanetTrait::Industrial]), &[faction, Faction::TheEmbersOfMuaat]);
    Setup { scenario: s.scenario.rules(version).goods(0, goods).commodities(0, commodities), ..s }
}

fn with_roster(mut rig: Rig<Exploring>) -> Rig<Exploring> {
    let mut table = explore_table();
    let mut roster = ti4_engine_core::query::tech::UnitRoster::default();
    roster.set(Faction::TheNaazRokhaAlliance, UnitBaseType::Mech, Some(Unit::Eidolon));
    table.facts.roster = Some(Box::leak(Box::new(roster)));
    table.facts.card_groups = (0..ActionCard::COUNT as u8).collect();
    rig = rig.with_table(table);
    rig
}

#[test]
fn a_table_missing_the_texts_or_the_words_declines_and_the_gates_read_their_own_condition() {
    covers!("rules::explore_windows::Exploring");
    use ti4_engine_core::state::EWhy;
    let spot = planet_with(&[PlanetTrait::Cultural]);
    for (filled, words) in [(true, false), (false, true)] {
        let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
        let (mut state, kernel) = s.scenario.build();
        stack(&mut state, ExploreType::Cultural, &[Explore::CulturalRelicFragment1]);
        let mut table = explore_table();
        table.codes.explore.filled = filled;
        if !words {
            table.facts.explore_words = None;
        }
        let mut rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Cultural), false)).with_table(table);
        rig.run();
        assert_eq!(rig.driver.window.unsupported(), Some(EWhy::NoTexts), "filled {filled} words {words}");
    }
    // A Titans seat explores like any other.
    let s = setup(spot, &[Faction::TheTitansOfUl, Faction::TheArborec]);
    let (mut quiet, _, _) = build(s, ExploreType::Cultural, &[Explore::CulturalRelicFragment1], false, EDeck::Fixed(ExploreType::Cultural));
    quiet.run();
    assert!(quiet.driver.window.explored() && quiet.driver.window.unsupported().is_none(), "no timing handle: nothing to react to");
    let sys = plain_systems()[0];
    let (mut state, kernel) = Scenario::new(&[Faction::TheTitansOfUl, Faction::TheArborec]).map_from(&[sys]).build();
    stack(&mut state, ExploreType::Frontier, &[Explore::UnknownRelicFragment1]);
    state.explore_mut().frontier.insert(sys);
    let mut frontier = Rig::from_parts(state, kernel, Exploring::frontier(Seat(0), sys, true, true)).with_table(explore_table());
    frontier.run();
    assert!(frontier.driver.window.explored(), "a frontier token has no planet to name");
}

#[test]
fn every_entropic_field_and_ship_gives_its_tokens_and_goods() {
    covers!("rules::explore_windows::Exploring");
    for (card, tokens, goods) in [(Explore::MinorEntropicField, 1, 1), (Explore::EntropicField, 1, 2), (Explore::MajorEntropicField, 1, 3), (Explore::KeleresShip1, 2, 0), (Explore::KeleresShip2, 2, 0)] {
        // Before version 12 the tokens go to the strategy pool unasked.
        let (mut rig, _, _) = build(holdings_setup(11, 0, 0, Faction::TheArborec), ExploreType::Hazardous, &[card], false, EDeck::Fixed(ExploreType::Hazardous));
        rig.run();
        assert!(rig.is_done(), "{card:?}");
        assert_eq!((goods_of_seat(&rig), rig.state.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens), (goods, 2 + tokens), "{card:?}");
    }
    // From version 12 each token is a question: a Keleres ship asks twice, with the number left.
    let (mut rig, _, _) = build(holdings_setup(LATEST_RULES, 0, 0, Faction::TheArborec), ExploreType::Hazardous, &[Explore::KeleresShip1], false, EDeck::Fixed(ExploreType::Hazardous));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { left: 2 })));
    rig.pick_opt(Opt::Strategy(SOpt::Pool(Pool::Tactic)));
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { left: 1 })));
    rig.pick_opt(Opt::Strategy(SOpt::Pool(Pool::Fleet)));
    assert!(rig.is_done());
}

fn goods_of_seat(rig: &Rig<Exploring>) -> i16 {
    rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods
}

#[test]
fn the_options_of_a_functioning_base_and_local_fabricators_follow_what_the_seat_holds() {
    covers!("rules::explore_windows::Exploring");
    let opts = |card: Explore, goods: i16, commodities: i16, faction: Faction| {
        let (rig, system, _) = build(holdings_setup(LATEST_RULES, goods, commodities, faction), ExploreType::Industrial, &[card], false, EDeck::Fixed(ExploreType::Industrial));
        let mut rig = with_roster(rig);
        if faction == Faction::TheNaazRokhaAlliance {
            // The Eidolon Maximum on the board bars placing a mech by an effect.
            rig.state.board_mut().sys_mut(system).units.push(Piece::new(Unit::EidolonMaximum, Seat(0)));
        }
        rig.run();
        assert!(matches!(rig.asked().ask, Ask::Explore(ti4_engine_core::choice::explore::Ask::Reward(_))), "a single option is still asked: {}", rig.transcript());
        rig.opts()
    };
    for card in [Explore::FunctioningBase1, Explore::LocalFabricators1] {
        assert_eq!(opts(card, 0, 0, Faction::TheArborec), vec![ex(EOpt::Gain)], "{card:?}");
        assert_eq!(opts(card, 1, 0, Faction::TheArborec), vec![ex(EOpt::Gain), ex(EOpt::SpendTradeGood)], "{card:?}");
        assert_eq!(opts(card, 0, 1, Faction::TheArborec), vec![ex(EOpt::Gain), ex(EOpt::SpendCommodity)], "{card:?}");
    }
    // A seat whose faction bars placing its mech by an effect is not offered the options that place one.
    assert_eq!(opts(Explore::LocalFabricators1, 2, 2, Faction::TheNaazRokhaAlliance), vec![ex(EOpt::Gain)]);
    assert_eq!(opts(Explore::FunctioningBase1, 2, 2, Faction::TheNaazRokhaAlliance).len(), 3, "the base places nothing");
}

#[test]
fn spending_a_commodity_and_gaining_one_are_exact() {
    covers!("rules::explore_windows::Exploring");
    let run = |card: Explore, pick: EOpt| {
        let (rig, _, planet) = build(holdings_setup(LATEST_RULES, 0, 2, Faction::TheArborec), ExploreType::Industrial, &[card], false, EDeck::Fixed(ExploreType::Industrial));
        let mut rig = with_roster(rig);
        rig.state.decks_mut().action.set(vec![ActionCard::Spy, ActionCard::Rout]);
        rig.run();
        rig.pick_opt(ex(pick));
        assert!(rig.is_done(), "{}", rig.transcript());
        (rig, planet)
    };
    let (rig, _) = run(Explore::FunctioningBase1, EOpt::SpendCommodity);
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!((p.econ.commodities, p.cards.hand.len()), (1, 1));
    let (rig, planet) = run(Explore::LocalFabricators1, EOpt::SpendCommodity);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities, 1);
    assert_eq!(rig.state.board().planet(planet).units.as_slice().iter().filter(|u| u.kind().data().base_type == UnitBaseType::Mech).count(), 1);
    for card in [Explore::FunctioningBase1, Explore::LocalFabricators1] {
        let (rig, _) = run(card, EOpt::Gain);
        assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities, 3, "{card:?}: one commodity");
    }
}

#[test]
fn a_lost_crew_draws_exactly_two_and_a_mercenary_without_unit_forms_is_named() {
    covers!("rules::explore_windows::Exploring");
    use ti4_engine_core::state::EWhy;
    let spot = planet_with(&[PlanetTrait::Hazardous]);
    let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let (mut state, kernel) = s.scenario.build();
    stack(&mut state, ExploreType::Hazardous, &[Explore::LostCrew1]);
    state.decks_mut().action.set(vec![ActionCard::Spy, ActionCard::Rout, ActionCard::Veto, ActionCard::Plague]);
    let mut table = explore_table();
    table.facts.card_groups = (0..ActionCard::COUNT as u8).collect();
    let mut rig = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Hazardous), false)).with_table(table);
    rig.run();
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 2);
    // The window itself, not hosted, asks the discard of a draw over the limit.
    let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let (mut state, kernel) = s.scenario.build();
    stack(&mut state, ExploreType::Hazardous, &[Explore::LostCrew1]);
    state.decks_mut().action.set(vec![ActionCard::Spy, ActionCard::Rout]);
    for card in ActionCard::ALL.iter().copied().skip(10).take(7) {
        state.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(card);
    }
    let mut table = explore_table();
    table.facts.card_groups = (0..ActionCard::COUNT as u8).collect();
    let mut over = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Hazardous), false)).with_table(table);
    over.run();
    assert!(matches!(over.asked().ask, Ask::Cards(_)), "a window held in memory asks the discard: {}", over.transcript());
    // A Mercenary Outfit needs the unit forms of the scope.
    let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let (mut state, kernel) = s.scenario.build();
    stack(&mut state, ExploreType::Hazardous, &[Explore::MercenaryOutfit1]);
    let mut bare = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Hazardous), false)).with_table(explore_table());
    bare.run();
    bare.pick_opt(ex(EOpt::Place));
    assert_eq!(bare.driver.window.unsupported(), Some(EWhy::NoLink));
}

#[test]
fn a_zero_conversion_announces_nothing_and_a_fuel_source_pays_no_goods() {
    covers!("rules::explore_windows::Exploring");
    // Version 18 still offers the conversion to a seat without commodities: the gain is zero and nothing is announced.
    let (mut rig, _, _) = build(holdings_setup(18, 0, 0, Faction::TheArborec), ExploreType::Industrial, &[Explore::AbandonedWarehouses1], true, EDeck::Fixed(ExploreType::Industrial));
    rig.run();
    rig.pick_opt(ex(EOpt::Convert));
    assert!(rig.is_done());
    assert!(rig.driver.emitted.iter().all(|e| e.kind != EventKind::TradeGoodsGained));
    // A Volatile Fuel Source pays a command token and no trade good.
    let spot = planet_with(&[PlanetTrait::Industrial]);
    let sc = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let (mut state, kernel) = sc.scenario.unit(spot.0, Some(spot.1), Unit::InfantryI, 0, 1).build();
    stack(&mut state, ExploreType::Industrial, &[Explore::VolatileFuelSource1]);
    let mut fuel = Rig::from_parts(state, kernel, Exploring::planet(Seat(0), spot.1, EDeck::Fixed(ExploreType::Industrial), true)).with_table(explore_table());
    fuel.run();
    fuel.pick_opt(ex(EOpt::RemoveInfantry));
    fuel.pick_opt(Opt::Strategy(SOpt::Pool(Pool::Tactic)));
    assert!(fuel.is_done());
    assert_eq!(goods_of_seat(&fuel), 0);
    assert!(fuel.driver.emitted.iter().all(|e| e.kind != EventKind::TradeGoodsGained));
}

#[test]
fn the_last_survivors_a_table_without_unit_forms_a_one_good_conversion_and_a_gain_from_nothing() {
    covers!("rules::explore_windows::Exploring");
    use ti4_engine_core::state::EWhy;
    // No unit forms in the table: the placing options are offered (nothing can be said to forbid), placing is named, and nothing is charged.
    let (mut rig, _, _) = build(holdings_setup(LATEST_RULES, 1, 0, Faction::TheArborec), ExploreType::Industrial, &[Explore::LocalFabricators1], false, EDeck::Fixed(ExploreType::Industrial));
    rig.run();
    assert_eq!(rig.opts(), vec![ex(EOpt::Gain), ex(EOpt::SpendTradeGood)]);
    rig.pick_opt(ex(EOpt::SpendTradeGood));
    assert_eq!(rig.driver.window.unsupported(), Some(EWhy::NoLink));
    assert_eq!(goods_of_seat(&rig), 1, "no good is spent on a mech that cannot be placed");
    // One commodity converts to one announced good.
    let (mut conv, _, _) = build(holdings_setup(LATEST_RULES, 0, 1, Faction::TheArborec), ExploreType::Industrial, &[Explore::AbandonedWarehouses1], true, EDeck::Fixed(ExploreType::Industrial));
    conv.run();
    conv.pick_opt(ex(EOpt::Convert));
    assert_eq!(conv.driver.emitted.iter().filter(|e| e.kind == EventKind::TradeGoodsGained).count(), 1);
    // A gain is one commodity from none.
    for card in [Explore::FunctioningBase1, Explore::LocalFabricators1] {
        let (rig, _, _) = build(holdings_setup(LATEST_RULES, 0, 0, Faction::TheArborec), ExploreType::Industrial, &[card], false, EDeck::Fixed(ExploreType::Industrial));
        let mut rig = with_roster(rig);
        rig.run();
        rig.pick_opt(ex(EOpt::Gain));
        assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities, 1, "{card:?}");
    }
}

/// The question of a linked child that asks inside a hosted exploration names the invasion the position is in, like every other question of the window.
#[test]
fn a_discard_put_to_the_tape_names_the_open_invasion() {
    covers!("rules::explore_windows::Exploring");
    use ti4_engine_core::containers::Cow;
    use ti4_engine_core::state::{ActiveInvasion, Code};
    let spot = planet_with(&[PlanetTrait::Industrial]);
    let s = setup(spot, &[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    let s = Setup { scenario: s.scenario.goods(0, 2), ..s };
    let (system, planet) = (s.system, s.planet);
    let (mut state, kernel) = s.scenario.build();
    stack(&mut state, ExploreType::Industrial, &[Explore::FunctioningBase1]);
    state.decks_mut().action.set(vec![ActionCard::Spy, ActionCard::Rout]);
    for card in ActionCard::ALL.iter().copied().skip(10).take(7) {
        state.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(card);
    }
    state.transient_mut().invasion.active = Some(Cow::new(ActiveInvasion { system, invader: Seat(0), seq: 5, phase: Code(1), planet: Some(planet), defender: None, ground_round: 0, last_step: None }));
    let table = explore_table_with_links();
    let mk = |run| Driver::new(Exploring::hosted(run), ti4_engine_core::kernel::Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), table.clone(), 1);
    let mut driver = mk(ti4_engine_core::state::ExploreRun::new(Seat(0), ti4_engine_core::state::ETarget::Planet(planet), EDeck::Fixed(ExploreType::Industrial), false));
    let Status::Asking(_) = driver.run(&mut state, &kernel).unwrap() else { panic!("the base asks") };
    let mut resumed = mk(driver.window.run().clone());
    assert!(resumed.restore_stage(&state, &kernel));
    let Status::Asking(discard) = resumed.resume(&mut state, &kernel, Pick::Opt(ex(EOpt::SpendTradeGood))).unwrap() else { panic!("a discard") };
    assert_eq!(discard.ctx.invasion_seq, Some(5));
}
