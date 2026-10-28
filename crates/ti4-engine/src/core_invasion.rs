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
        let mut env = window_env(gs, &cx, sources)?;
        register_ground_slots(&mut env.kernel.hooks);
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

/// A bombardment die group as the dual compares it: the planet, the unit type, the hit number and the faces.
pub type BombardDiceRow = (String, String, u32, Vec<u32>);

impl InvasionView<'_> {
    /// `invasion::roll_bombard_plan`'s dice, drawn from the core's copy of the dice stream standing at
    /// `positions`: the die groups in plan order and where the streams stand afterwards.
    #[must_use]
    pub fn roll_bombard(
        &mut self,
        gs: &GameState,
        seed: u64,
        positions: &crate::rng::RngPositions,
        system: &SystemId,
        invader: &PlayerId,
    ) -> Option<(Vec<BombardDiceRow>, crate::rng::RngPositions)> {
        use ti4_engine_core::state::{Rng as CoreRng, RngDomain, SeedSource};
        let (sys, seat) = (typed::<System>(system.as_str())?, self.seat(invader)?);
        let mut typed_pos: Vec<(RngDomain, u128)> = Vec::new();
        for (t, p) in positions {
            typed_pos.push((crate::core_sync::codec::rng::domain(&mut self.cx, t).ok()?, *p));
        }
        let seeds = crate::core_sync::codec::rng::TextSeeds(&self.cx);
        let mut rng = CoreRng::new(seed);
        rng.set_positions(&typed_pos, &seeds as &dyn SeedSource);
        let plan = ci::bombard_plan(self.rd_for(gs, invader)?, sys, seat);
        let drawn = ci::roll_bombard(&plan, &mut rng, &seeds);
        let after = crate::core_sync::codec::rng::positions(&self.cx, &rng).ok()?;
        let rows = drawn.into_iter().map(|d| (d.planet.as_str().to_owned(), d.unit.as_str().to_owned(), d.hits_on, d.faces)).collect();
        Some((rows, after))
    }
}

/// A side's ground pools: per hit number the dice and the unit types behind them.
pub type PoolRows = std::collections::BTreeMap<i64, (i64, std::collections::BTreeMap<String, u32>)>;

/// A unit on a planet as the dual compares it: owner, type, damaged, galvanized.
pub type UnitRow = (String, String, bool, bool);

/// A ground hit as the dual compares it: 0 wasted, 1 sustained, 2 destroyed, and the unit.
pub type GroundHitRow = (u8, Option<StrUnit>);

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

    /// `invasion::apply_ground_hit_with_origin`'s decision for one hit on `player`'s ground forces on `planet`.
    #[must_use]
    pub fn ground_hit(&self, system: &SystemId, planet: &PlanetId, player: &PlayerId, unit_ability: bool) -> Option<GroundHitRow> {
        use ci::GroundHit as G;
        let (sys, pl, seat) = (typed::<System>(system.as_str())?, typed::<Planet>(planet.as_str())?, self.seat(player)?);
        Some(match ci::ground_hit(self.rd(), sys, pl, seat, unit_ability) {
            G::Sustained(p) => (1, Some(self.str_unit(p)?)),
            G::Destroyed(p) => (2, Some(self.str_unit(p)?)),
            G::Wasted => (0, None),
        })
    }

    /// `invasion::take_bombard_hits`: how many ground forces `produced` hits destroy, and the planet's units
    /// after (owner, type, damaged, galvanized).
    #[must_use]
    pub fn take_bombard_hits(&self, system: &SystemId, planet: &PlanetId, owner: &PlayerId, produced: usize) -> Option<(usize, Vec<UnitRow>)> {
        let (sys, pl, seat) = (typed::<System>(system.as_str())?, typed::<Planet>(planet.as_str())?, self.seat(owner)?);
        let (after, destroyed) = ci::take_hits(self.rd(), sys, pl, seat, produced, true);
        let rows = after
            .board()
            .planet(pl)
            .units
            .iter()
            .map(|p| Some((self.player(p.owner())?.to_string(), p.kind().as_str().to_owned(), p.sustained(), p.galvanized())))
            .collect::<Option<Vec<_>>>()?;
        Some((destroyed, rows))
    }

    /// `invasion::ground_combat_value`.
    #[must_use]
    pub fn ground_combat_value(&self, player: &PlayerId, system: &SystemId, planet: &PlanetId, unit_type: &str) -> Option<Option<i64>> {
        let (sys, pl, seat, unit) = (typed::<System>(system.as_str())?, typed::<Planet>(planet.as_str())?, self.seat(player)?, typed::<Unit>(unit_type)?);
        Some(ci::ground_combat_value(self.rd(), seat, sys, pl, unit))
    }

    /// `invasion::ground_pools`.
    #[must_use]
    pub fn ground_pools(&self, player: &PlayerId, system: &SystemId, planet: &PlanetId) -> Option<PoolRows> {
        let (sys, pl, seat) = (typed::<System>(system.as_str())?, typed::<Planet>(planet.as_str())?, self.seat(player)?);
        Some(
            ci::ground_pools(self.rd(), seat, sys, pl)
                .into_iter()
                .map(|p| (p.value, (p.dice, p.types.iter().map(|(u, n)| (u.as_str().to_owned(), *n)).collect())))
                .collect(),
        )
    }

    /// `invasion::custodians_cost`.
    #[must_use]
    pub fn custodians_cost(&self, player: &PlayerId) -> Option<i64> {
        Some(ci::custodians_cost(self.rd(), self.seat(player)?))
    }

    /// The board-reading gate of `invasion::custodians_removable`.
    #[must_use]
    pub fn custodians_gate(&self, player: &PlayerId, system: &SystemId) -> Option<bool> {
        Some(ci::custodians_gate(self.rd(), self.seat(player)?, typed::<System>(system.as_str())?))
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

/// The ground hook slots of the faction packages (`ti4_engine_core_factions`), taken from a table of their own
/// so the seam providers the combat kernel carries are not registered twice. Only the Naalu Matriarch's
/// `temporary_ground_force` and the generic `may_sustain` prohibitions are still answered by the string modules (through the oracle of the combat bridge);
/// it dies with its shard.
fn register_ground_slots(h: &mut ti4_engine_core::kernel::hooks::HookTable) {
    use crate::core_combat::with_oracle;
    let mut native = ti4_engine_core::kernel::hooks::HookTable::new();
    ti4_engine_core_factions::register_all(&mut native);
    h.ground = native.ground;
    // The generic prohibitions of `factions::hooks_combat` (a law or card that takes SUSTAIN DAMAGE away) have no
    // native provider yet: the string answer, through the oracle of the combat bridge.
    h.ground.may_sustain.push(|_, _, unit| {
        with_oracle(|o| {
            o.with_unit(unit, |cu| {
                let cu = crate::factions::CombatUnit { context: if unit.ctx == ti4_engine_core::kernel::hooks::CombatCtx::Space { "space" } else { "ground" }, ..*cu };
                crate::factions::hooks_ground::may_sustain(&o.gs, ContentStore::embedded(), o.sources, &cu)
            })
        })
        .flatten()
        .unwrap_or(true)
    });
    h.ground.temporary_ground_force.push(|_, _, who, system, planet, piece| {
        with_oracle(|o| {
            let (player, unit) = (o.player(who)?, StrUnit {
                type_id: ti4_model::id::UnitTypeId::new(piece.kind().as_str()),
                owner: o.player(piece.owner())?,
                sustained_damage: piece.sustained(),
                galvanized: piece.galvanized(),
            });
            Some(crate::factions::hooks_ground::temporary_ground_force(
                &o.gs,
                ContentStore::embedded(),
                o.sources,
                &player,
                &SystemId::new(system.as_str()),
                &PlanetId::new(planet.as_str()),
                &unit,
            ))
        })
        .flatten()
        .unwrap_or(false)
    });
}

/// [`window_env`] with the ground hook slots of the string faction modules (for the invasion windows).
#[must_use]
pub fn ground_env(gs: &GameState, cx: &Ctx, sources: SourceSet) -> Option<WindowEnv> {
    let mut env = window_env(gs, cx, sources)?;
    register_ground_slots(&mut env.kernel.hooks);
    Some(env)
}
