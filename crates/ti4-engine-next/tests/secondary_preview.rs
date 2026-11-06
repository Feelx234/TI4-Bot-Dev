//! D2-8f: `secondary_preview::preview_secondary` on the typed core (a throwaway native `Game`) against the legacy string flow it replaces, on
//! played positions: every strategy card, every follower, the window question and the chain of follow-up questions an answer script walks.
//! The whole `SecondaryPreview` is compared (it has one serde shape on both sides).
mod surface_common;

use surface_common::{content, positions};
use ti4_engine as thin;
use ti4_engine_legacy as old;
use ti4_model::content_types::ContentType;
use ti4_model::id::{PlayerId, StrategyCardId};
use ti4_model::state::{GameState, Phase};

#[derive(Default, Debug)]
struct Tally {
    asked: usize,
    questions: usize,
    would_not: usize,
    complete: usize,
    rejected: usize,
    payments: usize,
    skipped: usize,
}

fn cards() -> Vec<StrategyCardId> {
    content().records(ContentType::StrategyCards).iter().filter_map(|record| record.text("id")).filter(|alias| alias.starts_with("pok") && old::strategy_cards::card_kind(content(), alias).is_some()).map(StrategyCardId::new).collect()
}

fn run_old(state: &GameState, card: &StrategyCardId, primary: &PlayerId, follower: &PlayerId, answers: &[String], galaxy: &ti4_content::galaxy::Galaxy) -> serde_json::Value {
    serde_json::to_value(old::secondary_preview::preview_secondary(state, content(), ti4_model::content_types::POK, Some(galaxy), card, primary, follower, answers)).unwrap()
}

fn run_new(state: &GameState, card: &StrategyCardId, primary: &PlayerId, follower: &PlayerId, answers: &[String], galaxy: &ti4_content::galaxy::Galaxy) -> serde_json::Value {
    serde_json::to_value(thin::secondary_preview::preview_secondary(state, content(), ti4_model::content_types::POK, Some(galaxy), card, primary, follower, answers)).unwrap()
}

/// Walk the chain: the preview with the answers so far; when it is a question, answer with its first real option (and, once, with its decline).
fn walk(state: &GameState, galaxy: &ti4_content::galaxy::Galaxy, card: &StrategyCardId, primary: &PlayerId, follower: &PlayerId, answers: Vec<String>, depth: usize, tally: &mut Tally, label: &str) {
    let want = run_old(state, card, primary, follower, &answers, galaxy);
    let mut got = run_new(state, card, primary, follower, &answers, galaxy);
    tally.asked += 1;
    // The multi-ask conversion (`docs/MULTIASK1_RESUME.md`): Leadership's purchase is ONE question on the core (how many tokens and every
    // atom paid), where the string asks a yes, each atom and the pool in turn. Past the window question the two flows differ by design: the
    // core's question after the window's yes is the one purchase question.
    if old::strategy_cards::card_kind(content(), card.as_str()) == Some(ti4_content::StrategyCardKind::Leadership) && !answers.is_empty() {
        // (The string's next question is the first payment, or the token's pool when the payment took its lone atom unasked.)
        if got["status"] == "question" && got["choice"]["context"]["subtype"] == "buy_token_with_influence" {
            assert_eq!(want["status"], "question", "{label}: {card} {primary}->{follower} answers {answers:?}: {want}");
            assert!(matches!(want["choice"]["context"]["subtype"].as_str(), Some("pay_influence" | "gain_command_token")), "{label}: {want}");
            assert_eq!(got["choice"]["multi_pick"]["kind"], "purchase", "{label}: {got}");
            tally.questions += 1;
        } else {
            assert_eq!(want, got, "{label}: {card} {primary}->{follower} answers {answers:?}");
        }
        return;
    }
    // A payment is ONE multi-pick question on the core: the same question as the string's first payment question, plus the multi-pick
    // rule (`Choice::multi_pick`); answering one atom pays the whole bill (that atom first), where the string asks the next atom. The
    // chain is compared up to the payment question.
    let payment = want["status"] == "question" && want["choice"]["context"]["subtype"].as_str().is_some_and(|s| s.starts_with("pay_"));
    if payment {
        assert_eq!(got["choice"]["multi_pick"]["kind"], "payment", "{label}: {got}");
        got["choice"].as_object_mut().unwrap().remove("multi_pick");
    }
    assert_eq!(want, got, "{label}: {card} {primary}->{follower} answers {answers:?}");
    match want["status"].as_str() {
        Some("question") => {
            tally.questions += 1;
            tally.payments += usize::from(!want["payment"].is_null());
            tally.skipped += usize::from(want["skipped"].as_array().is_some_and(|s| !s.is_empty()));
            if payment {
                return;
            }
            if !answers.is_empty() {
                let mut bogus = answers.clone();
                bogus.push("__not_offered__".to_owned());
                walk(state, galaxy, card, primary, follower, bogus, 0, tally, label);
            }
            if depth == 0 {
                return;
            }
            let options = want["choice"]["options"].as_array().cloned().unwrap_or_default();
            let real = options.iter().find(|o| o["kind"] != "decline" && o["id"] != "no");
            let decline = options.iter().find(|o| o["kind"] == "decline" || o["id"] == "no");
            if let Some(option) = real {
                let mut next = answers.clone();
                next.push(option["id"].as_str().unwrap().to_owned());
                walk(state, galaxy, card, primary, follower, next, depth - 1, tally, label);
            }
            if answers.is_empty() && let Some(option) = decline {
                let mut next = answers;
                next.push(option["id"].as_str().unwrap().to_owned());
                walk(state, galaxy, card, primary, follower, next, 0, tally, label);
            }
        }
        Some("would_not_be_asked") => tally.would_not += 1,
        Some("complete") => tally.complete += 1,
        Some("rejected") => tally.rejected += 1,
        _ => {}
    }
}

#[test]
fn the_secondary_preview_on_the_core_answers_as_the_string_flow_does() {
    let mut tally = Tally::default();
    let cards = cards();
    assert!(cards.len() >= 8, "{} cards", cards.len());
    for pos in positions() {
        // (A secondary is only ever offered in the action phase: see `a_position_outside_the_action_phase_is_previewed_as_the_action_phase_it_is`.)
        if pos.state.phase != Phase::Action {
            continue;
        }
        let players: Vec<PlayerId> = pos.state.players.iter().map(|p| p.id.clone()).collect();
        for card in &cards {
            for (i, follower) in players.iter().enumerate().skip(1).take(2) {
                let primary = &players[(i + 1) % players.len()];
                if primary == follower {
                    continue;
                }
                walk(&pos.state, &pos.galaxy, card, primary, follower, Vec::new(), 4, &mut tally, &pos.label);
            }
        }
    }
    println!("{tally:?}");
    assert!(tally.questions > 20 && tally.would_not > 0 && tally.complete > 0 && tally.rejected > 0 && tally.payments > 0, "{tally:?}");
}

#[test]
fn a_preview_with_nothing_to_preview_says_so_as_the_string_flow_does() {
    let pos = &positions()[0];
    let players: Vec<PlayerId> = pos.state.players.iter().map(|p| p.id.clone()).collect();
    let card = StrategyCardId::new("pok1leadership");
    for (primary, follower, answers, card) in [
        (&players[0], &players[0], vec![], card.clone()),
        (&players[0], &PlayerId::new("nobody"), vec![], card.clone()),
        (&players[0], &players[1], vec![], StrategyCardId::new("no_such_card")),
        (&players[0], &players[1], vec!["definitely-not-an-option".to_owned()], card.clone()),
        (&players[0], &players[1], vec!["x".to_owned(); thin::secondary_preview::MAX_ANSWERS + 1], card),
    ] {
        assert_eq!(run_old(&pos.state, &card, primary, follower, &answers, &pos.galaxy), run_new(&pos.state, &card, primary, follower, &answers, &pos.galaxy), "{primary}->{follower} {card} {answers:?}");
    }
}

/// The string stamps the question with the phase of the position it is given; a secondary is only ever asked in the action phase, and the core
/// opens its one-follower window there (`Game::prepare_hypothetical_secondary`). Only the phase of the question's context differs.
#[test]
fn a_position_outside_the_action_phase_is_previewed_as_the_action_phase_it_is() {
    let pos = positions().into_iter().find(|p| p.state.phase == Phase::Strategy).expect("a position in the strategy phase");
    let players: Vec<PlayerId> = pos.state.players.iter().map(|p| p.id.clone()).collect();
    let card = StrategyCardId::new("pok4construction");
    let answers = vec!["yes".to_owned()];
    let (mut want, mut got) = (run_old(&pos.state, &card, &players[2], &players[1], &answers, &pos.galaxy), run_new(&pos.state, &card, &players[2], &players[1], &answers, &pos.galaxy));
    assert_eq!(want["choice"]["context"]["phase"], "strategy");
    assert_eq!(got["choice"]["context"]["phase"], "action");
    want["choice"]["context"]["phase"] = "action".into();
    got["choice"]["context"]["phase"] = "action".into();
    assert_eq!(want, got);
}
