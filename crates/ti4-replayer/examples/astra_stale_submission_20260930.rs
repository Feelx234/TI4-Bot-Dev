//! Reproduce replaying an old click against a new identical-looking question.
use ti4_replayer::{
    BranchId, ManualControl, ManualSubmission, PendingManualChoice, SeatControl, SubmitOutcome,
};
fn main() {
    let choice = ti4_engine::choice::Choice::new(
        ti4_model::id::PlayerId::new("seat0"),
        "choose",
        vec![
            ti4_engine::choice::ChoiceOption::new("a", "action"),
            ti4_engine::choice::ChoiceOption::new("b", "action"),
        ],
    );
    let first = PendingManualChoice::new(BranchId::SOURCE, 7, 1, None, &choice).unwrap();
    let second = PendingManualChoice::new(BranchId::SOURCE, 7, 2, None, &choice).unwrap();
    let old_click = ManualSubmission {
        fingerprint: first.fingerprint.clone(),
        option_id: "a".into(),
    };
    let mut gate = ManualControl::new(SeatControl::all_auto());
    gate.publish(first);
    assert!(matches!(
        gate.submit(&old_click),
        SubmitOutcome::Accepted { .. }
    ));
    gate.publish(second);
    let outcome = gate.submit(&old_click);
    println!(
        "OLD_CLICK_AT_NEXT_ASK {outcome:?}; pending_remaining={}",
        gate.pending().is_some()
    );
    assert!(
        matches!(
            outcome,
            SubmitOutcome::Stale { .. } | SubmitOutcome::Duplicate
        ),
        "a click for ask 1 must not answer ask 2"
    );
}
