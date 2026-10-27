//! Strategy phase and strategy cards (package B8; LRR 26, 83, 91): who picks, which cards are on the
//! mat, which cards a seat may still use, what a secondary costs and who may follow it. The reads of
//! `phase::{strategy_pick_order, next_strategy_picker}`, `draft::strategy_options`,
//! `Player::unused_strategy_cards`, `strategy::{secondary_costs_token, secondary_is_free,
//! secondary_eligible, secondary_can_do_something}` and `strategy_cards::commodity_limit`.
//!
//! What the core cannot see yet is asked of an [`Env`]: the faction modules (until package C), the
//! research, payment and secret-hand rules whose owners have not landed, and the Keleres agent. The
//! string engine implements the trait, so the answers stay exact; the hook table of the [`Kernel`]
//! is asked first and replaces the `Env` when the factions are ported.

use crate::containers::SmallList;
use crate::generated::{Faction, StrategyCard, StrategyCardKind, System};
use crate::ids::Seat;
use crate::kernel::Kernel;
use crate::query::{board, seats};
use crate::query::seats::SeatOrder;
use crate::state::{CoreState, Player};

/// `RULES_IMPERIAL_SECRET_CAP` of `ti4-model`: from this rules version Imperial's secondary needs an
/// unscored secret to draw against.
pub const RULES_IMPERIAL_SECRET_CAP: u32 = 18;

/// What the strategy rules ask of the factions and of rules not yet in the core.
pub trait Env {
    /// `faction_abilities::secondary_is_free`: the seat's faction waives the token (Hacan's Masters
    /// of Trade, the modules' hooks).
    fn secondary_is_free(&self, who: Seat, card: Option<StrategyCardKind>) -> bool;
    /// `faction_abilities::substitutes_primary`: the secondary is resolved as the primary (Jol-Nar).
    fn substitutes_primary(&self, who: Seat, card: Option<StrategyCardKind>) -> bool;
    /// `strategy_cards::leadership_influence_eligible`: three influence, the Keleres agent included.
    fn leadership_affordable(&self, who: Seat) -> bool;
    /// `!technology::researchable(..).is_empty()`.
    fn can_research(&self, who: Seat) -> bool;
    /// `!breakthroughs::specialty_research_planets(..).is_empty()`.
    fn has_specialty_research_planet(&self, who: Seat) -> bool;
    /// `payment::affordable(TECHNOLOGY_SECONDARY_COST, Resources)`, also as the Keleres agent grants.
    fn technology_secondary_affordable(&self, who: Seat) -> bool;
    /// `secrets::hand_is_all_scored`.
    fn secret_hand_all_scored(&self, who: Seat) -> bool;
    /// `production::capacity(.., system) > 0`.
    fn can_produce_in(&self, who: Seat, system: System) -> bool;
}

/// What the rules read besides the state: the kernel (hook table) and the [`Env`].
#[derive(Clone, Copy)]
pub struct Cx<'a> {
    pub core: &'a CoreState,
    pub kernel: &'a Kernel,
    pub env: &'a dyn Env,
}

fn player(core: &CoreState, seat: Seat) -> Option<&Player> {
    core.players().iter().find(|p| p.seat == seat)
}

/// `phase::strategy_pick_order`: who picks, in order, for the whole phase: clockwise from the speaker,
/// seats under Political Stability left out, every second lap reversed.
#[must_use]
pub fn pick_order(core: &CoreState) -> SmallList<Seat, 16> {
    let seats: SeatOrder = seats::speaker_order(core);
    let mut out = SmallList::new();
    let laps = usize::from(core.meta().strategy_cards_per_player);
    for lap in 0..laps {
        let eligible = seats.as_slice().iter().filter(|s| player(core, **s).is_none_or(|p| !p.econ.stability));
        if lap % 2 == 0 {
            for s in eligible {
                out.push(*s);
            }
        } else {
            for s in eligible.collect::<SmallList<&Seat, 8>>().as_slice().iter().rev() {
                out.push(**s);
            }
        }
    }
    out
}

/// `phase::next_strategy_picker`: the seat whose turn it is to pick, or `None` when the draft is done.
#[must_use]
pub fn next_picker(core: &CoreState) -> Option<Seat> {
    let order = pick_order(core);
    if order.is_empty() || core.strategy().unclaimed.is_empty() {
        return None;
    }
    let dealt: usize = core.players().iter().map(|p| p.strat.len()).sum();
    let per = usize::from(core.meta().strategy_cards_per_player);
    let retained: usize = core.players().iter().filter(|p| p.econ.stability).map(|p| p.strat.len().min(per)).sum();
    let picks = dealt.saturating_sub(retained);
    if picks >= order.len() {
        return None;
    }
    order.as_slice().get(picks).copied()
}

/// One open pick of the draft: who picks and from which cards (mat order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub picker: Seat,
    pub cards: SmallList<StrategyCard, 8>,
}

/// `draft::strategy_options` without the labels: the picker and the unclaimed cards in mat order.
#[must_use]
pub fn draft(core: &CoreState) -> Option<Draft> {
    let picker = next_picker(core)?;
    Some(Draft { picker, cards: core.strategy().unclaimed.clone() })
}

/// `Player::unused_strategy_cards`: held cards whose strategic action is still available, in hand order.
#[must_use]
pub fn unused_cards(core: &CoreState, seat: Seat) -> SmallList<StrategyCard, 3> {
    let mut out = SmallList::new();
    if let Some(p) = player(core, seat) {
        for c in p.strat.as_slice() {
            if !p.strat_exhausted.contains(*c) {
                out.push(*c);
            }
        }
    }
    out
}

/// `strategy_cards::commodity_limit` of a faction.
#[must_use]
pub fn commodity_limit(faction: Faction) -> i32 {
    i32::from(faction.data().commodities)
}

/// `strategy::secondary_costs_token`: every secondary but Leadership's takes a strategy token. An
/// unknown card (`None`) costs one.
#[must_use]
pub fn secondary_costs_token(card: Option<StrategyCardKind>) -> bool {
    card != Some(StrategyCardKind::Leadership)
}

/// `strategy::secondary_is_free`: the seat follows `card` without a token.
#[must_use]
pub fn secondary_is_free(cx: &Cx<'_>, who: Seat, card: Option<StrategyCardKind>) -> bool {
    card.is_some()
        && (cx.kernel.hooks.general.secondary_is_free(cx.core, cx.kernel, who, card) || cx.env.secondary_is_free(who, card))
}

/// `strategy::secondary_eligible`: whether the seat may follow `card` at all.
#[must_use]
pub fn secondary_eligible(cx: &Cx<'_>, who: Seat, card: StrategyCard) -> bool {
    let kind = Some(card.data().kind);
    if kind == Some(StrategyCardKind::Leadership) {
        // 52.3 costs influence rather than a strategy token.
        return cx.env.leadership_affordable(who);
    }
    let Some(p) = player(cx.core, who) else {
        return false;
    };
    if !(!secondary_costs_token(kind) || secondary_is_free(cx, who, kind) || p.econ.strategic_tokens > 0) {
        return false;
    }
    can_do_something(cx, who, p, card.data().kind)
}

/// `strategy::secondary_can_do_something`: some secondaries resolve to nothing under conditions the
/// offer did not check (and still charge the token).
#[must_use]
pub fn can_do_something(cx: &Cx<'_>, who: Seat, p: &Player, kind: StrategyCardKind) -> bool {
    match kind {
        StrategyCardKind::Technology => {
            if !cx.env.can_research(who) {
                return false;
            }
            cx.kernel.hooks.general.substitutes_primary(cx.core, cx.kernel, who, Some(kind))
                || cx.env.substitutes_primary(who, Some(kind))
                || cx.env.has_specialty_research_planet(who)
                || cx.env.technology_secondary_affordable(who)
        }
        StrategyCardKind::Imperial if cx.core.meta().rules_version >= RULES_IMPERIAL_SECRET_CAP => {
            !cx.env.secret_hand_all_scored(who)
        }
        StrategyCardKind::Diplomacy => {
            board::controlled_planets(cx.core, who).as_slice().iter().any(|(_, planet)| board::exhausted(cx.core, *planet))
        }
        StrategyCardKind::Trade => i32::from(p.econ.commodities) < commodity_limit(p.faction),
        StrategyCardKind::Warfare => p.home_system.is_some_and(|home| cx.env.can_produce_in(who, home)),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Players, Seats};

    struct NoEnv;
    impl Env for NoEnv {
        fn secondary_is_free(&self, _: Seat, _: Option<StrategyCardKind>) -> bool {
            false
        }
        fn substitutes_primary(&self, _: Seat, _: Option<StrategyCardKind>) -> bool {
            false
        }
        fn leadership_affordable(&self, _: Seat) -> bool {
            true
        }
        fn can_research(&self, _: Seat) -> bool {
            true
        }
        fn has_specialty_research_planet(&self, _: Seat) -> bool {
            false
        }
        fn technology_secondary_affordable(&self, _: Seat) -> bool {
            false
        }
        fn secret_hand_all_scored(&self, _: Seat) -> bool {
            false
        }
        fn can_produce_in(&self, _: Seat, _: System) -> bool {
            true
        }
    }

    fn core(n: u8, per: u8) -> CoreState {
        let mut s = CoreState::all_loaded();
        let players: Vec<Player> = (0..n).map(|i| Player::new(Seat(i), Faction::ALL[usize::from(i)])).collect();
        let seats_list: Vec<Seat> = (0..n).map(Seat).collect();
        s.seats = Seats::new(&seats_list, &seats_list, n);
        s.players = Players::from_vec(players);
        s.meta.strategy_cards_per_player = per;
        s.strategy.unclaimed = StrategyCard::ALL[..8].iter().copied().collect();
        s
    }

    #[test]
    fn snake_order_for_two_cards_each() {
        let mut c = core(3, 2);
        c.meta.speaker = Seat(1);
        let order: Vec<u8> = pick_order(&c).as_slice().iter().map(|s| s.0).collect();
        assert_eq!(order, [1, 2, 0, 0, 2, 1]);
        assert_eq!(next_picker(&c), Some(Seat(1)));
    }

    #[test]
    fn follow_rules() {
        let mut c = core(3, 1);
        let k = Kernel::plain();
        let cx = Cx { core: &c, kernel: &k, env: &NoEnv };
        assert!(!secondary_costs_token(Some(StrategyCardKind::Leadership)));
        assert!(secondary_costs_token(None));
        assert!(!secondary_is_free(&cx, Seat(0), None));
        let _ = &mut c;
    }
}
