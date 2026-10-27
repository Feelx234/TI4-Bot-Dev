//! `Area::Production` (package B3): the typed production reads of `ti4_engine_core::rules::production`
//! behind the string interface. One [`ProdView`] is the typed view of one position plus the string
//! engine's answers to what the core cannot see yet (faction hooks, Ral Nel, Quietus, entropic scars,
//! space stations): it implements the core's `Env`. A text id the tables do not know makes a method
//! answer `None` and the caller keeps the string answer.

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::generated::{System, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::query::Scope;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::rules::production::{self as rules, Cx, Spot};
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::GameState;
use ti4_model::units::Unit as StrUnit;

use crate::core_sync::{CoreView, Ctx, Sources};
use crate::generated::IdStr;

/// What the production reads need.
pub const READS: SliceSet = SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Marks, Slice::Strategy, Slice::Cards, Slice::Trade, Slice::Transient]);

/// The typed view of one position.
pub struct ProdView<'a> {
    view: CoreView,
    cx: Ctx,
    scope: Scope,
    blanked: bool,
    kernel: &'static Kernel,
    _position: std::marker::PhantomData<&'a GameState>,
}

/// The factions whose string modules still answer production questions with hooks the typed seams do not
/// carry yet (package CB): the typed reads decline and the caller keeps the string answer.
fn unported_economy(state: &GameState) -> bool {
    use crate::factions::{arborec, bastion, crimson, ghost, mentak, muaat, naaz, ralnel, saar};
    let held = [
        arborec::MODULE.alias,
        bastion::MODULE.alias,
        crimson::MODULE.alias,
        ghost::MODULE.alias,
        mentak::MODULE.alias,
        muaat::MODULE.alias,
        naaz::MODULE.alias,
        ralnel::MODULE.alias,
        saar::MODULE.alias,
    ];
    state.players.iter().any(|p| held.contains(&p.faction.as_str()))
}

/// The kernel of the production reads: the seams of the laws and relics, and the bridge to the legacy
/// hook slots (empty here: every faction that fills one is declined by [`unported_economy`]). Built once.
fn kernel() -> &'static Kernel {
    static KERNEL: std::sync::OnceLock<Kernel> = std::sync::OnceLock::new();
    KERNEL.get_or_init(|| {
        let mut k = crate::core_adapter::kernel_for(ti4_model::content_types::FULL).clone();
        k.hooks = ti4_engine_core::hooks::HookTable::new();
        ti4_engine_core_factions::register_all(&mut k.hooks);
        ti4_engine_core::kernel::seam::providers::register_core(&mut k.hooks);
        ti4_engine_core::kernel::seam::adapters::bridge_legacy(&mut k.hooks.seam);
        k
    })
}

fn typed<T: IdStr>(id: &str) -> Option<T> {
    T::from_id(id)
}

impl<'a> ProdView<'a> {
    /// The view of a position; `None` when something is not mapped.
    #[must_use]
    pub fn of(state: &'a GameState, _content: &'a ContentStore, sources: SourceSet) -> Option<Self> {
        if unported_economy(state) {
            return None;
        }
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::of(state, &mut cx, Sources { galaxy: None::<&Galaxy>, rng: None }, READS).ok()?;
        Some(Self {
            view,
            cx,
            scope: crate::core_adapter::scope(sources),
            blanked: crate::laws::blanked(state),
            kernel: kernel(),
            _position: std::marker::PhantomData,
        })
    }

    fn rules_cx(&self) -> Cx<'_> {
        Cx::new(&self.view, self.kernel, self.scope, self.blanked)
    }

    fn seat(&self, p: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(p.as_str())
    }

    fn pid(&self, s: Seat) -> PlayerId {
        PlayerId::new(self.cx.owners.id(s).unwrap_or_default())
    }

    fn str_unit(&self, piece: ti4_engine_core::state::Piece) -> StrUnit {
        let mut u = StrUnit::new(UnitTypeId::new(piece.kind().as_str()), self.pid(piece.owner()));
        u.sustained_damage = piece.sustained();
        u.galvanized = piece.galvanized();
        u
    }

    /// `production::producers`.
    #[must_use]
    pub fn producers(&self, player: &PlayerId, system: &SystemId) -> Option<Vec<(StrUnit, Option<PlanetId>)>> {
        let (who, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        Some(
            rules::producers(&self.rules_cx(), who, sys)
                .as_slice()
                .iter()
                .map(|(p, pl)| (self.str_unit(*p), pl.map(|x| PlanetId::new(x.as_str()))))
                .collect(),
        )
    }

    /// `production::mobile_docks`.
    #[must_use]
    pub fn mobile_docks(&self, player: &PlayerId, system: &SystemId) -> Option<Vec<StrUnit>> {
        let (who, sys) = (self.seat(player)?, typed::<System>(system.as_str())?);
        Some(rules::mobile_docks(&self.rules_cx(), who, sys).as_slice().iter().map(|p| self.str_unit(*p)).collect())
    }

    /// `production::capacity`.
    #[must_use]
    pub fn capacity(&self, player: &PlayerId, system: &SystemId) -> Option<i64> {
        Some(rules::capacity(&self.rules_cx(), self.seat(player)?, typed(system.as_str())?))
    }

    /// `production::module_production`.
    #[must_use]
    pub fn module_production(&self, player: &PlayerId, system: &SystemId) -> Option<i64> {
        Some(rules::module_production(&self.rules_cx(), self.seat(player)?, typed(system.as_str())?))
    }

    /// `production::barred_capacity` for a unit base type.
    #[must_use]
    pub fn barred_capacity(&self, player: &PlayerId, system: &SystemId, base: &str) -> Option<i64> {
        Some(rules::barred_capacity(&self.rules_cx(), self.seat(player)?, typed(system.as_str())?, typed::<UnitBaseType>(base)?))
    }

    /// `production::structures_on`.
    #[must_use]
    pub fn structures_on(&self, player: &PlayerId, planet: &PlanetId, base: &str) -> Option<usize> {
        Some(rules::structures_on(&self.rules_cx(), self.seat(player)?, typed(planet.as_str())?, typed::<UnitBaseType>(base)?))
    }

    /// `production::structure_allowed`.
    #[must_use]
    pub fn structure_allowed(&self, player: &PlayerId, planet: &PlanetId, base: &str) -> Option<bool> {
        Some(rules::structure_allowed(&self.rules_cx(), self.seat(player)?, typed(planet.as_str())?, typed::<UnitBaseType>(base)?))
    }

    /// `production::placements`, as the string spots (`space` or the planet id).
    #[must_use]
    pub fn placements(&self, player: &PlayerId, system: &SystemId, unit_id: &str) -> Option<Vec<String>> {
        let spots = rules::placements(&self.rules_cx(), self.seat(player)?, typed(system.as_str())?, typed::<Unit>(unit_id)?);
        Some(
            spots
                .as_slice()
                .iter()
                .map(|s| match s {
                    Spot::Space => crate::production::SPACE.to_owned(),
                    Spot::Planet(p) => p.as_str().to_owned(),
                })
                .collect(),
        )
    }

    /// `production::war_machine_bonus`.
    #[must_use]
    pub fn war_machine_bonus(&self, player: &PlayerId) -> Option<i64> {
        Some(rules::war_machine_bonus(&self.view, self.seat(player)?))
    }

    /// `production::price_of_under` with the law read from the position.
    #[must_use]
    pub fn price_of(&self, unit_id: &str) -> Option<(i64, usize)> {
        Some(rules::price_of(&self.rules_cx(), typed::<Unit>(unit_id)?))
    }

    fn oceans(&self, who: Seat) -> Vec<ti4_engine_core::generated::Planet> {
        ti4_engine_core_factions::cb_economy::deepwrought::query::spendable_oceans(&self.view, who).as_slice().to_vec()
    }

    fn with_extras<T>(&self, who: Seat, f: impl FnOnce(&Cx<'_>, &rules::Extras<'_>) -> T) -> T {
        let oceans = self.oceans(who);
        let ex = rules::Extras {
            commodities: ti4_engine_core_factions::cb_economy::keleres::query::spendable_commodities(&self.view, who),
            oceans: &oceans,
        };
        f(&self.rules_cx(), &ex)
    }

    /// `production::trade_good_worth`.
    #[must_use]
    pub fn trade_good_worth(&self, player: &PlayerId) -> Option<i64> {
        Some(rules::trade_good_worth(&self.rules_cx(), self.seat(player)?))
    }

    /// `production::available`.
    #[must_use]
    pub fn available(&self, player: &PlayerId, kind: crate::production::Spend) -> Option<i64> {
        let who = self.seat(player)?;
        Some(self.with_extras(who, |cx, ex| rules::available(cx, who, core_spend(kind), ex)))
    }

    /// `production::payment_faces`.
    #[must_use]
    pub fn payment_faces(&self, player: &PlayerId, planet: &PlanetId, kind: crate::production::Spend) -> Option<Vec<(crate::production::Spend, i64)>> {
        let (who, pl) = (self.seat(player)?, typed(planet.as_str())?);
        Some(
            rules::payment_faces(&self.rules_cx(), who, pl, core_spend(kind))
                .as_slice()
                .iter()
                .map(|(k, w)| (engine_spend(*k), *w))
                .collect(),
        )
    }
}

const fn core_spend(s: crate::production::Spend) -> ti4_engine_core::query::economy::Spend {
    match s {
        crate::production::Spend::Resources => ti4_engine_core::query::economy::Spend::Resources,
        crate::production::Spend::Influence => ti4_engine_core::query::economy::Spend::Influence,
    }
}

const fn engine_spend(s: ti4_engine_core::query::economy::Spend) -> crate::production::Spend {
    match s {
        ti4_engine_core::query::economy::Spend::Resources => crate::production::Spend::Resources,
        ti4_engine_core::query::economy::Spend::Influence => crate::production::Spend::Influence,
    }
}

/// `production::buildable_for` over the typed tables: what a seat may produce, as unit ids. The gates and
/// the upgrade rules are the string's (a war sun needs its technology unless the faction prints one;
/// an owned upgrade replaces the unit; mechs and flagships exist only as the faction's own). `None`:
/// something is not in the tables, or a module could rename a form (Mentak's Corsair, whose seats the
/// view declines anyway).
#[must_use]
pub fn buildable_for(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId) -> Option<Vec<String>> {
    use ti4_engine_core::query::tech as q;
    if unported_economy(state) {
        return None;
    }
    let seat = state.player(player)?;
    let faction: ti4_engine_core::generated::Faction = typed(seat.faction.as_str())?;
    let owned = crate::technology_core::owned_set(seat)?;
    let scope = crate::technology_core::scope(sources);
    let roster = crate::technology_core::roster_for(content, sources)?;
    let mut out = Vec::new();
    for base_id in crate::production::BUILDABLE {
        let base: UnitBaseType = typed(base_id)?;
        let own = q::faction_unit(roster, faction, base);
        if base_id == "warsun" {
            let has = seat.technologies.iter().any(|t| t.as_str() == "ws" || t.as_str().ends_with("ws"));
            if !has && own.is_none() {
                continue;
            }
        }
        let chosen = q::unlocked_upgrade(scope, base, faction, &owned)
            .or(own)
            .map(|u| IdStr::as_str(u).to_owned())
            .or_else(|| (!matches!(base_id, "mech" | "flagship")).then(|| base_id.to_owned()));
        out.extend(chosen);
    }
    Some(out)
}
