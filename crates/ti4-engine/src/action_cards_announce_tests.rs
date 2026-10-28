//! Lockstep tests of the announce/resolve flow of a played card (package B6): the component action's path
//! (`reactions::announce` as one core window), a card played from the reaction slot, and a card declared in a window
//! and announced when the window closes. Each compares the string run with the core window through
//! `Area::ActionCards` on `Dual`; a bail is a failure unless the test is about the bail.
#![cfg(all(test, feature = "typed-core"))]

use ti4_content::ContentStore;
use ti4_model::id::{ActionCardId, PlayerId};
use ti4_model::state::GameState;

use crate::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Table};
use crate::dual::{AreaMask, Backend, with_mask};
use crate::action_cards::tests::question_position;

/// Takes a reaction slot whenever one is offered and the last option of every other question (a first-option
/// default would hide an order difference).
pub(crate) struct PlaySlots;

impl Decider for PlaySlots {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if let Some(slot) = choice.options.iter().find(|o| o.id.starts_with("reaction:")) {
            return Ok(slot.clone());
        }
        choice.options.last().cloned().ok_or_else(|| IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() })
    }
}

/// Declines every slot and takes the last option of every other question.
pub(crate) struct DeclineSlots;

impl Decider for DeclineSlots {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if let Some(decline) = choice.options.iter().find(|o| o.is_decline()) {
            return Ok(decline.clone());
        }
        PlaySlots.choose(choice)
    }
}

/// Announce `alias` as played by `player` with the resolver armed for the table; whether the play stood.
pub(crate) fn announce(state: &mut GameState, alias: &str, player: &PlayerId, decider: Box<dyn Decider>) -> bool {
    let mut table = Table::with_default(decider);
    let mut dice = crate::dice::Dice::new();
    let mut rng = crate::rng::GameRng::new(0);
    let mut sequence = crate::event::EventSequence::new();
    let mut resolver = crate::fixtures::armed_resolver(state);
    let mut context = crate::timing::TimingContext {
        state,
        content: ContentStore::embedded(),
        sources: ti4_model::content_types::POK,
        table: &mut table,
        dice: &mut dice,
        rng: &mut rng,
        event_sequence: &mut sequence,
        galaxy: None,
    };
    crate::reactions::announce(&mut context, &mut resolver, player, &ActionCardId::new(alias)).expect("announced")
}

const MASK: AreaMask = AreaMask::STRING;

fn dual() -> AreaMask {
    MASK.with(crate::dual::Area::ActionCards, Backend::Dual)
}

fn windows(label: &str) -> (u64, u64) {
    let r = crate::dual::report();
    let w = r.windows.get(label).cloned().unwrap_or_default();
    (w.compared, w.bailed)
}

/// The component action's path: the card is announced, resolves in the core window and is discarded; the windows
/// around the two events have nothing to offer.
#[test]
fn a_played_card_is_announced_resolved_and_discarded_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let cases: &[&str] = &[
        "mb1", "coup", "rally", "insub", "assassin", "imp_rider", "tech_rider", "sanction", "unexpected", "mining_initiative",
        "uprising", "cripple", "meltdown", "veto", "industrial_initiative", "silence_space", "dp1", "tactical", "messiah",
        "f_deployment", "war_effort", "insider", "sabo1",
    ];
    let _ = crate::dual::take_report();
    for alias in cases {
        let mut state = question_position();
        state.player_mut(&a).unwrap().action_cards = vec![ActionCardId::new(*alias)];
        let before = state.clone();
        // `perform` spends the card before it announces.
        let index = 0;
        crate::action_cards::discard(&mut state, &a, index);
        with_mask(dual(), || announce(&mut state, alias, &a, Box::new(DeclineSlots)));
        assert_eq!(state.discarded_action_cards.last(), Some(&ActionCardId::new(*alias)), "{alias} is on the pile");
        assert!(serde_json::to_string(&state).unwrap() != serde_json::to_string(&before).unwrap(), "{alias} changed nothing");
    }
    let report = crate::dual::report();
    let shown: String = format!("{:?}", report.differences).chars().take(1500).collect();
    assert!(report.differences.is_empty(), "{} differences: {shown}", report.differences.len());
    assert_eq!(windows("action_cards::announce"), (cases.len() as u64, 0), "{:?}", report.windows);
    let _ = crate::dual::take_report();
}

/// A Sabotage played from the reaction slot of `ACTION_CARD_PLAYED`: it is spent, announced (its own windows open),
/// and cancels the card being played, whose effect never runs while its spend stands.
#[test]
fn a_sabotage_played_from_the_slot_cancels_the_play_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    let _ = crate::dual::take_report();
    let mut state = question_position();
    state.player_mut(&b).unwrap().action_cards = vec![ActionCardId::new("sabo1")];
    state.player_mut(&a).unwrap().action_cards = vec![ActionCardId::new("insub")];
    crate::action_cards::discard(&mut state, &a, 0);
    let tokens = state.player(&b).unwrap().tactic_tokens;
    let stood = with_mask(dual(), || announce(&mut state, "insub", &a, Box::new(PlaySlots)));
    assert!(!stood, "the play was cancelled");
    assert_eq!(state.player(&b).unwrap().tactic_tokens, tokens, "Insubordination never ran");
    assert!(state.player(&b).unwrap().action_cards.is_empty(), "Sabotage was spent");
    assert_eq!(state.discarded_action_cards.len(), 2, "both cards are on the pile");
    let report = crate::dual::report();
    let shown: String = format!("{:?}", report.differences).chars().take(1500).collect();
    assert!(report.differences.is_empty(), "{} differences: {shown}", report.differences.len());
    assert_eq!(windows("action_cards::announce"), (1, 0), "{:?}", report.windows);
    let _ = crate::dual::take_report();
}

/// Reverse Engineer is played into the window after a component-action card was discarded: from rules version 3 it
/// is only declared there (spent), and announced when the window closes, taking the discarded card.
#[test]
fn a_card_declared_in_a_window_is_announced_at_its_close_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    let _ = crate::dual::take_report();
    let mut state = question_position();
    assert!(state.rules_version >= ti4_model::state::RULES_SABOTAGE_TIMING);
    state.player_mut(&b).unwrap().action_cards = vec![ActionCardId::new("reverse_engineer")];
    state.player_mut(&a).unwrap().action_cards = vec![ActionCardId::new("industrial_initiative")];
    crate::action_cards::discard(&mut state, &a, 0);
    let stood = with_mask(dual(), || announce(&mut state, "industrial_initiative", &a, Box::new(PlaySlots)));
    assert!(stood);
    assert_eq!(state.player(&b).unwrap().action_cards, vec![ActionCardId::new("industrial_initiative")], "Reverse Engineer took the card");
    assert_eq!(state.discarded_action_cards.last(), Some(&ActionCardId::new("reverse_engineer")));
    let report = crate::dual::report();
    let shown: String = format!("{:?}", report.differences).chars().take(1500).collect();
    assert!(report.differences.is_empty(), "{} differences: {shown}", report.differences.len());
    assert_eq!(windows("action_cards::announce"), (1, 0), "{:?}", report.windows);
    let _ = crate::dual::take_report();
}
