//! `Area::LeadersAbilities` (package B7): the typed leader reads of `ti4_engine_core::rules::leaders`
//! behind the string interface. A [`LeaderView`] is the typed view of one position plus the string
//! engine's answers to what the core cannot see yet (faction hooks, the galaxy, the supply): it
//! implements the core's `Env`. A text id the tables do not know makes a method answer `None` and the
//! caller keeps the string answer. Hold one view per option pass; build it with the reads the pass needs.

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::bits::SysSet;
use ti4_engine_core::generated::{Leader, LeaderType, Planet, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::query::Scope;
use ti4_engine_core::rules::leaders::{self as rules, Cx, Env};
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlanetId, PlayerId};
use ti4_model::state::GameState;

use crate::core_sync::{CoreView, Ctx, Sources};
use crate::generated::IdStr;

/// The slices the unlock check reads (statuses, trade goods, technologies, the board, laws for planet
/// values, scored objectives, event feats).
pub const READS_UNLOCK: SliceSet =
    SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Objectives]);
/// The slices the action-phase offer reads.
pub const READS_ACTIONS: SliceSet = SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board]);
/// The slices `has_commander_ability` reads (statuses, the grants, faceup Alliance notes).
pub const READS_ABILITY: SliceSet = SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Cards, Slice::Marks]);
/// Statuses only.
pub const READS_STATUS: SliceSet = SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players]);

/// The typed view of one position.
pub struct LeaderView<'a> {
    view: CoreView,
    cx: Ctx,
    scope: Scope,
    blanked: bool,
    state: &'a GameState,
    content: Option<&'a ContentStore>,
    sources: SourceSet,
    galaxy: Option<&'a Galaxy>,
}

fn typed<T: IdStr>(id: &str) -> Option<T> {
    T::from_id(id)
}

impl<'a> LeaderView<'a> {
    /// The view of a position with these slices; `None` when something is not mapped. A call site
    /// that has no content store (`has_commander_ability`) passes `None`: only the methods that need
    /// none may be asked of such a view.
    #[must_use]
    pub fn of(
        state: &'a GameState,
        content: Option<&'a ContentStore>,
        sources: SourceSet,
        galaxy: Option<&'a Galaxy>,
        reads: SliceSet,
    ) -> Option<Self> {
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::of(state, &mut cx, Sources { galaxy: None::<&Galaxy>, rng: None }, reads).ok()?;
        Some(Self {
            view,
            cx,
            scope: crate::core_adapter::scope(sources),
            blanked: crate::laws::blanked(state),
            state,
            content,
            sources,
            galaxy,
        })
    }

    fn rules_cx(&self) -> Cx<'_> {
        Cx { core: &self.view, scope: self.scope, blanked: self.blanked, env: self }
    }

    fn seat(&self, p: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(p.as_str())
    }

    fn pid(&self, s: Seat) -> PlayerId {
        PlayerId::new(self.cx.owners.id(s).unwrap_or_default())
    }

    fn ids(list: &[Leader]) -> Vec<LeaderId> {
        list.iter().map(|l| LeaderId::new(l.as_str())).collect()
    }

    /// `leaders::status`.
    #[must_use]
    pub fn status(&self, player: &PlayerId, leader: &LeaderId) -> Option<Option<ti4_model::state::LeaderStatus>> {
        use ti4_engine_core::state::LeaderStatus as C;
        use ti4_model::state::LeaderStatus as S;
        let (who, l) = (self.seat(player)?, typed::<Leader>(leader.as_str())?);
        Some(rules::status(&self.view, who, l).map(|s| match s {
            C::Locked => S::Locked,
            C::Readied => S::Readied,
            C::Exhausted => S::Exhausted,
            C::Unlocked => S::Unlocked,
            C::Purged => S::Purged,
        }))
    }

    /// `leaders::of_kind`.
    #[must_use]
    pub fn of_kind(&self, player: &PlayerId, kind: &str) -> Option<Vec<LeaderId>> {
        let who = self.seat(player)?;
        // A kind the corpus does not name matches no leader.
        let Some(wanted) = typed::<LeaderType>(kind) else { return Some(Vec::new()) };
        Some(Self::ids(rules::of_kind(&self.view, who, wanted).as_slice()))
    }

    /// `leaders::usable`.
    #[must_use]
    pub fn usable(&self, player: &PlayerId) -> Option<Vec<LeaderId>> {
        Some(Self::ids(rules::usable(&self.view, self.seat(player)?).as_slice()))
    }

    /// `leaders::combines_planet_values`.
    #[must_use]
    pub fn combines_planet_values(&self, player: &PlayerId) -> Option<bool> {
        Some(rules::combines_planet_values(&self.view, self.seat(player)?))
    }

    /// `promissory::has_commander_ability`.
    #[must_use]
    pub fn has_commander_ability(&self, player: &PlayerId, commander: &str) -> Option<bool> {
        Some(rules::has_commander_ability(&self.rules_cx(), self.seat(player)?, typed(commander)?))
    }

    /// `leaders::commander_unlocked` (outer `None`: not mapped).
    #[must_use]
    pub fn commander_unlocked(&self, player: &PlayerId, leader: &LeaderId) -> Option<Option<bool>> {
        Some(rules::commander_unlocked(&self.rules_cx(), self.seat(player)?, typed(leader.as_str())?))
    }

    /// The leaders `leaders::check_unlocks` unlocks, in its order.
    #[must_use]
    pub fn unlockable(&self, player: &PlayerId) -> Option<Vec<LeaderId>> {
        Some(Self::ids(rules::unlockable(&self.rules_cx(), self.seat(player)?).as_slice()))
    }

    /// The own leaders `leaders::component_actions` offers.
    #[must_use]
    pub fn action_candidates(&self, player: &PlayerId) -> Option<Vec<LeaderId>> {
        let who = self.seat(player)?;
        let delivered = |l: Leader| crate::leaders::action_leader_delivered(&LeaderId::new(l.as_str()));
        Some(Self::ids(rules::action_candidates(&self.rules_cx(), who, delivered).as_slice()))
    }
}

impl Env for LeaderView<'_> {
    fn faction_commander_unlocked(&self, who: Seat, leader: Leader) -> Option<bool> {
        crate::factions::commander_unlocked(
            self.state,
            self.content?,
            self.sources,
            self.galaxy,
            &self.pid(who),
            &LeaderId::new(leader.as_str()),
        )
    }

    fn resource_bonus(&self, planet: Planet) -> i64 {
        crate::factions::bastion_units::resource_bonus(self.state, &PlanetId::new(planet.as_str()))
    }

    fn mecatol_area(&self) -> Option<SysSet> {
        let galaxy = self.galaxy?;
        let mecatol = crate::seating::mecatol_in_galaxy(galaxy);
        let mut set = SysSet::EMPTY;
        for id in galaxy.adjacent(mecatol).into_iter().chain([mecatol]) {
            if let Some(sys) = typed::<System>(id) {
                set.insert(sys.index());
            }
        }
        Some(set)
    }

    fn leader_action(&self, who: Seat, leader: Leader) -> Option<bool> {
        crate::factions::leader_action(self.state, self.content?, &self.pid(who), &LeaderId::new(leader.as_str()))
    }

    fn imperia_grants(&self, who: Seat, commander: Leader) -> bool {
        crate::factions::mahact::imperia_grants(self.state, &self.pid(who), commander.as_str())
    }

    fn other_action_resolves(&self, who: Seat, leader: Leader) -> bool {
        self.content.is_some_and(|content| {
            crate::leaders::can_resolve_action(self.state, content, &self.pid(who), &LeaderId::new(leader.as_str()))
        })
    }
}
