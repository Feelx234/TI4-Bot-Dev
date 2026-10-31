//! The strategy draft (`Game::legal_options` in the strategy phase, `take_timed_strategy_card` and the end of the draft in
//! `apply_choice`): "choose a strategy card", asked of the picker `rules::strategy::next_picker` names; the pick takes the card off the
//! mat (the trade goods on it go to the picker) inside the typed `STRATEGY_CARD_CHOSEN` event, and the plain event follows.
//!
//! At the end of the draft the string offers Imperial Arbiter's swap and the Quantum Datahub Node's: the live job stops with a counted
//! gap where either could be offered ([`TurnGap::ImperialArbiter`], [`TurnGap::QuantumDatahub`]).
//!
//! The string never journals the last card (`table.auto_resolve`: "only one strategy card left"); that is the table's, not the core's.

use super::inline::{gap, halted, player_payload, sync_clock, Flow};
use crate::choice::strategy::{rule_ctx, Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::generic::Rule;
use crate::choice::{Ask, Choice, Opt, Opts};
use crate::generated::{Agenda, StrategyCard, Technology};
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::fx;
use crate::query::{laws, reach};
use crate::rules::{strategy, strategy_flow};
use crate::state::{CoreState, EventKind, Key, Outcome, PlainKind, PlainRec, Val};

/// The question of the draft, or `None` when the draft is done.
#[must_use]
pub fn question(core: &CoreState) -> Option<Choice> {
    let draft = strategy::draft(core)?;
    Some(Choice {
        actor: draft.picker,
        ask: Ask::Strategy(SAsk::Draft),
        ctx: rule_ctx(Rule::R26, SSub::Draft),
        opts: draft.cards.as_slice().iter().map(|c| Opt::Strategy(SOpt::Card(*c))).collect::<Opts>(),
    })
}

/// Whether Imperial Arbiter's owner and some other seat both hold a strategy card (`Game::imperial_arbiter`'s gate).
fn arbiter_could_swap(core: &CoreState) -> bool {
    let Some(Outcome::Seat(owner)) = laws::elected(core, reach::blanked(core), Agenda::ImperialArbiter) else { return false };
    let mine = core.players().by_seat(owner).is_some_and(|p| !p.strat.is_empty());
    mine && core.players().iter().any(|p| p.seat != owner && !p.strat.is_empty())
}

/// `faction_techs::offer_quantum_datahub`'s gate: a holder with a token, three trade goods and a card, and another seat holding a card.
fn datahub_could_swap(core: &CoreState) -> bool {
    core.players().iter().any(|p| {
        reach::technology_text(core, p.seat, Technology::QuantumDatahubNode)
            && p.econ.strategic_tokens > 0
            && p.econ.trade_goods >= QUANTUM_DATAHUB_GOODS
            && !p.strat.is_empty()
            && core.players().iter().any(|o| o.seat != p.seat && !o.strat.is_empty())
    })
}

/// `faction_techs::QUANTUM_DATAHUB_GOODS`.
const QUANTUM_DATAHUB_GOODS: i16 = 3;

/// The job: `card` is taken off the mat by the picker.
pub fn run_draft(sx: &mut Sx<'_>, card: StrategyCard) -> Flow {
    let Some(draft) = strategy::draft(sx.state) else {
        gap(sx, TurnGap::ReplayDiverged);
        return Flow::Halt;
    };
    if !draft.cards.as_slice().contains(&card) || sx.state.players().by_seat(draft.picker).is_none() {
        gap(sx, TurnGap::ReplayDiverged);
        return Flow::Halt;
    }
    let picker = draft.picker;
    // The card's payload names the picker and the card; the slot is the handoff Public Disgrace reads back in the After window.
    sync_clock(sx);
    sx.state.transient_mut().misc.last_strategy_choice = Some((picker, card));
    let goods = sx.state.strategy().goods.get(&card).copied().unwrap_or(0);
    let payload = player_payload(picker).with(Key::Card, Val::Strategy(card)).with(Key::Goods, Val::Int(i64::from(goods)));
    let source = sx.table.codes.turn.strategy_card;
    let mut taken = false;
    let done = sx.emit_with(fx::event(EventKind::StrategyCardChosen, payload), |_, host, _| {
        taken = strategy_flow::take_card(host.state, picker, card).is_some();
        if taken && goods > 0 {
            fx::stage(host.state, fx::trade_goods_gained(picker, goods, source));
        }
    });
    if halted(sx) {
        return Flow::Halt;
    }
    let Ok(done) = done else {
        gap(sx, TurnGap::EmissionRefused);
        return Flow::Halt;
    };
    if done.cancelled || !taken {
        gap(sx, TurnGap::EventCancelled);
        return Flow::Halt;
    }
    sx.plain(PlainRec::bare(PlainKind::StrategyCardChosen));
    // Imperial Arbiter and the Quantum Datahub Node are offered once the last card is gone: "at the end of the strategy phase".
    if strategy::draft(sx.state).is_none() {
        if arbiter_could_swap(sx.state) {
            gap(sx, TurnGap::ImperialArbiter);
            return Flow::Halt;
        }
        if datahub_could_swap(sx.state) {
            gap(sx, TurnGap::QuantumDatahub);
            return Flow::Halt;
        }
    }
    Flow::Done
}
