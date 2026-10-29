//! Committee Formation's election as a [`Window`] (package B9c-2; string `Game::committee_formation` after the owner
//! discarded the card, and the record `open_next_vote` writes).
//!
//! "Before players vote on an agenda that requires a player to be elected, the owner of this card may discard this
//! card to choose a player to be elected. Players do not vote on that agenda." The discard is the
//! [`OfferDiscard`](super::offer_discard::OfferDiscard) window; this one asks the owner which player is elected, then
//! lists the agenda with that outcome (no votes, never speaker-decided) and, for a law, enacts it. Copied from the
//! string: nothing resolves the agenda's effect, no typed `AGENDA_RESOLVED` is emitted through the resolver (the plain
//! `AGENDA_RESOLVED:{agenda}:{player}` and `LAW_ENACTED` of `Game::emit` go to the emission queue, `sx.plain`), and a table
//! with no players asks nothing and records nothing.

use super::is_law;
use crate::choice::agenda::{Ask as AAsk, Opt as AOpt};
use crate::choice::generic::Content;
use crate::choice::{Choice, Opt, Opts, Origin, Pick};
use crate::generated::Agenda;
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx_agenda, Kernel, Next};
use crate::keys::Outcome;
use crate::state::{Code, CoreState, PlainArg, PlainKind, PlainRec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Asking,
    Done,
}

/// The election by Committee Formation.
#[derive(Debug, Clone)]
pub struct Committee {
    agenda: Agenda,
    owner: Seat,
    /// The interned text the question's origin names (display only).
    source: Code,
    stage: Stage,
    chosen: Option<Seat>,
}

impl Committee {
    #[must_use]
    pub const fn new(agenda: Agenda, owner: Seat, source: Code) -> Self {
        Self { agenda, owner, source, stage: Stage::Start, chosen: None }
    }

    /// The player the owner chose; `None` when nobody was asked.
    #[must_use]
    pub const fn chosen(&self) -> Option<Seat> {
        self.chosen
    }
}

impl Window for Committee {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            self.stage = if sx.state.players().iter().next().is_none() { Stage::Done } else { Stage::Asking };
        }
        match self.stage {
            Stage::Asking => Next::Continue(crate::choice::Ask::Agenda(AAsk::Committee { agenda: self.agenda })),
            _ => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        if self.stage != Stage::Asking {
            return None;
        }
        let opts: Opts = s.players().iter().map(|p| Opt::Agenda(AOpt::Elect(p.seat))).collect();
        Some(super::question(self.owner, AAsk::Committee { agenda: self.agenda }, Origin::Content(Content::Other(self.source)), opts))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Agenda(AOpt::Elect(seat))) = pick else { return Err(Illegal) };
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        self.chosen = Some(seat);
        sx.plain(PlainRec::two(PlainKind::AgendaResolved, PlainArg::Agenda(self.agenda), PlainArg::Outcome(Outcome::Seat(seat))));
        let law = is_law(self.agenda);
        fx_agenda::log_voted(sx.state, self.agenda, law, Some(Outcome::Seat(seat)), &[], false, false);
        if law {
            fx_agenda::enact(sx.state, self.agenda, Outcome::Seat(seat));
            sx.plain(PlainRec::two(PlainKind::LawEnacted, PlainArg::Agenda(self.agenda), PlainArg::Outcome(Outcome::Seat(seat))));
        }
        self.stage = Stage::Done;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, Status};
    use crate::generated::Faction;
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{Player, Players, Seats};

    fn game(n: u8) -> CoreState {
        let mut s = CoreState::all_loaded();
        let players: Vec<Player> = (0..n).map(|i| Player::new(Seat(i), Faction::ALL[usize::from(i)])).collect();
        let list: Vec<Seat> = (0..n).map(Seat).collect();
        s.seats = Seats::new(&list, &list, n);
        s.players = Players::from_vec(players);
        s
    }

    fn start(agenda: Agenda, s: &mut CoreState) -> (Driver<Committee>, Status) {
        let k = Kernel::plain();
        let mut d = Driver::new(Committee::new(agenda, Seat(1), Code(3)), Resolver::new(&[Seat(1)], Some(Seat(1))), AbilityTable::default(), 1);
        let status = d.run(s, &k).unwrap();
        (d, status)
    }

    #[test]
    fn the_owner_names_a_player_and_a_law_is_listed_and_enacted() {
        let mut s = game(3);
        s.meta_mut().round = 4;
        let (mut d, status) = start(Agenda::PoliticalCensure, &mut s);
        let Status::Asking(c) = status else { panic!("asks") };
        assert_eq!(c.actor, Seat(1));
        assert_eq!(c.opts.len(), 3, "every player is a candidate, the owner too");
        let pick = c.pick(2).unwrap();
        assert_eq!(d.resume(&mut s, &Kernel::plain(), pick).unwrap(), Status::Done);
        assert_eq!(d.window.chosen(), Some(Seat(2)));
        assert_eq!(s.agenda().laws.get(&Agenda::PoliticalCensure), Some(&Outcome::Seat(Seat(2))));
        let rec = s.agenda().log.last().unwrap();
        assert_eq!((rec.round, rec.outcome, rec.law, rec.enacted, rec.discarded, rec.speaker_decided, rec.votes.len()), (4, Some(Outcome::Seat(Seat(2))), true, true, false, false, 0));
    }

    #[test]
    fn the_election_queues_the_resolution_and_a_law_enactment_as_plain_events() {
        use crate::state::queue_kinds;
        use PlainKind::{AgendaResolved, LawEnacted};
        let mut s = game(3);
        let (mut d, status) = start(Agenda::PoliticalCensure, &mut s);
        let Status::Asking(c) = status else { panic!("asks") };
        assert!(queue_kinds(&s).is_empty(), "nothing before the answer");
        d.resume(&mut s, &Kernel::plain(), c.pick(2).unwrap()).unwrap();
        assert_eq!(queue_kinds(&s), [Ok(AgendaResolved), Ok(LawEnacted)], "no typed resolution is emitted (copied from the string)");
        let first = s.ledger.events.iter().find_map(|e| e.plain().copied());
        assert_eq!(first, Some(PlainRec::two(AgendaResolved, PlainArg::Agenda(Agenda::PoliticalCensure), PlainArg::Outcome(Outcome::Seat(Seat(2))))));
        // A directive is resolved but not enacted.
        let mut s = game(3);
        let (mut d, status) = start(Agenda::ArmedForcesStandardization, &mut s);
        let Status::Asking(c) = status else { panic!("asks") };
        d.resume(&mut s, &Kernel::plain(), c.pick(0).unwrap()).unwrap();
        assert_eq!(queue_kinds(&s), [Ok(AgendaResolved)]);
    }

    #[test]
    fn a_directive_is_listed_but_not_enacted() {
        let mut s = game(3);
        let directive = Agenda::ArmedForcesStandardization;
        assert_eq!(directive.data().kind, crate::generated::AgendaType::Directive);
        let (mut d, status) = start(directive, &mut s);
        let Status::Asking(c) = status else { panic!("asks") };
        d.resume(&mut s, &Kernel::plain(), c.pick(0).unwrap()).unwrap();
        assert!(s.agenda().laws.is_empty());
        assert!(!s.agenda().log.last().unwrap().law);
    }

    #[test]
    fn a_pick_that_is_not_a_seat_is_refused_and_an_empty_table_asks_nothing() {
        let mut s = game(2);
        let (mut d, _) = start(Agenda::PoliticalCensure, &mut s);
        assert!(d.resume(&mut s, &Kernel::plain(), Pick::Opt(Opt::Decline)).is_err());
        assert!(s.agenda().log.iter().next().is_none());
        let mut empty = CoreState::all_loaded();
        let (d, status) = start(Agenda::PoliticalCensure, &mut empty);
        assert_eq!(status, Status::Done);
        assert_eq!(d.window.chosen(), None);
        assert!(empty.agenda().log.iter().next().is_none());
    }
}
