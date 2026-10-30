//! D4-2c: `rules::reactions` (Instinct Training, Infantry II, the L1Z1X agent, the Tellurian and Saar reactions) as registered abilities that
//! open reaction windows around an event the kit's `EmitEvent` window emits.

use ti4_engine_core::generated::ActionCard;
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::rules::reactions::{self, instinct_training::INSTINCT_TRAINING};
use ti4_engine_core::state::{EventKind, Key, Payload, Val};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::windows::EmitEvent;

fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    reactions::register(&mut t);
    t
}

fn card_played_by(who: u8) -> EmitEvent {
    EmitEvent::new(fx::event(EventKind::ActionCardPlayed, Payload::new().with(Key::Player, Val::Seat(Seat(who))).with(Key::Card, Val::Action(ActionCard::Sabotage1))))
}

fn xxcha_table(tokens: i16) -> Scenario {
    Scenario::new(&[Faction::TheXxchaKingdom, Faction::TheVuilraithCabal]).tokens(0, 2, 2, tokens).tech(0, Technology::InstinctTraining)
}

#[test]
fn instinct_training_cancels_a_card_played_by_another_seat_at_the_price_of_a_strategy_token() {
    covers!("rules::reactions::instinct_training");
    let mut rig = Rig::new(xxcha_table(2), card_played_by(1)).with_abilities(table());
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0), "the holder is asked whether to react");
    rig.assert_options_sound();
    let nested_opts = rig.opts();
    assert!(nested_opts.len() >= 2 && nested_opts.last() == Some(&Opt::Decline));
    // First option: use the ability; the ability asks again (use or decline).
    rig.pick(0);
    assert!(!rig.is_done(), "the ability puts its own question");
    rig.assert_options_sound();
    let use_it = rig.opts().iter().position(|o| *o != Opt::Decline).expect("a use option");
    rig.pick(use_it);
    assert!(rig.is_done());
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.econ.strategic_tokens, 1);
    assert!(p.tech.exhausted.contains(Technology::InstinctTraining));
    assert!(rig.driver.emitted[0].cancelled, "the card play was cancelled");
    assert_eq!(rig.state.pending().staged.len(), 1);
    assert_eq!(INSTINCT_TRAINING.0, 5);
    smoke_quiet(|| Rig::new(xxcha_table(2), card_played_by(1)).with_abilities(table()));
}

#[test]
fn instinct_training_is_not_offered_for_the_holders_own_card_without_a_token_or_when_exhausted() {
    covers!("rules::reactions::instinct_training");
    let mut own = Rig::new(xxcha_table(2), card_played_by(0)).with_abilities(table());
    own.run();
    assert!(own.is_done(), "the holder's own card");
    let mut none = Rig::new(xxcha_table(0), card_played_by(1)).with_abilities(table());
    none.run();
    assert!(none.is_done(), "no strategy token");
    let mut spent = xxcha_table(2).state();
    spent.players_mut().by_seat_mut(Seat(0)).unwrap().tech.exhausted.insert(Technology::InstinctTraining);
    let mut rig = Rig::from_parts(spent, ti4_engine_core_tests::kernel_all(), card_played_by(1)).with_abilities(table());
    rig.run();
    assert!(rig.is_done(), "the technology is exhausted");
}

#[test]
fn the_driver_says_whether_the_question_is_a_nested_one_what_it_was_asked_about_and_whether_it_ended() {
    covers!("driver::Driver");
    let mut rig = Rig::new(xxcha_table(2), card_played_by(1)).with_abilities(table());
    assert!(!rig.driver.is_done() && rig.driver.nested_facts().is_none() && rig.driver.nested_state().is_none(), "before the first step");
    rig.run();
    let facts = rig.driver.nested_facts().expect("the reaction window's question is a nested one");
    assert_eq!(facts.event.kind, EventKind::ActionCardPlayed);
    assert!(rig.driver.nested_state().is_some(), "the state the question was put in is kept for rendering it");
    assert!(!rig.driver.is_done());
    assert!(rig.driver.unknown().is_none());
    let use_it = rig.pick(0).opts().iter().position(|o| *o != Opt::Decline).expect("a use option");
    rig.pick(use_it);
    assert!(rig.driver.is_done());
    assert!(rig.driver.nested_facts().is_none(), "the last report was the end of the window");
}

#[test]
fn a_driver_is_seeded_only_when_given_seeds_and_without_them_every_stream_is_zeros() {
    covers!("driver::Driver");
    use ti4_engine_core::driver::NoSeeds;
    use ti4_engine_core::state::{RngDomain, SeedSource};
    let plain = Rig::new(xxcha_table(2), card_played_by(1));
    assert!(!plain.driver.is_seeded());
    let seeded = Rig::new(xxcha_table(2), card_played_by(1)).seeded(7);
    assert!(seeded.driver.is_seeded());
    for domain in [RngDomain::Dice, RngDomain::Agendas, RngDomain::Galaxy] {
        assert_eq!(NoSeeds.seed_for(3, domain), [0u8; 32]);
    }
}
