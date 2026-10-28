//! The agenda phase's reads (package B9; LRR 8): what an agenda elects, who may vote and in which
//! order, which planets can be exhausted for votes, who wins. The reads of `vote::{outcomes,
//! voted_agenda, extra_votes, flat_vote_amount, votable_planets, is_law, undisputed_winner,
//! tiebreak_candidates}`, `VoteWindow::new`'s order, `agenda::resolve_agenda_phase`'s reveal order,
//! `laws::{agenda_ready_limit, political_secret_bars}`.
//!
//! What the core cannot see yet is asked of an [`Env`]: the faction modules whose hooks are not
//! registered in the [`Kernel`] table (Nekro's bar, Elder Qanoj, Argent's first vote, the Deepwrought
//! oceans) and the "all laws are blank this turn" flag. The string engine implements the trait, so the
//! answers stay exact.
//!
//! Text order. The string engine sorts planets and secret objectives by id text and keeps vote counts
//! in a text-keyed map. The typed lists below are in the text order of their members (planets through
//! the A1 prefix table) and the tied set of [`Tiebreak::Tied`] in [`text_cmp`] order (the seats are numbered
//! in the text order of their owner ids).

pub mod window;

use core::cmp::Ordering;

use crate::containers::SmallList;
use crate::choice::agenda::{Ask, Face, Frame, Opt, Shown, Subtype, Tally};
use crate::choice::generic::{Content, Rule};
use crate::choice::{Choice, Ctx as ChoiceCtx, Opt as TopOpt, Opts, Origin};
use crate::generated::{Agenda, AgendaElects, AgendaType, Leader, Planet, SecretObjective, StrategyCard, Technology};
use crate::ids::Seat;
use crate::keys::{Prediction, VoteBar, order};
use crate::query::{board, economy, laws, seats};
use crate::query::economy::Spend;
use crate::state::{CoreState, ObjectiveRef, Outcome};
use crate::tables::{Scope, source_bit};

/// `ti4_model::state::RULES_TECH_EFFECTS`.
pub const RULES_TECH_EFFECTS: u32 = 15;
/// `ti4_model::state::RULES_FACTION_BUGFIXES`.
pub const RULES_FACTION_BUGFIXES: u32 = 17;
/// `ti4_model::state::RULES_CRISIS_PACT`.
pub const RULES_CRISIS_PACT: u32 = 31;
/// `ti4_model::state::RULES_CLASSIFIED_LEAKS`.
pub const RULES_CLASSIFIED_LEAKS: u32 = 32;

/// What the agenda rules ask of the factions.
pub trait Env {
    /// `factions::nekro::is_nekro`: "You cannot vote on agendas".
    fn is_nekro(&self, who: Seat) -> bool;
    /// `leaders::elder_qanoj`: game effects cannot stop this seat voting.
    fn elder_qanoj(&self, who: Seat) -> bool;
    /// `factions::hooks_cards::votes_first`.
    fn votes_first(&self, who: Seat) -> bool;
    /// `factions::deepwrought::oceans`: the ocean cards the seat has in play (they vote like planets).
    fn oceans(&self, who: Seat) -> SmallList<Planet, 4>;
    /// `factions::bastion_units::resource_bonus`: the Last Bastion's Helios dock raises the resource value of
    /// its own planet (the hook term `extra` of `economy::value_now`).
    fn resource_bonus(&self, planet: Planet) -> i64;
}

/// What the rules read besides the state.
#[derive(Clone, Copy)]
pub struct Cx<'a> {
    pub core: &'a CoreState,
    /// The game's source scope (which planets exist).
    pub scope: Scope,
    /// `laws::blanked` (Law's Order).
    pub blanked: bool,
    pub env: &'a dyn Env,
}

/// Text order of planet ids.
#[must_use]
pub fn planet_text_cmp(a: Planet, b: Planet) -> Ordering {
    order::cmp_indexed(order::PLANET, a.index(), 0, b.index(), 0)
}

/// Whether an agenda is a law (`vote::is_law`, 8.20): a passed outcome stays in play.
#[must_use]
pub fn is_law(agenda: Agenda) -> bool {
    agenda.data().kind == AgendaType::Law
}

/// `vote::voted_agenda`: the agenda whose outcomes are voted on: the face-down agenda under Covert
/// Legislation, else `agenda` itself.
#[must_use]
pub fn voted_agenda(core: &CoreState, agenda: Agenda) -> Agenda {
    match core.agenda().covert {
        Some(hidden) if agenda == Agenda::CovertLegislation => hidden,
        _ => agenda,
    }
}

/// `vote::outcomes`: what may be voted for on `agenda` (8.8 to 8.11). Empty when the agenda elects
/// something with no legal candidate.
#[must_use]
pub fn outcomes(cx: &Cx<'_>, agenda: Agenda) -> SmallList<Outcome, 16> {
    let core = cx.core;
    let mut out = SmallList::new();
    match agenda.data().elects {
        AgendaElects::ForAgainst | AgendaElects::HiddenNextAgenda => {
            out.push(Outcome::For);
            out.push(Outcome::Against);
        }
        AgendaElects::Player => {
            for p in core.players().iter() {
                out.push(Outcome::Seat(p.seat));
            }
        }
        kind @ (AgendaElects::Planet | AgendaElects::NonHomeNonMecatolPlanet) => {
            // 8.11: only a planet somebody controls may be elected.
            let mut planets: SmallList<Planet, 64> = SmallList::new();
            for (planet, cell) in core.board().planets() {
                if cell.control.is_some() {
                    planets.push(planet);
                }
            }
            planets.as_mut_slice().sort_unstable_by(|a, b| planet_text_cmp(*a, *b));
            for planet in planets.as_slice() {
                if kind == AgendaElects::NonHomeNonMecatolPlanet {
                    let data = planet.data();
                    let mecatol = matches!(planet, Planet::MecatolRex | Planet::MecatolRexTe);
                    let in_scope = cx.scope.0 & source_bit(data.source) != 0;
                    if mecatol || (in_scope && data.homeworld_of.is_some()) {
                        continue;
                    }
                }
                out.push(Outcome::Planet(*planet));
            }
        }
        AgendaElects::Law => {
            for law in laws::in_play(core, cx.blanked).as_slice() {
                out.push(Outcome::Law(*law));
            }
        }
        // Galactic Crisis Pact: any of the game's strategy cards, lowest initiative first (rules
        // version 31; below it nothing was a candidate).
        AgendaElects::StrategyCard if core.meta().rules_at_least(RULES_CRISIS_PACT) => {
            let mut cards: SmallList<(i32, StrategyCard), 12> = SmallList::new();
            for (card, initiative) in core.strategy().initiative.iter() {
                cards.push((*initiative, *card));
            }
            cards.as_mut_slice().sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.index().cmp(&b.1.index())));
            for (_, card) in cards.as_slice() {
                out.push(Outcome::StrategyCard(*card));
            }
        }
        // Classified Document Leaks: a scored secret not already public (rules version 32).
        AgendaElects::ScoredSecretObjective if core.meta().rules_at_least(RULES_CLASSIFIED_LEAKS) => {
            let objectives = core.objectives();
            let mut secrets: SmallList<SecretObjective, 16> = SmallList::new();
            for set in &objectives.scored {
                for s in set.secret.iter() {
                    if !objectives.revealed.as_slice().contains(&ObjectiveRef::Secret(s)) {
                        secrets.push(s);
                    }
                }
            }
            secrets.as_mut_slice().sort_unstable_by_key(|s| s.index());
            let mut last = None;
            for s in secrets.as_slice() {
                if last != Some(*s) {
                    out.push(Outcome::Secret(*s));
                }
                last = Some(*s);
            }
        }
        AgendaElects::StrategyCard | AgendaElects::ScoredSecretObjective => {}
    }
    out
}

/// `vote::extra_votes`: votes a card added to this seat's total for the agenda being voted on.
#[must_use]
pub fn extra_votes(core: &CoreState, seat: Seat) -> i64 {
    core.players()
        .iter()
        .find(|p| p.seat == seat)
        .and_then(|p| p.card_timers.extra_votes_agenda)
        .filter(|e| e.agenda == core.meta().agenda_seq)
        .map_or(0, |e| e.votes)
}

/// `vote::flat_vote_amount`: Representative Government, one vote each and nothing to exhaust.
#[must_use]
pub fn flat_vote_amount(cx: &Cx<'_>) -> Option<i64> {
    laws::flat_votes(cx.core, cx.blanked).then_some(1)
}

/// The influence a planet casts when exhausted (`vote::influence_of`).
#[must_use]
pub fn influence_of(cx: &Cx<'_>, planet: Planet) -> i64 {
    economy::value_now(cx.core, cx.scope, cx.blanked, planet, Spend::Influence, 0)
}

/// `vote::votable_planets`: readied planets the seat controls (the ocean cards after them) that carry
/// influence.
#[must_use]
pub fn votable_planets(cx: &Cx<'_>, seat: Seat) -> SmallList<Planet, 24> {
    let mut out = SmallList::new();
    let own = board::controlled_planets(cx.core, seat);
    let oceans = cx.env.oceans(seat);
    for planet in own.as_slice().iter().map(|(_, p)| *p).chain(oceans.as_slice().iter().copied()) {
        if !board::exhausted(cx.core, planet) && influence_of(cx, planet) > 0 {
            out.push(planet);
        }
    }
    out
}

/// `phase::agenda` reveal order (`agenda::resolve_agenda_phase`): clockwise from the speaker's left, so
/// the speaker is last.
#[must_use]
pub fn reveal_order(core: &CoreState) -> SmallList<Seat, 8> {
    let mut order = seats::speaker_order(core);
    if !order.is_empty() {
        order.as_mut_slice().rotate_left(1);
    }
    order
}

/// `VoteWindow::new`: the order the votes are asked in. Clockwise from the speaker; a seat that made a
/// prediction (Imperial Rider's cost) or is Nekro does not vote (Elder Qanoj always does); Argent-style
/// first voters take the opening seats; the speaker votes last of the rest; Hack Election's seats after
/// them.
#[must_use]
pub fn voting_order(cx: &Cx<'_>) -> SmallList<Seat, 8> {
    let core = cx.core;
    let speaker = core.meta().speaker;
    let mut order = seats::clockwise_from(core, speaker);
    order.retain(|s| {
        (!core.agenda().predictions.contains_key(s) && !cx.env.is_nekro(*s)) || cx.env.elder_qanoj(*s)
    });
    let votes_last = |s: &Seat| {
        core.players()
            .iter()
            .find(|p| p.seat == *s)
            .is_some_and(|p| p.card_timers.hack_votes_last_agenda.get() == Some(core.meta().agenda_seq))
    };
    let mut out = SmallList::new();
    let hackers: SmallList<Seat, 8> = order.as_slice().iter().copied().filter(|s| votes_last(s)).collect_list();
    let rest: SmallList<Seat, 8> = order.as_slice().iter().copied().filter(|s| !votes_last(s)).collect_list();
    for s in rest.as_slice().iter().filter(|s| cx.env.votes_first(**s)) {
        out.push(*s);
    }
    let firsts = out.len();
    for s in rest.as_slice().iter().filter(|s| !cx.env.votes_first(**s)) {
        if *s != speaker {
            out.push(*s);
        }
    }
    // The speaker votes after the others unless a first voter slot took them.
    let speaker_first = out.as_slice()[..firsts].contains(&speaker);
    if rest.as_slice().contains(&speaker) && !speaker_first {
        out.push(speaker);
    }
    for s in hackers.as_slice() {
        out.push(*s);
    }
    out
}

/// A small iterator-to-`SmallList` shim (`SmallList` has no `FromIterator` in every build).
trait CollectList<T: Copy> {
    fn collect_list<const N: usize>(self) -> SmallList<T, N>;
}

impl<T: Copy, I: Iterator<Item = T>> CollectList<T> for I {
    fn collect_list<const N: usize>(self) -> SmallList<T, N> {
        let mut out = SmallList::new();
        for x in self {
            out.push(x);
        }
        out
    }
}

/// `laws::agenda_ready_limit`: Checks and Balances (Against) readies only three planets.
#[must_use]
pub fn ready_limit(core: &CoreState, blanked: bool) -> Option<usize> {
    (laws::active(core, blanked, Agenda::ChecksAndBalances)
        && laws::elected(core, blanked, Agenda::ChecksAndBalances) == Some(Outcome::Against))
    .then_some(3)
}

/// `laws::political_secret_bars`: until the agenda it was played on is resolved, the target cannot play
/// action cards or use faction abilities (rules version 17).
#[must_use]
pub fn political_secret_bars(core: &CoreState, seat: Seat) -> bool {
    core.meta().rules_at_least(RULES_FACTION_BUGFIXES)
        && core.agenda().predictions.get(&seat) == Some(&Prediction::Barred(VoteBar::PoliticalSecret))
}

/// The votes of a ballot: each outcome with the votes it received (outcomes nobody backed are absent).
pub type Counts<'a> = &'a [(Outcome, i64)];

fn best(counts: Counts<'_>) -> i64 {
    counts.iter().map(|(_, c)| *c).max().unwrap_or(0)
}

/// `vote::undisputed_winner` (8.19): most votes wins; `None` when a tie or a silent table needs the
/// speaker.
#[must_use]
pub fn undisputed_winner(counts: Counts<'_>, choices: &[Outcome]) -> Option<Outcome> {
    if choices.is_empty() {
        return None;
    }
    let best = best(counts);
    let mut tied = counts.iter().filter(|(_, c)| *c == best && *c > 0).map(|(o, _)| *o);
    match (tied.next(), tied.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

/// The order of two outcomes of one election by their texts (`BTreeMap<String, _>` order): the ids of seats,
/// planets, strategy cards, laws and secrets are numbered in text order already; only `for`/`against` swap.
#[must_use]
pub fn text_cmp(a: Outcome, b: Outcome) -> Ordering {
    match (a, b) {
        (Outcome::For, Outcome::Against) => Ordering::Greater,
        (Outcome::Against, Outcome::For) => Ordering::Less,
        _ => a.cmp(&b),
    }
}

/// What the speaker chooses between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tiebreak {
    /// The outcomes that tie for the most votes, in text order.
    Tied(SmallList<Outcome, 8>),
    /// Nobody voted: everything on offer, in offer order.
    Silent(SmallList<Outcome, 16>),
}

/// `vote::tiebreak_candidates`.
#[must_use]
pub fn tiebreak_candidates(counts: Counts<'_>, choices: &[Outcome]) -> Tiebreak {
    let best = best(counts);
    let mut tied: SmallList<Outcome, 8> = counts.iter().filter(|(_, c)| *c == best && *c > 0).map(|(o, _)| *o).collect_list();
    if tied.is_empty() {
        Tiebreak::Silent(SmallList::from_slice(choices))
    } else {
        tied.as_mut_slice().sort_unstable_by(|a, b| text_cmp(*a, *b));
        Tiebreak::Tied(tied)
    }
}

// --- the questions of a vote ----------------------------------------------------------------------------

/// How a planet is offered to a voter: the planet, the face it is exhausted for, the votes it casts.
pub type PlanetOffer = (Planet, Face, i64);

/// `VoteWindow::planet_offers`: the planets `seat` may exhaust now. Everyone exhausts a planet for its
/// influence; the Executive Order `spender` may also exhaust it for its resources. Elder Qanoj adds a vote
/// per planet.
#[must_use]
pub fn planet_offers(cx: &Cx<'_>, seat: Seat, spender: bool) -> SmallList<PlanetOffer, 24> {
    let qanoj = i64::from(cx.env.elder_qanoj(seat));
    let mut out = SmallList::new();
    let own = board::controlled_planets(cx.core, seat);
    let oceans = cx.env.oceans(seat);
    for planet in own.as_slice().iter().map(|(_, p)| *p).chain(oceans.as_slice().iter().copied()) {
        if board::exhausted(cx.core, planet) {
            continue;
        }
        let influence = influence_of(cx, planet);
        if influence > 0 {
            out.push((planet, Face::Influence, influence + qanoj));
        }
        if spender {
            let resources =
                economy::value_now(cx.core, cx.scope, cx.blanked, planet, Spend::Resources, cx.env.resource_bonus(planet));
            if resources > 0 {
                out.push((planet, Face::Resources, resources + qanoj));
            }
        }
    }
    out
}

/// The frame of a question asked of `actor` on `agenda`.
#[must_use]
pub fn frame(core: &CoreState, agenda: Agenda, actor: Seat) -> Frame {
    Frame { agenda, covert: core.agenda().covert, reveal: agenda == Agenda::CovertLegislation && actor == core.meta().speaker }
}

fn subtype_of(ask: &Ask) -> Subtype {
    ask.subtype()
}

fn question(actor: Seat, ask: Ask, origin: Origin, opts: Opts) -> Choice {
    let ctx = ChoiceCtx {
        origin,
        subtype: crate::choice::Subtype::Agenda(subtype_of(&ask)),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    };
    Choice { actor, ask: crate::choice::Ask::Agenda(ask), ctx, opts }
}

fn votes32(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

fn count(counts: Counts<'_>, outcome: Outcome) -> i64 {
    counts.iter().find(|(o, _)| *o == outcome).map_or(0, |(_, c)| *c)
}

fn back(cx: &Cx<'_>, outcome: Outcome, votes: i64) -> TopOpt {
    let system = match outcome {
        Outcome::Planet(p) => board::system_of(cx.core, cx.scope, p),
        _ => None,
    };
    TopOpt::Agenda(Opt::Back(outcome, Shown { votes: votes32(votes), system }))
}

/// 8.10, `Stage::Outcome`: `voter` names an outcome among `choices` (or abstains). `bound`: Genetic
/// Recombination forces the voter to back that outcome and not to abstain.
#[must_use]
pub fn vote_question(
    cx: &Cx<'_>,
    agenda: Agenda,
    voter: Seat,
    choices: &[Outcome],
    counts: Counts<'_>,
    bound: Option<Outcome>,
) -> Choice {
    let mut opts = Opts::default();
    for o in choices {
        if bound.is_none_or(|forced| forced == *o) {
            opts.push(back(cx, *o, count(counts, *o)));
        }
    }
    if bound.is_none() {
        opts.push(TopOpt::Decline);
    }
    question(voter, Ask::Vote { frame: frame(cx.core, agenda, voter) }, Origin::Rule(Rule::R8_10), opts)
}

/// 8.11, `Stage::Planets`: which planet to exhaust for `outcome`. `None` when nothing is left to exhaust, or
/// when the vote has more outcomes than the ledger holds (the core does not express it). `must_cast`:
/// Genetic Recombination makes the first planet not optional.
#[must_use]
pub fn planets_question(
    cx: &Cx<'_>,
    agenda: Agenda,
    voter: Seat,
    outcome: Outcome,
    votes: i64,
    spender: bool,
    choices: &[Outcome],
    counts: Counts<'_>,
    must_cast: bool,
) -> Option<Choice> {
    let offers = planet_offers(cx, voter, spender);
    if offers.is_empty() {
        return None;
    }
    let mut tally = Tally::empty();
    for o in choices {
        if !tally.push(*o, votes32(count(counts, *o))) {
            return None;
        }
    }
    let mut opts = Opts::default();
    for (planet, face, cast) in offers.as_slice() {
        let system = board::system_of(cx.core, cx.scope, *planet);
        opts.push(TopOpt::Agenda(Opt::Exhaust { planet: *planet, face: *face, shown: Shown { votes: votes32(*cast), system } }));
    }
    if !must_cast {
        opts.push(TopOpt::Decline);
    }
    let ask = Ask::Planets { frame: frame(cx.core, agenda, voter), outcome, votes: votes32(votes), spender, tally };
    Some(question(voter, ask, Origin::Rule(Rule::R8_11), opts))
}

/// `Stage::TradeGoods`: spend 1 to `goods` trade goods for `rate` votes each. `None` beyond 255 goods.
#[must_use]
pub fn trade_goods_question(
    core: &CoreState,
    agenda: Agenda,
    voter: Seat,
    outcome: Outcome,
    votes: i64,
    goods: i64,
    rate: i64,
) -> Option<Choice> {
    let n = u8::try_from(goods.max(0)).ok()?;
    let mut opts = Opts::default();
    for spent in 1..=n {
        opts.push(TopOpt::Agenda(Opt::Spend(spent)));
    }
    opts.push(TopOpt::Decline);
    let origin = if rate == 2 {
        Origin::Content(Content::Leader(Leader::GilaTheSilvertongue))
    } else {
        Origin::Content(Content::Technology(Technology::ExecutiveOrder))
    };
    let ask = Ask::TradeGoods {
        frame: frame(core, agenda, voter),
        outcome,
        votes: votes32(votes),
        goods: votes32(goods),
        rate: u8::try_from(rate).ok()?,
    };
    Some(question(voter, ask, origin, opts))
}

/// `Stage::Predictive`: cast 3 more votes for `outcome` or not.
#[must_use]
pub fn predictive_question(core: &CoreState, agenda: Agenda, voter: Seat, outcome: Outcome, votes: i64) -> Choice {
    let mut opts = Opts::default();
    opts.push(TopOpt::Agenda(Opt::CastPredictive));
    opts.push(TopOpt::Decline);
    let ask = Ask::Predictive { frame: frame(core, agenda, voter), outcome, votes: votes32(votes) };
    question(voter, ask, Origin::Content(Content::Technology(Technology::PredictiveIntelligence)), opts)
}

/// `Stage::Recombine`: the holder names the outcome `voter` must back.
#[must_use]
pub fn recombine_question(core: &CoreState, agenda: Agenda, holder: Seat, voter: Seat, choices: &[Outcome]) -> Choice {
    let mut opts = Opts::default();
    for o in choices {
        opts.push(TopOpt::Agenda(Opt::Demand(*o)));
    }
    opts.push(TopOpt::Decline);
    let ask = Ask::Recombine { frame: frame(core, agenda, holder), voter };
    question(holder, ask, Origin::Content(Content::Technology(Technology::GeneticRecombination)), opts)
}

/// `Stage::Tribute`: the voter backs `outcome` (`can_vote`: some planet is left to exhaust) or returns a
/// fleet token (`can_pay`). `None` when neither.
#[must_use]
pub fn tribute_question(core: &CoreState, agenda: Agenda, voter: Seat, outcome: Outcome, can_vote: bool, can_pay: bool) -> Option<Choice> {
    let mut opts = Opts::default();
    if can_vote {
        opts.push(TopOpt::Agenda(Opt::TributeVote));
    }
    if can_pay {
        opts.push(TopOpt::Agenda(Opt::TributeToken));
    }
    if opts.is_empty() {
        return None;
    }
    let ask = Ask::Tribute { frame: frame(core, agenda, voter), outcome };
    Some(question(voter, ask, Origin::Content(Content::Technology(Technology::GeneticRecombination)), opts))
}

/// 8.16, `Stage::Tiebreak`: the speaker picks among `candidates` (tied, or everything on offer).
#[must_use]
pub fn tiebreak_question(cx: &Cx<'_>, agenda: Agenda, candidates: &[Outcome]) -> Choice {
    let speaker = cx.core.meta().speaker;
    let mut opts = Opts::default();
    for o in candidates {
        opts.push(back(cx, *o, 0));
    }
    question(speaker, Ask::Tiebreak { frame: frame(cx.core, agenda, speaker) }, Origin::Rule(Rule::R8_16), opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoEnv;
    impl Env for NoEnv {
        fn is_nekro(&self, _: Seat) -> bool {
            false
        }
        fn elder_qanoj(&self, _: Seat) -> bool {
            false
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

    #[test]
    fn ties_and_silence() {
        let choices = [Outcome::For, Outcome::Against];
        assert_eq!(undisputed_winner(&[(Outcome::For, 3)], &choices), Some(Outcome::For));
        assert_eq!(undisputed_winner(&[(Outcome::For, 3), (Outcome::Against, 3)], &choices), None);
        assert_eq!(undisputed_winner(&[], &choices), None);
        assert_eq!(undisputed_winner(&[(Outcome::For, 3)], &[]), None);
        assert!(matches!(tiebreak_candidates(&[], &choices), Tiebreak::Silent(l) if l.len() == 2));
        assert!(matches!(tiebreak_candidates(&[(Outcome::For, 2), (Outcome::Against, 2)], &choices), Tiebreak::Tied(l) if l.len() == 2));
    }

    #[test]
    fn plain_agendas_offer_for_and_against() {
        let core = CoreState::all_loaded();
        let cx = Cx { core: &core, scope: Scope(!0), blanked: false, env: &NoEnv };
        let o = outcomes(&cx, Agenda::ChecksAndBalances);
        assert_eq!(o.as_slice(), [Outcome::For, Outcome::Against]);
        assert!(flat_vote_amount(&cx).is_none());
    }
}
