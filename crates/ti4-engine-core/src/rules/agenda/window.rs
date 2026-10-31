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
use crate::generated::{Agenda, Leader, Planet, Technology};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::keys::Outcome;
use crate::state::CoreState;
use crate::tables::Scope;

use super::{Cx, Env, Tiebreak};
use crate::query::leaders;

/// Votes of Predictive Intelligence's additional cast.
pub const PREDICTIVE_VOTES: i64 = 3;
/// Votes the Blood Pact adds when the partner backed the same outcome.
pub const BLOOD_PACT_VOTES: i64 = 4;

/// What the window cannot read off the state or the kernel, taken when it opens. The faction answers (Elder
/// Qanoj, Gila the Silvertongue, the vote bonuses, the oceans, Last Bastion's dock, the Keleres commodities) come
/// from the kernel's hook table and the commander helper `rules::leaders::has_commander_ability_in`.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub scope: Scope,
    /// `laws::blanked`.
    pub blanked: bool,
    /// The Blood Pact partners of a seat: it gets [`BLOOD_PACT_VOTES`] for each that backed the same outcome.
    pub pact: [SeatSet; 8],
}

impl Facts {
    #[must_use]
    pub const fn new(scope: Scope, blanked: bool) -> Self {
        Self { scope, blanked, pact: [SeatSet::EMPTY; 8] }
    }

    /// The facts the position says: the kernel's source scope, the blanked laws and the Blood Pact partners.
    #[must_use]
    pub fn of_core(core: &CoreState, k: &Kernel) -> Self {
        Self { scope: crate::query::reach::scope_of(k), blanked: crate::query::reach::blanked(core), pact: pact_partners(core) }
    }
}

/// `empyrean_units::blood_pact_votes`: a faceup Blood Pact held by a seat other than its owner links the two (each gets
/// [`BLOOD_PACT_VOTES`] when the other backed the same outcome). The owner is the first seat in seating order that plays the note's faction
/// (`promissory::seat_of`).
#[must_use]
pub fn pact_partners(core: &CoreState) -> [SeatSet; 8] {
    let mut pact = [SeatSet::EMPTY; 8];
    for (key, holder) in core.cards().promissory.iter() {
        if key.note != crate::generated::PromissoryNote::BloodPact || !core.cards().promissory_faceup.contains_key(key) {
            continue;
        }
        let owner = core.seats().seating_order().iter().copied().find(|seat| core.players().by_seat(*seat).is_some_and(|p| p.faction == key.faction));
        let Some(owner) = owner else { continue };
        if owner == *holder {
            continue;
        }
        pact[usize::from(holder.0) & 7].insert(owner);
        pact[usize::from(owner.0) & 7].insert(*holder);
    }
    pact
}

/// The [`Env`] of the vote: the hook table and the commander ability, over the state.
pub struct KernelEnv<'a> {
    pub core: &'a CoreState,
    pub kernel: &'a Kernel,
}

/// `VoteWindow::new`'s order for the position (the live path: the Nekro bar, a rider's cost, Argent's first vote, Hack Election's last one).
#[must_use]
pub fn voting_order_of(core: &CoreState, k: &Kernel) -> SmallList<Seat, 8> {
    let env = KernelEnv { core, kernel: k };
    let facts = Facts::of_core(core, k);
    super::voting_order(&Cx { core, scope: facts.scope, blanked: facts.blanked, env: &env })
}

impl Env for KernelEnv<'_> {
    fn is_nekro(&self, who: Seat) -> bool {
        // Galactic Threat: "You cannot vote on agendas" (`factions::nekro::is_nekro`: the seat plays the Nekro Virus).
        self.core.players().by_seat(who).is_some_and(|p| p.faction == crate::generated::Faction::TheNekroVirus)
    }
    fn elder_qanoj(&self, who: Seat) -> bool {
        leaders::has_commander_ability_in(self.core, who, Leader::ElderQanoj)
    }
    fn votes_first(&self, who: Seat) -> bool {
        self.kernel.hooks.cards.votes_first(self.core, who)
    }
    fn oceans(&self, who: Seat) -> SmallList<Planet, 4> {
        self.kernel.hooks.direct.deepwrought_oceans(self.core, who)
    }
    fn resource_bonus(&self, planet: Planet) -> i64 {
        self.kernel.hooks.direct.bastion_units_resource_bonus(self.core, planet)
    }
}

/// Where the vote has reached (the payload type of `Pending`, `state::VoteStage`).
pub use crate::state::VoteStage as Stage;

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
    /// What each voter cast, in voting order, abstentions included (`None`: abstained). Display only (the string mirrors it to
    /// `GameState::agenda_casts`); no rule reads it.
    casts: SmallList<(Seat, Option<Outcome>, i64), 8>,
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
            casts: SmallList::new(),
        }
    }

    // -- the payload of a held vote (`state::Vote`): the machine is rebuilt from it and the position at every live call -----------------

    /// The vote a payload stands for. `facts` are what the position says (`Facts::of_core`); everything else is the payload's.
    #[must_use]
    pub fn restore(p: &crate::state::Vote, facts: Facts) -> Self {
        let mut offered = 0u8;
        for i in p.recombination_offered.as_slice() {
            offered |= 1 << (i & 7);
        }
        Self {
            agenda: p.agenda,
            choices: p.choices.as_slice().iter().copied().collect(),
            order: p.order.as_slice().iter().copied().collect(),
            stage: p.stage,
            counts: p.counts.as_slice().iter().copied().collect(),
            ballot: p.ballot.as_slice().iter().copied().collect(),
            facts,
            spender: p.spender,
            offered,
            obligation: p.obligation,
            demanded: p.demanded,
            predictive: p.predictive.as_slice().iter().copied().collect(),
            casts: p.casts.as_slice().iter().copied().collect(),
        }
    }

    /// The payload of this vote (the inverse of [`Self::restore`]).
    #[must_use]
    pub fn to_payload(&self) -> crate::state::Vote {
        crate::state::Vote {
            agenda: self.agenda,
            choices: self.choices.as_slice().iter().copied().collect(),
            order: self.order.as_slice().iter().copied().collect(),
            stage: self.stage,
            ballot: self.ballot.iter().map(|(s, o)| (*s, *o)).collect(),
            counts: self.counts.iter().map(|(o, c)| (*o, *c)).collect(),
            casts: self.casts.as_slice().iter().copied().collect(),
            spender: self.spender,
            recombination_offered: (0..8u8).filter(|i| self.offered & (1 << i) != 0).collect(),
            obligation: self.obligation,
            demanded: self.demanded,
            predictive: self.predictive.as_slice().iter().copied().collect(),
        }
    }

    /// The agenda voted on (Covert Legislation itself under Covert Legislation).
    #[must_use]
    pub const fn agenda(&self) -> Agenda {
        self.agenda
    }

    /// The outcomes on offer.
    #[must_use]
    pub fn choices(&self) -> &[Outcome] {
        self.choices.as_slice()
    }

    /// Who backed what, in seat order.
    #[must_use]
    pub fn ballot(&self) -> SmallList<(Seat, Outcome), 8> {
        self.ballot.iter().map(|(s, o)| (*s, *o)).collect()
    }

    /// What each voter cast so far, abstentions included.
    #[must_use]
    pub fn casts(&self) -> &[(Seat, Option<Outcome>, i64)] {
        self.casts.as_slice()
    }

    /// The seats in the order the votes are asked.
    #[must_use]
    pub fn order(&self) -> &[Seat] {
        self.order.as_slice()
    }

    /// The stage the vote has reached.
    #[must_use]
    pub const fn stage_now(&self) -> Stage {
        self.stage
    }

    /// Whether the pending question is a voter naming an outcome (or abstaining), as opposed to the planets, trade goods, Genetic
    /// Recombination or tribute questions after it (`VoteWindow::is_outcome_question`).
    #[must_use]
    pub fn is_outcome_question(&self) -> bool {
        matches!(self.stage, Stage::Outcome(index) if usize::from(index) < self.order.len())
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

    /// The votes each outcome has so far (outcomes nobody backed are absent).
    #[must_use]
    pub fn counts(&self) -> SmallList<(Outcome, i64), 16> {
        self.counts_list()
    }

    #[must_use]
    pub const fn is_done(&self) -> bool {
        matches!(self.stage, Stage::Done(_))
    }

    fn with_cx<R>(&self, core: &CoreState, k: &Kernel, f: impl FnOnce(&Cx<'_>) -> R) -> R {
        let env = KernelEnv { core, kernel: k };
        f(&Cx { core, scope: self.facts.scope, blanked: self.facts.blanked, env: &env })
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

    fn offers(&self, core: &CoreState, k: &Kernel, seat: Seat) -> SmallList<super::PlanetOffer, 24> {
        self.with_cx(core, k, |cx| super::planet_offers(cx, seat, self.is_spender(seat)))
    }

    /// `VoteWindow::trade_goods_rate`.
    fn rate(&self, core: &CoreState, seat: Seat, votes: i64) -> i64 {
        if votes > 0 && leaders::has_commander_ability_in(core, seat, Leader::GilaTheSilvertongue) {
            2
        } else {
            i64::from(self.is_spender(seat))
        }
    }

    /// `supply::spendable_goods`: trade goods plus the commodities an open Keleres agent window lets the seat spend.
    fn goods(core: &CoreState, k: &Kernel, seat: Seat) -> i64 {
        core.players().by_seat(seat).map_or(0, |p| i64::from(p.econ.trade_goods.max(0))) + k.hooks.direct.keleres_spendable_commodities(core, seat)
    }

    /// `supply::potential_goods`: [`Self::goods`] plus what a readied Keleres agent could still grant.
    fn potential_goods(core: &CoreState, k: &Kernel, seat: Seat) -> i64 {
        let granted = if k.hooks.direct.keleres_agent_could_grant(core, seat) {
            core.players().by_seat(seat).map_or(0, |p| i64::from(p.econ.commodities.max(0)))
        } else {
            0
        };
        Self::goods(core, k, seat) + granted
    }

    /// `supply::spend_goods`: commodities first (those the agent made spendable), then trade goods.
    fn spend_goods(core: &mut CoreState, k: &Kernel, seat: Seat, amount: i64) {
        let commodities = k.hooks.direct.keleres_spendable_commodities(core, seat);
        let from_commodities = amount.min(commodities);
        if let Some(p) = core.players_mut().by_seat_mut(seat) {
            p.econ.commodities -= i16::try_from(from_commodities).unwrap_or(0);
            p.econ.trade_goods -= i16::try_from(amount - from_commodities).unwrap_or(0);
        }
    }

    /// `mahact::recombination_holder`: another seat with a ready Genetic Recombination.
    fn holder(core: &CoreState, voter: Seat) -> Option<Seat> {
        // The Mahact's own card, or the Nekro's Valefar Assimilator carrying its text.
        core.players().iter().find(|p| p.seat != voter && crate::query::reach::technology_text_ready(core, p.seat, Technology::GeneticRecombination)).map(|p| p.seat)
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
    fn question(&self, core: &CoreState, k: &Kernel) -> Option<Choice> {
        self.with_cx(core, k, |cx| self.question_in(cx, core, k))
    }

    fn question_in(&self, cx: &Cx<'_>, core: &CoreState, k: &Kernel) -> Option<Choice> {
        let counts = self.counts_list();
        match self.stage {
            Stage::Done(_) => None,
            Stage::Outcome(index) => {
                let voter = self.voter(index)?;
                let bound = self.obligation.filter(|(at, _)| *at == index).map(|(_, o)| o);
                Some(super::vote_question(cx, self.agenda, voter, self.choices.as_slice(), counts.as_slice(), bound))
            }
            Stage::Recombine(index) => {
                let voter = self.voter(index)?;
                let holder = Self::holder(core, voter)?;
                Some(super::recombine_question(core, self.agenda, holder, voter, self.choices.as_slice()))
            }
            Stage::Tribute(index) => {
                let voter = self.voter(index)?;
                let (_, outcome) = self.demanded.filter(|(at, _)| *at == index)?;
                super::tribute_question(core, self.agenda, voter, outcome, !self.offers(core, k, voter).is_empty(), Self::can_pay(core, voter))
            }
            Stage::Planets { index, outcome, votes } => {
                let voter = self.voter(index)?;
                let must_cast = votes == 0 && self.obligation.is_some_and(|(at, backed)| at == index && backed == outcome);
                super::planets_question(
                    cx,
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
                super::trade_goods_question(core, self.agenda, voter, outcome, votes, Self::goods(core, k, voter), self.rate(core, voter, votes))
            }
            Stage::Predictive { index, outcome, votes } => {
                Some(super::predictive_question(core, self.agenda, self.voter(index)?, outcome, votes))
            }
            Stage::Tiebreak => {
                let candidates = self.tied(&counts);
                Some(super::tiebreak_question(cx, self.agenda, candidates.as_slice()))
            }
        }
    }

    /// Advance past the stages that ask nothing (`VoteWindow::settle`): Genetic Recombination's offer to its holder, the end of the
    /// ballot (the votes are decided, Predictive Intelligence settled), a planets stage with nothing to exhaust. Stops at a question or
    /// at the end. [`Self::step`] runs it before it names the question; the live arm runs it when the vote opens.
    pub fn settle(&mut self, core: &mut CoreState, k: &Kernel) {
        loop {
            // Genetic Recombination: "before a player casts votes", once per voter, the holder is asked.
            if let Stage::Outcome(index) = self.stage
                && usize::from(index) < self.order.len()
                && self.offered & (1 << index) == 0
            {
                self.offered |= 1 << index;
                if self.voter(index).is_some_and(|v| Self::holder(core, v).is_some()) {
                    self.stage = Stage::Recombine(index);
                    break;
                }
            }
            match self.stage {
                Stage::Outcome(index) if usize::from(index) >= self.order.len() => {
                    self.stage = self.close();
                    self.settle_predictions(core);
                }
                Stage::Planets { index, outcome, votes } => {
                    let empty = self.voter(index).is_none_or(|s| self.offers(core, k, s).is_empty());
                    if empty {
                        self.finish_planets(core, k, index, outcome, votes);
                    } else {
                        break;
                    }
                }
                _ => break,
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
    fn finish_planets(&mut self, core: &CoreState, k: &Kernel, index: u8, outcome: Outcome, votes: i64) {
        let may_spend = self.voter(index).is_some_and(|v| self.rate(core, v, votes) > 0 && Self::potential_goods(core, k, v) > 0);
        if may_spend {
            self.stage = Stage::TradeGoods { index, outcome, votes };
        } else {
            self.bank(core, k, index, outcome, votes);
        }
    }

    /// `VoteWindow::bank`: offer Predictive Intelligence's extra votes when a ready card is held and any vote
    /// was cast, else record them and move on.
    fn bank(&mut self, core: &CoreState, k: &Kernel, index: u8, outcome: Outcome, votes: i64) {
        if votes > 0 && self.voter(index).is_some_and(|v| Self::predictive_ready(core, v)) {
            self.stage = Stage::Predictive { index, outcome, votes };
        } else {
            self.record(core, k, index, outcome, votes);
            self.stage = Stage::Outcome(index + 1);
        }
    }

    /// `VoteWindow::record`: bank one voter's votes; nothing cast, nothing recorded.
    fn record(&mut self, core: &CoreState, k: &Kernel, index: u8, outcome: Outcome, votes: i64) {
        let Some(seat) = self.voter(index) else { return };
        if votes <= 0 {
            // Voted for an outcome but cast nothing: the table is told (display list), nothing is banked.
            self.abstain(seat);
            return;
        }
        let pact = self.facts.pact[usize::from(seat.0) & 7]
            .iter()
            .filter(|other| self.ballot.get(other) == Some(&outcome))
            .count();
        let total = votes
            + k.hooks.general.vote_bonus(core, seat)
            + k.hooks.cards.vote_bonus_with_content(core, k, seat)
            + super::extra_votes(core, seat)
            + BLOOD_PACT_VOTES * i64::try_from(pact).unwrap_or(0);
        self.ballot.insert(seat, outcome);
        *self.counts.get_or_insert_with(outcome, || 0) += total;
        self.casts.push((seat, Some(outcome), total));
    }

    /// A seat that cast nothing (display list only).
    fn abstain(&mut self, seat: Seat) {
        self.casts.push((seat, None, 0));
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
        self.settle(sx.state, sx.kernel);
        match self.question(sx.state, sx.kernel) {
            Some(choice) => Next::Continue(choice.ask),
            None => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.question(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let question = self.question(sx.state, sx.kernel).ok_or(Illegal)?;
        let Pick::Opt(opt) = pick else { return Err(Illegal) };
        if !question.opts.as_slice().contains(&opt) {
            return Err(Illegal);
        }
        match (self.stage, opt) {
            (Stage::Recombine(index), TopOpt::Decline) => self.stage = Stage::Outcome(index),
            (Stage::Recombine(index), TopOpt::Agenda(Opt::Demand(outcome))) => {
                let voter = self.voter(index).ok_or(Illegal)?;
                let holder = Self::holder(sx.state, voter).ok_or(Illegal)?;
                crate::query::reach::exhaust_technology_text(sx.state, holder, Technology::GeneticRecombination);
                let can_vote = !self.offers(sx.state, sx.kernel, voter).is_empty();
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
            (Stage::Outcome(index), TopOpt::Decline) => {
                // 8.14: an abstention casts nothing and is not recorded as a vote (the display list still tells the table).
                if let Some(seat) = self.voter(index) {
                    self.abstain(seat);
                }
                self.stage = Stage::Outcome(index + 1);
            }
            (Stage::Outcome(index), TopOpt::Agenda(Opt::Back(outcome, _))) => {
                self.stage = Stage::Planets { index, outcome, votes: 0 };
            }
            (Stage::Planets { index, outcome, votes }, TopOpt::Decline) => self.finish_planets(sx.state, sx.kernel, index, outcome, votes),
            (Stage::Planets { index, outcome, votes }, TopOpt::Agenda(Opt::Exhaust { planet, face, .. })) => {
                let seat = self.voter(index).ok_or(Illegal)?;
                let cast = self
                    .offers(sx.state, sx.kernel, seat)
                    .as_slice()
                    .iter()
                    .find(|(p, f, _)| *p == planet && *f == face)
                    .map(|(_, _, cast)| *cast)
                    .ok_or(Illegal)?;
                fx::exhaust_planet(sx.state, planet);
                self.stage = Stage::Planets { index, outcome, votes: votes + cast };
            }
            (Stage::TradeGoods { index, outcome, votes }, TopOpt::Decline) => self.bank(sx.state, sx.kernel, index, outcome, votes),
            (Stage::TradeGoods { index, outcome, votes }, TopOpt::Agenda(Opt::Spend(spent))) => {
                let seat = self.voter(index).ok_or(Illegal)?;
                let rate = self.rate(sx.state, seat, votes);
                Self::spend_goods(sx.state, sx.kernel, seat, i64::from(spent));
                self.bank(sx.state, sx.kernel, index, outcome, votes + rate * i64::from(spent));
            }
            (Stage::Predictive { index, outcome, votes }, TopOpt::Decline) => {
                self.record(sx.state, sx.kernel, index, outcome, votes);
                self.stage = Stage::Outcome(index + 1);
            }
            (Stage::Predictive { index, outcome, votes }, TopOpt::Agenda(Opt::CastPredictive)) => {
                let seat = self.voter(index).ok_or(Illegal)?;
                self.predictive.push((seat, outcome));
                self.record(sx.state, sx.kernel, index, outcome, votes + PREDICTIVE_VOTES);
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
