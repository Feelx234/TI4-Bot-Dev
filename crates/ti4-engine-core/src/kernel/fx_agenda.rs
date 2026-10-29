//! Writers of the agenda log and the laws (packages B9c-1, B9c-2): the rows a revealed agenda leaves without a vote
//! (`Game::note_discarded_agenda` over `push_agenda_record`), the row of a finished vote (`Game::log_agenda`), a law
//! entering play (`GameState::enact_law`) and the Classified Document Leaks publication (`laws::classified_leak`).

use crate::generated::{Agenda, SecretObjective};
use crate::keys::Outcome;
use crate::state::{AgendaRec, Code, CoreState, ObjectiveRef};

/// List an agenda that was revealed and discarded without a vote, so the table is told. `law`: the agenda is a
/// law (`vote::is_law`); `reason` the interned `no_choice` / `replaced` of the string. It is not a resolved
/// agenda: no outcome, no votes, never enacted, never redirected.
pub fn log_discarded(s: &mut CoreState, agenda: Agenda, law: bool, reason: Code) {
    let round = s.meta().round;
    s.agenda_mut().log.push(AgendaRec {
        round,
        agenda,
        outcome: None,
        votes: crate::containers::SortedMap::new(),
        speaker_decided: false,
        law,
        enacted: false,
        discarded: true,
        redirected_to: None,
        discard_reason: Some(reason),
    });
}

/// `Game::push_agenda_record` for a finished vote: the public record of `agenda`'s vote. `counts` are the votes
/// each outcome received, `law` whether the agenda is a law (`vote::is_law`). A law is `enacted` when it was not
/// discarded and its outcome is not Against; the redirect (Confusing / Confounding Legal Text, still set when the
/// record is written) is listed unless it names the outcome itself or the agenda was discarded.
pub fn log_voted(
    s: &mut CoreState,
    agenda: Agenda,
    law: bool,
    outcome: Option<Outcome>,
    counts: &[(Outcome, i64)],
    speaker_decided: bool,
    discarded: bool,
) {
    let round = s.meta().round;
    let redirected_to = if discarded { None } else { s.agenda().elected_override.map(Outcome::Seat).filter(|to| Some(*to) != outcome) };
    s.agenda_mut().log.push(AgendaRec {
        round,
        agenda,
        outcome,
        votes: counts.iter().copied().collect(),
        speaker_decided,
        law,
        enacted: law && !discarded && outcome.is_some_and(|o| o != Outcome::Against),
        discarded,
        redirected_to,
        discard_reason: None,
    });
}

/// `GameState::enact_law`: `law` stays in play with the outcome it was elected on (an existing entry is replaced).
pub fn enact(s: &mut CoreState, law: Agenda, outcome: Outcome) {
    s.agenda_mut().laws.insert(law, outcome);
}

/// `laws::classified_leak`: the elected secret objective joins the revealed objectives; whether it was new. The
/// string checks that the law is in play (not blanked); the callers enact it first.
pub fn leak_secret(s: &mut CoreState, secret: SecretObjective) -> bool {
    let card = ObjectiveRef::Secret(secret);
    let revealed = &mut s.objectives_mut().revealed;
    if revealed.as_slice().contains(&card) {
        return false;
    }
    revealed.push(card);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_discarded_agenda_is_logged_without_an_outcome() {
        let mut s = CoreState::default();
        s.meta_mut().round = 3;
        log_discarded(&mut s, Agenda::ALL[1], true, Code(7));
        let rec = s.agenda().log.last().unwrap();
        assert_eq!((rec.round, rec.agenda, rec.law, rec.discarded, rec.enacted), (3, Agenda::ALL[1], true, true, false));
        assert_eq!((rec.outcome, rec.redirected_to, rec.speaker_decided, rec.discard_reason), (None, None, false, Some(Code(7))));
        assert!(rec.votes.is_empty());
        log_discarded(&mut s, Agenda::ALL[2], false, Code(8));
        assert_eq!(s.agenda().log.iter().count(), 2);
        assert!(!s.agenda().log.last().unwrap().law);
    }

    #[test]
    fn a_finished_vote_is_logged_with_its_flags() {
        let mut s = CoreState::default();
        s.meta_mut().round = 2;
        let seat = crate::ids::Seat(1);
        let counts = [(Outcome::For, 3), (Outcome::Against, 1)];
        log_voted(&mut s, Agenda::ALL[1], true, Some(Outcome::For), &counts, false, false);
        let rec = s.agenda().log.last().unwrap();
        assert_eq!((rec.round, rec.law, rec.enacted, rec.discarded, rec.speaker_decided), (2, true, true, false, false));
        assert_eq!((rec.outcome, rec.redirected_to, rec.discard_reason), (Some(Outcome::For), None, None));
        assert_eq!(rec.votes.get(&Outcome::For), Some(&3));
        // Against is never enacted, a discard never is and lists no redirect.
        s.agenda_mut().elected_override = Some(seat);
        log_voted(&mut s, Agenda::ALL[1], true, Some(Outcome::Against), &counts, true, false);
        let rec = s.agenda().log.last().unwrap();
        assert_eq!((rec.enacted, rec.redirected_to, rec.speaker_decided), (false, Some(Outcome::Seat(seat)), true));
        log_voted(&mut s, Agenda::ALL[1], true, Some(Outcome::For), &counts, false, true);
        let rec = s.agenda().log.last().unwrap();
        assert_eq!((rec.enacted, rec.redirected_to, rec.discarded), (false, None, true));
        // A redirect to the elected seat itself is not a redirect; a vote without an outcome enacts nothing.
        log_voted(&mut s, Agenda::ALL[2], false, Some(Outcome::Seat(seat)), &counts, false, false);
        assert_eq!(s.agenda().log.last().unwrap().redirected_to, None);
        log_voted(&mut s, Agenda::ALL[2], true, None, &[], false, false);
        assert!(!s.agenda().log.last().unwrap().enacted);
    }

    #[test]
    fn a_law_is_enacted_and_a_secret_leaks_once() {
        let mut s = CoreState::default();
        enact(&mut s, Agenda::ALL[3], Outcome::For);
        enact(&mut s, Agenda::ALL[3], Outcome::Against);
        assert_eq!(s.agenda().laws.get(&Agenda::ALL[3]), Some(&Outcome::Against));
        assert_eq!(s.agenda().laws.len(), 1);
        let secret = SecretObjective::ALL[0];
        assert!(leak_secret(&mut s, secret));
        assert!(!leak_secret(&mut s, secret));
        assert_eq!(s.objectives().revealed.as_slice(), [ObjectiveRef::Secret(secret)]);
    }
}
