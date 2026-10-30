//! The dispatch of the answer to the action menu (package D1b2; the `Phase::Action` arm of `Game::apply_choice`): which of the
//! arms the chosen option takes, and the choice-free writes of the arms that are the core's. Everything an option of a source
//! that is not ported does is LENT (`Dispatch::Lend`: the reverse lend runs the string body at the label `turn::component`;
//! documented bail D1b2-bail-04): faction, leader, technology, expedition, relic, exploration, action-card, transaction, contact
//! and payment options.

use crate::choice::turn::Opt;
use crate::generated::StrategyCard;
use crate::ids::Seat;
use crate::state::{CoreState, Feat, TransientFlags};

/// The arm an answer of the menu takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    /// "pass": the seat is out of the phase.
    Pass,
    /// A tactical action begins: the activation question.
    Tactical,
    /// A strategic action of the card begins.
    Strategic(StrategyCard),
    /// The closing menu's "end your turn".
    EndTurn,
    /// The answer to Minister of War's question: the window it belongs to takes it (not an arm of the menu).
    Token,
    /// A source the core does not run: the string body runs through the reverse lend.
    Lend,
}

/// The arm of the chosen option.
#[must_use]
pub const fn dispatch(opt: Opt) -> Dispatch {
    match opt {
        Opt::Pass => Dispatch::Pass,
        Opt::Tactical => Dispatch::Tactical,
        Opt::Strategic { card, .. } => Dispatch::Strategic(card),
        Opt::EndTurn => Dispatch::EndTurn,
        Opt::Token(_) => Dispatch::Token,
        Opt::Lent(_) => Dispatch::Lend,
    }
}

/// `passed = true` of the seat that passes.
pub fn mark_passed(core: &mut CoreState, seat: Seat) {
    if let Some(p) = core.players_mut().by_seat_mut(seat) {
        p.econ.passed = true;
    }
}

/// Prove Endurance: the moment the last seat passes, the fact is recorded against a fresh occurrence ("last to pass" is decided by
/// there being nobody left who has not). Returns the occurrence, or `None` while a seat has yet to pass.
pub fn last_to_pass(core: &mut CoreState, seat: Seat) -> Option<u64> {
    if !core.players().iter().all(|p| p.econ.passed) {
        return None;
    }
    let meta = core.meta_mut();
    meta.feat_occurrence_seq += 1;
    let occurrence = meta.feat_occurrence_seq;
    if let Some(p) = core.players_mut().by_seat_mut(seat) {
        let entry = (Feat::LastToPass, occurrence);
        if !p.misc.event_feats.as_slice().contains(&entry) {
            p.misc.event_feats.push(entry);
            p.misc.event_feats.as_mut_slice().sort_unstable();
        }
    }
    Some(occurrence)
}

/// An explicit pass declines the extra action Master Plan may have granted.
pub fn decline_additional_action(core: &mut CoreState) {
    core.transient_mut().misc.flags.clear(TransientFlags::ADDITIONAL_ACTION);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Code;

    #[test]
    fn every_native_option_has_its_arm_and_a_lent_one_is_lent() {
        assert_eq!(dispatch(Opt::Pass), Dispatch::Pass);
        assert_eq!(dispatch(Opt::Tactical), Dispatch::Tactical);
        assert_eq!(dispatch(Opt::EndTurn), Dispatch::EndTurn);
        let card = StrategyCard::ALL[0];
        assert_eq!(dispatch(Opt::Strategic { card, bare: true }), Dispatch::Strategic(card));
        assert_eq!(dispatch(Opt::Lent(Code(3))), Dispatch::Lend);
        assert_eq!(dispatch(Opt::Token(crate::generated::System::ALL[0])), Dispatch::Token);
    }
}
