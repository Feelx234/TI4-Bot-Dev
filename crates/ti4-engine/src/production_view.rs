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
use crate::core_adapter::typed;
use crate::generated::IdStr;

/// What the production reads need.
pub const READS: SliceSet = SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Marks, Slice::Strategy, Slice::Cards, Slice::Trade, Slice::Transient, Slice::Explore]);

/// The typed view of one position.
pub struct ProdView<'a> {
    view: std::rc::Rc<ti4_engine_core::state::CoreState>,
    cx: Ctx,
    scope: Scope,
    blanked: bool,
    kernel: &'static Kernel,
    _position: std::marker::PhantomData<&'a GameState>,
}

/// The factions whose string modules still answer production questions with hooks the typed seams do not
/// carry yet (package CB): the typed reads decline and the caller keeps the string answer.
pub(crate) fn unported_economy(state: &GameState) -> bool {
    use crate::factions::{bastion, crimson};
    let held = [
        bastion::MODULE.alias,
        crimson::MODULE.alias,
    ];
    state.players.iter().any(|p| held.contains(&p.faction.as_str()))
}

/// Whether the producing seat or a seat that can act on it holds a faction whose production hooks the typed
/// window does not carry yet, for the windows that pay and place themselves (the invasion's custodians and toll
/// payments): the seat's own (Deepwrought oceans, Keleres commodities, Naaz mechs, Mentak's Corsair form) and the
/// cross-seat ones (Yssaril's borrowed agents, Nomad's agent watch; Argent's agent is an ability of the core
/// table), on top of [`unported_economy`].
pub(crate) fn unported_window(state: &GameState, player: &PlayerId) -> bool {
    use crate::factions::{deepwrought, keleres, mentak, naaz, nomad, yssaril};
    let own = [
        deepwrought::MODULE.alias,
        keleres::MODULE_M.alias,
        keleres::MODULE_X.alias,
        keleres::MODULE_A.alias,
        naaz::MODULE.alias,
        mentak::MODULE.alias,
    ];
    let cross = [nomad::MODULE.alias, yssaril::MODULE.alias];
    unported_economy(state)
        || state.player(player).is_some_and(|p| own.contains(&p.faction.as_str()))
        || state.players.iter().any(|p| cross.contains(&p.faction.as_str()))
}

/// [`unported_window`], and any Keleres seat at the table: the triggered productions (Sling Relay, Integrated
/// Economy) pay through the shared payment window, which offers a Keleres holder the agent when the payer holds
/// commodities (the tier-W offer stays string).
pub(crate) fn unported_triggered(state: &GameState, player: &PlayerId) -> bool {
    use crate::factions::keleres;
    let keleres = [keleres::MODULE_M.alias, keleres::MODULE_X.alias, keleres::MODULE_A.alias];
    unported_window(state, player) || state.players.iter().any(|p| keleres.contains(&p.faction.as_str()))
}

/// The same question for the production window and its payment steps, which follow the table's abilities and carry
/// the faction hooks of the seats of Deepwrought, Keleres, Naaz, Nomad and Yssaril (the lockstep tests
/// `the_tier_w_factions_run_in_lockstep_on_the_core_production_window` and the golden dual). Left to the string: the
/// Last Bastion and the Crimson (`unported_economy`) and Mentak's Corsair form.
pub(crate) fn unported_production(state: &GameState, player: &PlayerId) -> bool {
    use crate::factions::mentak;
    unported_economy(state) || state.player(player).is_some_and(|p| p.faction.as_str() == mentak::MODULE.alias)
}

/// The kernel of the production reads: the seams of the laws and relics, and the bridge to the legacy
/// hook slots (empty here: every faction that fills one is declined by [`unported_economy`]). Built once.
pub(crate) fn kernel() -> &'static Kernel {
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


impl<'a> ProdView<'a> {
    /// The view of a position; `None` when something is not mapped.
    #[must_use]
    pub fn of(state: &'a GameState, _content: &'a ContentStore, sources: SourceSet) -> Option<Self> {
        if unported_economy(state) {
            return None;
        }
        let mut cx = Ctx::of(state).ok()?;
        let view = std::rc::Rc::new(CoreView::of(state, &mut cx, Sources { galaxy: None::<&Galaxy>, rng: None }, READS).ok()?.into_state());
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
        Cx::new(&*self.view, self.kernel, self.scope, self.blanked)
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
                    Spot::Remote(system, at) => format!("{}{}{}", system.as_str(), crate::production::REMOTE_SEPARATOR, at.map_or(crate::production::SPACE, |p| p.as_str())),
                })
                .collect(),
        )
    }

    /// `production::war_machine_bonus`.
    #[must_use]
    pub fn war_machine_bonus(&self, player: &PlayerId) -> Option<i64> {
        Some(rules::war_machine_bonus(&*self.view, self.seat(player)?))
    }

    /// `production::price_of_under` with the law read from the position.
    #[must_use]
    pub fn price_of(&self, unit_id: &str) -> Option<(i64, usize)> {
        Some(rules::price_of(&self.rules_cx(), typed::<Unit>(unit_id)?))
    }

    fn oceans(&self, who: Seat) -> Vec<ti4_engine_core::generated::Planet> {
        ti4_engine_core_factions::cb_economy::deepwrought::query::spendable_oceans(&*self.view, who).as_slice().to_vec()
    }

    fn with_extras<T>(&self, who: Seat, f: impl FnOnce(&Cx<'_>, &rules::Extras<'_>) -> T) -> T {
        let oceans = self.oceans(who);
        let ex = rules::Extras {
            commodities: ti4_engine_core_factions::cb_economy::keleres::query::spendable_commodities(&*self.view, who),
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

    /// `production::payment_options`, rendered from the typed options.
    #[must_use]
    pub fn payment_options(&self, player: &PlayerId, kind: crate::production::Spend, paid: i64, cost: i64) -> Option<Vec<crate::choice::ChoiceOption>> {
        use ti4_engine_core::choice::{Ask as CoreAsk, Opt as CoreOpt};
        let who = self.seat(player)?;
        let (ask, opts) = self.with_extras(who, |cx, ex| rules::pay_opts(cx, who, core_spend(kind), paid, cost, ex));
        let render = crate::render::Render::default();
        let ask = CoreAsk::Production(ti4_engine_core::choice::production::Ask::Pay(ask));
        Some(opts.as_slice().iter().map(|o| crate::render::option(&render, &ask, &CoreOpt::Production(*o))).collect())
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
    // Mentak's Corsair form override (`unit_form_override`) is not a core hook yet.
    if unported_economy(state) || state.player(player).is_some_and(|p| p.faction.as_str() == crate::factions::mentak::MODULE.alias) {
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

/// Oceans and commodities of a seat as the payment reads need them.
fn extras_of(core: &ti4_engine_core::state::CoreState, who: Seat) -> (i64, Vec<ti4_engine_core::generated::Planet>) {
    (
        ti4_engine_core_factions::cb_economy::keleres::query::spendable_commodities(core, who),
        ti4_engine_core_factions::cb_economy::deepwrought::query::spendable_oceans(core, who).as_slice().to_vec(),
    )
}

/// The facts of one Choosing option in the position the renderer was given (`Render::at`).
#[must_use]
pub fn choosing_info(r: &crate::render::Render, ask: &ti4_engine_core::choice::production::ChooseAsk, unit: Unit) -> Option<rules::BuildInfo> {
    let pos = r.position()?;
    let core: &ti4_engine_core::state::CoreState = &pos.state;
    let scope = ti4_engine_core::tables::Scope(u32::from(pos.kernel.sources.0));
    let cx = Cx::new(core, pos.kernel, scope, ti4_engine_core::query::reach::blanked(core));
    let (commodities, oceans) = extras_of(core, ask.who);
    let ex = rules::Extras { commodities, oceans: &oceans };
    let before = rules::standing_before(&cx, ask.who, ask);
    let spendable = rules::available(&cx, ask.who, ti4_engine_core::query::economy::Spend::Resources, &ex);
    rules::build_for(&cx, ask.who, ask, unit, &ex, &before, spendable)
}

/// What a string `ProductionWindow` says about itself that the Choosing question depends on.
#[derive(Debug, Clone)]
pub struct ChooseInputs<'a> {
    pub system: &'a SystemId,
    pub limit: i64,
    pub remaining: i64,
    pub free_capacity: i64,
    pub small_exemptions: i64,
    pub credit: i64,
    pub discount_remaining: i64,
    pub fleet_limit: i64,
    pub free_this_use: bool,
    pub flagship_free: bool,
    pub only_unit: Option<&'a str>,
    pub max_unit_cost: Option<i64>,
    /// Ids of the units produced so far in this use.
    pub produced: Vec<&'a str>,
    pub armada: i64,
}

impl ProdView<'_> {
    /// The Choosing question's builds and exchanges (`choosing_options` without the Winnu agent option),
    /// rendered from the typed options. `None` when something is not mapped.
    #[must_use]
    pub fn build_options(&self, player: &PlayerId, inp: &ChooseInputs<'_>, buildable: &[String]) -> Option<Vec<crate::choice::ChoiceOption>> {
        use ti4_engine_core::choice::{Ask as CoreAsk, Opt as CoreOpt, production as cp};
        let who = self.seat(player)?;
        let mut produced = [0_u8; 11];
        for id in &inp.produced {
            let base = typed::<Unit>(id)?.data().base_type;
            produced[base as usize] = produced[base as usize].saturating_add(1);
        }
        let ask = cp::ChooseAsk {
            who,
            system: typed(inp.system.as_str())?,
            limit: i32::try_from(inp.limit).ok()?,
            remaining: i32::try_from(inp.remaining).ok()?,
            free_capacity: i32::try_from(inp.free_capacity).ok()?,
            small_exemptions: i32::try_from(inp.small_exemptions).ok()?,
            credit: i32::try_from(inp.credit).ok()?,
            discount_remaining: i32::try_from(inp.discount_remaining).ok()?,
            fleet_limit: i32::try_from(inp.fleet_limit).ok()?,
            free_this_use: inp.free_this_use,
            flagship_free: inp.flagship_free,
            only_unit: match inp.only_unit {
                Some(id) => Some(typed(id)?),
                None => None,
            },
            max_unit_cost: match inp.max_unit_cost {
                Some(m) => Some(i32::try_from(m).ok()?),
                None => None,
            },
            produced,
            armada: i32::try_from(inp.armada).ok()?,
            buildable: {
                let mut slots = [None; 9];
                for (slot, id) in slots.iter_mut().zip(buildable) {
                    *slot = Some(typed::<Unit>(id)?);
                }
                if buildable.len() > 9 {
                    return None;
                }
                slots
            },
        };
        let opts = self.with_extras(who, |cx, ex| rules::build_opts(cx, who, &ask, ex));
        let render = crate::render::Render::default();
        let position = crate::render::Position { state: self.view.clone(), kernel: self.kernel };
        let ask = CoreAsk::Production(cp::Ask::Choose(ask));
        Some(render.at(position, |r| opts.as_slice().iter().map(|o| crate::render::option(r, &ask, &CoreOpt::Production(*o))).collect()))
    }
}

/// The facts of one placement option in the renderer's position.
#[must_use]
pub fn placing_info(r: &crate::render::Render, ask: &ti4_engine_core::choice::production::PlaceAsk, spot: rules::Spot) -> Option<rules::PlaceInfo> {
    let pos = r.position()?;
    let core: &ti4_engine_core::state::CoreState = &pos.state;
    let scope = ti4_engine_core::tables::Scope(u32::from(pos.kernel.sources.0));
    let cx = Cx::new(core, pos.kernel, scope, ti4_engine_core::query::reach::blanked(core));
    Some(rules::place_info(&cx, ask, spot))
}

impl ProdView<'_> {
    /// The placement question's options, rendered from the typed ones: empty with fewer than two spots.
    #[must_use]
    pub fn placement_options(&self, player: &PlayerId, system: &SystemId, unit_id: &str, made: usize, fleet_limit: i64) -> Option<Vec<crate::choice::ChoiceOption>> {
        use ti4_engine_core::choice::{Ask as CoreAsk, Opt as CoreOpt, production as cp};
        let ask = cp::PlaceAsk {
            who: self.seat(player)?,
            system: typed(system.as_str())?,
            unit: typed(unit_id)?,
            made: u8::try_from(made).ok()?,
            fleet_limit: i32::try_from(fleet_limit).ok()?,
        };
        let opts = rules::place_opts(&self.rules_cx(), &ask);
        let render = crate::render::Render::default();
        let position = crate::render::Position { state: self.view.clone(), kernel: self.kernel };
        let ask = CoreAsk::Production(cp::Ask::Place(ask));
        Some(render.at(position, |r| opts.as_slice().iter().map(|o| crate::render::option(r, &ask, &CoreOpt::Production(*o))).collect()))
    }
}
