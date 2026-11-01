//! The two swaps offered once the last strategy card of the draft is gone: Imperial Arbiter (`Game::imperial_arbiter`) and the Quantum
//! Datahub Node (`faction_techs::offer_quantum_datahub`). Both are part of the draft job ([`super::draft::run_draft`]): the questions go
//! to the tape like every other question of a job, the swap is written when the last answer is in.
//!
//! The Arbiter's owner may discard the law (the generic offer, [`OfferDiscard`]) to give one of their cards for one of another seat's:
//! the string moves the cards by hand (the taken card goes on the END of the holding, an exhausted card keeps its place), the Datahub
//! swap goes through `strategy_write::swap` (the holdings stay in initiative order and an exhaustion moves with the card).

use super::inline::{gap, halted, Flow};
use super::inline::run;
use crate::choice::generic::Content;
use crate::choice::strategy::{Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::technology::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Agenda, StrategyCard, Technology};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::TurnGap;
use crate::query::{laws, reach};
use crate::rules::agenda::offer_discard::OfferDiscard;
use crate::rules::strategy_flow;
use crate::state::{PlainArg, PlainKind, PlainRec};

/// `faction_techs::QUANTUM_DATAHUB_GOODS`: what the Datahub's holder hands the partner.
pub const QUANTUM_DATAHUB_GOODS: i32 = 3;

fn context(origin: Origin, subtype: Subtype) -> Ctx {
    Ctx { origin, subtype, optional: false, target: None, outstanding: SmallList::new(), space_battle: false, invasion_seq: None }
}

/// The cards a seat holds, in the order it holds them.
fn held(sx: &Sx<'_>, seat: Seat) -> Vec<StrategyCard> {
    sx.state.players().by_seat(seat).map(|p| p.strat.as_slice().to_vec()).unwrap_or_default()
}

/// The cards of every other seat with their holders, in table order (the string's `theirs`).
fn others(sx: &Sx<'_>, seat: Seat) -> Vec<(Seat, StrategyCard)> {
    sx.state.players().iter().filter(|p| p.seat != seat).flat_map(|p| p.strat.as_slice().iter().map(move |c| (p.seat, *c))).collect()
}

/// Imperial Arbiter, "at the end of the strategy phase": the owner may discard it to swap 1 of their strategy cards with 1 of another
/// player's. Nothing is asked unless the owner and some other seat both hold a card.
pub fn arbiter(sx: &mut Sx<'_>) -> Flow {
    let Some(crate::state::Outcome::Seat(owner)) = laws::elected(sx.state, reach::blanked(sx.state), Agenda::ImperialArbiter) else { return Flow::Done };
    let mine = held(sx, owner);
    let theirs = others(sx, owner);
    if mine.is_empty() || theirs.is_empty() {
        return Flow::Done;
    }
    let codes = sx.table.codes.turn;
    let mut offer = OfferDiscard::new(Agenda::ImperialArbiter, owner, codes.arbiter_prompt, codes.arbiter);
    if run(sx, &mut offer) == Flow::Halt {
        return Flow::Halt;
    }
    if !offer.taken() {
        return Flow::Done;
    }
    let origin = Origin::Content(Content::Other(codes.arbiter));
    let give = Choice {
        actor: owner,
        ask: Ask::Strategy(SAsk::ArbiterGive),
        ctx: context(origin, Subtype::Strategy(SSub::ArbiterGive)),
        opts: mine.iter().map(|c| Opt::Strategy(SOpt::Card(*c))).collect::<Opts>(),
    };
    let given = sx.ask(&give);
    if halted(sx) {
        return Flow::Halt;
    }
    let Pick::Opt(Opt::Strategy(SOpt::Card(given))) = given else { return diverged(sx) };
    if !mine.contains(&given) {
        return diverged(sx);
    }
    let take = Choice {
        actor: owner,
        ask: Ask::Strategy(SAsk::ArbiterTake),
        ctx: context(origin, Subtype::Strategy(SSub::ArbiterTake)),
        opts: theirs.iter().map(|(holder, c)| Opt::Strategy(SOpt::Take(*holder, *c))).collect::<Opts>(),
    };
    let taken = sx.ask(&take);
    if halted(sx) {
        return Flow::Halt;
    }
    let Pick::Opt(Opt::Strategy(SOpt::Take(holder, card))) = taken else { return diverged(sx) };
    if !theirs.contains(&(holder, card)) {
        return diverged(sx);
    }
    // The string moves the cards by hand: the card taken goes on the end of the owner's holding, the card given on the end of the other's.
    if let Some(p) = sx.state.players_mut().by_seat_mut(owner) {
        let mut cards: Vec<StrategyCard> = p.strat.as_slice().iter().copied().filter(|c| *c != given).collect();
        cards.push(card);
        p.strat = cards.into_iter().collect();
    }
    if let Some(p) = sx.state.players_mut().by_seat_mut(holder) {
        let mut cards: Vec<StrategyCard> = p.strat.as_slice().iter().copied().filter(|c| *c != card).collect();
        cards.push(given);
        p.strat = cards.into_iter().collect();
    }
    sx.plain(PlainRec::two(PlainKind::StrategyCardsSwapped, PlainArg::Seat(owner), PlainArg::Seat(holder)));
    Flow::Done
}

fn diverged(sx: &Sx<'_>) -> Flow {
    gap(sx, TurnGap::ReplayDiverged);
    Flow::Halt
}

/// The seat that would be offered the Quantum Datahub Node's swap: the first with the technology's text, a strategy token, three trade goods
/// and a card (`offer_quantum_datahub`'s `find`).
#[must_use]
pub fn datahub_holder(core: &crate::state::CoreState) -> Option<Seat> {
    core.players()
        .iter()
        .find(|p| reach::technology_text(core, p.seat, Technology::QuantumDatahubNode) && p.econ.strategic_tokens > 0 && i32::from(p.econ.trade_goods) >= QUANTUM_DATAHUB_GOODS && !p.strat.is_empty())
        .map(|p| p.seat)
}

/// The Quantum Datahub Node: "spend a strategy token and give another player 3 of your trade goods; if you do, give 1 of your strategy
/// cards to that player and take 1 of theirs". One question: every (partner, card given, card taken), or a decline.
pub fn datahub(sx: &mut Sx<'_>) -> Flow {
    let Some(holder) = datahub_holder(sx.state) else { return Flow::Done };
    let mine = held(sx, holder);
    let mut swaps: Vec<(Seat, StrategyCard, StrategyCard)> = Vec::new();
    for partner in sx.state.players().iter().filter(|p| p.seat != holder) {
        for given in &mine {
            for taken in partner.strat.as_slice() {
                swaps.push((partner.seat, *given, *taken));
            }
        }
    }
    if swaps.is_empty() {
        return Flow::Done;
    }
    let mut opts: Opts = swaps.iter().map(|(partner, give, take)| Opt::Technology(TOpt::Swap { partner: *partner, give: *give, take: *take })).collect();
    opts.push(Opt::Decline);
    let question = Choice {
        actor: holder,
        ask: Ask::Technology(TAsk::QuantumDatahub),
        ctx: context(Origin::Content(Content::Technology(Technology::QuantumDatahubNode)), Subtype::Technology(TSub::QuantumDatahubSwap)),
        opts,
    };
    let pick = sx.ask(&question);
    if halted(sx) {
        return Flow::Halt;
    }
    let (partner, given, taken) = match pick {
        Pick::Opt(Opt::Decline) => return Flow::Done,
        Pick::Opt(Opt::Technology(TOpt::Swap { partner, give, take })) if swaps.contains(&(partner, give, take)) => (partner, give, take),
        _ => return diverged(sx),
    };
    let reason = sx.table.codes.turn.quantum_datahub;
    if !fx::spend_strategy_token_staged(sx.state, holder, reason) {
        return Flow::Done;
    }
    fx::spend_trade_goods(sx.state, holder, QUANTUM_DATAHUB_GOODS);
    fx::gain_trade_goods(sx.state, partner, QUANTUM_DATAHUB_GOODS);
    fx::stage_gain(sx.state, partner, QUANTUM_DATAHUB_GOODS, reason);
    strategy_flow::swap(sx.state, holder, given, taken);
    strategy_flow::swap(sx.state, partner, taken, given);
    sx.plain(PlainRec::two(PlainKind::QuantumDatahubSwap, PlainArg::Seat(holder), PlainArg::Seat(partner)));
    Flow::Done
}
