//! Production, placement, payment, action-card and exploration hooks (`factions/hooks_economy.rs`).

use super::{Hr, Spend, hook_group};
use crate::containers::SmallList;
use crate::generated::{ActionCard, Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::{Cx, Kernel};
use crate::state::CoreState;

/// Destinations a module adds for production: `(system, planet)`.
pub type Destinations = SmallList<(System, Option<Planet>), 4>;
/// Alternative PDS placements a module offers: `(code, arg)` of the module (rendered by the adapter).
pub type Alternatives = SmallList<super::ComponentAction, 4>;

hook_group! {
    /// `hooks_economy::EconomyHooks`.
    EconomyHooks {
        /// Value of one planet when spent (fold). String: `payment`, `production`, `supply`.
        fold planet_spend_value(s: &CoreState, k: &Kernel, who: Seat, planet: Planet, kind: Spend; value: i64) -> i64;
        /// Worth of one trade good (fold). String: `payment`.
        fold trade_good_worth(s: &CoreState, who: Seat; worth: i64) -> i64;
        /// A unit base cannot be produced by this producer base for `who`. String: `production`.
        any cannot_produce(s: &CoreState, k: &Kernel, who: Seat, unit: UnitBaseType, producer: UnitBaseType) -> bool;
        /// A card effect may not place this unit. String: `action_cards`.
        any effect_placement_forbidden(s: &CoreState, k: &Kernel, who: Seat, unit: Unit) -> bool;
        /// Alternative placements instead of a PDS. String: `production`.
        concat pds_placement_alternative(s: &CoreState, k: &Kernel, who: Seat, system: System, planet: Planet) -> Alternatives;
        /// Perform the chosen PDS alternative; true if claimed.
        any pds_placement_alternative_performed(cx: &mut Cx<'_>, who: Seat, system: System, planet: Planet, option: super::ComponentAction) -> bool;
        /// The form a captured unit returns in (fold). String: `production`, `fleet`.
        fold captured_unit_return_form(s: &CoreState, k: &Kernel, owner: Seat; unit: Unit) -> Unit;
        /// Systems other than the producing one where units of `base` can be placed (sorted, no
        /// duplicates, the producing system filtered out by the dispatcher in `production`).
        concat_sorted production_destinations(s: &CoreState, k: &Kernel, who: Seat, system: System, base: UnitBaseType) -> Destinations;
        /// Produce `unit` by exchange instead of paying. String: `production`.
        any production_unit_exchange(s: &CoreState, k: &Kernel, who: Seat, system: System, unit: Unit) -> bool;
        /// Perform that exchange; true if claimed.
        any production_unit_exchange_performed(cx: &mut Cx<'_>, who: Seat, system: System, unit: Unit) -> bool;
        /// Extra cards drawn with `requested` (summed). String: `strategy_cards`, `action_cards`.
        sum action_card_draw_bonus(s: &CoreState, k: &Kernel, who: Seat, requested: usize) -> usize;
        /// A draw of `requested` cards is about to happen (may ask). String: `strategy_cards`.
        try_each action_card_draw_requested(cx: &mut Cx<'_>, who: Seat, requested: usize) -> Hr<()>;
        /// Cards were drawn (may ask). String: `strategy_cards`.
        try_each action_cards_drawn(cx: &mut Cx<'_>, who: Seat, drawn: &[ActionCard]) -> Hr<()>;
        /// The action-card hand limit (fold). String: `action_cards`, `status`.
        fold action_card_limit(s: &CoreState, k: &Kernel, who: Seat; limit: usize) -> usize;
        /// This player may not hold or play action cards. String: `laws`, `reactions`.
        any action_cards_forbidden(s: &CoreState, who: Seat) -> bool;
        /// Extra production capacity in a system (summed). String: `production`.
        sum extra_production(s: &CoreState, k: &Kernel, who: Seat, system: System) -> i64;
        /// Extra production capacity of one planet (summed). String: `production`.
        sum extra_production_planet(s: &CoreState, k: &Kernel, who: Seat, system: System, planet: Planet) -> i64;
        /// Production cost reduction in a system (summed, floored at zero by the caller). String: `production`.
        sum production_cost_reduction(s: &CoreState, who: Seat, system: System) -> i64;
        /// Draw an extra exploration card. String: `exploration`.
        any explore_extra_draw(s: &CoreState, k: &Kernel, who: Seat, planet: Planet) -> bool;
        /// A planet was explored. String: `exploration`.
        each explored(cx: &mut Cx<'_>, who: Seat, planet: Planet) -> ();
    }
}
