//! How far one undo of a seat goes back: one answer, or a movement, the landings or a purchase
//! whole. The server (`ti4-server`, `session::undo`) and the browser host (`ti4-wasm`) read the
//! same answers the same way.

use ti4_engine::choice::{Choice, ChoiceOption};

/// One answer of a seat, as an undo reads it: what was asked, and what was chosen.
pub trait Answered {
    /// The subtype of the context of the choice (`""` without one).
    fn subtype(&self) -> &str;
    /// The kind of the chosen option.
    fn kind(&self) -> &str;
}

impl Answered for (Choice, ChoiceOption) {
    fn subtype(&self) -> &str {
        self.0
            .context
            .as_ref()
            .map_or("", |context| context.subtype.as_str())
    }

    fn kind(&self) -> &str {
        &self.1.kind
    }
}

/// Whether the choice at `at` is in the middle of a movement (a ship has moved, or a hold is
/// open), of the landings (the answer before it landed a unit) or of a production (a purchase
/// that is being paid and placed). An undo that opens such a choice again goes on: a movement
/// is taken back whole.
pub fn inside<A: Answered>(tail: &[A], at: usize) -> bool {
    let before = at.checked_sub(1).map(|index| &tail[index]);
    match tail[at].subtype() {
        "load_cargo" => true,
        "movement_step" => before.is_some_and(|answer| {
            answer.kind() == ti4_engine::tactical::MOVE_KIND || answer.subtype() == "load_cargo"
        }),
        "commit_ground_forces" => {
            before.is_some_and(|answer| answer.kind() == ti4_engine::invasion::COMMIT_KIND)
        }
        "produce_unit" | "pay_resources" | "place_unit" => tail[..at]
            .iter()
            .rev()
            .find(|answer| {
                answer.kind() != ti4_engine::production::PAY_KIND
                    && answer.kind() != ti4_engine::production::PLACE_KIND
            })
            .is_some_and(|answer| answer.kind() == ti4_engine::production::PRODUCE_KIND),
        _ => false,
    }
}

/// The place of the first answer that one undo takes back, at `fence` or later. An answer for
/// which `passed` holds is no answer of the player (a window that the game declines for the
/// Never cards of the seat): the game would decline it again, so the undo goes back over it to
/// the answer before.
pub fn start<A: Answered>(tail: &[A], fence: usize, passed: impl Fn(&A) -> bool) -> Option<usize> {
    let mut found = None;
    for at in (fence..tail.len()).rev() {
        if passed(&tail[at]) {
            continue;
        }
        found = Some(at);
        if !inside(tail, at) {
            break;
        }
    }
    found
}
