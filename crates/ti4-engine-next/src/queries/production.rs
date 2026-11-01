//! Production facts for the product (typed rewrite of the reads of `production.rs` of the legacy crate): the typed `ProdView` of
//! `production_view.rs` over the core's `rules::production`, with the faction hooks of the core kernel. A position the core cannot hold
//! answers the neutral value (0 / empty), where the legacy answers from its string rules: a refused position never reaches here.

use ti4_content::ContentStore;
use ti4_engine_core::generated::Unit;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::Scope;
use ti4_engine_core::rules::production::{self as rules, Cx, Spot};
use ti4_engine_core::state::{CoreState, Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;

use crate::core_sync::{CoreView, Ctx, Sources};
use crate::generated::IdStr;
use crate::ties::core_adapter::typed;
use crate::ties::production_view::kernel;

pub use crate::kinds::production::{AGENT_OPTION_ID, AGENT_OPTION_KIND, BUILDABLE, PAY_KIND, PLACE_KIND, PRODUCE_KIND, REMOTE_SEPARATOR, SPACE, UNLOCKED_BY};

/// The two things a planet card can be exhausted for (LRR 75.2, 47).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spend {
    Resources,
    Influence,
}

const fn core_spend(s: Spend) -> ti4_engine_core::query::economy::Spend {
    match s {
        Spend::Resources => ti4_engine_core::query::economy::Spend::Resources,
        Spend::Influence => ti4_engine_core::query::economy::Spend::Influence,
    }
}

/// What the production reads need.
const READS: SliceSet = SliceSet::of(&[
    Slice::Meta,
    Slice::Seats,
    Slice::Players,
    Slice::Board,
    Slice::Agenda,
    Slice::Marks,
    Slice::Strategy,
    Slice::Cards,
    Slice::Trade,
    Slice::Transient,
    Slice::Explore,
]);

/// The typed view of one position.
pub struct ProdView {
    view: CoreState,
    cx: Ctx,
    scope: Scope,
    blanked: bool,
    kernel: &'static Kernel,
}

impl ProdView {
    /// The view of a position; `None` when something is not mapped.
    #[must_use]
    pub fn of(state: &GameState, sources: SourceSet) -> Option<Self> {
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::of(state, &mut cx, Sources { galaxy: None, rng: None }, READS).ok()?.into_state();
        let blanked = ti4_engine_core::query::reach::blanked(&view);
        Some(Self { view, cx, scope: crate::ties::core_adapter::scope(sources), blanked, kernel: kernel() })
    }

    fn rules_cx(&self) -> Cx<'_> {
        Cx::new(&self.view, self.kernel, self.scope, self.blanked)
    }

    fn seat(&self, p: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(p.as_str())
    }

    /// `production::capacity` (68.1a).
    #[must_use]
    pub fn capacity(&self, player: &PlayerId, system: &SystemId) -> Option<i64> {
        Some(rules::capacity(&self.rules_cx(), self.seat(player)?, typed(system.as_str())?))
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
                    Spot::Space => SPACE.to_owned(),
                    Spot::Planet(p) => p.as_str().to_owned(),
                    Spot::Remote(system, at) => format!("{}{}{}", system.as_str(), REMOTE_SEPARATOR, at.map_or(SPACE, |p| p.as_str())),
                    Spot::Card => crate::render::production::spot_text(*s),
                })
                .collect(),
        )
    }

    /// `production::trade_good_worth`.
    #[must_use]
    pub fn trade_good_worth(&self, player: &PlayerId) -> Option<i64> {
        Some(rules::trade_good_worth(&self.rules_cx(), self.seat(player)?))
    }

    /// `production::buildable_for`: what a seat may produce, as unit ids (a war sun needs its technology unless the faction prints one; an
    /// owned upgrade replaces the unit; mechs and flagships exist only as the faction's own; a module may name another form, the kernel's
    /// `unit_form_override`). `None`: something is not in the tables.
    #[must_use]
    pub fn buildable_for(&self, state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId) -> Option<Vec<String>> {
        use ti4_engine_core::generated::{Faction, UnitBaseType};
        use ti4_engine_core::query::tech as q;
        let (seat, who) = (state.player(player)?, self.seat(player)?);
        let faction: Faction = typed(seat.faction.as_str())?;
        let mut owned = ti4_engine_core::state::TechSet::new();
        for tech in &seat.technologies {
            owned.insert(crate::ties::technology_core::tech(tech.as_str())?);
        }
        let scope = crate::ties::technology_core::scope(sources);
        let roster = crate::ties::technology_core::roster_for(content, sources)?;
        let mut out = Vec::new();
        for base_id in BUILDABLE {
            let base: UnitBaseType = typed(base_id)?;
            let own = q::faction_unit(roster, faction, base);
            if base_id == "warsun" {
                let has = seat.technologies.iter().any(|t| t.as_str() == "ws" || t.as_str().ends_with("ws"));
                if !has && own.is_none() {
                    continue;
                }
            }
            let chosen = q::unlocked_upgrade(scope, base, faction, &owned).or(own);
            let chosen_id = chosen.map(|u| IdStr::as_str(u).to_owned()).or_else(|| (!matches!(base_id, "mech" | "flagship")).then(|| base_id.to_owned()));
            // A module may name another form for this base type (Mentak's Corsair, The Table's Grace): the kernel's hook folds them all.
            let overridden = typed::<Unit>(chosen_id.as_deref().unwrap_or_default())
                .and_then(|unit| self.kernel.hooks.strategy.unit_form_override(&self.view, self.kernel, who, base, unit))
                .map(|form| IdStr::as_str(form).to_owned());
            out.extend(overridden.or(chosen_id));
        }
        Some(out)
    }

    /// `production::native_payment_planets`: the ready planets that pay `kind` in their own currency, each with the worth of that face, as the
    /// payment loop would list them for an unbounded bill. Cross-source faces (Archon's Gift) are left out.
    #[must_use]
    pub fn native_payment_planets(&self, player: &PlayerId, kind: Spend) -> Option<Vec<(PlanetId, i64)>> {
        let who = self.seat(player)?;
        let oceans = ti4_engine_core_factions::cb_economy::deepwrought::query::spendable_oceans(&self.view, who).as_slice().to_vec();
        let ex = rules::Extras { commodities: 0, oceans: &oceans };
        let cx = self.rules_cx();
        Some(
            rules::spendable_planets(&cx, who, &ex)
                .as_slice()
                .iter()
                .filter_map(|planet| {
                    rules::payment_faces(&cx, who, *planet, core_spend(kind))
                        .as_slice()
                        .iter()
                        .find(|(source, _)| *source == core_spend(kind))
                        .map(|(_, worth)| (PlanetId::new(planet.as_str()), *worth))
                })
                .collect(),
        )
    }

    /// `production::available`.
    #[must_use]
    pub fn available(&self, player: &PlayerId, kind: Spend) -> Option<i64> {
        let who = self.seat(player)?;
        let oceans = ti4_engine_core_factions::cb_economy::deepwrought::query::spendable_oceans(&self.view, who).as_slice().to_vec();
        let ex = rules::Extras { commodities: ti4_engine_core_factions::cb_economy::keleres::query::spendable_commodities(&self.view, who), oceans: &oceans };
        Some(rules::available(&self.rules_cx(), who, core_spend(kind), &ex))
    }
}

/// Spendable resources or influence, counting trade goods (LRR 75.3, 47.3).
#[must_use]
pub fn available(state: &GameState, _content: &ContentStore, sources: SourceSet, player: &PlayerId, kind: Spend) -> i64 {
    ProdView::of(state, sources).and_then(|v| v.available(player, kind)).unwrap_or(0)
}

/// 68.1a: the production values of all the player's producing units here, combined.
#[must_use]
pub fn capacity(state: &GameState, _content: &ContentStore, sources: SourceSet, player: &PlayerId, system: &SystemId) -> i64 {
    ProdView::of(state, sources).and_then(|v| v.capacity(player, system)).unwrap_or(0)
}

/// Where a produced unit may go. [`SPACE`] denotes the space area.
#[must_use]
pub fn placements(state: &GameState, _content: &ContentStore, sources: SourceSet, player: &PlayerId, system: &SystemId, kind: &ti4_content::units::UnitType<'_>) -> Vec<String> {
    ProdView::of(state, sources).and_then(|v| v.placements(player, system, kind.id())).unwrap_or_default()
}

/// What one trade good is worth as payment: two with the `mc` technology, else one.
#[must_use]
pub fn trade_good_worth(state: &GameState, player: &PlayerId) -> i64 {
    ProdView::of(state, ti4_model::content_types::FULL).and_then(|v| v.trade_good_worth(player)).unwrap_or(1)
}

/// The price under whatever laws are in play (Regulated Conscription halves the yield): `(resources, units made)`.
#[must_use]
pub fn price_of_under(state: Option<&GameState>, kind: &ti4_content::units::UnitType<'_>) -> (i64, usize) {
    let single = state.is_some_and(super::laws::single_unit_production);
    rules::price(<Unit as IdStr>::from_id(kind.id()).and_then(|u| u.data().cost_halves), single)
}

/// What this player can produce: a war sun needs its technology (or a faction that prints one), an owned upgrade replaces the unit it covers,
/// mechs and flagships exist only as the faction's own. Empty when the core cannot hold the position.
#[must_use]
pub fn buildable_for(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId) -> Vec<String> {
    ProdView::of(state, sources).and_then(|v| v.buildable_for(state, content, sources, player)).unwrap_or_default()
}

/// The ready planets that pay `kind` in their own currency, each with the worth of that face, exactly as the payment loop would list them for
/// an unbounded bill. Cross-source faces (Archon's Gift) are left out. Empty when the core cannot hold the position.
#[must_use]
pub fn native_payment_planets(state: &GameState, _content: &ContentStore, sources: SourceSet, player: &PlayerId, kind: Spend) -> Vec<(PlanetId, i64)> {
    ProdView::of(state, sources).and_then(|v| v.native_payment_planets(player, kind)).unwrap_or_default()
}

/// One thing that raises a planet's value, for display: `(source name, resources, influence)`: the laws attached to the planet, its attachment
/// cards, and the Helios docks. Their sum is what the value of the planet now exceeds its printed one by. The laws and the Helios bonus are the
/// core's; a position the core cannot hold lists the attachment cards only.
#[must_use]
pub fn planet_value_boosts(state: &GameState, content: &ContentStore, planet: &PlanetId) -> Vec<(String, i64, i64)> {
    use ti4_engine_core::query::laws::{ValueLaw, planet_value_bonus_sources};
    let view = ProdView::of(state, ti4_model::content_types::FULL);
    let typed_planet = typed::<ti4_engine_core::generated::Planet>(planet.as_str());
    let mut out: Vec<(String, i64, i64)> = match (&view, typed_planet) {
        (Some(v), Some(p)) => planet_value_bonus_sources(&v.view, v.blanked, p)
            .as_slice()
            .iter()
            .map(|(law, r, i)| {
                let name = match law {
                    ValueLaw::CoreMining => "Core Mining",
                    ValueLaw::SenateSanctuary => "Senate Sanctuary",
                    ValueLaw::TerraformingInitiative => "Terraforming Initiative",
                };
                (name.to_owned(), *r, *i)
            })
            .collect(),
        _ => Vec::new(),
    };
    for id in state.planet_attachments.get(planet).into_iter().flatten() {
        let Some(record) = content.get(ti4_model::content_types::ContentType::Attachments, id) else {
            continue;
        };
        let r = record.int("resourcesModifier").unwrap_or(0);
        let i = record.int("influenceModifier").unwrap_or(0);
        if r != 0 || i != 0 {
            out.push((record.text("name").unwrap_or(id).to_owned(), r, i));
        }
    }
    if let (Some(v), Some(p)) = (&view, typed_planet) {
        let helios = v.kernel.hooks.direct.bastion_units_resource_bonus(&v.view, p);
        if helios != 0 {
            out.push(("Helios dock".to_owned(), helios, 0));
        }
    }
    out
}
