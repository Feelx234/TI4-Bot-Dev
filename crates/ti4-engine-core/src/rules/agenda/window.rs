//! The vote of one agenda as a [`Window`] (package B9; string `vote::VoteWindow`, LRR 8.2ii to 8.19).
//!
//! Stages: each voter names an outcome or abstains (`Outcome`), exhausts planets for it (`Planets`), the
//! votes are banked; when everyone has voted the most votes win, a tie or a silent table goes to the
//! speaker (`Tiebreak`). The window carries the order the votes are asked in (the adapter computes it with
//! the faction hooks: Nekro, Argent, Hack Election) and a snapshot of the facts the string engine reads from
//! the faction modules ([`Facts`]).
//!
//! Not expressed here, so the adapter does not open this window for such a position (the string window
//! runs alone): Genetic Recombination (Mahact), Gila the Silvertongue's and the Executive Order's trade
//! goods, Predictive Intelligence, the Deepwrought oceans, the faction vote bonuses (`vote_bonus`,
//! Blood Pact) and a vote with more than [`Tally::MAX`] outcomes. Their questions exist as `Ask`s and
//! render byte for byte (slice 2), their stages are the next step.

use crate::choice::agenda::{Opt, Tally};
use crate::choice::{Choice, Opt as TopOpt, Pick};
use crate::containers::{SeatSet, SmallList, SortedMap};
use crate::generated::{Agenda, Planet};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::keys::Outcome;
use crate::state::CoreState;
use crate::tables::Scope;

use super::{Cx, Env, Tiebreak};

/// What the string engine reads from the faction modules, taken when the window opens.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub scope: Scope,
    /// `laws::blanked`.
    pub blanked: bool,
    /// Seats with Elder Qanoj's ability: each planet they exhaust casts one more vote.
    pub qanoj: SeatSet,
}

impl Env for Facts {
    fn is_nekro(&self, _: Seat) -> bool {
        false
    }
    fn elder_qanoj(&self, who: Seat) -> bool {
        self.qanoj.contains(who)
    }
    fn votes_first(&self, _: Seat) -> bool {
        false
    }
    fn oceans(&self, _: Seat) -> SmallList<Planet, 4> {
        SmallList::new()
    }
    fn resource_bonus(&self, _: Planet) -> i64 {
        0
    }
}

/// Where the vote has reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Asking `order[index]` which outcome to back.
    Outcome(u8),
    /// Asking `order[index]` which planet to exhaust for `outcome`, `votes` cast so far.
    Planets { index: u8, outcome: Outcome, votes: i64 },
    /// The speaker decides a tie or a silent table.
    Tiebreak,
    /// Finished, with the winning outcome if there was one.
    Done(Option<Outcome>),
}

/// One agenda's vote.
#[derive(Debug, Clone)]
pub struct Vote {
    agenda: Agenda,
    choices: SmallList<Outcome, 16>,
    order: SmallList<Seat, 8>,
    stage: Stage,
    counts: SortedMap<Outcome, i64, 16>,
    facts: Facts,
}

impl Vote {
    /// Open the vote on `agenda` over `choices` (as `vote::outcomes` lists them) for the voters in `order`.
    #[must_use]
    pub fn new(agenda: Agenda, choices: &[Outcome], order: &[Seat], facts: Facts) -> Self {
        Self {
            agenda,
            choices: SmallList::from_slice(choices),
            order: SmallList::from_slice(order),
            stage: if choices.is_empty() { Stage::Done(None) } else { Stage::Outcome(0) },
            counts: SortedMap::new(),
            facts,
        }
    }

    /// The winning outcome, once finished.
    #[must_use]
    pub const fn winner(&self) -> Option<Outcome> {
        match self.stage {
            Stage::Done(w) => w,
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_done(&self) -> bool {
        matches!(self.stage, Stage::Done(_))
    }

    fn cx<'a>(&'a self, core: &'a CoreState) -> Cx<'a> {
        Cx { core, scope: self.facts.scope, blanked: self.facts.blanked, env: &self.facts }
    }

    fn counts_list(&self) -> SmallList<(Outcome, i64), 16> {
        let mut out = SmallList::new();
        for (o, c) in self.counts.iter() {
            out.push((*o, *c));
        }
        out
    }

    fn voter(&self, index: u8) -> Option<Seat> {
        self.order.as_slice().get(usize::from(index)).copied()
    }

    /// The question of the current stage (the string `pending_choice`).
    fn question(&self, core: &CoreState) -> Option<Choice> {
        let cx = self.cx(core);
        let counts = self.counts_list();
        match self.stage {
            Stage::Done(_) => None,
            Stage::Outcome(index) => {
                Some(super::vote_question(&cx, self.agenda, self.voter(index)?, self.choices.as_slice(), counts.as_slice(), None))
            }
            Stage::Planets { index, outcome, votes } => super::planets_question(
                &cx,
                self.agenda,
                self.voter(index)?,
                outcome,
                votes,
                false,
                self.choices.as_slice(),
                counts.as_slice(),
                false,
            ),
            Stage::Tiebreak => {
                let candidates = self.tied(&counts);
                Some(super::tiebreak_question(&cx, self.agenda, candidates.as_slice()))
            }
        }
    }

    fn tied(&self, counts: &SmallList<(Outcome, i64), 16>) -> SmallList<Outcome, 16> {
        match super::tiebreak_candidates(counts.as_slice(), self.choices.as_slice()) {
            Tiebreak::Tied(list) => SmallList::from_slice(list.as_slice()),
            Tiebreak::Silent(list) => list,
        }
    }

    /// A voter is done exhausting planets: bank the votes (no trade goods or Predictive Intelligence here).
    fn bank(&mut self, core: &CoreState, index: u8, outcome: Outcome, votes: i64) {
        if votes > 0
            && let Some(seat) = self.voter(index)
        {
            let total = votes + super::extra_votes(core, seat);
            let at = self.counts.get_or_insert_with(outcome, || 0);
            *at += total;
        }
        self.stage = Stage::Outcome(index + 1);
    }

    /// Everyone has voted: decide, or hand it to the speaker.
    fn close(&self) -> Stage {
        let counts = self.counts_list();
        if let Some(w) = super::undisputed_winner(counts.as_slice(), self.choices.as_slice()) {
            return Stage::Done(Some(w));
        }
        let candidates = self.tied(&counts);
        match candidates.as_slice() {
            [] => Stage::Done(None),
            [only] => Stage::Done(Some(*only)),
            _ => Stage::Tiebreak,
        }
    }
}

impl Window for Vote {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Outcome(index) if usize::from(index) >= self.order.len() => self.stage = self.close(),
                Stage::Outcome(_) | Stage::Tiebreak => break,
                Stage::Planets { index, outcome, votes } => {
                    let seat = self.voter(index);
                    let empty = seat.is_none_or(|s| super::planet_offers(&self.cx(sx.state), s, false).is_empty());
                    if empty {
                        self.bank(sx.state, index, outcome, votes);
                    } else {
                        break;
                    }
                }
                Stage::Done(_) => return Next::Done,
            }
        }
        match self.question(sx.state) {
            Some(choice) => Next::Continue(choice.ask),
            None => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        self.question(s)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let question = self.question(sx.state).ok_or(Illegal)?;
        let Pick::Opt(opt) = pick else { return Err(Illegal) };
        if !question.opts.as_slice().contains(&opt) {
            return Err(Illegal);
        }
        match (self.stage, opt) {
            (Stage::Outcome(index), TopOpt::Decline) => self.stage = Stage::Outcome(index + 1),
            (Stage::Outcome(index), TopOpt::Agenda(Opt::Back(outcome, _))) => {
                self.stage = Stage::Planets { index, outcome, votes: 0 };
            }
            (Stage::Planets { index, outcome, votes }, TopOpt::Decline) => self.bank(sx.state, index, outcome, votes),
            (Stage::Planets { index, outcome, votes }, TopOpt::Agenda(Opt::Exhaust { planet, face, .. })) => {
                let seat = self.voter(index).ok_or(Illegal)?;
                let cast = super::planet_offers(&self.cx(sx.state), seat, false)
                    .as_slice()
                    .iter()
                    .find(|(p, f, _)| *p == planet && *f == face)
                    .map(|(_, _, cast)| *cast)
                    .ok_or(Illegal)?;
                fx::exhaust_planet(sx.state, planet);
                self.stage = Stage::Planets { index, outcome, votes: votes + cast };
            }
            (Stage::Tiebreak, TopOpt::Agenda(Opt::Back(outcome, _))) => self.stage = Stage::Done(Some(outcome)),
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, Status};
    use crate::generated::Faction;
    use crate::kernel::Resolver;
    use crate::kernel::stage::AbilityTable;
    use crate::state::{Player, Players, Seats};

    fn facts() -> Facts {
        Facts { scope: Scope(!0), blanked: false, qanoj: SeatSet::EMPTY }
    }

    fn game(n: u8) -> CoreState {
        let mut s = CoreState::all_loaded();
        let players: Vec<Player> = (0..n).map(|i| Player::new(Seat(i), Faction::ALL[usize::from(i)])).collect();
        let list: Vec<Seat> = (0..n).map(Seat).collect();
        s.seats = Seats::new(&list, &list, n);
        s.players = Players::from_vec(players);
        s
    }

    fn run(window: Vote, s: &mut CoreState, answers: &[TopOpt]) -> (Driver<Vote>, Status) {
        let k = Kernel::plain();
        let mut d = Driver::new(window, Resolver::new(&[Seat(0)], Some(Seat(0))), AbilityTable::default(), 1);
        let mut status = d.run(s, &k).unwrap();
        for a in answers {
            assert!(matches!(status, Status::Asking(_)), "asks before {a:?}");
            status = d.resume(s, &k, Pick::Opt(*a)).unwrap();
        }
        (d, status)
    }

    #[test]
    fn nobody_backs_anything_and_the_speaker_decides_a_silent_table() {
        let mut s = game(2);
        let w = Vote::new(Agenda::ChecksAndBalances, &[Outcome::For, Outcome::Against], &[Seat(0), Seat(1)], facts());
        let (d, status) = run(w, &mut s, &[TopOpt::Decline, TopOpt::Decline]);
        let Status::Asking(c) = status else { panic!("the speaker is asked") };
        assert_eq!(c.opts.len(), 2);
        let (mut d, k) = (d, Kernel::plain());
        let pick = c.pick(1).unwrap();
        assert_eq!(d.resume(&mut s, &k, pick).unwrap(), Status::Done);
        assert_eq!(d.window.winner(), Some(Outcome::Against));
    }

    #[test]
    fn a_wrong_pick_is_refused_and_a_finished_vote_has_a_winner() {
        let mut s = game(1);
        let w = Vote::new(Agenda::ChecksAndBalances, &[Outcome::For, Outcome::Against], &[Seat(0)], facts());
        let k = Kernel::plain();
        let mut d = Driver::new(w, Resolver::new(&[Seat(0)], Some(Seat(0))), AbilityTable::default(), 1);
        d.run(&mut s, &k).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Number(1)), Err(crate::driver::DriverError::Illegal));
        // a seat with no planets: backing "for" casts nothing (the planets stage is skipped), the table is silent
        let back = TopOpt::Agenda(Opt::Back(Outcome::For, crate::choice::agenda::Shown::default()));
        let status = d.resume(&mut s, &k, Pick::Opt(back)).unwrap();
        assert!(matches!(status, Status::Asking(_)), "silence goes to the speaker");
    }
}
