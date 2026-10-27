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
use ti4_engine_core::generated::{Planet, System, Unit};
use ti4_engine_core::state::Piece;
use ti4_model::units::Unit as StrUnit;
use crate::core_combat::OptionRow;
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

    /// The read bundle with the invader's L1Z1X commander ability from the leaders package (B7), which also
    /// knows the Mahact Imperia and the Alliance notes.
    fn rd_for(&self, gs: &GameState, invader: &PlayerId) -> Option<Rd<'_>> {
        let held = crate::leaders_view::LeaderView::of(gs, None, SourceSet::default(), None, crate::leaders_view::READS_ABILITY)?
            .has_commander_ability(invader, "l1z1xcommander")?;
        Some(self.rd().with_l1z1x_commander(held))
    }

    #[must_use]
    pub fn bombardable(&self, gs: &GameState, system: &SystemId, planet: &PlanetId, invader: &PlayerId) -> Option<bool> {
        let (sys, pl, seat) = (typed::<System>(system.as_str())?, typed::<Planet>(planet.as_str())?, self.seat(invader)?);
        Some(ci::bombardable(self.rd_for(gs, invader)?, sys, pl, seat))
    }

    /// `invasion::bombard_rolls`: the rolls of the bombardment per planet, unrolled.
    #[must_use]
    pub fn bombard_plan(&self, gs: &GameState, system: &SystemId, invader: &PlayerId) -> Option<Vec<PlannedBombard>> {
        let (sys, seat) = (typed::<System>(system.as_str())?, self.seat(invader)?);
        let plan = ci::bombard_plan(self.rd_for(gs, invader)?, sys, seat);
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

impl InvasionView<'_> {
    fn str_unit(&self, p: Piece) -> Option<StrUnit> {
        Some(StrUnit {
            type_id: ti4_model::id::UnitTypeId::new(p.kind().as_str()),
            owner: self.player(p.owner())?,
            sustained_damage: p.sustained(),
            galvanized: p.galvanized(),
        })
    }

    fn piece(&self, u: &StrUnit) -> Option<Piece> {
        Some(
            Piece::new(typed::<Unit>(u.type_id.as_str())?, self.seat(&u.owner)?)
                .with_sustained(u.sustained_damage)
                .with_galvanized(u.galvanized),
        )
    }

    /// `invasion::landable`.
    #[must_use]
    pub fn landable(&self, player: &PlayerId, system: &SystemId) -> Option<Vec<StrUnit>> {
        let (seat, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        ci::landable(self.rd(), seat, sys).into_iter().map(|p| self.str_unit(p)).collect()
    }

    /// `invasion::landable_planets`.
    #[must_use]
    pub fn landable_planets(&self, system: &SystemId) -> Option<Vec<PlanetId>> {
        let sys = typed::<System>(system.as_str())?;
        Some(ci::landable_planets(self.rd(), sys).into_iter().map(|p| PlanetId::new(p.as_str())).collect())
    }

    /// The landings of `invasion::commit_options` over `troops` standing in space and `planets`, rendered:
    /// (id, kind, label, payload) per option.
    #[must_use]
    pub fn commit_options(&self, system: &SystemId, troops: &[StrUnit], planets: &[PlanetId]) -> Option<Vec<OptionRow>> {
        use ti4_engine_core::choice::invasion::Ask as IAsk;
        use ti4_engine_core::choice::Ask as AnyAsk;
        let sys = typed::<System>(system.as_str())?;
        let pieces: Vec<Piece> = troops.iter().map(|u| self.piece(u)).collect::<Option<_>>()?;
        let planets: Vec<Planet> = planets.iter().map(|p| typed::<Planet>(p.as_str())).collect::<Option<_>>()?;
        let opts = ci::commit_opts(&pieces, &planets);
        let r = crate::render::Render::new(Vec::new());
        let any = AnyAsk::Invasion(IAsk::CommitGround { system: sys });
        Some(
            opts.iter()
                .map(|o| {
                    let o = crate::render::option(&r, &any, o);
                    (o.id, o.kind, o.label, o.payload)
                })
                .collect(),
        )
    }
}
