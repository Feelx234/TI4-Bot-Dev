//! D4-2c: `rules::action_cards`. The reads (hand limit, who may play, a legal target) are table-tested against the card texts; every ported card is
//! then played in one rich position under four answer policies (contract at every question, conservation, determinism).

use std::panic::{AssertUnwindSafe, catch_unwind};

use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{ActionCard, Agenda, Explore, ExploreType, Planet, PublicObjective, Relic, SecretObjective, Unit};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::query::laws;
use ti4_engine_core::rules::action_cards::effects::{Env, Play};
use ti4_engine_core::rules::action_cards::{self as ac, Facts, Placed};
use ti4_engine_core::state::{Code, Outcome};
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

// ---- the reads --------------------------------------------------------------------------------

#[test]
fn the_hand_limit_is_seven_or_three_under_sanctions_and_a_faction_may_change_it() {
    let mut s = four().state();
    assert_eq!(ac::HAND_LIMIT, 7);
    assert_eq!(ac::hand_limit(&s, false, |n| n), 7);
    assert_eq!(ac::hand_limit(&s, false, |n| n + 1), 8, "a faction adds to what the law left");
    s.agenda_mut().laws.insert(Agenda::ExecutiveSanctions, Outcome::Empty);
    assert_eq!(ac::hand_limit(&s, false, |n| n), 3);
    assert_eq!(ac::hand_limit(&s, true, |n| n), 7, "blanked laws do not apply");
}

#[test]
fn who_may_play_an_action_card() {
    let mut s = four().state();
    assert!(ac::may_play(&s, false, Seat(0), false, false));
    assert!(!ac::may_play(&s, false, Seat(0), true, false), "a faction forbids it");
    assert!(!ac::may_play(&s, false, Seat(0), false, true), "Political Secret");
    s.agenda_mut().laws.insert(Agenda::PoliticalCensure, Outcome::Seat(Seat(0)));
    assert!(!ac::may_play(&s, false, Seat(0), false, false), "Political Censure on this seat");
    assert!(ac::may_play(&s, false, Seat(1), false, false));
    assert!(ac::may_play(&s, true, Seat(0), false, false));
}

#[test]
fn component_actions_are_the_cards_printed_so() {
    let n = ActionCard::ALL.into_iter().filter(|c| ac::is_component_action(*c)).count();
    assert!(n > 0 && n < ActionCard::COUNT);
    for c in ActionCard::ALL {
        assert_eq!(ac::is_component_action(c), c.data().component_action);
    }
}

#[test]
fn a_card_with_a_condition_needs_a_target_and_the_others_always_have_one() {
    let s = four();
    let sys = plain_systems();
    let planet = sys[1].data().planets[0];
    let has = |st: &CoreState, c: ActionCard| ac::has_legal_target(st, false, ALL_SOURCES, Seat(0), c);
    let empty = s.clone().state();
    assert_eq!(has(&empty, ActionCard::Spy), Some(false), "nobody holds a card");
    assert_eq!(has(&empty, ActionCard::Insubordination), Some(false), "nobody has a tactic token");
    assert_eq!(has(&empty, ActionCard::RepealLaw), Some(false), "no law in play");
    assert_eq!(has(&empty, ActionCard::Plague), Some(false));
    assert_eq!(has(&empty, ActionCard::CrippleDefenses), Some(false));
    assert_eq!(has(&empty, ActionCard::Scuttle), Some(false));
    assert_eq!(has(&empty, ActionCard::WarEffort), Some(false));
    assert_eq!(has(&empty, ActionCard::UnexpectedAction), Some(false), "no token of ours on the board");
    assert_eq!(has(&empty, ActionCard::RiseOfAMessiah), Some(false));
    assert_eq!(has(&empty, ActionCard::UnstablePlanet), Some(false));
    assert_eq!(has(&empty, ActionCard::SeizeArtifact), None, "the string decides");
    assert_eq!(has(&empty, ActionCard::Summit), Some(true), "no condition");

    let rich = s.tokens(1, 2, 2, 1).hand(1, &[ActionCard::ALL[3]]).control(planet, 1).unit(sys[1], Some(planet), Unit::PDSI, 1, 1).unit(sys[2], None, Unit::CruiserI, 0, 1).command_token(sys[3], 0).state();
    assert_eq!(has(&rich, ActionCard::Spy), Some(true));
    assert_eq!(has(&rich, ActionCard::Insubordination), Some(true));
    assert_eq!(has(&rich, ActionCard::Plague), Some(true), "a rival controls a planet");
    assert_eq!(has(&rich, ActionCard::CrippleDefenses), Some(true), "a PDS");
    assert_eq!(has(&rich, ActionCard::Scuttle), Some(true), "our cruiser");
    assert_eq!(has(&rich, ActionCard::WarEffort), Some(true));
    assert_eq!(has(&rich, ActionCard::UnexpectedAction), Some(true));
    let mut law = rich.clone();
    law.agenda_mut().laws.insert(Agenda::FleetRegulations, Outcome::Empty);
    assert_eq!(has(&law, ActionCard::RepealLaw), Some(true));
    assert_eq!(laws::in_play(&law, false).len(), 1);
    let mut blanked = law.clone();
    assert_eq!(ac::has_legal_target(&blanked, true, ALL_SOURCES, Seat(0), ActionCard::RepealLaw), Some(false), "blanked: no law in play");
    blanked.agenda_mut().laws.clear();
    assert_eq!(has(&blanked, ActionCard::RepealLaw), Some(false));
}

// ---- every ported card, one position ----------------------------------------------------------

fn rich_state() -> CoreState {
    let s = plain_systems();
    let p = |i: usize| s[i].data().planets[0];
    let mut sc = four()
        .active(Some(1))
        .active_system(s[1])
        .tokens(0, 3, 3, 2)
        .tokens(1, 3, 2, 1)
        .tokens(2, 2, 2, 1)
        .tokens(3, 2, 2, 1)
        .goods(0, 4)
        .goods(1, 3)
        .commodities(1, 2)
        .command_token(s[2], 0)
        .command_token(s[3], 1)
        .unit(s[1], None, Unit::CruiserI, 0, 2)
        .unit(s[1], None, Unit::DreadnoughtI, 0, 1)
        .unit(s[1], None, Unit::FighterI, 0, 2)
        .unit(s[1], None, Unit::CruiserI, 1, 2)
        .unit(s[1], None, Unit::DestroyerI, 1, 1)
        .unit(s[2], None, Unit::DestroyerI, 0, 1)
        .unit(s[2], None, Unit::CarrierI, 1, 1)
        .unit(s[3], None, Unit::CarrierI, 0, 1)
        .unit(s[3], None, Unit::CruiserI, 2, 1)
        .control(p(4), 0)
        .unit(s[4], Some(p(4)), Unit::InfantryI, 0, 3)
        .control(p(5), 0)
        .unit(s[5], Some(p(5)), Unit::SpaceDockI, 0, 1)
        .control(p(6), 1)
        .unit(s[6], Some(p(6)), Unit::InfantryI, 1, 2)
        .unit(s[6], Some(p(6)), Unit::PDSI, 1, 1)
        .control(p(2), 2)
        .unit(s[2], Some(p(2)), Unit::InfantryI, 2, 2)
        .hand(1, &[ActionCard::ALL[10], ActionCard::ALL[11], ActionCard::ALL[12]])
        .hand(2, &[ActionCard::ALL[20], ActionCard::ALL[21]])
        .hand(3, &[ActionCard::ALL[30]]);
    sc = sc.control(p(1), 1);
    let mut st = sc.state();
    let d = st.decks_mut();
    d.action = Deck::new(ActionCard::ALL[40..60].to_vec());
    d.agenda = Deck::new(Agenda::ALL[..8].to_vec());
    d.secret = Deck::new(SecretObjective::ALL[..8].to_vec());
    d.relic = Deck::new(vec![Relic::ALL[0], Relic::ALL[1], Relic::ALL[2]]);
    d.objective = Deck::new(PublicObjective::ALL[..6].to_vec());
    for (i, t) in [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial].into_iter().enumerate() {
        let cards: Vec<Explore> = Explore::ALL.into_iter().filter(|e| e.data().deck == t).take(6).collect();
        d.explore[t as usize] = Deck::new(cards);
        d.explore_present |= 1 << i as u8;
    }
    st.cards_mut().discarded_action.edit().extend(ActionCard::ALL[60..64].iter().copied());
    st.agenda_mut().laws.insert(Agenda::FleetRegulations, Outcome::Empty);
    st.objectives_mut().revealed.push(ti4_engine_core::state::ObjectiveRef::Public(PublicObjective::ALL[6]));
    let _ = Planet::COUNT;
    st
}

fn play_rig(card: ActionCard) -> Option<Rig<Play>> {
    let st = rich_state();
    let play = Play::of(&st, card, Seat(0), env())?;
    Some(Rig::from_parts(st, ti4_engine_core_tests::kernel_all(), play).with_table(linked()).seeded(card.index() as u64 + 1))
}

#[test]
fn every_ported_card_plays_to_the_end_in_a_rich_position_under_four_policies() {
    let mut declined = Vec::new();
    let mut broke = Vec::new();
    let mut played = 0;
    for card in ActionCard::ALL {
        if play_rig(card).is_none() {
            declined.push(card);
            continue;
        }
        played += 1;
        covers_card!(card);
        let r = catch_unwind(AssertUnwindSafe(|| {
            smoke_quiet(|| play_rig(card).expect("a play"));
        }));
        if let Err(e) = r {
            let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string())).unwrap_or_default();
            broke.push(format!("{card:?}: {}", msg.lines().take(6).collect::<Vec<_>>().join(" | ")));
        }
    }
    eprintln!("played {played} cards, {} not ported to a play window here: {declined:?}", declined.len());
    assert!(broke.is_empty(), "{} card(s) broke the contract:\n{}", broke.len(), broke.join("\n"));
    assert!(played >= 100, "only {played} cards have a play window in the rich position");
}
