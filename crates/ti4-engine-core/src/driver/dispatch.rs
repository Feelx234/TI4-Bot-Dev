//! The dispatch of the answer to the action menu (package D1b2, KM2; the `Phase::Action` arm of `Game::apply_choice`): which of the
//! arms the chosen option takes, and the choice-free writes of the arms that are the core's. Every option of the menu has its arm:
//! the component actions (leader, technology, relic, exploration, action card, faction) are run by `driver::action`, a transaction
//! opens the trade window, pass/tactical/strategic/end-of-turn are the arms of their lanes. Only the options of dropped content
//! (contacts, payments, Thunder's Edge's expedition, a legacy id) are [`Dispatch::Refused`]: no live menu lists them.

use crate::choice::turn::Opt;
use crate::generated::{Explore, Leader, StrategyCard, Technology};
use crate::ids::Seat;
use crate::kernel::hooks::sites::ComponentAction;
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
    /// A leader's component action (`leaders::use_leader`); `borrowed`: through Ssruu.
    Leader { leader: Leader, borrowed: bool },
    /// A technology's component action (`technology::perform_component`).
    Technology(Technology),
    /// A relic action: purging fragments or using a relic (`relics::perform`).
    Relic,
    /// An Enigmatic Device exploration card (`exploration::perform_action`).
    Device(Explore),
    /// An action card played as a component action (`action_cards::perform`).
    ActionCard,
    /// A transaction opens with a neighbour (`transactions::open`).
    Transaction(Seat),
    /// A faction's component action (`faction_abilities::perform_component`).
    Faction(ComponentAction),
    /// An option the live menu never lists (dropped content or an option only the string reads): refused, never lent.
    Refused,
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
        Opt::Leader(leader) => Dispatch::Leader { leader, borrowed: false },
        Opt::Borrowed(leader) => Dispatch::Leader { leader, borrowed: true },
        Opt::Tech(tech) => Dispatch::Technology(tech),
        Opt::Purge(_) | Opt::UseRelic(_) => Dispatch::Relic,
        Opt::Device(card) => Dispatch::Device(card),
        Opt::Play { .. } | Opt::Salvaged(_) => Dispatch::ActionCard,
        Opt::Open { with, .. } => Dispatch::Transaction(with),
        Opt::Faction(action) => Dispatch::Faction(action),
        Opt::Lent(_) => Dispatch::Refused,
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
        assert_eq!(dispatch(Opt::Lent(Code(3))), Dispatch::Refused);
        let leader = crate::generated::Leader::ALL[0];
        assert_eq!(dispatch(Opt::Leader(leader)), Dispatch::Leader { leader, borrowed: false });
        assert_eq!(dispatch(Opt::Borrowed(leader)), Dispatch::Leader { leader, borrowed: true });
        assert_eq!(dispatch(Opt::Tech(crate::generated::Technology::SlingRelay)), Dispatch::Technology(crate::generated::Technology::SlingRelay));
        assert_eq!(dispatch(Opt::Purge(crate::generated::ExploreType::Cultural)), Dispatch::Relic);
        assert_eq!(dispatch(Opt::UseRelic(crate::generated::Relic::ALL[0])), Dispatch::Relic);
        assert_eq!(dispatch(Opt::Play { index: 2, card: crate::generated::ActionCard::ALL[0] }), Dispatch::ActionCard);
        assert_eq!(dispatch(Opt::Salvaged(crate::generated::ActionCard::ALL[0])), Dispatch::ActionCard);
        assert_eq!(dispatch(Opt::Open { with: Seat(1), faction: crate::generated::Faction::ALL[1] }), Dispatch::Transaction(Seat(1)));
        assert_eq!(dispatch(Opt::Token(crate::generated::System::ALL[0])), Dispatch::Token);
    }

    fn two_seats() -> CoreState {
        use crate::generated::Faction;
        use crate::state::{Player, Players};
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::ALL[0]), Player::new(Seat(1), Faction::ALL[1])]);
        s
    }

    #[test]
    fn a_pass_marks_the_seat_and_only_the_last_seat_to_pass_records_prove_endurance() {
        let mut s = two_seats();
        mark_passed(&mut s, Seat(0));
        assert!(s.players.edit(0).econ.passed && !s.players.edit(1).econ.passed);
        assert_eq!(last_to_pass(&mut s, Seat(0)), None, "the other seat has yet to pass");
        assert_eq!(s.meta.feat_occurrence_seq, 0, "no occurrence was begun");
        mark_passed(&mut s, Seat(1));
        s.meta.feat_occurrence_seq = 4;
        assert_eq!(last_to_pass(&mut s, Seat(1)), Some(5), "a fresh occurrence");
        assert_eq!(s.meta.feat_occurrence_seq, 5);
        assert_eq!(s.players.edit(1).misc.event_feats.as_slice(), &[(Feat::LastToPass, 5)]);
        assert!(s.players.edit(0).misc.event_feats.as_slice().is_empty(), "only the passer scores it");
        // The same occurrence is never recorded twice.
        s.meta.feat_occurrence_seq = 4;
        assert_eq!(last_to_pass(&mut s, Seat(1)), Some(5));
        assert_eq!(s.players.edit(1).misc.event_feats.as_slice().len(), 1);
    }

    #[test]
    fn an_explicit_pass_declines_the_additional_action() {
        let mut s = two_seats();
        s.transient.misc.flags.set(TransientFlags::ADDITIONAL_ACTION);
        decline_additional_action(&mut s);
        assert!(!s.transient.misc.flags.has(TransientFlags::ADDITIONAL_ACTION));
    }
}
