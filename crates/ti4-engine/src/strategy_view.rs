//! `Area::Strategy` (package B8): the typed strategy reads of `ti4_engine_core::rules::strategy`
//! behind the string interface. One [`StratView`] is the typed view of one position (the codecs of
//! the Meta, Seats, Players and Strategy slices, plus the Board when a follower's eligibility is
//! asked) held for every question asked of that position: the draft's picker and cards, the whole
//! follower loop of a secondary window. It also implements the core's `Env`: the answers of the
//! faction modules and of the rules whose owners have not landed come from the string engine, so
//! the sums stay exact. A text id the tables do not know makes a method answer `None` and the
//! caller keeps the string answer.

use ti4_content::ContentStore;
use ti4_engine_core::generated::{Faction, StrategyCard, StrategyCardKind as CoreKind, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::rules::strategy::{self as rules, Cx, Env};
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, StrategyCardId, SystemId};
use ti4_model::state::GameState;

use crate::core_sync::{CoreView, Ctx, Sources};
use crate::generated::IdStr;

/// What the draft and the held cards need.
pub const READS_LIGHT: SliceSet = SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Strategy]);
/// The same with the board (the exhausted planets of Diplomacy's secondary).
pub const READS_FULL: SliceSet =
    SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Strategy]);

/// The typed view of one position.
pub struct StratView<'a> {
    view: CoreView,
    cx: Ctx,
    kernel: Kernel,
    state: &'a GameState,
    content: &'a ContentStore,
    sources: SourceSet,
}

/// The kind of the content crate for a core kind.
const fn content_kind(kind: CoreKind) -> ti4_content::StrategyCardKind {
    use ti4_content::StrategyCardKind as K;
    match kind {
        CoreKind::Construction => K::Construction,
        CoreKind::Diplomacy => K::Diplomacy,
        CoreKind::Imperial => K::Imperial,
        CoreKind::Leadership => K::Leadership,
        CoreKind::Politics => K::Politics,
        CoreKind::Technology => K::Technology,
        CoreKind::Trade => K::Trade,
        CoreKind::Warfare => K::Warfare,
    }
}

/// The core's kind for a content kind.
#[must_use]
pub const fn core_kind(kind: ti4_content::StrategyCardKind) -> CoreKind {
    use ti4_content::StrategyCardKind as K;
    match kind {
        K::Construction => CoreKind::Construction,
        K::Diplomacy => CoreKind::Diplomacy,
        K::Imperial => CoreKind::Imperial,
        K::Leadership => CoreKind::Leadership,
        K::Politics => CoreKind::Politics,
        K::Technology => CoreKind::Technology,
        K::Trade => CoreKind::Trade,
        K::Warfare => CoreKind::Warfare,
    }
}

impl<'a> StratView<'a> {
    fn build(state: &'a GameState, content: &'a ContentStore, sources: SourceSet, reads: SliceSet) -> Option<Self> {
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::of(state, &mut cx, Sources { galaxy: None, rng: None }, reads).ok()?;
        Some(Self { view, cx, kernel: Kernel::plain(), state, content, sources })
    }

    /// The view of a position for the draft and the held cards; `None` when something is not mapped.
    #[must_use]
    pub fn light(state: &'a GameState) -> Option<Self> {
        Self::build(state, ContentStore::embedded(), ti4_model::content_types::FULL, READS_LIGHT)
    }

    /// The view for the follower checks, whatever backend the area runs on (the offline check).
    #[must_use]
    pub fn full(state: &'a GameState, content: &'a ContentStore, sources: SourceSet) -> Option<Self> {
        Self::build(state, content, sources, READS_FULL)
    }

    /// The view for the follower checks of one position, or `None` when the area runs on the string
    /// side (nothing is built then) or something is not mapped.
    #[must_use]
    pub fn open(state: &'a GameState, content: &'a ContentStore, sources: SourceSet) -> Option<Self> {
        if crate::dual::backend(crate::dual::Area::Strategy) == crate::dual::Backend::String {
            return None;
        }
        Self::build(state, content, sources, READS_FULL)
    }

    fn rules_cx(&self) -> Cx<'_> {
        Cx { core: &self.view, kernel: &self.kernel, env: self }
    }

    fn seat(&self, p: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(p.as_str())
    }

    fn pid(&self, s: Seat) -> Option<PlayerId> {
        self.cx.owners.id(s).map(PlayerId::new)
    }

    fn card_id(c: StrategyCard) -> StrategyCardId {
        StrategyCardId::new(c.as_str())
    }

    /// `phase::strategy_pick_order`.
    #[must_use]
    pub fn pick_order(&self) -> Option<Vec<PlayerId>> {
        rules::pick_order(&self.view).as_slice().iter().map(|s| self.pid(*s)).collect()
    }

    /// `phase::next_strategy_picker`.
    #[must_use]
    pub fn next_picker(&self) -> Option<Option<PlayerId>> {
        match rules::next_picker(&self.view) {
            None => Some(None),
            Some(s) => Some(Some(self.pid(s)?)),
        }
    }

    /// The picker and the unclaimed cards of `draft::strategy_options`.
    #[must_use]
    pub fn draft(&self) -> Option<Option<(PlayerId, Vec<StrategyCardId>)>> {
        match rules::draft(&self.view) {
            None => Some(None),
            Some(d) => Some(Some((self.pid(d.picker)?, d.cards.as_slice().iter().map(|c| Self::card_id(*c)).collect()))),
        }
    }

    /// `Player::unused_strategy_cards`, owned.
    #[must_use]
    pub fn unused_cards(&self, player: &PlayerId) -> Option<Vec<StrategyCardId>> {
        let seat = self.seat(player)?;
        Some(rules::unused_cards(&self.view, seat).as_slice().iter().map(|c| Self::card_id(*c)).collect())
    }

    /// `strategy::secondary_eligible` (the view must be [`Self::open`]ed, it reads the board).
    #[must_use]
    pub fn secondary_eligible(&self, player: &PlayerId, card: &StrategyCardId) -> Option<bool> {
        let (seat, card) = (self.seat(player)?, StrategyCard::from_id(card.as_str())?);
        Some(rules::secondary_eligible(&self.rules_cx(), seat, card))
    }
}

/// `strategy_cards::commodity_limit` of a seat: its faction's printed commodity value (needs no view).
#[must_use]
pub fn commodity_limit(state: &GameState, player: &PlayerId) -> Option<i32> {
    let seat = state.player(player);
    Some(seat.map_or(0, |s| Faction::from_id(s.faction.as_str()).map_or(0, rules::commodity_limit)))
}

/// `strategy::secondary_costs_token` of a card id (needs no view).
#[must_use]
pub fn secondary_costs_token(card: &StrategyCardId) -> Option<bool> {
    Some(rules::secondary_costs_token(Some(StrategyCard::from_id(card.as_str())?.data().kind)))
}

impl Env for StratView<'_> {
    fn secondary_is_free(&self, who: Seat, card: Option<CoreKind>) -> bool {
        let Some(p) = self.pid(who) else { return false };
        crate::faction_abilities::secondary_is_free(self.state, self.content, &p, card.map(content_kind))
    }

    fn substitutes_primary(&self, who: Seat, card: Option<CoreKind>) -> bool {
        let Some(p) = self.pid(who) else { return false };
        crate::faction_abilities::substitutes_primary(self.state, self.content, &p, card.map(content_kind))
    }

    fn leadership_affordable(&self, who: Seat) -> bool {
        self.pid(who).is_some_and(|p| {
            crate::strategy_cards::leadership_influence_eligible(self.state, self.content, self.sources, &p)
        })
    }

    fn can_research(&self, who: Seat) -> bool {
        self.pid(who).is_some_and(|p| {
            !crate::technology::researchable(self.state, self.content, self.sources, &p).is_empty()
        })
    }

    fn has_specialty_research_planet(&self, who: Seat) -> bool {
        self.pid(who).is_some_and(|p| {
            !crate::breakthroughs::specialty_research_planets(self.state, self.content, self.sources, &p).is_empty()
        })
    }

    fn technology_secondary_affordable(&self, who: Seat) -> bool {
        use crate::production::Spend;
        let Some(p) = self.pid(who) else { return false };
        let cost = crate::strategy_cards::TECHNOLOGY_SECONDARY_COST;
        crate::payment::affordable(self.state, self.content, self.sources, &p, cost, Spend::Resources)
            // Xander Alexin Victori III: commodities as trade goods, offered when the paid research opens.
            || crate::factions::keleres::with_agent_granted(self.state, &p, |granted| {
                crate::payment::affordable(granted, self.content, self.sources, &p, cost, Spend::Resources)
            })
            .unwrap_or(false)
    }

    fn secret_hand_all_scored(&self, who: Seat) -> bool {
        self.pid(who).is_some_and(|p| crate::secrets::hand_is_all_scored(self.state, self.content, &p))
    }

    fn can_produce_in(&self, who: Seat, system: System) -> bool {
        self.pid(who).is_some_and(|p| {
            crate::production::capacity(
                self.state,
                self.content,
                self.sources,
                &p,
                &SystemId::new(system.as_str()),
            ) > 0
        })
    }
}
