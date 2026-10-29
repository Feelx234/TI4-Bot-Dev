//! Lockstep tests of the cards ported in the closing package of the action-card area (B6c, docs/B6_CARDS.md): each card
//! through the announce/resolve window (`reactions::announce`) on `Area::ActionCards` = `Dual`. A bail is a failure.
#![cfg(all(test, feature = "typed-core"))]

use ti4_model::id::{ActionCardId, PlayerId};
use ti4_model::state::GameState;

use crate::action_cards::tests::closing_position;
use crate::action_cards_announce_tests::{DeclineSlots, announce};
use crate::dual::{AreaMask, Backend, with_mask};

fn dual() -> AreaMask {
    AreaMask::STRING.with(crate::dual::Area::ActionCards, Backend::Dual)
}

fn windows(label: &str) -> (u64, u64) {
    let r = crate::dual::report();
    let w = r.windows.get(label).cloned().unwrap_or_default();
    (w.compared, w.bailed)
}

/// Play each of `cases` (alias, position) as the component action of seat `a` and compare the announce window; the
/// position must change and the card must be on the pile.
fn announce_all(cases: &[(&str, GameState)]) {
    let a = PlayerId::new("a");
    let _ = crate::dual::take_report();
    for (alias, start) in cases {
        let mut state = start.clone();
        state.player_mut(&a).unwrap().action_cards = vec![ActionCardId::new(*alias)];
        let before = state.clone();
        crate::action_cards::discard(&mut state, &a, 0);
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

/// Repeal Law, Spy, Plague and Unstable Planet through the announcement: the dice and the streams are compared with
/// the string's, the staged ground-force deaths are the string's rows.
#[test]
fn repeal_law_spy_plague_and_unstable_planet_are_announced_and_resolved_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let cases: Vec<(&str, GameState)> = ["repeal", "spy", "plague", "unstable"].iter().map(|alias| (*alias, closing_position())).collect();
    announce_all(&cases);
}
