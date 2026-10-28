//! The vote of one agenda as a [`Window`] (package B9; string `vote::VoteWindow`, LRR 8.2ii to 8.19).
//!
//! Stages: Genetic Recombination's offer to its holder (`Recombine`) and its tribute (`Tribute`), each voter
//! names an outcome or abstains (`Outcome`), exhausts planets for it (`Planets`), may spend trade goods
//! (`TradeGoods`: Gila the Silvertongue, the Executive Order spender) and cast Predictive Intelligence's three
//! extra votes (`Predictive`); the votes are banked; when everyone has voted the most votes win, a tie or a
//! silent table goes to the speaker (`Tiebreak`). The window carries the order the votes are asked in (the
//! adapter computes it with the faction hooks: Nekro, Argent, Hack Election) and a snapshot of the facts the
//! string engine reads from the faction modules ([`Facts`]).
//!
//! Not expressed here, so the adapter does not open this window for such a position (the string window
//! runs alone): the Deepwrought oceans, a Valefar Assimilator carrying Genetic Recombination, an open or
//! grantable Keleres agent for a voter with commodities (the game driver offers it between the stages), a
//! Last Bastion resource bonus under the Executive Order, and a vote with more than [`Tally::MAX`] outcomes.

use crate::choice::agenda::Opt;
use crate::choice::{Choice, Opt as TopOpt, Pick};
use crate::containers::{SeatSet, SmallList, SortedMap};
use crate::generated::{Agenda, Planet, Technology};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::keys::Outcome;
use crate::state::CoreState;
use crate::tables::Scope;

use super::{Cx, Env, Tiebreak};

/// Votes of Predictive Intelligence's additional cast.
pub const PREDICTIVE_VOTES: i64 = 3;
/// Votes the Blood Pact adds when the partner backed the same outcome.
pub const BLOOD_PACT_VOTES: i64 = 4;

/// What the string engine reads from the faction modules, taken when the window opens.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub scope: Scope,
    /// `laws::blanked`.
    pub blanked: bool,
    /// Seats with Elder Qanoj's ability: each planet they exhaust casts one more vote.
    pub qanoj: SeatSet,
    /// Seats with Gila the Silvertongue's ability: two votes per trade good spent.
    pub gila: SeatSet,
    /// Votes a seat's cards and commanders add once it casts any (`leaders::vote_bonus` plus the content hooks).
    pub bonus: [i64; 8],
    /// The Blood Pact partners of a seat: it gets [`BLOOD_PACT_VOTES`] for each that backed the same outcome.
    pub pact: [SeatSet; 8],
}

impl Facts {
    #[must_use]
    pub const fn new(scope: Scope, blanked: bool) -> Self {
        Self { scope, blanked, qanoj: SeatSet::EMPTY, gila: SeatSet::EMPTY, bonus: [0; 8], pact: [SeatSet::EMPTY; 8] }
    }
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
    /// Genetic Recombination: asking its holder which outcome `order[index]` must back.
    Recombine(u8),
    /// Genetic Recombination was used on `order[index]`: back the outcome or return a fleet token.
    Tribute(u8),
    /// Asking `order[index]` which planet to exhaust for `outcome`, `votes` cast so far.
    Planets { index: u8, outcome: Outcome, votes: i64 },
    /// Asking `order[index]` how many trade goods to spend for votes.
    TradeGoods { index: u8, outcome: Outcome, votes: i64 },
    /// Asking `order[index]`, who holds a ready Predictive Intelligence, whether to cast 3 more votes.
    Predictive { index: u8, outcome: Outcome, votes: i64 },
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
    /// Who backed what (the Blood Pact reads it).
    ballot: SortedMap<Seat, Outcome, 8>,
    facts: Facts,
    /// Executive Order: the seat that may spend resources and trade goods as votes.
    spender: Option<Seat>,
    /// Bit `i`: voter `i` was already offered to Genetic Recombination's holder.
    offered: u8,
    /// Genetic Recombination in force: the voter's index and the outcome they must back.
    obligation: Option<(u8, Outcome)>,
    /// The outcome the holder demanded of the voter answering [`Stage::Tribute`].
    demanded: Option<(u8, Outcome)>,
    /// Predictive Intelligence holders who cast the 3 extra votes, with the outcome they backed.
    predictive: SmallList<(Seat, Outcome), 8>,
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
            ballot: SortedMap::new(),
            facts,
            spender: None,
            offered: 0,
            obligation: None,
            demanded: None,
            predictive: SmallList::new(),
        }
    }

    /// Let `seat` spend resources and trade goods as votes (Executive Order).
    #[must_use]
    pub const fn with_spender(mut self, seat: Seat) -> Self {
        self.spender = Some(seat);
        self
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

    fn is_spender(&self, seat: Seat) -> bool {
        self.spender == Some(seat)
    }

    fn offers(&self, core: &CoreState, seat: Seat) -> SmallList<super::PlanetOffer, 24> {
        super::planet_offers(&self.cx(core), seat, self.is_spender(seat))
    }

    /// `VoteWindow::trade_goods_rate`.
    fn rate(&self, seat: Seat, votes: i64) -> i64 {
        if votes > 0 && self.facts.gila.contains(seat) { 2 } else { i64::from(self.is_spender(seat)) }
    }

    fn goods(core: &CoreState, seat: Seat) -> i64 {
        core.players().by_seat(seat).map_or(0, |p| i64::from(p.econ.trade_goods.max(0)))
    }

    /// `mahact::recombination_holder`: another seat with a ready Genetic Recombination.
    fn holder(core: &CoreState, voter: Seat) -> Option<Seat> {
        core.players()
            .iter()
            .find(|p| p.seat != voter && p.tech.owned.contains(Technology::GeneticRecombination) && !p.tech.exhausted.contains(Technology::GeneticRecombination))
            .map(|p| p.seat)
    }

    fn can_pay(core: &CoreState, voter: Seat) -> bool {
        core.players().by_seat(voter).is_some_and(|p| p.econ.fleet_tokens > 0)
    }

    /// `vote::predictive_ready`.
    fn predictive_ready(core: &CoreState, seat: Seat) -> bool {
        core.meta().rules_at_least(super::RULES_TECH_EFFECTS)
            && core
                .players()
                .by_seat(seat)
                .is_some_and(|p| p.tech.owned.contains(Technology::PredictiveIntelligence) && !p.tech.exhausted.contains(Technology::PredictiveIntelligence))
    }

    /// The question of the current stage (the string `pending_choice`).
    fn question(&self, core: &CoreState) -> Option<Choice> {
        let cx = self.cx(core);
        let counts = self.counts_list();
        match self.stage {
            Stage::Done(_) => None,
            Stage::Outcome(index) => {
                let voter = self.voter(index)?;
                let bound = self.obligation.filter(|(at, _)| *at == index).map(|(_, o)| o);
                Some(super::vote_question(&cx, self.agenda, voter, self.choices.as_slice(), counts.as_slice(), bound))
            }
            Stage::Recombine(index) => {
                let voter = self.voter(index)?;
                let holder = Self::holder(core, voter)?;
                Some(super::recombine_question(core, self.agenda, holder, voter, self.choices.as_slice()))
            }
            Stage::Tribute(index) => {
                let voter = self.voter(index)?;
                let (_, outcome) = self.demanded.filter(|(at, _)| *at == index)?;
                super::tribute_question(core, self.agenda, voter, outcome, !self.offers(core, voter).is_empty(), Self::can_pay(core, voter))
            }
            Stage::Planets { index, outcome, votes } => {
                let voter = self.voter(index)?;
                let must_cast = votes == 0 && self.obligation.is_some_and(|(at, backed)| at == index && backed == outcome);
                super::planets_question(
                    &cx,
                    self.agenda,
                    voter,
                    outcome,
                    votes,
                    self.is_spender(voter),
                    self.spender.is_some(),
                    self.choices.as_slice(),
                    counts.as_slice(),
                    must_cast,
                )
            }
            Stage::TradeGoods { index, outcome, votes } => {
                let voter = self.voter(index)?;
                super::trade_goods_question(core, self.agenda, voter, outcome, votes, Self::goods(core, voter), self.rate(voter, votes))
            }
            Stage::Predictive { index, outcome, votes } => {
                Some(super::predictive_question(core, self.agenda, self.voter(index)?, outcome, votes))
            }
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

    /// `VoteWindow::finish_planets`: a voter is done exhausting planets; offer trade goods when they may spend
    /// them and have some, else bank.
    fn finish_planets(&mut self, core: &CoreState, index: u8, outcome: Outcome, votes: i64) {
        let may_spend = self.voter(index).is_some_and(|v| self.rate(v, votes) > 0 && Self::goods(core, v) > 0);
        if may_spend {
            self.stage = Stage::TradeGoods { index, outcome, votes };
        } else {
            self.bank(core, index, outcome, votes);
        }
    }

    /// `VoteWindow::bank`: offer Predictive Intelligence's extra votes when a ready card is held and any vote
    /// was cast, else record them and move on.
    fn bank(&mut self, core: &CoreState, index: u8, outcome: Outcome, votes: i64) {
        if votes > 0 && self.voter(index).is_some_and(|v| Self::predictive_ready(core, v)) {
            self.stage = Stage::Predictive { index, outcome, votes };
        } else {
            self.record(core, index, outcome, votes);
            self.stage = Stage::Outcome(index + 1);
        }
    }

    /// `VoteWindow::record`: bank one voter's votes; nothing cast, nothing recorded.
    fn record(&mut self, core: &CoreState, index: u8, outcome: Outcome, votes: i64) {
        let Some(seat) = self.voter(index) else { return };
        if votes <= 0 {
            return;
        }
        let pact = self.facts.pact[usize::from(seat.0) & 7]
            .iter()
            .filter(|other| self.ballot.get(other) == Some(&outcome))
            .count();
        let total = votes
            + self.facts.bonus[usize::from(seat.0) & 7]
            + super::extra_votes(core, seat)
            + BLOOD_PACT_VOTES * i64::try_from(pact).unwrap_or(0);
        self.ballot.insert(seat, outcome);
        *self.counts.get_or_insert_with(outcome, || 0) += total;
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

    /// `settle_predictions`: each holder who cast the extra votes exhausts the card unless the outcome won.
    fn settle_predictions(&mut self, core: &mut CoreState) {
        let Stage::Done(winner) = self.stage else { return };
        for (seat, backed) in self.predictive.as_slice() {
            if winner != Some(*backed)
                && let Some(p) = core.players_mut().by_seat_mut(*seat)
            {
                p.tech.exhausted.insert(Technology::PredictiveIntelligence);
            }
        }
        self.predictive.clear();
    }

    fn pay_tribute(core: &mut CoreState, voter: Seat) {
        if let Some(p) = core.players_mut().by_seat_mut(voter)
            && p.econ.fleet_tokens > 0
        {
            p.econ.fleet_tokens -= 1;
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
            // Genetic Recombination: "before a player casts votes", once per voter, the holder is asked.
            if let Stage::Outcome(index) = self.stage
                && usize::from(index) < self.order.len()
                && self.offered & (1 << index) == 0
            {
                self.offered |= 1 << index;
                if self.voter(index).is_some_and(|v| Self::holder(sx.state, v).is_some()) {
                    self.stage = Stage::Recombine(index);
                    break;
                }
            }
            match self.stage {
                Stage::Outcome(index) if usize::from(index) >= self.order.len() => {
                    self.stage = self.close();
                    self.settle_predictions(sx.state);
                }
                Stage::Planets { index, outcome, votes } => {
                    let empty = self.voter(index).is_none_or(|s| self.offers(sx.state, s).is_empty());
                    if empty {
                        self.finish_planets(sx.state, index, outcome, votes);
                    } else {
                        break;
                    }
                }
                Stage::Done(_) => return Next::Done,
                _ => break,
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
            (Stage::Recombine(index), TopOpt::Decline) => self.stage = Stage::Outcome(index),
            (Stage::Recombine(index), TopOpt::Agenda(Opt::Demand(outcome))) => {
                let voter = self.voter(index).ok_or(Illegal)?;
                let holder = Self::holder(sx.state, voter).ok_or(Illegal)?;
                if let Some(p) = sx.state.players_mut().by_seat_mut(holder) {
                    p.tech.exhausted.insert(Technology::GeneticRecombination);
                }
                let can_vote = !self.offers(sx.state, voter).is_empty();
                let can_pay = Self::can_pay(sx.state, voter);
                self.stage = Stage::Outcome(index);
                match (can_vote, can_pay) {
                    (true, true) => {
                        self.demanded = Some((index, outcome));
                        self.stage = Stage::Tribute(index);
                    }
                    (true, false) => self.obligation = Some((index, outcome)),
                    (false, true) => Self::pay_tribute(sx.state, voter),
                    (false, false) => {}
                }
            }
            (Stage::Tribute(index), TopOpt::Agenda(Opt::TributeVote)) => {
                self.obligation = self.demanded.take();
                self.stage = Stage::Outcome(index);
            }
            (Stage::Tribute(index), TopOpt::Agenda(Opt::TributeToken)) => {
                self.demanded = None;
                let voter = self.voter(index).ok_or(Illegal)?;
                Self::pay_tribute(sx.state, voter);
                self.stage = Stage::Outcome(index);
            }
            (Stage::Outcome(index), TopOpt::Decline) => self.stage = Stage::Outcome(index + 1),
            (Stage::Outcome(index), TopOpt::Agenda(Opt::Back(outcome, _))) => {
                self.stage = Stage::Planets { index, outcome, votes: 0 };
            }
            (Stage::Planets { index, outcome, votes }, TopOpt::Decline) => self.finish_planets(sx.state, index, outcome, votes),
            (Stage::Planets { index, outcome, votes }, TopOpt::Agenda(Opt::Exhaust { planet, face, .. })) => {
                let seat = self.voter(index).ok_or(Illegal)?;
                let cast = self
                    .offers(sx.state, seat)
                    .as_slice()
                    .iter()
                    .find(|(p, f, _)| *p == planet && *f == face)
                    .map(|(_, _, cast)| *cast)
                    .ok_or(Illegal)?;
                fx::exhaust_planet(sx.state, planet);
                self.stage = Stage::Planets { index, outcome, votes: votes + cast };
            }
            (Stage::TradeGoods { index, outcome, votes }, TopOpt::Decline) => self.bank(sx.state, index, outcome, votes),
            (Stage::TradeGoods { index, outcome, votes }, TopOpt::Agenda(Opt::Spend(spent))) => {
                let seat = self.voter(index).ok_or(Illegal)?;
                let rate = self.rate(seat, votes);
                let p = sx.state.players_mut().by_seat_mut(seat).ok_or(Illegal)?;
                p.econ.trade_goods -= i16::from(spent);
                self.bank(sx.state, index, outcome, votes + rate * i64::from(spent));
            }
            (Stage::Predictive { index, outcome, votes }, TopOpt::Decline) => {
                self.record(sx.state, index, outcome, votes);
                self.stage = Stage::Outcome(index + 1);
            }
            (Stage::Predictive { index, outcome, votes }, TopOpt::Agenda(Opt::CastPredictive)) => {
                let seat = self.voter(index).ok_or(Illegal)?;
                self.predictive.push((seat, outcome));
                self.record(sx.state, index, outcome, votes + PREDICTIVE_VOTES);
                self.stage = Stage::Outcome(index + 1);
            }
            (Stage::Tiebreak, TopOpt::Agenda(Opt::Back(outcome, _))) => {
                self.stage = Stage::Done(Some(outcome));
                self.settle_predictions(sx.state);
            }
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
        Facts::new(Scope(!0), false)
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
