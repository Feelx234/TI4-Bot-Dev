//! The read-only secondary preview on a live session: a follower can ask what the secondary in
//! progress would offer while the primary is still resolving; the answer is exactly what the
//! real question later offers, the preview changes nothing, and the cases that mean nothing are
//! refused with a reason.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use ti4_content::ContentStore;
use ti4_engine::secondary_preview::SecondaryPreview;
use ti4_model::id::{PlayerId, StrategyCardId};
use ti4_server::protocol::server::{PreviewRefusal, PreviewResult, SecondaryPreviewMsg};
use ti4_server::protocol::status::ViewerRole;
use ti4_server::session::preview::PreviewRequest;
use ti4_server::session::{GameSession, MockClient, SeatController, SessionConfig};

struct Table {
    session: Arc<GameSession>,
    p1: PlayerId,
    p2: PlayerId,
    card: String,
    c1: MockClient,
    c2: MockClient,
}

/// p1 (human) holds Technology and is in its action; p2 (human) and p3 (bot) follow.
fn start(card_name: &str, card_id: &str) -> Table {
    let content = ContentStore::embedded();
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let p3 = PlayerId::new("p3");
    let ids = vec![p1.clone(), p2.clone(), p3.clone()];
    let (mut state, galaxy) =
        ti4_server::map::create_game_with_map(content, &ids, 4242).expect("game");
    let card = StrategyCardId::new(card_id);
    state.unclaimed_strategy_cards.retain(|c| c != &card);
    state.player_mut(&p1).unwrap().strategy_cards = vec![card.clone()];
    ti4_engine::phase::advance_phase(&mut state);
    ti4_engine::phase::begin_action_turn(&mut state, &p1);
    for seat in &mut state.players {
        seat.strategic_tokens = 2;
        seat.trade_goods = 6;
    }
    let tiles = ti4_server::map::build_board_tiles(content, &galaxy);
    let config = SessionConfig::new(format!("preview_{card_name}"), state)
        .with_player_ids(ids)
        .with_galaxy(galaxy, tiles)
        .with_seat(p1.clone(), SeatController::Human)
        .with_seat(p2.clone(), SeatController::Human)
        .with_seat(p3, SeatController::BotFirstOption);
    let session = Arc::new(GameSession::start(config));
    let c1 = MockClient::connect(session.clone(), ViewerRole::Player(p1.clone()));
    let c2 = MockClient::connect(session.clone(), ViewerRole::Player(p2.clone()));
    Table { session, p1, p2, card: card_id.to_owned(), c1, c2 }
}

fn wait_until(what: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if ready() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

fn pending(table: &Table) -> Option<(PlayerId, String, u64)> {
    table.session.current_pending_decision()
}

fn pending_prompt(table: &Table, seat: &PlayerId) -> Option<String> {
    let client = if seat == &table.p1 { &table.c1 } else { &table.c2 };
    client.snapshot().pending_choice.map(|envelope| envelope.choice.prompt)
}

fn ask(table: &Table, seat: &PlayerId, answers: &[&str]) -> SecondaryPreviewMsg {
    table.session.preview_secondary(
        seat,
        &PreviewRequest {
            request_id: 1,
            card: table.card.clone(),
            primary: table.p1.to_string(),
            answers: answers.iter().map(ToString::to_string).collect(),
        },
    )
}

/// p1 takes the strategic action and is then asked to research (the primary), so the game is
/// mid-step with p2 not yet asked.
fn into_the_primary(table: &Table) {
    wait_until("p1's action", || {
        pending(table).is_some_and(|(seat, _, _)| seat == table.p1)
            && pending_prompt(table, &table.p1).as_deref() == Some("action phase")
    });
    let (_, nonce, version) = pending(table).unwrap();
    table.c1.submit(&nonce, version, "strategic").expect("strategic accepted");
    wait_until("the primary's research question", || {
        pending_prompt(table, &table.p1).as_deref() == Some("research a technology")
    });
}

/// The primary researches the first technology that needs no faction waiver, which ends its part.
fn finish_primary(table: &Table) {
    let options: Vec<String> = table
        .c1
        .snapshot()
        .pending_choice
        .expect("the primary is asked")
        .choice
        .options
        .iter()
        .map(|option| option.id.clone())
        .collect();
    for option in options {
        let Some((seat, nonce, version)) = pending(table) else { return };
        if seat != table.p1 {
            return;
        }
        table.c1.submit(&nonce, version, &option).expect("p1 researches");
        thread::sleep(Duration::from_millis(150));
        if pending_prompt(table, &table.p1).as_deref() != Some("research a technology") {
            return;
        }
    }
    panic!("no technology could be researched by the primary");
}

fn preview_of(message: &SecondaryPreviewMsg) -> &SecondaryPreview {
    match &message.outcome {
        PreviewResult::Preview { preview } => preview,
        PreviewResult::Refused { reason, detail } => panic!("refused {reason:?}: {detail}"),
    }
}

#[test]
fn a_follower_previews_exactly_what_the_real_question_then_offers() {
    let table = start("technology", "pok7technology");
    into_the_primary(&table);

    // The primary is still resolving: p2 asks what the secondary would offer, and what follows a yes.
    let window = ask(&table, &table.p2, &[]);
    let SecondaryPreview::Question { choice: window_question, step: 0, .. } = preview_of(&window)
    else {
        panic!("expected the window question, got {window:?}");
    };
    let research = ask(&table, &table.p2, &["yes"]);
    let SecondaryPreview::Question { choice: research_question, payment, step: 1, .. } =
        preview_of(&research)
    else {
        panic!("expected the research question, got {research:?}");
    };
    let payment = payment.as_ref().expect("the payment plan is previewed");
    assert_eq!(payment.cost, 4);
    assert_eq!(window.card, "pok7technology");
    assert!(window.as_of_decisions > 0);

    // Now p1 finishes the primary; p2 is really asked the same two questions.
    finish_primary(&table);
    wait_until("p2's secondary question", || {
        pending(&table).is_some_and(|(seat, _, _)| seat == table.p2)
    });
    let real = table.c2.snapshot().pending_choice.unwrap().choice;
    assert_eq!(&real, window_question, "the window question is exactly the preview");
    let (_, nonce, version) = pending(&table).unwrap();
    table.c2.submit(&nonce, version, "yes").expect("p2 follows");
    wait_until("p2's research question", || {
        pending_prompt(&table, &table.p2).as_deref() == Some("research a technology")
    });
    let real = table.c2.snapshot().pending_choice.unwrap().choice;
    assert_eq!(&real, research_question, "the follow-up is exactly the preview");

    // Once asked, a follower is refused.
    let late = ask(&table, &table.p2, &[]);
    assert!(matches!(
        late.outcome,
        PreviewResult::Refused { reason: PreviewRefusal::AlreadyAsked, .. }
    ));
    table.session.stop();
}

#[test]
fn the_primary_and_a_game_without_an_action_in_progress_are_refused() {
    let table = start("tech_refusals", "pok7technology");
    wait_until("p1's action", || {
        pending_prompt(&table, &table.p1).as_deref() == Some("action phase")
    });
    // Nothing is in progress yet.
    let early = ask(&table, &table.p2, &[]);
    assert!(matches!(
        early.outcome,
        PreviewResult::Refused { reason: PreviewRefusal::NoStrategicAction, .. }
    ));
    into_the_primary(&table);
    let own = ask(&table, &table.p1, &[]);
    assert!(matches!(
        own.outcome,
        PreviewResult::Refused { reason: PreviewRefusal::IsPrimary, .. }
    ));
    // A different card than the one in progress.
    let mut wrong = PreviewRequest {
        request_id: 2,
        card: "pok1leadership".to_owned(),
        primary: table.p1.to_string(),
        answers: vec![],
    };
    let refused = table.session.preview_secondary(&table.p2, &wrong);
    assert!(matches!(
        refused.outcome,
        PreviewResult::Refused { reason: PreviewRefusal::NoStrategicAction, .. }
    ));
    // A primary that did not play it.
    wrong.card = table.card.clone();
    wrong.primary = "p3".to_owned();
    let refused = table.session.preview_secondary(&table.p2, &wrong);
    assert!(matches!(
        refused.outcome,
        PreviewResult::Refused { reason: PreviewRefusal::NoStrategicAction, .. }
    ));
    table.session.stop();
}

#[test]
fn a_preview_changes_nothing_in_the_live_session() {
    let table = start("tech_readonly", "pok7technology");
    into_the_primary(&table);
    let state = serde_json::to_vec(&table.session.current_state()).unwrap();
    let log = table.session.decision_log();
    let version = table.session.game_version();
    let pending_before = pending(&table);
    let history = serde_json::to_vec(&table.session.replay_export().0).unwrap();
    for answers in [&[][..], &["yes"][..], &["no"][..], &["yes", "zzz"][..]] {
        let _ = ask(&table, &table.p2, answers);
    }
    assert_eq!(state, serde_json::to_vec(&table.session.current_state()).unwrap());
    assert_eq!(log, table.session.decision_log());
    assert_eq!(version, table.session.game_version());
    assert_eq!(pending_before, pending(&table));
    assert_eq!(history, serde_json::to_vec(&table.session.replay_export().0).unwrap());
    // The game still plays on: the preview did not disturb the worker.
    finish_primary(&table);
    wait_until("p2's real question", || {
        pending(&table).is_some_and(|(seat, _, _)| seat == table.p2)
    });
    table.session.stop();
}

#[test]
fn the_answer_does_not_depend_on_other_seats_hidden_information() {
    // Two games that differ only in what the OTHER seats hold privately give p2 the same preview.
    let preview_with = |tweak: &dyn Fn(&mut ti4_model::state::GameState)| {
        let content = ContentStore::embedded();
        let ids = vec![PlayerId::new("p1"), PlayerId::new("p2"), PlayerId::new("p3")];
        let (mut state, _) =
            ti4_server::map::create_game_with_map(content, &ids, 4242).expect("game");
        let card = StrategyCardId::new("pok7technology");
        state.unclaimed_strategy_cards.retain(|c| c != &card);
        state.player_mut(&ids[0]).unwrap().strategy_cards = vec![card.clone()];
        for seat in &mut state.players {
            seat.strategic_tokens = 2;
            seat.trade_goods = 6;
        }
        tweak(&mut state);
        ti4_engine::secondary_preview::preview_secondary(
            &state,
            content,
            ti4_model::content_types::POK,
            None,
            &card,
            &ids[0],
            &ids[1],
            &["yes".to_owned()],
        )
    };
    let plain = preview_with(&|_| {});
    let tweaked = preview_with(&|state| {
        for seat in &mut state.players {
            if seat.id.as_str() != "p2" {
                seat.action_cards.clear();
                seat.trade_goods += 3;
                seat.commodities = 0;
            }
        }
    });
    assert_eq!(plain, tweaked);
}
