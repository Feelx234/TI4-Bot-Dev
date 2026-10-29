//! The typed copies of the windows D1a's remainder types (package D1a; docs/todo/D1b.md W5): the Winnu hero's follower
//! selection, the status phase's token gain with its report, the vote of one agenda, and an open transaction. A child of
//! `d1_pending`, which calls these from `observe` (string field -> typed window) and `roundtrip` (typed window -> string
//! field, compared with the field it was read from). The windows follow the string (they have no core transition yet).
//!
//! Counted bails (`Err` of the `*_of` functions): a seat, agenda, outcome, system, planet, strategy card or objective the
//! tables do not know, a deal the core has no shape for (a free-form deal, a Pride of Kenara planet card), a finished
//! transaction window.

use ti4_engine_core::choice::strategy::Pool;
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Agenda, Faction, Planet, PublicObjective, SecretObjective, StrategyCard, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::Outcome;
use ti4_engine_core::state::{LeaderFollowers, ObjectiveRef, StatusReport, Tokens, Trade, Vote, VoteStage, Voting, Window};
use ti4_model::id::{ObjectiveId, PlanetId, PlayerId, StrategyCardId, SystemId};
use ti4_model::state::{AgendaCast, TokenPool};

use super::super::Game;
use crate::choice::Choice;
use crate::core_sync::Ctx;
use crate::generated::IdStr;
use crate::render::Render;
use crate::status::StatusPhaseReport;
use crate::tokens::{TokenGain, TokenPlacement};
use crate::vote::{Ballot, Stage, VoteParts, VoteWindow};

pub(super) type Res<T> = Result<T, &'static str>;

fn seat(r: &Render, p: &PlayerId) -> Res<Seat> {
    r.seat(p.as_str()).ok_or("a seat the table does not know")
}

fn seats<const N: usize>(r: &Render, ps: &[PlayerId]) -> Res<SmallList<Seat, N>> {
    ps.iter().map(|p| seat(r, p)).collect()
}

fn players(r: &Render, ss: &[Seat]) -> Vec<PlayerId> {
    ss.iter().map(|s| r.player(*s)).collect()
}

fn count(n: usize) -> Res<u32> {
    u32::try_from(n).map_err(|_| "a count above u32")
}

fn index(n: usize) -> Res<u8> {
    u8::try_from(n).map_err(|_| "a voter index above 255")
}

fn pool_of(p: TokenPool) -> Pool {
    match p {
        TokenPool::Tactic => Pool::Tactic,
        TokenPool::Fleet => Pool::Fleet,
        TokenPool::Strategic => Pool::Strategic,
    }
}

fn pool_back(p: Pool) -> TokenPool {
    match p {
        Pool::Tactic => TokenPool::Tactic,
        Pool::Fleet => TokenPool::Fleet,
        Pool::Strategic => TokenPool::Strategic,
    }
}

fn objective_of(id: &ObjectiveId) -> Res<ObjectiveRef> {
    PublicObjective::from_id(id.as_str())
        .map(ObjectiveRef::Public)
        .or_else(|| SecretObjective::from_id(id.as_str()).map(ObjectiveRef::Secret))
        .ok_or("an objective the tables do not know")
}

fn objective_back(o: ObjectiveRef) -> ObjectiveId {
    ObjectiveId::new(match o {
        ObjectiveRef::Public(p) => p.as_str(),
        ObjectiveRef::Secret(s) => s.as_str(),
    })
}

// --- leader followers ------------------------------------------------------------------------------------------------

/// The follower selection: the seats still to be asked (read off the pending questions' `yes|<seat>` options) and the
/// seats that may follow so far.
pub(super) fn leader_followers_of(g: &Game<'_>, r: &Render) -> Res<Option<Window>> {
    let Some((choices, followers)) = &g.leader_followers else { return Ok(None) };
    let mut asks = SmallList::new();
    for c in choices {
        let who = c
            .options
            .first()
            .and_then(|o| o.id.strip_prefix("yes|"))
            .ok_or("a follower question the tables do not know")?;
        asks.push(seat(r, &PlayerId::new(who))?);
    }
    Ok(Some(Window::LeaderFollowers(LeaderFollowers { asks, followers: seats(r, followers)? })))
}

/// The string field a typed follower selection stands for: the questions are the tail of the ones the position produces
/// (`factions::leader_strategy_followers`), as long as the selection has seats left to ask. `None` without a hero in play.
pub(super) fn leader_followers_back(g: &Game<'_>, r: &Render, w: &LeaderFollowers) -> Option<(Vec<Choice>, Vec<PlayerId>)> {
    let (player, leader, card) = g.leader_strategy.as_ref()?;
    let all = crate::factions::leader_strategy_followers(&g.state, player, leader, card);
    let skip = all.len().checked_sub(w.asks.len())?;
    Some((all[skip..].to_vec(), players(r, w.followers.as_slice())))
}

// --- tokens ----------------------------------------------------------------------------------------------------------

fn report_of(rep: &StatusPhaseReport, r: &Render) -> Res<StatusReport> {
    Ok(StatusReport {
        action_card_draws: seats(r, &rep.action_card_draws)?,
        revealed_objective: rep.revealed_objective.as_ref().map(objective_of).transpose()?,
        action_cards_drawn: rep.action_cards_drawn.iter().map(|(p, n)| Ok((seat(r, p)?, count(*n)?))).collect::<Res<_>>()?,
        returned_command_tokens: rep
            .returned_command_tokens
            .iter()
            .map(|(s, p)| Ok((System::from_id(s.as_str()).ok_or("a system the tables do not know")?, seat(r, p)?)))
            .collect::<Res<_>>()?,
        crown_of_emphidia: rep.crown_of_emphidia.as_ref().map(|p| seat(r, p)).transpose()?,
        readied_planets: rep
            .readied_planets
            .iter()
            .map(|p| Planet::from_id(p.as_str()).ok_or("a planet the tables do not know"))
            .collect::<Res<_>>()?,
        readied_leaders: rep.readied_leaders.iter().map(|(p, n)| Ok((seat(r, p)?, count(*n)?))).collect::<Res<_>>()?,
        returned_strategy_cards: rep
            .returned_strategy_cards
            .iter()
            .map(|(p, c)| Ok((seat(r, p)?, StrategyCard::from_id(c.as_str()).ok_or("a strategy card the tables do not know")?)))
            .collect::<Res<_>>()?,
        initiative_order: seats(r, &rep.initiative_order)?,
        repaired_units: count(rep.repaired_units)?,
        game_ended: rep.game_ended,
    })
}

fn report_back(rep: &StatusReport, r: &Render) -> StatusPhaseReport {
    StatusPhaseReport {
        action_card_draws: players(r, rep.action_card_draws.as_slice()),
        revealed_objective: rep.revealed_objective.map(objective_back),
        action_cards_drawn: rep.action_cards_drawn.as_slice().iter().map(|(s, n)| (r.player(*s), *n as usize)).collect(),
        returned_command_tokens: rep.returned_command_tokens.as_slice().iter().map(|(s, p)| (SystemId::new(s.as_str()), r.player(*p))).collect(),
        crown_of_emphidia: rep.crown_of_emphidia.map(|s| r.player(s)),
        readied_planets: rep.readied_planets.as_slice().iter().map(|p| PlanetId::new(p.as_str())).collect(),
        readied_leaders: rep.readied_leaders.as_slice().iter().map(|(s, n)| (r.player(*s), *n as usize)).collect(),
        returned_strategy_cards: rep.returned_strategy_cards.as_slice().iter().map(|(s, c)| (r.player(*s), StrategyCardId::new(c.as_str()))).collect(),
        initiative_order: players(r, rep.initiative_order.as_slice()),
        repaired_units: rep.repaired_units as usize,
        game_ended: rep.game_ended,
    }
}

pub(super) fn tokens_of(g: &Game<'_>, r: &Render) -> Res<Option<Window>> {
    let Some((gain, report)) = &g.tokens else { return Ok(None) };
    let (pending, placed) = gain.parts();
    Ok(Some(Window::Tokens(Box::new(Tokens {
        pending: seats(r, &pending)?,
        placed: placed.iter().map(|t| Ok((seat(r, &t.player)?, pool_of(t.pool)))).collect::<Res<_>>()?,
        report: report_of(report, r)?,
    }))))
}

pub(super) fn tokens_back(t: &Tokens, r: &Render) -> (TokenGain, Box<StatusPhaseReport>) {
    let placed = t.placed.as_slice().iter().map(|(s, p)| TokenPlacement { player: r.player(*s), pool: pool_back(*p) }).collect();
    (TokenGain::from_parts(players(r, t.pending.as_slice()), placed), Box::new(report_back(&t.report, r)))
}

// --- voting ----------------------------------------------------------------------------------------------------------

/// The typed vote and the agendas still to come.
pub(super) fn voting_of(g: &Game<'_>, r: &Render, cx: &Ctx) -> Res<Option<Window>> {
    let Some((window, queue)) = &g.voting else { return Ok(None) };
    let p = window.parts();
    let agenda = Agenda::from_id(&p.alias).ok_or("an agenda the tables do not know")?;
    // Covert Legislation is voted on the outcomes of the face-down agenda.
    let voted = match g.state.covert_agenda.as_deref() {
        Some(hidden) if p.alias == "covert" => Agenda::from_id(hidden),
        _ => Some(agenda),
    };
    let elects = voted.map(|a| a.data().elects);
    let outcome = |t: &str| cx.keys.parse_outcome(t, elects).ok_or("an outcome the tables do not know");
    let stage = match &p.stage {
        Stage::Outcome(i) => VoteStage::Outcome(index(*i)?),
        Stage::Recombine(i) => VoteStage::Recombine(index(*i)?),
        Stage::Tribute(i) => VoteStage::Tribute(index(*i)?),
        Stage::Planets { index: i, outcome: o, votes } => VoteStage::Planets { index: index(*i)?, outcome: outcome(o)?, votes: *votes },
        Stage::TradeGoods { index: i, outcome: o, votes } => VoteStage::TradeGoods { index: index(*i)?, outcome: outcome(o)?, votes: *votes },
        Stage::Predictive { index: i, outcome: o, votes } => VoteStage::Predictive { index: index(*i)?, outcome: outcome(o)?, votes: *votes },
        Stage::Tiebreak => VoteStage::Tiebreak,
        Stage::Done(w) => VoteStage::Done(w.as_deref().map(outcome).transpose()?),
    };
    let held = |o: &Option<(usize, String)>| -> Res<Option<(u8, Outcome)>> { o.as_ref().map(|(i, t)| Ok((index(*i)?, outcome(t)?))).transpose() };
    let vote = Vote {
        agenda,
        choices: p.choices.iter().map(|c| outcome(c)).collect::<Res<_>>()?,
        order: seats(r, &p.order)?,
        stage,
        ballot: p.ballot.votes.iter().map(|(who, t)| Ok((seat(r, who)?, outcome(t)?))).collect::<Res<_>>()?,
        counts: p.ballot.counts.iter().map(|(t, n)| Ok((outcome(t)?, *n))).collect::<Res<_>>()?,
        casts: p
            .casts
            .iter()
            .map(|c| Ok((seat(r, &c.player)?, c.outcome.as_deref().map(outcome).transpose()?, c.votes)))
            .collect::<Res<_>>()?,
        spender: p.spender.as_ref().map(|s| seat(r, s)).transpose()?,
        recombination_offered: p.recombination_offered.iter().map(|i| index(*i)).collect::<Res<_>>()?,
        obligation: held(&p.obligation)?,
        demanded: held(&p.demanded)?,
        predictive: p.predictive.iter().map(|(who, t)| Ok((seat(r, who)?, outcome(t)?))).collect::<Res<_>>()?,
    };
    let queue = queue.iter().map(|a| Agenda::from_id(a).ok_or("an agenda the tables do not know")).collect::<Res<_>>()?;
    Ok(Some(Window::Voting(Box::new(Voting { vote, queue }))))
}

/// The string field a typed vote stands for; `None` for an outcome the key codec cannot print.
pub(super) fn voting_back(v: &Voting, r: &Render, cx: &Ctx) -> Option<(Box<VoteWindow>, Vec<String>)> {
    let text = |o: Outcome| cx.keys.render_outcome(o);
    let queue = v.queue.as_slice().iter().map(|a| a.as_str().to_owned()).collect();
    let v = &v.vote;
    let stage = match v.stage {
        VoteStage::Outcome(i) => Stage::Outcome(usize::from(i)),
        VoteStage::Recombine(i) => Stage::Recombine(usize::from(i)),
        VoteStage::Tribute(i) => Stage::Tribute(usize::from(i)),
        VoteStage::Planets { index, outcome, votes } => Stage::Planets { index: usize::from(index), outcome: text(outcome)?, votes },
        VoteStage::TradeGoods { index, outcome, votes } => Stage::TradeGoods { index: usize::from(index), outcome: text(outcome)?, votes },
        VoteStage::Predictive { index, outcome, votes } => Stage::Predictive { index: usize::from(index), outcome: text(outcome)?, votes },
        VoteStage::Tiebreak => Stage::Tiebreak,
        VoteStage::Done(w) => Stage::Done(match w {
            Some(o) => Some(text(o)?),
            None => None,
        }),
    };
    let held = |o: Option<(u8, Outcome)>| -> Option<Option<(usize, String)>> {
        match o {
            Some((i, out)) => Some(Some((usize::from(i), text(out)?))),
            None => Some(None),
        }
    };
    let mut ballot = Ballot::default();
    for (who, o) in v.ballot.as_slice() {
        ballot.votes.insert(r.player(*who), text(*o)?);
    }
    for (o, n) in v.counts.as_slice() {
        ballot.counts.insert(text(*o)?, *n);
    }
    let mut casts = Vec::new();
    for (who, o, votes) in v.casts.as_slice() {
        casts.push(AgendaCast { player: r.player(*who), outcome: match o { Some(o) => Some(text(*o)?), None => None }, votes: *votes });
    }
    let mut predictive = Vec::new();
    for (who, o) in v.predictive.as_slice() {
        predictive.push((r.player(*who), text(*o)?));
    }
    let parts = VoteParts {
        alias: v.agenda.as_str().to_owned(),
        choices: v.choices.as_slice().iter().map(|o| text(*o)).collect::<Option<_>>()?,
        order: players(r, v.order.as_slice()),
        stage,
        ballot,
        casts,
        spender: v.spender.map(|s| r.player(s)),
        recombination_offered: v.recombination_offered.as_slice().iter().map(|i| usize::from(*i)).collect(),
        obligation: held(v.obligation)?,
        demanded: held(v.demanded)?,
        predictive,
    };
    Some((Box::new(VoteWindow::from_parts(parts)), queue))
}

// --- trade -----------------------------------------------------------------------------------------------------------

pub(super) fn trade_of(g: &Game<'_>, r: &Render) -> Res<Option<Window>> {
    let Some(window) = &g.trade else { return Ok(None) };
    let p = window.parts();
    if p.done {
        return Err("a finished transaction window");
    }
    let deal = match &p.answering {
        Some((_, id)) => {
            // The Support swap names the two factions; its option id does not.
            let faction = |who: &PlayerId| crate::core_adapter::typed::<Faction>(g.state.player(who)?.faction.as_str());
            let swap = faction(&p.proposer).zip(faction(&p.partner));
            Some(crate::render::trade::shape_of(id, swap).ok_or("a deal the core has no shape for")?)
        }
        None => None,
    };
    Ok(Some(Window::Trade(Trade { proposer: seat(r, &p.proposer)?, partner: seat(r, &p.partner)?, rounds_left: p.rounds_left, deal })))
}

/// What a typed transaction stands for, as far as the string window can be compared without its terms (those are a function
/// of the shape and the position): the two chairs, the proposals left and the option id of the deal on the table.
pub(super) fn trade_key(t: &Trade, r: &Render) -> (PlayerId, PlayerId, u8, Option<String>) {
    (r.player(t.proposer), r.player(t.partner), t.rounds_left, t.deal.map(|s| crate::render::trade::deal_option(s).id))
}

/// The same key read off the string window.
pub(super) fn trade_key_of(g: &Game<'_>) -> Option<(PlayerId, PlayerId, u8, Option<String>)> {
    let p = g.trade.as_ref()?.parts();
    Some((p.proposer, p.partner, p.rounds_left, p.answering.map(|(_, id)| id)))
}
