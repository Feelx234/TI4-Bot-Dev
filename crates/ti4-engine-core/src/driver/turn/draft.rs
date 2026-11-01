//! The strategy draft (`Game::legal_options` in the strategy phase, `take_timed_strategy_card` and the end of the draft in
//! `apply_choice`): "choose a strategy card", asked of the picker `rules::strategy::next_picker` names; the pick takes the card off the
//! mat (the trade goods on it go to the picker) inside the typed `STRATEGY_CARD_CHOSEN` event, and the plain event follows.
//!
//! At the end of the draft the string offers Imperial Arbiter's swap and the Quantum Datahub Node's ([`super::swaps`]), inside the same job.
//!
//! The string never journals the last card (`table.auto_resolve`: "only one strategy card left"); that is the table's, not the core's.

use super::inline::{gap, halted, player_payload, sync_clock, Flow};
use super::swaps;
use crate::choice::strategy::{rule_ctx, Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::generic::Rule;
use crate::choice::{Ask, Choice, Opt, Opts};
use crate::generated::StrategyCard;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::fx;
use crate::rules::{strategy, strategy_flow};
use crate::state::{CoreState, EventKind, Key, PlainKind, PlainRec, Val};

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
        if swaps::arbiter(sx) == Flow::Halt || swaps::datahub(sx) == Flow::Halt {
            return Flow::Halt;
        }
    }
    Flow::Done
}
