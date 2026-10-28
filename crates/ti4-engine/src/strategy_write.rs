//! `Area::Strategy` (package B8), the mutation half of the strategy phase flow: a card taken from the mat,
//! dealt, exhausted, swapped, exchanged or dropped, the mat stocked with trade goods and laid out again, as
//! `ti4_engine_core::rules::strategy_flow` functions behind `dual::write`. Each wrapper holds both sides: the
//! string write it replaces and the core rule. The core half answers `None` (the string write runs) when the
//! position is outside the tables: more than 16 owners, a card id the tables do not know, a deck reserve is
//! not relevant here. See `docs/ENGINE_KNOWN_ISSUES.md`, B8.

use ti4_model::id::{PlayerId, StrategyCardId};
use ti4_model::state::GameState;

use crate::dual::{write, Area};

/// `GameState::deal_strategy_card`.
pub fn deal(state: &mut GameState, player: &PlayerId, card: &StrategyCardId) -> bool {
    write(
        Area::Strategy,
        state,
        "strategy.deal",
        |s| s.deal_strategy_card(player, card.clone()),
        |s| real::deal(s, player, card),
    )
}

/// `GameState::exhaust_strategy_card`.
pub fn exhaust(state: &mut GameState, player: &PlayerId, card: &StrategyCardId) -> bool {
    write(
        Area::Strategy,
        state,
        "strategy.exhaust",
        |s| s.exhaust_strategy_card(player, card.clone()),
        |s| real::exhaust(s, player, card),
    )
}

/// `GameState::swap_strategy_card`.
pub fn swap(state: &mut GameState, player: &PlayerId, out: &StrategyCardId, into: &StrategyCardId) -> bool {
    write(
        Area::Strategy,
        state,
        "strategy.swap",
        |s| s.swap_strategy_card(player, out, into.clone()),
        |s| real::swap(s, player, out, into),
    )
}

/// `GameState::clear_strategy_cards`.
pub fn clear(state: &mut GameState, player: &PlayerId) {
    write(
        Area::Strategy,
        state,
        "strategy.clear",
        |s| s.clear_strategy_cards(player),
        |s| real::clear(s, player),
    );
}

/// `draft::take_strategy_card` from the collection of the trade goods on, to the card leaving the mat; the
/// card must be on the mat and the picker seated (checked by the caller). `source` is the staged event's.
pub fn take_card(state: &mut GameState, player: &PlayerId, index: usize, card: &StrategyCardId) {
    // The event is staged by the string side after the write on both backends (not a codec slice).
    let goods = state.strategy_card_goods.get(card).copied().unwrap_or(0);
    let seated = state.player(player).is_some();
    write(
        Area::Strategy,
        state,
        "strategy.take_card",
        |s| {
            let goods = s.strategy_card_goods.remove(card).unwrap_or(0);
            if goods > 0
                && let Some(seat) = s.player_mut(player)
            {
                seat.trade_goods += goods;
            }
            let dealt = s.deal_strategy_card(player, card.clone());
            debug_assert!(dealt, "the checked strategy picker must be dealable");
            s.unclaimed_strategy_cards.remove(index);
        },
        |s| real::take_card(s, player, card),
    );
    if goods > 0 && seated {
        crate::supply::note_trade_goods_gained(state, player, goods, "strategy_card");
    }
}

/// `phase::stock_unclaimed_cards`.
pub fn stock_unclaimed(state: &mut GameState) {
    write(
        Area::Strategy,
        state,
        "strategy.stock_unclaimed",
        |s| {
            for card in s.unclaimed_strategy_cards.clone() {
                *s.strategy_card_goods.entry(card).or_insert(0) += 1;
            }
        },
        real::stock_unclaimed,
    );
}

/// The mat of `phase::begin_next_round`: the round's cards without the ones kept under Political Stability.
pub fn lay_out_mat(state: &mut GameState, cards: &[StrategyCardId]) {
    write(
        Area::Strategy,
        state,
        "strategy.lay_out_mat",
        |s| {
            crate::strategy::clear_initiative_overrides(s);
            let held: std::collections::BTreeSet<StrategyCardId> = s
                .players
                .iter()
                .filter(|player| player.stability)
                .flat_map(|player| player.strategy_cards.iter().cloned())
                .collect();
            s.unclaimed_strategy_cards = cards.iter().filter(|c| !held.contains(*c)).cloned().collect();
        },
        |s| real::lay_out_mat(s, cards),
    );
}

/// `strategy::exchange_strategy_cards` once its checks passed on the string side.
pub fn exchange(
    state: &mut GameState,
    a: &PlayerId,
    card_a: &StrategyCardId,
    b: &PlayerId,
    card_b: &StrategyCardId,
    string: impl Fn(&mut GameState) -> bool,
) -> bool {
    write(Area::Strategy, state, "strategy.exchange", string, |s| real::exchange(s, a, card_a, b, card_b))
}

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::generated::StrategyCard;
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::rules::strategy_flow as rules;
    use ti4_engine_core::state::{CoreState, Slice};
    use ti4_model::id::{PlayerId, StrategyCardId};
    use ti4_model::state::GameState;

    use crate::core_adapter::typed;
    use crate::core_sync::{Footprint, Sources};

    const WRITES: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Strategy],
        &[Slice::Players, Slice::Strategy],
    );

    fn lend<T>(
        gs: &mut GameState,
        texts: &[&str],
        seats: &[&PlayerId],
        f: impl FnOnce(&mut CoreState, &[Seat], &[ti4_engine_core::state::Code]) -> T,
    ) -> Option<T> {
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let who: Vec<Seat> = seats.iter().map(|p| r.seat(p.as_str())).collect::<Option<_>>()?;
        let codes: Vec<_> =
            texts.iter().map(|t| cx.texts.intern(Slice::Strategy, t)).collect::<Result<_, _>>().ok()?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), WRITES, |core, _| f(core, &who, &codes)).ok()
    }

    fn card(id: &StrategyCardId) -> Option<StrategyCard> {
        typed::<StrategyCard>(id.as_str())
    }

    pub fn deal(gs: &mut GameState, p: &PlayerId, c: &StrategyCardId) -> Option<bool> {
        let c = card(c)?;
        lend(gs, &[], &[p], |core, w, _| rules::deal(core, w[0], c))
    }

    pub fn exhaust(gs: &mut GameState, p: &PlayerId, c: &StrategyCardId) -> Option<bool> {
        let c = card(c)?;
        lend(gs, &[], &[p], |core, w, _| rules::exhaust(core, w[0], c))
    }

    pub fn swap(gs: &mut GameState, p: &PlayerId, out: &StrategyCardId, into: &StrategyCardId) -> Option<bool> {
        let (o, i) = (card(out)?, card(into)?);
        lend(gs, &[], &[p], |core, w, _| rules::swap(core, w[0], o, i))
    }

    pub fn clear(gs: &mut GameState, p: &PlayerId) -> Option<()> {
        lend(gs, &[], &[p], |core, w, _| rules::clear(core, w[0]))
    }

    pub fn take_card(gs: &mut GameState, p: &PlayerId, c: &StrategyCardId) -> Option<()> {
        let c = card(c)?;
        lend(gs, &[], &[p], |core, w, _| rules::take_card(core, w[0], c))?
    }

    pub fn stock_unclaimed(gs: &mut GameState) -> Option<()> {
        lend(gs, &[], &[], |core, _, _| rules::stock_unclaimed(core))
    }

    pub fn lay_out_mat(gs: &mut GameState, cards: &[StrategyCardId]) -> Option<()> {
        let list: Vec<StrategyCard> = cards.iter().map(card).collect::<Option<_>>()?;
        lend(gs, &[], &[], |core, _, _| rules::lay_out_mat(core, &list))
    }

    pub fn exchange(
        gs: &mut GameState,
        a: &PlayerId,
        ca: &StrategyCardId,
        b: &PlayerId,
        cb: &StrategyCardId,
    ) -> Option<bool> {
        let (x, y) = (card(ca)?, card(cb)?);
        lend(gs, &[], &[a, b], |core, w, _| rules::exchange(core, w[0], x, w[1], y))
    }
}

#[cfg(not(feature = "typed-core"))]
mod real {
    use ti4_model::id::{PlayerId, StrategyCardId};
    use ti4_model::state::GameState;

    pub fn deal(_: &mut GameState, _: &PlayerId, _: &StrategyCardId) -> Option<bool> {
        None
    }
    pub fn exhaust(_: &mut GameState, _: &PlayerId, _: &StrategyCardId) -> Option<bool> {
        None
    }
    pub fn swap(_: &mut GameState, _: &PlayerId, _: &StrategyCardId, _: &StrategyCardId) -> Option<bool> {
        None
    }
    pub fn clear(_: &mut GameState, _: &PlayerId) -> Option<()> {
        None
    }
    pub fn take_card(_: &mut GameState, _: &PlayerId, _: &StrategyCardId) -> Option<()> {
        None
    }
    pub fn stock_unclaimed(_: &mut GameState) -> Option<()> {
        None
    }
    pub fn lay_out_mat(_: &mut GameState, _: &[StrategyCardId]) -> Option<()> {
        None
    }
    pub fn exchange(
        _: &mut GameState,
        _: &PlayerId,
        _: &StrategyCardId,
        _: &PlayerId,
        _: &StrategyCardId,
    ) -> Option<bool> {
        None
    }
}

#[cfg(test)]
mod tests {
    use ti4_content::ContentStore;
    use ti4_model::content_types::POK;

    use super::*;
    use crate::dual::{report, take_report, with_mask, AreaMask, Backend};

    fn game() -> GameState {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "arborec")], POK);
        state.strategy_card_goods.insert(state.unclaimed_strategy_cards[4].clone(), 2);
        state
    }

    /// The whole flow in lockstep: the draft (with goods on a card), exhaustion, a swap, an exchange, the
    /// stock of the mat, the clear and the next mat; every step ran in the core (the probes) and none differed.
    #[test]
    fn the_strategy_flow_runs_in_lockstep_with_the_core() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
            let mut state = game();
            let all = state.unclaimed_strategy_cards.clone();
            #[cfg(feature = "typed-core")]
            {
                let mut probe = state.clone();
                assert!(real::stock_unclaimed(&mut probe).is_some());
                assert!(real::take_card(&mut probe, &state.speaker, &all[4]).is_some());
            }
            while let Some(choice) = crate::draft::strategy_options(&state, ContentStore::embedded()) {
                // The last option every time: a first-option default would hide an order difference.
                let pick = choice.options.last().cloned().unwrap();
                crate::draft::take_strategy_card(&mut state, ContentStore::embedded(), pick).unwrap();
            }
            crate::phase::stock_unclaimed_cards(&mut state);
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            let ca = state.player(&a).unwrap().strategy_cards[0].clone();
            let cb = state.player(&b).unwrap().strategy_cards[0].clone();
            assert!(exhaust(&mut state, &a, &ca));
            assert!(!exhaust(&mut state, &a, &ca));
            assert!(crate::strategy::exchange_strategy_cards(&mut state, &a, &ca, &b, &cb));
            assert!(crate::strategy::exchange_strategy_cards(&mut state, &a, &cb, &b, &ca));
            clear(&mut state, &b);
            crate::phase::begin_next_round(&mut state, all);
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(r.areas[&Area::Strategy].calls >= 8, "{r:?}");
        let _ = take_report();
    }
}
