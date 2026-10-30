//! Agenda, laws, the invasion and the space battle, promissory notes in play: `Observed::{law_outcome,
//! agenda_in_play, agendas_resolved_this_round, agenda_prediction_of, invasion, space_battle,
//! promissory_notes, support_holders}`.

use super::Seen;
use crate::generated::{Agenda, System};
use crate::ids::Seat;
use crate::keys::{Outcome, Prediction};
use crate::state::{ActiveInvasion, NoteKey};

/// The public invasion boundary (`ActiveInvasion`).
pub type InvasionFacts = ActiveInvasion;

/// The agenda in play with its history this round.
#[derive(Debug, Clone, Copy)]
pub struct AgendaFacts {
    pub in_play: Option<Agenda>,
    pub resolved_this_round: usize,
}

impl Seen<'_> {
    /// The outcome elected for a law in play (`Observed::law_outcome`).
    #[must_use]
    pub fn law_outcome(&self, law: Agenda) -> Option<Outcome> {
        self.st().agenda().laws.get(&law).copied()
    }

    /// The agenda being voted on (`Observed::agenda_in_play`).
    #[must_use]
    pub fn agenda_in_play(&self) -> Option<Agenda> {
        self.st().agenda().in_play
    }

    /// How many agendas were resolved this round, discarded ones not counted
    /// (`Observed::agendas_resolved_this_round`).
    #[must_use]
    pub fn agendas_resolved_this_round(&self) -> usize {
        let round = self.round();
        self.st().agenda().log.iter().filter(|r| r.round == round && r.discard_reason.is_none()).count()
    }

    /// The outcome a seat predicted on the agenda in play (`Observed::agenda_prediction_of`).
    #[must_use]
    pub fn agenda_prediction_of(&self, seat: Seat) -> Option<Prediction> {
        self.st().agenda().predictions.get(&seat).copied()
    }

    /// The public invasion boundary (`Observed::invasion`).
    #[must_use]
    pub fn invasion(&self) -> Option<&ActiveInvasion> {
        self.st().transient().invasion.active.as_deref()
    }

    /// The public battle boundary: `(system, attacker, defender)` (`Observed::space_battle`).
    #[must_use]
    pub fn space_battle(&self) -> Option<(System, Seat, Seat)> {
        self.st().transient().combat.active_space_combat
    }

    /// The agenda facts as one value.
    #[must_use]
    pub fn agenda_facts(&self) -> AgendaFacts {
        AgendaFacts { in_play: self.agenda_in_play(), resolved_this_round: self.agendas_resolved_this_round() }
    }

    /// Promissory notes faceup in a play area, with their holder (`Observed::promissory_notes`), in the
    /// typed key order.
    #[must_use]
    pub fn promissory_notes_in_play(&self) -> Vec<(NoteKey, Seat)> {
        let cards = self.st().cards();
        cards.promissory.iter().filter(|(k, _)| cards.promissory_faceup.contains_key(k)).map(|(k, h)| (*k, *h)).collect()
    }

    /// Support for the Throne: owner -> the seat holding it faceup (`Observed::support_holders`).
    #[must_use]
    pub fn support_holders(&self) -> Vec<(Seat, Seat)> {
        self.st().trade().support_holders.iter().map(|(o, h)| (*o, *h)).collect()
    }
}
