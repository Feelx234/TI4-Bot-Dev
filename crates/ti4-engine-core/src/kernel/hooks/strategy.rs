//! Strategy, research and unit-form hooks (`factions/hooks_strategy.rs`, 8 slots).

use super::{ExtraColour, ResearchWaiver, Waiver, hook_group};
use crate::containers::SmallList;
use crate::generated::{Technology, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::{Cx, Kernel};
use crate::state::CoreState;
use crate::generated::StrategyCardKind;

/// Secondary waivers offered.
pub type Waivers = SmallList<Waiver, 4>;
/// Extra prerequisite colours.
pub type Colours = SmallList<ExtraColour, 4>;

hook_group! {
    /// `hooks_strategy::StrategyHooks`.
    StrategyHooks {
        /// The strategy phase ended (state only). String: `phase`.
        each strategy_phase_ended(cx: &mut Cx<'_>) -> ();
        /// Waivers offered to `follower` on `leader`'s secondary. String: `strategy`.
        concat secondary_waivers(s: &CoreState, k: &Kernel, follower: Seat, leader: Seat, card: Option<StrategyCardKind>) -> Waivers;
        /// A waiver was used. String: `strategy`.
        each secondary_waived(cx: &mut Cx<'_>, follower: Seat, leader: Seat, waiver: Waiver) -> ();
        /// `who` may score objectives without controlling their home system. String: `objectives`.
        any scores_without_home(s: &CoreState, who: Seat) -> bool;
        /// Extra prerequisite colours `who` counts. String: `technology`.
        concat extra_prerequisite_colours(s: &CoreState, k: &Kernel, who: Seat) -> Colours;
        /// A research waiver offered for a technology (first module that has one; the research window asks every module's, see
        /// [`StrategyHooks::research_waiver_offers`]). String: `technology`.
        first research_waiver_offer(s: &CoreState, k: &Kernel, who: Seat, tech: Technology) -> Option<ResearchWaiver>;
        /// A research waiver was paid with `payment`. String: `technology`.
        any research_waiver_paid(cx: &mut Cx<'_>, who: Seat, tech: Technology, payment: Waiver) -> bool;
        /// The form a unit takes when `who` builds `from` as `to` (first module that overrides). String: `production`.
        first unit_form_override(s: &CoreState, k: &Kernel, who: Seat, base: UnitBaseType, unit: Unit) -> Option<Unit>;
    }
}

impl StrategyHooks {
    /// Every module's research waiver for `tech`, in module order (`hooks_strategy::research_waiver_offers`: the place in this list is the
    /// index of the answer `waiver|{index}`, and `first research_waiver_offer` keeps only the first).
    #[must_use]
    pub fn research_waiver_offers(&self, s: &CoreState, k: &Kernel, who: Seat, tech: Technology) -> Vec<ResearchWaiver> {
        self.research_waiver_offer.iter().filter_map(|offer| offer(s, k, who, tech)).collect()
    }
}
