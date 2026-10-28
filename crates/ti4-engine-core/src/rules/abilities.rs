//! Faction ability plumbing: the abilities a seat has through its faction and the queries the shared
//! modules make of them (package B7; `faction_abilities::{of_player, has, combat_modifier,
//! fleet_supply, armada_bonus, status_tokens, waived_prerequisites, substitutes_primary,
//! trades_action_cards, ignores_neighbours, secondary_is_free}`).
//!
//! Each query is the sum of the printed ability's term (read here from the faction's ability list) and
//! the term of the faction modules, which the [`Env`] supplies: the slots of the same name on
//! `HookTable::general` (the string engine implements the trait with its own `factions::` calls until
//! the modules are ported and the table is filled).

use crate::generated::{Ability, StrategyCardKind, TechType, Technology};
use crate::ids::Seat;
use crate::kernel::hooks::CombatCtx;
use crate::state::CoreState;

/// What the faction modules add to the ability terms.
pub trait Env {
    /// `factions::combat_modifier`.
    fn combat_hook(&self, who: Seat, ctx: CombatCtx) -> i64;
    /// `factions::fleet_supply` (a fold: `limit` in, limit out).
    fn fleet_supply_hook(&self, who: Seat, limit: i32) -> i32;
    /// `factions::status_tokens` (a fold).
    fn status_tokens_hook(&self, who: Seat, count: i32) -> i32;
    /// `factions::waived_prerequisites`.
    fn waived_hook(&self, who: Seat, tech: Technology) -> usize;
    /// `factions::substitutes_primary`.
    fn substitutes_primary_hook(&self, who: Seat, card: Option<StrategyCardKind>) -> bool;
    /// `factions::trades_action_cards`.
    fn trades_action_cards_hook(&self, who: Seat) -> bool;
    /// `factions::ignores_neighbours`.
    fn ignores_neighbours_hook(&self, who: Seat) -> bool;
    /// `factions::secondary_is_free`.
    fn secondary_is_free_hook(&self, who: Seat, card: Option<StrategyCardKind>) -> bool;
}

/// The state and the faction modules' answers.
#[derive(Clone, Copy)]
pub struct Cx<'a> {
    pub core: &'a CoreState,
    pub env: &'a dyn Env,
}

/// `faction_abilities::of_player`: the faction's printed abilities, in record order.
#[must_use]
pub fn of_player(core: &CoreState, who: Seat) -> &'static [Ability] {
    core.players().by_seat(who).map_or(&[], |p| p.faction.data().abilities)
}

/// `faction_abilities::has`.
#[must_use]
pub fn has(core: &CoreState, who: Seat, ability: Ability) -> bool {
    of_player(core, who).contains(&ability)
}

/// `faction_abilities::combat_modifier`: Fragile -1, Unrelenting +1, and the modules.
#[must_use]
pub fn combat_modifier(cx: &Cx<'_>, who: Seat, ctx: CombatCtx) -> i64 {
    let own: i64 = of_player(cx.core, who)
        .iter()
        .map(|a| match a {
            Ability::Fragile => -1,
            Ability::Unrelenting => 1,
            _ => 0,
        })
        .sum();
    own + cx.env.combat_hook(who, ctx)
}

/// `faction_abilities::fleet_supply`: Armada adds two, then the modules.
#[must_use]
pub fn fleet_supply(cx: &Cx<'_>, who: Seat, base: i32) -> i32 {
    cx.env.fleet_supply_hook(who, base + armada_bonus(cx.core, who))
}

/// `faction_abilities::armada_bonus`.
#[must_use]
pub fn armada_bonus(core: &CoreState, who: Seat) -> i32 {
    if has(core, who, Ability::Armada) { 2 } else { 0 }
}

/// `promissory::held_foreign` for Cybernetic Enhancements: notes `who` holds that another faction owns.
fn held_cybernetic(core: &CoreState, who: Seat) -> i32 {
    let Some(me) = core.players().by_seat(who) else { return 0 };
    let cards = core.cards();
    let held = cards
        .promissory
        .iter()
        .filter(|(k, holder)| **holder == who && k.note == crate::generated::PromissoryNote::CyberneticEnhancements && k.faction != me.faction)
        .count();
    i32::try_from(held).unwrap_or(0)
}

/// `faction_abilities::status_tokens`: Versatile +1, Hyper Metabolism +1, each foreign Cybernetic
/// Enhancements +1, then the modules.
#[must_use]
pub fn status_tokens(cx: &Cx<'_>, who: Seat, base: i32) -> i32 {
    let versatile = i32::from(has(cx.core, who, Ability::Versatile));
    let hyper = cx.core.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(Technology::HyperMetabolism));
    let count = base + versatile + i32::from(hyper) + held_cybernetic(cx.core, who);
    cx.env.status_tokens_hook(who, count)
}

/// `faction_abilities::waived_prerequisites`: Analytical waives one prerequisite of a non-upgrade.
#[must_use]
pub fn waived_prerequisites(cx: &Cx<'_>, who: Seat, tech: Technology) -> usize {
    let upgrade = tech.data().types.contains(&TechType::Unitupgrade);
    let own = usize::from(!upgrade && has(cx.core, who, Ability::Analytical));
    own + cx.env.waived_hook(who, tech)
}

/// `faction_abilities::substitutes_primary`: Brilliant, for Technology.
#[must_use]
pub fn substitutes_primary(cx: &Cx<'_>, who: Seat, card: Option<StrategyCardKind>) -> bool {
    (has(cx.core, who, Ability::Brilliant) && card == Some(StrategyCardKind::Technology)) || cx.env.substitutes_primary_hook(who, card)
}

/// `faction_abilities::trades_action_cards`: Arbiters.
#[must_use]
pub fn trades_action_cards(cx: &Cx<'_>, who: Seat) -> bool {
    has(cx.core, who, Ability::Arbiters) || cx.env.trades_action_cards_hook(who)
}

/// `faction_abilities::ignores_neighbours`: Guild Ships.
#[must_use]
pub fn ignores_neighbours(cx: &Cx<'_>, who: Seat) -> bool {
    has(cx.core, who, Ability::GuildShips) || cx.env.ignores_neighbours_hook(who)
}

/// `faction_abilities::secondary_is_free`: Masters of Trade, for Trade.
#[must_use]
pub fn secondary_is_free(cx: &Cx<'_>, who: Seat, card: Option<StrategyCardKind>) -> bool {
    (has(cx.core, who, Ability::MastersOfTrade) && card == Some(StrategyCardKind::Trade)) || cx.env.secondary_is_free_hook(who, card)
}
