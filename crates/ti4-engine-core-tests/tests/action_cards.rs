//! D4-2c: `rules::action_cards`. The reads (hand limit, who may play, a legal target) are table-tested against the card texts; every ported card is
//! then played in one rich position under four answer policies (contract at every question, conservation, determinism).

use std::panic::{AssertUnwindSafe, catch_unwind};

use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{ActionCard, Agenda, Explore, ExploreType, Planet, PublicObjective, Relic, SecretObjective, Unit};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::query::laws;
use ti4_engine_core::rules::action_cards::effects::Play;
use ti4_engine_core::rules::action_cards::{self as ac, Env, Facts, Placed};
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
        facts: Facts { neutral: Some(Seat(1)), rules_version: 35, ..Facts::default() },
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
    assert!(played >= 80, "only {played} cards have a play window in the rich position");
}

// ---- a few cards against their printed text ---------------------------------------------------

mod cards {
    use ti4_engine_core::choice::cards::Opt as COpt;
    use ti4_engine_core::query::{board, economy};

    use super::*;

    fn goods(s: &CoreState, seat: u8) -> i16 {
        s.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
    }

    fn cards(o: COpt) -> Opt {
        Opt::Cards(o)
    }

    #[test]
    fn mining_initiative_pays_trade_goods_equal_to_the_resources_of_a_planet_we_control() {
        covers_card!(ActionCard::MiningInitiative);
        let mut probe = play_rig(ActionCard::MiningInitiative).expect("a play");
        probe.run();
        let spots: Vec<(System, Planet)> = probe.opts().iter().filter_map(|o| if let Opt::Cards(COpt::Spot(s, p)) = o { Some((*s, *p)) } else { None }).collect();
        assert!(spots.len() >= 2, "every planet seat 0 controls is a choice: {spots:?}");
        let ours: Vec<Planet> = board::controlled_planets(&rich_state(), Seat(0)).as_slice().iter().map(|(_, p)| *p).collect();
        assert_eq!(spots.iter().map(|(_, p)| *p).collect::<Vec<_>>(), ours);
        for (i, (_, planet)) in spots.iter().enumerate() {
            let mut rig = play_rig(ActionCard::MiningInitiative).unwrap();
            rig.run();
            rig.assert_options_sound();
            let before = goods(&rig.state, 0);
            rig.pick(i);
            assert!(rig.is_done());
            assert_eq!(goods(&rig.state, 0), before + i16::from(planet.data().resources), "{planet:?}");
        }
    }

    #[test]
    fn uprising_exhausts_a_planet_of_another_player_and_pays_its_resources() {
        covers_card!(ActionCard::Uprising);
        let mut rig = play_rig(ActionCard::Uprising).expect("a play");
        rig.run();
        assert!(!rig.opts().is_empty());
        let Opt::Cards(COpt::Spot(_, planet)) = rig.opts()[0] else { panic!("a planet") };
        assert!(board::control(&rig.state, planet).is_some_and(|o| o != Seat(0)), "another player's planet");
        let before = goods(&rig.state, 0);
        let worth = economy::value_now(&rig.state, ALL_SOURCES, false, planet, economy::Spend::Resources, 0);
        rig.pick(0);
        assert!(rig.is_done());
        assert!(board::exhausted(&rig.state, planet));
        assert_eq!(i64::from(goods(&rig.state, 0)), i64::from(before) + worth);
    }

    #[test]
    fn repeal_law_removes_the_law_chosen() {
        covers_card!(ActionCard::RepealLaw);
        let mut rig = play_rig(ActionCard::RepealLaw).expect("a play");
        rig.run();
        assert!(rig.is_done(), "a single law in play is the only choice: taken unasked");
        assert!(rig.state.agenda().laws.is_empty());
        assert!(rig.state.agenda().log.iter().next().is_none(), "repealing is not an agenda vote");
    }

    #[test]
    fn summit_gives_two_command_tokens_one_pool_at_a_time() {
        covers_card!(ActionCard::Summit);
        let mut rig = play_rig(ActionCard::Summit).expect("a play");
        rig.run();
        let before = rig.state.players().by_seat(Seat(0)).unwrap().econ;
        let total = |s: &CoreState| {
            let e = s.players().by_seat(Seat(0)).unwrap().econ;
            e.tactic_tokens + e.fleet_tokens + e.strategic_tokens
        };
        let start = total(&rig.state);
        assert_eq!(start, before.tactic_tokens + before.fleet_tokens + before.strategic_tokens);
        rig.pick(0);
        assert!(!rig.is_done(), "two tokens: a second question");
        rig.pick(1);
        assert!(rig.is_done());
        assert_eq!(total(&rig.state), start + 2);
    }

    #[test]
    fn spy_takes_a_random_card_of_the_chosen_player_with_the_dice_of_the_game() {
        covers_card!(ActionCard::Spy);
        for seed in 0..6u64 {
            let mut rig = play_rig(ActionCard::Spy).expect("a play").seeded(seed);
            rig.run();
            let offered: Vec<Seat> = rig.opts().iter().filter_map(|o| if let Opt::Cards(COpt::Seat(s)) = o { Some(*s) } else { None }).collect();
            assert_eq!(offered, vec![Seat(1), Seat(2), Seat(3)], "every other player with a card in hand");
            let victim = Seat(1);
            let was: Vec<ActionCard> = rig.state.players().by_seat(victim).unwrap().cards.hand.as_slice().to_vec();
            rig.pick_opt(cards(COpt::Seat(victim)));
            assert!(rig.is_done());
            let now = rig.state.players().by_seat(victim).unwrap().cards.hand.len();
            assert_eq!(now, was.len() - 1, "one card taken");
            let mine = rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice().to_vec();
            assert_eq!(mine.len(), 1);
            assert!(was.contains(&mine[0]), "and it was one of theirs");
        }
    }
}

// ---- P10 (report-only): the question of seat 0 does not depend on the identity of other seats' hidden cards ------------------

#[test]
fn p10_the_question_of_the_holder_does_not_depend_on_which_cards_the_others_hold() {
    let mut findings = Vec::new();
    let mut compared = 0;
    for card in ActionCard::ALL {
        let make = |st: &CoreState| -> Option<Rig<Play>> {
            let play = Play::of(st, card, Seat(0), env())?;
            Some(Rig::from_parts(st.clone(), ti4_engine_core_tests::kernel_all(), play).with_table(linked()).seeded(card.index() as u64 + 1))
        };
        let (diffs, n) = ti4_engine_core_tests::props::p10_view_independence(&make, &rich_state());
        compared += n;
        findings.extend(diffs.into_iter().map(|d| format!("{card:?}: {d}")));
    }
    eprintln!("P10: {compared} questions of seat 0 compared, {} findings", findings.len());
    for f in findings.iter().take(30) {
        eprintln!("P10 FINDING {f}");
    }
    assert!(compared >= 30, "only {compared} questions compared: the property is vacuous");
}
