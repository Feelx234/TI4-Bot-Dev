use super::*;
use crate::choice::{AlwaysDecline, Capturing};
use ti4_model::content_types::POK;

fn pid(id: &str) -> PlayerId {
    PlayerId::new(id)
}

/// Three seats; `a` holds the named card and is the primary. Every seat has tokens and four
/// trade goods.
fn dealt(primary_card: &str) -> (GameState, StrategyCardId) {
    let content = ContentStore::embedded();
    let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "letnev"), ("c", "muaat")], POK);
    let card = state
        .unclaimed_strategy_cards
        .iter()
        .find(|card| {
            crate::strategy_cards::card_name(content, card.as_str()).as_deref()
                == Some(primary_card)
        })
        .cloned()
        .expect("the deck carries this card");
    state.unclaimed_strategy_cards.retain(|c| c != &card);
    assert!(state.deal_strategy_card(&pid("a"), card.clone()));
    state.phase = ti4_model::state::Phase::Action;
    for seat in &mut state.players {
        seat.strategic_tokens = 3;
        seat.trade_goods = 4;
    }
    (state, card)
}

fn preview(state: &GameState, card: &StrategyCardId, answers: &[&str]) -> SecondaryPreview {
    let answers: Vec<String> = answers.iter().map(ToString::to_string).collect();
    preview_secondary(
        state,
        ContentStore::embedded(),
        POK,
        None,
        card,
        &pid("a"),
        &pid("b"),
        &answers,
    )
}

fn strategic() -> ChoiceOption {
    ChoiceOption::labelled("strategic", "action", "go")
}

/// The first question the REAL flow puts to `b` after "yes", through the same window and the
/// same `follow` the game driver uses.
fn real_follow_up(state: &GameState, card: &StrategyCardId) -> Option<Choice> {
    let content = ContentStore::embedded();
    let mut state = state.clone();
    let mut window =
        crate::strategy::begin_strategic_action(&mut state, content, &pid("a"), strategic())
            .expect("a holds the card");
    let first = window
        .next_choice(&mut state, content, POK)
        .expect("a window question");
    assert_eq!(first.player, pid("b"));
    let yes = first.options.iter().find(|o| o.id == "yes").cloned().unwrap();
    window.take_choice(&mut state, content, POK, yes).unwrap();
    let (decider, seen) = Capturing::new(Box::new(AlwaysDecline));
    let mut table = Table::with_default(Box::new(decider));
    let _ = crate::strategy_cards::follow(
        &mut state,
        content,
        POK,
        None,
        &mut table,
        &pid("b"),
        card.as_str(),
    );
    let seen = seen.borrow();
    seen.first().cloned()
}

#[test]
fn the_window_question_is_the_real_windows_question() {
    let content = ContentStore::embedded();
    for name in ["Technology", "Warfare", "Construction", "Politics", "Imperial", "Trade"] {
        let (mut state, card) = dealt(name);
        state.player_mut(&pid("b")).unwrap().commodities = 0;
        let mut copy = state.clone();
        let mut window =
            crate::strategy::begin_strategic_action(&mut copy, content, &pid("a"), strategic())
                .unwrap();
        let real = window
            .next_choice(&mut copy, content, POK)
            .unwrap_or_else(|| panic!("{name}: b is first"));
        assert_eq!(real.player, pid("b"), "{name}: {:?}", real.prompt);
        let SecondaryPreview::Question { choice, step, .. } = preview(&state, &card, &[]) else {
            panic!("{name}: expected the window question");
        };
        assert_eq!(step, 0);
        assert_eq!(choice, real, "{name}: the preview IS the real window question");
    }
}

#[test]
fn the_technology_follow_up_is_the_real_one_with_its_payment() {
    let (state, card) = dealt("Technology");
    let real = real_follow_up(&state, &card).expect("b is asked to research");
    assert_eq!(real.context.as_ref().unwrap().subtype, "research_technology");
    let SecondaryPreview::Question {
        choice,
        step,
        payment,
        skipped,
    } = preview(&state, &card, &["yes"])
    else {
        panic!("expected the research question");
    };
    assert_eq!(step, 1);
    assert!(skipped.is_empty());
    assert_eq!(choice, real, "same options, payloads and context as the real question");
    let payment = payment.expect("the 4 resources are paid by a plan");
    assert_eq!(payment.cost, 4);
    assert!(payment.worth >= 4);
}

#[test]
fn a_decline_completes_and_a_bad_answer_is_unavailable() {
    let (state, card) = dealt("Technology");
    assert!(matches!(
        preview(&state, &card, &["no"]),
        SecondaryPreview::Complete { .. }
    ));
    assert!(matches!(
        preview(&state, &card, &["maybe"]),
        SecondaryPreview::Unavailable { .. }
    ));
    assert!(matches!(
        preview(&state, &card, &["yes", "not-a-technology"]),
        SecondaryPreview::Unavailable { .. }
    ));
}

#[test]
fn a_seat_that_would_not_be_asked_gets_the_reason() {
    let (mut state, card) = dealt("Technology");
    state.player_mut(&pid("b")).unwrap().strategic_tokens = 0;
    assert_eq!(
        preview(&state, &card, &[]),
        SecondaryPreview::WouldNotBeAsked {
            blocker: SecondaryBlocker::NoStrategyToken
        }
    );

    let (mut state, card) = dealt("Technology");
    state.player_mut(&pid("b")).unwrap().trade_goods = 0;
    let owned: Vec<_> = state
        .controlled_planets(&pid("b"))
        .into_iter()
        .map(|(_, planet)| planet.clone())
        .collect();
    state.exhausted_planets.extend(owned);
    assert_eq!(
        preview(&state, &card, &[]),
        SecondaryPreview::WouldNotBeAsked {
            blocker: SecondaryBlocker::CannotPayResources
        }
    );

    let (mut state, card) = dealt("Trade");
    let limit =
        crate::strategy_cards::commodity_limit(&state, ContentStore::embedded(), &pid("b"));
    state.player_mut(&pid("b")).unwrap().commodities = limit;
    assert_eq!(
        preview(&state, &card, &[]),
        SecondaryPreview::WouldNotBeAsked {
            blocker: SecondaryBlocker::CommoditiesFull
        }
    );
}

#[test]
fn construction_and_warfare_follow_ups_are_the_real_questions() {
    for (name, subtype) in [("Construction", "place_structure"), ("Warfare", "produce_unit")] {
        let (state, card) = dealt(name);
        let real = real_follow_up(&state, &card).expect("the flow asks something");
        assert_eq!(real.context.as_ref().unwrap().subtype, subtype, "{name}");
        let SecondaryPreview::Question { choice, .. } = preview(&state, &card, &["yes"]) else {
            panic!("{name}: expected a question");
        };
        assert_eq!(choice, real, "{name}");
    }
}

#[test]
fn politics_imperial_and_trade_are_never_run() {
    // Politics and Imperial draw cards; the preview must not touch the decks. Nothing follows
    // the window question for any of the three.
    for name in ["Politics", "Imperial", "Trade"] {
        let (state, card) = dealt(name);
        let before = serde_json::to_value(&state).unwrap();
        assert!(matches!(
            preview(&state, &card, &["yes"]),
            SecondaryPreview::Complete { .. }
        ));
        assert_eq!(before, serde_json::to_value(&state).unwrap(), "{name}");
    }
}

#[test]
fn a_preview_changes_nothing_it_was_given() {
    for name in ["Technology", "Warfare", "Construction", "Diplomacy", "Leadership"] {
        let (mut state, card) = dealt(name);
        let owned: Vec<_> = state
            .controlled_planets(&pid("b"))
            .into_iter()
            .map(|(_, planet)| planet.clone())
            .collect();
        state.exhausted_planets.extend(owned);
        let before = serde_json::to_value(&state).unwrap();
        let cursor = state.deck_reserve.as_ref().map(|reserve| reserve.cursor());
        for answers in [&[][..], &["yes"][..], &["yes", "x"][..], &["no"][..]] {
            let _ = preview(&state, &card, answers);
        }
        assert_eq!(before, serde_json::to_value(&state).unwrap(), "{name}: state");
        assert_eq!(
            cursor,
            state.deck_reserve.as_ref().map(|reserve| reserve.cursor()),
            "{name}: deck cursor"
        );
    }
}
