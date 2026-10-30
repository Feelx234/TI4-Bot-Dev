//! D4 lane A3: the ported action cards that lanes A1 and A2 did not play, against their printed text. The first batch: the cards that name a player,
//! an agenda outcome, a law, a strategy card, a planet or a promissory note. Each test builds the position the card needs, plays the card through
//! the real window and checks what the card says changed (and that nothing else did).

use ti4_engine_core::choice::cards::Opt as CO;
use ti4_engine_core::containers::{Deck, SmallList};
use ti4_engine_core::generated::{ActionCard, Agenda, Explore, ExploreType, Planet, PromissoryNote, StrategyCard, Unit};
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Prediction, PredictionTag, VoteBar, Word};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::rules::action_cards::effects::Play;
use ti4_engine_core::rules::action_cards::{Env, Facts, Placed};
use ti4_engine_core::state::{Code, NoteKey, Outcome};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn four() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheWinnu]).map_ring(7)
}

fn linked() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut t);
    t
}

fn env() -> Env {
    Env {
        mahact_held: 0,
        scope: ALL_SOURCES,
        blanked: false,
        action_card: Code(0),
        supply_base: Code(0),
        note_order: |a, b| a.cmp(&b),
        infantry: Placed { unit: Some(Unit::InfantryI), on_card: 0 },
        cruiser: Placed { unit: Some(Unit::CruiserI), on_card: 0 },
        fighter: Placed { unit: Some(Unit::FighterI), on_card: 0 },
        facts: Facts { neutral: Some(Seat(1)), action_draw_plain: true, rules_version: 35, ..Facts::default() },
    }
}

fn solo(card: ActionCard, st: CoreState, seed: u64) -> Option<Rig<Play>> {
    let play = Play::of(&st, card, Seat(0), env())?;
    Some(Rig::from_parts(st, ti4_engine_core_tests::kernel_all(), play).with_table(linked()).seeded(seed))
}

fn rig(card: ActionCard, st: CoreState) -> Rig<Play> {
    solo(card, st, 1).unwrap_or_else(|| panic!("{card:?} has a play"))
}

use ti4_engine_core::choice::cards::{Ask as CAsk, What};

fn what(r: &Rig<Play>) -> What {
    match &r.asked().ask {
        Ask::Cards(CAsk::Pick { what, .. }) => *what,
        other => panic!("not a pick: {other:?}"),
    }
}

fn o(x: CO) -> Opt {
    Opt::Cards(x)
}

fn seats_of(rig: &Rig<Play>) -> Vec<u8> {
    rig.opts().iter().filter_map(|x| if let Opt::Cards(CO::Seat(s)) = x { Some(s.0) } else { None }).collect()
}

fn sys(i: usize) -> System {
    plain_systems()[i]
}

fn planet(i: usize) -> Planet {
    sys(i).data().planets[0]
}

fn goods(s: &CoreState, seat: u8) -> i16 {
    s.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
}

// ---- Sabotage, Assassinate Representative, Confusing Legal Text --------------------------------------------

#[test]
fn sabotage_has_no_effect_of_its_own() {
    covers_card!(ActionCard::Sabotage1);
    for card in [ActionCard::Sabotage1, ActionCard::Sabotage2, ActionCard::Sabotage3, ActionCard::Sabotage4] {
        let st = four().hand(0, &[card]).state();
        let mut r = rig(card, st.clone());
        r.run();
        assert!(r.is_done(), "{card:?}: a counter, nothing to resolve");
        assert!(r.state == st, "{card:?}: nothing changed");
    }
}

#[test]
fn assassinate_representative_bars_the_chosen_player_from_voting_unless_they_predicted() {
    covers_card!(ActionCard::AssassinateRepresentative);
    let mut r = rig(ActionCard::AssassinateRepresentative, four().state());
    r.run();
    assert_eq!(what(&r), What::Player);
    assert_eq!(seats_of(&r), vec![0, 1, 2, 3], "every player, the holder too");
    r.assert_options_sound();
    r.pick_opt(o(CO::Seat(Seat(2))));
    assert!(r.is_done());
    assert_eq!(r.state.agenda().predictions.get(&Seat(2)), Some(&Prediction::Barred(VoteBar::Assassin)));
    assert_eq!(r.state.agenda().predictions.len(), 1);
    // A player who already predicted keeps the prediction.
    let mut st = four().state();
    st.agenda_mut().predictions.insert(Seat(2), Prediction::Plain(Outcome::For));
    let mut again = rig(ActionCard::AssassinateRepresentative, st);
    again.run().pick_opt(o(CO::Seat(Seat(2))));
    assert_eq!(again.state.agenda().predictions.get(&Seat(2)), Some(&Prediction::Plain(Outcome::For)));
}

#[test]
fn confusing_legal_text_elects_the_player_the_holder_names() {
    covers_card!(ActionCard::ConfusingLegalText);
    let mut r = rig(ActionCard::ConfusingLegalText, four().state());
    r.run();
    assert_eq!(what(&r), What::Elect);
    assert_eq!(seats_of(&r), vec![1, 2, 3], "the other players");
    r.assert_options_sound();
    r.pick_opt(o(CO::Seat(Seat(3))));
    assert!(r.is_done());
    assert_eq!(r.state.agenda().elected_override, Some(Seat(3)));
}

// ---- the riders ----------------------------------------------------------------------------------------------

fn voting() -> CoreState {
    let mut st = four().state();
    st.agenda_mut().in_play = Some(Agenda::ALL[0]);
    st.agenda_mut().choices = [Outcome::For, Outcome::Against].into_iter().collect();
    st
}

#[test]
fn a_rider_predicts_an_outcome_of_the_agenda_and_the_tag_names_which_rider() {
    for (card, tag) in [
        (ActionCard::ImperialRider, None),
        (ActionCard::ConstructionRider, Some(PredictionTag::ConstRider)),
        (ActionCard::DiplomacyRider, Some(PredictionTag::DiploRider)),
        (ActionCard::LeadershipRider, Some(PredictionTag::LeadRider)),
        (ActionCard::PoliticsRider, Some(PredictionTag::PoliticRider)),
        (ActionCard::TechnologyRider, Some(PredictionTag::TechRider)),
        (ActionCard::TradeRider, Some(PredictionTag::TradeRider)),
        (ActionCard::WarfareRider, Some(PredictionTag::WarRider)),
        (ActionCard::Sanction, Some(PredictionTag::Sanction)),
    ] {
        let mut r = rig(card, voting());
        r.run();
        assert_eq!(r.opts(), vec![o(CO::Outcome(Outcome::For)), o(CO::Outcome(Outcome::Against))], "{card:?}");
        r.assert_options_sound();
        r.pick_opt(o(CO::Outcome(Outcome::Against)));
        assert!(r.is_done());
        let want = tag.map_or(Prediction::Plain(Outcome::Against), |t| Prediction::Tagged(Outcome::Against, t));
        assert_eq!(r.state.agenda().predictions.get(&Seat(0)), Some(&want), "{card:?}");
        assert_eq!(r.state.agenda().predictions.len(), 1, "{card:?}");
    }
}

#[test]
fn a_rider_is_not_played_when_the_agenda_is_unknown_and_more_than_one_could_be_meant() {
    let mut st = four().state();
    st.agenda_mut().choices = [Outcome::For, Outcome::Against].into_iter().collect();
    assert!(solo(ActionCard::ImperialRider, st.clone(), 1).is_none(), "no agenda in play but two outcomes on offer: the string's");
    // Nothing on offer at all, or a single outcome: the card is played against the first agenda's frame.
    st.agenda_mut().choices = SmallList::new();
    let mut none = rig(ActionCard::ImperialRider, st.clone());
    none.run();
    assert!(none.is_done(), "no outcomes to predict");
    assert!(none.state.agenda().predictions.is_empty());
}

// ---- the table's flags and stamps ---------------------------------------------------------------------------

#[test]
fn puppets_on_a_string_gives_the_holder_an_extra_action_only_after_passing() {
    covers_card!(ActionCard::PuppetsOnAString);
    let mut r = rig(ActionCard::PuppetsOnAString, four().state());
    r.run();
    assert!(r.is_done());
    assert!(!r.state.transient().misc.flags.has(ti4_engine_core::state::TransientFlags::PUPPET_ACTION), "not passed: nothing");
    let mut st = four().state();
    st.players_mut().by_seat_mut(Seat(0)).unwrap().econ.passed = true;
    let mut passed = rig(ActionCard::PuppetsOnAString, st);
    passed.run();
    assert!(passed.state.transient().misc.flags.has(ti4_engine_core::state::TransientFlags::PUPPET_ACTION));
}

#[test]
fn extreme_duress_marks_the_active_player_as_the_holders_victim_unless_it_is_the_holder() {
    covers_card!(ActionCard::ExtremeDuress);
    let mut r = rig(ActionCard::ExtremeDuress, four().active(Some(2)).state());
    r.run();
    assert!(r.is_done());
    assert_eq!(r.state.players().by_seat(Seat(2)).unwrap().econ.duress_by, Some(Seat(0)));
    assert_eq!(r.state.players().by_seat(Seat(1)).unwrap().econ.duress_by, None);
    let mut own = rig(ActionCard::ExtremeDuress, four().active(Some(0)).state());
    own.run();
    assert_eq!(own.state.players().by_seat(Seat(0)).unwrap().econ.duress_by, None, "the holder is not its own victim");
    let mut nobody = rig(ActionCard::ExtremeDuress, four().active(None).state());
    nobody.run();
    assert!(nobody.state.players().iter().all(|p| p.econ.duress_by.is_none()));
}

#[test]
fn veto_sets_aside_the_top_agenda_as_the_replacement() {
    covers_card!(ActionCard::Veto3);
    for card in [ActionCard::Veto, ActionCard::Veto3, ActionCard::Veto4] {
        let mut st = four().state();
        st.decks_mut().agenda = Deck::new(vec![Agenda::ALL[3], Agenda::ALL[4], Agenda::ALL[5]]);
        let mut r = rig(card, st);
        r.run();
        assert!(r.is_done());
        assert_eq!(r.state.agenda().veto_replacement, Some(Agenda::ALL[3]), "{card:?}");
        assert_eq!(r.state.decks().agenda.cards(), &[Agenda::ALL[4], Agenda::ALL[5]]);
    }
    let mut empty = rig(ActionCard::Veto, four().state());
    empty.run();
    assert_eq!(empty.state.agenda().veto_replacement, None, "an empty deck has nothing to reveal");
}

#[test]
fn insider_information_lets_the_holder_look_at_the_top_three_agendas_and_only_the_holder() {
    covers_card!(ActionCard::InsiderInformation);
    let key = |seat: u8| MarkKey::PrivateSeat(Seat(seat), Word::InsiderPeek);
    let mut st = four().state();
    st.decks_mut().agenda = Deck::new(vec![Agenda::ALL[1], Agenda::ALL[2], Agenda::ALL[3], Agenda::ALL[4]]);
    let mut r = rig(ActionCard::InsiderInformation, st);
    r.run();
    assert!(r.is_done());
    let want = MarkVal::List([Agenda::ALL[1], Agenda::ALL[2], Agenda::ALL[3]].iter().map(|a| MarkVal::Atom(Atom::Agenda(*a))).collect());
    assert_eq!(r.state.marks().tail.get(&key(0)), Some(&want));
    assert_eq!(r.state.marks().tail.get(&key(1)), None);
    assert_eq!(r.state.decks().agenda.cards().len(), 4, "the deck is not touched");
    // Two cards left: two are shown. None left: an earlier peek is erased.
    let mut two = four().state();
    two.decks_mut().agenda = Deck::new(vec![Agenda::ALL[1], Agenda::ALL[2]]);
    let mut r2 = rig(ActionCard::InsiderInformation, two);
    r2.run();
    assert!(matches!(r2.state.marks().tail.get(&key(0)), Some(MarkVal::List(l)) if l.len() == 2));
    let mut old = four().state();
    old.marks_mut().tail.edit().insert(key(0), MarkVal::List(vec![MarkVal::Atom(Atom::Agenda(Agenda::ALL[1]))]));
    let mut r3 = rig(ActionCard::InsiderInformation, old);
    r3.run();
    assert_eq!(r3.state.marks().tail.get(&key(0)), None);
}

#[test]
fn in_the_silence_of_space_lets_ships_of_a_system_move_through_enemy_ships_this_activation() {
    covers_card!(ActionCard::InTheSilenceOfSpace);
    let mut st = four().unit(sys(1), None, Unit::CruiserI, 0, 1).unit(sys(3), None, Unit::CruiserI, 0, 1).unit(sys(2), None, Unit::CruiserI, 1, 1).state();
    st.meta_mut().activation_seq = 9;
    let mut r = rig(ActionCard::InTheSilenceOfSpace, st);
    r.run();
    assert_eq!(what(&r), What::Silence);
    assert_eq!(r.opts(), vec![o(CO::System(sys(1))), o(CO::System(sys(3)))], "the systems with our ships, in id order");
    r.assert_options_sound();
    r.pick(1);
    assert!(r.is_done());
    let t = r.state.players().by_seat(Seat(0)).unwrap().card_timers.clone();
    assert_eq!(t.silence_activation.get(), Some(9));
    assert_eq!(t.silence_system, Some(sys(3)));
    assert_eq!(r.state.players().by_seat(Seat(1)).unwrap().card_timers.silence_system, None);
}

// ---- Diplomatic Pressure ---------------------------------------------------------------------------------------

#[test]
fn diplomatic_pressure_takes_a_promissory_note_from_the_hand_of_the_chosen_player_who_picks_it() {
    covers_card!(ActionCard::DiplomaticPressure1);
    let ceasefire = NoteKey { note: PromissoryNote::Ceasefire, faction: Faction::TheWinnu };
    let support = NoteKey { note: PromissoryNote::SupportForTheThrone, faction: Faction::TheFederationOfSol };
    let mut st = four().state();
    st.cards_mut().promissory.edit().insert(ceasefire, Seat(2));
    st.cards_mut().promissory.edit().insert(support, Seat(2));
    let mut r = rig(ActionCard::DiplomaticPressure1, st);
    r.run();
    assert_eq!(seats_of(&r), vec![1, 2, 3], "any other player");
    r.assert_options_sound();
    assert_eq!(what(&r), What::Player);
    r.pick_opt(o(CO::Seat(Seat(2))));
    assert_eq!(what(&r), What::Note);
    // The victim chooses which note goes.
    assert_eq!(r.asked().actor, Seat(2));
    assert_eq!(r.opts().len(), 2);
    r.assert_options_sound();
    r.pick_opt(o(CO::Note(ceasefire)));
    assert!(r.is_done());
    assert_eq!(r.state.cards().promissory.get(&ceasefire), Some(&Seat(0)), "the holder has the note");
    assert_eq!(r.state.cards().promissory.get(&support), Some(&Seat(2)));
    // A player without a note: nothing to take.
    let mut empty = rig(ActionCard::DiplomaticPressure2, four().state());
    empty.run().pick_opt(o(CO::Seat(Seat(1))));
    assert!(empty.is_done() || empty.opts().is_empty());
}

// ---- the combat cards that act on the active system ------------------------------------------------------------------

#[test]
fn intercept_bars_the_other_combatants_from_retreating_this_combat_round() {
    covers_card!(ActionCard::Intercept);
    let mut st = four().active_system(sys(1)).unit(sys(1), None, Unit::CruiserI, 0, 1).unit(sys(1), None, Unit::CruiserI, 1, 1).unit(sys(1), None, Unit::CarrierI, 2, 1).unit(sys(2), None, Unit::CruiserI, 3, 1).state();
    st.meta_mut().combat_round_seq = 4;
    let mut r = rig(ActionCard::Intercept, st.clone());
    r.run();
    assert!(r.is_done());
    let barred = |s: &CoreState, n: u8| s.players().by_seat(Seat(n)).unwrap().card_timers.retreat_barred_round.get();
    assert_eq!((barred(&r.state, 0), barred(&r.state, 1), barred(&r.state, 2), barred(&r.state, 3)), (None, Some(4), Some(4), None));
    // The holder is not in the fight: nothing is barred.
    let mut away = st.clone();
    away.board_mut().sys_mut(sys(1)).units.retain(|u| u.owner() != Seat(0));
    let mut r2 = rig(ActionCard::Intercept, away);
    r2.run();
    assert!((0..4).all(|n| barred(&r2.state, n).is_none()));
    // No active system: nothing.
    let mut idle = four().unit(sys(1), None, Unit::CruiserI, 0, 1).unit(sys(1), None, Unit::CruiserI, 1, 1).state();
    idle.meta_mut().active_system = None;
    let mut r3 = rig(ActionCard::Intercept, idle);
    r3.run();
    assert!((0..4).all(|n| barred(&r3.state, n).is_none()));
}

#[test]
fn rescue_moves_a_ship_of_ours_from_another_system_into_the_active_system_where_we_have_one() {
    covers_card!(ActionCard::Rescue);
    let st = four()
        .active(Some(1))
        .active_system(sys(1))
        .unit(sys(1), None, Unit::CruiserI, 0, 1)
        .unit(sys(2), None, Unit::DestroyerI, 0, 1)
        .unit(sys(3), None, Unit::CarrierI, 0, 1)
        .command_token(sys(3), 0) // our token: a ship there may not be rescued
        .unit(sys(4), None, Unit::FighterI, 0, 1) // a fighter is a ship
        .unit(sys(5), None, Unit::CruiserI, 1, 1)
        .state();
    let mut r = rig(ActionCard::Rescue, st.clone());
    r.run();
    assert_eq!(what(&r), What::Rescue);
    assert_eq!(r.opts(), vec![o(CO::System(sys(2))), o(CO::System(sys(4)))], "systems without our token that hold a ship of ours, not the active one");
    r.assert_options_sound();
    r.pick(0);
    assert!(r.is_done());
    let ships = |s: &CoreState, i: usize| s.board().sys(sys(i)).units.iter().filter(|u| u.owner() == Seat(0)).count();
    assert_eq!((ships(&r.state, 1), ships(&r.state, 2)), (2, 0), "the destroyer joined the cruiser");
    // We are the active player, or have no ship in the active system: nothing is offered.
    let mut own_turn = st.clone();
    own_turn.meta_mut().active = Some(Seat(0));
    let mut r2 = rig(ActionCard::Rescue, own_turn);
    r2.run();
    assert!(r2.is_done() && ships(&r2.state, 2) == 1);
    let mut alone = st.clone();
    alone.board_mut().sys_mut(sys(1)).units.retain(|u| u.owner() != Seat(0));
    let mut r3 = rig(ActionCard::Rescue, alone);
    r3.run();
    assert!(r3.is_done() && ships(&r3.state, 2) == 1);
}

// ---- trade goods, strategy cards and laws --------------------------------------------------------------------------

#[test]
fn manipulate_investments_puts_five_trade_goods_on_strategy_cards_and_at_least_three_distinct_ones() {
    covers_card!(ActionCard::ManipulateInvestments);
    let mut st = four().state();
    st.strategy_mut().unclaimed = StrategyCard::ALL.iter().copied().take(8).collect();
    let mut r = rig(ActionCard::ManipulateInvestments, st);
    r.run();
    assert!(matches!(r.asked().ask, Ask::Cards(ti4_engine_core::choice::cards::Ask::Pick { what: ti4_engine_core::choice::cards::What::Invest { step: 1, owed: 3 }, .. })));
    let cards: Vec<StrategyCard> = StrategyCard::ALL.iter().copied().filter(|c| ALL_SOURCES.0 & ti4_engine_core::tables::source_bit(c.data().source) != 0).collect();
    assert_eq!(r.opts().len(), cards.len());
    r.assert_options_sound();
    let invest = |r: &Rig<Play>| match r.asked().ask {
        Ask::Cards(ti4_engine_core::choice::cards::Ask::Pick { what: ti4_engine_core::choice::cards::What::Invest { step, owed }, .. }) => (step, owed),
        ref other => panic!("{other:?}"),
    };
    let first = cards[0];
    r.pick_opt(o(CO::Strat(first)));
    r.pick_opt(o(CO::Strat(first)));
    assert_eq!(invest(&r), (3, 2), "two tokens placed, two more distinct cards owed");
    assert_eq!(r.opts().len(), cards.len(), "three tokens left for two cards owed: any card");
    r.pick_opt(o(CO::Strat(cards[1])));
    assert_eq!(invest(&r), (4, 1));
    assert_eq!(r.opts().len(), cards.len(), "two tokens left for one card owed: any card");
    r.pick_opt(o(CO::Strat(first)));
    assert_eq!(invest(&r), (5, 1));
    assert!(!r.opts().contains(&o(CO::Strat(first))) && !r.opts().contains(&o(CO::Strat(cards[1]))), "the last token must go to a third card");
    assert_eq!(r.opts().len(), cards.len() - 2);
    r.pick_opt(o(CO::Strat(cards[2])));
    assert!(r.is_done());
    let g = |c: StrategyCard| r.state.strategy().goods.get(&c).copied().unwrap_or(0);
    assert_eq!((g(first), g(cards[1]), g(cards[2])), (3, 1, 1), "five trade goods on three different cards");
    assert_eq!(cards.iter().map(|c| g(*c)).sum::<i32>(), 5);
}

#[test]
fn repeal_law_removes_the_law_the_holder_names() {
    covers_card!(ActionCard::RepealLaw);
    let mut st = four().state();
    st.agenda_mut().laws.insert(Agenda::FleetRegulations, Outcome::Empty);
    st.agenda_mut().laws.insert(Agenda::ExecutiveSanctions, Outcome::Empty);
    let mut r = rig(ActionCard::RepealLaw, st);
    r.run();
    assert_eq!(what(&r), What::Law);
    assert_eq!(r.opts().len(), 2);
    r.assert_options_sound();
    r.pick_opt(o(CO::Law(Agenda::FleetRegulations)));
    assert!(r.is_done());
    assert!(!r.state.agenda().laws.contains_key(&Agenda::FleetRegulations));
    assert!(r.state.agenda().laws.contains_key(&Agenda::ExecutiveSanctions));
}

#[test]
fn spy_takes_a_random_card_from_the_hand_of_the_chosen_player_by_the_die() {
    covers_card!(ActionCard::Spy);
    let hand = [ActionCard::ALL[3], ActionCard::ALL[4], ActionCard::ALL[5]];
    let st = four().hand(1, &hand).hand(2, &[ActionCard::ALL[9]]).state();
    let mut r = rig(ActionCard::Spy, st.clone());
    r.run();
    assert_eq!(what(&r), What::Player);
    assert_eq!(seats_of(&r), vec![1, 2], "players holding a card");
    r.assert_options_sound();
    r.pick_opt(o(CO::Seat(Seat(1))));
    let face = r.driver.window.faces()[0] as usize;
    let taken = hand[(face - 1) % 3];
    assert!(r.is_done());
    assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[taken]);
    assert_eq!(r.state.players().by_seat(Seat(1)).unwrap().cards.hand.len(), 2);
    assert!(!r.state.players().by_seat(Seat(1)).unwrap().cards.hand.as_slice().contains(&taken));
    // Alone with one candidate: taken unasked (a single option).
    let one = four().hand(2, &[ActionCard::ALL[9]]).state();
    let mut r2 = rig(ActionCard::Spy, one);
    r2.run();
    assert!(r2.is_done());
    assert_eq!(r2.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[ActionCard::ALL[9]]);
}

#[test]
fn lucky_shot_destroys_a_ship_of_another_player_in_a_system_where_the_holder_has_a_planet() {
    covers_card!(ActionCard::LuckyShot);
    let st = four()
        .control(planet(1), 0)
        .control(planet(2), 0)
        .unit(sys(1), None, Unit::DestroyerI, 1, 1)
        .unit(sys(1), None, Unit::CarrierI, 1, 1) // not a destroyer, cruiser or dreadnought
        .unit(sys(2), None, Unit::DreadnoughtI, 2, 2) // two identical hulls: one option
        .unit(sys(2), None, Unit::CruiserI, 0, 1) // ours
        .unit(sys(3), None, Unit::CruiserI, 1, 1) // no planet of ours there
        .state();
    let mut r = rig(ActionCard::LuckyShot, st);
    r.run();
    assert_eq!(what(&r), What::Hull);
    assert_eq!(r.opts().len(), 2, "the destroyer, and the dreadnought once");
    r.assert_options_sound();
    r.pick(1);
    assert!(r.is_done());
    let count = |i: usize, seat: u8, kind: Unit| r.state.board().sys(sys(i)).units.iter().filter(|u| u.owner() == Seat(seat) && u.kind() == kind).count();
    assert_eq!(count(2, 2, Unit::DreadnoughtI), 1, "one of the two dreadnoughts is destroyed");
    assert_eq!(count(1, 1, Unit::DestroyerI), 1);
    assert_eq!(count(1, 1, Unit::CarrierI), 1);
    assert_eq!(count(3, 1, Unit::CruiserI), 1);
}

#[test]
fn exchange_program_lets_both_players_land_infantry_on_a_planet_if_the_other_accepts() {
    covers_card!(ActionCard::ExchangeProgram);
    let sc = four().control(planet(1), 2).unit(sys(1), Some(planet(1)), Unit::InfantryI, 2, 2).control(planet(2), 2);
    let st = sc.clone().state();
    let mut r = rig(ActionCard::ExchangeProgram, st);
    r.run();
    assert_eq!(seats_of(&r), vec![1, 2, 3]);
    assert_eq!(what(&r), What::Player);
    r.pick_opt(o(CO::Seat(Seat(2))));
    // Only planets of the other player with their own ground forces are offered; one candidate: taken unasked, then the other answers.
    assert_eq!(what(&r), What::Accept);
    assert_eq!(r.asked().actor, Seat(2), "the other player accepts or refuses");
    assert_eq!(r.opts(), vec![o(CO::Answer(true)), o(CO::Answer(false))]);
    r.pick(0);
    assert!(r.is_done());
    let on = |s: &CoreState, seat: u8| s.board().planet(planet(1)).units.iter().filter(|u| u.owner() == Seat(seat)).count();
    assert_eq!((on(&r.state, 0), on(&r.state, 2)), (1, 3), "one infantry for each");
    // Refused: both pay a fleet token.
    let mut no = rig(ActionCard::ExchangeProgram, sc.tokens(0, 2, 2, 2).tokens(2, 2, 2, 2).state());
    no.run().pick_opt(o(CO::Seat(Seat(2))));
    no.pick(1);
    assert!(no.is_done());
    assert_eq!(on(&no.state, 0), 0);
    let fleet = |s: &CoreState, n: u8| s.players().by_seat(Seat(n)).unwrap().econ.fleet_tokens;
    assert_eq!((fleet(&no.state, 0), fleet(&no.state, 2)), (1, 1), "a refused exchange costs both a fleet token");
}

#[test]
fn reparations_exhausts_a_planet_of_the_new_controller_and_readies_one_of_ours() {
    covers_card!(ActionCard::Reparations);
    // Seat 1 just took a planet from the holder (seat 0).
    let mut st = four().control(planet(1), 1).control(planet(2), 1).control(planet(3), 0).exhaust(planet(3)).control(planet(4), 0).state();
    st.transient_mut().combat.last_control_gained = Some((sys(1), planet(1), Seat(1), Some(Seat(0))));
    let mut r = rig(ActionCard::Reparations, st.clone());
    r.run();
    // The new controller chooses which of their planets is exhausted.
    assert_eq!(what(&r), What::Exhaust);
    assert_eq!(r.asked().actor, Seat(1));
    assert_eq!(r.opts().len(), 2);
    r.assert_options_sound();
    r.pick_opt(o(CO::Spot(sys(2), planet(2))));
    assert!(r.state.board().planet(planet(2)).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED));
    // Then the holder readies one of its exhausted planets (only one: unasked).
    assert!(r.is_done());
    assert!(!r.state.board().planet(planet(3)).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED), "readied");
    // The planet was not taken from the holder: no effect at all.
    let mut other = st.clone();
    other.transient_mut().combat.last_control_gained = Some((sys(1), planet(1), Seat(1), Some(Seat(2))));
    let mut r2 = rig(ActionCard::Reparations, other.clone());
    r2.run();
    assert!(r2.is_done() && r2.state == other);
    // The holder is the one who gained it: no effect either.
    let mut own = st;
    own.transient_mut().combat.last_control_gained = Some((sys(1), planet(1), Seat(0), Some(Seat(0))));
    let mut r3 = rig(ActionCard::Reparations, own.clone());
    r3.run();
    assert!(r3.is_done() && r3.state == own);
}

#[test]
fn public_disgrace_takes_the_strategy_card_the_other_player_just_picked_and_makes_them_pick_again() {
    covers_card!(ActionCard::PublicDisgrace);
    let mut st = four().state();
    st.players_mut().by_seat_mut(Seat(1)).unwrap().strat.push(StrategyCard::Leadership);
    st.strategy_mut().unclaimed = [StrategyCard::Politics, StrategyCard::Trade].into_iter().collect();
    st.transient_mut().misc.last_strategy_choice = Some((Seat(1), StrategyCard::Leadership));
    let mut r = rig(ActionCard::PublicDisgrace, st.clone());
    r.run();
    // The card went back to the mat and the picker chooses another, not the same.
    assert_eq!(r.asked().actor, Seat(1));
    assert_eq!(what(&r), What::Redraft { first: StrategyCard::Leadership, by: Seat(0) });
    assert_eq!(r.opts(), vec![o(CO::Strat(StrategyCard::Politics)), o(CO::Strat(StrategyCard::Trade))]);
    r.assert_options_sound();
    r.pick_opt(o(CO::Strat(StrategyCard::Trade)));
    assert!(r.is_done());
    let held = |s: &CoreState, n: u8| s.players().by_seat(Seat(n)).unwrap().strat.as_slice().to_vec();
    assert_eq!(held(&r.state, 1), vec![StrategyCard::Trade]);
    assert!(r.state.strategy().unclaimed.as_slice().contains(&StrategyCard::Leadership));
    assert!(!r.state.strategy().unclaimed.as_slice().contains(&StrategyCard::Trade));
    // Our own pick, or no pick at all: nothing.
    let mut mine = st.clone();
    mine.transient_mut().misc.last_strategy_choice = Some((Seat(0), StrategyCard::Leadership));
    mine.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(StrategyCard::Leadership);
    let mut r2 = rig(ActionCard::PublicDisgrace, mine.clone());
    r2.run();
    assert!(r2.is_done() && r2.state == mine);
    // Nothing else on the mat: the first choice stands.
    let mut bare = st.clone();
    bare.strategy_mut().unclaimed = SmallList::new();
    let mut r3 = rig(ActionCard::PublicDisgrace, bare);
    r3.run();
    assert!(r3.is_done());
    assert_eq!(held(&r3.state, 1), vec![StrategyCard::Leadership], "if able: it was the only card");
}

#[test]
fn archaeological_expedition_draws_three_cards_of_a_planets_deck_and_keeps_the_fragments() {
    covers_card!(ActionCard::ArchaeologicalExpedition);
    let cultural = Planet::Quann; // a cultural planet
    let sc = four().control(cultural, 0);
    let mut st = sc.state();
    let deck: Vec<Explore> = Explore::ALL.into_iter().filter(|e| e.data().deck == ExploreType::Cultural).collect();
    let fragments: Vec<Explore> = deck.iter().copied().filter(|e| e.data().resolution == ti4_engine_core::generated::ExploreResolution::Fragment).take(2).collect();
    let others: Vec<Explore> = deck.iter().copied().filter(|e| e.data().resolution != ti4_engine_core::generated::ExploreResolution::Fragment).take(3).collect();
    assert_eq!(fragments.len(), 2);
    st.decks_mut().explore[ExploreType::Cultural as usize] = Deck::new(vec![fragments[0], others[0], fragments[1], others[1], others[2]]);
    st.decks_mut().explore_present |= 1 << ExploreType::Cultural as u8;
    let mut r = rig(ActionCard::ArchaeologicalExpedition, st);
    r.run();
    assert!(r.is_done(), "a single planet: taken unasked");
    let held = r.state.players().by_seat(Seat(0)).unwrap().cards.fragments.get(ExploreType::Cultural);
    assert_eq!(held, Some(2), "two fragments among the first three cards");
    assert_eq!(r.state.decks().explore[ExploreType::Cultural as usize].cards().len(), 2, "three cards were drawn");
    // A deck with only two cards left: the third draw finds nothing.
    let mut short = four().control(cultural, 0).state();
    short.decks_mut().explore[ExploreType::Cultural as usize] = Deck::new(vec![fragments[0], fragments[1]]);
    short.decks_mut().explore_present |= 1 << ExploreType::Cultural as u8;
    let mut r2 = rig(ActionCard::ArchaeologicalExpedition, short);
    r2.run();
    assert_eq!(r2.state.players().by_seat(Seat(0)).unwrap().cards.fragments.get(ExploreType::Cultural), Some(2));
    // No planet we control: no play of any use.
    let none = four().state();
    let mut r3 = rig(ActionCard::ArchaeologicalExpedition, none.clone());
    r3.run();
    assert!(r3.is_done() && r3.state == none);
}

#[test]
fn bribery_spends_trade_goods_for_extra_votes_on_this_agenda() {
    covers_card!(ActionCard::Bribery);
    let mut st = four().goods(0, 3).state();
    st.meta_mut().agenda_seq = 2;
    let mut r = rig(ActionCard::Bribery, st.clone());
    r.run();
    assert_eq!(what(&r), What::Spend);
    assert_eq!(r.opts(), (0..=3).map(|n| o(CO::Count(n))).collect::<Vec<_>>(), "any number, zero included");
    r.assert_options_sound();
    r.pick_opt(o(CO::Count(2)));
    assert!(r.is_done());
    assert_eq!(goods(&r.state, 0), 1);
    assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().card_timers.extra_votes_agenda, Some(ti4_engine_core::state::ExtraVotes { agenda: 2, votes: 2 }));
    // Zero spends nothing and adds nothing.
    let mut zero = rig(ActionCard::Bribery, st.clone());
    zero.run().pick_opt(o(CO::Count(0)));
    assert_eq!(goods(&zero.state, 0), 3);
    assert_eq!(zero.state.players().by_seat(Seat(0)).unwrap().card_timers.extra_votes_agenda, None);
    // Votes pile up with earlier ones of this agenda, not with another agenda's.
    let mut earlier = st.clone();
    earlier.players_mut().by_seat_mut(Seat(0)).unwrap().card_timers.extra_votes_agenda = Some(ti4_engine_core::state::ExtraVotes { agenda: 2, votes: 5 });
    let mut r2 = rig(ActionCard::Bribery, earlier);
    r2.run().pick_opt(o(CO::Count(1)));
    assert_eq!(r2.state.players().by_seat(Seat(0)).unwrap().card_timers.extra_votes_agenda.unwrap().votes, 6);
    let mut older = st;
    older.players_mut().by_seat_mut(Seat(0)).unwrap().card_timers.extra_votes_agenda = Some(ti4_engine_core::state::ExtraVotes { agenda: 1, votes: 5 });
    let mut r3 = rig(ActionCard::Bribery, older);
    r3.run().pick_opt(o(CO::Count(3)));
    assert_eq!(r3.state.players().by_seat(Seat(0)).unwrap().card_timers.extra_votes_agenda.unwrap().votes, 3);
    // No trade good: nothing to ask.
    let mut broke = rig(ActionCard::Bribery, four().state());
    broke.run();
    assert!(broke.is_done());
}

#[test]
fn lie_in_wait_takes_one_action_card_from_each_of_two_trading_neighbours() {
    covers_card!(ActionCard::LieInWait);
    let st = four().hand(1, &[ActionCard::ALL[3], ActionCard::ALL[4]]).hand(2, &[ActionCard::ALL[5]]).hand(3, &[ActionCard::ALL[6]]).state();
    let mut e = env();
    e.facts.traders = [Some(Seat(1)), Some(Seat(2))];
    let play = Play::of(&st, ActionCard::LieInWait, Seat(0), e).expect("a play");
    let mut r = Rig::from_parts(st, ti4_engine_core_tests::kernel_all(), play).with_table(linked()).seeded(1);
    r.run();
    assert_eq!(what(&r), What::Lie { from: Seat(1) });
    assert_eq!(r.opts().len(), 2, "the two cards of the first trader");
    r.assert_options_sound();
    r.pick_opt(o(CO::Card(ActionCard::ALL[4])));
    // The second trader holds one card: taken unasked.
    assert!(r.is_done());
    let mine = r.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice().to_vec();
    assert_eq!(mine, vec![ActionCard::ALL[4], ActionCard::ALL[5]]);
    assert_eq!(r.state.players().by_seat(Seat(1)).unwrap().cards.hand.as_slice(), &[ActionCard::ALL[3]]);
    assert!(r.state.players().by_seat(Seat(2)).unwrap().cards.hand.is_empty());
    assert_eq!(r.state.players().by_seat(Seat(3)).unwrap().cards.hand.len(), 1, "a seat who did not trade keeps its card");
    // A trader with no card is passed over; no trader at all: nothing.
    let none = four().hand(2, &[ActionCard::ALL[5]]).state();
    let mut e2 = env();
    e2.facts.traders = [Some(Seat(1)), Some(Seat(2))];
    let play2 = Play::of(&none, ActionCard::LieInWait, Seat(0), e2).expect("a play");
    let mut r2 = Rig::from_parts(none, ti4_engine_core_tests::kernel_all(), play2).with_table(linked()).seeded(1);
    r2.run();
    assert!(r2.is_done());
    assert_eq!(r2.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[ActionCard::ALL[5]]);
    let idle = four().hand(1, &[ActionCard::ALL[3]]).state();
    let mut r3 = rig(ActionCard::LieInWait, idle.clone());
    r3.run();
    assert!(r3.is_done() && r3.state == idle);
}

#[test]
fn seize_artifact_takes_a_relic_fragment_from_a_neighbour_who_holds_one() {
    covers_card!(ActionCard::SeizeArtifact);
    let mut st = four().state();
    st.players_mut().by_seat_mut(Seat(1)).unwrap().cards.fragments.set(ExploreType::Hazardous, 2);
    st.players_mut().by_seat_mut(Seat(1)).unwrap().cards.fragments.set(ExploreType::Cultural, 1);
    st.players_mut().by_seat_mut(Seat(2)).unwrap().cards.fragments.set(ExploreType::Industrial, 1);
    let mut r = rig(ActionCard::SeizeArtifact, st);
    r.run();
    // Seat 1 is the only neighbour (seats 3 and 1) with a fragment: taken unasked; then which kind.
    assert_eq!(what(&r), What::Fragment);
    assert_eq!(r.opts(), vec![o(CO::Fragment(ExploreType::Cultural)), o(CO::Fragment(ExploreType::Hazardous))]);
    r.assert_options_sound();
    r.pick_opt(o(CO::Fragment(ExploreType::Hazardous)));
    assert!(r.is_done());
    let frags = |r: &Rig<Play>, seat: u8, kind: ExploreType| r.state.players().by_seat(Seat(seat)).unwrap().cards.fragments.get(kind);
    assert_eq!((frags(&r, 0, ExploreType::Hazardous), frags(&r, 1, ExploreType::Hazardous)), (Some(1), Some(1)));
    assert_eq!(frags(&r, 1, ExploreType::Cultural), Some(1));
    // A neighbour's last fragment of a kind leaves the count absent, and the opposite neighbour is reachable too.
    let mut st = four().state();
    st.players_mut().by_seat_mut(Seat(3)).unwrap().cards.fragments.set(ExploreType::Frontier, 1);
    let mut r2 = rig(ActionCard::SeizeArtifact, st);
    r2.run();
    assert!(r2.is_done(), "one neighbour, one kind: everything is taken unasked");
    assert_eq!(frags(&r2, 3, ExploreType::Frontier), None);
    assert_eq!(frags(&r2, 0, ExploreType::Frontier), Some(1));
    // Only a player across the table holds one: no neighbour to seize from.
    let mut far = four().state();
    far.players_mut().by_seat_mut(Seat(2)).unwrap().cards.fragments.set(ExploreType::Frontier, 1);
    let mut r3 = rig(ActionCard::SeizeArtifact, far.clone());
    r3.run();
    assert!(r3.is_done() && r3.state == far);
}

#[test]
fn archaeological_expedition_uses_the_first_trait_of_a_planet_with_two() {
    covers_card!(ActionCard::ArchaeologicalExpedition);
    // Olergodt is hazardous and cultural; the card draws from the first printed trait's deck only (D4A3-info-03).
    let mut st = four().control(Planet::Olergodt, 0).state();
    for kind in [ExploreType::Hazardous, ExploreType::Cultural] {
        let cards: Vec<Explore> = Explore::ALL.into_iter().filter(|e| e.data().deck == kind).take(4).collect();
        st.decks_mut().explore[kind as usize] = Deck::new(cards);
        st.decks_mut().explore_present |= 1 << kind as u8;
    }
    let mut r = rig(ActionCard::ArchaeologicalExpedition, st);
    r.run();
    assert!(r.is_done(), "one planet, one deck: taken unasked");
    assert_eq!(r.state.decks().explore[ExploreType::Hazardous as usize].cards().len(), 1, "three cards drawn from the hazardous deck");
    assert_eq!(r.state.decks().explore[ExploreType::Cultural as usize].cards().len(), 4, "the cultural deck is untouched");
}
