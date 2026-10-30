//! The steps after a movement step as a chain (package D1b-2; `AftermathWindow::{settle, next_step}` of `game.rs`): where the
//! chain goes when the step it is in has nothing left to ask. The live steps are windows of their own areas (the space combat,
//! the invasion, the production step); this module is the part that is the driver's: which step follows, and the announcements
//! between them. The facts that decide it (a ship of the active seat still holds the space, a contact is possible) are
//! the caller's reads of the position.
//!
//! L5: composes nothing itself; the adapter feeds it the facts the areas own.

use crate::state::{AStage, PlainKind};

/// What the chain does when the step it is in is finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settled {
    /// The space combat is over. `holds`: the active seat keeps the space (a ship of its own is there, or Coalescence /
    /// a Crimson deploy forces the invasion); `contacts`: a diplomatic contact is possible (never with diplomacy off).
    Fighting { holds: bool, contacts: bool },
    /// The invasion is resolved.
    Invading { contacts: bool },
    /// The active seat chose to go on after the pause to negotiate.
    Negotiating { invade: bool },
    /// The production step is resolved.
    Producing,
}

/// The step that follows (`AftermathWindow::settle` and `next_step`). The pause to negotiate is entered only when a contact is
/// possible; without one the chain goes straight on. After a won fight the invasion opens when the active seat holds the
/// space, else production.
#[must_use]
pub fn next(settled: Settled) -> AStage {
    match settled {
        Settled::Fighting { holds, contacts } if contacts => AStage::Negotiating { invade: holds },
        Settled::Fighting { holds, .. } | Settled::Negotiating { invade: holds } => step_after_pause(holds),
        Settled::Invading { contacts } if contacts => AStage::Negotiating { invade: false },
        Settled::Invading { .. } => AStage::Producing,
        Settled::Producing => AStage::Done,
    }
}

fn step_after_pause(invade: bool) -> AStage {
    if invade { AStage::Invading } else { AStage::Producing }
}

/// The plain events a finished step announces, in the order the string pushes them (`AftermathWindow::log`, drained into the
/// event log after the step): the fight is resolved when it lasted at least a round, the invasion and the production step
/// always when they end.
#[must_use]
pub fn announced(settled: Settled, fought: bool) -> Option<PlainKind> {
    match settled {
        Settled::Fighting { .. } => fought.then_some(PlainKind::SpaceCombatResolved),
        Settled::Invading { .. } => Some(PlainKind::InvasionResolved),
        Settled::Producing => Some(PlainKind::ProductionResolved),
        Settled::Negotiating { .. } => None,
    }
}

/// Whether the feats of the fight are to be noted now: once, when the fight has an outcome (`feats_noted` guards a settle that
/// loops).
#[must_use]
pub const fn note_feats(feats_noted: bool, has_outcome: bool) -> bool {
    has_outcome && !feats_noted
}

/// The invasion step opens an invasion record and announces it (`INVASION_BEGAN`); production never does.
#[must_use]
pub const fn opens_invasion(stage: AStage) -> bool {
    matches!(stage, AStage::Invading)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_won_fight_goes_to_the_invasion_when_the_active_seat_holds_the_space_else_to_production() {
        assert_eq!(next(Settled::Fighting { holds: true, contacts: false }), AStage::Invading);
        assert_eq!(next(Settled::Fighting { holds: false, contacts: false }), AStage::Producing);
    }

    #[test]
    fn a_possible_contact_pauses_the_chain_and_remembers_which_step_follows() {
        assert_eq!(next(Settled::Fighting { holds: true, contacts: true }), AStage::Negotiating { invade: true });
        assert_eq!(next(Settled::Fighting { holds: false, contacts: true }), AStage::Negotiating { invade: false });
        assert_eq!(next(Settled::Invading { contacts: true }), AStage::Negotiating { invade: false });
        assert_eq!(next(Settled::Negotiating { invade: true }), AStage::Invading);
        assert_eq!(next(Settled::Negotiating { invade: false }), AStage::Producing);
    }

    #[test]
    fn the_invasion_goes_to_production_and_production_ends_the_chain() {
        assert_eq!(next(Settled::Invading { contacts: false }), AStage::Producing);
        assert_eq!(next(Settled::Producing), AStage::Done);
    }

    #[test]
    fn each_step_announces_its_end_in_its_own_words_and_a_fight_of_no_rounds_none() {
        assert_eq!(announced(Settled::Fighting { holds: true, contacts: false }, true), Some(PlainKind::SpaceCombatResolved));
        assert_eq!(announced(Settled::Fighting { holds: true, contacts: false }, false), None);
        assert_eq!(announced(Settled::Invading { contacts: false }, false), Some(PlainKind::InvasionResolved));
        assert_eq!(announced(Settled::Producing, false), Some(PlainKind::ProductionResolved));
        assert_eq!(announced(Settled::Negotiating { invade: true }, true), None);
    }

    #[test]
    fn the_feats_of_a_fight_are_noted_once() {
        assert!(note_feats(false, true));
        assert!(!note_feats(true, true), "a settle that loops does not record them twice");
        assert!(!note_feats(false, false), "no outcome yet");
        assert!(opens_invasion(AStage::Invading) && !opens_invasion(AStage::Producing));
    }
}
