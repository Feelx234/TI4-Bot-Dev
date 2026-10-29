//! Writers of the agenda log (package B9c-1): the rows a revealed agenda leaves without a vote
//! (`Game::note_discarded_agenda` over `push_agenda_record`).

use crate::generated::Agenda;
use crate::state::{AgendaRec, Code, CoreState};

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
}
