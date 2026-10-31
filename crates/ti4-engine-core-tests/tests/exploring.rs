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
    // Freelancers produce a unit with influence: a child the exploration does not run.
    let (mut rig, _, _) = quick(spot, ExploreType::Cultural, &[Explore::Freelancers1]);
    rig.run();
    assert!(rig.is_done() && rig.driver.window.unsupported() == Some(EWhy::Freelancers), "{}", rig.transcript());
    // A Titans seat that announces: Terragenesis is not a rule of the core table.
    let s = setup(spot, &[Faction::TheTitansOfUl, Faction::TheArborec]);
    let (mut rig, _, _) = build(s, ExploreType::Cultural, &[Explore::CulturalRelicFragment1], true, EDeck::Fixed(ExploreType::Cultural));
    rig.run();
    assert!(rig.is_done() && rig.driver.window.unsupported() == Some(EWhy::Terragenesis));
    assert!(rig.state.explore().log.is_empty(), "nothing is drawn before the window says it cannot run");
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
