//! `Area::Invasion` (package B2): the typed invasion rules of `ti4_engine_core::rules::invasion` behind a
//! string interface.
//!
//! One [`InvasionView`] is built per call site (the codecs of the slices the rules read run once). Every
//! method answers a question of `invasion.rs` with the same value in the same order, or `None` when
//! something is not mapped (an id the tables do not know, a rule held in a way the core does not model yet),
//! in which case the caller keeps the string answer. Nothing here decides a rule. The faction modules not
//! ported to `ti4-engine-core-factions` answer through the same string bridge as the combat view
//! (`core_combat::window_env`).

use ti4_content::ContentStore;
use ti4_engine_core::generated::{Planet, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::rules::invasion as ci;
use ti4_engine_core::rules::invasion::Rd;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;

use crate::core_combat::{window_env, WindowEnv, READS};
use crate::core_sync::{CoreView, Ctx, Sources};
use crate::generated::IdStr;
use crate::invasion::PlannedBombard;

fn typed<T: IdStr>(id: &str) -> Option<T> {
    T::from_id(id)
}

/// The typed view of one position, for the invasion queries.
pub struct InvasionView<'g> {
    _position: std::marker::PhantomData<&'g GameState>,
    view: CoreView,
    cx: Ctx,
    env: WindowEnv,
    blanked: bool,
}

impl<'g> InvasionView<'g> {
    /// The view of a position; `None` when something is not mapped.
    #[must_use]
    pub fn of(gs: &'g GameState, _content: &'g ContentStore, sources: SourceSet) -> Option<Self> {
        let mut cx = Ctx::of(gs).ok()?;
        let view = CoreView::of(gs, &mut cx, Sources { galaxy: None, rng: None }, READS).ok()?;
        let env = window_env(gs, &cx, sources)?;
        Some(Self { _position: std::marker::PhantomData, view, cx, env, blanked: crate::laws::blanked(gs) })
    }

    fn seat(&self, p: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(p.as_str())
    }

    fn player(&self, s: Seat) -> Option<PlayerId> {
        self.cx.owners.id(s).map(PlayerId::new)
    }

    fn kernel(&self) -> &Kernel {
        &self.env.kernel
    }

    fn rd(&self) -> Rd<'_> {
        Rd::new(&self.view, self.kernel()).with_blanked(self.blanked)
    }

    /// `invasion::bombardable`. `None` while the invader holds the L1Z1X commander's ability in a way only
    /// the leaders package models (the Mahact Imperia, an "An" promissory note).
    #[must_use]
    pub fn bombardable(&self, gs: &GameState, system: &SystemId, planet: &PlanetId, invader: &PlayerId) -> Option<bool> {
        if commander_hole(gs, invader) {
            return None;
        }
        let (sys, pl, seat) = (typed::<System>(system.as_str())?, typed::<Planet>(planet.as_str())?, self.seat(invader)?);
        Some(ci::bombardable(self.rd(), sys, pl, seat))
    }

    /// `invasion::bombard_rolls`: the rolls of the bombardment per planet, unrolled.
    #[must_use]
    pub fn bombard_plan(&self, gs: &GameState, system: &SystemId, invader: &PlayerId) -> Option<Vec<PlannedBombard>> {
        if commander_hole(gs, invader) {
            return None;
        }
        let (sys, seat) = (typed::<System>(system.as_str())?, self.seat(invader)?);
        let plan = ci::bombard_plan(self.rd(), sys, seat);
        plan.into_iter()
            .map(|p| {
                Some(PlannedBombard {
                    planet: PlanetId::new(p.planet.as_str()),
                    rolls: p.rolls.iter().map(|r| (r.unit.as_str().to_owned(), r.hits_on, r.dice)).collect(),
                    held: p.held,
                    victims: p.victims.iter().map(|s| self.player(s)).collect::<Option<_>>()?,
                })
            })
            .collect()
    }
}

/// The L1Z1X commander's ability is held by the Mahact Imperia or an "An" promissory note: not modelled by
/// the core until the leaders package (B7) lands `has_commander_ability`.
fn commander_hole(gs: &GameState, who: &PlayerId) -> bool {
    gs.player(who).is_some_and(|p| p.faction.as_str() == "mahact")
        || gs.promissory_notes.iter().any(|(note, holder)| holder == who && crate::promissory::alias_of(note) == "an")
}
